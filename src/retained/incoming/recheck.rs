//! Joint stabilizing live freshness recheck for incoming retained custody.

use super::discovery::{BatchReader, notes_mention_suffixes};
use super::{IncomingCustody, IncomingDiagnostic, IncomingItem, PreparedIncoming, TrackedProject};
use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::registry::Registry;
use crate::repo::Project;
use crate::retained::basis::{self, PinnedTask};
use crate::retained::{freshness, read};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[cfg(test)]
thread_local! {
    pub(crate) static DURING_INCOMING_RECHECK:
        std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

const STALE_ACTION: &str =
    "Publish or recover the participant's clean Git basis and retry 'tasks retained state'";
const SCAN_FAILED_ACTION: &str = "Repair invalid or unreadable files in the project's tasks directory before relying on incoming retained custody";

struct ProjectRecheckState {
    active: bool,
    checked_head: String,
    checked_edges: BTreeSet<(String, String)>,
}

enum ClassifiedRelPath {
    Ignore,
    TaskMd(TaskId),
    TaskAttachment { tid: TaskId, rest: String },
    TaskFilesDir(TaskId),
    Invalid(String),
}

fn stale_focal_error(detail: impl Into<String>) -> Error {
    Error::RetainedStaleBasis(detail.into())
}

fn stale_diag(
    project: &str,
    holder: Option<String>,
    detail: impl Into<String>,
) -> IncomingDiagnostic {
    IncomingDiagnostic {
        code: "retained_stale_basis".into(),
        project: project.into(),
        holder,
        host: None,
        identity: None,
        source: None,
        detail: detail.into(),
        action: STALE_ACTION.into(),
    }
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
        action: SCAN_FAILED_ACTION.into(),
    }
}

fn drop_project_items(items: &mut Vec<IncomingItem>, tp: &TrackedProject) {
    let prefix_dash = format!("{}-", tp.project.prefix);
    items.retain(|item| {
        item.source.basis.project != tp.project.prefix
            && !item.holder.starts_with(&prefix_dash)
            && !tp
                .participants
                .values()
                .any(|p| p.canonical_id == item.holder)
    });
}

fn drop_holder_items(items: &mut Vec<IncomingItem>, holder_canon: &str) {
    items.retain(|item| item.holder != holder_canon);
}

