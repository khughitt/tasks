# Task filters implementation plan

**Status:** executed, 2026-09-30. Task: tasks-964c95.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tasks list` and `tasks ready` select by priority, size, complexity, and process
through one shared filter type that also serves `list --parked`.

**Architecture:** A new `src/filter.rs` holds `TaskFilter` (the parsed selection) and
`Fields` (a borrowed view built from a `Task` or a `ParkedRow`). `TaskFilter::matches`
is the only predicate over record fields. A clap `FilterArgs` group is flattened into
`list` and `ready`. `ready_tasks` applies the filter at candidacy, so every warning after
it speaks about selected tasks only.

**Tech Stack:** Rust (edition 2024), clap 4 with `clap_complete`, serde; integration tests
in `tests/cli.rs` through `tests/common/mod.rs`; the shared CLI inventory `cli.toml` in ops.

**Spec:** `docs/specs/2026-09-30-task-filters-design.md` (approved after review round 2).
Also amended by this plan: `docs/specs/2026-09-12-task-complexity-design.md` §4.1.

## Global Constraints

- Work in the tasks worktree `.worktrees/task-filters` (branch `feat/task-filters`). For
  ops, run `work-link --ensure .worktrees` in the ops checkout, then
  `git worktree add .worktrees/task-filters-cli -b feat/task-filters-cli`, then
  `git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/task-filters-cli`,
  then `just setup` there.
- Edit ops `cli.toml` first, then copy it byte for byte to this worktree's `tools/cli.toml`.
  Never hand-edit `tools/cli.toml` into a state the ops source does not have.
- Filter flags: `-p/--priority` (0–4), `--size` (`xs s m l xl none`), `--complexity`
  (`low mid high none`), `--process` (`direct planned none`), all repeatable. `--tag`
  (repeatable, all-of), `--owner`, `--source`, `--parent` (one value each), `--parallel`.
  `--status` stays on `list` only.
- Repeats of one flag widen (OR); different flags narrow (AND). `--tag` keeps all-of.
- `none` selects an unset field. Priority has no `none`.
- `--complexity` matches the effective rating (`complexity::effective` / `complexity::higher`),
  is silent, and never reads `TASKS_MAX_COMPLEXITY`.
- `TaskFilter::parse` is fallible and runs before any scan; `--parent not-an-id` fails
  `invalid_id`.
- Default pools: `list` = open minus shelved (every status with `--periodic`);
  `list --parked` = every open status, shelved included. An explicit `--status` replaces
  the pool.
- No JSON or pretty output shape changes. No new dependency.
- `next`, `prime`, `sample`, `tree`, `graph`, `tags`, `quiet` gain no flags.
- Tests: `just test-one <runner args>` while working, `just test-fast` and `just check`
  before each commit. Never call `cargo test` directly.
- Conventional commits, no attribution trailers. `tasks check` before every commit.

## Review Focus

- `--parent` given under a retired (aliased) prefix must match children whose record names
  the live prefix. Tested in Task 1, Step 1 (`list_parent_filter_resolves_a_retired_prefix`).
- An out-of-range priority (`-p 5`) or unknown size (`--size huge`) must be a usage error
  (exit 2) naming the option. Tested in Task 1, Step 1 (`list_filters_refuse_unknown_values`).
- A task whose record is unassessed but carries an escalation must match its escalated level
  and must not match `none`. Tested in the Task 1 unit tests and end to end in Task 1, Step 1.
- `list --periodic` with a filter must keep closed recurring tasks, because its pool is every
  status. Tested in Task 1, Step 1 (`list_periodic_keeps_its_pool_under_a_filter`).
- Repeating one value (`--size s --size s`) must select the same tasks as giving it once.
  Tested in the Task 1 unit tests.

---

### Task 1: The filter module, wired into `list` and `list --parked`

**Files:**
- Create: `src/filter.rs`
- Modify: `src/main.rs` (add `mod filter;` in alphabetical order, after `mod error;`)
- Modify: `src/cli.rs` (new `FilterArgs`; `Command::List` loses `tags`, `owner`, `source`,
  `parent` and flattens `filter: FilterArgs`)
- Modify: `src/commands/mod.rs` (the `Command::List` dispatch arm)
- Modify: `src/commands/list.rs` (`list`, `list_parked`)
- Modify: `src/complete.rs` (three filter candidate lists)
- Modify: ops `cli.toml` (the `tasks list` row), then copy to `tools/cli.toml`
- Test: `src/filter.rs` (unit), `tests/cli.rs` (end to end), `src/surface.rs` (conformance)

**Interfaces:**
- Produces, in `src/cli.rs`:
  `pub struct FilterArgs { pub priorities: Vec<u8>, pub sizes: Vec<String>, pub complexities: Vec<String>, pub processes: Vec<String>, pub tags: Vec<String>, pub owner: Option<String>, pub source: Option<String>, pub parent: Option<String>, pub parallel: bool }`
  (`#[derive(Args, Debug, Default, Clone)]`).
- Produces, in `src/filter.rs`:
  - `pub const NONE: &str = "none";`
  - `pub struct TaskFilter` (`#[derive(Debug, Default)]`, private fields) with
    `pub fn parse(args: &FilterArgs, statuses: &[String], registry: &Registry) -> Result<TaskFilter>`,
    `pub fn statuses(&self) -> &[Status]`, `pub fn is_empty(&self) -> bool`,
    `pub fn matches(&self, fields: &Fields) -> bool`.
  - `pub struct Fields<'a>` with public fields `status: Status, priority: u8, size: Option<Size>, complexity: Option<Complexity>, process: Option<Process>, tags: &'a [String], owner: Option<&'a str>, source: Option<&'a str>, parent: Option<TaskId>, parallel: bool`, and
    `pub fn of_task(task: &'a Task, claims: &ClaimSnapshot, registry: &Registry) -> Fields<'a>`,
    `pub fn of_row(row: &'a ParkedRow, registry: &Registry) -> Option<Fields<'a>>`.
  - `pub fn check_parent(filter: &TaskFilter, all: &[Task], named_elsewhere: impl Fn(&TaskId) -> bool) -> Result<()>`.
- Produces, in `src/complete.rs`: `pub fn filter_sizes()`, `pub fn filter_complexities()`,
  `pub fn filter_processes()`, each `-> Vec<CompletionCandidate>`.
