# Work Claims Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `doing` claims cross-worktree visibility, agent identity, and liveness, by moving the claim out of git into a per-prefix state store guarded by a per-project mutation lock.

**Architecture:** A new `src/claims.rs` owns the claim record, its TOML store at `$XDG_STATE_HOME/tasks/claims/<prefix>.toml`, a `/proc`-based liveness test, and a `<prefix>.lock` mutation lock. The store is keyed by *project prefix*, which a worktree and its main checkout share while their roots differ — that is what makes claims visible across worktrees with no worktree enumeration. The guard lives in `transition()`/`save()` in `commands/mod.rs`, the chokepoint that `start`, `close`, `edit --status` and the interactive editor all already pass through.

**Tech Stack:** Rust 2024, clap 4.6, serde/toml, `std::fs::File::lock`, `/proc`. No new dependencies.

**Spec:** `docs/specs/2026-09-05-work-claims-design.md`

## Global Constraints

- **No new dependencies.** `File::lock` requires rustc ≥ 1.89; the project is on 1.98.1.
- **JSON output is the contract.** Every change here is additive: new optional fields, one new error kind. Never change an existing shape.
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
- Consumes: `crate::error::{Error, Result}`, `crate::model::TaskId`, `crate::repo::atomic_write`, `crate::time`.
- Produces: `Claim`, `ClaimStore`, `ClaimStore::{path_for, load, save, get, insert, remove, prune_with}`.

- [ ] **Step 1: Write the failing test**

In a new `src/claims.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn store_roundtrips_and_is_keyed_by_prefix() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("claims/sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();

        let mut store = ClaimStore::load_from(&path).unwrap();
        assert!(store.get(&id).is_none(), "absent store loads empty");
        store.insert(&id, sample());
        store.save().unwrap();

        let reloaded = ClaimStore::load_from(&path).unwrap();
        assert_eq!(reloaded.get(&id).unwrap().session, "s1");
        assert_eq!(reloaded.get(&id).unwrap().pid, Some(42));
    }

    #[test]
    fn remove_reports_whether_anything_was_there() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("claims/sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();
        let mut store = ClaimStore::load_from(&path).unwrap();
        store.insert(&id, sample());
        assert!(store.remove(&id).is_some());
        assert!(store.remove(&id).is_none());
    }

    #[test]
    fn path_prefers_xdg_state_home_then_home() {
        unsafe { std::env::set_var("XDG_STATE_HOME", "/xdg") };
        assert_eq!(
            ClaimStore::path_for("sci").unwrap(),
            std::path::PathBuf::from("/xdg/tasks/claims/sci.toml")
        );
        unsafe { std::env::remove_var("XDG_STATE_HOME") };
        unsafe { std::env::set_var("HOME", "/home/x") };
        assert_eq!(
            ClaimStore::path_for("sci").unwrap(),
            std::path::PathBuf::from("/home/x/.local/state/tasks/claims/sci.toml")
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib claims::`
Expected: FAIL — `cannot find type Claim` / `ClaimStore`.

- [ ] **Step 3: Write minimal implementation**

At the top of `src/claims.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One session's advisory hold on a task. Lives outside git deliberately: a claim is
/// ephemeral machine state, and putting it on a branch is what made `doing` invisible
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
    /// state rather than config: a claim is disposable, and losing the file costs only
    /// the overlay, never a task.
    pub fn path_for(prefix: &str) -> Result<PathBuf> {
        let base = if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join(format!("tasks/claims/{prefix}.toml")))
    }

    pub fn load(prefix: &str) -> Result<ClaimStore> {
        Self::load_from(&Self::path_for(prefix)?)
    }

    pub fn load_from(path: &Path) -> Result<ClaimStore> {
        let claims = if path.exists() {
            let text = std::fs::read_to_string(path)?;
            toml::from_str::<StoreFile>(&text)
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
    /// exercised deterministically in tests; `prune_dead` in Task 2 is the real caller.
    pub fn prune_with(&mut self, keep: impl Fn(&Claim) -> bool) {
        self.claims.retain(|_, claim| keep(claim));
    }

    /// Every claimed id paired with its claim, for the read paths.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Claim)> {
        self.claims.iter()
    }
}
```

Register the module in `src/main.rs` alongside the existing `mod` lines:

```rust
mod claims;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib claims::`
Expected: PASS (3 tests).

Note: `path_prefers_xdg_state_home_then_home` mutates process env, which is shared across
threads. Run it serially if it proves flaky: `cargo test --lib claims:: -- --test-threads=1`.

- [ ] **Step 5: Isolate the test environment**

Every end-to-end test must be prevented from touching the developer's real state
directory or inheriting a real agent identity. In `tests/common/mod.rs`, extend the
`env_remove` chain in `cmd` (currently `tests/common/mod.rs:22-28`):

```rust
    pub fn cmd(&self, dir: &Path) -> Command {
        let mut c = Command::cargo_bin("tasks").unwrap();
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
        c
    }
```

`HOME` already points at a temp dir, so with `XDG_STATE_HOME` removed the claim store
lands under `$HOME/.local/state/tasks/claims/` inside that temp dir automatically.

- [ ] **Step 6: Verify isolation and commit**

Run: `just check && cargo test`
Expected: PASS, and no file appears under your real `~/.local/state/tasks/`.

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
- Produces: `Liveness`, `ProcStat`, `liveness(claim: &Claim) -> Liveness`, `liveness_with(claim, now: OffsetDateTime, boot_id: Option<&str>, stat: impl Fn(u32) -> ProcStat) -> Liveness`, `ClaimStore::prune_dead`, `TTL_HOURS`.

