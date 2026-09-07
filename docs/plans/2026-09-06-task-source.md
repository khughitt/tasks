# Task Source Implementation Plan

**Status:** not started (2026-09-06)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give a task record one optional, opaque `source` string that says where the task came from, settable at `add`, replaceable and clearable at `edit`, carried in every JSON task object.

**Architecture:** One `Option<String>` on `Task`, parsed and written by `format.rs` between `tags` and `spec`, validated as a non-empty single line by the same `validate_line` that guards `title`. It reaches the CLI through the shared `FieldArgs` (`--source`) plus an `edit`-only `--no-source`, and reaches JSON through `Task` (for `show`/`next`) and a new `TaskSummary` field (for `list`/`ready`/`prime`/`tree`). Nothing reads or resolves the value.

**Tech Stack:** Rust 2024, clap 4.6, serde. No new dependencies.

**Spec:** `docs/specs/2026-09-06-task-source-design.md`

**Task:** `tasks-13a0b6`

## Global Constraints

- **JSON output is the contract.** Every change here is additive: one new key on `Task` and one on `TaskSummary`, `null` when absent. Never change an existing shape. (`AGENTS.md`)
- **This crate has no library target.** Unit tests run as `cargo test --bin tasks <filter>`; `cargo test --lib` fails with "no library targets found".
- **Fail early with a typed error; no silent fallbacks.** An empty or multi-line source is `Error::Validation` on every write path and `Error::Parse` when read from a file.
- **`:` is a reserved frontmatter character** (`src/frontmatter.rs:11`), so `render_scalar` quotes any value containing one and `parse_scalar` strips the quotes. Most sources contain `:`; the tests assert both spellings round-trip.
- **`Task` is constructed as a full struct literal in six places** (`src/commands/add.rs:13`, `src/format.rs:93`, and test helpers in `src/similarity.rs:91`, `src/hierarchy.rs:176`, `src/query.rs:208`, `src/repo.rs:472`). Adding the field breaks all six until each gains `source: None` (or the parsed value).
- **Gates.** `just check` before every commit (fmt, clippy `-D warnings`, `tasks check`); `just test` is `cargo test`. Rebuild and reinstall after CLI changes: `cargo install --path .`.
- **Each plan task has a step child under `tasks-13a0b6`** (Task 1: `tasks-6ddc18`, Task 2: `tasks-8480d6`, Task 3: `tasks-db2d78`, Task 4: `tasks-e3f36d`). `tasks start <step>` before its first step and `tasks done <step> "<what landed>"` in the same commit as its code; the commit blocks below include the `done`. The parent refuses `done` while any child is open, so Task 4 cannot close without them.
- **Composition > inheritance. Explicit > defensive.** No paths like `/home/<user>` in comments or docs.
- Conventional commits. **No AI-attribution trailers or footers.**

---

### Task 1: The `source` field in the model and on disk

**Files:**
- Modify: `src/model.rs:227` (the `Task` struct, after `tags`)
- Modify: `src/format.rs:6-10` (`KEYS`), `:93-118` (parse literal), `:227-265` (`validate_task`), `:280-312` (`serialize_task`)
- Modify: `src/commands/add.rs:13-30` (`blank`)
- Modify: `src/similarity.rs:91`, `src/hierarchy.rs:176`, `src/query.rs:208`, `src/repo.rs:472` (test helpers)
- Test: `src/format.rs` tests module (after `parallel_false_in_a_file_is_dropped_on_the_next_write`)

**Interfaces:**
- Produces: `Task.source: Option<String>`; frontmatter key `source`; `validate_task` rejects empty/multi-line.

- [ ] **Step 1: Write the failing unit tests**

Add to the `tests` module at the bottom of `src/format.rs`, after `parallel_false_in_a_file_is_dropped_on_the_next_write`:

```rust
    #[test]
    fn source_round_trips_bare_and_sits_after_tags() {
        let text = MINIMAL.replace("tags: []", "tags: []\nsource: keep-note-42");
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.source.as_deref(), Some("keep-note-42"));
        let out = serialize_task(&t);
        assert!(out.contains("tags: []\nsource: keep-note-42\n---\n"), "{out}");
        assert_eq!(parse_task(&out, "x").unwrap(), t);
    }

    #[test]
    fn source_with_a_colon_is_quoted_on_write_and_unquoted_on_read() {
        let text = FULL.replace(
            "tags: [world-index, cut-12]",
            "tags: [world-index, cut-12]\nsource: \"mail:<42@example.org>\"",
        );
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.source.as_deref(), Some("mail:<42@example.org>"));
        let out = serialize_task(&t);
        // written after tags, before spec, quoted because of the colon
        assert!(
            out.contains(
                "tags: [world-index, cut-12]\nsource: \"mail:<42@example.org>\"\nspec: docs/specs/"
            ),
            "{out}"
        );
        assert_eq!(parse_task(&out, "x").unwrap(), t);
    }

    #[test]
    fn source_is_omitted_when_absent() {
        let t = parse_task(MINIMAL, "x").unwrap();
        assert_eq!(t.source, None);
        assert!(!serialize_task(&t).contains("source:"));
    }

    #[test]
    fn rejects_empty_or_multiline_source() {
        let err = parse_task(&MINIMAL.replace("tags: []", "tags: []\nsource: \"\""), "x")
            .unwrap_err();
        assert!(err.to_string().contains("source must not be empty"), "{err}");

        let mut t = parse_task(MINIMAL, "x").unwrap();
        t.source = Some("a\nb".into());
        let err = validate_task(&t).unwrap_err();
        assert!(err.to_string().contains("source must be a single line"), "{err}");
        t.source = Some(String::new());
        let err = validate_task(&t).unwrap_err();
        assert!(err.to_string().contains("source must not be empty"), "{err}");
    }
```

- [ ] **Step 2: Run them to verify they fail to compile**

Run: `cargo test --bin tasks source_`
Expected: compile error, `no field 'source' on type Task`.

- [ ] **Step 3: Add the field to the model**

In `src/model.rs`, inside `pub struct Task`, after `pub tags: Vec<String>,`:

```rust
    /// Where the task came from: an opaque, single-line reference such as a URL or a
    /// message id. Stored and returned, never interpreted or resolved. See
    /// docs/specs/2026-09-06-task-source-design.md.
    pub source: Option<String>,
```

- [ ] **Step 4: Parse, validate, and write it**

In `src/format.rs`:

`KEYS` becomes 16 entries, with `"source"` after `"tags"`:

```rust
const KEYS: [&str; 16] = [
    "id", "title", "status", "priority", "size", "parallel", "owner", "created", "updated",
    "depends", "parent", "tags", "source", "spec", "plan", "step",
];
```

In the `Task { ... }` literal inside `parse_task`, after `tags: list("tags")?,`:

```rust
        source: scalar("source")?,
```

In `validate_task`, after the `for tag in &t.tags` loop:

```rust
    if let Some(source) = &t.source {
        validate_line("source", source)?;
    }
```

In `serialize_task`, right after `pairs.push((String::from("tags"), Value::List(t.tags.clone())));` and before the `spec` push:

```rust
    if let Some(v) = &t.source {
        pairs.push(("source".into(), s(v)));
    }
```

`s` is the existing `Value::Scalar` closure; `render_scalar` quotes the value when it contains a reserved character, which is what the colon test asserts.

- [ ] **Step 5: Fix every `Task` literal**

Run: `cargo build --all-targets 2>&1 | grep -n 'missing field'`

Add `source: None,` after the `tags:` line in each literal the compiler names. There are five besides `parse_task`: `blank` in `src/commands/add.rs`, and the test helpers `feedback_task` in `src/similarity.rs`, `task` in `src/hierarchy.rs`, `t` in `src/query.rs`, `sample` in `src/repo.rs`. Rerun the build until it is clean.

- [ ] **Step 6: Run the unit tests**

Run: `cargo test --bin tasks source_ && cargo test --bin tasks format::`
Expected: the four new tests pass; every existing `format::tests` test still passes. The `FULL` fixture is unchanged, so `full_roundtrip` is unaffected.

- [ ] **Step 7: Run the gate and commit**

Run: `just check && just test`
Expected: clean. `tasks check` passes because no task file in `tasks/` carries `source` yet.

```bash
tasks done tasks-6ddc18 "source on Task, parsed/validated/written after tags"
git add src/model.rs src/format.rs src/commands/add.rs src/similarity.rs src/hierarchy.rs src/query.rs src/repo.rs tasks/
git commit -m "feat(model): optional source field, parsed and written after tags"
```

---

### Task 2: `--source` on add and edit, `--no-source` on edit

**Files:**
- Modify: `src/cli.rs:31-58` (`FieldArgs`), `:61-80` (`EditArgs`)
- Modify: `src/commands/mod.rs:20` (import), `:276-278` (`apply_fields`, after the `parent` block)
- Modify: `src/commands/edit.rs:32-47` (`has_flags`), `:60-65` (clearing flags)
- Test: `tests/cli.rs` (new test after `edit_tags_append_and_remove_instead_of_replacing`)

**Interfaces:**
- Consumes: `Task.source` from Task 1.
- Produces: `FieldArgs.source: Option<String>`, `EditArgs.no_source: bool`.

- [ ] **Step 1: Write the failing end-to-end test**

Append to `tests/cli.rs`:

```rust
#[test]
fn source_is_set_by_add_replaced_and_cleared_by_edit() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let id = id_of(env.json(
        &dir,
        &["add", "From mail", "--source", "mail:<42@example.org>"],
    ));
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        "mail:<42@example.org>"
    );
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("tags: []\nsource: \"mail:<42@example.org>\"\n"),
        "{raw}"
    );
    let pretty = env.pretty(&dir, &["show", &id]);
    assert!(pretty.contains("source: \"mail:<42@example.org>\""), "{pretty}");

    env.json(
        &dir,
        &["edit", &id, "--source", "https://example.org/issues/7"],
    );
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        "https://example.org/issues/7"
    );

    env.json(&dir, &["edit", &id, "--no-source"]);
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        serde_json::Value::Null
    );
    assert!(!env.read(&dir, &format!("tasks/{id}.md")).contains("source:"));

    // absent is null, never a missing key
    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    let shown = env.json(&dir, &["show", &plain]);
    assert!(shown["task"].get("source").is_some(), "{shown}");
    assert_eq!(shown["task"]["source"], serde_json::Value::Null);

    // empty and multi-line are validation errors on both write paths
    assert_eq!(env.fail(&dir, &["add", "Bad", "--source", ""]), "validation");
    assert_eq!(env.fail(&dir, &["add", "Bad", "--source", "a\nb"]), "validation");
    assert_eq!(
        env.fail(&dir, &["edit", &plain, "--source", ""]),
        "validation"
    );
    // clap rejects the conflicting pair before any command runs
    env.cmd(&dir)
        .args(["edit", &plain, "--source", "x", "--no-source"])
        .assert()
        .code(2);

    // a repository with sourced tasks passes check
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]), "{check}");

    // the flag completes; nothing in complete.rs mentions it
    let flags = env.complete(&dir, "bash", 3, &["tasks", "add", "T", "--sou"]);
    assert_eq!(flags, ["--source"]);
    let flags = env.complete(&dir, "bash", 3, &["tasks", "edit", &plain, "--no-s"]);
    assert_eq!(flags, ["--no-source"]);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test cli source_is_set`
Expected: FAIL. `add` exits with a clap error, `unexpected argument '--source'`, so the first `env.json` panics.

- [ ] **Step 3: Add the flags**

In `src/cli.rs`, inside `FieldArgs` after the `parent` field:

```rust
    /// Where the task came from: an opaque, single-line reference such as a URL or a
    /// message id. Never interpreted. On `edit` this replaces; see `--no-source`.
    #[arg(long)]
    pub source: Option<String>,
```

Inside `EditArgs`, after `no_parallel`:

```rust
    /// Clear the source.
    #[arg(long, conflicts_with = "source")]
    pub no_source: bool,
```

- [ ] **Step 4: Apply the flags**

In `src/commands/mod.rs`, extend the import on line 20:

```rust
use crate::format::{validate_body, validate_line, validate_note_text, validate_task};
```

In `apply_fields`, after the `if let Some(parent) = &fields.parent { ... }` block and before the `spec` block:

```rust
    if let Some(source) = &fields.source {
        validate_line("source", source)?;
        task.source = Some(source.clone());
    }
```

In `src/commands/edit.rs`, add two lines to `has_flags` after `|| args.no_parallel`:

```rust
        || fields.source.is_some()
        || args.no_source
```

and after the `if args.no_parallel { task.parallel = false; }` block:

```rust
    if args.no_source {
        task.source = None;
    }
```

`apply_fields` runs after this, so `--no-source` alone clears and `--source` alone replaces; clap forbids both together.

- [ ] **Step 5: Run the test**

Run: `cargo test --test cli source_is_set`
Expected: PASS. Bash completion emits bare values (zsh is the shell that appends `:description`), and `--sou` / `--no-s` are prefixes only these two flags match on `add` / `edit`, so the two completion assertions hold with no change to `src/complete.rs`.

- [ ] **Step 6: Run the gate and commit**

Run: `just check && just test && cargo install --path .`
Expected: clean; the installed `tasks` now accepts `--source`.

```bash
tasks done tasks-8480d6 "--source on add/edit, --no-source on edit, end-to-end test"
git add src/cli.rs src/commands/mod.rs src/commands/edit.rs tests/cli.rs tasks/
git commit -m "feat(cli): --source on add and edit, --no-source on edit"
```

---

### Task 3: `source` in summary rows

**Files:**
- Modify: `src/output.rs:105-121` (`TaskSummary`), `:157-175` (`TaskSummary::of`)
- Test: `tests/cli.rs` (extend the Task 2 test)

**Interfaces:**
- Consumes: `Task.source`.
- Produces: `TaskSummary.source: Option<String>`, so `list`, `ready`, `prime`, and `tree` rows carry it.

- [ ] **Step 1: Write the failing assertions**

Add to the end of `source_is_set_by_add_replaced_and_cleared_by_edit` in `tests/cli.rs`:

```rust
    // summary rows carry it too, so `list` can answer "what came from this reference"
    let sourced = id_of(env.json(
        &dir,
        &["add", "Sourced", "--source", "note:abc"],
    ));
    let rows = env.json(&dir, &["list"]);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == sourced)
        .unwrap();
    assert_eq!(row["source"], "note:abc");
    let plain_row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == plain)
        .unwrap();
    assert_eq!(plain_row["source"], serde_json::Value::Null);
    let ready = env.json(&dir, &["ready"]);
    assert!(
        ready.as_array().unwrap().iter().all(|r| r.get("source").is_some()),
        "{ready}"
    );
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test cli source_is_set`
Expected: FAIL at `assert_eq!(row["source"], "note:abc")` with `Null`.

- [ ] **Step 3: Add the summary field**

In `src/output.rs`, inside `pub struct TaskSummary` after `pub tags: Vec<String>,`:

```rust
    pub source: Option<String>,
```

In `TaskSummary::of`, after `tags: task.tags.clone(),`:

```rust
            source: task.source.clone(),
```

- [ ] **Step 4: Run the test**

Run: `cargo test --test cli source_is_set`
Expected: PASS. Pretty tables are built from named columns in `table`, so no column appears and no pretty snapshot changes.

- [ ] **Step 5: Run the gate and commit**

Run: `just check && just test`

```bash
tasks done tasks-db2d78 "source on TaskSummary for list/ready/prime/tree"
git add src/output.rs tests/cli.rs tasks/
git commit -m "feat(output): source on summary rows"
```

---

### Task 4: Documentation and close-out

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md:3-6` (status), `:108-125` (§3.1 field table), `:217-219` (add usage), `:260-261` (edit usage)
- Modify: `docs/specs/2026-09-06-task-source-design.md:3` (status)
- Modify: `docs/plans/2026-09-06-task-source.md:3` (status)
- Modify: `skills/tasks/SKILL.md:39`
- Modify: `README.md:74` (usage examples; insert after the `--parent` line)

- [ ] **Step 1: Field table and usage in the main design spec**

In `docs/specs/2026-08-29-tasks-design.md` §3.1, insert after the `tags` row:

```markdown
| `source`   | string              | no       | Opaque origin reference (a URL, a message id); single line; never interpreted. Written after `tags`. See `2026-09-06-task-source-design.md`. |
```

In the `tasks add` usage block (line 217-219), add `[--source REF]` after `[--step TEXT]`:

```
tasks add <title> [-b|--body TEXT] [--status idea|todo] [-p N] [--size S] [--parallel]
          [--tag T]... [--depends ID]... [--spec NAME] [--plan NAME] [--step TEXT]
          [--source REF] [--parent ID] [--project PREFIX]
```

In the `tasks edit` usage block (line 260-261), add `[--source REF | --no-source]`:

```
tasks edit <id> [same field flags as add] [--status S] [--body -] [--force]
           [--parent ID | --no-parent] [--parallel|--no-parallel] [--rm-tag T]... [--no-tags]
           [--source REF | --no-source]
```

Extend the status header on line 3-6 with `source 2026-09-06` inside the parenthetical list, after `color 2026-09-03`.

- [ ] **Step 2: Skill and README**

In `skills/tasks/SKILL.md` line 39, extend the flag list:

```
Never edit `tasks/*.md` directly. `tasks edit <id> --title/--body/-p/--size/--tag/--depends/--spec/--plan/--step/--parent/--no-parent/--source/--no-source`
```

and add one sentence after the `--rm-tag` sentence on line 42:

```
`--source <ref>` records where a task came from (a URL, a message id, a note); tasks never interprets it.
```

In `README.md`, after the `tasks add "Emit rows" --parent sci-4f2a9c` line:

```
    tasks add "Reply to Dana" --source "mail:<42@example.org>"  # where it came from; never interpreted
```

- [ ] **Step 3: Status lines**

`docs/specs/2026-09-06-task-source-design.md` line 3: `Status: implemented (2026-09-06)` with the actual date. `docs/plans/2026-09-06-task-source.md` line 3: `**Status:** implemented (<date>)`.

- [ ] **Step 4: Close the task and commit**

```bash
tasks done tasks-e3f36d "field table, usage blocks, skill, README, status lines"
tasks done tasks-13a0b6 "source field on the record, --source/--no-source, JSON on Task and TaskSummary"
just check
git add docs/specs/2026-08-29-tasks-design.md docs/specs/2026-09-06-task-source-design.md docs/plans/2026-09-06-task-source.md skills/tasks/SKILL.md README.md tasks/
git commit -m "docs: document the task source field; close tasks-13a0b6"
```

`tasks check` in `just check` verifies the plan headings still match the step children filed against this plan.
