use super::journal::{Journal, JournalAttachment};
use super::{MoveCtx, attachment_bytes};
use crate::attachments::{self, Ledger};
use crate::claims::{ClaimSnapshot, ClaimStore, Escalation, Park};
use crate::commands::{self, Ctx, append_note, append_stamped_note};
use crate::error::{Error, Result};
use crate::format::{serialize_physical, validate_task};
use crate::hierarchy;
use crate::history::ArchiveDescriptor;
use crate::model::{Relation, Status, Task, TaskId};
use crate::registry::Registry;
use crate::rename::inventory::digest;
use crate::repo::Project;
use crate::resolve::{DocKind, LocatedEntry, Resolver};

pub fn prepare(ctx: &mut MoveCtx) -> Result<Journal> {
    let (source, source_text) = ctx.source.project.read_task_with_raw(&ctx.source_id)?;
    commands::refuse_stale_copy(
        &mut ctx.source,
        &source,
        &source_text,
        commands::Writer::Command,
    )?;
    commands::guard_write(&ctx.registry, &ctx.source.shorthand, &source)?;
    let id = &source.id;
    if !source.status.is_open() {
        return Err(Error::Validation(format!(
            "{id} is {}; only open tasks move",
            source.status.as_str()
        )));
    }
    if let Some(parent) = &source.parent {
        return Err(Error::Validation(format!(
            "{id} has parent {parent}; run `tasks edit {id} --no-parent`, move it, then re-parent it in the destination"
        )));
    }
    validate_childless(ctx)?;
    let claims = ClaimSnapshot::load(std::iter::once(ctx.source.project.prefix.as_str()))?;
    if let Some((claim, live)) = claims.get(id)
        && !live.is_stale()
    {
        let me = crate::claims::resolve_identity(&mut ctx.source.warnings);
        let remedy = match (commands::ownership(claim, &me)?, live) {
            (commands::Ownership::Foreign, crate::claims::Liveness::Unverifiable(why)) => format!(
                "unverifiable: {why}{}",
                commands::release::hint(&id.to_string(), &claim.session)
            ),
            (commands::Ownership::Foreign, _) => "that session must park it first".to_string(),
            _ => format!("park it with `tasks park {id} \"<next step>\"` first"),
        };
        return Err(Error::Claimed(
            id.to_string(),
            format!("{}; {remedy}", claim.session),
        ));
    }
    commands::reject_retired(&ctx.registry, &ctx.source.project)?;
    commands::reject_retired(&ctx.registry, &ctx.target)?;
    super::reject_operations(ctx, None)?;
    let now = crate::time::after(&source.updated)?;
    let target_id = ctx.target.new_id()?;
    let pair = build_pair(&mut ctx.source, &ctx.target, &source, target_id, &now, "")?;
    let target = &pair.target;
    validate_destination_docs(&ctx.target, &ctx.registry, target, id)?;
    validate_record(&ctx.target, &ctx.registry, target)?;
    let destination = commands::open_ctx(Some(&ctx.target.records))?;
    commands::dep::ensure_acyclic(&destination, target)?;
    if let Some((other, why)) = collision(&ctx.target, &ctx.registry, id)? {
        return Err(Error::Validation(format!(
            "{} already holds {other} for {id} ({why}); inspect it: drop one, or keep the re-file and drop {id}",
            ctx.target.prefix
        )));
    }
    attachments::check_storage(&ctx.source.project, id)?;
    attachments::check_storage(&ctx.target, &target.id)?;
    validate_record(&ctx.source.project, &ctx.registry, &pair.tombstone)?;
    Ok(Journal {
        source_records: (!source.note_archives.is_empty())
            .then(|| ctx.source.project.records.clone()),
        source: id.to_string(),
        target: pair.target.id.to_string(),
        source_root: ctx.source.project.root.clone(),
        target_root: ctx.target.root.clone(),
        source_digest: digest(source_text.as_bytes()),
        source_text,
        target_text: pair.target_text,
        tombstone_text: pair.tombstone_text,
        attachments: pair.attachments,
        source_park: pair.source_park,
        target_park: pair.target_park,
        target_escalation: pair.target_escalation,
        source_escalation: pair.source_escalation,
    })
}

