# Wait-Aware Review State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Return a reviewed idea to the scope queue on its own when it waits on nothing recorded, and rank ideas that have new evidence (`woken`) ahead of legacy ones that have none (`stranded`).

**Architecture:**
- **Records.** Three new frontmatter fields:
  - `awaiting_user`: a synced wait-on-user marker.
  - `reviewed_at`: stamped with `reviewed`.
  - `closed`: stamped by every move to done or dropped.
- **Derived state.** A pure `review::state` replaces `review::needs_review`. It judges an idea against:
  - its marker, its deferral and its latest note (for a pending proposal);
  - a per-scan dependency view, `review::Deps`, built once per scan by the read commands and passed to `TaskSummary::of`, `Fields::of_task` and the parked rows.
- **Output.** Rows gain `review_reason`.
- **Migration.** A one-time Python tool turns legacy prose waits into dependency edges and markers. It works from a frozen snapshot and runs under a writer pause.

**Tech Stack:** Rust (clap, serde, `time`, assert_cmd integration tests); Python 3 standard library for the backfill tool.

**Spec:** `docs/specs/2026-10-17-wait-aware-review-design.md` (accepted after review round 2). Read it with this plan; section numbers below refer to it. It amends `docs/specs/2026-10-11-review-state-design.md`, whose plan `docs/plans/2026-10-11-review-state.md` touched the same files and is a useful map.

## Global Constraints

- **Stored fields** (spec §3):
  - `awaiting_user: true` is written only when set, as `Value::Raw("true")`, like `reviewed`.
  - `reviewed_at: <stamp>` and `closed: <stamp>` are written only when set, as `Value::Raw`, in the `updated` stamp form (`2026-10-17T09:59:00Z`).
- **`review_reason`** has exactly these values: `unreviewed`, `woken`, `stranded`, `unassessed` (spec §4.2). It is serialized only when `needs_review` is true, and `needs_review` is exactly `review_reason.is_some()`.
- **A reviewed idea waits** when any holds (spec §4.1):
  - `awaiting_user`;
  - `defer::is_deferred(task, now)`;
  - `proposal::pending(task).is_some()`;
  - some dependency resolves (following `moved_to`) to an open task, or cannot be resolved.
- **Woken** (spec §4.2) requires `reviewed_at`, and then either:
  - a dependency's `closed >= reviewed_at` (equal stamps count as woken); or
  - a deferral due on a date strictly later than `reviewed_at`'s UTC date.
- **Exact texts:**
  - Refusal: `<id> is not reviewed; only a reviewed idea carries awaiting_user`.
  - Plain-note warning with the marker set: `<id> stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope`.
  - Without the marker the existing warning is unchanged: `<id> stays reviewed; add --no-reviewed to readmit it for scope`.
  - Editor refusals: `reviewed_at is stamped by reviewing the idea; it cannot be edited` and `closed is stamped by closing the task; it cannot be edited`.
- **`check` findings:** kind `review_status` for `awaiting_user` without `reviewed` and for `reviewed_at` without `reviewed`; kind `closed_status` for `closed` on a task that is not done or dropped (spec §3.4).
- **Dependency views:** one `Deps` per scan, never shared between scans, so a parked row read from its recorded checkout is judged with that checkout's scan (spec §4.3). Parked views keep `Prefer::Registered` and `Prefer::Recorded` exactly as today.
- **Notes:** the rule reads only the latest note, for the proposal test (spec §4.3).
- **CLI surface:** `note --awaiting-user`; `edit --awaiting-user` and `edit --no-awaiting-user`. The rows land in ops's `cli.toml` first, and this repo adopts them with `vendored adopt` in the parser commit (AGENTS.md Rules). Never edit `tools/cli.toml` by hand.
- **Rollout:** no binary from this branch runs on any host until every host that reads synced records upgrades together (spec §7.1); see Rollout.
- **Tests:** run them only through `just test-one <args>` and `just test-fast`, never `cargo test` directly (AGENTS.md).

## Review Focus

1. A reviewed idea with two edges, one closed after its review and one still open, is waiting, not woken. Test in Task 4 (unit).
2. A reviewed idea whose only dependency is shelved is waiting: a shelf is open. Test in Task 5.
3. A dependency in another registered project closes. From inside the idea's project, and under `--all-projects`, the idea is `woken`. Test in Task 5.
4. A recurring dependency completes and is then reopened with `start`. The reopen clears `closed`, so the idea waits again. Test in Task 2 (the stamp) and Task 4 (unit: an open dependency outranks an old close).
5. `edit --reviewed` on an idea that is already reviewed re-stamps `reviewed_at`, as `note --reviewed` does. Test in Task 3.

## Tracking

- **Close the design task.** tasks-1a3280 ends when this plan is reviewed. Its closeout runs on the design branch `feat/wait-aware-review`, before that branch merges:
  1. File the implementation goal under tasks-5408fe: `tasks add "Implement wait-aware review state" --parent tasks-5408fe --plan 2026-10-17-wait-aware-review --spec 2026-10-17-wait-aware-review-design -p 2 --size l --complexity high --process planned --source tasks-1a3280`.
  2. Under that goal, add one child per `### Task N:` heading below, with `--plan 2026-10-17-wait-aware-review --step "Task N: <title>"`, `--process direct`, and the complexity named in each task's heading line.
  3. Run `tasks note tasks-163055 --no-reviewed "<decision and the goal's id>"`.
  4. Note tasks-22f407 and tasks-d4e73d (below).
  5. Update `docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md` with the decisions.
- **Run the implementation.** The implementation goal runs this plan in its own worktree. The Rollout below is its last step.

## Concurrent work

- **tasks-22f407** (`list --pending`) also moves `pending_proposal` to a shared place. This plan moves it to `src/proposal.rs` as `proposal::pending` (Task 4). Whichever lands second rebases onto that module rather than adding a second helper. The design-task closeout notes this on tasks-22f407.
- **Rebase onto main** before Task 2 and again before the final review. Expect textual conflicts only near `KEYS`, `parse_task` and `TaskSummary`.

---

### Task 1: The CLI rows in ops (complexity: low)

The `parser_surface_equals_table` test fails until the vendored `tools/cli.toml` lists every parser option, and `tools/cli.toml` arrives only from ops main. These rows must land in ops before Task 3 adopts them.

**Files:**
- Modify (in an ops worktree, then ops main): `cli.toml`

**Interfaces:**
- Produces: ops main's `cli.toml` with `{ names = ["--awaiting-user"], value = "none" }` on `note` and `edit`, and `{ names = ["--no-awaiting-user"], value = "none" }` on `edit`. Task 3 adopts it.

- [ ] **Step 1: Create the ops worktree**

In the ops main checkout (`tasks --pretty root ops-88a0fa` prints it; clean tree on `main`):

```bash
work-link --ensure .worktrees
git worktree add -b feat/cli-awaiting-user .worktrees/cli-awaiting-user main
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/cli-awaiting-user
```

ops's justfile defines no `setup`.

- [ ] **Step 2: Add the rows**

In `.worktrees/cli-awaiting-user/cli.toml`, inside the `path = ["edit"]` block:
- after `{ names = ["--no-reviewed"], value = "none" },` add `{ names = ["--no-awaiting-user"], value = "none" },`
- after `{ names = ["--reviewed"], value = "none" },` add `{ names = ["--awaiting-user"], value = "none" },`

Inside the `path = ["note"]` block, after `{ names = ["--no-reviewed"], value = "none" },` add `{ names = ["--awaiting-user"], value = "none" },`.

Run: `git diff --stat`
Expected: `cli.toml | 3 +++` and nothing else.

- [ ] **Step 3: Run ops's gate in the worktree**

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 4: Commit, merge locally, remove the worktree**

```bash
git add cli.toml
git commit -m "feat(cli): awaiting-user marker on tasks note and edit"
cd "$(tasks --pretty root ops-88a0fa)"
git merge --ff-only feat/cli-awaiting-user
git -C .worktrees/cli-awaiting-user status --ignored --porcelain   # expect nothing to keep
git worktree unlock .worktrees/cli-awaiting-user
git worktree remove .worktrees/cli-awaiting-user
git branch -d feat/cli-awaiting-user
```

Expected: the merge fast-forwards. If it does not, rebase the branch onto ops main, rerun Step 3, and merge again. ops is a personal-profile repository, so a local merge needs no further approval (global Decisions rule).

---

### Task 2: The stored fields `awaiting_user`, `reviewed_at`, `closed` (complexity: mid)

**Files:**
- Modify: `src/model.rs` (the `Task` struct and the `Task { … }` literal near line 149)
- Modify: `src/format.rs` (`KEYS`, `parse_task`, `quote_timestamps`, `validate_task`, `serialize_task`, unit tests)
- Modify: every other `Task { … }` literal. Find them with `grep -rn "reviewed: false," src`: `src/defer.rs`, `src/halt.rs`, `src/periodic.rs`, `src/lanes.rs`, `src/query.rs`, `src/repo.rs`, `src/hierarchy.rs`, `src/complexity.rs`, `src/similarity.rs`, `src/commands/add.rs`, and any others the grep lists.
- Modify: `src/commands/mod.rs` (`transition`)
- Modify: `src/review.rs` (`validate`, a new `not_reviewed`, unit tests)
- Modify: `src/commands/edit.rs` (`check_invariants`)
- Modify: `src/commands/check.rs` (findings)
- Modify: `src/moving/preflight.rs:98-101`, `src/moving/journal.rs:272-275` (tombstone)
- Test: `src/format.rs`, `src/review.rs`, `tests/review.rs`, `tests/move.rs`

**Interfaces:**
- Produces:
  - Fields: `Task::awaiting_user: bool`, `Task::reviewed_at: Option<String>`, `Task::closed: Option<String>`.
  - `crate::review::not_reviewed(task: &Task) -> crate::error::Error`.
  - Check finding kinds `review_status` (extended) and `closed_status`.

- [ ] **Step 1: Write the failing format test** in `src/format.rs`'s `mod tests`:

```rust
#[test]
fn review_and_close_stamps_round_trip_and_are_omitted_when_unset() {
    let text = "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\nreviewed: true\n\
                awaiting_user: true\nreviewed_at: 2026-10-17T09:00:00Z\n\
                created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                closed: 2026-10-17T10:00:00Z\ndepends: []\ntags: []\n---\n";
    let task = parse_task(text, "sci-000001.md").unwrap();
    assert!(task.awaiting_user);
    assert_eq!(task.reviewed_at.as_deref(), Some("2026-10-17T09:00:00Z"));
    assert_eq!(task.closed.as_deref(), Some("2026-10-17T10:00:00Z"));
    let written = serialize_task(&task);
    assert!(written.contains("\nawaiting_user: true\n"), "{written}");
    assert!(written.contains("\nreviewed_at: 2026-10-17T09:00:00Z\n"), "{written}");
    assert!(written.contains("\nclosed: 2026-10-17T10:00:00Z\n"), "{written}");
    assert_eq!(parse_task(&written, "sci-000001.md").unwrap(), task);
    let mut cleared = task.clone();
    cleared.awaiting_user = false;
    cleared.reviewed_at = None;
    cleared.closed = None;
    let written = serialize_task(&cleared);
    for key in ["awaiting_user", "reviewed_at", "closed"] {
        assert!(!written.contains(key), "{key} written when unset: {written}");
    }
    assert!(parse_task(&text.replace("2026-10-17T09:00:00Z", "soon"), "f").is_err());
    assert!(parse_task(&text.replace("2026-10-17T10:00:00Z", "2026-13-45T00:00:00Z"), "f").is_err());
}
```


