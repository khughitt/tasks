# Park Reasons and Lifecycle Stamps Implementation Plan

**Status:** approved, not yet implemented

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every record gains two stamps written by `transition()` and nothing else — `started` on the first transition into `doing`, `completed` on every completing transition and cleared on any transition out of `done` — and `park` gains an optional `--reason` from a six-word vocabulary that rides the store entry, the park note, and `park.reason` in every JSON view.

**Architecture:** The stamps are two optional timestamps threaded exactly as `last_done` is (model.rs field, format.rs `KEYS`/parse/`quote_timestamps`/validate/serialize, `Value::Raw`), stamped in the one place every status change funnels through (`transition()`, src/commands/mod.rs) beside the `model` stamp, guarded from hand edits in `check_invariants`, and reported by `check` when a persisted record carries `completed` while open. The reason is a `Reason` enum beside `WaitingOn` in claims.rs, an `Option` on `Park` that older store files lack, and one new field on `ParkInfo` — the single park block `ShowFields`, `TaskSummary`, and `ParkedRow` all embed — so no view carries a second copy.

**Tech Stack:** Rust 2024, clap 4.6, serde, toml. No new dependencies.

**Spec:** `docs/specs/2026-09-11-park-reason-and-stamps-design.md`

**Task:** `tasks-82b559`

## Global Constraints

- **JSON output is the contract; every change here is additive.** New keys `started` and `completed` (null when absent) on the full `Task` in `show`/`next` (free via the derived `Serialize`), on `TaskSummary` (`list`/`ready`/`prime`/`tree`), and on `ParkedRow`; new key `reason` (null when absent) inside `park` wherever a `park` block appears. Nothing existing changes shape or meaning. (`AGENTS.md`; spec §3 "JSON", §4 "Rows")
- **The stamps are written by `transition()` and by nothing else.** No `add` or `edit` flag sets them; an editor save that sets, moves, or clears one is refused. (spec §3 "Not editable")
- **`completed` on an open record parses.** It is a `check` finding, never a parse error, because the editor path parses the edited text before `transition()` clears the stamp. (spec §3 "Validation")
- **The reason is optional with no default and no `unknown` value.** A value outside `review`, `decision`, `approval`, `environment`, `dependency`, `session` is a validation error listing the six. Any reason combines with either `--waiting-on`. (spec §2, §4)
- **This crate has no library target, and every module is private.** An unused `pub fn` or unread struct field fails `cargo clippy --all-targets -- -D warnings` with `dead_code`; use from `#[cfg(test)]` does not count. Every helper below is introduced in the task whose production code consumes it. Unit tests run as `cargo test --bin tasks <filter>`; end-to-end tests as `cargo test --test cli <filter>`.
- **Gates.** `just check` before every commit (fmt, clippy `-D warnings`, `tasks check`); `just test` is `cargo test`. Rebuild and reinstall after CLI changes so the tracker in use is the code under test: `cargo install --path .`.
- **Struct-literal sites that break when a field is added** (each task names the ones it touches): `Task` (`src/commands/add.rs:25`, `src/repo.rs:607`, `src/hierarchy.rs:248`, `src/query.rs:249`, `src/similarity.rs:102`, test helper `task_with` at `src/model.rs:78`); `TaskSummary` (constructor at `src/output.rs:243`, test fixture `row` at `src/output.rs:1100`); `ParkedRow` (`resolved` at `src/output.rs:296`, `unresolved` at `src/output.rs:316`); `Park` (`src/commands/park.rs:48`, test at `src/claims.rs:970`); `ParkInfo::of` (`src/output.rs:198`).
- **End-to-end tests isolate the environment.** `TestEnv::cmd`/`raw` set `HOME` to a temp dir and scrub `TASKS_*`. Session-identified commands use `as_agent(&env, &dir, "name")` (tests/cli.rs:5070); editor-driven edits use `editor_script(&dir, "<sh body>")` (tests/cli.rs:2166) with `.env("EDITOR", &editor)`.
- **Commit hygiene.** Never `git add -A`: two untracked feedback reports (`tasks/tasks-74a525.md`, `tasks/tasks-b5add6.md`) are unrelated and stay out. Conventional commits; no AI-attribution trailers. This work lives in the `.worktrees/waiting-on` checkout on branch `waiting-on`; every path below is relative to that checkout.
- **Each plan task has a step child under `tasks-82b559`**; the ids are listed at the end of this header. `tasks start <step>` before its first step and `tasks done <step> "<what landed>"` in the same commit as its code; the commit blocks below include the `done`.

**Step children:** Task 1 `tasks-bfe479`, Task 2 `tasks-1a8e3b`, Task 3 `tasks-a7387b`, Task 4 `tasks-8da6fa`.

