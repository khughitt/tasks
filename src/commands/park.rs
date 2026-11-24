use super::{ClaimIntent, Ctx, append_stamped_note, id_out, load, owner_name, save};
use crate::claims::{Escalation, Liveness, Needs, Park, Reason, WaitingOn, describe_stop};
use crate::decisions::{build_artifact_subject, find_resolution};
use crate::entries::{DecisionKind, parse_uuidv4};
use crate::error::{Error, Result};
use crate::model::{Complexity, Status};
use crate::output::Output;

pub struct ParkRequest {
    pub id: String,
    pub next_step: String,
    pub waiting_on: String,
    pub reason: Option<String>,
    pub complexity: Option<String>,
    pub needs: Option<String>,
    pub minutes: Option<u32>,
    pub decision: Option<String>,
    pub decision_kind: Option<String>,
    pub decision_origin: Option<String>,
    pub decision_text: Option<String>,
    pub artifact_repo: Option<String>,
    pub artifact_path: Option<String>,
    pub artifact_commit: Option<String>,
    pub artifact_sha256: Option<String>,
    pub by: Option<String>,
    pub source: Option<String>,
}

/// Set a task down (docs/ownership.md). Validate everything, refuse a foreign live claim, then let
/// `save` write the note and the entry in that order.
pub fn run(mut ctx: Ctx, req: ParkRequest) -> Result<Output> {
    let ParkRequest {
        id,
        next_step,
        waiting_on,
        reason,
        complexity,
        needs,
        minutes,
        decision,
        decision_kind,
        decision_origin,
        decision_text,
        artifact_repo,
        artifact_path,
        artifact_commit,
        artifact_sha256,
        by,
        source,
    } = req;
    let mut task = load(&mut ctx, &id)?;
    if !task.status.is_open() {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "parked".into(),
        ));
    }
    if task.status == Status::Shelved {
        return Err(Error::InvalidTransition(
            "shelved".into(),
            format!("parked (`tasks unshelve {id}` first)"),
        ));
    }
    crate::format::validate_line("next_step", &next_step)?;
    let waiting_on = WaitingOn::parse(&waiting_on)?;
    let reason = reason.as_deref().map(Reason::parse).transpose()?;
    let complexity = complexity.as_deref().map(Complexity::parse).transpose()?;
    let owner = owner_name(&ctx.project)?;
    let resolution = ctx.resolve_for_guard()?;
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let held = existing.as_ref().map(|claim| claim.session.clone());
    let ownership = existing
        .as_ref()
        .map(|claim| crate::commands::ownership(claim, &resolution, &ctx.host));
    let me = match (&existing, ownership) {
        (_, Some(crate::commands::Ownership::ByIdentity)) => resolution.require()?,
        (Some(claim), Some(crate::commands::Ownership::ByProof)) => {
            crate::claims::continuation_identity(claim)
        }
        (_, Some(crate::commands::Ownership::Foreign)) => resolution
            .require()
            .map_err(|error| crate::claims::name_the_claim(error, held.as_deref()))?,
        // Parking an unclaimed task records a session, so it is acquisition too.
        (None, None) => resolution.require()?,
        (None, Some(_)) | (Some(_), None) => unreachable!("ownership is mapped from existing"),
    };

    // The claim rules (docs/ownership.md), as a guard: a live or unverifiable foreign claim
    // refuses (no --force) with non-authorizing recovery hints, a stale one is taken over
    // with the warning `start` gives, and our own is simply replaced.
    let takeover = match (&existing, ownership) {
        (Some(existing), Some(crate::commands::Ownership::Foreign)) => {
            let live = crate::claims::liveness(existing);
            match live {
                Liveness::Live | Liveness::Unverifiable(_) => {
                    return Err(Error::Claimed(
                        task.id.to_string(),
                        ctx.foreign_claim_detail(&task.id, existing, &live, true),
                    ));
                }
                Liveness::Stale(_) => Some(format!(
                    "took over {}",
                    Ctx::describe_claim(existing, &live)
                )),
            }
        }
        _ => None,
    };
    if let Some(warning) = takeover {
        ctx.warnings.push(warning);
    }

    // Current contract (docs/ownership.md): --complexity belongs to --reason capability alone; the level never falls
    // below the effective rating; waiting on the agent it is required and must clear the
    // variable's cutoff, and an escalation is recorded; waiting on the user nothing is.
    let escalation = match (reason, complexity) {
        (Some(Reason::Capability), level) => {
            let escalated = ctx
                .claims_mut()?
                .escalation(&task.id)
                .map(|escalation| escalation.level);
            let current = crate::complexity::higher(task.complexity, escalated);
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
    let needs = needs.as_deref().map(Needs::parse).transpose()?;
    let (needs, minutes) = match (reason, needs, minutes) {
        (Some(Reason::Quiet), needs, Some(minutes)) => {
            (Some(needs.unwrap_or(Needs::Idle)), Some(minutes))
        }
        (Some(Reason::Quiet), _, None) => {
            return Err(Error::Validation(
                "--reason quiet needs --minutes <n>".into(),
            ));
        }
        (_, Some(_), _) => {
            return Err(Error::Validation(
                "--needs on park needs --reason quiet".into(),
            ));
        }
        (_, None, Some(_)) => {
            return Err(Error::Validation(
                "--minutes on park needs --reason quiet".into(),
            ));
        }
        (_, None, None) => (None, None),
    };

    // Stage B human-request park linkage:
    // `--waiting-on user --reason review|decision|approval` must link an open decision on this task
    // (`--decision <uuid>`) or open one inline (`--decision-kind/--decision-origin/--decision-text`).
    let is_human_request_park = waiting_on == WaitingOn::User
        && matches!(
            reason,
            Some(Reason::Review | Reason::Decision | Reason::Approval)
        );
    let any_decision_flag = decision.is_some()
        || decision_kind.is_some()
        || decision_origin.is_some()
        || decision_text.is_some()
        || artifact_repo.is_some()
        || artifact_path.is_some()
        || artifact_commit.is_some()
        || artifact_sha256.is_some()
        || by.is_some()
        || source.is_some();

    let decision_id = if !is_human_request_park {
        if any_decision_flag {
            return Err(Error::Validation(
                "decision flags on park are only valid with --waiting-on user --reason review|decision|approval".into(),
            ));
        }
        None
    } else if decision_kind.is_some() || decision_origin.is_some() || decision_text.is_some() {
        let Some(kind) = decision_kind else {
            return Err(Error::Validation(
                "inline decision open on park requires --decision-kind, --decision-origin, and --decision-text".into(),
            ));
        };
        let Some(origin) = decision_origin else {
            return Err(Error::Validation(
                "inline decision open on park requires --decision-kind, --decision-origin, and --decision-text".into(),
            ));
        };
        let Some(text) = decision_text else {
            return Err(Error::Validation(
                "inline decision open on park requires --decision-kind, --decision-origin, and --decision-text".into(),
            ));
        };
        let opened_id = super::decision::prepare_inline_for_park(
            &mut ctx,
            &mut task,
            super::decision::InlineParkDecision {
                id: decision,
                kind,
                origin,
                text,
                artifact_repo,
                artifact_path,
                artifact_commit,
                artifact_sha256,
                by,
                source,
                require_approval: reason == Some(Reason::Approval),
            },
        )?;
        Some(opened_id)
    } else {
        let Some(raw_id) = decision else {
            return Err(Error::Validation(
                "parking with --waiting-on user --reason review|decision|approval requires --decision <uuid> or inline --decision-kind/--decision-origin/--decision-text".into(),
            ));
        };
        if by.is_some() || source.is_some() {
            return Err(Error::Validation(
                "--by and --source on park are only valid when opening a decision inline".into(),
            ));
        }
        let linked_id = parse_uuidv4(&raw_id)?;
        let expected_pin = build_artifact_subject(
            artifact_repo.as_deref(),
            artifact_path.as_deref(),
            artifact_commit.as_deref(),
            artifact_sha256.as_deref(),
        )?;
        let Some(entry) = task.entries.iter().find(|e| e.id == linked_id) else {
            return Err(Error::DecisionNotFound(format!(
                "decision {linked_id} not found on {} (cannot link park to a missing or wrong-task decision)",
                task.id
            )));
        };
        let Some(data) = entry.as_decision() else {
            return Err(Error::DecisionConflict(format!(
                "entry {linked_id} on {} is not a decision",
                task.id
            )));
        };
        if !task.open_decisions.iter().any(|id| id == &linked_id)
            || find_resolution(&task, &linked_id).is_some()
        {
            return Err(Error::DecisionConflict(format!(
                "decision {linked_id} on {} is already resolved and cannot be linked to a park",
                task.id
            )));
        }
        if reason == Some(Reason::Approval) && data.kind != DecisionKind::Approval {
            return Err(Error::Validation(format!(
                "--reason approval requires linking an approval decision, got {}",
                data.kind.as_str()
            )));
        }
        if let Some(pin) = expected_pin
            && entry.subject != pin
        {
            return Err(Error::Validation(format!(
                "decision {linked_id} on {} does not match the supplied artifact pin",
                task.id
            )));
        }
        Some(linked_id)
    };

    if let Some(level) = complexity {
        task.complexity = Some(level);
    }

    let stop_summary = match &decision_id {
        Some(did) => format!(
            "parked (waiting on {}, decision {did}): {next_step}",
            describe_stop(waiting_on, reason, needs, minutes)
        ),
        None => format!(
            "parked (waiting on {}): {next_step}",
            describe_stop(waiting_on, reason, needs, minutes)
        ),
    };
    append_stamped_note(&mut ctx, &mut task, &owner, &stop_summary)?;
    let resolver = crate::resolve::Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let cited =
        crate::related::new_citations(&ctx.registry, Some(&task.body), &task.body, &[&next_step]);
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
    let park = Park {
        owner,
        session: me.tagged,
        host: crate::claims::hostname(),
        worktree: ctx.project.resume_root().display().to_string(),
        at: crate::time::now(),
        next_step,
        waiting_on,
        reason,
        needs,
        minutes,
        decision_id,
        title: task.title.clone(),
    };
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::Park { park, escalation }));
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
