use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::output::{TaskSummary, TreeNode};
use crate::query::ready_order;
use crate::registry::Registry;
use crate::repo::Project;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use time::OffsetDateTime;

#[cfg(test)]
thread_local! {
    static INDEX_CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INDEX_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_test_index_counts() {
    INDEX_CONSTRUCTIONS.with(|c| c.set(0));
    INDEX_LOOKUPS.with(|c| c.set(0));
}

#[cfg(test)]
pub(crate) fn test_index_counts() -> (usize, usize) {
    (
        INDEX_CONSTRUCTIONS.with(|c| c.get()),
        INDEX_LOOKUPS.with(|c| c.get()),
    )
}

#[cfg(test)]
fn record_construction() {
    INDEX_CONSTRUCTIONS.with(|c| c.set(c.get() + 1));
}

#[cfg(not(test))]
fn record_construction() {}

#[cfg(test)]
fn record_lookup() {
    INDEX_LOOKUPS.with(|c| c.set(c.get() + 1));
}

#[cfg(not(test))]
fn record_lookup() {}

pub struct HierarchyIndex {
    ids: Vec<TaskId>,
    by_id: HashMap<TaskId, usize>,
    children: Vec<Vec<usize>>,
    orphan_children: HashMap<TaskId, Vec<usize>>,
    open_counts: Vec<usize>,
    effective_open: Vec<bool>,
    active_counts: Vec<usize>,
    lane_slots: Vec<Option<usize>>,
    enclosing_lanes: Vec<Option<usize>>,
}

pub type ProjectIndexes = BTreeMap<String, Arc<HierarchyIndex>>;
pub type ProjectRecords<'a> = BTreeMap<String, Vec<&'a Task>>;

impl HierarchyIndex {
    pub fn new(all: &[&Task], registry: &Registry) -> Self {
        record_construction();
        let n = all.len();
        let mut ids = Vec::with_capacity(n);
        let mut by_id = HashMap::with_capacity(n);
        for (slot, task) in all.iter().enumerate() {
            let canonical = registry.canonical_id(&task.id);
            by_id.entry(canonical.clone()).or_insert(slot);
            ids.push(canonical);
        }

        let mut parent_slots = vec![None; n];
        let mut children = vec![Vec::new(); n];
        let mut orphan_children: HashMap<TaskId, Vec<usize>> = HashMap::new();
        for (slot, task) in all.iter().enumerate() {
            let Some(raw_parent) = &task.parent else {
                continue;
            };
            let parent = registry.canonical_id(raw_parent);
            if parent.prefix != ids[slot].prefix {
                continue;
            }
            if let Some(&parent_slot) = by_id.get(&parent) {
                parent_slots[slot] = Some(parent_slot);
                children[parent_slot].push(slot);
            } else {
                orphan_children.entry(parent).or_default().push(slot);
            }
        }

        let mut acyclic = vec![false; n];
        let mut open_counts = vec![0; n];
        let mut effective_open = vec![false; n];
        let mut active_counts = vec![0; n];
        let mut lane_slots = vec![None; n];
        let mut enclosing_lanes = vec![None; n];
        let mut postorder = Vec::with_capacity(n);
        let mut stack = Vec::new();

        for root in 0..n {
            if parent_slots[root].is_some() {
                continue;
            }
            enclosing_lanes[root] = None;
            lane_slots[root] = all[root].lane.then_some(root);
            stack.push(root);
            while let Some(u) = stack.pop() {
                acyclic[u] = true;
                postorder.push(u);
                let inherited_lane = lane_slots[u];
                for &child in &children[u] {
                    enclosing_lanes[child] = inherited_lane;
                    lane_slots[child] = if all[child].lane {
                        Some(child)
                    } else {
                        inherited_lane
                    };
                    stack.push(child);
                }
            }
        }

        for &u in postorder.iter().rev() {
            let mut open = 0;
            let mut active = 0;
            for &child in &children[u] {
                if all[child].status != Status::Dropped {
                    effective_open[u] |= all[child].status.is_open() || effective_open[child];
                }
                open += open_counts[child] + usize::from(all[child].status.is_open());
                active += active_counts[child] + usize::from(is_active(all[child]));
            }
            open_counts[u] = open;
            active_counts[u] = active;
        }

        // Corrupt parent cycles (or tails whose parent chain enters a cycle) use visited-set walks.
        for u in 0..n {
            if acyclic[u] {
                continue;
            }
            let mut seen = HashSet::from([u]);
            let mut curr = parent_slots[u];
            let mut above = None;
            while let Some(p) = curr {
                if !seen.insert(p) {
                    break;
                }
                if all[p].lane {
                    above = Some(p);
                    break;
                }
                curr = parent_slots[p];
            }
            enclosing_lanes[u] = above;
            lane_slots[u] = if all[u].lane { Some(u) } else { above };

            let mut visited = HashSet::from([u]);
            let mut dfs = vec![u];
            let mut open = 0;
            let mut active = 0;
            while let Some(curr_slot) = dfs.pop() {
                for &child in &children[curr_slot] {
                    if visited.insert(child) {
                        open += usize::from(all[child].status.is_open());
                        active += usize::from(is_active(all[child]));
                        dfs.push(child);
                    }
                }
            }
            open_counts[u] = open;
            active_counts[u] = active;
        }

        HierarchyIndex {
            ids,
            by_id,
            children,
            orphan_children,
            open_counts,
            effective_open,
            active_counts,
            lane_slots,
            enclosing_lanes,
        }
    }

    pub fn position(&self, canonical_id: &TaskId) -> Option<usize> {
        record_lookup();
        self.by_id.get(canonical_id).copied()
    }

    pub fn children(&self, at: usize) -> &[usize] {
        record_lookup();
        &self.children[at]
    }

    pub(crate) fn children_of_id(&self, canonical_id: &TaskId) -> &[usize] {
        record_lookup();
        if let Some(&slot) = self.by_id.get(canonical_id) {
            &self.children[slot]
        } else if let Some(slots) = self.orphan_children.get(canonical_id) {
            slots.as_slice()
        } else {
            &[]
        }
    }

    pub fn descendants(&self, at: usize) -> Vec<usize> {
        record_lookup();
        let mut out = Vec::new();
        let mut visited = HashSet::from([at]);
        let mut stack = vec![at];
        while let Some(current) = stack.pop() {
            for &child in &self.children[current] {
                if visited.insert(child) {
                    out.push(child);
                    stack.push(child);
                }
            }
        }
        out
    }

    pub fn open_descendant_count(&self, at: usize) -> usize {
        record_lookup();
        self.open_counts[at]
    }

    pub fn active_descendant_count(&self, at: usize) -> usize {
        record_lookup();
        self.active_counts[at]
    }

    pub fn lane_of(&self, at: usize) -> Option<&TaskId> {
        record_lookup();
        self.lane_slots[at].map(|slot| &self.ids[slot])
    }

    pub fn enclosing_lane(&self, at: usize) -> Option<usize> {
        record_lookup();
        self.enclosing_lanes[at]
    }
}

pub fn project_records<'a>(
    all: &'a [Task],
    opened: &[String],
    registry: &Registry,
) -> ProjectRecords<'a> {
    let mut records: ProjectRecords<'a> = opened
        .iter()
        .map(|prefix| (registry.canonical_prefix(prefix).to_string(), Vec::new()))
        .collect();
    for task in all {
        let prefix = registry.canonical_id(&task.id).prefix;
        records.entry(prefix).or_default().push(task);
    }
    records
}

