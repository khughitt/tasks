//! Incoming `next_owner` retained custody queries across same-project and all-projects scopes.

mod discovery;
mod recheck;

#[cfg(test)]
pub(crate) use discovery::notes_mention_suffixes;
#[cfg(test)]
pub(crate) use recheck::DURING_INCOMING_RECHECK;

use super::basis::PinnedTask;
use super::model::{Identity, RetentionRecord, Source, decode_record, decode_resolution};
use crate::error::Result;
use crate::model::{Status, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use crate::scope::{self, Origin, Scope};
use discovery::{BatchReader, PinnedProjectScan};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const SPACELESS_RETAINED_ACTION: &str = "Append a valid 'retained: {\"version\":1,...}' successor note if needed and a 'retention resolution: ...' note via 'tasks note' binding bad.basis and bad.ordinal";

fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncomingScope {
    Project,
    AllProjects,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingCustody {
    pub scope: IncomingScope,
    #[serde(deserialize_with = "required_option")]
    pub complete: Option<bool>,
    pub items: Vec<IncomingItem>,
    pub diagnostics: Vec<IncomingDiagnostic>,
}

impl IncomingCustody {
    pub fn none() -> Self {
        Self {
            scope: IncomingScope::None,
            complete: None,
            items: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingItem {
    pub holder: String,
    pub identity: Identity,
    pub record: RetentionRecord,
    pub source: Source,
    pub observed_at: String,
    #[serde(deserialize_with = "required_option")]
    pub checkpoint: Option<Source>,
    pub coverage: String,
}

impl IncomingItem {
    pub(crate) fn sort_key(&self) -> (&str, &Identity) {
        (&self.holder, &self.identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingDiagnostic {
    pub code: String,
    pub project: String,
    #[serde(deserialize_with = "required_option")]
    pub holder: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub host: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub identity: Option<Identity>,
    #[serde(deserialize_with = "required_option")]
    pub source: Option<Source>,
    pub detail: String,
    pub action: String,
}

type SourceSortKey<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, &'a str, u64);

fn source_sort_key(source: &Source) -> SourceSortKey<'_> {
    (
        &source.basis.project,
        &source.basis.layout,
        &source.basis.records_subdir,
        &source.basis.commit,
        &source.basis.task_path,
        &source.basis.retained_ref,
        source.ordinal,
    )
}

impl IncomingDiagnostic {
    #[allow(clippy::type_complexity)]
    pub(crate) fn sort_key(
        &self,
    ) -> (
        &str,
        Option<&str>,
        &str,
        Option<&str>,
        Option<&Identity>,
        Option<SourceSortKey<'_>>,
        &str,
        &str,
    ) {
        (
            &self.project,
            self.holder.as_deref(),
            &self.code,
            self.host.as_deref(),
            self.identity.as_ref(),
            self.source.as_ref().map(source_sort_key),
            &self.detail,
            &self.action,
        )
    }
}

#[derive(Clone)]
pub(crate) struct ParticipantEntry {
    pub canonical_id: String,
    pub pinned: PinnedTask,
}

pub(crate) struct TrackedProject {
    pub project: Project,
    pub git_root: PathBuf,
    pub basis_commit: String,
    pub ref_b: Option<String>,
    pub tasks_rel: String,
    pub is_focal_project: bool,
    pub in_scan_scope: bool,
    pub participants: BTreeMap<TaskId, ParticipantEntry>,
    pub failed_candidates: BTreeSet<TaskId>,
}

pub(crate) struct PreparedIncoming {
    pub custody: IncomingCustody,
    pub suffixes: BTreeSet<String>,
    pub tracked_projects: Vec<TrackedProject>,
}

fn upsert_tracked_project(
    tracked: &mut Vec<TrackedProject>,
    known_git_roots: &mut BTreeSet<PathBuf>,
    project: Project,
    meta: discovery::SnapshotMeta,
    is_focal_project: bool,
    in_scan_scope: bool,
) -> usize {
    known_git_roots.insert(meta.git_root.clone());
    if let Some(idx) = tracked
        .iter()
        .position(|t| t.project.prefix == project.prefix && t.project.records == project.records)
    {
        if is_focal_project {
            tracked[idx].is_focal_project = true;
        }
        if in_scan_scope {
            tracked[idx].in_scan_scope = true;
        }
        return idx;
    }
    tracked.push(TrackedProject {
        project,
        git_root: meta.git_root,
        basis_commit: meta.basis_commit,
        ref_b: meta.ref_b,
        tasks_rel: meta.tasks_rel,
        is_focal_project,
        in_scan_scope,
        participants: BTreeMap::new(),
        failed_candidates: BTreeSet::new(),
    });
    tracked.len() - 1
}

fn record_failed_candidate(tracked: &mut [TrackedProject], project: &Project, raw_id: &TaskId) {
    if let Some(tp) = tracked
        .iter_mut()
        .find(|t| t.project.prefix == project.prefix && t.project.records == project.records)
    {
        tp.failed_candidates.insert(raw_id.clone());
    }
}

fn record_participant(
    tracked: &mut [TrackedProject],
    focal_ctx_project: &Project,
    focal_id: &TaskId,
    project: &Project,
    raw_id: &TaskId,
    canonical_id: String,
    pinned: PinnedTask,
) {
    if project.prefix == focal_ctx_project.prefix
        && project.records == focal_ctx_project.records
        && raw_id == focal_id
    {
        return;
    }
    if let Some(tp) = tracked
        .iter_mut()
        .find(|t| t.project.prefix == project.prefix && t.project.records == project.records)
    {
        tp.participants
            .entry(raw_id.clone())
            .or_insert(ParticipantEntry {
                canonical_id,
                pinned,
            });
    }
}

fn move_incomplete_diag(
    project: impl Into<String>,
    holder: Option<String>,
    detail: impl Into<String>,
) -> IncomingDiagnostic {
    IncomingDiagnostic {
        code: "move_history_incomplete".into(),
        project: project.into(),
        holder,
        host: None,
        identity: None,
        source: None,
        detail: detail.into(),
        action: "Restore clean, reciprocal moved_from/moved_to task records across participating projects before relying on incoming retained custody".into(),
    }
}

fn owner_matches_focal(
    registry: &Registry,
    record: &RetentionRecord,
    focal_ids: &BTreeSet<TaskId>,
    suffixes: &BTreeSet<String>,
    focal_hops: &[(Project, TaskId)],
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> bool {
    let Some(raw_tid) = record.next_owner.strip_prefix("task:") else {
        return false;
    };
    let Ok(owner_id) = TaskId::parse(raw_tid) else {
        return false;
    };
    if !suffixes.contains(&owner_id.hex) {
        return false;
    }
    let canonical_owner = registry.canonical_id(&owner_id);
    if focal_ids.contains(&canonical_owner) {
        return true;
    }
    for (hop_project, hop_canon) in focal_hops {
        if hop_canon.hex != owner_id.hex
            || !super::lineage::owns_prefix(hop_project, &owner_id.prefix)
        {
            continue;
        }
        let Ok(current_in_proj) =
            TaskId::parse(&format!("{}-{}", hop_project.prefix, hop_canon.hex))
        else {
            continue;
        };
        match super::lineage::equivalent(hop_project, &owner_id, &current_in_proj) {
            Ok(true) => return true,
            Ok(false) => {}
            Err(err) => {
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: hop_project.prefix.clone(),
                    holder: Some(current_in_proj.to_string()),
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Repair [retention_lineage] and pinned rename history before relying on incoming retained custody".into(),
                });
            }
        }
    }
    false
}

pub(crate) fn collect(
    focal_project: &Project,
    registry: &Registry,
    focal_id: &TaskId,
    focal_pinned: &PinnedTask,
    host: Option<&str>,
    all_projects: bool,
) -> Result<(PreparedIncoming, Vec<String>)> {
    let scope = if all_projects {
        IncomingScope::AllProjects
    } else {
        IncomingScope::Project
    };
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();

    let focal_meta = discovery::snapshot_meta(focal_project)?;
    let mut tracked_projects = Vec::new();
    let mut known_git_roots = BTreeSet::new();
    upsert_tracked_project(
        &mut tracked_projects,
        &mut known_git_roots,
        focal_project.clone(),
        focal_meta,
        true,
        true,
    );

    // §3.1 Focal task move-identity set (`focal_ids`, `suffixes`, `focal_hops`).
    let focal_canon = registry.canonical_id(&focal_pinned.task.id);
    let mut focal_ids = BTreeSet::from([focal_canon.clone()]);
    let mut suffixes = BTreeSet::from([focal_canon.hex.clone()]);
    let mut focal_hops = vec![(focal_project.clone(), focal_canon.clone())];
    let own_canon_prefix = registry.canonical_prefix(&focal_project.prefix).to_string();

    for forward in [true, false] {
        let mut current_task = focal_pinned.task.clone();
        let mut prev_canon = focal_canon.clone();
        loop {
            let pointer = if forward {
                &current_task.moved_to
            } else {
                &current_task.moved_from
            };
            let Some(pointer) = pointer else { break };
            let hop_id = registry.canonical_id(pointer);
            if focal_ids.contains(&hop_id) {
                diagnostics.push(move_incomplete_diag(
                    hop_id.prefix.clone(),
                    Some(hop_id.to_string()),
                    format!("Cycle detected in focal move chain at {hop_id}"),
                ));
                break;
            }
            let hop_project = if hop_id.prefix == own_canon_prefix {
                focal_project.clone()
            } else {
                let opened =
                    match scope::open_registered(registry, &hop_id.prefix, Origin::Id(&hop_id)) {
                        Ok(p) => p,
                        Err(err) => {
                            diagnostics.push(move_incomplete_diag(
                                hop_id.prefix.clone(),
                                Some(hop_id.to_string()),
                                err.to_string(),
                            ));
                            break;
                        }
                    };
                match super::read::inherit(focal_project, opened) {
                    Ok(p) => p,
                    Err(err) => {
                        diagnostics.push(move_incomplete_diag(
                            hop_id.prefix.clone(),
                            Some(hop_id.to_string()),
                            err.to_string(),
                        ));
                        break;
                    }
                }
            };

            if !(hop_project.prefix == focal_project.prefix
                && hop_project.records == focal_project.records)
            {
                match discovery::snapshot_meta(&hop_project) {
                    Ok(hop_meta) => {
                        upsert_tracked_project(
                            &mut tracked_projects,
                            &mut known_git_roots,
                            hop_project.clone(),
                            hop_meta,
                            false,
                            false,
                        );
                    }
                    Err(err) => {
                        diagnostics.push(move_incomplete_diag(
                            hop_id.prefix.clone(),
                            Some(hop_id.to_string()),
                            err.to_string(),
                        ));
                        break;
                    }
                }
            }

            let read_hop_id = if hop_id.prefix == hop_project.prefix {
                hop_id.clone()
            } else {
                TaskId::parse(&format!("{}-{}", hop_project.prefix, hop_id.hex))
                    .unwrap_or_else(|_| hop_id.clone())
            };
            let hop_pinned = match super::basis::read(&hop_project, &read_hop_id, None) {
                Ok(p) => p,
                Err(err) => {
                    diagnostics.push(move_incomplete_diag(
                        hop_id.prefix.clone(),
                        Some(hop_id.to_string()),
                        err.to_string(),
                    ));
                    break;
                }
            };
            let reverse = if forward {
                &hop_pinned.task.moved_from
            } else {
                &hop_pinned.task.moved_to
            };
            let reciprocal = registry.canonical_id(&hop_pinned.task.id) == hop_id
                && reverse
                    .as_ref()
                    .is_some_and(|r| registry.canonical_id(r) == prev_canon);
            if !reciprocal {
                diagnostics.push(move_incomplete_diag(
                    hop_id.prefix.clone(),
                    Some(hop_id.to_string()),
                    format!(
                        "Move hop {hop_id} lacks reciprocal {} pointer to {prev_canon}",
                        if forward { "moved_from" } else { "moved_to" }
                    ),
                ));
                break;
            }
            record_participant(
                &mut tracked_projects,
                focal_project,
                focal_id,
                &hop_project,
                &read_hop_id,
                hop_id.to_string(),
                hop_pinned.clone(),
            );
            focal_ids.insert(hop_id.clone());
            suffixes.insert(hop_id.hex.clone());
            focal_hops.push((hop_project, hop_id.clone()));
            current_task = hop_pinned.task;
            prev_canon = hop_id;
        }
    }

    // §3.3 Project scope selection.
    let mut scan_projects = vec![focal_project.clone()];
    if all_projects {
        let (opened_scope, scope_warnings) =
            Scope::open_all_with_retired(registry, &focal_project.records)?;
        warnings.extend(scope_warnings);
        if registry.projects.is_empty() {
            diagnostics.push(IncomingDiagnostic {
                code: "registry_empty".into(),
                project: focal_project.prefix.clone(),
                holder: None,
                host: None,
                identity: None,
                source: None,
                detail: "Host project registry is empty; cross-project incoming custody cannot be verified".into(),
                action: "Register projects in projects.toml via `tasks init` before relying on --all-projects".into(),
            });
        }
        for prefix in registry.projects.keys() {
            if !scope::is_reachable(registry, prefix)? {
                diagnostics.push(IncomingDiagnostic {
                    code: "project_unreachable".into(),
                    project: prefix.clone(),
                    holder: None,
                    host: None,
                    identity: None,
                    source: None,
                    detail: format!(
                        "Registered project {prefix:?} is unreachable or missing tasks/.config.toml"
                    ),
                    action: format!(
                        "Restore {prefix:?}'s checkout/records or repoint it with `tasks init --force` before relying on incoming retained custody"
                    ),
                });
            }
        }
        for other in opened_scope.projects() {
            if registry.canonical_prefix(&other.prefix) != own_canon_prefix
                && other.records != focal_project.records
            {
                scan_projects.push(other.clone());
            }
        }
    }

    // §3.4 Snapshot-bound candidate discovery per project.
    let mut batch_readers: BTreeMap<PathBuf, BatchReader> = BTreeMap::new();
    let mut pinned_scans: Vec<PinnedProjectScan> = Vec::new();
    for proj in &scan_projects {
        if let Some(scan) = discovery::discover_project(
            focal_project,
            registry,
            proj,
            &known_git_roots,
            &mut batch_readers,
            &focal_ids,
            &suffixes,
            &mut diagnostics,
        ) {
            let is_focal = scan.project.prefix == focal_project.prefix
                && scan.project.records == focal_project.records;
            upsert_tracked_project(
                &mut tracked_projects,
                &mut known_git_roots,
                scan.project.clone(),
                discovery::SnapshotMeta {
                    git_root: scan.git_root.clone(),
                    basis_commit: scan.basis_commit.clone(),
                    ref_b: scan.ref_b.clone(),
                    tasks_rel: scan.tasks_rel.clone(),
                },
                is_focal,
                true,
            );
            pinned_scans.push(scan);
        }
    }
    drop(batch_readers);

    // Map canonical prefix -> pinned in-scope project for §3.5 Step 3 (`Moved` candidate holders).
    let mut in_scope_by_prefix: BTreeMap<String, Project> = BTreeMap::new();
    for scan in &pinned_scans {
        let canon_prefix = registry.canonical_prefix(&scan.project.prefix).to_string();
        in_scope_by_prefix
            .entry(canon_prefix)
            .or_insert_with(|| scan.project.clone());
    }

    // §3.5 Pinned candidate verification, `Moved` holders, spaceless resolutions, and blast radius.
    for scan in &pinned_scans {
        let p_pinned = &scan.project;
        for cand_id in &scan.candidates {
            let holder_canon = registry.canonical_id(cand_id).to_string();

            let cand_pinned = match super::basis::read(p_pinned, cand_id, None) {
                Ok(p) => p,
                Err(err) => {
                    record_failed_candidate(&mut tracked_projects, p_pinned, cand_id);
                    diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: p_pinned.prefix.clone(),
                        holder: Some(holder_canon),
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: "Repair the candidate holder's task record, required note archives, or basis before relying on incoming retained custody".into(),
                    });
                    continue;
                }
            };
            if let Err(err) = super::proof::validate_history(p_pinned, &cand_pinned, host) {
                record_failed_candidate(&mut tracked_projects, p_pinned, cand_id);
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: p_pinned.prefix.clone(),
                    holder: Some(holder_canon),
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Repair the candidate holder's retention proof history before relying on incoming retained custody".into(),
                });
                continue;
            }
            if let Err(err) = super::reconcile::validate_terminal_history(p_pinned, &cand_pinned) {
                record_failed_candidate(&mut tracked_projects, p_pinned, cand_id);
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: p_pinned.prefix.clone(),
                    holder: Some(holder_canon),
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Reconcile or repair the candidate holder's terminal operation history before relying on incoming retained custody".into(),
                });
                continue;
            }
            let cand_state = match super::state::replay(
                p_pinned,
                &cand_pinned.task,
                &cand_pinned.basis,
                host,
            ) {
                Ok(st) => st,
                Err(err) => {
                    record_failed_candidate(&mut tracked_projects, p_pinned, cand_id);
                    diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: p_pinned.prefix.clone(),
                        holder: Some(holder_canon),
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: "Repair the candidate holder's retention state history before relying on incoming retained custody".into(),
                    });
                    continue;
                }
            };

            record_participant(
                &mut tracked_projects,
                focal_project,
                focal_id,
                p_pinned,
                cand_id,
                holder_canon.clone(),
                cand_pinned.clone(),
            );

            // Step 3: `Moved` candidate holders (`cand_pinned.task.status == Status::Moved`).
            if cand_pinned.task.status == Status::Moved {
                verify_moved_holder_chain(
                    focal_project,
                    registry,
                    focal_id,
                    p_pinned,
                    &holder_canon,
                    &cand_pinned,
                    all_projects,
                    &own_canon_prefix,
                    &in_scope_by_prefix,
                    &mut tracked_projects,
                    &mut diagnostics,
                );
                continue;
            }

            // Step 4: Live/closed non-`Moved` candidate holders.
            evaluate_live_or_closed_holder(
                registry,
                p_pinned,
                &holder_canon,
                &cand_pinned,
                &cand_state,
                host,
                &focal_ids,
                &suffixes,
                &focal_hops,
                &mut items,
                &mut diagnostics,
            );
        }
    }

    Ok((
        PreparedIncoming {
            custody: IncomingCustody {
                scope,
                complete: Some(diagnostics.is_empty()),
                items,
                diagnostics,
            },
            suffixes,
            tracked_projects,
        },
        warnings,
    ))
}

