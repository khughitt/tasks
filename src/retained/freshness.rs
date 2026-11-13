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
    equal_trees(&fresh, &from.commit, &to.commit, &paths)?;
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
            equal_trees(&fresh, parent, commit, &paths)?;
        }
    }
    Ok(())
}
