use super::model::*;
use crate::config::HostConfig;
use crate::{
    error::{Error, Result},
    model::{Status, Task},
    repo::Project,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct PendingOperation {
    pub note: OperationNote,
}
fn latest(task: &Task) -> Result<Vec<OperationNote>> {
    let mut phases = BTreeMap::<(String, String), OperationNote>::new();
    for note in &task.notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let n = decode_operation(raw)?;
            let key = (n.key.coordinator.clone(), n.key.operation_id.clone());
            validate_transition(phases.get(&key), &n)?;
            phases.insert(key, n);
        }
    }
    Ok(phases.into_values().collect())
}
fn one_pending(mut live: impl Iterator<Item = OperationNote>) -> Result<Option<PendingOperation>> {
    let first = live.next();
    if live.next().is_some() {
        return Err(Error::RetainedHistoryBlocked(
            "Competing retained operation intents; recover exact operation history".into(),
        ));
    }
    Ok(first.map(|note| PendingOperation { note }))
}
#[cfg(test)]
pub fn pending(task: &Task) -> Result<Option<PendingOperation>> {
    one_pending(
        latest(task)?
            .into_iter()
            .filter(|n| !matches!(n.phase.as_str(), "completed" | "reconciled")),
    )
}
fn pending_here(project: &Project, task: &Task) -> Result<Option<PendingOperation>> {
    let mut live = Vec::new();
    for note in latest(task)? {
        if note.phase == "completed" {
            continue;
        }
        if matches!(note.phase.as_str(), "reconcile_ready" | "reconciled")
            && super::reconcile::reservation_ended(project, task, &note)?
        {
            continue;
        }
        live.push(note);
    }
    one_pending(live.into_iter())
}
fn reserved(text: &str) -> bool {
    [
        "retained:",
        "retention operation:",
        "retention checkpoint:",
        "attached:",
        "detached:",
    ]
    .iter()
    .any(|p| text.starts_with(p))
}
pub fn protected(name: &str) -> bool {
    (name.starts_with("retained-op-") && name.ends_with(".request.json"))
        || ((name.starts_with("retained-manifest-") || name.starts_with("retained-checkpoint-"))
            && name.ends_with(".json"))
}
pub enum WriteAction<'a> {
    Archive,
    Detach(&'a str),
    Identity,
    Retire,
}
pub fn permission(project: &Project, task: &Task, action: WriteAction<'_>) -> Result<()> {
    if let WriteAction::Detach(name) = action
        && protected(name)
    {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{name} is protected retained evidence; preserve its immutable locator"
        )));
    }
    if matches!(action, WriteAction::Identity) {
        let mut artifacts = crate::attachments::ledger(task)
            .keys()
            .any(|n| protected(n));
        if crate::attachments::check_storage(project, &task.id)? {
            for entry in std::fs::read_dir(crate::attachments::task_dir(project, &task.id))? {
                let entry = entry?;
                if entry.file_name().to_str().is_some_and(protected) {
                    artifacts = true;
                }
            }
        }
        if artifacts
            || task
                .notes
                .iter()
                .any(|n| n.text.starts_with("retention operation: "))
        {
            return Err(Error::RetainedHistoryBlocked(format!(
                "{} carries immutable retention locators; ID-changing transformations require an explicit locator-preservation design",
                task.id
            )));
        }
    }
    if let Some(p) = pending_here(project, task)? {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{} is reserved by {}/{} at {}; resume or reconcile that operation before mutation",
            task.id, p.note.key.coordinator, p.note.key.operation_id, p.note.phase
        )));
    }
    Ok(())
}
pub fn project_permission(project: &Project, identity: bool) -> Result<()> {
    for task in project.scan()? {
        permission(
            project,
            &task,
            if identity {
                WriteAction::Identity
            } else {
                WriteAction::Retire
            },
        )?;
    }
    Ok(())
}
pub fn check_write(
    project: &Project,
    old: &Task,
    new: &Task,
    permission: WritePermission,
) -> Result<()> {
    if let WritePermission::Retained(ref authority) = permission {
        return authority.validate(project, old, new);
    }
    if old
        .notes
        .iter()
        .any(|n| n.text.starts_with("retention operation: "))
        && !new.notes.starts_with(&old.notes)
    {
        return Err(invalid(
            "immutable retained operation history cannot be rewritten",
        ));
    }
    if new.notes.starts_with(&old.notes) {
        for n in &new.notes[old.notes.len()..] {
            if n.text.starts_with("retention operation:")
                || n.text
                    .strip_prefix("retention checkpoint:")
                    .is_some_and(|s| s.trim_start().starts_with('{'))
            {
                return Err(invalid(
                    "machine retention envelopes require the owned retained publication path",
                ));
            }
            if let Some((crate::attachments::Ledger::Detached, name)) =
                crate::attachments::parse_ledger_note(&n.text)
                && protected(name)
            {
                return Err(invalid(
                    "protected retained evidence cannot be detached by a generic note",
                ));
            }
        }
    }
    if pending_here(project, old)?.is_none() {
        return Ok(());
    }
    let mut allowed = old.clone();
    allowed.updated = new.updated.clone();
    allowed.notes = new.notes.clone();
    if matches!(permission, WritePermission::Claim)
        && new.status == Status::Doing
        && old.status.is_open()
    {
        let at = new.started.as_deref().unwrap_or(&new.updated);
        crate::time::parse(at)?;
        crate::commands::apply_status_effects(&mut allowed, Status::Doing, at, None)?;
        allowed.owner = Some(crate::commands::owner_name(project)?);
    }
    if matches!(permission, WritePermission::Park) {
        allowed.complexity = new.complexity;
    }
    if allowed != *new
        || !new.notes.starts_with(&old.notes)
        || new.notes[old.notes.len()..]
            .iter()
            .any(|n| reserved(&n.text))
    {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{} has a pending retained operation; only unrelated appended notes, start/claim refresh and park may commute",
            old.id
        )));
    }
    Ok(())
}

pub struct Authority {
    key: OperationKey,
    recovery: Option<Box<StagedRecovery>>,
}
#[derive(Clone)]
struct StagedRecovery {
    base: Task,
    from_sha: String,
    backup_name: String,
    backup_sha: String,
}
const STAGED_ACTION: &str = "Reinspect the original request; unpublished task preserved as ";
pub enum WritePermission {
    Ordinary,
    Claim,
    Park,
    Retained(Authority),
}
impl Authority {
    fn validate(&self, project: &Project, old: &Task, new: &Task) -> Result<()> {
        let baseline = if let Some(recovery) = &self.recovery {
            let raw = project.read_raw(&old.id)?.into_bytes();
            let backup = super::verify::read_local(
                &crate::attachments::task_dir(project, &old.id).join(&recovery.backup_name),
                project.attachments_max_bytes,
            )?;
            if super::verify::sha(&raw) != recovery.from_sha
                || super::verify::sha(&backup) != recovery.backup_sha
                || crate::attachments::ledger(new).get(&recovery.backup_name)
                    != Some(&crate::attachments::Ledger::Attached)
            {
                return Err(invalid(
                    "staged recovery preimage or preserved backup changed",
                ));
            }
            &recovery.base
        } else {
            old
        };
        if !new.notes.starts_with(&baseline.notes) {
            return Err(invalid("publication cannot rewrite published history"));
        }
        if let Some(p) = pending_here(project, baseline)?
            && p.note.key != self.key
        {
            return Err(invalid("publication lacks the task reservation"));
        }
        for n in &new.notes[baseline.notes.len()..] {
            if let Some(raw) = n.text.strip_prefix("retention operation: ")
                && decode_operation(raw)?.key != self.key
            {
                return Err(invalid("publication carries a foreign operation"));
            }
        }
        Ok(())
    }
}