- Consumes: `crate::commands::parse_id(&Registry, &str) -> Result<TaskId>`,
  `crate::complexity::{effective, higher}`, `Status::parse`, `Size::parse`,
  `Complexity::parse`, `Process::parse`, `Error::TaskNotFound(String)`.

- [ ] **Step 1: Write the failing end-to-end tests**

Append to `tests/cli.rs`:

```rust
fn list_ids(value: &serde_json::Value) -> Vec<String> {
    value["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn list_filters_by_priority_size_complexity_and_process() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let a = id_of(env.json(
        &sci,
        &["add", "A", "-p", "0", "--size", "s", "--complexity", "low", "--process", "direct"],
    ));
    let b = id_of(env.json(
        &sci,
        &["add", "B", "-p", "1", "--size", "m", "--complexity", "mid", "--process", "planned"],
    ));
    let c = id_of(env.json(&sci, &["add", "C", "-p", "2"]));

    assert_eq!(list_ids(&env.json(&sci, &["list", "-p", "0"])), [a.clone()]);
    assert_eq!(
        list_ids(&env.json(&sci, &["list", "-p", "0", "--priority", "1"])),
        [a.clone(), b.clone()],
        "repeats widen"
    );
    assert_eq!(list_ids(&env.json(&sci, &["list", "--size", "m"])), [b.clone()]);
    assert_eq!(list_ids(&env.json(&sci, &["list", "--size", "none"])), [c.clone()]);
    assert_eq!(
        list_ids(&env.json(&sci, &["list", "--size", "s", "--size", "none"])),
        [a.clone(), c.clone()]
    );
    assert_eq!(list_ids(&env.json(&sci, &["list", "--complexity", "mid"])), [b.clone()]);
    assert_eq!(list_ids(&env.json(&sci, &["list", "--complexity", "none"])), [c.clone()]);
    assert_eq!(list_ids(&env.json(&sci, &["list", "--process", "direct"])), [a.clone()]);
    assert_eq!(list_ids(&env.json(&sci, &["list", "--process", "none"])), [c.clone()]);
    assert!(
        list_ids(&env.json(&sci, &["list", "-p", "0", "--size", "m"])).is_empty(),
        "different flags narrow"
    );
}

#[test]
fn list_complexity_filter_reads_the_escalated_rating() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let mid = id_of(env.json(&sci, &["add", "Mid", "--complexity", "mid"]));
    let bare = id_of(env.json(&sci, &["add", "Bare"]));
    let path = env.claim_store("sci");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            "[escalations.\"{mid}\"]\nlevel = \"high\"\nat = \"2026-09-30T00:00:00Z\"\nsession = \"s:a\"\n\
             [escalations.\"{bare}\"]\nlevel = \"high\"\nat = \"2026-09-30T00:00:00Z\"\nsession = \"s:a\"\n"
        ),
    )
    .unwrap();

    // Equal priorities: list order falls to last activity, then id, so compare as sets.
    let mut high = list_ids(&env.json(&sci, &["list", "--complexity", "high"]));
    high.sort();
    let mut expected = vec![mid.clone(), bare.clone()];
    expected.sort();
    assert_eq!(high, expected);
    assert!(list_ids(&env.json(&sci, &["list", "--complexity", "mid"])).is_empty());
    assert!(
        list_ids(&env.json(&sci, &["list", "--complexity", "none"])).is_empty(),
        "an escalated record is not unassessed"
    );
}

#[test]
fn list_parked_applies_the_filter_and_keeps_shelved_in_its_pool() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let small = id_of(env.json(&sci, &["add", "Small", "--size", "s"]));
    let shelved = id_of(env.json(&sci, &["add", "Shelved", "--size", "s"]));
    env.json(&sci, &["shelve", &shelved, "later"]);
    let large = id_of(env.json(&sci, &["add", "Large", "--size", "l"]));
    let root = sci.display().to_string();
    for id in [&small, &shelved, &large] {
        write_park(&env, "sci", id, "agent-a", "agent", &root);
    }
    // A park whose task no scan holds: an unresolved row.
    write_park(&env, "sci", "sci-ffffff", "agent-a", "agent", &root);

    let all = list_ids(&env.json(&sci, &["list", "--parked"]));
    assert_eq!(all.len(), 4, "{all:?}");
    assert!(all.contains(&shelved), "shelved stays in the parked pool");
    assert!(all.contains(&"sci-ffffff".to_string()));

    let mut small_rows = list_ids(&env.json(&sci, &["list", "--parked", "--size", "s"]));
    small_rows.sort();
    let mut expected = vec![small.clone(), shelved.clone()];
    expected.sort();
    assert_eq!(small_rows, expected, "the unresolved row drops once a filter is given");

    assert_eq!(
        list_ids(&env.json(&sci, &["list", "--parked", "-p", "2", "--size", "l"])),
        [large.clone()]
    );
}

#[test]
fn list_parent_filter_keeps_its_errors() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    assert_eq!(
        env.fail(&sci, &["list", "--parent", "not-an-id"]),
        "invalid_id"
    );
    assert_eq!(
        env.fail(&sci, &["list", "--parent", "sci-abcdef"]),
        "task_not_found"
    );
    assert_eq!(
        env.fail(&sci, &["list", "--parked", "--parent", "sci-abcdef"]),
        "task_not_found"
    );
}

#[test]
fn list_parent_filter_resolves_a_retired_prefix() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal"]));
    let child = id_of(env.json(&sci, &["add", "Child", "--parent", &goal]));
    alias_registry(&env, "old", "sci");
    let retired = format!("old-{}", &goal["sci-".len()..]);
    assert_eq!(
        list_ids(&env.json(&sci, &["list", "--parent", &retired])),
        [child]
    );
}

#[test]
fn list_periodic_keeps_its_pool_under_a_filter() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let sweep = id_of(env.json(&sci, &["add", "Sweep", "--size", "s", "--every", "30d"]));
    env.json(&sci, &["start", &sweep]);
    env.json(&sci, &["done", &sweep, "swept"]);
    assert_eq!(
        list_ids(&env.json(&sci, &["list", "--periodic", "--size", "s"])),
        [sweep]
    );
    assert!(list_ids(&env.json(&sci, &["list", "--periodic", "--size", "m"])).is_empty());
}

#[test]
fn list_filters_refuse_unknown_values() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let err = env.usage(&sci, &["list", "-p", "5"]);
    assert!(err.contains("--priority") && err.contains("5"), "{err}");
    let err = env.usage(&sci, &["list", "--size", "huge"]);
    assert!(err.contains("--size") && err.contains("huge"), "{err}");
    let err = env.usage(&sci, &["list", "--process", "maybe"]);
    assert!(err.contains("--process") && err.contains("maybe"), "{err}");
}

#[test]
fn list_filter_flags_complete_with_none() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    assert_eq!(
        env.complete_values(&sci, "bash", 3, &["tasks", "list", "--size", ""]),
        vec!["xs", "s", "m", "l", "xl", "none"]
    );
    assert_eq!(
        env.complete_values(&sci, "bash", 3, &["tasks", "list", "--complexity", ""]),
        vec!["low", "mid", "high", "none"]
    );
    assert_eq!(
        env.complete_values(&sci, "bash", 3, &["tasks", "list", "--process", ""]),
        vec!["direct", "planned", "none"]
    );
    assert_eq!(
        env.complete_values(&sci, "bash", 3, &["tasks", "list", "-p", ""]),
        vec!["0", "1", "2", "3", "4"]
    );
}
```

