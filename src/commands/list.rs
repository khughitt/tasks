use super::ReadCtx;
use crate::error::{Error, Result};
use crate::model::{Size, Status, Task, TaskId};
use crate::output::{Counts, ListOut, NextOut, Output, PrimeOut, TaskSummary};
use crate::query::{is_ready, sort_list, sort_ready};
use crate::scope::Scope;
use std::collections::HashMap;

fn resolve_dependency(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<Task>> {
    match all.iter().find(|task| &task.id == id) {
        Some(task) => Ok(Some(task.clone())),
        None => ctx.resolve_task(id),
    }
}

pub fn list(
    mut ctx: ReadCtx,
    statuses: Vec<String>,
    tags: Vec<String>,
    owner: Option<String>,
    parent: Option<String>,
) -> Result<Output> {
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let mut tasks = ctx.scope.scan()?;
    let all = tasks.clone();
    let prefixes = ctx.scope.prefixes();
    let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
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
    for task in &tasks {
        for dependency in &task.depends {
            if resolve_dependency(&ctx, &all, dependency)?.is_none() {
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
            .map(|task| TaskSummary::of(task, &all, Some(&claims)))
            .collect(),
        warnings: ctx.warnings,
    }))
}

/// Ready tasks in ready order; pushes a warning per unreachable dependency.
pub fn ready_tasks(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &crate::claims::ClaimSnapshot,
) -> Result<Vec<Task>> {
    let mut warnings = Vec::new();
    let mut closed: HashMap<TaskId, Option<bool>> = HashMap::new();
    for task in all.iter().filter(|task| task.status == Status::Todo) {
        for dependency in &task.depends {
            if closed.contains_key(dependency) {
                continue;
            }
            let value =
                resolve_dependency(ctx, all, dependency)?.map(|task| !task.status.is_open());
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
    ready.retain(|task| match claims.live(&task.id) {
        Some(claim) => {
            warnings.push(format!(
                "{} omitted: claimed by session {} in {} — `tasks start --force {}` to take it over",
                task.id, claim.session, claim.worktree, task.id
            ));
            false
        }
        None => true,
    });
    ctx.warnings.extend(warnings);
    Ok(ready)
}

pub fn ready(mut ctx: ReadCtx, size: Option<String>, limit: Option<usize>) -> Result<Output> {
    let size = size.map(|size| Size::parse(&size)).transpose()?;
    let all = ctx.scope.scan()?;
    let prefixes = ctx.scope.prefixes();
    let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
    let mut tasks = ready_tasks(&mut ctx, &all, &claims)?;
    if let Some(size) = size {
        tasks.retain(|task| task.size == Some(size));
    }
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims)))
            .collect(),
        warnings: ctx.warnings,
    }))
}

/// The head of `ready` in the show shape, so a caller can start on it without a second
/// lookup. Nothing ready is a normal state: null, warnings, exit 0.
pub fn next(mut ctx: ReadCtx) -> Result<Output> {
    let all = ctx.scope.scan()?;
    let prefixes = ctx.scope.prefixes();
    let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
    let ready = ready_tasks(&mut ctx, &all, &claims)?;
    let next = match ready.into_iter().next() {
        None => None,
        Some(task) => {
            let project = ctx
                .scope
                .projects()
                .iter()
                .find(|project| project.prefix == task.id.prefix)
                .expect("a ready task was scanned from a project in scope");
            let mut warnings = Vec::new();
            let fields = super::show::describe(
                project,
                &ctx.registry,
                task,
                &all,
                Some(&claims),
                &mut warnings,
            )?;
            ctx.warnings.extend(warnings);
            Some(fields)
        }
    };
    Ok(Output::Next(Box::new(NextOut {
        next,
        warnings: ctx.warnings,
    })))
}

pub fn prime(mut ctx: ReadCtx) -> Result<Output> {
    let all = ctx.scope.scan()?;
    let prefixes = ctx.scope.prefixes();
    let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
    let counts = Counts::of(&all);
    let ready = ready_tasks(&mut ctx, &all, &claims)?;
    let mut doing: Vec<Task> = all
        .iter()
        .filter(|task| task.status == Status::Doing || claims.live(&task.id).is_some())
        .cloned()
        .collect();
    sort_list(&mut doing);
    let roadmap = crate::hierarchy::forest(&all, None, false, Some(&claims));
    let mut closeout: Vec<Task> = all
        .iter()
        .filter(|task| {
            // spec §4.3: todo, doing, or blocked; an idea is open but not a candidate
            matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
                && !crate::hierarchy::children(&all, &task.id).is_empty()
                && crate::hierarchy::open_descendants(&all, &task.id).is_empty()
        })
        .cloned()
        .collect();
    sort_ready(&mut closeout);
    let wide = matches!(ctx.scope, Scope::All(_));
    for project in ctx.scope.projects() {
        if let Some(files) = project.uncommitted_task_files()?
            && !files.is_empty()
        {
            let message = format!("uncommitted task files: {}", files.join(", "));
            ctx.warnings.push(if wide {
                format!("{}: {message}", project.prefix)
            } else {
                message
            });
        }
    }
    for task in &all {
        if let Some(claim) = claims.live(&task.id)
            && matches!(task.status, Status::Todo | Status::Idea)
        {
            ctx.warnings.push(format!(
                "{} is claimed as doing in {} but this checkout's copy says {}; the two copies will conflict on merge",
                task.id,
                claim.worktree,
                task.status.as_str()
            ));
        }
    }
    for (id, claim, why) in claims.stale() {
        ctx.warnings.push(format!(
            "{id} has a stale claim from session {} ({why}); `tasks start --force {id}` to take it over",
            claim.session
        ));
    }
    Ok(Output::Prime(PrimeOut {
        prefix: match &ctx.scope {
            Scope::Local(project) => Some(project.prefix.clone()),
            Scope::All(_) => None,
        },
        projects: ctx.scope.prefixes(),
        counts,
        ready: ready
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims)))
            .collect(),
        doing: doing
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims)))
            .collect(),
        roadmap,
        closeout: closeout
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims)))
            .collect(),
        warnings: ctx.warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_resolution_prefers_the_captured_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "sci").unwrap();
        let dependency =
            crate::commands::add::blank(&project, "Dependency".into(), Status::Todo).unwrap();
        std::fs::write(project.root.join(crate::repo::CONFIG_REL), "not toml = [").unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.register("sci", &project.root).unwrap();
        let ctx = ReadCtx {
            scope: Scope::All(vec![]),
            registry,
            warnings: vec![],
        };

        assert_eq!(
            resolve_dependency(&ctx, std::slice::from_ref(&dependency), &dependency.id).unwrap(),
            Some(dependency)
        );
    }
}
