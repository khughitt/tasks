# Review State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mark the rows that still need a look in every pretty task table with a left-most `*`, backed by a stored `reviewed` flag on ideas and a derived `needs_review` value for leaf todos with no process.

**Architecture:** One new boolean on the task record (`reviewed`, ideas only), enforced in `format::validate_task` and cleared by every status transition. A pure predicate in a new `src/review.rs` derives `needs_review` for summary rows, filter fields and parked rows. Pretty tables prefix the id cell with `* ` or two spaces, reserved once per output through a new `Marks` value that also carries the existing `||` and type decisions. A one-time Python backfill in `tools/` marks ideas scoped under today's note rule, from a frozen snapshot.

**Tech Stack:** Rust (clap, serde, assert_cmd integration tests), Python 3 standard library for the backfill tool.

**Spec:** `docs/specs/2026-10-11-review-state-design.md` (accepted after review round 5). Read it with this plan; section numbers below refer to it.

## Global Constraints

- The stored field is `reviewed`; the derived row value is `needs_review`; the filter is `--needs-review`; the write flags are `--reviewed` and `--no-reviewed` (spec §2, §3.3, §5.2).
- `reviewed: true` is written to frontmatter only when true, as `Value::Raw("true")`, like `parallel` and `lane` (spec §3.1).
- Only an idea carries `reviewed`. The refusal message is exactly `<id> is <status>; only an idea carries reviewed` (spec §3.2).
- The plain-note warning is exactly `<id> stays reviewed; add --no-reviewed to readmit it for scope` (spec §3.3).
- `needs_review` = idea without `reviewed`, or status `todo`/`blocked` with `process` unset, not a lane, and no children. Nothing else (spec §4).
- JSON: `Task` and `TaskSummary` carry `reviewed` always; `TaskSummary` carries `needs_review` always; `ParkedRow` carries both as optional, omitted on an unresolved row (spec §5.1, §5.4).
- The mark is reserved only when at least one row in the output needs review; a fully reviewed output renders exactly as before (spec §5.3).
- Pending proposals stay note-based: `sample.rs::pending_proposal` is unchanged (spec §6).
- No live backfill and no write of `reviewed` on any real project until every host reading synced records runs the new binary (spec §7.0).
- Tests go through `just test-one <args>` and `just test-fast`; never `cargo test` directly (AGENTS.md).

## Review Focus

1. `edit <todo> --status idea --reviewed` in one command: the transition runs first, so the flag lands on the now-idea record and the edit succeeds. Test in Task 2.
2. `edit <todo> --no-reviewed`: clearing on a non-idea is a harmless no-op that succeeds, never a refusal. Test in Task 2.
3. A marked child under an unmarked goal in `tree --pretty`: the `*` stays in the left-most column, before the indent, so it never drifts right with depth. Test in Task 4.
4. Backfill over an idea with no notes at all: not a candidate, untouched. Test in Task 6.
5. Backfill with a corrupt snapshot file: refuses before any edit and names the file. Test in Task 6.

## Tracking

tasks-a9b8d3 is the design task and ends when this plan is reviewed. On acceptance, file one implementation task under the same parent: `--process planned --plan 2026-10-11-review-state`, complexity mid, size m, sourced from tasks-a9b8d3, and run this plan under it. The rollout below is that task's last step.

## Concurrent work

tasks-91f63b (external document references) is in progress in another worktree and changes `src/format.rs` (`validate_doc_shape`) and resolution code. Rebase onto main before Task 1 and again before the final review; expect a textual conflict only near `validate_task`.

---

### Task 1: The `reviewed` record field

**Files:**
- Create: `src/review.rs`
- Modify: `src/main.rs` (or `src/lib.rs`, wherever modules are declared: add `mod review;` beside `mod present;`)
- Modify: `src/model.rs` (the `Task` struct and its test helper `task_with`)
- Modify: `src/format.rs` (`KEYS`, `parse_task`, `serialize_task`, `validate_task`, unit tests)
- Modify: `src/commands/mod.rs` (`transition`)
- Modify: `src/moving/preflight.rs:90-93`, `src/moving/journal.rs:272-275` (tombstone)
- Modify: `src/commands/check.rs` (new `review_status` finding)
- Modify: every `Task { … }` literal: `src/defer.rs`, `src/halt.rs`, `src/periodic.rs`, `src/lanes.rs`, `src/query.rs`, `src/repo.rs`, `src/hierarchy.rs`, `src/complexity.rs`, `src/similarity.rs`, `src/commands/add.rs`
- Test: `src/format.rs` unit tests; create `tests/review.rs`

**Interfaces:**
- Produces: `Task::reviewed: bool`; `crate::review::not_an_idea(task: &Task) -> crate::error::Error`; `check` finding kind `review_status`.

- [ ] **Step 1: Write the failing unit test** in `src/format.rs`'s `mod tests`:

```rust
    #[test]
    fn reviewed_is_written_only_when_true_and_round_trips() {
        let text = "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\nreviewed: true\n\
                    created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                    depends: []\ntags: []\n---\n";
        let task = parse_task(text, "sci-000001.md").unwrap();
        assert!(task.reviewed);
        assert!(serialize_task(&task).contains("\nreviewed: true\n"));
        let mut cleared = task.clone();
        cleared.reviewed = false;
        assert!(!serialize_task(&cleared).contains("reviewed"));
        assert!(!parse_task(&serialize_task(&cleared), "sci-000001.md").unwrap().reviewed);
    }

    #[test]
    fn validate_task_refuses_reviewed_on_a_non_idea() {
        let text = "---\nid: sci-000001\ntitle: T\nstatus: todo\npriority: 2\nreviewed: true\n\
                    created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                    depends: []\ntags: []\n---\n";
        let task = parse_task(text, "sci-000001.md").unwrap();
        let error = validate_task(&task).unwrap_err().to_string();
        assert!(error.contains("sci-000001 is todo; only an idea carries reviewed"), "{error}");
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one reviewed_is_written_only_when_true`
Expected: compile error, `unknown key "reviewed"` or no field `reviewed` on `Task`.

- [ ] **Step 3: Add the field, parsing, serialization and validation**

In `src/model.rs`, after `pub lane: bool,` in `struct Task`:

```rust
    /// A scope pass has reviewed this idea and recorded its verdict. Meaningful only on an
    /// idea: `validate_task` refuses it elsewhere and every status transition clears it.
    /// See docs/specs/2026-10-11-review-state-design.md §3.
    pub reviewed: bool,
```

In `task_with` (model.rs tests) add `reviewed: false,` after `lane: false,`.

Create `src/review.rs`:

```rust
//! Review state: the stored `reviewed` flag on ideas and the derived `needs_review` mark.
//! See docs/specs/2026-10-11-review-state-design.md.

use crate::error::Error;
use crate::model::Task;

/// The refusal for `reviewed` on anything but an idea (spec §3.2).
pub fn not_an_idea(task: &Task) -> Error {
    Error::Validation(format!(
        "{} is {}; only an idea carries reviewed",
        task.id,
        task.status.as_str()
    ))
}
```

Declare the module next to the others (`mod review;`).

In `src/format.rs`: change `const KEYS: [&str; 29]` to `[&str; 30]` and insert `"reviewed",` after `"lane",`. In `parse_task`, after `lane: boolean("lane")?,` add `reviewed: boolean("reviewed")?,`. In `serialize_task`, after the `if t.lane { … }` block:

```rust
    // Raw for the same reason as `parallel`.
    if t.reviewed {
        pairs.push(("reviewed".into(), Value::Raw("true".into())));
    }
```

In `validate_task`, after the `priority` check:

```rust
    if t.reviewed && t.status != Status::Idea {
        return Err(crate::review::not_an_idea(t));
    }
```

Add `reviewed: false,` after `lane: false,` in every `Task { … }` literal listed under Files. `cargo build --all-targets` (run through `just test-one reviewed`) names any you miss.

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `just test-one reviewed_is_written_only_when_true` then `just test-one validate_task_refuses_reviewed`
Expected: PASS.

