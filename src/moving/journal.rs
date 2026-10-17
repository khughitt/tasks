use crate::attachments::validate_name;
use crate::claims::{Escalation, Park};
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::rename::inventory::digest;
use crate::repo::atomic_write;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalAttachment {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    /// Full ids as strings: TaskId serializes but does not deserialize.
    pub source: String,
    pub target: String,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub source_text: String,
    pub source_digest: String,
    pub target_text: String,
    pub tombstone_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<JournalAttachment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_park: Option<Park>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_escalation: Option<Escalation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_park: Option<Park>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_escalation: Option<Escalation>,
}

impl Journal {
    pub fn source_id(&self) -> TaskId {
        TaskId::parse(&self.source).expect("validated journal source id")
    }

    pub fn target_id(&self) -> TaskId {
        TaskId::parse(&self.target).expect("validated journal target id")
    }

    pub fn path(source: &TaskId) -> Result<PathBuf> {
        Self::path_with(source, |key| std::env::var_os(key))
    }

    pub(crate) fn path_with(
        source: &TaskId,
        get: impl Fn(&str) -> Option<OsString>,
    ) -> Result<PathBuf> {
        TaskId::parse(&source.to_string())
            .map_err(|error| journal_error(Path::new(&source.to_string()), error))?;
        let base = if let Some(state) = get("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join(format!("tasks/move/{source}.toml")))
    }

    pub fn load(source: &TaskId) -> Result<Option<Journal>> {
        Self::load_from(&Self::path(source)?)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Option<Journal>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let journal: Journal = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        journal.validate(path)?;
        Ok(Some(journal))
    }

    pub fn save(&self) -> Result<()> {
        let source = TaskId::parse(&self.source)
            .map_err(|error| journal_error(Path::new(&self.source), error))?;
        self.save_to(&Self::path(&source)?)
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        self.validate(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(
            path,
            toml::to_string(self)
                .expect("validated journal serializes")
                .as_bytes(),
        )
    }

    pub fn remove(&self) -> Result<()> {
        let source = TaskId::parse(&self.source)
            .map_err(|error| journal_error(Path::new(&self.source), error))?;
        let path = Self::path(&source)?;
        self.validate(&path)?;
        Self::remove_from(&path)
    }

    pub(crate) fn remove_from(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn pending() -> Result<Vec<Journal>> {
        let directory = Self::path(&TaskId::parse("aa-000000").expect("valid placeholder id"))?
            .parent()
            .expect("journal path has a parent")
            .to_path_buf();
        Self::pending_in(&directory)
    }

    pub(crate) fn pending_in(directory: &Path) -> Result<Vec<Journal>> {
        let read_dir = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut paths = Vec::new();
        for entry in read_dir {
            let path = entry?.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
                paths.push(path);
            }
        }
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                Self::load_from(&path)?.ok_or_else(|| {
                    Error::Config(format!("{} disappeared while being read", path.display()))
                })
            })
            .collect()
    }

    pub fn names_prefix(&self, prefix: &str) -> bool {
        self.source_id().prefix == prefix || self.target_id().prefix == prefix
    }

    pub fn names_id(&self, id: &TaskId) -> bool {
        self.source == id.to_string() || self.target == id.to_string()
    }

    pub(crate) fn pending_error(&self) -> Error {
        Error::MovePending(format!(
            "a move of {} to {} is pending on this host; finish it with `tasks move {} --to {}`",
            self.source,
            self.target,
            self.source,
            self.target_id().prefix
        ))
    }