---

### Task 1: The started and completed stamps on the record

**Files:**
- Modify: `src/model.rs:320` (fields after `updated`), `:78` (test helper `task_with`)
- Modify: `src/format.rs:6` (`KEYS`), `:115` (parse), `:136` (`quote_timestamps`), `:272` (`validate_task`), `:350` (`serialize_task`)
- Modify: `src/commands/add.rs:25`, `src/repo.rs:607`, `src/hierarchy.rs:248`, `src/query.rs:249`, `src/similarity.rs:102` (Task literals)
- Modify: `src/output.rs:138-150` (`TaskSummary` fields), `:243` (constructor), `:1100` (test fixture `row`), `:273-290` (`ParkedRow` fields), `:296` (`resolved`), `:316` (`unresolved`)
- Test: `src/format.rs` tests module, after `rejects_a_bad_cadence_and_a_bare_anchor` (`:451`)

**Interfaces:**
- Produces: `Task::started: Option<String>`, `Task::completed: Option<String>`; both accepted in frontmatter between `updated` and `last_done`, quoted by `quote_timestamps`, validated as RFC 3339 by `validate_task`, serialized as `Value::Raw`, omitted when absent. `TaskSummary::started`/`completed` and `ParkedRow::started`/`completed` (`Option<String>`, null in JSON when absent) copied from the task.
- Consumed by: Task 2 writes them and tests the summary rows; Task 4 documents them. JSON on `show`/`next` is free through `Task`'s derived `Serialize`.

- [ ] **Step 1: Write the failing unit tests**

In `src/format.rs`'s tests module, after `rejects_a_bad_cadence_and_a_bare_anchor`:

```rust
    #[test]
    fn stamps_round_trip_between_updated_and_last_done() {
        let mut task = parse_task(FULL, "f").unwrap();
        task.every = Some(crate::periodic::Interval::parse("2w").unwrap());
        task.last_done = Some("2026-09-01T08:00:00Z".into());
        task.started = Some("2026-08-30T09:00:00Z".into());
        task.completed = Some("2026-09-01T08:00:00Z".into());
        let text = serialize_task(&task);
        assert!(
            text.contains(
                "updated: 2026-08-29T14:02:11Z\nstarted: 2026-08-30T09:00:00Z\n\
                 completed: 2026-09-01T08:00:00Z\nlast_done: 2026-09-01T08:00:00Z\n"
            ),
            "stamps follow updated and precede last_done: {text}"
        );
        let back = parse_task(&text, "f").unwrap();
        assert_eq!(back.started, task.started);
        assert_eq!(back.completed, task.completed);
    }

    #[test]
    fn stamps_are_omitted_when_absent() {
        let task = parse_task(FULL, "f").unwrap();
        assert_eq!(task.started, None);
        assert_eq!(task.completed, None);
        let text = serialize_task(&task);
        assert!(
            !text.contains("started:") && !text.contains("completed:"),
            "{text}"
        );
    }

    #[test]
    fn rejects_an_unparsable_stamp() {
        let with = |line: &str| FULL.replace("size: m\n", &format!("size: m\n{line}\n"));
        assert!(parse_task(&with("started: yesterday"), "f").is_err());
        assert!(parse_task(&with("completed: 2026-13-45T00:00:00Z"), "f").is_err());
    }

    #[test]
    fn completed_on_an_open_record_parses() {
        // The editor path parses a reopened record before `transition()` clears the
        // stamp, so this state must parse; `check` reports it instead (spec §3).
        let text = FULL.replace("size: m\n", "size: m\ncompleted: 2026-09-01T08:00:00Z\n");
        let task = parse_task(&text, "f").unwrap();
        assert_eq!(task.status, Status::Todo);
        assert_eq!(task.completed.as_deref(), Some("2026-09-01T08:00:00Z"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks format::tests::stamps 2>&1 | tail -20`
Expected: compile error — `no field `started` on type `Task``.

- [ ] **Step 3: Add the fields to `Task` and every literal**

In `src/model.rs`, after `pub updated: String,`:

```rust
    /// When work first began: stamped by the first transition into `doing` and never
    /// moved. See docs/specs/2026-09-11-park-reason-and-stamps-design.md §3.
    pub started: Option<String>,
    /// When the task was last completed: stamped by every completing transition and
    /// cleared by any transition out of `done`. Unlike `last_done` it exists on every
    /// record and does not survive a reopen. Same spec, §3.
    pub completed: Option<String>,
```

