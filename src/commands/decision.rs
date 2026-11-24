//! `tasks decision open|answer|withdraw` (Stage A: named drop and suggestion decisions).

use super::{Ctx, append_stamped_note, load, owner_name, save, transition};
use crate::cli::DecisionAction;
use crate::decisions::{
    DependentsIndex, find_resolution, logical_decision, task_scope_fingerprint,
};
use crate::entries::{
    DecisionData, DecisionKind, Entry, EntryPayload, EntrySubject, ResolutionData,
    ResolutionOutcome, ScopeSubject, Subject, new_uuidv4, parse_uuidv4, upgrade_to_format_2,
    validate_entries,
};
use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::{DecisionOut, Output};
use crate::repo::Project;

pub fn dispatch(
    dir: Option<&std::path::Path>,
    action: DecisionAction,
    host: std::sync::Arc<crate::config::HostConfig>,
) -> Result<Output> {
    match action {
        DecisionAction::Open {
            task,
            id,
            kind,
            origin,
            text,
            scope_verdict,
            by,
            source,
            opened_at,
            opening_scope,
        } => {
            let ctx = super::open_id_write_ctx(dir, &task, host)?;
            open(
                ctx,
                OpenRequest {
                    task,
                    id,
                    kind,
                    origin,
                    text,
                    scope_verdict,
                    by,
                    source,
                    opened_at,
                    opening_scope,
                },
            )
        }
        DecisionAction::Answer {
            task,
            decision,
            accept,
            decline,
            seen_scope,
            by,
            source,
        } => {
            let ctx = super::open_id_write_ctx(dir, &task, host)?;
            let (outcome, text) = match (accept, decline) {
                (Some(text), None) => (ResolutionOutcome::Accept, text),
                (None, Some(text)) => (ResolutionOutcome::Decline, text),
                _ => {
                    return Err(Error::Validation(
                        "answer requires exactly one of --accept or --decline".into(),
                    ));
                }
            };
            resolve(
                ctx,
                ResolveRequest {
                    task,
                    decision,
                    outcome,
                    text,
                    seen_scope,
                    by,
                    source,
                    action_label: "answered",
                },
            )
        }
        DecisionAction::Withdraw {
            task,
            decision,
            reason,
            by,
            source,
        } => {
            let ctx = super::open_id_write_ctx(dir, &task, host)?;
            resolve(
                ctx,
                ResolveRequest {
                    task,
                    decision,
                    outcome: ResolutionOutcome::Withdraw,
                    text: reason,
                    seen_scope: None,
                    by,
                    source,
                    action_label: "withdrawn",
                },
            )
        }
    }
}

struct OpenRequest {
    task: String,
    id: Option<String>,
    kind: String,
    origin: String,
    text: String,
    scope_verdict: Option<String>,
    by: Option<String>,
    source: Option<String>,
    opened_at: Option<String>,
    opening_scope: Option<String>,
}

struct ResolveRequest {
    task: String,
    decision: String,
    outcome: ResolutionOutcome,
    text: String,
    seen_scope: Option<String>,
    by: Option<String>,
    source: Option<String>,
    action_label: &'static str,
}

fn validate_hex64(field: &str, value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Validation(format!(
            "{field} must be a 64-character lowercase hex digest"
        )));
    }
    Ok(())
}

fn validate_nonempty_multiline(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Validation(format!("{field} cannot be empty")));
    }
    Ok(())
}

fn resolve_actor_and_source(
    project: &Project,
    by: Option<String>,
    source: Option<String>,
    require_pair: bool,
) -> Result<(String, String)> {
    if require_pair && (by.is_none() || source.is_none()) {
        return Err(Error::Validation(
            "--opened-at requires both explicit --by and --source".into(),
        ));
    }
    if source.is_some() && by.is_none() {
        return Err(Error::Validation("--source requires explicit --by".into()));
    }
    let actor = match by {
        Some(actor) => {
            crate::format::validate_line("by", &actor)?;
            if actor.trim().is_empty() {
                return Err(Error::Validation("--by cannot be empty".into()));
            }
            actor
        }
        None => owner_name(project)?,
    };
    let source = match source {
        Some(source) => {
            crate::format::validate_line("source", &source)?;
            if source.trim().is_empty() {
                return Err(Error::Validation("--source cannot be empty".into()));
            }
            source
        }
        None => "direct".to_string(),
    };
    Ok((actor, source))
}

