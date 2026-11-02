//! The apply phases G2-G10 (spec §4.5). Each phase is idempotent as the protocol table
//! says, so a resume from the phase the classifier names writes the same bytes. Nothing is
//! committed before G9, which commits every path the run owns (one commit per git working
//! tree) and checks that each is clean; G10 removes the journal only after that.

use super::classify::{
    CarriedState, ClosedState, CompletionSnapshot, ListedState, Phase, RecordRead, Snapshot,
    carries, closing_note, moved_out, note_suffix, retained,
};
use super::journal::MigrationJournal;
use super::list::{Decision, LoadedList};
use super::{MigrateCtx, attachment_file, record_path};
use crate::attachments;
use crate::claims::ClaimStore;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::moving::classify::FileState;
use crate::moving::{attachment_state, record_state};
use crate::output::{CarriedOut, ClosedOut, DroppedOut, InboundOut, MigrateOut, WokenOut};
use crate::rename::inventory::digest;
use crate::repo::{Project, atomic_write};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

fn id(text: &str) -> TaskId {
    TaskId::parse(text).expect("validated journal id")
}

fn want(text: &str) -> FileState {
    FileState::Digest(digest(text.as_bytes()))
}

/// The journal belongs to this run's projects as they are registered now: the same
/// successor, roots and records directories, and parks re-keyed to the successor's resume
/// root. Anything else refuses before a snapshot is read.
pub fn check_identity(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    if journal.source != ctx.source.project.prefix || journal.successor != ctx.target.prefix {
        return Err(Error::MovePending(format!(
            "the pending migration is of {} to {}, not {} to {}; finish it with `tasks migrate {}`",
            journal.source,
            journal.successor,
            ctx.source.project.prefix,
            ctx.target.prefix,
            crate::stale::quote(&journal.list_path.display().to_string())
        )));
    }
    for (saved_root, saved_records, project) in [
        (
            &journal.source_root,
            &journal.source_records,
            &ctx.source.project,
        ),
        (&journal.target_root, &journal.target_records, &ctx.target),
    ] {
        if crate::rename::root_identity(saved_root)? != crate::rename::root_identity(&project.root)?
            || crate::rename::root_identity(saved_records)?
                != crate::rename::root_identity(&project.records)?
        {
            return Err(Error::MovePending(format!(
                "{} is registered at {} (records {}), not {} (records {}); restore its registration before resuming",
                project.prefix,
                project.root.display(),
                project.records.display(),
                saved_root.display(),
                saved_records.display()
            )));
        }
    }
    let resume_root = ctx.target.resume_root().display().to_string();
    if journal.carried.iter().any(|entry| {
        entry
            .target_park
            .as_ref()
            .is_some_and(|park| park.worktree != resume_root)
    }) {
        return Err(Error::MovePending(
            "a journal target park does not name the successor's resume root".into(),
        ));
    }
    Ok(())
}

/// What is on disk for every entry of the journal.
pub fn snapshot(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<Snapshot> {
    let source = &ctx.source.project;
    let source_store = ClaimStore::load(&source.prefix)?;
    let target_store = ClaimStore::load(&ctx.target.prefix)?;
    let mut carried = Vec::new();
    for entry in &journal.carried {
        let (from, to) = (id(&entry.source), id(&entry.target));
        let mut source_files = BTreeMap::new();
        let mut target_files = BTreeMap::new();
        for attachment in &entry.attachments {
            source_files.insert(
                attachment.name.clone(),
                attachment_state(source, &from, &attachment.name)?,
            );
            target_files.insert(
                attachment.name.clone(),
                attachment_state(&ctx.target, &to, &attachment.name)?,
            );
        }
        let source_dir_empty = attachments::check_storage(source, &from)?
            && std::fs::read_dir(attachments::task_dir(source, &from))?
                .next()
                .is_none();
        carried.push(CarriedState {
            source: record_state(&source.task_path(&from))?,
            target: record_state(&ctx.target.task_path(&to))?,
            source_files,
            target_files,
            source_dir_empty,
            source_park: source_store.park(&from).cloned(),
            source_escalation: source_store.escalation(&from).cloned(),
            target_park: target_store.park(&to).cloned(),
            target_escalation: target_store.escalation(&to).cloned(),
        });
    }
    let mut closed = Vec::new();
    for entry in &journal.closed {
        let listed = id(&entry.id);
        closed.push(ClosedState {
            record: record_state(&source.task_path(&listed))?,
            park: source_store.park(&listed).cloned(),
            escalation: source_store.escalation(&listed).cloned(),
        });
    }
    Ok(Snapshot { carried, closed })
}

/// `id`'s record read for the journal-less classification: parsed, but with its archived
/// note segments left unresolved (their presence is judged per attachment).
fn read_record(project: &Project, id: &TaskId) -> Result<RecordRead> {
    let raw = match std::fs::read_to_string(project.task_path(id)) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RecordRead::Absent);
        }
        Err(error) => return Err(error.into()),
    };
    let physical = match crate::format::parse_physical(&raw, &format!("tasks/{id}.md")) {
        Ok(physical) => physical,
        Err(error) => return Ok(RecordRead::Unreadable(error.to_string())),
    };
    if physical.id() != id {
        return Ok(RecordRead::Unreadable(format!(
            "its id field is {}",
            physical.id()
        )));
    }
    let inline = physical.inline_notes.clone();
    Ok(RecordRead::Present(Box::new(physical.assemble(inline))))
}

