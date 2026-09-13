# Quiet Queue Implementation Plan

Status: implemented (2026-09-13)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A park reason `quiet` with a recorded recipe (`needs`, `minutes`), and a `tasks quiet` command that lists such parks across every project as resume briefs, resolved from each park's recorded checkout first.

**Architecture:** The vocabulary and the two recipe fields live on the existing `Park` entry in the claim store (`src/claims.rs`); `park` validates and writes them (`src/commands/park.rs`); every view that embeds a park block gets them through `ParkInfo` (`src/output.rs`). The parked resolver (`src/commands/parked.rs`) gains a preference parameter so `tasks quiet` (new `src/commands/quiet.rs`) can resolve from the recorded worktree first while `list --parked` and `prime` keep their order.

**Tech Stack:** Rust 2024, clap 4 (derive + `clap_complete` candidates), serde + toml for the store, `assert_cmd` + `tempfile` integration tests in `tests/cli.rs`.

**Spec:** `docs/specs/2026-09-13-quiet-queue-design.md`

## Global Constraints

- JSON output is the contract; additions only. New keys: `park.needs`, `park.minutes` (both `null` when absent) on every park block; a new `Output::Quiet` with `{"tasks": [ParkedRow...], "warnings": [...]}`.
- Fail early with a typed error: a bad flag value is `Error::Validation`; no silent fallback.
- Store files written before this change must load unchanged: new `Park` fields are `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- **This crate has no library target, and every module is private.** An unused `pub fn`, enum variant, or unread field fails `cargo clippy --all-targets -- -D warnings` with `dead_code`; use from `#[cfg(test)]` does not count. Every helper below is introduced in the task whose production code consumes it, which is why this plan has three tasks, not five. Unit tests run as `cargo test --bin tasks <filter>`; end-to-end tests as `cargo test --test cli <filter>`. Never pipe a test or check command through `tail` or `grep`: the pipe hides Cargo's exit status.
- A function with more than seven parameters needs `#[allow(clippy::too_many_arguments)]` directly above it, the repo's existing targeted allowance (`src/commands/list.rs:24`, `src/commands/feedback.rs:148`).
- `just check` (fmt, clippy `-D warnings`, `tasks check`) must pass before every commit; the pre-commit hook runs it. `just test` is `cargo test`.
- Conventional commits, no attribution trailers.
- Never edit `tasks/*.md` by hand; `tasks start` the step task before a task and `tasks done` it in the task's commit.
- Paths in this plan are relative to the worktree `.worktrees/quiet-queue/`.

## Deliverables

| Task | Deliverable | Depends on |
|------|-------------|------------|
| 1 | `Reason::Quiet`, the `Needs` enum, `Park.needs`/`Park.minutes`, `describe_stop` with the recipe, `park --needs/--minutes` with its validation, the note form, the two keys in every park view | — |
| 2 | The parked resolver with a `Prefer` parameter and the `tasks quiet` command that consumes it: CLI, resolution order, ordering, `-n`, JSON and pretty briefs, warnings kept | 1 |
| 3 | Skill, README, doc comments, spec status, reinstall | 2 |

---

### Task 1: The quiet reason and its recipe on park

**Files:**
- Modify: `src/claims.rs` (the `Reason` enum around line 129, `describe_stop` at 178, `Park` at 186, the unit tests from 1107)
- Modify: `src/complete.rs` (after `reason()` at line 53)
- Modify: `src/cli.rs` (the `Park` variant at line 293)
- Modify: `src/commands/mod.rs` (the `Command::Park` dispatch at line 1061)
- Modify: `src/commands/park.rs`
- Modify: `src/output.rs` (`ParkInfo` at line 193 and the two `describe_stop` call sites, around lines 942 and 1101)
- Test: `src/claims.rs` unit tests; `tests/cli.rs` after `park_without_a_reason_records_none_and_re_parking_drops_a_previous_one` (line 994)

**Interfaces:**
- Produces: `claims::Reason::Quiet` (`"quiet"`); `claims::Needs { Idle, Headless }` with `ALL`, `parse(&str) -> Result<Needs>`, `as_str(self) -> &'static str`; `Park.needs: Option<Needs>`, `Park.minutes: Option<u32>`; `claims::describe_stop(who: WaitingOn, reason: Option<Reason>, needs: Option<Needs>, minutes: Option<u32>) -> String`; `complete::needs() -> Vec<CompletionCandidate>`; `ParkInfo.needs`, `ParkInfo.minutes`; `park::run(ctx, id, next_step, waiting_on, reason, complexity, needs: Option<String>, minutes: Option<u32>)`; the note form `parked (waiting on user, quiet; idle, 50 min): <next step>`.

- [x] **Step 1: Write the failing unit tests in `src/claims.rs`**

Replace `reason_parses_its_seven_values_only` with `reason_parses_its_eight_values_only`, delete `describe_stop_names_the_who_and_the_reason_when_given` (the new `describe_stop_appends_the_recipe_when_present` covers both of its assertions), and add the rest. In the existing `sample_park()` helper, and in every other `Park { ... }` literal in the test module (`a_park_without_a_reason_loads_and_a_park_with_one_writes_the_key`, `inserting_a_park_displaces_a_claim_on_the_same_id`, `inserting_a_claim_displaces_a_park_on_the_same_id`, any other `grep -n "Park {" src/claims.rs` finds), add `needs: None, minutes: None,` after `reason: ...,`.

