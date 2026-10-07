//! Linked checkouts: a path's git facts and the registered project whose `[checkout]`
//! names its repository (ops docs/specs/2026-10-01-linked-checkouts-design.md §4.1).

use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

/// git's view of a directory, canonical: the repository's common git directory.
#[derive(Debug, Clone, PartialEq)]
pub struct GitTree {
    pub common: PathBuf,
}

impl GitTree {
    /// `None` when git says `dir` is not inside a repository, or there is no git
    /// executable; any other git failure is an `io` error. `LC_ALL=C` pins the one
    /// message matched, as `Project::git_toplevel` does. One line per requested fact, in
    /// the order of the flags.
    pub fn of(dir: &Path) -> Result<Option<GitTree>> {
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
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
                return Ok(None);
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
        Ok(Some(GitTree { common: next()? }))
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
/// are `checkout_conflict`.
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
        [one] => crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)
            .map(Some),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_checkout_is_the_parent_of_a_dot_git_common_dir_only() {
        let tree = |common: &str| GitTree {
            common: common.into(),
        };
        assert_eq!(tree("/r/.git").main_checkout(), Some(PathBuf::from("/r")));
        assert_eq!(
            tree("/srv/r.git").main_checkout(),
            None,
            "a bare repository has none"
        );
    }

    #[test]
    fn outside_a_repository_there_is_no_tree() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitTree::of(dir.path()).unwrap(), None);
    }
}
