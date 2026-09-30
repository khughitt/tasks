use std::io::IsTerminal;
use unicode_width::UnicodeWidthChar;

use crate::error::{Error, Result};
use crate::model::{Complexity, Process, Size, Status, Task};
use crate::registry::Registry;
use crate::style::{Painter, Style, When};
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
    pub warnings: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
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

/// `add`'s own shape: the id plus what happened to it. `add` is the one write that can
/// decline to write -- a sourced add whose origin and title already exist reuses that
/// record -- so the caller needs to tell a fresh id from an old one without parsing
/// warnings. Same `id, action, warnings` shape `feedback` already returns.
#[derive(Serialize)]
pub struct AddOut {
    pub id: String,
    /// `created`, or `reused` when `--source` matched an existing task.
    pub action: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct RootOut {
    pub prefix: String,
    pub root: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct ProjectRow {
    pub prefix: String,
    pub root: String,
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

/// Everything `show` says about one task, without the warnings, so `next` can embed it.
#[derive(Serialize)]
pub struct ShowFields {
    pub task: Task,
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

#[derive(Serialize, Clone)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<Complexity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    pub parallel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub created: String,
    pub updated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
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
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub worktree: String,
    pub started: String,
    pub seen: String,
    pub live: bool,
}

impl ClaimInfo {
    pub fn of(claim: &crate::claims::Claim, live: &crate::claims::Liveness) -> ClaimInfo {
        ClaimInfo {
            owner: claim.owner.clone(),
            session: claim.session.clone(),
            host: claim.host.clone(),
            pid: claim.pid,
            worktree: claim.worktree.clone(),
            started: claim.started.clone(),
            seen: claim.seen.clone(),
            live: live == &crate::claims::Liveness::Live,
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
    /// `all` is the scan the row came from; counts are computed against it.
    pub fn of(
        task: &Task,
        all: &[Task],
        claims: Option<&crate::claims::ClaimSnapshot>,
        registry: &Registry,
        now: OffsetDateTime,
    ) -> TaskSummary {
        TaskSummary {
            id: task.id.to_string(),
            title: task.title.clone(),
            status: task.status,
            priority: task.priority,
            size: task.size,
            complexity: task.complexity,
            process: task.process,
            parallel: task.parallel,
            owner: task.owner.clone(),
            created: task.created.clone(),
            updated: task.updated.clone(),
            started: task.started.clone(),
            completed: task.completed.clone(),
            tags: task.tags.clone(),
            source: task.source.clone(),
            model: task.model.clone(),
            agent: task.agent.clone(),
            depends: task.depends.iter().map(ToString::to_string).collect(),
            parent: task.parent.as_ref().map(ToString::to_string),
            child_count: crate::hierarchy::children(all, &task.id, registry).len(),
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
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
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
            status: Some(summary.status),
            priority: Some(summary.priority),
            size: summary.size,
            complexity: summary.complexity,
            process: summary.process,
            parallel: summary.parallel,
            owner: summary.owner,
            created: Some(summary.created),
            updated: Some(summary.updated),
            started: summary.started,
            completed: summary.completed,
            tags: summary.tags,
            source: summary.source,
            model: summary.model,
            agent: summary.agent,
            depends: summary.depends,
            parent: summary.parent,
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
            status: None,
            priority: None,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            owner: None,
            created: None,
            updated: None,
            started: None,
            completed: None,
            tags: Vec::new(),
            source: None,
            model: None,
            agent: None,
            depends: Vec::new(),
            parent: None,
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

#[derive(Serialize, Default)]
pub struct Counts {
    pub idea: usize,
    pub todo: usize,
    pub doing: usize,
    pub blocked: usize,
    pub shelved: usize,
    pub done: usize,
    pub dropped: usize,
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
            }
        }
        counts
    }

    pub fn total(&self) -> usize {
        self.idea + self.todo + self.doing + self.blocked + self.shelved + self.done + self.dropped
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
    }
    columns.push(CountColumn {
        label: "total",
        value: counts.total(),
        style: None,
    });
    columns
}

/// A zero is dimmed whatever its status: a red `0` under `blocked` reads as an alarm.
fn count_column(label: &'static str, value: usize, status: Status) -> CountColumn {
    CountColumn {
        label,
        value,
        style: Some(if value == 0 {
            Style::Chrome
        } else {
            Style::Status(status)
        }),
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

struct Cell {
    text: String,
    align: Align,
    style: Option<Style>,
}

fn cell(text: impl Into<String>, align: Align, style: Option<Style>) -> Cell {
    Cell {
        text: text.into(),
        align,
        style,
    }
}

/// Pad first, paint last: ANSI bytes count toward `{:<n}` widths, so every cell reaches its
/// column's visible width before the painter wraps it. The last cell of a row is never
/// padded, so no line carries trailing whitespace.
fn grid_text(grid: &[Vec<Cell>], painter: &Painter) -> String {
    let columns = grid.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|index| {
            grid.iter()
                .filter_map(|row| row.get(index))
                .map(|cell| cell.text.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut rendered = String::new();
    for row in grid {
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                rendered.push_str("  ");
            }
            let width = widths[index];
            let padded = if index + 1 == row.len() {
                cell.text.clone()
            } else {
                match cell.align {
                    Align::Left => format!("{:<width$}", cell.text),
                    Align::Right => format!("{:>width$}", cell.text),
                }
            };
            rendered.push_str(&match cell.style {
                Some(style) => painter.paint(style, &padded),
                None => padded,
            });
        }
        rendered.push('\n');
    }
    rendered
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
pub struct PrimeOut {
    /// The local project; null under --all-projects.
    pub prefix: Option<String>,
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
    pub roadmap: Vec<TreeNode>,
    pub closeout: Vec<TaskSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub halts: Vec<HaltRow>,
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

#[derive(Serialize)]
pub struct FeedbackOut {
    pub id: String,
    pub action: String,
    pub path: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct CheckOut {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

/// One variant per command payload. Later tasks add variants; `pretty` grows with them.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Output {
    Init(InitOut),
    Rename(RenameOut),
    Id(IdOut),
    Add(AddOut),
    Root(RootOut),
    Projects(ProjectsOut),
    Show(Box<ShowOut>),
    Next(Box<NextOut>),
    List(ListOut),
    Parked(ParkedOut),
    Quiet(QuietOut),
    Claims(ClaimsOut),
    Prime(PrimeOut),
    Graph(GraphOut),
    Check(CheckOut),
    Tree(TreeOut),
    Tags(TagsOut),
    Feedback(FeedbackOut),
    Attach(AttachOut),
    Detach(DetachOut),
}

pub fn render(out: &Output, format: Format, painter: &Painter, wrap: Wrap) -> String {
    match format {
        Format::Json => serde_json::to_string(out).expect("output serializes"),
        Format::Pretty => pretty(out, painter, wrap),
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

fn pretty(out: &Output, painter: &Painter, wrap: Wrap) -> String {
    match out {
        Output::Init(o) => o.prefix.clone(),
        Output::Rename(o) => o.prefix.clone(),
        Output::Id(o) => o.id.clone(),
        Output::Add(o) => o.id.clone(),
        Output::Root(o) => o.root.clone(),
        Output::Projects(o) if o.projects.is_empty() => String::new(),
        Output::Projects(o) => {
            // Header labels come from the same call the rows use, so a column can never
            // appear in one and not the other.
            let mut header = vec![cell("project", Align::Left, Some(Style::Chrome))];
            for column in count_columns(&Counts::default(), o.closed) {
                header.push(cell(column.label, Align::Right, Some(Style::Chrome)));
            }
            header.push(cell("activity", Align::Left, Some(Style::Chrome)));
            if o.paths {
                header.push(cell("root", Align::Left, Some(Style::Chrome)));
            }
            let mut grid = vec![header];
            for row in &o.projects {
                let mut cells = vec![cell(&row.prefix, Align::Left, Some(Style::Chrome))];
                match &row.counts {
                    Some(counts) => {
                        cells.extend(count_columns(counts, o.closed).into_iter().map(|column| {
                            cell(column.value.to_string(), Align::Right, column.style)
                        }))
                    }
                    None => cells.extend(
                        count_columns(&Counts::default(), o.closed)
                            .iter()
                            .map(|_| cell("-", Align::Right, Some(Style::Chrome))),
                    ),
                }
                // An unreachable project has no activity to report, so that column says
                // why instead of printing a dash the eye would skip.
                cells.push(match (&row.counts, &row.last_activity) {
                    (None, _) => cell("unreachable", Align::Left, Some(Style::Error)),
                    (Some(_), Some(at)) => {
                        cell(crate::time::day(at), Align::Left, Some(date_role(at)))
                    }
                    (Some(_), None) => cell("-", Align::Left, Some(Style::Chrome)),
                });
                if o.paths {
                    cells.push(cell(&row.root, Align::Left, None));
                }
                grid.push(cells);
            }
            grid_text(&grid, painter)
        }
        Output::Show(o) => show_text(&o.fields, painter),
        Output::Next(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            match &o.next {
                Some(fields) => show_text(fields, painter),
                None => "nothing ready".into(),
            }
        ),
        Output::List(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            table(
                &o.tasks,
                o.date,
                painter,
                id_width(o.tasks.iter().map(|row| row.id.as_str())),
                any_parallel(&o.tasks),
                any_type(&o.tasks),
                wrap,
            )
        ),
        Output::Parked(o) => parked_table(
            &o.tasks,
            painter,
            id_width(o.tasks.iter().map(|row| row.id.as_str())),
            wrap,
        ),
        Output::Quiet(o) => quiet_briefs(
            &o.tasks,
            painter,
            id_width(o.tasks.iter().map(|row| row.id.as_str())),
        ),
        Output::Claims(o) => {
            let width = id_width(o.claims.iter().map(|row| row.id.as_str()));
            o.claims
                .iter()
                .map(|row| {
                    let live = if row.claim.live { "live" } else { "stale" };
                    format!(
                        "{:<width$}  {}  {}  {}",
                        row.id, row.claim.session, live, row.claim.worktree
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Output::Prime(o) => {
            // One decision for the whole output: prime's blocks align today only because
            // every width is fixed, and a per-section decision would break that.
            let parallel_column = any_parallel(&o.closeout)
                || any_parallel_tree(&o.roadmap)
                || any_parallel(&o.ready)
                || any_parallel(&o.doing);
            let type_column = any_type(&o.closeout)
                || any_type_tree(&o.roadmap)
                || any_type(&o.ready)
                || any_type(&o.doing);
            let id_width = id_width(
                o.closeout
                    .iter()
                    .chain(&o.ready)
                    .chain(&o.doing)
                    .map(|row| row.id.as_str())
                    .chain(o.parked.iter().map(|row| row.id.as_str())),
            )
            .max(id_width_tree(&o.roadmap));
            let header = match &o.prefix {
                Some(prefix) => format!("project {prefix}"),
                None => format!("projects {}", o.projects.join(", ")),
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
            rendered.push_str(&table(
                &o.closeout,
                DateColumn::Updated,
                painter,
                id_width,
                parallel_column,
                type_column,
                wrap,
            ));
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "roadmap:")
            ));
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    rendered.push_str(&tree_text(
                        std::slice::from_ref(node),
                        0,
                        painter,
                        id_width,
                        parallel_column,
                        type_column,
                        wrap,
                    ));
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    rendered.push_str(&table(
                        std::slice::from_ref(&node.summary),
                        DateColumn::Updated,
                        painter,
                        id_width,
                        parallel_column,
                        type_column,
                        wrap,
                    ));
                }
            }
            rendered.push_str(&format!(
                "{listed_under_ready} childless root(s) are listed under ready\n"
            ));
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "parked:")
            ));
            rendered.push_str(&parked_table(&o.parked, painter, id_width, wrap));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "ready:")));
            rendered.push_str(&table(
                &o.ready,
                DateColumn::Updated,
                painter,
                id_width,
                parallel_column,
                type_column,
                wrap,
            ));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "doing:")));
            rendered.push_str(&table(
                &o.doing,
                DateColumn::Updated,
                painter,
                id_width,
                parallel_column,
                type_column,
                wrap,
            ));
            rendered
        }
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
        Output::Tree(o) => tree_text(
            &o.nodes,
            0,
            painter,
            id_width_tree(&o.nodes),
            any_parallel_tree(&o.nodes),
            any_type_tree(&o.nodes),
            wrap,
        ),
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
        Output::Feedback(o) => format!("{} {}", o.action, o.id),
        Output::Attach(o) => o.file.path.clone(),
        Output::Detach(o) if o.removed => format!(
            "removed {}; git history keeps committed bytes, uncommitted ones are gone",
            o.path
        ),
        Output::Detach(o) => format!("recorded {} as detached; no file was present", o.name),
    }
}

/// The `serialize_task` text for `task`, with its frontmatter values painted in the roles
/// the `list` table gives the same fields. Keys, the `---` delimiters, the body, and the
/// notes are returned verbatim: that is prose and file text, and it stays copy-pasteable.
///
/// Painting the writer's output rather than teaching `serialize_task` about a painter keeps
/// escape sequences unreachable from the code that writes task files to disk, and leaves one
/// serializer to maintain: a field added later is simply unpainted until it is named below.
fn paint_frontmatter(text: &str, task: &Task, painter: &Painter) -> String {
    // Same split as `format::parse_task`: no frontmatter line is ever exactly `---`, so the
    // first `\n---\n` is the closing delimiter.
    let Some((fields, rest)) = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return text.into();
    };
    let mut rendered = String::from("---\n");
    for line in fields.lines() {
        rendered.push_str(&paint_field(line, task, painter));
        rendered.push('\n');
    }
    rendered.push_str("---\n");
    rendered.push_str(rest);
    rendered
}

/// One `key: value` frontmatter line. The value is painted, never the key, and a key with
/// no role in the table is left alone.
fn paint_field(line: &str, task: &Task, painter: &Painter) -> String {
    let Some((key, value)) = line.split_once(": ") else {
        return line.into();
    };
    let style = match key {
        // an id is chrome wherever it appears, including `show`'s own footers below
        "id" | "parent" | "depends" => Style::Chrome,
        "owner" | "tags" => Style::Chrome,
        "status" => Style::Status(task.status),
        "priority" => Style::Priority(task.priority),
        _ => return line.into(),
    };
    format!("{key}: {}", painter.paint(style, value))
}

fn show_text(o: &ShowFields, painter: &Painter) -> String {
    let mut rendered = paint_frontmatter(&crate::format::serialize_task(&o.task), &o.task, painter);
    rendered.push_str(&format!(
        "\nProcess: {}\n",
        o.task.process.map(Process::as_str).unwrap_or("unassessed")
    ));
    if let Some(periodic) = &o.periodic {
        let due = match (&periodic.due, periodic.due_now) {
            (Some(due), _) => Some(crate::time::day(due)),
            (None, true) => Some("now"),
            (None, false) => None,
        };
        if let Some(due) = due {
            rendered.push_str(&format!("\n# periodic\ndue: {due}\n"));
        }
    }
    if let Some(deferred) = &o.deferred {
        let state = if deferred.due { " (due)" } else { "" };
        rendered.push_str(&format!("\n# deferred\nuntil: {}{state}\n", deferred.until));
    }
    let related_row = |id: &str, status: Option<Status>, title: &str| {
        let status = match status {
            Some(status) => painter.paint(Style::Status(status), status.as_str()),
            None => "?".into(),
        };
        format!(
            "- {} [{status}] {title}\n",
            painter.paint(Style::Chrome, id)
        )
    };
    if !o.depends_on.is_empty() {
        rendered.push_str("\n# depends on\n");
        for dependency in &o.depends_on {
            let title = dependency.title.as_deref().unwrap_or("(unresolved)");
            rendered.push_str(&related_row(&dependency.id, dependency.status, title));
        }
    }
    if let Some(found) = o.step_found {
        rendered.push_str(&if found {
            "\n# step found\n".to_string()
        } else {
            format!("\n{}\n", painter.paint(Style::Error, "# step MISSING"))
        });
    }
    if let Some(parent) = &o.parent {
        rendered.push_str("\n# parent\n");
        rendered.push_str(&related_row(&parent.id, Some(parent.status), &parent.title));
    }
    if !o.children.is_empty() {
        rendered.push_str("\n# children\n");
        for child in &o.children {
            rendered.push_str(&related_row(&child.id, Some(child.status), &child.title));
        }
    }
    if !o.files.is_empty() {
        rendered.push_str("\n# files\n");
        for file in &o.files {
            rendered.push_str(&format!(
                "- {} ({})\n",
                file.path,
                crate::attachments::human_size(file.bytes)
            ));
        }
    }
    if let Some(park) = &o.park {
        rendered.push_str("\n# parked\n");
        rendered.push_str(&format!(
            "- waiting on {} since {}: {}\n",
            crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes),
            crate::time::day(&park.at),
            park.next_step
        ));
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("  session {} in {}", park.session, park.worktree),
        ));
        rendered.push('\n');
    }
    if let Some(escalation) = &o.escalation {
        rendered.push_str("\n# escalation\n");
        rendered.push_str(&format!(
            "- needs at least {} since {}\n",
            escalation.level.as_str(),
            crate::time::day(&escalation.at)
        ));
        rendered
            .push_str(&painter.paint(Style::Chrome, &format!("  session {}", escalation.session)));
        rendered.push('\n');
    }
    rendered
}

