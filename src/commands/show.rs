use super::Ctx;
use crate::error::{Error, Result};
use crate::model::Task;
use crate::output::{DepInfo, Output, Related, ShowFields, ShowOut};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, DocRead, Fetch, Located, Resolver, UnavailableCause, has_heading};
use std::collections::HashMap;
use std::path::Path;
use time::OffsetDateTime;

pub fn run(mut ctx: Ctx, id: String, fetch: Fetch) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claims = crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, all) = read_record(&mut ctx, &id, &claims)?;
    let project = recorded.as_ref().unwrap_or(&ctx.project);
    let now = crate::time::parse(&crate::time::now())?;
    let fields = describe(
        project,
        &ctx.registry,
        task,
        &all,
        Some(&claims),
        &mut ctx.warnings,
        now,
        fetch,
    )?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
}

pub(super) fn read_record(
    ctx: &mut Ctx,
    id: &crate::model::TaskId,
    claims: &crate::claims::ClaimSnapshot,
) -> Result<(Option<Project>, Task, Vec<Task>)> {
    let found = match ctx.project.read_task(id) {
        Ok(task) => (None, task, ctx.project.scan()?),
        Err(Error::TaskNotFound(_)) => {
            let (worktree, project, task, all) = recorded_elsewhere(id, claims)?;
            ctx.warnings.push(format!(
                "{id} exists only in {worktree}; shown from that checkout"
            ));
            (Some(project), task, all)
        }
        Err(error) => return Err(error),
    };
    match crate::moving::journal::refuse_pending_id(&ctx.registry.canonical_id(&found.1.id)) {
        Err(error @ Error::MovePending(_)) => ctx.warnings.push(error.to_string()),
        result => result?,
    }
    Ok(found)
}

/// Record-home spec §6.1: a record this checkout lacks is read from the checkout named by
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
    let unavailable = |why: String| {
        Error::TaskNotFound(id.to_string()).with_suffix(&format!(
            " ({how} in {worktree}, which is unavailable{why})"
        ))
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
    task: Task,
    all: &[Task],
    claims: Option<&crate::claims::ClaimSnapshot>,
    warnings: &mut Vec<String>,
    now: OffsetDateTime,
    fetch: Fetch,
) -> Result<ShowFields> {
    let mut resolver = Resolver::new(project, registry);
    resolver.set_fetch(fetch);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        // The scan the caller already holds answers first, so `next` describes the same
        // snapshot it chose from; only ids outside it touch the filesystem.
        let dependency_id = registry.canonical_id(dependency);
        let resolved = match all.iter().find(|candidate| candidate.id == dependency_id) {
            Some(found) => Some(found.clone()),
            None => resolver.resolve_task(dependency)?,
        };
        match resolved {
            Some(task) => depends_on.push(DepInfo {
                id: dependency.to_string(),
                title: Some(task.title),
                status: Some(task.status),
                resolved: true,
            }),
            None => {
                warnings.push(format!("dependency {dependency} is unreachable"));
                depends_on.push(DepInfo {
                    id: dependency.to_string(),
                    title: None,
                    status: None,
                    resolved: false,
                });
            }
        }
    }
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
            Located::Here(_) | Located::Main { .. } => Some(resolver.step_exists(plan, step)?),
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
    };
    let parent = match &task.parent {
        Some(id) => match all
            .iter()
            .find(|candidate| candidate.id == registry.canonical_id(id))
        {
            Some(found) => Some(related(found)),
            None => {
                warnings.push(format!("parent {id} not found"));
                None
            }
        },
        None => None,
    };
    let mut kids = crate::hierarchy::children(all, &task.id, registry);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
    let (attached, problems) = crate::attachments::audit_task(project, &task)?;
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
    let files = attached
        .into_iter()
        .map(|file| crate::output::FileInfo {
            name: file.name,
            path: file.path.display().to_string(),
            bytes: file.bytes,
        })
        .collect();
    // Keyed by the prefix as stored, which is how the view prints the id; looked up by
    // the live prefix a retired one resolves to, once per prefix.
    let mut looked_up = HashMap::from([(project.prefix.clone(), project.color)]);
    let named = task
        .depends
        .iter()
        .chain(&task.moved_to)
        .chain(&task.moved_from);
    for id in named {
        if !looked_up.contains_key(&id.prefix) {
            let live = registry.canonical_id(id).prefix;
            let color = if live == project.prefix {
                project.color
            } else {
                crate::resolve::color_or_warning(registry, &live, warnings)
            };
            looked_up.insert(id.prefix.clone(), color);
        }
    }
    let project_colors: crate::output::ProjectColors = looked_up
        .into_iter()
        .filter_map(|(prefix, color)| color.map(|color| (prefix, color)))
        .collect();
    Ok(ShowFields {
        project_colors,
        spec_path,
        plan_path,
        step_found,
        depends_on,
        parent,
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
        inventory,
        task,
    })
}
