# Claim Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `tasks release <id> "<why>" [--adopting <old>]`, which removes another session's claim that reads `Unverifiable` or `Stale` and records why, without touching the task's lifecycle. Make every refusal such a claim causes name it.

**Architecture:** A new `src/commands/release.rs` reads the claim, refuses an own or live claim, writes one stamped note through the ordinary record path, then removes the claim from its store. The stale-copy check gains a `Writer::Release(session)` variant, so `occupants` skips that session's non-live claims. `--adopting <old>` takes both prefixes' mutation locks in sorted order. It is gated by a new `rename::adopt::accepts` built on adoption's own `classify_registry`, and it looks the claim up in `<old>`'s store under `<old>-<hex>`. One pair of hint helpers in `release.rs` feeds the refusals in rename, adopt, retire and move, and `ready_tasks`' omission warning.

**Tech Stack:** Rust (clap, serde, toml), `assert_cmd` integration tests in `tests/cli.rs` and `tests/move.rs`.

**Spec:** `docs/specs/2026-10-15-claim-release-design.md` (accepted at revision 3, spec round 3; plan-stage corrections in revision 4: `stale` without a reason, `prime` not `lanes`, "rerun the same command"). Read it with this plan. Section numbers below refer to it.

## Global Constraints

- The verb is `tasks release <id> "<why>"`; the reason is positional and required. The flag is `--adopting <old>` (spec §1).
- Released only when the claim is foreign and reads `Unverifiable` or `Stale`. No claim and own claim are `validation`; a live foreign claim is `claimed` (spec §2).
- Plain refusal texts: "`<id>` has no claim"; "the claim on `<id>` is yours; `tasks park <id> "<next step>"` or `tasks done <id>` releases it"; live: `describe_claim` + "; only its session releases a live claim; `tasks start --force <id>` takes it over" (spec §2).
- Under `--adopting`, the own and live refusals never name `park`, `done` or `start --force`. Own: "the claim on `<old>-<hex>` is this session's, recorded before the rename; it clears when this session ends, and `tasks rename <old> <new> --adopt` then proceeds". Live: `describe_claim` + "; the claim on `<old>-<hex>` was recorded before the rename; it clears when session `<s>` ends, and `tasks rename <old> <new> --adopt` then proceeds" (spec §2).
- An unresolved identity counts as foreign (spec §2).
- Note text: `released <session label> (owner <o>, unverifiable: <reason>); reason: <why>`, or `(owner <o>, stale)` for a stale claim. The session label is `takeover_summary`'s: `Unix session` for a `sid:<n>` claim, else `session <s>`. Stamped with lifecycle provenance. Not a lifecycle marker (spec §3).
- Status, owner, park and escalation are untouched; the claim and its holds go (spec §3).
- Record first, then store, all under the held mutation lock(s). A store failure after the note is the store's error kind with the suffix `; the note on <id> landed but its claim was not removed; rerun the same \`tasks release\` command` (spec §4).
- `occupants` skips every non-`Live` claim of the released session, for `release` only. Parks still count (spec §5).
- Hint text: `; if session <s> is gone, run \`tasks release <id> "<why>"\``. Adoption form: `; if session <s> is gone, run \`tasks release <new>-<hex> "<why>" --adopting <old>\``. Hints appear only for `Unverifiable` (spec §6).
- `--adopting <old>` is accepted only when `classify_registry(registry, project, old, project.prefix)` returns `Fresh`. Locks are taken in sorted prefix order (spec §1).
- Output is `IdOut`; no JSON shape changes; the claim store format is unchanged (spec §7).
- Version 0.4.0 (spec, Documentation).
- Tests run through `just test-one <args>` (one filter per run) and `just test-fast`; never `cargo test` directly (AGENTS.md).
- Run `cargo fmt` before every commit: the pre-commit hook runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- Conventional commits, no AI-attribution trailers.

## Review Focus

1. **A reason containing quotes or shell metacharacters** (`tasks release <id> 'it said "done"; rm'`). The note keeps it verbatim, and the store-failure suffix does not try to reprint it. Test in Task 1 (`release_keeps_a_quoted_reason_verbatim`).
2. **Releasing a claim whose task file is missing from this checkout** (the claim exists, the record does not). It must fail with `task_not_found` before the store is touched, so the claim stays. Test in Task 1 (`release_without_a_record_leaves_the_claim`).
3. **`--adopting` naming this checkout's own prefix.** It must refuse with `validation` before taking any lock, since a second lock on the same prefix in one process would block forever. Test in Task 2 (`release_adopting_refusals_write_nothing`, own-prefix case).
4. **A stale claim's reason naming a pid** (`pid 4242 is gone`). The note must not contain the pid. Test in Task 1 (`release_records_a_stale_claim_without_its_pid`).
5. **A bare six-hex suffix** (`tasks release 4f2a9c "<why>"`) inside the project. It resolves like any other id and releases. Test in Task 1 (`release_accepts_a_bare_suffix`).

## Tracking

Task tasks-f9fc01 owns this plan. One child per `### Task N:` heading, each `--process direct --complexity mid`: the plan settles the approach, and each step still needs care at its call sites. Run `tasks start <child>` before a step and `tasks done <child>` when its commit lands. tasks-f9fc01 closes after Task 3 and the final whole-branch review.

## Setup

- Work in `.worktrees/claim-release` on branch `feat/claim-release`. It exists and holds the claim.
- The justfile defines no `setup` recipe and AGENTS.md names no setup command, so none runs.
- Before Task 1, run the baseline: `just test-fast` must pass on the branch as it stands (docs-only changes so far). Record the result in a task note.

---

### Task 1: `tasks release` for a claim in this project's store

**Files:**
- Create: `src/commands/release.rs`
- Modify: `src/cli.rs` (the `Command` enum after `Drop`, and `commit_verb`)
- Modify: `src/commands/mod.rs`: `pub mod release;`; dispatch after `Command::Drop`; `Ctx::session_label` factored out of `takeover_summary`; `Writer` gains `Release(&str)`; `refuse_stale_copy` and `occupants` take it
- Modify: `src/commands/edit.rs:303,346`, `src/commands/feedback.rs:276`, `src/moving/preflight.rs:17-22` (only if the compiler requires a lifetime on `Writer`; the variants they pass are unchanged)
- Modify: `skills/tasks/SKILL.md:97`, `README.md` (PID-view paragraph near line 315; command list near line 399)
- Test: `tests/cli.rs` (append after `pid_view_heartbeat_after_the_recorded_pid_died_keeps_the_claim_held`)

