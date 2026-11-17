//! Goal membership across projects for one command (docs/task-model.md): every record
//! reachable from a goal by repeatedly following its `parent` children (same project) and
//! its declared pieces (resolved through moves), excluding the goal itself.

use crate::error::Result;
use crate::hierarchy::HierarchyIndex;
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

struct ProjectScan {
    tasks: Vec<Task>,
    index: Arc<HierarchyIndex>,
}

impl ProjectScan {
    fn new(tasks: Vec<Task>, registry: &Registry) -> Self {
        let refs: Vec<&Task> = tasks.iter().collect();
        let index = Arc::new(HierarchyIndex::new(&refs, registry));
        ProjectScan { tasks, index }
    }
}

/// Goal membership across projects for one command: "every record reachable from the goal
/// by repeatedly following two edges, excluding the goal itself: 1. `parent` children
/// (same project), and 2. declared pieces, each resolved through moves" (spec §6.1).
pub struct Membership<'r> {
    registry: &'r Registry,
    /// Canonical prefix → that project's records and hierarchy index; `None` when unreachable.
    projects: BTreeMap<String, Option<ProjectScan>>,
    memo: HashMap<TaskId, Vec<Task>>,
    reached: BTreeSet<String>,
    warnings: Vec<String>,
}

impl<'r> Membership<'r> {
    /// Seeds the cache with every opened in-scope prefix (an empty project still gets an
    /// entry, and the caller's own unregistered local project is never treated as foreign),
    /// then fills those entries from `scanned`, grouped by canonical prefix.
    pub fn new(registry: &'r Registry, opened: &[String], scanned: &[Task]) -> Self {
        let mut grouped: BTreeMap<String, Vec<Task>> = opened
            .iter()
            .map(|prefix| (registry.canonical_prefix(prefix).to_string(), Vec::new()))
            .collect();
        for task in scanned {
            let prefix = registry.canonical_id(&task.id).prefix;
            grouped.entry(prefix).or_default().push(task.clone());
        }
        let projects = grouped
            .into_iter()
            .map(|(prefix, tasks)| (prefix, Some(ProjectScan::new(tasks, registry))))
            .collect();
        Membership {
            registry,
            projects,
            memo: HashMap::new(),
            reached: BTreeSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Deduplicated by canonical id, with a visited set; memoized per goal.
    pub fn members(&mut self, goal: &Task) -> Result<Vec<Task>> {
        let root = self.registry.canonical_id(&goal.id);
        if let Some(members) = self.memo.get(&root) {
            return Ok(members.clone());
        }
        let mut visited = BTreeSet::from([root.clone()]);
        let mut queue = std::collections::VecDeque::from([goal.clone()]);
        let mut members = Vec::new();
        let registry = self.registry;
        while let Some(record) = queue.pop_front() {
            let canonical_record = registry.canonical_id(&record.id);
            let prefix = canonical_record.prefix.clone();
            let _ = self.project(&prefix)?;
            let kids: Vec<Task> = match self.projects[&prefix].as_ref() {
                Some(entry) => entry
                    .index
                    .children_of_id(&canonical_record)
                    .iter()
                    .map(|&slot| entry.tasks[slot].clone())
                    .collect(),
                None => Vec::new(),
            };
            let mut reached = kids;
            for piece in &record.pieces {
                // Opening the piece's project first records why an unreachable one is
                // missing, and loads the scan its children are read from.
                self.project(&registry.canonical_id(piece).prefix)?;
                if let Some(found) =
                    crate::dependency::resolve(registry, piece, |id| self.lookup(id))?
                {
                    reached.push(found);
                }
            }
            for task in reached {
                if visited.insert(self.registry.canonical_id(&task.id)) {
                    members.push(task.clone());
                    queue.push_back(task);
                }
            }
        }
        for member in &members {
            self.reached
                .insert(self.registry.canonical_id(&member.id).prefix);
        }
        self.memo.insert(root, members.clone());
        Ok(members)
    }

    /// One record by canonical id: the cached scan when its project is loaded, else a
    /// single registered read (never a whole-project scan).
    pub fn lookup(&mut self, id: &TaskId) -> Result<Option<Task>> {
        let id = self.registry.canonical_id(id);
        match self.projects.get(&id.prefix) {
            Some(Some(entry)) => Ok(entry
                .index
                .position(&id)
                .map(|slot| entry.tasks[slot].clone())),
            Some(None) => Ok(None),
            None => crate::resolve::resolve_registered_canonical(self.registry, &id),
        }
    }

    /// A project's records, scanning it once; None when it is unreachable.
    pub fn project(&mut self, prefix: &str) -> Result<Option<&[Task]>> {
        let prefix = self.registry.canonical_prefix(prefix).to_string();
        if !self.projects.contains_key(&prefix) {
            let scan = self.open(&prefix)?;
            self.projects.insert(prefix.clone(), scan);
        }
        Ok(self.projects[&prefix]
            .as_ref()
            .map(|entry| entry.tasks.as_slice()))
    }

    /// Cached per-project hierarchy index, opening through [`Self::project`] when needed.
    pub fn hierarchy(&mut self, prefix: &str) -> Result<Option<Arc<HierarchyIndex>>> {
        let canonical = self.registry.canonical_prefix(prefix).to_string();
        let _ = self.project(&canonical)?;
        Ok(self.projects[&canonical]
            .as_ref()
            .map(|entry| Arc::clone(&entry.index)))
    }

    /// Opens and scans a project not in scope. Each warning is pushed once per prefix,
    /// because the result is cached.
    fn open(&mut self, prefix: &str) -> Result<Option<ProjectScan>> {
        let Some(dir) = crate::scope::config_dir(self.registry, prefix) else {
            self.warnings
                .push(format!("project {prefix} is not registered on this host"));
            return Ok(None);
        };
        if !crate::scope::has_config(&dir)? {
            let root = self.registry.project_root(prefix).unwrap_or(&dir);
            self.warnings.push(format!(
                "project {prefix} at {} is unreachable",
                root.display()
            ));
            return Ok(None);
        }
        let project =
            crate::scope::open_registered(self.registry, prefix, crate::scope::Origin::Prefix)?;
        let scan = project
            .scan()
            .map_err(|error| error.in_project(&project.prefix))?;
        Ok(Some(ProjectScan::new(scan, self.registry)))
    }

    /// Canonical prefixes of every member returned so far.
    pub fn reached_prefixes(&self) -> BTreeSet<String> {
        self.reached.clone()
    }

    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }
}

#[cfg(test)]
impl<'r> Membership<'r> {
    /// Preloaded scans per canonical prefix; `None` marks an unreachable project.
    pub(crate) fn preloaded(
        registry: &'r Registry,
        projects: Vec<(&str, Option<Vec<Task>>)>,
    ) -> Self {
        Membership {
            registry,
            projects: projects
                .into_iter()
                .map(|(prefix, scan)| {
                    (
                        prefix.to_string(),
                        scan.map(|tasks| ProjectScan::new(tasks, registry)),
                    )
                })
                .collect(),
            memo: HashMap::new(),
            reached: BTreeSet::new(),
            warnings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::tests::task;
    use crate::model::Status;

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    fn ids(members: &[Task]) -> BTreeSet<String> {
        members.iter().map(|task| task.id.to_string()).collect()
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn membership_closure_children_and_pieces() {
        let registry = Registry::default();
        let goal = task("aa-000000", None, Status::Todo);
        let mut child = task("aa-000001", Some("aa-000000"), Status::Todo);
        child.depends = vec![id("bb-000000")];
        child.pieces = vec![id("bb-000000")];
        let mut piece = task("bb-000000", None, Status::Todo);
        let mut piece_child = task("bb-000001", Some("bb-000000"), Status::Todo);
        piece_child.depends = vec![id("cc-000000")];
        piece_child.pieces = vec![id("cc-000000")];
        piece.depends = vec![];
        let deep = task("cc-000000", None, Status::Done);
        let mut membership = Membership::preloaded(
            &registry,
            vec![
                ("aa", Some(vec![goal.clone(), child])),
                ("bb", Some(vec![piece, piece_child])),
                ("cc", Some(vec![deep])),
            ],
        );
        let members = membership.members(&goal).unwrap();
        assert_eq!(
            ids(&members),
            set(&["aa-000001", "bb-000000", "bb-000001", "cc-000000"])
        );
        assert_eq!(members.len(), 4);
        assert_eq!(membership.reached_prefixes(), set(&["aa", "bb", "cc"]));
    }

    #[test]
    fn membership_dedup_and_mixed_cycle_terminates() {
        let registry = Registry::default();
        let mut goal = task("aa-000000", None, Status::Todo);
        goal.depends = vec![id("bb-000000")];
        goal.pieces = vec![id("bb-000000")];
        let mut child = task("aa-000001", Some("aa-000000"), Status::Todo);
        child.depends = vec![id("bb-000000")];
        child.pieces = vec![id("bb-000000")];
        let mut piece = task("bb-000000", None, Status::Todo);
        piece.depends = vec![id("aa-000000")];
        piece.pieces = vec![id("aa-000000")];
        let mut membership = Membership::preloaded(
            &registry,
            vec![
                ("aa", Some(vec![goal.clone(), child])),
                ("bb", Some(vec![piece])),
            ],
        );
        let members = membership.members(&goal).unwrap();
        assert_eq!(ids(&members), set(&["aa-000001", "bb-000000"]));
        assert_eq!(members.len(), 2);
    }

    #[test]
    fn membership_resolves_moved_piece() {
        let registry = Registry::default();
        let mut goal = task("aa-000000", None, Status::Todo);
        goal.depends = vec![id("bb-000000")];
        goal.pieces = vec![id("bb-000000")];
        let mut tombstone = task("bb-000000", None, Status::Moved);
        tombstone.moved_to = Some(id("cc-000005"));
        let mut successor = task("cc-000005", None, Status::Todo);
        successor.moved_from = Some(id("bb-000000"));
        let mut membership = Membership::preloaded(
            &registry,
            vec![
                ("aa", Some(vec![goal.clone()])),
                ("bb", Some(vec![tombstone])),
                ("cc", Some(vec![successor])),
            ],
        );
        let members = membership.members(&goal).unwrap();
        assert_eq!(ids(&members), set(&["cc-000005"]));
    }

    #[test]
    fn membership_unresolved_piece_is_not_member() {
        let registry = Registry::default();
        let mut goal = task("aa-000000", None, Status::Todo);
        goal.depends = vec![id("bb-000009"), id("zz-000001")];
        goal.pieces = vec![id("bb-000009"), id("zz-000001")];
        let mut membership = Membership::preloaded(
            &registry,
            vec![
                ("aa", Some(vec![goal.clone()])),
                ("bb", Some(vec![task("bb-000000", None, Status::Todo)])),
                ("zz", None),
            ],
        );
        assert!(membership.members(&goal).unwrap().is_empty());
    }
}
