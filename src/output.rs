use crate::error::Error;
use crate::model::{Size, Status, Task};
use crate::registry::Registry;
use crate::style::{Painter, Style};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Pretty,
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
    pub prefix: String,
    pub previous: String,
    pub root: String,
    pub tasks: usize,
    pub parks: usize,
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
    pub title: Option<String>,
    /// Typed, but serde still emits the same lowercase strings as before.
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
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub step_found: Option<bool>,
    pub depends_on: Vec<DepInfo>,
    pub parent: Option<Related>,
    pub children: Vec<Related>,
    pub claim: Option<ClaimInfo>,
    pub park: Option<ParkInfo>,
    pub periodic: Option<PeriodicInfo>,
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
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub priority: u8,
    pub size: Option<Size>,
    pub parallel: bool,
    pub owner: Option<String>,
    pub created: String,
    pub updated: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub depends: Vec<String>,
    pub parent: Option<String>,
    pub child_count: usize,
    pub open_descendant_count: usize,
    pub claim: Option<ClaimInfo>,
    pub park: Option<ParkInfo>,
    pub periodic: Option<PeriodicInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimInfo {
    pub owner: String,
    pub session: String,
    pub host: String,
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

/// The park entry as JSON: everything but the title snapshot, which is the row's own
/// `title` (spec §5.4).
#[derive(Debug, Clone, Serialize)]
pub struct ParkInfo {
    pub at: String,
    pub next_step: String,
    pub waiting_on: crate::claims::WaitingOn,
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
            session: park.session.clone(),
            owner: park.owner.clone(),
            host: park.host.clone(),
            worktree: park.worktree.clone(),
        }
    }
}

