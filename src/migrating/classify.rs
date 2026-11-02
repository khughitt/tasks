//! Recovery classification (spec §4.5 "Classification"), pure and closed as `move`'s, with
//! two entry paths: `classify_with_journal` decides `Resume` or `Refuse` from a pending
//! run's journal; `classify_without_journal` decides `Fresh`, `Complete` or `Refuse` from
//! the records, the cleanup state and the commits alone (its snapshot is read by
//! `apply::completion_snapshot`).

use super::journal::MigrationJournal;
use super::list::{Decision, LoadedList};
use crate::claims::{Escalation, Park};
use crate::model::{Status, Task};
use crate::moving::classify::FileState;
use crate::rename::inventory::digest;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
    /// No record carries the list's sha256: preflight the list and start a run.
    Fresh,
    /// Continue from this phase: every earlier phase's effects are present.
    Resume(Phase),
    /// The records, the cleanup and the commits all show the end state; nothing to write.
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

/// One record as the journal-less classification reads it: parsed without resolving its
/// archived note segments, so `notes` holds the inline notes only (every migration note is
/// appended inline) and `note_archives` the descriptors.
#[derive(Debug, Clone)]
pub enum RecordRead {
    Absent,
    /// Present but not a record of this id: the parse error or mismatch.
    Unreadable(String),
    Present(Box<Task>),
}

/// What is on disk for one entry of the decision list, read without a journal.
#[derive(Debug, Clone)]
pub struct ListedState {
    /// The listed id, canonical when it parses; as written otherwise.
    pub id: String,
    /// `None` for an entry without one of the three decisions.
    pub decision: Option<Decision>,
    /// The entry's reason text, `[reasons]` keys resolved (empty for a carry).
    pub reason: String,
    pub record: RecordRead,
    /// The successor record the tombstone's `moved_to` names, when it names one.
    pub target: Option<RecordRead>,
    /// The source state of every attachment the tombstone marks moved out
    /// (`moved_out`) or retains as a note-archive segment (`retained`).
    pub source_files: BTreeMap<String, FileState>,
    /// This host's source store holds a park or an escalation keyed by the listed id.
    pub park: bool,
    pub escalation: bool,
}

/// Everything `classify_without_journal` reads, gathered by `apply::completion_snapshot`.
#[derive(Debug, Clone)]
pub struct CompletionSnapshot {
    pub source: String,
    pub successor: String,
    /// The source records directory, for naming attachment paths.
    pub source_records: PathBuf,
    /// Parallel to the list's `[[task]]` entries.
    pub listed: Vec<ListedState>,
    /// Successor records whose notes carry the list's sha256, by id.
    pub successor_carriers: Vec<String>,
    /// Successor records whose text holds the list's sha256 suffix but that do not parse
    /// as their id's record: `(id, detail)`. Counted as carriers, so never `Fresh`.
    pub successor_unreadable: Vec<(String, String)>,
    /// The run's owned paths (derived from the list and the records) that are not clean,
    /// under records directories that set `[git] commit`.
    pub uncommitted: Vec<PathBuf>,
}

/// ` list: sha256:<hex>`, the suffix of every migration note of this list (spec §4.4).
pub fn note_suffix(list: &LoadedList) -> String {
    format!(" list: sha256:{}", list.sha256)
}

/// Some note of `task` carries the list's sha256.
pub fn carries(task: &Task, suffix: &str) -> bool {
    task.notes.iter().any(|note| note.text.ends_with(suffix))
}

/// The attachments a tombstone's ledger marks `detached: <name>: moved to <new id>`.
pub fn moved_out(task: &Task) -> Vec<String> {
    let Some(to) = &task.moved_to else {
        return Vec::new();
    };
    let tail = format!(": moved to {to}");
    let names: BTreeSet<String> = task
        .notes
        .iter()
        .filter_map(|note| {
            let name = note.text.strip_prefix("detached: ")?.strip_suffix(&tail)?;
            crate::attachments::validate_name(name).ok()?;
            Some(name.to_string())
        })
        .collect();
    names.into_iter().collect()
}

/// The note-archive segment attachments a record's `note_archives` names.
pub fn retained(task: &Task) -> Vec<String> {
    task.note_archives
        .iter()
        .map(|archive| archive.file_name())
        .collect()
}

