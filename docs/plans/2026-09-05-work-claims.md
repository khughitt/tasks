# Work Claims Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `doing` claims cross-worktree visibility, agent identity, and liveness, by moving the claim out of git into a per-prefix state store guarded by a per-project mutation lock.

**Architecture:** A new `src/claims.rs` owns the claim record, its TOML store at `$XDG_STATE_HOME/tasks/claims/<prefix>.toml`, a `/proc`-based liveness test, and a `<prefix>.lock` mutation lock. The store is keyed by *project prefix*, which a worktree and its main checkout share while their roots differ — that is what makes claims visible across worktrees with no worktree enumeration.

`transition()` in `commands/mod.rs` performs the **guard only** and records an intent; **all claim persistence happens inside `save()`, after every validation has passed**, with the previous claim captured and restored if the task-file write fails. Keeping persistence in one validated operation is what stops a rejected edit, a validation failure, or a failed write from mutating the store — those failure paths were spread across three places in an earlier draft of this plan and are the single most important thing to get right here.

**Tech Stack:** Rust 2024, clap 4.6, serde/toml, `std::fs::File::lock`, `/proc`. No new dependencies.

**Spec:** `docs/specs/2026-09-05-work-claims-design.md`

## Global Constraints

- **No new dependencies.** `File::lock` requires rustc ≥ 1.89; the project is on 1.98.1.
- **JSON output is the contract.** Every change here is additive: new optional fields, one new error kind. Never change an existing shape.
- **This crate has no library target.** Unit tests run as `cargo test --bin tasks <filter>`; `cargo test --lib` fails with "no library targets found".
- **`ShowOut.fields` is `#[serde(flatten)]`** (`src/output.rs:78`), so `show` JSON exposes the claim at `v["claim"]`, *not* `v["fields"]["claim"]`. A wrong path silently passes null assertions.
- **Errors serialize as `{"error": {"kind", "detail"}}`** (`src/output.rs:449`). There is no `message` field.
- **Unit tests must never mutate process environment.** The gate runs tests in parallel, so `set_var` in one test corrupts another. Every environment- or path-dependent function in this plan takes an injected lookup (`*_with`, `*_at`) and the tests call those.
- **Fail early with a typed error; no silent fallbacks.** (`AGENTS.md`)
- **Composition > inheritance. Explicit > defensive.**
- Conventional commits. **No AI-attribution trailers or footers.**
- `just check` before every commit (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`). `just gate` adds `cargo test`.
- Rebuild the tracker after CLI changes: `cargo install --path .`
- Claim-store paths resolve `XDG_STATE_HOME`, else `$HOME/.local/state`, mirroring `Registry::path`.
- Two "worktrees" in tests are two temp project dirs sharing one prefix: `env.init("sci")`, then `tasks init --prefix sci --force` in the second dir. They share a claim store because it is keyed by prefix.

---

### Task 1: Claim record, store, and test isolation

**Files:**
- Create: `src/claims.rs`
- Modify: `src/main.rs` (add `mod claims;`)
- Modify: `tests/common/mod.rs:20-29` (env isolation)
- Test: `src/claims.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::error::{Error, Result}`, `crate::model::TaskId`, `crate::repo::atomic_write`.
- Produces: `Claim`, `ClaimStore`, `ClaimStore::{path_with, path_for, load, load_from, save, get, insert, remove, prune_with, iter}`.

**Path resolution is injected.** `path_with` takes the environment lookup so the tests never
call `set_var`: the gate runs tests in parallel, and a mutated `HOME` or `XDG_STATE_HOME`
corrupts whatever else is running.

- [ ] **Step 1: Write the failing test**

In a new `src/claims.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn sample() -> Claim {
        Claim {
            owner: "main".into(),
            session: "s1".into(),
            pid: Some(42),
            pid_start: Some(999),
            boot_id: Some("boot-a".into()),
            host: "h".into(),
            worktree: "/tmp/wt".into(),
            started: "2026-09-05T08:00:00Z".into(),
            seen: "2026-09-05T08:00:00Z".into(),
        }
    }

    /// An injected environment. Returning `OsString` matches `std::env::var_os`, which is
    /// what the production caller passes.
    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| {
            owned
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn store_roundtrips_and_is_keyed_by_prefix() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("claims/sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();

        let mut store = ClaimStore::load_from(&path).unwrap();
        assert!(store.get(&id).is_none(), "an absent store loads empty");
        store.insert(&id, sample());
        store.save().unwrap();

        let reloaded = ClaimStore::load_from(&path).unwrap();
        assert_eq!(reloaded.get(&id).unwrap().session, "s1");
        assert_eq!(reloaded.get(&id).unwrap().pid, Some(42));
    }

    #[test]
    fn remove_reports_whether_anything_was_there() {
        let home = tempfile::tempdir().unwrap();
        let mut store = ClaimStore::load_from(&home.path().join("sci.toml")).unwrap();
        let id = TaskId::parse("sci-000001").unwrap();
        store.insert(&id, sample());
        assert!(store.remove(&id).is_some());
        assert!(store.remove(&id).is_none());
    }

    #[test]
    fn path_prefers_xdg_state_home_then_home() {
        assert_eq!(
            ClaimStore::path_with("sci", env_of(&[("XDG_STATE_HOME", "/xdg")])).unwrap(),
            PathBuf::from("/xdg/tasks/claims/sci.toml")
        );
        assert_eq!(
            ClaimStore::path_with("sci", env_of(&[("HOME", "/home/x")])).unwrap(),
            PathBuf::from("/home/x/.local/state/tasks/claims/sci.toml")
        );
        assert_eq!(
            ClaimStore::path_with("sci", env_of(&[]))
                .unwrap_err()
                .kind(),
            "config",
            "no state directory is a typed error, not a silent fallback"
        );
    }

    #[test]
    fn prune_with_drops_exactly_what_the_predicate_rejects() {
        let home = tempfile::tempdir().unwrap();
        let mut store = ClaimStore::load_from(&home.path().join("sci.toml")).unwrap();
        let keep = TaskId::parse("sci-000001").unwrap();
        let go = TaskId::parse("sci-000002").unwrap();
        store.insert(&keep, sample());
        store.insert(&go, Claim { pid: Some(43), ..sample() });
        store.prune_with(|claim| claim.pid == Some(42));
        assert!(store.get(&keep).is_some());
        assert!(store.get(&go).is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks claims::`
Expected: FAIL — `cannot find type Claim` / `ClaimStore`. (Not `--lib`: this crate has no
library target.)

- [ ] **Step 3: Write minimal implementation**

At the top of `src/claims.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One session's advisory hold on a task. Lives outside git deliberately: a claim is
/// ephemeral machine state, and keeping it on a branch is what made `doing` invisible
/// across worktrees in the first place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub owner: String,
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_id: Option<String>,
    pub host: String,
    pub worktree: String,
    pub started: String,
    pub seen: String,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StoreFile {
    #[serde(default)]
    claims: BTreeMap<String, Claim>,
}

#[derive(Debug)]
pub struct ClaimStore {
    path: PathBuf,
    claims: BTreeMap<String, Claim>,
}

impl ClaimStore {
    /// `XDG_STATE_HOME`, else `$HOME/.local/state`. Mirrors `Registry::path`, but under
    /// state rather than config: a claim is disposable, and losing the file costs only the
    /// overlay, never a task.
    ///
    /// The lookup is a parameter so tests can exercise resolution without `set_var`; the
    /// gate runs tests in parallel and process environment is shared.
    pub fn path_with(prefix: &str, get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf> {
        let base = if let Some(state) = get("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join(format!("tasks/claims/{prefix}.toml")))
    }

    pub fn path_for(prefix: &str) -> Result<PathBuf> {
        Self::path_with(prefix, std::env::var_os)
    }

    pub fn load(prefix: &str) -> Result<ClaimStore> {
        Self::load_from(&Self::path_for(prefix)?)
    }

    pub fn load_from(path: &Path) -> Result<ClaimStore> {
        let claims = if path.exists() {
            toml::from_str::<StoreFile>(&std::fs::read_to_string(path)?)
                .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?
                .claims
        } else {
            BTreeMap::new()
        };
        Ok(ClaimStore {
            path: path.to_path_buf(),
            claims,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = StoreFile {
            claims: self.claims.clone(),
        };
        atomic_write(
            &self.path,
            toml::to_string(&file)
                .expect("claim store serializes")
                .as_bytes(),
        )
    }

    pub fn get(&self, id: &TaskId) -> Option<&Claim> {
        self.claims.get(&id.to_string())
    }

    pub fn insert(&mut self, id: &TaskId, claim: Claim) {
        self.claims.insert(id.to_string(), claim);
    }

    pub fn remove(&mut self, id: &TaskId) -> Option<Claim> {
        self.claims.remove(&id.to_string())
    }

    /// Drops every claim `keep` rejects. The predicate is injected so liveness can be
    /// exercised deterministically; `prune_dead` in Task 2 is the real caller.
    pub fn prune_with(&mut self, keep: impl Fn(&Claim) -> bool) {
        self.claims.retain(|_, claim| keep(claim));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Claim)> {
        self.claims.iter()
    }
}
```

Add `mod claims;` to `src/main.rs` alongside the other `mod` lines.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks claims::`
Expected: PASS (4 tests). They pass under the default parallel runner, because none of them
touch process environment.

- [ ] **Step 5: Isolate the end-to-end test environment**

`tests/common/mod.rs` must stop the suite inheriting a real state directory or a real agent
identity. Extend the `env_remove` chain in `cmd` (`tests/common/mod.rs:22-28`):

```rust
        c.env("HOME", self.home.path())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_STATE_HOME")
            .env_remove("TASKS_FORMAT")
            .env_remove("TASKS_OWNER")
            .env_remove("TASKS_SESSION")
            .env_remove("TASKS_SESSION_PID")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("CLAUDE_PID")
            .env_remove("TASKS_COLOR")
            .env_remove("NO_COLOR")
            .env("USER", "tester")
            .current_dir(dir);
```

`HOME` already points at a temp dir, so with `XDG_STATE_HOME` removed the store lands under
`$HOME/.local/state/tasks/claims/` inside it. Add a helper the later tasks use:

```rust
    /// The claim store file for `prefix`, inside this environment's temp HOME.
    pub fn claim_store(&self, prefix: &str) -> PathBuf {
        self.home
            .path()
            .join(format!(".local/state/tasks/claims/{prefix}.toml"))
    }
```

- [ ] **Step 6: Verify isolation and commit**

Run: `just check && cargo test`
Expected: PASS, and nothing appears under your real `~/.local/state/tasks/`.

```bash
git add src/claims.rs src/main.rs tests/common/mod.rs
git commit -m "feat(claims): add the per-prefix claim store"
```

---

### Task 2: Liveness

**Files:**
- Modify: `src/claims.rs`
- Test: `src/claims.rs`

**Interfaces:**
- Consumes: `Claim` from Task 1.
- Produces: `Liveness::{Live, Stale}`, `ProcStat::{NotFound, Unreadable, Found}`, `proc_stat`, `boot_id`, `liveness`, `liveness_with`, `ClaimStore::prune_dead`, `TTL_HOURS`.

**Absence of evidence is never evidence of death.** Three distinct "we cannot tell" cases —
no recorded pid, no recorded `pid_start`, unreadable `/proc` — all take the TTL path. Only
positive evidence (the pid is gone, the recorded start time differs, a zombie, a different
boot) makes a claim stale ahead of the TTL.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module:

```rust
    fn at(s: &str) -> time::OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn seen(when: &str) -> Claim {
        Claim {
            seen: when.into(),
            ..sample()
        }
    }

    fn found(starttime: u64) -> impl Fn(u32) -> ProcStat {
        move |_| ProcStat::Found {
            state: 'S',
            starttime,
        }
    }

    #[test]
    fn a_confirmed_live_process_beats_any_ttl() {
        assert_eq!(
            liveness_with(
                &seen("2026-09-01T00:00:00Z"),
                at("2026-09-05T00:00:00Z"),
                Some("boot-a"),
                found(999)
            ),
            Liveness::Live,
            "an agent that thinks for days is not dead"
        );
    }

    #[test]
    fn a_confirmed_dead_pid_is_stale_at_once_with_no_grace() {
        assert!(matches!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                |_| ProcStat::NotFound
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn a_recycled_pid_does_not_resurrect_a_claim() {
        assert!(matches!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                found(12345) // != claim.pid_start
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn a_missing_recorded_start_time_takes_the_ttl_path_not_the_reuse_path() {
        // Acquisition records `pid_start: None` when /proc was unreadable at the time. If
        // /proc becomes readable later, there is still no evidence of reuse — only an
        // absence of evidence — so the claim must not be declared stale on that basis.
        let no_evidence = Claim {
            pid_start: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        assert_eq!(
            liveness_with(
                &no_evidence,
                at("2026-09-05T01:00:00Z"),
                Some("boot-a"),
                found(999)
            ),
            Liveness::Live,
            "inside the TTL"
        );
        assert!(
            matches!(
                liveness_with(
                    &no_evidence,
                    at("2026-09-05T05:00:00Z"),
                    Some("boot-a"),
                    found(999)
                ),
                Liveness::Stale(_)
            ),
            "and past it, by TTL rather than by a false reuse verdict"
        );
    }

    #[test]
    fn a_reboot_invalidates_the_pid_and_start_time_pair() {
        assert!(
            matches!(
                liveness_with(
                    &seen("2026-09-05T00:00:00Z"),
                    at("2026-09-05T00:00:01Z"),
                    Some("boot-b"),
                    found(999)
                ),
                Liveness::Stale(_)
            ),
            "start time counts ticks since boot, so the pair means nothing across boots"
        );
    }

    #[test]
    fn a_zombie_is_dead() {
        assert!(matches!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                |_| ProcStat::Found { state: 'Z', starttime: 999 }
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn unreadable_proc_falls_to_ttl_rather_than_reading_as_death() {
        assert_eq!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T03:59:00Z"),
                Some("boot-a"),
                |_| ProcStat::Unreadable
            ),
            Liveness::Live,
            "a permission failure is not proof of death"
        );
        assert!(matches!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T04:00:01Z"),
                Some("boot-a"),
                |_| ProcStat::Unreadable
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn an_unreadable_boot_id_or_a_pidless_claim_falls_to_ttl() {
        assert_eq!(
            liveness_with(&seen("2026-09-05T00:00:00Z"), at("2026-09-05T01:00:00Z"), None, found(999)),
            Liveness::Live
        );
        let pidless = Claim {
            pid: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        assert!(matches!(
            liveness_with(&pidless, at("2026-09-05T05:00:00Z"), Some("boot-a"), found(999)),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn proc_stat_parses_a_comm_containing_spaces_and_parentheses() {
        assert_eq!(
            parse_proc_stat("7 (weird ) name) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 4242 x"),
            ProcStat::Found { state: 'S', starttime: 4242 },
            "fields are counted from after the LAST ')'"
        );
        assert_eq!(parse_proc_stat("garbage"), ProcStat::Unreadable);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks claims::`
Expected: FAIL — `cannot find value liveness_with` / `ProcStat`.

- [ ] **Step 3: Write minimal implementation**

Append to `src/claims.rs`:

```rust
/// Hours a claim whose liveness cannot be established stays live. It applies *only* on the
/// unverifiable path: a confirmed-live process outlives it, and a confirmed-dead one gets
/// no grace at all.
pub const TTL_HOURS: i64 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Liveness {
    Live,
    Stale(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcStat {
    NotFound,
    /// The process may exist; we could not tell. Not evidence of death.
    Unreadable,
    Found { state: char, starttime: u64 },
}

/// The `comm` field can contain spaces and parentheses, so fields are taken from after the
/// *last* `)`: in that remainder, state is field 1 and start time is field 20 (fields 3 and
/// 22 of the whole line).
pub fn parse_proc_stat(line: &str) -> ProcStat {
    let Some((_, rest)) = line.rsplit_once(") ") else {
        return ProcStat::Unreadable;
    };
    let fields: Vec<&str> = rest.split_whitespace().collect();
    match (
        fields.first().and_then(|s| s.chars().next()),
        fields.get(19).and_then(|s| s.parse().ok()),
    ) {
        (Some(state), Some(starttime)) => ProcStat::Found { state, starttime },
        _ => ProcStat::Unreadable,
    }
}

pub fn proc_stat(pid: u32) -> ProcStat {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(text) => parse_proc_stat(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ProcStat::NotFound,
        Err(_) => ProcStat::Unreadable,
    }
}

pub fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|id| id.trim().to_string())
}

pub fn liveness(claim: &Claim) -> Liveness {
    liveness_with(
        claim,
        time::OffsetDateTime::now_utc(),
        boot_id().as_deref(),
        proc_stat,
    )
}

pub fn liveness_with(
    claim: &Claim,
    now: time::OffsetDateTime,
    boot_id: Option<&str>,
    stat: impl Fn(u32) -> ProcStat,
) -> Liveness {
    let ttl = || match crate::time::parse(&claim.seen) {
        Ok(seen) if now - seen <= time::Duration::hours(TTL_HOURS) => Liveness::Live,
        Ok(seen) => Liveness::Stale(format!(
            "not seen for {}h",
            (now - seen).whole_hours().max(0)
        )),
        // A corrupt timestamp is not liveness evidence either way; treating it as gone at
        // least keeps a garbled store from pinning a task forever.
        Err(_) => Liveness::Stale(format!("unreadable timestamp {:?}", claim.seen)),
    };

    let Some(pid) = claim.pid else { return ttl() };
    match (boot_id, &claim.boot_id) {
        // A different boot makes the pid/start-time pair meaningless: this is positive
        // evidence of death, not an absence of evidence.
        (Some(current), Some(recorded)) if current != recorded => {
            return Liveness::Stale("recorded on an earlier boot".into());
        }
        (None, _) | (_, None) => return ttl(),
        _ => {}
    }
    match stat(pid) {
        ProcStat::NotFound => Liveness::Stale(format!("pid {pid} is gone")),
        ProcStat::Unreadable => ttl(),
        ProcStat::Found { state: 'Z', .. } => Liveness::Stale(format!("pid {pid} is a zombie")),
        // No recorded start time is an absence of evidence, not evidence of reuse.
        ProcStat::Found { starttime, .. } => match claim.pid_start {
            Some(recorded) if recorded != starttime => {
                Liveness::Stale(format!("pid {pid} was reused by another process"))
            }
            Some(_) => Liveness::Live,
            None => ttl(),
        },
    }
}

impl ClaimStore {
    pub fn prune_dead(&mut self) {
        self.prune_with(|claim| liveness(claim) == Liveness::Live);
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks claims::`
Expected: PASS (13 tests).

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs
git commit -m "feat(claims): judge liveness by pid, boot id, and a four-hour fallback"
```

---

### Task 3: The mutation lock

**Files:**
- Modify: `src/claims.rs`
- Test: `src/claims.rs`

**Interfaces:**
- Produces: `MutationLock`, `MutationLock::{acquire, acquire_at, path_for}`.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module. These use `acquire_at` with a temp path, so no test touches
process environment:

```rust
    #[test]
    fn the_lock_serializes_two_writers() {
        use std::sync::{Arc, Barrier};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.lock");

        let held = MutationLock::acquire_at(&path).unwrap();
        let ready = Arc::new(Barrier::new(2));
        let waiter = {
            let ready = Arc::clone(&ready);
            let path = path.clone();
            std::thread::spawn(move || {
                ready.wait();
                let _second = MutationLock::acquire_at(&path).unwrap();
                std::time::Instant::now()
            })
        };

        ready.wait();
        std::thread::sleep(std::time::Duration::from_millis(150));
        let released = std::time::Instant::now();
        drop(held);

        assert!(
            waiter.join().unwrap() >= released,
            "the second writer must not enter before the first leaves"
        );
    }

    #[test]
    fn the_lock_is_a_separate_file_from_the_store() {
        let store = ClaimStore::path_with("sci", env_of(&[("XDG_STATE_HOME", "/xdg")])).unwrap();
        let lock = MutationLock::path_with("sci", env_of(&[("XDG_STATE_HOME", "/xdg")])).unwrap();
        assert_ne!(store, lock);
        assert_eq!(lock, PathBuf::from("/xdg/tasks/claims/sci.lock"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks claims::`
Expected: FAIL — `cannot find type MutationLock`.

- [ ] **Step 3: Write minimal implementation**

Append to `src/claims.rs`:

```rust
/// Serializes every read-modify-write against one project: the task markdown files *and*
/// the claim store together.
///
/// It has to span both. The guard in `transition` and the write in `save` are separate
/// steps, so a lock over the claim store alone leaves a takeover race in the gap between
/// them. And `note`, whatever its append-only meaning, rewrites the whole markdown file at
/// the storage layer, so an unserialized note can clobber a concurrent status change.
///
/// The lock is its own file because `atomic_write` replaces the store's inode by rename:
/// locking the store itself would leave each writer holding a lock on a file no longer at
/// that path. A process that dies holding it has it released by the kernel, so there is no
/// stale-lock recovery path.
#[derive(Debug)]
pub struct MutationLock {
    _file: std::fs::File,
}

impl MutationLock {
    pub fn path_with(prefix: &str, get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf> {
        Ok(ClaimStore::path_with(prefix, get)?.with_file_name(format!("{prefix}.lock")))
    }

    pub fn path_for(prefix: &str) -> Result<PathBuf> {
        Self::path_with(prefix, std::env::var_os)
    }

    pub fn acquire(prefix: &str) -> Result<MutationLock> {
        Self::acquire_at(&Self::path_for(prefix)?)
    }

    pub fn acquire_at(path: &Path) -> Result<MutationLock> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        file.lock()
            .map_err(|error| Error::Io(format!("locking {}: {error}", path.display())))?;
        Ok(MutationLock { _file: file })
    }
}
```

The lock releases when `MutationLock` drops, because dropping the `File` closes the
descriptor.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks claims::`
Expected: PASS (15 tests), under the default parallel runner.

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs
git commit -m "feat(claims): add the per-project mutation lock"
```

---

### Task 4: Wire the lock into Ctx without locking the read commands

**Files:**
- Modify: `src/commands/mod.rs:29-41` (`Ctx`, `open_ctx`), `src/commands/mod.rs:255-346` (`run`), `src/commands/edit.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `MutationLock` from Task 3.
- Produces: `Ctx { project, registry, warnings, lock: Option<MutationLock>, claims: Option<ClaimStore>, pending_claim: Option<ClaimIntent> }`, `open_write_ctx`, `Ctx::claims_mut`.

`show`, `graph`, `check`, `add` and `feedback` keep the unlocked `open_ctx`; the eight write
paths move to `open_write_ctx`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn read_commands_do_not_take_the_mutation_lock() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let lock = env.claim_store("sci").with_file_name("sci.lock");

    env.json(&sci, &["show", &id]);
    env.json(&sci, &["check"]);
    env.json(&sci, &["graph"]);
    assert!(!lock.exists(), "read commands must not create the lock");

    env.json(&sci, &["note", &id, "hello"]);
    assert!(lock.exists(), "a write command takes it");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tasks --test cli read_commands_do_not_take`
Expected: FAIL — the lock file never appears.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/mod.rs`:

```rust
use crate::claims::{ClaimStore, MutationLock};

/// What `save` must do to the claim store once every validation has passed. Recorded by
/// the guard in `transition`; **nothing is persisted until `save` acts on it.**
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release,
}

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for the life of a write command; `None` for the read commands that also take a
    /// `Ctx` (`show`, `graph`, `check`) and for the two create-only paths.
    pub lock: Option<MutationLock>,
    claims: Option<ClaimStore>,
    pending_claim: Option<(TaskId, ClaimIntent)>,
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    Ok(Ctx {
        project: Project::locate(&start)?,
        registry: Registry::load()?,
        warnings: Vec::new(),
        lock: None,
        claims: None,
        pending_claim: None,
    })
}

/// `open_ctx` plus the project's mutation lock, held until the command ends.
///
/// The lock cannot simply live in `open_ctx`: `show` (`mod.rs:289`), `graph`
/// (`mod.rs:329`) and `check` (`mod.rs:330`) take a `Ctx` too and must stay read-only, and
/// an exclusive lock would have them block on writers and on each other. Readers need no
/// shared lock either — `atomic_write` publishes by rename, so a reader sees one whole
/// version or another, never a torn one.
pub fn open_write_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let mut ctx = open_ctx(dir)?;
    ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
    Ok(ctx)
}

impl Ctx {
    /// The claim store, loaded on first use. Only reachable with the lock held, so every
    /// read-check-write against it sits inside one critical section.
    pub fn claims_mut(&mut self) -> Result<&mut ClaimStore> {
        if self.lock.is_none() {
            return Err(Error::Io(
                "claim store touched without the mutation lock".into(),
            ));
        }
        if self.claims.is_none() {
            self.claims = Some(ClaimStore::load(&self.project.prefix)?);
        }
        Ok(self.claims.as_mut().expect("just loaded"))
    }
}
```

Switch exactly the eight write paths in `run` to `open_write_ctx` — `Edit`, `Note`, `Start`,
`Done`, `Drop`, `Block`, `Unblock`, `Dep` — leaving `Show`, `Graph`, `Check`, `Add` and
`Feedback` on `open_ctx`. Add the three new fields to the `Ctx { .. }` literal in the
`Command::Add` arm (`src/commands/mod.rs:280-285`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --bin tasks --test cli read_commands_do_not_take`
Expected: PASS.

- [ ] **Step 5: Release the lock around the interactive editor**

`editor()` waits on `$EDITOR`, which can be a human's minutes. Drop the lock before spawning
and re-acquire before validating. Immediately before the `std::process::Command::new("sh")`
call in `src/commands/edit.rs`:

```rust
    // The lock must not span a human's editing session. The raw-content comparison further
    // down is what protects the gap, and because no claim is persisted until `save`, a
    // rejected concurrent edit cannot leave a claim behind either.
    let prefix = ctx.project.prefix.clone();
    ctx.lock = None;
```

and immediately after the `if !status.success()` block:

```rust
    ctx.lock = Some(crate::claims::MutationLock::acquire(&prefix)?);
```

- [ ] **Step 6: Verify and commit**

Run: `just check && cargo test`
Expected: PASS.

```bash
git add src/commands tests/cli.rs
git commit -m "feat(claims): hold a mutation lock for write commands only"
```

---

### Task 5: Session identity and the `claimed` error kind

**Files:**
- Modify: `src/claims.rs`, `src/error.rs`
- Test: `src/claims.rs`

**Interfaces:**
- Produces: `Identity { session, pid }`, `identity() -> Result<Identity>`, `identity_from(get, session_pid) -> Result<Identity>`, `unix_session_id`, `hostname`, `Error::Claimed(String, String)` with kind `"claimed"`.

**Unresolvable identity is a typed error, never a shared default.** A placeholder session
would make every anonymous caller pass the same-session check, defeating both the refusal
and ownership-aware release. TTL can stand in for unavailable *liveness*; nothing stands in
for unavailable *identity*.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn identity_prefers_the_explicit_pair() {
        let id = identity_from(
            env_of(&[
                ("TASKS_SESSION", "explicit"),
                ("TASKS_SESSION_PID", "7"),
                ("CLAUDE_CODE_SESSION_ID", "claude"),
                ("CLAUDE_PID", "9"),
            ]),
            Some(11),
        )
        .unwrap();
        assert_eq!(id.session, "explicit");
        assert_eq!(id.pid, Some(7));
    }

    #[test]
    fn a_level_never_borrows_another_levels_pid() {
        // Claude Code old enough to export the session but not CLAUDE_PID.
        let id = identity_from(env_of(&[("CLAUDE_CODE_SESSION_ID", "claude")]), Some(11)).unwrap();
        assert_eq!(id.session, "claude");
        assert_eq!(
            id.pid, None,
            "a session without its own pid must not be welded to the fallback pid"
        );
    }

    #[test]
    fn falls_back_to_the_unix_session_id() {
        let id = identity_from(env_of(&[]), Some(11)).unwrap();
        assert_eq!(id.session, "sid:11");
        assert_eq!(id.pid, Some(11));
    }

    #[test]
    fn an_empty_variable_does_not_count_as_set() {
        let id = identity_from(env_of(&[("TASKS_SESSION", "")]), Some(11)).unwrap();
        assert_eq!(id.session, "sid:11", "emptiness is filtered inside the helper");
    }

    #[test]
    fn unresolvable_identity_is_an_error_not_a_shared_placeholder() {
        let error = identity_from(env_of(&[]), None).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(
            error.to_string().contains("TASKS_SESSION"),
            "the message names the way out: {error}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks claims::`
Expected: FAIL — `cannot find function identity_from`.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Who holds a claim, and the pid that proves it is still alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub session: String,
    pub pid: Option<u32>,
}

/// The caller's Unix session id, from `/proc/self/stat` field 6 (field 4 of the remainder
/// after the last `)`). Stable across commands from one terminal, and itself a pid.
pub fn unix_session_id() -> Option<u32> {
    let text = std::fs::read_to_string("/proc/self/stat").ok()?;
    let (_, rest) = text.rsplit_once(") ")?;
    rest.split_whitespace().nth(3)?.parse().ok()
}

pub fn identity() -> Result<Identity> {
    identity_from(std::env::var_os, unix_session_id())
}

/// Session and pid resolve as a **pair from a single level**. A level that yields a session
/// but no usable pid yields `pid: None` and falls to the TTL path; it is never welded to an
/// unrelated fallback pid.
///
/// Level 2 names Claude Code's variables because `skills/tasks/SKILL.md` ships to agent
/// projects, so agents are a first-class consumer and this makes claims work with no setup.
/// `CLAUDE_PID` needs a recent Claude Code; without it, level 2 still supplies the session.
///
/// Level 3 is *terminal* identity, not agent identity: several agents can share one
/// terminal, and a terminal outlives the agent that ran in it. Subagents inside one Claude
/// Code process likewise share level 2. `TASKS_SESSION` is the escape hatch for both.
///
/// When nothing resolves, this is an error. A placeholder session would make every
/// anonymous caller pass the same-session check, silently turning "no identity" into
/// "shared ownership" and defeating both the refusal and ownership-aware release.
pub fn identity_from(
    get: impl Fn(&str) -> Option<OsString>,
    session_pid: Option<u32>,
) -> Result<Identity> {
    let var = |key: &str| {
        get(key)
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty())
    };
    let pid_of = |key: &str| var(key).and_then(|value| value.parse().ok());
    if let Some(session) = var("TASKS_SESSION") {
        return Ok(Identity {
            session,
            pid: pid_of("TASKS_SESSION_PID"),
        });
    }
    if let Some(session) = var("CLAUDE_CODE_SESSION_ID") {
        return Ok(Identity {
            session,
            pid: pid_of("CLAUDE_PID"),
        });
    }
    match session_pid {
        Some(pid) => Ok(Identity {
            session: format!("sid:{pid}"),
            pid: Some(pid),
        }),
        None => Err(Error::Config(
            "cannot determine a session identity: set TASKS_SESSION (no \
             CLAUDE_CODE_SESSION_ID, and /proc/self/stat is unreadable)"
                .into(),
        )),
    }
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}
```

In `src/error.rs` add the variant, its `kind` (`"claimed"`), and its `with_suffix` arm:

```rust
    #[error("{0} is claimed by {1}")]
    Claimed(String, String),
```
```rust
            Error::Claimed(..) => "claimed",
```
```rust
            Error::Claimed(id, detail) => Error::Claimed(id, detail + suffix),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks claims::`
Expected: PASS (20 tests).

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/error.rs
git commit -m "feat(claims): resolve session identity as a pair, failing when unresolvable"
```
---

### Task 6: Claims in the JSON and pretty output

This comes *before* the guard so the later tasks' assertions have a contract to assert
against. It is tested with a hand-written store fixture, so it depends on nothing but Task 1.

**Files:**
- Modify: `src/output.rs` (`ClaimInfo`, `TaskSummary`, `ShowFields`, `pretty`), `src/commands/show.rs`, `src/commands/list.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `ClaimInfo { owner, session, host, pid, worktree, started, seen, live }`, `ClaimInfo::of(&Claim)`, `TaskSummary.claim`, `ShowFields.claim`, `TaskSummary::of(task, all, Option<&ClaimStore>)`.
- Test helper produced: `write_claim(&TestEnv, prefix, id, session, live)`.

**`ShowOut.fields` is `#[serde(flatten)]`** (`src/output.rs:78`), so `show` exposes the claim
at `v["claim"]`. Asserting `v["fields"]["claim"]` silently passes every null assertion and
fails every positive one.

- [ ] **Step 1: Write the failing test**

Add the fixture helper and the test to `tests/cli.rs`:

```rust
/// Writes a claim straight into the store.
///
/// A `live` claim carries the *test process's own* pid, start time and boot id, so the
/// liveness rules confirm it. That matters: a fixture without this metadata is stale, and a
/// stale claim can be pruned out from under an assertion, letting a test pass without ever
/// exercising what it names. `seen` is deliberately ancient, which also pins the rule that a
/// confirmed-live process beats any TTL.
fn write_claim(env: &TestEnv, prefix: &str, id: &str, session: &str, live: bool) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (pid, pid_start, boot) = if live {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let rest = stat.rsplit_once(") ").unwrap().1.to_string();
        let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
        (std::process::id(), start, boot.trim().to_string())
    } else {
        (0, 1, "not-this-boot".to_string())
    };
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {pid}\n\
         pid_start = {pid_start}\nboot_id = \"{boot}\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n"
    ));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn claim_appears_in_show_and_list_json() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // ShowOut.fields is #[serde(flatten)], so the claim is at the top level.
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());

    write_claim(&env, "sci", &id, "agent-a", true);

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["claim"]["session"], "agent-a");
    assert_eq!(v["claim"]["live"], true);
    assert_eq!(v["claim"]["worktree"], "/elsewhere");
    assert_eq!(v["claim"]["pid"], std::process::id());

    let list = env.json(&sci, &["list"]);
    assert_eq!(list["tasks"][0]["claim"]["session"], "agent-a");
}

#[test]
fn a_dead_claim_is_reported_as_not_live() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "ghost", false);
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["live"], false);
}

#[test]
fn pretty_rows_name_the_claim_holder_not_the_local_owner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "agent-a", true);

    let out = env.cmd(&sci).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        text.contains("someone"),
        "the claim's own owner, which the local file may not know: {text}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks --test cli claim_appears_in_show`
Expected: FAIL — `claim` is null even with the fixture in place.

- [ ] **Step 3: Write minimal implementation**

In `src/output.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ClaimInfo {
    pub owner: String,
    pub session: String,
    pub host: String,
    pub pid: Option<u32>,
    pub worktree: String,
    pub started: String,
    pub seen: String,
    pub live: bool,
}

impl ClaimInfo {
    pub fn of(claim: &crate::claims::Claim) -> ClaimInfo {
        ClaimInfo {
            owner: claim.owner.clone(),
            session: claim.session.clone(),
            host: claim.host.clone(),
            pid: claim.pid,
            worktree: claim.worktree.clone(),
            started: claim.started.clone(),
            seen: claim.seen.clone(),
            live: crate::claims::liveness(claim) == crate::claims::Liveness::Live,
        }
    }
}
```

Add `pub claim: Option<ClaimInfo>` to `TaskSummary` and `ShowFields`, and give
`TaskSummary::of` a third parameter:

```rust
    pub fn of(
        task: &Task,
        all: &[Task],
        claims: Option<&crate::claims::ClaimStore>,
    ) -> TaskSummary {
        // ... existing fields unchanged ...
        claim: claims
            .and_then(|store| store.get(&task.id))
            .map(ClaimInfo::of),
    }
```

Read paths load the store with `ClaimStore::load(&prefix)?` and take **no lock**:
`atomic_write` publishes by rename, so a reader sees one whole version or another.

In the `table` helper inside `pretty`, prefer the claim's own owner. The local file's
`owner` can be an older branch's value — the claim is the live fact:

```rust
    let owner = match &row.claim {
        // The holder as the claim records it. `row.owner` comes from this checkout's copy
        // of the file, which may predate the claim entirely.
        Some(claim) if claim.live => format!("@{} [{}]", claim.owner, claim.session),
        Some(claim) => format!("@{} [{} stale]", claim.owner, claim.session),
        None => match &row.owner {
            Some(owner) => format!("@{owner}"),
            None => String::new(),
        },
    };
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli claim && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/output.rs src/commands tests/cli.rs
git commit -m "feat(output): report the claim on show, list, and pretty rows"
```

---

### Task 7: The guard, acquire, and destination-based release

The core task, and one unit: the guard, the acquire and the release are a single
transaction that a reviewer cannot sensibly split.

**The rule that shapes all of it: `transition()` guards and records an intent; `save()` is
the only place that persists anything.** An earlier draft acquired inside `transition()`,
which meant a later validation failure, a rejected concurrent edit, or a failed write could
each leave the store mutated — three separate failure paths. With persistence in one
validated operation there is one.

**Files:**
- Modify: `src/commands/mod.rs` (`transition`, `save`, `Ctx` claim helpers), `src/commands/status.rs` (`note` heartbeat)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Ctx::claims_mut`, `ClaimIntent` (Task 4), `identity`, `liveness`, `Claim`, `hostname` (Tasks 1-5), `ClaimInfo` (Task 6).
- Produces: `transition(ctx: &mut Ctx, ..)`, `save(ctx: &mut Ctx, ..)`, `Ctx::describe_claim`.

`transition` and `save` change from `&Ctx` to `&mut Ctx`; every caller in
`src/commands/{status,edit,add,dep}.rs` needs `mut ctx`. Mechanical — the compiler lists them.

- [ ] **Step 1: Write the failing tests**

```rust
/// Two project roots sharing one prefix: what a main checkout and a worktree look like to a
/// store keyed by prefix. The second `init` needs `--force` because the registry refuses to
/// re-point a prefix silently.
fn two_roots(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf) {
    let a = env.init("sci");
    let b = env.init_forced("sci");
    (a, b)
}

/// Run `tasks` as a named agent with a live pid.
fn as_agent<'a>(env: &'a TestEnv, dir: &std::path::Path, session: &str) -> assert_cmd::Command {
    let mut cmd = env.cmd(dir);
    cmd.env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    cmd
}

fn err_kind(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    v["error"]["kind"].as_str().unwrap().to_string()
}

fn err_detail(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    // The error shape is {"error": {"kind", "detail"}} — there is no `message` field.
    v["error"]["detail"].as_str().unwrap().to_string()
}

#[test]
fn a_live_claim_from_another_session_refuses_start() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();

    let out = as_agent(&env, &sci, "agent-b").args(["start", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "claimed");
    assert!(err_detail(&out).contains("agent-a"), "{}", err_detail(&out));
}

#[test]
fn a_displaced_session_cannot_close_the_task_it_lost() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    as_agent(&env, &sci, "agent-b").args(["start", "--force", &id]).assert().success();

    for args in [
        vec!["done", &id, "landed"],
        vec!["drop", &id, "nope"],
        vec!["block", &id, "waiting"],
        vec!["edit", &id, "--status", "done"],
    ] {
        let out = as_agent(&env, &sci, "agent-a").args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "A must not close via {args:?}");
        assert_eq!(err_kind(&out), "claimed", "{args:?}");
    }

    assert_eq!(env.json(&sci, &["show", &id])["claim"]["session"], "agent-b");
}

#[test]
fn release_follows_the_claim_not_the_local_doing_status() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    // Root B's copy is the pre-claim file: still `todo`, the ordinary cross-worktree case.
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();

    as_agent(&env, &a, "agent-a").args(["start", &id]).assert().success();
    assert_eq!(env.json(&a, &["show", &id])["claim"]["session"], "agent-a");

    // The same session closes it from root B, where the local status was never `doing`.
    as_agent(&env, &b, "agent-a").args(["done", &id, "landed"]).assert().success();

    assert!(
        env.json(&a, &["show", &id])["claim"].is_null(),
        "released even though this checkout never left doing"
    );
}

#[test]
fn one_checkouts_closed_copy_does_not_prune_a_live_claim() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();

    // Root A closed it first; root B is a branch that reopened and claimed it since. Both
    // branch states must be established before the claim exists, or A's own close would be
    // refused — correctly — and the test would never reach what it is about.
    env.json(&a, &["edit", &id, "--status", "done"]);
    as_agent(&env, &b, "agent-b").args(["start", &id]).assert().success();

    // A write from root A must not treat its own stale `done` as authority over the claim.
    // `note` is the right probe: it is never refused, and it touches the store.
    as_agent(&env, &a, "agent-a").args(["note", &id, "still here"]).assert().success();

    assert_eq!(
        env.json(&b, &["show", &id])["claim"]["session"],
        "agent-b",
        "one checkout's view cannot establish that a shared claim is obsolete"
    );
}

#[test]
fn re_running_a_close_retries_a_failed_release() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let store = env.claim_store("sci");

    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    // Keep the real, live claim the store now holds; a hand-built one would be stale and
    // could be pruned away, letting this test pass without exercising owner release.
    let live_claim = std::fs::read_to_string(&store).unwrap();

    as_agent(&env, &sci, "agent-a").args(["done", &id, "landed"]).assert().success();
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());

    // Simulate a release that failed after the file write: the task is closed, the real
    // claim is still there.
    std::fs::write(&store, &live_claim).unwrap();
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["live"], true);

    // `start --force` cannot recover this: can_transition rejects done -> doing.
    let out = as_agent(&env, &sci, "agent-a").args(["start", "--force", &id]).output().unwrap();
    assert_eq!(err_kind(&out), "invalid_transition");

    // Re-running the closing command can, because a same-status transition still releases.
    as_agent(&env, &sci, "agent-a").args(["done", &id, "landed"]).assert().success();
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());
}

