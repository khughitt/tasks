# Task Hierarchy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One nestable task type: a `parent` field, subtree-aware closing and readiness, a `tree` view, roadmap and close-out sections in `prime`, and a reverse drift check for plan headings.

**Architecture:** The tree logic lives in one new module, `src/hierarchy.rs`, that works on `&[Task]` slices the way `query.rs` does, so commands scan once and pass the slice. Validation of a `parent` on write mirrors how `depends` is validated in `apply_fields` and the editor path. Output shapes only gain fields; nothing existing changes meaning.

**Tech Stack:** Rust 2024 edition, clap derive, serde, the repo's own frontmatter parser. Tests: unit tests in-module plus end-to-end tests in `tests/cli.rs` through `assert_cmd`.

**Spec:** `docs/specs/2026-09-03-task-hierarchy-design.md` (read it first; every task below cites its sections).

## Global Constraints

- Gates before every commit: `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`.
- Rebuild and reinstall after CLI changes so the tracker used for `tasks done` is the code under test: `cargo install --path .`
- JSON output is the contract. Every field listed in spec §6 is always present; optional values are `null`. No other shape changes.
- Fail early with a typed `Error`; no silent fallbacks.
- `parent` is same-project only. Nesting depth is unbounded; every rule is stated in terms of descendants (spec §2.2).
- Error kinds introduced here: `open_descendants` (closing refused), `cycle` (parent loop, existing kind). Check finding kinds: `dangling_parent`, `foreign_parent`, `parent_cycle` (errors); `open_child_of_closed_parent`, `unlinked_step` (warnings).
- Each task below is one tracker task; close it with `tasks done <id> "<what landed>"` in the commit that lands it. The ids are given per task.
- Conventional commits, no AI-attribution trailers.

---

### Task 1: Add the parent field with validation and cycle check

Tracker: `tasks-c80832`. Spec §2, §3, §4.1.

**Files:**
- Create: `src/hierarchy.rs`
- Modify: `src/main.rs` (add `mod hierarchy;`), `src/model.rs:213-229` (Task struct), `src/format.rs` (KEYS, parse, serialize, tests), `src/cli.rs` (FieldArgs, Edit), `src/commands/mod.rs` (apply_fields, dispatch), `src/commands/edit.rs` (flags), `src/commands/add.rs` (Task literal), `src/commands/check.rs`, `src/repo.rs` (write_task, test literal), `src/query.rs` (test literal)
- Also, if the feedback plan has landed first: `src/commands/feedback.rs` (`create` literal) and `src/similarity.rs` (test literal) gain `parent: None,`. Whichever plan lands second makes the other compile; never merge a branch that does not build.
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `Task.parent: Option<TaskId>`; `hierarchy::validate_parent(project: &Project, task: &Task) -> Result<()>`, called by `Project::write_task` so every write path is covered; `hierarchy::parent_cycle(tasks: &[Task], start: &TaskId) -> Option<Vec<TaskId>>`; `edit::run(..., no_parent: bool, ...)`.

- [ ] **Step 1: Write the failing e2e tests**

Add to `tests/cli.rs` after `fn write_doc`:

```rust
fn id_of(value: serde_json::Value) -> String {
    value["id"].as_str().unwrap().to_string()
}

#[test]
fn parent_is_validated_persisted_and_clearable() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.init("fam");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &goal]));
    let raw = env.read(&dir, &format!("tasks/{child}.md"));
    assert!(
        raw.contains(&format!("depends: []\nparent: {goal}\ntags: []\n")),
        "{raw}"
    );
    assert_eq!(env.json(&dir, &["show", &child])["task"]["parent"], goal);
    assert_eq!(
        env.json(&dir, &["show", &goal])["task"]["parent"],
        serde_json::Value::Null
    );

    assert_eq!(
        env.fail(&dir, &["add", "x", "--parent", "sci-ffffff"]),
        "unresolvable_id"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--parent", "fam-000001"]),
        "validation"
    );
    assert_eq!(env.fail(&dir, &["edit", &goal, "--parent", &child]), "cycle");
    assert_eq!(env.fail(&dir, &["edit", &goal, "--parent", &goal]), "cycle");
    let grandchild = id_of(env.json(&dir, &["add", "Grandchild", "--parent", &child]));
    assert_eq!(
        env.fail(&dir, &["edit", &goal, "--parent", &grandchild]),
        "cycle"
    );
    let files = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension().is_some_and(|x| x == "md"))
        .count();
    assert_eq!(files, 3, "rejected adds wrote nothing: goal, child, grandchild only");

    env.json(&dir, &["edit", &child, "--no-parent"]);
    assert_eq!(
        env.json(&dir, &["show", &child])["task"]["parent"],
        serde_json::Value::Null
    );
}

#[test]
fn editor_path_validates_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child"]));
    let set = editor_script(
        &dir,
        &format!("sed -i 's/^depends: \\[\\]$/depends: []\\nparent: {goal}/' \"$1\""),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &set)
        .args(["edit", &child])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(env.json(&dir, &["show", &child])["task"]["parent"], goal);

    let loop_back = editor_script(
        &dir,
        &format!("sed -i 's/^depends: \\[\\]$/depends: []\\nparent: {child}/' \"$1\""),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &loop_back)
        .args(["edit", &goal])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "cycle");
}

#[test]
fn check_reports_parent_problems() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    let c = id_of(env.json(&dir, &["add", "C"]));
    let d = id_of(env.json(&dir, &["add", "D"]));
    let e = id_of(env.json(&dir, &["add", "E"]));
    let f = id_of(env.json(&dir, &["add", "F", "--parent", &a]));
    // a -> b loop with f as a tail into it, c dangling, d foreign, e its own parent;
    // written by hand to simulate drift
    let set_parent = |id: &str, parent: &str| {
        let raw = env.read(&dir, &format!("tasks/{id}.md"));
        std::fs::write(
            dir.join(format!("tasks/{id}.md")),
            raw.replace("depends: []\n", &format!("depends: []\nparent: {parent}\n")),
        )
        .unwrap();
    };
    set_parent(&a, &b);
    set_parent(&c, "sci-ffffff");
    set_parent(&d, "fam-000001");
    set_parent(&e, &e);
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<(String, String)> = check["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["kind"].as_str().unwrap().to_string(),
                f["id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(kinds.contains(&("parent_cycle".into(), a.clone().min(b.clone()))), "{check}");
    assert!(kinds.contains(&("dangling_parent".into(), c.clone())), "{check}");
    assert!(kinds.contains(&("foreign_parent".into(), d.clone())), "{check}");
    assert!(kinds.contains(&("parent_cycle".into(), e.clone())), "self-edge: {check}");
    assert_eq!(
        kinds.iter().filter(|(kind, _)| kind == "parent_cycle").count(),
        2,
        "each cycle is reported once, at its lowest member, even with the tail f: {check}"
    );
    assert!(!kinds.iter().any(|(_, id)| id == &f), "the tail is not a cycle member: {check}");
    assert!(
        !kinds.iter().any(|(kind, _)| kind == "parse"),
        "a self-edge is a hierarchy finding, not a parse error: {check}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run each (cargo accepts one filter before `--`):

```bash
cargo test --test cli parent_is_validated_persisted_and_clearable
cargo test --test cli editor_path_validates_parent
cargo test --test cli check_reports_parent_problems
```

Expected: FAIL (clap rejects `--parent`; the test for check fails on missing finding kinds).

- [ ] **Step 3: Add the field to the model and the file format**

`src/model.rs`, in `pub struct Task` after `pub depends: Vec<TaskId>,`:

```rust
    pub parent: Option<TaskId>,
