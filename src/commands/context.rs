use super::ClaimIntent;
use crate::claims::{ClaimStore, MutationLock};
use crate::cli::ScopeArgs;
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::model::{Complexity, Task, TaskId};
use crate::output::Output;
use crate::registry::Registry;
use crate::repo::Project;
use crate::scope::{Origin, Scope};
use crate::shorthand::Shorthand;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Ctx {
    pub host: Arc<HostConfig>,
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for a write command; absent from reads and during an interactive edit.
    pub lock: Option<MutationLock>,
    /// The host-wide holds lock, taken after `lock` by every acquire that records holds
    /// and every claim replacement that changes them. Held until the command ends: past
    /// `save`'s publish, record write, and rollback (docs/user-guide.md).
    pub(super) holds_lock: Option<MutationLock>,
    pub(super) routing: Routing,
    pub(super) claims: Option<ClaimStore>,
    pub(super) pending_claim: Option<(TaskId, ClaimIntent)>,
    /// This close only finishes a previous completion's pending claim-store cleanup.
    pub recovered: bool,
    /// Set by `reassess` to what was given -- `Some(level)` for `--complexity <level>`,
    /// `None` for `--no-complexity`; the outer `Option` is absent when no reassessment
    /// was requested. Taken once by `save`, which performs the actual removal and, only
    /// once it knows whether that reached disk, reports success or names the retry.
    pub(super) clear_escalation: Option<Option<Complexity>>,
    /// Set by the claim guard when this command displaces a foreign claim: the summary a
    /// task record may keep. Host, pid, and worktree stay in the warning and the claim
    /// store, which never reach git.
    pub takeover: Option<String>,
    /// `start --reason` / `edit --reason`: what lets `--force` past a held need (lanes/needs
    /// docs/task-model.md). Set by the command before the hold guard runs.
    pub(super) need_reason: Option<String>,
    /// The held needs this command acquires past, with their holders: recorded by the hold
    /// guard, turned into the acquired task's notes by `record_need_overrides` before
    /// `save`. The holders' notes wait for `note_need_holders`, after `save` has landed.
    pub(super) need_overrides: Vec<(String, crate::holds::Holder)>,
    /// What a bare id suffix means in this invocation: the caller's project, not the one
    /// the command routed to.
    pub shorthand: Shorthand,
}

impl Ctx {
    pub(super) fn new(
        project: Project,
        registry: Registry,
        routing: Routing,
        shorthand: Shorthand,
        host: Arc<HostConfig>,
    ) -> Self {
        Self {
            host,
            project,
            registry,
            shorthand,
            warnings: Vec::new(),
            lock: None,
            holds_lock: None,
            routing,
            claims: None,
            pending_claim: None,
            recovered: false,
            clear_escalation: None,
            takeover: None,
            need_reason: None,
            need_overrides: Vec::new(),
        }
    }
}

pub fn open_ctx(dir: Option<&Path>, host: Arc<HostConfig>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    let project = Project::locate(&start)?;
    let registry = Registry::load()?;
    reject_stale_local(&registry, &project)?;
    let shorthand = Shorthand::located(start, project.prefix.clone());
    Ok(Ctx::new(
        project,
        registry,
        Routing::Local,
        shorthand,
        host.clone(),
    ))
}

/// Opens a write context for a command that takes an optional `--project <prefix>` target
/// (`add`, `commit`): an explicit target replaces the lookup, and `lock_and_revalidate`
/// acquires the project lock and revalidates.
pub fn open_project_write_ctx(
    dir: Option<&Path>,
    project: Option<String>,
    host: Arc<HostConfig>,
) -> Result<Ctx> {
    open_project_write_ctx_inner(dir, project, host, false)
}

pub fn open_project_write_ctx_allow_retired(
    dir: Option<&Path>,
    project: Option<String>,
    host: Arc<HostConfig>,
) -> Result<Ctx> {
    open_project_write_ctx_inner(dir, project, host, true)
}

