//! Recovery classification (spec §4.5 "Classification"), pure and closed as `move`'s. This
//! is the path with a journal; the journal-less path decides `Fresh`, `Complete` or
//! `Refuse` from the records alone.

use super::journal::MigrationJournal;
use super::list::LoadedList;
use crate::claims::{Escalation, Park};
use crate::moving::classify::FileState;
use crate::rename::inventory::digest;
use std::collections::BTreeMap;

/// The apply phases G2-G9, in order; G10 (removing the journal) follows the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Journal,
    Attachments,
    Destination,
    Store,
    Tombstones,
    Closures,
    Cleanup,
    Commit,
}

impl Phase {
    /// The `TASKS_MIGRATE_STOP_AFTER` value that stops after this phase.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Journal => "journal",
            Phase::Attachments => "attachments",
            Phase::Destination => "destination",
            Phase::Store => "store",
            Phase::Tombstones => "tombstones",
            Phase::Closures => "closures",
            Phase::Cleanup => "cleanup",
            Phase::Commit => "commit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationRecovery {
    // `Fresh` and `Complete` are the journal-less classification's verdicts (plan Task 6).
    #[allow(dead_code)]
    Fresh,
    /// Continue from this phase: every earlier phase's effects are present.
    Resume(Phase),
    #[allow(dead_code)]
    Complete,
    Refuse(String),
}

/// What is on disk for one carried entry of the journal.
#[derive(Debug, Clone)]
pub struct CarriedState {
    pub source: FileState,
    pub target: FileState,
    pub source_files: BTreeMap<String, FileState>,
    pub target_files: BTreeMap<String, FileState>,
    /// The source attachment directory exists and holds nothing.
    pub source_dir_empty: bool,
    pub source_park: Option<Park>,
    pub source_escalation: Option<Escalation>,
    pub target_park: Option<Park>,
    pub target_escalation: Option<Escalation>,
}

/// What is on disk for one dropped or closed entry.
#[derive(Debug, Clone)]
pub struct ClosedState {
    pub record: FileState,
    pub park: Option<Park>,
    pub escalation: Option<Escalation>,
}

/// Parallel to the journal's `carried` and `closed`.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub carried: Vec<CarriedState>,
    pub closed: Vec<ClosedState>,
}

fn sha(text: &str) -> FileState {
    FileState::Digest(digest(text.as_bytes()))
}

