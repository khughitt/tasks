use super::{Ctx, load};
use crate::error::Result;
use crate::output::{DepInfo, Output, ShowOut};
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
    Ok(Output::Show(Box::new(ShowOut {
        spec_path: task.spec.as_deref().map(|path| resolver.abs(path)),
        plan_path: task.plan.as_deref().map(|path| resolver.abs(path)),
        step_found,
        depends_on,
        task,
        warnings: ctx.warnings,
    })))
}