- [ ] **Step 2: Run it to verify it fails**

Run: `just test-one review_and_close_stamps_round_trip`
Expected: FAIL to compile: no field `awaiting_user` on `Task`.

- [ ] **Step 3: Add the fields**

In `src/model.rs`, after `pub reviewed: bool,`:

```rust
    /// The idea waits on the user's answer. Only on a reviewed idea; cleared with
    /// `reviewed` and by every status transition.
    /// See docs/specs/2026-10-17-wait-aware-review-design.md §3.1.
    pub awaiting_user: bool,
    /// When the latest review was recorded: set and cleared with `reviewed`. Absent on a
    /// review recorded before the field existed. Same spec §3.2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
```

After `pub completed: Option<String>,`:

```rust
    /// When the task last closed (done or dropped): stamped with `completed` on a
    /// completion and alone on a drop, cleared by reopening, never set by `move`.
    /// Same spec §3.3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
```

In `src/format.rs`:
- `KEYS` becomes `[&str; 33]`. Insert `"awaiting_user", "reviewed_at"` after `"reviewed"`, and `"closed"` after `"completed"`.
- In `parse_task`'s `Task { … }`, after `reviewed: boolean("reviewed")?,` add `awaiting_user: boolean("awaiting_user")?,` and `reviewed_at: scalar("reviewed_at")?,`; after `completed: scalar("completed")?,` add `closed: scalar("closed")?,`.
- In `quote_timestamps`, add `|| line.starts_with("reviewed_at: ")` and `|| line.starts_with("closed: ")` to the condition.
- In `validate_task`, after the `completed` check:

```rust
    if let Some(reviewed_at) = &t.reviewed_at {
        crate::time::parse(reviewed_at)?;
    }
    if let Some(closed) = &t.closed {
        crate::time::parse(closed)?;
    }
```

- In `serialize_task`, after the `reviewed` block:

```rust
    // Raw for the same reason as `parallel`.
    if t.awaiting_user {
        pairs.push(("awaiting_user".into(), Value::Raw("true".into())));
    }
    if let Some(reviewed_at) = &t.reviewed_at {
        pairs.push(("reviewed_at".into(), Value::Raw(reviewed_at.clone())));
    }
```

  and after the `completed` block:

```rust
    if let Some(closed) = &t.closed {
        pairs.push((String::from("closed"), Value::Raw(closed.clone())));
    }
```

Add `awaiting_user: false, reviewed_at: None, closed: None,` to every `Task { … }` literal the grep listed, next to `reviewed: false,`.

- [ ] **Step 4: Run it to verify it passes**

Run: `just test-one review_and_close_stamps_round_trip`
Expected: PASS.

- [ ] **Step 5: Write the failing validity unit test** in `src/review.rs`'s `mod tests`:

```rust
#[test]
fn validate_refuses_marker_or_stamp_without_reviewed() {
    let parse = |extra: &str| {
        crate::format::parse_task(
            &format!(
                "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\n{extra}\
                 created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
            "sci-000001.md",
        )
        .unwrap()
    };
    let error = validate(&parse("awaiting_user: true\n")).unwrap_err().to_string();
    assert!(
        error.contains("sci-000001 is not reviewed; only a reviewed idea carries awaiting_user"),
        "{error}"
    );
    let error = validate(&parse("reviewed_at: 2026-10-17T09:00:00Z\n"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("sci-000001 carries reviewed_at without reviewed"), "{error}");
    assert!(validate(&parse("reviewed: true\nawaiting_user: true\nreviewed_at: 2026-10-17T09:00:00Z\n")).is_ok());
    assert!(validate(&parse("reviewed: true\n")).is_ok(), "a legacy review has no stamp");
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `just test-one validate_refuses_marker_or_stamp_without_reviewed`
Expected: FAIL: `validate` returns `Ok` for the first record.

- [ ] **Step 7: Extend `validate`** in `src/review.rs`:

```rust
/// The refusal for `awaiting_user` without `reviewed` (wait-aware spec §3.1).
pub fn not_reviewed(task: &Task) -> Error {
    Error::Validation(format!(
        "{} is not reviewed; only a reviewed idea carries awaiting_user",
        task.id
    ))
}

/// The write-time rule (review-state spec §3.2, wait-aware spec §3.1–3.2). Never called
/// from parsing: `parse_task` must accept a hand-edited record so `check` can report it
/// and an editor save can transition it.
pub fn validate(task: &Task) -> Result<()> {
    if task.reviewed && task.status != Status::Idea {
        return Err(not_an_idea(task));
    }
    if task.awaiting_user && !task.reviewed {
        return Err(not_reviewed(task));
    }
    if task.reviewed_at.is_some() && !task.reviewed {
        return Err(Error::Validation(format!(
            "{} carries reviewed_at without reviewed",
            task.id
        )));
    }
    Ok(())
}
```

Run: `just test-one validate_refuses_marker_or_stamp_without_reviewed`
Expected: PASS.

- [ ] **Step 8: Write the failing CLI tests** in `tests/review.rs`. Add the helpers below `seed_reviewed`:

```rust
/// Inserts frontmatter lines into a record by hand, before its priority line: states the
/// writers never produce, or stamps a test needs at a fixed instant.
fn seed(dir: &Path, id: &str, lines: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let seeded = text.replacen("\npriority: ", &format!("\n{lines}\npriority: "), 1);
    assert_ne!(seeded, text, "no priority line in {id}");
    std::fs::write(&path, seeded).unwrap();
}

/// Replaces the value of a frontmatter line the record already carries.
fn restamp(dir: &Path, id: &str, key: &str, value: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("\n{key}: ");
    let start = text.find(&prefix).unwrap_or_else(|| panic!("no {key} in {id}")) + prefix.len();
    let end = start + text[start..].find('\n').unwrap();
    std::fs::write(&path, format!("{}{value}{}", &text[..start], &text[end..])).unwrap();
}

fn task(env: &TestEnv, dir: &Path, id: &str) -> serde_json::Value {
    env.json(dir, &["show", id])["task"].clone()
}

fn check_kinds(env: &TestEnv, dir: &Path) -> Vec<String> {
    let out = env.cmd(dir).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_string())
        .collect()
}
```

The tests:

```rust
#[test]
fn done_and_drop_stamp_closed_and_reopening_clears_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let done = id_of(env.json(&dir, &["add", "D"]));
    env.json(&dir, &["done", &done]);
    let shown = task(&env, &dir, &done);
    assert!(shown["closed"].is_string(), "{shown}");
    assert_eq!(shown["closed"], shown["completed"]);
    let dropped = id_of(env.json(&dir, &["add", "X"]));
    env.json(&dir, &["drop", &dropped]);
    let shown = task(&env, &dir, &dropped);
    assert!(shown["closed"].is_string(), "{shown}");
    assert!(shown.get("completed").is_none(), "{shown}");
    env.json(&dir, &["edit", &done, "--status", "todo"]);
    assert!(task(&env, &dir, &done).get("closed").is_none());
    assert!(!env.read(&dir, &format!("tasks/{done}.md")).contains("closed:"));
    // A recurring completion stamps it, a reopen clears it, the next completion stamps it again.
    let every = id_of(env.json(&dir, &["add", "R", "--every", "7d"]));
    env.json(&dir, &["start", &every]);
    env.json(&dir, &["done", &every]);
    assert!(task(&env, &dir, &every)["closed"].is_string());
    env.json(&dir, &["start", &every]);
    assert!(task(&env, &dir, &every).get("closed").is_none());
    env.json(&dir, &["done", &every]);
    assert!(task(&env, &dir, &every)["closed"].is_string());
}

#[test]
fn every_status_transition_clears_the_review_stamp_and_marker() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(&dir, &id, "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\nawaiting_user: true");
    env.json(&dir, &["edit", &id, "--status", "todo"]);
    let text = env.read(&dir, &format!("tasks/{id}.md"));
    for key in ["reviewed", "reviewed_at", "awaiting_user"] {
        assert!(!text.contains(&format!("\n{key}:")), "{key} kept: {text}");
    }
}

#[test]
fn check_reports_review_stamps_and_closed_out_of_place() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    seed(&dir, &todo, "awaiting_user: true");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(&dir, &idea, "reviewed_at: 2026-10-17T09:00:00Z");
    let open = id_of(env.json(&dir, &["add", "O"]));
    seed(&dir, &open, "closed: 2026-10-17T09:00:00Z");
    let kinds = check_kinds(&env, &dir);
    assert_eq!(kinds.iter().filter(|k| *k == "review_status").count(), 2, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| *k == "closed_status").count(), 1, "{kinds:?}");
}

#[test]
fn the_editor_refuses_to_change_closed_or_reviewed_at() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let done = id_of(env.json(&dir, &["add", "D"]));
    env.json(&dir, &["done", &done]);
    let file = format!("tasks/{done}.md");
    let before = env.read(&dir, &file);
    let script = editor_script(&dir, "sed -i 's/^closed: .*/closed: 2020-01-01T00:00:00Z/' \"$1\"");
    let out = env.cmd(&dir).env("EDITOR", &script).args(["edit", &done]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("closed is stamped by closing the task; it cannot be edited")
    );
    assert_eq!(env.read(&dir, &file), before);
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(&dir, &idea, "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z");
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let script = editor_script(&dir, "sed -i 's/^reviewed_at: .*/reviewed_at: 2020-01-01T00:00:00Z/' \"$1\"");
    let out = env.cmd(&dir).env("EDITOR", &script).args(["edit", &idea]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("reviewed_at is stamped by reviewing the idea; it cannot be edited")
    );
    assert_eq!(env.read(&dir, &file), before);
}
```

In `tests/move.rs` (which has `id_of`):

```rust
#[test]
fn move_clears_review_stamps_on_the_tombstone_and_keeps_them_on_the_target() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
    let path = source.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap().replacen(
        "\npriority: ",
        "\nreviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\nawaiting_user: true\npriority: ",
        1,
    );
    std::fs::write(&path, text).unwrap();
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    let tombstone = env.read(&source, &format!("tasks/{id}.md"));
    for key in ["reviewed", "reviewed_at", "awaiting_user", "closed"] {
        assert!(!tombstone.contains(&format!("\n{key}:")), "{key} on tombstone: {tombstone}");
    }
    let target = env.json(&destination, &["show", &moved])["task"].clone();
    assert_eq!(target["reviewed"], true);
    assert_eq!(target["reviewed_at"], "2026-10-17T09:00:00Z");
    assert_eq!(target["awaiting_user"], true);
}
```

- [ ] **Step 9: Run them to verify they fail**

Run: `just test-one --test review closed` then `just test-one --test review review_stamp`, `just test-one --test review out_of_place`, `just test-one --test review the_editor_refuses_to_change`, `just test-one --test move move_clears_review_stamps`
Expected: each FAILS. `closed` is never written; the transition keeps `reviewed_at`; `check` reports no `closed_status`; the editor accepts the change; and the tombstone fails validation or keeps `reviewed_at`.

- [ ] **Step 10: Implement**

`src/commands/mod.rs`, `transition()`. Directly after the `if completing { … } else if … { task.completed = None; }` block:

```rust
    // Wait-aware spec §3.3: every close stamps `closed`; a reopen clears it. `move` never
    // passes through here, so a tombstone carries none.
    if closing {
        task.closed = Some(now.clone());
    } else if matches!(task.status, Status::Done | Status::Dropped) && to.is_open() {
        task.closed = None;
    }
