//! Declared goal pieces: foreign tasks that are part of a goal. Each piece is also one of
//! the goal's dependencies, so it keeps gating; only goal accounting reads the list
//! (docs/task-model.md).
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

/// Editor-save guard. Reader stage: refuses any change to `pieces`; when `depends` or
/// `lane` changed, runs `validate_list` on `edited`.
pub(crate) fn validate_edit(
    registry: &Registry,
    _resolver: &Resolver<'_>,
    original: &Task,
    edited: &Task,
) -> Result<()> {
    if original.pieces != edited.pieces {
        return Err(Error::Validation(format!(
            "{} changes its pieces in the editor; pieces writers are not enabled yet",
            edited.id
        )));
    }
    if original.depends != edited.depends || original.lane != edited.lane {
        validate_list(registry, edited)?;
    }
    Ok(())
}