```

`src/format.rs`:

```rust
const KEYS: [&str; 14] = [
    "id", "title", "status", "priority", "size", "owner", "created", "updated", "depends",
    "parent", "tags", "spec", "plan", "step",
];
```

In `parse_task`, in the `Task { ... }` literal after `depends,`:

```rust
        parent: scalar("parent")?
            .map(|p| TaskId::parse(&p))
            .transpose()
            .map_err(|e| perr(file, e.to_string()))?,
```

`validate_task` does not learn any parent rule. Self-parent, foreign parent, missing parent, and loops are all hierarchy facts, and `hierarchy::validate_parent` (Step 4) owns them so that the editor path reports `cycle` and `check` reports `parent_cycle`, never `parse`. A file with `parent` equal to its own id therefore parses; it is `write_task` and `check` that reject it.

In `serialize_task`, replace the `pairs.extend([...])` block so `parent` follows `depends`:

```rust
    pairs.extend([
        (String::from("created"), Value::Raw(t.created.clone())),
        (String::from("updated"), Value::Raw(t.updated.clone())),
        (
            String::from("depends"),
            Value::List(t.depends.iter().map(ToString::to_string).collect()),
        ),
    ]);
    if let Some(parent) = &t.parent {
        pairs.push(("parent".into(), s(&parent.to_string())));
    }
    pairs.push((String::from("tags"), Value::List(t.tags.clone())));
```

Add `parent: None,` to every `Task { ... }` literal: `src/commands/add.rs`, the `sample` helper in `src/repo.rs` tests, and the `t` helper in `src/query.rs` tests.

Add a unit test in `src/format.rs` tests:

```rust
    #[test]
    fn parent_roundtrips_after_depends() {
        let with_parent = MINIMAL.replace("depends: []", "depends: []\nparent: sci-000002");
        let t = parse_task(&with_parent, "x").unwrap();
        assert_eq!(t.parent.as_ref().unwrap().to_string(), "sci-000002");
        assert_eq!(serialize_task(&t), with_parent);
        let own = MINIMAL.replace("depends: []", "depends: []\nparent: sci-000001");
        assert!(parse_task(&own, "x").is_ok(), "self-parent is a hierarchy rule, not a format rule");
    }
```

- [ ] **Step 4: Create `src/hierarchy.rs` with parent validation**

```rust
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::collections::HashMap;

/// Rejects a `parent` that is foreign, missing, or would make `task` its own ancestor.
/// Reads ancestors from disk, so it is the write-path check; `check` uses `parent_cycle`.
pub fn validate_parent(project: &Project, task: &Task) -> Result<()> {
    let Some(parent) = &task.parent else {
        return Ok(());
    };
    if parent.prefix != project.prefix {
        return Err(Error::Validation(format!(
            "parent {parent} must be in this project ({})",
            project.prefix
        )));
    }
    if parent == &task.id {
        return Err(Error::Cycle(format!("{parent} -> {parent}")));
    }
    if !project.task_path(parent).is_file() {
        return Err(Error::UnresolvableId(parent.to_string()));
    }
    let mut path = vec![task.id.clone()];
    let mut current = Some(parent.clone());
    while let Some(id) = current {
        if path.contains(&id) {
            path.push(id);
            return Err(Error::Cycle(join(&path)));
        }
        let ancestor = project.read_task(&id)?;
        path.push(id);
        current = ancestor.parent;
    }
    Ok(())
}

/// Walks the parent chain upward from `start` and returns the loop it runs into, if any,
/// as `a -> b -> a`: only the cycle, never the tail that led into it. The path starts at
/// whichever member the walk enters first (`b -> a -> b` from `b`), so callers that
/// deduplicate must key on the set of members, as `check` does, not on the order. Missing
/// parents end the walk without a cycle.
pub fn parent_cycle(tasks: &[Task], start: &TaskId) -> Option<Vec<TaskId>> {
    let parents: HashMap<&TaskId, &TaskId> = tasks
        .iter()
        .filter_map(|task| task.parent.as_ref().map(|parent| (&task.id, parent)))
        .collect();
    let mut path = vec![start.clone()];
    let mut current = parents.get(start).copied();
    while let Some(id) = current {
        if let Some(position) = path.iter().position(|item| item == id) {
            let mut cycle = path[position..].to_vec();
            cycle.push(id.clone());
            return Some(cycle);
        }
        path.push(id.clone());
        current = parents.get(id).copied();
    }
    None
}