```rust
    #[test]
    fn reason_parses_its_eight_values_only() {
        for (text, reason) in [
            ("review", Reason::Review),
            ("decision", Reason::Decision),
            ("approval", Reason::Approval),
            ("environment", Reason::Environment),
            ("dependency", Reason::Dependency),
            ("session", Reason::Session),
            ("capability", Reason::Capability),
            ("quiet", Reason::Quiet),
        ] {
            assert_eq!(Reason::parse(text).unwrap(), reason);
            assert_eq!(reason.as_str(), text);
        }
        match Reason::parse("boredom") {
            Err(Error::Validation(detail)) => {
                assert!(
                    detail.contains(
                        "review, decision, approval, environment, dependency, session, capability, quiet"
                    ),
                    "{detail}"
                );
                assert!(detail.contains("\"boredom\""), "{detail}");
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn needs_parses_its_two_values_only() {
        assert_eq!(Needs::parse("idle").unwrap(), Needs::Idle);
        assert_eq!(Needs::parse("headless").unwrap(), Needs::Headless);
        assert_eq!(Needs::Idle.as_str(), "idle");
        assert_eq!(Needs::Headless.as_str(), "headless");
        match Needs::parse("sometimes") {
            Err(Error::Validation(detail)) => {
                assert!(detail.contains("idle, headless"), "{detail}");
                assert!(detail.contains("\"sometimes\""), "{detail}");
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn describe_stop_appends_the_recipe_when_present() {
        assert_eq!(describe_stop(WaitingOn::User, None, None, None), "user");
        assert_eq!(
            describe_stop(WaitingOn::Agent, Some(Reason::Session), None, None),
            "agent, session"
        );
        assert_eq!(
            describe_stop(
                WaitingOn::User,
                Some(Reason::Quiet),
                Some(Needs::Idle),
                Some(50)
            ),
            "user, quiet; idle, 50 min"
        );
        assert_eq!(
            describe_stop(WaitingOn::User, Some(Reason::Quiet), Some(Needs::Headless), None),
            "user, quiet; headless",
            "a hand-written store may carry one field without the other"
        );
        assert_eq!(
            describe_stop(WaitingOn::User, Some(Reason::Quiet), None, Some(5)),
            "user, quiet; 5 min"
        );
    }

    #[test]
    fn a_quiet_park_round_trips_its_recipe_and_old_files_load() {
        let (dir, mut store) = store_from(A_PARK);
        let two = TaskId::parse("sci-000002").unwrap();
        assert_eq!(store.park(&two).unwrap().needs, None, "pre-recipe files load");
        assert_eq!(store.park(&two).unwrap().minutes, None);

        let three = TaskId::parse("sci-000003").unwrap();
        store.insert_park(
            &three,
            Park {
                owner: "o".into(),
                session: "a".into(),
                host: "h".into(),
                worktree: "/w".into(),
                at: "2026-09-13T10:00:00Z".into(),
                next_step: "rerun the preflight".into(),
                waiting_on: WaitingOn::User,
                reason: Some(Reason::Quiet),
                needs: Some(Needs::Headless),
                minutes: Some(50),
                title: "T".into(),
            },
        );
        store.save().unwrap();
        let path = dir.path().join("sci.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("reason = \"quiet\""), "{text}");
        assert!(text.contains("needs = \"headless\""), "{text}");
        assert!(text.contains("minutes = 50"), "{text}");
        assert_eq!(text.matches("needs =").count(), 1, "the old park writes no key: {text}");
        assert_eq!(text.matches("minutes =").count(), 1, "{text}");
        let reloaded = ClaimStore::load_from(&path).unwrap();
        let park = reloaded.park(&three).unwrap();
        assert_eq!(park.needs, Some(Needs::Headless));
        assert_eq!(park.minutes, Some(50));
    }

    #[test]
    fn the_store_does_not_police_a_recipe_without_the_quiet_reason() {
        let text = format!("{A_PARK}needs = \"idle\"\nminutes = 20\n");
        let (_dir, store) = store_from(&text);
        let two = TaskId::parse("sci-000002").unwrap();
        let park = store.park(&two).unwrap();
        assert_eq!(park.reason, None, "only `park` pairs the fields; the store loads them");
        assert_eq!(park.needs, Some(Needs::Idle));
        assert_eq!(park.minutes, Some(20));
    }
```

- [x] **Step 2: Write the failing integration tests in `tests/cli.rs`**

Add after line 1042:

```rust
#[test]
fn a_quiet_park_records_its_recipe_in_the_entry_the_note_and_every_park_view() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "rerun the readiness preflight, then capture",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "50",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "quiet");
    assert_eq!(v["park"]["needs"], "idle", "--needs defaults to idle: {v}");
    assert_eq!(v["park"]["minutes"], 50);
    assert_eq!(
        env.json(&sci, &["list", "--parked"])["tasks"][0]["park"]["minutes"],
        50
    );
    assert_eq!(
        env.json(&sci, &["prime"])["parked"][0]["park"]["needs"],
        "idle"
    );
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains(
            "parked (waiting on user, quiet; idle, 50 min): rerun the readiness preflight, then capture"
        ),
        "{raw}"
    );
    let text = env.pretty(&sci, &["show", &id]);
    assert!(text.contains("waiting on user, quiet; idle, 50 min since"), "{text}");
    let table = env.pretty(&sci, &["list", "--parked"]);
    assert!(table.contains("waits on user, quiet; idle, 50 min"), "{table}");

    // headless, waiting on the agent: `next` hands it back with the recipe
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "log out, then run the power capture",
            "--reason",
            "quiet",
            "--needs",
            "headless",
            "--minutes",
            "90",
        ])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], id);
    assert_eq!(next["next"]["park"]["needs"], "headless");
    assert_eq!(next["next"]["park"]["minutes"], 90);

    // a re-park under another reason drops the recipe
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "read the sheet", "--waiting-on", "user", "--reason", "review"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "review");
    assert!(v["park"]["needs"].is_null(), "{v}");
    assert!(v["park"]["minutes"].is_null(), "{v}");

    // a park without any reason prints null for both keys
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "write §3"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"]["needs"].is_null() && v["park"]["minutes"].is_null(), "{v}");
}

#[test]
fn quiet_park_flags_are_validated_together() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let err = error_of(&env, &sci, &["park", &id, "x", "--reason", "quiet"]);
    assert_eq!(err["error"]["kind"], "validation");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--reason quiet needs --minutes <n>"),
        "{err}"
    );

    let err = error_of(&env, &sci, &["park", &id, "x", "--minutes", "10"]);
    assert_eq!(err["error"]["kind"], "validation");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--minutes on park needs --reason quiet"),
        "{err}"
    );

    let err = error_of(&env, &sci, &["park", &id, "x", "--needs", "idle"]);
    assert_eq!(err["error"]["kind"], "validation");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--needs on park needs --reason quiet"),
        "{err}"
    );

    let err = error_of(
        &env,
        &sci,
        &["park", &id, "x", "--reason", "environment", "--minutes", "10"],
    );
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--minutes on park needs --reason quiet"),
        "another reason is refused the same way: {err}"
    );

    let err = error_of(
        &env,
        &sci,
        &["park", &id, "x", "--reason", "quiet", "--minutes", "10", "--needs", "sometimes"],
    );
    assert!(
        err["error"]["detail"].as_str().unwrap().contains("idle, headless"),
        "{err}"
    );

    let err = error_of(
        &env,
        &sci,
        &["park", &id, "x", "--reason", "quiet", "--minutes", "10", "--complexity", "high"],
    );
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--complexity on park needs --reason capability"),
        "{err}"
    );

    // the range is clap's: 0 and 1441 are usage errors, exit 2
    for bad in ["0", "1441"] {
        let out = env
            .cmd(&sci)
            .args(["park", &id, "x", "--reason", "quiet", "--minutes", bad])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "--minutes {bad}");
    }

    assert!(
        env.json(&sci, &["show", &id])["park"].is_null(),
        "nothing landed"
    );
}

#[test]
fn start_on_a_quiet_park_removes_the_entry_and_its_recipe() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "capture", "--waiting-on", "user", "--reason", "quiet", "--minutes", "5"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"].is_null(), "{v}");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(!store.contains("minutes"), "{store}");
}
```