/// A record's cadence and where it sits in the cycle (spec §5.4). `due_now` is carried
/// explicitly because `due: null` cannot distinguish "not applicable" from "due now with
/// no anchor".
#[derive(Debug, Clone, Serialize)]
pub struct PeriodicInfo {
    pub every: String,
    pub last_done: Option<String>,
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
            parallel: task.parallel,
            owner: task.owner.clone(),
            created: task.created.clone(),
            updated: task.updated.clone(),
            tags: task.tags.clone(),
            source: task.source.clone(),
            model: task.model.clone(),
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
            periodic: PeriodicInfo::of(task, now),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ParkedRow {
    pub id: String,
    pub title: String,
    pub status: Option<Status>,
    pub priority: Option<u8>,
    pub size: Option<Size>,
    pub parallel: bool,
    pub owner: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub depends: Vec<String>,
    pub parent: Option<String>,
    pub child_count: Option<usize>,
    pub open_descendant_count: Option<usize>,
    pub claim: Option<ClaimInfo>,
    pub park: Option<ParkInfo>,
    pub phase: Option<crate::model::Phase>,
}

impl ParkedRow {
    pub fn resolved(summary: TaskSummary, phase: crate::model::Phase) -> ParkedRow {
        ParkedRow {
            id: summary.id,
            title: summary.title,
            status: Some(summary.status),
            priority: Some(summary.priority),
            size: summary.size,
            parallel: summary.parallel,
            owner: summary.owner,
            created: Some(summary.created),
            updated: Some(summary.updated),
            tags: summary.tags,
            source: summary.source,
            model: summary.model,
            depends: summary.depends,
            parent: summary.parent,
            child_count: Some(summary.child_count),
            open_descendant_count: Some(summary.open_descendant_count),
            claim: summary.claim,
            park: summary.park,
            phase: Some(phase),
        }
    }
    pub fn unresolved(id: &str, park: &crate::claims::Park) -> ParkedRow {
        ParkedRow {
            id: id.into(),
            title: park.title.clone(),
            status: None,
            priority: None,
            size: None,
            parallel: false,
            owner: None,
            created: None,
            updated: None,
            tags: Vec::new(),
            source: None,
            model: None,
            depends: Vec::new(),
            parent: None,
            child_count: None,
            open_descendant_count: None,
            claim: None,
            park: Some(ParkInfo::of(park)),
            phase: None,
        }
    }
}

#[derive(Serialize)]
pub struct ParkedOut {
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
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub date: DateColumn,
}

#[derive(Serialize)]
pub struct TreeNode {
    #[serde(flatten)]
    pub summary: TaskSummary,
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
                Status::Done => counts.done += 1,
                Status::Dropped => counts.dropped += 1,
            }
        }
        counts
    }

    pub fn total(&self) -> usize {
        self.idea + self.todo + self.doing + self.blocked + self.done + self.dropped
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

#[derive(Serialize)]
pub struct PrimeOut {
    /// The local project; null under --all-projects.
    pub prefix: Option<String>,
    /// Every prefix in scope; one entry locally.
    pub projects: Vec<String>,
    pub counts: Counts,
    pub periodic: PeriodicSummary,
    /// Pretty-only, like `ProjectsOut`: JSON always carries every count.
    #[serde(skip)]
    pub closed: bool,
    pub ready: Vec<TaskSummary>,
    pub parked: Vec<ParkedRow>,
    pub doing: Vec<TaskSummary>,
    pub roadmap: Vec<TreeNode>,
    pub closeout: Vec<TaskSummary>,
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
    Prime(PrimeOut),
    Graph(GraphOut),
    Check(CheckOut),
    Tree(TreeOut),
    Tags(TagsOut),
    Feedback(FeedbackOut),
}

pub fn render(out: &Output, format: Format, painter: &Painter) -> String {
    match format {
        Format::Json => serde_json::to_string(out).expect("output serializes"),
        Format::Pretty => pretty(out, painter),
    }
}

fn pretty(out: &Output, painter: &Painter) -> String {
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
                    (Some(_), Some(at)) => cell(crate::time::day(at), Align::Left, None),
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
        Output::Next(o) => match &o.next {
            Some(fields) => show_text(fields, painter),
            None => "nothing ready".into(),
        },
        Output::List(o) => table(&o.tasks, o.date, painter, any_parallel(&o.tasks)),
        Output::Parked(o) => parked_table(&o.tasks, painter),
        Output::Prime(o) => {
            // One decision for the whole output: prime's blocks align today only because
            // every width is fixed, and a per-section decision would break that.
            let parallel_column = any_parallel(&o.closeout)
                || any_parallel_tree(&o.roadmap)
                || any_parallel(&o.ready)
                || any_parallel(&o.doing);
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
            let mut rendered = format!("{header}\n{}\n", counts.join("  "));
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

            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "closeout:")
            ));
            rendered.push_str(&table(
                &o.closeout,
                DateColumn::Updated,
                painter,
                parallel_column,
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
                        parallel_column,
                    ));
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    rendered.push_str(&table(
                        std::slice::from_ref(&node.summary),
                        DateColumn::Updated,
                        painter,
                        parallel_column,
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
            rendered.push_str(&parked_table(&o.parked, painter));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "ready:")));
            rendered.push_str(&table(
                &o.ready,
                DateColumn::Updated,
                painter,
                parallel_column,
            ));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "doing:")));
            rendered.push_str(&table(
                &o.doing,
                DateColumn::Updated,
                painter,
                parallel_column,
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
        Output::Tree(o) => tree_text(&o.nodes, 0, painter, any_parallel_tree(&o.nodes)),
        Output::Tags(o) => {
            let mut rendered = String::new();
            for row in &o.tags {
                let parts: Vec<String> = row
                    .projects
                    .iter()
                    .map(|(prefix, count)| format!("{prefix} {count}"))
                    .collect();
                let breakdown = painter.paint(Style::Chrome, &format!("  ({})", parts.join(", ")));
                rendered.push_str(&format!("{:>4}  {}{breakdown}\n", row.count, row.tag));
            }
            rendered
        }
        Output::Feedback(o) => format!("{} {}", o.action, o.id),
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
        "priority" if task.priority <= 1 => Style::Emphasis,
        _ => return line.into(),
    };
    format!("{key}: {}", painter.paint(style, value))
}

fn show_text(o: &ShowFields, painter: &Painter) -> String {
    let mut rendered = paint_frontmatter(&crate::format::serialize_task(&o.task), &o.task, painter);
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
    if let Some(park) = &o.park {
        rendered.push_str("\n# parked\n");
        rendered.push_str(&format!(
            "- waiting on {} since {}: {}\n",
            park.waiting_on.as_str(),
            crate::time::day(&park.at),
            park.next_step
        ));
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("  session {} in {}", park.session, park.worktree),
        ));
        rendered.push('\n');
    }
    rendered
}

