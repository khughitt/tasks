use super::{
    ClaimIntent, Ctx, append_note, append_stamped_note, follow_holder, id_out, load, owner_name,
    save, transition,
};
use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::output::Output;

pub fn start(mut ctx: Ctx, id: String, force: bool, reason: Option<String>) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    if task.status == Status::Shelved {
        return Err(Error::InvalidTransition(
            "shelved".into(),
            format!("doing (`tasks unshelve {id}` first)"),
        ));
    }
    // A closed task reopens only to todo; a closed recurrence reopens straight to doing.
    let reopens_to_todo = match task.status {
        Status::Done => task.every.is_none(),
        Status::Dropped => true,
        _ => false,
    };
    if reopens_to_todo {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            format!("doing (`tasks edit {id} --status todo` first)"),
        ));
    }
    if reason.is_some() && !force {
        return Err(Error::Validation("--reason requires --force".into()));
    }
    if let Some(reason) = reason.as_deref() {
        crate::format::validate_line("reason", reason)?;
    }
    let local = ctx.project.scan()?;
    let snapshot = crate::halt::snapshot(&ctx.project, &ctx.registry, &local)?;
    if force
        && !snapshot.halts().is_empty()
        && reason
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(Error::Validation(
            "--force under a halt requires --reason".into(),
        ));
    }
    let blockers = snapshot.blocking(&task);
    let overriding_halt = !blockers.is_empty();
    if !force && !blockers.is_empty() {
        let ids = blockers
            .iter()
            .map(|halt| halt.id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::Halted(format!(
            "{id} is stopped by halt {ids}; override with `tasks start {id} --force --reason \"...\"`"
        )));
    }
    // Current contract (docs/user-guide.md): the same reason lets --force past a held need.
    ctx.need_reason = reason.clone();
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    let owner = owner_name(&ctx.project)?;
    task.owner = Some(owner.clone());
    crate::format::validate_task(&task)?;
    ctx.project.validate_docs(&task)?;
    crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, &task)?;
    crate::hierarchy::validate_periodic(&ctx.project, &ctx.registry, &task)?;
    crate::hierarchy::validate_defer(&ctx.project, &ctx.registry, &task)?;
    crate::hierarchy::validate_lanes(&ctx.project, &ctx.registry, &task)?;
    let session = match &ctx.pending_claim {
        Some((_, ClaimIntent::Acquire(claim))) => claim.session.clone(),
        _ => unreachable!("start prepared an acquire claim"),
    };
    if !blockers.is_empty() {
        let reason = reason.as_deref().expect("override reason was validated");
        let ids = blockers
            .iter()
            .map(|halt| halt.id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let target_note = format!("halt override: started past {ids} by {session}: {reason}");
        crate::format::validate_note_text(&target_note)?;
        for mut halt in blockers.into_iter().cloned() {
            super::guard_write(&ctx.registry, &ctx.shorthand, &halt)?;
            append_note(
                &mut halt,
                &owner,
                &format!(
                    "halt override: attempted {} by {session}: {reason}",
                    task.id
                ),
            )?;
            halt.updated = crate::time::after(&halt.updated)?;
            crate::format::validate_task(&halt)?;
            snapshot.authority().validate_docs(&halt)?;
            // Append-only audit note: this authority write skips load's sibling stale-copy guard.
            // No retired check here: the halt is in this project, which already passed
            // `reject_retired` under the lock, so a retired project never reaches this write.
            snapshot.authority().write_task(&ctx.registry, &halt)?;
        }
        append_note(&mut task, &owner, &target_note)?;
    }
    let holder_notes = super::record_need_overrides(&mut ctx, &mut task)?;
    let overriding_need = holder_notes.is_some();
    // Persist the redacted takeover summary, adding the reason when supplied.
    let mut reason_used = false;
    if let Some(takeover) = ctx.takeover.take() {
        let note = if let Some(reason) = reason.as_deref() {
            reason_used = true;
            format!("{takeover}; reason: {reason}")
        } else {
            takeover
        };
        append_note(&mut task, &owner, &note)?;
    }
    if reason.is_some() && !reason_used && !overriding_halt && !overriding_need {
        ctx.warnings
            .push("--reason was unused because no override or takeover was needed".into());
    }
    save(&mut ctx, &mut task)?;
    // Only now that the acquire has landed may a holder be told it happened.
    if let Some(notes) = holder_notes {
        super::note_need_holders(&mut ctx, notes);
    }
    super::finish(&mut ctx);
    warn_if_uncommitted_with_worktrees(&mut ctx, &task);
    Ok(id_out(ctx, &task))
}

