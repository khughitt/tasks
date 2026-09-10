use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::{TaskSummary, TreeNode};
use crate::query::ready_order;
use crate::registry::Registry;
use crate::repo::Project;
use std::collections::HashMap;
use time::OffsetDateTime;

/// Rejects a `parent` that is foreign, missing, or would make `task` its own ancestor.
/// Reads ancestors from disk, so it is the write-path check; `check` uses `parent_cycle`.
pub fn validate_parent(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    let Some(stored_parent) = &task.parent else {
        return Ok(());
    };
    let parent = registry.canonical_id(stored_parent);
    let task_id = registry.canonical_id(&task.id);
    if parent.prefix != project.prefix {
        return Err(Error::Validation(format!(
            "parent {parent} must be in this project ({})",
            project.prefix
        )));
    }
    if parent == task_id {
        return Err(Error::Cycle(format!("{parent} -> {parent}")));
    }
    if !project.task_path(&parent).is_file() {
        return Err(Error::UnresolvableId(format!(
            "parent {parent} does not exist"
        )));
    }
    // spec §4.7: a recurrence is never a goal, so nothing may hang beneath one.
    if project.read_task(&parent)?.every.is_some() {
        return Err(Error::Validation(format!(
            "{parent} is a recurrence and cannot have children; clear its cadence with \
             `tasks edit {parent} --no-every` first"
        )));
    }
    let mut path = vec![task_id];
    let mut current = Some(parent);
    while let Some(id) = current {
        if path.contains(&id) {
            path.push(id);
            return Err(Error::Cycle(join(&path)));
        }
        let ancestor = match project.read_task(&id) {
            Ok(ancestor) => ancestor,
            Err(Error::TaskNotFound(_)) => break,
            Err(error) => return Err(error),
        };
        path.push(id);
        current = ancestor
            .parent
            .as_ref()
            .map(|parent| registry.canonical_id(parent));
    }
    Ok(())
}

/// Walks the parent chain upward from `start` and returns the loop it runs into, if any,
/// as `a -> b -> a`: only the cycle, never the tail that led into it. The path starts at
/// whichever member the walk enters first (`b -> a -> b` from `b`), so callers that
/// deduplicate must key on the set of members, as `check` does, not on the order. Missing
/// parents end the walk without a cycle.
pub fn parent_cycle(tasks: &[Task], start: &TaskId, registry: &Registry) -> Option<Vec<TaskId>> {
    let parents: HashMap<TaskId, TaskId> = tasks
        .iter()
        .filter_map(|task| {
            task.parent.as_ref().map(|parent| {
                (
                    registry.canonical_id(&task.id),
                    registry.canonical_id(parent),
                )
            })
        })
        .collect();
    let start = registry.canonical_id(start);
    let mut path = vec![start.clone()];
    let mut current = parents.get(&start).cloned();
    while let Some(id) = current {
        if let Some(position) = path.iter().position(|item| item == &id) {
            let mut cycle = path[position..].to_vec();
            cycle.push(id);
            return Some(cycle);
        }
        path.push(id.clone());
        current = parents.get(&id).cloned();
    }
    None
}

/// Direct children of `id`, in the order they appear in `tasks`.
pub fn children<'a>(tasks: &'a [Task], id: &TaskId, registry: &Registry) -> Vec<&'a Task> {
    let id = registry.canonical_id(id);
    tasks
        .iter()
        .filter(|task| {
            task.parent
                .as_ref()
                .is_some_and(|parent| registry.canonical_id(parent) == id)
        })
        .collect()
}

/// Every task below `id`, depth first. A visited set makes a corrupt loop terminate.
pub fn descendants<'a>(tasks: &'a [Task], id: &TaskId, registry: &Registry) -> Vec<&'a Task> {
    let mut out = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<&TaskId> = vec![id];
    while let Some(current) = stack.pop() {
        for child in children(tasks, current, registry) {
            if visited.insert(&child.id) {
                out.push(child);
                stack.push(&child.id);
            }
        }
    }
    out
}

