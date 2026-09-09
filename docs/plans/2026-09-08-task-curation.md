# Task curation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `tasks sample` command that draws random curable tasks, and a `curate` skill
that runs one bounded maintenance pass over what it draws.

**Architecture:** `sample` is a read command beside `list` and `ready`: it scans the scope,
filters to the curable pool (open-and-not-doing, not live-claimed, not recently updated),
draws without replacement with a seedable `fastrand` generator, and returns rows in the
`list` shape. The skill is prose that drives the existing CLI: `sample`, then `show`,
`edit`, and `note`, every one of them run as `tasks -C <root>` so the pass reads and writes
the same checkout.

**Tech Stack:** Rust 2024, clap, `fastrand` (already a dependency), `time`.

**Spec:** `docs/specs/2026-09-08-task-curation-design.md`

## Global Constraints

- **JSON output is the contract.** `sample` returns `{"tasks": [...], "warnings": [...]}`
  with rows in the existing `TaskSummary` shape. No existing shape changes.
- **Fail early with a typed error; no silent fallbacks.** Bad `--older-than` or `--seed`
  values are clap parse errors (exit 2). An empty pool is not an error.
- **`just check` before every commit; `just gate` before the final one.** `check` is
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && tasks check`; the
  pre-commit hook runs it for you and refuses on failure.
- **Conventional commits, no AI-attribution trailers.** The pre-commit hook rejects a
  `Claude-Session:` line; do not add one.
- **There is no library target.** End-to-end tests live in `tests/cli.rs` and run against
  the built binary: `cargo test --test cli <filter>`.
- **Every task ends by reinstalling and closing its task in the same commit as the code**
  (AGENTS.md): `cargo install --path .`, then `tasks done <child-id> "<what landed>"`, and
  the `tasks/*.md` change goes into that commit. The child ids are listed under each task
  heading once the plan is linked; `tasks tree tasks-c3c0d1` shows them.
- **Never edit `tasks/*.md` by hand.** Tests that need a backdated `updated:` use the
  `stamp` helper already in `tests/cli.rs`; that is test scaffolding, not a workflow.
- **Work in the worktree** `.worktrees/curation` on branch `design/curation`.

---

### Task 1: `tasks sample`

**Files:**
- Create: `src/commands/sample.rs`
- Modify: `src/commands/mod.rs` (module list at the top; dispatch `match` near line 735)
- Modify: `src/cli.rs` (add a `Sample` variant after `Next`, around line 215)
- Modify: `docs/specs/2026-08-29-tasks-design.md` (section 5 usage block, after the
  `tasks ready` entry near line 268)
- Modify: `README.md` (the `## Use` block, after the `tasks ready --parallel -n 3` line)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ReadCtx` (`src/commands/mod.rs:300`) with `scope.scan()`,
  `scope.prefixes()`, and `warnings`; `crate::claims::ClaimSnapshot::load(prefixes)` and
  `.live(&id) -> Option<&Claim>` (`src/claims.rs:274`); `crate::time::parse(&str) ->
  Result<OffsetDateTime>` (`src/time.rs`); `TaskSummary::of(task, &all, Some(&claims),
  &registry)` and `ListOut { tasks, warnings, date: DateColumn::Updated }` (`src/output.rs`).
- Produces: `commands::sample::sample(ctx: ReadCtx, count: usize, older_than: u64, seed:
  Option<u64>) -> Result<Output>`; the CLI surface `tasks sample [-n N] [--older-than DAYS]
  [--seed U64] [--project P | --all-projects]`. Task 2's skill calls this surface.

- [x] **Step 1: Write the failing tests**

Append to `tests/cli.rs`. The helpers `id_of`, `stamp`, `write_claim`, and `TestEnv` already
exist in that file and in `tests/common/mod.rs`.

```rust
/// A task backdated far enough to be in the default sample pool.
fn old_task(env: &TestEnv, dir: &std::path::Path, title: &str, status_args: &[&str]) -> String {
    let mut args = vec!["add", title, "-p", "2"];
    args.extend_from_slice(status_args);
    let id = id_of(env.json(dir, &args));
    stamp(dir, &id, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    id
}

fn sampled_ids(v: &serde_json::Value) -> Vec<String> {
    v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn sample_draws_only_from_the_curable_pool() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = old_task(&env, &dir, "Idea", &["--status", "idea"]);
    let todo = old_task(&env, &dir, "Todo", &[]);
    let blocked = id_of(env.json(&dir, &["add", "Blocked", "-p", "2"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    stamp(&dir, &blocked, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let doing = old_task(&env, &dir, "Doing", &[]);
    env.json(&dir, &["start", &doing]);
    stamp(&dir, &doing, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let done = old_task(&env, &dir, "Done", &[]);
    env.json(&dir, &["done", &done]);
    stamp(&dir, &done, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let recent = id_of(env.json(&dir, &["add", "Recent", "-p", "2"]));
    let live = old_task(&env, &dir, "Live claim", &[]);
    write_claim(&env, "sci", &live, "other-session", true);
    let stale = old_task(&env, &dir, "Stale claim", &[]);
    write_claim(&env, "sci", &stale, "gone-session", false);

    let v = env.json(&dir, &["sample", "-n", "10", "--seed", "1"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![idea.clone(), todo.clone(), blocked.clone(), stale.clone()];
    expected.sort();
    assert_eq!(ids, expected, "{v}");
    for absent in [&doing, &done, &recent, &live] {
        assert!(!ids.contains(absent), "{absent} must not be drawn: {v}");
    }
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&live) && w.contains("omitted") && w.contains("other-session")
        }),
        "{v}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("pool holds 4")),
        "{v}"
    );
    // rows are list rows: the same keys, with the claim reported on the stale one
    let row = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == stale)
        .unwrap();
    assert_eq!(row["claim"]["live"], false, "{row}");
    assert!(row.get("child_count").is_some(), "{row}");
}

#[test]
fn sample_older_than_zero_admits_fresh_tasks_and_goals_stay_in() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&dir, &["add", "Child", "-p", "2", "--parent", &goal]));
    // clock skew: a record stamped in the future is "within" every positive window
    let future = id_of(env.json(&dir, &["add", "Future", "-p", "2"]));
    stamp(&dir, &future, "2026-01-01T00:00:00Z", "2030-01-01T00:00:00Z");

    let v = env.json(&dir, &["sample", "-n", "10"]);
    assert_eq!(sampled_ids(&v), Vec::<String>::new(), "{v}");

    let v = env.json(&dir, &["sample", "-n", "10", "--older-than", "0"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![goal, child, future];
    expected.sort();
    assert_eq!(ids, expected, "{v}");
}

#[test]
fn sample_bounds_older_than_at_the_cli() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    old_task(&env, &dir, "T", &[]);
    for bad in ["36501", "10000000", "18446744073709551615", "-1"] {
        let out = env
            .cmd(&dir)
            .args(["sample", "--older-than", bad])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "--older-than {bad} must be a parse error");
    }
    let v = env.json(&dir, &["sample", "--older-than", "36500"]);
    assert!(sampled_ids(&v).is_empty(), "{v}");
}

/// Pins the seeded draw so a change of generator or generator width is caught. The
/// expected indices were computed with fastrand 2.5.0 by the same algorithm
/// (`Rng::with_seed(seed)`, then `rng.u64(i..n)` for each slot) over a pool of twenty
/// ids sorted lexically; on a 32-bit target `Rng::usize` would give different values.
#[test]
fn sample_seeded_draw_is_a_known_answer() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for n in 0..20u32 {
        let id = format!("sci-{n:06x}");
        std::fs::write(
            dir.join(format!("tasks/{id}.md")),
            format!(
                "---\nid: {id}\ntitle: T{n}\nstatus: todo\npriority: 2\n\
                 created: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
        )
        .unwrap();
    }
    let v = env.json(&dir, &["sample", "--seed", "7"]);
    assert_eq!(
        sampled_ids(&v),
        vec!["sci-00000f", "sci-000005", "sci-00000e"],
        "{v}"
    );
    let v = env.json(&dir, &["sample", "--seed", "8"]);
    assert_eq!(
        sampled_ids(&v),
        vec!["sci-000005", "sci-000002", "sci-000000"],
        "{v}"
    );
    let v = env.json(&dir, &["sample", "-n", "5", "--seed", "7"]);
    assert_eq!(
        sampled_ids(&v),
        vec!["sci-00000f", "sci-000005", "sci-00000e", "sci-00000b", "sci-000001"],
        "{v}"
    );
}

#[test]
fn sample_is_reproducible_by_seed_and_defaults_to_three() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for n in 0..20 {
        old_task(&env, &dir, &format!("T{n}"), &[]);
    }
    let a = sampled_ids(&env.json(&dir, &["sample", "--seed", "7"]));
    let b = sampled_ids(&env.json(&dir, &["sample", "--seed", "7"]));
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
    let c = sampled_ids(&env.json(&dir, &["sample", "--seed", "8"]));
    assert_ne!(a, c, "two seeds over twenty tasks should not draw the same three in order");
    let five = sampled_ids(&env.json(&dir, &["sample", "-n", "5", "--seed", "7"]));
    assert_eq!(five.len(), 5);
    let mut unique = five.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5, "without replacement");
}

#[test]
fn sample_of_an_empty_pool_is_empty_with_a_warning() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let v = env.json(&dir, &["sample"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("pool holds 0")),
        "{v}"
    );
    let out = env.cmd(&dir).args(["--pretty", "sample"]).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn sample_scopes_like_the_other_read_commands() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();
    let s = old_task(&env, &sci, "S", &[]);
    let f = old_task(&env, &fam, "F", &[]);

    let v = env.json(&sci, &["sample", "-n", "10", "--project", "fam"]);
    assert_eq!(sampled_ids(&v), vec![f.clone()], "{v}");

    let v = env.json(nowhere.path(), &["sample", "-n", "10", "--all-projects", "--seed", "1"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![s.clone(), f.clone()];
    expected.sort();
    assert_eq!(ids, expected, "{v}");

    let out = env
        .cmd(&sci)
        .args(["sample", "--project", "fam", "--all-projects"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "clap conflict");

    let out = env
        .cmd(&sci)
        .args(["--pretty", "sample", "-n", "10", "--project", "fam"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&f) && text.contains("F"), "{text}");
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli sample_ 2>&1 | tail -20`
Expected: every `sample_` test fails (seven of them); most fail on clap's "unrecognized
subcommand 'sample'" (exit 2) surfacing as a JSON parse panic in `env.json`, and the
bounds test fails on its final `env.json` call.

- [x] **Step 3: Add the CLI variant**

In `src/cli.rs`, after the `Next { .. }` variant:

```rust
    /// Random open tasks for a curation pass: idea, todo, or blocked; not live-claimed;
    /// not updated within --older-than days. Rows are list rows.
    Sample {
        /// How many to draw (without replacement).
        #[arg(short = 'n', long, default_value_t = 3)]
        count: usize,
        /// Exclude tasks updated within this many days (0 to 36500); 0 skips the age
        /// check entirely.
        #[arg(
            long,
            default_value_t = 7,
            value_name = "DAYS",
            value_parser = clap::value_parser!(u64).range(0..=36500)
        )]
        older_than: u64,
        /// Fix the draw so a pass can be reproduced.
        #[arg(long)]
        seed: Option<u64>,
        #[command(flatten)]
        scope: ScopeArgs,
    },
```

- [x] **Step 4: Write the command**

Create `src/commands/sample.rs`:

```rust
//! `tasks sample`: a uniform draw from the curable pool, for the curate skill.
//!
//! The pool is the definition of "worth a maintenance look": open but not in hand
//! (`idea`, `todo`, `blocked`), not live-claimed, and not touched within the age window.
//! Age and status exclusions are silent; a live-claim omission is reported like `ready`
//! reports it, so a reader can tell a small pool from a busy one.

use super::ReadCtx;
use crate::error::Result;
use crate::model::{Status, Task};
use crate::output::{DateColumn, ListOut, Output, TaskSummary};

pub fn sample(
    mut ctx: ReadCtx,
    count: usize,
    older_than: u64,
    seed: Option<u64>,
) -> Result<Output> {
    let all = ctx.scope.scan()?;
    let prefixes = ctx.scope.prefixes();
    let claims = crate::claims::ClaimSnapshot::load(prefixes.iter().map(String::as_str))?;
    // Bounded to <= 36500 at the CLI, so the cast is exact and the subtraction stays far
    // inside OffsetDateTime's range. Zero means no age check at all: a future-dated
    // record from clock skew is still admitted.
    let cutoff = (older_than > 0).then(|| {
        time::OffsetDateTime::now_utc() - time::Duration::days(older_than as i64)
    });

    let mut pool: Vec<&Task> = Vec::new();
    for task in &all {
        if !matches!(task.status, Status::Idea | Status::Todo | Status::Blocked) {
            continue;
        }
        if let Some(cutoff) = cutoff
            && crate::time::parse(&task.updated)? > cutoff
        {
            continue;
        }
        if let Some(claim) = claims.live(&task.id) {
            ctx.warnings.push(format!(
                "{} omitted: claimed by session {} in {}",
                task.id, claim.session, claim.worktree
            ));
            continue;
        }
        pool.push(task);
    }
    // Scan order depends on the filesystem; a seed must not.
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    let pool_size = pool.len();
    if pool_size < count {
        ctx.warnings.push(format!(
            "asked for {count}; the pool holds {pool_size} (open, unclaimed, not updated in {older_than} days)"
        ));
    }

    let mut rng = match seed {
        Some(seed) => fastrand::Rng::with_seed(seed),
        None => fastrand::Rng::new(),
    };
    // Partial Fisher–Yates: the first `take` slots are a uniform draw without replacement.
    // `u64`, not `usize`: fastrand's usize generator differs between 32- and 64-bit
    // targets, and a seed must reproduce the same draw everywhere.
    let take = count.min(pool_size);
    for i in 0..take {
        let j = rng.u64(i as u64..pool_size as u64) as usize;
        pool.swap(i, j);
    }
    let drawn = &pool[..take];

    Ok(Output::List(ListOut {
        tasks: drawn
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    }))
}
```

If `task.id` does not implement `Ord`, sort by its string form instead:
`pool.sort_by_key(|task| task.id.to_string())`.

- [x] **Step 5: Register and dispatch**

In `src/commands/mod.rs`, add `pub mod sample;` to the module list (alphabetical, after
`root`), and in the dispatch `match` after the `Command::Next` arm:

```rust
        Command::Sample {
            count,
            older_than,
            seed,
            scope,
        } => sample::sample(open_read_ctx(dir, &scope)?, count, older_than, seed),
```

- [x] **Step 6: Run the tests to verify they pass**

Run: `cargo test --test cli sample_ 2>&1 | tail -20`
Expected: 7 passed. Then `just check` to confirm fmt and clippy are clean. If clippy
flags the `as i64` cast despite the comment, `i64::try_from(older_than).expect("bounded
to 36500 by the CLI")` is the right replacement: the range is enforced before this code
runs, so an error path here would be dead.

- [x] **Step 7: Document the command**

In `docs/specs/2026-08-29-tasks-design.md` section 5, after the `tasks ready` entry:

```
tasks sample [-n N] [--older-than DAYS] [--seed U64] [--project P | --all-projects]
    N tasks (default 3) drawn uniformly without replacement from the curable pool: status
    idea, todo, or blocked; no live claim; updated more than DAYS days ago (default 7,
    at most 36500; 0 skips the age check, so even a future-dated record is admitted).
    Rows are list rows. Fewer than N in the pool returns the
    pool with a warning; an empty pool is an empty list, exit 0. --seed fixes the draw.
    Live-claim omissions are warned like ready's. The read side of the curate skill
    (docs/specs/2026-09-08-task-curation-design.md).
```

In `README.md`, in the `## Use` block after the `tasks ready --parallel -n 3` line:

```
    tasks sample -n 3                # random open tasks for a curation pass (see skills/curate)
```

- [x] **Step 8: Reinstall, close the task, commit**

```bash
cargo install --path .
tasks done tasks-d23dde "tasks sample: curable pool, seedable draw, list-shaped rows"
git add src/cli.rs src/commands/mod.rs src/commands/sample.rs tests/cli.rs \
        docs/specs/2026-08-29-tasks-design.md README.md tasks/
git commit -m "feat(sample): draw random curable tasks for a curation pass"
```

---

### Task 2: The curate skill

**Files:**
- Create: `skills/curate/SKILL.md`
- Modify: `skills/tasks/SKILL.md` (the "Recording work" list; add one bullet at its end)
- Modify: `AGENTS.md` (the Layout list, the `skills/tasks/SKILL.md` line)
- Modify: `README.md` (the `## Agent skill` section, near line 147)

**Interfaces:**
- Consumes: `tasks sample` from Task 1; existing `tasks -C <dir>`, `root`, `show`, `tree`,
  `list`, `edit`, `note`. `show` JSON has `task.status`, `task.updated`, and `claim`
  (`null` or an object with `live: bool`).
- Produces: the skill file; nothing in code depends on it.

- [x] **Step 1: Write the skill**

Create `skills/curate/SKILL.md` with exactly this content:

````markdown
---
name: curate
description: Use when asked to curate, review, tidy, or audit the task corpus (`/curate`). Samples random open tasks and runs one bounded maintenance pass over them; never part of the session protocol.
---

# curate

One pass improves the tasks it draws. It creates no tasks, drops none, and changes no
priority; those are proposals for the human at the end. Invoke deliberately:

    /curate [n] [--project <prefix> | --all-projects]

Default: three tasks from the current project.

## 1. Sample

    tasks sample -n <n> [--project <prefix> | --all-projects]

The pool is already the right one: `idea`, `todo`, or `blocked`; no live claim; not updated
in the last 7 days. Read the warnings: a shortfall names the pool size, and each live-claim
omission names the holder.

## 2. One root per task

`sample --project` and `--all-projects` read registered roots, but `show`, `edit`, and
`note` prefer the current checkout when the id's prefix matches it. From a worktree you
could sample one copy of a task and rewrite another. So, before touching a sampled task,
fix its root and run **every** later command for it as `tasks -C <root> ...`:

- `sample` ran unscoped: the root is the current directory.
- `sample` ran with `--project` or `--all-projects`: the root is the path that
  `tasks --pretty root <id>` prints. The default JSON form is an object; its `root`
  field is the same path. Never pass the JSON to `-C`.

Grep, `git log`, and every other piece of evidence gathering run in that same root.

## 3. Per task

1. **Read.** `tasks -C <root> show <id>`; keep `task.updated`. Read the spec, plan, and
   parent it links to. `tasks -C <root> tree <id>` when it has children.
2. **Evidence.** Grep the code and docs the task names. Does the described thing already
   exist in the tree? If so, find the commit (`git log -S` or `git log --grep`). Search
   open titles and tags for a probable duplicate: `tasks -C <root> list` and read titles.
   For a goal, check that its children cover it.
3. **Verdict**, exactly one:
   - `keep`: clear, current, correctly linked; nothing to change.
   - `refined`: prose or links improved; meaning unchanged.
   - `stale`: the described thing landed, or its premise no longer holds. Proposal: drop,
     with the commit.
   - `duplicate`: another open task covers it. Proposal: drop or merge, naming the other id.
   - `decision`: not actionable without a choice the human owns. Write the questions into
     the record (step 5); the summary asks.
   - `decompose`: a goal is missing children it needs. Proposal: the children, one line
     each.
4. **Revalidate**, immediately before the first write: `tasks -C <root> show <id>` again.
   Skip the task, writing nothing, and report the reason if any of these hold:
   - `task.status` is not `idea`, `todo`, or `blocked` (became active);
   - `claim` is present with `live: true` (claimed; a stale claim does not count);
   - `task.updated` differs from the stamp taken in step 1 (changed).
   This is a check, not a lock. A session that starts the task in the seconds between
   the recheck and your write gets a prose edit and a `curate:` note on a task it holds;
   the note says what happened. Accepted.
5. **Edit**, only through the CLI, only these:
   - `--title`, `--body`: rewrite for clarity and brevity. Keep every fact. State the
     assumptions the text implies. Collect questions you cannot answer under one
     `## Open questions` heading at the end of the body; reuse an existing one. Notes
     are stored apart from the body and survive the rewrite.
   - `--spec`, `--plan <topic> --step "<heading>"`, `--parent`, `--depends`: fix when
     the linked thing exists and the link is missing or wrong.
   - `--size`: set or correct.
   - `--tag`: add a tag the project already uses (`tasks -C <root> tags`) when it
     clearly applies. Never invent one.
   Never: status, priority, `--parallel`, `--source`, `add`, `drop`, or `tasks/*.md`
   by hand.
6. **Note.** Exactly one:

       tasks -C <root> note <id> "curate: <verdict>; <what changed>[; proposal: <text>]"

   The `proposal:` segment is present only for `stale`, `duplicate`, `decision`, and
   `decompose`. The note is the audit trail and moves the task out of the pool for the
   age window.

## Pending proposals

A task whose most recent note is a `curate:` note carrying a `proposal:` segment is not
re-curated. Report it as `pending` with the proposal text and move on. Any later note
clears it: the human records the decision with `tasks note <id> "<decision>"` whether
they acted on the proposal or declined it. A `keep` or `refined` task simply re-enters
the pool after the age window; reviewing it again is the maintenance.

## Bounds

- Zero new tasks per pass. Curation improves what exists; adding is a proposal.
- Prefer shorter. A rewrite that grows the body without adding a fact is wrong.
- Touch only the sampled tasks. A duplicate found in passing is named in the proposal,
  not edited.

## Summary to the human

One line per sampled task: id, verdict (or `skipped: <reason>` / `pending: <proposal>`),
one phrase of what changed. Then the decisions that are the human's, grouped, omitting
empty groups:

- **drops**: id, and the commit or the duplicate id;
- **priority changes**: id, from, to, why;
- **children to add**: the goal, then one line per child;
- **questions**: id, then the open questions a `decision` verdict wrote into the record.

Nothing else. A pass with nothing in those groups says so in one line.

## What a good task reads like

- The title names the outcome, not the activity.
- The body answers why, what done looks like, and where to look. It repeats nothing the
  linked spec already says.
- Assumptions are sentences, not implications. Unknowns are open questions, not hedges.
- A body is as short as those three answers allow.

## Kinds

Task kinds (templates) are derived from passes, not written up front. After a handful of
passes, cluster what was seen; that work is tasks-5b73bf in the tasks project.
````

- [x] **Step 2: Point the other docs at it**

In `skills/tasks/SKILL.md`, at the end of the "Recording work" bullet list:

```markdown
- Curating the corpus (random open tasks, one bounded maintenance pass each): the
  `curate` skill, shipped beside this one. Never part of the session protocol.
```

In `AGENTS.md`, replace the Layout line for the tasks skill with:

```markdown
- `skills/tasks/SKILL.md` — the agent skill shipped to other projects; keep it in step with CLI changes.
  `skills/curate/SKILL.md` — the curation pass (`tasks sample`, then bounded edits); same rule.
```

In `README.md`, in the `## Agent skill` section after the two `ln -s ... skills/tasks`
lines, add the matching pair for curate:

```
    ln -s "$PWD/skills/curate" ~/.claude/skills/curate
    ln -s "$PWD/skills/curate" ~/.agents/skills/curate   # other harnesses
```

and one sentence after the install block: "`skills/curate/SKILL.md` is the maintenance
pass: `/curate` samples open tasks and refines them within fixed bounds."

- [x] **Step 3: Dry-run the skill against this repository**

Without writing anything, walk sections 1 through 3 step 4 for one task, to prove every
command the skill names exists and yields the fields the skill reads:

```bash
tasks sample -n 1 --seed 1 --project tasks
id=<the id it printed>
root=$(tasks --pretty root "$id")
tasks -C "$root" show "$id" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["task"]["status"], d["task"]["updated"], d["claim"])'
tasks -C "$root" tree "$id" --pretty
tasks -C "$root" tags --pretty
```

Expected: a status in `idea|todo|blocked`, an RFC 3339 stamp, and `None` or a claim
object; the tree and tags print. If any command errors, the skill text is wrong; fix the
text, not the CLI.

- [x] **Step 4: Check and commit**

```bash
just check
tasks done tasks-470076 "curate skill: sample, root, evidence, verdict, revalidate, bounded edits, summary"
git add skills/curate/SKILL.md skills/tasks/SKILL.md AGENTS.md README.md tasks/
git commit -m "feat(skills): add the curate skill for bounded task maintenance passes"
```

---

### Task 3: Close-out

**Files:**
- Modify: `docs/specs/2026-09-08-task-curation-design.md` (the `Status:` line)
- Modify: `tasks/` via the CLI only

- [x] **Step 1: Correct the spec status**

Change the status line at the top of `docs/specs/2026-09-08-task-curation-design.md` to:

```
Status: implemented (2026-09-09)
```

Then grep for the same claim elsewhere and fix any drift:

```bash
grep -rn 'task-curation' README.md AGENTS.md docs/ skills/
```

- [x] **Step 2: Run the full gate**

Run: `just gate`
Expected: fmt, clippy, `tasks check`, and `cargo test` all pass. Paste the last lines of
the output in the commit body if anything was non-obvious.

- [x] **Step 3: Run one real pass**

Now that the binary and the skill are installed, run `/curate 3 --project tasks` in a
fresh session (or follow the skill by hand here). This is the skill's acceptance test.
Record what the pass produced in a note on the goal:

```bash
tasks note tasks-c3c0d1 "first pass: <n> tasks, verdicts <list>, <k> proposals"
```

If the pass exposed a defect in the skill text, fix it in this task and mention it in the
same note.

- [x] **Step 4: Close the goal and commit**

```bash
tasks done tasks-3c2dfb "spec status, gate, first pass"
tasks done tasks-c3c0d1 "tasks sample and the curate skill landed; kinds follow in tasks-5b73bf"
git add docs/specs/2026-09-08-task-curation-design.md skills/ tasks/
git commit -m "docs(curation): mark the design implemented and record the first pass"
```

Then hand off with the finishing-a-development-branch skill: the branch is
`design/curation` in `.worktrees/curation`, merging into `main`.
