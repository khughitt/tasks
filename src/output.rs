use crate::error::Error;
use crate::model::{Size, Status, Task};
use crate::style::{Painter, Style};
use serde::Serialize;

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
}

#[derive(Serialize)]
pub struct IdOut {
    pub id: String,
    pub warnings: Vec<String>,
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

#[derive(Serialize)]
pub struct ShowOut {
    pub task: Task,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub step_found: Option<bool>,
    pub depends_on: Vec<DepInfo>,
    pub parent: Option<Related>,
    pub children: Vec<Related>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub priority: u8,
    pub size: Option<Size>,
    pub owner: Option<String>,
    pub updated: String,
    pub tags: Vec<String>,
    pub depends: Vec<String>,
    pub parent: Option<String>,
    pub child_count: usize,
    pub open_descendant_count: usize,
}

impl TaskSummary {
    /// `all` is the scan the row came from; counts are computed against it.
    pub fn of(task: &Task, all: &[Task]) -> TaskSummary {
        TaskSummary {
            id: task.id.to_string(),
            title: task.title.clone(),
            status: task.status,
            priority: task.priority,
            size: task.size,
            owner: task.owner.clone(),
            updated: task.updated.clone(),
            tags: task.tags.clone(),
            depends: task.depends.iter().map(ToString::to_string).collect(),
            parent: task.parent.as_ref().map(ToString::to_string),
            child_count: crate::hierarchy::children(all, &task.id).len(),
            open_descendant_count: crate::hierarchy::open_descendants(all, &task.id).len(),
        }
    }
}

#[derive(Serialize)]
pub struct ListOut {
    pub tasks: Vec<TaskSummary>,
    pub warnings: Vec<String>,
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
}

#[derive(Serialize)]
pub struct PrimeOut {
    /// The local project; null under --all-projects.
    pub prefix: Option<String>,
    /// Every prefix in scope; one entry locally.
    pub projects: Vec<String>,
    pub counts: Counts,
    pub ready: Vec<TaskSummary>,
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
    Id(IdOut),
    Show(Box<ShowOut>),
    List(ListOut),
    Prime(PrimeOut),
    Graph(GraphOut),
    Check(CheckOut),
    Tree(TreeOut),
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
        Output::Id(o) => o.id.clone(),
        Output::Show(o) => {
            let mut rendered = crate::format::serialize_task(&o.task);
            // Footer rows only. The serialize_task text above stays plain: it is file
            // text and has to remain copy-pasteable.
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
            rendered
        }
        Output::List(o) => table(&o.tasks, painter),
        Output::Prime(o) => {
            let c = &o.counts;
            let header = match &o.prefix {
                Some(prefix) => format!("project {prefix}"),
                None => format!("projects {}", o.projects.join(", ")),
            };
            let mut rendered = format!(
                "{header}\nidea {}  todo {}  doing {}  blocked {}  done {}  dropped {}\n",
                c.idea, c.todo, c.doing, c.blocked, c.done, c.dropped
            );
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "closeout:")
            ));
            rendered.push_str(&table(&o.closeout, painter));
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "roadmap:")
            ));
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    rendered.push_str(&tree_text(std::slice::from_ref(node), 0, painter));
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    rendered.push_str(&table(std::slice::from_ref(&node.summary), painter));
                }
            }
            rendered.push_str(&format!(
                "{listed_under_ready} childless root(s) are listed under ready\n"
            ));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "ready:")));
            rendered.push_str(&table(&o.ready, painter));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "doing:")));
            rendered.push_str(&table(&o.doing, painter));
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
        Output::Tree(o) => tree_text(&o.nodes, 0, painter),
        Output::Feedback(o) => format!("{} {}", o.action, o.id),
    }
}

fn tree_text(nodes: &[TreeNode], depth: usize, painter: &Painter) -> String {
    let mut rendered = String::new();
    for node in nodes {
        let row = table(std::slice::from_ref(&node.summary), painter);
        rendered.push_str(&"  ".repeat(depth));
        rendered.push_str(&row);
        rendered.push_str(&tree_text(&node.children, depth + 1, painter));
    }
    rendered
}

/// Pad first, paint last: ANSI bytes count toward `{:<n}` widths, so every width-sensitive
/// field is formatted to its final visible width before the painter wraps it.
pub fn table(rows: &[TaskSummary], painter: &Painter) -> String {
    let mut rendered = String::new();
    for row in rows {
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
        let owner = row
            .owner
            .as_ref()
            .map(|owner| painter.paint(Style::Chrome, &format!(" @{owner}")))
            .unwrap_or_default();
        rendered.push_str(&format!(
            "{id}  {priority} {size:<2} {status} {}{tags}{owner}\n",
            row.title
        ));
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
        Output::Id(o) => o.warnings.clone(),
        Output::Show(o) => o.warnings.clone(),
        Output::List(o) => o.warnings.clone(),
        Output::Prime(o) => o.warnings.clone(),
        Output::Graph(o) => o.warnings.clone(),
        Output::Check(o) => o
            .warnings
            .iter()
            .map(|finding| format!("{} [{}] {}", finding.file, finding.kind, finding.detail))
            .collect(),
        Output::Tree(o) => o.warnings.clone(),
        Output::Feedback(o) => o.warnings.clone(),
    }
}
