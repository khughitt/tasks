# Shelved Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `shelved` status (open, hidden from the default views, entered only through `tasks shelve <id> "<wake condition>"`) and teach `tasks sample` the `scope:` note prefix.

**Architecture:** `Status::Shelved` joins the enum as an open status, so every dependency, hierarchy, and closeout rule follows from `is_open` unchanged. Two new subcommands go through the existing `close`/`transition` path in `src/commands/status.rs`; the hiding is done at the three views that filter by status (`list`, the hierarchy `forest` behind `tree` and `prime`'s roadmap, and `check`'s new warning). Guards (`start`, `park`, `edit --status`, the editor path, shelving a goal) are typed errors at the command that would otherwise misbehave.

**Tech Stack:** Rust, clap, serde; end-to-end tests in `tests/cli.rs` against the built binary (`TestEnv`, `as_agent`, `editor_script`, `write_claim`, `old_task`, `stamp`, `id_of`).

**Spec:** `docs/specs/2026-09-12-scope-pass-design.md` §3 (status), §4.7 (`sample` prefix), §6 (tests). Task: tasks-470e8c.

## Global Constraints

- JSON output is the contract. The `status` enum gains `shelved`; `prime` and `projects` count objects gain a `shelved` field; `shelve`/`unshelve` return the id shape `{"id", "warnings"}`. No other shape changes.
- Fail early with a typed error; no silent fallbacks. Reuse `Error::OpenDescendants` (kind `open_descendants`), `Error::InvalidTransition` (kind `invalid_transition`), and `Error::Validation` (kind `validation`).
- Never edit `tasks/*.md` by hand; `tasks check` before every commit; `just check` is the pre-commit hook (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`).
- Conventional commits; no AI-attribution trailers.
- Work in `.worktrees/scope/` on branch `scope`. Every path below is relative to that worktree.
- The package has only a binary target: focused unit tests run as `cargo test --bin tasks <filter>`, one end-to-end test as `cargo test --test cli <name>`, and the full suite as `just test` (the repository's timing wrapper); the end-to-end tests build the binary themselves.

---

### Task 1: `Status::Shelved` in the model, the counts, and the styles

**Files:**
- Modify: `src/model.rs:196-250` (enum, `ALL`, `parse`/`as_str`, tests at 25-50)
- Modify: `src/output.rs:423-475` (`Counts`, `count_columns`)
- Modify: `src/style.rs:88-95` and the table at `:160-170`
- Modify: `src/commands/mod.rs:830-840` (the store-cleanup recovery hint)

**Interfaces:**
- Produces: `Status::Shelved` (`as_str() == "shelved"`, `is_open() == true`), `Counts::shelved: usize`, a `shelved` count column between `blocked` and the closed columns.

- [ ] **Step 1: Write the failing unit tests**

In `src/model.rs`, extend the two existing tests:

```rust
    #[test]
    fn status_roundtrip_and_openness() {
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()).unwrap(), s);
        }
        assert_eq!(Status::ALL.len(), 7);
        assert_eq!(Status::parse("shelved").unwrap(), Status::Shelved);
        assert!(Status::Idea.is_open());
        assert!(Status::Blocked.is_open());
        assert!(Status::Shelved.is_open(), "shelved keeps blocking dependents");
        assert!(!Status::Done.is_open());
        assert!(!Status::Dropped.is_open());
    }

    #[test]
    fn transition_table() {
        use Status::*;
        // ... existing asserts unchanged ...
        assert!(Status::can_transition(Idea, Shelved));
        assert!(Status::can_transition(Doing, Shelved));
        assert!(Status::can_transition(Shelved, Idea));
        assert!(Status::can_transition(Shelved, Todo));
        assert!(!Status::can_transition(Done, Shelved), "closed reopens to todo only");
        assert!(!Status::can_transition(Dropped, Shelved));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --bin tasks model::tests`
Expected: compile error, `no variant named Shelved`.

- [ ] **Step 3: Add the variant**

`src/model.rs`:

```rust
pub enum Status {
    Idea,
    Todo,
    Doing,
    Blocked,
    Shelved,
    Done,
    Dropped,
}

impl Status {
    pub const ALL: [Status; 7] = [
        Status::Idea,
        Status::Todo,
        Status::Doing,
        Status::Blocked,
        Status::Shelved,
        Status::Done,
        Status::Dropped,
    ];
    // parse unchanged

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Idea => "idea",
            Status::Todo => "todo",
            Status::Doing => "doing",
            Status::Blocked => "blocked",
            Status::Shelved => "shelved",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }
    // is_open and can_transition unchanged: Shelved is open by construction
```

- [ ] **Step 4: Fix the exhaustive matches the compiler now reports**

`src/output.rs` — `Counts` and its two functions:

```rust
pub struct Counts {
    pub idea: usize,
    pub todo: usize,
    pub doing: usize,
    pub blocked: usize,
    pub shelved: usize,
    pub done: usize,
    pub dropped: usize,
}

impl Counts {
    pub fn of(tasks: &[Task]) -> Counts {
        let mut counts = Counts::default();
        for task in tasks {
            match task.status {
                Status::Idea => counts.idea += 1,
                Status::Todo => counts.todo += 1,
                Status::Doing => counts.doing += 1,
                Status::Blocked => counts.blocked += 1,
                Status::Shelved => counts.shelved += 1,
                Status::Done => counts.done += 1,
                Status::Dropped => counts.dropped += 1,
            }
        }
        counts
    }

    pub fn total(&self) -> usize {
        self.idea + self.todo + self.doing + self.blocked + self.shelved + self.done + self.dropped
    }
}

pub fn count_columns(counts: &Counts, closed: bool) -> Vec<CountColumn> {
    let mut columns = vec![
        count_column("idea", counts.idea, Status::Idea),
        count_column("todo", counts.todo, Status::Todo),
        count_column("doing", counts.doing, Status::Doing),
        count_column("blocked", counts.blocked, Status::Blocked),
        // Open, so it belongs with the open columns: a project with thirty shelved ideas
        // says so in one number (spec §3.2).
        count_column("shelved", counts.shelved, Status::Shelved),
    ];
    // rest unchanged
```

`src/style.rs` — the status colour, dim blue (an idea that is out of sight), in the `match style` and in the table test at `:160-170`:

```rust
            Style::Status(Status::Blocked) => "31",
            Style::Status(Status::Shelved) => "2;34",
```

and add `(Style::Status(Status::Shelved), "2;34"),` to the table the test at `:163` iterates.

`src/commands/mod.rs:830-840` — the recovery hint when the store cleanup fails after a status save:

```rust
                    let recovery = match task.status {
                        Status::Done => format!("run `tasks done {id}`"),
                        Status::Dropped => format!("run `tasks drop {id}`"),
                        Status::Blocked => format!("run `tasks block {id}`"),
                        Status::Shelved => format!("run `tasks shelve {id} \"<wake condition>\"`"),
                        Status::Todo | Status::Idea | Status::Doing => {
                            "the store is unchanged and a same-status edit will not retry cleanup"
                                .into()
                        }
                    };
```

- [ ] **Step 5: Build, run the unit tests, then the whole suite**

Run: `cargo build && cargo test --bin tasks model::tests && just test`
Expected: all pass. If a `tests/cli.rs` test pins the exact `counts` object of `prime` or `projects` (search `"blocked":` in `tests/cli.rs`), add `"shelved": 0` at the same position in that expectation — that is the contract change this task makes, nothing else.

- [ ] **Step 6: Commit**

```bash
git add src/model.rs src/output.rs src/style.rs src/commands/mod.rs tests/cli.rs
git commit -m "feat(model): add the shelved status as an open status"
```

---

### Task 2: `tasks shelve` and `tasks unshelve`

**Files:**
- Modify: `src/cli.rs:334-345` (beside `Block`/`Unblock`)
- Modify: `src/commands/status.rs:135-150` (beside `block`/`unblock`)
- Modify: `src/commands/mod.rs:35-45` (`ClaimIntent::Release`), `:205-216` (the release intent), `:806-830` (the release branch), `:1076-1077` (dispatch)
- Modify: `src/complete.rs:99` (`id_directed` command list)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Status::Shelved` (Task 1); `close(ctx, id, to, message, force)` and `transition` in `status.rs`/`mod.rs`; `crate::hierarchy::open_descendants`.
- Produces: `status::shelve(ctx, id, wake: String)`, `status::unshelve(ctx, id)`; `ClaimIntent::Release { clear_park: bool, announce_escalation: bool }`.

- [ ] **Step 1: Write the failing end-to-end tests**

Append to `tests/cli.rs` (helpers `as_agent`, `id_of`, `write_claim` already exist).
`env.fail` asserts exit 1 and returns only the error *kind*; when a test needs the message
too, use this helper, added once beside `as_agent`:

```rust
/// The parsed `{"error": {"kind", "message"}}` of a command expected to exit 1.
fn error_of(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stderr).unwrap()
}
```

```rust
#[test]
fn shelve_writes_the_status_and_the_note_and_unshelve_returns_to_idea() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--size", "s"]));
    let v = env.json(&sci, &["shelve", &id, "when profiles have two consumers"]);
    assert_eq!(v["id"], id);
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "shelved");
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(
        notes.last().unwrap()["text"],
        "shelved: when profiles have two consumers"
    );
    assert_eq!(shown["task"]["size"], "s", "fields survive the shelf");

    env.json(&sci, &["unshelve", &id]);
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "idea");
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(notes.last().unwrap()["text"], "unshelved");
}

#[test]
fn shelve_requires_a_wake_condition_and_unshelve_requires_shelved() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let out = env.cmd(&sci).args(["shelve", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "usage error, nothing written");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    let err = error_of(&env, &sci, &["unshelve", &id]);
    assert_eq!(err["error"]["kind"], "invalid_transition");
    assert!(err["error"]["message"].as_str().unwrap().contains("unshelve requires shelved"));
}

#[test]
fn shelve_follows_the_claim_rules_and_clears_a_park_and_its_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let held = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    write_claim(&env, "sci", &held, "agent-z", true);
    let err = as_agent(&env, &sci, "agent-a")
        .args(["shelve", &held, "later"])
        .output()
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&err.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "claimed");

    let parked = id_of(env.json(&sci, &["add", "Parked", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &sci, "agent-a")
        .args(["start", &parked])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args([
            "park", &parked, "stuck", "--reason", "capability", "--complexity", "high",
        ])
        .assert()
        .success();
    assert!(!env.json(&sci, &["show", &parked])["escalation"].is_null());

    let v: serde_json::Value = {
        let out = as_agent(&env, &sci, "agent-a")
            .args(["shelve", &parked, "after the rack lands"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w.as_str().unwrap().starts_with(&format!(
            "cleared the escalation of {parked} to high"
        ))),
        "{warnings:?}"
    );
    let shown = env.json(&sci, &["show", &parked]);
    assert!(shown["park"].is_null(), "shelving clears the park entry");
    assert!(shown["escalation"].is_null(), "and its escalation");
    assert!(shown["claim"].is_null(), "and releases the claim");
}

#[test]
fn shelve_refuses_a_goal_with_unshelved_open_descendants() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2"]));
    let a = id_of(env.json(&sci, &["add", "A", "--parent", &goal]));
    let b = id_of(env.json(&sci, &["add", "B", "--parent", &goal]));
    let err = error_of(&env, &sci, &["shelve", &goal, "someday"]);
    assert_eq!(err["error"]["kind"], "open_descendants");
    let message = err["error"]["message"].as_str().unwrap();
    assert!(message.contains(&a) && message.contains(&b), "{message}");
    assert_eq!(env.json(&sci, &["show", &goal])["task"]["status"], "todo");

    env.json(&sci, &["shelve", &a, "someday"]);
    env.json(&sci, &["done", &b, "landed"]);
    env.json(&sci, &["shelve", &goal, "someday"]);
    assert_eq!(env.json(&sci, &["show", &goal])["task"]["status"], "shelved");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --test cli shelve`
Expected: 4 failures; the CLI reports `unrecognized subcommand 'shelve'`.

- [ ] **Step 3: Add the subcommands**

`src/cli.rs`, after `Unblock`:

```rust
    /// Put a task out of active work: status=shelved, hidden from the default views,
    /// open for dependencies. The wake condition is required and becomes the note.
    Shelve {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// What would bring the task back.
        wake: String,
    },
    /// Return a shelved task to idea.
    Unshelve {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
```

`src/commands/mod.rs` dispatch, beside the `Block`/`Unblock` arms:

```rust
        Command::Shelve { id, wake } => status::shelve(open_id_write_ctx(dir, &id)?, id, wake),
        Command::Unshelve { id } => status::unshelve(open_id_write_ctx(dir, &id)?, id),
```

`src/complete.rs:99`: add `"shelve", "unshelve",` to the id-directed command list after `"unblock"`.

`src/commands/status.rs`, after `unblock`:

```rust
/// A goal is shelved only after its open descendants are; `ready` reads each child's own
/// status, so shelving the goal alone would hide it while its children stayed eligible
/// (spec §3.1).
pub fn shelve(mut ctx: Ctx, id: String, wake: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    crate::format::validate_line("wake condition", &wake)?;
    let all = ctx.project.scan()?;
    let unshelved: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id, &ctx.registry)
        .iter()
        .filter(|descendant| descendant.status != Status::Shelved)
        .map(|descendant| descendant.id.to_string())
        .collect();
    if !unshelved.is_empty() {
        return Err(Error::OpenDescendants(
            task.id.to_string(),
            format!("{} (shelve or close them first)", unshelved.join(", ")),
        ));
    }
    transition(&mut ctx, &mut task, Status::Shelved, false)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &format!("shelved: {wake}"))?;
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn unshelve(mut ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    if task.status != Status::Shelved {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "idea (unshelve requires shelved)".into(),
        ));
    }
    transition(&mut ctx, &mut task, Status::Idea, false)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, "unshelved")?;
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
```

If `crate::format::validate_line` has a different name or signature in this tree, use whatever `park.rs:23` calls to validate `next_step`; the point is a one-line, non-empty message.

- [ ] **Step 4: Clear the park entry and announce the escalation**

`src/commands/mod.rs`, the intent enum:

```rust
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release {
        clear_park: bool,
        /// Report a removed escalation even without an explicit reassessment. `shelve`
        /// sets it: the escalation drove resumption, and a shelved task is not resumed
        /// (spec §3.3). `done` and `drop` keep their silence.
        announce_escalation: bool,
    },
    Park { ... },
}
```

The release intent in `claim_guard` (`:205-216`):

```rust
            (
                id.clone(),
                ClaimIntent::Release {
                    clear_park: matches!(to, Status::Done | Status::Dropped | Status::Shelved),
                    announce_escalation: to == Status::Shelved,
                },
            )
```

The release branch (`:806-830`): destructure `ClaimIntent::Release { clear_park, announce_escalation }` and change the warning condition:

```rust
                Ok(()) => {
                    if let Some(escalation) = &removed_escalation
                        && (clear_escalation.is_some() || announce_escalation)
                    {
                        ctx.warnings.push(format!(
                            "cleared the escalation of {id} to {} recorded by session {} at {}",
                            escalation.level.as_str(),
                            escalation.session,
                            escalation.at
                        ));
                    }
                }
```

Any other constructor of `ClaimIntent::Release` in the tree (grep `ClaimIntent::Release`) passes `announce_escalation: false`.

- [ ] **Step 5: Run the new tests, then the suite**

Run: `cargo test --test cli shelve && just test`
Expected: all pass. The existing test `reassessment_clears_on_every_edit_shape_and_closing_clears_too` still passes: `done`/`drop` behaviour is unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/commands/status.rs src/commands/mod.rs src/complete.rs tests/cli.rs
git commit -m "feat(cli): add shelve and unshelve"
```

---

### Task 3: Guards — `start`, `park`, `edit --status`, and the editor path

**Files:**
- Modify: `src/commands/status.rs:6-20` (`start`)
- Modify: `src/commands/park.rs:14-24`
- Modify: `src/commands/edit.rs:126-133` (flag path) and `:217-224` (editor path)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Status::Shelved`, `tasks shelve` (Tasks 1–2), `editor_script` test helper.
- Produces: nothing new; four refusals.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn start_and_park_refuse_a_shelved_task_and_name_unshelve() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);
    for args in [vec!["start", id.as_str()], vec!["park", id.as_str(), "next"]] {
        let err = error_of(&env, &sci, &args);
        assert_eq!(err["error"]["kind"], "invalid_transition", "{args:?}");
        assert!(
            err["error"]["message"].as_str().unwrap().contains("tasks unshelve"),
            "{args:?}: {err}"
        );
    }
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "shelved");
    assert!(env.json(&sci, &["show", &id])["park"].is_null());
}