fn tree_text(
    nodes: &[TreeNode],
    depth: usize,
    painter: &Painter,
    id_width: usize,
    parallel_column: bool,
    type_column: bool,
    wrap: Wrap,
) -> String {
    let mut rendered = String::new();
    for node in nodes {
        let row = table(
            std::slice::from_ref(&node.summary),
            DateColumn::Updated,
            painter,
            id_width,
            parallel_column,
            type_column,
            wrap.indented(depth * 2),
        );
        rendered.push_str(&"  ".repeat(depth));
        rendered.push_str(&row);
        rendered.push_str(&tree_text(
            &node.children,
            depth + 1,
            painter,
            id_width,
            parallel_column,
            type_column,
            wrap,
        ));
    }
    rendered
}

/// Whether a pretty rendering must reserve the parallel column. Decided once per command
/// output and passed into `table`: `tree_text` and `prime`'s roadmap call `table` one row
/// at a time, so a per-call decision would shift dates between adjacent siblings.
pub fn any_parallel(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| row.parallel)
}

pub fn any_parallel_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.parallel || any_parallel_tree(&node.children))
}

/// The one-letter type marker for a summary row: `p` for a record carrying a cadence, and
/// nothing otherwise. A future type is another arm here, another letter in the same slot;
/// the column is reserved once per output (see `any_type`), so it is never a layout change.
fn type_letter(row: &TaskSummary) -> Option<char> {
    row.periodic.as_ref().map(|_| 'p')
}