/// The dependents a drop or close note released and the dependencies it left open, read
/// from the note `<verb> in migration to <succ>: <reason>[; released: …][; still open: …]`
/// followed by `suffix`. `None` when `task` has no such note.
pub fn closing_note(
    task: &Task,
    decision: Decision,
    successor: &str,
    reason: &str,
    suffix: &str,
) -> Option<(Vec<String>, Vec<String>)> {
    let verb = match decision {
        Decision::Drop => "dropped",
        Decision::Close => "closed",
        Decision::Carry => return None,
    };
    let head = format!("{verb} in migration to {successor}: {reason}");
    let ids = |text: &str| -> Vec<String> { text.split(", ").map(String::from).collect() };
    task.notes.iter().rev().find_map(|note| {
        let middle = note.text.strip_prefix(&head)?.strip_suffix(suffix)?;
        let (released, open) = match middle.split_once("; still open: ") {
            Some((released, open)) => (released, ids(open)),
            None => (middle, Vec::new()),
        };
        let released = match released {
            "" => Vec::new(),
            text => ids(text.strip_prefix("; released: ")?),
        };
        Some((released, open))
    })
}

/// Spec §4.5, without a journal: `Complete` only when every listed record shows its
/// decision's end state with this list's sha256, every carried attachment the tombstone
/// moved out is gone from the source while every note-archive segment it names is present
/// with its descriptor's digest, no listed id has a park or escalation in the source store,
/// and no owned path is uncommitted where `[git] commit` is set; `Fresh` when no record
/// carries the sha256; otherwise `Refuse`, naming every unfinished item.
pub fn classify_without_journal(
    list: &LoadedList,
    snapshot: &CompletionSnapshot,
) -> MigrationRecovery {
    let suffix = note_suffix(list);
    let mut carrying = Vec::new();
    let mut not_carrying = Vec::new();
    for listed in &snapshot.listed {
        match &listed.record {
            RecordRead::Present(task) if carries(task, &suffix) => {
                carrying.push(listed.id.clone());
            }
            RecordRead::Present(_) => not_carrying.push(listed.id.clone()),
            RecordRead::Absent => not_carrying.push(format!("{} (missing)", listed.id)),
            RecordRead::Unreadable(detail) => {
                not_carrying.push(format!("{} (unreadable: {detail})", listed.id));
            }
        }
    }
    carrying.extend(snapshot.successor_carriers.iter().cloned());
    carrying.extend(
        snapshot
            .successor_unreadable
            .iter()
            .map(|(id, detail)| format!("{id} (unreadable: {detail})")),
    );
    if carrying.is_empty() {
        return MigrationRecovery::Fresh;
    }
    if !not_carrying.is_empty() {
        return unfinished(
            list,
            vec![format!(
                "records carrying this list's sha256: {}; records not carrying it: {}",
                carrying.join(", "),
                not_carrying.join(", ")
            )],
        );
    }
    let mut items = Vec::new();
    let mut targets = BTreeSet::new();
    let source_file =
        |id: &str, name: &str| super::attachment_file(&snapshot.source_records, id, name);
    for listed in &snapshot.listed {
        let RecordRead::Present(task) = &listed.record else {
            unreachable!("every listed record carries the sha256 here");
        };
        let id = &listed.id;
        match listed.decision {
            Some(Decision::Carry) => {
                let Some(to) = task
                    .moved_to
                    .as_ref()
                    .filter(|_| task.status == Status::Moved)
                else {
                    items.push(format!(
                        "{id} carries this list's sha256 but is {}, not a tombstone",
                        task.status.as_str()
                    ));
                    continue;
                };
                if !task
                    .notes
                    .iter()
                    .any(|note| note.text == format!("moved to {to}{suffix}"))
                {
                    items.push(format!(
                        "{id}'s tombstone does not note `moved to {to}` with this list's sha256"
                    ));
                }
                targets.insert(to.to_string());
                match &listed.target {
                    Some(RecordRead::Present(target))
                        if target.moved_from.as_ref().map(ToString::to_string).as_ref()
                            == Some(id)
                            && target
                                .notes
                                .iter()
                                .any(|note| note.text == format!("moved from {id}{suffix}")) => {}
                    Some(RecordRead::Present(_)) => items.push(format!(
                        "{to} does not point back to {id} with this list's sha256"
                    )),
                    Some(RecordRead::Absent) | None => items.push(format!(
                        "{id}'s tombstone points to {to}, which is not a {} record",
                        snapshot.successor
                    )),
                    Some(RecordRead::Unreadable(detail)) => {
                        items.push(format!("{to} is unreadable: {detail}"));
                    }
                }
                for name in moved_out(task) {
                    if listed.source_files.get(&name) != Some(&FileState::Absent) {
                        items.push(format!(
                            "attachment {} is still in the source (moved to {to})",
                            source_file(id, &name).display()
                        ));
                    }
                }
                for archive in &task.note_archives {
                    let name = archive.file_name();
                    if listed.source_files.get(&name)
                        != Some(&FileState::Digest(archive.sha256.clone()))
                    {
                        items.push(format!(
                            "note-archive segment {} is missing or differs from its descriptor's digest; restore it from a verified copy",
                            source_file(id, &name).display()
                        ));
                    }
                }
            }
            Some(decision @ (Decision::Drop | Decision::Close)) => {
                let (want, verb) = if decision == Decision::Drop {
                    (Status::Dropped, "drop")
                } else {
                    (Status::Done, "close")
                };
                if task.status != want
                    || closing_note(task, decision, &snapshot.successor, &listed.reason, &suffix)
                        .is_none()
                {
                    items.push(format!(
                        "{id} carries this list's sha256 but is {} without the {verb} note the list decides",
                        task.status.as_str()
                    ));
                }
            }
            None => items.push(format!(
                "{id} carries this list's sha256 but its list entry decides nothing"
            )),
        }
        for (held, kind) in [(listed.park, "park"), (listed.escalation, "escalation")] {
            if held {
                items.push(format!("{id} {kind} in {}'s store", snapshot.source));
            }
        }
    }
    for carrier in &snapshot.successor_carriers {
        if !targets.contains(carrier) {
            items.push(format!(
                "{carrier} carries this list's sha256 but no tombstone points to it"
            ));
        }
    }
    for (id, detail) in &snapshot.successor_unreadable {
        items.push(format!(
            "{id} names this list's sha256 but is unreadable: {detail}"
        ));
    }
    for path in &snapshot.uncommitted {
        items.push(format!("uncommitted {}", path.display()));
    }
    if items.is_empty() {
        MigrationRecovery::Complete
    } else {
        unfinished(list, items)
    }
}

