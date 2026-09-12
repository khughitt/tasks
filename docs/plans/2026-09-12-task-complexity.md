# Task Complexity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `complexity` rating on task records, a `--max-complexity` cutoff (flag and `TASKS_MAX_COMPLEXITY`) honoured by every picker feed, an escalation entry in the shared claim store written by `park --reason capability`, a `check` warning for unrated plan steps, and rename support for the new entry.

**Architecture:** `Complexity` is a three-level enum beside `Size` in `model.rs`, parsed and serialized with the other optional frontmatter scalars. A new `src/complexity.rs` owns cutoff resolution, the effective-rating rule, and the hide-and-count filter that `ready`, `next`, and `prime` apply. The claim store gains a third map, `escalations`, whose entries survive `start` and later parks and are removed only by an explicit `edit --complexity`/`--no-complexity` or by closing the task. `park` gains `--complexity`, valid only with `--reason capability`.

**Tech Stack:** Rust 2024, clap 4 (derive + clap_complete), serde + toml, assert_cmd end-to-end tests in `tests/cli.rs`.

**Spec:** `docs/specs/2026-09-12-task-complexity-design.md`

## Global Constraints

- Levels are exactly `low`, `mid`, `high`, ordered low < mid < high; absent means unassessed and is never defaulted (spec §3.1, §4.3).
- JSON output is the contract: every addition is a new key; no existing key changes shape (AGENTS.md, spec §3.2).
- Fail early with a typed error; no silent fallbacks (AGENTS.md).
- `tasks/*.md` is written only by the binary; tests go through the CLI.
- Gate before every commit: `just check` (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`); `just test` is `cargo test`. The pre-commit hook runs `check`.
- The crate is a binary only: unit tests run as `cargo test --bin tasks <filter>`; there is no `--lib` target. Test-only constructors carry `#[cfg(test)]` so the warnings-as-errors gate never sees them unused.
- Conventional commits; no AI-attribution trailers.
- Warning strings are exactly the spec's: `max-complexity <level>: <n> above cutoff hidden` and `max-complexity <level>: <n> unassessed hidden` (§4.3).
- After the last task, `cargo install --path .` so the tracker used by the session protocol is the code under test.

---

### Task 1: The `Complexity` type and its frontmatter field

**Files:**
- Modify: `src/model.rs` (after `impl Size`, around line 262; `Task` struct around line 300)
- Modify: `src/format.rs` (`KEYS` allowlist line 6; `parse_task` around line 100; `serialize_task` around line 344; tests around line 415)
- Modify: `src/repo.rs:626` (a `Task` literal with `size: None`)
- Test: unit tests in `src/model.rs` and `src/format.rs`

**Interfaces:**
- Produces: `crate::model::Complexity` — `enum Complexity { Low, Mid, High }` deriving `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize` with `#[serde(rename_all = "lowercase")]`; `Complexity::ALL: [Complexity; 3]`; `Complexity::parse(&str) -> Result<Complexity>`; `Complexity::as_str(self) -> &'static str`.
- Produces: `Task.complexity: Option<Complexity>`, frontmatter key `complexity`, written only when set, placed directly after `size`.

- [ ] **Step 1: Write the failing unit tests**

In `src/model.rs` tests module, after `size_order`:

```rust
    #[test]
    fn complexity_order_and_parse() {
        assert!(Complexity::Low < Complexity::Mid);
        assert!(Complexity::Mid < Complexity::High);
        for level in Complexity::ALL {
            assert_eq!(Complexity::parse(level.as_str()).unwrap(), level);
        }
        let error = Complexity::parse("medium").unwrap_err().to_string();
        assert!(error.contains("low, mid, high"), "{error}");
    }
```

In `src/format.rs` tests module, after `minimal_roundtrip`:

```rust
    #[test]
    fn complexity_round_trips_after_size_and_rejects_unknown_levels() {
        let text = MINIMAL.replace("priority: 2\n", "priority: 2\nsize: m\ncomplexity: mid\n");
        // Through the allowlist first: an unlisted key is rejected before any field parses.
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.complexity, Some(Complexity::Mid));
        assert_eq!(serialize_task(&t), text);
        let bad = MINIMAL.replace("priority: 2\n", "priority: 2\ncomplexity: medium\n");
        let error = parse_task(&bad, "x").unwrap_err().to_string();
        assert!(error.contains("low, mid, high"), "{error}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks complexity`
Expected: compile error — `Complexity` is not defined and `Task` has no `complexity` field.

- [ ] **Step 3: Add the enum and the field**

In `src/model.rs`, after `impl Size { ... }`:

```rust
/// The reasoning and judgment a task demands given its current spec, plan, and context.
/// Three levels, ordered; absent means unassessed and is never defaulted. See
/// docs/specs/2026-09-12-task-complexity-design.md §3.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    Low,
    Mid,
    High,
}

impl Complexity {
    pub const ALL: [Complexity; 3] = [Complexity::Low, Complexity::Mid, Complexity::High];

    pub fn parse(s: &str) -> Result<Complexity> {
        Complexity::ALL
            .into_iter()
            .find(|level| level.as_str() == s)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "unknown complexity {s:?}; expected one of low, mid, high"
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Complexity::Low => "low",
            Complexity::Mid => "mid",
            Complexity::High => "high",
        }
    }
}
```

In the `Task` struct, directly after `pub size: Option<Size>,`:

```rust
    /// The judgment the task demands; absent is unassessed. Set by `add`/`edit
    /// --complexity` and by `park --reason capability`. See
    /// docs/specs/2026-09-12-task-complexity-design.md.
    pub complexity: Option<Complexity>,
```

In `src/format.rs`, the `KEYS` allowlist at line 6 rejects any frontmatter key it does not name before field parsing runs, so a saved rated task would fail to load without this: change `const KEYS: [&str; 21]` to `[&str; 22]` and add `"complexity",` directly after `"size",`.

In `parse_task`, directly after the `size:` initializer:

```rust
        complexity: scalar("complexity")?
            .map(|s| Complexity::parse(&s))
            .transpose()
            .map_err(|e| perr(file, e.to_string()))?,
```

and add `Complexity` to the `use crate::model::{...}` line at the top of `format.rs`.

In `serialize_task`, directly after the `size` push:

```rust
    if let Some(level) = t.complexity {
        pairs.push(("complexity".into(), s(level.as_str())));
    }
```

In `src/repo.rs:626`, after `size: None,` add `complexity: None,`.

- [ ] **Step 4: Build and fix every other `Task` literal**

Run: `cargo build --all-targets 2>&1 | grep -n "missing field .complexity" -A3`
Add `complexity: None,` after `size` in each reported literal (expect `src/query.rs` tests around line 237 and possibly `src/hierarchy.rs`, `src/periodic.rs`, `src/similarity.rs` tests).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --bin tasks complexity`
Expected: 2 passed.

Run: `cargo test`
Expected: all pass (the existing `full_roundtrip` fixtures have no `complexity` line and stay byte-identical).

- [ ] **Step 6: Commit**

```bash
git add src/model.rs src/format.rs src/repo.rs src/query.rs
git commit -m "feat(model): complexity rating on the task record"
```

---

### Task 2: `--complexity` on `add` and `edit`, `--no-complexity`, JSON rows, pretty column, completions

**Files:**
- Modify: `src/cli.rs` (`FieldArgs` after `size`, line ~54; `EditArgs` after `no_source`, line ~101)
- Modify: `src/commands/mod.rs` (`apply_fields`, line ~425)
- Modify: `src/commands/edit.rs` (`has_flags`, line ~55; clearing block, line ~87)
- Modify: `src/complete.rs` (after `sizes`, line ~40)
- Modify: `src/output.rs` (`TaskSummary` line ~140, `TaskSummary::of` line ~252, `ParkedRow` line ~284, `ParkedRow::resolved` line ~310, `ParkedRow::unresolved` line ~329, table row line ~1001 and the format string line ~1045)
- Modify: `README.md:94`, `skills/tasks/SKILL.md` (the `tasks add` scoped-task line and the `tasks edit` flag list)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Complexity` from Task 1.
- Produces: `FieldArgs.complexity: Option<String>`, `EditArgs.no_complexity: bool`, `crate::complete::complexities()`, `TaskSummary.complexity: Option<Complexity>`, `ParkedRow.complexity: Option<Complexity>`.

- [ ] **Step 1: Write the failing end-to-end test**

Append to `tests/cli.rs`:

```rust
#[test]
fn complexity_is_set_cleared_listed_and_completed() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Rate me", "-p", "2", "--complexity", "mid"]));
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "mid");
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"][0]["complexity"], "mid");
    let text = std::fs::read_to_string(sci.join(format!("tasks/{id}.md"))).unwrap();
    assert!(text.contains("\ncomplexity: mid\n"), "{text}");

    let pretty = env.pretty(&sci, &["list"]);
    assert!(pretty.contains(" mid "), "{pretty}");

    env.json(&sci, &["edit", &id, "--complexity", "high"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["complexity"], "high");
    env.json(&sci, &["edit", &id, "--no-complexity"]);
    assert!(env.json(&sci, &["show", &id])["task"]["complexity"].is_null());
    let pretty = env.pretty(&sci, &["list"]);
    assert!(pretty.contains(" -    "), "unassessed shows a dash: {pretty}");

    assert_eq!(env.fail(&sci, &["add", "Bad", "--complexity", "medium"]), "validation");
    assert_eq!(env.fail(&sci, &["edit", &id, "--complexity", "5"]), "validation");
    let out = env.cmd(&sci).args(["edit", &id, "--complexity", "low", "--no-complexity"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "clap conflict is a usage error");

    let values = env.complete_values(&sci, "bash", 4, &["tasks", "add", "T", "--complexity", ""]);
    assert_eq!(values, vec!["low", "mid", "high"]);
    let values = env.complete_values(&sci, "zsh", 3, &["tasks", "edit", &id, "--complexity", ""]);
    assert!(values.iter().any(|value| value.starts_with("high")), "{values:?}");
}
```

(Check `complete_values`' signature at `tests/common/mod.rs:172` and match the existing `--size` completion test near `tests/cli.rs` grep `"--size", ""` for the index convention; adjust the index arguments to that convention.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli complexity_is_set_cleared_listed_and_completed`
Expected: FAIL — `unexpected argument '--complexity'`.

- [ ] **Step 3: Add the flags**

In `src/cli.rs` `FieldArgs`, after the `size` field:

```rust
    /// The judgment the task demands: low, mid, or high. Absent is unassessed.
    #[arg(long, add = ArgValueCandidates::new(crate::complete::complexities))]
    pub complexity: Option<String>,
```

In `EditArgs`, after `no_source`:

```rust
    /// Clear the complexity rating (back to unassessed).
    #[arg(long, conflicts_with = "complexity")]
    pub no_complexity: bool,
```

In `src/complete.rs`, after `sizes`:

```rust
/// The three `--complexity` / `--max-complexity` levels.
pub fn complexities() -> Vec<CompletionCandidate> {
    plain(Complexity::ALL.iter().map(|level| level.as_str()))
}
```

and add `Complexity` to the `use crate::model::{...}` import in `complete.rs`.

- [ ] **Step 4: Apply and clear the field**

In `src/commands/mod.rs` `apply_fields`, after the `size` block:

```rust
    if let Some(level) = &fields.complexity {
        task.complexity = Some(Complexity::parse(level)?);
    }
```

Add `Complexity` to the `use crate::model::{...}` import in `commands/mod.rs`.

In `src/commands/edit.rs` `has_flags`, add `|| fields.complexity.is_some() || args.no_complexity` after `|| fields.size.is_some()`. In the clearing block after `if args.no_source { ... }`:

```rust
    if args.no_complexity {
        task.complexity = None;
    }
```

(Task 7 adds the escalation clearing beside this; leave a plain field clear here.)

- [ ] **Step 5: JSON rows and the pretty column**

In `src/output.rs`:

- `TaskSummary`: after `pub size: Option<Size>,` add `pub complexity: Option<Complexity>,`.
- `TaskSummary::of`: after `size: task.size,` add `complexity: task.complexity,`.
- `ParkedRow`: after `pub size: Option<Size>,` add `pub complexity: Option<Complexity>,`.
- `ParkedRow::resolved`: after `size: summary.size,` add `complexity: summary.complexity,`.
- `ParkedRow::unresolved`: after `size: None,` add `complexity: None,`.
- Table row (line ~1001): after `let size = row.size.map(Size::as_str).unwrap_or("-");` add
  `let complexity = row.complexity.map(Complexity::as_str).unwrap_or("-");` and change the
  format string to `"{id}  {priority} {size:<2} {complexity:<4} {status} {mark}{date}  {}{tags}{cadence}{owner}\n"`.
- Add `Complexity` to the `use crate::model::{...}` import.

Check `tree_text` and the parked table (`parked_table`) for a copy of the same row format; if they share `table` they need nothing, if they carry their own format string, add the column there too so rows align.

- [ ] **Step 6: Run the test and the suite**

Run: `cargo test --test cli complexity_is_set_cleared_listed_and_completed`
Expected: PASS.

Run: `cargo test`
Expected: all pass. If a pretty-table snapshot test fails on the new column, update its expected string — the column is intended.

- [ ] **Step 7: Document the flag**

`README.md:94`: change the example to
`tasks add "Bank the ledger" -p 1 --size m --complexity low --tag ledger`.

`skills/tasks/SKILL.md`: in the "Recording work" scoped-task line, add `--complexity <low|mid|high>` after `--size <xs|s|m|l|xl>`; in the `tasks edit` flag list add `--complexity`/`--no-complexity` after `--size`. The rubric and protocol text land in Task 10.

- [ ] **Step 8: Gate and commit**

Run: `just check && cargo test`
Expected: clean.

```bash
git add src/cli.rs src/commands/mod.rs src/commands/edit.rs src/complete.rs src/output.rs tests/cli.rs README.md skills/tasks/SKILL.md
git commit -m "feat(cli): --complexity on add and edit, list column, completions"
```

---

### Task 3: The escalation entry in the claim store

**Files:**
- Modify: `src/claims.rs` (`Park` struct line ~185; `StoreFile` line ~201; `ClaimStore` line ~209; `load_from` line ~246; `save` line ~285; accessors line ~320; `ClaimSnapshot` line ~430)
- Test: unit tests in `src/claims.rs` (existing tests module near line 1100)

**Interfaces:**
- Produces: `crate::claims::Escalation { level: Complexity, at: String, session: String }` deriving `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`.
- Produces on `ClaimStore`: `escalation(&TaskId) -> Option<&Escalation>`, `insert_escalation(&TaskId, Escalation)`, `remove_escalation(&TaskId) -> Option<Escalation>`, `escalations() -> impl Iterator<Item = (&String, &Escalation)>`, `carries_nothing(&self) -> bool` (no parks and no escalations).
- Produces on `ClaimSnapshot`: `escalation(&TaskId) -> Option<&Escalation>`.

- [ ] **Step 1: Write the failing unit test**

In the `src/claims.rs` tests module, after `park_and_remove_park_read_and_clear_one_entry`:

```rust
    #[test]
    fn escalations_round_trip_and_are_independent_of_claims_and_parks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        let id = TaskId::parse("sci-4f2a9c").unwrap();
        let mut store = ClaimStore::load_from(&path).unwrap();
        assert!(store.carries_nothing());
        store.insert_escalation(
            &id,
            Escalation {
                level: crate::model::Complexity::High,
                at: "2026-09-12T10:00:00Z".into(),
                session: "s:agent-a".into(),
            },
        );
        assert!(!store.carries_nothing());
        store.save().unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[escalations.sci-4f2a9c]"), "{text}");
        assert!(text.contains("level = \"high\""), "{text}");

        let mut store = ClaimStore::load_from(&path).unwrap();
        assert_eq!(
            store.escalation(&id).map(|e| e.level),
            Some(crate::model::Complexity::High)
        );
        // A claim on the same id leaves the escalation alone; so does a park.
        store.insert(&id, sample_claim());
        assert!(store.escalation(&id).is_some());
        store.remove(&id);
        store.insert_park(&id, sample_park());
        assert!(store.escalation(&id).is_some());
        assert!(store.remove_escalation(&id).is_some());
        assert!(store.remove_escalation(&id).is_none());

        let bad = "[escalations.sci-4f2a9c]\nlevel = \"high\"\nat = \"yesterday\"\nsession = \"s\"\n";
        std::fs::write(&path, bad).unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err().to_string();
        assert!(error.contains("escalation sci-4f2a9c has an unreadable at"), "{error}");
    }
```

Look at the existing test `park_and_remove_park_read_and_clear_one_entry` (line ~1121) for how it builds a `Claim` and a `Park`; if there are no `sample_claim()` / `sample_park()` helpers, copy the literals it uses into two small `fn sample_claim() -> Claim` / `fn sample_park() -> Park` helpers in the tests module.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --bin tasks escalations_round_trip`
Expected: compile error — `Escalation` and the accessors do not exist.

- [ ] **Step 3: Add the struct, the map, and the accessors**

In `src/claims.rs`, after the `Park` struct:

```rust
/// A session under a cutoff found the task needs more reasoning than it could supply and
/// raised its rating. Shared outside git so every checkout's picker sees the new level
/// before the record merges; survives `start` and later parks; removed only by an explicit
/// `edit --complexity`/`--no-complexity` or by closing the task. See
/// docs/specs/2026-09-12-task-complexity-design.md §5.1.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Escalation {
    pub level: crate::model::Complexity,
    pub at: String,
    /// Scheme-tagged (`Identity::tagged`); opaque to tasks.
    pub session: String,
}
```

In `StoreFile` add a third field:

```rust
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    escalations: BTreeMap<String, Escalation>,
```

In `ClaimStore` add `escalations: BTreeMap<String, Escalation>,`.

In `load_from`: destructure `let StoreFile { claims, parks, escalations } = file;`, and after the parks loop add:

```rust
        for (id, escalation) in &escalations {
            crate::time::parse(&escalation.at).map_err(|error| {
                Error::Config(format!(
                    "{}: escalation {id} has an unreadable at: {error}",
                    path.display()
                ))
            })?;
        }
