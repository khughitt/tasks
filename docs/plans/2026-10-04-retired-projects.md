# Retired projects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A project can declare itself retired in `tasks/.config.toml`; the tracker then refuses every record write from every checkout, leaves the project out of wide reads, and keeps its ids resolving.

**Architecture:** A validated `[retired]` table becomes `Project.retired`. The write path's single lock choke point (`lock_and_revalidate`) refuses when the checkout or the registered root declares retirement. `retire`/`unretire` are new commands that edit the config with `toml_edit` under the project's `MutationLock`, after scanning every linked worktree for open tasks. `Scope::open_members` skips retired members.

**Tech Stack:** Rust 2024, clap, serde + `toml`, new dependency `toml_edit` (already in the local cargo cache at 0.25.x), `assert_cmd`-style integration tests in `tests/cli.rs` with `tests/common/mod.rs`'s `TestEnv`.

**Spec:** ops `docs/specs/2026-10-04-project-lifecycle-design.md` §3, §4, §8, §9 (accepted, review round 3). Goal ops-3b3e4d; this piece is tasks-132852; vocabulary ops-d0ef34.

## Global Constraints

- `[retired]` keys: `date` (required, `YYYY-MM-DD`), `reason` (required, one line, no control characters, not blank), `successor` (optional). Any other key is a config error.
- The successor is validated only by `tasks retire`: registered on this host, not retired, not the retiring prefix. Reading a stored table never resolves it.
- Error kind for refused writes: `retired_project`. The message names the stored successor when present and says `tasks unretire` in the registered checkout is the way back.
- `retire`/`unretire` run only in the registered checkout; in a linked worktree they refuse, naming the registered root.
- `retire` refuses on: any open task (idea, todo, doing, blocked, shelved — `Status::is_open`) in any checkout's copy; any live claim on the prefix; any linked worktree that cannot be read.
- `unregister` keeps working on a retired project. `init --force` and `rename` refuse.
- `--all-projects` and `--group` reads skip a retired project silently. Single-project reads add the warning `<prefix> is retired (<date>)` plus `; successor <s>` when stored.
- Internal naming: the rename feature already owns "retired prefix" (`Registry.aliases`, check kind `retired_prefix`). Name the new type `Retirement` and the field `Project.retired`; never reuse rename's alias vocabulary for it.
- Do not bump the crate version; host evidence is the install source commit (spec §9).

## Lock acquisition order

Every command that takes more than one lock takes them in this order, and no command takes them in any other:

1. `MutationLock(<prefix>)` — per project, `$XDG_STATE_HOME/tasks/claims/<prefix>.lock`. Two prefixes (rename) in sorted order.
2. `Registry::lock()` — `projects.lock` beside the registry.
3. `holds::lock()` — the holds lock, only by commands that already take it.

| Command | Locks, in order | Change |
|---|---|---|
| record writers (`add`, `note`, `edit`, `start`, `park`, `done`, `drop`, `block`, `unblock`, `shelve`, `unshelve`, `dep`, `attach`, `detach`, `feedback` on its target) | `MutationLock(p)` [→ holds] via `lock_and_revalidate` | retired check added inside, after the re-open |
| `rename` | `MutationLock(old)`, `MutationLock(new)` sorted → `Registry::lock` | retired check added after the locks |
| `init`, `init --force` | `MutationLock(p)` → `Registry::lock` | **new**: today it takes only the registry lock. Taking the project lock first keeps the order above and serialises `init --force` (which repoints the root) against `retire` reading that root |
| `retire`, `unretire` | `MutationLock(p)` only; the registry is read with `Registry::load` without its lock | new commands. Nothing that changes `p`'s root can run meanwhile, because `init --force` and `rename` now hold `MutationLock(p)` first |
| `unregister`, `group set`, `group rm` | `Registry::lock` only | unchanged. `group set` reads member configs without project locks; a member retired a moment later is skipped by reads anyway |

## Review Focus

1. A worktree created before `retire`, whose own config has no `[retired]`, attempting writes → refused (Task 2 tests every write command from such a worktree).
2. An open task that exists only in a linked worktree, with no claim → `retire` refused naming the task and that checkout (Task 4).
3. A writer and `retire` racing → either the write lands and `retire` sees it and refuses, or `retire` lands and the write is refused; never a written open task in a retired project (Task 4 concurrent test).
4. A stored successor that is unregistered on this host, or itself retired → every read still works and shows the string (Task 1 and Task 5 tests).
5. `edit` with no flags (the `$EDITOR` path) drops and re-takes the lock → refused at the re-take if retirement landed meanwhile (Task 2 test with a scripted editor).

---

### Task 1: The `[retired]` table

