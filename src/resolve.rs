use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

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

/// Where a linked spec or plan is read from. A linked worktree lacks every document its
/// project keeps out of git (`.git/info/exclude`, a gitignore) while the main checkout still
/// holds it, so a document absent here is looked for in the main checkout before it counts
/// as missing; otherwise every fresh worktree of such a project fails its own gate.
pub enum Located {
    Here(PathBuf),
    Main { root: PathBuf, path: PathBuf },
    Missing,
}

impl Located {
    pub fn into_path(self) -> Option<PathBuf> {
        match self {
            Located::Here(path) | Located::Main { path, .. } => Some(path),
            Located::Missing => None,
        }
    }
}

pub struct Resolver<'a> {
    pub project: &'a Project,
    pub registry: &'a Registry,
    /// Asked of git once, and only when some document is absent here.
    main: OnceCell<Option<PathBuf>>,
    /// One line per document read from the main checkout, for the caller's warnings.
    from_main: RefCell<Vec<String>>,
}

impl<'a> Resolver<'a> {
    pub fn new(project: &'a Project, registry: &'a Registry) -> Resolver<'a> {
        Resolver {
            project,
            registry,
            main: OnceCell::new(),
            from_main: RefCell::new(Vec::new()),
        }
    }

    /// `Ok(None)` when the id is unreachable (unregistered prefix, missing root, or
    /// missing file); `Err` when a file exists but cannot be parsed.
    pub fn resolve_task(&self, id: &TaskId) -> Result<Option<Task>> {
        let id = &self.registry.canonical_id(id);
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

    /// The main checkout's root when this project is a linked worktree of it.
    fn main_root(&self) -> Result<Option<&PathBuf>> {
        if let Some(main) = self.main.get() {
            return Ok(main.as_ref());
        }
        let main = self.project.main_checkout_root()?;
        Ok(self.main.get_or_init(|| main).as_ref())
    }

    /// `rel` in this checkout, else in the main checkout (see [`Located`]).
    pub fn locate(&self, rel: &str) -> Result<Located> {
        let here = self.project.root.join(rel);
        if here.is_file() {
            return Ok(Located::Here(here));
        }
        Ok(match self.main_root()? {
            Some(root) if root.join(rel).is_file() => Located::Main {
                root: root.clone(),
                path: root.join(rel),
            },
            _ => Located::Missing,
        })
    }

    /// The file `rel` is read from, noting a main-checkout copy for [`Self::take_warnings`].
    fn find(&self, kind: DocKind, rel: &str) -> Result<Option<PathBuf>> {
        Ok(match self.locate(rel)? {
            Located::Here(path) => Some(path),
            Located::Main { root, path } => {
                self.note_main(kind, rel, &root);
                Some(path)
            }
            Located::Missing => None,
        })
    }

    fn note_main(&self, kind: DocKind, rel: &str, root: &Path) {
        let line = format!(
            "{} {rel} is not in this checkout; read from the main checkout at {}",
            kind.name(),
            root.display()
        );
        let mut from_main = self.from_main.borrow_mut();
        if !from_main.contains(&line) {
            from_main.push(line);
        }
    }

    /// The warnings for documents read from the main checkout so far, once each.
    pub fn take_warnings(&self) -> Vec<String> {
        self.from_main.take()
    }

    /// A bare name is searched for in this checkout's roots and, only when none matches
    /// here, in the main checkout's.
    pub fn resolve_doc(&self, kind: DocKind, name_or_path: &str) -> Result<String> {
        crate::format::validate_line(kind.name(), name_or_path)?;
        let dirs = self.dirs(kind);
        if name_or_path.contains('/') || name_or_path.ends_with(".md") {
            crate::format::validate_doc_path(kind.name(), dirs, name_or_path)?;
            if self.find(kind, name_or_path)?.is_none() {
                return Err(Error::DocNotFound(format!(
                    "{} {name_or_path:?} does not exist",
                    kind.name()
                )));
            }
            return Ok(name_or_path.to_string());
        }

        let mut matches = doc_matches(&self.project.root, dirs, name_or_path)?;
        let mut main = None;
        if matches.is_empty()
            && let Some(root) = self.main_root()?
        {
            matches = doc_matches(root, dirs, name_or_path)?;
            main = Some(root);
        }
        match matches.len() {
            0 => Err(Error::DocNotFound(format!(
                "no {} matching {name_or_path:?} under {}/{}",
                kind.name(),
                dirs.join("/ or "),
                crate::format::roots_hint(kind.name())
            ))),
            1 => {
                let found = matches.remove(0);
                if let Some(root) = main {
                    self.note_main(kind, &found, root);
                }
                Ok(found)
            }
            _ => Err(Error::Ambiguous(format!(
                "{name_or_path:?} matches several {}s: {}",
                kind.name(),
                matches.join(", ")
            ))),
        }
    }

    /// A plan absent from both checkouts is the read error it always was.
    pub fn step_exists(&self, plan_rel: &str, step: &str) -> Result<bool> {
        let path = self
            .find(DocKind::Plan, plan_rel)?
            .unwrap_or_else(|| self.project.root.join(plan_rel));
        Ok(has_heading(&std::fs::read_to_string(path)?, step))
    }

    /// Where `rel` is read from; this checkout's path when it is in neither.
    pub fn abs(&self, kind: DocKind, rel: &str) -> Result<String> {
        let path = self
            .find(kind, rel)?
            .unwrap_or_else(|| self.project.root.join(rel));
        Ok(path.display().to_string())
    }
}

/// `<dir>/<file>` for each `.md` file under `root`'s `dirs` whose name contains `name`,
/// sorted.
fn doc_matches(root: &Path, dirs: &[String], name: &str) -> Result<Vec<String>> {
    let mut matches = Vec::new();
    for dir in dirs {
        let full = root.join(dir);
        if full.is_dir() {
            for entry in std::fs::read_dir(full)? {
                let path = entry?.path();
                let Some(file) = path.file_name().and_then(|file| file.to_str()) else {
                    continue;
                };
                if path.is_file() && file.ends_with(".md") && file.contains(name) {
                    matches.push(format!("{dir}/{file}"));
                }
            }
        }
    }
    matches.sort();
    Ok(matches)
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
    let id = &registry.canonical_id(id);
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

/// Whether `text` has a markdown heading reading exactly `step`.
pub fn has_heading(text: &str, step: &str) -> bool {
    text.lines().any(|line| heading_text(line) == Some(step))
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
