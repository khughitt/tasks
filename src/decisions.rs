//! Named human decisions: scope fingerprint v1, unified per-kind hold predicates,
//! cross-registry open-dependent observation, and logical decision projection.

use crate::entries::{DecisionData, DecisionKind, Entry, EntryPayload, ResolutionData, Subject};
use crate::model::{Task, TaskId};
use crate::output::{LogicalDecision, LogicalResolution, OpenDependentRow, OpenDependentsOut};
use crate::registry::Registry;
use crate::repo::Project;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

const SCOPE_FINGERPRINT_DOMAIN_V1: &str = "tasks.scope.1";

fn normalize_lf(input: &str) -> Cow<'_, str> {
    if input.contains('\r') {
        Cow::Owned(input.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(input)
    }
}

fn feed_length_prefixed(hasher: &mut Sha256, field: &str) {
    let normalized = normalize_lf(field);
    let bytes = normalized.as_bytes();
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

/// Scope fingerprint v1 (Task state spec):
/// SHA-256 of `len_be64(s) || utf8_lf(s)` for:
/// 1. `"tasks.scope.1"`
/// 2. `title`
/// 3. `logical_body`
/// 4. `adopted_brief_pin` (compact JSON array of strings when present; empty `""` when absent,
///    which is always the case in format 2).
pub fn scope_fingerprint_v1(
    title: &str,
    logical_body: &str,
    adopted_brief_pin: Option<&[&str]>,
) -> String {
    let mut hasher = Sha256::new();
    feed_length_prefixed(&mut hasher, SCOPE_FINGERPRINT_DOMAIN_V1);
    feed_length_prefixed(&mut hasher, title);
    feed_length_prefixed(&mut hasher, logical_body);
    match adopted_brief_pin {
        Some(pin) => {
            let encoded = serde_json::to_string(pin).expect("pin array serializes");
            feed_length_prefixed(&mut hasher, &encoded);
        }
        None => feed_length_prefixed(&mut hasher, ""),
    }
    format!("{:x}", hasher.finalize())
}

/// Current scope fingerprint v1 for a format-1 or format-2 task record.
pub fn task_scope_fingerprint(task: &Task) -> String {
    scope_fingerprint_v1(&task.title, &task.body, None)
}

/// Actionable open decisions on `task` (empty when `task` is closed).
pub fn actionable_open_decisions(task: &Task) -> Vec<(&Entry, &DecisionData)> {
    if !task.status.is_open() || task.open_decisions.is_empty() {
        return Vec::new();
    }
    let open_set: BTreeSet<&str> = task.open_decisions.iter().map(String::as_str).collect();
    task.entries
        .iter()
        .filter_map(|entry| {
            if !open_set.contains(entry.id.as_str()) {
                return None;
            }
            entry.as_decision().map(|data| (entry, data))
        })
        .collect()
}

/// Open `drop` decision holding execution (`pick_up`, `resume`, and every `start` / `edit --status doing`).
pub fn holds_execution(task: &Task) -> Option<(&Entry, &DecisionData)> {
    actionable_open_decisions(task)
        .into_iter()
        .find(|(_, data)| data.kind == DecisionKind::Drop)
}

/// Open decisions holding `assess` (`drop` and `question` across `idea`, `todo`, `blocked`).
pub fn holds_assess(task: &Task) -> Vec<(&Entry, &DecisionData)> {
    actionable_open_decisions(task)
        .into_iter()
        .filter(|(_, data)| matches!(data.kind, DecisionKind::Drop | DecisionKind::Question))
        .collect()
}

/// Open decisions holding `curate` (`drop`, `suggestion`, `question`, `approval`).
pub fn holds_curate(task: &Task) -> Vec<(&Entry, &DecisionData)> {
    actionable_open_decisions(task)
}

/// Find the resolution entry targeting `decision_id`, if any.
pub fn find_resolution<'a>(
    task: &'a Task,
    decision_id: &str,
) -> Option<(&'a Entry, &'a ResolutionData)> {
    task.entries.iter().find_map(|entry| {
        let res = entry.as_resolution()?;
        match &entry.subject {
            Subject::Entry(target) if target.entry == decision_id => Some((entry, res)),
            _ => None,
        }
    })
}

/// Convert an `Entry` known to be a resolution into its logical JSON wire shape.
pub fn to_logical_resolution(entry: &Entry, data: &ResolutionData) -> LogicalResolution {
    LogicalResolution {
        id: entry.id.clone(),
        entry_type: "resolution",
        at: entry.at.clone(),
        by: entry.by.clone(),
        subject: entry.subject.clone(),
        data: data.clone(),
        provenance: entry.provenance.clone(),
    }
}