/// The paths a run of this list owns, derived without its journal (spec §4.5 "Commits"):
/// every listed record, and for each carried tombstone its new record and, on both sides,
/// each attachment file it moved out or retains. Same layout as
/// `MigrationJournal::owned_paths`.
fn completion_paths(
    ctx: &MigrateCtx,
    listed: &[ListedState],
) -> BTreeMap<PathBuf, BTreeSet<PathBuf>> {
    let (source, target) = (&ctx.source.project.records, &ctx.target.records);
    let mut owned: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    for state in listed {
        if TaskId::parse(&state.id).is_err() {
            continue;
        }
        owned
            .entry(source.clone())
            .or_default()
            .insert(record_path(source, &state.id));
        let RecordRead::Present(task) = &state.record else {
            continue;
        };
        let Some(to) = &task.moved_to else {
            continue;
        };
        let to = to.to_string();
        let names: BTreeSet<String> = moved_out(task).into_iter().chain(retained(task)).collect();
        for name in &names {
            owned
                .entry(source.clone())
                .or_default()
                .insert(attachment_file(source, &state.id, name));
        }
        let at_target = owned.entry(target.clone()).or_default();
        at_target.insert(record_path(target, &to));
        for name in &names {
            at_target.insert(attachment_file(target, &to, name));
        }
    }
    owned
}

