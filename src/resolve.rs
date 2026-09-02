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
    pub fn dirs(self) -> &'static [&'static str] {
        match self {
            DocKind::Spec => crate::format::SPEC_DIRS,
            DocKind::Plan => crate::format::PLAN_DIRS,
        }
    }

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
        let project;
        let project = if id.prefix == self.project.prefix {
            self.project
        } else {
            let Some(root) = self.registry.project_root(&id.prefix) else {
                return Ok(None);
            };
            if !root.join(crate::repo::CONFIG_REL).try_exists()? {
                return Ok(None);
            }
            project = Project::open(root)?;
            &project
        };
        if !project.task_path(id).try_exists()? {
            return Ok(None);
        }
        match project.read_task(id) {
            Ok(task) => Ok(Some(task)),
            Err(Error::TaskNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn resolve_doc(&self, kind: DocKind, name_or_path: &str) -> Result<String> {
        crate::format::validate_line(kind.name(), name_or_path)?;
        let dirs = kind.dirs();
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

/// `### Task 1: x` -> `Some("Task 1: x")`; non-headings -> None.
pub fn heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    if trimmed.len() == line.len() {
        return None;
    }
    trimmed.strip_prefix(' ').map(str::trim_end)
}
