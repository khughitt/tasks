# Relay Ancestry Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in identity level that names a tasks session by the relay agent it runs under, adopting that agent's process handle as claim proof, without changing liveness or the claim file format.

**Architecture:** A new `src/relay/` module resolves identity in four stages — explicit override, configuration, platform support, ancestry — and only then reads relay's `agents.json` to verify the nearest harness ancestor. `claims::identity_from` gains that level as a lazily-invoked closure, so the stages cannot run out of order. Claim ownership is then decided in two ways rather than one: by identity when it resolves, and otherwise by re-deriving the claim's own recorded process proof locally, which needs no registry. Which of the two established ownership decides what gets recorded, and only the proof-only path substitutes the claim's own identity.

**Tech Stack:** Rust 2024, `serde_json` for the schema-1 snapshot, `toml` for the host config, `/proc` for ancestry, process start times, and the effective uid. No new dependencies.

**Spec:** `docs/specs/2026-09-22-relay-ancestry-identity-design.md`

## Global Constraints

- Relay identity is **opt-in**. With it off, behaviour is byte-for-byte today's, including every path where an unresolvable identity is fatal today. Every task touching an existing path carries a test asserting that.
- **Explicit identity is authoritative.** With `TASKS_SESSION` set, a session mismatch is foreign — full stop. Ownership proof is a relay-mode fallback and never overrides the explicit pair, because two workers sharing one process are distinguished by nothing else.
- **Ownership proof never rewrites identity.** When ownership rests on *proof alone*, the operation records the claim's own session (`claims::continuation_identity`). When the resolved identity already matched the claim, that resolved identity is used with its current fields, exactly as today. Either way `existing.session` is never rewritten. Spec §6.4.
- **Parking releases the claim**, and its proof with it (`Store::insert_park` removes it). Parking and closing a held claim survive registry loss; *resuming* a parked task is a fresh acquisition and needs the registry back or the explicit override.
- **Acquisition requires resolved identity**, and every path that is not a continuation by the owner is acquisition: first claim, takeover, stale-claim takeover, and `start --force` alike. `--force` displaces an owner; it never substitutes for one.
- The four stages run in this order, each reached only by passing the one before: explicit override (`TASKS_SESSION`) → configuration → platform support → ancestry. Spec §7. Unknown ancestry is refused **before** the registry is opened, so a registry error can never mask an ancestry error.
- Liveness is not modified. `claims::liveness` and `liveness_with` keep their current signatures and bodies.
- The claim file format does not change. Adopted proof goes into the existing `pid`, `pid_start`, `boot_id`, `host` fields.
- Registry reads are Rust-only and reject a non-private path exactly as relay's own `checkPrivate` does. Tasks never spawns Node and never writes to `agents.json`.
- Recognized harness `comm` names and their agent harnesses: `claude → claude-code`, `codex → codex`, `opencode → opencode`. Exactly these three.
- Only `platform: "linux"` handles are adopted. A Darwin handle must parse without error and then be refused as an identity candidate.
- **Every commit builds green under `-D warnings`.** A module whose consumer arrives in a later task carries a file-level `#![allow(dead_code)]` naming the task that removes it — the same device `tests/common/mod.rs` already uses. Task 5 removes `src/config.rs`'s; Task 6 removes the last one, from `src/relay/mod.rs`, where `resolve::same_session` finally gains a production consumer.
- JSON output shapes do not change in this plan.
- Run `just test-fast <name>` while working and `just gate` before the final commit of each task. Test filters below name **test functions**, not the items under test. Never run `cargo test` directly.

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

Create `src/config.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_without_the_key_leaves_relay_off() {
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
    fn a_config_relay_key_is_read() {
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
    fn a_config_typo_is_loud_rather_than_silently_off() {
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

Run: `just test-fast a_config_`
Expected: FAIL — `HostConfig` is not defined.

- [ ] **Step 3: Write the implementation**

Above the test module in `src/config.rs`:

```rust
// The relay level is this file's only consumer and arrives in Task 5, which removes this.
#![allow(dead_code)]

use crate::error::{Error, Result};
use std::path::PathBuf;

/// The host-local configuration, `$XDG_CONFIG_HOME/tasks/config.toml` or
/// `$HOME/.config/tasks/config.toml`. Host-local deliberately: relay availability is a
/// property of a machine, and the per-project `tasks/.config.toml` is committed and syncs
/// between hosts.
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

Note the inner attribute must be the file's first line, above the `use` statements.

