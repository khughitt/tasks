//! The `moved` tombstone and the pointer between a moved task's two records. See
//! docs/specs/2026-10-08-task-move-design.md §3.

use crate::error::{Error, Result};
use crate::model::{Status, Task};

/// A tombstone carries exactly one pointer, to another project; a destination names
/// another project as its source. Anything else is a hand edit outside `tasks move`.
pub fn validate(task: &Task) -> Result<()> {
    match (task.status, &task.moved_to) {
        (Status::Moved, None) => {
            return Err(Error::Validation(format!(
                "{} is moved but has no moved_to",
                task.id
            )));
        }
        (Status::Moved, Some(to)) if to.prefix == task.id.prefix => {
            return Err(Error::Validation(format!(
                "{} moved_to {to} is in the same project",
                task.id
            )));
        }
        (status, Some(_)) if status != Status::Moved => {
            return Err(Error::Validation(format!(
                "{} has moved_to but status {}",
                task.id,
                status.as_str()
            )));
        }
        _ => {}
    }
    if let Some(from) = &task.moved_from
        && from.prefix == task.id.prefix
    {
        return Err(Error::Validation(format!(
            "{} moved_from {from} is in the same project",
            task.id
        )));
    }
    Ok(())
}