/// What `classify_without_journal` reads (spec §4.5, without a journal): each listed
/// record and the successor record its tombstone names, the source state of every
/// attachment the tombstone moved out or retains, this host's source store entries, the
/// successor records that carry the list's sha256, and, once any record carries it, the
/// owned paths left uncommitted.
pub fn completion_snapshot(ctx: &MigrateCtx, list: &LoadedList) -> Result<CompletionSnapshot> {
    let source = &ctx.source.project;
    let store = ClaimStore::load(&source.prefix)?;
    let suffix = note_suffix(list);
    let mut listed = Vec::new();
    for entry in &list.list.tasks {
        let decision = Decision::parse(&entry.decision);
        let reason =
            super::project::reason_text(&list.list, entry.reason.as_deref().unwrap_or("").trim())
                .to_string();
        let Ok(parsed) = TaskId::parse(entry.id.trim()) else {
            listed.push(ListedState {
                id: entry.id.clone(),
                decision,
                reason,
                record: RecordRead::Unreadable("not a task id".into()),
                target: None,
                source_files: BTreeMap::new(),
                park: false,
                escalation: false,
            });
            continue;
        };
        let listed_id = ctx.registry.canonical_id(&parsed);
        let record = read_record(source, &listed_id)?;
        let mut target = None;
        let mut source_files = BTreeMap::new();
        if let RecordRead::Present(task) = &record
            && let Some(to) = &task.moved_to
        {
            let to = ctx.registry.canonical_id(to);
            target = Some(if to.prefix == ctx.target.prefix {
                read_record(&ctx.target, &to)?
            } else {
                RecordRead::Absent
            });
            for name in moved_out(task).into_iter().chain(retained(task)) {
                let state = attachment_state(source, &listed_id, &name)?;
                source_files.insert(name, state);
            }
        }
        listed.push(ListedState {
            id: listed_id.to_string(),
            decision,
            reason,
            record,
            target,
            source_files,
            park: store.park(&listed_id).is_some(),
            escalation: store.escalation(&listed_id).is_some(),
        });
    }
    let mut successor_carriers = Vec::new();
    let mut successor_unreadable = Vec::new();
    for id in ctx.target.task_files()? {
        let raw = std::fs::read_to_string(ctx.target.task_path(&id))?;
        if !raw.contains(&suffix) {
            continue;
        }
        match read_record(&ctx.target, &id)? {
            RecordRead::Present(task) if carries(&task, &suffix) => {
                successor_carriers.push(id.to_string());
            }
            RecordRead::Present(_) | RecordRead::Absent => {}
            // Its text names this list: evidence of a lost run, never a fresh one.
            RecordRead::Unreadable(detail) => successor_unreadable.push((id.to_string(), detail)),
        }
    }
    let any_carries = !successor_carriers.is_empty()
        || !successor_unreadable.is_empty()
        || listed.iter().any(
            |state| matches!(&state.record, RecordRead::Present(task) if carries(task, &suffix)),
        );
    let uncommitted = if any_carries {
        super::commit::uncommitted(&completion_paths(ctx, &listed))?
    } else {
        Vec::new()
    };
    Ok(CompletionSnapshot {
        source: source.prefix.clone(),
        successor: ctx.target.prefix.clone(),
        source_records: source.records.clone(),
        listed,
        successor_carriers,
        successor_unreadable,
        uncommitted,
    })
}

/// The report of a run the journal-less classification found `complete` (spec §4.7),
/// rebuilt from the records: new ids from the tombstones, parents and rewrites from the new
/// records, reasons from the list, released and still-open ids from the drop and close
/// notes. `park` says whether the successor's store holds a park for the new id now; the
/// pairs' inbound and woken dependents are not recorded and are left empty.
pub fn completed_output(
    ctx: &MigrateCtx,
    list: &LoadedList,
    snapshot: &CompletionSnapshot,
) -> Result<MigrateOut> {
    let suffix = note_suffix(list);
    let target_store = ClaimStore::load(&ctx.target.prefix)?;
    let mut carried = Vec::new();
    let mut dropped = Vec::new();
    let mut closed = Vec::new();
    for state in &snapshot.listed {
        let RecordRead::Present(task) = &state.record else {
            unreachable!("a complete run has every listed record");
        };
        match state.decision {
            Some(Decision::Carry) => {
                let (Some(to), Some(RecordRead::Present(target))) = (&task.moved_to, &state.target)
                else {
                    unreachable!("a complete carry has its tombstone and new record");
                };
                let mut attachments: Vec<String> =
                    moved_out(task).into_iter().chain(retained(task)).collect();
                attachments.sort();
                attachments.dedup();
                carried.push(CarriedOut {
                    from: state.id.clone(),
                    id: Some(to.to_string()),
                    parent: target.parent.as_ref().map(ToString::to_string),
                    rewrites: target
                        .notes
                        .iter()
                        .filter_map(|note| {
                            note.text
                                .strip_prefix("migration: ")?
                                .strip_suffix(&suffix)
                                .map(String::from)
                        })
                        .collect(),
                    attachments,
                    park: target_store.park(to).is_some(),
                });
            }
            Some(decision) => {
                let (released, open_depends) =
                    closing_note(task, decision, &snapshot.successor, &state.reason, &suffix)
                        .expect("a complete drop or close has its note");
                if decision == Decision::Drop {
                    dropped.push(DroppedOut {
                        id: state.id.clone(),
                        reason: state.reason.clone(),
                        released,
                    });
                } else {
                    closed.push(ClosedOut {
                        id: state.id.clone(),
                        reason: state.reason.clone(),
                        open_depends,
                        released,
                    });
                }
            }
            None => unreachable!("a complete run decided every entry"),
        }
    }
    let paths = shown_paths(completion_paths(ctx, &snapshot.listed));
    Ok(MigrateOut {
        source: snapshot.source.clone(),
        successor: snapshot.successor.clone(),
        list: format!("sha256:{}", list.sha256),
        recovery: "complete".into(),
        carried,
        dropped,
        closed,
        inbound: Vec::new(),
        woken: Vec::new(),
        unread_projects: Vec::new(),
        paths,
        warnings: ctx.source.warnings.clone(),
        refusals: Vec::new(),
        body_changes: Vec::new(),
        titles: Default::default(),
    })
}

