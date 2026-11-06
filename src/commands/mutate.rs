use super::{ClaimIntent, Ctx, dep, parse_id};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::format::{normalize_note_text, validate_body, validate_line, validate_task};
use crate::model::{Complexity, Note, Process, Size, Status, Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, Resolver};
use std::io::Read;
use std::path::Path;

/// $TASKS_OWNER, else the current git branch, else $USER. Whatever is chosen must
/// satisfy `validate_owner`; an invalid value is an error, never sanitized.
pub fn owner_name(project: &Project) -> Result<String> {
    let name = raw_owner_name(project)?;
    crate::format::validate_owner(&name)?;
    Ok(name)
}

fn raw_owner_name(project: &Project) -> Result<String> {
    if let Ok(owner) = std::env::var("TASKS_OWNER")
        && !owner.is_empty()
    {
        return Ok(owner);
    }
    if let Ok(output) = crate::git::at(project.resume_root())
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        && output.status.success()
    {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() && branch != "HEAD" {
            return Ok(branch);
        }
    }
    match std::env::var("USER") {
        Ok(user) if !user.is_empty() => Ok(user),
        _ => Err(Error::Config(
            "cannot determine an owner: set TASKS_OWNER (no git branch, USER unset)".into(),
        )),
    }
}

/// Applies the field flags present in `fields` to `task`, validating each against the repo.
pub fn apply_fields(
    ctx: &mut Ctx,
    task: &mut Task,
    fields: &FieldArgs,
    original: Option<&Task>,
) -> Result<()> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let plan_changed =
        fields.plan.as_ref().or(task.plan.as_ref()) != original.and_then(|task| task.plan.as_ref());
    let step_changed =
        fields.step.as_ref().or(task.step.as_ref()) != original.and_then(|task| task.step.as_ref());
    if let Some(body) = &fields.body {
        let body = if body == "-" {
            let mut body = String::new();
            std::io::stdin().read_to_string(&mut body)?;
            body
        } else {
            body.clone()
        };
        validate_body(&body)?;
        task.body = body;
    }
    let mut handles = std::collections::HashSet::new();
    for text in &fields.refs {
        let entry = crate::refs::parse_entry(text)?;
        if !handles.insert(entry.handle.clone()) {
            return Err(Error::Validation(format!(
                "--ref names {} twice",
                entry.handle
            )));
        }
        if let Some(existing) = task
            .refs
            .iter_mut()
            .find(|existing| existing.handle == entry.handle)
        {
            *existing = entry;
        } else {
            task.refs.push(entry);
        }
    }
    crate::refs::require_targets(&resolver, &ctx.project, task, original)?;
    if original.is_none_or(|old| old.body != task.body || old.refs != task.refs) {
        crate::refs::refuse_problems(task)?;
    }
    if let Some(priority) = fields.priority {
        if priority > 4 {
            return Err(Error::Validation("priority must be 0-4".into()));
        }
        task.priority = priority;
    }
    if let Some(size) = &fields.size {
        task.size = Some(Size::parse(size)?);
    }
    if let Some(level) = &fields.complexity {
        task.complexity = Some(Complexity::parse(level)?);
    }
    if let Some(process) = &fields.process {
        task.process = Some(Process::parse(process)?);
    }
    if let Some(origin) = &fields.origin {
        task.origin = Some(crate::model::Origin::parse(origin)?);
    }
    if let Some(every) = &fields.every {
        task.every = Some(crate::periodic::Interval::parse(every)?);
    }
    if let Some(value) = &fields.defer {
        if !crate::defer::can_carry(task.status) {
            return Err(Error::Validation(format!(
                "{} is {}; a deferral can only sit on an idea, todo, or blocked task",
                task.id,
                task.status.as_str()
            )));
        }
        let today = crate::time::parse(&crate::time::now())?.date();
        task.defer = Some(crate::defer::Defer::resolve(value, today)?);
    }
    // Setting only. `edit --no-parallel` clears it before this runs, mirroring --no-tags.
    if fields.parallel {
        task.parallel = true;
    }
    // Setting only. `edit --no-lane` clears it before this runs, like --no-parallel.
    if fields.lane {
        task.lane = true;
    }
    // Additive, never a replacement: a triage `--tag` must not silently drop the tags a
    // task already carries. `edit` removes with `--rm-tag` / `--no-tags`.
    for tag in &fields.tags {
        if !task.tags.contains(tag) {
            task.tags.push(tag.clone());
        }
    }
    // Additive like `--tag`. Only the names being added are checked: grammar, then the
    // project's vocabulary (docs/user-guide.md), so `edit --rm-need` can still clear
    // a need the vocabulary has since dropped.
    for need in &fields.needs {
        crate::needs::validate_name(need)?;
    }
    crate::needs::require_declared(&ctx.project.needs, &fields.needs)?;
    for need in &fields.needs {
        if !task.needs.contains(need) {
            task.needs.push(need.clone());
        }
    }
    // Additive like `--tag`: an edit that names one dependency must not drop the others.
    // `edit --no-depends` clears the list before this runs; `dep --rm` removes one.
    let warnings = dep::add_dependencies(ctx, &resolver, task, &fields.depends)?;
    ctx.warnings.extend(warnings);
    if let Some(parent) = &fields.parent {
        task.parent = Some(parse_id(&ctx.registry, &ctx.shorthand, parent)?);
    }
    if let Some(source) = &fields.source {
        validate_line("source", source)?;
        task.source = Some(source.clone());
    }
    if let Some(agent) = &fields.agent {
        validate_line("agent", agent)?;
        task.agent = Some(agent.clone());
    }
    if let Some(spec) = &fields.spec {
        task.spec = Some(
            if crate::format::external_doc(spec)?.is_some()
                && Some(spec) == original.and_then(|task| task.spec.as_ref())
            {
                spec.clone()
            } else {
                resolver.resolve_doc(DocKind::Spec, spec)?
            },
        );
    }
    if let Some(plan) = &fields.plan {
        task.plan = Some(
            if crate::format::external_doc(plan)?.is_some()
                && Some(plan) == original.and_then(|task| task.plan.as_ref())
            {
                plan.clone()
            } else {
                resolver.resolve_doc(DocKind::Plan, plan)?
            },
        );
    }
    if let Some(step) = &fields.step {
        task.step = Some(step.clone());
    }
    match (&task.plan, &task.step) {
        (None, Some(_)) => return Err(Error::Validation("--step requires a plan".into())),
        (Some(plan), Some(step))
            if (crate::format::external_doc(plan)?.is_none() || plan_changed || step_changed)
                && !resolver.step_exists(plan, step)? =>
        {
            return Err(Error::Validation(format!(
                "heading {step:?} not found in {plan}"
            )));
        }
        _ => {}
    }
    validate_task(task)?;
    crate::defer::validate(task)?;
    ctx.project.validate_docs(task)?;
    ctx.warnings.extend(resolver.take_warnings());
    Ok(())
}

