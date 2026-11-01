//! `tasks goals`: which goals the user asked for, their progress, recent activity and the
//! work in them that needs attention (records spec 2026-11-01-goals-view-design §5). Pure
//! over one registry scan and this host's claim snapshot.

use crate::claims::{ClaimSnapshot, Reason, WaitingOn};
use crate::error::Result;
use crate::hierarchy;
use crate::model::{Origin, Status, Task};
use crate::palette::Rgb;
use crate::registry::Registry;
use serde::Serialize;
use time::OffsetDateTime;

/// A `doing` record untouched this long is stalled (§5.4). Fixed: no option changes it.
pub const STALL_AFTER: time::Duration = time::Duration::days(3);
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

/// One attention flag: how many records raise it, and the one to open first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flag {
    pub count: usize,
    pub first: String,
    pub since: String,
}

/// What in a goal needs a look (§5.4). Only nonzero flags are present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Attention {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Flag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Flag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub you: Option<Flag>,
    /// Pretty-only: the paused lane counted under `blocked`, so the row can name it.
    #[serde(skip)]
    pub paused_lane: Option<String>,
}

impl Attention {
    pub fn is_empty(&self) -> bool {
        self.stalled.is_none() && self.blocked.is_none() && self.you.is_none()
    }
}

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

/// A record's effective timestamp (§5.4): its park's `at` when parked on this host; for an
/// idea awaiting the user, `reviewed_at`, else `updated`; otherwise `updated`.
pub fn effective_at<'a>(task: &'a Task, claims: &'a ClaimSnapshot) -> &'a str {
    if let Some(park) = claims.park(&task.id) {
        return &park.at;
    }
    if task.status == Status::Idea && task.awaiting_user {
        return task.reviewed_at.as_deref().unwrap_or(&task.updated);
    }
    &task.updated
}

/// A record raising a flag, with the timestamp that orders it.
struct Hit<'a> {
    at: OffsetDateTime,
    id: String,
    since: &'a str,
}

impl<'a> Hit<'a> {
    fn new(id: String, since: &'a str) -> Result<Hit<'a>> {
        Ok(Hit {
            at: crate::time::parse(since)?,
            id,
            since,
        })
    }
}

/// The flag over `hits`, naming the one with the oldest timestamp, ties on id.
fn flag(hits: Vec<Hit>) -> Option<Flag> {
    let count = hits.len();
    hits.into_iter()
        .min_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)))
        .map(|first| Flag {
            count,
            first: first.id,
            since: first.since.to_string(),
        })
}

/// Whether a `doing` record is stalled, and since when. A goal record (it has descendants
/// or is a lane) is judged by its whole subtree: stalled only when nothing in it is newer
/// than `STALL_AFTER` and nothing in it is held; `since` is the subtree's newest stamp.
fn stalled_since<'a>(
    task: &'a Task,
    all: &'a [Task],
    claims: &'a ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<Hit<'a>>> {
    if let Some(park) = claims.park(&task.id)
        && (park.waiting_on == WaitingOn::User
            || matches!(park.reason, Some(Reason::Dependency | Reason::Quiet)))
    {
        return Ok(None);
    }
    let descendants = hierarchy::descendants(all, &task.id, registry);
    let hit = if hierarchy::is_goal(task, !descendants.is_empty()) {
        let subtree: Vec<&Task> = std::iter::once(task).chain(descendants).collect();
        if subtree
            .iter()
            .any(|record| claims.held(&record.id).is_some())
        {
            return Ok(None);
        }
        let mut newest = Hit::new(task.id.to_string(), effective_at(task, claims))?;
        for record in &subtree[1..] {
            let at = effective_at(record, claims);
            let parsed = crate::time::parse(at)?;
            if parsed > newest.at {
                newest.at = parsed;
                newest.since = at;
            }
        }
        newest
    } else {
        if claims.park(&task.id).is_none() && claims.held(&task.id).is_some() {
            return Ok(None);
        }
        Hit::new(task.id.to_string(), effective_at(task, claims))?
    };
    Ok((now - hit.at >= STALL_AFTER).then_some(hit))
}

