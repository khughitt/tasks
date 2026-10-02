# Additive `edit --depends` implementation plan

**Status:** draft for review. Design task: tasks-e9af16; implementation task: tasks-8efda8
(re-scoped against this plan when it is approved).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `edit --depends` appends to a task's dependencies instead of replacing them, and
`edit --no-depends` clears them (alone) or makes `--depends` a replacement (together).

**Architecture:** One helper, `dep::add_dependencies`, holds the add logic that `dep --on`
already has: canonical-identity dedup, the already-present warning, validation, and the
final cycle check. `dep::run` and `apply_fields` (which serves `add` and `edit`) both call
it. `edit::run` clears the list for `--no-depends` before `apply_fields`, beside the
`--no-tags` clear.

**Tech Stack:** Rust (edition 2024), clap 4; integration tests in `tests/cli.rs` through
`tests/common/mod.rs`.

**Spec:** `docs/specs/2026-10-02-edit-depends-design.md` (approved after review round 1).

## Global Constraints

- Work in `.worktrees/edit-depends` (branch `design/edit-depends`). Run tests only through
  `just test-one …` and `just test-fast`; never `cargo test` directly. `just check` before
  each commit.
- JSON shapes are unchanged; the only new output is a string in `warnings`.
- Stored spellings of untouched dependencies never change. A new edge is stored canonical.
- Any failure leaves the task file byte-identical (`edit` and `dep` save once at the end).
- `--no-depends` alone walks no graph. `--depends` with any value always ends with
  `dep::ensure_acyclic`, even when every id named is already present.
- `add --depends` and the `$EDITOR` path keep their current behaviour.
- Conventional commits, no AI-attribution trailers. `tasks done` for each step task goes
  in the same commit as its code.

## Review Focus

1. `edit --depends X` naming a dependency already stored under a retired spelling: one
   edge, stored spelling kept, the `dep --on` warning (Task 1, step 1, part B).
2. `edit --depends X` with X already present on a task that also stores an unreachable
   dependency: fails `unresolvable_id`, file unchanged (Task 1, step 1, part D).
3. `edit --no-depends` on a task whose only dependency is unreachable: succeeds (Task 2,
   step 1, part A).
4. `edit --no-depends` alone must not open `$EDITOR` (Task 2, step 1, part A: `env.json`
   would hang or fail if it did).
5. `dep --on` keeps its overlap check, removal-first order, and warnings after the
   refactor (Task 1, step 4: the existing `dep` tests).

---

### Task 1: Shared add helper; `edit --depends` appends

**Files:**
- Modify: `src/commands/dep.rs` (new `add_dependencies`; `run` calls it)
- Modify: `src/commands/mod.rs:635-651` (`apply_fields` dependency block)
- Modify: `src/cli.rs:166` (`FieldArgs::depends` help)
- Test: `tests/cli.rs` (new test after
  `dep_on_and_rm_together_swap_in_one_save_or_change_nothing`)