Add `started: None,` and `completed: None,` directly after `updated: ...,` in each literal: `src/model.rs` `task_with`, `src/commands/add.rs`, `src/repo.rs`, `src/hierarchy.rs`, `src/query.rs`, `src/similarity.rs`. Run `cargo build 2>&1 | grep -n 'missing field'` until it is silent.

- [ ] **Step 4: Thread the fields through format.rs**

`KEYS` becomes `[&str; 21]` with `"started", "completed",` inserted after `"updated",`.

In `parse_task`, after `updated,` in the `Task { .. }` literal:

```rust
        started: scalar("started")?,
        completed: scalar("completed")?,
```

In `quote_timestamps`, extend the condition:

```rust
            if line.starts_with("created: ")
                || line.starts_with("updated: ")
                || line.starts_with("started: ")
                || line.starts_with("completed: ")
                || line.starts_with("last_done: ")
```

In `validate_task`, after `crate::time::parse(&t.updated)?;`:

```rust
    if let Some(started) = &t.started {
        crate::time::parse(started)?;
    }
    if let Some(completed) = &t.completed {
        crate::time::parse(completed)?;
    }
```

In `serialize_task`, between the `created`/`updated` pairs and the `last_done` push:

```rust
    if let Some(started) = &t.started {
        pairs.push((String::from("started"), Value::Raw(started.clone())));
    }
    if let Some(completed) = &t.completed {
        pairs.push((String::from("completed"), Value::Raw(completed.clone())));
    }
```

- [ ] **Step 5: Carry the stamps on the summary and parked rows**

In `src/output.rs`, `TaskSummary` gains, after `pub updated: String,`:

```rust
    pub started: Option<String>,
    pub completed: Option<String>,
```

The constructor (`:243`) sets `started: task.started.clone(), completed: task.completed.clone(),` after `updated`; the test fixture `row` (`:1100`) sets both to `None`. `ParkedRow` gains the same two fields after `pub updated: Option<String>,`; `resolved` sets `started: summary.started, completed: summary.completed,` and `unresolved` sets both `None`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --bin tasks format::tests 2>&1 | tail -5` then `cargo test 2>&1 | tail -3`
Expected: all format tests pass, including the four new ones; the suite is green (the JSON keys are additive).

- [ ] **Step 7: Gate and commit**

```bash
just check
git add src/model.rs src/format.rs src/output.rs src/commands/add.rs src/repo.rs src/hierarchy.rs src/query.rs src/similarity.rs
tasks done tasks-bfe479 "started and completed parse, validate, and serialize between updated and last_done"
git add tasks/
git commit -m "feat(record): started and completed stamps on the task record"
```

---

### Task 2: Transition stamps, editor invariants, and the check finding

**Files:**
- Modify: `src/commands/mod.rs:609-624` (`transition()`, the `completing` block)
- Modify: `src/commands/edit.rs:11-32` (`check_invariants`)
- Modify: `src/commands/check.rs:66` (the per-task loop, before the `retired_prefix` check)
- Test: `tests/cli.rs`, after `edit_status_done_and_editor_flips_stamp_like_done` (`:2306`)

**Interfaces:**
- Consumes: `Task::started`, `Task::completed` from Task 1.
- Produces: `transition()` stamps `started` when `to == Doing && task.started.is_none()`, stamps `completed` when `completing`, clears `completed` when `task.status == Done && to != Done`; `check_invariants` errors `started is stamped by starting the task; it cannot be edited` / `completed is stamped by completing the task; it cannot be edited`; `check` warning kind `completed_stamp_on_open_task`.

- [ ] **Step 1: Write the failing end-to-end tests**

In `tests/cli.rs`, after `edit_status_done_and_editor_flips_stamp_like_done`:

```rust
#[test]
fn start_stamps_started_once_and_later_starts_leave_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    assert!(env.json(&sci, &["show", &id])["task"]["started"].is_null());

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let first = env.json(&sci, &["show", &id])["task"]["started"].clone();
    assert!(first.is_string(), "{first}");

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "resume later"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["started"], first, "resume");

    as_agent(&env, &sci, "agent-b")
        .args(["start", &id, "--force"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["started"], first, "takeover");

    // The claim holder reopens and restarts through `edit --status`; both funnel
    // through `transition()` and neither moves the stamp.
    as_agent(&env, &sci, "agent-b")
        .args(["edit", &id, "--status", "todo"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["edit", &id, "--status", "doing"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["started"], first, "edit --status doing");

    // Summary and parked rows carry the stamps too (spec §3 "JSON").
    assert_eq!(env.json(&sci, &["list"])["tasks"][0]["started"], first);
    assert!(env.json(&sci, &["list"])["tasks"][0]["completed"].is_null());
    as_agent(&env, &sci, "agent-b")
        .args(["park", &id, "later", "--waiting-on", "user"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["prime"])["parked"][0]["started"], first);
}

#[test]
fn done_stamps_completed_and_reopening_clears_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["start", &id]);
    env.json(&sci, &["done", &id, "first pass"]);
    let v = env.json(&sci, &["show", &id]);
    let first = v["task"]["completed"].clone();
    assert!(first.is_string(), "{v}");
    assert!(v["task"]["started"].is_string(), "started survives completion");

    env.json(&sci, &["edit", &id, "--status", "todo"]);
    let v = env.json(&sci, &["show", &id]);
    assert!(v["task"]["completed"].is_null(), "reopen clears: {v}");
    assert!(v["task"]["started"].is_string(), "reopen leaves started: {v}");

    std::thread::sleep(std::time::Duration::from_millis(1100));
    env.json(&sci, &["start", &id]);
    env.json(&sci, &["done", &id, "second pass"]);
    let second = env.json(&sci, &["show", &id])["task"]["completed"].clone();
    assert!(second.is_string());
    assert_ne!(second, first, "a fresh completion restamps");
}

