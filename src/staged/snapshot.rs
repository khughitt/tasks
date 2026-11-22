//! The staged snapshot: one project's records tree as written from the index Git will
//! publish, beside the HEAD commit and HEAD's tree for the same directory.
//!
//! Every read here names an object id, so the snapshot cannot change while a check runs.
//! The index is the one Git hands a pre-commit hook (`GIT_INDEX_FILE`, which a pathspec
//! commit points at a temporary index), or the repository's own index outside a hook.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// The environment variable Git sets for hooks to name the index being committed.
pub(crate) const INDEX_ENV: &str = "GIT_INDEX_FILE";

#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    /// The repository's top level.
    pub(crate) top: PathBuf,
    /// The repository's common Git directory, shared by its worktrees.
    pub(crate) common: PathBuf,
    /// The records directory relative to `top`: `.` when they are the same.
    pub(crate) subdir: String,
    /// The staged tree of the records directory.
    pub(crate) tree: String,
    /// HEAD, absent before the first commit.
    pub(crate) head: Option<Head>,
}

#[derive(Debug, Clone)]
pub(crate) struct Head {
    pub(crate) commit: String,
    /// HEAD's tree of the records directory, absent when HEAD holds none.
    pub(crate) tree: Option<String>,
}

/// One path that differs between two trees, from `git diff-tree --raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) status: char,
    pub(crate) old_mode: String,
    pub(crate) new_mode: String,
    pub(crate) old_oid: String,
    pub(crate) new_oid: String,
    /// Relative to the records directory.
    pub(crate) path: String,
}

fn failed(what: &str, detail: impl std::fmt::Display) -> Error {
    Error::StagedSnapshot(format!("cannot read the staged records: {what}: {detail}"))
}

fn run(top: &Path, index: Option<&Path>, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = crate::git::at(top);
    if let Some(index) = index {
        command.env(INDEX_ENV, index);
    }
    let out = command.args(args).output()?;
    if !out.status.success() {
        return Err(failed(
            &format!("git {}", args.first().copied().unwrap_or("")),
            String::from_utf8_lossy(&out.stderr).trim(),
        ));
    }
    Ok(out.stdout)
}

fn line(bytes: Vec<u8>, what: &str) -> Result<String> {
    let text = String::from_utf8(bytes).map_err(|_| failed(what, "output is not UTF-8"))?;
    let text = text.strip_suffix('\n').unwrap_or(&text);
    if text.is_empty() || text.contains('\n') {
        return Err(failed(what, format!("unexpected output {text:?}")));
    }
    Ok(text.into())
}

/// The index to read: the hook's `GIT_INDEX_FILE` (relative to this process's working
/// directory, as Git resolves it for the hook), else the repository's own index.
pub(crate) fn index_file(records: &Path, inherited: Option<std::ffi::OsString>) -> Result<PathBuf> {
    if let Some(value) = inherited {
        if value.is_empty() {
            return Err(failed(INDEX_ENV, "is set but empty"));
        }
        let path = PathBuf::from(value);
        return Ok(if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        });
    }
    let path = line(
        run(
            records,
            None,
            &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        )?,
        "index path",
    )?;
    Ok(PathBuf::from(path))
}

