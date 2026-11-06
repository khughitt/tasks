use time::OffsetDateTime;

use crate::model::{Status, Task, TaskId};
use crate::present;
use crate::registry::Registry;
use crate::style::Style;

use super::contract::{
    ClaimInfo, Counts, DeferredInfo, InventoryRow, MultipartOut, Origin, Output, ParkInfo,
    ParkedRow, PeriodicInfo, ProjectColors, RowLocation, RowStatus, TaskSummary, TreeNode,
};

impl InventoryRow {
    pub fn new(origin: Origin, reference: String, location: RowLocation) -> Self {
        Self {
            origin,
            required: false,
            reference,
            location,
            status: RowStatus::Unchecked,
            handle: None,
            relation: None,
            kind: None,
            description: None,
            path: None,
            step: None,
            step_found: None,
            cause: None,
            multipart: None,
        }
    }

    pub fn label(&self) -> &str {
        self.handle.as_deref().unwrap_or_else(|| {
            if self.origin == Origin::Attachment {
                self.reference
                    .strip_prefix("files:")
                    .expect("attachment reference")
            } else {
                self.origin.as_str()
            }
        })
    }
}

pub fn color_of(colors: &ProjectColors, id: &str) -> Option<crate::palette::Rgb> {
    TaskId::parse(id)
        .ok()
        .and_then(|id| colors.get(&id.prefix).copied())
}

fn paint_summary(colors: &ProjectColors, row: &mut TaskSummary) {
    row.project_color = color_of(colors, &row.id);
}

fn paint_tree(colors: &ProjectColors, nodes: &mut [TreeNode]) {
    for node in nodes {
        paint_summary(colors, &mut node.summary);
        paint_tree(colors, &mut node.children);
    }
}

fn paint_lane(colors: &ProjectColors, lane: &mut crate::lanes::LaneRow) {
    paint_summary(colors, &mut lane.lane);
    if let Some(pick) = &mut lane.pick {
        paint_summary(colors, pick);
    }
    lane.active
        .iter_mut()
        .for_each(|row| paint_summary(colors, row));
}

impl Output {
    /// Fill `project_color` on every task row a pretty view of this output shows. `claims`
    /// is left alone: it opens no checkout (docs/user-guide.md).
    pub fn paint_projects(&mut self, colors: &ProjectColors) {
        match self {
            Output::List(o) => o
                .tasks
                .iter_mut()
                .for_each(|row| paint_summary(colors, row)),
            Output::Parked(o) => o
                .tasks
                .iter_mut()
                .for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Quiet(o) => o
                .tasks
                .iter_mut()
                .for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Goals(o) => o
                .goals
                .iter_mut()
                .for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Tree(o) => paint_tree(colors, &mut o.nodes),
            Output::Blockers(o) => o
                .blockers
                .iter_mut()
                .for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Lanes(o) => o.lanes.iter_mut().for_each(|lane| paint_lane(colors, lane)),
            Output::Prime(o) => {
                for row in o
                    .closeout
                    .iter_mut()
                    .chain(o.ready.iter_mut())
                    .chain(o.doing.iter_mut())
                {
                    paint_summary(colors, row);
                }
                for row in &mut o.parked {
                    row.project_color = color_of(colors, &row.id);
                }
                paint_tree(colors, &mut o.roadmap);
                o.lanes.iter_mut().for_each(|lane| paint_lane(colors, lane));
            }
            _ => {}
        }
    }
}

impl ClaimInfo {
    pub fn of(claim: &crate::claims::Claim, live: &crate::claims::Liveness) -> ClaimInfo {
        ClaimInfo {
            owner: claim.owner.clone(),
            session: claim.session.clone(),
            root_session: claim.root_session.clone(),
            host: claim.host.clone(),
            pid: claim.pid,
            worktree: claim.worktree.clone(),
            started: claim.started.clone(),
            seen: claim.seen.clone(),
            live: !live.is_stale(),
            unverified: match live {
                crate::claims::Liveness::Unverifiable(why) => Some(why.clone()),
                _ => None,
            },
            holds: claim.holds.clone(),
        }
    }
}

impl ParkInfo {
    pub fn of(park: &crate::claims::Park) -> ParkInfo {
        ParkInfo {
            at: park.at.clone(),
            next_step: park.next_step.clone(),
            waiting_on: park.waiting_on,
            reason: park.reason,
            needs: park.needs,
            minutes: park.minutes,
            session: park.session.clone(),
            owner: park.owner.clone(),
            host: park.host.clone(),
            worktree: park.worktree.clone(),
        }
    }
}

impl PeriodicInfo {
    pub fn of(task: &Task, now: OffsetDateTime) -> Option<PeriodicInfo> {
        let every = task.every?;
        Some(PeriodicInfo {
            every: every.to_string(),
            last_done: task.last_done.clone(),
            due: crate::periodic::due(task).map(crate::time::format),
            due_now: crate::periodic::is_due(task, now),
        })
    }
}