#[test]
fn editor_reopen_keeps_the_completed_line_and_transition_clears_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["done", &id, "landed"]);
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("\ncompleted: "), "{raw}");

    // The saved text flips the status and keeps the stamp line: it must parse, and the
    // transition then clears the stamp (spec §3 "Validation").
    let editor = editor_script(&sci, "sed -i 's/^status: done$/status: todo/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo");
    assert!(v["task"]["completed"].is_null(), "{v}");
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(!raw.contains("\ncompleted: "), "{raw}");
}

#[test]
fn recurring_completion_stamps_both_and_recovery_stamps_neither() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.json(&sci, &["done", &id, "first"]);
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(
        v["task"]["completed"], v["periodic"]["last_done"],
        "one instant for both stamps: {v}"
    );
    let first = v["task"]["completed"].clone();

    // Reopening the due occurrence clears the telemetry stamp and keeps the anchor.
    env.json(&sci, &["start", &id]);
    let v = env.json(&sci, &["show", &id]);
    assert!(v["task"]["completed"].is_null(), "{v}");
    assert_eq!(v["periodic"]["last_done"], first, "the anchor survives: {v}");
    assert!(v["task"]["started"].is_string());

    std::thread::sleep(std::time::Duration::from_millis(1100));
    env.json(&sci, &["done", &id, "second"]);
    let second = env.json(&sci, &["show", &id])["task"]["completed"].clone();
    assert_ne!(second, first);

    // An interrupted completion whose claim cleanup failed: the retry takes the
    // claim-release branch, which is not a completion and stamps nothing.
    env.json(&sci, &["start", &id]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: doing", "status: done")).unwrap();
    let before = env.json(&sci, &["show", &id])["task"]["completed"].clone();
    assert!(before.is_null(), "the reopen cleared it; the hand edit did not restore it");
    let out = env
        .cmd(&sci)
        .args(["done", &id, "cleanup"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(value["warnings"].to_string().contains("already completed"), "{value}");
    assert!(
        env.json(&sci, &["show", &id])["task"]["completed"].is_null(),
        "the recovery branch stamps nothing"
    );
}

#[test]
fn editor_refuses_to_set_move_or_clear_a_stamp() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["start", &id]);
    let started = env.json(&sci, &["show", &id])["task"]["started"]
        .as_str()
        .unwrap()
        .to_string();

    let cases: [(&str, &str); 3] = [
        (
            "set completed",
            "sed -i 's/^updated: \\(.*\\)$/updated: \\1\\ncompleted: 2026-09-01T00:00:00Z/' \"$1\"",
        ),
        ("move started", "sed -i 's/^started: .*$/started: 2020-01-01T00:00:00Z/' \"$1\""),
        ("clear started", "sed -i '/^started: /d' \"$1\""),
    ];
    for (label, body) in cases {
        let editor = editor_script(&sci, body);
        let out = env
            .cmd(&sci)
            .args(["edit", &id])
            .env("EDITOR", &editor)
            .output()
            .unwrap();
        assert!(!out.status.success(), "{label} must be refused");
        assert_eq!(err_kind(&out), "validation", "{label}");
        assert!(
            err_detail(&out).contains("cannot be edited"),
            "{label}: {}",
            err_detail(&out)
        );
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["started"], started,
            "{label}: nothing landed"
        );
    }
}

