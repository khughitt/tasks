# Claim PID View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Judge a claim's pid only from the PID view it was established in, and give every other judgement a protective `Unverifiable` verdict that nothing prunes, takes over without `--force`, or releases holds on.

**Architecture:** A new `src/pidview.rs` reads the caller's PID view: the PID namespace that its `/proc` numbers pids in, identified by the nsfs `(dev, ino)`, and unknown unless `NSpid` has one entry. The claim write in `commands/mod.rs::claim_guard` records the view through a pure `claims::evidence` function. That function applies the establishment rules and the heartbeat table of spec §2. `claims::liveness_with` takes the current view and returns `Unverifiable` when the recorded and current views are absent, unknown or different. Every call site then asks `is_stale()` instead of comparing with `Live`.

**Tech Stack:** Rust (serde, toml, clap, assert_cmd integration tests), Linux procfs.

**Spec:** `docs/specs/2026-10-14-claim-pid-view-design.md` (accepted at revision 3, spec round 3). Read it with this plan. Section numbers below refer to it.

## Global Constraints

- The claim field is `pid_view: Option<String>`, formatted `"<dev>:<ino>"`, serialized only when present (spec §2).
- A view is known only when `/proc/self/status` has exactly one `NSpid` entry and `/proc/self/ns/pid` stats. Any failed read means unknown (spec §1).
- The initial PID namespace is recognized by nsfs inode `PROC_PID_INIT_INO` = `0xEFFFFFFC` (4026531836) (spec §2).
- Exported pids (`TASKS_SESSION_PID`, `CLAUDE_PID`) establish a view only when the writer's known view is the initial namespace and `proc_stat` finds the pid. Numeric or ancestry matches never establish one (spec §2).
- Verdict order for a claim with a pid: a known differing boot gives `Stale`; then an absent recorded view, an unknown current view or differing views give `Unverifiable`; then today's checks run (spec §3).
- `Unverifiable` reasons are exactly `recorded in another PID view`, `PID view not established when recorded` and `this command cannot establish its PID view`. They carry no pid or namespace values (spec §3).
- Only `Stale` permits automatic action (spec §4).
- JSON: `ClaimInfo.live` is `true` for `Live` and `Unverifiable`. The new optional `unverified` holds the reason, present only for `Unverifiable` (spec §5).
- The takeover note wording is `unverifiable, forced`, and the claim description wording is `unverifiable: <why>` (spec §4).
- No new warnings (spec §4).
- Tests go through `just test-one <args>` and `just test-fast`; never `cargo test` directly (AGENTS.md).
- Conventional commits, no AI-attribution trailers.

## Review Focus

1. **A claim store written by an older binary, with `pid` and no `pid_view`.** It must load and read `Unverifiable` ("PID view not established when recorded"), never `Stale`, even when its pid is gone. Test in Task 2 (`a_legacy_pid_claim_is_unverifiable_even_when_its_pid_is_gone`).
2. **A malformed recorded `pid_view` string** such as `"garbage"`. It must compare unequal and read `Unverifiable`, never panic or read `Stale`. Test in Task 2 (`a_malformed_recorded_view_is_unverifiable`).
3. **`sid:` with session id 0**, from a session leader outside the procfs's namespace. It must record no view. Test in Task 1 (`a_zero_session_id_establishes_nothing`).
4. **The owner's repeated `start` with `TASKS_SESSION` and no `TASKS_SESSION_PID`**, after a claim that recorded a pid and a view. It must keep the earlier pid, start and view (heartbeat row "absent"). Test in Task 1 (`a_heartbeat_without_a_pid_keeps_the_recorded_evidence`).
5. **`tasks claims --pretty` and `prime --pretty` showing an `Unverifiable` claim.** They must say `unverifiable`, not `stale` and not `live`. Test in Task 2 (`pretty_claims_name_an_unverifiable_claim`).

## Tracking

Task tasks-860db2 owns this plan. One child per `### Task N:` heading. Each child is `--process direct --complexity mid`: the plan settles the approach, and each step still needs care at its call sites or namespace cases. Run `tasks start <child>` before implementing a step and `tasks done <child>` when its commit lands. tasks-860db2 closes after Task 3 and the final whole-branch review.

## Setup

- Work in `.worktrees/claim-pid-view` on branch `fix/claim-pid-view`. It already exists and holds the claim.
- The justfile defines no `setup` recipe and AGENTS.md names no setup command, so none runs.
- Before Task 1, run the baseline: `just test-fast` must pass on the branch as it stands (docs-only changes so far). Record the result in a task note.

---

### Task 1: Record the PID view and the process evidence

**Files:**
- Create: `src/pidview.rs`
- Modify: `src/main.rs` (add `mod pidview;` in alphabetical order, after `mod periodic;`)
- Modify: `src/claims.rs`:
  - `Identity` (lines 12-19) and `identity_from` (lines 54-130);
  - `Claim` (lines 142-163);
  - `continuation_identity` (lines 906-923);
  - a new `PidSource`, `Evidence` and `evidence`, placed after `boot_id()` (around line 815);
  - every `Claim { … }` literal in tests.
- Modify: `src/commands/mod.rs:520-550` (the `ClaimIntent::Acquire` construction in `claim_guard`)
- Modify: `src/output.rs`, the three `crate::claims::Claim { … }` test literals (add `pid_view: None`)
- Test: unit tests in `src/pidview.rs` and `src/claims.rs`; integration tests in `tests/cli.rs`

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces:
  - `crate::pidview::PidView { pub dev: u64, pub ino: u64 }`, which derives `Debug, Clone, Copy, PartialEq, Eq`;
  - `PidView::is_initial(&self) -> bool`;
  - `impl std::fmt::Display for PidView`, giving `"{dev}:{ino}"`;
  - `crate::pidview::PROC_PID_INIT_INO: u64`;
  - `crate::pidview::from_parts(status: Option<&str>, ns: Option<(u64, u64)>) -> Option<PidView>`;
  - `crate::pidview::current() -> Option<PidView>`, cached per process;
  - `crate::claims::PidSource { Relay, UnixSession, Exported }`;
  - `Identity.pid_source: Option<PidSource>`;
  - `Claim.pid_view: Option<String>`;
  - `crate::claims::Evidence { pid, pid_start, boot_id, pid_view }`;
  - `crate::claims::evidence(me, own, view, boot_id, stat) -> Evidence`.

- [ ] **Step 1: Write the failing view-reader tests**