/// The report's `paths`: each owned path relative to its records directory.
fn shown_paths(owned: BTreeMap<PathBuf, BTreeSet<PathBuf>>) -> BTreeMap<String, Vec<String>> {
    owned
        .into_iter()
        .map(|(records, paths)| {
            let shown = paths
                .iter()
                .map(|path| {
                    path.strip_prefix(&records)
                        .expect("owned paths lie under their records directory")
                        .display()
                        .to_string()
                })
                .collect();
            (records.display().to_string(), shown)
        })
        .collect()
}

/// The run's report (spec §4.7), from the journal alone, so a fresh run, a resume and an
/// `--explain` of a pending run report the same rows.
pub fn output(ctx: &MigrateCtx, journal: &MigrationJournal, recovery: &str) -> Result<MigrateOut> {
    let mut carried = Vec::new();
    for entry in &journal.carried {
        let target = crate::format::parse_physical(&entry.target_text, &entry.target)?;
        carried.push(CarriedOut {
            from: entry.source.clone(),
            id: Some(entry.target.clone()),
            parent: target.parent().map(ToString::to_string),
            rewrites: entry.rewrites.clone(),
            attachments: entry.attachments.iter().map(|a| a.name.clone()).collect(),
            park: entry.source_park.is_some(),
        });
    }
    let mut dropped = Vec::new();
    let mut closed = Vec::new();
    for entry in &journal.closed {
        let released = journal.release.get(&entry.id).cloned().unwrap_or_default();
        match entry.decision() {
            Decision::Drop => dropped.push(DroppedOut {
                id: entry.id.clone(),
                reason: entry.reason.clone(),
                released,
            }),
            Decision::Close => closed.push(ClosedOut {
                id: entry.id.clone(),
                reason: entry.reason.clone(),
                open_depends: entry.open_depends.clone(),
                released,
            }),
            Decision::Carry => unreachable!("validated journal decision"),
        }
    }
    let paths = shown_paths(journal.owned_paths());
    Ok(MigrateOut {
        source: journal.source.clone(),
        successor: journal.successor.clone(),
        list: format!("sha256:{}", journal.list_sha256),
        recovery: recovery.into(),
        carried,
        dropped,
        closed,
        inbound: journal
            .inbound
            .iter()
            .map(|(id, dependents)| InboundOut {
                id: id.clone(),
                dependents: dependents.clone(),
            })
            .collect(),
        woken: journal
            .woken
            .iter()
            .map(|(id, shelves)| WokenOut {
                id: id.clone(),
                shelves: shelves.clone(),
            })
            .collect(),
        unread_projects: journal.unread_projects.clone(),
        paths,
        warnings: ctx.source.warnings.clone(),
        refusals: Vec::new(),
        body_changes: Vec::new(),
        titles: Default::default(),
    })
}

/// `TASKS_MIGRATE_STOP_AFTER=<phase>` stops after `phase` (tests). What the phases wrote is
/// left uncommitted, as an interruption leaves it: G9 commits it on the rerun.
fn stop_after(phase: Phase) -> bool {
    let stop = std::env::var("TASKS_MIGRATE_STOP_AFTER").is_ok_and(|stop| stop == phase.name());
    if stop {
        crate::written::begin(Some("migrate"));
    }
    stop
}