If `alias_registry`'s signature differs from `(env, alias, target)`, read it at the top of
`tests/cli.rs` (near line 740) and match it; do not write a second helper.

- [ ] **Step 2: Run the tests to see them fail**

Run: `just test-one --test cli list_`
Expected: the new tests fail. Clap refuses `-p`, `--size`, `--complexity`, and `--process`
on `list` as unknown arguments.

- [ ] **Step 3: Create `src/filter.rs` with its unit tests**

```rust
//! One selection over record fields for the read commands that filter
//! (docs/specs/2026-09-30-task-filters-design.md). Repeats of one field widen (any of
//! them); different fields narrow (all of them); `--tag` alone is all-of. Each command
//! keeps its own default status pool; an explicit `--status` replaces it here.

use crate::claims::ClaimSnapshot;
use crate::cli::FilterArgs;
use crate::error::{Error, Result};
use crate::model::{Complexity, Process, Size, Status, Task, TaskId};
use crate::output::ParkedRow;
use crate::registry::Registry;

/// The flag value that selects a record whose field is unset.
pub const NONE: &str = "none";

#[derive(Debug, Default)]
pub struct TaskFilter {
    statuses: Vec<Status>,
    priorities: Vec<u8>,
    sizes: Vec<Option<Size>>,
    complexities: Vec<Option<Complexity>>,
    processes: Vec<Option<Process>>,
    tags: Vec<String>,
    owner: Option<String>,
    source: Option<String>,
    parent: Option<TaskId>,
    parallel: bool,
}

/// What the filter reads from a record, whichever row type carries it.
pub struct Fields<'a> {
    pub status: Status,
    pub priority: u8,
    pub size: Option<Size>,
    /// The effective rating: the record's or a shared escalation's, whichever is higher.
    pub complexity: Option<Complexity>,
    pub process: Option<Process>,
    pub tags: &'a [String],
    pub owner: Option<&'a str>,
    pub source: Option<&'a str>,
    /// Canonical, so a retired prefix and its live one compare equal.
    pub parent: Option<TaskId>,
    pub parallel: bool,
}

fn optional<T>(value: &str, parse: impl Fn(&str) -> Result<T>) -> Result<Option<T>> {
    if value == NONE {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}

fn any_of<T: PartialEq>(wanted: &[T], value: &T) -> bool {
    wanted.is_empty() || wanted.contains(value)
}

impl TaskFilter {
    /// Fallible: clap has refused unknown enum values already, but `--parent` is free
    /// text and fails `invalid_id` here, before any scan.
    pub fn parse(args: &FilterArgs, statuses: &[String], registry: &Registry) -> Result<TaskFilter> {
        Ok(TaskFilter {
            statuses: statuses
                .iter()
                .map(|status| Status::parse(status))
                .collect::<Result<_>>()?,
            priorities: args.priorities.clone(),
            sizes: args
                .sizes
                .iter()
                .map(|value| optional(value, Size::parse))
                .collect::<Result<_>>()?,
            complexities: args
                .complexities
                .iter()
                .map(|value| optional(value, Complexity::parse))
                .collect::<Result<_>>()?,
            processes: args
                .processes
                .iter()
                .map(|value| optional(value, Process::parse))
                .collect::<Result<_>>()?,
            tags: args.tags.clone(),
            owner: args.owner.clone(),
            source: args.source.clone(),
            parent: args
                .parent
                .as_deref()
                .map(|id| crate::commands::parse_id(registry, id))
                .transpose()?,
            parallel: args.parallel,
        })
    }

    /// The explicit `--status` set; empty means the command's default pool applies.
    pub fn statuses(&self) -> &[Status] {
        &self.statuses
    }

    /// No field is constrained.
    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty()
            && self.priorities.is_empty()
            && self.sizes.is_empty()
            && self.complexities.is_empty()
            && self.processes.is_empty()
            && self.tags.is_empty()
            && self.owner.is_none()
            && self.source.is_none()
            && self.parent.is_none()
            && !self.parallel
    }

    pub fn matches(&self, fields: &Fields) -> bool {
        any_of(&self.statuses, &fields.status)
            && any_of(&self.priorities, &fields.priority)
            && any_of(&self.sizes, &fields.size)
            && any_of(&self.complexities, &fields.complexity)
            && any_of(&self.processes, &fields.process)
            && self.tags.iter().all(|tag| fields.tags.contains(tag))
            && self.owner.as_deref().is_none_or(|owner| fields.owner == Some(owner))
            && self.source.as_deref().is_none_or(|source| fields.source == Some(source))
            && self.parent.as_ref().is_none_or(|parent| fields.parent.as_ref() == Some(parent))
            && (!self.parallel || fields.parallel)
    }
}

impl<'a> Fields<'a> {
    pub fn of_task(task: &'a Task, claims: &ClaimSnapshot, registry: &Registry) -> Fields<'a> {
        Fields {
            status: task.status,
            priority: task.priority,
            size: task.size,
            complexity: crate::complexity::effective(task, claims),
            process: task.process,
            tags: &task.tags,
            owner: task.owner.as_deref(),
            source: task.source.as_deref(),
            parent: task.parent.as_ref().map(|parent| registry.canonical_id(parent)),
            parallel: task.parallel,
        }
    }

    /// None for an unresolved park: it has no record to match.
    pub fn of_row(row: &'a ParkedRow, registry: &Registry) -> Option<Fields<'a>> {
        Some(Fields {
            status: row.status?,
            priority: row.priority?,
            size: row.size,
            complexity: crate::complexity::higher(
                row.complexity,
                row.escalation.as_ref().map(|escalation| escalation.level),
            ),
            process: row.process,
            tags: &row.tags,
            owner: row.owner.as_deref(),
            source: row.source.as_deref(),
            parent: row
                .parent
                .as_deref()
                .and_then(|parent| TaskId::parse(parent).ok())
                .map(|id| registry.canonical_id(&id)),
            parallel: row.parallel,
        })
    }
}

/// `--parent` must name a task in scope. `named_elsewhere` admits one that only a row
/// outside the scan names (a parked row read from another checkout).
pub fn check_parent(
    filter: &TaskFilter,
    all: &[Task],
    named_elsewhere: impl Fn(&TaskId) -> bool,
) -> Result<()> {
    match &filter.parent {
        Some(parent) if !all.iter().any(|task| task.id == *parent) && !named_elsewhere(parent) => {
            Err(Error::TaskNotFound(parent.to_string()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_TAGS: &[String] = &[];

    fn fields() -> Fields<'static> {
        Fields {
            status: Status::Todo,
            priority: 2,
            size: Some(Size::S),
            complexity: Some(Complexity::Mid),
            process: None,
            tags: NO_TAGS,
            owner: None,
            source: None,
            parent: None,
            parallel: false,
        }
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        let filter = TaskFilter::default();
        assert!(filter.is_empty());
        assert!(filter.matches(&fields()));
    }

    #[test]
    fn repeats_widen_and_fields_narrow() {
        let filter = TaskFilter {
            priorities: vec![0, 2],
            sizes: vec![Some(Size::M), Some(Size::S)],
            ..TaskFilter::default()
        };
        assert!(!filter.is_empty());
        assert!(filter.matches(&fields()));
        let narrower = TaskFilter {
            priorities: vec![2],
            sizes: vec![Some(Size::M)],
            ..TaskFilter::default()
        };
        assert!(!narrower.matches(&fields()));
    }

    #[test]
    fn a_repeated_value_selects_as_once() {
        let twice = TaskFilter {
            sizes: vec![Some(Size::S), Some(Size::S)],
            ..TaskFilter::default()
        };
        let once = TaskFilter {
            sizes: vec![Some(Size::S)],
            ..TaskFilter::default()
        };
        assert_eq!(twice.matches(&fields()), once.matches(&fields()));
        assert!(twice.matches(&fields()));
    }

    #[test]
    fn none_selects_unset_alone_or_mixed() {
        let unassessed = TaskFilter {
            processes: vec![None],
            ..TaskFilter::default()
        };
        assert!(unassessed.matches(&fields()));
        let mixed = TaskFilter {
            processes: vec![Some(Process::Direct), None],
            ..TaskFilter::default()
        };
        assert!(mixed.matches(&fields()));
        let set_only = TaskFilter {
            processes: vec![Some(Process::Direct)],
            ..TaskFilter::default()
        };
        assert!(!set_only.matches(&fields()));
    }

    #[test]
    fn tags_are_all_of_and_scalars_are_exact() {
        let tags = vec!["cli".to_string(), "picker".to_string()];
        let tagged = Fields {
            tags: &tags,
            owner: Some("keith"),
            parallel: true,
            ..fields()
        };
        let both = TaskFilter {
            tags: vec!["cli".into(), "picker".into()],
            owner: Some("keith".into()),
            parallel: true,
            ..TaskFilter::default()
        };
        assert!(both.matches(&tagged));
        let extra = TaskFilter {
            tags: vec!["cli".into(), "docs".into()],
            ..TaskFilter::default()
        };
        assert!(!extra.matches(&tagged));
        let parallel = TaskFilter {
            parallel: true,
            ..TaskFilter::default()
        };
        assert!(!parallel.matches(&fields()));
    }

    #[test]
    fn an_escalation_above_the_record_is_the_rating_matched() {
        // of_task reads complexity::effective; this pins the same rule of_row uses.
        assert_eq!(
            crate::complexity::higher(None, Some(Complexity::High)),
            Some(Complexity::High)
        );
        let escalated = Fields {
            complexity: crate::complexity::higher(Some(Complexity::Mid), Some(Complexity::High)),
            ..fields()
        };
        let high = TaskFilter {
            complexities: vec![Some(Complexity::High)],
            ..TaskFilter::default()
        };
        let none = TaskFilter {
            complexities: vec![None],
            ..TaskFilter::default()
        };
        assert!(high.matches(&escalated));
        assert!(!none.matches(&escalated));
    }

    #[test]
    fn an_unresolved_park_row_has_no_fields() {
        let park = crate::claims::Park {
            owner: "o".into(),
            session: "s:a".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-09-30T00:00:00Z".into(),
            next_step: "n".into(),
            waiting_on: crate::claims::WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "T".into(),
        };
        let row = ParkedRow::unresolved("sci-ffffff", &park);
        assert!(Fields::of_row(&row, &Registry::default()).is_none());
    }
}
```

