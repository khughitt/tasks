# Multi-Project Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Registry-wide read commands (`list`, `ready`, `prime`, `tree`, and the new `next`, `tags`, `projects`, `root`) that work from any directory, plus `add --project <prefix>` with `feedback` built on the same code path.

**Architecture:** One new module, `src/scope.rs`, holds the shared prefix resolver (`open_registered`) and the `Scope` type (one local project, or every reachable registered project). Read commands that accept `--all-projects` take a `ReadCtx` wrapping a `Scope`; write commands keep the existing `Ctx` with a mandatory local project. `add --project` is a dispatch change: the target project is opened by prefix and handed to the unchanged `add` as its `Ctx`. `show` is split into a reusable `describe` so `next` can print the show shape.

**Tech Stack:** Rust 2024 edition, clap derive, serde. Tests: unit tests in-module plus end-to-end tests in `tests/cli.rs` with several projects registered in one temporary home (`TestEnv`).

**Spec:** `docs/specs/2026-09-04-multi-project-design.md` (read it first; each task cites its sections).

## Global Constraints

- Gates before every task commit: `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`. The final tracker-only goal close reruns only `tasks check`, since code and docs are unchanged.
- Rebuild and reinstall after CLI changes: `cargo install --path .`
- JSON output is the contract. Additive only, except `prime.prefix` becoming `string | null` (null only under `--all-projects`). New shapes, all fields always present:
  - `next -> { next: ShowFields | null, warnings }` where `ShowFields` is the `show` object without `warnings`
  - `root -> { prefix, root, warnings }`
  - `tags -> { tags: [{ tag, count, projects: { <prefix>: count } }], warnings }`; a count is the number of tasks carrying the tag, once each
  - `projects -> { projects: [{ prefix, root, reachable, counts: Counts | null }], warnings }`
  - `prime += projects: [prefix]`
- Registry-wide scope never locates a local project (spec §3.2). `--all-projects` is defined only on `list`, `ready`, `prime`, `tree`, `next`, `tags`.
- An explicit `add --project <prefix>` always targets that prefix's registered root. It does not locate or prefer a current checkout carrying the same prefix.
- Reachability outcomes (spec §3.2): missing root or config warns and skips; malformed config or prefix mismatch is a `config` error.
- Prefix resolution (spec §3.1): an unregistered or config-less prefix is `unresolvable_id` when reached through an id and `config` when typed as a prefix; a mismatch is always `config`.
- No new error kinds. Fail early with a typed `Error`; no silent fallbacks.
- Each task below is one tracker task: `tasks start <id>` before touching code, `tasks done <id> "<what landed>"` in its commit, and `tasks check` after `done` so the mutation it commits is the one checked.
- Conventional commits, no AI-attribution trailers.

---

### Task 1: Shared prefix resolver in a new scope module

Tracker: `tasks-64319f`. Spec §3.1.

Before anything else: `tasks start tasks-64319f`.

**Files:**
- Create: `src/scope.rs`
- Modify: `src/main.rs` (module), `src/commands/show.rs:9-29`, `src/commands/feedback.rs:13-34`
- Test: `src/scope.rs` (unit), existing `tests/cli.rs` (no change; behavior preserved)

**Interfaces:**
- Consumes: `Registry::project_root(&self, prefix: &str) -> Option<&Path>`, `Project::open(root: &Path) -> Result<Project>`, `repo::CONFIG_REL`.
- Produces: `scope::Origin<'a> { Id(&'a TaskId), Prefix }`; `scope::has_config(root: &Path) -> Result<bool>`; `scope::open_registered(registry: &Registry, prefix: &str, origin: Origin) -> Result<Project>`.

- [ ] **Step 1: Write the failing unit tests**

Create `src/scope.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::path::Path;

/// How a prefix reached the resolver, which decides the error kind when it cannot be
/// opened (spec §3.1): an id that cannot be followed is `unresolvable_id`; a prefix a
/// person typed is `config`. A registered project whose config disagrees with the
/// registry is `config` either way.
#[derive(Clone, Copy)]
pub enum Origin<'a> {
    Id(&'a TaskId),
    Prefix,
}

/// The config file exists and is a file. `try_exists` so a permission error surfaces
/// instead of reading as absent; `is_file` so a directory at that path is not mistaken
/// for a config. Every reachability test in the tool goes through this one function.
pub fn has_config(root: &Path) -> Result<bool> {
    let config = root.join(CONFIG_REL);
    Ok(config.try_exists()? && config.is_file())
}

/// Opens the project registered as `prefix`, applying the three checks every
/// cross-project path shares: registered, config present, prefix agrees.
pub fn open_registered(registry: &Registry, prefix: &str, origin: Origin) -> Result<Project> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with(prefix: &str, root: &Path) -> Registry {
        let mut registry = Registry::default();
        registry.register(prefix, root).unwrap();
        registry
    }

    fn write_config(root: &Path, prefix: &str) {
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::write(root.join(CONFIG_REL), format!("prefix = \"{prefix}\"\n")).unwrap();
    }

    #[test]
    fn unregistered_prefix_kind_follows_origin() {
        let registry = Registry::default();
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::UnresolvableId(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn missing_config_kind_follows_origin() {
        let dir = tempfile::tempdir().unwrap();
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::UnresolvableId(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn prefix_mismatch_is_config_whatever_the_origin() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "fam");
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::Config(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn opens_a_matching_project() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "sci");
        let registry = registry_with("sci", dir.path());
        let project = open_registered(&registry, "sci", Origin::Prefix).unwrap();
        assert_eq!(project.prefix, "sci");
        assert_eq!(project.root, dir.path().canonicalize().unwrap());
    }
}
```

