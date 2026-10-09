pub mod classify;
pub mod journal;
pub mod preflight;

use crate::attachments::{self, EntryState};
use crate::claims::ClaimStore;
use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::MoveOut;
use crate::registry::Registry;
use crate::rename::inventory::digest;
use crate::repo::{Project, atomic_write};
use classify::{FileState, Recovery, Snapshot};
use journal::Journal;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct MoveCtx {
    pub source: Ctx,
    pub target: Project,
    pub registry: Registry,
    pub source_id: TaskId,
}

pub fn reject_operations(ctx: &MoveCtx, own: Option<&Journal>) -> Result<()> {
    for project in [&ctx.source.project, &ctx.target] {
        crate::commands::reject_pending_rename(project)?;
        for journal in Journal::pending()? {
            if own == Some(&journal) {
                continue;
            }
            if [&journal.source_id(), &journal.target_id()]
                .iter()
                .any(|id| ctx.registry.canonical_id(id).prefix == project.prefix)
            {
                return Err(journal.pending_error());
            }
        }
    }
    Ok(())
}

pub fn attachment_path(project: &Project, id: &TaskId, name: &str) -> Result<PathBuf> {
    attachments::validate_name(name)?;
    attachments::check_storage(project, id)?;
    Ok(attachments::task_dir(project, id).join(name))
}

fn attachment_bytes(project: &Project, id: &TaskId, name: &str) -> Result<Option<Vec<u8>>> {
    let path = attachment_path(project, id, name)?;
    match attachments::entry_state(&path)? {
        EntryState::Absent => Ok(None),
        EntryState::File(_) => Ok(Some(std::fs::read(path)?)),
        EntryState::Unsafe(detail) => Err(Error::AttachmentUnsafe(detail)),
    }
}

fn attachment_state(project: &Project, id: &TaskId, name: &str) -> Result<FileState> {
    Ok(attachment_bytes(project, id, name)?
        .map(|bytes| FileState::Digest(digest(&bytes)))
        .unwrap_or(FileState::Absent))
}

fn record_state(path: &Path) -> Result<FileState> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(FileState::Digest(digest(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileState::Absent),
        Err(error) => Err(error.into()),
    }
}

pub fn snapshot(ctx: &MoveCtx, journal: &Journal) -> Result<Snapshot> {
    for (saved, project) in [
        (&journal.source_root, &ctx.source.project),
        (&journal.target_root, &ctx.target),
    ] {
        if crate::rename::root_identity(saved)? != crate::rename::root_identity(&project.root)? {
            return Err(Error::MovePending(format!(
                "{} is registered at {}, not {}; restore its registration before resuming",
                project.prefix,
                project.root.display(),
                saved.display()
            )));
        }
    }
    if journal.source_id() != ctx.source_id || journal.target_id().prefix != ctx.target.prefix {
        return Err(Error::MovePending(format!(
            "journal names a move of {} to {}; rerun `tasks move {} --to {}`",
            journal.source,
            journal.target,
            journal.source,
            journal.target_id().prefix
        )));
    }
    let source_id = journal.source_id();
    let target_id = journal.target_id();
    let mut source_files = BTreeMap::new();
    let mut target_files = BTreeMap::new();
    for attachment in &journal.attachments {
        source_files.insert(
            attachment.name.clone(),
            attachment_state(&ctx.source.project, &source_id, &attachment.name)?,
        );
        target_files.insert(
            attachment.name.clone(),
            attachment_state(&ctx.target, &target_id, &attachment.name)?,
        );
    }
    let store = ClaimStore::load(&ctx.target.prefix)?;
    Ok(Snapshot {
        journal: journal.clone(),
        source: record_state(&ctx.source.project.task_path(&source_id))?,
        target: record_state(&ctx.target.task_path(&target_id))?,
        source_files,
        target_files,
        target_park: store.park(&target_id).cloned(),
        target_escalation: store.escalation(&target_id).cloned(),
    })
}

pub fn output(ctx: &MoveCtx, journal: &Journal, recovery: &str) -> MoveOut {
    let mut paths = BTreeMap::new();
    for (root, id) in [
        (&journal.source_root, &journal.source),
        (&journal.target_root, &journal.target),
    ] {
        let mut entries = vec![format!("tasks/{id}.md")];
        entries.extend(
            journal
                .attachments
                .iter()
                .map(|a| format!("tasks/files/{id}/{}", a.name)),
        );
        paths.insert(root.display().to_string(), entries);
    }
    MoveOut {
        id: journal.target.clone(),
        from: journal.source.clone(),
        recovery: recovery.into(),
        attachments: journal.attachments.iter().map(|a| a.name.clone()).collect(),
        park: journal.target_park.is_some(),
        inbound: Vec::new(),
        paths,
        warnings: ctx.source.warnings.clone(),
    }
}

pub fn inbound(ctx: &MoveCtx, out: &mut MoveOut) -> Result<()> {
    let (scope, warnings) = crate::scope::Scope::open_all(&ctx.registry, &ctx.source.project.root)?;
    out.warnings.extend(warnings);
    for task in scope.scan()? {
        if task
            .depends
            .iter()
            .any(|dep| ctx.registry.canonical_id(dep) == ctx.source_id)
        {
            out.inbound.push(task.id.to_string());
        }
    }
    out.inbound.sort();
    Ok(())
}