**Files:**
- Modify: `src/repo.rs` (`Config` :106-123, a new `RetiredConfig`, a validator beside `feedback_scope` :183-194, `Project` :68-89, `Project::open` :259-288)
- Modify: `src/commands/check.rs` (per-task loop from :64)
- Test: unit tests in `src/repo.rs`'s `#[cfg(test)]` module; `tests/cli.rs`

**Interfaces:**
- Produces: `pub struct Retirement { pub date: String, pub successor: Option<String>, pub reason: String }` (derive `Clone, Debug, PartialEq, serde::Serialize`); `Project.retired: Option<Retirement>`. Warning text is Task 5's.

- [ ] **Step 1: Write the failing tests**

In `src/repo.rs` tests:

```rust
#[test]
fn a_retired_table_is_read_with_and_without_a_successor() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tasks")).unwrap();
    std::fs::write(
        dir.path().join(CONFIG_REL),
        "prefix = \"old\"\n\n[retired]\ndate = \"2026-10-04\"\nsuccessor = \"new\"\nreason = \"superseded\"\n",
    )
    .unwrap();
    let project = Project::open(dir.path()).unwrap();
    assert_eq!(
        project.retired,
        Some(Retirement { date: "2026-10-04".into(), successor: Some("new".into()), reason: "superseded".into() })
    );
    std::fs::write(
        dir.path().join(CONFIG_REL),
        "prefix = \"old\"\n\n[retired]\ndate = \"2026-10-04\"\nreason = \"ended\"\n",
    )
    .unwrap();
    assert_eq!(Project::open(dir.path()).unwrap().retired.unwrap().successor, None);
}

#[test]
fn a_bad_retired_table_is_a_config_error() {
    for body in [
        "[retired]\nreason = \"r\"\n",                                  // no date
        "[retired]\ndate = \"2026-10-04\"\n",                           // no reason
        "[retired]\ndate = \"04/10/2026\"\nreason = \"r\"\n",           // not ISO
        "[retired]\ndate = \"2026-10-04\"\nreason = \"two\\nlines\"\n", // control char
        "[retired]\ndate = \"2026-10-04\"\nreason = \"  \"\n",          // blank
        "[retired]\ndate = \"2026-10-04\"\nreason = \"r\"\nextra = 1\n",// unknown key
        "[retired]\ndate = \"2026-10-04\"\nreason = \"r\"\nsuccessor = \"Bad Prefix\"\n",
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("tasks")).unwrap();
        std::fs::write(dir.path().join(CONFIG_REL), format!("prefix = \"old\"\n\n{body}")).unwrap();
        let error = Project::open(dir.path()).err().expect(body);
        assert_eq!(error.kind(), "config", "{body}");
    }
}
```

In `tests/cli.rs`:

```rust
#[test]
fn check_reports_an_open_task_in_a_retired_project_as_an_error() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.json(&old, &["add", "T", "-p", "2"]);
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[retired]\ndate = \"2026-10-04\"\nreason = \"r\"\n")).unwrap();
    let out = env.cmd(&old).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report["errors"].as_array().unwrap().iter().any(|f| f["kind"] == "open_in_retired"));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one a_retired_table a_bad_retired_table check_reports_an_open_task_in_a_retired`
Expected: compile error, `Retirement` not found.

- [ ] **Step 3: Implement**

```rust
// in Config
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retired: Option<RetiredConfig>,

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetiredConfig {
    date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    successor: Option<String>,
    reason: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Retirement {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub successor: Option<String>,
    pub reason: String,
}

/// The project's own retirement declaration (ops lifecycle spec §3). The successor is
/// checked for prefix grammar only: whether it is registered is `tasks retire`'s question,
/// asked once, so a host without the successor still reads the table.
fn retirement(raw: Option<RetiredConfig>) -> Result<Option<Retirement>> {
    let Some(table) = raw else {
        return Ok(None);
    };
    let bad = |what: &str| Err(Error::Config(format!("{CONFIG_REL}: [retired] {what}")));
    if chrono::NaiveDate::parse_from_str(&table.date, "%Y-%m-%d").is_err() || table.date.len() != 10 {
        return bad("date must be YYYY-MM-DD");
    }
    if table.reason.chars().any(char::is_control) || table.reason.chars().all(char::is_whitespace) {
        return bad("reason must be one line of text with no control characters");
    }
    if let Some(successor) = &table.successor {
        if !is_valid_prefix(successor) {
            return bad(&format!("successor {successor:?} is not a prefix"));
        }
    }
    Ok(Some(Retirement { date: table.date, successor: table.successor, reason: table.reason }))
}
```