#[test]
fn a_stale_claim_is_taken_over_without_force_but_with_a_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "dead-agent", false);

    let out = as_agent(&env, &sci, "agent-b").args(["start", &id]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("dead-agent")),
        "taking over a stale claim names the displaced holder: {v}"
    );
}

#[test]
fn a_repeated_start_by_the_owner_keeps_the_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    let first = env.json(&sci, &["show", &id])["claim"]["started"].clone();
    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["session"], "agent-a");
    assert!(!first.is_null());
}
```

Add `init_forced` to `tests/common/mod.rs`:

```rust
    /// A second project root under an already-registered prefix — a worktree, as far as a
    /// prefix-keyed claim store is concerned.
    pub fn init_forced(&mut self, prefix: &str) -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        self.dirs.push(dir);
        self.json(&path, &["init", "--prefix", prefix, "--force"]);
        path
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks --test cli -- claim release_follows one_checkouts re_running a_displaced a_repeated`
Expected: FAIL — nothing refuses, nothing releases.

- [ ] **Step 3: Write the implementation**

In `src/commands/mod.rs`, add to `impl Ctx`:

```rust
    pub fn describe_claim(claim: &crate::claims::Claim, live: &Liveness) -> String {
        let pid = match claim.pid {
            Some(pid) => format!(", pid {pid}"),
            None => String::new(),
        };
        let state = match live {
            Liveness::Live => "live".to_string(),
            Liveness::Stale(why) => format!("stale: {why}"),
        };
        format!(
            "session {} (owner {}, host {}{pid}, worktree {}, since {}, {state})",
            claim.session, claim.owner, claim.host, claim.worktree, claim.started
        )
    }

    /// Guard only. Decides whether this session may make the change and records what
    /// `save` should do — **and persists nothing**, so a validation failure, a rejected
    /// concurrent edit, or a failed write cannot leave the store mutated.
    ///
    /// Release is destination-based: any destination other than `doing` releases this
    /// session's claim. It must not key off *leaving* a local `doing`, because a session
    /// can hold the shared claim while its own checkout still reads `todo` — the ordinary
    /// cross-worktree case — and its `done` there would otherwise strand the claim.
    fn claim_guard(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let me = crate::claims::identity()?;
        let owner = owner_name(&self.project)?;
        let worktree = self.project.root.display().to_string();
        let store = self.claims_mut()?;

        let mut warning = None;
        if let Some(existing) = store.get(id) {
            let live = crate::claims::liveness(existing);
            let mine = existing.session == me.session;
            match (&live, mine) {
                (Liveness::Live, false) if !(force && to == Status::Doing) => {
                    return Err(Error::Claimed(
                        id.to_string(),
                        Ctx::describe_claim(existing, &live),
                    ));
                }
                (Liveness::Live, false) => {
                    warning = Some(format!(
                        "took over a live claim held by {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                }
                (Liveness::Stale(_), false) => {
                    warning = Some(format!("took over {}", Ctx::describe_claim(existing, &live)));
                }
                _ => {}
            }
        }

        self.pending_claim = Some(if to == Status::Doing {
            let now = crate::time::now();
            let started = self
                .claims_mut()?
                .get(id)
                .filter(|existing| existing.session == me.session)
                .map(|existing| existing.started.clone())
                .unwrap_or_else(|| now.clone());
            (
                id.clone(),
                ClaimIntent::Acquire(crate::claims::Claim {
                    owner,
                    pid_start: me.pid.and_then(|pid| match crate::claims::proc_stat(pid) {
                        crate::claims::ProcStat::Found { starttime, .. } => Some(starttime),
                        _ => None,
                    }),
                    session: me.session,
                    pid: me.pid,
                    boot_id: crate::claims::boot_id(),
                    host: crate::claims::hostname(),
                    worktree,
                    started,
                    seen: now,
                }),
            )
        } else {
            (id.clone(), ClaimIntent::Release)
        });

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }
```

Then `transition` and `save`:

```rust
pub fn transition(ctx: &mut Ctx, task: &mut Task, to: Status, force: bool) -> Result<()> {
    if !Status::can_transition(task.status, to) {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            to.as_str().into(),
        ));
    }
    // Guard before the dependency and descendant checks, so a session that no longer holds
    // the task is told *that* rather than something incidental.
    ctx.claim_guard(&task.id, to, force)?;
    if to == Status::Done && task.status != Status::Done && !force {
        let open = open_deps(ctx, task)?;
        if !open.is_empty() {
            return Err(Error::OpenDependencies(task.id.to_string(), open.join(", ")));
        }
    }
    let closing = matches!(to, Status::Done | Status::Dropped) && task.status != to;
    if closing && !(force && to == Status::Done) {
        let all = ctx.project.scan()?;
        let open: Vec<String> = crate::hierarchy::open_descendants(&all, &task.id)
            .iter()
            .map(|task| task.id.to_string())
            .collect();
        if !open.is_empty() {
            return Err(Error::OpenDescendants(task.id.to_string(), open.join(", ")));
        }
    }
    task.status = to;
    Ok(())
}

/// The only place claim state is persisted.
///
/// Everything that can reject the change runs first and touches nothing. From there the
/// store and the task file move together, in the order that fails toward "claim held": a
/// claim with no file update makes an idle task look busy and self-heals when the session
/// dies, while a file update with no claim is the invisibility bug this exists to remove.
pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    ctx.project.validate_docs(task)?;

    match ctx.pending_claim.take() {
        Some((id, ClaimIntent::Acquire(claim))) => {
            let store = ctx.claims_mut()?;
            // Captured, never assumed absent: a repeated `start` by the owner and a forced
            // takeover both write over an existing claim, and a blanket removal on failure
            // would erase work someone still holds.
            let previous = store.get(&id).cloned();
            store.prune_dead();
            store.insert(&id, claim);
            store.save()?;

            let Err(error) = ctx.project.write_task(task) else {
                return Ok(());
            };
            let store = ctx.claims_mut()?;
            match previous {
                Some(previous) => store.insert(&id, previous),
                None => {
                    store.remove(&id);
                }
            }
            // Warnings on `Ctx` are dropped when a command returns `Err`, so recovery
            // guidance has to travel on the error itself.
            let suffix = match store.save() {
                Ok(()) => String::new(),
                Err(inner) => format!(
                    " (the previous claim on {id} could not be restored: {inner}; \
                     run `tasks start --force {id}` to reclaim it)"
                ),
            };
            Err(error.with_suffix(&suffix))
        }
        Some((id, ClaimIntent::Release)) => {
            ctx.project.write_task(task)?;
            let store = ctx.claims_mut()?;
            store.prune_dead();
            store.remove(&id);
            if let Err(error) = store.save() {
                // The task is closed now, so `start --force` cannot recover this:
                // `can_transition` rejects `done -> doing`. Re-running the same closing
                // command can, because a same-status transition still releases.
                ctx.warnings.push(format!(
                    "{id} was closed but its claim was not released ({error}); \
                     re-run the same command to retry the release"
                ));
            }
            Ok(())
        }
        None => ctx.project.write_task(task),
    }
}
```

In `src/commands/status.rs`, make callers `mut` and add the heartbeat to `note`:

```rust
pub fn note(mut ctx: Ctx, id: String, text: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &text)?;
    save(&mut ctx, &mut task)?;
    // The heartbeat, and only on our own claim: `note` never touches a foreign one and is
    // never refused. It is still serialized under the mutation lock, because a note
    // rewrites the whole markdown file however append-only it is in meaning.
    let me = crate::claims::identity()?;
    let store = ctx.claims_mut()?;
    if let Some(claim) = store.get(&task.id).cloned()
        && claim.session == me.session
    {
        store.insert(
            &task.id,
            crate::claims::Claim {
                seen: crate::time::now(),
                ..claim
            },
        );
        store.save()?;
    }
    Ok(id_out(ctx, &task))
}
```

Apply `mut ctx` and the `&mut ctx` argument in `start`, `close`, `unblock`, `edit::run`,
`editor`, `add::run` and `dep::run`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli && cargo test`
Expected: PASS. `a_displaced_session_cannot_close_the_task_it_lost` and
`a_repeated_start_by_the_owner_keeps_the_claim` need Task 8's `--force`; run them at the end
of Task 8 if they fail only on the unknown flag.