/// The goal's attention flags over its own record and its active descendants (§5.4), or
/// None when it raises none.
pub fn attention(
    goal: &Task,
    all: &[Task],
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<Attention>> {
    let records: Vec<&Task> = std::iter::once(goal)
        .chain(hierarchy::descendants(all, &goal.id, registry))
        .filter(|task| hierarchy::is_active(task))
        .collect();
    let (mut stalled, mut blocked, mut you) = (Vec::new(), Vec::new(), Vec::new());
    for task in &records {
        let id = || task.id.to_string();
        if task.status == Status::Blocked {
            blocked.push(Hit::new(id(), effective_at(task, claims))?);
        }
        if task.status == Status::Doing
            && let Some(hit) = stalled_since(task, all, claims, registry, now)?
        {
            stalled.push(hit);
        }
        let parked_on_user = claims
            .park(&task.id)
            .is_some_and(|park| park.waiting_on == WaitingOn::User);
        if parked_on_user || (task.status == Status::Idea && task.awaiting_user) {
            you.push(Hit::new(id(), effective_at(task, claims))?);
        }
    }
    let paused_lane = hierarchy::paused_lane(all, goal, registry);
    if let Some(lane) = paused_lane {
        blocked.push(Hit::new(lane.id.to_string(), effective_at(lane, claims))?);
    }
    let out = Attention {
        stalled: flag(stalled),
        blocked: flag(blocked),
        you: flag(you),
        paused_lane: paused_lane.map(|lane| lane.id.to_string()),
    };
    Ok((!out.is_empty()).then_some(out))
}

/// The selected rows, open ones first by subtree activity, then total, then id; closed
/// ones after, newest close first, then id.
pub fn goal_rows(
    all: &[Task],
    claims: &ClaimSnapshot,
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
        let row = |closed: Option<String>, attention: Option<Attention>| -> Result<GoalRow> {
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
                attention,
                closed,
            })
        };
        match task.status {
            Status::Idea | Status::Todo | Status::Doing | Status::Blocked => {
                let row = row(None, attention(task, all, claims, registry, now)?)?;
                open.push((crate::time::parse(&row.last_activity)?, row));
            }
            Status::Done => {
                let stamp = closed_at(task);
                let at = crate::time::parse(stamp)?;
                if now - at < CLOSED_WINDOW {
                    closed.push((at, row(Some(stamp.to_string()), None)?));
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

    use crate::claims::Park;
    use std::collections::BTreeMap;

    fn park_at(id: &str, at: &str) -> ClaimSnapshot {
        let park = Park {
            owner: "o".into(),
            session: "s".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: at.into(),
            next_step: "next".into(),
            waiting_on: WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: id.into(),
        };
        ClaimSnapshot::from_parts(
            BTreeMap::new(),
            BTreeMap::from([(id.to_string(), park)]),
            BTreeMap::new(),
        )
    }

    #[test]
    fn effective_at_prefers_park_then_review_then_updated() {
        let plain = task("aa-000001", None, Status::Doing);
        let none = ClaimSnapshot::default();
        assert_eq!(effective_at(&plain, &none), "2026-09-03T00:00:00Z");
        let parked = park_at("aa-000001", "2026-09-01T00:00:00Z");
        assert_eq!(effective_at(&plain, &parked), "2026-09-01T00:00:00Z");
        let mut idea = task("aa-000002", None, Status::Idea);
        idea.awaiting_user = true;
        assert_eq!(effective_at(&idea, &none), "2026-09-03T00:00:00Z");
        idea.reviewed_at = Some("2026-09-02T00:00:00Z".into());
        assert_eq!(effective_at(&idea, &none), "2026-09-02T00:00:00Z");
    }

    #[test]
    fn future_stamp_is_never_stalled() {
        let goal = task("aa-000000", None, Status::Todo);
        let mut leaf = task("aa-000001", Some("aa-000000"), Status::Doing);
        leaf.updated = "2099-01-01T00:00:00Z".into();
        let all = vec![goal.clone(), leaf];
        let now = crate::time::parse("2026-11-01T00:00:00Z").unwrap();
        let flags = attention(
            &goal,
            &all,
            &ClaimSnapshot::default(),
            &Registry::default(),
            now,
        )
        .unwrap();
        assert_eq!(flags, None);
    }

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