```

and include `escalations` in the returned `ClaimStore`. In `save`, add `escalations: self.escalations.clone(),` to the `StoreFile` literal.

`parks_renamed_text` (line ~347) also builds a `StoreFile` and will not compile without the field. Until Task 9 teaches `rename` to carry escalations, give it `escalations: BTreeMap::new(),` with the comment `// Not carried until rename learns escalations (plan Task 9); a rename in between drops them.` — an explicit interim, not a fallback, and Task 9 replaces the whole function.

Accessors, after `remove_park`:

```rust
    pub fn escalation(&self, id: &TaskId) -> Option<&Escalation> {
        self.escalations.get(&id.to_string())
    }

    /// Replaces any earlier entry; the caller has already checked the level never falls.
    pub fn insert_escalation(&mut self, id: &TaskId, escalation: Escalation) {
        self.escalations.insert(id.to_string(), escalation);
    }

    pub fn remove_escalation(&mut self, id: &TaskId) -> Option<Escalation> {
        self.escalations.remove(&id.to_string())
    }

    pub fn escalations(&self) -> impl Iterator<Item = (&String, &Escalation)> {
        self.escalations.iter()
    }

    /// Nothing `rename` would need to carry: no parks and no escalations. Claims are
    /// never carried; a rename refuses while any are live.
    pub fn carries_nothing(&self) -> bool {
        self.parks.is_empty() && self.escalations.is_empty()
    }
```

`insert` and `insert_park` stay as they are: neither touches `escalations`.

- [ ] **Step 4: The read-side snapshot**

In `ClaimSnapshot` (line ~430) add a field `escalations: BTreeMap<String, Escalation>`; in `load_from_paths` add `let mut escalations = BTreeMap::new();` and inside the per-store loop:

```rust
            for (id, escalation) in store.escalations() {
                escalations.insert(id.clone(), escalation.clone());
            }
```

Return it in the struct literal, and add:

```rust
    pub fn escalation(&self, id: &TaskId) -> Option<&Escalation> {
        self.escalations.get(&id.to_string())
    }
```

Grep for every other `ClaimSnapshot { by_id, parks }` literal (tests) and add `escalations: BTreeMap::new()`.

- [ ] **Step 5: Run the tests**

Run: `cargo test --bin tasks claims`
Expected: all pass, including the new one.

- [ ] **Step 6: Commit**

```bash
git add src/claims.rs
git commit -m "feat(claims): escalation entries in the shared store"
```

---

### Task 4: The cutoff module

**Files:**
- Create: `src/complexity.rs`
- Modify: `src/main.rs` (the `mod` list) — add `mod complexity;`
- Test: unit tests in `src/complexity.rs`

