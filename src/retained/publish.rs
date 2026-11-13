use super::model::*;
use crate::config::HostConfig;
use crate::{
    error::{Error, Result},
    model::{Status, Task},
    registry::Registry,
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
pub(crate) fn pending_here(project: &Project, task: &Task) -> Result<Option<PendingOperation>> {
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
pub(super) fn reserved(text: &str) -> bool {
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
    registry: &Registry,
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
        crate::commands::apply_status_effects(registry, &mut allowed, Status::Doing, at, None)?;
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
    phase_bases: BTreeMap<crate::model::TaskId, Basis>,
    original_publications: BTreeMap<Vec<u8>, OriginalPublication>,
    retry: bool,
    durable_phase: bool,
    pub(super) committed_release: bool,
    pub metrics: super::verify::Metrics,
    pub warnings: Vec<String>,
    contexts: BTreeMap<String, crate::commands::Ctx>,
    repositories: BTreeMap<String, Vec<u8>>,
}
struct OriginalPublication {
    basis: Basis,
    tip: Basis,
    _permit: super::read::MemoPermit,
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
    let repositories = contexts
        .iter()
        .map(|(prefix, ctx)| {
            repository_identity(&ctx.project).map(|identity| (prefix.clone(), identity))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let records = contexts
        .values()
        .map(|ctx| ctx.project.records.clone())
        .collect::<Vec<_>>();
    let guard = PhaseGuard::acquire(&records)?;
    let mut op_warnings = Vec::new();
    for (prefix, ctx) in &contexts {
        if repository_identity(&ctx.project)? != repositories[prefix] {
            return Err(invalid("publication repository identity changed"));
        }
    }
    guard.recover(&records, &mut op_warnings)?;
    for ctx in contexts.values_mut() {
        crate::commands::revalidate_retained_ctx(ctx)?;
    }
    let mut reads = None;
    for ctx in contexts.values_mut() {
        ctx.project = match &reads {
            Some(shared) => super::read::inherit(shared, ctx.project.clone())?,
            None => super::read::start_live(&ctx.project)?,
        };
        reads = Some(ctx.project.clone());
    }
    let mut tasks = BTreeMap::new();
    let mut retry = false;
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
        retry |= pinned.task.notes.iter().any(|entry| {
            entry
                .text
                .strip_prefix("retention operation: ")
                .and_then(|raw| decode_operation(raw).ok())
                .is_some_and(|note| note.key == *key)
        });
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
    drop(guard);
    Ok(OperationCtx {
        key: key.clone(),
        started,
        staged_recovery: None,
        phase_bases: BTreeMap::new(),
        original_publications: BTreeMap::new(),
        retry,
        durable_phase: false,
        committed_release: false,
        tasks,
        warnings: op_warnings,
        contexts,
        repositories,
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
    key: &OperationKey,
    metrics: &mut super::verify::Metrics,
) -> Result<()> {
    preimage_checked(project, id, input, key, metrics, None)
}
fn preimage_checked(
    project: &Project,
    id: &crate::model::TaskId,
    input: &Basis,
    key: &OperationKey,
    metrics: &mut super::verify::Metrics,
    warm: Option<&super::freshness::WarmRange>,
) -> Result<()> {
    let original = super::basis::read_exact(project, id, input)?;
    metrics.history_bytes_read += original.history_bytes_read;
    let fresh = super::freshness::context(project)?;
    let current = super::basis::read(&fresh, id, Some("HEAD"))?;
    metrics.history_bytes_read += current.history_bytes_read;
    let mut published = false;
    for note in &original.task.notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            published |= decode_operation(raw)?.key == *key;
        }
    }
    if published && warm.is_none() {
        super::freshness::phase_unchanged(project, id, input, &current.basis, &original.task)?;
    } else {
        super::freshness::check(
            project,
            id,
            input,
            &current.basis,
            &original.task,
            if published {
                super::freshness::ProgressPolicy::PendingNotes
            } else {
                super::freshness::ProgressPolicy::Exact
            },
            warm,
        )?;
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
    fn preimage_basis(&self, id: &crate::model::TaskId) -> Result<&Basis> {
        if self.committed_release {
            // A proven sender release permits only missing receipts against current work.
            return Ok(&self.tasks[id].basis);
        }
        self.phase_bases
            .get(id)
            .ok_or_else(|| invalid("durable participant phase basis was not recovered"))
    }
    pub(super) fn refresh_completion_target(&mut self, id: &crate::model::TaskId) -> Result<()> {
        if !self.committed_release {
            return Err(invalid("completion refresh requires proved sender release"));
        }
        let previous = &self.tasks[id];
        let project = super::freshness::context(self.project(id))?;
        super::read::refresh(&project)?;
        super::basis::validate_locator(&project, id, &previous.basis)?;
        let current = super::basis::read(&project, id, Some("HEAD"))?;
        if current.basis.retained_ref != previous.basis.retained_ref
            || !current.task.notes.starts_with(&previous.task.notes)
        {
            return Err(Error::RetainedStaleBasis(
                "Completion target changed its branch or published operation history".into(),
            ));
        }
        super::freshness::current(&project, id, &current, true)?;
        self.metrics.history_bytes_read += current.history_bytes_read;
        // Post-release receipts use current work, without changing the last durable P.
        self.tasks.insert(id.clone(), current);
        Ok(())
    }
    pub fn revalidate(&mut self) -> Result<()> {
        self.heartbeat()?;
        for id in self.tasks.keys().cloned().collect::<Vec<_>>() {
            let project = self.project(&id).clone();
            let phase = self.preimage_basis(&id)?.clone();
            preimage(&project, &id, &phase, &self.key, &mut self.metrics)?;
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
    let result = batch_inner(ctx, id, artifacts, notes);
    if result.is_err() {
        ctx.retry = true;
        for context in ctx.contexts.values() {
            super::read::refresh(&context.project)?;
        }
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PhaseScope {
    Target,
    All,
}

pub(super) fn phase_scope(
    notes: &[GeneratedNote],
    target: &crate::model::TaskId,
    committed_release: bool,
) -> Result<PhaseScope> {
    let mut previous: Option<OperationNote> = None;
    let mut scope = PhaseScope::Target;
    for note in notes {
        let Some(raw) = note.text.strip_prefix("retention operation: ") else {
            continue;
        };
        let operation = decode_operation(raw)?;
        if !operation
            .input_bases
            .iter()
            .any(|input| input.task == target.to_string())
        {
            return Err(invalid("phase target is not an operation participant"));
        }
        if let Some(prior) = &previous {
            if prior.key != operation.key {
                return Err(invalid("publication carries multiple operations"));
            }
            validate_transition(Some(prior), &operation)?;
        }
        let all = match operation.phase.as_str() {
            "intent" => operation.input_bases.len() > 1,
            "receiver_published" | "sender_released" | "reconciled" => true,
            "completed" => operation.input_bases.len() > 1 && !committed_release,
            "items_published" if operation.input_bases.len() != 1 => {
                return Err(invalid("checkpoint phase requires one participant"));
            }
            "items_published" | "incomplete" | "reconcile_intent" | "reconcile_ready" => false,
            _ => return Err(invalid("unknown publication phase")),
        };
        if all {
            scope = PhaseScope::All;
        }
        previous = Some(operation);
    }
    if previous.is_none() {
        return Err(invalid("every publication requires its phase receipt"));
    }
    Ok(scope)
}

fn repository_identity(project: &Project) -> Result<Vec<u8>> {
    super::basis::git(
        project,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
        ],
    )
}

struct PhaseGuard {
    locks: BTreeMap<std::path::PathBuf, crate::record_commit::CommitLock>,
}
impl PhaseGuard {
    fn acquire(records: &[std::path::PathBuf]) -> Result<Self> {
        let mut repositories = BTreeMap::new();
        for records in records {
            let common = crate::record_commit::common_dir(records)
                .map_err(Error::RetainedPublicationUnsupported)?;
            repositories.entry(common).or_insert(records);
        }
        let mut guard = Self {
            locks: BTreeMap::new(),
        };
        for (common, records) in repositories {
            super::test_observer::observe("lock_wait", &common.to_string_lossy())?;
            let lock = crate::record_commit::lock(records)
                .map_err(Error::RetainedPublicationUnsupported)?;
            guard.locks.insert(common.clone(), lock);
            super::test_observer::observe("lock_acquired", &common.to_string_lossy())?;
        }
        super::test_observer::observe("guard_acquired", "")?;
        Ok(guard)
    }
    fn recover(&self, records: &[std::path::PathBuf], warnings: &mut Vec<String>) -> Result<bool> {
        let mut changed = false;
        for records in records.iter().collect::<std::collections::BTreeSet<_>>() {
            let common = crate::record_commit::common_dir(records)
                .map_err(Error::RetainedPublicationUnsupported)?;
            let lock = self
                .locks
                .get(&common)
                .ok_or_else(|| invalid("publication repository changed under held lock"))?;
            let recovery = lock
                .recover(records)
                .map_err(Error::RetainedHistoryBlocked)?;
            changed |= !recovery.unstaged.is_empty();
            if let Some(warning) = crate::record_commit::recovery_warning(&recovery.unstaged) {
                warnings.push(warning);
            }
            warnings.extend(recovery.warnings);
            let (refused, notes) =
                lock.commit_paths(records, &[], "chore(tasks): retained preflight");
            warnings.extend(notes);
            if !refused.is_empty() {
                return Err(Error::RetainedHistoryBlocked(refused.join("; ")));
            }
        }
        Ok(changed)
    }
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        for (common, lock) in std::mem::take(&mut self.locks) {
            drop(lock);
            let _ = super::test_observer::observe("lock_released", &common.to_string_lossy());
        }
        let _ = super::test_observer::observe("guard_released", "");
    }
}

fn batch_inner(
    ctx: &mut OperationCtx,
    id: &crate::model::TaskId,
    artifacts: &[NewArtifact],
    notes: &[GeneratedNote],
) -> Result<Published> {
    let scope = phase_scope(notes, id, ctx.committed_release)?;
    for note in notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let operation = decode_operation(raw)?;
            if operation.key != ctx.key
                || operation.input_bases.len() != ctx.tasks.len()
                || operation
                    .input_bases
                    .iter()
                    .any(|input| !ctx.tasks.keys().any(|id| id.to_string() == input.task))
            {
                return Err(invalid(
                    "publication operation differs from opened participants",
                ));
            }
        }
    }
    let ids = match scope {
        PhaseScope::Target => vec![id.clone()],
        PhaseScope::All => ctx.tasks.keys().cloned().collect(),
    };
    let mut warmed = BTreeMap::new();
    for participant in &ids {
        let project = ctx.project(participant);
        let input = ctx.preimage_basis(participant)?.clone();
        let original = super::basis::read_exact(project, participant, &input)?;
        ctx.metrics.history_bytes_read += original.history_bytes_read;
        let project = ctx.project(participant);
        let current = super::basis::read(project, participant, Some("HEAD"))?;
        let mut published = false;
        for note in &original.task.notes {
            if let Some(raw) = note.text.strip_prefix("retention operation: ") {
                published |= decode_operation(raw)?.key == ctx.key;
            }
        }
        let policy = if published {
            super::freshness::ProgressPolicy::PendingNotes
        } else {
            super::freshness::ProgressPolicy::Exact
        };
        let warm = super::freshness::warm(
            project,
            participant,
            &input,
            &current.basis,
            &original.task,
            policy,
        )?;
        if let Some(warm) = warm {
            warmed.insert(participant.clone(), warm);
        }
    }
    super::test_observer::observe("prechecked", &id.to_string())?;
    let records = ids
        .iter()
        .map(|id| ctx.project(id).records.clone())
        .collect::<Vec<_>>();
    let guard = PhaseGuard::acquire(&records)?;
    for participant in &ids {
        if repository_identity(ctx.project(participant))? != ctx.repositories[&participant.prefix] {
            return Err(invalid("publication repository identity changed"));
        }
    }
    if guard.recover(&records, &mut ctx.warnings)? {
        warmed.clear();
        ctx.retry = true;
    }
    for participant in &ids {
        let context = ctx
            .contexts
            .get_mut(&participant.prefix)
            .expect("participant context");
        let old = context.project.clone();
        crate::commands::revalidate_retained_ctx(context)?;
        super::freshness::context(&old)?;
        context.project = super::read::inherit(&super::read::live(&old), context.project.clone())?;
        if repository_identity(&context.project)? != ctx.repositories[&participant.prefix] {
            return Err(invalid("publication repository identity changed"));
        }
        super::read::refresh(&context.project)?;
        crate::commands::retained_owner(context, participant)?;
        let project = context.project.clone();
        let input = ctx.preimage_basis(participant)?.clone();
        preimage_checked(
            &project,
            participant,
            &input,
            &ctx.key,
            &mut ctx.metrics,
            warmed.get(participant),
        )?;
        if project.read_task(participant)? != ctx.tasks[participant].task {
            return Err(Error::RetainedStaleBasis(
                "Participant changed between retained phases".into(),
            ));
        }
    }
    batch_guarded(ctx, id, artifacts, notes, &guard)
}

fn batch_guarded(
    ctx: &mut OperationCtx,
    id: &crate::model::TaskId,
    artifacts: &[NewArtifact],
    notes: &[GeneratedNote],
    guard: &PhaseGuard,
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
    let locked_tip = super::basis::resolve(&context.project, "HEAD")?;
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
    let mut may_seed = !ctx.retry && recovery.is_none();
    if let Some(recovery) = &recovery {
        task = recovery.base.clone();
    }
    may_seed &= !notes.iter().any(|new| {
        task.notes
            .iter()
            .any(|old| old.at == new.at && old.text == new.text)
    });
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
    ctx.retry |= reused.is_some() || recovery.is_some();
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
        let mut previous = previous;
        for generated in notes {
            if let Some(raw) = generated.text.strip_prefix("retention operation: ") {
                let note = decode_operation(raw)?;
                validate_transition(previous.as_ref(), &note)?;
                previous = Some(note);
            }
        }
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
    let (refused, warnings) = guard
        .locks
        .get(&common)
        .ok_or_else(|| invalid("publication repository changed under held lock"))?
        .commit_paths(
            &context.project.records,
            &paths,
            &format!("chore(tasks): retained {} {}", id, operation.phase),
        );
    ctx.warnings.extend(warnings);
    if !refused.is_empty() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Operation {}/{} publication incomplete at {}: {}",
            ctx.key.coordinator,
            ctx.key.operation_id,
            operation.phase,
            refused.join("; ")
        )));
    }
    stop_phase("commit", &operation.phase);
    super::read::refresh(&context.project)?;
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
    if publication.phase == "sender_released" {
        ctx.committed_release = true;
    }
    if may_seed && reused.is_none() && pinned.basis.commit != locked_tip {
        let parents = super::basis::git(
            &context.project,
            &["rev-list", "--parents", "-n", "1", &pinned.basis.commit],
        )?;
        let parents = std::str::from_utf8(&parents).map_err(invalid)?;
        if parents.split_whitespace().nth(1) == Some(locked_tip.as_str()) {
            let project = context.project.clone();
            for i in start..start + notes.len() {
                if let Some(raw) = pinned.task.notes[i]
                    .text
                    .strip_prefix("retention operation: ")
                {
                    let note = decode_operation(raw)?;
                    let input = super::lineage::participant(&project, &note, id)?;
                    // A historical locator after rename still needs the existing search.
                    let mut original = input.basis.clone();
                    original.commit = pinned.basis.commit.clone();
                    if original == pinned.basis {
                        let key = publication_key(
                            &project,
                            id,
                            &ctx.key,
                            &input.basis,
                            &pinned.task.notes[..=i],
                            &pinned.basis,
                        )?;
                        ctx.remember_publication(&project, key, original, pinned.basis.clone())?;
                    }
                }
            }
        }
    }
    ctx.phase_bases.insert(id.clone(), pinned.basis.clone());
    ctx.tasks.insert(id.clone(), pinned);
    super::test_observer::observe("readback", &publication.phase)?;
    Ok(Published {
        publication,
        sources,
    })
}

fn publication_key(
    project: &Project,
    id: &crate::model::TaskId,
    operation: &OperationKey,
    context: &Basis,
    prefix: &[crate::model::Note],
    tip: &Basis,
) -> Result<Vec<u8>> {
    let repository = super::basis::git(
        project,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
        ],
    )?;
    let mut current = tip.clone();
    current.commit.clear();
    serde_json::to_vec(&(
        repository,
        &project.records,
        &project.checkout,
        &project.retention_lineage,
        project.attachments_max_bytes,
        id,
        operation,
        context,
        prefix,
        current,
    ))
    .map_err(invalid)
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
    fn remember_publication(
        &mut self,
        project: &Project,
        key: Vec<u8>,
        basis: Basis,
        tip: Basis,
    ) -> Result<()> {
        // Replacing an entry first returns its charge to the shared invocation budget.
        self.original_publications.remove(&key);
        let value = serde_json::to_vec(&(&basis, &tip)).map_err(invalid)?;
        if let Some(bytes) = key.len().checked_add(value.len())
            && let Some(permit) = super::read::reserve_memo(project, bytes)?
        {
            self.original_publications.insert(
                key,
                OriginalPublication {
                    basis,
                    tip,
                    _permit: permit,
                },
            );
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
        let project = super::freshness::context(self.project(id))?;
        super::read::refresh(&project)?;
        let tip = super::basis::read(&project, id, Some("HEAD"))?;
        self.metrics.history_bytes_read += tip.history_bytes_read;
        let mut publications = Vec::new();
        for (i, n) in task.notes.iter().enumerate() {
            if let Some(raw) = n.text.strip_prefix("retention operation: ") {
                let note = decode_operation(raw)?;
                if note.key == self.key {
                    let input = super::lineage::participant(&project, &note, id)?;
                    let historical_task = input.task.clone();
                    let context = input.basis.clone();
                    let key = publication_key(
                        &project,
                        id,
                        &self.key,
                        &context,
                        &task.notes[..=i],
                        &tip.basis,
                    )?;
                    super::basis::validate_locator(&project, id, &context)?;
                    let cached = match self.original_publications.get(&key) {
                        Some(entry) if entry.tip == tip.basis => Some(entry.basis.clone()),
                        Some(entry) => {
                            let chain = super::basis::git(
                                &project,
                                &["rev-list", "--first-parent", &tip.basis.commit],
                            )?;
                            let chain = std::str::from_utf8(&chain).map_err(invalid)?;
                            if chain.lines().any(|commit| commit == entry.tip.commit) {
                                let previous = super::basis::read_exact(&project, id, &entry.tip)?;
                                self.metrics.history_bytes_read += previous.history_bytes_read;
                                match super::freshness::unchanged(
                                    &project,
                                    id,
                                    &entry.tip,
                                    &tip.basis,
                                    &previous.task,
                                ) {
                                    Ok(()) => Some(entry.basis.clone()),
                                    Err(Error::RetainedStaleBasis(_)) => None,
                                    Err(error) => return Err(error),
                                }
                            } else {
                                None
                            }
                        }
                        None => None,
                    };
                    let basis = match cached {
                        Some(basis) => basis,
                        None => super::basis::publication_basis(
                            &project,
                            id,
                            &context,
                            &task.notes[..=i],
                            &mut self.metrics.history_bytes_read,
                        )?,
                    };
                    self.remember_publication(&project, key, basis.clone(), tip.basis.clone())?;
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
// Recover P from published history, never from open's current HEAD or a staged phase.
pub(super) fn recover_phase_bases(ctx: &mut OperationCtx, binding: &Binding) -> Result<()> {
    recover_bases(ctx, binding, None)
}
pub(super) fn recover_reconciliation_bases(
    ctx: &mut OperationCtx,
    binding: &Binding,
    inputs: &[Input],
) -> Result<()> {
    recover_bases(ctx, binding, Some(inputs))
}
fn recover_bases(
    ctx: &mut OperationCtx,
    binding: &Binding,
    reconciliation: Option<&[Input]>,
) -> Result<()> {
    let mut bases = BTreeMap::new();
    let mut durable = false;
    for input in &binding.inputs {
        let id = crate::model::TaskId::parse(&input.task)?;
        let project = ctx.project(&id).clone();
        let head = super::basis::read_exact(&project, &id, &ctx.tasks[&id].basis)?;
        ctx.metrics.history_bytes_read += head.history_bytes_read;
        let mut phase = None;
        for (index, entry) in head.task.notes.iter().enumerate() {
            if let Some(raw) = entry.text.strip_prefix("retention operation: ") {
                let note = decode_operation(raw)?;
                if note.key == ctx.key {
                    if note.input_bases != binding.inputs || note.host != binding.host {
                        return Err(invalid(
                            "durable phase differs from its immutable request binding",
                        ));
                    }
                    phase = Some((index, note));
                }
            }
        }
        let reconciling = phase.as_ref().is_some_and(|(_, note)| {
            matches!(
                note.phase.as_str(),
                "reconcile_intent" | "reconcile_ready" | "reconciled"
            )
        });
        let mut basis = if let Some((index, note)) = phase {
            let basis = super::basis::publication_basis(
                &project,
                &id,
                &input.basis,
                &head.task.notes[..=index],
                &mut ctx.metrics.history_bytes_read,
            )?;
            let publication = super::basis::read_exact(&project, &id, &basis)?;
            ctx.metrics.history_bytes_read += publication.history_bytes_read;
            if publication.task.notes.get(index) != head.task.notes.get(index) {
                return Err(invalid(
                    "durable phase readback differs from its publication",
                ));
            }
            let locator = note.request.pinned(&id.to_string(), &basis)?;
            if super::verify::artifact(&project, &locator, &mut ctx.metrics)? != binding.bytes {
                return Err(invalid(
                    "durable phase request differs from its original exact bytes",
                ));
            }
            durable = true;
            basis
        } else {
            input.basis.clone()
        };
        if let Some(inputs) = reconciliation
            && !reconciling
        {
            // A fresh directive explicitly pins the state being reconciled. Its own
            // durable phases subsequently become P, without changing original B.
            basis = inputs
                .iter()
                .find(|input| input.task == id.to_string())
                .ok_or_else(|| invalid("reconciliation participant missing"))?
                .basis
                .clone();
        }
        bases.insert(id, basis);
    }
    ctx.phase_bases = bases;
    ctx.durable_phase = durable;
    Ok(())
}

pub(super) fn new_admission(ctx: &OperationCtx) -> Result<()> {
    if ctx.durable_phase {
        return Ok(());
    }
    for id in ctx.tasks.keys() {
        let project = ctx.project(id);
        if project.kind == crate::repo::Kind::InTree {
            return Err(Error::RetainedPublicationUnsupported(format!(
                "New native retained custody for {id} at {} is unsupported in an in-tree records layout; use detached or linked records for new operations, or the manual route where supported. Existing retained history blocks records migration until supported recovery; manual accounting cannot bypass a pending operation or publication failure",
                project.records.display()
            )));
        }
    }
    Ok(())
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
            super::freshness::current(&project, &id, &old, true)?;
            if pinned.task != old.task {
                return Err(Error::RetainedStaleBasis(
                    "A new operation requires unchanged committed participant inputs".into(),
                ));
            }
        } else {
            let phase = ctx.preimage_basis(&id)?.clone();
            preimage(&project, &id, &phase, &ctx.key, &mut ctx.metrics)?;
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
        warnings: {
            let mut warnings = std::mem::take(&mut ctx.warnings);
            if replay {
                warnings.push(
                    "Completed operation replay; current filesystem bytes were not inspected"
                        .into(),
                );
            }
            warnings
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn open_test(
        dir: Option<&Path>,
        ids: &[String],
        key: &OperationKey,
        config: Arc<HostConfig>,
    ) -> Result<OperationCtx> {
        let mut ctx = super::open(dir, ids, key, config)?;
        ctx.phase_bases = ctx
            .tasks
            .iter()
            .map(|(id, task)| (id.clone(), task.basis.clone()))
            .collect();
        Ok(ctx)
    }

    const CLI_HELPER_TEST: &str = "retained::publish::tests::retained_publish_cli_helper";
    const CLI_JSON_PREFIX: &str = "TASKS_PUBLICATION_CLI_JSON:";

    fn isolated_env(home: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
        let mut vars: Vec<(&'static str, std::ffi::OsString)> = vec![
            ("PATH", std::env::var_os("PATH").expect("PATH is set")),
            ("HOME", home.into()),
            ("XDG_CONFIG_HOME", home.join("config").into_os_string()),
            ("XDG_STATE_HOME", home.join("state").into_os_string()),
            (
                "GIT_TRACE2_EVENT",
                home.join("publication-trace.jsonl").into_os_string(),
            ),
            ("USER", "tester".into()),
            ("LC_ALL", "C".into()),
            ("GIT_CONFIG_GLOBAL", "/dev/null".into()),
            ("GIT_CONFIG_NOSYSTEM", "1".into()),
            ("GIT_AUTHOR_NAME", "t".into()),
            ("GIT_AUTHOR_EMAIL", "t@t".into()),
            ("GIT_COMMITTER_NAME", "t".into()),
            ("GIT_COMMITTER_EMAIL", "t@t".into()),
        ];
        for key in ["GIT_CEILING_DIRECTORIES", "TMPDIR"] {
            if let Some(value) = std::env::var_os(key) {
                vars.push((key, value));
            }
        }
        vars
    }

    fn git(home: &Path, dir: &Path, args: &[&str]) -> Vec<u8> {
        let out = crate::git::at(dir)
            .env_clear()
            .envs(isolated_env(home))
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        out.stdout
    }

    fn assert_repo(home: &Path, dir: &Path) {
        let read = |arg| {
            Path::new(
                std::str::from_utf8(&git(
                    home,
                    dir,
                    &["rev-parse", "--path-format=absolute", arg],
                ))
                .unwrap()
                .trim(),
            )
            .canonicalize()
            .unwrap()
        };
        let dir = dir.canonicalize().unwrap();
        assert_eq!(read("--show-toplevel"), dir);
        assert!(read("--absolute-git-dir").starts_with(&dir));
        assert!(read("--git-common-dir").starts_with(&dir));
    }

    #[test]
    fn retained_publish_cli_helper() {
        let Ok(dir) = std::env::var("TASKS_PUBLICATION_CLI_DIR") else {
            return;
        };
        let args: Vec<String> =
            serde_json::from_str(&std::env::var("TASKS_PUBLICATION_CLI_ARGS").unwrap()).unwrap();
        let mut argv = vec!["tasks".to_string(), "-C".to_string(), dir];
        argv.extend(args);
        let cli = <crate::cli::Cli as clap::Parser>::try_parse_from(argv).unwrap();
        let host = std::sync::Arc::new(crate::config::HostConfig::default());
        let out = match crate::commands::run(cli, Some(host)) {
            Ok(out) => out,
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        };
        let failed_check = matches!(&out, crate::output::Output::Check(c) if !c.errors.is_empty());
        let rendered = crate::output::render(
            &out,
            crate::output::Format::Json,
            &crate::style::Painter::new(
                crate::style::ColorMode::Never,
                crate::output::Format::Json,
                false,
            ),
            &crate::table::Layout {
                width: None,
                headers: false,
            },
        );
        println!(
            "{CLI_JSON_PREFIX}{}",
            serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&rendered).unwrap())
                .unwrap()
        );
        if failed_check {
            std::process::exit(1);
        }
    }

    fn run_cli(home: &Path, dir: &Path, args: &[&str]) -> serde_json::Value {
        let exe = std::env::current_exe().unwrap();
        let out = std::process::Command::new(exe)
            .env_clear()
            .envs(isolated_env(home))
            .env("TASKS_SESSION", "retained-fixture")
            .env("TASKS_PUBLICATION_CLI_DIR", dir)
            .env(
                "TASKS_PUBLICATION_CLI_ARGS",
                serde_json::to_string(args).unwrap(),
            )
            .args(["--exact", CLI_HELPER_TEST, "--nocapture"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).unwrap();
        let line = stdout
            .lines()
            .find_map(|line| line.strip_prefix(CLI_JSON_PREFIX))
            .unwrap_or_else(|| panic!("missing JSON output in {stdout:?}"));
        serde_json::from_str(line).unwrap()
    }

    fn fixture(test: &str) -> Option<(PathBuf, crate::model::TaskId)> {
        if let Ok(dir) = std::env::var("TASKS_PUBLICATION_FIXTURE") {
            return Some((
                dir.into(),
                crate::model::TaskId::parse(&std::env::var("TASKS_PUBLICATION_ID").unwrap())
                    .unwrap(),
            ));
        }
        let home = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("code");
        std::fs::create_dir_all(&dir).unwrap();
        git(home.path(), &dir, &["init", "-q", "-b", "main"]);
        assert_repo(home.path(), &dir);
        std::fs::write(dir.join("README.md"), "Fixture\n").unwrap();
        git(home.path(), &dir, &["add", "README.md"]);
        git(home.path(), &dir, &["commit", "-qm", "seed"]);
        run_cli(home.path(), &dir, &["init", "--prefix", "sci"]);
        let config = dir.join("tasks/.config.toml");
        let mut value: toml::Value =
            toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        value.as_table_mut().unwrap().insert(
            "git".into(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "commit".into(),
                toml::Value::Boolean(true),
            )])),
        );
        std::fs::write(&config, toml::to_string(&value).unwrap()).unwrap();
        git(home.path(), &dir, &["add", "."]);
        git(home.path(), &dir, &["commit", "-qm", "configuration"]);
        let id = run_cli(home.path(), &dir, &["add", "Retained fixture"])["id"]
            .as_str()
            .unwrap()
            .to_string();
        run_cli(home.path(), &dir, &["start", &id]);
        let exe = std::env::current_exe().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let hook = dir.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nTASKS_PUBLICATION_CLI_DIR={} TASKS_PUBLICATION_CLI_ARGS='[\"check\"]' exec {} --exact {CLI_HELPER_TEST} --nocapture >/dev/null\n",
                quote(dir.to_str().unwrap()),
                quote(exe.to_str().unwrap()),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let extra = tempfile::tempdir().unwrap();
        let second = if test.ends_with("same_git_lock") || test.contains("retained_phase_scope") {
            git(home.path(), extra.path(), &["init", "-qb", "main"]);
            assert_repo(home.path(), extra.path());
            run_cli(
                home.path(),
                extra.path(),
                &[
                    "init",
                    "--prefix",
                    "fam",
                    "--records",
                    dir.to_str().unwrap(),
                ],
            );
            let config = dir.join("fam/tasks/.config.toml");
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
            git(home.path(), &dir, &["add", "fam/tasks/.config.toml"]);
            git(
                home.path(),
                &dir,
                &["commit", "-qm", "second records configuration"],
            );
            let id = run_cli(
                home.path(),
                &dir,
                &["add", "Second participant", "--project", "fam"],
            )["id"]
                .as_str()
                .unwrap()
                .to_string();
            run_cli(home.path(), &dir, &["start", &id]);
            Some(id)
        } else {
            None
        };
        let command = || {
            let mut c = std::process::Command::new(std::env::current_exe().unwrap());
            c.env_clear()
                .envs(isolated_env(home.path()))
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_PUBLICATION_FIXTURE", &dir)
                .env("TASKS_PUBLICATION_ID", &id)
                .args(["--exact", test, "--nocapture"]);
            if let Some(second) = &second {
                c.env("TASKS_PUBLICATION_SECOND", second);
            }
            c
        };
        if test.ends_with("same_git_lock") {
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
    #[test]
    fn retained_freshness_commit_range() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_freshness_commit_range")
        else {
            return;
        };
        // This predicate fixture deliberately commits raw trees, not attachment ledgers.
        std::fs::remove_file(dir.join(".git/hooks/pre-commit")).unwrap();
        let home = std::env::var_os("HOME").unwrap();
        let home = Path::new(&home);
        let project = Project::open(&dir).unwrap();
        let files = crate::attachments::task_dir(&project, &id);
        std::fs::create_dir_all(&files).unwrap();
        std::fs::write(files.join("evidence"), "original").unwrap();
        git(home, &dir, &["add", "."]);
        git(home, &dir, &["commit", "-qm", "protected file"]);
        let pinned = super::super::basis::read(&project, &id, None).unwrap();
        let from = &pinned.basis;
        let check = || {
            let mut to = from.clone();
            to.commit = super::super::basis::resolve(&project, "HEAD").unwrap();
            super::super::freshness::unchanged(&project, &id, from, &to, &pinned.task)
        };
        assert!(check().is_ok(), "same basis");
        for case in [
            "unrelated",
            "record_bytes",
            "attachment_content",
            "attachment_mode",
            "attachment_add",
            "attachment_delete",
            "archive_change",
            "ABA",
            "merged_side_edit",
            "merge_other_parent",
        ] {
            git(home, &dir, &["reset", "--hard", &from.commit]);
            git(home, &dir, &["clean", "-fd"]);
            let path = match case {
                "unrelated" => dir.join("README.md"),
                "record_bytes" => project.task_path(&id),
                "attachment_add" => files.join("extra"),
                "archive_change" => files.join(format!("notes-{}.notes", "a".repeat(64))),
                _ => files.join("evidence"),
            };
            if case == "attachment_delete" {
                std::fs::remove_file(&path).unwrap();
            } else if case == "attachment_mode" {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            } else if case == "merged_side_edit" || case == "merge_other_parent" {
                git(home, &dir, &["checkout", "-B", "side"]);
                std::fs::write(&path, "changed").unwrap();
                git(home, &dir, &["add", "."]);
                git(home, &dir, &["commit", "-qm", "side edit"]);
                if case == "merged_side_edit" {
                    std::fs::write(&path, "original").unwrap();
                    git(home, &dir, &["add", "."]);
                    git(home, &dir, &["commit", "-qm", "side revert"]);
                }
                git(home, &dir, &["checkout", "main"]);
                std::fs::write(dir.join("README.md"), "unrelated main").unwrap();
                git(home, &dir, &["add", "."]);
                git(home, &dir, &["commit", "-qm", "main edit"]);
                git(
                    home,
                    &dir,
                    &[
                        "merge",
                        "--no-ff",
                        "-s",
                        "ours",
                        "side",
                        "-m",
                        "merge retaining main",
                    ],
                );
            } else {
                let old = std::fs::read(&path).unwrap_or_default();
                let mut bytes = old.clone();
                bytes.extend_from_slice(b"\n");
                std::fs::write(&path, bytes).unwrap();
                if case == "ABA" {
                    git(home, &dir, &["add", "."]);
                    git(home, &dir, &["commit", "-qm", "intervening edit"]);
                    std::fs::write(&path, old).unwrap();
                }
            }
            if !case.starts_with("merge") {
                git(home, &dir, &["add", "."]);
                git(home, &dir, &["commit", "-qm", case]);
            }
            let result = check();
            if case == "unrelated" {
                assert!(result.is_ok(), "{case}: {result:?}");
            } else {
                assert!(
                    matches!(result, Err(Error::RetainedStaleBasis(_))),
                    "{case}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn retained_freshness_refusal() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_freshness_refusal")
        else {
            return;
        };
        let home = std::env::var_os("HOME").unwrap();
        let home = Path::new(&home);
        let project = Project::open(&dir).unwrap();
        let pinned = super::super::basis::read(&project, &id, None).unwrap();
        for field in ["branch", "project", "layout", "subdir", "path", "missing"] {
            let mut to = pinned.basis.clone();
            match field {
                "branch" => to.retained_ref = "refs/heads/other".into(),
                "project" => to.project = "other".into(),
                "layout" => to.layout = "detached".into(),
                "subdir" => to.records_subdir = "other".into(),
                "path" => to.task_path = "tasks/sci-000002.md".into(),
                "missing" => to.commit = "0".repeat(40),
                _ => unreachable!(),
            }
            assert!(
                super::super::freshness::unchanged(&project, &id, &pinned.basis, &to, &pinned.task)
                    .is_err(),
                "{field}"
            );
        }
        git(home, &dir, &["checkout", "-qb", "divergent"]);
        std::fs::write(dir.join("side"), "side").unwrap();
        git(home, &dir, &["add", "."]);
        git(home, &dir, &["commit", "-qm", "side"]);
        let mut from = pinned.basis.clone();
        from.commit = super::super::basis::resolve(&project, "HEAD").unwrap();
        git(home, &dir, &["checkout", "main"]);
        assert!(
            super::super::freshness::unchanged(&project, &id, &from, &pinned.basis, &pinned.task)
                .is_err()
        );
        std::fs::rename(dir.join(".git"), dir.join("hidden-git")).unwrap();
        assert!(
            super::super::freshness::unchanged(
                &project,
                &id,
                &pinned.basis,
                &pinned.basis,
                &pinned.task
            )
            .is_err()
        );
    }

    #[test]
    fn retained_state_concurrent_unrelated() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_state_concurrent_unrelated")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let project = Project::open(&dir).unwrap();
        let original = super::super::basis::read(&project, &id, None).unwrap();
        for case in ["unrelated", "participant", "ABA", "branch"] {
            git(&home, &dir, &["checkout", "main"]);
            git(&home, &dir, &["reset", "--hard", &original.basis.commit]);
            let cli = <crate::cli::Cli as clap::Parser>::try_parse_from([
                "tasks",
                "-C",
                dir.to_str().unwrap(),
                "retained",
                "state",
                &id.to_string(),
            ])
            .unwrap();
            let dir_copy = dir.clone();
            let home_copy = home.clone();
            let path = project.task_path(&id);
            crate::commands::retained::BEFORE_STATE_RETURN.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    if case == "branch" {
                        git(&home_copy, &dir_copy, &["checkout", "-B", "other"]);
                        return;
                    }
                    let path = if case == "unrelated" {
                        dir_copy.join("README.md")
                    } else {
                        path
                    };
                    let original = std::fs::read(&path).unwrap();
                    let mut edited = original.clone();
                    edited.push(b'\n');
                    std::fs::write(&path, edited).unwrap();
                    git(&home_copy, &dir_copy, &["add", "."]);
                    git(
                        &home_copy,
                        &dir_copy,
                        &["commit", "-qm", "concurrent commit"],
                    );
                    if case == "ABA" {
                        std::fs::write(&path, original).unwrap();
                        git(&home_copy, &dir_copy, &["add", "."]);
                        git(&home_copy, &dir_copy, &["commit", "-qm", "revert"]);
                    }
                }))
            });
            let result = crate::commands::run(cli, Some(Arc::new(HostConfig::default())));
            if case == "unrelated" {
                let crate::output::Output::RetainedState(out) = result.unwrap() else {
                    panic!("state output")
                };
                let historical =
                    super::super::state::replay(&project, &original.task, &original.basis, None)
                        .unwrap();
                assert_eq!(
                    serde_json::to_value(out).unwrap(),
                    serde_json::to_value(historical).unwrap()
                );
            } else {
                assert!(
                    matches!(result, Err(Error::RetainedStaleBasis(_))),
                    "{case} must refuse"
                );
            }
        }
    }

    #[test]
    fn retained_admission_no_history_walk() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_admission_no_history_walk")
        else {
            return;
        };
        let p = Project::open(&dir).unwrap();
        let input = super::super::basis::read(&p, &id, None).unwrap().basis;
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        std::fs::write(dir.join("README.md"), "foreign record commit").unwrap();
        git(&home, &dir, &["add", "README.md"]);
        git(&home, &dir, &["commit", "-qm", "unrelated"]);
        let key = OperationKey {
            coordinator: id.to_string(),
            operation_id: "d".repeat(32),
        };
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let calls = super::super::proof::HISTORY_CALLS.with(|n| n.get());
        validate_inputs(
            &mut ctx,
            &[Input {
                task: id.to_string(),
                basis: input,
            }],
        )
        .unwrap();
        assert_eq!(super::super::proof::HISTORY_CALLS.with(|n| n.get()), calls);
    }

    #[test]
    fn retained_phase_freshness_cached_view() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_freshness_cached_view")
        else {
            return;
        };
        let p = Project::open(&dir).unwrap();
        let cached = super::super::read::start(&p).unwrap();
        let pinned = super::super::basis::read(&cached, &id, Some("HEAD")).unwrap();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let path = p.task_path(&id);
        let mut raw = std::fs::read(&path).unwrap();
        raw.push(b'\n');
        std::fs::write(&path, raw).unwrap();
        git(&home, &dir, &["add", "tasks"]);
        git(&home, &dir, &["commit", "-qm", "participant bytes"]);
        assert!(
            preimage(
                &cached,
                &id,
                &pinned.basis,
                &OperationKey {
                    coordinator: id.to_string(),
                    operation_id: "0".repeat(32)
                },
                &mut super::super::verify::Metrics::default()
            )
            .is_err(),
            "cached HEAD must not hide participant bytes"
        );
    }

    #[test]
    fn retained_phase_freshness_configuration() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_freshness_configuration")
        else {
            return;
        };
        let p = Project::open(&dir).unwrap();
        let pinned = super::super::basis::read(&p, &id, None).unwrap();
        let path = dir.join("tasks/.config.toml");
        let raw = std::fs::read_to_string(&path).unwrap();
        for setting in ["commit", "cap"] {
            let mut config: toml::Value = toml::from_str(&raw).unwrap();
            if setting == "commit" {
                config["git"]["commit"] = toml::Value::Boolean(false);
            } else {
                config.as_table_mut().unwrap().insert(
                    "attachments".into(),
                    toml::Value::try_from(serde_json::json!({"max_bytes":1})).unwrap(),
                );
            }
            std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
            assert!(
                preimage(
                    &p,
                    &id,
                    &pinned.basis,
                    &OperationKey {
                        coordinator: id.to_string(),
                        operation_id: "0".repeat(32)
                    },
                    &mut super::super::verify::Metrics::default()
                )
                .is_err(),
                "changed {setting} must not reuse old publication configuration"
            );
        }
    }

    #[test]
    fn retained_phase_freshness_readback() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_freshness_readback")
        else {
            return;
        };
        let (key, a, n) = example(&id);
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let before = ctx.phase_bases[&id].clone();
        std::fs::write(
            dir.join(".git/hooks/pre-commit"),
            format!(
                "#!/bin/sh\nprintf tampered > 'tasks/files/{0}/{1}'\ngit add -- 'tasks/files/{0}/{1}'\n",
                id, a.name
            ),
        )
        .unwrap();
        assert!(batch(&mut ctx, &id, &[a], &[n]).is_err());
        assert_eq!(
            ctx.phase_bases[&id], before,
            "failed artifact readback cannot advance P"
        );
    }

    #[test]
    fn retained_phase_freshness_recover_committed_and_staged() {
        let Some((dir, id)) = fixture(
            "retained::publish::tests::retained_phase_freshness_recover_committed_and_staged",
        ) else {
            return;
        };
        let (key, a, n) = example(&id);
        let operation =
            decode_operation(n.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let binding = Binding {
            bytes: a.bytes.clone(),
            request: operation.request.clone(),
            name: a.name.clone(),
            inputs: operation.input_bases.clone(),
            host: operation.host.clone(),
        };
        let original = binding.inputs[0].basis.clone();
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let hook = dir.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(
            batch(
                &mut ctx,
                &id,
                &[NewArtifact {
                    name: a.name.clone(),
                    bytes: a.bytes.clone()
                }],
                &[GeneratedNote {
                    at: n.at.clone(),
                    text: n.text.clone()
                }]
            )
            .is_err()
        );
        drop(ctx);
        let mut ctx = super::open(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        recover_phase_bases(&mut ctx, &binding).unwrap();
        assert_eq!(ctx.phase_bases[&id], original);
        assert!(!ctx.durable_phase, "staged intent is not durable");
        std::fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
        let p = batch(&mut ctx, &id, &[a], &[n]).unwrap().publication.basis;
        assert_eq!(ctx.phase_bases[&id], p);
        drop(ctx);
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        std::fs::write(dir.join("README.md"), "unrelated").unwrap();
        git(&home, &dir, &["add", "README.md"]);
        git(&home, &dir, &["commit", "-qm", "unrelated"]);
        let mut ctx = super::open(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        assert_ne!(ctx.tasks[&id].basis.commit, p.commit);
        recover_phase_bases(&mut ctx, &binding).unwrap();
        assert_eq!(ctx.phase_bases[&id], p);
        assert!(ctx.durable_phase);
        ctx.revalidate().unwrap();
    }

    fn publication_searches(home: &Path) -> usize {
        std::fs::read_to_string(home.join("publication-trace.jsonl"))
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|event| {
                event["event"] == "start"
                    && event["argv"].as_array().is_some_and(|args| {
                        args.iter().any(|arg| arg == "log")
                            && args.iter().any(|arg| arg == "--first-parent")
                    })
            })
            .count()
    }

    fn memo_fixture(
        dir: &Path,
        id: &crate::model::TaskId,
    ) -> (OperationCtx, NewArtifact, GeneratedNote) {
        let (key, artifact, note) = example(id);
        let ctx = open_test(
            Some(dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        (ctx, artifact, note)
    }

    #[test]
    fn retained_publication_memo_original_commit() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_publication_memo_original_commit")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let original = batch(
            &mut ctx,
            &id,
            &[artifact],
            &[GeneratedNote {
                at: note.at.clone(),
                text: note.text.clone(),
            }],
        )
        .unwrap()
        .publication
        .basis;
        let before = publication_searches(&home);
        let memo_basis = ctx.phase_publications(&id).unwrap()[0].basis.clone();
        assert_eq!(
            publication_searches(&home),
            before,
            "a verified new append must avoid a publication search"
        );
        let plain = Project::open(&dir).unwrap();
        let receipt =
            decode_operation(note.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let uncached_publication_basis = super::super::basis::publication_basis(
            &plain,
            &id,
            &receipt.input_bases[0].basis,
            &ctx.tasks[&id].task.notes,
            &mut 0,
        )
        .unwrap();
        assert_eq!(memo_basis, uncached_publication_basis);
        assert_eq!(memo_basis, original);
        std::fs::write(dir.join("README.md"), "unrelated").unwrap();
        git(&home, &dir, &["add", "README.md"]);
        git(&home, &dir, &["commit", "-qm", "unrelated"]);
        let before = publication_searches(&home);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert_eq!(
            publication_searches(&home),
            before,
            "unchanged first-parent extension reuses original proof"
        );
        let no_op_retry_head = batch(&mut ctx, &id, &[], &[note])
            .unwrap()
            .publication
            .basis
            .commit;
        assert_ne!(no_op_retry_head, original.commit);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert!(
            ctx.retry,
            "a no-op retry cannot later seed a missing receipt"
        );
        let mut progress = ctx.tasks[&id].task.clone();
        crate::commands::append_note(&mut progress, "tester", "Ordinary progress").unwrap();
        let context = &ctx.contexts[&id.prefix];
        context
            .project
            .write_task(&context.registry, &progress)
            .unwrap();
        crate::written::take();
        git(&home, &dir, &["add", "tasks"]);
        git(&home, &dir, &["commit", "-qm", "ordinary progress"]);
        let before = publication_searches(&home);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert!(
            publication_searches(&home) > before,
            "ordinary appended notes require a fresh exact publication search"
        );
    }

    #[test]
    fn retained_publication_memo_first_parent() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_publication_memo_first_parent")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let input = ctx.tasks[&id].basis.clone();
        let original = batch(&mut ctx, &id, &[artifact], &[note])
            .unwrap()
            .publication
            .basis;
        let before = publication_searches(&home);
        ctx.phase_publications(&id).unwrap();
        assert_eq!(
            publication_searches(&home),
            before,
            "new publication must seed original proof"
        );
        let tree = String::from_utf8(git(
            &home,
            &dir,
            &["rev-parse", &format!("{}^{{tree}}", original.commit)],
        ))
        .unwrap();
        let merge = String::from_utf8(git(
            &home,
            &dir,
            &[
                "commit-tree",
                tree.trim(),
                "-p",
                &input.commit,
                "-p",
                &original.commit,
                "-m",
                "second-parent publication",
            ],
        ))
        .unwrap();
        git(
            &home,
            &dir,
            &["update-ref", "refs/heads/main", merge.trim()],
        );
        let before = publication_searches(&home);
        let actual = ctx.phase_publications(&id).unwrap()[0].basis.clone();
        let second_parent_only_requires_search = publication_searches(&home) > before;
        assert!(second_parent_only_requires_search);
        assert_eq!(actual.commit, merge.trim());
        assert_ne!(actual, original);
    }

    #[test]
    fn retained_publication_memo_recovery() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_publication_memo_recovery")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let key = ctx.key.clone();
        let copy_note = || GeneratedNote {
            at: note.at.clone(),
            text: note.text.clone(),
        };
        let original = batch(&mut ctx, &id, &[artifact], &[copy_note()])
            .unwrap()
            .publication
            .basis;
        drop(ctx);
        std::fs::write(dir.join("README.md"), "unrelated retry").unwrap();
        git(&home, &dir, &["add", "README.md"]);
        git(&home, &dir, &["commit", "-qm", "unrelated retry"]);
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let no_op_retry_head = batch(&mut ctx, &id, &[], &[copy_note()])
            .unwrap()
            .publication
            .basis
            .commit;
        assert_ne!(no_op_retry_head, original.commit);
        let before = publication_searches(&home);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert!(
            publication_searches(&home) > before,
            "retry must resolve original publication"
        );
        let before = publication_searches(&home);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert_eq!(
            publication_searches(&home),
            before,
            "successful searched proof is reusable"
        );
    }

    #[test]
    fn retained_publication_memo_budget() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_publication_memo_budget")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let mut permits = Vec::new();
        while let Some(permit) = super::super::read::reserve_memo(ctx.project(&id), 0).unwrap() {
            permits.push(permit);
        }
        let original = batch(&mut ctx, &id, &[artifact], &[note])
            .unwrap()
            .publication
            .basis;
        assert!(ctx.original_publications.is_empty());
        let before = publication_searches(&home);
        for _ in 0..2 {
            assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        }
        assert_eq!(
            publication_searches(&home),
            before + 2,
            "saturated budget continues with exact uncached search"
        );
        assert!(ctx.original_publications.is_empty());
        drop(permits);
        ctx.phase_publications(&id).unwrap();
        assert_eq!(ctx.original_publications.len(), 1);
    }

    #[test]
    fn retained_publication_memo_staged_and_failed_readback() {
        let Some((dir, id)) = fixture(
            "retained::publish::tests::retained_publication_memo_staged_and_failed_readback",
        ) else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let key = ctx.key.clone();
        let copy_artifact = || NewArtifact {
            name: artifact.name.clone(),
            bytes: artifact.bytes.clone(),
        };
        let copy_note = || GeneratedNote {
            at: note.at.clone(),
            text: note.text.clone(),
        };
        let hook = dir.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(batch(&mut ctx, &id, &[copy_artifact()], &[copy_note()]).is_err());
        assert!(ctx.original_publications.is_empty());
        drop(ctx);
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        std::fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
        let original = batch(&mut ctx, &id, &[copy_artifact()], &[copy_note()])
            .unwrap()
            .publication
            .basis;
        assert!(
            ctx.original_publications.is_empty(),
            "staged recovery must not seed readback as an original proof"
        );
        let before = publication_searches(&home);
        assert_eq!(ctx.phase_publications(&id).unwrap()[0].basis, original);
        assert!(publication_searches(&home) > before);
        // A failed readback of a later phase must retain P and never seed that phase.
        let old = ctx.phase_bases[&id].clone();
        std::fs::write(&hook, format!("#!/bin/sh\nprintf tampered > 'tasks/files/{id}/{}'\ngit add -- 'tasks/files/{id}/{}'\n", artifact.name, artifact.name)).unwrap();
        let operation =
            decode_operation(note.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let binding = Binding {
            bytes: artifact.bytes.clone(),
            name: artifact.name.clone(),
            request: operation.request,
            inputs: operation.input_bases,
            host: operation.host,
        };
        let at = crate::time::after(&ctx.tasks[&id].task.updated).unwrap();
        let incomplete = receipt(
            &mut ctx,
            &binding,
            &id,
            PhaseOutputs::Incomplete(IncompleteOutput {
                attempt: 1,
                previous_phase: "intent".into(),
                checkpoint: None,
                action: "retry".into(),
            }),
            &at,
        )
        .unwrap();
        assert!(batch(&mut ctx, &id, &[copy_artifact()], &[incomplete]).is_err());
        assert_eq!(
            ctx.original_publications.len(),
            1,
            "failed readback must not seed a new phase"
        );
        assert_eq!(ctx.phase_bases[&id], old);
        assert!(ctx.retry);
    }

    #[test]
    fn retained_publication_memo_same_phase_attempts() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_publication_memo_same_phase_attempts")
        else {
            return;
        };
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let operation =
            decode_operation(note.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let binding = Binding {
            bytes: artifact.bytes.clone(),
            name: artifact.name.clone(),
            request: operation.request,
            inputs: operation.input_bases,
            host: operation.host,
        };
        let intent = batch(&mut ctx, &id, &[artifact], &[note])
            .unwrap()
            .publication
            .basis;
        let mut bases = vec![intent];
        for attempt in 1..=2 {
            let at = crate::time::after(&ctx.tasks[&id].task.updated).unwrap();
            let note = receipt(
                &mut ctx,
                &binding,
                &id,
                PhaseOutputs::Incomplete(IncompleteOutput {
                    attempt,
                    previous_phase: if attempt == 1 { "intent" } else { "incomplete" }.into(),
                    checkpoint: None,
                    action: "retry exact request".into(),
                }),
                &at,
            )
            .unwrap();
            bases.push(
                batch(&mut ctx, &id, &[], &[note])
                    .unwrap()
                    .publication
                    .basis,
            );
        }
        let before = publication_searches(&home);
        let publications = ctx.phase_publications(&id).unwrap();
        assert!(
            publication_searches(&home) >= before + 2,
            "own later phase invalidates older exact range proofs"
        );
        assert_eq!(
            publications
                .iter()
                .map(|p| p.basis.clone())
                .collect::<Vec<_>>(),
            bases
        );
        assert_ne!(publications[1].basis, publications[2].basis);
        assert_eq!(ctx.original_publications.len(), 3);
        let before = publication_searches(&home);
        ctx.phase_publications(&id).unwrap();
        assert_eq!(publication_searches(&home), before);
    }

    #[test]
    fn retained_publication_memo_nonparticipant_refuses() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_publication_memo_nonparticipant_refuses")
        else {
            return;
        };
        let (mut ctx, artifact, note) = memo_fixture(&dir, &id);
        let foreign = crate::model::TaskId::parse("fam-000000").unwrap();
        assert!(batch(&mut ctx, &foreign, &[artifact], &[note]).is_err());
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
        let mut ctx = open_test(
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
            let home = PathBuf::from(std::env::var_os("HOME").unwrap());
            git(&home, &dir, &["add", "foreign"]);
            assert!(
                open_test(
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
            let mut ctx = open_test(
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
            let mut ctx = open_test(
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
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string()],
            &key,
            std::sync::Arc::new(crate::config::HostConfig::default()),
        )
        .unwrap();
        assert_eq!(ctx.contexts.len(), 1);
        assert_eq!(
            ctx.repositories
                .values()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1
        );
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
        let mut ctx = open_test(
            Some(&dir),
            &ids,
            &key,
            std::sync::Arc::new(crate::config::HostConfig::default()),
        )
        .unwrap();
        let (_, a, mut n) = example(&id);
        let mut note =
            decode_operation(n.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let second_id = crate::model::TaskId::parse(&second).unwrap();
        if !note.input_bases.iter().any(|input| input.task == second) {
            note.input_bases.push(Input {
                task: second.clone(),
                basis: ctx.tasks[&second_id].basis.clone(),
            });
        }
        n.text = format!(
            "retention operation: {}",
            serde_json::to_string(&note).unwrap()
        );
        assert_eq!(ctx.contexts.len(), 2);
        assert_eq!(
            ctx.repositories
                .values()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1
        );
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
        let second = crate::model::TaskId::parse(&second).unwrap();
        assert!(ctx.tasks.contains_key(&second));
        let mut permits = Vec::new();
        while let Some(permit) = super::super::read::reserve_memo(ctx.project(&second), 0).unwrap()
        {
            permits.push(permit);
        }
        assert!(
            super::super::read::reserve_memo(ctx.project(&id), 0)
                .unwrap()
                .is_none(),
            "peer projects share the publication budget"
        );
        drop(permits);
        assert!(
            super::super::read::reserve_memo(ctx.project(&id), 0)
                .unwrap()
                .is_some()
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
    fn generated(operation: &OperationNote, outputs: PhaseOutputs) -> GeneratedNote {
        let mut operation = operation.clone();
        operation.phase = outputs.phase().into();
        operation.outputs = outputs;
        GeneratedNote {
            at: operation.at.clone(),
            text: format!(
                "retention operation: {}",
                serde_json::to_string(&operation).unwrap()
            ),
        }
    }
    fn pair(
        ctx: &OperationCtx,
        id: &crate::model::TaskId,
    ) -> (NewArtifact, OperationNote, crate::model::TaskId) {
        let (_, artifact, note) = example(id);
        let mut operation =
            decode_operation(note.text.strip_prefix("retention operation: ").unwrap()).unwrap();
        let peer = ctx.tasks.keys().find(|peer| *peer != id).unwrap().clone();
        operation.input_bases.push(Input {
            task: peer.to_string(),
            basis: ctx.tasks[&peer].basis.clone(),
        });
        (artifact, operation, peer)
    }
    #[test]
    fn retained_phase_scope_failure_and_reconcile() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_scope_failure_and_reconcile")
        else {
            return;
        };
        let (key, _, _) = example(&id);
        let second = std::env::var("TASKS_PUBLICATION_SECOND").unwrap();
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string(), second],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let (artifact, operation, peer) = pair(&ctx, &id);
        batch(
            &mut ctx,
            &id,
            &[artifact],
            &[generated(&operation, PhaseOutputs::Intent(Empty {}))],
        )
        .unwrap();
        let project = ctx.project(&peer);
        let path = project.task_path(&peer);
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, raw.replace("Second participant", "Changed peer")).unwrap();
        let incomplete = || {
            generated(
                &operation,
                PhaseOutputs::Incomplete(IncompleteOutput {
                    attempt: 1,
                    previous_phase: "intent".into(),
                    checkpoint: None,
                    action: "Original peer failure".into(),
                }),
            )
        };
        let receiver = generated(
            &operation,
            PhaseOutputs::Receiver(ReceiverOutput {
                items: vec![],
                artifacts: vec![],
                mapping: vec![],
            }),
        );
        assert!(
            batch(&mut ctx, &id, &[], &[receiver]).is_err(),
            "receiver checks both participants"
        );
        let release = generated(
            &operation,
            PhaseOutputs::Sender(SenderOutput {
                items: vec![],
                receiver_basis: ctx.tasks[&peer].basis.clone(),
                mapping: vec![],
            }),
        );
        assert!(
            batch(&mut ctx, &id, &[], &[release]).is_err(),
            "release checks both participants"
        );
        batch(&mut ctx, &id, &[], &[incomplete()]).expect("stale peer cannot prevent incomplete");
        std::fs::write(&path, "malformed peer record\n").unwrap();
        let directive = operation.request.clone();
        let recon = generated(
            &operation,
            PhaseOutputs::ReconcileIntent(ReconcileIntentOutput {
                directive: directive.clone(),
                reconciliation_id: "b".repeat(32),
            }),
        );
        batch(&mut ctx, &id, &[], &[recon]).expect("reconcile intent checks target only");
        let ready = generated(
            &operation,
            PhaseOutputs::Ready(ReadyOutput {
                directive: directive.clone(),
                reconciliation_id: "b".repeat(32),
                items: vec![],
                unresolved: vec![],
            }),
        );
        batch(&mut ctx, &id, &[], &[ready]).expect("ready checks target only");
        let terminal = generated(
            &operation,
            PhaseOutputs::Reconciled(ReconciledOutput {
                directive,
                reconciliation_id: "b".repeat(32),
                ready_bases: vec![],
                unresolved: vec![],
            }),
        );
        assert!(
            batch(&mut ctx, &id, &[], &[terminal]).is_err(),
            "terminal checks every participant"
        );
        let path = ctx.project(&id).task_path(&id);
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, raw.replace("Retained fixture", "Changed target")).unwrap();
        assert!(
            batch(&mut ctx, &id, &[], &[incomplete()]).is_err(),
            "stale target refuses incomplete"
        );
    }
    #[test]
    fn retained_phase_scope_release_readback() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_scope_release_readback")
        else {
            return;
        };
        let (key, _, _) = example(&id);
        let second = std::env::var("TASKS_PUBLICATION_SECOND").unwrap();
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string(), second],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let (artifact, operation, peer) = pair(&ctx, &id);
        batch(
            &mut ctx,
            &id,
            &[artifact],
            &[generated(&operation, PhaseOutputs::Intent(Empty {}))],
        )
        .unwrap();
        let release = generated(
            &operation,
            PhaseOutputs::Sender(SenderOutput {
                items: vec![],
                receiver_basis: ctx.tasks[&peer].basis.clone(),
                mapping: vec![],
            }),
        );
        let hook = dir.join(".git/hooks/pre-commit");
        let original_hook = std::fs::read(&hook).unwrap();
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(
            batch(
                &mut ctx,
                &id,
                &[],
                &[GeneratedNote {
                    at: release.at.clone(),
                    text: release.text.clone()
                }]
            )
            .is_err()
        );
        assert!(
            !ctx.committed_release,
            "failed readback must not prove release"
        );
        // Retry the same saved phase: it becomes proven only after exact readback.
        let working = ctx.project(&id).read_task(&id).unwrap();
        ctx.tasks.get_mut(&id).unwrap().task = working;
        std::fs::write(&hook, original_hook).unwrap();
        batch(&mut ctx, &id, &[], &[release]).unwrap();
        assert!(
            ctx.committed_release,
            "successful release readback applies in-process"
        );
        let path = ctx.project(&peer).task_path(&peer);
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            raw.replace("Second participant", "Changed peer after release"),
        )
        .unwrap();
        let completed = generated(
            &operation,
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "transferred".into(),
                publications: vec![],
                checkpoint: None,
            }),
        );
        batch(&mut ctx, &id, &[], &[completed])
            .expect("proved release completion checks only its target");
    }
    #[test]
    fn retained_phase_scope_release_failed_readback() {
        let Some((dir, id)) =
            fixture("retained::publish::tests::retained_phase_scope_release_failed_readback")
        else {
            return;
        };
        let (key, _, _) = example(&id);
        let second = std::env::var("TASKS_PUBLICATION_SECOND").unwrap();
        let mut ctx = open_test(
            Some(&dir),
            &[id.to_string(), second],
            &key,
            Arc::new(HostConfig::default()),
        )
        .unwrap();
        let (artifact, operation, peer) = pair(&ctx, &id);
        batch(
            &mut ctx,
            &id,
            &[artifact],
            &[generated(&operation, PhaseOutputs::Intent(Empty {}))],
        )
        .unwrap();
        let before = ctx.phase_bases[&id].clone();
        let release = generated(
            &operation,
            PhaseOutputs::Sender(SenderOutput {
                items: vec![],
                receiver_basis: ctx.tasks[&peer].basis.clone(),
                mapping: vec![],
            }),
        );
        std::fs::write(dir.join(".git/hooks/pre-commit"), format!("#!/bin/sh\nprintf '%s\\n' '- 2026-11-03T00:00:01Z (fixture): hook changed readback' >> 'tasks/{id}.md'\ngit add -- 'tasks/{id}.md'\n")).unwrap();
        assert!(batch(&mut ctx, &id, &[], &[release]).is_err());
        assert!(
            !ctx.committed_release,
            "failed exact readback cannot prove release"
        );
        assert_eq!(
            ctx.phase_bases[&id], before,
            "failed exact readback cannot advance P"
        );
    }
    #[test]
    fn retained_phase_scope_table_and_mixed_recovery() {
        let id = crate::model::TaskId::parse("sci-000001").unwrap();
        let basis = Basis {
            commit: "a".repeat(40),
            retained_ref: "refs/heads/main".into(),
            project: "sci".into(),
            layout: "in-tree".into(),
            records_subdir: ".".into(),
            task_path: "tasks/sci-000001.md".into(),
        };
        let operation = OperationNote {
            version: 1,
            key: OperationKey {
                coordinator: id.to_string(),
                operation_id: "a".repeat(32),
            },
            phase: "intent".into(),
            request: ArtifactLocator::Attachment {
                task: id.to_string(),
                name: "retained-op-a.request.json".into(),
                basis: None,
                sha256: "a".repeat(64),
            },
            input_bases: vec![
                Input {
                    task: id.to_string(),
                    basis: basis.clone(),
                },
                Input {
                    task: "sci-000002".into(),
                    basis: Basis {
                        task_path: "tasks/sci-000002.md".into(),
                        ..basis.clone()
                    },
                },
            ],
            host: "fixture".into(),
            at: "2026-11-03T00:00:00Z".into(),
            observer: None,
            outputs: PhaseOutputs::Intent(Empty {}),
            diagnostics: vec![],
        };
        let directive = operation.request.clone();
        let cases = vec![
            (PhaseOutputs::Intent(Empty {}), PhaseScope::All),
            (
                PhaseOutputs::Receiver(ReceiverOutput {
                    items: vec![],
                    artifacts: vec![],
                    mapping: vec![],
                }),
                PhaseScope::All,
            ),
            (
                PhaseOutputs::Sender(SenderOutput {
                    items: vec![],
                    receiver_basis: basis.clone(),
                    mapping: vec![],
                }),
                PhaseScope::All,
            ),
            (
                PhaseOutputs::Incomplete(IncompleteOutput {
                    attempt: 1,
                    previous_phase: "intent".into(),
                    checkpoint: None,
                    action: "preserve".into(),
                }),
                PhaseScope::Target,
            ),
            (
                PhaseOutputs::ReconcileIntent(ReconcileIntentOutput {
                    directive: directive.clone(),
                    reconciliation_id: "b".repeat(32),
                }),
                PhaseScope::Target,
            ),
            (
                PhaseOutputs::Ready(ReadyOutput {
                    directive: directive.clone(),
                    reconciliation_id: "b".repeat(32),
                    items: vec![],
                    unresolved: vec![],
                }),
                PhaseScope::Target,
            ),
            (
                PhaseOutputs::Reconciled(ReconciledOutput {
                    directive,
                    reconciliation_id: "b".repeat(32),
                    ready_bases: vec![],
                    unresolved: vec![],
                }),
                PhaseScope::All,
            ),
        ];
        for (outputs, expected) in cases {
            assert_eq!(
                phase_scope(&[generated(&operation, outputs)], &id, false).unwrap(),
                expected
            );
        }
        let mut checkpoint = operation.clone();
        checkpoint.input_bases.truncate(1);
        for outputs in [
            PhaseOutputs::Intent(Empty {}),
            PhaseOutputs::Items(ItemsOutput {
                items: vec![],
                artifacts: vec![],
                resolutions: vec![],
            }),
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "no_local_items".into(),
                publications: vec![],
                checkpoint: None,
            }),
        ] {
            assert_eq!(
                phase_scope(&[generated(&checkpoint, outputs)], &id, false).unwrap(),
                PhaseScope::Target
            );
        }
        let intent = generated(&operation, PhaseOutputs::Intent(Empty {}));
        let incomplete = generated(
            &operation,
            PhaseOutputs::Incomplete(IncompleteOutput {
                attempt: 1,
                previous_phase: "intent".into(),
                checkpoint: None,
                action: "preserve".into(),
            }),
        );
        let completed = generated(
            &operation,
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "transferred".into(),
                publications: vec![],
                checkpoint: None,
            }),
        );
        assert_eq!(
            phase_scope(&[intent, incomplete], &id, true).unwrap(),
            PhaseScope::All,
            "mixed recovery unions every phase"
        );
        assert_eq!(
            phase_scope(&[completed], &id, false).unwrap(),
            PhaseScope::All
        );
        let completed = generated(
            &operation,
            PhaseOutputs::Completed(CompletedOutput {
                outcome: "transferred".into(),
                publications: vec![],
                checkpoint: None,
            }),
        );
        assert_eq!(
            phase_scope(&[completed], &id, true).unwrap(),
            PhaseScope::Target
        );
        let mut foreign = operation.clone();
        foreign.key.operation_id = "b".repeat(32);
        assert!(
            phase_scope(
                &[
                    generated(&operation, PhaseOutputs::Intent(Empty {})),
                    generated(
                        &foreign,
                        PhaseOutputs::Incomplete(IncompleteOutput {
                            attempt: 1,
                            previous_phase: "intent".into(),
                            checkpoint: None,
                            action: "preserve".into()
                        })
                    )
                ],
                &id,
                false
            )
            .is_err()
        );
        assert!(
            phase_scope(
                &[
                    generated(
                        &operation,
                        PhaseOutputs::Completed(CompletedOutput {
                            outcome: "transferred".into(),
                            publications: vec![],
                            checkpoint: None
                        })
                    ),
                    generated(&operation, PhaseOutputs::Intent(Empty {}))
                ],
                &id,
                false
            )
            .is_err()
        );
    }
}
