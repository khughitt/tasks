//! Review state: the stored `reviewed` flag on ideas and the derived `needs_review` mark.
//! See docs/specs/2026-10-11-review-state-design.md.

use crate::error::{Error, Result};
use crate::model::{Status, Task};

/// The refusal for `reviewed` on anything but an idea (spec §3.2).
pub fn not_an_idea(task: &Task) -> Error {
    Error::Validation(format!(
        "{} is {}; only an idea carries reviewed",
        task.id,
        task.status.as_str()
    ))
}

/// The write-time rule (spec §3.2). Never called from parsing: `parse_task` must accept
/// a hand-edited record so `check` can report it and an editor save can transition it.
pub fn validate(task: &Task) -> Result<()> {
    if task.reviewed && task.status != Status::Idea {
        return Err(not_an_idea(task));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validate_refuses_reviewed_on_a_non_idea_only() {
        let mut task = crate::format::parse_task(
            "---\nid: sci-000001\ntitle: T\nstatus: todo\npriority: 2\nreviewed: true\n\
             created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
             depends: []\ntags: []\n---\n",
            "sci-000001.md",
        )
        .unwrap();
        let error = validate(&task).unwrap_err().to_string();
        assert!(
            error.contains("sci-000001 is todo; only an idea carries reviewed"),
            "{error}"
        );
        task.status = crate::model::Status::Idea;
        assert!(validate(&task).is_ok());
        task.status = crate::model::Status::Todo;
        task.reviewed = false;
        assert!(validate(&task).is_ok());
    }
}