Use whichever date crate `src/` already depends on (check `Cargo.toml`; the `defer` parser parses `YYYY-MM-DD` — reuse its function rather than adding chrono if chrono is absent). Add `pub retired: Option<Retirement>` to `Project` and `retired: retirement(config.retired)?,` in `Project::open`. Every other `Project { .. }` literal (grep `Project {`) gets `retired: None`.

In `check.rs`, inside the per-task loop:

```rust
if ctx.project.retired.is_some() && task.status.is_open() {
    errors.push(Finding::new(&task, "open_in_retired",
        format!("{} is {} in a retired project; close it or `tasks unretire`", task.id, task.status)));
}
```

(match the existing `Finding` constructor's shape at the nearest existing push.)

- [ ] **Step 4: Run to verify they pass**

Run: `just test-one a_retired_table a_bad_retired_table check_reports_an_open_task_in_a_retired`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/repo.rs src/commands/check.rs tests/cli.rs
git commit -m "feat(config): read and validate a [retired] table"
```

### Task 2: Refuse writes from every checkout

**Files:**
- Modify: `src/error.rs` (variant, `with_suffix` :73-110, `kind` :112-146)
- Modify: `src/commands/mod.rs` (`lock_and_revalidate` :668-697)
- Modify: `src/commands/feedback.rs` (lock at :121 and `guarded_update` :306 — confirm both run after a retired check on the target)
- Modify: `src/commands/edit.rs` (re-lock at :318 — confirm it goes through `lock_and_revalidate`; if it calls `MutationLock::acquire` directly, route it through the check)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Project.retired` (Task 1).
- Produces: `Error::RetiredProject(String)` with kind `"retired_project"`; `pub fn reject_retired(registry: &Registry, project: &Project) -> Result<()>` in `commands/mod.rs`, used again by Task 3.

- [ ] **Step 1: Write the failing tests**

```rust
/// Declare `[retired]` in a checkout's config by hand (Task 4's `retire` comes later).
fn declare_retired(root: &std::path::Path, successor: Option<&str>) {
    let config = root.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[retired]\ndate = \"2026-10-04\"\nreason = \"superseded\"\n");
    if let Some(s) = successor {
        text.push_str(&format!("successor = \"{s}\"\n"));
    }
    std::fs::write(&config, text).unwrap();
}

const WRITES: &[&[&str]] = &[
    &["add", "N", "-p", "2"],
    &["note", "{id}", "x"],
    &["edit", "{id}", "--title", "y"],
    &["start", "{id}"],
    &["park", "{id}", "next"],
    &["done", "{id}", "m"],
    &["drop", "{id}", "m"],
    &["block", "{id}", "m"],
    &["unblock", "{id}"],
    &["shelve", "{id}", "m"],
    &["unshelve", "{id}"],
    &["dep", "{id}", "--on", "{id2}"],
];

#[test]
fn every_write_is_refused_in_a_retired_project_from_the_registered_checkout() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    let id2 = id_of(env.json(&old, &["add", "U", "-p", "2"]));
    declare_retired(&old, Some("new"));
    for args in WRITES {
        let args: Vec<String> = args.iter().map(|a| a.replace("{id2}", &id2).replace("{id}", &id)).collect();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        assert_eq!(env.fail(&old, &args), "retired_project", "{args:?}");
    }
}

#[test]
fn a_worktree_made_before_retirement_cannot_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&main, None); // only the registered checkout knows
    assert!(!std::fs::read_to_string(side.join("tasks/.config.toml")).unwrap().contains("[retired]"));
    for args in [vec!["add", "N", "-p", "2"], vec!["note", &id, "x"], vec!["done", &id, "m"]] {
        assert_eq!(env.fail(&side, &args), "retired_project", "{args:?}");
    }
}

#[test]
fn a_worktree_that_declares_retirement_refuses_even_if_main_does_not() {
    let mut env = TestEnv::new();
    let (_main, side, _id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&side, None);
    assert_eq!(env.fail(&side, &["add", "N", "-p", "2"]), "retired_project");
}

#[test]
fn feedback_into_a_retired_project_is_refused() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[feedback]\nscope = \"s\"\n")).unwrap();
    declare_retired(&old, None);
    let here = env.init("here");
    assert_eq!(
        env.fail(&here, &["feedback", "--project", "old", "a tool line", "--category", "gap"]),
        "retired_project"
    );
}

#[test]
fn the_refusal_names_the_successor_and_the_way_back() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, Some("new"));
    let out = env.cmd(&old).args(["add", "N", "-p", "2"]).output().unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("new") && detail.contains("tasks unretire"), "{detail}");
}

#[test]
fn an_editor_save_is_refused_when_retirement_landed_while_it_was_open() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    // The editor script declares retirement, then changes the title, so the re-lock sees it.
    let script = old.join("editor.sh");
    std::fs::write(&script, format!(
        "#!/bin/sh\nprintf '\\n[retired]\\ndate = \"2026-10-04\"\\nreason = \"r\"\\n' >> {}\nsed -i 's/^title: .*/title: changed/' \"$1\"\n",
        old.join("tasks/.config.toml").display()
    )).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let out = env.cmd(&old).env("EDITOR", &script).args(["edit", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("retired_project"));
}
```

(Check the record's title line form in an existing editor test and adjust the `sed` to it.)

A command that validates its arguments before taking the lock (for example `unblock` on a
task that is not blocked) may fail with its own kind first; if one does, put the task in
the state that command expects before declaring retirement, so the test reaches the lock.

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one every_write_is_refused a_worktree_made_before a_worktree_that_declares feedback_into_a_retired the_refusal_names an_editor_save_is_refused`
Expected: FAIL — writes succeed.

- [ ] **Step 3: Implement**

`src/error.rs`:

```rust
    #[error("{0}")]
    RetiredProject(String),
// with_suffix: RetiredProject(m) => RetiredProject(format!("{m}{suffix}")),  (match the neighbours)
// kind:        RetiredProject(_) => "retired_project",
```

`src/commands/mod.rs`:

```rust
/// A retired project takes no writes (ops lifecycle spec §4). The registered checkout is
/// the authority, but a worktree made before the retirement has no `[retired]` of its own
/// until a merge, so either declaring it is enough.
pub fn reject_retired(registry: &Registry, project: &Project) -> Result<()> {
    let registered = registry
        .project_root(&project.prefix)
        .filter(|root| *root != project.root.as_path())
        .and_then(|root| Project::open(root).ok());
    let declared = project.retired.as_ref().or(registered.as_ref().and_then(|p| p.retired.as_ref()));
    let Some(retirement) = declared else { return Ok(()) };
    let authority = registry.project_root(&project.prefix).unwrap_or(&project.root);
    let successor = retirement.successor.as_ref().map(|s| format!("; its successor is {s}")).unwrap_or_default();
    Err(Error::RetiredProject(format!(
        "{} was retired on {}{successor}; run `tasks unretire` in {} to write here again",
        project.prefix, retirement.date, authority.display()
    )))
}
```

In `lock_and_revalidate`, after `reject_stale_local(&registry, &project)?;` add `reject_retired(&registry, &project)?;`. An unreadable registered root is not a reason to refuse a write here: `reject_stale_local` already owns that case.

Then audit the writes that bypass `save`/`create` (research list): `status.rs:84` halt-override audit note writes into the registered authority — it runs inside a command that already passed `lock_and_revalidate`, and the halt task is in the same project, so it inherits the refusal; `note_need_holders` (mod.rs:1196) writes notes on *other* projects' holder tasks — a retired project holds no claims (Task 4 guarantees none at retirement, and `start` is refused afterwards), so no holder can be retired; `feedback::guarded_update` (feedback.rs:306) — make sure the target project passes `reject_retired` under the lock at feedback.rs:121 before either the create or the recurrence update. Add a one-line comment at each of the first two saying why no check is needed.

- [ ] **Step 4: Run to verify they pass**

Run: the Step 2 command. Expected: PASS. Then `just test-fast`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/commands/mod.rs src/commands/feedback.rs src/commands/edit.rs src/commands/status.rs tests/cli.rs
git commit -m "feat(write): refuse writes to a retired project from every checkout"
```

### Task 3: `init` takes the project lock; `init --force` and `rename` refuse

**Files:**
- Modify: `src/commands/init.rs` (:9-40)
- Modify: `src/commands/rename.rs` (after the locks at :85-87 and :101-107)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `reject_retired` (Task 2), `MutationLock::acquire`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn init_waits_for_the_project_lock() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let held = hold_project_lock(&env, "old");
    let mut child = env.raw(&old).args(["init", "--prefix", "old", "--force"]).spawn().unwrap();
    let finished_early = wait_bounded(&mut child, Duration::from_millis(500));
    drop(held);
    let out = reap(child, REAP);
    assert!(!finished_early, "init --force must wait for the project lock");
    assert!(out.expect("reaped").status.success());
}

#[test]
fn init_force_and_rename_refuse_a_retired_project() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    assert_eq!(env.fail(&old, &["init", "--prefix", "old", "--force"]), "retired_project");
    assert_eq!(env.fail(&old, &["rename", "old", "older"]), "retired_project");
}

