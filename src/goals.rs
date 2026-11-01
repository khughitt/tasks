//! `tasks goals`: which goals the user asked for, their progress, recent activity and the
//! work in them that needs attention (records spec 2026-11-01-goals-view-design §5). Pure
//! over one registry scan and this host's claim snapshot.

use crate::claims::ClaimSnapshot;
use crate::error::Result;
use crate::hierarchy;
use crate::model::{Origin, Status, Task};
use crate::palette::Rgb;
use crate::registry::Registry;
use serde::Serialize;
use time::OffsetDateTime;

/// A done goal stays listed, in the closed section, this long after it closed (§5.1).
pub const CLOSED_WINDOW: time::Duration = time::Duration::days(14);

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A goal's progress over its descendants (§5.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Pieces {
    pub done: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub ideas: usize,
}

/// What in a goal needs a look. Filled by the attention rules (§5.4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Attention {}

#[derive(Debug, Clone, Serialize)]
pub struct GoalRow {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    pub priority: u8,
    pub lane: bool,
    /// The project's identity color, filled by `Output::paint_projects`; serialized as
    /// `color` (§5.5), unlike task rows.
    #[serde(rename = "color", skip_serializing_if = "Option::is_none")]
    pub project_color: Option<Rgb>,
    pub last_activity: String,
    pub pieces: Pieces,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention: Option<Attention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
}

/// `done` over `done|todo|doing|blocked`, with ideas counted apart. Shelved, dropped and
/// moved pieces are not counted.
pub fn pieces(descendants: &[&Task]) -> Pieces {
    let mut out = Pieces::default();
    for task in descendants {
        match task.status {
            Status::Done => {
                out.done += 1;
                out.total += 1;
            }
            Status::Todo | Status::Doing | Status::Blocked => out.total += 1,
            Status::Idea => out.ideas += 1,
            Status::Shelved | Status::Dropped | Status::Moved => {}
        }
    }
    out
}

/// The newest `updated` stamp among the goal and all its descendants, whatever their
/// status (§5.3), compared as parsed instants.
pub fn last_activity(goal: &Task, descendants: &[&Task]) -> Result<String> {
    let mut newest = (crate::time::parse(&goal.updated)?, goal.updated.as_str());
    for task in descendants {
        let at = crate::time::parse(&task.updated)?;
        if at > newest.0 {
            newest = (at, task.updated.as_str());
        }
    }
    Ok(newest.1.to_string())
}

/// When a done goal closed: `closed`, else `completed`, else `updated` on older records.
pub fn closed_at(goal: &Task) -> &str {
    goal.closed
        .as_deref()
        .or(goal.completed.as_deref())
        .unwrap_or(&goal.updated)
}

/// The selected rows, open ones first by subtree activity, then total, then id; closed
/// ones after, newest close first, then id.
pub fn goal_rows(
    all: &[Task],
    _claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Vec<GoalRow>> {
    let mut open: Vec<(OffsetDateTime, GoalRow)> = Vec::new();
    let mut closed: Vec<(OffsetDateTime, GoalRow)> = Vec::new();
    for task in all {
        if !matches!(task.origin, Some(Origin::User | Origin::Mixed)) {
            continue;
        }
        let descendants = hierarchy::descendants(all, &task.id, registry);
        if !hierarchy::is_goal(task, !descendants.is_empty()) {
            continue;
        }
        let row = |closed: Option<String>| -> Result<GoalRow> {
            Ok(GoalRow {
                id: task.id.to_string(),
                title: task.title.clone(),
                status: task.status,
                origin: task.origin,
                priority: task.priority,
                lane: task.lane,
                project_color: None,
                last_activity: last_activity(task, &descendants)?,
                pieces: pieces(&descendants),
                attention: None,
                closed,
            })
        };
        match task.status {
            Status::Idea | Status::Todo | Status::Doing | Status::Blocked => {
                let row = row(None)?;
                open.push((crate::time::parse(&row.last_activity)?, row));
            }
            Status::Done => {
                let stamp = closed_at(task);
                let at = crate::time::parse(stamp)?;
                if now - at < CLOSED_WINDOW {
                    closed.push((at, row(Some(stamp.to_string()))?));
                }
            }
            Status::Shelved | Status::Dropped | Status::Moved => {}
        }
    }
    open.sort_by(|(a_at, a), (b_at, b)| {
        b_at.cmp(a_at)
            .then_with(|| b.pieces.total.cmp(&a.pieces.total))
            .then_with(|| a.id.cmp(&b.id))
    });
    closed.sort_by(|(a_at, a), (b_at, b)| b_at.cmp(a_at).then_with(|| a.id.cmp(&b.id)));
    Ok(open.into_iter().chain(closed).map(|(_, row)| row).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::tests::task;

    #[test]
    fn pieces_skip_moved_shelved_and_dropped() {
        let records = [
            task("aa-000001", Some("aa-000000"), Status::Moved),
            task("aa-000002", Some("aa-000000"), Status::Shelved),
            task("aa-000003", Some("aa-000000"), Status::Dropped),
            task("aa-000004", Some("aa-000000"), Status::Done),
        ];
        let refs: Vec<&Task> = records.iter().collect();
        assert_eq!(
            pieces(&refs),
            Pieces {
                done: 1,
                total: 1,
                ideas: 0
            }
        );
    }
}
