//! The staged snapshot: one project's records tree as written from the index Git will
//! publish, beside the HEAD commit and HEAD's tree for the same directory.
//!
//! Every read here names an object id, so the snapshot cannot change while a check runs.
//! The index is the one Git hands the records repository's pre-commit hook
//! (`GIT_INDEX_FILE`, which a pathspec commit points at a temporary index), or the
//! repository's own index otherwise. A code checkout's hook also exports
//! `GIT_INDEX_FILE`, naming the code commit's index, which says nothing about records
//! kept in another repository or worktree.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// The environment variable Git sets for hooks to name the index being committed.
pub(crate) const INDEX_ENV: &str = "GIT_INDEX_FILE";

/// How Git, under `LC_ALL=C`, says that a directory is in no repository.
pub(crate) const NOT_A_REPOSITORY: &str = "not a git repository";

/// How `git write-tree --prefix` says that the index holds nothing there.
pub(crate) const PREFIX_NOT_STAGED: &str = "git-write-tree: prefix ";

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

/// What an inherited `GIT_INDEX_FILE` names.
pub(crate) enum Inherited {
    /// Nothing: it is unset.
    None,
    /// An index of the repository and worktree holding the records: the commit being made
    /// publishes it.
    Records(PathBuf),
    /// Anything else, as given: in a code checkout's hook, the code commit's index.
    Foreign(std::ffi::OsString),
}

/// Classifies the inherited `GIT_INDEX_FILE`, resolved against this process's working
/// directory as Git resolves it for the hook.
///
/// An exported `GIT_DIR` names the repository Git uses the index with. A main checkout's
/// hook exports none, but Git keeps an index, and the temporary ones it commits from, in
/// the worktree's own Git directory, so that directory names the owner wherever the hook
/// has gone since. An index anywhere else was named by hand and belongs to the repository
/// the working directory selects.
pub(crate) fn inherited_index(records: &Path) -> Result<Inherited> {
    let Some(value) = std::env::var_os(INDEX_ENV) else {
        return Ok(Inherited::None);
    };
    let index = std::env::current_dir()?.join(&value);
    if !index.is_file() {
        return Ok(Inherited::Foreign(value));
    }
    let dir = index
        .parent()
        .ok_or_else(|| failed(INDEX_ENV, format!("{} has no directory", index.display())))?
        .canonicalize()?;
    let owner = if std::env::var_os("GIT_DIR").is_none() && is_git_dir(&dir)? {
        Some(dir)
    } else {
        selected_git_dir()?
    };
    let own = git_dir(line(
        run(records, None, &["rev-parse", "--absolute-git-dir"])?,
        "Git directory",
    )?)?;
    Ok(if owner.as_deref() == Some(own.as_path()) {
        Inherited::Records(index)
    } else {
        Inherited::Foreign(value)
    })
}

fn git_dir(path: String) -> Result<PathBuf> {
    Ok(PathBuf::from(path).canonicalize()?)
}

/// Whether `dir` is itself a Git directory, a linked worktree's included.
fn is_git_dir(dir: &Path) -> Result<bool> {
    Ok(crate::git::at(dir)
        .args(["rev-parse", "--resolve-git-dir", "."])
        .output()?
        .status
        .success())
}

/// The Git directory the inherited environment and working directory select, if any.
fn selected_git_dir() -> Result<Option<PathBuf>> {
    let out = std::process::Command::new("git")
        .env("LC_ALL", "C")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()?;
    if out.status.success() {
        return git_dir(line(out.stdout, "Git directory")?).map(Some);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains(NOT_A_REPOSITORY) {
        return Ok(None);
    }
    Err(failed("git rev-parse", stderr.trim()))
}

/// The index a staged check reads: the hook's, else the repository's own. An inherited
/// index of another repository is refused: `--staged` asks about the commit being made,
/// and that commit publishes no records of this project.
pub(crate) fn staged_index(records: &Path) -> Result<PathBuf> {
    match inherited_index(records)? {
        Inherited::Records(index) => Ok(index),
        Inherited::None => repository_index(records),
        Inherited::Foreign(value) => Err(failed(
            INDEX_ENV,
            format!(
                "{value:?} names no index of the records repository at {}; run plain \
                 `tasks check` from another repository's hook, or unset {INDEX_ENV}",
                records.display()
            ),
        )),
    }
}

/// The own index of the repository holding `records`.
pub(crate) fn repository_index(records: &Path) -> Result<PathBuf> {
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

/// Reads the staged tree of `records` from `index`, and HEAD's. `write-tree` locks and
/// rewrites the index it reads, so it gets a private copy: a check neither waits on nor
/// disturbs another session's commit in the same repository. Git replaces an index by
/// rename, so the copy is one whole version.
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
    let scratch = super::scratch::Scratch::create()?;
    let copy = scratch.path().join("index");
    std::fs::copy(index, &copy)
        .map_err(|error| failed("index", format!("{}: {error}", index.display())))?;
    let tree = line(run(&top, Some(&copy), &write)?, "staged tree")?;
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