#[test]
fn unregister_still_works_on_a_retired_project() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    let here = env.init("here");
    env.json(&here, &["unregister", "old"]);
}
```

(`raw` returns a `Command`; use `.stdout(Stdio::piped()).stderr(Stdio::piped())` as the neighbouring lock tests do.)

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one init_waits_for_the_project_lock init_force_and_rename_refuse unregister_still_works_on_a_retired`
Expected: FAIL — init does not wait; init --force and rename succeed.

- [ ] **Step 3: Implement**

`init.rs`, before `let _lock = Registry::lock()?;`:

```rust
    // Lock order (retired-projects plan): the project lock, then the registry lock, as
    // rename takes them. `init --force` repoints the root `retire` reads.
    let _project_lock = crate::claims::MutationLock::acquire(&prefix)?;
```

and when `force` is set, before `registry.repoint`: if a config exists at the currently registered root, open it and `reject_retired(&registry, &project)?` (so re-pointing a retired prefix is refused). A plain `init` that registers an existing retired checkout on a new host is allowed: it writes no record.

`rename.rs`: after both `MutationLock`s and the registry lock are held on each path, open the source project and call `super::reject_retired(&registry, &project)?`.

- [ ] **Step 4: Run to verify they pass**

Run: the Step 2 command, then `just test-fast`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/init.rs src/commands/rename.rs tests/cli.rs
git commit -m "feat(init): take the project lock first; init --force and rename refuse a retired project"
```

### Task 4: `tasks retire` and `tasks unretire`

**Prerequisite:** ops-d0ef34 has landed on ops main (the `cli.toml` rows). Adopt them first: `python3 "$OPS/bin/vendored" adopt cli.toml` with `OPS` the ops checkout, which rewrites `tools/cli.toml`.

**Files:**
- Modify: `Cargo.toml` (add `toml_edit = "0.25"`), `tools/cli.toml` (adopted), `src/cli.rs` (variants beside `Unregister` :330), `src/commands/mod.rs` (dispatch arms in `run` :1620-1814, `pub mod retire;`), `src/output.rs` (`Output::Retire`, pretty arm, `warnings_of` arm)
- Modify: `src/repo.rs` (expose `worktree_roots` as `pub fn checkout_roots(&self) -> Result<Vec<PathBuf>>`)
- Create: `src/commands/retire.rs`
- Test: `tests/cli.rs`; `surface::tests::parser_surface_equals_table` covers the rows

**Interfaces:**
- Consumes: `Retirement`, `Project.retired` (Task 1); `Error::RetiredProject` (Task 2); `ClaimStore::load`, `claims::liveness`, `Ctx::describe_claim`; `Status::is_open`.
- Produces: `Command::Retire { successor: Option<String>, reason: String }`, `Command::Unretire`; JSON output `{"prefix": "...", "retired": {date, successor?, reason} | null, "warnings": []}`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn retire_writes_the_table_and_unretire_removes_it_keeping_comments() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let new = env.init("new");
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("# keep me\n{text}")).unwrap();
    let out = env.json(&old, &["retire", "--successor", "new", "--reason", "superseded"]);
    assert_eq!(out["retired"]["successor"], "new");
    let written = std::fs::read_to_string(&config).unwrap();
    assert!(written.contains("# keep me") && written.contains("[retired]"));
    assert_eq!(env.fail(&old, &["add", "N", "-p", "2"]), "retired_project");
    env.json(&old, &["unretire"]);
    let restored = std::fs::read_to_string(&config).unwrap();
    assert!(restored.contains("# keep me") && !restored.contains("[retired]"));
    env.json(&old, &["add", "N", "-p", "2"]);
    let _ = new;
}

#[test]
fn retire_refuses_an_open_task() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "open_tasks");
    env.json(&old, &["drop", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
}

#[test]
fn retire_refuses_its_own_prefix_or_an_unregistered_successor() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    for bad in ["old", "nowhere"] {
        assert_eq!(env.fail(&old, &["retire", "--successor", bad, "--reason", "r"]), "config", "{bad}");
    }
}

#[test]
fn retire_refuses_a_successor_that_is_itself_retired() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let mid = env.init("mid");
    env.json(&mid, &["retire", "--reason", "r"]);
    assert_eq!(env.fail(&old, &["retire", "--successor", "mid", "--reason", "r"]), "config");
}

#[test]
fn retire_refuses_a_live_claim_on_the_prefix() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["start", &id]);
    // Close the record without releasing the claim by editing the file directly, the way a
    // sync could leave it.
    let path = old.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap().replace("status: doing", "status: done");
    std::fs::write(&path, text).unwrap();
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "claimed");
}

#[test]
fn retire_refuses_an_open_task_that_exists_only_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    env.json(&main, &["drop", &id, "m"]);
    let only_here = id_of(env.json(&side, &["add", "W", "-p", "2"]));
    // No claim: `add` takes none.
    let out = env.cmd(&main).args(["retire", "--reason", "r"]).output().unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "open_tasks");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(&only_here) && detail.contains(side.to_str().unwrap()), "{detail}");
}

#[test]
fn retire_refuses_an_unreadable_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    env.json(&main, &["drop", &id, "m"]);
    std::fs::set_permissions(side.join("tasks"), std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let kind = env.fail(&main, &["retire", "--reason", "r"]);
    std::fs::set_permissions(side.join("tasks"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert_eq!(kind, "unreadable_checkout");
}

#[test]
fn retire_and_unretire_refuse_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    env.json(&main, &["drop", &id, "m"]);
    assert_eq!(env.fail(&side, &["retire", "--reason", "r"]), "not_registered_checkout");
    assert_eq!(env.fail(&side, &["unretire"]), "not_registered_checkout");
}

#[test]
fn a_writer_and_retire_racing_never_leave_an_open_task_in_a_retired_project() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    env.json(&main, &["drop", &id, "m"]);

    // Order 1: the writer holds the lock first. Retire waits, then sees the new task.
    let held = hold_project_lock(&env, "old");
    let mut writer = env.raw(&side);
    writer.args(["add", "W", "-p", "2"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let writer = writer.spawn().unwrap();
    let mut retire = env.raw(&main);
    retire.args(["retire", "--reason", "r"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut retire = retire.spawn().unwrap();
    let retire_waited = !wait_bounded(&mut retire, Duration::from_millis(300));
    drop(held);
    let (w, r) = (reap(writer, REAP), reap(retire, REAP));
    assert!(retire_waited, "retire must wait for the project lock");
    let (w, r) = (w.expect("writer reaped"), r.expect("retire reaped"));
    if w.status.success() {
        assert!(!r.status.success(), "retire must refuse once the writer's task exists");
        assert!(String::from_utf8_lossy(&r.stderr).contains("open_tasks"));
    } else {
        assert!(String::from_utf8_lossy(&w.stderr).contains("retired_project"));
    }

    // Whichever won, the end state holds: either old is retired with no open task in any
    // checkout, or it is not retired.
    let rows = env.json(&main, &["projects"]);
    let row = rows["projects"].as_array().unwrap().iter().find(|r| r["prefix"] == "old").unwrap();
    if !row["retired"].is_null() {
        assert_eq!(env.fail(&side, &["add", "X", "-p", "2"]), "retired_project");
    }
}
```

