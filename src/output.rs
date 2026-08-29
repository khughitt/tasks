use crate::error::Error;
use crate::model::Task;
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
pub struct ShowOut {
    pub task: Task,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub step_found: Option<bool>,
    pub depends_on: Vec<DepInfo>,
    pub warnings: Vec<String>,
}

/// One variant per command payload. Later tasks add variants; `pretty` grows with them.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Output {
    Init(InitOut),
    Id(IdOut),
    Show(ShowOut),
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
            rendered
        }
    }
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
    }
}
