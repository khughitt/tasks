//! A private directory under the temporary directory, removed when dropped.

use crate::error::{Error, Result};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn create() -> Result<Scratch> {
        loop {
            let candidate = std::env::temp_dir().join(format!(
                "tasks-staged-{}-{:016x}",
                std::process::id(),
                fastrand::u64(..)
            ));
            // Private to this user: the copy holds the project's records.
            match std::fs::DirBuilder::new().mode(0o700).create(&candidate) {
                Ok(()) => return Ok(Scratch(candidate)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(Error::StagedSnapshot(format!(
                        "cannot create a private directory {}: {error}",
                        candidate.display()
                    )));
                }
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "tasks: cleanup needs attention: could not remove {}: {error}",
                self.0.display()
            );
        }
    }
}