pub struct OperationCtx {
    pub key: OperationKey,
    started: std::time::Instant,
    staged_recovery: Option<(crate::model::TaskId, StagedRecovery)>,
    pub tasks: BTreeMap<crate::model::TaskId, super::basis::PinnedTask>,
    pub metrics: super::verify::Metrics,
    contexts: BTreeMap<String, crate::commands::Ctx>,
    locks: BTreeMap<std::path::PathBuf, crate::record_commit::CommitLock>,
}
pub struct NewArtifact {
    pub name: String,
    pub bytes: Vec<u8>,
}
pub struct GeneratedNote {
    pub at: String,
    pub text: String,
}
pub struct Published {
    pub publication: Publication,
    pub sources: Vec<Source>,
}
pub fn open(
    dir: Option<&std::path::Path>,
    ids: &[String],
    key: &OperationKey,
    config: Arc<HostConfig>,
) -> Result<OperationCtx> {
    let started = std::time::Instant::now();
    key.validate()?;
    let mut contexts = BTreeMap::new();
    let mut resolved = std::collections::BTreeSet::new();
    for raw in ids {
        let ctx = crate::commands::open_id_unlocked_ctx(dir, raw, config.clone())?;
        let id = crate::commands::parse_id(&ctx.registry, &ctx.shorthand, raw)?;
        if id.to_string() != *raw {
            return Err(invalid("operation participants require canonical task ids"));
        }
        if let Some(existing) = contexts.get(&id.prefix) {
            let existing: &crate::commands::Ctx = existing;
            if existing.project.records != ctx.project.records {
                return Err(invalid(
                    "same-prefix participants have different record pairings",
                ));
            }
        } else {
            contexts.insert(id.prefix.clone(), ctx);
        }
        resolved.insert(id);
    }
    for ctx in contexts.values_mut() {
        ctx.lock = Some(crate::claims::MutationLock::acquire(&ctx.project.prefix)?);
    }
    for ctx in contexts.values_mut() {
        crate::commands::revalidate_retained_ctx(ctx)?;
        if !ctx.project.commit_records {
            return Err(Error::RetainedPublicationUnsupported("Retained publication requires Git records with [git] commit = true; use the manual immutable-publication route".into()));
        }
    }
    let mut repositories = BTreeMap::new();
    for ctx in contexts.values() {
        let common = crate::record_commit::common_dir(&ctx.project.records)
            .map_err(Error::RetainedPublicationUnsupported)?;
        repositories
            .entry(common)
            .or_insert_with(|| ctx.project.records.clone());
    }
    let mut locks = BTreeMap::new();
    for (common, records) in repositories {
        let lock =
            crate::record_commit::lock(&records).map_err(Error::RetainedPublicationUnsupported)?;
        let warnings = lock.commit_paths(&records, &[], "chore(tasks): retained preflight");
        if !warnings.is_empty() {
            return Err(Error::RetainedHistoryBlocked(warnings.join("; ")));
        }
        locks.insert(common, lock);
    }
    let mut tasks = BTreeMap::new();
    for id in resolved {
        let ctx = contexts.get_mut(&id.prefix).expect("resolved prefix");
        crate::commands::retained_owner(ctx, &id)?;
        let mut pinned = super::basis::read(&ctx.project, &id, Some("HEAD"))?;
        let working = crate::commands::load(ctx, &id.to_string())?;
        let completed = working.notes.iter().any(|n| {
            n.text
                .strip_prefix("retention operation: ")
                .and_then(|s| decode_operation(s).ok())
                .is_some_and(|n| {
                    n.key == *key && matches!(n.phase.as_str(), "completed" | "reconciled")
                })
        });
        if let Some(p) = pending_here(&ctx.project, &working)?
            && p.note.key != *key
            && !completed
        {
            return Err(invalid("task is reserved by another retained operation"));
        }
        if working != pinned.task {
            let mut expected = pinned.task.clone();
            expected.updated = working.updated.clone();
            expected.notes = working.notes.clone();
            expected.owner = working.owner.clone();
            expected.complexity = working.complexity;
            if expected != working
                || !working.notes.starts_with(&pinned.task.notes)
                || !working.notes[pinned.task.notes.len()..].iter().any(|n| {
                    n.text
                        .strip_prefix("retention operation: ")
                        .and_then(|raw| decode_operation(raw).ok())
                        .is_some_and(|n| n.key == *key)
                })
            {
                return Err(Error::RetainedStaleBasis("Unpublished task changes are outside this operation; publish or recover their exact preimage first".into()));
            }
            for n in &working.notes[pinned.task.notes.len()..] {
                if let Some(raw) = n.text.strip_prefix("retention operation: ")
                    && decode_operation(raw)?.key != *key
                {
                    return Err(invalid("foreign unpublished operation"));
                }
            }
            pinned.task = working;
        }
        tasks.insert(id, pinned);
    }
    let mut metrics = super::verify::Metrics::default();
    metrics.history_bytes_read = tasks.values().map(|p| p.history_bytes_read).sum();
    let mut owners = Vec::new();
    for id in tasks.keys() {
        let ctx = contexts.get_mut(&id.prefix).expect("participant prefix");
        owners.push((
            id.clone(),
            ctx.claims_mut()?
                .get(id)
                .expect("owned claim")
                .session
                .clone(),
        ));
    }
    metrics.track(owners);
    Ok(OperationCtx {
        key: key.clone(),
        started,
        staged_recovery: None,
        tasks,
        contexts,
        locks,
        metrics,
    })
}
impl OperationCtx {
    pub fn project(&self, id: &crate::model::TaskId) -> &Project {
        &self
            .contexts
            .get(&id.prefix)
            .expect("operation participant prefix")
            .project
    }
}
fn stop_phase(boundary: &str, phase: &str) {
    stop(boundary);
    stop(&format!("{boundary}:{phase}"));
}
pub fn stop(boundary: &str) {
    if std::env::var("TASKS_RETAINED_STOP_AFTER").as_deref() == Ok(boundary) {
        eprintln!(
            "retained test interruption metrics: {}",
            serde_json::to_string(&super::verify::Metrics::default().finish())
                .expect("metrics serialize")
        );
        std::process::exit(86)
    }
}
fn preimage(
    project: &Project,
    id: &crate::model::TaskId,
    input: &Basis,
    metrics: &mut super::verify::Metrics,
) -> Result<()> {
    let original = super::basis::read_exact(project, id, input)?;
    metrics.history_bytes_read += original.history_bytes_read;
    let current = super::basis::read(project, id, Some("HEAD"))?;
    metrics.history_bytes_read += current.history_bytes_read;
    if current.task != original.task || current.basis.retained_ref != input.retained_ref {
        return Err(Error::RetainedStaleBasis(
            "Committed task or publishing branch changed between phases".into(),
        ));
    }
    for (name, state) in crate::attachments::ledger(&original.task) {
        if state == crate::attachments::Ledger::Attached {
            let path = format!("{}/tasks/files/{id}/{name}", input.records_subdir);
            let bytes = super::basis::blob(
                project,
                &input.commit,
                path.trim_start_matches("./"),
                Some(project.attachments_max_bytes),
                &mut metrics.history_bytes_read,
            )?;
            if super::verify::read_local(
                &crate::attachments::task_dir(project, id).join(&name),
                project.attachments_max_bytes,
            )? != bytes
            {
                return Err(Error::RetainedStaleBasis(format!(
                    "Required attachment {name} differs from its committed preimage"
                )));
            }
        }
    }
    Ok(())
}
impl OperationCtx {
    pub fn revalidate(&mut self) -> Result<()> {
        self.heartbeat()?;
        for id in self.tasks.keys().cloned().collect::<Vec<_>>() {
            let project = self.project(&id).clone();
            preimage(&project, &id, &self.tasks[&id].basis, &mut self.metrics)?;
            if project.read_task(&id)? != self.tasks[&id].task {
                return Err(Error::RetainedStaleBasis(
                    "Participant changed between retained phases".into(),
                ));
            }
        }
        Ok(())
    }
}
pub fn batch(
    ctx: &mut OperationCtx,
    id: &crate::model::TaskId,
    artifacts: &[NewArtifact],
    notes: &[GeneratedNote],
) -> Result<Published> {
    let expected = &ctx
        .tasks
        .get(id)
        .ok_or_else(|| invalid("publication names a nonparticipant"))?
        .task;
    let context = ctx
        .contexts
        .get_mut(&id.prefix)
        .expect("participant context");
    crate::commands::retained_owner(context, id)?;
    crate::attachments::check_storage(&context.project, id)?;
    let input = &ctx.tasks.get(id).expect("participant").basis;
    preimage(&context.project, id, input, &mut ctx.metrics)?;
    let mut task = context.project.read_task(id)?;
    if task != *expected {
        return Err(Error::RetainedStaleBasis(
            "Task changed between retained phases; retry after recovering the exact preimage"
                .into(),
        ));
    }
    let recovery = ctx.staged_recovery.take();
    let recovery = match recovery {
        Some((target, recovery)) if target == *id => Some(recovery),
        Some(_) => return Err(invalid("staged recovery targets another participant")),
        None => None,
    };
    if let Some(recovery) = &recovery {
        task = recovery.base.clone();
    }
    let mut sources = Vec::new();
    if notes.is_empty() {
        return Err(invalid("publication requires generated notes"));
    }
    let reused = task
        .notes
        .windows(notes.len())
        .enumerate()
        .rev()
        .find(|(_, window)| {
            window
                .iter()
                .zip(notes)
                .all(|(old, new)| old.at == new.at && old.text == new.text)
        })
        .map(|(i, _)| i);
    let phase = notes
        .iter()
        .rev()
        .find_map(|n| n.text.strip_prefix("retention operation: "))
        .ok_or_else(|| invalid("every publication requires its phase receipt"))?;
    let operation = decode_operation(phase)?;
    if operation.key != ctx.key {
        return Err(invalid("publication key mismatch"));
    }
    if reused.is_none() {
        let previous = task.notes.iter().rev().find_map(|n| {
            n.text
                .strip_prefix("retention operation: ")
                .and_then(|s| decode_operation(s).ok())
                .filter(|n| n.key == ctx.key)
        });
        validate_transition(previous.as_ref(), &operation)?;
    }

    for artifact in artifacts {
        crate::attachments::refuse_reserved(&context.project, &task, &artifact.name)?;
    }

    for artifact in artifacts {
        crate::attachments::validate_name(&artifact.name)?;
        if artifact.bytes.len() as u64 > context.project.attachments_max_bytes {
            return Err(Error::AttachmentTooLarge("Retained artifact exceeds the configured cap; preserve raw bytes and use the labelled portable fallback before retry".into()));
        }
        if crate::attachments::ledger(&task).get(&artifact.name)
            == Some(&crate::attachments::Ledger::Detached)
        {
            return Err(invalid(
                "retained artifact name has a detached history; choose a fresh name before publishing",
            ));
        }
        crate::attachments::check_storage(&context.project, id)?;
        crate::attachments::ensure_task_dir(&context.project, id)?;
        let path = crate::attachments::task_dir(&context.project, id).join(&artifact.name);
        match crate::attachments::entry_state(&path)? {
            crate::attachments::EntryState::Absent => {
                crate::attachments::write_new(&path, &artifact.bytes)?
            }
            crate::attachments::EntryState::File(_) => {
                if super::verify::read_local(&path, context.project.attachments_max_bytes)?
                    != artifact.bytes
                {
                    return Err(invalid(
                        "immutable retained artifact name already holds different bytes",
                    ));
                }
            }
            crate::attachments::EntryState::Unsafe(detail) => {
                return Err(Error::AttachmentUnsafe(detail));
            }
        }
        crate::written::record(&context.project.records, &path);
        let mut parent = path.parent();
        while let Some(dir) = parent {
            std::fs::File::open(dir)?.sync_all()?;
            if dir == context.project.records {
                break;
            }
            parent = dir.parent();
        }
        if reused.is_some()
            && crate::attachments::ledger(&task).get(&artifact.name)
                != Some(&crate::attachments::Ledger::Attached)
        {
            return Err(invalid(
                "a phase receipt cannot substitute for its missing attachment ledger",
            ));
        }
        match crate::attachments::ledger(&task).get(&artifact.name) {
            Some(crate::attachments::Ledger::Attached) => {}
            Some(crate::attachments::Ledger::Detached) => {
                return Err(invalid(
                    "retained artifact name was detached; preserve that history and use a new immutable name",
                ));
            }
            None => {
                crate::commands::append_note(
                    &mut task,
                    &crate::commands::owner_name(&context.project)?,
                    &crate::attachments::attached_note(
                        &artifact.name,
                        artifact.bytes.len() as u64,
                        None,
                    ),
                )?;
            }
        }
    }
    stop_phase("artifact", &operation.phase);
    let start = reused.unwrap_or(task.notes.len());
    if reused.is_none() {
        for n in notes {
            crate::time::parse(&n.at)?;
            crate::commands::append_note(
                &mut task,
                &crate::commands::owner_name(&context.project)?,
                &n.text,
            )?;
            task.notes.last_mut().expect("new note").at = n.at.clone();
        }
        let max = notes
            .iter()
            .map(|n| n.at.as_str())
            .chain(std::iter::once(task.updated.as_str()))
            .max()
            .expect("updated");
        task.updated = crate::time::after(max)?;
    }
    crate::format::validate_task(&task)?;
    let (_, problems) = crate::attachments::audit_task(&context.project, &task)?;
    if !problems.is_empty() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Retained publication attachment audit refused: {problems:?}"
        )));
    }
    context.project.write_task_permitted(
        &context.registry,
        &task,
        || Ok(()),
        WritePermission::Retained(Authority {
            key: ctx.key.clone(),
            recovery: recovery.map(Box::new),
        }),
    )?;
    stop_phase("save", &operation.phase);
    let written = crate::written::take();
    if written
        .roots
        .keys()
        .any(|root| root != &context.project.records)
    {
        return Err(invalid(
            "publication encountered written paths belonging to another phase/project",
        ));
    }
    let paths = written
        .roots
        .get(&context.project.records)
        .map(|p| p.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let common = crate::record_commit::common_dir(&context.project.records)
        .map_err(Error::RetainedPublicationUnsupported)?;
    let warnings = ctx
        .locks
        .get(&common)
        .ok_or_else(|| invalid("publication repository changed under held lock"))?
        .commit_paths(
            &context.project.records,
            &paths,
            &format!("chore(tasks): retained {} {}", id, operation.phase),
        );
    if !warnings.is_empty() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Operation {}/{} publication incomplete at {}: {}",
            ctx.key.coordinator,
            ctx.key.operation_id,
            operation.phase,
            warnings.join("; ")
        )));
    }
    stop_phase("commit", &operation.phase);
    let pinned = super::basis::read(&context.project, id, None)?;
    ctx.metrics.history_bytes_read += pinned.history_bytes_read;
    if pinned.task != task {
        return Err(Error::RetainedHistoryBlocked("Committed retained record differs from intended publication; preserve exact evidence and recover".into()));
    }
    for artifact in artifacts {
        let path = format!(
            "{}/tasks/files/{id}/{}",
            pinned.basis.records_subdir, artifact.name
        );
        let bytes = super::basis::blob(
            &context.project,
            &pinned.basis.commit,
            path.trim_start_matches("./"),
            Some(context.project.attachments_max_bytes),
            &mut ctx.metrics.history_bytes_read,
        )?;
        if bytes != artifact.bytes {
            return Err(invalid(
                "committed retained artifact differs from exact intended bytes",
            ));
        }
    }
    for i in start..start + notes.len() {
        sources.push(Source {
            basis: pinned.basis.clone(),
            ordinal: i as u64 + 1,
        });
    }
    let publication = Publication {
        task: id.to_string(),
        phase: operation.phase,
        basis: pinned.basis.clone(),
    };
    ctx.tasks.insert(id.clone(), pinned);
    Ok(Published {
        publication,
        sources,
    })
}

