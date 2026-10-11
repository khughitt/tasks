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

/// What a write does to the `reviewed` flag (spec §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    Keep,
    Set,
    Clear,
}

impl Review {
    /// clap's `conflicts_with` keeps both from arriving together.
    pub fn from_flags(set: bool, clear: bool) -> Review {
        match (set, clear) {
            (true, _) => Review::Set,
            (_, true) => Review::Clear,
            _ => Review::Keep,
        }
    }
}

/// Applies `review` before anything is written, so a refusal leaves the file untouched.
/// `Keep` on a reviewed idea warns: the write does not readmit it.
pub fn apply(task: &mut Task, review: Review, warnings: &mut Vec<String>) -> Result<()> {
    match review {
        Review::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            task.reviewed = true;
        }
        Review::Clear => task.reviewed = false,
        Review::Keep => {
            if task.reviewed {
                warnings.push(format!(
                    "{} stays reviewed; add --no-reviewed to readmit it for scope",
                    task.id
                ));
            }
        }
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