    pub(crate) fn validate(&self, path: &Path) -> Result<()> {
        let source = TaskId::parse(&self.source).map_err(|error| journal_error(path, error))?;
        let target = TaskId::parse(&self.target).map_err(|error| journal_error(path, error))?;
        if source.prefix == target.prefix {
            return Err(Error::Config(format!(
                "{}: source and target prefixes are both {:?}",
                path.display(),
                source.prefix
            )));
        }
        if path.file_stem().and_then(|stem| stem.to_str()) != Some(&self.source) {
            return Err(Error::Config(format!(
                "{}: filename does not match source {:?}",
                path.display(),
                self.source
            )));
        }
        if !self.source_root.is_absolute() || !self.target_root.is_absolute() {
            return Err(Error::Config(format!(
                "{}: source_root and target_root must be absolute",
                path.display()
            )));
        }
        validate_digest(path, "source_digest", &self.source_digest)?;
        if self.source_digest != digest(self.source_text.as_bytes()) {
            return Err(Error::Config(format!(
                "{}: source_text does not digest to source_digest",
                path.display()
            )));
        }
        let mut seen = BTreeSet::new();
        for attachment in &self.attachments {
            validate_name(&attachment.name).map_err(|error| journal_error(path, error))?;
            if !seen.insert(&attachment.name) {
                return Err(Error::Config(format!(
                    "{}: duplicate attachment {:?}",
                    path.display(),
                    attachment.name
                )));
            }
            validate_digest(path, "attachment sha256", &attachment.sha256)?;
        }
        if self.source_park.is_some() != self.target_park.is_some() {
            return Err(Error::Config(format!(
                "{}: source_park and target_park must both be present or absent",
                path.display()
            )));
        }
        if self.source_escalation != self.target_escalation {
            return Err(Error::Config(format!(
                "{}: source_escalation and target_escalation must match",
                path.display()
            )));
        }
        if let Some(target) = &self.target_park {
            let mut original = target.clone();
            if let Some(source) = &self.source_park {
                original.worktree.clone_from(&source.worktree);
            }
            if self.source_park.as_ref() != Some(&original) {
                return Err(Error::Config(format!(
                    "{}: target_park must match source_park except for worktree",
                    path.display()
                )));
            }
        }
        self.validate_records()
            .map_err(|error| journal_error(path, error))
    }

    fn validate_records(&self) -> Result<()> {
        use crate::attachments::{Ledger, ledger};
        use crate::format::parse_task;
        use crate::model::Status;
        let source = parse_task(&self.source_text, &format!("{}.md", self.source))?;
        let target = parse_task(&self.target_text, &format!("{}.md", self.target))?;
        let tombstone = parse_task(&self.tombstone_text, &format!("{}.md", self.source))?;
        let refuse = || {
            Error::Validation("journal records do not describe one move preserving the source fields, history and attachment ledger".into())
        };
        if source.id != self.source_id()
            || !source.status.is_open()
            || source.parent.is_some()
            || target.updated != tombstone.updated
            || crate::time::parse(&target.updated)? <= crate::time::parse(&source.updated)?
        {
            return Err(refuse());
        }
        let attached: BTreeSet<_> = ledger(&source)
            .into_iter()
            .filter_map(|(name, state)| (state == Ledger::Attached).then_some(name))
            .collect();
        if attached != self.attachments.iter().map(|a| a.name.clone()).collect() {
            return Err(refuse());
        }
        let mut expected_target = source.clone();
        expected_target.id = self.target_id();
        expected_target.moved_from = Some(source.id.clone());
        expected_target.updated.clone_from(&target.updated);
        let mut expected_tombstone = source.clone();
        expected_tombstone.status = Status::Moved;
        expected_tombstone.reviewed = false;
        expected_tombstone.reviewed_at = None;
        expected_tombstone.awaiting_user = false;
        expected_tombstone.moved_to = Some(target.id.clone());
        expected_tombstone.updated.clone_from(&target.updated);
        for (actual, expected, suffix) in [
            (
                &target,
                &mut expected_target,
                vec![format!("moved from {}", source.id)],
            ),
            (
                &tombstone,
                &mut expected_tombstone,
                std::iter::once(format!("moved to {}", target.id))
                    .chain(self.attachments.iter().map(|a| {
                        crate::attachments::detached_note(
                            &a.name,
                            &format!("moved to {}", target.id),
                        )
                    }))
                    .collect(),
            ),
        ] {
            if actual.notes.len() != source.notes.len() + suffix.len() {
                return Err(refuse());
            }
            for (note, text) in actual.notes[source.notes.len()..].iter().zip(suffix) {
                if note.text != text {
                    return Err(refuse());
                }
                expected.notes.push(note.clone());
            }
            if actual != expected {
                return Err(refuse());
            }
        }
        Ok(())
    }
}

pub fn refuse_pending_id(id: &TaskId) -> Result<()> {
    for journal in Journal::pending()? {
        if journal.names_id(id) {
            return Err(journal.pending_error());
        }
    }
    Ok(())
}

fn validate_digest(path: &Path, field: &str, value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{}: {field} is not a lowercase SHA-256 digest",
            path.display()
        )))
    }
}

