//! `tasks goals`: which goals the user asked for, their progress, recent activity and the
//! work in them that needs attention (records spec docs/user-guide.md). Reads members
//! across projects through `Membership` and uses this host's claim snapshot.

use crate::claims::{ClaimSnapshot, Reason, WaitingOn};
use crate::dependency::Verdict;
use crate::error::Result;
use crate::hierarchy;
use crate::membership::Membership;
use crate::model::{Origin, Status, Task, TaskId};
use crate::palette::Rgb;
use crate::registry::Registry;
use serde::Serialize;
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// A `doing` record untouched this long is stalled. Fixed: no option changes it.
pub const STALL_AFTER: time::Duration = time::Duration::days(3);
/// Under `--closed`, a done goal is listed in the closed section this long after it
/// closed; without the flag no done goal is listed.
pub const CLOSED_WINDOW: time::Duration = time::Duration::days(14);

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GoalSort {
    #[default]
    Project,
    Activity,
    Priority,
}

impl GoalSort {
    pub const ALL: [Self; 3] = [Self::Project, Self::Activity, Self::Priority];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Activity => "activity",
            Self::Priority => "priority",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.as_str() == value)
            .ok_or_else(|| {
                crate::error::Error::Validation(format!(
                    "unknown goals sort {value:?}; expected project, activity, or priority"
                ))
            })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GoalOrder {
    pub sort: GoalSort,
    pub reverse: bool,
}

/// A goal's progress over its members (descendants and declared pieces).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Pieces {
    pub done: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub ideas: usize,
}

/// A goal's dependencies outside its own members: prerequisites, reported apart
/// from progress so a goal whose members are all done but whose prerequisites are not never
/// reads as finished. `total` counts distinct targets, `open` those not yet satisfied, and
/// `unresolved` those no record answers (never satisfied, never open).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Dependencies {
    pub open: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub unresolved: usize,
}

/// One attention flag: how many records raise it, and the one to open first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flag {
    pub count: usize,
    pub first: String,
    pub since: String,
}

/// What in a goal needs a look. Only nonzero flags are present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Attention {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Flag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Flag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub you: Option<Flag>,
    /// Pretty-only: the paused lanes counted under `blocked`, so the row can name one.
    #[serde(skip)]
    pub paused_lane: Vec<String>,
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
    /// `color` , unlike task rows.
    #[serde(rename = "color", skip_serializing_if = "Option::is_none")]
    pub project_color: Option<Rgb>,
    /// The truecolor shade for task IDs, present exactly when color is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_color: Option<Rgb>,
    #[serde(skip)]
    pub activity: Option<crate::style::Activity>,
    pub last_activity: String,
    pub pieces: Pieces,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Dependencies>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention: Option<Attention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
}