#[test]
fn edit_refuses_a_transition_into_shelved_and_allows_edits_of_a_shelved_record() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let err = error_of(&env, &sci, &["edit", &id, "--status", "shelved"]);
    assert_eq!(err["error"]["kind"], "validation");
    assert!(err["error"]["message"].as_str().unwrap().contains("tasks shelve"));
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    let into = editor_script(&sci, "sed -i 's/^status: todo$/status: shelved/' \"$1\"");
    let out = env
        .cmd(&sci)
        .env("EDITOR", &into)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "validation");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    env.json(&sci, &["shelve", &id, "later"]);
    let retitle = editor_script(&sci, "sed -i 's/^title: T$/title: Renamed/' \"$1\"");
    let out = env
        .cmd(&sci)
        .env("EDITOR", &retitle)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["title"], "Renamed");
    assert_eq!(shown["task"]["status"], "shelved", "an edit keeps the shelf");

    env.json(&sci, &["edit", &id, "--status", "todo"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo", "explicit reopen");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --test cli shelved`
Expected: both fail (`start` succeeds where it should refuse; `edit --status shelved` succeeds).

- [ ] **Step 3: Add the guards**

`src/commands/status.rs`, at the top of `start`, after `load`:

```rust
    if task.status == Status::Shelved {
        return Err(Error::InvalidTransition(
            "shelved".into(),
            format!("doing (`tasks unshelve {id}` first)"),
        ));
    }
```

`src/commands/park.rs`, after the `is_open` check:

```rust
    if task.status == Status::Shelved {
        return Err(Error::InvalidTransition(
            "shelved".into(),
            format!("parked (`tasks unshelve {id}` first)"),
        ));
    }
```

(`park.rs` imports `Status` from `crate::model` if it does not already.)

`src/commands/edit.rs`, both branches. A shared helper at the bottom of the file:

```rust
/// The wake condition is the entry ticket to the shelf, and only `shelve` collects it
/// (spec §3.1). A save that keeps an already-shelved status never reaches here.
fn refuse_shelving(id: &crate::model::TaskId) -> Result<()> {
    Err(Error::Validation(format!(
        "use `tasks shelve {id} \"<wake condition>\"` to shelve a task"
    )))
}
```

Flag path (`:126-133`):

```rust
    if let Some(status) = args.status {
        let to = Status::parse(&status)?;
        if to == task.status {
            ctx.refuse_foreign_live_claim(&task.id)?;
            ctx.preserve_claim_store(&task.id);
        } else {
            if to == Status::Shelved {
                refuse_shelving(&task.id)?;
            }
            transition(&mut ctx, &mut task, to, args.force)?;
        }
    }
