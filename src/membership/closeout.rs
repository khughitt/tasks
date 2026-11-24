//! Evidence for a fresh goal completion and for prime's closeout invitation.
use super::{Membership, Walk};
use crate::dependency::{self, Verdict};
use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Display;

#[derive(Default)]
pub(crate) struct Closeout {
    pub(crate) direct: Vec<TaskId>,
    pub(crate) open: BTreeSet<TaskId>,
    pub(crate) missing: BTreeSet<(TaskId, TaskId)>,
    has_foreign_hold: bool,
}

fn joined<T: Display>(items: impl IntoIterator<Item = T>, separator: &str) -> String {
    items
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

impl Closeout {
    fn frontier(&self) -> String {
        joined(
            self.missing
                .iter()
                .map(|(source, target)| format!("{source} -> {target}")),
            ", ",
        )
    }

    pub(crate) fn refusal(&self, goal: &TaskId) -> Option<Error> {
        let id = goal.to_string();
        if !self.direct.is_empty() {
            return Some(Error::OpenDependencies(id, joined(&self.direct, ", ")));
        }
        if !self.missing.is_empty() {
            let mut detail = self.frontier();
            if !self.open.is_empty() {
                detail.push_str(&format!("; open members: {}", joined(&self.open, ", ")));
            }
            return Some(Error::UnresolvedGoalMembers(id, detail));
        }
        if self.open.is_empty() {
            return None;
        }
        let detail = joined(&self.open, ", ");
        Some(if self.has_foreign_hold {
            Error::OpenGoalMembers(id, detail)
        } else {
            Error::OpenDescendants(id, detail)
        })
    }

    pub(crate) fn waived(
        &self,
        goal: &TaskId,
        registry: &crate::registry::Registry,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.direct.is_empty() {
            warnings.push(format!(
                "{goal} forced completion: direct dependencies: {}",
                joined(
                    self.direct
                        .iter()
                        .map(|id| registry.canonical_id(id))
                        .collect::<BTreeSet<_>>(),
                    ", "
                )
            ));
        }
        if !self.missing.is_empty() {
            warnings.push(format!(
                "{goal} forced completion: unresolved goal members: {}",
                self.frontier()
            ));
        }
        if !self.open.is_empty() {
            warnings.push(format!(
                "{goal} forced completion: open members: {}",
                joined(&self.open, ", ")
            ));
        }
        warnings
    }

    /// Either invite the goal, or explain holds other than unfinished member work.
    pub(crate) fn offer(&self, goal: &TaskId, warnings: &mut Vec<String>) -> bool {
        if !self.missing.is_empty() {
            warnings.push(format!(
                "{goal} has unresolved goal members: {}",
                self.frontier()
            ));
            return false;
        }
        if !self.open.is_empty() {
            return false;
        }
        if self.direct.is_empty() {
            return true;
        }
        warnings.push(format!(
            "{goal} has no open children or pieces but is held by open prerequisites: {}; it joins closeout when they close",
            joined(&self.direct, ", ")
        ));
        false
    }
}

enum Visit {
    Enter(Box<Task>),
    Leave(TaskId),
}

impl Membership<'_> {
    pub(crate) fn closeout(&mut self, goal: &Task) -> Result<Closeout> {
        let registry = self.registry;
        let root = registry.canonical_id(&goal.id);
        let mut evidence = Closeout::default();
        let mut stack = vec![Visit::Enter(Box::new(goal.clone()))];
        let mut path = Vec::<TaskId>::new();
        let mut active = HashMap::new();
        let mut finished = HashSet::new();
        while let Some(visit) = stack.pop() {
            let task = match visit {
                Visit::Leave(id) => {
                    active.remove(&id);
                    path.pop();
                    finished.insert(id);
                    continue;
                }
                Visit::Enter(task) => task,
            };
            let id = registry.canonical_id(&task.id);
            if finished.contains(&id) || task.status == Status::Dropped {
                continue;
            }
            if let Some(&start) = active.get(&id) {
                let mut cycle = path[start..].to_vec();
                let least = cycle.iter().enumerate().min_by_key(|(_, k)| *k).unwrap().0;
                cycle.rotate_left(least);
                cycle.push(cycle[0].clone());
                return Err(Error::GoalMembershipCycle(
                    root.to_string(),
                    joined(&cycle, " -> "),
                ));
            }
            if id != root && task.status.is_open() {
                evidence.open.insert(id.clone());
            }
            active.insert(id.clone(), path.len());
            path.push(id.clone());
            stack.push(Visit::Leave(id.clone()));
            let (mut reached, missing) = self.neighbors(&task, Walk::Completion)?;
            evidence
                .missing
                .extend(missing.into_iter().map(|key| (id.clone(), key)));
            reached.sort_by_key(|t| registry.canonical_id(&t.id));
            stack.extend(reached.into_iter().rev().map(|t| Visit::Enter(Box::new(t))));
        }

        // The unique local parent chain determines whether an open member also
        // has an effective local-only path, independently of DFS visit order.
        if !evidence.open.is_empty() {
            let local = self.projects[&root.prefix]
                .as_ref()
                .expect("goal scan is seeded");
            let mut local_open = BTreeSet::new();
            let mut pending = vec![root.clone()];
            let mut seen = HashSet::new();
            while let Some(parent) = pending.pop() {
                if !seen.insert(parent.clone()) {
                    continue;
                }
                for &slot in local.index.children_of_id(&parent) {
                    let child = &local.tasks[slot];
                    if child.status != Status::Dropped {
                        let id = registry.canonical_id(&child.id);
                        if child.status.is_open() {
                            local_open.insert(id.clone());
                        }
                        pending.push(id);
                    }
                }
            }
            evidence.has_foreign_hold = !evidence.open.is_subset(&local_open);
        }
        for dep in &goal.depends {
            let found = dependency::resolve(registry, dep, |key| self.lookup(key))?;
            if dependency::verdict(found.as_ref().filter(|t| t.status != Status::Moved))
                != Verdict::Satisfied
            {
                evidence.direct.push(dep.clone());
            }
        }
        Ok(evidence)
    }
}