Add `mod scope;` to `src/main.rs` (alphabetically, after `mod resolve;`).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test scope::`
Expected: four failures panicking at `not yet implemented`.

- [ ] **Step 3: Implement `open_registered`**

Replace the `todo!()` body:

```rust
pub fn open_registered(registry: &Registry, prefix: &str, origin: Origin) -> Result<Project> {
    let fail = |detail: String| match origin {
        Origin::Id(id) => Error::UnresolvableId(format!("{id}: {detail}")),
        Origin::Prefix => Error::Config(detail),
    };
    let Some(root) = registry.project_root(prefix) else {
        return Err(fail(format!("no project registered as {prefix:?}")));
    };
    if !has_config(root)? {
        return Err(fail(format!(
            "project {prefix:?} at {} has no {CONFIG_REL}; run `tasks init` there",
            root.display()
        )));
    }
    let project = Project::open(root)?;
    if project.prefix != prefix {
        return Err(Error::Config(format!(
            "registry maps {prefix:?} to {}, whose prefix is {:?}; fix the registry",
            root.display(),
            project.prefix
        )));
    }
    Ok(project)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test scope::`
Expected: 4 passed.

- [ ] **Step 5: Use it in `show`**

In `src/commands/show.rs`, replace the foreign branch. The whole `let project: &Project = ...` block becomes:

```rust
    let foreign;
    let project: &Project = if id.prefix == ctx.project.prefix {
        &ctx.project
    } else {
        foreign = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
        &foreign
    };
```

Add `use crate::scope::Origin;`, change the error import to `use crate::error::Result;`, and change the repository import to `use crate::repo::Project;`.

- [ ] **Step 6: Use it in `feedback`**

In `src/commands/feedback.rs`, replace `locate_target`:

```rust
/// The upstream project: the registry entry whose prefix is `tasks`. The unregistered
/// case gets a hint the generic resolver cannot know: where the upstream lives.
pub fn locate_target(registry: &Registry) -> Result<Project> {
    if registry.project_root(TARGET_PREFIX).is_none() {
        return Err(Error::Config(format!(
            "no project registered as {TARGET_PREFIX:?}; clone the upstream tasks repository and run `tasks init` there"
        )));
    }
    crate::scope::open_registered(registry, TARGET_PREFIX, Origin::Prefix)
}
```

Add `use crate::scope::Origin;` and change the repository import to `use crate::repo::Project;` (`Error` remains in use for the target-specific hint).

- [ ] **Step 7: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test; the existing `show_resolves_a_foreign_id_read_only` and `feedback_fails_early_without_a_target_or_a_reporter` tests still pass because only messages changed, not kinds.

- [ ] **Step 8: Commit**

```bash
tasks done tasks-64319f "scope::open_registered shared by show and feedback"
tasks check
git add src/scope.rs src/main.rs src/commands/show.rs src/commands/feedback.rs tasks/
git commit -m "refactor: one resolver for registered prefixes"
```

---

### Task 2: Scope, ReadCtx, and `list --all-projects` outside a project

Tracker: `tasks-fe2041`. Spec §2 (one scope), §3.2, §3.3, §4.1 (`list`).

Before anything else: `tasks start tasks-fe2041`.

**Files:**
- Modify: `src/scope.rs`, `src/resolve.rs:31-55`, `src/commands/mod.rs` (ReadCtx, `open_read_ctx`, dispatch for `List`), `src/commands/list.rs:1-80`
- Test: `tests/cli.rs`, `src/scope.rs` (unit)

**Interfaces:**
- Consumes: `scope::has_config`, `scope::open_registered` (Task 1); `Project::scan`, `Project::locate`, `Registry::load`, `Registry.projects: BTreeMap<String, PathBuf>`.
- Produces:
  - `scope::is_reachable(root: &Path) -> Result<bool>`; `scope::registry_warnings(registry: &Registry, cwd: &Path) -> Result<Vec<String>>`.
  - `scope::Scope { Local(Project), All(Vec<Project>) }` with `projects(&self) -> &[Project]`, `prefixes(&self) -> Vec<String>`, `scan(&self) -> Result<Vec<Task>>`, `scan_each(&self) -> Result<Vec<(&Project, Vec<Task>)>>`, `resolve_task(&self, registry: &Registry, id: &TaskId) -> Result<Option<Task>>`, `Scope::open_all(registry: &Registry, cwd: &Path) -> Result<(Scope, Vec<String>)>`.
  - `resolve::resolve_registered(registry: &Registry, id: &TaskId) -> Result<Option<Task>>` and `resolve::read_present(project: &Project, id: &TaskId) -> Result<Option<Task>>` (the two halves of `Resolver::resolve_task`).
  - `commands::ReadCtx { scope: Scope, registry: Registry, warnings: Vec<String> }` with `resolve_task(&self, id: &TaskId) -> Result<Option<Task>>`; `commands::open_read_ctx(dir: Option<&Path>, all_projects: bool) -> Result<ReadCtx>`; `commands::start_dir(dir: Option<&Path>) -> Result<PathBuf>`.
  - `list::list(ctx: ReadCtx, statuses, tags, owner, parent) -> Result<Output>` (no `all_projects` parameter; the scope carries it). `list::ready_tasks(ctx: &mut ReadCtx, all: &[Task])`, `list::ready(ctx: ReadCtx, ...)`, `list::prime(ctx: ReadCtx)` take `ReadCtx` (behavior unchanged in this task).

- [ ] **Step 1: Write the failing e2e tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn all_projects_needs_no_local_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "S"]);
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["title"], "S");
    assert_eq!(v["warnings"], serde_json::json!([]));
}

#[test]
fn all_projects_warns_on_empty_registry_and_unregistered_current_project() {
    let env = TestEnv::new();
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!(["registry is empty"]));

    // a project that was initialised under another home is not in this registry
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lone = tempfile::tempdir().unwrap();
    let other = TestEnv::new();
    other.json(lone.path(), &["init", "--prefix", "lon"]);
    env.json(&sci, &["add", "S"]);
    let v = env.json(lone.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(
        v["warnings"],
        serde_json::json!(["current project lon is not registered"])
    );
}

#[test]
fn all_projects_rejects_a_prefix_mismatch() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::write(fam.join("tasks/.config.toml"), "prefix = \"zzz\"\n").unwrap();
    assert_eq!(env.fail(&sci, &["list", "--all-projects"]), "config");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli all_projects_`
Expected: `all_projects_needs_no_local_project` fails with kind `no_project`; the empty-registry test fails because the current `list` walks only from inside a project; the mismatch test fails because today the mismatched project is simply scanned.

- [ ] **Step 3: Split `Resolver::resolve_task` into reusable halves**

In `src/resolve.rs`, replace `resolve_task` and add two free functions:

```rust
    /// `Ok(None)` when the id is unreachable (unregistered prefix, missing root, or
    /// missing file); `Err` when a file exists but cannot be parsed.
    pub fn resolve_task(&self, id: &TaskId) -> Result<Option<Task>> {
        if id.prefix == self.project.prefix {
            read_present(self.project, id)
        } else {
            resolve_registered(self.registry, id)
        }
    }
```

and, outside the `impl`:

```rust
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
    let project = crate::scope::open_registered(
        registry,
        &id.prefix,
        crate::scope::Origin::Id(id),
    )?;
    read_present(&project, id)
}
```

- [ ] **Step 4: Add `Scope` to `src/scope.rs`**

Below `open_registered` (`Path` and `CONFIG_REL` are already imported):

```rust
use crate::model::Task;

/// The first two outcomes of spec §3.2: a root that exists and holds a config. The third
/// (a config that does not parse or disagrees with the key) surfaces from
/// `open_registered`, which every reachable root then goes through.
pub fn is_reachable(root: &Path) -> Result<bool> {
    Ok(root.try_exists()? && has_config(root)?)
}

/// The two warnings every registry-wide command shares: an empty registry, and a `cwd`
/// inside a project the registry does not know, which would otherwise vanish from its
/// own portfolio view. The only look at `cwd` a wide command takes, and read-only.
pub fn registry_warnings(registry: &Registry, cwd: &Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if registry.projects.is_empty() {
        warnings.push("registry is empty".into());
    }
    match Project::locate(cwd) {
        Ok(current) if !registry.projects.contains_key(&current.prefix) => {
            warnings.push(format!(
                "current project {} is not registered",
                current.prefix
            ));
        }
        Ok(_) | Err(Error::NoProject(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(warnings)
}

/// What a read command looks at: one project, or every reachable registered project.
pub enum Scope {
    Local(Project),
    All(Vec<Project>),
}

impl Scope {
    /// Every registered project that is reachable, in registry (prefix) order, plus the
    /// warnings the walk produced. Never locates a local project; the only look at `cwd`
    /// is to warn when it lies inside a project the registry does not know.
    pub fn open_all(registry: &Registry, cwd: &Path) -> Result<(Scope, Vec<String>)> {
        let mut warnings = registry_warnings(registry, cwd)?;
        let mut projects = Vec::new();
        for (prefix, root) in &registry.projects {
            if !is_reachable(root)? {
                warnings.push(format!(
                    "project {prefix} at {} is unreachable",
                    root.display()
                ));
                continue;
            }
            projects.push(open_registered(registry, prefix, Origin::Prefix)?);
        }
        Ok((Scope::All(projects), warnings))
    }

    pub fn projects(&self) -> &[Project] {
        match self {
            Scope::Local(project) => std::slice::from_ref(project),
            Scope::All(projects) => projects,
        }
    }

    pub fn prefixes(&self) -> Vec<String> {
        self.projects()
            .iter()
            .map(|project| project.prefix.clone())
            .collect()
    }

    /// The union of every project's tasks, projects in scope order.
    pub fn scan(&self) -> Result<Vec<Task>> {
        let mut all = Vec::new();
        for project in self.projects() {
            all.extend(project.scan()?);
        }
        Ok(all)
    }

    /// One scan per project, for commands that group by project (`tree`).
    pub fn scan_each(&self) -> Result<Vec<(&Project, Vec<Task>)>> {
        self.projects()
            .iter()
            .map(|project| Ok((project, project.scan()?)))
            .collect()
    }

    /// A project in scope answers for its own prefix; anything else goes through the
    /// registry, leniently, as dependency resolution always has.
    pub fn resolve_task(&self, registry: &Registry, id: &TaskId) -> Result<Option<Task>> {
        match self
            .projects()
            .iter()
            .find(|project| project.prefix == id.prefix)
        {
            Some(project) => crate::resolve::read_present(project, id),
            None => crate::resolve::resolve_registered(registry, id),
        }
    }
}
```

Add unit tests to the `tests` module in `src/scope.rs`:

```rust
    #[test]
    fn open_all_skips_unreachable_and_errors_on_malformed() {
        let good = tempfile::tempdir().unwrap();
        write_config(good.path(), "sci");
        let gone = tempfile::tempdir().unwrap();
        let mut registry = registry_with("sci", good.path());
        registry.register("fam", gone.path()).unwrap();
        let (scope, warnings) = Scope::open_all(&registry, gone.path()).unwrap();
        assert_eq!(scope.prefixes(), ["sci"]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("project fam at "), "{warnings:?}");

        std::fs::create_dir_all(gone.path().join("tasks")).unwrap();
        std::fs::write(gone.path().join(CONFIG_REL), "not toml = [").unwrap();
        assert!(matches!(
            Scope::open_all(&registry, gone.path()),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn scope_and_dependency_resolution_share_config_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(CONFIG_REL)).unwrap();
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(!is_reachable(dir.path()).unwrap());
        assert!(
            crate::resolve::resolve_registered(&registry, &id)
                .unwrap()
                .is_none(),
            "dependency resolution treats a non-file config as unreachable"
        );
        let (scope, warnings) = Scope::open_all(&registry, dir.path()).unwrap();
        assert!(scope.projects().is_empty());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));

        std::fs::remove_dir_all(dir.path().join(CONFIG_REL)).unwrap();
        write_config(dir.path(), "zzz");
        assert!(matches!(
            crate::resolve::resolve_registered(&registry, &id),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn open_all_warns_on_empty_registry_and_unregistered_cwd() {
        let registry = Registry::default();
        let nowhere = tempfile::tempdir().unwrap();
        let (_, warnings) = Scope::open_all(&registry, nowhere.path()).unwrap();
        assert_eq!(warnings, ["registry is empty"]);

        let lone = tempfile::tempdir().unwrap();
        write_config(lone.path(), "lon");
        let (_, warnings) = Scope::open_all(&registry, lone.path()).unwrap();
        assert_eq!(
            warnings,
            ["registry is empty", "current project lon is not registered"]
        );
    }
```

- [ ] **Step 5: Add `ReadCtx` and `open_read_ctx` to `src/commands/mod.rs`**

After `open_ctx`:

```rust
pub struct ReadCtx {
    pub scope: Scope,
    pub registry: Registry,
    pub warnings: Vec<String>,
}

impl ReadCtx {
    pub fn resolve_task(&self, id: &TaskId) -> Result<Option<Task>> {
        self.scope.resolve_task(&self.registry, id)
    }
}

pub fn start_dir(dir: Option<&Path>) -> Result<PathBuf> {
    Ok(match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    })
}

/// Read commands: the local project, or with `all_projects` every reachable registered
/// project and no local lookup at all (spec §3.2).
pub fn open_read_ctx(dir: Option<&Path>, all_projects: bool) -> Result<ReadCtx> {
    let start = start_dir(dir)?;
    let registry = Registry::load()?;
    if all_projects {
        let (scope, warnings) = Scope::open_all(&registry, &start)?;
        return Ok(ReadCtx {
            scope,
            registry,
            warnings,
        });
    }
    Ok(ReadCtx {
        scope: Scope::Local(Project::locate(&start)?),
        registry,
        warnings: Vec::new(),
    })
}
```

Make `open_ctx` use `start_dir` too. Add `use crate::scope::Scope;` and `use std::path::PathBuf;`.

- [ ] **Step 6: Convert `list.rs` to `ReadCtx`**

Replace the head of `list` and the dependency loop:

```rust
pub fn list(
    mut ctx: ReadCtx,
    statuses: Vec<String>,
    tags: Vec<String>,
    owner: Option<String>,
    parent: Option<String>,
) -> Result<Output> {
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let mut tasks = ctx.scope.scan()?;
    let all = tasks.clone();
```

(The `if all_projects { ... }` loop is deleted.) The retain block is unchanged. The dependency warning loop becomes:

```rust
    for task in &tasks {
        for dependency in &task.depends {
            if ctx.resolve_task(dependency)?.is_none() {
                ctx.warnings.push(format!(
                    "{}: dependency {dependency} is unreachable",
                    task.id
                ));
            }
        }
    }
```

`ready_tasks`, `ready`, and `prime` change only their context type in this task:

```rust
pub fn ready_tasks(ctx: &mut ReadCtx, all: &[Task]) -> Result<Vec<Task>> {
```

with `resolver.resolve_task(dependency)?` replaced by `ctx.resolve_task(dependency)?` (delete the `let resolver = ...` line), and

```rust
pub fn ready(mut ctx: ReadCtx, size: Option<String>, limit: Option<usize>) -> Result<Output> {
    let size = size.map(|size| Size::parse(&size)).transpose()?;
    let all = ctx.scope.scan()?;
```

```rust
pub fn prime(mut ctx: ReadCtx) -> Result<Output> {
    let all = ctx.scope.scan()?;
```

In `prime`, the uncommitted-files block needs a project. For now (wide `prime` is Task 3) write it over the scope's projects, which for local scope is the one project:

```rust
    for project in ctx.scope.projects() {
        if let Some(files) = project.uncommitted_task_files()?
            && !files.is_empty()
        {
            ctx.warnings
                .push(format!("uncommitted task files: {}", files.join(", ")));
        }
    }
```

and `prefix: ctx.project.prefix.clone()` becomes `prefix: ctx.scope.projects()[0].prefix.clone()` (Task 3 replaces this). Update imports: `use super::ReadCtx;`, drop `Resolver`, `Project`, `CONFIG_REL`, `HashMap` only if unused (the `closed` map in `ready_tasks` still uses `HashMap`).

- [ ] **Step 7: Update dispatch**

In `commands::run`:

```rust
        Command::List {
            statuses,
            tags,
            owner,
            parent,
            all_projects,
        } => list::list(
            open_read_ctx(dir, all_projects)?,
            statuses,
            tags,
            owner,
            parent,
        ),
        Command::Ready { size, limit } => list::ready(open_read_ctx(dir, false)?, size, limit),
        Command::Prime => list::prime(open_read_ctx(dir, false)?),
```

- [ ] **Step 8: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test, including the three existing `list_all_projects_*` tests (the "missing config" one still sees exactly one warning: fam is unreachable, sci is registered).

- [ ] **Step 9: Commit**

```bash
tasks done tasks-fe2041 "Scope and ReadCtx; list --all-projects runs from anywhere"
tasks check
git add src/scope.rs src/resolve.rs src/commands/mod.rs src/commands/list.rs tests/cli.rs tasks/
git commit -m "feat: registry-wide scope for list, no local project needed"
```

---

### Task 3: `ready --all-projects` and `prime --all-projects`

Tracker: `tasks-6d33e6`. Spec §4.1 (`ready`, `prime`), §5 (`prime` shape and pretty header).

Before anything else: `tasks start tasks-6d33e6`.

**Files:**
- Modify: `src/cli.rs` (`Ready`, `Prime`), `src/commands/mod.rs` (dispatch), `src/commands/list.rs` (`prime`), `src/output.rs` (`PrimeOut`, pretty header, `Counts::of`)
- Test: `tests/cli.rs`, `src/query.rs` (the documented created and id tiebreaks)

**Interfaces:**
- Consumes: `ReadCtx`, `Scope::projects`, `Scope::prefixes` (Task 2).
- Produces: `PrimeOut.prefix: Option<String>`, `PrimeOut.projects: Vec<String>`; `Counts::of(tasks: &[Task]) -> Counts` (reused by `projects` in Task 6).

- [ ] **Step 1: Write the failing e2e tests and the id-tiebreak characterization**

Append to `tests/cli.rs`:

```rust
#[test]
fn ready_all_projects_orders_across_projects() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "3"]));
    let high = id_of(env.json(&fam, &["add", "High", "-p", "1", "--size", "s"]));
    let mid = id_of(env.json(&sci, &["add", "Mid", "-p", "1", "--size", "m"]));
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["ready", "--all-projects"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [high.as_str(), mid.as_str(), low.as_str()]);
    let v = env.json(nowhere.path(), &["ready", "--all-projects", "-n", "1"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
}

#[test]
fn prime_all_projects_reports_scope_and_per_project_uncommitted_files() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&fam)
        .status()
        .unwrap();
    assert!(status.success());
    let s = id_of(env.json(&sci, &["add", "S"]));
    let f = id_of(env.json(&fam, &["add", "F"]));
    env.json(&sci, &["start", &s]);

    let local = env.json(&sci, &["prime"]);
    assert_eq!(local["prefix"], "sci");
    assert_eq!(local["projects"], serde_json::json!(["sci"]));

    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["prime", "--all-projects"]);
    assert_eq!(v["prefix"], serde_json::Value::Null);
    assert_eq!(v["projects"], serde_json::json!(["fam", "sci"]));
    assert_eq!(v["counts"]["todo"], 1);
    assert_eq!(v["counts"]["doing"], 1);
    assert_eq!(v["ready"][0]["id"], f);
    assert_eq!(v["doing"][0]["id"], s);
    assert_eq!(v["roadmap"].as_array().unwrap().len(), 2);
    assert_eq!(
        v["warnings"],
        serde_json::json!([format!(
            "fam: uncommitted task files: tasks/.config.toml, tasks/{f}.md"
        )])
    );
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "prime", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("projects fam, sci\n"), "{text}");
}
```

In `src/query.rs`, append to the existing `tests` module. This characterizes the existing
final comparator rather than a new behavior, so it passes before the feature code changes:

```rust
#[test]
fn ready_order_uses_created_then_full_id_for_remaining_ties() {
    let mut newer = t("aaa-000001", Status::Todo, 1, Some(Size::S), &[]);
    newer.created = "2026-08-29T00:00:02Z".into();
    let mut tasks = vec![
        newer,
        t("sci-000001", Status::Todo, 1, Some(Size::S), &[]),
        t("fam-ffffff", Status::Todo, 1, Some(Size::S), &[]),
        t("fam-000001", Status::Todo, 1, Some(Size::S), &[]),
    ];
    sort_ready(&mut tasks);
    let ids: Vec<String> = tasks.iter().map(|task| task.id.to_string()).collect();
    assert_eq!(
        ids,
        ["fam-000001", "fam-ffffff", "sci-000001", "aaa-000001"]
    );
}
```

- [ ] **Step 2: Run the characterization and verify the new command tests fail**

Run: `cargo test query::tests::ready_order_uses_created_then_full_id_for_remaining_ties`, then `cargo test --test cli all_projects_orders`, then `cargo test --test cli prime_all_projects`.
Expected: the characterization test passes; both new e2e tests fail with a clap usage error (exit 2) because the flag does not exist on `ready` and `prime`.

- [ ] **Step 3: Add the flags and dispatch**

`src/cli.rs`:

```rust
    /// Actionable tasks: todo with all dependencies closed.
    Ready {
        #[arg(long)]
        size: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
```

```rust
    /// Session context for agents.
    Prime {
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
```

`src/commands/mod.rs`:

```rust
        Command::Ready {
            size,
            limit,
            all_projects,
        } => list::ready(open_read_ctx(dir, all_projects)?, size, limit),
        Command::Prime { all_projects } => list::prime(open_read_ctx(dir, all_projects)?),
```

- [ ] **Step 4: Change `PrimeOut` and the pretty header**

`src/output.rs`:

```rust
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
```

Add to `Counts`:

```rust
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
```

In `pretty`, the `Output::Prime` arm's header:

```rust
            let header = match &o.prefix {
                Some(prefix) => format!("project {prefix}"),
                None => format!("projects {}", o.projects.join(", ")),
            };
            let mut rendered = format!(
                "{header}\nidea {}  todo {}  doing {}  blocked {}  done {}  dropped {}\n",
                c.idea, c.todo, c.doing, c.blocked, c.done, c.dropped
            );
```

- [ ] **Step 5: Finish `prime` in `list.rs`**

Replace the counts loop with `let counts = Counts::of(&all);`. Replace the uncommitted-files block with the per-project form:

```rust
    let wide = matches!(ctx.scope, Scope::All(_));
    for project in ctx.scope.projects() {
        if let Some(files) = project.uncommitted_task_files()?
            && !files.is_empty()
        {
            let message = format!("uncommitted task files: {}", files.join(", "));
            ctx.warnings.push(if wide {
                format!("{}: {message}", project.prefix)
            } else {
                message
            });
        }
    }
```

and the output head:

```rust
    Ok(Output::Prime(PrimeOut {
        prefix: match &ctx.scope {
            Scope::Local(project) => Some(project.prefix.clone()),
            Scope::All(_) => None,
        },
        projects: ctx.scope.prefixes(),
        counts,
```

Import `crate::scope::Scope`.

- [ ] **Step 6: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test. `prime_reports_counts_ready_and_doing` still passes: `prefix` serializes as `"sci"`.

- [ ] **Step 7: Commit**

```bash
tasks done tasks-6d33e6 "ready and prime take --all-projects; prime reports projects in scope"
tasks check
git add src/cli.rs src/commands/mod.rs src/commands/list.rs src/output.rs src/query.rs tests/cli.rs tasks/
git commit -m "feat: ready and prime across all projects"
```

---

### Task 4: `tree --all-projects` as per-project forests

Tracker: `tasks-d1d97d`. Spec §3.2 (the `tree` exception), §4.1 (`tree`), §6 (clap conflict).

Before anything else: `tasks start tasks-d1d97d`.

**Files:**
- Modify: `src/cli.rs` (`Tree`), `src/commands/mod.rs` (dispatch), `src/commands/tree.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ReadCtx`, `Scope::scan_each` (Task 2), `hierarchy::forest(all: &[Task], root: Option<&TaskId>, include_closed: bool) -> Vec<TreeNode>`.
- Produces: `tree::run(ctx: ReadCtx, id: Option<String>, all: bool) -> Result<Output>`.

- [ ] **Step 1: Write the failing e2e test**

Append to `tests/cli.rs`:

```rust
#[test]
fn tree_all_projects_groups_by_project_in_registry_order() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    // sci's root outranks fam's, but registry order (fam before sci) wins
    let s_goal = id_of(env.json(&sci, &["add", "S goal", "-p", "0"]));
    let s_child = id_of(env.json(&sci, &["add", "S child", "--parent", &s_goal]));
    let f_goal = id_of(env.json(&fam, &["add", "F goal", "-p", "3"]));
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["tree", "--all-projects"]);
    let nodes = v["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "{v}");
    assert_eq!(nodes[0]["id"], f_goal);
    assert_eq!(nodes[1]["id"], s_goal);
    assert_eq!(nodes[1]["children"][0]["id"], s_child);

    let out = env
        .cmd(nowhere.path())
        .args(["tree", &s_goal, "--all-projects"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "id and --all-projects conflict");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli tree_all_projects`
Expected: FAIL, clap rejects `--all-projects` on `tree` (exit 2 where success was expected).

- [ ] **Step 3: Add the flag with the conflict**

`src/cli.rs`:

```rust
    /// The task hierarchy as nested nodes (open work only unless --all).
    Tree {
        id: Option<String>,
        #[arg(long)]
        all: bool,
        /// Every reachable registered project, one forest each; needs no local project.
        #[arg(long, conflicts_with = "id")]
        all_projects: bool,
    },
```

`src/commands/mod.rs`:

```rust
        Command::Tree {
            id,
            all,
            all_projects,
        } => tree::run(open_read_ctx(dir, all_projects)?, id, all),
```

- [ ] **Step 4: Build one forest per project**

`src/commands/tree.rs`:

```rust
use super::ReadCtx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::{Output, TreeOut};

/// One forest per project in scope, concatenated in scope order. The forest builder
/// sorts every root globally, so a single run over the union would interleave projects
/// (spec §3.2). With an id the scope is always local (clap conflict), so the single
/// project's scan is checked for it.
pub fn run(ctx: ReadCtx, id: Option<String>, all: bool) -> Result<Output> {
    let root = id.as_deref().map(TaskId::parse).transpose()?;
    let mut nodes = Vec::new();
    for (_, tasks) in ctx.scope.scan_each()? {
        if let Some(root) = &root
            && !tasks.iter().any(|task| &task.id == root)
        {
            return Err(Error::TaskNotFound(root.to_string()));
        }
        nodes.extend(crate::hierarchy::forest(&tasks, root.as_ref(), all));
    }
    Ok(Output::Tree(TreeOut {
        nodes,
        warnings: ctx.warnings,
    }))
}
```

- [ ] **Step 5: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test, including `tree_nests_prunes_and_orders` (local behavior unchanged).

- [ ] **Step 6: Commit**

```bash
tasks done tasks-d1d97d "tree --all-projects: one forest per project"
tasks check
git add src/cli.rs src/commands/mod.rs src/commands/tree.rs tests/cli.rs tasks/
git commit -m "feat: tree across all projects, grouped by project"
```

---

### Task 5: `next`

Tracker: `tasks-789d72`. Spec §2 (`next`), §4.2 (`next`), §5 (`next` shape and pretty).

Before anything else: `tasks start tasks-789d72`.

**Files:**
- Modify: `src/cli.rs` (`Next`), `src/commands/mod.rs` (dispatch), `src/commands/show.rs` (split into `describe`), `src/commands/list.rs` (`next`), `src/output.rs` (`ShowFields`, `ShowOut`, `NextOut`, `Output::Next`, `show_text`, `warnings_of`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `list::ready_tasks(ctx: &mut ReadCtx, all: &[Task]) -> Result<Vec<Task>>` (Task 2), `Scope::projects` (Task 2), `Resolver`.
- Produces: `output::ShowFields { task, spec_path, plan_path, step_found, depends_on, parent, children }`; `output::ShowOut { fields: ShowFields (flattened), warnings }`; `output::NextOut { next: Option<ShowFields>, warnings }`; `Output::Next(Box<NextOut>)`; `show::describe(project: &Project, registry: &Registry, task: Task, all: &[Task], warnings: &mut Vec<String>) -> Result<ShowFields>`; `list::next(ctx: ReadCtx) -> Result<Output>`.

- [ ] **Step 1: Write the failing e2e test**

Append to `tests/cli.rs`:

```rust
#[test]
fn next_is_the_head_of_ready_in_show_shape() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();

    let empty = env.json(nowhere.path(), &["next", "--all-projects"]);
    assert_eq!(empty["next"], serde_json::Value::Null);
    assert_eq!(empty["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "next", "--all-projects"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "nothing ready");

    // Top and Piece are both P1; Top is sized and Piece is not, so Top sorts first
    // whatever the clock says (timestamps have second precision; ids are random).
    let dep = id_of(env.json(&sci, &["add", "Dep", "-p", "3"]));
    let top = id_of(env.json(
        &fam,
        &["add", "Top", "-p", "1", "--size", "s", "-b", "do the thing", "--depends", &dep],
    ));
    let goal = id_of(env.json(&fam, &["add", "Goal", "-p", "0"]));
    let piece = id_of(env.json(&fam, &["add", "Piece", "-p", "1", "--parent", &goal]));

    // locally, while Top is still blocked on Dep: Piece, with its parent resolved
    let v = env.json(&fam, &["next"]);
    assert_eq!(v["next"]["task"]["id"], piece, "{v}");
    assert_eq!(v["next"]["parent"]["id"], goal);
    assert!(v["next"].get("warnings").is_none(), "warnings live at the top: {v}");

    // once Dep closes, Top is the head across projects, and its dependency is described
    env.json(&sci, &["done", &dep]);
    let v = env.json(nowhere.path(), &["next", "--all-projects"]);
    assert_eq!(v["next"]["task"]["id"], top, "{v}");
    assert_eq!(v["next"]["task"]["body"], "do the thing");
    assert_eq!(v["next"]["depends_on"][0]["id"], dep);
    assert_eq!(v["next"]["depends_on"][0]["status"], "done");
    assert_eq!(v["next"]["spec_path"], serde_json::Value::Null);
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "next", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("title: Top") && text.contains("# depends on"), "{text}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli next_is_the_head`
Expected: FAIL, unknown subcommand `next` (exit 2).

- [ ] **Step 3: Split the show shape**

`src/output.rs`: replace `ShowOut` with

```rust
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
```

Add `Next(Box<NextOut>)` to `Output` after `Show` (boxed like `Show`, since a `Task` inside makes the variant large). In `warnings_of` add `Output::Next(o) => o.warnings.clone(),`. In `pretty`, move the body of the `Output::Show(o)` arm into a function and call it from both arms:

```rust
        Output::Show(o) => show_text(&o.fields, painter),
        Output::Next(o) => match &o.next {
            Some(fields) => show_text(fields, painter),
            None => "nothing ready".into(),
        },
```

```rust
fn show_text(o: &ShowFields, painter: &Painter) -> String {
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
```

This is the existing `Output::Show` arm body verbatim with `o` now a `&ShowFields`.

- [ ] **Step 4: Extract `describe` in `show.rs`**

```rust
use super::Ctx;
use crate::error::Result;
use crate::model::{Task, TaskId};
use crate::output::{DepInfo, Output, Related, ShowFields, ShowOut};
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::Resolver;
use crate::scope::Origin;

pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let foreign;
    let project: &Project = if id.prefix == ctx.project.prefix {
        &ctx.project
    } else {
        foreign = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
        &foreign
    };
    let task = project.read_task(&id)?;
    let all = project.scan()?;
    let fields = describe(project, &ctx.registry, task, &all, &mut ctx.warnings)?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
}

/// The `show` view of `task`, which lives in `project`. `all` is a scan containing that
/// project's tasks (a union is fine: dependencies, parent, and children are looked up
/// by id, and ids carry their prefix). Unreachable dependencies and a missing parent are
/// pushed to `warnings`, never errors.
pub fn describe(
    project: &Project,
    registry: &Registry,
    task: Task,
    all: &[Task],
    warnings: &mut Vec<String>,
) -> Result<ShowFields> {
    let resolver = Resolver::new(project, registry);
    let mut depends_on = Vec::new();
    for dependency in &task.depends {
        // The scan the caller already holds answers first, so `next` describes the same
        // snapshot it chose from; only ids outside it touch the filesystem.
        let resolved = match all.iter().find(|candidate| &candidate.id == dependency) {
            Some(found) => Some(found.clone()),
            None => resolver.resolve_task(dependency)?,
        };
        match resolved {
            Some(task) => depends_on.push(DepInfo {
                id: dependency.to_string(),
                title: Some(task.title),
                status: Some(task.status),
                resolved: true,
            }),
            None => {
                warnings.push(format!("dependency {dependency} is unreachable"));
                depends_on.push(DepInfo {
                    id: dependency.to_string(),
                    title: None,
                    status: None,
                    resolved: false,
                });
            }
        }
    }
    let step_found = match (&task.plan, &task.step) {
        (Some(plan), Some(step)) => Some(resolver.step_exists(plan, step)?),
        _ => None,
    };
    let related = |task: &Task| Related {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: task.status,
    };
    let parent = match &task.parent {
        Some(id) => match all.iter().find(|candidate| &candidate.id == id) {
            Some(found) => Some(related(found)),
            None => {
                warnings.push(format!("parent {id} not found"));
                None
            }
        },
        None => None,
    };
    let mut kids = crate::hierarchy::children(all, &task.id);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
    // `task` is moved last; the two paths above it borrow it first.
    Ok(ShowFields {
        spec_path: task.spec.as_deref().map(|path| resolver.abs(path)),
        plan_path: task.plan.as_deref().map(|path| resolver.abs(path)),
        step_found,
        depends_on,
        parent,
        children,
        task,
    })
}
```

- [ ] **Step 5: Add `next` to `list.rs`, the flag, and dispatch**

`src/commands/list.rs`:

```rust
/// The head of `ready` in the show shape, so a caller can start on it without a second
/// lookup. Nothing ready is a normal state: null, warnings, exit 0.
pub fn next(mut ctx: ReadCtx) -> Result<Output> {
    let all = ctx.scope.scan()?;
    let ready = ready_tasks(&mut ctx, &all)?;
    let next = match ready.into_iter().next() {
        None => None,
        Some(task) => {
            let project = ctx
                .scope
                .projects()
                .iter()
                .find(|project| project.prefix == task.id.prefix)
                .expect("a ready task was scanned from a project in scope");
            let mut warnings = Vec::new();
            let fields =
                super::show::describe(project, &ctx.registry, task, &all, &mut warnings)?;
            ctx.warnings.extend(warnings);
            Some(fields)
        }
    };
    Ok(Output::Next(Box::new(NextOut {
        next,
        warnings: ctx.warnings,
    })))
}
```

Import `NextOut`. `src/cli.rs`, after `Ready`:

```rust
    /// The first ready task, in the show shape; null when nothing is ready.
    Next {
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
```

`src/commands/mod.rs`:

```rust
        Command::Next { all_projects } => list::next(open_read_ctx(dir, all_projects)?),
```

- [ ] **Step 6: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test. The existing `show` tests pass unchanged: `flatten` keeps the JSON keys at the top level.

- [ ] **Step 7: Commit**

```bash
tasks done tasks-789d72 "tasks next: head of ready in the show shape, local or across projects"
tasks check
git add src/cli.rs src/commands/mod.rs src/commands/show.rs src/commands/list.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat: tasks next"
```

---

### Task 6: `root` and `projects`

Tracker: `tasks-5afcc4`. Spec §4.2 (`root`, `projects`), §5 (shapes and pretty).

Before anything else: `tasks start tasks-5afcc4`.

**Files:**
- Create: `src/commands/root.rs`, `src/commands/projects.rs`
- Modify: `src/cli.rs`, `src/commands/mod.rs` (modules, dispatch), `src/output.rs` (`RootOut`, `ProjectRow`, `ProjectsOut`, `Output` variants, pretty, `warnings_of`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `scope::open_registered` (Task 1), `scope::is_reachable`, `scope::registry_warnings`, `commands::start_dir` (Task 2), `Counts::of` (Task 3), `Registry::load`, `Registry.projects`.
- Produces: `root::run(id: String, dir: Option<&Path>) -> Result<Output>`, `projects::run(dir: Option<&Path>) -> Result<Output>`, `output::RootOut { prefix, root, warnings }`, `output::ProjectRow { prefix, root, reachable, counts: Option<Counts> }`, `output::ProjectsOut { projects: Vec<ProjectRow>, warnings }`.

- [ ] **Step 1: Write the failing e2e tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn root_prints_the_registered_root_of_an_id() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["root", "sci-000000"]);
    assert_eq!(v["prefix"], "sci");
    assert_eq!(v["root"], sci.to_str().unwrap());
    assert_eq!(v["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "root", "sci-000000"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), sci.to_str().unwrap());
    assert_eq!(
        env.fail(nowhere.path(), &["root", "zzz-000000"]),
        "unresolvable_id"
    );
    assert_eq!(env.fail(nowhere.path(), &["root", "bogus"]), "invalid_id");

    let mut other_env = TestEnv::new();
    let unregistered = other_env.init("lon");
    let v = env.json(&unregistered, &["root", "sci-000000"]);
    assert_eq!(
        v["warnings"],
        serde_json::json!(["current project lon is not registered"])
    );
}

#[test]
fn projects_lists_the_registry_with_reachability_and_counts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["add", "B", "--status", "idea"]);
    env.json(&sci, &["done", &a]);
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["projects"]);
    let rows = v["projects"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["prefix"], "fam");
    assert_eq!(rows[0]["reachable"], false);
    assert_eq!(rows[0]["counts"], serde_json::Value::Null);
    assert_eq!(rows[1]["prefix"], "sci");
    assert_eq!(rows[1]["root"], sci.to_str().unwrap());
    assert_eq!(rows[1]["reachable"], true);
    assert_eq!(rows[1]["counts"]["idea"], 1);
    assert_eq!(rows[1]["counts"]["done"], 1);
    assert_eq!(v["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("fam") && text.contains("unreachable"), "{text}");
    assert!(text.contains("sci") && text.contains("idea 1"), "{text}");

    std::fs::write(fam.join("tasks/.config.toml"), "not toml = [").unwrap();
    assert_eq!(env.fail(nowhere.path(), &["projects"]), "config");

    // the shared registry warnings apply here too
    let fresh = TestEnv::new();
    let v = fresh.json(nowhere.path(), &["projects"]);
    assert_eq!(v["projects"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!(["registry is empty"]));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli root_prints` then `cargo test --test cli projects_lists`
Expected: both FAIL with unknown subcommand (exit 2).

- [ ] **Step 3: Output shapes**

`src/output.rs`:

```rust
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
}

#[derive(Serialize)]
pub struct ProjectsOut {
    pub projects: Vec<ProjectRow>,
    pub warnings: Vec<String>,
}
```

Add `Root(RootOut)` and `Projects(ProjectsOut)` to `Output`; in `warnings_of` add both arms returning `o.warnings.clone()`; in `pretty`:

```rust
        Output::Root(o) => o.root.clone(),
        Output::Projects(o) => {
            let width = o
                .projects
                .iter()
                .map(|row| row.prefix.len())
                .max()
                .unwrap_or(0);
            let mut rendered = String::new();
            for row in &o.projects {
                let prefix = painter.paint(Style::Chrome, &format!("{:<width$}", row.prefix));
                let state = match &row.counts {
                    Some(c) => format!(
                        "idea {}  todo {}  doing {}  blocked {}  done {}  dropped {}",
                        c.idea, c.todo, c.doing, c.blocked, c.done, c.dropped
                    ),
                    None => painter.paint(Style::Error, "unreachable"),
                };
                rendered.push_str(&format!("{prefix}  {}  {state}\n", row.root));
            }
            rendered
        }
```

- [ ] **Step 4: The commands**

`src/commands/root.rs`:

```rust
use crate::error::Result;
use crate::model::TaskId;
use crate::output::{Output, RootOut};
use crate::registry::Registry;
use crate::scope::{Origin, open_registered, registry_warnings};
use std::path::Path;

/// Where the id's project lives, for a shell alias or a dashboard to jump to. Runs
/// outside any project. The task file is not checked: a missing file is `show`'s to
/// report, and the caller asked for the root.
pub fn run(id: String, dir: Option<&Path>) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let project = open_registered(&registry, &id.prefix, Origin::Id(&id))?;
    Ok(Output::Root(RootOut {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        warnings,
    }))
}
```

`src/commands/projects.rs`:

```rust
use crate::error::Result;
use crate::output::{Counts, Output, ProjectRow, ProjectsOut};
use crate::registry::Registry;
use crate::scope::{Origin, is_reachable, open_registered, registry_warnings};
use std::path::Path;

/// The registry as rows. Reachability is the wide scope's rule (spec §3.2) through the
/// same helpers: a missing root or config is an unreachable row rather than a warning,
/// because here the row is the report; a malformed config is an error. The two shared
/// warnings (empty registry, unregistered current project) are the same as wide scope.
pub fn run(dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut rows = Vec::new();
    for (prefix, root) in &registry.projects {
        let reachable = is_reachable(root)?;
        let counts = if reachable {
            let project = open_registered(&registry, prefix, Origin::Prefix)?;
            Some(Counts::of(&project.scan()?))
        } else {
            None
        };
        rows.push(ProjectRow {
            prefix: prefix.clone(),
            root: root.display().to_string(),
            reachable,
            counts,
        });
    }
    Ok(Output::Projects(ProjectsOut {
        projects: rows,
        warnings,
    }))
}
```

`src/cli.rs`, after `Unregister`:

```rust
    /// The registry: every project, whether it is reachable, and its status counts.
    Projects,
    /// The registered root of the project an id belongs to.
    Root { id: String },
```

`src/commands/mod.rs`: `pub mod projects;`, `pub mod root;`, and

```rust
        Command::Projects => projects::run(dir),
        Command::Root { id } => root::run(id, dir),
```

- [ ] **Step 5: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-5afcc4 "tasks root and tasks projects"
tasks check
git add src/commands/root.rs src/commands/projects.rs src/cli.rs src/commands/mod.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat: tasks root and tasks projects"
```

---

### Task 7: `tags`

Tracker: `tasks-6680f2`. Spec §2 (tags), §4.2 (`tags`), §5 (shape and pretty).

Before anything else: `tasks start tasks-6680f2`.

**Files:**
- Create: `src/commands/tags.rs`
- Modify: `src/cli.rs`, `src/commands/mod.rs`, `src/output.rs` (`TagRow`, `TagsOut`, `Output::Tags`, pretty, `warnings_of`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ReadCtx`, `Scope::scan` (Task 2).
- Produces: `tags::run(ctx: ReadCtx, statuses: Vec<String>) -> Result<Output>`, `output::TagRow { tag, count, projects: BTreeMap<String, usize> }`, `output::TagsOut { tags: Vec<TagRow>, warnings }`.

- [ ] **Step 1: Write the failing e2e test**

Append to `tests/cli.rs`:

```rust
#[test]
fn tags_counts_per_project_and_filters_by_status() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "A", "--tag", "testing", "--tag", "perf"]);
    // a repeated tag counts the task once
    env.json(&sci, &["add", "B", "--tag", "testing", "--tag", "testing"]);
    env.json(&fam, &["add", "C", "--tag", "testing"]);
    let old = id_of(env.json(&fam, &["add", "D", "--tag", "legacy"]));
    env.json(&fam, &["done", &old]);

    let local = env.json(&sci, &["tags"]);
    assert_eq!(
        local["tags"],
        serde_json::json!([
            { "tag": "testing", "count": 2, "projects": { "sci": 2 } },
            { "tag": "perf", "count": 1, "projects": { "sci": 1 } }
        ])
    );

    let nowhere = tempfile::tempdir().unwrap();
    let wide = env.json(nowhere.path(), &["tags", "--all-projects"]);
    assert_eq!(wide["tags"][0]["tag"], "testing");
    assert_eq!(wide["tags"][0]["count"], 3);
    assert_eq!(
        wide["tags"][0]["projects"],
        serde_json::json!({ "fam": 1, "sci": 2 })
    );
    assert_eq!(wide["tags"].as_array().unwrap().len(), 2, "legacy is on a done task");

    let closed = env.json(nowhere.path(), &["tags", "--all-projects", "--status", "done"]);
    assert_eq!(
        closed["tags"],
        serde_json::json!([{ "tag": "legacy", "count": 1, "projects": { "fam": 1 } }])
    );

    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "tags", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("testing") && text.contains("fam 1, sci 2"), "{text}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli tags_counts`
Expected: FAIL, unknown subcommand (exit 2).

- [ ] **Step 3: Shape and pretty**

`src/output.rs`:

```rust
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
```

Add `Tags(TagsOut)` to `Output`, the `warnings_of` arm, and in `pretty`:

```rust
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
```

(The per-project map is always rendered; locally it names the one project, which keeps the wide and local renderings the same shape.)

- [ ] **Step 4: The command**

`src/commands/tags.rs`:

```rust
use super::ReadCtx;
use crate::error::Result;
use crate::model::Status;
use crate::output::{Output, TagRow, TagsOut};
use std::collections::{BTreeMap, BTreeSet};

/// Tag frequencies over open tasks, or over the given statuses: how many tasks carry
/// each tag. Visibility only: this is how a shared vocabulary would be chosen, not
/// enforced.
pub fn run(ctx: ReadCtx, statuses: Vec<String>) -> Result<Output> {
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let all = ctx.scope.scan()?;
    let mut rows: BTreeMap<&str, TagRow> = BTreeMap::new();
    for task in all.iter().filter(|task| {
        if statuses.is_empty() {
            task.status.is_open()
        } else {
            statuses.contains(&task.status)
        }
    }) {
        // a task counts once per tag, however many times the tag is listed on it
        let distinct: BTreeSet<&str> = task.tags.iter().map(String::as_str).collect();
        for tag in distinct {
            let row = rows.entry(tag).or_insert_with(|| TagRow {
                tag: tag.to_string(),
                count: 0,
                projects: BTreeMap::new(),
            });
            row.count += 1;
            *row.projects.entry(task.id.prefix.clone()).or_default() += 1;
        }
    }
    let mut tags: Vec<TagRow> = rows.into_values().collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    Ok(Output::Tags(TagsOut {
        tags,
        warnings: ctx.warnings,
    }))
}
```

`src/cli.rs`, after `Tree`:

```rust
    /// Tag frequencies (open tasks unless --status), per project.
    Tags {
        #[arg(long = "status")]
        statuses: Vec<String>,
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
```

`src/commands/mod.rs`: `pub mod tags;` and

```rust
        Command::Tags {
            statuses,
            all_projects,
        } => tags::run(open_read_ctx(dir, all_projects)?, statuses),
```

- [ ] **Step 5: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-6680f2 "tasks tags: frequencies per project"
tasks check
git add src/commands/tags.rs src/cli.rs src/commands/mod.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat: tasks tags"
```

---

### Task 8: `add --project` with `feedback` on the same path

Tracker: `tasks-f89af3`. Spec §2 (write exception), §4.3, §4.4.

Before anything else: `tasks start tasks-f89af3`.

**Files:**
- Modify: `src/cli.rs` (`Add`), `src/commands/mod.rs` (dispatch), `src/commands/add.rs` (`blank`), `src/commands/feedback.rs` (`create` uses `blank`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `scope::open_registered` (Task 1), `commands::create(project, task)`, `commands::apply_fields(ctx, task, fields)`.
- Produces: `add::blank(project: &Project, title: String, status: Status) -> Result<Task>` (fresh id from `project`, timestamps now, every other field at its default).

- [ ] **Step 1: Write the failing e2e test**

Append to `tests/cli.rs`:

```rust
#[test]
fn add_project_creates_in_the_named_project_from_anywhere() {
    let mut env = TestEnv::new();
    let ops = env.init("ops");
    let fam = env.init("fam");
    write_doc(&fam, "docs/specs/fam-thing.md", "# Fam thing\n");
    let goal = id_of(env.json(&ops, &["add", "Cross-cutting goal"]));
    let groundwork = id_of(env.json(&ops, &["add", "Groundwork"]));
    let fam_parent = id_of(env.json(&fam, &["add", "Fam goal"]));
    let nowhere = tempfile::tempdir().unwrap();

    // a foreign --depends resolves through the registry from the target's point of view
    let id = id_of(env.json(nowhere.path(), &[
        "add", "Fam piece", "--project", "fam", "--parent", &fam_parent,
        "--spec", "fam-thing", "--depends", &groundwork, "--tag", "audit",
    ]));
    assert!(id.starts_with("fam-"), "{id}");
    let shown = env.json(&fam, &["show", &id]);
    assert_eq!(shown["task"]["parent"], fam_parent);
    assert_eq!(shown["task"]["spec"], "docs/specs/fam-thing.md");
    assert_eq!(shown["task"]["depends"][0], groundwork);
    assert_eq!(shown["task"]["tags"][0], "audit");
    assert_eq!(shown["task"]["status"], "todo");

    // validated against the target, not the caller: ops has no such spec or parent
    assert_eq!(
        env.fail(&ops, &["add", "x", "--project", "fam", "--spec", "nope"]),
        "doc_not_found"
    );
    assert_eq!(
        env.fail(&ops, &["add", "x", "--project", "fam", "--parent", &goal]),
        "validation"
    );
    // an explicit prefix targets the registry root, not a displaced checkout with the
    // same prefix
    let displaced = tempfile::tempdir().unwrap();
    std::fs::create_dir(displaced.path().join("tasks")).unwrap();
    std::fs::write(
        displaced.path().join("tasks/.config.toml"),
        "prefix = \"ops\"\n",
    )
    .unwrap();
    let registered = id_of(env.json(
        displaced.path(),
        &["add", "Local", "--project", "ops"],
    ));
    assert!(registered.starts_with("ops-"));
    assert!(ops.join(format!("tasks/{registered}.md")).is_file());
    assert!(!displaced.path().join(format!("tasks/{registered}.md")).exists());
    // an unknown prefix is config, since a person typed it
    assert_eq!(
        env.fail(nowhere.path(), &["add", "x", "--project", "zzz"]),
        "config"
    );
    // the wire-up: the goal depends on the piece, leaves ready while it is open, and
    // returns once it closes
    env.json(&ops, &["dep", &goal, "--on", &id]);
    let in_ready = |env: &TestEnv| {
        env.json(&ops, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == goal)
    };
    assert!(!in_ready(&env));
    env.json(&ops, &["done", &groundwork]);
    env.json(&fam, &["done", &id]);
    assert!(in_ready(&env), "the goal is the verify-and-close step now");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli add_project_creates`
Expected: FAIL, unexpected argument `--project` (exit 2).

- [ ] **Step 3: The flag and lazy dispatch**

`src/cli.rs`:

```rust
    /// Create a task.
    Add {
        title: String,
        #[arg(long, default_value = "todo")]
        status: String,
        /// Create it in this registered project instead of the current one; needs no
        /// local project. Every field is validated against that project.
        #[arg(long)]
        project: Option<String>,
        #[command(flatten)]
        fields: FieldArgs,
    },
```

`src/commands/mod.rs`:

```rust
        Command::Add {
            title,
            status,
            project,
            fields,
        } => {
            // The one write command that may run without a local project (spec §2):
            // an explicit target replaces the lookup, and the unchanged `add` validates
            // every field against whichever project it is handed.
            let ctx = match project {
                Some(prefix) => {
                    let registry = Registry::load()?;
                    let project = crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
                    Ctx {
                        project,
                        registry,
                        warnings: Vec::new(),
                    }
                }
                None => open_ctx(dir)?,
            };
            add::run(ctx, title, status, fields)
        }
```

Add `use crate::scope::Origin;`.

- [ ] **Step 4: One constructor for new tasks**

`src/commands/add.rs`:

```rust
use super::{Ctx, apply_fields, create, id_out};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::output::Output;
use crate::repo::Project;

/// A new task in `project` with a fresh id, timestamps of now, and every other field at
/// its default. The single constructor behind `add` and `feedback`, so a file created
/// in another project is shaped exactly as one created locally.
pub fn blank(project: &Project, title: String, status: Status) -> Result<Task> {
    let now = crate::time::now();
    Ok(Task {
        id: project.new_id()?,
        title,
        status,
        priority: 2,
        size: None,
        owner: None,
        created: now.clone(),
        updated: now,
        depends: vec![],
        parent: None,
        tags: vec![],
        spec: None,
        plan: None,
        step: None,
        body: String::new(),
        notes: vec![],
    })
}

pub fn run(ctx: Ctx, title: String, status: String, fields: FieldArgs) -> Result<Output> {
    let status = Status::parse(&status)?;
    if !matches!(status, Status::Idea | Status::Todo) {
        return Err(Error::Validation(
            "add accepts --status idea or todo".into(),
        ));
    }
    let mut task = blank(&ctx.project, title, status)?;
    apply_fields(&ctx, &mut task, &fields)?;
    create(&ctx.project, &mut task)?;
    Ok(id_out(ctx, &task))
}
```

`src/commands/feedback.rs`, replace its private `create`:

```rust
fn create(
    target: &Project,
    summary: String,
    body: String,
    category: &str,
    from: &str,
) -> Result<Task> {
    let mut task = super::add::blank(target, summary, Status::Idea)?;
    task.tags = vec!["feedback".into(), category.into(), from.into()];
    task.body = body;
    super::create(target, &mut task)?;
    Ok(task)
}
```

- [ ] **Step 5: Run the gates**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo install --path .`
Expected: all pass, and the installed `tasks` is now the code under test, including every `feedback_*` test.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-f89af3 "add --project creates in a registered project; feedback shares the constructor"
tasks check
git add src/cli.rs src/commands/mod.rs src/commands/add.rs src/commands/feedback.rs tests/cli.rs tasks/
git commit -m "feat: add --project"
```

---

### Task 9: Documentation, skill, and reinstall

Tracker: `tasks-559ff6`. Spec §7, §9 (the in-repo part).

Before anything else: `tasks start tasks-559ff6`.

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md` (§5 command reference, §5.1 shapes, §6), `docs/specs/2026-09-04-multi-project-design.md` (status line), `skills/tasks/SKILL.md`, `AGENTS.md`, `README.md`
- Test: `tasks check` (the spec link on the goal task), a manual smoke run of each new command from a directory outside any project

- [ ] **Step 1: Command reference in the original design**

In `docs/specs/2026-08-29-tasks-design.md` §5:

Replace the `add`, `show`, `list`, `tree`, `ready`, and `prime` entries with the text below (each is the existing entry plus the new flag's sentence), and insert the `next`, `tags`, `projects`, and `root` entries after `prime`:

```
tasks add <title> [-b|--body TEXT] [--status idea|todo] [-p N] [--size S]
          [--tag T]... [--depends ID]... [--spec NAME] [--plan NAME] [--step TEXT]
          [--parent ID] [--project PREFIX]
    Create a task. Default status todo, priority 2. --spec/--plan accept either a
    repo-relative path under a configured root or a bare name resolved as the unique
    match across the configured roots (error on 0 or >1 matches). --depends ids and
    --step headings are validated before anything is written. --project creates the
    task in that registered project instead of the current one, validating every field
    against it; no local project is needed. An unregistered prefix is config.

tasks show <id>
    The full task with resolved spec/plan paths, each dependency's title and status,
    whether the step heading still resolves, and the parent and direct children. An id
    with another registered prefix is read from that project, read-only, with the doc
    paths resolved against that project's root; an unregistered or unreachable prefix is
    unresolvable_id.

tasks list [--status S]... [--tag T]... [--owner O] [--all-projects] [--parent ID]
    Default: open tasks, sorted by priority then updated desc. --all-projects walks the
    registry and needs no local project (§6).

tasks tree [<id>] [--all] [--all-projects]
    The hierarchy as nested nodes: the whole forest, or the subtree under <id>. This is
    the read side of parent, as graph is of depends. Without --all the forest is pruned
    to nodes that are open or have an open descendant, so a closed ancestor of open work
    stays visible as context, with its closed status, rather than hiding the work
    beneath it. --all includes every task. Roots and siblings are in ready order
    (priority, size, created); a parent precedes its children. With --all-projects, one
    forest per reachable registered project, concatenated in registry order; <id>
    conflicts with it.

tasks ready [--size S] [-n N] [--all-projects]
    Actionable tasks: todo, no children, and all dependencies closed. Sorted by
    priority, then size (xs first, unsized last), then created, then id.
    --all-projects: the same order over every reachable registered project; no project
    grouping or weighting (the final id tiebreak orders by prefix only among tasks equal
    on everything else).

tasks next [--all-projects]
    The first task of ready in the show shape, or null when nothing is ready (exit 0).

tasks prime [--all-projects]
    Agent session context: prefix, counts by status, the ready list, doing tasks
    with owners, the roadmap (open forest) and closeout list. Intended to be run at
    the start of every agent session. Warns about uncommitted files under tasks/
    (project-relative, from git status, transient temp files excluded); silent when the
    root is not inside a git repository or git is absent. --all-projects: the same over
    every reachable registered project; prefix is null, projects lists the scope, and the
    uncommitted-files warning is emitted per project, prefixed with its prefix.

tasks tags [--status S]... [--all-projects]
    Tag frequencies over open tasks (or the given statuses), with a count per project.

tasks projects
    Every registry entry: root, reachability, and status counts when reachable. Needs no
    project.

tasks root <id>
    The registered root of the id's project. Needs no project; unregistered prefix is
    unresolvable_id.
```

In §5.1 shapes, append:

```
prime       += projects: [string]; prefix is string|null (null under --all-projects)
next        -> { next: ShowFields|null, warnings }   ShowFields = show without warnings
root        -> { prefix, root, warnings }
tags        -> { tags: [{ tag, count, projects: { <prefix>: int } }], warnings }
projects    -> { projects: [{ prefix, root, reachable: bool, counts: Counts|null }], warnings }
```

In §6, after the paragraph defining *unreachable*, add:

```
`--all-projects` (on list, ready, prime, tree, next, tags) reads the registry and locates
no local project: a missing root or config is a warning and the entry is skipped; a
malformed config or a prefix that disagrees with the registry key is a config error.
`projects` applies the same test but reports an unreachable entry as a row with
reachable=false rather than a warning, since the row is the report; a malformed entry is
still a config error and emits the two wide-scope warnings (empty registry; current
directory inside an unregistered project). `root` resolves one prefix strictly:
unregistered or without a config is unresolvable_id, mismatched is config. On success it
emits the unregistered-current-project warning; an empty registry cannot produce a
successful root lookup. See docs/specs/2026-09-04-multi-project-design.md.
```

- [ ] **Step 2: Skill and agent guide**

`skills/tasks/SKILL.md`, session protocol, add after item 2:

```
   With nothing in hand, `tasks next` prints the first ready task in full; `tasks next
   --all-projects` does the same across every registered project.
```

In "Recording work", after the "Blocking on another project" bullet:

```
- Work spanning projects: a goal in the hub project, then one
  `tasks add "<piece>" --project <prefix>` per affected project and one
  `tasks dep <goal> --on <piece>` each. The goal returns to `ready` when the last piece
  closes; verify and `tasks done` it then. `tasks root <id>` prints where a piece lives.
```

`AGENTS.md`, session protocol, first bullet becomes:

```
- Start with `tasks prime`; pick from `tasks ready` (or `tasks next`); `tasks start <id>` before changing code.
```

`README.md` "Use" block, after the `tasks tree` line:

```
    tasks next                       # the first ready task, in full
    tasks next --all-projects        # the same across every registered project
    tasks projects                   # the registry: reachable? counts?
    tasks add "Piece" --project fam  # create in another registered project
```

- [ ] **Step 3: Mark the spec implemented**

In `docs/specs/2026-09-04-multi-project-design.md` change the status line to:

```
**Status:** implemented <date of this commit, YYYY-MM-DD>; see docs/plans/2026-09-04-multi-project.md. Task: tasks-3029be.
```

- [ ] **Step 4: Reinstall and smoke-test**

```bash
cargo install --path .
cd "$(mktemp -d)"
tasks projects --pretty
tasks next --all-projects --pretty | head -5
tasks tags --all-projects --pretty | head -5
tasks root tasks-3029be --pretty
```

Expected: the real registry renders, no errors; unreachable rows, if any, say so.

- [ ] **Step 5: Run the gates and commit**

```bash
cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings
tasks done tasks-559ff6 "docs, skill, README, and spec status for multi-project support"
tasks check
git add docs/ skills/ AGENTS.md README.md tasks/
git commit -m "docs: multi-project commands in the design, skill, and README"
```

Then close the goal from `tasks prime`'s closeout list:

```bash
tasks prime --pretty
tasks done tasks-3029be "multi-project support landed"
tasks check
git add tasks/ && git commit -m "chore: close multi-project goal"
```