```

In the `if task.status != to { … }` block, after `task.reviewed = false;`:

```rust
        task.reviewed_at = None;
        task.awaiting_user = false;
```

`src/commands/edit.rs`, `check_invariants`. After the `completed` refusal:

```rust
    if edited.closed != original.closed {
        return Err(Error::Validation(
            "closed is stamped by closing the task; it cannot be edited".into(),
        ));
    }
    if edited.reviewed_at != original.reviewed_at {
        return Err(Error::Validation(
            "reviewed_at is stamped by reviewing the idea; it cannot be edited".into(),
        ));
    }
```

`src/commands/check.rs`. After the existing `review_status` finding:

```rust
        // Wait-aware spec §3.4: the writers refuse these, so only a hand edit lands here.
        if task.awaiting_user && !task.reviewed {
            errors.push(finding(
                Some(task),
                file.clone(),
                "review_status",
                "carries awaiting_user without reviewed; only a reviewed idea carries it".into(),
            ));
        }
        if task.reviewed_at.is_some() && !task.reviewed {
            errors.push(finding(
                Some(task),
                file.clone(),
                "review_status",
                "carries reviewed_at without reviewed".into(),
            ));
        }
        if task.closed.is_some() && !matches!(task.status, Status::Done | Status::Dropped) {
            errors.push(finding(
                Some(task),
                file.clone(),
                "closed_status",
                format!(
                    "is {} and carries closed; only a done or dropped task carries it",
                    task.status.as_str()
                ),
            ));
        }
```

`src/moving/preflight.rs`, after `tombstone.reviewed = false;`, and `src/moving/journal.rs`, after `expected_tombstone.reviewed = false;`, add the same two lines (with `expected_tombstone` in the journal):

```rust
    tombstone.reviewed_at = None;
    tombstone.awaiting_user = false;
```

- [ ] **Step 11: Run them to verify they pass**

Run the five commands from Step 9.
Expected: PASS.

- [ ] **Step 12: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src tests
git commit -m "feat(review): store awaiting_user, reviewed_at and closed on the record"
```

---

### Task 3: Writing the marker and the review stamp (complexity: mid)

**Files:**
- Modify: `src/review.rs` (`Awaiting`, `apply`, `stamp_edited`)
- Modify: `src/cli.rs` (`EditArgs` near line 314; `Command::Note` near line 620)
- Modify: `src/commands/mod.rs` (the `Command::Note` dispatch near line 2017)
- Modify: `src/commands/status.rs` (`note`)
- Modify: `src/commands/edit.rs` (`has_flags`, the review block near line 272, `editor`)
- Modify: `src/show_text.rs` (the `reviewed` row near line 252; the `completed` row near line 272)
- Modify: `tools/cli.toml`, adopted from ops main and never edited by hand
- Test: `src/review.rs`, `tests/review.rs`

**Interfaces:**
- Consumes: Task 2's fields and `review::not_reviewed`; Task 1's rows on ops main.
- Produces:
  - `crate::review::Awaiting { Keep, Set, Clear }` with `Awaiting::from_flags(set: bool, clear: bool) -> Awaiting`.
  - `crate::review::apply(task: &mut Task, review: Review, awaiting: Awaiting, warnings: &mut Vec<String>) -> Result<()>`.
  - `crate::review::stamp_edited(original: &Task, edited: &mut Task)`.
  - `status::note(ctx, id, text, stamp, review: Review, awaiting: Awaiting)`.

- [ ] **Step 1: Adopt the CLI rows**

```bash
python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml
git diff --stat -- tools/cli.toml
```

Expected: `tools/cli.toml | 3 +++`. `parser_surface_equals_table` now fails until Step 6 adds the parser options; that is the RED for the surface.

- [ ] **Step 2: Write the failing CLI tests** in `tests/review.rs`:

```rust
#[test]
fn reviewing_stamps_reviewed_at_and_a_rereview_restamps_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let first = task(&env, &dir, &id)["reviewed_at"].as_str().unwrap().to_string();
    assert!(first.ends_with('Z'), "{first}");
    restamp(&dir, &id, "reviewed_at", "2026-01-01T00:00:00Z");
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed again"]);
    assert_ne!(task(&env, &dir, &id)["reviewed_at"], "2026-01-01T00:00:00Z");
    restamp(&dir, &id, "reviewed_at", "2026-01-01T00:00:00Z");
    env.json(&dir, &["edit", &id, "--reviewed"]);
    assert_ne!(task(&env, &dir, &id)["reviewed_at"], "2026-01-01T00:00:00Z");
}

#[test]
fn awaiting_user_is_set_with_a_review_and_cleared_by_readmission() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "--awaiting-user", "scope: question"]);
    let shown = task(&env, &dir, &id);
    assert_eq!(shown["reviewed"], true);
    assert_eq!(shown["awaiting_user"], true);
    // A plain note keeps both and says so.
    let out = env.json(&dir, &["note", &id, "an aside"]);
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains(&format!(
            "{id} stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope"
        )),
        "{warnings}"
    );
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], true);
    // edit clears and sets the marker alone, without a warning.
    let out = env.json(&dir, &["edit", &id, "--no-awaiting-user"]);
    assert!(!out["warnings"].to_string().contains("stays reviewed"), "{out}");
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], false);
    assert_eq!(task(&env, &dir, &id)["reviewed"], true);
    env.json(&dir, &["edit", &id, "--awaiting-user"]);
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], true);
    // The answer readmits it: both flags and the stamp go.
    env.json(&dir, &["note", &id, "--no-reviewed", "answer: yes"]);
    let shown = task(&env, &dir, &id);
    assert_eq!(shown["reviewed"], false);
    assert_eq!(shown["awaiting_user"], false);
    assert!(shown.get("reviewed_at").is_none(), "{shown}");
}

#[test]
fn awaiting_user_is_refused_without_a_review_and_on_a_non_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let out = env.cmd(&dir).args(["note", &idea, "--awaiting-user", "q"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains(&format!("{idea} is not reviewed; only a reviewed idea carries awaiting_user"))
    );
    assert_eq!(env.read(&dir, &file), before);
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    let out = env.cmd(&dir).args(["edit", &todo, "--awaiting-user"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("only an idea carries reviewed"));
    assert_eq!(env.read(&dir, &file), before);
    // clap: the marker cannot ride on a readmission.
    env.usage(&dir, &["note", &idea, "--awaiting-user", "--no-reviewed", "x"]);
    env.usage(&dir, &["edit", &idea, "--awaiting-user", "--no-awaiting-user"]);
}

#[test]
fn the_editor_stamps_reviewed_at_with_reviewed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let add = editor_script(&dir, "sed -i 's/^priority: /reviewed: true\\npriority: /' \"$1\"");
    env.cmd(&dir).env("EDITOR", &add).args(["edit", &idea]).assert().success();
    assert!(task(&env, &dir, &idea)["reviewed_at"].is_string());
    // Removing reviewed while the marker stays is refused, file unchanged.
    env.json(&dir, &["edit", &idea, "--awaiting-user"]);
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let drop_reviewed = editor_script(&dir, "sed -i '/^reviewed: /d' \"$1\"");
    let out = env.cmd(&dir).env("EDITOR", &drop_reviewed).args(["edit", &idea]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("only a reviewed idea carries awaiting_user"));
    assert_eq!(env.read(&dir, &file), before);
    // Removing both clears the stamp.
    env.json(&dir, &["edit", &idea, "--no-awaiting-user"]);
    env.cmd(&dir).env("EDITOR", &drop_reviewed).args(["edit", &idea]).assert().success();
    assert!(task(&env, &dir, &idea).get("reviewed_at").is_none());
}

#[test]
fn show_prints_the_review_date_the_marker_and_closed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &idea, "--reviewed", "--awaiting-user", "scope: question"]);
    restamp(&dir, &idea, "reviewed_at", "2026-01-02T03:04:05Z");
    let shown = env.pretty(&dir, &["show", &idea]);
    assert!(shown.contains("reviewed") && shown.contains("yes (2026-01-02)"), "{shown}");
    assert!(shown.contains("awaiting") && shown.contains("user"), "{shown}");
    let legacy = id_of(env.json(&dir, &["add", "L", "--status", "idea"]));
    seed_reviewed(&dir, &legacy);
    let shown = env.pretty(&dir, &["show", &legacy]);
    assert!(shown.contains("yes") && !shown.contains("yes ("), "{shown}");
    let dropped = id_of(env.json(&dir, &["add", "X"]));
    env.json(&dir, &["drop", &dropped]);
    assert!(env.pretty(&dir, &["show", &dropped]).contains("closed"));
}
```


- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one --test review reviewing_stamps`, `just test-one --test review awaiting_user`, `just test-one --test review the_editor_stamps`, `just test-one --test review show_prints_the_review`
Expected: FAIL. There is no `reviewed_at` yet, and `--awaiting-user` is rejected by clap as an unexpected argument.

- [ ] **Step 4: Rework `apply`** in `src/review.rs`. It replaces the current `apply`:

```rust
/// What a write does to the `awaiting_user` marker (wait-aware spec §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awaiting {
    Keep,
    Set,
    Clear,
}

impl Awaiting {
    /// clap's `conflicts_with` keeps both from arriving together.
    pub fn from_flags(set: bool, clear: bool) -> Awaiting {
        match (set, clear) {
            (true, _) => Awaiting::Set,
            (_, true) => Awaiting::Clear,
            _ => Awaiting::Keep,
        }
    }
}

/// Applies `review`, then `awaiting`, before anything is written, so a refusal leaves the
/// file untouched. A write that changes neither (a plain note) on a reviewed idea warns:
/// it does not readmit the idea.
pub fn apply(
    task: &mut Task,
    review: Review,
    awaiting: Awaiting,
    warnings: &mut Vec<String>,
) -> Result<()> {
    match review {
        Review::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            task.reviewed = true;
            task.reviewed_at = Some(crate::time::now());
        }
        Review::Clear => {
            task.reviewed = false;
            task.reviewed_at = None;
            task.awaiting_user = false;
        }
        Review::Keep => {}
    }
    match awaiting {
        Awaiting::Set => {
            if task.status != Status::Idea {
                return Err(not_an_idea(task));
            }
            if !task.reviewed {
                return Err(not_reviewed(task));
            }
            task.awaiting_user = true;
        }
        Awaiting::Clear => task.awaiting_user = false,
        Awaiting::Keep => {}
    }
    if review == Review::Keep && awaiting == Awaiting::Keep && task.reviewed {
        warnings.push(if task.awaiting_user {
            format!(
                "{} stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope",
                task.id
            )
        } else {
            format!(
                "{} stays reviewed; add --no-reviewed to readmit it for scope",
                task.id
            )
        });
    }
    Ok(())
}

/// The editor's half of wait-aware spec §3.2: `reviewed_at` follows a save's change to
/// `reviewed`. `check_invariants` has already refused a direct change to the stamp.
pub fn stamp_edited(original: &Task, edited: &mut Task) {
    match (original.reviewed, edited.reviewed) {
        (false, true) => edited.reviewed_at = Some(crate::time::now()),
        (true, false) => edited.reviewed_at = None,
        _ => {}
    }
}
```

- [ ] **Step 5: Thread it through `note` and `edit`**

`src/cli.rs`, `Command::Note`, after `no_reviewed`:

```rust
        /// Mark the reviewed idea as waiting on the user's answer, in the same write.
        #[arg(long, conflicts_with = "no_reviewed")]
        awaiting_user: bool,