pub struct Binding {
    pub bytes: Vec<u8>,
    pub request: ArtifactLocator,
    pub name: String,
    pub inputs: Vec<Input>,
    pub host: String,
}
impl OperationCtx {
    pub fn phase(&self, id: &crate::model::TaskId) -> Result<Option<(usize, OperationNote)>> {
        let task = &self
            .tasks
            .get(id)
            .ok_or_else(|| invalid("unknown participant"))?
            .task;
        for (i, n) in task.notes.iter().enumerate().rev() {
            if let Some(raw) = n.text.strip_prefix("retention operation: ") {
                let note = decode_operation(raw)?;
                if note.key == self.key {
                    return Ok(Some((i, note)));
                }
            }
        }
        Ok(None)
    }
    pub fn observer(&mut self, id: &crate::model::TaskId) -> Result<Option<String>> {
        let ctx = self
            .contexts
            .get_mut(&id.prefix)
            .ok_or_else(|| invalid("unknown participant"))?;
        Ok(ctx
            .resolve_for_guard()?
            .identity()
            .map(|i| i.tagged.clone()))
    }
    pub fn heartbeat(&mut self) -> Result<()> {
        let ids = self.tasks.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let ctx = self.contexts.get_mut(&id.prefix).expect("participant");
            crate::commands::retained_owner(ctx, &id)?;
        }
        Ok(())
    }
    pub fn phase_publications(&mut self, id: &crate::model::TaskId) -> Result<Vec<Publication>> {
        let task = self
            .tasks
            .get(id)
            .ok_or_else(|| invalid("unknown participant"))?
            .task
            .clone();
        let project = self.project(id).clone();
        let mut publications = Vec::new();
        for (i, n) in task.notes.iter().enumerate() {
            if let Some(raw) = n.text.strip_prefix("retention operation: ") {
                let note = decode_operation(raw)?;
                if note.key == self.key {
                    let input = super::lineage::participant(&project, &note, id)?;
                    let historical_task = input.task.clone();
                    let context = input.basis.clone();
                    let basis = super::basis::publication_basis(
                        &project,
                        id,
                        &context,
                        &task.notes[..=i],
                        &mut self.metrics.history_bytes_read,
                    )?;
                    publications.push(Publication {
                        task: historical_task,
                        phase: note.phase,
                        basis,
                    });
                }
            }
        }
        Ok(publications)
    }
}
pub fn binding<T: serde::de::DeserializeOwned + PartialEq>(
    ctx: &mut OperationCtx,
    request: &T,
    provided: &[u8],
    inputs: &[Input],
    host: &str,
    name: &str,
) -> Result<Binding> {
    let id = crate::model::TaskId::parse(&ctx.key.coordinator)?;
    let project = ctx.project(&id).clone();
    crate::attachments::validate_name(name)?;
    crate::attachments::check_storage(&project, &id)?;
    let existing = ctx.phase(&id)?;
    let mut bytes = provided.to_vec();
    let mut locator = None;
    if let Some((index, note)) = existing {
        if note.input_bases != inputs || note.host != host {
            return Err(invalid(
                "same operation key is bound to different input bases or host",
            ));
        }
        let pinned = ctx.tasks.get(&id).expect("coordinator");
        let committed = super::basis::read_exact(&project, &id, &pinned.basis)?;
        ctx.metrics.history_bytes_read += committed.history_bytes_read;
        if committed.task.notes.get(index) == pinned.task.notes.get(index) {
            let loc = note.request.pinned(&id.to_string(), &pinned.basis)?;
            bytes = super::verify::artifact(&project, &loc, &mut ctx.metrics)?;
        } else {
            let path = crate::attachments::task_dir(&project, &id).join(name);
            if matches!(
                crate::attachments::entry_state(&path)?,
                crate::attachments::EntryState::File(_)
            ) {
                bytes = super::verify::read_local(&path, project.attachments_max_bytes)?;
            }
            let expected = match &note.request {
                ArtifactLocator::Attachment { sha256, .. }
                | ArtifactLocator::Git { sha256, .. } => sha256,
                _ => return Err(invalid("operation request must be a file")),
            };
            if super::verify::hash_bytes(&bytes, &mut ctx.metrics) != *expected {
                return Err(invalid(
                    "unpublished request bytes are missing or differ from the original intent",
                ));
            }
        }
        locator = Some(note.request);
    } else {
        let path = crate::attachments::task_dir(&project, &id).join(name);
        match crate::attachments::entry_state(&path)? {
            crate::attachments::EntryState::Absent => {}
            crate::attachments::EntryState::File(_) => {
                bytes = super::verify::read_local(&path, project.attachments_max_bytes)?;
            }
            crate::attachments::EntryState::Unsafe(detail) => {
                return Err(Error::AttachmentUnsafe(detail));
            }
        }
    }
    if decode_request::<T>(&bytes)? != *request {
        return Err(invalid(
            "same operation key has different parsed intent; restore its original request",
        ));
    }
    let request = locator.unwrap_or_else(|| ArtifactLocator::Attachment {
        task: id.to_string(),
        name: name.into(),
        basis: None,
        sha256: super::verify::hash_bytes(&bytes, &mut ctx.metrics),
    });
    Ok(Binding {
        bytes,
        request,
        name: name.into(),
        inputs: inputs.to_vec(),
        host: host.into(),
    })
}
pub fn validate_bases(ctx: &OperationCtx, inputs: &[Input]) -> Result<()> {
    if inputs.len() != ctx.tasks.len() {
        return Err(invalid("request inputs differ from locked participants"));
    }
    for input in inputs {
        let id = crate::model::TaskId::parse(&input.task)?;
        if !ctx.tasks.contains_key(&id) {
            return Err(invalid("unlocked input participant"));
        }
        super::basis::validate_locator(ctx.project(&id), &id, &input.basis)?;
    }
    Ok(())
}
pub fn validate_inputs(ctx: &mut OperationCtx, inputs: &[Input]) -> Result<()> {
    let coordinator = crate::model::TaskId::parse(&ctx.key.coordinator)?;
    let retry = ctx.phase(&coordinator)?.is_some();
    validate_since(ctx, inputs, inputs, retry)
}
pub fn validate_since(
    ctx: &mut OperationCtx,
    inputs: &[Input],
    phase_inputs: &[Input],
    retry: bool,
) -> Result<()> {
    for input in inputs {
        let id = crate::model::TaskId::parse(&input.task)?;
        let pinned = ctx
            .tasks
            .get(&id)
            .ok_or_else(|| invalid("request inputs differ from locked participants"))?;
        let project = ctx.project(&id).clone();
        super::basis::validate_locator(&project, &id, &input.basis)?;
        if input.basis.retained_ref != pinned.basis.retained_ref {
            return Err(Error::RetainedStaleBasis(
                "Publish from the input's named retained branch".into(),
            ));
        }
        let old = super::basis::read_exact(&project, &id, &input.basis)?;
        ctx.metrics.history_bytes_read += old.history_bytes_read;
        if !retry {
            if input.basis != pinned.basis || pinned.task != old.task {
                return Err(Error::RetainedStaleBasis("A new operation requires exact current committed inputs; refresh its basis before retry".into()));
            }
        } else {
            let current = &ctx.tasks.get(&id).expect("participant").task;
            let mut expected = old.task.clone();
            expected.notes = current.notes.clone();
            expected.updated = current.updated.clone();
            expected.owner = current.owner.clone();
            expected.complexity = current.complexity;
            if expected != *current || !current.notes.starts_with(&old.task.notes) {
                return Err(Error::RetainedStaleBasis(
                    "Operation input has non-append edits; recover its exact reserved preimage"
                        .into(),
                ));
            }
            let mut items = std::collections::BTreeSet::new();
            let mut attachments = std::collections::BTreeSet::new();
            let mut checkpoints = std::collections::BTreeSet::new();
            for note in &current.notes[old.task.notes.len()..] {
                if let Some(raw) = note.text.strip_prefix("retention operation: ") {
                    let n = decode_operation(raw)?;
                    if n.key != ctx.key || n.input_bases != phase_inputs {
                        return Err(invalid(
                            "foreign phase or changed intent after the original input",
                        ));
                    }
                    match &n.outputs {
                        PhaseOutputs::Items(o) => items.extend(o.items.iter().copied()),
                        PhaseOutputs::Receiver(o) => items.extend(o.items.iter().copied()),
                        PhaseOutputs::Sender(o) => items.extend(o.items.iter().copied()),
                        PhaseOutputs::Ready(o) => items.extend(o.items.iter().copied()),
                        PhaseOutputs::Completed(o) => checkpoints.extend(o.checkpoint),
                        PhaseOutputs::Incomplete(o) => checkpoints.extend(o.checkpoint),
                        _ => {}
                    }
                    for artifact in observation_artifacts(&project, current, &n)? {
                        if let ArtifactLocator::Attachment { task, name, .. } = artifact
                            && task == id.to_string()
                        {
                            attachments.insert(name);
                        }
                    }
                }
            }
            for (index, note) in current.notes.iter().enumerate().skip(old.task.notes.len()) {
                let ordinal = index as u64 + 1;
                let valid = if note.text.starts_with("retained:") {
                    items.contains(&ordinal)
                } else if note.text.starts_with("retention checkpoint:") {
                    checkpoints.contains(&ordinal)
                        && note
                            .text
                            .strip_prefix("retention checkpoint: ")
                            .and_then(|raw| decode_checkpoint(raw).ok())
                            .is_some_and(|n| n.operation == ctx.key)
                } else if note.text.starts_with("attached:") || note.text.starts_with("detached:") {
                    crate::attachments::parse_ledger_note(&note.text).is_some_and(
                        |(state, name)| {
                            state == crate::attachments::Ledger::Attached
                                && attachments.contains(name)
                        },
                    )
                } else {
                    true
                };
                if !valid {
                    return Err(Error::RetainedStaleBasis("Intervening retention/evidence note is not bound to this operation's receipts; recover its exact reserved input".into()));
                }
            }
        }
    }
    Ok(())
}
pub fn receipt(
    ctx: &mut OperationCtx,
    binding: &Binding,
    id: &crate::model::TaskId,
    outputs: PhaseOutputs,
    at: &str,
) -> Result<GeneratedNote> {
    let request = if let Some((_, previous)) = ctx.phase(id)? {
        previous.request
    } else if id.to_string() == ctx.key.coordinator {
        binding.request.clone()
    } else {
        let coordinator = crate::model::TaskId::parse(&ctx.key.coordinator)?;
        binding.request.pinned(
            &coordinator.to_string(),
            &ctx.tasks.get(&coordinator).expect("coordinator").basis,
        )?
    };
    let note = OperationNote {
        version: 1,
        key: ctx.key.clone(),
        phase: outputs.phase().into(),
        request,
        input_bases: binding.inputs.clone(),
        host: binding.host.clone(),
        at: at.into(),
        observer: ctx.observer(id)?,
        outputs,
        diagnostics: Vec::new(),
    };
    let text = format!(
        "retention operation: {}",
        serde_json::to_string(&note).map_err(invalid)?
    );
    decode_operation(text.strip_prefix("retention operation: ").expect("prefix"))?;
    Ok(GeneratedNote {
        at: at.into(),
        text,
    })
}
pub fn intent(ctx: &mut OperationCtx, binding: &Binding, id: &crate::model::TaskId) -> Result<()> {
    let note = if let Some((index, phase)) = ctx.phase(id)? {
        if phase.phase != "intent" {
            return Ok(());
        }
        let pinned = ctx.tasks.get(id).expect("participant");
        let committed = super::basis::read_exact(ctx.project(id), id, &pinned.basis)?;
        let committed_note = committed.task.notes.get(index) == pinned.task.notes.get(index);
        ctx.metrics.history_bytes_read += committed.history_bytes_read;
        if committed_note {
            return Ok(());
        }
        let n = &ctx.tasks.get(id).expect("participant").task.notes[index];
        GeneratedNote {
            at: n.at.clone(),
            text: n.text.clone(),
        }
    } else {
        let at = crate::time::after(&ctx.tasks.get(id).expect("participant").task.updated)?;
        receipt(ctx, binding, id, PhaseOutputs::Intent(Empty {}), &at)?
    };
    let artifacts = if id.to_string() == ctx.key.coordinator {
        vec![NewArtifact {
            name: binding.name.clone(),
            bytes: binding.bytes.clone(),
        }]
    } else {
        Vec::new()
    };
    batch(ctx, id, &artifacts, &[note])?;
    stop("intent");
    Ok(())
}
pub fn result(
    ctx: &mut OperationCtx,
    outcome: String,
    counts: Counts,
    actions: Vec<String>,
    replay: bool,
) -> Result<MutationResult> {
    let mut publications = Vec::new();
    let ids = ctx.tasks.keys().cloned().collect::<Vec<_>>();
    for id in ids {
        publications.extend(ctx.phase_publications(&id)?);
    }
    publications.sort_by(|a, b| a.task.cmp(&b.task));
    ctx.metrics.wall_ns = ctx.started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    Ok(MutationResult {
        version: 1,
        operation: ctx.key.clone(),
        outcome,
        publications,
        counts,
        actions,
        metrics: ctx.metrics.finish(),
        warnings: if replay {
            vec!["Completed operation replay; current filesystem bytes were not inspected".into()]
        } else {
            Vec::new()
        },
    })
}

