# Model Provenance Implementation Plan

**Status:** approved (2026-09-10)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every fresh completion of a task stamps an optional opaque `model` field from the harness-exported `TASKS_MODEL`, correctable via `tasks edit --model/--no-model`, and the field rides the JSON contract (`show`/`next` full task, `TaskSummary`, `ParkedRow`) so `tasks list --status done | jq` can answer "which model completed what".

**Architecture:** One new optional string on `Task`, threaded exactly the way `source` is (model.rs field, format.rs `KEYS`/parse/validate/serialize, additive JSON keys). The write path is a single stamp inside `transition()` gated on `completing` (a fresh transition into `done`), so `done`, `edit --status done`, and editor-driven flips all stamp with one rule while the claim-release retry branch (`ctx.recovered`) and reopens provably never touch the field. Correction flags live on `EditArgs` only; a completing edit stamps last, so corrections are a subsequent edit.

**Tech Stack:** Rust 2024, clap 4.6, serde. No new dependencies.

**Spec:** `docs/specs/2026-09-10-model-provenance-design.md`

**Task:** `tasks-222dab`

## Global Constraints

- **JSON output is the contract; every change here is additive.** New key `model` (null when absent) on the full `Task` in `show`/`next` (free via the derived `Serialize`), on `TaskSummary` (`list`/`ready`/`prime`/`tree`), and on `ParkedRow`. Nothing existing changes shape or meaning. (`AGENTS.md`; spec "JSON" rule)
- **The value is opaque.** Non-empty, single-line; never interpreted, resolved, or validated against a registry. Same rule and error text shape as `source` (`format::validate_line`). (spec "Value" rule)
- **Supply is env-only.** `TASKS_MODEL` read at completion time; unset or empty records nothing and clears on a fresh completion; non-Unicode is an explicit validation error naming the variable. No flag on `done`; no harness auto-detection. (spec "Supply"/"Recompletion clears" rules)
- **This crate has no library target, and every module is private.** An unused `pub fn` or unread struct field fails `cargo clippy --all-targets -- -D warnings` with `dead_code`; use from `#[cfg(test)]` does not count. Every helper below is introduced in the task whose production code consumes it. Unit tests run as `cargo test --bin tasks <filter>`; end-to-end tests as `cargo test --test cli <filter>`.
- **Gates.** `just check` before every commit (fmt, clippy `-D warnings`, `tasks check`); `just test` is `cargo test`. Rebuild and reinstall after CLI changes so the tracker in use is the code under test: `cargo install --path .`.
- **Struct-literal sites that break when a field is added** (each task names the ones it touches): `Task` (`src/commands/add.rs:29`, test helper `task_with` at `src/model.rs:82`); `TaskSummary` (constructor at `src/output.rs:253`, test fixture at `src/output.rs:1106`); `ParkedRow` (`resolved` at `src/output.rs:297`, `unresolved` at `src/output.rs:316`).
- **End-to-end tests isolate the environment.** `TestEnv::cmd`/`raw` set `HOME` to a temp dir and scrub the process environment; Task 2 adds `TASKS_MODEL` to both scrub lists before any stamping code exists, so a harness that exports the variable can never change an unrelated test's outcome. Tests that need the variable set it explicitly with `.env("TASKS_MODEL", ...)` on the command, the way `as_agent` sets `TASKS_SESSION` (tests/cli.rs:4805).
- **Commit hygiene.** Never `git add -A`: two untracked feedback reports (`tasks/tasks-74a525.md`, `tasks/tasks-b5add6.md`) are unrelated and stay out. Conventional commits; no AI-attribution trailers.
- **Each plan task has a step child under `tasks-222dab`**; the ids are listed at the end of this header. `tasks start <step>` before its first step and `tasks done <step> "<what landed>"` in the same commit as its code; the commit blocks below include the `done`.

**Step children:** Task 1 `tasks-332ea5`, Task 2 `tasks-e84da5`, Task 3 `tasks-d132e6`, Task 4 `tasks-ec0953`, Task 5 `tasks-6bd197`.

---

### Task 1: The `model` field on the record