fn join(path: &[TaskId]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    pub(crate) fn task(id: &str, parent: Option<&str>, status: Status) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: id.into(),
            status,
            priority: 2,
            size: None,
            owner: None,
            created: "2026-09-03T00:00:00Z".into(),
            updated: "2026-09-03T00:00:00Z".into(),
            depends: vec![],
            parent: parent.map(|p| TaskId::parse(p).unwrap()),
            tags: vec![],
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn parent_cycle_finds_loops_of_any_length_and_ignores_chains() {
        let a = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b = task("xx-000002", Some("xx-000003"), Status::Todo);
        let c = task("xx-000003", Some("xx-000001"), Status::Todo);
        let cycle = parent_cycle(&[a.clone(), b.clone(), c], &a.id).unwrap();
        assert_eq!(cycle.len(), 4);
        assert_eq!(cycle[0], cycle[3]);
        let root = task("xx-000003", None, Status::Todo);
        assert!(parent_cycle(&[a.clone(), b, root], &a.id).is_none());
        let dangling = task("xx-000009", Some("xx-000008"), Status::Todo);
        assert!(parent_cycle(std::slice::from_ref(&dangling), &dangling.id).is_none());
        let own = task("xx-000007", Some("xx-000007"), Status::Todo);
        assert_eq!(parent_cycle(std::slice::from_ref(&own), &own.id).unwrap().len(), 2);
        // a tail entering a loop reports only the loop: C -> A -> B -> A yields
        // A -> B -> A from C, the same as from A; from B the same members in B's order
        let tail = task("xx-000005", Some("xx-000001"), Status::Todo);
        let a2 = task("xx-000001", Some("xx-000002"), Status::Todo);
        let b2 = task("xx-000002", Some("xx-000001"), Status::Todo);
        let all = [tail.clone(), a2.clone(), b2.clone()];
        let ids = |path: &[TaskId]| path.iter().map(ToString::to_string).collect::<Vec<_>>();
        let from_tail = parent_cycle(&all, &tail.id).unwrap();
        assert_eq!(ids(&from_tail), ["xx-000001", "xx-000002", "xx-000001"]);
        assert_eq!(ids(&parent_cycle(&all, &a2.id).unwrap()), ids(&from_tail));
        let from_b = parent_cycle(&all, &b2.id).unwrap();
        assert_eq!(ids(&from_b), ["xx-000002", "xx-000001", "xx-000002"]);
        let members = |path: &[TaskId]| {
            let mut m: Vec<String> = ids(&path[..path.len() - 1]);
            m.sort();
            m
        };
        assert_eq!(members(&from_b), members(&from_tail), "same set, so check dedupes");
    }
}
```

Add `mod hierarchy;` to `src/main.rs` in alphabetical position (after `mod frontmatter;`).

Hook the validation into the writers so that every command that writes a task, including `note`, `start`, `done`, `dep`, unrelated `edit` flags, and the feedback plan's direct writes, refuses a dangling, foreign, or cyclic parent. `src/repo.rs`:

```rust
    pub fn write_task(&self, task: &Task) -> Result<()> {
        crate::hierarchy::validate_parent(self, task)?;
        atomic_write(&self.task_path(&task.id), serialize_task(task).as_bytes())
    }
```

There are two writers if the feedback plan has landed first: its Task 1 adds `Project::create_task` (exclusive creation, used by `add` and by the feedback command) with `create_task_with` behind it. Add the same `crate::hierarchy::validate_parent(self, task)?;` as the first statement of `create_task_with`, before its id loop. Parent validity does not depend on the new id, since a fresh id is never an existing task and so never the parent. If the feedback plan has not landed, `add` still writes through `save` and `write_task`, and the feedback plan adds the call itself when it lands second. The e2e test above (`add x --parent sci-ffffff` is `unresolvable_id`) fails loudly if either writer is missed.

Add a unit test in `src/repo.rs` tests:

```rust
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
```

- [ ] **Step 5: Wire the flags and the write paths**

`src/cli.rs`, in `FieldArgs` after `step`:

```rust
    /// Make this task part of another task (same project).
    #[arg(long)]
    pub parent: Option<String>,
```

In `Command::Edit` after `force`:

```rust
        /// Detach from the parent.
        #[arg(long, conflicts_with = "parent")]
        no_parent: bool,
```

`src/commands/mod.rs`, in `apply_fields` after the `depends` block and before the `spec` block:

```rust
    if let Some(parent) = &fields.parent {
        task.parent = Some(TaskId::parse(parent)?);
    }
```

No validation call here: `save` ends in `write_task`, which validates (Step 4), and doing it once keeps the rule in one place.

In `run`, the `Command::Edit` arm gains `no_parent` in both the pattern and the call: `edit::run(open_ctx(dir)?, id, title, status, force, no_parent, fields)`.

`src/commands/edit.rs`: add `no_parent: bool` to `run` between `force` and `mut fields`; add `|| fields.parent.is_some() || no_parent` to `has_flags`; after loading the task and before `apply_fields`:

```rust
    if no_parent {
        task.parent = None;
    }
```

The editor path needs no extra call either: it ends in `save(&ctx, &mut edited).map_err(keep)?`, so a bad parent comes back as `cycle`, `unresolvable_id`, or `validation` with the "edit kept at" suffix like any other rejection.

- [ ] **Step 6: Add the check findings**

`src/commands/check.rs`, inside the `for task in &tasks` loop after the dependency loop:

```rust
        if let Some(parent) = &task.parent {
            if parent.prefix != ctx.project.prefix {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "foreign_parent",
                    format!("parent {parent} is not in this project"),
                ));
            } else if !ctx.project.task_path(parent).try_exists()? {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "dangling_parent",
                    format!("parent {parent} does not exist"),
                ));
            }
        }
```

After the dependency-cycle loop (before `Ok(Output::Check(...))`):

```rust
    let mut seen_parent_cycles = BTreeSet::new();
    for task in &tasks {
        if let Some(cycle) = crate::hierarchy::parent_cycle(&tasks, &task.id) {
            let mut key: Vec<String> = cycle[..cycle.len() - 1]
                .iter()
                .map(ToString::to_string)
                .collect();
            key.sort();
            if seen_parent_cycles.insert(key) {
                let lowest = cycle[..cycle.len() - 1].iter().min().unwrap();
                let path: Vec<String> = cycle.iter().map(ToString::to_string).collect();
                errors.push(Finding {
                    id: Some(lowest.to_string()),
                    file: format!("tasks/{lowest}.md"),
                    kind: "parent_cycle".into(),
                    detail: path.join(" -> "),
                });
            }
        }
    }
```

A self-edge is a two-element path from `parent_cycle` and is reported as `parent_cycle` like any longer loop; the e2e test asserts no `parse` finding appears for it.

- [ ] **Step 7: Run all gates**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```
Expected: all pass, including the three new e2e tests and the two new unit tests.

- [ ] **Step 8: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-c80832 "parent field with prefix/existence/cycle validation on add, edit, and editor path; check reports dangling_parent, foreign_parent, parent_cycle"
tasks check
git add -A
git commit -m "feat: add parent field with validation and cycle check"
```

---

### Task 2: Open-descendant counting, closing rules, and ready excludes parents

Tracker: `tasks-91e336`. Spec §2.2, §4.2, §4.3 (ready), §6 (`TaskSummary`).

**Files:**
- Modify: `src/hierarchy.rs`, `src/error.rs`, `src/commands/mod.rs` (transition), `src/output.rs` (TaskSummary), `src/query.rs` (is_ready), `src/commands/list.rs`, `src/commands/check.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Task.parent` from Task 1.
- Produces: `hierarchy::children<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task>`; `hierarchy::open_descendants<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task>`; `TaskSummary::of(task: &Task, all: &[Task]) -> TaskSummary` (replaces `From<&Task>`); `query::is_ready(task, has_children: bool, lookup)`; `Error::OpenDescendants(String, String)` with kind `open_descendants`.

- [ ] **Step 1: Write the failing e2e tests**

```rust
#[test]
fn closing_rules_walk_descendants() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    let c = id_of(env.json(&dir, &["add", "C", "--parent", &b]));
    assert_eq!(env.fail(&dir, &["done", &a]), "open_descendants");
    assert_eq!(env.fail(&dir, &["drop", &a]), "open_descendants");
    env.json(&dir, &["done", &b, "forced past c", "--force"]);
    assert_eq!(
        env.fail(&dir, &["done", &a]),
        "open_descendants",
        "c is still open under the force-closed b"
    );
    let out = env.cmd(&dir).args(["drop", &a, "--force"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "drop has no override flag");
    env.json(&dir, &["done", &c]);
    env.json(&dir, &["done", &a]);
    assert_eq!(env.json(&dir, &["show", &a])["task"]["status"], "done");
}