#[test]
fn check_reports_completed_on_an_open_record() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["done", &id, "landed"]);
    // A hand edit outside the binary reopens without dropping the stamp.
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: done", "status: todo")).unwrap();

    let v = env.json(&sci, &["check"]);
    assert!(v["errors"].as_array().unwrap().is_empty(), "it parses: {v}");
    let warnings = v["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{v}");
    assert_eq!(warnings[0]["kind"], "completed_stamp_on_open_task");
    assert_eq!(warnings[0]["id"], id);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli stamp 2>&1 | grep -E 'test .* (ok|FAILED)'`
Expected: `start_stamps_started_once_and_later_starts_leave_it`, `done_stamps_completed_and_reopening_clears_it`, `editor_reopen_keeps_the_completed_line_and_transition_clears_it`, `recurring_completion_stamps_both_and_recovery_stamps_neither`, `editor_refuses_to_set_move_or_clear_a_stamp`, and `check_reports_completed_on_an_open_record` all FAILED (the stamps stay null; the editor accepts the edits; `check` reports nothing).

- [ ] **Step 3: Stamp in `transition()`**

In `src/commands/mod.rs`, replace the block from `let completing = ...` through the end of the function with:

```rust
    let completing = to == Status::Done && task.status != Status::Done;
    // One instant for everything this transition stamps, so a recurring completion's
    // `completed` and `last_done` agree to the second (spec §3).
    let now = crate::time::now();
    if to == Status::Doing && task.started.is_none() {
        task.started = Some(now.clone());
    }
    if completing {
        task.model = completion_model()?;
        task.completed = Some(now.clone());
    } else if task.status == Status::Done && to != Status::Done {
        // Reopening, including `start` on a due recurrence: the telemetry stamp goes,
        // the cadence anchor stays.
        task.completed = None;
    }
    task.status = to;
    if completing && let Some(every) = task.every {
        let next = crate::periodic::add(crate::time::parse(&now)?, every).ok_or_else(|| {
            Error::Validation(format!(
                "completing now plus every {every} is not a representable timestamp"
            ))
        })?;
        task.last_done = Some(now);
        let owner = owner_name(&ctx.project)?;
        append_note(
            task,
            &owner,
            &format!("completed; next due {}", next.date()),
        )?;
    }
    Ok(())
```

The claim-release retry branch (`ctx.recovered`, `to == Done && task.status == Done`) reaches this block with `completing` false and the `else if` false, so it stamps nothing.

- [ ] **Step 4: Guard the stamps in `check_invariants`**

In `src/commands/edit.rs`, after the `last_done` check and before `Ok(())`:

```rust
    // Both stamps are written by `transition()` and by nothing else. Unlike `last_done`,
    // no other field's edit needs to clear them, so clearing is refused as well.
    if edited.started != original.started {
        return Err(Error::Validation(
            "started is stamped by starting the task; it cannot be edited".into(),
        ));
    }
    if edited.completed != original.completed {
        return Err(Error::Validation(
            "completed is stamped by completing the task; it cannot be edited".into(),
        ));
    }