**Files:**
- Modify: `src/model.rs:329` (field after `source`), `:82` (test helper `task_with`)
- Modify: `src/format.rs:5` (`KEYS`), `:121` (parse), `:304` (`validate_task`), `:356` (`serialize_task`)
- Modify: `src/commands/add.rs:29` (Task literal)
- Test: `src/format.rs` tests module (after the `source` tests at `:628-685`)

**Interfaces:**
- Produces: `Task::model: Option<String>`; `model` accepted in frontmatter between `source` and `spec` and round-tripped byte-for-byte; `validate_task` rejects empty/multi-line values with `model must not be empty` / `model must be a single line`.
- Consumed by: every later task. Task 2 writes it; Tasks 3 and 4 surface it.

- [ ] **Step 1: Write the failing unit tests**

In `src/format.rs`'s tests module, after `rejects_empty_or_multiline_source` (`:667`):

```rust
    #[test]
    fn model_round_trips_bare_and_sits_between_source_and_spec() {
        let text = MINIMAL.replace(
            "tags: []",
            "tags: []\nsource: keep-note-42\nmodel: claude-fable-5-1\nspec: docs/specs/x.md",
        );
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.model.as_deref(), Some("claude-fable-5-1"));
        let out = serialize_task(&t);
        assert!(
            out.contains("source: keep-note-42\nmodel: claude-fable-5-1\nspec: docs/specs/x.md\n"),
            "{out}"
        );
    }

    #[test]
    fn model_with_a_colon_is_quoted_on_write_and_unquoted_on_read() {
        let text = MINIMAL.replace("tags: []", "tags: []\nmodel: \"urn:x:y\"");
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.model.as_deref(), Some("urn:x:y"));
        let out = serialize_task(&t);
        assert!(out.contains("model: \"urn:x:y\"\n"), "{out}");
    }

    #[test]
    fn model_is_omitted_when_absent() {
        let t = parse_task(MINIMAL, "x").unwrap();
        assert_eq!(t.model, None);
        assert!(!serialize_task(&t).contains("model:"));
    }

    #[test]
    fn rejects_empty_or_multiline_model() {
        let err =
            parse_task(&MINIMAL.replace("tags: []", "tags: []\nmodel: \"\""), "x").unwrap_err();
        assert!(
            err.to_string().contains("model must not be empty"),
            "{err}"
        );
        let mut t = parse_task(MINIMAL, "x").unwrap();
        t.model = Some("a\nb".into());
        let err = validate_task(&t).unwrap_err();
        assert!(
            err.to_string().contains("model must be a single line"),
            "{err}"
        );
        t.model = Some(String::new());
        let err = validate_task(&t).unwrap_err();
        assert!(
            err.to_string().contains("model must not be empty"),
            "{err}"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks model`
Expected: FAIL to compile (`no field model` on `Task`).

- [ ] **Step 3: Add the field and thread it through the record layer**

In `src/model.rs`, after the `source` field (`:329`):

```rust
    /// The model id the harness reported (`TASKS_MODEL`) for the session that ran the
    /// latest completion transition; `None` when unknown or cleared. Stored and returned,
    /// never interpreted. See docs/specs/2026-09-10-model-provenance-design.md.
    pub model: Option<String>,
```

In the same file's test helper `task_with` (`:82`), add `model: None,` after `source: None,`.

In `src/commands/add.rs` (`:29`), add `model: None,` after `source: None,`.

In `src/format.rs`:
- `const KEYS: [&str; 19]` (was 18), inserting `"model",` after `"source",`.
- In `parse_task`, after `source: scalar("source")?,` (`:121`): `model: scalar("model")?,`.
- In `validate_task`, after the `source` block (`:304-306`):

```rust
    if let Some(model) = &t.model {
        validate_line("model", model)?;
    }
```

- In `serialize_task`, after the `source` push (`:356-358`):

```rust
    if let Some(v) = &t.model {
        pairs.push(("model".into(), s(v)));
    }
```

- [ ] **Step 4: Run the tests and the gate**

