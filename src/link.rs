//! Linked checkouts: a path's git facts and the registered project whose `[checkout]`
//! names its repository (ops docs/specs/2026-10-01-linked-checkouts-design.md §4.1).

use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

/// git's view of a directory, canonical: the repository's common git directory.
#[derive(Debug, Clone, PartialEq)]
pub struct GitTree {
    pub top: PathBuf,
    pub common: PathBuf,
    pub git_dir: PathBuf,
}

impl GitTree {
    /// `None` when git says `dir` is not inside a repository and no ancestor has a
    /// `.git` marker, or there is no git executable; broken metadata is an `io` error.
    /// `LC_ALL=C` pins the message matched, as `Project::git_toplevel` does. One line per requested fact, in
    /// the order of the flags.
    pub fn of(dir: &Path) -> Result<Option<GitTree>> {
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-common-dir",
                "--absolute-git-dir",
            ])
            .env("LC_ALL", "C")
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                let mut marker = false;
                for ancestor in dir.canonicalize()?.ancestors() {
                    match ancestor.join(".git").symlink_metadata() {
                        Ok(_) => {
                            marker = true;
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                if !marker {
                    return Ok(None);
                }
            }
            return Err(Error::Io(format!(
                "git rev-parse in {} failed ({}): {}",
                dir.display(),
                output.status,
                stderr.trim()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines().map(|line| PathBuf::from(line).canonicalize());
        let mut next = || -> Result<PathBuf> {
            lines
                .next()
                .ok_or_else(|| {
                    Error::Io(format!(
                        "git rev-parse in {} printed too little",
                        dir.display()
                    ))
                })?
                .map_err(Error::from)
        };
        Ok(Some(GitTree {
            top: next()?,
            common: next()?,
            git_dir: next()?,
        }))
    }

    /// This working tree is the main checkout, not a linked worktree.
    pub fn is_main(&self) -> bool {
        self.git_dir == self.common
    }

    /// Every working tree of the repository, canonical where it still exists, main first.
    pub fn worktrees(&self) -> Result<Vec<PathBuf>> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.top)
            .args(["worktree", "list", "--porcelain", "-z"])
            .env("LC_ALL", "C")
            .output()?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git worktree list in {} failed: {}",
                self.top.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter_map(|field| field.strip_prefix("worktree "))
            .map(|listed| {
                let listed = PathBuf::from(listed);
                listed.canonicalize().unwrap_or(listed)
            })
            .collect())
    }

    /// The repository's main checkout: the common directory's parent when that is a
    /// `.git` directory. `None` for a bare repository or a separate git dir.
    pub fn main_checkout(&self) -> Option<PathBuf> {
        if self.common.file_name()? != ".git" {
            return None;
        }
        self.common.parent().map(Path::to_path_buf)
    }
}

/// The registered project whose `[checkout]` is `tree`'s main checkout. Two such projects
/// are `checkout_conflict`. Opened with `worktree` set to `tree`'s top level.
pub fn linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    let Some(main) = tree.main_checkout() else {
        return Ok(None);
    };
    let matches: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.checkout == main)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [one] => {
            let mut project =
                crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)?;
            project.worktree = Some(tree.top.clone());
            Ok(Some(project))
        }
        many => Err(Error::CheckoutConflict(format!(
            "{} is declared as the checkout of {}; keep one project's [checkout]",
            main.display(),
            many.iter()
                .map(|l| l.prefix.as_str())
                .collect::<Vec<_>>()
                .join(" and ")
        ))),
    }
}

/// Linked-checkouts decision 5 and §8.1: the repository holding `tree` keeps our records
/// when a registered project's root is in it (the same common git directory), or its top
/// level holds a `tasks/.config.toml` tracked in `HEAD` as a regular file. Only roots
/// inside one of the repository's working trees can share its common directory, so git
/// runs only for those.
pub fn holds_records(registry: &Registry, tree: &GitTree) -> Result<bool> {
    let worktrees = tree.worktrees()?;
    for root in registry.projects.values() {
        if !root.try_exists()? {
            continue;
        }
        let root = root.canonicalize()?;
        if !worktrees.iter().any(|worktree| root.starts_with(worktree)) {
            continue;
        }
        if GitTree::of(&root)?.is_some_and(|theirs| theirs.common == tree.common) {
            return Ok(true);
        }
    }
    tracked_config(&tree.top)
}

