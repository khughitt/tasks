//! Pending proposals: a curate or scope note awaiting the human's decision. Shared by
//! `sample`'s pool and the review state (wait-aware spec §4.1).

use crate::model::Task;

/// The proposal text of a task whose most recent note is a curate or scope note awaiting
/// the human's decision. Any later note clears it, which is how the human answers.
pub fn pending(task: &Task) -> Option<&str> {
    let text = task.notes.last()?.text.as_str();
    let rest = text
        .strip_prefix("curate:")
        .or_else(|| text.strip_prefix("scope:"))?;
    rest.split_once("proposal:")
        .map(|(_, proposal)| proposal.trim())
}