**Interfaces:**
- Consumes: `crate::claims::{Claim, ClaimStore, Liveness, liveness, resolve_identity}`; `commands::{ownership, Ownership, parse_id, owner_name, append_stamped_note, save, id_out, guard_write, refuse_stale_copy, ClaimIntent::PreserveStore}`; `Ctx::describe_claim`.
- Produces:
  - `pub fn release::run(ctx: Ctx, id: String, why: String) -> Result<Output>` (Task 2 changes its signature).
  - `pub(crate) fn Ctx::session_label(claim: &Claim) -> String`.
  - `pub enum Writer<'a> { Command, Feedback, Release(&'a str) }`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
/// The one note `release` wrote on `id`.
fn release_note(env: &TestEnv, dir: &std::path::Path, id: &str) -> serde_json::Value {
    let shown = env.json(dir, &["show", id]);
    let notes: Vec<serde_json::Value> = shown["task"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|note| note["text"].as_str().unwrap().starts_with("released "))
        .cloned()
        .collect();
    assert_eq!(notes.len(), 1, "{shown}");
    notes[0].clone()
}

#[test]
fn release_removes_an_unverifiable_claim_and_records_why() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    json_as(&env, &sci, "sandboxed", &["start", &id]);
    set_pid_view(&env, "sci", &id, "0:1");
    let owner = stored_claim(&env, "sci", &id)["owner"].as_str().unwrap().to_string();
    let before = env.json(&sci, &["show", &id])["task"].clone();

    let out = as_agent(&env, &sci, "releaser")
        .env("CLAUDE_CODE_SESSION_ID", "11111111-2222-3333-4444-555555555555")
        .args(["release", &id, "sandbox exited"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["id"], id);

    assert!(env.json(&sci, &["show", &id])["claim"].is_null());
    let note = release_note(&env, &sci, &id);
    assert_eq!(
        note["text"],
        format!(
            "released session sandboxed (owner {owner}, unverifiable: recorded in another \
             PID view); reason: sandbox exited"
        )
    );
    assert_eq!(
        note["harness_session"],
        "claude-code:11111111-2222-3333-4444-555555555555"
    );
    let after = env.json(&sci, &["show", &id])["task"].clone();
    for field in ["status", "owner"] {
        assert_eq!(after[field], before[field], "{field}");
    }

    // The next start needs no --force and records no takeover.
    json_as(&env, &sci, "next-agent", &["start", &id]);
    let shown = env.json(&sci, &["show", &id]).to_string();
    assert!(!shown.contains("took over"), "{shown}");
}

#[test]
fn release_frees_the_claims_exclusive_hold() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    hold_vocab(&sci);
    let holding = id_of(env.json(&sci, &["add", "Capture", "-p", "2", "--need", "quiet"]));
    let waiting = id_of(env.json(&sci, &["add", "Rerun", "-p", "2", "--need", "quiet"]));
    json_as(&env, &sci, "sandboxed", &["start", &holding]);
    set_pid_view(&env, "sci", &holding, "0:1");
    let error = error_as(&env, &sci, "agent-b", &["start", &waiting]);
    assert_eq!(error["error"]["kind"], "need_held", "{error}");

    json_as(&env, &sci, "releaser", &["release", &holding, "sandbox exited"]);
    json_as(&env, &sci, "agent-b", &["start", &waiting]);
}

#[test]
fn release_records_a_stale_claim_without_its_pid() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id().to_string();
    env.cmd(&sci)
        .env("TASKS_SESSION", "gone")
        .env("TASKS_SESSION_PID", &pid)
        .args(["start", &id])
        .assert()
        .success();
    child.kill().unwrap();
    child.wait().unwrap();
    let owner = stored_claim(&env, "sci", &id)["owner"].as_str().unwrap().to_string();

    json_as(&env, &sci, "releaser", &["release", &id, "it died"]);
    let note = release_note(&env, &sci, &id);
    let text = note["text"].as_str().unwrap();
    assert_eq!(text, format!("released session gone (owner {owner}, stale); reason: it died"));
    assert!(!text.contains(&pid), "{text}");
}

#[test]
fn release_refusals_write_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let free = id_of(env.json(&sci, &["add", "Free", "-p", "2"]));
    let mine = id_of(env.json(&sci, &["add", "Mine", "-p", "2"]));
    let live = id_of(env.json(&sci, &["add", "Live", "-p", "2"]));
    json_as(&env, &sci, "me", &["start", &mine]);
    json_as(&env, &sci, "other", &["start", &live]);
    let records: Vec<(String, String)> = [&free, &mine, &live]
        .iter()
        .map(|id| (id.to_string(), env.read(&sci, &format!("tasks/{id}.md"))))
        .collect();
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();

    let error = error_as(&env, &sci, "me", &["release", &free, "x"]);
    assert_eq!(error["error"]["kind"], "validation", "{error}");
    assert!(error["error"]["detail"].as_str().unwrap().contains(&format!("{free} has no claim")));

    let error = error_as(&env, &sci, "me", &["release", &mine, "x"]);
    assert_eq!(error["error"]["kind"], "validation", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("is yours"), "{detail}");
    assert!(detail.contains(&format!("tasks park {mine}")), "{detail}");

    let error = error_as(&env, &sci, "me", &["release", &live, "x"]);
    assert_eq!(error["error"]["kind"], "claimed", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("only its session releases a live claim"), "{detail}");
    assert!(detail.contains(&format!("tasks start --force {live}")), "{detail}");

    for (id, before) in &records {
        assert_eq!(&env.read(&sci, &format!("tasks/{id}.md")), before, "{id}");
    }
    assert_eq!(std::fs::read_to_string(env.claim_store("sci")).unwrap(), store);
}

#[test]
fn release_keeps_a_quoted_reason_verbatim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    json_as(&env, &sci, "sandboxed", &["start", &id]);
    set_pid_view(&env, "sci", &id, "0:1");
    let why = "it said \"done\"; rm -rf `x` $HOME";
    json_as(&env, &sci, "releaser", &["release", &id, why]);
    let text = release_note(&env, &sci, &id)["text"].as_str().unwrap().to_string();
    assert!(text.ends_with(&format!("; reason: {why}")), "{text}");
}

