//! Shared staged-index record guard for `check --staged` and plain `check` in Git-backed
//! records checkouts: classifies index blobs changed since HEAD (including rename targets
//! and unborn HEAD) and refuses changed unsupported or malformed records even when the
//! working tree holds valid bytes.

use crate::entries::{RecordEnvelope, inspect_envelope};
use crate::error::{Error, Result};
use crate::format::parse_physical;
use crate::model::TaskId;
use crate::output::Finding;
use crate::repo::Project;
use crate::staged::snapshot::{self, Snapshot};
use std::collections::BTreeSet;

/// Git's well-known empty tree OID, used to diff against an unborn HEAD or absent project tree.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Default)]
pub(crate) struct StagedGuardReport {
    pub(crate) errors: Vec<Finding>,
    pub(crate) untouched_future_warnings: Vec<Finding>,
}

fn task_id_from_path(path: &str) -> Option<&str> {
    let name = path.strip_prefix("tasks/")?;
    if name.contains('/') || name.starts_with('.') {
        return None;
    }
    name.strip_suffix(".md")
}

/// Inspects every staged task file added or modified relative to HEAD (in `snap`'s scoped
/// project tree), plus collects `unsupported_record_version` warnings for untouched future
/// siblings in `snap.tree`.
pub(crate) fn check_index(project: &Project, snap: &Snapshot) -> Result<StagedGuardReport> {
    let base_tree = snap
        .head
        .as_ref()
        .and_then(|head| head.tree.as_deref())
        .unwrap_or(EMPTY_TREE);
    let changes = snapshot::changes(&snap.top, base_tree, &snap.tree)?;

    let mut report = StagedGuardReport::default();
    let mut changed_task_paths = BTreeSet::new();

    for change in &changes {
        let Some(stem) = task_id_from_path(&change.path) else {
            continue;
        };
        // Deleted records have no new blob to validate.
        if change.status == 'D' {
            continue;
        }
        changed_task_paths.insert(change.path.clone());
        let Ok(id) = TaskId::parse(stem) else {
            report.errors.push(Finding {
                id: None,
                file: change.path.clone(),
                kind: "parse".into(),
                detail: format!("{}: filename is not a valid task id", change.path),
            });
            continue;
        };
        if !matches!(change.new_mode.as_str(), "100644" | "100755") {
            report.errors.push(Finding {
                id: Some(id.to_string()),
                file: change.path.clone(),
                kind: "parse".into(),
                detail: format!(
                    "{}: staged record must be a regular file (mode {})",
                    change.path, change.new_mode
                ),
            });
            continue;
        }
        let bytes = snapshot::blob(&snap.top, &change.new_oid)?;
        let Ok(text) = String::from_utf8(bytes) else {
            report.errors.push(Finding {
                id: Some(id.to_string()),
                file: change.path.clone(),
                kind: "parse".into(),
                detail: format!("{}: staged record is not valid UTF-8", change.path),
            });
            continue;
        };
        match inspect_envelope(&text, &change.path) {
            Ok(RecordEnvelope::Future(future)) => {
                if future.id != id || future.id.prefix != project.prefix {
                    report.errors.push(Finding {
                        id: Some(id.to_string()),
                        file: change.path.clone(),
                        kind: "parse".into(),
                        detail: format!(
                            "{}: staged record id {} does not match filename/project {}",
                            change.path, future.id, project.prefix
                        ),
                    });
                } else {
                    report.errors.push(Finding {
                        id: Some(future.id.to_string()),
                        file: change.path.clone(),
                        kind: "unsupported_record_version".into(),
                        detail: format!(
                            "staged change writes unsupported record format {} to {}",
                            future.format, change.path
                        ),
                    });
                }
            }
            Ok(RecordEnvelope::Format1 | RecordEnvelope::Format2) => {
                match parse_physical(&text, &change.path) {
                    Ok(physical) => {
                        if physical.id() != &id {
                            report.errors.push(Finding {
                                id: Some(id.to_string()),
                                file: change.path.clone(),
                                kind: "parse".into(),
                                detail: format!("{}: id field is {}", change.path, physical.id()),
                            });
                        } else if physical.id().prefix != project.prefix {
                            report.errors.push(Finding {
                                id: Some(id.to_string()),
                                file: change.path.clone(),
                                kind: "parse".into(),
                                detail: format!(
                                    "{}: prefix {:?} does not match project prefix {:?}",
                                    change.path,
                                    physical.id().prefix,
                                    project.prefix
                                ),
                            });
                        }
                    }
                    Err(error) => {
                        report.errors.push(Finding {
                            id: Some(id.to_string()),
                            file: change.path.clone(),
                            kind: "parse".into(),
                            detail: error.to_string(),
                        });
                    }
                }
            }
            Err(error) => {
                report.errors.push(Finding {
                    id: Some(id.to_string()),
                    file: change.path.clone(),
                    kind: "parse".into(),
                    detail: error.to_string(),
                });
            }
        }
    }

    // Collect warnings for untouched future-format siblings in the staged tree so both
    // fast-path and full staged audits surface them consistently.
    let tree_list = crate::git::at(&snap.top)
        .args(["ls-tree", "-z", "--full-tree", &snap.tree, "--", "tasks/"])
        .output()?;
    if tree_list.status.success() {
        for raw_entry in tree_list
            .stdout
            .split(|b| *b == 0)
            .filter(|r| !r.is_empty())
        {
            let Ok(entry_str) = std::str::from_utf8(raw_entry) else {
                continue;
            };
            let Some((meta, path)) = entry_str.split_once('\t') else {
                continue;
            };
            if changed_task_paths.contains(path) {
                continue;
            }
            let Some(stem) = task_id_from_path(path) else {
                continue;
            };
            let Ok(id) = TaskId::parse(stem) else {
                continue;
            };
            let parts: Vec<&str> = meta.split(' ').collect();
            if parts.len() != 3 || !matches!(parts[0], "100644" | "100755") {
                continue;
            }
            let Ok(bytes) = snapshot::blob(&snap.top, parts[2]) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            if let Ok(RecordEnvelope::Future(future)) = inspect_envelope(text, path)
                && future.id == id
                && future.id.prefix == project.prefix
            {
                report.untouched_future_warnings.push(Finding {
                    id: Some(future.id.to_string()),
                    file: path.to_string(),
                    kind: "unsupported_record_version".into(),
                    detail: future.warning(),
                });
            }
        }
    }

    Ok(report)
}

/// Runs the changed-index record guard for plain `tasks check` when the records checkout is
/// inside a Git repository with an existing index. Non-Git checkouts return `Ok(None)`.
pub(crate) fn check_plain_git_index(project: &Project) -> Result<Option<StagedGuardReport>> {
    let inherited = std::env::var_os(snapshot::INDEX_ENV);
    let index = match snapshot::index_file(&project.records, inherited) {
        Ok(path) => path,
        Err(Error::StagedSnapshot(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    if !index.is_file() {
        return Ok(None);
    }
    let snap = match snapshot::read(&project.records, &index) {
        Ok(snap) => snap,
        // A project directory not yet staged in the index has no staged changes to guard.
        Err(Error::StagedSnapshot(detail))
            if detail.contains("not a git repository") || detail.contains("prefix") =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    check_index(project, &snap).map(Some)
}