- [ ] **Step 5: Write the failing integration tests** in a new `tests/review.rs`:

```rust
mod common;
use common::TestEnv;
use std::path::Path;

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

/// Writes `reviewed: true` into a record by hand: reaches states the CLI writes from
/// Task 2 on, or (on a non-idea) never writes at all.
fn seed_reviewed(dir: &Path, id: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let seeded = text.replacen("\npriority: ", "\nreviewed: true\npriority: ", 1);
    assert_ne!(seeded, text, "no priority line in {id}");
    std::fs::write(&path, seeded).unwrap();
}

fn reviewed(env: &TestEnv, dir: &Path, id: &str) -> bool {
    env.json(dir, &["show", id])["task"]["reviewed"].as_bool().unwrap()
}

fn editor_script(dir: &Path, body: &str) -> String {
    let p = dir.join("editor.sh");
    std::fs::write(&p, format!("{body}\n")).unwrap();
    format!("sh '{}'", p.display())
}

#[test]
fn every_status_transition_clears_reviewed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [
        vec!["edit", "ID", "--status", "todo"],
        vec!["shelve", "ID", "later"],
        vec!["drop", "ID"],
    ] {
        let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
        seed_reviewed(&dir, &id);
        assert!(reviewed(&env, &dir, &id));
        let args: Vec<&str> = args.iter().map(|a| if *a == "ID" { id.as_str() } else { a }).collect();
        env.json(&dir, &args);
        assert!(!reviewed(&env, &dir, &id), "{args:?} keeps reviewed");
        assert!(!env.read(&dir, &format!("tasks/{id}.md")).contains("reviewed"));
    }
    // Back to idea arrives unreviewed.
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed_reviewed(&dir, &id);
    env.json(&dir, &["shelve", &id, "later"]);
    env.json(&dir, &["unshelve", &id]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "idea");
    assert!(!reviewed(&env, &dir, &id));
}

#[test]
fn a_moved_idea_leaves_an_unreviewed_tombstone_and_a_reviewed_destination() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    env.init("wali");
    let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
    seed_reviewed(&source, &id);
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    let tombstone = env.json(&source, &["show", &id]);
    assert_eq!(tombstone["task"]["status"], "moved");
    assert_eq!(tombstone["task"]["reviewed"], false);
    assert_eq!(env.json(&source, &["show", &moved])["task"]["reviewed"], true);
}

#[test]
fn an_editor_save_follows_the_idea_only_rule() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let add_line = editor_script(&dir, "sed -i 's/^priority: /reviewed: true\\npriority: /' \"$1\"");
    // Same status, non-idea: refused, file unchanged.
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    let out = env.cmd(&dir).env("EDITOR", &add_line).args(["edit", &todo]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("only an idea carries reviewed"));
    assert_eq!(env.read(&dir, &file), before);
    // Same status, idea: allowed, then removable.
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.cmd(&dir).env("EDITOR", &add_line).args(["edit", &idea]).assert().success();
    assert!(reviewed(&env, &dir, &idea));
    let drop_line = editor_script(&dir, "sed -i '/^reviewed: /d' \"$1\"");
    env.cmd(&dir).env("EDITOR", &drop_line).args(["edit", &idea]).assert().success();
    assert!(!reviewed(&env, &dir, &idea));
    // Status change from a reviewed idea: the transition drops the flag.
    seed_reviewed(&dir, &idea);
    let to_todo = editor_script(&dir, "sed -i 's/^status: idea/status: todo/' \"$1\"");
    env.cmd(&dir).env("EDITOR", &to_todo).args(["edit", &idea]).assert().success();
    assert!(!reviewed(&env, &dir, &idea));
}

#[test]
fn check_reports_reviewed_on_a_non_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    seed_reviewed(&dir, &todo);
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<&str> = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"review_status"), "{report}");
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `just test-one --test review`
Expected: `every_status_transition_clears_reviewed` fails (flag kept after the transition is refused by `validate_task`, or kept on disk); the move test fails in `validate_task` or keeps the flag on the tombstone; `check_reports_reviewed_on_a_non_idea` finds no `review_status`. The editor test may already pass: `validate_task` refuses the same-status save.

- [ ] **Step 7: Clear on transition, on the move tombstone, and report in check**

In `src/commands/mod.rs` `transition`, extend the existing block:

```rust
    if task.status != to {
        // spec §2.2: a deferral is spent by a status change.
        task.defer = None;
        // Review-state spec §3.2: a status change spends a review; a return to idea
        // arrives unreviewed.
        task.reviewed = false;
    }
```

In `src/moving/preflight.rs`, after `tombstone.status = Status::Moved;` add `tombstone.reviewed = false;`. In `src/moving/journal.rs`, after `expected_tombstone.status = Status::Moved;` add `expected_tombstone.reviewed = false;` (the journal's verification must expect what preflight writes).

In `src/commands/check.rs`, immediately before the comment `// A merge or a hand edit can leave a lane inside a lane.`:

```rust
        // Review-state spec §3.2: the writers refuse it, so only a hand edit lands here.
        if task.reviewed && task.status != Status::Idea {
            errors.push(finding(
                Some(task),
                file.clone(),
                "review_status",
                format!(
                    "is {} and carries reviewed; only an idea carries it",
                    task.status.as_str()
                ),
            ));
        }
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one --test review` then `just test-fast`
Expected: all PASS.

- [ ] **Step 9: Commit**

```bash
git add src tests/review.rs
git commit -m "feat(review): store reviewed on ideas and clear it on every status change"
```

---

### Task 2: Writing the flag: `note` and `edit`

**Files:**
- Modify: `src/review.rs` (add `Review` and `apply`)
- Modify: `src/cli.rs` (`Command::Note`, `EditArgs`)
- Modify: `src/commands/mod.rs` (`Command::Note` dispatch)
- Modify: `src/commands/status.rs` (`note`)
- Modify: `src/commands/edit.rs` (flag path)
- Test: `tests/review.rs`; `tests/cli.rs` (stale-copy guard beside the existing `stale_detail` tests near line 12208)

**Interfaces:**
- Consumes: `Task::reviewed`, `crate::review::not_an_idea` (Task 1).
- Produces: `crate::review::Review { Keep, Set, Clear }`, `Review::from_flags(set: bool, clear: bool) -> Review`, `crate::review::apply(task: &mut Task, review: Review, warnings: &mut Vec<String>) -> Result<()>`; CLI `note --reviewed|--no-reviewed`, `edit --reviewed|--no-reviewed`.

- [ ] **Step 1: Write the failing tests** in `tests/review.rs`:

```rust
fn warnings(v: &serde_json::Value) -> Vec<String> {
    v["warnings"]
        .as_array()
        .map(|w| w.iter().map(|s| s.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

#[test]
fn note_sets_and_clears_reviewed_in_the_same_write() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed; x"]);
    let shown = env.json(&dir, &["show", &id]);
    assert_eq!(shown["task"]["reviewed"], true);
    assert_eq!(shown["task"]["notes"][0]["text"], "scope: briefed; x");
    env.json(&dir, &["note", &id, "--no-reviewed", "finding"]);
    assert!(!reviewed(&env, &dir, &id));
    // A second pass sets it again with a second audit note.
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed; y"]);
    let notes = env.json(&dir, &["show", &id])["task"]["notes"].clone();
    let texts: Vec<&str> = notes.as_array().unwrap().iter().map(|n| n["text"].as_str().unwrap()).collect();
    assert_eq!(texts, ["scope: briefed; x", "finding", "scope: briefed; y"]);
    assert!(reviewed(&env, &dir, &id));
}

#[test]
fn a_plain_note_keeps_reviewed_and_warns_once() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let quiet = env.json(&dir, &["note", &id, "before review"]);
    assert!(warnings(&quiet).is_empty(), "{quiet}");
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let out = env.json(&dir, &["note", &id, "curate: tidy"]);
    assert_eq!(
        warnings(&out),
        [format!("{id} stays reviewed; add --no-reviewed to readmit it for scope")]
    );
    assert!(reviewed(&env, &dir, &id));
}

#[test]
fn a_pending_drop_proposal_is_reviewed_and_still_pending_in_sample() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "scope: drop; landed; proposal: drop as covered"]);
    assert!(reviewed(&env, &dir, &id));
    let sample = env.json(&dir, &["sample", "--older-than", "0d"]);
    assert!(
        warnings(&sample).iter().any(|w| w.starts_with(&format!("{id} pending:"))),
        "{sample}"
    );
}

#[test]
fn setting_reviewed_on_a_non_idea_is_refused_and_writes_nothing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    for args in [
        vec!["note", todo.as_str(), "--reviewed", "x"],
        vec!["edit", todo.as_str(), "--reviewed"],
        vec!["edit", todo.as_str(), "--status", "todo", "--reviewed"],
    ] {
        assert_eq!(env.fail(&dir, &args), "validation", "{args:?}");
        assert_eq!(env.read(&dir, &file), before, "{args:?}");
    }
    // Clearing on a non-idea is a harmless no-op (Review Focus 2).
    env.json(&dir, &["edit", &todo, "--no-reviewed"]);
    assert!(!reviewed(&env, &dir, &todo));
    // An idea promoted in the same edit refuses the flag; a todo demoted to idea takes it
    // (Review Focus 1).
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    assert_eq!(env.fail(&dir, &["edit", &idea, "--status", "todo", "--reviewed"]), "validation");
    env.json(&dir, &["edit", &todo, "--status", "idea", "--reviewed"]);
    assert!(reviewed(&env, &dir, &todo));
    env.json(&dir, &["edit", &todo, "--no-reviewed"]);
    assert!(!reviewed(&env, &dir, &todo));
}

#[test]
fn the_flag_forms_conflict() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.usage(&dir, &["note", &id, "--reviewed", "--no-reviewed", "x"]);
    env.usage(&dir, &["edit", &id, "--reviewed", "--no-reviewed"]);
}

#[test]
fn reviewed_writes_follow_the_plain_writes_under_a_foreign_live_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let plain = id_of(env.json(&dir, &["add", "P", "--status", "idea"]));
    let flagged = id_of(env.json(&dir, &["add", "F", "--status", "idea"]));
    write_claim(&env, "sci", &plain, "someone-else");
    write_claim(&env, "sci", &flagged, "someone-else");
    let a = env.cmd(&dir).args(["note", &plain, "x"]).output().unwrap();
    let b = env.cmd(&dir).args(["note", &flagged, "--reviewed", "x"]).output().unwrap();
    assert_eq!(a.status.code(), b.status.code());
    assert_eq!(reviewed(&env, &dir, &flagged), b.status.success());
    let c = env.cmd(&dir).args(["edit", &plain, "-p", "1"]).output().unwrap();
    let d = env.cmd(&dir).args(["edit", &flagged, "--no-reviewed"]).output().unwrap();
    assert_eq!(c.status.code(), d.status.code());
}

/// A live claim held by another session, written straight into the store.
fn write_claim(env: &TestEnv, prefix: &str, id: &str, session: &str) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let rest = stat.rsplit_once(") ").unwrap().1.to_string();
    let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {}\n\
         pid_start = {start}\nboot_id = \"{}\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n",
        std::process::id(),
        boot.trim()
    ));
    std::fs::write(&path, text).unwrap();
}
```

In `tests/cli.rs`, directly after the test that loops `stale_detail(&env, &main, &args)` over `note`, `edit`, `start`, `done` (near line 12208), add:

```rust
#[test]
fn reviewed_writes_from_a_stale_copy_refuse_like_their_plain_forms() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let file = format!("tasks/{id}.md");
    let before = env.read(&main, &file);
    for args in [
        vec!["note", id.as_str(), "--reviewed", "x"],
        vec!["note", id.as_str(), "--no-reviewed", "x"],
        vec!["edit", id.as_str(), "--reviewed"],
        vec!["edit", id.as_str(), "--no-reviewed"],
    ] {
        stale_detail(&env, &main, &args);
        assert_eq!(env.read(&main, &file), before, "{args:?}");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test review note_sets_and_clears` and `just test-one reviewed_writes_from_a_stale_copy`
Expected: usage errors (exit 2): `unexpected argument '--reviewed'`.

- [ ] **Step 3: Add `Review` and `apply`** to `src/review.rs`:

```rust
use crate::error::Result;
use crate::model::Status;

/// What a write does to the `reviewed` flag (spec §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    Keep,
    Set,
    Clear,
}

impl Review {
    /// clap's `conflicts_with` keeps both from arriving together.
    pub fn from_flags(set: bool, clear: bool) -> Review {
        match (set, clear) {
            (true, _) => Review::Set,
            (_, true) => Review::Clear,
            _ => Review::Keep,
        }
    }
}

/// Applies `review` before anything is written, so a refusal leaves the file untouched.
/// `Keep` on a reviewed idea warns: the write does not readmit it.
pub fn apply(task: &mut Task, review: Review, warnings: &mut Vec<String>) -> Result<()> {
    match review {
        Review::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            task.reviewed = true;
        }
        Review::Clear => task.reviewed = false,
        Review::Keep => {
            if task.reviewed {
                warnings.push(format!(
                    "{} stays reviewed; add --no-reviewed to readmit it for scope",
                    task.id
                ));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Wire `note`.** In `src/cli.rs`, `Command::Note` gains, after `stamp: bool,`:

```rust
        /// Mark the idea reviewed in the same write: a scope pass's verdict note.
        #[arg(long, conflicts_with = "no_reviewed")]
        reviewed: bool,
        /// Readmit the idea for review in the same write: a finding that wakes it.
        #[arg(long)]
        no_reviewed: bool,
```

In `src/commands/mod.rs`:

```rust
        Command::Note {
            id,
            text,
            stamp,
            reviewed,
            no_reviewed,
        } => status::note(
            open_id_write_ctx(dir, &id)?,
            id,
            text,
            stamp,
            crate::review::Review::from_flags(reviewed, no_reviewed),
        ),
```

In `src/commands/status.rs`, `note` takes `review: crate::review::Review` as its last parameter and applies it first:

```rust
pub fn note(
    mut ctx: Ctx,
    id: String,
    text: String,
    stamp: bool,
    review: crate::review::Review,
) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    crate::review::apply(&mut task, review, &mut ctx.warnings)?;
    let owner = owner_name(&ctx.project)?;
```

(the rest of the function is unchanged). Update any other caller of `status::note` the compiler names, passing `crate::review::Review::Keep`.

- [ ] **Step 5: Wire `edit`.** In `src/cli.rs` `EditArgs`, after `no_lane`:

```rust
    /// Mark this idea reviewed: a scope pass has recorded its verdict. Ideas only.
    #[arg(long, conflicts_with = "no_reviewed")]
    pub reviewed: bool,
    /// Readmit this idea for review.
    #[arg(long)]
    pub no_reviewed: bool,
