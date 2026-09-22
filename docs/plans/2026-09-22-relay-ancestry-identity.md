# Relay Ancestry Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in identity level that names a tasks session by the relay agent it runs under, adopting that agent's process handle as claim proof, without changing liveness or the claim file format.

**Architecture:** A new `src/relay/` module resolves identity in four stages — explicit override, configuration, platform support, ancestry — and only then reads relay's `agents.json` to verify the nearest harness ancestor. `claims::identity_from` gains that level as a lazily-invoked closure, so the stages cannot run out of order. Claim ownership is then decided in two ways rather than one: by identity when it resolves, and otherwise by re-deriving the claim's own recorded process proof locally, which needs no registry.

**Tech Stack:** Rust 2024, `serde`/`serde_json` (already dependencies) for the schema-1 snapshot, `toml` for the host config, `/proc` for ancestry and process start times. No new dependencies.

**Spec:** `docs/specs/2026-09-22-relay-ancestry-identity-design.md`

## Global Constraints

- Relay identity is **opt-in**. With it off, behaviour is byte-for-byte today's. Every task that touches an existing path carries a test asserting that.
- The four stages run in this order and each is reached only by passing the one before: explicit override (`TASKS_SESSION`) → configuration → platform support → ancestry. Spec §7.
- Liveness is not modified. `claims::liveness` and `liveness_with` keep their current signatures and bodies.
- The claim file format does not change. Adopted proof goes into the existing `pid`, `pid_start`, `boot_id`, `host` fields.
- Registry reads are Rust-only. Tasks never spawns Node and never writes to `agents.json`.
- Recognized harness `comm` names and their agent harnesses: `claude → claude-code`, `codex → codex`, `opencode → opencode`. Exactly these three.
- Only `platform: "linux"` handles are adopted. A Darwin handle must parse without error and then be refused as an identity candidate.
- Every takeover requires a resolved identity. `--force` displaces an owner; it never substitutes for one.
- JSON output shapes do not change in this plan.
- Run `just test-fast <name>` while working and `just gate` before the final commit of each task. Never run `cargo test` directly.

---

