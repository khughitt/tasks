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
