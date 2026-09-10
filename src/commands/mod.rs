pub mod add;
pub mod check;
pub mod dep;
pub mod edit;
pub mod feedback;
pub mod graph;
pub mod init;
pub mod list;
pub mod park;
pub mod parked;
pub mod projects;
pub mod rename;
pub mod root;
pub mod sample;
pub mod show;
pub mod status;
pub mod tags;
pub mod tree;
pub mod unregister;

use crate::claims::{ClaimStore, Liveness, MutationLock};
use crate::cli::{Cli, Command, FieldArgs, ScopeArgs};
use crate::error::{Error, Result};
use crate::format::{validate_body, validate_line, validate_note_text, validate_task};
use crate::model::{Note, Size, Status, Task, TaskId};
use crate::output::Output;
use crate::registry::Registry;
use crate::repo::{Project, SiblingCopy};
use crate::resolve::{DocKind, Resolver};
use crate::scope::{Origin, Scope};
use std::path::{Path, PathBuf};

/// What `save` must do to the claim store once every validation has passed. Recorded by the
/// guard in `transition` (or by `park`); **nothing is persisted until `save` acts on it.**
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release { clear_park: bool },
    Park(crate::claims::Park),
    PreserveStore,
}

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for a write command; absent from reads and during an interactive edit.
    pub lock: Option<MutationLock>,
    routing: Routing,
    claims: Option<ClaimStore>,
    pending_claim: Option<(TaskId, ClaimIntent)>,
    /// This close only finishes a previous completion's pending claim-store cleanup.
    pub recovered: bool,
}

impl Ctx {
    /// The claim store, loaded on first use. Only reachable with the lock held, so every
    /// read-check-write against it sits inside one critical section.
    pub fn claims_mut(&mut self) -> Result<&mut ClaimStore> {
        if self.lock.is_none() {
            return Err(Error::Io(
                "claim store touched without the mutation lock".into(),
            ));
        }
        if self.claims.is_none() {
            self.claims = Some(ClaimStore::load(&self.project.prefix)?);
        }
        Ok(self.claims.as_mut().expect("just loaded"))
    }

    pub fn describe_claim(claim: &crate::claims::Claim, live: &Liveness) -> String {
        let pid = match claim.pid {
            Some(pid) => format!(", pid {pid}"),
            None => String::new(),
        };
        let state = match live {
            Liveness::Live => "live".to_string(),
            Liveness::Stale(why) => format!("stale: {why}"),
        };
        let age = (time::OffsetDateTime::now_utc()
            - crate::time::parse(&claim.started).expect("loaded claim timestamp is valid"))
        .whole_seconds()
        .max(0);
        format!(
            "session {} (owner {}, host {}{pid}, worktree {}, since {}, age {age}s, {state})",
            claim.session, claim.owner, claim.host, claim.worktree, claim.started
        )
    }

    pub fn refuse_foreign_live_claim(&mut self, id: &TaskId) -> Result<()> {
        let me = crate::claims::identity()?;
        let store = self.claims_mut()?;
        if let Some(existing) = store.get(id) {
            let live = crate::claims::liveness(existing);
            if live == Liveness::Live && existing.session != me.session {
                return Err(Error::Claimed(
                    id.to_string(),
                    Ctx::describe_claim(existing, &live),
                ));
            }
        }
        Ok(())
    }

    pub fn preserve_claim_store(&mut self, id: &TaskId) {
        self.pending_claim = Some((id.clone(), ClaimIntent::PreserveStore));
    }