Create `src/pidview.rs` with only the tests module, so the module compiles once its items exist:

```rust
//! The caller's PID view: the PID namespace its `/proc` numbers pids in (claim PID-view
//! design §1). A pid means something only together with the view it was read through.

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = "Name:\tx\nNSpid:\t1584044\nNSpgid:\t1\n";

    #[test]
    fn one_nspid_entry_names_the_callers_namespace() {
        assert_eq!(
            from_parts(Some(ONE), Some((5, 4026531836))),
            Some(PidView { dev: 5, ino: 4026531836 })
        );
    }

    #[test]
    fn an_ancestor_procfs_is_unknown() {
        let two = "NSpid:\t1678460\t2\n";
        assert_eq!(from_parts(Some(two), Some((5, 4026533721))), None);
    }

    #[test]
    fn a_missing_or_unreadable_input_is_unknown() {
        assert_eq!(from_parts(None, Some((5, 1))), None, "status unreadable");
        assert_eq!(from_parts(Some("Name:\tx\n"), Some((5, 1))), None, "no NSpid line");
        assert_eq!(from_parts(Some("NSpid:\t\n"), Some((5, 1))), None, "empty NSpid");
        assert_eq!(from_parts(Some(ONE), None), None, "namespace link unreadable");
    }

    #[test]
    fn only_the_init_inode_is_initial() {
        assert!(PidView { dev: 5, ino: PROC_PID_INIT_INO }.is_initial());
        assert!(!PidView { dev: 5, ino: 4026533720 }.is_initial());
        assert_eq!(PROC_PID_INIT_INO, 4026531836);
    }

    #[test]
    fn a_view_formats_as_dev_colon_ino() {
        assert_eq!(PidView { dev: 5, ino: 7 }.to_string(), "5:7");
    }
}
```

Add `mod pidview;` to `src/main.rs` after `mod periodic;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one pidview`
Expected: compile errors: `cannot find function from_parts`, `cannot find struct PidView`.

- [ ] **Step 3: Implement the view reader**

Insert above the tests module in `src/pidview.rs`:

```rust
use std::sync::OnceLock;

/// nsfs inode of the initial PID namespace, a fixed kernel constant (`PROC_PID_INIT_INO`
/// in include/linux/proc_ns.h). A kernel that changed it would make every view read as
/// non-initial, which only withholds views (spec §2), never misjudges a claim.
pub const PROC_PID_INIT_INO: u64 = 0xEFFF_FFFC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidView {
    pub dev: u64,
    pub ino: u64,
}

impl PidView {
    pub fn is_initial(&self) -> bool {
        self.ino == PROC_PID_INIT_INO
    }
}

impl std::fmt::Display for PidView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.dev, self.ino)
    }
}

/// `status` is `/proc/self/status`; `ns` is the `(st_dev, st_ino)` of `/proc/self/ns/pid`,
/// following the link. `NSpid` lists the caller's pid from the procfs's namespace down to
/// its own, so exactly one entry means the procfs numbers in the caller's namespace. Any
/// other count, or any failed read, is unknown (spec §1).
pub fn from_parts(status: Option<&str>, ns: Option<(u64, u64)>) -> Option<PidView> {
    let nspid = status?.lines().find_map(|line| line.strip_prefix("NSpid:"))?;
    if nspid.split_whitespace().count() != 1 {
        return None;
    }
    let (dev, ino) = ns?;
    Some(PidView { dev, ino })
}

/// The caller's view, read once per process: a process does not change PID namespace.
pub fn current() -> Option<PidView> {
    static VIEW: OnceLock<Option<PidView>> = OnceLock::new();
    *VIEW.get_or_init(|| {
        use std::os::unix::fs::MetadataExt;
        let status = std::fs::read_to_string("/proc/self/status").ok();
        let ns = std::fs::metadata("/proc/self/ns/pid")
            .ok()
            .map(|meta| (meta.dev(), meta.ino()));
        from_parts(status.as_deref(), ns)
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-one pidview`
Expected: 5 passed. A dead-code warning on `current` is expected until Step 9 uses it.

- [ ] **Step 5: Write the failing evidence tests**

In `src/claims.rs`, add `pid_view: None,` to every `Claim { … }` literal in the tests module (`sample()`, `claim_of`, and the others; `grep -n 'Claim {$' src/claims.rs` lists them). Add the same line to the three literals in `src/output.rs`. Then append to the tests module:

```rust
    const INIT: crate::pidview::PidView = crate::pidview::PidView {
        dev: 5,
        ino: crate::pidview::PROC_PID_INIT_INO,
    };
    const CHILD: crate::pidview::PidView = crate::pidview::PidView {
        dev: 5,
        ino: 4026533720,
    };

    fn ident(pid: Option<u32>, source: Option<PidSource>) -> Identity {
        Identity {
            session: "s1".into(),
            tagged: "s1".into(),
            pid,
            proof: None,
            root_session: None,
            pid_source: source,
        }
    }

    fn boot() -> Option<String> {
        Some("boot-a".into())
    }

    #[test]
    fn an_exported_pid_found_in_the_initial_namespace_records_the_view() {
        let got = evidence(&ident(Some(42), Some(PidSource::Exported)), None, Some(&INIT), boot(), found(999));
        assert_eq!(got.pid_view, Some(INIT.to_string()));
        assert_eq!((got.pid, got.pid_start), (Some(42), Some(999)));
    }

    #[test]
    fn an_exported_pid_records_no_view_outside_the_initial_namespace() {
        // Spec round 2: a colliding ancestor is found, and still establishes nothing.
        let got = evidence(&ident(Some(2), Some(PidSource::Exported)), None, Some(&CHILD), boot(), found(999));
        assert_eq!(got.pid_view, None);
        assert_eq!((got.pid, got.pid_start), (Some(2), Some(999)));
    }

    #[test]
    fn an_exported_pid_not_found_or_in_an_unknown_view_records_no_view() {
        let missing = evidence(&ident(Some(42), Some(PidSource::Exported)), None, Some(&INIT), boot(), |_| ProcStat::NotFound);
        assert_eq!((missing.pid_view, missing.pid_start), (None, None));
        let unknown = evidence(&ident(Some(42), Some(PidSource::Exported)), None, None, boot(), found(999));
        assert_eq!(unknown.pid_view, None);
    }

    #[test]
    fn a_found_session_id_records_the_view_in_any_known_view() {
        let got = evidence(&ident(Some(7), Some(PidSource::UnixSession)), None, Some(&CHILD), boot(), found(999));
        assert_eq!(got.pid_view, Some(CHILD.to_string()));
    }

    #[test]
    fn a_zero_session_id_establishes_nothing() {
        let got = evidence(&ident(Some(0), Some(PidSource::UnixSession)), None, Some(&INIT), boot(), found(999));
        assert_eq!(got.pid_view, None);
    }

    #[test]
    fn a_relay_match_records_the_view_from_its_proof() {
        let me = Identity {
            proof: Some(Proof { pid_start: 555, boot_id: "boot-r".into(), host: "h".into() }),
            ..ident(Some(42), Some(PidSource::Relay))
        };
        let got = evidence(&me, None, Some(&CHILD), boot(), |_| ProcStat::NotFound);
        assert_eq!(got.pid_view, Some(CHILD.to_string()));
        assert_eq!((got.pid_start, got.boot_id.as_deref()), (Some(555), Some("boot-r")));
    }

    fn recorded() -> Claim {
        Claim { pid_view: Some(INIT.to_string()), ..sample() }
    }

    #[test]
    fn an_established_heartbeat_replaces_the_evidence() {
        let got = evidence(&ident(Some(43), Some(PidSource::Exported)), Some(&recorded()), Some(&INIT), boot(), found(1000));
        assert_eq!((got.pid, got.pid_start), (Some(43), Some(1000)));
        assert_eq!(got.pid_view, Some(INIT.to_string()));
    }

    #[test]
    fn an_unestablished_heartbeat_of_the_same_pid_keeps_the_evidence() {
        // The sandbox heartbeat: same exported pid, child view, so nothing is established.
        let own = recorded();
        let got = evidence(&ident(own.pid, Some(PidSource::Exported)), Some(&own), Some(&CHILD), boot(), |_| ProcStat::NotFound);
        assert_eq!(got, Evidence::of(&own));
    }

    #[test]
    fn an_unestablished_heartbeat_of_a_new_pid_records_it_without_a_view() {
        let got = evidence(&ident(Some(77), Some(PidSource::Exported)), Some(&recorded()), Some(&CHILD), boot(), found(31));
        assert_eq!((got.pid, got.pid_start, got.pid_view), (Some(77), Some(31), None));
    }

    #[test]
    fn a_heartbeat_without_a_pid_keeps_the_recorded_evidence() {
        let own = recorded();
        let got = evidence(&ident(None, None), Some(&own), Some(&INIT), boot(), found(999));
        assert_eq!(got, Evidence::of(&own));
    }

    #[test]
    fn a_proof_continuation_keeps_the_claims_evidence() {
        let own = recorded();
        let got = evidence(&continuation_identity(&own), Some(&own), Some(&INIT), boot(), found(999));
        assert_eq!(got, Evidence::of(&own));
    }
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `just test-one claims::tests`
Expected: compile errors: `no field pid_source`, `cannot find type PidSource`, `cannot find function evidence`.

- [ ] **Step 7: Add `PidSource`, `Identity.pid_source`, `Claim.pid_view` and `evidence`**

In `src/claims.rs`, add to `Identity` after `root_session`:

```rust
    /// How `pid` was obtained, which decides whether a claim write can establish its PID
    /// view (claim PID-view design §2). `None` when there is no pid, or when the pid is
    /// carried from a claim rather than supplied by this write.
    pub pid_source: Option<PidSource>,
```

Above `Identity`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidSource {
    /// Matched through the caller's own `/proc` ancestry with the start time.
    Relay,
    /// `/proc/self/stat`'s session id, translated by the kernel into the procfs's namespace.
    UnixSession,
    /// `TASKS_SESSION_PID` or `CLAUDE_PID`: a number whose namespace this process cannot see.
    Exported,
}
```

In `identity_from`, set `pid_source` on each level:
- `TASKS_SESSION`: `pid_source: pid_of("TASKS_SESSION_PID").map(|_| PidSource::Exported)`. Bind `let pid = pid_of("TASKS_SESSION_PID");` first and use `pid` for both fields.
- Relay: `pid_source: Some(PidSource::Relay)`.
- `CLAUDE_CODE_SESSION_ID`: bind `let pid = pid_of("CLAUDE_PID");` and set `pid_source: pid.map(|_| PidSource::Exported)`.
- Codex: `pid_source: None`.
- `sid:`: `pid_source: Some(PidSource::UnixSession)`.

In `continuation_identity`, add `pid_source: None`.

In `Claim`, after `boot_id`:

```rust
    /// The PID view `pid` was established in, `"<dev>:<ino>"` (claim PID-view design §2).
    /// Absent when no write could establish it; such a pid is then never judged (§3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_view: Option<String>,
```

After `boot_id()`, add:

```rust
/// The process evidence a claim write records (claim PID-view design §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub pid: Option<u32>,
    pub pid_start: Option<u64>,
    pub boot_id: Option<String>,
    pub pid_view: Option<String>,
}

impl Evidence {
    pub fn of(claim: &Claim) -> Evidence {
        Evidence {
            pid: claim.pid,
            pid_start: claim.pid_start,
            boot_id: claim.boot_id.clone(),
            pid_view: claim.pid_view.clone(),
        }
    }
}

/// What a claim write records. `own` is the caller's existing claim when this write
/// continues it (a heartbeat), and `None` for a fresh acquisition or a takeover. A view is
/// recorded only when this write establishes the pid in a known view. Exported pids
/// establish one only in the initial PID namespace, because no numeric match can prove an
/// inherited number's namespace (spec round 2). A heartbeat that establishes nothing keeps
/// the recorded evidence unless it reports a different pid (§2's table).
pub fn evidence(
    me: &Identity,
    own: Option<&Claim>,
    view: Option<&crate::pidview::PidView>,
    boot_id: Option<String>,
    stat: impl Fn(u32) -> ProcStat,
) -> Evidence {
    let start = match &me.proof {
        Some(proof) => Some(proof.pid_start),
        None => me.pid.and_then(|pid| match stat(pid) {
            ProcStat::Found { starttime, .. } => Some(starttime),
            _ => None,
        }),
    };
    let established = match (me.pid, me.pid_source) {
        (Some(_), Some(PidSource::Relay)) => true,
        (Some(pid), Some(PidSource::UnixSession)) => pid != 0 && start.is_some(),
        (Some(_), Some(PidSource::Exported)) => {
            view.is_some_and(|view| view.is_initial()) && start.is_some()
        }
        _ => false,
    };
    let fresh = Evidence {
        pid: me.pid,
        pid_start: start,
        boot_id: match &me.proof {
            Some(proof) => Some(proof.boot_id.clone()),
            None => boot_id,
        },
        pid_view: view.filter(|_| established).map(|view| view.to_string()),
    };
    match own {
        Some(own) if fresh.pid_view.is_none() && (me.pid.is_none() || me.pid == own.pid) => {
            Evidence::of(own)
        }
        _ => fresh,
    }
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one claims::tests`
Expected: all pass, including the 11 new tests.

