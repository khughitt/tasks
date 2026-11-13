//! Exact participant freshness across committed records changes.
use super::{basis, model::Basis};
use crate::{
    error::{Error, Result},
    model::{Task, TaskId},
    repo::Project,
};

fn stale() -> Error {
    Error::RetainedStaleBasis(
        "Retained participant changed since its pinned basis; recover or refresh its exact inputs"
            .into(),
    )
}

fn paths(id: &TaskId, basis: &Basis, task: &Task) -> Vec<String> {
    let files = format!("{}/tasks/files/{id}", basis.records_subdir)
        .trim_start_matches("./")
        .to_string();
    std::iter::once(basis.task_path.clone())
        .chain(std::iter::once(files.clone()))
        .chain(
            task.note_archives
                .iter()
                .map(|a| format!("{files}/{}", a.file_name())),
        )
        .collect()
}

fn equal_trees(project: &Project, from: &str, to: &str, paths: &[String]) -> Result<()> {
    let mut args = vec![
        "diff",
        "--quiet",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        from,
        to,
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let out = super::read::run(project, &args)?.output;
    match out.status.code() {
        Some(0) => Ok(()),
        Some(1) => Err(stale()),
        _ => Err(Error::RetainedHistoryBlocked(format!(
            "Retained tree comparison failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) enum ProgressPolicy {
    Exact,
    PendingNotes,
}

/// A successful immutable range proof, charged to the invocation's shared budget.
pub(super) struct WarmRange {
    key: Vec<u8>,
    head: String,
    _permit: super::read::MemoPermit,
}

fn commuting_progress(old: &Task, new: &Task) -> bool {
    let mut allowed = old.clone();
    allowed.updated = new.updated.clone();
    allowed.owner = new.owner.clone();
    allowed.complexity = new.complexity;
    allowed.notes = new.notes.clone();
    allowed == *new
        && new.notes.len() > old.notes.len()
        && new.notes.starts_with(&old.notes)
        && !new.notes[old.notes.len()..]
            .iter()
            .any(|note| super::publish::reserved(&note.text))
}

pub(super) fn unchanged(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
) -> Result<()> {
    check(project, id, from, to, task, ProgressPolicy::Exact, None)
}

pub(super) fn phase_unchanged(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
) -> Result<()> {
    check(
        project,
        id,
        from,
        to,
        task,
        ProgressPolicy::PendingNotes,
        None,
    )
}

fn range_key(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
    policy: ProgressPolicy,
) -> Result<Vec<u8>> {
    // Repository discovery is deliberately uncached, like the live locator checks.
    let repository = basis::git(
        project,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
        ],
    )?;
    let mut context = to.clone();
    context.commit.clear();
    serde_json::to_vec(&(
        repository,
        id,
        from,
        context,
        task,
        paths(id, from, task),
        policy,
        &project.records,
        &project.checkout,
        &project.retention_lineage,
        project.attachments_max_bytes,
    ))
    .map_err(super::model::invalid)
}

pub(super) fn warm(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
    policy: ProgressPolicy,
) -> Result<Option<WarmRange>> {
    let fresh = live_context(project)?;
    validate_range(&fresh, id, from, to, task, policy, None)?;
    let key = range_key(&fresh, id, from, to, task, policy)?;
    let Some(charge) = key.len().checked_add(to.commit.len()) else {
        return Ok(None);
    };
    Ok(
        super::read::reserve_memo(&fresh, charge)?.map(|permit| WarmRange {
            key,
            head: to.commit.clone(),
            _permit: permit,
        }),
    )
}

fn live_context(project: &Project) -> Result<Project> {
    let live = super::read::live(project);
    super::read::refresh(&live)?;
    context(&live)
}

pub(super) fn check(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
    policy: ProgressPolicy,
    warm: Option<&WarmRange>,
) -> Result<()> {
    let fresh = live_context(project)?;
    let matched = match warm {
        Some(proof)
            if proof.key == range_key(&fresh, id, from, to, task, policy)?
                && super::read::ancestor(&fresh, &proof.head, &to.commit)? =>
        {
            Some(proof.head.as_str())
        }
        _ => None,
    };
    validate_range(&fresh, id, from, to, task, policy, matched)
}

fn compare_edge(
    project: &Project,
    id: &TaskId,
    context: &Basis,
    from: &str,
    to: &str,
    paths: &[String],
    policy: ProgressPolicy,
) -> Result<()> {
    match equal_trees(project, from, to, paths) {
        Ok(()) => return Ok(()),
        Err(Error::RetainedStaleBasis(_)) if policy == ProgressPolicy::PendingNotes => {}
        Err(error) => return Err(error),
    }
    equal_trees(project, from, to, &paths[1..])?;
    let mut before = context.clone();
    before.commit = from.into();
    let mut after = context.clone();
    after.commit = to.into();
    let mut bytes = 0;
    let old = basis::task_at(project, id, &before, &mut bytes)?;
    let new = basis::task_at(project, id, &after, &mut bytes)?;
    if !commuting_progress(&old, &new) {
        return Err(stale());
    }
    let old_entry = basis::git(project, &["ls-tree", "-z", from, "--", &context.task_path])?;
    let new_entry = basis::git(project, &["ls-tree", "-z", to, "--", &context.task_path])?;
    if old_entry.get(..7) != new_entry.get(..7) {
        return Err(stale());
    }
    let physical = crate::history::physical_for_save(&new, |archive| {
        let path = format!(
            "{}/tasks/files/{}/{}",
            context.records_subdir,
            new.id,
            archive.file_name()
        );
        basis::blob(
            project,
            to,
            path.trim_start_matches("./"),
            Some(project.attachments_max_bytes),
            &mut bytes,
        )
    })?;
    // Parsed equality alone would hide byte-only edits; accept only writer output.
    let raw = basis::blob(project, to, &context.task_path, None, &mut bytes)?;
    if raw != crate::format::serialize_physical(&physical).as_bytes() {
        return Err(stale());
    }
    Ok(())
}

fn validate_range(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
    policy: ProgressPolicy,
    warmed_head: Option<&str>,
) -> Result<()> {
    from.validate()?;
    to.validate()?;
    if from.project != to.project
        || from.layout != to.layout
        || from.records_subdir != to.records_subdir
        || from.task_path != to.task_path
        || from.retained_ref != to.retained_ref
    {
        return Err(stale());
    }
    // Refs and pairing are live observations, even when a caller owns a read session.
    let fresh = super::read::live(project);
    let branch = basis::git(&fresh, &["symbolic-ref", "-q", "HEAD"])?;
    if branch.strip_suffix(b"\n").unwrap_or(&branch) != to.retained_ref.as_bytes() {
        return Err(stale());
    }
    if !super::read::ancestor(&fresh, &from.commit, &to.commit)? {
        return Err(stale());
    }
    basis::validate_locator(&fresh, id, from)?;
    basis::validate_locator(&fresh, id, to)?;
    let paths = paths(id, from, task);
    compare_edge(&fresh, id, from, &from.commit, &to.commit, &paths, policy)?;
    // No path-limited traversal: it can hide reverted side-branch and merge-parent edits.
    let range = format!("{}..{}", from.commit, to.commit);
    let commits = match warmed_head {
        Some(head) => basis::git(
            &fresh,
            &["rev-list", "--parents", &to.commit, "--not", head],
        )?,
        None => basis::git(&fresh, &["rev-list", "--parents", &range])?,
    };
    let commits = std::str::from_utf8(&commits).map_err(super::model::invalid)?;
    for line in commits.lines() {
        let mut oids = line.split_whitespace();
        let commit = oids
            .next()
            .ok_or_else(|| super::model::invalid("empty commit range entry"))?;
        for parent in oids {
            compare_edge(&fresh, id, from, parent, commit, &paths, policy)?;
        }
    }
    Ok(())
}

fn working_file(project: &Project, commit: &str, path: &str, cap: Option<u64>) -> Result<()> {
    working_file_expected(project, commit, path, cap, None)
}
fn working_file_expected(
    project: &Project,
    commit: &str,
    path: &str,
    cap: Option<u64>,
    expected: Option<&[u8]>,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let root = basis::root(project)?;
    let local = root.join(path);
    let meta = std::fs::symlink_metadata(&local)?;
    if !meta.is_file() {
        return Err(stale());
    }
    let entry = basis::git(project, &["ls-tree", "-z", commit, "--", path])?;
    let mode = if meta.permissions().mode() & 0o100 == 0 {
        b"100644 blob ".as_slice()
    } else {
        b"100755 blob ".as_slice()
    };
    if !entry.starts_with(mode) {
        return Err(stale());
    }
    let mut bytes_read = 0;
    let committed;
    let expected = match expected {
        Some(bytes) => bytes,
        None => {
            committed = basis::blob(project, commit, path, cap, &mut bytes_read)?;
            &committed
        }
    };
    let actual = super::verify::read_local(&local, cap.unwrap_or(u64::MAX))?;
    if actual != expected {
        return Err(stale());
    }
    Ok(())
}

fn working_attachments(project: &Project, id: &TaskId, now: &Basis) -> Result<()> {
    working_attachments_allowed(project, id, now, &std::collections::BTreeMap::new())
}
fn working_attachments_allowed(
    project: &Project,
    id: &TaskId,
    now: &Basis,
    allowed: &std::collections::BTreeMap<String, &[u8]>,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let present = crate::attachments::check_storage(project, id)?;
    let root = basis::root(project)?;
    let directory = crate::attachments::task_dir(project, id);
    let relative = directory
        .strip_prefix(&root)
        .map_err(super::model::invalid)?
        .to_str()
        .ok_or_else(|| super::model::invalid("attachment directory must be UTF-8"))?;
    let raw = basis::git(
        project,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--full-tree",
            &now.commit,
            "--",
            relative,
        ],
    )?;
    let mut expected = std::collections::BTreeSet::new();
    for entry in raw.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).map_err(super::model::invalid)?;
        let (_, path) = entry
            .split_once('\t')
            .ok_or_else(|| super::model::invalid("invalid attachment tree entry"))?;
        if !super::model::member_path(path) {
            return Err(stale());
        }
        expected.insert(path.to_string());
    }
    let task_directory = directory.clone();
    let mut pending = if present { vec![directory] } else { Vec::new() };
    let mut found = std::collections::BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let path = entry?.path();
            let meta = std::fs::symlink_metadata(&path)?;
            let relative = path
                .strip_prefix(&root)
                .map_err(super::model::invalid)?
                .to_str()
                .ok_or_else(|| super::model::invalid("attachment path must be UTF-8"))?
                .to_string();
            if meta.is_dir() {
                let prefix = format!("{relative}/");
                if !expected.iter().any(|p| p.starts_with(&prefix)) {
                    return Err(stale());
                }
                pending.push(path);
            } else {
                if !meta.is_file() {
                    return Err(stale());
                }
                if !expected.contains(&relative) {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or_else(stale)?;
                    let bytes = allowed
                        .get(name)
                        .filter(|_| path.parent() == Some(task_directory.as_path()))
                        .ok_or_else(stale)?;
                    if meta.permissions().mode() & 0o100 != 0
                        || super::verify::read_local(&path, project.attachments_max_bytes)?
                            != *bytes
                    {
                        return Err(stale());
                    }
                    continue;
                }
                working_file(
                    project,
                    &now.commit,
                    &relative,
                    Some(project.attachments_max_bytes),
                )?;
                found.insert(relative);
            }
        }
    }
    if found != expected {
        return Err(stale());
    }
    Ok(())
}