fn warn_if_uncommitted_with_worktrees(ctx: &mut Ctx, task: &Task) {
    let files = match ctx.project.uncommitted_task_files() {
        Ok(Some(files)) => files,
        Ok(None) => return,
        Err(error) => {
            ctx.warnings.push(format!(
                "task {} was started, but git could not inspect its uncommitted file ({error})",
                task.id
            ));
            return;
        }
    };
    let file = format!("tasks/{}.md", task.id);
    if !files.contains(&file) {
        return;
    }
    // A worktree branched after the record was committed holds it already: the change is
    // this start's own, and there is nothing left to commit before branching.
    match ctx.project.linked_worktree_head_has(&file) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            ctx.warnings.push(format!(
                "task {} was started, but git could not inspect its worktree's HEAD ({error})",
                task.id
            ));
            return;
        }
    }
    let listed = match crate::git::at(&ctx.project.records)
        .args(["worktree", "list", "--porcelain"])
        .output()
    {
        Ok(listed) => listed,
        Err(error) => {
            ctx.warnings.push(format!(
                "task {} was started, but git could not list worktrees ({error})",
                task.id
            ));
            return;
        }
    };
    if !listed.status.success() {
        ctx.warnings.push(format!(
            "task {} was started, but git worktree list failed ({}): {}",
            task.id,
            listed.status,
            String::from_utf8_lossy(&listed.stderr).trim()
        ));
        return;
    }
    let count = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| line.starts_with("worktree "))
        .count();
    if count > 1 {
        ctx.warnings.push(format!(
            "{file} is uncommitted and this repo has {count} worktrees; commit it before branching or the copies diverge"
        ));
    }
}

pub fn read_notes(reader: impl std::io::BufRead) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| {
            Error::Validation(format!(
                "stdin line {}: cannot read note: {error}",
                index + 1
            ))
        })?;
        let text = crate::format::normalize_note_text(&line)
            .map_err(|error| Error::Validation(format!("stdin line {}: {error}", index + 1)))?;
        texts.push(text.to_string());
    }
    if texts.is_empty() {
        return Err(Error::Validation(
            "stdin is empty; expected at least one note".into(),
        ));
    }
    Ok(texts)
}

pub fn note(
    mut ctx: Ctx,
    id: String,
    texts: Vec<String>,
    stamp: bool,
    review: crate::review::Review,
    awaiting: crate::review::Awaiting,
) -> Result<Output> {
    let mut task = super::load_for_writer(&mut ctx, &id, super::Writer::Note)?;
    crate::review::apply(&mut task, review, awaiting, &mut ctx.warnings)?;
    let owner = owner_name(&ctx.project)?;
    let at = crate::time::now();
    let mut duplicates = Vec::new();
    for text in &texts {
        let duplicate = crate::growth::first_duplicate(&task.notes, text);
        if stamp {
            append_stamped_note(&mut ctx, &mut task, &owner, text)?;
        } else {
            append_note(&mut task, &owner, text)?;
        }
        let note = task.notes.last_mut().expect("note appended");
        note.at.clone_from(&at);
        if let Some(index) = duplicate {
            let bytes = note.text.len();
            duplicates.push(format!(
                "the note landed; its {bytes} bytes duplicate prior logical note {}; link bulky detail in an attachment or records document, or use the approved checkpoint procedure",
                index + 1,
            ));
        }
    }
    // Identity and the store are resolved *before* the file write. Doing it afterwards
    // means a corrupt store returns an error after the note has already landed, and the
    // obvious retry then duplicates it.
    //
    // `resolve_for_guard` is what keeps the two failure kinds apart, and is why `note`
    // needs no special case of its own: with relay off it raises exactly where `identity`
    // raised before, so an unresolvable native identity is still fatal and the note still
    // does not land; with relay on it carries the failure, and the note lands.
    let me = ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    let resolver = crate::resolve::Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let note_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let cited =
        crate::related::new_citations(&ctx.registry, Some(&task.body), &task.body, &note_refs);
    let change = crate::related::RelatedChange::default();
    let link_warnings = crate::related::link_citations(
        &ctx.project,
        &ctx.registry,
        &resolver,
        &mut task,
        &cited,
        &change,
    )?;
    ctx.warnings.extend(link_warnings);
    save(&mut ctx, &mut task)?;
    ctx.warnings.extend(duplicates);

    // Use the pruned store so a note cannot revive a stale claim.
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let mine = match &existing {
        Some(claim) => {
            crate::commands::ownership(claim, &me, &ctx.host)?
                != crate::commands::Ownership::Foreign
        }
        None => false,
    };

    // The note has landed. If a claim exists that we could not establish ownership of
    // *because our own identity did not resolve*, say so: silence here would look
    // identical to an ordinary foreign note, which is a different situation entirely.
    if let Some(claim) = &existing
        && !mine
        && me.identity().is_none()
    {
        ctx.warnings.push(format!(
            "the note landed, but the claim heartbeat on {} was not refreshed (this \
             session's identity could not be resolved, so ownership of the claim held by \
             {} could not be established); the claim may look stale to other sessions",
            task.id, claim.session
        ));
    }
    // Refresh only our live claim, and move it to this checkout (docs/project-operations.md).
    follow_holder(&mut ctx, &task.id, Some(&me), "the note landed");
    Ok(id_out(ctx, &task))
}