```

In `src/commands/edit.rs`, add `|| args.reviewed || args.no_reviewed` to `has_flags`, and immediately after the `if let Some(status) = args.status { … }` block (so a status change lands first):

```rust
    // Review-state spec §3.3: after any status change, so `--status idea --reviewed`
    // sets it and `--status todo --reviewed` is refused.
    if args.reviewed || args.no_reviewed {
        crate::review::apply(
            &mut task,
            crate::review::Review::from_flags(args.reviewed, args.no_reviewed),
            &mut ctx.warnings,
        )?;
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `just test-one --test review` then `just test-one reviewed_writes_from_a_stale_copy` then `just test-fast`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src tests
git commit -m "feat(review): set and clear reviewed with note and edit"
```

---

### Task 3: Derived `needs_review`, JSON and `--needs-review`

**Files:**
- Modify: `src/review.rs` (add `needs_review`)
- Modify: `src/output.rs` (`TaskSummary`, `TaskSummary::of`, `ParkedRow`, `ParkedRow::resolved`, `ParkedRow::unresolved`, test literals `row`, `recurring`, `long_row`, `parked_with`)
- Modify: `src/filter.rs` (`TaskFilter`, `Fields`, `Fields::of_task`, `Fields::of_row`, tests' `fields()`)
- Modify: `src/cli.rs` (`FilterArgs`)
- Modify: `src/commands/list.rs:68,217`, `src/commands/parked.rs:235` (`Fields::of_task` callers)
- Test: `src/review.rs` unit tests; `tests/review.rs`

**Interfaces:**
- Consumes: `Task::reviewed` (Task 1).
- Produces: `crate::review::needs_review(status: Status, reviewed: bool, process: Option<Process>, lane: bool, child_count: usize) -> bool`; `TaskSummary::{reviewed: bool, needs_review: bool}`; `ParkedRow::{reviewed: Option<bool>, needs_review: Option<bool>}`; `Fields::needs_review: bool`; `Fields::of_task(task, all, claims, registry)`; `FilterArgs::needs_review: bool`.

- [ ] **Step 1: Write the failing unit test** in `src/review.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Process;

    #[test]
    fn needs_review_follows_spec_section_4() {
        use Status::*;
        assert!(needs_review(Idea, false, None, false, 0));
        assert!(!needs_review(Idea, true, None, false, 0));
        assert!(needs_review(Todo, false, None, false, 0));
        assert!(needs_review(Blocked, false, None, false, 0));
        assert!(!needs_review(Todo, false, Some(Process::Direct), false, 0));
        assert!(!needs_review(Todo, false, None, false, 2), "a goal");
        assert!(!needs_review(Todo, false, None, true, 0), "a lane");
        for status in [Doing, Shelved, Done, Dropped, Moved] {
            assert!(!needs_review(status, false, None, false, 0), "{status:?}");
        }
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `just test-one needs_review_follows_spec`
Expected: compile error, `cannot find function needs_review`.

- [ ] **Step 3: Implement the predicate** in `src/review.rs`:

```rust
use crate::model::Process;

/// Whether a row needs a look (spec §4): an idea no scope pass has reviewed, or a leaf
/// todo or blocked task whose process is unassessed. Goals, lanes and everything in hand
/// or out of sight never do.
pub fn needs_review(
    status: Status,
    reviewed: bool,
    process: Option<Process>,
    lane: bool,
    child_count: usize,
) -> bool {
    match status {
        Status::Idea => !reviewed,
        Status::Todo | Status::Blocked => process.is_none() && !lane && child_count == 0,
        _ => false,
    }
}
```

Run: `just test-one needs_review_follows_spec` — Expected: PASS.

- [ ] **Step 4: Write the failing integration tests** in `tests/review.rs`:

```rust
fn rows(v: &serde_json::Value) -> Vec<(String, bool)> {
    v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["id"].as_str().unwrap().to_string(), r["needs_review"].as_bool().unwrap()))
        .collect()
}

/// One of each kind spec §4 distinguishes, returned as (marked ids, unmarked ids).
fn mixed_project(env: &mut TestEnv) -> (std::path::PathBuf, Vec<String>, Vec<String>) {
    let dir = env.init("sci");
    let new_idea = id_of(env.json(&dir, &["add", "New", "--status", "idea"]));
    let briefed = id_of(env.json(&dir, &["add", "Briefed", "--status", "idea"]));
    env.json(&dir, &["note", &briefed, "--reviewed", "scope: briefed"]);
    let raw_todo = id_of(env.json(&dir, &["add", "Raw"]));
    let assessed = id_of(env.json(&dir, &["add", "Assessed", "--process", "direct"]));
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &goal, "--process", "direct"]));
    let blocked = id_of(env.json(&dir, &["add", "Blocked"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    let doing = id_of(env.json(&dir, &["add", "Doing"]));
    env.json(&dir, &["start", &doing]);
    (
        dir,
        vec![new_idea, raw_todo, blocked],
        vec![briefed, assessed, goal, child, doing],
    )
}

#[test]
fn needs_review_marks_exactly_spec_section_4_in_json_and_the_filter() {
    let mut env = TestEnv::new();
    let (dir, marked, unmarked) = mixed_project(&mut env);
    let listed = rows(&env.json(&dir, &["list"]));
    for id in &marked {
        assert!(listed.contains(&(id.clone(), true)), "{id} should need review: {listed:?}");
    }
    for id in &unmarked {
        assert!(listed.contains(&(id.clone(), false)), "{id} should not: {listed:?}");
    }
    let filtered: Vec<String> = rows(&env.json(&dir, &["list", "--needs-review"]))
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let mut want = marked.clone();
    want.sort();
    let mut got = filtered.clone();
    got.sort();
    assert_eq!(got, want);
    // `ready` never shows ideas, so its filter narrows to the unassessed todo.
    let ready: Vec<String> = rows(&env.json(&dir, &["ready", "--needs-review"]))
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ready, [marked[1].clone()]);
    // `show` carries the stored flag.
    assert_eq!(env.json(&dir, &["show", &unmarked[0]])["task"]["reviewed"], true);
}

/// A park entry written straight into the store, for a task or for an id with no record.
fn write_park(env: &TestEnv, prefix: &str, id: &str) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[parks.\"{id}\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"next\"\n\
         waiting_on = \"user\"\ntitle = \"T\"\n"
    ));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn parked_rows_carry_needs_review_and_filter_on_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let unreviewed = id_of(env.json(&dir, &["add", "U", "--status", "idea"]));
    let reviewed_idea = id_of(env.json(&dir, &["add", "R", "--status", "idea"]));
    env.json(&dir, &["note", &reviewed_idea, "--reviewed", "scope: briefed"]);
    write_park(&env, "sci", &unreviewed);
    write_park(&env, "sci", &reviewed_idea);
    write_park(&env, "sci", "sci-abcdef");
    let parked = env.json(&dir, &["list", "--parked"]);
    let by_id = |id: &str| {
        parked["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} missing: {parked}"))
    };
    assert_eq!(by_id(&unreviewed)["needs_review"], true);
    assert_eq!(by_id(&reviewed_idea)["needs_review"], false);
    assert!(by_id("sci-abcdef").get("needs_review").is_none());
    let filtered: Vec<String> = env.json(&dir, &["list", "--parked", "--needs-review"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(filtered, [unreviewed]);
}
```

If `block` takes no reason argument or `--process` is not accepted by `add`, adjust the call to the CLI's actual form (`tasks block --help`, `tasks add --help`); keep the states the same.

- [ ] **Step 5: Run them to verify they fail**

Run: `just test-one --test review needs_review_marks` and `just test-one --test review parked_rows_carry`
Expected: FAIL (`needs_review` missing from rows; `--needs-review` is an unexpected argument).

- [ ] **Step 6: Add the summary and parked fields.** In `src/output.rs` `struct TaskSummary`, after `pub lane: bool,`:

```rust
    /// The stored review flag; always present, like `parallel`.
    pub reviewed: bool,
    /// The row needs a look (review-state spec §4). Computed from the scan, never stored.
    pub needs_review: bool,
```

In `TaskSummary::of`, compute the child count once and use it twice:

```rust
        let child_count = crate::hierarchy::children(all, &task.id, registry).len();
        TaskSummary {
            // … existing fields …
            lane: task.lane,
            reviewed: task.reviewed,
            needs_review: crate::review::needs_review(
                task.status,
                task.reviewed,
                task.process,
                task.lane,
                child_count,
            ),
            // … existing fields …
            child_count,
```

In `struct ParkedRow`, after `pub lane: bool,`:

```rust
    /// From the record; omitted on an unresolved row (review-state spec §5.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_review: Option<bool>,
```

In `ParkedRow::resolved`: `reviewed: Some(summary.reviewed), needs_review: Some(summary.needs_review),`. In `ParkedRow::unresolved`: `reviewed: None, needs_review: None,`. Add `reviewed: false, needs_review: false,` to the `TaskSummary` literals in the `output.rs` tests (`row`, `recurring`, `long_row`) and `reviewed: None, needs_review: None,` (or `Some(false)`) to any `ParkedRow` literal the compiler names.

- [ ] **Step 7: Add the filter.** In `src/cli.rs` `FilterArgs`, after `parallel`:

```rust
    /// Only rows that need review: ideas no scope pass has reviewed, and leaf todo or
    /// blocked tasks with no process.
    #[arg(long)]
    pub needs_review: bool,
```

In `src/filter.rs`: `TaskFilter` gains `needs_review: bool`, set in `parse` from `args.needs_review`; `is_empty` adds `&& !self.needs_review`; `matches` adds `&& (!self.needs_review || fields.needs_review)`. `Fields` gains `pub needs_review: bool`. `Fields::of_task` becomes:

```rust
    pub fn of_task(
        task: &'a Task,
        all: &[Task],
        claims: &ClaimSnapshot,
        registry: &Registry,
    ) -> Fields<'a> {
        Fields {
            // … existing fields …
            parallel: task.parallel,
            needs_review: crate::review::needs_review(
                task.status,
                task.reviewed,
                task.process,
                task.lane,
                crate::hierarchy::children(all, &task.id, registry).len(),
            ),
        }
    }
```

`Fields::of_row` adds `needs_review: row.needs_review?,`. Update the three callers to pass the scan they already hold: `src/commands/list.rs:68` → `Fields::of_task(task, &all, &claims, &ctx.registry)`; `src/commands/list.rs:217` → `Fields::of_task(task, all, claims, &ctx.registry)`; `src/commands/parked.rs:235` → `Fields::of_task(task, all, claims, &ctx.registry)`. Add `needs_review: false,` to the `fields()` helper in `filter.rs`'s tests.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one --test review` then `just test-fast`
Expected: all PASS.

- [ ] **Step 9: Commit**

```bash
git add src tests/review.rs
git commit -m "feat(review): derive needs_review on rows and filter with --needs-review"
```

---

### Task 4: The `*` mark in pretty tables and `show`

**Files:**
- Modify: `src/present.rs` (add `review_mark`)
- Modify: `src/output.rs` (`Marks`, `any_needs_review_tree`, `task_columns`, `task_row`, `tree_rows`, `task_table`, `Output::Prime`, `Output::Tree`, `parked_row`, `parked_table`, `lane_table`, their test callers)
- Modify: `src/show_text.rs` (overview row)
- Test: `src/output.rs` unit tests; `tests/review.rs`

**Interfaces:**
- Consumes: `TaskSummary::needs_review`, `ParkedRow::needs_review` (Task 3).
- Produces: `present::review_mark(needs_review: bool, reserved: bool) -> Span`; `output::Marks { review, parallel, kind }` with `Marks::of(&[TaskSummary])`, `Marks::of_tree(&[TreeNode])`, `Marks::or(self, Marks)`; `task_columns(marks: Marks)`; `task_row(row, date, painter, marks: Marks, indent)`; `tree_rows(nodes, depth, painter, marks: Marks, rows)`; `parked_row(row, painter, review: bool)`; `parked_table(rows, painter, layout, widths, review: bool)`; `lane_table(rows, painter, layout, widths, detail, pad: bool)`.

The mark is a two-character prefix of the id cell, not a separate column: `prime` shares the id width across sections by treating column 0 as the id (`shared.0[0]`, `Widths::id_only`), and `tree` already indents inside the id cell. Update spec §5.3 and §5.4 to say so in this task's commit (Step 9).

- [ ] **Step 1: Write the failing tests** in `tests/review.rs`:

```rust
/// The first two characters of each pretty row that names `id`.
fn prefix_of(pretty: &str, id: &str) -> String {
    let line = pretty
        .lines()
        .find(|l| l.contains(id))
        .unwrap_or_else(|| panic!("{id} not in:\n{pretty}"));
    line.chars().take(2).collect()
}

#[test]
fn pretty_list_marks_exactly_the_rows_json_marks() {
    let mut env = TestEnv::new();
    let (dir, marked, unmarked) = mixed_project(&mut env);
    let pretty = env.pretty(&dir, &["list"]);
    for id in &marked {
        assert_eq!(prefix_of(&pretty, id), "* ", "{id}:\n{pretty}");
    }
    for id in &unmarked {
        assert_eq!(prefix_of(&pretty, id), "  ", "{id}:\n{pretty}");
    }
    let ready = env.pretty(&dir, &["ready"]);
    assert_eq!(prefix_of(&ready, &marked[1]), "* ");
    let tree = env.pretty(&dir, &["tree"]);
    assert_eq!(prefix_of(&tree, &marked[0]), "* ");
}

#[test]
fn a_fully_reviewed_listing_renders_without_the_prefix() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A", "--process", "direct"]));
    let pretty = env.pretty(&dir, &["list"]);
    assert!(pretty.starts_with(&id), "{pretty}");
}

#[test]
fn the_mark_stays_left_of_a_tree_indent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &goal]));
    let tree = env.pretty(&dir, &["tree"]);
    let line = tree.lines().find(|l| l.contains(&child)).unwrap();
    assert!(line.starts_with(&format!("*   {child}")), "{tree}");
    assert!(tree.lines().find(|l| l.contains(&goal)).unwrap().starts_with(&format!("  {goal}")));
}

#[test]
fn parked_views_mark_the_same_rows_as_their_json() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let unreviewed = id_of(env.json(&dir, &["add", "U", "--status", "idea"]));
    let reviewed_idea = id_of(env.json(&dir, &["add", "R", "--status", "idea"]));
    env.json(&dir, &["note", &reviewed_idea, "--reviewed", "scope: briefed"]);
    write_park(&env, "sci", &unreviewed);
    write_park(&env, "sci", &reviewed_idea);
    write_park(&env, "sci", "sci-abcdef");
    let parked = env.pretty(&dir, &["list", "--parked"]);
    assert_eq!(prefix_of(&parked, &unreviewed), "* ");
    assert_eq!(prefix_of(&parked, &reviewed_idea), "  ");
    assert_eq!(prefix_of(&parked, "sci-abcdef"), "  ");
    let prime = env.pretty(&dir, &["prime"]);
    let section = prime.split("parked:").nth(1).unwrap();
    assert_eq!(prefix_of(section, &unreviewed), "* ");
}

#[test]
fn show_names_a_reviewed_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    assert!(!env.pretty(&dir, &["show", &id]).contains("reviewed"));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let shown = env.pretty(&dir, &["show", &id]);
    assert!(shown.lines().any(|l| l.starts_with("reviewed") && l.trim_end().ends_with("yes")), "{shown}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test review pretty_list_marks`
Expected: FAIL, rows begin with the id.

- [ ] **Step 3: Add `review_mark`** to `src/present.rs` after `row_id`:

```rust
/// The review prefix of an id cell (review-state spec §5.3): `* ` on a row that needs
/// review, two spaces on any other row of an output that reserves it, nothing otherwise.
pub fn review_mark(needs_review: bool, reserved: bool) -> Span {
    let text = match (reserved, needs_review) {
        (false, _) => "",
        (true, true) => "* ",
        (true, false) => "  ",
    };
    Span {
        text: text.into(),
        style: needs_review.then_some(Style::Emphasis),
    }
}
```

- [ ] **Step 4: Replace the `parallel`/`kind` booleans with `Marks`** in `src/output.rs`. Add beside `any_parallel`:

```rust
/// The optional marks a pretty task table reserves, decided once per command output so a
/// per-section decision cannot shift columns between adjacent sections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// The `* ` prefix on the id cell (review-state spec §5.3).
    pub review: bool,
    /// The `||` column.
    pub parallel: bool,
    /// The type column.
    pub kind: bool,
}

impl Marks {
    pub fn of(rows: &[TaskSummary]) -> Marks {
        Marks {
            review: rows.iter().any(|row| row.needs_review),
            parallel: any_parallel(rows),
            kind: any_type(rows),
        }
    }

    pub fn of_tree(nodes: &[TreeNode]) -> Marks {
        Marks {
            review: any_needs_review_tree(nodes),
            parallel: any_parallel_tree(nodes),
            kind: any_type_tree(nodes),
        }
    }

    pub fn or(self, other: Marks) -> Marks {
        Marks {
            review: self.review || other.review,
            parallel: self.parallel || other.parallel,
            kind: self.kind || other.kind,
        }
    }
}

pub fn any_needs_review_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.needs_review || any_needs_review_tree(&node.children))
}
```

Change `task_columns(parallel: bool, kind: bool)` to `task_columns(marks: Marks)` (body: `if marks.parallel { … }`, `if marks.kind { … }`). Change `task_row`'s `parallel: bool, kind: bool` parameters to `marks: Marks`, use `marks.parallel` / `marks.kind` in its body, and build the id cell as:

```rust
        format!(
            "{}{}{}",
            present::review_mark(row.needs_review, marks.review).paint(painter),
            "  ".repeat(indent),
            present::row_id(&row.id, row.project_color).paint(painter)
        ),
```

Change `tree_rows`' `parallel: bool, kind: bool` to `marks: Marks` and pass `marks` through. In `task_table`: `let marks = Marks::of(rows);` and use it for both `task_row` and `task_columns`. In `Output::Tree`: `let marks = Marks::of_tree(&o.nodes);`. Update the test callers at the old `task_columns(false, false)` / `tree_rows(…, false, false, …)` sites to `Marks::default()`, and the two test helpers that compute `parallel`/`kind` to `Marks::of` / `Marks::of_tree`.

- [ ] **Step 5: Parked and lane tables.** `parked_row(row: &ParkedRow, painter: &Painter, review: bool)` builds its first cell as:

```rust
        format!(
            "{}{}",
            present::review_mark(row.needs_review == Some(true), review).paint(painter),
            present::row_id(&row.id, row.project_color).paint(painter)
        ),
```

`parked_table` gains a final `review: bool` parameter and passes it to `parked_row`. `Output::Parked` calls it with `o.tasks.iter().any(|row| row.needs_review == Some(true))`; the `output.rs` tests pass `false`. `lane_table` gains a final `pad: bool` and prefixes its id cell with `"  "` when set; `Output::Lanes` passes `false`.

- [ ] **Step 6: `prime`.** Replace the `parallel`/`kind` computation with:

```rust
            let mut marks = Marks::of(&o.closeout)
                .or(Marks::of_tree(&o.roadmap))
                .or(Marks::of(&o.ready))
                .or(Marks::of(&o.doing));
            marks.review |= o.parked.iter().any(|row| row.needs_review == Some(true));
            let columns = task_columns(marks);
```

pass `marks` to every `task_row` and `tree_rows` call in the arm, and widen the shared id width by the prefix for the sections that measure raw ids:

```rust
            let pad = if marks.review { 2 } else { 0 };
            let id = shared
                .id()
                .unwrap_or(0)
                .max(o.parked.iter().map(|row| row.id.len() + pad).max().unwrap_or(0))
                .max(o.lanes.iter().map(|row| row.lane.id.len() + pad).max().unwrap_or(0));
```

Call `lane_table(…, false, marks.review)` and `parked_table(…, marks.review)` in the prime arm.

- [ ] **Step 7: `show`.** In `src/show_text.rs`, after the `if t.parallel { … }` row:

```rust
    if t.reviewed {
        rows.push(("reviewed", vec![Span::plain("yes")]));
    }
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one --test review` then `just test-fast`
Expected: all PASS, including the existing pretty-table snapshots in `tests/cli.rs` (their fixtures carry no unreviewed rows only if they use assessed todos; where an existing snapshot now gains the prefix because its fixture holds an idea or a raw todo, update the expected text to include the `* ` / two-space prefix and say so in the commit message).

- [ ] **Step 9: Align the spec and commit.** In `docs/specs/2026-10-11-review-state-design.md` §5.3, replace "a one-character column ahead of `id`, holding `*` for a row that needs review and a space otherwise" with "a two-character prefix of the id cell, `* ` for a row that needs review and two spaces otherwise", replace "The header is `*`." with "The id column keeps its header; `prime` shares the id width across its sections through column 0 and `tree` already indents inside the id cell, so a prefix keeps both intact.", and in §5.4 replace "`parked_columns` gains the same leading one-character column" with "`parked_row` gains the same id-cell prefix". Remove "the lanes view" from §5.3's list of `task_columns` callers (it renders lane rows through `lane_table`, which only pads for alignment in `prime`).

```bash
git add src tests docs/specs/2026-10-11-review-state-design.md
git commit -m "feat(review): mark rows needing review in pretty tables and show"
```

---

### Task 5: Skills, README and spec status

**Files:**
- Modify: `skills/scope/SKILL.md`
- Modify: `skills/curate/SKILL.md`
- Modify: `skills/tasks/SKILL.md`
- Modify: `README.md`
- Modify: `docs/specs/2026-10-11-review-state-design.md` (Status line)

**Interfaces:**
- Consumes: the CLI surface from Tasks 2–4.

- [ ] **Step 1: scope skill.** In `skills/scope/SKILL.md` §1, replace the sentences from "List rows omit notes." through "A later note readmits it." with:

```markdown
Select with `tasks list --status idea --needs-review [--tag <tag>] [--project <prefix>]`:
it returns exactly the ideas no scope pass has reviewed, without reading notes. An idea
leaves that list when a pass writes its audit note with `--reviewed`, and returns only
when a waking finding is noted with `--no-reviewed` or the idea comes back to `idea`
from another status. Incidental notes (curate, comments) do not readmit it.
```

Keep the remainder of the paragraph ("Existing briefs, goals, or research do not by themselves exclude…", the explicit-id bypass, the three-document ceiling) and change "whose latest note readmits it" to "that `--needs-review` returns".

In §3, replace the audit-note paragraph with:

```markdown
Every successfully processed member receives exactly one audit note. For `briefed`,
`question` and `drop` it sets the flag in the same write:

    tasks -C <root> note <id> --reviewed "scope: <verdict>; <what changed>[; brief: <path>][; proposal: <text>]"

`scoped` and `shelved` change the status first, which clears the flag; their audit note
is a plain `tasks note` (`--reviewed` is refused on a non-idea).
```

In §5's research template, replace the `Ideas it wakes:` line with:

```markdown
Ideas it wakes: On completion, run `tasks note <id> --no-reviewed "<finding>"` on each named waiting idea, before this result's branch merges.
```

In §7, change "its completion must add a finding note to every waiting idea" to "its completion must add a finding note with `--no-reviewed` to every waiting idea".

- [ ] **Step 2: curate skill.** In `skills/curate/SKILL.md`, after the sentence about `sample` and `curate:`/`scope:` proposals (line ~101), add:

```markdown
A curate note does not readmit an idea for scope: only `tasks note <id> --no-reviewed`
does, so write that form only when the note is new evidence that warrants another
scope pass.
```

- [ ] **Step 3: tasks skill.** In `skills/tasks/SKILL.md`, in the list/ready filters paragraph (the one listing `--parallel` as a switch), add:

```markdown
  `--needs-review` keeps rows that still need a look: ideas no scope pass has reviewed
  (`reviewed` unset) and leaf todo or blocked tasks with no process. Pretty tables mark
  those rows with a leading `*`.
```

- [ ] **Step 4: README.** After the paragraph ending "update every host reading synced tasks before the first move.", add:

```markdown
Ideas carry an optional `reviewed` flag (set by `note --reviewed` and `edit --reviewed`,
cleared by `--no-reviewed` and by every status change), and summary rows carry a derived
`needs_review`; `--needs-review` filters on it and pretty tables mark it with `*`. Older
binaries cannot parse records carrying `reviewed`: update every host reading synced
tasks before the first review write.
```

- [ ] **Step 5: Spec status and commit rule.** Change `Status: accepted (plan pending)` to `Status: implemented (plan docs/plans/2026-10-11-review-state.md); rollout pending`. In §7.2 replace "Each project's writes land in one `chore(tasks)` commit in that project." with "A project without `[git] commit` gets one `chore(tasks)` commit of the run's writes; a project with `[git] commit = true` commits each edit as the CLI does (record-commit design §3.2)." After §7.2's live-claim bullet, add: "`tools/review-backfill` also refuses a whole pass while this host's claim store holds a live claim in the project, as the pause check requires; the next pass after the claim is released applies the untouched entries."

- [ ] **Step 6: Verify and commit**

Run: `just check`
Expected: PASS (hygiene, fmt, clippy, `tasks check`).

```bash
git add skills README.md docs/specs/2026-10-11-review-state-design.md
git commit -m "docs(review): teach scope, curate and tasks the review flag"
```

---

### Task 6: The backfill tool

**Files:**
- Create: `tools/review-backfill` (Python 3, standard library only, executable)
- Create: `tests/review_backfill.rs`

**Interfaces:**
- Consumes: the CLI from Tasks 1–3 (`show`, `list --status idea`, `edit --reviewed`, `claims`, `prime`), all as JSON.
- Produces: `tools/review-backfill --state DIR [--root ROOT] [--tasks BIN] (--freeze-only | --writers-paused)`. Snapshot file `DIR/<prefix>.json`:
  `{"prefix": str, "frozen_at": str, "entries": [{"id": str, "updated": str, "done": bool, "outcome": str|null}]}`. Report on stdout: `{"prefix", "frozen": bool, "set": [id], "changed": [{"id", "detail"}], "failed": [{"id", "detail"}], "remaining": int}`. Exit 0 when nothing failed, 1 when an edit failed or the pause check refused, 2 on usage or a corrupt snapshot.

- [ ] **Step 1: Write the failing tests** in `tests/review_backfill.rs`:

```rust
mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn tool() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/review-backfill")
}

/// Runs the tool against `dir` with the suite's isolated HOME and the built binary.
fn backfill(env: &TestEnv, dir: &Path, state: &Path, extra: &[&str]) -> (i32, serde_json::Value) {
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(tool())
        .args(["--state", state.to_str().unwrap(), "--root", dir.to_str().unwrap()])
        .args(["--tasks", assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap()])
        .args(extra)
        .env("HOME", env.home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_FORMAT")
        .env_remove("TASKS_SESSION")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .output()
        .unwrap();
    let code = out.status.code().unwrap();
    let report = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    if report.is_null() {
        eprintln!("stderr: {}", String::from_utf8_lossy(&out.stderr));
    }
    (code, report)
}

fn reviewed(env: &TestEnv, dir: &Path, id: &str) -> bool {
    env.json(dir, &["show", id])["task"]["reviewed"].as_bool().unwrap()
}

fn updated(env: &TestEnv, dir: &Path, id: &str) -> String {
    env.json(dir, &["show", id])["task"]["updated"].as_str().unwrap().to_string()
}

/// Ideas in each state the eligibility rule distinguishes.
struct Fixture {
    dir: PathBuf,
    briefed: String,
    proposed: String,
    readmitted: String,
    untouched: String,
    no_notes: String,
}

fn fixture(env: &mut TestEnv) -> Fixture {
    let dir = env.init("sci");
    let idea = |env: &TestEnv, title: &str| id_of(env.json(&dir, &["add", title, "--status", "idea"]));
    let briefed = idea(env, "Briefed");
    env.json(&dir, &["note", &briefed, "scope: briefed; brief: x"]);
    let proposed = idea(env, "Proposed");
    env.json(&dir, &["note", &proposed, "curate: stale; proposal: drop"]);
    let readmitted = idea(env, "Readmitted");
    env.json(&dir, &["note", &readmitted, "scope: briefed"]);
    env.json(&dir, &["note", &readmitted, "finding from research"]);
    let untouched = idea(env, "Untouched");
    env.json(&dir, &["note", &untouched, "just a comment"]);
    let no_notes = idea(env, "No notes");
    Fixture { dir, briefed, proposed, readmitted, untouched, no_notes }
}

#[test]
fn freezes_today_s_rule_and_sets_only_its_candidates() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    let mut set: Vec<String> = report["set"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect();
    set.sort();
    let mut want = vec![f.briefed.clone(), f.proposed.clone()];
    want.sort();
    assert_eq!(set, want);
    assert!(reviewed(&env, &f.dir, &f.briefed));
    assert!(reviewed(&env, &f.dir, &f.proposed));
    for id in [&f.readmitted, &f.untouched, &f.no_notes] {
        assert!(!reviewed(&env, &f.dir, id), "{id}");
    }
}

#[test]
fn a_rerun_rewrites_nothing_and_reads_no_notes() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    let file = format!("tasks/{}.md", f.briefed);
    let before = env.read(&f.dir, &file);
    // Readmitted after the first run, by note and by bare edit (latest note still scope:).
    env.json(&f.dir, &["note", &f.proposed, "--no-reviewed", "new finding"]);
    env.json(&f.dir, &["edit", &f.briefed, "--no-reviewed"]);
    let briefed_after = env.read(&f.dir, &file);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["set"].as_array().unwrap().is_empty(), "{report}");
    assert!(!reviewed(&env, &f.dir, &f.proposed));
    assert!(!reviewed(&env, &f.dir, &f.briefed));
    assert_eq!(env.read(&f.dir, &file), briefed_after);
    assert_ne!(before, briefed_after);
}

#[test]
fn an_interrupted_run_or_a_later_scope_pass_is_reported_not_reset() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    // Interrupted: the edit landed, the entry was never marked done; then readmitted.
    env.json(&f.dir, &["edit", &f.briefed, "--reviewed"]);
    env.json(&f.dir, &["edit", &f.briefed, "--no-reviewed"]);
    // An ordinary scope pass after the freeze, then readmitted.
    env.json(&f.dir, &["note", &f.proposed, "--reviewed", "scope: briefed"]);
    env.json(&f.dir, &["edit", &f.proposed, "--no-reviewed"]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["set"].as_array().unwrap().is_empty(), "{report}");
    let changed: Vec<&str> = report["changed"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(changed.contains(&f.briefed.as_str()) && changed.contains(&f.proposed.as_str()), "{report}");
    assert!(!reviewed(&env, &f.dir, &f.briefed));
    assert!(!reviewed(&env, &f.dir, &f.proposed));
    // Reported once: a third run reports nothing.
    let (_, third) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert!(third["changed"].as_array().unwrap().is_empty(), "{third}");
}

#[test]
fn apply_needs_the_operator_s_confirmation_and_no_live_claim() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, _) = backfill(&env, &f.dir, state.path(), &[]);
    assert_eq!(code, 2, "no mode is a usage error");
    let stamp = updated(&env, &f.dir, &f.briefed);
    // A live claim in the project refuses the pass before any edit.
    env.json(&f.dir, &["start", &f.untouched]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    assert_eq!(updated(&env, &f.dir, &f.briefed), stamp);
    // Once the claim is released, the next pass sets the untouched candidate (spec §8 case 14).
    env.json(&f.dir, &["park", &f.untouched, "later"]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(reviewed(&env, &f.dir, &f.briefed));
}

#[test]
fn a_corrupt_snapshot_refuses_before_any_edit() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("sci.json"), "{not json").unwrap();
    let stamp = updated(&env, &f.dir, &f.briefed);
    let (code, _) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 2);
    assert_eq!(updated(&env, &f.dir, &f.briefed), stamp);
}
```

`tempfile` is already a dev-dependency (`tests/common` uses `TempDir`). `start` on an idea may be refused by the CLI; if so, start a todo added for the purpose instead — the point is one live claim in the project.

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test review_backfill`
Expected: FAIL, `python3: can't open file '…/tools/review-backfill'`.

- [ ] **Step 3: Write the tool** at `tools/review-backfill` and `chmod +x` it:

```python
#!/usr/bin/env python3
"""One-time backfill of the `reviewed` flag (docs/specs/2026-10-11-review-state-design.md §7.2).

The first run on a project freezes its candidates -- open, unreviewed ideas whose latest
note starts with `scope:` or is a `curate:`/`scope:` note carrying a pending `proposal:`
-- with each record's `updated` stamp. Every run applies the flag only to entries whose
record is still an unreviewed idea with an unchanged stamp, and reports the rest. Notes
are read only at the freeze.

Run every apply pass inside a writer pause: no other session may write the project's
task records on any host (spec §7.2). --writers-paused is the operator's confirmation;
the tool also refuses while this host's claim store holds a live claim in the project.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone


class Usage(Exception):
    pass


def run(tasks, root, *args):
    env = dict(os.environ, TASKS_FORMAT="json")
    out = subprocess.run([tasks, "-C", root, *args], capture_output=True, text=True, env=env)
    return out.returncode, out.stdout, out.stderr


def read_json(tasks, root, *args):
    code, stdout, stderr = run(tasks, root, *args)
    if code != 0:
        raise RuntimeError(f"tasks {' '.join(args)} failed: {stderr.strip()}")
    return json.loads(stdout)


def eligible(task):
    """Today's scope eligibility rule, inverted: true when a pass has already looked."""
    notes = task.get("notes") or []
    if not notes:
        return False
    text = notes[-1]["text"]
    if text.startswith("scope:"):
        return True
    return text.startswith("curate:") and "proposal:" in text


def save(path, snapshot):
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".review-backfill-")
    with os.fdopen(fd, "w") as handle:
        json.dump(snapshot, handle, indent=1)
        handle.write("\n")
    os.replace(tmp, path)


def load(path):
    try:
        with open(path) as handle:
            snapshot = json.load(handle)
        snapshot["entries"]
        return snapshot
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise Usage(f"{path} is not a backfill snapshot ({error}); inspect it before rerunning")


def freeze(tasks, root, prefix, path):
    entries = []
    for row in read_json(tasks, root, "list", "--status", "idea")["tasks"]:
        if row.get("reviewed"):
            continue
        task = read_json(tasks, root, "show", row["id"])["task"]
        if task["status"] == "idea" and not task.get("reviewed") and eligible(task):
            entries.append({"id": task["id"], "updated": task["updated"], "done": False, "outcome": None})
    snapshot = {
        "prefix": prefix,
        "frozen_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "entries": entries,
    }
    save(path, snapshot)
    return snapshot


def live_claims(tasks, root, prefix):
    claims = read_json(tasks, root, "claims")["claims"]
    return [c["id"] for c in claims if c.get("prefix") == prefix and c.get("live")]


def apply(tasks, root, path, snapshot, report):
    for entry in snapshot["entries"]:
        if entry["done"]:
            continue
        code, stdout, stderr = run(tasks, root, "show", entry["id"])
        if code != 0:
            entry.update(done=True, outcome="changed: record not readable")
            report["changed"].append({"id": entry["id"], "detail": stderr.strip()})
            save(path, snapshot)
            continue
        task = json.loads(stdout)["task"]
        if task["status"] != "idea" or task.get("reviewed") or task["updated"] != entry["updated"]:
            detail = (f"status {task['status']}, reviewed {str(bool(task.get('reviewed'))).lower()}, "
                      f"updated {task['updated']} (frozen {entry['updated']})")
            entry.update(done=True, outcome=f"changed: {detail}")
            report["changed"].append({"id": entry["id"], "detail": detail})
            save(path, snapshot)
            continue
        code, _, stderr = run(tasks, root, "edit", entry["id"], "--reviewed")
        if code == 0:
            entry.update(done=True, outcome="set")
            report["set"].append(entry["id"])
        else:
            entry["outcome"] = f"failed: {stderr.strip()}"
            report["failed"].append({"id": entry["id"], "detail": stderr.strip()})
        save(path, snapshot)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--state", required=True, help="directory holding <prefix>.json snapshots")
    parser.add_argument("--root", default=".", help="project checkout (default: current directory)")
    parser.add_argument("--tasks", default="tasks", help="the tasks binary to run")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--freeze-only", action="store_true", help="write the snapshot and stop")
    mode.add_argument("--writers-paused", action="store_true",
                      help="confirm every other writer of this project's records is paused on every host")
    args = parser.parse_args()

    try:
        prefix = read_json(args.tasks, args.root, "prime")["prefix"]
        path = os.path.join(args.state, f"{prefix}.json")
        os.makedirs(args.state, exist_ok=True)
        report = {"prefix": prefix, "frozen": False, "set": [], "changed": [], "failed": [], "remaining": 0}
        if os.path.exists(path):
            snapshot = load(path)
        else:
            if args.writers_paused:
                held = live_claims(args.tasks, args.root, prefix)
                if held:
                    report["failed"] = [{"id": i, "detail": "live claim; pause its writer first"} for i in held]
                    print(json.dumps(report, indent=1))
                    return 1
            snapshot = freeze(args.tasks, args.root, prefix, path)
            report["frozen"] = True
        if args.writers_paused:
            held = live_claims(args.tasks, args.root, prefix)
            if held:
                report["failed"] = [{"id": i, "detail": "live claim; pause its writer first"} for i in held]
                print(json.dumps(report, indent=1))
                return 1
            apply(args.tasks, args.root, path, snapshot, report)
        report["remaining"] = sum(1 for e in snapshot["entries"] if not e["done"])
        print(json.dumps(report, indent=1))
        return 1 if report["failed"] else 0
    except Usage as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
```

argparse exits 2 on a missing mode, which the test expects.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-one --test review_backfill` then `just test-fast`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/review-backfill tests/review_backfill.rs
git commit -m "feat(review): add the one-time reviewed backfill tool"
```

---

## Rollout (after the branch is reviewed; operator steps, not implementer tasks)

These change live state across hosts. Each step that touches a host other than this one, or writes real records, is confirmed with the user at the time.

1. **Writer pause, merge, binaries.** Pause scope and curate passes and other agent sessions that write task records, on every host that reads synced task records. Merge the branch to main. Run `cargo install --path .` on this host, then on each other reading host (in its synced main checkout once the merge has arrived there). On each host confirm `tasks note --help` lists `--no-reviewed`. The skills are symlinks into the main checkout, so they go live with the merge; the pause covers the window until both binaries are installed (spec §7.0).
2. **Backfill, per registered project.** With writers still paused: on each host, `tasks claims` shows no live claim in the project; the operator confirms every other writer is paused. Then `tools/review-backfill --state ~/.local/state/tasks/review-backfill --root <project root> --writers-paused`. Read the report: `set`, `changed`, `failed`. Rerun after fixing any `failed` entry, under the same pause. A project without `[git] commit` gets one `chore(tasks): backfill reviewed on ideas` commit of its `tasks/` changes; a project with `[git] commit = true` commits each edit as the CLI does.
3. **Resume writers.** Resume the paused sessions.
4. **Old wakers.** Run `tasks search "Ideas it wakes" --all-projects` and record the count of open research tasks still carrying the plain-note wording on the implementation task (spec §7.3). Their bodies are not rewritten.
5. **Close out.** Note the approved finding on tasks-e34b99 (`--no-reviewed`, since it wakes it) and update `docs/notes/2026-09-30-work-selection-brief.md` in the same commit (tasks-a9b8d3's "Ideas it wakes").