```

Editor path (`:217-224`):

```rust
    if status == original.status {
        ctx.refuse_foreign_live_claim(&original.id).map_err(keep)?;
        ctx.preserve_claim_store(&original.id);
    } else {
        if status == Status::Shelved {
            refuse_shelving(&original.id).map_err(keep)?;
        }
        transition(&mut ctx, &mut edited, status, false).map_err(keep)?;
    }
```

- [ ] **Step 4: Run the tests, then the suite**

Run: `cargo test --test cli shelved && just test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/commands/status.rs src/commands/park.rs src/commands/edit.rs tests/cli.rs
git commit -m "feat(cli): refuse start, park, and edits into shelved"
```

---

### Task 4: Visibility — `list`, the hierarchy, `check`, and the rules that follow from openness

**Files:**
- Modify: `src/commands/list.rs:60-70` and `:150-160` (the two default status filters)
- Modify: `src/commands/parked.rs:89-102` (`candidates`, the second feed of `next`)
- Modify: `src/hierarchy.rs:150-215` (`forest`/`node`) and its callers `src/commands/tree.rs:23`, `src/commands/list.rs:362`, and the unit tests at `src/hierarchy.rs:337-353`
- Modify: `src/commands/check.rs:98-140` (the dependency loop)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Status::Shelved`, `tasks shelve`, the `write_park` test helper at `tests/cli.rs:356`.
- Produces: `hierarchy::is_active(&Task) -> bool` (open and not shelved); `hierarchy::Shelved { Hidden, UnderShownParent }`, a new parameter of `forest` after `include_closed`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn shelved_is_hidden_from_list_ready_next_sample_and_prime_but_counted() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);
    stamp(&sci, &id, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");

    let ids = |v: &serde_json::Value| -> Vec<String> {
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(ids(&env.json(&sci, &["list"])).is_empty(), "hidden by default");
    assert_eq!(ids(&env.json(&sci, &["list", "--status", "shelved"])), vec![id.clone()]);
    assert!(ids(&env.json(&sci, &["ready"])).is_empty());
    assert!(env.json(&sci, &["next"])["next"].is_null());
    assert!(ids(&env.json(&sci, &["sample", "-n", "5", "--seed", "1"])).is_empty());

    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["counts"]["shelved"], 1);
    assert_eq!(prime["counts"]["idea"], 0);
    assert!(prime["roadmap"].as_array().unwrap().is_empty(), "a shelved root is not roadmap");
    let projects = env.json(&sci, &["projects"]);
    let row = projects["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["prefix"] == "sci")
        .unwrap();
    assert_eq!(row["counts"]["shelved"], 1);

    let pretty = env.pretty(&sci, &["prime"]);
    assert!(pretty.contains("shelved 1"), "{pretty}");
}

#[test]
fn a_shelved_child_stays_visible_where_hiding_it_would_lie() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&sci, &["add", "Child", "--parent", &goal]));
    let root = id_of(env.json(&sci, &["add", "Root", "--status", "idea"]));
    env.json(&sci, &["shelve", &child, "later"]);
    env.json(&sci, &["shelve", &root, "later"]);

    // show: the child is listed with its status
    let shown = env.json(&sci, &["show", &goal]);
    let children = shown["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["status"], "shelved");

    // tree: the goal shows its shelved child, and a shelved subgoal shows the shelved
    // leaf that keeps it open; the shelved root is hidden unless --all
    let subgoal = id_of(env.json(&sci, &["add", "Subgoal", "--parent", &goal]));
    let leaf = id_of(env.json(&sci, &["add", "Leaf", "--parent", &subgoal]));
    env.json(&sci, &["shelve", &leaf, "later"]);
    env.json(&sci, &["shelve", &subgoal, "later"]);
    let tree = env.json(&sci, &["tree"]);
    let roots = tree["nodes"].as_array().unwrap();
    assert_eq!(roots.len(), 1, "{tree}");
    assert_eq!(roots[0]["id"], goal);
    let kids = roots[0]["children"].as_array().unwrap();
    let mut kid_ids: Vec<&str> = kids.iter().map(|k| k["id"].as_str().unwrap()).collect();
    kid_ids.sort();
    let mut expected = vec![child.as_str(), subgoal.as_str()];
    expected.sort();
    assert_eq!(kid_ids, expected, "{tree}");
    let shown_subgoal = kids.iter().find(|k| k["id"] == subgoal).unwrap();
    assert_eq!(shown_subgoal["children"][0]["id"], leaf, "{tree}");
    let all = env.json(&sci, &["tree", "--all"]);
    assert_eq!(all["nodes"].as_array().unwrap().len(), 2);

    // prime: the goal is in the roadmap without any shelved row, and not in closeout
    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["roadmap"][0]["id"], goal);
    fn statuses(node: &serde_json::Value, out: &mut Vec<String>) {
        out.push(node["status"].as_str().unwrap().to_string());
        for child in node["children"].as_array().unwrap() {
            statuses(child, out);
        }
    }
    let mut seen = Vec::new();
    for node in prime["roadmap"].as_array().unwrap() {
        statuses(node, &mut seen);
    }
    assert!(!seen.iter().any(|s| s == "shelved"), "roadmap hides every shelved row: {prime}");
    assert_eq!(seen.len(), 1, "only the goal itself: {prime}");
    assert!(prime["closeout"].as_array().unwrap().is_empty());

    // done refuses while the shelved child is open
    assert_eq!(env.fail(&sci, &["done", &goal, "finished"]), "open_descendants");
}