#[test]
fn ready_never_lists_a_task_with_children_and_summaries_carry_counts() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let leaf = id_of(env.json(&dir, &["add", "Leaf", "--parent", &goal]));
    let ready = env.json(&dir, &["ready"]);
    let ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [leaf.as_str()]);
    let list = env.json(&dir, &["list"]);
    let goal_row = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == goal)
        .unwrap();
    assert_eq!(goal_row["parent"], serde_json::Value::Null);
    assert_eq!(goal_row["child_count"], 1);
    assert_eq!(goal_row["open_descendant_count"], 1);
    let leaf_row = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == leaf)
        .unwrap();
    assert_eq!(leaf_row["parent"], goal);
    assert_eq!(leaf_row["child_count"], 0);
    env.json(&dir, &["done", &leaf]);
    let ready = env.json(&dir, &["ready"]);
    assert_eq!(ready["tasks"].as_array().unwrap().len(), 0, "a parent is never ready");
}

#[test]
fn check_warns_on_open_child_of_closed_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    env.json(&dir, &["done", &b]);
    env.json(&dir, &["done", &a]);
    env.json(&dir, &["edit", &b, "--status", "todo"]);
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warning = &check["warnings"].as_array().unwrap()[0];
    assert_eq!(warning["kind"], "open_child_of_closed_parent");
    assert_eq!(warning["id"], b);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run each:

```bash
cargo test --test cli closing_rules_walk_descendants
cargo test --test cli ready_never_lists
cargo test --test cli check_warns_on_open_child
```

Expected: FAIL (`done a` succeeds today; `child_count` missing).

- [ ] **Step 3: Add subtree helpers to `src/hierarchy.rs`**

```rust
/// Direct children of `id`, in the order they appear in `tasks`.
pub fn children<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|task| task.parent.as_ref() == Some(id))
        .collect()
}

/// Every task below `id`, depth first. A visited set makes a corrupt loop terminate.
pub fn descendants<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    let mut out = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<&TaskId> = vec![id];
    while let Some(current) = stack.pop() {
        for child in children(tasks, current) {
            if visited.insert(&child.id) {
                out.push(child);
                stack.push(&child.id);
            }
        }
    }
    out
}

pub fn open_descendants<'a>(tasks: &'a [Task], id: &TaskId) -> Vec<&'a Task> {
    descendants(tasks, id)
        .into_iter()
        .filter(|task| task.status.is_open())
        .collect()
}
```

Unit test, in the same `tests` module:

```rust
    #[test]
    fn open_descendants_see_through_a_closed_middle_node() {
        let a = task("xx-000001", None, Status::Todo);
        let b = task("xx-000002", Some("xx-000001"), Status::Done);
        let c = task("xx-000003", Some("xx-000002"), Status::Todo);
        let all = [a.clone(), b, c];
        assert_eq!(children(&all, &a.id).len(), 1);
        assert_eq!(descendants(&all, &a.id).len(), 2);
        let open: Vec<String> = open_descendants(&all, &a.id)
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(open, ["xx-000003"]);
    }
```

- [ ] **Step 4: The open-work rule in `transition`**

`src/error.rs`: add a variant after `OpenDependencies`:

```rust
    #[error("{0} has open descendants: {1}")]
    OpenDescendants(String, String),
```

Add it to `with_suffix` (`Error::OpenDescendants(id, detail) => Error::OpenDescendants(id, detail + suffix),`) and to `kind` (`Error::OpenDescendants(..) => "open_descendants",`).

`src/commands/mod.rs`, in `transition`, after the existing open-deps block:

```rust
    let closing = matches!(to, Status::Done | Status::Dropped) && task.status != to;
    if closing && !(force && to == Status::Done) {
        let all = ctx.project.scan()?;
        let open: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id)
            .iter()
            .map(|task| task.id.to_string())
            .collect();
        if !open.is_empty() {
            return Err(Error::OpenDescendants(task.id.to_string(), open.join(", ")));
        }
    }
```

- [ ] **Step 5: Summaries with counts, and ready excludes parents**

`src/output.rs`: replace `impl From<&Task> for TaskSummary` with:

```rust
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
```

and add the three fields to the struct after `depends`:

```rust
    pub parent: Option<String>,
    pub child_count: usize,
    pub open_descendant_count: usize,
```

`src/query.rs`:

```rust
/// `lookup` returns Some(closed?) for a reachable dependency, None if unreachable.
/// A task with children is a goal, not work, and is never ready.
pub fn is_ready(task: &Task, has_children: bool, lookup: &dyn Fn(&TaskId) -> Option<bool>) -> bool {
    task.status == Status::Todo
        && !has_children
        && task.depends.iter().all(|d| lookup(d) == Some(true))
}
```

Update the unit test `ready_requires_todo_and_closed_deps` to pass `false` as the new argument in each call and add:

```rust
        assert!(!is_ready(&a, true, &closed_all), "parents are never ready");
```

`src/commands/list.rs`: every `TaskSummary::from` becomes `TaskSummary::of(task, &all)`. In `list`, bind `let all = tasks.clone();` immediately **after** the `--all-projects` extension loop and before `tasks.retain`, so that foreign rows get their counts from their own project's tasks (the extension pulls in each project's full scan, and `parent` never crosses projects, so counting within the combined set is exact). In `ready_tasks`, the call becomes:

```rust
        let has_children = !crate::hierarchy::children(all, &task.id).is_empty();
        if is_ready(task, has_children, &lookup) {
```

`ready` and `prime` already have `all` in scope; use `TaskSummary::of(task, &all)`.

- [ ] **Step 6: Check warning**

`src/commands/check.rs`, in the per-task loop after the parent existence checks from Task 1:

```rust
        if let Some(parent) = &task.parent
            && task.status.is_open()
            && let Some(parent_task) = tasks.iter().find(|candidate| &candidate.id == parent)
            && !parent_task.status.is_open()
        {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "open_child_of_closed_parent",
                format!("open under {} which is {}", parent, parent_task.status.as_str()),
            ));
        }
```

- [ ] **Step 7: Run all gates**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```
Expected: all pass. The existing `prime_reports_counts_ready_and_doing` and `ready_excludes_ideas_doing_and_open_deps` tests still pass because they use no parents.

- [ ] **Step 8: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-91e336 "descendant walk; done/drop refuse with open descendants (done --force overrides); ready is leaves only; TaskSummary gains parent, child_count, open_descendant_count; check warns open_child_of_closed_parent"
tasks check
git add -A
git commit -m "feat: subtree-aware closing rules and readiness"
```

