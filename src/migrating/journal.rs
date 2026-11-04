//! The host-local migration journal (docs/project-operations.md): `move`'s journal widened to a set. It holds
//! the list's bytes and sha256, every listed record's bytes and the texts the run writes,
//! the attachment digests and the claim-store entries before and after, so a resume writes
//! the same bytes and any invocation can rebuild the paths the run owns.

use super::MigrateCtx;
use super::list::{Decision, LoadedList};
use super::project::Projection;
use crate::attachments::validate_name;
use crate::claims::{Escalation, Park};
use crate::error::{Error, Result};
use crate::model::{Status, TaskId};
use crate::moving::journal::JournalAttachment;
use crate::rename::inventory::digest;
use crate::repo::atomic_write;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One carried task: the fields of `move`'s journal, per task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarriedEntry {
    /// Full ids as strings: TaskId serializes but does not deserialize.
    pub source: String,
    pub target: String,
    pub source_text: String,
    pub source_digest: String,
    pub target_text: String,
    pub tombstone_text: String,
    /// The rewrite notes' texts, for the output (docs/project-operations.md).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewrites: Vec<String>,
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

/// One dropped or closed task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedEntry {
    pub id: String,
    /// `drop` or `close`.
    pub decision: String,
    /// The reason's text as the note records it.
    pub reason: String,
    pub source_text: String,
    pub source_digest: String,
    pub closed_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_depends: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_park: Option<Park>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_escalation: Option<Escalation>,
}