- [ ] **Step 9: Write the failing integration tests**

In `tests/cli.rs`, near `write_claim`, add the helper:

```rust
/// This test process's PID view, read as `src/pidview.rs` reads it: the namespace its
/// `/proc` numbers pids in, and `None` unless `NSpid` has exactly one entry.
fn test_pid_view() -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let nspid = status.lines().find_map(|line| line.strip_prefix("NSpid:"))?;
    if nspid.split_whitespace().count() != 1 {
        return None;
    }
    let ns = std::fs::metadata("/proc/self/ns/pid").ok()?;
    Some(format!("{}:{}", ns.dev(), ns.ino()))
}

fn stored_claim(env: &TestEnv, prefix: &str, id: &str) -> toml::Value {
    let store: toml::Value =
        toml::from_str(&std::fs::read_to_string(env.claim_store(prefix)).unwrap()).unwrap();
    store["claims"][id].clone()
}
```

And the tests:

```rust
#[test]
fn an_exported_pid_records_its_pid_view_only_in_the_initial_namespace() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.cmd(&sci)
        .env("TASKS_SESSION", "viewer")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", &id])
        .assert()
        .success();
    let claim = stored_claim(&env, "sci", &id);
    // The test process is the binary's parent, so the binary shares its view.
    let expected = test_pid_view().filter(|view| view.ends_with(":4026531836"));
    assert_eq!(
        claim.get("pid_view").and_then(|v| v.as_str()).map(str::to_string),
        expected,
        "{claim}"
    );
}

#[test]
fn a_heartbeat_with_a_missing_pid_records_it_without_a_view() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    for pid in [std::process::id().to_string(), "4194303".to_string()] {
        env.cmd(&sci)
            .env("TASKS_SESSION", "viewer")
            .env("TASKS_SESSION_PID", pid)
            .args(["start", &id])
            .assert()
            .success();
    }
    let claim = stored_claim(&env, "sci", &id);
    assert_eq!(claim["pid"].as_integer(), Some(4194303), "{claim}");
    assert!(claim.get("pid_view").is_none(), "{claim}");
    assert!(claim.get("pid_start").is_none(), "{claim}");
}
```

- [ ] **Step 10: Run them to verify they fail**

Run: `just test-one --test cli pid_view`
Expected: `an_exported_pid_records_…` FAILS on a host whose test process is in the initial namespace (left `None`, right `Some("…:4026531836")`). `a_heartbeat_with_a_missing_pid_…` may already pass, since today's write drops `pid_start` for a missing pid. It stays as the heartbeat-row-3 guard.

- [ ] **Step 11: Record the evidence in `claim_guard`**

In `src/commands/mod.rs`, inside `claim_guard`, replace the `pid_start`, `boot_id` and `pid` fields of the `ClaimIntent::Acquire(crate::claims::Claim { … })` literal (lines 523-545) with values from `evidence`. Before the literal, after `let started = …;`:

```rust
            let evidence = crate::claims::evidence(
                &me,
                existing.as_ref().filter(|_| mine),
                crate::pidview::current().as_ref(),
                crate::claims::boot_id(),
                crate::claims::proc_stat,
            );
```

In the literal:

```rust
                    pid_start: evidence.pid_start,
                    boot_id: evidence.boot_id,
                    pid_view: evidence.pid_view,
                    // `host` is unchanged: the proof's host, else this host.
                    …
                    pid: evidence.pid,
```

Keep `host`, `session`, `root_session`, `worktree`, `started`, `seen` and `holds` as they are. `me` is used after this point only for `me.session`, so move it in after computing `evidence`, as today.

- [ ] **Step 12: Run the integration tests and the fast suite**

Run: `just test-one --test cli pid_view`, then `just test-fast`
Expected: both new tests pass; the fast suite passes. No liveness behaviour has changed yet, because `liveness_with` does not read `pid_view` until Task 2.

- [ ] **Step 13: Lint and commit**

```bash
just check
git add src/pidview.rs src/main.rs src/claims.rs src/commands/mod.rs src/output.rs tests/cli.rs
git commit -m "feat(claims): record the PID view a claim's pid was established in"
```

---

### Task 2: Judge only from the recording view; protect every other verdict

**Files:**
- Modify: `src/claims.rs`:
  - `Liveness` (lines 676-680);
  - `prune_dead` (620-622);
  - `ClaimSnapshot::live` and `stale` (753-773);
  - `liveness`/`liveness_with` (817-860);
  - the liveness tests (1760-1940).
- Modify: `src/holds.rs:61-110`:
  - pass the view through `load` and `load_from_paths`;
  - the check at line 101.
- Modify: `src/commands/mod.rs`:
  - `takeover_summary` and `describe_claim` (150-185);
  - `refuse_foreign_live_claim` (213-219);
  - `update_holds` (234);
  - `guard_holds` (388);
  - `claim_guard` takeover match (472-497).
- Modify: `src/commands/park.rs:58-66`, `src/commands/retire.rs:75-76`, `src/rename/mod.rs:319-320`, `src/rename/adopt.rs:154-155`
- Modify: `src/commands/list.rs:274,280,302,649,752,828`, `src/lanes.rs:287,367`, `src/commands/show.rs:60`, `src/commands/sample.rs:53`, `src/moving/preflight.rs:38` (`.live(` becomes `.held(`)
- Modify: `src/output.rs:443-473` (`ClaimInfo`) and `:1288` (claims table); `src/present.rs:143-150`
- Test: unit tests in `src/claims.rs` and `src/output.rs`; integration tests in `tests/cli.rs`, plus the two fixtures `write_claim` (line 911) and `write_detailed_claim` (line 10444)