```

`EditArgs`, after `no_reviewed`:

```rust
    /// Mark this reviewed idea as waiting on the user's answer.
    #[arg(long, conflicts_with_all = ["no_reviewed", "no_awaiting_user"])]
    pub awaiting_user: bool,
    /// Clear the wait on the user, leaving the idea reviewed.
    #[arg(long)]
    pub no_awaiting_user: bool,
```

`src/commands/mod.rs`, the `Command::Note` arm: destructure `awaiting_user` and pass `crate::review::Awaiting::from_flags(awaiting_user, false)` as `status::note`'s new last argument.

`src/commands/status.rs`, `note`: add the parameter `awaiting: crate::review::Awaiting` and call `crate::review::apply(&mut task, review, awaiting, &mut ctx.warnings)?;`. Compile and fix any other caller of `status::note` or `review::apply` the compiler names by passing `Awaiting::Keep`.

`src/commands/edit.rs`:
- in `has_flags`, add `|| args.awaiting_user || args.no_awaiting_user`;
- replace the review block near line 274 with:

```rust
    if args.reviewed || args.no_reviewed || args.awaiting_user || args.no_awaiting_user {
        crate::review::apply(
            &mut task,
            crate::review::Review::from_flags(args.reviewed, args.no_reviewed),
            crate::review::Awaiting::from_flags(args.awaiting_user, args.no_awaiting_user),
            &mut ctx.warnings,
        )?;
    }
```

- in `editor`, directly after `check_invariants(&original, &edited).map_err(keep)?;`, add `crate::review::stamp_edited(&original, &mut edited);`.

- [ ] **Step 6: The `show` rows** in `src/show_text.rs`. Replace the `reviewed` row:

```rust
    if t.reviewed {
        let mut value = vec![Span::plain("yes")];
        if let Some(at) = &t.reviewed_at {
            value.extend([Span::plain(" ("), present::date(at), Span::plain(")")]);
        }
        rows.push(("reviewed", value));
    }
    if t.awaiting_user {
        rows.push(("awaiting", vec![Span::plain("user")]));
    }
```

After the `completed` row:

```rust
    if let Some(closed) = &t.closed {
        rows.push(("closed", vec![present::date(closed)]));
    }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run the four commands from Step 3, then `just test-one parser_surface_equals_table`.
Expected: PASS.

- [ ] **Step 8: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src tests tools/cli.toml
git commit -m "feat(review): mark ideas awaiting the user and stamp reviewed_at"
```

---

### Task 4: The derived state and per-scan dependency views (complexity: high)

**Files:**
- Create: `src/proposal.rs`
- Modify: `src/main.rs` (add `mod proposal;` beside `mod review;`)
- Modify: `src/commands/sample.rs` (use `proposal::pending`; delete `pending_proposal`)
- Modify: `src/defer.rs` (`Defer::date`)
- Modify: `src/review.rs` (`ReviewReason`, `DepState`, `Deps`, `state`; delete `needs_review`)
- Modify: `src/output.rs` (`TaskSummary::of`, the `review_reason` field; `ParkedRow`)
- Modify: `src/filter.rs` (`Fields::of_task`)
- Modify: `src/commands/list.rs` (`review_deps`; every `TaskSummary::of` and `Fields::of_task` call)
- Modify: `src/commands/parked.rs` (`sourced_rows`, `resolved_row`, `recorded_or_fallback`, `resolve_recorded`, the filter near line 235)
- Modify: `src/hierarchy.rs` (`forest`, `node`), `src/lanes.rs` (`Inputs`, `row`), and their callers in `src/commands/`
- Test: `src/review.rs`; existing fixtures in `tests/review.rs` and `tests/external_docs.rs`

**Interfaces:**
- Consumes: Task 2's fields; Task 3's `--awaiting-user` (to update fixtures).
- Produces:
  - `crate::proposal::pending(task: &Task) -> Option<&str>` and `Defer::date(self) -> time::Date`.
  - `crate::review::ReviewReason` (`Serialize`, lowercase: `Unreviewed | Woken | Stranded | Unassessed`).
  - `crate::review::DepState { Open, Closed { closed: Option<String> } }`.
  - `crate::review::Deps` with:
    - `Deps::of_scan(scan: &[Task], resolve: impl FnMut(&TaskId) -> Result<Option<Task>>) -> Result<Deps>`;
    - `Deps::from_states(states: impl IntoIterator<Item = (TaskId, DepState)>) -> Deps` (tests and fixtures).
  - `crate::review::state(task: &Task, child_count: usize, deps: &Deps, now: OffsetDateTime) -> Option<ReviewReason>`.
  - `commands::list::review_deps(ctx: &ReadCtx, scan: &[Task]) -> Result<Deps>` (`pub(super)`).
  - `TaskSummary::of(task, all, deps: &Deps, claims, registry, now)`; `TaskSummary::review_reason: Option<ReviewReason>`.
  - `ParkedRow::review_reason: Option<ReviewReason>`.
  - `Fields::of_task(task, all, deps: &Deps, claims, registry, now: OffsetDateTime)`.

- [ ] **Step 1: Move the proposal helper.** Create `src/proposal.rs`:

```rust
//! Pending proposals: a curate or scope note awaiting the human's decision. Shared by
//! `sample`'s pool and the review state (wait-aware spec §4.1).

use crate::model::Task;

/// The proposal text of a task whose most recent note is a curate or scope note awaiting
/// the human's decision. Any later note clears it, which is how the human answers.
pub fn pending(task: &Task) -> Option<&str> {
    let text = task.notes.last()?.text.as_str();
    let rest = text
        .strip_prefix("curate:")
        .or_else(|| text.strip_prefix("scope:"))?;
    rest.split_once("proposal:")
        .map(|(_, proposal)| proposal.trim())
}
```

Add `mod proposal;` in `src/main.rs`. In `src/commands/sample.rs`, delete `pending_proposal` and call `crate::proposal::pending(task)` where it was called. In `src/defer.rs`, inside `impl Defer`:

```rust
    /// The calendar date the deferral comes due.
    pub fn date(self) -> Date {
        self.0
    }
