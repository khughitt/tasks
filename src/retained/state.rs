use super::model::*;
use crate::error::Result;
use crate::model::Task;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct RetainedItem {
    pub identity: Identity,
    pub record: Option<RetentionRecord>,
    pub source: Option<Source>,
    pub observed_at: String,
    pub checkpoint: Option<Source>,
    pub coverage: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct CheckpointEntry {
    pub source: Source,
    pub note: Option<CheckpointNote>,
    pub text: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct OperationEntry {
    pub source: Source,
    pub note: OperationNote,
}
#[derive(Debug, Clone, Serialize)]
pub struct RetainedState {
    pub version: u8,
    pub contract: String,
    pub task: String,
    pub basis: Basis,
    pub host_filter: Option<String>,
    pub complete: bool,
    pub items: Vec<RetainedItem>,
    pub checkpoints: Vec<CheckpointEntry>,
    pub operations: Vec<OperationEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<String>,
}

const RETAINED_DIAGNOSTIC_ACTION: &str = "Recover exact history, append valid version: 1 'retained:' successor note(s) for each affected identity (or none when no identities exist; use <records>/tasks/files/<id>/<name> for an existing attached manifest, including on closed tasks without reopening), then append 'retention resolution: {\"version\":1,\"bad\":<source>,\"successors\":[...],\"reason\":\"...\"}' or pass resolutions to tasks retained checkpoint";
const ENVELOPE_DIAGNOSTIC_ACTION: &str =
    "Publish an explicit source-bound correction after recovering exact history";

#[allow(clippy::too_many_arguments)]
fn apply_resolution(
    project: &crate::repo::Project,
    task_id: &crate::model::TaskId,
    notes: &[crate::model::Note],
    items: &BTreeMap<Identity, RetainedItem>,
    diagnostics: &mut [Diagnostic],
    source: &Source,
    relation: &ResolutionRelation,
    operation_host: Option<&str>,
    standalone: bool,
) -> Result<()> {
    if relation.bad.ordinal == 0 || relation.bad.ordinal >= source.ordinal {
        return Err(invalid(
            "resolution bad ordinal must precede resolution source",
        ));
    }
    let bad_id = super::lineage::historical_id(&relation.bad.basis)?;
    if !super::lineage::same(project, &bad_id.to_string(), &task_id.to_string())? {
        return Err(invalid("resolution bad basis belongs to a different task"));
    }
    if !line(&relation.reason) {
        return Err(invalid("resolution reason must be a non-empty single line"));
    }
    let bad_idx = usize::try_from(relation.bad.ordinal - 1).map_err(invalid)?;
    if standalone
        && !notes
            .get(bad_idx)
            .is_some_and(|n| n.text.starts_with("retained:"))
    {
        return Err(invalid(
            "retention resolution may only target a retained: item note",
        ));
    }
    let mut seen_ordinals = std::collections::BTreeSet::new();
    let mut seen_identities = std::collections::BTreeSet::new();
    let mut corrected = Vec::with_capacity(relation.successors.len());
    for &o in &relation.successors {
        if o <= relation.bad.ordinal || o >= source.ordinal || !seen_ordinals.insert(o) {
            return Err(invalid(
                "resolution successors must be distinct ordinals between bad.ordinal and resolution",
            ));
        }
        let idx = usize::try_from(o - 1).map_err(invalid)?;
        let raw = notes
            .get(idx)
            .and_then(|n| n.text.strip_prefix("retained: "))
            .ok_or_else(|| invalid("resolution successor is not a retained: item note"))?;
        let record = decode_record(raw)?;
        if record.version != Some(1) {
            return Err(invalid("resolution successor requires version: 1"));
        }
        if !standalone
            && operation_host
                .is_some_and(|op_host| record.state != "kept" || record.host != op_host)
        {
            return Err(invalid(
                "checkpoint resolution successor must be a kept record on the operation host",
            ));
        }
        let identity = record.identity();
        if !seen_identities.insert(identity.clone()) {
            return Err(invalid(
                "resolution successors must have distinct item identities",
            ));
        }
        corrected.push(identity);
    }
    let mut matched = false;
    let mut covered_any = false;
    for d in diagnostics.iter_mut() {
        if d.source
            .as_ref()
            .is_some_and(|p| p.ordinal == relation.bad.ordinal)
        {
            matched = true;
            let covered = match (&d.identity, &d.host) {
                (Some(id), _) => {
                    !corrected.is_empty()
                        && if standalone {
                            corrected.as_slice() == std::slice::from_ref(id)
                        } else {
                            corrected.contains(id)
                        }
                }
                (None, Some(h)) => {
                    corrected.iter().all(|c| c.host == *h)
                        && items
                            .keys()
                            .filter(|i| i.host == *h)
                            .all(|i| corrected.contains(i))
                }
                (None, None) => items.keys().all(|i| corrected.contains(i)),
            };
            if covered {
                d.resolved_by = Some(source.clone());
                covered_any = true;
            }
        }
    }
    if standalone && (!matched || !covered_any) {
        return Err(invalid(if !matched {
            "retention resolution does not match a diagnostic at bad.ordinal"
        } else {
            "retention resolution does not cover all affected identities in scope"
        }));
    }
    Ok(())
}

pub fn replay(
    project: &crate::repo::Project,
    task: &Task,
    basis: &Basis,
    host: Option<&str>,
) -> Result<RetainedState> {
    let mut s = RetainedState {
        version: 1,
        contract: "retention-1".into(),
        task: task.id.to_string(),
        basis: basis.clone(),
        host_filter: host.map(str::to_string),
        complete: true,
        items: Vec::new(),
        checkpoints: Vec::new(),
        operations: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    };
    let mut items = BTreeMap::<Identity, RetainedItem>::new();
    let mut checkpoints = BTreeMap::<String, Source>::new();
    let mut phases = BTreeMap::<(String, String), OperationNote>::new();
    for (i, note) in task.notes.iter().enumerate() {
        let source = Source {
            basis: basis.clone(),
            ordinal: i as u64 + 1,
        };
        let parsed = if let Some(raw) = note.text.strip_prefix("retained: ") {
            match decode_record(raw) {
                Ok(r) => {
                    let identity = r.identity();
                    items.insert(
                        identity.clone(),
                        RetainedItem {
                            identity,
                            record: Some(r),
                            source: Some(source.clone()),
                            observed_at: note.at.clone(),
                            checkpoint: None,
                            coverage: "recorded".into(),
                        },
                    );
                    Ok(())
                }
                Err(e) => Err((raw, e.to_string(), RETAINED_DIAGNOSTIC_ACTION)),
            }
        } else if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let decoded = decode_operation(raw).and_then(|n| {
                let local = super::lineage::participant(project, &n, &task.id)?
                    .task
                    .clone();
                n.request.pinned(&local, basis)?;
                if n.input_bases
                    .first()
                    .is_none_or(|b| b.task != n.key.coordinator)
                    || !n.input_bases.iter().any(|b| b.task == local)
                    || (matches!(
                        n.phase.as_str(),
                        "items_published" | "sender_released" | "reconciled"
                    ) && local != n.key.coordinator)
                    || (n.phase == "receiver_published"
                        && (n.input_bases.len() != 2 || n.input_bases[1].task != local))
                {
                    return Err(invalid(
                        "operation participant role conflicts with its source task",
                    ));
                }
                let key = (n.key.coordinator.clone(), n.key.operation_id.clone());
                let previous = phases.get(&key);
                validate_transition(previous, &n)?;
                let ordinals: &[u64] = match &n.outputs {
                    PhaseOutputs::Items(o) => &o.items,
                    PhaseOutputs::Receiver(o) => &o.items,
                    PhaseOutputs::Sender(o) => &o.items,
                    PhaseOutputs::Ready(o) => &o.items,
                    _ => &[],
                };
                let mut seen = std::collections::BTreeSet::new();
                for ordinal in ordinals {
                    let record = ordinal
                        .checked_sub(1)
                        .and_then(|v| usize::try_from(v).ok())
                        .filter(|v| *v < i)
                        .and_then(|v| task.notes.get(v))
                        .and_then(|note| note.text.strip_prefix("retained: "))
                        .and_then(|raw| decode_record(raw).ok())
                        .ok_or_else(|| {
                            invalid("operation item lacks a preceding retained source")
                        })?;
                    if !seen.insert(*ordinal) || record.version != Some(1) || record.host != n.host
                    {
                        return Err(invalid(
                            "operation item source conflicts with its observation",
                        ));
                    }
                    let mapped = match &n.outputs {
                        PhaseOutputs::Receiver(o) => Some((&o.mapping, false)),
                        PhaseOutputs::Sender(o) => Some((&o.mapping, true)),
                        _ => None,
                    };
                    if let Some((mapping, sender)) = mapped {
                        let matches = mapping
                            .iter()
                            .filter(|m| {
                                (if sender { &m.sender } else { &m.receiver }) == &record.identity()
                            })
                            .count();
                        if mapping.len() != ordinals.len()
                            || matches != 1
                            || (sender && record.state != "released")
                            || (!sender
                                && (record.state != "kept" || record.verification != "verified"))
                        {
                            return Err(invalid(
                                "operation mapping conflicts with retained source",
                            ));
                        }
                    }
                }
                phases.insert(key, n.clone());
                Ok(n)
            });
            match decoded {
                Ok(n) => {
                    if let PhaseOutputs::Items(out) = &n.outputs {
                        // Exact historical source and successor bytes are verified by the pinned reader.
                        for relation in &out.resolutions {
                            let _ = apply_resolution(
                                project,
                                &task.id,
                                &task.notes,
                                &items,
                                &mut s.diagnostics,
                                &source,
                                relation,
                                Some(&n.host),
                                false,
                            );
                        }
                    }
                    s.operations.push(OperationEntry {
                        source: source.clone(),
                        note: n,
                    });
                    Ok(())
                }
                Err(e) => Err((raw, e.to_string(), ENVELOPE_DIAGNOSTIC_ACTION)),
            }
        } else if let Some(raw) = note.text.strip_prefix("retention checkpoint: ") {
            if raw.starts_with('{') {
                let checked = decode_checkpoint(raw).and_then(|n| {
                    let key = (
                        n.operation.coordinator.clone(),
                        n.operation.operation_id.clone(),
                    );
                    if n.complete
                        && (!super::lineage::same(
                            project,
                            &n.operation.coordinator,
                            &task.id.to_string(),
                        )? || !phases.get(&key).is_some_and(|p| {
                            p.host == n.host
                                && matches!(
                                    p.phase.as_str(),
                                    "intent" | "items_published" | "incomplete"
                                )
                        }))
                    {
                        return Err(invalid(
                            "checkpoint requires its matching unfinished operation intent",
                        ));
                    }
                    Ok(n)
                });
                match checked {
                    Ok(n) => {
                        if n.complete {
                            checkpoints.insert(n.host.clone(), source.clone());
                        }
                        s.checkpoints.push(CheckpointEntry {
                            source: source.clone(),
                            note: Some(n),
                            text: None,
                        });
                        Ok(())
                    }
                    Err(e) => Err((raw, e.to_string(), ENVELOPE_DIAGNOSTIC_ACTION)),
                }
            } else {
                s.checkpoints.push(CheckpointEntry {
                    source: source.clone(),
                    note: None,
                    text: Some(raw.into()),
                });
                Ok(())
            }
        } else if note.text.starts_with("retention resolution:") {
            let outcome = note
                .text
                .strip_prefix("retention resolution: ")
                .ok_or_else(|| invalid("missing space after 'retention resolution:'"))
                .and_then(decode_resolution)
                .and_then(|res| {
                    let relation = ResolutionRelation {
                        bad: res.bad,
                        successors: res.successors,
                        reason: res.reason,
                    };
                    apply_resolution(
                        project,
                        &task.id,
                        &task.notes,
                        &items,
                        &mut s.diagnostics,
                        &source,
                        &relation,
                        None,
                        true,
                    )
                });
            if let Err(e) = outcome {
                let detail = e.to_string();
                s.diagnostics.push(Diagnostic {
                    code: if detail.contains("unsupported_version") {
                        "unsupported_version"
                    } else {
                        "malformed_record"
                    }
                    .into(),
                    scope: "task".into(),
                    host: None,
                    identity: None,
                    source: Some(source.clone()),
                    detail,
                    action: ENVELOPE_DIAGNOSTIC_ACTION.into(),
                    resolved_by: None,
                });
            }
            Ok(())
        } else {
            Ok(())
        };
        if let Err((raw, detail, action)) = parsed {
            // Partial identity is diagnostic scope only, never a usable predecessor.
            let v = strict(raw.as_bytes()).unwrap_or(Value::Null);
            let h = v
                .get("host")
                .and_then(Value::as_str)
                .filter(|h| text(h))
                .map(str::to_string);
            let identity = serde_json::from_value::<Identity>(
                serde_json::json!({"host":v.get("host"),"kind":v.get("kind"),"path":v.get("path")}),
            )
            .ok()
            .filter(|identity| {
                Declaration {
                    kind: identity.kind.clone(),
                    path: identity.path.clone(),
                    purpose: "Diagnostic scope".into(),
                    cleanup_trigger: "Recover exact history".into(),
                }
                .record(&identity.host, &task.id.to_string())
                .is_ok()
            });
            s.diagnostics.push(Diagnostic {
                code: if detail.contains("unsupported_version") {
                    "unsupported_version"
                } else {
                    "malformed_record"
                }
                .into(),
                scope: if identity.is_some() {
                    "identity"
                } else if h.is_some() {
                    "host"
                } else {
                    "task"
                }
                .into(),
                host: h,
                identity,
                source: Some(source),
                detail,
                action: action.into(),
                resolved_by: None,
            });
        }
    }
    for item in items.values_mut() {
        let blocked = s
            .diagnostics
            .iter()
            .filter(|d| d.resolved_by.is_none())
            .any(|d| {
                d.identity.as_ref().map_or_else(
                    || d.host.as_ref().is_none_or(|h| *h == item.identity.host),
                    |id| *id == item.identity,
                )
            });
        if blocked {
            item.record = None;
            item.coverage = "blocked".into();
        } else if let Some(cp) = checkpoints.get(&item.identity.host) {
            item.checkpoint = Some(cp.clone());
            item.coverage = if item.source.as_ref().is_some_and(|p| p.ordinal > cp.ordinal) {
                "changed"
            } else {
                "recorded"
            }
            .into();
        }
        if item
            .record
            .as_ref()
            .is_some_and(|r| r.verification == "unknown")
        {
            item.coverage = "unknown".into();
        }
    }
    s.diagnostics
        .retain(|d| d.host.is_none() || host.is_none_or(|f| d.host.as_deref() == Some(f)));
    s.complete = s.diagnostics.iter().all(|d| d.resolved_by.is_some());
    s.items = items
        .into_values()
        .filter(|i| host.is_none_or(|h| h == i.identity.host))
        .collect();
    Ok(s)
}

pub fn verification_requirement(task: &Task, current: &RetentionRecord) -> RetentionRecord {
    if current.verification != "unknown" || current.kind == "worktree" {
        return current.clone();
    }
    for note in task.notes.iter().rev() {
        if let Some(raw) = note.text.strip_prefix("retained: ")
            && let Ok(record) = decode_record(raw)
            && record.identity() == current.identity()
        {
            if record.state == "released" {
                break;
            }
            if record.verification == "verified" {
                let mut required = current.clone();
                required.verification = "verified".into();
                required.sha256 = record.sha256;
                required.manifest = record.manifest;
                required.verification_action = None;
                return required;
            }
        }
    }
    current.clone()
}