```

- [ ] **Step 5: Report the inconsistent record from `check`**

In `src/commands/check.rs`, add `Status` to the `crate::model` import, and at the top of the `for task in &tasks` loop body, after `let file = ...;`:

```rust
        if let Some(completed) = &task.completed
            && task.status != Status::Done
        {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "completed_stamp_on_open_task",
                format!(
                    "completed {completed} but status is {}; only a hand edit outside tasks leaves this",
                    task.status.as_str()
                ),
            ));
        }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --test cli stamp 2>&1 | grep -E 'test .* (ok|FAILED)'` then `cargo test 2>&1 | tail -3`
Expected: the six new tests pass; the whole suite is green. If `recurring_restamps_and_recovery_preserves_the_stamp` or any periodic test breaks, the `now` refactor changed the anchor value — `last_done` must still be the completion instant, which `Some(now)` is.

- [ ] **Step 7: Gate and commit**

```bash
just check
git add src/commands/mod.rs src/commands/edit.rs src/commands/check.rs tests/cli.rs
tasks done tasks-1a8e3b "transition stamps started and completed; editor refuses to touch them; check reports a completed stamp on an open record"
git add tasks/
git commit -m "feat(transition): stamp started and completed, guard them from edits, report drift"
```

---

### Task 3: park --reason on the store, the note, and every park view

**Files:**
- Modify: `src/claims.rs:95-140` (`Reason` beside `WaitingOn`; `Park::reason`), `:970` (test literal), tests module (after `waiting_on_parses_its_two_values_only`, `:955`)
- Modify: `src/cli.rs:279-292` (`Park` args), `src/commands/mod.rs:895-899` (dispatch), `src/commands/park.rs:8-58`
- Modify: `src/output.rs:187-208` (`ParkInfo` + `of`), `:899-905` (pretty `show`), `:1047-1052` (`parked_table`)
- Modify: `src/complete.rs:43` (add `reason` beside `waiting_on`)
- Test: `tests/cli.rs`, after `park_refuses_a_closed_task_and_validates_its_arguments` (`:680`)

**Interfaces:**
- Produces: `claims::Reason` (`ALL`, `parse`, `as_str`); `claims::describe_stop(who: WaitingOn, reason: Option<Reason>) -> String` returning `"user"` or `"user, review"`; `Park::reason: Option<Reason>`; `ParkInfo::reason: Option<Reason>` (JSON `park.reason`); `park --reason <WHY>`; `complete::reason()`.
- Consumed by: Task 4 documents the flag and the vocabulary.

- [ ] **Step 1: Write the failing unit tests**

In `src/claims.rs`'s tests module, after `waiting_on_parses_its_two_values_only`:

```rust
    #[test]
    fn reason_parses_its_six_values_only() {
        for (text, reason) in [
            ("review", Reason::Review),
            ("decision", Reason::Decision),
            ("approval", Reason::Approval),
            ("environment", Reason::Environment),
            ("dependency", Reason::Dependency),
            ("session", Reason::Session),
        ] {
            assert_eq!(Reason::parse(text).unwrap(), reason);
            assert_eq!(reason.as_str(), text);
        }
        match Reason::parse("boredom") {
            Err(Error::Validation(detail)) => {
                assert!(detail.contains("review, decision, approval, environment, dependency, session"), "{detail}");
                assert!(detail.contains("\"boredom\""), "{detail}");
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn describe_stop_names_the_who_and_the_reason_when_given() {
        assert_eq!(describe_stop(WaitingOn::User, None), "user");
        assert_eq!(describe_stop(WaitingOn::Agent, Some(Reason::Session)), "agent, session");
    }

    #[test]
    fn a_park_without_a_reason_loads_and_a_park_with_one_writes_the_key() {
        let (dir, mut store) = store_from(A_PARK);
        let two = TaskId::parse("sci-000002").unwrap();
        assert_eq!(store.park(&two).unwrap().reason, None, "pre-reason store files load");

        let three = TaskId::parse("sci-000003").unwrap();
        store.insert_park(
            &three,
            Park {
                owner: "o".into(),
                session: "a".into(),
                host: "h".into(),
                worktree: "/w".into(),
                at: "2026-09-11T10:00:00Z".into(),
                next_step: "open the sheet".into(),
                waiting_on: WaitingOn::User,
                reason: Some(Reason::Review),
                title: "T".into(),
            },
        );
        store.save().unwrap();
        let path = dir.path().join("sci.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("reason = \"review\""), "{text}");
        assert_eq!(
            text.matches("reason =").count(),
            1,
            "the reason-less park writes no key: {text}"
        );
        let reloaded = ClaimStore::load_from(&path).unwrap();
        assert_eq!(reloaded.park(&three).unwrap().reason, Some(Reason::Review));
    }
```

- [ ] **Step 2: Write the failing end-to-end tests**

In `tests/cli.rs`, after `park_refuses_a_closed_task_and_validates_its_arguments`:

```rust
#[test]
fn park_reason_rides_the_entry_the_note_and_every_park_view() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "open the round-02 sheet", "--waiting-on", "user", "--reason", "review"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "review");
    assert_eq!(v["park"]["waiting_on"], "user");
    assert_eq!(env.json(&sci, &["list", "--parked"])["tasks"][0]["park"]["reason"], "review");
    assert_eq!(env.json(&sci, &["prime"])["parked"][0]["park"]["reason"], "review");
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("parked (waiting on user, review): open the round-02 sheet"),
        "{raw}"
    );
    let text = env.pretty(&sci, &["show", &id]);
    assert!(text.contains("waiting on user, review since"), "{text}");
    let table = env.pretty(&sci, &["list", "--parked"]);
    assert!(table.contains("waits on user, review"), "{table}");

    // `next` hands back agent-parked work with the same block.
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "rerun after the restart", "--reason", "environment"])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["task"]["id"], id);
    assert_eq!(next["park"]["reason"], "environment");
    assert_eq!(next["park"]["waiting_on"], "agent");
}

