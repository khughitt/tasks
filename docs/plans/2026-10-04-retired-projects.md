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
- `just test-one` takes one Cargo filter. Every new test's name starts with its task's marker (`retired_table_`, `retired_write_`, `retired_lock_`, `retire_cmd_`, `retired_read_`), so one filter runs a task's tests.
- A failure to read a retirement declaration never means "active": the error propagates, and the write it guards does not happen.

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
3. A writer and `retire` racing → either the write lands and `retire` sees it and refuses, or `retire` lands and the write is refused; never a written open task in a retired project (Task 4: one test per order, each with a single child behind the test's lock, plus `retire` waiting for the lock).
4. A stored successor that is unregistered on this host, or itself retired → every read still works and shows the string (Task 1 and Task 5 tests).
5. `edit` with no flags (the `$EDITOR` path) drops and re-takes the lock → refused at the re-take if retirement landed meanwhile (Task 2 test with a scripted editor).
6. A registered checkout whose config is unparsable or names another prefix → a worktree write is refused with `config` and changes nothing; `retire` given an invalid reason writes nothing (Task 2, Task 4).

---

### Task 1: The `[retired]` table

**Files:**
- Modify: `src/repo.rs` (`Config` :106-123, a new `RetiredConfig`, a validator beside `feedback_scope` :183-194, `Project` :68-89, `Project::open` :259-288)
- Modify: `src/commands/check.rs` (per-task loop from :64)
- Test: unit tests in `src/repo.rs`'s `#[cfg(test)]` module (named `retired_table_*`, so one filter runs them with the CLI tests); `tests/cli.rs`

**Interfaces:**
- Produces: `pub struct Retirement { pub date: String, pub successor: Option<String>, pub reason: String }` (derive `Clone, Debug, PartialEq, serde::Serialize`); `Project.retired: Option<Retirement>`. Warning text is Task 5's.

- [ ] **Step 1: Write the failing tests**

In `src/repo.rs` tests:

```rust
#[test]
fn retired_table_is_read_with_and_without_a_successor() {
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
fn retired_table_bad_is_a_config_error() {
    for body in [
        "[retired]\nreason = \"r\"\n",                                  // no date
        "[retired]\ndate = \"2026-10-04\"\n",                           // no reason
        "[retired]\ndate = \"04/10/2026\"\nreason = \"r\"\n",           // not ISO
        "[retired]\ndate = \"2026-02-30\"\nreason = \"r\"\n",           // no such day
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
fn retired_table_check_reports_an_open_task() {
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

Run: `just test-one retired_table_`
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
/// asked once, so a host without the successor still reads the table. `tasks retire`
/// calls this too, before it writes, so it never writes a table the next command would
/// refuse to read.
pub fn validate_retirement(r: &Retirement) -> Result<()> {
    let bad = |what: String| Err(Error::Config(format!("{CONFIG_REL}: [retired] {what}")));
    if !is_iso_date(&r.date) {
        return bad("date must be YYYY-MM-DD".into());
    }
    if r.reason.chars().any(char::is_control) || r.reason.chars().all(char::is_whitespace) {
        return bad("reason must be one line of text with no control characters".into());
    }
    if let Some(successor) = &r.successor {
        if !is_valid_prefix(successor) {
            return bad(format!("successor {successor:?} is not a prefix"));
        }
    }
    Ok(())
}

/// `YYYY-MM-DD` naming a real calendar day, with the `time` crate the project already uses.
fn is_iso_date(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) =
        (text[0..4].parse::<i32>(), text[5..7].parse::<u8>(), text[8..10].parse::<u8>())
    else {
        return false;
    };
    time::Month::try_from(m).is_ok_and(|m| time::Date::from_calendar_date(y, m, d).is_ok())
}

fn retirement(raw: Option<RetiredConfig>) -> Result<Option<Retirement>> {
    let Some(table) = raw else {
        return Ok(None);
    };
    let r = Retirement { date: table.date, successor: table.successor, reason: table.reason };
    validate_retirement(&r)?;
    Ok(Some(r))
}
```

If `src/` already has a `YYYY-MM-DD` validator (the `defer` parser), use it in place of `is_iso_date`. Add `pub retired: Option<Retirement>` to `Project` and `retired: retirement(config.retired)?,` in `Project::open`. Every other `Project { .. }` literal (grep `Project {`) gets `retired: None`.

In `check.rs`, inside the per-task loop:

```rust
if ctx.project.retired.is_some() && task.status.is_open() {
    errors.push(Finding::new(&task, "open_in_retired",
        format!("{} is {} in a retired project; close it or `tasks unretire`", task.id, task.status)));
}
```

(match the existing `Finding` constructor's shape at the nearest existing push.)

- [ ] **Step 4: Run to verify they pass**

Run: `just test-one retired_table_`
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
fn retired_write_every_command_is_refused_in_the_registered_checkout() {
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
fn retired_write_a_worktree_made_before_retirement_cannot_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&main, None); // only the registered checkout knows
    assert!(!std::fs::read_to_string(side.join("tasks/.config.toml")).unwrap().contains("[retired]"));
    for args in [vec!["add", "N", "-p", "2"], vec!["note", &id, "x"], vec!["done", &id, "m"]] {
        assert_eq!(env.fail(&side, &args), "retired_project", "{args:?}");
    }
}

#[test]
fn retired_write_attach_and_detach_are_refused() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    let file = old.join("shot.txt");
    std::fs::write(&file, "x").unwrap();
    env.json(&old, &["attach", &id, file.to_str().unwrap()]);
    declare_retired(&old, None);
    assert_eq!(env.fail(&old, &["attach", &id, file.to_str().unwrap()]), "retired_project");
    assert_eq!(env.fail(&old, &["detach", &id, "shot.txt", "why"]), "retired_project");
}