/// Whether a pretty rendering must reserve the type column. Same once-per-output rule as
/// `any_parallel`: `tree_text` and `prime`'s roadmap call `table` one row at a time, so a
/// per-call decision would shift dates between adjacent siblings.
pub fn any_type(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| type_letter(row).is_some())
}

pub fn any_type_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| type_letter(&node.summary).is_some() || any_type_tree(&node.children))
}

/// The id column's width: the longest id in the output. Same once-per-output rule as
/// `any_parallel`. One project's ids share a length, but `--all-projects` mixes prefixes,
/// and an unpadded id shifts every later column of its row.
fn id_width<'a>(ids: impl IntoIterator<Item = &'a str>) -> usize {
    ids.into_iter().map(str::len).max().unwrap_or(0)
}

fn id_width_tree(nodes: &[TreeNode]) -> usize {
    nodes
        .iter()
        .map(|node| node.summary.id.len().max(id_width_tree(&node.children)))
        .max()
        .unwrap_or(0)
}

/// The date role for a validated timestamp or `YYYY-MM-DD` day.
fn date_role(timestamp: &str) -> Style {
    let day = crate::time::calendar_day(crate::time::day(timestamp))
        .expect("records and park entries carry validated dates");
    Style::Date(When::On(day))
}

/// Whether a pretty rendering of `out` paints from the theme's colors, and so whether
/// the stdout painter needs them (date spec §3.1; priority spec §3.1). Every output with
/// a priority column also has a date column.
pub fn needs_theme(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Tree(_)
            | Output::Parked(_)
            | Output::Quiet(_)
            | Output::Projects(_)
    )
}

