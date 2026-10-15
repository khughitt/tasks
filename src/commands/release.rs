//! `tasks release`: remove another session's claim that this command cannot judge live,
//! and record why, without a lifecycle (claim-release spec).

use super::{ClaimIntent, Ctx, Ownership, Writer};
use crate::claims::{Claim, Liveness};
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;

pub fn run(mut ctx: Ctx, id: String, why: String) -> Result<Output> {
    require_reason(&why)?;
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claim = ctx
        .claims_mut()?
        .get(&id)
        .cloned()
        .ok_or_else(|| Error::Validation(format!("{id} has no claim")))?;
    let live = judge(&mut ctx, &claim, &id)?;
    let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
    let store = ctx.claims_mut()?;
    store.remove(&id);
    store
        .save()
        .map_err(|error| error.with_suffix(&landed(&task.id)))?;
    Ok(super::id_out(ctx, &task))
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

/// Spec §2: refuse an own or live claim; return the verdict that permits the release.
fn judge(ctx: &mut Ctx, claim: &Claim, id: &TaskId) -> Result<Liveness> {
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    if super::ownership(claim, &me)? != Ownership::Foreign {
        return Err(Error::Validation(format!(
            "the claim on {id} is yours; `tasks park {id} \"<next step>\"` or `tasks done {id}` \
             releases it"
        )));
    }
    let live = crate::claims::liveness(claim);
    if matches!(live, Liveness::Live) {
        return Err(Error::Claimed(
            id.to_string(),
            format!(
                "{}; only its session releases a live claim; `tasks start --force {id}` takes \
                 it over",
                Ctx::describe_claim(claim, &live)
            ),
        ));
    }
    Ok(live)
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
    let (mut task, raw) = ctx.project.read_task_with_raw(id)?;
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

fn landed(id: &TaskId) -> String {
    format!(
        "; the note on {id} landed but its claim was not removed; rerun the same \
         `tasks release` command"
    )
}