pub fn classify_with_journal(
    journal: &MigrationJournal,
    snapshot: &Snapshot,
    list: &LoadedList,
) -> MigrationRecovery {
    use MigrationRecovery::Refuse;
    if list.sha256 != journal.list_sha256 {
        return Refuse(format!(
            "the decision list changed since this migration started (sha256:{} when it started, sha256:{} now); a pending run continues only with the list it started with: restore {} to those bytes (the journal holds them) and rerun",
            journal.list_sha256,
            list.sha256,
            journal.list_path.display()
        ));
    }
    assert_eq!(journal.carried.len(), snapshot.carried.len());
    assert_eq!(journal.closed.len(), snapshot.closed.len());
    for (j, s) in journal.carried.iter().zip(&snapshot.carried) {
        let original = FileState::Digest(j.source_digest.clone());
        let tombstone = sha(&j.tombstone_text);
        let target = sha(&j.target_text);
        if s.source != original && s.source != tombstone {
            return Refuse(format!(
                "{} no longer matches the journal or the tombstone it would write; it changed during the migration",
                j.source
            ));
        }
        if s.target != FileState::Absent && s.target != target {
            return Refuse(format!(
                "{} exists with bytes the journal did not write",
                j.target
            ));
        }
        if s.source == tombstone && s.target == FileState::Absent {
            return Refuse(format!(
                "{} is already a tombstone but {} is missing",
                j.source, j.target
            ));
        }
        for attachment in &j.attachments {
            let want = FileState::Digest(attachment.sha256.clone());
            for (id, files) in [(&j.source, &s.source_files), (&j.target, &s.target_files)] {
                if let Some(FileState::Digest(actual)) = files.get(&attachment.name)
                    && actual != &attachment.sha256
                {
                    return Refuse(format!(
                        "attachment {} in {id} differs from its journal digest",
                        attachment.name
                    ));
                }
            }
            let here = s.source_files.get(&attachment.name) == Some(&want);
            let there = s.target_files.get(&attachment.name) == Some(&want);
            if attachment.retain_source && !here {
                return Refuse(format!(
                    "required source archive {} of {} is missing; restore exact source bytes before retrying",
                    attachment.name, j.source
                ));
            }
            if !here && !there {
                return Refuse(format!(
                    "attachment {} has no copy with its journal digest in {} or {}; restore it from a verified backup, then rerun",
                    attachment.name, j.source, j.target
                ));
            }
        }
        if s.source_park.is_some() && s.source_park != j.source_park {
            return Refuse(format!("{}'s park differs from the journal's", j.source));
        }
        if s.source_escalation.is_some() && s.source_escalation != j.source_escalation {
            return Refuse(format!(
                "{}'s escalation differs from the journal's",
                j.source
            ));
        }
        if s.target_park.is_some() && s.target_park != j.target_park {
            return Refuse(format!(
                "{}'s park differs from the one the journal carries",
                j.target
            ));
        }
        if s.target_escalation.is_some() && s.target_escalation != j.target_escalation {
            return Refuse(format!(
                "{}'s escalation differs from the journal's",
                j.target
            ));
        }
    }
    for (j, s) in journal.closed.iter().zip(&snapshot.closed) {
        if s.record != FileState::Digest(j.source_digest.clone()) && s.record != sha(&j.closed_text)
        {
            return Refuse(format!(
                "{} no longer matches the journal or the record the migration would write; it changed during the migration",
                j.id
            ));
        }
        if s.park.is_some() && s.park != j.source_park {
            return Refuse(format!("{}'s park differs from the journal's", j.id));
        }
        if s.escalation.is_some() && s.escalation != j.source_escalation {
            return Refuse(format!("{}'s escalation differs from the journal's", j.id));
        }
    }
    MigrationRecovery::Resume(first_unfinished(journal, snapshot))
}