- [ ] **Step 5: Commit**

```bash
git add src/commands tests
git commit -m "feat(claims): guard status changes and persist claims inside save"
```

---

### Task 8: `tasks start --force` takeover

**Files:**
- Modify: `src/cli.rs:125-126`, `src/commands/mod.rs` (`Command::Start` arm), `src/commands/status.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `Command::Start { id, force }`, `status::start(ctx, id, force)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn force_takeover_records_a_note_naming_the_displaced_session() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    as_agent(&env, &sci, "agent-b").args(["start", "--force", &id]).assert().success();

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("agent-a"), "the takeover is recorded in the notes: {raw}");
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["session"], "agent-b");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tasks --test cli force_takeover`
Expected: FAIL — `unexpected argument '--force'`.

- [ ] **Step 3: Write minimal implementation**

`src/cli.rs`:

```rust
    /// Claim a task: status=doing, owner=you.
    Start {
        id: String,
        /// Take over a claim another live session holds.
        #[arg(long)]
        force: bool,
    },
```

`src/commands/mod.rs`:

```rust
        Command::Start { id, force } => status::start(open_write_ctx(dir)?, id, force),
```

`src/commands/status.rs`:

```rust
pub fn start(mut ctx: Ctx, id: String, force: bool) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let before = ctx.warnings.len();
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    task.owner = Some(owner_name(&ctx.project)?);
    // A takeover displaces someone; the task's own record should say so, not just the
    // ephemeral warning stream.
    let takeovers: Vec<String> = ctx.warnings[before..].to_vec();
    let owner = owner_name(&ctx.project)?;
    for takeover in takeovers {
        append_note(&mut task, &owner, &takeover)?;
    }
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli && cargo test`
Expected: PASS, including the two Task 7 tests that needed `--force`.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/commands tests/cli.rs
git commit -m "feat(start): add --force to take over a claim"
```