#[allow(clippy::too_many_arguments)]
fn verify_moved_holder_chain(
    focal_project: &Project,
    registry: &Registry,
    focal_id: &TaskId,
    origin_project: &Project,
    origin_holder_canon: &str,
    origin_pinned: &PinnedTask,
    all_projects: bool,
    own_canon_prefix: &str,
    in_scope_by_prefix: &BTreeMap<String, Project>,
    tracked_projects: &mut [TrackedProject],
    diagnostics: &mut Vec<IncomingDiagnostic>,
) {
    let mut visited = BTreeSet::from([registry.canonical_id(&origin_pinned.task.id)]);
    let mut curr_pinned = origin_pinned.clone();
    let mut curr_canon = registry.canonical_id(&origin_pinned.task.id);

    while curr_pinned.task.status == Status::Moved {
        let Some(next_ptr) = &curr_pinned.task.moved_to else {
            diagnostics.push(move_incomplete_diag(
                origin_project.prefix.clone(),
                Some(origin_holder_canon.to_string()),
                format!("Moved candidate holder {curr_canon} has no moved_to target"),
            ));
            return;
        };
        let next_canon = registry.canonical_id(next_ptr);
        if !visited.insert(next_canon.clone()) {
            diagnostics.push(move_incomplete_diag(
                origin_project.prefix.clone(),
                Some(origin_holder_canon.to_string()),
                format!("Cycle detected in moved candidate holder chain at {next_canon}"),
            ));
            return;
        }
        if !all_projects && next_canon.prefix != own_canon_prefix {
            diagnostics.push(IncomingDiagnostic {
                code: "holder_moved_out_of_scope".into(),
                project: origin_project.prefix.clone(),
                holder: Some(origin_holder_canon.to_string()),
                host: None,
                identity: None,
                source: None,
                detail: format!(
                    "Candidate holder {origin_holder_canon} moved to {next_canon} outside project scope"
                ),
                action: "Re-run tasks retained state with --all-projects to verify the moved holder's live successor".into(),
            });
            return;
        }
        let Some(next_project) = in_scope_by_prefix.get(&next_canon.prefix) else {
            diagnostics.push(move_incomplete_diag(
                origin_project.prefix.clone(),
                Some(origin_holder_canon.to_string()),
                format!(
                    "Moved candidate holder {curr_canon} targets {next_canon} in unavailable or unpinned project {:?}",
                    next_canon.prefix
                ),
            ));
            return;
        };
        let read_next_id = if next_canon.prefix == next_project.prefix {
            next_canon.clone()
        } else {
            TaskId::parse(&format!("{}-{}", next_project.prefix, next_canon.hex))
                .unwrap_or_else(|_| next_canon.clone())
        };
        let next_pinned = match super::basis::read(next_project, &read_next_id, None) {
            Ok(p) => p,
            Err(err) => {
                record_failed_candidate(tracked_projects, next_project, &read_next_id);
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: next_project.prefix.clone(),
                    holder: Some(next_canon.to_string()),
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Repair the moved candidate holder's successor task record before relying on incoming retained custody".into(),
                });
                return;
            }
        };
        record_participant(
            tracked_projects,
            focal_project,
            focal_id,
            next_project,
            &read_next_id,
            next_canon.to_string(),
            next_pinned.clone(),
        );
        let reciprocal = registry.canonical_id(&next_pinned.task.id) == next_canon
            && next_pinned
                .task
                .moved_from
                .as_ref()
                .is_some_and(|m| registry.canonical_id(m) == curr_canon);
        if !reciprocal {
            diagnostics.push(move_incomplete_diag(
                next_project.prefix.clone(),
                Some(next_canon.to_string()),
                format!("Moved holder successor {next_canon} lacks reciprocal moved_from to {curr_canon}"),
            ));
            return;
        }
        let tomb_retained: Vec<&str> = curr_pinned
            .task
            .notes
            .iter()
            .filter(|n| n.text.starts_with("retained:"))
            .map(|n| n.text.as_str())
            .collect();
        let move_note_idx = next_pinned.task.notes.iter().position(|n| {
            n.text
                .strip_prefix("moved from ")
                .and_then(|s| s.split_whitespace().next())
                .and_then(|tok| TaskId::parse(tok).ok())
                .is_some_and(|tid| registry.canonical_id(&tid) == curr_canon)
        });
        let Some(move_idx) = move_note_idx else {
            diagnostics.push(move_incomplete_diag(
                next_project.prefix.clone(),
                Some(next_canon.to_string()),
                format!(
                    "Moved holder successor {next_canon} is missing 'moved from {curr_canon}' note"
                ),
            ));
            return;
        };
        let next_pre_move_retained: Vec<&str> = next_pinned.task.notes[..move_idx]
            .iter()
            .filter(|n| n.text.starts_with("retained:"))
            .map(|n| n.text.as_str())
            .collect();
        if !next_pre_move_retained.starts_with(&tomb_retained)
            || next_pre_move_retained.len() != tomb_retained.len()
        {
            diagnostics.push(move_incomplete_diag(
                next_project.prefix.clone(),
                Some(next_canon.to_string()),
                format!(
                    "Moved holder successor {next_canon} does not preserve exact pre-move retained: note prefix from {curr_canon}"
                ),
            ));
            return;
        }
        curr_pinned = next_pinned;
        curr_canon = next_canon;
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_live_or_closed_holder(
    registry: &Registry,
    p_pinned: &Project,
    holder_canon: &str,
    cand_pinned: &PinnedTask,
    cand_state: &super::state::RetainedState,
    host: Option<&str>,
    focal_ids: &BTreeSet<TaskId>,
    suffixes: &BTreeSet<String>,
    focal_hops: &[(Project, TaskId)],
    items: &mut Vec<IncomingItem>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) {
    // 1. Verified standalone spaceless resolutions (`resolved_spaceless`).
    let mut resolved_spaceless = BTreeSet::new();
    for (idx, note) in cand_pinned.task.notes.iter().enumerate() {
        let res_ordinal = idx as u64 + 1;
        if let Some(raw) = note.text.strip_prefix("retention resolution: ")
            && let Ok(res) = decode_resolution(raw)
        {
            let res_had_diag = cand_state
                .diagnostics
                .iter()
                .any(|d| d.source.as_ref().is_some_and(|s| s.ordinal == res_ordinal));
            if !res_had_diag {
                resolved_spaceless.insert(res.bad.ordinal);
            }
        }
    }

    // 2. Active incoming items + 3. Blocked holder identities (`item.coverage == "blocked"`).
    let mut holder_items: Vec<IncomingItem> = Vec::new();
    for item in &cand_state.items {
        if let Some(record) = &item.record {
            if record.state == "kept"
                && owner_matches_focal(
                    registry,
                    record,
                    focal_ids,
                    suffixes,
                    focal_hops,
                    diagnostics,
                )
            {
                holder_items.push(IncomingItem {
                    holder: holder_canon.to_string(),
                    identity: item.identity.clone(),
                    record: record.clone(),
                    source: item.source.clone().expect("recorded item has source"),
                    observed_at: item.observed_at.clone(),
                    checkpoint: item.checkpoint.clone(),
                    coverage: item.coverage.clone(),
                });
            }
        } else if item.coverage == "blocked" {
            let mut matched_historical = false;
            for note in &cand_pinned.task.notes {
                if let Some(raw) = note.text.strip_prefix("retained: ")
                    && let Ok(hist_rec) = decode_record(raw)
                    && hist_rec.identity() == item.identity
                    && owner_matches_focal(
                        registry,
                        &hist_rec,
                        focal_ids,
                        suffixes,
                        focal_hops,
                        diagnostics,
                    )
                {
                    matched_historical = true;
                    break;
                }
            }
            if matched_historical {
                let blocking_ordinals: Vec<String> = cand_state
                    .diagnostics
                    .iter()
                    .filter(|d| d.resolved_by.is_none())
                    .filter(|d| {
                        d.identity.as_ref().map_or_else(
                            || d.host.as_ref().is_none_or(|h| *h == item.identity.host),
                            |id| *id == item.identity,
                        )
                    })
                    .filter_map(|d| d.source.as_ref().map(|s| s.ordinal.to_string()))
                    .collect();
                diagnostics.push(IncomingDiagnostic {
                    code: "holder_item_blocked".into(),
                    project: p_pinned.prefix.clone(),
                    holder: Some(holder_canon.to_string()),
                    host: Some(item.identity.host.clone()),
                    identity: Some(item.identity.clone()),
                    source: item.source.clone(),
                    detail: format!(
                        "Candidate holder item {} on {} is quarantined by unresolved retention diagnostic(s) at ordinal(s) [{}]",
                        item.identity.path,
                        holder_canon,
                        blocking_ordinals.join(", ")
                    ),
                    action: "Repair the unresolved retention diagnostic(s) on the candidate holder via `retention resolution:` or `tasks retained checkpoint`".into(),
                });
            }
        }
    }

    // 4. Unresolved `cand_state` diagnostics whose source note mentions `suffixes`
    //    (or task-scoped `retention resolution:` failures targeting a relevant note on this holder).
    for d in cand_state
        .diagnostics
        .iter()
        .filter(|d| d.resolved_by.is_none())
    {
        let relevant = d
            .source
            .as_ref()
            .and_then(|s| usize::try_from(s.ordinal - 1).ok())
            .and_then(|idx| cand_pinned.task.notes.get(idx))
            .is_some_and(|note| {
                if discovery::notes_mention_suffixes(std::slice::from_ref(note), suffixes) {
                    return true;
                }
                if let Some(raw_res) = note.text.strip_prefix("retention resolution:") {
                    let Some(trimmed) = raw_res.strip_prefix(' ') else {
                        return true;
                    };
                    let Ok(res) = decode_resolution(trimmed) else {
                        return true;
                    };
                    let Some(bad_idx) = usize::try_from(res.bad.ordinal - 1).ok() else {
                        return true;
                    };
                    let Some(bad_note) = cand_pinned.task.notes.get(bad_idx) else {
                        return true;
                    };
                    if discovery::notes_mention_suffixes(std::slice::from_ref(bad_note), suffixes) {
                        return true;
                    }
                    return cand_pinned.task.notes[..bad_idx].iter().any(|prior| {
                        prior
                            .text
                            .strip_prefix("retained: ")
                            .and_then(|r| decode_record(r).ok())
                            .is_some_and(|rec| {
                                owner_matches_focal(
                                    registry,
                                    &rec,
                                    focal_ids,
                                    suffixes,
                                    focal_hops,
                                    diagnostics,
                                )
                            })
                    });
                }
                false
            });
        if relevant {
            diagnostics.push(IncomingDiagnostic {
                code: d.code.clone(),
                project: p_pinned.prefix.clone(),
                holder: Some(holder_canon.to_string()),
                host: d.host.clone(),
                identity: d.identity.clone(),
                source: d.source.clone(),
                detail: d.detail.clone(),
                action: d.action.clone(),
            });
        }
    }

    // 5. Query-only unresolved spaceless `retained:` diagnostics + blast-radius removal.
    for (idx, note) in cand_pinned.task.notes.iter().enumerate() {
        let Some(raw) = note
            .text
            .strip_prefix("retained:")
            .filter(|r| !r.starts_with(' '))
        else {
            continue;
        };
        let ordinal = idx as u64 + 1;
        if resolved_spaceless.contains(&ordinal) {
            continue;
        }
        let (extracted_host, extracted_identity) =
            super::state::extract_partial_scope(raw, &cand_pinned.task.id);
        if let (Some(filter_h), Some(ext_h)) = (host, extracted_host.as_deref())
            && filter_h != ext_h
        {
            continue;
        }

        let in_blast_radius = |id: &Identity| -> bool {
            if let Some(target_id) = &extracted_identity {
                id == target_id
            } else if let Some(target_host) = &extracted_host {
                id.host == *target_host
            } else {
                true
            }
        };

        let mentions_directly =
            discovery::notes_mention_suffixes(std::slice::from_ref(note), suffixes);
        let mut quarantines_prior_match = false;
        if !mentions_directly {
            for prior_note in &cand_pinned.task.notes[..idx] {
                if let Some(prior_raw) = prior_note.text.strip_prefix("retained: ")
                    && let Ok(prior_rec) = decode_record(prior_raw)
                    && in_blast_radius(&prior_rec.identity())
                    && owner_matches_focal(
                        registry,
                        &prior_rec,
                        focal_ids,
                        suffixes,
                        focal_hops,
                        diagnostics,
                    )
                {
                    quarantines_prior_match = true;
                    break;
                }
            }
        }

        if mentions_directly || quarantines_prior_match {
            diagnostics.push(IncomingDiagnostic {
                code: "malformed_record".into(),
                project: p_pinned.prefix.clone(),
                holder: Some(holder_canon.to_string()),
                host: extracted_host.clone(),
                identity: extracted_identity.clone(),
                source: Some(Source {
                    basis: cand_pinned.basis.clone(),
                    ordinal,
                }),
                detail: "missing space after 'retained:'".into(),
                action: SPACELESS_RETAINED_ACTION.into(),
            });
            holder_items
                .retain(|item| !(item.source.ordinal < ordinal && in_blast_radius(&item.identity)));
        }
    }

    items.extend(holder_items);
}

pub(crate) fn recheck_freshness(
    focal_project: &Project,
    registry: &Registry,
    focal_id: &TaskId,
    focal_pinned: &PinnedTask,
    prepared: PreparedIncoming,
) -> Result<IncomingCustody> {
    recheck::recheck_freshness(focal_project, registry, focal_id, focal_pinned, prepared)
}
