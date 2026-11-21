//! The v1 supervised-start handoff uses inherited descriptor 3 on Linux.
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub task: String,
    pub session: String,
    pub record: PathBuf,
    pub before_sha256: String,
    dev: String,
    ino: String,
}

impl std::str::FromStr for Binding {
    type Err = serde_json::Error;
    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(text)
    }
}

impl Binding {
    /// Entry owns the inherited descriptor until process exit. Never unlock the
    /// shared open file description, including on an error.
    pub fn confine(&self, id: &str) -> Result<()> {
        let flags = unsafe { libc::fcntl(3, libc::F_GETFD) };
        if flags < 0 || unsafe { libc::fcntl(3, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            return Err(Error::ClaimFenceRefused(
                "claim fence requires inherited descriptor 3".into(),
            ));
        }
        let meta = std::fs::metadata("/proc/self/fd/3")?;
        let info = std::fs::read_to_string("/proc/self/fdinfo/3")?;
        let exclusive = info.lines().any(|line| {
            matches!(
                line.split_ascii_whitespace().collect::<Vec<_>>().as_slice(),
                ["lock:", _, "FLOCK", "ADVISORY", "WRITE", _, _, "0", "EOF"]
            )
        });
        if !exclusive
            || !meta.is_file()
            || meta.dev().to_string() != self.dev
            || meta.ino().to_string() != self.ino
        {
            return Err(Error::ClaimFenceRefused(
                "claim fence requires the specified exclusively locked file".into(),
            ));
        }
        if self.task != id
            || self.session.is_empty()
            || std::env::var("TASKS_SESSION").ok().as_ref() != Some(&self.session)
        {
            return Err(Error::ClaimFenceRefused(
                "claim fence task or session mismatch".into(),
            ));
        }
        Ok(())
    }

    pub fn check_source(&self, id: &str, record: &Path, raw: &str) -> Result<()> {
        if self.task != id
            || self.record != record.canonicalize()?
            || self.before_sha256 != crate::history::sha256(raw.as_bytes())
        {
            return Err(Error::ClaimFenceRefused(
                "claim fence source or preimage mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Publication {
    Committed { binding: Binding, commit: String },
    Unknown { binding: Binding },
}

/// Compare raw Git blob contents with the deterministic serialization of the
/// task that save just published. Git clean filters can change the staged blob.
pub fn matches_authored_record(
    result: &crate::record_commit::Committed,
    binding: &Binding,
    project: &crate::repo::Project,
    task: &crate::model::Task,
) -> bool {
    let Ok(path) = binding.record.strip_prefix(&result.toplevel) else {
        return false;
    };
    let Ok(physical) = project.physical_for_save(task) else {
        return false;
    };
    let Some(path) = path.to_str() else {
        return false;
    };
    crate::git::at(&result.toplevel)
        .args([
            "--no-replace-objects",
            "cat-file",
            "blob",
            &format!("{}:{path}", result.commit),
        ])
        .output()
        .is_ok_and(|out| {
            out.status.success()
                && out.stdout == crate::format::serialize_physical(&physical).as_bytes()
        })
}