---

### Task 3: Add tree command, show parent/children, list --parent

Tracker: `tasks-afe69c`. Spec §4.4, §6.

**Files:**
- Modify: `src/hierarchy.rs`, `src/query.rs` (expose the ready comparator), `src/output.rs`, `src/cli.rs`, `src/commands/mod.rs` (dispatch), `src/commands/list.rs`, `src/commands/show.rs`
- Create: `src/commands/tree.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `hierarchy::children`, `open_descendants`, `TaskSummary::of` from Task 2.
- Produces: `output::TreeNode { summary: TaskSummary (flattened), children: Vec<TreeNode> }`; `output::Related { id, title, status }`; `hierarchy::forest(all: &[Task], root: Option<&TaskId>, include_closed: bool) -> Vec<TreeNode>`; `query::ready_order(a: &Task, b: &Task) -> Ordering`; `Output::Tree(TreeOut)`.

- [ ] **Step 1: Write the failing e2e tests**

```rust
#[test]
fn tree_nests_prunes_and_orders() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "1"]));
    let big = id_of(env.json(&dir, &["add", "Big", "--parent", &goal, "--size", "l"]));
    let small = id_of(env.json(&dir, &["add", "Small", "--parent", &goal, "--size", "xs"]));
    let deep = id_of(env.json(&dir, &["add", "Deep", "--parent", &big]));
    let loner = id_of(env.json(&dir, &["add", "Loner", "-p", "3"]));
    env.json(&dir, &["done", &small]);

    let tree = env.json(&dir, &["tree"]);
    let nodes = tree["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["id"], goal);
    assert_eq!(nodes[1]["id"], loner);
    let goal_children = nodes[0]["children"].as_array().unwrap();
    assert_eq!(goal_children.len(), 1, "closed Small is pruned: {tree}");
    assert_eq!(goal_children[0]["id"], big);
    assert_eq!(goal_children[0]["children"][0]["id"], deep);
    assert_eq!(nodes[0]["child_count"], 2);
    assert_eq!(nodes[0]["open_descendant_count"], 2);

    let all = env.json(&dir, &["tree", "--all"]);
    let goal_children = all["nodes"][0]["children"].as_array().unwrap();
    let ids: Vec<&str> = goal_children.iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [small.as_str(), big.as_str()], "ready order: xs before l");

    let sub = env.json(&dir, &["tree", &big]);
    assert_eq!(sub["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(sub["nodes"][0]["id"], big);
    assert_eq!(env.fail(&dir, &["tree", "sci-ffffff"]), "task_not_found");

    // an open task under a closed parent stays visible with its closed ancestor
    env.json(&dir, &["done", &deep]);
    env.json(&dir, &["done", &big]);
    env.json(&dir, &["edit", &deep, "--status", "todo"]);
    let tree = env.json(&dir, &["tree"]);
    let big_node = &tree["nodes"][0]["children"][0];
    assert_eq!(big_node["id"], big);
    assert_eq!(big_node["status"], "done");
    assert_eq!(big_node["children"][0]["id"], deep);

    let out = env.cmd(&dir).args(["--pretty", "tree"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("{goal}  P1")), "{text}");
    assert!(text.contains(&format!("  {big}  P2")), "children are indented: {text}");
}

#[test]
fn show_reports_parent_and_children_and_list_filters_by_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    // priorities pin the order: children are reported in ready order, not id order
    let two = id_of(env.json(&dir, &["add", "Two", "--parent", &goal, "-p", "2"]));
    let one = id_of(env.json(&dir, &["add", "One", "--parent", &goal, "-p", "1"]));
    let other = id_of(env.json(&dir, &["add", "Other"]));
    let shown = env.json(&dir, &["show", &one]);
    assert_eq!(shown["parent"]["id"], goal);
    assert_eq!(shown["parent"]["title"], "Goal");
    assert_eq!(shown["parent"]["status"], "todo");
    assert_eq!(shown["children"], serde_json::json!([]));
    let shown = env.json(&dir, &["show", &goal]);
    assert_eq!(shown["parent"], serde_json::Value::Null);
    let kids: Vec<&str> = shown["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(kids, [one.as_str(), two.as_str()]);
    let filtered = env.json(&dir, &["list", "--parent", &goal]);
    let ids: Vec<&str> = filtered["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&other.as_str()));
    assert_eq!(env.fail(&dir, &["list", "--parent", "sci-ffffff"]), "task_not_found");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run each:

```bash
cargo test --test cli tree_nests_prunes_and_orders
cargo test --test cli show_reports_parent_and_children
```

Expected: FAIL (`tree` is an unknown subcommand, exit 2).

- [ ] **Step 3: Output shapes**

`src/output.rs`:

```rust
#[derive(Serialize)]
pub struct Related {
    pub id: String,
    pub title: String,
    pub status: String,
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
```

`ShowOut` gains, after `depends_on`:

```rust
    pub parent: Option<Related>,
    pub children: Vec<Related>,
```

`Output` gains `Tree(TreeOut)`. In `pretty`: `Output::Tree(o) => tree_text(&o.nodes, 0)` with:

```rust
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
```

In the `Output::Show` pretty arm, after the depends-on footer:

```rust
            if let Some(parent) = &o.parent {
                rendered.push_str(&format!(
                    "\n# parent\n- {} [{}] {}\n",
                    parent.id, parent.status, parent.title
                ));
            }
            if !o.children.is_empty() {
                rendered.push_str("\n# children\n");
                for child in &o.children {
                    rendered.push_str(&format!("- {} [{}] {}\n", child.id, child.status, child.title));
                }
            }
```

`warnings_of` gains `Output::Tree(o) => o.warnings.clone(),`.

- [ ] **Step 4: Forest builder and the ready comparator**

`src/query.rs`: extract the comparator so `sort_ready` and the forest share it:

```rust
pub fn ready_order(a: &Task, b: &Task) -> Ordering {
    a.priority
        .cmp(&b.priority)
        .then_with(|| match (a.size, b.size) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| a.created.cmp(&b.created))
        .then_with(|| a.id.cmp(&b.id))
}

pub fn sort_ready(tasks: &mut [Task]) {
    tasks.sort_by(ready_order);
}
```

`src/hierarchy.rs`:

```rust
use crate::output::{TaskSummary, TreeNode};
use crate::query::ready_order;

/// The forest under `root` (or every root when `None`). Without `include_closed`, a node
/// is kept when it is open or has an open descendant, so a closed ancestor of open work
/// stays visible as context. Roots and siblings are in ready order.
pub fn forest(all: &[Task], root: Option<&TaskId>, include_closed: bool) -> Vec<TreeNode> {
    let mut tops: Vec<&Task> = match root {
        Some(id) => all.iter().filter(|task| &task.id == id).collect(),
        None => all.iter().filter(|task| task.parent.is_none()).collect(),
    };
    tops.sort_by(|a, b| ready_order(a, b));
    tops.into_iter()
        .filter_map(|task| node(all, task, include_closed, &mut std::collections::HashSet::new()))
        .collect()
}

fn node(
    all: &[Task],
    task: &Task,
    include_closed: bool,
    visited: &mut std::collections::HashSet<TaskId>,
) -> Option<TreeNode> {
    if !visited.insert(task.id.clone()) {
        return None;
    }
    let keep = include_closed || task.status.is_open() || !open_descendants(all, &task.id).is_empty();
    if !keep {
        return None;
    }
    let mut kids = children(all, &task.id);
    kids.sort_by(|a, b| ready_order(a, b));
    Some(TreeNode {
        summary: TaskSummary::of(task, all),
        children: kids
            .into_iter()
            .filter_map(|child| node(all, child, include_closed, visited))
            .collect(),
    })
}
```

Unit test:

```rust
    #[test]
    fn forest_prunes_closed_leaves_but_keeps_closed_ancestors_of_open_work() {
        let root = task("xx-000001", None, Status::Todo);
        let closed_leaf = task("xx-000002", Some("xx-000001"), Status::Done);
        let closed_mid = task("xx-000003", Some("xx-000001"), Status::Done);
        let open_deep = task("xx-000004", Some("xx-000003"), Status::Todo);
        let all = [root, closed_leaf, closed_mid, open_deep];
        let nodes = forest(&all, None, false);
        assert_eq!(nodes.len(), 1);
        let kids: Vec<&str> = nodes[0].children.iter().map(|n| n.summary.id.as_str()).collect();
        assert_eq!(kids, ["xx-000003"]);
        assert_eq!(nodes[0].children[0].children[0].summary.id, "xx-000004");
        assert_eq!(forest(&all, None, true)[0].children.len(), 2);
        assert_eq!(forest(&all, Some(&all[2].id), true)[0].summary.id, "xx-000003");
    }
```

- [ ] **Step 5: Commands**

`src/cli.rs`: add to `Command`:

```rust
    /// The task hierarchy as nested nodes (open work only unless --all).
    Tree {
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
```

and to `Command::List` after `owner`:

```rust
        /// Only direct children of this task.
        #[arg(long)]
        parent: Option<String>,
```

`src/commands/tree.rs`:

```rust
use super::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::{Output, TreeOut};

pub fn run(ctx: Ctx, id: Option<String>, all: bool) -> Result<Output> {
    let tasks = ctx.project.scan()?;
    let root = id.as_deref().map(TaskId::parse).transpose()?;
    if let Some(root) = &root
        && !tasks.iter().any(|task| &task.id == root)
    {
        return Err(Error::TaskNotFound(root.to_string()));
    }
    Ok(Output::Tree(TreeOut {
        nodes: crate::hierarchy::forest(&tasks, root.as_ref(), all),
        warnings: ctx.warnings,
    }))
}
```

Register `pub mod tree;` in `src/commands/mod.rs` and dispatch `Command::Tree { id, all } => tree::run(open_ctx(dir)?, id, all)`. Add `parent` to the `Command::List` pattern and pass it to `list::list(..., parent, all_projects)`.

`src/commands/list.rs`, in `list`, add a `parent: Option<String>` parameter after `owner`; before `tasks.retain`:

```rust
    let parent = parent.as_deref().map(TaskId::parse).transpose()?;
    if let Some(parent) = &parent
        && !all.iter().any(|task| &task.id == parent)
    {
        return Err(Error::TaskNotFound(parent.to_string()));
    }
```

and inside the retain closure add `let parent_ok = parent.as_ref().is_none_or(|p| task.parent.as_ref() == Some(p));` and `&& parent_ok`. (`all` is the pre-filter clone introduced in Task 2.)

`src/commands/show.rs`: after building `depends_on`:

```rust
    let project = &ctx.project; // the feedback plan's Task 2 replaces this binding
    let all = project.scan()?;
    let related = |task: &crate::model::Task| Related {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: task.status.as_str().into(),
    };
    let parent = task
        .parent
        .as_ref()
        .and_then(|id| all.iter().find(|candidate| &candidate.id == id))
        .map(related);
    let mut kids = crate::hierarchy::children(&all, &task.id);
    kids.sort_by(|a, b| crate::query::ready_order(a, b));
    let children = kids.into_iter().map(related).collect();
```

and pass `parent, children,` into `ShowOut`. Import `Related` from `crate::output`. Children are in ready order (priority, size, created), the same order `tree` uses; `Project::scan` order is id order, which is random with respect to creation and must not leak into the contract.

**Cross-plan integration.** The feedback plan's Task 2 makes `show` resolve foreign ids by binding `let project: &Project = …` (local or foreign) at the top of `run`. If that has landed, delete the `let project = &ctx.project;` line above so the scan reads the foreign project; relationships must never silently come back empty for a foreign task. If it has not landed, keep the line; the feedback plan says the same from its side. Then extend the show test so the seam is proven:

```rust
    // only once the feedback plan's Task 2 has landed: relationships resolve for a
    // foreign id too
    let fam = env.init("fam");
    let far = id_of(env.json(&fam, &["add", "Far"]));
    let far_kid = id_of(env.json(&fam, &["add", "Far kid", "--parent", &far]));
    assert_eq!(env.json(&dir, &["show", &far])["children"][0]["id"], far_kid);
    assert_eq!(env.json(&dir, &["show", &far_kid])["parent"]["id"], far);
```

- [ ] **Step 6: Run all gates**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```
Expected: all pass.

- [ ] **Step 7: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-afe69c "tree command with pruning and ready order; show reports parent and children; list --parent"
tasks check
git add -A
git commit -m "feat: tree command, show parent/children, list --parent"
```

---

### Task 4: Add roadmap and closeout to prime

Tracker: `tasks-fb75b5`. Spec §4.3 (prime), §6 (`prime`, pretty rules).

**Files:**
- Modify: `src/output.rs` (PrimeOut, pretty), `src/commands/list.rs` (prime)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `hierarchy::forest`, `hierarchy::children`, `hierarchy::open_descendants`, `query::sort_ready`, `TaskSummary::of`.
- Produces: `PrimeOut.roadmap: Vec<TreeNode>`, `PrimeOut.closeout: Vec<TaskSummary>`.

- [ ] **Step 1: Write the failing e2e test**

```rust
#[test]
fn prime_shows_roadmap_and_closeout() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let leaf = id_of(env.json(&dir, &["add", "Leaf", "--parent", &goal]));
    let loner = id_of(env.json(&dir, &["add", "Loner"]));
    env.json(&dir, &["start", &goal]);

    let prime = env.json(&dir, &["prime"]);
    assert_eq!(prime["closeout"], serde_json::json!([]));
    let roadmap = prime["roadmap"].as_array().unwrap();
    assert_eq!(roadmap.len(), 2, "{prime}");
    assert_eq!(roadmap[0]["id"], goal);
    assert_eq!(roadmap[0]["children"][0]["id"], leaf);
    assert_eq!(roadmap[1]["id"], loner);
    let ready: Vec<&str> = prime["ready"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(!ready.contains(&goal.as_str()));

    env.json(&dir, &["done", &leaf]);
    let prime = env.json(&dir, &["prime"]);
    assert_eq!(prime["closeout"][0]["id"], goal, "a doing parent surfaces: {prime}");
    assert!(prime["ready"].as_array().unwrap().iter().all(|t| t["id"] != goal));

    let parked = id_of(env.json(&dir, &["add", "Parked", "--status", "idea"]));
    let kid = id_of(env.json(&dir, &["add", "Kid", "--parent", &parked]));
    env.json(&dir, &["done", &kid]);
    let prime = env.json(&dir, &["prime"]);
    assert!(
        prime["closeout"].as_array().unwrap().iter().all(|t| t["id"] != parked),
        "an idea is never a close-out candidate: {prime}"
    );

    let stuck = id_of(env.json(&dir, &["add", "Stuck"]));
    env.json(&dir, &["block", &stuck, "waiting"]);
    let out = env.cmd(&dir).args(["--pretty", "prime"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\ncloseout:\n"), "{text}");
    let roadmap = text.split("\nroadmap:\n").nth(1).unwrap().split("\nready:\n").next().unwrap();
    assert!(roadmap.contains(&goal), "roots with children print as subtrees: {roadmap}");
    assert!(roadmap.contains(&parked), "an idea with children is still a subtree: {roadmap}");
    assert!(roadmap.contains(&stuck), "a childless root absent from ready is printed: {roadmap}");
    assert!(!roadmap.contains(&loner), "a childless root present in ready is only counted: {roadmap}");
    assert!(
        roadmap.contains("1 open task(s) without children are listed under ready"),
        "{roadmap}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli prime_shows_roadmap_and_closeout`
Expected: FAIL (`closeout` is null).

- [ ] **Step 3: Implement**

`src/output.rs`, `PrimeOut` gains after `doing`:

```rust
    pub roadmap: Vec<TreeNode>,
    pub closeout: Vec<TaskSummary>,
```

In the `Output::Prime` pretty arm, before `"\nready:\n"`:

```rust
            rendered.push_str("\ncloseout:\n");
            rendered.push_str(&table(&o.closeout));
            rendered.push_str("\nroadmap:\n");
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    rendered.push_str(&tree_text(std::slice::from_ref(node), 0));
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    rendered.push_str(&table(std::slice::from_ref(&node.summary)));
                }
            }
            rendered.push_str(&format!(
                "{listed_under_ready} open task(s) without children are listed under ready\n"
            ));
```

Three rules, each for a reason. A root is a subtree when it has children at all (`child_count`, not the pruned `children` array, so a goal whose children are all closed still prints as a goal rather than being miscounted as childless). A childless root is only counted when it actually appears in `ready`, so nothing is hidden behind the count. Every other childless root (blocked, idea, doing, or todo with an open dependency) is printed, because it is in the roadmap and nowhere else on the screen.

`src/commands/list.rs`, in `prime`, before constructing `PrimeOut`:

```rust
    let roadmap = crate::hierarchy::forest(&all, None, false);
    let mut closeout: Vec<Task> = all
        .iter()
        .filter(|task| {
            // spec §4.3: todo, doing, or blocked; an idea is open but not a candidate
            matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
                && !crate::hierarchy::children(&all, &task.id).is_empty()
                && crate::hierarchy::open_descendants(&all, &task.id).is_empty()
        })
        .cloned()
        .collect();
    sort_ready(&mut closeout);
```

and pass `roadmap, closeout: closeout.iter().map(|task| TaskSummary::of(task, &all)).collect(),`.

- [ ] **Step 4: Run all gates**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```
Expected: all pass. `prime_reports_counts_ready_and_doing` keeps passing; it does not assert on the new fields.

- [ ] **Step 5: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-fb75b5 "prime gains roadmap (pruned open forest) and closeout (open parents with no open descendant); pretty prints both sections"
tasks check
git add -A
git commit -m "feat: roadmap and closeout sections in prime"
```

---

### Task 5: Warn on plan headings with no task (unlinked_step)

Tracker: `tasks-01e911`. Spec §4.5. Independent of Tasks 1–4.

**Files:**
- Modify: `src/commands/check.rs`, `src/resolve.rs` (a heading iterator)
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `resolve::step_headings(text: &str) -> Vec<String>` (heading texts that start with `Task <digits>:`); check warning kind `unlinked_step`.

- [ ] **Step 1: Write the failing e2e test**

```rust
#[test]
fn check_warns_on_plan_headings_without_a_task() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(
        &dir,
        "docs/plans/2026-09-03-p.md",
        "# Plan\n\n## Overview\n\n### Task 1: one\n\n### Task 2: two\n\n### Notes on Task 3\n",
    );
    write_doc(&dir, "docs/plans/2026-09-03-unlinked.md", "### Task 1: nobody\n");
    env.json(&dir, &["add", "A", "--plan", "p", "--step", "Task 1: one"]);
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warnings = check["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{check}");
    assert_eq!(warnings[0]["kind"], "unlinked_step");
    assert_eq!(warnings[0]["file"], "docs/plans/2026-09-03-p.md");
    assert_eq!(warnings[0]["id"], serde_json::Value::Null);
    assert!(warnings[0]["detail"].as_str().unwrap().contains("Task 2: two"));
}
```

The unlinked plan file produces no warning: only plans that some task links are checked.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli check_warns_on_plan_headings_without_a_task`
Expected: FAIL (zero warnings).

- [ ] **Step 3: Implement**

`src/resolve.rs`, next to `heading_text`:

```rust
/// Heading texts of the form `Task <digits>: …`, in file order.
pub fn step_headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(heading_text)
        .filter(|heading| {
            heading
                .strip_prefix("Task ")
                .and_then(|rest| rest.split_once(':'))
                .is_some_and(|(number, _)| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_string)
        .collect()
}
```

Unit test in `src/resolve.rs` (add a `#[cfg(test)] mod tests` if absent):

```rust
    #[test]
    fn step_headings_match_only_the_task_n_convention() {
        let text = "# P\n### Task 1: one\n## Task 12: twelve\n### Notes on Task 3\n### Task x: no\nTask 4: not a heading\n";
        assert_eq!(step_headings(text), ["Task 1: one", "Task 12: twelve"]);
    }
```

`src/commands/check.rs`, before `Ok(Output::Check(...))`:

```rust
    let mut linked_plans: Vec<&String> = tasks.iter().filter_map(|task| task.plan.as_ref()).collect();
    linked_plans.sort();
    linked_plans.dedup();
    for plan in linked_plans {
        let path = ctx.project.root.join(plan);
        if !path.is_file() {
            continue; // already an error above
        }
        let text = std::fs::read_to_string(path)?;
        for heading in crate::resolve::step_headings(&text) {
            let linked = tasks
                .iter()
                .any(|task| task.plan.as_ref() == Some(plan) && task.step.as_deref() == Some(&heading));
            if !linked {
                warnings.push(finding(
                    None,
                    plan.clone(),
                    "unlinked_step",
                    format!("heading {heading:?} has no task"),
                ));
            }
        }
    }
```

- [ ] **Step 4: Run all gates**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```
Expected: all pass. `check_passes_clean_repo_and_reports_drift` links its only heading, so it stays warning-free before the drift edits; after the rename to `Task 1: uno` that test now sees one extra `unlinked_step` warning in addition to the `step_missing` error. Update its warning assertions accordingly (the test already inspects warnings for the foreign dependency; add the new kind to the expected set).

Then run `tasks check` in this repository: the two plans written by this design (`2026-09-03-task-hierarchy.md`, `2026-09-03-feedback.md`) link every heading, so it stays clean. `docs/plans/2026-08-29-tasks.md` is not linked by any open task and is not scanned.

- [ ] **Step 5: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-01e911 "check warns unlinked_step for Task N: headings in linked plans that no task references"
tasks check
git add -A
git commit -m "feat: warn on plan headings with no task"
```

---

### Task 6: Update original design, skill, and README for the hierarchy protocol

Tracker: `tasks-43e062`. Spec §5, §7. Depends on Tasks 4 and 5.

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md`, `docs/specs/2026-09-03-task-hierarchy-design.md` (status line), `skills/tasks/SKILL.md`, `README.md`, `AGENTS.md`

- [ ] **Step 1: Original design**

In `docs/specs/2026-08-29-tasks-design.md`:

- §1 non-goals: replace the line `- No hierarchy (epics/subtasks), no \`split\` command, no scheduling math.` with `- No task kinds and no scheduling math; hierarchy is one \`parent\` field (docs/specs/2026-09-03-task-hierarchy-design.md).`
- §3 example frontmatter: add `parent: sci-1a2b3c` after the `depends:` line. §3.1: add the row `| \`parent\` | task id | no | Same project; must exist; a task cannot be its own ancestor. Written after \`depends\`. |` after `depends`.
- §3.3: rename "Open-deps rule" to "Open-work rule: a task may become `done` only when every dependency and every descendant is closed, unless `--force` is given; `drop` refuses while any descendant is open and has no override." Change the `ready` definition line to: `\`ready\` = status \`todo\`, no children, and every entry in \`depends\` is closed.`
- §5: `add` gains `[--parent ID]`; `edit` gains `[--parent ID | --no-parent]`; `list` gains `[--parent ID]`; add the `tasks tree [<id>] [--all]` entry (copy the text from the hierarchy design §4.4); `show` gains "the parent and direct children"; `prime` gains "the roadmap (open forest) and closeout list"; `check` gains "parent problems (dangling, foreign, cycle), open child of closed parent, plan headings with no task".
- §5.1 shapes: copy the block from the hierarchy design §6 verbatim into the shapes list (`Task += parent`, `TaskSummary += …`, `show += …`, `TreeNode`, `tree ->`, `prime += …`, `check += …`).
- §7: add the bullet "`check` warns `unlinked_step` for a `Task N:` heading in a linked plan that no task references."
- §8: replace the three superpowers bullets with the protocol in the hierarchy design §5 (tasks first; `--parent` for decomposition; never pick a task with children; close out from `prime.closeout`; brainstorming attaches with `edit --spec`; writing-plans attaches with `edit --plan` and adds children with `--parent --plan --step`).
- Status header: append `hierarchy <landing date>` with the landing date.

- [ ] **Step 2: Skill**

In `skills/tasks/SKILL.md`:

- Session protocol step 1 becomes: "`tasks prime` — roadmap (the open goal tree), closeout (goals whose work is all done), the ready list, and who is working on what."
- Step 2 gains: "Never pick a task with children; those are goals. `ready` already omits them."
- Add step 7: "When a goal appears under `closeout`, confirm it is met and `tasks done <id> "<verdict>"`, or add the children still missing."
- "Recording work": replace the Splitting bullet with: "Decomposing: `tasks add "<piece>" --parent <goal>` for each part; `tasks dep` only for ordering between the pieces. A goal that is committed work is a `todo` with a body, however large; `idea` is for uncommitted thoughts."
- Add "`tasks tree [<id>]` shows the hierarchy; `tasks edit <id> --parent <goal>` / `--no-parent` moves a task."
- "With superpowers": brainstorming runs against an existing task and attaches with `tasks edit <id> --spec <topic>`; deliverables become children with `--parent <id> --spec <topic>`. writing-plans attaches with `tasks edit <id> --plan <topic>` and adds one child per `### Task N:` heading with `--parent <id> --plan <topic> --step "Task N: <title>"`; `tasks check` warns on any heading left without a task.
- Update the `tasks edit` flag list to include `--parent/--no-parent`.

- [ ] **Step 3: README and AGENTS.md**

`README.md` "Use" block: add `tasks add "Emit rows" --parent sci-4f2a9c` after the first `add`, and `tasks tree` after `tasks ready`. "For agents" step 4: add `tasks tree` with the comment `# the goal hierarchy`. `AGENTS.md` session protocol: add "Decompose goals with `--parent`; close a goal from `prime`'s closeout list."

- [ ] **Step 4: Status lines**

`docs/specs/2026-09-03-task-hierarchy-design.md` status becomes `implemented (<landing date>); see docs/plans/2026-09-03-task-hierarchy.md`. Grep both `README.md` and `skills/tasks/SKILL.md` for "split" and "no hierarchy" to catch drift.

- [ ] **Step 5: Verify, close the task and the umbrella, commit**

Run the gates:

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && tasks check
```

Then, since this is the last piece under the umbrella:

```bash
tasks done tasks-43e062 "design, skill, README, AGENTS.md describe the hierarchy protocol; both spec statuses updated"
tasks done tasks-061851 "hierarchy shipped: parent field, subtree rules, tree, roadmap/closeout, unlinked_step; docs and skill rewritten tasks-first"
tasks check
git add -A
git commit -m "docs: hierarchy protocol in design, skill, and README"
```

If `tasks done tasks-061851` refuses because a feedback-side task is still open, that is a dependency mistake from the split, not a reason to force: the umbrella depends only on the six hierarchy tasks, so investigate before using `--force`.