The `_with` suffix follows the codebase's existing injection idiom (`atomic_write_with`,
`create_task_with` in `src/repo.rs`).

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src/claims.rs`:

```rust
    fn at(s: &str) -> time::OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn claim_seen(seen: &str) -> Claim {
        Claim {
            seen: seen.into(),
            ..sample()
        }
    }

    #[test]
    fn a_confirmed_live_process_beats_any_ttl() {
        let c = claim_seen("2026-09-01T00:00:00Z"); // days stale by the clock
        let live = |_| ProcStat::Found {
            state: 'S',
            starttime: 999,
        };
        assert_eq!(
            liveness_with(&c, at("2026-09-05T00:00:00Z"), Some("boot-a"), live),
            Liveness::Live,
            "an agent that thinks for days is not dead"
        );
    }

    #[test]
    fn a_confirmed_dead_pid_is_stale_at_once_with_no_grace() {
        let c = claim_seen("2026-09-05T00:00:00Z"); // seen one second ago
        let gone = |_| ProcStat::NotFound;
        assert!(matches!(
            liveness_with(&c, at("2026-09-05T00:00:01Z"), Some("boot-a"), gone),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn a_recycled_pid_does_not_resurrect_a_claim() {
        let c = claim_seen("2026-09-05T00:00:00Z");
        let recycled = |_| ProcStat::Found {
            state: 'S',
            starttime: 12345, // != claim.pid_start
        };
        assert!(matches!(
            liveness_with(&c, at("2026-09-05T00:00:01Z"), Some("boot-a"), recycled),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn a_reboot_invalidates_the_pid_and_starttime_pair() {
        let c = claim_seen("2026-09-05T00:00:00Z");
        let live = |_| ProcStat::Found {
            state: 'S',
            starttime: 999,
        };
        assert!(
            matches!(
                liveness_with(&c, at("2026-09-05T00:00:01Z"), Some("boot-b"), live),
                Liveness::Stale(_)
            ),
            "starttime counts ticks since boot, so the pair means nothing across boots"
        );
    }

    #[test]
    fn a_zombie_is_dead() {
        let c = claim_seen("2026-09-05T00:00:00Z");
        let zombie = |_| ProcStat::Found {
            state: 'Z',
            starttime: 999,
        };
        assert!(matches!(
            liveness_with(&c, at("2026-09-05T00:00:01Z"), Some("boot-a"), zombie),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn unreadable_proc_falls_to_ttl_rather_than_reading_as_death() {
        let unreadable = |_| ProcStat::Unreadable;
        let fresh = claim_seen("2026-09-05T00:00:00Z");
        assert_eq!(
            liveness_with(&fresh, at("2026-09-05T03:59:00Z"), Some("boot-a"), unreadable),
            Liveness::Live,
            "permission failure is not proof of death"
        );
        let old = claim_seen("2026-09-05T00:00:00Z");
        assert!(matches!(
            liveness_with(&old, at("2026-09-05T04:00:01Z"), Some("boot-a"), unreadable),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn an_unreadable_boot_id_and_a_pidless_claim_both_fall_to_ttl() {
        let live = |_| ProcStat::Found {
            state: 'S',
            starttime: 999,
        };
        let c = claim_seen("2026-09-05T00:00:00Z");
        assert_eq!(
            liveness_with(&c, at("2026-09-05T01:00:00Z"), None, live),
            Liveness::Live
        );
        let pidless = Claim {
            pid: None,
            ..claim_seen("2026-09-05T00:00:00Z")
        };
        assert!(matches!(
            liveness_with(&pidless, at("2026-09-05T05:00:00Z"), Some("boot-a"), live),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn prune_dead_keeps_the_live_and_drops_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let mut store = ClaimStore::load_from(&home.path().join("sci.toml")).unwrap();
        let alive = TaskId::parse("sci-000001").unwrap();
        let dead = TaskId::parse("sci-000002").unwrap();
        store.insert(&alive, sample());
        store.insert(&dead, Claim { pid: Some(43), ..sample() });
        store.prune_with(|claim| claim.pid == Some(42));
        assert!(store.get(&alive).is_some());
        assert!(store.get(&dead).is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib claims::`
Expected: FAIL — `cannot find value liveness_with` / `ProcStat`.

- [ ] **Step 3: Write minimal implementation**

Append to `src/claims.rs`:

```rust
/// Hours a claim whose liveness cannot be established stays live. It applies *only* on
/// the unverifiable path: a confirmed-live process outlives it, and a confirmed-dead one
/// gets no grace at all.
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

/// Reads `/proc/<pid>/stat`. The `comm` field can contain spaces and parentheses, so the
/// fields are taken from after the *last* `)`: in that remainder state is field 1 and
/// starttime is field 20 (fields 3 and 22 overall).
pub fn proc_stat(pid: u32) -> ProcStat {
    let text = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ProcStat::NotFound,
        Err(_) => return ProcStat::Unreadable,
    };
    let Some((_, rest)) = text.rsplit_once(") ") else {
        return ProcStat::Unreadable;
    };
    let fields: Vec<&str> = rest.split_whitespace().collect();
    match (fields.first(), fields.get(19)) {
        (Some(state), Some(starttime)) => match (state.chars().next(), starttime.parse()) {
            (Some(state), Ok(starttime)) => ProcStat::Found { state, starttime },
            _ => ProcStat::Unreadable,
        },
        _ => ProcStat::Unreadable,
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
        // An unparsable timestamp is corruption, not liveness evidence; treat it as gone
        // rather than pinning the task forever.
        Err(_) => Liveness::Stale(format!("unreadable timestamp {:?}", claim.seen)),
    };

    let Some(pid) = claim.pid else { return ttl() };
    match (boot_id, &claim.boot_id) {
        // On a different boot the pid/starttime pair is meaningless, so it is not
        // "unverifiable" — it is positively dead.
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
        ProcStat::Found { starttime, .. } if Some(starttime) != claim.pid_start => {
            Liveness::Stale(format!("pid {pid} was reused by another process"))
        }
        ProcStat::Found { .. } => Liveness::Live,
    }
}

impl ClaimStore {
    pub fn prune_dead(&mut self) {
        self.prune_with(|claim| liveness(claim) == Liveness::Live);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib claims::`
Expected: PASS (11 tests).

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
- Produces: `MutationLock`, `MutationLock::acquire(prefix: &str) -> Result<MutationLock>`, `MutationLock::path_for(prefix: &str) -> Result<PathBuf>`.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module:

```rust
    #[test]
    fn the_lock_serializes_two_writers() {
        use std::sync::{Arc, Barrier};
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", home.path()) };

        let lock = MutationLock::acquire("sci").unwrap();
        let start = Arc::new(Barrier::new(2));
        let other = {
            let start = Arc::clone(&start);
            let home = home.path().to_path_buf();
            std::thread::spawn(move || {
                unsafe { std::env::set_var("XDG_STATE_HOME", &home) };
                start.wait();
                let _held = MutationLock::acquire("sci").unwrap();
                std::time::Instant::now()
            })
        };
        start.wait();
        std::thread::sleep(std::time::Duration::from_millis(120));
        let released = std::time::Instant::now();
        drop(lock);

        let acquired = other.join().unwrap();
        assert!(
            acquired >= released,
            "the second writer must not enter before the first leaves"
        );
        unsafe { std::env::remove_var("XDG_STATE_HOME") };
    }

    #[test]
    fn the_lock_is_a_separate_file_from_the_store() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", home.path()) };
        assert_ne!(
            MutationLock::path_for("sci").unwrap(),
            ClaimStore::path_for("sci").unwrap()
        );
        unsafe { std::env::remove_var("XDG_STATE_HOME") };
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib claims:: -- --test-threads=1`
Expected: FAIL — `cannot find type MutationLock`.

- [ ] **Step 3: Write minimal implementation**

Append to `src/claims.rs`:

```rust
/// Serializes every read-modify-write against one project: the task markdown files *and*
/// the claim store together.
///
/// It has to span both. Checking ownership in `transition` and writing in `save` are
/// separate steps, so a lock over the claim store alone leaves a takeover race in the gap
/// between them. And `note`, whatever its append-only meaning, rewrites the whole markdown
/// file at the storage layer, so an unserialized note can clobber a concurrent status
/// change.
///
/// The lock is its own file because `atomic_write` replaces the store's inode by rename:
/// locking the store itself would leave each writer holding a lock on a file that is no
/// longer at that path. A process that dies holding it has it released by the kernel, so
/// there is no stale-lock recovery path.
#[derive(Debug)]
pub struct MutationLock {
    _file: std::fs::File,
}

impl MutationLock {
    pub fn path_for(prefix: &str) -> Result<PathBuf> {
        Ok(ClaimStore::path_for(prefix)?.with_file_name(format!("{prefix}.lock")))
    }

    pub fn acquire(prefix: &str) -> Result<MutationLock> {
        let path = Self::path_for(prefix)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        file.lock()
            .map_err(|error| Error::Io(format!("locking {}: {error}", path.display())))?;
        Ok(MutationLock { _file: file })
    }
}
```

The lock releases when `MutationLock` drops, because dropping the `File` closes the
descriptor.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib claims:: -- --test-threads=1`
Expected: PASS (13 tests).

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs
git commit -m "feat(claims): add the per-project mutation lock"
```

---

### Task 4: Wire the lock into Ctx without locking the read commands

**Files:**
- Modify: `src/commands/mod.rs:29-41` (`Ctx`, `open_ctx`), `src/commands/mod.rs:255-346` (`run`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `MutationLock` from Task 3.
- Produces: `Ctx { project, registry, warnings, lock: Option<MutationLock>, claims: Option<ClaimStore> }`, `open_write_ctx(dir: Option<&Path>) -> Result<Ctx>`, `Ctx::claims_mut(&mut self) -> Result<&mut ClaimStore>`.

`show`, `graph`, `check`, `add` and `feedback` keep the unlocked `open_ctx`. The eight
write paths move to `open_write_ctx`.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`:

```rust
#[test]
fn read_commands_do_not_take_the_mutation_lock() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // The lock file only exists once a write command has taken it.
    let lock = env
        .home
        .path()
        .join(".local/state/tasks/claims/sci.lock");
    env.json(&sci, &["show", &id]);
    env.json(&sci, &["check"]);
    assert!(!lock.exists(), "read commands must not create the lock");

    env.json(&sci, &["start", &id]);
    assert!(lock.exists(), "a write command takes the lock");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli read_commands_do_not_take`
Expected: FAIL — the lock file never appears, because nothing takes it yet.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/mod.rs`, extend `Ctx` and add the locked constructor:

```rust
use crate::claims::{ClaimStore, MutationLock};

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for the life of a write command; `None` for the read commands that also take
    /// a `Ctx` (`show`, `graph`, `check`) and for the two create-only paths.
    pub lock: Option<MutationLock>,
    claims: Option<ClaimStore>,
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    Ok(Ctx {
        project: Project::locate(&start)?,
        registry: Registry::load()?,
        warnings: Vec::new(),
        lock: None,
        claims: None,
    })
}

/// `open_ctx` plus the project's mutation lock, held until the command ends.
///
/// The lock cannot simply live in `open_ctx`: `show`, `graph` and `check` take a `Ctx`
/// too and must stay read-only, and giving them an exclusive lock would have them block
/// on writers and on each other. Readers need no shared lock either — `atomic_write`
/// publishes by rename, so a reader sees one whole version or another, never a torn one.
pub fn open_write_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let mut ctx = open_ctx(dir)?;
    ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
    Ok(ctx)
}

impl Ctx {
    /// The claim store, loaded on first use. Only reachable with the lock held, so every
    /// read-check-write against it is inside one critical section.
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

Then in `run`, switch exactly the eight write paths (leave `Show`, `Graph`, `Check`,
`Add`, `Feedback` on `open_ctx`):

```rust
        } => edit::run(open_write_ctx(dir)?, id, title, status, force, no_parent, fields),
        Command::Prime { all_projects } => list::prime(open_read_ctx(dir, all_projects)?),
        Command::Note { id, text } => status::note(open_write_ctx(dir)?, id, text),
        Command::Start { id } => status::start(open_write_ctx(dir)?, id),
        Command::Done { id, message, force } => {
            status::close(open_write_ctx(dir)?, id, Status::Done, message, force)
        }
        Command::Drop { id, message } => {
            status::close(open_write_ctx(dir)?, id, Status::Dropped, message, false)
        }
        Command::Block { id, message } => status::block(open_write_ctx(dir)?, id, message),
        Command::Unblock { id } => status::unblock(open_write_ctx(dir)?, id),
        Command::Dep { id, on, rm } => dep::run(open_write_ctx(dir)?, id, on, rm),
```

Add the `Ctx { .. }` literal in the `Command::Add` arm (`src/commands/mod.rs:280-285`) the
two new fields:

```rust
                    Ctx {
                        project,
                        registry,
                        warnings: Vec::new(),
                        lock: None,
                        claims: None,
                    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli read_commands_do_not_take`
Expected: PASS. The `start` at the end creates the lock file even though it does not yet
write a claim.

- [ ] **Step 5: Release the lock around the interactive editor**

In `src/commands/edit.rs`, `editor()` waits on `$EDITOR`, which can be a human's minutes.
Drop the lock before spawning and re-acquire before validating. The existing raw-content
comparison at `edit.rs:134-142` already covers the unlocked window with
`concurrent_modification`, so no new mechanism is needed. Immediately before the
`std::process::Command::new("sh")` call:

```rust
    // The lock must not span a human's editing session; the raw-content comparison below
    // is what protects the gap.
    let prefix = ctx.project.prefix.clone();
    ctx.lock = None;
```

and immediately after the `if !status.success()` block:

```rust
    ctx.lock = Some(crate::claims::MutationLock::acquire(&prefix)?);
```

`editor` takes `mut ctx: Ctx` already, so no signature change is needed.

- [ ] **Step 6: Verify and commit**

Run: `just check && cargo test`
Expected: PASS.

```bash
git add src/commands/mod.rs src/commands/edit.rs tests/cli.rs
git commit -m "feat(claims): hold a mutation lock for write commands only"
```

---

### Task 5: Session identity and the `claimed` error kind

**Files:**
- Modify: `src/claims.rs`, `src/error.rs:3-37` (variants), `src/error.rs:74-90` (`kind`), `src/error.rs:47-71` (`with_suffix`)
- Test: `src/claims.rs`

**Interfaces:**
- Produces: `Identity { session: String, pid: Option<u32> }`, `identity() -> Identity`, `identity_from(get: impl Fn(&str) -> Option<String>, session_pid: Option<u32>) -> Identity`, `Error::Claimed(String, String)` with kind `"claimed"`.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src/claims.rs`:

```rust
    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn identity_prefers_the_explicit_pair() {
        let id = identity_from(
            &env_of(&[
                ("TASKS_SESSION", "explicit"),
                ("TASKS_SESSION_PID", "7"),
                ("CLAUDE_CODE_SESSION_ID", "claude"),
                ("CLAUDE_PID", "9"),
            ]),
            Some(11),
        );
        assert_eq!(id.session, "explicit");
        assert_eq!(id.pid, Some(7));
    }

    #[test]
    fn a_level_never_borrows_another_levels_pid() {
        // Claude Code old enough to export the session but not CLAUDE_PID.
        let id = identity_from(&env_of(&[("CLAUDE_CODE_SESSION_ID", "claude")]), Some(11));
        assert_eq!(id.session, "claude");
        assert_eq!(
            id.pid, None,
            "a session without its own pid must not be welded to the fallback pid"
        );
    }

    #[test]
    fn falls_back_to_the_unix_session_id() {
        let id = identity_from(&env_of(&[]), Some(11));
        assert_eq!(id.session, "sid:11");
        assert_eq!(id.pid, Some(11));
    }

    #[test]
    fn an_empty_env_var_does_not_count_as_set() {
        let id = identity_from(&env_of(&[("TASKS_SESSION", "")]), Some(11));
        assert_eq!(id.session, "sid:11");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib claims::`
Expected: FAIL — `cannot find function identity_from`.

- [ ] **Step 3: Write minimal implementation**

Append to `src/claims.rs`:

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

pub fn identity() -> Identity {
    identity_from(
        |key| std::env::var(key).ok().filter(|value| !value.is_empty()),
        unix_session_id(),
    )
}

/// Session and pid are resolved as a **pair from a single level**. A level that yields a
/// session but no usable pid yields `pid: None` and falls to the TTL path; it is never
/// welded to an unrelated fallback pid.
///
/// Level 2 names Claude Code's variables because `skills/tasks/SKILL.md` ships to agent
/// projects, so agents are a first-class consumer and this makes claims work with no
/// setup. `CLAUDE_PID` needs a recent Claude Code; without it level 2 still supplies the
/// session.
///
/// Level 3 is *terminal* identity, not agent identity: several agents can share one
/// terminal, and a terminal outlives the agent that ran in it. Subagents inside one Claude
/// Code process likewise share level 2. `TASKS_SESSION` is the escape hatch for both.
pub fn identity_from(get: impl Fn(&str) -> Option<String>, session_pid: Option<u32>) -> Identity {
    let pid_of = |key: &str| get(key).and_then(|value| value.parse().ok());
    if let Some(session) = get("TASKS_SESSION") {
        return Identity {
            session,
            pid: pid_of("TASKS_SESSION_PID"),
        };
    }
    if let Some(session) = get("CLAUDE_CODE_SESSION_ID") {
        return Identity {
            session,
            pid: pid_of("CLAUDE_PID"),
        };
    }
    Identity {
        session: match session_pid {
            Some(pid) => format!("sid:{pid}"),
            None => "unknown".into(),
        },
        pid: session_pid,
    }
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}
```

In `src/error.rs`, add the variant next to the other typed failures:

```rust
    #[error("{0} is claimed by {1}")]
    Claimed(String, String),
```

its `kind`:

```rust
            Error::Claimed(..) => "claimed",
```

and its `with_suffix` arm:

```rust
            Error::Claimed(id, detail) => Error::Claimed(id, detail + suffix),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib claims::`
Expected: PASS (17 tests).

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/error.rs
git commit -m "feat(claims): resolve session identity as a pair and add the claimed error"
```

---

### Task 6: The guard, acquire, and destination-based release

This is the core task. It is one unit because the guard, the acquire and the release are a
single transaction — a reviewer cannot sensibly accept one without the others.

**Files:**
- Modify: `src/commands/mod.rs` (`transition`, `save`, new `Ctx` claim helpers), `src/commands/status.rs` (`note` heartbeat)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Ctx::claims_mut` (Task 4), `identity`, `liveness`, `Liveness`, `Claim`, `hostname` (Tasks 1-5).
- Produces: `transition(ctx: &mut Ctx, task: &mut Task, to: Status, force: bool) -> Result<()>` (now `&mut Ctx`), `save(ctx: &mut Ctx, task: &mut Task) -> Result<()>` (now `&mut Ctx`), `Ctx::describe_claim(&Claim, &Liveness) -> String`.

`transition` and `save` change from `&Ctx` to `&mut Ctx`. Every caller in
`src/commands/{status,edit,add,dep}.rs` needs `mut ctx` — mechanical, and the compiler
lists them all.

- [ ] **Step 1: Write the failing tests**

Add to `tests/cli.rs`. `two_roots` gives two temp projects sharing one prefix, which is
what a main checkout and a worktree look like to the claim store:

```rust
/// Two project roots sharing one prefix: what a main checkout and a worktree look like to
/// a store keyed by prefix. The second `init` needs `--force` because the registry refuses
/// to re-point a prefix silently.
fn two_roots(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf) {
    let a = env.init("sci");
    let dir = tempfile::tempdir().unwrap();
    let b = dir.path().canonicalize().unwrap();
    std::mem::forget(dir); // keep it alive for the test process
    env.json(&b, &["init", "--prefix", "sci", "--force"]);
    (a, b)
}

#[test]
fn a_live_claim_from_another_session_refuses_start() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    env.cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &std::process::id().to_string())
        .assert()
        .success();

    let out = env
        .cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-b")
        .env("TASKS_SESSION_PID", &std::process::id().to_string())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "claimed");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("agent-a"),
        "the refusal names the holder: {v}"
    );
}

#[test]
fn a_displaced_session_cannot_close_the_task_it_lost() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    let a = |args: &[&str]| {
        env.cmd(&sci)
            .args(args)
            .env("TASKS_SESSION", "agent-a")
            .env("TASKS_SESSION_PID", &pid)
            .output()
            .unwrap()
    };
    let b = |args: &[&str]| {
        env.cmd(&sci)
            .args(args)
            .env("TASKS_SESSION", "agent-b")
            .env("TASKS_SESSION_PID", &pid)
            .output()
            .unwrap()
    };

    assert!(a(&["start", &id]).status.success());
    assert!(b(&["start", "--force", &id]).status.success());

    for args in [
        vec!["done", &id, "landed"],
        vec!["drop", &id, "nope"],
        vec!["block", &id, "waiting"],
        vec!["edit", &id, "--status", "done"],
    ] {
        let out = a(&args);
        assert_eq!(out.status.code(), Some(1), "A must not close {args:?}");
        let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(v["error"]["kind"], "claimed", "{args:?}");
    }

    // B's claim survived every one of A's attempts.
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["fields"]["claim"]["session"], "agent-b");
}

#[test]
fn release_follows_the_claim_not_the_local_doing_status() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    // Root A claims it. Root B's copy of the file is untouched and still reads `todo` —
    // the ordinary cross-worktree case.
    env.cmd(&a)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md")))
        .ok();
    std::fs::write(
        b.join(format!("tasks/{id}.md")),
        env.read(&a, &format!("tasks/{id}.md")).replace("status: doing", "status: todo"),
    )
    .unwrap();

    // The same session closes it from root B, where the local status was never `doing`.
    env.cmd(&b)
        .args(["done", &id, "landed"])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    let v = env.json(&a, &["show", &id]);
    assert!(
        v["fields"]["claim"].is_null(),
        "the claim must be released even though this checkout never left doing: {v}"
    );
}

#[test]
fn a_stale_local_done_does_not_prune_a_live_claim() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    // Root B holds a live claim.
    env.cmd(&b)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-b")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    // Root A's copy is an older `done` — a branch that closed the task before B reopened it.
    env.json(&a, &["edit", &id, "--status", "done"]);

    // A write from root A must not treat its own view as authority over the shared claim.
    env.json(&a, &["edit", &id, "-p", "1"]);
    let v = env.json(&b, &["show", &id]);
    assert_eq!(
        v["fields"]["claim"]["session"], "agent-b",
        "one checkout's view cannot establish that a shared claim is obsolete: {v}"
    );
}

#[test]
fn re_running_a_close_retries_the_release() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();
    let session = "agent-a";

    env.cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    // Simulate a release that failed after the file write: the task is closed, the claim
    // is still there. `start --force` cannot recover this — `done -> doing` is rejected.
    let store = env.home.path().join(".local/state/tasks/claims/sci.toml");
    std::fs::write(
        &store,
        format!(
            "[claims.\"{id}\"]\nowner = \"tester\"\nsession = \"{session}\"\npid = {pid}\n\
             host = \"h\"\nworktree = \"/tmp\"\nstarted = \"2026-09-05T00:00:00Z\"\n\
             seen = \"2026-09-05T00:00:00Z\"\n"
        ),
    )
    .unwrap();
    assert_eq!(env.fail(&sci, &["start", "--force", &id]), "invalid_transition");

    // Re-running the original closing command is the recovery: a same-status transition
    // still attempts the release.
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["fields"]["claim"].is_null(), "the retry released it: {v}");
}

#[test]
fn a_stale_claim_is_taken_over_with_a_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // pid 0 never names a live process, so this claim is confirmed dead, not merely old.
    let store = env.home.path().join(".local/state/tasks/claims/sci.toml");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(
        &store,
        format!(
            "[claims.\"{id}\"]\nowner = \"ghost\"\nsession = \"dead-agent\"\npid = 0\n\
             pid_start = 1\nboot_id = \"nope\"\nhost = \"h\"\nworktree = \"/tmp\"\n\
             started = \"2026-09-05T00:00:00Z\"\nseen = \"2026-09-05T00:00:00Z\"\n"
        ),
    )
    .unwrap();

    let v = env.json(&sci, &["start", &id]);
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w.as_str().unwrap().contains("dead-agent")),
        "taking over a stale claim names the displaced holder: {v}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli claim`
Expected: FAIL — `start` has no `--force`, `show` has no `claim` field, nothing refuses.

- [ ] **Step 3: Write the implementation**

In `src/commands/mod.rs`, add the claim transaction to `Ctx`:

```rust
use crate::claims::{Claim, ClaimStore, Liveness, MutationLock, identity, liveness};

/// What `save` must do to the claim store once the task file is on disk.
enum ClaimOp {
    /// Acquired before the write; undo it if the write fails.
    Acquired(TaskId),
    /// Release after the write succeeds.
    ReleaseAfterSave(TaskId),
}
```

Add to `impl Ctx`:

```rust
    pub fn describe_claim(claim: &Claim, liveness: &Liveness) -> String {
        let pid = match claim.pid {
            Some(pid) => format!(" pid {pid}"),
            None => String::new(),
        };
        let state = match liveness {
            Liveness::Live => "live".to_string(),
            Liveness::Stale(why) => format!("stale: {why}"),
        };
        format!(
            "{} (session {}, owner {}, host {}{pid}, worktree {}, since {}, {state})",
            claim.session, claim.session, claim.owner, claim.host, claim.worktree, claim.started
        )
    }

    /// Guard and acquire, run inside `transition` while the mutation lock is held.
    ///
    /// Release is **destination-based**: any destination other than `doing` releases this
    /// session's claim. It must not key off *leaving* a local `doing`, because a session
    /// can hold the shared claim while its own checkout still reads `todo` — the ordinary
    /// cross-worktree case — and its `done` there would otherwise strand the claim.
    fn claim_transition(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let me = identity();
        let store = self.claims_mut()?;
        let mut warning = None;

        if let Some(existing) = store.get(id) {
            let live = liveness(existing);
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
                    warning = Some(format!(
                        "took over {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                }
                _ => {}
            }
        }

        // Liveness pruning is one of exactly two ways a claim goes away; the other is a
        // successful guarded release below. Local status never prunes: this checkout's
        // view of the task may be the older one, and another branch may have reopened and
        // claimed it since.
        store.prune_dead();

        self.pending_claim = if to == Status::Doing {
            let now = crate::time::now();
            let claim = Claim {
                owner: owner_name(&self.project)?,
                session: me.session,
                pid: me.pid,
                pid_start: me.pid.and_then(|pid| match crate::claims::proc_stat(pid) {
                    crate::claims::ProcStat::Found { starttime, .. } => Some(starttime),
                    _ => None,
                }),
                boot_id: crate::claims::boot_id(),
                host: crate::claims::hostname(),
                worktree: self.project.root.display().to_string(),
                started: now.clone(),
                seen: now,
            };
            self.claims_mut()?.insert(id, claim);
            self.claims_mut()?.save()?;
            Some(ClaimOp::Acquired(id.clone()))
        } else if self
            .claims_mut()?
            .get(id)
            .is_some_and(|claim| claim.session == me.session)
        {
            Some(ClaimOp::ReleaseAfterSave(id.clone()))
        } else {
            self.claims_mut()?.save()?; // persist the pruning
            None
        };

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }

    /// Settle the claim store once the task file write has resolved.
    fn finish_claim(&mut self, wrote: bool) -> Result<()> {
        match self.pending_claim.take() {
            // Acquire failed at the file write: undo the claim so a failed `start` does
            // not leave the task looking busy.
            Some(ClaimOp::Acquired(id)) if !wrote => {
                let store = self.claims_mut()?;
                store.remove(&id);
                if let Err(error) = store.save() {
                    self.warnings.push(format!(
                        "{id} is still claimed after a failed write ({error}); \
                         run `tasks start --force {id}` to reclaim it"
                    ));
                }
            }
            Some(ClaimOp::ReleaseAfterSave(id)) if wrote => {
                let store = self.claims_mut()?;
                store.remove(&id);
                if let Err(error) = store.save() {
                    // Liveness pruning will not reclaim this while the session lives, and
                    // `start --force` cannot: `can_transition` rejects `done -> doing`.
                    self.warnings.push(format!(
                        "{id} was closed but its claim was not released ({error}); \
                         re-run the same command to retry the release"
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
```

Add `pending_claim: Option<ClaimOp>` to `Ctx` (defaulting to `None` in both constructors
and in the `Command::Add` literal), then thread it through `transition` and `save`:

```rust
pub fn transition(ctx: &mut Ctx, task: &mut Task, to: Status, force: bool) -> Result<()> {
    if !Status::can_transition(task.status, to) {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            to.as_str().into(),
        ));
    }
    // The claim guard runs before the dependency and descendant checks so that a session
    // that no longer holds the task is told *that*, rather than something incidental.
    ctx.claim_transition(&task.id, to, force)?;
    if to == Status::Done && task.status != Status::Done && !force {
        let open = open_deps(ctx, task)?;
        if !open.is_empty() {
            return Err(Error::OpenDependencies(
                task.id.to_string(),
                open.join(", "),
            ));
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

pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    ctx.project.validate_docs(task)?;
    // Acquire is claim-then-file and release is file-then-claim, so both orders fail
    // toward "claim held": a claim with no file update makes an idle task look busy and
    // self-heals when the session dies, while a file update with no claim is the
    // invisibility bug this exists to remove.
    let wrote = ctx.project.write_task(task);
    ctx.finish_claim(wrote.is_ok())?;
    wrote
}
```

`open_deps` takes `&Ctx`; calling it with a `&mut Ctx` reborrows fine.

In `src/commands/status.rs`, make the callers `mut` and add the heartbeat:

```rust
pub fn note(mut ctx: Ctx, id: String, text: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &text)?;
    save(&mut ctx, &mut task)?;
    // The heartbeat, and only on our own claim: `note` never touches a foreign one and is
    // never refused. It is still serialized under the mutation lock, because a note
    // rewrites the whole markdown file however append-only it is in meaning.
    let me = crate::claims::identity();
    let store = ctx.claims_mut()?;
    if let Some(claim) = store.get(&task.id)
        && claim.session == me.session
    {
        let mut refreshed = claim.clone();
        refreshed.seen = crate::time::now();
        store.insert(&task.id, refreshed);
        store.save()?;
    }
    Ok(id_out(ctx, &task))
}
```

Apply `mut ctx` and `save(&mut ctx, ...)` / `transition(&mut ctx, ...)` in `start`,
`close`, `unblock` (`src/commands/status.rs`), `edit::run` and `editor`
(`src/commands/edit.rs`), `add::run` (`src/commands/add.rs`) and `dep::run`
(`src/commands/dep.rs`). The compiler names every one.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test cli claim && cargo test`
Expected: PASS. `a_live_claim_from_another_session_refuses_start`,
`a_displaced_session_cannot_close_the_task_it_lost`,
`release_follows_the_claim_not_the_local_doing_status`,
`a_stale_local_done_does_not_prune_a_live_claim`, `re_running_a_close_retries_the_release`
and `a_stale_claim_is_taken_over_with_a_warning` all pass once Tasks 7 and 8 land the
`--force` flag and the `claim` output field; run them at the end of Task 8 if they fail
only on those two.

- [ ] **Step 5: Commit**

```bash
git add src/commands src/claims.rs tests/cli.rs
git commit -m "feat(claims): guard status changes and release by destination"
```

---

### Task 7: `tasks start --force` takeover

**Files:**
- Modify: `src/cli.rs:125-126`, `src/commands/mod.rs` (`Command::Start` arm), `src/commands/status.rs` (`start`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `claim_transition(.., force)` from Task 6.
- Produces: `Command::Start { id: String, force: bool }`; `status::start(ctx, id, force)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn force_takeover_records_a_note_naming_the_displaced_session() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    env.cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();
    env.cmd(&sci)
        .args(["start", "--force", &id])
        .env("TASKS_SESSION", "agent-b")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("agent-a"),
        "the takeover is recorded in the task's notes: {raw}"
    );
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["fields"]["claim"]["session"], "agent-b");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli force_takeover`
Expected: FAIL — `unexpected argument '--force'`.

- [ ] **Step 3: Write minimal implementation**

In `src/cli.rs`, replace the `Start` variant:

```rust
    /// Claim a task: status=doing, owner=you.
    Start {
        id: String,
        /// Take over a claim another live session holds.
        #[arg(long)]
        force: bool,
    },
```

In `src/commands/mod.rs`:

```rust
        Command::Start { id, force } => status::start(open_write_ctx(dir)?, id, force),
```

In `src/commands/status.rs`:

```rust
pub fn start(mut ctx: Ctx, id: String, force: bool) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let before = ctx.warnings.len();
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    task.owner = Some(owner_name(&ctx.project)?);
    // A takeover displaces someone; the task's own record should say so, not just the
    // ephemeral warning stream.
    let takeovers: Vec<String> = ctx.warnings[before..].to_vec();
    for takeover in takeovers {
        let owner = owner_name(&ctx.project)?;
        append_note(&mut task, &owner, &takeover)?;
    }
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
```

`append_note` is already imported in `status.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli force_takeover`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/commands tests/cli.rs
git commit -m "feat(start): add --force to take over a claim"
```

---

### Task 8: Claims in the JSON and pretty output

**Files:**
- Modify: `src/output.rs:90-102` (`TaskSummary`), `src/output.rs:66-74` (`ShowFields`), `src/output.rs:254+` (`pretty`), `src/commands/show.rs`, `src/commands/list.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `ClaimInfo { owner, session, host, pid, worktree, started, seen, live }`; `TaskSummary.claim: Option<ClaimInfo>`; `ShowFields.claim: Option<ClaimInfo>`; `ClaimInfo::of(&Claim) -> ClaimInfo`.
- Consumed by Tasks 6, 7 and 9's assertions.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn claim_appears_in_show_and_list_json() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    let unclaimed = env.json(&sci, &["show", &id]);
    assert!(unclaimed["fields"]["claim"].is_null());

    env.cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["fields"]["claim"]["session"], "agent-a");
    assert_eq!(v["fields"]["claim"]["live"], true);
    assert_eq!(v["fields"]["claim"]["pid"], pid.parse::<u64>().unwrap());

    let list = env.json(&sci, &["list", "--status", "doing"]);
    assert_eq!(list["tasks"][0]["claim"]["session"], "agent-a");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli claim_appears_in_show`
Expected: FAIL — `claim` is null after `start`.

- [ ] **Step 3: Write minimal implementation**

In `src/output.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
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

Add `pub claim: Option<ClaimInfo>` to `TaskSummary` and `ShowFields`. `TaskSummary::of`
gains a `claims: Option<&ClaimStore>` parameter — pass `None` from call sites that have no
store and `Some(&store)` from `list`, `prime`, `ready` and `next`:

```rust
impl TaskSummary {
    pub fn of(task: &Task, all: &[Task], claims: Option<&crate::claims::ClaimStore>) -> TaskSummary {
        // ... existing field initialisation unchanged ...
        claim: claims
            .and_then(|store| store.get(&task.id))
            .map(ClaimInfo::of),
    }
}
```

Read paths load the store read-only with `ClaimStore::load(&prefix)?` — no lock, because
`atomic_write` publishes by rename.

In `pretty`, mark a claimed row. In the `table` helper, append to the owner column:

```rust
    let owner = match (&row.owner, &row.claim) {
        (_, Some(claim)) if claim.live => format!("@{} [{}]", row.owner.as_deref().unwrap_or("-"), claim.session),
        (Some(owner), _) => format!("@{owner}"),
        (None, _) => String::new(),
    };
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli claim_appears_in_show && cargo test --test cli claim`
Expected: PASS, including the Task 6 and Task 7 tests that assert on `fields.claim`.

- [ ] **Step 5: Commit**

```bash
git add src/output.rs src/commands tests/cli.rs
git commit -m "feat(output): report the claim on show, list, and pretty rows"
```

---

### Task 9: `ready`/`next` exclusion and the `prime` overlay

**Files:**
- Modify: `src/commands/list.rs:153-210` (`prime`), `ready_tasks`, `next`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `ClaimStore`, `liveness`, `ClaimInfo` from Tasks 1, 2 and 8.
- Produces: no new public names; `prime`'s `doing` predicate and three warning kinds.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn ready_omits_claimed_tasks_including_your_own_and_says_why() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--size", "s"]));
    let pid = std::process::id().to_string();

    assert_eq!(env.json(&sci, &["ready"])["tasks"][0]["id"], id);

    env.cmd(&sci)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

    // Even to the session that holds it: `start` is the authoritative check, and a silent
    // omission is worse than an explained one.
    let out = env
        .cmd(&sci)
        .args(["ready"])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tasks"].as_array().unwrap().len(), 0, "{v}");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains(&id)),
        "the omission is explained: {v}"
    );
    assert_eq!(env.json(&sci, &["next"])["next"], serde_json::Value::Null);
}

#[test]
fn prime_shows_a_claim_made_in_another_root_and_warns_about_divergence() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    let pid = std::process::id().to_string();

    // Root A claims it; root B's copy still reads `todo` — the tasks-8f4b41 sequence.
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();
    env.cmd(&a)
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", &pid)
        .assert()
        .success();

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

    let store = env.home.path().join(".local/state/tasks/claims/sci.toml");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(
        &store,
        format!(
            "[claims.\"{id}\"]\nowner = \"ghost\"\nsession = \"dead-agent\"\npid = 0\n\
             pid_start = 1\nboot_id = \"nope\"\nhost = \"h\"\nworktree = \"/tmp\"\n\
             started = \"2026-09-05T00:00:00Z\"\nseen = \"2026-09-05T00:00:00Z\"\n"
        ),
    )
    .unwrap();

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

Run: `cargo test --test cli ready_omits_claimed && cargo test --test cli prime_`
Expected: FAIL — claimed tasks still appear in `ready`, no warnings.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/list.rs`, load the store once per project in scope and filter:

```rust
/// Live claims for every project in scope, keyed by task id.
fn live_claims(ctx: &ReadCtx) -> Result<std::collections::BTreeMap<String, Claim>> {
    let mut live = std::collections::BTreeMap::new();
    for project in ctx.scope.projects() {
        let store = ClaimStore::load(&project.prefix)?;
        for (id, claim) in store.iter() {
            if liveness(claim) == Liveness::Live {
                live.insert(id.clone(), claim.clone());
            }
        }
    }
    Ok(live)
}
```

In `ready_tasks`, after the existing filters:

```rust
    let claimed = live_claims(ctx)?;
    ready.retain(|task| {
        match claimed.get(&task.id.to_string()) {
            Some(claim) => {
                ctx.warnings.push(format!(
                    "{} omitted: claimed by {} in {} — `tasks start --force {}` to take it over",
                    task.id, claim.session, claim.worktree, task.id
                ));
                false
            }
            None => true,
        }
    });
```

`next` already reads the head of `ready_tasks`, so it inherits the exclusion.

In `prime`, widen the `doing` predicate and add the two warnings:

```rust
    let claimed = live_claims(&ctx)?;
    let mut doing: Vec<Task> = all
        .iter()
        // Local `doing`, or a live claim: that is how a claim made in another worktree
        // surfaces here even though this checkout's file still reads `todo`.
        .filter(|task| {
            task.status == Status::Doing || claimed.contains_key(&task.id.to_string())
        })
        .cloned()
        .collect();
```

and after the uncommitted-files loop:

```rust
    for task in &all {
        let key = task.id.to_string();
        if let Some(claim) = claimed.get(&key)
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
    // A stale claim over a local `todo` matches neither the `doing` predicate above nor
    // the divergence warning, so without this it would appear nowhere.
    for project in ctx.scope.projects() {
        for (id, claim) in ClaimStore::load(&project.prefix)?.iter() {
            if let Liveness::Stale(why) = liveness(claim) {
                ctx.warnings.push(format!(
                    "{id} has a stale claim from {} ({why}); \
                     `tasks start --force {id}` to take it over",
                    claim.session
                ));
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test cli ready_omits_claimed && cargo test --test cli prime_ && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/list.rs tests/cli.rs
git commit -m "feat(list): exclude claimed tasks from ready and overlay claims on prime"
```

---

### Task 10: Lock `feedback` recurrence

**Files:**
- Modify: `src/commands/feedback.rs:199-222` (`guarded_update`) and its recurrence caller
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `MutationLock` from Task 3.
- Produces: `guarded_update` unchanged in signature; the lock is taken by its recurrence caller.

`add`-style creation stays lock-free — `create_task` links the file into place exclusively,
so it has no read-modify-write to protect. Recurrence is different: `guarded_update`
re-reads the raw file and compares it against its snapshot (`feedback.rs:213`), but the
window between that comparison and `write_task` (`feedback.rs:215`) is still a check-write
race.

Only the **target** project's lock is taken. The source `Ctx` stays unlocked, so no command
ever holds two locks — which also keeps this correct when source and target are the same
project, as they are in this repository, since a second `flock` on the same file from the
same process would deadlock.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn feedback_recurrence_serializes_against_a_concurrent_writer() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let first = env.json(&sci, &["feedback", "the thing is slow", "--category", "friction"]);
    let id = first["id"].as_str().unwrap().to_string();

    // Two recurrences of the same report race; both must land.
    let mut handles = Vec::new();
    for n in 0..2 {
        let mut cmd = env.cmd(&sci);
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

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("detail 0"), "first update survived: {raw}");
    assert!(raw.contains("detail 1"), "second update survived: {raw}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli feedback_recurrence_serializes`
Expected: FAIL intermittently — one update lost, or `concurrent_modification`. Run it a few
times: `for i in $(seq 20); do cargo test --test cli feedback_recurrence_serializes || break; done`

- [ ] **Step 3: Write minimal implementation**

In `src/commands/feedback.rs`, take the target lock at the top of the recurrence path,
before the first read, and hold it until the update returns:

```rust
    // Recurrence is a read-modify-write, and `guarded_update`'s raw-content comparison
    // still leaves a window between the check and `write_task`. Only the target's lock is
    // needed; the source `Ctx` stays unlocked, so no command ever holds two.
    let _lock = crate::claims::MutationLock::acquire(&target.prefix)?;
    let task = guarded_update(target, &id, eligible, mutate)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `for i in $(seq 20); do cargo test --test cli feedback_recurrence_serializes || break; done`
Expected: PASS every time.

- [ ] **Step 5: Commit**

```bash
git add src/commands/feedback.rs tests/cli.rs
git commit -m "fix(feedback): lock the target project for recurrence updates"
```

---

### Task 11: The uncommitted-before-branching warning

**Files:**
- Modify: `src/commands/status.rs` (`start`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Project::uncommitted_task_files` (`src/repo.rs:323`).
- Produces: one warning string from `start`.

This is the narrower half of tasks-8f4b41 and covers only the *reverse* order — worktrees
already exist, then `start`. The order actually reported (start first, worktree created
afterwards) is covered by the divergence warning in Task 9, which fires in the new worktree
at the moment the disagreement becomes observable.

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

    // One worktree: nothing to diverge from yet.
    let v = env.json(&sci, &["start", &id]);
    assert!(
        !v["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("worktree")),
        "a single-worktree repo has nothing to warn about: {v}"
    );
    git(&["add", "-A"]);
    git(&["commit", "-qm", "start"]);

    let wt = sci.join("wt");
    git(&["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()]);
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

Run: `cargo test --test cli start_warns_when_a_repo_with_several_worktrees`
Expected: FAIL — no such warning.

- [ ] **Step 3: Write minimal implementation**

In `src/commands/status.rs`, add a helper and call it at the end of `start`, after `save`:

```rust
/// Warns when `start` leaves the task file uncommitted in a repo that already has more
/// than one worktree. Every `start` leaves the file uncommitted — the protocol commits at
/// `done` — so warning unconditionally would be noise; the extra worktree is what makes it
/// matter.
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

and in `start`, immediately before `Ok(id_out(ctx, &task))`:

```rust
    warn_if_uncommitted_with_worktrees(&mut ctx, &task)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli start_warns_when_a_repo_with_several_worktrees`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/status.rs tests/cli.rs
git commit -m "feat(start): warn when a claim stays uncommitted in a multi-worktree repo"
```

---

### Task 12: Documentation and closeout

**Files:**
- Modify: `skills/tasks/SKILL.md`, `AGENTS.md`, `docs/specs/2026-08-29-tasks-design.md`, `docs/specs/2026-09-05-work-claims-design.md`
- Test: `just gate`

- [ ] **Step 1: Document the claim protocol in the shipped skill**

`skills/tasks/SKILL.md` goes out to other projects, so it is where agents learn this. Add
to the session protocol section, after the `tasks start` step:

```markdown
- `tasks start` records a claim outside git — visible from every worktree of the project,
  carrying your session identity and a liveness handle. Starting a task another live
  session holds fails with `claimed`; `tasks start --force <id>` takes it over and records
  the takeover in the task's notes. `ready` and `next` omit claimed tasks and say so in
  their warnings.
- Set `TASKS_SESSION` (and `TASKS_SESSION_PID`, if you have a long-lived process id) when
  several agents share one terminal or one harness process. Without it, agents that share
  a session id are indistinguishable to the claim store, which is the case this exists to
  prevent.
```

- [ ] **Step 2: Note the gates in AGENTS.md**

Under `## Layout`, add `src/claims.rs` to the module list:

```markdown
  `claims.rs` (out-of-git work claims: the per-prefix store, liveness, the mutation lock),
```

- [ ] **Step 3: Correct the design doc of record**

`docs/specs/2026-08-29-tasks-design.md:112` describes `owner` as the whole claim story.
Add a pointer next to it so the two docs do not drift:

```markdown
| `owner`    | string              | no       | Advisory claim; set by `start`; `[A-Za-z0-9._/@+-]+`. Session identity and liveness live outside git — see `2026-09-05-work-claims-design.md`. |
```

- [ ] **Step 4: Correct this design's own status header**

In `docs/specs/2026-09-05-work-claims-design.md`, change the header to reflect what landed
and confirm each "Known gaps" entry is still true:

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

**Spec coverage:** Store and prefix keying → Task 1. Locking → Tasks 3, 4, 10. Identity →
Task 5. Liveness → Task 2. Command behaviour, destination-based release, write ordering,
pruning → Task 6. `start --force` → Task 7. JSON contract → Task 8. `ready`/`prime` and all
three warnings → Task 9. `feedback` recurrence → Task 10. The tasks-8f4b41 warning → Task
11. Docs and the "Known gaps" recheck → Task 12. The three known gaps are deliberately not
implemented and are recorded as such.

**Type consistency:** `ClaimStore::{path_for, load, load_from, save, get, insert, remove,
prune_with, prune_dead, iter}`, `Claim`, `Liveness::{Live, Stale}`, `ProcStat::{NotFound,
Unreadable, Found}`, `Identity {session, pid}`, `MutationLock::{path_for, acquire}`,
`ClaimInfo::of`, `Error::Claimed` are used under those exact names throughout.
`TaskSummary::of` gains a third parameter in Task 8; Task 9's call sites pass `Some(&store)`.
`transition` and `save` take `&mut Ctx` from Task 6 onward.