```

Run: `just test-one sample`
Expected: PASS (no behaviour change).

- [ ] **Step 2: Write the failing unit tests** in `src/review.rs`'s `mod tests`. They replace `needs_review_follows_spec_section_4`:

```rust
    fn idea(extra: &str, depends: &str) -> Task {
        crate::format::parse_task(
            &format!(
                "---\nid: sci-000001\ntitle: T\nstatus: idea\npriority: 2\n{extra}\
                 created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                 depends: [{depends}]\ntags: []\n---\n"
            ),
            "sci-000001.md",
        )
        .unwrap()
    }

    fn at(text: &str) -> OffsetDateTime {
        crate::time::parse(text).unwrap()
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    fn closed(stamp: &str) -> DepState {
        DepState::Closed { closed: Some(stamp.into()) }
    }

    const REVIEWED: &str = "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\n";
    const NOW: &str = "2026-10-20T00:00:00Z";

    #[test]
    fn state_follows_spec_section_4() {
        use ReviewReason::*;
        let none = Deps::from_states([]);
        let now = at(NOW);
        assert_eq!(state(&idea("", ""), 0, &none, now), Some(Unreviewed));
        assert_eq!(state(&idea(REVIEWED, ""), 0, &none, now), Some(Stranded));
        assert_eq!(state(&idea("reviewed: true\n", ""), 0, &none, now), Some(Stranded), "legacy");
        let marked = format!("{REVIEWED}awaiting_user: true\n");
        assert_eq!(state(&idea(&marked, ""), 0, &none, now), None);
        // Todos keep review-state §4.
        let todo = |extra: &str| {
            crate::format::parse_task(
                &format!(
                    "---\nid: sci-000002\ntitle: T\nstatus: todo\npriority: 2\n{extra}\
                     created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                     depends: []\ntags: []\n---\n"
                ),
                "sci-000002.md",
            )
            .unwrap()
        };
        assert_eq!(state(&todo(""), 0, &none, now), Some(Unassessed));
        assert_eq!(state(&todo("process: direct\n"), 0, &none, now), None);
        assert_eq!(state(&todo(""), 2, &none, now), None, "a goal");
        assert_eq!(state(&todo("lane: true\n"), 0, &none, now), None, "a lane");
    }

    #[test]
    fn dependencies_wait_wake_or_strand() {
        use ReviewReason::*;
        let now = at(NOW);
        let one = |dep: DepState| Deps::from_states([(id("sci-0000aa"), dep)]);
        let task = idea(REVIEWED, "sci-0000aa");
        assert_eq!(state(&task, 0, &one(DepState::Open), now), None);
        assert_eq!(state(&task, 0, &one(closed("2026-10-18T00:00:00Z")), now), Some(Woken));
        assert_eq!(
            state(&task, 0, &one(closed("2026-10-17T09:00:00Z")), now),
            Some(Woken),
            "equal stamps count as woken"
        );
        assert_eq!(state(&task, 0, &one(closed("2026-10-17T08:59:59Z")), now), Some(Stranded));
        assert_eq!(
            state(&task, 0, &one(DepState::Closed { closed: None }), now),
            Some(Stranded),
            "a close from before the stamp existed"
        );
        let legacy = idea("reviewed: true\n", "sci-0000aa");
        assert_eq!(state(&legacy, 0, &one(closed("2026-10-18T00:00:00Z")), now), Some(Stranded));
        // Review Focus 1 and 4: any open edge keeps it waiting, whatever else closed.
        let two = idea(REVIEWED, "sci-0000aa, sci-0000bb");
        let mixed = Deps::from_states([
            (id("sci-0000aa"), closed("2026-10-18T00:00:00Z")),
            (id("sci-0000bb"), DepState::Open),
        ]);
        assert_eq!(state(&two, 0, &mixed, now), None);
    }

    #[test]
    fn deferrals_and_proposals_wait() {
        use ReviewReason::*;
        let none = Deps::from_states([]);
        let now = at(NOW);
        assert_eq!(state(&idea(&format!("{REVIEWED}defer: 2026-11-01\n"), ""), 0, &none, now), None);
        assert_eq!(
            state(&idea(&format!("{REVIEWED}defer: 2026-10-18\n"), ""), 0, &none, now),
            Some(Woken),
            "came due after the review"
        );
        assert_eq!(
            state(&idea(&format!("{REVIEWED}defer: 2026-10-17\n"), ""), 0, &none, now),
            Some(Stranded),
            "due on the review's own date: the review saw it"
        );
        let mut proposed = idea(REVIEWED, "");
        proposed.notes.push(crate::model::Note {
            at: "2026-10-17T09:00:00Z".into(),
            by: "scope".into(),
            text: "scope: drop; landed; proposal: drop it".into(),
            provenance: None,
        });
        assert_eq!(state(&proposed, 0, &none, now), None);
        proposed.notes.push(crate::model::Note {
            at: "2026-10-17T10:00:00Z".into(),
            by: "keith".into(),
            text: "keep it".into(),
            provenance: None,
        });
        assert_eq!(state(&proposed, 0, &none, now), Some(Stranded));
    }

    #[test]
    fn a_view_resolves_only_reviewed_ideas_edges_once_each() {
        let reviewed = idea(REVIEWED, "sci-0000aa");
        let unreviewed = {
            let mut task = idea("", "sci-0000bb");
            task.id = id("sci-000003");
            task
        };
        let mut calls = Vec::new();
        let deps = Deps::of_scan(&[reviewed.clone(), reviewed.clone(), unreviewed], |dep| {
            calls.push(dep.to_string());
            Ok(None)
        })
        .unwrap();
        assert_eq!(calls, ["sci-0000aa"]);
        assert_eq!(
            state(&reviewed, 0, &deps, at(NOW)),
            None,
            "an unresolvable edge counts as open"
        );
    }
```


- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one review::tests`
Expected: FAIL to compile: `state`, `Deps`, `DepState` and `ReviewReason` are not defined.

- [ ] **Step 4: Implement the state** in `src/review.rs`. Delete `needs_review`, and add:

```rust
use crate::model::TaskId;
use std::collections::HashMap;
use time::OffsetDateTime;

/// Why a row needs a look (wait-aware spec §4.2). Absent when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewReason {
    Unreviewed,
    Woken,
    Stranded,
    Unassessed,
}

/// What one dependency edge says once resolved (spec §4.1). An edge that cannot be
/// resolved is `Open`, as `ready` treats it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepState {
    Open,
    Closed { closed: Option<String> },
}

/// The edges of the reviewed ideas in one scan, resolved against that scan (spec §4.3).
/// Built once per scan and never shared: a parked row read from its recorded checkout is
/// judged with that checkout's view, so one id can be open in one view and done in
/// another.
pub struct Deps {
    states: HashMap<TaskId, DepState>,
}

impl Deps {
    /// Resolves every edge of every reviewed idea in `scan`, each distinct id once.
    /// `resolve` follows `moved_to` chains and returns `None` for an unresolvable edge.
    pub fn of_scan(
        scan: &[Task],
        mut resolve: impl FnMut(&TaskId) -> Result<Option<Task>>,
    ) -> Result<Deps> {
        let mut states = HashMap::new();
        for task in scan.iter().filter(|task| task.status == Status::Idea && task.reviewed) {
            for dependency in &task.depends {
                if states.contains_key(dependency) {
                    continue;
                }
                let state = match resolve(dependency)? {
                    Some(found) if !found.status.is_open() => DepState::Closed {
                        closed: found.closed.clone(),
                    },
                    _ => DepState::Open,
                };
                states.insert(dependency.clone(), state);
            }
        }
        Ok(Deps { states })
    }

    pub fn from_states(states: impl IntoIterator<Item = (TaskId, DepState)>) -> Deps {
        Deps {
            states: states.into_iter().collect(),
        }
    }

    fn state(&self, dependency: &TaskId) -> &DepState {
        self.states.get(dependency).unwrap_or_else(|| {
            panic!("{dependency} is not in this view: a row is judged with the view of the scan it came from")
        })
    }
}

/// The derived review state (wait-aware spec §4.2; todos keep review-state spec §4).
pub fn state(
    task: &Task,
    child_count: usize,
    deps: &Deps,
    now: OffsetDateTime,
) -> Option<ReviewReason> {
    match task.status {
        Status::Idea if !task.reviewed => Some(ReviewReason::Unreviewed),
        Status::Idea if waits(task, deps, now) => None,
        Status::Idea if woken(task, deps) => Some(ReviewReason::Woken),
        Status::Idea => Some(ReviewReason::Stranded),
        Status::Todo | Status::Blocked
            if task.process.is_none() && !task.lane && child_count == 0 =>
        {
            Some(ReviewReason::Unassessed)
        }
        _ => None,
    }
}

/// Spec §4.1: a recorded wait.
fn waits(task: &Task, deps: &Deps, now: OffsetDateTime) -> bool {
    task.awaiting_user
        || crate::defer::is_deferred(task, now)
        || crate::proposal::pending(task).is_some()
        || task
            .depends
            .iter()
            .any(|dependency| *deps.state(dependency) == DepState::Open)
}

/// Spec §4.2: new evidence since the review. Called only when nothing waits, so a
/// deferral here has come due. Equal stamps count as woken; the deferral test is strict.
fn woken(task: &Task, deps: &Deps) -> bool {
    let Some(reviewed_at) = task.reviewed_at.as_deref().map(stamp) else {
        return false;
    };
    let closed_since = task.depends.iter().any(|dependency| {
        matches!(deps.state(dependency), DepState::Closed { closed: Some(closed) } if stamp(closed) >= reviewed_at)
    });
    closed_since || task.defer.is_some_and(|defer| defer.date() > reviewed_at.date())
}

/// Record stamps were validated by `parse_task`.
fn stamp(text: &str) -> OffsetDateTime {
    crate::time::parse(text).expect("record stamps are validated at parse")
}
```

The crate does not build again until Step 5 threads the callers of the deleted `needs_review`; the unit tests run in Step 6.

- [ ] **Step 5: Thread the views**

`src/commands/list.rs`, beside `resolve_dependency`:

```rust
/// The dependency view for rows of `scan` (wait-aware spec §4.3): one per scan, never
/// shared, so each row is judged against the copies its own checkout holds.
pub(super) fn review_deps(ctx: &ReadCtx, scan: &[Task]) -> Result<crate::review::Deps> {
    crate::review::Deps::of_scan(scan, |id| resolve_dependency(ctx, scan, id))
}
```

`src/output.rs`, `TaskSummary`:
- add, after `needs_review`:

```rust
    /// Why the row needs a look (wait-aware spec §4.2); present exactly when
    /// `needs_review` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_reason: Option<crate::review::ReviewReason>,
```

- `TaskSummary::of` gains `deps: &crate::review::Deps` after `all`, and computes:

```rust
        let review_reason = crate::review::state(task, child_count, deps, now);
```

  setting `needs_review: review_reason.is_some(), review_reason,`.
- `ParkedRow` gains the same `review_reason` field (with `skip_serializing_if`): `resolved` copies `summary.review_reason`, and `unresolved` sets `None`.

`src/filter.rs`, `Fields::of_task` gains `deps: &crate::review::Deps` after `all`, and `now: OffsetDateTime` last. It computes `needs_review: crate::review::state(task, crate::hierarchy::children(all, &task.id, registry).len(), deps, now).is_some()`.

Call sites. Each builds or receives the view of the scan whose slice it passes as `all`:
- `list::list`: after `scan_with_claims`, `let deps = review_deps(&ctx, &all)?;`. Pass `&deps` to `Fields::of_task` (with `now`) and to `TaskSummary::of`.
- `list.rs` near line 229 (the ready pick) and line 536: build `review_deps(ctx, all)?` once per function from the scan they hold.
- `prime` (near lines 884–901): one `review_deps(&ctx, &all)?` for `ready`, `closeout`, the roadmap and the lanes. For `doing`, build one view per `elsewhere` scan before the closure, `let elsewhere_deps: Vec<_> = elsewhere.iter().map(|(_, scan)| review_deps(&ctx, scan)).collect::<Result<_>>()?;`, and pick the view at the same index as the matching scan, or `&deps` for `all`.
- `src/commands/sample.rs:97`: `review_deps(&ctx, &all)?`.
- `src/hierarchy.rs`: `forest` and `node` take `deps: &crate::review::Deps` and pass it down; each caller in `src/commands/` passes the view of the `all` it passes.
- `src/lanes.rs`: `Inputs` gains `pub deps: &'a crate::review::Deps`, and `row`'s `summary` closure passes `inputs.deps`; each constructor of `Inputs` builds it from `inputs.all`.
- `src/commands/parked.rs`:
  - `sourced_rows` builds `let deps = super::list::review_deps(ctx, all)?;` before the loop, and `resolved_row(task, all, &deps, …)` passes it.
  - `resolve_recorded` gains `ctx: &ReadCtx` and builds `super::list::review_deps(ctx, &scan)?` from the recorded checkout's own scan. `recorded_or_fallback` passes `ctx`.
  - The filter near line 235 passes the view of the `all` it holds, and `now`.

Do not change which copy a parked row shows: `Prefer` and `recorded_is_scanned` stay as they are.

- [ ] **Step 6: Build, run the unit tests, then the fast suite**

Run: `just test-one review::tests`
Expected: PASS for the four new tests.

Run: `just test-fast`
Expected: the build succeeds. These tests now fail because a reviewed idea with no recorded wait is `stranded`:
- `tests/review.rs`: `needs_review_marks_exactly_spec_section_4_in_json_and_the_filter` (through `mixed_project`), `parked_rows_carry_needs_review_and_filter_on_it`, and any other test that expects a reviewed idea without a wait to need no review;
- `tests/external_docs.rs`: `external_doc_plan_filter_composes_with_needs_review`.

Any other failure is a threading bug: fix it, not the test.

- [ ] **Step 7: Give those fixtures a recorded wait**

These tests mean "a reviewed idea that waits", so record the wait the new rule reads. Change each `note … --reviewed "scope: briefed"` (or `"reviewed"`) in those fixtures to `note … --reviewed --awaiting-user "scope: question"`. Keep every assertion as it is. In `mixed_project`, the briefed idea stays in `unmarked`.

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src tests
git commit -m "feat(review): derive review_reason from recorded waits on per-scan dependency views"
```

---

### Task 5: Row fields and view-level acceptance (complexity: mid)

**Files:**
- Modify: `src/output.rs` (`TaskSummary` fields `awaiting_user`, `reviewed_at`, `closed`)
- Test: `tests/review.rs` (spec §8 cases 1–8, 12, 13; Review Focus 2 and 3), `tests/cli.rs` (case 16)

**Interfaces:**
- Consumes: Task 4's `review_reason`; Task 3's writers; Task 2's fields.
- Produces: `TaskSummary::awaiting_user: bool` (always serialized), `TaskSummary::reviewed_at: Option<String>`, `TaskSummary::closed: Option<String>` (each serialized only when set).

- [ ] **Step 1: Write the failing tests** in `tests/review.rs`. Add a helper:

```rust
/// `(needs_review, review_reason)` of one row of `list` JSON, or `None` when not listed.
fn reason(env: &TestEnv, dir: &Path, args: &[&str], id: &str) -> Option<(bool, Option<String>)> {
    env.json(dir, args)["tasks"].as_array().unwrap().iter().find(|row| row["id"] == id).map(|row| {
        (
            row["needs_review"].as_bool().unwrap(),
            row.get("review_reason").map(|r| r.as_str().unwrap().to_string()),
        )
    })
}

fn queued(env: &TestEnv, dir: &Path, id: &str) -> Option<String> {
    reason(env, dir, &["list", "--needs-review"], id).map(|(_, why)| why.unwrap())
}
```

The tests:

```rust
#[test]
fn an_edge_wakes_its_idea_on_done_and_on_drop() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for close in [["done"].as_slice(), ["drop"].as_slice()] {
        let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
        let follow_up = id_of(env.json(&dir, &["add", "F"]));
        env.json(&dir, &["dep", &idea, "--on", &follow_up]);
        env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed"]);
        assert_eq!(queued(&env, &dir, &idea), None);
        let before = env.read(&dir, &format!("tasks/{idea}.md"));
        let mut args = close.to_vec();
        args.push(follow_up.as_str());
        env.json(&dir, &args);
        assert_eq!(queued(&env, &dir, &idea).as_deref(), Some("woken"));
        assert_eq!(env.read(&dir, &format!("tasks/{idea}.md")), before, "no write touched the idea");
    }
}

#[test]
fn a_moved_dependency_is_followed_to_its_successor() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let other = env.init("wali");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F"]));
    env.json(&dir, &["dep", &idea, "--on", &follow_up]);
    env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed"]);
    let moved = id_of(env.json(&dir, &["move", &follow_up, "--to", "wali"]));
    assert_eq!(queued(&env, &dir, &idea), None, "the successor is still open");
    env.json(&other, &["done", &moved]);
    assert_eq!(queued(&env, &dir, &idea).as_deref(), Some("woken"));
}