impl From<&crate::multipart::declaration::Declaration> for MultipartOut {
    fn from(declaration: &crate::multipart::declaration::Declaration) -> Self {
        Self {
            name: declaration.name.clone(),
            bytes: declaration.bytes,
            sha256: declaration.sha256.clone(),
            parts: declaration.parts,
            catalog: crate::multipart::names::catalog_name(&declaration.name),
            caption: declaration.caption.clone(),
        }
    }
}

impl DeferredInfo {
    pub fn of(task: &Task, now: OffsetDateTime) -> Option<DeferredInfo> {
        let until = task.defer?;
        Some(DeferredInfo {
            until: until.to_string(),
            due: crate::defer::is_due(task, now),
        })
    }
}

impl TaskSummary {
    /// `all` is the scan the row came from; counts are computed against it, and `deps` is
    /// that scan's dependency view (docs/user-guide.md).
    pub fn of(
        task: &Task,
        all: &[Task],
        deps: &crate::review::Deps,
        claims: Option<&crate::claims::ClaimSnapshot>,
        registry: &Registry,
        now: OffsetDateTime,
    ) -> TaskSummary {
        let child_count = crate::hierarchy::children(all, &task.id, registry).len();
        let review_reason = crate::review::state(task, child_count, deps, now);
        TaskSummary {
            id: task.id.to_string(),
            title: task.title.clone(),
            project_color: None,
            status: task.status,
            priority: task.priority,
            size: task.size,
            complexity: task.complexity,
            process: task.process,
            parallel: task.parallel,
            lane: task.lane,
            reviewed: task.reviewed,
            awaiting_user: task.awaiting_user,
            reviewed_at: task.reviewed_at.clone(),
            needs_review: review_reason.is_some(),
            review_reason,
            needs: task.needs.clone(),
            owner: task.owner.clone(),
            created: task.created.clone(),
            updated: task.updated.clone(),
            started: task.started.clone(),
            completed: task.completed.clone(),
            closed: task.closed.clone(),
            tags: task.tags.clone(),
            source: task.source.clone(),
            step: task.step.clone(),
            model: task.model.clone(),
            agent: task.agent.clone(),
            origin: task.origin,
            depends: task.depends.iter().map(ToString::to_string).collect(),
            wake_on: task.wake_on.clone(),
            parent: task.parent.as_ref().map(ToString::to_string),
            in_lane: crate::hierarchy::lane_of(all, task, registry),
            child_count,
            open_descendant_count: crate::hierarchy::open_descendants(all, &task.id, registry)
                .len(),
            claim: claims
                .and_then(|snapshot| snapshot.get(&task.id))
                .map(|(claim, live)| ClaimInfo::of(claim, live)),
            park: claims
                .and_then(|snapshot| snapshot.park(&task.id))
                .map(ParkInfo::of),
            escalation: claims
                .and_then(|snapshot| snapshot.escalation(&task.id))
                .cloned(),
            periodic: PeriodicInfo::of(task, now),
            deferred: DeferredInfo::of(task, now),
        }
    }
}

impl ParkedRow {
    pub fn resolved(summary: TaskSummary, phase: crate::model::Phase) -> ParkedRow {
        ParkedRow {
            id: summary.id,
            title: summary.title,
            project_color: summary.project_color,
            status: Some(summary.status),
            priority: Some(summary.priority),
            size: summary.size,
            complexity: summary.complexity,
            process: summary.process,
            parallel: summary.parallel,
            lane: summary.lane,
            reviewed: Some(summary.reviewed),
            needs_review: Some(summary.needs_review),
            review_reason: summary.review_reason,
            needs: summary.needs,
            owner: summary.owner,
            created: Some(summary.created),
            updated: Some(summary.updated),
            started: summary.started,
            completed: summary.completed,
            tags: summary.tags,
            source: summary.source,
            step: summary.step,
            model: summary.model,
            agent: summary.agent,
            origin: summary.origin,
            depends: summary.depends,
            wake_on: summary.wake_on,
            parent: summary.parent,
            in_lane: summary.in_lane,
            child_count: Some(summary.child_count),
            open_descendant_count: Some(summary.open_descendant_count),
            claim: summary.claim,
            park: summary.park,
            escalation: summary.escalation,
            phase: Some(phase),
            deferred: summary.deferred,
        }
    }
    pub fn unresolved(id: &str, park: &crate::claims::Park) -> ParkedRow {
        ParkedRow {
            id: id.into(),
            title: park.title.clone(),
            project_color: None,
            status: None,
            priority: None,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            lane: false,
            reviewed: None,
            needs_review: None,
            review_reason: None,
            needs: Vec::new(),
            owner: None,
            created: None,
            updated: None,
            started: None,
            completed: None,
            tags: Vec::new(),
            source: None,
            step: None,
            model: None,
            agent: None,
            origin: None,
            depends: Vec::new(),
            wake_on: Vec::new(),
            parent: None,
            in_lane: None,
            child_count: None,
            open_descendant_count: None,
            claim: None,
            park: Some(ParkInfo::of(park)),
            escalation: None,
            phase: None,
            deferred: None,
        }
    }
}