/// `save` for a task that does not exist yet: validates, then creates exclusively. Takes
/// the project rather than `Ctx` because the feedback command creates in another project.
pub fn create(project: &Project, registry: &Registry, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    project.validate_docs(task)?;
    project.create_task(registry, task)
}

pub fn append_note(task: &mut Task, by: &str, text: &str) -> Result<()> {
    if text.contains(['\n', '\r']) {
        return Err(Error::Validation(format!(
            "note text must be a single line; keep the message to one line and attach the full output with `tasks attach {} - --name <file> --caption \"<what it shows>\"`",
            task.id
        )));
    }
    let text = normalize_note_text(text)?;
    task.notes.push(Note {
        at: crate::time::now(),
        by: by.to_string(),
        text: text.to_string(),
        provenance: None,
        occurrence: Default::default(),
    });
    Ok(())
}

/// Appends a note stamped with the harness session provenance, when the environment names one.
pub fn append_stamped_note(ctx: &mut Ctx, task: &mut Task, by: &str, text: &str) -> Result<()> {
    append_stamped_note_to(&mut ctx.warnings, task, by, text)
}

/// `append_stamped_note` for a caller that collects its warnings without a `Ctx`.
pub(crate) fn append_stamped_note_to(
    warnings: &mut Vec<String>,
    task: &mut Task,
    by: &str,
    text: &str,
) -> Result<()> {
    append_note(task, by, text)?;
    match crate::provenance::resolve_from(|name| std::env::var_os(name)) {
        Ok(provenance) => task.notes.last_mut().expect("just appended").provenance = provenance,
        Err(diagnostic) => warnings.push(diagnostic),
    }
    Ok(())
}

/// The origin and origin session a filer supplied, validated before any write.
#[derive(Clone, Copy, Default)]
pub struct Attribution<'a> {
    pub origin: Option<crate::model::Origin>,
    pub session: Option<&'a str>,
}

