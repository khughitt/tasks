//! Snapshot-bound Git and non-Git candidate discovery for incoming retained custody.

use super::IncomingDiagnostic;
use crate::error::{Error, Result};
use crate::model::{Note, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

pub(crate) struct PinnedProjectScan {
    pub project: Project,
    pub git_root: PathBuf,
    pub basis_commit: String,
    pub ref_b: Option<String>,
    pub tasks_rel: String,
    pub candidates: BTreeSet<TaskId>,
}

pub(crate) struct BatchReader {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Drop for BatchReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl BatchReader {
    pub(crate) fn spawn(project: &Project) -> Result<Self> {
        let ctx = super::super::read::at(project, &project.records)?;
        let mut cmd = ctx.command(&["cat-file", "--batch"])?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        let stdin = BufWriter::new(
            child
                .stdin
                .take()
                .ok_or_else(|| Error::Io("missing cat-file stdin".into()))?,
        );
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| Error::Io("missing cat-file stdout".into()))?,
        );
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    pub(crate) fn read_blob(
        &mut self,
        oid: &str,
        max_bytes: Option<u64>,
        path_label: &str,
    ) -> Result<Vec<u8>> {
        writeln!(self.stdin, "{oid}")?;
        self.stdin.flush()?;

        let mut header = Vec::new();
        let read_hdr = self.stdout.read_until(b'\n', &mut header)?;
        if read_hdr == 0 || !header.ends_with(b"\n") {
            return Err(Error::RetainedHistoryBlocked(format!(
                "cat-file --batch returned truncated header for {oid} ({path_label})"
            )));
        }
        let header_str = std::str::from_utf8(&header)
            .map_err(|_| {
                Error::RetainedHistoryBlocked(format!(
                    "cat-file --batch returned non-UTF-8 header for {oid}"
                ))
            })?
            .trim_end_matches(['\r', '\n']);
        let mut parts = header_str.split(' ');
        let got_oid = parts.next().unwrap_or("");
        let obj_kind = parts.next().unwrap_or("");
        let size_str = parts.next().unwrap_or("");
        if got_oid != oid || obj_kind != "blob" || parts.next().is_some() {
            return Err(Error::RetainedHistoryBlocked(format!(
                "cat-file --batch unexpected header {header_str:?} for {oid} ({path_label})"
            )));
        }
        let size = size_str.parse::<u64>().map_err(|_| {
            Error::RetainedHistoryBlocked(format!(
                "cat-file --batch invalid blob size {size_str:?} for {oid}"
            ))
        })?;
        if let Some(cap) = max_bytes
            && size > cap
        {
            let drain_len = size.saturating_add(1);
            let mut take = (&mut self.stdout).take(drain_len);
            let _ = io::copy(&mut take, &mut io::sink())?;
            return Err(Error::AttachmentTooLarge(format!(
                "pinned {path_label} exceeds attachment cap {cap}"
            )));
        }
        let usize_len = usize::try_from(size).map_err(|_| {
            Error::RetainedHistoryBlocked(format!("blob {oid} exceeds address space"))
        })?;
        let mut buf = vec![0u8; usize_len];
        self.stdout.read_exact(&mut buf)?;
        let mut nl = [0u8; 1];
        self.stdout.read_exact(&mut nl)?;
        if nl != *b"\n" {
            return Err(Error::RetainedHistoryBlocked(format!(
                "cat-file --batch missing trailing newline for {oid}"
            )));
        }
        crate::history::record_metadata_read(buf.len());
        Ok(buf)
    }
}

#[derive(Clone)]
pub(super) struct SnapshotMeta {
    pub git_root: PathBuf,
    pub basis_commit: String,
    pub ref_b: Option<String>,
    pub tasks_rel: String,
}

pub(super) fn snapshot_meta(project: &Project) -> Result<SnapshotMeta> {
    let git_root = super::super::basis::root(project)?;
    let basis_commit = super::super::basis::resolve(project, "HEAD")?;
    let sym = super::super::read::run(project, &["symbolic-ref", "-q", "HEAD"])?.output;
    let ref_b = match sym.status.code() {
        Some(0) => Some(
            String::from_utf8(sym.stdout)
                .map_err(|_| Error::RetainedHistoryBlocked("non-UTF-8 symbolic-ref".into()))?
                .trim()
                .to_string(),
        ),
        Some(1) => None,
        _ => {
            return Err(Error::RetainedHistoryBlocked(format!(
                "symbolic-ref failed: {}",
                String::from_utf8_lossy(&sym.stderr).trim()
            )));
        }
    };
    let sub = super::super::lineage::subdir(project)?;
    let tasks_rel = if sub == "." {
        "tasks".to_string()
    } else {
        format!("{sub}/tasks")
    };
    Ok(SnapshotMeta {
        git_root,
        basis_commit,
        ref_b,
        tasks_rel,
    })
}

fn scan_failed_diag(
    project: &str,
    holder: Option<String>,
    detail: impl Into<String>,
) -> IncomingDiagnostic {
    IncomingDiagnostic {
        code: "project_scan_failed".into(),
        project: project.into(),
        holder,
        host: None,
        identity: None,
        source: None,
        detail: detail.into(),
        action: "Repair invalid or unreadable files in the project's tasks directory before relying on incoming retained custody".into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn discover_project(
    focal_ctx_project: &Project,
    registry: &Registry,
    project: &Project,
    known_git_roots: &BTreeSet<PathBuf>,
    batch_readers: &mut BTreeMap<PathBuf, BatchReader>,
    focal_ids: &BTreeSet<TaskId>,
    suffixes: &BTreeSet<String>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> Option<PinnedProjectScan> {
    let under_known_root = known_git_roots
        .iter()
        .any(|root| project.records.starts_with(root));
    if !under_known_root {
        match crate::link::GitTree::of(&project.records) {
            Err(err) => {
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: project.prefix.clone(),
                    holder: None,
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Repair the project's Git repository metadata before relying on incoming retained custody".into(),
                });
                return None;
            }
            Ok(None) => {
                scan_non_git_project(registry, project, focal_ids, suffixes, diagnostics);
                return None;
            }
            Ok(Some(_)) => {}
        }
    }

    let pinned_project = if project.retention_reads.is_some() {
        project.clone()
    } else {
        match super::super::read::inherit(focal_ctx_project, project.clone()) {
            Ok(p) => p,
            Err(err) => {
                diagnostics.push(IncomingDiagnostic {
                    code: err.kind().into(),
                    project: project.prefix.clone(),
                    holder: None,
                    host: None,
                    identity: None,
                    source: None,
                    detail: err.to_string(),
                    action: "Initialize or repair the project's Git HEAD and named branch before relying on incoming retained custody".into(),
                });
                return None;
            }
        }
    };

    let meta = match snapshot_meta(&pinned_project) {
        Ok(meta) => meta,
        Err(err) => {
            diagnostics.push(IncomingDiagnostic {
                code: err.kind().into(),
                project: pinned_project.prefix.clone(),
                holder: None,
                host: None,
                identity: None,
                source: None,
                detail: err.to_string(),
                action: "Recover the project's pinned Git HEAD before relying on incoming retained custody".into(),
            });
            return None;
        }
    };

    let candidates = scan_pinned_tree(
        registry,
        &pinned_project,
        &meta.git_root,
        &meta.basis_commit,
        &meta.tasks_rel,
        batch_readers,
        focal_ids,
        suffixes,
        diagnostics,
    );

    Some(PinnedProjectScan {
        project: pinned_project,
        git_root: meta.git_root,
        basis_commit: meta.basis_commit,
        ref_b: meta.ref_b,
        tasks_rel: meta.tasks_rel,
        candidates,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_pinned_tree(
    registry: &Registry,
    project: &Project,
    git_root: &Path,
    basis_commit: &str,
    tasks_rel: &str,
    batch_readers: &mut BTreeMap<PathBuf, BatchReader>,
    focal_ids: &BTreeSet<TaskId>,
    suffixes: &BTreeSet<String>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> BTreeSet<TaskId> {
    let mut candidates = BTreeSet::new();
    let tree_bytes = match super::super::basis::git(
        project,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--full-tree",
            basis_commit,
            "--",
            tasks_rel,
        ],
    ) {
        Ok(bytes) => bytes,
        Err(err) => {
            diagnostics.push(scan_failed_diag(&project.prefix, None, err.to_string()));
            return candidates;
        }
    };

    let tasks_prefix = format!("{tasks_rel}/");
    let mut task_blobs: BTreeMap<TaskId, String> = BTreeMap::new();
    let mut file_blobs: BTreeMap<TaskId, BTreeMap<String, (String, String, String)>> =
        BTreeMap::new();

    for raw_entry in tree_bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let Ok(entry) = std::str::from_utf8(raw_entry) else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                "non-UTF-8 entry in tasks Git tree",
            ));
            continue;
        };
        let Some((meta, full_path)) = entry.split_once('\t') else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("malformed ls-tree entry {entry:?}"),
            ));
            continue;
        };
        let mut meta_parts = meta.split_whitespace();
        let (Some(mode), Some(obj_type), Some(oid), None) = (
            meta_parts.next(),
            meta_parts.next(),
            meta_parts.next(),
            meta_parts.next(),
        ) else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("malformed ls-tree metadata {meta:?}"),
            ));
            continue;
        };
        let Some(rel) = full_path.strip_prefix(&tasks_prefix) else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("unexpected ls-tree path {full_path:?} outside {tasks_prefix}"),
            ));
            continue;
        };

        if !rel.contains('/') {
            if rel.starts_with('.') {
                continue;
            }
            let regular = (mode == "100644" || mode == "100755") && obj_type == "blob";
            let Some(stem) = rel.strip_suffix(".md") else {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    None,
                    format!("unexpected non-task entry {full_path:?} in tasks tree"),
                ));
                continue;
            };
            let Ok(tid) = TaskId::parse(stem) else {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    None,
                    format!("invalid task filename {full_path:?}"),
                ));
                continue;
            };
            if !regular {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(registry.canonical_id(&tid).to_string()),
                    format!("non-regular task entry {full_path:?} ({mode} {obj_type})"),
                ));
                continue;
            }
            task_blobs.insert(tid, oid.to_string());
        } else if let Some(after_files) = rel.strip_prefix("files/") {
            let Some((tid_str, rest)) = after_files.split_once('/') else {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    None,
                    format!("malformed entry under tasks/files: {full_path:?}"),
                ));
                continue;
            };
            let Ok(tid) = TaskId::parse(tid_str) else {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    None,
                    format!("invalid task directory under tasks/files: {full_path:?}"),
                ));
                continue;
            };
            if rest.is_empty() {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(registry.canonical_id(&tid).to_string()),
                    format!("empty attachment name in {full_path:?}"),
                ));
                continue;
            }
            file_blobs.entry(tid).or_default().insert(
                rest.to_string(),
                (mode.to_string(), obj_type.to_string(), oid.to_string()),
            );
        } else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("unexpected nested entry {full_path:?} in tasks tree"),
            ));
        }
    }

    if task_blobs.is_empty() {
        return candidates;
    }

    if !batch_readers.contains_key(git_root) {
        match BatchReader::spawn(project) {
            Ok(reader) => {
                batch_readers.insert(git_root.to_path_buf(), reader);
            }
            Err(err) => {
                diagnostics.push(scan_failed_diag(&project.prefix, None, err.to_string()));
                return candidates;
            }
        }
    }
    let reader = batch_readers
        .get_mut(git_root)
        .expect("inserted batch reader");

    for (cand_id, oid) in task_blobs {
        if focal_ids.contains(&registry.canonical_id(&cand_id)) {
            continue;
        }
        let task_rel_path = format!("{tasks_rel}/{cand_id}.md");
        let raw_bytes = match reader.read_blob(&oid, None, &task_rel_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(registry.canonical_id(&cand_id).to_string()),
                    err.to_string(),
                ));
                continue;
            }
        };
        let Ok(raw_md) = std::str::from_utf8(&raw_bytes) else {
            candidates.insert(cand_id);
            continue;
        };
        if !raw_md.contains("retained:") && !raw_md.contains("note_archives:") {
            continue;
        }
        let Ok(physical) = crate::format::parse_physical(raw_md, &task_rel_path) else {
            candidates.insert(cand_id);
            continue;
        };
        let cand_files = file_blobs.get(&cand_id);
        let logical = crate::history::load_logical(physical, |archive| {
            let name = archive.file_name();
            let archive_path = format!("{tasks_rel}/files/{cand_id}/{name}");
            let Some((mode, obj_type, arc_oid)) = cand_files.and_then(|m| m.get(&name)) else {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Missing pinned archive {basis_commit}:{archive_path}"
                )));
            };
            if !((mode == "100644" || mode == "100755") && obj_type == "blob") {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Unsafe pinned archive {basis_commit}:{archive_path}"
                )));
            }
            reader.read_blob(arc_oid, Some(project.attachments_max_bytes), &archive_path)
        });
        match logical {
            Ok(task) if task.id == cand_id => {
                if notes_mention_suffixes(&task.notes, suffixes) {
                    candidates.insert(cand_id);
                }
            }
            Ok(_) | Err(_) => {
                candidates.insert(cand_id);
            }
        }
    }

    candidates
}

