use super::{
    ClaimIntent, Ctx, append_note, append_stamped_note, follow_holder, id_out, load, owner_name,
    save, transition,
};
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::output::Output;

pub fn start(mut ctx: Ctx, id: String, force: bool, reason: Option<String>) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    if task.status == Status::Shelved {
        return Err(Error::InvalidTransition(
            "shelved".into(),
            format!("doing (`tasks unshelve {id}` first)"),
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
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    let owner = owner_name(&ctx.project)?;
    task.owner = Some(owner.clone());
    crate::format::validate_task(&task)?;
    ctx.project.validate_docs(&task)?;
    crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, &task)?;
    crate::hierarchy::validate_periodic(&ctx.project, &ctx.registry, &task)?;
    crate::hierarchy::validate_defer(&ctx.project, &ctx.registry, &task)?;
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
            snapshot.authority().write_task(&ctx.registry, &halt)?;
        }
        append_note(&mut task, &owner, &target_note)?;
    }
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
    if reason.is_some() && !reason_used && !overriding_halt {
        ctx.warnings
            .push("--reason was unused because no override or takeover was needed".into());
    }
    save(&mut ctx, &mut task)?;
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
    let listed = match std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .env("LC_ALL", "C")
        .current_dir(&ctx.project.root)
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

pub fn note(mut ctx: Ctx, id: String, text: String, stamp: bool) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    if stamp {
        append_stamped_note(&mut ctx, &mut task, &owner, &text)?;
    } else {
        append_note(&mut task, &owner, &text)?;
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
    save(&mut ctx, &mut task)?;

    // Use the pruned store so a note cannot revive a stale claim.
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let mine = match &existing {
        Some(claim) => {
            crate::commands::ownership(claim, &me)? != crate::commands::Ownership::Foreign
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
    // Refresh only our live claim, and move it to this checkout (record-home spec §4).
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
    if let Some(message) = message
        && !ctx.recovered
    {
        let owner = owner_name(&ctx.project)?;
        append_stamped_note(&mut ctx, &mut task, &owner, &message)?;
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
pub fn shelve(mut ctx: Ctx, id: String, wake: String) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    crate::format::validate_line("wake condition", &wake)?;
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
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &format!("shelved: {wake}"))?;
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