fn phase_artifacts(note: &OperationNote) -> Result<Vec<ArtifactLocator>> {
    let mut artifacts = vec![note.request.clone()];
    match &note.outputs {
        PhaseOutputs::Items(o) => artifacts.extend(o.artifacts.clone()),
        PhaseOutputs::Receiver(o) => artifacts.extend(o.artifacts.clone()),
        PhaseOutputs::ReconcileIntent(o) => artifacts.push(o.directive.clone()),
        PhaseOutputs::Ready(o) => artifacts.push(o.directive.clone()),
        PhaseOutputs::Reconciled(o) => artifacts.push(o.directive.clone()),
        PhaseOutputs::Incomplete(o) => {
            if let Some(raw) = o.action.strip_prefix(STAGED_ACTION) {
                artifacts.push(decode_request(raw.as_bytes())?);
            }
        }
        _ => {}
    }
    Ok(artifacts)
}
pub(super) fn observation_artifacts(
    project: &Project,
    task: &Task,
    note: &OperationNote,
) -> Result<Vec<ArtifactLocator>> {
    let mut artifacts = phase_artifacts(note)?;
    if let PhaseOutputs::Ready(out) = &note.outputs {
        for ordinal in &out.items {
            let raw = task
                .notes
                .get(*ordinal as usize - 1)
                .and_then(|n| n.text.strip_prefix("retained: "))
                .ok_or_else(|| invalid("ready observation source missing"))?;
            let record = decode_record(raw)?;
            if let (Some(path), Some(digest)) = (record.manifest.value(), record.sha256.value()) {
                let path = std::path::Path::new(path);
                let input = super::lineage::participant(project, note, &task.id)?;
                let name = format!("retained-manifest-{digest}.json");
                let suffix = format!("tasks/files/{}/{name}", input.task);
                // The producing host's absolute root is not a Git locator. Bind the
                // generated name to its participant and ledger, then pin at publication.
                if path.ends_with(&suffix) {
                    if crate::attachments::ledger(task).get(&name)
                        != Some(&crate::attachments::Ledger::Attached)
                    {
                        return Err(invalid("ready manifest lacks an attachment ledger binding"));
                    }
                    artifacts.push(ArtifactLocator::Attachment {
                        task: input.task.clone(),
                        name,
                        basis: None,
                        sha256: digest.into(),
                    });
                }
            }
        }
    }
    Ok(artifacts)
}
pub fn recover_saved(
    ctx: &mut OperationCtx,
    id: &crate::model::TaskId,
    binding: &Binding,
) -> Result<()> {
    let project = ctx.project(id).clone();
    let pinned = ctx
        .tasks
        .get(id)
        .ok_or_else(|| invalid("unknown recovery participant"))?;
    let original = super::basis::read_exact(&project, id, &pinned.basis)?;
    ctx.metrics.history_bytes_read += original.history_bytes_read;
    if original.task == pinned.task {
        return Ok(());
    }
    let working = pinned.task.clone();
    let Some((_, phase)) = ctx.phase(id)? else {
        return Err(invalid(
            "unpublished data lacks its original operation phase",
        ));
    };
    if matches!(phase.phase.as_str(), "completed" | "sender_released") {
        let prior = original
            .task
            .notes
            .iter()
            .rev()
            .filter_map(|n| {
                n.text
                    .strip_prefix("retention operation: ")
                    .and_then(|r| decode_operation(r).ok())
            })
            .find(|n| n.key == ctx.key)
            .ok_or_else(|| invalid("unsafe staged phase lacks a committed intent"))?;
        let raw = project.read_raw(id)?.into_bytes();
        let from_sha = super::verify::hash_bytes(&raw, &mut ctx.metrics);
        let bytes=serde_json::to_vec(&serde_json::json!({"version":1,"operation":ctx.key,"basis":original.basis,"raw_sha256":from_sha,"raw_task":String::from_utf8(raw).map_err(invalid)?,"action":"Reinspect before publishing an interrupted completion or sender release"})).map_err(invalid)?;
        let backup_sha = super::verify::hash_bytes(&bytes, &mut ctx.metrics);
        let name = format!("retained-checkpoint-{backup_sha}.json");
        let mut base = original.task;
        base.owner = working.owner.clone();
        base.complexity = working.complexity;
        base.updated = working.updated.clone();
        let at = crate::time::after(&base.updated)?;
        let locator = ArtifactLocator::Attachment {
            task: id.to_string(),
            name: name.clone(),
            basis: None,
            sha256: backup_sha.clone(),
        };
        let action = format!(
            "{STAGED_ACTION}{}",
            serde_json::to_string(&locator).map_err(invalid)?
        );
        let attempt = base
            .notes
            .iter()
            .filter_map(|n| {
                n.text
                    .strip_prefix("retention operation: ")
                    .and_then(|r| decode_operation(r).ok())
            })
            .filter(|n| n.key == ctx.key && n.phase == "incomplete")
            .count() as u64
            + 1;
        let note = receipt(
            ctx,
            binding,
            id,
            PhaseOutputs::Incomplete(IncompleteOutput {
                attempt,
                previous_phase: prior.phase,
                checkpoint: None,
                action,
            }),
            &at,
        )?;
        ctx.staged_recovery = Some((
            id.clone(),
            StagedRecovery {
                base,
                from_sha,
                backup_name: name.clone(),
                backup_sha,
            },
        ));
        batch(ctx, id, &[NewArtifact { name, bytes }], &[note])?;
        return Ok(());
    }
    let suffix = &working.notes[original.task.notes.len()..];
    let mut expected = BTreeMap::new();
    let mut items = std::collections::BTreeSet::new();
    for note in suffix {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let operation = decode_operation(raw)?;
            if operation.key != ctx.key || operation.input_bases != binding.inputs {
                return Err(invalid("unpublished phase differs from its bound intent"));
            }
            for artifact in observation_artifacts(&project, &working, &operation)? {
                if let ArtifactLocator::Attachment {
                    task, name, sha256, ..
                } = artifact
                    && task == id.to_string()
                    && expected
                        .insert(name, sha256.clone())
                        .is_some_and(|old| old != sha256)
                {
                    return Err(invalid("conflicting staged artifact digests"));
                }
            }
            let ordinals = match operation.outputs {
                PhaseOutputs::Items(o) => o.items,
                PhaseOutputs::Receiver(o) => o.items,
                PhaseOutputs::Sender(o) => o.items,
                PhaseOutputs::Ready(o) => o.items,
                _ => Vec::new(),
            };
            items.extend(ordinals);
        }
    }
    for (index, note) in working
        .notes
        .iter()
        .enumerate()
        .skip(original.task.notes.len())
    {
        if note.text.starts_with("retained: ") && !items.contains(&(index as u64 + 1)) {
            return Err(invalid(
                "unpublished retained object lacks its exact batch receipt",
            ));
        }
    }
    let before = crate::attachments::ledger(&original.task);
    let mut artifacts = Vec::new();
    for (name, state) in crate::attachments::ledger(&working) {
        if state == crate::attachments::Ledger::Attached && before.get(&name) != Some(&state) {
            let bytes = super::verify::read_local(
                &crate::attachments::task_dir(&project, id).join(&name),
                project.attachments_max_bytes,
            )?;
            let hash = super::verify::hash_bytes(&bytes, &mut ctx.metrics);
            if expected.get(&name) != Some(&hash) {
                return Err(invalid(
                    "unpublished attachment lacks its exact artifact digest in the phase receipt",
                ));
            }
            artifacts.push(NewArtifact { name, bytes });
        }
    }
    let notes = suffix
        .iter()
        .map(|n| GeneratedNote {
            at: n.at.clone(),
            text: n.text.clone(),
        })
        .collect::<Vec<_>>();
    batch(ctx, id, &artifacts, &notes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixture(test: &str) -> Option<(PathBuf, crate::model::TaskId)> {
        if let Ok(dir) = std::env::var("TASKS_PUBLICATION_FIXTURE") {
            return Some((
                dir.into(),
                crate::model::TaskId::parse(&std::env::var("TASKS_PUBLICATION_ID").unwrap())
                    .unwrap(),
            ));
        }
        let f = crate::common::retained::RetainedEnv::new("in-tree");
        f.json(&["start", &f.id]);
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tasks");
        use std::os::unix::fs::PermissionsExt;
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let hook = f.dir.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nexec {} -C {} check >/dev/null\n",
                quote(binary.to_str().unwrap()),
                quote(f.dir.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let extra = tempfile::tempdir().unwrap();
        let second = if test.ends_with("same_git_lock") {
            crate::common::git(extra.path(), &["init", "-qb", "main"]);
            f.env.json(
                extra.path(),
                &[
                    "init",
                    "--prefix",
                    "fam",
                    "--records",
                    f.dir.to_str().unwrap(),
                ],
            );
            let config = f.dir.join("fam/tasks/.config.toml");
            let mut value: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
            value.as_table_mut().unwrap().insert(
                "git".into(),
                toml::Value::Table(toml::map::Map::from_iter([(
                    "commit".into(),
                    toml::Value::Boolean(true),
                )])),
            );
            std::fs::write(config, toml::to_string(&value).unwrap()).unwrap();
            crate::common::git(&f.dir, &["add", "fam/tasks/.config.toml"]);
            crate::common::git(&f.dir, &["commit", "-qm", "second records configuration"]);
            let id = f.json(&["add", "Second participant", "--project", "fam"])["id"]
                .as_str()
                .unwrap()
                .to_string();
            f.json(&["start", &id]);
            Some(id)
        } else {
            None
        };
        let command = || {
            let mut c = std::process::Command::new(std::env::current_exe().unwrap());
            c.env_clear()
                .envs(crate::common::isolated_env(f.env.home.path()))
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_PUBLICATION_FIXTURE", &f.dir)
                .env("TASKS_PUBLICATION_ID", &f.id)
                .args(["--exact", test, "--nocapture"]);
            if let Some(second) = &second {
                c.env("TASKS_PUBLICATION_SECOND", second);
            }
            c
        };
        if second.is_some() {
            let mut children = vec![
                command()
                    .env("TASKS_PUBLICATION_REVERSE", "0")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap(),
                command()
                    .env("TASKS_PUBLICATION_REVERSE", "1")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap(),
            ];
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while children.iter_mut().any(|c| c.try_wait().unwrap().is_none()) {
                if std::time::Instant::now() > deadline {
                    for c in &mut children {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    panic!("opposite-order retained locking timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            for c in children {
                let out = c.wait_with_output().unwrap();
                assert!(out.status.success(), "{out:?}");
            }
            return None;
        }
        if let Some(boundary) = test.rsplit_once("_crash_").map(|p| p.1) {
            let out = command()
                .env("TASKS_RETAINED_STOP_AFTER", boundary)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(86), "{out:?}");
        }
        let out = command().output().unwrap();
        assert!(out.status.success(), "{out:?}");
        None
    }
    fn example(id: &crate::model::TaskId) -> (OperationKey, NewArtifact, GeneratedNote) {
        let key = OperationKey {
            coordinator: id.to_string(),
            operation_id: "a".repeat(32),
        };
        let name = format!("retained-op-{}.request.json", key.operation_id);
        let bytes = b"{}".to_vec();
        let p = Project::open(std::path::Path::new(
            &std::env::var("TASKS_PUBLICATION_FIXTURE").unwrap(),
        ))
        .unwrap();
        let existing = p
            .read_task(id)
            .unwrap()
            .notes
            .into_iter()
            .rev()
            .find_map(|n| {
                n.text
                    .strip_prefix("retention operation: ")
                    .and_then(|s| decode_operation(s).ok())
            });
        let note = if let Some(note) = existing {
            note
        } else {
            let input = super::super::basis::read(&p, id, None).unwrap().basis;
            OperationNote {
                version: 1,
                key: key.clone(),
                phase: "intent".into(),
                request: ArtifactLocator::Attachment {
                    task: id.to_string(),
                    name: name.clone(),
                    basis: None,
                    sha256: super::super::verify::sha(&bytes),
                },
                input_bases: vec![Input {
                    task: id.to_string(),
                    basis: input,
                }],
                host: crate::claims::hostname(),
                at: "2026-11-03T00:00:00Z".into(),
                observer: None,
                outputs: PhaseOutputs::Intent(Empty {}),
                diagnostics: vec![],
            }
        };
        (
            key,
            NewArtifact { name, bytes },
            GeneratedNote {
                at: note.at.clone(),
                text: format!(
                    "retention operation: {}",
                    serde_json::to_string(&note).unwrap()
                ),
            },
        )
    }
    #[test]
    fn retained_publish_orphan_request() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_publish_orphan_request")
        else {
            return;
        };
        let (key, a, n) = example(&id);
        let p = Project::open(&dir).unwrap();
        crate::attachments::ensure_task_dir(&p, &id).unwrap();
        let path = crate::attachments::task_dir(&p, &id).join(&a.name);
        std::fs::write(&path, &a.bytes).unwrap();
        let mut ctx = open(
            Some(&dir),
            &[id.to_string()],
            &key,
            std::sync::Arc::new(crate::config::HostConfig::default()),
        )
        .unwrap();
        assert_eq!(ctx.project(&id).prefix, id.prefix);
        let result = batch(&mut ctx, &id, &[a], &[n]).unwrap();
        assert_eq!(result.publication.task, id.to_string());
        assert_eq!(result.sources.len(), 1);
        assert!(
            !super::super::basis::git(
                &p,
                &[
                    "ls-files",
                    "--error-unmatch",
                    path.strip_prefix(&dir).unwrap().to_str().unwrap()
                ]
            )
            .unwrap()
            .is_empty()
        );
        assert!(crate::written::take().roots.is_empty());
    }
    fn publish_case(test: &str, case: &str) {
        let Some((dir, id)) = fixture(test) else {
            return;
        };
        let (key, mut a, n) = example(&id);
        let p = Project::open(&dir).unwrap();
        if case == "foreign_index" {
            std::fs::write(dir.join("foreign"), "foreign").unwrap();
            crate::common::git(&dir, &["add", "foreign"]);
            assert!(
                open(
                    Some(&dir),
                    &[id.to_string()],
                    &key,
                    std::sync::Arc::new(crate::config::HostConfig::default())
                )
                .is_err()
            );
            assert!(
                !p.read_task(&id)
                    .unwrap()
                    .notes
                    .iter()
                    .any(|n| n.text.starts_with("retention operation: "))
            );
            return;
        }
        if case == "different" {
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            let path = crate::attachments::task_dir(&p, &id).join(&a.name);
            std::fs::write(&path, "different").unwrap();
            let mut ctx = open(
                Some(&dir),
                &[id.to_string()],
                &key,
                std::sync::Arc::new(crate::config::HostConfig::default()),
            )
            .unwrap();
            assert!(batch(&mut ctx, &id, &[a], &[n]).is_err());
            assert_eq!(std::fs::read(path).unwrap(), b"different");
            return;
        }
        if case == "missing_ledger" {
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            std::fs::write(
                crate::attachments::task_dir(&p, &id).join(&a.name),
                &a.bytes,
            )
            .unwrap();
            let path = p.task_path(&id);
            let mut raw = std::fs::read_to_string(&path).unwrap();
            raw.push_str(&format!("- {} (fixture): {}\n", n.at, n.text));
            std::fs::write(path, raw).unwrap();
            let mut ctx = open(
                Some(&dir),
                &[id.to_string()],
                &key,
                std::sync::Arc::new(crate::config::HostConfig::default()),
            )
            .unwrap();
            assert!(batch(&mut ctx, &id, &[a], &[n]).is_err());
            return;
        }
        if case == "manifest" {
            a.name = "retained-manifest-a.json".into();
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            std::fs::write(
                crate::attachments::task_dir(&p, &id).join(&a.name),
                &a.bytes,
            )
            .unwrap();
        }
        if case == "warning" {
            std::fs::write(dir.join(".git/hooks/pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
        }
        let mut ctx = open(
            Some(&dir),
            &[id.to_string()],
            &key,
            std::sync::Arc::new(crate::config::HostConfig::default()),
        )
        .unwrap();
        assert_eq!(ctx.contexts.len(), 1);
        assert_eq!(ctx.locks.len(), 1);
        let before = super::super::basis::resolve(&p, "HEAD").unwrap();
        let result = batch(&mut ctx, &id, &[a], &[n]);
        if case == "warning" {
            assert!(result.is_err());
            assert_eq!(before, super::super::basis::resolve(&p, "HEAD").unwrap());
            assert!(pending(&p.read_task(&id).unwrap()).unwrap().is_some());
        } else {
            let result = result.unwrap();
            assert_eq!(result.publication.phase, "intent");
            assert_eq!(
                pending(&p.read_task(&id).unwrap())
                    .unwrap()
                    .unwrap()
                    .note
                    .key,
                key
            );
            assert!(crate::written::take().roots.is_empty());
        }
    }
    #[test]
    fn retained_publish_orphan_manifest() {
        publish_case(
            "retained::publish::tests::retained_publish_orphan_manifest",
            "manifest",
        )
    }
    #[test]
    fn retained_publish_foreign_index() {
        publish_case(
            "retained::publish::tests::retained_publish_foreign_index",
            "foreign_index",
        )
    }
    #[test]
    fn retained_publication_warning() {
        publish_case(
            "retained::publish::tests::retained_publication_warning",
            "warning",
        )
    }
    #[test]
    fn retained_publish_crash_artifact() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_artifact",
            "crash",
        )
    }
    #[test]
    fn retained_publish_crash_save() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_save",
            "crash",
        )
    }
    #[test]
    fn retained_publish_crash_commit() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_commit",
            "crash",
        )
    }
    #[test]
    fn retained_publish_same_git_lock() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_publish_same_git_lock")
        else {
            return;
        };
        let key = OperationKey {
            coordinator: id.to_string(),
            operation_id: "a".repeat(32),
        };
        let second = std::env::var("TASKS_PUBLICATION_SECOND").unwrap();
        let mut ids = vec![id.to_string(), second.clone()];
        if std::env::var("TASKS_PUBLICATION_REVERSE").as_deref() == Ok("1") {
            ids.reverse();
        }
        let mut ctx = open(
            Some(&dir),
            &ids,
            &key,
            std::sync::Arc::new(crate::config::HostConfig::default()),
        )
        .unwrap();
        let (_, a, n) = example(&id);
        assert_eq!(ctx.contexts.len(), 2);
        assert_eq!(ctx.locks.len(), 1);
        let path = dir.join("fam/unrelated.txt");
        if !path.exists() {
            std::fs::write(&path, "unrelated").unwrap();
        }
        batch(&mut ctx, &id, &[a], &[n]).unwrap();
        let out = crate::git::at(&dir)
            .args(["ls-files", "--error-unmatch", "fam/unrelated.txt"])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "unrelated");
        assert!(
            ctx.tasks
                .contains_key(&crate::model::TaskId::parse(&second).unwrap())
        );
    }
    #[test]
    fn retained_publish_different_orphan() {
        publish_case(
            "retained::publish::tests::retained_publish_different_orphan",
            "different",
        )
    }
    #[test]
    fn retained_publish_receipt_without_ledger() {
        publish_case(
            "retained::publish::tests::retained_publish_receipt_without_ledger",
            "missing_ledger",
        )
    }
}

