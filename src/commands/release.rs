//! `tasks release`: remove another session's claim that this command cannot judge live,
//! and record why, without a lifecycle (claim-release spec).

use super::{ClaimIntent, Ctx, Ownership, Writer};
use crate::claims::{Claim, ClaimStore, Liveness, MutationLock};
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;
use std::path::Path;
use std::sync::Arc;

/// The refusal hint for a held claim that reads `Unverifiable` (docs/task-model.md).
pub fn hint(id: &str, session: &str) -> String {
    format!("; if session {session} is gone, run `tasks release {id} \"<why>\"`")
}

/// `hint`, omitted when this caller owns `claim` (`tasks release` would refuse "yours").
pub(crate) fn hint_if_foreign(
    id: &str,
    claim: &Claim,
    me: &crate::claims::Resolution,
    host: &HostConfig,
) -> String {
    if super::ownership(claim, me, host) == Ownership::Foreign {
        hint(id, &claim.session)
    } else {
        String::new()
    }
}

/// The same, for a claim adoption finds in the old prefix's store (docs/task-model.md).
pub fn adopting_hint(renamed: &str, session: &str, old: &str) -> String {
    format!(
        "; if session {session} is gone, run `tasks release {renamed} \"<why>\" --adopting {old}`"
    )
}

/// `adopting_hint`, omitted when this caller owns `claim`.
pub(crate) fn adopting_hint_if_foreign(
    renamed: &str,
    claim: &Claim,
    old: &str,
    me: &crate::claims::Resolution,
    host: &HostConfig,
) -> String {
    if super::ownership(claim, me, host) == Ownership::Foreign {
        adopting_hint(renamed, &claim.session, old)
    } else {
        String::new()
    }
}

/// `--adopting <old>`: the old prefix, its store, and its lock, held to the command's end.
pub struct Adopting {
    old: String,
    store: ClaimStore,
    _lock: MutationLock,
}

/// The write context. Under `--adopting`, both prefixes' locks in sorted order, as
/// `rename --adopt` takes them, and only for a predecessor adoption would accept.
pub fn open(
    dir: Option<&Path>,
    id: &str,
    adopting: Option<&str>,
    host: Arc<HostConfig>,
) -> Result<(Ctx, Option<Adopting>)> {
    let Some(old) = adopting else {
        return Ok((super::open_id_write_ctx(dir, id, host.clone())?, None));
    };
    // Checked before any lock: `old` names a lock file, so a path-like value could name
    // another prefix's lock, and a second lock on one prefix would wait on itself.
    if !crate::model::is_valid_prefix(old) {
        return Err(Error::Validation(format!(
            "--adopting {old}: not a valid prefix (2-8 lowercase letters and digits, a letter first)"
        )));
    }
    let mut ctx = super::open_id_ctx(dir, id, host.clone())?;
    let same_prefix = |peer: &str| {
        Error::Validation(format!(
            "--adopting {peer} names this checkout's own prefix; it takes the prefix being adopted"
        ))
    };
    if old == ctx.project.prefix {
        return Err(same_prefix(old));
    }
    // A first check before locking spares a typo a lock file; only the one under both
    // locks below counts.
    eligible(&ctx, old)?;
    let routing = ctx.routing.clone();
    // Both locks are acquired in sorted order and dropped together on retry if the
    // checkout's prefix changes; `ctx.registry` is refreshed while both locks are held.
    let lock = super::lock_pair_and_revalidate(&mut ctx, &routing, old, same_prefix)?;
    eligible(&ctx, old)?;
    let store = ClaimStore::load(old)?;
    Ok((
        ctx,
        Some(Adopting {
            old: old.to_string(),
            store,
            _lock: lock,
        }),
    ))
}

/// Current contract (docs/task-model.md): `old` is a predecessor adoption would take for this checkout.
fn eligible(ctx: &Ctx, old: &str) -> Result<()> {
    match crate::rename::adopt::accepts(&ctx.registry, &ctx.project, old)? {
        None => Ok(()),
        Some(reason) => Err(Error::Validation(format!(
            "--adopting {old}: {reason}; it is accepted only while `tasks rename {old} {} \
             --adopt` would adopt it",
            ctx.project.prefix
        ))),
    }
}