Error kinds: reuse `claimed` (from `rename::authorize`'s `Error::Claimed`); add `open_tasks`, `unreadable_checkout`, `not_registered_checkout` as new variants in `src/error.rs` if no existing kind fits (grep `kind()` first: rename's dirty-tree and second-worktree refusals may already have suitable kinds — reuse one if its meaning matches exactly). A bad successor is `Error::Config`.

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one retire_ a_writer_and_retire_racing`
Expected: FAIL — `retire` is an unknown subcommand.

- [ ] **Step 3: Implement**

`src/commands/retire.rs`:

```rust
//! `tasks retire` / `tasks unretire` (ops lifecycle spec §3–§4). Lock order: this
//! project's MutationLock only; the registry is read, never locked, because every command
//! that changes this prefix's root (`init --force`, `rename`) takes this lock first.

use crate::claims::{ClaimStore, MutationLock};
use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::{Project, Retirement, CONFIG_REL};

pub fn retire(project: Project, successor: Option<String>, reason: String) -> Result<Retirement> {
    let _lock = MutationLock::acquire(&project.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &project)?;
    if let Some(s) = &successor {
        let live = registry.canonical_prefix(s);
        let ok = live != project.prefix
            && registry.project_root(live).is_some()
            && crate::scope::open_registered(&registry, live, crate::scope::Origin::Prefix)
                .map(|p| p.retired.is_none())
                .unwrap_or(false);
        if !ok {
            return Err(Error::Config(format!(
                "successor {s:?} must be a registered, unretired prefix other than {}", project.prefix
            )));
        }
    }
    reject_live_claims(&project)?;
    reject_open_copies(&project)?;
    let retirement = Retirement { date: crate::time::today(), successor, reason };
    write_table(&project, Some(&retirement))?;
    Ok(retirement)
}