**Interfaces:**
- Produces: `crate::complexity::ENV: &str = "TASKS_MAX_COMPLEXITY"`.
- Produces: `crate::complexity::cutoff(flag: Option<&str>) -> Result<Option<Complexity>>` — validates the variable whenever called, flag wins.
- Produces: `crate::complexity::cutoff_with(flag: Option<&str>, env: Option<Result<String, std::env::VarError>>) -> Result<Option<Complexity>>` — the injectable form for tests (the gate runs tests in parallel; never `set_var`).
- Produces: `crate::complexity::effective(task: &Task, claims: &ClaimSnapshot) -> Option<Complexity>`.
- Produces: `crate::complexity::Hidden { above: usize, unassessed: usize }` (derive `Debug, Default, PartialEq, Eq`).
- Produces: `crate::complexity::apply(tasks: &mut Vec<Task>, cutoff: Complexity, claims: &ClaimSnapshot) -> Hidden`.
- Produces: `crate::complexity::warnings(cutoff: Complexity, hidden: &Hidden) -> Vec<String>`.

- [ ] **Step 1: Write the failing unit tests**

Create `src/complexity.rs` with the tests first:

```rust
//! The cutoff a session picks under, and the effective rating it is compared with.
//! See docs/specs/2026-09-12-task-complexity-design.md §4.

use crate::claims::ClaimSnapshot;
use crate::error::{Error, Result};
use crate::model::{Complexity, Task};

pub const ENV: &str = "TASKS_MAX_COMPLEXITY";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Status, TaskId};
    use std::collections::BTreeMap;

    fn task(id: &str, complexity: Option<Complexity>) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: "t".into(),
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity,
            parallel: false,
            every: None,
            owner: None,
            created: "2026-09-12T00:00:00Z".into(),
            updated: "2026-09-12T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    fn snapshot_with(id: &str, level: Complexity) -> ClaimSnapshot {
        let mut escalations = BTreeMap::new();
        escalations.insert(
            id.to_string(),
            crate::claims::Escalation {
                level,
                at: "2026-09-12T00:00:00Z".into(),
                session: "s:a".into(),
            },
        );
        ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), escalations)
    }

    #[test]
    fn flag_wins_and_the_variable_is_validated_regardless() {
        assert_eq!(cutoff_with(None, None).unwrap(), None);
        assert_eq!(cutoff_with(None, Some(Ok(String::new()))).unwrap(), None);
        assert_eq!(
            cutoff_with(None, Some(Ok("mid".into()))).unwrap(),
            Some(Complexity::Mid)
        );
        assert_eq!(
            cutoff_with(Some("low"), Some(Ok("mid".into()))).unwrap(),
            Some(Complexity::Low)
        );
        let error = cutoff_with(Some("high"), Some(Ok("huge".into()))).unwrap_err();
        assert!(error.to_string().contains("TASKS_MAX_COMPLEXITY"), "{error}");
        assert!(cutoff_with(Some("huge"), None).is_err());
    }

    #[test]
    fn effective_takes_the_higher_of_record_and_escalation() {
        let empty = ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        assert_eq!(effective(&task("sci-000001", None), &empty), None);
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Low)), &empty),
            Some(Complexity::Low)
        );
        let escalated = snapshot_with("sci-000001", Complexity::High);
        assert_eq!(effective(&task("sci-000001", None), &escalated), Some(Complexity::High));
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Low)), &escalated),
            Some(Complexity::High)
        );
        let lower = snapshot_with("sci-000001", Complexity::Low);
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Mid)), &lower),
            Some(Complexity::Mid)
        );
    }

    #[test]
    fn apply_hides_above_and_unassessed_and_counts_each() {
        let empty = ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let mut tasks = vec![
            task("sci-000001", Some(Complexity::Low)),
            task("sci-000002", Some(Complexity::Mid)),
            task("sci-000003", Some(Complexity::High)),
            task("sci-000004", None),
        ];
        let hidden = apply(&mut tasks, Complexity::Mid, &empty);
        assert_eq!(hidden, Hidden { above: 1, unassessed: 1 });
        let ids: Vec<String> = tasks.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(ids, vec!["sci-000001", "sci-000002"]);
        assert_eq!(
            warnings(Complexity::Mid, &hidden),
            vec![
                "max-complexity mid: 1 above cutoff hidden",
                "max-complexity mid: 1 unassessed hidden"
            ]
        );
        assert!(warnings(Complexity::Mid, &Hidden::default()).is_empty());
    }
}
```

This needs a `ClaimSnapshot::from_parts(by_id, parks, escalations)` constructor. Add it to `src/claims.rs` beside `load_from_paths`:

```rust
    /// The literal form, for tests that need a snapshot without files.
    #[cfg(test)]
    pub fn from_parts(
        by_id: BTreeMap<String, (Claim, Liveness)>,
        parks: BTreeMap<String, Park>,
        escalations: BTreeMap<String, Escalation>,
    ) -> ClaimSnapshot {
        ClaimSnapshot { by_id, parks, escalations }
    }
```

Add `mod complexity;` to `src/main.rs` next to the other `mod` lines.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks complexity::tests`
Expected: compile error — `cutoff_with`, `effective`, `apply`, `warnings`, `Hidden` undefined.

- [ ] **Step 3: Implement the module**

Above the tests module in `src/complexity.rs`:

```rust
/// The cutoff in force: the flag, else `TASKS_MAX_COMPLEXITY`. The variable is validated
/// whenever a picker runs, even when the flag overrides it, so a harness with a bad value
/// hears about it at once (spec §4.2).
pub fn cutoff(flag: Option<&str>) -> Result<Option<Complexity>> {
    cutoff_with(flag, Some(std::env::var(ENV)))
}

/// `env` is the variable's lookup result, injected so tests never touch the process
/// environment; `None` stands for an unset variable.
pub fn cutoff_with(
    flag: Option<&str>,
    env: Option<std::result::Result<String, std::env::VarError>>,
) -> Result<Option<Complexity>> {
    let from_env = match env {
        None | Some(Err(std::env::VarError::NotPresent)) => None,
        Some(Err(std::env::VarError::NotUnicode(_))) => {
            return Err(Error::Validation(format!("{ENV} is not valid UTF-8")));
        }
        Some(Ok(value)) if value.is_empty() => None,
        Some(Ok(value)) => Some(Complexity::parse(&value).map_err(|_| {
            Error::Validation(format!(
                "{ENV} must be one of low, mid, high, got {value:?}"
            ))
        })?),
    };
    match flag {
        Some(flag) => Ok(Some(Complexity::parse(flag)?)),
        None => Ok(from_env),
    }
}

/// The record's rating or the shared escalation, whichever is higher (spec §4.1).
pub fn effective(task: &Task, claims: &ClaimSnapshot) -> Option<Complexity> {
    let escalated = claims.escalation(&task.id).map(|escalation| escalation.level);
    match (task.complexity, escalated) {
        (Some(record), Some(escalation)) => Some(record.max(escalation)),
        (record, escalation) => record.or(escalation),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Hidden {
    pub above: usize,
    pub unassessed: usize,
}

/// Removes what the cutoff hides and counts it by cause. Unassessed is hidden, never
/// defaulted (spec §4.3).
pub fn apply(tasks: &mut Vec<Task>, cutoff: Complexity, claims: &ClaimSnapshot) -> Hidden {
    let mut hidden = Hidden::default();
    tasks.retain(|task| match effective(task, claims) {
        None => {
            hidden.unassessed += 1;
            false
        }
        Some(level) if level > cutoff => {
            hidden.above += 1;
            false
        }
        Some(_) => true,
    });
    hidden
}

pub fn warnings(cutoff: Complexity, hidden: &Hidden) -> Vec<String> {
    let mut out = Vec::new();
    if hidden.above > 0 {
        out.push(format!(
            "max-complexity {}: {} above cutoff hidden",
            cutoff.as_str(),
            hidden.above
        ));
    }
    if hidden.unassessed > 0 {
        out.push(format!(
            "max-complexity {}: {} unassessed hidden",
            cutoff.as_str(),
            hidden.unassessed
        ));
    }
    out
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --bin tasks complexity::tests`
Expected: 3 passed. `cargo clippy --all-targets -- -D warnings` may flag the public functions as dead code until Task 5 wires them; if so, add `#[allow(dead_code)]` on the module line in `main.rs` and remove it in Task 5.

- [ ] **Step 5: Commit**

```bash
git add src/complexity.rs src/main.rs src/claims.rs
git commit -m "feat(complexity): cutoff resolution and the effective-rating filter"
```

---

### Task 5: `--max-complexity` on `ready` and `next`; the variable on `prime`

**Files:**
- Modify: `src/cli.rs` (`Ready` line ~219, `Next` line ~231)
- Modify: `src/commands/mod.rs` (dispatch of `Ready` line ~887, `Next` line ~892)
- Modify: `src/commands/list.rs` (`ready` line ~250, `next` line ~282, `prime` line ~321)
- Modify: `src/commands/parked.rs` (`candidates` line ~89 — unchanged signature; the filter is applied by the caller)
- Modify: `tests/common/mod.rs` (`TestEnv::cmd` line ~19 and `TestEnv::raw` line ~38)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `crate::complexity::{cutoff, apply, warnings}` (Task 4).
- Produces: `list::ready(ctx, size, parallel, limit, max_complexity: Option<String>)`, `list::next(ctx, max_complexity: Option<String>)`.

- [ ] **Step 1: Isolate the suite from the harness's own cutoff**

The picker now reads `TASKS_MAX_COMPLEXITY`, and the suite may run inside a harness that sets it; every unrated fixture would vanish from `ready`. In `tests/common/mod.rs`, add `.env_remove("TASKS_MAX_COMPLEXITY")` after `.env_remove("TASKS_MODEL")` in **both** `cmd` and `raw`. Cutoff tests then set the variable explicitly on the command.

- [ ] **Step 2: Write the failing end-to-end tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn ready_and_next_hide_above_cutoff_and_unassessed_with_counts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "1", "--complexity", "low"]));
    let mid = id_of(env.json(&sci, &["add", "Mid", "-p", "2", "--complexity", "mid"]));
    let high = id_of(env.json(&sci, &["add", "High", "-p", "0", "--complexity", "high"]));
    let none = id_of(env.json(&sci, &["add", "Unassessed", "-p", "0"]));

    let v = env.json(&sci, &["ready"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 4, "no cutoff, nothing hidden");
    assert!(v["warnings"].as_array().unwrap().is_empty());

    let v = env.json(&sci, &["ready", "--max-complexity", "mid"]);
    let ids: Vec<&str> = v["tasks"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec![low.as_str(), mid.as_str()], "priority order kept: {v}");
    let warnings: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(
        warnings,
        vec!["max-complexity mid: 1 above cutoff hidden", "max-complexity mid: 1 unassessed hidden"]
    );

    let v = env.json(&sci, &["ready", "--max-complexity", "low", "-n", "1"]);
    assert_eq!(v["tasks"][0]["id"], low);
    let warnings: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, vec!["max-complexity low: 2 above cutoff hidden", "max-complexity low: 1 unassessed hidden"]);

    let v = env.json(&sci, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], low, "{v}");

    let v = env.json(&sci, &["ready", "--max-complexity", "high"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 3);
    let warnings: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, vec!["max-complexity high: 1 unassessed hidden"]);

    // The cutoff composes with --size and --parallel, and its counts are the cutoff's
    // alone: the size and parallel filters run after it and are not counted (spec §4.1).
    env.json(&sci, &["edit", &low, "--size", "s", "--parallel"]);
    env.json(&sci, &["edit", &mid, "--size", "m"]);
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--size", "s"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let warnings: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, vec!["max-complexity mid: 1 above cutoff hidden", "max-complexity mid: 1 unassessed hidden"]);
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--parallel"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--parallel", "--size", "m"]);
    assert!(v["tasks"].as_array().unwrap().is_empty());

    // Across projects the counts are one total for the scope, not one line per project.
    let fam = env.init("fam");
    env.json(&fam, &["add", "Fam unassessed", "-p", "2"]);
    env.json(&fam, &["add", "Fam high", "-p", "2", "--complexity", "high"]);
    let v = env.json(&sci, &["ready", "--all-projects", "--max-complexity", "mid"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2, "{v}");
    let warnings: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, vec!["max-complexity mid: 2 above cutoff hidden", "max-complexity mid: 2 unassessed hidden"]);
    let v = env.json(&fam, &["next", "--all-projects", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], low, "priority order across the scope");

    assert_eq!(env.fail(&sci, &["ready", "--max-complexity", "huge"]), "validation");
    let _ = (high, none);
}

