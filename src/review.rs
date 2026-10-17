//! Review state: the stored `reviewed` flag on ideas and the derived `needs_review` mark
//! with its `review_reason`. See docs/specs/2026-10-11-review-state-design.md and
//! docs/specs/2026-10-17-wait-aware-review-design.md.

use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::model::{Status, Task};
use std::collections::HashMap;
use time::OffsetDateTime;

/// Why a row needs a look (wait-aware spec §4.2). Absent when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewReason {
    Unreviewed,
    Woken,
    Stranded,
    Unassessed,
}

/// What one dependency edge says once resolved (spec §4.1). An edge that cannot be
/// resolved is `Open`, as `ready` treats it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepState {
    Open,
    Closed { closed: Option<String> },
}

/// The edges of the reviewed ideas in one scan, resolved against that scan (spec §4.3).
/// Built once per scan and never shared: a parked row read from its recorded checkout is
/// judged with that checkout's view, so one id can be open in one view and done in
/// another.
pub struct Deps {
    states: HashMap<TaskId, DepState>,
}

impl Deps {
    /// Resolves every edge of every reviewed idea in `scan`, each distinct id once.
    /// `resolve` follows `moved_to` chains and returns `None` for an unresolvable edge.
    pub fn of_scan(
        scan: &[Task],
        mut resolve: impl FnMut(&TaskId) -> Result<Option<Task>>,
    ) -> Result<Deps> {
        let mut states = HashMap::new();
        for task in scan
            .iter()
            .filter(|task| task.status == Status::Idea && task.reviewed)
        {
            for dependency in &task.depends {
                if states.contains_key(dependency) {
                    continue;
                }
                let state = match resolve(dependency)? {
                    Some(found) if !found.status.is_open() => DepState::Closed {
                        closed: found.closed.clone(),
                    },
                    _ => DepState::Open,
                };
                states.insert(dependency.clone(), state);
            }
        }
        Ok(Deps { states })
    }

    /// A view from given states, for unit fixtures.
    #[cfg(test)]
    pub fn from_states(states: impl IntoIterator<Item = (TaskId, DepState)>) -> Deps {
        Deps {
            states: states.into_iter().collect(),
        }
    }

    fn state(&self, dependency: &TaskId) -> &DepState {
        self.states.get(dependency).unwrap_or_else(|| {
            panic!("{dependency} is not in this view: a row is judged with the view of the scan it came from")
        })
    }
}

/// The derived review state (wait-aware spec §4.2; todos keep review-state spec §4).
pub fn state(
    task: &Task,
    child_count: usize,
    deps: &Deps,
    now: OffsetDateTime,
) -> Option<ReviewReason> {
    match task.status {
        Status::Idea if !task.reviewed => Some(ReviewReason::Unreviewed),
        Status::Idea if waits(task, deps, now) => None,
        Status::Idea if woken(task, deps) => Some(ReviewReason::Woken),
        Status::Idea => Some(ReviewReason::Stranded),
        Status::Todo | Status::Blocked
            if task.process.is_none() && !task.lane && child_count == 0 =>
        {
            Some(ReviewReason::Unassessed)
        }
        _ => None,
    }
}

/// Spec §4.1: a recorded wait.
fn waits(task: &Task, deps: &Deps, now: OffsetDateTime) -> bool {
    task.awaiting_user
        || crate::defer::is_deferred(task, now)
        || crate::proposal::pending(task).is_some()
        || task
            .depends
            .iter()
            .any(|dependency| *deps.state(dependency) == DepState::Open)
}

/// Spec §4.2: new evidence since the review. Called only when nothing waits, so a
/// deferral here has come due. Equal stamps count as woken; the deferral test is strict.
fn woken(task: &Task, deps: &Deps) -> bool {
    let Some(reviewed_at) = task.reviewed_at.as_deref().map(stamp) else {
        return false;
    };
    let closed_since = task.depends.iter().any(|dependency| {
        matches!(deps.state(dependency), DepState::Closed { closed: Some(closed) } if stamp(closed) >= reviewed_at)
    });
    closed_since
        || task
            .defer
            .is_some_and(|defer| defer.date() > reviewed_at.date())
}

