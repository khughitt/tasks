use crate::lanes::LaneRow;
use crate::model::{Complexity, Process, Size, Status, Task, TaskId};
use serde::Serialize;

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

/// One reason a decision list cannot run, and the list key or command that settles it.
#[derive(Serialize)]
pub struct RefusalOut {
    pub id: String,
    pub rule: String,
    pub settle: String,
}

#[derive(Serialize)]
pub struct BodyChange {
    pub id: String,
    pub diff: String,
}

#[derive(Serialize)]
pub struct CarriedOut {
    pub from: String,
    /// Assigned only when a run is being resumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub parent: Option<String>,
    pub rewrites: Vec<String>,
    pub attachments: Vec<String>,
    pub park: bool,
}

#[derive(Serialize)]
pub struct DroppedOut {
    pub id: String,
    pub reason: String,
    pub released: Vec<String>,
}

#[derive(Serialize)]
pub struct ClosedOut {
    pub id: String,
    pub reason: String,
    pub open_depends: Vec<String>,
    pub released: Vec<String>,
}

#[derive(Serialize)]
pub struct InboundOut {
    pub id: String,
    pub dependents: Vec<String>,
}

#[derive(Serialize)]
pub struct WokenOut {
    pub id: String,
    pub shelves: Vec<String>,
}

/// `tasks migrate <list>`: docs/task-model.md `refusals` and `body_changes` appear on `--explain`.
#[derive(Serialize)]
pub struct MigrateOut {
    pub source: String,
    pub successor: String,
    /// `sha256:<64 hex>` of the list file.
    pub list: String,
    pub recovery: String,
    pub carried: Vec<CarriedOut>,
    pub dropped: Vec<DroppedOut>,
    pub closed: Vec<ClosedOut>,
    pub inbound: Vec<InboundOut>,
    pub woken: Vec<WokenOut>,
    pub unread_projects: Vec<String>,
    pub paths: std::collections::BTreeMap<String, Vec<String>>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<RefusalOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub body_changes: Vec<BodyChange>,
    /// Titles for the pretty review, keyed by v1 id and, where assigned, new id. Not JSON.
    #[serde(skip)]
    pub titles: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct DraftOut {
    pub path: String,
    pub tasks: usize,
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
pub struct ExportOut {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub integrity: crate::multipart::publication::Integrity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<crate::multipart::validate::Validation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct RawOut {
    pub bytes: Vec<u8>,
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

/// `tasks where` (docs/project-operations.md). Every project object carries `root`,
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

/// One input's answer (docs/task-model.md). Fields that do not apply are omitted.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multipart: Option<MultipartRefOut>,
}

#[derive(Serialize)]
pub struct MultipartRefOut {
    pub name: String,
    pub parts: usize,
}

/// Everything `show` says about one task, without the warnings, so `next` can embed it.
#[derive(Serialize)]
pub struct ShowFields {
    pub task: Task,
    pub needs_review: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_reason: Option<crate::review::ReviewReason>,
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
    pub multipart: Vec<MultipartOut>,
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

/// Each in-scope prefix's configured colour (docs/user-guide.md).
pub type ProjectColors = std::collections::HashMap<String, crate::palette::Rgb>;

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
    /// The row needs a look (docs/user-guide.md). Computed from the scan, never stored.
    pub needs_review: bool,
    /// Why the row needs a look (docs/user-guide.md); present exactly when
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wake_on: Vec<TaskId>,
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
    /// Why a held claim could not be judged from here (docs/ownership.md). `live`
    /// stays `true` for it, so consumers that read `false` as "free" stay safe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unverified: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
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
/// nothing partial to warn about (docs/ownership.md)
///.
#[derive(Serialize)]
pub struct ClaimsOut {
    pub claims: Vec<ClaimRow>,
}

/// The park entry as JSON: everything but the title snapshot, which is the row's own
/// `title` (docs/task-model.md).
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

/// A record's cadence and where it sits in the cycle (docs/task-model.md). `due_now` is carried
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

/// One attachment as `show`, `next`, and `attach` report it. `path` is absolute.
#[derive(Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_of: Option<String>,
}

#[derive(Serialize)]
pub struct MultipartOut {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub parts: usize,
    pub catalog: String,
    pub caption: Option<String>,
}

#[derive(Serialize)]
pub struct AttachOut {
    pub id: String,
    pub files: Vec<FileInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multipart: Option<MultipartOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication: Option<crate::multipart::publication::Publication>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<crate::multipart::validate::Validation>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct ArchivePreview {
    pub id: String,
    pub source_sha256: String,
    pub archive_name: String,
    pub archive_bytes: u64,
    pub note_count: usize,
    pub record_bytes_before: u64,
    pub record_bytes_after: u64,
    pub eligible: bool,
    pub reasons: Vec<String>,
}

#[derive(Serialize)]
pub struct ArchiveApplyOut {
    pub id: String,
    pub source_sha256: String,
    pub archive_name: String,
    pub record_bytes_before: u64,
    pub record_bytes_after: u64,
    pub action: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DetachOut {
    pub id: String,
    pub name: String,
    pub path: String,
    /// False when the ledger alone recorded the detach: no file was present.
    pub removed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication: Option<crate::multipart::publication::Publication>,
    pub warnings: Vec<String>,
}

/// A record's deferral and whether the clock has spent it (docs/task-model.md). Named apart from
/// the raw `Task.defer` string so the two shapes never collide.
#[derive(Debug, Clone, Serialize)]
pub struct DeferredInfo {
    pub until: String,
    pub due: bool,
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
    /// From the record; omitted on an unresolved row (docs/user-guide.md).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_review: Option<bool>,
    /// Omitted on an unresolved row, as `needs_review` is (docs/user-guide.md).
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wake_on: Vec<TaskId>,
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

/// `tasks goals` (docs/user-guide.md).
#[derive(Serialize)]
pub struct GoalsOut {
    pub observed_at: String,
    pub park_evidence: ParkEvidence,
    pub goals: Vec<crate::goals::GoalRow>,
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

/// What `prime` says about the cadences that are not yet due (docs/task-model.md). Due records are
/// work, not schedule, and are counted nowhere here.
#[derive(Serialize, Default)]
pub struct PeriodicSummary {
    pub scheduled: usize,
    pub next_due: Option<String>,
    /// Pretty-only, like `PrimeOut::closed`: calendar days from the command's captured
    /// `now` to `next_due` (docs/task-model.md), so the line can say "in 7d" without reaching for a
    /// second clock.
    #[serde(skip)]
    pub in_days: Option<i64>,
}

/// The `prime` line for deferrals (defer): what is waiting, the soonest date, and
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

/// `tasks lanes` (docs/user-guide.md).
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub growth: Option<Vec<crate::growth::GrowthRow>>,
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
    RetainedState(crate::retained::state::RetainedState),
    RetainedMutation(crate::retained::model::MutationResult),
    AttachmentEligibility(AttachmentEligibilityOut),
    Init(InitOut),
    Rename(RenameOut),
    Move(MoveOut),
    MigrateDraft(DraftOut),
    Migrate(MigrateOut),
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
    Raw(RawOut),
    Export(ExportOut),
    Next(Box<NextOut>),
    List(ListOut),
    Parked(ParkedOut),
    Quiet(QuietOut),
    Goals(GoalsOut),
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
    ArchivePreview(ArchivePreview),
    ArchiveApply(ArchiveApplyOut),
    Detach(DetachOut),
    Retire(RetireOut),
}
