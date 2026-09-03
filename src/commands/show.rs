use super::{Ctx, load};
use crate::error::Result;
use crate::output::{DepInfo, Output, Related, ShowOut};
use crate::resolve::Resolver;

pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let task = load(&ctx, &id)?;
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        match resolver.resolve_task(dependency)? {
            Some(task) => depends_on.push(DepInfo {
                id: dependency.to_string(),
                title: Some(task.title),
                status: Some(task.status.as_str().into()),
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
    let project = &ctx.project; // the feedback plan's Task 2 replaces this binding
    let all = project.scan()?;
    let related = |task: &crate::model::Task| Related {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: task.status.as_str().into(),
    };
    let parent = task
        .parent
        .as_ref()
        .and_then(|id| all.iter().find(|candidate| &candidate.id == id))
        .map(related);
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