Run: `cargo test --bin tasks model && just check`
Expected: the four new tests PASS; fmt/clippy/`tasks check` clean.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-332ea5 "model field on Task, parsed/validated/serialized between source and spec"
git add src/model.rs src/format.rs src/commands/add.rs tasks/tasks-332ea5.md tasks/tasks-222dab.md
git commit -m "feat(provenance): add the model field to the task record"
```

---

### Task 2: Completion stamps `model` from `TASKS_MODEL`

**Files:**
- Modify: `src/commands/mod.rs` (`transition` at `:531`, new helper beside it)
- Modify: `tests/common/mod.rs:20-36` (`cmd`), `:38-55` (`raw`)
- Test: `tests/cli.rs` (new tests; helpers `id_of` `:214`, `editor_script` `:2166`, `as_agent` `:4805`)

**Interfaces:**
- Consumes: `Task::model` (Task 1); `format::validate_line` (already `pub`).
- Produces: on any fresh transition into `done`, `task.model` is set from `TASKS_MODEL` (`Some`/`None`, replacing whatever was there); a non-Unicode value fails the command with `TASKS_MODEL is not valid Unicode`. No other transition touches the field.

- [ ] **Step 1: Scrub the variable in both test command builders**

In `tests/common/mod.rs`, add to `cmd` (after the `TASKS_SESSION_PID` removal) and to the identical block in `raw`:

```rust
            .env_remove("TASKS_MODEL")
```

This lands first so every existing completion test is hermetic against the caller's environment before the stamp exists.

- [ ] **Step 2: Write the failing end-to-end tests**

In `tests/cli.rs`, after `completing_a_recurrence_anchors_and_notes_every_completion_path` (`:2199`):

```rust
#[test]
fn done_stamps_model_from_tasks_model_and_absence_records_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let stamped = id_of(env.json(&sci, &["add", "Stamped", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &stamped, "landed"])
        .env("TASKS_MODEL", "claude-fable-5-1")
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &stamped])["task"]["model"],
        "claude-fable-5-1"
    );

    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    env.json(&sci, &["done", &plain, "landed"]);
    assert!(env.json(&sci, &["show", &plain])["task"]["model"].is_null());
}

#[test]
fn recompletion_without_the_variable_clears_the_stamp() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Rework", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "first pass"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "A");
    env.json(&sci, &["edit", &id, "--status", "todo"]);
    env.json(&sci, &["done", &id, "second pass"]);
    assert!(
        env.json(&sci, &["show", &id])["task"]["model"].is_null(),
        "a fresh completion with no TASKS_MODEL records unknown, not the stale stamp"
    );
}

#[test]
fn recurring_occurrence_restamps_and_retries_preserve_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.cmd(&sci)
        .args(["done", &id, "first"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    env.json(&sci, &["start", &id]);
    env.cmd(&sci)
        .args(["done", &id, "second"])
        .env("TASKS_MODEL", "B")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");

    // A bare repeat after a clean completion has no claim to release: it errors
    // before any write and cannot restamp.
    env.cmd(&sci)
        .args(["done", &id, "third"])
        .env("TASKS_MODEL", "C")
        .assert()
        .failure();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");

    // Simulate an interrupted completion whose claim cleanup failed: the record
    // says done while this checkout still holds the claim (the arrangement
    // cleanup_retry_releases_this_checkouts_claim_without_a_second_occurrence
    // proves reaches the claim-release branch).
    env.json(&sci, &["start", &id]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("status: doing", "status: done")
            .replace("updated: ", "last_done: 2026-01-01T00:00:00Z\nupdated: "),
    )
    .unwrap();
    let out = env
        .cmd(&sci)
        .args(["done", &id, "cleanup"])
        .env("TASKS_MODEL", "C")
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(
        value["warnings"].to_string().contains("already completed"),
        "{value}"
    );
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["model"],
        "B",
        "the claim-release branch is not a completion and never restamps"
    );
}

#[test]
fn edit_status_done_and_editor_flips_stamp_like_done() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let via_flag = id_of(env.json(&sci, &["add", "Flag", "-p", "2"]));
    env.cmd(&sci)
        .args(["edit", &via_flag, "--status", "done"])
        .env("TASKS_MODEL", "E")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &via_flag])["task"]["model"], "E");

    let via_editor = id_of(env.json(&sci, &["add", "Editor", "-p", "2"]));
    let editor = editor_script(&sci, "sed -i 's/^status: todo$/status: done/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &via_editor])
        .env("EDITOR", &editor)
        .env("TASKS_MODEL", "F")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &via_editor])["task"]["model"], "F");
}