**Interfaces:**
- Consumes: from Task 1, `crate::pidview::{PidView, current}`, `Claim.pid_view`, and the test helper `test_pid_view()` and `stored_claim()` in `tests/cli.rs`.
- Produces:
  - `Liveness::Unverifiable(String)`;
  - `Liveness::is_stale(&self) -> bool`;
  - `liveness_with(claim, now, boot_id, view: Option<&PidView>, stat)`;
  - `ClaimSnapshot::held(&self, id) -> Option<&Claim>`, replacing `live`;
  - `HoldSnapshot::load_from_paths(stores, now, boot_id, view: Option<&PidView>, stat)`;
  - `ClaimInfo.unverified: Option<String>`.

- [ ] **Step 1: Write the failing verdict tests**

In `src/claims.rs` tests:
- Change `sample()` to set `pid_view: Some(INIT.to_string())` (the `INIT` constant comes from Task 1).
- Change every existing `liveness_with(` call to pass `Some(&INIT)` as the new fourth argument, between the boot id and the stat closure.

Then add:

```rust
    #[test]
    fn a_claim_from_another_view_is_unverifiable_however_it_would_read() {
        for stat in [ProcStat::NotFound, ProcStat::Found { state: 'S', starttime: 12345 }] {
            assert_eq!(
                liveness_with(&seen("2026-09-05T00:00:00Z"), at("2026-09-05T00:00:01Z"), Some("boot-a"), Some(&CHILD), |_| stat.clone()),
                Liveness::Unverifiable("recorded in another PID view".into())
            );
        }
    }

    #[test]
    fn an_unverifiable_claim_never_ages_into_stale() {
        assert!(matches!(
            liveness_with(&seen("2026-09-01T00:00:00Z"), at("2026-09-05T00:00:00Z"), Some("boot-a"), Some(&CHILD), |_| ProcStat::Unreadable),
            Liveness::Unverifiable(_)
        ), "a live owner past four hours keeps its claim against a sandboxed caller");
    }

    #[test]
    fn a_legacy_pid_claim_is_unverifiable_even_when_its_pid_is_gone() {
        let legacy = Claim { pid_view: None, ..seen("2026-09-05T00:00:00Z") };
        assert_eq!(
            liveness_with(&legacy, at("2026-09-05T00:00:01Z"), Some("boot-a"), Some(&INIT), |_| ProcStat::NotFound),
            Liveness::Unverifiable("PID view not established when recorded".into())
        );
    }

    #[test]
    fn an_unknown_current_view_is_unverifiable() {
        assert_eq!(
            liveness_with(&seen("2026-09-05T00:00:00Z"), at("2026-09-05T00:00:01Z"), Some("boot-a"), None, |_| ProcStat::NotFound),
            Liveness::Unverifiable("this command cannot establish its PID view".into())
        );
    }

    #[test]
    fn a_malformed_recorded_view_is_unverifiable() {
        let odd = Claim { pid_view: Some("garbage".into()), ..seen("2026-09-05T00:00:00Z") };
        assert!(matches!(
            liveness_with(&odd, at("2026-09-05T00:00:01Z"), Some("boot-a"), Some(&INIT), |_| ProcStat::NotFound),
            Liveness::Unverifiable(_)
        ));
    }

    #[test]
    fn a_reboot_is_stale_whatever_the_view() {
        for view in [Some(&INIT), Some(&CHILD), None] {
            let legacy = Claim { pid_view: None, ..seen("2026-09-05T00:00:00Z") };
            assert!(matches!(
                liveness_with(&legacy, at("2026-09-05T00:00:01Z"), Some("boot-b"), view, found(999)),
                Liveness::Stale(_)
            ));
        }
    }

    #[test]
    fn a_pidless_claim_takes_the_ttl_whatever_the_view() {
        let pidless = Claim { pid: None, pid_view: None, ..seen("2026-09-05T00:00:00Z") };
        for view in [Some(&INIT), Some(&CHILD), None] {
            assert!(matches!(
                liveness_with(&pidless, at("2026-09-05T05:00:00Z"), Some("boot-a"), view, found(999)),
                Liveness::Stale(_)
            ));
        }
    }

    #[test]
    fn only_stale_is_stale() {
        assert!(Liveness::Stale("x".into()).is_stale());
        assert!(!Liveness::Live.is_stale());
        assert!(!Liveness::Unverifiable("x".into()).is_stale());
    }

    #[test]
    fn prune_keeps_an_unverifiable_claim() {
        let home = tempfile::tempdir().unwrap();
        let mut store = ClaimStore::load_from(&home.path().join("sci.toml")).unwrap();
        let id = TaskId::parse("sci-000001").unwrap();
        // This boot, so the boot rule cannot make it stale; a view no namespace has.
        store.insert(&id, Claim {
            pid_view: Some("0:1".into()),
            pid: Some(4_194_303),
            boot_id: boot_id(),
            ..sample()
        });
        store.prune_dead();
        assert!(store.get(&id).is_some());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one claims::tests`
Expected: compile errors: `liveness_with` takes 4 arguments, no variant `Unverifiable`, no method `is_stale`.

- [ ] **Step 3: Implement the verdict**

In `src/claims.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Liveness {
    Live,
    Stale(String),
    /// Judged through a PID view other than the one that recorded the pid, or where
    /// either is unknown (claim PID-view design §3). Protected like `Live`: only `Stale`
    /// permits pruning, takeover without `--force`, or releasing holds (§4).
    Unverifiable(String),
}

impl Liveness {
    pub fn is_stale(&self) -> bool {
        matches!(self, Liveness::Stale(_))
    }
}
```

`liveness`:

```rust
pub fn liveness(claim: &Claim) -> Liveness {
    liveness_with(
        claim,
        time::OffsetDateTime::now_utc(),
        boot_id().as_deref(),
        crate::pidview::current().as_ref(),
        proc_stat,
    )
}
```

`liveness_with`: add the parameter `view: Option<&crate::pidview::PidView>` after `boot_id`. Replace the body from `let Some(pid) = claim.pid else { return ttl() };` through the boot match with:

```rust
    let Some(pid) = claim.pid else { return ttl() };
    // A reboot ends every view, so it needs no view check (§3 rule 1).
    if let (Some(current), Some(recorded)) = (boot_id, &claim.boot_id)
        && current != recorded
    {
        return Liveness::Stale("recorded on an earlier boot".into());
    }
    match (&claim.pid_view, view) {
        (None, _) => {
            return Liveness::Unverifiable("PID view not established when recorded".into());
        }
        (Some(_), None) => {
            return Liveness::Unverifiable("this command cannot establish its PID view".into());
        }
        (Some(recorded), Some(current)) if *recorded != current.to_string() => {
            return Liveness::Unverifiable("recorded in another PID view".into());
        }
        _ => {}
    }
    if boot_id.is_none() || claim.boot_id.is_none() {
        return ttl();
    }
```