/// `done` over `done|todo|doing|blocked`, with ideas counted apart. Shelved, dropped and
/// moved members are not counted.
pub fn pieces(members: &[&Task]) -> Pieces {
    let mut out = Pieces::default();
    for task in members {
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

/// The distinct dependency targets of the goal record and of its counted members (`Pieces`'s
/// `total`), followed through moves, that lie outside the goal's membership; None when there
/// are none. `lookup` reads one record by canonical id, so a target in a project outside
/// the scanned scope is judged the same as one inside it.
pub fn dependencies(
    goal: &Task,
    members: &[&Task],
    registry: &Registry,
    mut lookup: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Option<Dependencies>> {
    let member_ids: BTreeSet<TaskId> = std::iter::once(goal)
        .chain(members.iter().copied())
        .map(|task| registry.canonical_id(&task.id))
        .collect();
    let sources = std::iter::once(goal).chain(members.iter().copied().filter(|task| {
        matches!(
            task.status,
            Status::Done | Status::Todo | Status::Doing | Status::Blocked
        )
    }));
    let mut seen = BTreeSet::new();
    let mut out = Dependencies::default();
    for source in sources {
        for stored in &source.depends {
            let found = crate::dependency::resolve(registry, stored, &mut lookup)?;
            // Only a record that answers can be inside the membership: an unresolved target is
            // never ordinary sequencing, so it is keyed by its stored id and always counted.
            let target = match &found {
                Some(task) => registry.canonical_id(&task.id),
                None => registry.canonical_id(stored),
            };
            if (found.is_some() && member_ids.contains(&target)) || !seen.insert(target) {
                continue;
            }
            out.total += 1;
            match crate::dependency::verdict(found.as_ref()) {
                Verdict::Satisfied => {}
                Verdict::Open => out.open += 1,
                Verdict::Unresolved => out.unresolved += 1,
            }
        }
    }
    Ok((out.total > 0).then_some(out))
}

/// The newest `updated` stamp among the goal and all its members, whatever their
/// status, compared as parsed instants.
pub fn last_activity(goal: &Task, members: &[&Task]) -> Result<String> {
    let mut newest = (crate::time::parse(&goal.updated)?, goal.updated.as_str());
    for task in members {
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

/// A record's effective timestamp : its park's `at` when parked on this host; for an
/// idea awaiting the user, `reviewed_at`, else `updated`; otherwise `updated`.
pub fn effective_at<'a>(task: &'a Task, claims: &'a ClaimSnapshot) -> &'a str {
    if let Some(park) = claims.park(&task.id) {
        return &park.at;
    }
    if task.format >= 2
        && let Some(at) = crate::decisions::earliest_open_decision_at(task)
    {
        return at;
    }
    if task.status == Status::Idea && task.awaiting_user {
        return task.reviewed_at.as_deref().unwrap_or(&task.updated);
    }
    &task.updated
}

/// A record raising a flag, with the timestamp that orders it.
struct Hit {
    at: OffsetDateTime,
    id: String,
    since: String,
}

impl Hit {
    fn new(id: String, since: &str) -> Result<Hit> {
        Ok(Hit {
            at: crate::time::parse(since)?,
            id,
            since: since.to_string(),
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
            since: first.since,
        })
}

/// Parked on this host waiting on the user, or on a dependency or a quiet host: the wait
/// names who or what moves it next, so it is not stalled.
fn waits_on_named(task: &Task, claims: &ClaimSnapshot) -> bool {
    claims.park(&task.id).is_some_and(|park| {
        park.waiting_on == WaitingOn::User
            || matches!(park.reason, Some(Reason::Dependency | Reason::Quiet))
    })
}

/// Whether a `doing` record is stalled, and since when. A goal record (it has members or is
/// a lane) is judged by itself and its members: stalled only when nothing among them is
/// newer than `STALL_AFTER`, nothing among them is held, and no active record among them
/// waits on something named; `since` is the newest stamp among them.
fn stalled_since(
    task: &Task,
    membership: &mut Membership<'_>,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
) -> Result<Option<Hit>> {
    if waits_on_named(task, claims) {
        return Ok(None);
    }
    let members = membership.members(task)?;
    let hit = if hierarchy::is_goal(task, !members.is_empty()) {
        let subtree: Vec<&Task> = std::iter::once(task).chain(&members).collect();
        // Someone holds it, or its work waits on something named: not stalled.
        if subtree.iter().any(|record| {
            claims.held(&record.id).is_some()
                || (hierarchy::is_active(record) && waits_on_named(record, claims))
        }) {
            return Ok(None);
        }
        let mut newest = Hit::new(task.id.to_string(), effective_at(task, claims))?;
        for record in &subtree[1..] {
            let at = effective_at(record, claims);
            let parsed = crate::time::parse(at)?;
            if parsed > newest.at {
                newest.at = parsed;
                newest.since = at.to_string();
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

/// The goal's attention flags over its own record and its active members, or None when it
/// raises none. Each distinct paused lane enclosing the goal or an active member adds one
/// blocked record, unless that lane is itself already a blocked hit.
pub fn attention(
    goal: &Task,
    members: &[Task],
    membership: &mut Membership<'_>,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<Attention>> {
    let records: Vec<&Task> = std::iter::once(goal)
        .chain(members)
        .filter(|task| hierarchy::is_active(task))
        .collect();
    let (mut stalled, mut blocked, mut you) = (Vec::new(), Vec::new(), Vec::new());
    for task in &records {
        let id = || task.id.to_string();
        if task.status == Status::Blocked {
            blocked.push(Hit::new(id(), effective_at(task, claims))?);
        }
        if task.status == Status::Doing
            && let Some(hit) = stalled_since(task, membership, claims, now)?
        {
            stalled.push(hit);
        }
        let parked_on_user = claims
            .park(&task.id)
            .is_some_and(|park| park.waiting_on == WaitingOn::User);
        if parked_on_user
            || crate::decisions::effective_awaiting_user(task)
                && (task.format >= 2 || task.status == Status::Idea)
        {
            you.push(Hit::new(id(), effective_at(task, claims))?);
        }
    }
    let mut paused_lane = Vec::new();
    for record in &records {
        let canonical = registry.canonical_id(&record.id);
        let Some(index) = membership.hierarchy(&canonical.prefix)? else {
            continue;
        };
        let Some(scan) = membership.project(&canonical.prefix)? else {
            continue;
        };
        let Some(at) = index.position(&canonical) else {
            continue;
        };
        let Some(lane_slot) = index.enclosing_lane(at) else {
            continue;
        };
        let lane = &scan[lane_slot];
        if lane.status != Status::Blocked {
            continue;
        }
        let id = registry.canonical_id(&lane.id).to_string();
        if paused_lane.contains(&id) || blocked.iter().any(|hit| hit.id == id) {
            continue;
        }
        blocked.push(Hit::new(id.clone(), effective_at(lane, claims))?);
        paused_lane.push(id);
    }
    let out = Attention {
        stalled: flag(stalled),
        blocked: flag(blocked),
        you: flag(you),
        paused_lane,
    };
    Ok((!out.is_empty()).then_some(out))
}

/// The goals `goal_rows` emits, split as (active, closed): user or mixed origin goals that
/// are open, and, only with `include_closed`, those done inside [`CLOSED_WINDOW`].
/// Shelved, dropped, moved and older done goals are never walked.
pub fn shown<'a>(
    all: &'a [Task],
    hierarchy: &hierarchy::HierarchyView<'a>,
    registry: &Registry,
    now: OffsetDateTime,
    include_closed: bool,
) -> Result<(Vec<&'a Task>, Vec<&'a Task>)> {
    let (mut active, mut closed) = (Vec::new(), Vec::new());
    for task in all {
        let canonical = registry.canonical_id(&task.id);
        if !matches!(task.origin, Some(Origin::User | Origin::Mixed))
            || !hierarchy::is_goal(task, hierarchy.child_count(&canonical) > 0)
        {
            continue;
        }
        match task.status {
            Status::Idea | Status::Todo | Status::Doing | Status::Blocked => active.push(task),
            Status::Done if include_closed => {
                let at = crate::time::parse(closed_at(task))?;
                if now - at < CLOSED_WINDOW {
                    closed.push(task);
                }
            }
            Status::Done | Status::Shelved | Status::Dropped | Status::Moved => {}
        }
    }
    Ok((active, closed))
}

/// The selected rows, ordered within separate open and recently closed sections.
/// Activity means activity among the goal and its members for open rows and closure for
/// closed rows. Open dependencies change neither order nor flags. Closed rows appear only
/// when `include_closed` is set. Every number is over the goal's members across projects.
#[allow(clippy::too_many_arguments)]
pub fn goal_rows<'a>(
    all: &'a [Task],
    hierarchy: &hierarchy::HierarchyView<'a>,
    membership: &mut Membership<'_>,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
    include_closed: bool,
    order: GoalOrder,
) -> Result<Vec<GoalRow>> {
    let mut open: Vec<(OffsetDateTime, String, GoalRow)> = Vec::new();
    let mut closed: Vec<(OffsetDateTime, String, GoalRow)> = Vec::new();
    let (active, recent) = shown(all, hierarchy, registry, now, include_closed)?;
    for task in active.into_iter().chain(recent) {
        let members = membership.members(task)?;
        let refs: Vec<&Task> = members.iter().collect();
        let dependencies = dependencies(task, &refs, registry, |id| membership.lookup(id))?;
        let open_row = task.status != Status::Done;
        let attention = if open_row {
            attention(task, &members, membership, claims, registry, now)?
        } else {
            None
        };
        let stamp = closed_at(task);
        let row = GoalRow {
            id: task.id.to_string(),
            title: task.title.clone(),
            status: task.status,
            origin: task.origin,
            priority: task.priority,
            lane: task.lane,
            project_color: None,
            id_color: None,
            activity: None,
            last_activity: last_activity(task, &refs)?,
            pieces: pieces(&refs),
            dependencies,
            attention,
            closed: (!open_row).then(|| stamp.to_string()),
        };
        let project = registry.canonical_id(&task.id).prefix;
        if open_row {
            open.push((crate::time::parse(&row.last_activity)?, project, row));
        } else {
            closed.push((crate::time::parse(stamp)?, project, row));
        }
    }
    let sort = |rows: &mut Vec<(OffsetDateTime, String, GoalRow)>, count_pieces: bool| {
        rows.sort_by(|(a_at, a_project, a), (b_at, b_project, b)| {
            let primary = match order.sort {
                GoalSort::Project => a_project.cmp(b_project),
                GoalSort::Activity => std::cmp::Ordering::Equal,
                GoalSort::Priority => a.priority.cmp(&b.priority),
            };
            let ordering = primary
                .then_with(|| b_at.cmp(a_at))
                .then_with(|| {
                    if count_pieces {
                        b.pieces.total.cmp(&a.pieces.total)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .then_with(|| a.id.cmp(&b.id));
            if order.reverse {
                ordering.reverse()
            } else {
                ordering
            }
        });
    };
    sort(&mut open, true);
    sort(&mut closed, false);
    Ok(open
        .into_iter()
        .chain(closed)
        .map(|(_, _, row)| row)
        .collect())
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
            decision_id: None,
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
        let registry = Registry::default();
        let mut membership = Membership::new(&registry, &["aa".to_string()], &all);
        let members = membership.members(&goal).unwrap();
        let flags = attention(
            &goal,
            &members,
            &mut membership,
            &ClaimSnapshot::default(),
            &registry,
            now,
        )
        .unwrap();
        assert_eq!(flags, None);
    }

    #[test]
    fn moved_tombstone_under_goal_is_never_flagged() {
        let goal = task("aa-000000", None, Status::Todo);
        let mut moved = task("aa-000001", Some("aa-000000"), Status::Moved);
        moved.updated = "2026-09-01T00:00:00Z".into();
        let park = Park {
            owner: "o".into(),
            session: "s".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-09-01T00:00:00Z".into(),
            next_step: "next".into(),
            waiting_on: WaitingOn::User,
            reason: None,
            needs: None,
            minutes: None,
            decision_id: None,
            title: "aa-000001".into(),
        };
        let claims = ClaimSnapshot::from_parts(
            BTreeMap::new(),
            BTreeMap::from([("aa-000001".to_string(), park)]),
            BTreeMap::new(),
        );
        let all = vec![goal.clone(), moved];
        let now = crate::time::parse("2026-11-01T00:00:00Z").unwrap();
        let registry = Registry::default();
        let mut membership = Membership::new(&registry, &["aa".to_string()], &all);
        let members = membership.members(&goal).unwrap();
        let flags = attention(&goal, &members, &mut membership, &claims, &registry, now).unwrap();
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

    #[test]
    fn shown_rejects_malformed_closed_stamp() {
        let mut goal = task("aa-000000", None, Status::Done);
        goal.origin = Some(Origin::User);
        goal.lane = true;
        goal.updated = "not-a-timestamp".into();
        let all = vec![goal];
        let registry = Registry::default();
        let hierarchy = hierarchy::view_of(&all, &["aa".to_string()], &registry);
        let now = crate::time::parse("2026-11-01T00:00:00Z").unwrap();
        assert!(shown(&all, &hierarchy, &registry, now, true).is_err());
    }
}
