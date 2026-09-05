use super::{Ctx, append_note, id_out, load, owner_name, save, transition};
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::output::Output;

pub fn start(mut ctx: Ctx, id: String, force: bool) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let before = ctx.warnings.len();
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    let owner = owner_name(&ctx.project)?;
    task.owner = Some(owner.clone());
    // A takeover displaces someone; the task's own record should say so, not just the
    // ephemeral warning stream.
    for takeover in &ctx.warnings[before..] {
        append_note(&mut task, &owner, takeover)?;
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

pub fn note(mut ctx: Ctx, id: String, text: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &text)?;
    // Identity and the store are resolved *before* the file write. Doing it afterwards
    // means an unresolvable identity or a corrupt store returns an error after the note has
    // already landed, and the obvious retry then duplicates it.
    let me = crate::claims::identity()?;
    ctx.claims_mut()?;
    save(&mut ctx, &mut task)?;

    // Use the pruned store so a note cannot revive a stale claim.
    let mine = ctx
        .claims_mut()?
        .get(&task.id)
        .cloned()
        .filter(|claim| claim.session == me.session);

    // The heartbeat, and only on our own claim: `note` never touches a foreign one and is
    // never refused. It is still serialized under the mutation lock, because a note rewrites
    // the whole markdown file however append-only it is in meaning.
    if let Some(claim) = mine {
        let store = ctx.claims_mut()?;
        store.insert(
            &task.id,
            crate::claims::Claim {
                seen: crate::time::now(),
                ..claim
            },
        );
        if let Err(error) = store.save() {
            // The note is on disk, so this cannot be an error — say plainly what did and
            // did not happen, as the release-failure path does.
            ctx.warnings.push(format!(
                "the note landed, but the claim heartbeat on {} was not refreshed \
                 ({error}); the claim may look stale to other sessions",
                task.id
            ));
        }
    }
    Ok(id_out(ctx, &task))
}

pub fn close(
    mut ctx: Ctx,
    id: String,
    to: Status,
    message: Option<String>,
    force: bool,
) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    transition(&mut ctx, &mut task, to, force)?;
    if let Some(message) = message {
        let owner = owner_name(&ctx.project)?;
        append_note(&mut task, &owner, &message)?;
    }
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn block(ctx: Ctx, id: String, message: Option<String>) -> Result<Output> {
    close(ctx, id, Status::Blocked, message, false)
}

pub fn unblock(mut ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
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
