use super::{Ctx, Ownership, ownership, parse_id, show};
use crate::claims::Liveness;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::shorthand::Shorthand;
use std::path::Path;

/// Reads `id` for a write and refuses when another checkout's copy must not be left behind
/// (docs/project-operations.md). Every write command loads through this or `load_with_raw`, under the mutation lock
/// and before it reads any other input, so a refusal consumes nothing and creates nothing.
pub fn load(ctx: &mut Ctx, id: &str) -> Result<Task> {
    load_for_writer(ctx, id, Writer::Command)
}

pub fn load_for_writer(ctx: &mut Ctx, id: &str, writer: Writer<'_>) -> Result<Task> {
    let id = parse_id(&ctx.registry, &ctx.shorthand, id)?;
    load_with_raw_for(ctx, &id, writer).map(|(task, _)| task)
}

/// The editor also needs the original bytes for its unlocked editing window.
pub(super) fn load_with_raw(ctx: &mut Ctx, id: &TaskId) -> Result<(Task, String)> {
    load_with_raw_for(ctx, id, Writer::Command)
}

fn load_with_raw_for(ctx: &mut Ctx, id: &TaskId, writer: Writer<'_>) -> Result<(Task, String)> {
    let (task, raw) = match ctx.project.read_task_with_raw(id) {
        Ok(found) => found,
        Err(Error::TaskNotFound(_)) => {
            // An id a pending run reserves but has not written yet is frozen too.
            crate::moving::journal::refuse_pending_id(&ctx.registry.canonical_id(id))?;
            return Err(missing_record(ctx, id)?);
        }
        Err(error) => return Err(error),
    };
    refuse_stale_copy(ctx, &task, &raw, writer)?;
    guard_write(&ctx.registry, &ctx.shorthand, &task)?;
    Ok((task, raw))
}

/// A missing local record names its recorded checkout, but never routes a write there.
fn missing_record(ctx: &mut Ctx, id: &TaskId) -> Result<Error> {
    let snapshot =
        crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (worktree, _, _, _) = show::recorded_elsewhere(id, &snapshot)?;
    let root = Path::new(worktree);
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let (work, unknown) = match occupants(ctx, root, None) {
        Ok(Occupants { work, unknown }) => (work, unknown),
        Err(error) => (Vec::new(), Some(error)),
    };
    let remedy = if work.is_empty() {
        crate::stale::Remedy::Rerun(&args)
    } else {
        crate::stale::Remedy::Occupied(work)
    };
    let next = crate::stale::remedy(id, root, remedy, unknown.as_ref());
    Ok(Error::TaskNotFoundNext {
        id: id.to_string(),
        next: format!("exists only in {worktree}; nothing was written. {next}"),
    })
}

/// The guard every record write passes before it mutates (docs/project-operations.md).
/// `load` calls it; so does each writer that reads a record by other means.
pub(crate) fn guard_write(registry: &Registry, shorthand: &Shorthand, task: &Task) -> Result<()> {
    crate::moving::journal::refuse_pending_id(&registry.canonical_id(&task.id))?;
    crate::tombstone::refuse_write(
        task,
        |arg| {
            shorthand
                .parse(registry, arg)
                .is_ok_and(|given| registry.canonical_id(&given) == task.id)
        },
        registry,
    )
}

/// Who is writing, which decides whether a `-C` retry can be offered (docs/task-model.md).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Writer<'a> {
    Command,
    /// `tasks note` (not a feedback recurrence): append-only and never moves a foreign claim.
    Note,
    /// A feedback recurrence: `-C` would change the reporting project.
    Feedback,
    /// `release` of this task's claim: the target's non-live claim (and, for non-`sid:<pid>`
    /// sessions, that session's other non-live claims) occupy no worktree, since the caller
    /// asserts the holder is gone (docs/ownership.md).
    Release {
        id: &'a crate::model::TaskId,
        session: &'a str,
    },
}

/// The record-home  check for `task` as loaded (`raw` is the bytes it was read from).
/// Its warnings are added once, however many times one command checks.
pub fn refuse_stale_copy(ctx: &mut Ctx, task: &Task, raw: &str, writer: Writer<'_>) -> Result<()> {
    let (newer, warnings) = crate::stale::compare(&ctx.project, &task.id, &task.updated, raw);
    for warning in warnings {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    let Some(newer) = newer else {
        return Ok(());
    };
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let gone = match writer {
        Writer::Release { id, session } => Some((id, session)),
        Writer::Command | Writer::Note | Writer::Feedback => None,
    };
    let (work, unknown) = match occupants(ctx, &newer.root, gone) {
        Ok(Occupants { work, unknown }) => (work, unknown),
        Err(error) => (Vec::new(), Some(error)),
    };
    // The sibling lookup just ran the same git query, so a failure here is not expected. If
    // it happens, only the handoff ordering is lost; the retry it falls back to is valid.
    let main = ctx.project.main_checkout_root().ok().flatten();
    let remedy = if !work.is_empty() {
        if writer == Writer::Note && !newer.same_stamp {
            crate::stale::Remedy::OccupiedNote(work, &args)
        } else {
            crate::stale::Remedy::Occupied(work)
        }
    } else if newer.same_stamp {
        crate::stale::Remedy::Merge
    } else if writer == Writer::Feedback {
        crate::stale::Remedy::New
    } else if main.as_ref() == Some(&newer.root) {
        crate::stale::Remedy::Handoff(&args)
    } else {
        crate::stale::Remedy::Rerun(&args)
    };
    // A refusal prints only its error object, so unknown ownership must travel in its
    // detail, before the remedy so the shell retry remains the final command.
    let detail = crate::stale::refusal(&task.id, &newer, &task.updated, remedy, unknown.as_ref());
    Err(Error::StaleCopy(detail))
}

struct Occupants {
    work: Vec<(String, String)>,
    unknown: Option<Error>,
}

/// docs/task-model.md rule 1: every task that a session other than the caller holds
/// non-stale (`Live` or `Unverifiable`) in `root`. Claims are compared by `ownership`.
/// A parked record is not an active occupant. `gone` is `(released_id, session)` for a
/// `release`: the released task's non-live claim occupies nothing, and for a non-`sid:<pid>`
/// session (`sid:1` can recur across sandboxes) its other non-live claims occupy nothing too.
fn occupants(
    ctx: &mut Ctx,
    root: &Path,
    gone: Option<(&crate::model::TaskId, &str)>,
) -> Result<Occupants> {
    let snapshot =
        crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let me = crate::claims::resolve_identity(&ctx.host, &mut ctx.warnings);
    let released_key = gone.map(|(id, _)| id.to_string());
    let mut work = Vec::new();
    for (task, (claim, liveness)) in snapshot.iter() {
        let released = match (&released_key, gone) {
            (Some(target), Some((_, session))) => {
                !matches!(liveness, Liveness::Live)
                    && (task == target
                        || (!super::is_unix_session(session) && claim.session == session))
            }
            _ => false,
        };
        if !liveness.is_stale()
            && !released
            && Path::new(&claim.worktree) == root
            && ownership(claim, &me, &ctx.host) == Ownership::Foreign
        {
            work.push((task.clone(), claim.session.clone()));
        }
    }
    let unknown = match me {
        crate::claims::Resolution::Failed(error) => Some(error),
        crate::claims::Resolution::Resolved(_) => None,
    };
    Ok(Occupants { work, unknown })
}