/// Current contract (docs/task-model.md): the reason is the one thing the record keeps that the tool cannot know.
pub(super) fn require_reason(why: &str) -> Result<()> {
    if why.trim().is_empty() {
        return Err(Error::Validation(
            "tasks release needs a reason: why the holder is known to be gone".into(),
        ));
    }
    Ok(())
}

pub fn run(mut ctx: Ctx, adopting: Option<Adopting>, id: String, why: String) -> Result<Output> {
    require_reason(&why)?;
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    match adopting {
        None => {
            let claim = ctx
                .claims_mut()?
                .get(&id)
                .cloned()
                .ok_or_else(|| Error::Validation(format!("{id} has no claim")))?;
            let live = judge(&mut ctx, &claim, &id, None)?;
            let task = write_note(&mut ctx, &id, &claim, &live, &why, None)?;
            let store = ctx.claims_mut()?;
            store.remove(&id);
            store
                .save()
                .map_err(|error| error.with_suffix(&landed(&task.id)))?;
            Ok(super::id_out(ctx, &task))
        }
        Some(mut adopting) => {
            let key = TaskId::parse(&format!("{}-{}", adopting.old, id.hex))?;
            let claim = adopting
                .store
                .get(&key)
                .cloned()
                .ok_or_else(|| Error::Validation(format!("{key} has no claim")))?;
            let live = judge(&mut ctx, &claim, &key, Some((&adopting.old, &id)))?;
            let task = write_note(&mut ctx, &id, &claim, &live, &why, Some(&adopting.old))?;
            adopting.store.remove(&key);
            adopting
                .store
                .save()
                .map_err(|error| error.with_suffix(&landed(&task.id)))?;
            Ok(super::id_out(ctx, &task))
        }
    }
}

/// Current contract (docs/task-model.md): refuse an own or live claim; return the verdict that permits the release.
/// `adopting` is `(old, routed id)` under `--adopting`, whose refusals name adoption,
/// never the routed id's `park`, `done` or `start --force`, which act on the other store,
/// and never promise that the holder's end clears the claim.
fn judge(
    ctx: &mut Ctx,
    claim: &Claim,
    key: &TaskId,
    adopting: Option<(&str, &TaskId)>,
) -> Result<Liveness> {
    let me = crate::claims::resolve_identity(&ctx.host, &mut ctx.warnings);
    let live = crate::claims::liveness(claim);
    let own = super::ownership(claim, &me, &ctx.host) != Ownership::Foreign;
    match (own, matches!(live, Liveness::Live), adopting) {
        (true, _, None) => Err(Error::Validation(format!(
            "the claim on {key} is yours; `tasks park {key} \"<next step>\"` or `tasks done {key}` \
             releases it"
        ))),
        (true, _, Some((old, id))) => Err(Error::Validation(format!(
            "the claim on {key} is this session's, recorded before the rename. Once this session \
             ends, `tasks rename {old} {} --adopt` proceeds if the claim reads stale; if it \
             reads unverifiable, a later session releases it with `tasks release {id} \
             \"<why>\" --adopting {old}`",
            id.prefix
        ))),
        (false, true, None) => Err(Error::Claimed(
            key.to_string(),
            format!(
                "{}; only its session releases a live claim; `tasks start --force {key}` takes \
                 it over",
                Ctx::describe_claim(claim, &live)
            ),
        )),
        (false, true, Some((old, id))) => Err(Error::Claimed(
            key.to_string(),
            format!(
                "{}; the claim on {key} was recorded before the rename. Once session {} ends, \
                 `tasks rename {old} {} --adopt` proceeds if the claim reads stale; if it reads \
                 unverifiable, rerun this release",
                Ctx::describe_claim(claim, &live),
                claim.session,
                id.prefix
            ),
        )),
        (false, false, _) => Ok(live),
    }
}