- [ ] **Step 4: Add `FilterArgs` and flatten it into `list`**

In `src/cli.rs`, after `ScopeArgs`:

```rust
/// Selection over record fields, shared by `list` and `ready`. Repeats of one flag widen
/// (any of them); different flags narrow (all of them); `--tag` is all-of.
#[derive(Args, Debug, Default, Clone)]
pub struct FilterArgs {
    /// Only tasks of this priority (repeatable). Repeats of one filter widen; different
    /// filters narrow.
    #[arg(
        short = 'p',
        long = "priority",
        value_name = "N",
        value_parser = clap::value_parser!(u8).range(0..=4),
        add = ArgValueCandidates::new(crate::complete::priorities),
        add = ValueSet
    )]
    pub priorities: Vec<u8>,
    /// Only tasks of this size (repeatable); `none` selects unsized tasks.
    #[arg(
        long = "size",
        value_name = "SIZE",
        add = ArgValueCandidates::new(crate::complete::filter_sizes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub sizes: Vec<String>,
    /// Only tasks at this effective complexity (repeatable); `none` selects unassessed
    /// tasks. A selection, not the session cutoff.
    #[arg(
        long = "complexity",
        value_name = "LEVEL",
        add = ArgValueCandidates::new(crate::complete::filter_complexities),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub complexities: Vec<String>,
    /// Only tasks with this process (repeatable); `none` selects unassessed tasks.
    #[arg(
        long = "process",
        value_name = "PROCESS",
        add = ArgValueCandidates::new(crate::complete::filter_processes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub processes: Vec<String>,
    /// Filter by tag (repeatable); a task must carry every one.
    #[arg(long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,
    /// Only tasks owned by this value.
    #[arg(long)]
    pub owner: Option<String>,
    /// Only tasks whose source is exactly this reference; matched byte for byte,
    /// never interpreted, so it answers "what came from here" for any origin.
    #[arg(long)]
    pub source: Option<String>,
    /// Only direct children of this task.
    #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
    pub parent: Option<String>,
    /// Only tasks marked safe to run beside each other.
    #[arg(long)]
    pub parallel: bool,
}
```