pub fn unretire(project: Project) -> Result<()> {
    let _lock = MutationLock::acquire(&project.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &project)?;
    write_table(&project, None)
}

/// The registered checkout, re-opened under the lock; a linked worktree is refused.
fn authority(registry: &Registry, here: &Project) -> Result<Project> {
    let root = registry.project_root(&here.prefix).ok_or_else(|| {
        Error::Config(format!("{} is not registered on this host", here.prefix))
    })?;
    if root != here.root.as_path() {
        return Err(Error::NotRegisteredCheckout(format!(
            "run this in the registered checkout of {}: {}", here.prefix, root.display()
        )));
    }
    Project::open(root)
}

fn reject_live_claims(project: &Project) -> Result<()> {
    let store = ClaimStore::load(&project.prefix)?;
    for claim in store.iter() {
        if matches!(crate::claims::liveness(claim), crate::claims::Liveness::Live) {
            return Err(Error::Claimed(claim.id.clone(), super::Ctx::describe_claim(claim)));
        }
    }
    Ok(())
}

/// Every checkout's copy of tasks/, the registered one included: a todo filed in a worktree
/// and never merged is open work too.
fn reject_open_copies(project: &Project) -> Result<()> {
    let mut open = Vec::new();
    for root in project.checkout_roots()? {
        let dir = root.join("tasks");
        let entries = std::fs::read_dir(&dir).map_err(|e| {
            Error::UnreadableCheckout(format!("{}: {e}", dir.display()))
        })?;
        for entry in entries {
            let path = entry.map_err(|e| Error::UnreadableCheckout(format!("{}: {e}", dir.display())))?.path();
            if path.extension().is_some_and(|x| x == "md") {
                let task = crate::repo::read_task_file(&path)
                    .map_err(|e| Error::UnreadableCheckout(format!("{}: {e}", path.display())))?;
                if task.status.is_open() {
                    open.push(format!("{} ({}) in {}", task.id, task.status, root.display()));
                }
            }
        }
    }
    if open.is_empty() { Ok(()) } else {
        Err(Error::OpenTasks(format!("close these first: {}", open.join(", "))))
    }
}