impl ClosedEntry {
    pub fn decision(&self) -> Decision {
        Decision::parse(&self.decision).expect("validated journal decision")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationJournal {
    /// The source and successor prefixes.
    pub source: String,
    pub successor: String,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    /// The records directories (a detached project's differ from its root): the run's
    /// owned paths are rebuilt from these.
    pub source_records: PathBuf,
    pub target_records: PathBuf,
    /// The list as given to the run that wrote this journal, absolute.
    pub list_path: PathBuf,
    pub list_sha256: String,
    pub list_text: String,
    /// The open successor tasks carried records are placed under (resume revalidation).
    #[serde(default)]
    pub parent_targets: Vec<String>,
    /// Registered projects that could not be read for dependents.
    #[serde(default)]
    pub unread_projects: Vec<String>,
    /// Each dropped or closed id to the open dependents it releases.
    #[serde(default)]
    pub release: BTreeMap<String, Vec<String>>,
    /// Each carried id to its dependents outside the carried set.
    #[serde(default)]
    pub inbound: BTreeMap<String, Vec<String>>,
    /// Each dropped or closed id to the shelves whose `wake_on` names it.
    #[serde(default)]
    pub woken: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub carried: Vec<CarriedEntry>,
    #[serde(default)]
    pub closed: Vec<ClosedEntry>,
}

fn prefix_id(prefix: &str) -> Result<TaskId> {
    TaskId::parse(&format!("{prefix}-000000"))
}

fn ids(ids: &[TaskId]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

impl MigrationJournal {
    pub fn path(prefix: &str) -> Result<PathBuf> {
        Self::path_with(prefix, |key| std::env::var_os(key))
    }

    pub(crate) fn path_with(
        prefix: &str,
        get: impl Fn(&str) -> Option<OsString>,
    ) -> Result<PathBuf> {
        prefix_id(prefix).map_err(|error| journal_error(Path::new(prefix), error))?;
        let base = if let Some(state) = get("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join(format!("tasks/migrate/{prefix}.toml")))
    }

    pub fn load(prefix: &str) -> Result<Option<Self>> {
        Self::load_from(&Self::path(prefix)?)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let journal: Self = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        journal.validate(path)?;
        Ok(Some(journal))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path(&self.source)?)
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        self.validate(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        atomic_write(path, text.as_bytes())
    }

    pub fn remove(&self) -> Result<()> {
        let path = Self::path(&self.source)?;
        self.validate(&path)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Every migration journal on this host, sorted by source prefix.
    pub fn pending() -> Result<Vec<Self>> {
        let directory = Self::path("aa")?
            .parent()
            .expect("journal path has a parent")
            .to_path_buf();
        Self::pending_in(&directory)
    }

    pub(crate) fn pending_in(directory: &Path) -> Result<Vec<Self>> {
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
        self.source == prefix || self.successor == prefix
    }

    /// A listed id (carried, dropped or closed) or a new id the run assigns.
    pub fn names_id(&self, id: &TaskId) -> bool {
        let id = id.to_string();
        self.carried
            .iter()
            .any(|entry| entry.source == id || entry.target == id)
            || self.closed.iter().any(|entry| entry.id == id)
    }

    pub fn pending_error(&self) -> Error {
        Error::MovePending(format!(
            "a migration of {} to {} is pending on this host; finish it with `tasks migrate {}`",
            self.source,
            self.successor,
            crate::stale::quote(&self.list_path.display().to_string())
        ))
    }

    /// The journal a fresh run writes at G2, from a projection with no refusal.
    pub fn from_projection(
        ctx: &MigrateCtx,
        projection: &Projection,
        list: &LoadedList,
        list_path: &Path,
    ) -> Result<Self> {
        if projection.list_sha != list.sha256 {
            return Err(Error::Validation(
                "the projection was rendered from another list; rerun the migration".into(),
            ));
        }
        let list_text = String::from_utf8(list.bytes.clone())
            .map_err(|error| Error::Validation(format!("decision list is not UTF-8: {error}")))?;
        let list_path = std::fs::canonicalize(list_path).map_err(|error| {
            Error::Validation(format!(
                "cannot resolve decision list {}: {error}",
                list_path.display()
            ))
        })?;
        let carried = projection
            .carried
            .iter()
            .map(|item| CarriedEntry {
                source: item.from.to_string(),
                target: item.pair.target.id.to_string(),
                source_digest: digest(item.source_text.as_bytes()),
                source_text: item.source_text.clone(),
                target_text: item.pair.target_text.clone(),
                tombstone_text: item.pair.tombstone_text.clone(),
                rewrites: item.rewrites.clone(),
                attachments: item.pair.attachments.clone(),
                source_park: item.pair.source_park.clone(),
                source_escalation: item.pair.source_escalation.clone(),
                target_park: item.pair.target_park.clone(),
                target_escalation: item.pair.target_escalation.clone(),
            })
            .collect();
        let closed = projection
            .closed
            .iter()
            .map(|item| ClosedEntry {
                id: item.id.to_string(),
                decision: match item.decision {
                    Decision::Drop => "drop",
                    Decision::Close => "close",
                    Decision::Carry => unreachable!("a carried entry is never rendered as closed"),
                }
                .into(),
                reason: item.reason.clone(),
                source_digest: digest(item.source_text.as_bytes()),
                source_text: item.source_text.clone(),
                closed_text: item.closed_text.clone(),
                open_depends: ids(&item.open_depends),
                source_park: item.source_park.clone(),
                source_escalation: item.source_escalation.clone(),
            })
            .collect();
        // The bytes journaled (and replaced at G6/G7) must be the bytes the list reviewed.
        let reviewed: BTreeMap<String, String> = list
            .list
            .tasks
            .iter()
            .filter_map(|entry| {
                let id = TaskId::parse(entry.id.trim()).ok()?;
                Some((
                    ctx.registry.canonical_id(&id).to_string(),
                    entry.digest.clone(),
                ))
            })
            .collect();
        let map = |m: &BTreeMap<TaskId, Vec<TaskId>>| {
            m.iter()
                .map(|(id, deps)| (id.to_string(), ids(deps)))
                .collect()
        };
        let journal = Self {
            source: ctx.source.project.prefix.clone(),
            successor: ctx.target.prefix.clone(),
            source_root: ctx.source.project.root.clone(),
            target_root: ctx.target.root.clone(),
            source_records: ctx.source.project.records.clone(),
            target_records: ctx.target.records.clone(),
            list_path,
            list_sha256: list.sha256.clone(),
            list_text,
            parent_targets: ids(&projection.parent_targets),
            unread_projects: projection.unread_projects.clone(),
            release: projection
                .closed
                .iter()
                .map(|item| (item.id.to_string(), ids(&item.released)))
                .collect(),
            inbound: map(&projection.inbound),
            woken: map(&projection.woken),
            carried,
            closed,
        };
        journal.check_reviewed(&reviewed)?;
        Ok(journal)
    }

    /// Every listed record's journaled bytes digest to its list entry's `digest`
    /// (`reviewed`: canonical id to `sha256:<hex>`), so a run never replaces bytes the
    /// review did not see.
    pub(crate) fn check_reviewed(&self, reviewed: &BTreeMap<String, String>) -> Result<()> {
        let listed = self
            .carried
            .iter()
            .map(|e| (&e.source, &e.source_digest))
            .chain(self.closed.iter().map(|e| (&e.id, &e.source_digest)));
        for (id, sha) in listed {
            let want = format!("sha256:{sha}");
            match reviewed.get(id) {
                Some(digest) if *digest == want => {}
                found => {
                    return Err(Error::Validation(format!(
                        "{id} changed after the list was reviewed (list digest {}, record {want}); nothing was written; re-read it and set `digest`, or redraft",
                        found.map_or("absent", String::as_str)
                    )));
                }
            }
        }
        Ok(())
    }

    /// For each records directory, every record and attachment path the run owns (absolute):
    /// the listed ids' records, the new ids' records, and every carried attachment in its
    /// directory on both sides (named file by file, since a record commit stages exactly
    /// the paths it is given). Derived from the journal alone.
    pub fn owned_paths(&self) -> BTreeMap<PathBuf, BTreeSet<PathBuf>> {
        let mut owned: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
        let (record, file) = (super::record_path, super::attachment_file);
        for entry in &self.carried {
            let source = owned.entry(self.source_records.clone()).or_default();
            source.insert(record(&self.source_records, &entry.source));
            for attachment in &entry.attachments {
                source.insert(file(&self.source_records, &entry.source, &attachment.name));
            }
            let target = owned.entry(self.target_records.clone()).or_default();
            target.insert(record(&self.target_records, &entry.target));
            for attachment in &entry.attachments {
                target.insert(file(&self.target_records, &entry.target, &attachment.name));
            }
        }
        for entry in &self.closed {
            owned
                .entry(self.source_records.clone())
                .or_default()
                .insert(record(&self.source_records, &entry.id));
        }
        owned
    }

    pub(crate) fn validate(&self, path: &Path) -> Result<()> {
        let bad = |what: String| Error::Config(format!("{}: {what}", path.display()));
        prefix_id(&self.source).map_err(|error| journal_error(path, error))?;
        prefix_id(&self.successor).map_err(|error| journal_error(path, error))?;
        if self.source == self.successor {
            return Err(bad(format!(
                "source and successor are both {:?}",
                self.source
            )));
        }
        if path.file_stem().and_then(|stem| stem.to_str()) != Some(&self.source) {
            return Err(bad(format!(
                "filename does not match source {:?}",
                self.source
            )));
        }
        for (name, dir) in [
            ("source_root", &self.source_root),
            ("target_root", &self.target_root),
            ("source_records", &self.source_records),
            ("target_records", &self.target_records),
            ("list_path", &self.list_path),
        ] {
            if !dir.is_absolute() {
                return Err(bad(format!("{name} must be absolute")));
            }
        }
        validate_digest(path, "list_sha256", &self.list_sha256)?;
        if digest(self.list_text.as_bytes()) != self.list_sha256 {
            return Err(bad("list_text does not digest to list_sha256".into()));
        }
        let mut seen = BTreeSet::new();
        let mut id_in = |text: &str, prefix: &str| -> Result<TaskId> {
            let id = TaskId::parse(text).map_err(|error| journal_error(path, error))?;
            if id.prefix != prefix {
                return Err(bad(format!("{text} is not a {prefix} id")));
            }
            if !seen.insert(text.to_string()) {
                return Err(bad(format!("{text} appears twice")));
            }
            Ok(id)
        };
        for entry in &self.carried {
            id_in(&entry.source, &self.source)?;
            id_in(&entry.target, &self.successor)?;
            validate_source(path, &entry.source_text, &entry.source_digest)?;
            let target = physical(path, &entry.target_text, &entry.target)?;
            if target.status() == Status::Moved {
                return Err(bad(format!("{} is written as a tombstone", entry.target)));
            }
            let tombstone = physical(path, &entry.tombstone_text, &entry.source)?;
            if tombstone.status() != Status::Moved {
                return Err(bad(format!("{}'s tombstone is not moved", entry.source)));
            }
            let mut names = BTreeSet::new();
            for attachment in &entry.attachments {
                validate_name(&attachment.name).map_err(|error| journal_error(path, error))?;
                if !names.insert(&attachment.name) {
                    return Err(bad(format!(
                        "duplicate attachment {:?} on {}",
                        attachment.name, entry.source
                    )));
                }
                validate_digest(path, "attachment sha256", &attachment.sha256)?;
            }
            if entry.source_park.is_some() != entry.target_park.is_some() {
                return Err(bad(format!(
                    "{}: source_park and target_park must both be present or absent",
                    entry.source
                )));
            }
            if entry.source_escalation != entry.target_escalation {
                return Err(bad(format!(
                    "{}: source_escalation and target_escalation must match",
                    entry.source
                )));
            }
            if let (Some(source), Some(target)) = (&entry.source_park, &entry.target_park) {
                let mut original = target.clone();
                original.worktree.clone_from(&source.worktree);
                if &original != source {
                    return Err(bad(format!(
                        "{}: target_park must match source_park except for worktree",
                        entry.source
                    )));
                }
            }
        }
        for entry in &self.closed {
            id_in(&entry.id, &self.source)?;
            validate_source(path, &entry.source_text, &entry.source_digest)?;
            let want = match Decision::parse(&entry.decision) {
                Some(Decision::Drop) => Status::Dropped,
                Some(Decision::Close) => Status::Done,
                _ => {
                    return Err(bad(format!(
                        "{}: decision {:?} is not drop or close",
                        entry.id, entry.decision
                    )));
                }
            };
            if physical(path, &entry.closed_text, &entry.id)?.status() != want {
                return Err(bad(format!(
                    "{}'s closed record is not {}",
                    entry.id,
                    want.as_str()
                )));
            }
        }
        let closed: BTreeSet<&str> = self.closed.iter().map(|e| e.id.as_str()).collect();
        for id in self.release.keys() {
            if !closed.contains(id.as_str()) {
                return Err(bad(format!(
                    "release names {id}, which is not dropped or closed"
                )));
            }
        }
        Ok(())
    }
}

/// The freeze over listed and new ids: a pending migration naming `id` refuses.
pub fn refuse_pending_id(id: &TaskId) -> Result<()> {
    for journal in MigrationJournal::pending()? {
        if journal.names_id(id) {
            return Err(journal.pending_error());
        }
    }
    Ok(())
}

/// The first pending migration naming `prefix` as its source or successor, other than the
/// one whose source is `except`.
pub fn pending_naming(prefix: &str, except: Option<&str>) -> Result<Option<MigrationJournal>> {
    Ok(MigrationJournal::pending()?
        .into_iter()
        .find(|journal| Some(journal.source.as_str()) != except && journal.names_prefix(prefix)))
}

fn physical(path: &Path, text: &str, id: &str) -> Result<crate::format::PhysicalTask> {
    let record = crate::format::parse_physical(text, &format!("{id}.md"))
        .map_err(|error| journal_error(path, error))?;
    if record.id().to_string() != id {
        return Err(Error::Config(format!(
            "{}: the text journaled for {id} is the record of {}",
            path.display(),
            record.id()
        )));
    }
    Ok(record)
}

fn validate_source(path: &Path, text: &str, sha: &str) -> Result<()> {
    validate_digest(path, "source_digest", sha)?;
    if digest(text.as_bytes()) != sha {
        return Err(Error::Config(format!(
            "{}: a source_text does not digest to its source_digest",
            path.display()
        )));
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
mod tests {
    use super::*;
    use crate::claims::WaitingOn;

    const SOURCE: &str = "---\nid: forge-00000a\ntitle: A\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\n---\n";

    fn sample() -> MigrationJournal {
        let target = SOURCE
            .replace("forge-00000a", "forge2-00000b")
            .replace("tags: []\n", "tags: []\nmoved_from: forge-00000a\n");
        let tombstone = SOURCE
            .replace("status: todo", "status: moved")
            .replace("tags: []\n", "tags: []\nmoved_to: forge2-00000b\n");
        let dropped_source = SOURCE.replace("forge-00000a", "forge-00000c");
        let dropped = dropped_source.replace("status: todo", "status: dropped");
        let list = "version = 1\nsource = \"forge\"\n\"multi\" = \"\"\"\nline\n\"\"\"\n";
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
        let mut target_park = park.clone();
        target_park.worktree = "/target".into();
        MigrationJournal {
            source: "forge".into(),
            successor: "forge2".into(),
            source_root: "/source".into(),
            target_root: "/target".into(),
            source_records: "/source".into(),
            target_records: "/target".into(),
            list_path: "/home/list.toml".into(),
            list_sha256: digest(list.as_bytes()),
            list_text: list.into(),
            parent_targets: vec![],
            unread_projects: vec![],
            release: [("forge-00000c".to_string(), vec!["ops-000001".to_string()])]
                .into_iter()
                .collect(),
            inbound: BTreeMap::new(),
            woken: BTreeMap::new(),
            carried: vec![CarriedEntry {
                source: "forge-00000a".into(),
                target: "forge2-00000b".into(),
                source_digest: digest(SOURCE.as_bytes()),
                source_text: SOURCE.into(),
                target_text: target,
                tombstone_text: tombstone,
                rewrites: vec!["parent forge-000009 not carried; set to none".into()],
                attachments: vec![JournalAttachment {
                    retain_source: false,
                    name: "shot.png".into(),
                    sha256: digest(b"png"),
                }],
                source_park: Some(park),
                source_escalation: None,
                target_park: Some(target_park),
                target_escalation: None,
            }],
            closed: vec![ClosedEntry {
                id: "forge-00000c".into(),
                decision: "drop".into(),
                reason: "gone".into(),
                source_digest: digest(dropped_source.as_bytes()),
                source_text: dropped_source,
                closed_text: dropped,
                open_depends: vec![],
                source_park: None,
                source_escalation: None,
            }],
        }
    }

    fn assert_invalid(journal: &MigrationJournal) {
        let error = journal.validate(Path::new("forge.toml")).unwrap_err();
        assert_eq!(error.kind(), "config", "{error}");
    }

    #[test]
    fn journal_round_trips_through_its_path() {
        let state = tempfile::tempdir().unwrap();
        let journal = sample();
        let path = MigrationJournal::path_with("forge", |key| {
            (key == "XDG_STATE_HOME").then(|| state.path().as_os_str().to_owned())
        })
        .unwrap();
        assert_eq!(path, state.path().join("tasks/migrate/forge.toml"));
        journal.save_to(&path).unwrap();
        assert_eq!(
            MigrationJournal::load_from(&path).unwrap(),
            Some(journal.clone())
        );
        assert_eq!(
            MigrationJournal::pending_in(&state.path().join("tasks/migrate")).unwrap(),
            vec![journal.clone()]
        );
        assert!(journal.names_prefix("forge") && journal.names_prefix("forge2"));
        assert!(!journal.names_prefix("ops"));
        for id in ["forge-00000a", "forge2-00000b", "forge-00000c"] {
            assert!(journal.names_id(&TaskId::parse(id).unwrap()), "{id}");
        }
        assert!(!journal.names_id(&TaskId::parse("forge-00000d").unwrap()));
        let error = journal.pending_error();
        assert_eq!(error.kind(), "move_pending");
        assert!(
            error
                .to_string()
                .contains("a migration of forge to forge2 is pending on this host; finish it with `tasks migrate /home/list.toml`"),
            "{error}"
        );
    }

    #[test]
    fn owned_paths_cover_records_and_carried_attachment_directories() {
        let owned = sample().owned_paths();
        let source: Vec<_> = owned[Path::new("/source")].iter().cloned().collect();
        assert_eq!(
            source,
            [
                "/source/tasks/files/forge-00000a/shot.png",
                "/source/tasks/forge-00000a.md",
                "/source/tasks/forge-00000c.md",
            ]
            .map(PathBuf::from)
        );
        let target: Vec<_> = owned[Path::new("/target")].iter().cloned().collect();
        assert_eq!(
            target,
            [
                "/target/tasks/files/forge2-00000b/shot.png",
                "/target/tasks/forge2-00000b.md"
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn journaled_bytes_must_be_the_reviewed_ones() {
        let j = sample();
        let reviewed = |carried: &str, closed: &str| -> BTreeMap<String, String> {
            [
                ("forge-00000a".to_string(), format!("sha256:{carried}")),
                ("forge-00000c".to_string(), format!("sha256:{closed}")),
            ]
            .into_iter()
            .collect()
        };
        let (a, c) = (&j.carried[0].source_digest, &j.closed[0].source_digest);
        j.check_reviewed(&reviewed(a, c)).unwrap();
        for digests in [reviewed(&digest(b"x"), c), reviewed(a, &digest(b"x"))] {
            let error = j.check_reviewed(&digests).unwrap_err();
            assert_eq!(error.kind(), "validation", "{error}");
            assert!(error.to_string().contains("digest"), "{error}");
        }
        let mut missing = reviewed(a, c);
        missing.remove("forge-00000c");
        assert_eq!(j.check_reviewed(&missing).unwrap_err().kind(), "validation");
    }

    #[test]
    fn validate_rejects_incoherent_journals() {
        let mut j = sample();
        j.list_text.push('x');
        assert_invalid(&j);
        let mut j = sample();
        j.carried[0].source_text.push('x');
        assert_invalid(&j);
        let mut j = sample();
        j.carried[0].target = "ops-00000b".into();
        assert_invalid(&j);
        let mut j = sample();
        j.closed[0].id = "forge-00000a".into();
        assert_invalid(&j);
        let mut j = sample();
        j.carried[0].tombstone_text = j.carried[0].source_text.clone();
        assert_invalid(&j);
        let mut j = sample();
        j.carried[0].target_park.as_mut().unwrap().next_step = "other".into();
        assert_invalid(&j);
        let mut j = sample();
        j.closed[0].decision = "carry".into();
        assert_invalid(&j);
        let mut j = sample();
        j.carried[0].attachments[0].name = "../x".into();
        assert_invalid(&j);
        let mut j = sample();
        j.release.insert("forge-00000a".into(), vec![]);
        assert_invalid(&j);
        let mut j = sample();
        j.successor = "forge".into();
        assert_invalid(&j);
        assert_eq!(
            sample()
                .validate(Path::new("forge2.toml"))
                .unwrap_err()
                .kind(),
            "config"
        );
    }
}