In `Command::List`, delete the `tags`, `owner`, `source`, and `parent` fields and their
attributes. Add `#[command(flatten)] filter: FilterArgs,` directly after `statuses`.
Update the `after_help` example to
`"Examples:\n  tasks list --sort updated\n  tasks list --status todo --tag cli\n  tasks list -p 0 -p 1 --size s --size xs"`.

In `src/commands/mod.rs`, replace the `Command::List { … }` arm with:

```rust
        Command::List {
            statuses,
            filter,
            sort,
            reverse,
            parked,
            periodic,
            deferred,
            scope,
        } => list::list(
            open_read_ctx(dir, &scope)?,
            statuses,
            filter,
            sort,
            reverse,
            parked,
            periodic,
            deferred,
        ),
```

Add `mod filter;` to `src/main.rs` after `mod error;`.

- [ ] **Step 5: Add the completion lists**

In `src/complete.rs`, after `processes()`:

```rust
/// `--size` as a filter: the sizes, then `none` for unsized.
pub fn filter_sizes() -> Vec<CompletionCandidate> {
    plain(
        Size::ALL
            .iter()
            .map(|size| size.as_str())
            .chain([crate::filter::NONE]),
    )
}

/// `--complexity` as a filter: the levels, then `none` for unassessed.
pub fn filter_complexities() -> Vec<CompletionCandidate> {
    plain(
        Complexity::ALL
            .iter()
            .map(|level| level.as_str())
            .chain([crate::filter::NONE]),
    )
}

/// `--process` as a filter: the processes, then `none` for unassessed.
pub fn filter_processes() -> Vec<CompletionCandidate> {
    plain(
        Process::ALL
            .iter()
            .map(|process| process.as_str())
            .chain([crate::filter::NONE]),
    )
}
```

- [ ] **Step 6: Route `list` and `list --parked` through the filter**

In `src/commands/list.rs`, add `use crate::cli::FilterArgs;` and
`use crate::filter::{Fields, TaskFilter, check_parent};`. Drop the now-unused
`Status::parse` mapping. Replace `list` and `list_parked`'s signatures and filter code:

```rust
#[allow(clippy::too_many_arguments)]
pub fn list(
    mut ctx: ReadCtx,
    statuses: Vec<String>,
    filter: FilterArgs,
    sort: String,
    reverse: bool,
    parked: bool,
    periodic: bool,
    deferred: bool,
) -> Result<Output> {
    let sort = SortKey::parse(&sort)?;
    let filter = TaskFilter::parse(&filter, &statuses, &ctx.registry)?;
    if parked {
        return list_parked(ctx, filter);
    }
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    check_parent(&filter, &all, |_| false)?;
    let mut tasks = all.clone();
    tasks.retain(|task| {
        let periodic_ok = !periodic || task.every.is_some();
        let deferred_ok = !deferred || task.defer.is_some();
        // The default pool when --status is absent: open minus shelved, or every status
        // for --periodic, since most of a healthy series is closed (spec §5.2).
        let pool_ok = !filter.statuses().is_empty()
            || periodic
            || (task.status.is_open() && task.status != Status::Shelved);
        periodic_ok
            && deferred_ok
            && pool_ok
            && filter.matches(&Fields::of_task(task, &claims, &ctx.registry))
    });
    // … the dependency warnings, sort, and output below are unchanged …
}

fn list_parked(mut ctx: ReadCtx, filter: TaskFilter) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let rows = super::parked::rows(&mut ctx, &all, &claims, now)?;
    let warnings = std::mem::take(&mut ctx.warnings);
    check_parent(&filter, &all, |parent| {
        rows.iter().any(|row| {
            Fields::of_row(row, &ctx.registry).is_some_and(|fields| fields.parent.as_ref() == Some(parent))
        })
    })?;
    let tasks = rows
        .into_iter()
        .filter(|row| match Fields::of_row(row, &ctx.registry) {
            // An unresolved park has no record to match: shown only when nothing filters.
            None => filter.is_empty(),
            // The parked pool is every open status, shelved included, so a surviving
            // shelved park stays visible for cleanup.
            Some(fields) => {
                (!filter.statuses().is_empty() || fields.status.is_open()) && filter.matches(&fields)
            }
        })
        .collect();
    Ok(Output::Parked(ParkedOut { tasks, warnings }))
}
```

Delete the old `row_parent` closure, the `filtered` flag, and the per-field checks. Keep
`list`'s existing dependency-warning loop, sort, and `Output::List` construction exactly
as they are.

- [ ] **Step 7: Change the ops inventory, then copy it**

In the ops worktree (see Global Constraints), replace the `tasks list` row's `options` in
`cli.toml` with:

```toml
options = [
  { shared = "status", role = "filter", value = "enum", values = ["idea", "todo", "doing", "blocked", "shelved", "done", "dropped"], repeatable = true },
  { names = ["--priority", "-p"], value = "enum", values = ["0", "1", "2", "3", "4"], repeatable = true },
  { names = ["--size"], value = "enum", values = ["xs", "s", "m", "l", "xl", "none"], repeatable = true },
  { names = ["--complexity"], value = "enum", values = ["low", "mid", "high", "none"], repeatable = true },
  { names = ["--process"], value = "enum", values = ["direct", "planned", "none"], repeatable = true },
  { shared = "tag", role = "filter", value = "string", repeatable = true },
  { names = ["--owner"], value = "string" },
  { names = ["--source"], value = "string" },
  { names = ["--parent"], value = "ref" },
  { names = ["--parallel"], value = "none" },
  { shared = "sort", role = "select", value = "enum", values = ["priority", "updated", "created"], default = "priority" },
  { shared = "reverse", value = "none" },
  { names = ["--parked"], value = "none" },
  { names = ["--periodic"], value = "none" },
  { names = ["--deferred"], value = "none" },
  { shared = "project", role = "select", value = "string", repeatable = false },
  { shared = "all_projects", value = "none" },
]
```

Run in the ops worktree: `just test-one test_cli`. Expected: PASS. Then copy:
`cp <ops worktree>/cli.toml .worktrees/task-filters/tools/cli.toml`. Leave the ops change
uncommitted; Task 3 publishes and commits it.

- [ ] **Step 8: Run the focused tests**

Run: `just test-one filter::` then `just test-one --test cli list_` then `just test-one surface`
Expected: all PASS. If the surface test reports a difference, fix the ops row first and
copy again. Never edit `tools/cli.toml` alone.

- [ ] **Step 9: Run the fast suite and the check, then commit**

Run: `just test-fast` and `just check`. Expected: both pass.

```bash
git add src/filter.rs src/main.rs src/cli.rs src/commands/mod.rs src/commands/list.rs src/complete.rs tools/cli.toml tests/cli.rs
git commit -m "feat(list): filter by priority, size, complexity, and process"
```

---

### Task 2: `ready` takes the same filter at candidacy

**Files:**
- Modify: `src/cli.rs` (`Command::Ready` loses `size` and `parallel`, flattens `filter: FilterArgs`)
- Modify: `src/commands/mod.rs` (the `Command::Ready` arm)
- Modify: `src/commands/list.rs` (`ready_tasks`, `ready`, `next`, `prime`)
- Modify: `tests/cli.rs` (the cutoff composition test in
  `ready_and_next_hide_above_cutoff_and_unassessed_with_counts`; the completion test that
  expects `ready --size` to offer `xs s m l xl`)
- Modify: `docs/specs/2026-09-12-task-complexity-design.md` §4.1 and its verification bullet
- Modify: ops `cli.toml` (the `tasks ready` row), then copy to `tools/cli.toml`
- Test: `tests/cli.rs`, `src/surface.rs`

**Interfaces:**
- Consumes (Task 1): `FilterArgs`, `TaskFilter::{parse, matches}`, `Fields::of_task`,
  `check_parent`, `complete::filter_sizes`.
- Produces: `pub fn ready_tasks(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot, snapshots: &HashMap<String, HaltSnapshot>, filter: &TaskFilter, now: OffsetDateTime) -> Result<Picked>`,
  and `pub fn ready(ctx: ReadCtx, filter: FilterArgs, limit: Option<usize>, max_complexity: Option<String>) -> Result<Output>`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn ready_takes_the_list_filters() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "1"]));
    let child = id_of(env.json(
        &sci,
        &["add", "Child", "-p", "0", "--size", "s", "--tag", "cli", "--parent", &goal],
    ));
    let loose = id_of(env.json(&sci, &["add", "Loose", "-p", "2"]));
    let medium = id_of(env.json(&sci, &["add", "Medium", "-p", "3", "--size", "m"]));

    let ids = |args: &[&str]| list_ids(&env.json(&sci, args));
    assert_eq!(ids(&["ready", "-p", "0"]), [child.clone()]);
    assert_eq!(ids(&["ready", "--tag", "cli"]), [child.clone()]);
    assert_eq!(ids(&["ready", "--parent", &goal]), [child.clone()]);
    assert_eq!(
        ids(&["ready", "--size", "s", "--size", "none"]),
        [child.clone(), loose.clone()]
    );
    assert_eq!(ids(&["ready", "--size", "m"]), [medium.clone()], "one value, as before");
    assert_eq!(env.fail(&sci, &["ready", "--parent", "not-an-id"]), "invalid_id");
    assert_eq!(env.fail(&sci, &["ready", "--parent", "sci-abcdef"]), "task_not_found");
}

#[test]
fn ready_warns_only_about_selected_tasks() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let small = id_of(env.json(&sci, &["add", "Small", "--size", "s"]));
    let claimed = id_of(env.json(&sci, &["add", "Claimed", "--size", "m"]));
    write_claim(&env, "sci", &claimed, "other-session", true);

    let v = env.json(&sci, &["ready"]);
    assert!(v["warnings"].to_string().contains(&claimed), "{v}");
    let v = env.json(&sci, &["ready", "--size", "s"]);
    assert_eq!(list_ids(&v), [small]);
    assert!(
        !v["warnings"].to_string().contains(&claimed),
        "a claimed task outside the selection is not reported: {v}"
    );
}

