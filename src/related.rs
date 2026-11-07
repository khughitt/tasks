//! Operations and helpers for related tasks and cross-record task citations.

use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::resolve::Resolver;

/// Tracks changes made to a task's `related` list during a write.
///
/// Task 8 consumes this to apply precedence between explicit changes and automatic citation linking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelatedChange {
    pub added: Vec<TaskId>,
    pub removed: Vec<TaskId>,
    pub cleared: bool,
    pub editor_changed: bool,
}

/// Adds each of `values` to `task.related` unless the task is already related to it under
/// any spelling. Shorthand-parses and canonicalizes; refuses a self link with
/// `Error::Validation("<id> cannot relate to itself")`; a missing record is `Error::UnresolvableId`;
/// a tombstone goes through `tombstone::refuse_reference(&found, "related task")`;
/// an id already present warns `<task> is already related to <id>; unchanged`.
/// Repeats within one call are deduplicated silently.
/// Returns the warnings.
pub fn add_related(
    ctx: &Ctx,
    resolver: &Resolver<'_>,
    task: &mut Task,
    values: &[String],
) -> Result<Vec<String>> {
    let existing = task.related.len();
    let mut warnings = Vec::new();
    for value in values {
        let given = ctx.shorthand.parse(&ctx.registry, value)?;
        let target = ctx.registry.canonical_id(&given);
        if target == task.id {
            return Err(Error::Validation(format!(
                "{target} cannot relate to itself"
            )));
        }
        match resolver.resolve_task(&target)? {
            None => return Err(Error::UnresolvableId(target.to_string())),
            Some(found) => crate::tombstone::refuse_reference(&found, "related task")?,
        }
        match task
            .related
            .iter()
            .position(|item| ctx.registry.canonical_id(item) == target)
        {
            Some(index) if index < existing => {
                let warning = format!("{} is already related to {target}; unchanged", task.id);
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            Some(_) => {}
            None => task.related.push(target),
        }
    }
    Ok(warnings)
}

/// Removes each of `ids` from `task.related`. Removals never resolve the id. An absent
/// id refuses with `Error::Validation("<task> is not related to <id>")`, matching `dep --rm`.
pub fn remove_related(registry: &Registry, task: &mut Task, ids: &[TaskId]) -> Result<()> {
    for id in ids {
        let canonical = registry.canonical_id(id);
        let before = task.related.len();
        task.related
            .retain(|item| registry.canonical_id(item) != canonical);
        if task.related.len() == before {
            return Err(Error::Validation(format!(
                "{} is not related to {id}",
                task.id
            )));
        }
    }
    Ok(())
}

/// Validates an edited task's `related` list on editor save.
///
/// Ids already stored on `original` skip resolution so an unreachable stored id survives
/// an unrelated save. New ids are checked for self-link, resolved, and checked against
/// tombstones.
pub fn validate_editor_related(
    registry: &Registry,
    resolver: &Resolver<'_>,
    original: &Task,
    edited: &Task,
) -> Result<()> {
    for related in &edited.related {
        let canonical = registry.canonical_id(related);
        if canonical == edited.id {
            return Err(Error::Validation(format!(
                "{canonical} cannot relate to itself"
            )));
        }
        if !original
            .related
            .iter()
            .any(|stored| registry.canonical_id(stored) == canonical)
        {
            let found = resolver
                .resolve_task(&canonical)?
                .ok_or_else(|| Error::UnresolvableId(canonical.to_string()))?;
            crate::tombstone::refuse_reference(&found, "related task")?;
        }
    }
    Ok(())
}

/// A task-id source reference (§5.4): exactly one task id whose prefix this host's registry knows.
///
/// The id keeps its stored spelling; callers canonicalize for lookup.
pub fn source_task_id(registry: &Registry, source: &str) -> Option<TaskId> {
    TaskId::parse(source)
        .ok()
        .filter(|id| registry.knows_prefix(&id.prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn source_task_id_recognizes_registered_prefixes_only() {
        let mut r = Registry::default();
        r.register("aa", Path::new("/tmp/aa")).unwrap();
        r.aliases.insert("old".into(), "aa".into());
        r.groups.insert("grp".into(), vec!["aa".into()]);

        // Live prefix
        assert_eq!(
            source_task_id(&r, "aa-123456"),
            Some(TaskId::parse("aa-123456").unwrap())
        );
        // Retired prefix keeps stored spelling
        assert_eq!(
            source_task_id(&r, "old-123456"),
            Some(TaskId::parse("old-123456").unwrap())
        );
        // Group name is rejected
        assert_eq!(source_task_id(&r, "grp-123456"), None);
        // Unregistered prefix is rejected
        assert_eq!(source_task_id(&r, "zz-123456"), None);
        // Surrounding text or invalid grammar is rejected
        assert_eq!(source_task_id(&r, "from aa-123456"), None);
        assert_eq!(source_task_id(&r, " aa-123456"), None);
        assert_eq!(source_task_id(&r, "aa-123456 "), None);
        assert_eq!(source_task_id(&r, "aa-123456/x"), None);
    }
}