fn live_ref_and_head(live_p: &Project) -> Result<(Option<String>, String)> {
    let sym = read::run(live_p, &["symbolic-ref", "-q", "HEAD"])?.output;
    let ref_now = match sym.status.code() {
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
    let head_now = basis::resolve(live_p, "HEAD")?;
    Ok((ref_now, head_now))
}

fn verify_ref_and_ancestry_cached(
    tp: &TrackedProject,
    live_p: &Project,
    checked_head: &str,
    repo_heads: &mut BTreeMap<PathBuf, (Option<String>, String)>,
) -> Result<String> {
    let (ref_now, head_now) = if let Some(cached) = repo_heads.get(&tp.git_root) {
        cached.clone()
    } else {
        let pair = live_ref_and_head(live_p)?;
        repo_heads.insert(tp.git_root.clone(), pair.clone());
        pair
    };
    if ref_now != tp.ref_b {
        return Err(stale_focal_error(format!(
            "Project {} HEAD symbolic-ref changed from {:?} to {:?} during live freshness recheck",
            tp.project.prefix, tp.ref_b, ref_now
        )));
    }
    if !read::ancestor(live_p, checked_head, &head_now)? {
        return Err(stale_focal_error(format!(
            "Project {} HEAD {} does not descend from pinned commit {}",
            tp.project.prefix, head_now, checked_head
        )));
    }
    Ok(head_now)
}

fn classify_rel_path(tasks_rel: &str, full_path: &str, allow_dir_slash: bool) -> ClassifiedRelPath {
    let tasks_prefix = format!("{tasks_rel}/");
    let Some(rel) = full_path.strip_prefix(&tasks_prefix) else {
        if allow_dir_slash && full_path.trim_end_matches('/') == tasks_rel {
            return ClassifiedRelPath::Ignore;
        }
        return ClassifiedRelPath::Invalid(format!(
            "unexpected path {full_path:?} outside {tasks_prefix}"
        ));
    };
    if rel.is_empty() {
        return ClassifiedRelPath::Ignore;
    }
    if !rel.contains('/') {
        if rel.starts_with('.') {
            return ClassifiedRelPath::Ignore;
        }
        if allow_dir_slash && rel == "files" {
            return ClassifiedRelPath::Ignore;
        }
        let Some(stem) = rel.strip_suffix(".md") else {
            return ClassifiedRelPath::Invalid(format!(
                "unexpected non-task entry {full_path:?} in tasks directory"
            ));
        };
        return match TaskId::parse(stem) {
            Ok(tid) => ClassifiedRelPath::TaskMd(tid),
            Err(_) => ClassifiedRelPath::Invalid(format!("invalid task filename {full_path:?}")),
        };
    }
    if let Some(after_files) = rel.strip_prefix("files/") {
        if after_files.is_empty() {
            return ClassifiedRelPath::Ignore;
        }
        if let Some((tid_str, rest)) = after_files.split_once('/') {
            let Ok(tid) = TaskId::parse(tid_str) else {
                return ClassifiedRelPath::Invalid(format!(
                    "invalid task directory under tasks/files: {full_path:?}"
                ));
            };
            if rest.is_empty() {
                if allow_dir_slash && full_path.ends_with('/') {
                    return ClassifiedRelPath::TaskFilesDir(tid);
                }
                return ClassifiedRelPath::Invalid(format!(
                    "empty attachment entry in {full_path:?}"
                ));
            }
            return ClassifiedRelPath::TaskAttachment {
                tid,
                rest: rest.to_string(),
            };
        }
        if allow_dir_slash && let Ok(tid) = TaskId::parse(after_files) {
            return ClassifiedRelPath::TaskFilesDir(tid);
        }
        return ClassifiedRelPath::Invalid(format!(
            "malformed entry under tasks/files: {full_path:?}"
        ));
    }
    ClassifiedRelPath::Invalid(format!(
        "unexpected nested entry {full_path:?} in tasks directory"
    ))
}

fn check_task_at_commit(
    project: &Project,
    live_p: &Project,
    reader: &mut BatchReader,
    rev: &str,
    tasks_rel: &str,
    tid: &TaskId,
    suffixes: &BTreeSet<String>,
) -> bool {
    let md_rel = format!("{tasks_rel}/{tid}.md");
    let files_rel = format!("{tasks_rel}/files/{tid}");
    let tree_bytes = match basis::git(
        live_p,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--full-tree",
            rev,
            "--",
            &md_rel,
            &files_rel,
        ],
    ) {
        Ok(b) => b,
        Err(_) => return true,
    };

    let mut md_entry: Option<(String, String, String)> = None;
    let mut file_entries: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    let files_prefix = format!("{files_rel}/");

    for raw in tree_bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let Ok(line) = std::str::from_utf8(raw) else {
            return true;
        };
        let Some((meta, path)) = line.split_once('\t') else {
            return true;
        };
        let mut parts = meta.split_whitespace();
        let (Some(mode), Some(obj_type), Some(oid), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return true;
        };
        if path == md_rel {
            md_entry = Some((mode.to_string(), obj_type.to_string(), oid.to_string()));
        } else if let Some(rest) = path.strip_prefix(&files_prefix) {
            file_entries.insert(
                rest.to_string(),
                (mode.to_string(), obj_type.to_string(), oid.to_string()),
            );
        } else {
            return true;
        }
    }

    if let Some((mode, obj_type, oid)) = md_entry {
        if !((mode == "100644" || mode == "100755") && obj_type == "blob") {
            return true;
        }
        let Ok(raw_bytes) = reader.read_blob(&oid, None, &md_rel) else {
            return true;
        };
        let Ok(raw_md) = std::str::from_utf8(&raw_bytes) else {
            return true;
        };
        let Ok(physical) = crate::format::parse_physical(raw_md, &md_rel) else {
            return true;
        };
        let logical = crate::history::load_logical(physical, |archive| {
            let name = archive.file_name();
            let arc_path = format!("{files_rel}/{name}");
            let Some((amode, atype, aoid)) = file_entries.get(&name) else {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Missing archive {rev}:{arc_path}"
                )));
            };
            if !((amode == "100644" || amode == "100755") && atype == "blob") {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Unsafe archive {rev}:{arc_path}"
                )));
            }
            reader.read_blob(aoid, Some(project.attachments_max_bytes), &arc_path)
        });
        match logical {
            Ok(task) if &task.id == tid => notes_mention_suffixes(&task.notes, suffixes),
            Ok(_) | Err(_) => true,
        }
    } else {
        for (name, (mode, obj_type, oid)) in &file_entries {
            if name.starts_with("notes-") && name.ends_with(".notes") {
                if !((mode == "100644" || mode == "100755") && obj_type == "blob") {
                    return true;
                }
                let arc_path = format!("{files_rel}/{name}");
                let Ok(bytes) =
                    reader.read_blob(oid, Some(project.attachments_max_bytes), &arc_path)
                else {
                    return true;
                };
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    return true;
                };
                if text.contains("retained:") {
                    return true;
                }
            }
        }
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_commit_edges(
    ctx: &Ctx,
    focal_id: &TaskId,
    tp: &TrackedProject,
    live_p: &Project,
    head_1: &str,
    checked_edges: &mut BTreeSet<(String, String)>,
    suffixes: &BTreeSet<String>,
    items: &mut Vec<IncomingItem>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> Result<()> {
    let range = format!("{}..{head_1}", tp.basis_commit);
    let raw_commits = match basis::git(live_p, &["rev-list", "--parents", &range]) {
        Ok(b) => b,
        Err(err) => {
            if tp.is_focal_project {
                return Err(err);
            }
            drop_project_items(items, tp);
            diagnostics.push(IncomingDiagnostic {
                code: err.kind().into(),
                project: tp.project.prefix.clone(),
                holder: None,
                host: None,
                identity: None,
                source: None,
                detail: err.to_string(),
                action: STALE_ACTION.into(),
            });
            return Ok(());
        }
    };
    let Ok(commits_str) = std::str::from_utf8(&raw_commits) else {
        if tp.is_focal_project {
            return Err(stale_focal_error("non-UTF-8 rev-list output"));
        }
        drop_project_items(items, tp);
        diagnostics.push(stale_diag(
            &tp.project.prefix,
            None,
            "non-UTF-8 rev-list output",
        ));
        return Ok(());
    };
    let mut edges: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in commits_str.lines() {
        let mut oids = line.split_whitespace();
        let Some(commit) = oids.next() else {
            continue;
        };
        edges.push((commit, oids.collect()));
    }
    if edges.is_empty() {
        return Ok(());
    }
    let graph: BTreeMap<&str, &[&str]> = edges.iter().map(|(c, p)| (*c, p.as_slice())).collect();
    let seeds = BTreeSet::from([tp.basis_commit.as_str()]);
    let lin = freshness::lineage(&graph, &seeds);

    let mut reader: Option<BatchReader> = None;

    for (commit, parents) in &edges {
        for (p_idx, parent) in parents.iter().enumerate() {
            if p_idx > 0 && lin.contains(commit) && !seeds.contains(parent) && !lin.contains(parent)
            {
                continue;
            }
            if !checked_edges.insert(((*parent).to_string(), (*commit).to_string())) {
                continue;
            }
            let diff_bytes = match basis::git(
                live_p,
                &[
                    "diff-tree",
                    "-r",
                    "--name-only",
                    "--no-commit-id",
                    "-z",
                    parent,
                    commit,
                    "--",
                    &tp.tasks_rel,
                ],
            ) {
                Ok(b) => b,
                Err(err) => {
                    if tp.is_focal_project {
                        return Err(err);
                    }
                    drop_project_items(items, tp);
                    diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: tp.project.prefix.clone(),
                        holder: None,
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: STALE_ACTION.into(),
                    });
                    return Ok(());
                }
            };

            let mut touched_tids: BTreeSet<TaskId> = BTreeSet::new();
            for raw_entry in diff_bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
                let Ok(path_str) = std::str::from_utf8(raw_entry) else {
                    diagnostics.push(scan_failed_diag(
                        &tp.project.prefix,
                        None,
                        format!("non-UTF-8 diff-tree path on {parent}..{commit}"),
                    ));
                    continue;
                };
                match classify_rel_path(&tp.tasks_rel, path_str, false) {
                    ClassifiedRelPath::Ignore => {}
                    ClassifiedRelPath::TaskMd(tid)
                    | ClassifiedRelPath::TaskAttachment { tid, .. }
                    | ClassifiedRelPath::TaskFilesDir(tid) => {
                        touched_tids.insert(tid);
                    }
                    ClassifiedRelPath::Invalid(detail) => {
                        diagnostics.push(scan_failed_diag(&tp.project.prefix, None, detail));
                    }
                }
            }

            for tid in touched_tids {
                if tp.is_focal_project && &tid == focal_id {
                    return Err(stale_focal_error(format!(
                        "Queried task {focal_id} changed on commit edge {parent}..{commit} since its pinned basis"
                    )));
                }
                if let Some(entry) = tp.participants.get(&tid) {
                    drop_holder_items(items, &entry.canonical_id);
                    diagnostics.push(stale_diag(
                        &tp.project.prefix,
                        Some(entry.canonical_id.clone()),
                        format!(
                            "Participant {} changed on commit edge {parent}..{commit} since its pinned basis",
                            entry.canonical_id
                        ),
                    ));
                    continue;
                }

                if reader.is_none() {
                    match BatchReader::spawn(live_p) {
                        Ok(r) => reader = Some(r),
                        Err(err) => {
                            if tp.is_focal_project {
                                return Err(err);
                            }
                            drop_project_items(items, tp);
                            diagnostics.push(scan_failed_diag(
                                &tp.project.prefix,
                                None,
                                err.to_string(),
                            ));
                            return Ok(());
                        }
                    }
                }
                let batch = reader.as_mut().expect("batch reader spawned");
                let tid_canon = ctx.registry.canonical_id(&tid).to_string();
                let bad_parent = check_task_at_commit(
                    &tp.project,
                    live_p,
                    batch,
                    parent,
                    &tp.tasks_rel,
                    &tid,
                    suffixes,
                );
                let bad_commit = check_task_at_commit(
                    &tp.project,
                    live_p,
                    batch,
                    commit,
                    &tp.tasks_rel,
                    &tid,
                    suffixes,
                );
                if bad_parent || bad_commit {
                    drop_holder_items(items, &tid_canon);
                    diagnostics.push(stale_diag(
                        &tp.project.prefix,
                        Some(tid_canon.clone()),
                        format!(
                            "Task {tid_canon} changed on commit edge {parent}..{commit} with matching or unverified retained history"
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn parse_porcelain_z_paths<'a>(
    status_bytes: &'a [u8],
    fallback_prefix: &str,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> Vec<&'a str> {
    let mut paths = Vec::new();
    let mut fields = status_bytes.split(|b| *b == 0);
    while let Some(raw) = fields.next() {
        if raw.is_empty() {
            continue;
        }
        if raw.len() < 4 || raw[2] != b' ' {
            diagnostics.push(scan_failed_diag(
                fallback_prefix,
                None,
                format!(
                    "malformed git status entry {:?}",
                    String::from_utf8_lossy(raw)
                ),
            ));
            continue;
        }
        let x = raw[0];
        let y = raw[1];
        match std::str::from_utf8(&raw[3..]) {
            Ok(p) => paths.push(p),
            Err(_) => {
                diagnostics.push(scan_failed_diag(
                    fallback_prefix,
                    None,
                    "non-UTF-8 path in git status",
                ));
            }
        }
        if (matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C'))
            && let Some(orig_raw) = fields.next()
            && !orig_raw.is_empty()
        {
            match std::str::from_utf8(orig_raw) {
                Ok(p) => paths.push(p),
                Err(_) => {
                    diagnostics.push(scan_failed_diag(
                        fallback_prefix,
                        None,
                        "non-UTF-8 rename source path in git status",
                    ));
                }
            }
        }
    }
    paths
}

fn touches_required_task_inputs(
    tid: &TaskId,
    pinned: &PinnedTask,
    touched_subpaths: &BTreeSet<String>,
) -> bool {
    let md_sub = format!("{tid}.md");
    if touched_subpaths.contains(&md_sub) {
        return true;
    }
    if pinned.task.note_archives.is_empty() {
        return false;
    }
    let dir_sub = format!("files/{tid}");
    if touched_subpaths.contains(&dir_sub) {
        return true;
    }
    pinned.task.note_archives.iter().any(|arc| {
        let arc_sub = format!("files/{tid}/{}", arc.file_name());
        touched_subpaths.contains(&arc_sub)
    })
}

fn check_staged_and_working_tree_task(
    project: &Project,
    live_p: &Project,
    reader: &mut BatchReader,
    checked_head: &str,
    tasks_rel: &str,
    tid: &TaskId,
    suffixes: &BTreeSet<String>,
) -> bool {
    // (a) Checked HEAD state
    if check_task_at_commit(
        project,
        live_p,
        reader,
        checked_head,
        tasks_rel,
        tid,
        suffixes,
    ) {
        return true;
    }

    // (b) Staged Git index (`ls-files --stage -z -- <tasks_rel>/<tid>.md <tasks_rel>/files/<tid>`)
    let md_rel = format!("{tasks_rel}/{tid}.md");
    let files_rel = format!("{tasks_rel}/files/{tid}");
    let stage_bytes = match basis::git(
        live_p,
        &["ls-files", "--stage", "-z", "--", &md_rel, &files_rel],
    ) {
        Ok(b) => b,
        Err(_) => return true,
    };

    let mut stage0_md: Option<(String, String)> = None;
    let mut stage0_files: BTreeMap<String, (String, String)> = BTreeMap::new();
    let files_prefix = format!("{files_rel}/");

    for raw in stage_bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let Ok(line) = std::str::from_utf8(raw) else {
            return true;
        };
        let Some((meta, path)) = line.split_once('\t') else {
            return true;
        };
        let mut parts = meta.split_whitespace();
        let (Some(mode), Some(oid), Some(stage), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return true;
        };
        if stage != "0" {
            return true;
        }
        if path == md_rel {
            stage0_md = Some((mode.to_string(), oid.to_string()));
        } else if let Some(rest) = path.strip_prefix(&files_prefix) {
            stage0_files.insert(rest.to_string(), (mode.to_string(), oid.to_string()));
        } else {
            return true;
        }
    }

    if let Some((mode, oid)) = stage0_md {
        if mode != "100644" && mode != "100755" {
            return true;
        }
        let Ok(raw_bytes) = reader.read_blob(&oid, None, &md_rel) else {
            return true;
        };
        let Ok(raw_md) = std::str::from_utf8(&raw_bytes) else {
            return true;
        };
        let Ok(physical) = crate::format::parse_physical(raw_md, &md_rel) else {
            return true;
        };
        let logical = crate::history::load_logical(physical, |archive| {
            let name = archive.file_name();
            let arc_path = format!("{files_rel}/{name}");
            let Some((amode, aoid)) = stage0_files.get(&name) else {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Missing staged archive :0:{arc_path}"
                )));
            };
            if amode != "100644" && amode != "100755" {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "Unsafe staged archive :0:{arc_path}"
                )));
            }
            reader.read_blob(aoid, Some(project.attachments_max_bytes), &arc_path)
        });
        match logical {
            Ok(task) if &task.id == tid => {
                if notes_mention_suffixes(&task.notes, suffixes) {
                    return true;
                }
            }
            Ok(_) | Err(_) => return true,
        }
    } else {
        for (name, (mode, oid)) in &stage0_files {
            if name.starts_with("notes-") && name.ends_with(".notes") {
                if mode != "100644" && mode != "100755" {
                    return true;
                }
                let arc_path = format!("{files_rel}/{name}");
                let Ok(bytes) =
                    reader.read_blob(oid, Some(project.attachments_max_bytes), &arc_path)
                else {
                    return true;
                };
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    return true;
                };
                if text.contains("retained:") {
                    return true;
                }
            }
        }
    }

    // (c) Working tree on disk
    let wt_md = project.records.join(format!("tasks/{tid}.md"));
    match std::fs::symlink_metadata(&wt_md) {
        Ok(meta) => {
            if !meta.is_file() {
                return true;
            }
            let Ok(raw_bytes) = std::fs::read(&wt_md) else {
                return true;
            };
            crate::history::record_metadata_read(raw_bytes.len());
            let Ok(raw_md) = std::str::from_utf8(&raw_bytes) else {
                return true;
            };
            let file_label = format!("tasks/{tid}.md");
            let Ok(physical) = crate::format::parse_physical(raw_md, &file_label) else {
                return true;
            };
            let logical = crate::history::load_logical(physical, |archive| {
                crate::history::read_archive_at(
                    &project.records,
                    tid,
                    archive,
                    project.attachments_max_bytes,
                )
            });
            match logical {
                Ok(task) if &task.id == tid => {
                    if notes_mention_suffixes(&task.notes, suffixes) {
                        return true;
                    }
                }
                Ok(_) | Err(_) => return true,
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let wt_files_dir = project.records.join(format!("tasks/files/{tid}"));
            match std::fs::symlink_metadata(&wt_files_dir) {
                Ok(meta) => {
                    if !meta.is_dir() {
                        return true;
                    }
                    let Ok(entries) = std::fs::read_dir(&wt_files_dir) else {
                        return true;
                    };
                    for entry in entries {
                        let Ok(entry) = entry else {
                            return true;
                        };
                        let fname = entry.file_name();
                        let Some(name) = fname.to_str() else {
                            return true;
                        };
                        if name.starts_with("notes-") && name.ends_with(".notes") {
                            let path = entry.path();
                            let Ok(fmeta) = std::fs::symlink_metadata(&path) else {
                                return true;
                            };
                            if !fmeta.is_file() || fmeta.len() > project.attachments_max_bytes {
                                return true;
                            }
                            let Ok(bytes) = std::fs::read(&path) else {
                                return true;
                            };
                            crate::history::record_metadata_read(bytes.len());
                            let Ok(text) = std::str::from_utf8(&bytes) else {
                                return true;
                            };
                            if text.contains("retained:") {
                                return true;
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return true,
            }
        }
        Err(_) => return true,
    }

    false
}

#[allow(clippy::too_many_arguments)]
fn scan_project_status_paths(
    registry: &Registry,
    focal_id: &TaskId,
    focal_pinned: &PinnedTask,
    tp: &TrackedProject,
    live_p: &Project,
    checked_head: &str,
    raw_paths: &[&str],
    reader: &mut Option<BatchReader>,
    suffixes: &BTreeSet<String>,
    items: &mut Vec<IncomingItem>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> Result<()> {
    let mut touched_by_tid: BTreeMap<TaskId, BTreeSet<String>> = BTreeMap::new();
    for full_path in raw_paths {
        match classify_rel_path(&tp.tasks_rel, full_path, true) {
            ClassifiedRelPath::Ignore => {}
            ClassifiedRelPath::TaskMd(tid) => {
                touched_by_tid
                    .entry(tid.clone())
                    .or_default()
                    .insert(format!("{tid}.md"));
            }
            ClassifiedRelPath::TaskFilesDir(tid) => {
                touched_by_tid
                    .entry(tid.clone())
                    .or_default()
                    .insert(format!("files/{tid}"));
            }
            ClassifiedRelPath::TaskAttachment { tid, rest } => {
                touched_by_tid
                    .entry(tid.clone())
                    .or_default()
                    .insert(format!("files/{tid}/{rest}"));
            }
            ClassifiedRelPath::Invalid(detail) => {
                diagnostics.push(scan_failed_diag(&tp.project.prefix, None, detail));
            }
        }
    }

    for (tid, subpaths) in touched_by_tid {
        if tp.is_focal_project && &tid == focal_id {
            if touches_required_task_inputs(focal_id, focal_pinned, &subpaths) {
                return Err(stale_focal_error(
                    "Task or required history has unpublished changes; publish them before querying current retained state, or explicitly select a historical --at basis",
                ));
            }
            continue;
        }
        if let Some(entry) = tp.participants.get(&tid) {
            if touches_required_task_inputs(&tid, &entry.pinned, &subpaths) {
                drop_holder_items(items, &entry.canonical_id);
                diagnostics.push(stale_diag(
                    &tp.project.prefix,
                    Some(entry.canonical_id.clone()),
                    format!(
                        "Participant {} has unpublished changes in its task record or note archives",
                        entry.canonical_id
                    ),
                ));
            }
            continue;
        }

        if reader.is_none() {
            match BatchReader::spawn(live_p) {
                Ok(r) => *reader = Some(r),
                Err(err) => {
                    if tp.is_focal_project {
                        return Err(err);
                    }
                    drop_project_items(items, tp);
                    diagnostics.push(scan_failed_diag(&tp.project.prefix, None, err.to_string()));
                    return Ok(());
                }
            }
        }
        let batch = reader.as_mut().expect("batch reader spawned");
        let tid_canon = registry.canonical_id(&tid).to_string();
        if check_staged_and_working_tree_task(
            &tp.project,
            live_p,
            batch,
            checked_head,
            &tp.tasks_rel,
            &tid,
            suffixes,
        ) {
            drop_holder_items(items, &tid_canon);
            diagnostics.push(stale_diag(
                &tp.project.prefix,
                Some(tid_canon.clone()),
                format!(
                    "Task {tid_canon} has unpublished index or working-tree changes introducing matching or unverified retained history"
                ),
            ));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_grouped_live_statuses(
    registry: &Registry,
    focal_id: &TaskId,
    focal_pinned: &PinnedTask,
    tracked_projects: &[TrackedProject],
    states: &[ProjectRecheckState],
    suffixes: &BTreeSet<String>,
    items: &mut Vec<IncomingItem>,
    diagnostics: &mut Vec<IncomingDiagnostic>,
) -> Result<()> {
    let mut by_root: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (idx, tp) in tracked_projects.iter().enumerate() {
        if states[idx].active && tp.in_scan_scope {
            by_root.entry(tp.git_root.clone()).or_default().push(idx);
        }
    }

    for indices in by_root.into_values() {
        let rep_tp = &tracked_projects[indices[0]];
        let rep_live = read::live(&rep_tp.project);
        let mut args = vec![
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored",
            "--",
        ];
        for &idx in &indices {
            args.push(&tracked_projects[idx].tasks_rel);
        }
        let status_bytes = match basis::git(&rep_live, &args) {
            Ok(b) => b,
            Err(err) => {
                if indices
                    .iter()
                    .any(|&i| tracked_projects[i].is_focal_project)
                {
                    return Err(err);
                }
                for &idx in &indices {
                    let tp = &tracked_projects[idx];
                    drop_project_items(items, tp);
                    diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: tp.project.prefix.clone(),
                        holder: None,
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: STALE_ACTION.into(),
                    });
                }
                continue;
            }
        };
        if status_bytes.is_empty() {
            continue;
        }

        let raw_paths = parse_porcelain_z_paths(&status_bytes, &rep_tp.project.prefix, diagnostics);
        let mut routed: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        for full_path in raw_paths {
            let trimmed = full_path.trim_end_matches('/');
            let matched = indices
                .iter()
                .copied()
                .filter(|&idx| {
                    let rel = &tracked_projects[idx].tasks_rel;
                    trimmed == rel
                        || full_path
                            .strip_prefix(rel.as_str())
                            .is_some_and(|rest| rest.starts_with('/'))
                })
                .max_by_key(|&idx| tracked_projects[idx].tasks_rel.len());
            if let Some(idx) = matched {
                routed.entry(idx).or_default().push(full_path);
            }
        }

        let mut reader: Option<BatchReader> = None;
        for &idx in &indices {
            let Some(proj_paths) = routed.get(&idx) else {
                continue;
            };
            let tp = &tracked_projects[idx];
            let live_p = read::live(&tp.project);
            scan_project_status_paths(
                registry,
                focal_id,
                focal_pinned,
                tp,
                &live_p,
                &states[idx].checked_head,
                proj_paths,
                &mut reader,
                suffixes,
                items,
                diagnostics,
            )?;
        }
    }

    Ok(())
}

pub(crate) fn recheck_freshness(
    ctx: &Ctx,
    focal_id: &TaskId,
    focal_pinned: &PinnedTask,
    mut prepared: PreparedIncoming,
) -> Result<IncomingCustody> {
    let mut states: Vec<ProjectRecheckState> = prepared
        .tracked_projects
        .iter()
        .map(|tp| ProjectRecheckState {
            active: true,
            checked_head: tp.basis_commit.clone(),
            checked_edges: BTreeSet::new(),
        })
        .collect();

    let mut converged = false;

    for _iter in 0..4 {
        // 1. Pre-scan `(ref_1, head_1)`, participant range checks, and `diff-tree` edge scan.
        let mut pre_heads: BTreeMap<PathBuf, (Option<String>, String)> = BTreeMap::new();
        for (idx, tp) in prepared.tracked_projects.iter().enumerate() {
            if !states[idx].active {
                continue;
            }
            let live_p = read::live(&tp.project);
            let head_1 = match verify_ref_and_ancestry_cached(
                tp,
                &live_p,
                &states[idx].checked_head,
                &mut pre_heads,
            ) {
                Ok(h) => h,
                Err(err) => {
                    if tp.is_focal_project {
                        return Err(err);
                    }
                    drop_project_items(&mut prepared.custody.items, tp);
                    prepared.custody.diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: tp.project.prefix.clone(),
                        holder: None,
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: STALE_ACTION.into(),
                    });
                    states[idx].active = false;
                    continue;
                }
            };

            if head_1 != states[idx].checked_head {
                if tp.is_focal_project {
                    freshness::current(&ctx.project, focal_id, focal_pinned, false)?;
                }
                for (raw_id, entry) in &tp.participants {
                    if let Err(err) = freshness::current(&tp.project, raw_id, &entry.pinned, false)
                    {
                        drop_holder_items(&mut prepared.custody.items, &entry.canonical_id);
                        prepared.custody.diagnostics.push(IncomingDiagnostic {
                            code: err.kind().into(),
                            project: tp.project.prefix.clone(),
                            holder: Some(entry.canonical_id.clone()),
                            host: None,
                            identity: None,
                            source: None,
                            detail: err.to_string(),
                            action: STALE_ACTION.into(),
                        });
                    }
                }
                if tp.in_scan_scope {
                    scan_commit_edges(
                        ctx,
                        focal_id,
                        tp,
                        &live_p,
                        &head_1,
                        &mut states[idx].checked_edges,
                        &prepared.suffixes,
                        &mut prepared.custody.items,
                        &mut prepared.custody.diagnostics,
                    )?;
                }
                states[idx].checked_head = head_1;
            }
        }

        // 2. Test hook, live participant freshness, and live Git index/working-tree scan.
        #[cfg(test)]
        DURING_INCOMING_RECHECK.with(|hook| {
            if let Some(action) = hook.borrow_mut().take() {
                action();
            }
        });

        freshness::current(&ctx.project, focal_id, focal_pinned, false)?;

        for (idx, tp) in prepared.tracked_projects.iter().enumerate() {
            if !states[idx].active {
                continue;
            }
            for (raw_id, entry) in &tp.participants {
                if let Err(err) = freshness::current(&tp.project, raw_id, &entry.pinned, false) {
                    drop_holder_items(&mut prepared.custody.items, &entry.canonical_id);
                    prepared.custody.diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: tp.project.prefix.clone(),
                        holder: Some(entry.canonical_id.clone()),
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: STALE_ACTION.into(),
                    });
                }
            }
        }

        scan_grouped_live_statuses(
            &ctx.registry,
            focal_id,
            focal_pinned,
            &prepared.tracked_projects,
            &states,
            &prepared.suffixes,
            &mut prepared.custody.items,
            &mut prepared.custody.diagnostics,
        )?;

        // 3 & 4. Post-scan `(ref_2, head_2)` verification and joint convergence check.
        let mut all_stationary = true;
        let mut post_heads: BTreeMap<PathBuf, (Option<String>, String)> = BTreeMap::new();
        for (idx, tp) in prepared.tracked_projects.iter().enumerate() {
            if !states[idx].active {
                continue;
            }
            let live_p = read::live(&tp.project);
            match verify_ref_and_ancestry_cached(
                tp,
                &live_p,
                &states[idx].checked_head,
                &mut post_heads,
            ) {
                Ok(head_2) => {
                    if head_2 != states[idx].checked_head {
                        all_stationary = false;
                    }
                }
                Err(err) => {
                    if tp.is_focal_project {
                        return Err(err);
                    }
                    drop_project_items(&mut prepared.custody.items, tp);
                    prepared.custody.diagnostics.push(IncomingDiagnostic {
                        code: err.kind().into(),
                        project: tp.project.prefix.clone(),
                        holder: None,
                        host: None,
                        identity: None,
                        source: None,
                        detail: err.to_string(),
                        action: STALE_ACTION.into(),
                    });
                    states[idx].active = false;
                }
            }
        }

        if all_stationary {
            converged = true;
            break;
        }
    }

    if !converged {
        let mut final_heads: BTreeMap<PathBuf, (Option<String>, String)> = BTreeMap::new();
        for (idx, tp) in prepared.tracked_projects.iter().enumerate() {
            if !states[idx].active {
                continue;
            }
            let live_p = read::live(&tp.project);
            let head_2 = verify_ref_and_ancestry_cached(
                tp,
                &live_p,
                &states[idx].checked_head,
                &mut final_heads,
            )?;
            if head_2 != states[idx].checked_head {
                if tp.is_focal_project {
                    return Err(stale_focal_error(
                        "Task project HEAD advanced repeatedly during live freshness recheck; retry tasks retained state",
                    ));
                }
                drop_project_items(&mut prepared.custody.items, tp);
                prepared.custody.diagnostics.push(stale_diag(
                    &tp.project.prefix,
                    None,
                    "Project records HEAD advanced repeatedly during live freshness recheck; retry tasks retained state",
                ));
            }
        }
    }

    prepared
        .custody
        .items
        .sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    prepared
        .custody
        .diagnostics
        .sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    prepared.custody.diagnostics.dedup();
    prepared.custody.complete = Some(prepared.custody.diagnostics.is_empty());
    Ok(prepared.custody)
}