### Task 1: Host configuration file

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (add `mod config;`)
- Test: `src/config.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `crate::error::{Error, Result}`.
- Produces: `config::HostConfig { relay_identity: bool }`, `HostConfig::path() -> Result<PathBuf>`, `HostConfig::load() -> Result<HostConfig>`, `HostConfig::parse(text: &str, path: &str) -> Result<HostConfig>`.

- [ ] **Step 1: Write the failing tests**

Add to a new `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_without_the_key_leaves_relay_off() {
        assert_eq!(
            HostConfig::parse("", "c.toml").unwrap(),
            HostConfig { relay_identity: false }
        );
        assert_eq!(
            HostConfig::parse("[identity]\n", "c.toml").unwrap(),
            HostConfig { relay_identity: false }
        );
    }

    #[test]
    fn the_relay_key_is_read() {
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = true\n", "c.toml").unwrap(),
            HostConfig { relay_identity: true }
        );
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = false\n", "c.toml").unwrap(),
            HostConfig { relay_identity: false }
        );
    }

    #[test]
    fn a_typo_is_loud_rather_than_silently_off() {
        for text in [
            "[identity]\nrelayy = true\n",   // unknown key in the table
            "[identityy]\nrelay = true\n",   // unknown table
            "[identity]\nrelay = \"yes\"\n", // wrong type
            "[identity\nrelay = true\n",     // malformed
        ] {
            let error = HostConfig::parse(text, "c.toml").unwrap_err().to_string();
            assert!(error.contains("c.toml"), "{text:?}: {error}");
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast config`
Expected: FAIL — `src/config.rs` does not exist, or `HostConfig` is not defined.

- [ ] **Step 3: Write the implementation**

At the top of `src/config.rs`, above the test module:

```rust
use crate::error::{Error, Result};
use std::path::PathBuf;

/// The host-local configuration, `$XDG_CONFIG_HOME/tasks/config.toml` or
/// `$HOME/.config/tasks/config.toml`. Host-local deliberately: relay availability is a
/// property of a machine, and the per-project `tasks/.config.toml` is committed and
/// syncs between hosts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostConfig {
    pub relay_identity: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    #[serde(default)]
    relay: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileShape {
    #[serde(default)]
    identity: FileIdentity,
}

impl HostConfig {
    pub fn path() -> Result<PathBuf> {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home).join("tasks/config.toml"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".config/tasks/config.toml"));
        }
        Err(Error::Config(
            "neither XDG_CONFIG_HOME nor HOME is set".into(),
        ))
    }

    /// An absent file is not an error: every field takes its default. A present but
    /// unreadable or invalid file is, because it was written on purpose.
    pub fn load() -> Result<HostConfig> {
        let path = HostConfig::path()?;
        let shown = path.display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => HostConfig::parse(&text, &shown),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(HostConfig::default())
            }
            Err(error) => Err(Error::Io(format!("{shown}: {error}"))),
        }
    }

    pub fn parse(text: &str, path: &str) -> Result<HostConfig> {
        let file: FileShape =
            toml::from_str(text).map_err(|error| Error::Config(format!("{path}: {error}")))?;
        Ok(HostConfig {
            relay_identity: file.identity.relay,
        })
    }
}
```

Add `mod config;` to `src/main.rs` beside the other `mod` declarations.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast config`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat(config): a host-local tasks config.toml with the relay identity opt-in"
```

---

### Task 2: Relay snapshot reader

**Files:**
- Create: `src/relay/mod.rs`, `src/relay/snapshot.rs`
- Modify: `src/main.rs` (add `mod relay;`)
- Test: `src/relay/snapshot.rs` (inline `mod tests`)

The spec's §8 sketches a single `src/relay.rs`; a three-file `src/relay/` directory is the same decomposition with one responsibility per file, following the existing `src/rename/` precedent.

**Interfaces:**
- Consumes: `crate::error::{Error, Result}`.
- Produces:
  - `relay::snapshot::Handle { platform: String, host: String, boot_id: Option<String>, pid: u32, start: u64 }`
  - `relay::snapshot::Agent { id: String, harness: String, session_id: String, process: Option<Handle> }`
  - `relay::snapshot::Snapshot { agents: Vec<Agent> }`
  - `relay::snapshot::path() -> Result<PathBuf>`
  - `relay::snapshot::parse(text: &str) -> Result<Snapshot>`
  - `relay::snapshot::load() -> Result<Snapshot>`

- [ ] **Step 1: Write the failing tests**

In `src/relay/snapshot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn linux_agent(id: &str, harness: &str, session: &str, pid: u32, start: &str) -> String {
        format!(
            r#""{id}": {{"id":"{id}","harness":"{harness}","sessionId":"{session}",
              "scope":"session","cwd":"/w","repoRoot":null,"remote":null,
              "projectKey":"k","project":"p","state":"idle","updatedAt":1,
              "process":{{"platform":"linux","host":"testhost",
                "bootId":"0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01","pid":{pid},"start":"{start}"}}}}"#
        )
    }

    fn snapshot_of(agents: &str) -> String {
        format!(
            r#"{{"schema":1,"generation":"11111111-2222-4333-8444-555555555555",
               "revision":7,"agents":{{{agents}}}}}"#
        )
    }

    #[test]
    fn a_valid_snapshot_parses_every_field() {
        let text = snapshot_of(&linux_agent("codex:s1", "codex", "s1", 42, "998877"));
        let snapshot = parse(&text).unwrap();
        assert_eq!(snapshot.agents.len(), 1);
        let agent = &snapshot.agents[0];
        assert_eq!(agent.id, "codex:s1");
        assert_eq!(agent.harness, "codex");
        assert_eq!(agent.session_id, "s1");
        let process = agent.process.as_ref().unwrap();
        assert_eq!(process.platform, "linux");
        assert_eq!(process.host, "testhost");
        assert_eq!(process.pid, 42);
        assert_eq!(process.start, 998877);
        assert!(process.boot_id.is_some());
    }

    #[test]
    fn a_darwin_handle_parses_and_keeps_its_platform() {
        // Parse-only: refusing it as an identity candidate is Task 4's job, and it must
        // not be refused here by being unreadable.
        let text = snapshot_of(
            r#""codex:d1": {"id":"codex:d1","harness":"codex","sessionId":"d1",
               "scope":"session","cwd":"/w","repoRoot":null,"remote":null,
               "projectKey":"k","project":"p","state":"idle","updatedAt":1,
               "process":{"platform":"darwin","host":"mac","bootId":null,
                 "pid":9,"start":"1750000000"}}"#,
        );
        let snapshot = parse(&text).unwrap();
        assert_eq!(snapshot.agents[0].process.as_ref().unwrap().platform, "darwin");
    }

    #[test]
    fn the_largest_linux_start_is_a_format_case_not_an_overflow() {
        let text = snapshot_of(&linux_agent("codex:s2", "codex", "s2", 42, "18446744073709551615"));
        assert_eq!(
            parse(&text).unwrap().agents[0].process.as_ref().unwrap().start,
            u64::MAX
        );
        let over = snapshot_of(&linux_agent("codex:s3", "codex", "s3", 42, "18446744073709551616"));
        assert!(parse(&over).is_err());
    }

    #[test]
    fn a_null_process_handle_is_valid() {
        let text = snapshot_of(
            r#""codex:n1": {"id":"codex:n1","harness":"codex","sessionId":"n1",
               "scope":"session","cwd":"/w","repoRoot":null,"remote":null,
               "projectKey":"k","project":"p","state":"idle","updatedAt":1,
               "process":null}"#,
        );
        assert!(parse(&text).unwrap().agents[0].process.is_none());
    }

    #[test]
    fn schema_violations_are_refused() {
        let cases = [
            snapshot_of(&linux_agent("codex:s1", "codex", "MISMATCH", 42, "1")), // id != harness:session
            snapshot_of(&linux_agent("wrong-key", "codex", "s1", 42, "1")),      // map key != id
            snapshot_of(&linux_agent("nope:s1", "nope", "s1", 42, "1")),         // unknown harness
            snapshot_of(&linux_agent("codex:s1", "codex", "s1", 0, "1")),        // pid out of range
            snapshot_of(&linux_agent("codex:s1", "codex", "s1", 42, "007")),     // non-canonical start
            r#"{"schema":2,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#.into(),
            "not json".into(),
        ];
        for text in cases {
            assert!(parse(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn an_empty_registry_is_a_valid_empty_snapshot() {
        let text = snapshot_of("");
        assert!(parse(&text).unwrap().agents.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast snapshot`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

`src/relay/mod.rs`:

```rust
pub mod snapshot;
```

Top of `src/relay/snapshot.rs`:

```rust
use crate::error::{Error, Result};
use std::path::PathBuf;

/// relay's published process handle. `start` is opaque here: on Linux it is the same
/// `starttime` tick count `/proc/<pid>/stat` reports, and on Darwin an epoch, which is
/// why the platform travels with it and why only Linux handles are ever adopted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle {
    pub platform: String,
    pub host: String,
    pub boot_id: Option<String>,
    pub pid: u32,
    pub start: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub harness: String,
    pub session_id: String,
    pub process: Option<Handle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub agents: Vec<Agent>,
}

/// The three harnesses relay publishes. Tasks refuses any other value rather than
/// carrying a session it cannot map back to a `comm`.
const HARNESSES: [&str; 3] = ["claude-code", "codex", "opencode"];

fn invalid(detail: &str) -> Error {
    Error::Config(format!("relay registry: {detail}"))
}

/// `$RELAY_STATE_DIR`, else `$XDG_STATE_HOME/relay`, else `$HOME/.local/state/relay`.
pub fn path() -> Result<PathBuf> {
    let dir = if let Some(dir) = std::env::var_os("RELAY_STATE_DIR") {
        PathBuf::from(dir)
    } else if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(state_home).join("relay")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".local/state/relay")
    } else {
        return Err(invalid(
            "neither RELAY_STATE_DIR, XDG_STATE_HOME nor HOME is set",
        ));
    };
    if !dir.is_absolute() {
        return Err(invalid(&format!("{} is not absolute", dir.display())));
    }
    Ok(dir.join("agents.json"))
}

pub fn load() -> Result<Snapshot> {
    let path = path()?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| invalid(&format!("{}: {error}", path.display())))?;
    parse(&text)
}

/// Canonical decimal u64 text, exactly as relay's `parseStart` produces it: no leading
/// zeros, no sign, no separators.
fn parse_start(value: &str) -> Result<u64> {
    let canonical = value == "0"
        || (value.starts_with(|c: char| c.is_ascii_digit() && c != '0')
            && value.bytes().all(|b| b.is_ascii_digit()));
    if !canonical {
        return Err(invalid(&format!("start {value:?} is not canonical decimal")));
    }
    value
        .parse()
        .map_err(|_| invalid(&format!("start {value:?} does not fit in u64")))
}

pub fn parse(text: &str) -> Result<Snapshot> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|error| invalid(&error.to_string()))?;
    if raw.get("schema").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(invalid("schema must be 1"));
    }
    let map = raw
        .get("agents")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("agents must be an object"))?;

    let mut agents = Vec::with_capacity(map.len());
    for (key, value) in map {
        let text_at = |field: &str| {
            value
                .get(field)
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string)
        };
        let id = text_at("id").ok_or_else(|| invalid("agent id must be text"))?;
        let harness = text_at("harness").ok_or_else(|| invalid("harness must be text"))?;
        let session_id = text_at("sessionId").ok_or_else(|| invalid("sessionId must be text"))?;
        if !HARNESSES.contains(&harness.as_str()) {
            return Err(invalid(&format!("unknown harness {harness:?}")));
        }
        if id != format!("{harness}:{session_id}") || id != *key {
            return Err(invalid(&format!("agent {key:?} has an inconsistent id")));
        }

        let process = match value.get("process") {
            None | Some(serde_json::Value::Null) => None,
            Some(handle) => {
                let handle_text = |field: &str| {
                    handle
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                };
                let platform = handle_text("platform")
                    .filter(|p| p == "linux" || p == "darwin")
                    .ok_or_else(|| invalid("platform must be linux or darwin"))?;
                let host =
                    handle_text("host").ok_or_else(|| invalid("handle host must be text"))?;
                let pid = handle
                    .get("pid")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|pid| *pid >= 1 && *pid <= u32::MAX as u64)
                    .ok_or_else(|| invalid("handle pid out of range"))? as u32;
                let start = parse_start(
                    &handle_text("start").ok_or_else(|| invalid("start must be text"))?,
                )?;
                let boot_id = match handle.get("bootId") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(value) => {
                        let id = value
                            .as_str()
                            .ok_or_else(|| invalid("bootId must be text or null"))?;
                        if platform != "linux" {
                            return Err(invalid("only a linux handle carries a bootId"));
                        }
                        Some(id.to_string())
                    }
                };
                Some(Handle {
                    platform,
                    host,
                    boot_id,
                    pid,
                    start,
                })
            }
        };
        agents.push(Agent {
            id,
            harness,
            session_id,
            process,
        });
    }
    agents.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Snapshot { agents })
}
```

Add `mod relay;` to `src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast snapshot`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/relay/ src/main.rs
git commit -m "feat(relay): read and validate the schema-1 agent registry in Rust"
```

---

### Task 3: Ancestry walk