impl Counts {
    pub fn of(tasks: &[Task]) -> Counts {
        let mut counts = Counts::default();
        for task in tasks {
            match task.status {
                Status::Idea => counts.idea += 1,
                Status::Todo => counts.todo += 1,
                Status::Doing => counts.doing += 1,
                Status::Blocked => counts.blocked += 1,
                Status::Shelved => counts.shelved += 1,
                Status::Done => counts.done += 1,
                Status::Dropped => counts.dropped += 1,
                Status::Moved => counts.moved += 1,
            }
        }
        counts
    }

    pub fn total(&self) -> usize {
        self.idea
            + self.todo
            + self.doing
            + self.blocked
            + self.shelved
            + self.done
            + self.dropped
            + self.moved
    }
}

/// One column of a status-count row: its header label, the value, and the role the value
/// is painted in. Values come back unpadded and unpainted because ANSI bytes count toward
/// `{:<n}` widths - the caller pads to the column width first, then paints.
pub(super) struct CountColumn {
    pub label: &'static str,
    pub value: usize,
    pub style: Option<Style>,
}

/// The status columns of a counts row, in display order. `total` is always last and always
/// counts every status, so hiding the closed columns never loses a task. One definition for
/// `projects` and `prime`, so the two cannot drift.
pub(super) fn count_columns(counts: &Counts, closed: bool) -> Vec<CountColumn> {
    let mut columns = vec![
        count_column("idea", counts.idea, Status::Idea),
        count_column("todo", counts.todo, Status::Todo),
        count_column("doing", counts.doing, Status::Doing),
        count_column("blocked", counts.blocked, Status::Blocked),
        count_column("shelved", counts.shelved, Status::Shelved),
    ];
    if closed {
        columns.push(count_column("done", counts.done, Status::Done));
        columns.push(count_column("dropped", counts.dropped, Status::Dropped));
        columns.push(count_column("moved", counts.moved, Status::Moved));
    }
    columns.push(CountColumn {
        label: "total",
        value: counts.total(),
        style: None,
    });
    columns
}

fn count_column(label: &'static str, value: usize, status: Status) -> CountColumn {
    CountColumn {
        label,
        value,
        style: present::count(status, value).style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_counts_as_closed_and_in_the_total() {
        let task = crate::history::fixture(
            "---\nid: sci-000001\ntitle: T\nstatus: moved\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-02T00:00:00Z\ndepends: []\ntags: []\nmoved_to: wali-00000b\n---\n",
            "tasks/sci-000001.md",
        ).unwrap();
        let counts = Counts::of(&[task]);
        assert_eq!(counts.moved, 1);
        assert_eq!(counts.total(), 1);
        let columns = count_columns(&counts, true);
        let moved = columns
            .iter()
            .find(|column| column.label == "moved")
            .unwrap();
        assert_eq!(moved.value, 1);
        assert!(matches!(moved.style, Some(Style::Status(Status::Moved))));
        assert!(
            !count_columns(&counts, false)
                .iter()
                .any(|column| column.label == "moved")
        );
    }

    #[test]
    fn claim_info_carries_holds_only_when_the_claim_has_some() {
        let claim = crate::claims::Claim {
            owner: "o".into(),
            session: "s".into(),
            pid: None,
            pid_start: None,
            boot_id: None,
            pid_view: None,
            host: "h".into(),
            worktree: "/w".into(),
            started: "2026-10-03T00:00:00Z".into(),
            seen: "2026-10-03T00:00:00Z".into(),
            holds: Vec::new(),
            root_session: None,
        };
        let live = crate::claims::Liveness::Live;
        let bare = serde_json::to_value(ClaimInfo::of(&claim, &live)).unwrap();
        assert!(bare.get("holds").is_none(), "{bare}");
        assert!(bare.get("root_session").is_none());
        let rooted = crate::claims::Claim {
            root_session: Some("codex:root".into()),
            ..claim.clone()
        };
        assert_eq!(
            serde_json::to_value(ClaimInfo::of(&rooted, &live)).unwrap()["root_session"],
            "codex:root"
        );
        let holding = crate::claims::Claim {
            holds: vec!["quiet".into()],
            ..claim
        };
        let shown = serde_json::to_value(ClaimInfo::of(&holding, &live)).unwrap();
        assert_eq!(shown["holds"], serde_json::json!(["quiet"]));
    }

    #[test]
    fn an_unverifiable_claim_is_held_with_its_reason() {
        let claim = crate::claims::Claim {
            owner: "o".into(),
            session: "s".into(),
            pid: Some(1),
            pid_start: None,
            boot_id: None,
            pid_view: None,
            host: "h".into(),
            worktree: "/w".into(),
            started: "2026-10-03T00:00:00Z".into(),
            seen: "2026-10-03T00:00:00Z".into(),
            holds: Vec::new(),
            root_session: None,
        };
        let why = crate::claims::Liveness::Unverifiable("recorded in another PID view".into());
        let json = serde_json::to_value(ClaimInfo::of(&claim, &why)).unwrap();
        assert_eq!(json["live"], true);
        assert_eq!(json["unverified"], "recorded in another PID view");
        let live =
            serde_json::to_value(ClaimInfo::of(&claim, &crate::claims::Liveness::Live)).unwrap();
        assert!(live.get("unverified").is_none(), "{live}");
    }
}
