use super::{Ctx, apply_fields, id_out, load, save, transition};
use crate::attachments::parse_ledger_note;
use crate::cli::EditArgs;
use crate::error::{Error, Result};
use crate::format::parse_task;
use crate::model::{Note, Status, Task, TaskId};
use crate::output::Output;
use crate::resolve::{DocKind, Resolver};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn guard_new_start(ctx: &Ctx, task: &Task) -> Result<()> {
    let local = ctx.project.scan()?;
    let snapshot = crate::halt::snapshot(&ctx.project, &ctx.registry, &local)?;
    if snapshot.allows(task) {
        return Ok(());
    }
    let ids = snapshot
        .blocking(task)
        .iter()
        .map(|halt| halt.id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(Error::Halted(format!(
        "{} is stopped by halt {ids}; override with `tasks start {} --force --reason \"...\"`",
        task.id, task.id
    )))
}

pub fn check_invariants(original: &Task, edited: &Task) -> Result<()> {
    if edited.id != original.id {
        return Err(Error::Validation("id is immutable".into()));
    }
    if edited.created != original.created {
        return Err(Error::Validation("created is immutable".into()));
    }
    // Both stamps are written by `transition()` and by nothing else. Unlike `last_done`,
    // no other field's edit needs to clear them, so clearing is refused as well.
    if edited.started != original.started {
        return Err(Error::Validation(
            "started is stamped by starting the task; it cannot be edited".into(),
        ));
    }
    if edited.completed != original.completed {
        return Err(Error::Validation(
            "completed is stamped by completing the task; it cannot be edited".into(),
        ));
    }
    if !notes_unchanged_or_trimmed(&original.notes, &edited.notes) {
        return Err(Error::Validation(
            "notes are append-only; use `tasks note` (an edit may only remove a note's trailing spaces and tabs)"
                .into(),
        ));
    }
    // The anchor is stamped by a completion and by nothing else. Clearing it is allowed,
    // because clearing the cadence has to clear it; setting or moving it would move the
    // schedule with no occurrence behind it.
    if edited.last_done != original.last_done && edited.last_done.is_some() {
        return Err(Error::Validation(
            "last_done is stamped by completing the task; it cannot be edited".into(),
        ));
    }
    Ok(())
}

/// Spec §5.3: the one change the editor may make to notes is removing trailing spaces and
/// tabs from existing texts. Count, order, stamps, authors, and provenance stay, and so
/// does every attachment-ledger entry a text records.
fn notes_unchanged_or_trimmed(original: &[Note], edited: &[Note]) -> bool {
    original.len() == edited.len()
        && original.iter().zip(edited).all(|(old, new)| {
            old.at == new.at
                && old.by == new.by
                && old.provenance == new.provenance
                && (new.text == old.text || new.text == old.text.trim_end_matches([' ', '\t']))
                && parse_ledger_note(&old.text) == parse_ledger_note(&new.text)
        })
}

pub fn run(mut ctx: Ctx, id: String, mut args: EditArgs) -> Result<Output> {
    // `--status` conflicts with `--need` at the CLI, so the two uses never meet.
    let adds_needs = !args.fields.needs.is_empty();
    if args.force && args.status.as_deref() != Some("done") && !adds_needs {
        return Err(Error::Validation(
            "--force requires --status done, or --need to add a need another session holds".into(),
        ));
    }
    if args.reason.is_some() && !(args.force && adds_needs) {
        return Err(Error::Validation(
            "--reason requires --force and --need".into(),
        ));
    }
    if let Some(reason) = args.reason.as_deref() {
        crate::format::validate_line("reason", reason)?;
    }
    let fields = &args.fields;
    let has_flags = args.title.is_some()
        || args.status.is_some()
        || fields.body.is_some()
        || fields.priority.is_some()
        || fields.size.is_some()
        || fields.complexity.is_some()
        || args.no_complexity
        || fields.process.is_some()
        || args.no_process
        || fields.parallel
        || args.no_parallel
        || fields.every.is_some()
        || args.no_every
        || fields.defer.is_some()
        || args.no_defer
        || !fields.tags.is_empty()
        || !fields.depends.is_empty()
        || fields.spec.is_some()
        || fields.plan.is_some()
        || fields.step.is_some()
        || args.no_spec
        || args.no_plan
        || args.no_step
        || fields.parent.is_some()
        || args.no_parent
        || fields.source.is_some()
        || args.no_source
        || fields.agent.is_some()
        || args.no_agent
        || args.model.is_some()
        || args.no_model
        || args.no_tags
        || args.no_depends
        || !fields.needs.is_empty()
        || !args.rm_needs.is_empty()
        || args.no_needs
        || !args.rm_tags.is_empty();
    if !has_flags {
        return editor(ctx, id);
    }

    let mut task = load(&mut ctx, &id)?;
    let original_needs = task.needs.clone();
    if args.fields.body.as_deref() == Some("-") {
        let mut body = String::new();
        std::io::stdin().read_to_string(&mut body)?;
        args.fields.body = Some(body);
    }
    if let Some(title) = args.title {
        task.title = title;
    }
    if args.no_parent {
        task.parent = None;
    }
    if args.no_parallel {
        task.parallel = false;
    }
    if args.no_every {
        task.every = None;
        task.last_done = None;
    }
    if args.no_defer {
        task.defer = None;
    }
    if args.no_source {
        task.source = None;
    }
    if args.no_agent {
        task.agent = None;
    }
    if args.no_spec {
        task.spec = None;
    }
    if args.no_step {
        task.step = None;
    }
    if args.no_plan {
        if let Some(step) = &task.step {
            return Err(Error::Validation(format!(
                "{} links step {step:?} in its plan; clear it too with --no-step",
                task.id
            )));
        }
        task.plan = None;
    }
    if args.no_complexity {
        task.complexity = None;
    }
    if args.no_process {
        task.process = None;
    }
    if args.no_model {
        task.model = None;
    }
    if let Some(model) = &args.model {
        task.model = Some(model.clone());
    }
    // Clear, then remove, then let `apply_fields` append: `--no-tags --tag x` is the
    // wholesale replace `--tag` used to perform by itself, and `--no-depends --depends x`
    // the one `--depends` used to.
    if args.no_tags {
        task.tags.clear();
    }
    if args.no_depends {
        task.depends.clear();
    }
    for tag in &args.rm_tags {
        let before = task.tags.len();
        task.tags.retain(|existing| existing != tag);
        if task.tags.len() == before {
            return Err(Error::Validation(format!(
                "{} is not tagged {tag:?}",
                task.id
            )));
        }
    }
    // Mirrors the tag flags: clear, then remove, then `apply_fields` appends.
    if args.no_needs {
        task.needs.clear();
    }
    for need in &args.rm_needs {
        let before = task.needs.len();
        task.needs.retain(|existing| existing != need);
        if task.needs.len() == before {
            return Err(Error::Validation(format!(
                "{} does not need {need:?}",
                task.id
            )));
        }
    }
    apply_fields(&mut ctx, &mut task, &args.fields)?;
    // Lanes/needs design §4.4: needs change under no live claim or the caller's own, and
    // under the caller's own the claim's holds follow in the same save.
    // The target's override notes are appended here and land with its save. The
    // holders' notes are held in `holder_notes` until that save has succeeded.
    let mut holder_notes = None;
    if task.needs != original_needs {
        ctx.refuse_foreign_live_claim(&task.id)?;
        ctx.need_reason = args.reason.clone();
        ctx.update_holds(&task, args.force)?;
        holder_notes = super::record_need_overrides(&mut ctx, &mut task)?;
    }
    if args.reason.is_some() && holder_notes.is_none() {
        ctx.warnings
            .push("--reason was unused because no held need was added".into());
    }
    if let Some(status) = args.status {
        let to = Status::parse(&status)?;
        if to == task.status {
            ctx.refuse_foreign_live_claim(&task.id)?;
            ctx.preserve_claim_store(&task.id);
        } else {
            if to == Status::Shelved {
                refuse_shelving(&task.id)?;
            }
            if to == Status::Doing {
                guard_new_start(&ctx, &task)?;
            }
            transition(&mut ctx, &mut task, to, args.force)?;
        }
    }
    if args.fields.complexity.is_some() || args.no_complexity {
        // `task.complexity` already carries what was given: `--no-complexity` cleared it
        // above, `--complexity <level>` set it in `apply_fields`, and nothing since has
        // touched it.
        ctx.reassess(&task.id, task.complexity)?;
    }
    save(&mut ctx, &mut task)?;
    // Lanes/needs design §4.5: a holder learns of the override only once it has landed.
    // A save refused by its own validation (a parent cycle, say) writes no note anywhere.
    if let Some(notes) = holder_notes {
        super::note_need_holders(&mut ctx, notes);
    }
    super::follow_holder(&mut ctx, &task.id, None, "the edit landed");
    Ok(id_out(ctx, &task))
}

fn editor(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let (original, original_raw) = ctx.project.read_task_with_raw(&id)?;
    super::refuse_stale_copy(&mut ctx, &original, &original_raw, super::Writer::Command)?;
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

    // The raw-content comparison below protects this unlocked editing window.
    let routing = ctx.routing.clone();
    let original_root = ctx.project.root.clone();
    ctx.lock = None;
    ctx.claims = None;
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
    super::lock_and_revalidate(&mut ctx, &routing).map_err(keep)?;
    if ctx.project.prefix != original.id.prefix || ctx.project.root != original_root {
        return Err(keep(Error::ConcurrentModification(
            original.id.to_string(),
            "the project's identity changed while editing; retry".into(),
        )));
    }

    // Spec record-home §3.1: the lock was released while the editor was open, so another
    // checkout may have written since the first check.
    super::refuse_stale_copy(&mut ctx, &original, &original_raw, super::Writer::Command).map_err(
        |error| match error {
            Error::StaleCopy(detail) => Error::StaleCopy(format!(
                "A rerun opens a fresh editor on the newer copy, so copy your changes over \
                 from the kept file{suffix}. {detail}"
            )),
            error => keep(error),
        },
    )?;

    let edited_raw = std::fs::read_to_string(&tmp).map_err(|error| keep(error.into()))?;
    let mut edited = parse_task(&edited_raw, &tmp_display).map_err(keep)?;
    check_invariants(&original, &edited).map_err(keep)?;
    // `save` overwrites `updated`, so restoring it changes nothing in the record -- but it
    // is also the baseline `save` compares other checkouts against, and a hand-edited stamp
    // must not be able to talk that check out of firing.
    edited.updated = original.updated.clone();

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
    let from_main = resolver.take_warnings();
    ctx.warnings.extend(from_main);
    super::dep::ensure_acyclic(&ctx, &edited).map_err(keep)?;
    let status = edited.status;
    // spec §2.2: a save may change the status or the date, never both, since the
    // transition below clears the date and would silently discard a new one.
    if status != original.status && edited.defer != original.defer {
        return Err(keep(Error::Validation(
            "a save that changes the status cannot also change defer; change the status \
             first, then set the date"
                .into(),
        )));
    }
    // Lanes-needs spec §4.4: status and needs change in separate operations, as status
    // and defer do, so every acquire reads the needs already on the record.
    if status != original.status && edited.needs != original.needs {
        return Err(keep(Error::Validation(
            "a save that changes the status cannot also change needs; change the status \
             first, then the needs"
                .into(),
        )));
    }
    // spec §3.2: the status rule is the writers' to enforce. A status-changing save
    // reaches `transition`, which clears the field; an equal-status save must not leave
    // a deferral on a status that cannot carry one.
    if status == original.status && edited.defer.is_some() && !crate::defer::can_carry(status) {
        return Err(keep(Error::Validation(format!(
            "{} is {}; a deferral can only sit on an idea, todo, or blocked task",
            original.id,
            status.as_str()
        ))));
    }
    // Lanes-needs spec §4.2: only names this save adds must be declared (the grammar
    // was checked by `parse_task`); one the vocabulary has since dropped may stay or go.
    let added: Vec<String> = edited
        .needs
        .iter()
        .filter(|need| !original.needs.contains(*need))
        .cloned()
        .collect();
    crate::needs::require_declared(&ctx.project.needs, &added).map_err(keep)?;
    edited.status = original.status;
    if status == original.status {
        ctx.refuse_foreign_live_claim(&original.id).map_err(keep)?;
        ctx.preserve_claim_store(&original.id);
        // Lanes/needs design §4.4: no flags here, so a held need refuses and names
        // `tasks edit <id> --need <n> --force --reason`.
        if edited.needs != original.needs {
            ctx.update_holds(&edited, false).map_err(keep)?;
        }
    } else {
        if status == Status::Shelved {
            refuse_shelving(&original.id).map_err(keep)?;
        }
        if status == Status::Doing {
            guard_new_start(&ctx, &edited).map_err(keep)?;
        }
        transition(&mut ctx, &mut edited, status, false).map_err(keep)?;
    }

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
    save(&mut ctx, &mut edited).map_err(keep)?;
    super::follow_holder(&mut ctx, &edited.id, None, "the edit landed");
    if let Err(error) = std::fs::remove_file(&tmp) {
        ctx.warnings.push(format!(
            "edit saved, but could not remove {tmp_display}: {error}"
        ));
    }
    Ok(id_out(ctx, &edited))
}

/// Only `shelve` collects the wake condition required to enter the shelf.
fn refuse_shelving(id: &crate::model::TaskId) -> Result<()> {
    Err(Error::Validation(format!(
        "use `tasks shelve {id} \"<wake condition>\"` to shelve a task"
    )))
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

    fn note(text: &str) -> Note {
        Note {
            at: "2026-09-30T00:00:00Z".into(),
            by: "tester".into(),
            text: text.into(),
            provenance: None,
        }
    }

    #[test]
    fn a_trim_that_changes_a_ledger_entry_is_refused() {
        // Blank captions and reasons are refused at write time; a hand-written record can
        // still hold one, and trimming it would turn a ledger note into plain text.
        for text in ["attached: a.png (3 bytes): ", "detached: a.png:  "] {
            let trimmed = text.trim_end();
            assert!(parse_ledger_note(text).is_some(), "{text}");
            assert!(
                !notes_unchanged_or_trimmed(&[note(text)], &[note(trimmed)]),
                "{text}"
            );
        }
        assert!(notes_unchanged_or_trimmed(
            &[note("attached: a.png (3 bytes): shot \t")],
            &[note("attached: a.png (3 bytes): shot")],
        ));
    }

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