- [x] **Step 3: Run both test sets to verify they fail**

Run: `cargo test --bin tasks claims::`
Expected: compile errors naming `Reason::Quiet`, `Needs`, `needs`, `minutes`, and the four-argument `describe_stop`.

Run: `cargo test --test cli quiet_park`
Expected: compile succeeds (the integration tests only drive the binary) and the tests fail: `--minutes` is an unknown argument (exit 2) and `--reason quiet` alone succeeds today.

- [x] **Step 4: Add `Reason::Quiet`, the `Needs` enum, the fields, and the recipe in `describe_stop`**

In `src/claims.rs`, change the `Reason` doc comment and enum:

```rust
/// Why the work stopped, from the eight-word vocabulary of
/// docs/specs/2026-09-11-park-reason-and-stamps-design.md §4,
/// docs/specs/2026-09-12-task-complexity-design.md §5, and
/// docs/specs/2026-09-13-quiet-queue-design.md §3. Orthogonal to `WaitingOn`:
/// the readers act on who, this only describes the stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Review,
    Decision,
    Approval,
    Environment,
    Dependency,
    Session,
    Capability,
    Quiet,
}

impl Reason {
    pub const ALL: [Reason; 8] = [
        Reason::Review,
        Reason::Decision,
        Reason::Approval,
        Reason::Environment,
        Reason::Dependency,
        Reason::Session,
        Reason::Capability,
        Reason::Quiet,
    ];
```

Add `Reason::Quiet => "quiet",` to `as_str`. After the `impl Reason` block add:

```rust
/// What "a free host" means for a quiet park (quiet-queue design §4): `idle`, the
/// desktop session may stay up but nothing else runs; `headless`, the ordinary desktop
/// session is stopped before the work starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Needs {
    Idle,
    Headless,
}

impl Needs {
    pub const ALL: [Needs; 2] = [Needs::Idle, Needs::Headless];

    pub fn parse(s: &str) -> Result<Needs> {
        Needs::ALL
            .into_iter()
            .find(|needs| needs.as_str() == s)
            .ok_or_else(|| {
                let accepted: Vec<&str> = Needs::ALL.iter().map(|needs| needs.as_str()).collect();
                Error::Validation(format!(
                    "--needs must be one of {}, got {s:?}",
                    accepted.join(", ")
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Needs::Idle => "idle",
            Needs::Headless => "headless",
        }
    }
}
```

Replace `describe_stop`:

```rust
/// The parenthetical the park note and the park views share: `user`, `user, review`, or
/// with a quiet recipe `user, quiet; idle, 50 min`. Either recipe field alone is still
/// printed: the store does not police the pairing, only `park` does.
pub fn describe_stop(
    who: WaitingOn,
    reason: Option<Reason>,
    needs: Option<Needs>,
    minutes: Option<u32>,
) -> String {
    let mut text = match reason {
        Some(reason) => format!("{}, {}", who.as_str(), reason.as_str()),
        None => who.as_str().to_string(),
    };
    let recipe: Vec<String> = needs
        .map(|needs| needs.as_str().to_string())
        .into_iter()
        .chain(minutes.map(|minutes| format!("{minutes} min")))
        .collect();
    if !recipe.is_empty() {
        text.push_str("; ");
        text.push_str(&recipe.join(", "));
    }
    text
}
```

In `Park`, after the `reason` field:

```rust
    /// With `reason: quiet`, what "free" means for this work. Absent otherwise and in
    /// store files written before the quiet queue existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<Needs>,
    /// With `reason: quiet`, expected wall-clock minutes once started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
```

- [x] **Step 5: Carry the fields through `ParkInfo` and completion**

In `src/output.rs`, change the two `describe_stop` calls (in `show_text` around line 942 and `parked_table` around line 1101) to `crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes)` and add to `ParkInfo`:

```rust
    pub needs: Option<crate::claims::Needs>,
    pub minutes: Option<u32>,
```

with `needs: park.needs, minutes: park.minutes,` in `ParkInfo::of` after `reason: park.reason,`. Serde derives `null` for `None` on `ParkInfo` already (it has no `skip_serializing_if`), which is the JSON contract.

In `src/complete.rs`, change the `reason()` doc comment to "The eight `park --reason` accepts." and add after it:

```rust
/// The two `park --needs` accepts.
pub fn needs() -> Vec<CompletionCandidate> {
    plain(crate::claims::Needs::ALL.iter().map(|needs| needs.as_str()))
}
```

- [x] **Step 6: Add the flags to the CLI and the dispatch**

In `src/cli.rs`, in the `Park` variant after the `complexity` field:

```rust
        /// With --reason quiet: what a free host means for this work, idle (the desktop
        /// may stay up but nothing else runs; the default) or headless (the ordinary
        /// desktop session is stopped first).
        #[arg(long, value_name = "COND", add = ArgValueCandidates::new(crate::complete::needs))]
        needs: Option<String>,
        /// With --reason quiet: expected wall-clock minutes once started, 1 to 1440.
        /// Required with that reason; refused with any other.
        #[arg(
            long,
            value_name = "N",
            value_parser = clap::value_parser!(u32).range(1..=1440)
        )]
        minutes: Option<u32>,
```