/// The earliest phase whose effects are not all present. Phases run in order, so every
/// phase before it finished.
fn first_unfinished(journal: &MigrationJournal, snapshot: &Snapshot) -> Phase {
    let carried = || journal.carried.iter().zip(&snapshot.carried);
    if carried().any(|(j, s)| {
        j.attachments
            .iter()
            .any(|a| s.target_files.get(&a.name) != Some(&FileState::Digest(a.sha256.clone())))
    }) {
        return Phase::Attachments;
    }
    if carried().any(|(_, s)| s.target == FileState::Absent) {
        return Phase::Destination;
    }
    if carried().any(|(j, s)| {
        (j.target_park.is_some() && s.target_park.is_none())
            || (j.target_escalation.is_some() && s.target_escalation.is_none())
    }) {
        return Phase::Store;
    }
    if carried().any(|(j, s)| s.source != sha(&j.tombstone_text)) {
        return Phase::Tombstones;
    }
    if journal
        .closed
        .iter()
        .zip(&snapshot.closed)
        .any(|(j, s)| s.record != sha(&j.closed_text))
    {
        return Phase::Closures;
    }
    let source_left = carried().any(|(j, s)| {
        s.source_park.is_some()
            || s.source_escalation.is_some()
            || s.source_dir_empty
            || j.attachments.iter().any(|a| {
                !a.retain_source && s.source_files.get(&a.name) != Some(&FileState::Absent)
            })
    }) || snapshot
        .closed
        .iter()
        .any(|s| s.park.is_some() || s.escalation.is_some());
    if source_left {
        return Phase::Cleanup;
    }
    Phase::Commit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moving::journal::JournalAttachment;

    const SOURCE: &str = "---\nid: forge-00000a\ntitle: A\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\n---\n";

    fn list(text: &str) -> LoadedList {
        LoadedList {
            list: toml::from_str(
                "version = 1\nsource = \"forge\"\nsuccessor = \"forge2\"\ndrafted = \"d\"\n",
            )
            .unwrap(),
            bytes: text.as_bytes().to_vec(),
            sha256: digest(text.as_bytes()),
        }
    }

    fn journal() -> MigrationJournal {
        let dropped_source = SOURCE.replace("forge-00000a", "forge-00000c");
        MigrationJournal {
            source: "forge".into(),
            successor: "forge2".into(),
            source_root: "/s".into(),
            target_root: "/t".into(),
            source_records: "/s".into(),
            target_records: "/t".into(),
            list_path: "/l.toml".into(),
            list_sha256: digest(b"list"),
            list_text: "list".into(),
            parent_targets: vec![],
            unread_projects: vec![],
            release: BTreeMap::new(),
            inbound: BTreeMap::new(),
            woken: BTreeMap::new(),
            carried: vec![super::super::journal::CarriedEntry {
                source: "forge-00000a".into(),
                target: "forge2-00000b".into(),
                source_digest: digest(SOURCE.as_bytes()),
                source_text: SOURCE.into(),
                target_text: "target".into(),
                tombstone_text: "tombstone".into(),
                rewrites: vec![],
                attachments: vec![JournalAttachment {
                    retain_source: false,
                    name: "shot.png".into(),
                    sha256: digest(b"png"),
                }],
                source_park: None,
                source_escalation: None,
                target_park: None,
                target_escalation: None,
            }],
            closed: vec![super::super::journal::ClosedEntry {
                id: "forge-00000c".into(),
                decision: "drop".into(),
                reason: "gone".into(),
                source_digest: digest(dropped_source.as_bytes()),
                source_text: dropped_source,
                closed_text: "dropped".into(),
                open_depends: vec![],
                source_park: None,
                source_escalation: None,
            }],
        }
    }

    fn untouched(j: &MigrationJournal) -> Snapshot {
        let png = FileState::Digest(digest(b"png"));
        Snapshot {
            carried: vec![CarriedState {
                source: FileState::Digest(j.carried[0].source_digest.clone()),
                target: FileState::Absent,
                source_files: [("shot.png".to_string(), png)].into_iter().collect(),
                target_files: [("shot.png".to_string(), FileState::Absent)]
                    .into_iter()
                    .collect(),
                source_dir_empty: false,
                source_park: None,
                source_escalation: None,
                target_park: None,
                target_escalation: None,
            }],
            closed: vec![ClosedState {
                record: FileState::Digest(j.closed[0].source_digest.clone()),
                park: None,
                escalation: None,
            }],
        }
    }

    fn refuses(j: &MigrationJournal, s: &Snapshot, name: &str) {
        match classify_with_journal(j, s, &list("list")) {
            MigrationRecovery::Refuse(reason) => assert!(reason.contains(name), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn each_phase_is_recognised_from_its_effects() {
        let j = journal();
        let mut s = untouched(&j);
        let at = |s: &Snapshot| classify_with_journal(&j, s, &list("list"));
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Attachments));
        let png = FileState::Digest(digest(b"png"));
        s.carried[0].target_files.insert("shot.png".into(), png);
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Destination));
        s.carried[0].target = sha("target");
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Tombstones));
        s.carried[0].source = sha("tombstone");
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Closures));
        s.closed[0].record = sha("dropped");
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Cleanup));
        s.carried[0]
            .source_files
            .insert("shot.png".into(), FileState::Absent);
        s.carried[0].source_dir_empty = true;
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Cleanup));
        s.carried[0].source_dir_empty = false;
        assert_eq!(at(&s), MigrationRecovery::Resume(Phase::Commit));
    }

    #[test]
    fn a_changed_list_or_hand_edit_refuses() {
        let j = journal();
        let s = untouched(&j);
        match classify_with_journal(&j, &s, &list("other")) {
            MigrationRecovery::Refuse(reason) => assert!(reason.contains("list"), "{reason}"),
            other => panic!("{other:?}"),
        }
        let mut edited = s.clone();
        edited.carried[0].source = sha("hand");
        refuses(&j, &edited, "forge-00000a");
        let mut edited = s.clone();
        edited.closed[0].record = sha("hand");
        refuses(&j, &edited, "forge-00000c");
        let mut edited = s.clone();
        edited.carried[0].target = sha("foreign");
        refuses(&j, &edited, "forge2-00000b");
        let mut edited = s;
        edited.carried[0]
            .source_files
            .insert("shot.png".into(), FileState::Absent);
        refuses(&j, &edited, "shot.png");
    }
}