/// Current contract (docs/task-model.md): load the record past the stale-copy check that skips this session's
/// claims, append the stamped note, and write the record without touching the store.
fn write_note(
    ctx: &mut Ctx,
    id: &TaskId,
    claim: &Claim,
    live: &Liveness,
    why: &str,
    adopting: Option<&str>,
) -> Result<Task> {
    let (mut task, raw) = ctx
        .project
        .read_task_with_raw(id)
        .map_err(|error| match error {
            Error::TaskNotFound(_) => Error::TaskNotFoundNext {
                id: id.to_string(),
                next: elsewhere(ctx, id, claim, adopting),
            },
            other => other,
        })?;
    super::refuse_stale_copy(
        ctx,
        &task,
        &raw,
        Writer::Release {
            id: &task.id,
            session: &claim.session,
        },
    )?;
    super::guard_write(&ctx.registry, &ctx.shorthand, &task)?;
    let owner = super::owner_name(&ctx.project)?;
    super::append_stamped_note(ctx, &mut task, &owner, &summary(claim, live, why))?;
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::PreserveStore));
    super::save(ctx, &mut task)?;
    Ok(task)
}

/// The note: `takeover_summary`'s redaction, the verdict, and the person's reason.
fn summary(claim: &Claim, live: &Liveness, why: &str) -> String {
    let verdict = match live {
        Liveness::Unverifiable(reason) => format!("unverifiable: {reason}"),
        // A stale reason can name a host pid; a record keeps none.
        Liveness::Stale(_) => "stale".to_string(),
        Liveness::Live => unreachable!("judge refuses a live claim"),
    };
    format!(
        "released {} (owner {}, {verdict}); reason: {why}",
        Ctx::session_label(claim),
        claim.owner
    )
}

/// A record missing here: the claim stays, and when the holder's worktree has the record,
/// name where it lives and the way forward (a hint elsewhere may have sent the person here).
fn elsewhere(ctx: &Ctx, id: &TaskId, claim: &Claim, adopting: Option<&str>) -> String {
    let here = ctx.project.resume_root();
    let worktree = Path::new(&claim.worktree);
    if worktree == here {
        return "the claim stays".to_string();
    }
    match adopting {
        None if worktree.join(format!("tasks/{id}.md")).is_file() => format!(
            "the claim stays. Its record is in {}: rerun this release there with \
             `tasks -C {} release {id} \"<why>\"`",
            claim.worktree,
            crate::stale::quote(&claim.worktree)
        ),
        Some(old) if worktree.join(format!("tasks/{id}.md")).is_file() => {
            let adoptable_there = crate::repo::Project::open(worktree)
                .ok()
                .and_then(|wt| crate::rename::adopt::accepts(&ctx.registry, &wt, old).ok())
                == Some(None);
            if adoptable_there {
                format!(
                    "the claim stays. Its record is in {}: rerun this release there with \
                     `tasks -C {} release {id} \"<why>\" --adopting {old}`",
                    claim.worktree,
                    crate::stale::quote(&claim.worktree)
                )
            } else {
                format!(
                    "the claim stays. Its record `tasks/{id}.md` is in {}: merge that branch \
                     into {}, then rerun this release",
                    claim.worktree,
                    here.display()
                )
            }
        }
        Some(old) => {
            let old_id = format!("{old}-{}", id.hex);
            if worktree.join(format!("tasks/{old_id}.md")).is_file() {
                format!(
                    "the claim stays. Its pre-rename record `tasks/{old_id}.md` is in {}: \
                     rebase or merge that worktree onto the rename and merge it into {}, then \
                     rerun this release",
                    claim.worktree,
                    here.display()
                )
            } else {
                "the claim stays".to_string()
            }
        }
        None => "the claim stays".to_string(),
    }
}

fn landed(id: &TaskId) -> String {
    format!(
        "; the note on {id} landed but its claim was not removed; rerun the same \
         `tasks release` command"
    )
}