pub fn project_indexes(records: &ProjectRecords<'_>, registry: &Registry) -> ProjectIndexes {
    records
        .iter()
        .map(|(prefix, tasks)| {
            (
                prefix.clone(),
                Arc::new(HierarchyIndex::new(tasks, registry)),
            )
        })
        .collect()
}

pub struct HierarchyView<'a> {
    records: ProjectRecords<'a>,
    indexes: ProjectIndexes,
}

impl<'a> HierarchyView<'a> {
    pub fn new(records: ProjectRecords<'a>, indexes: ProjectIndexes, registry: &Registry) -> Self {
        assert_eq!(
            records.len(),
            indexes.len(),
            "HierarchyView records and indexes key count mismatch"
        );
        for (prefix, tasks) in &records {
            assert_eq!(
                registry.canonical_prefix(prefix),
                prefix.as_str(),
                "HierarchyView prefix {prefix} must be canonical"
            );
            let index = indexes
                .get(prefix)
                .unwrap_or_else(|| panic!("HierarchyView missing index for prefix {prefix}"));
            assert_eq!(
                tasks.len(),
                index.ids.len(),
                "HierarchyView length mismatch for prefix {prefix}"
            );
            for (slot, task) in tasks.iter().enumerate() {
                let canonical = registry.canonical_id(&task.id);
                assert_eq!(
                    canonical.prefix, *prefix,
                    "HierarchyView record {canonical} stored under wrong prefix {prefix}"
                );
                assert_eq!(
                    index.ids[slot], canonical,
                    "HierarchyView slot {slot} mismatch in prefix {prefix}"
                );
            }
        }
        HierarchyView { records, indexes }
    }

    fn entry_and_slot(&self, canonical_id: &TaskId) -> (&[&'a Task], &HierarchyIndex, usize) {
        let tasks = self.records.get(&canonical_id.prefix).unwrap_or_else(|| {
            panic!(
                "HierarchyView missing project records for {}",
                canonical_id.prefix
            )
        });
        let index = self.indexes.get(&canonical_id.prefix).unwrap_or_else(|| {
            panic!(
                "HierarchyView missing project index for {}",
                canonical_id.prefix
            )
        });
        let slot = index.position(canonical_id).unwrap_or_else(|| {
            panic!("HierarchyView missing record {canonical_id} in captured scan")
        });
        (tasks.as_slice(), index.as_ref(), slot)
    }

    pub fn get(&self, canonical_id: &TaskId) -> Option<&'a Task> {
        let tasks = self.records.get(&canonical_id.prefix)?;
        let index = self.indexes.get(&canonical_id.prefix)?;
        let slot = index.position(canonical_id)?;
        Some(tasks[slot])
    }

    pub fn children(&self, canonical_id: &TaskId) -> Vec<&'a Task> {
        let (tasks, index, slot) = self.entry_and_slot(canonical_id);
        index.children(slot).iter().map(|&c| tasks[c]).collect()
    }

    pub fn descendants(&self, canonical_id: &TaskId) -> Vec<&'a Task> {
        let (tasks, index, slot) = self.entry_and_slot(canonical_id);
        index
            .descendants(slot)
            .into_iter()
            .map(|d| tasks[d])
            .collect()
    }

    pub fn child_count(&self, canonical_id: &TaskId) -> usize {
        let (_, index, slot) = self.entry_and_slot(canonical_id);
        index.children(slot).len()
    }

    pub fn open_descendant_count(&self, canonical_id: &TaskId) -> usize {
        let (_, index, slot) = self.entry_and_slot(canonical_id);
        index.open_descendant_count(slot)
    }

    /// A conservative local exclusion. Nodes in corrupt cyclic components
    /// pass through to the completion evaluator for a path diagnostic.
    pub fn has_effective_open_descendant(&self, canonical_id: &TaskId) -> bool {
        let (_, index, slot) = self.entry_and_slot(canonical_id);
        index.effective_open[slot]
    }

    pub fn active_descendant_count(&self, canonical_id: &TaskId) -> usize {
        let (_, index, slot) = self.entry_and_slot(canonical_id);
        index.active_descendant_count(slot)
    }

    pub fn lane_of(&self, canonical_id: &TaskId) -> Option<&'a Task> {
        let (tasks, index, slot) = self.entry_and_slot(canonical_id);
        let lane_id = index.lane_of(slot)?;
        let lane_slot = index
            .position(lane_id)
            .expect("HierarchyIndex lane_of references a slot in the same index");
        Some(tasks[lane_slot])
    }

    pub fn enclosing_lane(&self, canonical_id: &TaskId) -> Option<&'a Task> {
        let (tasks, index, slot) = self.entry_and_slot(canonical_id);
        index.enclosing_lane(slot).map(|l| tasks[l])
    }

    pub fn paused_lane(&self, canonical_id: &TaskId) -> Option<&'a Task> {
        self.enclosing_lane(canonical_id)
            .filter(|lane| lane.status == Status::Blocked)
    }
}

pub fn view_of<'a>(all: &'a [Task], opened: &[String], registry: &Registry) -> HierarchyView<'a> {
    let records = project_records(all, opened, registry);
    let indexes = project_indexes(&records, registry);
    HierarchyView::new(records, indexes, registry)
}

/// Rejects a `parent` that is foreign, missing, or would make `task` its own ancestor.
/// Reads ancestors from disk, so it is the write-path check; `check` uses `parent_cycle`.
pub fn validate_parent(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    validate_parent_among(project, registry, task, &[])
}

