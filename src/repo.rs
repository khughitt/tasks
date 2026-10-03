use crate::error::{Error, Result};
use crate::format::{parse_task, serialize_task, validate_doc_path};
use crate::model::{Task, TaskId, is_valid_prefix};
use crate::registry::Registry;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CONFIG_REL: &str = "tasks/.config.toml";

pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    atomic_write_with(path, contents, || fastrand::u32(..0x100_0000))
}

fn atomic_write_with(
    path: &Path,
    contents: &[u8],
    mut candidate: impl FnMut() -> u32,
) -> Result<()> {
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} has no file name", path.display()),
        )
    })?;
    for _ in 0..16 {
        let mut temp_name = OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(".{:06x}.tmp", candidate()));
        let temp = path.with_file_name(temp_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(contents) {
                    drop(file);
                    let _ = std::fs::remove_file(&temp);
                    return Err(error.into());
                }
                drop(file);
                if let Err(error) = std::fs::rename(&temp, path) {
                    let _ = std::fs::remove_file(&temp);
                    return Err(error.into());
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(Error::Io(format!(
        "could not allocate an atomic temp for {} after 16 attempts",
        path.display()
    )))
}

pub const DEFAULT_SPEC_DIRS: &[&str] = &[
    "docs/specs",
    "docs/designs",
    "docs/superpowers/specs",
    "docs/superpowers/designs",
];
pub const DEFAULT_PLAN_DIRS: &[&str] = &["docs/plans", "docs/superpowers/plans"];

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub prefix: String,
    pub color: Option<crate::palette::Rgb>,
    /// Roots a `spec` link may live under; also the search path for bare spec names.
    pub spec_dirs: Vec<String>,
    /// Roots a `plan` link may live under; also the search path for bare plan names.
    pub plan_dirs: Vec<String>,
    /// The tag dictionary: tag -> one-line meaning, from the config's `[tags]` table.
    /// `None` when the project keeps no dictionary; then `check` says nothing about
    /// tags. See docs/specs/2026-09-11-tag-dictionary-design.md.
    pub tags: Option<BTreeMap<String, String>>,
    /// The `[feedback]` table's scope: `Some` means the project accepts feedback, and the
    /// line says what it owns. See ops docs/specs/2026-09-26-ecosystem-feedback-design.md.
    pub feedback: Option<String>,
    /// The need vocabulary from the config's `[needs]` tables: name -> meaning and
    /// exclusivity. Empty when the project declares none. See
    /// docs/specs/2026-10-03-lanes-needs-groups-design.md §4.1.
    pub needs: crate::needs::Vocabulary,
    /// The per-file cap from `[attachments] max_bytes`; `attachments::DEFAULT_MAX_BYTES` when unset.
    pub attachments_max_bytes: u64,
}

