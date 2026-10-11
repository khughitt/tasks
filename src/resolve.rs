use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Doc {
    pub reference: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub enum UnavailableCause {
    MissingScheme { scheme: String, config: PathBuf },
    FetchFailed(String),
    BudgetExhausted,
}

impl UnavailableCause {
    pub fn detail(&self) -> String {
        match self {
            Self::MissingScheme { scheme, config } => format!(
                "{}: [docs.schemes] has no {scheme}; a record may reference it, but this host cannot resolve it",
                config.display()
            ),
            Self::FetchFailed(detail) => detail.clone(),
            Self::BudgetExhausted => "budget exhausted".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DocRead {
    Available(Doc),
    Unavailable {
        reference: String,
        cause: UnavailableCause,
    },
}

impl DocRead {
    pub fn required(self) -> Result<Doc> {
        match self {
            Self::Available(doc) => Ok(doc),
            Self::Unavailable { reference, cause } => {
                let detail = format!("{reference}: {}", cause.detail());
                Err(match cause {
                    UnavailableCause::MissingScheme { .. } => Error::Config(detail),
                    UnavailableCause::FetchFailed(_) | UnavailableCause::BudgetExhausted => {
                        Error::DocNotFound(detail)
                    }
                })
            }
        }
    }
}

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
    host: OnceCell<crate::config::HostConfig>,
    docs: RefCell<HashMap<String, DocRead>>,
    fetch_budget: Option<Duration>,
    fetch_deadline: OnceCell<Instant>,
}

impl<'a> Resolver<'a> {
    pub fn new(project: &'a Project, registry: &'a Registry) -> Resolver<'a> {
        Resolver {
            project,
            registry,
            main: OnceCell::new(),
            from_main: RefCell::new(Vec::new()),
            host: OnceCell::new(),
            docs: RefCell::new(HashMap::new()),
            fetch_budget: None,
            fetch_deadline: OnceCell::new(),
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
        if crate::format::external_doc(name_or_path)?.is_some() {
            return Ok(self.read(kind, name_or_path)?.required()?.reference);
        }
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
        Ok(has_heading(
            &self.read(DocKind::Plan, plan_rel)?.required()?.body,
            step,
        ))
    }

    pub fn read(&self, kind: DocKind, value: &str) -> Result<DocRead> {
        let Some((scheme, rest)) = crate::format::external_doc(value)? else {
            let path = self
                .find(kind, value)?
                .unwrap_or_else(|| self.project.root.join(value));
            return Ok(DocRead::Available(Doc {
                reference: value.into(),
                body: std::fs::read_to_string(path)?,
            }));
        };
        if let Some(cached) = self.docs.borrow().get(value) {
            return Ok(cached.clone());
        }
        let host = self.host_config()?;
        let result = match host.docs_schemes.get(scheme) {
            None => self.missing_scheme(value, scheme)?,
            Some(command) => {
                let (deadline, budget_limited) = self.fetch_limit();
                crate::doc_fetch::fetch(
                    crate::doc_fetch::FetchRequest {
                        reference: value.into(),
                        command: command.clone(),
                        rest: rest.into(),
                        budget_limited,
                    },
                    deadline,
                )
            }
        };
        self.cache_doc(value, result.clone());
        Ok(result)
    }

    fn host_config(&self) -> Result<&crate::config::HostConfig> {
        if self.host.get().is_none() {
            let loaded = crate::config::HostConfig::load()?;
            self.host
                .set(loaded)
                .expect("host config is initialized once");
        }
        Ok(self.host.get().expect("host config was initialized"))
    }

    fn missing_scheme(&self, value: &str, scheme: &str) -> Result<DocRead> {
        Ok(DocRead::Unavailable {
            reference: value.into(),
            cause: UnavailableCause::MissingScheme {
                scheme: scheme.into(),
                config: crate::config::HostConfig::path()?,
            },
        })
    }

    fn cache_doc(&self, value: &str, result: DocRead) {
        let mut cache = self.docs.borrow_mut();
        if let DocRead::Available(doc) = &result {
            cache.insert(doc.reference.clone(), result.clone());
        }
        cache.insert(value.into(), result);
    }

    pub fn set_fetch_budget(&mut self, budget: Duration) {
        self.fetch_budget = Some(budget);
    }

    fn fetch_limit(&self) -> (Instant, bool) {
        let now = Instant::now();
        let strict = now + crate::doc_fetch::STRICT_TIMEOUT;
        match self.fetch_budget {
            None => (strict, false),
            Some(budget) => {
                let shared = *self.fetch_deadline.get_or_init(|| now + budget);
                (shared.min(strict), shared <= strict)
            }
        }
    }

    pub fn prefetch_external(&self, references: &[String]) -> Result<Vec<String>> {
        if references.is_empty() {
            return Ok(vec![]);
        }
        let host = self.host_config()?;
        let mut requests = Vec::new();
        for reference in references {
            if self.docs.borrow().contains_key(reference) {
                continue;
            }
            let (scheme, rest) =
                crate::format::external_doc(reference)?.expect("prefetch references are external");
            match host.docs_schemes.get(scheme) {
                None => self.cache_doc(reference, self.missing_scheme(reference, scheme)?),
                Some(command) => requests.push(crate::doc_fetch::FetchRequest {
                    reference: reference.clone(),
                    command: command.clone(),
                    rest: rest.into(),
                    budget_limited: false,
                }),
            }
        }
        if !requests.is_empty() {
            std::thread::scope(|scope| -> Result<()> {
                let (sender, receiver) = std::sync::mpsc::channel();
                let mut next = 0;
                let mut active = 0;
                loop {
                    while active < 4 && next < requests.len() {
                        let (deadline, budget_limited) = self.fetch_limit();
                        if Instant::now() >= deadline {
                            break;
                        }
                        let mut request = requests[next].clone();
                        request.budget_limited = budget_limited;
                        next += 1;
                        let reference = request.reference.clone();
                        let sent = sender.clone();
                        match std::thread::Builder::new().spawn_scoped(scope, move || {
                            let result = crate::doc_fetch::fetch(request, deadline);
                            let _ = sent.send((reference, result));
                        }) {
                            Ok(_) => active += 1,
                            Err(error) => self.cache_doc(
                                &requests[next - 1].reference,
                                DocRead::Unavailable {
                                    reference: requests[next - 1].reference.clone(),
                                    cause: UnavailableCause::FetchFailed(format!(
                                        "failed to start fetch worker: {error}"
                                    )),
                                },
                            ),
                        }
                    }
                    if active == 0 {
                        break;
                    }
                    let (reference, result) = receiver.recv().map_err(|error| {
                        Error::Io(format!("fetch worker disconnected: {error}"))
                    })?;
                    active -= 1;
                    self.cache_doc(&reference, result);
                }
                for request in &requests[next..] {
                    self.cache_doc(
                        &request.reference,
                        DocRead::Unavailable {
                            reference: request.reference.clone(),
                            cause: UnavailableCause::BudgetExhausted,
                        },
                    );
                }
                Ok(())
            })?;
        }
        Ok(references
            .iter()
            .filter(|reference| {
                matches!(
                    self.docs.borrow().get(*reference),
                    Some(DocRead::Unavailable {
                        cause: UnavailableCause::BudgetExhausted,
                        ..
                    })
                )
            })
            .cloned()
            .collect())
    }

    /// Where `rel` is read from; this checkout's path when it is in neither.
    pub fn abs(&self, kind: DocKind, rel: &str) -> Result<String> {
        if crate::format::external_doc(rel)?.is_some() {
            return Ok(rel.into());
        }
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
    use std::time::Duration;

    fn prefetch_fixture() -> (tempfile::TempDir, Project, Registry, String) {
        let temp = tempfile::tempdir().unwrap();
        let project = Project::init(temp.path(), "xx", None).unwrap();
        std::fs::create_dir(temp.path().join("started")).unwrap();
        std::fs::create_dir(temp.path().join("released")).unwrap();
        let script = temp.path().join("fetch.sh");
        std::fs::write(&script, format!("printf '%s' \"$$\" > '{0}/started/'\"$1\"\nwhile [ ! -f '{0}/released/'\"$1\" ]; do sleep 0.01; done\nprintf '{{\"id\":\"%s\",\"body\":\"ok\"}}' \"$1\"\n", temp.path().display())).unwrap();
        let command = format!("sh '{}'", script.display());
        (temp, project, Registry::default(), command)
    }

    #[test]
    fn prefetch_external_budget_is_shared_and_unstarted_refs_are_cached() {
        let (temp, project, registry, command) = prefetch_fixture();
        let mut resolver = Resolver::new(&project, &registry);
        resolver
            .host
            .set(crate::config::HostConfig {
                relay_identity: false,
                docs_schemes: [("stub".into(), command)].into(),
            })
            .unwrap();
        resolver.set_fetch_budget(Duration::from_millis(150));
        let refs: Vec<String> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|name| format!("stub:{name}"))
            .collect();
        let started = Instant::now();
        assert_eq!(resolver.prefetch_external(&refs).unwrap(), refs);
        assert!(started.elapsed() < Duration::from_secs(2));
        for reference in &refs {
            assert!(matches!(
                resolver.read(DocKind::Plan, reference).unwrap(),
                DocRead::Unavailable {
                    cause: UnavailableCause::BudgetExhausted,
                    ..
                }
            ));
        }
        let files: Vec<_> = std::fs::read_dir(temp.path().join("started"))
            .unwrap()
            .collect();
        assert_eq!(files.len(), 4);
        for entry in files {
            let pid = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            let state = std::fs::read_to_string(format!("/proc/{pid}/stat"));
            assert!(state.is_err() || state.unwrap().rsplit_once(") ").unwrap().1.starts_with('Z'));
        }
    }

    #[test]
    fn prefetch_external_starts_at_most_four_then_refills() {
        let (temp, project, registry, command) = prefetch_fixture();
        let root = temp.path().to_owned();
        let refs: Vec<String> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|name| format!("stub:{name}"))
            .collect();
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || {
                let mut resolver = Resolver::new(&project, &registry);
                resolver
                    .host
                    .set(crate::config::HostConfig {
                        relay_identity: false,
                        docs_schemes: [("stub".into(), command)].into(),
                    })
                    .unwrap();
                resolver.set_fetch_budget(Duration::from_secs(3));
                assert!(resolver.prefetch_external(&refs).unwrap().is_empty());
                for reference in refs {
                    assert_eq!(
                        resolver
                            .read(DocKind::Plan, &reference)
                            .unwrap()
                            .required()
                            .unwrap()
                            .body,
                        "ok"
                    );
                }
            });
            let deadline = Instant::now() + Duration::from_secs(2);
            while std::fs::read_dir(root.join("started")).unwrap().count() < 4 {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(std::fs::read_dir(root.join("started")).unwrap().count(), 4);
            std::fs::write(root.join("released/a"), "").unwrap();
            while !root.join("started/e").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!root.join("started/f").exists());
            for name in ["b", "c", "d", "e", "f"] {
                std::fs::write(root.join("released").join(name), "").unwrap();
            }
            worker.join().unwrap();
        });
    }

    #[test]
    fn prefetch_external_missing_scheme_is_not_budget_exhaustion() {
        let (_, project, registry, _) = prefetch_fixture();
        let mut resolver = Resolver::new(&project, &registry);
        resolver
            .host
            .set(crate::config::HostConfig::default())
            .unwrap();
        resolver.set_fetch_budget(Duration::ZERO);
        assert!(
            resolver
                .prefetch_external(&["absent:x".into()])
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            resolver.read(DocKind::Plan, "absent:x").unwrap(),
            DocRead::Unavailable {
                cause: UnavailableCause::MissingScheme { .. },
                ..
            }
        ));
    }

    #[test]
    fn step_headings_match_only_the_task_n_convention() {
        let text = "# P\n### Task 1: one\n## Task 12: twelve\n### Notes on Task 3\n### Task x: no\nTask 4: not a heading\n";
        assert_eq!(step_headings(text), ["Task 1: one", "Task 12: twelve"]);
    }
}