/// Whether a pretty rendering of `out` shows priorities, and so whether a palette
/// without magenta costs it anything (priority spec §3.3).
pub fn shows_priority(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_) | Output::Prime(_) | Output::Tree(_) | Output::Quiet(_)
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

/// The visible length of text the painter has already wrapped in SGR sequences: padding
/// lands before painting, so ANSI bytes must not count toward a wrap width.
fn visible_width(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for terminator in chars.by_ref() {
                if terminator.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            count += c.width().unwrap_or(0);
        }
    }
    count
}

// ponytail: per-character widths can split joined emoji; use grapheme clusters if needed.
fn columns(chars: &[(char, Option<Style>)]) -> usize {
    chars.iter().map(|(c, _)| c.width().unwrap_or(0)).sum()
}

/// Where pretty wrapping applies: the target width, and the indentation a tree node
/// already occupies before its row. `Wrap::NONE` leaves every row on one line.
#[derive(Clone, Copy, Default)]
pub struct Wrap {
    width: Option<usize>,
    indent: usize,
}

impl Wrap {
    /// Every row on one line, whatever its length.
    pub const NONE: Wrap = Wrap {
        width: None,
        indent: 0,
    };

    /// Wrap at `width` with no tree indentation.
    pub fn at(width: usize) -> Wrap {
        Wrap {
            width: Some(width),
            indent: 0,
        }
    }