#[test]
fn non_unicode_tasks_model_fails_the_completion() {
    use std::os::unix::ffi::OsStrExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Bytes", "-p", "2"]));
    let out = env
        .cmd(&sci)
        .args(["done", &id, "x"])
        .env("TASKS_MODEL", std::ffi::OsStr::from_bytes(b"\xff\xfe"))
        .assert()
        .failure();
    let err = String::from_utf8_lossy(&out.get_output().stderr);
    assert!(err.contains("TASKS_MODEL is not valid Unicode"), "{err}");
    assert!(
        env.json(&sci, &["show", &id])["task"]["model"].is_null(),
        "a failed completion writes nothing"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --test cli model`
Expected: the new tests FAIL (or compile-fail on the missing field until Task 1 lands): the stamp assertions get `null`.

- [ ] **Step 4: Implement the stamp**

In `src/commands/mod.rs`, beside `transition` (`:531`):

```rust
/// The model the harness reports for this completion, from `TASKS_MODEL`. Unset or
/// empty records nothing; a non-Unicode value is an explicit error, never a silent skip.
fn completion_model() -> Result<Option<String>> {
    match std::env::var_os("TASKS_MODEL") {
        None => Ok(None),
        Some(value) => {
            let value = value.into_string().map_err(|_| {
                Error::Validation("TASKS_MODEL is not valid Unicode".into())
            })?;
            if value.is_empty() {
                return Ok(None);
            }
            crate::format::validate_line("TASKS_MODEL", &value)?;
            Ok(Some(value))
        }
    }
}
```

In `transition`, between `let completing = ...` (`:592`) and `task.status = to;` (`:593`):

```rust
    if completing {
        task.model = completion_model()?;
    }
```

The recovered claim-release branch (`:540-568`) falls through with `completing == false`, so retries never restamp; reopens transition out of `done`, so they never stamp either. Erroring before `task.status = to;` keeps a bad variable from half-applying a completion.

- [ ] **Step 5: Run the tests and the gate**

Run: `cargo test --test cli model && cargo test --test cli && just check`
Expected: new tests PASS; the whole e2e suite still PASS (proves the scrub kept it hermetic); gate clean.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-e84da5 "fresh completions stamp model from TASKS_MODEL; retries and reopens never touch it"
git add src/commands/mod.rs tests/common/mod.rs tests/cli.rs tasks/tasks-e84da5.md tasks/tasks-222dab.md
git commit -m "feat(provenance): stamp the completing model from TASKS_MODEL"
```

---

### Task 3: `edit --model` / `--no-model` correction flags

**Files:**
- Modify: `src/cli.rs:101-104` (EditArgs, after `no_source`)
- Modify: `src/commands/edit.rs:55` (`has_flags`), `:82-84` (flag application in `run`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Task::model` (Task 1); the stamp (Task 2) — the completing-edit precedence test depends on `transition` stamping after `apply_fields` returns.
- Produces: `EditArgs { model: Option<String>, no_model: bool }`; corrections apply in `run` before the status block, so a same-invocation completion overwrites them (spec "Correction" rule).

- [ ] **Step 1: Write the failing end-to-end tests**

In `tests/cli.rs`, after the Task 2 tests:

```rust
#[test]
fn edit_model_replaces_and_no_model_clears() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Fix", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    env.json(&sci, &["edit", &id, "--model", "B"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");
    env.json(&sci, &["edit", &id, "--no-model"]);
    assert!(env.json(&sci, &["show", &id])["task"]["model"].is_null());
    env.fail(&sci, &["edit", &id, "--model", ""]);
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--model", "x", "--no-model"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "conflicting flags must fail");
}

#[test]
fn a_completing_edit_stamps_last_over_a_same_invocation_correction() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    for extra in [&["--model", "B"][..], &["--no-model"][..]] {
        let id = id_of(env.json(&sci, &["add", "Both", "-p", "2"]));
        env.cmd(&sci)
            .args(["edit", &id, "--status", "done"])
            .args(extra)
            .env("TASKS_MODEL", "A")
            .assert()
            .success();
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["model"],
            "A",
            "the fresh completion's stamp wins over {extra:?}"
        );
        env.json(&sci, &["edit", &id, "--model", "B"]);
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["model"],
            "B",
            "the correction holds in a following, non-completing edit"
        );
    }
}
```

(The second test's loop reuses a completed-then-corrected task name; each iteration adds a fresh task, so no state leaks between `--model` and `--no-model`.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli model`
Expected: FAIL — `unexpected argument '--model'`.