#[test]
fn park_without_a_reason_records_none_and_re_parking_drops_a_previous_one() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "write §3"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"]["reason"].is_null(), "{v}");
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("parked (waiting on agent): write §3"), "old note form: {raw}");

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "decide the shape", "--waiting-on", "user", "--reason", "decision"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["park"]["reason"], "decision");
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "decide the shape", "--waiting-on", "user"])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["park"]["reason"].is_null(),
        "a re-park without the flag records none"
    );

    assert_eq!(
        env.fail(&sci, &["park", &id, "x", "--reason", "boredom"]),
        "validation"
    );
    assert!(
        env.json(&sci, &["show", &id])["park"]["reason"].is_null(),
        "nothing landed"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --bin tasks claims::tests::reason 2>&1 | tail -5` and `cargo test --test cli park_reason park_without 2>&1 | tail -5`
Expected: compile errors — `Reason` and `describe_stop` are undefined; `Park` has no field `reason`.

- [ ] **Step 4: Add `Reason`, `describe_stop`, and `Park::reason`**

In `src/claims.rs`, directly after the `impl WaitingOn` block:

```rust
/// Why the work stopped, from the six-word vocabulary of
/// docs/specs/2026-09-11-park-reason-and-stamps-design.md §4. Orthogonal to `WaitingOn`:
/// the readers act on who, this only describes the stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Review,
    Decision,
    Approval,
    Environment,
    Dependency,
    Session,
}

impl Reason {
    pub const ALL: [Reason; 6] = [
        Reason::Review,
        Reason::Decision,
        Reason::Approval,
        Reason::Environment,
        Reason::Dependency,
        Reason::Session,
    ];

