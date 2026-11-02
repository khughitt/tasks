//! The apply phases G2-G10 (spec §4.5). Each phase is idempotent as the protocol table
//! says, so a resume from the phase the classifier names writes the same bytes. Nothing is
//! committed before G9, which commits every path the run owns (one commit per git working
//! tree) and checks that each is clean; G10 removes the journal only after that.

use super::MigrateCtx;
use super::classify::{CarriedState, ClosedState, Phase, Snapshot};
use super::journal::MigrationJournal;
use super::list::Decision;
use crate::attachments;
use crate::claims::ClaimStore;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::moving::classify::FileState;
use crate::moving::{attachment_state, record_state};
use crate::output::{CarriedOut, ClosedOut, DroppedOut, InboundOut, MigrateOut, WokenOut};
use crate::rename::inventory::digest;
use crate::repo::{Project, atomic_write};
use std::collections::BTreeMap;
use std::io::Write;

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
    let paths = journal
        .owned_paths()
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
        .collect();
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

/// Executes G2-G10 from `from` (`Phase::Journal` for a fresh run).
pub fn run(ctx: &mut MigrateCtx, journal: MigrationJournal, from: Phase) -> Result<MigrateOut> {
    let recovery = if from == Phase::Journal {
        "fresh"
    } else {
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
            Phase::Journal => journal.save()?,
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
