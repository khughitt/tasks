//! Atomic publication for attach. The record rename commits the transaction. A
//! host-local intent identifies its two exact record states and the staged files.
use super::{DirState, EntryState, dir_state, entry_state};
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::{Project, atomic_write};
use crate::state_store::digest;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

const LINK_PROBE: &str = ".link-probe";

#[derive(Serialize, Deserialize)]
struct Journal {
    task: String,
    nonce: u64,
    before: String,
    after: String,
    files: Vec<(String, String)>,
    created_task_dir: bool,
}

fn journal_path(project: &Project) -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(
        state
            .join("tasks/attach")
            .join(digest(project.records.as_os_str().as_encoded_bytes())),
    )
}

// Independent of the project mutation lock, which the opener may already own.
// Only raw file operations run under this lock, and none opens a Project.
fn lock(project: &Project) -> Result<File> {
    let dir = File::open(project.tasks_dir())?;
    dir.lock().map_err(|e| Error::Io(e.to_string()))?;
    Ok(dir)
}

fn stop(boundary: &str) {
    if cfg!(debug_assertions) && std::env::var("TASKS_ATTACH_STOP_AFTER").as_deref() == Ok(boundary)
    {
        std::process::exit(86);
    }
}

impl Journal {
    fn stage(&self, project: &Project, id: &TaskId) -> PathBuf {
        super::task_dir(project, id).join(format!(".attach-{:016x}", self.nonce))
    }

    fn finish(&self, project: &Project, id: &TaskId, path: &Path) -> Result<()> {
        if id.prefix != project.prefix {
            return Err(Error::Io(
                "attach journal prefix differs from project".into(),
            ));
        }
        super::check_storage(project, id)?;
        let current = digest(&super::eligibility::read_regular(&project.task_path(id))?);
        let committed = current == self.after;
        if !committed && current != self.before {
            return Err(Error::Io(format!(
                "cannot recover attach for {id}: record differs from both journal revisions"
            )));
        }
        let stage = self.stage(project, id);
        if let DirState::Unsafe(detail) = dir_state(&stage)? {
            return Err(Error::AttachmentUnsafe(detail));
        }
        if committed {
            super::ensure_task_dir(project, id)?;
        }
        for (index, (name, expected)) in self.files.iter().enumerate() {
            super::validate_name(name)?;
            let src = stage.join(name);
            if committed {
                let dest = super::task_dir(project, id).join(name);
                match entry_state(&dest)? {
                    EntryState::Absent => {
                        if !matches!(entry_state(&src)?, EntryState::File(_))
                            || digest(&super::eligibility::read_regular(&src)?) != *expected
                        {
                            return Err(Error::Io(format!(
                                "staged bytes for {id}/{name} missing or changed"
                            )));
                        }
                        fs::hard_link(&src, &dest)?;
                    }
                    EntryState::File(_)
                        if digest(&super::eligibility::read_regular(&dest)?) == *expected => {}
                    _ => {
                        return Err(Error::AttachmentUnsafe(format!(
                            "cannot recover {id}/{name}: destination changed"
                        )));
                    }
                }
                stop(&format!("published:{}", index + 1));
            }
            match fs::remove_file(&src) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        if !committed {
            match fs::remove_file(stage.join(LINK_PROBE)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        match fs::remove_dir(stage) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if !committed && self.created_task_dir {
            match fs::remove_dir(super::task_dir(project, id)) {
                Ok(()) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(e) => return Err(e.into()),
            }
        }
        fs::remove_file(path)?;
        Ok(())
    }
}

// Keep the snapshot lock only while reading the record and its files. Providers
// and Git hooks may themselves call tasks and must run outside the lock.
pub(crate) fn read_guard(project: &Project, requested: impl Fn(&TaskId) -> bool) -> Result<File> {
    let dir = File::open(project.tasks_dir())?;
    dir.lock_shared()?;
    if let Some(path) = journal_path(project)
        && path.try_exists()?
    {
        dir.unlock()?;
        dir.lock()?;
        recover_locked(project, &path, requested)?;
    }
    Ok(dir)
}

pub(crate) fn recover(project: &Project) -> Result<()> {
    if let Some(path) = journal_path(project)
        && path.try_exists()?
    {
        let _guard = lock(project)?;
        recover_locked(project, &path, |_| true)?;
    }
    Ok(())
}

fn recover_locked(
    project: &Project,
    path: &Path,
    requested: impl Fn(&TaskId) -> bool,
) -> Result<()> {
    match fs::read(path) {
        Ok(bytes) => {
            let journal: Journal = serde_json::from_slice(&bytes)
                .map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
            let id = TaskId::parse(&journal.task)?;
            if requested(&id) {
                journal.finish(project, &id, path)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn publish(
    project: &Project,
    id: &TaskId,
    record: &Path,
    raw: &[u8],
    files: &[(String, Vec<u8>)],
) -> Result<()> {
    let _guard = lock(project)?;
    if files.is_empty() {
        return atomic_write(record, raw);
    }
    let path = journal_path(project)
        .ok_or_else(|| Error::Config("neither XDG_STATE_HOME nor HOME is set".into()))?;
    if path.try_exists()? {
        return Err(Error::Io("attach recovery is pending".into()));
    }
    let journal = Journal {
        task: id.to_string(),
        nonce: fastrand::u64(..),
        before: digest(&super::eligibility::read_regular(record)?),
        after: digest(raw),
        created_task_dir: !super::check_storage(project, id)?,
        files: files
            .iter()
            .map(|(name, bytes)| (name.clone(), digest(bytes)))
            .collect(),
    };
    let stage = journal.stage(project, id);
    if dir_state(&stage)? != DirState::Absent {
        return Err(Error::AttachmentExists(format!(
            "staging directory already exists at {}",
            stage.display()
        )));
    }
    fs::create_dir_all(path.parent().expect("journal directory"))?;
    atomic_write(
        &path,
        &serde_json::to_vec(&journal).expect("journal serializes"),
    )?;
    let result = (|| -> Result<()> {
        super::ensure_task_dir(project, id)?;
        fs::DirBuilder::new().mode(0o700).create(&stage)?;
        for (index, (name, bytes)) in files.iter().enumerate() {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(stage.join(name))?;
            file.write_all(bytes)?;
            file.sync_all()?;
            stop(&format!("staged:{}", index + 1));
        }
        // Recovery must be able to hard-link staged files before the record can claim them.
        let probe = stage.join(LINK_PROBE);
        fs::hard_link(stage.join(&files[0].0), &probe)?;
        stop("probed");
        fs::remove_file(probe)?;
        atomic_write(record, raw)?;
        stop("saved");
        Ok(())
    })();
    journal.finish(project, id, &path)?;
    result
}
