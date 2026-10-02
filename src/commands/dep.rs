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
        let id = ctx.registry.canonical_id(id);
        if id == candidate.id {
            return Ok(Some(
                candidate
                    .depends
                    .iter()
                    .map(|dependency| ctx.registry.canonical_id(dependency))
                    .collect(),
            ));
        }
        Ok(resolver.resolve_task(&id)?.map(|task| {
            task.depends
                .iter()
                .map(|dependency| ctx.registry.canonical_id(dependency))
                .collect()
        }))
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

/// Adds each of `values` to `task.depends` unless the task already depends on it under
/// any spelling, then checks the final graph. Returns a warning for each id the task
/// already held before the call; repeats within one call are deduplicated silently.
/// Shared by `dep --on` and by `--depends` on `add` and `edit`.
pub fn add_dependencies(
    ctx: &Ctx,
    resolver: &Resolver<'_>,
    task: &mut Task,
    values: &[String],
) -> Result<Vec<String>> {
    let existing = task.depends.len();
    let mut warnings = Vec::new();
    for value in values {
        let given = TaskId::parse_input(value)?;
        let dependency = ctx.registry.canonical_id(&given);
        if dependency == task.id {
            return Err(Error::Cycle(format!("{dependency} -> {dependency}")));
        }
        if resolver.resolve_task(&dependency)?.is_none() {
            return Err(Error::UnresolvableId(dependency.to_string()));
        }
        match task
            .depends
            .iter()
            .position(|item| ctx.registry.canonical_id(item) == dependency)
        {
            Some(index) if index < existing => {
                let warning = already_depends(&task.id, &dependency, &task.depends[index], &given);
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            Some(_) => {}
            None => task.depends.push(dependency),
        }
    }
    if !values.is_empty() {
        ensure_acyclic(ctx, task)?;
    }
    Ok(warnings)
}

/// Explains an `--on` that named a dependency the task already had. Alias spellings
/// name one task, so the add changed nothing; when the spellings differ, it says that
/// `--rm` with either removes that one edge, which an `--on`/`--rm` pair would.
fn already_depends(task: &TaskId, canonical: &TaskId, stored: &TaskId, given: &TaskId) -> String {
    if stored == given {
        return format!("{task} already depends on {canonical}; nothing changed");
    }
    let spellings = [("stored as", stored), ("given as", given)]
        .into_iter()
        .filter(|(_, spelling)| *spelling != canonical)
        .map(|(label, spelling)| format!("{label} {spelling}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{task} already depends on {canonical} ({spellings}); both spellings name one task, \
         so nothing changed and --rm with either spelling removes that one dependency: \
         --on then --rm does not rewrite the stored prefix"
    )
}

pub fn run(mut ctx: Ctx, id: String, on: Vec<String>, rm: Vec<String>) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    let additions = on
        .iter()
        .map(|value| super::parse_id(&ctx.registry, value))
        .collect::<Result<Vec<_>>>()?;
    let removals = rm
        .iter()
        .map(|value| super::parse_id(&ctx.registry, value))
        .collect::<Result<Vec<_>>>()?;
    if let Some(both) = additions
        .iter()
        .find(|dependency| removals.contains(dependency))
    {
        return Err(Error::Validation(format!(
            "{both} is named by both --on and --rm"
        )));
    }

    // Removals go first and never walk the graph, so a stored unreachable reference can
    // always be cleaned up.
    for dependency in &removals {
        let before = task.depends.len();
        task.depends
            .retain(|item| ctx.registry.canonical_id(item) != *dependency);
        if task.depends.len() == before {
            return Err(Error::Validation(format!(
                "{} does not depend on {dependency}",
                task.id
            )));
        }
    }
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let warnings = add_dependencies(&ctx, &resolver, &mut task, &on)?;
    ctx.warnings.extend(warnings);
    save(&mut ctx, &mut task)?;
    super::follow_holder(&mut ctx, &task.id, None, "the dependency change landed");
    Ok(id_out(ctx, &task))
}
