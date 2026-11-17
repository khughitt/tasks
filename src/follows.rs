//! Explicit continuation links; these never participate in scheduling.
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::resolve::Resolver;

fn validate_list(registry: &Registry, task: &Task) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for id in &task.follows {
        let canonical = registry.canonical_id(id);
        if canonical == registry.canonical_id(&task.id) {
            return Err(Error::Validation(format!(
                "{} cannot follow itself",
                task.id
            )));
        }
        if !seen.insert(canonical.clone()) {
            return Err(Error::Validation(format!(
                "duplicate follows target {canonical}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn append(
    registry: &Registry,
    resolver: &Resolver<'_>,
    task: &mut Task,
    ids: &[TaskId],
) -> Result<Vec<String>> {
    let existing = task.follows.len();
    let mut warnings = Vec::new();
    for id in ids {
        if *id == registry.canonical_id(&task.id) {
            return Err(Error::Validation(format!("{id} cannot follow itself")));
        }
        let found = resolver
            .resolve_canonical_task(id)?
            .ok_or_else(|| Error::UnresolvableId(id.to_string()))?;
        crate::tombstone::refuse_reference(&found, "follows task")?;
        match task
            .follows
            .iter()
            .position(|stored| registry.canonical_id(stored) == *id)
        {
            Some(index) if index < existing => {
                let warning = format!("{} already follows {id}; unchanged", task.id);
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            Some(_) => {}
            None => task.follows.push(id.clone()),
        }
    }
    validate_list(registry, task)?;
    Ok(warnings)
}

pub(crate) fn remove(registry: &Registry, task: &mut Task, ids: &[TaskId]) -> Result<()> {
    for id in ids {
        let before = task.follows.len();
        task.follows
            .retain(|stored| registry.canonical_id(stored) != *id);
        if task.follows.len() == before {
            return Err(Error::Validation(format!(
                "{} does not follow {id}",
                task.id
            )));
        }
    }
    validate_list(registry, task)
}

pub(crate) fn validate_edit(
    registry: &Registry,
    resolver: &Resolver<'_>,
    original: &Task,
    edited: &Task,
) -> Result<()> {
    if original.follows == edited.follows {
        return Ok(());
    }
    validate_list(registry, edited)?;
    for id in &edited.follows {
        let canonical = registry.canonical_id(id);
        if !original
            .follows
            .iter()
            .any(|stored| registry.canonical_id(stored) == canonical)
        {
            let found = resolver
                .resolve_canonical_task(&canonical)?
                .ok_or_else(|| Error::UnresolvableId(canonical.to_string()))?;
            crate::tombstone::refuse_reference(&found, "follows task")?;
        }
    }
    Ok(())
}

pub(crate) struct IdentitySet {
    pub ids: std::collections::BTreeSet<TaskId>,
    pub warning: Option<String>,
}

/// Only the focal task's reciprocal move chain can extend its identity.
pub(crate) fn identity_set(
    registry: &Registry,
    focal: &Task,
    mut lookup: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> IdentitySet {
    let mut identity = IdentitySet {
        ids: std::collections::BTreeSet::from([registry.canonical_id(&focal.id)]),
        warning: None,
    };
    // Forward and backward paths are disjoint in a valid chain. Seen ids both detect
    // loops and ensure each history record is looked up at most once.
    for forward in [true, false] {
        let mut current = focal.clone();
        loop {
            let pointer = if forward {
                &current.moved_to
            } else {
                &current.moved_from
            };
            let Some(pointer) = pointer else { break };
            let id = registry.canonical_id(pointer);
            let next = if identity.ids.contains(&id) {
                None
            } else {
                lookup(&id).ok().flatten()
            };
            let valid = next.as_ref().is_some_and(|next| {
                let back = if forward {
                    &next.moved_from
                } else {
                    &next.moved_to
                };
                registry.canonical_id(&next.id) == id
                    && back.as_ref().is_some_and(|back| {
                        registry.canonical_id(back) == registry.canonical_id(&current.id)
                    })
            });
            if !valid {
                identity.warning = Some(format!(
                    "{} follows move-history coverage is incomplete",
                    focal.id
                ));
                return identity;
            }
            identity.ids.insert(id);
            current = next.unwrap();
        }
    }
    identity
}

pub(crate) fn followers<'a>(
    registry: &Registry,
    all: &'a [Task],
    identity: &IdentitySet,
) -> Vec<&'a Task> {
    let mut found: Vec<_> = all
        .iter()
        .filter(|task| {
            task.status != crate::model::Status::Moved
                && !identity.ids.contains(&registry.canonical_id(&task.id))
                && task
                    .follows
                    .iter()
                    .any(|id| identity.ids.contains(&registry.canonical_id(id)))
        })
        .collect();
    found.sort_by(|a, b| crate::query::ready_order(a, b));
    let mut seen = std::collections::BTreeSet::new();
    found.retain(|task| seen.insert(registry.canonical_id(&task.id)));
    found
}

#[cfg(test)]
mod inverse_tests {
    use super::*;
    use std::path::Path;
    fn rec(id: &str, extra: &str) -> Task {
        let status = if extra.contains("moved_to:") {
            "moved"
        } else {
            "todo"
        };
        crate::history::fixture(&format!("---\nid: {id}\ntitle: T\nstatus: {status}\npriority: 2\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\ndepends: []\ntags: []\n{extra}---\n"), "fixture").unwrap()
    }
    fn id(s: &str) -> TaskId {
        TaskId::parse(s).unwrap()
    }
    #[test]
    fn follows_inverse_ordinary_never_looks_up_unrelated_edges() {
        let r = Registry::default();
        let focal = rec("aa-000001", "");
        let identity = identity_set(&r, &focal, |_| {
            panic!("ordinary focal must make zero lookups")
        });
        let tasks = vec![focal, rec("bb-000002", "follows: [zz-000001, aa-000001]\n")];
        assert_eq!(
            followers(&r, &tasks, &identity)
                .iter()
                .map(|t| &t.id)
                .collect::<Vec<_>>(),
            vec![&id("bb-000002")]
        );
        assert!(identity.warning.is_none());
    }
    #[test]
    fn follows_inverse_history_is_reciprocal_bounded_and_alias_aware() {
        let mut r = Registry::default();
        r.register("aa", Path::new("/tmp/aa")).unwrap();
        r.aliases.insert("oldaa".into(), "aa".into());
        let first = rec("aa-000001", "moved_to: bb-000002\n");
        let middle = rec(
            "bb-000002",
            "moved_from: oldaa-000001\nmoved_to: cc-000003\n",
        );
        let last = rec("cc-000003", "moved_from: bb-000002\n");
        let chain = vec![first.clone(), middle, last.clone()];
        for focal in [&first, &last] {
            let mut calls = Vec::new();
            let identity = identity_set(&r, focal, |id| {
                calls.push(id.clone());
                Ok(chain.iter().find(|t| &t.id == id).cloned())
            });
            assert_eq!(calls.len(), 2);
            assert_eq!(identity.ids.len(), 3);
            assert!(identity.warning.is_none());
            let mut invalid_self = last.clone();
            invalid_self.follows = vec![id("oldaa-000001")];
            let mut follower = rec("aa-000004", "follows: [oldaa-000001, cc-000003]\n");
            let tasks = vec![invalid_self, follower.clone(), follower.clone()];
            assert_eq!(followers(&r, &tasks, &identity).len(), 1);
            follower.status = crate::model::Status::Moved;
            assert!(followers(&r, &[follower], &identity).is_empty());
        }
    }
    #[test]
    fn follows_inverse_broken_history_warns_once_and_keeps_verified_matches() {
        let r = Registry::default();
        let focal = rec("aa-000001", "moved_to: bb-000002\n");
        for next in [
            None,
            Some(rec("bb-000002", "moved_from: aa-ffffff\n")),
            Some(rec(
                "bb-000002",
                "moved_from: aa-000001\nmoved_to: aa-000001\n",
            )),
        ] {
            let mut calls = 0;
            let identity = identity_set(&r, &focal, |_| {
                calls += 1;
                Ok(next.clone())
            });
            assert_eq!(calls, 1);
            assert!(identity.warning.is_some());
            let tasks = vec![rec("aa-000004", "follows: [aa-000001]\n")];
            assert_eq!(followers(&r, &tasks, &identity).len(), 1);
            assert!(!identity.ids.contains(&id("aa-ffffff")));
        }
    }
}
