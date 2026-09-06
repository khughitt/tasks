# Parallel Candidates Implementation Plan

**Status:** implemented (2026-09-06)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a task be marked as safe to run beside other marked tasks, so a session dispatching several agents can ask `ready --parallel` for the set it can hand out at once.

**Architecture:** One boolean on `Task`, written to frontmatter as `parallel: true` and omitted when false. It reaches the CLI as `--parallel` / `--no-parallel`, reaches JSON through `Task` (for `show`/`next`) and a new `TaskSummary` field (for `list`/`ready`/`prime`/`tree`), and reaches pretty output as a `||` column whose visibility is decided **once per command output** by the caller and passed into `table`. Readiness and ordering are untouched: parallelism filters, it never sorts.

**Tech Stack:** Rust 2024, clap 4.6, serde. No new dependencies.

**Spec:** `docs/specs/2026-09-06-parallel-candidates-design.md`

**Task:** `tasks-cf1bda`

## Global Constraints

- **JSON output is the contract.** Every change here is additive — one new key on `Task` and one on `TaskSummary`. Never change an existing shape. (`AGENTS.md`)
- **This crate has no library target.** Unit tests run as `cargo test --bin tasks <filter>`; `cargo test --lib` fails with "no library targets found".
- **Fail early with a typed error; no silent fallbacks.** A frontmatter `parallel` value that is not `true` or `false` is a parse error, never a falsy default.
- **Composition > inheritance. Explicit > defensive.**
- **`frontmatter::parse` discards quoting.** `parse_scalar` takes the quoted branch for `"true"` and returns `Value::Scalar("true")`, byte-identical to what the bare word yields. Both spellings therefore read alike with no parser change. Emission still needs `Value::Raw`, because `needs_quotes` quotes the literal `true` (`src/frontmatter.rs:131`).
- **Pretty chrome is ASCII-only.** The marker is `||`, not `∥`.
- Conventional commits. **No AI-attribution trailers or footers.**
- `just check` before every commit (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`). `just gate` adds `cargo test`.
- Rebuild the tracker after CLI changes: `cargo install --path .`
- Every `Task` struct literal must gain the new field or the build breaks. There are five: `src/commands/add.rs:29` (production), and test fixtures at `src/similarity.rs:107`, `src/hierarchy.rs:192`, `src/query.rs:224`, `src/repo.rs:488`.

---

### Task 1: The `parallel` field in the model and on disk

**Files:**
- Modify: `src/model.rs:150-168` (the `Task` struct)
- Modify: `src/format.rs:5-9` (`KEYS`), `:83-107` (the `Task` literal in `parse_task`), `:264-270` (`serialize_task`)
- Modify: `src/commands/add.rs:29`, `src/similarity.rs:107`, `src/hierarchy.rs:192`, `src/query.rs:224`, `src/repo.rs:488`
- Test: `src/format.rs` (the existing `mod tests` at the end of the file)

**Interfaces:**
- Consumes: nothing.
- Produces: `Task.parallel: bool`. Frontmatter key `parallel`, emitted only when true, positioned between `size` and `owner`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/format.rs`:

```rust
#[test]
fn parallel_round_trips_and_is_omitted_when_false() {
    let mut t = parse_task(MINIMAL, "x").unwrap();
    assert!(!t.parallel, "absent key reads as false");
    assert!(
        !serialize_task(&t).contains("parallel"),
        "false is never written"
    );

    t.parallel = true;
    let text = serialize_task(&t);
    assert!(
        text.contains("\nparallel: true\n"),
        "emitted unquoted: {text}"
    );
    assert!(parse_task(&text, "x").unwrap().parallel);
}

#[test]
fn parallel_accepts_both_spellings_and_rejects_anything_else() {
    // frontmatter::parse discards quoting, so the quoted forms are indistinguishable
    // from the bare words by the time parse_task sees them.
    for (value, expected) in [
        ("true", true),
        ("\"true\"", true),
        ("false", false),
        ("\"false\"", false),
    ] {
        let text = MINIMAL.replace("depends: []", &format!("parallel: {value}\ndepends: []"));
        assert_eq!(
            parse_task(&text, "x").unwrap().parallel,
            expected,
            "parallel: {value}"
        );
    }
    for bad in ["maybe", "1", "True", "yes"] {
        let text = MINIMAL.replace("depends: []", &format!("parallel: {bad}\ndepends: []"));
        let err = parse_task(&text, "x").unwrap_err().to_string();
        assert!(
            err.contains("parallel must be true or false"),
            "parallel: {bad} gave {err}"
        );
    }
}

#[test]
fn parallel_false_in_a_file_is_dropped_on_the_next_write() {
    let text = MINIMAL.replace("depends: []", "parallel: false\ndepends: []");
    let t = parse_task(&text, "x").unwrap();
    assert!(!t.parallel);
    assert!(!serialize_task(&t).contains("parallel"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks format::tests::parallel`
Expected: FAIL to compile — `no field 'parallel' on type 'Task'`.

- [ ] **Step 3: Add the field to the model**

In `src/model.rs`, in `pub struct Task`, immediately after `pub size: Option<Size>,`:

```rust
    /// Marked safe to run beside any other task marked parallel. Hand-set; nothing
    /// infers or validates it. See docs/specs/2026-09-06-parallel-candidates-design.md.
    pub parallel: bool,
```

- [ ] **Step 4: Add `parallel: false` to every Task literal**

One line in each of these five, next to the existing `size:` field so the literal keeps declaration order:

- `src/commands/add.rs:29` — in `blank()`, after `size: None,`
- `src/similarity.rs:107`, `src/hierarchy.rs:192`, `src/query.rs:224`, `src/repo.rs:488` — the test fixtures

```rust
        parallel: false,
```

- [ ] **Step 5: Parse the key**

In `src/format.rs`, extend `KEYS` to 15 entries, inserting `"parallel"` after `"size"`:

```rust
const KEYS: [&str; 15] = [
    "id", "title", "status", "priority", "size", "parallel", "owner", "created", "updated",
    "depends", "parent", "tags", "spec", "plan", "step",
];
```

In `parse_task`, after the `list` closure and before the `priority` binding, add a closure beside the existing `scalar` / `required` / `list` ones:

```rust
    // Absent is false; a value that is neither boolean is an error, never a falsy default.
    let boolean = |k: &str| -> Result<bool> {
        match scalar(k)? {
            None => Ok(false),
            Some(v) if v == "true" => Ok(true),
            Some(v) if v == "false" => Ok(false),
            Some(v) => Err(perr(file, format!("{k} must be true or false, not {v:?}"))),
        }
    };
```

Then in the `Task { .. }` literal, after `size: ...,`:

```rust
        parallel: boolean("parallel")?,
```

- [ ] **Step 6: Serialize the key**

In `serialize_task`, immediately after the `if let Some(z) = t.size { .. }` block and before the `owner` block:

```rust
    // Raw, not Scalar: needs_quotes quotes the literal `true`, which would write
    // `parallel: "true"` — readable back, but out of step with every other scalar.
    if t.parallel {
        pairs.push(("parallel".into(), Value::Raw("true".into())));
    }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test --bin tasks format::tests::parallel`
Expected: PASS (3 tests).

- [ ] **Step 8: Run the full gate**

Run: `just gate`
Expected: PASS. Existing tests are unaffected — no file on disk carries the key yet.

- [ ] **Step 9: Commit**

```bash
git add src/model.rs src/format.rs src/commands/add.rs src/similarity.rs src/hierarchy.rs src/query.rs src/repo.rs
git commit -m "feat(model): add the parallel field to the task record"
```

---

### Task 2: `--parallel` and `--no-parallel` on add and edit

**Files:**
- Modify: `src/cli.rs:32-52` (`FieldArgs`), `:54-75` (`EditArgs`)
- Modify: `src/commands/mod.rs:233-280` (`apply_fields`)
- Modify: `src/commands/edit.rs:31-46` (the `has_flags` test), `:58-64` (the clearing block)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Task.parallel` from Task 1.
- Produces: `FieldArgs.parallel: bool`, `EditArgs.no_parallel: bool`. `add --parallel` sets the field; `edit --parallel` sets it; `edit --no-parallel` clears it; absent on `edit` leaves it alone.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`:

```rust
#[test]
fn parallel_is_set_by_flag_and_cleared_by_no_parallel() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    assert_eq!(env.json(&dir, &["show", &plain])["task"]["parallel"], false);

    let marked = id_of(env.json(&dir, &["add", "Marked", "--parallel"]));
    assert_eq!(env.json(&dir, &["show", &marked])["task"]["parallel"], true);
    assert!(
        env.read(&dir, &format!("tasks/{marked}.md"))
            .contains("\nparallel: true\n"),
        "the key is written unquoted"
    );

    // An unrelated edit must not disturb the flag.
    env.json(&dir, &["edit", &marked, "-p", "1"]);
    assert_eq!(env.json(&dir, &["show", &marked])["task"]["parallel"], true);

    env.json(&dir, &["edit", &marked, "--no-parallel"]);
    assert_eq!(env.json(&dir, &["show", &marked])["task"]["parallel"], false);
    assert!(
        !env.read(&dir, &format!("tasks/{marked}.md")).contains("parallel"),
        "the key is dropped, not written false"
    );

    env.json(&dir, &["edit", &plain, "--parallel"]);
    assert_eq!(env.json(&dir, &["show", &plain])["task"]["parallel"], true);
}

#[test]
fn parallel_and_no_parallel_conflict() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A"]));
    let out = env
        .cmd(&dir)
        .args(["edit", &id, "--parallel", "--no-parallel"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "clap must reject the pair");
}
```

**The path is `v["task"]["parallel"]`.** `ShowOut.fields` is `#[serde(flatten)]` (`src/output.rs:78-82`), which lifts `ShowFields`' own members to the top level — so `claim` sits at `v["claim"]`. But `ShowFields.task` (`src/output.rs:67`) is an ordinary nested field, so everything from the task record stays one level down. `tests/cli.rs:328` reads `shown["task"]["parent"]` for the same reason. A wrong path here compares `null` against `false` and passes whether or not the feature works.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli parallel_is_set_by_flag`
Expected: FAIL — `unexpected argument '--parallel'`.

- [ ] **Step 3: Add the flags**

In `src/cli.rs`, in `FieldArgs`, after the `size` field:

```rust
    /// Mark as safe to run beside other tasks marked parallel. On `edit` this sets the
    /// flag; see `--no-parallel` to clear it.
    #[arg(long)]
    pub parallel: bool,
```

In `EditArgs`, next to `no_parent` and `no_tags`:

```rust
    /// Clear the parallel marker.
    #[arg(long, conflicts_with = "parallel")]
    pub no_parallel: bool,
```

- [ ] **Step 4: Apply the flag**

In `src/commands/mod.rs`, in `apply_fields`, after the `if let Some(size) = &fields.size { .. }` block:

```rust
    // Setting only. `edit --no-parallel` clears it before this runs, mirroring --no-tags.
    if fields.parallel {
        task.parallel = true;
    }
```

In `src/commands/edit.rs`, beside the existing `if args.no_parent { .. }` block, add:

```rust
    if args.no_parallel {
        task.parallel = false;
    }
```

Both flags must join the `has_flags` chain, or `edit <id> --parallel` with no other flag falls through to the `$EDITOR` branch instead of setting the field:

```rust
        || fields.parallel
        || args.no_parallel
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test cli parallel`
Expected: PASS (2 tests).

- [ ] **Step 6: Run the full gate**

Run: `just gate`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/commands/mod.rs src/commands/edit.rs tests/cli.rs
git commit -m "feat(cli): set and clear the parallel marker on add and edit"
```

---

### Task 3: `ready --parallel`

**Files:**
- Modify: `src/cli.rs:141-150` (`Command::Ready`)
- Modify: `src/commands/mod.rs:528-532` (the `Command::Ready` dispatch arm)
- Modify: `src/commands/list.rs:134-154` (`ready`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Task.parallel` from Task 1, `--parallel` from Task 2.
- Produces: `list::ready(ctx, size, limit, parallel: bool)`. The filter runs after the readiness test and after the `--size` filter, before `--limit` truncates.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`:

```rust
#[test]
fn ready_parallel_filters_and_still_honours_limit() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // C outranks both marked tasks, so it heads the unfiltered ready list. That is what
    // makes the -n 1 assertion below able to catch a truncate-before-filter regression:
    // with the filter in the wrong place, `--parallel -n 1` truncates to [C] and then
    // filters to nothing. Give them distinct priorities — equal priority and size would
    // fall through to `created`, and whenever a marked task happened to sort first the
    // broken order would still pass.
    env.json(&dir, &["add", "C", "-p", "0"]);
    let a = id_of(env.json(&dir, &["add", "A", "-p", "1", "--parallel"]));
    let b = id_of(env.json(&dir, &["add", "B", "-p", "2", "--parallel"]));

    let ids = |v: serde_json::Value| -> Vec<String> {
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };

    assert_eq!(ids(env.json(&dir, &["ready"])).len(), 3);
    assert_eq!(
        ids(env.json(&dir, &["ready", "--parallel"])),
        [a.clone(), b.clone()],
        "marked only, in the usual ready order"
    );
    assert_eq!(
        ids(env.json(&dir, &["ready", "--parallel", "-n", "1"])),
        [a],
        "the limit applies after the filter"
    );

    // A doing task is not ready, so it never joins the marked set.
    env.json(&dir, &["start", &b]);
    assert_eq!(ids(env.json(&dir, &["ready", "--parallel"])).len(), 1);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli ready_parallel_filters`
Expected: FAIL — `unexpected argument '--parallel'`.

- [ ] **Step 3: Add the flag**

In `src/cli.rs`, in `Command::Ready`, after the `size` field:

```rust
        /// Only tasks marked safe to run beside each other.
        #[arg(long)]
        parallel: bool,
```

- [ ] **Step 4: Thread it through**

In `src/commands/mod.rs`, the dispatch arm becomes:

```rust
        Command::Ready {
            size,
            parallel,
            limit,
            all_projects,
        } => list::ready(open_read_ctx(dir, all_projects)?, size, parallel, limit),
```

In `src/commands/list.rs`, change the signature and add the filter between the size filter and the truncation:

```rust
pub fn ready(
    mut ctx: ReadCtx,
    size: Option<String>,
    parallel: bool,
    limit: Option<usize>,
) -> Result<Output> {
```

```rust
    if parallel {
        tasks.retain(|task| task.parallel);
    }
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
```

`sort_ready` has already run inside `ready_tasks`, and `retain` preserves order, so the marked set comes back in the usual ready order. Do not touch `is_ready` or `ready_order`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --test cli ready_parallel_filters`
Expected: PASS.

- [ ] **Step 6: Run the full gate**

Run: `just gate`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/commands/mod.rs src/commands/list.rs tests/cli.rs
git commit -m "feat(ready): filter to parallel candidates with --parallel"
```

---

### Task 4: `parallel` in summaries and the conditional pretty column

**Files:**
- Modify: `src/output.rs:91-106` (`TaskSummary`), `:135-160` (`TaskSummary::of`), `:332` `:347` `:361` `:372` `:374` `:394` (the `table` / `tree_text` call sites), `:494-507` (`tree_text`), `:511-551` (`table`)
- Test: `src/output.rs` (a new `mod tests` at the end of the file), `tests/cli.rs`

**Interfaces:**
- Consumes: `Task.parallel` from Task 1, and **`add --parallel` from Task 2** — the end-to-end test in Step 8 has no other way to mark a task, so this task cannot pass its gate until Task 2 has landed.
- Produces: `TaskSummary.parallel: bool`; `table(rows, date, painter, parallel_column: bool)`; `tree_text(nodes, depth, painter, parallel_column: bool)`; `fn any_parallel(&[TaskSummary]) -> bool` and `fn any_parallel_tree(&[TreeNode]) -> bool`.

- [ ] **Step 1: Write the failing unit test**

`src/output.rs` has no test module. Add one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorMode;

    fn row(id: &str, parallel: bool) -> TaskSummary {
        TaskSummary {
            id: id.into(),
            title: format!("title {id}"),
            status: Status::Todo,
            priority: 2,
            size: None,
            owner: None,
            created: "2026-09-06T00:00:00Z".into(),
            updated: "2026-09-06T00:00:00Z".into(),
            tags: vec![],
            depends: vec![],
            parent: None,
            child_count: 0,
            open_descendant_count: 0,
            claim: None,
            parallel,
        }
    }

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, Format::Pretty, false)
    }

    #[test]
    fn the_marker_column_is_absent_when_nothing_is_marked() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_parallel(&rows));
        let text = table(&rows, DateColumn::Updated, &plain(), false);
        assert!(!text.contains("||"), "{text}");
        assert!(text.contains("todo    2026-09-06"), "{text}");
    }

    #[test]
    fn mixed_siblings_share_one_column_layout() {
        // The case a per-call decision inside `table` gets wrong: an unmarked sibling
        // must reserve the same width as its marked neighbour, or the date and title
        // shift between adjacent lines.
        let nodes = vec![
            TreeNode {
                summary: row("xx-000001", true),
                children: vec![],
            },
            TreeNode {
                summary: row("xx-000002", false),
                children: vec![],
            },
        ];
        assert!(any_parallel_tree(&nodes));
        let text = tree_text(&nodes, 0, &plain(), true);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo    || 2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo       2026-09-06"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn any_parallel_tree_finds_a_marked_descendant() {
        let nodes = vec![TreeNode {
            summary: row("xx-000001", false),
            children: vec![TreeNode {
                summary: row("xx-000002", true),
                children: vec![],
            }],
        }];
        assert!(any_parallel_tree(&nodes));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --bin tasks output::tests`
Expected: FAIL to compile — `TaskSummary` has no field `parallel`, and `any_parallel` is not defined.

- [ ] **Step 3: Add the summary field**

In `src/output.rs`, in `pub struct TaskSummary`, after `pub claim: Option<ClaimInfo>,`:

```rust
    pub parallel: bool,
```

and in `TaskSummary::of`, in the returned literal:

```rust
            parallel: task.parallel,
```

- [ ] **Step 4: Add the visibility helpers**

Immediately above `pub fn table`:

```rust
/// Whether a pretty rendering must reserve the parallel column. Decided once per command
/// output and passed into `table`: `tree_text` and `prime`'s roadmap call `table` one row
/// at a time, so a per-call decision would shift dates between adjacent siblings.
pub fn any_parallel(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| row.parallel)
}

pub fn any_parallel_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.parallel || any_parallel_tree(&node.children))
}
```

- [ ] **Step 5: Render the column**

Change the signature to `pub fn table(rows: &[TaskSummary], date: DateColumn, painter: &Painter, parallel_column: bool) -> String`, and inside the loop, before the final `push_str`:

```rust
        // Unpainted: it is already distinct, and painting the blank spacer would wrap
        // whitespace in ANSI for no gain.
        let mark = match (parallel_column, row.parallel) {
            (false, _) => "",
            (true, true) => "|| ",
            (true, false) => "   ",
        };
```

and make the format string:

```rust
        rendered.push_str(&format!(
            "{id}  {priority} {size:<2} {status} {mark}{date}  {}{tags}{owner}\n",
            row.title
        ));
```

Change `tree_text` to take and forward the flag:

```rust
fn tree_text(
    nodes: &[TreeNode],
    depth: usize,
    painter: &Painter,
    parallel_column: bool,
) -> String {
    let mut rendered = String::new();
    for node in nodes {
        let row = table(
            std::slice::from_ref(&node.summary),
            DateColumn::Updated,
            painter,
            parallel_column,
        );
        rendered.push_str(&"  ".repeat(depth));
        rendered.push_str(&row);
        rendered.push_str(&tree_text(&node.children, depth + 1, painter, parallel_column));
    }
    rendered
}
```

- [ ] **Step 6: Decide visibility at each call site**

`Output::List` (`src/output.rs:332`):

```rust
        Output::List(o) => table(&o.tasks, o.date, painter, any_parallel(&o.tasks)),
```

`Output::Tree` (`:394`):

```rust
        Output::Tree(o) => tree_text(&o.nodes, 0, painter, any_parallel_tree(&o.nodes)),
```

`Output::Prime` (`:333-375`): compute once at the top of the arm, before the header is built, and pass it to all four `table`/`tree_text` calls in that arm (closeout, the roadmap's `tree_text`, the roadmap's childless-root `table`, `ready`, and `doing`):

```rust
            // One decision for the whole output: prime's blocks align today only because
            // every width is fixed, and a per-section decision would break that.
            let parallel_column = any_parallel(&o.closeout)
                || any_parallel_tree(&o.roadmap)
                || any_parallel(&o.ready)
                || any_parallel(&o.doing);
```

- [ ] **Step 7: Run the unit tests to verify they pass**

Run: `cargo test --bin tasks output::tests`
Expected: PASS (3 tests).

- [ ] **Step 8: Write the end-to-end test**

Add to `tests/cli.rs`:

```rust
#[test]
fn pretty_rows_show_the_parallel_marker_only_when_something_is_marked() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Plain", "-p", "1"]);

    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("||"), "no column when nothing is marked:\n{text}");

    env.json(&dir, &["add", "Marked", "-p", "0", "--parallel"]);
    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].contains("|| "), "{}", lines[0]);
    assert!(!lines[1].contains("||"), "{}", lines[1]);
    assert_eq!(
        lines[0].find("Marked"),
        lines[1].find("Plain"),
        "titles must start in the same column:\n{text}"
    );

    // The JSON key rides on every summary.
    let v = env.json(&dir, &["list"]);
    let flags: Vec<bool> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["parallel"].as_bool().unwrap())
        .collect();
    assert_eq!(flags, [true, false]);
}
```

- [ ] **Step 9: Run the full gate**

Run: `just gate`
Expected: PASS. If any existing pretty-output test fails, it is asserting on a row that now carries the column — check whether that test's fixture marks anything; if it does not, the column must be absent and the failure is a real regression.

- [ ] **Step 10: Commit**

```bash
git add src/output.rs tests/cli.rs
git commit -m "feat(output): carry parallel on summaries and mark it in pretty rows"
```

---

### Task 5: Documentation and close-out

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md` (frontmatter example ~`:85`, field table ~`:113`, the `add` / `ready` synopses ~`:215` `:251`, the `Task` and `TaskSummary` JSON shapes ~`:357` `:365`)
- Modify: `skills/tasks/SKILL.md` (under `## Recording work`)
- Modify: `docs/specs/2026-09-06-parallel-candidates-design.md` (status header)
- Modify: `docs/plans/2026-09-06-parallel-candidates.md` (status header)
- Modify: `README.md` (only if the dispatch flow reads as worth showing under `## Use`)

**Interfaces:**
- Consumes: everything above.
- Produces: no code.

- [ ] **Step 1: Update the tracker's own design doc**

In `docs/specs/2026-08-29-tasks-design.md`, all four places:

1. The frontmatter example: add `parallel: true` between `size: m` and the next key.
2. The field table, as a new row after `size`:

```markdown
| `parallel` | bool                | no       | Safe to run beside other tasks marked `parallel`. Omitted when false. |
```

3. The synopses:

```
tasks add <title> [-b|--body TEXT] [--status idea|todo] [-p N] [--size S] [--parallel]
tasks ready [--size S] [--parallel] [-n N] [--all-projects]
```

and add `[--parallel|--no-parallel]` to the `edit` synopsis.

4. The JSON shapes: add `parallel` to the `Task` field list and to `TaskSummary`.

- [ ] **Step 2: Update the shipped skill**

In `skills/tasks/SKILL.md`, under `## Recording work`, after the decomposition bullet:

```markdown
- Dispatching several agents at once: mark each self-contained task with
  `tasks edit <id> --parallel`, then `tasks ready --parallel -n <N>` for the set to hand
  out. The marker asserts only that marked tasks do not collide with *each other* — it
  says nothing about unmarked tasks or about work already in flight, so read `prime`'s
  `doing` list before dispatching. Re-examine a task's marker whenever its scope changes;
  a stale marker is a wrong assertion. `--no-parallel` clears it.
```

Verify the wording against `docs/specs/2026-09-06-parallel-candidates-design.md` §"What the promise covers" — the skill is the only place most agents will read this, so the two rules (check `doing`, re-examine on scope change) must both survive.

- [ ] **Step 3: Correct both status headers**

`docs/specs/2026-09-06-parallel-candidates-design.md`:

```markdown
Status: implemented (2026-09-06)
```

`docs/plans/2026-09-06-parallel-candidates.md`:

```markdown
**Status:** implemented (2026-09-06)
```

Per the project's rule: when work lands, its design doc's status is corrected in the same change.

- [ ] **Step 4: Run the full gate**

Run: `just gate`
Expected: PASS, including `tasks check` — which warns about any `### Task N:` heading in this plan with no matching task, so every child task must be `done` by now.

- [ ] **Step 5: Reinstall and smoke-test the real tracker**

```bash
cargo install --path .
tasks ready --parallel --pretty
tasks list --pretty
```

Expected: `ready --parallel` returns nothing (no task in this repo is marked yet) and `list --pretty` shows no `||` column. Mark one task and confirm the column appears, then clear it.

- [ ] **Step 6: Commit and close the task**

```bash
git add docs/ skills/ README.md
tasks done tasks-cf1bda "parallel: bool on the task record; --parallel/--no-parallel on add and edit; ready --parallel; conditional || column decided once per output"
git add tasks/
git commit -m "docs(tasks): document the parallel-candidate marker"
```

`tasks done` refuses while any descendant is open, so every child of `tasks-cf1bda` must be closed first.
