use crate::error::Error;
use crate::model::{Size, Status, Task};
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
    pub status: Option<String>,
    pub resolved: bool,
}

#[derive(Serialize)]
pub struct Related {
    pub id: String,
    pub title: String,
    pub status: String,
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

#[derive(Serialize)]
pub struct PrimeOut {
    pub prefix: String,
    pub counts: Counts,
    pub ready: Vec<TaskSummary>,
    pub doing: Vec<TaskSummary>,
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
}

pub fn render(out: &Output, format: Format) -> String {
    match format {
        Format::Json => serde_json::to_string(out).expect("output serializes"),
        Format::Pretty => pretty(out),
    }
}

fn pretty(out: &Output) -> String {
    match out {
        Output::Init(o) => o.prefix.clone(),
        Output::Id(o) => o.id.clone(),
        Output::Show(o) => {
            let mut rendered = crate::format::serialize_task(&o.task);
            if !o.depends_on.is_empty() {
                rendered.push_str("\n# depends on\n");
                for dependency in &o.depends_on {
                    let status = dependency.status.as_deref().unwrap_or("?");
                    let title = dependency.title.as_deref().unwrap_or("(unresolved)");
                    rendered.push_str(&format!("- {} [{status}] {title}\n", dependency.id));
                }
            }
            if let Some(found) = o.step_found {
                rendered.push_str(&format!(
                    "\n# step {}\n",
                    if found { "found" } else { "MISSING" }
                ));
            }
            if let Some(parent) = &o.parent {
                rendered.push_str(&format!(
                    "\n# parent\n- {} [{}] {}\n",
                    parent.id, parent.status, parent.title
                ));
            }
            if !o.children.is_empty() {
                rendered.push_str("\n# children\n");
                for child in &o.children {
                    rendered.push_str(&format!(
                        "- {} [{}] {}\n",
                        child.id, child.status, child.title
                    ));
                }
            }
            rendered
        }
        Output::List(o) => table(&o.tasks),
        Output::Prime(o) => {
            let c = &o.counts;
            let mut rendered = format!(
                "project {}\nidea {}  todo {}  doing {}  blocked {}  done {}  dropped {}\n",
                o.prefix, c.idea, c.todo, c.doing, c.blocked, c.done, c.dropped
            );
            rendered.push_str("\nready:\n");
            rendered.push_str(&table(&o.ready));
            rendered.push_str("\ndoing:\n");
            rendered.push_str(&table(&o.doing));
            rendered
        }
        Output::Graph(o) => o.text.clone(),
        Output::Check(o) => {
            let mut rendered = String::new();
            for finding in &o.errors {
                rendered.push_str(&format!(
                    "error: {} [{}] {}\n",
                    finding.file, finding.kind, finding.detail
                ));
            }
            if o.errors.is_empty() {
                rendered.push_str("ok\n");
            }
            rendered
        }
        Output::Tree(o) => tree_text(&o.nodes, 0),
    }
}

fn tree_text(nodes: &[TreeNode], depth: usize) -> String {
    let mut rendered = String::new();
    for node in nodes {
        let row = table(std::slice::from_ref(&node.summary));
        rendered.push_str(&"  ".repeat(depth));
        rendered.push_str(&row);
        rendered.push_str(&tree_text(&node.children, depth + 1));
    }
    rendered
}

pub fn table(rows: &[TaskSummary]) -> String {
    let mut rendered = String::new();
    for row in rows {
        let size = row.size.map(Size::as_str).unwrap_or("-");
        let owner = row.owner.as_deref().unwrap_or("");
        let tags = if row.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", row.tags.join(", "))
        };
        rendered.push_str(&format!(
            "{}  P{} {:<2} {:<7} {}{}{}\n",
            row.id,
            row.priority,
            size,
            row.status.as_str(),
            row.title,
            tags,
            if owner.is_empty() {
                String::new()
            } else {
                format!(" @{owner}")
            }
        ));
    }
    rendered
}

pub fn render_error(e: &Error) -> String {
    serde_json::json!({ "error": { "kind": e.kind(), "detail": e.to_string() } }).to_string()
}

/// stderr text for warnings in pretty mode.
pub fn pretty_warnings(warnings: &[String]) -> String {
    warnings.iter().map(|w| format!("warning: {w}\n")).collect()
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
    }
}