#[test]
fn next_never_hands_out_a_shelved_task_even_with_a_surviving_park_entry() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);
    // A failed store cleanup after `shelve` can leave the park entry behind (spec §3.3
    // names the recovery); the picker must still refuse it.
    write_park(&env, "sci", &id, "agent-a", "agent", &sci.display().to_string());
    assert!(env.json(&sci, &["next"])["next"].is_null());
    let prime = env.json(&sci, &["prime"]);
    assert!(prime["ready"].as_array().unwrap().is_empty());
    // The leftover entry is an anomaly, and the parked section shows it as one: the
    // row carries the record's status, which is how a reader notices and repairs it.
    let parked = prime["parked"].as_array().unwrap();
    assert_eq!(parked.len(), 1, "{prime}");
    assert_eq!(parked[0]["id"], id);
    assert_eq!(parked[0]["status"], "shelved");
}

#[test]
fn a_dependency_on_a_shelved_task_holds_ready_and_check_names_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let dep = id_of(env.json(&sci, &["add", "Dep", "-p", "2"]));
    let work = id_of(env.json(&sci, &["add", "Work", "-p", "2", "--depends", &dep]));
    env.json(&sci, &["shelve", &dep, "later"]);

    let ready = env.json(&sci, &["ready"]);
    assert!(ready["tasks"].as_array().unwrap().is_empty(), "{ready}");
    let check = env.json(&sci, &["check"]);
    let warning = check["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "shelved_dep")
        .unwrap_or_else(|| panic!("{check}"));
    assert_eq!(warning["id"], work);
    assert_eq!(
        warning["detail"],
        format!("depends on shelved {dep}: unshelve it or drop the dependency")
    );

    // a shelved task depending on a shelved task is not a finding
    env.json(&sci, &["shelve", &work, "later"]);
    let check = env.json(&sci, &["check"]);
    assert!(
        !check["warnings"].as_array().unwrap().iter().any(|w| w["kind"] == "shelved_dep"),
        "{check}"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --test cli shelved`
Expected: four failures: `list` shows the shelved task; `tree` shows the shelved root; `next` hands out the parked shelved task; `check` has no `shelved_dep` warning.

- [ ] **Step 3: Hide from `list`**

`src/commands/list.rs`, both default filters. At `:60-70`:

```rust
        let status_ok = if !statuses.is_empty() {
            statuses.contains(&task.status)
        } else if periodic {
            // Most of a healthy series is closed at any moment (spec §5.2).
            true
        } else {
            // Shelved is open but out of sight; `--status shelved` is its view.
            task.status.is_open() && task.status != Status::Shelved
        };
```

At `:150-160` (`list_parked`), the same substitution:

```rust
            let status_ok = if statuses.is_empty() {
                status.is_open() && status != Status::Shelved
            } else {
                statuses.contains(&status)
            };
```

`ready` and `sample` need no change: `is_actionable` is `todo` or due, and `sample`'s pool names its statuses. `next` has a second feed, `parked::candidates`, which accepts every open status but `blocked`; add the shelf to that exclusion in `src/commands/parked.rs:95-98`:

```rust
        if park.waiting_on != WaitingOn::Agent
            || !task.status.is_open()
            || matches!(task.status, Status::Blocked | Status::Shelved)
            || !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty()
        {
            continue;
        }
```

`parked::rows` (the parked section of `prime` and `list --parked`) is left alone on purpose: it lists store entries, and a park entry surviving on a shelved record is an anomaly the row should show, not hide.

- [ ] **Step 4: The hierarchy rule, one mode per view**

`tree` and `prime`'s roadmap share `forest` but have different contracts (spec §3.2): the
roadmap hides every shelved row; `tree` shows a shelved node whenever its parent is shown,
so a goal that cannot close shows why, all the way down (`active goal → shelved subgoal →
shelved leaf` shows the leaf: it is what keeps the subgoal open). A shelved root is hidden
in both unless `--all`.

`src/hierarchy.rs`. Add beside `open_descendants`:

```rust
/// Open and in sight. A shelved task is open (it holds dependents and its goal) but is
/// not itself a reason to show an ancestor; spec §3.2.
pub fn is_active(task: &Task) -> bool {
    task.status.is_open() && task.status != Status::Shelved
}

/// What `forest` does with a shelved node it reaches through a kept parent. A shelved
/// root is hidden either way unless closed nodes are included.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shelved {
    /// `prime`'s roadmap: no shelved row anywhere.
    Hidden,
    /// `tree`: shown under a shown parent, recursively, so the reason a goal cannot
    /// close is visible.
    UnderShownParent,
}
```

(import `Status` from `crate::model` if the module does not already.) `forest` gains the
parameter after `include_closed` and passes it, with `via_parent: bool`, into `node`:

```rust
pub fn forest(
    all: &[Task],
    root: Option<&TaskId>,
    include_closed: bool,
    shelved: Shelved,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    now: OffsetDateTime,
) -> Vec<TreeNode> {
    // tops unchanged ...
    tops.into_iter()
        .filter_map(|task| {
            node(all, task, include_closed, shelved, false, claims, registry,
                 &mut std::collections::HashSet::new(), now)
        })
        .collect()
}

fn node(
    all: &[Task],
    task: &Task,
    include_closed: bool,
    shelved: Shelved,
    via_parent: bool,
    claims: Option<&crate::claims::ClaimSnapshot>,
    registry: &Registry,
    visited: &mut std::collections::HashSet<TaskId>,
    now: OffsetDateTime,
) -> Option<TreeNode> {
    if !visited.insert(task.id.clone()) {
        return None;
    }
    let keep = include_closed
        || is_active(task)
        // A closed or shelved ancestor of active work stays visible as context.
        || open_descendants(all, &task.id, registry).iter().any(|d| is_active(d))
        // Reached through a kept parent: the view decides.
        || (task.status == Status::Shelved && via_parent && shelved == Shelved::UnderShownParent);
    if !keep {
        return None;
    }
    let mut kids = children(all, &task.id, registry);
    kids.sort_by(|a, b| ready_order(a, b));
    Some(TreeNode {
        summary: TaskSummary::of(task, all, claims, registry, now),
        children: kids
            .into_iter()
            .filter_map(|child| {
                node(all, child, include_closed, shelved, true, claims, registry, visited, now)
            })
            .collect(),
    })
}
```

Callers: `src/commands/tree.rs:23` passes `crate::hierarchy::Shelved::UnderShownParent`;
`src/commands/list.rs:362` (the roadmap) passes `crate::hierarchy::Shelved::Hidden`; the
three unit tests at `src/hierarchy.rs:337-353` pass `Shelved::Hidden`. Update the doc
comment on `forest`: "Without `include_closed`, a node is kept when it is open and not
shelved, has such a descendant, or (with `Shelved::UnderShownParent`) is a shelved child of
a kept parent."

- [ ] **Step 5: The `check` warning**

`src/commands/check.rs`, inside the `for dependency in &task.depends` loop, after the retired-prefix check and before the `if dependency_id.prefix == ctx.project.prefix` branch. The local branch already knows whether the file exists; resolve the record once through the same `resolver` the foreign branch uses:

```rust
            if task.status.is_open()
                && task.status != Status::Shelved
                && let Ok(Some(dependency_task)) = foreign(&dependency_id)
                && dependency_task.status == Status::Shelved
            {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "shelved_dep",
                    format!("depends on shelved {dependency}: unshelve it or drop the dependency"),
                ));
            }