- [ ] **Step 3: Add the flags and wire them**

In `src/cli.rs`, in `EditArgs` after `no_source` (`:101-103`):

```rust
    /// Replace the model stamp recorded at completion; see `--no-model`.
    #[arg(long)]
    pub model: Option<String>,
    /// Clear the model stamp.
    #[arg(long, conflicts_with = "model")]
    pub no_model: bool,
```

In `src/commands/edit.rs`:
- In `has_flags` (`:38-58`), add after `|| args.no_source`:

```rust
        || args.model.is_some()
        || args.no_model
```

- In `run`, after the `no_source` block (`:82-84`) and before `apply_fields` (`:100`):

```rust
    if args.no_model {
        task.model = None;
    }
    if let Some(model) = &args.model {
        task.model = Some(model.clone());
    }
```

This ordering is the precedence rule: corrections are applied to the in-memory record first, and the status block's `transition` (`:101-108`) stamps over them when the same invocation completes the task.

- [ ] **Step 4: Run the tests and the gate**

Run: `cargo test --test cli model && just check`
Expected: PASS; gate clean.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-d132e6 "edit --model/--no-model correct the stamp; a completing edit stamps last"
git add src/cli.rs src/commands/edit.rs tests/cli.rs tasks/tasks-d132e6.md tasks/tasks-222dab.md
git commit -m "feat(provenance): correct the model stamp through edit"
```

---

### Task 4: `model` in summary JSON and pretty output

**Files:**
- Modify: `src/output.rs:146` (`TaskSummary`), `:253` (constructor), `:282` (`ParkedRow`), `:305` (`resolved`), `:327` (`unresolved`), `:1106` (test fixture)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Task::model` (Task 1).
- Produces: `TaskSummary.model` and `ParkedRow.model` (`Option<String>`, serialized null when absent). The full-task JSON in `show`/`next` needs nothing: `Task` derives `Serialize`, so the key appeared with Task 1; this task's tests pin that down.

- [ ] **Step 1: Write the failing end-to-end tests**

In `tests/cli.rs`, after the Task 3 tests:

```rust
#[test]
fn summary_rows_and_parked_rows_carry_model() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let done_id = id_of(env.json(&sci, &["add", "Done", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &done_id, "landed"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    let rows = env.json(&sci, &["list", "--status", "done"]);
    let row = rows["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == done_id)
        .unwrap()
        .clone();
    assert_eq!(row["model"], "A", "list rows carry the stamp");

    let open_id = id_of(env.json(&sci, &["add", "Open", "-p", "2"]));
    let open = env.json(&sci, &["list"]);
    let row = open["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == open_id)
        .unwrap()
        .clone();
    assert!(row["model"].is_null(), "the key is present and null when absent");

    // A parked row for a previously completed task keeps its attribution.
    env.json(&sci, &["edit", &done_id, "--status", "todo"]);
    env.json(&sci, &["park", &done_id, "resume here"]);
    let prime = env.json(&sci, &["prime"]);
    let parked = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == done_id)
        .unwrap()
        .clone();
    assert_eq!(parked["model"], "A");
}

#[test]
fn show_pretty_prints_the_model_line() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Pretty", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_MODEL", "claude-fable-5-1")
        .assert()
        .success();
    let out = env
        .cmd(&sci)
        .args(["--pretty", "show", &id])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(text.contains("model: claude-fable-5-1\n"), "{text}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli model`
Expected: the row assertions FAIL (`model` key missing → null against `"A"`); the pretty test passes already (the line comes free from `serialize_task` via `paint_frontmatter`, output.rs:811) and is here to pin the behavior.