fn scan_non_git_project(
    registry: &Registry,
    project: &Project,
    focal_ids: &BTreeSet<TaskId>,
    suffixes: &BTreeSet<String>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) {
    let tasks_dir = project.records.join("tasks");
    let entries = match std::fs::read_dir(&tasks_dir) {
        Ok(entries) => entries,
        Err(err) => {
            diagnostics.push(scan_failed_diag(&project.prefix, None, err.to_string()));
            return;
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                diagnostics.push(scan_failed_diag(&project.prefix, None, err.to_string()));
                continue;
            }
        };
        let file_name = entry.file_name();
        if file_name.as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                diagnostics.push(scan_failed_diag(&project.prefix, None, err.to_string()));
                continue;
            }
        };
        let Some(name_str) = file_name.to_str() else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                "non-UTF-8 entry in non-Git tasks directory",
            ));
            continue;
        };
        if name_str == "files" && meta.is_dir() {
            continue;
        }
        let Some(stem) = name_str.strip_suffix(".md") else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("unexpected entry {name_str:?} in non-Git tasks directory"),
            ));
            continue;
        };
        let Ok(cand_id) = TaskId::parse(stem) else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                None,
                format!("invalid task filename {name_str:?}"),
            ));
            continue;
        };
        if !meta.is_file() {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                Some(registry.canonical_id(&cand_id).to_string()),
                format!("non-regular task file {name_str:?}"),
            ));
            continue;
        }
        files.push((cand_id, path));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    for (cand_id, path) in files {
        let canon = registry.canonical_id(&cand_id);
        if focal_ids.contains(&canon) {
            continue;
        }
        let raw_bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(err) => {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(canon.to_string()),
                    err.to_string(),
                ));
                continue;
            }
        };
        crate::history::record_metadata_read(raw_bytes.len());
        let Ok(raw_md) = std::str::from_utf8(&raw_bytes) else {
            diagnostics.push(scan_failed_diag(
                &project.prefix,
                Some(canon.to_string()),
                format!("tasks/{cand_id}.md is not valid UTF-8"),
            ));
            continue;
        };
        if !raw_md.contains("retained:") && !raw_md.contains("note_archives:") {
            continue;
        }
        let file_label = format!("tasks/{cand_id}.md");
        let physical = match crate::format::parse_physical(raw_md, &file_label) {
            Ok(p) => p,
            Err(err) => {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(canon.to_string()),
                    err.to_string(),
                ));
                continue;
            }
        };
        let logical = crate::history::load_logical(physical, |archive| {
            crate::history::read_archive_at(
                &project.records,
                &cand_id,
                archive,
                project.attachments_max_bytes,
            )
        });
        match logical {
            Ok(task) if task.id == cand_id => {
                if notes_mention_suffixes(&task.notes, suffixes) {
                    diagnostics.push(IncomingDiagnostic {
                        code: "retained_publication_unsupported".into(),
                        project: project.prefix.clone(),
                        holder: Some(canon.to_string()),
                        host: None,
                        identity: None,
                        source: None,
                        detail: "Candidate holder's project records are not Git-backed; incoming retained custody requires a Git-pinned basis".into(),
                        action: "Inspect the holder's non-Git records manually via the immutable-snapshot recovery route or initialize Git-backed records".into(),
                    });
                }
            }
            Ok(task) => {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(canon.to_string()),
                    format!("tasks/{cand_id}.md declares mismatched id {}", task.id),
                ));
            }
            Err(err) => {
                diagnostics.push(scan_failed_diag(
                    &project.prefix,
                    Some(canon.to_string()),
                    err.to_string(),
                ));
            }
        }
    }
}