/// `TASKS_MIGRATE_PAUSE_BEFORE_JOURNAL=<path>` (tests): a fresh run creates `path` after
/// preflight and waits, up to a minute, for it to be removed before writing the journal,
/// so a test can write to a project the run does not lock in the window between
/// preflight's reads and G2.
fn pause_before_journal() -> Result<()> {
    let Some(marker) = std::env::var_os("TASKS_MIGRATE_PAUSE_BEFORE_JOURNAL") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    std::fs::write(&marker, b"")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while marker.exists() {
        if std::time::Instant::now() > deadline {
            return Err(Error::Io(format!(
                "{} was not removed within a minute",
                marker.display()
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}

/// Executes G2-G10 from `from` (`Phase::Journal` for a fresh run).
pub fn run(ctx: &mut MigrateCtx, journal: MigrationJournal, from: Phase) -> Result<MigrateOut> {
    let recovery = if from == Phase::Journal {
        "fresh"
    } else {
        // Spec §4.5: before any resumed phase, the inputs outside the journal.
        let refusals = super::freeze::revalidate(ctx, &journal, from)?;
        if !refusals.is_empty() {
            return Err(super::freeze::refusal_error(&journal, &refusals, false));
        }
        "resumed"
    };
    let mut out = output(ctx, &journal, recovery)?;
    let phases = [
        Phase::Journal,
        Phase::Attachments,
        Phase::Destination,
        Phase::Store,
        Phase::Tombstones,
        Phase::Closures,
        Phase::Cleanup,
        Phase::Commit,
    ];
    for phase in phases.into_iter().filter(|phase| *phase >= from) {
        match phase {
            Phase::Journal => {
                pause_before_journal()?;
                journal.save()?;
                // A write in a project this run does not lock (or another host's synced
                // write) may have landed between preflight's reads and the journal, which
                // starts the freeze: judge the same inputs a resume judges. The journal
                // stays, so the freeze admits the restoring commands the refusal names.
                let refusals = super::freeze::revalidate(ctx, &journal, Phase::Journal)?;
                if !refusals.is_empty() {
                    return Err(super::freeze::refusal_error(&journal, &refusals, true));
                }
            }
            Phase::Attachments => copy_attachments(ctx, &journal)?,
            Phase::Destination => create_destinations(ctx, &journal)?,
            Phase::Store => write_destination_store(ctx, &journal)?,
            Phase::Tombstones => {
                for entry in &journal.carried {
                    replace(
                        &ctx.source.project,
                        &entry.source,
                        &entry.source_digest,
                        &entry.tombstone_text,
                    )?;
                }
            }
            Phase::Closures => {
                for entry in &journal.closed {
                    replace(
                        &ctx.source.project,
                        &entry.id,
                        &entry.source_digest,
                        &entry.closed_text,
                    )?;
                }
            }
            Phase::Cleanup => cleanup(ctx, &journal)?,
            Phase::Commit => commit(&journal, &mut out.warnings)?,
        }
        if stop_after(phase) {
            return Ok(out);
        }
    }
    audit(ctx, &journal)?;
    journal.remove()?;
    Ok(out)
}

/// G3: every carried attachment copied to its new id, verified by digest.
fn copy_attachments(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    for entry in &journal.carried {
        crate::moving::copy_attachments(
            &ctx.source.project,
            &id(&entry.source),
            &ctx.target,
            &id(&entry.target),
            &entry.attachments,
        )?;
    }
    Ok(())
}

/// G4: each new record created exclusively from its journaled text. The projection
/// validated the whole set; a present file with that digest is skipped.
fn create_destinations(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    for entry in &journal.carried {
        let target = id(&entry.target);
        let path = ctx.target.task_path(&target);
        if record_state(&path)? == FileState::Absent {
            create_exact(&ctx.target, &target, &entry.target_text)?;
        }
        if record_state(&path)? != want(&entry.target_text) {
            return Err(Error::MovePending(format!(
                "{target} differs from its journal text"
            )));
        }
    }
    Ok(())
}

/// Links `text` into place as `id`'s record without ever replacing a file there.
fn create_exact(project: &Project, id: &TaskId, text: &str) -> Result<()> {
    let path = project.task_path(id);
    for _ in 0..16 {
        let temp = project
            .tasks_dir()
            .join(format!(".{id}.{:06x}.new.tmp", fastrand::u32(..0x100_0000)));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let written = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        let linked = written.and_then(|()| std::fs::hard_link(&temp, &path));
        let _ = std::fs::remove_file(&temp);
        return match linked {
            Ok(()) => Ok(()),
            // A concurrent writer's file: the digest check that follows refuses it.
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    Err(Error::Io(format!(
        "could not allocate a creation temp for {id} after 16 attempts"
    )))
}

/// G5: the carried parks and escalations, re-keyed, in the successor's store.
fn write_destination_store(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    let mut store = ClaimStore::load(&ctx.target.prefix)?;
    let mut changed = false;
    for entry in &journal.carried {
        let target = id(&entry.target);
        if let Some(park) = &entry.target_park {
            match store.park(&target) {
                None => {
                    store.insert_park(&target, park.clone());
                    changed = true;
                }
                Some(present) if present != park => {
                    return Err(Error::MovePending(format!(
                        "{target}'s park differs from the one the journal carries"
                    )));
                }
                Some(_) => {}
            }
        }
        if let Some(escalation) = &entry.target_escalation {
            match store.escalation(&target) {
                None => {
                    store.insert_escalation(&target, escalation.clone());
                    changed = true;
                }
                Some(present) if present != escalation => {
                    return Err(Error::MovePending(format!(
                        "{target}'s escalation differs from the journal's"
                    )));
                }
                Some(_) => {}
            }
        }
    }
    if changed {
        store.save()?;
    }
    Ok(())
}

/// G6 and G7: a listed record replaced by its journaled text, unless it already holds it;
/// any bytes but the journaled source refuse.
fn replace(project: &Project, listed: &str, source_digest: &str, text: &str) -> Result<()> {
    let path = project.task_path(&id(listed));
    let state = record_state(&path)?;
    if state == want(text) {
        return Ok(());
    }
    if state != FileState::Digest(source_digest.to_string()) {
        return Err(Error::MovePending(format!(
            "{listed} no longer matches the journal; it changed during the migration"
        )));
    }
    atomic_write(&path, text.as_bytes())
}

/// G8: source attachment bytes (all but retained note archives) and source store entries
/// of every listed id removed.
fn cleanup(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    let source = &ctx.source.project;
    for entry in &journal.carried {
        crate::moving::remove_source_attachments(
            source,
            &id(&entry.source),
            &ctx.target,
            &id(&entry.target),
            &entry.attachments,
        )?;
    }
    let mut store = ClaimStore::load(&source.prefix)?;
    let mut changed = false;
    let entries = journal
        .carried
        .iter()
        .map(|e| (&e.source, &e.source_park, &e.source_escalation))
        .chain(
            journal
                .closed
                .iter()
                .map(|e| (&e.id, &e.source_park, &e.source_escalation)),
        );
    for (listed, park, escalation) in entries {
        changed |= crate::moving::remove_source_entries(
            &mut store,
            &id(listed),
            park.as_ref(),
            escalation.as_ref(),
        )?;
    }
    if changed {
        store.save()?;
    }
    Ok(())
}

/// G9: every owned path committed, one commit per git working tree, then checked clean.
fn commit(journal: &MigrationJournal, warnings: &mut Vec<String>) -> Result<()> {
    let owned = journal.owned_paths();
    crate::written::begin(Some("migrate"));
    for (records, paths) in &owned {
        for path in paths {
            crate::written::record(records, path);
        }
    }
    let mut commit_warnings = Vec::new();
    crate::record_commit::commit_written(&mut commit_warnings);
    let left = super::commit::uncommitted(&owned)?;
    if !left.is_empty() {
        let mut detail = format!(
            "the migration's records are written but not committed: {}",
            left.iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        if !commit_warnings.is_empty() {
            detail.push_str(&format!(" ({})", commit_warnings.join("; ")));
        }
        detail.push_str(&format!(
            "; fix the cause, then rerun `tasks migrate {}`",
            crate::stale::quote(&journal.list_path.display().to_string())
        ));
        return Err(Error::MovePending(detail));
    }
    warnings.extend(commit_warnings);
    Ok(())
}

/// Before G10: each carried pair's attachment ledger agrees with its files.
fn audit(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<()> {
    for entry in &journal.carried {
        crate::moving::audit_pair(
            &ctx.source.project,
            &id(&entry.source),
            &ctx.target,
            &id(&entry.target),
        )?;
    }
    Ok(())
}