impl<'a> Attribution<'a> {
    pub fn parse(origin: Option<&str>, session: Option<&'a str>) -> Result<Self> {
        let origin = origin.map(crate::model::Origin::parse).transpose()?;
        if let Some(key) = session {
            crate::provenance::validate_session_key(key)
                .map_err(|reason| Error::Validation(format!("--origin-session: {reason}")))?;
        }
        Ok(Attribution { origin, session })
    }
}

/// The harness identity of one command run, resolved once from the process environment.
#[derive(Default)]
pub struct Stamp {
    pub provenance: Option<crate::model::HarnessProvenance>,
    pub root_session: Option<String>,
}

/// Resolves the stamp for this run. A resolver error pushes its diagnostic (which names
/// variables, never values) and yields an empty stamp: identity never blocks a write.
pub fn occurrence_stamp(warnings: &mut Vec<String>) -> Stamp {
    let get = |name: &str| std::env::var_os(name);
    match crate::provenance::resolve_from(get) {
        Ok(provenance) => Stamp {
            provenance,
            root_session: crate::provenance::resolve_root_from(get),
        },
        Err(diagnostic) => {
            warnings.push(diagnostic);
            Stamp::default()
        }
    }
}

/// Appends an occurrence note: the stamp's native pair and `root_session`, plus the
/// given origin and origin session.
pub fn append_occurrence_note(
    task: &mut Task,
    at: &str,
    by: &str,
    text: &str,
    stamp: &Stamp,
    origin: Option<crate::model::Origin>,
    origin_session: Option<&str>,
) -> Result<()> {
    append_note(task, by, text)?;
    let note = task.notes.last_mut().expect("just appended");
    note.at = at.to_string();
    note.provenance = stamp.provenance.clone();
    note.occurrence = crate::model::NoteOccurrence {
        root_session: stamp.root_session.clone(),
        origin,
        origin_session: origin_session.map(str::to_string),
    };
    Ok(())
}

/// Ids of dependencies that are open or unreachable.
pub fn open_deps(ctx: &Ctx, task: &Task) -> Result<Vec<String>> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
    let mut open = Vec::new();
    for dependency in &task.depends {
        // The project's own file answers for its prefix, registered roots for the rest.
        let found =
            crate::dependency::resolve(&ctx.registry, dependency, |id| resolver.resolve_task(id))?;
        if crate::dependency::verdict(found.as_ref()) != crate::dependency::Verdict::Satisfied {
            open.push(dependency.to_string());
        }
    }
    Ok(open)
}

/// A provenance variable the harness exports (`TASKS_MODEL`, `TASKS_AGENT`). Unset or
/// empty records nothing; a non-Unicode or multi-line value is an explicit error, never
/// a silent skip.
fn provenance_var(name: &str) -> Result<Option<String>> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => {
            let value = value
                .into_string()
                .map_err(|_| Error::Validation(format!("{name} is not valid Unicode")))?;
            if value.is_empty() {
                return Ok(None);
            }
            crate::format::validate_line(name, &value)?;
            Ok(Some(value))
        }
    }
}

/// The model the harness reports for this completion, from `TASKS_MODEL`.
pub(crate) fn completion_model() -> Result<Option<String>> {
    provenance_var("TASKS_MODEL")
}

/// The agent filing a task: an explicit `--agent` when given, validated first so a bad
/// environment cannot fail an add that names a valid agent; otherwise `TASKS_AGENT`.
pub fn creation_agent(explicit: Option<&str>) -> Result<Option<String>> {
    match explicit {
        Some(agent) => {
            validate_line("agent", agent)?;
            Ok(Some(agent.to_string()))
        }
        None => provenance_var("TASKS_AGENT"),
    }
}

/// When a recurrence completed at `now` comes due again.
fn next_due(now: &str, every: crate::periodic::Interval) -> Result<time::OffsetDateTime> {
    crate::periodic::add(crate::time::parse(now)?, every).ok_or_else(|| {
        Error::Validation(format!(
            "completing now plus every {every} is not a representable timestamp"
        ))
    })
}