    /// The same width under a tree node's existing indentation.
    pub fn indented(self, indent: usize) -> Wrap {
        Wrap {
            indent: self.indent + indent,
            ..self
        }
    }
}

/// Below this many columns for the title, a wrapped row renders one word per line, which
/// reads worse than an overflowing row; the row stays unwrapped instead.
const TITLE_FLOOR: usize = 20;

/// One table row: the fixed prefix (already padded and painted), then the trailing
/// title-and-suffix segment wrapped within the width left over. Continuation lines indent
/// to the title's start, past the tree indentation in `wrap`. Styles sit on the pieces,
/// so a suffix keeps its role across a break.
fn render_row(
    prefix: &str,
    pieces: &[(&str, Option<Style>)],
    painter: &Painter,
    wrap: Wrap,
) -> String {
    let chars: Vec<(char, Option<Style>)> = pieces
        .iter()
        .flat_map(|(text, style)| text.chars().map(move |c| (c, *style)))
        .collect();
    let single = format!("{prefix}{}\n", paint_runs(&chars, painter));
    let Some(width) = wrap.width else {
        return single;
    };
    let fixed = wrap.indent + visible_width(prefix);
    let available = width.saturating_sub(fixed);
    if available < TITLE_FLOOR || columns(&chars) <= available {
        return single;
    }
    let lines = wrap_lines(&chars, available);
    if lines.is_empty() {
        return single;
    }
    let mut rendered = format!("{prefix}{}\n", paint_runs(&lines[0], painter));
    let continuation = " ".repeat(fixed);
    for line in &lines[1..] {
        rendered.push_str(&continuation);
        rendered.push_str(&paint_runs(line, painter));
        rendered.push('\n');
    }
    rendered
}

/// Paint runs of one style each. Adjacent pieces sharing a style merge into one run; the
/// visible text is unchanged, so `ColorMode::Never` output matches painting each piece
/// whole.
fn paint_runs(chars: &[(char, Option<Style>)], painter: &Painter) -> String {
    let mut rendered = String::new();
    let mut index = 0;
    while index < chars.len() {
        let style = chars[index].1;
        let end = chars[index..]
            .iter()
            .take_while(|(_, s)| *s == style)
            .count()
            + index;
        let text: String = chars[index..end].iter().map(|(c, _)| c).collect();
        rendered.push_str(&match style {
            Some(style) => painter.paint(style, &text),
            None => text,
        });
        index = end;
    }
    rendered
}