fn open_project_write_ctx_inner(
    dir: Option<&Path>,
    project: Option<String>,
    host: Arc<HostConfig>,
    allow_retired: bool,
) -> Result<Ctx> {
    let routing = project.clone().map_or(Routing::Local, Routing::Registered);
    let mut ctx = match project {
        Some(prefix) => {
            let registry = Registry::load()?;
            let project = crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
            let shorthand = Shorthand::new(start_dir(dir)?);
            Ctx::new(project, registry, routing.clone(), shorthand, host)
        }
        None => open_ctx(dir, host)?,
    };
    lock_and_revalidate_inner(&mut ctx, &routing, allow_retired)?;
    Ok(ctx)
}

/// A checkout whose own prefix the registry has retired. Nothing else sees this:
/// `open_registered` validates the registered destination, which after a rename is
/// perfectly consistent, and an in-tree hit in `Project::locate` does not read the registry.
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

/// The context for the project an id names. A prefix matching the local project
/// keeps that checkout, so `-C` and worktrees still win; any other prefix is followed
/// through the registry. With no local project, the registry is the only target.
/// The warning a single-project read carries for a retired project, or None.
pub fn retired_warning(project: &Project) -> Option<String> {
    project.retired.as_ref().map(|r| {
        let successor = r
            .successor
            .as_ref()
            .map(|s| format!("; successor {s}"))
            .unwrap_or_default();
        format!("{} is retired ({}){successor}", project.prefix, r.date)
    })
}

pub(super) fn open_id_ctx(dir: Option<&Path>, id: &str, host: Arc<HostConfig>) -> Result<Ctx> {
    let mut ctx = open_id_ctx_unwarned(dir, id, host.clone())?;
    ctx.warnings.extend(retired_warning(&ctx.project));
    Ok(ctx)
}

pub fn open_id_unlocked_ctx(dir: Option<&Path>, id: &str, host: Arc<HostConfig>) -> Result<Ctx> {
    open_id_ctx(dir, id, host.clone())
}

fn open_id_ctx_unwarned(dir: Option<&Path>, id: &str, host: Arc<HostConfig>) -> Result<Ctx> {
    let mut ctx = match open_ctx(dir, host.clone()) {
        Ok(ctx) => ctx,
        Err(Error::NoProject(_)) => {
            let registry = Registry::load()?;
            let shorthand = Shorthand::new(start_dir(dir)?);
            let id = parse_id(&registry, &shorthand, id)?;
            let project = crate::scope::open_registered(&registry, &id.prefix, Origin::Id(&id))?;
            let routing = Routing::Registered(id.prefix);
            return Ok(Ctx::new(
                project,
                registry,
                routing,
                shorthand,
                host.clone(),
            ));
        }
        Err(error) => return Err(error),
    };
    let id = parse_id(&ctx.registry, &ctx.shorthand, id)?;
    if id.prefix != ctx.project.prefix {
        ctx.project = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
        ctx.routing = Routing::Registered(id.prefix);
    }
    Ok(ctx)
}

/// The lock and claim store follow the resolved project, including outside a checkout.
pub fn open_id_write_ctx(dir: Option<&Path>, id: &str, host: Arc<HostConfig>) -> Result<Ctx> {
    let mut ctx = open_id_ctx(dir, id, host.clone())?;
    let routing = ctx.routing.clone();
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
    lock_and_revalidate_inner(ctx, routing, false)
}