/// Exact physical/index preimage, with only the caller's preserved staged task and new artifacts allowed.
pub(super) fn working_preimage(
    project: &Project,
    id: &TaskId,
    pinned: &basis::PinnedTask,
    staged_raw: Option<&str>,
    allowed: &std::collections::BTreeMap<String, &[u8]>,
) -> Result<()> {
    let paths = paths(id, &pinned.basis, &pinned.task);
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        "--cached",
        "--name-only",
        "HEAD",
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    if !basis::git(project, &args)?.is_empty() {
        return Err(stale());
    }
    working_file_expected(
        project,
        &pinned.basis.commit,
        &pinned.basis.task_path,
        None,
        staged_raw.map(str::as_bytes),
    )?;
    for archive in &pinned.task.note_archives {
        let path = format!(
            "{}/tasks/files/{id}/{}",
            pinned.basis.records_subdir,
            archive.file_name()
        );
        working_file(
            project,
            &pinned.basis.commit,
            path.trim_start_matches("./"),
            None,
        )?;
    }
    working_attachments_allowed(project, id, &pinned.basis, allowed)
}

pub(super) fn context(project: &Project) -> Result<Project> {
    let fresh = Project::locate(&project.records)?;
    if fresh.records != project.records
        || fresh.prefix != project.prefix
        || fresh.kind != project.kind
        || fresh.checkout != project.checkout
        || fresh.retention_lineage != project.retention_lineage
        || fresh.commit_records != project.commit_records
        || fresh.attachments_max_bytes != project.attachments_max_bytes
    {
        return Err(stale());
    }
    super::read::inherit(&super::read::live(project), fresh)
}

