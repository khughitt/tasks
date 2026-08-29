use super::{Ctx, id_out, load, save};
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;
use crate::query::find_cycle;
use crate::resolve::Resolver;

/// Fails when `candidate` participates in a cycle or reaches an unknown task.
pub fn ensure_acyclic(ctx: &Ctx, candidate: &Task) -> Result<()> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let edges = |id: &TaskId| -> Result<Option<Vec<TaskId>>> {
        if *id == candidate.id {
            return Ok(Some(candidate.depends.clone()));
        }
        Ok(resolver.resolve_task(id)?.map(|task| task.depends))
    };
    if let Some(cycle) = find_cycle(&candidate.id, &edges)? {
        return Err(Error::Cycle(
            cycle
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" -> "),
        ));
    }
    Ok(())
}

pub fn run(ctx: Ctx, id: String, on: Vec<String>, rm: Vec<String>) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    if !on.is_empty() {
        let resolver = Resolver::new(&ctx.project, &ctx.registry);
        for value in &on {
            let dependency = TaskId::parse(value)?;
            if dependency == task.id {
                return Err(Error::Cycle(format!("{dependency} -> {dependency}")));
            }
            if resolver.resolve_task(&dependency)?.is_none() {
                return Err(Error::UnresolvableId(dependency.to_string()));
            }
            if !task.depends.contains(&dependency) {
                task.depends.push(dependency);
            }
        }
        ensure_acyclic(&ctx, &task)?;
    } else {
        for value in &rm {
            let dependency = TaskId::parse(value)?;
            let before = task.depends.len();
            task.depends.retain(|item| item != &dependency);
            if task.depends.len() == before {
                return Err(Error::Validation(format!(
                    "{} does not depend on {dependency}",
                    task.id
                )));
            }
        }
    }
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