The `match stat(pid)` that follows stays as it is.

`prune_dead`:

```rust
    pub fn prune_dead(&mut self) {
        self.prune_with(|claim| !liveness(claim).is_stale());
    }
```

`ClaimSnapshot`: rename `live` to `held`, and protect `Unverifiable`:

```rust
    /// The claim on `id` unless it is stale: `Live` and `Unverifiable` both hold (§4).
    pub fn held(&self, id: &TaskId) -> Option<&Claim> {
        match self.get(id) {
            Some((claim, liveness)) if !liveness.is_stale() => Some(claim),
            _ => None,
        }
    }
```

In `stale()`, make the non-stale arm `_ => None`.

- [ ] **Step 4: Run the unit tests**

Run: `just test-one claims::tests`
Expected: claims tests pass. The crate still fails to compile elsewhere (holds, the commands, `.live(` callers); Steps 5-7 fix those.

- [ ] **Step 5: Update the hold snapshot**

In `src/holds.rs` `load`, pass `crate::pidview::current().as_ref()` after the boot id. In `load_from_paths`, add the parameter `view: Option<&crate::pidview::PidView>` after `boot_id`. The check at line 101 becomes:

```rust
                if claim.holds.is_empty()
                    || crate::claims::liveness_with(claim, now, boot_id, view, &stat).is_stale()
                {
```

Update the comment above it to read `Held unless stale (§4.4; claim PID-view design §4): …`.

In the `src/holds.rs` tests module, add the view the fixtures record:

```rust
    const HERE: crate::pidview::PidView = crate::pidview::PidView {
        dev: 5,
        ino: crate::pidview::PROC_PID_INIT_INO,
    };
```

In `entry()`, make the pid line record it, so every existing test keeps its verdict:

```rust
        let pid = pid
            .map(|pid| {
                format!("pid = {pid}\npid_start = 7\nboot_id = \"boot\"\npid_view = \"5:4026531836\"\n")
            })
            .unwrap_or_default();
```

Pass `Some(&HERE)` as the new fourth argument of every `HoldSnapshot::load_from_paths(` call in the tests; `grep -n 'load_from_paths(' src/holds.rs` lists them. Then add:

```rust
    #[test]
    fn an_unverifiable_claim_keeps_its_holds() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &["quiet"]),
        )];
        let elsewhere = crate::pidview::PidView { dev: 5, ino: 4026533720 };
        // `gone` would make the claim stale in its own view; from another view it holds.
        let (snapshot, warnings) = HoldSnapshot::load_from_paths(
            stores,
            at("2026-10-03T11:00:00Z"),
            Some("boot"),
            Some(&elsewhere),
            gone,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(snapshot.holder("quiet", &id("sci-000002"), &nobody()).is_some());
    }
```

- [ ] **Step 6: Update the commands**

`src/commands/mod.rs`:

```rust
    pub fn takeover_summary(claim: &crate::claims::Claim, live: &Liveness) -> String {
        let how = match live {
            Liveness::Live => "live, forced",
            Liveness::Unverifiable(_) => "unverifiable, forced",
            Liveness::Stale(_) => "stale",
        };
```

In `describe_claim`, add the arm `Liveness::Unverifiable(why) => format!("unverifiable: {why}"),`.

`refuse_foreign_live_claim`: `if !live.is_stale() {` replaces `if live == Liveness::Live {`.

`update_holds`: `if crate::claims::liveness(&existing).is_stale() { return Ok(()); }`.

The `guard_holds` condition at line 388: `&& !crate::claims::liveness(&existing).is_stale()`.

`claim_guard`'s takeover match:

```rust
        if let Some(existing) = &existing {
            let live = crate::claims::liveness(existing);
            match (live.is_stale(), mine) {
                (false, false) if !(force && to == Status::Doing) => {
                    return Err(Error::Claimed(
                        id.to_string(),
                        Ctx::describe_claim(existing, &live),
                    ));
                }
                (false, false) => {
                    let what = match live {
                        Liveness::Unverifiable(_) => "an unverifiable",
                        _ => "a live",
                    };
                    warning = Some(format!(
                        "took over {what} claim held by {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                    self.takeover = Some(Ctx::takeover_summary(existing, &live));
                }
                (true, false) => {
                    warning = Some(format!("took over {}", Ctx::describe_claim(existing, &live)));
                    self.takeover = Some(Ctx::takeover_summary(existing, &live));
                }
                _ => {}
            }
        }
```

`src/commands/park.rs`: replace the `Liveness::Live =>` arm with `Liveness::Live | Liveness::Unverifiable(_) =>`.

`src/commands/retire.rs`, `src/rename/mod.rs`, `src/rename/adopt.rs`: replace `matches!(live, Liveness::Live)` and `live == Liveness::Live` with `!live.is_stale()`.

`src/commands/list.rs:649`: `if liveness.is_stale() { continue; }`.

Rename every `claims.live(` / `.live(&` call on a `ClaimSnapshot` to `.held(` in `list.rs`, `lanes.rs`, `show.rs`, `sample.rs` and `moving/preflight.rs`. `grep -rn '\.live(' src` must then list nothing.

- [ ] **Step 7: Update the JSON and the pretty output**

`src/output.rs` `ClaimInfo`: add after `live`:

```rust
    /// Why a held claim could not be judged from here (claim PID-view design §5). `live`
    /// stays `true` for it, so consumers that read `false` as "free" stay safe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unverified: Option<String>,
```

In `ClaimInfo::of`:

```rust
            live: !live.is_stale(),
            unverified: match live {
                crate::claims::Liveness::Unverifiable(why) => Some(why.clone()),
                _ => None,
            },
```

Line 1288 (the claims table): replace the state cell with

```rust
                        match (&row.claim.unverified, row.claim.live) {
                            (Some(_), _) => "unverifiable",
                            (None, true) => "live",
                            (None, false) => "stale",
                        }
                        .into(),
```

`src/present.rs::claim`:

```rust
/// `<owner> [<session>]`, with ` stale` or ` unverifiable` inside the brackets when the
/// claim is not plainly live.
pub fn claim(claim: &ClaimInfo) -> Span {
    let state = match (&claim.unverified, claim.live) {
        (Some(_), _) => " unverifiable",
        (None, true) => "",
        (None, false) => " stale",
    };
    Span::styled(
        format!("{} [{}{state}]", claim.owner, claim.session),
        Style::Chrome,
    )
}
```