fn lock_and_revalidate_inner(ctx: &mut Ctx, routing: &Routing, allow_retired: bool) -> Result<()> {
    for _ in 0..4 {
        ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
        let registry = Registry::load()?;
        reject_pending_rename(&ctx.project)?;
        let project = match routing {
            Routing::Local => Project {
                worktree: ctx.project.worktree.clone(),
                ..Project::open(&ctx.project.records)?
            },
            Routing::Registered(prefix) => {
                let live = registry.canonical_prefix(prefix);
                crate::scope::open_registered(&registry, live, Origin::Prefix)?
            }
        };
        if project.prefix == ctx.project.prefix && project.records == ctx.project.records {
            reject_stale_local(&registry, &project)?;
            if !allow_retired {
                reject_retired(&registry, &project)?;
            }
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

/// A retired project takes no writes (docs/project-operations.md). The registered copy (its
/// `scope::config_dir`: a detached project's records directory, else its root) is the
/// authority, but a worktree made before the retirement has no `[retired]` of its own
/// until a merge, so either declaring it is enough. `reject_stale_local` only compares the
/// prefix with the registry's aliases; a registered checkout that does not open, or whose
/// config names another prefix, is an error here, never "not retired": the write it would
/// have guarded does not happen.
pub fn reject_retired(registry: &Registry, project: &Project) -> Result<()> {
    let Some(retirement) = effective_retirement(registry, project)? else {
        return Ok(());
    };
    let authority = crate::scope::config_dir(registry, &project.prefix)
        .unwrap_or_else(|| project.records.clone());
    let successor = retirement
        .successor
        .as_ref()
        .map(|s| format!("; its successor is {s}"))
        .unwrap_or_default();
    Err(Error::RetiredProject(format!(
        "{} was retired on {}{successor}; run `tasks unretire` in {} to write here again",
        project.prefix,
        retirement.date,
        authority.display()
    )))
}

/// The retirement `project` is under: its own `[retired]`, else its registered copy's
/// when it is another copy (a worktree made before the retirement). The registered copy
/// is decided by records location: two records worktrees of a detached project share one
/// code root.
pub fn effective_retirement(
    registry: &Registry,
    project: &Project,
) -> Result<Option<crate::repo::Retirement>> {
    let registered = match crate::scope::config_dir(registry, &project.prefix) {
        Some(dir) if dir != project.records => Some(
            crate::scope::open_registered(registry, &project.prefix, Origin::Prefix).map_err(
                |error| {
                    Error::Config(format!(
                        "cannot tell whether {} is retired: its registered checkout {} did not \
                         open ({error}); fix that checkout, then retry",
                        project.prefix,
                        dir.display()
                    ))
                },
            )?,
        ),
        _ => None,
    };
    Ok(project
        .retired
        .clone()
        .or(registered.and_then(|p| p.retired)))
}

/// Recorded names and root remain stable through the config/registry transition.
pub fn reject_pending_rename(project: &Project) -> Result<()> {
    reject_pending_rename_at(Some(&project.root), &project.prefix)
}

pub fn reject_pending_rename_at(root: Option<&Path>, prefix: &str) -> Result<()> {
    match pending_rename_at(root, prefix)? {
        Some(detail) => Err(Error::Validation(detail)),
        None => Ok(()),
    }
}

/// The refusal text of an unfinished rename naming this root or prefix, if there is one;
/// every other failure (an unreadable inventory) is an error.
pub fn pending_rename_at(root: Option<&Path>, prefix: &str) -> Result<Option<String>> {
    let root = root.map(crate::rename::root_identity).transpose()?;
    for inventory in crate::rename::inventory::Inventory::pending()? {
        let inventory_root = crate::rename::root_identity(&inventory.root)?;
        if root.as_deref() == Some(inventory_root.as_path())
            || inventory.source == prefix
            || inventory.target == prefix
        {
            return Ok(Some(format!(
                "a rename of {:?} to {:?} is unfinished; finish it with `tasks rename {} {}` or roll it back (see the design doc)",
                inventory.source, inventory.target, inventory.source, inventory.target
            )));
        }
    }
    Ok(None)
}

/// The refusal text of a pending move naming this prefix, if there is one.
pub fn pending_move(prefix: &str) -> Result<Option<String>> {
    for journal in crate::moving::journal::Journal::pending()? {
        if journal.names_prefix(prefix) {
            return Ok(Some(match journal.pending_error() {
                Error::MovePending(detail) => detail,
                other => unreachable!("pending_error is move_pending, got {other:?}"),
            }));
        }
    }
    Ok(None)
}

pub fn reject_pending_operations(root: Option<&Path>, prefix: &str) -> Result<()> {
    reject_pending_rename_at(root, prefix)?;
    for journal in crate::moving::journal::Journal::pending()? {
        if journal.names_prefix(prefix) {
            return Err(journal.pending_error());
        }
    }
    if let Some(journal) = crate::migrating::journal::pending_naming(prefix, None)? {
        return Err(journal.pending_error());
    }
    Ok(())
}

/// A read command that takes an id: the id's prefix routes to its registered project, the
/// way `show` and every write command route. Routing is what the multi-project design
/// already specifies -- the subtree of an id is read from that id's own project -- and
/// `open_registered` is the same call `show` makes, so a prefix naming no project gives
/// `show`'s error rather than a misleading `task_not_found`.
///
/// An explicit `--project`, `--group` or `--all-projects` is the caller naming the scope
/// and wins over the prefix.
pub fn open_id_read_ctx(
    dir: Option<&Path>,
    scope: &ScopeArgs,
    id: Option<&str>,
    host: Arc<HostConfig>,
) -> Result<ReadCtx> {
    if scope.project.is_some() || scope.all_projects || scope.group.is_some() {
        return open_read_ctx(dir, scope, host.clone());
    }
    let Some(id) = id else {
        return open_read_ctx(dir, scope, host.clone());
    };
    let ctx = open_id_ctx(dir, id, host.clone())?;
    Ok(ReadCtx {
        host: host.clone(),
        scope: Scope::Local(Box::new(ctx.project)),
        registry: ctx.registry,
        warnings: ctx.warnings,
        shorthand: ctx.shorthand,
    })
}

pub(super) fn open_filtered_read_ctx(
    dir: Option<&Path>,
    scope: &ScopeArgs,
    parent: Option<&str>,
    under: Option<&str>,
    host: Arc<HostConfig>,
) -> Result<ReadCtx> {
    if scope.project.is_none()
        && !scope.all_projects
        && scope.group.is_none()
        && let (Some(parent), Some(under)) = (parent, under)
    {
        // Agree before routing, retaining the original checkout's shorthand and errors.
        let (registry, shorthand) = match open_ctx(dir, host.clone()) {
            Ok(ctx) => (ctx.registry, ctx.shorthand),
            Err(Error::NoProject(_)) => (Registry::load()?, Shorthand::new(start_dir(dir)?)),
            Err(error) => return Err(error),
        };
        let parent = parse_id(&registry, &shorthand, parent)?;
        let under = parse_id(&registry, &shorthand, under)?;
        if parent.prefix != under.prefix {
            return Err(Error::Validation(format!(
                "--parent {parent} and --under {under} name different projects; \
                 use IDs from one project or select an explicit scope"
            )));
        }
    }
    open_id_read_ctx(dir, scope, parent.or(under), host.clone()).map_err(|error| match error {
        Error::NoProject(dir) => Error::NoProjectNext {
            dir,
            next: "choose --all-projects or --project <prefix> to set the read scope".into(),
        },
        error => error,
    })
}

pub struct ReadCtx {
    pub host: Arc<HostConfig>,
    pub scope: Scope,
    pub registry: Registry,
    pub warnings: Vec<String>,
    pub shorthand: Shorthand,
}

/// Parses a user-supplied id, full or a bare suffix, under the registry's current prefix
/// names.
pub fn parse_id(registry: &Registry, shorthand: &Shorthand, id: &str) -> Result<TaskId> {
    Ok(registry.canonical_id(&shorthand.parse(registry, id)?))
}

impl ReadCtx {
    /// Every in-scope project's configured colour, keyed by prefix.
    pub fn project_colors(&self) -> crate::output::ProjectColors {
        self.scope
            .projects()
            .iter()
            .filter_map(|project| project.color.map(|color| (project.prefix.clone(), color)))
            .collect()
    }

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
        None => crate::repo::current_start()?,
    })
}

/// Read commands: the local project, one named registered project, or with
/// `all_projects` every reachable one, or with `group` the reachable members of that
/// group. All three flags skip the local lookup entirely
/// (docs/task-model.md), so either works from a directory inside no project at all. A named
/// project is a `Local` scope like any other, so every command's output is what it
/// would be run inside that project's registered root — a worktree sharing the prefix
/// does not displace it, matching `add --project`.
pub fn open_read_ctx(
    dir: Option<&Path>,
    scope: &ScopeArgs,
    host: Arc<HostConfig>,
) -> Result<ReadCtx> {
    let start = start_dir(dir)?;
    if scope.all_projects || scope.group.is_some() {
        let registry = Registry::load()?;
        let (scope, warnings) = match &scope.group {
            Some(name) => Scope::open_group(&registry, &start, name)?,
            None => Scope::open_all(&registry, &start)?,
        };
        return Ok(ReadCtx {
            host: host.clone(),
            scope,
            registry,
            warnings,
            shorthand: Shorthand::new(start),
        });
    }
    // The local arm locates before loading the registry, so a cwd outside every project
    // outside git still reports `no_project` rather than a malformed registry's `config`.
    let (project, registry, shorthand) = match &scope.project {
        Some(prefix) => {
            let registry = Registry::load()?;
            let project = crate::scope::open_registered(&registry, prefix, Origin::Prefix)?;
            (project, registry, Shorthand::new(start))
        }
        None => {
            let project = Project::locate(&start)?;
            let registry = Registry::load()?;
            reject_stale_local(&registry, &project)?;
            let shorthand = Shorthand::located(start, project.prefix.clone());
            (project, registry, shorthand)
        }
    };
    let warnings = retired_warning(&project).into_iter().collect();
    Ok(ReadCtx {
        host: host.clone(),
        scope: Scope::Local(Box::new(project)),
        registry,
        warnings,
        shorthand,
    })
}

/// Commits what this write command wrote, while its locks are still held (record-commit
/// docs/task-model.md), and returns the absolute paths committed. Idempotent: the journal is
/// drained, so a second call commits nothing.
pub fn finish(ctx: &mut Ctx) -> std::collections::BTreeSet<std::path::PathBuf> {
    crate::record_commit::commit_written(&mut ctx.warnings)
}

pub fn finish_report(ctx: &mut Ctx) -> Vec<crate::record_commit::Committed> {
    crate::record_commit::commit_written_report(&mut ctx.warnings)
}

pub fn id_out(mut ctx: Ctx, task: &Task) -> Output {
    finish(&mut ctx);
    Output::Id(crate::output::IdOut {
        id: task.id.to_string(),
        warnings: ctx.warnings,
    })
}

pub fn revalidate_retained_ctx(ctx: &mut Ctx) -> Result<()> {
    let registry = Registry::load()?;
    reject_pending_rename(&ctx.project)?;
    let project = match &ctx.routing {
        Routing::Local => Project {
            worktree: ctx.project.worktree.clone(),
            ..Project::open(&ctx.project.records)?
        },
        Routing::Registered(prefix) => crate::scope::open_registered(
            &registry,
            registry.canonical_prefix(prefix),
            Origin::Prefix,
        )?,
    };
    if project.prefix != ctx.project.prefix || project.records != ctx.project.records {
        return Err(Error::RetainedStaleBasis(
            "Project pairing changed while locking; retry from the exact current pairing".into(),
        ));
    }
    reject_stale_local(&registry, &project)?;
    reject_retired(&registry, &project)?;
    reject_pending_operations(Some(&project.root), &project.prefix)?;
    ctx.project = project;
    ctx.registry = registry;
    Ok(())
}