fn resolve_provenance(warnings: &mut Vec<String>) -> Option<crate::model::HarnessProvenance> {
    match crate::provenance::resolve_from(|name| std::env::var_os(name)) {
        Ok(prov) => prov,
        Err(diag) => {
            warnings.push(diag);
            None
        }
    }
}

fn open(mut ctx: Ctx, req: OpenRequest) -> Result<Output> {
    let kind = DecisionKind::parse(&req.kind)?;
    if !matches!(kind, DecisionKind::Drop | DecisionKind::Suggestion) {
        return Err(Error::Validation(format!(
            "Stage A supports only drop and suggestion decisions, got {}",
            kind.as_str()
        )));
    }
    crate::format::validate_line("origin", &req.origin)?;
    if req.origin.trim().is_empty() {
        return Err(Error::Validation("--origin cannot be empty".into()));
    }
    validate_nonempty_multiline("--text", &req.text)?;

    let explicit_id = req.id.as_deref().map(parse_uuidv4).transpose()?;
    if req.opening_scope.is_some() && req.opened_at.is_none() {
        return Err(Error::Validation(
            "--opening-scope requires --opened-at".into(),
        ));
    }
    if let Some(at) = &req.opened_at {
        let parsed = crate::time::parse(at)?;
        if crate::time::format(parsed) != *at {
            return Err(Error::Validation(format!(
                "--opened-at must be canonical RFC 3339 UTC (YYYY-MM-DDTHH:MM:SSZ), got {at:?}"
            )));
        }
    }
    if let Some(digest) = &req.opening_scope {
        validate_hex64("--opening-scope", digest)?;
    }
    let scope_verdict = match &req.scope_verdict {
        Some(verdict) => {
            let clean = super::status::validate_note_line(verdict, "--scope-verdict")?;
            Some(clean.to_string())
        }
        None => None,
    };

    let (actor, source) =
        resolve_actor_and_source(&ctx.project, req.by, req.source, req.opened_at.is_some())?;

    let mut task = load(&mut ctx, &req.task)?;
    let observed_at = crate::time::now();

    // 1. Check idempotent replay FIRST, before status or upgrade checks.
    if let Some(ref requested_id) = explicit_id
        && let Some(existing) = task.entries.iter().find(|e| e.id == *requested_id)
    {
        let Some(existing_data) = existing.as_decision() else {
            return Err(Error::DecisionConflict(format!(
                "entry {requested_id} on {} is not a decision",
                task.id
            )));
        };
        let existing_source = existing_data.source.as_deref().unwrap_or("direct");
        let subject_matches = match &req.opened_at {
            Some(hist_at) => {
                existing.at == *hist_at
                    && existing.subject
                        == Subject::Scope(ScopeSubject {
                            scope: req.opening_scope.clone(),
                        })
            }
            None => matches!(
                existing.subject,
                Subject::Scope(ScopeSubject { scope: Some(_) })
            ),
        };
        let payload_matches = existing.by == actor
            && existing_data.kind == kind
            && existing_data.origin == req.origin
            && existing_data.text == req.text
            && existing_source == source
            && existing_data.scope_verdict == scope_verdict
            && subject_matches;
        if !payload_matches {
            return Err(Error::DecisionConflict(format!(
                "decision {requested_id} already exists on {} with different parameters",
                task.id
            )));
        }
        let dependents = DependentsIndex::build(&ctx.registry, &[&ctx.project]);
        let decision = logical_decision(
            &task,
            existing,
            existing_data,
            &ctx.registry,
            &dependents,
            &observed_at,
        );
        let resolution = decision.resolution.clone();
        return Ok(Output::Decision(Box::new(DecisionOut {
            id: task.id.to_string(),
            action: "opened",
            replayed: true,
            decision,
            resolution,
            warnings: ctx.warnings,
        })));
    }

    // 2. Fresh open requires an open task and safe format-1 -> format-2 upgrade.
    if !task.status.is_open() {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "open decision (task must be open)".into(),
        ));
    }
    upgrade_to_format_2(&mut task, false)?;

    let entry_id = match explicit_id {
        Some(id) => id,
        None => new_uuidv4()?,
    };
    let at = req.opened_at.clone().unwrap_or_else(|| observed_at.clone());
    let subject = if req.opened_at.is_some() {
        Subject::Scope(ScopeSubject {
            scope: req.opening_scope.clone(),
        })
    } else {
        Subject::Scope(ScopeSubject {
            scope: Some(task_scope_fingerprint(&task)),
        })
    };
    let provenance = resolve_provenance(&mut ctx.warnings);
    let data = DecisionData {
        kind,
        origin: req.origin,
        text: req.text,
        source: Some(source),
        scope_verdict: scope_verdict.clone(),
    };
    let entry = Entry {
        id: entry_id.clone(),
        at,
        by: actor.clone(),
        subject,
        payload: EntryPayload::Decision(data),
        provenance,
    };

    if let Some(verdict) = scope_verdict {
        if task.status == Status::Idea {
            task.reviewed = true;
            task.reviewed_at = Some(observed_at.clone());
        }
        append_stamped_note(&mut ctx, &mut task, &actor, &verdict)?;
    }

    task.entries.push(entry);
    task.open_decisions.push(entry_id.clone());
    validate_entries(task.format, &task.open_decisions, &task.entries)?;

    // Opening a drop on a `doing` or live-claimed task warns the owner to wind down and park
    // without seizing the claim.
    if kind == DecisionKind::Drop {
        let live_claim = ctx
            .claims_mut()?
            .get(&task.id)
            .is_some_and(|claim| !crate::claims::liveness(claim).is_stale());
        if task.status == Status::Doing || live_claim {
            ctx.warnings.push(format!(
                "{} is currently doing; wind down active work and run `tasks park {} \"<next step>\" --waiting-on user --reason decision` (no claim was seized)",
                task.id, task.id
            ));
        }
    }

    save(&mut ctx, &mut task)?;
    super::finish(&mut ctx);

    let saved_entry = task
        .entries
        .iter()
        .find(|e| e.id == entry_id)
        .expect("entry just pushed");
    let saved_data = saved_entry.as_decision().expect("decision just pushed");
    let dependents = DependentsIndex::build(&ctx.registry, &[&ctx.project]);
    let decision = logical_decision(
        &task,
        saved_entry,
        saved_data,
        &ctx.registry,
        &dependents,
        &observed_at,
    );
    Ok(Output::Decision(Box::new(DecisionOut {
        id: task.id.to_string(),
        action: "opened",
        replayed: false,
        decision,
        resolution: None,
        warnings: ctx.warnings,
    })))
}