    pub fn parse(s: &str) -> Result<Reason> {
        Reason::ALL
            .into_iter()
            .find(|reason| reason.as_str() == s)
            .ok_or_else(|| {
                let accepted: Vec<&str> = Reason::ALL.iter().map(|reason| reason.as_str()).collect();
                Error::Validation(format!(
                    "--reason must be one of {}, got {s:?}",
                    accepted.join(", ")
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Review => "review",
            Reason::Decision => "decision",
            Reason::Approval => "approval",
            Reason::Environment => "environment",
            Reason::Dependency => "dependency",
            Reason::Session => "session",
        }
    }
}

/// The parenthetical the park note and the park views share: `user` or `user, review`.
pub fn describe_stop(who: WaitingOn, reason: Option<Reason>) -> String {
    match reason {
        Some(reason) => format!("{}, {}", who.as_str(), reason.as_str()),
        None => who.as_str().to_string(),
    }
}
```

In `pub struct Park`, after `pub waiting_on: WaitingOn,`:

```rust
    /// Why the work stopped, when the parking session said. Absent in store files written
    /// before the vocabulary existed and whenever `park` ran without `--reason`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
```

Add `reason: None,` to the `Park` literal in `inserting_a_park_displaces_a_claim_on_the_same_id` (`src/claims.rs:970`).

- [ ] **Step 5: Thread the flag through cli, dispatch, and the park command**

In `src/cli.rs`, in the `Park` variant after `waiting_on: String,`:

```rust
        /// Why the work stopped: review, decision, approval, environment, dependency, or
        /// session. Optional; absent means not recorded.
        #[arg(
            long,
            value_name = "WHY",
            add = ArgValueCandidates::new(crate::complete::reason)
        )]
        reason: Option<String>,
```

In `src/complete.rs`, after `waiting_on()`:

```rust
/// The six `park --reason` accepts.
pub fn reason() -> Vec<CompletionCandidate> {
    plain(crate::claims::Reason::ALL.iter().map(|reason| reason.as_str()))
}
```

In `src/commands/mod.rs`, the `Command::Park { .. }` arm destructures `reason` too and passes it: `park::run(open_id_write_ctx(dir, &id)?, id, next_step, waiting_on, reason)`.

In `src/commands/park.rs`:

```rust
use crate::claims::{Liveness, Park, Reason, WaitingOn, describe_stop};

pub fn run(
    mut ctx: Ctx,
    id: String,
    next_step: String,
    waiting_on: String,
    reason: Option<String>,
) -> Result<Output> {
```

after `let waiting_on = WaitingOn::parse(&waiting_on)?;`:

```rust
    let reason = reason.as_deref().map(Reason::parse).transpose()?;
```

the note becomes:

```rust
    append_note(
        &mut task,
        &owner,
        &format!(
            "parked (waiting on {}): {next_step}",
            describe_stop(waiting_on, reason)
        ),
    )?;
```

and the `Park { .. }` literal gains `reason,` after `waiting_on,`.

- [ ] **Step 6: Surface it on `ParkInfo` and the pretty views**

In `src/output.rs`, `ParkInfo` gains `pub reason: Option<crate::claims::Reason>,` after `waiting_on`, and `ParkInfo::of` sets `reason: park.reason,`.

Pretty `show` (the `# parked` block):

```rust
        rendered.push_str(&format!(
            "- waiting on {} since {}: {}\n",
            crate::claims::describe_stop(park.waiting_on, park.reason),
            crate::time::day(&park.at),
            park.next_step
        ));
```

`parked_table` row:

```rust
        rendered.push_str(&format!(
            "{id}  {status} {phase} waits on {:<18} {}  {}\n",
            crate::claims::describe_stop(park.waiting_on, park.reason),
            crate::time::day(&park.at),
            row.title
        ));
```

`{:<18}` fits the longest stop, `agent, environment`. If an existing pretty test asserts on the exact old spacing after `waits on user`, widen its expectation; the substring assertions in this plan's tests do not depend on width.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test 2>&1 | tail -3`
Expected: green, including the three claims units and the two park end-to-end tests. Then `cargo install --path .` so the installed `tasks` completes `--reason`.

- [ ] **Step 8: Gate and commit**

```bash
just check
git add src/claims.rs src/cli.rs src/complete.rs src/commands/mod.rs src/commands/park.rs src/output.rs tests/cli.rs
tasks done tasks-a7387b "park --reason with the six-word vocabulary on the store entry, the note, and park.reason in every view"
git add tasks/
git commit -m "feat(park): --reason records why the work stopped"
```

---

### Task 4: Documentation and closeout

**Files:**
- Modify: `skills/tasks/SKILL.md:35-39` (session protocol step 5)
- Modify: `README.md:58-59` (the park paragraph under "For agents") and `:92-93` (the `Use` examples)
- Modify: `docs/specs/2026-09-11-park-reason-and-stamps-design.md:3` (status), `:117-121` (§5 Docs)
- Modify: `docs/plans/2026-09-11-park-reason-and-stamps.md:3` (status)

**Interfaces:**
- Consumes: everything above. Produces nothing in code.

- [ ] **Step 1: The skill**

In `skills/tasks/SKILL.md`, step 5 currently reads:

```
5. `tasks park <id> "<next step>" [--waiting-on user]` before ending a turn that waits on
   the user, or whenever you set work down. It records the next step and this session in
   the shared store, releases your claim, and leaves status alone; `start` resumes it.
```

Extend it so the whole step reads:

```
5. `tasks park <id> "<next step>" [--waiting-on user] [--reason <why>]` before ending a
   turn that waits on the user, or whenever you set work down. It records the next step
   and this session in the shared store, releases your claim, and leaves status alone;
   `start` resumes it. Add `--reason` when one of these fits, and leave it off otherwise:
   `review` (the user must inspect and judge an artifact), `decision` (only the user can
   decide), `approval` (you hold a recommendation and want it confirmed), `environment`
   (the checkout or machine cannot run the work), `dependency` (another task or project
   must land first), `session` (the session is ending before the work is).
```

- [ ] **Step 2: The README**

Replace the park paragraph at `README.md:58-59` with:

```
`park` sets a task down with its next step in the same store, who it waits on, and
optionally why — `--reason review|decision|approval|environment|dependency|session`;
`start` resumes it, and `prime` lists parked work first. Every record also carries two
stamps written only by status changes: `started`, the first time work began, and
`completed`, the latest completion (cleared by a reopen). Design:
`docs/specs/2026-09-11-park-reason-and-stamps-design.md`.
```

At `README.md:93`, change the example to:

```
    tasks park <id> "next step" --reason review   # set it down; tasks list --parked to see what is parked
```

- [ ] **Step 3: Status lines and the spec's §5**

Spec line 3: `Status: implemented (<today's date>)`. Plan line 3: `**Status:** implemented (<today's date>)`. In the spec's §5, replace "The README's park section gains the vocabulary table, and its record-format section lists `started` and `completed` beside `last_done`." with "The README's park paragraph names the vocabulary and the two stamps in prose; it has no record-format section to extend." — the README has no such section, and the spec should not claim one was edited.

- [ ] **Step 4: Verify the docs and the tree agree, then close**

Run: `just check` (ops-check verifies every backticked path in README.md and AGENTS.md exists) and `tasks check`.
Expected: `{"errors":[],"warnings":[]}`.

```bash
git add skills/tasks/SKILL.md README.md docs/specs/2026-09-11-park-reason-and-stamps-design.md docs/plans/2026-09-11-park-reason-and-stamps.md
tasks done tasks-8da6fa "skill step 5, README, and both status lines updated"
tasks done tasks-82b559 "started/completed stamps and park --reason landed with tests and docs"
git add tasks/
git commit -m "docs(park): document --reason and the lifecycle stamps; close tasks-82b559"
```

Then hand the branch to the finishing-a-development-branch skill: the worktree is `.worktrees/waiting-on` on branch `waiting-on`, to be merged into `main` of the tasks repository.