```

`foreign` is the closure at `check.rs:58` and resolves local ids too (it wraps `resolver.resolve_task`); if it is restricted to foreign prefixes in this tree, call `resolver.resolve_task(&dependency_id)` directly. Import `Status` if needed.

- [ ] **Step 6: Run the tests, then the suite**

Run: `cargo test --test cli shelved && just test`
Expected: all pass. If an existing `tree`/`prime` test pinned the old `forest` keep rule for closed ancestors, the new rule keeps them for *active* descendants only; a closed ancestor of only-shelved work is now hidden, which is the spec's rule — adjust that test's fixture, not the rule.

- [ ] **Step 7: Commit**

```bash
git add src/commands/list.rs src/commands/parked.rs src/hierarchy.rs src/commands/tree.rs src/commands/check.rs tests/cli.rs
git commit -m "feat: hide shelved from the default views and name shelved dependencies"
```

---

### Task 5: `tasks sample` recognises the `scope:` prefix

**Files:**
- Modify: `src/commands/sample.rs:1-25`
- Test: `tests/cli.rs` (beside `sample_draws_only_from_the_curable_pool`)

**Interfaces:**
- Consumes: `old_task`, `stamp`, `sampled_ids` test helpers at `tests/cli.rs:10175-10192`.
- Produces: `pending_proposal` accepting `curate:` or `scope:`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sample_treats_a_scope_proposal_like_a_curate_proposal() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let pending = old_task(&env, &dir, "Pending", &["--status", "idea"]);
    env.json(
        &dir,
        &["note", &pending, "scope: drop; landed in abc1234; proposal: drop, abc1234"],
    );
    stamp(&dir, &pending, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let readmitted = old_task(&env, &dir, "Readmitted", &["--status", "idea"]);
    env.json(&dir, &["note", &readmitted, "scope: drop; proposal: drop, dup of x"]);
    env.json(&dir, &["note", &readmitted, "declined: keep it"]);
    stamp(&dir, &readmitted, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let briefed = old_task(&env, &dir, "Briefed", &["--status", "idea"]);
    env.json(&dir, &["note", &briefed, "scope: briefed; brief: docs/notes/x-brief.md"]);
    stamp(&dir, &briefed, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");

    let v = env.json(&dir, &["sample", "-n", "10", "--seed", "1"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![readmitted.clone(), briefed.clone()];
    expected.sort();
    assert_eq!(ids, expected, "a scope verdict without a proposal stays in the pool");
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap() == format!("{pending} pending: drop, abc1234")),
        "{warnings:?}"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test cli sample_treats_a_scope_proposal`