/// A record of `project` as it will stand: one of `projected` (records not yet written,
/// which win), else the one on disk. `None` when neither holds it.
fn projected_read(
    project: &Project,
    projected: &[Task],
    registry: &Registry,
    id: &TaskId,
) -> Result<Option<Task>> {
    if let Some(task) = projected
        .iter()
        .find(|task| registry.canonical_id(&task.id) == *id)
    {
        return Ok(Some(task.clone()));
    }
    match project.read_task(id) {
        Ok(task) => Ok(Some(task)),
        Err(Error::TaskNotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// `validate_parent` over the project as it will stand once `projected` is written: a
/// parent or ancestor may be one of those records. With `projected` empty, it is the
/// write-path check.
pub fn validate_parent_among(
    project: &Project,
    registry: &Registry,
    task: &Task,
    projected: &[Task],
) -> Result<()> {
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
    let projected_parent = projected
        .iter()
        .any(|record| registry.canonical_id(&record.id) == parent);
    if !projected_parent && !project.task_path(&parent).is_file() {
        return Err(Error::UnresolvableId(format!(
            "parent {parent} does not exist"
        )));
    }
    // A recurrence or deferred task is never a goal, so nothing may hang beneath one.
    let parent_record = projected_read(project, projected, registry, &parent)?
        .ok_or_else(|| Error::TaskNotFound(parent.to_string()))?;
    crate::tombstone::refuse_reference(&parent_record, "parent")?;
    crate::moving::journal::refuse_pending_id(&parent)?;
    if parent_record.every.is_some() {
        return Err(Error::Validation(format!(
            "{parent} is a recurrence and cannot have children; clear its cadence with \
             `tasks edit {parent} --no-every` first"
        )));
    }
    if parent_record.defer.is_some() && parent_record.status != Status::Shelved {
        return Err(Error::Validation(format!(
            "{parent} is deferred and cannot have children; clear the date with \
             `tasks edit {parent} --no-defer` first"
        )));
    }
    let mut path = vec![task_id];
    let mut current = Some(parent);
    while let Some(id) = current {
        if path.contains(&id) {
            path.push(id);
            return Err(Error::Cycle(join(&path)));
        }
        let Some(ancestor) = projected_read(project, projected, registry, &id)? else {
            break;
        };
        path.push(id);
        current = ancestor
            .parent
            .as_ref()
            .map(|parent| registry.canonical_id(parent));
    }
    Ok(())
}

/// No lane may have a lane as an ancestor. Covers both directions for the record being
/// written: a lane above it when it is a lane, and a lane below it when it becomes a lane
/// or moves under one. Ancestors are read from disk, as in `validate_parent`, which runs
/// first and has already refused a parent loop. Only a write that creates nesting is refused,
/// and the project is scanned only when the record's subtree could newly meet a lane, so
/// ordinary writes inside a lane stay cheap; nesting already on disk is `check`'s to report.
pub fn validate_lanes(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    validate_lanes_among(project, registry, task, &[])
}

/// `validate_lanes` over the project as it will stand once `projected` is written: an
/// ancestor or descendant may be one of those records. `task` itself is judged against
/// what is on disk, so a projected record not yet written has no stored copy.
pub fn validate_lanes_among(
    project: &Project,
    registry: &Registry,
    task: &Task,
    projected: &[Task],
) -> Result<()> {
    let task_id = registry.canonical_id(&task.id);
    // A record not yet on disk has no children, and any lane above it is new nesting.
    let stored = match project.read_task(&task_id) {
        Ok(stored) => Some(stored),
        Err(Error::TaskNotFound(_)) => None,
        Err(error) => return Err(error),
    };
    let canonical = |parent: &Option<TaskId>| parent.as_ref().map(|id| registry.canonical_id(id));
    let moved = stored
        .as_ref()
        .is_none_or(|stored| canonical(&stored.parent) != canonical(&task.parent));
    let became_lane = task.lane && stored.as_ref().is_none_or(|stored| !stored.lane);
    // Only a write that introduces nesting is refused; a write that leaves nesting already
    // on disk as it is (a note, a status change) goes through.
    if !became_lane && !moved {
        return Ok(());
    }
    let above = lane_above(project, registry, task, projected)?;
    if task.lane
        && let Some(outer) = &above
    {
        return Err(Error::NestedLane(format!(
            "{task_id} cannot be a lane inside lane {outer}; a sub-effort inside a lane is an \
             ordinary child goal"
        )));
    }
    if stored.is_none() || (!became_lane && above.is_none()) {
        return Ok(());
    }
    let mut all: Vec<Task> = project
        .scan()?
        .into_iter()
        .filter(|stored| {
            !projected
                .iter()
                .any(|record| registry.canonical_id(&record.id) == stored.id)
        })
        .collect();
    all.extend(projected.iter().cloned());
    let Some(inner) = descendants(&all, &task_id, registry)
        .into_iter()
        .find(|descendant| descendant.lane)
    else {
        return Ok(());
    };
    Err(Error::NestedLane(match above {
        Some(outer) => format!(
            "moving {task_id} under lane {outer} would put lane {} inside it",
            inner.id
        ),
        None => format!("{task_id} cannot be a lane: lane {} is below it", inner.id),
    }))
}

/// The nearest lane above `task` on disk. A loop or a missing parent ends the walk.
fn lane_above(
    project: &Project,
    registry: &Registry,
    task: &Task,
    projected: &[Task],
) -> Result<Option<TaskId>> {
    let mut seen = HashSet::new();
    let mut current = task
        .parent
        .as_ref()
        .map(|parent| registry.canonical_id(parent));
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            break;
        }
        let Some(ancestor) = projected_read(project, projected, registry, &id)? else {
            break;
        };
        if ancestor.lane {
            return Ok(Some(id));
        }
        current = ancestor
            .parent
            .as_ref()
            .map(|parent| registry.canonical_id(parent));
    }
    Ok(None)
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

pub fn is_active(task: &Task) -> bool {
    task.status.is_open() && task.status != Status::Shelved
}

/// The one test for "this task is a goal", used wherever a goal is treated specially. A
/// lane is a goal even before its first child exists, and so is a record that declares
/// pieces (docs/task-model.md).
pub fn is_goal(task: &Task, has_children: bool) -> bool {
    has_children || task.lane || !task.pieces.is_empty()
}

/// The nearest lane strictly above `task` in `all`, walking `parent` links. A visited set
/// ends a corrupt loop, and a parent missing from `all` ends the walk.
pub fn enclosing_lane<'a>(all: &'a [Task], task: &Task, registry: &Registry) -> Option<&'a Task> {
    let mut seen = std::collections::HashSet::new();
    let mut current = task
        .parent
        .as_ref()
        .map(|parent| registry.canonical_id(parent));
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            return None;
        }
        let ancestor = all.iter().find(|candidate| candidate.id == id)?;
        if ancestor.lane {
            return Some(ancestor);
        }
        current = ancestor
            .parent
            .as_ref()
            .map(|parent| registry.canonical_id(parent));
    }
    None
}