#[test]
fn retired_write_an_unreadable_registered_checkout_refuses_a_worktree_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    let config = main.join("tasks/.config.toml");
    let good = std::fs::read(&config).unwrap();
    let before = std::fs::read(side.join(format!("tasks/{id}.md"))).unwrap();
    for broken in [&b"prefix = \"old\"\n[retired]\nreason = 3\n"[..], &b"prefix = \"other\"\n"[..]] {
        std::fs::write(&config, broken).unwrap();
        assert_eq!(env.fail(&side, &["note", &id, "x"]), "config");
        assert_eq!(std::fs::read(side.join(format!("tasks/{id}.md"))).unwrap(), before);
    }
    std::fs::write(&config, &good).unwrap();
    env.json(&side, &["note", &id, "x"]);
}

#[test]
fn retired_write_a_worktree_that_declares_it_refuses() {
    let mut env = TestEnv::new();
    let (_main, side, _id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&side, None);
    assert_eq!(env.fail(&side, &["add", "N", "-p", "2"]), "retired_project");
}

#[test]
fn retired_write_feedback_is_refused() {
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
fn retired_write_refusal_names_the_successor_and_the_way_back() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, Some("new"));
    let out = env.cmd(&old).args(["add", "N", "-p", "2"]).output().unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("new") && detail.contains("tasks unretire"), "{detail}");
}