Add to the `src/output.rs` tests, beside the existing `ClaimInfo` serialization test:

```rust
    #[test]
    fn an_unverifiable_claim_is_held_with_its_reason() {
        let claim = crate::claims::Claim {
            owner: "o".into(), session: "s".into(), pid: Some(1), pid_start: None,
            boot_id: None, pid_view: None, host: "h".into(), worktree: "/w".into(),
            started: "2026-10-03T00:00:00Z".into(), seen: "2026-10-03T00:00:00Z".into(),
            holds: Vec::new(), root_session: None,
        };
        let why = crate::claims::Liveness::Unverifiable("recorded in another PID view".into());
        let json = serde_json::to_value(ClaimInfo::of(&claim, &why)).unwrap();
        assert_eq!(json["live"], true);
        assert_eq!(json["unverified"], "recorded in another PID view");
        let live = serde_json::to_value(ClaimInfo::of(&claim, &crate::claims::Liveness::Live)).unwrap();
        assert!(live.get("unverified").is_none(), "{live}");
    }
```

- [ ] **Step 8: Run the unit tests**

Run: `just test-one claims:: holds:: output::`
Expected: all pass, and the crate compiles.

- [ ] **Step 9: Update the integration fixtures**

In `tests/cli.rs`, both fixtures record the test process's view, so their claims are judged as before:

- `write_claim` (line 911): after the `seen = …` line of the format string, append `pid_view` when `test_pid_view()` is `Some`:

```rust
    let view = test_pid_view()
        .map(|view| format!("pid_view = \"{view}\"\n"))
        .unwrap_or_default();
```

  Then add `{view}` at the end of the pushed text.

- `write_detailed_claim` (line 10444): the same, for both its live and its gone-pid variants. A gone pid in the recording view must stay `Stale`, which the leak tests rely on.

Add a fixture writer with an explicit view:

```rust
/// A claim held by this test process but recorded in `view`.
fn write_claim_in_view(env: &TestEnv, prefix: &str, id: &str, session: &str, view: &str) {
    write_claim(env, prefix, id, session, true);
    let path = env.claim_store(prefix);
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    store["claims"][id]
        .as_table_mut()
        .unwrap()
        .insert("pid_view".into(), toml::Value::String(view.into()));
    std::fs::write(&path, toml::to_string(&store).unwrap()).unwrap();
}
```

- [ ] **Step 10: Write the failing integration tests**

```rust
#[test]
fn a_claim_from_another_pid_view_is_held_not_pruned_or_taken() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let held = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    let other = id_of(env.json(&sci, &["add", "Other", "-p", "2"]));
    write_claim_in_view(&env, "sci", &held, "sandboxed", "0:1");

    // An unrelated claim write prunes nothing.
    env.cmd(&sci).env("TASKS_SESSION", "bystander").args(["start", &other]).assert().success();
    assert!(stored_claim(&env, "sci", &held).get("session").is_some());

    // Another session cannot take it without --force.
    let out = env.cmd(&sci).env("TASKS_SESSION", "bystander").args(["start", &held]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"claimed\"")
        || String::from_utf8_lossy(&out.stderr).contains("claimed"));

    // tasks claims reports it held, with the reason.
    let claims = env.json(&sci, &["claims"]);
    let row = claims["claims"].as_array().unwrap().iter().find(|row| row["id"] == held).unwrap().clone();
    assert_eq!(row["live"], true, "{row}");
    assert_eq!(row["unverified"], "recorded in another PID view", "{row}");

    // --force takes it over, and the note says how.
    env.cmd(&sci)
        .env("TASKS_SESSION", "bystander")
        .args(["start", "--force", "--reason", "sandbox gone", &held])
        .assert()
        .success();
    let shown = env.json(&sci, &["show", &held]);
    assert!(shown.to_string().contains("unverifiable, forced"), "{shown}");
}

#[test]
fn pretty_claims_name_an_unverifiable_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    write_claim_in_view(&env, "sci", &id, "sandboxed", "0:1");
    let out = env.cmd(&sci).env("TASKS_FORMAT", "pretty").args(["claims"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("unverifiable"), "{text}");
    assert!(!text.contains("stale"), "{text}");
}
```

Run: `just test-one --test cli unverifiable pid_view`. Before Steps 1-9 these tests failed. Now expect both to pass. If either fails, fix the code, not the test.

- [ ] **Step 11: Run the fast suite and resolve the expected fixture fallout**

Run: `just test-fast`
Expected: passes, apart from tests in one category. A test that records a claim through an exported pid that does not exist (for example `TASKS_SESSION_PID=999999`) and then expects that claim to read stale now sees it held. Spec §2 makes that claim record no view, so it is `Unverifiable`. Resolve each such test by its intent:
- If it needs a stale claim, write it with `write_claim(.., false)`, the boot-mismatch fixture.
- If it is about something else (identity errors, as in the test near line 21009), keep the setup and adjust only the liveness expectation, to `live: true` with `unverified`.

List every test changed this way in the commit body. Any other failure is a defect in Steps 1-9: fix the code.

- [ ] **Step 12: Lint and commit**

```bash
just check
git add src tests/cli.rs
git commit -m "fix(claims): judge pid liveness only from the PID view that recorded it

Claims judged through another or unknown PID view read Unverifiable, which
nothing prunes, takes over without --force, or releases holds on."
```

---

### Task 3: Document the views and run the namespace acceptance

**Files:**
- Modify: `README.md`:
  - the PID-namespace paragraph (around line 302, "Relay process matching also requires…");
  - the Upgrading paragraph (around line 309);
  - the `tasks claims` line (around line 380).
- Modify: `skills/tasks/SKILL.md`, the claim paragraph in step 3 of the session protocol (around line 92)
- Create (attachment, not repository code): `pidview-acceptance.sh` and `pidview-acceptance-evidence.json`, attached to tasks-860db2 with `tasks attach`

**Interfaces:**
- Consumes: the built worktree binary (`target/debug/tasks` after `cargo build`, or the path `just` builds), with Tasks 1 and 2 merged into it.
- Produces: documentation and attached evidence; no code.

- [ ] **Step 1: Update the README**

Replace the sentence "Claim liveness judged across PID views is tracked as tasks-860db2." with:

```markdown
A claim records the PID view its pid was established in: the PID namespace that the
writer's `/proc` numbers pids in. Only a command in that same view judges the pid.
Elsewhere, and wherever either view is unknown, the claim is `unverifiable`: it is held,
and nothing prunes it, takes it over without `--force`, or releases its holds. A
reboot, its owner's `park` or `done`, or `start --force` clears it. An exported pid
(`TASKS_SESSION_PID`, `CLAUDE_PID`) establishes a view only in the initial PID
namespace, because an inherited number's namespace cannot be proven. Inside a container
or sandbox, such claims stay held after their session ends. `TASKS_SESSION_PID` must
name a process in the namespace of the command that sets it.
```

Add to the Upgrading paragraph:

```markdown
Claims judged by PID view need every `tasks` that reads or writes a host's claim stores
at the same version or later: each `tasks` on `PATH` in each harness's environment,
including one a sandbox can execute. An older binary judges every pid through its own
`/proc`. It can prune a claim that another view holds, it drops the recorded view when
it saves, and it reports that claim as stale. Check `tasks --version` for each.
```

Extend the `tasks claims` line's comment to read:

```
    tasks claims                     # every claim in every project's store; live = held (unverified: why it could not be judged here); what a Stop hook reads
```

- [ ] **Step 2: Update the skill**

After the sentence ending "`tasks start --force <id>` takes it over and records that in the task's notes." in `skills/tasks/SKILL.md`, add:

```markdown
   A claim whose pid this command cannot judge (recorded in another PID view, such as
   a sandbox's, or before views were recorded) is held and shows as `unverifiable`.
   Its own view, its owner, a reboot or `start --force` releases it.
```

- [ ] **Step 3: Write and run the acceptance script**

Write the acceptance script at `$TMPDIR/pidview-acceptance.sh`, outside the repository. Model it on tasks-fe6c59's `pidns-probe.sh`; `tasks show tasks-fe6c59` prints its path. The script must:
- use a private `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `RELAY_STATE_DIR` under a `mktemp -d` directory, and delete them on exit (`trap … EXIT INT TERM`);
- run the worktree binary by explicit path;
- use only unprivileged `unshare --user --map-current-user --pid --fork [--mount-proc]`, with no `codex sandbox`, no model call and no live desktop.

It must run these cases. Each prints `case`, `expected`, `observed` and `verdict` as one JSON object per line:

1. **prune-from-sandbox.** On the host, `TASKS_SESSION=A TASKS_SESSION_PID=$$ tasks start T1`; `pid_view` must be recorded. In a private-`/proc` child, `TASKS_SESSION=B tasks start T2`. Back on the host, T1's claim must still exist, and `TASKS_SESSION=C tasks start T1` must fail with `claimed`.
2. **ns-explicit-pid1.** In a private-`/proc` child, `TASKS_SESSION=N TASKS_SESSION_PID=1 tasks start T3`, with the child kept alive by `sleep` in the background. The claim must record no `pid_view`. On the host, `tasks claims` shows it `live: true` with `unverified`, and `TASKS_SESSION=C tasks start T3` fails with `claimed`.
3. **procfs unmount and reuse.** Namespace A: a private-`/proc` child whose pid 1 runs `sleep 120`. In A, a claim on T4 through the Unix session fallback (no `TASKS_SESSION`) records A's view. Record `stat -c %d /proc` and A's namespace inode. Unmount A's `/proc` (`umount /proc` inside A's mount namespace). Start namespace B with `--mount-proc` and record its `/proc` device, and whether it equals A's. From B, `TASKS_SESSION=B tasks start T5`. T4's claim must still exist while A's pid 1 lives.
4. **exported host pid.** In a private-`/proc` child, `TASKS_SESSION=E TASKS_SESSION_PID=<a live host pid, the script's own $$>` with `tasks start T6`. The claim must record no view. A second child write (`TASKS_SESSION=F tasks start T7`) must leave it in place.
5. **colliding ancestor.** Outer namespace O: `unshare … --mount-proc sh -c 'sleep 300 & …'`, so that the intended owner is pid 2 in O. O launches inner namespace I with `--mount-proc`. In I, arrange an ancestor at pid 2 that exits later (`sh -c 'sh -c "…" ; sleep 1'`, with the inner pid noted from `NSpid`). Run with `TASKS_SESSION=O2 TASKS_SESSION_PID=2` inherited.
   - I's `tasks start T8` records no view.
   - After I's pid 2 exits, `TASKS_SESSION=X tasks start T9` in I leaves T8's claim in place.
   - `TASKS_SESSION=X tasks start T8` in I fails with `claimed`.
   - An owner claim recorded by O on T10 (`TASKS_SESSION=O2 TASKS_SESSION_PID=2 tasks start T10` from O, which records no view because O is not initial), heartbeated from I with the same session and pid, keeps its recorded `pid` and `pid_start`.
   - `NSpid` has one entry inside I.
6. **heartbeat from a sandbox.** On the host, `TASKS_SESSION=H TASKS_SESSION_PID=$$ tasks start T11`, recording the host view. From a private-`/proc` child, `TASKS_SESSION=H TASKS_SESSION_PID=$$` (the same number, absent in the child) runs `tasks start T11`. Afterwards the store still shows the host `pid_view` and the original `pid_start`.

Run it once and redirect the output to `$TMPDIR/pidview-acceptance-evidence.json`. Every verdict must be `pass`. A case the environment cannot run, such as a refused `umount`, is reported as `unavailable` with the error. Record it in a task note; do not weaken the case to pass.

- [ ] **Step 4: Attach the evidence**

```bash
tasks attach tasks-860db2 "$TMPDIR/pidview-acceptance.sh" --caption "Disposable acceptance script for the PID-view design: unshare cases, private config/state, deleted after"
tasks attach tasks-860db2 "$TMPDIR/pidview-acceptance-evidence.json" --caption "Acceptance results: six unshare cases with verdicts"
tasks note tasks-860db2 "acceptance: <n> pass, <m> unavailable (<which, why>)"
```

Fill in `<n>`, `<m>` and the reasons from the run.

- [ ] **Step 5: Gate and commit**

```bash
just check
just test-fast
git add README.md skills/tasks/SKILL.md tasks/
git commit -m "docs: claims judged by PID view, coordinated upgrade, unverifiable claims"
```

- [ ] **Step 6: Reinstall the CLI**

AGENTS.md requires reinstalling after CLI changes so the tracker in use is the code under test. This replaces the host's `tasks` binary, a host-wide action. Do it only after the final whole-branch review passes and the branch merges, together with every other `tasks` the host runs (spec §7). Until then the worktree binary runs by explicit path.