#[test]
fn ready_selection_and_cutoff_intersect() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let mid = id_of(env.json(&sci, &["add", "Mid", "--complexity", "mid"]));
    env.json(&sci, &["add", "High", "--complexity", "high"]);
    env.json(&sci, &["add", "Unassessed"]);

    let v = env.json(&sci, &["ready", "--complexity", "mid", "--max-complexity", "mid"]);
    assert_eq!(list_ids(&v), [mid]);
    assert!(v["warnings"].as_array().unwrap().is_empty(), "{v}");

    let v = env.json(&sci, &["ready", "--complexity", "high", "--max-complexity", "mid"]);
    assert!(list_ids(&v).is_empty());
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(warnings, vec!["max-complexity mid: 1 above cutoff hidden"]);
}
```

If `write_claim`'s live claim needs a matching session environment to count as someone
else's, follow the pattern of the existing claim-omission tests in `tests/cli.rs` (search
for `omitted: claimed by session`) rather than inventing one.

Change the existing assertions:

1. In `ready_and_next_hide_above_cutoff_and_unassessed_with_counts`, replace everything from
   the comment "The cutoff composes with --size and --parallel, and its counts are the
   cutoff's alone" through the `assert_eq!(warnings, vec![…])` that follows
   `ready --max-complexity mid --size s` with the block below. The later `--parallel`
   assertions stay as they are.

```rust
    // The selection runs before the cutoff, so the cutoff's counts cover only selected
    // tasks (docs/specs/2026-09-30-task-filters-design.md, amending complexity §4.1):
    // only Low is size s, and Low is within the cutoff.
    env.json(&sci, &["edit", &low, "--size", "s", "--parallel"]);
    env.json(&sci, &["edit", &mid, "--size", "m"]);
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--size", "s"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    assert!(v["warnings"].as_array().unwrap().is_empty(), "{v}");
```

2. In the completion test that asserts
   `env.complete(&sci, "bash", 3, &["tasks", "ready", "--size", ""])`, change the expected
   list to `["xs", "s", "m", "l", "xl", "none"]`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `just test-one --test cli ready_`
Expected: the new tests fail (`-p`, `--tag`, and `--parent` are unknown on `ready`), and so
do the two edited assertions.

- [ ] **Step 3: Flatten `FilterArgs` into `ready`**

In `src/cli.rs`, in `Command::Ready`, delete the `size` and `parallel` fields and their
attributes and add `#[command(flatten)] filter: FilterArgs,` as the first field.

In `src/commands/mod.rs`:

```rust
        Command::Ready {
            filter,
            limit,
            max_complexity,
            scope,
        } => list::ready(open_read_ctx(dir, &scope)?, filter, limit, max_complexity),
```

- [ ] **Step 4: Filter at candidacy in `ready_tasks`**

In `src/commands/list.rs`, add a `filter: &TaskFilter` parameter to `ready_tasks`, after
`snapshots`. Build the selected candidates once and use them in both loops:

```rust
    let selected: Vec<&Task> = all
        .iter()
        .filter(|task| {
            is_candidate(task, now) && filter.matches(&Fields::of_task(task, claims, &ctx.registry))
        })
        .collect();
    for task in &selected {
        for dependency in &task.depends {
            // … unchanged dependency lookup …
        }
    }
    // …
    for task in selected {
        // … unchanged, minus the `is_candidate` check, which `selected` already applied …
    }
```

Dependency resolution still reads from `all`, which is unchanged.

In `ready`:

```rust
pub fn ready(
    mut ctx: ReadCtx,
    filter: FilterArgs,
    limit: Option<usize>,
    max_complexity: Option<String>,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let filter = TaskFilter::parse(&filter, &[], &ctx.registry)?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    check_parent(&filter, &all, |_| false)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let mut picked = ready_tasks(&mut ctx, &all, &claims, &snapshots, &filter, now)?;
    // … halts, retain_allowed, cutoff block unchanged …
    // Delete the `if let Some(size)` and `if parallel` retain blocks.
    // … deferred_omission, limit, output unchanged …
}
```

In `next` and `prime`, pass `&TaskFilter::default()` as the new argument. Remove the
now-unused `Size` import if clippy reports it.

- [ ] **Step 5: Amend the complexity design**

In `docs/specs/2026-09-12-task-complexity-design.md` §4.1, replace the `ready` bullet with:

```markdown
- `ready`: the selection filters (`--priority`, `--size`, `--complexity`, `--process`,
  `--tag`, `--owner`, `--source`, `--parent`, `--parallel`) run first, at candidacy
  (amended by `2026-09-30-task-filters-design.md`); the cutoff then removes tasks above
  the level and unassessed tasks, before `-n`. Its counts cover only selected tasks.
```

In its verification section, change "composes with `--size`, `--parallel`, `-n`" to
"composes with the selection filters and `-n`, counting only selected tasks".

- [ ] **Step 6: Change the ops inventory, then copy it**

In the ops worktree, replace the `tasks ready` row's `options` in `cli.toml` with:

```toml
options = [
  { names = ["--priority", "-p"], value = "enum", values = ["0", "1", "2", "3", "4"], repeatable = true },
  { names = ["--size"], value = "enum", values = ["xs", "s", "m", "l", "xl", "none"], repeatable = true },
  { names = ["--complexity"], value = "enum", values = ["low", "mid", "high", "none"], repeatable = true },
  { names = ["--process"], value = "enum", values = ["direct", "planned", "none"], repeatable = true },
  { shared = "tag", role = "filter", value = "string", repeatable = true },
  { names = ["--owner"], value = "string" },
  { names = ["--source"], value = "string" },
  { names = ["--parent"], value = "ref" },
  { names = ["--parallel"], value = "none" },
  { shared = "limit", value = "int" },
  { names = ["--max-complexity"], value = "enum", values = ["low", "mid", "high"] },
  { shared = "project", role = "select", value = "string", repeatable = false },
  { shared = "all_projects", value = "none" },
]
```

Run ops `just test-one test_cli`. Expected: PASS. Copy `cli.toml` to
`.worktrees/task-filters/tools/cli.toml` again.

- [ ] **Step 7: Run the focused tests**

Run: `just test-one --test cli ready_`, then `just test-one --test cli complet`, then
`just test-one surface`, then `just test-one --test cli next_` and
`just test-one --test cli prime`.
Expected: all PASS. A failure in `next_` or `prime` tests is a regression, not an
assertion to update.

- [ ] **Step 8: Run the fast suite and the check, then commit**

Run: `just test-fast` and `just check`. Expected: both pass.

```bash
git add src/cli.rs src/commands/mod.rs src/commands/list.rs tools/cli.toml tests/cli.rs docs/specs/2026-09-12-task-complexity-design.md
git commit -m "feat(ready): select with the shared task filter before the cutoff"
```

---

### Task 3: Documentation, review, and rollout

**Files:**
- Modify: `README.md` (the `tasks list` examples block near line 252)
- Modify: `skills/tasks/SKILL.md` (session protocol step 2, the `tasks list` paragraph)
- Modify: `docs/specs/2026-09-30-task-filters-design.md` (status line)
- Ops: commit `cli.toml`; publish vendor copies with `just vendor-cli`