#[test]
fn the_cutoff_variable_drives_ready_next_and_prime_and_the_flag_wins() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "2", "--complexity", "low"]));
    let high = id_of(env.json(&sci, &["add", "High", "-p", "0", "--complexity", "high"]));
    let with_env = |args: &[&str], value: &str| -> serde_json::Value {
        let out = env.cmd(&sci).env("TASKS_MAX_COMPLEXITY", value).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let v = with_env(&["ready"], "low");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let v = with_env(&["next"], "low");
    assert_eq!(v["next"]["task"]["id"], low);
    let v = with_env(&["prime"], "low");
    assert_eq!(v["ready"].as_array().unwrap().len(), 1, "{v}");
    assert!(v["warnings"].as_array().unwrap().iter().any(|w| w == "max-complexity low: 1 above cutoff hidden"));
    let v = with_env(&["ready"], "");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2, "empty means no cutoff");

    // The flag wins over the variable.
    let v = with_env(&["ready", "--max-complexity", "high"], "low");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2);
    // An invalid variable fails even when the flag is given.
    let out = env.cmd(&sci).env("TASKS_MAX_COMPLEXITY", "huge").args(["ready", "--max-complexity", "high"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let out = env.cmd(&sci).env("TASKS_MAX_COMPLEXITY", "huge").args(["prime"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let _ = high;
}

#[test]
fn next_skips_parked_work_above_the_cutoff_and_falls_through() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let parked = id_of(env.json(&sci, &["add", "Parked high", "-p", "0", "--complexity", "high"]));
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &sci, "agent-a").args(["park", &parked, "continue"]).assert().success();

    let v = env.json(&sci, &["next"]);
    assert_eq!(v["next"]["task"]["id"], parked, "parked work waiting on the agent comes first");
    let v = env.json(&sci, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], low, "{v}");
    let v = env.json(&sci, &["next", "--max-complexity", "low"]);
    assert_eq!(v["next"]["task"]["id"], low);
    env.json(&sci, &["edit", &low, "--complexity", "high"]);
    let v = env.json(&sci, &["next", "--max-complexity", "low"]);
    assert!(v["next"].is_null(), "{v}");
    assert!(v["warnings"].as_array().unwrap().iter().any(|w| w == "max-complexity low: 2 above cutoff hidden"), "{v}");
}

#[test]
fn prime_closeout_is_filtered_by_the_goals_own_rating() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2", "-b", "committed"]));
    let child = id_of(env.json(&sci, &["add", "Child", "--parent", &goal]));
    env.json(&sci, &["done", &child, "landed"]);
    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["closeout"][0]["id"], goal);
    let out = env.cmd(&sci).env("TASKS_MAX_COMPLEXITY", "mid").arg("prime").output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["closeout"].as_array().unwrap().is_empty(), "unrated goal hidden: {v}");
    assert!(v["warnings"].as_array().unwrap().iter().any(|w| w == "max-complexity mid: 1 unassessed hidden"), "{v}");
    env.json(&sci, &["edit", &goal, "--complexity", "low"]);
    let out = env.cmd(&sci).env("TASKS_MAX_COMPLEXITY", "mid").arg("prime").output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["closeout"][0]["id"], goal);
}
```

Check the exact JSON keys of `prime` output (`PrimeOut` in `src/output.rs`, grep `pub struct PrimeOut`) — the ready section may be named `ready` and closeout `closeout`; adjust the test if they differ.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --test cli cutoff`
Expected: FAIL — `unexpected argument '--max-complexity'`.

- [ ] **Step 4: Add the flags and thread them**

In `src/cli.rs`, in `Ready` after `size`:

```rust
        /// Hide tasks rated above this level and unassessed tasks; overrides
        /// TASKS_MAX_COMPLEXITY.
        #[arg(long, value_name = "LEVEL", add = ArgValueCandidates::new(crate::complete::complexities))]
        max_complexity: Option<String>,
```

and the same field in `Next` before `scope`.

In `src/commands/mod.rs` dispatch:

```rust
        Command::Ready { size, parallel, limit, max_complexity, scope } => {
            list::ready(open_read_ctx(dir, &scope)?, size, parallel, limit, max_complexity)
        }
        Command::Next { max_complexity, scope } => {
            list::next(open_read_ctx(dir, &scope)?, max_complexity)
        }
```

- [ ] **Step 5: Apply the cutoff in the three pickers**

In `src/commands/list.rs` `ready`, add the parameter `max_complexity: Option<String>` and resolve the cutoff before the scan (so a bad flag or variable fails before any work):

```rust
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let size = size.map(|size| Size::parse(&size)).transpose()?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let mut tasks = ready_tasks(&mut ctx, &all, &claims, now)?;
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut tasks, cutoff, &claims);
        ctx.warnings.extend(crate::complexity::warnings(cutoff, &hidden));
    }
    if let Some(size) = size {
```

In `next`, add `max_complexity: Option<String>` and:

```rust
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let _ = super::parked::rows(&mut ctx, &all, &claims, now)?;
    let candidates = super::parked::candidates(&mut ctx, &all, &claims)?;
    let ready = ready_tasks(&mut ctx, &all, &claims, now)?;
    // One pool in pick order — parked candidates first, then the ready list — with each
    // task once, so a parked todo that is also ready is hidden and counted once.
    let mut pool = candidates;
    for task in ready {
        if !pool.iter().any(|candidate| candidate.id == task.id) {
            pool.push(task);
        }
    }
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut pool, cutoff, &claims);
        ctx.warnings.extend(crate::complexity::warnings(cutoff, &hidden));
    }
    let next = match pool.into_iter().next() {
```

The counts are distinct tasks: in `next_skips_parked_work_above_the_cutoff_and_falls_through` the parked `high` task is both a candidate and ready, and the final call reports `2 above cutoff hidden`, not 3.

In `prime`, after `let ready = ready_tasks(...)?;` make it `let mut ready = ...;`, and after the closeout loop's `sort_ready(&mut closeout);`:

```rust
    if let Some(cutoff) = crate::complexity::cutoff(None)? {
        let mut hidden = crate::complexity::apply(&mut ready, cutoff, &claims);
        let from_closeout = crate::complexity::apply(&mut closeout, cutoff, &claims);
        hidden.above += from_closeout.above;
        hidden.unassessed += from_closeout.unassessed;
        ctx.warnings.extend(crate::complexity::warnings(cutoff, &hidden));
    }
```

Move the `cutoff(None)?` call to the top of `prime` (before the scan) so an invalid variable fails first, holding the result in a `let cutoff = ...;` and using it here.

Remove any `#[allow(dead_code)]` added in Task 4.

- [ ] **Step 6: Run the tests and the suite**

Run: `cargo test --test cli cutoff && cargo test --test cli next_skips && cargo test --test cli prime_closeout`
Expected: PASS.

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Gate and commit**

Run: `just check`

```bash
git add src/cli.rs src/commands/mod.rs src/commands/list.rs src/main.rs tests/cli.rs tests/common/mod.rs
git commit -m "feat(picker): --max-complexity on ready and next, TASKS_MAX_COMPLEXITY on prime"
```

---

### Task 6: `park --reason capability --complexity`, the escalation write, and its recovery contract