Expected: FAIL — `pending` is drawn and no pending warning is printed.

- [ ] **Step 3: Accept both prefixes**

`src/commands/sample.rs`:

```rust
/// The proposal text of a task whose most recent note is a curate or scope note awaiting
/// the human's decision. Any later note clears it, which is how the human answers.
fn pending_proposal(task: &Task) -> Option<&str> {
    let text = task.notes.last()?.text.as_str();
    let rest = text
        .strip_prefix("curate:")
        .or_else(|| text.strip_prefix("scope:"))?;
    rest.split_once("proposal:")
        .map(|(_, proposal)| proposal.trim())
}
```

Update the module doc comment's "`curate:` note" to "`curate:` or `scope:` note".

- [ ] **Step 4: Run the test, then the suite**

Run: `cargo test --test cli sample && just test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/commands/sample.rs tests/cli.rs
git commit -m "feat(sample): treat a scope: proposal as pending"
```

---

### Task 6: Documentation, protocol text, and the reinstall

**Files:**
- Modify: `docs/specs/2026-08-29-tasks-design.md:112` (status table), `:146-166` (statuses and transitions), `:279` (`sample` pool), `:319` (`block`/`unblock` reference), `:434` (`prime` shape)
- Modify: `docs/specs/2026-09-12-scope-pass-design.md:3` (status header)
- Modify: `README.md` (the status list near `:76`, the commands list)
- Modify: `skills/tasks/SKILL.md` (session protocol step 2, "Recording work")
- Modify: `skills/curate/SKILL.md` ("Pending proposals")
- Modify: `AGENTS.md` (session protocol)