- [ ] **Step 3: Add the field to the summary rows**

In `src/output.rs`:
- `TaskSummary` (`:146`): `pub model: Option<String>,` after `pub source: Option<String>,`.
- The `TaskSummary` constructor (`:253`): `model: task.model.clone(),` after `source: task.source.clone(),`.
- `ParkedRow` (`:282`): `pub model: Option<String>,` after `pub source: Option<String>,`.
- `ParkedRow::resolved` (`:305`): `model: summary.model,` after `source: summary.source,`.
- `ParkedRow::unresolved` (`:327`): `model: None,` after `source: None,`.
- The `TaskSummary` test fixture (`:1106`): `model: None,` after `source: None,`.

Then `cargo check` and fix any remaining struct-literal site the compiler names.

- [ ] **Step 4: Run the tests and the gate**

Run: `cargo test --test cli model && cargo test && just check`
Expected: PASS everywhere; gate clean.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-ec0953 "model rides TaskSummary and ParkedRow; pretty show prints the line"
git add src/output.rs tests/cli.rs tasks/tasks-ec0953.md tasks/tasks-222dab.md
git commit -m "feat(provenance): carry model in summary JSON and pretty output"
```

---

### Task 5: Documentation and closeout

**Files:**
- Modify: `skills/tasks/SKILL.md` (session protocol step 6)
- Modify: `README.md:53` (where `TASKS_SESSION` is documented)
- Modify: `docs/specs/2026-09-10-model-provenance-design.md:3` (status header)

- [ ] **Step 1: Document the variable and the stamp**

In `skills/tasks/SKILL.md`, in session protocol step 6 (the `tasks done` paragraph), add after the first sentence's clause about dependencies:

```markdown
   When the harness exports `TASKS_MODEL` (the model id it is running), every fresh
   completion stamps the record's `model` field with it — latest-completion attribution,
   cleared by a recompletion without the variable; `tasks edit --model/--no-model`
   corrects it.
```

In `README.md`, beside the `TASKS_SESSION` documentation (`:53`):

```markdown
`TASKS_MODEL` per harness process records which model completed each task: a fresh
`done` stamps the record's `model` field from it (and clears the stamp when it is
unset); correct a wrong stamp with `tasks edit --model`/`--no-model`.
```

In `docs/specs/2026-09-10-model-provenance-design.md`, replace the status header:

```markdown
Status: implemented (2026-09-10)
```

- [ ] **Step 2: Rebuild, reinstall, and run the full gate**

```bash
cargo install --path .
just gate
```

Expected: `just test` and `just check` both clean, and the installed `tasks` binary is the code just landed (the session's own `done` below uses it).

- [ ] **Step 3: Close the children and the goal, then commit**

```bash
tasks done tasks-6bd197 "TASKS_MODEL documented in the skill and README; spec marked implemented"
tasks done tasks-222dab "model provenance landed: stamped at completion, correctable, in the JSON contract"
git add skills/tasks/SKILL.md README.md docs/specs/2026-09-10-model-provenance-design.md docs/plans/2026-09-10-model-provenance.md tasks/tasks-6bd197.md tasks/tasks-222dab.md
git commit -m "docs(provenance): document the model stamp and close the goal"
```

(The plan file itself is committed in this final change, with its status header updated to `**Status:** implemented (2026-09-10)`.)

---

## Self-Review Notes

- **Spec coverage:** Value rules → Task 1 (validation units). Supply/Write path/Recompletion clears → Task 2 (stamp + all four regression tests). Correction + precedence → Task 3. Frontmatter → Task 1. JSON/Pretty → Task 4. Docs → Task 5. Deferrals are honored by omission: no `list --model`, no extra params, no start/note stamping, no auto-detection.
- **Ordering constraint:** Task 2's env scrub (Step 1) lands in the same commit as the stamp; Task 3 depends on Task 2's stamp for the precedence test; Task 4 depends on Task 1's field. Tasks must run in order.
- **Type consistency:** `Task::model: Option<String>` (Task 1) is what `completion_model` writes (Task 2), what `EditArgs.model` clones into it (Task 3), and what `TaskSummary::of` clones out of it (Task 4).