---

### Task 9: `ready`/`next` exclusion and the `prime` overlay

**Files:**
- Modify: `src/commands/list.rs` (`ready_tasks`, `prime`), `src/commands/show.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `ClaimSnapshot`, `ClaimSnapshot::{load, live, get, stale}`.

**One snapshot per command.** Loading the store separately for the ready filter, the doing
predicate and the stale warnings lets one `prime` contradict itself — a claim can go stale
between two of those reads. Load each prefix once and reuse it everywhere, including under
`--all-projects`.

**`ready_tasks` already skips anything whose status is not `Todo`** (`src/commands/list.rs:87-89`).
So a task the current session started *locally* is excluded by status before any claim logic
runs, and no claim warning fires for it — correct, and not something to test for. The case
that matters, and the one the exclusion exists for, is a task whose **local** file still says
`todo` while another root holds a live claim.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn ready_omits_a_task_claimed_from_another_root_and_says_why() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--size", "s"]));
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();

    assert_eq!(env.json(&b, &["ready"])["tasks"][0]["id"], id);

    as_agent(&env, &a, "agent-a").args(["start", &id]).assert().success();

    // Root B's copy still reads `todo`, so only the claim can exclude it.
    let v = env.json(&b, &["ready"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 0, "{v}");
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("agent-a")
        }),
        "a silent omission is worse than an explained one: {v}"
    );
    assert_eq!(env.json(&b, &["next"])["next"], serde_json::Value::Null);
}

#[test]
fn prime_shows_a_claim_made_in_another_root_and_warns_about_divergence() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();

    as_agent(&env, &a, "agent-a").args(["start", &id]).assert().success();

    let v = env.json(&b, &["prime"]);
    assert!(
        v["doing"].as_array().unwrap().iter().any(|t| t["id"] == id.as_str()),
        "a claim made in another worktree shows as doing here: {v}"
    );
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("conflict")
        }),
        "the divergent copies are called out: {v}"
    );
}

#[test]
fn prime_warns_about_a_stale_claim_over_a_local_todo() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "dead-agent", false);

    let v = env.json(&sci, &["prime"]);
    // The `doing` predicate needs a *live* claim, so without this warning a stale claim
    // over a local `todo` would appear nowhere at all.
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains("dead-agent") && w.contains(&id)
        }),
        "{v}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks --test cli -- ready_omits prime_shows prime_warns`
