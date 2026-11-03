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

pub fn replay(task: &Task, basis: &Basis, host: Option<&str>) -> Result<RetainedState> {
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
                Err(e) => Err((raw, e.to_string())),
            }
        } else if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let decoded = decode_operation(raw).and_then(|n| {
                n.request.pinned(&task.id.to_string(), basis)?;
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
                }
                phases.insert(key, n.clone());
                Ok(n)
            });
            match decoded {
                Ok(n) => {
                    if let PhaseOutputs::Items(out) = &n.outputs {
                        for relation in &out.resolutions {
                            if relation.bad.ordinal == 0
                                || relation.bad.ordinal >= source.ordinal
                                || relation.bad.basis.task_path != basis.task_path
                                || relation.bad.basis.project != basis.project
                                || relation.successors.is_empty()
                                || relation
                                    .successors
                                    .iter()
                                    .any(|v| *v <= relation.bad.ordinal || *v >= source.ordinal)
                                || !line(&relation.reason)
                            {
                                continue;
                            }
                            // Exact historical source and successor bytes are verified by the pinned reader.
                            for d in &mut s.diagnostics {
                                if d.source
                                    .as_ref()
                                    .is_some_and(|p| p.ordinal == relation.bad.ordinal)
                                {
                                    let corrected = relation
                                        .successors
                                        .iter()
                                        .filter_map(|o| {
                                            task.notes
                                                .get(*o as usize - 1)
                                                .and_then(|n| n.text.strip_prefix("retained: "))
                                                .and_then(|r| decode_record(r).ok())
                                                .filter(|r| r.version == Some(1))
                                                .map(|r| r.identity())
                                        })
                                        .collect::<Vec<_>>();
                                    let covered = corrected.len() == relation.successors.len()
                                        && !corrected.is_empty()
                                        && if let Some(identity) = &d.identity {
                                            corrected.contains(identity)
                                        } else {
                                            corrected.iter().all(|i| {
                                                d.host.as_ref().is_none_or(|h| *h == i.host)
                                            }) && items
                                                .keys()
                                                .filter(|i| {
                                                    d.host.as_ref().is_none_or(|h| *h == i.host)
                                                })
                                                .all(|i| corrected.contains(i))
                                        };
                                    if covered {
                                        d.resolved_by = Some(source.clone());
                                    }
                                }
                            }
                        }
                    }
                    s.operations.push(OperationEntry {
                        source: source.clone(),
                        note: n,
                    });
                    Ok(())
                }
                Err(e) => Err((raw, e.to_string())),
            }
        } else if let Some(raw) = note.text.strip_prefix("retention checkpoint: ") {
            if raw.starts_with('{') {
                let checked = decode_checkpoint(raw).and_then(|n| {
                    let key = (
                        n.operation.coordinator.clone(),
                        n.operation.operation_id.clone(),
                    );
                    if n.complete
                        && (n.operation.coordinator != task.id.to_string()
                            || !phases.get(&key).is_some_and(|p| {
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
                    Err(e) => Err((raw, e.to_string())),
                }
            } else {
                s.checkpoints.push(CheckpointEntry {
                    source: source.clone(),
                    note: None,
                    text: Some(raw.into()),
                });
                Ok(())
            }
        } else {
            Ok(())
        };
        if let Err((raw, detail)) = parsed {
            // Partial identity is diagnostic scope only, never a usable predecessor.
            let v = strict(raw.as_bytes()).unwrap_or(Value::Null);
            let h = v
                .get("host")
                .and_then(Value::as_str)
                .filter(|h| text(h))
                .map(str::to_string);
            if host.is_some_and(|filter| h.as_deref().is_some_and(|h| h != filter)) {
                continue;
            }
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
                action:
                    "Publish an explicit source-bound correction after recovering exact history"
                        .into(),
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