/// Record stamps were validated by `parse_task`.
fn stamp(text: &str) -> OffsetDateTime {
    crate::time::parse(text).expect("record stamps are validated at parse")
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

    fn idea(extra: &str, depends: &str) -> Task {
        crate::format::parse_task(
            &format!(
                "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\n{extra}\
                 created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                 depends: [{depends}]\ntags: []\n---\n"
            ),
            "sci-000001.md",
        )
        .unwrap()
    }

    fn at(text: &str) -> OffsetDateTime {
        crate::time::parse(text).unwrap()
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    fn closed(stamp: &str) -> DepState {
        DepState::Closed {
            closed: Some(stamp.into()),
        }
    }

    const REVIEWED: &str = "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\n";
    const NOW: &str = "2026-10-20T00:00:00Z";

    #[test]
    fn state_follows_spec_section_4() {
        use ReviewReason::*;
        let none = Deps::from_states([]);
        let now = at(NOW);
        assert_eq!(state(&idea("", ""), 0, &none, now), Some(Unreviewed));
        assert_eq!(state(&idea(REVIEWED, ""), 0, &none, now), Some(Stranded));
        assert_eq!(
            state(&idea("reviewed: true\n", ""), 0, &none, now),
            Some(Stranded),
            "legacy"
        );
        let marked = format!("{REVIEWED}awaiting_user: true\n");
        assert_eq!(state(&idea(&marked, ""), 0, &none, now), None);
        // Todos keep review-state §4.
        let todo = |extra: &str| {
            crate::format::parse_task(
                &format!(
                    "---\nid: sci-000002\ntitle: T\nstatus: todo\npriority: 2\n{extra}\
                     created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                     depends: []\ntags: []\n---\n"
                ),
                "sci-000002.md",
            )
            .unwrap()
        };
        assert_eq!(state(&todo(""), 0, &none, now), Some(Unassessed));
        assert_eq!(state(&todo("process: direct\n"), 0, &none, now), None);
        assert_eq!(state(&todo(""), 2, &none, now), None, "a goal");
        assert_eq!(state(&todo("lane: true\n"), 0, &none, now), None, "a lane");
    }

    #[test]
    fn dependencies_wait_wake_or_strand() {
        use ReviewReason::*;
        let now = at(NOW);
        let one = |dep: DepState| Deps::from_states([(id("sci-0000aa"), dep)]);
        let task = idea(REVIEWED, "sci-0000aa");
        assert_eq!(state(&task, 0, &one(DepState::Open), now), None);
        assert_eq!(
            state(&task, 0, &one(closed("2026-10-18T00:00:00Z")), now),
            Some(Woken)
        );
        assert_eq!(
            state(&task, 0, &one(closed("2026-10-17T09:00:00Z")), now),
            Some(Woken),
            "equal stamps count as woken"
        );
        assert_eq!(
            state(&task, 0, &one(closed("2026-10-17T08:59:59Z")), now),
            Some(Stranded)
        );
        assert_eq!(
            state(&task, 0, &one(DepState::Closed { closed: None }), now),
            Some(Stranded),
            "a close from before the stamp existed"
        );
        let legacy = idea("reviewed: true\n", "sci-0000aa");
        assert_eq!(
            state(&legacy, 0, &one(closed("2026-10-18T00:00:00Z")), now),
            Some(Stranded)
        );
        // Review Focus 1 and 4: any open edge keeps it waiting, whatever else closed.
        let two = idea(REVIEWED, "sci-0000aa, sci-0000bb");
        let mixed = Deps::from_states([
            (id("sci-0000aa"), closed("2026-10-18T00:00:00Z")),
            (id("sci-0000bb"), DepState::Open),
        ]);
        assert_eq!(state(&two, 0, &mixed, now), None);
    }

    #[test]
    fn deferrals_and_proposals_wait() {
        use ReviewReason::*;
        let none = Deps::from_states([]);
        let now = at(NOW);
        assert_eq!(
            state(
                &idea(&format!("{REVIEWED}defer: 2026-11-01\n"), ""),
                0,
                &none,
                now
            ),
            None
        );
        assert_eq!(
            state(
                &idea(&format!("{REVIEWED}defer: 2026-10-18\n"), ""),
                0,
                &none,
                now
            ),
            Some(Woken),
            "came due after the review"
        );
        assert_eq!(
            state(
                &idea(&format!("{REVIEWED}defer: 2026-10-17\n"), ""),
                0,
                &none,
                now
            ),
            Some(Stranded),
            "due on the review's own date: the review saw it"
        );
        let mut proposed = idea(REVIEWED, "");
        proposed.notes.push(crate::model::Note {
            at: "2026-10-17T09:00:00Z".into(),
            by: "scope".into(),
            text: "scope: drop; landed; proposal: drop it".into(),
            provenance: None,
        });
        assert_eq!(state(&proposed, 0, &none, now), None);
        proposed.notes.push(crate::model::Note {
            at: "2026-10-17T10:00:00Z".into(),
            by: "keith".into(),
            text: "keep it".into(),
            provenance: None,
        });
        assert_eq!(state(&proposed, 0, &none, now), Some(Stranded));
    }

    #[test]
    fn a_view_resolves_only_reviewed_ideas_edges_once_each() {
        let reviewed = idea(REVIEWED, "sci-0000aa");
        let unreviewed = {
            let mut task = idea("", "sci-0000bb");
            task.id = id("sci-000003");
            task
        };
        let mut calls = Vec::new();
        let deps = Deps::of_scan(&[reviewed.clone(), reviewed.clone(), unreviewed], |dep| {
            calls.push(dep.to_string());
            Ok(None)
        })
        .unwrap();
        assert_eq!(calls, ["sci-0000aa"]);
        assert_eq!(
            state(&reviewed, 0, &deps, at(NOW)),
            None,
            "an unresolvable edge counts as open"
        );
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