Expected: FAIL — claimed tasks still appear, no warnings.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/list.rs`:

```rust
/// Every claim in scope, read **once** per command.
///
/// Reading the store separately for the ready filter, the doing predicate and the stale
/// warnings would let a single `prime` contradict itself, since a claim can go stale
/// between two reads. Liveness is evaluated here, once, and reused everywhere.
pub struct ClaimSnapshot {
    by_id: std::collections::BTreeMap<String, (Claim, Liveness)>,
    stores: Vec<ClaimStore>,
}

impl ClaimSnapshot {
    pub fn load<'a>(prefixes: impl Iterator<Item = &'a str>) -> Result<ClaimSnapshot> {
        let mut by_id = std::collections::BTreeMap::new();
        let mut stores = Vec::new();
        for prefix in prefixes {
            let store = ClaimStore::load(prefix)?;
            for (id, claim) in store.iter() {
                by_id.insert(id.clone(), (claim.clone(), liveness(claim)));
            }
            stores.push(store);
        }
        Ok(ClaimSnapshot { by_id, stores })
    }

    pub fn live(&self, id: &TaskId) -> Option<&Claim> {
        match self.by_id.get(&id.to_string()) {
            Some((claim, Liveness::Live)) => Some(claim),
            _ => None,
        }
    }

    pub fn stale(&self) -> impl Iterator<Item = (&String, &Claim, &String)> {
        self.by_id.iter().filter_map(|(id, (claim, live))| match live {
            Liveness::Stale(why) => Some((id, claim, why)),
            Liveness::Live => None,
        })
    }

    /// For `TaskSummary::of`, which reads claims by id.
    pub fn stores(&self) -> &[ClaimStore] {
        &self.stores
    }
}
```

In `ready_tasks`, after the existing filters build `ready`:

```rust
    let claims = ClaimSnapshot::load(ctx.scope.prefixes().iter().map(String::as_str))?;
    ready.retain(|task| match claims.live(&task.id) {
        Some(claim) => {
            warnings.push(format!(
                "{} omitted: claimed by session {} in {} — \
                 `tasks start --force {}` to take it over",
                task.id, claim.session, claim.worktree, task.id
            ));
            false
        }
        None => true,
    });
