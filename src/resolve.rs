use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Spec,
    Plan,
}

impl DocKind {
    pub fn name(self) -> &'static str {
        match self {
            DocKind::Spec => "spec",
            DocKind::Plan => "plan",
        }
    }
}

pub struct Resolver<'a> {
    pub project: &'a Project,
    pub registry: &'a Registry,
}

impl<'a> Resolver<'a> {
    pub fn new(project: &'a Project, registry: &'a Registry) -> Resolver<'a> {
        Resolver { project, registry }
    }

    /// `Ok(None)` when the id is unreachable (unregistered prefix, missing root, or
    /// missing file); `Err` when a file exists but cannot be parsed.
    pub fn resolve_task(&self, id: &TaskId) -> Result<Option<Task>> {
        if id.prefix == self.project.prefix {
            read_present(self.project, id)
        } else {
            resolve_registered(self.registry, id)
        }
    }

    /// The project's configured roots for `kind`: the validation boundary for explicit
    /// paths and the search path for bare names.
    pub fn dirs(&self, kind: DocKind) -> &[String] {
        match kind {
            DocKind::Spec => &self.project.spec_dirs,
            DocKind::Plan => &self.project.plan_dirs,
        }
    }

    pub fn resolve_doc(&self, kind: DocKind, name_or_path: &str) -> Result<String> {
        crate::format::validate_line(kind.name(), name_or_path)?;
        let dirs = self.dirs(kind);
        if name_or_path.contains('/') || name_or_path.ends_with(".md") {
            crate::format::validate_doc_path(kind.name(), dirs, name_or_path)?;
            if !self.project.root.join(name_or_path).is_file() {
                return Err(Error::DocNotFound(format!(
                    "{} {name_or_path:?} does not exist",
                    kind.name()
                )));
            }
            return Ok(name_or_path.to_string());
        }

        let mut matches = Vec::new();
        for dir in dirs {
            let full = self.project.root.join(dir);
            if full.is_dir() {
                for entry in std::fs::read_dir(full)? {
                    let path = entry?.path();
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if path.is_file() && name.ends_with(".md") && name.contains(name_or_path) {
                        matches.push(format!("{dir}/{name}"));
                    }
                }
            }
        }
        matches.sort();
        match matches.len() {
            0 => Err(Error::DocNotFound(format!(
                "no {} matching {name_or_path:?} under {}/",
                kind.name(),
                dirs.join("/ or ")
            ))),
            1 => Ok(matches.remove(0)),
            _ => Err(Error::Ambiguous(format!(
                "{name_or_path:?} matches several {}s: {}",
                kind.name(),
                matches.join(", ")
            ))),
        }
    }

    pub fn step_exists(&self, plan_rel: &str, step: &str) -> Result<bool> {
        let text = std::fs::read_to_string(self.project.root.join(plan_rel))?;
        Ok(text.lines().any(|line| heading_text(line) == Some(step)))
    }

    pub fn abs(&self, rel: &str) -> String {
        Path::new(&self.project.root)
            .join(rel)
            .display()
            .to_string()
    }
}

/// The task if its file exists in `project`; `Ok(None)` when it does not.
pub fn read_present(project: &Project, id: &TaskId) -> Result<Option<Task>> {
    if !project.task_path(id).try_exists()? {
        return Ok(None);
    }
    match project.read_task(id) {
        Ok(task) => Ok(Some(task)),
        Err(Error::TaskNotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Follows a foreign id through the registry. Lenient on purpose: an unregistered
/// prefix or a missing root or config is `Ok(None)`, because callers report those as
/// unreachable-dependency warnings. Once those cases are excluded, the strict shared
/// opener makes malformed config or a registry/config prefix mismatch a config error.
pub fn resolve_registered(registry: &Registry, id: &TaskId) -> Result<Option<Task>> {
    let Some(root) = registry.project_root(&id.prefix) else {
        return Ok(None);
    };
    if !crate::scope::has_config(root)? {
        return Ok(None);
    }
    let project =
        crate::scope::open_registered(registry, &id.prefix, crate::scope::Origin::Id(id))?;
    read_present(&project, id)
}

/// `### Task 1: x` -> `Some("Task 1: x")`; non-headings -> None.
pub fn heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    if trimmed.len() == line.len() {
        return None;
    }
    trimmed.strip_prefix(' ').map(str::trim_end)
}

/// Heading texts of the form `Task <digits>: …`, in file order.
pub fn step_headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(heading_text)
        .filter(|heading| {
            heading
                .strip_prefix("Task ")
                .and_then(|rest| rest.split_once(':'))
                .is_some_and(|(number, _)| {
                    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
                })
        })
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_headings_match_only_the_task_n_convention() {
        let text = "# P\n### Task 1: one\n## Task 12: twelve\n### Notes on Task 3\n### Task x: no\nTask 4: not a heading\n";
        assert_eq!(step_headings(text), ["Task 1: one", "Task 12: twelve"]);
    }
}
