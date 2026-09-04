use super::Ctx;
use crate::error::Result;
use crate::model::{Task, TaskId};
use crate::output::{DepInfo, Output, Related, ShowFields, ShowOut};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::Resolver;
use crate::scope::Origin;

pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let foreign;
    let project: &Project = if id.prefix == ctx.project.prefix {
        &ctx.project
    } else {
        foreign = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
        &foreign
    };
    let task = project.read_task(&id)?;
    let all = project.scan()?;
    let fields = describe(project, &ctx.registry, task, &all, &mut ctx.warnings)?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
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
    warnings: &mut Vec<String>,
) -> Result<ShowFields> {
    let resolver = Resolver::new(project, registry);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        // The scan the caller already holds answers first, so `next` describes the same
        // snapshot it chose from; only ids outside it touch the filesystem.
        let resolved = match all.iter().find(|candidate| &candidate.id == dependency) {
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
        Some(id) => match all.iter().find(|candidate| &candidate.id == id) {
            Some(found) => Some(related(found)),
            None => {
                warnings.push(format!("parent {id} not found"));
                None
            }
        },
        None => None,
    };
    let mut kids = crate::hierarchy::children(all, &task.id);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
    Ok(ShowFields {
        spec_path: task.spec.as_deref().map(|path| resolver.abs(path)),
        plan_path: task.plan.as_deref().map(|path| resolver.abs(path)),
        step_found,
        depends_on,
        parent,
        children,
        task,
    })
}
