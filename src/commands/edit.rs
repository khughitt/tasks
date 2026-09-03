use super::{Ctx, apply_fields, id_out, load, save, transition};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::format::parse_task;
use crate::model::{Status, Task, TaskId};
use crate::output::Output;
use crate::resolve::{DocKind, Resolver};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub fn check_invariants(original: &Task, edited: &Task) -> Result<()> {
    if edited.id != original.id {
        return Err(Error::Validation("id is immutable".into()));
    }
    if edited.created != original.created {
        return Err(Error::Validation("created is immutable".into()));
    }
    if edited.notes != original.notes {
        return Err(Error::Validation(
            "notes are append-only; use `tasks note`".into(),
        ));
    }
    Ok(())
}

pub fn run(
    ctx: Ctx,
    id: String,
    title: Option<String>,
    status: Option<String>,
    force: bool,
    no_parent: bool,
    mut fields: FieldArgs,
) -> Result<Output> {
    if force && status.as_deref() != Some("done") {
        return Err(Error::Validation("--force requires --status done".into()));
    }
    let has_flags = title.is_some()
        || status.is_some()
        || fields.body.is_some()
        || fields.priority.is_some()
        || fields.size.is_some()
        || !fields.tags.is_empty()
        || !fields.depends.is_empty()
        || fields.spec.is_some()
        || fields.plan.is_some()
        || fields.step.is_some()
        || fields.parent.is_some()
        || no_parent;
    if !has_flags {
        return editor(ctx, id);
    }

    let mut task = load(&ctx, &id)?;
    if fields.body.as_deref() == Some("-") {
        let mut body = String::new();
        std::io::stdin().read_to_string(&mut body)?;
        fields.body = Some(body);
    }
    if let Some(title) = title {
        task.title = title;
    }
    if no_parent {
        task.parent = None;
    }
    apply_fields(&ctx, &mut task, &fields)?;
    if let Some(status) = status {
        transition(&ctx, &mut task, Status::parse(&status)?, force)?;
    }
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

fn editor(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let (original, original_raw) = ctx.project.read_task_with_raw(&id)?;
    let editor = std::env::var("EDITOR")
        .ok()
        .filter(|editor| !editor.is_empty())
        .ok_or_else(|| Error::Editor("EDITOR is not set".into()))?;
    let tmp = create_edit_temp(
        &ctx.project.tasks_dir(),
        &original.id,
        &original_raw,
        || fastrand::u32(..0x100_0000),
    )?;
    let tmp_display = tmp.display().to_string();
    let suffix = format!(" (edit kept at {tmp_display})");
    let keep = |error: Error| error.with_suffix(&suffix);

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("tasks-edit")
        .arg(&tmp)
        .status()
        .map_err(|error| keep(Error::Editor(format!("failed to run {editor}: {error}"))))?;
    if !status.success() {
        return Err(Error::Editor(format!(
            "{editor} exited with {status}; edit kept at {tmp_display}"
        )));
    }

    let edited_raw = std::fs::read_to_string(&tmp).map_err(|error| keep(error.into()))?;
    let mut edited = parse_task(&edited_raw, &tmp_display).map_err(keep)?;
    check_invariants(&original, &edited).map_err(keep)?;

    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    if let Some(spec) = &edited.spec {
        resolver.resolve_doc(DocKind::Spec, spec).map_err(keep)?;
    }
    if let Some(plan) = &edited.plan {
        resolver.resolve_doc(DocKind::Plan, plan).map_err(keep)?;
    }
    if let (Some(plan), Some(step)) = (&edited.plan, &edited.step)
        && !resolver.step_exists(plan, step).map_err(keep)?
    {
        return Err(keep(Error::Validation(format!(
            "heading {step:?} not found in {plan}"
        ))));
    }
    for dependency in &edited.depends {
        if resolver.resolve_task(dependency).map_err(keep)?.is_none() {
            return Err(keep(Error::Validation(format!(
                "dependency {dependency} is unreachable"
            ))));
        }
    }
    super::dep::ensure_acyclic(&ctx, &edited).map_err(keep)?;
    let status = edited.status;
    edited.status = original.status;
    transition(&ctx, &mut edited, status, false).map_err(keep)?;

    match ctx.project.read_raw(&original.id) {
        Ok(current) if current == original_raw => {}
        Ok(_) | Err(Error::TaskNotFound(_)) => {
            return Err(Error::ConcurrentModification(
                original.id.to_string(),
                format!("your edit is kept at {tmp_display}"),
            ));
        }
        Err(error) => return Err(keep(error)),
    }
    save(&ctx, &mut edited).map_err(keep)?;
    if let Err(error) = std::fs::remove_file(&tmp) {
        ctx.warnings.push(format!(
            "edit saved, but could not remove {tmp_display}: {error}"
        ));
    }
    Ok(id_out(ctx, &edited))
}

fn create_edit_temp(
    tasks_dir: &Path,
    id: &TaskId,
    raw: &str,
    mut candidate: impl FnMut() -> u32,
) -> Result<PathBuf> {
    for _ in 0..16 {
        let path = tasks_dir.join(format!(".{id}.{:06x}.edit.md", candidate()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(raw.as_bytes())?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(Error::Validation(
        "could not allocate an edit temp after 16 attempts".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_temp_retries_without_following_existing_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = dir.path().join("tasks");
        std::fs::create_dir(&tasks).unwrap();
        let id = TaskId::parse("sci-000001").unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, "original").unwrap();
        let existing = tasks.join(".sci-000001.000001.edit.md");
        std::os::unix::fs::symlink(&target, &existing).unwrap();

        let mut candidates = [1, 2].into_iter();
        let allocated =
            create_edit_temp(&tasks, &id, "edited", || candidates.next().unwrap()).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
        assert_eq!(std::fs::read_link(&existing).unwrap(), target);
        assert_eq!(allocated, tasks.join(".sci-000001.000002.edit.md"));
        assert_eq!(std::fs::read_to_string(allocated).unwrap(), "edited");
    }
}