fn write_table(project: &Project, retirement: Option<&Retirement>) -> Result<()> {
    let path = project.root.join(CONFIG_REL);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text.parse()
        .map_err(|e| Error::Config(format!("{CONFIG_REL}: {e}")))?;
    match retirement {
        Some(r) => {
            let mut table = toml_edit::Table::new();
            table["date"] = toml_edit::value(&r.date);
            if let Some(s) = &r.successor { table["successor"] = toml_edit::value(s); }
            table["reason"] = toml_edit::value(&r.reason);
            doc["retired"] = toml_edit::Item::Table(table);
        }
        None => { doc.remove("retired"); }
    }
    crate::repo::atomic_write(&path, doc.to_string().as_bytes())?;
    Ok(())
}
```

Names to confirm against the code and use as they are: `crate::time::today()` (whatever `defer` uses for today's date), `crate::repo::read_task_file` (the parser `sibling_task_copies` uses per file), `crate::repo::atomic_write` (rename uses one), `claim.id`. `checkout_roots` wraps the private `worktree_roots` (repo.rs:507-554), which already returns the main root first and keeps a missing worktree's raw path — a missing path then fails `read_dir` and is reported as unreadable, as the spec requires.

`unretire` on a project that is not retired succeeds and changes nothing (idempotent; the output's `retired` is null).

`cli.rs`:

```rust
    /// Declare this project retired: no task may be open, no claim live, in any checkout
    Retire {
        #[arg(long)]
        successor: Option<String>,
        #[arg(long)]
        reason: String,
    },
    /// Remove this project's retirement declaration
    Unretire,
```

Dispatch: `Command::Retire { successor, reason } => retire::retire(open_ctx(dir)?.project, successor, reason).map(...)` into a new `Output::Retire(RetireOut { prefix, retired: Option<Retirement>, warnings })`; pretty: `old retired 2026-10-04 → new: superseded` / `old is not retired`.

- [ ] **Step 4: Run to verify they pass**

Run: `just test-one retire_ a_writer_and_retire_racing parser_surface_equals_table`, then `just test-fast`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock tools/cli.toml src/cli.rs src/commands/mod.rs src/commands/retire.rs src/output.rs src/repo.rs src/error.rs tests/cli.rs
git commit -m "feat(retire): tasks retire and unretire, scanning every checkout under the project lock"
```

### Task 5: Reads — wide reads skip, single reads warn, `projects` reports, `group set` refuses