```

`next` reads the head of `ready_tasks`, so it inherits the exclusion.

In `prime`, take one snapshot and use it for the predicate, the summaries and both warnings:

```rust
    let claims = ClaimSnapshot::load(ctx.scope.prefixes().iter().map(String::as_str))?;
    let mut doing: Vec<Task> = all
        .iter()
        // Local `doing`, or a live claim: that is how a claim made in another worktree
        // surfaces here even though this checkout's file still reads `todo`.
        .filter(|task| task.status == Status::Doing || claims.live(&task.id).is_some())
        .cloned()
        .collect();
```

and after the uncommitted-files loop:

```rust
    for task in &all {
        if let Some(claim) = claims.live(&task.id)
            && matches!(task.status, Status::Todo | Status::Idea)
        {
            ctx.warnings.push(format!(
                "{} is claimed as doing in {} but this checkout's copy says {}; \
                 the two copies will conflict on merge",
                task.id,
                claim.worktree,
                task.status.as_str()
            ));
        }
    }
    for (id, claim, why) in claims.stale() {
        // Neither the `doing` predicate nor the divergence warning above covers a stale
        // claim, so without this it would appear nowhere.
        ctx.warnings.push(format!(
            "{id} has a stale claim from session {} ({why}); \
             `tasks start --force {id}` to take it over",
            claim.session
        ));
    }