/// One other checkout's copy of a record, as `sibling_task_copies` found it.
#[derive(Debug)]
pub enum SiblingCopy {
    /// The project root the copy was found in, the `updated` stamp it carries there, and
    /// its bytes, which a same-stamp comparison needs (record-home spec §3.1).
    Found {
        root: PathBuf,
        updated: String,
        raw: String,
    },
    /// The copy is present but could not be read or parsed.
    Unreadable { root: PathBuf, detail: String },
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Config {
    prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spec_dirs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan_dirs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tags: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    feedback: Option<FeedbackConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attachments: Option<AttachmentsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    needs: Option<crate::needs::Vocabulary>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackConfig {
    scope: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_bytes: Option<u64>,
}

/// The per-file attachment cap: positive, or the default when the table or key is absent.
/// A negative value fails in the TOML parse; zero fails here.
fn attachments_max_bytes(raw: Option<AttachmentsConfig>) -> Result<u64> {
    match raw.and_then(|table| table.max_bytes) {
        None => Ok(crate::attachments::DEFAULT_MAX_BYTES),
        Some(0) => Err(Error::Config(format!(
            "{CONFIG_REL}: [attachments] max_bytes must be a positive integer"
        ))),
        Some(bytes) => Ok(bytes),
    }
}

/// A dictionary entry is a valid tag with a one-line, non-empty meaning; anything else
/// is a config error naming the entry.
fn tag_dictionary(
    raw: Option<BTreeMap<String, String>>,
) -> Result<Option<BTreeMap<String, String>>> {
    let Some(entries) = raw else {
        return Ok(None);
    };
    for (tag, meaning) in &entries {
        crate::format::validate_line("tag", tag)
            .and_then(|()| crate::format::validate_line("meaning", meaning))
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: [tags] {tag:?}: {error}")))?;
    }
    Ok(Some(entries))
}

/// Each `[needs]` name follows the tag grammar and each meaning is one non-empty line;
/// anything else is a config error naming the entry. Unknown keys and a missing meaning
/// already failed the TOML parse (`deny_unknown_fields`).
fn need_vocabulary(raw: Option<crate::needs::Vocabulary>) -> Result<crate::needs::Vocabulary> {
    let vocabulary = raw.unwrap_or_default();
    for (name, decl) in &vocabulary {
        crate::needs::validate_name(name)
            .and_then(|()| crate::format::validate_line("meaning", &decl.meaning))
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: [needs] {name:?}: {error}")))?;
    }
    Ok(vocabulary)
}

/// The scope of a `[feedback]` table: no control character (Unicode `Cc`: line breaks,
/// tabs, and the rest) and at least one non-whitespace character, since a blank owner
/// line would route no one anywhere. ops's `ops-projects` applies the same rule to the
/// same file (`feedback_scope_problem`); change both together.
fn feedback_scope(raw: Option<FeedbackConfig>) -> Result<Option<String>> {
    let Some(table) = raw else {
        return Ok(None);
    };
    if table.scope.chars().any(char::is_control) || table.scope.chars().all(char::is_whitespace) {
        return Err(Error::Config(format!(
            "{CONFIG_REL}: [feedback] scope must be one line of text with no control characters"
        )));
    }
    Ok(Some(table.scope))
}

/// Normalizes a configured doc root: one trailing slash is dropped; anything that is not a
/// plain relative path (`a/b`, no `.`/`..`/empty segments) is a config error.
fn doc_root(key: &str, raw: &str) -> Result<String> {
    let dir = raw.strip_suffix('/').unwrap_or(raw);
    let normalized = !dir.is_empty()
        && dir
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    if !normalized {
        return Err(Error::Config(format!(
            "{CONFIG_REL}: {key} entry {raw:?} must be a normalized relative path"
        )));
    }
    Ok(dir.to_string())
}

fn doc_roots(key: &str, configured: Option<Vec<String>>, defaults: &[&str]) -> Result<Vec<String>> {
    let Some(configured) = configured else {
        return Ok(defaults.iter().map(|dir| dir.to_string()).collect());
    };
    if configured.is_empty() {
        return Err(Error::Config(format!(
            "{CONFIG_REL}: {key} must list at least one directory"
        )));
    }
    configured.iter().map(|raw| doc_root(key, raw)).collect()
}

impl Project {
    pub fn init(root: &Path, prefix: &str) -> Result<Project> {
        if !is_valid_prefix(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
            )));
        }
        let config = root.join(CONFIG_REL);
        if config.exists() {
            let existing = Self::open(root)?;
            if existing.prefix != prefix {
                return Err(Error::Config(format!(
                    "{} already exists with prefix {:?}",
                    config.display(),
                    existing.prefix
                )));
            }
        }
        std::fs::create_dir_all(root.join("tasks"))?;
        if !config.exists() {
            let text = toml::to_string(&Config {
                prefix: prefix.into(),
                color: None,
                spec_dirs: None,
                plan_dirs: None,
                tags: None,
                feedback: None,
                attachments: None,
                needs: None,
            })
            .expect("config serializes");
            atomic_write(&config, text.as_bytes())?;
        }
        let project = Self::open(root)?;
        std::fs::create_dir_all(project.root.join(&project.spec_dirs[0]))?;
        std::fs::create_dir_all(project.root.join(&project.plan_dirs[0]))?;
        Ok(project)
    }

    pub fn open(root: &Path) -> Result<Project> {
        let root = root.canonicalize()?;
        let text = std::fs::read_to_string(root.join(CONFIG_REL))?;
        let config: Config = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
        if !is_valid_prefix(&config.prefix) {
            return Err(Error::Config(format!(
                "{CONFIG_REL}: bad prefix {:?}",
                config.prefix
            )));
        }
        Ok(Project {
            root,
            prefix: config.prefix,
            color: config
                .color
                .map(|color| {
                    crate::palette::Rgb::parse_hex(&color).ok_or_else(|| {
                        Error::Config(format!("{CONFIG_REL}: color {color:?} must be #RRGGBB"))
                    })
                })
                .transpose()?,
            spec_dirs: doc_roots("spec_dirs", config.spec_dirs, DEFAULT_SPEC_DIRS)?,
            plan_dirs: doc_roots("plan_dirs", config.plan_dirs, DEFAULT_PLAN_DIRS)?,
            tags: tag_dictionary(config.tags)?,
            feedback: feedback_scope(config.feedback)?,
            needs: need_vocabulary(config.needs)?,
            attachments_max_bytes: attachments_max_bytes(config.attachments)?,
        })
    }

    /// Checks that a task's `spec`/`plan` links lie under this project's configured roots.
    /// Path shape (normalized, `.md`) is already covered by `validate_task`.
    pub fn validate_docs(&self, task: &Task) -> Result<()> {
        if let Some(spec) = &task.spec {
            validate_doc_path("spec", &self.spec_dirs, spec)?;
        }
        if let Some(plan) = &task.plan {
            validate_doc_path("plan", &self.plan_dirs, plan)?;
        }
        Ok(())
    }

    pub fn locate(start: &Path) -> Result<Project> {
        let start = start.canonicalize()?;
        let mut directory = Some(start.as_path());
        while let Some(path) = directory {
            if path.join(CONFIG_REL).is_file() {
                return Self::open(path);
            }
            directory = path.parent();
        }
        Err(Error::NoProject(start))
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    pub fn task_path(&self, id: &TaskId) -> PathBuf {
        self.tasks_dir().join(format!("{id}.md"))
    }

    pub fn read_raw(&self, id: &TaskId) -> Result<String> {
        let path = self.task_path(id);
        if !path.is_file() {
            return Err(Error::TaskNotFound(id.to_string()));
        }
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn read_task(&self, id: &TaskId) -> Result<Task> {
        self.read_task_with_raw(id).map(|(task, _)| task)
    }

    pub fn read_task_with_raw(&self, id: &TaskId) -> Result<(Task, String)> {
        let raw = self.read_raw(id)?;
        let file = format!("tasks/{id}.md");
        let task = parse_task(&raw, &file)?;
        self.validate_docs(&task).map_err(|error| Error::Parse {
            file: file.clone(),
            detail: error.to_string(),
        })?;
        if &task.id != id {
            return Err(Error::Parse {
                file,
                detail: format!("id field is {}", task.id),
            });
        }
        if task.id.prefix != self.prefix {
            return Err(Error::Parse {
                file: format!("tasks/{id}.md"),
                detail: format!(
                    "prefix {:?} does not match project prefix {:?}",
                    task.id.prefix, self.prefix
                ),
            });
        }
        Ok((task, raw))
    }

    fn task_files(&self) -> Result<Vec<TaskId>> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(self.tasks_dir())? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(stem) = name.strip_suffix(".md") else {
                continue;
            };
            if stem.starts_with('.') {
                continue;
            }
            ids.push(TaskId::parse(stem).map_err(|_| Error::Parse {
                file: format!("tasks/{name}"),
                detail: "filename is not a valid task id".into(),
            })?);
        }
        ids.sort();
        Ok(ids)
    }

    pub fn scan(&self) -> Result<Vec<Task>> {
        self.task_files()?
            .into_iter()
            .map(|id| self.read_task(&id))
            .collect()
    }

    pub fn scan_lenient(&self) -> (Vec<Task>, Vec<Error>) {
        let mut tasks = Vec::new();
        let mut errors = Vec::new();
        let entries = match std::fs::read_dir(self.tasks_dir()) {
            Ok(entries) => entries,
            Err(error) => return (tasks, vec![error.into()]),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    errors.push(error.into());
                    continue;
                }
            };
            let path = entry.path();
            let Some(name) = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let Some(stem) = name.strip_suffix(".md") else {
                continue;
            };
            if stem.starts_with('.') {
                continue;
            }
            match TaskId::parse(stem) {
                Ok(id) => match self.read_task(&id) {
                    Ok(task) => tasks.push(task),
                    Err(error) => errors.push(error),
                },
                Err(_) => errors.push(Error::Parse {
                    file: format!("tasks/{name}"),
                    detail: "filename is not a valid task id".into(),
                }),
            }
        }
        tasks.sort_by(|left, right| left.id.cmp(&right.id));
        (tasks, errors)
    }

    pub fn write_task(&self, registry: &Registry, task: &Task) -> Result<()> {
        crate::hierarchy::validate_parent(self, registry, task)?;
        crate::hierarchy::validate_lanes(self, registry, task)?;
        crate::hierarchy::validate_periodic(self, registry, task)?;
        crate::hierarchy::validate_defer(self, registry, task)?;
        atomic_write(&self.task_path(&task.id), serialize_task(task).as_bytes())
    }

    fn git(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        std::process::Command::new("git")
            .args(args)
            .env("LC_ALL", "C")
            .current_dir(&self.root)
            .output()
    }

    /// git's own repository discovery from this project's root, canonicalized. `None` in
    /// exactly two documented cases: git itself says the root is not inside a repository,
    /// or there is no git executable. Any other git failure (permissions, a corrupt index,
    /// an unexpected exit) is an `io` error, not a skip.
    ///
    /// A repository whose HEAD is unreadable is reported by git as "not a git repository"
    /// and is treated the same way, since distinguishing the two would mean reimplementing
    /// discovery. `LC_ALL=C` pins git's messages to English so that the one string match
    /// is stable.
    fn git_toplevel(&self) -> Result<Option<PathBuf>> {
        match self.git(&["rev-parse", "--show-toplevel"]) {
            Ok(output) if output.status.success() => {
                let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim_end());
                Ok(Some(path.canonicalize().unwrap_or(path)))
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("not a git repository") {
                    return Ok(None);
                }
                Err(Error::Io(format!(
                    "git rev-parse in {} failed ({}): {}",
                    self.root.display(),
                    output.status,
                    stderr.trim()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn worktree_count(&self) -> Result<Option<usize>> {
        if self.git_toplevel()?.is_none() {
            return Ok(None);
        }
        let output = self.git(&["worktree", "list", "--porcelain", "-z"])?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git worktree list in {} failed: {}",
                self.root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(Some(
            output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|field| field.starts_with(b"worktree "))
                .count(),
        ))
    }

    /// This project's root in every worktree of the repository, in git's order, so the
    /// main worktree comes first; this checkout is included. `None` in the two cases
    /// `git_toplevel` documents.
    fn worktree_roots(&self) -> Result<Option<Vec<PathBuf>>> {
        let Some(toplevel) = self.git_toplevel()? else {
            return Ok(None);
        };
        // A nested project sits at the same offset below the top level in every worktree.
        let Ok(offset) = self.root.strip_prefix(&toplevel) else {
            return Err(Error::Io(format!(
                "git reports {} as the top level, which does not contain {}",
                toplevel.display(),
                self.root.display()
            )));
        };
        let listed = self.git(&["worktree", "list", "--porcelain", "-z"])?;
        if !listed.status.success() {
            return Err(Error::Io(format!(
                "git worktree list in {} failed ({}): {}",
                self.root.display(),
                listed.status,
                String::from_utf8_lossy(&listed.stderr).trim()
            )));
        }
        // porcelain with -z: NUL-terminated fields, paths verbatim (no quoting of spaces or
        // non-ASCII). Every record opens with a `worktree <path>` field; the rest describe
        // the checkout's head and are not ours to read.
        let stdout = String::from_utf8_lossy(&listed.stdout);
        let roots = stdout
            .split('\0')
            .filter_map(|field| field.strip_prefix("worktree "))
            .map(|listed| {
                let listed = PathBuf::from(listed);
                // A worktree git still lists but that is no longer on disk canonicalizes
                // to nothing; leave the path as given and let a read below it find nothing.
                let worktree = listed.canonicalize().unwrap_or(listed);
                // `join` on an empty offset would leave a trailing separator on the path
                // that goes into every warning.
                if offset.as_os_str().is_empty() {
                    worktree
                } else {
                    worktree.join(offset)
                }
            })
            .collect();
        Ok(Some(roots))
    }

    /// This project's root in the repository's main worktree, when this checkout is a
    /// linked worktree of it. `None` in the main worktree itself and in the two cases
    /// `git_toplevel` documents.
    pub fn main_checkout_root(&self) -> Result<Option<PathBuf>> {
        let Some(roots) = self.worktree_roots()? else {
            return Ok(None);
        };
        Ok(roots.into_iter().next().filter(|main| *main != self.root))
    }

    /// This record as it stands in every *other* worktree of the repository. `None` in the
    /// two cases `git_toplevel` documents.
    ///
    /// A worktree holding no copy of the record is absent from the result: it branched
    /// before the task existed and has nothing to say. A worktree whose copy cannot be read
    /// or parsed is reported rather than dropped, because a copy we cannot compare is not a
    /// copy we know to agree.
    pub fn sibling_task_copies(&self, id: &TaskId) -> Result<Option<Vec<SiblingCopy>>> {
        let Some(roots) = self.worktree_roots()? else {
            return Ok(None);
        };
        let file = format!("tasks/{id}.md");
        let mut copies = Vec::new();
        for root in roots {
            if root == self.root {
                continue;
            }
            let raw = match std::fs::read_to_string(root.join(&file)) {
                Ok(raw) => raw,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    copies.push(SiblingCopy::Unreadable {
                        root,
                        detail: error.to_string(),
                    });
                    continue;
                }
            };
            match parse_task(&raw, &file) {
                Ok(task) => copies.push(SiblingCopy::Found {
                    root,
                    updated: task.updated,
                    raw,
                }),
                Err(error) => copies.push(SiblingCopy::Unreadable {
                    root,
                    detail: error.to_string(),
                }),
            }
        }
        Ok(Some(copies))
    }

    /// Project-relative paths of changed or untracked files under tasks/, or `None` in the
    /// two cases `git_toplevel` documents.
    pub fn uncommitted_task_files(&self) -> Result<Option<Vec<String>>> {
        let Some(toplevel) = self.git_toplevel()? else {
            return Ok(None);
        };
        let output = self.git(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            "tasks/",
        ])?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git status in {} failed ({}): {}",
                self.root.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        // porcelain v1 with -z: NUL-terminated records, paths verbatim (no quoting of
        // spaces or non-ASCII), and a rename or copy record carries the new path in the
        // record and the old path as the next NUL-terminated field. Paths are relative to
        // the repository top level, which is not the project root for a nested project.
        let mut files = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut fields = stdout.split('\0').filter(|field| !field.is_empty());
        while let Some(record) = fields.next() {
            let (status, repo_relative) = record.split_at(3.min(record.len()));
            if status.starts_with(['R', 'C']) {
                fields.next(); // the source path of the rename or copy
            }
            let absolute = toplevel.join(repo_relative); // bound first: strip_prefix borrows it
            let Ok(relative) = absolute.strip_prefix(&self.root) else {
                return Err(Error::Io(format!(
                    "git status reported {repo_relative:?}, which is not under {}",
                    self.root.display()
                )));
            };
            let name = relative
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.ends_with(".tmp") || name.ends_with(".edit.md") {
                continue; // transient files of an in-flight write or edit
            }
            files.push(relative.display().to_string());
        }
        Ok(Some(files))
    }

    /// Assigns a fresh id and links the file into place with an exclusive operation, so a
    /// concurrent creator that drew the same id can never be overwritten: on a collision
    /// the id is regenerated. The temp file lives under tasks/ like every other write.
    pub fn create_task(&self, registry: &Registry, task: &mut Task) -> Result<()> {
        self.create_task_with(registry, task, || fastrand::u32(..0x100_0000))
    }

    fn create_task_with(
        &self,
        registry: &Registry,
        task: &mut Task,
        mut candidate: impl FnMut() -> u32,
    ) -> Result<()> {
        crate::hierarchy::validate_parent(self, registry, task)?;
        crate::hierarchy::validate_lanes(self, registry, task)?;
        // A new record has no children, so these refuse only a lane.
        crate::hierarchy::validate_periodic(self, registry, task)?;
        crate::hierarchy::validate_defer(self, registry, task)?;
        for _ in 0..16 {
            task.id = TaskId {
                prefix: self.prefix.clone(),
                hex: format!("{:06x}", candidate()),
            };
            let path = self.task_path(&task.id);
            if path.exists() {
                continue;
            }
            let temp = path.with_file_name(format!(".{}.new.tmp", task.id));
            let mut file = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            if let Err(error) = file.write_all(serialize_task(task).as_bytes()) {
                drop(file);
                let _ = std::fs::remove_file(&temp);
                return Err(error.into());
            }
            drop(file);
            let linked = std::fs::hard_link(&temp, &path);
            let _ = std::fs::remove_file(&temp);
            match linked {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(Error::Validation(
            "could not allocate a free id after 16 attempts".into(),
        ))
    }

    pub fn new_id(&self) -> Result<TaskId> {
        for _ in 0..16 {
            let id = TaskId {
                prefix: self.prefix.clone(),
                hex: format!("{:06x}", fastrand::u32(..0x100_0000)),
            };
            if !self.task_path(&id).exists() {
                return Ok(id);
            }
        }
        Err(Error::Validation(
            "could not allocate a free id after 16 attempts".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn temp_project() -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::init(dir.path(), "tst").unwrap();
        (dir, p)
    }

    fn sample(p: &Project) -> Task {
        Task {
            id: p.new_id().unwrap(),
            title: "T".into(),
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            lane: false,
            needs: vec![],
            every: None,
            defer: None,
            owner: None,
            created: crate::time::now(),
            updated: crate::time::now(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn tag_dictionary_is_optional_and_its_entries_are_validated() {
        let (dir, p) = temp_project();
        assert_eq!(p.tags, None, "init writes no dictionary");
        let config = dir.path().join("tasks/.config.toml");
        std::fs::write(
            &config,
            "prefix = \"tst\"\n\n[tags]\ntesting = \"Tests, gates, and CI.\"\nperf = \"Speed or memory.\"\n",
        )
        .unwrap();
        let p = Project::open(dir.path()).unwrap();
        let dictionary = p.tags.unwrap();
        assert_eq!(dictionary.len(), 2);
        assert_eq!(dictionary["testing"], "Tests, gates, and CI.");

        std::fs::write(&config, "prefix = \"tst\"\n\n[tags]\ntesting = \"\"\n").unwrap();
        let error = Project::open(dir.path()).unwrap_err().to_string();
        assert!(error.contains("[tags] \"testing\""), "{error}");
        assert!(error.contains("must not be empty"), "{error}");
    }

    #[test]
    fn need_vocabulary_is_optional_and_its_entries_are_validated() {
        let (dir, p) = temp_project();
        assert!(p.needs.is_empty(), "init declares no needs");
        let config = dir.path().join("tasks/.config.toml");
        std::fs::write(
            &config,
            "prefix = \"tst\"\n\n[needs.quiet]\nmeaning = \"an idle host\"\nexclusive = true\n\n\
             [needs.owner]\nmeaning = \"the owner judges an image\"\n",
        )
        .unwrap();
        let p = Project::open(dir.path()).unwrap();
        assert_eq!(p.needs.len(), 2);
        assert!(p.needs["quiet"].exclusive);
        assert!(!p.needs["owner"].exclusive, "exclusive defaults to false");
        assert_eq!(p.needs["owner"].meaning, "the owner judges an image");

        // Each malformed table is a config error; ours name the entry.
        for (table, fragment) in [
            ("[needs.quiet]\n", None),
            ("[needs.quiet]\nmeaning = \"x\"\ncapacity = 2\n", None),
            (
                "[needs.quiet]\nmeaning = \"x\"\nexclusive = \"yes\"\n",
                None,
            ),
            ("[needs]\nquiet = \"an idle host\"\n", None),
            (
                "[needs.quiet]\nmeaning = \"\"\n",
                Some("[needs] \"quiet\": meaning must not be empty"),
            ),
            (
                "[needs.Quiet]\nmeaning = \"x\"\n",
                Some("[needs] \"Quiet\": need \"Quiet\""),
            ),
        ] {
            std::fs::write(&config, format!("prefix = \"tst\"\n\n{table}")).unwrap();
            let error = Project::open(dir.path()).unwrap_err();
            assert_eq!(error.kind(), "config", "{table}");
            if let Some(fragment) = fragment {
                assert!(error.to_string().contains(fragment), "{table}: {error}");
            }
        }
    }

    #[test]
    fn init_creates_layout_is_idempotent_and_refuses_other_prefix() {
        let (dir, p) = temp_project();
        assert!(dir.path().join("tasks/.config.toml").is_file());
        assert!(dir.path().join("docs/specs").is_dir());
        assert!(dir.path().join("docs/plans").is_dir());
        assert_eq!(p.prefix, "tst");
        std::fs::remove_dir(dir.path().join("docs/plans")).unwrap();
        assert!(
            Project::init(dir.path(), "tst").is_ok(),
            "rerun with the same prefix recovers"
        );
        assert!(dir.path().join("docs/plans").is_dir());
        assert!(Project::init(dir.path(), "oth").is_err());
        assert!(Project::init(tempfile::tempdir().unwrap().path(), "Bad").is_err());
    }

    #[test]
    fn locate_walks_up() {
        let (dir, _) = temp_project();
        let deep = dir.path().join("src/a/b");
        std::fs::create_dir_all(&deep).unwrap();
        let p = Project::locate(&deep).unwrap();
        assert_eq!(p.root, dir.path().canonicalize().unwrap());
        assert!(Project::locate(tempfile::tempdir().unwrap().path()).is_err());
    }

    #[test]
    fn write_scan_read_roundtrip() {
        let (_dir, p) = temp_project();
        let registry = Registry::default();
        let t = sample(&p);
        p.write_task(&registry, &t).unwrap();
        assert_eq!(p.read_task(&t.id).unwrap(), t);
        let (from_raw, raw) = p.read_task_with_raw(&t.id).unwrap();
        assert_eq!(from_raw, t);
        assert_eq!(parse_task(&raw, "test").unwrap(), t);
        assert_eq!(p.scan().unwrap(), vec![t.clone()]);
        assert!(std::fs::read_dir(p.tasks_dir()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn write_task_refuses_a_bad_parent_on_any_write() {
        let (_dir, p) = temp_project();
        let registry = Registry::default();
        let mut t = sample(&p);
        t.parent = Some(TaskId {
            prefix: "tst".into(),
            hex: "ffffff".into(),
        });
        assert!(matches!(
            p.write_task(&registry, &t),
            Err(Error::UnresolvableId(_))
        ));
        t.parent = Some(t.id.clone());
        assert!(matches!(p.write_task(&registry, &t), Err(Error::Cycle(_))));
        t.parent = None;
        p.write_task(&registry, &t).unwrap();
    }

    #[test]
    fn write_task_survives_a_missing_ancestor_further_up_the_chain() {
        let (_dir, p) = temp_project();
        let registry = Registry::default();
        let grandparent = sample(&p);
        p.write_task(&registry, &grandparent).unwrap();
        let mut parent = sample(&p);
        parent.parent = Some(grandparent.id.clone());
        p.write_task(&registry, &parent).unwrap();
        let mut child = sample(&p);
        child.parent = Some(parent.id.clone());
        p.write_task(&registry, &child).unwrap();

        std::fs::remove_file(p.task_path(&grandparent.id)).unwrap();

        child.title = "unrelated change".into();
        p.write_task(&registry, &child).unwrap();
    }

    #[test]
    fn create_task_takes_the_next_free_id_and_leaves_no_temp() {
        let (_dir, p) = temp_project();
        let registry = Registry::default();
        let mut first = sample(&p);
        first.id = TaskId {
            prefix: "tst".into(),
            hex: "000001".into(),
        };
        first.title = "first".into();
        p.write_task(&registry, &first).unwrap();
        let mut second = sample(&p);
        second.title = "second".into();
        let mut candidates = [1, 2].into_iter();
        p.create_task_with(&registry, &mut second, || candidates.next().unwrap())
            .unwrap();
        assert_eq!(second.id.hex, "000002");
        assert_eq!(p.read_task(&first.id).unwrap().title, "first");
        assert_eq!(p.read_task(&second.id).unwrap().title, "second");
        assert!(std::fs::read_dir(p.tasks_dir()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn new_id_uses_prefix_and_avoids_existing() {
        let (_dir, p) = temp_project();
        let id = p.new_id().unwrap();
        assert_eq!(id.prefix, "tst");
        assert_eq!(id.hex.len(), 6);
    }

    #[test]
    fn rejects_task_with_foreign_prefix_in_this_project() {
        let (_dir, p) = temp_project();
        let registry = Registry::default();
        let mut t = sample(&p);
        t.id = TaskId {
            prefix: "oth".into(),
            hex: "000001".into(),
        };
        p.write_task(&registry, &t).unwrap();
        assert!(p.read_task(&t.id).is_err());
        assert!(p.scan().is_err());
    }

    #[test]
    fn scan_fails_on_bad_file_but_lenient_reports() {
        let (_dir, p) = temp_project();
        std::fs::write(p.tasks_dir().join("tst-000000.md"), "garbage").unwrap();
        assert!(p.scan().is_err());
        let (ok, errs) = p.scan_lenient();
        assert!(ok.is_empty());
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn atomic_write_retries_without_following_a_symlink_collision() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let victim = dir.path().join("victim");
        std::fs::write(&target, "old").unwrap();
        std::fs::write(&victim, "untouched").unwrap();
        let collision = dir.path().join(".target.000001.tmp");
        std::os::unix::fs::symlink(&victim, &collision).unwrap();
        let mut candidates = [1, 2].into_iter();

        atomic_write_with(&target, b"new", || candidates.next().unwrap()).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        assert_eq!(std::fs::read_link(&collision).unwrap(), victim);
        assert!(!dir.path().join(".target.000002.tmp").exists());
    }
}
