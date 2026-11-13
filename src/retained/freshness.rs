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

pub(super) fn unchanged(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
) -> Result<()> {
    validate_range(project, id, from, to, task, None)
}

pub(super) fn phase_unchanged(
    project: &Project,
    id: &TaskId,
    from: &Basis,
    to: &Basis,
    task: &Task,
    permitted: fn(&Task, &Task) -> bool,
) -> Result<()> {
    validate_range(project, id, from, to, task, Some(permitted))
}

fn compare_edge(
    project: &Project,
    id: &TaskId,
    context: &Basis,
    from: &str,
    to: &str,
    paths: &[String],
    permitted: Option<fn(&Task, &Task) -> bool>,
) -> Result<()> {
    match equal_trees(project, from, to, paths) {
        Ok(()) => return Ok(()),
        Err(Error::RetainedStaleBasis(_)) if permitted.is_some() => {}
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
    if !permitted.expect("progress policy")(&old, &new) {
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
    permitted: Option<fn(&Task, &Task) -> bool>,
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
    let mut fresh = project.clone();
    fresh.retention_reads = None;
    if !super::read::ancestor(&fresh, &from.commit, &to.commit)? {
        return Err(stale());
    }
    basis::validate_locator(&fresh, id, from)?;
    basis::validate_locator(&fresh, id, to)?;
    let paths = paths(id, from, task);
    compare_edge(
        &fresh,
        id,
        from,
        &from.commit,
        &to.commit,
        &paths,
        permitted,
    )?;
    // No path-limited traversal: it can hide reverted side-branch and merge-parent edits.
    let range = format!("{}..{}", from.commit, to.commit);
    let commits = basis::git(&fresh, &["rev-list", "--parents", &range])?;
    let commits = std::str::from_utf8(&commits).map_err(super::model::invalid)?;
    for line in commits.lines() {
        let mut oids = line.split_whitespace();
        let commit = oids
            .next()
            .ok_or_else(|| super::model::invalid("empty commit range entry"))?;
        for parent in oids {
            compare_edge(&fresh, id, from, parent, commit, &paths, permitted)?;
        }
    }
    Ok(())
}

fn working_file(project: &Project, commit: &str, path: &str, cap: Option<u64>) -> Result<()> {
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
    let expected = basis::blob(project, commit, path, cap, &mut bytes_read)?;
    let actual = super::verify::read_local(&local, cap.unwrap_or(u64::MAX))?;
    if actual != expected {
        return Err(stale());
    }
    Ok(())
}

fn working_attachments(project: &Project, id: &TaskId, now: &Basis) -> Result<()> {
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
                if !meta.is_file() || !expected.contains(&relative) {
                    return Err(stale());
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
    Ok(fresh)
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