```

Pass the snapshot's stores into `TaskSummary::of` at every call site in `list`, `ready`,
`next`, `prime` and `show`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands tests/cli.rs
git commit -m "feat(list): exclude claimed tasks from ready and overlay claims on prime"
```

---

### Task 10: Lock `feedback` recurrence

**Files:**
- Modify: `src/commands/feedback.rs` (the recurrence caller of `guarded_update`)
- Test: `tests/cli.rs`

`add`-style creation stays lock-free — `create_task` links the file into place exclusively,
so it has no read-modify-write to protect. Recurrence is different: `guarded_update`
(`feedback.rs:199`) re-reads the raw file and compares it against its snapshot
(`feedback.rs:213`), but the window between that comparison and `write_task`
(`feedback.rs:215`) is still a check-write race.

Only the **target** project's lock is taken. The source `Ctx` stays unlocked, so no command
ever holds two locks — which also keeps this correct when source and target are the same
project, as they are in this repository, since a second `flock` on the same file from the
same process would deadlock.

- [ ] **Step 1: Write the failing test**

`feedback` writes into the project registered under the `tasks` prefix, which
`feedback_env()` (`tests/cli.rs:2261`) sets up; a test that skips it has no target at all.

```rust
#[test]
fn feedback_recurrence_serializes_against_concurrent_recurrences() {
    let (env, target, reporter) = feedback_env();
    let first = env.json(
        &reporter,
        &["feedback", "the thing is slow", "--category", "friction"],
    );
    let id = first["id"].as_str().unwrap().to_string();

    let mut handles = Vec::new();
    for n in 0..4 {
        let mut cmd = env.cmd(&reporter);
        cmd.args([
            "feedback",
            "the thing is slow",
            "--category",
            "friction",
            "--recur",
            &id,
            "-b",
            &format!("detail {n}"),
        ]);
        handles.push(std::thread::spawn(move || cmd.output().unwrap()));
    }
    for handle in handles {
        let out = handle.join().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }

    // The report lives in the *target* project, not the reporter's.
    let raw = std::fs::read_to_string(target.join(format!("tasks/{id}.md"))).unwrap();
    for n in 0..4 {
        assert!(raw.contains(&format!("detail {n}")), "update {n} was lost: {raw}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `for i in $(seq 20); do cargo test --bin tasks --test cli feedback_recurrence_serializes || break; done`
Expected: FAIL on some iterations — a lost update, or `concurrent_modification` after eight
rounds. If 20 iterations all pass, raise the thread count rather than concluding the race is
absent.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/feedback.rs`, take the target lock before the first read of the recurrence
path and hold it until the update returns:

```rust
    // Recurrence is a read-modify-write, and `guarded_update`'s raw-content comparison
    // still leaves a window between the check and `write_task`. Only the target's lock is
    // needed; the source `Ctx` stays unlocked, so no command ever holds two.
    let _lock = crate::claims::MutationLock::acquire(&target.prefix)?;
    let task = guarded_update(target, &id, eligible, mutate)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `for i in $(seq 20); do cargo test --bin tasks --test cli feedback_recurrence_serializes || break; done`
Expected: PASS every iteration.

- [ ] **Step 5: Commit**

```bash
git add src/commands/feedback.rs tests/cli.rs
git commit -m "fix(feedback): lock the target project for recurrence updates"
```

---

### Task 11: The uncommitted-before-branching warning

**Files:**
- Modify: `src/commands/status.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Project::uncommitted_task_files` (`src/repo.rs:323`).

This is the narrower half of tasks-8f4b41 and covers only the *reverse* order — worktrees
already exist, then `start`. The order actually reported (start first, worktree created
afterwards) is covered by Task 9's divergence warning, which fires in the new worktree at the
moment the disagreement becomes observable, and is regression-tested in Task 12.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn start_warns_when_a_repo_with_several_worktrees_leaves_the_claim_uncommitted() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&sci)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap()
    };
    git(&["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "seed"]);

    let v = env.json(&sci, &["start", &id]);
    assert!(
        !v["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("worktree")),
        "a single-worktree repo has nothing to warn about: {v}"
    );
    git(&["add", "-A"]);
    git(&["commit", "-qm", "start"]);

    git(&["worktree", "add", "-q", "-b", "side", sci.join("wt").to_str().unwrap()]);
    let other = id_of(env.json(&sci, &["add", "U", "-p", "2"]));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "add U"]);

    let v = env.json(&sci, &["start", &other]);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&other) && w.contains("worktree")
        }),
        "{v}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tasks --test cli start_warns_when_a_repo_with_several_worktrees`
Expected: FAIL — no such warning.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/status.rs`:

```rust
/// Warns when `start` leaves the task file uncommitted in a repo that already has more than
/// one worktree. Every `start` leaves the file uncommitted — the protocol commits at `done`
/// — so warning unconditionally would be noise; the extra worktree is what makes it matter.
fn warn_if_uncommitted_with_worktrees(ctx: &mut Ctx, task: &Task) -> Result<()> {
    let Some(files) = ctx.project.uncommitted_task_files()? else {
        return Ok(()); // not a git checkout, or no git
    };
    let file = format!("tasks/{}.md", task.id);
    if !files.contains(&file) {
        return Ok(());
    }
    let listed = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .env("LC_ALL", "C")
        .current_dir(&ctx.project.root)
        .output()?;
    if !listed.status.success() {
        return Ok(());
    }
    let count = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| line.starts_with("worktree "))
        .count();
    if count > 1 {
        ctx.warnings.push(format!(
            "{file} is uncommitted and this repo has {count} worktrees; \
             commit it before branching or the copies diverge"
        ));
    }
    Ok(())
}
```

and call it in `start`, immediately before `Ok(id_out(ctx, &task))`:

```rust
    warn_if_uncommitted_with_worktrees(&mut ctx, &task)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --bin tasks --test cli start_warns_when_a_repo_with_several_worktrees`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/status.rs tests/cli.rs
git commit -m "feat(start): warn when a claim stays uncommitted in a multi-worktree repo"
```

---

### Task 12: Concurrency and failure regressions

The lock and the rollback are the two things most likely to be silently wrong, and neither
is covered by the happy-path tests above. This task adds only tests.

**Outcome-counting, not timing.** Each concurrency test asserts a property that holds under
*every* interleaving — exactly one winner, or no update lost — rather than hoping threads
collide. A test that merely launches two threads and hopes proves nothing when it passes.

**Files:**
- Modify: `tests/cli.rs`

- [ ] **Step 1: Write the tests**