**Interfaces:**
- Produces: `pub fn add_dependencies(ctx: &Ctx, resolver: &Resolver<'_>, task: &mut Task, values: &[String]) -> Result<Vec<String>>`
  in `src/commands/dep.rs`. Appends each value's canonical id unless the task already
  depends on it under any spelling, returns the deduplicated already-present warnings,
  and ends with `ensure_acyclic(ctx, task)` when `values` is not empty. It takes `&Ctx`
  and a caller-owned `Resolver` (not `&mut Ctx`) because `apply_fields` holds a resolver
  borrowing `ctx.project` and `ctx.registry` for its whole body; the caller extends
  `ctx.warnings` with the result.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`, before
`fn stored_retired_references_resolve_detect_cycles_and_keep_their_spelling`:

```rust
#[test]
fn edit_depends_appends_keeps_spellings_and_validates_the_final_graph() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let task = id_of(env.json(&sci, &["add", "Task", "-p", "2"]));
    let deps: Vec<String> = (0..6)
        .map(|n| id_of(env.json(&sci, &["add", &format!("Dep {n}"), "-p", "2"])))
        .collect();
    let task_path = sci.join("tasks").join(format!("{task}.md"));

    // A: five edges, then a sixth through edit: all six remain, the new one last.
    env.json(&sci, &["dep", &task, "--on", &deps[0], &deps[1], &deps[2], &deps[3], &deps[4]]);
    let added = env.json(&sci, &["edit", &task, "--depends", &deps[5]]);
    assert_eq!(added["warnings"], serde_json::json!([]), "{added}");
    assert_eq!(
        env.json(&sci, &["show", &task])["task"]["depends"],
        serde_json::json!(deps)
    );

    // B: an already-present edge named by its retired spelling warns and keeps one edge.
    alias_registry(&env, "old", "sci");
    let retired = deps[0].replacen("sci-", "old-", 1);
    let again = env.json(&sci, &["edit", &task, "--depends", &retired]);
    let warning = again["warnings"][0].as_str().unwrap();
    assert!(
        warning.starts_with(&format!(
            "{task} already depends on {} (given as {retired})",
            deps[0]
        )),
        "{warning}"
    );
    // The reverse: the stored spelling is retired and the live one is supplied.
    std::fs::write(
        &task_path,
        std::fs::read_to_string(&task_path)
            .unwrap()
            .replacen(&deps[0], &retired, 1),
    )
    .unwrap();
    let live = env.json(&sci, &["edit", &task, "--depends", &deps[0]]);
    assert!(
        live["warnings"][0]
            .as_str()
            .unwrap()
            .contains(&format!("(stored as {retired})")),
        "{live}"
    );
    let text = std::fs::read_to_string(&task_path).unwrap();
    assert!(text.contains(&retired), "{text}");
    assert_eq!(
        env.json(&sci, &["show", &task])["task"]["depends"]
            .as_array()
            .unwrap()
            .len(),
        6
    );

    // C: a cycle, or an unresolvable id beside a valid one, changes nothing.
    let upstream = id_of(env.json(&sci, &["add", "Upstream", "-p", "2"]));
    let fresh = id_of(env.json(&sci, &["add", "Fresh", "-p", "2"]));
    env.json(&sci, &["dep", &upstream, "--on", &task]);
    let before = std::fs::read_to_string(&task_path).unwrap();
    assert_eq!(env.fail(&sci, &["edit", &task, "--depends", &upstream]), "cycle");
    assert_eq!(
        env.fail(&sci, &["edit", &task, "--depends", &fresh, "--depends", "sci-000000"]),
        "unresolvable_id"
    );
    assert_eq!(std::fs::read_to_string(&task_path).unwrap(), before);

    // D: a duplicate-only add still checks the final graph, so a stored unreachable
    // dependency fails it and the file stays as it was.
    std::fs::write(
        &task_path,
        before.replacen(&deps[1], "zzz-000001", 1),
    )
    .unwrap();
    let before = std::fs::read_to_string(&task_path).unwrap();
    assert_eq!(
        env.fail(&sci, &["edit", &task, "--depends", &deps[2]]),
        "unresolvable_id"
    );
    assert_eq!(std::fs::read_to_string(&task_path).unwrap(), before);
}
```

Note for part B: `deps[0]` appears once in the file (the `depends:` line), so
`replacen(…, 1)` rewrites only the stored reference. In part C, "Upstream" depends on the
task, so adding it closes a cycle; "Fresh" is resolvable, and `sci-000000` is a
well-formed id no task has.

- [ ] **Step 2: Run test to verify it fails**

Run: `just test-one --test cli edit_depends_appends`
Expected: FAIL in part A: `depends` holds only `deps[5]` (replacement).

- [ ] **Step 3: Write the helper and use it in both callers**

In `src/commands/dep.rs`, add above `already_depends`:

```rust
/// Adds each of `values` to `task.depends` unless the task already depends on it under
/// any spelling, then checks the final graph. Returns a warning for each id the task
/// already held before the call; repeats within one call are deduplicated silently.
/// Shared by `dep --on` and by `--depends` on `add` and `edit`.
pub fn add_dependencies(
    ctx: &Ctx,
    resolver: &Resolver<'_>,
    task: &mut Task,
    values: &[String],
) -> Result<Vec<String>> {
    let existing = task.depends.len();
    let mut warnings = Vec::new();
    for value in values {
        let given = TaskId::parse_input(value)?;
        let dependency = ctx.registry.canonical_id(&given);
        if dependency == task.id {
            return Err(Error::Cycle(format!("{dependency} -> {dependency}")));
        }
        if resolver.resolve_task(&dependency)?.is_none() {
            return Err(Error::UnresolvableId(dependency.to_string()));
        }
        match task
            .depends
            .iter()
            .position(|item| ctx.registry.canonical_id(item) == dependency)
        {
            Some(index) if index < existing => {
                let warning = already_depends(&task.id, &dependency, &task.depends[index], &given);
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            Some(_) => {}
            None => task.depends.push(dependency),
        }
    }
    if !values.is_empty() {
        ensure_acyclic(ctx, task)?;
    }
    Ok(warnings)
}
```

In `dep::run`, replace the whole `if !additions.is_empty() { … }` block with:

```rust
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let warnings = add_dependencies(&ctx, &resolver, &mut task, &on)?;
    ctx.warnings.extend(warnings);
```

`additions` is still needed for the overlap check above it; its `given` half is now
unused, so reduce it to the canonical ids:

```rust
    let additions = on
        .iter()
        .map(|value| super::parse_id(&ctx.registry, value))
        .collect::<Result<Vec<_>>>()?;
    let removals = rm
        .iter()
        .map(|value| super::parse_id(&ctx.registry, value))
        .collect::<Result<Vec<_>>>()?;
    if let Some(both) = additions.iter().find(|dependency| removals.contains(dependency)) {
        return Err(Error::Validation(format!(
            "{both} is named by both --on and --rm"
        )));
    }
```

In `src/commands/mod.rs::apply_fields`, replace the block from
`if !fields.depends.is_empty() {` through its closing `}` (the one ending
`dep::ensure_acyclic(ctx, task)?;`) with:

```rust
    // Additive like `--tag`: an edit that names one dependency must not drop the others.
    // `edit --no-depends` clears the list before this runs; `dep --rm` removes one.
    let warnings = dep::add_dependencies(ctx, &resolver, task, &fields.depends)?;
    ctx.warnings.extend(warnings);
```

In `src/cli.rs`, change the `FieldArgs::depends` doc comment to:

```rust
    /// Depend on another task (repeatable). On `edit` this appends; see `--no-depends`
    /// and `dep --rm`.
```

- [ ] **Step 4: Run the focused and neighbouring tests**

Run: `just test-one --test cli edit_depends_appends`
Expected: PASS.
Run: `just test-one --test cli dep`
Expected: PASS (the existing `dep` tests, including the swap and the already-present
warning).
Run: `just test-one --test cli depends`
Expected: PASS (`add --depends` and the `edit --status done --depends` refusal).

- [ ] **Step 5: Gate and commit**

Run: `cargo fmt && just test-fast && just check`
Expected: all pass.

```bash
tasks done <task-1-id> "edit --depends appends through dep::add_dependencies, shared with dep --on and add"
git add src/cli.rs src/commands/dep.rs src/commands/mod.rs tests/cli.rs tasks/
git commit -m "feat(edit): --depends appends instead of replacing the list"
```

---

### Task 2: `edit --no-depends`; docs

**Files:**
- Modify: `src/cli.rs:248-250` (`EditArgs`, new `no_depends` beside `no_tags`)
- Modify: `src/commands/edit.rs:116` (`has_flags`) and `:177-180` (clear before
  `apply_fields`)
- Modify: `docs/specs/2026-08-29-tasks-design.md:330-345` (`edit` entry) and `:685-690`
  (§6.1 cycle detection)
- Modify: `README.md` (example beside `--tag … --rm-tag` at about line 285)
- Modify: `skills/tasks/SKILL.md:161` (edit flag list)
- Modify: `skills/curate/SKILL.md:73-74` (link repair)
- Test: `tests/cli.rs` (new test after the Task 1 test)

**Interfaces:**
- Consumes: `dep::add_dependencies` from Task 1, through `apply_fields` (no direct call).
- Produces: `EditArgs::no_depends: bool`, flag `--no-depends`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn edit_no_depends_clears_and_with_depends_replaces() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let task = id_of(env.json(&sci, &["add", "Task", "-p", "2"]));
    let a = id_of(env.json(&sci, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&sci, &["add", "B", "-p", "2"]));
    let c = id_of(env.json(&sci, &["add", "C", "-p", "2"]));
    env.json(&sci, &["dep", &task, "--on", &a, &b]);

    // Replace: exactly the ids given, and no warning for an id that was there before.
    let replaced = env.json(&sci, &["edit", &task, "--no-depends", "--depends", &c, "--depends", &a]);
    assert_eq!(replaced["warnings"], serde_json::json!([]), "{replaced}");
    assert_eq!(
        env.json(&sci, &["show", &task])["task"]["depends"],
        serde_json::json!([c, a])
    );

    // A: clear, alone, walks no graph: an unreachable stored dependency goes too.
    let task_path = sci.join("tasks").join(format!("{task}.md"));
    std::fs::write(
        &task_path,
        std::fs::read_to_string(&task_path)
            .unwrap()
            .replacen(&c, "zzz-000001", 1),
    )
    .unwrap();
    // EDITOR=false: if `--no-depends` alone fell through to the editor path, the call
    // would fail here instead of hanging on an inherited editor.
    env.cmd(&sci)
        .env("EDITOR", "false")
        .args(["edit", &task, "--no-depends"])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &task])["task"]
            .get("depends")
            .is_none()
    );
}
```

The `EDITOR=false` call pins Review Focus 4: the editor path would exit non-zero.

- [ ] **Step 2: Run test to verify it fails**

Run: `just test-one --test cli edit_no_depends`
Expected: FAIL: clap rejects `--no-depends` as an unexpected argument.

- [ ] **Step 3: Implement the flag**

In `src/cli.rs`, `EditArgs`, after `no_tags`:

```rust
    /// Clear every dependency; with `--depends`, replaces the list wholesale.
    #[arg(long)]
    pub no_depends: bool,
```

In `src/commands/edit.rs`, add to `has_flags` after `|| args.no_tags`:

```rust
        || args.no_depends
```

and extend the clear before `apply_fields`:

```rust
    // Clear, then remove, then let `apply_fields` append: `--no-tags --tag x` is the
    // wholesale replace `--tag` used to perform by itself, and `--no-depends --depends x`
    // the one `--depends` used to.
    if args.no_tags {
        task.tags.clear();
    }
    if args.no_depends {
        task.depends.clear();
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `just test-one --test cli edit_no_depends`
Expected: PASS.

- [ ] **Step 5: Update the docs**

`docs/specs/2026-08-29-tasks-design.md`, `edit` entry: add `[--no-depends]` to the
synopsis line that has `[--no-tags]`, and after the sentence ending "--no-tags with --tag
is a wholesale replace." insert:

```
    --depends appends the same way: an id the task already depends on under any
    spelling changes nothing and warns, and every other stored edge keeps its spelling.
    --no-depends clears the list without walking the graph, and --no-depends with
    --depends is a wholesale replace. `dep --rm` removes one.
```

§6.1: in both sentences that list `dep --on` and `add --depends`, make it
"`dep --on` and `--depends` on `add` and `edit`".

`README.md`, after the `tasks edit sci-4f2a9c --tag cli --rm-tag triage` example line:

```
    tasks edit sci-4f2a9c --depends sci-7b1e04       # --depends adds; --no-depends clears
```

`skills/tasks/SKILL.md:161`: insert `/--no-depends` after `--depends` in the flag list,
and append to the sentence after the list that explains `--tag` (the one beginning
"`--tag` adds a tag and leaves the rest alone"): "`--depends` likewise adds;
`--no-depends` clears, and with `--depends` replaces the list."

`skills/curate/SKILL.md:73-74`: replace the bullet with:

```
   - `--spec`, `--plan <topic> --step "<heading>"`, `--parent`: fix when the linked
     thing exists and the link is missing or wrong. A missing dependency is added with
     `--depends`; a wrong one is replaced with `tasks dep <id> --on <right> --rm <wrong>`.
```

- [ ] **Step 6: Gate and commit**

Run: `cargo fmt && just test-fast && just check`
Expected: all pass.

```bash
tasks done <task-2-id> "edit --no-depends clears, and with --depends replaces; spec, README, tasks and curate skills updated"
git add src/cli.rs src/commands/edit.rs tests/cli.rs docs/specs/2026-08-29-tasks-design.md README.md skills/ tasks/
git commit -m "feat(edit): --no-depends clears or, with --depends, replaces dependencies"
```

After both tasks: `cargo install --path .` from the main checkout once the branch is
merged, so the tracker in use is the code under test.
