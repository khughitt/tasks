//! Who a drop or close releases, which shelves it wakes, and who depends on a carried task
//! (docs/project-operations.md "Dropped and closed tasks: who they release", `inbound`/`woken`). Read
//! across every reachable registered project, retired ones included, as `move`'s inbound
//! dependents are; a project that cannot be read is named, never guessed at.

use crate::error::Result;
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use crate::scope::Scope;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every record of every reachable registered project, the registered projects that could
/// not be read, and the warnings the walk produced.
pub struct Wide {
    pub tasks: Vec<Task>,
    pub unread_projects: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn read(registry: &Registry, cwd: &Path) -> Result<Wide> {
    let (scope, warnings) = Scope::open_all_with_retired(registry, cwd)?;
    let read: BTreeSet<&str> = scope
        .projects()
        .iter()
        .map(|project| project.prefix.as_str())
        .collect();
    let unread_projects = registry
        .projects
        .keys()
        .filter(|prefix| !read.contains(prefix.as_str()))
        .cloned()
        .collect();
    Ok(Wide {
        tasks: scope.scan()?,
        unread_projects,
        warnings,
    })
}

/// For each of `ids` with any, the records whose `pick` edges name it (canonically),
/// among the records `keep` admits; each list sorted and without repeats.
fn edges_into(
    scope_tasks: &[Task],
    registry: &Registry,
    ids: &BTreeSet<TaskId>,
    keep: impl Fn(&Task) -> bool,
    pick: impl Fn(&Task) -> &[TaskId],
) -> BTreeMap<TaskId, Vec<TaskId>> {
    let mut out: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
    for task in scope_tasks.iter().filter(|task| keep(task)) {
        for target in pick(task) {
            let target = registry.canonical_id(target);
            if ids.contains(&target) {
                out.entry(target)
                    .or_default()
                    .insert(registry.canonical_id(&task.id));
            }
        }
    }
    out.into_iter()
        .map(|(id, set)| (id, set.into_iter().collect()))
        .collect()
}

/// The open records that depend on each of `ids`: what closing it would let proceed.
pub fn open_dependents(
    scope_tasks: &[Task],
    registry: &Registry,
    ids: &BTreeSet<TaskId>,
) -> BTreeMap<TaskId, Vec<TaskId>> {
    edges_into(
        scope_tasks,
        registry,
        ids,
        |task| task.status.is_open(),
        |task| &task.depends,
    )
}

/// The shelves whose `wake_on` names each of `ids`: closing it prompts their review, never
/// their readiness.
pub fn woken_shelves(
    scope_tasks: &[Task],
    registry: &Registry,
    ids: &BTreeSet<TaskId>,
) -> BTreeMap<TaskId, Vec<TaskId>> {
    edges_into(
        scope_tasks,
        registry,
        ids,
        |task| task.status == Status::Shelved,
        |task| &task.wake_on,
    )
}

/// Every record, whatever its status, that depends on each of `ids`: `move`'s inbound.
pub fn dependents(
    scope_tasks: &[Task],
    registry: &Registry,
    ids: &BTreeSet<TaskId>,
) -> BTreeMap<TaskId, Vec<TaskId>> {
    edges_into(scope_tasks, registry, ids, |_| true, |task| &task.depends)
}
