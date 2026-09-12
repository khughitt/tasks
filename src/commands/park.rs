use super::{ClaimIntent, Ctx, append_note, id_out, load, owner_name, save};
use crate::claims::{Escalation, Liveness, Park, Reason, WaitingOn, describe_stop};
use crate::error::{Error, Result};
use crate::model::Complexity;
use crate::output::Output;

/// Set a task down (spec §3). Validate everything, refuse a foreign live claim, then let
/// `save` write the note and the entry in that order.
pub fn run(
    mut ctx: Ctx,
    id: String,
    next_step: String,
    waiting_on: String,
    reason: Option<String>,
    complexity: Option<String>,
) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    if !task.status.is_open() {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "parked".into(),
        ));
    }
    crate::format::validate_line("next_step", &next_step)?;
    let waiting_on = WaitingOn::parse(&waiting_on)?;
    let reason = reason.as_deref().map(Reason::parse).transpose()?;
    let complexity = complexity.as_deref().map(Complexity::parse).transpose()?;
    let owner = owner_name(&ctx.project)?;
    let me = crate::claims::identity()?;

    // The claim rules of §3.1, as a guard: a live foreign claim refuses (no --force), a
    // stale one is taken over with the warning `start` gives, our own is simply replaced.
    let takeover = {
        let store = ctx.claims_mut()?;
        match store.get(&task.id) {
            Some(existing) if existing.session != me.session => {
                let live = crate::claims::liveness(existing);
                let description = Ctx::describe_claim(existing, &live);
                match live {
                    Liveness::Live => {
                        return Err(Error::Claimed(task.id.to_string(), description));
                    }
                    Liveness::Stale(_) => Some(format!("took over {description}")),
                }
            }
            _ => None,
        }
    };
    if let Some(warning) = takeover {
        ctx.warnings.push(warning);
    }

    // Spec §5: --complexity belongs to --reason capability alone; the level never falls
    // below the effective rating; waiting on the agent it is required and must clear the
    // variable's cutoff, and an escalation is recorded; waiting on the user nothing is.
    let escalation = match (reason, complexity) {
        (Some(Reason::Capability), level) => {
            let escalated = ctx
                .claims_mut()?
                .escalation(&task.id)
                .map(|escalation| escalation.level);
            let current = match (task.complexity, escalated) {
                (Some(record), Some(escalated)) => Some(record.max(escalated)),
                (record, escalated) => record.or(escalated),
            };
            if let (Some(level), Some(current)) = (level, current)
                && level < current
            {
                return Err(Error::Validation(format!(
                    "--complexity {} is below the effective rating {} of {}",
                    level.as_str(),
                    current.as_str(),
                    task.id
                )));
            }
            match waiting_on {
                WaitingOn::User => None,
                WaitingOn::Agent => {
                    // Checked before requiring a level at all: under a high cutoff there is
                    // no level left to escalate to, so demanding --complexity first would
                    // send the caller looking for a level that cannot exist.
                    let cutoff = crate::complexity::cutoff(None)?;
                    if cutoff == Some(Complexity::High) {
                        return Err(Error::Validation(format!(
                            "{}=high leaves no level to escalate to; park --waiting-on user so a person can decompose or reassign it",
                            crate::complexity::ENV
                        )));
                    }
                    let level = level.ok_or_else(|| {
                        Error::Validation(
                            "--reason capability waiting on the agent needs --complexity <level>"
                                .into(),
                        )
                    })?;
                    if let Some(cutoff) = cutoff
                        && level <= cutoff
                    {
                        return Err(Error::Validation(format!(
                            "--complexity {} does not exceed {}={}",
                            level.as_str(),
                            crate::complexity::ENV,
                            cutoff.as_str()
                        )));
                    }
                    Some(Escalation {
                        level,
                        at: crate::time::now(),
                        session: me.tagged.clone(),
                    })
                }
            }
        }
        (_, Some(_)) => {
            return Err(Error::Validation(
                "--complexity on park needs --reason capability".into(),
            ));
        }
        (_, None) => None,
    };
    if let Some(level) = complexity {
        task.complexity = Some(level);
    }

    append_note(
        &mut task,
        &owner,
        &format!(
            "parked (waiting on {}): {next_step}",
            describe_stop(waiting_on, reason)
        ),
    )?;
    let park = Park {
        owner,
        session: me.tagged,
        host: crate::claims::hostname(),
        worktree: ctx.project.root.display().to_string(),
        at: crate::time::now(),
        next_step,
        waiting_on,
        reason,
        title: task.title.clone(),
    };
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::Park { park, escalation }));
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
