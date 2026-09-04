use super::{Ctx, append_note};
use crate::error::{Error, Result};
use crate::format::{validate_body, validate_line, validate_note_text, validate_task};
use crate::model::{Status, Task, TaskId};
use crate::output::{FeedbackOut, Output};
use crate::registry::Registry;
use crate::repo::Project;
use crate::scope::Origin;
use crate::similarity::Match;

pub const TARGET_PREFIX: &str = "tasks";
pub const CATEGORIES: [&str; 4] = ["friction", "gap", "idea", "positive"];

/// The upstream project: the registry entry whose prefix is `tasks`. The unregistered
/// case gets a hint the generic resolver cannot know: where the upstream lives.
pub fn locate_target(registry: &Registry) -> Result<Project> {
    if registry.project_root(TARGET_PREFIX).is_none() {
        return Err(Error::Config(format!(
            "no project registered as {TARGET_PREFIX:?}; clone the upstream tasks repository and run `tasks init` there"
        )));
    }
    crate::scope::open_registered(registry, TARGET_PREFIX, Origin::Prefix)
}

pub const NOTE_AUTHOR: &str = "feedback";

fn is_open_feedback(task: &Task) -> bool {
    task.status.is_open() && task.tags.iter().any(|tag| tag == "feedback")
}

pub fn run(
    mut ctx: Ctx,
    summary: String,
    category: String,
    body: Option<String>,
    recur: Option<String>,
    new: bool,
) -> Result<Output> {
    if !CATEGORIES.contains(&category.as_str()) {
        return Err(Error::Validation(format!(
            "category must be one of {}",
            CATEGORIES.join(", ")
        )));
    }
    validate_line("summary", &summary)?;
    if crate::similarity::tokens(&summary).is_empty() {
        return Err(Error::Validation(
            "summary needs at least one word of three or more letters or digits".into(),
        ));
    }
    let body = body.unwrap_or_default();
    validate_body(&body)?;
    let target = locate_target(&ctx.registry)?;
    let from = format!("from:{}", ctx.project.prefix);

    // (id, automatic): an automatic match must still carry the same title when written.
    // Only the automatic branch scans the target; --recur reads one task and --new reads
    // none, so an unrelated malformed file there cannot block an explicit request.
    let existing: Option<(TaskId, bool)> = match (&recur, new) {
        (Some(id), _) => {
            let id = TaskId::parse(id)?;
            let not_feedback = || {
                Error::Validation(format!(
                    "{id} is not an open feedback task in {TARGET_PREFIX:?}"
                ))
            };
            let task = match target.read_task(&id) {
                Ok(task) => task,
                Err(Error::TaskNotFound(_)) => return Err(not_feedback()),
                Err(error) => return Err(error),
            };
            if !is_open_feedback(&task) {
                return Err(not_feedback());
            }
            Some((id, false))
        }
        (None, true) => None,
        (None, false) => {
            let candidates: Vec<Task> = target
                .scan()?
                .into_iter()
                .filter(is_open_feedback)
                .collect();
            match crate::similarity::find(&summary, &candidates) {
                Match::Exact(id) => Some((id, true)),
                Match::None => None,
                Match::Ambiguous(candidates) => {
                    let listed: Vec<String> = candidates
                        .iter()
                        .map(|(id, title)| format!("{id} ({title:?})"))
                        .collect();
                    return Err(Error::Ambiguous(format!(
                        "matching open feedback exists: {}; rerun with --recur <id> or --new",
                        listed.join(", ")
                    )));
                }
            }
        }
    };
    let (task, action) = match existing {
        Some((id, automatic)) => (
            recur_into(
                &ctx, &target, &id, automatic, &summary, &body, &category, &from,
            )?,
            "recurred",
        ),
        None => (create(&target, summary, body, &category, &from)?, "created"),
    };
    let path = target.task_path(&task.id);
    ctx.warnings.push(format!(
        "filed as uncommitted {} in {}; a maintainer there reviews and commits it",
        path.display(),
        target.root.display()
    ));
    Ok(Output::Feedback(FeedbackOut {
        id: task.id.to_string(),
        action: action.into(),
        path: path.display().to_string(),
        warnings: ctx.warnings,
    }))
}

fn create(
    target: &Project,
    summary: String,
    body: String,
    category: &str,
    from: &str,
) -> Result<Task> {
    let now = crate::time::now();
    let mut task = Task {
        id: target.new_id()?,
        title: summary,
        status: Status::Idea,
        priority: 2,
        size: None,
        owner: None,
        created: now.clone(),
        updated: now,
        depends: vec![],
        parent: None,
        tags: vec!["feedback".into(), category.into(), from.into()],
        spec: None,
        plan: None,
        step: None,
        body,
        notes: vec![],
    };
    super::create(target, &mut task)?;
    Ok(task)
}

