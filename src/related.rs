//! Operations and helpers for related tasks and cross-record task citations.

use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
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

/// Appends each target in `targets` to `task.related` in order, unless `related` already
/// holds it by canonical id.
///
/// Wake targets leaving a shelf carry into `related` exactly as stored, with no resolution.
pub fn carry_wake_targets(
    registry: &Registry,
    task: &mut Task,
    targets: impl IntoIterator<Item = TaskId>,
) {
    for target in targets {
        let canonical = registry.canonical_id(&target);
        if !task
            .related
            .iter()
            .any(|existing| registry.canonical_id(existing) == canonical)
        {
            task.related.push(target);
        }
    }
}

/// Returns canonical task ids cited in `text`, deduplicated, in order of first appearance.
///
/// Splits text into maximal runs of `[A-Za-z0-9_-]`. A run is a citation when the whole
/// run matches `[a-z][a-z0-9]{1,7}-[0-9a-f]{6}` and `registry.knows_prefix(prefix)` holds.
pub fn cited_ids(registry: &Registry, text: &str) -> Vec<TaskId> {
    let mut result = Vec::new();
    for token in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-') {
        if token.is_empty() {
            continue;
        }
        let Some((prefix, hex)) = token.split_once('-') else {
            continue;
        };
        if hex.contains('-') {
            continue;
        }
        if !crate::model::is_valid_prefix(prefix) {
            continue;
        }
        let hex_ok = hex.len() == 6
            && hex
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if !hex_ok {
            continue;
        }
        if !registry.knows_prefix(prefix) {
            continue;
        }
        let task_id = TaskId {
            prefix: prefix.to_string(),
            hex: hex.to_string(),
        };
        let canonical = registry.canonical_id(&task_id);
        if !result.contains(&canonical) {
            result.push(canonical);
        }
    }
    result
}

/// Identifies newly cited task ids across supplied notes and new body text.
///
/// This is every id in `notes`, plus the ids in `new_body` that are absent from
/// `cited_ids(old_body)`, deduplicated in order.
pub fn new_citations(
    registry: &Registry,
    old_body: Option<&str>,
    new_body: &str,
    notes: &[&str],
) -> Vec<TaskId> {
    let mut result = Vec::new();
    for note in notes {
        for id in cited_ids(registry, note) {
            if !result.contains(&id) {
                result.push(id);
            }
        }
    }
    let old_ids = old_body
        .map(|body| cited_ids(registry, body))
        .unwrap_or_default();
    for id in cited_ids(registry, new_body) {
        if !old_ids.contains(&id) && !result.contains(&id) {
            result.push(id);
        }
    }
    result
}

