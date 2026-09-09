//! Parked read views and `next` candidates.

use super::ReadCtx;
use crate::claims::{ClaimSnapshot, Park, WaitingOn};
use crate::error::Result;
use crate::model::{Phase, Status, Task, TaskId};
use crate::output::{ParkedRow, TaskSummary};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::Path;

pub fn rows(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot) -> Result<Vec<ParkedRow>> {
    let mut entries: Vec<(&String, &Park)> = claims.parks().collect();
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
        if let Some(task) = all.iter().find(|task| task.id == id) {
            rows.push(ParkedRow::resolved(
                TaskSummary::of(task, all, Some(claims), &ctx.registry),
                Phase::of(task),
            ));
            continue;
        }
        match resolve_elsewhere(&id, park, claims, &ctx.registry) {
            Ok(Some(row)) => {
                ctx.warnings.push(format!(
                    "{id} is parked in {}; resume it from that checkout",
                    park.worktree
                ));
                rows.push(row);
            }
            Ok(None) => {
                ctx.warnings.push(format!(
                    "{id} is parked in {}, which is unavailable; the row shows the park entry only",
                    park.worktree
                ));
                rows.push(ParkedRow::unresolved(key, park));
            }
            Err(error) => {
                ctx.warnings.push(format!("{id} is parked in {}, which is unavailable ({error}); the row shows the park entry only", park.worktree));
                rows.push(ParkedRow::unresolved(key, park));
            }
        }
    }
    Ok(rows)
}

fn resolve_elsewhere(
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    registry: &Registry,
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
        TaskSummary::of(task, &scan, Some(claims), registry),
        Phase::of(task),
    )))
}

pub fn candidates(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot) -> Result<Vec<Task>> {
    let mut found = Vec::new();
    for task in all {
        let Some(park) = claims.park(&task.id) else {
            continue;
        };
        if park.waiting_on != WaitingOn::Agent
            || !task.status.is_open()
            || task.status == Status::Blocked
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
            found.push((park.at.clone(), task.clone()));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(found.into_iter().map(|(_, task)| task).collect())
}