pub fn open_descendants<'a>(tasks: &'a [Task], id: &TaskId, registry: &Registry) -> Vec<&'a Task> {
    descendants(tasks, id, registry)
        .into_iter()
        .filter(|task| task.status.is_open())
        .collect()
}

/// spec §4.7: `is_ready` excludes any task with children, so a cadence on a goal could
/// never fire. Refuse it at the write rather than leave a silent dead end. Scans only when
/// a cadence is actually set, which is rare.
pub fn validate_periodic(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    if task.every.is_none() {
        return Ok(());
    }
    let all = project.scan()?;
    let kids = children(&all, &task.id, registry);
    if !kids.is_empty() {
        return Err(Error::Validation(format!(
            "{} has children ({}) and cannot be a recurrence; a task with children is a \
             goal, and a goal is never ready",
            task.id,
            kids.iter()
                .map(|kid| kid.id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(())
}

/// The forest under `root` (or every root when `None`). Without `include_closed`, a node
/// is kept when it is open or has an open descendant, so a closed ancestor of open work
/// stays visible as context. Roots and siblings are in ready order. A task whose parent is
/// missing from `all` is treated as a root; members of a parent cycle are not shown at all
/// (`check` reports them as `parent_cycle`).
pub fn forest(
    all: &[Task],
    root: Option<&TaskId>,
    include_closed: bool,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    now: OffsetDateTime,
) -> Vec<TreeNode> {
    let mut tops: Vec<&Task> = match root {
        Some(id) => {
            let id = registry.canonical_id(id);
            all.iter().filter(|task| task.id == id).collect()
        }
        None => all
            .iter()
            .filter(|task| {
                task.parent.as_ref().is_none_or(|parent| {
                    let parent = registry.canonical_id(parent);
                    !all.iter().any(|candidate| candidate.id == parent)
                })
            })
            .collect(),
    };
    tops.sort_by(|a, b| ready_order(a, b));
    tops.into_iter()
        .filter_map(|task| {
            node(
                all,
                task,
                include_closed,
                claims,
                registry,
                &mut std::collections::HashSet::new(),
                now,
            )
        })
        .collect()
}

fn node(
    all: &[Task],
    task: &Task,
    include_closed: bool,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    visited: &mut std::collections::HashSet<TaskId>,
    now: OffsetDateTime,
) -> Option<TreeNode> {
    if !visited.insert(task.id.clone()) {
        return None;
    }
    let keep = include_closed
        || task.status.is_open()
        || !open_descendants(all, &task.id, registry).is_empty();
    if !keep {
        return None;
    }
    let mut kids = children(all, &task.id, registry);
    kids.sort_by(|a, b| ready_order(a, b));
    Some(TreeNode {
        summary: TaskSummary::of(task, all, claims, registry, now),
        children: kids
            .into_iter()
            .filter_map(|child| node(all, child, include_closed, claims, registry, visited, now))
            .collect(),
    })
}

fn join(path: &[TaskId]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    pub(crate) fn task(id: &str, parent: Option<&str>, status: Status) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: id.into(),
            status,
            priority: 2,
            size: None,
            parallel: false,
            every: None,
            owner: None,
            created: "2026-09-03T00:00:00Z".into(),
            updated: "2026-09-03T00:00:00Z".into(),
            last_done: None,
            depends: vec![],
            parent: parent.map(|p| TaskId::parse(p).unwrap()),
            tags: vec![],
            source: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn parent_cycle_finds_loops_of_any_length_and_ignores_chains() {
        let registry = Registry::default();
        let a = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b = task("xx-000002", Some("xx-000003"), Status::Todo);
        let c = task("xx-000003", Some("xx-000001"), Status::Todo);
        let cycle = parent_cycle(&[a.clone(), b.clone(), c], &a.id, &registry).unwrap();
        assert_eq!(cycle.len(), 4);
        assert_eq!(cycle[0], cycle[3]);
        let root = task("xx-000003", None, Status::Todo);
        assert!(parent_cycle(&[a.clone(), b, root], &a.id, &registry).is_none());
        let dangling = task("xx-000009", Some("xx-000008"), Status::Todo);
        assert!(parent_cycle(std::slice::from_ref(&dangling), &dangling.id, &registry).is_none());
        let own = task("xx-000007", Some("xx-000007"), Status::Todo);
        assert_eq!(
            parent_cycle(std::slice::from_ref(&own), &own.id, &registry)
                .unwrap()
                .len(),
            2
        );
        // a tail entering a loop reports only the loop: C -> A -> B -> A yields
        // A -> B -> A from C, the same as from A; from B the same members in B's order
        let tail = task("xx-000005", Some("xx-000001"), Status::Todo);
        let a2 = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b2 = task("xx-000002", Some("xx-000001"), Status::Todo);
        let all = [tail.clone(), a2.clone(), b2.clone()];
        let ids = |path: &[TaskId]| path.iter().map(ToString::to_string).collect::<Vec<_>>();
        let from_tail = parent_cycle(&all, &tail.id, &registry).unwrap();
        assert_eq!(ids(&from_tail), ["xx-000001", "xx-000002", "xx-000001"]);
        assert_eq!(
            ids(&parent_cycle(&all, &a2.id, &registry).unwrap()),
            ids(&from_tail)
        );
        let from_b = parent_cycle(&all, &b2.id, &registry).unwrap();
        assert_eq!(ids(&from_b), ["xx-000002", "xx-000001", "xx-000002"]);
        let members = |path: &[TaskId]| {
            let mut m: Vec<String> = ids(&path[..path.len() - 1]);
            m.sort();
            m
        };
        assert_eq!(
            members(&from_b),
            members(&from_tail),
            "same set, so check dedupes"
        );
    }

    #[test]
    fn open_descendants_see_through_a_closed_middle_node() {
        let registry = Registry::default();
        let a = task("xx-000001", None, Status::Todo);
        let b = task("xx-000002", Some("xx-000001"), Status::Done);
        let c = task("xx-000003", Some("xx-000002"), Status::Todo);
        let all = [a.clone(), b, c];
        assert_eq!(children(&all, &a.id, &registry).len(), 1);
        assert_eq!(descendants(&all, &a.id, &registry).len(), 2);
        let open: Vec<String> = open_descendants(&all, &a.id, &registry)
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(open, ["xx-000003"]);
    }

    #[test]
    fn forest_prunes_closed_leaves_but_keeps_closed_ancestors_of_open_work() {
        let registry = Registry::default();
        let root = task("xx-000001", None, Status::Todo);
        let closed_leaf = task("xx-000002", Some("xx-000001"), Status::Done);
        let closed_mid = task("xx-000003", Some("xx-000001"), Status::Done);
        let open_deep = task("xx-000004", Some("xx-000003"), Status::Todo);
        let all = [root, closed_leaf, closed_mid, open_deep];
        let now = crate::time::parse("2026-01-01T00:00:00Z").unwrap();
        let nodes = forest(&all, None, false, None, &registry, now);
        assert_eq!(nodes.len(), 1);
        let kids: Vec<&str> = nodes[0]
            .children
            .iter()
            .map(|n| n.summary.id.as_str())
            .collect();
        assert_eq!(kids, ["xx-000003"]);
        assert_eq!(nodes[0].children[0].children[0].summary.id, "xx-000004");
        assert_eq!(
            forest(&all, None, true, None, &registry, now)[0]
                .children
                .len(),
            2
        );
        assert_eq!(
            forest(&all, Some(&all[2].id), true, None, &registry, now)[0]
                .summary
                .id,
            "xx-000003"
        );
    }
}
