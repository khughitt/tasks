//! Files a task carries under `tasks/files/<id>/`, and the ledger notes that say which
//! record owns each one. See docs/specs/2026-09-28-task-attachments-design.md.

use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const FILES_DIR: &str = "files";
pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 128;

/// One path component of `[A-Za-z0-9._-]`, at most 128 bytes, not starting with `.`. No
/// space or colon, so a name parses unambiguously out of a ledger note.
pub fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidAttachmentName(format!(
            "attachment name {name:?} must be 1-{MAX_NAME_BYTES} bytes of [A-Za-z0-9._-], not starting with '.'"
        )))
    }
}

pub fn files_root(project: &Project) -> PathBuf {
    project.tasks_dir().join(FILES_DIR)
}

pub fn task_dir(project: &Project, id: &TaskId) -> PathBuf {
    files_root(project).join(id.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirState {
    Absent,
    Directory,
    Unsafe(String),
}

/// A storage directory's state, read without following a symlink.
pub fn dir_state(path: &Path) -> Result<DirState> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(DirState::Directory),
        Ok(meta) => Ok(DirState::Unsafe(format!(
            "{} is a {}, not a directory",
            path.display(),
            kind_of(&meta)
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(DirState::Absent),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryState {
    Absent,
    /// A regular file of this many bytes.
    File(u64),
    Unsafe(String),
}

/// An attachment's state, read without following a symlink or opening the entry.
pub fn entry_state(path: &Path) -> Result<EntryState> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(EntryState::File(meta.len())),
        Ok(meta) => Ok(EntryState::Unsafe(format!(
            "{} is a {}, not a regular file",
            path.display(),
            kind_of(&meta)
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EntryState::Absent),
        Err(error) => Err(error.into()),
    }
}

fn kind_of(meta: &std::fs::Metadata) -> &'static str {
    let kind = meta.file_type();
    if kind.is_symlink() {
        "symlink"
    } else if kind.is_dir() {
        "directory"
    } else if kind.is_file() {
        "regular file"
    } else {
        "special file"
    }
}

/// Checks both storage directories without following links. `Ok(true)` when the task's
/// directory exists; an unsafe one is `attachment_unsafe`.
pub fn check_storage(project: &Project, id: &TaskId) -> Result<bool> {
    for path in [files_root(project), task_dir(project, id)] {
        match dir_state(&path)? {
            DirState::Directory => {}
            DirState::Absent => return Ok(false),
            DirState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    Ok(true)
}

/// Creates whichever storage directories are missing. `create_dir` fails on an existing
/// symlink rather than writing through it. `Ok(true)` when the task directory was created.
pub fn ensure_task_dir(project: &Project, id: &TaskId) -> Result<bool> {
    let mut created = false;
    for path in [files_root(project), task_dir(project, id)] {
        match dir_state(&path)? {
            DirState::Directory => {}
            DirState::Absent => {
                std::fs::create_dir(&path)?;
                created = true;
            }
            DirState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    Ok(created)
}

/// Reads at most `max + 1` bytes, so a source over the cap costs no more memory than the
/// cap; one byte over is `attachment_too_large`.
pub fn read_capped(reader: impl Read, max: u64, what: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::AttachmentTooLarge(format!(
            "{what} is more than {max} bytes, the cap ([attachments] max_bytes in tasks/.config.toml)"
        )));
    }
    Ok(bytes)
}

/// Writes `bytes` to `path` through a `create_new` temp beside it, fsynced, then renamed.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("attachment paths end in a validated name");
    let temp = path.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written.and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ledger {
    Attached,
    Detached,
}

pub fn attached_note(name: &str, bytes: u64, caption: Option<&str>) -> String {
    match caption {
        Some(caption) => format!("attached: {name} ({bytes} bytes): {caption}"),
        None => format!("attached: {name} ({bytes} bytes)"),
    }
}

pub fn detached_note(name: &str, why: &str) -> String {
    format!("detached: {name}: {why}")
}

/// The ledger entry a note text records, if it is one.
pub fn parse_ledger_note(text: &str) -> Option<(Ledger, &str)> {
    if let Some(rest) = text.strip_prefix("attached: ") {
        let (name, rest) = rest.split_once(' ')?;
        let (count, rest) = rest.strip_prefix('(')?.split_once(" bytes)")?;
        let counted = !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit());
        let tail = rest.is_empty() || rest.starts_with(": ");
        return (counted && tail && validate_name(name).is_ok())
            .then_some((Ledger::Attached, name));
    }
    let (name, why) = text.strip_prefix("detached: ")?.split_once(": ")?;
    (!why.is_empty() && validate_name(name).is_ok()).then_some((Ledger::Detached, name))
}

/// Each name's latest ledger entry in `task`'s notes. A name is live when it is `Attached`.
pub fn ledger(task: &Task) -> BTreeMap<String, Ledger> {
    let mut state = BTreeMap::new();
    for note in &task.notes {
        if let Some((entry, name)) = parse_ledger_note(&note.text) {
            state.insert(name.to_string(), entry);
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_storage_rule() {
        for good in ["a.png", "Shot_2026-09-28.PNG", "x", &"a".repeat(128)] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", ".a", "a/b", "a b", "a:b", "é.png", &"a".repeat(129)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ledger_notes_round_trip_and_reject_near_misses() {
        assert_eq!(
            parse_ledger_note(&attached_note("a.png", 12, None)),
            Some((Ledger::Attached, "a.png"))
        );
        assert_eq!(
            parse_ledger_note(&attached_note("a.png", 12, Some("x (3 bytes): y"))),
            Some((Ledger::Attached, "a.png"))
        );
        assert_eq!(
            parse_ledger_note(&detached_note("a.png", "wrong: it was old")),
            Some((Ledger::Detached, "a.png"))
        );
        for text in [
            "attached: a.png",
            "attached: a.png (x bytes)",
            "attached: a.png (12 bytes)trailing",
            "attached: .a (1 bytes)",
            "detached: a.png",
            "detached: a.png: ",
            "note about attached: a.png (1 bytes)",
        ] {
            assert_eq!(parse_ledger_note(text), None, "{text}");
        }
    }
}