**Files:**
- Create: `src/relay/ancestry.rs`
- Modify: `src/relay/mod.rs` (add `pub mod ancestry;`)
- Test: `src/relay/ancestry.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `relay::ancestry::ProcEntry { pid: u32, ppid: u32, comm: String, start: u64 }`
  - `relay::ancestry::Scope` — `Harness(ProcEntry)` | `Outside` | `Unknown(u32)`
  - `relay::ancestry::HARNESS_COMMS: [(&str, &str); 3]` — `comm` to agent `harness`
  - `relay::ancestry::parse_entry(line: &str) -> Option<ProcEntry>`
  - `relay::ancestry::walk(from: u32, read: impl Fn(u32) -> Option<String>) -> Scope`
  - `relay::ancestry::read_stat(pid: u32) -> Option<String>`
  - `relay::ancestry::self_ppid() -> Option<u32>`

The walk is shared: the scope test of spec §4.2 and the ownership proof of §6.2 both need the same nearest boundary.

- [ ] **Step 1: Write the failing tests**

In `src/relay/ancestry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// `pid (comm) state ppid …` with starttime at remainder index 19.
    fn stat(pid: u32, comm: &str, ppid: u32, start: u64) -> String {
        let filler = (0..17)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        format!("{pid} ({comm}) S {ppid} {filler} {start} x")
    }

    fn tree(rows: &[(u32, &str, u32, u64)]) -> impl Fn(u32) -> Option<String> {
        let map: HashMap<u32, String> = rows
            .iter()
            .map(|(pid, comm, ppid, start)| (*pid, stat(*pid, comm, *ppid, *start)))
            .collect();
        move |pid| map.get(&pid).cloned()
    }

    #[test]
    fn an_entry_parses_a_comm_containing_spaces_and_parentheses() {
        let entry = parse_entry("7 (weird ) name) S 3 a b c d e f g h i j k l m n o p q 4242 x")
            .unwrap();
        assert_eq!(entry.pid, 7);
        assert_eq!(entry.comm, "weird ) name");
        assert_eq!(entry.ppid, 3);
        assert_eq!(entry.start, 4242);
    }

    #[test]
    fn the_nearest_harness_ancestor_wins() {
        // tasks(10) -> codex(9) -> claude(8) -> sh(2) -> init(1)
        let read = tree(&[
            (9, "codex", 8, 900),
            (8, "claude", 2, 800),
            (2, "sh", 1, 200),
            (1, "init", 0, 1),
        ]);
        match walk(9, read) {
            Scope::Harness(entry) => {
                assert_eq!(entry.comm, "codex");
                assert_eq!(entry.pid, 9);
                assert_eq!(entry.start, 900);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_plain_shell_chain_is_outside() {
        let read = tree(&[(5, "zsh", 2, 500), (2, "systemd", 1, 200), (1, "init", 0, 1)]);
        assert!(matches!(walk(5, read), Scope::Outside));
    }

    #[test]
    fn an_unreadable_level_is_unknown_not_outside() {
        // 5 is readable, its parent 4 is not: the verdict is unknown, never Outside.
        let read = tree(&[(5, "zsh", 4, 500)]);
        assert!(matches!(walk(5, read), Scope::Unknown(4)));
    }

    #[test]
    fn every_recognized_comm_is_matched() {
        for (comm, harness) in HARNESS_COMMS {
            let read = tree(&[(9, comm, 1, 900), (1, "init", 0, 1)]);
            match walk(9, read) {
                Scope::Harness(entry) => assert_eq!(entry.comm, comm),
                other => panic!("{comm} -> {other:?}"),
            }
            assert!(!harness.is_empty());
        }
    }

    #[test]
    fn a_cyclic_chain_terminates() {
        let read = tree(&[(5, "zsh", 6, 500), (6, "zsh", 5, 600)]);
        assert!(matches!(walk(5, read), Scope::Unknown(_)));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast ancestry`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

Top of `src/relay/ancestry.rs`:

```rust
/// One process on the caller's ancestry chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcEntry {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub start: u64,
}

/// What the walk established about the caller's position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The nearest recognized harness ancestor.
    Harness(ProcEntry),
    /// The whole chain was readable and held no harness.
    Outside,
    /// The chain could not be read at this pid. Not evidence of a plain shell.
    Unknown(u32),
}

/// `comm` as the kernel reports it, and the agent `harness` relay publishes for it.
/// The `tty` predicate relay's own adapters apply belongs to qualified process
/// resolution, not to this test: a headless harness must not become a shell caller.
pub const HARNESS_COMMS: [(&str, &str); 3] = [
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

/// A chain longer than this is malformed; walking it forever is not an option.
const MAX_DEPTH: usize = 64;

/// The `comm` field can contain spaces and parentheses, so everything after the *last*
/// `)` is positional: state is field 1, ppid field 2, start time field 20 — the same
/// rule `claims::parse_proc_stat` documents for fields 3 and 22 of the whole line.
pub fn parse_entry(line: &str) -> Option<ProcEntry> {
    let pid = line.split_whitespace().next()?.parse().ok()?;
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    let comm = line.get(open + 1..close)?.to_string();
    let rest: Vec<&str> = line.get(close + 1..)?.split_whitespace().collect();
    Some(ProcEntry {
        pid,
        ppid: rest.get(1)?.parse().ok()?,
        comm,
        start: rest.get(19)?.parse().ok()?,
    })
}

pub fn read_stat(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
}

pub fn self_ppid() -> Option<u32> {
    parse_entry(&read_stat_self()?).map(|entry| entry.ppid)
}

fn read_stat_self() -> Option<String> {
    std::fs::read_to_string("/proc/self/stat").ok()
}

/// Walk upward from `from`, returning the nearest harness ancestor. An unreadable level
/// short-circuits to `Unknown`: unknown ancestry is never reported as `Outside`.
pub fn walk(from: u32, read: impl Fn(u32) -> Option<String>) -> Scope {
    let mut pid = from;
    let mut seen = Vec::with_capacity(MAX_DEPTH);
    for _ in 0..MAX_DEPTH {
        if pid == 0 {
            return Scope::Outside;
        }
        if seen.contains(&pid) {
            return Scope::Unknown(pid);
        }
        seen.push(pid);
        let Some(line) = read(pid) else {
            return Scope::Unknown(pid);
        };
        let Some(entry) = parse_entry(&line) else {
            return Scope::Unknown(pid);
        };
        if HARNESS_COMMS.iter().any(|(comm, _)| *comm == entry.comm) {
            return Scope::Harness(entry);
        }
        if entry.pid == 1 {
            return Scope::Outside;
        }
        pid = entry.ppid;
    }
    Scope::Unknown(pid)
}
```

Add `pub mod ancestry;` to `src/relay/mod.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast ancestry`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/relay/
git commit -m "feat(relay): walk process ancestry to the nearest harness boundary"
```

---

### Task 4: The relay level — match, harness agreement, scoped hints, errors

**Files:**
- Create: `src/relay/resolve.rs`
- Modify: `src/relay/mod.rs` (add `pub mod resolve;`)
- Test: `src/relay/resolve.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `snapshot::{Snapshot, Agent, Handle}`, `ancestry::{Scope, ProcEntry, HARNESS_COMMS}`.
- Produces:
  - `relay::resolve::Resolved { session: String, pid: u32, pid_start: u64, boot_id: String, host: String }`
  - `relay::resolve::hint_for(comm: &str, get: &impl Fn(&str) -> Option<String>) -> Result<Option<String>>`
  - `relay::resolve::same_session(claim_session: &str, comm: &str, session_id: &str) -> bool`
  - `relay::resolve::resolve(scope: Scope, snapshot: &Snapshot, host: &str, boot_id: Option<&str>, get: &impl Fn(&str) -> Option<String>) -> Result<Option<Resolved>>` — `Ok(None)` means out of scope.

- [ ] **Step 1: Write the failing tests**

In `src/relay/resolve.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::snapshot::{Agent, Handle, Snapshot};

    const BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";

    fn ancestor(comm: &str, pid: u32, start: u64) -> Scope {
        Scope::Harness(ProcEntry {
            pid,
            ppid: 1,
            comm: comm.into(),
            start,
        })
    }

    fn agent(harness: &str, session: &str, platform: &str, pid: u32, start: u64) -> Agent {
        Agent {
            id: format!("{harness}:{session}"),
            harness: harness.into(),
            session_id: session.into(),
            process: Some(Handle {
                platform: platform.into(),
                host: "testhost".into(),
                boot_id: (platform == "linux").then(|| BOOT.to_string()),
                pid,
                start,
            }),
        }
    }

    fn snapshot(agents: Vec<Agent>) -> Snapshot {
        Snapshot { agents }
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| {
            owned
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty())
        }
    }

    fn go(scope: Scope, snap: &Snapshot, e: &impl Fn(&str) -> Option<String>) -> Result<Option<Resolved>> {
        resolve(scope, snap, "testhost", Some(BOOT), e)
    }

    #[test]
    fn an_exact_match_adopts_the_agent_id_and_its_proof() {
        let snap = snapshot(vec![agent("codex", "s1", "linux", 42, 900)]);
        let resolved = go(ancestor("codex", 42, 900), &snap, &env(&[]))
            .unwrap()
            .unwrap();
        assert_eq!(resolved.session, "codex:s1");
        assert_eq!(resolved.pid, 42);
        assert_eq!(resolved.pid_start, 900);
        assert_eq!(resolved.boot_id, BOOT);
        assert_eq!(resolved.host, "testhost");
    }

    #[test]
    fn out_of_scope_yields_none_without_consulting_the_snapshot() {
        assert!(go(Scope::Outside, &snapshot(vec![]), &env(&[]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn unknown_ancestry_is_an_error_not_a_shell() {
        assert!(go(Scope::Unknown(7), &snapshot(vec![]), &env(&[])).is_err());
    }

    #[test]
    fn the_harness_must_agree_with_the_ancestor_comm() {
        // Satisfies host, pid, start and boot for a codex ancestor, but is a claude-code
        // row. Without the harness comparison this would be adopted and key the claim to
        // the wrong session.
        let snap = snapshot(vec![agent("claude-code", "c1", "linux", 42, 900)]);
        let error = go(ancestor("codex", 42, 900), &snap, &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("codex"), "{error}");
    }

    #[test]
    fn a_darwin_handle_is_refused_as_an_identity_candidate() {
        let snap = snapshot(vec![agent("codex", "s1", "darwin", 42, 900)]);
        let error = go(ancestor("codex", 42, 900), &snap, &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("darwin"), "{error}");
    }

    #[test]
    fn proof_must_match_in_every_component() {
        let snap = snapshot(vec![agent("codex", "s1", "linux", 42, 900)]);
        assert!(go(ancestor("codex", 43, 900), &snap, &env(&[])).is_err()); // pid
        assert!(go(ancestor("codex", 42, 901), &snap, &env(&[])).is_err()); // start
        assert!(resolve(ancestor("codex", 42, 900), &snap, "other", Some(BOOT), &env(&[])).is_err()); // host
        assert!(resolve(
            ancestor("codex", 42, 900),
            &snap,
            "testhost",
            Some("22222222-2222-4222-8222-222222222222"),
            &env(&[])
        )
        .is_err()); // boot
    }

    #[test]
    fn an_empty_registry_under_a_live_harness_is_the_explicit_identity_error() {
        // The real Codex case: SessionStart runs at the first turn, not at launch.
        let error = go(ancestor("codex", 42, 900), &snapshot(vec![]), &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn two_agents_on_one_handle_are_ambiguous() {
        let snap = snapshot(vec![
            agent("codex", "s1", "linux", 42, 900),
            agent("codex", "s2", "linux", 42, 900),
        ]);
        let error = go(ancestor("codex", 42, 900), &snap, &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("codex:s1") && error.contains("codex:s2"), "{error}");
    }

    #[test]
    fn a_hint_from_another_harness_is_ignored() {
        // A codex session nested under a claude one inherits CLAUDE_CODE_SESSION_ID.
        // Its own proof and its own variable are valid and must be accepted.
        let snap = snapshot(vec![agent("codex", "s1", "linux", 42, 900)]);
        let e = env(&[("CLAUDE_CODE_SESSION_ID", "outer"), ("CODEX_SESSION_ID", "s1")]);
        assert_eq!(
            go(ancestor("codex", 42, 900), &snap, &e).unwrap().unwrap().session,
            "codex:s1"
        );
    }

    #[test]
    fn a_hint_for_the_nearest_harness_must_agree() {
        let snap = snapshot(vec![agent("claude-code", "c1", "linux", 42, 900)]);
        let e = env(&[("CLAUDE_CODE_SESSION_ID", "different")]);
        assert!(go(ancestor("claude", 42, 900), &snap, &e).is_err());
        let ok = env(&[("CLAUDE_CODE_SESSION_ID", "c1")]);
        assert!(go(ancestor("claude", 42, 900), &snap, &ok).unwrap().is_some());
    }

    #[test]
    fn the_two_codex_variables_must_agree_with_each_other() {
        let snap = snapshot(vec![agent("codex", "s1", "linux", 42, 900)]);
        let e = env(&[("CODEX_SESSION_ID", "s1"), ("CODEX_THREAD_ID", "other")]);
        assert!(go(ancestor("codex", 42, 900), &snap, &e).is_err());
        // Either alone is the one compared.
        let only_thread = env(&[("CODEX_THREAD_ID", "s1")]);
        assert!(go(ancestor("codex", 42, 900), &snap, &only_thread).unwrap().is_some());
    }

    #[test]
    fn an_empty_variable_is_unset() {
        let snap = snapshot(vec![agent("claude-code", "c1", "linux", 42, 900)]);
        let e = env(&[("CLAUDE_CODE_SESSION_ID", "")]);
        assert!(go(ancestor("claude", 42, 900), &snap, &e).unwrap().is_some());
    }

    #[test]
    fn opencode_compares_no_variable() {
        let snap = snapshot(vec![agent("opencode", "o1", "linux", 42, 900)]);
        let e = env(&[("CLAUDE_CODE_SESSION_ID", "x"), ("CODEX_SESSION_ID", "y")]);
        assert!(go(ancestor("opencode", 42, 900), &snap, &e).unwrap().is_some());
    }

    #[test]
    fn known_representations_of_one_session_compare_equal() {
        for stored in ["c1", "claude:c1", "claude-code:c1"] {
            assert!(same_session(stored, "claude", "c1"), "{stored}");
        }
        for stored in ["s1", "codex:s1"] {
            assert!(same_session(stored, "codex", "s1"), "{stored}");
        }
        // Anything else is a genuine mismatch, never a change of notation.
        assert!(!same_session("claude-code:other", "claude", "c1"));
        assert!(!same_session("sid:4242", "claude", "c1"));
        assert!(!same_session("codex:c1", "claude", "c1"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast resolve`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

Top of `src/relay/resolve.rs`:

```rust
use crate::error::{Error, Result};
use crate::relay::ancestry::{HARNESS_COMMS, ProcEntry, Scope};
use crate::relay::snapshot::Snapshot;

/// A matched agent: its id becomes the claim session, its handle the claim's proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub session: String,
    pub pid: u32,
    pub pid_start: u64,
    pub boot_id: String,
    pub host: String,
}

const SET_EXPLICIT: &str = "set TASKS_SESSION and TASKS_SESSION_PID to name this session explicitly";

fn refuse(detail: String) -> Error {
    Error::Config(format!("relay identity: {detail}; {SET_EXPLICIT}"))
}

fn harness_for(comm: &str) -> Option<&'static str> {
    HARNESS_COMMS
        .iter()
        .find(|(name, _)| *name == comm)
        .map(|(_, harness)| *harness)
}

/// The session hint belonging to `comm`, and only to it. A nested harness inherits its
/// parent's environment, so a variable from another harness says nothing about this one.
pub fn hint_for(comm: &str, get: &impl Fn(&str) -> Option<String>) -> Result<Option<String>> {
    match comm {
        "claude" => Ok(get("CLAUDE_CODE_SESSION_ID")),
        "codex" => match (get("CODEX_SESSION_ID"), get("CODEX_THREAD_ID")) {
            (Some(session), Some(thread)) if session != thread => Err(refuse(format!(
                "CODEX_SESSION_ID {session:?} conflicts with CODEX_THREAD_ID {thread:?}"
            ))),
            (Some(value), _) | (None, Some(value)) => Ok(Some(value)),
            (None, None) => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Whether `claim_session` names the session `session_id` under `comm`'s harness. The
/// same session is written more than one way across the levels: natively a Claude claim
/// stores the raw id, its tagged form is `claude:<id>`, and a relay agent id is
/// `claude-code:<id>`. Exactly those known forms compare equal; any other difference is
/// a real mismatch, never a change of notation.
pub fn same_session(claim_session: &str, comm: &str, session_id: &str) -> bool {
    let mut forms = vec![session_id.to_string()];
    if let Some(harness) = harness_for(comm) {
        forms.push(format!("{harness}:{session_id}"));
    }
    if comm == "claude" {
        forms.push(format!("claude:{session_id}"));
    }
    forms.iter().any(|form| form == claim_session)
}

/// `Ok(None)` means the caller is out of scope and the native ladder applies unchanged.
pub fn resolve(
    scope: Scope,
    snapshot: &Snapshot,
    host: &str,
    boot_id: Option<&str>,
    get: &impl Fn(&str) -> Option<String>,
) -> Result<Option<Resolved>> {
    let nearest: ProcEntry = match scope {
        Scope::Outside => return Ok(None),
        Scope::Unknown(pid) => {
            return Err(refuse(format!(
                "cannot read /proc/{pid}/stat, so this caller's ancestry is unknown"
            )));
        }
        Scope::Harness(entry) => entry,
    };
    let harness = harness_for(&nearest.comm)
        .ok_or_else(|| refuse(format!("unrecognized harness {:?}", nearest.comm)))?;
    let boot_id = boot_id.ok_or_else(|| refuse("the host boot id is unreadable".into()))?;

    let matched: Vec<&crate::relay::snapshot::Agent> = snapshot
        .agents
        .iter()
        .filter(|agent| {
            agent.process.as_ref().is_some_and(|process| {
                process.host == host
                    && process.pid == nearest.pid
                    && process.start == nearest.start
            })
        })
        .collect();

    let agent = match matched.as_slice() {
        [] => {
            return Err(refuse(format!(
                "no relay agent matches the nearest {} ancestor, pid {}",
                nearest.comm, nearest.pid
            )));
        }
        [one] => *one,
        many => {
            let ids: Vec<&str> = many.iter().map(|agent| agent.id.as_str()).collect();
            return Err(refuse(format!(
                "several relay agents match pid {}: {}",
                nearest.pid,
                ids.join(", ")
            )));
        }
    };

    if agent.harness != harness {
        return Err(refuse(format!(
            "the agent matching pid {} is {:?}, but the nearest ancestor comm {:?} maps to {:?}",
            nearest.pid, agent.harness, nearest.comm, harness
        )));
    }
    let process = agent.process.as_ref().expect("filtered on a present handle");
    if process.platform != "linux" {
        return Err(refuse(format!(
            "agent {} has a {:?} handle, which is not an identity candidate",
            agent.id, process.platform
        )));
    }
    if process.boot_id.as_deref() != Some(boot_id) {
        return Err(refuse(format!(
            "agent {} was recorded on an earlier boot",
            agent.id
        )));
    }
    if let Some(hint) = hint_for(&nearest.comm, get)?
        && hint != agent.session_id
    {
        return Err(refuse(format!(
            "the environment names session {hint:?} but the matched agent is {}",
            agent.id
        )));
    }

    Ok(Some(Resolved {
        session: agent.id.clone(),
        pid: process.pid,
        pid_start: process.start,
        boot_id: boot_id.to_string(),
        host: host.to_string(),
    }))
}
```

Add `pub mod resolve;` to `src/relay/mod.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast resolve`
Expected: PASS, 13 tests.

- [ ] **Step 5: Commit**

```bash
git add src/relay/
git commit -m "feat(relay): match the nearest harness ancestor and adopt its agent id"
```

---

### Task 5: The level in the ladder, adoption, and the platform refusal

**Files:**
- Modify: `src/claims.rs:8-88` (the `Identity` struct and `identity`/`identity_from`)
- Modify: `src/relay/mod.rs` (add the staged entry point)
- Modify: `src/commands/mod.rs:210-218` (claim field adoption)
- Test: `src/claims.rs` (inline `mod tests`), `src/relay/mod.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `relay::resolve::Resolved`, `config::HostConfig`, `relay::ancestry`, `relay::snapshot`.
- Produces:
  - `claims::Proof { pid_start: u64, boot_id: String, host: String }`
  - `claims::Identity` gains `pub proof: Option<Proof>`
  - `claims::identity_from(get, session_pid, relay: impl FnOnce() -> Result<Option<Resolved>>, warnings) -> Result<Identity>`
  - `relay::level() -> Result<Option<Resolved>>` — stages 2, 3 and 4 of spec §7
  - `claims::identity(warnings) -> Result<Identity>` keeps its signature, passing `relay::level`

- [ ] **Step 1: Write the failing tests**

Add to `src/claims.rs`'s `mod tests`:

```rust
    fn no_relay() -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
        || Ok(None)
    }

    fn relay_says(session: &str) -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
        let session = session.to_string();
        move || {
            Ok(Some(crate::relay::resolve::Resolved {
                session,
                pid: 42,
                pid_start: 900,
                boot_id: "boot".into(),
                host: "testhost".into(),
            }))
        }
    }

    #[test]
    fn the_explicit_pair_beats_relay_and_relay_is_never_consulted() {
        let identity = identity_from(
            env_of(&[("TASKS_SESSION", "explicit"), ("TASKS_SESSION_PID", "7")]),
            Some(11),
            || panic!("the relay level must not be reached when TASKS_SESSION is set"),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(identity.session, "explicit");
        assert_eq!(identity.pid, Some(7));
        assert!(identity.proof.is_none());
    }

    #[test]
    fn relay_beats_the_native_variables_and_carries_its_proof() {
        let identity = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "raw")]),
            Some(11),
            relay_says("claude-code:raw"),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(identity.session, "claude-code:raw");
        assert_eq!(identity.tagged, "claude-code:raw");
        assert_eq!(identity.pid, Some(42));
        let proof = identity.proof.unwrap();
        assert_eq!(proof.pid_start, 900);
        assert_eq!(proof.boot_id, "boot");
        assert_eq!(proof.host, "testhost");
    }

    #[test]
    fn out_of_scope_leaves_the_native_ladder_untouched() {
        let identity = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "raw")]),
            Some(11),
            no_relay(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(identity.session, "raw");
        assert_eq!(identity.tagged, "claude:raw");
        assert!(identity.proof.is_none());
    }

    #[test]
    fn a_relay_error_is_returned_rather_than_falling_to_a_lower_level() {
        let error = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "raw")]),
            Some(11),
            || Err(Error::Config("relay identity: no match".into())),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("no match"), "{error}");
    }
```

Add to a new `mod tests` in `src/relay/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsupported_platform_refuses_only_once_relay_is_enabled() {
        assert!(matches!(stage_platform(false, "macos"), Ok(())));
        let error = stage_platform(true, "macos").unwrap_err().to_string();
        assert!(error.contains("macos"), "{error}");
        assert!(stage_platform(true, "linux").is_ok());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast identity_ && just test-fast stage_platform`
Expected: FAIL — `identity_from` takes three arguments, `Identity` has no `proof`, `stage_platform` is undefined.

- [ ] **Step 3: Write the implementation**

In `src/claims.rs`, extend the identity types and the ladder:

```rust
/// Process proof adopted from a matched relay handle, written into the claim's existing
/// fields. Absent on every native level, which derives what it can from `pid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub pid_start: u64,
    pub boot_id: String,
    pub host: String,
}

pub struct Identity {
    pub session: String,
    pub pid: Option<u32>,
    pub tagged: String,
    pub proof: Option<Proof>,
}
```

Every existing `Identity { … }` literal in `claims.rs` gains `proof: None`. Then, in
`identity_from`, insert the relay level immediately after the `TASKS_SESSION` block and
before the `CLAUDE_CODE_SESSION_ID` block:

```rust
    // Stage 1 of spec §7 is above: an explicit pair short-circuits, so the relay level is
    // never invoked and its stages never run.
    if let Some(resolved) = relay()? {
        return Ok(Identity {
            tagged: resolved.session.clone(),
            session: resolved.session,
            pid: Some(resolved.pid),
            proof: Some(Proof {
                pid_start: resolved.pid_start,
                boot_id: resolved.boot_id,
                host: resolved.host,
            }),
        });
    }
```

and change the signature to:

```rust
pub fn identity_from(
    get: impl Fn(&str) -> Option<OsString>,
    session_pid: Option<u32>,
    relay: impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>>,
    warnings: &mut Vec<String>,
) -> Result<Identity> {
```

with `identity` passing the production level:

```rust
pub fn identity(warnings: &mut Vec<String>) -> Result<Identity> {
    identity_from(
        |key| std::env::var_os(key),
        unix_session_id(),
        crate::relay::level,
        warnings,
    )
}
```

In `src/relay/mod.rs`, add the staged entry point:

```rust
pub mod ancestry;
pub mod resolve;
pub mod snapshot;

use crate::config::HostConfig;
use crate::error::{Error, Result};

/// Stage 3 of spec §7. The refusal belongs to the relay level, not to startup: a session
/// with `TASKS_SESSION` set never reaches it, so it can never take away the recovery path.
pub fn stage_platform(enabled: bool, os: &str) -> Result<()> {
    if enabled && os != "linux" {
        return Err(Error::Config(format!(
            "relay identity is enabled but unsupported on {os}: the ancestry walk needs \
             /proc. Disable [identity].relay, or set TASKS_SESSION to name this session."
        )));
    }
    Ok(())
}

/// Stages 2, 3 and 4 of spec §7, in order. `Ok(None)` means relay is off or the caller
/// is out of scope; either way the native ladder applies unchanged.
pub fn level() -> Result<Option<resolve::Resolved>> {
    // Stage 2: configuration. Nothing below runs when relay is not enabled.
    if !HostConfig::load()?.relay_identity {
        return Ok(None);
    }
    // Stage 3: platform support.
    stage_platform(true, std::env::consts::OS)?;
    // Stage 4: ancestry, then the registry.
    let scope = match ancestry::self_ppid() {
        Some(ppid) => ancestry::walk(ppid, ancestry::read_stat),
        None => ancestry::Scope::Unknown(0),
    };
    if matches!(scope, ancestry::Scope::Outside) {
        return Ok(None);
    }
    let snapshot = snapshot::load()?;
    resolve::resolve(
        scope,
        &snapshot,
        &crate::claims::hostname(),
        crate::claims::boot_id().as_deref(),
        &|key| {
            std::env::var_os(key)
                .and_then(|value| value.into_string().ok())
                .filter(|value| !value.is_empty())
        },
    )
}
```

In `src/commands/mod.rs`, replace the `pid_start` derivation in `claim_guard` so adopted
proof is used when present:

```rust
                ClaimIntent::Acquire(crate::claims::Claim {
                    owner,
                    pid_start: match &me.proof {
                        Some(proof) => Some(proof.pid_start),
                        None => me.pid.and_then(|pid| match crate::claims::proc_stat(pid) {
                            crate::claims::ProcStat::Found { starttime, .. } => Some(starttime),
                            _ => None,
                        }),
                    },
                    boot_id: match &me.proof {
                        Some(proof) => Some(proof.boot_id.clone()),
                        None => crate::claims::boot_id(),
                    },
                    host: match &me.proof {
                        Some(proof) => proof.host.clone(),
                        None => crate::claims::hostname(),
                    },
                    session: me.session,
                    pid: me.pid,
                    worktree,
                    started,
                    seen: now,
                }),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast claims && just test-fast relay`
Expected: PASS. Every pre-existing `claims` test still passes unchanged.

- [ ] **Step 5: Commit**

```bash
git add src/claims.rs src/relay/ src/commands/mod.rs
git commit -m "feat(claims): add the opt-in relay level and adopt its process proof"
```

---

### Task 6: Continuity in the guards

**Files:**
- Modify: `src/claims.rs` (add `Resolution` and `proves_ownership`)
- Modify: `src/commands/mod.rs:120-137` (`refuse_foreign_live_claim`), `:168-200` (`claim_guard`)
- Modify: `src/commands/park.rs:38`
- Test: `src/claims.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `claims::{Claim, Identity, Proof}`, `relay::ancestry::{Scope, ProcEntry}`, `relay::resolve::same_session`.
- Produces:
  - `claims::Resolution` — `Resolved(Identity)` | `Failed(Error)`, with
    `Resolution::identity(&self) -> Option<&Identity>` and `Resolution::require(self) -> Result<Identity>`
  - `claims::resolve_identity(warnings: &mut Vec<String>) -> Resolution`
  - `claims::proves_ownership(claim: &Claim, scope: &Scope, host: &str, boot_id: Option<&str>, get: &impl Fn(&str) -> Option<String>) -> bool`
  - `Ctx::owns(&mut self, claim: &Claim, me: &Resolution) -> bool` (`pub(crate)`, Task 7 calls it)
  - `Ctx::resolve_for_guard(&mut self) -> Result<Resolution>`

- [ ] **Step 1: Write the failing tests**

Add to `src/claims.rs`'s `mod tests`:

```rust
    use crate::relay::ancestry::{ProcEntry, Scope};

    const BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";

    fn claim_of(session: &str, pid: Option<u32>, pid_start: Option<u64>) -> Claim {
        Claim {
            owner: "tester".into(),
            session: session.into(),
            pid,
            pid_start,
            boot_id: Some(BOOT.into()),
            host: "testhost".into(),
            worktree: "/w".into(),
            started: "2026-09-22T00:00:00Z".into(),
            seen: "2026-09-22T00:00:00Z".into(),
        }
    }

    fn nearest(comm: &str, pid: u32, start: u64) -> Scope {
        Scope::Harness(ProcEntry { pid, ppid: 1, comm: comm.into(), start })
    }

    fn no_env() -> impl Fn(&str) -> Option<String> {
        |_| None
    }

    #[test]
    fn the_owner_is_proved_from_the_claims_own_contents() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some(BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_nested_harness_cannot_prove_the_outer_sessions_claim() {
        // Claude(8) owns the claim; a codex(9) session launched beneath it has 8 among
        // its ancestors but its *nearest* boundary is 9. It must be refused.
        let claim = claim_of("claude-code:c1", Some(8), Some(800));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 9, 900),
            "testhost",
            Some(BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_contradicted_hint_defeats_the_proof() {
        let claim = claim_of("claude-code:c1", Some(42), Some(900));
        let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "other".to_string());
        assert!(!proves_ownership(
            &claim,
            &nearest("claude", 42, 900),
            "testhost",
            Some(BOOT),
            &env
        ));
    }

    #[test]
    fn a_known_representation_change_still_proves_ownership() {
        for stored in ["c1", "claude:c1", "claude-code:c1"] {
            let claim = claim_of(stored, Some(42), Some(900));
            let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "c1".to_string());
            assert!(
                proves_ownership(&claim, &nearest("claude", 42, 900), "testhost", Some(BOOT), &env),
                "{stored}"
            );
        }
    }

    #[test]
    fn proof_fails_on_a_reused_pid_a_new_boot_or_another_host() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        let scope = nearest("codex", 42, 901); // pid reused: different starttime
        assert!(!proves_ownership(&claim, &scope, "testhost", Some(BOOT), &no_env()));
        let scope = nearest("codex", 42, 900);
        assert!(!proves_ownership(
            &claim,
            &scope,
            "testhost",
            Some("22222222-2222-4222-8222-222222222222"),
            &no_env()
        ));
        assert!(!proves_ownership(&claim, &scope, "other", Some(BOOT), &no_env()));
    }

    #[test]
    fn a_claim_without_proof_cannot_be_proved() {
        // Native Claude without CLAUDE_PID, and every Codex claim, carry pid: None.
        let claim = claim_of("codex:s1", None, None);
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some(BOOT),
            &no_env()
        ));
    }

    #[test]
    fn out_of_scope_and_unknown_ancestry_prove_nothing() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(!proves_ownership(&claim, &Scope::Outside, "testhost", Some(BOOT), &no_env()));
        assert!(!proves_ownership(&claim, &Scope::Unknown(4), "testhost", Some(BOOT), &no_env()));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast proves_ownership`
Expected: FAIL — `proves_ownership` is undefined.

- [ ] **Step 3: Write the implementation**

In `src/claims.rs`:

```rust
/// A resolution that may be carried rather than raised. Spec §6.2.2: an identity error is
/// deferred across the two steps where a right to act can be established without it, and
/// raised the moment acquisition is reached.
pub enum Resolution {
    Resolved(Identity),
    Failed(Error),
}

impl Resolution {
    pub fn identity(&self) -> Option<&Identity> {
        match self {
            Resolution::Resolved(identity) => Some(identity),
            Resolution::Failed(_) => None,
        }
    }

    /// Acquisition, including every takeover: `--force` displaces an owner but never
    /// substitutes for one, so there is nothing to record without a resolved identity.
    pub fn require(self) -> Result<Identity> {
        match self {
            Resolution::Resolved(identity) => Ok(identity),
            Resolution::Failed(error) => Err(error),
        }
    }
}

pub fn resolve_identity(warnings: &mut Vec<String>) -> Resolution {
    match identity(warnings) {
        Ok(identity) => Resolution::Resolved(identity),
        Err(error) => Resolution::Failed(error),
    }
}

/// Spec §6.2: does this caller own `claim`, proved from the claim's own recorded process
/// handle and the caller's ancestry, with no registry read?
pub fn proves_ownership(
    claim: &Claim,
    scope: &crate::relay::ancestry::Scope,
    host: &str,
    boot_id: Option<&str>,
    get: &impl Fn(&str) -> Option<String>,
) -> bool {
    let crate::relay::ancestry::Scope::Harness(nearest) = scope else {
        return false;
    };
    let (Some(pid), Some(pid_start)) = (claim.pid, claim.pid_start) else {
        return false;
    };
    // 1. same host, same boot.
    if claim.host != host || claim.boot_id.as_deref() != boot_id || boot_id.is_none() {
        return false;
    }
    // 2. the *nearest* harness boundary, not any ancestor: a session nested under the
    //    owner has the owner among its ancestors and must still be refused.
    if nearest.pid != pid || nearest.start != pid_start {
        return false;
    }
    // 3. a scoped hint must not contradict the claim, so that deferring a resolution
    //    error cannot turn a contradicted session into an accepted owner.
    match crate::relay::resolve::hint_for(&nearest.comm, get) {
        Ok(Some(hint)) => crate::relay::resolve::same_session(&claim.session, &nearest.comm, &hint),
        Ok(None) => true,
        Err(_) => false,
    }
}
```

In `src/commands/mod.rs`, add the shared ownership decision to `Ctx`:

```rust
    /// Spec §6.2.2 steps 2 and 3. `None` means this caller is not the owner, which is
    /// the point at which acquisition rules apply.
    fn owns(&mut self, claim: &crate::claims::Claim, me: &crate::claims::Resolution) -> bool {
        if let Some(identity) = me.identity()
            && claim.session == identity.session
        {
            return true;
        }
        let scope = match crate::relay::ancestry::self_ppid() {
            Some(ppid) => crate::relay::ancestry::walk(ppid, crate::relay::ancestry::read_stat),
            None => crate::relay::ancestry::Scope::Unknown(0),
        };
        crate::claims::proves_ownership(
            claim,
            &scope,
            &crate::claims::hostname(),
            crate::claims::boot_id().as_deref(),
            &|key| {
                std::env::var_os(key)
                    .and_then(|value| value.into_string().ok())
                    .filter(|value| !value.is_empty())
            },
        )
    }
```

Add the shared resolution helper beside it. A relay-level failure is carryable; a native
failure stays fatal, which is what keeps relay-off behaviour byte-for-byte today's:

```rust
    /// Resolve identity, keeping a relay-level failure carryable so §6.2.2 can defer it.
    /// A failure with relay off is raised here, exactly where `identity` raised it before.
    pub(crate) fn resolve_for_guard(&mut self) -> Result<crate::claims::Resolution> {
        let me = crate::claims::resolve_identity(&mut self.warnings);
        match me {
            crate::claims::Resolution::Failed(error) if !crate::relay::enabled()? => Err(error),
            other => Ok(other),
        }
    }
```

Rewrite `refuse_foreign_live_claim`. It guards an edit rather than acquiring anything, so
it needs no identity when there is no claim and none when the claim is stale — and when it
does refuse, refusing records no owner and so needs none either:

```rust
    pub fn refuse_foreign_live_claim(&mut self, id: &TaskId) -> Result<()> {
        let Some(existing) = self.claims_mut()?.get(id).cloned() else {
            return Ok(());
        };
        let live = crate::claims::liveness(&existing);
        if live != Liveness::Live {
            return Ok(());
        }
        let me = self.resolve_for_guard()?;
        if self.owns(&existing, &me) {
            return Ok(());
        }
        Err(Error::Claimed(
            id.to_string(),
            Ctx::describe_claim(&existing, &live),
        ))
    }
```

Rewrite the head of `claim_guard`. The only structural changes are `resolve_for_guard` in
place of `identity`, `self.owns(…)` in place of `existing.session == me.session`, and
`me.require()?` before every path that records an owner:

```rust
    fn claim_guard(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let me = self.resolve_for_guard()?;
        let owner = owner_name(&self.project)?;
        let worktree = self.project.root.display().to_string();

        let mut warning = None;
        let existing = self.claims_mut()?.get(id).cloned();
        let mine = match &existing {
            Some(existing) => self.owns(existing, &me),
            None => false,
        };
        if let Some(existing) = &existing {
            let live = crate::claims::liveness(existing);
            match (&live, mine) {
                (Liveness::Live, false) if !(force && to == Status::Doing) => {
                    return Err(Error::Claimed(
                        id.to_string(),
                        Ctx::describe_claim(existing, &live),
                    ));
                }
                // Both remaining branches take the claim over, and a takeover records a
                // new owner: `--force` displaces an owner but never substitutes for one.
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

        self.pending_claim = Some(if to == Status::Doing {
            // Acquisition, whether first claim or takeover: identity is required here.
            let me = me.require()?;
            let now = crate::time::now();
            let started = existing
                .filter(|_| mine)
                .map(|existing| existing.started.clone())
                .unwrap_or_else(|| now.clone());
            (
                id.clone(),
                ClaimIntent::Acquire(crate::claims::Claim {
                    owner,
                    pid_start: match &me.proof {
                        Some(proof) => Some(proof.pid_start),
                        None => me.pid.and_then(|pid| match crate::claims::proc_stat(pid) {
                            crate::claims::ProcStat::Found { starttime, .. } => Some(starttime),
                            _ => None,
                        }),
                    },
                    boot_id: match &me.proof {
                        Some(proof) => Some(proof.boot_id.clone()),
                        None => crate::claims::boot_id(),
                    },
                    host: match &me.proof {
                        Some(proof) => proof.host.clone(),
                        None => crate::claims::hostname(),
                    },
                    session: me.session,
                    pid: me.pid,
                    worktree,
                    started,
                    seen: now,
                }),
            )
        } else {
            // A release by its owner: proof is enough, so a lost registry cannot strand it.
            (
                id.clone(),
                ClaimIntent::Release {
                    clear_park: matches!(to, Status::Done | Status::Dropped | Status::Shelved),
                    announce_escalation: to == Status::Shelved,
                },
            )
        });

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }
```

This supersedes the Task 5 edit to the `Acquire` literal: the field adoption is the same,
and it now lives in the rewritten function.

In `src/commands/park.rs:38`, replace the identity line with the same shape. A park is a
release by its owner, so it proceeds on proof:

```rust
    let me = ctx.resolve_for_guard()?;
```

and wherever the function compares `existing.session` to `me.session`, use
`ctx.owns(&existing, &me)` instead. Park records no new owner, so it never calls
`require()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast claims && just test-fast commands`
Expected: PASS.

- [ ] **Step 5: Run the whole suite**

Run: `just gate`
Expected: PASS — no pre-existing claim or park test changes behaviour.

- [ ] **Step 6: Commit**

```bash
git add src/claims.rs src/commands/mod.rs src/commands/park.rs
git commit -m "feat(claims): prove claim ownership without the registry; takeover still needs identity"
```

---

### Task 7: Continuity in `note`

**Files:**
- Modify: `src/commands/status.rs:82-124`
- Test: `tests/cli.rs`

`note` is deliberately not folded into Task 6's common path. It guards nothing: it never
refuses a foreign claim and never touches one, and its identity resolution exists only to
decide whether to refresh its own claim's heartbeat.

**Interfaces:**
- Consumes: `Ctx::resolve_for_guard`, `Ctx::owns`, `relay::enabled`.
- Produces: no new public interface.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.rs`:

```rust
#[test]
fn note_never_fails_on_an_unresolvable_relay_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));

    // Relay enabled, with a state directory that holds no registry at all.
    let config = env.home.path().join(".config/tasks/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[identity]\nrelay = true\n").unwrap();
    let state = env.home.path().join("empty-relay-state");
    std::fs::create_dir_all(&state).unwrap();

    let out = env
        .cmd(&dir)
        .env("RELAY_STATE_DIR", &state)
        .args(["note", &id, "still lands"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let shown = env.json(&dir, &["show", &id]);
    assert!(
        shown["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note["text"].as_str().unwrap().contains("still lands")),
        "the note must land even though relay identity could not resolve"
    );
}

#[test]
fn a_foreign_note_is_accepted_and_leaves_the_foreign_claim_alone() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    env.cmd(&dir)
        .env("TASKS_SESSION", "owner")
        .env("TASKS_SESSION_PID", &std::process::id().to_string())
        .args(["start", &id])
        .assert()
        .success();
    let before = std::fs::read_to_string(env.claim_store("sci")).unwrap();

    env.cmd(&dir)
        .env("TASKS_SESSION", "stranger")
        .args(["note", &id, "from elsewhere"])
        .assert()
        .success();
    let after = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(before, after, "a foreign note must not refresh or alter the claim");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast note_never_fails && just test-fast a_foreign_note`
Expected: the first FAILs with a `config` error at exit 1; the second passes already and
must keep passing.

- [ ] **Step 3: Write the implementation**

In `status::note`, replace the identity resolution and the `mine` filter:

```rust
    // Identity and the store are resolved *before* the file write. Doing it afterwards
    // means a corrupt store returns an error after the note has already landed, and the
    // obvious retry then duplicates it.
    //
    // A relay-level resolution failure is not fatal here: `note` guards nothing, so a
    // lost registry must not cost the caller a note. A claim that can be neither matched
    // nor proved is simply not ours, which is the same outcome as a foreign claim.
    let me = ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    save(&mut ctx, &mut task)?;

    // Use the pruned store so a note cannot revive a stale claim.
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let mine = existing.filter(|claim| ctx.owns(claim, &me));
```

`Ctx::owns` and `Ctx::resolve_for_guard` are both `pub(crate)` for this call site. The
heartbeat block below is unchanged, as is its warning on a failed save.

`resolve_for_guard` is what keeps the two cases apart, and is the reason `note` needs no
special-casing of its own: with relay off it raises exactly where `identity` raised
before, so an unresolvable native identity is still fatal and the note still does not
land; with relay on it carries the failure, `owns` answers `false`, and the note lands
with no heartbeat — the same outcome as a foreign claim, which `note` has always allowed.

Add to `src/relay/mod.rs`:

```rust
/// Whether the relay level is configured on at all. Read by callers that treat a
/// relay-level failure differently from a native one.
pub fn enabled() -> Result<bool> {
    Ok(HostConfig::load()?.relay_identity)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast note`
Expected: PASS, both tests.

- [ ] **Step 5: Commit**

```bash
git add src/commands/status.rs src/relay/mod.rs tests/cli.rs
git commit -m "feat(note): keep the heartbeat on proof and never lose a note to relay resolution"
```

**Note on `relay::enabled`:** `resolve_for_guard` (Task 6) calls it, so it must exist by
the end of Task 6. Add it there if Task 6 is implemented first; this task's step is a
no-op in that case.

---

### Task 8: End-to-end acceptance tests and documentation

**Files:**
- Modify: `tests/common/mod.rs:19-40` and `:43-66` (clear `RELAY_STATE_DIR` in `cmd` and `raw`)
- Modify: `tests/cli.rs`
- Modify: `README.md`, `skills/tasks/SKILL.md`
- Test: `tests/cli.rs`

The reviewer asked for the nested-harness, contradictory-hint, registry-loss and relay-off
cases as explicit acceptance tests. They need a real harness ancestor, which a test cannot
get from the test runner — so each spawns a copy of `/bin/sh` named for the harness, which
writes a registry naming its own pid and start time and then runs `tasks` as a child.
Nothing is `exec`ed: `exec` would replace the shim and destroy the ancestry under test.

**Interfaces:**
- Consumes: everything above.
- Produces: `tests/cli.rs::harness_shim(env, comm, script) -> std::process::Output`.

- [ ] **Step 1: Add the harness shim helper and the acceptance tests**

Add to `tests/cli.rs`:

```rust
/// Run `script` under a process whose `comm` is `comm`, so the `tasks` it launches has a
/// recognized harness ancestor. A copy of `/bin/sh` supplies the comm; the script writes
/// the registry from its own `/proc/self/stat` because the pid is not known until then.
fn harness_shim(dir: &std::path::Path, home: &std::path::Path, comm: &str, script: &str) -> std::process::Output {
    let shim = home.join(comm);
    std::fs::copy("/bin/sh", &shim).unwrap();
    let mut perms = std::fs::metadata(&shim).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&shim, perms).unwrap();
    std::process::Command::new(&shim)
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_SESSION")
        .env_remove("TASKS_SESSION_PID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .env("TASKS_BIN", assert_cmd::cargo::cargo_bin("tasks"))
        .output()
        .unwrap()
}

/// Shell that writes a one-agent registry naming this shim's own process, then runs tasks.
/// `comm` here has no spaces or parentheses, so field 22 is safe to take positionally.
const WRITE_REGISTRY_THEN: &str = r#"
set -e
start=$(awk '{print $22}' /proc/self/stat)
boot=$(cat /proc/sys/kernel/random/boot_id)
host=$(cat /proc/sys/kernel/hostname)
mkdir -p "$RELAY_STATE_DIR"
cat > "$RELAY_STATE_DIR/agents.json" <<EOF
{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,
 "agents":{"$AGENT_ID":{"id":"$AGENT_ID","harness":"$HARNESS","sessionId":"$SESSION",
  "scope":"session","cwd":"/w","repoRoot":null,"remote":null,"projectKey":"k",
  "project":"p","state":"idle","updatedAt":1,
  "process":{"platform":"linux","host":"$host","bootId":"$boot","pid":$$,"start":"$start"}}}}
EOF
"#;

fn relay_home(env: &TestEnv) -> (std::path::PathBuf, std::path::PathBuf) {
    let config = env.home.path().join(".config/tasks/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[identity]\nrelay = true\n").unwrap();
    let state = env.home.path().join(".local/state/relay");
    (config, state)
}

#[test]
fn relay_mode_keys_a_claim_by_the_agent_id_with_the_handles_proof() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);

    let script = format!(
        "{WRITE_REGISTRY_THEN}\nexport RELAY_STATE_DIR HARNESS SESSION AGENT_ID\n\"$TASKS_BIN\" start {id}\n"
    );
    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={}\nHARNESS=codex\nSESSION=s1\nAGENT_ID=codex:s1\n{script}",
            state.display()
        ),
    );
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));

    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("codex:s1"), "{store}");
    assert!(store.contains("pid_start"), "{store}");
    assert!(store.contains("boot_id"), "{store}");
}

#[test]
fn the_owner_can_park_after_the_registry_is_removed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);

    let script = format!(
        "{WRITE_REGISTRY_THEN}\nexport RELAY_STATE_DIR HARNESS SESSION AGENT_ID\n\
         \"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" park {id} 'next step'\n"
    );
    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={}\nHARNESS=codex\nSESSION=s1\nAGENT_ID=codex:s1\n{script}",
            state.display()
        ),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the owner must be able to park with the registry gone: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_nested_harness_cannot_close_the_outer_sessions_task() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);

    // A claude shim claims the task, then runs a codex shim beneath itself which tries to
    // close it. The inner session's nearest boundary is the codex process, so its
    // ownership proof against the claude claim must fail and the close must be refused.
    let inner = env.home.path().join("codex");
    std::fs::copy("/bin/sh", &inner).unwrap();
    let mut perms = std::fs::metadata(&inner).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&inner, perms).unwrap();

    let script = format!(
        "{WRITE_REGISTRY_THEN}\nexport RELAY_STATE_DIR HARNESS SESSION AGENT_ID TASKS_BIN\n\
         \"$TASKS_BIN\" start {id}\n\
         \"{}\" -c '\"$TASKS_BIN\" done {id} landed' && echo INNER_CLOSED\n",
        inner.display()
    );
    let out = harness_shim(
        &dir,
        env.home.path(),
        "claude",
        &format!(
            "RELAY_STATE_DIR={}\nHARNESS=claude-code\nSESSION=c1\nAGENT_ID=claude-code:c1\n{script}",
            state.display()
        ),
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        !text.contains("INNER_CLOSED"),
        "a session nested under the owner must not close its task: {text}"
    );
    assert_eq!(env.json(&dir, &["show", &id])["status"], "doing");
}

#[test]
fn a_contradicted_hint_is_refused_even_though_the_proof_matches() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);

    let script = format!(
        "{WRITE_REGISTRY_THEN}\nexport RELAY_STATE_DIR HARNESS SESSION AGENT_ID\n\
         CODEX_SESSION_ID=someone-else \"$TASKS_BIN\" start {id}\n"
    );
    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={}\nHARNESS=codex\nSESSION=s1\nAGENT_ID=codex:s1\n{script}",
            state.display()
        ),
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("someone-else") && text.contains("codex:s1"), "{text}");
}

