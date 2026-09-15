//! Parked read views and `next` candidates.

use super::ReadCtx;
use crate::claims::{ClaimSnapshot, Park, Reason, WaitingOn};
use crate::error::Result;
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
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
) -> Result<Vec<ParkedRow>> {
    rows_preferring(ctx, all, claims, now, Prefer::Registered, None)
}

pub fn rows_preferring(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    prefer: Prefer,
    reason: Option<Reason>,
) -> Result<Vec<ParkedRow>> {
    let mut entries: Vec<(&String, &Park)> = claims
        .parks()
        .filter(|(_, park)| reason.is_none_or(|reason| park.reason == Some(reason)))
        .collect();
    entries.sort_by(|a, b| b.1.at.cmp(&a.1.at).then_with(|| a.0.cmp(b.0)));
    let mut rows = Vec::new();
    for (key, park) in entries {
        let id = match TaskId::parse(key) {
            Ok(id) => ctx.registry.canonical_id(&id),
            Err(error) => {
                ctx.warnings.push(format!(
                    "park entry {key:?} is not a task id ({error}); shown as is"
                ));
                rows.push(ParkedRow::unresolved(key, park));
                continue;
            }
        };
        let registered = all
            .iter()
            .find(|task| task.id == id)
            .map(|task| resolved_row(task, all, claims, &ctx.registry, now));
        let recorded_is_scanned = ctx
            .scope
            .projects()
            .iter()
            .any(|project| project.root == Path::new(&park.worktree));
        let row = match (prefer, registered) {
            (_, Some(row)) if recorded_is_scanned => Some(row),
            (Prefer::Registered, Some(row)) => Some(row),
            (Prefer::Registered, None) => recorded_or_fallback(ctx, &id, park, claims, now, None),
            (Prefer::Recorded, registered) => {
                recorded_or_fallback(ctx, &id, park, claims, now, registered)
            }
        };
        rows.push(row.unwrap_or_else(|| ParkedRow::unresolved(key, park)));
    }
    Ok(rows)
}

fn resolved_row(
    task: &Task,
    all: &[Task],
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> ParkedRow {
    ParkedRow::resolved(
        TaskSummary::of(task, all, Some(claims), registry, now),
        Phase::of(task),
    )
}

fn recorded_or_fallback(
    ctx: &mut ReadCtx,
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    fallback: Option<ParkedRow>,
) -> Option<ParkedRow> {
    match resolve_recorded(id, park, claims, &ctx.registry, now) {
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

fn resolve_recorded(
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<ParkedRow>> {
    let root = Path::new(&park.worktree);
    if !crate::scope::has_config(root)? {
        return Ok(None);
    }
    let project = Project::open(root)?;
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let scan = project.scan()?;
    let Some(task) = scan.iter().find(|task| task.id == *id) else {
        return Ok(None);
    };
    Ok(Some(ParkedRow::resolved(
        TaskSummary::of(task, &scan, Some(claims), registry, now),
        Phase::of(task),
    )))
}

pub fn candidates(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: time::OffsetDateTime,
) -> Result<crate::query::Picked> {
    let mut found = Vec::new();
    let mut deferred = Vec::new();
    for task in all {
        let Some(park) = claims.park(&task.id) else {
            continue;
        };
        if park.waiting_on != WaitingOn::Agent
            || !task.status.is_open()
            || matches!(task.status, Status::Blocked | Status::Shelved)
            || !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty()
        {
            continue;
        }
        let mut held = false;
        for dependency in &task.depends {
            match super::list::resolve_dependency(ctx, all, dependency)? {
                Some(dependency) if !dependency.status.is_open() => {}
                _ => {
                    held = true;
                    break;
                }
            }
        }
        if !held {
            if crate::defer::is_deferred(task, now) {
                deferred.push(task.clone());
            } else {
                found.push((park.at.clone(), task.clone()));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(crate::query::Picked {
        tasks: found.into_iter().map(|(_, task)| task).collect(),
        deferred,
    })
}