Update the `reason` field's doc comment to list `quiet`: "Why the work stopped: review, decision, approval, environment, dependency, session, capability, or quiet. Optional; absent means not recorded."

In `src/commands/mod.rs`, extend the dispatch:

```rust
        Command::Park {
            id,
            next_step,
            waiting_on,
            reason,
            complexity,
            needs,
            minutes,
        } => park::run(
            open_id_write_ctx(dir, &id)?,
            id,
            next_step,
            waiting_on,
            reason,
            complexity,
            needs,
            minutes,
        ),
```

- [x] **Step 7: Validate and record the recipe in `park::run`**

In `src/commands/park.rs`, add `Needs` to the `use crate::claims::{...}` line and change the signature, with the repo's targeted allowance above it:

```rust
/// Set a task down (spec §3). Validate everything, refuse a foreign live claim, then let
/// `save` write the note and the entry in that order.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mut ctx: Ctx,
    id: String,
    next_step: String,
    waiting_on: String,
    reason: Option<String>,
    complexity: Option<String>,
    needs: Option<String>,
    minutes: Option<u32>,
) -> Result<Output> {
```

Immediately *after* the whole `let escalation = match (reason, complexity) { ... };` block (so a caller who passes both wrong flags gets the complexity error first, as the test expects) and before `if let Some(level) = complexity`, add:

```rust
    // Quiet-queue design §4: the recipe belongs to --reason quiet alone. --minutes is
    // required there (the queue is read before bed, so the length must be known) and
    // --needs defaults to idle; with any other reason, or none, both are refused.
    let needs = needs.as_deref().map(Needs::parse).transpose()?;
    let (needs, minutes) = match (reason, needs, minutes) {
        (Some(Reason::Quiet), needs, Some(minutes)) => {
            (Some(needs.unwrap_or(Needs::Idle)), Some(minutes))
        }
        (Some(Reason::Quiet), _, None) => {
            return Err(Error::Validation(
                "--reason quiet needs --minutes <n>".into(),
            ));
        }
        (_, Some(_), _) => {
            return Err(Error::Validation(
                "--needs on park needs --reason quiet".into(),
            ));
        }
        (_, None, Some(_)) => {
            return Err(Error::Validation(
                "--minutes on park needs --reason quiet".into(),
            ));
        }
        (_, None, None) => (None, None),
    };
```

Change the note call to `describe_stop(waiting_on, reason, needs, minutes)` and the `Park` literal to carry `needs, minutes,` after `reason,`. `grep -rn "Park {\|describe_stop(" src/` must then show no other call site without the new fields or arguments.

- [x] **Step 8: Run the tests and the gate**

Run: `cargo test --bin tasks claims::`
Expected: ok, every unit test passes.

Run: `cargo test --test cli park`
Expected: ok, including the three new tests and the pre-existing reason tests.

Run: `just check`
Expected: fmt, clippy, and `tasks check` all pass. A `dead_code` or `too_many_arguments` failure here means a helper landed without its consumer; fix it in this task, never with a broader `allow`.

- [x] **Step 9: Commit**

```bash
tasks done tasks-fff91a "quiet reason, Needs enum, Park.needs/minutes, park --needs/--minutes with validation, the note form, and park.needs/minutes in every view"
git add src/claims.rs src/complete.rs src/cli.rs src/commands/mod.rs src/commands/park.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat(park): quiet reason with a needs and minutes recipe"
```

---

### Task 2: A recorded-first resolver and `tasks quiet`

**Files:**
- Modify: `src/commands/parked.rs`
- Create: `src/commands/quiet.rs`
- Modify: `src/cli.rs` (a new `Quiet` variant after `Tags`), `src/commands/mod.rs` (`pub mod quiet;` and dispatch), `src/output.rs` (`Output::Quiet`, `QuietOut`, `quiet_briefs`, `warnings_of`)
- Test: `tests/cli.rs` (new tests after `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next`, which ends around line 5935; one line in `project_and_all_projects_conflict_on_every_read_command` at line 1945)

**Interfaces:**
- Consumes: `claims::Reason::Quiet`, `claims::Needs`, `ParkInfo.needs`/`.minutes` from Task 1; `Scope::projects(&self) -> &[Project]` (`src/scope.rs:110`); `Project.root: PathBuf` (`src/repo.rs:70`); `open_read_ctx(dir, &ScopeArgs)` from `src/commands/mod.rs`; the test helpers `as_agent`, `id_of`, `error_of` already in `tests/cli.rs` (`two_roots` is *not* used: `init_forced` re-points the registry at the second root, and these tests need the worktree unregistered).
- Produces: `parked::Prefer { Registered, Recorded }`; `parked::rows_preferring(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot, now: OffsetDateTime, prefer: Prefer, reason: Option<Reason>) -> Result<Vec<ParkedRow>>`; `parked::rows(...)` unchanged in signature, equal to `rows_preferring(..., Prefer::Registered, None)`; `Command::Quiet { limit: Option<usize>, project: Option<String>, all_projects: bool }`; `quiet::run(ctx: ReadCtx, limit: Option<usize>) -> Result<Output>`; `Output::Quiet(QuietOut { tasks: Vec<ParkedRow>, warnings: Vec<String> })`; `output::quiet_briefs(rows: &[ParkedRow], painter: &Painter) -> String`.

The resolver refactor and the command land in one commit because `Prefer::Recorded` has no constructor until `quiet::run` exists, and an unconstructed variant fails the gate. The refactor is still verified on its own: step 3 runs the whole suite after it, before the command exists.

- [x] **Step 1: Write the failing integration tests**

Add to `tests/cli.rs` after `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next`:

```rust
/// A second checkout of an already-registered prefix that the registry does not point
/// at: what a git worktree is to the tracker. (`init_forced` would re-point the
/// registry, which is the opposite of what these tests need.)
fn unregistered_checkout(prefix: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(path.join("tasks")).unwrap();
    std::fs::write(
        path.join("tasks/.config.toml"),
        format!("prefix = \"{prefix}\"\n"),
    )
    .unwrap();
    (dir, path)
}

#[test]
fn a_park_whose_task_file_was_deleted_in_its_own_checkout_still_warns_unavailable() {
    // Guards the resolver's shortcut: when the recorded worktree is the scanned root
    // but the task file is gone, the parked views must still say so, as they did before
    // the preference existed.
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "finish", "--waiting-on", "user", "--reason", "quiet", "--minutes", "5"])
        .assert()
        .success();
    std::fs::remove_file(sci.join(format!("tasks/{id}.md"))).unwrap();

    for args in [vec!["list", "--parked"], vec!["prime"], vec!["quiet"]] {
        let v = env.json(&sci, &args);
        let rows = if args[0] == "prime" { &v["parked"] } else { &v["tasks"] };
        let row = &rows.as_array().unwrap()[0];
        assert_eq!(row["id"], id, "{args:?}: {v}");
        assert!(row["status"].is_null(), "store-only: {v}");
        assert_eq!(row["title"], "T");
        assert!(
            v["warnings"]
                .to_string()
                .contains("which is unavailable; the row shows the park entry only"),
            "{args:?}: {v}"
        );
    }
}

#[test]
fn quiet_lists_quiet_parks_across_projects_by_priority_then_park_time() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();

    let empty = env.json(nowhere.path(), &["quiet"]);
    assert_eq!(empty, serde_json::json!({"tasks": [], "warnings": []}));
    assert_eq!(env.pretty(nowhere.path(), &["quiet"]).trim(), "");

    let later = id_of(env.json(&sci, &["add", "Later capture", "-p", "2"]));
    let review = id_of(env.json(&sci, &["add", "Sheet", "-p", "0"]));
    let first = id_of(env.json(&fam, &["add", "First capture", "-p", "2"]));
    let urgent = id_of(env.json(&fam, &["add", "Urgent sweep", "-p", "1"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &review, "look", "--waiting-on", "user", "--reason", "review"])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args(["park", &first, "run it", "--waiting-on", "user", "--reason", "quiet", "--minutes", "30"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &later, "run it too", "--waiting-on", "user", "--reason", "quiet", "--minutes", "45", "--needs", "headless"])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args(["park", &urgent, "sweep", "--reason", "quiet", "--minutes", "5"])
        .assert()
        .success();

    let v = env.json(nowhere.path(), &["quiet"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [urgent.as_str(), first.as_str(), later.as_str()], "{v}");
    assert_eq!(v["warnings"], serde_json::json!([]));
    assert_eq!(v["tasks"][2]["park"]["needs"], "headless");
    assert_eq!(v["tasks"][2]["park"]["minutes"], 45);
    assert_eq!(v["tasks"][2]["park"]["worktree"], sci.to_string_lossy().as_ref());

    let one = env.json(nowhere.path(), &["quiet", "-n", "1"]);
    assert_eq!(one["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(one["tasks"][0]["id"], urgent);

    let only_sci = env.json(nowhere.path(), &["quiet", "--project", "sci"]);
    assert_eq!(only_sci["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(only_sci["tasks"][0]["id"], later);

    let explicit = env.json(&sci, &["quiet", "--all-projects"]);
    assert_eq!(explicit["tasks"].as_array().unwrap().len(), 3, "redundant, accepted");

    let text = env.pretty(nowhere.path(), &["quiet"]);
    let brief = text.lines().collect::<Vec<_>>();
    assert!(brief[0].starts_with(&urgent), "{text}");
    assert!(brief[0].contains("P1") && brief[0].contains("idle") && brief[0].contains("5 min"), "{text}");
    assert!(brief[0].contains("Urgent sweep"), "{text}");
    assert!(brief[1].trim_start().starts_with("next: sweep"), "{text}");
    assert!(
        brief[2].trim_start().starts_with("in:") && brief[2].contains(&*fam.to_string_lossy()),
        "{text}"
    );
    assert_eq!(brief[3], "", "one blank line between briefs: {text}");
    assert!(text.contains("headless") && text.contains("45 min"), "{text}");
}

#[test]
fn quiet_resolves_from_the_recorded_checkout_before_the_registered_copy() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    let (_keep, wt) = unregistered_checkout("sci");
    let id = id_of(env.json(&main, &["add", "Capture", "-p", "3"]));
    std::fs::copy(
        main.join(format!("tasks/{id}.md")),
        wt.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    env.json(&main, &["done", &id, "closed on main"]);
    env.json(&wt, &["edit", &id, "-p", "1"]);
    as_agent(&env, &wt, "agent-a")
        .args(["park", &id, "capture", "--waiting-on", "user", "--reason", "quiet", "--minutes", "20"])
        .assert()
        .success();

    // the parked listing from the registered root still trusts its own (done) copy
    let parked = env.json(&main, &["list", "--parked"]);
    assert!(parked["tasks"].as_array().unwrap().is_empty(), "{parked}");

    let v = env.json(&main, &["quiet"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["tasks"][0]["id"], id);
    assert_eq!(v["tasks"][0]["status"], "todo");
    assert_eq!(v["tasks"][0]["priority"], 1, "the worktree's priority, not main's");
    assert_eq!(v["tasks"][0]["park"]["worktree"], wt.to_string_lossy().as_ref());
    assert!(
        v["warnings"].to_string().contains("resume it from that checkout"),
        "{v}"
    );

    // the worktree gone: fall back to the registered copy, which is closed, so excluded
    std::fs::remove_dir_all(wt.join("tasks")).unwrap();
    let v = env.json(&main, &["quiet"]);
    assert!(v["tasks"].as_array().unwrap().is_empty(), "{v}");
    assert!(
        v["warnings"].to_string().contains("showing the registered copy"),
        "{v}"
    );
}

#[test]
fn quiet_keeps_scan_warnings_when_the_queue_is_empty() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let v = env.json(&sci, &["quiet"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1, "{v}");
    assert!(v["warnings"][0].as_str().unwrap().contains("fam"), "{v}");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "quiet"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
    assert!(String::from_utf8_lossy(&out.stderr).contains("unreachable"));
}

#[test]
fn quiet_includes_a_store_only_entry_last() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let real = id_of(env.json(&sci, &["add", "Real", "-p", "3"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &real, "run", "--waiting-on", "user", "--reason", "quiet", "--minutes", "10"])
        .assert()
        .success();
    let path = env.claim_store("sci");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "[parks.\"sci-0000ff\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\n\
         worktree = \"/gone\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"finish\"\n\
         waiting_on = \"user\"\nreason = \"quiet\"\nneeds = \"idle\"\nminutes = 15\ntitle = \"Ghost\"\n",
    );
    std::fs::write(&path, text).unwrap();
    let v = env.json(&sci, &["quiet", "--project", "sci"]);
    let rows = v["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{v}");
    assert_eq!(rows[0]["id"], real);
    assert_eq!(rows[1]["id"], "sci-0000ff", "no priority sorts last");
    assert!(rows[1]["priority"].is_null());
    assert_eq!(rows[1]["title"], "Ghost");
    assert_eq!(rows[1]["park"]["minutes"], 15);
    let text = env.pretty(&sci, &["quiet", "--project", "sci"]);
    assert!(text.contains("sci-0000ff  P-"), "{text}");
}
```

