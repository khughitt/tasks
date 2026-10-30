use super::{
    DirState, EntryState, Ledger, Problem, Severity, dir_state, entry_state, ledger, validate_name,
};
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::AttachmentEligibilityOut;
use crate::repo::Project;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct EligibilityRow {
    pub path: String,
    pub eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    pub problems: Vec<Problem>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Snapshot {
    path: PathBuf,
    metadata: Metadata,
}

fn same(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn changed(path: &Path) -> Error {
    Error::Validation(format!(
        "{} changed during attachment eligibility query",
        path.display()
    ))
}

impl Snapshot {
    fn take(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            metadata: std::fs::symlink_metadata(path)?,
        })
    }

    fn verify(&self) -> Result<()> {
        let current = std::fs::symlink_metadata(&self.path).map_err(|_| changed(&self.path))?;
        if !same(&self.metadata, &current) {
            return Err(changed(&self.path));
        }
        Ok(())
    }

    fn read(&self, cap: Option<u64>) -> Result<Vec<u8>> {
        if !self.metadata.is_file() {
            return Err(Error::AttachmentUnsafe(format!(
                "{} is not a regular file",
                self.path.display()
            )));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.path)?;
        if !same(&self.metadata, &file.metadata()?) {
            return Err(changed(&self.path));
        }
        let bytes = match cap {
            Some(cap) => super::read_capped(&mut file, cap, &self.path.display().to_string())?,
            None => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                bytes
            }
        };
        if !same(&self.metadata, &file.metadata()?) {
            return Err(changed(&self.path));
        }
        self.verify()?;
        Ok(bytes)
    }
}

pub(crate) fn local_query(dir: &Path, paths: &[String]) -> Result<AttachmentEligibilityOut> {
    let records = dir.canonicalize()?;
    let tasks = Snapshot::take(&records.join("tasks"))?;
    if !tasks.metadata.is_dir() {
        return Err(Error::AttachmentUnsafe(
            "tasks is not a real directory".into(),
        ));
    }
    let config = Snapshot::take(&records.join("tasks/.config.toml"))?;
    let bytes = config.read(None)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| Error::Config(e.to_string()))?;
    let project = Project::from_config(records.clone(), text)?;
    let attachments = super::eligibility(&project, paths)?;
    config.verify()?;
    tasks.verify()?;
    Ok(AttachmentEligibilityOut {
        version: 1,
        records,
        prefix: project.prefix,
        config_sha256: digest(&bytes),
        attachments,
    })
}