**Files:**
- Modify: `src/claims.rs` (`Reason` enum line ~128; `ALL`, `as_str`, the doc comment "six-word")
- Modify: `src/cli.rs` (`Park` line ~279)
- Modify: `src/commands/mod.rs` (`ClaimIntent::Park` line ~37; `save`'s Park branch line ~760; dispatch line ~903)
- Modify: `src/commands/park.rs`
- Modify: `src/complete.rs` (`reason` doc comment)
- Modify: `src/output.rs` (`TaskSummary`, `TaskSummary::of`, `ShowFields`, `ParkedRow`, `show_text`)
- Modify: `src/commands/show.rs` (`describe`, line ~115)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Escalation`, `ClaimStore::{escalation, insert_escalation}` (Task 3); `crate::complexity::{cutoff, ENV}` (Task 4).
- Produces: `Reason::Capability` (`"capability"`); `ClaimIntent::Park { park: Park, escalation: Option<Escalation> }`; `park::run(ctx, id, next_step, waiting_on, reason, complexity: Option<String>)`; `TaskSummary.escalation`, `ShowFields.escalation`, `ParkedRow.escalation`: `Option<Escalation>`.

- [ ] **Step 1: Write the failing end-to-end tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn capability_park_validates_the_level_and_records_the_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "low"]));

    assert_eq!(env.fail(&sci, &["park", &id, "stuck", "--complexity", "high"]), "validation");
    assert_eq!(env.fail(&sci, &["park", &id, "stuck", "--reason", "capability"]), "validation");
    assert_eq!(env.fail(&sci, &["park", &id, "stuck", "--reason", "capability", "--complexity", "medium"]), "validation");
    // Never below the effective rating.
    env.json(&sci, &["edit", &id, "--complexity", "mid"]);
    assert_eq!(env.fail(&sci, &["park", &id, "stuck", "--reason", "capability", "--complexity", "low"]), "validation");

    // Under a cutoff the level must exceed it.
    let under = |value: &str, level: &str| {
        as_agent(&env, &sci, "agent-a")
            .env("TASKS_MAX_COMPLEXITY", value)
            .args(["park", &id, "needs a decision the plan leaves open", "--reason", "capability", "--complexity", level])
            .output()
            .unwrap()
    };
    let out = under("mid", "mid");
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    let out = under("high", "high");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--waiting-on user"));
    let out = under("mid", "high");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "high");
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(v["escalation"]["session"], "s:agent-a", "check the tagged form in an existing park test and adjust");
    assert_eq!(v["park"]["reason"], "capability");
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"][0]["escalation"]["level"], "high");
    let pretty = env.pretty(&sci, &["show", &id]);
    assert!(pretty.contains("# escalation"), "{pretty}");
    assert!(pretty.contains("agent, capability"), "{pretty}");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("[escalations."), "{store}");

    // A record already high accepts high under a lower or absent cutoff (the retry shape).
    let out = under("mid", "high");
    assert!(out.status.success());
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "again", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
}

#[test]
fn capability_park_waiting_on_the_user_records_no_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "high"]));
    as_agent(&env, &sci, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "high")
        .args(["park", &id, "decompose this", "--reason", "capability", "--waiting-on", "user"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["escalation"].is_null(), "{v}");
    assert_eq!(v["park"]["waiting_on"], "user");
    let v = env.json(&sci, &["ready"]);
    assert!(v["tasks"].as_array().unwrap().is_empty(), "parked on the user is omitted");
    // --complexity is optional here but still never lowers.
    assert_eq!(
        env.fail(&sci, &["park", &id, "x", "--reason", "capability", "--waiting-on", "user", "--complexity", "low"]),
        "validation"
    );
}

#[test]
fn capability_park_fails_loudly_when_the_store_write_fails_and_the_rerun_succeeds() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "low"]));
    let plain = id_of(env.json(&sci, &["add", "P", "-p", "2"]));
    let store = env.claim_store("sci");
    // Create the store directory and the lock so only the atomic temp file fails.
    as_agent(&env, &sci, "agent-a").args(["park", &plain, "warm up"]).assert().success();
    use std::os::unix::fs::PermissionsExt;
    let state_dir = store.parent().unwrap();
    let original = std::fs::metadata(state_dir).unwrap().permissions();
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let capability = as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "stuck", "--reason", "capability", "--complexity", "high"])
        .output()
        .unwrap();
    let ordinary = as_agent(&env, &sci, "agent-a")
        .args(["park", &plain, "later", "--reason", "session"])
        .output()
        .unwrap();
    std::fs::set_permissions(state_dir, original).unwrap();

    assert_eq!(capability.status.code(), Some(1), "{}", String::from_utf8_lossy(&capability.stdout));
    let error: serde_json::Value = serde_json::from_slice(&capability.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("rerun the same `tasks park` command"), "{message}");
    assert!(message.contains(&format!("escalation of {id} to high")), "{message}");
    assert!(ordinary.status.success(), "an ordinary park keeps the warning contract");

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "high", "the record write landed");
    assert!(v["escalation"].is_null(), "the store write did not");

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "stuck", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(v["park"]["reason"], "capability");
}
```

Look at `tests/cli.rs:756` (`park_reason_rides_the_entry_the_note_and_every_park_view`) for the exact tagged session string the JSON reports and correct the `"s:agent-a"` expectation to that form. Check the error JSON key for the message (`error.message` or `error.detail`) in `src/main.rs` or an existing `fail`-style test and adjust.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli capability_park`
Expected: FAIL — `unexpected argument '--complexity'` for `park`, and `capability` rejected as a reason.

- [ ] **Step 3: The seventh reason**

In `src/claims.rs`: add `Capability` to `enum Reason` (last), change `ALL` to `[Reason; 7]` including `Reason::Capability`, add `Reason::Capability => "capability"` to `as_str`, and update the doc comment to "the seven-word vocabulary of docs/specs/2026-09-11-park-reason-and-stamps-design.md §4 and docs/specs/2026-09-12-task-complexity-design.md §5". In `src/complete.rs` change the `reason` doc comment to "The seven `park --reason` accepts." Grep `tests/cli.rs` and `src/claims.rs` for a test enumerating six reasons (`"six"` or an array of the six names) and add `capability`.

- [ ] **Step 4: The flag and the intent**

In `src/cli.rs` `Park`, after `reason`:

```rust
        /// With --reason capability: the rating the work actually needs. Written to the
        /// record and, when waiting on the agent, to the shared store as an escalation.
        #[arg(long, value_name = "LEVEL", add = ArgValueCandidates::new(crate::complete::complexities))]
        complexity: Option<String>,
```

Update the `reason` doc comment on `Park` to list `capability`.

In `src/commands/mod.rs`, change the intent variant:

```rust
    Park {
        park: crate::claims::Park,
        escalation: Option<crate::claims::Escalation>,
    },
```

and the dispatch to pass `complexity`:

```rust
        Command::Park { id, next_step, waiting_on, reason, complexity } => park::run(
            open_id_write_ctx(dir, &id)?,
            id,
            next_step,
            waiting_on,
            reason,
            complexity,
        ),
```

- [ ] **Step 5: Validation in `park::run`**

Replace the body of `src/commands/park.rs` `run` from the `let reason = ...` line through the `ctx.pending_claim = ...` line with:

```rust
    let reason = reason.as_deref().map(Reason::parse).transpose()?;
    let complexity = complexity.as_deref().map(Complexity::parse).transpose()?;
    let owner = owner_name(&ctx.project)?;
    let me = crate::claims::identity()?;

    // The claim rules of §3.1, as a guard: a live foreign claim refuses (no --force), a
    // stale one is taken over with the warning `start` gives, our own is simply replaced.
    let takeover = {
        let store = ctx.claims_mut()?;
        match store.get(&task.id) {
            Some(existing) if existing.session != me.session => {
                let live = crate::claims::liveness(existing);
                let description = Ctx::describe_claim(existing, &live);
                match live {
                    Liveness::Live => {
                        return Err(Error::Claimed(task.id.to_string(), description));
                    }
                    Liveness::Stale(_) => Some(format!("took over {description}")),
                }
            }
            _ => None,
        }
    };
    if let Some(warning) = takeover {
        ctx.warnings.push(warning);
    }

    // Spec §5: --complexity belongs to --reason capability alone; the level never falls
    // below the effective rating; waiting on the agent it is required and must clear the
    // variable's cutoff, and an escalation is recorded; waiting on the user nothing is.
    let escalation = match (reason, complexity) {
        (Some(Reason::Capability), level) => {
            let escalated = ctx
                .claims_mut()?
                .escalation(&task.id)
                .map(|escalation| escalation.level);
            let current = match (task.complexity, escalated) {
                (Some(record), Some(escalated)) => Some(record.max(escalated)),
                (record, escalated) => record.or(escalated),
            };
            if let (Some(level), Some(current)) = (level, current)
                && level < current
            {
                return Err(Error::Validation(format!(
                    "--complexity {} is below the effective rating {} of {}",
                    level.as_str(),
                    current.as_str(),
                    task.id
                )));
            }
            match waiting_on {
                WaitingOn::User => None,
                WaitingOn::Agent => {
                    let level = level.ok_or_else(|| {
                        Error::Validation(
                            "--reason capability waiting on the agent needs --complexity <level>"
                                .into(),
                        )
                    })?;
                    if let Some(cutoff) = crate::complexity::cutoff(None)? {
                        if cutoff == Complexity::High {
                            return Err(Error::Validation(format!(
                                "{}=high leaves no level to escalate to; park --waiting-on user so a person can decompose or reassign it",
                                crate::complexity::ENV
                            )));
                        }
                        if level <= cutoff {
                            return Err(Error::Validation(format!(
                                "--complexity {} does not exceed {}={}",
                                level.as_str(),
                                crate::complexity::ENV,
                                cutoff.as_str()
                            )));
                        }
                    }
                    Some(Escalation {
                        level,
                        at: crate::time::now(),
                        session: me.tagged.clone(),
                    })
                }
            }
        }
        (_, Some(_)) => {
            return Err(Error::Validation(
                "--complexity on park needs --reason capability".into(),
            ));
        }
        (_, None) => None,
    };
    if let Some(level) = complexity {
        task.complexity = Some(level);
    }

    append_note(
        &mut task,
        &owner,
        &format!(
            "parked (waiting on {}): {next_step}",
            describe_stop(waiting_on, reason)
        ),
    )?;
    let park = Park {
        owner,
        session: me.tagged,
        host: crate::claims::hostname(),
        worktree: ctx.project.root.display().to_string(),
        at: crate::time::now(),
        next_step,
        waiting_on,
        reason,
        title: task.title.clone(),
    };
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::Park { park, escalation }));
```

Add `complexity: Option<String>` as the last parameter of `run`, and extend the imports: `use crate::claims::{Escalation, Liveness, Park, Reason, WaitingOn, describe_stop};` and `use crate::model::Complexity;`. Keep the note text as it is — the rating change is visible in the record and the escalation, and the note already names the reason.

- [ ] **Step 6: The store write and its failure**

In `src/commands/mod.rs` `save`, replace the Park branch:

```rust
        Some((id, ClaimIntent::Park { park, escalation })) => {
            // Record first, then store: a note with no entry is a trail that says what was
            // intended, while an entry with no note would be state the record never saw.
            ctx.project.write_task(&ctx.registry, task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.insert_park(&id, park);
            if let Some(escalation) = &escalation {
                store.insert_escalation(&id, escalation.clone());
            }
            if let Err(error) = store.save() {
                // For an escalation the store write is the cross-checkout guarantee
                // (spec §5.2), so its failure is the command's failure; the raised
                // rating stays, and equal-to-current lets the rerun pass.
                if let Some(escalation) = escalation {
                    return Err(Error::Validation(format!(
                        "the note and rating landed, but the escalation of {id} to {} was not \
                         recorded ({error}); rerun the same `tasks park` command",
                        escalation.level.as_str()
                    )));
                }
                ctx.warnings.push(format!(
                    "the note landed, but parking on {id} was not updated ({error}); a previous \
                     park entry, if any, is intact"
                ));
            }
            Ok(())
        }
```

- [ ] **Step 7: Report the escalation**

In `src/output.rs`:

- `TaskSummary`: after `pub park: Option<ParkInfo>,` add `pub escalation: Option<crate::claims::Escalation>,`; in `TaskSummary::of` after the `park:` initializer add
  `escalation: claims.and_then(|snapshot| snapshot.escalation(&task.id)).cloned(),`.
- `ShowFields`: after `pub park: Option<ParkInfo>,` add `pub escalation: Option<crate::claims::Escalation>,`.
- `ParkedRow`: add the same field; `resolved` copies `summary.escalation`; `unresolved` sets `escalation: None`.
- `show_text`, after the `# parked` block:

```rust
    if let Some(escalation) = &o.escalation {
        rendered.push_str("\n# escalation\n");
        rendered.push_str(&format!(
            "- needs at least {} since {}\n",
            escalation.level.as_str(),
            crate::time::day(&escalation.at)
        ));
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("  session {}", escalation.session),
        ));
        rendered.push('\n');
    }
```

In `src/commands/show.rs` `describe`, after the `park:` initializer:

```rust
        escalation: claims
            .and_then(|snapshot| snapshot.escalation(&task.id))
            .cloned(),
```

- [ ] **Step 8: Run the tests and the suite**

Run: `cargo test --test cli capability_park`
Expected: 3 passed.

Run: `cargo test`
Expected: all pass; fix any test that enumerates the reason vocabulary.

- [ ] **Step 9: Gate and commit**

Run: `just check`

```bash
git add src/claims.rs src/cli.rs src/commands/mod.rs src/commands/park.rs src/commands/show.rs src/complete.rs src/output.rs tests/cli.rs
git commit -m "feat(park): --reason capability --complexity records an escalation"
```

---

### Task 7: Reassessment and closing clear the escalation; the two-worktree lifecycle

**Files:**
- Modify: `src/commands/mod.rs` (`ClaimIntent` line ~35; a new `Ctx::reassess` beside `preserve_claim_store` line ~104; `save`'s Release branch line ~736 and a new branch)
- Modify: `src/commands/edit.rs` (after the status handling, before `save`, line ~130)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ClaimStore::remove_escalation` (Task 3).
- Produces: `ClaimIntent::ClearEscalation(Escalation)`; `Ctx::reassess(&mut self, id: &TaskId) -> Result<()>`; private `Ctx.clear_escalation: bool`.

Note on the lifecycle test: `park` never changes status, so a checkout that has `start`ed the task stays `doing` and a later `edit --status doing` is a same-status edit (the `ClearEscalation` branch, task file first). The acquire rollback is only reached from `todo`; the test sets and asserts that precondition.

- [ ] **Step 1: Write the failing end-to-end tests**

Append to `tests/cli.rs`:

```rust
/// A second checkout of `sci` holding a copy of `id`'s record as it was before the escalation.
fn second_checkout(env: &mut TestEnv, first: &std::path::Path, id: &str) -> std::path::PathBuf {
    let second = tempfile::tempdir().unwrap();
    let path = second.path().canonicalize().unwrap();
    std::fs::create_dir_all(path.join("tasks")).unwrap();
    std::fs::write(path.join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();
    std::fs::copy(first.join(format!("tasks/{id}.md")), path.join(format!("tasks/{id}.md"))).unwrap();
    std::mem::forget(second);
    path
}

#[test]
fn an_escalation_governs_every_checkout_through_resume_and_reparking() {
    let mut env = TestEnv::new();
    // A is the registered root and stays stale throughout; B and C are unregistered
    // checkouts holding the record as it was before the escalation.
    let a = env.init("sci");
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--complexity", "low"]));
    let b = second_checkout(&mut env, &a, &id);
    let c = second_checkout(&mut env, &a, &id);

    // The escalation is made from B, so the registered root never sees the raised record.
    as_agent(&env, &b, "agent-b")
        .env("TASKS_MAX_COMPLEXITY", "mid")
        .args(["park", &id, "interacting behaviour outside the assessed scope", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "low", "A's record is stale");
    assert_eq!(v["escalation"]["level"], "high", "the store is shared");

    for dir in [&a, &b, &c] {
        let v = env.json(dir, &["next", "--max-complexity", "mid"]);
        assert!(v["next"].is_null(), "{}: {v}", dir.display());
        let v = env.json(dir, &["ready", "--max-complexity", "mid"]);
        assert!(v["tasks"].as_array().unwrap().is_empty());
        assert!(v["warnings"].as_array().unwrap().iter().any(|w| w == "max-complexity mid: 1 above cutoff hidden"), "{v}");
    }
    let v = env.json(&a, &["next"]);
    assert_eq!(v["next"]["task"]["id"], id, "an unrestricted session still gets it");

    // --all-projects reads the registered root's stale record with the shared store.
    let v = env.json(&c, &["next", "--all-projects", "--max-complexity", "mid"]);
    assert!(v["next"].is_null(), "{v}");
    let v = env.json(&b, &["ready", "--all-projects", "--max-complexity", "mid"]);
    assert!(v["tasks"].as_array().unwrap().is_empty());
    assert!(v["warnings"].as_array().unwrap().iter().any(|w| w == "max-complexity mid: 1 above cutoff hidden"), "{v}");

    // A stronger session resumes in C and parks again for the session.
    as_agent(&env, &c, "agent-c").args(["start", &id]).assert().success();
    as_agent(&env, &c, "agent-c").args(["park", &id, "half done", "--reason", "session"]).assert().success();
    for dir in [&a, &b, &c] {
        let v = env.json(dir, &["next", "--max-complexity", "mid"]);
        assert!(v["next"].is_null(), "{}: escalation must survive start and re-park: {v}", dir.display());
    }
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(v["park"]["reason"], "session");

    // A stale checkout cannot lower it through another capability park.
    let out = as_agent(&env, &a, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "low")
        .args(["park", &id, "x", "--reason", "capability", "--complexity", "mid"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("effective rating high"));
    as_agent(&env, &a, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "low")
        .args(["park", &id, "x", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["park"]["reason"], "capability", "A's park replaced C's");

    // A combined status-and-rating edit whose task write fails must leave the escalation
    // and the previous park standing: the acquire path saves the store first and rolls
    // back on failure. C must be `todo` so the edit is a transition into `doing`.
    as_agent(&env, &c, "agent-c").args(["edit", &id, "--status", "todo"]).assert().success();
    let v = env.json(&c, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo", "precondition: the edit below acquires");
    assert!(v["claim"].is_null(), "precondition: no claim to displace");
    assert_eq!(v["park"]["reason"], "capability", "precondition: a park to restore");
    use std::os::unix::fs::PermissionsExt;
    let c_tasks = c.join("tasks");
    let original = std::fs::metadata(&c_tasks).unwrap().permissions();
    std::fs::set_permissions(&c_tasks, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &c, "agent-c")
        .args(["edit", &id, "--status", "doing", "--complexity", "mid"])
        .output()
        .unwrap();
    std::fs::set_permissions(&c_tasks, original).unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["escalation"]["level"], "high", "rolled back with the park: {v}");
    assert_eq!(v["park"]["reason"], "capability", "the previous park is back: {v}");
    assert!(v["claim"].is_null(), "the acquired claim was rolled back: {v}");
    let v = env.json(&c, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "low", "the record write never landed");

    // Explicit reassessment from C clears it, with a warning naming what was overridden.
    let v = env.json(&c, &["edit", &id, "--complexity", "mid"]);
    let warning = v["warnings"][0].as_str().unwrap();
    assert!(warning.contains("cleared the escalation of") && warning.contains("to high"), "{warning}");
    let v = env.json(&c, &["show", &id]);
    assert!(v["escalation"].is_null(), "{v}");
    let v = env.json(&c, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], id, "C offers it at its own mid");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(!store.contains("[escalations."), "{store}");
}

#[test]
fn reassessment_clears_on_every_edit_shape_and_closing_clears_too() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let escalate = |id: &str| {
        as_agent(&env, &sci, "agent-a")
            .args(["park", id, "stuck", "--reason", "capability", "--complexity", "high"])
            .assert()
            .success();
    };
    let has_escalation = |id: &str| !env.json(&sci, &["show", id])["escalation"].is_null();

    let same_status = id_of(env.json(&sci, &["add", "Same", "-p", "2", "--complexity", "low"]));
    escalate(&same_status);
    env.json(&sci, &["note", &same_status, "still there"]);
    env.json(&sci, &["edit", &same_status, "--size", "s"]);
    assert!(has_escalation(&same_status), "notes and unrelated edits leave it");
    env.json(&sci, &["edit", &same_status, "--status", "todo", "--complexity", "low"]);
    assert!(!has_escalation(&same_status), "a same-status edit still persists the clear");

    let no_level = id_of(env.json(&sci, &["add", "Clear", "-p", "2", "--complexity", "low"]));
    escalate(&no_level);
    env.json(&sci, &["edit", &no_level, "--no-complexity"]);
    assert!(!has_escalation(&no_level));

    let transition = id_of(env.json(&sci, &["add", "Scoped", "--status", "idea", "--complexity", "low"]));
    escalate(&transition);
    env.json(&sci, &["edit", &transition, "--status", "todo", "-p", "2", "--complexity", "mid"]);
    assert!(!has_escalation(&transition));

    let closed = id_of(env.json(&sci, &["add", "Done", "-p", "2", "--complexity", "low"]));
    escalate(&closed);
    env.json(&sci, &["done", &closed, "landed"]);
    assert!(!has_escalation(&closed));
    let dropped = id_of(env.json(&sci, &["add", "Dropped", "-p", "2", "--complexity", "low"]));
    escalate(&dropped);
    env.json(&sci, &["drop", &dropped, "no longer needed"]);
    assert!(!has_escalation(&dropped));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli escalation`
Expected: FAIL on the first `assert!(!has_escalation(...))` or the "must survive" assertion (the escalation is never cleared today, and `edit` reports no warning).

- [ ] **Step 3: The intent and `reassess`**

The removal must not happen before `save`: the Acquire branch writes the store *before* the task file and rolls the claim and park back when the task write fails, and an escalation removed up front would not come back. So `reassess` only records the intent, and every `save` branch performs the removal at the point where it can be undone or is already safe.

In `src/commands/mod.rs` add a variant to `ClaimIntent`:

```rust
    /// An explicit rating replaced an escalation; the store must be saved even on a
    /// same-status edit, which otherwise preserves it.
    ClearEscalation(crate::claims::Escalation),
```

Add a private field to `Ctx`: `clear_escalation: bool,` (initialised `false` in every `Ctx { ... }` literal — grep `recovered: false`), and to `impl Ctx`, after `preserve_claim_store`:

```rust
    /// `edit --complexity` / `--no-complexity`: the explicit rating is the new truth, so the
    /// shared escalation goes, with a warning naming what was overridden (spec §5.1).
    /// Records only; `save` removes the entry where a failed task write can still restore
    /// it. Called after the status handling so a transition's own intent — which saves
    /// the store anyway — is kept, and only a store-preserving intent is replaced.
    pub fn reassess(&mut self, id: &TaskId) -> Result<()> {
        let Some(escalation) = self.claims_mut()?.escalation(id).cloned() else {
            return Ok(());
        };
        self.warnings.push(format!(
            "cleared the escalation of {id} to {} recorded by session {} at {}",
            escalation.level.as_str(),
            escalation.session,
            escalation.at
        ));
        self.clear_escalation = true;
        if matches!(
            self.pending_claim,
            None | Some((_, ClaimIntent::PreserveStore))
        ) {
            self.pending_claim = Some((id.clone(), ClaimIntent::ClearEscalation(escalation)));
        }
        Ok(())
    }
```

In `save`, take the flag once at the top: `let clear_escalation = std::mem::take(&mut ctx.clear_escalation);`, then:

- **Acquire branch** (store before task): after `let previous_park = store.park(&id).cloned();` add
  `let previous_escalation = if clear_escalation { store.remove_escalation(&id) } else { None };`
  and in the rollback after the failed task write, before the `match (previous, previous_park)`, add
  `if let Some(escalation) = previous_escalation { store.insert_escalation(&id, escalation); }`.
- **Release branch** (task before store): after `store.remove(&id);` add
  `if clear_escalation || clear_park { store.remove_escalation(&id); }` and drop the separate `if clear_park` escalation line (keep `remove_park` under `clear_park`).
- **New branch** before `PreserveStore`:

```rust
        Some((id, ClaimIntent::ClearEscalation(escalation))) => {
            ctx.project.write_task(&ctx.registry, task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove_escalation(&id);
            if let Err(error) = store.save() {
                ctx.warnings.push(format!(
                    "{id}'s rating was saved but the escalation to {} could not be cleared \
                     ({error}); rerun `tasks edit {id} --complexity <level>`",
                    escalation.level.as_str()
                ));
            }
            Ok(())
        }
```

The Park branch never sees the flag (`park` does not call `reassess`), and `PreserveStore`/`None` are replaced by `ClearEscalation` whenever the flag is set.

- [ ] **Step 4: Call it from `edit`**

In `src/commands/edit.rs`, after the `if let Some(status) = args.status { ... }` block and before `save(&mut ctx, &mut task)?;`:

```rust
    if args.fields.complexity.is_some() || args.no_complexity {
        ctx.reassess(&task.id)?;
    }
```

- [ ] **Step 5: Run the tests and the suite**

Run: `cargo test --test cli escalation`
Expected: 2 passed.

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Gate and commit**

Run: `just check`

```bash
git add src/commands/mod.rs src/commands/edit.rs tests/cli.rs
git commit -m "feat(edit): an explicit rating clears the escalation; closing clears it too"
```

---

### Task 8: `check` warns on an unrated plan step

**Files:**
- Modify: `src/commands/check.rs` (the per-task loop that has `file` in scope, around line 160)
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: finding kind `unrated_step`, detail `plan step without a complexity rating`, as a warning.

- [ ] **Step 1: Write the failing end-to-end test**

Append to `tests/cli.rs`:

```rust
#[test]
fn check_warns_on_an_open_plan_step_without_a_rating() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    write_doc(&sci, "docs/plans/x.md", "# x\n\n### Task 1: do it\n");
    let step = id_of(env.json(&sci, &["add", "Step", "-p", "2", "--plan", "x", "--step", "Task 1: do it"]));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    let v = env.json(&sci, &["check"]);
    let warnings = v["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|w| w["kind"] == "unrated_step" && w["id"] == step), "{v}");
    assert!(!warnings.iter().any(|w| w["id"] == plain), "missing complexity elsewhere is not a finding");
    env.json(&sci, &["edit", &step, "--complexity", "low"]);
    let v = env.json(&sci, &["check"]);
    assert!(!v["warnings"].as_array().unwrap().iter().any(|w| w["kind"] == "unrated_step"), "{v}");
    env.json(&sci, &["edit", &step, "--no-complexity"]);
    env.json(&sci, &["done", &step, "landed"]);
    let v = env.json(&sci, &["check"]);
    assert!(!v["warnings"].as_array().unwrap().iter().any(|w| w["kind"] == "unrated_step"), "closed steps are silent: {v}");
}
```

(`write_doc` is the helper at `tests/cli.rs:289`; check that `add --plan x` resolves a bare plan name against `docs/plans/` the way `add_resolves_spec_plan_and_step` at line ~1279 does, and copy its plan-file shape if the heading form differs.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli check_warns_on_an_open_plan_step`
Expected: FAIL — no `unrated_step` warning.

- [ ] **Step 3: Add the finding**

In `src/commands/check.rs`, inside the per-task loop where `open_child_of_closed_parent` is pushed (the block that has `file` bound for the task), add:

```rust
        // Spec §6: the plan step is the unit of delegation, and rating it is the
        // planner's duty. Elsewhere a missing rating is not a finding, like size.
        if task.status.is_open() && task.step.is_some() && task.complexity.is_none() {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "unrated_step",
                "plan step without a complexity rating".into(),
            ));
        }
```

- [ ] **Step 4: Run the test and the suite**

Run: `cargo test --test cli check_warns_on_an_open_plan_step`
Expected: PASS.

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Gate and commit**

Run: `just check` — note `tasks check` in this repository will now warn on this plan's own open step tasks, which have no rating yet; warnings do not fail the gate. Task 10 rates them.

```bash
git add src/commands/check.rs tests/cli.rs
git commit -m "feat(check): warn on an open plan step without a complexity rating"
```

---

### Task 9: `rename` carries escalations

**Files:**
- Modify: `src/claims.rs` (`parks_renamed_text` line ~347 → `carried_renamed_text`)
- Modify: `src/rename/inventory.rs` (line ~105)
- Modify: `src/rename/mod.rs` (lines ~61–70, ~118–130, ~232–256)
- Modify: `src/output.rs` (`RenameOut` line ~24)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ClaimStore::{escalations, carries_nothing}` (Task 3).
- Produces: `ClaimStore::carried_renamed_text(&self, source, target) -> Result<String>` (parks and escalations re-keyed, no claims); `RenameOut.escalations: usize`.

- [ ] **Step 1: Write the failing end-to-end tests**

Find the existing rename tests (grep `fn rename_` in `tests/cli.rs`) and read the one that renames with a park entry to copy its setup (registry, no live claims, single worktree). Then append:

```rust
#[test]
fn rename_carries_an_escalation_only_store_and_refuses_one_at_the_target() {
    let mut env = TestEnv::new();
    let dot = env.init("dot");
    let id = id_of(env.json(&dot, &["add", "T", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &dot, "agent-a")
        .args(["park", &id, "stuck", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
    // Resume and finish the session so the store holds an escalation and nothing else.
    as_agent(&env, &dot, "agent-a").args(["start", &id]).assert().success();
    as_agent(&env, &dot, "agent-a").args(["edit", &id, "--status", "todo"]).assert().success();
    let store = std::fs::read_to_string(env.claim_store("dot")).unwrap();
    assert!(store.contains("[escalations.") && !store.contains("[parks."), "{store}");

    let v = env.json(&dot, &["rename", "dot", "dots"]);
    assert_eq!(v["escalations"], 1, "{v}");
    assert_eq!(v["parks"], 0);
    assert!(!env.claim_store("dot").exists());
    let store = std::fs::read_to_string(env.claim_store("dots")).unwrap();
    assert!(store.contains("[escalations.dots-"), "{store}");
    let new_id = id.replace("dot-", "dots-");
    let v = env.json(&dot, &["show", &new_id]);
    assert_eq!(v["escalation"]["level"], "high");
    let v = env.json(&dot, &["ready", "--max-complexity", "mid"]);
    assert!(v["tasks"].as_array().unwrap().is_empty(), "still hidden after the rename: {v}");

    // A target store holding only an escalation is a destination conflict.
    let other = env.init("fam");
    let other_id = id_of(env.json(&other, &["add", "F", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &other, "agent-b")
        .args(["park", &other_id, "stuck", "--reason", "capability", "--complexity", "high"])
        .assert()
        .success();
    as_agent(&env, &other, "agent-b").args(["start", &other_id]).assert().success();
    as_agent(&env, &other, "agent-b").args(["edit", &other_id, "--status", "todo"]).assert().success();
    // Leave `fam` registered but make its store the target of a rename from `dots`.
    env.json(&other, &["unregister", "fam"]);
    let out = env.cmd(&dot).args(["rename", "dots", "fam"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("target store holds"), "{}", String::from_utf8_lossy(&out.stderr));
}
```

Also extend the existing interrupted-rename test that uses `TASKS_RENAME_STOP_AFTER` (grep it in `tests/cli.rs`) with an escalation in the source store before the stop, and assert after the resume that the target store carries it. If that test is table-driven over phases, add the escalation to its fixture rather than duplicating the test.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli rename_carries_an_escalation`
Expected: FAIL — `escalations` missing from the output, or the store not migrated.

- [ ] **Step 3: Generalise the carried set**

In `src/claims.rs`, rename `parks_renamed_text` to `carried_renamed_text` and make it carry both maps:

```rust
    /// The store `rename` writes for the target prefix: this store's park and escalation
    /// entries with their ids re-prefixed, and no claims. Pure; nothing on disk changes.
    pub fn carried_renamed_text(&self, source: &str, target: &str) -> Result<String> {
        let rekey = |key: &String, kind: &str| -> Result<String> {
            let id = TaskId::parse(key)?;
            if id.prefix != source {
                return Err(Error::Config(format!(
                    "{kind} entry {key} does not belong to prefix {source:?}"
                )));
            }
            Ok(format!("{target}-{}", id.hex))
        };
        let mut parks = BTreeMap::new();
        for (key, park) in &self.parks {
            parks.insert(rekey(key, "park")?, park.clone());
        }
        let mut escalations = BTreeMap::new();
        for (key, escalation) in &self.escalations {
            escalations.insert(rekey(key, "escalation")?, escalation.clone());
        }
        Ok(toml::to_string(&StoreFile {
            claims: BTreeMap::new(),
            parks,
            escalations,
        })
        .expect("claim store serializes"))
    }
```

In `src/rename/inventory.rs:105`, replace `store.parks().next().is_none()` with `store.carries_nothing()` and the call with `store.carried_renamed_text(&project.prefix, target)?`. The inventory field keeps its on-disk name `parks_store`: an inventory written by the previous binary must still resume.

In `src/rename/mod.rs`:
- Line ~61: beside `source_parks`, count `source_escalations` the same way with `.escalations().count()`; the "load the target's counts instead" condition becomes `source_parks == 0 && source_escalations == 0 && (...)`, and both counts come from the target in that branch.
- Line ~118: the destination refusal collects `target_parks` from `.parks()` and `target_escalations` from `.escalations()`; refuse when either is non-empty, with the message
  `"target store holds {n} parked task(s) and {m} escalation(s) ({ids}); remove or resume them before renaming"` built from both lists.
- Line ~238: replace `ClaimStore::load_from(&dest)?.parks().next().is_some()` with `!ClaimStore::load_from(&dest)?.carries_nothing()`.
- Line ~252: after `out.parks = ...` add `out.escalations = ClaimStore::load_from(&dest)?.escalations().count();`.

In `src/output.rs` `RenameOut`, after `pub parks: usize,` add `pub escalations: usize,`, and set it wherever `RenameOut` is constructed (grep `RenameOut {`), from the counts computed above.

- [ ] **Step 4: Run the tests and the suite**

Run: `cargo test --test cli rename`
Expected: all rename tests pass, including the new one.

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Gate and commit**

Run: `just check`

```bash
git add src/claims.rs src/rename/inventory.rs src/rename/mod.rs src/output.rs tests/cli.rs
git commit -m "feat(rename): carry escalations beside park entries"
```

---

### Task 10: Docs, skills, the spec's status, and rating this plan's own steps

**Files:**
- Modify: `skills/tasks/SKILL.md`
- Modify: `skills/curate/SKILL.md` (the allowed-edits list, line ~75)
- Modify: `README.md` (the `TASKS_MODEL` paragraph line ~55; the reason table line ~60)
- Modify: `docs/specs/2026-09-11-park-reason-and-stamps-design.md` (§4)
- Modify: `docs/specs/2026-09-08-prefix-rename-design.md` (§5.1 P6, §5.2, §5.3, §5.6)
- Modify: `docs/specs/2026-09-12-task-complexity-design.md` (status line)
- Modify: `tasks/tasks-be447b.md` and this plan's step tasks — through the CLI only

**Interfaces:** none; text only.

- [ ] **Step 1: The tasks skill**

In `skills/tasks/SKILL.md`:

- Session protocol step 2, after the `ready` sentence, add:

  > A session under a cutoff (`TASKS_MAX_COMPLEXITY=<low|mid|high>` set by its harness, or `--max-complexity <level>` on `ready`/`next`) picks only through `ready` and `next`, which hide tasks rated above the level and unassessed tasks and say in warnings how many they hid. Do not take work from `prime`'s parked or roadmap sections or from `list --parked`, and close goals only when `prime`'s closeout offers them. The variable is the harness form; the flag is for a person at a terminal.

- Session protocol step 5 (`park`), extend the reason list with `capability` (the work needs more reasoning than this session can supply), and add after it:

  > Escalate on an observable trigger, not a feeling: the implementation needs a decision the spec or plan leaves unresolved; investigation reveals interacting behaviour outside the assessed scope; a bounded attempt makes no progress or has no way to establish correctness. Record the evidence in a note, then `tasks park <id> "<where it stopped and why>" --reason capability --complexity <level>`: the level must be at least the task's current rating and above your cutoff, and it is written to the record and to the shared store so no checkout's picker offers it under that cutoff again. When no level above the cutoff exists, `--waiting-on user` instead, so a person can decompose or reassign it. An environment or credential failure is `--reason environment` and never raises the rating. If the command reports that the escalation was not recorded, rerun it as it was.

- Recording work, the scoped-task line: `--complexity <low|mid|high>` is already there from Task 2; add a sentence after the bullet:

  > `complexity` is the reasoning and judgment the task demands given its current spec, plan, and context — `low`: the approach is established, the relevant context is identified, and correctness has a clear check; `mid`: bounded investigation or implementation choices remain, scope and acceptance criteria are clear; `high`: substantial discovery, subtle reasoning about interacting behaviour, or an unresolved architectural judgment. Rate it when scoping, next to priority and size; rate ready work first. A precisely specified concurrent algorithm can still be `high`; touching many files does not make a task `high`. `edit --complexity` or `--no-complexity` is an explicit reassessment and clears any escalation.

- "With superpowers", writing-plans bullet: add "and `--complexity <level>` on every step child — a plan is evidence for a lower rating, not a guarantee, and `check` warns on an open step without one."

- [ ] **Step 2: The curate skill and the README**

`skills/curate/SKILL.md` allowed edits, after the `--size` bullet:

```
   - `--complexity`: set or correct against the rubric in the tasks skill (low: approach
     established, context identified, clear check; mid: bounded choices remain, scope
     clear; high: discovery, interacting behaviour, or an open architectural call).
```

`README.md`: after the `TASKS_MODEL` paragraph add:

> `TASKS_MAX_COMPLEXITY` per harness process is the envelope a session picks within: `ready`, `next`, and `prime` hide tasks rated above it and unassessed tasks, and say how many. `--max-complexity` on `ready`/`next` overrides it for one call. Design: `docs/specs/2026-09-12-task-complexity-design.md`.

Add a row to the reason table:

```
| `capability`  | the work needs more reasoning than this session can supply; `--complexity` raises the rating |
```

- [ ] **Step 3: Pointers in the two earlier specs**

`docs/specs/2026-09-11-park-reason-and-stamps-design.md` §4: after the six-word table or list, add one line: "A seventh word, `capability`, is defined in `2026-09-12-task-complexity-design.md` §5; it is the one reason with a companion field."

`docs/specs/2026-09-08-prefix-rename-design.md`: in §5.1 P6, §5.2 (the target-store refusal), §5.3, and §5.6 step 4, change "park entries" to "park and escalation entries" where the store's carried contents are meant, and add to §5.1 a one-line pointer: "Escalations are defined in `2026-09-12-task-complexity-design.md` §5.1 and are carried exactly as parks are."

- [ ] **Step 4: Install, rate the plan's steps, mark the spec**

Run: `cargo install --path .`

Then rate every step task of this plan (they were created unrated because the flag did not exist). Each rating is a judgment about the difficulty that remains after this plan, per the spec's rubric — a plan is evidence, not a guarantee — so they are set one by one, not bulk-labelled. `tasks tree tasks-be447b --pretty` maps titles to ids.

| step | level | why |
|------|-------|-----|
| Task 1 | `low` | code given in full; the round-trip test is the check |
| Task 2 | `low` | flag plumbing after existing patterns; e2e test is the check |
| Task 3 | `low` | a third map mirroring `parks`; unit test is the check |
| Task 4 | `low` | pure functions with pinned strings and unit tests |
| Task 5 | `mid` | three pickers, distinct counting across two candidate paths, `prime` key names to confirm against `PrimeOut` |
| Task 6 | `mid` | level rules with two routes, the intent change through `save`, and a fault-injected recovery test |
| Task 7 | `mid` | rollback ordering in the acquire path; correctness is only visible through the three-checkout and forced-failure tests |
| Task 8 | `low` | one finding in an existing loop |
| Task 9 | `mid` | interrupted-rename recovery and the destination refusal touch inventory state |
| Task 10 | `low` | text, with the rubric supplied |

```bash
tasks edit <task-1-id> --complexity low
tasks edit <task-2-id> --complexity low
tasks edit <task-3-id> --complexity low
tasks edit <task-4-id> --complexity low
tasks edit <task-5-id> --complexity mid
tasks edit <task-6-id> --complexity mid
tasks edit <task-7-id> --complexity mid
tasks edit <task-8-id> --complexity low
tasks edit <task-9-id> --complexity mid
tasks edit <task-10-id> --complexity low
tasks check
```

Steps already `done` by then are closed and need no rating; rate the ones still open (normally only this task).

Change the spec's status line to `Status: implemented (<today>)`.

- [ ] **Step 5: Gate and commit**

Run: `just check && cargo test`
Expected: clean; `tasks check` reports no `unrated_step` warnings.

```bash
git add skills/ README.md docs/specs/ tasks/
git commit -m "docs: complexity rubric, cutoff, and escalation in the skills, README, and specs"
```

---

## Self-review

**Spec coverage.** §3.1 rubric → Task 10 (skill text). §3.2 record, JSON rows, pretty column, not a sort key → Tasks 1, 2, 6 (escalation in rows). §3.3 add/edit/no-complexity, completion, curate, no implicit set → Tasks 2, 10; `park` sets the record only with an explicit `--complexity` (Task 6). §4.1 ready/next/prime incl. closeout, effective rating, composition with `--size`/`--parallel`/`-n`, list/show/tree/sample/start unchanged → Tasks 4, 5. §4.2 flag, variable, precedence, validate-always → Tasks 4, 5. §4.3 warning strings, distinct counts, one total under `--all-projects` → Task 4 (strings), Task 5 (wiring, cross-project test), Task 7 (`--all-projects` through the lifecycle). §5 routes, level rules, `capability` reason, flag-only sessions checked against the effective rating without a cutoff check → Task 6 (`cutoff(None)` reads only the variable). §5.1 entry shape, survives start/park/note/unrelated edit, cleared by edit --complexity/--no-complexity with warning and preserved when that edit's task write fails, cleared by done/dropped, never pruned by readers, carried by rename → Tasks 3, 7, 9. §5.2 write order, exit 1 with the rerun message for capability parks only, ordinary parks warn → Task 6. §6 `unrated_step` → Task 8. §7 docs → Task 10. §8 tests: each bullet has a test in Tasks 2, 5, 6, 7, 8, 9; the interrupted-rename case is folded into the existing stop-after test in Task 9.

**Placeholders.** None: every step names its file and shows its code or exact text.

**Type consistency.** `Complexity::{Low, Mid, High}` / `as_str` / `parse` (Task 1) are what Tasks 2, 4, 6 call. `Escalation { level, at, session }` (Task 3) is what Task 6 constructs, Task 7 clears, Task 9 re-keys, and `complexity::effective` reads via `ClaimSnapshot::escalation` (Task 4). `ClaimIntent::Park { park, escalation }` (Task 6) and `ClaimIntent::ClearEscalation(Escalation)` (Task 7) are both matched in `save`. `list::ready(ctx, size, parallel, limit, max_complexity)` and `list::next(ctx, max_complexity)` (Task 5) match the dispatch there. `ClaimStore::carries_nothing` (Task 3) is what Task 9 uses.
