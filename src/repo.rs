use crate::error::{Error, Result};
use crate::format::{parse_task, serialize_task, validate_doc_path};
use crate::model::{Task, TaskId, is_valid_prefix};
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
pub const DEFAULT_PLAN_DIRS: &[&str] = &["docs/plans"];

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub prefix: String,
    /// Roots a `spec` link may live under; also the search path for bare spec names.
    pub spec_dirs: Vec<String>,
    /// Roots a `plan` link may live under; also the search path for bare plan names.
    pub plan_dirs: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Config {
    prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spec_dirs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan_dirs: Option<Vec<String>>,
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
                spec_dirs: None,
                plan_dirs: None,
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
            spec_dirs: doc_roots("spec_dirs", config.spec_dirs, DEFAULT_SPEC_DIRS)?,
            plan_dirs: doc_roots("plan_dirs", config.plan_dirs, DEFAULT_PLAN_DIRS)?,
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

    pub fn write_task(&self, task: &Task) -> Result<()> {
        crate::hierarchy::validate_parent(self, task)?;
        atomic_write(&self.task_path(&task.id), serialize_task(task).as_bytes())
    }

    /// Assigns a fresh id and links the file into place with an exclusive operation, so a
    /// concurrent creator that drew the same id can never be overwritten: on a collision
    /// the id is regenerated. The temp file lives under tasks/ like every other write.
    pub fn create_task(&self, task: &mut Task) -> Result<()> {
        self.create_task_with(task, || fastrand::u32(..0x100_0000))
    }

    fn create_task_with(&self, task: &mut Task, mut candidate: impl FnMut() -> u32) -> Result<()> {
        crate::hierarchy::validate_parent(self, task)?;
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
            owner: None,
            created: crate::time::now(),
            updated: crate::time::now(),
            depends: vec![],
            parent: None,
            tags: vec![],
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
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
        let t = sample(&p);
        p.write_task(&t).unwrap();
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
        let mut t = sample(&p);
        t.parent = Some(TaskId {
            prefix: "tst".into(),
            hex: "ffffff".into(),
        });
        assert!(matches!(p.write_task(&t), Err(Error::UnresolvableId(_))));
        t.parent = Some(t.id.clone());
        assert!(matches!(p.write_task(&t), Err(Error::Cycle(_))));
        t.parent = None;
        p.write_task(&t).unwrap();
    }

    #[test]
    fn write_task_survives_a_missing_ancestor_further_up_the_chain() {
        let (_dir, p) = temp_project();
        let grandparent = sample(&p);
        p.write_task(&grandparent).unwrap();
        let mut parent = sample(&p);
        parent.parent = Some(grandparent.id.clone());
        p.write_task(&parent).unwrap();
        let mut child = sample(&p);
        child.parent = Some(parent.id.clone());
        p.write_task(&child).unwrap();

        std::fs::remove_file(p.task_path(&grandparent.id)).unwrap();

        child.title = "unrelated change".into();
        p.write_task(&child).unwrap();
    }

    #[test]
    fn create_task_takes_the_next_free_id_and_leaves_no_temp() {
        let (_dir, p) = temp_project();
        let mut first = sample(&p);
        first.id = TaskId {
            prefix: "tst".into(),
            hex: "000001".into(),
        };
        first.title = "first".into();
        p.write_task(&first).unwrap();
        let mut second = sample(&p);
        second.title = "second".into();
        let mut candidates = [1, 2].into_iter();
        p.create_task_with(&mut second, || candidates.next().unwrap())
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
        let mut t = sample(&p);
        t.id = TaskId {
            prefix: "oth".into(),
            hex: "000001".into(),
        };
        p.write_task(&t).unwrap();
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