#[test]
fn release_without_a_record_leaves_the_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    json_as(&env, &sci, "sandboxed", &["start", &id]);
    set_pid_view(&env, "sci", &id, "0:1");
    std::fs::remove_file(sci.join(format!("tasks/{id}.md"))).unwrap();
    let error = error_as(&env, &sci, "releaser", &["release", &id, "x"]);
    assert_eq!(error["error"]["kind"], "task_not_found", "{error}");
    assert!(stored_claim(&env, "sci", &id).get("session").is_some());
}

#[test]
fn release_accepts_a_bare_suffix() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    json_as(&env, &sci, "sandboxed", &["start", &id]);
    set_pid_view(&env, "sci", &id, "0:1");
    let hex = id.strip_prefix("sci-").unwrap();
    json_as(&env, &sci, "releaser", &["release", hex, "gone"]);
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());
}

#[test]
fn release_store_failure_after_the_note_names_the_rerun() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    json_as(&env, &sci, "sandboxed", &["start", &id]);
    set_pid_view(&env, "sci", &id, "0:1");
    let claims = env.claim_store("sci").parent().unwrap().to_path_buf();
    std::fs::set_permissions(&claims, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(claims.join("probe"), "").is_ok() {
        std::fs::remove_file(claims.join("probe")).unwrap();
        std::fs::set_permissions(&claims, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: a read-only directory is writable here (running as root)");
        return;
    }
    let error = error_as(&env, &sci, "releaser", &["release", &id, "gone"]);
    std::fs::set_permissions(&claims, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(error["error"]["kind"], "io", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.ends_with(&format!(
            "; the note on {id} landed but its claim was not removed; rerun the same \
             `tasks release` command"
        )),
        "{detail}"
    );
    assert!(stored_claim(&env, "sci", &id).get("session").is_some());
    release_note(&env, &sci, &id);
}

#[test]
fn release_from_main_skips_the_gone_sessions_claims_in_the_stale_copy_check() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let other = id_of(env.json(&side, &["add", "Other work", "-p", "2"]));
    as_agent(&env, &side, "gone").args(["start", &id]).assert().success();
    as_agent(&env, &side, "gone").args(["start", &other]).assert().success();
    set_pid_view(&env, "sci", &id, "0:1");
    set_pid_view(&env, "sci", &other, "0:1");
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");

    // The gone session's two claims occupy nothing for its release: the remedy is the rerun.
    let release = ["release", id.as_str(), "sandbox exited"];
    let detail = stale_detail(&env, &main, &release);
    assert!(!detail.contains("Wait for that branch to merge"), "{detail}");
    assert!(detail.contains("tasks -C "), "{detail}");

    // A note from main still names the holder, as before.
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert!(detail.contains("Wait for that branch to merge"), "{detail}");

    // Another session's live claim there still occupies it.
    let third = id_of(env.json(&side, &["add", "Third", "-p", "2"]));
    as_agent(&env, &side, "agent-c").args(["start", &third]).assert().success();
    let detail = stale_detail(&env, &main, &release);
    assert!(detail.contains("Wait for that branch to merge"), "{detail}");
    assert!(detail.contains("agent-c"), "{detail}");

    // The printed rerun, made where the newer copy is, succeeds.
    as_agent(&env, &side, "releaser").args(release).assert().success();
    assert!(env.json(&side, &["show", &id])["claim"].is_null());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test cli release_`
Expected: the nine new tests FAIL. Clap rejects `release` as an unrecognized subcommand, so each fails on its first `release` call. The filter also matches two existing tests, `release_follows_the_claim_not_the_local_doing_status` and `done_retries_a_failed_release_after_a_status_edit`, which pass.

- [ ] **Step 3: Add the subcommand to `src/cli.rs`**

After the `Drop { … }` variant:

```rust
    /// Release another session's claim that this command cannot judge live, and record
    /// why. Status, owner and park are untouched; the next `start` needs no --force.
    #[command(
        after_help = "Examples:\n  tasks release sci-4f2a9c \"the sandbox exited\"\n  tasks release sci-4f2a9c \"container removed\""
    )]
    Release {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// Why the holder is known to be gone; kept verbatim in the task's note.
        why: String,
    },
```

In `commit_verb`, after `Command::Drop { .. } => "drop",`:

```rust
            Command::Release { .. } => "release",
```

- [ ] **Step 4: Widen `Writer` and teach `occupants` the released session (`src/commands/mod.rs`)**

Replace the `Writer` enum:

```rust
/// Who is writing, which decides whether a `-C` retry can be offered (spec §3.2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Writer<'a> {
    Command,
    /// A feedback recurrence: `-C` would change the reporting project.
    Feedback,
    /// `release` of this session's claim: its non-live claims occupy no worktree, since the
    /// caller asserts the session is gone (claim-release spec §5).
    Release(&'a str),
}
```

Change the signature of `refuse_stale_copy` to `writer: Writer<'_>`. Inside it, replace the `occupants(ctx, &newer.root)` call with:

```rust
    let gone = match writer {
        Writer::Release(session) => Some(session),
        Writer::Command | Writer::Feedback => None,
    };
    let (work, unknown) = match occupants(ctx, &newer.root, gone) {
```

Change `occupants` to:

```rust
/// Spec §3.2 rule 1: every task that a session other than the caller holds live in `root`,
/// or parked there. Claims are compared by `ownership`, parks by their tagged session.
/// An unresolvable identity makes every park someone else's. `gone` is the session a
/// `release` asserts has ended: its claims that do not read `Live` occupy nothing.
fn occupants(ctx: &mut Ctx, root: &Path, gone: Option<&str>) -> Result<Occupants> {
    let snapshot =
        crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let mine = me.identity().map(|identity| identity.tagged.clone());
    let mut work = Vec::new();
    for (task, (claim, liveness)) in snapshot.iter() {
        let released =
            gone == Some(claim.session.as_str()) && !matches!(liveness, Liveness::Live);
        if !liveness.is_stale()
            && !released
            && Path::new(&claim.worktree) == root
            && ownership(claim, &me)? == Ownership::Foreign
        {
            work.push((task.clone(), claim.session.clone()));
        }
    }
```

(The rest of `occupants` is unchanged.) If `cargo build` reports a missing lifetime at the other `refuse_stale_copy` callers (`edit.rs`, `feedback.rs`, `moving/preflight.rs`), no edit is needed beyond what the compiler names. They pass `Writer::Command` or `Writer::Feedback` as before.

- [ ] **Step 5: Factor the session label out of `takeover_summary`**

In `impl Ctx` in `src/commands/mod.rs`, replace the `let session = match … ;` block of `takeover_summary` with a call, and add the helper above it:

```rust
    /// How a task record names a claim's session: a Unix session's pid never reaches git.
    pub(crate) fn session_label(claim: &crate::claims::Claim) -> String {
        match claim
            .session
            .strip_prefix("sid:")
            .and_then(|pid| pid.parse::<u32>().ok())
        {
            Some(_) => "Unix session".to_string(),
            None => format!("session {}", claim.session),
        }
    }
```

and in `takeover_summary`:

```rust
        format!(
            "took over {} (owner {}, {how})",
            Ctx::session_label(claim),
            claim.owner
        )
```

- [ ] **Step 6: Write `src/commands/release.rs`**

```rust
//! `tasks release`: remove another session's claim that this command cannot judge live,
//! and record why, without a lifecycle (claim-release spec).

use super::{Ctx, ClaimIntent, Ownership, Writer};
use crate::claims::{Claim, Liveness};
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;

pub fn run(mut ctx: Ctx, id: String, why: String) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claim = ctx
        .claims_mut()?
        .get(&id)
        .cloned()
        .ok_or_else(|| Error::Validation(format!("{id} has no claim")))?;
    let live = judge(&mut ctx, &claim, &id)?;
    let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
    let store = ctx.claims_mut()?;
    store.remove(&id);
    store.save().map_err(|error| error.with_suffix(&landed(&task.id)))?;
    Ok(super::id_out(ctx, &task))
}

/// Spec §2: refuse an own or live claim; return the verdict that permits the release.
fn judge(ctx: &mut Ctx, claim: &Claim, id: &TaskId) -> Result<Liveness> {
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    if super::ownership(claim, &me)? != Ownership::Foreign {
        return Err(Error::Validation(format!(
            "the claim on {id} is yours; `tasks park {id} \"<next step>\"` or `tasks done {id}` \
             releases it"
        )));
    }
    let live = crate::claims::liveness(claim);
    if matches!(live, Liveness::Live) {
        return Err(Error::Claimed(
            id.to_string(),
            format!(
                "{}; only its session releases a live claim; `tasks start --force {id}` takes \
                 it over",
                Ctx::describe_claim(claim, &live)
            ),
        ));
    }
    Ok(live)
}

/// Spec §3–§5: load the record past the stale-copy check that skips this session's
/// claims, append the stamped note, and write the record without touching the store.
fn write_note(
    ctx: &mut Ctx,
    id: &TaskId,
    claim: &Claim,
    live: &Liveness,
    why: &str,
) -> Result<Task> {
    let (mut task, raw) = ctx.project.read_task_with_raw(id)?;
    super::refuse_stale_copy(ctx, &task, &raw, Writer::Release(&claim.session))?;
    super::guard_write(&ctx.registry, &ctx.shorthand, &task)?;
    let owner = super::owner_name(&ctx.project)?;
    super::append_stamped_note(ctx, &mut task, &owner, &summary(claim, live, why))?;
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::PreserveStore));
    super::save(ctx, &mut task)?;
    Ok(task)
}

/// The note: `takeover_summary`'s redaction, the verdict, and the person's reason.
fn summary(claim: &Claim, live: &Liveness, why: &str) -> String {
    let verdict = match live {
        Liveness::Unverifiable(reason) => format!("unverifiable: {reason}"),
        // A stale reason can name a host pid; a record keeps none.
        Liveness::Stale(_) => "stale".to_string(),
        Liveness::Live => unreachable!("judge refuses a live claim"),
    };
    format!(
        "released {} (owner {}, {verdict}); reason: {why}",
        Ctx::session_label(claim),
        claim.owner
    )
}

fn landed(id: &TaskId) -> String {
    format!(
        "; the note on {id} landed but its claim was not removed; rerun the same \
         `tasks release` command"
    )
}
```

In `src/commands/mod.rs`, add `pub mod release;` between `pub mod quiet;` and `pub mod rename;`. Add the dispatch after the `Command::Drop` arm:

```rust
        Command::Release { id, why } => release::run(open_id_write_ctx(dir, &id)?, id, why),
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `just test-one --test cli release_`
Expected: PASS, the nine new tests and the two existing ones. Then run `just test-one --test cli an_unverifiable_claim_still_occupies_its_worktree` (unchanged behaviour for `note`). Expected: PASS.

- [ ] **Step 8: Document the verb**

`skills/tasks/SKILL.md` line 97, replace:

```
   Its own view, its owner, a reboot or `start --force` releases it.
```

with:

```
   Its own view, its owner, a reboot or `start --force` releases it. When you know its
   holder is gone (a finished sandbox, a removed container), `tasks release <id> "<why>"`
   removes it with one note and leaves status, owner and park alone; a live claim is
   refused.
```

`README.md`, in the PID-view paragraph, replace `A reboot, its owner's \`park\` or \`done\`, or \`start --force\` clears it.` with:

```
A reboot, its owner's `park` or `done`, or `start --force` clears it, and so does
`tasks release <id> "<why>"` once you know the holder is gone: it records why in one note
and leaves the task's lifecycle alone.
```

In the README command list, after the `tasks claims` line, add:

```
    tasks release <id> "sandbox exited"  # drop a claim this command cannot judge, holder known gone; records why
```

- [ ] **Step 9: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src/cli.rs src/commands/mod.rs src/commands/release.rs tests/cli.rs skills/tasks/SKILL.md README.md
git commit -m "feat(release): release a foreign claim whose holder is gone, with a note"
```

(Add `src/commands/edit.rs src/commands/feedback.rs src/moving/preflight.rs` only if Step 4 changed them.)

---

### Task 2: `--adopting <old>` and adoption's hint

**Files:**
- Modify: `src/cli.rs` (the `Release` variant)
- Modify: `src/commands/release.rs` (full replacement below)
- Modify: `src/commands/mod.rs` (the `Command::Release` dispatch)
- Modify: `src/rename/adopt.rs` (a new `pub(crate) fn accepts`; the claim loop in `run`, lines 147-157)
- Modify: `skills/tasks/SKILL.md` (the sentence added in Task 1), `README.md` (the command-list line added in Task 1)
- Test: `tests/cli.rs` (append after `adopt_carries_an_orphaned_park_and_reports_it`)

**Interfaces:**
- Consumes: Task 1's `release::run` internals (`judge`, `write_note`, `summary`, `landed`), `Writer::Release`, `Ctx::session_label`; `commands::{open_id_ctx, open_id_write_ctx, lock_and_revalidate}`; `crate::claims::{ClaimStore, MutationLock}`.
- Produces:
  - `pub fn release::open(dir: Option<&Path>, id: &str, adopting: Option<&str>) -> Result<(Ctx, Option<release::Adopting>)>`
  - `pub fn release::run(ctx: Ctx, adopting: Option<release::Adopting>, id: String, why: String) -> Result<Output>`
  - `pub fn release::hint(id: &str, session: &str) -> String`
  - `pub fn release::adopting_hint(renamed: &str, session: &str, old: &str) -> String`
  - `pub(crate) fn rename::adopt::accepts(registry: &Registry, project: &Project, old: &str) -> Result<Option<String>>`: `None` when adoption would accept `old` for this checkout, else the reason.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`, after `adopt_carries_an_orphaned_park_and_reports_it`:

```rust
/// A claim on `old_id` in `old`'s store under the adopt fixture's state directory. With
/// `view`, it carries this test process's live pid recorded in that view; without, it is
/// a pid-less claim seen in 2099, which reads live by its TTL.
fn adopt_old_claim(env: &TestEnv, old_id: &str, session: &str, view: Option<&str>) {
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    let process = match view {
        Some(view) => {
            let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
            let rest = stat.rsplit_once(") ").unwrap().1.to_string();
            let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
            let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
            format!(
                "pid = {}\npid_start = {start}\nboot_id = \"{}\"\npid_view = \"{view}\"\n",
                std::process::id(),
                boot.trim()
            )
        }
        None => String::new(),
    };
    std::fs::write(
        state.join("old.toml"),
        format!(
            "[claims.\"{old_id}\"]\nowner = \"tester\"\nsession = \"{session}\"\nhost = \"host\"\n\
             worktree = \"/old\"\nstarted = \"2099-01-01T00:00:00Z\"\n\
             seen = \"2099-01-01T00:00:00Z\"\n{process}"
        ),
    )
    .unwrap();
}

#[test]
fn release_adopting_follows_the_adoption_hint_end_to_end() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));

    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    let detail = err_detail(&out);
    let hint = format!("tasks release {new_id} \"<why>\" --adopting old");
    assert!(detail.contains(&hint), "{detail}");

    // Run what the hint prints, with a reason in place of the placeholder.
    let out = adopt_cmd(&env, &dir)
        .env("TASKS_SESSION", "releaser")
        .args(["release", &new_id, "sandbox exited", "--adopting", "old"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let shown = adopt_json(&env, &dir, &["show", &new_id]);
    assert!(
        shown.to_string().contains(
            "released session sandboxed (owner tester, unverifiable: recorded in another PID \
             view); reason: sandbox exited"
        ),
        "{shown}"
    );

    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
}

#[test]
fn release_adopting_refuses_own_and_live_claims_without_ordinary_remedies() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    let record = std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap();
    let old_store = env.home.path().join("state/tasks/claims/old.toml");

    for (session, kind, expected) in [
        ("me", "validation", "is this session's, recorded before the rename"),
        ("someone-else", "claimed", "was recorded before the rename; it clears when session someone-else ends"),
    ] {
        adopt_old_claim(&env, &old_id, session, None);
        let before = std::fs::read(&old_store).unwrap();
        let out = adopt_cmd(&env, &dir)
            .env("TASKS_SESSION", "me")
            .args(["release", &new_id, "x", "--adopting", "old"])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), kind, "{session}");
        let detail = err_detail(&out);
        assert!(detail.contains(expected), "{detail}");
        assert!(detail.contains("tasks rename old new --adopt"), "{detail}");
        for remedy in ["tasks park", "tasks done", "start --force"] {
            assert!(!detail.contains(remedy), "{remedy} in {detail}");
        }
        assert_eq!(std::fs::read(&old_store).unwrap(), before);
        assert_eq!(std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap(), record);
    }
}

#[test]
fn release_adopting_refusals_write_nothing() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));
    let record = std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap();

    for old in ["new", "nope"] {
        let out = adopt_cmd(&env, &dir)
            .env("TASKS_SESSION", "releaser")
            .args(["release", &new_id, "x", "--adopting", old])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "validation", "--adopting {old}");
        assert!(err_detail(&out).contains(&format!("--adopting {old}")), "{out:?}");
    }
    assert_eq!(std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap(), record);
    assert!(!env.home.path().join("state/tasks/claims/nope.lock").exists());

    // After adoption completes, the old prefix is no longer adoptable here.
    std::fs::remove_file(env.home.path().join("state/tasks/claims/old.toml")).unwrap();
    adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    let out = adopt_cmd(&env, &dir)
        .env("TASKS_SESSION", "releaser")
        .args(["release", &new_id, "x", "--adopting", "old"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
}

#[test]
fn adopt_hints_plain_release_for_an_unverifiable_claim_in_the_new_store() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));
    let state = env.home.path().join("state/tasks/claims");
    std::fs::rename(state.join("old.toml"), state.join("new.toml")).unwrap();
    let text = std::fs::read_to_string(state.join("new.toml")).unwrap();
    std::fs::write(state.join("new.toml"), text.replace(&old_id, &new_id)).unwrap();
    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    let detail = err_detail(&out);
    assert!(detail.contains(&format!("tasks release {new_id} \"<why>\"`")), "{detail}");
    assert!(!detail.contains("--adopting"), "{detail}");
}
```

The fixture's `adopt_cmd` sets `XDG_STATE_HOME` to `<home>/state`, which is why these tests write the store there rather than through `env.claim_store`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test cli release_adopting_`
Expected: FAIL. Clap rejects `--adopting`, and the end-to-end test fails first on the missing hint in adoption's refusal.

- [ ] **Step 3: Add `accepts` and the hints in `src/rename/adopt.rs`**

After `classify_registry`, add:

```rust
/// Whether adoption would take `old` as this checkout's predecessor: `None` when it would
/// (the stage in which it checks the old store's claims), else why not. `release
/// --adopting` asks this, so it never reaches a store adoption would not
/// (claim-release spec §1).
pub(crate) fn accepts(registry: &Registry, project: &Project, old: &str) -> Result<Option<String>> {
    Ok(match classify_registry(registry, project, old, &project.prefix)? {
        Stage::Fresh(_) => None,
        Stage::Refuse(reason) => Some(reason),
        Stage::ResumeRegistry | Stage::ResumeCleanup | Stage::Complete => {
            Some(format!("this host has already adopted {old:?}"))
        }
    })
}
```

Replace the claim loop in `run`:

```rust
    if matches!(stage, Stage::Fresh(_) | Stage::ResumeRegistry) {
        // An old-store claim's record is now `<new>-<hex>` here, so its release needs
        // `--adopting`; a new-store claim's record routes as usual.
        for (id, claim) in source.iter() {
            let live = crate::claims::liveness(claim);
            if !live.is_stale() {
                let hint = match live {
                    Liveness::Unverifiable(_) => {
                        let renamed = format!("{new}-{}", TaskId::parse(id)?.hex);
                        crate::commands::release::adopting_hint(&renamed, &claim.session, old)
                    }
                    _ => String::new(),
                };
                return Err(Error::Claimed(
                    id.clone(),
                    Ctx::describe_claim(claim, &live) + &hint,
                ));
            }
        }
        for (id, claim) in target.iter() {
            let live = crate::claims::liveness(claim);
            if !live.is_stale() {
                let hint = match live {
                    Liveness::Unverifiable(_) => {
                        crate::commands::release::hint(id, &claim.session)
                    }
                    _ => String::new(),
                };
                return Err(Error::Claimed(
                    id.clone(),
                    Ctx::describe_claim(claim, &live) + &hint,
                ));
            }
        }
    }
```

Change the import line to `use crate::claims::{ClaimStore, Liveness};`.

- [ ] **Step 4: Replace `src/commands/release.rs`**

```rust
//! `tasks release`: remove another session's claim that this command cannot judge live,
//! and record why, without a lifecycle (claim-release spec).

use super::{ClaimIntent, Ctx, Ownership, Writer};
use crate::claims::{Claim, ClaimStore, Liveness, MutationLock};
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::Output;
use std::path::Path;

/// The refusal hint for a held claim that reads `Unverifiable` (spec §6).
pub fn hint(id: &str, session: &str) -> String {
    format!("; if session {session} is gone, run `tasks release {id} \"<why>\"`")
}

/// The same, for a claim adoption finds in the old prefix's store (spec §6).
pub fn adopting_hint(renamed: &str, session: &str, old: &str) -> String {
    format!(
        "; if session {session} is gone, run `tasks release {renamed} \"<why>\" --adopting {old}`"
    )
}

/// `--adopting <old>`: the old prefix, its store, and its lock, held to the command's end.
pub struct Adopting {
    old: String,
    store: ClaimStore,
    _lock: MutationLock,
}

/// The write context. Under `--adopting`, both prefixes' locks in sorted order, as
/// `rename --adopt` takes them, and only for a predecessor adoption would accept (§1).
pub fn open(dir: Option<&Path>, id: &str, adopting: Option<&str>) -> Result<(Ctx, Option<Adopting>)> {
    let Some(old) = adopting else {
        return Ok((super::open_id_write_ctx(dir, id)?, None));
    };
    let mut ctx = super::open_id_ctx(dir, id)?;
    let prefix = ctx.project.prefix.clone();
    if old == prefix {
        // Checked before any lock: a second lock on one prefix would wait on itself.
        return Err(Error::Validation(format!(
            "--adopting {old} names this checkout's own prefix; it takes the prefix being adopted"
        )));
    }
    let early = if old < prefix.as_str() {
        Some(MutationLock::acquire(old)?)
    } else {
        None
    };
    let routing = ctx.routing.clone();
    super::lock_and_revalidate(&mut ctx, &routing)?;
    if ctx.project.prefix != prefix {
        return Err(Error::Validation(format!(
            "--adopting {old}: this checkout's prefix changed from {prefix:?} while locking; retry"
        )));
    }
    if let Some(reason) = crate::rename::adopt::accepts(&ctx.registry, &ctx.project, old)? {
        return Err(Error::Validation(format!(
            "--adopting {old}: {reason}; it is accepted only while `tasks rename {old} {prefix} \
             --adopt` would adopt it"
        )));
    }
    let lock = match early {
        Some(lock) => lock,
        None => MutationLock::acquire(old)?,
    };
    let store = ClaimStore::load(old)?;
    Ok((
        ctx,
        Some(Adopting {
            old: old.to_string(),
            store,
            _lock: lock,
        }),
    ))
}

pub fn run(mut ctx: Ctx, adopting: Option<Adopting>, id: String, why: String) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    match adopting {
        None => {
            let claim = ctx
                .claims_mut()?
                .get(&id)
                .cloned()
                .ok_or_else(|| Error::Validation(format!("{id} has no claim")))?;
            let live = judge(&mut ctx, &claim, &id, None)?;
            let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
            let store = ctx.claims_mut()?;
            store.remove(&id);
            store.save().map_err(|error| error.with_suffix(&landed(&task.id)))?;
            Ok(super::id_out(ctx, &task))
        }
        Some(mut adopting) => {
            let key = TaskId::parse(&format!("{}-{}", adopting.old, id.hex))?;
            let claim = adopting
                .store
                .get(&key)
                .cloned()
                .ok_or_else(|| Error::Validation(format!("{key} has no claim")))?;
            let new = ctx.project.prefix.clone();
            let live = judge(&mut ctx, &claim, &key, Some((&adopting.old, &new)))?;
            let task = write_note(&mut ctx, &id, &claim, &live, &why)?;
            adopting.store.remove(&key);
            adopting
                .store
                .save()
                .map_err(|error| error.with_suffix(&landed(&task.id)))?;
            Ok(super::id_out(ctx, &task))
        }
    }
}

/// Spec §2: refuse an own or live claim; return the verdict that permits the release.
/// `rename` is `(old, new)` under `--adopting`, whose refusals name adoption, never the
/// routed id's `park`, `done` or `start --force`, which act on the other store.
fn judge(
    ctx: &mut Ctx,
    claim: &Claim,
    key: &TaskId,
    rename: Option<(&str, &str)>,
) -> Result<Liveness> {
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let live = crate::claims::liveness(claim);
    let own = super::ownership(claim, &me)? != Ownership::Foreign;
    match (own, matches!(live, Liveness::Live), rename) {
        (true, _, None) => Err(Error::Validation(format!(
            "the claim on {key} is yours; `tasks park {key} \"<next step>\"` or `tasks done {key}` \
             releases it"
        ))),
        (true, _, Some((old, new))) => Err(Error::Validation(format!(
            "the claim on {key} is this session's, recorded before the rename; it clears when \
             this session ends, and `tasks rename {old} {new} --adopt` then proceeds"
        ))),
        (false, true, None) => Err(Error::Claimed(
            key.to_string(),
            format!(
                "{}; only its session releases a live claim; `tasks start --force {key}` takes \
                 it over",
                Ctx::describe_claim(claim, &live)
            ),
        )),
        (false, true, Some((old, new))) => Err(Error::Claimed(
            key.to_string(),
            format!(
                "{}; the claim on {key} was recorded before the rename; it clears when session \
                 {} ends, and `tasks rename {old} {new} --adopt` then proceeds",
                Ctx::describe_claim(claim, &live),
                claim.session
            ),
        )),
        (false, false, _) => Ok(live),
    }
}