#[test]
fn an_empty_registry_under_a_live_harness_refuses_rather_than_using_terminal_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("agents.json"),
        r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#,
    )
    .unwrap();

    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={} \"$TASKS_BIN\" start {id}\n",
            state.display()
        ),
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("TASKS_SESSION"), "{text}");
    let store = env.claim_store("sci");
    assert!(!store.exists() || !std::fs::read_to_string(&store).unwrap().contains("sid:"));
}

#[test]
fn the_explicit_pair_still_works_under_a_live_harness_with_an_empty_registry() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let (_, state) = relay_home(&env);
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("agents.json"),
        r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#,
    )
    .unwrap();

    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={} TASKS_SESSION=explicit \"$TASKS_BIN\" start {id}\n",
            state.display()
        ),
    );
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::read_to_string(env.claim_store("sci")).unwrap().contains("explicit"));
}

#[test]
fn relay_off_leaves_a_harness_session_on_the_native_ladder() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // No config file at all: relay is off, and the registry below is never consulted.
    let state = env.home.path().join(".local/state/relay");

    let script = format!(
        "{WRITE_REGISTRY_THEN}\nexport RELAY_STATE_DIR HARNESS SESSION AGENT_ID\n\
         CODEX_SESSION_ID=s1 \"$TASKS_BIN\" start {id}\n"
    );
    let out = harness_shim(
        &dir,
        env.home.path(),
        "codex",
        &format!(
            "RELAY_STATE_DIR={}\nHARNESS=codex\nSESSION=s1\nAGENT_ID=codex:s1\n{script}",
            state.display()
        ),
    );
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(
        store.contains("session = \"s1\"") && !store.contains("codex:s1"),
        "relay off must key the claim by the native raw id: {store}"
    );
}
```

- [ ] **Step 2: Clear `RELAY_STATE_DIR` in the harness**

In `tests/common/mod.rs`, add `.env_remove("RELAY_STATE_DIR")` to both `cmd` and `raw`,
beside the existing `.env_remove("XDG_STATE_HOME")`, so no test inherits a real registry.

- [ ] **Step 3: Run the acceptance tests**

Run: `just test-fast relay_mode && just test-fast nested_harness && just test-fast contradicted_hint && just test-fast registry`
Expected: PASS.

- [ ] **Step 4: Update the documentation**

In `README.md`, add a section under the claims documentation describing
`~/.config/tasks/config.toml`, the `[identity] relay` key, that it is host-local and
opt-in, that identity is adopted between sessions rather than during one, and that
`TASKS_SESSION` remains the recovery path from any relay identity error.

In `skills/tasks/SKILL.md`, extend the claims paragraph of the session protocol: under
relay mode a claim is keyed by the relay agent id (`<harness>:<sessionId>`), the owner can
still park and close with the registry unavailable, and a relay identity error is resolved
by setting `TASKS_SESSION`/`TASKS_SESSION_PID`.

- [ ] **Step 5: Run the whole suite**

Run: `just gate`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/ README.md skills/tasks/SKILL.md
git commit -m "test(relay): acceptance tests for nesting, hints, registry loss and relay off"
```