Add `mod config;` to `src/main.rs` beside the other `mod` declarations.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast a_config_`
Expected: PASS, 3 tests.

- [ ] **Step 5: Verify the commit builds green**

Run: `just check`
Expected: PASS — `cargo clippy --all-targets -- -D warnings` is clean because of the file-level allow.

- [ ] **Step 6: Commit**

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

This reader is a **second implementation of a published schema**, so it must refuse
everything the producer refuses. A row tasks accepts but relay would reject is a row whose
meaning tasks has guessed at. The validation below mirrors `validateSnapshot`,
`validateAgent`, `validateHandle` and `checkPrivate` in relay's `src/protocol.js` and
`src/bus/store.js`.

**Interfaces:**
- Consumes: `crate::error::{Error, Result}`.
- Produces:
  - `relay::snapshot::Handle { platform: String, host: String, boot_id: Option<String>, pid: u32, start: u64 }`
  - `relay::snapshot::Agent { id: String, harness: String, session_id: String, scope: String, process: Option<Handle> }`
  - `relay::snapshot::Snapshot { generation: String, revision: u64, agents: Vec<Agent> }`
  - `relay::snapshot::path() -> Result<PathBuf>`
  - `relay::snapshot::parse(text: &str) -> Result<Snapshot>`
  - `relay::snapshot::load() -> Result<Snapshot>`

- [ ] **Step 1: Write the failing tests**

Create `src/relay/snapshot.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";

    /// A complete, schema-valid agent. Tests mutate one field at a time from here, so a
    /// refusal can only be caused by the field under test.
    fn agent_json(overrides: &[(&str, &str)]) -> String {
        let mut fields: Vec<(&str, String)> = vec![
            ("id", "\"codex:s1\"".into()),
            ("harness", "\"codex\"".into()),
            ("sessionId", "\"s1\"".into()),
            ("scope", "\"session\"".into()),
            ("cwd", "\"/w\"".into()),
            ("repoRoot", "null".into()),
            ("remote", "null".into()),
            ("projectKey", "\"k\"".into()),
            ("project", "\"p\"".into()),
            ("state", "\"idle\"".into()),
            ("updatedAt", "1".into()),
            (
                "process",
                format!(
                    r#"{{"platform":"linux","host":"testhost","bootId":"{BOOT}","pid":42,"start":"900"}}"#
                ),
            ),
        ];
        for (key, value) in overrides {
            match fields.iter_mut().find(|(name, _)| name == key) {
                Some(slot) => slot.1 = (*value).to_string(),
                None => fields.push((key, (*value).to_string())),
            }
        }
        let body = fields
            .iter()
            .filter(|(_, value)| value != "\u{0}") // the sentinel for "omit this field"
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{body}}}")
    }

    fn snapshot_json(key: &str, agent: &str) -> String {
        format!(
            r#"{{"schema":1,"generation":"11111111-2222-4333-8444-555555555555",
               "revision":7,"agents":{{"{key}":{agent}}}}}"#
        )
    }

    fn valid() -> String {
        snapshot_json("codex:s1", &agent_json(&[]))
    }

    #[test]
    fn a_snapshot_parses_every_field() {
        let snapshot = parse(&valid()).unwrap();
        assert_eq!(snapshot.generation, "11111111-2222-4333-8444-555555555555");
        assert_eq!(snapshot.revision, 7);
        let agent = &snapshot.agents[0];
        assert_eq!(agent.id, "codex:s1");
        assert_eq!(agent.harness, "codex");
        assert_eq!(agent.session_id, "s1");
        assert_eq!(agent.scope, "session");
        let process = agent.process.as_ref().unwrap();
        assert_eq!(process.platform, "linux");
        assert_eq!(process.host, "testhost");
        assert_eq!(process.pid, 42);
        assert_eq!(process.start, 900);
        assert_eq!(process.boot_id.as_deref(), Some(BOOT));
    }

    #[test]
    fn a_snapshot_accepts_a_darwin_handle_for_parsing_only() {
        // Refusing it as an identity candidate is Task 4's job; it must not be refused
        // here by being unreadable.
        let agent = agent_json(&[(
            "process",
            r#"{"platform":"darwin","host":"mac","bootId":null,"pid":9,"start":"1750000000"}"#,
        )]);
        let snapshot = parse(&snapshot_json("codex:s1", &agent)).unwrap();
        assert_eq!(snapshot.agents[0].process.as_ref().unwrap().platform, "darwin");
    }

    #[test]
    fn a_snapshot_accepts_the_largest_start_and_refuses_an_overflow() {
        let big = agent_json(&[(
            "process",
            &format!(
                r#"{{"platform":"linux","host":"testhost","bootId":"{BOOT}","pid":42,"start":"18446744073709551615"}}"#
            ),
        )]);
        assert_eq!(
            parse(&snapshot_json("codex:s1", &big)).unwrap().agents[0]
                .process
                .as_ref()
                .unwrap()
                .start,
            u64::MAX
        );
        let over = agent_json(&[(
            "process",
            &format!(
                r#"{{"platform":"linux","host":"testhost","bootId":"{BOOT}","pid":42,"start":"18446744073709551616"}}"#
            ),
        )]);
        assert!(parse(&snapshot_json("codex:s1", &over)).is_err());
    }

    #[test]
    fn a_snapshot_accepts_a_null_handle_and_an_opencode_process_scope() {
        let null_handle = agent_json(&[("process", "null")]);
        assert!(parse(&snapshot_json("codex:s1", &null_handle)).unwrap().agents[0]
            .process
            .is_none());
        let opencode = agent_json(&[
            ("id", "\"opencode:o1\""),
            ("harness", "\"opencode\""),
            ("sessionId", "\"o1\""),
            ("scope", "\"process\""),
        ]);
        assert_eq!(
            parse(&snapshot_json("opencode:o1", &opencode)).unwrap().agents[0].scope,
            "process"
        );
    }

    #[test]
    fn a_snapshot_refuses_what_the_producer_refuses() {
        let cases: Vec<(&str, String)> = vec![
            ("schema 2", r#"{"schema":2,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#.into()),
            ("no generation", r#"{"schema":1,"revision":1,"agents":{}}"#.into()),
            ("bad generation", r#"{"schema":1,"generation":"nope","revision":1,"agents":{}}"#.into()),
            ("no revision", r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","agents":{}}"#.into()),
            ("no agents", r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1}"#.into()),
            ("not json", "not json".into()),
            ("key != id", snapshot_json("other", &agent_json(&[]))),
            ("id != harness:session", snapshot_json("codex:s1", &agent_json(&[("sessionId", "\"other\"")]))),
            ("unknown harness", snapshot_json("nope:s1", &agent_json(&[("id", "\"nope:s1\""), ("harness", "\"nope\"")]))),
            ("wrong scope for harness", snapshot_json("codex:s1", &agent_json(&[("scope", "\"process\"")]))),
            ("relative cwd", snapshot_json("codex:s1", &agent_json(&[("cwd", "\"w\"")]))),
            ("missing cwd", snapshot_json("codex:s1", &agent_json(&[("cwd", "\u{0}")]))),
            ("missing remote", snapshot_json("codex:s1", &agent_json(&[("remote", "\u{0}")]))),
            ("empty remote", snapshot_json("codex:s1", &agent_json(&[("remote", "\"\"")]))),
            ("relative repoRoot", snapshot_json("codex:s1", &agent_json(&[("repoRoot", "\"rel\"")]))),
            ("empty projectKey", snapshot_json("codex:s1", &agent_json(&[("projectKey", "\"\"")]))),
            ("unknown state", snapshot_json("codex:s1", &agent_json(&[("state", "\"sleeping\"")]))),
            ("negative updatedAt", snapshot_json("codex:s1", &agent_json(&[("updatedAt", "-1")]))),
            ("unsafe updatedAt", snapshot_json("codex:s1", &agent_json(&[("updatedAt", "9007199254740992")]))),
            ("unsafe revision", r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":9007199254740992,"agents":{}}"#.into()),
            ("missing process", snapshot_json("codex:s1", &agent_json(&[("process", "\u{0}")]))),
            ("pid 0", snapshot_json("codex:s1", &agent_json(&[("process", &format!(r#"{{"platform":"linux","host":"testhost","bootId":"{BOOT}","pid":0,"start":"900"}}"#))]))),
            ("non-canonical start", snapshot_json("codex:s1", &agent_json(&[("process", &format!(r#"{{"platform":"linux","host":"testhost","bootId":"{BOOT}","pid":42,"start":"007"}}"#))]))),
            ("bad boot uuid", snapshot_json("codex:s1", &agent_json(&[("process", r#"{"platform":"linux","host":"testhost","bootId":"not-a-uuid","pid":42,"start":"900"}"#)]))),
            ("darwin with a bootId", snapshot_json("codex:s1", &agent_json(&[("process", &format!(r#"{{"platform":"darwin","host":"mac","bootId":"{BOOT}","pid":9,"start":"1"}}"#))]))),
            ("missing bootId", snapshot_json("codex:s1", &agent_json(&[("process", r#"{"platform":"linux","host":"testhost","pid":42,"start":"900"}"#)]))),
        ];
        for (name, text) in cases {
            assert!(parse(&text).is_err(), "{name} must be refused: {text}");
        }
    }

    #[test]
    fn a_snapshot_with_no_agents_is_valid_and_empty() {
        let text = r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#;
        assert!(parse(text).unwrap().agents.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast a_snapshot_`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Write the implementation**

`src/relay/mod.rs`:

```rust
// The identity ladder is this module's consumer and arrives in Task 5, which removes this.
#![allow(dead_code)]

pub mod snapshot;
```

Above the test module in `src/relay/snapshot.rs`:

```rust
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// relay's published process handle. `start` is opaque here: on Linux it is the same
/// `starttime` tick count `/proc/<pid>/stat` reports, and on Darwin an epoch, which is why
/// the platform travels with it and why only Linux handles are ever adopted.
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
    pub scope: String,
    pub process: Option<Handle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: String,
    pub revision: u64,
    pub agents: Vec<Agent>,
}

const HARNESSES: [&str; 3] = ["claude-code", "codex", "opencode"];
/// relay's `integer()` is `Number.isSafeInteger(v) && v >= 0`, so its numeric fields stop
/// at 2^53 - 1. `start` is exempt: it travels as decimal *text* through `parseStart` and
/// uses the full u64 range.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const STATES: [&str; 6] = [
    "idle",
    "working",
    "needs-input",
    "needs-approval",
    "done",
    "error",
];

fn invalid(detail: &str) -> Error {
    Error::Config(format!("relay registry: {detail}"))
}

/// relay's own uuid predicate: version 1-8, variant 8/9/a/b, case-insensitive.
fn is_uuid(value: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != groups.len() {
        return false;
    }
    for (part, width) in parts.iter().zip(groups) {
        if part.len() != width || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
    }
    let version = parts[2].as_bytes()[0];
    let variant = parts[3].as_bytes()[0].to_ascii_lowercase();
    (b'1'..=b'8').contains(&version) && matches!(variant, b'8' | b'9' | b'a' | b'b')
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

/// Our own uid, without a libc dependency: `/proc/self` is owned by the process's uid.
/// Linux-only, which relay identity already is.
fn own_uid() -> Result<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self")
        .map(|meta| meta.uid())
        .map_err(|error| invalid(&format!("cannot read /proc/self: {error}")))
}

/// relay's `checkPrivate`: no symlink, the expected type, owned by us, and exactly 0700
/// for a directory or 0600 for a file. `Ok(false)` means absent.
fn check_private(path: &Path, directory: bool) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(invalid(&format!(
                "cannot inspect {}: {error}",
                path.display()
            )));
        }
    };
    let expected_mode = if directory { 0o700 } else { 0o600 };
    let type_ok = if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    };
    if meta.file_type().is_symlink()
        || !type_ok
        || meta.uid() != own_uid()?
        || meta.mode() & 0o7777 != expected_mode
    {
        return Err(invalid(&format!(
            "{} must be privately owned with a safe type and mode",
            path.display()
        )));
    }
    Ok(true)
}

pub fn load() -> Result<Snapshot> {
    let path = path()?;
    let dir = path
        .parent()
        .ok_or_else(|| invalid("the registry path has no directory"))?;
    if !check_private(dir, true)? {
        return Err(invalid(&format!("{} does not exist", dir.display())));
    }
    if !check_private(&path, false)? {
        return Err(invalid(&format!("{} does not exist", path.display())));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| invalid(&format!("{}: {error}", path.display())))?;
    parse(&text)
}

fn text_field(value: &serde_json::Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| invalid(&format!("{field} must be non-empty text")))
}

/// `null` or non-empty text, with the field required to be present — relay distinguishes
/// an explicit null from an absent key, and so must this.
fn nullable_text(value: &serde_json::Value, field: &str, absolute: bool) -> Result<Option<String>> {
    match value.get(field) {
        None => Err(invalid(&format!("{field} is required"))),
        Some(serde_json::Value::Null) => Ok(None),
        Some(_) => {
            let text = text_field(value, field)?;
            if absolute && !Path::new(&text).is_absolute() {
                return Err(invalid(&format!("{field} must be an absolute path")));
            }
            Ok(Some(text))
        }
    }
}

fn parse_handle(value: &serde_json::Value) -> Result<Handle> {
    let platform = text_field(value, "platform")?;
    if platform != "linux" && platform != "darwin" {
        return Err(invalid("platform must be linux or darwin"));
    }
    let host = text_field(value, "host")?;
    let pid = value
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .filter(|pid| *pid >= 1 && *pid <= u32::MAX as u64)
        .ok_or_else(|| invalid("handle pid out of range"))? as u32;
    let start = parse_start(&text_field(value, "start")?)?;
    let boot_id = match value.get("bootId") {
        None => return Err(invalid("bootId is required")),
        Some(serde_json::Value::Null) => None,
        Some(id) => {
            let id = id
                .as_str()
                .ok_or_else(|| invalid("bootId must be text or null"))?;
            if platform != "linux" {
                return Err(invalid("only a linux handle carries a bootId"));
            }
            if !is_uuid(id) {
                return Err(invalid(&format!("bootId {id:?} is not a uuid")));
            }
            Some(id.to_string())
        }
    };
    Ok(Handle {
        platform,
        host,
        boot_id,
        pid,
        start,
    })
}

fn parse_agent(key: &str, value: &serde_json::Value) -> Result<Agent> {
    if !value.is_object() {
        return Err(invalid(&format!("agent {key:?} is not an object")));
    }
    let id = text_field(value, "id")?;
    let harness = text_field(value, "harness")?;
    let session_id = text_field(value, "sessionId")?;
    if !HARNESSES.contains(&harness.as_str()) {
        return Err(invalid(&format!("unknown harness {harness:?}")));
    }
    if id != format!("{harness}:{session_id}") || id != key {
        return Err(invalid(&format!("agent {key:?} has an inconsistent id")));
    }
    let scope = text_field(value, "scope")?;
    let expected_scope = if harness == "opencode" {
        "process"
    } else {
        "session"
    };
    if scope != expected_scope {
        return Err(invalid(&format!(
            "agent {key:?} has scope {scope:?}, expected {expected_scope:?}"
        )));
    }
    let cwd = text_field(value, "cwd")?;
    if !Path::new(&cwd).is_absolute() {
        return Err(invalid("cwd must be an absolute path"));
    }
    nullable_text(value, "repoRoot", true)?;
    nullable_text(value, "remote", false)?;
    text_field(value, "projectKey")?;
    text_field(value, "project")?;
    let state = text_field(value, "state")?;
    if !STATES.contains(&state.as_str()) {
        return Err(invalid(&format!("unknown state {state:?}")));
    }
    if value
        .get("updatedAt")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .is_none()
    {
        return Err(invalid("updatedAt must be a non-negative safe integer"));
    }
    let process = match value.get("process") {
        None => return Err(invalid("process is required")),
        Some(serde_json::Value::Null) => None,
        Some(handle) => Some(parse_handle(handle)?),
    };
    Ok(Agent {
        id,
        harness,
        session_id,
        scope,
        process,
    })
}

pub fn parse(text: &str) -> Result<Snapshot> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|error| invalid(&error.to_string()))?;
    if raw.get("schema").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(invalid("schema must be 1"));
    }
    let generation = text_field(&raw, "generation")?;
    if !is_uuid(&generation) {
        return Err(invalid("generation must be a uuid"));
    }
    let revision = raw
        .get("revision")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .ok_or_else(|| invalid("revision must be a non-negative safe integer"))?;
    let map = raw
        .get("agents")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("agents must be an object"))?;

    let mut agents = Vec::with_capacity(map.len());
    for (key, value) in map {
        agents.push(parse_agent(key, value)?);
    }
    agents.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Snapshot {
        generation,
        revision,
        agents,
    })
}
```

Add `mod relay;` to `src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast a_snapshot_`
Expected: PASS, 6 tests.

- [ ] **Step 5: Verify the commit builds green**

Run: `just check`
Expected: PASS.

- [ ] **Step 6: Commit**

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

The walk is shared: the scope test of spec §4.2 and the ownership proof of §6.2 both need
the same nearest boundary.

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
  - `relay::ancestry::proc_available() -> bool`
  - `relay::ancestry::current_scope() -> Scope`

- [ ] **Step 1: Write the failing tests**

Create `src/relay/ancestry.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// `pid (comm) state ppid …` with starttime at remainder index 19.
    fn stat(pid: u32, comm: &str, ppid: u32, start: u64) -> String {
        let filler = (0..17).map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
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
    fn an_ancestry_entry_parses_a_comm_containing_spaces_and_parentheses() {
        let entry =
            parse_entry("7 (weird ) name) S 3 a b c d e f g h i j k l m n o p q 4242 x").unwrap();
        assert_eq!(entry.pid, 7);
        assert_eq!(entry.comm, "weird ) name");
        assert_eq!(entry.ppid, 3);
        assert_eq!(entry.start, 4242);
    }

    #[test]
    fn an_ancestry_walk_stops_at_the_nearest_harness() {
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
    fn an_ancestry_walk_of_a_plain_shell_is_outside() {
        let read = tree(&[(5, "zsh", 2, 500), (2, "systemd", 1, 200), (1, "init", 0, 1)]);
        assert!(matches!(walk(5, read), Scope::Outside));
    }

    #[test]
    fn an_ancestry_walk_reports_unknown_rather_than_outside() {
        // 5 is readable, its parent 4 is not: unknown ancestry is never Outside.
        let read = tree(&[(5, "zsh", 4, 500)]);
        assert!(matches!(walk(5, read), Scope::Unknown(4)));
    }

    #[test]
    fn an_ancestry_walk_matches_every_recognized_comm() {
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
    fn an_ancestry_walk_terminates_on_a_cycle() {
        let read = tree(&[(5, "zsh", 6, 500), (6, "zsh", 5, 600)]);
        assert!(matches!(walk(5, read), Scope::Unknown(_)));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast an_ancestry_`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Write the implementation**

Above the test module in `src/relay/ancestry.rs`:

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

/// `comm` as the kernel reports it, and the agent `harness` relay publishes for it. The
/// `tty` predicate relay's own adapters apply belongs to qualified process resolution, not
/// to this test: a headless harness must not become a shell caller.
pub const HARNESS_COMMS: [(&str, &str); 3] = [
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

/// A chain longer than this is malformed; walking it forever is not an option.
const MAX_DEPTH: usize = 64;

/// The `comm` field can contain spaces and parentheses, so everything after the *last* `)`
/// is positional: state is field 1, ppid field 2, start time field 20 — the same rule
/// `claims::parse_proc_stat` documents for fields 3 and 22 of the whole line.
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

fn read_self_stat() -> Option<String> {
    std::fs::read_to_string("/proc/self/stat").ok()
}

/// Whether this host exposes the process tree at all. Stage 3 of spec §7 asks this, not
/// what the target triple says: a Linux build without a mounted `/proc` cannot establish
/// scope either.
pub fn proc_available() -> bool {
    read_self_stat().is_some()
}

pub fn self_ppid() -> Option<u32> {
    parse_entry(&read_self_stat()?).map(|entry| entry.ppid)
}

/// This caller's own position, resolved against the live `/proc`.
pub fn current_scope() -> Scope {
    match self_ppid() {
        Some(ppid) => walk(ppid, read_stat),
        None => Scope::Unknown(0),
    }
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

Run: `just test-fast an_ancestry_`
Expected: PASS, 6 tests.

- [ ] **Step 5: Verify the commit builds green**

Run: `just check`
Expected: PASS.

- [ ] **Step 6: Commit**

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
- Consumes: `snapshot::{Snapshot, Agent}`, `ancestry::{Scope, ProcEntry, HARNESS_COMMS}`.
- Produces:
  - `relay::resolve::Resolved { session: String, pid: u32, pid_start: u64, boot_id: String, host: String }`
  - `relay::resolve::hint_for(comm: &str, get: &impl Fn(&str) -> Option<String>) -> Result<Option<String>>`
  - `relay::resolve::same_session(claim_session: &str, comm: &str, session_id: &str) -> bool`
  - `relay::resolve::resolve(scope: Scope, load: impl FnOnce() -> Result<Snapshot>, host: &str, boot_id: Option<&str>, get: &impl Fn(&str) -> Option<String>) -> Result<Option<Resolved>>`

The snapshot arrives as a **closure**, not a value. Unknown ancestry and out-of-scope are
both decided before it is called, so the registry is never opened for a caller whose scope
is not established and a registry error can never mask an ancestry error.

- [ ] **Step 1: Write the failing tests**

Create `src/relay/resolve.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::snapshot::{Agent, Handle};

    const BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";
    const OTHER_BOOT: &str = "22222222-2222-4222-8222-222222222222";

    fn ancestor(comm: &str, pid: u32, start: u64) -> Scope {
        Scope::Harness(ProcEntry {
            pid,
            ppid: 1,
            comm: comm.into(),
            start,
        })
    }

    fn agent(harness: &str, session: &str, platform: &str, pid: u32, start: u64, boot: &str) -> Agent {
        Agent {
            id: format!("{harness}:{session}"),
            harness: harness.into(),
            session_id: session.into(),
            scope: if harness == "opencode" { "process" } else { "session" }.into(),
            process: Some(Handle {
                platform: platform.into(),
                host: "testhost".into(),
                boot_id: (platform == "linux").then(|| boot.to_string()),
                pid,
                start,
            }),
        }
    }

    fn snap(agents: Vec<Agent>) -> Snapshot {
        Snapshot {
            generation: "11111111-2222-4333-8444-555555555555".into(),
            revision: 1,
            agents,
        }
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

    fn go(
        scope: Scope,
        agents: Vec<Agent>,
        e: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Resolved>> {
        resolve(scope, || Ok(snap(agents)), "testhost", Some(BOOT), e)
    }

    #[test]
    fn a_match_adopts_the_agent_id_and_its_proof() {
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![agent("codex", "s1", "linux", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");
        assert_eq!(resolved.pid, 42);
        assert_eq!(resolved.pid_start, 900);
        assert_eq!(resolved.boot_id, BOOT);
        assert_eq!(resolved.host, "testhost");
    }

    #[test]
    fn a_match_is_not_attempted_out_of_scope() {
        let out = resolve(
            Scope::Outside,
            || panic!("the registry must not be opened out of scope"),
            "testhost",
            Some(BOOT),
            &env(&[]),
        )
        .unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn a_match_refuses_unknown_ancestry_before_opening_the_registry() {
        let error = resolve(
            Scope::Unknown(7),
            || panic!("the registry must not be opened for unknown ancestry"),
            "testhost",
            Some(BOOT),
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains('7'), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn a_match_requires_the_harness_to_agree_with_the_ancestor_comm() {
        // Satisfies host, pid, start and boot for a codex ancestor, but is a claude-code
        // row. Without the harness comparison this would be adopted and key the claim to
        // the wrong session.
        let error = go(
            ancestor("codex", 42, 900),
            vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("codex"), "{error}");
    }

    #[test]
    fn a_match_refuses_a_darwin_handle() {
        let error = go(
            ancestor("codex", 42, 900),
            vec![agent("codex", "s1", "darwin", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("darwin"), "{error}");
    }

    #[test]
    fn a_match_requires_every_proof_component() {
        let agents = || vec![agent("codex", "s1", "linux", 42, 900, BOOT)];
        assert!(go(ancestor("codex", 43, 900), agents(), &env(&[])).is_err()); // pid
        assert!(go(ancestor("codex", 42, 901), agents(), &env(&[])).is_err()); // start
        assert!(
            resolve(ancestor("codex", 42, 900), || Ok(snap(agents())), "elsewhere", Some(BOOT), &env(&[]))
                .is_err()
        ); // host
        assert!(
            resolve(ancestor("codex", 42, 900), || Ok(snap(agents())), "testhost", Some(OTHER_BOOT), &env(&[]))
                .is_err()
        ); // boot
    }

    #[test]
    fn a_match_on_an_empty_registry_is_the_explicit_identity_error() {
        // The real Codex case: SessionStart runs at the first turn, not at launch.
        let error = go(ancestor("codex", 42, 900), vec![], &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn a_match_is_ambiguous_only_among_fully_qualifying_candidates() {
        // Two rows share the handle and both qualify: ambiguous.
        let error = go(
            ancestor("codex", 42, 900),
            vec![
                agent("codex", "s1", "linux", 42, 900, BOOT),
                agent("codex", "s2", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("codex:s1") && error.contains("codex:s2"), "{error}");

        // A stale row from an earlier boot shares host, pid and start but cannot qualify.
        // Exactly one candidate satisfies the spec, so this must resolve, not refuse.
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![
                agent("codex", "old", "linux", 42, 900, OTHER_BOOT),
                agent("codex", "s1", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");

        // Likewise a wrong-harness row and a darwin row alongside one good row.
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![
                agent("claude-code", "c1", "linux", 42, 900, BOOT),
                agent("codex", "d1", "darwin", 42, 900, BOOT),
                agent("codex", "s1", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");
    }

    #[test]
    fn a_hint_from_another_harness_is_ignored() {
        // A codex session nested under a claude one inherits CLAUDE_CODE_SESSION_ID. Its
        // own proof and its own variable are valid and must be accepted.
        let e = env(&[("CLAUDE_CODE_SESSION_ID", "outer"), ("CODEX_SESSION_ID", "s1")]);
        assert_eq!(
            go(ancestor("codex", 42, 900), vec![agent("codex", "s1", "linux", 42, 900, BOOT)], &e)
                .unwrap()
                .unwrap()
                .session,
            "codex:s1"
        );
    }

    #[test]
    fn a_hint_for_the_nearest_harness_must_agree() {
        let agents = || vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)];
        let wrong = env(&[("CLAUDE_CODE_SESSION_ID", "different")]);
        assert!(go(ancestor("claude", 42, 900), agents(), &wrong).is_err());
        let right = env(&[("CLAUDE_CODE_SESSION_ID", "c1")]);
        assert!(go(ancestor("claude", 42, 900), agents(), &right).unwrap().is_some());
    }

    #[test]
    fn a_hint_pair_for_codex_must_agree_with_itself() {
        let agents = || vec![agent("codex", "s1", "linux", 42, 900, BOOT)];
        let split = env(&[("CODEX_SESSION_ID", "s1"), ("CODEX_THREAD_ID", "other")]);
        assert!(go(ancestor("codex", 42, 900), agents(), &split).is_err());
        let only_thread = env(&[("CODEX_THREAD_ID", "s1")]);
        assert!(go(ancestor("codex", 42, 900), agents(), &only_thread).unwrap().is_some());
    }

    #[test]
    fn a_hint_that_is_empty_is_unset_and_opencode_compares_none() {
        let claude = vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)];
        let empty = env(&[("CLAUDE_CODE_SESSION_ID", "")]);
        assert!(go(ancestor("claude", 42, 900), claude, &empty).unwrap().is_some());

        let opencode = vec![agent("opencode", "o1", "linux", 42, 900, BOOT)];
        let noisy = env(&[("CLAUDE_CODE_SESSION_ID", "x"), ("CODEX_SESSION_ID", "y")]);
        assert!(go(ancestor("opencode", 42, 900), opencode, &noisy).unwrap().is_some());
    }

    #[test]
    fn a_session_compares_equal_across_its_known_representations_only() {
        for stored in ["c1", "claude:c1", "claude-code:c1"] {
            assert!(same_session(stored, "claude", "c1"), "{stored}");
        }
        for stored in ["s1", "codex:s1"] {
            assert!(same_session(stored, "codex", "s1"), "{stored}");
        }
        assert!(!same_session("claude-code:other", "claude", "c1"));
        assert!(!same_session("sid:4242", "claude", "c1"));
        assert!(!same_session("codex:c1", "claude", "c1"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast a_match_ && just test-fast a_hint_ && just test-fast a_session_`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Write the implementation**

Above the test module in `src/relay/resolve.rs`:

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

const SET_EXPLICIT: &str =
    "set TASKS_SESSION and TASKS_SESSION_PID to name this session explicitly";

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

/// Whether `claim_session` names the session `session_id` under `comm`'s harness. The same
/// session is written more than one way across the levels: natively a Claude claim stores
/// the raw id, its tagged form is `claude:<id>`, and a relay agent id is
/// `claude-code:<id>`. Exactly those known forms compare equal; any other difference is a
/// real mismatch, never a change of notation.
pub fn same_session(claim_session: &str, comm: &str, session_id: &str) -> bool {
    if claim_session == session_id {
        return true;
    }
    if let Some(harness) = harness_for(comm)
        && claim_session == format!("{harness}:{session_id}")
    {
        return true;
    }
    comm == "claude" && claim_session == format!("claude:{session_id}")
}

/// Why nothing qualified. Spec §5 requires the harness and platform refusals to name what
/// was wrong, which the bare "no match" line cannot do, so a row that *is* this process is
/// diagnosed against each predicate in turn.
fn no_match(
    same_process: &[&crate::relay::snapshot::Agent],
    nearest: &ProcEntry,
    harness: &str,
    boot_id: &str,
) -> String {
    for agent in same_process {
        let process = agent.process.as_ref().expect("filtered on a present handle");
        if agent.harness != harness {
            return format!(
                "the agent on pid {} is {:?}, but the nearest ancestor comm {:?} maps to {:?}",
                nearest.pid, agent.harness, nearest.comm, harness
            );
        }
        if process.platform != "linux" {
            return format!(
                "agent {} has a {:?} handle, which is not an identity candidate",
                agent.id, process.platform
            );
        }
        if process.boot_id.as_deref() != Some(boot_id) {
            return format!("agent {} was recorded on an earlier boot", agent.id);
        }
    }
    format!(
        "no relay agent matches the nearest {} ancestor, pid {}",
        nearest.comm, nearest.pid
    )
}

/// `Ok(None)` means the caller is out of scope and the native ladder applies unchanged.
/// `load` is invoked only once scope is established as a harness boundary.
pub fn resolve(
    scope: Scope,
    load: impl FnOnce() -> Result<Snapshot>,
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
    // The hint is read before the registry so a self-contradicting pair is reported as
    // itself rather than as a match failure.
    let hint = hint_for(&nearest.comm, get)?;
    // The loader's own errors name the cause and the path, but they are registry errors,
    // not identity errors: they must still carry the §5 recovery instruction, because a
    // caller whose registry is missing needs to be told how to name itself.
    let snapshot = load().map_err(|error| refuse(error.to_string()))?;

    // Rows whose handle names this very process. Narrowing here first is what lets a
    // refusal say *why*: a row that is this process but fails one predicate is worth
    // naming, an unrelated row is not.
    let same_process: Vec<&crate::relay::snapshot::Agent> = snapshot
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

    // Every predicate of spec §4.3 applies *before* cardinality: a row that cannot qualify
    // — wrong harness, wrong platform, an earlier boot — is not a rival candidate, and
    // counting it would turn one good match into a false ambiguity.
    let qualifying: Vec<&crate::relay::snapshot::Agent> = same_process
        .iter()
        .copied()
        .filter(|agent| {
            agent.harness == harness
                && agent.process.as_ref().is_some_and(|process| {
                    process.platform == "linux" && process.boot_id.as_deref() == Some(boot_id)
                })
        })
        .collect();

    let agent = match qualifying.as_slice() {
        [one] => *one,
        [] => return Err(refuse(no_match(&same_process, &nearest, harness, boot_id))),
        many => {
            let ids: Vec<&str> = many.iter().map(|agent| agent.id.as_str()).collect();
            return Err(refuse(format!(
                "several relay agents match pid {}: {}",
                nearest.pid,
                ids.join(", ")
            )));
        }
    };

    if let Some(hint) = hint
        && hint != agent.session_id
    {
        // A qualifying agent exists, so the disagreement is the environment's, not the
        // registry's.
        return Err(refuse(format!(
            "the environment names session {hint:?} but the matched agent is {}",
            agent.id
        )));
    }

    let process = agent.process.as_ref().expect("filtered on a present handle");
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

Run: `just test-fast a_match_ && just test-fast a_hint_ && just test-fast a_session_`
Expected: PASS, 13 tests.

- [ ] **Step 5: Verify the commit builds green**

Run: `just check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/relay/
git commit -m "feat(relay): match the nearest harness ancestor and adopt its agent id"
```

---

### Task 5: The level in the ladder, adoption, and the platform refusal

**Files:**
- Modify: `src/claims.rs` (the `Identity` struct and `identity`/`identity_from`)
- Modify: `src/relay/mod.rs` (the staged entry point; **keeps** its `#![allow(dead_code)]` until Task 6)
- Modify: `src/config.rs` (remove its `#![allow(dead_code)]`)
- Test: `src/claims.rs`, `src/relay/mod.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `relay::resolve::Resolved`, `config::HostConfig`, `relay::ancestry`, `relay::snapshot`.
- Produces:
  - `claims::Proof { pid_start: u64, boot_id: String, host: String }`
  - `claims::Identity` gains `pub proof: Option<Proof>`
  - `claims::identity_from(get, session_pid, relay: impl FnOnce() -> Result<Option<Resolved>>, warnings) -> Result<Identity>`
  - `relay::stage_platform(enabled: bool, proc_available: bool) -> Result<()>`
  - `relay::enabled() -> Result<bool>`
  - `relay::level() -> Result<Option<Resolved>>`
  - `claims::identity(warnings)` keeps its signature, passing `relay::level`

- [ ] **Step 1: Write the failing tests**

Add to `src/claims.rs`'s existing `mod tests`:

```rust
    fn relay_off() -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
        || Ok(None)
    }

    fn relay_resolves(session: &str) -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
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
    fn a_ladder_explicit_pair_short_circuits_the_relay_level() {
        let identity = identity_from(
            env_of(&[("TASKS_SESSION", "explicit"), ("TASKS_SESSION_PID", "7")]),
            Some(11),
            || panic!("the relay level must not run when TASKS_SESSION is set"),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(identity.session, "explicit");
        assert_eq!(identity.pid, Some(7));
        assert!(identity.proof.is_none());
    }

    #[test]
    fn a_ladder_relay_level_beats_the_native_variables() {
        let identity = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "raw")]),
            Some(11),
            relay_resolves("claude-code:raw"),
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
    fn a_ladder_out_of_scope_leaves_the_native_levels_untouched() {
        let identity = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "raw")]),
            Some(11),
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(identity.session, "raw");
        assert_eq!(identity.tagged, "claude:raw");
        assert!(identity.proof.is_none());
    }

    #[test]
    fn a_ladder_relay_error_does_not_fall_to_a_lower_level() {
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

Add a `mod tests` to `src/relay/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_platform_refusal_needs_both_relay_on_and_no_proc() {
        // Relay off: the stage is never reached, whatever the host offers.
        assert!(stage_platform(false, false).is_ok());
        assert!(stage_platform(false, true).is_ok());
        // Relay on: a host with a readable process tree passes; one without is refused.
        assert!(stage_platform(true, true).is_ok());
        let error = stage_platform(true, false).unwrap_err().to_string();
        assert!(error.contains("/proc"), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast a_ladder_ && just test-fast a_stage_`
Expected: FAIL — `identity_from` takes three arguments, `Identity` has no `proof`, and
`stage_platform` is undefined.

- [ ] **Step 3: Write the implementation**

In `src/claims.rs`, add `Proof`, extend `Identity`, and insert the level:

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

Every existing `Identity { … }` literal in `claims.rs` gains `proof: None`. Change the
signature and insert the relay level immediately after the `TASKS_SESSION` block, before
the `CLAUDE_CODE_SESSION_ID` block:

```rust
pub fn identity_from(
    get: impl Fn(&str) -> Option<OsString>,
    session_pid: Option<u32>,
    relay: impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>>,
    warnings: &mut Vec<String>,
) -> Result<Identity> {
```

```rust
    // Stage 1 of spec §7 is above: an explicit pair short-circuits, so the relay level is
    // never invoked and none of its stages run.
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

and pass the production level from `identity`:

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

Replace the contents of `src/relay/mod.rs`, **keeping** its `#![allow(dead_code)]`. The
ladder consumes `level` and `enabled`, but `resolve::same_session` has no production
consumer until Task 6, so removing the allowance here would fail the `-D warnings` gate:

```rust
// `resolve::same_session` gains its consumer in Task 6, which removes this.
#![allow(dead_code)]

pub mod ancestry;
pub mod resolve;
pub mod snapshot;

use crate::config::HostConfig;
use crate::error::{Error, Result};

/// Whether the relay level is configured on. Read by callers that treat a relay-level
/// failure differently from a native one.
pub fn enabled() -> Result<bool> {
    Ok(HostConfig::load()?.relay_identity)
}

/// Stage 3 of spec §7, asking what the host offers rather than what the target triple
/// says: a Linux build without a mounted `/proc` cannot establish scope either. The
/// refusal belongs to the relay level, not to startup, so a session with `TASKS_SESSION`
/// set never reaches it and it can never take away the recovery path.
pub fn stage_platform(enabled: bool, proc_available: bool) -> Result<()> {
    if enabled && !proc_available {
        return Err(Error::Config(
            "relay identity is enabled but this host does not expose /proc, so ancestry \
             cannot be established. Disable [identity].relay, or set TASKS_SESSION and \
             TASKS_SESSION_PID to name this session explicitly."
                .into(),
        ));
    }
    Ok(())
}

/// Stages 2, 3 and 4 of spec §7, in order. `Ok(None)` means relay is off or the caller is
/// out of scope; either way the native ladder applies unchanged.
pub fn level() -> Result<Option<resolve::Resolved>> {
    // Stage 2: configuration. Nothing below runs when relay is not enabled.
    if !enabled()? {
        return Ok(None);
    }
    // Stage 3: platform support.
    stage_platform(true, ancestry::proc_available())?;
    // Stage 4: ancestry, and only then the registry. `resolve` decides `Outside` and
    // `Unknown` before calling the loader, so an unknown-ancestry caller is refused as
    // such rather than by whatever the registry read would have said.
    resolve::resolve(
        ancestry::current_scope(),
        snapshot::load,
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

Remove the `#![allow(dead_code)]` line from `src/config.rs` — `relay::enabled` and
`relay::level` consume it now. **Keep** the one in `src/relay/mod.rs`: `resolve::same_session`
still has no production consumer until Task 6, and removing the allowance here would fail
the `-D warnings` gate.

Update the twelve existing `identity_from(…)` call sites in `claims.rs`'s `mod tests` to
pass the new third argument. They all take the same form:

```rust
// before
identity_from(env_of(&[…]), Some(11), &mut Vec::new())
// after
identity_from(env_of(&[…]), Some(11), relay_off(), &mut Vec::new())
```

`grep -n "identity_from(" src/claims.rs` lists all of them; the file has fourteen
occurrences in total — the definition, the call inside `identity`, and twelve in tests.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast a_ladder_ && just test-fast a_stage_`
Expected: PASS, 5 tests. Every pre-existing `claims` test still passes unchanged.

- [ ] **Step 5: Run the whole suite**

Run: `just gate`
Expected: PASS. `src/relay/mod.rs` still carries its allowance; Task 6 removes the last one.

- [ ] **Step 6: Commit**

```bash
git add src/claims.rs src/relay/ src/config.rs
git commit -m "feat(claims): add the opt-in relay level and carry its process proof"
```

---

### Task 6: Continuity in the guards

**Files:**
- Modify: `src/claims.rs` (add `Resolution`, `continuation_identity`, `proves_ownership`)
- Modify: `src/commands/mod.rs` (`refuse_foreign_live_claim`, `claim_guard`, new `Ctx` helpers)
- Modify: `src/commands/park.rs`
- Test: `src/claims.rs` (inline `mod tests`), `tests/cli.rs`

**Interfaces:**
- Consumes: `claims::{Claim, Identity, Proof}`, `relay::ancestry`, `relay::resolve::{hint_for, same_session}`, `relay::enabled`.
- Produces:
  - `claims::Resolution` — `Resolved(Identity)` | `Failed(Error)`, with `identity(&self) -> Option<&Identity>` and `require(self) -> Result<Identity>`
  - `claims::resolve_identity(warnings: &mut Vec<String>) -> Resolution`
  - `claims::continuation_identity(claim: &Claim) -> Identity`
  - `claims::name_the_claim(error: Error, held: Option<&str>) -> Error`
  - `claims::proves_ownership(claim: &Claim, scope: &Scope, host: &str, boot_id: Option<&str>, get: &impl Fn(&str) -> Option<String>) -> bool`
  - `commands::Ownership` — `ByIdentity` | `ByProof` | `Foreign` (`pub(crate)`)
  - `Ctx::resolve_for_guard(&mut self) -> Result<Resolution>` (`pub(crate)`)
  - `Ctx::ownership(&mut self, claim: &Claim, me: &Resolution) -> Result<Ownership>` (`pub(crate)`)

Also removes the last `#![allow(dead_code)]`, from `src/relay/mod.rs`: `same_session` gains
its production consumer here, so the file is staged in this task's commit.

- [ ] **Step 1: Write the failing unit tests**

Add to `src/claims.rs`'s `mod tests`:

```rust
    use crate::relay::ancestry::{ProcEntry, Scope};

    const PROOF_BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";

    fn claim_of(session: &str, pid: Option<u32>, pid_start: Option<u64>) -> Claim {
        Claim {
            owner: "tester".into(),
            session: session.into(),
            pid,
            pid_start,
            boot_id: Some(PROOF_BOOT.into()),
            host: "testhost".into(),
            worktree: "/w".into(),
            started: "2026-09-22T00:00:00Z".into(),
            seen: "2026-09-22T00:00:00Z".into(),
        }
    }

    fn nearest(comm: &str, pid: u32, start: u64) -> Scope {
        Scope::Harness(ProcEntry {
            pid,
            ppid: 1,
            comm: comm.into(),
            start,
        })
    }

    fn no_env() -> impl Fn(&str) -> Option<String> {
        |_| None
    }

    #[test]
    fn a_proof_establishes_the_owner_from_the_claims_own_contents() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_refuses_a_session_nested_under_the_owner() {
        // claude(8) owns the claim; a codex(9) session launched beneath it has 8 among its
        // ancestors, but its *nearest* boundary is 9. Two distinct harness processes with
        // two distinct identities: it must be refused.
        let claim = claim_of("claude-code:c1", Some(8), Some(800));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 9, 900),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_is_defeated_by_a_contradicted_hint() {
        let claim = claim_of("claude-code:c1", Some(42), Some(900));
        let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "other".to_string());
        assert!(!proves_ownership(
            &claim,
            &nearest("claude", 42, 900),
            "testhost",
            Some(PROOF_BOOT),
            &env
        ));
    }

    #[test]
    fn a_proof_survives_a_known_representation_change() {
        for stored in ["c1", "claude:c1", "claude-code:c1"] {
            let claim = claim_of(stored, Some(42), Some(900));
            let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "c1".to_string());
            assert!(
                proves_ownership(&claim, &nearest("claude", 42, 900), "testhost", Some(PROOF_BOOT), &env),
                "{stored}"
            );
        }
    }

    #[test]
    fn a_proof_fails_on_pid_reuse_a_new_boot_or_another_host() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 901),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some("22222222-2222-4222-8222-222222222222"),
            &no_env()
        ));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "elsewhere",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_is_unavailable_without_recorded_process_evidence() {
        // Native Claude without CLAUDE_PID, and every Codex claim, carry pid: None.
        let claim = claim_of("codex:s1", None, None);
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_needs_an_established_harness_boundary() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(!proves_ownership(&claim, &Scope::Outside, "testhost", Some(PROOF_BOOT), &no_env()));
        assert!(!proves_ownership(&claim, &Scope::Unknown(4), "testhost", Some(PROOF_BOOT), &no_env()));
    }

    #[test]
    fn a_continuation_identity_is_the_claims_own() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        let me = continuation_identity(&claim);
        assert_eq!(me.session, "codex:s1");
        assert_eq!(me.tagged, "codex:s1");
        assert_eq!(me.pid, Some(42));
        let proof = me.proof.unwrap();
        assert_eq!(proof.pid_start, 900);
        assert_eq!(proof.boot_id, PROOF_BOOT);
        assert_eq!(proof.host, "testhost");

        // A claim with no recorded proof yields an identity with none.
        assert!(continuation_identity(&claim_of("codex:s1", None, None)).proof.is_none());
    }
```

- [ ] **Step 2: Run the unit tests to verify they fail**

Run: `just test-fast a_proof_ && just test-fast a_continuation_`
Expected: FAIL — `proves_ownership` and `continuation_identity` are undefined.

- [ ] **Step 3: Write the claims-side implementation**

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

/// The identity to record when ownership was established by **proof alone** — the claim's
/// own. Spec §6.4: `existing.session` is never rewritten, so a claim keeps the identity it
/// was created with, and enabling relay mid-flight cannot re-key a natively-held claim.
///
/// This is deliberately *not* used when the resolved identity already matched the claim.
/// That path keeps using the freshly resolved identity, exactly as today, because the two
/// agree on the session anyway and the fresh one carries current fields — a repeated
/// `start` must still be able to replace a stale pid with the one this session supplies,
/// and a native level's `tagged` form (`codex:<id>` for a claim whose session is the raw
/// id) must still reach the park and escalation records that `tests/cli.rs` asserts.
pub fn continuation_identity(claim: &Claim) -> Identity {
    Identity {
        session: claim.session.clone(),
        tagged: claim.session.clone(),
        pid: claim.pid,
        proof: match (claim.pid_start, &claim.boot_id) {
            (Some(pid_start), Some(boot_id)) => Some(Proof {
                pid_start,
                boot_id: boot_id.clone(),
                host: claim.host.clone(),
            }),
            _ => None,
        },
    }
}

/// Spec §6.2: does this caller own `claim`, proved from the claim's own recorded process
/// handle and the caller's ancestry, with no registry read?
/// Name the claim an error was raised against, so a resolution failure on a claimed task
/// tells the operator which session to set `TASKS_SESSION` to. Spec §§5 and 6.5.
pub fn name_the_claim(error: Error, held: Option<&str>) -> Error {
    match held {
        Some(session) => Error::Config(format!(
            "{error}; the existing claim is held by session {session:?}"
        )),
        None => error,
    }
}

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
    if boot_id.is_none() || claim.host != host || claim.boot_id.as_deref() != boot_id {
        return false;
    }
    // 2. the *nearest* harness boundary, not any ancestor: a session nested under the
    //    owner has the owner among its ancestors and must still be refused.
    if nearest.pid != pid || nearest.start != pid_start {
        return false;
    }
    // 3. a scoped hint must not contradict the claim, so that deferring a resolution error
    //    cannot turn a contradicted session into an accepted owner.
    match crate::relay::resolve::hint_for(&nearest.comm, get) {
        Ok(Some(hint)) => crate::relay::resolve::same_session(&claim.session, &nearest.comm, &hint),
        Ok(None) => true,
        Err(_) => false,
    }
}
```

- [ ] **Step 4: Write the guard-side implementation**

Add the ownership verdict and two helpers to `src/commands/mod.rs`:

```rust
/// How this caller's right to act on an existing claim was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ownership {
    /// The resolved identity equals the claim's session. Today's path, unchanged: the
    /// resolved identity is used, with its current fields.
    ByIdentity,
    /// Identity did not match, or did not resolve at all, but the claim's own recorded
    /// process proof names this caller's nearest harness boundary.
    ByProof,
    /// Not the owner. Anything from here is acquisition.
    Foreign,
}
```


```rust
    /// Resolve identity, keeping a relay-level failure carryable so §6.2.2 can defer it. A
    /// failure with relay off is raised here, exactly where `identity` raised it before,
    /// which is what holds the relay-off guarantee at every call site rather than at some.
    pub(crate) fn resolve_for_guard(&mut self) -> Result<crate::claims::Resolution> {
        let me = crate::claims::resolve_identity(&mut self.warnings);
        match me {
            crate::claims::Resolution::Failed(error) if !crate::relay::enabled()? => Err(error),
            other => Ok(other),
        }
    }

    /// Spec §6.2.2 steps 2 and 3. Which of the two established the caller's right to act
    /// matters: only proof-only ownership records the claim's own identity, because only
    /// then is there no resolved identity that already agrees.
    pub(crate) fn ownership(
        &mut self,
        claim: &crate::claims::Claim,
        me: &crate::claims::Resolution,
    ) -> Result<Ownership> {
        if let Some(identity) = me.identity()
            && claim.session == identity.session
        {
            return Ok(Ownership::ByIdentity);
        }
        // Proof is a relay-mode fallback, and it never overrides the explicit pair: agents
        // sharing one process are distinguished by TASKS_SESSION and by nothing else, so an
        // explicit mismatch is foreign however the ancestry looks. Spec constraint §2.1.
        let explicit = std::env::var_os("TASKS_SESSION").is_some_and(|value| !value.is_empty());
        if explicit || !crate::relay::enabled()? {
            return Ok(Ownership::Foreign);
        }
        let proved = crate::claims::proves_ownership(
            claim,
            &crate::relay::ancestry::current_scope(),
            &crate::claims::hostname(),
            crate::claims::boot_id().as_deref(),
            &|key| {
                std::env::var_os(key)
                    .and_then(|value| value.into_string().ok())
                    .filter(|value| !value.is_empty())
            },
        );
        Ok(if proved {
            Ownership::ByProof
        } else {
            Ownership::Foreign
        })
    }
```

Rewrite `refuse_foreign_live_claim`. Identity is resolved first, exactly as before, so an
absent or stale claim behaves as it does today:

```rust
    pub fn refuse_foreign_live_claim(&mut self, id: &TaskId) -> Result<()> {
        let me = self.resolve_for_guard()?;
        let existing = self.claims_mut()?.get(id).cloned();
        let Some(existing) = existing else {
            return me.require().map(|_| ());
        };
        if self.ownership(&existing, &me)? != Ownership::Foreign {
            return Ok(());
        }
        // Not the owner. Today's behaviour resolved an identity here whatever the verdict,
        // so a relay-level failure must still surface rather than be silently tolerated —
        // and before any refusal, so the operator sees why identity failed rather than a
        // refusal that merely follows from it.
        let held = existing.session.clone();
        me.require()
            .map_err(|error| crate::claims::name_the_claim(error, Some(&held)))?;
        let live = crate::claims::liveness(&existing);
        if live == Liveness::Live {
            return Err(Error::Claimed(
                id.to_string(),
                Ctx::describe_claim(&existing, &live),
            ));
        }
        Ok(())
    }
```

Rewrite `claim_guard`:

```rust
    fn claim_guard(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let resolution = self.resolve_for_guard()?;
        let owner = owner_name(&self.project)?;
        let worktree = self.project.root.display().to_string();

        let existing = self.claims_mut()?.get(id).cloned();
        let ownership = match &existing {
            Some(claim) => self.ownership(claim, &resolution)?,
            None => Ownership::Foreign,
        };
        let mine = ownership != Ownership::Foreign;

        // The identity to record, decided *before* any refusal. A held resolution error
        // must be raised as itself the moment ownership fails (§6.2.2); letting a
        // `Claimed` refusal return first would hide why identity could not resolve.
        let me = match (&existing, ownership) {
            // Ownership by identity means resolution succeeded, so `require` cannot fail.
            // The resolved identity is used with its current fields, exactly as today: a
            // repeated `start` still replaces a stale pid, and a native level's `tagged`
            // form still reaches the park and escalation records.
            (_, Ownership::ByIdentity) => resolution.require()?,
            (Some(claim), Ownership::ByProof) => crate::claims::continuation_identity(claim),
            _ => {
                let held = existing.as_ref().map(|claim| claim.session.clone());
                resolution
                    .require()
                    .map_err(|error| crate::claims::name_the_claim(error, held.as_deref()))?
            }
        };

        let mut warning = None;
        if let Some(existing) = &existing {
            let live = crate::claims::liveness(existing);
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

        self.pending_claim = Some(if to == Status::Doing {
            let now = crate::time::now();
            let started = match (&existing, mine) {
                (Some(claim), true) => claim.started.clone(),
                _ => now.clone(),
            };
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

In `src/commands/park.rs`, replace the identity line at `:38`. `park` needs a concrete
`Identity` because it writes `me.tagged` into both the park entry (`:117`) and the
escalation record (`:166`), and compares `me.session` at `:45`; resolving to a
`Resolution` there would not compile. A proved owner supplies its continuation identity;
anything else — parking an unclaimed task, taking over a stale claim — is acquisition and
requires a resolved one:

```rust
    let resolution = ctx.resolve_for_guard()?;
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let held = existing.as_ref().map(|claim| claim.session.clone());
    let me = match &existing {
        Some(claim) => match ctx.ownership(claim, &resolution)? {
            crate::commands::Ownership::ByIdentity => resolution.require()?,
            crate::commands::Ownership::ByProof => crate::claims::continuation_identity(claim),
            crate::commands::Ownership::Foreign => resolution
                .require()
                .map_err(|error| crate::claims::name_the_claim(error, held.as_deref()))?,
        },
        // Parking an unclaimed task records a session, so it is acquisition too.
        None => resolution.require()?,
    };
```

Everything below is unchanged: `me.session` at `:45`, `me.tagged.clone()` at `:117` and
`me.tagged` at `:166` all still typecheck. For a proved owner `me.session` equals
`existing.session`, so the comparison at `:45` takes its own-claim branch; for an ordinary
identity match the freshly resolved identity is used, so a native Codex park still records
`codex:<id>` in `me.tagged` — which `tests/cli.rs:118` asserts.

- [ ] **Step 5: Write the command-level continuity tests**

Add to `tests/cli.rs`. These use the explicit pair, so they exercise continuity and the
acquisition rules without needing a harness ancestor; Task 8 covers the relay paths:

```rust
#[test]
fn a_continuity_repeated_start_by_the_owner_keeps_one_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    for _ in 0..2 {
        env.cmd(&dir)
            .env("TASKS_SESSION", "owner")
            .args(["start", &id])
            .assert()
            .success();
    }
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(store.matches("session = \"owner\"").count(), 1, "{store}");
}

// NOTE: the explicit-mismatch case lives in Task 8 as an acceptance test. It has to run
// under a harness shim with relay enabled and a matching boundary, or removing the
// explicit-identity guard from `Ctx::ownership` would leave it passing — proof would never
// have been consulted in the first place.

#[test]
fn a_continuity_park_and_close_by_the_owner_still_work() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let session = || ("TASKS_SESSION", "owner");
    env.cmd(&dir).env(session().0, session().1).args(["start", &id]).assert().success();
    env.cmd(&dir).env(session().0, session().1).args(["park", &id, "next"]).assert().success();
    env.cmd(&dir).env(session().0, session().1).args(["start", &id]).assert().success();
    env.cmd(&dir).env(session().0, session().1).args(["done", &id, "landed"]).assert().success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "done");
}
```

- [ ] **Step 6: Run the tests**

Run: `just test-fast a_proof_ && just test-fast a_continuation_ && just test-fast a_continuity_`
Expected: PASS — 7, 1 and 2 tests.

- [ ] **Step 7: Run the whole suite**

Run: `just gate`
Expected: PASS — every pre-existing claim, park and status test behaves unchanged. In
particular `a_codex_claim_outlives_the_command_that_made_it` (`tests/cli.rs:79`) must still
see `codex:thread-a` in the park record: it is the regression that catches a continuation
identity applied where an ordinary identity match belongs.

- [ ] **Step 8: Commit**

```bash
git add src/claims.rs src/commands/mod.rs src/commands/park.rs src/relay/mod.rs tests/cli.rs
git commit -m "feat(claims): prove claim ownership without the registry; acquisition still needs identity"
```

---

### Task 7: Continuity in `note`

**Files:**
- Modify: `src/commands/status.rs` (`note`, beginning at `:82`)
- Modify: `tests/common/mod.rs` (clear `RELAY_STATE_DIR`; add the harness shim helper)
- Test: `tests/cli.rs`

`note` is deliberately not folded into Task 6's common path. It guards nothing: it never
refuses a foreign claim and never touches one, and its identity resolution exists only to
decide whether to refresh its own claim's heartbeat. Spec §6.6.

This task introduces the harness shim, because it is the first test here that needs a real
harness ancestor: a test binary's parent is the test runner, so no ambient ancestry exists
and a test that assumes one silently exercises the out-of-scope path instead.

**Interfaces:**
- Consumes: `Ctx::resolve_for_guard`, `Ctx::ownership`, `commands::Ownership`.
- Produces: `tests/common/mod.rs::harness_shim(dir, home, comm, script) -> std::process::Output` and `WRITE_REGISTRY`.

- [ ] **Step 1: Add the harness shim to the test harness**

In `tests/common/mod.rs`, add `.env_remove("RELAY_STATE_DIR")` to both `cmd` and `raw`,
beside the existing `.env_remove("XDG_STATE_HOME")`, so no test inherits a real registry.
Then add:

```rust
/// Run `script` under a process whose `comm` is `comm`, so the `tasks` it launches has a
/// recognized harness ancestor. A copy of `/bin/sh` supplies the comm. Nothing is
/// `exec`ed: `exec` would replace the shim with `tasks`, which would inherit the shim's
/// pid and its parent, destroying the ancestry under test.
pub fn harness_shim(
    dir: &Path,
    home: &Path,
    comm: &str,
    script: &str,
) -> std::process::Output {
    use std::os::unix::fs::PermissionsExt;
    let shim = home.join(comm);
    std::fs::copy("/bin/sh", &shim).unwrap();
    let mut perms = std::fs::metadata(&shim).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&shim, perms).unwrap();
    std::process::Command::new(&shim)
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_FORMAT")
        .env_remove("TASKS_OWNER")
        .env_remove("TASKS_SESSION")
        .env_remove("TASKS_SESSION_PID")
        .env_remove("TASKS_MODEL")
        .env_remove("TASKS_AGENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_PID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .env("TASKS_BIN", assert_cmd::cargo::cargo_bin("tasks"))
        .output()
        .unwrap()
}

/// Shell function that writes a one-agent registry naming the *shim's own* process, with
/// the private mode relay requires, then leaves `$TASKS_BIN` ready to run.
///
/// The start token must come from `/proc/$$/stat`, the shim's own stat file. Reading
/// `/proc/self/stat` inside a `$(…)` substitution reads the *substituting* process — a
/// different process with a different start token — and pairing that with `$$` produces a
/// handle that matches only if two processes happened to start within one clock tick.
pub const WRITE_REGISTRY: &str = r#"
write_registry() {
  start=$(awk '{print $22}' "/proc/$$/stat")
  boot=$(cat /proc/sys/kernel/random/boot_id)
  host=$(cat /proc/sys/kernel/hostname)
  mkdir -p "$RELAY_STATE_DIR"
  chmod 700 "$RELAY_STATE_DIR"
  cat > "$RELAY_STATE_DIR/agents.json" <<EOF
{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,
 "agents":{"$AGENT_ID":{"id":"$AGENT_ID","harness":"$HARNESS","sessionId":"$SESSION",
  "scope":"session","cwd":"/w","repoRoot":null,"remote":null,"projectKey":"k",
  "project":"p","state":"idle","updatedAt":1,
  "process":{"platform":"linux","host":"$host","bootId":"$boot","pid":$$,"start":"$start"}}}}
EOF
  chmod 600 "$RELAY_STATE_DIR/agents.json"
}
"#;
```

Add a helper to `tests/cli.rs` that turns on relay for a test home and returns the state
directory:

```rust
/// Enable relay identity for this test's HOME and return its relay state directory.
fn relay_on(env: &TestEnv) -> std::path::PathBuf {
    let config = env.home.path().join(".config/tasks/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[identity]\nrelay = true\n").unwrap();
    env.home.path().join("relay-state")
}

/// The preamble every shim script shares: exports, then the registry writer.
fn shim_env(state: &std::path::Path, harness: &str, session: &str) -> String {
    format!(
        "RELAY_STATE_DIR={}\nHARNESS={harness}\nSESSION={session}\nAGENT_ID={harness}:{session}\n\
         export RELAY_STATE_DIR HARNESS SESSION AGENT_ID TASKS_BIN\n{}",
        state.display(),
        common::WRITE_REGISTRY
    )
}
```

- [ ] **Step 2: Write the failing tests**

Add to `tests/cli.rs`:

```rust
#[test]
fn a_note_lands_when_relay_identity_cannot_resolve() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // In scope under a live codex ancestor, but the registry holds no agents — the real
    // Codex case before its first SessionStart. Identity cannot resolve; the note must
    // land anyway, and must say why the heartbeat was skipped.
    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":1,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n\
         \"$TASKS_BIN\" note {id} 'still lands'\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);

    // `start` is acquisition and must fail; `note` must still succeed.
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"warnings\""), "note should have produced output: {text}");
    let shown = env.json(&dir, &["show", &id]);
    assert!(
        shown["task"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note["text"].as_str().unwrap().contains("still lands")),
        "the note must land even though relay identity could not resolve"
    );
}

#[test]
fn a_note_warns_when_ownership_evidence_is_unavailable() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Claim under a working registry, then remove it and note with a contradicted hint so
    // neither identity nor proof can establish ownership.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         CODEX_SESSION_ID=someone-else \"$TASKS_BIN\" note {id} 'orphan note'\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("heartbeat") && text.contains("not refreshed"),
        "an unprovable claim must say the heartbeat was skipped: {text}"
    );
}

#[test]
fn a_note_from_a_foreign_session_leaves_the_claim_alone() {
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

    let out = env
        .cmd(&dir)
        .env("TASKS_SESSION", "stranger")
        .args(["note", &id, "from elsewhere"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let after = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(before, after, "a foreign note must not refresh or alter the claim");
    // An ordinary foreign note resolved its identity fine, so it must not warn.
    assert!(!String::from_utf8_lossy(&out.stdout).contains("heartbeat"));
}

#[test]
fn a_note_with_relay_off_is_unchanged_under_a_harness_ancestor() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // No config file: relay is off, so the harness ancestor is irrelevant and the native
    // ladder resolves as it always has.
    let script = format!("\"$TASKS_BIN\" start {id}\n\"$TASKS_BIN\" note {id} 'native'\n");
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("heartbeat"));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `just test-fast a_note_`
Expected: `a_note_lands_when_relay_identity_cannot_resolve` and
`a_note_warns_when_ownership_evidence_is_unavailable` FAIL; the other two pass already and
must keep passing.

- [ ] **Step 4: Write the implementation**

In `status::note`, replace the identity resolution and the `mine` filter:

```rust
    // Identity and the store are resolved *before* the file write. Doing it afterwards
    // means a corrupt store returns an error after the note has already landed, and the
    // obvious retry then duplicates it.
    //
    // `resolve_for_guard` is what keeps the two failure kinds apart, and is why `note`
    // needs no special case of its own: with relay off it raises exactly where `identity`
    // raised before, so an unresolvable native identity is still fatal and the note still
    // does not land; with relay on it carries the failure, and the note lands.
    let me = ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    save(&mut ctx, &mut task)?;

    // Use the pruned store so a note cannot revive a stale claim.
    let existing = ctx.claims_mut()?.get(&task.id).cloned();
    let mine = match &existing {
        Some(claim) => ctx.ownership(claim, &me)? != crate::commands::Ownership::Foreign,
        None => false,
    };

    // The note has landed. If a claim exists that we could not establish ownership of
    // *because our own identity did not resolve*, say so: silence here would look
    // identical to an ordinary foreign note, which is a different situation entirely.
    if let Some(claim) = &existing
        && !mine
        && me.identity().is_none()
    {
        ctx.warnings.push(format!(
            "the note landed, but the claim heartbeat on {} was not refreshed (this \
             session's identity could not be resolved, so ownership of the claim held by \
             {} could not be established); the claim may look stale to other sessions",
            task.id, claim.session
        ));
    }
    let mine = existing.filter(|_| mine);
```

The heartbeat block below is unchanged, as is its own warning when the save fails.
`Ctx::ownership` and `Ctx::resolve_for_guard` are `pub(crate)` from Task 6. `note` does not
distinguish the two kinds of ownership: it writes no session anywhere, only a `seen`
timestamp on a claim it leaves otherwise untouched.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `just test-fast a_note_`
Expected: PASS, 4 tests.

- [ ] **Step 6: Run the whole suite**

Run: `just gate`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/commands/status.rs tests/
git commit -m "feat(note): keep the heartbeat on proof and never lose a note to relay resolution"
```

---

### Task 8: Acceptance tests and documentation

**Files:**
- Modify: `tests/cli.rs`
- Modify: `README.md`, `skills/tasks/SKILL.md`
- Test: `tests/cli.rs`

The four cases the spec review asked to keep explicit — nested harness, contradictory hint,
registry loss, relay off — plus acquisition and the private-path refusal. All reuse
`harness_shim` and `WRITE_REGISTRY` from Task 7.

**Interfaces:**
- Consumes: everything above.
- Produces: no new interface.

- [ ] **Step 1: Write the acceptance tests**

Add to `tests/cli.rs`:

```rust
#[test]
fn an_acceptance_relay_claim_is_keyed_by_the_agent_id_with_its_proof() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));

    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"codex:s1\""), "{store}");
    assert!(store.contains("pid_start"), "{store}");
    assert!(store.contains("boot_id"), "{store}");
}

#[test]
fn an_acceptance_owner_can_park_and_close_after_the_registry_is_removed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let parked = id_of(env.json(&dir, &["add", "To park", "-p", "2"]));
    let closed = id_of(env.json(&dir, &["add", "To close", "-p", "2"]));
    let state = relay_on(&env);

    // Two *separately held* claims. Park and close are both releases by the owner, but
    // `Store::insert_park` removes the claim along with its proof, so a task cannot be
    // parked and then resumed on the same run: resumption is acquisition again and needs
    // either a restored registry or the explicit override.
    let script = format!(
        "{}\nwrite_registry\n\
         \"$TASKS_BIN\" start {parked}\n\
         \"$TASKS_BIN\" start {closed}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" park {parked} 'next step'\n\
         \"$TASKS_BIN\" done {closed} landed\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the owner must park and close with the registry gone: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(env.json(&dir, &["show", &closed])["task"]["status"], "done");
}

#[test]
fn an_acceptance_resuming_a_parked_task_needs_identity_again() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Parking released the claim and its proof with it, so the resume is a fresh
    // acquisition. With the registry still gone it must refuse rather than quietly
    // claim under some other identity.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" park {id} 'next step'\n\
         \"$TASKS_BIN\" start {id} && echo RESUMED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("RESUMED"),
        "a parked task cannot be resumed without a resolvable identity"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("TASKS_SESSION"));
}

#[test]
fn an_acceptance_repeated_start_after_registry_loss_keeps_the_held_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(store.matches("session = \"codex:s1\"").count(), 1, "{store}");
}

#[test]
fn an_acceptance_nested_harness_cannot_close_the_outer_sessions_task() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // A claude shim claims the task, then runs a codex shim beneath itself which tries to
    // close it. The inner session's nearest boundary is the codex process, so its
    // ownership proof against the claude claim must fail.
    use std::os::unix::fs::PermissionsExt;
    let inner = env.home.path().join("codex");
    std::fs::copy("/bin/sh", &inner).unwrap();
    let mut perms = std::fs::metadata(&inner).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&inner, perms).unwrap();

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         \"{}\" -c '\"$TASKS_BIN\" done {id} landed' && echo INNER_CLOSED\n",
        shim_env(&state, "claude-code", "c1"),
        inner.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        !text.contains("INNER_CLOSED"),
        "a session nested under the owner must not close its task: {text}"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_contradicted_hint_is_refused_at_acquisition() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\nCODEX_SESSION_ID=someone-else \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("someone-else") && text.contains("codex:s1"), "{text}");
}

#[test]
fn an_acceptance_contradicted_hint_is_refused_on_a_held_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // The claim is held and carries proof; the registry is then removed, so ownership can
    // only come from proof. A contradicted hint must defeat it — a deferred resolution
    // error must not become an accepted owner.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         CODEX_SESSION_ID=someone-else \"$TASKS_BIN\" done {id} landed\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_empty_registry_refuses_rather_than_using_terminal_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":1,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("TASKS_SESSION"));
    let store = env.claim_store("sci");
    assert!(
        !store.exists() || !std::fs::read_to_string(&store).unwrap().contains("sid:"),
        "terminal identity must never be used in scope"
    );
}

#[test]
fn an_acceptance_world_readable_registry_is_refused() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\nchmod 644 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("privately owned"));
}

#[test]
fn an_acceptance_explicit_pair_works_under_a_harness_with_no_registry() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "RELAY_STATE_DIR={} TASKS_SESSION=explicit \"$TASKS_BIN\" start {id}\n",
        state.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::read_to_string(env.claim_store("sci")).unwrap().contains("explicit"));
}

#[test]
fn an_acceptance_explicit_mismatch_stays_foreign_under_one_harness() {
    // Two workers beneath the *same* shim, so the ancestry, host and boot all agree and
    // the claim's proof names their shared harness process. Only TASKS_SESSION tells them
    // apart. If `Ctx::ownership` stopped honouring the explicit pair, worker-b's proof
    // would succeed and this close would land — which is exactly the bypass to catch.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\
         TASKS_SESSION=worker-a TASKS_SESSION_PID=$$ \"$TASKS_BIN\" start {id}\n\
         TASKS_SESSION=worker-b TASKS_SESSION_PID=$$ \"$TASKS_BIN\" done {id} 'not mine' \
           && echo FOREIGN_CLOSED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("FOREIGN_CLOSED"),
        "an explicit session mismatch must stay foreign however the ancestry looks"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_mode_change_continues_a_natively_held_claim() {
    // Acquisition, the configuration change, and the continuation all happen inside **one**
    // shim. A second `harness_shim` would be a different process whose ancestry cannot
    // prove anything about the first one's claim, so the close would succeed through
    // ordinary stale takeover and the test would pass with proof-based continuity broken.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = env.home.path().join("relay-state");
    let after_start = env.home.path().join("after-start.toml");

    // Claim natively with a pid, so the claim carries proof and is keyed `c1`. Then enable
    // relay mid-run: identity now resolves to `claude-code:c1`, which does *not* equal the
    // claim's session, so only proof can establish ownership.
    // `set -e`: every command here must succeed, and a failed repeated `start` must not be
    // masked by a `done` that then takes the claim over.
    let script = format!(
        "set -e\n{}\nwrite_registry\n\
         CLAUDE_CODE_SESSION_ID=c1 CLAUDE_PID=$$ \"$TASKS_BIN\" start {id}\n\
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"$HOME/native.toml\"\n\
         mkdir -p \"$HOME/.config/tasks\"\n\
         printf '[identity]\\nrelay = true\\n' > \"$HOME/.config/tasks/config.toml\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" start {id}\n\
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" done {id} landed\n",
        shim_env(&state, "claude-code", "c1"),
        after_start.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a natively-held claim carrying proof must be continued: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let native = std::fs::read_to_string(env.home.path().join("native.toml")).unwrap();
    assert!(native.contains("session = \"c1\""), "claimed natively: {native}");

    // The repeated `start` under relay kept the claim's own key: no re-keying to
    // `claude-code:c1`, and one claim rather than a takeover of a foreign one.
    let refreshed = std::fs::read_to_string(&after_start).unwrap();
    assert!(refreshed.contains("session = \"c1\""), "re-keyed: {refreshed}");
    assert!(!refreshed.contains("claude-code:c1"), "re-keyed: {refreshed}");
    assert_eq!(refreshed.matches("session = ").count(), 1, "{refreshed}");

    // A continuation is not a takeover, so nothing may have warned about one.
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("took over"), "continuation must not take over: {text}");
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "done");
}

#[test]
fn an_acceptance_force_cannot_take_over_without_a_resolved_identity() {
    // A stale foreign claim and no resolvable identity. `--force` may displace an owner,
    // but a takeover records a new owner and there is none to record.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Claim as an unrelated session whose pid is long gone, so the claim reads stale.
    env.cmd(&dir)
        .env("TASKS_SESSION", "departed")
        .env("TASKS_SESSION_PID", "999999")
        .args(["start", &id])
        .assert()
        .success();

    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":1,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start --force {id} && echo FORCED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("FORCED"),
        "--force must not substitute for an unresolvable identity"
    );
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("TASKS_SESSION"), "{text}");
    // The held error names the claim it was raised against, per §§5 and 6.5.
    assert!(text.contains("departed"), "{text}");
}

#[test]
fn an_acceptance_relay_off_keeps_a_harness_session_on_the_native_ladder() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // No config file: relay is off, and the registry below is never consulted.
    let state = env.home.path().join("relay-state");

    let script = format!(
        "{}\nwrite_registry\nCODEX_SESSION_ID=s1 \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(
        store.contains("session = \"s1\"") && !store.contains("codex:s1"),
        "relay off must key the claim by the native raw id: {store}"
    );
}
```

- [ ] **Step 2: Run the acceptance tests**

Run: `just test-fast an_acceptance_`
Expected: PASS, 14 tests.

- [ ] **Step 3: Update the documentation**

In `README.md`, add a subsection to the claims documentation, `Relay identity (opt-in)`,
stating: the file is `~/.config/tasks/config.toml` (or `$XDG_CONFIG_HOME/tasks/config.toml`)
with `[identity] relay = true`; it is host-local, because relay availability is a property
of a machine and the per-project `tasks/.config.toml` syncs between hosts; in scope a claim
is keyed by the relay agent id `<harness>:<sessionId>` and carries that agent's process
proof; identity is adopted between sessions and never rewrites a held claim; the owner can park
and close a claim it already holds with the registry unavailable, while *resuming* a parked
task is a fresh acquisition and needs either the registry back or `TASKS_SESSION`; Linux
only; and `TASKS_SESSION` sits above the level and is the recovery path from every relay
identity error.

In `skills/tasks/SKILL.md`, extend the claims paragraph of the session protocol with three
sentences: under relay mode a claim is keyed by the relay agent id, the owner can still
park and close a claim it already holds while the registry is unavailable (resuming a
parked task needs the registry back, since parking releases the claim), and a relay
identity error is resolved by setting `TASKS_SESSION`/`TASKS_SESSION_PID`.

- [ ] **Step 4: Run the whole suite**

Run: `just gate`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/ README.md skills/tasks/SKILL.md
git commit -m "test(relay): acceptance tests for nesting, hints, registry loss and relay off"
```