pub fn close(
    mut ctx: Ctx,
    id: String,
    to: Status,
    message: Option<String>,
    force: bool,
) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    transition(&mut ctx, &mut task, to, force)?;
    if let Some(message) = &message
        && !ctx.recovered
    {
        let owner = owner_name(&ctx.project)?;
        append_stamped_note(&mut ctx, &mut task, &owner, message)?;
    }
    if let Some(message) = &message {
        let resolver = crate::resolve::Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
        let cited = crate::related::new_citations(
            &ctx.registry,
            Some(&task.body),
            &task.body,
            &[message.as_str()],
        );
        let change = crate::related::RelatedChange::default();
        let link_warnings = crate::related::link_citations(
            &ctx.project,
            &ctx.registry,
            &resolver,
            &mut task,
            &cited,
            &change,
        )?;
        ctx.warnings.extend(link_warnings);
    }
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn block(ctx: Ctx, id: String, message: Option<String>) -> Result<Output> {
    close(ctx, id, Status::Blocked, message, false)
}

pub fn unblock(mut ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    if task.status != Status::Blocked {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "todo (unblock requires blocked)".into(),
        ));
    }
    transition(&mut ctx, &mut task, Status::Todo, false)?;
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

/// A goal is shelved only after its open descendants are; `ready` reads each child's own
/// status, so shelving the goal alone would hide it while its children stayed eligible.
pub fn shelve(
    mut ctx: Ctx,
    id: String,
    wake: String,
    wake_on: Vec<String>,
    defer: String,
) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    crate::format::validate_line("wake condition", &wake)?;
    let checkpoint =
        crate::defer::Defer::resolve(&defer, crate::time::parse(&crate::time::now())?.date())?;
    let resolver = crate::resolve::Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let mut targets = Vec::new();
    for value in wake_on {
        let given = ctx.shorthand.parse(&ctx.registry, &value)?;
        let found =
            crate::dependency::resolve(&ctx.registry, &given, |id| resolver.resolve_task(id))?
                .ok_or_else(|| Error::UnresolvableId(given.to_string()))?;
        let target = ctx.registry.canonical_id(&found.id);
        if target == task.id {
            return Err(Error::Validation(format!(
                "{} cannot wake on itself",
                task.id
            )));
        }
        if targets.contains(&target) {
            continue;
        }
        if !found.status.is_open() {
            ctx.warnings.push(format!(
                "{target} is already closed; the shelf is immediately eligible for review"
            ));
        }
        targets.push(target);
    }
    let all = ctx.project.scan()?;
    let unshelved: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id, &ctx.registry)
        .iter()
        .filter(|descendant| descendant.status != Status::Shelved)
        .map(|descendant| descendant.id.to_string())
        .collect();
    if !unshelved.is_empty() {
        return Err(Error::OpenDescendants(
            task.id.to_string(),
            format!("{} (shelve or close them first)", unshelved.join(", ")),
        ));
    }
    transition(&mut ctx, &mut task, Status::Shelved, false)?;
    task.defer = Some(checkpoint);
    let dropped: Vec<TaskId> = task
        .wake_on
        .iter()
        .filter(|old| {
            let old_canonical = ctx.registry.canonical_id(old);
            !targets
                .iter()
                .any(|new| ctx.registry.canonical_id(new) == old_canonical)
        })
        .cloned()
        .collect();
    crate::related::carry_wake_targets(&ctx.registry, &mut task, dropped);
    task.wake_on = targets;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &format!("shelved: {wake}"))?;
    let resolver = crate::resolve::Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let cited =
        crate::related::new_citations(&ctx.registry, Some(&task.body), &task.body, &[&wake]);
    let change = crate::related::RelatedChange::default();
    let link_warnings = crate::related::link_citations(
        &ctx.project,
        &ctx.registry,
        &resolver,
        &mut task,
        &cited,
        &change,
    )?;
    ctx.warnings.extend(link_warnings);
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn unshelve(mut ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    if task.status != Status::Shelved {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "idea (unshelve requires shelved)".into(),
        ));
    }
    transition(&mut ctx, &mut task, Status::Idea, false)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, "unshelved")?;
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