- [ ] **Step 1: The tasks design spec**

`docs/specs/2026-08-29-tasks-design.md`:

- Status table row: `` `idea`, `todo`, `doing`, `blocked`, `shelved`, `done`, `dropped`. ``
- `:146`: `Statuses are *open* (`idea`, `todo`, `doing`, `blocked`, `shelved`) or *closed* (`done`, `dropped`).`
- After the `blocked` bullet at `:163`:

  ```markdown
  - `shelved`: open but out of sight; entered only by `tasks shelve <id> "<wake condition>"`,
    hidden from `list`, `ready`, `next`, `sample`, and `prime`'s roadmap, counted in `prime`
    and `projects`. `edit --status shelved` refuses. See `2026-09-12-scope-pass-design.md` §3.
  ```
- `:279` (`sample` pool): leave the pool statuses as they are; add to the pending sentence "a `curate:` or `scope:` note".
- After the `block`/`unblock` reference line at `:319`:

  ```
  tasks shelve <id> "<wake condition>" / tasks unshelve <id>
      status=shelved (the wake condition appended as `shelved: ...`; refuses a goal with
      open descendants that are not shelved; clears a park entry and its escalation) /
      status=idea. `start` and `park` refuse a shelved task.
  ```
- `:434`: `counts: { idea, todo, doing, blocked, shelved, done, dropped }`.
- The status header at the top: append `shelved 2026-09-<day>` to the implemented list, with the day the piece lands.