And in `project_and_all_projects_conflict_on_every_read_command`, change the loop list to `["list", "ready", "next", "prime", "tree", "tags", "quiet"]`.

- [x] **Step 2: Run the new tests to verify they fail**

Run: `cargo test --test cli quiet`
Expected: the `quiet_*` tests and `a_park_whose_task_file_was_deleted_in_its_own_checkout_still_warns_unavailable` fail with exit 2, `quiet` is not a subcommand (the deleted-file test fails on its `quiet` iteration; its `list --parked` and `prime` iterations pass today, which is the point).

- [x] **Step 3: Restructure the resolver around a preference**

Replace the body of `src/commands/parked.rs` from `pub fn rows` through the end of `fn resolve_elsewhere` (keep the `candidates` function below untouched) with:

```rust
/// Which copy of a parked task a view trusts when the registered checkout and the
/// park's recorded worktree both hold it. The parked listing and `prime` read the
/// checkout they were run in (`Registered`); the quiet queue reads where the work was
/// actually set down (`Recorded`), since a task reopened and parked in a worktree may
/// still read `done`, or carry a stale priority, in the registered copy
/// (quiet-queue design §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefer {
    Registered,
    Recorded,
}

pub fn rows(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
) -> Result<Vec<ParkedRow>> {
    rows_preferring(ctx, all, claims, now, Prefer::Registered, None)
}

pub fn rows_preferring(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    prefer: Prefer,
    reason: Option<Reason>,
) -> Result<Vec<ParkedRow>> {
    let mut entries: Vec<(&String, &Park)> = claims
        .parks()
        .filter(|(_, park)| reason.is_none_or(|reason| park.reason == Some(reason)))
        .collect();
    entries.sort_by(|a, b| b.1.at.cmp(&a.1.at).then_with(|| a.0.cmp(b.0)));
    let mut rows = Vec::new();
    for (key, park) in entries {
        let id = match TaskId::parse(key) {
            Ok(id) => ctx.registry.canonical_id(&id),
            Err(error) => {
                ctx.warnings.push(format!(
                    "park entry {key:?} is not a task id ({error}); shown as is"
                ));
                rows.push(ParkedRow::unresolved(key, park));
                continue;
            }
        };
        let registered = all
            .iter()
            .find(|task| task.id == id)
            .map(|task| resolved_row(task, all, claims, &ctx.registry, now));
        // When the recorded worktree is one of the scanned roots *and* the scan found
        // the task, the registered copy is the recorded copy: opening the root again
        // would only add a misleading "resume it from that checkout". A scanned root
        // that lacks the file still goes through `recorded_or_fallback`, so the
        // "unavailable" warning the parked views have always given survives.
        let recorded_is_scanned = ctx
            .scope
            .projects()
            .iter()
            .any(|project| project.root == Path::new(&park.worktree));
        let row = match (prefer, registered) {
            (_, Some(row)) if recorded_is_scanned => Some(row),
            (Prefer::Registered, Some(row)) => Some(row),
            (Prefer::Registered, None) => {
                recorded_or_fallback(ctx, &id, park, claims, now, None)
            }
            (Prefer::Recorded, registered) => {
                recorded_or_fallback(ctx, &id, park, claims, now, registered)
            }
        };
        rows.push(row.unwrap_or_else(|| ParkedRow::unresolved(key, park)));
    }
    Ok(rows)
}

fn resolved_row(
    task: &Task,
    all: &[Task],
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> ParkedRow {
    ParkedRow::resolved(
        TaskSummary::of(task, all, Some(claims), registry, now),
        Phase::of(task),
    )
}

/// The row from the park's recorded worktree, with the "resume it from that checkout"
/// warning; else `fallback` (the registered copy, when the caller preferred the recorded
/// one and has it) with the "unavailable" warning; else `None`, which the caller renders
/// store-only. Never errors: an unreadable worktree is a warning, not a failed listing.
fn recorded_or_fallback(
    ctx: &mut ReadCtx,
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    fallback: Option<ParkedRow>,
) -> Option<ParkedRow> {
    match resolve_recorded(id, park, claims, &ctx.registry, now) {
        Ok(Some(row)) => {
            ctx.warnings.push(format!(
                "{id} is parked in {}; resume it from that checkout",
                park.worktree
            ));
            Some(row)
        }
        Ok(None) => {
            ctx.warnings
                .push(unavailable(id, park, None, fallback.is_some()));
            fallback
        }
        Err(error) => {
            ctx.warnings
                .push(unavailable(id, park, Some(&error), fallback.is_some()));
            fallback
        }
    }
}

/// The two pre-existing "unavailable" messages byte for byte, plus the fallback form
/// the quiet queue adds.
fn unavailable(
    id: &TaskId,
    park: &Park,
    error: Option<&crate::error::Error>,
    fell_back: bool,
) -> String {
    let cause = match error {
        Some(error) => format!(" ({error})"),
        None => String::new(),
    };
    let shown = if fell_back {
        "showing the registered copy"
    } else {
        "the row shows the park entry only"
    };
    format!(
        "{id} is parked in {}, which is unavailable{cause}; {shown}",
        park.worktree
    )
}

fn resolve_recorded(
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<ParkedRow>> {
    let root = Path::new(&park.worktree);
    if !crate::scope::has_config(root)? {
        return Ok(None);
    }
    let project = Project::open(root)?;
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let scan = project.scan()?;
    let Some(task) = scan.iter().find(|task| task.id == *id) else {
        return Ok(None);
    };
    Ok(Some(resolved_row(task, &scan, claims, registry, now)))
}
```

