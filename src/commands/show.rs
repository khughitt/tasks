use super::Ctx;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::{DepInfo, Output, Related, ShowFields, ShowOut};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, DocRead, Fetch, Located, Resolver, UnavailableCause, has_heading};
use std::collections::HashMap;
use std::path::Path;
use time::OffsetDateTime;

pub fn run(mut ctx: Ctx, id: String, fetch: Fetch, all_projects: bool) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claims = crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, mut all) = read_record(&mut ctx, &id, &claims)?;
    let project = recorded.as_ref().unwrap_or(&ctx.project);
    if all_projects {
        widen(&ctx.registry, project, &mut all, &mut ctx.warnings)?;
    }
    let now = crate::time::parse(&crate::time::now())?;
    let fields = describe(
        project,
        &ctx.registry,
        &task.id,
        &all,
        Some(&claims),
        &mut ctx.warnings,
        now,
        fetch,
        &ctx.host,
    )?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
}

/// `show --all-projects`: add every other reachable registered project's records to the
/// scan `describe` reads, so its inverse rows (dependents, followers) see them. The
/// project's own records stay the copy already read, so worktree state is kept and no
/// record appears twice; same-project fields (parent, children, lane) are unchanged.
fn widen(
    registry: &Registry,
    project: &Project,
    all: &mut Vec<Task>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let (scope, opened) = crate::scope::Scope::open_all(registry, &project.root)?;
    warnings.extend(opened);
    let own = registry.canonical_prefix(&project.prefix);
    // Not `scan_each`: that would scan this project's registered copy only to drop it.
    for other in scope.projects().iter().filter(|other| other.prefix != own) {
        all.extend(
            other
                .scan()
                .map_err(|error| error.in_project(&other.prefix))?,
        );
    }
    Ok(())
}

pub(super) fn read_record(
    ctx: &mut Ctx,
    id: &crate::model::TaskId,
    claims: &crate::claims::ClaimSnapshot,
) -> Result<(Option<Project>, Task, Vec<Task>)> {
    let found = match ctx.project.read_task(id) {
        Ok(task) => (None, task, ctx.project.scan()?),
        Err(Error::TaskNotFound(_)) => {
            let (worktree, project, task, all) = match recorded_elsewhere(id, claims) {
                Err(Error::TaskNotFound(_)) => {
                    let scan = crate::recovery::discover(&ctx.project, id);
                    return Err(super::recovery::inspection_error(id, &scan));
                }
                result => result?,
            };
            ctx.warnings.push(format!(
                "{id} exists only in {worktree}; shown from that checkout"
            ));
            (Some(project), task, all)
        }
        Err(error) => return Err(error),
    };
    let id = ctx.registry.canonical_id(&found.1.id);
    match crate::moving::journal::refuse_pending_id(&id) {
        Err(error @ Error::MovePending(_)) => ctx.warnings.push(error.to_string()),
        result => result?,
    }
    // Current contract (docs/user-guide.md): a pending migration freezes its whole source and successor.
    for journal in crate::migrating::journal::MigrationJournal::pending()? {
        let warning = journal.pending_error().to_string();
        if journal.names_prefix(&id.prefix) && !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    Ok(found)
}

/// Current contract (docs/project-operations.md): a record this checkout lacks is read from the checkout named by
/// its live claim, or else by its park. A record present here is always read here.
pub(super) fn recorded_elsewhere<'a>(
    id: &crate::model::TaskId,
    claims: &'a crate::claims::ClaimSnapshot,
) -> Result<(&'a str, Project, Task, Vec<Task>)> {
    let named = claims
        .held(id)
        .map(|claim| ("claimed", claim.worktree.as_str()))
        .or_else(|| {
            claims
                .park(id)
                .map(|park| ("parked", park.worktree.as_str()))
        });
    let Some((how, worktree)) = named else {
        return Err(Error::TaskNotFound(id.to_string()));
    };
    let unavailable = |why: String| Error::TaskNotFoundNext {
        id: id.to_string(),
        next: format!("{how} in {worktree}, which is unavailable{why}"),
    };
    match super::parked::open_recorded(id, Path::new(worktree)) {
        Ok(Some((project, task, all))) => Ok((worktree, project, task, all)),
        Ok(None) => Err(unavailable(String::new())),
        Err(error) => Err(unavailable(format!(": {error}"))),
    }
}