pub fn run(ctx: &mut MoveCtx, journal: Journal) -> Result<MoveOut> {
    let resumed = Journal::load(&ctx.source_id)?.is_some();
    let mut out = output(ctx, &journal, if resumed { "resumed" } else { "fresh" });
    if let Recovery::Refuse(reason) = classify::classify(&snapshot(ctx, &journal)?) {
        return Err(Error::MovePending(reason));
    }
    let source = journal.source_id();
    let target = journal.target_id();
    let task = crate::format::parse_task(&journal.target_text, &format!("{target}.md"))?;
    let tombstone = crate::format::parse_task(&journal.tombstone_text, &format!("{source}.md"))?;
    if task.id != target || tombstone.id != source {
        return Err(Error::MovePending(
            "journal record ids disagree with its source or target".into(),
        ));
    }
    preflight::validate_record(&ctx.target, &ctx.registry, &task)?;
    preflight::validate_record(&ctx.source.project, &ctx.registry, &tombstone)?;
    inbound(ctx, &mut out)?;
    journal.save()?;
    if stop_after("journal") {
        return Ok(out);
    }
    for attachment in &journal.attachments {
        let want = FileState::Digest(attachment.sha256.clone());
        if attachment_state(&ctx.target, &target, &attachment.name)? != want {
            let path = attachment_path(&ctx.target, &target, &attachment.name)?;
            match attachments::entry_state(&path)? {
                EntryState::File(_) => {
                    return Err(Error::MovePending(format!(
                        "attachment {} in {target} differs from its journal digest",
                        attachment.name
                    )));
                }
                EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
                EntryState::Absent => {}
            }
            let bytes = attachment_bytes(&ctx.source.project, &source, &attachment.name)?
                .ok_or_else(|| {
                    Error::MovePending(format!("attachment {} is missing", attachment.name))
                })?;
            if digest(&bytes) != attachment.sha256 {
                return Err(Error::MovePending(format!(
                    "attachment {} differs from its journal digest",
                    attachment.name
                )));
            }
            attachments::ensure_task_dir(&ctx.target, &target)?;
            attachments::write_new(
                &attachment_path(&ctx.target, &target, &attachment.name)?,
                &bytes,
            )?;
        }
        verify_attachment(&ctx.target, &target, attachment)?;
    }
    if stop_after("attachments") {
        return Ok(out);
    }
    let target_path = ctx.target.task_path(&target);
    if record_state(&target_path)? == FileState::Absent {
        ctx.target.create_task_exact(&ctx.registry, &task)?;
    }
    if record_state(&target_path)? != FileState::Digest(digest(journal.target_text.as_bytes())) {
        return Err(Error::MovePending(format!(
            "{target} differs from its journal text"
        )));
    }
    if stop_after("destination") {
        return Ok(out);
    }
    let mut store = ClaimStore::load(&ctx.target.prefix)?;
    let mut changed = false;
    if let Some(park) = &journal.target_park
        && store.park(&target).is_none()
    {
        store.insert_park(&target, park.clone());
        changed = true;
    }
    if let Some(escalation) = &journal.target_escalation
        && store.escalation(&target).is_none()
    {
        store.insert_escalation(&target, escalation.clone());
        changed = true;
    }
    if changed {
        store.save()?;
    }
    if stop_after("store") {
        return Ok(out);
    }
    let source_path = ctx.source.project.task_path(&source);
    if record_state(&source_path)? != FileState::Digest(digest(journal.tombstone_text.as_bytes())) {
        atomic_write(&source_path, journal.tombstone_text.as_bytes())?;
    }
    if stop_after("tombstone") {
        return Ok(out);
    }
    for attachment in &journal.attachments {
        verify_attachment(&ctx.target, &target, attachment)?;
        let path = attachment_path(&ctx.source.project, &source, &attachment.name)?;
        match attachments::entry_state(&path)? {
            EntryState::File(_) => std::fs::remove_file(path)?,
            EntryState::Absent => {}
            EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    if attachments::check_storage(&ctx.source.project, &source)? {
        let dir = attachments::task_dir(&ctx.source.project, &source);
        if std::fs::read_dir(&dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
        }
    }
    let mut store = ClaimStore::load(&ctx.source.project.prefix)?;
    let park_removed = store.remove_park(&source).is_some();
    let escalation_removed = store.remove_escalation(&source).is_some();
    if park_removed || escalation_removed {
        store.save()?;
    }
    if stop_after("cleanup") {
        return Ok(out);
    }
    journal.remove()?;
    Ok(out)
}

fn verify_attachment(
    project: &Project,
    id: &TaskId,
    attachment: &journal::JournalAttachment,
) -> Result<()> {
    if attachment_state(project, id, &attachment.name)?
        != FileState::Digest(attachment.sha256.clone())
    {
        return Err(Error::MovePending(format!(
            "attachment {} in {id} does not match its journal digest",
            attachment.name
        )));
    }
    Ok(())
}

fn stop_after(phase: &str) -> bool {
    std::env::var("TASKS_MOVE_STOP_AFTER").is_ok_and(|stop| stop == phase)
}
