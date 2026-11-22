//! A private, read-only copy of one staged records tree, for the full staged audit.
//!
//! The copy is written with `git checkout-index` from a temporary index holding exactly
//! the staged tree, so file contents, modes and symlinks are what Git will publish. The
//! directory is removed when the copy is dropped.

use super::snapshot::INDEX_ENV;
use crate::error::{Error, Result};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

pub(crate) struct Content {
    scratch: PathBuf,
    root: PathBuf,
}

fn failed(detail: impl std::fmt::Display) -> Error {
    Error::StagedSnapshot(format!("cannot materialize the staged records: {detail}"))
}

impl Content {
    /// Writes `tree` from the repository at `top` into a fresh private directory.
    pub(crate) fn materialize(top: &Path, tree: &str) -> Result<Content> {
        let scratch = loop {
            let candidate = std::env::temp_dir().join(format!(
                "tasks-staged-{}-{:016x}",
                std::process::id(),
                fastrand::u64(..)
            ));
            // Private to this user: the copy holds the project's records.
            match std::fs::DirBuilder::new().mode(0o700).create(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(failed(format!("{}: {error}", candidate.display()))),
            }
        };
        let content = Content {
            root: scratch.join("records"),
            scratch,
        };
        let index = content.scratch.join("index");
        let git = |args: &[&str]| -> Result<()> {
            let out = crate::git::at(top)
                .env(INDEX_ENV, &index)
                .args(args)
                .output()?;
            if out.status.success() {
                Ok(())
            } else {
                Err(failed(format!(
                    "git {}: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr).trim()
                )))
            }
        };
        git(&["read-tree", tree])?;
        let prefix = format!(
            "--prefix={}/",
            content
                .root
                .to_str()
                .ok_or_else(|| failed("temporary directory path is not UTF-8"))?
        );
        git(&["checkout-index", "-a", "-q", &prefix])?;
        if !content.root.is_dir() {
            // An empty tree writes nothing; the records directory still exists.
            std::fs::create_dir(&content.root)?;
        }
        Ok(content)
    }

    /// The staged records directory.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for Content {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.scratch) {
            eprintln!(
                "tasks: cleanup needs attention: could not remove {}: {error}",
                self.scratch.display()
            );
        }
    }
}