/// The field effects of a status change to `to` stamped at `now`, with no checks and no
/// notes: `transition` runs its checks first, and `migrate` renders a drop or close
/// through this (docs/project-operations.md). `model` is recorded on a completion.
pub(crate) fn apply_status_effects(
    task: &mut Task,
    to: Status,
    now: &str,
    model: Option<String>,
) -> Result<()> {
    let completing = to == Status::Done && task.status != Status::Done;
    let closing = matches!(to, Status::Done | Status::Dropped) && task.status != to;
    if to == Status::Doing && task.started.is_none() {
        task.started = Some(now.to_string());
    }
    if completing {
        task.model = model;
        task.completed = Some(now.to_string());
    } else if task.status == Status::Done && to != Status::Done {
        task.completed = None;
    }
    // Current contract (docs/user-guide.md): every close stamps `closed`; a reopen clears it. `move` never
    // passes through here, so a tombstone carries none.
    if closing {
        task.closed = Some(now.to_string());
    } else if matches!(task.status, Status::Done | Status::Dropped) && to.is_open() {
        task.closed = None;
    }
    if task.status != to {
        // Current contract (docs/task-model.md): a deferral is spent by a status change.
        task.defer = None;
        if task.status == Status::Shelved {
            task.wake_on.clear();
        }
        // Current contract (docs/user-guide.md): a status change spends a review; a return to idea
        // arrives unreviewed.
        task.reviewed = false;
        task.reviewed_at = None;
        task.awaiting_user = false;
    }
    task.status = to;
    if completing && let Some(every) = task.every {
        next_due(now, every)?;
        task.last_done = Some(now.to_string());
    }
    Ok(())
}

pub fn transition(ctx: &mut Ctx, task: &mut Task, to: Status, force: bool) -> Result<()> {
    // Cadence permits early reopening; the clock only governs read-time visibility.
    let reopening = task.status == Status::Done && to == Status::Doing && task.every.is_some();
    if !reopening && !Status::can_transition(task.status, to) {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            to.as_str().into(),
        ));
    }
    if to == Status::Done
        && task.status == Status::Done
        && let Some(every) = task.every
    {
        let id = task.id.clone();
        let worktree = ctx.project.resume_root().display().to_string();
        let store = ctx.claims_mut()?;
        for entry_worktree in store
            .get(&id)
            .map(|claim| &claim.worktree)
            .into_iter()
            .chain(store.park(&id).map(|park| &park.worktree))
        {
            // Claims and parks are host-local, so a checkout missing here holds no work
            // of this occurrence: the release below may clear its entry.
            if entry_worktree != &worktree && Path::new(entry_worktree).try_exists()? {
                return Err(Error::Validation(format!(
                    "{id} is already done here but has work in {entry_worktree}; finish that occurrence in its owning checkout, or, if this closed record already covers it, integrate the record there and run `tasks done {id}` there to release it"
                )));
            }
        }
        if store.get(&id).is_none() && store.park(&id).is_none() {
            return Err(Error::Validation(format!(
                "{id} is already done and recurs every {every}; `tasks start {id}` before closing the next occurrence"
            )));
        }
        ctx.warnings.push(format!(
            "{id} was already completed; releasing its claim without recording another occurrence"
        ));
        ctx.recovered = true;
    }
    // Guard before the dependency and descendant checks, so a session that no longer holds
    // the task is told *that* rather than something incidental.
    ctx.claim_guard(&task.id, to, force)?;
    if to == Status::Doing {
        ctx.guard_holds(task, force)?;
    }
    if to == Status::Moved && task.status != Status::Moved {
        return Err(Error::Validation(
            "only `tasks move` sets status moved".into(),
        ));
    }
    let completing = to == Status::Done && task.status != Status::Done;
    if completing {
        crate::refs::require_outputs(
            &Resolver::new(&ctx.project, &ctx.registry, &ctx.host),
            &ctx.project,
            task,
        )?;
    }
    if completing && !force {
        let open = open_deps(ctx, task)?;
        if !open.is_empty() {
            return Err(Error::OpenDependencies(
                task.id.to_string(),
                open.join(", "),
            ));
        }
    }
    let closing = matches!(to, Status::Done | Status::Dropped) && task.status != to;
    if closing && !(force && to == Status::Done) {
        let all = ctx.project.scan()?;
        let open: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id, &ctx.registry)
            .iter()
            .map(|task| task.id.to_string())
            .collect();
        if !open.is_empty() {
            return Err(Error::OpenDescendants(task.id.to_string(), open.join(", ")));
        }
    }
    let first_start = task.started.is_none();
    // One instant for everything this transition stamps, so a recurring completion's
    // `completed` and `last_done` agree to the second (docs/task-model.md).
    let now = crate::time::now();
    let model = if completing {
        completion_model()?
    } else {
        None
    };
    apply_status_effects(task, to, &now, model)?;
    if completing && let Some(every) = task.every {
        let next = next_due(&now, every)?;
        let owner = owner_name(&ctx.project)?;
        append_stamped_note(
            ctx,
            task,
            &owner,
            &format!("completed; next due {}", next.date()),
        )?;
    } else if closing {
        let owner = owner_name(&ctx.project)?;
        append_stamped_note(ctx, task, &owner, to.as_str())?;
    } else if to == Status::Doing {
        let owner = owner_name(&ctx.project)?;
        append_stamped_note(
            ctx,
            task,
            &owner,
            if first_start { "started" } else { "resumed" },
        )?;
    }
    Ok(())
}

