use super::Ctx;
use crate::error::{Error, Result};
use crate::model::Task;
use crate::output::{DepInfo, Output, Related, ShowFields, ShowOut};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, Resolver};
use std::path::Path;
use time::OffsetDateTime;

pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &id)?;
    let claims = crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, all) = match ctx.project.read_task(&id) {
        Ok(task) => (None, task, ctx.project.scan()?),
        Err(Error::TaskNotFound(_)) => {
            let (project, task, all) = recorded_elsewhere(&id, &claims, &mut ctx.warnings)?;
            (Some(project), task, all)
        }
        Err(error) => return Err(error),
    };
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
    )?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
}

/// Record-home spec §6.1: a record this checkout lacks is read from the checkout named by
/// its live claim, or else by its park. A record present here is always read here.
fn recorded_elsewhere(
    id: &crate::model::TaskId,
    claims: &crate::claims::ClaimSnapshot,
    warnings: &mut Vec<String>,
) -> Result<(Project, Task, Vec<Task>)> {
    let named = claims
        .live(id)
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
        Ok(Some(found)) => {
            warnings.push(format!(
                "{id} exists only in {worktree}; shown from that checkout"
            ));
            Ok(found)
        }
        Ok(None) => Err(unavailable(String::new())),
        Err(error) => Err(unavailable(format!(": {error}"))),
    }
}

/// The `show` view of `task`, which lives in `project`. `all` is a scan containing that
/// project's tasks (a union is fine: dependencies, parent, and children are looked up
/// by id, and ids carry their prefix). Unreachable dependencies and a missing parent are
/// pushed to `warnings`, never errors.
pub fn describe(
    project: &Project,
    registry: &Registry,
    task: Task,
    all: &[Task],
    claims: Option<&crate::claims::ClaimSnapshot>,
    warnings: &mut Vec<String>,
    now: OffsetDateTime,
) -> Result<ShowFields> {
    let resolver = Resolver::new(project, registry);
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
        (Some(plan), Some(step)) => Some(resolver.step_exists(plan, step)?),
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
    warnings.extend(resolver.take_warnings());
    let files = attached
        .into_iter()
        .map(|file| crate::output::FileInfo {
            name: file.name,
            path: file.path.display().to_string(),
            bytes: file.bytes,
        })
        .collect();
    Ok(ShowFields {
        spec_path,
        plan_path,
        step_found,
        depends_on,
        parent,
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
        task,
    })
}
