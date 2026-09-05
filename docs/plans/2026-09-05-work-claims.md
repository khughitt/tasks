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
// Removed in Task 7, whose guard is the first consumer of the whole surface. Until then
// this is a binary crate's unreachable `pub` API, which `clippy -D warnings` rejects — and
// the pre-commit hook runs `just check`, so without this the intermediate commits could not
// land. Task 7 deletes the attribute and proves clippy is clean without it.
#![allow(dead_code)]

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
        // Wrapped in a closure, not passed directly: `std::env::var_os` is generic over
        // `AsRef<OsStr>`, so handing it to an `impl Fn(&str)` parameter fails with
        // "implementation of `Fn` is not general enough".
        Self::path_with(prefix, |key| std::env::var_os(key))
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
        // Timestamps are validated here, on the way in, so corruption is a typed error at
        // the boundary rather than something the liveness rules must interpret. A garbled
        // `seen` must never read as "stale": that would let the next writer discard an
        // unverifiable claim without `--force`, the exact opposite of the
        // absence-of-evidence rule everything else here follows.
        for (id, claim) in &claims {
            for (field, value) in [("started", &claim.started), ("seen", &claim.seen)] {
                crate::time::parse(value).map_err(|error| {
                    Error::Config(format!(
                        "{}: claim {id} has an unreadable {field}: {error}",
                        path.display()
                    ))
                })?;
            }
        }
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

Split `cmd` so a later task can spawn and inspect a child process without duplicating the
environment chain:

```rust
    /// A raw command with this environment applied. `cmd` wraps it; Task 11 spawns it.
    ///
    /// Both streams are piped explicitly. `Command::output()` would do that for itself, but
    /// `spawn()` inherits by default, and an inherited stream makes `wait_with_output()`
    /// hand back empty buffers — so every `err_kind` assertion on a spawned child would
    /// fail to find any JSON at all.
    pub fn raw(&self, dir: &Path) -> std::process::Command {
        let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin("tasks"));
        c.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("HOME", self.home.path())
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
    fn a_corrupt_timestamp_is_rejected_on_load_rather_than_read_as_stale() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("sci.toml");
        std::fs::write(
            &path,
            "[claims.\"sci-000001\"]\nowner = \"o\"\nsession = \"s\"\nhost = \"h\"\n\
             worktree = \"/w\"\nstarted = \"2026-09-05T00:00:00Z\"\nseen = \"garbage\"\n",
        )
        .unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(error.to_string().contains("seen"), "{error}");
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
        // Unreachable in production: `load_from` rejects a corrupt timestamp as a config
        // error before liveness ever sees it. Reachable only from a hand-built `Claim` in a
        // test, and `Live` is the conservative direction — an unreadable timestamp is an
        // absence of evidence, so it must not authorize an automatic takeover.
        Err(_) => Liveness::Live,
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
        // A closure, for the same higher-ranked-lifetime reason as `ClaimStore::path_for`.
        Self::path_with(prefix, |key| std::env::var_os(key))
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

Only the lock lands here. `ClaimIntent`, the `claims` and `pending_claim` fields and
`claims_mut()` all live in `commands/mod.rs`, which the `claims.rs` allowance does not cover
— adding them before Task 7 consumes them means `clippy -D warnings` rejects the commit.

```rust
use crate::claims::MutationLock;

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
    /// Held for the life of a write command; `None` for the read commands that also take a
    /// `Ctx` (`show`, `graph`, `check`) and for the two create-only paths.
    pub lock: Option<MutationLock>,
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = start_dir(dir)?;
    Ok(Ctx {
        project: Project::locate(&start)?,
        registry: Registry::load()?,
        warnings: Vec::new(),
        lock: None,
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
```

Switch exactly the eight write paths in `run` to `open_write_ctx` — `Edit`, `Note`, `Start`,
`Done`, `Drop`, `Block`, `Unblock`, `Dep` — leaving `Show`, `Graph`, `Check`, `Add` and
`Feedback` on `open_ctx`. Add `lock: None` to the `Ctx { .. }` literal in the `Command::Add`
arm (`src/commands/mod.rs:280-285`).

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
    // A closure, for the same higher-ranked-lifetime reason as `ClaimStore::path_for`.
    identity_from(|key| std::env::var_os(key), unix_session_id())
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

### Task 6: One claim snapshot, and claims in the output

Placed before the guard so the guard's tests have a contract to assert against; testable on
its own through a hand-written store fixture.

**Depends on Tasks 1 and 2** — the snapshot evaluates liveness, so the liveness rules must
already exist.

**Files:**
- Modify: `src/claims.rs` (`ClaimSnapshot`), `src/output.rs`, `src/commands/show.rs`, `src/commands/list.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `ClaimSnapshot`, `ClaimSnapshot::{load, get}` — `live` and `stale` arrive in Task 8 with their only consumer, because Task 7 removes the lint allowance and an unused method would fail the gate in between. `ClaimInfo`, `ClaimInfo::of(&Claim, &Liveness)`, `TaskSummary.claim`, `ShowFields.claim`, `TaskSummary::of(task, all, Option<&ClaimSnapshot>)`.
- Test helper produced: `write_claim(&TestEnv, prefix, id, session, live)`.

**Liveness is evaluated once per command and never recomputed.** The snapshot holds the
verdict beside the claim, and `ClaimInfo::of` takes that verdict rather than re-deriving it.
It deliberately keeps no `ClaimStore`s: a second handle on the raw store is a second route
around the cached result, and that is exactly how one `prime` ends up contradicting itself.

**`ShowOut.fields` is `#[serde(flatten)]`** (`src/output.rs:78`), so `show` exposes the claim
at `v["claim"]`. Asserting `v["fields"]["claim"]` silently passes every null assertion.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`:

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

    assert_eq!(env.json(&sci, &["list"])["tasks"][0]["claim"]["session"], "agent-a");
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

- [ ] **Step 3: Write the snapshot**

In `src/claims.rs`:

```rust
/// Every claim in scope, with its liveness verdict, read **once** per command.
///
/// Reading the store separately for the ready filter, the doing predicate, the stale
/// warnings and the row summaries would let a single `prime` contradict itself, since a
/// claim can go stale between two reads. Liveness is evaluated here, once, and every
/// consumer uses the stored verdict.
///
/// It holds no `ClaimStore`: keeping one would be a second handle on the raw claims and a
/// second route around the cached verdict, which is the bug this type exists to prevent.
#[derive(Debug, Default)]
pub struct ClaimSnapshot {
    by_id: BTreeMap<String, (Claim, Liveness)>,
}

impl ClaimSnapshot {
    pub fn load<'a>(prefixes: impl Iterator<Item = &'a str>) -> Result<ClaimSnapshot> {
        let mut by_id = BTreeMap::new();
        for prefix in prefixes {
            for (id, claim) in ClaimStore::load(prefix)?.iter() {
                let live = liveness(claim);
                by_id.insert(id.clone(), (claim.clone(), live));
            }
        }
        Ok(ClaimSnapshot { by_id })
    }

    pub fn get(&self, id: &TaskId) -> Option<&(Claim, Liveness)> {
        self.by_id.get(&id.to_string())
    }

}
```

`live()` and `stale()` are deliberately **not** added here. Task 7 deletes the
`#![allow(dead_code)]`, and a method whose only caller arrives in Task 8 would fail
`clippy -D warnings` in between; Task 8 adds them alongside the code that uses them.

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
    /// Takes the verdict rather than recomputing it, so every row in one command agrees.
    pub fn of(claim: &crate::claims::Claim, live: &crate::claims::Liveness) -> ClaimInfo {
        ClaimInfo {
            owner: claim.owner.clone(),
            session: claim.session.clone(),
            host: claim.host.clone(),
            pid: claim.pid,
            worktree: claim.worktree.clone(),
            started: claim.started.clone(),
            seen: claim.seen.clone(),
            live: *live == crate::claims::Liveness::Live,
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
        claims: Option<&crate::claims::ClaimSnapshot>,
    ) -> TaskSummary {
        // ... existing fields unchanged ...
        claim: claims
            .and_then(|snapshot| snapshot.get(&task.id))
            .map(|(claim, live)| ClaimInfo::of(claim, live)),
    }
```

`show`, `list`, `ready`, `next` and `prime` each build **one** `ClaimSnapshot` at the top and
pass `Some(&snapshot)` to every `TaskSummary::of` call in that command. Read paths take **no
lock**: `atomic_write` publishes by rename, so a reader sees one whole version or another.

In the `table` helper inside `pretty`, prefer the claim's own owner — `row.owner` comes from
this checkout's copy of the file, which may predate the claim entirely:

```rust
    let owner = match &row.claim {
        Some(claim) if claim.live => format!("@{} [{}]", claim.owner, claim.session),
        Some(claim) => format!("@{} [{} stale]", claim.owner, claim.session),
        None => match &row.owner {
            Some(owner) => format!("@{owner}"),
            None => String::new(),
        },
    };
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli claim && just check && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/output.rs src/commands tests/cli.rs
git commit -m "feat(output): report the claim on show, list, and pretty rows"
```

---

### Task 7: The guard, the transaction, and `start --force`

The core task, and one unit. The guard, the acquire, the release and the takeover flag are a
single transaction — `claim_guard` already takes `force`, so shipping the guard without the
flag would commit tests that cannot pass, and a reviewer cannot sensibly accept a refusal
with no sanctioned way through it.

**The rule that shapes all of it: `transition()` guards and records an intent; `save()` is
the only place that persists anything.** An earlier draft acquired inside `transition()`,
which meant a later validation failure, a rejected concurrent edit, or a failed write could
each leave the store mutated — three separate failure paths. With persistence in one
validated operation there is one.

**Depends on Tasks 2, 4, 5 and 6.**

**Files:**
- Modify: `src/claims.rs` (delete the module-level `#![allow(dead_code)]`), `src/commands/mod.rs`, `src/commands/status.rs`, `src/cli.rs:125-126`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `transition(ctx: &mut Ctx, ..)`, `save(ctx: &mut Ctx, ..)`, `Ctx::describe_claim`, `Command::Start { id, force }`, `status::start(ctx, id, force)`.
- Test helpers produced: `two_roots`, `as_agent`, `err_kind`, `err_detail`, `TestEnv::init_forced`.

`transition` and `save` change from `&Ctx` to `&mut Ctx`; every caller in
`src/commands/{status,edit,add,dep}.rs` needs `mut ctx`. Mechanical — the compiler lists them.

- [ ] **Step 1: Write the failing tests**

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

and to `tests/cli.rs`:

```rust
/// Two project roots sharing one prefix: what a main checkout and a worktree look like to a
/// store keyed by prefix.
fn two_roots(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf) {
    (env.init("sci"), env.init_forced("sci"))
}

/// Run `tasks` as a named agent with a live pid.
fn as_agent(env: &TestEnv, dir: &std::path::Path, session: &str) -> assert_cmd::Command {
    let mut cmd = env.cmd(dir);
    cmd.env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    cmd
}

// The error shape is {"error": {"kind", "detail"}} — there is no `message` field.
fn err_kind(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    v["error"]["kind"].as_str().unwrap().to_string()
}

fn err_detail(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
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

    // Both branch states are established *before* the claim exists. Otherwise A's own close
    // is refused — correctly — and the test never reaches what it is about.
    env.json(&a, &["edit", &id, "--status", "done"]);
    as_agent(&env, &b, "agent-b").args(["start", &id]).assert().success();

    // `note` is the right probe from A: never refused, and it touches the store.
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

    // A release that failed after the file write: task closed, real claim still present.
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
    as_agent(&env, &sci, "agent-a").args(["start", &id]).assert().success();
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["session"], "agent-a");
}

#[test]
fn note_fails_before_appending_when_identity_cannot_be_resolved() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let before = env.read(&sci, &format!("tasks/{id}.md"));

    // A corrupt store is the reachable version of "the heartbeat cannot proceed".
    let path = env.claim_store("sci");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not valid toml = [").unwrap();

    let out = env.cmd(&sci).args(["note", &id, "hello"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        env.read(&sci, &format!("tasks/{id}.md")),
        before,
        "a note that reports failure must not have landed; a retry would duplicate it"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks --test cli`
Expected: FAIL — no `--force` flag, nothing refuses, nothing releases.

- [ ] **Step 3: Write the implementation**

Delete the module-level `#![allow(dead_code)]` from the top of `src/claims.rs`. Every item is
reachable from here on, and Step 4 proves it.

First, the `Ctx` plumbing that Task 4 deliberately left out — it lives in
`src/commands/mod.rs`, which the `claims.rs` allowance never covered, so it could only land
in the task that consumes it:

```rust
use crate::claims::{ClaimStore, Liveness};

/// What `save` must do to the claim store once every validation has passed. Recorded by the
/// guard in `transition`; **nothing is persisted until `save` acts on it.**
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release,
}
```

Add both fields to `Ctx`, `None` for each in `open_ctx` and in the `Command::Add` literal:

```rust
    claims: Option<ClaimStore>,
    pending_claim: Option<(TaskId, ClaimIntent)>,
```

and the accessor:

```rust
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

`src/commands/mod.rs` — the `run` arm, then the guard:

```rust
        Command::Start { id, force } => status::start(open_write_ctx(dir)?, id, force),
```

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

    /// Guard only. Decides whether this session may make the change and records what `save`
    /// should do — **and persists nothing**, so a validation failure, a rejected concurrent
    /// edit, or a failed write cannot leave the store mutated.
    ///
    /// Release is destination-based: any destination other than `doing` releases this
    /// session's claim. It must not key off *leaving* a local `doing`, because a session can
    /// hold the shared claim while its own checkout still reads `todo` — the ordinary
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
            // would unclaim work someone still holds.
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

`src/commands/status.rs` — `start`, and `note` with the heartbeat ordered correctly:

```rust
pub fn start(mut ctx: Ctx, id: String, force: bool) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let before = ctx.warnings.len();
    transition(&mut ctx, &mut task, Status::Doing, force)?;
    let owner = owner_name(&ctx.project)?;
    task.owner = Some(owner.clone());
    // A takeover displaces someone; the task's own record should say so, not just the
    // ephemeral warning stream.
    for takeover in ctx.warnings[before..].to_vec() {
        append_note(&mut task, &owner, &takeover)?;
    }
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn note(mut ctx: Ctx, id: String, text: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &text)?;
    // Identity and the store are resolved *before* the file write. Doing it afterwards
    // means an unresolvable identity or a corrupt store returns an error after the note has
    // already landed, and the obvious retry then duplicates it.
    let me = crate::claims::identity()?;
    let mine = ctx
        .claims_mut()?
        .get(&task.id)
        .cloned()
        .filter(|claim| claim.session == me.session);

    save(&mut ctx, &mut task)?;

    // The heartbeat, and only on our own claim: `note` never touches a foreign one and is
    // never refused. It is still serialized under the mutation lock, because a note rewrites
    // the whole markdown file however append-only it is in meaning.
    if let Some(claim) = mine {
        let store = ctx.claims_mut()?;
        store.insert(
            &task.id,
            crate::claims::Claim {
                seen: crate::time::now(),
                ..claim
            },
        );
        if let Err(error) = store.save() {
            // The note is on disk, so this cannot be an error — say plainly what did and
            // did not happen, as the release-failure path does.
            ctx.warnings.push(format!(
                "the note landed, but the claim heartbeat on {} was not refreshed \
                 ({error}); the claim may look stale to other sessions",
                task.id
            ));
        }
    }
    Ok(id_out(ctx, &task))
}
```

Apply `mut ctx` and the `&mut ctx` argument in `close`, `unblock`, `edit::run`, `editor`,
`add::run` and `dep::run`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli && just check && cargo test`
Expected: PASS, with clippy clean now that the `allow(dead_code)` is gone.

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/cli.rs src/commands tests
git commit -m "feat(claims): guard status changes, persist inside save, and add start --force"
```

---

### Task 8: `ready`/`next` exclusion and the `prime` overlay

**Depends on Tasks 6 and 7** — its tests use `two_roots` and `as_agent`, and need `start` to
actually record a claim.

**Files:**
- Modify: `src/claims.rs` (`ClaimSnapshot::live`, `::stale`), `src/commands/list.rs`
- Test: `tests/cli.rs`

**One snapshot per command, threaded all the way through.** `prime` builds the snapshot once
and hands the same reference to `ready_tasks`, to the doing predicate, to both warning loops
and to every `TaskSummary::of`. `ready_tasks` therefore takes it as a parameter rather than
loading its own.

**`ready_tasks` already skips anything whose status is not `Todo`** (`src/commands/list.rs:87-89`),
so a task the current session started *locally* is excluded by status before any claim logic
runs, and no claim warning fires for it — correct, and not something to test for. The case
that matters is a task whose **local** file still says `todo` while another root holds a live
claim.

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
    // The `doing` predicate needs a *live* claim, so without this warning a stale claim over
    // a local `todo` would appear nowhere at all.
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains("dead-agent") && w.contains(&id)
        }),
        "{v}"
    );
}

#[test]
fn one_prime_never_contradicts_itself_about_a_claim() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--size", "s"]));
    std::fs::copy(a.join(format!("tasks/{id}.md")), b.join(format!("tasks/{id}.md"))).unwrap();
    as_agent(&env, &a, "agent-a").args(["start", &id]).assert().success();

    let v = env.json(&b, &["prime"]);
    let row = v["doing"].as_array().unwrap().iter().find(|t| t["id"] == id.as_str()).unwrap();
    // Same verdict in the predicate that put it here and in the row it produced.
    assert_eq!(row["claim"]["live"], true, "{v}");
    assert!(
        !v["ready"].as_array().unwrap().iter().any(|t| t["id"] == id.as_str()),
        "and the ready list agrees: {v}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --bin tasks --test cli -- ready_omits prime_shows prime_warns one_prime_never`
Expected: FAIL — claimed tasks still appear, no warnings.

- [ ] **Step 3: Write minimal implementation**

First add the two snapshot accessors, held back from Task 6 so they arrive with their only
caller and never sit unused under `clippy -D warnings`. In `src/claims.rs`:

```rust
impl ClaimSnapshot {
    pub fn live(&self, id: &TaskId) -> Option<&Claim> {
        match self.get(id) {
            Some((claim, Liveness::Live)) => Some(claim),
            _ => None,
        }
    }

    pub fn stale(&self) -> impl Iterator<Item = (&String, &Claim, &String)> {
        self.by_id
            .iter()
            .filter_map(|(id, (claim, live))| match live {
                Liveness::Stale(why) => Some((id, claim, why)),
                Liveness::Live => None,
            })
    }
}
```

Then change `ready_tasks` to take the snapshot rather than loading one:

```rust
pub fn ready_tasks(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
) -> Result<Vec<Task>> {
```

and after it has built `ready`:

```rust
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

`next` reads the head of `ready_tasks`, so it inherits the exclusion. In `prime`, build the
one snapshot and use it everywhere:

```rust
    let claims = ClaimSnapshot::load(ctx.scope.prefixes().iter().map(String::as_str))?;
    let ready = ready_tasks(&mut ctx, &all, &claims)?;
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

Every `TaskSummary::of` call in `prime`, `ready`, `next`, `list` and `show` takes
`Some(&claims)` from that same snapshot.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tasks --test cli && just check && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/commands tests/cli.rs
git commit -m "feat(list): exclude claimed tasks from ready and overlay claims on prime"
```

---

### Task 9: Lock `feedback` recurrence

**Depends on Task 3.**

**Files:**
- Modify: `src/commands/feedback.rs`
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

`feedback` writes into the project registered under the `tasks` prefix, which `feedback_env()`
(`tests/cli.rs:2261`) sets up; a test that skips it has no target at all.

```rust
#[test]
fn feedback_recurrence_serializes_against_concurrent_recurrences() {
    let (env, target, reporter) = feedback_env();
    let id = env.json(
        &reporter,
        &["feedback", "the thing is slow", "--category", "friction"],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut handles = Vec::new();
    for n in 0..4 {
        let mut cmd = env.cmd(&reporter);
        cmd.args([
            "feedback", "the thing is slow", "--category", "friction",
            "--recur", &id, "-b", &format!("detail {n}"),
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
rounds. This one *is* scheduling-dependent, which is why it is run repeatedly; Task 11 adds
the deterministic lock-contention test that does not rely on collisions happening.

- [ ] **Step 3: Write minimal implementation**

In the recurrence path of `src/commands/feedback.rs`, before the first read:

```rust
    // Recurrence is a read-modify-write, and `guarded_update`'s raw-content comparison still
    // leaves a window between the check and `write_task`. Only the target's lock is needed;
    // the source `Ctx` stays unlocked, so no command ever holds two.
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

### Task 10: The uncommitted-before-branching warning

**Depends on Task 7.**

**Files:**
- Modify: `src/commands/status.rs`
- Test: `tests/cli.rs`

This is the narrower half of tasks-8f4b41 and covers only the *reverse* order — worktrees
already exist, then `start`. The order actually reported (start first, worktree created
afterwards) is covered by Task 8's divergence warning and regression-tested in Task 11.

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

### Task 11: Concurrency and failure regressions

The lock and the rollback are the two things most likely to be silently wrong, and neither is
covered by the happy-path tests. This task adds only tests.

**Contention is forced, not hoped for.** Launching N processes and asserting on outcomes
proves nothing when they happen to run serially — an implementation with no lock at all
passes every such assertion. The tests below therefore hold the project lock *from the test
process* so the children are genuinely queued behind it.

**Two honest caveats about the blocking check.** It is a **timing-based heuristic**, not a
proof: it observes that a writer has not finished yet, which an unlocked writer that simply
has not got there yet also satisfies. It can therefore false-positive on a heavily loaded
machine. Proving the writer actually reached lock acquisition would need instrumentation the
binary does not have; the assertion is kept because it fails reliably against an unlocked
implementation, which is what it is for, and its wording says what it is.

Second, **nothing may be asserted while the lock is held.** A synchronous call that blocks
under a regression would hang the suite before ever reaching the assertion or releasing the
lock. Every child is waited on with a bounded `wait_bounded`, observations are collected into
plain booleans, the lock is released, and only then does anything assert.

**Depends on Tasks 7, 8 and 10.**

**Files:**
- Modify: `tests/cli.rs`

- [ ] **Step 1: Write the deterministic contention tests**

```rust
use std::fs::File;
use std::time::{Duration, Instant};

/// Hold the project's mutation lock from the test process itself.
fn hold_project_lock(env: &TestEnv, prefix: &str) -> File {
    let path = env.claim_store(prefix).with_file_name(format!("{prefix}.lock"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = File::options().create(true).truncate(false).write(true).open(&path).unwrap();
    file.lock().unwrap();
    file
}

/// Wait for a child, but never forever: a regression that makes a command block must fail
/// the assertion, not hang the suite while still holding the lock.
fn wait_bounded(child: &mut std::process::Child, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Collect a child's output, killing it if it outlives `limit`. `None` means it had to be
/// killed.
///
/// Every *final* wait goes through this too, not just the ones being measured. Releasing a
/// handshake or dropping the test's lock only removes the blocker the test knows about; a
/// command that deadlocks for some other reason — an editor path that kept the lock and then
/// tries to reacquire it, say — would still park a bare `wait()` forever and hang the suite
/// with the failure invisible.
fn reap(mut child: std::process::Child, limit: Duration) -> Option<std::process::Output> {
    if wait_bounded(&mut child, limit) {
        return Some(child.wait_with_output().expect("already exited"));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

const REAP: Duration = Duration::from_secs(30);

#[test]
fn a_write_command_waits_for_the_project_lock_and_a_read_command_does_not() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let held = hold_project_lock(&env, "sci");

    let mut writer = env.raw(&sci);
    writer
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let mut writing = writer.spawn().unwrap();

    // Spawned, not called synchronously: if a regression made reads take the lock, a
    // synchronous call here would deadlock against the lock this test is holding.
    let mut reader = env.raw(&sci);
    reader.args(["show", &id]);
    let mut reading = reader.spawn().unwrap();

    // Observations only — no assertions while the lock is held.
    let read_finished = wait_bounded(&mut reading, Duration::from_secs(10));
    // Timing-based, and deliberately so: this says the writer has not finished, which a
    // slow unlocked writer would also satisfy. It fails reliably against an implementation
    // that takes no lock, which is what it is for.
    let writer_still_blocked = writing.try_wait().unwrap().is_none();

    drop(held);

    let read = reap(reading, REAP);
    let wrote = reap(writing, REAP);

    assert!(read_finished, "read commands must not take the mutation lock");
    assert!(read.is_some(), "the read command never exited");
    let wrote = wrote.expect("the write command never exited after the lock was released");
    assert!(
        writer_still_blocked,
        "a write command must wait while the project lock is held"
    );
    assert!(wrote.status.success(), "{}", String::from_utf8_lossy(&wrote.stderr));
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["session"], "agent-a");
}

#[test]
fn simultaneous_starts_produce_exactly_one_winner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // Every process is spawned while the lock is held, so they are all genuinely queued on
    // it before any of them runs — real contention, not a hoped-for collision.
    let held = hold_project_lock(&env, "sci");
    let children: Vec<_> = (0..6)
        .map(|n| {
            let mut cmd = env.raw(&sci);
            cmd.args(["start", &id])
                .env("TASKS_SESSION", format!("agent-{n}"))
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    let outs: Vec<_> = children
        .into_iter()
        .map(|c| reap(c, REAP).expect("a queued start never exited"))
        .collect();
    assert_eq!(
        outs.iter().filter(|o| o.status.success()).count(),
        1,
        "exactly one session may hold the claim"
    );
    for out in outs.iter().filter(|o| !o.status.success()) {
        assert_eq!(err_kind(out), "claimed");
    }
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["live"], true);
}

#[test]
fn concurrent_claims_on_different_tasks_are_all_kept() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let ids: Vec<String> = (0..6)
        .map(|n| id_of(env.json(&sci, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();

    let held = hold_project_lock(&env, "sci");
    let children: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(n, id)| {
            let mut cmd = env.raw(&sci);
            cmd.args(["start", id])
                .env("TASKS_SESSION", format!("agent-{n}"))
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    for child in children {
        let out = reap(child, REAP).expect("a queued start never exited");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    // The store is one whole file per prefix, so an unserialized writer drops the claims it
    // never read.
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

    let held = hold_project_lock(&env, "sci");
    let mut children: Vec<_> = (0..5)
        .map(|n| {
            let mut cmd = env.raw(&sci);
            cmd.args(["note", &id, &format!("line {n}")])
                .env("TASKS_SESSION", "agent-a")
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    let mut status = env.raw(&sci);
    status.args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    children.push(status.spawn().unwrap());
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    for child in children {
        let out = reap(child, REAP).expect("a queued write never exited");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
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
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // A handshake, not a sleep: the editor announces that it is inside its unlocked window
    // and waits to be released, so the test never depends on how fast the machine is.
    let ready = sci.join("editor-ready");
    let go = sci.join("editor-go");
    let script = sci.join("editor.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch {ready}\nwhile [ ! -e {go} ]; do sleep 0.02; done\n\
             sed -i 's/^status: todo/status: doing/' \"$1\"\n",
            ready = ready.display(),
            go = go.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut editing = env.raw(&sci);
    editing
        .args(["edit", &id])
        .env("EDITOR", &script)
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let mut child = editing.spawn().unwrap();

    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready.exists() {
        if Instant::now() >= deadline {
            // Release and reap before failing, so a stuck editor cannot outlive the test.
            // Bounded, because writing `go` releases this test's handshake but cannot
            // guarantee the command exits.
            std::fs::write(&go, "").unwrap();
            reap(child, REAP);
            panic!("the editor never started");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // The editor is holding no lock now, so this must succeed rather than block — but it is
    // spawned and bounded anyway, because if a regression made it block, a synchronous call
    // would hang here with the editor child still parked on its handshake.
    let mut noting = env.raw(&sci);
    noting
        .args(["note", &id, "landed first"])
        .env("TASKS_SESSION", "agent-b")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let mut noting = noting.spawn().unwrap();
    let note_finished = wait_bounded(&mut noting, REAP);

    // Release the editor whatever happened, so the child is always reaped. Bounded, because
    // the handshake is the only blocker this test controls: an editor path that kept the
    // lock and then tried to reacquire it would deadlock past the `go` file.
    std::fs::write(&go, "").unwrap();
    let out = reap(child, REAP).expect("the editor never exited after the handshake");
    let note_out = reap(noting, REAP);

    assert!(note_finished, "the concurrent note blocked; the editor holds no lock here");
    let note_out = note_out.expect("the concurrent note never exited");
    assert!(note_out.status.success(), "{}", String::from_utf8_lossy(&note_out.stderr));
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

Run: `cargo test --bin tasks --test cli -- a_write_command_waits simultaneous concurrent a_failed the_reported_sequence`
Expected: PASS. Then repeat, since a concurrency test that passes once proves less than one
that passes twenty times:
`for i in $(seq 20); do cargo test --bin tasks --test cli -- a_write_command_waits simultaneous concurrent || break; done`

- [ ] **Step 3: Commit**

```bash
git add tests/cli.rs
git commit -m "test(claims): force lock contention and cover rollback"
```

---

### Task 12: Documentation and closeout

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

`docs/specs/2026-08-29-tasks-design.md:112` describes `owner` as the whole claim story. Add a
pointer so the two docs do not drift:

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

**Spec coverage:** Store and prefix keying → Task 1. Liveness and load-time timestamp
validation → Task 2. Locking → Tasks 3, 4, 9. Identity → Task 5. Snapshot and JSON contract →
Task 6. Command behaviour, destination-based release, write ordering, pruning, `--force` →
Task 7. `ready`/`prime` and all three warnings → Task 8. `feedback` recurrence → Task 9. The
tasks-8f4b41 warning → Task 10. Contention and rollback → Task 11. Docs and the "Known gaps"
recheck → Task 12. The three known gaps are deliberately unimplemented and recorded as such.

**Every commit is green.** `src/claims.rs` carries a module-level `#![allow(dead_code)]` from
Task 1 until Task 7 deletes it, because this is a binary crate: a `pub` item unreachable from
`main` fails `clippy -D warnings`, and the pre-commit hook runs `just check`.

That allowance covers `claims.rs` and nothing else, so anything it does not cover has to land
with its consumer. Task 4 therefore adds only `Ctx.lock` and `open_write_ctx`; `ClaimIntent`,
the `claims` and `pending_claim` fields and `claims_mut()` all live in `commands/mod.rs` and
move to Task 7. For the same reason `ClaimSnapshot::live` and `::stale` are held back from
Task 6 to Task 8 — Task 7 removes the allowance, and an unused method would fail the gate in
between. Task 7's Step 4 proves clippy is clean once the attribute is gone.

No task commits a test that a later task is required to make pass — `--force` moved into Task
7 with the guard that already takes it, and Task 6 precedes the guard so its output contract
exists first.

**Dependency order:** 1 → 2 → {3, 6}; 3 → {4, 9}; {2, 4, 5, 6} → 7; 7 → {8, 10}; {7, 8, 10} →
11; everything → 12.

**Type consistency:** `ClaimStore::{path_with, path_for, load, load_from, save, get, insert,
remove, prune_with, prune_dead, iter}`, `Claim`, `Liveness::{Live, Stale}`,
`ProcStat::{NotFound, Unreadable, Found}`, `parse_proc_stat`, `Identity {session, pid}`,
`identity() -> Result<Identity>`, `MutationLock::{path_with, path_for, acquire, acquire_at}`,
`ClaimSnapshot::{load, get}` (Task 6) and `::{live, stale}` (Task 8),
`ClaimInfo::of(&Claim, &Liveness)`,
`ClaimIntent::{Acquire, Release}`, `Error::Claimed` are used under those exact names
throughout. `TaskSummary::of` takes three parameters from Task 6 onward; `ready_tasks` takes
the snapshot from Task 8 onward; `transition` and `save` take `&mut Ctx` from Task 7 onward.
Test helpers: `TestEnv::{raw, cmd, claim_store, init_forced}` (Tasks 1 and 7), `write_claim`
(Task 6), `two_roots`/`as_agent`/`err_kind`/`err_detail` (Task 7), `hold_project_lock`,
`wait_bounded` and `reap` (Task 11) — every process wait in the suite goes through one of the
last two, so no regression can hang the gate instead of failing it. `TestEnv::raw` pipes both streams, without which
`wait_with_output` on a spawned child returns empty buffers and every `err_kind` assertion on
one fails.