fn value_mentions_suffix(value: &Value, suffixes: &BTreeSet<String>) -> bool {
    match value {
        Value::String(s) => {
            let lower = s.to_ascii_lowercase();
            suffixes.iter().any(|hex| lower.contains(hex.as_str()))
        }
        Value::Array(items) => items.iter().any(|v| value_mentions_suffix(v, suffixes)),
        Value::Object(map) => map.iter().any(|(k, v)| {
            let k_lower = k.to_ascii_lowercase();
            suffixes.iter().any(|hex| k_lower.contains(hex.as_str()))
                || value_mentions_suffix(v, suffixes)
        }),
        _ => false,
    }
}

pub(crate) fn notes_mention_suffixes(notes: &[Note], suffixes: &BTreeSet<String>) -> bool {
    if suffixes.is_empty() {
        return false;
    }
    for note in notes {
        let Some(after_prefix) = note.text.strip_prefix("retained:") else {
            continue;
        };
        if let Some(raw) = after_prefix.strip_prefix(' ')
            && let Ok(record) = super::super::model::decode_record(raw)
        {
            if record
                .next_owner
                .strip_prefix("task:")
                .and_then(|s| TaskId::parse(s).ok())
                .is_some_and(|tid| suffixes.contains(&tid.hex))
            {
                return true;
            }
            continue;
        }
        let lower = note.text.to_ascii_lowercase();
        if suffixes.iter().any(|hex| lower.contains(hex.as_str())) {
            return true;
        }
        let trimmed = after_prefix.trim();
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed)
            && value_mentions_suffix(&parsed, suffixes)
        {
            return true;
        }
    }
    false
}