#[test]
fn retired_write_editor_save_is_refused_after_retirement_lands() {
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

Run: `just test-one retired_write_`
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
/// until a merge, so either declaring it is enough. A registered checkout that cannot be
/// opened, or whose config names another prefix, is an error, never "not retired": the
/// write it would have guarded does not happen.
pub fn reject_retired(registry: &Registry, project: &Project) -> Result<()> {
    let registered = match registry.project_root(&project.prefix) {
        Some(root) if root != project.root.as_path() => Some(
            crate::scope::open_registered(registry, &project.prefix, Origin::Prefix).map_err(|error| {
                Error::Config(format!(
                    "cannot tell whether {} is retired: its registered checkout {} did not open ({error}); \
                     fix that checkout, then retry",
                    project.prefix,
                    root.display()
                ))
            })?,
        ),
        _ => None,
    };
    let declared = project
        .retired
        .as_ref()
        .or(registered.as_ref().and_then(|p| p.retired.as_ref()));
    let Some(retirement) = declared else { return Ok(()) };
    let authority = registry.project_root(&project.prefix).unwrap_or(&project.root);
    let successor = retirement
        .successor
        .as_ref()
        .map(|s| format!("; its successor is {s}"))
        .unwrap_or_default();
    Err(Error::RetiredProject(format!(
        "{} was retired on {}{successor}; run `tasks unretire` in {} to write here again",
        project.prefix,
        retirement.date,
        authority.display()
    )))
}
```

In `lock_and_revalidate`, after `reject_stale_local(&registry, &project)?;` add `reject_retired(&registry, &project)?;`. `reject_stale_local` only compares the prefix with the registry's aliases; it says nothing about whether the registered root can be read, so that case is `reject_retired`'s, and it refuses. `open_registered` already checks that the registered config exists and names this prefix, which is the identity check.

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
fn retired_lock_init_waits_for_the_project_lock() {
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
fn retired_lock_init_force_and_rename_refuse() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    assert_eq!(env.fail(&old, &["init", "--prefix", "old", "--force"]), "retired_project");
    assert_eq!(env.fail(&old, &["rename", "old", "older"]), "retired_project");
}

#[test]
fn retired_lock_unregister_still_works() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    let here = env.init("here");
    env.json(&here, &["unregister", "old"]);
}
```

(`raw` returns a `Command`; use `.stdout(Stdio::piped()).stderr(Stdio::piped())` as the neighbouring lock tests do.)

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one retired_lock_`
Expected: FAIL — init does not wait; init --force and rename succeed.

- [ ] **Step 3: Implement**

`init.rs`, before `let _lock = Registry::lock()?;`:

```rust
    // Lock order (retired-projects plan): the project lock, then the registry lock, as
    // rename takes them. `init --force` repoints the root `retire` reads.
    let _project_lock = crate::claims::MutationLock::acquire(&prefix)?;
```

and when `force` is set, before `registry.repoint`: refuse with `retired_project` when the new root's config declares `[retired]`, or when the currently registered root opens and declares it. Re-pointing moves the authority `retire` and every writer consult, so it is not done for a retired project. An old root that does not open does not block: repointing away from a moved or broken checkout is what `init --force` is for, and the new root is read either way. A plain `init` that registers an existing retired checkout on a new host is allowed: it writes no record.

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
- Modify: `Cargo.toml` (add `toml_edit = "0.25"`; the local cargo cache holds 0.25.x), `tools/cli.toml` (adopted), `src/cli.rs` (variants beside `Unregister` :330), `src/commands/mod.rs` (dispatch arms in `run` :1620-1814, `pub mod retire;`), `src/output.rs` (`Output::Retire`, pretty arm, `warnings_of` arm), `src/error.rs` (new kinds)
- Modify: `src/repo.rs` (expose the private `worktree_roots` :507-554 as `pub fn checkout_roots(&self) -> Result<Vec<PathBuf>>`)
- Create: `src/commands/retire.rs`
- Test: `tests/cli.rs`; `surface::tests::parser_surface_equals_table` covers the rows

**Interfaces:**
- Consumes: `Retirement`, `Project.retired`, `validate_retirement` (Task 1); `Error::RetiredProject`, `reject_retired` (Task 2); `ClaimStore::load(prefix)` and `ClaimStore::iter() -> impl Iterator<Item = (&String, &Claim)>` (claims.rs:374, :510); `claims::liveness(&Claim) -> Liveness` (:798); `Ctx::describe_claim(&Claim, &Liveness) -> String` (mod.rs:156); `reject_pending_rename(&Project)` (mod.rs:699); `crate::format::parse_task(&str, &str) -> Result<Task>`; `crate::repo::atomic_write(&Path, &[u8])` (`pub(crate)`, repo.rs:12); `Status::is_open`.
- Produces: `Command::Retire { successor: Option<String>, reason: String }`, `Command::Unretire`; JSON `{"prefix": "...", "retired": {date, successor?, reason} | null, "warnings": []}`; error kinds `open_tasks`, `unreadable_checkout`, `not_registered_checkout` (reuse an existing kind only if its meaning matches exactly).

- [ ] **Step 1: Write the failing tests**

```rust
/// A project that is its own git repository with one *closed* task committed, plus a
/// linked worktree branched after the close, so every checkout's copy is closed.
fn closed_repo_with_worktree(env: &mut TestEnv, prefix: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let main = env.init(prefix);
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    env.json(&main, &["drop", &id, "seed"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(&main, &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()]);
    (main, side)
}

fn spawn_piped(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> std::process::Child {
    let mut command = env.raw(dir);
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command.spawn().unwrap()
}

fn kind_of(output: &std::process::Output) -> String {
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    error["error"]["kind"].as_str().unwrap().to_string()
}

#[test]
fn retire_cmd_writes_the_table_and_unretire_removes_it_keeping_comments() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.init("new");
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
}

#[test]
fn retire_cmd_unretire_on_an_active_project_changes_nothing() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let before = std::fs::read(old.join("tasks/.config.toml")).unwrap();
    let out = env.json(&old, &["unretire"]);
    assert!(out["retired"].is_null());
    assert_eq!(std::fs::read(old.join("tasks/.config.toml")).unwrap(), before);
}

#[test]
fn retire_cmd_an_invalid_reason_leaves_the_config_unchanged() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let config = old.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    for reason in ["", "   ", "two\nlines", "tab\there"] {
        assert_eq!(env.fail(&old, &["retire", "--reason", reason]), "config", "{reason:?}");
        assert_eq!(std::fs::read(&config).unwrap(), before, "{reason:?}");
    }
    env.json(&old, &["add", "still writable", "-p", "2"]);
}