/// Links cited tasks to `task.related` according to spec §5.2.
///
/// Returns warnings. A citation never fails a write; resolver errors become warnings.
pub fn link_citations(
    project: &Project,
    registry: &Registry,
    resolver: &Resolver<'_>,
    task: &mut Task,
    cited: &[TaskId],
    change: &RelatedChange,
) -> Result<Vec<String>> {
    if change.cleared || change.editor_changed {
        return Ok(Vec::new());
    }

    let is_pending = crate::retained::publish::pending_here(project, task)?.is_some();
    let mut warnings = Vec::new();
    let mut would_link = Vec::new();

    let task_canonical = registry.canonical_id(&task.id);
    let source_task = task
        .source
        .as_deref()
        .and_then(|s| source_task_id(registry, s))
        .map(|id| registry.canonical_id(&id));

    for id in cited {
        let cited_canonical = registry.canonical_id(id);
        let resolved: Option<Task> =
            crate::dependency::resolve(registry, id, |id| resolver.resolve_task(id))
                .unwrap_or_default();

        let Some(dest) = resolved else {
            warnings.push(format!(
                "{} cites {id}, which resolves to no task here; not linked",
                task.id
            ));
            continue;
        };

        let dest_canonical = registry.canonical_id(&dest.id);

        let child_of_task = dest
            .parent
            .as_ref()
            .is_some_and(|p| registry.canonical_id(p) == task_canonical)
            || match resolver.resolve_task(id) {
                Ok(Some(orig)) => orig
                    .parent
                    .as_ref()
                    .is_some_and(|p| registry.canonical_id(p) == task_canonical),
                _ => false,
            };

        let is_skip = cited_canonical == task_canonical
            || dest_canonical == task_canonical
            || task.depends.iter().any(|d| {
                let c = registry.canonical_id(d);
                c == cited_canonical || c == dest_canonical
            })
            || task.parent.as_ref().is_some_and(|p| {
                let c = registry.canonical_id(p);
                c == cited_canonical || c == dest_canonical
            })
            || task.wake_on.iter().any(|w| {
                let c = registry.canonical_id(w);
                c == cited_canonical || c == dest_canonical
            })
            || task.moved_to.as_ref().is_some_and(|m| {
                let c = registry.canonical_id(m);
                c == cited_canonical || c == dest_canonical
            })
            || task.moved_from.as_ref().is_some_and(|m| {
                let c = registry.canonical_id(m);
                c == cited_canonical || c == dest_canonical
            })
            || source_task
                .as_ref()
                .is_some_and(|s| *s == cited_canonical || *s == dest_canonical)
            || task.related.iter().any(|r| {
                let c = registry.canonical_id(r);
                c == cited_canonical || c == dest_canonical
            })
            || change.removed.iter().any(|r| {
                let c = registry.canonical_id(r);
                c == cited_canonical || c == dest_canonical
            })
            || child_of_task
            || would_link.contains(&dest_canonical);

        if is_skip {
            continue;
        }

        if is_pending {
            would_link.push(dest_canonical);
        } else {
            task.related.push(dest_canonical.clone());
            would_link.push(dest_canonical);
        }
    }

    if is_pending {
        if would_link.is_empty() {
            return Ok(Vec::new());
        }
        let ids = would_link
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let id_part = if would_link.len() == 1 {
            would_link[0].to_string()
        } else {
            "<id>".to_string()
        };
        return Ok(vec![format!(
            "{} has a pending retained operation; not linked: {ids}; link them with tasks edit {} --related {id_part} once it completes",
            task.id, task.id
        )]);
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use std::path::Path;

    fn test_task(id: &str) -> Task {
        Task {
            note_archives: Vec::new(),
            wake_on: Vec::new(),
            id: TaskId::parse(id).unwrap(),
            title: "T".into(),
            status: Status::Todo,
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
            created: "2026-09-09T00:00:00Z".into(),
            updated: "2026-09-09T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            related: vec![],
            parent: None,
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
    fn carry_wake_targets_appends_in_order_deduplicating_by_canonical_id() {
        let mut r = Registry::default();
        r.register("aa", Path::new("/tmp/aa")).unwrap();
        r.register("bb", Path::new("/tmp/bb")).unwrap();
        r.aliases.insert("oldbb".into(), "bb".into());

        let mut task = test_task("aa-000001");
        task.related = vec![TaskId::parse("aa-111111").unwrap()];

        let targets = vec![
            TaskId::parse("aa-111111").unwrap(),    // duplicate of existing
            TaskId::parse("oldbb-222222").unwrap(), // new, stored spelling kept
            TaskId::parse("bb-222222").unwrap(),    // alias duplicate of preceding
            TaskId::parse("aa-333333").unwrap(),    // new
        ];

        carry_wake_targets(&r, &mut task, targets);

        assert_eq!(
            task.related,
            vec![
                TaskId::parse("aa-111111").unwrap(),
                TaskId::parse("oldbb-222222").unwrap(),
                TaskId::parse("aa-333333").unwrap(),
            ]
        );
    }

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

    #[test]
    fn cited_ids_detects_citations_and_skips_lookalikes() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/sci")).unwrap();
        r.register("bb", Path::new("/tmp/bb")).unwrap();
        r.aliases.insert("oldsci".into(), "sci".into());

        // Boundary: punctuation, markdown, path wrapping
        let text = "(sci-0d00d2) and sci-0d00d2. and `sci-0d00d2` and **sci-0d00d2** and tasks/files/sci-0d00d2/x.png";
        assert_eq!(
            cited_ids(&r, text),
            vec![TaskId::parse("sci-0d00d2").unwrap()]
        );

        // Canonicalization of retired prefix and deduplication
        let text2 = "oldsci-0d00d2 and sci-0d00d2 and bb-111111";
        assert_eq!(
            cited_ids(&r, text2),
            vec![
                TaskId::parse("sci-0d00d2").unwrap(),
                TaskId::parse("bb-111111").unwrap(),
            ]
        );

        // Lookalike tokens are not citations
        let lookalikes = "foo-sci-0d00d2 sci-0d00d2-x SCI-0D00D2 0d00d2 2026-11-07 zz-123456";
        assert_eq!(cited_ids(&r, lookalikes), Vec::<TaskId>::new());
    }

    #[test]
    fn new_citations_handles_notes_and_body_diffs() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/sci")).unwrap();
        r.register("bb", Path::new("/tmp/bb")).unwrap();

        let old_body = "mentions sci-000001";
        let new_body = "mentions sci-000001 and sci-000002";
        let notes = [
            "note with sci-000003 and sci-000001",
            "another note with sci-000004",
        ];

        // All ids in notes count, plus new body ids absent from old body
        let result = new_citations(&r, Some(old_body), new_body, &notes);
        assert_eq!(
            result,
            vec![
                TaskId::parse("sci-000003").unwrap(),
                TaskId::parse("sci-000001").unwrap(),
                TaskId::parse("sci-000004").unwrap(),
                TaskId::parse("sci-000002").unwrap(),
            ]
        );

        // Reworded body with same ids
        let reworded = "reworded without any new id: sci-000001";
        assert_eq!(
            new_citations(&r, Some(old_body), reworded, &[]),
            Vec::<TaskId>::new()
        );

        // No old body (e.g. add)
        assert_eq!(
            new_citations(&r, None, new_body, &[]),
            vec![
                TaskId::parse("sci-000001").unwrap(),
                TaskId::parse("sci-000002").unwrap(),
            ]
        );
    }
}
