use super::Ctx;
use crate::error::{Error, Result};
use crate::model::{Size, Status, Task, TaskId};
use crate::output::{Counts, ListOut, Output, PrimeOut, TaskSummary};
use crate::query::{is_ready, sort_list, sort_ready};
use crate::repo::{CONFIG_REL, Project};
use crate::resolve::Resolver;
use std::collections::HashMap;

pub fn list(
    mut ctx: Ctx,
    statuses: Vec<String>,
    tags: Vec<String>,
    owner: Option<String>,
    parent: Option<String>,
    all_projects: bool,
) -> Result<Output> {
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let mut tasks = ctx.project.scan()?;
    if all_projects {
        for (prefix, root) in &ctx.registry.projects {
            if *prefix == ctx.project.prefix {
                continue;
            }
            if !root.try_exists()? || !root.join(CONFIG_REL).try_exists()? {
                ctx.warnings.push(format!(
                    "project {prefix} at {} is unreachable",
                    root.display()
                ));
                continue;
            }
            tasks.extend(Project::open(root)?.scan()?);
        }
    }
    let all = tasks.clone();
    let parent = parent.as_deref().map(TaskId::parse).transpose()?;
    if let Some(parent) = &parent
        && !all.iter().any(|task| &task.id == parent)
    {
        return Err(Error::TaskNotFound(parent.to_string()));
    }
    tasks.retain(|task| {
        let status_ok = if statuses.is_empty() {
            task.status.is_open()
        } else {
            statuses.contains(&task.status)
        };
        let tags_ok = tags.iter().all(|tag| task.tags.contains(tag));
        let owner_ok = owner
            .as_ref()
            .is_none_or(|value| task.owner.as_ref() == Some(value));
        let parent_ok = parent
            .as_ref()
            .is_none_or(|p| task.parent.as_ref() == Some(p));
        status_ok && tags_ok && owner_ok && parent_ok
    });
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    for task in &tasks {
        for dependency in &task.depends {
            if resolver.resolve_task(dependency)?.is_none() {
                ctx.warnings.push(format!(
                    "{}: dependency {dependency} is unreachable",
                    task.id
                ));
            }
        }
    }
    sort_list(&mut tasks);
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all))
            .collect(),
        warnings: ctx.warnings,
    }))
}

/// Ready tasks in ready order; pushes a warning per unreachable dependency.
pub fn ready_tasks(ctx: &mut Ctx, all: &[Task]) -> Result<Vec<Task>> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let mut warnings = Vec::new();
    let mut closed: HashMap<TaskId, Option<bool>> = HashMap::new();
    for task in all.iter().filter(|task| task.status == Status::Todo) {
        for dependency in &task.depends {
            if closed.contains_key(dependency) {
                continue;
            }
            let value = match all.iter().find(|task| &task.id == dependency) {
                Some(local) => Some(!local.status.is_open()),
                None => resolver
                    .resolve_task(dependency)?
                    .map(|task| !task.status.is_open()),
            };
            closed.insert(dependency.clone(), value);
        }
    }
    let lookup = |id: &TaskId| -> Option<bool> { closed.get(id).copied().flatten() };
    let mut ready = Vec::new();
    for task in all {
        if task.status != Status::Todo {
            continue;
        }
        for dependency in &task.depends {
            if lookup(dependency).is_none() {
                warnings.push(format!(
                    "{}: dependency {dependency} is unreachable",
                    task.id
                ));
            }
        }
        let has_children = !crate::hierarchy::children(all, &task.id).is_empty();
        if is_ready(task, has_children, &lookup) {
            ready.push(task.clone());
        }
    }
    sort_ready(&mut ready);
    ctx.warnings.extend(warnings);
    Ok(ready)
}

pub fn ready(mut ctx: Ctx, size: Option<String>, limit: Option<usize>) -> Result<Output> {
    let size = size.map(|size| Size::parse(&size)).transpose()?;
    let all = ctx.project.scan()?;
    let mut tasks = ready_tasks(&mut ctx, &all)?;
    if let Some(size) = size {
        tasks.retain(|task| task.size == Some(size));
    }
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all))
            .collect(),
        warnings: ctx.warnings,
    }))
}

pub fn prime(mut ctx: Ctx) -> Result<Output> {
    let all = ctx.project.scan()?;
    let mut counts = Counts::default();
    for task in &all {
        match task.status {
            Status::Idea => counts.idea += 1,
            Status::Todo => counts.todo += 1,
            Status::Doing => counts.doing += 1,
            Status::Blocked => counts.blocked += 1,
            Status::Done => counts.done += 1,
            Status::Dropped => counts.dropped += 1,
        }
    }
    let ready = ready_tasks(&mut ctx, &all)?;
    let mut doing: Vec<Task> = all
        .iter()
        .filter(|task| task.status == Status::Doing)
        .cloned()
        .collect();
    sort_list(&mut doing);
    Ok(Output::Prime(PrimeOut {
        prefix: ctx.project.prefix.clone(),
        counts,
        ready: ready
            .iter()
            .map(|task| TaskSummary::of(task, &all))
            .collect(),
        doing: doing
            .iter()
            .map(|task| TaskSummary::of(task, &all))
            .collect(),
        warnings: ctx.warnings,
    }))
}
