//! Review state: the stored `reviewed` flag on ideas and the derived `needs_review` mark.
//! See docs/specs/2026-10-11-review-state-design.md.

use crate::error::{Error, Result};
use crate::model::{Process, Status, Task};

/// Whether a row needs a look (spec §4): an idea no scope pass has reviewed, or a leaf
/// todo or blocked task whose process is unassessed. Goals, lanes and everything in hand
/// or out of sight never do.
pub fn needs_review(
    status: Status,
    reviewed: bool,
    process: Option<Process>,
    lane: bool,
    child_count: usize,
) -> bool {
    match status {
        Status::Idea => !reviewed,
        Status::Todo | Status::Blocked => process.is_none() && !lane && child_count == 0,
        _ => false,
    }
}

/// The refusal for `reviewed` on anything but an idea (spec §3.2).
pub fn not_an_idea(task: &Task) -> Error {
    Error::Validation(format!(
        "{} is {}; only an idea carries reviewed",
        task.id,
        task.status.as_str()
    ))
}

/// The refusal for `awaiting_user` without `reviewed` (wait-aware spec §3.1).
pub fn not_reviewed(task: &Task) -> Error {
    Error::Validation(format!(
        "{} is not reviewed; only a reviewed idea carries awaiting_user",
        task.id
    ))
}

/// The write-time rule (review-state spec §3.2, wait-aware spec §3.1–3.2). Never called
/// from parsing: `parse_task` must accept a hand-edited record so `check` can report it
/// and an editor save can transition it.
pub fn validate(task: &Task) -> Result<()> {
    if task.reviewed && task.status != Status::Idea {
        return Err(not_an_idea(task));
    }
    if task.awaiting_user && !task.reviewed {
        return Err(not_reviewed(task));
    }
    if task.reviewed_at.is_some() && !task.reviewed {
        return Err(Error::Validation(format!(
            "{} carries reviewed_at without reviewed",
            task.id
        )));
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

/// What a write does to the `awaiting_user` marker (wait-aware spec §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awaiting {
    Keep,
    Set,
    Clear,
}

impl Awaiting {
    /// clap's `conflicts_with` keeps both from arriving together.
    pub fn from_flags(set: bool, clear: bool) -> Awaiting {
        match (set, clear) {
            (true, _) => Awaiting::Set,
            (_, true) => Awaiting::Clear,
            _ => Awaiting::Keep,
        }
    }
}

/// Applies `review`, then `awaiting`, before anything is written, so a refusal leaves the
/// file untouched. A write that changes neither (a plain note) on a reviewed idea warns:
/// it does not readmit the idea.
pub fn apply(
    task: &mut Task,
    review: Review,
    awaiting: Awaiting,
    warnings: &mut Vec<String>,
) -> Result<()> {
    match review {
        Review::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            task.reviewed = true;
            task.reviewed_at = Some(crate::time::now());
        }
        Review::Clear => {
            task.reviewed = false;
            task.reviewed_at = None;
            task.awaiting_user = false;
        }
        Review::Keep => {}
    }
    match awaiting {
        Awaiting::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            if !task.reviewed {
                return Err(not_reviewed(task));
            }
            task.awaiting_user = true;
        }
        Awaiting::Clear => task.awaiting_user = false,
        Awaiting::Keep => {}
    }
    if review == Review::Keep && awaiting == Awaiting::Keep && task.reviewed {
        warnings.push(if task.awaiting_user {
            format!(
                "{} stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope",
                task.id
            )
        } else {
            format!(
                "{} stays reviewed; add --no-reviewed to readmit it for scope",
                task.id
            )
        });
    }
    Ok(())
}

/// The editor's half of wait-aware spec §3.2: `reviewed_at` follows a save's change to
/// `reviewed`. `check_invariants` has already refused a direct change to the stamp.
pub fn stamp_edited(original: &Task, edited: &mut Task) {
    match (original.reviewed, edited.reviewed) {
        (false, true) => edited.reviewed_at = Some(crate::time::now()),
        (true, false) => edited.reviewed_at = None,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Process;

    #[test]
    fn validate_refuses_marker_or_stamp_without_reviewed() {
        let parse = |extra: &str| {
            crate::format::parse_task(
                &format!(
                    "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\n{extra}\
                     created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                     depends: []\ntags: []\n---\n"
                ),
                "sci-000001.md",
            )
            .unwrap()
        };
        let error = validate(&parse("awaiting_user: true\n"))
            .unwrap_err()
            .to_string();
        assert!(
            error
                .contains("sci-000001 is not reviewed; only a reviewed idea carries awaiting_user"),
            "{error}"
        );
        let error = validate(&parse("reviewed_at: 2026-10-17T09:00:00Z\n"))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("sci-000001 carries reviewed_at without reviewed"),
            "{error}"
        );
        assert!(
            validate(&parse(
                "reviewed: true\nawaiting_user: true\nreviewed_at: 2026-10-17T09:00:00Z\n"
            ))
            .is_ok()
        );
        assert!(
            validate(&parse("reviewed: true\n")).is_ok(),
            "a legacy review has no stamp"
        );
    }

    #[test]
    fn needs_review_follows_spec_section_4() {
        use Status::*;
        assert!(needs_review(Idea, false, None, false, 0));
        assert!(!needs_review(Idea, true, None, false, 0));
        assert!(needs_review(Todo, false, None, false, 0));
        assert!(needs_review(Blocked, false, None, false, 0));
        assert!(!needs_review(Todo, false, Some(Process::Direct), false, 0));
        assert!(!needs_review(Todo, false, None, false, 2), "a goal");
        assert!(!needs_review(Todo, false, None, true, 0), "a lane");
        for status in [Doing, Shelved, Done, Dropped, Moved] {
            assert!(!needs_review(status, false, None, false, 0), "{status:?}");
        }
    }

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
