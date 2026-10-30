use std::io::IsTerminal;

use crate::error::{Error, Result};
use crate::lanes::{Causes, LaneRow, LaneState};
use crate::model::{Complexity, Process, Size, Status, Task, TaskId};
use crate::present;
use crate::registry::Registry;
use crate::style::{Painter, Style};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Pretty,
}

impl Format {
    /// The output format from `--json`, `--pretty`, and `TASKS_FORMAT`. The variable is
    /// validated whenever it is set: a flag overrides a valid value, never the check on
    /// an invalid one.
    pub fn resolve(json: bool, pretty: bool, tasks_format: Option<&str>) -> Result<Format> {
        let from_env = match tasks_format {
            None | Some("json") => Format::Json,
            Some("pretty") => Format::Pretty,
            Some(other) => {
                return Err(Error::Config(format!(
                    "TASKS_FORMAT must be json or pretty, got {other:?}"
                )));
            }
        };
        Ok(match (json, pretty) {
            (true, _) => Format::Json,
            (false, true) => Format::Pretty,
            (false, false) => from_env,
        })
    }
}

#[derive(Serialize)]
pub struct InitOut {
    pub prefix: String,
    pub root: String,
    /// The records directory `init --records` paired the checkout with; omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Serialize)]
pub struct MoveOut {
    pub id: String,
    pub from: String,
    pub recovery: String,
    pub attachments: Vec<String>,
    pub park: bool,
    pub inbound: Vec<String>,
    pub paths: std::collections::BTreeMap<String, Vec<String>>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct RenameOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub prefix: String,
    pub previous: String,
    pub root: String,
    pub tasks: usize,
    pub parks: usize,
    pub escalations: usize,
    pub aliases: Vec<String>,
    pub recovery: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct IdOut {
    pub id: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DocOut {
    pub body: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct CommitOut {
    pub committed: usize,
    pub paths: Vec<String>,
    pub warnings: Vec<String>,
}

/// `add`'s own shape: the id plus what happened to it. `add` is the one write that can
/// decline to write -- a sourced add whose origin and title already exist reuses that
/// record -- so the caller needs to tell a fresh id from an old one without parsing
/// warnings. Same `id, action, warnings` shape `feedback` already returns.
#[derive(Serialize)]
pub struct AddOut {
    pub id: String,
    /// `created`, or `reused` when `--source` matched an existing task.
    pub action: String,
    /// The creation note's timestamp; absent when `reused` wrote nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct RootOut {
    pub prefix: String,
    pub root: String,
    pub warnings: Vec<String>,
}

/// `tasks where` (ops linked-checkouts §4, §8.1). Every project object carries `root`,
/// `checkout` and `records`; `linked` equals `kind == "linked"`.
#[derive(Serialize)]
pub struct WhereOut {
    pub project: Option<WhereProject>,
    pub holds_records: bool,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct WhereProject {
    pub prefix: String,
    pub root: String,
    pub checkout: String,
    pub records: String,
    pub kind: crate::repo::Kind,
    pub linked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<crate::palette::Rgb>,
}

#[derive(Serialize)]
pub struct ResolveOut {
    pub results: Vec<ResolveRow>,
    pub warnings: Vec<String>,
}

/// One input's answer (spec §3.2). Fields that do not apply are omitted.
#[derive(Serialize, Default)]
pub struct ResolveRow {
    pub input: String,
    pub kind: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The project's records directory; set whenever `root` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<String>,
    /// The resolved project's identity color, when its config sets one and was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<crate::palette::Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<crate::locations::Via>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relpath: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<crate::locations::Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectRow {
    pub prefix: String,
    /// The project's identity color. Absent when it sets none or was not read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<crate::palette::Rgb>,
    pub root: String,
    /// The registry's records pairing, else the root. Always present.
    pub records: String,
    /// In-tree, detached or linked; absent when the config could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::repo::Kind>,
    pub reachable: bool,
    /// Present only for a reachable project.
    pub counts: Option<Counts>,
    /// Every task whatever its status, so it exceeds the sum of the open counts on
    /// purpose: this is the project's size. Absent for an unreachable project, the way
    /// `counts` is - nothing was scanned, which is not the same as a zero.
    pub total: Option<usize>,
    /// The most recent `updated` across every task, closed ones included: closing a task
    /// is activity. Absent when nothing was scanned, and when a scanned project holds no
    /// tasks at all.
    pub last_activity: Option<String>,
    /// The project's `[retired]` table, when it has one. Its counts stay in the row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired: Option<crate::repo::Retirement>,
    /// The project's code checkout. Absent when the config could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `group set` and `group rm`: the group, and the members it now has (set) or had (rm).
#[derive(Serialize)]
pub struct GroupOut {
    pub name: String,
    pub members: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct GroupMember {
    pub prefix: String,
    /// The registered root exists and holds a config: the test `projects` applies.
    pub reachable: bool,
    /// Pretty-only: the colour the member's config sets.
    #[serde(skip)]
    pub color: Option<crate::palette::Rgb>,
}

#[derive(Serialize)]
pub struct GroupRow {
    pub name: String,
    pub members: Vec<GroupMember>,
}

#[derive(Serialize)]
pub struct GroupsOut {
    pub groups: Vec<GroupRow>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct ProjectsOut {
    pub projects: Vec<ProjectRow>,
    pub warnings: Vec<String>,
    /// Which columns the table shows. JSON carries every field either way, so column
    /// visibility never reaches the contract.
    #[serde(skip)]
    pub closed: bool,
    #[serde(skip)]
    pub paths: bool,
}

#[derive(Serialize)]
pub struct DepInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Typed, but serde still emits the same lowercase strings as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    pub resolved: bool,
}

#[derive(Serialize)]
pub struct Related {
    pub id: String,
    pub title: String,
    /// Typed, but serde still emits the same lowercase strings as before.
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Spec,
    Plan,
    Ref,
    Source,
    Attachment,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Plan => "plan",
            Self::Ref => "ref",
            Self::Source => "source",
            Self::Attachment => "attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RowLocation {
    Path,
    Attachment,
    External,
    Opaque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RowStatus {
    Present,
    Missing,
    Pending,
    Special,
    Available,
    Unavailable,
    Unchecked,
}

impl RowStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Pending => "pending",
            Self::Special => "special",
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Unchecked => "unchecked",
        }
    }
}

#[derive(Serialize)]
pub struct InventoryRow {
    pub origin: Origin,
    pub required: bool,
    pub reference: String,
    pub location: RowLocation,
    pub status: RowStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<crate::model::Relation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_found: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

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

/// Everything `show` says about one task, without the warnings, so `next` can embed it.
#[derive(Serialize)]
pub struct ShowFields {
    pub task: Task,
    /// Pretty-only colours of every project the view names (the record's own, its
    /// dependencies' and its move counterpart's), by prefix.
    #[serde(skip)]
    pub project_colors: ProjectColors,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_found: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<DepInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Related>,
    /// The nearest lane at or above the task; omitted when none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_lane: Option<TaskId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Related>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park: Option<ParkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation: Option<crate::claims::Escalation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodic: Option<PeriodicInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deferred: Option<DeferredInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inventory: Vec<InventoryRow>,
}

#[derive(Serialize)]
pub struct ShowOut {
    #[serde(flatten)]
    pub fields: ShowFields,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct NextOut {
    pub next: Option<ShowFields>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub halts: Vec<HaltRow>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct HaltRow {
    pub id: String,
    pub title: String,
    pub owner: Option<String>,
    pub priority: u8,
    #[serde(skip)]
    pub present_locally: bool,
}

/// Each in-scope prefix's configured colour (comfy-tables spec §5).
pub type ProjectColors = std::collections::HashMap<String, crate::palette::Rgb>;

pub fn color_of(colors: &ProjectColors, id: &str) -> Option<crate::palette::Rgb> {
    crate::model::TaskId::parse(id)
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
    /// is left alone: it opens no checkout (comfy-tables spec §5).
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

#[derive(Serialize, Clone)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    #[serde(skip)]
    pub project_color: Option<crate::palette::Rgb>,
    pub status: Status,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<Complexity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    pub parallel: bool,
    /// Marked as a lane; always present, like `parallel`.
    pub lane: bool,
    /// The stored review flag; always present, like `parallel`.
    pub reviewed: bool,
    /// The stored wait-on-user marker; always present, like `reviewed`.
    pub awaiting_user: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
    /// The row needs a look (review-state spec §4). Computed from the scan, never stored.
    pub needs_review: bool,
    /// Why the row needs a look (wait-aware spec §4.2); present exactly when
    /// `needs_review` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_reason: Option<crate::review::ReviewReason>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub created: String,
    pub updated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::model::Origin>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The nearest lane at or above this task: its own id when it is a lane. Computed
    /// from the scan, never stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_lane: Option<TaskId>,
    pub child_count: usize,
    pub open_descendant_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park: Option<ParkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation: Option<crate::claims::Escalation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodic: Option<PeriodicInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deferred: Option<DeferredInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimInfo {
    pub owner: String,
    pub session: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_session: Option<String>,
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub worktree: String,
    pub started: String,
    pub seen: String,
    pub live: bool,
    /// Why a held claim could not be judged from here (claim PID-view design §5). `live`
    /// stays `true` for it, so consumers that read `false` as "free" stay safe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unverified: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
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

/// One row of `tasks claims`: the claim as `prime` shows it, plus the id and the prefix
/// whose store holds it.
#[derive(Serialize)]
pub struct ClaimRow {
    pub id: String,
    pub prefix: String,
    #[serde(flatten)]
    pub claim: ClaimInfo,
}

/// `tasks claims`. No `warnings`: every store is read or the command fails, so there is
/// nothing partial to warn about (ai docs/specs/2026-09-24-turn-boundary-gate-design.md
/// §3.1).
#[derive(Serialize)]
pub struct ClaimsOut {
    pub claims: Vec<ClaimRow>,
}

/// The park entry as JSON: everything but the title snapshot, which is the row's own
/// `title` (spec §5.4).
#[derive(Debug, Clone, Serialize)]
pub struct ParkInfo {
    pub at: String,
    pub next_step: String,
    pub waiting_on: crate::claims::WaitingOn,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<crate::claims::Reason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<crate::claims::Needs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    pub session: String,
    pub owner: String,
    pub host: String,
    pub worktree: String,
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

/// A record's cadence and where it sits in the cycle (spec §5.4). `due_now` is carried
/// explicitly because an absent `due` cannot distinguish "not applicable" from "due now with
/// no anchor".
#[derive(Debug, Clone, Serialize)]
pub struct PeriodicInfo {
    pub every: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_done: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    pub due_now: bool,
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

/// One attachment as `show`, `next`, and `attach` report it. `path` is absolute.
#[derive(Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Serialize)]
pub struct AttachOut {
    pub id: String,
    pub file: FileInfo,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DetachOut {
    pub id: String,
    pub name: String,
    pub path: String,
    /// False when the ledger alone recorded the detach: no file was present.
    pub removed: bool,
    pub warnings: Vec<String>,
}

/// A record's deferral and whether the clock has spent it (spec §5.4). Named apart from
/// the raw `Task.defer` string so the two shapes never collide.
#[derive(Debug, Clone, Serialize)]
pub struct DeferredInfo {
    pub until: String,
    pub due: bool,
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
    /// that scan's dependency view (wait-aware spec §4.3).
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

#[derive(Serialize, Clone)]
pub struct ParkedRow {
    pub id: String,
    pub title: String,
    #[serde(skip)]
    pub project_color: Option<crate::palette::Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<Complexity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    pub parallel: bool,
    /// Marked as a lane; always present, like `parallel`.
    pub lane: bool,
    /// From the record; omitted on an unresolved row (review-state spec §5.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_review: Option<bool>,
    /// Omitted on an unresolved row, as `needs_review` is (wait-aware spec §5.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_reason: Option<crate::review::ReviewReason>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::model::Origin>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_lane: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_descendant_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park: Option<ParkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation: Option<crate::claims::Escalation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<crate::model::Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deferred: Option<DeferredInfo>,
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

#[derive(Serialize)]
pub struct ParkedOut {
    pub tasks: Vec<ParkedRow>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct QuietOut {
    pub tasks: Vec<ParkedRow>,
    pub warnings: Vec<String>,
}

/// Which date a pretty row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateColumn {
    Updated,
    Created,
    Due,
}

#[derive(Serialize)]
pub struct ListOut {
    pub tasks: Vec<TaskSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub halts: Vec<HaltRow>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub date: DateColumn,
}

#[derive(Serialize)]
pub struct TreeNode {
    #[serde(flatten)]
    pub summary: TaskSummary,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

#[derive(Serialize)]
pub struct TreeOut {
    pub nodes: Vec<TreeNode>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct TagRow {
    pub tag: String,
    /// From the dictionary of the project in scope, or of the first registered project
    /// that defines the tag under `--all-projects`; `null` when no dictionary has it.
    pub meaning: Option<String>,
    pub count: usize,
    /// Count per project; one key in local scope.
    pub projects: std::collections::BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct TagsOut {
    pub tags: Vec<TagRow>,
    pub warnings: Vec<String>,
}

/// One task's link to a document: through its `spec`, its `plan`, or a `--ref` entry,
/// which names its handle.
#[derive(Serialize)]
pub struct DocLink {
    pub id: String,
    pub status: String,
    pub via: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
}

#[derive(Serialize)]
pub struct DocRow {
    pub project: String,
    /// spec, plan, brief, handoff, or note.
    pub kind: &'static str,
    /// Relative to the project's records directory, as a task stores the link.
    pub path: String,
    /// The absolute path, to open the document.
    pub file: String,
    /// The first level-one heading, else the file's stem.
    pub title: String,
    /// Tasks of every status, in id order.
    pub tasks: Vec<DocLink>,
}

#[derive(Serialize)]
pub struct DocsOut {
    pub docs: Vec<DocRow>,
    pub warnings: Vec<String>,
    /// A registry-wide scope: pretty paths then name their project.
    #[serde(skip)]
    pub wide: bool,
}

#[derive(Serialize, Default)]
pub struct Counts {
    pub idea: usize,
    pub todo: usize,
    pub doing: usize,
    pub blocked: usize,
    pub shelved: usize,
    pub done: usize,
    pub dropped: usize,
    pub moved: usize,
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
pub struct CountColumn {
    pub label: &'static str,
    pub value: usize,
    pub style: Option<Style>,
}

/// The status columns of a counts row, in display order. `total` is always last and always
/// counts every status, so hiding the closed columns never loses a task. One definition for
/// `projects` and `prime`, so the two cannot drift.
pub fn count_columns(counts: &Counts, closed: bool) -> Vec<CountColumn> {
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

/// What `prime` says about the cadences that are not yet due (spec §5.3). Due records are
/// work, not schedule, and are counted nowhere here.
#[derive(Serialize, Default)]
pub struct PeriodicSummary {
    pub scheduled: usize,
    pub next_due: Option<String>,
    /// Pretty-only, like `PrimeOut::closed`: calendar days from the command's captured
    /// `now` to `next_due` (spec §5.3), so the line can say "in 7d" without reaching for a
    /// second clock.
    #[serde(skip)]
    pub in_days: Option<i64>,
}

/// The `prime` line for deferrals (defer §5.3): what is waiting, the soonest date, and
/// what has come due but still carries its date — the revisit queue.
#[derive(Serialize, Default)]
pub struct DeferredSummary {
    pub waiting: usize,
    pub next: Option<String>,
    pub in_days: Option<i64>,
    pub due: usize,
}

#[derive(Serialize)]
pub struct PrimeSummary {
    pub byte_limit: usize,
    pub complete: bool,
    pub sections: std::collections::BTreeMap<&'static str, PrimeSection>,
    pub full: Vec<String>,
}

#[derive(Serialize)]
pub struct PrimeSection {
    pub total: usize,
    pub shown: usize,
    pub omitted: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_omitted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descendants_omitted: Option<usize>,
}

#[derive(Serialize)]
pub struct PrimeOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<PrimeSummary>,
    /// The local project; null under --all-projects and --group.
    pub prefix: Option<String>,
    /// The group under --group; absent otherwise. `prefix` is then null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Every prefix in scope; one entry locally.
    pub projects: Vec<String>,
    pub counts: Counts,
    pub periodic: PeriodicSummary,
    pub deferred: DeferredSummary,
    /// Pretty-only, like `ProjectsOut`: JSON always carries every count.
    #[serde(skip)]
    pub closed: bool,
    pub ready: Vec<TaskSummary>,
    pub parked: Vec<ParkedRow>,
    pub doing: Vec<TaskSummary>,
    /// Every open lane in scope; always present, `[]` without lanes.
    pub lanes: Vec<LaneRow>,
    pub roadmap: Vec<TreeNode>,
    pub closeout: Vec<TaskSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub halts: Vec<HaltRow>,
    pub warnings: Vec<String>,
}

/// `tasks lanes` (lanes design §5.3).
#[derive(Serialize)]
pub struct LanesOut {
    pub lanes: Vec<LaneRow>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct GraphOut {
    pub format: String,
    pub text: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct Finding {
    pub id: Option<String>,
    pub file: String,
    pub kind: String,
    pub detail: String,
}

/// `tasks retire` / `tasks unretire`: the project and its declaration after the command.
#[derive(Serialize)]
pub struct RetireOut {
    pub prefix: String,
    pub retired: Option<crate::repo::Retirement>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct FeedbackOut {
    pub id: String,
    pub action: String,
    pub path: String,
    /// The `at` of the occurrence note this run wrote (`created` or `feedback from`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct CheckOut {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

/// Whether a blockers row needs the person: `user` only on recorded evidence (a park
/// waiting on the user, or a reviewed idea marked awaiting the user); never "actionable".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Attention {
    User,
    Unknown,
}

/// Where the park and claim evidence on blockers rows came from: this host's stores only.
#[derive(Serialize)]
pub struct ParkEvidence {
    pub host: String,
    pub scope: &'static str,
}

/// One ranked prerequisite (blocker-ranking spec, initial output contract).
#[derive(Serialize)]
pub struct BlockerRow {
    pub rank: usize,
    pub id: String,
    pub title: String,
    #[serde(skip)]
    pub project_color: Option<crate::palette::Rgb>,
    pub status: Status,
    pub priority: u8,
    /// Null for an out-of-scope target that is not a lane: its children were not scanned.
    pub goal: Option<bool>,
    pub cycle: bool,
    pub direct_work_count: usize,
    pub transitive_work_count: usize,
    pub direct_goal_count: usize,
    pub transitive_goal_count: usize,
    pub direct_work_by_status: crate::blockers::WorkByStatus,
    pub transitive_work_by_status: crate::blockers::WorkByStatus,
    pub dependencies_cleared_direct_count: usize,
    pub unmet_dependency_count: usize,
    pub unresolved_dependency_count: usize,
    pub attention: Attention,
    pub awaiting_user: bool,
    /// The task's declared resource needs; never the park's host requirement.
    pub task_needs: Vec<String>,
    /// A deferral still in the future.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub park: Option<ParkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub direct_examples: Vec<TaskId>,
}

#[derive(Serialize)]
pub struct BlockersOut {
    pub observed_at: String,
    pub coverage: crate::blockers::Coverage,
    pub park_evidence: ParkEvidence,
    pub goal_detection: &'static str,
    pub ranking: &'static str,
    pub total: usize,
    pub shown: usize,
    pub omitted: usize,
    pub limit: usize,
    pub blockers: Vec<BlockerRow>,
    pub unresolved: Vec<crate::blockers::UnresolvedDependency>,
    pub cycles: Vec<Vec<TaskId>>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct AttachmentEligibilityOut {
    pub version: u8,
    pub records: std::path::PathBuf,
    pub prefix: String,
    pub config_sha256: String,
    pub attachments: Vec<crate::attachments::EligibilityRow>,
}

/// One variant per command payload. Later tasks add variants; `pretty` grows with them.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Output {
    AttachmentEligibility(AttachmentEligibilityOut),
    Init(InitOut),
    Rename(RenameOut),
    Move(MoveOut),
    Id(IdOut),
    Commit(CommitOut),
    Add(AddOut),
    Root(RootOut),
    Where(WhereOut),
    Resolve(ResolveOut),
    Projects(ProjectsOut),
    Group(GroupOut),
    Groups(GroupsOut),
    Show(Box<ShowOut>),
    Doc(DocOut),
    Next(Box<NextOut>),
    List(ListOut),
    Parked(ParkedOut),
    Quiet(QuietOut),
    Claims(ClaimsOut),
    Prime(Box<PrimeOut>),
    Lanes(LanesOut),
    Blockers(Box<BlockersOut>),
    Graph(GraphOut),
    Check(CheckOut),
    Tree(TreeOut),
    Tags(TagsOut),
    Docs(DocsOut),
    Feedback(FeedbackOut),
    Attach(AttachOut),
    Detach(DetachOut),
    Retire(RetireOut),
}

pub fn render(
    out: &Output,
    format: Format,
    painter: &Painter,
    layout: &crate::table::Layout,
) -> String {
    if let Output::Doc(doc) = out {
        return doc.body.clone();
    }
    match format {
        Format::Json => serde_json::to_string(out).expect("output serializes"),
        Format::Pretty => pretty(out, painter, layout),
    }
}

fn halt_line(halts: &[HaltRow]) -> String {
    if halts.is_empty() {
        return String::new();
    }
    let named = halts
        .iter()
        .map(|halt| {
            let owner = halt
                .owner
                .as_ref()
                .map_or(String::new(), |owner| format!(", owner {owner}"));
            format!("{} (P{}, {}{})", halt.id, halt.priority, halt.title, owner)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let missing = if halts.iter().any(|halt| !halt.present_locally) {
        "; start a halt absent here from its registered checkout"
    } else {
        ""
    };
    format!("halt: {named}; allowed: halt work, linked work, or equally urgent priority{missing}\n")
}

fn pretty(out: &Output, painter: &Painter, layout: &crate::table::Layout) -> String {
    match out {
        Output::AttachmentEligibility(out) => out
            .attachments
            .iter()
            .map(|row| {
                if row.eligible {
                    format!("{}: eligible", row.path)
                } else {
                    format!(
                        "{}: ineligible\n{}",
                        row.path,
                        row.problems
                            .iter()
                            .map(|p| p.line())
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Init(o) => o.prefix.clone(),
        Output::Retire(o) => match &o.retired {
            Some(r) => {
                let successor = r
                    .successor
                    .as_ref()
                    .map(|s| format!(" → {s}"))
                    .unwrap_or_default();
                format!("{} retired {}{successor}: {}", o.prefix, r.date, r.reason)
            }
            None => format!("{} is not retired", o.prefix),
        },
        Output::Move(o) => {
            let mut text = format!("{} → {}", o.from, o.id);
            for id in &o.inbound {
                text.push_str(&format!("\n  inbound: {id}"));
            }
            text
        }
        Output::Rename(o) => o.prefix.clone(),
        Output::Id(o) => o.id.clone(),
        Output::Commit(o) => {
            if o.committed > 0 {
                format!("committed {} records", o.committed)
            } else {
                "nothing to commit".into()
            }
        }
        Output::Add(o) => o.id.clone(),
        Output::Root(o) => o.root.clone(),
        Output::Where(o) => match &o.project {
            None => "no project".into(),
            Some(p) => {
                let prefix = present::project(&p.prefix, p.color).paint(painter);
                match p.kind {
                    crate::repo::Kind::Linked => {
                        format!("{prefix} {} (linked to {})", p.root, p.checkout)
                    }
                    crate::repo::Kind::Detached => {
                        format!("{prefix} {} (records at {})", p.root, p.records)
                    }
                    crate::repo::Kind::InTree => format!("{prefix} {}", p.root),
                }
            }
        },

        Output::Resolve(o) => o
            .results
            .iter()
            .map(|r| {
                let answer = r.id.as_deref().or(r.prefix.as_deref()).unwrap_or("");
                let via = r
                    .via
                    .map(|v| {
                        serde_json::to_value(v)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .unwrap_or_default();
                [r.input.as_str(), r.status, answer, via.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("  ")
            })
            .collect::<Vec<_>>()
            .join("\n"),

        Output::Group(o) => o.name.clone(),
        Output::Groups(o) => o
            .groups
            .iter()
            .map(|row| {
                let members: Vec<String> = row
                    .members
                    .iter()
                    .map(|member| {
                        let prefix = present::project(&member.prefix, member.color).paint(painter);
                        if member.reachable {
                            prefix
                        } else {
                            format!("{prefix} (unreachable)")
                        }
                    })
                    .collect();
                format!("{}  {}", row.name, members.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Projects(o) if o.projects.is_empty() => String::new(),
        Output::Projects(o) => {
            use crate::table::{Column, Row};
            let mut columns = vec![Column::fixed("project")];
            for column in count_columns(&Counts::default(), o.closed) {
                columns.push(Column::right(column.label));
            }
            columns.push(Column::fixed("activity"));
            if o.paths {
                columns.push(Column::fixed("root"));
            }
            let rows: Vec<Row> = o
                .projects
                .iter()
                .map(|row| {
                    let mut cells = vec![present::project(&row.prefix, row.color).paint(painter)];
                    match &row.counts {
                        Some(counts) => cells.extend(
                            count_columns(counts, o.closed).into_iter().map(|column| {
                                let value = column.value.to_string();
                                column
                                    .style
                                    .map_or(value.clone(), |style| painter.paint(style, &value))
                            }),
                        ),
                        None => cells.extend(
                            count_columns(&Counts::default(), o.closed)
                                .iter()
                                .map(|_| painter.paint(Style::Chrome, "-")),
                        ),
                    }
                    cells.push(match (&row.counts, &row.last_activity) {
                        (None, _) => painter.paint(Style::Error, "unreachable"),
                        (Some(_), _) if row.retired.is_some() => {
                            painter.paint(Style::Chrome, "retired")
                        }
                        (Some(_), Some(at)) => present::date(at).paint(painter),
                        (Some(_), None) => painter.paint(Style::Chrome, "-"),
                    });
                    if o.paths {
                        cells.push(match row.kind {
                            Some(crate::repo::Kind::Detached) => {
                                format!("{} → {}", row.root, row.records)
                            }
                            _ => row.root.clone(),
                        });
                    }
                    Row::new(cells)
                })
                .collect();
            let always = crate::table::Layout {
                headers: true,
                ..*layout
            };
            crate::table::render(&columns, &rows, &always, None, painter)
        }
        Output::Show(o) => crate::show_text::render(&o.fields, painter, layout.width),
        Output::Doc(o) => o.body.clone(),
        Output::Next(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            match &o.next {
                Some(fields) => crate::show_text::render(fields, painter, layout.width),
                None => "nothing ready".into(),
            }
        ),
        Output::List(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            task_table(&o.tasks, o.date, painter, layout)
        ),
        Output::Parked(o) => parked_table(
            &o.tasks,
            painter,
            layout,
            None,
            o.tasks.iter().any(|row| row.needs_review == Some(true)),
        ),
        Output::Quiet(o) => quiet_table(&o.tasks, painter, layout),
        Output::Claims(o) => {
            use crate::table::{Column, Row};
            let columns = [
                Column::fixed("id"),
                Column::fixed("session"),
                Column::fixed("live"),
                Column::fixed("worktree"),
            ];
            let rows: Vec<Row> = o
                .claims
                .iter()
                .map(|row| {
                    Row::new(vec![
                        row.id.clone(),
                        row.claim.session.clone(),
                        match (&row.claim.unverified, row.claim.live) {
                            (Some(_), _) => "unverifiable",
                            (None, true) => "live",
                            (None, false) => "stale",
                        }
                        .into(),
                        row.claim.worktree.clone(),
                    ])
                })
                .collect();
            crate::table::render(&columns, &rows, layout, None, painter)
                .trim_end_matches('\n')
                .to_string()
        }
        Output::Prime(o) => {
            let mut marks = Marks::of(&o.closeout)
                .or(Marks::of_tree(&o.roadmap))
                .or(Marks::of(&o.ready))
                .or(Marks::of(&o.doing));
            marks.review |= o.parked.iter().any(|row| row.needs_review == Some(true));
            let columns = task_columns(marks);
            let rows_of = |rows: &[TaskSummary]| -> Vec<crate::table::Row> {
                rows.iter()
                    .map(|row| task_row(row, DateColumn::Updated, painter, marks, 0))
                    .collect()
            };
            let closeout = rows_of(&o.closeout);
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut roadmap = Vec::new();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    tree_rows(std::slice::from_ref(node), 0, painter, marks, &mut roadmap);
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    roadmap.push(task_row(
                        &node.summary,
                        DateColumn::Updated,
                        painter,
                        marks,
                        0,
                    ));
                }
            }
            let ready = rows_of(&o.ready);
            let doing = rows_of(&o.doing);
            let mut shared = crate::table::shared_widths(
                &columns,
                &[&closeout, &roadmap, &ready, &doing],
                layout.headers,
            );
            let pad = if marks.review { 2 } else { 0 };
            let id = shared
                .id()
                .unwrap_or(0)
                .max(
                    o.parked
                        .iter()
                        .map(|row| row.id.len() + pad)
                        .max()
                        .unwrap_or(0),
                )
                .max(
                    o.lanes
                        .iter()
                        .map(|row| row.lane.id.len() + pad)
                        .max()
                        .unwrap_or(0),
                );
            shared.0[0] = Some(id);
            let section = |rows: &[crate::table::Row]| {
                crate::table::render(&columns, rows, layout, Some(&shared), painter)
            };
            let header = match (&o.prefix, &o.group) {
                (Some(prefix), _) => format!("project {prefix}"),
                (None, Some(group)) => format!("group {group}: projects {}", o.projects.join(", ")),
                (None, None) => format!("projects {}", o.projects.join(", ")),
            };
            // One row, so labels stay beside their values instead of over them - but the
            // columns and their colors are the same definition `projects` renders.
            let counts: Vec<String> = count_columns(&o.counts, o.closed)
                .into_iter()
                .map(|column| {
                    let value = column.value.to_string();
                    let value = match column.style {
                        Some(style) => painter.paint(style, &value),
                        None => value,
                    };
                    format!("{} {value}", column.label)
                })
                .collect();
            let mut rendered = format!("{}{header}\n{}\n", halt_line(&o.halts), counts.join("  "));
            if o.periodic.scheduled > 0 {
                let next = o
                    .periodic
                    .next_due
                    .as_deref()
                    .expect("scheduled recurrences have a next due date");
                let days = o
                    .periodic
                    .in_days
                    .expect("a next due date has a relative day count");
                rendered.push_str(&format!(
                    "periodic: {} scheduled, next due {} (in {days}d)\n",
                    o.periodic.scheduled,
                    crate::time::day(next)
                ));
            }
            if o.deferred.waiting > 0 || o.deferred.due > 0 {
                let mut line = match (&o.deferred.next, o.deferred.in_days) {
                    (Some(next), Some(days)) => {
                        format!(
                            "deferred: {} waiting, next {next} (in {days}d)",
                            o.deferred.waiting
                        )
                    }
                    _ => format!("deferred: {} waiting", o.deferred.waiting),
                };
                if o.deferred.due > 0 {
                    line.push_str(&format!("; {} due", o.deferred.due));
                }
                rendered.push_str(&line);
                rendered.push('\n');
            }

            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "closeout:")
            ));
            rendered.push_str(&section(&closeout));
            if !o.lanes.is_empty() {
                rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "lanes:")));
                rendered.push_str(&lane_table(
                    &o.lanes,
                    painter,
                    layout,
                    Some(&crate::table::Widths::id_only(id, 4)),
                    false,
                    marks.review,
                ));
            }
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "roadmap:")
            ));
            rendered.push_str(&section(&roadmap));
            rendered.push_str(&format!(
                "{listed_under_ready} childless root(s) are listed under ready\n"
            ));
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "parked:")
            ));
            rendered.push_str(&parked_table(
                &o.parked,
                painter,
                layout,
                Some(&crate::table::Widths::id_only(id, 7)),
                marks.review,
            ));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "ready:")));
            rendered.push_str(&section(&ready));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "doing:")));
            rendered.push_str(&section(&doing));
            rendered
        }
        Output::Lanes(o) => lane_table(&o.lanes, painter, layout, None, true, false),
        Output::Blockers(o) => crate::blockers_text::render(o, painter, layout),
        Output::Graph(o) => o.text.clone(),
        Output::Check(o) => {
            let mut rendered = String::new();
            for finding in &o.errors {
                let line = format!(
                    "error: {} [{}] {}",
                    finding.file, finding.kind, finding.detail
                );
                rendered.push_str(&painter.paint(Style::Error, &line));
                rendered.push('\n');
            }
            if o.errors.is_empty() {
                rendered.push_str(&painter.paint(Style::Ok, "ok"));
                rendered.push('\n');
            }
            rendered
        }
        Output::Tree(o) => {
            let marks = Marks::of_tree(&o.nodes);
            let mut rows = Vec::new();
            tree_rows(&o.nodes, 0, painter, marks, &mut rows);
            crate::table::render(&task_columns(marks), &rows, layout, None, painter)
        }
        Output::Tags(o) => {
            let mut rendered = String::new();
            for row in &o.tags {
                let parts: Vec<String> = row
                    .projects
                    .iter()
                    .map(|(prefix, count)| format!("{prefix} {count}"))
                    .collect();
                let breakdown = painter.paint(Style::Chrome, &format!("  ({})", parts.join(", ")));
                let meaning = row
                    .meaning
                    .as_deref()
                    .map(|meaning| format!("  {meaning}"))
                    .unwrap_or_default();
                rendered.push_str(&format!(
                    "{:>4}  {}{meaning}{breakdown}\n",
                    row.count, row.tag
                ));
            }
            rendered
        }
        Output::Docs(o) => {
            use crate::table::{Column, Detail, Row};
            let columns = [
                Column::fixed("kind"),
                Column::fixed("path"),
                Column::wrap("title", 20, None),
            ];
            let rows: Vec<Row> = o
                .docs
                .iter()
                .map(|doc| {
                    let path = if o.wide {
                        format!("{}:{}", doc.project, doc.path)
                    } else {
                        doc.path.clone()
                    };
                    let mut row = Row::new(vec![doc.kind.to_string(), path, doc.title.clone()]);
                    if !doc.tasks.is_empty() {
                        let links: Vec<String> = doc
                            .tasks
                            .iter()
                            .map(|link| match &link.handle {
                                Some(handle) => format!("{} {} {handle}", link.id, link.via),
                                None => format!("{} {}", link.id, link.via),
                            })
                            .collect();
                        row.details.push(("tasks", Detail::Wrap(links.join(", "))));
                    }
                    row
                })
                .collect();
            crate::table::render(&columns, &rows, layout, None, painter)
        }
        Output::Feedback(o) => format!("{} {}", o.action, o.id),
        Output::Attach(o) => o.file.path.clone(),
        Output::Detach(o) if o.removed => format!(
            "removed {}; git history keeps committed bytes, uncommitted ones are gone",
            o.path
        ),
        Output::Detach(o) => format!("recorded {} as detached; no file was present", o.name),
    }
}

/// One row per lane: id, priority, title, then its state with the pick or its main
/// cause (lanes design §5.3). `detail` adds the guidance and the active claims under each
/// row, as `tasks lanes` prints them; `prime` leaves them out.
fn lane_table(
    rows: &[LaneRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
    detail: bool,
    pad: bool,
) -> String {
    use crate::table::{Column, Detail, Row};
    let columns = [
        Column::fixed("id"),
        Column::fixed("P"),
        Column::wrap("title", 20, None),
        Column::wrap("state", 20, None),
    ];
    let task = |row: &TaskSummary| {
        format!(
            "{} {}",
            present::id(&row.id, row.project_color).paint(painter),
            present::title(&row.title, row.project_color).paint(painter)
        )
    };
    let built: Vec<Row> = rows
        .iter()
        .map(|row| {
            let state = match row.state {
                LaneState::Ready => {
                    let pick = row.pick.as_ref().expect("a ready lane has a pick");
                    format!(
                        "{} {}",
                        painter.paint(Style::Emphasis, "ready →"),
                        task(pick)
                    )
                }
                LaneState::Held => {
                    let first = row
                        .held
                        .first()
                        .expect("a held lane names what it waits for");
                    painter.paint(
                        Style::Emphasis,
                        &format!("held: {} ← {}", first.need, first.holder),
                    )
                }
                LaneState::Waiting => painter.paint(Style::Emphasis, &waiting_text(&row.causes)),
                LaneState::Paused => painter.paint(Style::Emphasis, "paused"),
                LaneState::Empty => painter.paint(Style::Emphasis, "empty"),
            };
            let mut built = Row::new(vec![
                format!(
                    "{}{}",
                    if pad { "  " } else { "" },
                    present::id(&row.lane.id, row.lane.project_color).paint(painter)
                ),
                present::priority(row.lane.priority).paint(painter),
                present::title(&row.lane.title, row.lane.project_color).paint(painter),
                state,
            ]);
            if detail {
                if let Some(guidance) = &row.guidance {
                    built
                        .details
                        .push(("guide", Detail::Wrap(guidance.clone())));
                }
                for active in &row.active {
                    let holder = active
                        .claim
                        .as_ref()
                        .map(|claim| {
                            painter.paint(
                                Style::Chrome,
                                &format!(" @{} [{}]", claim.owner, claim.session),
                            )
                        })
                        .unwrap_or_default();
                    built
                        .details
                        .push(("active", Detail::Wrap(format!("{}{holder}", task(active)))));
                }
            }
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, widths, painter)
}

/// `waiting: 2 user, 1 deferred`: every cause with its count, the largest first, ties in
/// table order.
fn waiting_text(causes: &Causes) -> String {
    let mut entries = causes.entries();
    entries.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let parts: Vec<String> = entries
        .iter()
        .map(|(cause, count)| format!("{count} {cause}"))
        .collect();
    format!("waiting: {}", parts.join(", "))
}

/// The optional marks a pretty task table reserves, decided once per command output so a
/// per-section decision cannot shift columns between adjacent sections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// The `* ` prefix on the id cell (review-state spec §5.3).
    pub review: bool,
    /// The `||` column.
    pub parallel: bool,
    /// The type column.
    pub kind: bool,
}

impl Marks {
    pub fn of(rows: &[TaskSummary]) -> Marks {
        Marks {
            review: rows.iter().any(|row| row.needs_review),
            parallel: any_parallel(rows),
            kind: any_type(rows),
        }
    }

    pub fn of_tree(nodes: &[TreeNode]) -> Marks {
        Marks {
            review: any_needs_review_tree(nodes),
            parallel: any_parallel_tree(nodes),
            kind: any_type_tree(nodes),
        }
    }

    pub fn or(self, other: Marks) -> Marks {
        Marks {
            review: self.review || other.review,
            parallel: self.parallel || other.parallel,
            kind: self.kind || other.kind,
        }
    }
}

pub fn any_needs_review_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.needs_review || any_needs_review_tree(&node.children))
}

/// Whether a pretty rendering must reserve the parallel column. Decided once per command
/// output so a per-section decision cannot shift dates between adjacent siblings.
pub fn any_parallel(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| row.parallel)
}

pub fn any_parallel_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.parallel || any_parallel_tree(&node.children))
}

/// The one-letter type marker for a summary row: `≡` for a lane, `p` for a record
/// carrying a cadence, and nothing otherwise. A lane is a goal and never recurs, so the
/// two never compete for the slot. A future type is another arm here, another letter in
/// the same slot; the column is reserved once per output (see `any_type`), so it is never
/// a layout change.
fn type_letter(row: &TaskSummary) -> Option<char> {
    if row.lane {
        return Some('≡');
    }
    row.periodic.as_ref().map(|_| 'p')
}

/// Whether a pretty rendering must reserve the type column. Same once-per-output rule as
/// `any_parallel`, so dates stay aligned between adjacent siblings.
pub fn any_type(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| type_letter(row).is_some())
}

pub fn any_type_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| type_letter(&node.summary).is_some() || any_type_tree(&node.children))
}

/// Whether a pretty rendering of `out` paints from the theme's colors, and so whether
/// the stdout painter needs them (date spec §3.1; priority spec §3.1). Every output showing
/// a priority also shows dates, including show and next.
pub fn needs_theme(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Lanes(_)
            | Output::Blockers(_)
            | Output::Tree(_)
            | Output::Parked(_)
            | Output::Quiet(_)
            | Output::Projects(_)
            | Output::Show(_)
            | Output::Next(_)
    )
}

/// Whether a pretty rendering of `out` shows priorities, and so whether a palette
/// without magenta costs it anything (priority spec §3.3).
pub fn shows_priority(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Lanes(_)
            | Output::Blockers(_)
            | Output::Tree(_)
            | Output::Quiet(_)
            | Output::Show(_)
            | Output::Next(_)
    )
}

/// The width pretty tables wrap at: `COLUMNS` overrides the terminal attached to stdout
/// (TIOCGWINSZ). With neither, rows keep one line each, so piped output stays
/// line-oriented; a default width of 80 would break line-oriented pipes.
pub fn terminal_width() -> Result<Option<usize>> {
    let columns = match std::env::var("COLUMNS") {
        Ok(text) => Some(text),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(value)) => {
            return Err(Error::Config(format!(
                "COLUMNS must be valid UTF-8, got {value:?}"
            )));
        }
    };
    wrap_width(columns.as_deref(), terminal_attached_width())
}

/// `COLUMNS` wins over the attached terminal whenever it is set.
fn wrap_width(columns: Option<&str>, attached: Option<usize>) -> Result<Option<usize>> {
    match columns {
        Some(text) => {
            let width: usize = text.parse().map_err(|_| {
                Error::Config(format!("COLUMNS must be a positive integer, got {text:?}"))
            })?;
            if width == 0 {
                return Err(Error::Config(
                    "COLUMNS must be a positive integer, got 0".into(),
                ));
            }
            Ok(Some(width))
        }
        None => Ok(attached),
    }
}

fn terminal_attached_width() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    // SAFETY: `ioctl` with TIOCGWINSZ only writes through the pointed-to `winsize`.
    let mut size = unsafe { std::mem::zeroed::<libc::winsize>() };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0
        && size.ws_col > 0
    {
        Some(size.ws_col as usize)
    } else {
        None
    }
}

pub fn task_columns(marks: Marks) -> Vec<crate::table::Column> {
    use crate::table::Column;
    let mut columns = vec![
        Column::fixed("id"),
        Column::fixed("P"),
        Column::fixed("size"),
        Column::fixed("cx"),
        Column::fixed("process"),
        Column::fixed("status"),
    ];
    if marks.parallel {
        columns.push(Column::fixed("||"));
    }
    if marks.kind {
        columns.push(Column::fixed("type"));
    }
    columns.push(Column::fixed("date"));
    columns.push(Column::wrap("title", 20, None));
    columns
}

pub fn task_row(
    row: &TaskSummary,
    date: DateColumn,
    painter: &Painter,
    marks: Marks,
    indent: usize,
) -> crate::table::Row {
    let date = match date {
        DateColumn::Updated => Some(present::date(&row.updated)),
        DateColumn::Created => Some(present::date(&row.created)),
        DateColumn::Due => match (&row.periodic, &row.deferred) {
            (Some(periodic), _) => match (&periodic.due, periodic.due_now) {
                (Some(due), _) => Some(present::date(due)),
                (None, true) => Some(present::today()),
                (None, false) => None,
            },
            (None, Some(deferred)) => Some(present::date(&deferred.until)),
            (None, None) => None,
        },
    }
    .map(|span| span.paint(painter))
    .unwrap_or_else(|| "-".into());
    let mut cells = vec![
        format!(
            "{}{}{}",
            present::review_mark(row.needs_review, marks.review).paint(painter),
            "  ".repeat(indent),
            present::id(&row.id, row.project_color).paint(painter)
        ),
        present::priority(row.priority).paint(painter),
        present::size(row.size).paint(painter),
        present::complexity(row.complexity).paint(painter),
        present::process(row.process, "-").paint(painter),
        present::status(row.status).paint(painter),
    ];
    if marks.parallel {
        cells.push(if row.parallel {
            "||".into()
        } else {
            String::new()
        });
    }
    if marks.kind {
        cells.push(
            type_letter(row)
                .map(|letter| painter.paint(Style::Emphasis, &letter.to_string()))
                .unwrap_or_default(),
        );
    }
    cells.push(date);
    cells.push(title_cell(row, painter));
    crate::table::Row::new(cells)
}

/// The title and the suffixes that ride with it: tags, a due cadence, a deferral, a quiet
/// recipe, the owner.
fn title_cell(row: &TaskSummary, painter: &Painter) -> String {
    let mut cell = present::title(&row.title, row.project_color).paint(painter);
    if !row.tags.is_empty() {
        let tags = present::tags(&row.tags);
        cell.push(' ');
        cell.push_str(
            &present::Span {
                text: format!("[{}]", tags.text),
                ..tags
            }
            .paint(painter),
        );
    }
    if let Some(periodic) = row.periodic.as_ref().filter(|periodic| periodic.due_now) {
        let when = periodic
            .due
            .as_deref()
            .map(|due| crate::time::day(due).to_string())
            .unwrap_or_else(|| "now".into());
        cell.push_str("  ");
        cell.push_str(&painter.paint(
            Style::Emphasis,
            &format!("every {}, due {when}", periodic.every),
        ));
    }
    if let Some(deferred) = &row.deferred {
        let text = if deferred.due {
            format!("due {}", deferred.until)
        } else {
            format!("defer {}", deferred.until)
        };
        cell.push_str("  ");
        cell.push_str(&painter.paint(Style::Emphasis, &text));
    }
    if let Some(park) = row
        .park
        .as_ref()
        .filter(|park| park.reason == Some(crate::claims::Reason::Quiet))
    {
        let text = format!(
            "waits on {}",
            crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes)
        );
        cell.push_str("  ");
        cell.push_str(&painter.paint(Style::Emphasis, &text));
    }
    let owner = match &row.claim {
        Some(claim) => Some(present::claim(claim)),
        None => row.owner.as_deref().map(present::owner),
    };
    if let Some(owner) = owner {
        cell.push(' ');
        cell.push_str(
            &present::Span {
                text: format!("@{}", owner.text),
                ..owner
            }
            .paint(painter),
        );
    }
    cell
}

pub fn tree_rows(
    nodes: &[TreeNode],
    depth: usize,
    painter: &Painter,
    marks: Marks,
    rows: &mut Vec<crate::table::Row>,
) {
    for node in nodes {
        rows.push(task_row(
            &node.summary,
            DateColumn::Updated,
            painter,
            marks,
            depth,
        ));
        tree_rows(&node.children, depth + 1, painter, marks, rows);
    }
}

fn task_table(
    rows: &[TaskSummary],
    date: DateColumn,
    painter: &Painter,
    layout: &crate::table::Layout,
) -> String {
    let marks = Marks::of(rows);
    let built: Vec<crate::table::Row> = rows
        .iter()
        .map(|row| task_row(row, date, painter, marks, 0))
        .collect();
    crate::table::render(&task_columns(marks), &built, layout, None, painter)
}

pub fn parked_columns() -> Vec<crate::table::Column> {
    use crate::table::Column;
    vec![
        Column::fixed("id"),
        Column::fixed("status"),
        Column::fixed("phase"),
        Column::fixed("waits on"),
        Column::fixed("parked"),
        Column::wrap("title", 20, Some(50)),
        Column::wrap("next", 40, None),
    ]
}

pub fn parked_row(row: &ParkedRow, painter: &Painter, review: bool) -> crate::table::Row {
    let park = row.park.as_ref().expect("a parked row carries its park");
    crate::table::Row::new(vec![
        format!(
            "{}{}",
            present::review_mark(row.needs_review == Some(true), review).paint(painter),
            present::id(&row.id, row.project_color).paint(painter)
        ),
        present::status_of(row.status).paint(painter),
        row.phase
            .map(crate::model::Phase::as_str)
            .unwrap_or("-")
            .to_string(),
        crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes),
        present::date(&park.at).paint(painter),
        present::title(&row.title, row.project_color).paint(painter),
        painter.paint(Style::Chrome, &park.next_step),
    ])
}

pub fn parked_table(
    rows: &[ParkedRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
    review: bool,
) -> String {
    let built: Vec<crate::table::Row> = rows
        .iter()
        .filter(|row| row.park.is_some())
        .map(|row| parked_row(row, painter, review))
        .collect();
    crate::table::render(&parked_columns(), &built, layout, widths, painter)
}

pub fn quiet_table(rows: &[ParkedRow], painter: &Painter, layout: &crate::table::Layout) -> String {
    use crate::table::{Column, Detail, Row};
    let columns = [
        Column::fixed("id"),
        Column::fixed("P"),
        Column::fixed("needs"),
        Column::right("min"),
        Column::fixed("parked"),
        Column::wrap("title", 20, Some(50)),
        Column::wrap("next", 40, None),
    ];
    let built: Vec<Row> = rows
        .iter()
        .filter_map(|row| row.park.as_ref().map(|park| (row, park)))
        .map(|(row, park)| {
            let mut built = Row::new(vec![
                present::id(&row.id, row.project_color).paint(painter),
                match row.priority {
                    Some(priority) => present::priority(priority).paint(painter),
                    None => "P-".into(),
                },
                park.needs
                    .map(crate::claims::Needs::as_str)
                    .unwrap_or("-")
                    .to_string(),
                park.minutes
                    .map_or_else(|| "-".into(), |minutes| format!("{minutes} min")),
                present::date(&park.at).paint(painter),
                present::title(&row.title, row.project_color).paint(painter),
                painter.paint(Style::Chrome, &park.next_step),
            ]);
            built
                .details
                .push(("in", Detail::Whole(park.worktree.clone())));
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, None, painter)
}

pub fn render_error(e: &Error) -> String {
    serde_json::json!({ "error": { "kind": e.kind(), "detail": e.to_string() } }).to_string()
}

/// stderr text for warnings in pretty mode: a `── warnings (n)` rule, then one per line.
pub fn pretty_warnings(warnings: &[String], painter: &Painter, width: Option<usize>) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut rendered = present::rule(
        &format!("warnings ({})", warnings.len()),
        painter,
        width.unwrap_or(crate::show_text::MEASURE),
    );
    for warning in warnings {
        rendered.push_str(warning);
        rendered.push('\n');
    }
    rendered
}

pub fn warnings_of(out: &Output) -> Vec<String> {
    match out {
        Output::AttachmentEligibility(_) => Vec::new(),
        Output::Init(o) => o.warnings.clone(),
        Output::Retire(o) => o.warnings.clone(),
        Output::Rename(o) => o.warnings.clone(),
        Output::Move(o) => o.warnings.clone(),
        Output::Id(o) => o.warnings.clone(),
        Output::Commit(o) => o.warnings.clone(),
        Output::Add(o) => o.warnings.clone(),
        Output::Root(o) => o.warnings.clone(),
        Output::Where(o) => o.warnings.clone(),
        Output::Resolve(o) => o.warnings.clone(),
        Output::Group(o) => o.warnings.clone(),
        Output::Groups(o) => o.warnings.clone(),
        Output::Projects(o) => o.warnings.clone(),
        Output::Show(o) => o.warnings.clone(),
        Output::Doc(o) => o.warnings.clone(),
        Output::Next(o) => o.warnings.clone(),
        Output::List(o) => o.warnings.clone(),
        Output::Parked(o) => o.warnings.clone(),
        Output::Quiet(o) => o.warnings.clone(),
        Output::Claims(_) => Vec::new(),
        Output::Prime(o) => o.warnings.clone(),
        Output::Lanes(o) => o.warnings.clone(),
        Output::Blockers(o) => o.warnings.clone(),
        Output::Graph(o) => o.warnings.clone(),
        Output::Check(o) => o
            .warnings
            .iter()
            .map(|finding| format!("{} [{}] {}", finding.file, finding.kind, finding.detail))
            .collect(),
        Output::Tree(o) => o.warnings.clone(),
        Output::Tags(o) => o.warnings.clone(),
        Output::Docs(o) => o.warnings.clone(),
        Output::Feedback(o) => o.warnings.clone(),
        Output::Attach(o) => o.warnings.clone(),
        Output::Detach(o) => o.warnings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorMode;

    #[test]
    fn moved_counts_as_closed_and_in_the_total() {
        let task = crate::format::parse_task(
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

    fn row(id: &str, parallel: bool) -> TaskSummary {
        TaskSummary {
            id: id.into(),
            title: format!("title {id}"),
            project_color: None,
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            owner: None,
            created: "2026-09-06T00:00:00Z".into(),
            updated: "2026-09-06T00:00:00Z".into(),
            started: None,
            completed: None,
            tags: vec![],
            source: None,
            step: None,
            model: None,
            agent: None,
            origin: None,
            depends: vec![],
            parent: None,
            in_lane: None,
            child_count: 0,
            open_descendant_count: 0,
            claim: None,
            park: None,
            escalation: None,
            periodic: None,
            deferred: None,
            needs: vec![],
            parallel,
            lane: false,
            reviewed: false,
            awaiting_user: false,
            reviewed_at: None,
            closed: None,
            needs_review: false,
            review_reason: None,
        }
    }

    fn recurring(id: &str) -> TaskSummary {
        let mut row = row(id, false);
        row.periodic = Some(PeriodicInfo {
            every: "30d".into(),
            last_done: None,
            due: None,
            due_now: true,
        });
        row
    }

    fn layout_at(width: usize) -> crate::table::Layout {
        crate::table::Layout {
            width: Some(width),
            headers: false,
        }
    }

    fn render_tasks(
        rows: &[TaskSummary],
        date: DateColumn,
        painter: &Painter,
        parallel: bool,
        kind: bool,
        layout: crate::table::Layout,
    ) -> String {
        let marks = Marks {
            parallel,
            kind,
            ..Marks::of(rows)
        };
        let built: Vec<_> = rows
            .iter()
            .map(|row| task_row(row, date, painter, marks, 0))
            .collect();
        crate::table::render(&task_columns(marks), &built, &layout, None, painter)
    }

    fn render_tree(
        nodes: &[TreeNode],
        painter: &Painter,
        parallel: bool,
        kind: bool,
        layout: crate::table::Layout,
    ) -> String {
        let marks = Marks {
            parallel,
            kind,
            ..Marks::of_tree(nodes)
        };
        let mut built = Vec::new();
        tree_rows(nodes, 0, painter, marks, &mut built);
        crate::table::render(&task_columns(marks), &built, &layout, None, painter)
    }

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, Format::Pretty, false)
    }

    #[test]
    fn format_resolution_validates_the_env_var_whenever_it_is_set() {
        for (json, pretty, env, expected) in [
            (false, false, None, Format::Json),
            (false, false, Some("json"), Format::Json),
            (false, false, Some("pretty"), Format::Pretty),
            (false, true, None, Format::Pretty),
            (false, true, Some("json"), Format::Pretty),
            (false, true, Some("pretty"), Format::Pretty),
            (true, false, None, Format::Json),
            (true, false, Some("json"), Format::Json),
            (true, false, Some("pretty"), Format::Json),
        ] {
            assert_eq!(
                Format::resolve(json, pretty, env).unwrap(),
                expected,
                "--json {json}, --pretty {pretty}, TASKS_FORMAT {env:?}"
            );
        }
        for (json, pretty) in [(false, false), (false, true), (true, false)] {
            let error = Format::resolve(json, pretty, Some("xml"))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("TASKS_FORMAT must be json or pretty"),
                "--json {json}, --pretty {pretty}: {error}"
            );
        }
    }

    #[test]
    fn the_marker_column_is_absent_when_nothing_is_marked() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_parallel(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(!text.contains("||"), "{text}");
        assert!(text.contains("todo  2026-09-06"), "{text}");
    }

    #[test]
    fn the_type_column_is_absent_when_nothing_recurs() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(!text.contains(" p "), "{text}");
        assert!(text.contains("todo  2026-09-06"), "{text}");
    }

    #[test]
    fn a_recurring_row_carries_the_type_letter_and_keeps_its_neighbour_aligned() {
        let rows = [recurring("xx-000001"), row("xx-000002", false)];
        assert!(any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            true,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo  p  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo     2026-09-06"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn the_type_letter_is_painted_but_still_ascii() {
        let rows = [recurring("xx-000001")];
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &colored,
            false,
            true,
            crate::table::Layout::default(),
        );
        assert!(text.contains("\x1b[1mp\x1b[0m "), "{text:?}");
    }

    #[test]
    fn any_type_tree_finds_a_recurring_descendant() {
        let nodes = vec![TreeNode {
            summary: row("xx-000001", false),
            children: vec![TreeNode {
                summary: recurring("xx-000002"),
                children: vec![],
            }],
        }];
        assert!(any_type_tree(&nodes));
    }

    #[test]
    fn a_lane_row_carries_the_lane_mark_in_the_type_column() {
        let mut lane = row("xx-000001", false);
        lane.lane = true;
        let rows = [lane, row("xx-000002", false)];
        assert!(any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            true,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("todo  ≡  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo     2026-09-06"), "{}", lines[1]);
    }

    #[test]
    fn mixed_siblings_share_one_column_layout() {
        // The case a per-call decision inside `table` gets wrong: an unmarked sibling
        // must reserve the same width as its marked neighbour, or the date and title
        // shift between adjacent lines.
        let nodes = vec![
            TreeNode {
                summary: row("xx-000001", true),
                children: vec![],
            },
            TreeNode {
                summary: row("xx-000002", false),
                children: vec![],
            },
        ];
        assert!(any_parallel_tree(&nodes));
        let text = render_tree(
            &nodes,
            &plain(),
            true,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo  ||  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo      2026-09-06"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn ids_of_different_lengths_share_one_column() {
        // `--all-projects` mixes prefixes, so ids differ in length; the id column must
        // be as wide as the longest one, or every later column shifts per row.
        let out = Output::List(ListOut {
            tasks: vec![row("forge-0e720e", false), row("nrp-8e8fde", false)],
            halts: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        let text = pretty(&out, &plain(), &crate::table::Layout::default());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[1].starts_with("nrp-8e8fde    P2"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn any_parallel_tree_finds_a_marked_descendant() {
        let nodes = vec![TreeNode {
            summary: row("xx-000001", false),
            children: vec![TreeNode {
                summary: row("xx-000002", true),
                children: vec![],
            }],
        }];
        assert!(any_parallel_tree(&nodes));
    }

    fn dated() -> Painter {
        let today = ::time::Date::from_calendar_date(2026, ::time::Month::September, 6).unwrap();
        let palette = crate::palette::Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false)
            .with_recency(crate::style::Recency::new(today, &palette))
    }

    fn scaled() -> Painter {
        let palette =
            crate::palette::Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7")
                .unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false).with_priority_scale(
            crate::style::PriorityScale::new(palette.magenta.unwrap(), &palette),
        )
    }

    #[test]
    fn the_priority_column_carries_the_priority_role() {
        let mut urgent = row("xx-000001", false);
        urgent.priority = 0;
        let rows = [urgent, row("xx-000002", false)];
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &scaled(),
            false,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[1;38;2;215;95;215mP0\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(
            lines[1].contains("\x1b[38;2;163;88;159mP2\x1b[0m"),
            "{:?}",
            lines[1]
        );
    }

    #[test]
    fn the_date_column_carries_the_recency_role() {
        // row() is updated 2026-09-06, the painter's today: full cyan.
        let rows = [row("xx-000001", false)];
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &dated(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(
            text.contains("\x1b[38;2;0;215;255m2026-09-06\x1b[0m"),
            "{text:?}"
        );
    }

    #[test]
    fn a_due_column_paints_now_as_today_and_leaves_a_dash_plain() {
        let due_now = recurring("xx-000001");
        let undated = row("xx-000002", false);
        let text = render_tasks(
            &[due_now, undated],
            DateColumn::Due,
            &dated(),
            false,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[38;2;0;215;255mnow\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(lines[1].contains("todo  -    "), "{:?}", lines[1]);
        assert!(!lines[1].contains("38;2"), "{:?}", lines[1]);
    }

    #[test]
    fn theme_gates_cover_dates_and_priorities() {
        let list = Output::List(ListOut {
            tasks: vec![],
            halts: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        assert!(needs_theme(&list));
        assert!(shows_priority(&list));
        let show = Output::Next(Box::new(NextOut {
            next: None,
            halts: vec![],
            warnings: vec![],
        }));
        assert!(needs_theme(&show));
        assert!(shows_priority(&show));
        let parked = Output::Parked(ParkedOut {
            tasks: vec![],
            warnings: vec![],
        });
        assert!(needs_theme(&parked));
        assert!(!shows_priority(&parked));
        let id = Output::Id(IdOut {
            id: "xx-000001".into(),
            warnings: vec![],
        });
        assert!(!needs_theme(&id));
        assert!(!shows_priority(&id));
    }

    fn long_row() -> TaskSummary {
        let mut wrapped = row("xx-000001", false);
        wrapped.title = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu".into();
        wrapped
    }

    #[test]
    fn a_list_row_lays_out_its_columns_and_wraps_its_title_in_place() {
        let layout = crate::table::Layout {
            width: Some(80),
            headers: false,
        };
        let built: Vec<crate::table::Row> = [long_row()]
            .iter()
            .map(|row| task_row(row, DateColumn::Updated, &plain(), Marks::default(), 0))
            .collect();
        let text = crate::table::render(
            &task_columns(Marks::default()),
            &built,
            &layout,
            None,
            &plain(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        let title = lines[0].find("alpha").unwrap();
        for line in &lines[1..] {
            assert_eq!(line.len() - line.trim_start().len(), title, "{text}");
        }
        assert!(
            lines
                .iter()
                .all(|line| crate::table::visible_width(line) <= 80),
            "{text}"
        );
        let mut words = vec![&lines[0][title..]];
        words.extend(lines[1..].iter().map(|line| line.trim_start()));
        assert_eq!(words.join(" "), long_row().title, "{text}");
    }

    #[test]
    fn a_tree_indents_children_inside_the_id_column() {
        let child = TreeNode {
            summary: row("xx-000002", false),
            children: vec![],
        };
        let root = TreeNode {
            summary: row("xx-000001", false),
            children: vec![child],
        };
        let mut built = Vec::new();
        tree_rows(&[root], 0, &plain(), Marks::default(), &mut built);
        let text = crate::table::render(
            &task_columns(Marks::default()),
            &built,
            &crate::table::Layout::default(),
            None,
            &plain(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("xx-000001"), "{text}");
        assert!(lines[1].starts_with("  xx-000002"), "{text}");
    }

    #[test]
    fn wide_characters_fit_the_terminal_columns() {
        let mut row = long_row();
        row.title = "界".repeat(40);
        let text = render_tasks(
            &[row],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        for line in text.lines() {
            let columns: usize = line.chars().map(|c| if c == '界' { 2 } else { 1 }).sum();
            assert!(columns <= 80, "{columns} columns: {line:?}");
        }
    }

    #[test]
    fn a_spaces_only_title_keeps_the_row_without_trailing_whitespace() {
        let mut row = long_row();
        row.title = " ".repeat(40);
        let text = render_tasks(
            &[row],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
        assert!(text.starts_with("xx-000001"), "{text:?}");
        assert_eq!(text.trim_end(), text.lines().next().unwrap(), "{text:?}");
    }

    #[test]
    fn a_row_that_fits_the_width_stays_one_line_and_byte_identical() {
        let rows = [row("xx-000001", false)];
        let wrapped = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        let unwrapped = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert_eq!(wrapped.lines().count(), 1, "{wrapped:?}");
        assert_eq!(wrapped, unwrapped);
    }

    #[test]
    fn a_row_below_the_title_floor_stays_unwrapped() {
        // Fixed cells and gaps take 42 columns, so at width 60 only 18 remain
        // for the title: below the floor, the row overflows unwrapped.
        let text = render_tasks(
            &[long_row()],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(60),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
    }

    #[test]
    fn a_due_cadence_is_painted_once_when_wrapped() {
        let mut due = recurring("xx-000001");
        due.title = long_row().title;
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render_tasks(
            &[due],
            DateColumn::Due,
            &colored,
            false,
            true,
            layout_at(80),
        );
        assert!(text.contains("\x1b[1m"), "{text:?}");
        assert!(!text.contains("\x1b[1m\x1b[1m"), "{text:?}");
    }

    #[test]
    fn a_quiet_park_marker_wraps_with_the_row_and_is_painted_once() {
        let mut quiet = long_row();
        quiet.park = Some(ParkInfo {
            at: "2026-09-06T00:00:00Z".into(),
            next_step: "rerun the preflight".into(),
            waiting_on: crate::claims::WaitingOn::User,
            reason: Some(crate::claims::Reason::Quiet),
            needs: Some(crate::claims::Needs::Idle),
            minutes: Some(40),
            session: "s".into(),
            owner: "o".into(),
            host: "h".into(),
            worktree: "w".into(),
        });
        let marker = "waits on user, quiet; idle, 40 min";
        let render = |painter: &Painter, layout| {
            render_tasks(
                std::slice::from_ref(&quiet),
                DateColumn::Updated,
                painter,
                false,
                false,
                layout,
            )
        };
        assert!(render(&plain(), crate::table::Layout::default()).contains(marker));
        let wrapped = render(&plain(), layout_at(80));
        assert!(wrapped.lines().count() > 1, "{wrapped:?}");
        let joined: Vec<&str> = wrapped.lines().map(str::trim).collect();
        assert!(joined.join(" ").contains(marker), "{wrapped:?}");
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render(&colored, layout_at(80));
        assert!(text.contains("\x1b[1m"), "{text:?}");
        assert!(!text.contains("\x1b[1m\x1b[1m"), "{text:?}");
    }

    #[test]
    fn a_tree_node_wraps_under_its_own_indentation() {
        let mut child = long_row();
        child.id = "xx-000002".into();
        let nodes = vec![TreeNode {
            summary: long_row(),
            children: vec![TreeNode {
                summary: child,
                children: vec![],
            }],
        }];
        let text = render_tree(&nodes, &plain(), false, false, layout_at(80));
        let lines: Vec<&str> = text.lines().collect();
        let child_line = lines
            .iter()
            .position(|line| line.starts_with("  xx-000002"))
            .expect("child row: {text:?}");
        let title_start = lines[child_line].find("alpha").expect("child title");
        let continuation = lines[child_line + 1];
        assert_eq!(
            continuation.len() - continuation.trim_start().len(),
            title_start,
            "{text:?}"
        );
    }

    fn parked_with(next_step: &str, reason: Option<crate::claims::Reason>) -> ParkedRow {
        let mut parked = ParkedRow::resolved(long_row(), crate::model::Phase::Implementing);
        let quiet = reason == Some(crate::claims::Reason::Quiet);
        parked.park = Some(ParkInfo {
            at: "2026-09-06T00:00:00Z".into(),
            next_step: next_step.into(),
            waiting_on: crate::claims::WaitingOn::User,
            reason,
            needs: quiet.then_some(crate::claims::Needs::Headless),
            minutes: quiet.then_some(39),
            session: "s".into(),
            owner: "o".into(),
            host: "h".into(),
            worktree: "/a/long/checkout/path/that/runs/well/past/the/width/.worktrees/x".into(),
        });
        parked
    }

    const LONG_STEP: &str = "Agent: run the prepared smoke pilot and the full matrix, write and \
        attach the evidence, then hand the contact sheet to the owner for a judgement";

    #[test]
    fn a_parked_row_wraps_its_title() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let text = parked_table(
            &[parked_with("next step", None)],
            &plain(),
            &layout,
            None,
            false,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        let title_start = lines[0].find("alpha").unwrap();
        assert_eq!(
            lines[1].len() - lines[1].trim_start().len(),
            title_start,
            "{text}"
        );
    }

    #[test]
    fn a_parked_tables_wait_column_fits_its_longest_stop() {
        let rows = [
            parked_with("a", Some(crate::claims::Reason::Quiet)),
            parked_with("b", Some(crate::claims::Reason::Review)),
        ];
        let text = parked_table(
            &rows,
            &plain(),
            &crate::table::Layout::default(),
            None,
            false,
        );
        let dates: Vec<usize> = text
            .lines()
            .map(|line| line.find("2026-09-06").unwrap())
            .collect();
        assert_eq!(dates.len(), 2, "{text}");
        assert_eq!(dates[0], dates[1], "{text}");
        assert!(text.contains("39 min  2026-09-06"), "{text}");
    }

    #[test]
    fn a_parked_rows_next_step_wraps_beside_its_title() {
        let layout = crate::table::Layout {
            width: Some(160),
            headers: false,
        };
        let text = parked_table(
            &[parked_with(LONG_STEP, None)],
            &plain(),
            &layout,
            None,
            false,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("alpha") && lines[0].contains("Agent:"),
            "{text}"
        );
        assert!(
            lines.len() > 1 && !lines.iter().any(|l| l.contains("next:")),
            "{text}"
        );
        assert!(
            lines.iter().all(|l| crate::table::visible_width(l) <= 160),
            "{text}"
        );
    }

    #[test]
    fn a_narrow_parked_table_moves_next_under_the_row() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let text = parked_table(
            &[parked_with(LONG_STEP, None)],
            &plain(),
            &layout,
            None,
            false,
        );
        assert!(text.contains("        next:   Agent:"), "{text}");
    }

    #[test]
    fn an_unresolved_park_renders_placeholders() {
        let mut row = parked_with("x", None);
        row.status = None;
        row.phase = None;
        let text = parked_table(
            &[row],
            &plain(),
            &crate::table::Layout::default(),
            None,
            false,
        );
        assert!(text.starts_with("xx-000001  ?  -  user"), "{text}");
    }

    #[test]
    fn quiet_keeps_its_checkout_whole_under_the_row() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let row = parked_with(LONG_STEP, Some(crate::claims::Reason::Quiet));
        let worktree = row.park.as_ref().unwrap().worktree.clone();
        let text = quiet_table(&[row], &plain(), &layout);
        assert!(
            text.contains(&format!("        in:     {worktree}\n")),
            "{text}"
        );
    }

    #[test]
    fn columns_overrides_the_terminal_and_validates_input() {
        assert_eq!(wrap_width(Some("80"), Some(120)).unwrap(), Some(80));
        assert_eq!(wrap_width(None, Some(120)).unwrap(), Some(120));
        assert_eq!(wrap_width(None, None).unwrap(), None);
        assert!(wrap_width(Some("wide"), None).is_err());
        assert!(wrap_width(Some("0"), None).is_err());
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