fn unfinished(list: &LoadedList, items: Vec<String>) -> MigrationRecovery {
    MigrationRecovery::Refuse(format!(
        "a migration by this list (sha256:{}) is unfinished and has no journal on this host (lost mid-run, or run on another host and not fully synced): {}; finish it by hand, item by item: remove a source attachment only when the tombstone marks it `detached: <name>: moved to <new id>` and the destination copy has the same sha256, and never remove a note-archive segment; for a carried task's source store entry, write the entry re-keyed to the new id when the destination has none, remove the source entry when the destination's equals it, and change neither store when they differ until you have reconciled them; remove a dropped or closed task's source store entry; commit uncommitted paths by pathspec; then rerun the same `tasks migrate`",
        list.sha256,
        items.join("; ")
    ))
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

    fn task(id: &str, status: &str, notes: &[&str]) -> Task {
        let text = SOURCE
            .replace("forge-00000a", id)
            .replace("status: todo", &format!("status: {status}"));
        let physical = crate::format::parse_physical(&text, "t").unwrap();
        let mut task = physical.assemble(Vec::new());
        for note in notes {
            crate::commands::append_note(&mut task, "o", note).unwrap();
        }
        task
    }

    fn closed_state(id: &str, decision: Decision, record: RecordRead) -> ListedState {
        ListedState {
            id: id.into(),
            decision: Some(decision),
            reason: "gone".into(),
            record,
            target: None,
            source_files: BTreeMap::new(),
            park: false,
            escalation: false,
        }
    }

    fn completion(listed: Vec<ListedState>) -> CompletionSnapshot {
        CompletionSnapshot {
            source: "forge".into(),
            successor: "forge2".into(),
            source_records: "/s".into(),
            listed,
            successor_carriers: vec![],
            successor_unreadable: vec![],
            uncommitted: vec![],
        }
    }

    #[test]
    fn closing_notes_read_released_and_still_open_ids() {
        let suffix = " list: sha256:00";
        let note = |text: &str| task("forge-00000c", "done", &[text]);
        let read = |t: &Task| closing_note(t, Decision::Close, "forge2", "gone; x", suffix);
        assert_eq!(
            read(&note(
                "closed in migration to forge2: gone; x list: sha256:00"
            )),
            Some((vec![], vec![]))
        );
        assert_eq!(
            read(&note(
                "closed in migration to forge2: gone; x; released: ops-00000a, ops-00000b; still open: ops-00000c list: sha256:00"
            )),
            Some((
                vec!["ops-00000a".to_string(), "ops-00000b".to_string()],
                vec!["ops-00000c".to_string()]
            ))
        );
        assert_eq!(
            read(&note(
                "closed in migration to forge2: gone; x list: sha256:01"
            )),
            None
        );
        assert_eq!(
            read(&note(
                "closed in migration to forge2: gone; x; extra list: sha256:00"
            )),
            None
        );
    }

    #[test]
    fn without_a_journal_the_records_decide() {
        let l = list("list");
        let suffix = note_suffix(&l);
        let dropped = format!("dropped in migration to forge2: gone{suffix}");
        let fresh = completion(vec![closed_state(
            "forge-00000c",
            Decision::Drop,
            RecordRead::Present(Box::new(task("forge-00000c", "todo", &[]))),
        )]);
        assert_eq!(
            classify_without_journal(&l, &fresh),
            MigrationRecovery::Fresh
        );
        let done = completion(vec![closed_state(
            "forge-00000c",
            Decision::Drop,
            RecordRead::Present(Box::new(task("forge-00000c", "dropped", &[&dropped]))),
        )]);
        assert_eq!(
            classify_without_journal(&l, &done),
            MigrationRecovery::Complete
        );
        // The sha256 with the wrong outcome.
        let wrong = completion(vec![closed_state(
            "forge-00000c",
            Decision::Close,
            RecordRead::Present(Box::new(task("forge-00000c", "dropped", &[&dropped]))),
        )]);
        match classify_without_journal(&l, &wrong) {
            MigrationRecovery::Refuse(reason) => {
                assert!(reason.contains("forge-00000c carries"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        // A store entry or an uncommitted path keeps a final record set unfinished.
        let mut parked = done.clone();
        parked.listed[0].escalation = true;
        let mut dirty = done.clone();
        dirty.uncommitted.push("/s/tasks/forge-00000c.md".into());
        for (snapshot, name) in [
            (parked, "forge-00000c escalation"),
            (dirty, "uncommitted /s/tasks/forge-00000c.md"),
        ] {
            match classify_without_journal(&l, &snapshot) {
                MigrationRecovery::Refuse(reason) => assert!(reason.contains(name), "{reason}"),
                other => panic!("{other:?}"),
            }
        }
        // A successor record no tombstone points to.
        let mut stray = done;
        stray.successor_carriers.push("forge2-00000b".into());
        match classify_without_journal(&l, &stray) {
            MigrationRecovery::Refuse(reason) => {
                assert!(reason.contains("forge2-00000b carries"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn without_a_journal_a_carry_needs_both_halves_of_the_pair() {
        let l = list("list");
        let suffix = note_suffix(&l);
        let tombstone = |notes: &[&str]| {
            let mut t = task("forge-00000a", "todo", notes);
            t.status = Status::Moved;
            t.moved_to = Some(crate::model::TaskId::parse("forge2-00000b").unwrap());
            t
        };
        let new_record = |from: Option<&str>, notes: &[&str]| {
            let mut t = task("forge2-00000b", "todo", notes);
            t.moved_from = from.map(|id| crate::model::TaskId::parse(id).unwrap());
            RecordRead::Present(Box::new(t))
        };
        let moved_to = format!("moved to forge2-00000b{suffix}");
        let moved_from = format!("moved from forge-00000a{suffix}");
        let carry = |record: Task, target: Option<RecordRead>| {
            let mut state = closed_state(
                "forge-00000a",
                Decision::Carry,
                RecordRead::Present(Box::new(record)),
            );
            state.reason = String::new();
            state.target = target;
            let mut snapshot = completion(vec![state]);
            snapshot.successor_carriers = vec!["forge2-00000b".into()];
            classify_without_journal(&l, &snapshot)
        };
        assert_eq!(
            carry(
                tombstone(&[&moved_to]),
                Some(new_record(Some("forge-00000a"), &[&moved_from]))
            ),
            MigrationRecovery::Complete
        );
        let refuses = |verdict: MigrationRecovery, name: &str| match verdict {
            MigrationRecovery::Refuse(reason) => assert!(reason.contains(name), "{reason}"),
            other => panic!("{other:?}"),
        };
        // Not a tombstone.
        refuses(
            carry(
                task("forge-00000a", "todo", &[&moved_to]),
                Some(new_record(Some("forge-00000a"), &[&moved_from])),
            ),
            "forge-00000a carries this list's sha256 but is todo, not a tombstone",
        );
        // The tombstone's own note is missing (another note carries the sha256).
        refuses(
            carry(
                tombstone(&[&format!("other{suffix}")]),
                Some(new_record(Some("forge-00000a"), &[&moved_from])),
            ),
            "does not note `moved to forge2-00000b`",
        );
        // The new record does not point back.
        refuses(
            carry(
                tombstone(&[&moved_to]),
                Some(new_record(None, &[&moved_from])),
            ),
            "forge2-00000b does not point back to forge-00000a",
        );
        // The new record is absent.
        refuses(
            carry(tombstone(&[&moved_to]), Some(RecordRead::Absent)),
            "points to forge2-00000b, which is not a forge2 record",
        );
        // An unreadable successor record naming the list is never fresh.
        let mut snapshot = completion(vec![closed_state(
            "forge-00000c",
            Decision::Drop,
            RecordRead::Present(Box::new(task("forge-00000c", "todo", &[]))),
        )]);
        snapshot.successor_unreadable = vec![("forge2-00000b".into(), "bad".into())];
        refuses(
            classify_without_journal(&l, &snapshot),
            "forge2-00000b (unreadable: bad)",
        );
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