/// The exact retry for a reassessment whose store save failed: what was actually given,
/// not a placeholder, so running it verbatim redoes the same clear.
fn escalation_retry_hint(id: &TaskId, given: Option<Complexity>) -> String {
    match given {
        Some(level) => format!("rerun `tasks edit {id} --complexity {}`", level.as_str()),
        None => format!("rerun `tasks edit {id} --no-complexity`"),
    }
}

/// Persists status claim intents only after validation.
///
/// Everything that can reject the change runs first and touches nothing. From there the
/// store and the task file move together, in the order that fails toward "claim held": a
/// claim with no file update makes an idle task look busy and self-heals when the session
/// dies, while a file update with no claim is the invisibility bug this exists to remove.
pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::after(&task.updated)?;
    validate_task(task)?;
    ctx.project.validate_docs(task)?;
    ctx.project.validate_invariants(&ctx.registry, task)?;
    crate::migrating::freeze::check_stored(&ctx.registry, &ctx.project, task)?;
    let clear_escalation = std::mem::take(&mut ctx.clear_escalation);

    match ctx.pending_claim.take() {
        Some((id, ClaimIntent::Acquire(claim))) => {
            // Current contract (docs/user-guide.md): a claim whose holds differ from the one it replaces
            // is published, and on a failed record write rolled back, only under the holds
            // lock. A path that changed holds without taking it is a bug, refused here
            // before anything is written.
            let replaced = ctx
                .claims_mut()?
                .get(&id)
                .map(|claim| claim.holds.clone())
                .unwrap_or_default();
            if claim.holds != replaced && ctx.holds_lock.is_none() {
                return Err(Error::Io(format!(
                    "the claim on {id} changes its holds without the holds lock"
                )));
            }
            let store = ctx.claims_mut()?;
            // Captured, never assumed absent: a repeated `start` by the owner and a forced
            // takeover both write over an existing claim, and a blanket removal on failure
            // would unclaim work someone still holds.
            let previous = store.get(&id).cloned();
            let previous_park = store.park(&id).cloned();
            let previous_escalation = if clear_escalation.is_some() {
                store.remove_escalation(&id)
            } else {
                None
            };
            store.prune_dead();
            store.insert(&id, claim);
            // A failure here never removed the escalation from disk, so nothing about
            // clearing it is reported; the command fails with the store error itself.
            store.save()?;

            let error = match ctx.project.write_validated_task_permitted(
                task,
                || Ok(()),
                crate::retained::publish::WritePermission::Claim,
            ) {
                Ok(()) => {
                    if let Some(escalation) = &previous_escalation {
                        ctx.warnings.push(format!(
                            "cleared the escalation of {id} to {} recorded by session {} at {}",
                            escalation.level.as_str(),
                            escalation.session,
                            escalation.at
                        ));
                    }
                    return Ok(());
                }
                Err(error) => error,
            };
            let store = ctx.claims_mut()?;
            if let Some(escalation) = previous_escalation {
                store.insert_escalation(&id, escalation);
            }
            match (previous, previous_park) {
                (Some(previous), _) => store.insert(&id, previous),
                (None, Some(park)) => store.insert_park(&id, park),
                (None, None) => {
                    store.remove(&id);
                }
            }
            // Warnings on `Ctx` are dropped when a command returns `Err`, so recovery
            // guidance has to travel on the error itself. The escalation, if any, was just
            // restored above -- unchanged, so nothing about it belongs here either.
            let suffix = match store.save() {
                Ok(()) => String::new(),
                Err(inner) => format!(
                    " (the previous claim on {id} could not be restored: {inner}; \
                     run `tasks start --force {id}` to reclaim it)"
                ),
            };
            Err(error.with_suffix(&suffix))
        }
        Some((
            id,
            ClaimIntent::Release {
                clear_park,
                announce_escalation,
            },
        )) => {
            ctx.project.write_validated_task(task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove(&id);
            let removed_escalation = if clear_escalation.is_some() || clear_park {
                store.remove_escalation(&id)
            } else {
                None
            };
            if clear_park {
                store.remove_park(&id);
            }
            match store.save() {
                Ok(()) => {
                    if let Some(escalation) = &removed_escalation
                        && (clear_escalation.is_some() || announce_escalation)
                    {
                        ctx.warnings.push(format!(
                            "cleared the escalation of {id} to {} recorded by session {} at {}",
                            escalation.level.as_str(),
                            escalation.session,
                            escalation.at
                        ));
                    }
                }
                Err(error) => {
                    let recovery = match task.status {
                        Status::Done => format!("run `tasks done {id}`"),
                        Status::Dropped => format!("run `tasks drop {id}`"),
                        Status::Blocked => format!("run `tasks block {id}`"),
                        Status::Shelved => format!("run `tasks shelve {id} \"<wake condition>\"`"),
                        Status::Todo | Status::Idea | Status::Doing | Status::Moved => {
                            "the store is unchanged and a same-status edit will not retry cleanup"
                                .into()
                        }
                    };
                    ctx.warnings.push(format!(
                        "{id}'s status was saved but store cleanup failed ({error}); {recovery}"
                    ));
                    if let Some(given) = clear_escalation {
                        let escalation = removed_escalation
                            .expect("reassess found this escalation under the same lock");
                        ctx.warnings.push(format!(
                            "the escalation of {id} to {} was not cleared ({error}); {}",
                            escalation.level.as_str(),
                            escalation_retry_hint(&id, given)
                        ));
                    }
                }
            }
            Ok(())
        }
        Some((id, ClaimIntent::Park { park, escalation })) => {
            // Record first, then store: a note with no entry is a trail that says what was
            // intended, while an entry with no note would be state the record never saw.
            ctx.project.write_validated_task_permitted(
                task,
                || Ok(()),
                crate::retained::publish::WritePermission::Park,
            )?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.insert_park(&id, park);
            if let Some(escalation) = &escalation {
                store.insert_escalation(&id, escalation.clone());
            }
            // A failed store save is the command's failure, never a warning beside exit 0
            // (docs/ownership.md): the store's own error, naming what landed and the
            // rerun. For an escalation the raised rating stays, and equal-to-current lets
            // the rerun pass (docs/task-model.md).
            store.save().map_err(|error| {
                let landed = match &escalation {
                    Some(escalation) => format!(
                        "the note and rating on {id} landed but its escalation to {} was not \
                         recorded and it was not parked",
                        escalation.level.as_str()
                    ),
                    None => format!("the note on {id} landed but it was not parked"),
                };
                error.with_suffix(&format!("; {landed}; rerun the same `tasks park` command"))
            })
        }
        Some((id, ClaimIntent::ClearEscalation(escalation))) => {
            ctx.project.write_validated_task(task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove_escalation(&id);
            match store.save() {
                Ok(()) => ctx.warnings.push(format!(
                    "cleared the escalation of {id} to {} recorded by session {} at {}",
                    escalation.level.as_str(),
                    escalation.session,
                    escalation.at
                )),
                Err(error) => {
                    let given = clear_escalation
                        .expect("a ClearEscalation intent is only set alongside clear_escalation");
                    ctx.warnings.push(format!(
                        "{id}'s rating was saved but the escalation to {} could not be cleared \
                         ({error}); {}",
                        escalation.level.as_str(),
                        escalation_retry_hint(&id, given)
                    ));
                }
            }
            Ok(())
        }
        Some((_, ClaimIntent::PreserveStore)) => ctx.project.write_validated_task(task),
        None => {
            // Resolve the store before writing so a corrupt store cannot hide a landed edit.
            ctx.claims_mut()?;
            ctx.project.write_validated_task(task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            if let Err(error) = store.save() {
                ctx.warnings.push(format!(
                    "{} was saved, but stale claims could not be pruned ({error})",
                    task.id
                ));
            }
            Ok(())
        }
    }
}