/// Cross-registry index of direct open dependents, computed once per command invocation
/// with explicit per-project coverage tracking.
pub struct DependentsIndex {
    /// Canonical target task id -> sorted unique `(canonical dependent id, title)`.
    by_target: BTreeMap<TaskId, Vec<OpenDependentRow>>,
    /// Sorted canonical prefixes of projects that could not be completely read (unreachable,
    /// unreadable, or containing unsupported future-format records whose dependencies are unknown).
    unavailable_projects: Vec<String>,
}

impl DependentsIndex {
    /// Build a cross-registry open-dependents index.
    /// `local_overrides` are already-opened projects (e.g. from the active worktree or scope)
    /// whose copies supersede the registered copy for their prefix.
    pub fn build(registry: &Registry, local_overrides: &[&Project]) -> Self {
        let mut by_prefix: BTreeMap<String, Vec<Task>> = BTreeMap::new();
        let mut unavailable: BTreeSet<String> = BTreeSet::new();
        let mut scanned_prefixes: BTreeSet<String> = BTreeSet::new();

        for project in local_overrides {
            let prefix = registry.canonical_prefix(&project.prefix).to_string();
            if !scanned_prefixes.insert(prefix.clone()) {
                continue;
            }
            match project.scan_mixed() {
                Ok(mixed) => {
                    if !mixed.unsupported.is_empty() {
                        unavailable.insert(prefix.clone());
                    }
                    by_prefix.insert(prefix, mixed.tasks);
                }
                Err(_) => {
                    unavailable.insert(prefix);
                }
            }
        }

        for prefix in registry.projects.keys() {
            let canonical = registry.canonical_prefix(prefix).to_string();
            if !scanned_prefixes.insert(canonical.clone()) {
                continue;
            }
            match crate::scope::is_reachable(registry, &canonical) {
                Ok(true) => {}
                Ok(false) | Err(_) => {
                    unavailable.insert(canonical);
                    continue;
                }
            }
            let project = match crate::scope::open_registered(
                registry,
                &canonical,
                crate::scope::Origin::Prefix,
            ) {
                Ok(project) => project,
                Err(_) => {
                    unavailable.insert(canonical);
                    continue;
                }
            };
            if project.retired.is_some() {
                continue;
            }
            match project.scan_mixed() {
                Ok(mixed) => {
                    if !mixed.unsupported.is_empty() {
                        unavailable.insert(canonical.clone());
                    }
                    by_prefix.insert(canonical, mixed.tasks);
                }
                Err(_) => {
                    unavailable.insert(canonical);
                }
            }
        }

        let mut map: BTreeMap<TaskId, BTreeMap<TaskId, String>> = BTreeMap::new();
        for tasks in by_prefix.values() {
            for candidate in tasks {
                if !candidate.status.is_open() {
                    continue;
                }
                let dep_id = registry.canonical_id(&candidate.id);
                for target in &candidate.depends {
                    let canonical_target = registry.canonical_id(target);
                    map.entry(canonical_target)
                        .or_default()
                        .insert(dep_id.clone(), candidate.title.clone());
                }
            }
        }

        let by_target = map
            .into_iter()
            .map(|(target, deps)| {
                let rows = deps
                    .into_iter()
                    .map(|(id, title)| OpenDependentRow {
                        id: id.to_string(),
                        title,
                    })
                    .collect();
                (target, rows)
            })
            .collect();

        Self {
            by_target,
            unavailable_projects: unavailable.into_iter().collect(),
        }
    }

    pub fn open_dependents(
        &self,
        registry: &Registry,
        task_id: &TaskId,
        observed_at: &str,
    ) -> OpenDependentsOut {
        let canonical = registry.canonical_id(task_id);
        let tasks = self.by_target.get(&canonical).cloned().unwrap_or_default();
        OpenDependentsOut {
            tasks,
            complete: self.unavailable_projects.is_empty(),
            unavailable_projects: self.unavailable_projects.clone(),
            observed_at: observed_at.to_string(),
        }
    }
}

