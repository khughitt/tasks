# Lanes, needs, and project groups — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a project declare concurrent efforts (lanes), the shared resources that serialise them (needs, with exclusive holds), and named project groups inside the tracker, so `prime`, `ready`, and `next` show and honour them.

**Architecture:** Four slices. **Needs** adds a project vocabulary and a task field, plus session-level hiding (`--without`/`TASKS_WITHOUT`). **Holds** records exclusive needs on live claims and gates the picker and every acquire path across registered projects, under a host-wide lock. **Lanes** marks goals as lanes and adds the lanes view, `in_lane` membership, pausing, and `--under`. **Groups** adds registry-declared project sets as a scope. JSON stays additive.

**Tech Stack:** Rust (clap, serde, toml), end-to-end tests in `tests/cli.rs` against the built binary, the `just` test front door with `tools/tt`.

**Spec:** `docs/specs/2026-10-03-lanes-needs-groups-design.md` (approved at `8a7accd`). Task: tasks-ece1e2.

## Global Constraints

- **Order:** Task 0.1, then Slice 1 → Slice 2 → Slice 3 → Slice 4, each task starting from the previous task's end state. Slice 4 does not depend on the others' code, but it runs last. The inventory is adopted with `--at`, and each `--at` commit must be at or after the previous one (see below). Task numbers are `<slice>.<n>`.
- **Gates:** while working, `just test-one <filter>` or `just test-one --test cli <name>`; before each commit, `just test-fast` and `just check`. Never run `cargo test` directly. `tasks check` runs inside `just check`. Every commit passes `cargo clippy --all-targets -- -D warnings`.
- **Commits:** conventional commits, no attribution trailers. Commit on branch `ece1e2-work-selection` in `.worktrees/ece1e2-work-selection`.
- **JSON:** additive only. Each new field or payload gets its `+=` addendum in main design §5.1, in the task that ships it (§3.4 checklist of `docs/specs/2026-08-29-tasks-design.md`).
- **CLI inventory** (ops AGENTS.md rule; ops `docs/specs/2026-10-01-vendoring-rollout-order-design.md`, "Adopted file"):
  - Rows land in ops first.
  - This project adopts them with `vendored adopt`, in this worktree, and commits the copy together with the implementation that needs it.
  - Nobody writes `tools/cli.toml` by hand or with `cp`.

  How the plan does it:
  - Task 0.1 lands **one ops commit per CLI-touching task**, in execution order.
  - Each such task carries its row edit in a block titled **"Ops rows (landed by Task 0.1)"**, and adopts exactly the rows landed up to its own commit: `python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`. The sha is read from the `inventory: Task <n> = <sha>` note on tasks-ece1e2.
  - `$OPS` is the `ops` root from `tasks projects`.
  - **Approval boundary:** Task 0.1 Step 4 stops for the user before merging into ops `main`, because ops is shared tooling. No implementation task starts before that merge.
- **The installed `tasks`:** do not run `cargo install --path .` from this worktree before Task 5.1. That would repoint the host's installed `tasks`.
- **The cross-slice seam:** the hold check runs in `ready`/`next`/`prime` after `ready_tasks` (Slice 2, Task 2.5), not inside it. The lanes builder (Task 3.7) therefore sees steps before holds apply and counts them as `held` itself (spec §5.1 step 1).
- **Retiring `exclusive_of`'s lint attribute:** Slice 1 gives `needs::exclusive_of` the attribute `#[cfg_attr(not(test), expect(dead_code, ...))]`. The first Slice 2 task that calls it (Task 2.1) deletes that attribute in the same commit; otherwise clippy fails on the unfulfilled expectation.

## Review Focus

These are the five inputs most likely to bite a user that the spec implies but does not spell out. Each is pinned by a test in the owning task:

1. **`TASKS_WITHOUT` set on the host.** A value like `",quiet, "` parses to `{quiet}`. A project that does not declare `quiet` lists normally. The suite strips the variable so the host's value never leaks into it. *Task 1.6*
2. **Two sessions contending for `quiet` in different projects at the same moment.** This includes one session releasing its hold while its record write fails and is rolled back. Exactly one session holds `quiet` at a time, and a failed write never frees it early. *Task 2.6*
3. **Claim stores written by an older binary.** An entry without `holds` loads as holding nothing, and a TTL-only claim (no pid) holds until its TTL. *Tasks 2.1, 2.2*
4. **Lane shapes the spec only implies.** Covered: a lane whose steps sit under a sub-goal, a lane holding only a recurrence (due, not yet due, dropped), and a lane whose body is only a heading. Each gives the right state and partition. *Task 3.7*
5. **A group whose every member is unreachable.** It gives warnings and empty results, not an error. *Task 4.4*

---

## Slice 0 — CLI inventory in ops

### Task 0.1: Land every new CLI row in ops, one commit per task

The ops rule is in ops `AGENTS.md` and in ops `docs/specs/2026-10-01-vendoring-rollout-order-design.md` ("Adopted file"). A new CLI option changes `cli.toml` in ops first. The project then adopts the landed table with `vendored adopt`, committing the copy together with its implementation.

`--at` adopts every row landed up to a commit. So each CLI-touching task gets its own ops commit, landed in execution order. Each task then adopts exactly its own rows.

**Files:**
- Modify (ops worktree): `cli.toml`, ten commits.
- Task records: an ops task, plus `inventory:` notes on tasks-ece1e2.

**Interfaces:**
- Produces: ten notes on tasks-ece1e2 of the form `inventory: Task <n> = <full sha>`, one for each n in 1.3, 1.5, 1.6, 2.4, 3.1, 3.6, 3.7, 4.2, 4.4, 4.5. These are the CLI-touching tasks in execution order. Each task's adopt step reads its sha from these notes.

- [ ] **Step 1: Create the ops worktree.**
  `$OPS` is the `ops` root printed by `tasks projects`.
  ```bash
  cd "$OPS" && work-link --ensure .worktrees
  git worktree add .worktrees/ece1e2-cli -b feat/ece1e2-cli
  git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/ece1e2-cli
  cd .worktrees/ece1e2-cli && just setup
  ```

- [ ] **Step 2: File the ops task and link it to its piece.**
  This follows the ops procedure, step 2. In `$OPS`:
  ```bash
  tasks add "cli.toml rows for tasks lanes, needs, holds, and groups" --status todo -p 2 --source tasks-ece1e2 \
    -b "Rows for the tasks CLI from docs/specs/2026-10-03-lanes-needs-groups-design.md in the tasks repo; one commit per implementing task (tasks plan docs/plans/2026-10-03-lanes-needs-groups.md Task 0.1). tasks-ece1e2 adopts them."
  tasks dep <new ops id> --on tasks-ece1e2
  ```
  Commit the record on ops `main` as `chore(tasks): file <new ops id>`.

- [ ] **Step 3: Write the ten commits, in this order.**
  In `$OPS/.worktrees/ece1e2-cli`, do the following for each task, in the order 1.3, 1.5, 1.6, 2.4, 3.1, 3.6, 3.7, 4.2, 4.4, 4.5:
  1. Apply that task's **Ops rows (landed by Task 0.1)** block to `cli.toml`, exactly as written in the task.
  2. Run `just test-one test_cli`. Expected: PASS.
  3. Commit: `git commit -am "feat(cli): tasks rows for tasks-ece1e2 Task <n>"`.

  Then run `git log --oneline main..` and check that it lists exactly ten commits, in that order.

- [ ] **Step 4: STOP — ask the user before landing in ops `main`.**
  Ops is shared tooling: landing these rows changes a table every project adopts from. Show the user the ten commits (`git log --stat main..`) and wait for an explicit yes. On a no, park tasks-ece1e2 with `--waiting-on user --reason approval` and stop.

- [ ] **Step 5: Merge, push, and record the shas.**
  On approval, merge the branch into ops `main` with `--ff-only`, which keeps the ten shas. Push ops per its guide.

  Other projects then carry these rows harmlessly, because each project's test reads only its own rows. This project keeps passing until it adopts.

  Then record each sha in the tasks worktree:
  ```bash
  cd .worktrees/ece1e2-work-selection   # in the tasks repo
  for n in 1.3 1.5 1.6 2.4 3.1 3.6 3.7 4.2 4.4 4.5; do
    sha=$(git -C "$OPS" log --format=%H --grep "tasks rows for tasks-ece1e2 Task $n\$" -1 main)
    tasks note tasks-ece1e2 "inventory: Task $n = $sha"
  done
  tasks check && git add tasks && git commit -m "chore(tasks): record the ops inventory commits for tasks-ece1e2"
  ```
  Expected: ten notes, each with a 40-character sha.

- [ ] **Step 6: Remove the ops worktree.**
  ```bash
  cd "$OPS"
  tt-report
  git worktree unlock .worktrees/ece1e2-cli
  git worktree remove .worktrees/ece1e2-cli
  git branch -d feat/ece1e2-cli
  ```

---

## Slice 1 — Needs

Spec: `docs/specs/2026-10-03-lanes-needs-groups-design.md` §4.1 (vocabulary), §4.2 (field),
§4.3 (`--without` / `TASKS_WITHOUT`, `--need` filter), the status/needs separation rule of
§4.4, and the needs lines of §8, §9 and §10. Holds, `need_held`, the foreign-claim needs
refusal and `edit --reason` are Slice 2.

**Slice constraints**

- Every commit compiles without warnings under `cargo clippy --all-targets -- -D warnings`
  (the pre-commit gate). This crate has no `allow(dead_code)` anywhere, so each task
  introduces only items its own code reads. The one exception is `needs::exclusive_of`,
  which the contract has Slice 1 produce and only Slice 2 reads. It carries
  `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]`. Once Slice 2 calls it,
  the expectation goes unfulfilled, clippy fails, and Slice 2 must delete the attribute.
- CLI inventory: the rows live in ops `cli.toml` and land there first. Each CLI-touching
  task in this slice (1.3, 1.5, 1.6, in that order) keeps its exact row edit in a block
  titled **Ops rows (landed by Task 0.1)**. Task 0.1 applies those blocks in ops, one ops
  commit per task in execution order, and records each commit as the note
  `inventory: Task <n> = <sha>` on tasks-ece1e2. The task itself never edits ops. Its
  inventory step adopts the rows in this worktree with
  `python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where `$OPS` is the `ops`
  root from `tasks projects` and `<sha>` is the value of that task's inventory note
  (`tasks show tasks-ece1e2`). `--at` adopts every row landed up to that commit, so the
  tasks run in the order of the ops commits. Never write `tools/cli.toml` by hand or with
  `cp`. The adopt also refreshes `tools/cli_surface.py` when the pair changed; the
  commit adds it whenever `git status` shows it new or modified.
- Tests: `just test-one <filter>` while working, then `just test-fast` and `just check`
  before each commit. Never run `cargo test` directly. `tasks check` runs inside
  `just check`.
- Do not `cargo install --path .` from this worktree. That would repoint the host's
  installed `tasks`, which the plan's integration step owns.
- Conventional commits, with no attribution trailers.

**Review focus**

- `TASKS_WITHOUT=",quiet, "` parses to `{quiet}` (Task 1.6, unit
  `the_variable_is_trimmed_lenient_and_joins_the_strict_flag`, and end to end in
  `needs_without_variable_is_lenient_trimmed_and_joins_the_flag`).
- A need name with uppercase is refused at `add` with `validation` (Task 1.3,
  `needs_flags_refuse_an_undeclared_or_malformed_name`).
- `--without` under `--all-projects` is accepted when only one project declares the name
  (Task 1.6, `needs_without_refuses_a_name_no_project_in_scope_declares`).
- Hiding uses the vocabulary of the task's own project, never the union of the scope's.
  A stale `needs: [quiet]` record in a project that does not declare `quiet` stays
  visible under both `TASKS_WITHOUT=quiet` and `--all-projects --without quiet` (Task 1.6,
  unit `without_hides_only_where_the_owning_project_declares_the_name`, and end to end
  in `needs_without_hides_only_where_the_owning_project_declares_the_name`).
- A record whose need was removed from the vocabulary still lists and fails `tasks check`.
  Task 1.2 covers a hand-written record, Task 1.3 a flag-written one
  (`needs_check_errors_on_an_undeclared_need_on_every_record`,
  `needs_flags_a_need_dropped_from_the_vocabulary_still_lists_but_fails_check`).
- `edit --status doing --need quiet` is a usage error, and an editor save that changes
  both status and needs is refused. In both cases the record is unchanged (Task 1.4).

### File Structure

- Create `src/needs.rs`. It holds the need-name grammar (`validate_name`), the vocabulary
  types (`NeedDecl`, `Vocabulary`), `require_declared`, `exclusive_of` (for Slice 2), and
  the session availability set (`WITHOUT_ENV`, `Vocabularies`, `vocabulary_of`, `Without`).
- Modify `src/main.rs`: register `mod needs;`.
- Modify `src/error.rs`: add `Error::UnknownNeed`, kind `unknown_need`.
- Modify `src/model.rs`: add `Task.needs` and update the `task_with` helper.
- Modify `src/format.rs`: add `needs` to `KEYS` after `parallel`, then update
  `parse_task`, `validate_task`, `serialize_task`, and the round-trip tests.
- Modify `src/output.rs`: add `needs` to `TaskSummary`, `TaskSummary::of`, `ParkedRow`,
  `ParkedRow::resolved` and `ParkedRow::unresolved`, and update the `row` test helper.
- Modify `src/repo.rs`: parse `[needs]` into `Project.needs` and add a unit test.
- Modify `src/commands/check.rs`: report a record naming an undeclared need as an
  `unknown_need` error.
- Modify `src/cli.rs`: `FieldArgs.needs` (`--need`), `EditArgs.rm_needs` / `no_needs`,
  the `--status` conflicts, `FilterArgs.needs`, and a new `WithoutArgs` flattened into
  `ready`, `next` and `prime`.
- Modify `src/commands/mod.rs`: `apply_fields` validates and appends needs; the
  `Ready`/`Next`/`Prime` dispatch passes `--without`.
- Modify `src/commands/edit.rs`: flag presence, `--rm-need` / `--no-needs`, the editor
  save's check of added needs, and the status/needs refusal.
- Modify `src/filter.rs`: `TaskFilter.needs` and `Fields.needs` (all-of).
- Modify `src/commands/list.rs`: `ready`, `next` and `prime` apply `Without`.
- Modify the `Task` literals to add `needs: vec![]` in `src/commands/add.rs`,
  `src/halt.rs`, `src/hierarchy.rs`, `src/defer.rs`, `src/complexity.rs`,
  `src/periodic.rs`, `src/similarity.rs`, `src/repo.rs` and `src/query.rs`. In
  `src/halt.rs`, also add `needs` to its `Project` literals.
- Modify `tests/common/mod.rs`: `env_remove("TASKS_WITHOUT")` in `cmd`, `raw` and
  `shim_command`.
- Modify `tests/cli.rs`: add the end-to-end tests and three helpers that later slices
  reuse: `seed_needs`, `declare_needs` and `task_ids`.
- Adopt `tools/cli.toml` (and `tools/cli_surface.py` when the adopt refreshes it) from
  the ops rows Task 0.1 lands: `add` `--need`; `edit` `--need` / `--rm-need` /
  `--no-needs` (Task 1.3); `list` and `ready` `--need` (Task 1.5); `ready`, `next` and
  `prime` `--without` (Task 1.6).
- Modify `docs/specs/2026-08-29-tasks-design.md`: the §3.1 row, the §5 usage lines, the
  §5.1 `+=` addenda and the §6 `[needs]` config.
- Modify `skills/tasks/SKILL.md`, `skills/scope/SKILL.md` and `README.md`.

---

### Task 1.1: The `needs` field on the record

**Files:**
- Create: `src/needs.rs`
- Modify: `src/main.rs:16` (add `mod needs;` after `mod model;`)
- Modify: `src/model.rs:467` (`Task`, after `pub parallel: bool,`), `src/model.rs:128`
  (`task_with`)
- Modify: `src/format.rs:6-32` (`KEYS`), `:80-88` (closures), `:121` (`parse_task`),
  `:374-376` (`validate_task`), `:419-422` (`serialize_task`), and the tests after
  `parallel_false_in_a_file_is_dropped_on_the_next_write` (~line 769)
- Modify: `src/output.rs:207` and `:410` (`TaskSummary`), `:454`, `:503` and `:533`
  (`ParkedRow`), and `:1957` (`row` helper)
- Modify (`needs: vec![],` after the `parallel` line of each `Task` literal):
  `src/commands/add.rs:28`, `src/halt.rs:112`, `src/hierarchy.rs:305`,
  `src/defer.rs:145`, `src/complexity.rs:116`, `src/periodic.rs:75`,
  `src/similarity.rs:99`, `src/repo.rs:725`, `src/query.rs:283`
- Test: `src/needs.rs` (unit), `src/format.rs` (unit), `tests/cli.rs` (end to end)

**Interfaces:**
- Consumes: `crate::error::{Error, Result}`; the `frontmatter::Value::List` read and write
  that `tags` uses.
- Produces:
  - `pub fn crate::needs::validate_name(name: &str) -> Result<()>`. The tag grammar:
    non-empty, `[a-z0-9-]`, not starting with `-`. A violation is `Error::Validation`.
  - `pub needs: Vec<String>` on `Task`, `TaskSummary` and `ParkedRow`. Its serde
    attribute is `skip_serializing_if = "Vec::is_empty"`.
  - The frontmatter key `needs`. In `KEYS` it sits directly after `parallel`; Slice 3
    inserts `lane` between them. It is written only when non-empty, and a missing key
    reads as empty.
  - The `tests/cli.rs` helper
    `fn seed_needs(dir: &std::path::Path, id: &str, list: &str)`.

- [ ] **Step 1: Write the failing unit tests**

Create `src/needs.rs` holding only the module doc and its tests. The test module fails to
compile until Step 3 adds `validate_name`:

```rust
//! Shared resources a task's work uses, drawn from the vocabulary its project declares in
//! `[needs]` (docs/specs/2026-10-03-lanes-needs-groups-design.md §4).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_tag_grammar() {
        for good in ["quiet", "gpu-0", "2d", "needs-owner"] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", "Quiet", "-quiet", "a_b", "a b", "qüiet", "quiet\n"] {
            let error = validate_name(bad).unwrap_err();
            assert_eq!(error.kind(), "validation", "{bad:?}");
        }
    }
}
```

Register it in `src/main.rs`, after `mod model;`:

```rust
mod needs;
```

Append to the `tests` module of `src/format.rs`, after
`parallel_false_in_a_file_is_dropped_on_the_next_write`:

```rust
    #[test]
    fn needs_round_trip_after_parallel_and_are_omitted_when_empty() {
        let t = parse_task(MINIMAL, "x").unwrap();
        assert!(t.needs.is_empty(), "absent key reads as empty");
        assert!(!serialize_task(&t).contains("needs"));

        let text = MINIMAL.replace(
            "priority: 2\n",
            "priority: 2\nparallel: true\nneeds: [quiet, owner]\n",
        );
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.needs, ["quiet", "owner"]);
        let out = serialize_task(&t);
        assert!(
            out.contains("\nparallel: true\nneeds: [quiet, owner]\ncreated: "),
            "written after parallel: {out}"
        );
        assert_eq!(parse_task(&out, "x").unwrap(), t);
    }

    #[test]
    fn needs_empty_list_is_dropped_and_bad_values_are_refused() {
        let empty = MINIMAL.replace("tags: []", "tags: []\nneeds: []");
        let t = parse_task(&empty, "x").unwrap();
        assert!(t.needs.is_empty());
        assert!(!serialize_task(&t).contains("needs"));

        let upper = MINIMAL.replace("tags: []", "tags: []\nneeds: [Quiet]");
        let err = parse_task(&upper, "x").unwrap_err().to_string();
        assert!(err.contains("need \"Quiet\""), "{err}");

        let scalar = MINIMAL.replace("tags: []", "tags: []\nneeds: quiet");
        let err = parse_task(&scalar, "x").unwrap_err().to_string();
        assert!(err.contains("needs must be a list"), "{err}");
    }
```

- [ ] **Step 2: Run the unit tests to see them fail**

Run: `just test-one needs_`
Expected: compile failure. `cannot find function validate_name` and
`no field needs on type Task`.

- [ ] **Step 3: Implement the grammar, the field, and its serialization**

In `src/needs.rs`, above the test module:

```rust
use crate::error::{Error, Result};

/// The tag grammar the spec gives need names (§4.1): lowercase ASCII letters, digits, and
/// `-`, never starting with `-`, so a name cannot read as a flag. Shared with the
/// vocabulary keys, the record field, and the `--need` flags.
pub fn validate_name(name: &str) -> Result<()> {
    let well_formed = !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if well_formed {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "need {name:?} must be lowercase letters, digits, and '-', not starting with '-'"
        )))
    }
}
```

In `src/model.rs`, `Task`, directly after `pub parallel: bool,` (line 467):

```rust
    /// Shared resources the work uses, each declared in the project's `[needs]`
    /// vocabulary. Checked against it on write and by `check`, carried as-is on read.
    /// See docs/specs/2026-10-03-lanes-needs-groups-design.md §4.2.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
```

In `src/model.rs` `task_with` (line 128), after `parallel: false,`:

```rust
            needs: vec![],
```

In `src/format.rs`:

1. `KEYS` becomes `const KEYS: [&str; 26]`, with `"needs",` inserted directly after
   `"parallel",`.
2. After the `boolean` closure (ends ~line 88), add:

```rust
    // An optional list: absent is empty. `tags` and `depends` stay required.
    let optional_list = |k: &str| -> Result<Vec<String>> {
        match pairs.iter().find(|(key, _)| key == k) {
            None => Ok(Vec::new()),
            Some((_, Value::List(v))) => Ok(v.clone()),
            Some((_, Value::Scalar(_) | Value::Raw(_))) => {
                Err(perr(file, format!("{k} must be a list")))
            }
        }
    };
```

3. In the `Task` literal of `parse_task`, after `parallel: boolean("parallel")?,`:

```rust
        needs: optional_list("needs")?,
```

4. In `validate_task`, after the `for tag in &t.tags { ... }` loop:

```rust
    for need in &t.needs {
        crate::needs::validate_name(need)?;
    }
```

5. In `serialize_task`, directly after the `if t.parallel { ... }` block:

```rust
    if !t.needs.is_empty() {
        pairs.push(("needs".into(), Value::List(t.needs.clone())));
    }
```

In `src/output.rs`:

- `TaskSummary`, after `pub parallel: bool,` (line 207):

```rust
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
```

- `TaskSummary::of`, after `parallel: task.parallel,`:

```rust
            needs: task.needs.clone(),
```

- `ParkedRow`, after `pub parallel: bool,` (line 454), use the same two-line field as
  `TaskSummary`. In `ParkedRow::resolved`, after `parallel: summary.parallel,`, add
  `needs: summary.needs,`. In `ParkedRow::unresolved`, after `parallel: false,`, add
  `needs: Vec::new(),`.
- In the `row` test helper (~line 1957), add `needs: vec![],` before `parallel,`.

For every other `Task` literal, add `needs: vec![],` on the line after its `parallel`
line: `src/commands/add.rs:28` (`blank`), `src/halt.rs:112`, `src/hierarchy.rs:305`,
`src/defer.rs:145`, `src/complexity.rs:116`, `src/periodic.rs:75`,
`src/similarity.rs:99`, `src/repo.rs:725` and `src/query.rs:283`.

- [ ] **Step 4: Run the unit tests to see them pass**

Run: `just test-one needs_`
Expected: PASS. That covers `needs::tests::names_follow_the_tag_grammar` and the two
`format::tests::needs_*` tests.

- [ ] **Step 5: Write the failing end-to-end test**

Append to `tests/cli.rs`:

```rust
/// Writes `needs: [<list>]` into a record by hand, ahead of `created`. Fixture-only: it
/// reaches records the flags never write, such as an undeclared or since-removed need.
fn seed_needs(dir: &std::path::Path, id: &str, list: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\ncreated: "), "{text}");
    std::fs::write(
        &path,
        text.replacen("\ncreated: ", &format!("\nneeds: [{list}]\ncreated: "), 1),
    )
    .unwrap();
}

#[test]
fn needs_record_reaches_show_ready_and_parked_rows_and_stays_sparse() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let plain = id_of(env.json(&sci, &["add", "Plain"]));
    let needy = id_of(env.json(&sci, &["add", "Needy"]));
    seed_needs(&sci, &needy, "quiet, owner");

    let show = env.json(&sci, &["show", &needy]);
    assert_eq!(show["task"]["needs"], serde_json::json!(["quiet", "owner"]));
    assert!(
        env.json(&sci, &["show", &plain])["task"].get("needs").is_none(),
        "sparse on Task"
    );

    let ready = env.json(&sci, &["ready"]);
    let row = |id: &str| {
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(row(needy.as_str())["needs"], serde_json::json!(["quiet", "owner"]));
    assert!(row(plain.as_str()).get("needs").is_none(), "sparse on TaskSummary");

    // An unrelated edit keeps the list and writes it in its place: with no size,
    // complexity, process, or parallel set, directly after priority.
    env.json(&sci, &["edit", &needy, "-p", "1"]);
    let raw = env.read(&sci, &format!("tasks/{needy}.md"));
    assert!(
        raw.contains("\npriority: 1\nneeds: [quiet, owner]\ncreated: "),
        "{raw}"
    );

    env.json(&sci, &["park", &needy, "Resume the capture"]);
    let parked = env.json(&sci, &["list", "--parked"]);
    assert_eq!(
        parked["tasks"][0]["needs"],
        serde_json::json!(["quiet", "owner"]),
        "{parked}"
    );
}
```

- [ ] **Step 6: Run it**

Run: `just test-one --test cli needs_record_`
Expected: PASS. Steps 3 and 5 land together because the binary needs the field to build;
Step 1's unit tests are the ones that failed first.

- [ ] **Step 7: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass. The
surface test is unchanged because no flag was added.

```bash
git add src/needs.rs src/main.rs src/model.rs src/format.rs src/output.rs \
  src/commands/add.rs src/halt.rs src/hierarchy.rs src/defer.rs src/complexity.rs \
  src/periodic.rs src/similarity.rs src/repo.rs src/query.rs tests/cli.rs
git commit -m "feat(needs): carry a needs list on the task record"
```

---

### Task 1.2: The vocabulary in `[needs]`, and `check` against it

**Files:**
- Modify: `src/needs.rs` (types, `require_declared`, `exclusive_of`, unit tests)
- Modify: `src/error.rs:38`, `:89` and `:121` (the `UnknownNeed` variant, its
  `with_suffix` arm, and its kind)
- Modify: `src/repo.rs:69-86` (`Project`), `:103-117` (`Config`), after `:158`
  (`need_vocabulary`), `:225-232` (`init` literal), `:253-269` (`open`), and the tests
  after `tag_dictionary_is_optional_and_its_entries_are_validated` (~line 768)
- Modify: `src/halt.rs:183` and `:206` (`Project` literals)
- Modify: `src/commands/check.rs:89` (insert before the `completed` check)
- Test: `src/needs.rs`, `src/repo.rs` (unit), `tests/cli.rs` (end to end)

**Interfaces:**
- Consumes: `validate_name` (Task 1.1); `crate::format::validate_line`;
  `crate::repo::CONFIG_REL`.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)] #[serde(deny_unknown_fields)] pub struct NeedDecl { pub meaning: String, #[serde(default)] pub exclusive: bool }`.
    `Serialize` is there because `repo::Config` derives it.
  - `pub type Vocabulary = std::collections::BTreeMap<String, NeedDecl>;`
  - `pub fn require_declared(vocab: &Vocabulary, names: &[String]) -> Result<()>`. It
    returns `Error::UnknownNeed` for the first undeclared name, and the detail lists
    the declared names.
  - `pub fn exclusive_of(vocab: &Vocabulary, needs: &[String]) -> Vec<String>`. It
    returns the names that are both declared and exclusive, sorted and deduped. Slice 2
    is the caller.
  - `Error::UnknownNeed(String)`, with kind `"unknown_need"`.
  - `pub needs: crate::needs::Vocabulary` on `repo::Project`, empty when the project
    declares no needs.
  - A `check` finding of kind `unknown_need`, an error, on every record whatever its
    status.
  - The `tests/cli.rs` helper
    `fn declare_needs(dir: &std::path::Path, entries: &[(&str, &str, bool)])`.

- [ ] **Step 1: Write the failing unit tests**

Add to `src/needs.rs`'s `tests` module:

```rust
    fn vocab(entries: &[(&str, bool)]) -> Vocabulary {
        entries
            .iter()
            .map(|(name, exclusive)| {
                (
                    (*name).to_string(),
                    NeedDecl {
                        meaning: format!("{name} meaning"),
                        exclusive: *exclusive,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn require_declared_names_the_first_undeclared_need_and_the_vocabulary() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        assert!(require_declared(&declared, &["quiet".into(), "owner".into()]).is_ok());
        assert!(require_declared(&declared, &[]).is_ok());
        let error = require_declared(&declared, &["quiet".into(), "gpu".into()]).unwrap_err();
        assert_eq!(error.kind(), "unknown_need");
        let detail = error.to_string();
        assert!(detail.contains("\"gpu\""), "{detail}");
        assert!(detail.contains("declared: owner, quiet"), "{detail}");
        let error = require_declared(&Vocabulary::new(), &["quiet".into()]).unwrap_err();
        assert!(error.to_string().contains("none are declared"), "{error}");
    }

    #[test]
    fn exclusive_of_keeps_declared_exclusive_needs_sorted_once() {
        let declared = vocab(&[("quiet", true), ("gpu", true), ("owner", false)]);
        let needs: Vec<String> = ["quiet", "owner", "gpu", "quiet", "undeclared"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(exclusive_of(&declared, &needs), ["gpu", "quiet"]);
        assert!(exclusive_of(&declared, &["owner".into()]).is_empty());
    }
```

Add to `src/repo.rs`'s `tests` module, after
`tag_dictionary_is_optional_and_its_entries_are_validated`:

```rust
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
            ("[needs.quiet]\nmeaning = \"x\"\nexclusive = \"yes\"\n", None),
            ("[needs]\nquiet = \"an idle host\"\n", None),
            (
                "[needs.quiet]\nmeaning = \"\"\n",
                Some("[needs] \"quiet\": meaning must not be empty"),
            ),
            ("[needs.Quiet]\nmeaning = \"x\"\n", Some("[needs] \"Quiet\": need \"Quiet\"")),
        ] {
            std::fs::write(&config, format!("prefix = \"tst\"\n\n{table}")).unwrap();
            let error = Project::open(dir.path()).unwrap_err();
            assert_eq!(error.kind(), "config", "{table}");
            if let Some(fragment) = fragment {
                assert!(error.to_string().contains(fragment), "{table}: {error}");
            }
        }
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one needs::`, then `just test-one need_vocabulary`
Expected: compile failure. `cannot find type Vocabulary`, `NeedDecl`, `require_declared`
and `exclusive_of`, and `no field needs on Project`.

- [ ] **Step 3: Implement the vocabulary**

In `src/needs.rs`, add `use std::collections::{BTreeMap, BTreeSet};` below the existing
`use crate::error::{Error, Result};`, and add after `validate_name`:

```rust
/// One `[needs.<name>]` table (spec §4.1). `meaning` is one required line; `exclusive`
/// says the resource serves one session at a time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedDecl {
    pub meaning: String,
    #[serde(default)]
    pub exclusive: bool,
}

/// A project's declared needs, by name. Committed with the project like `[tags]`.
pub type Vocabulary = BTreeMap<String, NeedDecl>;

fn declared_list(vocab: &Vocabulary) -> String {
    if vocab.is_empty() {
        "none are declared".into()
    } else {
        format!(
            "declared: {}",
            vocab.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

/// Every name must be declared (spec §4.2, on write). The first that is not fails
/// `unknown_need`, naming what the project does declare.
pub fn require_declared(vocab: &Vocabulary, names: &[String]) -> Result<()> {
    match names.iter().find(|name| !vocab.contains_key(*name)) {
        None => Ok(()),
        Some(name) => Err(Error::UnknownNeed(format!(
            "need {name:?} is not declared in {}'s [needs]; {}",
            crate::repo::CONFIG_REL,
            declared_list(vocab)
        ))),
    }
}

/// The needs a claim holds (spec §4.4): those its own project declares exclusive. An
/// undeclared name is ignored. Sorted and deduped.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exclusive holds (slice 2) are its first caller")
)]
pub fn exclusive_of(vocab: &Vocabulary, needs: &[String]) -> Vec<String> {
    needs
        .iter()
        .filter(|need| vocab.get(*need).is_some_and(|decl| decl.exclusive))
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}
```

In `src/error.rs`, after `Halted(String),` (line 38):

```rust
    #[error("{0}")]
    UnknownNeed(String),
```

In `with_suffix`, after the `Error::Halted` arm:
`Error::UnknownNeed(detail) => Error::UnknownNeed(detail + suffix),`
In `kind`, after the `Error::Halted(_)` arm: `Error::UnknownNeed(_) => "unknown_need",`

In `src/repo.rs`:

- `Project`, after `pub feedback: Option<String>,`:

```rust
    /// The need vocabulary from the config's `[needs]` tables: name -> meaning and
    /// exclusivity. Empty when the project declares none. See
    /// docs/specs/2026-10-03-lanes-needs-groups-design.md §4.1.
    pub needs: crate::needs::Vocabulary,
```

- `Config`, after the `attachments` field:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    needs: Option<crate::needs::Vocabulary>,
```

- After `tag_dictionary`:

```rust
/// Each `[needs]` name follows the tag grammar and each meaning is one non-empty line;
/// anything else is a config error naming the entry. Unknown keys and a missing meaning
/// already failed the TOML parse (`deny_unknown_fields`).
fn need_vocabulary(
    raw: Option<crate::needs::Vocabulary>,
) -> Result<crate::needs::Vocabulary> {
    let vocabulary = raw.unwrap_or_default();
    for (name, decl) in &vocabulary {
        crate::needs::validate_name(name)
            .and_then(|()| crate::format::validate_line("meaning", &decl.meaning))
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: [needs] {name:?}: {error}")))?;
    }
    Ok(vocabulary)
}
```

- In the `init` `Config` literal, after `attachments: None,`, add `needs: None,`.
- In `open`, after `feedback: feedback_scope(config.feedback)?,`, add
  `needs: need_vocabulary(config.needs)?,`.

In `src/halt.rs`, in both test `Project` literals (lines 183 and 206), add
`needs: crate::needs::Vocabulary::new(),` after `feedback: None,`.

- [ ] **Step 4: Run the unit tests to see them pass**

Run: `just test-one needs::`, then `just test-one need_vocabulary`
Expected: PASS for `needs::tests::*` and `repo::tests::need_vocabulary_*`.

- [ ] **Step 5: Write the failing `check` test**

Append to `tests/cli.rs`:

```rust
/// Appends one `[needs.<name>]` table per entry to a test project's config.
fn declare_needs(dir: &std::path::Path, entries: &[(&str, &str, bool)]) {
    let path = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for (name, meaning, exclusive) in entries {
        text.push_str(&format!(
            "\n[needs.{name}]\nmeaning = \"{meaning}\"\nexclusive = {exclusive}\n"
        ));
    }
    std::fs::write(path, text).unwrap();
}

#[test]
fn needs_check_errors_on_an_undeclared_need_on_every_record() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    let declared = id_of(env.json(&sci, &["add", "Declared"]));
    let stale = id_of(env.json(&sci, &["add", "Stale"]));
    let closed = id_of(env.json(&sci, &["add", "Closed"]));
    env.json(&sci, &["done", &closed, "landed"]);
    seed_needs(&sci, &declared, "quiet");
    seed_needs(&sci, &stale, "quiet, gpu");
    seed_needs(&sci, &closed, "gpu");

    // Reads stay lenient: the record lists with its undeclared need.
    let listed = env.json(&sci, &["list"]);
    let row = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == stale.as_str())
        .unwrap();
    assert_eq!(row["needs"], serde_json::json!(["quiet", "gpu"]));

    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut errors: Vec<(String, String)> = report["errors"]
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
    errors.sort();
    let mut expected = vec![
        ("unknown_need".to_string(), stale.clone()),
        ("unknown_need".to_string(), closed.clone()),
    ];
    expected.sort();
    assert_eq!(errors, expected, "{report}");
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["detail"].as_str().unwrap().contains("\"gpu\"")),
        "{report}"
    );
}
```

- [ ] **Step 6: Run it to see it fail**

Run: `just test-one --test cli needs_check_`
Expected: FAIL. `check` exits 0, so the `Some(1)` assertion fails.

- [ ] **Step 7: Report undeclared needs from `check`**

In `src/commands/check.rs`, inside the per-task loop, immediately before
`if let Some(completed) = &task.completed` (line 89):

```rust
        // Lanes-needs spec §4.2: reads carry an undeclared need as-is; `check` refuses
        // it on every record, open or closed, since a reopen or an acquire reads it again.
        for need in &task.needs {
            if let Err(error) =
                crate::needs::require_declared(&ctx.project.needs, std::slice::from_ref(need))
            {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "unknown_need",
                    error.to_string(),
                ));
            }
        }
```

- [ ] **Step 8: Run it to see it pass**

Run: `just test-one --test cli needs_check_`
Expected: PASS.

- [ ] **Step 9: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass. Clippy
accepts `exclusive_of` through its `expect(dead_code)`.

```bash
git add src/needs.rs src/error.rs src/repo.rs src/halt.rs src/commands/check.rs tests/cli.rs
git commit -m "feat(needs): declare the need vocabulary and check records against it"
```

---

### Task 1.3: `--need`, `--rm-need`, `--no-needs` on add and edit

**Files:**
- Modify: `src/cli.rs:163-165` (`FieldArgs`, after `tags`) and `:246-251` (`EditArgs`,
  after `no_tags`)
- Modify: `src/commands/mod.rs:640-650` (`apply_fields`, after the `--tag` loop)
- Modify: `src/commands/edit.rs:116-118` (`has_flags`), `:187-196` (after the
  `rm_tags` loop), and `:331` (editor, before `edited.status = original.status;`)
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if the adopt changes it) at the
  ops commit Task 0.1 recorded as `inventory: Task 1.3 = <sha>`
- Test: `tests/cli.rs`, `src/surface.rs` (conformance)

**Ops rows (landed by Task 0.1):** the edit to ops `cli.toml` for this task only.

- In the `tasks add` row's `options`, directly after
  `{ shared = "tag", role = "set", value = "string", repeatable = true },`:

```toml
  { names = ["--need"], value = "string", repeatable = true },
```

- In the `tasks edit` row's `options`, directly after
  `{ names = ["--no-tags"], value = "none" },`:

```toml
  { names = ["--rm-need"], value = "string", repeatable = true },
  { names = ["--no-needs"], value = "none" },
```

  and directly after that row's
  `{ shared = "tag", role = "set", value = "string", repeatable = true },`:

```toml
  { names = ["--need"], value = "string", repeatable = true },
```

**Interfaces:**
- Consumes: `needs::validate_name`, `needs::require_declared` and `Project.needs`
  (Tasks 1.1–1.2); `declare_needs` and `seed_needs`.
- Produces:
  - `pub needs: Vec<String>` on `cli::FieldArgs` (`--need`, value name `NEED`,
    repeatable).
  - `pub rm_needs: Vec<String>` on `cli::EditArgs` (`--rm-need`, conflicts with
    `no_needs`).
  - `pub no_needs: bool` on `cli::EditArgs` (`--no-needs`).
  - The write rules. Only the names being added are validated: grammar first
    (`validation`), then declaration (`unknown_need`). `--rm-need` of a name the task
    does not have is `validation`. An editor save may add only declared names.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn needs_flags_set_append_remove_and_clear() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(
        &sci,
        &[
            ("quiet", "an idle host", true),
            ("owner", "the owner judges an image", false),
            ("gpu", "the GPU", true),
        ],
    );
    let id = id_of(env.json(
        &sci,
        &["add", "Capture", "--need", "quiet", "--need", "owner", "--need", "quiet"],
    ));
    let needs = |env: &TestEnv| env.json(&sci, &["show", &id])["task"]["needs"].clone();
    assert_eq!(needs(&env), serde_json::json!(["quiet", "owner"]), "repeats are no-ops");
    assert!(
        env.read(&sci, &format!("tasks/{id}.md"))
            .contains("\nneeds: [quiet, owner]\n")
    );

    env.json(&sci, &["edit", &id, "--need", "gpu"]);
    assert_eq!(needs(&env), serde_json::json!(["quiet", "owner", "gpu"]), "--need appends");

    env.json(&sci, &["edit", &id, "--rm-need", "owner"]);
    assert_eq!(needs(&env), serde_json::json!(["quiet", "gpu"]));
    assert_eq!(env.fail(&sci, &["edit", &id, "--rm-need", "owner"]), "validation");

    env.json(&sci, &["edit", &id, "--no-needs", "--need", "owner"]);
    assert_eq!(needs(&env), serde_json::json!(["owner"]), "--no-needs --need replaces");

    env.json(&sci, &["edit", &id, "--no-needs"]);
    assert!(env.json(&sci, &["show", &id])["task"].get("needs").is_none());
    assert!(
        !env.read(&sci, &format!("tasks/{id}.md")).contains("needs"),
        "the key is dropped, not written empty"
    );

    env.usage(&sci, &["edit", &id, "--rm-need", "quiet", "--no-needs"]);
}

#[test]
fn needs_flags_refuse_an_undeclared_or_malformed_name() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let _fam = env.init("fam");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);

    assert_eq!(env.fail(&sci, &["add", "Typo", "--need", "gpu"]), "unknown_need");
    assert_eq!(env.fail(&sci, &["add", "Upper", "--need", "Quiet"]), "validation");
    // Validated against the target project: fam declares nothing.
    assert_eq!(
        env.fail(&sci, &["add", "Elsewhere", "--project", "fam", "--need", "quiet"]),
        "unknown_need"
    );
    assert!(
        env.json(&sci, &["list", "--all-projects"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty(),
        "nothing was written"
    );

    let id = id_of(env.json(&sci, &["add", "Capture"]));
    let before = env.read(&sci, &format!("tasks/{id}.md"));
    assert_eq!(env.fail(&sci, &["edit", &id, "--need", "gpu"]), "unknown_need");
    assert_eq!(env.read(&sci, &format!("tasks/{id}.md")), before);

    // An editor save may add only declared names.
    let undeclared = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\nneeds: [gpu]/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &undeclared)
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unknown_need");
    assert_eq!(env.read(&sci, &format!("tasks/{id}.md")), before);
    let declared = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\nneeds: [quiet]/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &declared)
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["needs"],
        serde_json::json!(["quiet"])
    );
}

#[test]
fn needs_flags_a_need_dropped_from_the_vocabulary_still_lists_but_fails_check() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    let id = id_of(env.json(&sci, &["add", "Capture", "--need", "quiet"]));
    std::fs::write(sci.join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();

    let listed = env.json(&sci, &["list"]);
    assert_eq!(listed["tasks"][0]["needs"], serde_json::json!(["quiet"]));
    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["errors"][0]["kind"], "unknown_need", "{report}");

    // Edits that add nothing are not held to the vocabulary, and the stale name can go.
    env.json(&sci, &["edit", &id, "-p", "1"]);
    env.json(&sci, &["edit", &id, "--rm-need", "quiet"]);
    let report = env.check(&sci);
    assert!(report["errors"].as_array().unwrap().is_empty(), "{report}");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --test cli needs_flags_`
Expected: FAIL. clap rejects `--need` as an unexpected argument (exit 2) in each test.

- [ ] **Step 3: Add the flags**

In `src/cli.rs`, `FieldArgs`, directly after the `tags` field:

```rust
    /// Need a shared resource declared in `[needs]` (repeatable). On `edit` this
    /// appends; see `--rm-need` and `--no-needs`.
    #[arg(long = "need", value_name = "NEED")]
    pub needs: Vec<String>,
```

In `EditArgs`, directly after `no_tags`:

```rust
    /// Remove a need (repeatable); `--need` adds one.
    #[arg(long = "rm-need", value_name = "NEED", conflicts_with = "no_needs")]
    pub rm_needs: Vec<String>,
    /// Clear every need; with `--need`, replaces the list wholesale.
    #[arg(long)]
    pub no_needs: bool,
```

In `src/commands/mod.rs`, `apply_fields`, directly after the
`for tag in &fields.tags { ... }` loop:

```rust
    // Additive like `--tag`. Only the names being added are checked: grammar, then the
    // project's vocabulary (lanes-needs spec §4.2), so `edit --rm-need` can still clear
    // a need the vocabulary has since dropped.
    for need in &fields.needs {
        crate::needs::validate_name(need)?;
    }
    crate::needs::require_declared(&ctx.project.needs, &fields.needs)?;
    for need in &fields.needs {
        if !task.needs.contains(need) {
            task.needs.push(need.clone());
        }
    }
```

In `src/commands/edit.rs`:

- In `has_flags`, after `|| args.no_depends`:

```rust
        || !fields.needs.is_empty()
        || !args.rm_needs.is_empty()
        || args.no_needs
```

- Directly after the `for tag in &args.rm_tags { ... }` loop and before
  `apply_fields(...)`:

```rust
    // Mirrors the tag flags: clear, then remove, then `apply_fields` appends.
    if args.no_needs {
        task.needs.clear();
    }
    for need in &args.rm_needs {
        let before = task.needs.len();
        task.needs.retain(|existing| existing != need);
        if task.needs.len() == before {
            return Err(Error::Validation(format!(
                "{} does not need {need:?}",
                task.id
            )));
        }
    }
```

- In `editor`, directly before `edited.status = original.status;`:

```rust
    // Lanes-needs spec §4.2: only names this save adds must be declared (the grammar
    // was checked by `parse_task`); one the vocabulary has since dropped may stay or go.
    let added: Vec<String> = edited
        .needs
        .iter()
        .filter(|need| !original.needs.contains(*need))
        .cloned()
        .collect();
    crate::needs::require_declared(&ctx.project.needs, &added).map_err(keep)?;
```

- [ ] **Step 4: Adopt this task's rows**

In .worktrees/ece1e2-work-selection, run
`python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where `<sha>` is the value of
the `inventory: Task 1.3 = <sha>` note on tasks-ece1e2 (`tasks show tasks-ece1e2`). It
writes `tools/cli.toml` with the **Ops rows** above, and refreshes
`tools/cli_surface.py` when the pair changed. Then run
`just test-one parser_surface_equals_table`. Expected: PASS.

- [ ] **Step 5: Run the focused tests to see them pass**

Run: `just test-one --test cli needs_flags_`, then `just test-one surface`.
Expected: PASS for both. If the surface test reports a difference, change the parser to
match the adopted row. If the row itself is wrong, stop and report it: the correction
lands in ops through Task 0.1's procedure and is adopted again. Never edit
`tools/cli.toml`.

- [ ] **Step 6: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass. Run
`git status`; when it shows `tools/cli_surface.py` new or modified, add it as well.

```bash
git add src/cli.rs src/commands/mod.rs src/commands/edit.rs tools/cli.toml tests/cli.rs
git commit -m "feat(needs): set needs with add and edit flags"
```

---

### Task 1.4: Status and needs change in separate operations

**Files:**
- Modify: `src/cli.rs:193-205` (`EditArgs.status`: `conflicts_with = "defer"` becomes
  `conflicts_with_all`)
- Modify: `src/commands/edit.rs:312-321` (editor, directly after the status/defer
  refusal)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `EditArgs.rm_needs`, `EditArgs.no_needs`, `FieldArgs.needs` (Task 1.3).
- Produces:
  - `edit --status` conflicts at the CLI with `--need`, `--rm-need` and `--no-needs`
    (exit 2).
  - An editor save that changes both status and needs fails `validation` with the
    message "a save that changes the status cannot also change needs; change the status
    first, then the needs".
  - Slice 2 relies on this: every acquire reads the needs already on the record, and
    Slice 2's tests extend these fixtures with `start --force --reason`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn needs_status_flags_conflict_with_every_needs_flag_on_edit() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    let id = id_of(env.json(&sci, &["add", "Capture", "--need", "quiet"]));
    let path = format!("tasks/{id}.md");
    let before = env.read(&sci, &path);
    let needs_flags: [&[&str]; 3] = [&["--need", "quiet"], &["--rm-need", "quiet"], &["--no-needs"]];
    for needs in needs_flags {
        let mut args = vec!["edit", id.as_str(), "--status", "doing"];
        args.extend_from_slice(needs);
        let err = env.usage(&sci, &args);
        assert!(err.contains("--status"), "{needs:?}: {err}");
        assert_eq!(env.read(&sci, &path), before, "{needs:?} wrote nothing");
    }
    // Separately, each lands.
    env.json(&sci, &["edit", &id, "--status", "blocked"]);
    env.json(&sci, &["edit", &id, "--no-needs"]);
    let task = env.json(&sci, &["show", &id])["task"].clone();
    assert_eq!(task["status"], "blocked");
    assert!(task.get("needs").is_none());
}

#[test]
fn needs_status_editor_save_cannot_change_status_and_needs_together() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    let id = id_of(env.json(&sci, &["add", "Capture", "--status", "idea"]));
    let path = format!("tasks/{id}.md");
    let before = env.read(&sci, &path);

    let both = editor_script(
        &sci,
        "sed -i -e 's/^status: idea$/status: todo/' -e 's/^priority: 2$/priority: 2\\nneeds: [quiet]/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &both)
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains(
            "a save that changes the status cannot also change needs; change the status first, then the needs"
        ),
        "{}",
        err_detail(&out)
    );
    assert_eq!(env.read(&sci, &path), before, "the record is unchanged");

    let status = editor_script(&sci, "sed -i 's/^status: idea$/status: todo/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &status)
        .assert()
        .success();
    let needs = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\nneeds: [quiet]/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &needs)
        .assert()
        .success();
    let task = env.json(&sci, &["show", &id])["task"].clone();
    assert_eq!(task["status"], "todo");
    assert_eq!(task["needs"], serde_json::json!(["quiet"]));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --test cli needs_status_`
Expected: FAIL. The first test gets exit 0 where it expects exit 2 (the CLI pair is
accepted). The second gets exit 0 where it expects a `validation` error.

- [ ] **Step 3: Implement both refusals**

In `src/cli.rs`, `EditArgs.status`, replace `conflicts_with = "defer",` with:

```rust
        conflicts_with_all = ["defer", "needs", "rm_needs", "no_needs"],
```

Extend its doc comment with this line:
`/// Status, defer, and needs change in separate operations.`

In `src/commands/edit.rs`, `editor`, directly after the block that returns
`"a save that changes the status cannot also change defer; ..."`:

```rust
    // Lanes-needs spec §4.4: status and needs change in separate operations, as status
    // and defer do, so every acquire reads the needs already on the record.
    if status != original.status && edited.needs != original.needs {
        return Err(keep(Error::Validation(
            "a save that changes the status cannot also change needs; change the status \
             first, then the needs"
                .into(),
        )));
    }
```

- [ ] **Step 4: Run them to see them pass**

Run: `just test-one --test cli needs_status_`, then `just test-one surface`.
Expected: PASS. The surface table does not record conflicts, so this task has no ops
rows and `tools/cli.toml` is unchanged.

- [ ] **Step 5: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass.

```bash
git add src/cli.rs src/commands/edit.rs tests/cli.rs
git commit -m "feat(edit): change status and needs in separate operations"
```

---

### Task 1.5: `--need` filters `list` and `ready`

**Files:**
- Modify: `src/cli.rs:98-100` (`FilterArgs`, after `tags`)
- Modify: `src/filter.rs:18-29` (`TaskFilter`), `:32-44` (`Fields`), `:66-98`
  (`parse`), `:106-117` (`is_empty`), `:119-136` (`matches`), `:139-185`
  (`of_task`/`of_row`), and the tests (`fields()` helper and a new test)
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if the adopt changes it) at the
  ops commit Task 0.1 recorded as `inventory: Task 1.5 = <sha>`
- Test: `src/filter.rs` (unit), `tests/cli.rs`, `src/surface.rs`

**Ops rows (landed by Task 0.1):** the edit to ops `cli.toml` for this task only. In both
the `tasks list` and the `tasks ready` rows, directly after the
`{ shared = "tag", role = "filter", ... }` line:

```toml
  { names = ["--need"], value = "string", repeatable = true },
```

The ops validator requires `exception` only on shared rows, and `--need` is not shared.

**Interfaces:**
- Consumes: `Task.needs`, `ParkedRow.needs` (Task 1.1); `declare_needs`; `add --need`
  (Task 1.3).
- Produces:
  - `pub needs: Vec<String>` on `cli::FilterArgs` (`--need`, repeatable, all-of).
  - `needs: Vec<String>` on `filter::TaskFilter`.
  - `pub needs: &'a [String]` on `filter::Fields<'a>`.
  - The `tests/cli.rs` helper `fn task_ids(v: &serde_json::Value) -> Vec<String>`,
    which returns the rows of a `{tasks: [...]}` payload in payload order.

- [ ] **Step 1: Write the failing tests**

In `src/filter.rs` tests, add `needs: &[],` to the `fields()` helper after `tags: NO_TAGS,`
(that helper then fails to compile until Step 3), and add:

```rust
    #[test]
    fn needs_are_all_of_like_tags() {
        let needs = vec!["quiet".to_string(), "owner".to_string()];
        let needy = Fields {
            needs: &needs,
            ..fields()
        };
        let one = TaskFilter {
            needs: vec!["quiet".into()],
            ..TaskFilter::default()
        };
        assert!(!one.is_empty());
        assert!(one.matches(&needy));
        assert!(!one.matches(&fields()));
        let both = TaskFilter {
            needs: vec!["quiet".into(), "owner".into()],
            ..TaskFilter::default()
        };
        assert!(both.matches(&needy));
        let extra = TaskFilter {
            needs: vec!["quiet".into(), "gpu".into()],
            ..TaskFilter::default()
        };
        assert!(!extra.matches(&needy));
    }
```

Append to `tests/cli.rs`:

```rust
/// The `id` of every row of a `{tasks: [...]}` payload, in payload order.
fn task_ids(v: &serde_json::Value) -> Vec<String> {
    v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn needs_filter_list_ready_and_parked_select_all_of() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(
        &sci,
        &[("quiet", "an idle host", true), ("owner", "the owner judges", false)],
    );
    let quiet = id_of(env.json(&sci, &["add", "Quiet", "--need", "quiet"]));
    let both = id_of(env.json(
        &sci,
        &["add", "Both", "--need", "quiet", "--need", "owner"],
    ));
    env.json(&sci, &["add", "Plain"]);
    let sorted = |v: serde_json::Value| {
        let mut ids = task_ids(&v);
        ids.sort();
        ids
    };
    let mut either = vec![quiet.clone(), both.clone()];
    either.sort();
    assert_eq!(sorted(env.json(&sci, &["list", "--need", "quiet"])), either);
    assert_eq!(sorted(env.json(&sci, &["ready", "--need", "quiet"])), either);
    assert_eq!(
        sorted(env.json(&sci, &["list", "--need", "quiet", "--need", "owner"])),
        [both.clone()],
        "repeats narrow, like --tag"
    );
    assert_eq!(
        sorted(env.json(&sci, &["ready", "--need", "quiet", "--need", "owner"])),
        [both.clone()]
    );
    env.json(&sci, &["park", &quiet, "Rerun"]);
    env.json(&sci, &["park", &both, "Judge"]);
    assert_eq!(
        sorted(env.json(&sci, &["list", "--parked", "--need", "owner"])),
        [both.clone()]
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one filter::tests::needs_`, then
`just test-one --test cli needs_filter_`.
Expected: the unit test fails to compile (no field `needs` on `Fields` or
`TaskFilter`). The CLI test fails because clap rejects `list --need` (exit 2).

- [ ] **Step 3: Implement the filter**

In `src/cli.rs`, `FilterArgs`, directly after `tags`:

```rust
    /// Filter by need (repeatable); a task must need every one.
    #[arg(long = "need", value_name = "NEED")]
    pub needs: Vec<String>,
```

Make these changes in `src/filter.rs`. First, update the module doc's last sentence to
"`--tag` and `--need` are all-of." Then:

- `TaskFilter`: after `tags: Vec<String>,`, add `needs: Vec<String>,`.
- `Fields`: after `pub tags: &'a [String],`, add `pub needs: &'a [String],`.
- `parse`: after `tags: args.tags.clone(),`, add `needs: args.needs.clone(),`.
- `is_empty`: after `&& self.tags.is_empty()`, add `&& self.needs.is_empty()`.
- `matches`: after `&& self.tags.iter().all(|tag| fields.tags.contains(tag))`, add:

```rust
            && self.needs.iter().all(|need| fields.needs.contains(need))
```

- `of_task`: after `tags: &task.tags,`, add `needs: &task.needs,`.
- `of_row`: after `tags: &row.tags,`, add `needs: &row.needs,`.

- [ ] **Step 4: Adopt this task's rows**

In .worktrees/ece1e2-work-selection, run
`python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where `<sha>` is the value of
the `inventory: Task 1.5 = <sha>` note on tasks-ece1e2 (`tasks show tasks-ece1e2`). It
adopts the **Ops rows** above on top of Task 1.3's, and refreshes
`tools/cli_surface.py` when the pair changed. Then run
`just test-one parser_surface_equals_table`. Expected: PASS.

- [ ] **Step 5: Run the focused tests to see them pass**

Run `just test-one filter::`, `just test-one --test cli needs_filter_` and
`just test-one surface`.
Expected: PASS for all three. A surface difference is handled as in Task 1.3 Step 5:
never by editing `tools/cli.toml`.

- [ ] **Step 6: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass. Run
`git status`; when it shows `tools/cli_surface.py` new or modified, add it as well.

```bash
git add src/cli.rs src/filter.rs tools/cli.toml tests/cli.rs
git commit -m "feat(list): filter list and ready by need"
```

---

### Task 1.6: `--without` and `TASKS_WITHOUT` on ready, next, and prime

**Files:**
- Modify: `src/needs.rs` (`WITHOUT_ENV`, `Vocabularies`, `vocabulary_of` and `Without`,
  with unit tests)
- Modify: `src/cli.rs` (new `WithoutArgs` after `FilterArgs`, ~line 114; flattened into
  `Command::Ready` ~383, `Command::Next` ~403 and `Command::Prime` ~623)
- Modify: `src/commands/mod.rs:1332-1349` (the `Ready`/`Next`/`Prime` dispatch arms)
- Modify: `src/commands/list.rs` (imports; a new `vocabularies` helper; `ready`
  :301-340; `next` :343-380; `prime` :463-510)
- Modify: `tests/common/mod.rs` (`cmd`, `raw` and `shim_command`)
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if the adopt changes it) at the
  ops commit Task 0.1 recorded as `inventory: Task 1.6 = <sha>`
- Test: `src/needs.rs` (unit), `tests/cli.rs`, `src/surface.rs`

**Ops rows (landed by Task 0.1):** the edit to ops `cli.toml` for this task only. Add the
following line directly after the `--max-complexity` line of the `tasks ready` row and of
the `tasks next` row, and a third time directly after
`{ names = ["--closed"], value = "none" },` in the `tasks prime` row:

```toml
  { names = ["--without"], value = "string", repeatable = true },
```

**Where a withheld name hides (spec §4.3).** A need means what the task's own project
declares, so hiding is decided per task against that project's vocabulary, never against
the union of the scope's. The two sources differ only at resolution:

- `TASKS_WITHOUT` names are lenient: any name is accepted.
- `--without` names are strict: one that no project in scope declares is refused with
  `unknown_need`, because it is a typo.

Once resolved, both behave the same way. A name hides a task only when the task needs it
**and** the task's project declares it. So a flag name hides in the projects that declare
it, and nowhere else. A record still carrying `needs: [quiet]` in a project whose
`[needs]` no longer declares `quiet` is not hidden by either source; `tasks check`
already reports it as `unknown_need` (Task 1.2). Because the hiding rule is the same for
both sources, `Without` keeps one name set; strictness lives only in `resolve`.

**Interfaces:**
- Consumes: `Vocabulary`, `Error::UnknownNeed` and `Project.needs` (Task 1.2);
  `Task.needs`; `add --need` (Task 1.3); `task_ids`, `declare_needs`, `seed_needs`,
  `warnings_of` and `err_kind`.
- Produces in `src/needs.rs`:
  - `pub const WITHOUT_ENV: &str = "TASKS_WITHOUT";`
  - `pub type Vocabularies<'a> = std::collections::HashMap<&'a str, &'a Vocabulary>;`:
    each in-scope project's vocabulary, by prefix.
  - `pub fn vocabulary_of<'a>(vocabs: &Vocabularies<'a>, task: &Task) -> &'a Vocabulary`.
    It panics when the task's prefix is not in the map. Every task a view filters was
    scanned from a project in scope, and `Project::scan` refuses a record whose prefix
    differs from its project's, so a miss is a caller bug.
  - `#[derive(Debug, Default, Clone, PartialEq, Eq)] pub struct Without { names: BTreeSet<String> }`
    with these methods:
    - `pub fn resolve(flag: &[String], env: Option<&str>, vocabs: &Vocabularies<'_>) -> Result<Without>`.
      Flag names are strict: one that no vocabulary in `vocabs` declares is
      `unknown_need`. Env names are lenient: the value is split on `,`, trimmed, and empty
      entries dropped. The result is the union of the two.
    - `pub fn from_env(flag: &[String], vocabs: &Vocabularies<'_>) -> Result<Without>`.
      It reads `TASKS_WITHOUT`; non-UTF-8 is `validation`.
    - `pub fn hides(&self, task: &Task, vocab: &Vocabulary) -> bool`, where `vocab` is
      the vocabulary of the task's own project. True when the task needs a withheld name
      that `vocab` declares.
    - `pub fn is_empty(&self) -> bool`
    - `pub fn names(&self) -> impl Iterator<Item = &str>`
    - `pub fn retain(&self, tasks: &mut Vec<Task>, vocabs: &Vocabularies<'_>) -> usize`. It
      looks up each task's vocabulary with `vocabulary_of`, drops the hidden tasks, and
      returns how many went.
    - `pub fn warning(&self, hidden: usize) -> Option<String>`. The text is
      `without <names>: <n> task(s) hidden`.
- Produces in `src/cli.rs`:
  `pub struct WithoutArgs { #[arg(long = "without")] pub without: Vec<String> }`.
- Produces in `src/commands/list.rs`:
  - `pub(super) fn vocabularies(scope: &Scope) -> crate::needs::Vocabularies<'_>`. It
    takes the scope, not the `ReadCtx`, so a caller can hold the map while it pushes to
    `ctx.warnings`. The lanes view in Slice 3 reuses it.
  - These new signatures:
    - `ready(ctx, filter, limit, max_complexity, without: Vec<String>)`
    - `next(ctx, max_complexity, without: Vec<String>)`
    - `prime(ctx, closed, without: Vec<String>)`

- [ ] **Step 1: Write the failing unit tests**

Add to `src/needs.rs`'s `tests` module:

```rust
    fn task_needing(id: &str, needs: &str) -> Task {
        crate::format::parse_task(
            &format!(
                "---\nid: {id}\ntitle: T\nstatus: todo\npriority: 2\nneeds: [{needs}]\n\
                 created: 2026-10-03T00:00:00Z\nupdated: 2026-10-03T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
            "x",
        )
        .unwrap()
    }

    fn names(without: &Without) -> Vec<&str> {
        without.names().collect()
    }

    #[test]
    fn the_variable_is_trimmed_lenient_and_joins_the_strict_flag() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        let sci = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&[], Some(",quiet, "), &sci).unwrap();
        assert_eq!(names(&without), ["quiet"], "whitespace and empty entries drop");
        let without = Without::resolve(&[], Some(""), &sci).unwrap();
        assert!(without.is_empty(), "an empty variable contributes nothing");
        assert!(Without::resolve(&[], None, &sci).unwrap().is_empty());
        let without = Without::resolve(&[], Some("gpu"), &sci).unwrap();
        assert_eq!(names(&without), ["gpu"], "an undeclared variable name is no error");
        let without = Without::resolve(&["owner".into()], Some("quiet"), &sci).unwrap();
        assert_eq!(names(&without), ["owner", "quiet"], "the flag adds to the variable");

        let error = Without::resolve(&["gpu".into()], Some("quiet"), &sci).unwrap_err();
        assert_eq!(error.kind(), "unknown_need", "the flag is strict");
        let none = Vocabulary::new();
        let both = Vocabularies::from([("fam", &none), ("sci", &declared)]);
        assert!(
            Without::resolve(&["quiet".into()], None, &both).is_ok(),
            "one project in scope declaring it is enough"
        );
        let fam = Vocabularies::from([("fam", &none)]);
        let error = Without::resolve(&["quiet".into()], None, &fam).unwrap_err();
        assert_eq!(error.kind(), "unknown_need");
    }

    #[test]
    fn without_hides_a_task_needing_any_withheld_name_and_counts_it() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        let sci = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&["quiet".into()], None, &sci).unwrap();
        assert!(without.hides(&task_needing("sci-000001", "owner, quiet"), &declared));
        assert!(!without.hides(&task_needing("sci-000001", "owner"), &declared));
        let mut tasks = vec![
            task_needing("sci-000001", "quiet"),
            task_needing("sci-000002", "owner"),
        ];
        assert_eq!(without.retain(&mut tasks, &sci), 1);
        assert_eq!(tasks[0].needs, ["owner"]);
        assert_eq!(
            without.warning(1).as_deref(),
            Some("without quiet: 1 task(s) hidden")
        );
        assert_eq!(without.warning(0), None);
    }

    #[test]
    fn without_hides_only_where_the_owning_project_declares_the_name() {
        // sci declares quiet; fam declares nothing but keeps a record naming quiet. The
        // union of the two vocabularies declares quiet, so a union check would hide both.
        let declared = vocab(&[("quiet", true)]);
        let none = Vocabulary::new();
        let scope = Vocabularies::from([("sci", &declared), ("fam", &none)]);
        let needy = task_needing("sci-000001", "quiet");
        let stale = task_needing("fam-000001", "quiet");
        for (source, without) in [
            ("variable", Without::resolve(&[], Some("quiet"), &scope).unwrap()),
            ("flag", Without::resolve(&["quiet".into()], None, &scope).unwrap()),
        ] {
            assert!(without.hides(&needy, &declared), "{source}");
            assert!(!without.hides(&stale, &none), "{source}: fam never declared quiet");
            assert!(
                std::ptr::eq(vocabulary_of(&scope, &stale), &none),
                "{source}: the task's own project supplies the vocabulary"
            );
            let mut tasks = vec![needy.clone(), stale.clone()];
            assert_eq!(without.retain(&mut tasks, &scope), 1, "{source}");
            assert_eq!(tasks.len(), 1, "{source}");
            assert_eq!(tasks[0].id, stale.id, "{source}");
        }
    }

    #[test]
    #[should_panic(expected = "not in the vocabularies in scope")]
    fn a_task_from_outside_the_scope_is_a_caller_bug() {
        let declared = vocab(&[("quiet", true)]);
        let scope = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&[], Some("quiet"), &scope).unwrap();
        let _ = without.retain(&mut vec![task_needing("fam-000001", "quiet")], &scope);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one needs::`
Expected: compile failure, `cannot find type Without` and `cannot find type Vocabularies`.

- [ ] **Step 3: Implement `Without`**

In `src/needs.rs`, replace `use std::collections::{BTreeMap, BTreeSet};` with
`use std::collections::{BTreeMap, BTreeSet, HashMap};`, add `use crate::model::Task;`
to the imports, and add:

```rust
/// The variable a session sets for the needs it cannot meet (spec §4.3).
pub const WITHOUT_ENV: &str = "TASKS_WITHOUT";

/// Each in-scope project's vocabulary, by prefix. A task's needs mean what its own
/// project declares, so a view looks a task's vocabulary up here rather than in the
/// union of the scope's.
pub type Vocabularies<'a> = HashMap<&'a str, &'a Vocabulary>;

/// The vocabulary of the project `task` belongs to. Every task a view filters was scanned
/// from a project in scope, and `Project::scan` refuses a record whose prefix is not its
/// project's, so a miss is a bug in the caller, not a state to tolerate.
pub fn vocabulary_of<'a>(vocabs: &Vocabularies<'a>, task: &Task) -> &'a Vocabulary {
    vocabs
        .get(task.id.prefix.as_str())
        .copied()
        .unwrap_or_else(|| panic!("{}: project {} is not in the vocabularies in scope", task.id, task.id.prefix))
}

/// The needs a session cannot meet: the union of `--without` and `TASKS_WITHOUT`. A task
/// is hidden from `ready`, `next`, and `prime` when it needs one of them that its own
/// project declares.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Without {
    names: BTreeSet<String>,
}

impl Without {
    /// `flag` is strict: a name no vocabulary in scope declares is a typo, refused with
    /// `unknown_need`. `env` is lenient: set once per host for every project, so any name
    /// is accepted. Entries are comma-separated, trimmed, and empty ones dropped; an
    /// empty variable contributes nothing. Strictness ends here: `hides` treats every
    /// name alike, hiding only in the projects that declare it.
    pub fn resolve(flag: &[String], env: Option<&str>, vocabs: &Vocabularies<'_>) -> Result<Without> {
        let mut names = BTreeSet::new();
        for name in flag {
            if !vocabs.values().any(|vocab| vocab.contains_key(name)) {
                return Err(Error::UnknownNeed(format!(
                    "--without {name:?}: no project in scope declares this need in {}'s [needs]",
                    crate::repo::CONFIG_REL
                )));
            }
            names.insert(name.clone());
        }
        for name in env
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            names.insert(name.to_string());
        }
        Ok(Without { names })
    }

    /// `resolve` with the variable read from the process environment.
    pub fn from_env(flag: &[String], vocabs: &Vocabularies<'_>) -> Result<Without> {
        let env = match std::env::var(WITHOUT_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(Error::Validation(format!(
                    "{WITHOUT_ENV} is not valid UTF-8"
                )));
            }
        };
        Self::resolve(flag, env.as_deref(), vocabs)
    }

    /// `vocab` is the vocabulary of `task`'s own project. A name that project does not
    /// declare hides nothing there (spec §4.3), even when another project in scope
    /// declares it.
    pub fn hides(&self, task: &Task, vocab: &Vocabulary) -> bool {
        task.needs
            .iter()
            .any(|need| self.names.contains(need) && vocab.contains_key(need))
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Drops the hidden tasks, each judged by its own project's vocabulary, and returns
    /// how many went.
    pub fn retain(&self, tasks: &mut Vec<Task>, vocabs: &Vocabularies<'_>) -> usize {
        let before = tasks.len();
        tasks.retain(|task| !self.hides(task, vocabulary_of(vocabs, task)));
        before - tasks.len()
    }

    /// One warning per view, like the complexity cutoff's: nothing is hidden silently.
    pub fn warning(&self, hidden: usize) -> Option<String> {
        (hidden > 0).then(|| {
            format!(
                "without {}: {hidden} task(s) hidden",
                self.names().collect::<Vec<_>>().join(", ")
            )
        })
    }
}
```

- [ ] **Step 4: Run the unit tests to see them pass**

Run: `just test-one needs::`
Expected: PASS. Clippy's dead-code check would still fail at this point, because nothing
outside tests calls `Without` yet. Do not commit before Step 8.

- [ ] **Step 5: Write the failing end-to-end tests**

In `tests/common/mod.rs`, add `.env_remove("TASKS_WITHOUT")` after
`.env_remove("TASKS_MAX_COMPLEXITY")` in both `cmd` and `raw`, and after
`.env_remove("TASKS_AGENT")` in `shim_command`. A host that sets the variable must not
leak it into the suite.

Append to `tests/cli.rs`:

```rust
#[test]
fn needs_without_hides_needy_steps_from_ready_next_and_prime() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    declare_needs(
        &sci,
        &[("quiet", "an idle host", true), ("owner", "the owner judges", false)],
    );
    let quiet = id_of(env.json(&sci, &["add", "Capture", "-p", "0", "--need", "quiet"]));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));

    assert_eq!(task_ids(&env.json(&sci, &["ready"])), [quiet.clone(), plain.clone()]);
    let v = env.json(&sci, &["ready", "--without", "quiet"]);
    assert_eq!(task_ids(&v), [plain.clone()]);
    assert!(
        warnings_of(&v).contains(&"without quiet: 1 task(s) hidden".to_string()),
        "{v}"
    );
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], quiet.as_str());
    assert_eq!(
        env.json(&sci, &["next", "--without", "quiet"])["next"]["task"]["id"],
        plain.as_str()
    );
    let prime = env.json(&sci, &["prime", "--without", "quiet"]);
    let ready: Vec<&str> = prime["ready"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ready, [plain.as_str()], "{prime}");

    // A parked candidate goes through the same gate: next takes it first until hidden.
    let parked = id_of(env.json(&sci, &["add", "Recapture", "-p", "3", "--need", "quiet"]));
    env.json(&sci, &["park", &parked, "Rerun the capture"]);
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], parked.as_str());
    assert_eq!(
        env.json(&sci, &["next", "--without", "quiet"])["next"]["task"]["id"],
        plain.as_str()
    );
    // Withholding a need nothing ready uses hides nothing, and says nothing.
    let v = env.json(&sci, &["ready", "--without", "owner"]);
    assert_eq!(task_ids(&v).len(), 3);
    assert!(!warnings_of(&v).iter().any(|w| w.starts_with("without ")), "{v}");
}

#[test]
fn needs_without_refuses_a_name_no_project_in_scope_declares() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    env.json(&sci, &["add", "Capture", "--need", "quiet"]);
    let fam_task = id_of(env.json(&fam, &["add", "Fam work"]));

    for command in ["ready", "next", "prime"] {
        assert_eq!(
            env.fail(&fam, &[command, "--without", "quiet"]),
            "unknown_need",
            "{command}: fam declares no needs"
        );
        assert_eq!(
            env.fail(&sci, &[command, "--without", "gpu"]),
            "unknown_need",
            "{command}: a typo"
        );
        assert_eq!(
            env.fail(&sci, &[command, "--project", "fam", "--without", "quiet"]),
            "unknown_need",
            "{command}: the scope, not the cwd, supplies the vocabulary"
        );
    }
    // Under --all-projects one declaring project is enough; only its task is hidden.
    let v = env.json(&fam, &["ready", "--all-projects", "--without", "quiet"]);
    assert_eq!(task_ids(&v), [fam_task.clone()]);
    assert_eq!(
        env.json(&fam, &["next", "--all-projects", "--without", "quiet"])["next"]["task"]["id"],
        fam_task.as_str()
    );
    env.json(&fam, &["prime", "--all-projects", "--without", "quiet"]);
}

#[test]
fn needs_without_variable_is_lenient_trimmed_and_joins_the_flag() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    declare_needs(
        &sci,
        &[("quiet", "an idle host", true), ("owner", "the owner judges", false)],
    );
    let quiet = id_of(env.json(&sci, &["add", "Capture", "--need", "quiet"]));
    let owner = id_of(env.json(&sci, &["add", "Review", "--need", "owner"]));
    let plain = id_of(env.json(&sci, &["add", "Plain"]));
    let fam_task = id_of(env.json(&fam, &["add", "Fam work"]));
    let with_env = |dir: &std::path::Path, value: &str, args: &[&str]| -> serde_json::Value {
        let out = env
            .cmd(dir)
            .env("TASKS_WITHOUT", value)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let sorted = |v: serde_json::Value| {
        let mut ids = task_ids(&v);
        ids.sort();
        ids
    };

    let mut expected = vec![owner.clone(), plain.clone()];
    expected.sort();
    assert_eq!(sorted(with_env(&sci, ",quiet, ", &["ready"])), expected);
    let prime = with_env(&sci, "quiet", &["prime"]);
    assert!(
        !prime["ready"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == quiet.as_str()),
        "{prime}"
    );
    assert_eq!(sorted(with_env(&sci, "", &["ready"])).len(), 3, "empty hides nothing");

    // A project that never declares the name: nothing hidden, no error, on every view.
    assert_eq!(sorted(with_env(&fam, "quiet", &["ready"])), [fam_task.clone()]);
    assert_eq!(
        with_env(&fam, "quiet", &["next"])["next"]["task"]["id"],
        fam_task.as_str()
    );
    with_env(&fam, "quiet", &["prime"]);

    // The union: the variable withholds owner, the flag adds quiet.
    assert_eq!(
        sorted(with_env(&sci, "owner", &["ready", "--without", "quiet"])),
        [plain.clone()]
    );
    // The flag stays strict under the variable.
    let out = env
        .cmd(&sci)
        .env("TASKS_WITHOUT", "quiet")
        .args(["ready", "--without", "gpu"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unknown_need");
}

#[test]
fn needs_without_hides_only_where_the_owning_project_declares_the_name() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    declare_needs(&sci, &[("quiet", "an idle host", true)]);
    let capture = id_of(env.json(&sci, &["add", "Capture", "-p", "0", "--need", "quiet"]));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    // fam declares no needs, yet its record still names quiet (say, from a vocabulary
    // since dropped). The union of the scope's vocabularies declares quiet; fam's does not.
    let stale = id_of(env.json(&fam, &["add", "Stale", "-p", "1"]));
    seed_needs(&fam, &stale, "quiet");
    let run = |variable: Option<&str>, args: &[&str]| -> serde_json::Value {
        let mut cmd = env.cmd(&fam);
        if let Some(value) = variable {
            cmd.env("TASKS_WITHOUT", value);
        }
        let out = cmd.args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };

    // In fam alone the variable hides nothing, on every view.
    assert_eq!(task_ids(&run(Some("quiet"), &["ready"])), [stale.clone()]);
    assert_eq!(
        run(Some("quiet"), &["next"])["next"]["task"]["id"],
        stale.as_str()
    );
    let prime = run(Some("quiet"), &["prime"]);
    assert!(
        prime["ready"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == stale.as_str()),
        "{prime}"
    );

    // Across both projects the variable and the flag each hide sci's capture, and only it.
    assert_eq!(
        run(None, &["next", "--all-projects"])["next"]["task"]["id"],
        capture.as_str()
    );
    let mut expected = vec![plain.clone(), stale.clone()];
    expected.sort();
    for (source, variable, args) in [
        ("variable", Some("quiet"), &["--all-projects"][..]),
        ("flag", None, &["--all-projects", "--without", "quiet"][..]),
    ] {
        let v = run(variable, &[&["ready"][..], args].concat());
        let mut ids = task_ids(&v);
        ids.sort();
        assert_eq!(ids, expected, "{source}: {v}");
        assert!(
            warnings_of(&v).contains(&"without quiet: 1 task(s) hidden".to_string()),
            "{source}: {v}"
        );
        assert_eq!(
            run(variable, &[&["next"][..], args].concat())["next"]["task"]["id"],
            stale.as_str(),
            "{source}"
        );
        let prime = run(variable, &[&["prime"][..], args].concat());
        let ready: Vec<&str> = prime["ready"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect();
        assert!(!ready.contains(&capture.as_str()), "{source}: {prime}");
        assert!(ready.contains(&stale.as_str()), "{source}: {prime}");
    }
}
```

- [ ] **Step 6: Run them to see them fail**

Run: `just test-one --test cli needs_without_`
Expected: FAIL. clap rejects `--without` (exit 2), and the variable hides nothing.

- [ ] **Step 7: Wire `--without` into the pickers**

In `src/cli.rs`, directly after `FilterArgs`:

```rust
/// The needs this session cannot meet, shared by the pickers (lanes-needs spec §4.3).
#[derive(Args, Debug, Default, Clone)]
pub struct WithoutArgs {
    /// Hide tasks that need this (repeatable), in the projects that declare it; adds to
    /// TASKS_WITHOUT. Refused when no project in scope declares it.
    #[arg(long = "without", value_name = "NEED")]
    pub without: Vec<String>,
}
```

Add a flattened field to `Command::Ready` (after `max_complexity`), `Command::Next`
(after `max_complexity`) and `Command::Prime` (after `closed`):

```rust
        #[command(flatten)]
        without: WithoutArgs,
```

In `src/commands/mod.rs`, replace the three dispatch arms:

```rust
        Command::Ready {
            filter,
            limit,
            max_complexity,
            without,
            scope,
        } => list::ready(
            open_read_ctx(dir, &scope)?,
            filter,
            limit,
            max_complexity,
            without.without,
        ),
        Command::Next {
            max_complexity,
            without,
            scope,
        } => list::next(open_read_ctx(dir, &scope)?, max_complexity, without.without),
```

```rust
        Command::Prime {
            scope,
            closed,
            without,
        } => list::prime(open_read_ctx(dir, &scope)?, closed, without.without),
```

In `src/commands/list.rs`, add `use crate::needs::{Vocabularies, Without};` to the
imports, and directly after `warn_hidden`:

```rust
/// Every in-scope project's vocabulary, by prefix: the names `--without` may use, and
/// the vocabulary each task's needs are judged by (spec §4.3). It borrows the scope
/// alone, so a caller may hold it while pushing to `ctx.warnings`.
pub(super) fn vocabularies(scope: &Scope) -> Vocabularies<'_> {
    scope
        .projects()
        .iter()
        .map(|project| (project.prefix.as_str(), &project.needs))
        .collect()
}
```

`ready`: add the parameter `without: Vec<String>` after `max_complexity`. After
`let cutoff = ...;`, add:

```rust
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
```

Then, directly after `warn_hidden(&mut ctx, hidden);` and before
`if let Some(cutoff) = cutoff {`, add:

```rust
    if !without.is_empty() {
        let vocabs = vocabularies(&ctx.scope);
        let hidden = without.retain(&mut picked.tasks, &vocabs);
        let _ = without.retain(&mut picked.deferred, &vocabs);
        ctx.warnings.extend(without.warning(hidden));
    }
```

`next`: the signature becomes
`pub fn next(mut ctx: ReadCtx, max_complexity: Option<String>, without: Vec<String>) -> Result<Output>`.
After `let cutoff = ...;`, add the same
`let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;` line. After the
`for task in ready.deferred { ... }` loop that fills `omitted`, and before
`if let Some(cutoff) = cutoff {`, add the following. The pool holds the parked
candidates too, so they pass the same gate.

```rust
    if !without.is_empty() {
        let vocabs = vocabularies(&ctx.scope);
        let hidden = without.retain(&mut pool, &vocabs);
        let _ = without.retain(&mut omitted, &vocabs);
        ctx.warnings.extend(without.warning(hidden));
    }
```

`prime`: the signature becomes
`pub fn prime(mut ctx: ReadCtx, closed: bool, without: Vec<String>) -> Result<Output>`.
After `let cutoff = crate::complexity::cutoff(None)?;`, add the same `from_env` line.
Directly after `warn_hidden(&mut ctx, hidden);`, add:

```rust
    if !without.is_empty() {
        let hidden = without.retain(&mut ready, &vocabularies(&ctx.scope));
        ctx.warnings.extend(without.warning(hidden));
    }
```

Each block takes the map inside it: `halt_snapshots`, `ready_tasks` and `warn_hidden`
borrow `ctx` mutably, so the map cannot be held across them.

- [ ] **Step 8: Adopt this task's rows**

In .worktrees/ece1e2-work-selection, run
`python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where `<sha>` is the value of
the `inventory: Task 1.6 = <sha>` note on tasks-ece1e2 (`tasks show tasks-ece1e2`). It
adopts the **Ops rows** above on top of Tasks 1.3 and 1.5's, and refreshes
`tools/cli_surface.py` when the pair changed. Then run
`just test-one parser_surface_equals_table`. Expected: PASS.

- [ ] **Step 9: Run the focused tests to see them pass**

Run `just test-one needs::`, `just test-one --test cli needs_without_` and
`just test-one surface`. Then run `just test-one --test cli cutoff` to confirm the
cutoff tests are unchanged.
Expected: PASS for all. A surface difference is handled as in Task 1.3 Step 5: never by
editing `tools/cli.toml`.

- [ ] **Step 10: Run the fast suite and the check, then commit**

Run `cargo fmt`, then `just test-fast` and `just check`. Expected: both pass. All
existing picker, park, halt and filter tests pass unchanged. Run `git status`; when it
shows `tools/cli_surface.py` new or modified, add it as well.

```bash
git add src/needs.rs src/cli.rs src/commands/mod.rs src/commands/list.rs \
  tests/common/mod.rs tools/cli.toml tests/cli.rs
git commit -m "feat(ready): hide steps a session cannot meet with --without and TASKS_WITHOUT"
```

---

### Task 1.7: Document needs in the design, skills, and README

This task covers items 1–5 of the main design's §3.4 field checklist. Item 6 (`src`)
landed in Tasks 1.1–1.3.

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md`. Insert a §3.1 row after the
  `parallel` row (line 120). Change the §5 usage blocks for `add` (~line 259), `list`,
  `ready` (~311), `edit` (~331), `next` (~401) and `prime` (~406). Append §5.1 addenda
  before the block's closing fence (~line 575). Add `[needs]` to §6 (~line 627).
- Modify: `skills/tasks/SKILL.md`. Edit step 2 (~lines 28–43), the "Never edit"
  paragraph (~161–170) and the scoped-task recipe (~317).
- Modify: `skills/scope/SKILL.md`, after the process paragraph (~line 81).
- Modify: `README.md`, in the Use block (~line 274) and after the
  `TASKS_MAX_COMPLEXITY` paragraph (~line 153).

**Interfaces:**
- Consumes: the behaviour shipped in Tasks 1.1–1.6.
- Produces: documentation only. Slices 2 and 3 append their own lines beside these.

- [ ] **Step 1: The main design**

§3.1, a new row directly after the `parallel` row:

```markdown
| `needs`    | list of names       | no       | Shared resources the work uses, each declared in the project's `[needs]` vocabulary (§6); names use lowercase letters, digits, and `-`. Set by `add --need`; `edit --need` appends, `--rm-need` and `--no-needs` remove. Omitted when empty; written after `parallel`. An undeclared name is refused on write and is a `check` error, but is carried as-is on read. See `2026-10-03-lanes-needs-groups-design.md` §4. |
```

§5 `add`: after the `          [--process direct|planned]` line insert
`          [--need N]...`. Append this to the paragraph:
"--need names a shared resource from the project's `[needs]` vocabulary (§6),
repeatable. A name that breaks the grammar is validation, and an undeclared one is
unknown_need."

§5 `list`: the first line becomes
`tasks list [--status S]... [--tag T]... [--need N]... [--owner O] [--source REF]`.
Append: "--need is all-of, like --tag: a task must need every name given."

§5 `ready`: the line becomes
`tasks ready [--size S] [--parallel] [--need N]... [--without N]... [-n N] [--project P | --all-projects]`.
Append this paragraph:

```text
    --without N (repeatable) hides tasks needing N, as does each comma-separated name
    in TASKS_WITHOUT; the two are a union. The flag is strict: a name no project in
    scope declares is unknown_need. The variable is lenient: any name is accepted,
    since a host sets it once for every project. Either way a name hides a task only
    when the task's own project declares it, so a name hides nothing in a project
    that does not declare it, even a record still naming it. One warning,
    "without <names>: <n> task(s) hidden", counts what was hidden.
```

§5 `next` and `prime`: add `[--without N]...` to each usage line, with the sentence
"--without and TASKS_WITHOUT as for ready; next applies them to parked candidates too,
and prime to its ready list."

§5 `edit`: after the `           [--no-depends]` line insert
`           [--need N]... [--rm-need N]... [--no-needs]`. Append this paragraph:

```text
    --need appends like --tag; --rm-need removes one and is a validation error when
    the task lacks it; --no-needs clears, and with --need replaces. Only names being
    added must be declared, so a need since dropped from the vocabulary can still be
    removed. --status conflicts with all three, and an editor save that changes both
    the status and needs is refused: status and needs change in separate operations
    (2026-10-03-lanes-needs-groups-design.md §4.4).
```

§5.1, appended before the closing fence of the shapes block:

```text
Task        += needs: [string]                omitted when empty (lanes-needs §4.2)
TaskSummary += needs: [string]                omitted when empty
ParkedRow   += needs: [string]                omitted when empty or unresolved
ready/next/prime += one warning "without <names>: <n> task(s) hidden" when
                    --without or TASKS_WITHOUT hid ready work
check       += kind unknown_need (error): a record names a need [needs] does not declare
error kinds += unknown_need: add/edit --need or an editor save naming an undeclared
               need; ready/next/prime --without naming one no project in scope declares
```

§6, after the `prefix = "sci"` TOML block:

````markdown
A project may declare the shared resources its tasks use:

```toml
[needs.quiet]
meaning = "an idle host: a TTY with the desktop stopped"
exclusive = true
```

Each `[needs.<name>]` table takes a one-line `meaning` (required) and `exclusive`
(optional, default false); any other key, a missing meaning, or a name outside the tag
grammar is a `config` error. The names of exclusive needs form one namespace across the
host's projects: two projects using the same name mean the same machine resource. See
`2026-10-03-lanes-needs-groups-design.md` §4.1.
````

- [ ] **Step 2: `skills/tasks/SKILL.md`**

In step 2, directly after "The variable is the harness form; the flag is for a person at
a terminal.", add:

```markdown
   A session that cannot meet a shared resource says so: `TASKS_WITHOUT=quiet` (comma-
   separated) in a host's ordinary session environment, or `--without <need>` on
   `ready`/`next`/`prime`, hides tasks that need it, with one warning counting them.
   The flag refuses a name no project in scope declares; the variable never errors.
   A TTY session handed the idle host runs without the variable.
```

In the same step, after "and `--parallel` is a switch.", add:
"`--need` is all-of, like `--tag`."

In the "Never edit" paragraph, the flag list gains `/--need/--rm-need/--no-needs` after
`--no-defer`. After the paragraph that ends "`check` warns on open tasks carrying an
undefined one.", add:

```markdown
A step that uses a shared resource records it with `--need <name>` (repeatable), drawn
from the project's `[needs]` vocabulary in `tasks/.config.toml`:
`[needs.quiet]` with `meaning = "<one line>"` and, for a resource only one session can use
at a time, `exclusive = true`. Name an exclusive need after the host resource it stands
for: exclusive names are one namespace across every project on the host. Prefer a need to
an ad hoc `needs-*` tag. `add`/`edit` refuse an undeclared name (`unknown_need`), and
`check` errors on a record naming one; add the vocabulary entry first. `edit --need`
appends, `--rm-need` removes, and `--no-needs` clears. Change status and needs in
separate operations: `edit --status` refuses the needs flags, and an editor save may not
change both.
```

In the scoped-task recipe, after `--tag <group>` insert `[--need <name>]...`.

- [ ] **Step 3: `skills/scope/SKILL.md`**

After the paragraph ending "assigning it never makes an idea executable.", add:

```markdown
When a member's work uses a shared resource the project declares in `[needs]` (an idle
host, the owner's judgement), record it with `--need <name>` on the task you scope or
create. Never invent a name: an undeclared one is refused. Propose the vocabulary entry in
the handoff brief instead.
```

- [ ] **Step 4: `README.md`**

In the Use block, directly after the `tasks ready --parallel -n 3 ...` line:

```text
    tasks add "Capture the trace" --need quiet  # a shared resource from [needs] in tasks/.config.toml
    tasks edit <id> --rm-need quiet  # --need adds; --rm-need/--no-needs remove
    tasks list --need quiet          # tasks that need it (all-of, like --tag)
    tasks ready --without quiet      # hide steps this session cannot meet; also TASKS_WITHOUT
```

After the `TASKS_MAX_COMPLEXITY` paragraph (ending
"`docs/specs/2026-09-12-task-complexity-design.md`."):

```markdown
`TASKS_WITHOUT` (comma-separated need names) is the other half of a session's envelope:
`ready`, `next`, and `prime` hide tasks needing any of them, in the projects that declare
the name, and say how many.
`--without` adds to it for one call and refuses a name no project in scope declares; the
variable never errors, so a host in ordinary use can set `TASKS_WITHOUT=quiet` once for
every project. Needs are declared per project in `[needs]` (see the design's §6). Design:
`docs/specs/2026-10-03-lanes-needs-groups-design.md`.
```

- [ ] **Step 5: Check and commit**

Run `just check`. Expected: PASS. Design documents take the full check.

```bash
git add docs/specs/2026-08-29-tasks-design.md skills/tasks/SKILL.md skills/scope/SKILL.md README.md
git commit -m "docs(needs): document the needs field, vocabulary, and --without"
```

---

## Slice 2 — Exclusive holds

**Goal.** A claim records the exclusive needs its task uses (`holds`). Every acquire path
refuses a task whose exclusive need is held by another session's live claim on another
task (`need_held`), and `start --force --reason` overrides with audit notes. A needs change
is refused under another session's live claim, and under the caller's own claim it
recomputes `holds`. `ready`, `next` and `prime` hide held-back work with aggregated
warnings. A host-wide `claims/.holds.lock` makes the check-and-save atomic across
projects, and it covers every change to `holds` through `save`'s rollback. Spec: `docs/specs/2026-10-03-lanes-needs-groups-design.md` §4.4, §4.5, and the
holds parts of §8, §9 and §10.

**Starts from:** Slice 1's end state. This slice consumes these Slice 1 symbols exactly as
the contract names them:
- `crate::needs::{Vocabulary, NeedDecl, exclusive_of}`;
- `Project.needs: Vocabulary` and `Task.needs: Vec<String>`;
- `add --need`, `edit --need` / `--rm-need` / `--no-needs`, where `--status` conflicts
  with all three;
- the editor refusal "a save that changes the status cannot also change needs; …".

The plan also assumes the following clap field names, mirroring the tag flags. Use Slice
1's actual names if they differ.
- `FieldArgs.needs: Vec<String>` (`--need`).
- `EditArgs.rm_needs: Vec<String>` (`--rm-need`).
- `EditArgs.no_needs: bool` (`--no-needs`).
- The frontmatter writes `needs: [a, b]` as a flow list, as it writes `tags`.

Line numbers below are from the pre-Slice-1 tree. Locate each edit by the quoted anchor
text.

**Test identities.** `env.cmd` (no `TASKS_SESSION`) resolves to the test runner's Unix
session for every call, so two `env.json` calls are the *same* session. Every test that
needs two sessions uses the `as_agent`-based helpers below (`TASKS_SESSION` plus the
runner's live pid).

**"This session" is the claim-ownership rule.** Whether a hold is the caller's own is
decided by the rule every claim check already uses (work-claims §6.2.2): the resolved
identity equals the claim's session, or, in relay mode without an explicit
`TASKS_SESSION`, the claim's recorded process proof names the caller's harness. Task 2.2
moves that rule out of `Ctx` into the free function `commands::ownership`, so the read
views can apply it too. A session that moved from native identity `c1` to relay identity
`claude-code:c1` therefore keeps its holds (Task 2.2 extends the existing acceptance test
for that move).

**CLI inventory.** The rows live in ops `cli.toml` and land there first. The only
CLI-touching task in this slice is Task 2.4 (`edit --reason`). It keeps its exact row
edit in a block titled **Ops rows (landed by Task 0.1)**. Task 0.1 applies that block in
ops as its own commit, in execution order after Slice 1's, and records it as the note
`inventory: Task 2.4 = <sha>` on tasks-ece1e2. Task 2.4 never edits ops. Its inventory
step adopts the rows with `python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where
`$OPS` is the `ops` root from `tasks projects`. Never write `tools/cli.toml` by hand or
with `cp`.

**Clippy and Slice 1's lint attribute.** Slice 1 gives `needs::exclusive_of` the
attribute `#[cfg_attr(not(test), expect(dead_code, …))]`. Task 2.1 is its first caller,
so Task 2.1 deletes the attribute in the same commit. Otherwise the expectation goes
unfulfilled and clippy fails.

### File Structure

- `src/holds.rs`: **new.** It contains:
  - `Holder`;
  - `Mine`, the caller's own holding claims;
  - `HoldSnapshot`, with `load`, `load_from_paths`, `mine`, `holder` and `is_empty`;
  - `held_back`;
  - `HeldWarnings`;
  - `lock_path_with` and `lock`.

  It is the read model of holds across every registered claim store, plus the host-wide
  lock.
- `src/main.rs`: `mod holds;`.
- `src/needs.rs`: Task 2.1 deletes the `expect(dead_code)` attribute on `exclusive_of`.
- `src/claims.rs`: `Claim.holds`, and the test fixtures that build `Claim` literals.
- `src/output.rs`: `ClaimInfo.holds` (sparse).
- `src/error.rs`: `Error::NeedHeld`, kind `need_held`.
- `src/commands/mod.rs`:
  - `ownership`, moved out of `impl Ctx` into a free function, and `own_holds`;
  - new `Ctx` fields `holds_lock`, `need_reason` and `need_overrides`;
  - `Ctx::guard_holds`, called from `transition` on every move to `doing`;
  - `Ctx::update_holds`, the needs save under one's own claim;
  - `Ctx::take_holds_lock`, and the holds-lock check in `save`;
  - `record_need_overrides` and `note_need_holders`, the audit notes. The notes on a
    holder are written only after the acquiring save has landed.
- `src/commands/status.rs`: `start` passes `--reason` to the hold guard and writes the
  override notes. The `note` command's ownership call site.
- `src/commands/park.rs`: the ownership call site.
- `src/commands/edit.rs`: the `--force`/`--reason` rules, and the needs-change claim
  check plus holds update on the flag and editor paths.
- `src/cli.rs`: `EditArgs.reason`.
- `tools/cli.toml` (and `tools/cli_surface.py` when the adopt refreshes it): adopted from
  ops at Task 2.4's inventory commit, never edited here.
- `src/commands/list.rs`: `retain_unheld` in `ready`, `next` and `prime`.
- `tests/cli.rs`: the holds helpers and end-to-end tests, appended at the end of the file.
- `docs/specs/2026-08-29-tasks-design.md`, `docs/specs/2026-09-05-work-claims-design.md`,
  `skills/tasks/SKILL.md`, `README.md`: addenda.

---

### Task 2.1: Record exclusive needs as `holds` on every acquire

**Files**
- Modify `src/claims.rs:136-149` (`pub struct Claim`). Also the test fixtures that build
  `Claim` literals: `sample` (950-962), `claim_of` (981-993), `sample_claim` (1765-1777),
  and the literal in `inserting_a_claim_displaces_a_park_on_the_same_id` (2133-2145).
  Append the tests to `mod tests`.
- Modify `src/output.rs:242-268` (`ClaimInfo`, `ClaimInfo::of`), and add a test to
  `mod tests` (from 1924).
- Modify `src/commands/mod.rs`:
  - the `Claim` literal in `claim_guard` (322-356, anchor `seen: now,`);
  - a new `Ctx::guard_holds` in `impl Ctx` (before `claim_guard`, line 267);
  - `transition` (930-971, anchor `ctx.claim_guard(&task.id, to, force)?;`).
- Modify `src/needs.rs`: delete Slice 1's `expect(dead_code)` attribute on `exclusive_of`
  (anchor `reason = "exclusive holds (slice 2) are its first caller"`).
- Modify `tests/cli.rs`: append the helpers and the test.

**Interfaces**
- Consumes:
  - `crate::needs::exclusive_of(vocab: &Vocabulary, needs: &[String]) -> Vec<String>`;
  - `Project.needs: Vocabulary`;
  - `Task.needs: Vec<String>`.
- Produces:
  - `claims::Claim { …, pub holds: Vec<String> }`, with
    `#[serde(default, skip_serializing_if = "Vec::is_empty")]`;
  - `output::ClaimInfo { …, pub holds: Vec<String> }`, with
    `#[serde(skip_serializing_if = "Vec::is_empty")]`;
  - `impl Ctx { fn guard_holds(&mut self, task: &Task) -> Result<()> }`.

**Steps**

- [ ] **Write the failing unit tests.** Append to `mod tests` in `src/claims.rs`:

```rust
    #[test]
    fn a_claim_written_before_holds_loads_holding_nothing_and_saves_without_the_key() {
        let (dir, store) = store_from(A_CLAIM);
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(store.get(&id).unwrap().holds.is_empty());
        store.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join("sci.toml")).unwrap();
        assert!(!text.contains("holds"), "{text}");
    }

    #[test]
    fn holds_round_trip_on_a_claim_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();
        let mut store = ClaimStore::load_from(&path).unwrap();
        store.insert(
            &id,
            Claim {
                holds: vec!["quiet".into()],
                ..sample_claim()
            },
        );
        store.save().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("holds = [\"quiet\"]"), "{text}");
        assert_eq!(
            ClaimStore::load_from(&path).unwrap().get(&id).unwrap().holds,
            ["quiet"]
        );
    }
```

  Append to `mod tests` in `src/output.rs`:

```rust
    #[test]
    fn claim_info_carries_holds_only_when_the_claim_has_some() {
        let claim = crate::claims::Claim {
            owner: "o".into(),
            session: "s".into(),
            pid: None,
            pid_start: None,
            boot_id: None,
            host: "h".into(),
            worktree: "/w".into(),
            started: "2026-10-03T00:00:00Z".into(),
            seen: "2026-10-03T00:00:00Z".into(),
            holds: Vec::new(),
        };
        let live = crate::claims::Liveness::Live;
        let bare = serde_json::to_value(ClaimInfo::of(&claim, &live)).unwrap();
        assert!(bare.get("holds").is_none(), "{bare}");
        let holding = crate::claims::Claim {
            holds: vec!["quiet".into()],
            ..claim
        };
        let shown = serde_json::to_value(ClaimInfo::of(&holding, &live)).unwrap();
        assert_eq!(shown["holds"], serde_json::json!(["quiet"]));
    }
```

- [ ] **Write the failing end-to-end test.** It also adds the shared holds helpers, used by
  every later task. Append them to the end of `tests/cli.rs`:

```rust
// ---- Exclusive holds (lanes/needs design §4.4–§4.5) ----

/// The vocabulary every holds test uses: `quiet` is exclusive, `owner` is not.
fn hold_vocab(dir: &std::path::Path) {
    let path = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "\n[needs.quiet]\nmeaning = \"an idle host\"\nexclusive = true\n\
         \n[needs.owner]\nmeaning = \"the owner judges an image\"\n",
    );
    std::fs::write(&path, text).unwrap();
}

/// `tasks <args>` run as `session` (with the runner's live pid), parsed; it must succeed.
fn json_as(
    env: &TestEnv,
    dir: &std::path::Path,
    session: &str,
    args: &[&str],
) -> serde_json::Value {
    let out = as_agent(env, dir, session).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "tasks {args:?} as {session} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

/// The `{"error": …}` of `tasks <args>` run as `session`; it must exit 1.
fn error_as(
    env: &TestEnv,
    dir: &std::path::Path,
    session: &str,
    args: &[&str],
) -> serde_json::Value {
    let out = as_agent(env, dir, session).args(args).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "tasks {args:?} as {session}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stderr).unwrap()
}

/// The `holds` of the claim `show` reports on `id`: null when there is no claim or it
/// holds nothing (the key is sparse).
fn holds_of(env: &TestEnv, dir: &std::path::Path, id: &str) -> serde_json::Value {
    env.json(dir, &["show", id])["claim"]["holds"].clone()
}

/// A claim written straight into `prefix`'s store, holding `holds`.
fn write_hold(env: &TestEnv, prefix: &str, id: &str, session: &str, live: bool, holds: &[&str]) {
    write_claim(env, prefix, id, session, live);
    let path = env.claim_store(prefix);
    let mut text = std::fs::read_to_string(&path).unwrap();
    let list = holds
        .iter()
        .map(|need| format!("{need:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    // `write_claim` ends inside the claim's table, so this key lands in that entry.
    text.push_str(&format!("holds = [{list}]\n"));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn every_acquire_path_records_the_tasks_exclusive_needs_as_holds() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let started = id_of(env.json(
        &sci,
        &["add", "Capture", "-p", "2", "--need", "quiet", "--need", "owner"],
    ));
    let flagged = id_of(env.json(&sci, &["add", "Flagged", "-p", "2", "--need", "quiet"]));
    let edited = id_of(env.json(&sci, &["add", "Edited", "-p", "2", "--need", "quiet"]));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2", "--need", "owner"]));

    json_as(&env, &sci, "agent-a", &["start", &started]);
    json_as(&env, &sci, "agent-a", &["edit", &flagged, "--status", "doing"]);
    let editor = editor_script(&sci, "sed -i 's/status: todo/status: doing/' \"$1\"");
    let out = as_agent(&env, &sci, "agent-a")
        .env("EDITOR", editor)
        .args(["edit", &edited])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    json_as(&env, &sci, "agent-a", &["start", &plain]);

    for id in [&started, &flagged, &edited] {
        assert_eq!(holds_of(&env, &sci, id), serde_json::json!(["quiet"]), "{id}");
    }
    // A non-exclusive need is no hold, and an empty `holds` is absent everywhere.
    assert!(holds_of(&env, &sci, &plain).is_null());
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(store.matches("holds = [\"quiet\"]").count(), 3, "{store}");
}
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --bin tasks holds_round_trip_on_a_claim_entry` fails to compile: `struct
    Claim has no field named holds`.
  - After the struct change, `just test-one --test cli
    every_acquire_path_records_the_tasks_exclusive_needs_as_holds` fails with `left: Null,
    right: Array ["quiet"]`.

- [ ] **Implement.**

  In `src/claims.rs`, append to `pub struct Claim` after `pub seen: String,`:

```rust
    /// The exclusive needs this claim holds: the task's needs that its own project declares
    /// exclusive, computed on every acquire (lanes/needs design §4.4). Absent in entries
    /// written before holds existed, which therefore hold nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
```

  Add `holds: Vec::new(),` after the `seen:` line in these four `Claim { … }` literals in
  `mod tests`: `sample`, `claim_of`, `sample_claim`, and the literal inside
  `inserting_a_claim_displaces_a_park_on_the_same_id`. The literals that use `..sample()`
  or `..seen(…)` need nothing.

  In `src/output.rs`, add to `pub struct ClaimInfo` after `pub live: bool,`:

```rust
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
```

  In `ClaimInfo::of`, add after `live: …,`:

```rust
            holds: claim.holds.clone(),
```

  In `src/commands/mod.rs`, add to the `ClaimIntent::Acquire(crate::claims::Claim { … })`
  literal in `claim_guard`, after `seen: now,`:

```rust
                    // Filled by `guard_holds`, which knows the task's needs.
                    holds: Vec::new(),
```

  Add to `impl Ctx`, directly above `fn claim_guard`:

```rust
    /// Lanes/needs design §4.4: every acquire records the task's needs that this project
    /// declares exclusive (undeclared names ignored) as the claim's `holds`. Status and
    /// needs never change in one operation, so this always reads the needs on the record.
    fn guard_holds(&mut self, task: &Task) -> Result<()> {
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        match self.pending_claim.as_mut() {
            Some((_, ClaimIntent::Acquire(claim))) => claim.holds = holds,
            _ => unreachable!("claim_guard records an acquire for every move to doing"),
        }
        Ok(())
    }
```

  In `transition`, replace

```rust
    ctx.claim_guard(&task.id, to, force)?;
```

  with

```rust
    ctx.claim_guard(&task.id, to, force)?;
    if to == Status::Doing {
        ctx.guard_holds(task)?;
    }
```

- [ ] **Retire Slice 1's lint attribute on `exclusive_of`.** `guard_holds` is its first
  non-test caller, so the `expect(dead_code)` that Slice 1 put on it is now unfulfilled,
  and clippy's `-D warnings` fails on an unfulfilled expectation. In `src/needs.rs`,
  delete these four lines directly above `pub fn exclusive_of(`:

```rust
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exclusive holds (slice 2) are its first caller")
)]
```

  The doc comment above them stays. Then run
  `grep -n 'expect(dead_code' src/needs.rs`. Expected: no output.

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --bin tasks holds_round_trip_on_a_claim_entry`
  - `just test-one --bin tasks a_claim_written_before_holds`
  - `just test-one --bin tasks claim_info_carries_holds`
  - `just test-one --test cli every_acquire_path_records_the_tasks_exclusive_needs_as_holds`

- [ ] Run `just test-fast` (every existing test still passes), then `just check`. It runs
  clippy with `-D warnings`, which would fail on a leftover `expect(dead_code)`.

- [ ] **Commit.**
  `git add src/needs.rs src/claims.rs src/output.rs src/commands/mod.rs tests/cli.rs`
  `git commit -m "feat(claims): record a task's exclusive needs as holds on every acquire"`

---

### Task 2.2: The hold read model and the `need_held` refusal on every acquire path

**Files**
- Create `src/holds.rs`.
- Modify `src/main.rs`: add `mod holds;` after `mod hierarchy;`.
- Modify `src/error.rs`:
  - the variant list (lines 3-55, after `Halted`);
  - `with_suffix` (line 89 area, after the `Halted` arm);
  - `kind` (line 121 area, after `"halted"`).
- Modify `src/commands/mod.rs`:
  - `impl Ctx`: remove the method `ownership` (173-206, with its doc comment from line
    170), and re-add it as a free function after the `impl Ctx` block (before
    `pub fn open_ctx`, line 374), next to the new `own_holds`;
  - its call sites: `refuse_foreign_live_claim` (line 214), `claim_guard` (line 274),
    `occupants` (line 773) and `follow_holder` (line 825), plus the two doc comments that
    name `Ctx::ownership` (lines 762 and 791);
  - `Ctx::guard_holds` (from Task 2.1).
- Modify `src/commands/status.rs:179` and `src/commands/park.rs:42`: the `ctx.ownership`
  call sites.
- Modify `tests/cli.rs`:
  - append the tests;
  - extend `an_acceptance_mode_change_continues_a_natively_held_claim` (line 19975);
  - the two comments that name `Ctx::ownership` (lines 19379 and 19952).

**Interfaces**
- Consumes:
  - `claims::{Claim, ClaimStore::{path_for, load_from, iter}, liveness_with, boot_id, proc_stat, ProcStat, Liveness, Resolution, continuation_identity}`;
  - `Registry.projects`;
  - `needs::{Vocabulary, exclusive_of}`;
  - the body of today's `Ctx::ownership(&mut self, claim: &Claim, me: &Resolution) -> Result<Ownership>`.
    It never reads `self`.
- Produces:
  - `pub(crate) fn ownership(claim: &claims::Claim, me: &claims::Resolution) -> Result<Ownership>`
    in `commands`. This is the same rule with `&mut self` dropped. `Ctx::ownership` no
    longer exists.
  - `pub(crate) fn own_holds(snapshot: &holds::HoldSnapshot, me: &claims::Resolution) -> Result<holds::Mine>`
    in `commands`.
  - `pub struct Holder { pub task: TaskId, pub session: String, pub prefix: String }`,
    deriving Debug, Clone, PartialEq and Eq.
  - `#[derive(Debug, Default)] pub struct Mine`: the caller's own live holding claims.
    `Mine::default()` owns nothing, which is the unresolved-identity case: every hold
    counts as another session's.
  - `pub struct HoldSnapshot`, with:
    - `pub fn load(registry: &Registry, now: OffsetDateTime) -> (HoldSnapshot, Vec<String>)`;
    - `pub fn load_from_paths(stores: impl IntoIterator<Item = (String, PathBuf)>, now: OffsetDateTime, boot_id: Option<&str>, stat: impl Fn(u32) -> ProcStat) -> (HoldSnapshot, Vec<String>)`;
    - `pub fn mine(&self, owns: impl FnMut(&Claim) -> Result<bool>) -> Result<Mine>`;
    - `pub fn holder(&self, need: &str, task: &TaskId, mine: &Mine) -> Option<&Holder>`.
      **This changes the contract's** `session: Option<&str>` to `mine: &Mine`. `None`
      becomes `&Mine::default()`, and a session string becomes `own_holds(&snapshot, &me)?`.
  - `pub fn held_back(snapshot: &HoldSnapshot, vocab: &Vocabulary, task: &Task, mine: &Mine) -> Option<(String, Holder)>`.
    The last parameter changes the same way.
  - `Error::NeedHeld(String)`, with kind `"need_held"`.
  - `Ctx::guard_holds`, which now refuses.

**Steps**

- [ ] **Write the failing unit tests.** Create `src/holds.rs` with only its test module for
  now. The module body is added in the implementation step below.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::ProcStat;

    fn at(s: &str) -> OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn alive(_: u32) -> ProcStat {
        ProcStat::Found {
            state: 'S',
            starttime: 7,
        }
    }

    fn gone(_: u32) -> ProcStat {
        ProcStat::NotFound
    }

    /// An unresolved identity: no hold is the caller's.
    fn nobody() -> Mine {
        Mine::default()
    }

    /// The holds a caller owns by session equality alone, as an explicit
    /// `TASKS_SESSION` caller does under `commands::ownership`.
    fn session(snapshot: &HoldSnapshot, name: &str) -> Mine {
        snapshot.mine(|claim| Ok(claim.session == name)).unwrap()
    }

    /// One claim entry seen at 10:00. With a pid, its liveness is the process's; without
    /// one, the four-hour TTL's.
    fn entry(id: &str, session: &str, pid: Option<u32>, holds: &[&str]) -> String {
        let pid = pid
            .map(|pid| format!("pid = {pid}\npid_start = 7\nboot_id = \"boot\"\n"))
            .unwrap_or_default();
        let holds = if holds.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = holds.iter().map(|need| format!("{need:?}")).collect();
            format!("holds = [{}]\n", list.join(", "))
        };
        format!(
            "[claims.\"{id}\"]\nowner = \"o\"\nsession = \"{session}\"\n{pid}host = \"h\"\n\
             worktree = \"/w\"\nstarted = \"2026-10-03T10:00:00Z\"\n\
             seen = \"2026-10-03T10:00:00Z\"\n{holds}"
        )
    }

    fn store(dir: &std::path::Path, prefix: &str, text: &str) -> (String, PathBuf) {
        let path = dir.join(format!("{prefix}.toml"));
        std::fs::write(&path, text).unwrap();
        (prefix.to_string(), path)
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    #[test]
    fn a_live_claim_holds_and_a_dead_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &["quiet"]),
        )];
        let now = at("2026-10-03T11:00:00Z");
        let (live, warnings) =
            HoldSnapshot::load_from_paths(stores.clone(), now, Some("boot"), alive);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            live.holder("quiet", &id("sci-000002"), &nobody()),
            Some(&Holder {
                task: id("sci-000001"),
                session: "a".into(),
                prefix: "sci".into(),
            })
        );
        let (dead, _) = HoldSnapshot::load_from_paths(stores, now, Some("boot"), gone);
        assert!(dead.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn a_claim_without_a_pid_holds_until_its_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "codex:x", None, &["quiet"]),
        )];
        let (within, _) = HoldSnapshot::load_from_paths(
            stores.clone(),
            at("2026-10-03T13:59:00Z"),
            Some("boot"),
            alive,
        );
        assert!(within.holder("quiet", &id("sci-000002"), &nobody()).is_some());
        let (past, _) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T15:00:00Z"),
            Some("boot"),
            alive,
        );
        assert!(past.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn an_entry_written_before_holds_existed_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(dir.path(), "sci", &entry("sci-000001", "a", Some(42), &[]))];
        let (snapshot, warnings) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            alive,
        );
        assert!(warnings.is_empty());
        assert!(snapshot.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn an_unreadable_store_warns_and_the_others_still_count() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![
            store(dir.path(), "fam", "claims = [not toml"),
            store(dir.path(), "sci", &entry("sci-000001", "a", Some(42), &["quiet"])),
            // A store that was never written is empty, not unknown.
            ("ops".to_string(), dir.path().join("ops.toml")),
        ];
        let (snapshot, warnings) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            alive,
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("hold state unknown for fam ("), "{warnings:?}");
        assert!(snapshot.holder("quiet", &id("sci-000002"), &nobody()).is_some());
    }

    #[test]
    fn a_claim_key_that_is_not_a_task_id_warns_and_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(dir.path(), "sci", &entry("junk", "a", Some(42), &["quiet"]))];
        let (snapshot, warnings) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            alive,
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("hold state unknown for sci:"), "{warnings:?}");
        assert!(snapshot.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn the_target_task_and_the_callers_session_never_hold_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &["quiet"]),
        )];
        let (snapshot, _) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            alive,
        );
        // A claim on the target itself: a takeover, never a hold.
        assert!(snapshot.holder("quiet", &id("sci-000001"), &nobody()).is_none());
        // The holding session may take more work that needs what it holds.
        assert!(snapshot.holder("quiet", &id("sci-000002"), &session(&snapshot, "a")).is_none());
        assert!(snapshot.holder("quiet", &id("sci-000002"), &session(&snapshot, "b")).is_some());
        // Unresolved identity: every hold is someone else's.
        assert!(snapshot.holder("quiet", &id("sci-000002"), &nobody()).is_some());
        // A need nobody holds.
        assert!(snapshot.holder("gpu", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn whose_hold_it_is_follows_the_ownership_rule_not_the_session_string() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "c1", Some(42), &["quiet"]),
        )];
        let (snapshot, _) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            alive,
        );
        // A rule that owns the claim by its process (as relay proof does) makes the hold
        // the caller's own, though the caller's session string is not `c1`.
        let proved = snapshot.mine(|claim| Ok(claim.pid == Some(42))).unwrap();
        assert!(snapshot.holder("quiet", &id("sci-000002"), &proved).is_none());
        // The rule's error is the caller's error, never a silent "not mine".
        let error = snapshot
            .mine(|_| Err(crate::error::Error::Io("relay config unreadable".into())))
            .unwrap_err();
        assert!(error.to_string().contains("relay config unreadable"), "{error}");
    }
}
```

- [ ] **Write the failing end-to-end tests.** Append to `tests/cli.rs`:

```rust
#[test]
fn every_acquire_path_refuses_a_need_another_session_holds() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let holding = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let started = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    let flagged = id_of(env.json(&sci, &["add", "Sweep", "-p", "2", "--need", "quiet"]));
    let edited = id_of(env.json(&sci, &["add", "Trace", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &holding]);
    let records: Vec<(String, String)> = [&started, &flagged, &edited]
        .iter()
        .map(|id| (id.to_string(), env.read(&sci, &format!("tasks/{id}.md"))))
        .collect();

    let error = error_as(&env, &sci, "agent-b", &["start", &started]);
    assert_eq!(error["error"]["kind"], "need_held", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(&format!("held by {holding} (agent-a)")), "{detail}");
    assert!(
        detail.contains(&format!("tasks start {started} --force --reason")),
        "{detail}"
    );

    let error = error_as(&env, &sci, "agent-b", &["edit", &flagged, "--status", "doing"]);
    assert_eq!(error["error"]["kind"], "need_held", "{error}");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("tasks start {flagged} --force --reason")),
        "{error}"
    );

    let editor = editor_script(&sci, "sed -i 's/status: todo/status: doing/' \"$1\"");
    let out = as_agent(&env, &sci, "agent-b")
        .env("EDITOR", editor)
        .args(["edit", &edited])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(err_kind(&out), "need_held");
    assert!(err_detail(&out).contains(&format!("tasks start {edited} --force --reason")));

    for (id, before) in &records {
        assert_eq!(&env.read(&sci, &format!("tasks/{id}.md")), before, "{id}");
        assert!(env.json(&sci, &["show", id])["claim"].is_null(), "{id}");
    }
}

#[test]
fn a_holding_session_takes_more_and_a_takeover_of_the_held_task_is_not_held_back() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let first = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &first]);

    // A claim on the target itself never holds it back: --force alone takes it over.
    json_as(&env, &sci, "agent-b", &["start", &first, "--force"]);
    let shown = env.json(&sci, &["show", &first]);
    assert_eq!(shown["claim"]["session"], "agent-b");
    assert_eq!(shown["claim"]["holds"], serde_json::json!(["quiet"]));

    // The session that now holds quiet may start more work that needs it.
    json_as(&env, &sci, "agent-b", &["start", &second]);
    assert_eq!(holds_of(&env, &sci, &second), serde_json::json!(["quiet"]));
}

#[test]
fn a_park_releases_the_hold() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let first = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &first]);
    assert_eq!(
        error_as(&env, &sci, "agent-b", &["start", &second])["error"]["kind"],
        "need_held"
    );
    json_as(&env, &sci, "agent-a", &["park", &first, "wait for the idle host"]);
    json_as(&env, &sci, "agent-b", &["start", &second]);
    assert_eq!(holds_of(&env, &sci, &second), serde_json::json!(["quiet"]));
}

#[test]
fn a_claim_without_a_pid_holds_until_its_ttl() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let first = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    // TASKS_SESSION without TASKS_SESSION_PID: the claim lives by its TTL alone.
    env.cmd(&sci)
        .env("TASKS_SESSION", "ttl-agent")
        .args(["start", &first])
        .assert()
        .success();
    assert!(env.json(&sci, &["show", &first])["claim"]["pid"].is_null());
    assert_eq!(
        error_as(&env, &sci, "agent-b", &["start", &second])["error"]["kind"],
        "need_held"
    );

    // Past the TTL the claim is stale, and a stale claim holds nothing.
    let store = env.claim_store("sci");
    let aged: String = std::fs::read_to_string(&store)
        .unwrap()
        .lines()
        .map(|line| {
            if line.starts_with("seen = ") {
                "seen = \"2026-01-01T00:00:00Z\"\n".to_string()
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    std::fs::write(&store, aged).unwrap();
    json_as(&env, &sci, "agent-b", &["start", &second]);
    assert_eq!(holds_of(&env, &sci, &second), serde_json::json!(["quiet"]));
}

/// These pass before the gate exists; they keep it from over-reaching.
#[test]
fn claims_that_cannot_hold_do_not_hold_back_an_acquire() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let dead = id_of(env.json(&sci, &["add", "Dead", "-p", "2", "--need", "quiet"]));
    let older = id_of(env.json(&sci, &["add", "Older", "-p", "2", "--need", "quiet"]));
    let targets: Vec<String> = (0..3)
        .map(|n| id_of(env.json(&sci, &["add", &format!("T{n}"), "-p", "2", "--need", "quiet"])))
        .collect();

    // A dead claim holds nothing.
    write_hold(&env, "sci", &dead, "ghost", false, &["quiet"]);
    json_as(&env, &sci, "agent-b", &["start", &targets[0]]);
    // A live entry written before holds existed holds nothing.
    write_claim(&env, "sci", &older, "agent-old", true);
    json_as(&env, &sci, "agent-b", &["start", &targets[1]]);
    // A store left behind by an unregistered prefix is never read.
    write_hold(&env, "old", "old-a00001", "ghost", true, &["quiet"]);
    json_as(&env, &sci, "agent-b", &["start", &targets[2]]);
    for id in &targets {
        assert_eq!(holds_of(&env, &sci, id), serde_json::json!(["quiet"]), "{id}");
    }
}
```

- [ ] **Extend the native-to-relay acceptance test.** A session that moves from native
  identity `c1` to relay identity `claude-code:c1` keeps its claim by process proof. It
  must keep its holds the same way. In
  `an_acceptance_mode_change_continues_a_natively_held_claim` in `tests/cli.rs`, make
  three edits.

  First, replace

```rust
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = env.home.path().join("relay-state");
    let after_start = env.home.path().join("after-start.toml");
```

  with

```rust
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // Lanes/needs §4.4: both tasks need the exclusive `quiet`. The first claim's hold is
    // this session's only by proof once relay re-keys its identity, so the second start
    // passes the hold gate only if the gate uses the ownership rule, not session strings.
    hold_vocab(&dir);
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&dir, &["add", "Second", "-p", "2", "--need", "quiet"]));
    let state = env.home.path().join("relay-state");
    let after_start = env.home.path().join("after-start.toml");
```

  Second, in the script, replace

```rust
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" done {id} landed\n",
```

  with

```rust
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" start {second}\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" done {id} landed\n",
```

  The script runs under `set -e`, so a `need_held` refusal of the second start fails the
  shim's exit-code assertion. The `after-start.toml` copy is taken before the second
  start, so its one-claim assertion is unchanged.

  Third, append at the end of the test, after
  `assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "done");`:

```rust
    // The second start passed the hold gate: the first claim's hold was this session's by
    // proof, although its session string `c1` differs from the resolved `claude-code:c1`.
    let shown = env.json(&dir, &["show", &second]);
    assert_eq!(shown["task"]["status"], "doing", "{shown}");
    assert_eq!(shown["claim"]["holds"], serde_json::json!(["quiet"]), "{shown}");
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --bin tasks holds::tests` fails to compile: `HoldSnapshot`, `Holder`,
    `Mine` and `OffsetDateTime` are unresolved.
  - `just test-one --test cli an_acceptance_mode_change_continues_a_natively_held_claim`
    passes before the gate exists. It is the regression check for the gate below: a gate
    that compared session strings would refuse the second start with `need_held`.
  - `just test-one --test cli every_acquire_path_refuses_a_need_another_session_holds` fails
    because `start` succeeds (`tasks ["start", …] as agent-b: … should exit 1`).
  - `a_park_releases_the_hold` and `a_claim_without_a_pid_holds_until_its_ttl` fail at their
    first `need_held` assertion.

- [ ] **Implement.** Put this above the test module in `src/holds.rs`:

```rust
//! Exclusive holds (lanes/needs design §4.4): which live claims, in the claim store of
//! every registered project, hold which exclusive needs. Read without any project's lock;
//! an acquire that would record a hold reads it under the host-wide holds lock.

use crate::claims::{Claim, ClaimStore, Liveness, ProcStat};
use crate::error::Result;
use crate::model::{Task, TaskId};
use crate::needs::Vocabulary;
use crate::registry::Registry;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use time::OffsetDateTime;

/// The live claim that holds a need: its task, the session that owns it, and the prefix
/// of the store it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub task: TaskId,
    pub session: String,
    pub prefix: String,
}

/// The live holding claims that are the caller's own (§4.4 "Blocks other sessions
/// only"), by store prefix and task. Built by `HoldSnapshot::mine` with the claim
/// ownership rule (`commands::ownership`), never by comparing session strings. The
/// default owns nothing: with no resolved identity and no proof, every hold is another
/// session's.
#[derive(Debug, Default)]
pub struct Mine {
    claims: BTreeSet<(String, TaskId)>,
}

impl Mine {
    fn owns(&self, holder: &Holder) -> bool {
        self.claims
            .contains(&(holder.prefix.clone(), holder.task.clone()))
    }
}

/// Every live hold on the host, by need name. Exclusive need names are one namespace
/// across projects (§4.1), so a hold recorded in any project counts in every project that
/// declares the same name exclusive.
#[derive(Debug, Default)]
pub struct HoldSnapshot {
    by_need: BTreeMap<String, Vec<Holder>>,
    /// The full claim behind every holder: ownership by process proof reads its pid,
    /// start time, boot and host, not only its session.
    claims: BTreeMap<(String, TaskId), Claim>,
}

fn unknown(prefix: &str, error: &crate::error::Error) -> String {
    format!("hold state unknown for {prefix} ({error})")
}

impl HoldSnapshot {
    /// Reads the store of every registered prefix (§4.4 "Which stores are read"), not
    /// only those in scope: the same set `tasks claims` reads. Stores of unregistered or
    /// renamed prefixes are never read. Unlike `ClaimSnapshot`, a store that cannot be
    /// read is a warning, not a failure, and contributes no holds.
    pub fn load(registry: &Registry, now: OffsetDateTime) -> (HoldSnapshot, Vec<String>) {
        let mut warnings = Vec::new();
        let mut stores = Vec::new();
        for prefix in registry.projects.keys() {
            match ClaimStore::path_for(prefix) {
                Ok(path) => stores.push((prefix.clone(), path)),
                Err(error) => warnings.push(unknown(prefix, &error)),
            }
        }
        let (snapshot, more) = Self::load_from_paths(
            stores,
            now,
            crate::claims::boot_id().as_deref(),
            crate::claims::proc_stat,
        );
        warnings.extend(more);
        (snapshot, warnings)
    }

    /// The path form, with liveness inputs injected, so a test needs no environment.
    pub fn load_from_paths(
        stores: impl IntoIterator<Item = (String, PathBuf)>,
        now: OffsetDateTime,
        boot_id: Option<&str>,
        stat: impl Fn(u32) -> ProcStat,
    ) -> (HoldSnapshot, Vec<String>) {
        let mut snapshot = HoldSnapshot::default();
        let mut warnings = Vec::new();
        for (prefix, path) in stores {
            let store = match ClaimStore::load_from(&path) {
                Ok(store) => store,
                Err(error) => {
                    warnings.push(unknown(&prefix, &error));
                    continue;
                }
            };
            for (key, claim) in store.iter() {
                // Held only while live (§4.4): a dead claim, a park, or a doing status
                // alone holds nothing; a pidless claim holds for its TTL.
                if claim.holds.is_empty()
                    || crate::claims::liveness_with(claim, now, boot_id, &stat) != Liveness::Live
                {
                    continue;
                }
                let task = match TaskId::parse(key) {
                    Ok(task) => task,
                    Err(error) => {
                        warnings.push(format!(
                            "hold state unknown for {prefix}: claim entry {key:?} is not a \
                             task id ({error})"
                        ));
                        continue;
                    }
                };
                for need in &claim.holds {
                    snapshot.by_need.entry(need.clone()).or_default().push(Holder {
                        task: task.clone(),
                        session: claim.session.clone(),
                        prefix: prefix.clone(),
                    });
                }
                snapshot
                    .claims
                    .insert((prefix.clone(), task), claim.clone());
            }
        }
        (snapshot, warnings)
    }

    /// Which live holding claims are the caller's own, decided once per command. `owns`
    /// is the claim ownership rule (`commands::own_holds` passes `commands::ownership`:
    /// the resolved identity, else the claim's process proof). Its error is returned, never
    /// read as "not mine".
    pub fn mine(&self, mut owns: impl FnMut(&Claim) -> Result<bool>) -> Result<Mine> {
        let mut mine = Mine::default();
        for (key, claim) in &self.claims {
            if owns(claim)? {
                mine.claims.insert(key.clone());
            }
        }
        Ok(mine)
    }

    /// The first live claim holding `need` that is neither on `task` itself (a takeover is
    /// never a hold) nor the caller's own (the holding session may take more work needing
    /// it). `Mine::default()` means the caller owns nothing, and every hold counts as
    /// another session's.
    pub fn holder(&self, need: &str, task: &TaskId, mine: &Mine) -> Option<&Holder> {
        self.by_need
            .get(need)?
            .iter()
            .find(|holder| holder.task != *task && !mine.owns(holder))
    }
}

/// §4.4 "Blocks other sessions only": `task` is held back when its own project declares a
/// need exclusive and another session's live claim on another task holds a need of that
/// name. The first such need, in name order, with its holder.
pub fn held_back(
    snapshot: &HoldSnapshot,
    vocab: &Vocabulary,
    task: &Task,
    mine: &Mine,
) -> Option<(String, Holder)> {
    crate::needs::exclusive_of(vocab, &task.needs)
        .into_iter()
        .find_map(|need| {
            snapshot
                .holder(&need, &task.id, mine)
                .cloned()
                .map(|holder| (need, holder))
        })
}
```

  In `src/main.rs`, add `mod holds;` after `mod hierarchy;`.

  In `src/error.rs`:
  - After `Halted(String),` in the enum:

```rust
    #[error("{0}")]
    NeedHeld(String),
```

  - In `with_suffix`, after the `Halted` arm:

```rust
            Error::NeedHeld(detail) => Error::NeedHeld(detail + suffix),
```

  - In `kind`, after `Error::Halted(_) => "halted",`:

```rust
            Error::NeedHeld(_) => "need_held",
```

  In `src/commands/mod.rs`, move the ownership rule out of `Ctx`, so a read view (which
  has a `ReadCtx`, not a `Ctx`) applies the same rule as every claim check. Today's
  method body never reads `self`.
  - Delete the method `pub(crate) fn ownership(&mut self, claim: &crate::claims::Claim,
    me: &crate::claims::Resolution) -> Result<Ownership>` from `impl Ctx`, with its doc
    comment ("Spec §6.2.2 steps 2 and 3. …").
  - Add it back as a free function directly after the closing `}` of `impl Ctx` (above
    `pub fn open_ctx`). Its body is unchanged; only `&mut self,` is dropped. Below it, add
    `own_holds`:

```rust
/// Spec §6.2.2 steps 2 and 3. Which of the two established the caller's right to act
/// matters: only proof-only ownership records the claim's own identity, because only
/// then is there no resolved identity that already agrees.
pub(crate) fn ownership(
    claim: &crate::claims::Claim,
    me: &crate::claims::Resolution,
) -> Result<Ownership> {
    if let Some(identity) = me.identity()
        && claim.session == identity.session
    {
        return Ok(Ownership::ByIdentity);
    }
    // Proof is a relay-mode fallback, and it never overrides the explicit pair: agents
    // sharing one process are distinguished by TASKS_SESSION and by nothing else, so an
    // explicit mismatch is foreign however the ancestry looks. Spec constraint §2.1.
    let explicit = std::env::var_os("TASKS_SESSION").is_some_and(|value| !value.is_empty());
    if explicit || !crate::relay::enabled()? {
        return Ok(Ownership::Foreign);
    }
    let proved = crate::claims::proves_ownership(
        claim,
        &crate::relay::ancestry::current_scope(),
        &crate::claims::hostname(),
        crate::claims::boot_id().as_deref(),
        &|key| {
            std::env::var_os(key)
                .and_then(|value| value.into_string().ok())
                .filter(|value| !value.is_empty())
        },
    );
    Ok(if proved {
        Ownership::ByProof
    } else {
        Ownership::Foreign
    })
}

/// Lanes/needs design §4.4 "Blocks other sessions only": the live holding claims in
/// `snapshot` that are the caller's own, by the rule every claim check uses (`ownership`:
/// the resolved identity, else the claim's process proof). A session that moved from
/// native to relay identity therefore keeps its holds as it keeps its claims.
pub(crate) fn own_holds(
    snapshot: &crate::holds::HoldSnapshot,
    me: &crate::claims::Resolution,
) -> Result<crate::holds::Mine> {
    snapshot.mine(|claim| Ok(ownership(claim, me)? != Ownership::Foreign))
}
```

  - Update the call sites. In `src/commands/mod.rs`:
    - `if self.ownership(&existing, &me)? != Ownership::Foreign {` becomes
      `if ownership(&existing, &me)? != Ownership::Foreign {`;
    - `Some(claim) => self.ownership(claim, &resolution)?,` becomes
      `Some(claim) => ownership(claim, &resolution)?,`. This line is in `claim_guard`,
      inside the initializer of `let ownership = …`. A `let` binding is not in scope in
      its own initializer, so the call resolves to the function;
    - `&& ctx.ownership(claim, &me)? == Ownership::Foreign` becomes
      `&& ownership(claim, &me)? == Ownership::Foreign`;
    - `match ctx.ownership(&claim, me) {` becomes `match ownership(&claim, me) {`.

    In `src/commands/status.rs`,
    `Some(claim) => ctx.ownership(claim, &me)? != crate::commands::Ownership::Foreign,`
    becomes
    `Some(claim) => crate::commands::ownership(claim, &me)? != crate::commands::Ownership::Foreign,`.

    In `src/commands/park.rs`, `Some(claim) => match ctx.ownership(claim, &resolution)? {`
    becomes `Some(claim) => match crate::commands::ownership(claim, &resolution)? {`.
  - In the comments, replace `` `Ctx::ownership` `` with `` `ownership` ``. Run
    `grep -rn 'Ctx::ownership' src tests`. It lists `src/commands/mod.rs` (2) and
    `tests/cli.rs` (2) before the edit, and nothing after.

  Replace the body of `Ctx::guard_holds` (Task 2.1) and update its doc comment:

```rust
    /// Lanes/needs design §4.4–§4.5: every acquire records the task's needs that this
    /// project declares exclusive (undeclared names ignored) as the claim's `holds`, and is
    /// refused with `need_held` while another session's live claim on another task holds
    /// one of them. Status and needs never change in one operation, so this always reads
    /// the needs on the record, and `start --force --reason` acquires the same ones.
    ///
    /// "Another session" is decided by `ownership`, as for the claim itself. The caller is
    /// the identity this acquire records: the resolved one, or the claim's own on a
    /// proof-only continuation. A hold whose session string differs but whose process
    /// proof names this caller's harness is therefore its own.
    fn guard_holds(&mut self, task: &Task) -> Result<()> {
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        let me = match self.pending_claim.as_mut() {
            Some((_, ClaimIntent::Acquire(claim))) => {
                claim.holds = holds.clone();
                crate::claims::Resolution::Resolved(crate::claims::continuation_identity(claim))
            }
            _ => unreachable!("claim_guard records an acquire for every move to doing"),
        };
        if holds.is_empty() {
            return Ok(());
        }
        let (snapshot, warnings) = crate::holds::HoldSnapshot::load(
            &self.registry,
            time::OffsetDateTime::now_utc(),
        );
        self.warnings.extend(warnings);
        let mine = own_holds(&snapshot, &me)?;
        if let Some((need, holder)) =
            crate::holds::held_back(&snapshot, &self.project.needs, task, &mine)
        {
            return Err(Error::NeedHeld(format!(
                "{id} needs {need}, held by {} ({}); override with `tasks start {id} --force \
                 --reason \"...\"`",
                holder.task,
                holder.session,
                id = task.id
            )));
        }
        Ok(())
    }
```

  `continuation_identity` builds an `Identity` from a claim's own fields. Here it is
  applied to the pending claim, so its session is exactly the session this acquire
  records. `ownership` reads only that session and, for proof, the other claim and the
  process ancestry.

  `edit --status doing` and the editor move to `doing` pass through `transition`, so they
  are covered with no edit-side change. The editor's `.map_err(keep)` appends the kept-file
  suffix through `with_suffix`.

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --bin tasks holds::tests`
  - `just test-one --test cli every_acquire_path_refuses_a_need_another_session_holds`
  - `just test-one --test cli a_holding_session_takes_more`
  - `just test-one --test cli a_park_releases_the_hold`
  - `just test-one --test cli a_claim_without_a_pid_holds_until_its_ttl`
  - `just test-one --test cli claims_that_cannot_hold_do_not_hold_back_an_acquire`
  - `just test-one --test cli an_acceptance_mode_change_continues_a_natively_held_claim`
  - `just test-one --test cli an_acceptance_explicit_mismatch_stays_foreign_under_one_harness`
    (the moved `ownership` still honours the explicit pair)

- [ ] Run `just test-fast` and `tasks check`.

- [ ] **Commit.**
  `git add src/holds.rs src/main.rs src/error.rs src/commands/mod.rs src/commands/status.rs src/commands/park.rs tests/cli.rs`
  `git commit -m "feat(holds): refuse an acquire whose exclusive need another session holds"`

---

### Task 2.3: `start --force --reason` overrides a held need, with audit notes

**Files**
- Modify `src/commands/mod.rs`:
  - `pub struct Ctx` (57-80), adding two fields;
  - `Ctx::new` (96-110);
  - `Ctx::guard_holds`;
  - `transition` (the `guard_holds` call);
  - new free items `HolderNotes`, `record_need_overrides` and `note_need_holders`,
    placed after `append_stamped_note` (around line 880).
- Modify `src/commands/status.rs:9-102` (`start`).
- Modify `tests/cli.rs`: append the tests.

**Interfaces**
- Consumes: `holds::{Holder, Mine, HoldSnapshot::holder, held_back}`, `own_holds`, and
  `Error::NeedHeld`.
- Produces:
  - the `Ctx` fields `need_reason: Option<String>` and
    `need_overrides: Vec<(String, holds::Holder)>`;
  - `fn guard_holds(&mut self, task: &Task, force: bool) -> Result<()>`;
  - `pub(crate) struct HolderNotes`: the notes owed to same-project holders;
  - `pub(crate) fn record_need_overrides(ctx: &mut Ctx, task: &mut Task) -> Result<Option<HolderNotes>>`.
    It appends the acquired task's notes, which land with that task's own save. It
    returns `Some` when any held need was overridden.
  - `pub(crate) fn note_need_holders(ctx: &mut Ctx, notes: HolderNotes)`, called only
    after `save` has returned `Ok`. A failure there becomes a warning, because the
    acquisition has already landed.

  The notes have these exact forms:
  - on the acquired task: `need override: acquired while <need> held by <holder id>
    (<holder session>): <reason>`;
  - on a same-project holder: `need override: <task id> acquired <need> by <acquiring
    session> while this task held it: <reason>`.

  No holder note is written before the acquiring save succeeds. A refused or rolled-back
  acquire (a validation failure in `save`, a failed record write) therefore never leaves
  "acquired" on a holder.

  `--force` alone, on a held-back task, is a `validation` error.

**Steps**

- [ ] **Write the failing tests.** Append to `tests/cli.rs`:

```rust
#[test]
fn a_forced_start_past_a_held_need_needs_a_reason_and_notes_both_tasks() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let first = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &first]);
    let path = format!("tasks/{second}.md");
    let before = env.read(&sci, &path);

    // --force without --reason is refused when the task is held back.
    let error = error_as(&env, &sci, "agent-b", &["start", &second, "--force"]);
    assert_eq!(error["error"]["kind"], "validation", "{error}");
    assert!(
        error["error"]["detail"].as_str().unwrap().contains("requires --reason"),
        "{error}"
    );
    assert_eq!(env.read(&sci, &path), before);
    assert!(env.json(&sci, &["show", &second])["claim"].is_null());

    let out = json_as(
        &env,
        &sci,
        "agent-b",
        &["start", &second, "--force", "--reason", "capture window"],
    );
    assert!(
        !warnings_of(&out).iter().any(|w| w.contains("--reason was unused")),
        "{out}"
    );
    assert_eq!(holds_of(&env, &sci, &second), serde_json::json!(["quiet"]));
    let target = env.read(&sci, &path);
    assert!(
        target.contains(&format!(
            "need override: acquired while quiet held by {first} (agent-a): capture window"
        )),
        "{target}"
    );
    let holder = env.read(&sci, &format!("tasks/{first}.md"));
    assert!(
        holder.contains(&format!(
            "need override: {second} acquired quiet by agent-b while this task held it: \
             capture window"
        )),
        "{holder}"
    );
}

#[test]
fn a_need_override_notes_a_holder_only_in_the_same_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    hold_vocab(&sci);
    hold_vocab(&fam);
    let holding = id_of(env.json(&fam, &["add", "Fam capture", "-p", "2", "--need", "quiet"]));
    let target = id_of(env.json(&sci, &["add", "Sci capture", "-p", "2", "--need", "quiet"]));
    json_as(&env, &fam, "agent-a", &["start", &holding]);
    let holder_path = format!("tasks/{holding}.md");
    let holder_before = env.read(&fam, &holder_path);

    assert_eq!(
        error_as(&env, &sci, "agent-b", &["start", &target])["error"]["kind"],
        "need_held"
    );
    json_as(
        &env,
        &sci,
        "agent-b",
        &["start", &target, "--force", "--reason", "owner asked"],
    );
    assert!(env.read(&sci, &format!("tasks/{target}.md")).contains(&format!(
        "need override: acquired while quiet held by {holding} (agent-a): owner asked"
    )));
    // A cross-project write would need fam's lock, so fam's record is untouched.
    assert_eq!(env.read(&fam, &holder_path), holder_before);
}

#[test]
fn a_refused_status_and_needs_change_leaves_the_record_and_start_resolves_the_hold() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let first = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let second = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &first]);
    let path = format!("tasks/{second}.md");
    let before = env.read(&sci, &path);

    env.usage(&sci, &["edit", &second, "--status", "doing", "--need", "owner"]);
    assert_eq!(env.read(&sci, &path), before);

    let editor = editor_script(
        &sci,
        "sed -i -e 's/status: todo/status: doing/' \
         -e 's/^needs: \\[quiet\\]/needs: [owner, quiet]/' \"$1\"",
    );
    let out = as_agent(&env, &sci, "agent-b")
        .env("EDITOR", editor)
        .args(["edit", &second])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(err_detail(&out).contains("cannot also change needs"), "{out:?}");
    assert_eq!(env.read(&sci, &path), before);

    // The recorded needs are what the recovery acquires.
    assert_eq!(
        error_as(&env, &sci, "agent-b", &["start", &second])["error"]["kind"],
        "need_held"
    );
    json_as(
        &env,
        &sci,
        "agent-b",
        &["start", &second, "--force", "--reason", "capture window"],
    );
    assert_eq!(holds_of(&env, &sci, &second), serde_json::json!(["quiet"]));
}
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --test cli a_forced_start_past_a_held_need` fails because `--force`
    returns `need_held`, not `validation`.
  - `a_need_override_notes_a_holder_only_in_the_same_project` fails because
    `--force --reason` returns `need_held`.
  - `a_refused_status_and_needs_change_leaves_the_record` fails at the final
    `--force --reason` start.

- [ ] **Implement.**

  In `src/commands/mod.rs`, add to `pub struct Ctx` after `pub takeover: Option<String>,`:

```rust
    /// `start --reason` / `edit --reason`: what lets `--force` past a held need (lanes/needs
    /// design §4.5). Set by the command before the hold guard runs.
    need_reason: Option<String>,
    /// The held needs this command acquires past, with their holders: recorded by the hold
    /// guard, turned into the acquired task's notes by `record_need_overrides` before
    /// `save`. The holders' notes wait for `note_need_holders`, after `save` has landed.
    need_overrides: Vec<(String, crate::holds::Holder)>,
```

  In `Ctx::new`, after `takeover: None,`:

```rust
            need_reason: None,
            need_overrides: Vec::new(),
```

  Replace `Ctx::guard_holds` with:

```rust
    /// Lanes/needs design §4.4–§4.5: every acquire records the task's needs that this
    /// project declares exclusive (undeclared names ignored) as the claim's `holds`, and is
    /// refused with `need_held` while another session's live claim on another task holds
    /// one of them. Status and needs never change in one operation, so this always reads
    /// the needs on the record, and `start --force --reason` acquires the same ones.
    ///
    /// `force` comes only from `start` here (`edit --force` never reaches a move to doing).
    /// It overrides only with a reason; every held need is recorded for the audit notes.
    /// A claim on the task itself never holds it back, so a plain takeover needs none.
    /// "Another session" is decided by `ownership`, as in Task 2.2.
    fn guard_holds(&mut self, task: &Task, force: bool) -> Result<()> {
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        let me = match self.pending_claim.as_mut() {
            Some((_, ClaimIntent::Acquire(claim))) => {
                claim.holds = holds.clone();
                crate::claims::Resolution::Resolved(crate::claims::continuation_identity(claim))
            }
            _ => unreachable!("claim_guard records an acquire for every move to doing"),
        };
        if holds.is_empty() {
            return Ok(());
        }
        let (snapshot, warnings) = crate::holds::HoldSnapshot::load(
            &self.registry,
            time::OffsetDateTime::now_utc(),
        );
        self.warnings.extend(warnings);
        let mine = own_holds(&snapshot, &me)?;
        let Some((need, holder)) =
            crate::holds::held_back(&snapshot, &self.project.needs, task, &mine)
        else {
            return Ok(());
        };
        if !force {
            return Err(Error::NeedHeld(format!(
                "{id} needs {need}, held by {} ({}); override with `tasks start {id} --force \
                 --reason \"...\"`",
                holder.task,
                holder.session,
                id = task.id
            )));
        }
        if self
            .need_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(Error::Validation(format!(
                "--force past a held need requires --reason: {} needs {need}, held by {} ({})",
                task.id, holder.task, holder.session
            )));
        }
        self.need_overrides = holds
            .iter()
            .filter_map(|need| {
                snapshot
                    .holder(need, &task.id, &mine)
                    .map(|holder| (need.clone(), holder.clone()))
            })
            .collect();
        Ok(())
    }
```

  In `transition`, change `ctx.guard_holds(task)?;` to `ctx.guard_holds(task, force)?;`.

  Add after `append_stamped_note`:

```rust
/// The notes a need override owes to holders in this project, written by
/// `note_need_holders` only once the acquiring save has landed. A note saying a task
/// acquired a need must never outlive a refused or rolled-back acquire.
pub(crate) struct HolderNotes {
    owner: String,
    /// Holder task, need, note text.
    notes: Vec<(TaskId, String, String)>,
}

/// Lanes/needs design §4.5: the notes a need override leaves. The acquired task gets one
/// per held need now, so they land with its own save or not at all. A holder gets a
/// matching one only when it is in this project, the only one whose lock this command
/// holds. Those are returned, not written: the caller passes them to `note_need_holders`
/// after `save` succeeds. Both texts are validated here, before anything is saved.
/// `None` when no held need was overridden.
pub(crate) fn record_need_overrides(
    ctx: &mut Ctx,
    task: &mut Task,
) -> Result<Option<HolderNotes>> {
    let overrides = std::mem::take(&mut ctx.need_overrides);
    if overrides.is_empty() {
        return Ok(None);
    }
    let reason = ctx
        .need_reason
        .clone()
        .expect("the hold guard records an override only with a reason");
    let session = match &ctx.pending_claim {
        Some((_, ClaimIntent::Acquire(claim))) => claim.session.clone(),
        _ => unreachable!("a need override is recorded only on an acquire"),
    };
    let owner = owner_name(&ctx.project)?;
    let mut holders = HolderNotes {
        owner: owner.clone(),
        notes: Vec::new(),
    };
    for (need, holder) in &overrides {
        let target_note = format!(
            "need override: acquired while {need} held by {} ({}): {reason}",
            holder.task, holder.session
        );
        crate::format::validate_note_text(&target_note)?;
        if holder.prefix == ctx.project.prefix {
            let holder_note = format!(
                "need override: {} acquired {need} by {session} while this task held it: \
                 {reason}",
                task.id
            );
            crate::format::validate_note_text(&holder_note)?;
            holders
                .notes
                .push((holder.task.clone(), need.clone(), holder_note));
        }
        append_note(task, &owner, &target_note)?;
    }
    Ok(Some(holders))
}

/// Writes the holder notes `record_need_overrides` returned. Call it only after the
/// acquiring `save` returned `Ok`, still under the project lock. The acquisition has
/// landed by then, so a holder that cannot be noted is a warning, never the command's
/// failure.
pub(crate) fn note_need_holders(ctx: &mut Ctx, holders: HolderNotes) {
    for (holder, need, text) in holders.notes {
        let written = ctx.project.read_task(&holder).and_then(|mut held| {
            append_note(&mut held, &holders.owner, &text)?;
            held.updated = crate::time::after(&held.updated)?;
            validate_task(&held)?;
            ctx.project.validate_docs(&held)?;
            // Append-only audit note, as halt's: it skips load's stale-copy guard.
            ctx.project.write_task(&ctx.registry, &held)
        });
        match written {
            Ok(()) => {}
            Err(Error::TaskNotFound(_)) => ctx.warnings.push(format!(
                "{holder} holds {need} but its record is not in this checkout; no override \
                 note was written on it"
            )),
            Err(error) => ctx.warnings.push(format!(
                "the need override landed, but its note on {holder} was not written ({error})"
            )),
        }
    }
}
```

  In `src/commands/status.rs` `start`, insert directly above
  `transition(&mut ctx, &mut task, Status::Doing, force)?;`:

```rust
    // Lanes/needs design §4.5: the same reason lets --force past a held need.
    ctx.need_reason = reason.clone();
```

  Insert directly after the halt block (after the closing `}` of
  `if !blockers.is_empty() { … }`) and above `// Persist the redacted takeover summary`:

```rust
    let holder_notes = super::record_need_overrides(&mut ctx, &mut task)?;
    let overriding_need = holder_notes.is_some();
```

  Change `if reason.is_some() && !reason_used && !overriding_halt {` to:

```rust
    if reason.is_some() && !reason_used && !overriding_halt && !overriding_need {
```

  Replace

```rust
    save(&mut ctx, &mut task)?;
    warn_if_uncommitted_with_worktrees(&mut ctx, &task);
```

  with

```rust
    save(&mut ctx, &mut task)?;
    // Only now that the acquire has landed may a holder be told it happened.
    if let Some(notes) = holder_notes {
        super::note_need_holders(&mut ctx, notes);
    }
    warn_if_uncommitted_with_worktrees(&mut ctx, &task);
```

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --test cli a_forced_start_past_a_held_need`
  - `just test-one --test cli a_need_override_notes_a_holder_only_in_the_same_project`
  - `just test-one --test cli a_refused_status_and_needs_change_leaves_the_record`
  - `just test-one --test cli halt_start` (the halt override and unused-reason tests stay
    green)

- [ ] Run `just test-fast` and `tasks check`.

- [ ] **Commit.**
  `git add src/commands/mod.rs src/commands/status.rs tests/cli.rs`
  `git commit -m "feat(holds): override a held need with start --force --reason and audit notes"`

---

### Task 2.4: Needs changes under live claims, and `edit --reason`

**Files**
- Modify `src/cli.rs:191-256` (`EditArgs`): add `reason` after `force` (line 206).
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if the adopt changes it) at the ops
  commit Task 0.1 recorded as `inventory: Task 2.4 = <sha>`.
- Modify `src/commands/edit.rs`:
  - `run` (80-222): the `--force` check at 81-83, then after `load` (123), after
    `apply_fields` (197), and after `save` (219-220);
  - `editor` (332-334, the equal-status branch).
- Modify `src/commands/mod.rs`: add `Ctx::update_holds` after `refuse_foreign_live_claim`
  (line 232).
- Modify `tests/cli.rs`: append the tests.
- Test: `src/surface.rs` (`parser_surface_equals_table`, the conformance test).

**Ops rows (landed by Task 0.1):** the edit to ops `cli.toml` for this task only. In the
`tasks edit` row's `options` (`path = ["edit"]` under `cli.tasks`), directly after
`{ shared = "force", value = "none" },`:

```toml
  { names = ["--reason"], value = "string" },
```

**Interfaces**
- Consumes:
  - `Ctx::{refuse_foreign_live_claim, resolve_for_guard, claims_mut}`;
  - `ownership` and `own_holds` (Task 2.2);
  - `holds::{HoldSnapshot, Mine}`;
  - `record_need_overrides` and `note_need_holders` (Task 2.3);
  - `Ctx.need_reason` and `Ctx.need_overrides`;
  - Slice 1's `FieldArgs.needs`.
- Produces:
  - `EditArgs { …, pub reason: Option<String> }` (`--reason`);
  - `pub(crate) fn update_holds(&mut self, task: &Task, force: bool) -> Result<()>`.

  The edit rules become:
  - `--force` is valid with `--status done`, or with `--need`.
  - `--reason` is valid only with both `--force` and `--need`.
  - A `needs` change under another session's live claim is refused with `claimed`.
  - Under the caller's own live claim, the change recomputes `holds`. An added exclusive
    need that is held fails with `need_held`, and the error names
    `tasks edit <id> --need <n> --force --reason`.

**Steps**

- [ ] **Write the failing tests.** Append to `tests/cli.rs`:

```rust
#[test]
fn needs_changes_under_another_sessions_live_claim_are_refused_and_other_edits_are_not() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let id = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "owner"]));
    json_as(&env, &sci, "agent-b", &["start", &id]);
    let path = format!("tasks/{id}.md");
    let before = env.read(&sci, &path);

    for args in [
        vec!["edit", id.as_str(), "--need", "quiet"],
        vec!["edit", id.as_str(), "--rm-need", "owner"],
        vec!["edit", id.as_str(), "--no-needs"],
    ] {
        assert_eq!(
            error_as(&env, &sci, "agent-a", &args)["error"]["kind"],
            "claimed",
            "{args:?}"
        );
    }
    let editor = editor_script(&sci, "sed -i 's/^needs: \\[owner\\]/needs: [owner, quiet]/' \"$1\"");
    let out = as_agent(&env, &sci, "agent-a")
        .env("EDITOR", editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(err_kind(&out), "claimed");
    assert_eq!(env.read(&sci, &path), before);

    // A field edit that leaves needs alone keeps today's behaviour: no claim check.
    json_as(&env, &sci, "agent-a", &["edit", &id, "-p", "1", "--tag", "capture"]);
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["priority"], 1);
    assert_eq!(shown["task"]["needs"], serde_json::json!(["owner"]));
    assert_eq!(shown["claim"]["session"], "agent-b");
}

#[test]
fn removing_a_need_under_ones_own_claim_drops_it_from_holds() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let mine = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let theirs = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &mine]);
    assert_eq!(
        error_as(&env, &sci, "agent-b", &["start", &theirs])["error"]["kind"],
        "need_held"
    );

    json_as(&env, &sci, "agent-a", &["edit", &mine, "--rm-need", "quiet"]);
    assert!(holds_of(&env, &sci, &mine).is_null());
    assert_eq!(env.json(&sci, &["show", &mine])["claim"]["session"], "agent-a");
    json_as(&env, &sci, "agent-b", &["start", &theirs]);
}

#[test]
fn adding_an_exclusive_need_under_ones_own_claim_is_an_acquire() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let mine = id_of(env.json(&sci, &["add", "Mine", "-p", "2", "--need", "owner"]));
    let other = id_of(env.json(&sci, &["add", "Other", "-p", "2", "--need", "owner"]));
    let theirs = id_of(env.json(&sci, &["add", "Theirs", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &mine]);
    json_as(&env, &sci, "agent-a", &["start", &other]);
    json_as(&env, &sci, "agent-b", &["start", &theirs]);
    let path = format!("tasks/{mine}.md");
    let before = env.read(&sci, &path);

    let error = error_as(&env, &sci, "agent-a", &["edit", &mine, "--need", "quiet"]);
    assert_eq!(error["error"]["kind"], "need_held", "{error}");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("tasks edit {mine} --need quiet --force --reason")),
        "{error}"
    );
    assert_eq!(env.read(&sci, &path), before);
    assert_eq!(
        error_as(&env, &sci, "agent-a", &["edit", &mine, "--need", "quiet", "--force"])
            ["error"]["kind"],
        "validation"
    );
    assert_eq!(env.read(&sci, &path), before);
    assert!(holds_of(&env, &sci, &mine).is_null());

    let out = json_as(
        &env,
        &sci,
        "agent-a",
        &["edit", &mine, "--need", "quiet", "--force", "--reason", "share the capture"],
    );
    assert!(!warnings_of(&out).iter().any(|w| w.contains("--reason was unused")));
    assert_eq!(holds_of(&env, &sci, &mine), serde_json::json!(["quiet"]));
    assert!(env.read(&sci, &path).contains(&format!(
        "need override: acquired while quiet held by {theirs} (agent-b): share the capture"
    )));
    assert!(env.read(&sci, &format!("tasks/{theirs}.md")).contains(&format!(
        "need override: {mine} acquired quiet by agent-a while this task held it: share the \
         capture"
    )));

    // An editor save has no flags: it refuses and names the edit form.
    let editor = editor_script(&sci, "sed -i 's/^needs: \\[owner\\]/needs: [owner, quiet]/' \"$1\"");
    let out = as_agent(&env, &sci, "agent-a")
        .env("EDITOR", editor)
        .args(["edit", &other])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(err_kind(&out), "need_held");
    assert!(err_detail(&out).contains(&format!("tasks edit {other} --need quiet --force --reason")));
    assert!(holds_of(&env, &sci, &other).is_null());
}

#[test]
fn edit_force_and_reason_are_only_for_adding_a_held_need() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let id = id_of(env.json(&sci, &["add", "Capture", "-p", "2"]));
    assert_eq!(env.fail(&sci, &["edit", &id, "--reason", "why", "-p", "1"]), "validation");
    assert_eq!(env.fail(&sci, &["edit", &id, "--force", "-p", "1"]), "validation");
    assert_eq!(
        env.fail(&sci, &["edit", &id, "--force", "--reason", "why", "-p", "1"]),
        "validation"
    );
    assert_eq!(
        env.fail(&sci, &["edit", &id, "--need", "quiet", "--force", "--reason", "a\nb"]),
        "validation"
    );
    // No claim: adding the need acquires nothing, so the reason goes unused.
    let out = env.json(&sci, &["edit", &id, "--need", "quiet", "--force", "--reason", "why"]);
    assert!(warnings_of(&out).iter().any(|w| w.contains("--reason was unused")), "{out}");
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["needs"], serde_json::json!(["quiet"]));
    assert!(shown["claim"].is_null());
}

#[test]
fn a_need_override_that_save_refuses_leaves_both_records_and_the_claim_store_unchanged() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let mine = id_of(env.json(&sci, &["add", "Mine", "-p", "2", "--need", "owner"]));
    let theirs = id_of(env.json(&sci, &["add", "Theirs", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &mine]);
    json_as(&env, &sci, "agent-b", &["start", &theirs]);
    let records: Vec<(String, String)> = [&mine, &theirs]
        .iter()
        .map(|id| (id.to_string(), env.read(&sci, &format!("tasks/{id}.md"))))
        .collect();
    let store = std::fs::read(env.claim_store("sci")).unwrap();

    // Every hold check passes (--force --reason under agent-a's own claim). Only `save`
    // refuses, on a parent check no earlier step makes: a task cannot be its own parent.
    let error = error_as(
        &env,
        &sci,
        "agent-a",
        &["edit", &mine, "--need", "quiet", "--force", "--reason", "share", "--parent", &mine],
    );
    assert_eq!(error["error"]["kind"], "cycle", "{error}");
    // No "acquired quiet" note on the holder, no note on the target, no claim change.
    for (id, before) in &records {
        assert_eq!(&env.read(&sci, &format!("tasks/{id}.md")), before, "{id}");
    }
    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), store);
}
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --test cli needs_changes_under_another_sessions_live_claim` fails because
    `--need quiet` under agent-b's claim succeeds.
  - `removing_a_need_under_ones_own_claim` fails because `holds` is still `["quiet"]`.
  - `adding_an_exclusive_need_under_ones_own_claim` fails because `edit --need quiet`
    succeeds instead of returning `need_held`.
  - `edit_force_and_reason_are_only_for_adding_a_held_need` fails with `unexpected argument
    '--reason'`, a usage error (exit 2).
  - `a_need_override_that_save_refuses_leaves_both_records` fails the same way, with
    `--reason` unknown. Once the parser has `--reason`, it guards the order: holder notes
    written before `save` would leave `theirs` changed after the `cycle` refusal.

- [ ] **Adopt this task's rows.** In .worktrees/ece1e2-work-selection, run
  `python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, where `<sha>` is the value of
  the `inventory: Task 2.4 = <sha>` note on tasks-ece1e2 (`tasks show tasks-ece1e2`).
  `--at` adopts every row landed up to that commit: Slice 1's rows and the **Ops rows**
  above. It also refreshes `tools/cli_surface.py` when the pair changed. Then run
  `just test-one parser_surface_equals_table`. Expected: FAIL. The table has
  `edit --reason` and the parser does not yet. It passes after the `EditArgs` change
  below.

- [ ] **Implement.**

  In `src/cli.rs`, add to `EditArgs` after `pub force: bool,`:

```rust
    /// With --force and --need: why the need is added while another session holds it.
    /// Noted on the task, and on the holder when it is in this project.
    #[arg(long)]
    pub reason: Option<String>,
```

  In `src/commands/mod.rs`, add to `impl Ctx` after `refuse_foreign_live_claim`:

```rust
    /// Lanes/needs design §4.4: a save that changes `needs` under the caller's own live
    /// claim recomputes the claim's `holds` in the same save. A removed need drops out. An
    /// added exclusive need is an acquire: refused with `need_held` while another session's
    /// live claim on another task holds it, unless `force` and `need_reason` override. Only
    /// added needs are checked; what the claim already holds was checked when acquired.
    /// With no live claim of the caller's there is nothing to recompute. Runs after
    /// `refuse_foreign_live_claim`, under the project lock.
    pub(crate) fn update_holds(&mut self, task: &Task, force: bool) -> Result<()> {
        let Some(existing) = self.claims_mut()?.get(&task.id).cloned() else {
            return Ok(());
        };
        if crate::claims::liveness(&existing) != Liveness::Live {
            return Ok(());
        }
        let me = self.resolve_for_guard()?;
        if ownership(&existing, &me)? == Ownership::Foreign {
            return Ok(());
        }
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        if holds == existing.holds {
            return Ok(());
        }
        let added: Vec<String> = holds
            .iter()
            .filter(|need| !existing.holds.contains(*need))
            .cloned()
            .collect();
        if !added.is_empty() {
            let (snapshot, warnings) = crate::holds::HoldSnapshot::load(
                &self.registry,
                time::OffsetDateTime::now_utc(),
            );
            self.warnings.extend(warnings);
            // "Another session" by the ownership rule, as in `guard_holds`.
            let mine = own_holds(&snapshot, &me)?;
            let held: Vec<(String, crate::holds::Holder)> = added
                .iter()
                .filter_map(|need| {
                    snapshot
                        .holder(need, &task.id, &mine)
                        .map(|holder| (need.clone(), holder.clone()))
                })
                .collect();
            if let Some((need, holder)) = held.first() {
                if !force {
                    return Err(Error::NeedHeld(format!(
                        "{id} needs {need}, held by {} ({}); add it past the hold with \
                         `tasks edit {id} --need {need} --force --reason \"...\"`",
                        holder.task,
                        holder.session,
                        id = task.id
                    )));
                }
                if self
                    .need_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(Error::Validation(format!(
                        "--force past a held need requires --reason: {} needs {need}, held \
                         by {} ({})",
                        task.id, holder.task, holder.session
                    )));
                }
                self.need_overrides = held;
            }
        }
        // `save` treats this as an acquire: store first, restored if the record write fails.
        self.pending_claim = Some((
            task.id.clone(),
            ClaimIntent::Acquire(crate::claims::Claim { holds, ..existing }),
        ));
        Ok(())
    }
```

  In `src/commands/edit.rs` `run`, replace the opening check

```rust
    if args.force && args.status.as_deref() != Some("done") {
        return Err(Error::Validation("--force requires --status done".into()));
    }
```

  with

```rust
    // `--status` conflicts with `--need` at the CLI, so the two uses never meet.
    let adds_needs = !args.fields.needs.is_empty();
    if args.force && args.status.as_deref() != Some("done") && !adds_needs {
        return Err(Error::Validation(
            "--force requires --status done, or --need to add a need another session holds"
                .into(),
        ));
    }
    if args.reason.is_some() && !(args.force && adds_needs) {
        return Err(Error::Validation("--reason requires --force and --need".into()));
    }
    if let Some(reason) = args.reason.as_deref() {
        crate::format::validate_line("reason", reason)?;
    }
```

  Directly after `let mut task = load(&mut ctx, &id)?;`, add:

```rust
    let original_needs = task.needs.clone();
```

  Directly after `apply_fields(&mut ctx, &mut task, &args.fields)?;`, add:

```rust
    // Lanes/needs design §4.4: needs change under no live claim or the caller's own, and
    // under the caller's own the claim's holds follow in the same save.
    // The target's override notes are appended here and land with its save. The
    // holders' notes are held in `holder_notes` until that save has succeeded.
    let mut holder_notes = None;
    if task.needs != original_needs {
        ctx.refuse_foreign_live_claim(&task.id)?;
        ctx.need_reason = args.reason.clone();
        ctx.update_holds(&task, args.force)?;
        holder_notes = super::record_need_overrides(&mut ctx, &mut task)?;
    }
    if args.reason.is_some() && holder_notes.is_none() {
        ctx.warnings
            .push("--reason was unused because no held need was added".into());
    }
```

  At the end of `run`, replace

```rust
    save(&mut ctx, &mut task)?;
    super::follow_holder(&mut ctx, &task.id, None, "the edit landed");
```

  with

```rust
    save(&mut ctx, &mut task)?;
    // Lanes/needs design §4.5: a holder learns of the override only once it has landed.
    // A save refused by its own validation (a parent cycle, say) writes no note anywhere.
    if let Some(notes) = holder_notes {
        super::note_need_holders(&mut ctx, notes);
    }
    super::follow_holder(&mut ctx, &task.id, None, "the edit landed");
```

  In `editor`, replace the equal-status branch

```rust
    if status == original.status {
        ctx.refuse_foreign_live_claim(&original.id).map_err(keep)?;
        ctx.preserve_claim_store(&original.id);
    } else {
```

  with

```rust
    if status == original.status {
        ctx.refuse_foreign_live_claim(&original.id).map_err(keep)?;
        ctx.preserve_claim_store(&original.id);
        // Lanes/needs design §4.4: no flags here, so a held need refuses and names
        // `tasks edit <id> --need <n> --force --reason`.
        if edited.needs != original.needs {
            ctx.update_holds(&edited, false).map_err(keep)?;
        }
    } else {
```

  `update_holds` replaces the `PreserveStore` intent with an `Acquire` only when `holds`
  actually change. A later `reassess` keeps an `Acquire` intent, which already handles
  `clear_escalation`.

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --test cli needs_changes_under_another_sessions_live_claim`
  - `just test-one --test cli removing_a_need_under_ones_own_claim`
  - `just test-one --test cli adding_an_exclusive_need_under_ones_own_claim`
  - `just test-one --test cli edit_force_and_reason_are_only_for_adding_a_held_need`
  - `just test-one --test cli a_need_override_that_save_refuses_leaves_both_records`
  - `just test-one --test cli halt_edit_status_doing` (`--status doing --force` is still
    `validation`)
  - `just test-one parser_surface_equals_table` (the parser surface matches the adopted
    `tools/cli.toml`). If it reports a difference, change the parser to match the adopted
    row. If the row itself is wrong, stop and report it. The correction lands in ops
    through Task 0.1's procedure and is adopted again. Never edit `tools/cli.toml`.

- [ ] Run `cargo fmt`, then `just test-fast` and `just check`. Run `git status`; when it
  shows `tools/cli_surface.py` new or modified, add it to the commit as well.

- [ ] **Commit.**
  `git add src/cli.rs tools/cli.toml src/commands/edit.rs src/commands/mod.rs tests/cli.rs`
  (plus `tools/cli_surface.py` when `git status` showed it changed)
  `git commit -m "feat(holds): guard needs changes by the claim and recompute holds under one's own"`

---

### Task 2.5: Hide held-back work in `ready`, `next` and `prime`

**Files**
- Modify `src/holds.rs`: add `HoldSnapshot::is_empty` and `HeldWarnings`, with a unit test.
- Modify `src/commands/list.rs`:
  - a new `retain_unheld`, after `warn_hidden` (294-299);
  - a call after `warn_hidden(&mut ctx, hidden);` in `ready` (316), `next` (368) and
    `prime` (507).
- Modify `tests/cli.rs`:
  - append the tests;
  - extend `an_acceptance_mode_change_continues_a_natively_held_claim` again, with a
    `ready` under relay identity.

**Interfaces**
- Consumes:
  - `holds::{HoldSnapshot::load, held_back, Mine}`;
  - `commands::own_holds` (Task 2.2);
  - `claims::resolve_identity`;
  - `Project.needs`;
  - `NeedDecl.exclusive`.
- Produces:
  - `pub fn is_empty(&self) -> bool` on `HoldSnapshot`;
  - `#[derive(Debug, Default)] pub struct HeldWarnings`, with
    `pub fn add(&mut self, need: &str, holder: &Holder)` and
    `pub fn into_warnings(self) -> Vec<String>`. Each warning reads `<n> task(s) wait for
    <need>, held by <holder id> (<session>)`;
  - `fn retain_unheld(ctx: &mut ReadCtx, tasks: &mut Vec<Task>, now: OffsetDateTime) -> Result<()>`
    in `list.rs`. Its error is the ownership rule's own, such as an unreadable relay host
    config, which every claim check also raises.

  The gate runs in the three views, after `ready_tasks` and the halt filter. It does not
  run inside `ready_tasks`, so Slice 3's lanes builder can collect steps before holds apply
  (spec §5.1 step 1).

**Steps**

- [ ] **Write the failing unit test.** Append to `mod tests` in `src/holds.rs`:

```rust
    #[test]
    fn held_warnings_aggregate_per_need_and_holder() {
        let sci = Holder {
            task: id("sci-000001"),
            session: "a".into(),
            prefix: "sci".into(),
        };
        let fam = Holder {
            task: id("fam-000002"),
            session: "b".into(),
            prefix: "fam".into(),
        };
        let mut warnings = HeldWarnings::default();
        warnings.add("quiet", &sci);
        warnings.add("quiet", &sci);
        warnings.add("gpu", &fam);
        assert_eq!(
            warnings.into_warnings(),
            [
                "1 task(s) wait for gpu, held by fam-000002 (b)",
                "2 task(s) wait for quiet, held by sci-000001 (a)",
            ]
        );
    }
```

- [ ] **Write the failing end-to-end tests.** Append to `tests/cli.rs`:

```rust
#[test]
fn views_hide_work_held_back_by_another_sessions_hold_across_projects() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    hold_vocab(&sci);
    hold_vocab(&fam);
    let holding = id_of(env.json(&sci, &["add", "Sci capture", "-p", "2", "--need", "quiet"]));
    let sci_waiting = id_of(env.json(&sci, &["add", "Sci rerun", "-p", "2", "--need", "quiet"]));
    let first = id_of(env.json(&fam, &["add", "Fam capture", "-p", "1", "--need", "quiet"]));
    let second = id_of(env.json(
        &fam,
        &["add", "Fam sweep", "-p", "1", "--need", "quiet", "--need", "owner"],
    ));
    let free = id_of(env.json(&fam, &["add", "Fam docs", "-p", "3", "--need", "owner"]));
    let parked = id_of(env.json(&fam, &["add", "Fam resume", "-p", "0", "--need", "quiet"]));
    // A parked-agent candidate, parked before the hold exists: `next` would take it first.
    json_as(&env, &fam, "agent-d", &["start", &parked]);
    json_as(&env, &fam, "agent-d", &["park", &parked, "rerun the capture"]);
    json_as(&env, &sci, "agent-a", &["start", &holding]);

    let wait = |count: usize| format!("{count} task(s) wait for quiet, held by {holding} (agent-a)");
    let ids = |rows: &serde_json::Value| -> Vec<String> {
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect()
    };
    let waits = |value: &serde_json::Value| -> Vec<String> {
        warnings_of(value)
            .into_iter()
            .filter(|warning| warning.contains("wait for"))
            .collect()
    };

    let ready = json_as(&env, &fam, "agent-c", &["ready"]);
    assert_eq!(ids(&ready["tasks"]), [free.clone()]);
    assert_eq!(waits(&ready), [wait(2)]);

    let next = json_as(&env, &fam, "agent-c", &["next"]);
    assert_eq!(next["next"]["task"]["id"], free.as_str(), "{next}");
    assert_eq!(waits(&next), [wait(3)]);

    let prime = json_as(&env, &fam, "agent-c", &["prime"]);
    assert_eq!(ids(&prime["ready"]), [free.clone()]);
    assert_eq!(waits(&prime), [wait(2)]);

    // One warning per need and holder, however many projects the held tasks are in.
    let all = json_as(&env, &fam, "agent-c", &["ready", "--all-projects"]);
    assert_eq!(ids(&all["tasks"]), [free.clone()]);
    assert!(!ids(&all["tasks"]).contains(&sci_waiting));
    assert_eq!(waits(&all), [wait(3)]);

    // The holding session is already using the resource: nothing is held back from it.
    let own = json_as(&env, &fam, "agent-a", &["ready"]);
    for id in [&first, &second, &free] {
        assert!(ids(&own["tasks"]).contains(id), "{own}");
    }
    assert!(waits(&own).is_empty(), "{own}");
}

#[test]
fn an_unreadable_claim_store_leaves_hold_state_unknown_without_failing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let _fam = env.init("fam");
    hold_vocab(&sci);
    let id = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let store = env.claim_store("fam");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&store, "claims = [not toml").unwrap();

    for command in ["ready", "next", "prime"] {
        let out = env.json(&sci, &[command]);
        assert!(out.to_string().contains(&id), "{command}: {out}");
        assert!(
            warnings_of(&out)
                .iter()
                .any(|w| w.starts_with("hold state unknown for fam")),
            "{command}: {out}"
        );
    }
    let started = env.json(&sci, &["start", &id]);
    assert!(
        warnings_of(&started)
            .iter()
            .any(|w| w.starts_with("hold state unknown for fam")),
        "{started}"
    );
    assert_eq!(holds_of(&env, &sci, &id), serde_json::json!(["quiet"]));
}
```

- [ ] **Extend the native-to-relay acceptance test with a view.** In
  `an_acceptance_mode_change_continues_a_natively_held_claim` (as Task 2.2 left it),
  replace

```rust
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" start {second}\n\
```

  with

```rust
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" ready > \"$HOME/ready.json\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" start {second}\n\
```

  and append at the end of the test, after Task 2.2's `holds` assertion on `second`:

```rust
    // `ready` under relay identity applies the same rule: the second task is listed, not
    // hidden behind this session's own hold.
    let ready: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(env.home.path().join("ready.json")).unwrap(),
    )
    .unwrap();
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == second.as_str()),
        "{ready}"
    );
    assert!(
        !warnings_of(&ready).iter().any(|w| w.contains("wait for")),
        "{ready}"
    );
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --bin tasks held_warnings_aggregate` fails to compile: `HeldWarnings` is
    unresolved.
  - `just test-one --test cli views_hide_work_held_back` fails because `ready` lists
    `first`, `second` and `free`.
  - `an_unreadable_claim_store_leaves_hold_state_unknown` fails on `ready`, which has no
    `hold state unknown for fam` warning.
  - `an_acceptance_mode_change_continues_a_natively_held_claim` passes before the view
    gate exists. It is the regression check for the gate: a view that compared session
    strings would hide `second` from `claude-code:c1` behind `c1`'s hold.

- [ ] **Implement.**

  In `src/holds.rs`, add to `impl HoldSnapshot` after `holder`:

```rust
    /// No live hold anywhere: a view can skip resolving the caller's identity.
    pub fn is_empty(&self) -> bool {
        self.by_need.is_empty()
    }
```

  Add after `held_back`:

```rust
/// The views' one warning per need and holder (§4.5), however many tasks wait on it.
#[derive(Debug, Default)]
pub struct HeldWarnings {
    counts: BTreeMap<(String, String, String), usize>,
}

impl HeldWarnings {
    pub fn add(&mut self, need: &str, holder: &Holder) {
        *self
            .counts
            .entry((need.to_string(), holder.task.to_string(), holder.session.clone()))
            .or_default() += 1;
    }

    pub fn into_warnings(self) -> Vec<String> {
        self.counts
            .into_iter()
            .map(|((need, id, session), count)| {
                format!("{count} task(s) wait for {need}, held by {id} ({session})")
            })
            .collect()
    }
}
```

  In `src/commands/list.rs`, add after `fn warn_hidden`:

```rust
/// Lanes/needs design §4.5: drop tasks held back by another session's exclusive hold,
/// with one warning per need and holder; order is unchanged. Whether a hold is this
/// session's is decided as `occupants` decides a claim: by `ownership`, the resolved
/// identity or else the claim's process proof. With neither, every hold counts as another
/// session's. A scope whose projects declare no exclusive need reads no claim store and
/// resolves no identity, so its output is exactly what it was before holds existed.
fn retain_unheld(ctx: &mut ReadCtx, tasks: &mut Vec<Task>, now: OffsetDateTime) -> Result<()> {
    let vocabularies: HashMap<String, crate::needs::Vocabulary> = ctx
        .scope
        .projects()
        .iter()
        .filter(|project| project.needs.values().any(|need| need.exclusive))
        .map(|project| (project.prefix.clone(), project.needs.clone()))
        .collect();
    if vocabularies.is_empty() {
        return Ok(());
    }
    let (snapshot, warnings) = crate::holds::HoldSnapshot::load(&ctx.registry, now);
    ctx.warnings.extend(warnings);
    if snapshot.is_empty() {
        return Ok(());
    }
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let mine = super::own_holds(&snapshot, &me)?;
    let mut held = crate::holds::HeldWarnings::default();
    tasks.retain(|task| {
        let Some(vocabulary) = vocabularies.get(&task.id.prefix) else {
            return true;
        };
        match crate::holds::held_back(&snapshot, vocabulary, task, &mine) {
            Some((need, holder)) => {
                held.add(&need, &holder);
                false
            }
            None => true,
        }
    });
    ctx.warnings.extend(held.into_warnings());
    Ok(())
}
```

  In `ready`, after `warn_hidden(&mut ctx, hidden);`:

```rust
    retain_unheld(&mut ctx, &mut picked.tasks, now)?;
```

  In `next`, after `warn_hidden(&mut ctx, hidden);`. This runs on the pool, so
  parked-agent candidates go through the same gate.

```rust
    retain_unheld(&mut ctx, &mut pool, now)?;
```

  In `prime`, after `warn_hidden(&mut ctx, hidden);`:

```rust
    retain_unheld(&mut ctx, &mut ready, now)?;
```

  `ready --limit` truncates after this, so the limit counts only eligible rows.

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --bin tasks held_warnings_aggregate`
  - `just test-one --test cli views_hide_work_held_back`
  - `just test-one --test cli an_unreadable_claim_store_leaves_hold_state_unknown`
  - `just test-one --test cli an_acceptance_mode_change_continues_a_natively_held_claim`

- [ ] Run `just test-fast` (every existing picker, park, halt and filter test still passes)
  and `tasks check`.

- [ ] **Commit.**
  `git add src/holds.rs src/commands/list.rs tests/cli.rs`
  `git commit -m "feat(holds): hide work held back by another session from ready, next and prime"`

---

### Task 2.6: The host-wide holds lock

**Files**
- Modify `src/holds.rs`: add `lock_path_with` and `lock`, with a unit test.
- Modify `src/commands/mod.rs`:
  - `pub struct Ctx` (a `holds_lock` field right after `pub lock: Option<MutationLock>,`);
  - `Ctx::new`;
  - a new `Ctx::take_holds_lock`, directly above `fn guard_holds`;
  - `Ctx::guard_holds`;
  - `Ctx::update_holds`;
  - `save`, the `ClaimIntent::Acquire` arm (1062-1110, anchor
    `Some((id, ClaimIntent::Acquire(claim))) => {`).
- Modify `tests/cli.rs`: append the helpers and the tests.

**Interfaces**
- Consumes: `claims::{ClaimStore::path_with, MutationLock::{acquire_at, path_with}}`.
- Produces:
  - `pub fn lock_path_with(get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf>`,
    returning `<state>/tasks/claims/.holds.lock`;
  - `pub fn lock() -> Result<claims::MutationLock>`;
  - `Ctx.holds_lock: Option<MutationLock>`;
  - `fn take_holds_lock(&mut self) -> Result<()>` on `Ctx`. It is idempotent, and it
    refuses with `io` when the project lock is not held.

  The lock order is fixed: the project lock first, the holds lock second. The holds lock
  is taken by **every claim replacement that changes `holds`**, and by every acquire that
  records any. That covers an acquire with holds, an acquire replacing a claim that held
  something, and a needs save that adds, reduces or removes the last hold.

  It lives on `Ctx`, so it is released only when the command ends. That is after `save`
  has published the claim, written the record, and, when that write fails, restored the
  previous claim. The rollback restores a hold that the publish had released. Without
  the lock, another project's acquire could take the need in that window and leave two
  holders. `save` refuses a holds-changing `Acquire` made without the lock, so a future
  path that forgets it fails at once instead of racing.

  A release (`done`, `drop`, `park`, `shelve`, a status change away from `doing`) needs
  no holds lock. It writes the record first and removes the claim after. A failed
  removal leaves the hold in place, which errs toward held.

**Steps**

- [ ] **Write the failing unit test.** Append to `mod tests` in `src/holds.rs`:

```rust
    #[test]
    fn the_holds_lock_is_its_own_file_even_beside_a_holds_prefix() {
        let get = |key: &str| (key == "XDG_STATE_HOME").then(|| std::ffi::OsString::from("/xdg"));
        let lock = lock_path_with(get).unwrap();
        assert_eq!(lock, PathBuf::from("/xdg/tasks/claims/.holds.lock"));
        assert_ne!(
            lock,
            crate::claims::MutationLock::path_with("holds", get).unwrap()
        );
    }
```

- [ ] **Write the failing end-to-end tests.** Append to `tests/cli.rs`.

  How the concurrency test is made deterministic: the test holds `.holds.lock` itself, then
  spawns `start` in `sci` and in `fam`. Each child takes its own project lock and then
  queues on the holds lock. The test polls until both project locks are held, which proves
  both children are past their project lock and queued. Only then does it release.
  Whichever child enters first saves its claim before releasing, and the other must see
  that claim. So the outcome is fixed (one success, one `need_held`) even though the order
  is not. An implementation without the lock lets both pass the check. The blocking test
  states that directly: while the test holds the lock, a start that records holds stays
  blocked and one that records none finishes.

  The rollback test uses the same handshake. The record write is made to fail as
  `a_failed_task_write_leaves_no_claim_behind` makes it fail: the tasks directory is
  `0o500`, so reads work and `atomic_write` cannot create its temp file. The test holds
  `.holds.lock`. It makes `sci`'s tasks directory read-only and spawns `edit --rm-need
  quiet` on agent-a's claimed task in `sci`. It also spawns agent-b's `start` of a `quiet`
  task in `fam`. Then it waits until both project locks are held and checks that the
  removal has not finished. That check is the deterministic proof that a removal takes the
  holds lock before `save` publishes the reduced claim. Without the lock, the removal
  publishes `holds = []`, fails its write and rolls back while the test still holds the
  lock. After the release, either order gives one outcome:
  - the acquire runs first and sees agent-a's hold;
  - the removal runs first. It publishes, fails, restores the hold, and only then releases
    the lock, which is kept on `Ctx` until the command ends. The acquire then sees the
    restored hold.

  Both ways, the acquire gets `need_held`. The removal fails with `io` and leaves the
  store byte-identical. No order lets the acquire see the instant in between.

```rust
/// Holds the host-wide holds lock, as a concurrent acquire in another project would.
fn hold_holds_lock(env: &TestEnv) -> File {
    let path = env.claim_store("sci").with_file_name(".holds.lock");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .unwrap();
    file.lock().unwrap();
    file
}

/// Waits, bounded, until another process holds `prefix`'s project lock. A probe that gets
/// the lock drops it at once and tries again. It observes only and never asserts, so no
/// child is stranded by a panic here.
fn wait_for_project_lock_holder(env: &TestEnv, prefix: &str, limit: Duration) {
    let path = env.claim_store(prefix).with_file_name(format!("{prefix}.lock"));
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Ok(probe) = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            && let Err(std::fs::TryLockError::WouldBlock) = probe.try_lock()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `tasks <args>` spawned as `session` (with the runner's live pid), output piped.
fn spawn_as(
    env: &TestEnv,
    dir: &std::path::Path,
    session: &str,
    args: &[&str],
) -> std::process::Child {
    let mut cmd = env.raw(dir);
    cmd.args(args)
        .env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    cmd.spawn().unwrap()
}

#[test]
fn an_acquire_that_records_holds_waits_for_the_holds_lock_and_one_that_does_not_never_does() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    hold_vocab(&sci);
    let quiet = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    // In another project: the blocked start above holds sci's project lock.
    let plain = id_of(env.json(&fam, &["add", "Docs", "-p", "2"]));

    let held = hold_holds_lock(&env);
    let mut holding = spawn_as(&env, &sci, "agent-a", &["start", &quiet]);
    let mut free = spawn_as(&env, &fam, "agent-b", &["start", &plain]);
    // Observations only while the lock is held.
    let free_finished = wait_bounded(&mut free, Duration::from_secs(10));
    let holding_still_blocked = !wait_bounded(&mut holding, Duration::from_millis(300));
    drop(held);

    let free = reap(free, REAP);
    let holding = reap(holding, REAP);
    assert!(free_finished, "a start that records no holds must not take the holds lock");
    assert!(
        holding_still_blocked,
        "a start that records holds must wait for the holds lock"
    );
    assert!(free.expect("the plain start never exited").status.success());
    let holding = holding.expect("the holding start never exited after the release");
    assert!(holding.status.success(), "{}", String::from_utf8_lossy(&holding.stderr));
    assert_eq!(holds_of(&env, &sci, &quiet), serde_json::json!(["quiet"]));
}

#[test]
fn concurrent_starts_in_two_projects_contending_for_one_need_have_one_winner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    hold_vocab(&sci);
    hold_vocab(&fam);
    let in_sci = id_of(env.json(&sci, &["add", "Sci capture", "-p", "2", "--need", "quiet"]));
    let in_fam = id_of(env.json(&fam, &["add", "Fam capture", "-p", "2", "--need", "quiet"]));

    let held = hold_holds_lock(&env);
    let sci_start = spawn_as(&env, &sci, "agent-a", &["start", &in_sci]);
    let fam_start = spawn_as(&env, &fam, "agent-b", &["start", &in_fam]);
    wait_for_project_lock_holder(&env, "sci", Duration::from_secs(10));
    wait_for_project_lock_holder(&env, "fam", Duration::from_secs(10));
    drop(held);

    // Reap both before asserting anything.
    let reaped = [reap(sci_start, REAP), reap(fam_start, REAP)];
    assert!(reaped.iter().all(Option::is_some), "a queued start never exited");
    let outs: Vec<_> = reaped.into_iter().flatten().collect();
    assert_eq!(
        outs.iter().filter(|out| out.status.success()).count(),
        1,
        "exactly one session may take the idle host: {outs:?}"
    );
    let loser = outs.iter().find(|out| !out.status.success()).unwrap();
    assert_eq!(err_kind(loser), "need_held");
    let holding = [(&sci, &in_sci), (&fam, &in_fam)]
        .into_iter()
        .filter(|(dir, id)| holds_of(&env, dir, id) == serde_json::json!(["quiet"]))
        .count();
    assert_eq!(holding, 1);
}

#[test]
fn a_project_whose_prefix_is_holds_can_start_a_task_that_needs_an_exclusive_resource() {
    let mut env = TestEnv::new();
    let holds = env.init("holds");
    hold_vocab(&holds);
    let id = id_of(env.json(&holds, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    // Its project lock is `holds.lock`. Were the host-wide lock that same file, this start
    // would wait on itself forever; `reap` kills it instead of hanging the suite.
    let out = reap(spawn_as(&env, &holds, "agent-a", &["start", &id]), Duration::from_secs(30));
    let out = out.expect("start waited on its own lock");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(holds_of(&env, &holds, &id), serde_json::json!(["quiet"]));
}

#[test]
fn a_hold_removal_whose_record_write_fails_keeps_the_holds_lock_through_its_rollback() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf, std::fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    hold_vocab(&sci);
    hold_vocab(&fam);
    let mine = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let theirs = id_of(env.json(&fam, &["add", "Fam capture", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "agent-a", &["start", &mine]);
    let store_before = std::fs::read(env.claim_store("sci")).unwrap();
    let record_before = env.read(&sci, &format!("tasks/{mine}.md"));

    // The removal's record write fails: reads still work, `atomic_write` cannot create its
    // temp file.
    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let restore = Restore(tasks_dir, original);

    let held = hold_holds_lock(&env);
    let mut removal = spawn_as(&env, &sci, "agent-a", &["edit", &mine, "--rm-need", "quiet"]);
    let acquire = spawn_as(&env, &fam, "agent-b", &["start", &theirs]);
    // Observations only while the lock is held: both children are past their project
    // lock, and the removal has not published, failed and rolled back on its own.
    wait_for_project_lock_holder(&env, "sci", Duration::from_secs(10));
    wait_for_project_lock_holder(&env, "fam", Duration::from_secs(10));
    let removal_waited = !wait_bounded(&mut removal, Duration::from_millis(300));
    drop(held);

    // Reap both, and restore the directory, before asserting anything.
    let [removal, acquire] = [reap(removal, REAP), reap(acquire, REAP)];
    drop(restore);
    assert!(
        removal_waited,
        "removing a hold must take the holds lock before save publishes the reduced claim"
    );
    let removal = removal.expect("the removal never exited after the release");
    let acquire = acquire.expect("the acquire never exited after the release");
    assert_eq!(
        removal.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&removal.stderr)
    );
    assert_eq!(err_kind(&removal), "io");
    // Before the removal, or after its rollback restored the hold: never in between.
    assert_eq!(
        acquire.status.code(),
        Some(1),
        "the acquire saw quiet released: {}",
        String::from_utf8_lossy(&acquire.stdout)
    );
    assert_eq!(err_kind(&acquire), "need_held");
    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), store_before);
    assert_eq!(env.read(&sci, &format!("tasks/{mine}.md")), record_before);
    assert!(env.json(&fam, &["show", &theirs])["claim"].is_null());
}
```

- [ ] **Run the tests and confirm they fail.**
  - `just test-one --bin tasks the_holds_lock_is_its_own_file` fails to compile:
    `lock_path_with` is unresolved.
  - `just test-one --test cli an_acquire_that_records_holds_waits_for_the_holds_lock` fails
    because `holding_still_blocked` is false: the start finishes while the test holds the
    lock.
  - `concurrent_starts_in_two_projects` can pass by luck before the lock exists. Both
    children are queued on nothing, so both usually succeed and it fails with `left: 2,
    right: 1`. It is deterministic only once the lock exists.
  - `a_project_whose_prefix_is_holds` passes before and after. It guards the lock's file
    name.
  - `a_hold_removal_whose_record_write_fails_keeps_the_holds_lock_through_its_rollback`
    fails at `removal_waited`. Without the lock the removal never blocks: it publishes
    `holds = []`, fails its record write, and restores the claim while the test still
    holds the lock. That is the window in which another project's acquire could win the
    need.

- [ ] **Implement.**

  In `src/holds.rs`, add `use crate::claims::MutationLock;` and `use std::ffi::OsString;`
  to the imports (`Result` is imported since Task 2.2). Add after `HeldWarnings`:

```rust
/// `claims/.holds.lock`: one host-wide lock beside the per-prefix stores (§4.4 "Atomic
/// across projects"). A prefix cannot start with `.`, so no project's `<prefix>.lock` is
/// this file, including the lock of a project whose prefix is `holds`.
pub fn lock_path_with(get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf> {
    Ok(ClaimStore::path_with(".holds", get)?.with_file_name(".holds.lock"))
}

/// Taken after the project's mutation lock, never before, by every acquire that records
/// holds and every claim replacement that changes them (`Ctx::take_holds_lock`). It is
/// held around the hold check, the claim save, the record write and any rollback, so two
/// acquires in different projects cannot both win one need, and none sees a hold that a
/// failed write is about to restore.
pub fn lock() -> Result<MutationLock> {
    MutationLock::acquire_at(&lock_path_with(|key| std::env::var_os(key))?)
}
```

  In `src/commands/mod.rs`, add to `pub struct Ctx` directly after
  `pub lock: Option<MutationLock>,`:

```rust
    /// The host-wide holds lock, taken after `lock` by every acquire that records holds
    /// and every claim replacement that changes them. Held until the command ends: past
    /// `save`'s publish, record write, and rollback (lanes/needs design §4.4).
    holds_lock: Option<MutationLock>,
```

  In `Ctx::new`, after `lock: None,`:

```rust
            holds_lock: None,
```

  Add to `impl Ctx`, directly above `fn guard_holds`:

```rust
    /// Lanes/needs design §4.4 "Atomic across projects": the host-wide holds lock, after
    /// the project lock and never before. Kept on `Ctx` until the command ends, so it
    /// spans the hold check, `save`'s publish of the claim, the record write, and the
    /// restore of the previous claim when that write fails.
    fn take_holds_lock(&mut self) -> Result<()> {
        if self.lock.is_none() {
            return Err(Error::Io(
                "the holds lock was requested without the project lock".into(),
            ));
        }
        if self.holds_lock.is_none() {
            self.holds_lock = Some(crate::holds::lock()?);
        }
        Ok(())
    }
```

  In `Ctx::guard_holds`, directly after its first line
  `let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);`, insert:

```rust
        // What the claim this acquire replaces holds: a repeated start's, a takeover's, or
        // a dead entry's. `save` publishes the new claim before writing the record and
        // restores this one if the write fails, so a change either way needs the lock.
        let replaced = self
            .claims_mut()?
            .get(&task.id)
            .map(|claim| claim.holds.clone())
            .unwrap_or_default();
```

  Then, still in `Ctx::guard_holds`, replace

```rust
        if holds.is_empty() {
            return Ok(());
        }
        let (snapshot, warnings) = crate::holds::HoldSnapshot::load(
```

  with

```rust
        if holds.is_empty() && replaced.is_empty() {
            return Ok(());
        }
        // The project lock is already held (every write command takes it first).
        self.take_holds_lock()?;
        if holds.is_empty() {
            // Dropping a hold checks nothing; only its publish and rollback need the lock.
            return Ok(());
        }
        let (snapshot, warnings) = crate::holds::HoldSnapshot::load(
```

  In `Ctx::update_holds`, replace

```rust
        if holds == existing.holds {
            return Ok(());
        }
```

  with

```rust
        if holds == existing.holds {
            return Ok(());
        }
        // Any change, whether a hold is added, reduced, or the last one removed, is
        // published by `save` before the record write and restored if that write fails.
        // The lock spans both, so no other project's acquire sees a need released for the
        // instant in between and takes it.
        self.take_holds_lock()?;
```

  In `save`, replace the opening of the `Acquire` arm

```rust
        Some((id, ClaimIntent::Acquire(claim))) => {
            let store = ctx.claims_mut()?;
```

  with

```rust
        Some((id, ClaimIntent::Acquire(claim))) => {
            // Lanes/needs design §4.4: a claim whose holds differ from the one it replaces
            // is published, and on a failed record write rolled back, only under the holds
            // lock. A path that changed holds without taking it is a bug, refused here
            // before anything is written.
            let replaced = ctx
                .claims_mut()?
                .get(&id)
                .map(|claim| claim.holds.clone())
                .unwrap_or_default();
            if claim.holds != replaced && ctx.holds_lock.is_none() {
                return Err(Error::Io(format!(
                    "the claim on {id} changes its holds without the holds lock"
                )));
            }
            let store = ctx.claims_mut()?;
```

  The interactive editor clears `ctx.lock` before opening the editor, and `holds_lock` is
  still `None` at that point. Both are retaken in order after the editor exits, since
  `transition` and `update_holds` run after `lock_and_revalidate`.

- [ ] **Run the tests and confirm they pass.**
  - `just test-one --bin tasks the_holds_lock_is_its_own_file`
  - `just test-one --test cli an_acquire_that_records_holds_waits_for_the_holds_lock`
  - `just test-one --test cli concurrent_starts_in_two_projects`
  - `just test-one --test cli a_project_whose_prefix_is_holds`
  - `just test-one --test cli a_hold_removal_whose_record_write_fails_keeps_the_holds_lock`
  - `just test-one --test cli removing_a_need_under_ones_own_claim` (a reduction now takes
    the lock and still lands)
  - `just test-one --test cli a_failed_takeover_restores_the_previous_owners_claim` and
    `just test-one --test cli a_failed_task_write_leaves_no_claim_behind` (the rollback
    is unchanged for claims without holds)
  - `just test-one --test cli simultaneous_starts_produce_exactly_one_winner` (the
    project-lock behaviour is unchanged)

- [ ] Run `just test-fast` and `tasks check`.

- [ ] **Commit.**
  `git add src/holds.rs src/commands/mod.rs tests/cli.rs`
  `git commit -m "feat(holds): hold a host-wide lock across every holds change, through rollback"`

---

### Task 2.7: Document holds in the design addenda, the skill and the README

**Files**
- `docs/specs/2026-08-29-tasks-design.md` §5.1: add the `+=` lines inside the output-contract
  block, directly after the needs lines Slice 1 added. If Slice 1 added none there, put them
  after `check       += kinds deferred_goal, defer_status (errors)`.
- `docs/specs/2026-09-05-work-claims-design.md`:
  - "Store", after the paragraph that ends "An empty kind is omitted from the file.";
  - "Locking", at the end of that section.
- `skills/tasks/SKILL.md`, after the halt paragraph that ends "park's `--reason` below is a
  fixed choice from its own vocabulary." (lines 70-76).
- `README.md`:
  - after the halt paragraph that ends "unlike park's fixed-list `--reason`." (line 93);
  - the example `tasks start sci-4f2a9c --force --reason …` (line 297).

**Interfaces**
- Consumes: the shipped behaviour of Tasks 2.1–2.6.
- Produces: no code.

**Steps**

- [ ] **Edit `docs/specs/2026-08-29-tasks-design.md`.** Insert:

```
ClaimInfo   += holds: [string]          the exclusive needs the claim holds; omitted when empty
                                        (lanes/needs design §4.4). Reaches TaskSummary.claim,
                                        prime.doing rows, show, and tasks claims
ready/next  += a task whose exclusive need another session's live claim holds is omitted,
prime          with one warning per need and holder:
               "<n> task(s) wait for <need>, held by <id> (<session>)"
start       += refuses need_held when another session's live claim on another task holds one
               of the task's exclusive needs; --force --reason overrides and notes the task, and
               the holder when it is in the same project; --force alone is refused there.
               edit --status doing and editor saves to doing refuse the same way.
edit        += --reason (only with --force --need under the caller's own claim); a needs change
               under another session's live claim fails claimed
errors      += need_held
```

- [ ] **Edit `docs/specs/2026-09-05-work-claims-design.md`.** After "An empty kind is omitted
  from the file.", insert:

```markdown
A claim entry may also carry `holds = ["quiet"]`: the task's needs that its project
declares exclusive. They are computed on every acquire and recomputed by a needs save under
the holder's own claim (`2026-10-03-lanes-needs-groups-design.md` §4.4). An entry without
the key holds nothing. An older binary drops the key on save, which weakens the hold gate
and never blocks falsely.
```

  At the end of "Locking", insert:

```markdown
An acquire that records a non-empty `holds`, and any claim replacement that changes
`holds` (a needs save that adds, reduces or removes one), also takes the host-wide
`claims/.holds.lock`, after the project lock and never before. It holds the lock until the
command ends: across the hold check, the claim save, the record write, and the restore of
the previous claim when that write fails. Two projects therefore cannot both acquire one
exclusive need, and no acquire can see a hold that a failed write is about to restore. A
release takes no holds lock, because it removes the claim only after the record is
written. A prefix cannot start with `.`, so no project lock can be that file.
```

- [ ] **Edit `skills/tasks/SKILL.md`.** After the halt paragraph, insert, indented like it:

```markdown
   A need declared `exclusive` names a host resource (an idle host, a device) and serves
   one session at a time across every project on the host that uses the name. `start`
   records the task's exclusive needs as holds on its claim. `ready`, `next` and `prime`
   hide other sessions' steps that need a held resource, with a warning naming the holder.
   Starting such a step (or `edit --status doing`) fails with `need_held`. Override it
   deliberately with `tasks start <id> --force --reason "<why>"`, which notes the task and
   a same-project holder. Your own session may start more steps that need what it already
   holds. A park or a dead session releases the hold. A long run that keeps the resource
   should heartbeat with `tasks note`, or a pid-less claim lapses with its TTL. Change the
   needs of a claimed task only under your own claim (`start --force` takes it over first).
   Adding an exclusive need that another session holds takes `tasks edit <id> --need <n>
   --force --reason "<why>"`.
```

- [ ] **Edit `README.md`.** After the halt paragraph, insert:

```markdown
An exclusive need (`exclusive = true` under `[needs]`) is held by the live claim of the
task that needs it, across every registered project on the host. Other sessions' steps
that need it leave `ready`, `next` and `prime` with a warning naming the holder, and `start`
refuses them with `need_held`. `tasks start <id> --force --reason "…"` overrides with a
note on both tasks. A park or a dead session releases the hold.
```

  Change the example comment `# audited halt override` to
  `# audited halt or held-need override`.

- [ ] **Verify.** Run `tasks check`, then `just test-fast`. Some Rust tests read the design
  doc and README, so these files stay on the full check.

- [ ] **Commit.**
  `git add docs/specs/2026-08-29-tasks-design.md docs/specs/2026-09-05-work-claims-design.md skills/tasks/SKILL.md README.md`
  `git commit -m "docs(holds): document exclusive holds, need_held, and edit --reason"`

---

### Coverage of spec §10 "Holds"

| §10 case | Test |
|---|---|
| `start`, `edit --status doing`, and an editor save record `holds` | `every_acquire_path_records_the_tasks_exclusive_needs_as_holds` (2.1) |
| Each of them refuses with `need_held` | `every_acquire_path_refuses_a_need_another_session_holds` (2.2) |
| `--force --reason` overrides and writes the note(s); `--force` alone is refused | `a_forced_start_past_a_held_need_needs_a_reason_and_notes_both_tasks` and `a_need_override_notes_a_holder_only_in_the_same_project` (2.3) |
| A held task is absent from `ready`/`next`, with one aggregated warning, across two projects | `views_hide_work_held_back_by_another_sessions_hold_across_projects` (2.5) |
| The holding session can start a second task | `a_holding_session_takes_more_…` (2.2) |
| A park releases the hold | `a_park_releases_the_hold` (2.2) |
| A dead claim holds nothing | `claims_that_cannot_hold_…` (2.2) and `a_live_claim_holds_and_a_dead_one_does_not` (unit) |
| `edit --rm-need` updates `holds` | `removing_a_need_under_ones_own_claim_drops_it_from_holds` (2.4) |
| `edit --need quiet` while held is refused; `--force --reason` succeeds and notes | `adding_an_exclusive_need_under_ones_own_claim_is_an_acquire` (2.4) |
| A needs change under a foreign live claim is refused; other field edits are not | `needs_changes_under_another_sessions_live_claim_are_refused_and_other_edits_are_not` (2.4) |
| Status and needs together: usage error, editor refusal, record unchanged, `start --force --reason` resolves | `a_refused_status_and_needs_change_leaves_the_record_and_start_resolves_the_hold` (2.3) |
| A `--force` takeover of a `quiet` task needs no `--reason` | `a_holding_session_takes_more_and_a_takeover_of_the_held_task_is_not_held_back` (2.2) |
| A project with prefix `holds` | `a_project_whose_prefix_is_holds_…` (2.6) and `the_holds_lock_is_its_own_file_…` (unit) |
| A claim in an unregistered leftover store | `claims_that_cannot_hold_…` (2.2) |
| An unreadable store warns and does not fail | `an_unreadable_claim_store_leaves_hold_state_unknown_without_failing` (2.5) and `an_unreadable_store_warns_…` (unit) |
| Two concurrent cross-project `start`s have exactly one winner | `concurrent_starts_in_two_projects_…` and `an_acquire_that_records_holds_waits_for_the_holds_lock_…` (2.6) |
| A hold removal whose record write fails never lets a concurrent acquire take the need | `a_hold_removal_whose_record_write_fails_keeps_the_holds_lock_through_its_rollback` (2.6) |
| A session that moved from native to relay identity keeps its holds, in `start` and in `ready` | `an_acceptance_mode_change_continues_a_natively_held_claim` (extended in 2.2 and 2.5) and `whose_hold_it_is_follows_the_ownership_rule_…` (unit) |
| An override that `save` refuses leaves no note on either task and no claim change | `a_need_override_that_save_refuses_leaves_both_records_and_the_claim_store_unchanged` (2.4) |
| A TTL-only claim holds until its TTL | `a_claim_without_a_pid_holds_until_its_ttl` (2.2, end-to-end and unit) |
| An older entry without `holds` holds nothing | `claims_that_cannot_hold_…` (2.2), `an_entry_written_before_holds_existed_holds_nothing` (unit), and `a_claim_written_before_holds_loads_holding_nothing_…` (2.1 unit) |

### Spec ambiguities resolved

1. **Where the view gate sits.** The contract says "gates in `ready_tasks`/`next`/`prime`".
   The gate is applied in the `ready`, `next` and `prime` views, right after `ready_tasks`
   and the halt filter, not inside `ready_tasks`. Spec §5.1 step 1 needs Slice 3's lanes
   builder to collect steps *before* holds apply.
2. **The holder note's text.** The spec gives no text for it. It is:
   `need override: <task> acquired <need> by <session> while this task held it: <reason>`.
3. **How `<holder>` is rendered** in the target note and the refusals. It is
   `<holder id> (<session>)`, the same form as the view warning.
4. **The error kind for `--force` without `--reason` on a held-back task.** It is
   `validation`, as halt's "--force under a halt requires --reason" is.
5. **How "this session" is decided, in views and on acquire.** By the claim ownership
   rule (`commands::ownership`, moved out of `Ctx` in Task 2.2). That is the resolved
   identity's session, or, in relay mode without an explicit `TASKS_SESSION`, the
   holding claim's process proof. Session strings are not compared directly: the
   supported move from `c1` to `claude-code:c1` would otherwise hold a session back
   behind its own hold. This changes the contract's `holder(…, session: Option<&str>)`
   and `held_back(…, session: Option<&str>)` to take `&Mine`, which
   `HoldSnapshot::mine`/`own_holds` build once per command. `None` becomes
   `&Mine::default()`.
6. **When views read the stores.** A view reads the claim stores and resolves identity only
   when a project in scope declares an exclusive need. This keeps every existing view's
   output and warnings unchanged (§10 "existing tests pass unchanged").
7. **Which record the holder note goes on, and when.** It is written to this checkout's
   copy of the holder's record, under the project lock this command already holds. It is
   written only after the acquiring `save` has returned `Ok`, so a refused or rolled-back
   acquire never leaves "acquired" on a holder. Once the acquire has landed, a holder
   note that cannot be written is a warning, never the command's failure. If the record
   is not in this checkout, a warning replaces the note. Halt writes its "attempted" note
   before the save, to its registered authority checkout. Its text records an attempt,
   so that order is right for halt and wrong here.
8. **What a needs save checks.** A needs save under one's own claim checks only the
   *added* exclusive needs. Holds the claim already has were checked, or overridden, when
   they were acquired.
9. **Override notes cover every held need.** `held_back` still returns the first, as the
   contract defines it.
10. **`edit --force` is widened.** It is valid with `--status done`, or with `--need`.
    `edit --reason` requires both `--force` and `--need`. Given with no held need to
    override, it warns "unused", as `start` does.
11. **`tools/cli.toml` is vendored from ops**, and ops holds the authority copy. Task 0.1
    lands Task 2.4's **Ops rows** block in ops. Task 2.4 adopts it with
    `vendored adopt cli.toml --at <sha>` and never edits `tools/cli.toml`.
12. **The holds lock's scope.** It is taken by every claim replacement that changes
    `holds` (added, reduced, or emptied) and by every acquire that records any. It is
    held on `Ctx` through `save`'s publish, record write and rollback. `save` refuses a
    holds-changing `Acquire` made without it. A release takes no lock, because it removes
    the claim only after the record write.
13. **Assumed Slice 1 names.**
    - `FieldArgs.needs`, `EditArgs.rm_needs` and `EditArgs.no_needs`.
    - The flow-list frontmatter `needs: [a, b]`, which the editor scripts `sed` against.
    - The editor refusal's kind is not asserted, only its "cannot also change needs" text.

---

## Slice 3 — Lanes

Spec: `docs/specs/2026-10-03-lanes-needs-groups-design.md` §3, §5, and the lane parts of
§7–§10. This slice starts from the end state of Slices 1 (Needs) and 2 (Exclusive holds)
and uses their symbols exactly as the interface contract names them:
`crate::needs::{NeedDecl, Vocabulary, Without, exclusive_of}`,
`Project.needs`, `Task.needs`, `crate::cli::WithoutArgs`, `commands::list::vocabularies`,
`crate::holds::{HoldSnapshot, Holder, held_back}`, and the error kinds `unknown_need` and
`need_held`.

`Without` hides a task only where the task's own project declares the need (Slice 1 Task
1.6). It is built with `Without::from_env(&flag, &list::vocabularies(&ctx.scope))` and applied with
`hides(task, needs::vocabulary_of(vocabs, task))` or `retain(&mut tasks, &vocabs)`.

CLI inventory: Tasks 3.1, 3.6, and 3.7 change the parser. Each carries its exact ops
`cli.toml` edit in a block titled **"Ops rows (landed by Task 0.1)"**, which Task 0.1
lands as its own ops commit; the task itself never edits ops and never writes
`tools/cli.toml` by hand or with `cp`. It adopts its rows with
`python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, run in
`.worktrees/ece1e2-work-selection`, where `<sha>` is read from the task's
`inventory: Task 3.<n> = <sha>` note on tasks-ece1e2 (the step gives the command).
Because `--at` adopts every row landed up to that commit, these tasks run in the order of
their ops commits: 3.1, then 3.6, then 3.7.

Line numbers below are from `ce51d81` (spec approval). Slices 1–2 shift them, so every
edit also quotes the code it anchors on. Where Slices 1–2 added parameters or lines to a
function this slice also edits (`ready_tasks`, `next`, `prime`, `parked::candidates`, the
`Command::Next`/`Prime` dispatch arms), keep theirs and add this slice's beside them; the
code below shows the `ce51d81` text plus this slice's change, with the Slice 1–2 additions
named in the prose.

Every task ends with `cargo fmt`, `just test-fast`, `tasks check`, and a commit. The
pre-commit hook runs `just check` (rustfmt, clippy `-D warnings`, `tasks check`), so no
task may leave an unused item behind; the tasks are ordered so each one's new code has a
caller in the same commit.

### File Structure

- `src/model.rs` — `Task.lane: bool` after `parallel`; `task_with` test helper.
- `src/format.rs` — `KEYS` gains `"lane"` between `"parallel"` and `"needs"`; `parse_task`,
  `serialize_task`, round-trip test.
- `src/defer.rs`, `src/halt.rs`, `src/complexity.rs`, `src/repo.rs`, `src/query.rs`,
  `src/hierarchy.rs`, `src/periodic.rs`, `src/similarity.rs`, `src/commands/add.rs` —
  `lane: false` in each `Task` literal (test helpers and `add::blank`).
- `src/cli.rs` — `FieldArgs.lane`, `EditArgs.no_lane`, `FilterArgs.under`, `Next.under`,
  the `Lanes` subcommand.
- `src/commands/mod.rs` — `apply_fields` sets `lane`; `save` runs `validate_lanes`;
  `pub mod lanes`; dispatch for `Next` and `Lanes`.
- `src/commands/edit.rs` — `--no-lane`, the flag-presence list, the paused-on-marking
  warning on both the flag and editor paths.
- `src/error.rs` — `Error::NestedLane`, kind `nested_lane`.
- `src/hierarchy.rs` — `is_goal`, `enclosing_lane`, `lane_of`, `paused_lane`,
  `validate_lanes`; `validate_periodic`/`validate_defer` refuse a lane.
- `src/repo.rs` — `write_task` and `create_task_with` run the lane and goal validators.
- `src/commands/status.rs` — `start` runs `validate_lanes`.
- `src/commands/check.rs` — `periodic_goal`/`deferred_goal` through `is_goal`;
  `nested_lane` error; `childless_lane` warning.
- `src/query.rs` — `Picked.paused`, `paused_omissions`.
- `src/commands/list.rs` — `ready_tasks` through `is_goal` and the paused gate; `ready`
  and `next` warn through `warn_paused`; `--under` on `list`, `list --parked`, `ready`, `next`;
  `halt_snapshots` becomes `pub(super)`; `prime` builds `lanes`.
- `src/commands/parked.rs` — `candidates` through `is_goal`, the paused gate, and the
  filter.
- `src/filter.rs` — `TaskFilter.under`/`subtree`, `resolve_under`, `Fields.id`.
- `src/output.rs` — `TaskSummary`/`ParkedRow` `lane` and `in_lane`; `ShowFields.in_lane`;
  `≡` type mark; `LanesOut`; `Output::Lanes`; `PrimeOut.lanes`; `lane_lines` pretty.
- `src/commands/show.rs` — `describe` fills `in_lane`.
- `src/lanes.rs` (new) — the builder: `LaneRow`, `LaneState`, `HeldStep`, `HeldBy`,
  `Causes`, `Inputs`, `build`, `guidance`.
- `src/main.rs` — `mod lanes;`.
- `src/commands/lanes.rs` (new) — `tasks lanes` and the shared `rows` used by `prime`.
- `tests/cli.rs` — end-to-end tests for every lanes and lanes-view case of spec §10.
- `tools/cli.toml` (and `tools/cli_surface.py` when `vendored adopt` refreshes it) —
  adopted from ops in Tasks 3.1, 3.6, and 3.7; never edited in this repository.
- `docs/specs/2026-08-29-tasks-design.md` — §3.1 field row, §5 usage, §5.1 `+=` lines.
- `docs/specs/2026-09-03-task-hierarchy-design.md` — §4.3 paused-lane addendum.
- `skills/tasks/SKILL.md`, `skills/scope/SKILL.md`, `README.md` — §9 lanes guidance.

Field checklist (`2026-08-29-tasks-design.md` §3.4) for `lane`, all in Task 3.1:
1 §3.1 row · 2 §5.1 `+=` line for `Task`, `TaskSummary`, `ParkedRow` · 3 §5 `add` and
`edit` usage with `--no-lane` · 4 SKILL.md "Never edit" flag list (the scoped-task recipe
is for leaf work; Task 3.9 adds a lane recipe beside it) · 5 README add/edit block ·
6 model.rs (`Task`, `task_with`), format.rs (`KEYS`, `parse_task`, `serialize_task`,
round-trip test), output.rs (`TaskSummary::of`, `ParkedRow::resolved`,
`ParkedRow::unresolved`, `row` helper), cli.rs, add.rs (`blank`), edit.rs.

---

### Task 3.1: The `lane` field

**Files**
- Modify `src/model.rs`: `Task` (after `pub parallel: bool,`, line 467); `task_with`
  (line 128).
- Modify `src/format.rs`: `KEYS` (lines 6–32), `parse_task` literal (line 121),
  `serialize_task` (after line 422), tests (after line 749).
- Modify `Task` literals: `src/defer.rs:145`, `src/halt.rs:112`, `src/complexity.rs:116`,
  `src/repo.rs:725`, `src/query.rs:283`, `src/hierarchy.rs:305`, `src/periodic.rs:75`,
  `src/similarity.rs:99`, `src/commands/add.rs:28`.
- Modify `src/cli.rs`: `FieldArgs` (after line 156), `EditArgs` (after line 212).
- Modify `src/commands/mod.rs`: `apply_fields` (after line 641).
- Modify `src/commands/edit.rs`: `run` (lines 94–95, after line 137).
- Modify `src/output.rs`: `TaskSummary` (after line 207), `TaskSummary::of` (line 410),
  `ParkedRow` (after line 454), `resolved` (line 503), `unresolved` (line 533),
  `type_letter` (lines 1371–1376), test `row` helper (line 1957), new test.
- Modify `tests/cli.rs`: new test.
- Modify `docs/specs/2026-08-29-tasks-design.md` (§3.1 table line 120, §5 add line 259,
  edit line 331, §5.1 block), `skills/tasks/SKILL.md` (line 161), `README.md` (Use block,
  after line 274).
- Adopt `tools/cli.toml` (and `tools/cli_surface.py` if refreshed) with `vendored adopt`
  at the Task 3.1 ops commit.

**Ops rows (landed by Task 0.1)** — the edit to ops `cli.toml` for this task only, in the
`tasks` CLI section:

- `[[cli.tasks.commands]] path = ["add"]`: after
  `  { names = ["--parallel"], value = "none" },` insert
  ```toml
    { names = ["--lane"], value = "none" },
  ```
- `[[cli.tasks.commands]] path = ["edit"]` (the `tasks` row with `verb = "edit"`, not the
  `wali` one): after `  { names = ["--no-parallel"], value = "none" },` insert
  ```toml
    { names = ["--no-lane"], value = "none" },
  ```
  and after `  { names = ["--parallel"], value = "none" },` insert
  ```toml
    { names = ["--lane"], value = "none" },
  ```

**Interfaces**
- Consumes: `Task.needs` and `KEYS` entry `"needs"` (Slice 1).
- Produces: `pub lane: bool` on `Task`, `TaskSummary`, `ParkedRow`; `FieldArgs.lane: bool`;
  `EditArgs.no_lane: bool`; frontmatter key `lane` (written only when true, after
  `parallel`, before `needs`).

- [ ] **Step 1: Write the failing tests.**

  `src/format.rs`, in `mod tests` after `parallel_false_in_a_file_is_dropped_on_the_next_write`:

  ```rust
      #[test]
      fn lane_round_trips_after_parallel_and_is_omitted_when_false() {
          let mut t = parse_task(MINIMAL, "x").unwrap();
          assert!(!t.lane, "absent key reads as false");
          assert!(!serialize_task(&t).contains("lane"), "false is never written");

          t.parallel = true;
          t.lane = true;
          let text = serialize_task(&t);
          assert!(
              text.contains("\nparallel: true\nlane: true\n"),
              "unquoted, right after parallel: {text}"
          );
          assert!(parse_task(&text, "x").unwrap().lane);

          let bad = MINIMAL.replace("depends: []", "lane: yes\ndepends: []");
          let err = parse_task(&bad, "x").unwrap_err().to_string();
          assert!(err.contains("lane must be true or false"), "{err}");
      }
  ```

  `src/output.rs`, in `mod tests` after `any_type_tree_finds_a_recurring_descendant`:

  ```rust
      #[test]
      fn a_lane_row_carries_the_lane_mark_in_the_type_column() {
          let mut lane = row("xx-000001", false);
          lane.lane = true;
          let rows = [lane, row("xx-000002", false)];
          assert!(any_type(&rows));
          let text = table(
              &rows,
              DateColumn::Updated,
              &plain(),
              0,
              false,
              true,
              Wrap::NONE,
          );
          let lines: Vec<&str> = text.lines().collect();
          assert!(lines[0].contains("todo    ≡ 2026-09-06"), "{}", lines[0]);
          assert!(lines[1].contains("todo      2026-09-06"), "{}", lines[1]);
      }
  ```

  `tests/cli.rs`, after `prime_shows_roadmap_and_closeout`:

  ```rust
  #[test]
  fn lane_field_round_trips_through_add_edit_and_every_row() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "--parallel"]));
      let raw = env.read(&sci, &format!("tasks/{lane}.md"));
      assert!(raw.contains("\nparallel: true\nlane: true\n"), "{raw}");
      assert_eq!(env.json(&sci, &["show", &lane])["task"]["lane"], true);

      let plain = id_of(env.json(&sci, &["add", "Plain"]));
      assert_eq!(
          env.json(&sci, &["show", &plain])["task"]["lane"],
          false,
          "always present, like parallel"
      );
      let list = env.json(&sci, &["list"]);
      let row = |id: &str| {
          list["tasks"]
              .as_array()
              .unwrap()
              .iter()
              .find(|row| row["id"] == id)
              .unwrap()
              .clone()
      };
      assert_eq!(row(&lane)["lane"], true);
      assert_eq!(row(&plain)["lane"], false);

      env.json(&sci, &["edit", &plain, "--lane"]);
      assert_eq!(env.json(&sci, &["show", &plain])["task"]["lane"], true);
      env.json(&sci, &["edit", &plain, "--no-lane"]);
      assert!(
          !env.read(&sci, &format!("tasks/{plain}.md")).contains("lane"),
          "false is never written"
      );
      env.usage(&sci, &["edit", &plain, "--lane", "--no-lane"]);

      as_agent(&env, &sci, "agent-a")
          .args(["park", &lane, "split it"])
          .assert()
          .success();
      assert_eq!(env.json(&sci, &["list", "--parked"])["tasks"][0]["lane"], true);

      let text = env.pretty(&sci, &["list"]);
      let line = text.lines().find(|line| line.contains(&lane)).unwrap();
      assert!(line.contains("≡ "), "lane rows carry the mark: {text}");
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one lane_round_trips_after_parallel` — fails to compile:
  `error[E0609]: no field 'lane' on type 'model::Task'` (and on `TaskSummary`).
  `just test-one --test cli lane_field_round_trips` — the first `add` exits 2 with
  `unexpected argument '--lane'`, so `env.json` panics.

- [ ] **Step 3: Implement.**

  `src/model.rs`, after `pub parallel: bool,`:

  ```rust
      /// A goal meant to proceed alongside other lanes; its members are the `parent` tree
      /// below it. Hand-set by `add`/`edit --lane`, cleared by `--no-lane`. See
      /// docs/specs/2026-10-03-lanes-needs-groups-design.md §3.
      pub lane: bool,
  ```

  In `task_with` (model.rs tests), and in every `Task` literal listed under **Files**, add
  after `parallel: false,`:

  ```rust
              lane: false,
  ```

  `src/format.rs` — `KEYS` grows by one (Slice 1 left it at 26 with `"needs"` after
  `"parallel"`):

  ```rust
  const KEYS: [&str; 27] = [
      "id",
      "title",
      "status",
      "priority",
      "size",
      "complexity",
      "process",
      "parallel",
      "lane",
      "needs",
      "every",
      // ... the remaining keys unchanged
  ];
  ```

  In `parse_task`, after `parallel: boolean("parallel")?,`:

  ```rust
          lane: boolean("lane")?,
  ```

  In `serialize_task`, directly after the `if t.parallel { ... }` block and before Slice 1's
  `needs` push:

  ```rust
      // Raw for the same reason as `parallel`: a quoted `"true"` would read back but sit
      // out of step with every other scalar.
      if t.lane {
          pairs.push(("lane".into(), Value::Raw("true".into())));
      }
  ```

  `src/cli.rs`, `FieldArgs`, after `pub parallel: bool,`:

  ```rust
      /// Mark this goal as a lane: an effort meant to proceed alongside other lanes. On
      /// `edit` this sets the flag; see `--no-lane` to clear it.
      #[arg(long)]
      pub lane: bool,
  ```

  `EditArgs`, after `pub no_parallel: bool,`:

  ```rust
      /// Clear the lane marker.
      #[arg(long, conflicts_with = "lane")]
      pub no_lane: bool,
  ```

  `src/commands/mod.rs`, `apply_fields`, after the `if fields.parallel { ... }` block:

  ```rust
      // Setting only. `edit --no-lane` clears it before this runs, like --no-parallel.
      if fields.lane {
          task.lane = true;
      }
  ```

  `src/commands/edit.rs`, `run`: in `has_flags`, after `|| args.no_parallel` add

  ```rust
          || fields.lane
          || args.no_lane
  ```

  and after the `if args.no_parallel { task.parallel = false; }` block:

  ```rust
      if args.no_lane {
          task.lane = false;
      }
  ```

  `src/output.rs`: `TaskSummary`, after `pub parallel: bool,`:

  ```rust
      /// Marked as a lane (lanes design §3.1); always present, like `parallel`.
      pub lane: bool,
  ```

  `TaskSummary::of`, after `parallel: task.parallel,`: `lane: task.lane,`.
  `ParkedRow`, after `pub parallel: bool,`: `pub lane: bool,`.
  `ParkedRow::resolved`, after `parallel: summary.parallel,`: `lane: summary.lane,`.
  `ParkedRow::unresolved`, after `parallel: false,`: `lane: false,`.
  Test helper `row`, after `parallel,`: `lane: false,`.

  Replace `type_letter` (lines 1371–1376):

  ```rust
  /// The one-letter type marker for a summary row: `≡` for a lane, `p` for a record
  /// carrying a cadence, and nothing otherwise. A lane is a goal and never recurs, so the
  /// two never compete for the slot. A future type is another arm here, another letter in
  /// the same slot; the column is reserved once per output (see `any_type`), so it is never
  /// a layout change.
  fn type_letter(row: &TaskSummary) -> Option<char> {
      if row.lane {
          return Some('≡');
      }
      row.periodic.as_ref().map(|_| 'p')
  }
  ```

  Docs (field checklist items 1–5):

  `docs/specs/2026-08-29-tasks-design.md` §3.1, after the `parallel` row:

  ```markdown
  | `lane`     | bool                | no       | A goal meant to proceed alongside other lanes; its members are the `parent` tree below it. Set by `add`/`edit --lane`, cleared by `--no-lane`. Omitted when false; written after `parallel`. No lane may sit below another (`nested_lane`). See `2026-10-03-lanes-needs-groups-design.md`. |
  ```

  §5 usage: in the `tasks add` line insert `[--lane]` after `[--parallel]`; in the
  `tasks edit` line replace `[--parallel|--no-parallel]` with
  `[--parallel|--no-parallel] [--lane|--no-lane]`.

  §5.1, after the last `+=` line of the block (after Slices 1–2's own lines):

  ```text
  Task        += lane: bool                     always present, like parallel; written after parallel
  TaskSummary += lane: bool
  ParkedRow   += lane: bool                     false when unresolved
  ```

  `skills/tasks/SKILL.md` line 161, in the flag list, insert `--lane/--no-lane/` after
  `--process/--no-process/`.

  `README.md` Use block, after the `tasks ready --parallel -n 3` line:

  ```text
      tasks add "Capture lane" --lane -p 1 -b "Why it runs; first milestone"  # an effort beside the others
      tasks edit <id> --no-lane        # clear the lane marker
  ```

- [ ] **Step 4: Adopt this task's rows.** In `.worktrees/ece1e2-work-selection`:

  ```bash
  SHA=$(tasks show tasks-ece1e2 | grep -o 'inventory: Task 3\.1 = [0-9a-f]*' | awk '{print $NF}')
  test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
  ```

  It also refreshes `tools/cli_surface.py` when the pair changed. Run
  `just test-one parser_surface_equals_table`. Expected: PASS (the `--lane` and
  `--no-lane` rows match the parser).

- [ ] **Step 5: Run the tests and see them pass.**

  `just test-one lane_round_trips_after_parallel` and
  `just test-one a_lane_row_carries_the_lane_mark` pass;
  `just test-one --test cli lane_field_round_trips` passes.

- [ ] **Step 6: Gate.** `cargo fmt`, then `just test-fast` (all green; existing `parallel`
  and type-column tests unchanged), then `tasks check`.

- [ ] **Step 7: Commit.**

  ```bash
  git add src/model.rs src/format.rs src/defer.rs src/halt.rs src/complexity.rs \
    src/repo.rs src/query.rs src/hierarchy.rs src/periodic.rs src/similarity.rs \
    src/commands/add.rs src/cli.rs src/commands/mod.rs src/commands/edit.rs \
    src/output.rs tests/cli.rs docs/specs/2026-08-29-tasks-design.md \
    skills/tasks/SKILL.md README.md tools/cli.toml
  # tools/cli_surface.py too, when the adopt changed it:
  git add $(git ls-files --modified --others --exclude-standard tools/cli_surface.py)
  git commit -m "feat(lanes): add the lane field with --lane and --no-lane (tasks-ece1e2)"
  ```

---

### Task 3.2: A lane is a goal at every goal site

**Files**
- Modify `src/hierarchy.rs`: new `is_goal` (after `is_active`, line 137);
  `validate_periodic`/`validate_defer` (lines 152–194) share `refuse_goal`; new test.
- Modify `src/repo.rs`: `create_task_with` (after line 650).
- Modify `src/commands/list.rs`: `ready_tasks` (line 194–199).
- Modify `src/commands/parked.rs`: `candidates` (line 190).
- Modify `src/commands/check.rs`: `periodic_goal`/`deferred_goal` (lines 209–235); new
  `childless_lane` warning.
- Modify `tests/cli.rs`: new test.

**Interfaces**
- Produces: `pub fn is_goal(task: &Task, has_children: bool) -> bool` in
  `src/hierarchy.rs`; check warning kind `childless_lane`.
- Consumes: `Task.lane` (3.1).

- [ ] **Step 1: Write the failing tests.**

  `src/hierarchy.rs` tests, after `open_descendants_see_through_a_closed_middle_node`:

  ```rust
      #[test]
      fn a_lane_is_a_goal_with_or_without_children() {
          let mut lane = task("xx-000001", None, Status::Todo);
          assert!(!is_goal(&lane, false));
          assert!(is_goal(&lane, true));
          lane.lane = true;
          assert!(is_goal(&lane, false), "a lane is a goal before its first child");
      }
  ```

  `tests/cli.rs`:

  ```rust
  #[test]
  fn a_childless_lane_is_never_ready_never_parked_and_never_deferred_or_recurring() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "0"]));
      let other = id_of(env.json(&sci, &["add", "Other", "-p", "3"]));
      let ready: Vec<String> = env.json(&sci, &["ready"])["tasks"]
          .as_array()
          .unwrap()
          .iter()
          .map(|row| row["id"].as_str().unwrap().to_string())
          .collect();
      assert_eq!(ready, [other.clone()], "a lane is a goal even with no children");

      as_agent(&env, &sci, "agent-a")
          .args(["park", &lane, "split it"])
          .assert()
          .success();
      assert_eq!(
          env.json(&sci, &["next"])["next"]["task"]["id"],
          other,
          "a lane is never a parked candidate"
      );

      assert_eq!(env.fail(&sci, &["edit", &lane, "--defer", "2099-01-01"]), "validation");
      assert_eq!(env.fail(&sci, &["edit", &lane, "--every", "7d"]), "validation");
      assert_eq!(
          env.fail(&sci, &["add", "Sweep lane", "--lane", "--every", "7d"]),
          "validation"
      );
      assert_eq!(
          env.fail(&sci, &["add", "Later lane", "--lane", "--defer", "2099-01-01"]),
          "validation"
      );
      let plain = id_of(env.json(&sci, &["add", "Plain"]));
      env.json(&sci, &["edit", &plain, "--every", "7d"]);
      assert_eq!(
          env.fail(&sci, &["edit", &plain, "--lane"]),
          "validation",
          "a recurrence cannot become a lane"
      );

      let check = env.check(&sci);
      assert!(
          check["warnings"]
              .as_array()
              .unwrap()
              .iter()
              .any(|f| f["kind"] == "childless_lane" && f["id"] == lane),
          "{check}"
      );
      assert!(check["errors"].as_array().unwrap().is_empty(), "{check}");
      env.json(&sci, &["add", "Step", "--parent", &lane]);
      assert!(
          !env.check(&sci)["warnings"].to_string().contains("childless_lane"),
          "a lane with a step is not childless"
      );

      // A cadence written onto a lane by hand is a periodic goal.
      let path = sci.join(format!("tasks/{lane}.md"));
      let text = std::fs::read_to_string(&path).unwrap();
      std::fs::write(&path, text.replace("depends: []\n", "every: 7d\ndepends: []\n")).unwrap();
      let out = env.cmd(&sci).args(["check"]).output().unwrap();
      assert_eq!(out.status.code(), Some(1));
      let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
      assert!(
          check["errors"]
              .as_array()
              .unwrap()
              .iter()
              .any(|f| f["kind"] == "periodic_goal" && f["id"] == lane),
          "{check}"
      );
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one a_lane_is_a_goal_with_or_without_children` — fails to compile:
  `cannot find function 'is_goal' in this scope`.
  `just test-one --test cli a_childless_lane_is_never_ready` — fails at the first
  assertion: `ready` lists the lane beside `Other`.

- [ ] **Step 3: Implement.**

  `src/hierarchy.rs`, after `is_active`:

  ```rust
  /// Lanes design §3.2: the one test for "this task is a goal", used wherever a goal is
  /// treated specially. A lane is a goal even before its first child exists.
  pub fn is_goal(task: &Task, has_children: bool) -> bool {
      has_children || task.lane
  }
  ```

  Replace `validate_periodic` and `validate_defer` (lines 152–194):

  ```rust
  /// spec §4.7: `readiness` excludes a goal, so a cadence on one could never fire. Refuse it
  /// at the write rather than leave a silent dead end.
  pub fn validate_periodic(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
      if task.every.is_none() {
          return Ok(());
      }
      refuse_goal(project, registry, task, "be a recurrence")
  }

  /// A deferred goal would never appear in a picker.
  pub fn validate_defer(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
      if task.defer.is_none() {
          return Ok(());
      }
      refuse_goal(project, registry, task, "be deferred")
  }

  /// Refuses `task` when it is a goal (`is_goal`). A lane is refused without a scan, and a
  /// record not yet on disk has no children, so the project is scanned only when the field
  /// is set on an existing record that is not a lane, which is rare.
  fn refuse_goal(project: &Project, registry: &Registry, task: &Task, what: &str) -> Result<()> {
      let kids: Vec<String> = if task.lane || !project.task_path(&task.id).is_file() {
          Vec::new()
      } else {
          children(&project.scan()?, &task.id, registry)
              .iter()
              .map(|kid| kid.id.to_string())
              .collect()
      };
      if !is_goal(task, !kids.is_empty()) {
          return Ok(());
      }
      Err(Error::Validation(if kids.is_empty() {
          format!(
              "{} is a lane and cannot {what}; a lane is a goal, and a goal is never ready",
              task.id
          )
      } else {
          format!(
              "{} has children ({}) and cannot {what}; a task with children is a goal, and a \
               goal is never ready",
              task.id,
              kids.join(", ")
          )
      }))
  }
  ```

  `src/repo.rs`, `create_task_with`, after
  `crate::hierarchy::validate_parent(self, registry, task)?;`:

  ```rust
          // A new record has no children, so these refuse only a lane (lanes design §3.2).
          crate::hierarchy::validate_periodic(self, registry, task)?;
          crate::hierarchy::validate_defer(self, registry, task)?;
  ```

  `src/commands/list.rs`, `ready_tasks`, replace lines 194–199:

  ```rust
          // Lanes design §3.2: a lane is a goal even before its first child exists.
          let goal = crate::hierarchy::is_goal(
              task,
              !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty(),
          );
          match readiness(task, goal, &lookup, now) {
              Readiness::Ready => ready.push(task.clone()),
              Readiness::Deferred => deferred.push(task.clone()),
              Readiness::Not => {}
          }
  ```

  `src/commands/parked.rs`, `candidates`, replace
  `|| !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty()` with:

  ```rust
              || crate::hierarchy::is_goal(
                  task,
                  !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty(),
              )
  ```

  `src/commands/check.rs`, replace the `every` and `defer` goal blocks (lines 209–235):

  ```rust
          if task.every.is_some() {
              let kids = crate::hierarchy::children(&tasks, &task.id, &ctx.registry);
              if crate::hierarchy::is_goal(task, !kids.is_empty()) {
                  let detail = if kids.is_empty() {
                      "is a lane and has a cadence; a goal is never ready, so the cadence can \
                       never fire"
                          .to_string()
                  } else {
                      format!(
                          "has a cadence and children ({}); a goal is never ready, so the cadence can never fire",
                          kids.iter().map(|kid| kid.id.to_string()).collect::<Vec<_>>().join(", ")
                      )
                  };
                  errors.push(finding(Some(task), file.clone(), "periodic_goal", detail));
              }
          }
          if task.defer.is_some() {
              let kids = crate::hierarchy::children(&tasks, &task.id, &ctx.registry);
              if crate::hierarchy::is_goal(task, !kids.is_empty()) {
                  let detail = if kids.is_empty() {
                      "is a lane and is deferred; a goal is never ready, so the deferral hides \
                       nothing"
                          .to_string()
                  } else {
                      format!(
                          "is deferred and has children ({}); a goal is never ready, so the deferral hides nothing",
                          kids.iter().map(|kid| kid.id.to_string()).collect::<Vec<_>>().join(", ")
                      )
                  };
                  errors.push(finding(Some(task), file.clone(), "deferred_goal", detail));
              }
              // spec §3.2: the status rule lives here and in the writers, not in parsing.
              if !crate::defer::can_carry(task.status) {
                  // ... the defer_status block, unchanged
              }
          }
          // Lanes design §3.3: a lane is often filed before its steps exist, so a childless
          // one is a reminder, not an error.
          if task.lane
              && crate::hierarchy::is_active(task)
              && crate::hierarchy::children(&tasks, &task.id, &ctx.registry).is_empty()
          {
              warnings.push(finding(
                  Some(task),
                  file.clone(),
                  "childless_lane",
                  format!(
                      "is a lane with no steps yet; add them with `tasks add \"<step>\" --parent {}`",
                      task.id
                  ),
              ));
          }
  ```

- [ ] **Step 4: Run the tests and see them pass.**

  `just test-one a_lane_is_a_goal_with_or_without_children`,
  `just test-one --test cli a_childless_lane_is_never_ready`, and the existing
  `just test-one --test cli check_reports_deferred_goals` (message text unchanged).

- [ ] **Step 5: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 6: Commit.**

  ```bash
  git add src/hierarchy.rs src/repo.rs src/commands/list.rs src/commands/parked.rs \
    src/commands/check.rs tests/cli.rs
  git commit -m "feat(lanes): treat a lane as a goal before it has children (tasks-ece1e2)"
  ```

---

### Task 3.3: `in_lane` on every row

**Files**
- Modify `src/hierarchy.rs`: new `enclosing_lane`, `lane_of` (after `is_goal`); new test.
- Modify `src/output.rs`: import `TaskId`; `ShowFields` (after `parent`, line 150);
  `TaskSummary` (after `parent`, line 230), `TaskSummary::of` (line 421); `ParkedRow`
  (after `parent`, line 474), `resolved` (line 519), `unresolved` (line 549); test `row`.
- Modify `src/commands/show.rs`: `describe` literal (line 159).
- Modify `tests/cli.rs`: new test.

**Interfaces**
- Produces: `pub fn enclosing_lane<'a>(all: &'a [Task], task: &Task, registry: &Registry) -> Option<&'a Task>`;
  `pub fn lane_of(all: &[Task], task: &Task, registry: &Registry) -> Option<TaskId>`;
  `in_lane: Option<TaskId>` (sparse) on `TaskSummary`, `ParkedRow`, `ShowFields`.

- [ ] **Step 1: Write the failing tests.**

  `src/hierarchy.rs` tests:

  ```rust
      #[test]
      fn lane_of_is_the_task_itself_or_its_nearest_lane_ancestor() {
          let registry = Registry::default();
          let mut lane = task("xx-000001", None, Status::Todo);
          lane.lane = true;
          let goal = task("xx-000002", Some("xx-000001"), Status::Todo);
          let step = task("xx-000003", Some("xx-000002"), Status::Todo);
          let loose = task("xx-000004", None, Status::Todo);
          let all = [lane.clone(), goal, step.clone(), loose.clone()];
          assert_eq!(lane_of(&all, &step, &registry), Some(lane.id.clone()));
          assert_eq!(lane_of(&all, &lane, &registry), Some(lane.id.clone()));
          assert!(enclosing_lane(&all, &lane, &registry).is_none(), "strictly above");
          assert_eq!(lane_of(&all, &loose, &registry), None);
          let a = task("xx-000005", Some("xx-000006"), Status::Todo);
          let b = task("xx-000006", Some("xx-000005"), Status::Todo);
          assert_eq!(
              lane_of(&[a.clone(), b], &a, &registry),
              None,
              "a parent loop ends the walk"
          );
      }
  ```

  `tests/cli.rs`:

  ```rust
  #[test]
  fn in_lane_names_the_nearest_lane_on_ready_next_show_and_parked_rows() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let fam = env.init("fam");
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "1"]));
      let sub = id_of(env.json(&sci, &["add", "Sub-goal", "--parent", &lane]));
      let step = id_of(env.json(&sci, &["add", "Step", "--parent", &sub, "-p", "0"]));
      let loose = id_of(env.json(&sci, &["add", "Loose", "-p", "1"]));
      let row = |value: &serde_json::Value, id: &str| {
          value["tasks"]
              .as_array()
              .unwrap()
              .iter()
              .find(|row| row["id"] == id)
              .unwrap()
              .clone()
      };

      let ready = env.json(&sci, &["ready"]);
      assert_eq!(row(&ready, &step)["in_lane"], lane, "{ready}");
      assert!(row(&ready, &loose).get("in_lane").is_none(), "sparse: {ready}");
      let list = env.json(&sci, &["list"]);
      assert_eq!(row(&list, &lane)["in_lane"], lane, "a lane names itself");
      assert_eq!(row(&list, &sub)["in_lane"], lane);

      let next = env.json(&sci, &["next"]);
      assert_eq!(next["next"]["task"]["id"], step);
      assert_eq!(next["next"]["in_lane"], lane, "{next}");
      assert_eq!(env.json(&sci, &["show", &sub])["in_lane"], lane);
      assert!(env.json(&sci, &["show", &loose]).get("in_lane").is_none());

      as_agent(&env, &sci, "agent-a")
          .args(["park", &step, "ask", "--waiting-on", "user"])
          .assert()
          .success();
      let parked = env.json(&sci, &["list", "--parked"]);
      assert_eq!(parked["tasks"][0]["in_lane"], lane, "{parked}");
      assert_eq!(env.json(&sci, &["prime"])["parked"][0]["in_lane"], lane);

      // Another project's lane, read across the registry, names its own lane.
      let fam_lane = id_of(env.json(&fam, &["add", "Fam lane", "--lane"]));
      let fam_step = id_of(env.json(&fam, &["add", "Fam step", "--parent", &fam_lane]));
      let wide = env.json(&sci, &["ready", "--all-projects"]);
      assert_eq!(row(&wide, &fam_step)["in_lane"], fam_lane, "{wide}");
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one lane_of_is_the_task_itself` — fails to compile: `cannot find function
  'lane_of'`. `just test-one --test cli in_lane_names_the_nearest_lane` — fails:
  `row(&ready, &step)["in_lane"]` is `Null`.

- [ ] **Step 3: Implement.**

  `src/hierarchy.rs`, after `is_goal`:

  ```rust
  /// The nearest lane strictly above `task` in `all`, walking `parent` links. A visited set
  /// ends a corrupt loop, and a parent missing from `all` ends the walk.
  pub fn enclosing_lane<'a>(all: &'a [Task], task: &Task, registry: &Registry) -> Option<&'a Task> {
      let mut seen = std::collections::HashSet::new();
      let mut current = task.parent.as_ref().map(|parent| registry.canonical_id(parent));
      while let Some(id) = current {
          if !seen.insert(id.clone()) {
              return None;
          }
          let ancestor = all.iter().find(|candidate| candidate.id == id)?;
          if ancestor.lane {
              return Some(ancestor);
          }
          current = ancestor
              .parent
              .as_ref()
              .map(|parent| registry.canonical_id(parent));
      }
      None
  }

  /// Lanes design §3.6: the lane a row belongs to, which is the task itself when it is a
  /// lane, else its nearest lane ancestor. Computed from the scan, never stored.
  pub fn lane_of(all: &[Task], task: &Task, registry: &Registry) -> Option<TaskId> {
      if task.lane {
          return Some(registry.canonical_id(&task.id));
      }
      enclosing_lane(all, task, registry).map(|lane| lane.id.clone())
  }
  ```

  `src/output.rs`: change the model import to
  `use crate::model::{Complexity, Process, Size, Status, Task, TaskId};`.

  `ShowFields`, after `pub parent: Option<Related>,`:

  ```rust
      /// The nearest lane at or above the task (lanes design §3.6); omitted when none.
      #[serde(skip_serializing_if = "Option::is_none")]
      pub in_lane: Option<TaskId>,
  ```

  `TaskSummary`, after `pub parent: Option<String>,`:

  ```rust
      /// The nearest lane at or above this task: its own id when it is a lane. Computed
      /// from the scan, never stored (lanes design §3.6).
      #[serde(skip_serializing_if = "Option::is_none")]
      pub in_lane: Option<TaskId>,
  ```

  `TaskSummary::of`, after `parent: task.parent.as_ref().map(ToString::to_string),`:

  ```rust
              in_lane: crate::hierarchy::lane_of(all, task, registry),
  ```

  `ParkedRow`, after `pub parent: Option<String>,`:

  ```rust
      #[serde(skip_serializing_if = "Option::is_none")]
      pub in_lane: Option<TaskId>,
  ```

  `ParkedRow::resolved`: `in_lane: summary.in_lane,` after `parent: summary.parent,`;
  `ParkedRow::unresolved`: `in_lane: None,` after `parent: None,`; test `row`:
  `in_lane: None,` after `parent: None,`.

  `src/commands/show.rs`, `describe`, in the `ShowFields` literal after `parent,`:

  ```rust
          in_lane: crate::hierarchy::lane_of(all, &task, registry),
  ```

- [ ] **Step 4: Run the tests and see them pass.**

  `just test-one lane_of_is_the_task_itself` and
  `just test-one --test cli in_lane_names_the_nearest_lane`.

- [ ] **Step 5: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 6: Commit.**

  ```bash
  git add src/hierarchy.rs src/output.rs src/commands/show.rs tests/cli.rs
  git commit -m "feat(lanes): name each row's lane with in_lane (tasks-ece1e2)"
  ```

---

### Task 3.4: No nested lanes, on every write path and in `check`

**Files**
- Modify `src/error.rs`: variant (after line 38), `with_suffix` (after line 89), `kind`
  (after line 121).
- Modify `src/hierarchy.rs`: new `validate_lanes`, `lane_above` (after `validate_parent`,
  line 65).
- Modify `src/repo.rs`: `write_task` (after line 416), `create_task_with` (after its
  `validate_parent` call).
- Modify `src/commands/mod.rs`: `save` (after line 1058).
- Modify `src/commands/status.rs`: `start` (after line 54).
- Modify `src/commands/check.rs`: per-task loop (beside the 3.2 `childless_lane` block).
- Modify `tests/cli.rs`: new test.

**Interfaces**
- Produces: `Error::NestedLane(String)` with kind `"nested_lane"`;
  `pub fn validate_lanes(project: &Project, registry: &Registry, task: &Task) -> Result<()>`;
  check error kind `nested_lane`.
- Consumes: `enclosing_lane` (3.3), `descendants`.

- [ ] **Step 1: Write the failing test.**

  ```rust
  #[test]
  fn nested_lanes_are_refused_on_every_write_path_and_reported_by_check() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let outer = id_of(env.json(&sci, &["add", "Outer", "--lane"]));
      let child = id_of(env.json(&sci, &["add", "Child", "--parent", &outer]));

      // add --lane --parent, directly and further down
      assert_eq!(
          env.fail(&sci, &["add", "Inner", "--lane", "--parent", &outer]),
          "nested_lane"
      );
      assert_eq!(
          env.fail(&sci, &["add", "Deeper", "--lane", "--parent", &child]),
          "nested_lane"
      );
      // edit --lane with a lane above
      assert_eq!(env.fail(&sci, &["edit", &child, "--lane"]), "nested_lane");
      // edit --lane with a lane below
      let top = id_of(env.json(&sci, &["add", "Top"]));
      env.json(&sci, &["edit", &outer, "--parent", &top]);
      assert_eq!(env.fail(&sci, &["edit", &top, "--lane"]), "nested_lane");
      // re-parenting a subtree that contains a lane under a lane
      let other = id_of(env.json(&sci, &["add", "Other", "--lane"]));
      assert_eq!(env.fail(&sci, &["edit", &top, "--parent", &other]), "nested_lane");
      assert_eq!(env.fail(&sci, &["edit", &outer, "--parent", &other]), "nested_lane");
      // ordinary writes inside a lane still land
      env.json(&sci, &["note", &child, "still writable"]);
      env.json(&sci, &["edit", &child, "-p", "1"]);

      // an editor save
      let set = editor_script(
          &sci,
          "sed -i 's/^depends: \\[\\]$/lane: true\\ndepends: []/' \"$1\"",
      );
      let out = env
          .cmd(&sci)
          .env("EDITOR", &set)
          .args(["edit", &child])
          .output()
          .unwrap();
      assert_eq!(out.status.code(), Some(1));
      let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
      assert_eq!(err["error"]["kind"], "nested_lane", "{err}");
      assert_eq!(
          env.json(&sci, &["show", &child])["task"]["lane"],
          false,
          "nothing written"
      );

      // check reports nesting written by hand
      let path = sci.join(format!("tasks/{child}.md"));
      let text = std::fs::read_to_string(&path).unwrap();
      std::fs::write(&path, text.replace("depends: []\n", "lane: true\ndepends: []\n")).unwrap();
      let out = env.cmd(&sci).args(["check"]).output().unwrap();
      assert_eq!(out.status.code(), Some(1));
      let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
      assert!(
          check["errors"]
              .as_array()
              .unwrap()
              .iter()
              .any(|f| f["kind"] == "nested_lane" && f["id"] == child),
          "{check}"
      );
  }
  ```

- [ ] **Step 2: Run it and see it fail.**

  `just test-one --test cli nested_lanes_are_refused` — the first `env.fail` panics: the
  add succeeds (exit 0) instead of exiting 1.

- [ ] **Step 3: Implement.**

  `src/error.rs`, after `Halted(String),`:

  ```rust
      #[error("{0}")]
      NestedLane(String),
  ```

  In `with_suffix`, after the `Halted` arm:
  `Error::NestedLane(detail) => Error::NestedLane(detail + suffix),`.
  In `kind`, after the `Halted` arm: `Error::NestedLane(_) => "nested_lane",`.

  `src/hierarchy.rs`, after `validate_parent` (add `use std::collections::HashSet;` to the
  imports):

  ```rust
  /// Lanes design §3.3: no lane may have a lane as an ancestor. Covers both directions for
  /// the record being written: a lane above it when it is a lane, and a lane below it when
  /// it becomes a lane or moves under one. Ancestors are read from disk, as in
  /// `validate_parent`, which runs first and has already refused a parent loop. The project
  /// is scanned only when the record's subtree could newly meet a lane, so ordinary writes
  /// inside a lane stay cheap; nesting that was already on disk is `check`'s to report.
  pub fn validate_lanes(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
      let task_id = registry.canonical_id(&task.id);
      let above = lane_above(project, registry, task)?;
      if task.lane
          && let Some(outer) = &above
      {
          return Err(Error::NestedLane(format!(
              "{task_id} cannot be a lane inside lane {outer}; a sub-effort inside a lane is an \
               ordinary child goal"
          )));
      }
      // A record not yet on disk has no children, so nothing can sit below it.
      let stored = match project.read_task(&task_id) {
          Ok(stored) => stored,
          Err(Error::TaskNotFound(_)) => return Ok(()),
          Err(error) => return Err(error),
      };
      let canonical =
          |parent: &Option<TaskId>| parent.as_ref().map(|id| registry.canonical_id(id));
      let moved = canonical(&stored.parent) != canonical(&task.parent);
      let became_lane = task.lane && !stored.lane;
      let moved_under_a_lane = !task.lane && above.is_some() && moved;
      if !became_lane && !moved_under_a_lane {
          return Ok(());
      }
      let all = project.scan()?;
      let Some(inner) = descendants(&all, &task_id, registry)
          .into_iter()
          .find(|descendant| descendant.lane)
      else {
          return Ok(());
      };
      Err(Error::NestedLane(match above {
          Some(outer) => format!(
              "moving {task_id} under lane {outer} would put lane {} inside it",
              inner.id
          ),
          None => format!("{task_id} cannot be a lane: lane {} is below it", inner.id),
      }))
  }

  /// The nearest lane above `task` on disk. A loop or a missing parent ends the walk.
  fn lane_above(project: &Project, registry: &Registry, task: &Task) -> Result<Option<TaskId>> {
      let mut seen = HashSet::new();
      let mut current = task.parent.as_ref().map(|parent| registry.canonical_id(parent));
      while let Some(id) = current {
          if !seen.insert(id.clone()) {
              break;
          }
          let ancestor = match project.read_task(&id) {
              Ok(ancestor) => ancestor,
              Err(Error::TaskNotFound(_)) => break,
              Err(error) => return Err(error),
          };
          if ancestor.lane {
              return Ok(Some(id));
          }
          current = ancestor
              .parent
              .as_ref()
              .map(|parent| registry.canonical_id(parent));
      }
      Ok(None)
  }
  ```

  `src/repo.rs`, `write_task`, after `crate::hierarchy::validate_parent(self, registry, task)?;`:

  ```rust
          crate::hierarchy::validate_lanes(self, registry, task)?;
  ```

  and the same line in `create_task_with` directly after its `validate_parent` call (before
  the 3.2 `validate_periodic` line).

  `src/commands/mod.rs`, `save`, after
  `crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, task)?;` (it must refuse
  before the claim store is touched):

  ```rust
      crate::hierarchy::validate_lanes(&ctx.project, &ctx.registry, task)?;
  ```

  `src/commands/status.rs`, `start`, after
  `crate::hierarchy::validate_defer(&ctx.project, &ctx.registry, &task)?;`:

  ```rust
      crate::hierarchy::validate_lanes(&ctx.project, &ctx.registry, &task)?;
  ```

  `src/commands/check.rs`, in the per-task loop, before the `childless_lane` block:

  ```rust
          // Lanes design §3.3: nesting a merge or a hand edit left behind.
          if task.lane
              && let Some(outer) = crate::hierarchy::enclosing_lane(&tasks, task, &ctx.registry)
          {
              errors.push(finding(
                  Some(task),
                  file.clone(),
                  "nested_lane",
                  format!(
                      "is a lane inside lane {}; a sub-effort inside a lane is an ordinary \
                       child goal",
                      outer.id
                  ),
              ));
          }
  ```

- [ ] **Step 4: Run the test and see it pass.**

  `just test-one --test cli nested_lanes_are_refused`; also
  `just test-one --test cli editor_path_validates_parent` and
  `just test-one --test cli parent_is_validated_persisted_and_clearable` stay green.

- [ ] **Step 5: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 6: Commit.**

  ```bash
  git add src/error.rs src/hierarchy.rs src/repo.rs src/commands/mod.rs \
    src/commands/status.rs src/commands/check.rs tests/cli.rs
  git commit -m "feat(lanes): refuse nested lanes on every write path and in check (tasks-ece1e2)"
  ```

---

### Task 3.5: Pausing a lane

**Files**
- Modify `src/hierarchy.rs`: new `paused_lane` (after `lane_of`).
- Modify `src/query.rs`: `Picked` (lines 171–176), new `paused_omissions` (after
  `deferred_omission`), new test.
- Modify `src/commands/list.rs`: `ready_tasks` (after `sort_ready(&mut ready);`, and the
  return), new `warn_paused` (after `warn_hidden`), `ready` (after its
  `deferred_omission` block), `next` (after its `deferred_omission` block).
- Modify `src/commands/parked.rs`: `candidates` (lines 181–216).
- Modify `src/commands/edit.rs`: `run` (after line 123, before line 218), `editor` (before
  its `save`), new `pause_warning`.
- Modify `docs/specs/2026-09-03-task-hierarchy-design.md` (after line 135).
- Modify `tests/cli.rs`: two new tests.

**Interfaces**
- Produces: `pub fn paused_lane<'a>(all: &'a [Task], task: &Task, registry: &Registry) -> Option<&'a Task>`;
  `Picked.paused: Vec<Task>` (work a paused lane set aside after the picker's own
  dependency, defer, claim, and park checks);
  `pub fn paused_omissions(paused: &BTreeMap<TaskId, BTreeSet<TaskId>>) -> Vec<String>`;
  `fn warn_paused(ctx, all, paused: Vec<Task>, snapshots, without: &Without, cutoff, claims)`
  in `src/commands/list.rs`; warning text `<n> task(s) hidden by paused lane <id>`.
- Consumes: `enclosing_lane` (3.3); `retain_allowed`, `complexity::apply`; Slice 1's
  resolved `without` binding in `ready` and `next` and its `Without::retain` (Task 1.6).

The warning counts a paused-lane task only when every other gate would offer it: it has
passed the dependency, defer, claim, and user-park checks inside the picker, and the halt,
`--without`, and complexity-cutoff gates the caller applies to its own list. A deferred
task in a paused lane is in neither the deferred count nor the paused one.

- [ ] **Step 1: Write the failing tests.**

  `src/query.rs` tests:

  ```rust
      #[test]
      fn paused_omissions_name_each_lane_with_its_count() {
          let mut paused: std::collections::BTreeMap<TaskId, std::collections::BTreeSet<TaskId>> =
              std::collections::BTreeMap::new();
          paused
              .entry(TaskId::parse("sci-00000a").unwrap())
              .or_default()
              .extend([
                  TaskId::parse("sci-000001").unwrap(),
                  TaskId::parse("sci-000002").unwrap(),
              ]);
          assert_eq!(
              paused_omissions(&paused),
              ["2 task(s) hidden by paused lane sci-00000a"]
          );
          assert!(paused_omissions(&std::collections::BTreeMap::new()).is_empty());
      }
  ```

  `tests/cli.rs`:

  ```rust
  #[test]
  fn a_blocked_lane_pauses_its_subtree_in_the_pickers_until_unblocked() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "1"]));
      let step = id_of(env.json(&sci, &["add", "Capture", "--parent", &lane, "-p", "0"]));
      let resumed = id_of(env.json(&sci, &["add", "Resume", "--parent", &lane, "-p", "0"]));
      as_agent(&env, &sci, "agent-a")
          .args(["park", &resumed, "rerun"])
          .assert()
          .success();
      // Neither of these would be offered with the lane unpaused, so neither is counted.
      let later = id_of(env.json(
          &sci,
          &["add", "Later", "--parent", &lane, "-p", "0", "--defer", "2099-01-01"],
      ));
      let asks = id_of(env.json(&sci, &["add", "Ask", "--parent", &lane, "-p", "0"]));
      as_agent(&env, &sci, "agent-a")
          .args(["park", &asks, "which host?", "--waiting-on", "user"])
          .assert()
          .success();
      let goal = id_of(env.json(&sci, &["add", "Plain goal", "-p", "2"]));
      let under_goal = id_of(env.json(&sci, &["add", "Under goal", "--parent", &goal, "-p", "2"]));
      let loose = id_of(env.json(&sci, &["add", "Loose", "-p", "3"]));
      env.json(&sci, &["block", &goal, "an ordinary blocked goal"]);
      env.json(&sci, &["block", &lane, "waiting on the idle host"]);
      let ids = |value: &serde_json::Value| -> Vec<String> {
          value["tasks"]
              .as_array()
              .unwrap()
              .iter()
              .map(|row| row["id"].as_str().unwrap().to_string())
              .collect()
      };

      let ready = env.json(&sci, &["ready"]);
      assert_eq!(
          ids(&ready),
          [under_goal.clone(), loose.clone()],
          "a blocked goal does not block its children; a paused lane does"
      );
      assert!(
          ready["warnings"]
              .to_string()
              .contains(&format!("2 task(s) hidden by paused lane {lane}")),
          "Capture and Resume only; Later is deferred and Ask waits on the user: {ready}"
      );
      let warnings = ready["warnings"].to_string();
      assert!(
          !warnings.contains("deferred task") && !warnings.contains(&later),
          "a deferred task in a paused lane is in neither count: {ready}"
      );
      assert!(!warnings.contains(&asks), "{ready}");
      let cut = env.json(&sci, &["ready", "--max-complexity", "low"]);
      assert!(
          !cut["warnings"].to_string().contains("paused lane"),
          "unrated steps the cutoff hides are not counted as paused: {cut}"
      );
      let next = env.json(&sci, &["next"]);
      assert_eq!(next["next"]["task"]["id"], under_goal, "the parked step is paused too");
      assert!(
          next["warnings"]
              .to_string()
              .contains(&format!("2 task(s) hidden by paused lane {lane}")),
          "counted once across parked and ready: {next}"
      );
      let prime = env.json(&sci, &["prime"]);
      let primed: Vec<&str> = prime["ready"]
          .as_array()
          .unwrap()
          .iter()
          .map(|row| row["id"].as_str().unwrap())
          .collect();
      assert!(!primed.contains(&step.as_str()) && !primed.contains(&resumed.as_str()));

      // A person who names a task directly gets it.
      env.json(&sci, &["start", &step]);
      env.json(&sci, &["unblock", &lane]);
      let ready = env.json(&sci, &["ready"]);
      assert!(ids(&ready).contains(&resumed), "{ready}");
      assert!(!ready["warnings"].to_string().contains("paused lane"), "{ready}");
  }

  #[test]
  fn marking_a_blocked_goal_as_a_lane_warns_that_it_pauses_the_subtree() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let goal = id_of(env.json(&sci, &["add", "Goal"]));
      env.json(&sci, &["add", "Kid", "--parent", &goal]);
      env.json(&sci, &["block", &goal, "stuck"]);
      let out = env.json(&sci, &["edit", &goal, "--lane"]);
      assert!(
          out["warnings"]
              .to_string()
              .contains(&format!("{goal} is blocked, so marking it a lane pauses it")),
          "{out}"
      );
      let open = id_of(env.json(&sci, &["add", "Open goal"]));
      let out = env.json(&sci, &["edit", &open, "--lane"]);
      assert_eq!(out["warnings"], serde_json::json!([]), "{out}");
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one paused_omissions_name_each_lane` — fails to compile: `cannot find function
  'paused_omissions'`. `just test-one --test cli a_blocked_lane_pauses` — `ready` lists
  `Capture` and `Resume`. `just test-one --test cli marking_a_blocked_goal` — the warnings
  array is empty.

- [ ] **Step 3: Implement.**

  `src/hierarchy.rs`, after `lane_of`:

  ```rust
  /// Lanes design §3.4: the paused lane above `task`, if any. A `blocked` lane pauses its
  /// descendants; the one place an ancestor's status gates a descendant, and only for
  /// lanes. The task itself is never its own pause: a lane is a goal and never ready.
  pub fn paused_lane<'a>(all: &'a [Task], task: &Task, registry: &Registry) -> Option<&'a Task> {
      enclosing_lane(all, task, registry).filter(|lane| lane.status == Status::Blocked)
  }
  ```

  `src/query.rs`: imports become
  `use std::collections::{BTreeMap, BTreeSet, HashSet};`. Replace `Picked`:

  ```rust
  /// What a picker found: the tasks it may hand out, the ones a deferral held back, and the
  /// ones a paused lane kept out (lanes design §3.4). `paused` holds only work that passed
  /// the picker's own dependency, defer, claim, and park checks; the caller applies its
  /// session gates to it before counting.
  #[derive(Default)]
  pub struct Picked {
      pub tasks: Vec<Task>,
      pub deferred: Vec<Task>,
      pub paused: Vec<Task>,
  }
  ```

  After `deferred_omission`:

  ```rust
  /// One warning per paused lane that kept work out, in lane id order.
  pub fn paused_omissions(paused: &BTreeMap<TaskId, BTreeSet<TaskId>>) -> Vec<String> {
      paused
          .iter()
          .map(|(lane, hidden)| format!("{} task(s) hidden by paused lane {lane}", hidden.len()))
          .collect()
  }
  ```

  `src/commands/list.rs`: imports become `use std::collections::{BTreeMap, BTreeSet, HashMap};`.
  The 3.2 readiness loop in `ready_tasks` is unchanged: dependencies and deferral are
  judged there for every candidate, paused or not. In `ready_tasks`, directly after
  `sort_ready(&mut ready);` and before the `ready.retain(...)` that drops live claims:

  ```rust
      // Lanes design §3.4: a paused lane keeps its subtree out of every picker. Its work
      // leaves here, before the claim and park checks that warn about what they drop, and
      // passes those same checks silently, so the paused count holds only work that would
      // otherwise be offered. A deferred task in a paused lane is in neither count.
      let (mut paused, mut ready): (Vec<Task>, Vec<Task>) = ready
          .into_iter()
          .partition(|task| crate::hierarchy::paused_lane(all, task, &ctx.registry).is_some());
      paused.retain(|task| {
          claims.live(&task.id).is_none()
              && !claims
                  .park(&task.id)
                  .is_some_and(|park| park.waiting_on == WaitingOn::User)
      });
      deferred.retain(|task| crate::hierarchy::paused_lane(all, task, &ctx.registry).is_none());
  ```

  and return `Ok(Picked { tasks: ready, deferred, paused })`.

  After `warn_hidden` (and Slice 1's `vocabularies` and Slice 2's `retain_unheld` beside
  it), add the one place the paused warning is built:

  ```rust
  /// Lanes design §3.4: one `<n> task(s) hidden by paused lane <id>` warning per lane,
  /// counting only work every other gate would offer. The pickers already applied the
  /// dependency, defer, claim, and park checks to `paused`; this applies the halt,
  /// `--without`, and cutoff gates the caller applies to its own list, without their
  /// warnings, then counts each task once under its lane.
  fn warn_paused(
      ctx: &mut ReadCtx,
      all: &[Task],
      mut paused: Vec<Task>,
      snapshots: &HashMap<String, HaltSnapshot>,
      without: &Without,
      cutoff: Option<crate::model::Complexity>,
      claims: &crate::claims::ClaimSnapshot,
  ) {
      let _ = retain_allowed(&mut paused, snapshots);
      {
          // Each task is hidden only where its own project declares the need (Task 1.6).
          let vocabularies = vocabularies(&ctx.scope);
          let _ = without.retain(&mut paused, &vocabularies);
      }
      if let Some(cutoff) = cutoff {
          let _ = crate::complexity::apply(&mut paused, cutoff, claims);
      }
      let mut by_lane: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
      for task in &paused {
          if let Some(lane) = crate::hierarchy::paused_lane(all, task, &ctx.registry) {
              by_lane
                  .entry(lane.id.clone())
                  .or_default()
                  .insert(task.id.clone());
          }
      }
      ctx.warnings
          .extend(crate::query::paused_omissions(&by_lane));
  }
  ```

  `ready`, after the `if let Some(warning) = deferred_omission(&picked.deferred) { ... }`
  block (`without` is the `Without` Slice 1 binds at the top of `ready`):

  ```rust
     
      warn_paused(&mut ctx, &all, picked.paused, &snapshots, &without, cutoff, &claims);
  ```

  `next`, after the `if let Some(warning) = deferred_omission(&omitted) { ... }` block. A
  parked todo is both a candidate and a ready row; `warn_paused` counts ids in a set, so
  it is counted once:

  ```rust
      let mut paused = candidates.paused;
      paused.extend(ready.paused);
     
      warn_paused(&mut ctx, &all, paused, &snapshots, &without, cutoff, &claims);
  ```

  `src/commands/parked.rs`, `candidates`: before the loop add
  `let mut paused = Vec::new();`, and replace the `if !held { ... }` block at the end of
  the loop body with:

  ```rust
          if held {
              continue;
          }
          // Lanes design §3.4: a candidate in a paused lane is set aside once it has passed
          // the dependency check; a deferred one is in neither the deferred nor the paused
          // count.
          let in_paused_lane = crate::hierarchy::paused_lane(all, task, &ctx.registry).is_some();
          match (crate::defer::is_deferred(task, now), in_paused_lane) {
              (true, true) => {}
              (true, false) => deferred.push(task.clone()),
              (false, true) => paused.push(task.clone()),
              (false, false) => found.push((park.at.clone(), task.clone())),
          }
  ```

  and return `Picked { tasks: ..., deferred, paused }`.

  `src/commands/edit.rs`, new function after `refuse_shelving`:

  ```rust
  /// Lanes design §3.4: marking a goal that is already `blocked` as a lane pauses it, which
  /// hides its whole subtree from the pickers at once.
  fn pause_warning(task: &Task, was_lane: bool) -> Option<String> {
      (task.lane && !was_lane && task.status == Status::Blocked).then(|| {
          format!(
              "{id} is blocked, so marking it a lane pauses it: its subtree leaves ready, \
               next, and prime's ready list until `tasks unblock {id}`",
              id = task.id
          )
      })
  }
  ```

  In `run`, after `let mut task = load(&mut ctx, &id)?;` add `let was_lane = task.lane;`,
  and directly before `save(&mut ctx, &mut task)?;`:

  ```rust
      if let Some(warning) = pause_warning(&task, was_lane) {
          ctx.warnings.push(warning);
      }
  ```

  In `editor`, directly before `save(&mut ctx, &mut edited).map_err(keep)?;`:

  ```rust
      if let Some(warning) = pause_warning(&edited, original.lane) {
          ctx.warnings.push(warning);
      }
  ```

  `docs/specs/2026-09-03-task-hierarchy-design.md`, after the paragraph ending "a child's
  owner is its own." (line 135):

  ```markdown
  **Addendum (2026-10-03, lanes):** one exception. A `blocked` lane (`lane: true`) is
  *paused*: its descendants leave `ready`, `next`, and `prime`'s ready list, and `ready` and
  `next` warn `<n> task(s) hidden by paused lane <id>`. `start` on a task inside it still
  works. An ordinary blocked goal keeps the rule above. See
  `2026-10-03-lanes-needs-groups-design.md` §3.4.
  ```

- [ ] **Step 4: Run the tests and see them pass.**

  `just test-one paused_omissions_name_each_lane`,
  `just test-one --test cli a_blocked_lane_pauses`,
  `just test-one --test cli marking_a_blocked_goal`.

- [ ] **Step 5: Gate.** `cargo fmt`, `just test-fast` (all picker, park, halt tests
  unchanged), `tasks check`.

- [ ] **Step 6: Commit.**

  ```bash
  git add src/hierarchy.rs src/query.rs src/commands/list.rs src/commands/parked.rs \
    src/commands/edit.rs docs/specs/2026-09-03-task-hierarchy-design.md tests/cli.rs
  git commit -m "feat(lanes): pause a blocked lane's subtree in ready and next (tasks-ece1e2)"
  ```

---

### Task 3.6: `--under` on `list`, `ready`, and `next`

**Files**
- Modify `src/cli.rs`: `FilterArgs` (after line 110), `Command::Next` (after line 412).
- Modify `src/filter.rs`: `TaskFilter` (lines 17–29), `Fields` (lines 32–45), `parse`
  (lines 61–99), `is_empty` (lines 107–118), `matches` (lines 120–141), new
  `resolve_under`, `of_task` (line 145), `of_row` (line 164), test helper `fields`.
- Modify `src/commands/list.rs`: `list` (lines 40, 46), `list_parked` (lines 106–116),
  `ready` (lines 308–311), `next` (lines 343–357).
- Modify `src/commands/parked.rs`: `candidates` signature and loop.
- Modify `src/commands/mod.rs`: `Command::Next` dispatch (lines 1338–1341).
- Modify `tests/cli.rs`: new test.
- Adopt `tools/cli.toml` (and `tools/cli_surface.py` if refreshed) with `vendored adopt`
  at the Task 3.6 ops commit.

**Ops rows (landed by Task 0.1)** — the edit to ops `cli.toml` for this task only, in the
`tasks` CLI section:

- `[[cli.tasks.commands]] path = ["list"]` and `[[cli.tasks.commands]] path = ["ready"]`:
  in each, after `  { names = ["--parent"], value = "ref" },` insert
  ```toml
    { names = ["--under"], value = "ref" },
  ```
- `[[cli.tasks.commands]] path = ["next"]`: after Slice 1's
  `  { names = ["--without"], value = "string", repeatable = true },` (which follows
  `--max-complexity`) insert
  ```toml
    { names = ["--under"], value = "ref" },
  ```

**Interfaces**
- Produces: `FilterArgs.under: Option<String>`; `Command::Next { under: Option<String>, .. }`;
  `TaskFilter::resolve_under(&mut self, all: &[Task], registry: &Registry) -> Result<()>`;
  `Fields.id: TaskId` (canonical);
  `parked::candidates(ctx, all, claims, filter: &TaskFilter, now)`;
  `list::next(ctx, max_complexity, without: Vec<String>, under: Option<String>)` (Slice 1's
  `without` parameter as Task 1.6 lands it, then `under`).

- [ ] **Step 1: Write the failing test.**

  ```rust
  #[test]
  fn under_selects_descendants_at_any_depth_on_list_ready_and_next() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "2"]));
      let sub = id_of(env.json(&sci, &["add", "Sub", "--parent", &lane, "-p", "2"]));
      let deep = id_of(env.json(&sci, &["add", "Deep", "--parent", &sub, "-p", "3"]));
      let direct = id_of(env.json(&sci, &["add", "Direct", "--parent", &lane, "-p", "4"]));
      let urgent = id_of(env.json(&sci, &["add", "Urgent outside", "-p", "0"]));
      let ids = |value: &serde_json::Value| -> Vec<String> {
          value["tasks"]
              .as_array()
              .unwrap()
              .iter()
              .map(|row| row["id"].as_str().unwrap().to_string())
              .collect()
      };

      let mut listed = ids(&env.json(&sci, &["list", "--under", &lane]));
      listed.sort();
      let mut expected = vec![sub.clone(), deep.clone(), direct.clone()];
      expected.sort();
      assert_eq!(listed, expected, "any depth, never the root itself");
      assert_eq!(
          ids(&env.json(&sci, &["list", "--parent", &lane])).len(),
          2,
          "--parent keeps its direct-child meaning"
      );
      assert_eq!(
          ids(&env.json(&sci, &["ready", "--under", &lane])),
          [deep.clone(), direct.clone()]
      );
      assert_eq!(
          ids(&env.json(&sci, &["ready", "--under", &lane, "-p", "4"])),
          [direct.clone()],
          "--under narrows with the other filters"
      );

      assert_eq!(
          env.json(&sci, &["next"])["next"]["task"]["id"],
          urgent,
          "without --under, priority still wins"
      );
      assert_eq!(env.json(&sci, &["next", "--under", &lane])["next"]["task"]["id"], deep);
      as_agent(&env, &sci, "agent-a")
          .args(["park", &urgent, "resume it"])
          .assert()
          .success();
      assert_eq!(
          env.json(&sci, &["next", "--under", &lane])["next"]["task"]["id"],
          deep,
          "a parked candidate outside the subtree is not taken"
      );

      as_agent(&env, &sci, "agent-a")
          .args(["park", &deep, "ask", "--waiting-on", "user"])
          .assert()
          .success();
      assert_eq!(ids(&env.json(&sci, &["list", "--parked", "--under", &lane])), [deep.clone()]);

      assert_eq!(env.fail(&sci, &["next", "--under", "sci-ffffff"]), "task_not_found");
      assert_eq!(env.fail(&sci, &["list", "--under", "sci-ffffff"]), "task_not_found");
      assert_eq!(env.fail(&sci, &["ready", "--under", "sci-ffffff"]), "task_not_found");
  }
  ```

- [ ] **Step 2: Run it and see it fail.**

  `just test-one --test cli under_selects_descendants` — the first `list --under` exits 2
  (`unexpected argument '--under'`) and `env.json` panics.

- [ ] **Step 3: Implement.**

  `src/cli.rs`, `FilterArgs`, after the `parent` field:

  ```rust
      /// Only descendants of this task, at any depth (`--parent` is direct children only).
      #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
      pub under: Option<String>,
  ```

  `Command::Next`, after `max_complexity` (and Slice 1's `without`):

  ```rust
          /// Pick only among descendants of this task, at any depth: how a session
          /// committed to one lane takes its next step.
          #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
          under: Option<String>,
  ```

  `src/filter.rs`: add `use std::collections::HashSet;`. In `TaskFilter`, after
  `parallel: bool,`:

  ```rust
      under: Option<TaskId>,
      /// `under`'s descendants, filled from the scan by `resolve_under`.
      subtree: Option<HashSet<TaskId>>,
  ```

  In `Fields`, first field:

  ```rust
      /// Canonical, so `--under` matches a retired spelling too.
      pub id: TaskId,
  ```

  In `parse`, after `parallel: args.parallel,`:

  ```rust
              under: args
                  .under
                  .as_deref()
                  .map(|id| crate::commands::parse_id(registry, shorthand, id))
                  .transpose()?,
              subtree: None,
  ```

  `is_empty` gains `&& self.under.is_none()`. `matches` gains, after the `parallel` clause:

  ```rust
              && match (&self.under, &self.subtree) {
                  (None, _) => true,
                  (Some(_), Some(subtree)) => subtree.contains(&fields.id),
                  (Some(under), None) => {
                      unreachable!("--under {under} is resolved against the scan before matching")
                  }
              }
  ```

  New method after `statuses`:

  ```rust
      /// `--under` names a task in scope; its descendants at any depth, never itself, are
      /// what the filter keeps (lanes design §5.4). Runs once the scan exists, before any
      /// `matches`; a no-op without `--under`.
      pub fn resolve_under(&mut self, all: &[Task], registry: &Registry) -> Result<()> {
          let Some(under) = &self.under else {
              return Ok(());
          };
          if !all.iter().any(|task| task.id == *under) {
              return Err(Error::TaskNotFound(under.to_string()));
          }
          self.subtree = Some(
              crate::hierarchy::descendants(all, under, registry)
                  .into_iter()
                  .map(|task| registry.canonical_id(&task.id))
                  .collect(),
          );
          Ok(())
      }
  ```

  `of_task` gains `id: registry.canonical_id(&task.id),`; `of_row` gains
  `id: registry.canonical_id(&TaskId::parse(&row.id).ok()?),`. Test helper `fields()`
  gains `id: TaskId::parse("xx-000001").unwrap(),`.

  `src/commands/list.rs`:
  - `list`: `let mut filter = TaskFilter::parse(...)?;` and after
    `check_parent(&filter, &all, |_| false)?;` add
    `filter.resolve_under(&all, &ctx.registry)?;`.
  - `list_parked(mut ctx: ReadCtx, mut filter: TaskFilter)`: after its `check_parent(...)?;`
    add `filter.resolve_under(&all, &ctx.registry)?;`.
  - `ready`: `let mut filter = ...;` and after its `check_parent` add
    `filter.resolve_under(&all, &ctx.registry)?;`.
  - `next` gains a last parameter `under: Option<String>`; its opening becomes:

  ```rust
  pub fn next(
      mut ctx: ReadCtx,
      max_complexity: Option<String>,
      without: Vec<String>,
      under: Option<String>,
  ) -> Result<Output> {
      let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
      // Slice 1's line, unchanged by this task.
      let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
      let mut filter = TaskFilter::parse(
          &FilterArgs {
              under,
              ..FilterArgs::default()
          },
          &[],
          &ctx.registry,
          &ctx.shorthand,
      )?;
      let (all, claims) = ctx.scan_with_claims()?;
      let now = crate::time::parse(&crate::time::now())?;
      filter.resolve_under(&all, &ctx.registry)?;
      let _ = super::parked::rows(&mut ctx, &all, &claims, now)?;
      let candidates = super::parked::candidates(&mut ctx, &all, &claims, &filter, now)?;
      let snapshots = halt_snapshots(&mut ctx, &all);
      let ready = ready_tasks(&mut ctx, &all, &claims, &snapshots, &filter, now)?;
      // ... the rest of `next` unchanged
  ```

  `src/commands/parked.rs`, `candidates`: add `use crate::filter::{Fields, TaskFilter};`,
  insert `filter: &TaskFilter,` before `now`, and make the loop's first statement

  ```rust
          if !filter.matches(&Fields::of_task(task, claims, &ctx.registry)) {
              continue;
          }
  ```

  `src/commands/mod.rs`: the `Command::Next` arm destructures `under` and passes it last:

  ```rust
          Command::Next {
              max_complexity,
              without,
              under,
              scope,
          } => list::next(
              open_read_ctx(dir, &scope)?,
              max_complexity,
              without.without,
              under,
          ),
  ```

- [ ] **Step 4: Adopt this task's rows.** In `.worktrees/ece1e2-work-selection`:

  ```bash
  SHA=$(tasks show tasks-ece1e2 | grep -o 'inventory: Task 3\.6 = [0-9a-f]*' | awk '{print $NF}')
  test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
  ```

  It also refreshes `tools/cli_surface.py` when the pair changed. Run
  `just test-one parser_surface_equals_table`. Expected: PASS (the three `--under` rows
  match the parser).

- [ ] **Step 5: Run the test and see it pass.**

  `just test-one --test cli under_selects_descendants`; the filter unit tests
  (`just test-one filter::`) and `just test-one --test cli show_reports_parent_and_children`
  stay green.

- [ ] **Step 6: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 7: Commit.**

  ```bash
  git add src/cli.rs src/filter.rs src/commands/list.rs src/commands/parked.rs \
    src/commands/mod.rs tests/cli.rs tools/cli.toml
  # tools/cli_surface.py too, when the adopt changed it:
  git add $(git ls-files --modified --others --exclude-standard tools/cli_surface.py)
  git commit -m "feat(filter): select descendants with --under on list, ready, and next (tasks-ece1e2)"
  ```

---

### Task 3.7: The lanes builder and `tasks lanes`

**Files**
- Create `src/lanes.rs`.
- Modify `src/main.rs`: `mod lanes;` after `mod hierarchy;` (and Slice 2's `mod holds;`).
- Create `src/commands/lanes.rs`.
- Modify `src/commands/mod.rs`: `pub mod lanes;` (after `pub mod init;`), dispatch arm.
- Modify `src/commands/list.rs`: `fn halt_snapshots` becomes `pub(super) fn halt_snapshots`
  (line 236).
- Modify `src/cli.rs`: `Command::Lanes` (after `Prime`).
- Modify `src/output.rs`: `LanesOut`, `Output::Lanes`, pretty arm, `lane_lines`,
  `waiting_text`, `needs_theme`, `shows_priority`, `warnings_of`.
- Modify `tests/cli.rs`: helpers and five new tests; add `"lanes"` to
  `project_and_all_projects_conflict_on_every_read_command`.
- Adopt `tools/cli.toml` (and `tools/cli_surface.py` if refreshed) with `vendored adopt`
  at the Task 3.7 ops commit.

**Ops rows (landed by Task 0.1)** — the edit to ops `cli.toml` for this task only, in the
`tasks` CLI section. After the whole `[[cli.tasks.commands]] path = ["prime"]` row (the
summary is the `Lanes` doc comment without its final period, as the surface test reads it):

```toml

[[cli.tasks.commands]]
path = ["lanes"]
summary = "Each open lane: its guidance, its state, and the step it could take now"
options = [
  { names = ["--without"], value = "string", repeatable = true },
  { names = ["--max-complexity"], value = "enum", values = ["low", "mid", "high"] },
  { shared = "project", role = "select", value = "string", repeatable = false },
  { shared = "all_projects", value = "none" },
]
```

The `--without` row is spelled as Slice 1 spells it on `ready`, `next`, and `prime`
(Task 1.6); if Slice 1 lands a different row there, this one matches it.

**Interfaces**
- Consumes: `held_back(&HoldSnapshot, &Vocabulary, &Task, Option<&str>) -> Option<(String, Holder)>`,
  `HoldSnapshot::load(&Registry, OffsetDateTime) -> (HoldSnapshot, Vec<String>)`,
  `exclusive_of(&Vocabulary, &[String]) -> Vec<String>`, `Without::hides(&Task) -> bool`,
  `Without::resolve(&[String], Option<&str>, &[&Vocabulary]) -> Result<Without>` (unit
  tests), `Without::from_env(&[String], &[&Vocabulary]) -> Result<Without>` with
  `commands::list::vocabularies(&ReadCtx)` (the command), all as Slice 1 Task 1.6 lands
  them; `WithoutArgs { without: Vec<String> }`, `Project.needs`, `is_goal`, and the
  existing readiness predicates (`query::is_candidate`, `defer::is_deferred`,
  `periodic::is_due`, `complexity::effective`, `HaltSnapshot::allows`).
- Produces (`src/lanes.rs`):
  `pub enum LaneState { Paused, Ready, Held, Waiting, Empty }`,
  `pub enum HeldBy { Claim, Pick }`,
  `pub struct HeldStep { pub id: TaskId, pub need: String, pub holder: TaskId, pub by: HeldBy }`,
  `pub struct Causes { pub active, held, without, cutoff, halt, deferred, periodic, user, blocked, depends, goal, other: usize }`
  with `pub fn entries(&self) -> Vec<(&'static str, usize)>`, `pub fn is_empty(&self) -> bool`,
  `pub struct LaneRow { pub lane: TaskSummary, pub guidance: Option<String>, pub state: LaneState, pub pick: Option<TaskSummary>, pub steps: usize, pub active: Vec<TaskSummary>, pub held: Vec<HeldStep>, pub causes: Causes }`,
  `pub struct Inputs<'a> { .. }`, `pub fn build(inputs: &Inputs) -> Vec<LaneRow>`,
  `pub fn guidance(body: &str) -> Option<String>`,
  `pub fn dependency_readers<'a>(all: &'a [Task], claims: &ClaimSnapshot, registry: &Registry, now: OffsetDateTime) -> Vec<&'a Task>`
  (the only records whose dependencies the view reads: live descendants of open, unpaused
  lanes that could be a step).
  (`src/commands/lanes.rs`): `pub fn run(ctx: ReadCtx, without: WithoutArgs, max_complexity: Option<String>) -> Result<Output>`,
  `pub(super) fn rows(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot, halts: &HashMap<String, HaltSnapshot>, cutoff: Option<Complexity>, without: &Without, now: OffsetDateTime) -> Result<Vec<LaneRow>>`.
  CLI: `tasks lanes [--without <n>]... [--max-complexity <c>] [--project P | --all-projects]`.
  JSON: `{lanes: [LaneRow], warnings}`.

- [ ] **Step 1: Write the failing unit tests.** Create `src/lanes.rs` holding only its test
  module for now, and register it with `mod lanes;` in `src/main.rs`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::claims::{ClaimSnapshot, Park, WaitingOn};
      use crate::holds::HoldSnapshot;
      use crate::model::{Status, Task, TaskId};
      use crate::needs::{NeedDecl, Vocabulary, Without};
      use crate::registry::Registry;
      use std::collections::{BTreeMap, HashMap};
      use time::OffsetDateTime;

      const NOW: &str = "2026-10-03T12:00:00Z";

      fn at(stamp: &str) -> OffsetDateTime {
          crate::time::parse(stamp).unwrap()
      }

      fn task(id: &str, parent: Option<&str>, status: Status, priority: u8) -> Task {
          Task {
              id: TaskId::parse(id).unwrap(),
              title: id.into(),
              status,
              priority,
              size: None,
              complexity: None,
              process: None,
              parallel: false,
              lane: false,
              needs: vec![],
              every: None,
              defer: None,
              owner: None,
              created: "2026-09-01T00:00:00Z".into(),
              updated: "2026-09-01T00:00:00Z".into(),
              started: None,
              completed: None,
              last_done: None,
              depends: vec![],
              parent: parent.map(|parent| TaskId::parse(parent).unwrap()),
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

      fn lane(id: &str, priority: u8) -> Task {
          let mut lane = task(id, None, Status::Todo, priority);
          lane.lane = true;
          lane
      }

      fn needs_quiet(mut task: Task) -> Task {
          task.needs = vec!["quiet".into()];
          task
      }

      fn agent_park() -> Park {
          Park {
              owner: "o".into(),
              session: "s:a".into(),
              host: "h".into(),
              worktree: "/w".into(),
              at: "2026-10-01T00:00:00Z".into(),
              next_step: "n".into(),
              waiting_on: WaitingOn::Agent,
              reason: None,
              needs: None,
              minutes: None,
              title: "T".into(),
          }
      }

      struct World {
          claims: ClaimSnapshot,
          vocabulary: Vocabulary,
          without: Without,
          holds: HoldSnapshot,
          registry: Registry,
      }

      fn world(claims: ClaimSnapshot) -> World {
          let registry = Registry::default();
          let (holds, _) = HoldSnapshot::load(&registry, at(NOW));
          World {
              claims,
              vocabulary: Vocabulary::from([(
                  "quiet".to_string(),
                  NeedDecl {
                      meaning: "an idle host".into(),
                      exclusive: true,
                  },
              )]),
              without: Without::default(),
              holds,
              registry,
          }
      }

      fn run(all: &[Task], world: &World) -> Vec<LaneRow> {
          let dependency = |id: &TaskId| {
              all.iter()
                  .find(|task| task.id == *id)
                  .map(|task| !task.status.is_open())
          };
          let halts = HashMap::new();
          let vocabularies = HashMap::from([("xx", &world.vocabulary)]);
          // The fixture's own session is "me": its claims are this caller's holds.
          let mine = world.holds.mine(|claim| Ok(claim.session == "me")).unwrap();
          build(&Inputs {
              all,
              claims: &world.claims,
              registry: &world.registry,
              dependency: &dependency,
              halts: &halts,
              cutoff: None,
              without: &world.without,
              holds: &world.holds,
              vocabularies: &vocabularies,
              mine: &mine,
              now: at(NOW),
          })
      }

      fn pick(row: &LaneRow) -> &str {
          row.pick.as_ref().map(|pick| pick.id.as_str()).unwrap_or("-")
      }

      #[test]
      fn guidance_is_the_first_paragraph_after_blank_and_heading_lines() {
          assert_eq!(
              guidance("\n# Captures\n\n## Why\n\nShip the captures.\nFirst milestone: one run.\n\nMore.")
                  .as_deref(),
              Some("Ship the captures. First milestone: one run.")
          );
          assert_eq!(guidance("# Only a heading"), None);
          assert_eq!(guidance("## Heading\n\n### Another"), None);
          assert_eq!(guidance(""), None);
          assert_eq!(
              guidance("#hashtag opens the paragraph").as_deref(),
              Some("#hashtag opens the paragraph"),
              "a heading needs a space after its hashes"
          );
      }

      #[test]
      fn lanes_come_in_ready_order_and_a_blocked_one_is_paused() {
          let mut paused = lane("xx-0000a1", 0);
          paused.status = Status::Blocked;
          let mut shelved = lane("xx-0000a3", 0);
          shelved.status = Status::Shelved;
          let mut closed = lane("xx-0000a4", 0);
          closed.status = Status::Done;
          let all = vec![
              paused,
              task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
              lane("xx-0000a2", 2),
              shelved,
              closed,
          ];
          let rows = run(&all, &world(ClaimSnapshot::default()));
          let ids: Vec<&str> = rows.iter().map(|row| row.lane.id.as_str()).collect();
          assert_eq!(ids, ["xx-0000a1", "xx-0000a2"], "shelved and closed lanes are absent");
          assert_eq!(rows[0].state, LaneState::Paused);
          assert!(rows[0].pick.is_none() && rows[0].steps == 0 && rows[0].causes.is_empty());
          assert_eq!(rows[1].state, LaneState::Empty);
      }

      #[test]
      fn a_parked_step_is_picked_before_ready_order_as_next_does() {
          let all = vec![
              lane("xx-0000a1", 0),
              task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
              task("xx-000002", Some("xx-0000a1"), Status::Todo, 3),
          ];
          let claims = ClaimSnapshot::from_parts(
              BTreeMap::new(),
              BTreeMap::from([("xx-000002".to_string(), agent_park())]),
              BTreeMap::new(),
          );
          let rows = run(&all, &world(claims));
          assert_eq!(pick(&rows[0]), "xx-000002");
          assert_eq!(rows[0].steps, 1);
      }

      #[test]
      fn a_sub_goal_counts_as_goal_and_its_children_are_steps() {
          let all = vec![
              lane("xx-0000a1", 0),
              task("xx-0000b1", Some("xx-0000a1"), Status::Todo, 0),
              task("xx-000001", Some("xx-0000b1"), Status::Todo, 2),
          ];
          let rows = run(&all, &world(ClaimSnapshot::default()));
          assert_eq!(rows[0].state, LaneState::Ready);
          assert_eq!(pick(&rows[0]), "xx-000001", "picked from inside the sub-goal");
          assert_eq!(rows[0].causes.entries(), vec![("goal", 1)]);
      }

      #[test]
      fn the_partition_counts_every_live_descendant_once() {
          let mut deferred_and_blocked = task("xx-000006", Some("xx-0000a1"), Status::Blocked, 2);
          deferred_and_blocked.defer = Some(crate::defer::Defer::parse("2099-01-01").unwrap());
          let mut depends = task("xx-000007", Some("xx-0000a1"), Status::Todo, 2);
          depends.depends = vec![TaskId::parse("xx-000005").unwrap()];
          let all = vec![
              lane("xx-0000a1", 1),
              task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
              task("xx-000002", Some("xx-0000a1"), Status::Todo, 1),
              task("xx-000003", Some("xx-0000a1"), Status::Idea, 2),
              task("xx-000004", Some("xx-0000a1"), Status::Doing, 2),
              task("xx-000005", Some("xx-0000a1"), Status::Blocked, 2),
              deferred_and_blocked,
              depends,
              task("xx-000008", Some("xx-0000a1"), Status::Done, 2),
              task("xx-000009", Some("xx-0000a1"), Status::Dropped, 2),
              task("xx-00000a", Some("xx-0000a1"), Status::Shelved, 2),
          ];
          let rows = run(&all, &world(ClaimSnapshot::default()));
          let row = &rows[0];
          assert_eq!(pick(row), "xx-000001");
          assert_eq!(row.steps, 1);
          assert_eq!(
              row.causes.entries(),
              vec![("deferred", 1), ("blocked", 1), ("depends", 1), ("other", 2)],
              "deferred and blocked counts once, under the earlier cause"
          );
          let counted: usize =
              1 + row.steps + row.causes.entries().iter().map(|(_, n)| n).sum::<usize>();
          assert_eq!(counted, 7, "seven live descendants, each once");
      }

      #[test]
      fn recurrences_count_while_live_and_a_dropped_one_does_not() {
          let every = crate::periodic::Interval::parse("30d").unwrap();
          let mut due = task("xx-000001", Some("xx-0000a1"), Status::Done, 2);
          due.every = Some(every);
          let mut soon = task("xx-000002", Some("xx-0000a2"), Status::Done, 2);
          soon.every = Some(every);
          soon.last_done = Some("2026-10-02T00:00:00Z".into());
          let mut gone = task("xx-000003", Some("xx-0000a3"), Status::Dropped, 2);
          gone.every = Some(every);
          let all = vec![
              lane("xx-0000a1", 0),
              due,
              lane("xx-0000a2", 1),
              soon,
              lane("xx-0000a3", 2),
              gone,
          ];
          let rows = run(&all, &world(ClaimSnapshot::default()));
          assert_eq!(rows[0].state, LaneState::Ready);
          assert_eq!(pick(&rows[0]), "xx-000001");
          assert_eq!(rows[1].state, LaneState::Waiting);
          assert_eq!(rows[1].causes.entries(), vec![("periodic", 1)]);
          assert_eq!(rows[2].state, LaneState::Empty);
      }

      #[test]
      fn an_earlier_lane_pick_holds_its_exclusive_need_for_later_lanes() {
          let mut all = vec![
              lane("xx-0000a1", 0),
              needs_quiet(task("xx-000001", Some("xx-0000a1"), Status::Todo, 2)),
              lane("xx-0000a2", 1),
              needs_quiet(task("xx-000002", Some("xx-0000a2"), Status::Todo, 0)),
              task("xx-000003", Some("xx-0000a2"), Status::Todo, 1),
          ];
          let world = world(ClaimSnapshot::default());
          let rows = run(&all, &world);
          assert_eq!(pick(&rows[0]), "xx-000001");
          assert_eq!(pick(&rows[1]), "xx-000003");
          let held = &rows[1].held[0];
          assert_eq!(
              (held.id.to_string(), held.need.as_str(), held.holder.to_string(), held.by),
              ("xx-000002".to_string(), "quiet", "xx-000001".to_string(), HeldBy::Pick)
          );
          assert_eq!(rows[1].causes.entries(), vec![("held", 1)]);
          assert_eq!(rows[1].steps, 0, "a held step counts under held, not steps");

          all.pop();
          let rows = run(&all, &world);
          assert_eq!(rows[1].state, LaneState::Held);
          assert!(rows[1].pick.is_none());
      }

      #[test]
      fn dependencies_are_read_only_for_work_that_could_be_a_step() {
          let mut paused = lane("xx-0000a2", 1);
          paused.status = Status::Blocked;
          let mut due = task("xx-000005", Some("xx-0000a1"), Status::Done, 2);
          due.every = Some(crate::periodic::Interval::parse("30d").unwrap());
          let all = vec![
              lane("xx-0000a1", 0),
              task("xx-000001", Some("xx-0000a1"), Status::Todo, 2),
              task("xx-000002", Some("xx-0000a1"), Status::Done, 2),
              task("xx-000003", Some("xx-0000a1"), Status::Shelved, 2),
              task("xx-000004", Some("xx-0000a1"), Status::Idea, 2),
              due,
              task("xx-000006", Some("xx-0000a1"), Status::Idea, 2),
              paused,
              task("xx-000007", Some("xx-0000a2"), Status::Todo, 2),
              task("xx-000008", None, Status::Todo, 2),
          ];
          let claims = ClaimSnapshot::from_parts(
              BTreeMap::new(),
              BTreeMap::from([("xx-000006".to_string(), agent_park())]),
              BTreeMap::new(),
          );
          let mut readers: Vec<String> =
              dependency_readers(&all, &claims, &Registry::default(), at(NOW))
                  .iter()
                  .map(|task| task.id.to_string())
                  .collect();
          readers.sort();
          assert_eq!(
              readers,
              ["xx-000001", "xx-000005", "xx-000006"],
              "a todo, a due recurrence, and an agent-parked idea; never closed, shelved, \
               an unparked idea, a paused lane's work, or work outside every lane"
          );
      }
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one lanes::tests` — fails to compile: `cannot find function 'build'`,
  `cannot find struct 'Inputs'`, `cannot find type 'LaneRow'`, `cannot find function
  'guidance'`, `cannot find function 'dependency_readers'`.

- [ ] **Step 3: Implement the builder.** Put this above the test module in `src/lanes.rs`:

  ```rust
  //! The lanes view (docs/specs/2026-10-03-lanes-needs-groups-design.md §5): one row per
  //! open, unshelved lane, with its guidance, the step it could take now, and why the rest
  //! of its live work cannot. `tasks lanes` and `prime` share it; `commands::lanes` gathers
  //! what it reads.

  use crate::claims::{ClaimSnapshot, WaitingOn};
  use crate::halt::HaltSnapshot;
  use crate::holds::{HoldSnapshot, Mine};
  use crate::model::{Complexity, Status, Task, TaskId};
  use crate::needs::{Vocabulary, Without};
  use crate::output::TaskSummary;
  use crate::registry::Registry;
  use serde::Serialize;
  use std::collections::{BTreeMap, HashMap};
  use time::OffsetDateTime;

  /// §5.2.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
  #[serde(rename_all = "lowercase")]
  pub enum LaneState {
      /// The lane is `blocked`.
      Paused,
      /// A pick exists.
      Ready,
      /// Steps exist, but every one waits for an exclusive need.
      Held,
      /// No step exists, and live descendants remain; `causes` says why.
      Waiting,
      /// No live descendant.
      Empty,
  }

  /// Who holds the need a skipped step waits for.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
  #[serde(rename_all = "lowercase")]
  pub enum HeldBy {
      /// A live claim of another session.
      Claim,
      /// The pick of an earlier lane in this view.
      Pick,
  }

  /// A step skipped because an exclusive need it uses is held.
  #[derive(Debug, Clone, Serialize)]
  pub struct HeldStep {
      pub id: TaskId,
      pub need: String,
      /// The claimed task, or the earlier lane's pick.
      pub holder: TaskId,
      pub by: HeldBy,
  }

  /// Every live descendant that is neither the pick nor an unpicked step, under its first
  /// cause in §5.1's table order. Serialized sparse, in that order.
  #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
  pub struct Causes {
      #[serde(skip_serializing_if = "is_zero")]
      pub active: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub held: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub without: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub cutoff: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub halt: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub deferred: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub periodic: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub user: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub blocked: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub depends: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub goal: usize,
      #[serde(skip_serializing_if = "is_zero")]
      pub other: usize,
  }

  fn is_zero(count: &usize) -> bool {
      *count == 0
  }

  impl Causes {
      /// The non-zero counts, in table order.
      pub fn entries(&self) -> Vec<(&'static str, usize)> {
          [
              ("active", self.active),
              ("held", self.held),
              ("without", self.without),
              ("cutoff", self.cutoff),
              ("halt", self.halt),
              ("deferred", self.deferred),
              ("periodic", self.periodic),
              ("user", self.user),
              ("blocked", self.blocked),
              ("depends", self.depends),
              ("goal", self.goal),
              ("other", self.other),
          ]
          .into_iter()
          .filter(|(_, count)| *count > 0)
          .collect()
      }

      pub fn is_empty(&self) -> bool {
          self.entries().is_empty()
      }

      fn add(&mut self, cause: Cause) {
          let slot = match cause {
              Cause::Active => &mut self.active,
              Cause::Without => &mut self.without,
              Cause::Cutoff => &mut self.cutoff,
              Cause::Halt => &mut self.halt,
              Cause::Deferred => &mut self.deferred,
              Cause::Periodic => &mut self.periodic,
              Cause::User => &mut self.user,
              Cause::Blocked => &mut self.blocked,
              Cause::Depends => &mut self.depends,
              Cause::Goal => &mut self.goal,
              Cause::Other => &mut self.other,
          };
          *slot += 1;
      }
  }

  /// The table's causes other than `held`, which only the pick loop can assign.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum Cause {
      Active,
      Without,
      Cutoff,
      Halt,
      Deferred,
      Periodic,
      User,
      Blocked,
      Depends,
      Goal,
      Other,
  }

  /// §5.3's `LaneRow`.
  #[derive(Clone, Serialize)]
  pub struct LaneRow {
      pub lane: TaskSummary,
      pub guidance: Option<String>,
      pub state: LaneState,
      pub pick: Option<TaskSummary>,
      /// Unpicked steps that were not skipped for a held need.
      pub steps: usize,
      /// Descendants with a live claim, reported in every state.
      pub active: Vec<TaskSummary>,
      #[serde(skip_serializing_if = "Vec::is_empty")]
      pub held: Vec<HeldStep>,
      #[serde(skip_serializing_if = "Causes::is_empty")]
      pub causes: Causes,
  }

  /// Everything the builder reads, gathered once by the command.
  pub struct Inputs<'a> {
      /// The scope's scan.
      pub all: &'a [Task],
      pub claims: &'a ClaimSnapshot,
      pub registry: &'a Registry,
      /// `Some(closed?)` for a reachable dependency, `None` for an unreachable one. Only
      /// the dependencies of `dependency_readers` are ever asked for.
      pub dependency: &'a dyn Fn(&TaskId) -> Option<bool>,
      /// Per prefix; a prefix without one gates nothing.
      pub halts: &'a HashMap<String, HaltSnapshot>,
      pub cutoff: Option<Complexity>,
      pub without: &'a Without,
      pub holds: &'a HoldSnapshot,
      /// Each scanned project's need vocabulary, by prefix.
      pub vocabularies: &'a HashMap<&'a str, &'a Vocabulary>,
      /// The caller's own live holds (`commands::own_holds`, Slice 2), for the hold gate;
      /// `Mine::default()` counts every hold as another session's.
      pub mine: &'a Mine,
      pub now: OffsetDateTime,
  }

  /// §5.1: every open, unshelved lane in scope, in ready order (§3.7). Earlier lanes win
  /// contested exclusive needs, because lane order is the person's ranking.
  pub fn build(inputs: &Inputs) -> Vec<LaneRow> {
      let mut lanes: Vec<&Task> = inputs
          .all
          .iter()
          .filter(|task| task.lane && crate::hierarchy::is_active(task))
          .collect();
      lanes.sort_by(|a, b| crate::query::ready_order(a, b));
      let mut picked: BTreeMap<String, TaskId> = BTreeMap::new();
      lanes
          .into_iter()
          .map(|lane| row(inputs, lane, &mut picked))
          .collect()
  }

  /// §3.5: the first paragraph of the body, after leading blank and heading lines, up to
  /// the next blank line. Its lines join with a space, so the guidance is one line.
  pub fn guidance(body: &str) -> Option<String> {
      let paragraph: Vec<&str> = body
          .lines()
          .skip_while(|line| line.trim().is_empty() || is_heading(line))
          .take_while(|line| !line.trim().is_empty())
          .map(str::trim)
          .collect();
      (!paragraph.is_empty()).then(|| paragraph.join(" "))
  }

  /// An ATX heading: one to six `#`, then a space or the end of the line.
  fn is_heading(line: &str) -> bool {
      let line = line.trim_start();
      let hashes = line.chars().take_while(|c| *c == '#').count();
      (1..=6).contains(&hashes) && line[hashes..].chars().next().is_none_or(char::is_whitespace)
  }

  /// §5.1 step 4: open and unshelved, or a `done` recurrence between occurrences, which
  /// `ready` offers again once due. A dropped record keeps its cadence but never returns.
  fn is_live(task: &Task) -> bool {
      crate::hierarchy::is_active(task) || (task.status == Status::Done && task.every.is_some())
  }

  /// Work `next` could hand out once its other gates pass: a `ready_tasks` candidate (todo,
  /// or a due recurrence) or an open, unblocked, unshelved parked-agent candidate. These
  /// are the records `ready_tasks` and `parked::candidates` read dependencies for, and the
  /// only ones the view reads them for.
  fn could_step(task: &Task, claims: &ClaimSnapshot, now: OffsetDateTime) -> bool {
      crate::query::is_candidate(task, now)
          || claims.park(&task.id).is_some_and(|park| {
              park.waiting_on == WaitingOn::Agent
                  && task.status.is_open()
                  && !matches!(task.status, Status::Blocked | Status::Shelved)
          })
  }

  /// The records whose dependencies the view resolves: live descendants of open, unpaused
  /// lanes that `could_step`. Closed and shelved descendants, an unparked `idea`, and all
  /// work under a paused lane are never read, so a dependency only such work names (a
  /// foreign record since garbled or gone) cannot fail `tasks lanes` or `prime`.
  pub fn dependency_readers<'a>(
      all: &'a [Task],
      claims: &ClaimSnapshot,
      registry: &Registry,
      now: OffsetDateTime,
  ) -> Vec<&'a Task> {
      all.iter()
          .filter(|lane| {
              lane.lane && crate::hierarchy::is_active(lane) && lane.status != Status::Blocked
          })
          .flat_map(|lane| crate::hierarchy::descendants(all, &lane.id, registry))
          .filter(|task| is_live(task) && could_step(task, claims, now))
          .collect()
  }

  /// Where one live descendant lands: a cause, or a step `next` would consider.
  enum Class<'a> {
      Cause(Cause),
      /// A parked-agent candidate, with its park time.
      Parked(&'a str),
      /// A `ready` row.
      Ready,
  }

  /// The gates `next` applies before holds, in §5.1's table order, so a descendant that
  /// several would stop counts under the first.
  fn classify<'a>(inputs: &Inputs<'a>, task: &Task) -> Class<'a> {
      let claims = inputs.claims;
      let park = claims.park(&task.id);
      let cause = if claims.live(&task.id).is_some() {
          Some(Cause::Active)
      } else if inputs
          .without
          .hides(task, crate::needs::vocabulary_of(inputs.vocabularies, task))
      {
          Some(Cause::Without)
      } else if inputs.cutoff.is_some_and(|cutoff| {
          crate::complexity::effective(task, claims).is_none_or(|level| level > cutoff)
      }) {
          Some(Cause::Cutoff)
      } else if inputs
          .halts
          .get(&task.id.prefix)
          .is_some_and(|halt| !halt.allows(task))
      {
          Some(Cause::Halt)
      } else if crate::defer::is_deferred(task, inputs.now) {
          Some(Cause::Deferred)
      } else if task.status == Status::Done && !crate::periodic::is_due(task, inputs.now) {
          Some(Cause::Periodic)
      } else if park.is_some_and(|park| park.waiting_on == WaitingOn::User) {
          Some(Cause::User)
      } else if task.status == Status::Blocked {
          Some(Cause::Blocked)
      } else if could_step(task, claims, inputs.now)
          && !task
              .depends
              .iter()
              .all(|dependency| (inputs.dependency)(dependency) == Some(true))
      {
          // Asked only of `dependency_readers` records, the ones whose dependencies were
          // resolved; a record that could never be a step falls through to `goal` or
          // `other`.
          Some(Cause::Depends)
      } else if crate::hierarchy::is_goal(
          task,
          !crate::hierarchy::children(inputs.all, &task.id, inputs.registry).is_empty(),
      ) {
          Some(Cause::Goal)
      } else {
          None
      };
      match (cause, park) {
          (Some(cause), _) => Class::Cause(cause),
          // `parked::candidates`: waiting on the agent and open; blocked and shelved are
          // already out above.
          (None, Some(park)) if park.waiting_on == WaitingOn::Agent && task.status.is_open() => {
              Class::Parked(park.at.as_str())
          }
          (None, _) if crate::query::is_candidate(task, inputs.now) => Class::Ready,
          // `idea`, or `doing` without a live claim.
          (None, _) => Class::Cause(Cause::Other),
      }
  }

  fn row(inputs: &Inputs, lane: &Task, picked: &mut BTreeMap<String, TaskId>) -> LaneRow {
      let summary = |task: &Task| {
          TaskSummary::of(task, inputs.all, Some(inputs.claims), inputs.registry, inputs.now)
      };
      let mut live: Vec<&Task> = crate::hierarchy::descendants(inputs.all, &lane.id, inputs.registry)
          .into_iter()
          .filter(|task| is_live(task))
          .collect();
      live.sort_by(|a, b| crate::query::ready_order(a, b));
      let mut out = LaneRow {
          lane: summary(lane),
          guidance: guidance(&lane.body),
          state: LaneState::Paused,
          pick: None,
          steps: 0,
          active: live
              .iter()
              .filter(|task| inputs.claims.live(&task.id).is_some())
              .map(|task| summary(task))
              .collect(),
          held: Vec::new(),
          causes: Causes::default(),
      };
      // §5.1: a paused lane lists its active work and nothing else.
      if lane.status == Status::Blocked {
          return out;
      }
      let mut parked: Vec<(&str, &Task)> = Vec::new();
      let mut ready: Vec<&Task> = Vec::new();
      for &task in &live {
          match classify(inputs, task) {
              Class::Cause(cause) => out.causes.add(cause),
              Class::Parked(at) => parked.push((at, task)),
              Class::Ready => ready.push(task),
          }
      }
      // `next`'s order: parked candidates newest first, then ready order (`live` is sorted).
      parked.sort_by(|a, b| b.0.cmp(a.0).then_with(|| a.1.id.cmp(&b.1.id)));
      let mut pick: Option<&Task> = None;
      for step in parked.into_iter().map(|(_, task)| task).chain(ready) {
          if pick.is_some() {
              out.steps += 1;
              continue;
          }
          match held_by(inputs, step, picked) {
              Some(held) => {
                  out.causes.held += 1;
                  out.held.push(held);
              }
              None => pick = Some(step),
          }
      }
      if let Some(step) = pick
          && let Some(vocabulary) = inputs.vocabularies.get(step.id.prefix.as_str())
      {
          for need in crate::needs::exclusive_of(vocabulary, &step.needs) {
              picked.entry(need).or_insert_with(|| step.id.clone());
          }
      }
      out.state = match (pick, out.held.is_empty(), live.is_empty()) {
          (Some(_), _, _) => LaneState::Ready,
          (None, false, _) => LaneState::Held,
          (None, true, false) => LaneState::Waiting,
          (None, true, true) => LaneState::Empty,
      };
      out.pick = pick.map(&summary);
      out
  }

  /// §5.1 step 3: the first exclusive need of `step` held by another session's live claim,
  /// else by an earlier lane's pick. Exclusive names are matched host-wide (§4.1).
  fn held_by(inputs: &Inputs, step: &Task, picked: &BTreeMap<String, TaskId>) -> Option<HeldStep> {
      let vocabulary = inputs.vocabularies.get(step.id.prefix.as_str())?;
      if let Some((need, holder)) =
          crate::holds::held_back(inputs.holds, vocabulary, step, inputs.mine)
      {
          return Some(HeldStep {
              id: step.id.clone(),
              need,
              holder: holder.task,
              by: HeldBy::Claim,
          });
      }
      crate::needs::exclusive_of(vocabulary, &step.needs)
          .into_iter()
          .find_map(|need| {
              picked.get(&need).map(|pick| HeldStep {
                  id: step.id.clone(),
                  holder: pick.clone(),
                  need,
                  by: HeldBy::Pick,
              })
          })
  }
  ```

- [ ] **Step 4: Run the unit tests and see them pass.** `just test-one lanes::tests`.
  (`build` has no non-test caller yet; Step 5 adds it before anything is committed.)

- [ ] **Step 5: Write the failing end-to-end tests.** These rely on `TestEnv` clearing
  `TASKS_WITHOUT` (Slice 1); if `tests/common/mod.rs` does not yet, add
  `.env_remove("TASKS_WITHOUT")` to both `cmd` and `raw` beside `TASKS_MAX_COMPLEXITY`, so
  a host that sets it cannot change these results. If Slices 1–2 already added helpers
  equivalent to `declare_lane_need` or `json_as_session`, use those and skip these. In
  `tests/cli.rs`, add `"lanes"` to the command list of
  `project_and_all_projects_conflict_on_every_read_command`, then add:

  ```rust
  /// Declares `name` in the project's need vocabulary (the `[needs]` format of lanes design
  /// §4.1).
  fn declare_lane_need(dir: &std::path::Path, name: &str, exclusive: bool) {
      let path = dir.join("tasks/.config.toml");
      let mut text = std::fs::read_to_string(&path).unwrap();
      text.push_str(&format!(
          "\n[needs.{name}]\nmeaning = \"{name}, for the lanes tests\"\nexclusive = {exclusive}\n"
      ));
      std::fs::write(&path, text).unwrap();
  }

  /// `tasks <args>` run as the named live session, parsed.
  fn json_as_session(
      env: &TestEnv,
      dir: &std::path::Path,
      session: &str,
      args: &[&str],
  ) -> serde_json::Value {
      let out = as_agent(env, dir, session).args(args).output().unwrap();
      assert!(
          out.status.success(),
          "tasks {args:?} as {session} failed: {}",
          String::from_utf8_lossy(&out.stderr)
      );
      serde_json::from_slice(&out.stdout).unwrap()
  }

  #[test]
  fn lanes_view_reports_every_state_and_picks_as_next_does() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      declare_lane_need(&sci, "quiet", true);
      let ready_lane = id_of(env.json(
          &sci,
          &[
              "add", "Ready lane", "--lane", "-p", "0", "-b",
              "# Ready lane\n\nShip the captures.\nFirst milestone: one clean run.\n\nDetail.",
          ],
      ));
      env.json(&sci, &["add", "Urgent step", "--parent", &ready_lane, "-p", "0"]);
      let resumed = id_of(env.json(&sci, &["add", "Resumed step", "--parent", &ready_lane, "-p", "3"]));
      as_agent(&env, &sci, "agent-a")
          .args(["park", &resumed, "pick it up"])
          .assert()
          .success();
      let held_lane = id_of(env.json(&sci, &["add", "Held lane", "--lane", "-p", "1"]));
      let waits = id_of(env.json(
          &sci,
          &["add", "Needs quiet", "--parent", &held_lane, "--need", "quiet"],
      ));
      let capture = id_of(env.json(&sci, &["add", "Capture", "--need", "quiet", "-p", "4"]));
      as_agent(&env, &sci, "other")
          .args(["start", &capture])
          .assert()
          .success();
      let waiting_lane = id_of(env.json(&sci, &["add", "Waiting lane", "--lane", "-p", "2"]));
      let review = id_of(env.json(&sci, &["add", "Review", "--parent", &waiting_lane]));
      as_agent(&env, &sci, "agent-a")
          .args(["park", &review, "look at it", "--waiting-on", "user"])
          .assert()
          .success();
      let empty_lane = id_of(env.json(&sci, &["add", "Empty lane", "--lane", "-p", "3"]));
      let paused_lane = id_of(env.json(&sci, &["add", "Paused lane", "--lane", "-p", "4"]));
      let running = id_of(env.json(&sci, &["add", "Running", "--parent", &paused_lane]));
      as_agent(&env, &sci, "other")
          .args(["start", &running])
          .assert()
          .success();
      env.json(&sci, &["block", &paused_lane, "the host is busy"]);
      let shelved_lane = id_of(env.json(&sci, &["add", "Shelved lane", "--lane"]));
      env.json(&sci, &["shelve", &shelved_lane, "next quarter"]);

      let view = json_as_session(&env, &sci, "me", &["lanes"]);
      let rows = view["lanes"].as_array().unwrap();
      let order: Vec<&str> = rows.iter().map(|row| row["lane"]["id"].as_str().unwrap()).collect();
      assert_eq!(
          order,
          [
              ready_lane.as_str(),
              held_lane.as_str(),
              waiting_lane.as_str(),
              empty_lane.as_str(),
              paused_lane.as_str()
          ],
          "lane order; the shelved lane is absent"
      );

      let ready = &rows[0];
      assert_eq!(ready["state"], "ready");
      assert_eq!(ready["pick"]["id"], resumed, "parked candidates first, as next takes them");
      assert_eq!(ready["steps"], 1);
      assert_eq!(ready["guidance"], "Ship the captures. First milestone: one clean run.");
      assert!(ready.get("held").is_none() && ready.get("causes").is_none(), "{ready}");
      assert_eq!(
          json_as_session(&env, &sci, "me", &["next", "--under", &ready_lane])["next"]["task"]["id"],
          resumed
      );

      let held = &rows[1];
      assert_eq!(held["state"], "held");
      assert_eq!(held["pick"], serde_json::Value::Null);
      assert_eq!(
          held["held"],
          serde_json::json!([{"id": waits, "need": "quiet", "holder": capture, "by": "claim"}])
      );
      assert_eq!(held["causes"], serde_json::json!({"held": 1}));
      assert_eq!(held["steps"], 0, "a held step counts under held only");

      assert_eq!(rows[2]["state"], "waiting");
      assert_eq!(rows[2]["causes"], serde_json::json!({"user": 1}));
      assert_eq!(rows[3]["state"], "empty");
      assert_eq!(rows[3]["guidance"], serde_json::Value::Null, "an empty body");

      let paused = &rows[4];
      assert_eq!(paused["state"], "paused");
      assert_eq!(paused["pick"], serde_json::Value::Null);
      assert_eq!(paused["active"][0]["id"], running);
      assert!(paused.get("causes").is_none(), "{paused}");

      let out = as_agent(&env, &sci, "me").args(["--pretty", "lanes"]).output().unwrap();
      let text = String::from_utf8_lossy(&out.stdout);
      assert!(text.contains(&format!("ready → {resumed} Resumed step")), "{text}");
      assert!(text.contains("    Ship the captures. First milestone: one clean run."), "{text}");
      assert!(text.contains(&format!("held: quiet ← {capture}")), "{text}");
      assert!(text.contains("waiting: 1 user"), "{text}");
      assert!(text.contains(&format!("    active: {running} Running @")), "{text}");

      env.json(&sci, &["unblock", &paused_lane]);
      let view = json_as_session(&env, &sci, "me", &["lanes"]);
      assert_eq!(view["lanes"][4]["state"], "waiting");
      assert_eq!(view["lanes"][4]["causes"], serde_json::json!({"active": 1}));
  }

  #[test]
  fn lanes_view_counts_every_live_descendant_once_including_recurrences() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let due_lane = id_of(env.json(&sci, &["add", "Due lane", "--lane", "-p", "0"]));
      let due = id_of(env.json(&sci, &["add", "Due sweep", "--parent", &due_lane]));
      env.json(&sci, &["done", &due]);
      env.json(&sci, &["edit", &due, "--every", "30d"]);
      let soon_lane = id_of(env.json(&sci, &["add", "Soon lane", "--lane", "-p", "1"]));
      let soon = id_of(env.json(&sci, &["add", "Soon sweep", "--parent", &soon_lane, "--every", "30d"]));
      env.json(&sci, &["start", &soon]);
      env.json(&sci, &["done", &soon]);
      let gone_lane = id_of(env.json(&sci, &["add", "Gone lane", "--lane", "-p", "2"]));
      let gone = id_of(env.json(&sci, &["add", "Gone sweep", "--parent", &gone_lane, "--every", "30d"]));
      env.json(&sci, &["drop", &gone, "retired"]);
      let deep_lane = id_of(env.json(&sci, &["add", "Deep lane", "--lane", "-p", "3"]));
      let deep_goal = id_of(env.json(&sci, &["add", "Deep goal", "--parent", &deep_lane]));
      let deep_step = id_of(env.json(&sci, &["add", "Deep step", "--parent", &deep_goal]));

      let mixed = id_of(env.json(&sci, &["add", "Mixed lane", "--lane", "-p", "4"]));
      let first = id_of(env.json(&sci, &["add", "First", "--parent", &mixed, "-p", "0"]));
      env.json(&sci, &["add", "Second", "--parent", &mixed, "-p", "1"]);
      let sub = id_of(env.json(&sci, &["add", "Sub-goal", "--parent", &mixed, "-p", "1"]));
      env.json(&sci, &["add", "Sub step", "--parent", &sub, "-p", "2"]);
      let blocked = id_of(env.json(&sci, &["add", "Blocked", "--parent", &mixed]));
      env.json(&sci, &["block", &blocked, "why"]);
      let both = id_of(env.json(&sci, &["add", "Deferred and blocked", "--parent", &mixed]));
      env.json(&sci, &["block", &both, "why"]);
      env.json(&sci, &["edit", &both, "--defer", "2099-01-01"]);
      env.json(&sci, &["add", "Depends", "--parent", &mixed, "--depends", &blocked]);
      env.json(&sci, &["add", "Idea", "--parent", &mixed, "--status", "idea"]);
      let closed = id_of(env.json(&sci, &["add", "Closed", "--parent", &mixed]));
      env.json(&sci, &["done", &closed, "landed"]);
      let shelved = id_of(env.json(&sci, &["add", "Shelved", "--parent", &mixed, "--status", "idea"]));
      env.json(&sci, &["shelve", &shelved, "later"]);

      let view = env.json(&sci, &["lanes"]);
      let rows = view["lanes"].as_array().unwrap();
      assert_eq!(rows[0]["state"], "ready", "{view}");
      assert_eq!(rows[0]["pick"]["id"], due, "a due recurrence is the pick");
      assert_eq!(rows[1]["state"], "waiting");
      assert_eq!(rows[1]["causes"], serde_json::json!({"periodic": 1}), "not empty");
      assert_eq!(rows[2]["state"], "empty", "a dropped recurrence is not live");
      assert_eq!(rows[3]["pick"]["id"], deep_step, "picked from inside the sub-goal");
      assert_eq!(rows[3]["causes"], serde_json::json!({"goal": 1}));

      let row = &rows[4];
      assert_eq!(row["lane"]["id"], mixed);
      assert_eq!(row["pick"]["id"], first);
      assert_eq!(row["steps"], 2, "Second and Sub step");
      assert_eq!(
          row["causes"],
          serde_json::json!({"deferred": 1, "blocked": 1, "depends": 1, "goal": 1, "other": 1}),
          "deferred-and-blocked counts once, under deferred; closed and shelved are absent"
      );
  }

  #[test]
  fn an_earlier_lane_wins_a_contested_exclusive_need_without_reordering_next() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      declare_lane_need(&sci, "quiet", true);
      let first_lane = id_of(env.json(&sci, &["add", "First lane", "--lane", "-p", "1"]));
      let a1 = id_of(env.json(&sci, &["add", "A1", "--parent", &first_lane, "--need", "quiet", "-p", "2"]));
      let second_lane = id_of(env.json(&sci, &["add", "Second lane", "--lane", "-p", "2"]));
      let b1 = id_of(env.json(&sci, &["add", "B1", "--parent", &second_lane, "--need", "quiet", "-p", "0"]));
      let b2 = id_of(env.json(&sci, &["add", "B2", "--parent", &second_lane, "-p", "3"]));

      let view = env.json(&sci, &["lanes"]);
      let rows = view["lanes"].as_array().unwrap();
      assert_eq!(rows[0]["pick"]["id"], a1);
      assert_eq!(rows[1]["pick"]["id"], b2, "the later lane picks its next step");
      assert_eq!(
          rows[1]["held"],
          serde_json::json!([{"id": b1, "need": "quiet", "holder": a1, "by": "pick"}])
      );
      assert_eq!(rows[1]["causes"], serde_json::json!({"held": 1}));
      assert_eq!(rows[1]["steps"], 0);
      assert_eq!(
          env.json(&sci, &["next"])["next"]["task"]["id"],
          b1,
          "lane order never reorders next"
      );

      env.json(&sci, &["drop", &b2, "not needed"]);
      let view = env.json(&sci, &["lanes"]);
      assert_eq!(view["lanes"][1]["state"], "held");
      assert_eq!(view["lanes"][1]["pick"], serde_json::Value::Null);
  }

  #[test]
  fn lanes_view_reports_the_session_gates_as_causes() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      declare_lane_need(&sci, "owner", false);
      let lane = id_of(env.json(&sci, &["add", "Lane", "--lane", "-p", "1"]));
      env.json(
          &sci,
          &["add", "Needs the owner", "--parent", &lane, "-p", "0", "--need", "owner", "--complexity", "low"],
      );
      env.json(&sci, &["add", "Unrated", "--parent", &lane, "-p", "1"]);
      env.json(
          &sci,
          &["add", "Below the halt", "--parent", &lane, "-p", "3", "--complexity", "low"],
      );
      env.json(&sci, &["add", "Stop the line", "-p", "0", "--tag", "halt"]);

      let view = env.json(&sci, &["lanes", "--without", "owner", "--max-complexity", "low"]);
      assert_eq!(view["lanes"][0]["state"], "waiting", "{view}");
      assert_eq!(
          view["lanes"][0]["causes"],
          serde_json::json!({"without": 1, "cutoff": 1, "halt": 1})
      );
      assert_eq!(env.json(&sci, &["lanes"])["lanes"][0]["state"], "ready");
      assert_eq!(env.fail(&sci, &["lanes", "--without", "nope"]), "unknown_need");
  }

  #[test]
  fn lanes_across_projects_name_their_own_lane() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let fam = env.init("fam");
      let sci_lane = id_of(env.json(&sci, &["add", "Sci lane", "--lane", "-p", "1"]));
      env.json(&sci, &["add", "Sci step", "--parent", &sci_lane]);
      let fam_lane = id_of(env.json(&fam, &["add", "Fam lane", "--lane", "-p", "0"]));
      let fam_goal = id_of(env.json(&fam, &["add", "Fam goal", "--parent", &fam_lane]));
      let fam_step = id_of(env.json(&fam, &["add", "Fam step", "--parent", &fam_goal]));

      let view = env.json(&sci, &["lanes", "--all-projects"]);
      let rows = view["lanes"].as_array().unwrap();
      assert_eq!(rows.len(), 2, "{view}");
      assert_eq!(rows[0]["lane"]["id"], fam_lane, "priority orders across projects");
      assert_eq!(rows[0]["lane"]["in_lane"], fam_lane);
      assert_eq!(rows[0]["pick"]["id"], fam_step);
      assert_eq!(rows[0]["pick"]["in_lane"], fam_lane);
      assert_eq!(rows[1]["pick"]["in_lane"], sci_lane);

      assert_eq!(env.json(&sci, &["lanes"])["lanes"].as_array().unwrap().len(), 1);
      assert_eq!(
          env.json(&sci, &["lanes", "--project", "fam"])["lanes"][0]["lane"]["id"],
          fam_lane
      );
  }
  ```

- [ ] **Step 6: Run them and see them fail.**

  `just test-one --test cli lanes_view_reports_every_state` (and the other four) — `tasks
  lanes` exits 2 with `unrecognized subcommand 'lanes'`, so `json_as_session`/`env.json`
  panic; `project_and_all_projects_conflict_on_every_read_command` still passes (exit 2 for
  an unknown subcommand), and keeps guarding the conflict once the command exists.

- [ ] **Step 7: Implement the command.**

  `src/cli.rs`, after the `Prime { .. }` variant:

  ```rust
      /// Each open lane: its guidance, its state, and the step it could take now.
      Lanes {
          #[command(flatten)]
          without: WithoutArgs,
          /// Hide steps rated above this level and unassessed steps; overrides
          /// TASKS_MAX_COMPLEXITY.
          #[arg(
              long,
              value_name = "LEVEL",
              add = ArgValueCandidates::new(crate::complete::complexities),
              add = ValueSet,
              value_parser = ValueSet
          )]
          max_complexity: Option<String>,
          #[command(flatten)]
          scope: ScopeArgs,
      },
  ```

  `src/commands/mod.rs`: `pub mod lanes;` after `pub mod init;`; dispatch arm after
  `Command::Prime`:

  ```rust
          Command::Lanes {
              without,
              max_complexity,
              scope,
          } => lanes::run(open_read_ctx(dir, &scope)?, without, max_complexity),
  ```

  `src/commands/list.rs`: `fn halt_snapshots(` becomes `pub(super) fn halt_snapshots(`.

  Create `src/commands/lanes.rs`:

  ```rust
  //! `tasks lanes`, and the `lanes` rows `prime` shares with it (lanes design §5). The
  //! builder is `crate::lanes`; this module gathers what it reads.

  use super::ReadCtx;
  use crate::claims::ClaimSnapshot;
  use crate::cli::WithoutArgs;
  use crate::error::Result;
  use crate::halt::HaltSnapshot;
  use crate::lanes::{Inputs, LaneRow};
  use crate::model::{Complexity, Task, TaskId};
  use crate::needs::{Vocabulary, Without};
  use crate::output::{LanesOut, Output};
  use std::collections::HashMap;
  use time::OffsetDateTime;

  pub fn run(
      mut ctx: ReadCtx,
      without: WithoutArgs,
      max_complexity: Option<String>,
  ) -> Result<Output> {
      let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
      // The strict-flag, lenient-variable union `ready`, `next`, and `prime` build.
      let without = Without::from_env(&without.without, &super::list::vocabularies(&ctx.scope))?;
      let (all, claims) = ctx.scan_with_claims()?;
      let now = crate::time::parse(&crate::time::now())?;
      let halts = super::list::halt_snapshots(&mut ctx, &all);
      let lanes = rows(&mut ctx, &all, &claims, &halts, cutoff, &without, now)?;
      Ok(Output::Lanes(LanesOut {
          lanes,
          warnings: ctx.warnings,
      }))
  }

  /// Every open, unshelved lane in scope as a `LaneRow`. Holds and this session's identity
  /// are read only when a lane exists, and a warning another section of the same command
  /// already gave (the hold-store warnings `ready` and `prime` also raise) is not repeated.
  /// Dependencies are resolved only for `dependency_readers`, as `ready_tasks` resolves
  /// only its candidates' (lanes design §5.1 step 1), so a dependency that only closed,
  /// shelved, or paused work names can never fail the view or `prime`.
  pub(super) fn rows(
      ctx: &mut ReadCtx,
      all: &[Task],
      claims: &ClaimSnapshot,
      halts: &HashMap<String, HaltSnapshot>,
      cutoff: Option<Complexity>,
      without: &Without,
      now: OffsetDateTime,
  ) -> Result<Vec<LaneRow>> {
      if !all
          .iter()
          .any(|task| task.lane && crate::hierarchy::is_active(task))
      {
          return Ok(Vec::new());
      }
      let mut closed: HashMap<TaskId, Option<bool>> = HashMap::new();
      for task in crate::lanes::dependency_readers(all, claims, &ctx.registry, now) {
          for dependency in &task.depends {
              if closed.contains_key(dependency) {
                  continue;
              }
              let value = super::list::resolve_dependency(ctx, all, dependency)?
                  .map(|found| !found.status.is_open());
              closed.insert(dependency.clone(), value);
          }
      }
      let (holds, mut fresh) = crate::holds::HoldSnapshot::load(&ctx.registry, now);
      let me = crate::claims::resolve_identity(&mut fresh);
      for warning in fresh {
          if !ctx.warnings.contains(&warning) {
              ctx.warnings.push(warning);
          }
      }
      // Same-session holds follow the claim ownership rule, including process proof
      // across a native-to-relay identity change (Slice 2 Task 2.2).
      let mine = super::own_holds(&holds, &me)?;
      let vocabularies: HashMap<&str, &Vocabulary> = ctx
          .scope
          .projects()
          .iter()
          .map(|project| (project.prefix.as_str(), &project.needs))
          .collect();
      let dependency = |id: &TaskId| closed.get(id).copied().flatten();
      Ok(crate::lanes::build(&Inputs {
          all,
          claims,
          registry: &ctx.registry,
          dependency: &dependency,
          halts,
          cutoff,
          without,
          holds: &holds,
          vocabularies: &vocabularies,
          mine: &mine,
          now,
      }))
  }
  ```

  `src/output.rs`: add `use crate::lanes::{Causes, LaneRow, LaneState};`. After `PrimeOut`:

  ```rust
  /// `tasks lanes` (lanes design §5.3).
  #[derive(Serialize)]
  pub struct LanesOut {
      pub lanes: Vec<LaneRow>,
      pub warnings: Vec<String>,
  }
  ```

  `Output` gains `Lanes(LanesOut),` after `Prime(PrimeOut),`. In `pretty`, after the
  `Output::Prime` arm:

  ```rust
          Output::Lanes(o) => lane_lines(
              &o.lanes,
              painter,
              id_width(o.lanes.iter().map(|row| row.lane.id.as_str())),
              true,
          ),
  ```

  New functions after `tree_text`:

  ```rust
  /// One line per lane: id, priority, title, then its state with the pick or its main
  /// cause (lanes design §5.3). `detail` adds the guidance and the active claims under each
  /// row, as `tasks lanes` prints them; `prime` leaves them out.
  fn lane_lines(rows: &[LaneRow], painter: &Painter, id_width: usize, detail: bool) -> String {
      let mut rendered = String::new();
      for row in rows {
          let id = painter.paint(Style::Chrome, &format!("{:<id_width$}", row.lane.id));
          let priority = painter.paint(
              Style::Priority(row.lane.priority),
              &format!("P{}", row.lane.priority),
          );
          let state = match row.state {
              LaneState::Ready => {
                  let pick = row.pick.as_ref().expect("a ready lane has a pick");
                  format!("ready → {} {}", pick.id, pick.title)
              }
              LaneState::Held => {
                  let first = row.held.first().expect("a held lane names what it waits for");
                  format!("held: {} ← {}", first.need, first.holder)
              }
              LaneState::Waiting => waiting_text(&row.causes),
              LaneState::Paused => "paused".into(),
              LaneState::Empty => "empty".into(),
          };
          rendered.push_str(&format!(
              "{id}  {priority}  {}  {}\n",
              row.lane.title,
              painter.paint(Style::Emphasis, &state)
          ));
          if !detail {
              continue;
          }
          if let Some(guidance) = &row.guidance {
              rendered.push_str(&format!("    {guidance}\n"));
          }
          for active in &row.active {
              let holder = active
                  .claim
                  .as_ref()
                  .map(|claim| format!(" @{} [{}]", claim.owner, claim.session))
                  .unwrap_or_default();
              rendered.push_str(&painter.paint(
                  Style::Chrome,
                  &format!("    active: {} {}{holder}", active.id, active.title),
              ));
              rendered.push('\n');
          }
      }
      rendered
  }

  /// `waiting: 2 user, 1 deferred`: every cause with its count, the largest first, ties in
  /// table order.
  fn waiting_text(causes: &Causes) -> String {
      let mut entries = causes.entries();
      entries.sort_by(|a, b| b.1.cmp(&a.1));
      let parts: Vec<String> = entries
          .iter()
          .map(|(cause, count)| format!("{count} {cause}"))
          .collect();
      format!("waiting: {}", parts.join(", "))
  }
  ```

  `needs_theme` and `shows_priority` each gain `| Output::Lanes(_)`; `warnings_of` gains
  `Output::Lanes(o) => o.warnings.clone(),`.

- [ ] **Step 8: Adopt this task's rows.** In `.worktrees/ece1e2-work-selection`:

  ```bash
  SHA=$(tasks show tasks-ece1e2 | grep -o 'inventory: Task 3\.7 = [0-9a-f]*' | awk '{print $NF}')
  test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
  ```

  It also refreshes `tools/cli_surface.py` when the pair changed. Run
  `just test-one parser_surface_equals_table`. Expected: PASS (the `lanes` command row and
  its four options match the parser).

- [ ] **Step 9: Run the tests and see them pass.**

  `just test-one lanes::tests`;
  `just test-one --test cli lanes_view_reports_every_state`;
  `just test-one --test cli lanes_view_counts_every_live_descendant`;
  `just test-one --test cli an_earlier_lane_wins`;
  `just test-one --test cli lanes_view_reports_the_session_gates`;
  `just test-one --test cli lanes_across_projects`;
  `just test-one --test cli project_and_all_projects_conflict`.

- [ ] **Step 10: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 11: Commit.**

  ```bash
  git add src/lanes.rs src/main.rs src/commands/lanes.rs src/commands/mod.rs \
    src/commands/list.rs src/cli.rs src/output.rs tests/cli.rs tools/cli.toml
  # tools/cli_surface.py when the adopt changed it, and tests/common/mod.rs when Step 5
  # had to clear TASKS_WITHOUT there:
  git add $(git ls-files --modified --others --exclude-standard \
    tools/cli_surface.py tests/common/mod.rs)
  git commit -m "feat(lanes): tasks lanes shows each lane's pick, held steps, and causes (tasks-ece1e2)"
  ```

---

### Task 3.8: The `lanes` section of `prime`

**Files**
- Modify `src/output.rs`: `PrimeOut` (after `doing`, line 813); `Output::Prime` pretty
  arm (between the closeout table and the `roadmap:` header, lines 1058–1068).
- Modify `src/commands/list.rs`: `prime` (after `ctx.warnings.extend(held);`, line 568;
  the `PrimeOut` literal, line 615).
- Modify `tests/cli.rs`: two new tests.

**Interfaces**
- Consumes: `commands::lanes::rows` (3.7); Slice 1's `without` binding, the `Without`
  Task 1.6 resolves at the top of `prime` from its `--without` and `TASKS_WITHOUT`;
  `prime`'s `cutoff`, `snapshots`, `claims`, `all`, `now`.
- Produces: `PrimeOut.lanes: Vec<LaneRow>` (always present, `[]` with no lanes); pretty
  `lanes:` block before `roadmap:`.

- [ ] **Step 1: Write the failing tests.**

  ```rust
  #[test]
  fn prime_always_carries_lanes_and_prints_them_before_the_roadmap() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      env.json(&sci, &["add", "Loose"]);
      let prime = env.json(&sci, &["prime"]);
      assert_eq!(prime["lanes"], serde_json::json!([]), "present and empty: {prime}");
      assert!(!env.pretty(&sci, &["prime"]).contains("lanes:"));

      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "1", "-b", "## Captures"]));
      let step = id_of(env.json(&sci, &["add", "Take one", "--parent", &lane]));
      let prime = env.json(&sci, &["prime"]);
      assert_eq!(prime["lanes"][0]["lane"]["id"], lane);
      assert_eq!(prime["lanes"][0]["state"], "ready");
      assert_eq!(prime["lanes"][0]["pick"]["id"], step);
      assert_eq!(
          prime["lanes"][0]["guidance"],
          serde_json::Value::Null,
          "a body that is only a heading has no guidance"
      );

      let text = env.pretty(&sci, &["prime"]);
      let lanes_at = text.find("\nlanes:\n").unwrap_or_else(|| panic!("{text}"));
      assert!(lanes_at < text.find("\nroadmap:\n").unwrap(), "{text}");
      assert!(text.contains(&format!("ready → {step} Take one")), "{text}");

      let all = env.json(&sci, &["prime", "--all-projects"]);
      assert_eq!(all["lanes"][0]["lane"]["id"], lane, "{all}");
  }

  #[test]
  fn prime_and_lanes_never_read_the_dependencies_of_closed_or_shelved_lane_work() {
      let mut env = TestEnv::new();
      let sci = env.init("sci");
      let fam = env.init("fam");
      let far = id_of(env.json(&fam, &["add", "Far"]));
      env.json(&fam, &["done", &far, "landed"]);
      let lane = id_of(env.json(&sci, &["add", "Captures", "--lane", "-p", "1"]));
      let step = id_of(env.json(&sci, &["add", "Take one", "--parent", &lane]));
      let closed = id_of(env.json(&sci, &["add", "Closed", "--parent", &lane, "--depends", &far]));
      env.json(&sci, &["done", &closed, "landed"]);
      let shelved = id_of(env.json(
          &sci,
          &["add", "Shelved", "--parent", &lane, "--status", "idea", "--depends", &far],
      ));
      env.json(&sci, &["shelve", &shelved, "later"]);
      // Garble the one record both depend on, as
      // check_reports_unparsable_foreign_dependency_as_warning does: resolving it is now a
      // parse error, so any read of these two records' dependencies fails the command.
      std::fs::write(fam.join(format!("tasks/{far}.md")), "garbage").unwrap();

      let prime = env.json(&sci, &["prime"]);
      let row = &prime["lanes"][0];
      assert_eq!(row["lane"]["id"], lane, "{prime}");
      assert_eq!(row["state"], "ready", "{row}");
      assert_eq!(row["pick"]["id"], step);
      assert_eq!(row["steps"], 0);
      assert!(
          row.get("causes").is_none() && row.get("held").is_none(),
          "a closed child and a shelved child are not live: {row}"
      );
      let view = env.json(&sci, &["lanes"]);
      assert_eq!(view["lanes"][0]["pick"]["id"], step, "{view}");
  }
  ```

- [ ] **Step 2: Run them and see them fail.**

  `just test-one --test cli prime_always_carries_lanes` — `prime["lanes"]` is `Null`, not
  `[]`. `just test-one --test cli prime_and_lanes_never_read` — `prime` succeeds without a
  `lanes` key, so `row["lane"]["id"]` is `Null` and the first assertion fails. (`tasks lanes`
  already passes here, because Task 3.7's `rows` reads only `dependency_readers`; a `rows`
  that read every descendant's dependencies would fail both commands on the garbled
  record.)

- [ ] **Step 3: Implement.**

  `src/output.rs`, `PrimeOut`, after `pub doing: Vec<TaskSummary>,`:

  ```rust
      /// Lanes design §5: every open lane in scope; always present, `[]` without lanes.
      pub lanes: Vec<crate::lanes::LaneRow>,
  ```

  In the `Output::Prime` arm, after the closeout `rendered.push_str(&table(&o.closeout, ...));`
  and before the `roadmap:` header:

  ```rust
              if !o.lanes.is_empty() {
                  rendered.push_str(&format!(
                      "\n{}\n",
                      painter.paint(Style::Emphasis, "lanes:")
                  ));
                  let lane_width = o.lanes.iter().map(|row| row.lane.id.len()).max().unwrap_or(0);
                  rendered.push_str(&lane_lines(&o.lanes, painter, lane_width, false));
              }
  ```

  `src/commands/list.rs`, `prime`, after `ctx.warnings.extend(held);`:

  ```rust
      // Lanes design §5: the same builder as `tasks lanes`, under this session's cutoff and
      // `--without` (`without` is the `Without` Slice 1 binds at the top of `prime`).
     
      let lanes = super::lanes::rows(&mut ctx, &all, &claims, &snapshots, cutoff, &without, now)?;
  ```

  In the `PrimeOut` literal, after the `doing: ...` field, add `lanes,`.

- [ ] **Step 4: Run the tests and see them pass.**

  `just test-one --test cli prime_always_carries_lanes`,
  `just test-one --test cli prime_and_lanes_never_read`, and the existing
  `just test-one --test cli prime_shows_roadmap_and_closeout` (no lanes, so its pretty text
  is unchanged).

- [ ] **Step 5: Gate.** `cargo fmt`, `just test-fast`, `tasks check`.

- [ ] **Step 6: Commit.**

  ```bash
  git add src/output.rs src/commands/list.rs tests/cli.rs
  git commit -m "feat(prime): carry the lanes view in prime (tasks-ece1e2)"
  ```

---

### Task 3.9: Lanes in the design, the skills, and the README

**Files**
- Modify `docs/specs/2026-08-29-tasks-design.md`: §5 usage (`list` line 286–288, `ready`
  line 311, `next` lines 401–404, `prime` lines 406–416, new `tasks lanes` entry after
  `prime`), §5.1 `+=` block.
- Modify `skills/tasks/SKILL.md`: Session protocol steps 1–2 (lines 17–48), Recording work
  task operations (after the "Decomposing" bullet, line 336).
- Modify `skills/scope/SKILL.md`: §4 (after line 110).
- Modify `README.md`: Use block (after the 3.1 lane lines), Work block (line 61).

**Interfaces**
- None (documentation of 3.1–3.8).

- [ ] **Step 1: Write the design doc entries.** In `docs/specs/2026-08-29-tasks-design.md`:

  - `tasks list` usage: append `[--under ID]` after `[--parent ID]`, and in its prose add:
    "--under keeps descendants of ID at any depth; --parent keeps direct children."
  - `tasks ready` usage line becomes
    `tasks ready [--size S] [--parallel] [--under ID] [-n N] [--project P | --all-projects]`
    (keeping Slice 1's `--need`/`--without` additions), and its prose gains: "A paused lane
    (a `blocked` goal marked `lane`) hides its descendants, with one warning per lane."
  - `tasks next` becomes:

    ```text
    tasks next [--under ID] [--project P | --all-projects]
        The most recently parked task waiting on the agent that is open, unblocked,
        with all dependencies resolved and closed, and childless, else the first ready task,
        in the show shape. --under picks only among descendants of ID at any depth: how a
        session committed to one lane takes its next step. Without it, lanes never reorder
        the pick.
    ```

  - `tasks prime` prose gains: "`lanes` lists every open lane with its guidance, state,
    and pick, as `tasks lanes` does; it is always present and empty without lanes. Pretty
    output prints a `lanes:` block before `roadmap:` when there is a lane."
  - New entry after `tasks prime`:

    ```text
    tasks lanes [--without NEED]... [--max-complexity LEVEL] [--project P | --all-projects]
        Every open, unshelved lane in lane order (priority, size, created, id), with its
        guidance (the first body paragraph after headings), its state (paused, ready, held,
        waiting, empty), the step it could take now, its active claims, the steps held for
        an exclusive need (by another session's claim or an earlier lane's pick), and the
        causes for the rest of its live descendants. See
        2026-10-03-lanes-needs-groups-design.md §5.
    ```

  - §5.1, after the 3.1 `lane` lines:

    ```text
    TaskSummary += in_lane: string                the nearest lane at or above (its own id for a lane); omitted when none
    ParkedRow   += in_lane: string                omitted when unresolved or none
    show        += in_lane: string                next carries the same field
    prime       += lanes: [LaneRow]               always present; [] when the scope has no lane
    lanes       -> { lanes: [LaneRow], warnings }
                   LaneRow = { lane: TaskSummary, guidance: string|null,
                     state: "paused"|"ready"|"held"|"waiting"|"empty", pick: TaskSummary|null,
                     steps: int, active: [TaskSummary],
                     held: [{ id, need, holder, by: "claim"|"pick" }] (omitted when empty),
                     causes: { active|held|without|cutoff|halt|deferred|periodic|user|
                               blocked|depends|goal|other: int } (non-zero only; omitted when empty) }
    ready/next  += one warning per paused lane: "<n> task(s) hidden by paused lane <id>"
    list/ready/next += --under ID: descendants at any depth; an ID not in scope is task_not_found
    errors      += nested_lane
    check       += kinds nested_lane (error), childless_lane (warning); periodic_goal and
                   deferred_goal also cover a lane with no children
    ```

- [ ] **Step 2: Write the skill guidance.** In `skills/tasks/SKILL.md`:

  Step 1 of Session protocol becomes:

  ```markdown
  1. `tasks prime` — lanes (the efforts meant to run side by side, each with its guidance
     and the step it could take now), roadmap (the open goal tree), closeout (goals whose
     work is all done), the ready list, and who is working on what.
  ```

  In step 2, after "Never pick a task with children; those are goals. `ready` already omits
  them.":

  ```markdown
     A lane (`lane: true`) is a goal too, even before it has children, and every row names
     its lane in `in_lane`. A session committed to one lane picks with
     `tasks next --under <lane>`; `--under` also narrows `list` and `ready` to descendants
     at any depth, while `--parent` stays direct children. Without `--under`, `next` keeps
     its priority order. `tasks lanes` answers what can run in parallel: each lane is
     `ready` with its pick, `held` on an exclusive need, `waiting` with its causes,
     `paused`, or `empty`.
  ```

  In Recording work, after the "Decomposing" bullet:

  ```markdown
  - An effort meant to run beside the project's other efforts:
    `tasks add "<effort>" --lane -p <0-4> -b "<why it exists and its first milestone>"`,
    then decompose it with `--parent` like any goal. Lead the body with that paragraph:
    `prime` and `tasks lanes` show it as the lane's guidance, skipping headings above it.
    Make a lane only for a standalone effort; a sub-effort inside a lane is an ordinary
    child goal, and a lane inside a lane is refused (`nested_lane`). A lane's priority
    ranks it in the lanes view and decides which lane wins a contested exclusive need; it
    never reorders `ready` or `next`. Pause a lane with `tasks block <lane> "<why>"`: its
    subtree leaves `ready`, `next`, and `prime`'s ready list, with a warning, until
    `tasks unblock <lane>`. `start` on a task inside a paused lane still works. Never
    pause with `shelve`, which hides the lane from every view and needs its subtree shelved
    first. `check` warns about a lane with no steps yet.
  ```

  In `skills/scope/SKILL.md` §4, after "Research and design follow-ups are children of the
  cluster goal.":

  ```markdown
  When the cluster is a standalone effort meant to run beside the project's other work,
  create that goal with `--lane` and lead its body with the guidance paragraph: why the
  effort exists and its first milestone. A cluster inside an existing lane gets an ordinary
  child goal, never a lane. When a scoped member's work uses a shared resource the project
  declares under `[needs]`, record it with `tasks edit <id> --need <name>`.
  ```

  (If Slice 1 already added the `--need` sentence here, keep only the lane sentences.)

- [ ] **Step 3: Write the README lines.** In the Use block, after the 3.1 lane lines:

  ```text
      tasks lanes                      # each lane: guidance, state, and the step it could take now
      tasks next --under sci-4f2a9c    # the next step inside one lane, at any depth
      tasks list --under sci-4f2a9c    # a goal's whole subtree (--parent is direct children)
      tasks block sci-4f2a9c "host busy"  # pause a lane; tasks unblock resumes it
  ```

  In the Work block (line 61), change the `tasks prime` comment to
  `# counts, lanes, ready list, who is doing what`.

- [ ] **Step 4: Check the documents against the code.** Run
  `tasks --help | grep -n lanes`, `tasks lanes --help`, `tasks next --help | grep under`
  using the worktree build (`cargo run -q -- lanes --help` and
  `cargo run -q -- next --help`), and confirm each flag and the JSON key names in §5.1
  match what the commands print.

- [ ] **Step 5: Gate.** `just test-fast`, `tasks check` (README and design docs keep the
  full pre-commit check).

- [ ] **Step 6: Commit.**

  ```bash
  git add docs/specs/2026-08-29-tasks-design.md skills/tasks/SKILL.md \
    skills/scope/SKILL.md README.md
  git commit -m "docs(lanes): record lanes in the design, skills, and README (tasks-ece1e2)"
  ```

---

### Spec decisions this slice makes

- **Guidance joins its lines with a space.** §3.5 says "runs up to the next blank line"
  but not how a multi-line paragraph is rendered; joining keeps the JSON value and the
  pretty line single-line, as a Markdown paragraph reads. A heading needs a space after its
  `#`s (`#hashtag` opens a paragraph).
- **The builder classifies each descendant itself** rather than calling `ready_tasks`, so
  the hold gate Slice 2 places in the pickers cannot remove steps before the view counts
  them as `held`. Parity with `next` is pinned by the parked-first unit test and the
  `next --under` comparison in the end-to-end test.
- **Gate causes apply to sub-goals too.** §5.1's "first cause in table order" is applied
  literally, so a sub-goal hidden by `--without`, the cutoff, or a halt counts under that
  cause rather than `goal`.
- **`steps` counts only unpicked steps after the pick.** Steps before the pick were all
  skipped for holds (they are `held`); steps after it are not tested against holds.
- **Pretty `prime` prints the `lanes:` block only when a lane exists**; JSON always carries
  `lanes`. This keeps every existing pretty `prime` output byte-identical.
- **The waiting line lists every cause, largest count first**, matching §5.3's example
  (`waiting: 2 user, 1 deferred`), with table order breaking ties.
- **The paused warning counts only work that would be offered if the lane were
  unpaused.** A paused-lane task is counted once it passes every other gate the command
  applies: the dependency and defer checks, the live-claim and user-park checks (inside
  `ready_tasks`, silently for paused work), and the halt, `--without`, and cutoff gates
  (in `warn_paused`, silently). It covers `ready` rows and parked candidates, counted once
  by id in `next`. A deferred step in a paused lane is in neither the deferred count nor
  the paused one. The exclusive-hold gate (Slice 2's `retain_unheld`) is not applied to
  paused work: a hold is another session's live, transient claim, and running that gate
  would read every registered claim store and repeat its warnings for work nobody can take
  anyway; a held step in a paused lane is still counted.
- **`depends` is judged only for work that could be a step.** The view resolves
  dependencies only for `dependency_readers` (live descendants of unpaused lanes that are
  todo, a due recurrence, or an open parked-agent candidate), the same records
  `ready_tasks` and `parked::candidates` read them for. A closed or shelved descendant, or
  anything under a paused lane, is never read, so a garbled or vanished foreign record that
  only such work depends on cannot fail `tasks lanes` or `prime`. The cost is one cause
  shift: an unparked `idea`, or a `doing` record without a live claim, that has an open
  dependency counts under `other` (or `goal`) rather than `depends`; neither could be a
  step either way.
- **`childless_lane` applies to open, unshelved lanes**, and `nested_lane` is reported at
  the inner lane.

---

## Slice 4 — Project groups

Spec: `docs/specs/2026-10-03-lanes-needs-groups-design.md` §6, with the groups parts of §7
(tasks-77dbc6), §8 (`prime.group`, the `groups` and `group set|rm` payloads, the
`unknown_group` kind), §9 (skill, README, main design) and §10 (**Groups**). This slice
executes **after Slice 3**, in the plan's fixed order 1 → 2 → 3 → 4, so each task's
`vendored adopt --at` commit is at or after the previous task's. Its code uses none of the
other slices' symbols. Its one cross-slice contact is `tasks lanes` (Slice 3). That command
flattens `ScopeArgs`, so it gains `--group` with no code change; Task 4.4 adds the `lanes`
row's `--group` option to its own ops rows and tests `lanes --group`.

**Goal:** a host can name a set of registered projects (`tasks group set vf nodes atoms`)
and read it with `--group vf` wherever `--project` and `--all-projects` are accepted, plus
`quiet`. The registry stays consistent through `init`, `rename`, `rename --adopt` and
`unregister`.

**Architecture:** `Registry.groups` (a `[groups]` table in `projects.toml`) is validated on
load beside the alias invariants. Every registry mutation keeps it valid:
- `register`/`repoint` refuse a group's name;
- `rename`/`adopt` retarget members;
- `unregister` prunes members and deletes emptied groups.

`Scope::All` becomes a struct variant carrying the requested `members` and the `group`
name. `open_read_ctx` opens a group through the same walk as `--all-projects`, restricted
to its members. Views that warn about missing projects (`open_all`'s walk,
`halt_snapshots`) iterate `members`, not the registry.

### Global constraints (this slice)

- Run tests only through `just test-one …` and `just test-fast`; never `cargo test`.
  `just check` (fmt, clippy `-D warnings`, `tasks check`) before every commit. `cargo fmt`
  may be run directly.
- JSON changes are additive:
  - `prime.group`, present only under `--group`;
  - `unregister` warnings;
  - the new `groups` and `group set|rm` payloads.
- Every new command and option is a row in the CLI inventory, and
  `surface::tests::parser_surface_equals_table` fails until the adopted rows match the
  parser. The rows land in ops first. Each CLI-touching task (4.2, 4.4, 4.5) keeps its
  exact edit to ops `cli.toml` in a block titled **"Ops rows (landed by Task 0.1)"**. Task 0.1
  lands that edit as one ops commit per task, in execution order, and records
  `inventory: Task <n> = <sha>` on tasks-ece1e2. The task itself never edits ops. It adopts
  with `python3 "$OPS/bin/vendored" adopt cli.toml --at <sha>`, run in
  `.worktrees/ece1e2-work-selection`. `vendored` also refreshes `tools/cli_surface.py` when
  the pair changed. The task then commits `tools/cli.toml` (and `tools/cli_surface.py`, if
  `git status` shows it changed) with its implementation. Never write `tools/cli.toml` by
  hand or with `cp`. `$OPS` is the `ops` root from `tasks projects`. The adopt step reads
  both values with the same three lines in every task:

  ```bash
  OPS=$(tasks projects | jq -r '.projects[] | select(.prefix == "ops") | .root')
  SHA=$(tasks show tasks-ece1e2 | jq -r '.task.notes[].text | capture("^inventory: Task 4\\.2 = (?<sha>[0-9a-f]{7,40})$").sha' | tail -n 1)
  test -n "$OPS" && test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
  ```

  Each task's copy of these lines names its own task number in the `capture` pattern.
  `--at` adopts every row landed up to that commit, so the tasks run in the ops commits'
  order: Slice 3's last CLI task, then 4.2, 4.4, 4.5.
- Line numbers in this slice's **Files** lists and steps are from the tree before Slice 1.
  Slices 1–3 shift them, so find each edit by the symbol named beside it. No Slice 1–3 code
  matches `Scope::All` by its tuple form, so the variant change in Task 4.4 reaches only the
  sites listed there.
- Group names use the tag grammar: non-empty, only `a-z`, `0-9`, `-`. This is the same
  character rule as Slice 1's need names. Each slice has its own predicate, so neither
  depends on the other.

### Decisions this slice makes where the spec is silent

- **Load validation.** `load` also validates each group's name: grammar, and no equality
  with a live or retired prefix. It also rejects an empty member list. All of these are
  `config` errors, so the invariant `set` enforces also holds for a hand-edited registry.
- **`group set` errors.** Bad or colliding names are `validation`. An unknown member is
  `config`, as `--project <unknown>` is.
- **Rename and adopt targets.** `rename` and `rename --adopt` refuse a target prefix equal
  to a group name. From `rename` this is refusal **R12** of its recovery classification.
  R12 applies with or without an inventory, as R7–R8 do, so `--explain` reports `refuse`
  exactly when execution refuses. Execution refuses with `config`, as for a retired target,
  at any stage and before any write. From adopt it is `validation`, raised by its pre-write
  classification, which is adopt's existing refusal kind. A name must never mean two things,
  so `is_taken` now includes group names too.
- **Rename reservations.** `group set` refuses a name that an unfinished rename reserves.
  That is the source or target of any pending inventory, the same pair `rename` refuses for
  a second rename. The check reads the inventories under the registry lock, as `init`'s
  check does. A rename writes its inventory while it holds that lock, so no reservation can
  appear between the check and the save. The refusal is `validation`, because `group set`
  makes every name collision `validation`. Rejected alternative: `config`, which is what
  `init` returns. A group name colliding with a reservation is the same mistake as one
  colliding with a prefix, so it gets the same kind.
- **Stored members.** Members are stored sorted and distinct. Scope order is registry
  (prefix) order, so nothing depends on the declared order.

### File Structure

- `src/registry.rs` — modify: `groups` field, load validation, `set_group`, `remove_group`,
  `group`, `refuse_group_name`, `retarget_groups`, `Unregistered`; `register`, `repoint`,
  `rename`, `adopt`, `unregister`, `is_taken` keep groups consistent; unit tests.
- `src/error.rs` — modify: `Error::UnknownGroup` → kind `unknown_group`.
- `src/commands/group.rs` — create: `set`, `rm`, `list` (`tasks group set|rm`, `tasks groups`).
- `src/commands/mod.rs` — modify: `pub mod group;`, dispatch, `open_read_ctx` and
  `open_id_read_ctx` accept `--group`, `quiet --group`.
- `src/commands/unregister.rs` — modify: warn per deleted group.
- `src/rename/adopt.rs` — modify: `classify_registry` refuses a group-named target.
- `src/rename/snapshot.rs` — modify: `RegistryState.target_group`, observed by `observe`;
  unit test.
- `src/rename/classify.rs` — modify: refusal R12 (target names a group); its unit tests
  and the independent transcription `expected`.
- `src/rename/mod.rs` — modify: an R12 refusal is `config`.
- `src/cli.rs` — modify: `ScopeArgs.group`, `Command::Group`/`GroupAction`,
  `Command::Groups`, `Tree` id conflict, `Quiet.group`.
- `src/scope.rs` — modify: `Scope::All { projects, members, group }`, `open_group`,
  `group()`; unit test.
- `src/commands/list.rs` — modify: struct-variant matches, `halt_snapshots` warns for
  members only, `prime` sets `group`.
- `src/commands/tags.rs` — modify: struct-variant match.
- `src/output.rs` — modify: `GroupOut`, `GroupRow`, `GroupMember`, `GroupsOut`,
  `PrimeOut.group`, pretty rendering, `warnings_of`.
- `src/complete.rs` — modify: `groups()` candidates, `Line.group`, `scoped` under `--group`.
- `tools/cli.toml` (and `tools/cli_surface.py` when `vendored` refreshes it) — adopted with
  `vendored adopt --at`, never edited. The rows are landed in ops by Task 0.1: `group`,
  `group set`, `group rm` and `groups` (Task 4.2); `--group` on list, ready, next, sample,
  prime, tree, tags and lanes (Task 4.4); and `--group` on quiet (Task 4.5).
- `tests/cli.rs` — modify: append the end-to-end group tests.
- `skills/tasks/SKILL.md`, `README.md`, `docs/specs/2026-08-29-tasks-design.md`,
  `docs/specs/2026-09-08-prefix-rename-design.md` — modify (Task 4.6).

---

### Task 4.1: The `[groups]` registry table and its load invariants

**Files:**
- Modify: `src/registry.rs:7-15` (struct), `src/registry.rs:59-73` (load validation), add
  `group_name_problem` and `is_valid_group_name`; tests module `src/registry.rs:246-419`.
- Modify: `tests/cli.rs` (append one test).

**Interfaces:**
- Consumes: `Registry::load_from(&Path) -> Result<Registry>`, `Error::Config`.
- Produces:
  - `pub groups: BTreeMap<String, Vec<String>>` on `Registry`;
  - `fn is_valid_group_name(name: &str) -> bool`, private to `registry.rs`;
  - `fn group_name_problem(&self, name: &str) -> Option<String>`, private.

- [ ] **Step 1: Write the failing unit tests** (append inside `mod tests` in `src/registry.rs`)

```rust
    #[test]
    fn groups_round_trip_and_an_empty_table_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.save_to(&path).unwrap();
        assert!(
            !std::fs::read_to_string(&path).unwrap().contains("[groups]"),
            "a registry without groups keeps its shape"
        );
        r.groups.insert("vf".into(), vec!["sci".into()]);
        r.save_to(&path).unwrap();
        assert_eq!(Registry::load_from(&path).unwrap().groups, r.groups);
    }

    #[test]
    fn load_rejects_an_invalid_group() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let base = "[projects]\nsci = \"/tmp/a\"\n\n[aliases]\nold = \"sci\"\n\n[groups]\n";
        for (groups, named) in [
            ("mix = [\"sci\", \"gone\"]\n", ["\"mix\"", "\"gone\""]),
            ("vf = [\"old\"]\n", ["\"vf\"", "\"old\""]),
            ("empty = []\n", ["\"empty\"", "no members"]),
            ("Bad = [\"sci\"]\n", ["\"Bad\"", "lowercase"]),
            ("sci = [\"sci\"]\n", ["\"sci\"", "registered prefix"]),
            ("old = [\"sci\"]\n", ["\"old\"", "retired prefix"]),
        ] {
            std::fs::write(&path, format!("{base}{groups}")).unwrap();
            let error = Registry::load_from(&path).unwrap_err();
            assert_eq!(error.kind(), "config", "{groups}");
            for word in named {
                assert!(error.to_string().contains(word), "{groups}: {error}");
            }
        }
        std::fs::write(&path, format!("{base}data-2 = [\"sci\"]\n")).unwrap();
        assert_eq!(
            Registry::load_from(&path).unwrap().groups["data-2"],
            ["sci"],
            "digits and - are the tag grammar"
        );
    }
```

- [ ] **Step 2: Write the failing end-to-end test** (append to `tests/cli.rs`)

```rust
#[test]
fn a_registry_naming_an_unregistered_group_member_fails_to_load() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[groups]\nmix = [\"sci\", \"gone\"]\n");
    std::fs::write(&path, text).unwrap();
    let error = error_of(&env, &sci, &["list"]);
    assert_eq!(error["error"]["kind"], "config");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("\"mix\"") && detail.contains("\"gone\""),
        "{detail}"
    );
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one registry::tests::groups_round_trip` — expected: compile error, `no field
groups on type Registry`.
Run: `just test-one --test cli a_registry_naming_an_unregistered_group_member_fails_to_load`
— expected: FAIL, `list` exits 0 (serde ignores the unknown `groups` table).

- [ ] **Step 4: Implement**

In `src/registry.rs`, add the field after `aliases` (lines 11-14):

```rust
    /// Group name -> member prefixes, each a live prefix, sorted and distinct
    /// (docs/specs/2026-10-03-lanes-needs-groups-design.md §6). Host-local like the rest
    /// of the registry. Not written when empty, so a registry without groups keeps its
    /// shape on save.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, Vec<String>>,
```

In `load_from`, after the alias loop (line 72) and before `Ok(registry)`:

```rust
        for (name, members) in &registry.groups {
            if let Some(problem) = registry.group_name_problem(name) {
                return Err(Error::Config(format!("{}: {problem}", path.display())));
            }
            if members.is_empty() {
                return Err(Error::Config(format!(
                    "{}: group {name:?} has no members",
                    path.display()
                )));
            }
            if let Some(member) = members
                .iter()
                .find(|member| !registry.projects.contains_key(*member))
            {
                return Err(Error::Config(format!(
                    "{}: group {name:?} names {member:?}, which is not a registered project; \
                     edit [groups] in this file",
                    path.display()
                )));
            }
        }
```

In `impl Registry`, after `is_taken`:

```rust
    /// Why `name` cannot name a group, if anything. The grammar is checked first. Then the
    /// two namespaces a group name shares: a live prefix and a retired one. `load` reports
    /// a problem as `config`; `set_group` reports it as `validation`.
    fn group_name_problem(&self, name: &str) -> Option<String> {
        if !is_valid_group_name(name) {
            return Some(format!(
                "group name {name:?} must use lowercase letters, digits, and -"
            ));
        }
        if self.projects.contains_key(name) {
            return Some(format!("group name {name:?} is a registered prefix"));
        }
        if let Some(target) = self.aliases.get(name) {
            return Some(format!(
                "group name {name:?} is a retired prefix of {target:?}"
            ));
        }
        None
    }
```

After the `impl Registry` block (before `#[cfg(test)]`):

```rust
/// The tag grammar: non-empty, lowercase ASCII letters, digits, and `-`.
fn is_valid_group_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `just test-one registry::tests` — expected: all PASS.
Run: `just test-one --test cli a_registry_naming_an_unregistered_group_member_fails_to_load` — expected: PASS.

- [ ] **Step 6: Gate**

Run: `cargo fmt && just check && just test-fast` — expected: clean, all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/registry.rs tests/cli.rs
git commit -m "feat(registry): declare project groups in a validated [groups] table"
```

---

### Task 4.2: `tasks group set|rm` and `tasks groups`

**Files:**
- Modify: `src/registry.rs` (add `set_group`, `remove_group`, `unknown_group` after
  `group_name_problem`; tests module).
- Modify: `src/error.rs:121-175` (variant), `:185-219` (`with_suffix`), `:221-250` (`kind`).
- Create: `src/commands/group.rs`.
- Modify: `src/commands/mod.rs:1-22` (module list), `:25` (imports), `:1281-1287` (dispatch beside `Unregister`/`Projects`).
- Modify: `src/cli.rs:284-310` (new variants after `Projects`), and a new `GroupAction` enum after `Command`.
- Modify: `src/output.rs` (structs after `ProjectsOut` at line 117; `Output` enum 831-852; `pretty` 884-890; `warnings_of` 1895-1920).
- Modify: `src/complete.rs:205-211` (add `groups()` after `prefixes()`).
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if it changes), through
  `vendored adopt --at` the Task 4.2 inventory commit. Never edit it.
- Modify: `tests/cli.rs` (append).

**Interfaces:**
- Consumes: `Registry::{load, lock, save, canonical_prefix, group_name_problem}`,
  `scope::{is_reachable, registry_warnings}`, `commands::start_dir`,
  `rename::inventory::Inventory::pending() -> Result<Vec<Inventory>>` (its `source` and
  `target` fields).
- Produces:
  - `Registry::set_group(&mut self, name: &str, members: &[String]) -> Result<Vec<String>>`
  - `Registry::remove_group(&mut self, name: &str) -> Result<Vec<String>>`
  - `Error::UnknownGroup(String)` (kind `unknown_group`)
  - `commands::group::{set(name: String, prefixes: Vec<String>) -> Result<Output>, rm(name: String) -> Result<Output>, list(dir: Option<&Path>) -> Result<Output>}`
  - `output::{GroupOut { name, members: Vec<String>, warnings }, GroupMember { prefix, reachable: bool }, GroupRow { name, members: Vec<GroupMember> }, GroupsOut { groups: Vec<GroupRow>, warnings }}`, `Output::Group`, `Output::Groups`
  - `cli::GroupAction::{Set { name, prefixes }, Rm { name }}`, `Command::Group { action }`, `Command::Groups`
  - `complete::groups() -> Vec<CompletionCandidate>`
  - `fn reject_rename_reservation(name: &str) -> Result<()>`, private to `commands/group.rs`

**Ops rows (landed by Task 0.1):** in ops `cli.toml`, table `cli.tasks`, insert these rows
directly after the `[[cli.tasks.commands]]` row whose `path = ["projects"]`. Other CLIs
have a `projects` row too, so match the one under `cli.tasks`. Insert after the `]` that
closes its `options`.

```toml
[[cli.tasks.commands]]
path = ["group"]
summary = "Named sets of registered projects, read together with --group"

[[cli.tasks.commands]]
path = ["group", "set"]
summary = "Create or replace a group; a retired prefix resolves to its live one"
args = [{ name = "name", value = "string", required = true }, { name = "prefixes", value = "string", required = true, variadic = true }]

[[cli.tasks.commands]]
path = ["group", "rm"]
summary = "Delete a group. Its projects stay registered"
args = [{ name = "name", value = "string", required = true }]

[[cli.tasks.commands]]
path = ["groups"]
summary = "Every group with its members and whether each is reachable"
```

- [ ] **Step 1: Write the failing unit tests** (append in `src/registry.rs` `mod tests`)

```rust
    #[test]
    fn set_group_resolves_aliases_stores_each_member_once_and_validates() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.register("fam", Path::new("/tmp/b")).unwrap();
        r.aliases.insert("old".into(), "fam".into());

        let stored = r
            .set_group("vf", &["sci".into(), "old".into(), "sci".into()])
            .unwrap();
        assert_eq!(stored, ["fam", "sci"]);
        assert_eq!(r.groups["vf"], ["fam", "sci"]);
        assert_eq!(r.set_group("vf", &["sci".into()]).unwrap(), ["sci"]);
        assert_eq!(r.groups["vf"], ["sci"], "set replaces");

        for name in ["", "Bad", "under_score", "fam", "old"] {
            assert_eq!(
                r.set_group(name, &["sci".into()]).unwrap_err().kind(),
                "validation",
                "{name:?}"
            );
        }
        assert_eq!(
            r.set_group("vf", &[]).unwrap_err().kind(),
            "validation",
            "a group needs a member"
        );
        assert_eq!(
            r.set_group("vf", &["nope".into()]).unwrap_err().kind(),
            "config"
        );
        assert_eq!(r.groups["vf"], ["sci"], "a refused set changes nothing");
    }

    #[test]
    fn remove_group_returns_its_members_and_an_unknown_name_is_unknown_group() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.set_group("vf", &["sci".into()]).unwrap();
        assert_eq!(r.remove_group("vf").unwrap(), ["sci"]);
        assert!(r.groups.is_empty());
        assert_eq!(r.remove_group("vf").unwrap_err().kind(), "unknown_group");
    }
```

- [ ] **Step 2: Write the failing end-to-end tests** (append to `tests/cli.rs`)

```rust
#[test]
fn group_set_rm_and_groups_manage_named_project_sets() {
    let mut env = TestEnv::new();
    env.init("sci");
    let fam = env.init("fam");
    alias_registry(&env, "old", "fam");
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.json(nowhere.path(), &["groups"]),
        serde_json::json!({"groups": [], "warnings": []})
    );

    // A retired prefix resolves to its live one, and a repeated prefix is stored once.
    assert_eq!(
        env.json(nowhere.path(), &["group", "set", "vf", "sci", "old", "sci"]),
        serde_json::json!({"name": "vf", "members": ["fam", "sci"], "warnings": []})
    );
    let registry: toml::Value = toml::from_str(
        &std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
    )
    .unwrap();
    let stored: Vec<&str> = registry["groups"]["vf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap())
        .collect();
    assert_eq!(stored, ["fam", "sci"]);

    // set replaces; groups lists every group with each member's reachability.
    env.json(nowhere.path(), &["group", "set", "vf", "sci"]);
    env.json(nowhere.path(), &["group", "set", "pair", "fam", "sci"]);
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.json(nowhere.path(), &["groups"])["groups"],
        serde_json::json!([
            {"name": "pair", "members": [
                {"prefix": "fam", "reachable": false},
                {"prefix": "sci", "reachable": true},
            ]},
            {"name": "vf", "members": [{"prefix": "sci", "reachable": true}]},
        ])
    );
    assert_eq!(
        env.pretty(nowhere.path(), &["groups"]).trim_end(),
        "pair  fam (unreachable), sci\nvf  sci"
    );

    let removed = env.json(nowhere.path(), &["group", "rm", "pair"]);
    assert_eq!(removed["members"], serde_json::json!(["fam", "sci"]));
    assert_eq!(
        env.fail(nowhere.path(), &["group", "rm", "pair"]),
        "unknown_group"
    );
    assert_eq!(env.pretty(nowhere.path(), &["group", "rm", "vf"]).trim(), "vf");
    assert_eq!(
        env.json(nowhere.path(), &["groups"])["groups"],
        serde_json::json!([])
    );
}

#[test]
fn group_set_refuses_bad_or_colliding_names_and_unknown_members() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    alias_registry(&env, "old", "fam");
    for name in ["", "Bad", "under_score", "sp ace"] {
        assert_eq!(
            env.fail(&sci, &["group", "set", name, "sci"]),
            "validation",
            "{name:?}"
        );
    }
    assert_eq!(
        env.fail(&sci, &["group", "set", "fam", "sci"]),
        "validation",
        "a live prefix"
    );
    assert_eq!(
        env.fail(&sci, &["group", "set", "old", "sci"]),
        "validation",
        "a retired prefix"
    );
    assert_eq!(env.fail(&sci, &["group", "set", "vf", "sci", "nope"]), "config");
    env.usage(&sci, &["group", "set", "vf"]);
    assert_eq!(env.json(&sci, &["groups"])["groups"], serde_json::json!([]));
    env.json(&sci, &["group", "set", "data-2", "sci"]);
}

#[test]
fn group_set_refuses_a_name_an_unfinished_rename_reserves() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&sci, &["add", "S", "-p", "2"]));
    let stopped = env
        .raw(&sci)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "lab"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");

    // `lab` is not yet a prefix, so only the reservation stands between it and a group.
    // A group there would make the resume rewrite files and config, then fail at the
    // registry step on the collision.
    let error = error_of(&env, &fam, &["group", "set", "lab", "fam"]);
    assert_eq!(error["error"]["kind"], "validation");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("reserved by unfinished rename sci -> lab"),
        "{detail}"
    );
    assert_eq!(env.json(&fam, &["groups"])["groups"], serde_json::json!([]));

    // The name stayed free, so the rename resumes and finishes.
    assert_eq!(
        env.json(&sci, &["rename", "sci", "lab"])["recovery"],
        "resume_files"
    );
    assert!(sci.join(format!("tasks/lab-{}.md", &id[4..])).is_file());
    // Finished, `lab` is a live prefix and `sci` a retired one: still refused, now as
    // prefixes rather than reservations.
    for name in ["lab", "sci"] {
        assert_eq!(
            env.fail(&fam, &["group", "set", name, "fam"]),
            "validation",
            "{name}"
        );
    }
    env.json(&fam, &["group", "set", "vf", "lab", "fam"]);
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one registry::tests::set_group` — expected: compile error, `no method named
set_group`.
Run: `just test-one --test cli group_set_` — expected: the three tests FAIL, `groups`/`group` are
unrecognized subcommands (exit 2).

- [ ] **Step 4: Implement the registry methods and the error kind**

`src/registry.rs`: change the import to `use std::collections::{BTreeMap, BTreeSet};` and
add after `group_name_problem`:

```rust
    /// Creates or replaces group `name`. A retired prefix resolves to its live one, and
    /// each member is stored once, in prefix order. Returns the stored members.
    pub fn set_group(&mut self, name: &str, members: &[String]) -> Result<Vec<String>> {
        if let Some(problem) = self.group_name_problem(name) {
            return Err(Error::Validation(problem));
        }
        if members.is_empty() {
            return Err(Error::Validation(format!(
                "group {name:?} needs at least one member"
            )));
        }
        let mut resolved = BTreeSet::new();
        for member in members {
            let live = self.canonical_prefix(member);
            if !self.projects.contains_key(live) {
                return Err(Error::Config(format!(
                    "no project registered as {member:?}"
                )));
            }
            resolved.insert(live.to_string());
        }
        let stored: Vec<String> = resolved.into_iter().collect();
        self.groups.insert(name.into(), stored.clone());
        Ok(stored)
    }

    /// Deletes group `name`, returning the members it had. Its projects stay registered.
    pub fn remove_group(&mut self, name: &str) -> Result<Vec<String>> {
        self.groups.remove(name).ok_or_else(|| unknown_group(name))
    }
```

and after `is_valid_group_name`:

```rust
fn unknown_group(name: &str) -> Error {
    Error::UnknownGroup(format!("no group named {name:?}; `tasks groups` lists them"))
}
```

`src/error.rs`: add the variant after `Halted(String)` (line 156):

```rust
    #[error("{0}")]
    UnknownGroup(String),
```

In `with_suffix`, after the `Error::Halted` arm:

```rust
            Error::UnknownGroup(detail) => Error::UnknownGroup(detail + suffix),
```

In `kind`, after `Error::Halted(_) => "halted",`:

```rust
            Error::UnknownGroup(_) => "unknown_group",
```

- [ ] **Step 5: Implement the payloads**

`src/output.rs`, after `ProjectsOut` (line 117):

```rust
/// `group set` and `group rm`: the group, and the members it now has (set) or had (rm).
#[derive(Serialize)]
pub struct GroupOut {
    pub name: String,
    pub members: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct GroupMember {
    pub prefix: String,
    /// The registered root exists and holds a config: the test `projects` applies.
    pub reachable: bool,
}

#[derive(Serialize)]
pub struct GroupRow {
    pub name: String,
    pub members: Vec<GroupMember>,
}

#[derive(Serialize)]
pub struct GroupsOut {
    pub groups: Vec<GroupRow>,
    pub warnings: Vec<String>,
}
```

Add `Group(GroupOut),` and `Groups(GroupsOut),` to `enum Output` after `Projects(ProjectsOut),`.
In `pretty`, after `Output::Root(o) => o.root.clone(),`:

```rust
        Output::Group(o) => o.name.clone(),
        Output::Groups(o) => o
            .groups
            .iter()
            .map(|row| {
                let members: Vec<String> = row
                    .members
                    .iter()
                    .map(|member| {
                        if member.reachable {
                            member.prefix.clone()
                        } else {
                            format!("{} (unreachable)", member.prefix)
                        }
                    })
                    .collect();
                format!("{}  {}", row.name, members.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
```

In `warnings_of`, after `Output::Root(o) => o.warnings.clone(),`:

```rust
        Output::Group(o) => o.warnings.clone(),
        Output::Groups(o) => o.warnings.clone(),
```

- [ ] **Step 6: Implement the commands**

Create `src/commands/group.rs`:

```rust
//! `tasks group set|rm` and `tasks groups`: named sets of registered projects
//! (docs/specs/2026-10-03-lanes-needs-groups-design.md §6). Registry-only, like
//! `unregister`: no project context, and the registry lock around every write.

use crate::error::{Error, Result};
use crate::output::{GroupMember, GroupOut, GroupRow, GroupsOut, Output};
use crate::registry::Registry;
use crate::rename::inventory::Inventory;
use crate::scope::{is_reachable, registry_warnings};
use std::path::Path;

pub fn set(name: String, prefixes: Vec<String>) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    reject_rename_reservation(&name)?;
    let members = registry.set_group(&name, &prefixes)?;
    registry.save()?;
    Ok(Output::Group(GroupOut {
        name,
        members,
        warnings: Vec::new(),
    }))
}

/// A name an unfinished rename reserves (its source or its target) stays free until that
/// rename finishes, as `init` and `rename` keep it. A group under the target would let the
/// resume rewrite files and config, then fail at the registry step. The caller holds the
/// registry lock, and a rename writes its inventory under that lock, so no reservation can
/// appear between this check and the save. `validation`, as every colliding group name is.
fn reject_rename_reservation(name: &str) -> Result<()> {
    for pending in Inventory::pending()? {
        if pending.source == name || pending.target == name {
            return Err(Error::Validation(format!(
                "group name {name:?} is reserved by unfinished rename {} -> {}; \
                 finish it with `tasks rename {} {}` first",
                pending.source, pending.target, pending.source, pending.target
            )));
        }
    }
    Ok(())
}

pub fn rm(name: String) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    let members = registry.remove_group(&name)?;
    registry.save()?;
    Ok(Output::Group(GroupOut {
        name,
        members,
        warnings: Vec::new(),
    }))
}

/// Every group in name order, each member with the reachability test `projects` uses.
pub fn list(dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut groups = Vec::new();
    for (name, members) in &registry.groups {
        let mut rows = Vec::new();
        for prefix in members {
            // `load` guarantees every member is registered; say so if that ever breaks.
            let root = registry.project_root(prefix).ok_or_else(|| {
                Error::Config(format!(
                    "group {name:?} names {prefix:?}, which is not a registered project"
                ))
            })?;
            rows.push(GroupMember {
                prefix: prefix.clone(),
                reachable: is_reachable(root)?,
            });
        }
        groups.push(GroupRow {
            name: name.clone(),
            members: rows,
        });
    }
    Ok(Output::Groups(GroupsOut { groups, warnings }))
}
```

`src/commands/mod.rs`: add `pub mod group;` after `pub mod graph;`. Change line 25 to
`use crate::cli::{Cli, Command, FieldArgs, GroupAction, ScopeArgs};`. In `run`, after the
`Command::Projects { .. } => projects::run(..),` arm:

```rust
        Command::Group { action } => match action {
            GroupAction::Set { name, prefixes } => group::set(name, prefixes),
            GroupAction::Rm { name } => group::rm(name),
        },
        Command::Groups => group::list(dir),
```

- [ ] **Step 7: Implement the CLI and completion**

`src/cli.rs`: after the `Projects { .. },` variant (ends line 310):

```rust
    /// Named sets of registered projects, read together with --group.
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Every group with its members and whether each is reachable.
    Groups,
```

After the closing brace of `enum Command`:

```rust
#[derive(Subcommand, Debug)]
pub enum GroupAction {
    /// Create or replace a group; a retired prefix resolves to its live one.
    Set {
        /// Lowercase letters, digits, and -; not a registered or retired prefix.
        name: String,
        #[arg(required = true, add = ArgValueCandidates::new(crate::complete::prefixes))]
        prefixes: Vec<String>,
    },
    /// Delete a group. Its projects stay registered.
    Rm {
        #[arg(add = ArgValueCandidates::new(crate::complete::groups))]
        name: String,
    },
}
```

`src/complete.rs`, after `prefixes()`:

```rust
/// Declared group names, in name order. An unreadable registry offers nothing.
pub fn groups() -> Vec<CompletionCandidate> {
    let Ok(registry) = Registry::load() else {
        return Vec::new();
    };
    plain(registry.groups.keys().cloned().collect::<Vec<_>>())
}
```

- [ ] **Step 8: Adopt this task's rows**

In `.worktrees/ece1e2-work-selection` (this also refreshes `tools/cli_surface.py` when the
pair changed):

```bash
OPS=$(tasks projects | jq -r '.projects[] | select(.prefix == "ops") | .root')
SHA=$(tasks show tasks-ece1e2 | jq -r '.task.notes[].text | capture("^inventory: Task 4\\.2 = (?<sha>[0-9a-f]{7,40})$").sha' | tail -n 1)
test -n "$OPS" && test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
```

Run: `just test-one parser_surface_equals_table` — expected: PASS. A mismatch means the
parser disagrees with the landed rows. Fix the parser to match. Never edit
`tools/cli.toml`: a row that is wrong goes back to Task 0.1 as a new ops commit.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `just test-one registry::tests` — expected: PASS.
Run: `just test-one --test cli group_set_` — expected: all three PASS.

- [ ] **Step 10: Gate**

Run: `cargo fmt && just check && just test-fast` — expected: clean, all pass.

- [ ] **Step 11: Commit**

```bash
git add src/registry.rs src/error.rs src/commands/group.rs src/commands/mod.rs src/cli.rs \
  src/output.rs src/complete.rs tools/cli.toml tests/cli.rs
git status --short tools/cli_surface.py | grep -q . && git add tools/cli_surface.py
git commit -m "feat(group): tasks group set|rm and tasks groups manage project groups"
```

---

### Task 4.3: Groups stay consistent through init, rename, adopt and unregister

**Files:**
- Modify: `src/registry.rs`:
  - `register` 94-112, `repoint` 116-126, `unregister` 128-151, `rename` 153-172,
    `adopt` 174-215, `is_taken` 239-243;
  - the unit test `unregister_removes_once_and_then_reports_the_prefix_is_absent` at
    331-347;
  - new tests.
- Modify: `src/commands/unregister.rs:1-20`.
- Modify: `src/rename/adopt.rs:194-204` (`classify_registry`, after the retired-target refusal).
- Modify: `src/rename/snapshot.rs:39-44` (`RegistryState`), `:195-207` (`observe`), tests
  module (new test after `observes_registry_config_and_absent_config_explicitly`).
- Modify: `src/rename/classify.rs:192-210` (`refusal`, after R8), tests `:276-280`
  (`inventory_only`), `:328-332`, `:350-390` (`each_refusal_fires_on_its_own_before_the_table`),
  `:436` (`expected`), `:584-588` (`enumerate_snapshots`).
- Modify: `src/rename/mod.rs:48-55` (the error kind of a refusal).
- Modify: `tests/cli.rs` (append).

**Interfaces:**
- Consumes: `Registry::set_group` (Task 4.2), `groups` (Task 4.1).
- Produces:
  - `pub struct Unregistered { pub root: PathBuf, pub aliases: Vec<String>, pub emptied_groups: Vec<String> }`;
  - `Registry::unregister(&mut self, prefix: &str) -> Result<Unregistered>`, which
    replaces the `(PathBuf, Vec<String>)` return;
  - `fn refuse_group_name(&self, prefix: &str) -> Result<()>`, private;
  - `fn retarget_groups(&mut self, from: &str, to: &str)`, private;
  - `is_taken` now true for a group name;
  - `pub target_group: bool` on `rename::snapshot::RegistryState`, true when the rename
    target names a group;
  - refusal `R12` from `rename::classify::classify` whenever `target_group` holds, with or
    without an inventory. `rename` turns it into `config`, and `--explain` reports
    `recovery: "refuse"` with the R12 reason as its first warning.

- [ ] **Step 1: Write the failing unit tests** (append in `src/registry.rs` `mod tests`)

```rust
    #[test]
    fn a_group_name_cannot_become_a_prefix() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.set_group("vf", &["sci".into()]).unwrap();
        for error in [
            r.register("vf", Path::new("/tmp/b")).unwrap_err(),
            r.repoint("vf", Path::new("/tmp/b")).unwrap_err(),
        ] {
            assert_eq!(error.kind(), "config");
            assert!(error.to_string().contains("tasks group rm vf"), "{error}");
        }
        assert!(r.is_taken("vf"), "a group's name is taken");
        assert_eq!(r.rename("sci", "vf").unwrap_err().kind(), "config");
        assert!(r.project_root("vf").is_none());
        assert_eq!(r.groups["vf"], ["sci"]);
    }

    #[test]
    fn rename_and_adopt_carry_group_membership() {
        let mut r = Registry::default();
        r.register("dot", Path::new("/tmp/d")).unwrap();
        r.register("ops", Path::new("/tmp/o")).unwrap();
        r.set_group("vf", &["dot".into(), "ops".into()]).unwrap();
        r.rename("dot", "dots").unwrap();
        assert_eq!(r.groups["vf"], ["dots", "ops"]);

        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("new");
        std::fs::create_dir(&root).unwrap();
        for partial_init in [false, true] {
            let mut r = Registry::default();
            r.register("old", &home.path().join("missing")).unwrap();
            let mut members = vec!["old".to_string()];
            if partial_init {
                r.register("new", &root.join(".")).unwrap();
                members.push("new".into());
            }
            r.set_group("vf", &members).unwrap();
            r.adopt("old", "new", &root).unwrap();
            assert_eq!(r.groups["vf"], ["new"], "partial_init={partial_init}");
        }

        let mut r = Registry::default();
        r.register("old", &home.path().join("missing")).unwrap();
        r.set_group("new", &["old".into()]).unwrap();
        assert_eq!(r.adopt("old", "new", &root).unwrap_err().kind(), "config");
        assert!(r.project_root("old").is_some(), "a refused adopt changes nothing");
    }

    #[test]
    fn unregister_prunes_groups_and_deletes_the_ones_it_empties() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.register("fam", Path::new("/tmp/b")).unwrap();
        r.set_group("pair", &["sci".into(), "fam".into()]).unwrap();
        r.set_group("solo", &["fam".into()]).unwrap();
        let removed = r.unregister("fam").unwrap();
        assert_eq!(removed.emptied_groups, ["solo"]);
        assert_eq!(r.groups.keys().collect::<Vec<_>>(), ["pair"]);
        assert_eq!(r.groups["pair"], ["sci"]);
    }
```

Update the existing `unregister_removes_once_and_then_reports_the_prefix_is_absent`
(lines 335-338) to the new return type:

```rust
        assert_eq!(
            r.unregister("sci").unwrap(),
            Unregistered {
                root: PathBuf::from("/tmp/a"),
                aliases: Vec::new(),
                emptied_groups: Vec::new(),
            }
        );
```

In `src/rename/snapshot.rs` `mod tests`, after
`observes_registry_config_and_absent_config_explicitly`:

```rust
    #[test]
    fn observes_whether_the_target_names_a_group() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.projects.insert("dot".into(), project.root.clone());
        let observed = |registry: &crate::registry::Registry| {
            observe(registry, &invocation(&project), None)
                .unwrap()
                .registry
                .target_group
        };
        assert!(!observed(&registry));
        registry.groups.insert("dot-set".into(), vec!["dot".into()]);
        assert!(!observed(&registry), "only a group named like the target counts");
        registry.groups.insert("dots".into(), vec!["dot".into()]);
        assert!(observed(&registry));
    }
```

In `src/rename/classify.rs` `mod tests`, extend
`each_refusal_fires_on_its_own_before_the_table`. Change `for rule in 1..=11 {` to
`for rule in 1..=12 {`, and add the arm before `_ => unreachable!(),`:

```rust
                12 => snap.registry.target_group = true,
```

R12 reads the registry alone, like R7 and R8. So change the no-inventory recheck's guard
`if (7..=8).contains(&rule) {` to:

```rust
            if matches!(rule, 7 | 8 | 12) {
```

In the same module, give each of the three `RegistryState { .. }` literals (in
`inventory_only`, in `every_phase_boundary_resumes_rather_than_refusing`, and in
`enumerate_snapshots`) the field `target_group: false,` after `alias`. The enumeration
keeps `false`, so its 1,572,480 count is unchanged. The independent transcription
`expected` must still model R12. Change its first `excluded` line to:

```rust
        let mut excluded = two_roots || foreign_target || registry.target_group;
```

- [ ] **Step 2: Write the failing end-to-end tests** (append to `tests/cli.rs`)

```rust
#[test]
fn init_refuses_a_prefix_that_names_a_group() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["group", "set", "vf", "sci"]);
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(env.fail(fresh.path(), &["init", "--prefix", "vf"]), "config");
    assert_eq!(
        env.fail(fresh.path(), &["init", "--prefix", "vf", "--force"]),
        "config"
    );
    assert!(!fresh.path().join("tasks").exists(), "a refused init writes nothing");
}

#[test]
fn rename_rewrites_group_members_and_refuses_a_group_name_as_target() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    env.json(&sci, &["group", "set", "vf", "sci", "fam"]);
    env.json(&sci, &["group", "set", "science", "fam"]);
    assert_eq!(env.fail(&sci, &["rename", "sci", "science"]), "config");
    env.json(&sci, &["rename", "sci", "lab"]);
    assert_eq!(
        env.json(&sci, &["groups"])["groups"][1],
        serde_json::json!({"name": "vf", "members": [
            {"prefix": "fam", "reachable": true},
            {"prefix": "lab", "reachable": true},
        ]})
    );
    // The retired name resolves on set, as any alias does.
    assert_eq!(
        env.json(&sci, &["group", "set", "vf", "sci"])["members"],
        serde_json::json!(["lab"])
    );
}

#[test]
fn unregister_removes_the_prefix_from_groups_and_deletes_emptied_ones() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    env.json(&sci, &["group", "set", "pair", "sci", "fam"]);
    env.json(&sci, &["group", "set", "solo", "fam"]);
    let out = env.json(&sci, &["unregister", "fam"]);
    assert_eq!(
        out["warnings"],
        serde_json::json!(["group solo lost its last member fam and was deleted"])
    );
    assert_eq!(
        env.json(&sci, &["groups"])["groups"],
        serde_json::json!([{"name": "pair", "members": [{"prefix": "sci", "reachable": true}]}])
    );
    // The saved registry loads: no dangling member.
    assert_eq!(env.json(&sci, &["list"])["tasks"], serde_json::json!([]));
}

#[test]
fn adopt_carries_group_membership_and_refuses_a_group_named_target() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    adopt_json(&env, &dir, &["group", "set", "new", "old"]);
    let refused = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    let error: serde_json::Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"].as_str().unwrap().contains("group"),
        "{error}"
    );

    adopt_json(&env, &dir, &["group", "rm", "new"]);
    adopt_json(&env, &dir, &["group", "set", "vf", "old"]);
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
    assert_eq!(
        adopt_json(&env, &dir, &["groups"])["groups"],
        serde_json::json!([{"name": "vf", "members": [{"prefix": "new", "reachable": true}]}])
    );
}

#[test]
fn rename_explain_and_execution_both_refuse_a_group_named_target() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&sci, &["add", "S", "-p", "2"]));
    env.json(&fam, &["group", "set", "vf", "fam"]);
    let state = env.home.path().join(".local/state/tasks/rename");

    // Fresh: explain predicts the refusal execution gives.
    let explained = env.json(&sci, &["rename", "sci", "vf", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"][0].as_str().unwrap().starts_with("R12:"),
        "{explained}"
    );
    assert_eq!(env.fail(&sci, &["rename", "sci", "vf"]), "config");
    assert!(!state.join("sci.toml").exists(), "refused before the inventory");

    // Mid-rename: a group that appears under the pending target blocks the resume before
    // any file moves. `group set` refuses the reserved name (Task 4.2), so only a hand
    // edit of the registry gets there.
    let stopped = env
        .raw(&sci)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "lab"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[groups]\n"), "{text}");
    std::fs::write(
        &path,
        text.replace("[groups]\n", "[groups]\nlab = [\"fam\"]\n"),
    )
    .unwrap();
    let explained = env.json(&sci, &["rename", "sci", "lab", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"][0].as_str().unwrap().starts_with("R12:"),
        "{explained}"
    );
    assert_eq!(env.fail(&sci, &["rename", "sci", "lab"]), "config");
    assert!(sci.join(format!("tasks/{id}.md")).is_file(), "no task file moved");
    assert!(!sci.join(format!("tasks/lab-{}.md", &id[4..])).exists());
    assert!(state.join("sci.toml").is_file(), "the inventory waits for the resume");

    env.json(&fam, &["group", "rm", "lab"]);
    assert_eq!(
        env.json(&sci, &["rename", "sci", "lab"])["recovery"],
        "resume_files"
    );
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one registry::tests` — expected: compile error, `cannot find struct
Unregistered`.
Run: `just test-one rename::` — expected: compile error, `no field target_group on type
RegistryState`.
Run: `just test-one --test cli group` — expected: the four new group tests FAIL. `init --prefix vf`
succeeds. `rename sci science` succeeds. `unregister` reports no warning, and the next
`list` is a `config` error naming `"fam"`. `adopt` refuses nothing.
Run: `just test-one --test cli rename_explain_and_execution_both_refuse_a_group_named_target`
— expected: FAIL at the first `recovery` assertion: `--explain` reports `"fresh"`, because
the group check (`is_taken`) runs only on the execution path, after explain has returned.

- [ ] **Step 4: Implement in `src/registry.rs`**

Before `impl Registry`:

```rust
/// What `unregister` removed with the project.
#[derive(Debug, PartialEq)]
pub struct Unregistered {
    pub root: PathBuf,
    /// Retired prefixes that resolved to the project.
    pub aliases: Vec<String>,
    /// Groups the project was the last member of, deleted with it.
    pub emptied_groups: Vec<String>,
}
```

First line of `register` and of `repoint` (before the alias check):

```rust
        self.refuse_group_name(prefix)?;
```

Replace `unregister` (128-151):

```rust
    /// Removes a project, every alias that targeted it, and its membership in every group.
    /// A group it leaves empty is deleted with it and reported. Leaving an alias or a
    /// member behind would break the load invariants and fail every later command. Once
    /// the project is gone its ids cannot resolve anyway, alias or not.
    pub fn unregister(&mut self, prefix: &str) -> Result<Unregistered> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "{prefix:?} is a retired prefix of {target:?}; unregister {target:?} to remove the project"
            )));
        }
        let root = self
            .projects
            .remove(prefix)
            .ok_or_else(|| Error::Config(format!("no project registered as {prefix:?}")))?;
        let aliases: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, target)| target.as_str() == prefix)
            .map(|(alias, _)| alias.clone())
            .collect();
        for alias in &aliases {
            self.aliases.remove(alias);
        }
        let mut emptied_groups = Vec::new();
        self.groups.retain(|name, members| {
            members.retain(|member| member != prefix);
            if members.is_empty() {
                emptied_groups.push(name.clone());
            }
            !members.is_empty()
        });
        Ok(Unregistered {
            root,
            aliases,
            emptied_groups,
        })
    }
```

In `rename`, after the `for live in self.aliases.values_mut()` loop (line 169):

```rust
        self.retarget_groups(source, target);
```

In `adopt`, after the retired-target refusal (line 188):

```rust
        if self.groups.contains_key(target) {
            return Err(Error::Config(format!(
                "target prefix {target:?} is the name of a group; remove it with \
                 `tasks group rm {target}` first"
            )));
        }
```

and in the `if self.projects.contains_key(target)` branch, after its alias loop (line 208):

```rust
            self.retarget_groups(source, target);
```

Replace `is_taken` (239-243):

```rust
    /// Whether a prefix may be claimed. A live prefix, a retired one and a group's name
    /// are all taken, because a name must never mean two things.
    pub fn is_taken(&self, prefix: &str) -> bool {
        self.projects.contains_key(prefix)
            || self.aliases.contains_key(prefix)
            || self.groups.contains_key(prefix)
    }
```

After `remove_group`:

```rust
    /// `init` and `register` never claim a group's name as a prefix.
    fn refuse_group_name(&self, prefix: &str) -> Result<()> {
        if self.groups.contains_key(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is the name of a group; remove it with \
                 `tasks group rm {prefix}` or choose another prefix"
            )));
        }
        Ok(())
    }

    /// Points every group's `from` member at `to`, keeping each member once and in order.
    fn retarget_groups(&mut self, from: &str, to: &str) {
        for members in self.groups.values_mut() {
            for member in members.iter_mut() {
                if member.as_str() == from {
                    *member = to.into();
                }
            }
            members.sort();
            members.dedup();
        }
    }
```

- [ ] **Step 5: Implement the callers**

Replace the body of `run` in `src/commands/unregister.rs` (lines 7-19):

```rust
pub fn run(prefix: String) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    let root = registry.project_root(registry.canonical_prefix(&prefix));
    super::reject_pending_rename_at(root, &prefix)?;
    let removed = registry.unregister(&prefix)?;
    registry.save()?;
    let warnings = removed
        .emptied_groups
        .iter()
        .map(|name| format!("group {name} lost its last member {prefix} and was deleted"))
        .collect();
    Ok(Output::Init(InitOut {
        prefix,
        root: removed.root.display().to_string(),
        warnings,
        aliases: removed.aliases,
    }))
}
```

In `src/rename/adopt.rs` `classify_registry`, after the `if registry.aliases.contains_key(new)`
refusal (line 202-204). Adopt refuses here, before any store is written, rather than in
`Registry::adopt`, which runs after the store stage:

```rust
    if registry.groups.contains_key(new) {
        return Ok(Stage::Refuse(format!(
            "target prefix {new:?} is the name of a group; remove it with `tasks group rm {new}` first"
        )));
    }
```

- [ ] **Step 6: Classify a group-named rename target (R12)**

The fresh-path check `registry.is_taken(&invocation.target)` in `src/rename/mod.rs`
(line 127) now covers group names. It runs only on the execution path, though, and only
for `Fresh`. `--explain` returns before it, and a resume never reaches it. So the group
collision becomes a classification refusal that both paths see.

`src/rename/snapshot.rs`, `RegistryState`, after `alias`:

```rust
    /// The target names a group in the registry: refusal R12, in every stage.
    pub target_group: bool,
```

In `observe`, the `RegistryState` literal gains, after `alias: …,`:

```rust
            target_group: registry.groups.contains_key(&invocation.target),
```

`src/rename/classify.rs`, in `refusal`, after the R8 block and before the final `None`:

```rust
    if snap.registry.target_group {
        return Some(format!(
            "R12: target prefix {:?} is the name of a group; remove it with \
             `tasks group rm {}` first",
            snap.invocation.target, snap.invocation.target
        ));
    }
```

`src/rename/mod.rs`: a refusal's kind becomes `config` when the target is a group, as it
already is for a retired target. Replace the condition at lines 49-51:

```rust
                    if registry.aliases.contains_key(&invocation.target)
                        || registry.groups.contains_key(&invocation.target)
                        || !(registry_old || registry_new)
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `just test-one registry::tests` — expected: PASS.
Run: `just test-one rename::` — expected: PASS, including
`rename::classify::tests::each_refusal_fires_on_its_own_before_the_table` (R12 with and
without an inventory) and `rename::snapshot::tests::observes_whether_the_target_names_a_group`.
Run: `just test-one --bin tasks every_enumerated_snapshot_gets_the_verdict_the_spec_names -- --include-ignored`
— expected: PASS with the 1,572,480 count unchanged (about 7 s).
Run: `just test-one --test cli group` and
`just test-one --test cli rename_explain_and_execution_both_refuse_a_group_named_target` —
expected: PASS.
Run: `just test-one --test cli unregister` and `just test-one --test cli adopt` and
`just test-one --test cli rename` — expected: the existing tests still PASS.

- [ ] **Step 8: Gate**

Run: `cargo fmt && just check && just test-fast` — expected: clean, all pass.

- [ ] **Step 9: Commit**

```bash
git add src/registry.rs src/commands/unregister.rs src/rename/adopt.rs \
  src/rename/snapshot.rs src/rename/classify.rs src/rename/mod.rs tests/cli.rs
git commit -m "feat(registry): keep project groups consistent through init, rename, adopt, and unregister"
```

---

### Task 4.4: `--group` scopes the read views to a group's members

**Files:**
- Modify: `src/registry.rs` (add `group` after `remove_group`).
- Modify: `src/scope.rs:84-122` (`Scope`, `open_all`, `projects`), new
  `open_members`, `open_group` and `group`; tests module `:156-290`.
- Modify: `src/cli.rs:38-53` (`ScopeArgs`), `:660-672` (`Tree` id conflict).
- Modify: `src/commands/mod.rs:497-515` (`open_id_read_ctx`), `:552-593` (`open_read_ctx`), `:1416-1426` (quiet's `ScopeArgs` literal).
- Modify: `src/commands/list.rs:254-263` (`halt_snapshots`), `:569`, `:601-606` (`prime`), `:650` (unit test).
- Modify: `src/commands/tags.rs:10-13`.
- Modify: `src/output.rs:777-797` (`PrimeOut.group`), `:1005-1008` (pretty header).
- Modify: `src/complete.rs:221-230` (`Line.group`), `:375-390` (`record`), `:535-563` (`scoped`), tests `:776-790`.
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if it changes), through
  `vendored adopt --at` the Task 4.4 inventory commit. Never edit it.
- Modify: `tests/cli.rs` (append).

**Interfaces:**
- Consumes: `Registry.groups`, `unknown_group` (Task 4.2), `scope::registry_warnings`,
  `tasks lanes` and its flattened `ScopeArgs` (Slice 3, Task 3.7).
- Produces:
  - `Registry::group(&self, name: &str) -> Result<&[String]>` (`unknown_group` when undeclared)
  - `Scope::All { projects: Vec<Project>, members: Vec<String>, group: Option<String> }`
  - `Scope::open_group(registry: &Registry, cwd: &Path, name: &str) -> Result<(Scope, Vec<String>)>`
  - `Scope::group(&self) -> Option<&str>`
  - `ScopeArgs.group: Option<String>` (`--group <NAME>`, conflicts with `--project` and `--all-projects`)
  - `PrimeOut.group: Option<String>` (JSON `group`, omitted when `None`)
  - `complete::Line.group: Option<String>`

Cross-slice note: Slice 3 has landed, and `tasks lanes` flattens `ScopeArgs`. It gains
`--group` from this task's `ScopeArgs` change with no code of its own, so its row is one of
this task's eight. The end-to-end tests below cover `lanes --group` beside the other read
views.

**Ops rows (landed by Task 0.1):** in ops `cli.toml`, table `cli.tasks`, edit each of the
eight `[[cli.tasks.commands]]` rows `list`, `ready`, `next`, `sample`, `prime`, `tree`,
`tags` and `lanes`. In each row, directly after the line
`  { shared = "all_projects", value = "none" },`, add:

```toml
  { names = ["--group"], value = "string" },
```

Leave `quiet` for Task 4.5, and leave `claims` unchanged.

- [ ] **Step 1: Write the failing unit tests**

In `src/scope.rs` `mod tests`:

```rust
    #[test]
    fn open_group_opens_and_warns_about_members_only() {
        let sci = tempfile::tempdir().unwrap();
        write_config(sci.path(), "sci");
        let gone = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let mut registry = registry_with("sci", sci.path());
        registry.register("fam", gone.path()).unwrap();
        registry.register("ops", other.path()).unwrap();
        registry
            .set_group("vf", &["sci".into(), "fam".into()])
            .unwrap();
        let (scope, warnings) = Scope::open_group(&registry, sci.path(), "vf").unwrap();
        assert_eq!(scope.prefixes(), ["sci"]);
        assert_eq!(scope.group(), Some("vf"));
        assert_eq!(warnings.len(), 1, "ops is unreachable but not asked for: {warnings:?}");
        assert!(warnings[0].starts_with("project fam at "), "{warnings:?}");
        assert!(matches!(
            Scope::open_group(&registry, sci.path(), "nope"),
            Err(Error::UnknownGroup(_))
        ));
        let (all, _) = Scope::open_all(&registry, sci.path()).unwrap();
        assert_eq!(all.group(), None);
    }
```

In `src/complete.rs` `mod tests`:

```rust
    #[test]
    fn group_is_recorded() {
        let line = walked(&["tasks", "list", "--group", "vf", "--parent", ""], 5);
        assert_eq!(line.group.as_deref(), Some("vf"));
        assert_eq!(walked(&["tasks", "list", "--parent", ""], 3).group, None);
    }
```

- [ ] **Step 2: Write the failing end-to-end tests** (append to `tests/cli.rs`)

```rust
/// Three registered projects; group `vf` holds the first two.
fn grouped_projects(
    env: &mut TestEnv,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let sci = env.init("sci");
    let fam = env.init("fam");
    let ops = env.init("ops");
    env.json(&sci, &["group", "set", "vf", "sci", "fam"]);
    (sci, fam, ops)
}

#[test]
fn group_scopes_the_read_views_to_its_members() {
    let mut env = TestEnv::new();
    let (sci, fam, ops) = grouped_projects(&mut env);
    let s = id_of(env.json(&sci, &["add", "S", "-p", "1"]));
    let f = id_of(env.json(&fam, &["add", "F", "-p", "2"]));
    let o = id_of(env.json(&ops, &["add", "O", "-p", "0", "--tag", "only-ops"]));
    let nowhere = tempfile::tempdir().unwrap();

    let ready = env.json(nowhere.path(), &["ready", "--group", "vf"]);
    let ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [s.as_str(), f.as_str()]);
    assert_eq!(
        env.json(nowhere.path(), &["next", "--group", "vf"])["next"]["task"]["id"],
        s,
        "ops's P0 is outside the group"
    );

    let prime = env.json(nowhere.path(), &["prime", "--group", "vf"]);
    assert_eq!(prime["group"], "vf");
    assert_eq!(prime["prefix"], serde_json::Value::Null);
    assert_eq!(prime["projects"], serde_json::json!(["fam", "sci"]));
    assert!(!prime["ready"].to_string().contains(&o), "{prime}");
    assert!(
        env.json(nowhere.path(), &["prime", "--all-projects"])
            .get("group")
            .is_none()
    );
    assert!(env.json(&sci, &["prime"]).get("group").is_none());
    let text = env.pretty(nowhere.path(), &["prime", "--group", "vf"]);
    assert!(text.starts_with("group vf: projects fam, sci\n"), "{text}");

    let tree = env.json(nowhere.path(), &["tree", "--group", "vf"]);
    assert_eq!(tree["nodes"].as_array().unwrap().len(), 2, "{tree}");
    for command in ["list", "sample", "tree"] {
        let out = env.json(nowhere.path(), &[command, "--group", "vf"]);
        assert!(!out.to_string().contains(&o), "{command}: {out}");
    }
    let tags = env.json(nowhere.path(), &["tags", "--group", "vf"]);
    assert!(!tags.to_string().contains("only-ops"), "{tags}");

    // `lanes` (Slice 3) flattens the same scope arguments.
    let lane = id_of(env.json(&ops, &["add", "Ops effort", "--lane"]));
    assert_eq!(
        env.json(nowhere.path(), &["lanes", "--group", "vf"])["lanes"],
        serde_json::json!([])
    );
    assert!(
        env.json(nowhere.path(), &["lanes", "--all-projects"])["lanes"]
            .to_string()
            .contains(&lane)
    );
}

#[test]
fn group_conflicts_with_the_other_scopes_and_an_unknown_name_is_unknown_group() {
    let mut env = TestEnv::new();
    let (sci, _, _) = grouped_projects(&mut env);
    for command in ["list", "ready", "next", "prime", "tree", "tags", "sample", "lanes"] {
        env.usage(&sci, &[command, "--group", "vf", "--project", "sci"]);
        env.usage(&sci, &[command, "--group", "vf", "--all-projects"]);
        assert_eq!(
            env.fail(&sci, &[command, "--group", "nope"]),
            "unknown_group",
            "{command}"
        );
    }
    let id = id_of(env.json(&sci, &["add", "Goal"]));
    env.usage(&sci, &["tree", id.as_str(), "--group", "vf"]);
}

#[test]
fn a_group_whose_members_are_all_unreachable_warns_and_reads_empty() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "S"]);
    env.json(&sci, &["group", "set", "away", "fam"]);
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let nowhere = tempfile::tempdir().unwrap();
    for command in ["list", "ready", "next", "prime", "tree", "tags", "sample", "lanes"] {
        let out = env.json(nowhere.path(), &[command, "--group", "away"]);
        let warnings = warnings_of(&out);
        assert!(
            warnings
                .iter()
                .any(|w| w.starts_with("project fam at ") && w.ends_with(" is unreachable")),
            "{command}: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.starts_with("sci") || w.contains("project sci ")),
            "{command}: {warnings:?}"
        );
    }
    assert_eq!(
        env.json(nowhere.path(), &["ready", "--group", "away"])["tasks"],
        serde_json::json!([])
    );
    assert!(env.json(nowhere.path(), &["next", "--group", "away"])["next"].is_null());
    let prime = env.json(nowhere.path(), &["prime", "--group", "away"]);
    assert_eq!(prime["projects"], serde_json::json!([]));
    assert!(
        warnings_of(&prime)
            .iter()
            .any(|w| w == "fam: halt state unknown (registered checkout unreachable)"),
        "{prime}"
    );
}

#[test]
fn group_halt_views_filter_members_and_warn_only_about_members() {
    let mut env = TestEnv::new();
    let (sci, fam, ops) = grouped_projects(&mut env);
    let sci_task = id_of(env.json(&sci, &["add", "Sci work", "-p", "2"]));
    let fam_task = id_of(env.json(&fam, &["add", "Fam work", "-p", "2"]));
    let halt = id_of(env.json(&sci, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(&sci, &["shelve", &halt, "pending"]);
    std::fs::remove_file(ops.join("tasks/.config.toml")).unwrap();
    let about_ops = |w: &String| w.starts_with("ops:") || w.starts_with("project ops ");
    for command in ["ready", "prime", "next"] {
        let output = env.json(&fam, &[command, "--group", "vf"]);
        assert_eq!(output["halts"][0]["id"], halt, "{output}");
        let rows = match command {
            "prime" => &output["ready"],
            "ready" => &output["tasks"],
            _ => &output["next"],
        };
        assert!(rows.to_string().contains(&fam_task), "{output}");
        assert!(!rows.to_string().contains(&sci_task), "{output}");
        assert!(
            !warnings_of(&output).iter().any(about_ops),
            "{command}: {output}"
        );
        let wide = env.json(&fam, &[command, "--all-projects"]);
        assert!(
            warnings_of(&wide)
                .iter()
                .any(|w| w == "ops: halt state unknown (registered checkout unreachable)"),
            "{command}: {wide}"
        );
    }
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one scope::tests::open_group` — expected: compile error, `no function
open_group`.
Run: `just test-one --test cli group_` — expected: the new tests FAIL with exit 2,
`unexpected argument '--group'`.

- [ ] **Step 4: Implement the registry lookup and the scope**

`src/registry.rs`, after `remove_group`:

```rust
    /// The members of group `name`, or `unknown_group` for a name not declared here.
    pub fn group(&self, name: &str) -> Result<&[String]> {
        self.groups
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| unknown_group(name))
    }
```

`src/scope.rs`, replace lines 84-115 (`enum Scope` through `projects`):

```rust
/// What a read command looks at: one project, or a registry-wide read. The wide read can
/// cover every registered project or the members of one group.
pub enum Scope {
    Local(Project),
    /// `projects` are the reachable ones among `members`, opened. `members` is what was
    /// asked for: every registered prefix under `--all-projects`, a group's members under
    /// `--group`. Warnings about absent projects name `members` only (spec 2026-10-03 §6.4).
    All {
        projects: Vec<Project>,
        members: Vec<String>,
        group: Option<String>,
    },
}

impl Scope {
    /// Every registered project that is reachable, in registry (prefix) order, plus the
    /// warnings the walk produced. Never locates a local project; the only look at `cwd`
    /// is to warn when it lies inside a project the registry does not know.
    pub fn open_all(registry: &Registry, cwd: &Path) -> Result<(Scope, Vec<String>)> {
        let members = registry.projects.keys().cloned().collect();
        Self::open_members(registry, cwd, members, None)
    }

    /// `--group <name>`: the group's members, walked as `open_all` walks the registry.
    /// A member that is unreachable is a warning, as it is under `--all-projects`. An
    /// undeclared name is `unknown_group`.
    pub fn open_group(registry: &Registry, cwd: &Path, name: &str) -> Result<(Scope, Vec<String>)> {
        let members = registry.group(name)?.to_vec();
        Self::open_members(registry, cwd, members, Some(name.to_string()))
    }

    fn open_members(
        registry: &Registry,
        cwd: &Path,
        members: Vec<String>,
        group: Option<String>,
    ) -> Result<(Scope, Vec<String>)> {
        let mut warnings = registry_warnings(registry, cwd)?;
        let mut projects = Vec::new();
        for (prefix, root) in registry
            .projects
            .iter()
            .filter(|(prefix, _)| members.contains(prefix))
        {
            if !is_reachable(root)? {
                warnings.push(format!(
                    "project {prefix} at {} is unreachable",
                    root.display()
                ));
                continue;
            }
            projects.push(open_registered(registry, prefix, Origin::Prefix)?);
        }
        Ok((
            Scope::All {
                projects,
                members,
                group,
            },
            warnings,
        ))
    }

    pub fn projects(&self) -> &[Project] {
        match self {
            Scope::Local(project) => std::slice::from_ref(project),
            Scope::All { projects, .. } => projects,
        }
    }

    /// The group named on the command line, under `--group`.
    pub fn group(&self) -> Option<&str> {
        match self {
            Scope::All { group, .. } => group.as_deref(),
            Scope::Local(_) => None,
        }
    }
```

- [ ] **Step 5: Implement the flag and the context**

`src/cli.rs` `ScopeArgs` (lines 38-53): replace the doc comment's first sentence with
`/// The four read scopes: the current project (no flag), one named registered project,`
`/// the members of a project group, or every reachable one.` Then add after
`all_projects`:

```rust
    /// The members of this project group (`tasks groups`); needs no local project.
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with_all = ["project", "all_projects"],
        add = ArgValueCandidates::new(crate::complete::groups)
    )]
    pub group: Option<String>,
```

`Tree` (line 664): `conflicts_with = "all_projects",` →
`conflicts_with_all = ["all_projects", "group"],`. Also update its doc comment to "One
forest per project in scope, so a registry-wide scope and an id conflict."

`src/commands/mod.rs` `open_id_read_ctx` line 502:

```rust
    if scope.project.is_some() || scope.all_projects || scope.group.is_some() {
```

and its doc sentence: "An explicit `--project`, `--group` or `--all-projects` is the caller naming
the scope and wins over the prefix."

`open_read_ctx`, replace lines 559-568:

```rust
    let start = start_dir(dir)?;
    if scope.all_projects || scope.group.is_some() {
        let registry = Registry::load()?;
        let (scope, warnings) = match &scope.group {
            Some(name) => Scope::open_group(&registry, &start, name)?,
            None => Scope::open_all(&registry, &start)?,
        };
        return Ok(ReadCtx {
            scope,
            registry,
            warnings,
            shorthand: Shorthand::new(start),
        });
    }
```

and in its doc comment: "or with `all_projects` every reachable one, or with `group` the
reachable members of that group. All three flags skip the local lookup entirely".

`Command::Quiet` arm (lines 1421-1424): the `ScopeArgs` literal gains `group: None,`, so it
still compiles. Task 4.5 wires quiet's own flag.

- [ ] **Step 6: Implement the views**

`src/commands/list.rs` `halt_snapshots`, replace lines 254-263:

```rust
    // Only what the scope asked for: under --group a non-member's checkout is no gap.
    if let Scope::All { members, .. } = &ctx.scope {
        let scoped = ctx.scope.prefixes();
        for prefix in members {
            if !scoped.contains(prefix) {
                ctx.warnings.push(format!(
                    "{prefix}: halt state unknown (registered checkout unreachable)"
                ));
            }
        }
    }
```

Line 569: `let wide = matches!(ctx.scope, Scope::All { .. });`.
In `prime`'s `PrimeOut` (lines 601-606):

```rust
        prefix: match &ctx.scope {
            Scope::Local(project) => Some(project.prefix.clone()),
            Scope::All { .. } => None,
        },
        group: ctx.scope.group().map(str::to_string),
```

Unit test line 650: `scope: Scope::All { projects: vec![], members: vec![], group: None },`.

`src/commands/tags.rs` line 12: `Scope::All { projects, .. } => projects.iter().collect(),`
and the doc on `meaning_of`: "or the first registered project's under a registry-wide scope".

`src/output.rs` `PrimeOut`, after `prefix`:

```rust
    /// The group under --group; absent otherwise. `prefix` is then null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
```

and update the `prefix` doc to "The local project; null under --all-projects and --group."
Pretty header (lines 1005-1008):

```rust
            let header = match (&o.prefix, &o.group) {
                (Some(prefix), _) => format!("project {prefix}"),
                (None, Some(group)) => format!("group {group}: projects {}", o.projects.join(", ")),
                (None, None) => format!("projects {}", o.projects.join(", ")),
            };
```

- [ ] **Step 7: Implement completion**

`src/complete.rs` `Line`: add `pub group: Option<String>,` after `all_projects`. In `record`,
after the `"project"` arm:

```rust
        "group" => {
            if let Some(value) = values.last() {
                line.group = Some((*value).to_string());
            }
        }
```

In `scoped`, replace the `if line.all_projects { … }` head (lines 548-553):

```rust
    if line.all_projects || line.group.is_some() {
        // Best effort, like every completion: an undeclared group offers nothing.
        let members: Vec<String> = match &line.group {
            Some(name) => registry.groups.get(name).cloned().unwrap_or_default(),
            None => registry.projects.keys().cloned().collect(),
        };
        for prefix in &members {
            if let Some(project) = open_prefix(&registry, prefix) {
                tasks.extend(project.scan_lenient().0);
            }
        }
    } else {
```

and add "`--group` narrows it to the group's members" to `scoped`'s doc comment.

- [ ] **Step 8: Adopt this task's rows**

In `.worktrees/ece1e2-work-selection` (this also refreshes `tools/cli_surface.py` when the
pair changed):

```bash
OPS=$(tasks projects | jq -r '.projects[] | select(.prefix == "ops") | .root')
SHA=$(tasks show tasks-ece1e2 | jq -r '.task.notes[].text | capture("^inventory: Task 4\\.4 = (?<sha>[0-9a-f]{7,40})$").sha' | tail -n 1)
test -n "$OPS" && test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
```

Run: `just test-one parser_surface_equals_table` — expected: PASS. The adopted `lanes`
row now carries `--group`, and so does the parser's (from the flattened `ScopeArgs`).
A mismatch means the parser disagrees with the landed rows. Fix the parser to match.
Never edit `tools/cli.toml`.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `just test-one scope::tests` and `just test-one complete::tests` — expected: PASS.
Run: `just test-one --test cli group_` and `just test-one --test cli a_group_whose_members` — expected: PASS.
Run: `just test-one --test cli all_projects`, `just test-one --test cli halt_views` and
`just test-one --test cli lanes` — expected: the existing tests PASS unchanged.

- [ ] **Step 10: Gate**

Run: `cargo fmt && just check && just test-fast` — expected: clean, all pass.

- [ ] **Step 11: Commit**

```bash
git add src/registry.rs src/scope.rs src/cli.rs src/commands/mod.rs src/commands/list.rs \
  src/commands/tags.rs src/output.rs src/complete.rs tools/cli.toml tests/cli.rs
git status --short tools/cli_surface.py | grep -q . && git add tools/cli_surface.py
git commit -m "feat(scope): --group reads a project group's members in the read views"
```

---

### Task 4.5: `quiet --group`; `claims` keeps its single scope

**Files:**
- Modify: `src/cli.rs:686-701` (`Quiet`).
- Modify: `src/commands/mod.rs:1416-1426` (quiet dispatch).
- Adopt: `tools/cli.toml` (and `tools/cli_surface.py` if it changes), through
  `vendored adopt --at` the Task 4.5 inventory commit. Never edit it.
- Modify: `tests/cli.rs` (append).

**Interfaces:**
- Consumes: `ScopeArgs.group`, `open_read_ctx` (Task 4.4), `complete::groups` (Task 4.2).
- Produces: `Command::Quiet { limit, project, all_projects, group: Option<String> }`.

**Ops rows (landed by Task 0.1):** in ops `cli.toml`, table `cli.tasks`, in the
`[[cli.tasks.commands]]` row `path = ["quiet"]`, directly after its line
`  { shared = "all_projects", value = "none" },`, add:

```toml
  { names = ["--group"], value = "string" },
```

The `claims` row is unchanged.

- [ ] **Step 1: Write the failing test** (append to `tests/cli.rs`)

```rust
#[test]
fn quiet_group_lists_only_member_parks_and_claims_keeps_its_single_scope() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["group", "set", "solo", "sci"]);
    let mine = id_of(env.json(&sci, &["add", "Capture here", "-p", "2"]));
    let theirs = id_of(env.json(&fam, &["add", "Capture there", "-p", "1"]));
    for (dir, id) in [(&sci, &mine), (&fam, &theirs)] {
        as_agent(&env, dir, "agent-a")
            .args([
                "park",
                id.as_str(),
                "run it",
                "--waiting-on",
                "user",
                "--reason",
                "quiet",
                "--minutes",
                "30",
            ])
            .assert()
            .success();
    }
    let nowhere = tempfile::tempdir().unwrap();
    let ids = |value: serde_json::Value| -> Vec<String> {
        value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        ids(env.json(nowhere.path(), &["quiet"])),
        [theirs.clone(), mine.clone()]
    );
    assert_eq!(
        ids(env.json(nowhere.path(), &["quiet", "--group", "solo"])),
        [mine.clone()]
    );
    env.usage(nowhere.path(), &["quiet", "--group", "solo", "--project", "sci"]);
    env.usage(nowhere.path(), &["quiet", "--group", "solo", "--all-projects"]);
    assert_eq!(
        env.fail(nowhere.path(), &["quiet", "--group", "nope"]),
        "unknown_group"
    );
    // By contract claims fails rather than answering for part of the registry.
    env.usage(nowhere.path(), &["claims", "--group", "solo"]);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `just test-one --test cli quiet_group_lists_only_member_parks` — expected: FAIL, `quiet
--group` exits 2 with `unexpected argument '--group'`.

- [ ] **Step 3: Implement**

`src/cli.rs` `Quiet`: give `project` `conflicts_with_all = ["all_projects", "group"]` in
place of `conflicts_with = "all_projects"`. Then add after `all_projects`:

```rust
        /// The members of this project group instead of all of them.
        #[arg(
            long,
            value_name = "NAME",
            conflicts_with = "all_projects",
            add = ArgValueCandidates::new(crate::complete::groups)
        )]
        group: Option<String>,
```

`src/commands/mod.rs`, replace the `Command::Quiet` arm:

```rust
        Command::Quiet {
            limit,
            project,
            all_projects: _,
            group,
        } => {
            let scope = ScopeArgs {
                all_projects: project.is_none() && group.is_none(),
                project,
                group,
            };
            quiet::run(open_read_ctx(dir, &scope)?, limit)
        }
```

- [ ] **Step 4: Adopt this task's rows**

In `.worktrees/ece1e2-work-selection` (this also refreshes `tools/cli_surface.py` when the
pair changed):

```bash
OPS=$(tasks projects | jq -r '.projects[] | select(.prefix == "ops") | .root')
SHA=$(tasks show tasks-ece1e2 | jq -r '.task.notes[].text | capture("^inventory: Task 4\\.5 = (?<sha>[0-9a-f]{7,40})$").sha' | tail -n 1)
test -n "$OPS" && test -n "$SHA" && python3 "$OPS/bin/vendored" adopt cli.toml --at "$SHA"
```

Run: `just test-one parser_surface_equals_table` — expected: PASS. Never edit
`tools/cli.toml`; a mismatch is fixed in the parser.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `just test-one --test cli quiet_` — expected: PASS (new and existing quiet tests).

- [ ] **Step 6: Gate**

Run: `cargo fmt && just check && just test-fast` — expected: clean, all pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/commands/mod.rs tools/cli.toml tests/cli.rs
git status --short tools/cli_surface.py | grep -q . && git add tools/cli_surface.py
git commit -m "feat(quiet): --group narrows the quiet queue to a project group"
```

---

### Task 4.6: Document project groups

**Files:**
- Modify: `skills/tasks/SKILL.md:50-54` (quiet line), `:196-203` (read-scope paragraph and a new groups paragraph after it).
- Modify: `README.md:286-288` (Use block), `:336-341` (registry paragraph), `:550` (Layout).
- Modify: `docs/specs/2026-08-29-tasks-design.md:576-579` (§5.1 addenda at the end of the shapes block), `:627-640` and `:652-660` (§6), and the §5 usage lines `[--project P | --all-projects]`.
- Modify: `docs/specs/2026-09-08-prefix-rename-design.md:316-330` (refusal R12).

**Interfaces:** none (documentation only).

- [ ] **Step 1: The skill**

In `skills/tasks/SKILL.md`, the quiet line (52-53): "`-n 1` is the top of the queue and
`--project <prefix>` narrows it." → "`-n 1` is the top of the queue, and `--project <prefix>`
or `--group <name>` narrows it."

In the read-scope paragraph (196-199): "each take `--project <prefix>` for one registered
project or `--all-projects` for every reachable one. Either works from anywhere," → "each take
`--project <prefix>` for one registered project, `--group <name>` for the members of a
project group, or `--all-projects` for every reachable one. Each works from anywhere,".
Add this paragraph after the one that ends "wins over the prefix.":

```markdown
A project group is a named set of registered projects, declared in this host's registry
and not synced. Each host declares its own groups.

- **Managing groups.** `tasks group set <name> <prefix>...` creates or replaces a group,
  and a retired prefix resolves to its live name. `tasks group rm <name>` deletes a group.
  `tasks groups` lists each group with whether each member is reachable.
- **Names.** Names use the tag grammar (lowercase letters, digits, `-`). A name cannot be
  a live or retired prefix, or a prefix an unfinished rename reserves. `init` and `rename`
  refuse a prefix that names a group. Groups may overlap.
- **Membership changes.** `rename` carries membership to the new prefix. `unregister`
  drops the prefix from every group, and deletes, with a warning, any group it leaves
  empty.
- **Reading a group.** Under `--group`, `prime` adds `group` and its `prefix` is null.
  Warnings about unreachable projects or unknown halt state name members only. A group
  whose members are all unreachable gives those warnings and empty results.
- **Errors and limits.** A misspelled name is `unknown_group`. `tasks claims` always reads
  every store.
```

- [ ] **Step 2: The README**

In the Use block, after `    tasks projects                   # the registry: reachable? counts?`:

```text
    tasks group set vf nodes atoms   # a named set of registered projects (this host only)
    tasks ready --group vf           # read a group; also list, next, prime, tree, tags,
                                     #   sample, lanes, quiet. Needs no local project.
    tasks groups                     # each group, its members, and whether they are reachable
    tasks group rm vf                # delete the group; its projects stay registered
```

Append to the registry paragraph that ends "retired names cannot be reused while
registered.":

```markdown
The registry can also declare project groups (`[groups]`), named sets of live prefixes
that `--group <name>` reads together. `rename` carries a member to its new prefix.
`unregister` removes the prefix from every group and deletes, with a warning, any group it
leaves empty. A group name is never a live or retired prefix.
```

Layout line 550: append `; [groups] name -> prefixes` to
`per-machine registry: live prefix -> repo path; retired -> live`.

- [ ] **Step 3: The main design**

In `docs/specs/2026-08-29-tasks-design.md` §5.1, insert before the closing fence of the
shapes block (after the `feedback ->` entry, line 578):

```text

prime       += group: string                  only under --group; prefix is null there
groups      -> { groups: [{ name, members: [{ prefix, reachable: bool }] }], warnings }
group set, group rm
            -> { name, members: [string], warnings }
               set: the members stored (live prefixes, sorted, distinct); rm: those it had
unregister  += warnings: one per group it emptied and deleted
errors      += unknown_group                  a --group or group rm name not declared
```

In §6, extend the registry example (after `fam = "~/d/familiar"`):

```toml

[groups]
verifiably = ["atoms", "nodes"]
```

Replace "The same seven read commands (list, ready, prime, tree, next, tags, sample) take
`--project <p>` and `--all-projects`, which conflict. Both locate no local project." with
"The same eight read commands (list, ready, prime, tree, next, tags, sample, lanes) take
`--project <p>`, `--group <name>`, and `--all-projects`, which conflict pairwise. None
locates a local project." Add a paragraph after the `--all-projects` paragraph (the one
ending "is a config error."):

```markdown
`--group <name>` reads the members of a project group declared in the registry's
`[groups]` table:

- **Each member** is handled by `--all-projects`' rules.
- **Warnings.** Warnings about unreachable projects and unknown halt state name members
  only, and an undeclared name is `unknown_group`.
- **Commands.** `quiet` takes it too; `claims` keeps its single registry-wide scope.
- **Names and members.** Group names use the tag grammar and never equal a live or retired
  prefix, and `group set` refuses one an unfinished rename reserves. Members are live prefixes, kept so by `rename` and `unregister`; a registry
  naming any other is a `config` error that names the group and the prefix.

See docs/specs/2026-10-03-lanes-needs-groups-design.md §6.
```

In §5, every usage line naming the two scopes gains the third. These are the lines for
list, tree, ready, sample, next, prime and tags, plus the `tasks lanes` entry Task 3.9
added:

```bash
sed -i 's/\[--project P | --all-projects\]/[--project P | --group NAME | --all-projects]/' \
  docs/specs/2026-08-29-tasks-design.md
grep -c -- '--project P | --group NAME | --all-projects' docs/specs/2026-08-29-tasks-design.md
grep -c -- '--project P | --all-projects\]' docs/specs/2026-08-29-tasks-design.md
```

Expected: the first count is at least 8 (seven before Slice 3, plus `lanes`). The second
prints `0`.

- [ ] **Step 4: The rename design (refusal R12)**

In `docs/specs/2026-09-08-prefix-rename-design.md`, "Refusals, evaluated before the table":
replace "**R7–R8 always apply**: they read the registry alone." with "**R7, R8 and R12 always
apply**: they read the registry alone." Then add a row to the table after the R8 row:

```markdown
| R12 | always | the target prefix is the name of a project group (docs/specs/2026-10-03-lanes-needs-groups-design.md §6) |
```

After the sentence "R9–R11 (attachment directories) are defined in
docs/specs/2026-09-28-task-attachments-design.md, "Rename".", add:

```markdown
R12 is the group collision that `is_taken` also refuses on a fresh rename. As a refusal
it is decided before the table, so `--explain` reports it, and a rename interrupted
before a group took its target refuses to resume rather than failing at the registry
step. `rename` reports it as `config`, as it reports a retired target.
```

- [ ] **Step 5: Record the idea's outcome**

Run: `tasks note tasks-77dbc6 "implemented by tasks-ece1e2 slice 4 (spec 2026-10-03 §6); close with the slice"`.
The plan controller closes the idea when the slice lands (spec §7).

- [ ] **Step 6: Gate**

Run: `just check && just test-fast` — expected: clean, all pass. The README and design
documents are on the full check.

- [ ] **Step 7: Commit**

```bash
git add skills/tasks/SKILL.md README.md docs/specs/2026-08-29-tasks-design.md \
  docs/specs/2026-09-08-prefix-rename-design.md tasks/tasks-77dbc6.md
git commit -m "docs(groups): document project groups in the skill, README, and main design"
```

### Spec coverage (§10 Groups and the §6 rules)

| Case | Test |
|---|---|
| `set`, `rm`, `groups` work; an alias resolves on `set`; a duplicate is stored once | `group_set_rm_and_groups_manage_named_project_sets`, `set_group_resolves_aliases_stores_each_member_once_and_validates` |
| tag grammar; collision with a prefix or a retired alias; an unknown member | `group_set_refuses_bad_or_colliding_names_and_unknown_members`, `load_rejects_an_invalid_group` |
| `init`/`register` refuse a group name | `init_refuses_a_prefix_that_names_a_group`, `a_group_name_cannot_become_a_prefix` |
| `rename` rewrites members; refuses a group-named target | `rename_rewrites_group_members_and_refuses_a_group_name_as_target`, `rename_and_adopt_carry_group_membership` |
| `rename --explain` and execution agree on a group-named target, fresh and mid-rename (R12) | `rename_explain_and_execution_both_refuse_a_group_named_target`, `each_refusal_fires_on_its_own_before_the_table`, `observes_whether_the_target_names_a_group` |
| `group set` cannot take a name an unfinished rename reserves | `group_set_refuses_a_name_an_unfinished_rename_reserves` |
| `adopt` keeps groups consistent | `adopt_carries_group_membership_and_refuses_a_group_named_target` |
| `unregister` prunes, deletes emptied groups, and warns | `unregister_removes_the_prefix_from_groups_and_deletes_emptied_ones`, `unregister_prunes_groups_and_deletes_the_ones_it_empties` |
| dangling member → `config` naming group and prefix | `a_registry_naming_an_unregistered_group_member_fails_to_load`, `load_rejects_an_invalid_group` |
| `--group` scopes prime, ready, `lanes`, and the other views; `prime.group`, null `prefix` | `group_scopes_the_read_views_to_its_members` |
| `--group` conflicts with `--project`/`--all-projects`; unknown name → `unknown_group` | `group_conflicts_with_the_other_scopes_and_an_unknown_name_is_unknown_group` |
| halt warnings name only members | `group_halt_views_filter_members_and_warn_only_about_members`, `open_group_opens_and_warns_about_members_only` |
| every member unreachable → warnings and empty results | `a_group_whose_members_are_all_unreachable_warns_and_reads_empty` |
| `quiet --group`; `claims` unchanged | `quiet_group_lists_only_member_parks_and_claims_keeps_its_single_scope` |

---


---

## Slice 5 — Integration

### Task 5.1: Integrate, install, and close the waiting ideas

**Files:**
- Modify: `docs/notes/2026-09-30-work-selection-brief.md`, `tasks/*.md` (task records only, through the CLI).

- [ ] **Step 1: Run the full gate in the worktree.**
  Run `just gate`. Expected: check plus the full suite, including the `#[ignore]`d enumeration, all pass.

- [ ] **Step 2: Confirm the adopted inventory is the landed head of Task 0.1.**
  Run `python3 "$OPS/bin/vendored" status`. Expected: the `tasks` project's `cli.toml` is `current`, or stale only at a commit that includes the last inventory commit.

- [ ] **Step 3: Close the ops side.**
  Run `tasks done <ops task from Task 0.1> "tasks adopted its rows at <last sha>"` in the ops checkout.
  Ops closes its task when its pieces close (ops vendoring design, closeout), and tasks-ece1e2 is that piece. So run this after Step 5.

- [ ] **Step 4: Record the decisions on the waiting ideas,** in the worktree, in one commit with the brief update:
  ```bash
  tasks note tasks-9bdd68 "scope: drop; proposal: drop as superseded by lanes (spec docs/specs/2026-10-03-lanes-needs-groups-design.md §7): lane priority orders the lanes view, next --under selects within one effort, and the committed lane field replaces the host-local focus"
  tasks note tasks-77dbc6 "implemented by tasks-ece1e2 slice 4 (spec §6)"
  tasks done tasks-77dbc6 "Project groups: registry [groups], tasks group set|rm, tasks groups, --group scope (spec §6)"
  tasks note tasks-e02860 "implemented by tasks-ece1e2 slices 1-3 (spec §3-§5)"
  tasks done tasks-e02860 "Lanes, needs, and exclusive holds landed (spec §3-§5)"
  ```
  Add a section `## Decisions (2026-10-03, tasks-ece1e2)` to the brief listing these three outcomes and the spec path. Then:
  ```bash
  tasks check && git add docs/notes/2026-09-30-work-selection-brief.md tasks && git commit -m "chore(tasks): record lanes, needs, and groups decisions"
  ```

- [ ] **Step 5: Merge into `main`, install, and close.**
  From the main checkout:
  ```bash
  git merge --no-ff ece1e2-work-selection
  cargo install --path .
  tasks lanes; tasks groups
  ```
  Expected: both commands run and print JSON.

  Then close the task:
  ```bash
  tasks done tasks-ece1e2 "Lanes, needs, holds, and project groups landed (spec 2026-10-03)"
  git add tasks && git commit -m "chore(tasks): close tasks-ece1e2"
  ```

  This is a personal-profile repository, so the local merge is a bounded decision. Pushing stays gated on the user.

- [ ] **Step 6: Clean up the worktree.**
  ```bash
  tt-report
  git worktree unlock .worktrees/ece1e2-work-selection
  git worktree remove .worktrees/ece1e2-work-selection
  ```
