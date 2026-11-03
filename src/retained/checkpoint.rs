use super::{
    basis,
    model::*,
    publish::{self, Binding, GeneratedNote, NewArtifact, OperationCtx},
    state, verify,
};
use crate::{
    error::{Error, Result},
    model::TaskId,
};
use std::collections::{BTreeMap, BTreeSet};

fn counts(view: &state::RetainedState) -> Result<Counts> {
    if !view.complete {
        return Err(Error::RetainedHistoryBlocked("Unresolved retained history prevents complete host accounting; supply exact source-bound inventory corrections".into()));
    }
    let mut counts = Counts {
        kept_verified: 0,
        kept_unknown: 0,
        released: 0,
    };
    for item in &view.items {
        let r = item
            .record
            .as_ref()
            .ok_or_else(|| invalid("blocked inventory row"))?;
        if r.state == "released" {
            counts.released += 1
        } else if r.verification == "verified" {
            counts.kept_verified += 1
        } else {
            counts.kept_unknown += 1
        }
    }
    Ok(counts)
}
fn view(ctx: &OperationCtx, id: &TaskId, host: &str) -> Result<state::RetainedState> {
    let pinned = ctx
        .tasks
        .get(id)
        .ok_or_else(|| invalid("checkpoint task is not locked"))?;
    state::replay(&pinned.task, &pinned.basis, Some(host))
}
struct Correction {
    bad: Source,
    identities: Vec<Identity>,
    reason: String,
}
struct Inventory {
    records: BTreeMap<Identity, RetentionRecord>,
    corrections: Vec<Correction>,
}
fn inventory(
    ctx: &mut OperationCtx,
    id: &TaskId,
    request: &CheckpointRequest,
) -> Result<Inventory> {
    let pinned = ctx.tasks.get(id).expect("checkpoint participant");
    let full = state::replay(&pinned.task, &pinned.basis, None)?;
    let local = view(ctx, id, &request.host)?;
    let mut declarations = BTreeMap::<Identity, Declaration>::new();
    for d in &request.declarations {
        let identity = d.record(&request.host, &id.to_string())?.identity();
        if declarations.insert(identity, d.clone()).is_some() {
            return Err(invalid("duplicate checkpoint declarations"));
        }
    }
    let mut corrections = Vec::new();
    let mut bad_sources = BTreeSet::new();
    for resolution in &request.resolutions {
        if !bad_sources.insert(resolution.bad.ordinal) {
            return Err(invalid("duplicate resolution source"));
        }
        basis::validate_locator(ctx.project(id), id, &resolution.bad.basis)?;
        let original = basis::read_exact(ctx.project(id), id, &resolution.bad.basis)?;
        ctx.metrics.history_bytes_read += original.history_bytes_read;
        let ordinal = usize::try_from(resolution.bad.ordinal).map_err(invalid)?;
        let current = &ctx.tasks.get(id).expect("participant").task;
        if ordinal == 0
            || original.task.notes.get(..ordinal) != current.notes.get(..ordinal)
            || ordinal > original.task.notes.len()
        {
            return Err(invalid(
                "resolution source is not the exact preceding bad history",
            ));
        }
        let diagnostic = full
            .diagnostics
            .iter()
            .find(|d| {
                d.source
                    .as_ref()
                    .is_some_and(|s| s.ordinal == resolution.bad.ordinal)
            })
            .ok_or_else(|| invalid("resolution does not name a blocked source"))?;
        if diagnostic.resolved_by.is_some() {
            continue;
        }
        let mut identities = Vec::new();
        for d in &resolution.declarations {
            let identity = d.record(&request.host, &id.to_string())?.identity();
            if identities.contains(&identity) {
                return Err(invalid("duplicate resolution declarations"));
            }
            if let Some(other) = declarations.get(&identity)
                && other != d
            {
                return Err(invalid("contradictory checkpoint terms"));
            }
            declarations.insert(identity.clone(), d.clone());
            identities.push(identity);
        }
        let required = if let Some(identity) = &diagnostic.identity {
            vec![identity.clone()]
        } else {
            full.items
                .iter()
                .filter(|i| {
                    diagnostic
                        .host
                        .as_ref()
                        .is_none_or(|h| *h == i.identity.host)
                })
                .map(|i| i.identity.clone())
                .collect()
        };
        if identities.is_empty()
            || diagnostic.host.as_ref().is_some_and(|h| *h != request.host)
            || required
                .iter()
                .any(|i| i.host != request.host || !identities.contains(i))
        {
            return Err(invalid(
                "resolution needs the complete blocked inventory on the executing host; recover other-host history explicitly before repair",
            ));
        }
        corrections.push(Correction {
            bad: resolution.bad.clone(),
            identities,
            reason: resolution.reason.clone(),
        });
    }
    for diagnostic in local.diagnostics.iter().filter(|d| d.resolved_by.is_none()) {
        if !corrections.iter().any(|r| {
            diagnostic
                .source
                .as_ref()
                .is_some_and(|s| r.bad.ordinal == s.ordinal)
        }) {
            return Err(Error::RetainedHistoryBlocked(
                "Checkpoint has unresolved history without a complete source-bound correction"
                    .into(),
            ));
        }
    }
    let mut records = BTreeMap::new();
    for item in local.items {
        if let Some(record) = item.record
            && record.state == "kept"
        {
            records.insert(item.identity, record);
        }
    }
    for (identity, d) in declarations {
        if let Some(record) = records.get_mut(&identity) {
            record.purpose = d.purpose;
            record.cleanup_trigger = d.cleanup_trigger;
        } else {
            records.insert(identity, d.record(&request.host, &id.to_string())?);
        }
    }
    for record in records.values() {
        verify::preflight(ctx.project(id), record)?;
        let path = verify::current_path(ctx.project(id), &record.path)?;
        if (record.kind == "scratch"
            || std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir())
                && record.kind != "worktree")
            && let Ok(root) = path.canonicalize()
        {
            let target = crate::attachments::task_dir(ctx.project(id), id);
            if target.starts_with(&root) {
                return Err(invalid(
                    "checkpoint metadata destination lies inside the inventoried container; use an off-container records layout",
                ));
            }
        }
    }
    Ok(Inventory {
        records,
        corrections,
    })
}
fn checkpoint_note(
    ctx: &mut OperationCtx,
    id: &TaskId,
    host: &str,
    observation: Option<(Basis, Counts, u64)>,
    actions: Vec<String>,
    at: &str,
) -> Result<GeneratedNote> {
    let (basis, counts, changed, complete) = match observation {
        Some((basis, counts, changed)) => (Some(basis), Some(counts), changed, true),
        None => (None, None, 0, false),
    };
    let note = CheckpointNote {
        version: 1,
        operation: ctx.key.clone(),
        observed_at: at.into(),
        host: host.into(),
        observer: ctx.observer(id)?,
        basis,
        scope: CheckpointScope {
            mode: "all_known_kept_on_host".into(),
            host: host.into(),
            other_hosts: "outside_observation".into(),
        },
        counts,
        changed,
        actions,
        complete,
    };
    let raw = serde_json::to_string(&note).map_err(invalid)?;
    decode_checkpoint(&raw)?;
    Ok(GeneratedNote {
        at: at.into(),
        text: format!("retention checkpoint: {raw}"),
    })
}
fn replay(
    ctx: &mut OperationCtx,
    id: &TaskId,
    request: &CheckpointRequest,
) -> Result<Option<MutationResult>> {
    let Some((index, operation)) = ctx.phase(id)? else {
        return Ok(None);
    };
    let PhaseOutputs::Completed(completed) = operation.outputs else {
        return Ok(None);
    };
    let current = ctx.tasks.get(id).expect("participant");
    let committed = basis::read_exact(ctx.project(id), id, &current.basis)?;
    ctx.metrics.history_bytes_read += committed.history_bytes_read;
    if committed.task.notes.get(index) != current.task.notes.get(index) {
        return Ok(None);
    }
    let context = request.inputs[0].basis.clone();
    let prefix = ctx.tasks.get(id).expect("participant").task.notes[..=index].to_vec();
    let project = ctx.project(id).clone();
    let source = basis::publication_basis(
        &project,
        id,
        &context,
        &prefix,
        &mut ctx.metrics.history_bytes_read,
    )?;
    let original = basis::read_exact(&project, id, &source)?;
    ctx.metrics.history_bytes_read += original.history_bytes_read;
    let (counts, actions) = if let Some(ordinal) = completed.checkpoint {
        let text = original
            .task
            .notes
            .get(ordinal as usize - 1)
            .and_then(|n| n.text.strip_prefix("retention checkpoint: "))
            .ok_or_else(|| invalid("completion checkpoint locator is missing"))?;
        let note = decode_checkpoint(text)?;
        if !note.complete || note.operation != ctx.key {
            return Err(invalid(
                "completion is not bound to its complete checkpoint",
            ));
        }
        (
            note.counts
                .ok_or_else(|| invalid("checkpoint counts missing"))?,
            note.actions,
        )
    } else {
        let state = state::replay(&original.task, &source, Some(&request.host))?;
        (counts(&state)?, Vec::new())
    };
    Ok(Some(publish::result(
        ctx,
        completed.outcome,
        counts,
        actions,
        true,
    )?))
}
fn changed(ctx: &OperationCtx, id: &TaskId) -> Result<u64> {
    let task = &ctx.tasks.get(id).expect("participant").task;
    let mut identities = BTreeSet::new();
    for note in &task.notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let operation = decode_operation(raw)?;
            if operation.key == ctx.key
                && let PhaseOutputs::Items(output) = operation.outputs
            {
                for ordinal in output.items {
                    let raw = task
                        .notes
                        .get(ordinal as usize - 1)
                        .and_then(|n| n.text.strip_prefix("retained: "))
                        .ok_or_else(|| invalid("item receipt lacks its retained object"))?;
                    identities.insert(decode_record(raw)?.identity());
                }
            }
        }
    }
    Ok(identities.len() as u64)
}
fn observe(
    ctx: &mut OperationCtx,
    id: &TaskId,
    request: &CheckpointRequest,
    binding: &Binding,
    inventory: Inventory,
) -> Result<MutationResult> {
    let before = view(ctx, id, &request.host)?;
    let at = crate::time::after(&ctx.tasks.get(id).expect("participant").task.updated)?;
    let mut records = Vec::new();
    let mut artifacts = BTreeMap::<String, NewArtifact>::new();
    let mut git = Vec::new();
    let mut preserved = Vec::new();
    let project = ctx.project(id).clone();
    for (identity, record) in inventory.records {
        ctx.heartbeat()?;
        let previous = request
            .preserved_previous
            .iter()
            .find(|p| p.identity == identity)
            .map(|p| &p.artifact);
        let pinned = ctx.tasks.get(id).expect("participant");
        let mut observation =
            verify::inspect_history(&project, pinned, &record, previous, &mut ctx.metrics)?;
        for locator in std::mem::take(&mut observation.preservations) {
            if !preserved.contains(&locator) {
                preserved.push(locator);
            }
        }
        if let Some(status) = observation.git.take() {
            git.push(serde_json::json!({"identity":identity,"status_porcelain_z":status}));
        }
        if let Some(manifest) = observation.manifest.take() {
            let name = format!("retained-manifest-{}.json", manifest.sha256);
            let target = crate::attachments::task_dir(&project, id).join(&name);
            observation.record.manifest = FieldString::Text(
                target
                    .to_str()
                    .ok_or_else(|| invalid("manifest path must be UTF-8"))?
                    .into(),
            );
            let prior = before
                .items
                .iter()
                .find(|i| i.identity == identity)
                .and_then(|i| i.record.as_ref());
            if prior != Some(&observation.record)
                || crate::attachments::ledger(&ctx.tasks.get(id).expect("participant").task)
                    .get(&name)
                    != Some(&crate::attachments::Ledger::Attached)
            {
                artifacts.insert(
                    name.clone(),
                    NewArtifact {
                        name,
                        bytes: manifest.bytes,
                    },
                );
            }
        }
        observation.record.validate()?;
        let prior = before
            .items
            .iter()
            .find(|i| i.identity == identity)
            .and_then(|i| i.record.as_ref());
        if prior != Some(&observation.record) {
            records.push((identity, observation.record));
        }
    }
    if !git.is_empty() {
        let bytes=serde_json::to_vec(&serde_json::json!({"version":1,"operation":ctx.key,"host":request.host,"observed_at":at,"worktrees":git})).map_err(invalid)?;
        let name = format!(
            "retained-checkpoint-{}.json",
            verify::hash_bytes(&bytes, &mut ctx.metrics)
        );
        artifacts.insert(name.clone(), NewArtifact { name, bytes });
    }
    let artifact_list = artifacts.into_values().collect::<Vec<_>>();
    let task = &ctx.tasks.get(id).expect("participant").task;
    let ledger = crate::attachments::ledger(task);
    let ledger_added = artifact_list
        .iter()
        .filter(|a| ledger.get(&a.name) != Some(&crate::attachments::Ledger::Attached))
        .count();
    let first = task.notes.len() + ledger_added + 1;
    let ordinals = records
        .iter()
        .enumerate()
        .map(|(i, _)| first as u64 + i as u64)
        .collect::<Vec<_>>();
    let relations = inventory
        .corrections
        .into_iter()
        .map(|c| {
            let successors = records
                .iter()
                .enumerate()
                .filter(|(_, r)| c.identities.contains(&r.0))
                .map(|(i, _)| ordinals[i])
                .collect();
            ResolutionRelation {
                bad: c.bad,
                successors,
                reason: c.reason,
            }
        })
        .collect();
    let mut notes = records
        .iter()
        .map(|(_, r)| {
            Ok(GeneratedNote {
                at: at.clone(),
                text: format!("retained: {}", serde_json::to_string(r).map_err(invalid)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut locators: Vec<ArtifactLocator> = artifact_list
        .iter()
        .map(|a| ArtifactLocator::Attachment {
            task: id.to_string(),
            name: a.name.clone(),
            basis: None,
            sha256: verify::hash_bytes(&a.bytes, &mut ctx.metrics),
        })
        .collect();
    locators.extend(preserved);
    notes.push(publish::receipt(
        ctx,
        binding,
        id,
        PhaseOutputs::Items(ItemsOutput {
            items: ordinals,
            artifacts: locators,
            resolutions: relations,
        }),
        &at,
    )?);
    let published = publish::batch(ctx, id, &artifact_list, &notes)?;
    if published
        .sources
        .iter()
        .take(records.len())
        .map(|s| s.ordinal)
        .ne((first..first + records.len()).map(|o| o as u64))
    {
        return Err(invalid("published item ordinals differ from their receipt"));
    }
    let item_basis = published.publication.basis;
    publish::stop("items_published");
    let current = view(ctx, id, &request.host)?;
    let counts = counts(&current)?;
    let actions = current
        .items
        .iter()
        .filter_map(|i| {
            i.record
                .as_ref()
                .and_then(|r| r.verification_action.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let outcome = if counts.kept_unknown > 0 {
        "accounted_unknown"
    } else {
        "verified"
    }
    .to_string();

    let at = crate::time::after(&ctx.tasks.get(id).expect("participant").task.updated)?;
    let checkpoint = checkpoint_note(
        ctx,
        id,
        &request.host,
        Some((item_basis, counts.clone(), changed(ctx, id)?)),
        actions.clone(),
        &at,
    )?;
    let ordinal = ctx.tasks.get(id).expect("participant").task.notes.len() as u64 + 1;
    let publications = ctx.phase_publications(id)?;
    let complete = publish::receipt(
        ctx,
        binding,
        id,
        PhaseOutputs::Completed(CompletedOutput {
            outcome: outcome.clone(),
            publications,
            checkpoint: Some(ordinal),
        }),
        &at,
    )?;
    publish::batch(ctx, id, &[], &[checkpoint, complete])?;
    publish::stop("completed");
    publish::result(ctx, outcome, counts, actions, false)
}
fn incomplete(ctx: &mut OperationCtx, id: &TaskId, binding: &Binding, error: &Error) -> Result<()> {
    let Some((_, prior)) = ctx.phase(id)? else {
        return Ok(());
    };
    if matches!(prior.phase.as_str(), "completed" | "reconciled") {
        return Ok(());
    }
    let attempt = ctx
        .tasks
        .get(id)
        .expect("participant")
        .task
        .notes
        .iter()
        .filter_map(|n| {
            n.text
                .strip_prefix("retention operation: ")
                .and_then(|s| decode_operation(s).ok())
        })
        .filter(|n| n.key == ctx.key && n.phase == "incomplete")
        .count() as u64
        + 1;
    let at = crate::time::after(&ctx.tasks.get(id).expect("participant").task.updated)?;
    let action = format!(
        "{error}; restore exact bytes/ownership and retry the original request, or reconcile custody without completing it"
    );
    let checkpoint = checkpoint_note(ctx, id, &binding.host, None, vec![action.clone()], &at)?;
    let ordinal = ctx.tasks.get(id).expect("participant").task.notes.len() as u64 + 1;
    let receipt = publish::receipt(
        ctx,
        binding,
        id,
        PhaseOutputs::Incomplete(IncompleteOutput {
            attempt,
            previous_phase: prior.phase,
            checkpoint: Some(ordinal),
            action,
        }),
        &at,
    )?;
    publish::batch(ctx, id, &[], &[checkpoint, receipt])?;
    Ok(())
}
pub fn run(
    mut ctx: OperationCtx,
    request: CheckpointRequest,
    provided: &[u8],
) -> Result<MutationResult> {
    request.validate()?;
    if request.host != crate::claims::hostname() {
        return Err(invalid("checkpoint host differs from the executing host"));
    }
    let id = TaskId::parse(&ctx.key.coordinator)?;
    if request.inputs.len() != 1 || request.inputs[0].task != id.to_string() {
        return Err(invalid("checkpoint input must name its coordinator"));
    }
    let name = format!("retained-op-{}.request.json", request.operation_id);
    publish::validate_bases(&ctx, &request.inputs)?;
    let binding = publish::binding(
        &mut ctx,
        &request,
        provided,
        &request.inputs,
        &request.host,
        &name,
    )?;
    if ctx
        .phase(&id)?
        .is_some_and(|(_, n)| n.phase == "reconciled")
    {
        return Err(invalid(
            "This operation was reconciled without completion; repair observations with a fresh checkpoint request",
        ));
    }
    if let Some(result) = replay(&mut ctx, &id, &request)? {
        return Ok(result);
    }
    publish::validate_inputs(&mut ctx, &request.inputs)?;
    publish::recover_saved(&mut ctx, &id, &binding)?;
    let inventory = inventory(&mut ctx, &id, &request)?;
    publish::intent(&mut ctx, &binding, &id)?;
    if inventory.records.is_empty() {
        let at = crate::time::after(&ctx.tasks.get(&id).expect("participant").task.updated)?;
        let counts = counts(&view(&ctx, &id, &request.host)?)?;
        let publications = ctx.phase_publications(&id)?;
        let complete = publish::receipt(
            &mut ctx,
            &binding,
            &id,
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "no_local_items".into(),
                publications,
                checkpoint: None,
            }),
            &at,
        )?;
        publish::batch(&mut ctx, &id, &[], &[complete])?;
        return publish::result(&mut ctx, "no_local_items".into(), counts, Vec::new(), false);
    }
    match observe(&mut ctx, &id, &request, &binding, inventory) {
        Ok(result) => Ok(result),
        Err(error) => {
            let failure = incomplete(&mut ctx, &id, &binding, &error);
            let phase = ctx
                .phase(&id)?
                .map(|p| p.1.phase)
                .unwrap_or_else(|| "unpublished".into());
            let suffix = match failure {
                Ok(()) => String::new(),
                Err(e) => format!("; failure receipt could not publish: {e}"),
            };
            Err(error.with_suffix(&format!(
                "; operation {}/{} last phase {phase}{suffix}",
                ctx.key.coordinator, ctx.key.operation_id
            )))
        }
    }
}