// Historical request replay retains current claims/locks but never admits a new intent.
pub fn replay(
    mut ctx: OperationCtx,
    provided: &[u8],
    reconciliation: bool,
) -> Result<MutationResult> {
    let mut from = None;
    for id in ctx.tasks.keys() {
        if super::lineage::same(ctx.project(id), &ctx.key.coordinator, &id.to_string())? {
            if from.is_some() {
                return Err(invalid("ambiguous historical coordinator"));
            }
            from = Some(id.clone());
        }
    }
    let from = from.ok_or_else(|| invalid("historical coordinator is not locked"))?;
    let project = ctx.project(&from).clone();
    let pinned = ctx.tasks[&from].clone();
    super::proof::validate_history(&project, &pinned, None)?;
    super::reconcile::validate_terminal_history(&project, &pinned)?;
    let (index, terminal) = ctx
        .phase(&from)?
        .ok_or_else(|| invalid("historical operation not found"))?;
    if terminal.phase
        != if reconciliation {
            "reconciled"
        } else {
            "completed"
        }
    {
        return Err(invalid(
            "historical retry requires the exact terminal receipt",
        ));
    }
    let input = super::lineage::participant(&project, &terminal, &from)?;
    let publication = super::basis::publication_basis(
        &project,
        &from,
        &input.basis,
        &pinned.task.notes[..=index],
        &mut ctx.metrics.history_bytes_read,
    )?;
    let locator = if reconciliation {
        match &terminal.outputs {
            PhaseOutputs::Reconciled(out) => &out.directive,
            _ => return Err(invalid("reconciliation receipt shape")),
        }
    } else {
        &terminal.request
    };
    let bytes = super::verify::artifact(
        &project,
        &locator.pinned(&input.task, &publication)?,
        &mut ctx.metrics,
    )?;
    let original: serde_json::Value = decode_request(&bytes)?;
    let supplied: serde_json::Value = decode_request(provided)?;
    if original != supplied {
        return Err(invalid(
            "historical retry differs from the immutable parsed request",
        ));
    }
    if reconciliation {
        return super::reconcile::result(&mut ctx, &from, &terminal, true);
    }
    if terminal.input_bases.len() == 1 {
        let request: CheckpointRequest = decode_request(provided)?;
        return super::checkpoint::replay(&mut ctx, &from, &request)?
            .ok_or_else(|| invalid("historical checkpoint receipt missing"));
    }
    let counts = super::transfer::counts(&mut ctx, &terminal.host)?;
    result(&mut ctx, "transferred".into(), counts, Vec::new(), true)
}