#[test]
fn an_unreachable_or_shelved_dependency_keeps_the_idea_waiting() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed"]);
    restamp(&dir, &idea, "depends", "[zzz-abcdef]");
    // The warning covers listed rows only, so read it from the unfiltered list.
    let out = env.json(&dir, &["list"]);
    assert!(out["warnings"].to_string().contains("zzz-abcdef is unreachable"), "{out}");
    assert_eq!(queued(&env, &dir, &idea), None);
    // Review Focus 2: a shelf is open.
    let shelved_wait = id_of(env.json(&dir, &["add", "S", "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F"]));
    env.json(&dir, &["dep", &shelved_wait, "--on", &follow_up]);
    env.json(&dir, &["note", &shelved_wait, "--reviewed", "scope: briefed"]);
    env.json(&dir, &["shelve", &follow_up, "later"]);
    assert_eq!(queued(&env, &dir, &shelved_wait), None);
}

#[test]
fn stranded_ideas_and_the_review_time_test() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let bare = id_of(env.json(&dir, &["add", "B", "--status", "idea"]));
    env.json(&dir, &["note", &bare, "--reviewed", "scope: briefed"]);
    assert_eq!(queued(&env, &dir, &bare).as_deref(), Some("stranded"));
    // Legacy: reviewed with no stamp, its edge closed since: still stranded.
    let legacy = id_of(env.json(&dir, &["add", "L", "--status", "idea"]));
    let done = id_of(env.json(&dir, &["add", "D"]));
    env.json(&dir, &["dep", &legacy, "--on", &done]);
    seed_reviewed(&dir, &legacy);
    env.json(&dir, &["done", &done]);
    assert_eq!(queued(&env, &dir, &legacy).as_deref(), Some("stranded"));
    // Closed before the review: stranded; a woken idea re-reviewed: stranded.
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F"]));
    env.json(&dir, &["dep", &idea, "--on", &follow_up]);
    env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed"]);
    env.json(&dir, &["done", &follow_up]);
    restamp(&dir, &follow_up, "closed", "2026-01-02T00:00:00Z");
    restamp(&dir, &idea, "reviewed_at", "2026-01-01T00:00:00Z");
    assert_eq!(queued(&env, &dir, &idea).as_deref(), Some("woken"));
    env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed; follow-up read"]);
    assert_eq!(queued(&env, &dir, &idea).as_deref(), Some("stranded"));
}

#[test]
fn proposals_users_and_deferrals_are_waits() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let proposed = id_of(env.json(&dir, &["add", "P", "--status", "idea"]));
    env.json(&dir, &["note", &proposed, "--reviewed", "scope: drop; landed; proposal: drop it"]);
    assert_eq!(queued(&env, &dir, &proposed), None);
    env.json(&dir, &["note", &proposed, "keep it"]);
    assert_eq!(queued(&env, &dir, &proposed).as_deref(), Some("stranded"));
    let asked = id_of(env.json(&dir, &["add", "Q", "--status", "idea"]));
    env.json(&dir, &["note", &asked, "--reviewed", "--awaiting-user", "scope: question"]);
    assert_eq!(queued(&env, &dir, &asked), None);
    env.json(&dir, &["note", &asked, "--no-reviewed", "answer"]);
    assert_eq!(queued(&env, &dir, &asked).as_deref(), Some("unreviewed"));
    let deferred = id_of(env.json(&dir, &["add", "D", "--status", "idea"]));
    seed(&dir, &deferred, "reviewed: true\nreviewed_at: 2026-01-01T12:00:00Z\ndefer: 2099-01-01");
    assert_eq!(queued(&env, &dir, &deferred), None);
    restamp(&dir, &deferred, "defer", "2026-01-02");
    assert_eq!(queued(&env, &dir, &deferred).as_deref(), Some("woken"));
    restamp(&dir, &deferred, "defer", "2026-01-01");
    assert_eq!(queued(&env, &dir, &deferred).as_deref(), Some("stranded"));
}

#[test]
fn a_dependency_in_another_project_wakes_the_idea_from_either_scope() {
    // Review Focus 3.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let other = env.init("wali");
    let follow_up = id_of(env.json(&other, &["add", "F"]));
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["dep", &idea, "--on", &follow_up]);
    env.json(&dir, &["note", &idea, "--reviewed", "scope: briefed"]);
    assert_eq!(queued(&env, &dir, &idea), None);
    env.json(&other, &["done", &follow_up]);
    assert_eq!(queued(&env, &dir, &idea).as_deref(), Some("woken"));
    assert_eq!(
        reason(&env, &dir, &["list", "--all-projects", "--needs-review"], &idea)
            .and_then(|(_, why)| why)
            .as_deref(),
        Some("woken")
    );
}

#[test]
fn every_view_agrees_on_review_state() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let waiting = id_of(env.json(&dir, &["add", "W", "--status", "idea"]));
    env.json(&dir, &["note", &waiting, "--reviewed", "--awaiting-user", "scope: question"]);
    let stranded = id_of(env.json(&dir, &["add", "S", "--status", "idea"]));
    env.json(&dir, &["note", &stranded, "--reviewed", "scope: briefed"]);
    let woken = id_of(env.json(&dir, &["add", "K", "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F", "--process", "direct"]));
    env.json(&dir, &["dep", &woken, "--on", &follow_up]);
    env.json(&dir, &["note", &woken, "--reviewed", "scope: briefed"]);
    env.json(&dir, &["done", &follow_up]);
    let fresh = id_of(env.json(&dir, &["add", "N", "--status", "idea"]));
    let marked = [stranded.clone(), woken.clone(), fresh.clone()];
    // JSON rows: needs_review and review_reason agree, the reason is absent exactly when false.
    let list = env.json(&dir, &["list"]);
    for row in list["tasks"].as_array().unwrap() {
        let needs = row["needs_review"].as_bool().unwrap();
        assert_eq!(needs, row.get("review_reason").is_some(), "{row}");
        assert!(row["awaiting_user"].is_boolean(), "{row}");
        let id = row["id"].as_str().unwrap().to_string();
        assert_eq!(needs, marked.contains(&id), "{row}");
    }
    // The filter, the pretty mark and tree return the same set.
    let mut filtered: Vec<String> = env.json(&dir, &["list", "--needs-review"])["tasks"]
        .as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect();
    filtered.sort();
    let mut want = marked.to_vec();
    want.sort();
    assert_eq!(filtered, want);
    let pretty = env.pretty(&dir, &["list"]);
    for line in pretty.lines() {
        for id in [&waiting, &stranded, &woken, &fresh] {
            if line.contains(id.as_str()) {
                assert_eq!(line.trim_start().starts_with('*'), marked.contains(id), "{line}");
            }
        }
    }
    let tree = env.json(&dir, &["tree"]).to_string();
    for id in &marked {
        assert!(tree.contains(id.as_str()), "{tree}");
    }
    assert_eq!(env.json(&dir, &["show", &waiting])["task"]["awaiting_user"], true);
    assert_eq!(env.json(&dir, &["show", &fresh])["task"]["awaiting_user"], false);
}
```

`list` rows carry `awaiting_user` only once Step 3 adds it; that assertion is this task's RED.

In `tests/cli.rs` (which has `repo_with_worktree`):

```rust
/// A park entry for `id` naming `worktree`, written straight into the claim store.
fn park_in(env: &TestEnv, prefix: &str, id: &str, worktree: &std::path::Path) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[parks.\"{id}\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\n\
         worktree = \"{}\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"next\"\n\
         waiting_on = \"user\"\ntitle = \"T\"\n",
        worktree.display()
    ));
    std::fs::write(&path, text).unwrap();
}

/// Sets `reviewed_at` on a record that `note --reviewed` already stamped.
fn backdate_review(dir: &std::path::Path, id: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text.find("\nreviewed_at: ").unwrap() + "\nreviewed_at: ".len();
    let end = start + text[start..].find('\n').unwrap();
    std::fs::write(&path, format!("{}2026-01-01T00:00:00Z{}", &text[..start], &text[end..])).unwrap();
}

#[test]
fn parked_review_state_follows_the_copy_each_row_shows() {
    let mut env = TestEnv::new();
    let (main, side, dependency) = repo_with_worktree(&mut env);
    // A exists in both checkouts; the registered copy wins.
    let a = id_of(env.json(&main, &["add", "A", "--status", "idea"]));
    env.json(&main, &["dep", &a, "--on", &dependency]);
    env.json(&main, &["note", &a, "--reviewed", "scope: briefed"]);
    backdate_review(&main, &a);
    std::fs::copy(main.join(format!("tasks/{a}.md")), side.join(format!("tasks/{a}.md"))).unwrap();
    // B exists only in the worktree.
    let b = id_of(env.json(&side, &["add", "B", "--status", "idea"]));
    env.json(&side, &["dep", &b, "--on", &dependency]);
    env.json(&side, &["note", &b, "--reviewed", "scope: briefed"]);
    backdate_review(&side, &b);
    // The dependency is done in the worktree's copy only.
    env.json(&side, &["done", &dependency]);
    park_in(&env, "sci", &a, &side);
    park_in(&env, "sci", &b, &side);
    for args in [["list", "--parked"].as_slice(), ["prime"].as_slice()] {
        let out = env.json(&main, args).to_string();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let rows = if args[0] == "prime" { &parsed["parked"] } else { &parsed["tasks"] };
        let row = |id: &str| {
            rows.as_array().unwrap().iter().find(|r| r["id"] == id).cloned()
                .unwrap_or_else(|| panic!("{id} missing: {out}"))
        };
        assert_eq!(row(&a)["needs_review"], false, "A follows main's open copy: {out}");
        assert_eq!(row(&b)["review_reason"], "woken", "B follows the worktree's done copy: {out}");
    }
    let queue = env.json(&main, &["list", "--needs-review"]).to_string();
    assert!(!queue.contains(&a), "{queue}");
}
```

If `done` in the worktree is refused, a `stale_copy` here means the fixture's two copies drifted: commit in `main` before creating the worktree's copies (as `repo_with_worktree` does for its seed task) and say so in a note.

- [ ] **Step 2: Run them to verify which fail**

Run: `just test-one --test review woken`, `just test-one --test review waits`, `just test-one --test review stranded`, `just test-one --test review agrees`, `just test-one --test review either_scope`, `just test-one --test cli parked_review_state_follows`
Expected: `every_view_agrees_on_review_state` FAILS on `awaiting_user` missing from `list` rows. The others PASS already on Task 4's logic. That is expected: they pin the views, and a failure in them is a Task 4 threading bug to fix before continuing.

- [ ] **Step 3: Add the row fields** in `src/output.rs`, `TaskSummary`, after `reviewed`:

```rust
    /// The stored wait-on-user marker; always present, like `reviewed`.
    pub awaiting_user: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
```

and after `completed`:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
```

In `TaskSummary::of`: `awaiting_user: task.awaiting_user, reviewed_at: task.reviewed_at.clone(), closed: task.closed.clone(),`.

- [ ] **Step 4: Run them to verify they pass**

Run the six commands from Step 2.
Expected: PASS.