/// Greedy word wrap over already-styled characters: a break lands on a space run, which is
/// dropped there, and a word longer than `available` is hard-split at the width. A word
/// keeps its style across the split.
fn wrap_lines(
    chars: &[(char, Option<Style>)],
    available: usize,
) -> Vec<Vec<(char, Option<Style>)>> {
    let mut lines: Vec<Vec<(char, Option<Style>)>> = Vec::new();
    let mut line: Vec<(char, Option<Style>)> = Vec::new();
    let mut pending: Option<&[(char, Option<Style>)]> = None;
    let mut index = 0;
    while index < chars.len() {
        let spaces = chars[index].0 == ' ';
        let end = chars[index..]
            .iter()
            .take_while(|(c, _)| (*c == ' ') == spaces)
            .count()
            + index;
        let run = &chars[index..end];
        index = end;
        if spaces {
            if !line.is_empty() {
                pending = Some(run);
            }
            continue;
        }
        let sep = pending.take().unwrap_or(&[]);
        if columns(&line) + columns(sep) + columns(run) <= available {
            line.extend_from_slice(sep);
            line.extend_from_slice(run);
        } else if columns(run) > available {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            fill_hard_split(&mut lines, &mut line, run, available);
        } else {
            lines.push(std::mem::take(&mut line));
            line.extend_from_slice(run);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Chunk a word longer than the width into full-width lines, leaving the tail on the
/// current line.
fn fill_hard_split(
    lines: &mut Vec<Vec<(char, Option<Style>)>>,
    line: &mut Vec<(char, Option<Style>)>,
    word: &[(char, Option<Style>)],
    available: usize,
) {
    let mut word = word;
    while columns(word) > available {
        let mut used = 0;
        let split = word
            .iter()
            .take_while(|(c, _)| {
                let next = used + c.width().unwrap_or(0);
                if next > available {
                    false
                } else {
                    used = next;
                    true
                }
            })
            .count()
            .max(1);
        let (chunk, rest) = word.split_at(split);
        lines.push(chunk.to_vec());
        word = rest;
    }
    line.extend_from_slice(word);
}

/// Pad first, paint last: ANSI bytes count toward `{:<n}` widths, so every width-sensitive
/// field is formatted to its final visible width before the painter wraps it.
pub fn table(
    rows: &[TaskSummary],
    date: DateColumn,
    painter: &Painter,
    id_width: usize,
    parallel_column: bool,
    type_column: bool,
    wrap: Wrap,
) -> String {
    let mut rendered = String::new();
    for row in rows {
        let (date, role) = match date {
            DateColumn::Updated => (
                crate::time::day(&row.updated).to_string(),
                Some(date_role(&row.updated)),
            ),
            DateColumn::Created => (
                crate::time::day(&row.created).to_string(),
                Some(date_role(&row.created)),
            ),
            DateColumn::Due => match (&row.periodic, &row.deferred) {
                (Some(periodic), _) => match (&periodic.due, periodic.due_now) {
                    (Some(due), _) => (crate::time::day(due).to_string(), Some(date_role(due))),
                    (None, true) => ("now".into(), Some(Style::Date(When::Today))),
                    (None, false) => ("-".into(), None),
                },
                (None, Some(deferred)) => {
                    (deferred.until.clone(), Some(date_role(&deferred.until)))
                }
                (None, None) => ("-".into(), None),
            },
        };
        let date = match role {
            Some(role) => painter.paint(role, &date),
            None => date,
        };
        let id = painter.paint(Style::Chrome, &format!("{:<id_width$}", row.id));
        let priority = painter.paint(Style::Priority(row.priority), &format!("P{}", row.priority));
        let size = row.size.map(Size::as_str).unwrap_or("-");
        let complexity = row.complexity.map(Complexity::as_str).unwrap_or("-");
        let process = row.process.map(Process::as_str).unwrap_or("-");
        let status = painter.paint(
            Style::Status(row.status),
            &format!("{:<7}", row.status.as_str()),
        );
        let tags = if row.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", row.tags.join(", "))
        };
        // A due row's status column reads `done`, which is true; the marker is what says why
        // it is here (spec §5.1). Not-yet-due rows never reach `ready`, and carry their date
        // in the due column of `list --periodic` instead.
        let cadence = match &row.periodic {
            Some(periodic) if periodic.due_now => {
                let when = periodic
                    .due
                    .as_deref()
                    .map(|due| crate::time::day(due).to_string())
                    .unwrap_or_else(|| "now".into());
                format!("  every {}, due {when}", periodic.every)
            }
            _ => String::new(),
        };
        // A deferred row says why it is absent from `ready`; a due one says why it is back.
        let deferral = match &row.deferred {
            Some(deferred) if deferred.due => format!("  due {}", deferred.until),
            Some(deferred) => format!("  defer {}", deferred.until),
            None => String::new(),
        };
        // A quiet park is waiting for an idle host, which the status column cannot say; the
        // recipe reads as it does in `list --parked` (quiet-queue spec §4).
        let quiet = match &row.park {
            Some(park) if park.reason == Some(crate::claims::Reason::Quiet) => format!(
                "  waits on {}",
                crate::claims::describe_stop(
                    park.waiting_on,
                    park.reason,
                    park.needs,
                    park.minutes
                )
            ),
            _ => String::new(),
        };
        let owner = match &row.claim {
            Some(claim) if claim.live => format!(" @{} [{}]", claim.owner, claim.session),
            Some(claim) => format!(" @{} [{} stale]", claim.owner, claim.session),
            None => row
                .owner
                .as_ref()
                .map(|owner| format!(" @{owner}"))
                .unwrap_or_default(),
        };
        // Unpainted: it is already distinct, and painting the blank spacer would wrap
        // whitespace in ANSI for no gain.
        let mark = match (parallel_column, row.parallel) {
            (false, _) => "",
            (true, true) => "|| ",
            (true, false) => "   ",
        };
        // A blank row still reserves the column's width so siblings stay aligned. The
        // blank is unpainted, like the parallel spacer: the letter carries the meaning,
        // and color is only styling, so redirected and ASCII output stay honest.
        let kind = match (type_column, type_letter(row)) {
            (false, _) => String::new(),
            (true, Some(letter)) => {
                format!("{} ", painter.paint(Style::Emphasis, &letter.to_string()))
            }
            (true, None) => "  ".into(),
        };
        let prefix = format!(
            "{id}  {priority} {size:<2} {complexity:<4} {process:<7} {status} {mark}{kind}{date}  "
        );
        rendered.push_str(&render_row(
            &prefix,
            &[
                (row.title.as_str(), None),
                (tags.as_str(), Some(Style::Chrome)),
                (cadence.as_str(), Some(Style::Emphasis)),
                (deferral.as_str(), Some(Style::Emphasis)),
                (quiet.as_str(), Some(Style::Emphasis)),
                (owner.as_str(), Some(Style::Chrome)),
            ],
            painter,
            wrap,
        ));
    }
    rendered
}

pub fn parked_table(rows: &[ParkedRow], painter: &Painter, id_width: usize, wrap: Wrap) -> String {
    let mut rendered = String::new();
    for row in rows {
        let Some(park) = &row.park else {
            continue;
        };
        let id = painter.paint(Style::Chrome, &format!("{:<id_width$}", row.id));
        let status = match row.status {
            Some(status) => {
                painter.paint(Style::Status(status), &format!("{:<7}", status.as_str()))
            }
            None => painter.paint(Style::Chrome, &format!("{:<7}", "?")),
        };
        let phase = format!(
            "{:<13}",
            row.phase.map(crate::model::Phase::as_str).unwrap_or("-")
        );
        let process = row.process.map(Process::as_str).unwrap_or("-");
        let parked = painter.paint(date_role(&park.at), crate::time::day(&park.at));
        let prefix = format!(
            "{id}  {status} {process:<7} {phase} waits on {:<18} {parked}  ",
            crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes)
        );
        rendered.push_str(&render_row(
            &prefix,
            &[(row.title.as_str(), None)],
            painter,
            wrap,
        ));
        rendered
            .push_str(&painter.paint(Style::Chrome, &format!("        next: {}", park.next_step)));
        rendered.push('\n');
    }
    rendered
}

pub fn quiet_briefs(rows: &[ParkedRow], painter: &Painter, id_width: usize) -> String {
    let mut rendered = String::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(park) = &row.park else {
            continue;
        };
        if index > 0 {
            rendered.push('\n');
        }
        let id = painter.paint(Style::Chrome, &format!("{:<id_width$}", row.id));
        let priority = match row.priority {
            Some(priority) => painter.paint(Style::Priority(priority), &format!("P{priority}")),
            None => "P-".into(),
        };
        let needs = park.needs.map(crate::claims::Needs::as_str).unwrap_or("-");
        let minutes = match park.minutes {
            Some(minutes) => format!("{minutes} min"),
            None => "-".into(),
        };
        let parked = painter.paint(date_role(&park.at), crate::time::day(&park.at));
        rendered.push_str(&format!(
            "{id}  {priority}  {needs:<8}  {minutes:>8}  parked {parked}  {}\n",
            row.title
        ));
        rendered
            .push_str(&painter.paint(Style::Chrome, &format!("        next: {}", park.next_step)));
        rendered.push('\n');
        rendered
            .push_str(&painter.paint(Style::Chrome, &format!("        in:   {}", park.worktree)));
        rendered.push('\n');
    }
    rendered
}