fn tree_text(nodes: &[TreeNode], depth: usize, painter: &Painter, parallel_column: bool) -> String {
    let mut rendered = String::new();
    for node in nodes {
        let row = table(
            std::slice::from_ref(&node.summary),
            DateColumn::Updated,
            painter,
            parallel_column,
        );
        rendered.push_str(&"  ".repeat(depth));
        rendered.push_str(&row);
        rendered.push_str(&tree_text(
            &node.children,
            depth + 1,
            painter,
            parallel_column,
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

/// Pad first, paint last: ANSI bytes count toward `{:<n}` widths, so every width-sensitive
/// field is formatted to its final visible width before the painter wraps it.
pub fn table(
    rows: &[TaskSummary],
    date: DateColumn,
    painter: &Painter,
    parallel_column: bool,
) -> String {
    let mut rendered = String::new();
    for row in rows {
        let date = match date {
            DateColumn::Updated => crate::time::day(&row.updated).to_string(),
            DateColumn::Created => crate::time::day(&row.created).to_string(),
            DateColumn::Due => match &row.periodic {
                Some(periodic) => match (&periodic.due, periodic.due_now) {
                    (Some(due), _) => crate::time::day(due).to_string(),
                    (None, true) => "now".into(),
                    (None, false) => "-".into(),
                },
                None => "-".into(),
            },
        };
        let id = painter.paint(Style::Chrome, &row.id);
        let priority = format!("P{}", row.priority);
        let priority = if row.priority <= 1 {
            painter.paint(Style::Emphasis, &priority)
        } else {
            priority
        };
        let size = row.size.map(Size::as_str).unwrap_or("-");
        let status = painter.paint(
            Style::Status(row.status),
            &format!("{:<7}", row.status.as_str()),
        );
        let tags = if row.tags.is_empty() {
            String::new()
        } else {
            painter.paint(Style::Chrome, &format!(" [{}]", row.tags.join(", ")))
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
                painter.paint(
                    Style::Emphasis,
                    &format!("  every {}, due {when}", periodic.every),
                )
            }
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
        let owner = painter.paint(Style::Chrome, &owner);
        // Unpainted: it is already distinct, and painting the blank spacer would wrap
        // whitespace in ANSI for no gain.
        let mark = match (parallel_column, row.parallel) {
            (false, _) => "",
            (true, true) => "|| ",
            (true, false) => "   ",
        };
        rendered.push_str(&format!(
            "{id}  {priority} {size:<2} {status} {mark}{date}  {}{tags}{cadence}{owner}\n",
            row.title
        ));
    }
    rendered
}

pub fn parked_table(rows: &[ParkedRow], painter: &Painter) -> String {
    let mut rendered = String::new();
    for row in rows {
        let Some(park) = &row.park else {
            continue;
        };
        let id = painter.paint(Style::Chrome, &row.id);
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
        rendered.push_str(&format!(
            "{id}  {status} {phase} waits on {:<5} {}  {}\n",
            park.waiting_on.as_str(),
            crate::time::day(&park.at),
            row.title
        ));
        rendered
            .push_str(&painter.paint(Style::Chrome, &format!("        next: {}", park.next_step)));
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
            owner: None,
            created: "2026-09-06T00:00:00Z".into(),
            updated: "2026-09-06T00:00:00Z".into(),
            tags: vec![],
            source: None,
            model: None,
            depends: vec![],
            parent: None,
            child_count: 0,
            open_descendant_count: 0,
            claim: None,
            park: None,
            periodic: None,
            parallel,
        }
    }

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, Format::Pretty, false)
    }

    #[test]
    fn the_marker_column_is_absent_when_nothing_is_marked() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_parallel(&rows));
        let text = table(&rows, DateColumn::Updated, &plain(), false);
        assert!(!text.contains("||"), "{text}");
        assert!(text.contains("todo    2026-09-06"), "{text}");
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
        let text = tree_text(&nodes, 0, &plain(), true);
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
}
