use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::collections::HashMap;

/// Rejects a `parent` that is foreign, missing, or would make `task` its own ancestor.
/// Reads ancestors from disk, so it is the write-path check; `check` uses `parent_cycle`.
pub fn validate_parent(project: &Project, task: &Task) -> Result<()> {
    let Some(parent) = &task.parent else {
        return Ok(());
    };
    if parent.prefix != project.prefix {
        return Err(Error::Validation(format!(
            "parent {parent} must be in this project ({})",
            project.prefix
        )));
    }
    if parent == &task.id {
        return Err(Error::Cycle(format!("{parent} -> {parent}")));
    }
    if !project.task_path(parent).is_file() {
        return Err(Error::UnresolvableId(parent.to_string()));
    }
    let mut path = vec![task.id.clone()];
    let mut current = Some(parent.clone());
    while let Some(id) = current {
        if path.contains(&id) {
            path.push(id);
            return Err(Error::Cycle(join(&path)));
        }
        let ancestor = project.read_task(&id)?;
        path.push(id);
        current = ancestor.parent;
    }
    Ok(())
}

/// Walks the parent chain upward from `start` and returns the loop it runs into, if any,
/// as `a -> b -> a`: only the cycle, never the tail that led into it. The path starts at
/// whichever member the walk enters first (`b -> a -> b` from `b`), so callers that
/// deduplicate must key on the set of members, as `check` does, not on the order. Missing
/// parents end the walk without a cycle.
pub fn parent_cycle(tasks: &[Task], start: &TaskId) -> Option<Vec<TaskId>> {
    let parents: HashMap<&TaskId, &TaskId> = tasks
        .iter()
        .filter_map(|task| task.parent.as_ref().map(|parent| (&task.id, parent)))
        .collect();
    let mut path = vec![start.clone()];
    let mut current = parents.get(start).copied();
    while let Some(id) = current {
        if let Some(position) = path.iter().position(|item| item == id) {
            let mut cycle = path[position..].to_vec();
            cycle.push(id.clone());
            return Some(cycle);
        }
        path.push(id.clone());
        current = parents.get(id).copied();
    }
    None
}

/// Direct children of `id`, in the order they appear in `tasks`.
pub fn children<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|task| task.parent.as_ref() == Some(id))
        .collect()
}

/// Every task below `id`, depth first. A visited set makes a corrupt loop terminate.
pub fn descendants<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    let mut out = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<&TaskId> = vec![id];
    while let Some(current) = stack.pop() {
        for child in children(tasks, current) {
            if visited.insert(&child.id) {
                out.push(child);
                stack.push(&child.id);
            }
        }
    }
    out
}

pub fn open_descendants<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    descendants(tasks, id)
        .into_iter()
        .filter(|task| task.status.is_open())
        .collect()
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
            owner: None,
            created: "2026-09-03T00:00:00Z".into(),
            updated: "2026-09-03T00:00:00Z".into(),
            depends: vec![],
            parent: parent.map(|p| TaskId::parse(p).unwrap()),
            tags: vec![],
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn parent_cycle_finds_loops_of_any_length_and_ignores_chains() {
        let a = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b = task("xx-000002", Some("xx-000003"), Status::Todo);
        let c = task("xx-000003", Some("xx-000001"), Status::Todo);
        let cycle = parent_cycle(&[a.clone(), b.clone(), c], &a.id).unwrap();
        assert_eq!(cycle.len(), 4);
        assert_eq!(cycle[0], cycle[3]);
        let root = task("xx-000003", None, Status::Todo);
        assert!(parent_cycle(&[a.clone(), b, root], &a.id).is_none());
        let dangling = task("xx-000009", Some("xx-000008"), Status::Todo);
        assert!(parent_cycle(std::slice::from_ref(&dangling), &dangling.id).is_none());
        let own = task("xx-000007", Some("xx-000007"), Status::Todo);
        assert_eq!(
            parent_cycle(std::slice::from_ref(&own), &own.id)
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
        let from_tail = parent_cycle(&all, &tail.id).unwrap();
        assert_eq!(ids(&from_tail), ["xx-000001", "xx-000002", "xx-000001"]);
        assert_eq!(ids(&parent_cycle(&all, &a2.id).unwrap()), ids(&from_tail));
        let from_b = parent_cycle(&all, &b2.id).unwrap();
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
        let a = task("xx-000001", None, Status::Todo);
        let b = task("xx-000002", Some("xx-000001"), Status::Done);
        let c = task("xx-000003", Some("xx-000002"), Status::Todo);
        let all = [a.clone(), b, c];
        assert_eq!(children(&all, &a.id).len(), 1);
        assert_eq!(descendants(&all, &a.id).len(), 2);
        let open: Vec<String> = open_descendants(&all, &a.id)
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(open, ["xx-000003"]);
    }
}
