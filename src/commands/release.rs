//! `tasks release`: remove another session's claim that this command cannot judge live,
//! and record why, without a lifecycle (claim-release spec).

use super::{ClaimIntent, Ctx, Ownership, Writer};
use crate::claims::{Claim, ClaimStore, Liveness, MutationLock};
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;
use std::path::Path;

/// The refusal hint for a held claim that reads `Unverifiable` (spec §6).
pub fn hint(id: &str, session: &str) -> String {
    format!("; if session {session} is gone, run `tasks release {id} \"<why>\"`")
}

/// The same, for a claim adoption finds in the old prefix's store (spec §6).
pub fn adopting_hint(renamed: &str, session: &str, old: &str) -> String {
    format!(
        "; if session {session} is gone, run `tasks release {renamed} \"<why>\" --adopting {old}`"
    )
}

/// `--adopting <old>`: the old prefix, its store, and its lock, held to the command's end.
pub struct Adopting {
    old: String,
    store: ClaimStore,
    _lock: MutationLock,
}

/// The write context. Under `--adopting`, both prefixes' locks in sorted order, as
/// `rename --adopt` takes them, and only for a predecessor adoption would accept (§1).
pub fn open(
    dir: Option<&Path>,
    id: &str,
    adopting: Option<&str>,
) -> Result<(Ctx, Option<Adopting>)> {
    let Some(old) = adopting else {
        return Ok((super::open_id_write_ctx(dir, id)?, None));
    };
    // Checked before any lock: `old` names a lock file, so a path-like value could name
    // another prefix's lock, and a second lock on one prefix would wait on itself.
    if !crate::model::is_valid_prefix(old) {
        return Err(Error::Validation(format!(
            "--adopting {old}: not a valid prefix (2-8 lowercase letters and digits, a letter first)"
        )));
    }
    let mut ctx = super::open_id_ctx(dir, id)?;
    let prefix = ctx.project.prefix.clone();
    if old == prefix {
        return Err(Error::Validation(format!(
            "--adopting {old} names this checkout's own prefix; it takes the prefix being adopted"
        )));
    }
    // A first check before locking spares a typo a lock file; only the one under both
    // locks below counts.
    eligible(&ctx, old)?;
    let early = if old < prefix.as_str() {
        Some(MutationLock::acquire(old)?)
    } else {
        None
    };
    let routing = ctx.routing.clone();
    super::lock_and_revalidate(&mut ctx, &routing)?;
    if ctx.project.prefix != prefix {
        return Err(Error::Validation(format!(
            "--adopting {old}: this checkout's prefix changed from {prefix:?} while locking; retry"
        )));
    }
    let lock = match early {
        Some(lock) => lock,
        None => MutationLock::acquire(old)?,
    };
    // Both locks are held: `init --force` and `rename` take a prefix's lock before they
    // change its root, so a registry read now stays true to the end of the command.
    ctx.registry = crate::registry::Registry::load()?;
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

/// Spec §1: `old` is a predecessor adoption would take for this checkout.
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

/// Spec §1: the reason is the one thing the record keeps that the tool cannot know.
fn require_reason(why: &str) -> Result<()> {
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
            let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
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
            let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
            adopting.store.remove(&key);
            adopting
                .store
                .save()
                .map_err(|error| error.with_suffix(&landed(&task.id)))?;
            Ok(super::id_out(ctx, &task))
        }
    }
}

/// Spec §2: refuse an own or live claim; return the verdict that permits the release.
/// `adopting` is `(old, routed id)` under `--adopting`, whose refusals name adoption,
/// never the routed id's `park`, `done` or `start --force`, which act on the other store,
/// and never promise that the holder's end clears the claim.
fn judge(
    ctx: &mut Ctx,
    claim: &Claim,
    key: &TaskId,
    adopting: Option<(&str, &TaskId)>,
) -> Result<Liveness> {
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let live = crate::claims::liveness(claim);
    let own = super::ownership(claim, &me)? != Ownership::Foreign;
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

/// Spec §3–§5: load the record past the stale-copy check that skips this session's
/// claims, append the stamped note, and write the record without touching the store.
fn write_note(
    ctx: &mut Ctx,
    id: &TaskId,
    claim: &Claim,
    live: &Liveness,
    why: &str,
) -> Result<Task> {
    let (mut task, raw) = ctx
        .project
        .read_task_with_raw(id)
        .map_err(|error| match error {
            Error::TaskNotFound(_) => error.with_suffix(&elsewhere(ctx, id, claim)),
            other => other,
        })?;
    super::refuse_stale_copy(ctx, &task, &raw, Writer::Release(&claim.session))?;
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
/// the rerun there is the way forward (a hint elsewhere may have sent the person here).
fn elsewhere(ctx: &Ctx, id: &TaskId, claim: &Claim) -> String {
    let worktree = Path::new(&claim.worktree);
    if worktree != ctx.project.root && worktree.join(format!("tasks/{id}.md")).is_file() {
        format!(
            "; the claim stays. Its record is in {0}: rerun this release there with \
             `tasks -C {0} release {id} \"<why>\"`",
            claim.worktree
        )
    } else {
        "; the claim stays".to_string()
    }
}

fn landed(id: &TaskId) -> String {
    format!(
        "; the note on {id} landed but its claim was not removed; rerun the same \
         `tasks release` command"
    )
}
