//! A private, read-only copy of one staged records tree, for the full staged audit.
//!
//! The copy is written with `git checkout-index` from a temporary index holding exactly
//! the staged tree, so file contents, modes and symlinks are what Git will publish. The
//! directory is removed when the copy is dropped.

use super::scratch::Scratch;
use super::snapshot::INDEX_ENV;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub(crate) struct Content {
    _scratch: Scratch,
    root: PathBuf,
}

fn failed(detail: impl std::fmt::Display) -> Error {
    Error::StagedSnapshot(format!("cannot materialize the staged records: {detail}"))
}

impl Content {
    /// Writes `tree` from the repository at `top` into a fresh private directory.
    pub(crate) fn materialize(top: &Path, tree: &str) -> Result<Content> {
        let scratch = Scratch::create()?;
        let index = scratch.path().join("index");
        let content = Content {
            root: scratch.path().join("records"),
            _scratch: scratch,
        };
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