#[test]
fn retire_cmd_refuses_an_open_task() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "open_tasks");
    env.json(&old, &["drop", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
}

#[test]
fn retire_cmd_refuses_its_own_prefix_an_unregistered_or_a_retired_successor() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let mid = env.init("mid");
    env.json(&mid, &["retire", "--reason", "r"]);
    for bad in ["old", "nowhere", "mid"] {
        assert_eq!(env.fail(&old, &["retire", "--successor", bad, "--reason", "r"]), "config", "{bad}");
    }
}

#[test]
fn retire_cmd_refuses_a_live_claim_on_the_prefix() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["start", &id]);
    // Close the record without releasing the claim, the way a sync can leave it.
    let path = old.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap().replace("status: doing", "status: done");
    std::fs::write(&path, text).unwrap();
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "claimed");
}

#[test]
fn retire_cmd_refuses_an_open_task_that_exists_only_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let only_here = id_of(env.json(&side, &["add", "W", "-p", "2"])); // `add` takes no claim
    let out = env.cmd(&main).args(["retire", "--reason", "r"]).output().unwrap();
    assert_eq!(kind_of(&out), "open_tasks");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(&only_here) && detail.contains(side.to_str().unwrap()), "{detail}");
}

#[test]
fn retire_cmd_refuses_an_unreadable_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(side.join("tasks"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let kind = env.fail(&main, &["retire", "--reason", "r"]);
    std::fs::set_permissions(side.join("tasks"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(kind, "unreadable_checkout");
    assert!(!std::fs::read_to_string(main.join("tasks/.config.toml")).unwrap().contains("[retired]"));
}

#[test]
fn retire_cmd_and_unretire_refuse_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (_main, side) = closed_repo_with_worktree(&mut env, "old");
    assert_eq!(env.fail(&side, &["retire", "--reason", "r"]), "not_registered_checkout");
    assert_eq!(env.fail(&side, &["unretire"]), "not_registered_checkout");
}

#[test]
fn retire_cmd_and_unretire_respect_a_pending_rename() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.json(&dir, &["drop", &id, "m"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();
    let config = dir.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    // Same kind a write gets during the freeze (a_pending_rename_freezes_the_project_*).
    assert_eq!(env.fail(&dir, &["retire", "--reason", "r"]), "validation");
    assert_eq!(env.fail(&dir, &["unretire"]), "validation");
    assert_eq!(std::fs::read(&config).unwrap(), before);
}

/// Retire takes the project lock: it waits behind a holder and finishes after release.
#[test]
fn retire_cmd_waits_for_the_project_lock() {
    let mut env = TestEnv::new();
    let (main, _side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut retire = spawn_piped(&env, &main, &["retire", "--reason", "r"]);
    let finished_early = wait_bounded(&mut retire, Duration::from_millis(500));
    drop(held);
    let out = reap(retire, REAP);
    assert!(!finished_early, "retire must wait for the project lock");
    assert!(out.expect("reaped").status.success());
}

/// Writer first: a stale-worktree write that holds the lock lands, and the retire that
/// follows it through the lock sees the new task and refuses.
#[test]
fn retire_cmd_after_a_worktree_writer_sees_its_task() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut writer = spawn_piped(&env, &side, &["add", "W", "-p", "2"]);
    let writer_waited = !wait_bounded(&mut writer, Duration::from_millis(300));
    drop(held);
    let writer = reap(writer, REAP).expect("writer reaped");
    assert!(writer_waited && writer.status.success());
    let retire = env.cmd(&main).args(["retire", "--reason", "r"]).output().unwrap();
    assert_eq!(kind_of(&retire), "open_tasks");
}

/// Retire first: once it holds the lock and lands, a writer queued behind it in a stale
/// worktree (whose own config has no [retired]) is refused.
#[test]
fn retire_cmd_before_a_worktree_writer_refuses_it() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut retire = spawn_piped(&env, &main, &["retire", "--reason", "r"]);
    let retire_waited = !wait_bounded(&mut retire, Duration::from_millis(300));
    drop(held);
    let retire = reap(retire, REAP).expect("retire reaped");
    assert!(retire_waited && retire.status.success());
    assert!(!std::fs::read_to_string(side.join("tasks/.config.toml")).unwrap().contains("[retired]"));
    let writer = env.cmd(&side).args(["add", "W", "-p", "2"]).output().unwrap();
    assert_eq!(kind_of(&writer), "retired_project");
    assert!(std::fs::read_dir(side.join("tasks")).unwrap().all(|e| {
        let path = e.unwrap().path();
        path.extension().is_none_or(|x| x != "md")
            || !std::fs::read_to_string(&path).unwrap().contains("title: W")
    }));
}
```

The two ordered tests are deterministic: each has one child behind the test's own lock, and the other side runs after that child has finished. Together with `retire_cmd_waits_for_the_project_lock` and the existing `a_write_command_waits_for_the_project_lock_and_a_read_command_does_not`, they show that `retire`'s scan-then-write and every writer's check-then-write run under the same lock, which is what excludes the interleaving. Check the record's `title:` line form against an existing record before relying on the last assertion.

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one retire_cmd_`
Expected: FAIL — `retire` is an unknown subcommand.

- [ ] **Step 3: Implement**

`src/commands/retire.rs`:

```rust
//! `tasks retire` / `tasks unretire` (ops lifecycle spec §3–§4). Lock order: this
//! project's MutationLock only; the registry is read, never locked, because every command
//! that changes this prefix's root (`init --force`, `rename`) takes this lock first.

use crate::claims::{ClaimStore, Liveness, MutationLock};
use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::{validate_retirement, Project, Retirement, CONFIG_REL};
use crate::scope::Origin;

pub fn retire(here: Project, successor: Option<String>, reason: String) -> Result<Retirement> {
    let _lock = MutationLock::acquire(&here.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &here)?;
    let date = time::OffsetDateTime::now_utc().date().to_string(); // Date's Display is YYYY-MM-DD
    let retirement = Retirement { date, successor, reason };
    validate_retirement(&retirement)?;
    if let Some(s) = &retirement.successor {
        let live = registry.canonical_prefix(s);
        let usable = live != project.prefix
            && registry.project_root(live).is_some()
            && crate::scope::open_registered(&registry, live, Origin::Prefix)?.retired.is_none();
        if !usable {
            return Err(Error::Config(format!(
                "successor {s:?} must be a registered, unretired prefix other than {}",
                project.prefix
            )));
        }
    }
    reject_live_claims(&project)?;
    reject_open_copies(&project)?;
    write_table(&project, Some(&retirement))?;
    Ok(retirement)
}

pub fn unretire(here: Project) -> Result<Option<Retirement>> {
    let _lock = MutationLock::acquire(&here.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &here)?;
    let was = project.retired.clone();
    if was.is_some() {
        write_table(&project, None)?;
    }
    Ok(None)
}

/// The registered checkout, re-opened under the lock, with the pending-rename freeze
/// applied as every writer applies it; a linked worktree is refused.
fn authority(registry: &Registry, here: &Project) -> Result<Project> {
    super::reject_pending_rename(here)?;
    let root = registry
        .project_root(&here.prefix)
        .ok_or_else(|| Error::Config(format!("{} is not registered on this host", here.prefix)))?;
    if root != here.root.as_path() {
        return Err(Error::NotRegisteredCheckout(format!(
            "run this in the registered checkout of {}: {}",
            here.prefix,
            root.display()
        )));
    }
    let project = Project::open(root)?;
    super::reject_stale_local(registry, &project)?;
    Ok(project)
}

fn reject_live_claims(project: &Project) -> Result<()> {
    let store = ClaimStore::load(&project.prefix)?;
    for (id, claim) in store.iter() {
        let live = crate::claims::liveness(claim);
        if matches!(live, Liveness::Live) {
            return Err(Error::Claimed(id.clone(), super::Ctx::describe_claim(claim, &live)));
        }
    }
    Ok(())
}

/// Every checkout's tasks/, the registered one included: a todo filed in a worktree and
/// never merged is open work too. Anything that cannot be read stops the retirement.
fn reject_open_copies(project: &Project) -> Result<()> {
    let unreadable = |what: &std::path::Path, e: &dyn std::fmt::Display| {
        Error::UnreadableCheckout(format!("{}: {e}", what.display()))
    };
    let mut open = Vec::new();
    for root in project.checkout_roots()? {
        let dir = root.join("tasks");
        for entry in std::fs::read_dir(&dir).map_err(|e| unreadable(&dir, &e))? {
            let path = entry.map_err(|e| unreadable(&dir, &e))?.path();
            if path.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).map_err(|e| unreadable(&path, &e))?;
            let file = format!("tasks/{}", path.file_name().unwrap().to_string_lossy());
            let task = crate::format::parse_task(&raw, &file).map_err(|e| unreadable(&path, &e))?;
            if task.status.is_open() {
                open.push(format!("{} ({}) in {}", task.id, task.status, root.display()));
            }
        }
    }
    if open.is_empty() {
        Ok(())
    } else {
        Err(Error::OpenTasks(format!("close these first: {}", open.join(", "))))
    }
}

fn write_table(project: &Project, retirement: Option<&Retirement>) -> Result<()> {
    let path = project.root.join(CONFIG_REL);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| Error::Config(format!("{CONFIG_REL}: {e}")))?;
    match retirement {
        Some(r) => {
            let mut table = toml_edit::Table::new();
            table["date"] = toml_edit::value(&r.date);
            if let Some(s) = &r.successor {
                table["successor"] = toml_edit::value(s);
            }
            table["reason"] = toml_edit::value(&r.reason);
            doc["retired"] = toml_edit::Item::Table(table);
        }
        None => {
            doc.remove("retired");
        }
    }
    crate::repo::atomic_write(&path, doc.to_string().as_bytes())
}
```

`checkout_roots` wraps `worktree_roots`, which returns the main root first and keeps a missing worktree's raw path; a missing path then fails `read_dir` and is reported as unreadable, as the spec requires. `TaskId` and `Status` must implement `Display` for the `format!` above; use their existing string forms if they do not. The pending-rename kind is whatever `reject_pending_rename` returns today (`validation` in the existing freeze test); keep it.

`src/error.rs`: `OpenTasks(String) => "open_tasks"`, `UnreadableCheckout(String) => "unreadable_checkout"`, `NotRegisteredCheckout(String) => "not_registered_checkout"`, each with its `with_suffix` arm.

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

Dispatch: `Command::Retire { successor, reason }` → `retire::retire(open_ctx(dir)?.project, successor, reason)` into `Output::Retire(RetireOut { prefix, retired: Some(r), warnings: vec![] })`; `Command::Unretire` → `RetireOut { retired: None, .. }`. Pretty: `old retired 2026-10-04 → new: superseded` / `old is not retired`.

- [ ] **Step 4: Run to verify they pass**

Run, one at a time: `just test-one retire_cmd_`, then `just test-one parser_surface_equals_table`, then `just test-fast`.
Expected: PASS.

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
fn retired_read_wide_reads_skip_it_without_a_warning() {
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
fn retired_read_a_group_made_before_retirement_skips_the_member() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    env.json(&live, &["group", "set", "pair", "live", "old"]);
    let closed = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &closed, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    env.json(&live, &["add", "L", "-p", "2"]);
    for args in [vec!["ready", "--group", "pair"], vec!["list", "--group", "pair", "--status", "dropped"]] {
        let out = env.json(&live, &args);
        assert!(!out.to_string().contains("\"old-"), "{args:?}");
        assert!(warnings_of(&out).iter().all(|w| !w.contains("old")), "{args:?}");
    }
}

#[test]
fn retired_read_a_dependency_on_a_retired_task_resolves_and_never_blocks() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let dep = id_of(env.json(&old, &["add", "D", "-p", "2"]));
    let waiter = id_of(env.json(&live, &["add", "W", "-p", "2"]));
    env.json(&live, &["dep", &waiter, "--on", &dep]);
    env.json(&old, &["done", &dep, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    let ready = env.json(&live, &["ready"]);
    assert!(ready["tasks"].as_array().unwrap().iter().any(|t| t["id"] == waiter.as_str()));
    let report = env.check(&live);
    assert!(report.as_object().is_none_or(|r| r.get("warnings").and_then(|w| w.as_array())
        .is_none_or(|w| w.iter().all(|f| f["kind"] != "unreachable_dep"))));
}

#[test]
fn retired_read_single_project_reads_and_show_warn() {
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
fn retired_read_a_gone_or_retired_successor_does_not_break_reads() {
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
fn retired_read_projects_reports_retirement_and_keeps_counts() {
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

Run: `just test-one retired_read_`
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