- [ ] **Step 5: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src tests
git commit -m "feat(review): carry the marker and stamps on rows; pin review state across views"
```

---

### Task 6: Skills, README and spec status (complexity: low)

**Files:**
- Modify: `skills/scope/SKILL.md` (§1 selection, §3 verdicts, §5 "Ideas it wakes")
- Modify: `skills/curate/SKILL.md` ("Pending proposals")
- Modify: `skills/tasks/SKILL.md` (the `--needs-review` paragraph near line 58)
- Modify: `README.md` (the `reviewed` paragraph near line 28)
- Modify: `docs/specs/2026-10-17-wait-aware-review-design.md` (status line), `docs/specs/2026-10-11-review-state-design.md` (amendment line)

**Interfaces:**
- Consumes: the behaviour from Tasks 2–5. No code.

- [ ] **Step 1: scope §1.** Replace the paragraph that begins "It returns the scope queue" through "re-scope (see …)" with:

```markdown
It returns the scope queue, the rows pretty tables mark `*`. Each row's `review_reason`
says why it is there: `unreviewed` (no pass has looked, or a finding readmitted it),
`woken` (a dependency closed, or a deferral came due, after the last review),
`stranded` (reviewed, but waiting on nothing recorded and with nothing new), or
`unassessed` (a leaf `todo` or `blocked` task without process). Draw the batch from
`unreviewed` and `woken` ideas first; when none is eligible, from `stranded` ideas; only
then from `unassessed` rows, each a re-scope (see "Re-scope a scoped task").
```

Replace the paragraph beginning "An idea leaves the queue when a pass writes its audit note" with:

```markdown
A reviewed idea stays out of the queue only while it waits on something recorded: an
open dependency, a pending `proposal:`, the user (`awaiting_user`), or a deferral not yet
due. It comes back by itself when that wait ends, or at once when it has none.
Incidental notes (curate, comments) neither readmit it nor change its wait. A todo or
blocked row leaves when a re-scope sets its process. An explicit id bypasses only this
review-state exclusion; it must still be an unclaimed idea at revalidation, or an
unclaimed `todo` or `blocked` task, which makes it a re-scope, and the pass still has the
three-document ceiling.
```

- [ ] **Step 2: scope §3.** In the verdict table:
- the `briefed` row's Required result ends: "make it depend on each follow-up that wakes it (§5). A brief that leaves nothing to wait on is not `briefed`: use `question` or `shelved`."
- the `question` row's Required result ends: "write the audit note with `--reviewed --awaiting-user`".

Below the audit-note command block, add:

```markdown
`question` adds `--awaiting-user` to that command. For a woken or stranded idea, gather
the closed follow-ups' outcomes as evidence (§2) before choosing; the new audit note
re-stamps the review, so edges closed earlier stop counting as new. Whoever answers a
question or declines a proposal records it with `tasks note <id> --no-reviewed
"<answer>"`, which readmits the idea with the answer.
```

- [ ] **Step 3: scope §5.** Replace the sentence "It does not readmit the idea, so the wake note stays." with: "When the follow-up closes, the edge readmits the idea by itself; the wake note still carries the finding to it."

- [ ] **Step 4: curate.** In "Pending proposals", replace "the human records the decision with `tasks note <id> "<decision>"` whether they acted on the proposal or declined it" with "the human records the decision with `tasks note <id> "<decision>"` whether they acted on the proposal or declined it; on an idea, a declined proposal is answered with `--no-reviewed` so scope looks again".

- [ ] **Step 5: tasks skill.** Replace the `--needs-review` paragraph with:

```markdown
   `--needs-review` keeps rows that still need a look, each with a `review_reason`:
   `unreviewed` ideas, `woken` and `stranded` reviewed ideas (no recorded wait: open
   dependency, pending proposal, `awaiting_user`, or future deferral), and `unassessed`
   leaf todo or blocked tasks with no process. Pretty tables mark those rows with a
   leading `*`; together they are the scope queue, which `/scope` with no ids draws from.
   `note --reviewed --awaiting-user` records that a reviewed idea waits on the user;
   `--no-reviewed` readmits it with the answer.
```

- [ ] **Step 6: README.** Replace the `reviewed` paragraph with:

```markdown
Ideas carry an optional `reviewed` flag (set by `note --reviewed` and `edit --reviewed`,
which also stamp `reviewed_at`; cleared by `--no-reviewed` and by every status change),
and an `awaiting_user` marker for a reviewed idea waiting on the user. Every close to
done or dropped stamps `closed`. Summary rows carry a derived `needs_review` and its
`review_reason` (`unreviewed`, `woken`, `stranded`, `unassessed`): a reviewed idea needs
review again when it waits on nothing recorded. `--needs-review` filters on it and
pretty tables mark it with `*`. Older binaries cannot parse records carrying `reviewed`,
`awaiting_user`, `reviewed_at` or `closed`: update every host reading synced tasks
before any of them runs this version, since every `done` and `drop` writes `closed`.
```

- [ ] **Step 7: Spec status lines.**
- In `docs/specs/2026-10-17-wait-aware-review-design.md`, replace the `Status:` line with `Status: implemented (plan docs/plans/2026-10-17-wait-aware-review.md)`; the Rollout appends its completion.
- In `docs/specs/2026-10-11-review-state-design.md`, add below its `Amended 2026-10-14` paragraph: `Amended 2026-10-17 (tasks-1a3280): reviewed ideas that wait on nothing recorded return to the queue; see docs/specs/2026-10-17-wait-aware-review-design.md, which supersedes §1's "without reading notes" for the proposal test and §4's idea rule.`

- [ ] **Step 8: Check and commit**

Run: `just check`
Expected: PASS (hygiene, fmt, clippy, `tasks check`).

```bash
git add skills README.md docs/specs
git commit -m "docs(review): wait-aware scope queue in skills, README and specs"
```

---

### Task 7: The backfill tool (complexity: mid)

**Files:**
- Create: `tools/wait-backfill` (Python 3, executable)
- Test: create `tests/wait_backfill.rs`

**Interfaces:**
- Consumes: the CLI from Tasks 2–5 (`list --needs-review` rows with `review_reason`; `show`; `dep --on`; `edit --awaiting-user`; `claims`).
- Produces: `tools/wait-backfill --state <dir> [--root <checkout>] [--tasks <bin>] (--freeze-only | --writers-paused)`, which prints a JSON report and exits 0 when clean, 1 on `failed`/`uncommitted`, and 2 on a usage or snapshot error.

The snapshot is `<state>/<prefix>.json`: `{"prefix", "frozen_at", "entries": [...]}`. Each entry is `{"id", "updated", "class", "named_open", "evidence", "decision", "done", "outcome"}`:
- `class` is `"auto"` or `"judge"`.
- `named_open` is a list of ids.
- `evidence` is the latest `scope:` note's text, or `null`.
- `decision` is `null`, `"leave"`, `"awaiting_user"`, or `{"dep": [ids]}`.
- `done` is a bool, and `outcome` is a string or `null`.

- [ ] **Step 1: Write the failing tests** in `tests/wait_backfill.rs`. Copy `id_of`, `tool`, `backfill` and `backfill_with` verbatim from `tests/review_backfill.rs`, changing only `tool()`'s path to `tools/wait-backfill`. Do not copy `updated` or `reviewed`: unused helpers fail `clippy --all-targets -D warnings`.

```rust
mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

// id_of, tool, backfill, backfill_with: copied from tests/review_backfill.rs (see above).

fn snapshot(state: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(state.join("sci.json")).unwrap()).unwrap()
}

fn decide(state: &Path, id: &str, decision: serde_json::Value) {
    let mut snap = snapshot(state);
    for entry in snap["entries"].as_array_mut().unwrap() {
        if entry["id"] == id {
            entry["decision"] = decision.clone();
        }
    }
    std::fs::write(state.join("sci.json"), serde_json::to_string_pretty(&snap).unwrap()).unwrap();
}

struct Fixture {
    dir: PathBuf,
    one: String,      // names exactly one open task: auto edge
    follow_up: String,
    several: String,  // names two open tasks: judge
    none: String,     // names nothing: judge
    closed: String,   // names only a closed task: judge
    waiting: String,  // has an open edge: not stranded, not a candidate
}

fn fixture(env: &mut TestEnv) -> Fixture {
    let dir = env.init("sci");
    let add_idea = |env: &TestEnv, title: &str| id_of(env.json(&dir, &["add", title, "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F"]));
    let second = id_of(env.json(&dir, &["add", "G"]));
    let finished = id_of(env.json(&dir, &["add", "H"]));
    env.json(&dir, &["done", &finished]);
    let one = add_idea(env, "One");
    env.json(&dir, &["note", &one, "--reviewed", &format!("scope: briefed; waits on {follow_up}")]);
    let several = add_idea(env, "Several");
    env.json(&dir, &["note", &several, "--reviewed", &format!("scope: briefed; see {follow_up} and {second}")]);
    let none = add_idea(env, "None");
    env.json(&dir, &["note", &none, "--reviewed", "scope: question; once Q1 is answered"]);
    let closed = add_idea(env, "Closed");
    env.json(&dir, &["note", &closed, "--reviewed", &format!("scope: briefed; waits on {finished}")]);
    let waiting = add_idea(env, "Waiting");
    env.json(&dir, &["dep", &waiting, "--on", &second]);
    env.json(&dir, &["note", &waiting, "--reviewed", "scope: briefed"]);
    Fixture { dir, one, follow_up, several, none, closed, waiting }
}

#[test]
fn freeze_classifies_stranded_ideas_by_their_latest_scope_note() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    let snap = snapshot(state.path());
    let entry = |id: &str| snap["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).cloned();
    let one = entry(&f.one).unwrap();
    assert_eq!(one["class"], "auto");
    assert_eq!(one["decision"], serde_json::json!({"dep": [f.follow_up]}));
    for id in [&f.several, &f.none, &f.closed] {
        let e = entry(id).unwrap();
        assert_eq!(e["class"], "judge", "{e}");
        assert!(e["decision"].is_null(), "{e}");
    }
    assert!(entry(&f.waiting).is_none(), "an idea with an open edge is not stranded");
}

#[test]
fn apply_runs_decisions_only_on_unchanged_records() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    decide(state.path(), &f.none, serde_json::json!("awaiting_user"));
    decide(state.path(), &f.closed, serde_json::json!("leave"));
    // `several` stays undecided. Before the apply, a scope pass re-reviews `several`
    // and gives it an edge: its frozen assumptions no longer hold.
    env.json(&f.dir, &["dep", &f.several, "--on", &f.follow_up]);
    env.json(&f.dir, &["note", &f.several, "--reviewed", "scope: briefed; new research"]);
    decide(state.path(), &f.several, serde_json::json!("awaiting_user"));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    let show = |id: &str| env.json(&f.dir, &["show", id])["task"].clone();
    assert!(show(&f.one)["depends"].to_string().contains(&f.follow_up));
    assert_eq!(show(&f.none)["awaiting_user"], true);
    assert_eq!(show(&f.several)["awaiting_user"], false, "changed since the freeze: no marker");
    assert!(report["changed"].to_string().contains(&f.several), "{report}");
    assert!(report["left"].to_string().contains(&f.closed), "{report}");
    // The re-reviewed idea wakes when its new dependency closes.
    env.json(&f.dir, &["done", &f.follow_up]);
    let queue = env.json(&f.dir, &["list", "--needs-review"]).to_string();
    assert!(queue.contains(&f.several), "{queue}");
}

#[test]
fn rerun_rewrites_nothing_and_reads_no_notes() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    decide(state.path(), &f.none, serde_json::json!("awaiting_user"));
    backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    let before: Vec<String> = [&f.one, &f.none, &f.several, &f.closed]
        .iter().map(|id| env.read(&f.dir, &format!("tasks/{id}.md"))).collect();
    // A later scope note would change the classification if notes were re-read.
    env.json(&f.dir, &["note", &f.closed, "scope: briefed; nothing to wait on"]);
    let closed_after = env.read(&f.dir, &format!("tasks/{}.md", f.closed));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    for (id, text) in [&f.one, &f.none, &f.several].iter().zip(&before) {
        assert_eq!(&env.read(&f.dir, &format!("tasks/{id}.md")), text);
    }
    assert_eq!(env.read(&f.dir, &format!("tasks/{}.md", f.closed)), closed_after);
}