- [ ] **Step 2: The scope-pass spec header**

`docs/specs/2026-09-12-scope-pass-design.md:3`:

```
Status: `shelved` implemented (2026-09-<day>, see docs/plans/2026-09-12-shelved-status.md); `scope` skill pending (tasks-0d50ff)
```

- [ ] **Step 3: README, skills, AGENTS.md**

`README.md`: wherever the statuses are listed, add `shelved` with one clause: "open but hidden; `tasks shelve <id> "<wake condition>"` / `tasks unshelve <id>`". Add the two commands to the command list beside `block`/`unblock`.

`skills/tasks/SKILL.md`:
- Session protocol step 2, after "Never pick an `idea`; scope it first.": "A `shelved` task is out of active work: `list --status shelved` sees it, `tasks unshelve <id>` brings it back as an idea."
- "Recording work", after the unscoped-thought bullet:

  ```markdown
  - Not now, but kept: `tasks shelve <id> "<what would bring it back>"`. Shelved work is open
    (it still blocks dependents and holds its goal open) but hidden from `list`, `ready`, and
    `prime`; `check` warns when open work depends on it. `edit --status shelved` refuses; only
    `shelve` writes the shelf. `tasks unshelve <id>` returns it to `idea`.
  ```

`skills/curate/SKILL.md`, "Pending proposals": "`sample` never draws a task whose most recent note is a `curate:` or `scope:` note carrying a `proposal:` segment".

`AGENTS.md`, session protocol, after the `park` line: "`tasks shelve <id> "<wake condition>"` for work to keep out of sight; `unshelve` brings it back."

- [ ] **Step 4: Reinstall and gate**

Run: `cargo install --path . && just gate`
Expected: `check` and `test` pass; the installed `tasks` now knows `shelve`.

- [ ] **Step 5: Close the piece and commit**

```bash
tasks done tasks-470e8c "shelved status, shelve/unshelve, guards, hidden views, check warning, sample scope: prefix"
tasks check
git add docs/ README.md skills/ AGENTS.md tasks/
git commit -m "docs: record the shelved status and close tasks-470e8c"
```

---

## Self-review

**Spec coverage (§3, §4.7, §6):**
- §3.1 commands, required message, claim rules, goal guard, `unshelve` → idea, `edit --status shelved` and editor refusals, editor edits of a shelved record — Tasks 2, 3.
- §3.2 hidden from `list`/`prime`/`sample`/`ready`/`next` (both feeds of `next`); counted in `prime`/`projects`; `show`, `tree` child rule (recursive) versus the roadmap's blanket hide, `tree --all`, `check` warning — Task 4 (Task 1 for counts).
- §3.3 open by construction (dependencies, `done` refusal, descendants, forest); `start`/`park` refuse; park entry and escalation cleared, warning new — Tasks 1, 2, 3, 4.
- §3.4 JSON: enum value, counts, id shape, check text — Tasks 1, 2, 4; completion candidates come from `Status::ALL` (Task 1).
- §4.7 `sample` `scope:` prefix — Task 5.
- §6 tests: every bullet has a test above except "`rename` moves a shelved record like any open one" — `rename` treats every record the same and never reads status (spec §3.3 says so); no test added, and the executor should confirm by grep (`grep -n 'status' src/commands/rename.rs`) before Task 6 and add one if a status branch exists.
- §5 documentation — Task 6.

**Placeholders:** none. Every code step shows the code.

**Type consistency:** `status::shelve(ctx, id: String, wake: String)`, `status::unshelve(ctx, id: String)`, `ClaimIntent::Release { clear_park, announce_escalation }`, `hierarchy::is_active(&Task)`, finding kind `shelved_dep`, note texts `shelved: <wake>` / `unshelved` are used identically across tasks.
