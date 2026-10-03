use crate::error::Result;
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use crate::scope::{self, Origin};
use std::collections::HashSet;

pub struct HaltSnapshot {
    authority: Project,
    halts: Vec<Task>,
    allowed: HashSet<TaskId>,
}

pub fn snapshot(project: &Project, registry: &Registry, local: &[Task]) -> Result<HaltSnapshot> {
    let authority = match registry.project_root(&project.prefix) {
        Some(_) => scope::open_registered(registry, &project.prefix, Origin::Prefix)?,
        None => project.clone(),
    };
    let records = if registry.project_root(&project.prefix).is_some() {
        authority.scan()?
    } else {
        local.to_vec()
    };
    let mut halts: Vec<Task> = records
        .into_iter()
        .filter(|task| task.tags.iter().any(|tag| tag == "halt") && task.status.is_open())
        .collect();
    halts.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    let allowed = halts
        .iter()
        .flat_map(|halt| reachable(halt, local))
        .collect();
    Ok(HaltSnapshot {
        authority,
        halts,
        allowed,
    })
}

impl HaltSnapshot {
    pub fn halts(&self) -> &[Task] {
        &self.halts
    }
    pub fn authority(&self) -> &Project {
        &self.authority
    }
    pub fn allows(&self, target: &Task) -> bool {
        target.status == Status::Doing
            || self.halts.is_empty()
            || self.allowed.contains(&target.id)
            || target.priority <= self.halts[0].priority
    }
    pub fn blocking(&self, target: &Task) -> Vec<&Task> {
        if self.allows(target) {
            return Vec::new();
        }
        self.halts
            .iter()
            .filter(|halt| halt.priority < target.priority)
            .collect()
    }
}

fn reachable(halt: &Task, local: &[Task]) -> HashSet<TaskId> {
    let mut pending = vec![halt.id.clone()];
    pending.extend(
        halt.depends
            .iter()
            .filter(|id| id.prefix == halt.id.prefix)
            .cloned(),
    );
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if id != halt.id
            && let Some(task) = local.iter().find(|task| task.id == id)
        {
            pending.extend(
                task.depends
                    .iter()
                    .filter(|dep| dep.prefix == id.prefix)
                    .cloned(),
            );
        }
        pending.extend(
            local
                .iter()
                .filter(|task| task.parent.as_ref() == Some(&id))
                .map(|task| task.id.clone()),
        );
    }
    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use std::path::PathBuf;

    fn task(hex: &str) -> Task {
        Task {
            id: TaskId::parse(&format!("sci-{hex}")).unwrap(),
            title: hex.into(),
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            needs: vec![],
            every: None,
            defer: None,
            owner: None,
            created: "2026-09-30T00:00:00Z".into(),
            updated: "2026-09-30T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn authority_root_dependencies_and_local_children_are_reachable() {
        let mut halt = task("000001");
        let mut child = task("000002");
        let dependency = task("000003");
        let mut stale_halt = halt.clone();
        halt.depends.push(dependency.id.clone());
        child.parent = Some(halt.id.clone());
        stale_halt.depends.clear();
        let ids = reachable(&halt, &[stale_halt, child.clone(), dependency.clone()]);
        assert!(ids.contains(&halt.id));
        assert!(ids.contains(&child.id));
        assert!(ids.contains(&dependency.id));
        let ids_without_local_halt = reachable(&halt, &[child.clone(), dependency.clone()]);
        assert!(ids_without_local_halt.contains(&dependency.id));
        assert!(ids_without_local_halt.contains(&child.id));
    }

    #[test]
    fn mixed_links_cross_two_generations_without_following_other_projects() {
        let mut halt = task("000001");
        let mut child = task("000002");
        let mut dependency = task("000003");
        let mut grandchild = task("000004");
        let foreign = TaskId::parse("fam-000001").unwrap();
        child.parent = Some(halt.id.clone());
        child.depends = vec![dependency.id.clone(), foreign.clone()];
        dependency.depends.push(halt.id.clone());
        grandchild.parent = Some(dependency.id.clone());
        halt.depends.push(foreign.clone());
        let ids = reachable(
            &halt,
            &[child.clone(), dependency.clone(), grandchild.clone()],
        );
        assert_eq!(ids.len(), 4);
        assert!(ids.contains(&grandchild.id));
        assert!(!ids.contains(&foreign));
    }

    #[test]
    fn every_open_halt_status_blocks_and_closed_statuses_do_not() {
        let project = Project {
            root: PathBuf::new(),
            prefix: "sci".into(),
            color: None,
            spec_dirs: vec![],
            plan_dirs: vec![],
            tags: None,
            feedback: None,
            needs: crate::needs::Vocabulary::new(),
            attachments_max_bytes: crate::attachments::DEFAULT_MAX_BYTES,
        };
        let target = task("000002");
        for status in Status::ALL {
            let mut halt = task("000001");
            halt.status = status;
            halt.priority = 0;
            halt.tags.push("halt".into());
            let view = snapshot(&project, &Registry::default(), &[halt, target.clone()]).unwrap();
            assert_eq!(view.allows(&target), !status.is_open(), "{status:?}");
        }
    }

    #[test]
    fn local_priority_parent_and_dependency_edits_make_work_eligible() {
        let project = Project {
            root: PathBuf::new(),
            prefix: "sci".into(),
            color: None,
            spec_dirs: vec![],
            plan_dirs: vec![],
            tags: None,
            feedback: None,
            needs: crate::needs::Vocabulary::new(),
            attachments_max_bytes: crate::attachments::DEFAULT_MAX_BYTES,
        };
        let mut halt = task("000001");
        halt.priority = 0;
        halt.tags.push("halt".into());
        let mut target = task("000002");
        let registry = Registry::default();
        let allowed = |halt: &Task, target: &Task| {
            snapshot(&project, &registry, &[halt.clone(), target.clone()])
                .unwrap()
                .allows(target)
        };
        assert!(!allowed(&halt, &target));
        target.priority = 0;
        assert!(allowed(&halt, &target));
        target.priority = 2;
        target.parent = Some(halt.id.clone());
        assert!(allowed(&halt, &target));
        target.parent = None;
        halt.depends.push(target.id.clone());
        assert!(allowed(&halt, &target));
    }
}