**Interfaces:** No new code interface.

- [ ] **Step 1: Document the filters**

In `README.md`, add after the `tasks list --source …` example line:

```text
    tasks list -p 0 -p 1 --size s --size xs   # P0 or P1, and small or extra-small
    tasks ready --complexity none             # ready work nobody has rated yet
```

In `skills/tasks/SKILL.md`, after the sentence ending "`--sort created` for the most
recently touched or added first (`--reverse` flips it).", add:

```markdown
   `list` and `ready` also take `-p/--priority`, `--size`, `--complexity`, and `--process`,
   each repeatable: repeats of one of these widen (any of them), and different flags
   narrow (all of them). `--size`, `--complexity`, and `--process` accept `none` for an
   unset field; `--priority` does not, since every task has one. `--tag` stays all-of: a
   task must carry every tag given. `--owner`, `--source`, and `--parent` take one value,
   and `--parallel` is a switch. `--complexity` is a selection over the effective rating,
   not the session cutoff: a session under a cutoff still picks only through `ready` and
   `next`.
```

- [ ] **Step 2: File the `--tag` follow-up**

```bash
tasks add "Decide whether repeated --tag filters widen, as the CLI vocabulary says" --status idea --tag cli -b "list and ready keep --tag all-of (tasks-964c95 left it); the shared vocabulary says repeatable filters widen. Decide and change or record an exception on the cli.toml rows."
```

Add a note on tasks-964c95 naming the returned id.

- [ ] **Step 3: Gate and request review**

Run: `just test-fast`, `just check`, and `git diff --check main...`. Expected: clean. Commit
the docs with `docs: describe the list and ready filters`. Then dispatch one implementation
review of the whole branch against the spec and this plan, and record it as
`review: impl round <n> — …` on tasks-964c95 before acting on it.

- [ ] **Step 4: Reconcile the ops worktree with ops main, then refresh the task worktree**

This happens after the review accepts and before tasks integrates, all in the two
worktrees. `vendor-cli --force` publishes the ops worktree's files as they are, so that
worktree must carry everything ops `main` has. The ops branch has no commits at this point.
Its `cli.toml` change is uncommitted, because ops pre-commit refuses the source until the
copies are published (Step 7). So reconcile the base under the uncommitted edit instead of
rebasing:

```bash
# in the ops worktree
git stash push -- cli.toml
git merge --ff-only main
git stash pop
```

If `stash pop` conflicts on `cli.toml`, main has changed the same rows. Stop, resolve it by
keeping main's content plus this plan's two row changes, and rerun ops
`just test-one test_cli`. Do not pull from `origin` here: the local ops `main` is the
branch this rollout merges into (Step 8). Then confirm, in the ops worktree:

```bash
git diff main --stat                       # names cli.toml only
git diff main -- cli.toml                  # only the tasks list and tasks ready rows
git diff --quiet main -- bin/cli_surface.py && echo helper-unchanged
```

Refresh the task worktree from the reconciled source and retest there:

```bash
cp <ops worktree>/cli.toml .worktrees/task-filters/tools/cli.toml
cd .worktrees/task-filters && just test-one surface && just test-fast && just check
```

If `tools/cli.toml` changed, commit it in the task worktree with
`chore(tools): take the reconciled CLI inventory`.

- [ ] **Step 5: Integrate tasks**

In the task worktree, mark the spec status "implemented" and the plan status "executed",
then `tasks done tasks-964c95 "<what landed>"`, and commit them together. From the main
checkout, fast-forward `main` to `feat/task-filters`. If `main` has moved, rebase the branch
in the task worktree first, rerun `just test-fast` there, then fast-forward. Run
`cargo install --path .` and `tasks check` from the main checkout. Then compare the
registered copy with the source:
`cmp <tasks main checkout>/tools/cli.toml <ops worktree>/cli.toml` must report no
difference.

- [ ] **Step 6: Preflight every destination by content, then publish**

`vendor-cli` writes `tools/cli.toml` and `tools/cli_surface.py` into every registered
project that has either file. `vendored check` compares contents but not git state, and a
clean `git status` does not prove the committed files equal the source. So check both
before publishing. Take the roots from `tasks projects --paths`. For each root that has
`tools/cli.toml` or `tools/cli_surface.py`, all of the following must hold:

```bash
git -C <root> status --porcelain -- tools/cli.toml tools/cli_surface.py   # empty
cmp <root>/tools/cli_surface.py <ops worktree>/bin/cli_surface.py         # equal
git -C <ops worktree> show main:cli.toml | cmp - <root>/tools/cli.toml    # equal
```

The last check says the destination holds exactly ops `main`'s inventory. Step 4 proved
that the source differs from `main` only by the two rows, so publishing changes only those
two rows. The one exception is the tasks main checkout: it must equal the new source
instead (checked in Step 5). If any root fails any check, stop and report the root, the
file, and the difference to the user. Never publish over it.

Then, from the ops worktree, run `just vendor-cli --force`. Afterwards, for every
destination except tasks:

```bash
git -C <root> diff --stat -- tools/cli.toml tools/cli_surface.py   # names tools/cli.toml only
git -C <root> diff -- tools/cli.toml                               # only the two tasks rows
```

For tasks, the same two commands print nothing. Run ops `just test-one test_cli`,
`just test-fast`, and `just check`.

- [ ] **Step 7: Commit ops and the copies**

Commit ops `cli.toml` with `feat(cli): tasks list and ready filter flags`. In each other
project whose `tools/cli.toml` changed, stage only that path
(`git -C <root> add -- tools/cli.toml`) and commit with `chore(tools): sync CLI vocabulary`.
Record those commits in a note on tasks-964c95.

- [ ] **Step 8: Merge ops and clean up**

Fast-forward ops `main` to `feat/task-filters-cli`. If ops `main` moved after Step 4, stop
and repeat Steps 4 and 6 for the new base before merging. Run `just check-vendored` from ops
main; expected: silent. In both worktrees, run `tt-report`. Check that no host pointer
resolves into them (`readlink -f ~/bin/* ~/.local/bin/* | grep -F .worktrees/task-filters`).
Then run `git worktree unlock` and `git worktree remove` on each, and delete the merged
branches.
