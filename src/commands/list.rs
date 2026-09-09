use super::ReadCtx;
use crate::error::{Error, Result};
use crate::model::{Size, Status, Task, TaskId};
use crate::output::{Counts, DateColumn, ListOut, NextOut, Output, PrimeOut, TaskSummary};
use crate::query::{SortKey, is_ready, sort_by_key, sort_list, sort_ready};
use crate::scope::Scope;
use std::collections::HashMap;

fn resolve_dependency(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<Task>> {
    let id = ctx.registry.canonical_id(id);
    match all.iter().find(|task| task.id == id) {
        Some(task) => Ok(Some(task.clone())),
        None => ctx.resolve_task(&id),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn list(
    mut ctx: ReadCtx,
    statuses: Vec<String>,
    tags: Vec<String>,
    owner: Option<String>,
    source: Option<String>,
    parent: Option<String>,
    sort: Option<String>,
    reverse: bool,
) -> Result<Output> {
    let sort = match sort {
        Some(key) => SortKey::parse(&key)?,
        None => SortKey::Priority,
    };
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let (all, claims) = ctx.scan_with_claims()?;
    let mut tasks = all.clone();
    let parent = parent
        .as_deref()
        .map(|id| super::parse_id(&ctx.registry, id))
        .transpose()?;
    if let Some(parent) = &parent
        && !all.iter().any(|task| task.id == *parent)
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
        let source_ok = source
            .as_ref()
            .is_none_or(|value| task.source.as_ref() == Some(value));
        let parent_ok = parent.as_ref().is_none_or(|p| {
            task.parent
                .as_ref()
                .is_some_and(|parent| ctx.registry.canonical_id(parent) == *p)
        });
        status_ok && tags_ok && owner_ok && source_ok && parent_ok
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
    sort_by_key(&mut tasks, sort);
    if reverse {
        tasks.reverse();
    }
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        warnings: ctx.warnings,
        date: sort.date_column(),
    }))
}

/// Why a live claim keeps a task out of a read command's rows. `ready` appends the
/// takeover hint; `sample` does not, since a curator never starts a task.
pub fn claim_omission(id: &TaskId, claim: &crate::claims::Claim) -> String {
    format!(
        "{id} omitted: claimed by session {} in {}",
        claim.session, claim.worktree
    )
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
        let has_children = !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty();
        if is_ready(task, has_children, &lookup) {
            ready.push(task.clone());
        }
    }
    sort_ready(&mut ready);
    ready.retain(|task| match claims.live(&task.id) {
        Some(claim) => {
            warnings.push(format!(
                "{} — `tasks start --force {}` to take it over",
                claim_omission(&task.id, claim),
                task.id
            ));
            false
        }
        None => true,
    });
    ctx.warnings.extend(warnings);
    Ok(ready)
}

pub fn ready(
    mut ctx: ReadCtx,
    size: Option<String>,
    parallel: bool,
    limit: Option<usize>,
) -> Result<Output> {
    let size = size.map(|size| Size::parse(&size)).transpose()?;
    let (all, claims) = ctx.scan_with_claims()?;
    let mut tasks = ready_tasks(&mut ctx, &all, &claims)?;
    if let Some(size) = size {
        tasks.retain(|task| task.size == Some(size));
    }
    if parallel {
        tasks.retain(|task| task.parallel);
    }
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    }))
}

/// The head of `ready` in the show shape, so a caller can start on it without a second
/// lookup. Nothing ready is a normal state: null, warnings, exit 0.
pub fn next(mut ctx: ReadCtx) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
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

pub fn prime(mut ctx: ReadCtx, closed: bool) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let counts = Counts::of(&all);
    let ready = ready_tasks(&mut ctx, &all, &claims)?;
    let mut doing: Vec<Task> = all
        .iter()
        .filter(|task| task.status == Status::Doing || claims.live(&task.id).is_some())
        .cloned()
        .collect();
    sort_list(&mut doing);
    let roadmap = crate::hierarchy::forest(&all, None, false, Some(&claims), &ctx.registry);
    // closeout is an invitation to run `done`, so it holds only what `done` will accept.
    // Children are one gate and dependencies are the other; listing a goal its dependencies
    // still hold would invite a close the tool then refuses with `open_dependencies`. Held
    // goals are named in a warning instead, so none of them goes silent.
    let mut closeout: Vec<Task> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    for task in &all {
        // spec §4.3: todo, doing, or blocked; an idea is open but not a candidate
        if !matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
            || crate::hierarchy::children(&all, &task.id, &ctx.registry).is_empty()
            || !crate::hierarchy::open_descendants(&all, &task.id, &ctx.registry).is_empty()
        {
            continue;
        }
        let mut open = Vec::new();
        for dependency in &task.depends {
            // An unreachable dependency counts as open, exactly as `done` treats it: what
            // we cannot resolve, we cannot call closed.
            match resolve_dependency(&ctx, &all, dependency)? {
                Some(dependency) if !dependency.status.is_open() => {}
                _ => open.push(dependency.to_string()),
            }
        }
        if open.is_empty() {
            closeout.push(task.clone());
        } else {
            held.push(format!(
                "{} has no open children but is held by open dependencies: {}; it joins \
                 closeout when they close",
                task.id,
                open.join(", ")
            ));
        }
    }
    sort_ready(&mut closeout);
    ctx.warnings.extend(held);
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
        closed,
        ready: ready
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        doing: doing
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        roadmap,
        closeout: closeout
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
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