/// Reads the staged tree of `records` from `index`, and HEAD's.
pub(crate) fn read(records: &Path, index: &Path) -> Result<Snapshot> {
    if !index.is_file() {
        return Err(failed(
            "index",
            format!("{} is not a file", index.display()),
        ));
    }
    let top = PathBuf::from(line(
        run(records, None, &["rev-parse", "--show-toplevel"])?,
        "repository top level",
    )?)
    .canonicalize()?;
    let relative = records.strip_prefix(&top).map_err(|_| {
        failed(
            "records directory",
            format!("{} is outside {}", records.display(), top.display()),
        )
    })?;
    let subdir = match relative.to_str() {
        Some("") => ".".to_string(),
        Some(sub) => sub.to_string(),
        None => return Err(failed("records directory", "path is not UTF-8")),
    };
    let common = PathBuf::from(line(
        run(
            &top,
            None,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?,
        "common Git directory",
    )?)
    .canonicalize()?;
    let mut write = vec!["write-tree".to_string()];
    if subdir != "." {
        write.push(format!("--prefix={subdir}/"));
    }
    let write: Vec<&str> = write.iter().map(String::as_str).collect();
    let tree = line(run(&top, Some(index), &write)?, "staged tree")?;
    let head = match run(
        &top,
        None,
        &["rev-parse", "--verify", "-q", "HEAD^{commit}"],
    ) {
        Ok(out) => Some(line(out, "HEAD")?),
        Err(_) => {
            // `rev-parse --verify -q` fails quietly only when HEAD names no commit yet.
            let unborn = crate::git::at(&top)
                .args(["symbolic-ref", "-q", "HEAD"])
                .output()?
                .status
                .success();
            if !unborn {
                return Err(failed(
                    "HEAD",
                    "names neither a commit nor an unborn branch",
                ));
            }
            None
        }
    };
    let head = match head {
        None => None,
        Some(commit) => {
            let tree = if subdir == "." {
                Some(line(
                    run(
                        &top,
                        None,
                        &["rev-parse", "--verify", &format!("{commit}^{{tree}}")],
                    )?,
                    "HEAD tree",
                )?)
            } else {
                tree_entry(&top, &commit, &subdir)?
                    .filter(|entry| entry.mode == "040000")
                    .map(|entry| entry.oid)
            };
            Some(Head { commit, tree })
        }
    };
    Ok(Snapshot {
        top,
        common,
        subdir,
        tree,
        head,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) mode: String,
    pub(crate) oid: String,
}

/// The entry at `path` in `treeish`, if any.
pub(crate) fn tree_entry(top: &Path, treeish: &str, path: &str) -> Result<Option<Entry>> {
    let out = run(
        top,
        None,
        &["ls-tree", "-z", "--full-tree", treeish, "--", path],
    )?;
    let Some(record) = out.split(|b| *b == 0).find(|r| !r.is_empty()) else {
        return Ok(None);
    };
    let record =
        std::str::from_utf8(record).map_err(|_| failed("ls-tree", "entry is not UTF-8"))?;
    let (meta, name) = record
        .split_once('\t')
        .ok_or_else(|| failed("ls-tree", format!("malformed entry {record:?}")))?;
    let fields: Vec<&str> = meta.split(' ').collect();
    if fields.len() != 3 || name != path {
        return Err(failed("ls-tree", format!("unexpected entry {record:?}")));
    }
    Ok(Some(Entry {
        mode: fields[0].into(),
        oid: fields[2].into(),
    }))
}

/// Every path that differs between the trees `from` and `to`, recursively, without
/// rename detection.
pub(crate) fn changes(top: &Path, from: &str, to: &str) -> Result<Vec<Change>> {
    let out = run(
        top,
        None,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--raw",
            "--no-renames",
            "--no-abbrev",
            from,
            to,
        ],
    )?;
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    let mut changes = Vec::new();
    while let Some(meta) = fields.next() {
        let meta =
            std::str::from_utf8(meta).map_err(|_| failed("diff-tree", "entry is not UTF-8"))?;
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        let path = fields
            .next()
            .ok_or_else(|| failed("diff-tree", format!("entry {meta:?} has no path")))?;
        let path =
            std::str::from_utf8(path).map_err(|_| failed("diff-tree", "path is not UTF-8"))?;
        let [old_mode, new_mode, old_oid, new_oid, status] = parts[..] else {
            return Err(failed("diff-tree", format!("malformed entry {meta:?}")));
        };
        let mut status_chars = status.chars();
        let (Some(status), None) = (status_chars.next(), status_chars.next()) else {
            return Err(failed(
                "diff-tree",
                format!("unexpected status in {meta:?}"),
            ));
        };
        changes.push(Change {
            status,
            old_mode: old_mode.into(),
            new_mode: new_mode.into(),
            old_oid: old_oid.into(),
            new_oid: new_oid.into(),
            path: path.into(),
        });
    }
    Ok(changes)
}

/// The bytes of blob `oid`.
pub(crate) fn blob(top: &Path, oid: &str) -> Result<Vec<u8>> {
    run(top, None, &["cat-file", "blob", oid])
}

/// Whether commit `older` is `newer` or one of its ancestors.
pub(crate) fn is_ancestor(top: &Path, older: &str, newer: &str) -> Result<bool> {
    let out = crate::git::at(top)
        .args(["merge-base", "--is-ancestor", older, newer])
        .output()?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failed(
            "git merge-base",
            String::from_utf8_lossy(&out.stderr).trim(),
        )),
    }
}