fn journal_error(path: &Path, error: Error) -> Error {
    Error::Config(format!("{}: {error}", path.display()))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::claims::WaitingOn;
    use crate::rename::inventory::digest;

    pub(in crate::moving) fn sample() -> Journal {
        use crate::format::{parse_task, serialize_task};
        let source_text = "---\nid: dots-00000a\ntitle: A\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\n---\n".to_string();
        let source = parse_task(&source_text, "dots-00000a.md").unwrap();
        let mut target = source.clone();
        target.id = TaskId::parse("wali-00000b").unwrap();
        target.moved_from = Some(source.id.clone());
        target.updated = "2026-10-02T00:00:00Z".into();
        crate::commands::append_note(&mut target, "owner", "moved from dots-00000a").unwrap();
        let mut tombstone = source;
        tombstone.status = crate::model::Status::Moved;
        tombstone.moved_to = Some(target.id.clone());
        tombstone.updated.clone_from(&target.updated);
        crate::commands::append_note(&mut tombstone, "owner", "moved to wali-00000b").unwrap();
        Journal {
            source: "dots-00000a".into(),
            target: "wali-00000b".into(),
            source_root: "/source".into(),
            target_root: "/target".into(),
            source_digest: digest(source_text.as_bytes()),
            source_text,
            target_text: serialize_task(&target),
            tombstone_text: serialize_task(&tombstone),
            attachments: vec![],
            source_park: None,
            source_escalation: None,
            target_park: None,
            target_escalation: None,
        }
    }

    fn assert_invalid(journal: &Journal, path: &Path) {
        let error = journal.validate(path).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn journal_round_trips_through_its_path() {
        let state = tempfile::tempdir().unwrap();
        let journal = sample();
        let path = Journal::path_with(&journal.source_id(), |key| {
            (key == "XDG_STATE_HOME").then(|| state.path().as_os_str().to_owned())
        })
        .unwrap();
        assert_eq!(path, state.path().join("tasks/move/dots-00000a.toml"));
        journal.save_to(&path).unwrap();
        assert_eq!(Journal::load_from(&path).unwrap(), Some(journal.clone()));
        assert_eq!(journal.target_id(), TaskId::parse("wali-00000b").unwrap());
        assert!(journal.names_prefix("dots"));
        assert!(journal.names_prefix("wali"));
        assert!(!journal.names_prefix("other"));
        assert!(journal.names_id(&journal.source_id()));
        assert!(journal.names_id(&journal.target_id()));
        assert!(!journal.names_id(&TaskId::parse("dots-00000b").unwrap()));
        Journal::remove_from(&path).unwrap();
        assert_eq!(Journal::load_from(&path).unwrap(), None);
        Journal::remove_from(&path).unwrap();
    }

    #[test]
    fn validate_rejects_a_source_digest_that_does_not_match_source_text() {
        let mut journal = sample();
        journal.source_digest = digest(b"x");
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
    }

    #[test]
    fn validate_rejects_a_same_project_target() {
        let mut journal = sample();
        journal.target = "dots-00000b".into();
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
    }

    #[test]
    fn validate_rejects_a_filename_that_is_not_the_source() {
        assert_invalid(&sample(), Path::new("wali-00000b.toml"));
    }

    #[test]
    fn validate_rejects_unsafe_attachment_names() {
        let mut journal = sample();
        for name in ["/etc/passwd", "../x", "a/b", ".hidden", ""] {
            journal.attachments = vec![JournalAttachment {
                name: name.into(),
                sha256: digest(b"png"),
            }];
            assert_invalid(&journal, Path::new("dots-00000a.toml"));
        }
        journal.attachments = vec![
            JournalAttachment {
                name: "shot.png".into(),
                sha256: digest(b"png")
            };
            2
        ];
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
    }

    #[test]
    fn validate_rejects_relative_roots() {
        let mut journal = sample();
        journal.source_root = "source".into();
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
        journal.source_root = "/source".into();
        journal.target_root = "target".into();
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
    }

    #[test]
    fn pending_in_reads_every_journal_sorted() {
        let state = tempfile::tempdir().unwrap();
        let first = sample();
        let mut second = sample();
        second.source = "dots-00000c".into();
        second.source_text = second.source_text.replace("dots-00000a", "dots-00000c");
        second.target_text = second.target_text.replace("dots-00000a", "dots-00000c");
        second.tombstone_text = second.tombstone_text.replace("dots-00000a", "dots-00000c");
        second.source_digest = digest(second.source_text.as_bytes());
        second
            .save_to(&state.path().join("dots-00000c.toml"))
            .unwrap();
        first
            .save_to(&state.path().join("dots-00000a.toml"))
            .unwrap();
        std::fs::write(state.path().join("unrelated.txt"), "ignored").unwrap();
        assert_eq!(
            Journal::pending_in(state.path()).unwrap(),
            vec![first, second]
        );
        assert!(
            Journal::pending_in(&state.path().join("missing"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn validate_rejects_invalid_ids_and_digests() {
        for field in ["source", "target"] {
            let mut journal = sample();
            if field == "source" {
                journal.source = "../x".into();
            } else {
                journal.target = "wali-00000B".into();
            }
            assert_invalid(&journal, Path::new("dots-00000a.toml"));
        }
        for bad in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
            let mut journal = sample();
            journal.source_digest = bad.clone();
            assert_invalid(&journal, Path::new("dots-00000a.toml"));
            journal.source_digest = digest(b"a");
            journal.attachments = vec![JournalAttachment {
                name: "shot.png".into(),
                sha256: bad,
            }];
            assert_invalid(&journal, Path::new("dots-00000a.toml"));
        }
    }

    #[test]
    fn validate_allows_only_worktree_to_change_in_target_park() {
        let mut journal = sample();
        let park = Park {
            owner: "owner".into(),
            session: "codex:test".into(),
            host: "host".into(),
            worktree: "/source".into(),
            at: "2026-10-09T00:00:00Z".into(),
            next_step: "continue".into(),
            waiting_on: WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "task".into(),
        };
        journal.target_park = Some(park.clone());
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
        journal.source_park = Some(park);
        let target = journal.target_park.as_mut().unwrap();
        target.worktree = "/target".into();
        journal.validate(Path::new("dots-00000a.toml")).unwrap();
        journal.target_park.as_mut().unwrap().next_step = "different".into();
        assert_invalid(&journal, Path::new("dots-00000a.toml"));
    }

    #[test]
    fn load_pending_and_save_reject_invalid_journals_before_action() {
        let state = tempfile::tempdir().unwrap();
        let path = state.path().join("dots-00000a.toml");
        let mut journal = sample();
        journal.attachments = vec![JournalAttachment {
            name: "../x".into(),
            sha256: digest(b"x"),
        }];
        assert_eq!(journal.save_to(&path).unwrap_err().kind(), "config");
        assert!(!path.exists());
        std::fs::write(&path, toml::to_string(&journal).unwrap()).unwrap();
        assert_eq!(Journal::load_from(&path).unwrap_err().kind(), "config");
        assert_eq!(
            Journal::pending_in(state.path()).unwrap_err().kind(),
            "config"
        );
    }

    #[test]
    fn incoherent_record_texts_fail_load_pending_and_save() {
        for field in ["target_text", "tombstone_text", "source_text"] {
            let state = tempfile::tempdir().unwrap();
            let path = state.path().join("dots-00000a.toml");
            let mut journal = sample();
            match field {
                "target_text" => {
                    journal.target_text = journal.target_text.replace("title: A", "title: Changed")
                }
                "tombstone_text" => journal.tombstone_text.clone_from(&journal.source_text),
                "source_text" => {
                    journal.source_text = journal
                        .source_text
                        .replace("status: todo", "status: dropped");
                    journal.source_digest = digest(journal.source_text.as_bytes());
                }
                _ => unreachable!(),
            }
            assert_eq!(journal.save_to(&path).unwrap_err().kind(), "config");
            assert!(!path.exists());
            std::fs::write(&path, toml::to_string(&journal).unwrap()).unwrap();
            assert_eq!(Journal::load_from(&path).unwrap_err().kind(), "config");
            assert_eq!(
                Journal::pending_in(state.path()).unwrap_err().kind(),
                "config"
            );
        }
    }

    #[test]
    fn state_path_uses_home_and_requires_a_state_base() {
        let source = sample().source_id();
        assert_eq!(
            Journal::path_with(&source, |key| (key == "HOME")
                .then(|| "/home/person".into()))
            .unwrap(),
            Path::new("/home/person/.local/state/tasks/move/dots-00000a.toml")
        );
        assert_eq!(
            Journal::path_with(&source, |_| None).unwrap_err().kind(),
            "config"
        );
    }
}
