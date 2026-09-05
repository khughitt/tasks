pub mod add;
pub mod check;
pub mod dep;
pub mod edit;
pub mod feedback;
pub mod graph;
pub mod init;
pub mod list;
pub mod projects;
pub mod root;
pub mod show;
pub mod status;
pub mod tags;
pub mod tree;
pub mod unregister;

use crate::claims::{ClaimStore, Liveness, MutationLock};
use crate::cli::{Cli, Command, FieldArgs};
use crate::error::{Error, Result};
use crate::format::{validate_body, validate_note_text, validate_task};
use crate::model::{Note, Size, Status, Task, TaskId};
use crate::output::Output;
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, Resolver};
use crate::scope::{Origin, Scope};
use std::path::{Path, PathBuf};

/// What `save` must do to the claim store once every validation has passed. Recorded by the
/// guard in `transition`; **nothing is persisted until `save` acts on it.**
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release,
}

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for a write command; absent from read and create-only commands.
    pub lock: Option<MutationLock>,
    claims: Option<ClaimStore>,
    pending_claim: Option<(TaskId, ClaimIntent)>,
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
            (id.clone(), ClaimIntent::Release)
        });

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    Ok(Ctx {
        project: Project::locate(&start)?,
        registry: Registry::load()?,
        warnings: Vec::new(),
        lock: None,
        claims: None,
        pending_claim: None,
    })
}

pub fn open_write_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let mut ctx = open_ctx(dir)?;
    ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
    Ok(ctx)
}

pub struct ReadCtx {
    pub scope: Scope,
    pub registry: Registry,
    pub warnings: Vec<String>,
}

impl ReadCtx {
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

/// Read commands: the local project, or with `all_projects` every reachable registered
/// project and no local lookup at all (spec §3.2).
pub fn open_read_ctx(dir: Option<&Path>, all_projects: bool) -> Result<ReadCtx> {
    let start = start_dir(dir)?;
    if all_projects {
        let registry = Registry::load()?;
        let (scope, warnings) = Scope::open_all(&registry, &start)?;
        return Ok(ReadCtx {
            scope,
            registry,
            warnings,
        });
    }
    let project = Project::locate(&start)?;
    Ok(ReadCtx {
        scope: Scope::Local(project),
        registry: Registry::load()?,
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
    if !fields.tags.is_empty() {
        task.tags = fields.tags.clone();
    }
    if !fields.depends.is_empty() {
        let mut dependencies = Vec::new();
        for dependency in &fields.depends {
            let id = TaskId::parse(dependency)?;
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
        task.parent = Some(TaskId::parse(parent)?);
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
pub fn create(project: &Project, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    project.validate_docs(task)?;
    project.create_task(task)
}

pub fn load(ctx: &Ctx, id: &str) -> Result<Task> {
    ctx.project.read_task(&TaskId::parse(id)?)
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
    if !Status::can_transition(task.status, to) {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            to.as_str().into(),
        ));
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
        let open: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id)
            .iter()
            .map(|task| task.id.to_string())
            .collect();
        if !open.is_empty() {
            return Err(Error::OpenDescendants(task.id.to_string(), open.join(", ")));
        }
    }
    task.status = to;
    Ok(())
}

/// Persists status claim intents only after validation.
///
/// Everything that can reject the change runs first and touches nothing. From there the
/// store and the task file move together, in the order that fails toward "claim held": a
/// claim with no file update makes an idle task look busy and self-heals when the session
/// dies, while a file update with no claim is the invisibility bug this exists to remove.
pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    ctx.project.validate_docs(task)?;
    crate::hierarchy::validate_parent(&ctx.project, task)?;

    match ctx.pending_claim.take() {
        Some((id, ClaimIntent::Acquire(claim))) => {
            let store = ctx.claims_mut()?;
            // Captured, never assumed absent: a repeated `start` by the owner and a forced
            // takeover both write over an existing claim, and a blanket removal on failure
            // would unclaim work someone still holds.
            let previous = store.get(&id).cloned();
            store.prune_dead();
            store.insert(&id, claim);
            store.save()?;

            let Err(error) = ctx.project.write_task(task) else {
                return Ok(());
            };
            let store = ctx.claims_mut()?;
            match previous {
                Some(previous) => store.insert(&id, previous),
                None => {
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
        Some((id, ClaimIntent::Release)) => {
            ctx.project.write_task(task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove(&id);
            if let Err(error) = store.save() {
                // The task is closed now, so `start --force` cannot recover this:
                // `can_transition` rejects `done -> doing`. Re-running the same closing
                // command can, because a same-status transition still releases.
                ctx.warnings.push(format!(
                    "{id} was closed but its claim was not released ({error}); \
                     re-run the same command to retry the release"
                ));
            }
            Ok(())
        }
        None => {
            // Resolve the store before writing so a corrupt store cannot hide a landed edit.
            ctx.claims_mut()?;
            ctx.project.write_task(task)?;
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
        Command::Unregister { prefix } => unregister::run(prefix),
        Command::Projects => projects::run(dir),
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
            let ctx = match project {
                Some(prefix) => {
                    let registry = Registry::load()?;
                    let project =
                        crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
                    Ctx {
                        project,
                        registry,
                        warnings: Vec::new(),
                        lock: None,
                        claims: None,
                        pending_claim: None,
                    }
                }
                None => open_ctx(dir)?,
            };
            add::run(ctx, title, status, fields)
        }
        Command::Show { id } => show::run(open_ctx(dir)?, id),
        Command::List {
            statuses,
            tags,
            owner,
            parent,
            sort,
            reverse,
            all_projects,
        } => list::list(
            open_read_ctx(dir, all_projects)?,
            statuses,
            tags,
            owner,
            parent,
            sort,
            reverse,
        ),
        Command::Ready {
            size,
            limit,
            all_projects,
        } => list::ready(open_read_ctx(dir, all_projects)?, size, limit),
        Command::Next { all_projects } => list::next(open_read_ctx(dir, all_projects)?),
        Command::Edit {
            id,
            title,
            status,
            force,
            no_parent,
            fields,
        } => edit::run(
            open_write_ctx(dir)?,
            id,
            title,
            status,
            force,
            no_parent,
            fields,
        ),
        Command::Prime { all_projects } => list::prime(open_read_ctx(dir, all_projects)?),
        Command::Note { id, text } => status::note(open_write_ctx(dir)?, id, text),
        Command::Start { id, force } => status::start(open_write_ctx(dir)?, id, force),
        Command::Done { id, message, force } => {
            status::close(open_write_ctx(dir)?, id, Status::Done, message, force)
        }
        Command::Drop { id, message } => {
            status::close(open_write_ctx(dir)?, id, Status::Dropped, message, false)
        }
        Command::Block { id, message } => status::block(open_write_ctx(dir)?, id, message),
        Command::Unblock { id } => status::unblock(open_write_ctx(dir)?, id),
        Command::Dep { id, on, rm } => dep::run(open_write_ctx(dir)?, id, on, rm),
        Command::Graph { format, all } => graph::run(open_ctx(dir)?, format, all),
        Command::Check => check::run(open_ctx(dir)?),
        Command::Tree {
            id,
            all,
            all_projects,
        } => tree::run(open_read_ctx(dir, all_projects)?, id, all),
        Command::Tags {
            statuses,
            all_projects,
        } => tags::run(open_read_ctx(dir, all_projects)?, statuses),
        Command::Feedback {
            summary,
            category,
            body,
            recur,
            new,
        } => feedback::run(open_ctx(dir)?, summary, category, body, recur, new),
    }
}
