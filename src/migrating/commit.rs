//! G9's check (docs/project-operations.md): every path the run owns is clean in its repository, for the
//! records directories that opt in to record commits.

use crate::error::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The owned paths (absolute, keyed by records directory) that are not clean in git, for
/// records directories whose config sets `[git] commit`; the others contribute nothing, as
/// does one outside git (whose commit reports `commit_unavailable`).
pub fn uncommitted(owned: &BTreeMap<PathBuf, BTreeSet<PathBuf>>) -> Result<Vec<PathBuf>> {
    let mut left = Vec::new();
    for (records, paths) in owned {
        if paths.is_empty() || !crate::repo::Project::open(records)?.commit_records {
            continue;
        }
        let Some(top) = crate::record_commit::working_tree(records)
            .map_err(|error| Error::Io(format!("{}: {error}", records.display())))?
        else {
            continue;
        };
        let canonical = records.canonicalize()?;
        let mut relative = Vec::new();
        for path in paths {
            let inside = path.strip_prefix(records).map_err(|_| {
                Error::Io(format!(
                    "{} is not under {}",
                    path.display(),
                    records.display()
                ))
            })?;
            let at_top = canonical.join(inside);
            let rel = at_top.strip_prefix(&top).map_err(|_| {
                Error::Io(format!(
                    "{} is not under its working tree {}",
                    at_top.display(),
                    top.display()
                ))
            })?;
            relative.push((path, rel.to_path_buf()));
        }
        let dirty = status(&top, relative.iter().map(|(_, rel)| rel.as_path()))?;
        for (path, rel) in relative {
            if dirty.iter().any(|entry| entry.starts_with(&rel)) {
                left.push(path.clone());
            }
        }
    }
    Ok(left)
}

/// Every path `git status` reports under `paths` (relative to the top level `top`).
fn status<'a>(top: &Path, paths: impl Iterator<Item = &'a Path>) -> Result<Vec<PathBuf>> {
    let output = crate::git::at(top)
        .args([
            "-c",
            "core.quotePath=false",
            "--literal-pathspecs",
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=all",
            "--",
        ])
        .args(paths)
        .output()?;
    if !output.status.success() {
        return Err(Error::Io(format!(
            "git status in {} failed: {}",
            top.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split('\0').filter(|field| !field.is_empty());
    let mut dirty = Vec::new();
    while let Some(record) = fields.next() {
        let (code, path) = record.split_at(3.min(record.len()));
        dirty.push(PathBuf::from(path));
        if code.starts_with(['R', 'C'])
            && let Some(original) = fields.next()
        {
            dirty.push(PathBuf::from(original));
        }
    }
    Ok(dirty)
}