/// The lane a row belongs to: the task itself when it is a lane, else its nearest lane
/// ancestor. Computed from the scan, never stored.
pub fn lane_of(all: &[Task], task: &Task, registry: &Registry) -> Option<TaskId> {
    if task.lane {
        return Some(registry.canonical_id(&task.id));
    }
    enclosing_lane(all, task, registry).map(|lane| lane.id.clone())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shelved {
    Hidden,
    UnderShownParent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visibility {
    All,
    Hidden,
    UnderShownParent,
}

/// Current contract (docs/task-model.md): `readiness` excludes a goal, so a cadence on one could never fire. Refuse it
/// at the write rather than leave a silent dead end.
pub fn validate_periodic(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    if task.every.is_none() {
        return Ok(());
    }
    refuse_goal(project, registry, task, "be a recurrence")
}

/// A deferred goal would never appear in a picker.
pub fn validate_defer(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    if task.defer.is_none() || task.status == Status::Shelved {
        return Ok(());
    }
    refuse_goal(project, registry, task, "be deferred")
}

/// Refuses `task` when it is a goal. A lane or a record with pieces is refused without a
/// scan, and a record not yet on disk has no children, so the project is scanned only when
/// the field is set on an existing record that is neither, which is rare.
fn refuse_goal(project: &Project, registry: &Registry, task: &Task, what: &str) -> Result<()> {
    let kids: Vec<String> =
        if task.lane || !task.pieces.is_empty() || !project.task_path(&task.id).is_file() {
            Vec::new()
        } else {
            children(&project.scan()?, &task.id, registry)
                .iter()
                .map(|kid| kid.id.to_string())
                .collect()
        };
    if !is_goal(task, !kids.is_empty()) {
        return Ok(());
    }
    Err(Error::Validation(if !kids.is_empty() {
        format!(
            "{} has children ({}) and cannot {what}; a task with children is a goal, and a \
             goal is never ready",
            task.id,
            kids.join(", ")
        )
    } else if !task.pieces.is_empty() {
        format!(
            "{} declares pieces ({}) and cannot {what}; a task with pieces is a goal, and a \
             goal is never ready",
            task.id,
            crate::pieces::listed(task)
        )
    } else {
        format!(
            "{} is a lane and cannot {what}; a lane is a goal, and a goal is never ready",
            task.id
        )
    }))
}

/// The forest under `root` (or every root when `None`). Without `include_closed`, a node
/// is kept when it is active, has an active descendant, or is a shelved child in
/// `Shelved::UnderShownParent` mode. Roots and siblings are in ready order. A task whose parent is
/// missing from `all` is treated as a root; members of a parent cycle are not shown at all
/// (`check` reports them as `parent_cycle`).
#[allow(clippy::too_many_arguments)]
pub fn forest(
    all: &[Task],
    hierarchy: &HierarchyView<'_>,
    deps: &crate::review::Deps,
    root: Option<&TaskId>,
    include_closed: bool,
    shelved: Shelved,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    now: OffsetDateTime,
) -> Vec<TreeNode> {
    let mut tops: Vec<&Task> = match root {
        Some(id) => {
            let id = registry.canonical_id(id);
            all.iter()
                .filter(|task| registry.canonical_id(&task.id) == id)
                .collect()
        }
        None => all
            .iter()
            .filter(|task| {
                let task_id = registry.canonical_id(&task.id);
                task.parent.as_ref().is_none_or(|parent| {
                    let parent = registry.canonical_id(parent);
                    parent.prefix != task_id.prefix || hierarchy.get(&parent).is_none()
                })
            })
            .collect(),
    };
    let visibility = if include_closed {
        Visibility::All
    } else if shelved == Shelved::UnderShownParent {
        Visibility::UnderShownParent
    } else {
        Visibility::Hidden
    };
    tops.sort_by(|a, b| ready_order(a, b));
    tops.into_iter()
        .filter(|task| visibility == Visibility::All || task.status != Status::Shelved)
        .filter_map(|task| {
            node(
                hierarchy,
                deps,
                task,
                visibility,
                claims,
                registry,
                &mut std::collections::HashSet::new(),
                now,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn node(
    hierarchy: &HierarchyView<'_>,
    deps: &crate::review::Deps,
    task: &Task,
    visibility: Visibility,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    visited: &mut std::collections::HashSet<TaskId>,
    now: OffsetDateTime,
) -> Option<TreeNode> {
    let canonical = registry.canonical_id(&task.id);
    if !visited.insert(canonical.clone()) {
        return None;
    }
    let keep = visibility == Visibility::All
        || if task.status == Status::Shelved {
            visibility == Visibility::UnderShownParent
        } else {
            is_active(task) || hierarchy.active_descendant_count(&canonical) > 0
        };
    if !keep {
        return None;
    }
    let mut kids = hierarchy.children(&canonical);
    kids.sort_by(|a, b| ready_order(a, b));
    Some(TreeNode {
        summary: TaskSummary::of(task, hierarchy, deps, claims, registry, now),
        children: kids
            .into_iter()
            .filter_map(|child| {
                node(
                    hierarchy, deps, child, visibility, claims, registry, visited, now,
                )
            })
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
pub(crate) mod tests {
    use super::*;
    use crate::model::Status;

    pub(crate) fn task(id: &str, parent: Option<&str>, status: Status) -> Task {
        Task {
            note_archives: Vec::new(),
            wake_on: Vec::new(),
            id: TaskId::parse(id).unwrap(),
            title: id.into(),
            status,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            lane: false,
            reviewed: false,
            awaiting_user: false,
            reviewed_at: None,
            closed: None,
            needs: vec![],
            every: None,
            defer: None,
            owner: None,
            created: "2026-09-03T00:00:00Z".into(),
            updated: "2026-09-03T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            related: vec![],
            follows: vec![],
            pieces: vec![],
            parent: parent.map(|p| TaskId::parse(p).unwrap()),
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            origin: None,
            spec: None,
            plan: None,
            step: None,
            moved_to: None,
            moved_from: None,
            body: String::new(),
            refs: vec![],
            notes: vec![],
        }
    }

    #[test]
    fn is_goal_counts_children_lanes_and_declared_pieces() {
        let plain = task("aa-000001", None, Status::Todo);
        assert!(!is_goal(&plain, false));
        assert!(is_goal(&plain, true));
        let mut lane = plain.clone();
        lane.lane = true;
        assert!(is_goal(&lane, false));
        let mut hub = plain.clone();
        hub.pieces = vec![TaskId::parse("bb-000001").unwrap()];
        assert!(is_goal(&hub, false));
    }

    #[test]
    fn lane_of_is_the_task_itself_or_its_nearest_lane_ancestor() {
        let registry = Registry::default();
        let mut lane = task("xx-000001", None, Status::Todo);
        lane.lane = true;
        let goal = task("xx-000002", Some("xx-000001"), Status::Todo);
        let step = task("xx-000003", Some("xx-000002"), Status::Todo);
        let loose = task("xx-000004", None, Status::Todo);
        let all = [lane.clone(), goal, step.clone(), loose.clone()];
        assert_eq!(lane_of(&all, &step, &registry), Some(lane.id.clone()));
        assert_eq!(lane_of(&all, &lane, &registry), Some(lane.id.clone()));
        assert!(
            enclosing_lane(&all, &lane, &registry).is_none(),
            "strictly above"
        );
        assert_eq!(lane_of(&all, &loose, &registry), None);
        let a = task("xx-000005", Some("xx-000006"), Status::Todo);
        let b = task("xx-000006", Some("xx-000005"), Status::Todo);
        assert_eq!(
            lane_of(&[a.clone(), b], &a, &registry),
            None,
            "a parent loop ends the walk"
        );
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
    fn a_lane_is_a_goal_with_or_without_children() {
        let mut lane = task("xx-000001", None, Status::Todo);
        assert!(!is_goal(&lane, false));
        assert!(is_goal(&lane, true));
        lane.lane = true;
        assert!(
            is_goal(&lane, false),
            "a lane is a goal before its first child"
        );
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
        let deps = crate::review::Deps::from_states([]);
        let view = view_of(&all, &["xx".to_string()], &registry);
        let nodes = forest(
            &all,
            &view,
            &deps,
            None,
            false,
            Shelved::Hidden,
            None,
            &registry,
            now,
        );
        assert_eq!(nodes.len(), 1);
        let kids: Vec<&str> = nodes[0]
            .children
            .iter()
            .map(|n| n.summary.id.as_str())
            .collect();
        assert_eq!(kids, ["xx-000003"]);
        assert_eq!(nodes[0].children[0].children[0].summary.id, "xx-000004");
        assert_eq!(
            forest(
                &all,
                &view,
                &deps,
                None,
                true,
                Shelved::Hidden,
                None,
                &registry,
                now
            )[0]
            .children
            .len(),
            2
        );
        assert_eq!(
            forest(
                &all,
                &view,
                &deps,
                Some(&all[2].id),
                true,
                Shelved::Hidden,
                None,
                &registry,
                now
            )[0]
            .summary
            .id,
            "xx-000003"
        );
    }

    #[test]
    fn hierarchy_index_scan_order() {
        let registry = Registry::default();
        let root = task("xx-000001", None, Status::Todo);
        let c_hi = task("xx-000009", Some("xx-000001"), Status::Todo);
        let c_lo = task("xx-000002", Some("xx-000001"), Status::Todo);
        let grand = task("xx-000005", Some("xx-000009"), Status::Doing);
        let all = [root.clone(), c_hi.clone(), c_lo.clone(), grand.clone()];
        let refs: Vec<&Task> = all.iter().collect();
        let index = HierarchyIndex::new(&refs, &registry);

        assert_eq!(index.position(&root.id), Some(0));
        assert_eq!(index.position(&c_hi.id), Some(1));
        assert_eq!(index.position(&c_lo.id), Some(2));
        assert_eq!(index.position(&grand.id), Some(3));
        assert_eq!(index.position(&TaskId::parse("xx-0000ff").unwrap()), None);
        assert_eq!(index.children(0), &[1, 2]);
        assert_eq!(index.children(1), &[3]);
        assert_eq!(index.children(2), &[] as &[usize]);

        let view = view_of(&all, &["xx".to_string()], &registry);
        let kid_ids: Vec<String> = view
            .children(&root.id)
            .into_iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(kid_ids, ["xx-000009", "xx-000002"]);
        assert_eq!(
            view.get(&grand.id).map(|t| t.id.to_string()),
            Some("xx-000005".to_string())
        );
        assert!(view.get(&TaskId::parse("xx-0000ff").unwrap()).is_none());
    }

    #[test]
    fn hierarchy_index_counts_and_lanes() {
        let registry = Registry::default();
        let mut empty_lane = task("xx-000010", None, Status::Todo);
        empty_lane.lane = true;
        let mut active_lane = task("xx-000020", None, Status::Todo);
        active_lane.lane = true;
        let goal = task("xx-000021", Some("xx-000020"), Status::Todo);
        let todo_leaf = task("xx-000022", Some("xx-000021"), Status::Todo);
        let idea_leaf = task("xx-000023", Some("xx-000021"), Status::Idea);
        let shelved_leaf = task("xx-000024", Some("xx-000021"), Status::Shelved);
        let done_leaf = task("xx-000025", Some("xx-000021"), Status::Done);
        let dropped_leaf = task("xx-000026", Some("xx-000021"), Status::Dropped);
        let loose = task("xx-000030", None, Status::Todo);
        let all = [
            empty_lane.clone(),
            active_lane.clone(),
            goal.clone(),
            todo_leaf.clone(),
            idea_leaf,
            shelved_leaf,
            done_leaf,
            dropped_leaf,
            loose.clone(),
        ];
        let view = view_of(&all, &["xx".to_string()], &registry);

        assert_eq!(view.child_count(&empty_lane.id), 0);
        assert_eq!(view.open_descendant_count(&empty_lane.id), 0);
        assert_eq!(view.active_descendant_count(&empty_lane.id), 0);
        assert_eq!(
            view.lane_of(&empty_lane.id).map(|t| &t.id),
            Some(&empty_lane.id)
        );
        assert!(view.enclosing_lane(&empty_lane.id).is_none());

        // Under goal: todo, idea, shelved are open (3); todo and idea are active (2).
        assert_eq!(view.child_count(&goal.id), 5);
        assert_eq!(view.open_descendant_count(&goal.id), 3);
        assert_eq!(view.active_descendant_count(&goal.id), 2);
        // Under active_lane: goal (open+active) + 3 open (2 active) = 4 open, 3 active.
        assert_eq!(view.open_descendant_count(&active_lane.id), 4);
        assert_eq!(view.active_descendant_count(&active_lane.id), 3);

        assert_eq!(
            view.lane_of(&todo_leaf.id).map(|t| &t.id),
            Some(&active_lane.id)
        );
        assert_eq!(
            view.enclosing_lane(&todo_leaf.id).map(|t| &t.id),
            Some(&active_lane.id)
        );
        assert!(view.paused_lane(&todo_leaf.id).is_none());
        assert!(view.lane_of(&loose.id).is_none());
        assert!(view.enclosing_lane(&loose.id).is_none());
    }

    #[test]
    fn hierarchy_index_closed_ancestor() {
        let registry = Registry::default();
        let root = task("xx-000001", None, Status::Todo);
        let done_child = task("xx-000002", Some("xx-000001"), Status::Done);
        let todo_grand = task("xx-000003", Some("xx-000002"), Status::Todo);
        let all_todo = [root.clone(), done_child.clone(), todo_grand];
        let view_todo = view_of(&all_todo, &["xx".to_string()], &registry);
        assert_eq!(view_todo.open_descendant_count(&root.id), 1);
        assert_eq!(view_todo.active_descendant_count(&root.id), 1);

        let shelved_grand = task("xx-000003", Some("xx-000002"), Status::Shelved);
        let all_shelved = [root.clone(), done_child, shelved_grand];
        let view_shelved = view_of(&all_shelved, &["xx".to_string()], &registry);
        assert_eq!(view_shelved.open_descendant_count(&root.id), 1);
        assert_eq!(view_shelved.active_descendant_count(&root.id), 0);
    }

    #[test]
    fn hierarchy_index_aliases_and_orphans() {
        let mut registry = Registry::default();
        registry.aliases.insert("old".into(), "xx".into());
        let mut lane = task("xx-000001", None, Status::Todo);
        lane.lane = true;
        let aliased_child = task("xx-000002", Some("old-000001"), Status::Todo);
        let orphan = task("xx-000003", Some("xx-000099"), Status::Todo);
        let all = [lane.clone(), aliased_child.clone(), orphan.clone()];
        let view = view_of(&all, &["xx".to_string()], &registry);

        assert_eq!(view.child_count(&lane.id), 1);
        assert_eq!(view.children(&lane.id)[0].id, aliased_child.id);
        assert_eq!(
            view.lane_of(&aliased_child.id).map(|t| &t.id),
            Some(&lane.id)
        );
        assert!(view.get(&TaskId::parse("xx-000099").unwrap()).is_none());
        assert!(view.lane_of(&orphan.id).is_none());
        assert!(view.enclosing_lane(&orphan.id).is_none());

        let now = crate::time::parse("2026-01-01T00:00:00Z").unwrap();
        let deps = crate::review::Deps::from_states([]);
        let roots: Vec<String> = forest(
            &all,
            &view,
            &deps,
            None,
            false,
            Shelved::Hidden,
            None,
            &registry,
            now,
        )
        .into_iter()
        .map(|n| n.summary.id)
        .collect();
        assert_eq!(roots, ["xx-000001", "xx-000003"]);
    }

    #[test]
    fn hierarchy_index_cycles_terminate() {
        let registry = Registry::default();
        let a = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b = task("xx-000002", Some("xx-000001"), Status::Todo);
        let tail = task("xx-000003", Some("xx-000001"), Status::Doing);
        let mut self_loop = task("xx-000004", Some("xx-000004"), Status::Todo);
        self_loop.lane = true;
        let valid_root = task("xx-000010", None, Status::Todo);
        let valid_child = task("xx-000011", Some("xx-000010"), Status::Todo);
        let all = [
            a.clone(),
            b.clone(),
            tail.clone(),
            self_loop.clone(),
            valid_root.clone(),
            valid_child,
        ];
        let view = view_of(&all, &["xx".to_string()], &registry);

        let a_desc: HashSet<String> = view
            .descendants(&a.id)
            .into_iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(
            a_desc,
            HashSet::from(["xx-000002".to_string(), "xx-000003".to_string()])
        );
        assert_eq!(view.open_descendant_count(&a.id), 2);
        assert_eq!(view.active_descendant_count(&a.id), 2);
        assert!(view.descendants(&self_loop.id).is_empty());
        assert_eq!(view.open_descendant_count(&self_loop.id), 0);
        assert!(view.enclosing_lane(&self_loop.id).is_none());
        assert_eq!(
            view.lane_of(&self_loop.id).map(|t| &t.id),
            Some(&self_loop.id)
        );
        assert!(view.lane_of(&a.id).is_none());
        assert!(view.lane_of(&tail.id).is_none());

        let now = crate::time::parse("2026-01-01T00:00:00Z").unwrap();
        let deps = crate::review::Deps::from_states([]);
        let roots = forest(
            &all,
            &view,
            &deps,
            None,
            false,
            Shelved::Hidden,
            None,
            &registry,
            now,
        );
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].summary.id, "xx-000010");
        assert_eq!(roots[0].summary.open_descendant_count, 1);

        let rooted_cycle = forest(
            &all,
            &view,
            &deps,
            Some(&a.id),
            true,
            Shelved::Hidden,
            None,
            &registry,
            now,
        );
        assert_eq!(rooted_cycle.len(), 1);
        assert_eq!(rooted_cycle[0].summary.id, "xx-000001");
    }

    #[test]
    fn hierarchy_index_multi_project_scope() {
        let registry = Registry::default();
        let mut aa_lane = task("aa-000001", None, Status::Todo);
        aa_lane.lane = true;
        aa_lane.priority = 2;
        let aa_child = task("aa-000002", Some("aa-000001"), Status::Todo);

        let mut bb_lane = task("bb-000001", None, Status::Todo);
        bb_lane.lane = true;
        bb_lane.priority = 0;
        let bb_child = task("bb-000002", Some("bb-000001"), Status::Todo);

        // Interleave prefixes in `all` so per-project slots differ from `all` positions.
        let all = vec![
            aa_lane.clone(),
            bb_child.clone(),
            aa_child.clone(),
            bb_lane.clone(),
        ];
        let opened = vec!["aa".to_string(), "bb".to_string()];
        let view = view_of(&all, &opened, &registry);
        let now = crate::time::parse("2026-10-03T12:00:00Z").unwrap();
        let deps = crate::review::Deps::from_states([]);

        // Prime forest sorts roots globally by ready_order (bb_lane P0 before aa_lane P2).
        let prime_roots = forest(
            &all,
            &view,
            &deps,
            None,
            false,
            Shelved::Hidden,
            None,
            &registry,
            now,
        );
        let prime_ids: Vec<&str> = prime_roots.iter().map(|n| n.summary.id.as_str()).collect();
        assert_eq!(prime_ids, ["bb-000001", "aa-000001"]);
        assert_eq!(prime_roots[0].children[0].summary.id, "bb-000002");
        assert_eq!(prime_roots[1].children[0].summary.id, "aa-000002");

        // Tree retains per-project grouping in scope order (aa then bb).
        let aa_slice = [aa_lane.clone(), aa_child.clone()];
        let bb_slice = [bb_child.clone(), bb_lane.clone()];
        let mut tree_nodes = forest(
            &aa_slice,
            &view,
            &deps,
            None,
            false,
            Shelved::UnderShownParent,
            None,
            &registry,
            now,
        );
        tree_nodes.extend(forest(
            &bb_slice,
            &view,
            &deps,
            None,
            false,
            Shelved::UnderShownParent,
            None,
            &registry,
            now,
        ));
        let tree_ids: Vec<&str> = tree_nodes.iter().map(|n| n.summary.id.as_str()).collect();
        assert_eq!(tree_ids, ["aa-000001", "bb-000001"]);

        // Each lane's descendants come strictly from its own project, never all[slot].
        let claims = crate::claims::ClaimSnapshot::default();
        let halts = HashMap::new();
        let without = crate::needs::Without::default();
        let holds = crate::holds::HoldSnapshot::default();
        let vocab = crate::needs::Vocabulary::default();
        let vocabs = HashMap::from([("aa", &vocab), ("bb", &vocab)]);
        let mine = crate::holds::Mine::default();
        let dep_fn = |_: &TaskId| Some(true);
        let lane_rows = crate::lanes::build(&crate::lanes::Inputs {
            all: &all,
            hierarchy: &view,
            claims: &claims,
            registry: &registry,
            deps: &deps,
            dependency: &dep_fn,
            halts: &halts,
            cutoff: None,
            without: &without,
            holds: &holds,
            vocabularies: &vocabs,
            mine: &mine,
            now,
        });
        assert_eq!(lane_rows.len(), 2);
        assert_eq!(lane_rows[0].lane.id, "bb-000001");
        assert_eq!(
            lane_rows[0].pick.as_ref().map(|p| p.id.as_str()),
            Some("bb-000002")
        );
        assert_eq!(lane_rows[1].lane.id, "aa-000001");
        assert_eq!(
            lane_rows[1].pick.as_ref().map(|p| p.id.as_str()),
            Some("aa-000002")
        );
    }

    #[test]
    fn hierarchy_index_empty_projects() {
        let dir_empty = tempfile::tempdir().unwrap();
        let dir_nonempty = tempfile::tempdir().unwrap();
        let proj_empty = Project::init(dir_empty.path(), "emp", None).unwrap();
        let proj_nonempty = Project::init(dir_nonempty.path(), "full", None).unwrap();
        let added =
            crate::commands::add::blank(&proj_nonempty, "Only task".into(), Status::Todo, None)
                .unwrap();

        let mut registry = Registry::default();
        registry.register("emp", &proj_empty.root).unwrap();
        registry.register("full", &proj_nonempty.root).unwrap();

        // Opened empty + nonempty project
        let all = vec![added.clone()];
        let opened = vec!["emp".to_string(), "full".to_string()];
        let records = project_records(&all, &opened, &registry);
        assert!(records.contains_key("emp") && records["emp"].is_empty());
        assert_eq!(records["full"].len(), 1);
        let indexes = project_indexes(&records, &registry);
        assert_eq!(indexes.len(), 2);
        let view = HierarchyView::new(records, indexes, &registry);
        assert!(view.get(&TaskId::parse("emp-000001").unwrap()).is_none());
        assert_eq!(view.get(&added.id).map(|t| &t.id), Some(&added.id));

        // Empty tree JSON & pretty, and entirely empty prime JSON & pretty parity without panic
        let make_ctx = |proj: Project| crate::commands::ReadCtx {
            host: Arc::new(crate::config::HostConfig::default()),
            scope: crate::scope::Scope::Local(Box::new(proj)),
            registry: registry.clone(),
            warnings: Vec::new(),
            shorthand: crate::shorthand::Shorthand::new(dir_empty.path().to_path_buf()),
        };
        let tree_out = crate::commands::tree::run(
            make_ctx(proj_empty.clone()),
            None,
            false,
            crate::output::Format::Pretty,
        )
        .unwrap();
        let painter = crate::style::Painter::new(
            crate::style::ColorMode::Never,
            crate::output::Format::Pretty,
            false,
        );
        let layout = crate::table::Layout::default();
        let tree_json: serde_json::Value = serde_json::from_str(&crate::output::render(
            &tree_out,
            crate::output::Format::Json,
            &painter,
            &layout,
        ))
        .unwrap();
        assert_eq!(tree_json["nodes"], serde_json::json!([]));
        assert_eq!(
            crate::output::render(&tree_out, crate::output::Format::Pretty, &painter, &layout),
            ""
        );

        let prime_out = crate::commands::list::prime(
            make_ctx(proj_empty),
            false,
            Vec::new(),
            None,
            crate::output::Format::Pretty,
        )
        .unwrap();
        let prime_json: serde_json::Value = serde_json::from_str(&crate::output::render(
            &prime_out,
            crate::output::Format::Json,
            &painter,
            &layout,
        ))
        .unwrap();
        assert_eq!(prime_json["roadmap"], serde_json::json!([]));
        assert_eq!(prime_json["ready"], serde_json::json!([]));
        let prime_pretty =
            crate::output::render(&prime_out, crate::output::Format::Pretty, &painter, &layout);
        assert!(prime_pretty.starts_with("project emp\n"), "{prime_pretty}");
    }

    #[test]
    fn hierarchy_index_lane_self_is_not_pause() {
        let registry = Registry::default();
        let mut blocked_lane = task("aa-000001", None, Status::Blocked);
        blocked_lane.lane = true;
        blocked_lane.origin = Some(crate::model::Origin::User);
        let mut sub_goal = task("aa-000002", Some("aa-000001"), Status::Todo);
        sub_goal.origin = Some(crate::model::Origin::User);
        let step = task("aa-000003", Some("aa-000002"), Status::Todo);

        let all = vec![blocked_lane.clone(), sub_goal.clone(), step.clone()];
        let view = view_of(&all, &["aa".to_string()], &registry);

        assert_eq!(
            view.lane_of(&blocked_lane.id).map(|t| &t.id),
            Some(&blocked_lane.id)
        );
        assert!(view.enclosing_lane(&blocked_lane.id).is_none());
        assert!(view.paused_lane(&blocked_lane.id).is_none());
        assert_eq!(
            view.enclosing_lane(&sub_goal.id).map(|t| &t.id),
            Some(&blocked_lane.id)
        );
        assert_eq!(
            view.paused_lane(&sub_goal.id).map(|t| &t.id),
            Some(&blocked_lane.id)
        );

        // goals::attention must not double-count blocked_lane as its own pause, while sub_goal
        // inside blocked_lane sees blocked_lane once as its paused_lane.
        let now = crate::time::parse("2026-11-01T00:00:00Z").unwrap();
        let claims = crate::claims::ClaimSnapshot::default();
        let mut membership = crate::membership::Membership::new(&registry, &["aa".into()], &all);
        let lane_members = membership.members(&blocked_lane).unwrap();
        let lane_att = crate::goals::attention(
            &blocked_lane,
            &lane_members,
            &mut membership,
            &claims,
            &registry,
            now,
        )
        .unwrap()
        .unwrap();
        assert_eq!(lane_att.blocked.as_ref().unwrap().count, 1);
        assert!(lane_att.paused_lane.is_empty());

        let sub_members = membership.members(&sub_goal).unwrap();
        let sub_att = crate::goals::attention(
            &sub_goal,
            &sub_members,
            &mut membership,
            &claims,
            &registry,
            now,
        )
        .unwrap()
        .unwrap();
        assert_eq!(sub_att.blocked.as_ref().unwrap().count, 1);
        assert_eq!(sub_att.paused_lane, vec!["aa-000001".to_string()]);
    }

    #[test]
    fn hierarchy_index_ready_and_filter_reuse() {
        let dir_aa = tempfile::tempdir().unwrap();
        let dir_bb = tempfile::tempdir().unwrap();
        let proj_aa = Project::init(dir_aa.path(), "aa", None).unwrap();
        let proj_bb = Project::init(dir_bb.path(), "bb", None).unwrap();
        let mut registry = Registry::default();
        registry.register("aa", &proj_aa.root).unwrap();
        registry.register("bb", &proj_bb.root).unwrap();

        let mut paused_lane = task("aa-000001", None, Status::Blocked);
        paused_lane.lane = true;
        let paused_step = task("aa-000002", Some("aa-000001"), Status::Todo);
        let goal = task("aa-000003", None, Status::Todo);
        let ready_step = task("aa-000004", Some("aa-000003"), Status::Todo);
        let mut deferred_step = task("aa-000005", Some("aa-000003"), Status::Todo);
        deferred_step.defer = Some(crate::defer::Defer::parse("2099-01-01").unwrap());
        let parked_step = task("aa-000006", Some("aa-000003"), Status::Todo);
        let closeout_goal = task("aa-000007", None, Status::Todo);
        let done_child = task("aa-000008", Some("aa-000007"), Status::Done);

        let all = vec![
            paused_lane.clone(),
            paused_step.clone(),
            goal.clone(),
            ready_step.clone(),
            deferred_step.clone(),
            parked_step.clone(),
            closeout_goal.clone(),
            done_child.clone(),
        ];
        let park = crate::claims::Park {
            owner: "o".into(),
            session: "s:a".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-10-01T00:00:00Z".into(),
            next_step: "resume".into(),
            waiting_on: crate::claims::WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "Parked".into(),
        };
        let claims = crate::claims::ClaimSnapshot::from_parts(
            BTreeMap::new(),
            BTreeMap::from([(parked_step.id.to_string(), park)]),
            BTreeMap::new(),
        );
        let now = crate::time::parse("2026-10-03T12:00:00Z").unwrap();
        let deps = crate::review::Deps::from_states([]);
        let opened = vec!["aa".to_string(), "bb".to_string()];

        reset_test_index_counts();
        let mut membership = crate::membership::Membership::new(&registry, &opened, &all);
        let view = membership.hierarchy_view(&all, &opened);
        // Exactly one index per captured project scan (aa + empty bb = 2), no union index.
        assert_eq!(test_index_counts().0, 2);

        let mut ctx = crate::commands::ReadCtx {
            host: Arc::new(crate::config::HostConfig::default()),
            scope: crate::scope::Scope::All {
                projects: vec![proj_aa, proj_bb],
                members: Vec::new(),
                group: None,
            },
            registry: registry.clone(),
            warnings: Vec::new(),
            shorthand: crate::shorthand::Shorthand::new(dir_aa.path().to_path_buf()),
        };

        // Fields::of_task + TaskFilter::resolve_under (--under goal)
        let mut under_filter = crate::filter::TaskFilter::parse(
            &crate::cli::FilterArgs {
                under: Some(goal.id.to_string()),
                ..crate::cli::FilterArgs::default()
            },
            &[],
            &registry,
            &ctx.shorthand,
        )
        .unwrap();
        under_filter.resolve_under(&view, &registry).unwrap();
        assert!(!under_filter.matches(&crate::filter::Fields::of_task(
            &goal, &view, &deps, &claims, &registry, now
        )));
        assert!(under_filter.matches(&crate::filter::Fields::of_task(
            &ready_step,
            &view,
            &deps,
            &claims,
            &registry,
            now
        )));

        // ready_tasks + warn_paused + parked::candidates
        let snapshots = HashMap::new();
        let picked = crate::commands::list::ready_tasks(
            &mut ctx,
            &all,
            &view,
            &deps,
            &claims,
            &snapshots,
            &crate::filter::TaskFilter::default(),
            now,
        )
        .unwrap();
        let ready_ids: Vec<String> = picked.tasks.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(ready_ids, ["aa-000004", "aa-000006"]);
        assert_eq!(picked.deferred.len(), 1);
        assert_eq!(picked.deferred[0].id, deferred_step.id);
        assert_eq!(picked.paused.len(), 1);
        assert_eq!(picked.paused[0].id, paused_step.id);

        crate::commands::list::warn_paused(
            &mut ctx,
            &view,
            picked.paused,
            &snapshots,
            &crate::needs::Without::default(),
            None,
            &claims,
        );
        assert!(
            ctx.warnings
                .iter()
                .any(|w| w.contains("1 task(s) hidden by paused lane aa-000001")),
            "{:?}",
            ctx.warnings
        );

        let cands = crate::commands::parked::candidates(
            &mut ctx,
            &all,
            &view,
            &deps,
            &claims,
            &under_filter,
            now,
        )
        .unwrap();
        assert_eq!(cands.tasks.len(), 1);
        assert_eq!(cands.tasks[0].id, parked_step.id);

        // forest + TaskSummary::of + closeout check + Membership reuse
        let nodes = forest(
            &all,
            &view,
            &deps,
            None,
            false,
            Shelved::Hidden,
            Some(&claims),
            &registry,
            now,
        );
        assert!(!nodes.is_empty());
        let summary = TaskSummary::of(&closeout_goal, &view, &deps, Some(&claims), &registry, now);
        assert_eq!(summary.child_count, 1);
        assert_eq!(summary.open_descendant_count, 0);
        assert_eq!(membership.members(&goal).unwrap().len(), 3);

        let (constructions, lookups) = test_index_counts();
        assert_eq!(
            constructions, 2,
            "no per-row or extra index constructions across read paths"
        );
        assert!(lookups > 0, "expected indexed lookups to be recorded");
    }
}
