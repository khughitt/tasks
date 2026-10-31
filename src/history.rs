use crate::attachments::{self, DirState, Ledger};
use crate::error::{Error, Result};
use crate::format::PhysicalTask;
use crate::model::{Note, Task, TaskId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveDescriptor {
    pub sha256: String,
    pub note_count: usize,
}

impl ArchiveDescriptor {
    pub fn parse(value: &str) -> Result<Self> {
        let parts: Vec<_> = value.split(':').collect();
        if parts.len() != 3
            || parts[0] != "v1"
            || parts[1].len() != 64
            || !parts[1]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || parts[2].is_empty()
            || !parts[2].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error::Validation("note archive descriptor must be v1:<64 lowercase SHA-256 hex>:<positive note count>".into()));
        }
        let note_count = parts[2]
            .parse::<usize>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| {
                Error::Validation(
                    "note archive count must be a positive integer within this platform's range"
                        .into(),
                )
            })?;
        Ok(Self {
            sha256: parts[1].into(),
            note_count,
        })
    }

    pub fn file_name(&self) -> String {
        format!("notes-{}.notes", self.sha256)
    }
}

impl std::fmt::Display for ArchiveDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v1:{}:{}", self.sha256, self.note_count)
    }
}

pub fn validate_descriptors(archives: &[ArchiveDescriptor]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for archive in archives {
        ArchiveDescriptor::parse(&archive.to_string())?;
        if !seen.insert(&archive.sha256) {
            return Err(Error::Validation(format!(
                "duplicate note archive {}",
                archive.file_name()
            )));
        }
    }
    Ok(())
}

fn archive_error(name: &str, detail: impl std::fmt::Display) -> Error {
    Error::Parse {
        file: name.into(),
        detail: format!(
            "{detail}; restore the exact archive bytes from a verified source; never skip required history"
        ),
    }
}

fn prefix(
    archives: &[ArchiveDescriptor],
    mut resolve: impl FnMut(&ArchiveDescriptor) -> Result<Vec<u8>>,
) -> Result<(Vec<Note>, u64)> {
    validate_descriptors(archives)?;
    let mut notes = Vec::new();
    let mut total_bytes = 0;
    for archive in archives {
        let name = archive.file_name();
        let bytes = resolve(archive).map_err(|error| archive_error(&name, error))?;
        if format!("{:x}", Sha256::digest(&bytes)) != archive.sha256 {
            return Err(archive_error(&name, "SHA-256 mismatch"));
        }
        let text = std::str::from_utf8(&bytes).map_err(|error| archive_error(&name, error))?;
        let segment = crate::format::parse_note_section(text, &name)
            .map_err(|error| archive_error(&name, error))?;
        if segment.len() != archive.note_count {
            return Err(archive_error(&name, "note count mismatch"));
        }
        total_bytes += bytes.len() as u64;
        notes.extend(segment);
    }
    Ok((notes, total_bytes))
}

fn validate_ledger(task: &Task) -> Result<()> {
    let ledger = attachments::ledger(task);
    for archive in &task.note_archives {
        let name = archive.file_name();
        if ledger.get(&name) != Some(&Ledger::Attached) {
            return Err(archive_error(
                &name,
                "required archive has no live attached ledger entry",
            ));
        }
    }
    Ok(())
}

pub fn load_logical(
    mut physical: PhysicalTask,
    resolve: impl FnMut(&ArchiveDescriptor) -> Result<Vec<u8>>,
) -> Result<Task> {
    let (mut notes, _) = prefix(&physical.archives, resolve)?;
    notes.append(&mut physical.inline_notes);
    let task = physical.assemble(notes);
    validate_ledger(&task)?;
    Ok(task)
}

pub fn physical_for_save(
    task: &Task,
    resolve: impl FnMut(&ArchiveDescriptor) -> Result<Vec<u8>>,
) -> Result<PhysicalTask> {
    let (archived, _) = prefix(&task.note_archives, resolve)?;
    if task.notes.get(..archived.len()) != Some(archived.as_slice()) {
        return Err(Error::Validation(format!(
            "{}: archived note prefix changed; immutable history cannot be edited",
            task.id
        )));
    }
    validate_ledger(task)?;
    Ok(PhysicalTask::project(
        task,
        task.notes[archived.len()..].to_vec(),
    ))
}