/// Spec §3–§5: load the record past the stale-copy check that skips this session's
/// claims, append the stamped note, and write the record without touching the store.
fn write_note(
    ctx: &mut Ctx,
    id: &TaskId,
    claim: &Claim,
    live: &Liveness,
    why: &str,
) -> Result<Task> {
    let (mut task, raw) = ctx.project.read_task_with_raw(id)?;
    super::refuse_stale_copy(ctx, &task, &raw, Writer::Release(&claim.session))?;
    super::guard_write(&ctx.registry, &ctx.shorthand, &task)?;
    let owner = super::owner_name(&ctx.project)?;
    super::append_stamped_note(ctx, &mut task, &owner, &summary(claim, live, why))?;
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::PreserveStore));
    super::save(ctx, &mut task)?;
    Ok(task)
}

/// The note: `takeover_summary`'s redaction, the verdict, and the person's reason.
fn summary(claim: &Claim, live: &Liveness, why: &str) -> String {
    let verdict = match live {
        Liveness::Unverifiable(reason) => format!("unverifiable: {reason}"),
        // A stale reason can name a host pid; a record keeps none.
        Liveness::Stale(_) => "stale".to_string(),
        Liveness::Live => unreachable!("judge refuses a live claim"),
    };
    format!(
        "released {} (owner {}, {verdict}); reason: {why}",
        Ctx::session_label(claim),
        claim.owner
    )
}

fn landed(id: &TaskId) -> String {
    format!(
        "; the note on {id} landed but its claim was not removed; rerun the same \
         `tasks release` command"
    )
}
```

`open_id_ctx` and `lock_and_revalidate` are already `pub` in `src/commands/mod.rs`. `MutationLock` must be importable from `crate::claims`; it is `pub struct MutationLock`.

- [ ] **Step 5: Wire the flag (`src/cli.rs`, `src/commands/mod.rs`)**

In the `Release` variant, add after `why`:

```rust
        /// The prefix this checkout is adopting (`tasks rename <old> <new> --adopt`): release
        /// the claim this host recorded under it before the rename.
        #[arg(long, value_name = "OLD")]
        adopting: Option<String>,
```

and append `\n  tasks release new-4f2a9c \"sandbox exited\" --adopting old` to its `after_help` examples.

Replace the dispatch arm:

```rust
        Command::Release { id, why, adopting } => {
            let (ctx, adopting) = release::open(dir, &id, adopting.as_deref())?;
            release::run(ctx, adopting, id, why)
        }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `just test-one --test cli release_`
Expected: PASS: Task 1's nine, Task 2's three `release_adopting_*`, and the two existing tests the filter matches.
Run: `just test-one --test cli adopt_`
Expected: PASS, including `adopt_hints_plain_release_for_an_unverifiable_claim_in_the_new_store` and every existing adoption test.

- [ ] **Step 7: Document the adoption form**

Append to the sentence added to `skills/tasks/SKILL.md` in Task 1:

```
   Before this host adopts a prefix another host renamed, a claim recorded under the old
   prefix is released with `tasks release <new id> "<why>" --adopting <old>`, which
   `tasks rename <old> <new> --adopt` names when it refuses.
```

In `README.md`'s command list, after the `tasks release` line, add:

```
    tasks release new-4f2a9c "gone" --adopting old  # the same for a claim recorded before a rename this host has not adopted
```

- [ ] **Step 8: Run the fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src/cli.rs src/commands/mod.rs src/commands/release.rs src/rename/adopt.rs tests/cli.rs skills/tasks/SKILL.md README.md
git commit -m "feat(release): reach an unadopted old claim with --adopting; adopt names it"
```

---

### Task 3: Name the release in every refusal; version 0.4.0

