//! Declared goal pieces: foreign tasks that are part of a goal. Each piece is also one of
//! the goal's dependencies, so it keeps gating; only goal accounting reads the list
//! (docs/task-model.md).
use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::resolve::Resolver;

/// True when `id` is, canonically, one of `task.pieces`.
pub(crate) fn is_piece(registry: &Registry, task: &Task, id: &TaskId) -> bool {
    let id = registry.canonical_id(id);
    task.pieces
        .iter()
        .any(|piece| registry.canonical_id(piece) == id)
}

/// The stored pieces, comma-separated, for messages.
pub(crate) fn listed(task: &Task) -> String {
    task.pieces
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The refusal for a lane that carries pieces.
pub(crate) fn lane_refusal(task: &Task) -> Error {
    Error::Validation(format!(
        "{} declares pieces ({}); a lane holds only local children, so it cannot carry pieces",
        task.id,
        listed(task)
    ))
}

/// The subset, foreign, self, duplicate and lane rules of spec §5.1/§5.2 over `task`.
pub(crate) fn validate_list(registry: &Registry, task: &Task) -> Result<()> {
    if task.pieces.is_empty() {
        return Ok(());
    }
    if task.lane {
        return Err(lane_refusal(task));
    }
    let own = registry.canonical_id(&task.id);
    let mut seen = std::collections::BTreeSet::new();
    for piece in &task.pieces {
        let canonical = registry.canonical_id(piece);
        if canonical == own {
            return Err(Error::Validation(format!(
                "{} cannot be its own piece",
                task.id
            )));
        }
        if canonical.prefix == own.prefix {
            return Err(Error::Validation(format!(
                "piece {piece} is in {}'s own project; make it a child with --parent instead",
                task.id
            )));
        }
        if !seen.insert(canonical.clone()) {
            return Err(Error::Validation(format!("duplicate piece {canonical}")));
        }
        if !task
            .depends
            .iter()
            .any(|dependency| registry.canonical_id(dependency) == canonical)
        {
            return Err(Error::Validation(format!(
                "piece {piece} is not a dependency of {}; a piece must stay a dependency",
                task.id
            )));
        }
    }
    Ok(())
}

/// Marks each value as a piece, adding it to `depends` through
/// `dep::add_dependencies` when absent (silently reusing a canonical match), in one save.
/// Returns "{task} already has piece {id}; unchanged" warnings for existing pieces.
pub(crate) fn append(
    ctx: &Ctx,
    resolver: &Resolver<'_>,
    task: &mut Task,
    values: &[String],
) -> Result<Vec<String>> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let registry = &ctx.registry;
    if task.lane {
        return Err(Error::Validation(format!(
            "{} is a lane; a lane holds only local children, so it cannot carry pieces",
            task.id
        )));
    }
    let own = registry.canonical_id(&task.id);
    let existing = task.pieces.len();
    let mut warnings = Vec::new();
    let mut new_dependencies = Vec::new();
    for value in values {
        let id = crate::commands::parse_id(registry, &ctx.shorthand, value)?;
        if id == own {
            return Err(Error::Validation(format!(
                "{} cannot be its own piece",
                task.id
            )));
        }
        // Same-project work is a child; refuse before reading anything.
        if id.prefix == own.prefix {
            return Err(Error::Validation(format!(
                "piece {id} is in {}'s own project; make it a child with --parent instead",
                task.id
            )));
        }
        let found = resolver
            .resolve_canonical_task(&id)?
            .ok_or_else(|| Error::UnresolvableId(id.to_string()))?;
        crate::tombstone::refuse_reference(&found, "piece")?;
        match task
            .pieces
            .iter()
            .position(|stored| registry.canonical_id(stored) == id)
        {
            Some(index) if index < existing => {
                let warning = format!("{} already has piece {id}; unchanged", task.id);
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                continue;
            }
            Some(_) => continue,
            None => task.pieces.push(id.clone()),
        }
        if !task
            .depends
            .iter()
            .any(|dependency| registry.canonical_id(dependency) == id)
        {
            new_dependencies.push(id.to_string());
        }
    }
    crate::commands::dep::add_dependencies(ctx, resolver, task, &new_dependencies)?;
    validate_list(registry, task)?;
    Ok(warnings)
}

/// Unmarks each id; it stays in `depends`. An id that is not a piece refuses.
pub(crate) fn remove(registry: &Registry, task: &mut Task, ids: &[TaskId]) -> Result<()> {
    for id in ids {
        let before = task.pieces.len();
        task.pieces
            .retain(|stored| registry.canonical_id(stored) != *id);
        if task.pieces.len() == before {
            return Err(Error::Validation(format!("{} has no piece {id}", task.id)));
        }
    }
    Ok(())
}

/// Editor-save guard: validates a changed list, or the unchanged list against a changed
/// `depends` or `lane`, and resolves newly introduced pieces, refusing missing targets
/// and tombstones.
pub(crate) fn validate_edit(
    registry: &Registry,
    resolver: &Resolver<'_>,
    original: &Task,
    edited: &Task,
) -> Result<()> {
    let changed = original.pieces != edited.pieces;
    if changed || original.depends != edited.depends || original.lane != edited.lane {
        validate_list(registry, edited)?;
    }
    if !changed {
        return Ok(());
    }
    for piece in &edited.pieces {
        let id = registry.canonical_id(piece);
        if !original
            .pieces
            .iter()
            .any(|stored| registry.canonical_id(stored) == id)
        {
            let found = resolver
                .resolve_canonical_task(&id)?
                .ok_or_else(|| Error::UnresolvableId(id.to_string()))?;
            crate::tombstone::refuse_reference(&found, "piece")?;
        }
    }
    Ok(())
}
