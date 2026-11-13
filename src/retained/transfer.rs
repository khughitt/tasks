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
use std::collections::BTreeMap;

pub fn purpose(receiver: &TaskId) -> String {
    format!("Transferred to {receiver}; custody transferred and bytes remain in place")
}
pub fn release_at(sender_updated: &str, receiver_kept: &[String]) -> Result<String> {
    let mut latest = crate::time::parse(sender_updated)?;
    for at in receiver_kept {
        latest = latest.max(crate::time::parse(at)?);
    }
    crate::time::after(&crate::time::format(latest))
}
fn phase(ctx: &OperationCtx, id: &TaskId, name: &str) -> Result<Option<(usize, OperationNote)>> {
    for (index, note) in ctx.tasks[id].task.notes.iter().enumerate() {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let note = decode_operation(raw)?;
            if note.key == ctx.key && note.phase == name {
                return Ok(Some((index, note)));
            }
        }
    }
    Ok(None)
}
fn committed(ctx: &mut OperationCtx, id: &TaskId, index: usize) -> Result<bool> {
    let pinned = &ctx.tasks[id];
    let exact = basis::read_exact(ctx.project(id), id, &pinned.basis)?;
    ctx.metrics.history_bytes_read += exact.history_bytes_read;
    Ok(exact.task.notes.get(index) == pinned.task.notes.get(index))
}
pub(super) fn counts(ctx: &mut OperationCtx, host: &str) -> Result<Counts> {
    let mut out = Counts {
        kept_verified: 0,
        kept_unknown: 0,
        released: 0,
    };
    for id in ctx.tasks.keys().cloned().collect::<Vec<_>>() {
        let publication = ctx
            .phase_publications(&id)?
            .into_iter()
            .find(|p| p.phase == "completed")
            .ok_or_else(|| invalid("transfer completion publication missing"))?;
        let pinned = basis::read_exact(ctx.project(&id), &id, &publication.basis)?;
        ctx.metrics.history_bytes_read += pinned.history_bytes_read;
        let view = state::replay(ctx.project(&id), &pinned.task, &pinned.basis, Some(host))?;
        for item in view.items {
            if let Some(r) = item.record {
                if r.state == "released" {
                    out.released += 1;
                } else if r.verification == "verified" {
                    out.kept_verified += 1;
                } else {
                    out.kept_unknown += 1;
                }
            }
        }
    }
    Ok(out)
}
fn mapping(
    ctx: &OperationCtx,
    from: &TaskId,
    to: &TaskId,
    request: &TransferRequest,
) -> Result<Vec<Mapping>> {
    request
        .items
        .iter()
        .map(|identity| {
            let mut receiver = identity.clone();
            if from.prefix != to.prefix && identity.kind == "worktree" {
                receiver.path = verify::current_path(ctx.project(from), &identity.path)?
                    .canonicalize()?
                    .to_str()
                    .ok_or_else(|| invalid("worktree path is not UTF-8"))?
                    .to_string();
            }
            Ok(Mapping {
                sender: identity.clone(),
                receiver,
            })
        })
        .collect()
}
fn selected(
    ctx: &OperationCtx,
    id: &TaskId,
    identities: &[Identity],
    owner: &TaskId,
) -> Result<Vec<RetentionRecord>> {
    let pinned = &ctx.tasks[id];
    let view = state::replay(ctx.project(id), &pinned.task, &pinned.basis, None)?;
    identities
        .iter()
        .map(|identity| {
            let item = view
                .items
                .iter()
                .find(|i| i.identity == *identity)
                .and_then(|i| i.record.clone())
                .ok_or_else(|| {
                    Error::RetainedHistoryBlocked(
                        "Selected identity is missing or its history is blocked".into(),
                    )
                })?;
            if item.state != "kept"
                || !super::lineage::owner_matches(ctx.project(id), &item.next_owner, owner)?
            {
                return Err(invalid(
                    "selected identity is not kept in the named task's custody",
                ));
            }
            Ok(item)
        })
        .collect()
}
fn preflight(
    ctx: &OperationCtx,
    from: &TaskId,
    to: &TaskId,
    request: &TransferRequest,
    mapping: &[Mapping],
) -> Result<()> {
    let originals = selected(ctx, from, &request.items, from)?;
    let source = state::replay(
        ctx.project(from),
        &ctx.tasks[from].task,
        &ctx.tasks[from].basis,
        Some(&request.host),
    )?;
    if source.items.iter().any(|i| i.record.is_none()) {
        return Err(Error::RetainedHistoryBlocked(
            "Sender local inventory has a blocked retained scope; repair its history first".into(),
        ));
    }
    if mapping
        .iter()
        .map(|m| &m.receiver)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != mapping.len()
    {
        return Err(invalid(
            "distinct selected identities collide in the receiving worktree representation",
        ));
    }
    let pinned = &ctx.tasks[to];
    let receiver = state::replay(
        ctx.project(to),
        &pinned.task,
        &pinned.basis,
        Some(&request.host),
    )?;
    let already = phase(ctx, to, "receiver_published")?.is_some();
    if !already {
        if receiver.diagnostics.iter().any(|d| {
            d.resolved_by.is_none()
                && (d.host.is_none() || d.host.as_deref() == Some(&request.host))
        }) {
            return Err(Error::RetainedHistoryBlocked(
                "Receiver retained history has an unresolved local scope".into(),
            ));
        }
        if mapping
            .iter()
            .any(|m| receiver.items.iter().any(|i| i.identity == m.receiver))
        {
            return Err(invalid(
                "receiver identity already exists outside this operation",
            ));
        }
    }
    for record in originals {
        let manifest = record
            .manifest
            .value()
            .map(|_| super::lineage::manifest_path(ctx.project(from), &ctx.tasks[from], &record))
            .transpose()?;
        verify::preflight(ctx.project(from), &record, manifest.as_deref())?;
        let path = verify::current_path(ctx.project(from), &record.path)?;
        if record.kind != "worktree" && std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            let root = path.canonicalize()?;
            for id in [from, to] {
                if crate::attachments::task_dir(ctx.project(id), id).starts_with(&root) {
                    return Err(invalid(
                        "transfer metadata destination lies inside the inventoried container",
                    ));
                }
            }
        }
    }
    Ok(())
}
fn observations(
    ctx: &mut OperationCtx,
    id: &TaskId,
    records: &[RetentionRecord],
) -> Result<Vec<verify::Observation>> {
    let project = ctx.project(id).clone();
    records.iter().map(|record| {
        let observation=verify::inspect_history(&project,&ctx.tasks[id],record,None,&mut ctx.metrics)?;
        if observation.record.verification!="verified" {return Err(Error::RetainedHistoryBlocked(format!("Transfer evidence remains unknown: {}",observation.record.verification_action.as_deref().unwrap_or("inspect exact bytes"))));}
        // Transfers preserve the verified version and cannot authorize an unexplained replacement.
        if record.verification=="verified" && record.kind!="worktree" && record.sha256!=observation.record.sha256 {return Err(invalid("transfer source changed from its verified byte version; repair it by checkpoint first"));}
        Ok(observation)
    }).collect()
}
fn receiver(
    ctx: &mut OperationCtx,
    from: &TaskId,
    to: &TaskId,
    request: &TransferRequest,
    binding: &Binding,
    mapping: &[Mapping],
) -> Result<()> {
    if phase(ctx, to, "receiver_published")?.is_some() {
        return Ok(());
    }
    let records = selected(ctx, from, &request.items, from)?;
    let observations = observations(ctx, from, &records)?;
    let mut artifacts = BTreeMap::new();
    let mut receiving = Vec::new();
    let mut git = Vec::new();
    for (mut observation, map) in observations.into_iter().zip(mapping) {
        let r = &mut observation.record;
        r.path = map.receiver.path.clone();
        r.purpose = request.purpose.clone();
        r.cleanup_trigger = request.cleanup_trigger.clone();
        r.next_owner = format!("task:{to}");
        r.handoff = None;
        if let Some(manifest) = observation.manifest {
            let name = format!("retained-manifest-{}.json", manifest.sha256);
            let target = crate::attachments::task_dir(ctx.project(to), to).join(&name);
            r.manifest = FieldString::Text(
                target
                    .to_str()
                    .ok_or_else(|| invalid("manifest path UTF-8"))?
                    .into(),
            );
            artifacts.insert(
                name.clone(),
                NewArtifact {
                    name,
                    bytes: manifest.bytes,
                },
            );
        }
        if let Some(status) = observation.git {
            git.push(serde_json::json!({"identity":map.receiver,"status_porcelain_z":status}));
        }
        r.validate()?;
        receiving.push(observation.record);
    }
    if !git.is_empty() {
        let bytes = serde_json::to_vec(
            &serde_json::json!({"version":1,"operation":ctx.key,"worktrees":git}),
        )
        .map_err(invalid)?;
        let name = format!(
            "retained-checkpoint-{}.json",
            verify::hash_bytes(&bytes, &mut ctx.metrics)
        );
        artifacts.insert(name.clone(), NewArtifact { name, bytes });
    }
    let artifacts = artifacts.into_values().collect::<Vec<_>>();
    let ledger = crate::attachments::ledger(&ctx.tasks[to].task);
    let added = artifacts
        .iter()
        .filter(|a| ledger.get(&a.name) != Some(&crate::attachments::Ledger::Attached))
        .count();
    let first = ctx.tasks[to].task.notes.len() + added + 1;
    let at = crate::time::after(&ctx.tasks[to].task.updated)?;
    let mut notes = receiving
        .iter()
        .map(|r| {
            Ok(GeneratedNote {
                at: at.clone(),
                text: format!("retained: {}", serde_json::to_string(r).map_err(invalid)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let locators = artifacts
        .iter()
        .map(|a| ArtifactLocator::Attachment {
            task: to.to_string(),
            name: a.name.clone(),
            basis: None,
            sha256: verify::hash_bytes(&a.bytes, &mut ctx.metrics),
        })
        .collect();
    notes.push(publish::receipt(
        ctx,
        binding,
        to,
        PhaseOutputs::Receiver(ReceiverOutput {
            items: (first..first + receiving.len()).map(|n| n as u64).collect(),
            artifacts: locators,
            mapping: mapping.to_vec(),
        }),
        &at,
    )?);
    publish::batch(ctx, to, &artifacts, &notes)?;
    publish::stop("receiver_published");
    Ok(())
}
fn release(
    ctx: &mut OperationCtx,
    from: &TaskId,
    to: &TaskId,
    request: &TransferRequest,
    binding: &Binding,
    mapping: &[Mapping],
) -> Result<()> {
    if let Some((index, _)) = phase(ctx, from, "sender_released")?
        && committed(ctx, from, index)?
    {
        return Ok(());
    }
    ctx.revalidate()?;
    let originals = selected(ctx, from, &request.items, from)?;
    observations(ctx, from, &originals)?;
    let target_ids = mapping
        .iter()
        .map(|m| m.receiver.clone())
        .collect::<Vec<_>>();
    let received = selected(ctx, to, &target_ids, to)?;
    observations(ctx, to, &received)?;
    let (index, phase) = phase(ctx, to, "receiver_published")?
        .ok_or_else(|| invalid("receiving publication missing"))?;
    if !committed(ctx, to, index)? {
        return Err(invalid("receiving phase is not committed"));
    }
    let PhaseOutputs::Receiver(output) = phase.outputs else {
        return Err(invalid("receiver phase shape"));
    };
    if output.mapping != mapping || output.items.len() != mapping.len() {
        return Err(invalid("receiver mapping conflicts with request"));
    }
    let mut at_times = Vec::new();
    for (ordinal, map) in output.items.iter().zip(mapping) {
        let note = ctx.tasks[to]
            .task
            .notes
            .get(*ordinal as usize - 1)
            .ok_or_else(|| invalid("receiver source missing"))?;
        let r = decode_record(
            note.text
                .strip_prefix("retained: ")
                .ok_or_else(|| invalid("receiver source kind"))?,
        )?;
        if r.identity() != map.receiver
            || !super::lineage::owner_matches(ctx.project(to), &r.next_owner, to)?
            || r.purpose != request.purpose
            || r.cleanup_trigger != request.cleanup_trigger
        {
            return Err(invalid("receiver source differs from agreed terms"));
        }
        at_times.push(note.at.clone());
    }
    let receiver_basis = ctx
        .phase_publications(to)?
        .into_iter()
        .find(|p| p.phase == "receiver_published")
        .ok_or_else(|| invalid("receiver commit unavailable"))?
        .basis;
    ctx.revalidate()?;
    let at = release_at(&ctx.tasks[from].task.updated, &at_times)?;
    let first = ctx.tasks[from].task.notes.len() + 1;
    let mut notes = originals
        .into_iter()
        .map(|mut r| {
            r.version = Some(1);
            r.state = "released".into();
            r.verification = "verified".into();
            r.purpose = purpose(to);
            r.next_owner = "none".into();
            r.cleanup_trigger = "none".into();
            r.sha256 = FieldString::Absent;
            r.manifest = FieldString::Absent;
            r.handoff = None;
            r.verification_action = None;
            r.validate()?;
            Ok(GeneratedNote {
                at: at.clone(),
                text: format!("retained: {}", serde_json::to_string(&r).map_err(invalid)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    notes.push(publish::receipt(
        ctx,
        binding,
        from,
        PhaseOutputs::Sender(SenderOutput {
            items: (first..first + notes.len()).map(|n| n as u64).collect(),
            receiver_basis,
            mapping: mapping.to_vec(),
        }),
        &at,
    )?);
    publish::batch(ctx, from, &[], &notes)?;
    publish::stop("sender_released");
    Ok(())
}
fn finish(ctx: &mut OperationCtx, from: &TaskId, to: &TaskId, binding: &Binding) -> Result<()> {
    let mut publications = ctx.phase_publications(from)?;
    publications.extend(ctx.phase_publications(to)?);
    // The sender completes last, so its completed receipt implies both reservations ended.
    for id in [to, from] {
        if ctx.phase(id)?.is_some_and(|p| p.1.phase == "completed") {
            continue;
        }
        let at = crate::time::after(&ctx.tasks[id].task.updated)?;
        let note = publish::receipt(
            ctx,
            binding,
            id,
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "transferred".into(),
                publications: publications.clone(),
                checkpoint: None,
            }),
            &at,
        )?;
        publish::batch(ctx, id, &[], &[note])?;
    }
    Ok(())
}
fn incomplete(
    ctx: &mut OperationCtx,
    from: &TaskId,
    binding: &Binding,
    error: &Error,
) -> Result<()> {
    let Some((_, prior)) = ctx.phase(from)? else {
        return Ok(());
    };
    if matches!(prior.phase.as_str(), "completed" | "reconciled") {
        return Ok(());
    }
    let attempt = ctx.tasks[from]
        .task
        .notes
        .iter()
        .filter_map(|n| {
            n.text
                .strip_prefix("retention operation: ")
                .and_then(|r| decode_operation(r).ok())
        })
        .filter(|n| n.key == ctx.key && n.phase == "incomplete")
        .count() as u64
        + 1;
    let at = crate::time::after(&ctx.tasks[from].task.updated)?;
    let note = publish::receipt(
        ctx,
        binding,
        from,
        PhaseOutputs::Incomplete(IncompleteOutput {
            attempt,
            previous_phase: prior.phase,
            checkpoint: None,
            action: format!(
                "{error}; restore exact evidence and retry the original request, or reconcile custody without release"
            ),
        }),
        &at,
    )?;
    publish::batch(ctx, from, &[], &[note])?;
    Ok(())
}
pub fn run(
    mut ctx: OperationCtx,
    request: TransferRequest,
    provided: &[u8],
) -> Result<MutationResult> {
    request.validate()?;
    if request.host != crate::claims::hostname() {
        return Err(invalid("transfer host differs from executing host"));
    }
    let from = TaskId::parse(&request.inputs[0].task)?;
    let to = TaskId::parse(&request.inputs[1].task)?;
    publish::validate_bases(&ctx, &request.inputs)?;
    let name = format!("retained-op-{}.request.json", request.operation_id);
    let binding = publish::binding(
        &mut ctx,
        &request,
        provided,
        &request.inputs,
        &request.host,
        &name,
    )?;
    if ctx
        .phase(&from)?
        .is_some_and(|(_, n)| n.phase == "reconciled")
    {
        return Err(invalid(
            "This transfer was reconciled without release; use a fresh operation for any new transfer",
        ));
    }
    if let Some((index, note)) = ctx.phase(&from)?
        && note.phase == "completed"
        && committed(&mut ctx, &from, index)?
    {
        let counts = counts(&mut ctx, &request.host)?;
        return publish::result(&mut ctx, "transferred".into(), counts, Vec::new(), true);
    }
    // A committed release needs only its missing final receipts, never a new observation.
    if let Some((index, _)) = phase(&ctx, &from, "sender_released")?
        && committed(&mut ctx, &from, index)?
    {
        ctx.committed_release = true;
        for id in [&from, &to] {
            publish::recover_saved(&mut ctx, id, &binding)?;
        }
        finish(&mut ctx, &from, &to, &binding)?;
        publish::stop("completed");
        let counts = counts(&mut ctx, &request.host)?;
        return publish::result(&mut ctx, "transferred".into(), counts, Vec::new(), false);
    }
    publish::recover_phase_bases(&mut ctx, &binding)?;
    publish::new_admission(&ctx)?;
    publish::validate_inputs(&mut ctx, &request.inputs)?;
    for id in [&from, &to] {
        publish::recover_saved(&mut ctx, id, &binding)?;
    }
    let map = mapping(&ctx, &from, &to, &request)?;
    preflight(&ctx, &from, &to, &request, &map)?;
    publish::intent(&mut ctx, &binding, &from)?;
    let result = (|| {
        publish::intent(&mut ctx, &binding, &to)?;
        publish::stop("receiver_intent");
        receiver(&mut ctx, &from, &to, &request, &binding, &map)?;
        release(&mut ctx, &from, &to, &request, &binding, &map)?;
        finish(&mut ctx, &from, &to, &binding)?;
        publish::stop("completed");
        let counts = counts(&mut ctx, &request.host)?;
        publish::result(&mut ctx, "transferred".into(), counts, Vec::new(), false)
    })();
    match result {
        Ok(out) => Ok(out),
        Err(error) => {
            let failure = incomplete(&mut ctx, &from, &binding, &error);
            let suffix = failure
                .err()
                .map(|e| format!("; incomplete receipt failed: {e}"))
                .unwrap_or_default();
            Err(error.with_suffix(&format!(
                "; transfer {}/{} remains pending{suffix}",
                ctx.key.coordinator, ctx.key.operation_id
            )))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_transfer_timestamp_floor() {
        let to = TaskId::parse("sci-000002").unwrap();
        assert_eq!(
            purpose(&to),
            "Transferred to sci-000002; custody transferred and bytes remain in place"
        );
        assert_eq!(
            release_at("2100-01-01T00:00:00Z", &["2100-01-01T00:00:00Z".into()]).unwrap(),
            "2100-01-01T00:00:01Z"
        );
        assert_eq!(
            release_at("2026-11-03T00:00:00Z", &["2100-01-01T00:00:02Z".into()]).unwrap(),
            "2100-01-01T00:00:03Z"
        );
    }
}