fn resolve(mut ctx: Ctx, req: ResolveRequest) -> Result<Output> {
    let decision_id = parse_uuidv4(&req.decision)?;
    validate_nonempty_multiline("answer/reason text", &req.text)?;
    if let Some(token) = &req.seen_scope {
        validate_hex64("--seen-scope", token)?;
    }
    let (actor, source) = resolve_actor_and_source(&ctx.project, req.by, req.source, false)?;

    let mut task = load(&mut ctx, &req.task)?;
    let observed_at = crate::time::now();

    let Some(decision_entry) = task.entries.iter().find(|e| e.id == decision_id).cloned() else {
        return Err(Error::DecisionNotFound(format!(
            "decision {decision_id} not found on {}",
            task.id
        )));
    };
    let Some(decision_data) = decision_entry.as_decision().cloned() else {
        return Err(Error::DecisionConflict(format!(
            "entry {decision_id} on {} is not a decision",
            task.id
        )));
    };

    // 1. Recognize completed replay FIRST, before status, scope, or claim guards.
    if let Some((existing_entry, existing_data)) = find_resolution(&task, &decision_id) {
        let matches = existing_data.outcome == req.outcome
            && existing_data.text == req.text
            && existing_entry.by == actor
            && existing_data.source == source
            && existing_data.seen_scope == req.seen_scope;
        if !matches {
            return Err(Error::DecisionConflict(format!(
                "decision {decision_id} on {} was already resolved as {} by {} at {}",
                task.id,
                existing_data.outcome.as_str(),
                existing_entry.by,
                existing_entry.at
            )));
        }
        // Recover host-local claim/park cleanup if an accepted drop was durably saved
        // before local store cleanup finished.
        if decision_data.kind == DecisionKind::Drop
            && req.outcome == ResolutionOutcome::Accept
            && task.status == Status::Dropped
        {
            let store = ctx.claims_mut()?;
            let had_local = store.get(&task.id).is_some()
                || store.park(&task.id).is_some()
                || store.escalation(&task.id).is_some();
            if had_local {
                store.prune_dead();
                store.remove(&task.id);
                store.remove_park(&task.id);
                store.remove_escalation(&task.id);
                if let Err(err) = store.save() {
                    ctx.warnings.push(format!(
                        "{} drop acceptance already landed, but local store cleanup failed ({err}); retry the same answer command",
                        task.id
                    ));
                }
            }
        }
        let dependents = DependentsIndex::build(&ctx.registry, &[&ctx.project]);
        let decision = logical_decision(
            &task,
            &decision_entry,
            &decision_data,
            &ctx.registry,
            &dependents,
            &observed_at,
        );
        let resolution = decision.resolution.clone();
        return Ok(Output::Decision(Box::new(DecisionOut {
            id: task.id.to_string(),
            action: req.action_label,
            replayed: true,
            decision,
            resolution,
            warnings: ctx.warnings,
        })));
    }

    // 2. Fresh resolution requires an open task.
    if !task.status.is_open() {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "resolve decision (task is closed; reopen first)".into(),
        ));
    }

    // Validate outcome compatibility with decision kind.
    match decision_data.kind {
        DecisionKind::Drop | DecisionKind::Suggestion | DecisionKind::Approval => {
            if !matches!(
                req.outcome,
                ResolutionOutcome::Accept
                    | ResolutionOutcome::Decline
                    | ResolutionOutcome::Withdraw
            ) {
                return Err(Error::Validation(format!(
                    "{} decision {} does not accept outcome {}",
                    decision_data.kind.as_str(),
                    decision_id,
                    req.outcome.as_str()
                )));
            }
        }
        DecisionKind::Question => {
            if !matches!(
                req.outcome,
                ResolutionOutcome::Reply | ResolutionOutcome::Withdraw
            ) {
                return Err(Error::Validation(format!(
                    "question decision {decision_id} requires --reply or withdraw, not {}",
                    req.outcome.as_str()
                )));
            }
        }
    }

    let is_drop_accept =
        decision_data.kind == DecisionKind::Drop && req.outcome == ResolutionOutcome::Accept;
    if !is_drop_accept && req.seen_scope.is_some() {
        return Err(Error::Validation(
            "--seen-scope is only valid when accepting a drop decision".into(),
        ));
    }
    if is_drop_accept {
        let Some(token) = req.seen_scope.as_deref() else {
            return Err(Error::Validation(format!(
                "accepting drop decision {decision_id} requires --seen-scope <token> from `tasks show {}` or `tasks list --pending`",
                task.id
            )));
        };
        let current_scope = task_scope_fingerprint(&task);
        if token != current_scope {
            return Err(Error::StaleScope(format!(
                "scope of {} changed since token {token} was observed (current {current_scope}); refresh with `tasks show {}` before accepting drop {decision_id}",
                task.id, task.id
            )));
        }
        // Run ordinary drop guards (foreign claim, open descendants) and status effects in the
        // same atomic replacement as the resolution entry.
        transition(&mut ctx, &mut task, Status::Dropped, false)?;
    }

    let resolution_id = new_uuidv4()?;
    let provenance = resolve_provenance(&mut ctx.warnings);
    let res_entry = Entry {
        id: resolution_id.clone(),
        at: observed_at.clone(),
        by: actor,
        subject: Subject::Entry(EntrySubject {
            entry: decision_id.clone(),
        }),
        payload: EntryPayload::Resolution(ResolutionData {
            outcome: req.outcome,
            text: req.text,
            source,
            seen_scope: req.seen_scope,
        }),
        provenance,
    };

    task.open_decisions.retain(|ptr| ptr != &decision_id);
    task.entries.push(res_entry);
    validate_entries(task.format, &task.open_decisions, &task.entries)?;

    save(&mut ctx, &mut task)?;
    super::finish(&mut ctx);

    let dependents = DependentsIndex::build(&ctx.registry, &[&ctx.project]);
    let decision = logical_decision(
        &task,
        &decision_entry,
        &decision_data,
        &ctx.registry,
        &dependents,
        &observed_at,
    );
    let resolution = decision.resolution.clone();
    Ok(Output::Decision(Box::new(DecisionOut {
        id: task.id.to_string(),
        action: req.action_label,
        replayed: false,
        decision,
        resolution,
        warnings: ctx.warnings,
    })))
}
