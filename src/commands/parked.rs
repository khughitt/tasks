//! Parked read views and `next` candidates.

use super::ReadCtx;
use super::actions::{Actions, Observation};
use crate::claims::{ClaimSnapshot, Park, Reason, WaitingOn};
use crate::error::Result;
use crate::filter::{Fields, TaskFilter};
use crate::model::{Phase, Status, Task, TaskId};
use crate::output::{ParkedRow, TaskSummary};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::Path;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefer {
    Registered,
    Recorded,
}

pub fn rows(
    ctx: &mut ReadCtx,
    all: &[Task],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
) -> Result<Vec<ParkedRow>> {
    Ok(sourced_rows(
        ctx,
        all,
        hierarchy,
        claims,
        now,
        Prefer::Registered,
        None,
        None,
        false,
    )?
    .into_iter()
    .map(|(row, _)| row)
    .collect())
}

pub fn rows_preferring(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    prefer: Prefer,
    reason: Option<Reason>,
) -> Result<Vec<ParkedRow>> {
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(all, &prefixes, &ctx.registry);
    Ok(sourced_rows(
        ctx, all, &hierarchy, claims, now, prefer, reason, None, false,
    )?
    .into_iter()
    .map(|(row, _)| row)
    .collect())
}

/// `rows`, each with the record its row was built from: the registered copy or the one in
/// the checkout the park recorded, whichever the row shows. `None` for an unresolved park.
/// `waiting_on` keeps only parks waiting on that party, unresolved ones included.
pub fn rows_with_records(
    ctx: &mut ReadCtx,
    all: &[Task],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    waiting_on: Option<WaitingOn>,
    actions: bool,
) -> Result<Vec<(ParkedRow, Option<Task>)>> {
    sourced_rows(
        ctx,
        all,
        hierarchy,
        claims,
        now,
        Prefer::Registered,
        None,
        waiting_on,
        actions,
    )
}

#[allow(clippy::too_many_arguments)]
fn sourced_rows(
    ctx: &mut ReadCtx,
    all: &[Task],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    prefer: Prefer,
    reason: Option<Reason>,
    waiting_on: Option<WaitingOn>,
    actions: bool,
) -> Result<Vec<(ParkedRow, Option<Task>)>> {
    // Selected from the park entry, before any row is built, so a park left out never
    // reads another checkout or warns.
    let mut entries: Vec<(&String, &Park)> = claims
        .parks()
        .filter(|(_, park)| reason.is_none_or(|reason| park.reason == Some(reason)))
        .filter(|(_, park)| waiting_on.is_none_or(|who| park.waiting_on == who))
        .collect();
    entries.sort_by(|a, b| b.1.at.cmp(&a.1.at).then_with(|| a.0.cmp(b.0)));
    // The registered rows' view; a row read from its recorded checkout builds that
    // checkout's own in `resolve_recorded` (docs/user-guide.md).
    let deps = super::list::review_deps(ctx, all)?;
    // The optional projection borrows the original scope while recorded rows add warnings.
    let registered_inputs = actions.then(|| (ctx.registry.clone(), ctx.scope.projects().to_vec()));
    let host = ctx.host.clone();
    let mut projection = registered_inputs.as_ref().map(|(registry, projects)| {
        Actions::new(
            Observation {
                registry,
                host: &host,
                projects,
                scan: all,
                claims,
                now,
            },
            hierarchy,
        )
    });
    let mut rows = Vec::new();
    for (key, park) in entries {
        let id = match TaskId::parse(key) {
            Ok(id) => ctx.registry.canonical_id(&id),
            Err(error) => {
                ctx.warnings.push(format!(
                    "park entry {key:?} is not a task id ({error}); shown as is"
                ));
                rows.push((ParkedRow::unresolved(key, park), None));
                continue;
            }
        };
        let registered = hierarchy.get(&id).map(|task| {
            let mut row = resolved_row(task, hierarchy, &deps, claims, &ctx.registry, now);
            if let Some(projection) = &mut projection {
                row.actions = Some(projection.explain(task, row.review_reason));
            }
            (row, task.clone())
        });
        let recorded_is_scanned = ctx
            .scope
            .projects()
            .iter()
            .any(|project| project.resume_root() == Path::new(&park.worktree));
        let row = match (prefer, registered) {
            (_, Some(row)) if recorded_is_scanned => Some(row),
            (Prefer::Registered, Some(row)) => Some(row),
            (Prefer::Registered, None) => {
                recorded_or_fallback(ctx, &id, park, claims, now, None, actions)
            }
            (Prefer::Recorded, registered) => {
                recorded_or_fallback(ctx, &id, park, claims, now, registered, actions)
            }
        };
        rows.push(match row {
            Some((row, task)) => (row, Some(task)),
            None => (ParkedRow::unresolved(key, park), None),
        });
    }
    Ok(rows)
}