#[test]
fn an_interrupted_pass_reports_the_entry_changed_and_writes_nothing() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    // Simulate a pass that ran the auto edge and died before marking the entry done.
    env.json(&f.dir, &["dep", &f.one, "--on", &f.follow_up]);
    let before = env.read(&f.dir, &format!("tasks/{}.md", f.one));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["changed"].to_string().contains(&f.one), "{report}");
    assert_eq!(env.read(&f.dir, &format!("tasks/{}.md", f.one)), before);
}

#[test]
fn a_live_claim_refuses_the_pass_and_a_later_pass_applies() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    let other = id_of(env.json(&f.dir, &["add", "Busy"]));
    env.cmd(&f.dir)
        .env("TASKS_SESSION", "agent-x")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", &other])
        .assert()
        .success();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    assert!(!env.json(&f.dir, &["show", &f.one])["task"]["depends"].to_string().contains(&f.follow_up));
    env.cmd(&f.dir)
        .env("TASKS_SESSION", "agent-x")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["park", &other, "later"])
        .assert()
        .success();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(env.json(&f.dir, &["show", &f.one])["task"]["depends"].to_string().contains(&f.follow_up));
}

#[test]
fn a_corrupt_snapshot_refuses_before_any_write() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("sci.json"), "{not json").unwrap();
    let before = env.read(&f.dir, &format!("tasks/{}.md", f.one));
    let (code, _) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 2);
    assert_eq!(env.read(&f.dir, &format!("tasks/{}.md", f.one)), before);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test wait_backfill`
Expected: FAIL: `tools/wait-backfill` does not exist (the report is `null`).

- [ ] **Step 3: Write the tool.** Create `tools/wait-backfill` and `chmod +x` it. It reuses `review-backfill`'s `run`, `read_json`, `error_kind`, `save` and `live_claims` verbatim (copy them; the tools are one-time and independent):

```python
#!/usr/bin/env python3
"""One-time backfill of recorded waits (docs/specs/2026-10-17-wait-aware-review-design.md §7.3).

The first run on a project freezes its candidates: open ideas whose review_reason is
`stranded`, each with its record's `updated` stamp, the open task ids its latest `scope:`
note names, and a class. `auto` (exactly one open id named) carries the decision
{"dep": [id]}; `judge` carries none until the operator writes one into the snapshot:
"leave", "awaiting_user", or {"dep": [ids]}. Every apply pass runs only decided entries
whose record is still a reviewed idea with an unchanged stamp, and reports the rest.
Notes are read only at the freeze.

Run every apply pass inside a writer pause: no other session may write the project's
task records on any host (spec §7.3). --writers-paused is the operator's confirmation;
the tool also refuses while this host's claim store holds a live claim in the project.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from datetime import datetime, timezone

ENTRY_KEYS = {"id", "updated", "class", "named_open", "evidence", "decision", "done", "outcome"}


class Usage(Exception):
    pass


# run, read_json, error_kind, save, live_claims: copied verbatim from tools/review-backfill,
# with save's temp-file prefix changed to ".wait-backfill-".


def latest_scope_note(task):
    for note in reversed(task.get("notes") or []):
        if note["text"].startswith("scope:"):
            return note["text"]
    return None


def named_open(tasks, root, prefix_pattern, idea_id, text):
    """Open tasks the note names, resolved through the CLI (which follows moves)."""
    found = []
    for named in dict.fromkeys(re.findall(prefix_pattern, text or "")):
        if named == idea_id:
            continue
        code, stdout, _ = run(tasks, root, "show", named)
        if code != 0:
            continue
        task = json.loads(stdout)["task"]
        if task["status"] not in ("done", "dropped", "moved"):
            found.append(task["id"])
    return found


def freeze(tasks, root, prefix, path):
    ids = r"\b[a-z][a-z0-9-]*-[0-9a-f]{6}\b"
    entries = []
    rows = read_json(tasks, root, "list", "--status", "idea", "--needs-review")["tasks"]
    for row in rows:
        if row.get("review_reason") != "stranded":
            continue
        task = read_json(tasks, root, "show", row["id"])["task"]
        evidence = latest_scope_note(task)
        open_ids = named_open(tasks, root, ids, task["id"], evidence)
        auto = len(open_ids) == 1
        entries.append({
            "id": task["id"],
            "updated": task["updated"],
            "class": "auto" if auto else "judge",
            "named_open": open_ids,
            "evidence": evidence,
            "decision": {"dep": open_ids} if auto else None,
            "done": False,
            "outcome": None,
        })
    snapshot = {
        "prefix": prefix,
        "frozen_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "entries": entries,
    }
    save(path, snapshot)
    return snapshot


def valid_decision(decision):
    if decision in (None, "leave", "awaiting_user"):
        return True
    return (
        isinstance(decision, dict)
        and set(decision) == {"dep"}
        and isinstance(decision["dep"], list)
        and decision["dep"]
        and all(isinstance(i, str) for i in decision["dep"])
    )


def load(path, prefix):
    """The whole snapshot, validated before anything is applied (no partial runs)."""
    def bad(why):
        return Usage(f"{path} is not a valid wait-backfill snapshot for {prefix}: {why}; inspect it before rerunning")
    try:
        with open(path) as handle:
            snapshot = json.load(handle)
    except json.JSONDecodeError as error:
        raise bad(f"invalid JSON ({error})")
    if not isinstance(snapshot, dict) or snapshot.get("prefix") != prefix:
        raise bad(f"prefix is {snapshot.get('prefix') if isinstance(snapshot, dict) else None!r}")
    if not isinstance(snapshot.get("frozen_at"), str) or not isinstance(snapshot.get("entries"), list):
        raise bad("frozen_at or entries missing")
    for index, entry in enumerate(snapshot["entries"]):
        ok = (
            isinstance(entry, dict)
            and set(entry) == ENTRY_KEYS
            and isinstance(entry["id"], str)
            and re.fullmatch(re.escape(prefix) + r"-[0-9a-f]{6}", entry["id"])
            and isinstance(entry["updated"], str)
            and entry["class"] in ("auto", "judge")
            and isinstance(entry["named_open"], list)
            and valid_decision(entry["decision"])
            and isinstance(entry["done"], bool)
            and (entry["outcome"] is None or isinstance(entry["outcome"], str))
        )
        if not ok:
            raise bad(f"entry {index} is {entry!r}")
    return snapshot


def command_for(entry):
    decision = entry["decision"]
    if decision == "awaiting_user":
        return ["edit", entry["id"], "--awaiting-user"]
    if isinstance(decision, dict):
        args = ["dep", entry["id"]]
        for target in decision["dep"]:
            args += ["--on", target]
        return args
    return None  # "leave"


def apply(tasks, root, path, snapshot, report):
    for entry in snapshot["entries"]:
        if entry["done"]:
            continue
        if entry["decision"] is None:
            report["undecided"].append(entry["id"])
            continue
        code, stdout, stderr = run(tasks, root, "show", entry["id"])
        if code != 0:
            if error_kind(stderr) == "task_not_found":
                entry.update(done=True, outcome="changed: record gone")
                report["changed"].append({"id": entry["id"], "detail": "record gone"})
            else:
                entry["outcome"] = f"failed: show: {stderr.strip()}"
                report["failed"].append({"id": entry["id"], "detail": f"show: {stderr.strip()}"})
            save(path, snapshot)
            continue
        task = json.loads(stdout)["task"]
        if task["status"] != "idea" or not task.get("reviewed") or task["updated"] != entry["updated"]:
            detail = (f"status {task['status']}, reviewed {str(bool(task.get('reviewed'))).lower()}, "
                      f"updated {task['updated']} (frozen {entry['updated']})")
            entry.update(done=True, outcome=f"changed: {detail}")
            report["changed"].append({"id": entry["id"], "detail": detail})
            save(path, snapshot)
            continue
        args = command_for(entry)
        if args is None:
            entry.update(done=True, outcome="left")
            report["left"].append(entry["id"])
            save(path, snapshot)
            continue
        code, stdout, stderr = run(tasks, root, *args)
        if code == 0:
            entry.update(done=True, outcome="applied")
            report["applied"].append({"id": entry["id"], "command": args})
            for warning in json.loads(stdout).get("warnings", []):
                if warning.startswith(("commit_failed:", "commit_deferred:")):
                    entry["outcome"] = f"applied; {warning}"
                    report["uncommitted"].append({"id": entry["id"], "warning": warning})
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
        report = {"prefix": prefix, "frozen": False, "applied": [], "left": [], "changed": [],
                  "undecided": [], "failed": [], "uncommitted": [], "remaining": 0}
        if os.path.exists(path):
            snapshot = load(path, prefix)
        else:
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
        return 1 if report["failed"] or report["uncommitted"] else 0
    except Usage as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
```

The freeze is read-only toward records, so `--freeze-only` needs no pause. A first `--writers-paused` run freezes and applies only the `auto` entries, since `judge` entries are undecided. The Rollout always freezes with `--freeze-only` first.

- [ ] **Step 4: Run them to verify they pass**

Run: `just test-one --test wait_backfill`
Expected: PASS.

- [ ] **Step 5: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add tools/wait-backfill tests/wait_backfill.rs
git commit -m "feat(tools): wait-backfill turns legacy prose waits into recorded ones"
```

---

## Rollout (after the branch is reviewed; operator steps, not implementer tasks)

These steps change live state across hosts. Confirm each step that touches a host other than this one, or writes real records, with the user at the time.

1. **Writer pause, merge, binaries, together.**
   - Pause scope and curate passes, and every other agent session that writes task records, on every host that reads synced task records.
   - Merge the branch to main.
   - Run `cargo install --path .` on this host, then on each other reading host, in its synced main checkout once the merge has arrived there.
   - On each host, confirm that `tasks note --help` lists `--awaiting-user`.
   - No host closes a task with the new binary until every host runs it. Every `done` and `drop` writes `closed` (spec §7.1). The skills are symlinks into the main checkout, so they go live with the merge.
   - Resume writers only after every host is upgraded.
2. **Freeze, per registered project.**
   - From each project's own checkout, run `tools/wait-backfill --state ~/.local/state/tasks/wait-backfill --root <project root> --freeze-only`. This only reads, so no pause is needed.
   - Show the user the snapshot's `judge` entries, with each one's evidence.
3. **Rule.** The user (or an agent in a scope pass from that checkout, with the user's agreement) writes a decision into each `judge` entry: `"leave"`, `"awaiting_user"`, or `{"dep": [ids]}`. Missed wakes and closed edges default to `"leave"`.
4. **Apply, per project, under a writer pause.**
   - On each reading host, `tasks claims` shows no live claim in the project, and the operator confirms that every other writer is paused.
   - Run the tool with `--writers-paused`.
   - Read `applied`, `left`, `changed`, `undecided`, `failed` and `uncommitted`, and rerun after fixing any `failed` entry.
5. **Commits.**
   - For every project the backfill touched, run `git -C <project root> status --porcelain -- tasks/`.
   - Where it lists anything, run `tasks commit` there and check again. Continue only when every project prints nothing.
   - A project without `[git] commit` gets its one `chore(tasks): backfill recorded waits` commit here.
6. **Record and resume.**
   - Note the per-project counts (`applied`, `left`, `changed`) on the implementation goal.
   - Append `rollout complete <date>` to the spec's status line.
   - Resume the paused writers.