    /// Guard only. Decides whether this session may make the change and records what `save`
    /// should do — **and persists nothing**, so a validation failure, a rejected concurrent
    /// edit, or a failed write cannot leave the store mutated.
    ///
    /// Release is destination-based: any destination other than `doing` releases this
    /// session's claim. It must not key off *leaving* a local `doing`, because a session can
    /// hold the shared claim while its own checkout still reads `todo` — the ordinary
    /// cross-worktree case — and its `done` there would otherwise strand the claim.
    fn claim_guard(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let me = crate::claims::identity()?;
        let owner = owner_name(&self.project)?;
        let worktree = self.project.root.display().to_string();
        let store = self.claims_mut()?;

        let mut warning = None;
        if let Some(existing) = store.get(id) {
            let live = crate::claims::liveness(existing);
            let mine = existing.session == me.session;
            match (&live, mine) {
                (Liveness::Live, false) if !(force && to == Status::Doing) => {
                    return Err(Error::Claimed(
                        id.to_string(),
                        Ctx::describe_claim(existing, &live),
                    ));
                }
                (Liveness::Live, false) => {
                    warning = Some(format!(
                        "took over a live claim held by {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                }
                (Liveness::Stale(_), false) => {
                    warning = Some(format!(
                        "took over {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                }
                _ => {}
            }
        }

        self.pending_claim = Some(if to == Status::Doing {
            let now = crate::time::now();
            let started = self
                .claims_mut()?
                .get(id)
                .filter(|existing| existing.session == me.session)
                .map(|existing| existing.started.clone())
                .unwrap_or_else(|| now.clone());
            (
                id.clone(),
                ClaimIntent::Acquire(crate::claims::Claim {
                    owner,
                    pid_start: me.pid.and_then(|pid| match crate::claims::proc_stat(pid) {
                        crate::claims::ProcStat::Found { starttime, .. } => Some(starttime),
                        _ => None,
                    }),
                    session: me.session,
                    pid: me.pid,
                    boot_id: crate::claims::boot_id(),
                    host: crate::claims::hostname(),
                    worktree,
                    started,
                    seen: now,
                }),
            )
        } else {
            (
                id.clone(),
                ClaimIntent::Release {
                    clear_park: matches!(to, Status::Done | Status::Dropped),
                },
            )
        });

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    let project = Project::locate(&start)?;
    let registry = Registry::load()?;
    reject_stale_local(&registry, &project)?;
    Ok(Ctx {
        project,
        registry,
        warnings: Vec::new(),
        lock: None,
        routing: Routing::Local,
        claims: None,
        pending_claim: None,
        recovered: false,
    })
}

/// A checkout whose own prefix the registry has retired. Nothing else sees this:
/// `open_registered` validates the registered destination, which after a rename is
/// perfectly consistent, and `Project::locate` never reads the registry at all.
pub fn reject_stale_local(registry: &Registry, project: &Project) -> Result<()> {
    let live = registry.canonical_prefix(&project.prefix);
    if live != project.prefix {
        return Err(Error::Config(format!(
            "this checkout's tasks/.config.toml says {:?}, which was renamed to {live:?}; \
             update the checkout (merge or rebase onto the rename) before using it here",
            project.prefix
        )));
    }
    Ok(())
}

/// The write context for the project an id names. A prefix matching the local project
/// keeps that checkout, so `-C` and worktrees still win; any other prefix is followed
/// through the registry, the rule `show` and `root` already use. A local project is
/// still required, so a cwd outside every project fails exactly as before. The lock and
/// the claim store key off `ctx.project`, so both follow the id to its project.
pub fn open_id_write_ctx(dir: Option<&Path>, id: &str) -> Result<Ctx> {
    let mut ctx = open_ctx(dir)?;
    let id = parse_id(&ctx.registry, id)?;
    let routing = if id.prefix == ctx.project.prefix {
        Routing::Local
    } else {
        ctx.project = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
        Routing::Registered(id.prefix)
    };
    lock_and_revalidate(&mut ctx, &routing)?;
    Ok(ctx)
}

/// Preserve the caller's checkout for local writes; follow the registry for explicit
/// project targets and ids belonging to another project.
#[derive(Clone)]
pub enum Routing {
    Local,
    Registered(String),
}

/// Re-resolve after acquiring the lock: a rename may have completed while we waited.
/// Retry a changed identity without holding both projects' locks at once.
pub fn lock_and_revalidate(ctx: &mut Ctx, routing: &Routing) -> Result<()> {
    for _ in 0..4 {
        ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
        let registry = Registry::load()?;
        reject_pending_rename(&ctx.project)?;
        let project = match routing {
            Routing::Local => Project::open(&ctx.project.root)?,
            Routing::Registered(prefix) => {
                let live = registry.canonical_prefix(prefix);
                crate::scope::open_registered(&registry, live, Origin::Prefix)?
            }
        };
        if project.prefix == ctx.project.prefix && project.root == ctx.project.root {
            reject_stale_local(&registry, &project)?;
            ctx.project = project;
            ctx.registry = registry;
            ctx.routing = routing.clone();
            return Ok(());
        }
        ctx.lock = None;
        ctx.project = project;
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring its lock; retry".into(),
    ))
}

/// Recorded names and root remain stable through the config/registry transition.
pub fn reject_pending_rename(project: &Project) -> Result<()> {
    reject_pending_rename_at(Some(&project.root), &project.prefix)
}

pub fn reject_pending_rename_at(root: Option<&Path>, prefix: &str) -> Result<()> {
    let root = root.map(crate::rename::root_identity).transpose()?;
    for inventory in crate::rename::inventory::Inventory::pending()? {
        let inventory_root = crate::rename::root_identity(&inventory.root)?;
        if root.as_deref() == Some(inventory_root.as_path())
            || inventory.source == prefix
            || inventory.target == prefix
        {
            return Err(Error::Validation(format!(
                "a rename of {:?} to {:?} is unfinished; finish it with `tasks rename {} {}` or roll it back (see the design doc)",
                inventory.source, inventory.target, inventory.source, inventory.target
            )));
        }
    }
    Ok(())
}

/// A read command that takes an id: the id's prefix routes to its registered project, the
/// way `show` and every write command route. Routing is what the multi-project design
/// already specifies -- the subtree of an id is read from that id's own project -- and
/// `open_registered` is the same call `show` makes, so a prefix naming no project gives
/// `show`'s error rather than a misleading `task_not_found`.
///
/// An explicit `--project` or `--all-projects` is the caller naming the scope and wins
/// over the prefix.
pub fn open_id_read_ctx(
    dir: Option<&Path>,
    scope: &ScopeArgs,
    id: Option<&str>,
) -> Result<ReadCtx> {
    let mut ctx = open_read_ctx(dir, scope)?;
    if scope.project.is_some() || scope.all_projects {
        return Ok(ctx);
    }
    let Some(id) = id else {
        return Ok(ctx);
    };
    let id = parse_id(&ctx.registry, id)?;
    if let Scope::Local(project) = &ctx.scope
        && id.prefix != project.prefix
    {
        ctx.scope = Scope::Local(crate::scope::open_registered(
            &ctx.registry,
            &id.prefix,
            Origin::Id(&id),
        )?);
    }
    Ok(ctx)
}

pub struct ReadCtx {
    pub scope: Scope,
    pub registry: Registry,
    pub warnings: Vec<String>,
}

/// Parses a user-supplied id under the registry's current prefix names.
pub fn parse_id(registry: &Registry, id: &str) -> Result<TaskId> {
    Ok(registry.canonical_id(&TaskId::parse(id)?))
}

impl ReadCtx {
    /// Every task in scope plus the claim snapshot for the scope's prefixes: the prologue
    /// of each read command that reports claims.
    pub fn scan_with_claims(&self) -> Result<(Vec<Task>, crate::claims::ClaimSnapshot)> {
        let all = self.scope.scan()?;
        let prefixes = self.scope.prefixes();
        let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
        Ok((all, claims))
    }

    pub fn resolve_task(&self, id: &TaskId) -> Result<Option<Task>> {
        self.scope.resolve_task(&self.registry, id)
    }
}

pub fn start_dir(dir: Option<&Path>) -> Result<PathBuf> {
    Ok(match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    })
}

/// Read commands: the local project, one named registered project, or with
/// `all_projects` every reachable one. Both flags skip the local lookup entirely
/// (spec §3.2), so either works from a directory inside no project at all. A named
/// project is a `Local` scope like any other, so every command's output is what it
/// would be run inside that project's registered root — a worktree sharing the prefix
/// does not displace it, matching `add --project`.
pub fn open_read_ctx(dir: Option<&Path>, scope: &ScopeArgs) -> Result<ReadCtx> {
    let start = start_dir(dir)?;
    if scope.all_projects {
        let registry = Registry::load()?;
        let (scope, warnings) = Scope::open_all(&registry, &start)?;
        return Ok(ReadCtx {
            scope,
            registry,
            warnings,
        });
    }
    // The local arm locates before loading the registry, so a cwd outside every project
    // still reports `no_project` rather than a malformed registry's `config`.
    let (project, registry) = match &scope.project {
        Some(prefix) => {
            let registry = Registry::load()?;
            let project = crate::scope::open_registered(&registry, prefix, Origin::Prefix)?;
            (project, registry)
        }
        None => {
            let project = Project::locate(&start)?;
            let registry = Registry::load()?;
            reject_stale_local(&registry, &project)?;
            (project, registry)
        }
    };
    Ok(ReadCtx {
        scope: Scope::Local(project),
        registry,
        warnings: Vec::new(),
    })
}

/// $TASKS_OWNER, else the current git branch, else $USER. Whatever is chosen must
/// satisfy `validate_owner`; an invalid value is an error, never sanitized.
pub fn owner_name(project: &Project) -> Result<String> {
    let name = raw_owner_name(project)?;
    crate::format::validate_owner(&name)?;
    Ok(name)
}

/// Applies the field flags present in `fields` to `task`, validating each against the repo.
pub fn apply_fields(ctx: &Ctx, task: &mut Task, fields: &FieldArgs) -> Result<()> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    if let Some(body) = &fields.body {
        validate_body(body)?;
        task.body = body.clone();
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
    if let Some(every) = &fields.every {
        task.every = Some(crate::periodic::Interval::parse(every)?);
    }
    // Setting only. `edit --no-parallel` clears it before this runs, mirroring --no-tags.
    if fields.parallel {
        task.parallel = true;
    }
    // Additive, never a replacement: a triage `--tag` must not silently drop the tags a
    // task already carries. `edit` removes with `--rm-tag` / `--no-tags`.
    for tag in &fields.tags {
        if !task.tags.contains(tag) {
            task.tags.push(tag.clone());
        }
    }
    if !fields.depends.is_empty() {
        let mut dependencies = Vec::new();
        for dependency in &fields.depends {
            let id = parse_id(&ctx.registry, dependency)?;
            if id == task.id {
                return Err(Error::Cycle(format!("{id} -> {id}")));
            }
            if resolver.resolve_task(&id)?.is_none() {
                return Err(Error::UnresolvableId(id.to_string()));
            }
            if !dependencies.contains(&id) {
                dependencies.push(id);
            }
        }
        task.depends = dependencies;
        dep::ensure_acyclic(ctx, task)?;
    }
    if let Some(parent) = &fields.parent {
        task.parent = Some(parse_id(&ctx.registry, parent)?);
    }
    if let Some(source) = &fields.source {
        validate_line("source", source)?;
        task.source = Some(source.clone());
    }
    if let Some(spec) = &fields.spec {
        task.spec = Some(resolver.resolve_doc(DocKind::Spec, spec)?);
    }
    if let Some(plan) = &fields.plan {
        task.plan = Some(resolver.resolve_doc(DocKind::Plan, plan)?);
    }
    if let Some(step) = &fields.step {
        task.step = Some(step.clone());
    }
    match (&task.plan, &task.step) {
        (None, Some(_)) => return Err(Error::Validation("--step requires a plan".into())),
        (Some(plan), Some(step)) if !resolver.step_exists(plan, step)? => {
            return Err(Error::Validation(format!(
                "heading {step:?} not found in {plan}"
            )));
        }
        _ => {}
    }
    validate_task(task)?;
    resolver.project.validate_docs(task)
}

/// `save` for a task that does not exist yet: validates, then creates exclusively. Takes
/// the project rather than `Ctx` because the feedback command creates in another project.
pub fn create(project: &Project, registry: &Registry, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    project.validate_docs(task)?;
    project.create_task(registry, task)
}

pub fn load(ctx: &Ctx, id: &str) -> Result<Task> {
    ctx.project.read_task(&parse_id(&ctx.registry, id)?)
}

pub fn id_out(ctx: Ctx, task: &Task) -> Output {
    Output::Id(crate::output::IdOut {
        id: task.id.to_string(),
        warnings: ctx.warnings,
    })
}

pub fn append_note(task: &mut Task, by: &str, text: &str) -> Result<()> {
    validate_note_text(text)?;
    task.notes.push(Note {
        at: crate::time::now(),
        by: by.to_string(),
        text: text.to_string(),
    });
    Ok(())
}

/// Ids of dependencies that are open or unreachable.
pub fn open_deps(ctx: &Ctx, task: &Task) -> Result<Vec<String>> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let mut open = Vec::new();
    for dependency in &task.depends {
        match resolver.resolve_task(dependency)? {
            Some(task) if !task.status.is_open() => {}
            _ => open.push(dependency.to_string()),
        }
    }
    Ok(open)
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
        let worktree = ctx.project.root.display().to_string();
        let store = ctx.claims_mut()?;
        for entry_worktree in store
            .get(&id)
            .map(|claim| &claim.worktree)
            .into_iter()
            .chain(store.park(&id).map(|park| &park.worktree))
        {
            if entry_worktree != &worktree {
                return Err(Error::Validation(format!(
                    "{id} is already done here but has work in {entry_worktree}; finish that occurrence in its owning checkout"
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
    if to == Status::Done && task.status != Status::Done && !force {
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
    let completing = to == Status::Done && task.status != Status::Done;
    task.status = to;
    if completing && let Some(every) = task.every {
        let at = crate::time::now();
        let next = crate::periodic::add(crate::time::parse(&at)?, every).ok_or_else(|| {
            Error::Validation(format!(
                "completing now plus every {every} is not a representable timestamp"
            ))
        })?;
        task.last_done = Some(at);
        let owner = owner_name(&ctx.project)?;
        append_note(
            task,
            &owner,
            &format!("completed; next due {}", next.date()),
        )?;
    }
    Ok(())
}

/// Persists status claim intents only after validation.
///
/// Everything that can reject the change runs first and touches nothing. From there the
/// store and the task file move together, in the order that fails toward "claim held": a
/// claim with no file update makes an idle task look busy and self-heals when the session
/// dies, while a file update with no claim is the invisibility bug this exists to remove.
/// Warn when another checkout of this repository holds a copy of `id` whose `updated` is
/// newer than `loaded`, the stamp on the copy this command read. That copy carries
/// something the write about to land does not, and a merge has to drop one of the two.
///
/// A checkout that is merely behind says nothing: that is the resting state of any
/// long-lived worktree, and warning on it would train the reader to skip the line.
///
/// Two limits, chosen rather than overlooked. `updated` has second precision, so two writes
/// to one record in the same second in two checkouts compare equal and slip through. And
/// once this write lands, our stamp is `now()` and beats the sibling's, so the warning
/// fires once per divergence and then falls quiet until the sibling writes again. Closing
/// the second gap means comparing content, which warns on every worktree that is merely
/// behind -- noise that would cost more than it catches.
///
/// Never refuses a write: the report this answers asked for a signal, not a gate.
fn warn_on_newer_sibling_copies(ctx: &mut Ctx, id: &TaskId, loaded: &str) {
    let copies = match ctx.project.sibling_task_copies(id) {
        Ok(Some(copies)) => copies,
        Ok(None) => return,
        Err(error) => {
            ctx.warnings.push(format!(
                "could not check other checkouts for a newer copy of {id} ({error})"
            ));
            return;
        }
    };
    for copy in copies {
        match copy {
            SiblingCopy::Found { root, updated } if updated.as_str() > loaded => {
                ctx.warnings.push(format!(
                    "tasks/{id}.md in {} is newer than this copy ({updated} there, {loaded} \
                     here); this write may omit changes from that copy; reconcile the copies \
                     before merging",
                    root.display()
                ));
            }
            SiblingCopy::Found { .. } => {}
            SiblingCopy::Unreadable { root, detail } => ctx.warnings.push(format!(
                "tasks/{id}.md in {} could not be read ({detail}); whether that copy has \
                 diverged from this one is unknown",
                root.display()
            )),
        }
    }
}

pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    // Until this line the record still carries the stamp it was loaded with, which is the
    // only baseline the divergence check below has; the bump destroys it.
    let loaded = std::mem::replace(&mut task.updated, crate::time::now());
    validate_task(task)?;
    ctx.project.validate_docs(task)?;
    crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, task)?;
    warn_on_newer_sibling_copies(ctx, &task.id, &loaded);

    match ctx.pending_claim.take() {
        Some((id, ClaimIntent::Acquire(claim))) => {
            let store = ctx.claims_mut()?;
            // Captured, never assumed absent: a repeated `start` by the owner and a forced
            // takeover both write over an existing claim, and a blanket removal on failure
            // would unclaim work someone still holds.
            let previous = store.get(&id).cloned();
            let previous_park = store.park(&id).cloned();
            store.prune_dead();
            store.insert(&id, claim);
            store.save()?;

            let Err(error) = ctx.project.write_task(&ctx.registry, task) else {
                return Ok(());
            };
            let store = ctx.claims_mut()?;
            match (previous, previous_park) {
                (Some(previous), _) => store.insert(&id, previous),
                (None, Some(park)) => store.insert_park(&id, park),
                (None, None) => {
                    store.remove(&id);
                }
            }
            // Warnings on `Ctx` are dropped when a command returns `Err`, so recovery
            // guidance has to travel on the error itself.
            let suffix = match store.save() {
                Ok(()) => String::new(),
                Err(inner) => format!(
                    " (the previous claim on {id} could not be restored: {inner}; \
                     run `tasks start --force {id}` to reclaim it)"
                ),
            };
            Err(error.with_suffix(&suffix))
        }
        Some((id, ClaimIntent::Release { clear_park })) => {
            ctx.project.write_task(&ctx.registry, task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove(&id);
            if clear_park {
                store.remove_park(&id);
            }
            if let Err(error) = store.save() {
                let recovery = match task.status {
                    Status::Done => format!("run `tasks done {id}`"),
                    Status::Dropped => format!("run `tasks drop {id}`"),
                    Status::Blocked => format!("run `tasks block {id}`"),
                    Status::Todo | Status::Idea | Status::Doing => {
                        "the store is unchanged and a same-status edit will not retry cleanup"
                            .into()
                    }
                };
                ctx.warnings.push(format!(
                    "{id}'s status was saved but store cleanup failed ({error}); {recovery}"
                ));
            }
            Ok(())
        }
        Some((id, ClaimIntent::Park(park))) => {
            // Record first, then store: a note with no entry is a trail that says what was
            // intended, while an entry with no note would be state the record never saw.
            ctx.project.write_task(&ctx.registry, task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.insert_park(&id, park);
            if let Err(error) = store.save() {
                ctx.warnings.push(format!(
                    "the note landed, but parking on {id} was not updated ({error}); a previous \\
                     park entry, if any, is intact"
                ));
            }
            Ok(())
        }
        Some((_, ClaimIntent::PreserveStore)) => ctx.project.write_task(&ctx.registry, task),
        None => {
            // Resolve the store before writing so a corrupt store cannot hide a landed edit.
            ctx.claims_mut()?;
            ctx.project.write_task(&ctx.registry, task)?;
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

fn raw_owner_name(project: &Project) -> Result<String> {
    if let Ok(owner) = std::env::var("TASKS_OWNER")
        && !owner.is_empty()
    {
        return Ok(owner);
    }
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&project.root)
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

pub fn run(cli: Cli) -> Result<Output> {
    let dir = cli.dir.as_deref();
    match cli.command {
        Command::Init { prefix, force } => init::run(dir, prefix, force),
        Command::Rename { old, new, explain } => rename::run(old, new, explain),
        Command::Unregister { prefix } => unregister::run(prefix),
        Command::Projects {
            sort,
            reverse,
            closed,
            paths,
        } => projects::run(dir, sort.as_deref(), reverse, closed, paths),
        Command::Root { id } => root::run(id, dir),
        Command::Add {
            title,
            status,
            project,
            fields,
        } => {
            // The one write command that may run without a local project (spec §2):
            // an explicit target replaces the lookup, and the unchanged `add` validates
            // every field against whichever project it is handed.
            let routing = project.clone().map_or(Routing::Local, Routing::Registered);
            let mut ctx = match project {
                Some(prefix) => {
                    let registry = Registry::load()?;
                    let project =
                        crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
                    Ctx {
                        project,
                        registry,
                        warnings: Vec::new(),
                        lock: None,
                        routing: routing.clone(),
                        claims: None,
                        pending_claim: None,
                        recovered: false,
                    }
                }
                None => open_ctx(dir)?,
            };
            lock_and_revalidate(&mut ctx, &routing)?;
            add::run(ctx, title, status, fields)
        }
        Command::Show { id } => show::run(open_ctx(dir)?, id),
        Command::List {
            statuses,
            tags,
            owner,
            source,
            parent,
            sort,
            reverse,
            parked,
            periodic,
            scope,
        } => list::list(
            open_read_ctx(dir, &scope)?,
            statuses,
            tags,
            owner,
            source,
            parent,
            sort,
            reverse,
            parked,
            periodic,
        ),
        Command::Ready {
            size,
            parallel,
            limit,
            scope,
        } => list::ready(open_read_ctx(dir, &scope)?, size, parallel, limit),
        Command::Next { scope } => list::next(open_read_ctx(dir, &scope)?),
        Command::Sample {
            count,
            older_than,
            seed,
            scope,
        } => sample::sample(open_read_ctx(dir, &scope)?, count, older_than, seed),
        Command::Edit { id, args } => edit::run(open_id_write_ctx(dir, &id)?, id, args),
        Command::Prime { scope, closed } => list::prime(open_read_ctx(dir, &scope)?, closed),
        Command::Note { id, text } => status::note(open_id_write_ctx(dir, &id)?, id, text),
        Command::Start { id, force } => status::start(open_id_write_ctx(dir, &id)?, id, force),
        Command::Park {
            id,
            next_step,
            waiting_on,
        } => park::run(open_id_write_ctx(dir, &id)?, id, next_step, waiting_on),
        Command::Done { id, message, force } => status::close(
            open_id_write_ctx(dir, &id)?,
            id,
            Status::Done,
            message,
            force,
        ),
        Command::Drop { id, message } => status::close(
            open_id_write_ctx(dir, &id)?,
            id,
            Status::Dropped,
            message,
            false,
        ),
        Command::Block { id, message } => status::block(open_id_write_ctx(dir, &id)?, id, message),
        Command::Unblock { id } => status::unblock(open_id_write_ctx(dir, &id)?, id),
        Command::Dep { id, on, rm } => dep::run(open_id_write_ctx(dir, &id)?, id, on, rm),
        Command::Graph { format, all } => graph::run(open_ctx(dir)?, format, all),
        Command::Check => check::run(open_ctx(dir)?),
        Command::Tree { id, all, scope } => {
            tree::run(open_id_read_ctx(dir, &scope, id.as_deref())?, id, all)
        }
        Command::Tags { statuses, scope } => tags::run(open_read_ctx(dir, &scope)?, statuses),
        Command::Feedback {
            summary,
            category,
            body,
            recur,
            new,
        } => feedback::run(open_ctx(dir)?, summary, category, body, recur, new),
    }
}