Compare `unavailable`'s two `fell_back == false` outputs against the two `format!` strings in `git show HEAD:src/commands/parked.rs`; they must be identical, because `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next` asserts on "which is unavailable" and the new deleted-file test asserts the full phrase.

Run the whole suite now, before the command exists, to prove the refactor changed nothing observable: `cargo test`. Expected: every pre-existing test passes; only the six new tests from step 1 fail. (`Prefer::Recorded` is unconstructed at this point, so `just check` would fail on `dead_code`; that is why this step does not commit.)

- [x] **Step 4: Add the CLI variant and the dispatch**

In `src/cli.rs`, after the `Tags` variant:

```rust
    /// Work parked waiting for an idle host, as resume briefs: every reachable project
    /// by default, priority first, then oldest park first.
    Quiet {
        /// At most this many briefs; -n 1 is the top of the queue.
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// One registered project instead of all of them; needs no local project.
        #[arg(
            long,
            conflicts_with = "all_projects",
            add = ArgValueCandidates::new(crate::complete::prefixes)
        )]
        project: Option<String>,
        /// The default; accepted so the flag reads like the other read commands.
        #[arg(long)]
        all_projects: bool,
    },
```

In `src/commands/mod.rs`: add `pub mod quiet;` to the module list (alphabetical, after `projects`), and after the `Command::Tags` dispatch:

```rust
        Command::Quiet {
            limit,
            project,
            all_projects: _,
        } => {
            // Quiet-queue design §5: the bedtime question is cross-project, so the scope
            // is every project unless one is named.
            let scope = ScopeArgs {
                all_projects: project.is_none(),
                project,
            };
            quiet::run(open_read_ctx(dir, &scope)?, limit)
        }
```

- [x] **Step 5: Write `src/commands/quiet.rs`**

```rust
//! `tasks quiet`: the queue of work parked waiting for an idle host
//! (docs/specs/2026-09-13-quiet-queue-design.md §5).
//!
//! Every park with reason `quiet` in scope, resolved from its recorded checkout first,
//! closed tasks dropped after resolution, ordered priority first then oldest park first.
//! Warnings from the scan and from resolution are kept whether or not a row survives:
//! an empty queue with no warnings is the only "nothing to run tonight".

use super::ReadCtx;
use super::parked::{Prefer, rows_preferring};
use crate::claims::Reason;
use crate::error::Result;
use crate::output::{Output, ParkedRow, QuietOut};

pub fn run(mut ctx: ReadCtx, limit: Option<usize>) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let rows = rows_preferring(
        &mut ctx,
        &all,
        &claims,
        now,
        Prefer::Recorded,
        Some(Reason::Quiet),
    )?;
    let mut tasks: Vec<ParkedRow> = rows
        .into_iter()
        .filter(|row| {
            row.park
                .as_ref()
                .is_some_and(|park| park.reason == Some(Reason::Quiet))
        })
        .filter(|row| row.status.is_none_or(|status| status.is_open()))
        .collect();
    tasks.sort_by(|a, b| {
        let priority = |row: &ParkedRow| row.priority.unwrap_or(u8::MAX);
        let at = |row: &ParkedRow| {
            row.park
                .as_ref()
                .map(|park| park.at.clone())
                .unwrap_or_default()
        };
        priority(a)
            .cmp(&priority(b))
            .then_with(|| at(a).cmp(&at(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::Quiet(QuietOut {
        tasks,
        warnings: ctx.warnings,
    }))
}
```

`quiet` passes `Some(Reason::Quiet)` so unrelated review, decision, and other parks are filtered before recorded-checkout resolution; their warnings never leak into the quiet output. `list --parked` and `prime` pass `None` and retain warnings for every parked entry.

- [x] **Step 6: Add the output type and the pretty briefs**

In `src/output.rs`, after `ParkedOut`:

```rust
#[derive(Serialize)]
pub struct QuietOut {
    pub tasks: Vec<ParkedRow>,
    pub warnings: Vec<String>,
}
```

Add `Quiet(QuietOut),` to `enum Output` after `Parked(ParkedOut),`. In `pretty()` add `Output::Quiet(o) => quiet_briefs(&o.tasks, painter),` after the `Parked` arm. In `warnings_of` add `Output::Quiet(o) => o.warnings.clone(),`. After `parked_table` add:

```rust
/// One brief per quiet park (quiet-queue design §5): the row, the next step, and the
/// checkout to open an agent in. No launch command: that belongs to the harness.
pub fn quiet_briefs(rows: &[ParkedRow], painter: &Painter) -> String {
    let mut rendered = String::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(park) = &row.park else {
            continue;
        };
        if index > 0 {
            rendered.push('\n');
        }
        let id = painter.paint(Style::Chrome, &row.id);
        let priority = match row.priority {
            Some(priority) => format!("P{priority}"),
            None => "P-".into(),
        };
        let needs = park.needs.map(crate::claims::Needs::as_str).unwrap_or("-");
        let minutes = match park.minutes {
            Some(minutes) => format!("{minutes} min"),
            None => "-".into(),
        };
        rendered.push_str(&format!(
            "{id}  {priority}  {needs:<8}  {minutes:>8}  parked {}  {}\n",
            crate::time::day(&park.at),
            row.title
        ));
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("        next: {}", park.next_step),
        ));
        rendered.push('\n');
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("        in:   {}", park.worktree),
        ));
        rendered.push('\n');
    }
    rendered
}
```

- [x] **Step 7: Run the tests and the gate**

Run: `cargo test --test cli quiet`
Expected: ok; the six new tests and the conflict test pass. If `quiet_lists_quiet_parks_across_projects_by_priority_then_park_time` fails on the `worktree` assertion, `park.worktree` is `ctx.project.root.display()` and `TestEnv::init` canonicalizes the temp path, so the two should match; if not, compare after `canonicalize()` on the test side.