pub fn render_error(e: &Error) -> String {
    serde_json::json!({ "error": { "kind": e.kind(), "detail": e.to_string() } }).to_string()
}

/// stderr text for warnings in pretty mode.
pub fn pretty_warnings(warnings: &[String], painter: &Painter) -> String {
    let prefix = painter.paint(Style::Warning, "warning:");
    warnings.iter().map(|w| format!("{prefix} {w}\n")).collect()
}

pub fn warnings_of(out: &Output) -> Vec<String> {
    match out {
        Output::Init(o) => o.warnings.clone(),
        Output::Rename(o) => o.warnings.clone(),
        Output::Id(o) => o.warnings.clone(),
        Output::Add(o) => o.warnings.clone(),
        Output::Root(o) => o.warnings.clone(),
        Output::Projects(o) => o.warnings.clone(),
        Output::Show(o) => o.warnings.clone(),
        Output::Next(o) => o.warnings.clone(),
        Output::List(o) => o.warnings.clone(),
        Output::Parked(o) => o.warnings.clone(),
        Output::Quiet(o) => o.warnings.clone(),
        Output::Claims(_) => Vec::new(),
        Output::Prime(o) => o.warnings.clone(),
        Output::Graph(o) => o.warnings.clone(),
        Output::Check(o) => o
            .warnings
            .iter()
            .map(|finding| format!("{} [{}] {}", finding.file, finding.kind, finding.detail))
            .collect(),
        Output::Tree(o) => o.warnings.clone(),
        Output::Tags(o) => o.warnings.clone(),
        Output::Feedback(o) => o.warnings.clone(),
        Output::Attach(o) => o.warnings.clone(),
        Output::Detach(o) => o.warnings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorMode;

    fn row(id: &str, parallel: bool) -> TaskSummary {
        TaskSummary {
            id: id.into(),
            title: format!("title {id}"),
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
            model: None,
            agent: None,
            depends: vec![],
            parent: None,
            child_count: 0,
            open_descendant_count: 0,
            claim: None,
            park: None,
            escalation: None,
            periodic: None,
            deferred: None,
            parallel,
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
        let text = table(
            &rows,
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::NONE,
        );
        assert!(!text.contains("||"), "{text}");
        assert!(text.contains("todo    2026-09-06"), "{text}");
    }

    #[test]
    fn the_type_column_is_absent_when_nothing_recurs() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_type(&rows));
        let text = table(
            &rows,
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::NONE,
        );
        assert!(!text.contains(" p "), "{text}");
        assert!(text.contains("todo    2026-09-06"), "{text}");
    }

    #[test]
    fn a_recurring_row_carries_the_type_letter_and_keeps_its_neighbour_aligned() {
        let rows = [recurring("xx-000001"), row("xx-000002", false)];
        assert!(any_type(&rows));
        let text = table(
            &rows,
            DateColumn::Updated,
            &plain(),
            0,
            false,
            true,
            Wrap::NONE,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo    p 2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo      2026-09-06"), "{}", lines[1]);
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
        let text = table(
            &rows,
            DateColumn::Updated,
            &colored,
            0,
            false,
            true,
            Wrap::NONE,
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
        let text = tree_text(&nodes, 0, &plain(), 0, true, false, Wrap::NONE);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo    || 2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo       2026-09-06"), "{}", lines[1]);
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
        let text = pretty(&out, &plain(), Wrap::NONE);
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
        let text = table(
            &rows,
            DateColumn::Updated,
            &scaled(),
            0,
            false,
            false,
            Wrap::NONE,
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
        let text = table(
            &rows,
            DateColumn::Updated,
            &dated(),
            0,
            false,
            false,
            Wrap::NONE,
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
        let text = table(
            &[due_now, undated],
            DateColumn::Due,
            &dated(),
            0,
            false,
            false,
            Wrap::NONE,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[38;2;0;215;255mnow\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(lines[1].contains("todo    -  "), "{:?}", lines[1]);
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
    fn a_long_title_wraps_under_itself_at_the_terminal_width() {
        let text = table(
            &[long_row()],
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::at(80),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text:?}");
        let title_start = lines[0].find("alpha").expect("title on the first line");
        for line in &lines[1..] {
            let content = line.trim_start();
            assert!(!content.is_empty(), "{text:?}");
            assert_eq!(line.len() - content.len(), title_start, "{text:?}");
        }
        for line in &lines {
            assert!(line.chars().count() <= 80, "{text:?}");
        }
        let mut words = vec![&lines[0][title_start..]];
        words.extend(lines[1..].iter().map(|line| line.trim_start()));
        assert_eq!(words.join(" "), long_row().title, "{text:?}");
    }

    #[test]
    fn wide_characters_fit_the_terminal_columns() {
        let mut row = long_row();
        row.title = "界".repeat(40);
        let text = table(
            &[row],
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::at(80),
        );
        for line in text.lines() {
            let columns: usize = line.chars().map(|c| if c == '界' { 2 } else { 1 }).sum();
            assert!(columns <= 80, "{columns} columns: {line:?}");
        }
    }

    #[test]
    fn a_spaces_only_title_does_not_panic_or_disappear() {
        let mut row = long_row();
        row.title = " ".repeat(40);
        let text = table(
            &[row],
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::at(80),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
        assert!(text.contains(&" ".repeat(40)), "{text:?}");
    }

    #[test]
    fn a_row_that_fits_the_width_stays_one_line_and_byte_identical() {
        let rows = [row("xx-000001", false)];
        let wrapped = table(
            &rows,
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::at(80),
        );
        let unwrapped = table(
            &rows,
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::NONE,
        );
        assert_eq!(wrapped.lines().count(), 1, "{wrapped:?}");
        assert_eq!(wrapped, unwrapped);
    }

    #[test]
    fn a_row_below_the_title_floor_stays_unwrapped() {
        // The fixed columns alone take 41 columns with an empty id column, so at width 60
        // only 19 remain for the title: below the floor, the row overflows unwrapped.
        let text = table(
            &[long_row()],
            DateColumn::Updated,
            &plain(),
            0,
            false,
            false,
            Wrap::at(60),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
    }

    #[test]
    fn a_word_longer_than_the_available_width_is_hard_split() {
        let chars: Vec<(char, Option<Style>)> = "abcdefghij".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 4);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn hard_split_does_not_join_the_previous_word() {
        let chars: Vec<(char, Option<Style>)> =
            "abcd bbbbbbbb".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 5);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["abcd", "bbbbb", "bbb"]);
    }

    #[test]
    fn a_break_drops_the_space_and_keeps_words_inline() {
        let chars: Vec<(char, Option<Style>)> = "ab cd ef".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 6);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["ab cd", "ef"]);
    }

    #[test]
    fn a_suffix_keeps_its_style_on_the_continuation_line() {
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let chars: Vec<(char, Option<Style>)> = "hello "
            .chars()
            .map(|c| (c, Some(Style::Chrome)))
            .chain("world".chars().map(|c| (c, Some(Style::Emphasis))))
            .collect();
        let lines = wrap_lines(&chars, 8);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(paint_runs(&lines[0], &colored), "\x1b[2mhello\x1b[0m");
        assert_eq!(paint_runs(&lines[1], &colored), "\x1b[1mworld\x1b[0m");
    }

    #[test]
    fn a_due_cadence_is_painted_once_when_wrapped() {
        let mut due = recurring("xx-000001");
        due.title = long_row().title;
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = table(
            &[due],
            DateColumn::Due,
            &colored,
            0,
            false,
            true,
            Wrap::at(80),
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
        let render = |painter: &Painter, wrap| {
            table(
                std::slice::from_ref(&quiet),
                DateColumn::Updated,
                painter,
                0,
                false,
                false,
                wrap,
            )
        };
        assert!(render(&plain(), Wrap::NONE).contains(marker));
        let wrapped = render(&plain(), Wrap::at(80));
        assert!(wrapped.lines().count() > 1, "{wrapped:?}");
        let joined: Vec<&str> = wrapped.lines().map(str::trim).collect();
        assert!(joined.join(" ").contains(marker), "{wrapped:?}");
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render(&colored, Wrap::at(80));
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
        let text = tree_text(&nodes, 0, &plain(), 0, false, false, Wrap::at(80));
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

    #[test]
    fn a_parked_row_wraps_its_title() {
        let mut parked = ParkedRow::resolved(long_row(), crate::model::Phase::Implementing);
        parked.park = Some(ParkInfo {
            at: "2026-09-06T00:00:00Z".into(),
            next_step: "next step".into(),
            waiting_on: crate::claims::WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            session: "s".into(),
            owner: "o".into(),
            host: "h".into(),
            worktree: "w".into(),
        });
        let text = parked_table(&[parked], &plain(), 0, Wrap::at(140));
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text:?}");
        let title_start = lines[0].find("alpha").expect("parked title");
        let continuation = lines[1];
        assert_eq!(
            continuation.len() - continuation.trim_start().len(),
            title_start,
            "{text:?}"
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
}