fn resolved_row(
    task: &Task,
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    deps: &crate::review::Deps,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> ParkedRow {
    ParkedRow::resolved(
        TaskSummary::of(task, hierarchy, deps, Some(claims), registry, now),
        Phase::of(task),
    )
}

fn recorded_or_fallback(
    ctx: &mut ReadCtx,
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    fallback: Option<(ParkedRow, Task)>,
    actions: bool,
) -> Option<(ParkedRow, Task)> {
    match resolve_recorded(ctx, id, park, claims, now, actions) {
        Ok(Some(row)) => {
            ctx.warnings.push(format!(
                "{id} is parked in {}; resume it from that checkout",
                park.worktree
            ));
            Some(row)
        }
        Ok(None) => {
            ctx.warnings
                .push(unavailable(id, park, None, fallback.is_some()));
            fallback
        }
        Err(error) => {
            ctx.warnings
                .push(unavailable(id, park, Some(&error), fallback.is_some()));
            fallback
        }
    }
}

fn unavailable(
    id: &TaskId,
    park: &Park,
    error: Option<&crate::error::Error>,
    fell_back: bool,
) -> String {
    let cause = error.map(|error| format!(" ({error})")).unwrap_or_default();
    let shown = if fell_back {
        "showing the registered copy"
    } else {
        "the row shows the park entry only"
    };
    format!(
        "{id} is parked in {}, which is unavailable{cause}; {shown}",
        park.worktree
    )
}

/// The row from the checkout the park recorded, judged with that checkout's own scan and
/// dependency view (docs/user-guide.md).
fn resolve_recorded(
    ctx: &mut ReadCtx,
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    actions: bool,
) -> Result<Option<(ParkedRow, Task)>> {
    let Some((project, task, scan)) = open_recorded(id, Path::new(&park.worktree))? else {
        return Ok(None);
    };
    let deps = super::list::recorded_review_deps(ctx, &project, &scan)?;
    let scan_hierarchy =
        crate::hierarchy::view_of(&scan, std::slice::from_ref(&project.prefix), &ctx.registry);
    let mut summary = TaskSummary::of(
        &task,
        &scan_hierarchy,
        &deps,
        Some(claims),
        &ctx.registry,
        now,
    );
    summary.activity =
        crate::activity::ActivityIndex::of_scan(&scan, &ctx.registry, now).get(&task.id);
    if actions {
        let mut projection = Actions::new(
            Observation {
                registry: &ctx.registry,
                host: &ctx.host,
                projects: std::slice::from_ref(&project),
                scan: &scan,
                claims,
                now,
            },
            &scan_hierarchy,
        );
        summary.actions = Some(projection.explain(&task, summary.review_reason));
    }
    let row = ParkedRow::resolved(summary, Phase::of(&task));
    Ok(Some((row, task)))
}

/// Opens the checkout a store entry recorded and reads `id` there, with that checkout's
/// whole scan so the task's children, dependencies, and documents resolve there too (park
/// docs/ownership.md). A recorded code worktree of a linked or detached project holds no config;
/// its project is the one its repository is paired or linked to (ops linked-checkouts
/// docs/project-operations.md). `None` when
/// the checkout is gone, belongs to another prefix, or lacks the record.
pub fn open_recorded(id: &TaskId, worktree: &Path) -> Result<Option<(Project, Task, Vec<Task>)>> {
    let project = if crate::scope::has_config(worktree)? {
        Project::open(worktree)?
    } else {
        if !worktree.try_exists()? {
            return Ok(None);
        }
        let Some(tree) = crate::link::GitTree::of(worktree)? else {
            return Ok(None);
        };
        match crate::link::project_of_tree(&Registry::load()?, &tree)? {
            Some(project) => project,
            None => return Ok(None),
        }
    };
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let mixed = project
        .scan_mixed()
        .map_err(|error| error.in_project(&project.prefix))?;
    if let Some(future) = mixed.unsupported.iter().find(|future| future.id == *id) {
        return Err(future.refusal());
    }
    let Some(task) = mixed.tasks.iter().find(|task| task.id == *id).cloned() else {
        return Ok(None);
    };
    Ok(Some((project, task, mixed.tasks)))
}

pub(super) fn resumable_lifecycle(task: &Task) -> bool {
    task.status.is_open() && !matches!(task.status, Status::Blocked | Status::Shelved)
}

/// `deps` is `all`'s dependency view (docs/user-guide.md).
pub fn candidates(
    ctx: &mut ReadCtx,
    all: &[Task],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    deps: &crate::review::Deps,
    claims: &ClaimSnapshot,
    filter: &TaskFilter,
    now: time::OffsetDateTime,
) -> Result<crate::query::Picked> {
    let mut found = Vec::new();
    let mut deferred = Vec::new();
    let mut paused = Vec::new();
    for task in all {
        if !filter.matches(&Fields::of_task(
            task,
            hierarchy,
            deps,
            claims,
            &ctx.registry,
            now,
        )) {
            continue;
        }
        let Some(park) = claims.park(&task.id) else {
            continue;
        };
        let canonical = ctx.registry.canonical_id(&task.id);
        if park.waiting_on != WaitingOn::Agent
            || !resumable_lifecycle(task)
            || crate::decisions::holds_execution(task).is_some()
            || crate::hierarchy::is_goal(task, hierarchy.child_count(&canonical) > 0)
        {
            continue;
        }
        let mut held = false;
        for dependency in &task.depends {
            if !super::list::satisfied(ctx, all, dependency)? {
                held = true;
                break;
            }
        }
        if held {
            continue;
        }
        // Current contract (docs/user-guide.md): a candidate in a paused lane is set aside once it has passed
        // the dependency check; a deferred one is in neither the deferred nor the paused
        // count.
        let in_paused_lane = hierarchy.paused_lane(&canonical).is_some();
        match (crate::defer::is_deferred(task, now), in_paused_lane) {
            (true, true) => {}
            (true, false) => deferred.push(task.clone()),
            (false, true) => paused.push(task.clone()),
            (false, false) => found.push((park.at.clone(), task.clone())),
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(crate::query::Picked {
        tasks: found.into_iter().map(|(_, task)| task).collect(),
        deferred,
        paused,
    })
}