```rust
#[test]
fn simultaneous_starts_produce_exactly_one_winner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let mut handles = Vec::new();
    for n in 0..6 {
        let mut cmd = as_agent(&env, &sci, &format!("agent-{n}"));
        cmd.args(["start", &id]);
        handles.push(std::thread::spawn(move || cmd.output().unwrap()));
    }
    let outs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let winners = outs.iter().filter(|o| o.status.success()).count();
    assert_eq!(winners, 1, "exactly one session may hold the claim");
    for out in outs.iter().filter(|o| !o.status.success()) {
        assert_eq!(err_kind(out), "claimed");
    }
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["live"],
        true,
        "and the winner's claim is the one that survived"
    );
}

#[test]
fn concurrent_claims_on_different_tasks_are_all_kept() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let ids: Vec<String> = (0..6)
        .map(|n| id_of(env.json(&sci, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();

    let mut handles = Vec::new();
    for (n, id) in ids.iter().enumerate() {
        let mut cmd = as_agent(&env, &sci, &format!("agent-{n}"));
        cmd.args(["start", id]);
        handles.push(std::thread::spawn(move || cmd.output().unwrap()));
    }
    for handle in handles {
        assert!(handle.join().unwrap().status.success());
    }

    // The store is one whole file per prefix, so an unserialized writer would drop the
    // claims it never read.
    for id in &ids {
        assert_eq!(
            env.json(&sci, &["show", id])["claim"]["live"],
            true,
            "{id} lost its claim to a concurrent write"
        );
    }
}

#[test]
fn concurrent_notes_and_a_status_change_lose_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let mut handles = Vec::new();
    for n in 0..5 {
        let mut cmd = as_agent(&env, &sci, "agent-a");
        cmd.args(["note", &id, &format!("line {n}")]);
        handles.push(std::thread::spawn(move || cmd.output().unwrap()));
    }
    let mut status = as_agent(&env, &sci, "agent-a");
    status.args(["start", &id]);
    handles.push(std::thread::spawn(move || status.output().unwrap()));

    for handle in handles {
        assert!(handle.join().unwrap().status.success());
    }

    // `note` rewrites the whole markdown file, so an unserialized note clobbers whatever
    // landed between its read and its write.
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    for n in 0..5 {
        assert!(raw.contains(&format!("line {n}")), "note {n} was lost: {raw}");
    }
    assert!(raw.contains("status: doing"), "the status change was lost: {raw}");
}

#[test]
fn a_concurrent_edit_during_an_interactive_edit_is_rejected_and_leaks_no_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // An EDITOR that sets the status to doing, slowly, so another writer lands first.
    let script = sci.join("slow-editor.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nsleep 1\nsed -i 's/^status: todo/status: doing/' \"$1\"\n",
    )
    .unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(
        &mut std::fs::metadata(&script).unwrap().permissions().clone(),
        0o755,
    );
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut editing = as_agent(&env, &sci, "agent-a");
    editing.args(["edit", &id]).env("EDITOR", script.to_str().unwrap());
    let editor = std::thread::spawn(move || editing.output().unwrap());

    std::thread::sleep(std::time::Duration::from_millis(300));
    as_agent(&env, &sci, "agent-b").args(["note", &id, "landed first"]).assert().success();

    let out = editor.join().unwrap();
    assert_eq!(err_kind(&out), "concurrent_modification");
    // The editor's `transition` ran before the comparison. Because no claim is persisted
    // until `save`, the rejected edit must not have left one behind.
    assert!(
        env.json(&sci, &["show", &id])["claim"].is_null(),
        "a rejected edit acquired a claim"
    );
}

#[test]
fn a_failed_task_write_leaves_no_claim_behind() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // Read still works; `atomic_write` cannot create its temp file.
    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let out = as_agent(&env, &sci, "agent-a").args(["start", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));

    std::fs::set_permissions(&tasks_dir, original).unwrap();
    assert!(
        env.json(&sci, &["show", &id])["claim"].is_null(),
        "acquire is rolled back when the task write fails"
    );
}

#[test]
fn a_failed_takeover_restores_the_previous_owners_claim() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();

    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-b").args(["start", "--force", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    std::fs::set_permissions(&tasks_dir, original).unwrap();

    // Rollback restores what was there; a blanket removal would unclaim A's live work.
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-a",
        "a failed takeover must not unclaim the previous holder"
    );
}

#[test]
fn the_reported_sequence_start_then_create_the_worktree() {
    let mut env = TestEnv::new();
    let a = env.init("sci");
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));

    // The bytes a later worktree would branch from: captured *before* the claim exists.
    let committed = env.read(&a, &format!("tasks/{id}.md"));
    as_agent(&env, &a, "agent-a").args(["start", &id]).assert().success();

    // Only now does the second worktree come into being, from the pre-start state.
    let b = env.init_forced("sci");
    std::fs::write(b.join(format!("tasks/{id}.md")), &committed).unwrap();

    let v = env.json(&b, &["prime"]);
    assert!(
        v["doing"].as_array().unwrap().iter().any(|t| t["id"] == id.as_str()),
        "the claim is visible in a worktree created after the start: {v}"
    );
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("conflict")
        }),
        "and the divergence is called out: {v}"
    );
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --bin tasks --test cli -- simultaneous concurrent a_failed a_concurrent_edit the_reported_sequence`
Expected: PASS. Then repeat, since concurrency tests that pass once prove less than
concurrency tests that pass twenty times:
`for i in $(seq 20); do cargo test --bin tasks --test cli -- simultaneous concurrent || break; done`

- [ ] **Step 3: Commit**

```bash
git add tests/cli.rs
git commit -m "test(claims): cover simultaneous claims, note races, and rollback"
```

---

### Task 13: Documentation and closeout

**Files:**
- Modify: `skills/tasks/SKILL.md`, `AGENTS.md`, `docs/specs/2026-08-29-tasks-design.md`, `docs/specs/2026-09-05-work-claims-design.md`

- [ ] **Step 1: Document the claim protocol in the shipped skill**

`skills/tasks/SKILL.md` goes out to other projects, so it is where agents learn this. Add
after the `tasks start` step of the session protocol:

```markdown
- `tasks start` records a claim outside git — visible from every worktree of the project,
  carrying your session identity and a liveness handle. Starting a task another live session
  holds fails with `claimed`; `tasks start --force <id>` takes it over and records the
  takeover in the task's notes. `ready` and `next` omit claimed tasks and say so in their
  warnings.
- Set `TASKS_SESSION` (and `TASKS_SESSION_PID`, if you have a long-lived process id) when
  several agents share one terminal or one harness process. Without it, agents that share a
  session id are indistinguishable to the claim store — which is the case this exists to
  prevent.
```

- [ ] **Step 2: Note the new module in AGENTS.md**

Under `## Layout`, in the `src/` list:

```markdown
  `claims.rs` (out-of-git work claims: the per-prefix store, liveness, the mutation lock),
```

- [ ] **Step 3: Correct the design doc of record**

`docs/specs/2026-08-29-tasks-design.md:112` describes `owner` as the whole claim story. Add
a pointer so the two docs do not drift:

```markdown
| `owner`    | string              | no       | Advisory claim; set by `start`; `[A-Za-z0-9._/@+-]+`. Session identity and liveness live outside git — see `2026-09-05-work-claims-design.md`. |
```

- [ ] **Step 4: Correct this design's own status header**

In `docs/specs/2026-09-05-work-claims-design.md`, set the header to what actually landed and
re-read "Known gaps" to confirm each entry is still true:

```markdown
Status: implemented (2026-09-05)
```

- [ ] **Step 5: Reinstall, run the full gate, and close the tasks**

```bash
cargo install --path .
just gate
tasks done tasks-8f4b41 "start's claim is visible from every worktree; the divergence warning landed, the merge conflict itself is a recorded gap"
tasks done tasks-d184e3 "claims moved out of git: per-prefix store, session identity, pid liveness, mutation lock, ready/prime overlay"
```

- [ ] **Step 6: Commit**

```bash
git add skills AGENTS.md docs tasks
git commit -m "docs(claims): document the claim protocol and close the reports"
```

---

## Self-Review

**Spec coverage:** Store and prefix keying → Task 1. Liveness → Task 2. Locking → Tasks 3, 4,
10. Identity → Task 5. JSON contract → Task 6. Command behaviour, destination-based release,
write ordering, pruning → Task 7. `start --force` → Task 8. `ready`/`prime` and all three
warnings → Task 9. `feedback` recurrence → Task 10. The tasks-8f4b41 warning → Task 11.
Concurrency and rollback → Task 12. Docs and the "Known gaps" recheck → Task 13. The three
known gaps are deliberately unimplemented and recorded as such.

**Ordering:** Task 6 (output shapes) precedes Task 7 (the guard) so the guard's tests have a
contract to assert against; it is testable on its own through a hand-written store fixture.
No task commits a test that a later task is required to make pass, except the two `--force`
tests in Task 7, which Step 4 flags explicitly.

**Type consistency:** `ClaimStore::{path_with, path_for, load, load_from, save, get, insert,
remove, prune_with, prune_dead, iter}`, `Claim`, `Liveness::{Live, Stale}`,
`ProcStat::{NotFound, Unreadable, Found}`, `parse_proc_stat`, `Identity {session, pid}`,
`identity() -> Result<Identity>`, `MutationLock::{path_with, path_for, acquire, acquire_at}`,
`ClaimInfo::of`, `ClaimSnapshot::{load, live, stale, stores}`, `ClaimIntent::{Acquire,
Release}`, `Error::Claimed` are used under those exact names throughout. `TaskSummary::of`
takes three parameters from Task 6 onward. `transition` and `save` take `&mut Ctx` from Task
7 onward. Test helpers `write_claim`, `two_roots`, `as_agent`, `err_kind`, `err_detail` are
defined in Tasks 6, 7 and 7 respectively and used thereafter; `TestEnv::{claim_store,
init_forced}` in Tasks 1 and 7.