#[allow(clippy::too_many_arguments)]
fn recur_into(
    ctx: &Ctx,
    target: &Project,
    id: &TaskId,
    automatic: bool,
    summary: &str,
    body: &str,
    category: &str,
    from: &str,
) -> Result<Task> {
    if !body.is_empty() {
        validate_note_text(body)?;
    }
    // The match was made against a snapshot. Every guarded read re-checks that the task
    // is still open feedback and, for an automatic match, still has the same title, so a
    // task that was closed, retagged, or renamed in between is never appended to.
    let wanted = automatic.then(|| crate::similarity::tokens(summary));
    let eligible = move |task: &Task| -> Result<()> {
        if !is_open_feedback(task) {
            return Err(Error::Validation(format!(
                "{} is no longer open feedback; rerun to file afresh",
                task.id
            )));
        }
        if let Some(wanted) = &wanted
            && crate::similarity::tokens(&task.title) != *wanted
        {
            return Err(Error::Validation(format!(
                "{} was renamed since it matched; rerun",
                task.id
            )));
        }
        Ok(())
    };
    // Fixed author: the reporter's TASKS_OWNER, branch, or user name must not leak into
    // the public upstream file. The reporting project is already in the note text.
    let prefix = ctx.project.prefix.clone();
    guarded_update(target, id, eligible, |task| {
        append_note(
            task,
            NOTE_AUTHOR,
            &format!("feedback from {prefix}: {summary}"),
        )?;
        if !body.is_empty() {
            append_note(task, NOTE_AUTHOR, &format!("detail from {prefix}: {body}"))?;
        }
        for tag in [from, category] {
            if !task.tags.iter().any(|existing| existing == tag) {
                task.tags.push(tag.to_string());
            }
        }
        Ok(())
    })
}

/// Read, check `eligible`, mutate, and replace `id` in `target`, retrying from the read
/// when the file changed between the read and the moment before the replace. Gives up
/// after eight rounds with `concurrent_modification`. `eligible` runs on every read, so a
/// decision made on an earlier snapshot is re-validated against what is actually about
/// to be rewritten. The same checks as `save` run before the write: `validate_task`, the
/// project's doc roots, and, inside `write_task`, the parent.
pub fn guarded_update(
    target: &Project,
    id: &TaskId,
    eligible: impl Fn(&Task) -> Result<()>,
    mut mutate: impl FnMut(&mut Task) -> Result<()>,
) -> Result<Task> {
    for _ in 0..8 {
        let (mut task, raw) = target.read_task_with_raw(id)?;
        eligible(&task)?;
        mutate(&mut task)?;
        task.updated = crate::time::now(); // second precision: a same-second repeat keeps it
        validate_task(&task)?;
        target.validate_docs(&task)?;
        if target.read_raw(id)? != raw {
            continue;
        }
        target.write_task(&task)?;
        return Ok(task);
    }
    Err(Error::ConcurrentModification(
        id.to_string(),
        "another writer kept changing it; retry".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_update_retries_after_a_concurrent_write_and_gives_up_eventually() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::init(dir.path(), "tst").unwrap();
        let mut seed = create(&project, "seed".into(), String::new(), "gap", "from:tst").unwrap();
        let id = seed.id.clone();

        let always = |_: &Task| Ok(());
        let mut calls = 0;
        let task = guarded_update(&project, &id, always, |task| {
            calls += 1;
            if calls == 1 {
                seed.title = "changed underneath".into();
                project.write_task(&seed).unwrap();
            }
            task.tags.push(format!("round-{calls}"));
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(task.title, "changed underneath");
        assert_eq!(task.tags.last().unwrap(), "round-2");
        assert_eq!(
            project.read_task(&id).unwrap().tags.last().unwrap(),
            "round-2"
        );

        // eligibility is judged on the fresh read, not on the snapshot the caller had
        seed.status = crate::model::Status::Done;
        project.write_task(&seed).unwrap();
        let before = project.read_raw(&id).unwrap();
        let error = guarded_update(
            &project,
            &id,
            |task| {
                if task.status.is_open() {
                    Ok(())
                } else {
                    Err(Error::Validation("closed".into()))
                }
            },
            |task| {
                task.tags.push("never".into());
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), "validation");
        assert_eq!(project.read_raw(&id).unwrap(), before, "nothing written");
        seed.status = crate::model::Status::Idea;

        let mut rounds = 0;
        let error = guarded_update(&project, &id, always, |_| {
            rounds += 1;
            seed.priority = (rounds % 5) as u8;
            project.write_task(&seed).unwrap();
            Ok(())
        })
        .unwrap_err();
        assert_eq!(rounds, 8);
        assert_eq!(error.kind(), "concurrent_modification");
    }
}
