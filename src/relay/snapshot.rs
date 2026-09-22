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
        return Err(invalid(&format!(
            "start {value:?} is not canonical decimal"
        )));
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
        assert_eq!(
            snapshot.agents[0].process.as_ref().unwrap().platform,
            "darwin"
        );
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
        assert!(
            parse(&snapshot_json("codex:s1", &null_handle))
                .unwrap()
                .agents[0]
                .process
                .is_none()
        );
        let opencode = agent_json(&[
            ("id", "\"opencode:o1\""),
            ("harness", "\"opencode\""),
            ("sessionId", "\"o1\""),
            ("scope", "\"process\""),
        ]);
        assert_eq!(
            parse(&snapshot_json("opencode:o1", &opencode))
                .unwrap()
                .agents[0]
                .scope,
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