**Files:**
- Modify: `src/scope.rs` (`Scope::open_members` :115-145)
- Modify: `src/commands/mod.rs` (`open_read_ctx` Local arm :808-827, `open_id_read_ctx` :737-742, `open_id_ctx` :629)
- Modify: `src/commands/projects.rs` (:65-107), `src/output.rs` (`ProjectRow` :91-107, grid :983-1024)
- Modify: `src/registry.rs` (`set_group` :330-352) or its caller in `src/commands/group.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Project.retired`.
- Produces: `ProjectRow.retired: Option<Retirement>` serialized as `"retired": {...}` when set; `pub fn retired_warning(prefix: &str, r: &Retirement) -> String` in `commands/mod.rs` returning `"{prefix} is retired ({date})"` + `"; successor {s}"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn wide_reads_skip_a_retired_project_without_a_warning() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let closed = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &closed, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    env.json(&live, &["add", "L", "-p", "2"]);
    for args in [
        vec!["list", "--all-projects", "--status", "dropped"],
        vec!["ready", "--all-projects"],
        vec!["prime", "--all-projects"],
        vec!["tags", "--all-projects"],
        vec!["sample", "--all-projects"],
        vec!["quiet"],
    ] {
        let out = env.json(&live, &args);
        let text = out.to_string();
        assert!(!text.contains("\"old"), "{args:?}: {text}");
        assert!(warnings_of(&out).iter().all(|w| !w.contains("old")), "{args:?}");
    }
    env.json(&live, &["group", "set", "pair", "live"]);
    assert_eq!(env.fail(&live, &["group", "set", "both", "live", "old"]), "retired_project");
}

#[test]
fn single_project_reads_and_show_work_and_warn() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &id, "m"]);
    env.json(&old, &["retire", "--successor", "live", "--reason", "r"]);
    let shown = env.json(&live, &["show", &id]);
    assert_eq!(shown["task"]["id"], id.as_str());
    assert!(warnings_of(&shown).iter().any(|w| w.contains("old is retired") && w.contains("successor live")));
    let listed = env.json(&live, &["list", "--project", "old", "--status", "dropped"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert!(warnings_of(&listed).iter().any(|w| w.contains("old is retired")));
}

#[test]
fn a_successor_that_is_gone_or_retired_does_not_break_reads() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let mid = env.init("mid");
    env.json(&old, &["retire", "--successor", "mid", "--reason", "r"]);
    env.json(&mid, &["retire", "--reason", "r"]);
    let here = env.init("here");
    env.json(&here, &["projects"]);
    env.json(&here, &["unregister", "mid"]);
    let rows = env.json(&here, &["projects"]);
    let row = rows["projects"].as_array().unwrap().iter().find(|r| r["prefix"] == "old").unwrap();
    assert_eq!(row["retired"]["successor"], "mid");
}

#[test]
fn projects_reports_retirement_and_keeps_counts() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["done", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    let rows = env.json(&old, &["projects"]);
    let row = rows["projects"].as_array().unwrap().iter().find(|r| r["prefix"] == "old").unwrap();
    assert_eq!(row["retired"]["reason"], "r");
    assert_eq!(row["counts"]["done"], 1);
    assert!(env.pretty(&old, &["projects"]).contains("retired"));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one wide_reads_skip single_project_reads a_successor_that_is_gone projects_reports_retirement`
Expected: FAIL.

- [ ] **Step 3: Implement**

`Scope::open_members`: after a member opens successfully, `if project.retired.is_some() { continue; }` (no warning). Single-project read arms: when `ctx.project.retired` is set, push `retired_warning(..)` onto the context's warnings; for `show` of an id in another project, push the same for the project that answered. `projects.rs`: set `retired: project.retired.clone()` on the row; pretty grid appends ` retired` to the activity cell. `group set`: for each member, open its registered root (`open_registered`) and return `Error::RetiredProject(format!("{m} is retired; a group lists active projects"))` when `retired` is set; an unreachable member keeps today's behaviour.

- [ ] **Step 4: Run to verify they pass**

Run: the Step 2 command, then `just test-fast`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/scope.rs src/commands/mod.rs src/commands/projects.rs src/commands/group.rs src/registry.rs src/output.rs tests/cli.rs
git commit -m "feat(read): wide reads skip retired projects; single reads warn; projects reports retirement"
```

### Task 6: Documentation and the gate

**Files:**
- Modify: `README.md` (registry and commands sections; error kinds beside `stale_copy`)
- Modify: `skills/tasks/SKILL.md` (a short "Retiring a project" paragraph under Prefix renames and recovery; `retired_project` in the write rules)
- Modify: `AGENTS.md` only if it lists commands

- [ ] **Step 1: Write the docs**

Skill paragraph (verbatim):

```markdown
## Retiring a project

A project that is over declares it: close every task (drop, or re-file in the successor
with `--source <old id>`), then `tasks retire [--successor <prefix>] --reason "<one line>"`
in its registered checkout. Every write to it is refused afterwards with `retired_project`,
from every checkout; its ids keep resolving, `--all-projects` and `--group` reads leave it
out, and `--project` reads warn. `tasks unretire` there undoes it. ops's
`docs/runbooks/project-lifecycle.md` has the whole procedure.
```

- [ ] **Step 2: Run the gate**

Run: `just check` then `just test-fast`. Expected: both pass (`tasks check` clean, clippy clean).

- [ ] **Step 3: Commit**

```bash
git add README.md skills/tasks/SKILL.md
git commit -m "docs: retiring a project"
```