/// The `show` view of `task`, which lives in `project`. `all` is a scan containing that
/// project's tasks (a union is fine: dependencies, parent, and children are looked up
/// by id, and ids carry their prefix). Unreachable dependencies and a missing parent are
/// pushed to `warnings`, never errors. Under `Fetch::Disabled` no provider command runs,
/// and an external plan's `step_found` is omitted: a skipped fetch is not an unavailable plan.
#[allow(clippy::too_many_arguments)]
pub fn describe(
    project: &Project,
    registry: &Registry,
    id: &TaskId,
    all: &[Task],
    claims: Option<&crate::claims::ClaimSnapshot>,
    warnings: &mut Vec<String>,
    now: OffsetDateTime,
    fetch: Fetch,
    host: &crate::config::HostConfig,
) -> Result<ShowFields> {
    let (task, (attached, problems)) = {
        let _guard = crate::attachments::transaction::read_guard(project, |owner| owner == id)?;
        let task = project.read_task(id)?;
        let files = crate::attachments::audit_task(project, &task)?;
        (task, files)
    };
    let mut resolver = Resolver::new(project, registry, host);
    let deps = crate::review::Deps::of_scan(std::slice::from_ref(&task), |id| {
        crate::dependency::resolve(registry, id, |id| {
            match all.iter().find(|candidate| candidate.id == *id) {
                Some(found) => Ok(Some(found.clone())),
                None => resolver.resolve_canonical_task(id),
            }
        })
    })?;
    warnings.extend(deps.warnings(std::slice::from_ref(&task)));
    let review_reason = crate::review::state(
        &task,
        crate::hierarchy::children(all, &task.id, registry).len(),
        &deps,
        now,
    );
    resolver.set_fetch(fetch);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        let mut row = resolve_row(registry, all, &resolver, dependency)?;
        row.piece = crate::pieces::is_piece(registry, &task, dependency);
        if !row.resolved {
            warnings.push(format!("dependency {dependency} is unreachable"));
        }
        depends_on.push(row);
    }
    let mut related_to = Vec::new();
    for related in &task.related {
        let row = resolve_row(registry, all, &resolver, related)?;
        if !row.resolved {
            warnings.push(format!("related task {related} is unreachable"));
        }
        related_to.push(row);
    }
    let mut follows_tasks = Vec::new();
    for related in &task.follows {
        let row = resolve_row(registry, all, &resolver, related)?;
        if !row.resolved {
            warnings.push(format!("follows task {related} is unreachable"));
        }
        follows_tasks.push(row);
    }
    let mut wake_targets = Vec::new();
    for target in &task.wake_on {
        let row = resolve_row(registry, all, &resolver, target)?;
        if !row.resolved {
            let already_warned = warnings
                .iter()
                .any(|w| w.contains(&format!("wake target {target} is unreachable")));
            if !already_warned {
                warnings.push(format!("wake target {target} is unreachable"));
            }
        }
        wake_targets.push(row);
    }
    let source_id = task
        .source
        .as_deref()
        .and_then(|source| crate::related::source_task_id(registry, source));
    let source_task = match &source_id {
        Some(id) => {
            let row = resolve_row(registry, all, &resolver, id)?;
            if !row.resolved {
                warnings.push(format!("source task {id} is unreachable"));
            }
            Some(row)
        }
        None => None,
    };
    let step_found = match (&task.plan, &task.step) {
        (Some(plan), None) if crate::format::external_doc(plan)?.is_some() => None,
        (Some(plan), Some(step)) if crate::format::external_doc(plan)?.is_some() => {
            resolver.set_fetch_budget(crate::doc_fetch::OPTIONAL_BUDGET);
            match resolver.read(DocKind::Plan, plan)? {
                DocRead::Available(doc) => Some(has_heading(&doc.body, step)),
                DocRead::Unavailable {
                    cause: UnavailableCause::FetchDisabled,
                    ..
                } => None,
                DocRead::Unavailable { reference, cause } => {
                    warnings.push(format!(
                        "{}plan {reference} is unavailable: {}",
                        if task.status.is_open() {
                            ""
                        } else {
                            "historical "
                        },
                        cause.detail()
                    ));
                    if task.status.is_open() {
                        None
                    } else {
                        Some(false)
                    }
                }
            }
        }
        (Some(plan), step)
            if !task.status.is_open()
                && matches!(resolver.locate(plan)?, Located::Missing)
                // Existing non-file paths and metadata errors must not become missing plans.
                && !project.doc_path(plan).try_exists()?
                && match project.main_checkout_root()? {
                    Some(main) => !main.join(plan).try_exists()?,
                    None => true,
                } =>
        {
            warnings.push(format!("historical plan {plan} is unavailable"));
            step.as_ref().map(|_| false)
        }
        (Some(plan), Some(step)) => match resolver.locate(plan)? {
            Located::Here(_) | Located::Main { .. } | Located::Code(_) => {
                Some(resolver.step_exists(plan, step)?)
            }
            Located::Missing => {
                let here = project.doc_path(plan);
                let existing = if here.try_exists()? {
                    Some(here)
                } else {
                    match project.main_checkout_root()? {
                        Some(main) if main.join(plan).try_exists()? => Some(main.join(plan)),
                        _ => None,
                    }
                };
                if let Some(path) = existing {
                    warnings.push(format!(
                        "plan {} is not a regular file; its step is not checked",
                        path.display()
                    ));
                }
                None
            }
        },
        _ => None,
    };
    let related = |task: &Task| Related {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: task.status,
        updated: task.updated.clone(),
    };
    let (parent, unresolved_parent) = match &task.parent {
        Some(id) => {
            let canonical = registry.canonical_id(id);
            match all.iter().find(|candidate| candidate.id == canonical) {
                Some(found) => (Some(related(found)), None),
                None => {
                    warnings.push(format!("parent {id} not found"));
                    (None, Some(canonical.to_string()))
                }
            }
        }
        None => (None, None),
    };
    let identity = crate::follows::identity_set(registry, &task, |id| {
        match all
            .iter()
            .find(|candidate| registry.canonical_id(&candidate.id) == *id)
        {
            Some(found) => Ok(Some(found.clone())),
            None => resolver.resolve_canonical_task(id),
        }
    });
    if let Some(warning) = &identity.warning {
        warnings.push(warning.clone());
    }
    let following = crate::follows::followers(registry, all, &identity);
    let canonical = registry.canonical_id(&task.id);
    let depending = crate::query::ready_unique(
        registry,
        crate::dependency::dependents(registry, all, &canonical)
            .filter(|candidate| candidate.status != crate::model::Status::Moved)
            .collect(),
    );
    let dependents = depending.iter().map(|task| related(task)).collect();
    let followed_by = following.iter().map(|task| related(task)).collect();
    let mut kids = crate::hierarchy::children(all, &task.id, registry);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
    warnings.extend(problems.iter().map(crate::attachments::Problem::line));
    let spec_path = match &task.spec {
        Some(path) => Some(resolver.abs(DocKind::Spec, path)?),
        None => None,
    };
    let plan_path = match &task.plan {
        Some(path) => Some(resolver.abs(DocKind::Plan, path)?),
        None => None,
    };
    let inventory =
        crate::inventory::build(&resolver, project, &task, step_found, fetch, warnings)?;
    warnings.extend(resolver.take_warnings());
    let declarations = crate::multipart::audit::declarations(&task);
    let multipart = declarations
        .values()
        .filter_map(|state| match state {
            crate::multipart::audit::DeclState::Live(declaration) => {
                Some(crate::output::MultipartOut::from(declaration))
            }
            crate::multipart::audit::DeclState::Closed(_) => None,
        })
        .collect();
    let files = attached
        .into_iter()
        .map(|file| crate::output::FileInfo {
            member_of: declarations
                .keys()
                .find(|logical| {
                    crate::multipart::audit::is_member(&file.name, logical)
                        && !crate::multipart::audit::later_owned(&task, logical, &file.name)
                })
                .cloned(),
            name: file.name,
            path: file.path.display().to_string(),
            bytes: file.bytes,
        })
        .collect();
    let mut looked_up = HashMap::from([(project.prefix.clone(), project.color)]);
    let named = task
        .depends
        .iter()
        .chain(&task.related)
        .chain(&task.follows)
        .chain(&task.wake_on)
        .chain(&task.moved_to)
        .chain(&task.moved_from)
        .chain(source_id.as_ref());
    let prefixes = named
        .map(|id| registry.canonical_id(id).prefix)
        .chain(depending.iter().map(|task| task.id.prefix.clone()))
        .chain(following.iter().map(|task| task.id.prefix.clone()));
    for prefix in prefixes {
        looked_up
            .entry(prefix.clone())
            .or_insert_with(|| crate::resolve::color_or_warning(registry, &prefix, warnings));
    }
    let move_color = |id: &TaskId| looked_up[&registry.canonical_id(id).prefix];
    let moved_to_color = task.moved_to.as_ref().and_then(move_color);
    let moved_from_color = task.moved_from.as_ref().and_then(move_color);
    let project_colors: crate::output::ProjectColors = looked_up
        .into_iter()
        .filter_map(|(prefix, color)| color.map(|color| (prefix, color)))
        .collect();
    Ok(ShowFields {
        needs_review: review_reason.is_some(),
        review_reason,
        project_colors,
        moved_to_color,
        moved_from_color,
        spec_path,
        plan_path,
        step_found,
        depends_on,
        related_to,
        follows_tasks,
        dependents,
        followed_by,
        wake_targets,
        source_task,
        parent,
        unresolved_parent,
        in_lane: crate::hierarchy::lane_of(all, &task, registry),
        children,
        claim: claims
            .and_then(|snapshot| snapshot.get(&task.id))
            .map(|(claim, live)| crate::output::ClaimInfo::of(claim, live)),
        park: claims
            .and_then(|snapshot| snapshot.park(&task.id))
            .map(crate::output::ParkInfo::of),
        escalation: claims
            .and_then(|snapshot| snapshot.escalation(&task.id))
            .cloned(),
        periodic: crate::output::PeriodicInfo::of(&task, now),
        deferred: crate::output::DeferredInfo::of(&task, now),
        files,
        multipart,
        inventory,
        task,
    })
}

fn resolve_row(
    registry: &Registry,
    all: &[Task],
    resolver: &Resolver<'_>,
    id: &crate::model::TaskId,
) -> Result<DepInfo> {
    let canonical = registry.canonical_id(id);
    let resolved = match all.iter().find(|candidate| candidate.id == canonical) {
        Some(found) => Some(found.clone()),
        None => resolver.resolve_canonical_task(&canonical)?,
    };
    let canonical_id = canonical.to_string();
    match resolved {
        Some(task) => Ok(DepInfo {
            id: id.to_string(),
            canonical_id,
            title: Some(task.title),
            status: Some(task.status),
            updated: Some(task.updated),
            resolved: true,
            piece: false,
        }),
        None => Ok(DepInfo {
            id: id.to_string(),
            canonical_id,
            title: None,
            status: None,
            updated: None,
            resolved: false,
            piece: false,
        }),
    }
}