pub fn archived_bytes(
    task: &Task,
    resolve: impl FnMut(&ArchiveDescriptor) -> Result<Vec<u8>>,
) -> Result<u64> {
    prefix(&task.note_archives, resolve).map(|(_, bytes)| bytes)
}

// Explicit records context also serves pinned journal/historical readers; no cwd lookup.
pub fn read_archive_at(
    records: &Path,
    id: &TaskId,
    archive: &ArchiveDescriptor,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    let directory = records.join(format!("tasks/files/{id}"));
    let name = archive.file_name();
    let path = directory.join(&name);
    let read = || -> Result<Vec<u8>> {
        for dir in [records.join("tasks/files"), directory] {
            match attachments::dir_state(&dir)? {
                DirState::Directory => {}
                _ => {
                    return Err(Error::AttachmentUnsafe(format!(
                        "{} is not a real archive directory",
                        dir.display()
                    )));
                }
            }
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(Error::AttachmentUnsafe(format!(
                "{} is not a regular archive file",
                path.display()
            )));
        }
        attachments::read_capped(file, max_bytes, &path.display().to_string())
    };
    read().map_err(|error| archive_error(&path.display().to_string(), error))
}

#[cfg(test)]
pub(crate) fn fixture(text: &str, file: &str) -> Result<Task> {
    load_logical(crate::format::parse_physical(text, file)?, |_| {
        Err(Error::Validation(
            "inline fixture has no archive resolver".into(),
        ))
    })
}

#[cfg(test)]
pub(crate) fn fixture_text(task: &Task) -> String {
    crate::format::serialize_physical(
        &physical_for_save(task, |_| {
            Err(Error::Validation(
                "inline fixture has no archive resolver".into(),
            ))
        })
        .expect("inline fixture history is complete"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_metadata_and_checked_save_boundary() {
        let bytes = b"## Notes\r\n\r\n- 2026-10-01T00:00:00Z (fixture): immutable\r\n  provenance: {\"harness_session\":\"codex:test\",\"harness_session_source\":\"CODEX_THREAD_ID\",\"origin\":\"user\"}\r\n";
        let sha = format!("{:x}", Sha256::digest(bytes));
        let raw = format!(
            "---\nid: dot-a00001\ntitle: Metadata\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\nnote_archives: [\"v1:{sha}:1\"]\n---\n\n## Notes\n\n- 2026-10-02T00:00:00Z (fixture): attached: notes-{sha}.notes ({} bytes)\n",
            bytes.len()
        );
        let physical = crate::format::parse_physical(&raw, "fixture").unwrap();
        assert_eq!(physical.id().to_string(), "dot-a00001");
        assert_eq!(physical.status(), crate::model::Status::Todo);
        assert_eq!(physical.updated(), "2026-10-01T00:00:00Z");
        assert_eq!(physical.inline_notes.len(), 1);
        assert_eq!(physical.archives.len(), 1);
        let mut task = load_logical(physical, |_| Ok(bytes.to_vec())).unwrap();
        assert_eq!(task.notes.len(), 2);
        assert_eq!(
            task.notes[0].provenance.as_ref().unwrap().harness_session,
            "codex:test"
        );
        assert_eq!(
            task.notes[0].occurrence.origin,
            Some(crate::model::Origin::User)
        );
        assert!(
            serde_json::to_value(&task)
                .unwrap()
                .get("note_archives")
                .is_none()
        );
        let saved = crate::format::serialize_physical(
            &physical_for_save(&task, |_| Ok(bytes.to_vec())).unwrap(),
        );
        assert!(!saved.contains("immutable"));
        assert!(
            fixture(&saved, "missing resolver")
                .unwrap_err()
                .to_string()
                .contains("no archive resolver")
        );
        let reload = load_logical(
            crate::format::parse_physical(&saved, "saved").unwrap(),
            |_| Ok(bytes.to_vec()),
        )
        .unwrap();
        assert_eq!(reload, task);
        task.notes[0].text = "modified".into();
        assert!(physical_for_save(&task, |_| Ok(bytes.to_vec())).is_err());
    }
}