/// Judge only the requested local owners and entries, never a registry or sibling.
pub fn eligibility(project: &Project, paths: &[String]) -> Result<Vec<EligibilityRow>> {
    if paths.is_empty() || paths.len() > 128 {
        return Err(Error::Validation(
            "give 1–128 unique attachment paths".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut requests = Vec::new();
    for path in paths {
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() != 4
            || parts[0] != "tasks"
            || parts[1] != "files"
            || parts[3].is_empty()
            || [".", ".."].contains(&parts[3])
            || !seen.insert(path)
        {
            return Err(Error::Validation(format!(
                "not a unique exact attachment path: {path}"
            )));
        }
        let id = TaskId::parse(parts[2]).map_err(|e| Error::Validation(e.to_string()))?;
        if id.prefix != project.prefix {
            return Err(Error::Validation(format!(
                "{id} does not belong to project {}",
                project.prefix
            )));
        }
        requests.push((path, id, parts[3]));
    }
    let mut snapshots = Vec::new();
    let mut owners = BTreeMap::new();
    let mut rows = Vec::new();
    for (path, id, name) in requests {
        let mut row = EligibilityRow {
            path: path.clone(),
            eligible: false,
            record_sha256: None,
            file_sha256: None,
            problems: Vec::new(),
        };
        let mut deny = |kind, severity, detail| {
            row.problems.push(Problem {
                severity,
                kind,
                id: Some(id.clone()),
                file: path.clone(),
                detail,
            })
        };
        if let Err(error) = validate_name(name) {
            deny("attachment_invalid", Severity::Warning, error.to_string());
            rows.push(row);
            continue;
        }
        let mut unsafe_storage = None;
        for dir in [
            project.tasks_dir(),
            super::files_root(project),
            super::task_dir(project, &id),
        ] {
            match dir_state(&dir)? {
                DirState::Directory => snapshots.push(Snapshot::take(&dir)?),
                DirState::Absent => break,
                DirState::Unsafe(detail) => {
                    unsafe_storage = Some(detail);
                    break;
                }
            }
        }
        if let Some(detail) = unsafe_storage {
            deny("attachment_unsafe", Severity::Error, detail);
            rows.push(row);
            continue;
        }
        let record = project.task_path(&id);
        if let std::collections::btree_map::Entry::Vacant(entry) = owners.entry(id.to_string()) {
            let owner = match entry_state(&record)? {
                EntryState::Absent => None,
                EntryState::Unsafe(detail) => {
                    deny("attachment_unsafe", Severity::Error, detail);
                    rows.push(row);
                    continue;
                }
                EntryState::File(_) => {
                    let snapshot = Snapshot::take(&record)?;
                    let raw = snapshot.read(None)?;
                    let file = format!("tasks/{id}.md");
                    let text = std::str::from_utf8(&raw).map_err(|e| Error::Parse {
                        file: file.clone(),
                        detail: e.to_string(),
                    })?;
                    let task = crate::format::parse_task(text, &file)?;
                    project.validate_docs(&task).map_err(|e| Error::Parse {
                        file: file.clone(),
                        detail: e.to_string(),
                    })?;
                    if task.id != id {
                        return Err(Error::Parse {
                            file,
                            detail: format!("id field is {}", task.id),
                        });
                    }
                    let result = (ledger(&task), digest(&raw));
                    snapshots.push(snapshot);
                    Some(result)
                }
            };
            entry.insert(owner);
        }
        let Some((ledger, record_hash)) = &owners[&id.to_string()] else {
            deny(
                "attachment_orphan",
                Severity::Error,
                "owner record is absent".into(),
            );
            rows.push(row);
            continue;
        };
        let entry = project.records.join(path);
        match entry_state(&entry)? {
            EntryState::Absent => deny(
                "attachment_missing",
                Severity::Error,
                "attachment file is absent".into(),
            ),
            EntryState::Unsafe(detail) => {
                let (kind, severity) =
                    if std::fs::symlink_metadata(&entry)?.file_type().is_symlink() {
                        ("attachment_unsafe", Severity::Error)
                    } else {
                        ("attachment_invalid", Severity::Warning)
                    };
                deny(kind, severity, detail);
            }
            EntryState::File(size) if size > project.attachments_max_bytes => {
                deny(
                    "attachment_too_large",
                    Severity::Warning,
                    format!("{size} bytes exceeds cap {}", project.attachments_max_bytes),
                );
            }
            EntryState::File(_) if ledger.get(name) != Some(&Ledger::Attached) => {
                deny(
                    "attachment_unnoted",
                    Severity::Error,
                    "no current attached ledger entry".into(),
                );
            }
            EntryState::File(_) => {
                let snapshot = Snapshot::take(&entry)?;
                let bytes = snapshot.read(Some(project.attachments_max_bytes))?;
                row.eligible = true;
                row.record_sha256 = Some(record_hash.clone());
                row.file_sha256 = Some(digest(&bytes));
                snapshots.push(snapshot);
            }
        }
        rows.push(row);
    }
    for snapshot in snapshots {
        snapshot.verify()?;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eligibility_snapshot_rejects_observed_content_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record");
        std::fs::write(&path, b"before").unwrap();
        let snapshot = Snapshot::take(&path).unwrap();
        std::fs::write(&path, b"changed contents").unwrap();
        assert!(snapshot.read(None).is_err());
        assert!(snapshot.verify().is_err());
    }

    #[test]
    fn eligibility_snapshot_rejects_same_bytes_replacement_and_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let replacement = dir.path().join("replacement");
        std::fs::write(&path, b"same bytes").unwrap();
        let snapshot = Snapshot::take(&path).unwrap();
        std::fs::write(&replacement, b"same bytes").unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert!(snapshot.read(None).is_err());
        assert!(snapshot.verify().is_err());
        let snapshot = Snapshot::take(&path).unwrap();
        std::fs::rename(&path, &replacement).unwrap();
        std::os::unix::fs::symlink(&replacement, &path).unwrap();
        assert!(snapshot.read(None).is_err());
    }
}