**Files:**
- Modify: `src/rename/mod.rs` (`authorize`, lines 316-330)
- Modify: `src/commands/retire.rs` (`reject_live_claims`, lines 72-84)
- Modify: `src/moving/preflight.rs` (the claim check, lines 37-46)
- Modify: `src/commands/list.rs` (`ready_tasks`' claim omission, lines 280-290)
- Modify: `Cargo.toml` (version), `Cargo.lock` (regenerated by the build)
- Test: `tests/cli.rs` (append after Task 2's tests), `tests/move.rs` (after `the_callers_own_claim_refuses_naming_park`)

**Interfaces:**
- Consumes: `release::hint(id: &str, session: &str) -> String` (Task 2); `commands::{ownership, Ownership}`; `crate::claims::{Liveness, resolve_identity, ClaimSnapshot::get}`.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn release_is_named_by_ready_next_retire_and_rename_for_an_unverifiable_claim_only() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let held = id_of(env.json(&sci, &["add", "Held", "-p", "1"]));
    // A todo record with a claim, as a main checkout sees one while a worktree holds it.
    write_claim_in_view(&env, "sci", &held, "sandboxed", "0:1");
    let hint = format!("tasks release {held} \"<why>\"");

    for command in [&["ready"][..], &["next"][..]] {
        let out = env.json(&sci, command);
        let warnings = out["warnings"].to_string();
        assert!(warnings.contains(&hint), "{command:?}: {warnings}");
    }
    for command in [&["retire", "--reason", "over"][..], &["rename", "sci", "lab"][..]] {
        let error = error_of(&env, &sci, command);
        assert_eq!(error["error"]["kind"], "claimed", "{command:?}: {error}");
        assert!(
            error["error"]["detail"].as_str().unwrap().contains(&hint),
            "{command:?}: {error}"
        );
    }

    // A live claim keeps the old wording everywhere.
    let live = id_of(env.json(&sci, &["add", "Live", "-p", "1"]));
    write_claim(&env, "sci", &live, "agent-b", true);
    let warnings = env.json(&sci, &["ready"])["warnings"].to_string();
    assert!(!warnings.contains(&format!("tasks release {live}")), "{warnings}");
}
```

`write_claim_in_view` and `write_claim` (tests/cli.rs:947-987) plant a claim on a todo record without starting it. `retire` checks claims before open copies, and `rename`'s `authorize` checks them before anything else, so both reach the claim. `error_of` is the existing helper at `tests/cli.rs:9435`.

Append to `tests/move.rs`, after `the_callers_own_claim_refuses_naming_park`:

```rust
fn unverifiable(env: &TestEnv, prefix: &str, id: &str) {
    let path = env.claim_store(prefix);
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    store["claims"][id]
        .as_table_mut()
        .unwrap()
        .insert("pid_view".into(), toml::Value::String("0:1".into()));
    std::fs::write(&path, toml::to_string(&store).unwrap()).unwrap();
}

#[test]
fn a_foreign_claim_refuses_move_with_its_sessions_remedy() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let live = id_of(env.json(&dots, &["add", "Live", "-p", "2"]));
    let gone = id_of(env.json(&dots, &["add", "Gone", "-p", "2"]));
    for id in [&live, &gone] {
        env.cmd(&dots)
            .env("TASKS_SESSION", "other")
            .env("TASKS_SESSION_PID", std::process::id().to_string())
            .args(["start", id])
            .assert()
            .success();
    }
    unverifiable(&env, "dots", &gone);

    let (kind, detail) = fail_detail(&env, &dots, &["move", &live, "--to", "wali"]);
    assert_eq!(kind, "claimed");
    assert!(detail.contains("that session must park it first"), "{detail}");
    assert!(!detail.contains("tasks park"), "{detail}");

    let (kind, detail) = fail_detail(&env, &dots, &["move", &gone, "--to", "wali"]);
    assert_eq!(kind, "claimed");
    assert!(detail.contains(&format!("tasks release {gone} \"<why>\"")), "{detail}");
    assert!(!detail.contains("tasks park"), "{detail}");
}
```

Check `fail_detail`'s return order in `tests/move.rs:156` (`(kind, detail)`, as `.0` is compared with `"claimed"` at line 1455) before relying on it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test cli release_is_named_by`
Expected: FAIL. `ready`'s warning lacks the hint.
Run: `just test-one --test move a_foreign_claim_refuses_move`
Expected: FAIL. The detail says "park it with `tasks park …` first".

- [ ] **Step 3: `ready_tasks` (`src/commands/list.rs`)**

Replace the claim-omission `retain`:

```rust
    ready.retain(|task| match claims.get(&task.id) {
        Some((claim, live)) if !live.is_stale() => {
            let release = match live {
                Liveness::Unverifiable(_) => {
                    super::release::hint(&task.id.to_string(), &claim.session)
                }
                _ => String::new(),
            };
            warnings.push(format!(
                "{} — {} to take it over{release}",
                claim_omission(&task.id, claim),
                force_hint(&task.id.to_string(), snapshots)
            ));
            false
        }
        _ => true,
    });
```

Add `use crate::claims::Liveness;` to the imports if it is not already there.

- [ ] **Step 4: `retire` (`src/commands/retire.rs`)**

```rust
fn reject_live_claims(project: &Project) -> Result<()> {
    let store = ClaimStore::load(&project.prefix)?;
    for (id, claim) in store.iter() {
        let live = crate::claims::liveness(claim);
        if !live.is_stale() {
            let hint = match live {
                crate::claims::Liveness::Unverifiable(_) => super::release::hint(id, &claim.session),
                _ => String::new(),
            };
            return Err(Error::Claimed(
                id.clone(),
                super::Ctx::describe_claim(claim, &live) + &hint,
            ));
        }
    }
    Ok(())
}
```

- [ ] **Step 5: `rename` (`src/rename/mod.rs`, `authorize`)**

```rust
    for prefix in [&invocation.source, &invocation.target] {
        for (id, claim) in ClaimStore::load(prefix)?.iter() {
            let live = crate::claims::liveness(claim);
            if !live.is_stale() {
                // Only the source's records live in this checkout; a claim in the target's
                // store has no record here to note, so it gets no hint.
                let hint = match live {
                    crate::claims::Liveness::Unverifiable(_) if prefix == &invocation.source => {
                        crate::commands::release::hint(id, &claim.session)
                    }
                    _ => String::new(),
                };
                return Err(Error::Claimed(
                    id.clone(),
                    Ctx::describe_claim(claim, &live) + &hint,
                ));
            }
        }
    }
```

- [ ] **Step 6: `move` (`src/moving/preflight.rs`)**

Replace the `if let Some(claim) = claims.held(id) { … }` block:

```rust
    if let Some((claim, live)) = claims.get(id)
        && !live.is_stale()
    {
        let me = crate::claims::resolve_identity(&mut ctx.source.warnings);
        let remedy = match (commands::ownership(claim, &me)?, live) {
            (commands::Ownership::Foreign, crate::claims::Liveness::Unverifiable(why)) => format!(
                "unverifiable: {why}{}",
                commands::release::hint(&id.to_string(), &claim.session)
            ),
            (commands::Ownership::Foreign, _) => "that session must park it first".to_string(),
            _ => format!("park it with `tasks park {id} \"<next step>\"` first"),
        };
        return Err(Error::Claimed(
            id.to_string(),
            format!("{}; {remedy}", claim.session),
        ));
    }
```

`Ownership` and `ownership` are `pub(crate)` in `src/commands/mod.rs`, so `crate::moving` reaches them.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `just test-one --test cli release_is_named_by`
Expected: PASS.
Run: `just test-one --test move a_foreign_claim_refuses_move`
Expected: PASS.
Run: `just test-one --test move the_callers_own_claim_refuses_naming_park`
Expected: PASS (own-claim wording unchanged).

- [ ] **Step 8: Version 0.4.0**

In `Cargo.toml`, set `version = "0.4.0"`. Run `cargo build --bin tasks` to regenerate `Cargo.lock`, then `./target/debug/tasks --version`.
Expected: `tasks 0.4.0`.

- [ ] **Step 9: Run the gate and commit**

Run: `just test-fast`, then `just check`.
Expected: both PASS.

```bash
git add src/rename/mod.rs src/commands/retire.rs src/moving/preflight.rs src/commands/list.rs tests/cli.rs tests/move.rs Cargo.toml Cargo.lock
git commit -m "feat(release): name tasks release in refusals for unverifiable claims; 0.4.0"
```

After the branch merges, run `cargo install --path .` from the main checkout so the installed tracker is 0.4.0.