/// A carried record and its tombstone, rendered but not yet validated in the destination.
pub(crate) struct RenderedPair {
    pub target: Task,
    pub target_text: String,
    pub tombstone: Task,
    pub tombstone_text: String,
    pub attachments: Vec<JournalAttachment>,
    pub source_park: Option<Park>,
    pub source_escalation: Option<Escalation>,
    pub target_park: Option<Park>,
    pub target_escalation: Option<Escalation>,
    /// The source's archived note segments, read once so a re-render needs no source.
    archives: Vec<(ArchiveDescriptor, Vec<u8>)>,
}

/// The construction half of a move (spec §3.2): the new record with `moved_from` and its
/// note, the tombstone with `moved_to`, its note and the `detached:` notes, the attachment
/// list, and the park and escalation re-keyed to the destination's resume root. It checks
/// nothing about the destination and does not ask whether `source` is open: the caller
/// validates, and may rewrite `target` and call `finish_pair` again. `note_suffix` is
/// appended to the `moved from`/`moved to` notes (empty for `move`).
pub(crate) fn build_pair(
    source_ctx: &mut Ctx,
    target_project: &Project,
    source: &Task,
    target_id: TaskId,
    now: &str,
    note_suffix: &str,
) -> Result<RenderedPair> {
    let id = &source.id;
    let owner = commands::owner_name(&source_ctx.project)?;
    let mut target = source.clone();
    target.id = target_id;
    target.moved_from = Some(id.clone());
    target.updated = now.to_string();
    append_stamped_note(
        source_ctx,
        &mut target,
        &owner,
        &format!("moved from {id}{note_suffix}"),
    )?;
    let mut tombstone = source.clone();
    if source.status == Status::Shelved {
        tombstone.wake_on.clear();
        tombstone.defer = None;
    }
    tombstone.status = Status::Moved;
    tombstone.reviewed = false;
    tombstone.reviewed_at = None;
    tombstone.awaiting_user = false;
    tombstone.moved_to = Some(target.id.clone());
    tombstone.updated = now.to_string();
    append_stamped_note(
        source_ctx,
        &mut tombstone,
        &owner,
        &format!("moved to {}{note_suffix}", target.id),
    )?;
    let mut attachments = Vec::new();
    for (name, state) in attachments::ledger(source) {
        if state != Ledger::Attached {
            continue;
        }
        let bytes = attachment_bytes(&source_ctx.project, id, &name)?.ok_or_else(|| Error::Validation(format!("{id}: attachment_missing: {name} is attached but its file is missing; restore it or run `tasks detach {id} {name} \"missing\"`")))?;
        let retain_source = source
            .note_archives
            .iter()
            .any(|archive| archive.file_name() == name);
        attachments.push(JournalAttachment {
            retain_source,
            name: name.clone(),
            sha256: digest(&bytes),
        });
        if !retain_source {
            append_note(
                &mut tombstone,
                &owner,
                &attachments::detached_note(&name, &format!("moved to {}", target.id)),
            )?;
        }
    }
    let store = ClaimStore::load(&source_ctx.project.prefix)?;
    let source_park = store.park(id).cloned();
    let source_escalation = store.escalation(id).cloned();
    let target_park = source_park.clone().map(|mut park| {
        park.worktree = target_project.resume_root().display().to_string();
        park
    });
    let archives = source
        .note_archives
        .iter()
        .map(|archive| {
            Ok((
                archive.clone(),
                source_ctx.project.read_archive(id, archive)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut pair = RenderedPair {
        target,
        target_text: String::new(),
        tombstone,
        tombstone_text: String::new(),
        attachments,
        source_park,
        target_escalation: source_escalation.clone(),
        source_escalation,
        target_park,
        archives,
    };
    finish_pair(&mut pair)?;
    Ok(pair)
}

/// Serialises both records of `pair` as they now stand (after any rewrites).
pub(crate) fn finish_pair(pair: &mut RenderedPair) -> Result<()> {
    let archives = &pair.archives;
    let resolve = |wanted: &ArchiveDescriptor| {
        archives
            .iter()
            .find(|(archive, _)| archive == wanted)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| Error::Validation(format!("{}: archive not read", wanted.file_name())))
    };
    pair.target_text =
        serialize_physical(&crate::history::physical_for_save(&pair.target, resolve)?);
    pair.tombstone_text = serialize_physical(&crate::history::physical_for_save(
        &pair.tombstone,
        resolve,
    )?);
    Ok(())
}

/// A destination record that already holds `id` as its `moved_from` or `source`: moving
/// `id` there again would make a second copy of one task.
pub(crate) fn collision(
    target: &Project,
    registry: &Registry,
    id: &TaskId,
) -> Result<Option<(TaskId, &'static str)>> {
    for other in target.scan()? {
        let why = if other
            .moved_from
            .as_ref()
            .is_some_and(|from| registry.canonical_id(from) == *id)
        {
            Some("moved_from")
        } else if other
            .source
            .as_deref()
            .and_then(|s| TaskId::parse(s).ok())
            .is_some_and(|from| registry.canonical_id(&from) == *id)
        {
            Some("source")
        } else {
            None
        };
        if let Some(why) = why {
            return Ok(Some((other.id, why)));
        }
    }
    Ok(None)
}

pub(super) fn validate_record(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    validate_record_among(project, registry, task, &[])
}

/// `validate_record` for a record written together with `projected`: its parent and lane
/// checks see those records as if they were already on disk.
pub(crate) fn validate_record_among(
    project: &Project,
    registry: &Registry,
    task: &Task,
    projected: &[Task],
) -> Result<()> {
    validate_task(task)?;
    crate::review::validate(task)?;
    crate::defer::validate(task)?;
    project.validate_docs(task)?;
    crate::needs::require_declared(&project.needs, &task.needs)?;
    hierarchy::validate_parent_among(project, registry, task, projected)?;
    hierarchy::validate_lanes_among(project, registry, task, projected)?;
    hierarchy::validate_periodic(project, registry, task)?;
    hierarchy::validate_defer(project, registry, task)
}

pub(crate) fn validate_destination_docs(
    project: &Project,
    registry: &Registry,
    task: &Task,
    source: &TaskId,
) -> Result<()> {
    let resolver = Resolver::new(project, registry);
    for (kind, path) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
        if let Some(path) = path
            && crate::format::external_doc(path)?.is_none()
        {
            resolver.resolve_doc(kind, path).map_err(|error| Error::Validation(format!("{source}: {error} in {}; run `tasks edit {source} --no-{}` or make the document valid there", project.prefix, kind.name())))?;
        }
    }
    if let (Some(plan), Some(step)) = (&task.plan, &task.step)
        && !resolver.step_exists(plan, step)?
    {
        return Err(Error::Validation(format!(
            "{source}: heading {step:?} not found in {plan} in {}; run `tasks edit {source} --no-plan` or add the heading there",
            project.prefix
        )));
    }
    for entry in &task.refs {
        if entry.relation == Relation::Output {
            continue;
        }
        let crate::refs::Location::Path(path) = crate::refs::location(&entry.reference)? else {
            continue;
        };
        match resolver.locate_entry(path)? {
            LocatedEntry::Here(_) | LocatedEntry::Main { .. } | LocatedEntry::Code(_) => {}
            LocatedEntry::Missing => {
                return Err(Error::Validation(format!(
                    "{source}: reference {} {} does not exist in {}; run `tasks edit {source} --rm-ref {}` or create it there",
                    entry.handle, entry.reference, project.prefix, entry.handle
                )));
            }
            LocatedEntry::Special(path) => {
                crate::resolve::refuse_special(&path).map_err(|error| match error {
                    Error::SpecialFile(detail) => Error::SpecialFile(format!(
                        "{source}: reference {}: {detail}; run `tasks edit {source} --rm-ref {}` or create it there",
                        entry.handle, entry.handle
                    )),
                    error => error,
                })?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_childless(ctx: &MoveCtx) -> Result<()> {
    let id = &ctx.source_id;
    let records = ctx.source.project.scan()?;
    let children = hierarchy::children(&records, id, &ctx.registry);
    if !children.is_empty() {
        return Err(Error::Validation(format!(
            "{id} has children {}; move or re-parent each child first",
            children
                .iter()
                .map(|t| t.id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(())
}
