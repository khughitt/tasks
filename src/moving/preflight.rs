use super::journal::{Journal, JournalAttachment};
use super::{MoveCtx, attachment_bytes};
use crate::attachments::{self, Ledger};
use crate::claims::{ClaimSnapshot, ClaimStore};
use crate::commands::{self, append_note, append_stamped_note};
use crate::error::{Error, Result};
use crate::format::{serialize_task, validate_task};
use crate::hierarchy;
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use crate::rename::inventory::digest;
use crate::repo::Project;
use crate::resolve::{DocKind, Resolver};

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
    if let Some(claim) = claims.live(id) {
        return Err(Error::Claimed(
            id.to_string(),
            format!(
                "{}; park it with `tasks park {id} \"<next step>\"` first",
                claim.session
            ),
        ));
    }
    commands::reject_retired(&ctx.registry, &ctx.source.project)?;
    commands::reject_retired(&ctx.registry, &ctx.target)?;
    super::reject_operations(ctx, None)?;
    let now = crate::time::after(&source.updated)?;
    let owner = commands::owner_name(&ctx.source.project)?;
    let mut target = source.clone();
    target.id = ctx.target.new_id()?;
    target.moved_from = Some(id.clone());
    target.updated = now.clone();
    append_stamped_note(
        &mut ctx.source,
        &mut target,
        &owner,
        &format!("moved from {id}"),
    )?;
    validate_destination_docs(&ctx.target, &ctx.registry, &target, id)?;
    validate_record(&ctx.target, &ctx.registry, &target)?;
    let destination = commands::open_ctx(Some(&ctx.target.root))?;
    commands::dep::ensure_acyclic(&destination, &target)?;
    for other in ctx.target.scan()? {
        let why = if other
            .moved_from
            .as_ref()
            .is_some_and(|from| ctx.registry.canonical_id(from) == *id)
        {
            Some("moved_from")
        } else if other
            .source
            .as_deref()
            .and_then(|s| TaskId::parse(s).ok())
            .is_some_and(|from| ctx.registry.canonical_id(&from) == *id)
        {
            Some("source")
        } else {
            None
        };
        if let Some(why) = why {
            return Err(Error::Validation(format!(
                "{} already holds {} for {id} ({why}); inspect it: drop one, or keep the re-file and drop {id}",
                ctx.target.prefix, other.id
            )));
        }
    }
    let mut tombstone = source.clone();
    tombstone.status = Status::Moved;
    tombstone.moved_to = Some(target.id.clone());
    tombstone.updated = now;
    append_stamped_note(
        &mut ctx.source,
        &mut tombstone,
        &owner,
        &format!("moved to {}", target.id),
    )?;
    attachments::check_storage(&ctx.source.project, id)?;
    attachments::check_storage(&ctx.target, &target.id)?;
    let mut attachments = Vec::new();
    for (name, state) in attachments::ledger(&source) {
        if state != Ledger::Attached {
            continue;
        }
        let bytes = attachment_bytes(&ctx.source.project, id, &name)?.ok_or_else(|| Error::Validation(format!("{id}: attachment_missing: {name} is attached but its file is missing; restore it or run `tasks detach {id} {name} \"missing\"`")))?;
        attachments.push(JournalAttachment {
            name: name.clone(),
            sha256: digest(&bytes),
        });
        append_note(
            &mut tombstone,
            &owner,
            &attachments::detached_note(&name, &format!("moved to {}", target.id)),
        )?;
    }
    validate_record(&ctx.source.project, &ctx.registry, &tombstone)?;
    let store = ClaimStore::load(&ctx.source.project.prefix)?;
    let source_park = store.park(id).cloned();
    let source_escalation = store.escalation(id).cloned();
    let target_park = source_park.clone().map(|mut park| {
        park.worktree = ctx.target.resume_root().display().to_string();
        park
    });
    Ok(Journal {
        source: id.to_string(),
        target: target.id.to_string(),
        source_root: ctx.source.project.root.clone(),
        target_root: ctx.target.root.clone(),
        source_digest: digest(source_text.as_bytes()),
        source_text,
        target_text: serialize_task(&target),
        tombstone_text: serialize_task(&tombstone),
        attachments,
        source_park,
        target_park,
        target_escalation: source_escalation.clone(),
        source_escalation,
    })
}

pub(super) fn validate_record(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    validate_task(task)?;
    project.validate_docs(task)?;
    crate::needs::require_declared(&project.needs, &task.needs)?;
    hierarchy::validate_parent(project, registry, task)?;
    hierarchy::validate_lanes(project, registry, task)?;
    hierarchy::validate_periodic(project, registry, task)?;
    hierarchy::validate_defer(project, registry, task)
}

fn validate_destination_docs(
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