pub(crate) fn current(
    project: &Project,
    id: &TaskId,
    pinned: &basis::PinnedTask,
    attachments: bool,
) -> Result<Basis> {
    let fresh = context(project)?;
    let branch = basis::git(&fresh, &["symbolic-ref", "-q", "HEAD"])?;
    if branch.strip_suffix(b"\n").unwrap_or(&branch) != pinned.basis.retained_ref.as_bytes() {
        return Err(stale());
    }
    let mut now = pinned.basis.clone();
    now.commit = basis::resolve(&fresh, "HEAD")?;
    unchanged(&fresh, id, &pinned.basis, &now, &pinned.task)?;
    basis::read_exact(&fresh, id, &now)?;
    let mut paths = paths(id, &now, &pinned.task);
    if !attachments {
        paths.remove(1);
    }
    let mut args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignored",
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    if !basis::git(&fresh, &args)?.is_empty() {
        return Err(Error::RetainedStaleBasis("Task or required history has unpublished changes; publish them before querying current retained state, or explicitly select a historical --at basis".into()));
    }
    for path in paths.iter().filter(|p| {
        **p != format!("{}/tasks/files/{id}", now.records_subdir).trim_start_matches("./")
    }) {
        working_file(&fresh, &now.commit, path, None)?;
    }
    if attachments {
        working_attachments(&fresh, id, &now)?;
    }
    Ok(now)
}