/// `tasks/.config.toml` is tracked in `HEAD` at `top` as a regular file: `ls-tree` names a
/// `blob` of mode `100644` or `100755`. A tree at that path (which `git cat-file -e`
/// accepts) or a symlink (`120000`) is not a config. A genuinely unborn branch tracks
/// nothing; every other failure to read `HEAD` (a malformed loose ref, a corrupt
/// `packed-refs`) is an `io` error, never "no records" (plan review round 2).
fn tracked_config(top: &Path) -> Result<bool> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(top)
            .args(args)
            .env("LC_ALL", "C")
            .output()
    };
    let listed = git(&["ls-tree", "-z", "HEAD", "--", CONFIG_PATH])?;
    if !listed.status.success() {
        if unborn(top)? {
            return Ok(false);
        }
        return Err(Error::Io(format!(
            "git ls-tree HEAD in {} failed: {}",
            top.display(),
            String::from_utf8_lossy(&listed.stderr).trim()
        )));
    }
    // `<mode> SP <type> SP <object> TAB <path> NUL`, or nothing when the path is untracked.
    let entry = String::from_utf8_lossy(&listed.stdout);
    let Some((meta, path)) = entry.trim_end_matches('\0').split_once('\t') else {
        return Ok(false);
    };
    let mut fields = meta.split(' ');
    Ok(path == CONFIG_PATH
        && matches!(
            (fields.next(), fields.next()),
            (Some("100644" | "100755"), Some("blob"))
        ))
}

const CONFIG_PATH: &str = crate::repo::CONFIG_REL;

/// `HEAD` names a branch that does not exist yet, and nothing is wrong: `symbolic-ref`
/// resolves `HEAD` to a ref name, and `for-each-ref` on that name lists nothing and says
/// nothing. A malformed loose ref fails `symbolic-ref` ("No such ref: HEAD"), and a
/// corrupt `packed-refs` fails both, so neither reads as unborn (both reproduced in
/// scratch repositories while planning).
fn unborn(top: &Path) -> Result<bool> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(top)
            .args(args)
            .env("LC_ALL", "C")
            .output()
    };
    let head = git(&["symbolic-ref", "-q", "HEAD"])?;
    if !head.status.success() {
        return Ok(false);
    }
    let name = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let listed = git(&["for-each-ref", "--format=%(refname)", &name])?;
    Ok(listed.status.success() && listed.stdout.is_empty() && listed.stderr.is_empty())
}

/// `to` relative to `from`, both absolute and canonical: the shared prefix dropped, one
/// `..` per remaining component of `from`, then the rest of `to`.
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in shared..from.len() {
        out.push("..");
    }
    for component in &to[shared..] {
        out.push(component.as_os_str());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_checkout_is_the_parent_of_a_dot_git_common_dir_only() {
        let tree = |common: &str| GitTree {
            top: "/r".into(),
            common: common.into(),
            git_dir: common.into(),
        };
        assert_eq!(tree("/r/.git").main_checkout(), Some(PathBuf::from("/r")));
        assert_eq!(
            tree("/srv/r.git").main_checkout(),
            None,
            "a bare repository has none"
        );
    }

    #[test]
    fn relative_path_climbs_then_descends() {
        assert_eq!(
            relative_path(Path::new("/d/o/nexc"), Path::new("/d/code")),
            PathBuf::from("../../code")
        );
        assert_eq!(
            relative_path(Path::new("/d/o"), Path::new("/d/o/x")),
            PathBuf::from("x")
        );
    }

    #[test]
    fn outside_a_repository_there_is_no_tree() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitTree::of(dir.path()).unwrap(), None);
    }
}
