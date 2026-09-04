use super::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::{DepInfo, Output, Related, ShowOut};
use crate::repo::{CONFIG_REL, Project};
use crate::resolve::Resolver;

pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let foreign;
    let project: &Project = if id.prefix == ctx.project.prefix {
        &ctx.project
    } else {
        let Some(root) = ctx.registry.project_root(&id.prefix) else {
            return Err(Error::UnresolvableId(id.to_string()));
        };
        if !root.join(CONFIG_REL).is_file() {
            return Err(Error::UnresolvableId(id.to_string()));
        }
        foreign = Project::open(root)?;
        if foreign.prefix != id.prefix {
            return Err(Error::Config(format!(
                "registry maps {:?} to {}, whose prefix is {:?}; fix the registry",
                id.prefix,
                root.display(),
                foreign.prefix
            )));
        }
        &foreign
    };
    let task = project.read_task(&id)?;
    let resolver = Resolver::new(project, &ctx.registry);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        match resolver.resolve_task(dependency)? {
            Some(task) => depends_on.push(DepInfo {
                id: dependency.to_string(),
                title: Some(task.title),
                status: Some(task.status),
                resolved: true,
            }),
            None => {
                ctx.warnings
                    .push(format!("dependency {dependency} is unreachable"));
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
    let all = project.scan()?;
    let related = |task: &crate::model::Task| Related {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: task.status,
    };
    let parent = match &task.parent {
        Some(id) => match all.iter().find(|candidate| &candidate.id == id) {
            Some(found) => Some(related(found)),
            None => {
                ctx.warnings.push(format!("parent {id} not found"));
                None
            }
        },
        None => None,
    };
    let mut kids = crate::hierarchy::children(&all, &task.id);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
    Ok(Output::Show(Box::new(ShowOut {
        spec_path: task.spec.as_deref().map(|path| resolver.abs(path)),
        plan_path: task.plan.as_deref().map(|path| resolver.abs(path)),
        step_found,
        depends_on,
        parent,
        children,
        task,
        warnings: ctx.warnings,
    })))
}