/// Build the logical JSON representation of a single decision entry on `task`.
pub fn logical_decision(
    task: &Task,
    entry: &Entry,
    data: &DecisionData,
    registry: &Registry,
    dependents: &DependentsIndex,
    observed_at: &str,
) -> LogicalDecision {
    let resolution = find_resolution(task, &entry.id)
        .map(|(res_entry, res_data)| to_logical_resolution(res_entry, res_data));
    let state = if resolution.is_some() {
        "resolved"
    } else if !task.status.is_open() {
        "overtaken"
    } else {
        "open"
    };
    let current_scope = task_scope_fingerprint(task);
    let (seen_scope, scope_changed_since_open) = match &entry.subject {
        Subject::Scope(scope) => {
            let changed = scope
                .scope
                .as_deref()
                .map(|open_hash| open_hash != current_scope);
            (Some(current_scope), changed)
        }
        _ => (None, None),
    };
    let open_dependents = dependents.open_dependents(registry, &task.id, observed_at);
    LogicalDecision {
        id: entry.id.clone(),
        entry_type: "decision",
        at: entry.at.clone(),
        by: entry.by.clone(),
        subject: entry.subject.clone(),
        data: data.clone(),
        provenance: entry.provenance.clone(),
        state,
        resolution,
        seen_scope,
        scope_changed_since_open,
        open_dependents,
    }
}

/// Build logical decision views for every decision entry in `task.entries` (open, resolved, overtaken).
pub fn logical_decisions(
    task: &Task,
    registry: &Registry,
    dependents: &DependentsIndex,
    observed_at: &str,
) -> Vec<LogicalDecision> {
    task.entries
        .iter()
        .filter_map(|entry| {
            let EntryPayload::Decision(data) = &entry.payload else {
                return None;
            };
            Some(logical_decision(
                task,
                entry,
                data,
                registry,
                dependents,
                observed_at,
            ))
        })
        .collect()
}

/// Whole nonnegative seconds between `opened_at` and `observed_at`, or `None` when unknown.
pub fn age_seconds(opened_at: Option<&str>, observed_at: OffsetDateTime) -> Option<u64> {
    let ts = opened_at?;
    let parsed = crate::time::parse(ts).ok()?;
    let secs = (observed_at - parsed).whole_seconds().max(0);
    Some(secs as u64)
}

/// Extract evidenced opening timestamp and available question text for a format-1
/// `awaiting_user: true` idea without fabricating unknown values.
pub fn awaiting_user_evidence(task: &Task) -> (Option<String>, Option<String>) {
    let opened_at = task.reviewed_at.clone().or_else(|| {
        task.notes
            .iter()
            .rev()
            .find(|n| n.text.starts_with("scope: question"))
            .map(|n| n.at.clone())
    });
    let text = extract_open_questions(&task.body).or_else(|| {
        task.notes
            .iter()
            .rev()
            .find(|n| n.text.starts_with("scope: question"))
            .map(|n| n.text.clone())
    });
    (opened_at, text)
}

fn extract_open_questions(body: &str) -> Option<String> {
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "## Open questions" {
            let mut collected = Vec::new();
            for next in lines {
                if next.starts_with("## ") {
                    break;
                }
                collected.push(next);
            }
            let text = collected.join("\n").trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_fingerprint_v1_golden_byte_level() {
        // Construct the exact byte sequence by hand:
        // len_be64("tasks.scope.1") || b"tasks.scope.1"
        // || len_be64("Café title") || "Café title".as_bytes()
        // || len_be64("Line 1\nLine 2") || b"Line 1\nLine 2"
        // || len_be64("") || b""
        let mut expected_bytes = Vec::new();
        for part in ["tasks.scope.1", "Café title", "Line 1\nLine 2", ""] {
            let b = part.as_bytes();
            expected_bytes.extend_from_slice(&(b.len() as u64).to_be_bytes());
            expected_bytes.extend_from_slice(b);
        }
        let expected = format!("{:x}", Sha256::digest(&expected_bytes));
        // UTF-8 byte length of "Café title" is 11 (not 10 chars).
        assert_eq!("Café title".len(), 11);
        assert_eq!(
            scope_fingerprint_v1("Café title", "Line 1\nLine 2", None),
            expected
        );
        // CRLF normalizes to LF, yielding the identical digest.
        assert_eq!(
            scope_fingerprint_v1("Café title", "Line 1\r\nLine 2", None),
            expected
        );
        // Non-empty brief pin changes the digest deterministically.
        let with_pin = scope_fingerprint_v1(
            "Café title",
            "Line 1\nLine 2",
            Some(&[
                "records",
                "tasks/docs/notes/b.md",
                "0123456789abcdef0123456789abcdef01234567",
            ]),
        );
        assert_ne!(with_pin, expected);
        assert_eq!(with_pin.len(), 64);
    }
}