Run: `cargo test`
Expected: ok, whole suite.

Run: `just check`
Expected: passes; `Prefer::Recorded` now has its constructor in `quiet::run`.

- [x] **Step 8: Commit**

```bash
tasks done tasks-0f780e "parked resolver takes a registered-or-recorded preference; tasks quiet lists quiet parks across projects as resume briefs, recorded checkout first, warnings kept"
git add src/commands/parked.rs src/commands/quiet.rs src/cli.rs src/commands/mod.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat(cli): tasks quiet lists work parked for an idle host as resume briefs"
```

---

### Task 3: Docs, skill, spec status, reinstall

**Files:**
- Modify: `skills/tasks/SKILL.md` (steps 2 and 5)
- Modify: `README.md` (the reason table around line 66; the command list around line 117)
- Modify: `docs/specs/2026-09-13-quiet-queue-design.md` (status line)
- Modify: `src/claims.rs`, `src/complete.rs` doc comments if any still say "seven"

**Interfaces:** none; documentation only.

- [x] **Step 1: Skill step 5: the reason and the recipe**

In `skills/tasks/SKILL.md` step 5, change the sentence listing the reasons so it ends `..., \`session\` (the session is ending before the work is), \`capability\` (the work needs more reasoning than this session can supply), \`quiet\` (the host is in use; the work is prepared and unattended and needs only an idle machine).` Then, after the sentence "An environment or credential failure is `--reason environment` and never raises the rating.", add:

```
   When a preflight or benchmark refuses on host load (CPU, GPU, load average, a
   competing application), park with
   `tasks park <id> "<the check to rerun, then what follows>" --reason quiet --waiting-on user --minutes <n>`,
   adding `--needs headless` when the desktop session itself is the load and must be
   stopped first (`idle`, the default, means the desktop may stay up but nothing else
   runs). `--minutes` is the expected wall-clock length once started and is required;
   the queue is read before bed. `quiet` is not `environment` (a missing tool or a
   restart) and not `decision` (a session the person must attend).
```

- [x] **Step 2: Skill step 2: where the queue is read**

In step 2, after the sentence ending "`tasks next --project <prefix>` reads one of them.", add:

```
   `tasks quiet` lists work parked waiting for an idle host across every registered
   project, priority first, as resume briefs with the checkout to open; `-n 1` is the
   top of the queue and `--project <prefix>` narrows it. It is the person's bedtime
   view, not a picker: resume an entry by opening a session in the checkout it names
   and running `tasks start <id>` there.
```

- [x] **Step 3: README**

Add a row to the reason table after `capability`:

```
| `quiet`       | the host is in use; the work is prepared, unattended, and needs only an idle machine — `--minutes <n>` (required) and `--needs idle\|headless` record the recipe |
```

After the sentence "`start` resumes it, and `prime` lists parked work first." add: "`tasks quiet` lists quiet parks across every registered project as resume briefs (design: `docs/specs/2026-09-13-quiet-queue-design.md`)." In the command list, after the `tasks park <id> "next step" --reason review` line add:

```
    tasks park <id> "rerun the preflight" --reason quiet --waiting-on user --minutes 50  # needs an idle host
    tasks quiet                      # what could run tonight, across every project; -n 1 for the top
```

- [x] **Step 4: Doc comments and spec status**

`grep -rn "seven" src/claims.rs src/complete.rs` and change each vocabulary count to eight. In `docs/specs/2026-09-13-quiet-queue-design.md`, change `Status: draft (2026-09-13)` to `Status: implemented (<today's date>)`.

- [x] **Step 5: Reinstall and verify the tracker used by the protocol is the new code**

Run: `cargo install --path .`
Expected: succeeds.

Run: `tasks quiet --pretty`
Expected: exit 0 and the real queue: the quiet parks across the registered projects, or nothing if none has been re-parked under `quiet` yet (existing entries still carry `environment`; re-parking them is the projects' business, not this task's).

- [x] **Step 6: Commit**

```bash
just check
tasks done tasks-ef99a5 "skill steps 2 and 5, README table and commands, doc-comment counts, spec status implemented, binary reinstalled"
git add skills/tasks/SKILL.md README.md docs/specs/2026-09-13-quiet-queue-design.md src/claims.rs src/complete.rs tasks/
git commit -m "docs(quiet): skill, README, and spec status for the quiet queue"
```

## Self-review

- **Spec coverage.** §3 vocabulary, independence, not-an-escalation: Task 1 (the escalation match already refuses `--complexity` for non-capability reasons; a test asserts it). §4 required minutes, default needs, refusals, `Needs` enum, store fields, note form, rows, host-name separation: Task 1. §5 default scope, recorded-first resolution with the three-step fallback and the shared resolver, ordering, `-n`, JSON shape, pretty briefs, empty-with-warnings: Task 2. §6 docs: Task 3. §7 tests: every bullet maps to a test in Task 1 or Task 2; the clap range makes `--minutes 0`/`1441` exit 2, which the spec's "fail" allows and Task 1's test pins. The deleted-file test in Task 2 guards the resolver shortcut, which the spec does not name but the refactor requires.
- **Gate.** Each task's commit is the first point at which every symbol it introduces has a production consumer: `Needs::parse` and `complete::needs` are consumed by `park` in Task 1; `Prefer::Recorded` by `quiet::run` in Task 2. `park::run` carries the targeted `too_many_arguments` allowance. No test or check command is piped.
- **Placeholders.** None; every step carries its code.
- **Type consistency.** `describe_stop(who, reason, needs, minutes)` is the Task 1 signature used in Task 2's views; `Prefer::Recorded`, `rows_preferring`, and `recorded_or_fallback` are named identically throughout Task 2; `QuietOut { tasks, warnings }` matches `Output::Quiet` and `warnings_of`; `complete::needs` is named the same in the CLI attribute and the module.
- **Tracker.** Three step tasks, `tasks-fff91a` (Task 1), `tasks-0f780e` (Task 2), `tasks-ef99a5` (Task 3), with `0f780e` depending on `fff91a` and `ef99a5` on `0f780e`, so `ready` offers them in order.
