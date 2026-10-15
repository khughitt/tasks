use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidSource {
    /// Matched through the caller's own `/proc` ancestry with the start time.
    Relay,
    /// `/proc/self/stat`'s session id, translated by the kernel into the procfs's namespace.
    UnixSession,
    /// `TASKS_SESSION_PID` or `CLAUDE_PID`: a number whose namespace this process cannot see.
    Exported,
}

/// Who holds a claim, and the pid that proves it is still alive. `session` is raw, for
/// matching against the store; `tagged` is the same session with the scheme of the level
/// that resolved it, for records that outlive the claim (a park entry, spec §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub session: String,
    pub pid: Option<u32>,
    pub tagged: String,
    pub proof: Option<Proof>,
    pub root_session: Option<String>,
    /// How `pid` was obtained, which decides whether a claim write can establish its PID
    /// view (claim PID-view design §2). `None` when there is no pid, or when the pid is
    /// carried from a claim rather than supplied by this write.
    pub pid_source: Option<PidSource>,
}

/// Process proof adopted from a matched relay handle, written into the claim's existing
/// fields. Absent on every native level, which derives what it can from `pid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub pid_start: u64,
    pub boot_id: String,
    pub host: String,
}

/// The caller's Unix session id, from `/proc/self/stat` field 6 (field 4 of the remainder
/// after the last `)`). Stable across commands from one terminal, and itself a pid.
pub fn unix_session_id() -> Option<u32> {
    let text = std::fs::read_to_string("/proc/self/stat").ok()?;
    let (_, rest) = text.rsplit_once(") ")?;
    rest.split_whitespace().nth(3)?.parse().ok()
}

pub fn identity(warnings: &mut Vec<String>) -> Result<Identity> {
    identity_from(
        |key| std::env::var_os(key),
        unix_session_id(),
        crate::relay::level,
        warnings,
    )
}

/// Session and pid resolve as a pair from a single level. A level that yields a session
/// but no usable pid yields `pid: None` and falls to the TTL path; it is never welded to
/// an unrelated fallback pid.
///
/// The Codex level validates both exports and selects the concrete `CODEX_THREAD_ID`
/// when present, otherwise the root `CODEX_SESSION_ID`. Malformed values refuse.
/// Codex runs each command as its own session leader, so the Unix fallback would die with
/// the command; the level exports no pid, so a Codex claim lives by the TTL.
pub fn identity_from(
    get: impl Fn(&str) -> Option<OsString>,
    session_pid: Option<u32>,
    relay: impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>>,
    _warnings: &mut Vec<String>,
) -> Result<Identity> {
    let var = |key: &str| {
        get(key)
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty())
    };
    let pid_of = |key: &str| var(key).and_then(|value| value.parse().ok());
    if let Some(session) = var("TASKS_SESSION") {
        let pid = pid_of("TASKS_SESSION_PID");
        return Ok(Identity {
            tagged: session.clone(),
            session,
            pid,
            proof: None,
            root_session: None,
            pid_source: pid.map(|_| PidSource::Exported),
        });
    }
    // Stage 1 of spec §7 is above: an explicit pair short-circuits, so the relay level is
    // never invoked and none of its stages run.
    if let Some(resolved) = relay()? {
        return Ok(Identity {
            tagged: resolved.session.clone(),
            session: resolved.session,
            pid: Some(resolved.pid),
            root_session: resolved.root_session,
            proof: Some(Proof {
                pid_start: resolved.pid_start,
                boot_id: resolved.boot_id,
                host: resolved.host,
            }),
            pid_source: Some(PidSource::Relay),
        });
    }
    if let Some(session) = var("CLAUDE_CODE_SESSION_ID") {
        let pid = pid_of("CLAUDE_PID");
        return Ok(Identity {
            tagged: format!("claude:{session}"),
            session,
            pid,
            proof: None,
            root_session: None,
            pid_source: pid.map(|_| PidSource::Exported),
        });
    }
    let invalid_native = |detail| {
        Error::Config(format!(
            "{detail}; correct the native variables or set TASKS_SESSION to this session's explicit identity (use the exact held session from tasks show when recovering an existing claim)"
        ))
    };
    let session = crate::native::value(&get, "CODEX_SESSION_ID").map_err(invalid_native)?;
    let thread = crate::native::value(&get, "CODEX_THREAD_ID").map_err(invalid_native)?;
    let root_session = session.as_ref().map(|id| format!("codex:{id}"));
    if let Some(session) = thread.or(session) {
        return Ok(Identity {
            tagged: format!("codex:{session}"),
            session,
            pid: None,
            proof: None,
            root_session,
            pid_source: None,
        });
    }
    match session_pid {
        Some(pid) => Ok(Identity {
            session: format!("sid:{pid}"),
            tagged: format!("sid:{pid}"),
            pid: Some(pid),
            proof: None,
            root_session: None,
            pid_source: Some(PidSource::UnixSession),
        }),
        None => Err(Error::Config(
            "cannot determine a session identity: set TASKS_SESSION (no \
             CLAUDE_CODE_SESSION_ID or CODEX_SESSION_ID, and /proc/self/stat is unreadable)"
                .into(),
        )),
    }
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

/// One session's advisory hold on a task. Lives outside git deliberately: a claim is
/// ephemeral machine state, and keeping it on a branch is what made `doing` invisible
/// across worktrees in the first place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub owner: String,
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_id: Option<String>,
    /// The PID view `pid` was established in, `"<dev>:<ino>"` (claim PID-view design §2).
    /// Absent when no write could establish it; such a pid is then never judged (§3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_view: Option<String>,
    pub host: String,
    pub worktree: String,
    pub started: String,
    pub seen: String,
    /// The exclusive needs this claim holds: the task's needs that its own project declares
    /// exclusive, computed on every acquire (lanes/needs design §4.4). Absent in entries
    /// written before holds existed, which therefore hold nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitingOn {
    User,
    Agent,
}

impl WaitingOn {
    /// Default first, the order the shared CLI table lists a value set in.
    pub const ALL: [WaitingOn; 2] = [WaitingOn::Agent, WaitingOn::User];

    pub fn parse(s: &str) -> Result<WaitingOn> {
        WaitingOn::ALL
            .into_iter()
            .find(|who| who.as_str() == s)
            .ok_or_else(|| {
                Error::Validation(format!("--waiting-on must be user or agent, got {s:?}"))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            WaitingOn::User => "user",
            WaitingOn::Agent => "agent",
        }
    }
}

/// Why the work stopped, from the eight-word vocabulary of the park designs.
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

    pub fn parse(s: &str) -> Result<Reason> {
        Reason::ALL
            .into_iter()
            .find(|reason| reason.as_str() == s)
            .ok_or_else(|| {
                let accepted: Vec<&str> =
                    Reason::ALL.iter().map(|reason| reason.as_str()).collect();
                Error::Validation(format!(
                    "--reason must be one of {}, got {s:?}",
                    accepted.join(", ")
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Review => "review",
            Reason::Decision => "decision",
            Reason::Approval => "approval",
            Reason::Environment => "environment",
            Reason::Dependency => "dependency",
            Reason::Session => "session",
            Reason::Capability => "capability",
            Reason::Quiet => "quiet",
        }
    }
}

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

/// A task set down with its next step: the store's other entry kind. No liveness, no
/// heartbeat, never pruned by a read; `start` replaces it, `done` and `drop` remove it.
/// See docs/specs/2026-09-09-park-design.md §4.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Park {
    pub owner: String,
    /// Scheme-tagged (`Identity::tagged`); opaque to tasks.
    pub session: String,
    pub host: String,
    pub worktree: String,
    pub at: String,
    pub next_step: String,
    pub waiting_on: WaitingOn,
    /// Why the work stopped, when the parking session said. Absent in store files written
    /// before the vocabulary existed and whenever `park` ran without `--reason`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<Needs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    /// Snapshot at park time, for rows whose task file is unreachable (spec §5.3).
    pub title: String,
}

/// A session under a cutoff found the task needs more reasoning than it could supply and
/// raised its rating. Shared outside git so every checkout's picker sees the new level
/// before the record merges; survives `start` and later parks; removed only by an explicit
/// `edit --complexity`/`--no-complexity` or by closing the task. See
/// docs/specs/2026-09-12-task-complexity-design.md §5.1.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Escalation {
    pub level: crate::model::Complexity,
    pub at: String,
    /// Scheme-tagged (`Identity::tagged`); opaque to tasks.
    pub session: String,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StoreFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adopted_from: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    claims: BTreeMap<String, Claim>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parks: BTreeMap<String, Park>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    escalations: BTreeMap<String, Escalation>,
}

#[derive(Debug)]
pub struct ClaimStore {
    path: PathBuf,
    adopted_from: Option<String>,
    claims: BTreeMap<String, Claim>,
    parks: BTreeMap<String, Park>,
    escalations: BTreeMap<String, Escalation>,
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
        Self::path_with(prefix, |key| std::env::var_os(key))
    }

    pub fn load(prefix: &str) -> Result<ClaimStore> {
        Self::load_from(&Self::path_for(prefix)?)
    }

    pub fn load_from(path: &Path) -> Result<ClaimStore> {
        let file = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str::<StoreFile>(&text)
                .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => StoreFile::default(),
            Err(error) => return Err(error.into()),
        };
        let StoreFile {
            adopted_from,
            claims,
            parks,
            escalations,
        } = file;
        for (id, claim) in &claims {
            if let Some(root) = &claim.root_session {
                match root.strip_prefix("codex:") {
                    Some(id) if !id.is_empty() && !id.chars().any(char::is_control) => {}
                    _ => {
                        return Err(Error::Config(format!(
                            "{}: claim {id} has an invalid root_session",
                            path.display()
                        )));
                    }
                }
            }
            for (field, value) in [("started", &claim.started), ("seen", &claim.seen)] {
                crate::time::parse(value).map_err(|error| {
                    Error::Config(format!(
                        "{}: claim {id} has an unreadable {field}: {error}",
                        path.display()
                    ))
                })?;
            }
        }
        for (id, park) in &parks {
            crate::time::parse(&park.at).map_err(|error| {
                Error::Config(format!(
                    "{}: park {id} has an unreadable at: {error}",
                    path.display()
                ))
            })?;
            crate::format::validate_line("next_step", &park.next_step).map_err(|error| {
                Error::Config(format!("{}: park {id}: {error}", path.display()))
            })?;
            if claims.contains_key(id) {
                return Err(Error::Config(format!(
                    "{}: {id} has both a claim and a park; one entry per task",
                    path.display()
                )));
            }
        }
        for (id, escalation) in &escalations {
            crate::time::parse(&escalation.at).map_err(|error| {
                Error::Config(format!(
                    "{}: escalation {id} has an unreadable at: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(ClaimStore {
            path: path.to_path_buf(),
            adopted_from,
            claims,
            parks,
            escalations,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = StoreFile {
            adopted_from: self.adopted_from.clone(),
            claims: self.claims.clone(),
            parks: self.parks.clone(),
            escalations: self.escalations.clone(),
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

    /// A claim displaces a park on the same id: one entry per task (spec §4).
    pub fn insert(&mut self, id: &TaskId, claim: Claim) {
        let key = id.to_string();
        self.parks.remove(&key);
        self.claims.insert(key, claim);
    }

    /// A park displaces a claim on the same id.
    pub fn insert_park(&mut self, id: &TaskId, park: Park) {
        let key = id.to_string();
        self.claims.remove(&key);
        self.parks.insert(key, park);
    }

    pub fn park(&self, id: &TaskId) -> Option<&Park> {
        self.parks.get(&id.to_string())
    }

    pub fn remove(&mut self, id: &TaskId) -> Option<Claim> {
        self.claims.remove(&id.to_string())
    }

    pub fn remove_park(&mut self, id: &TaskId) -> Option<Park> {
        self.parks.remove(&id.to_string())
    }

    pub fn escalation(&self, id: &TaskId) -> Option<&Escalation> {
        self.escalations.get(&id.to_string())
    }

    /// Replaces any earlier entry; the caller has already checked the level never falls.
    pub fn insert_escalation(&mut self, id: &TaskId, escalation: Escalation) {
        self.escalations.insert(id.to_string(), escalation);
    }

    pub fn remove_escalation(&mut self, id: &TaskId) -> Option<Escalation> {
        self.escalations.remove(&id.to_string())
    }

    pub fn escalations(&self) -> impl Iterator<Item = (&String, &Escalation)> {
        self.escalations.iter()
    }

    /// Nothing `rename` would need to carry: no parks and no escalations. Claims are
    /// never carried; a rename refuses while any are live.
    pub fn carries_nothing(&self) -> bool {
        self.parks.is_empty() && self.escalations.is_empty()
    }

    pub fn prune_with(&mut self, keep: impl Fn(&Claim) -> bool) {
        self.claims.retain(|_, claim| keep(claim));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Claim)> {
        self.claims.iter()
    }

    pub fn parks(&self) -> impl Iterator<Item = (&String, &Park)> {
        self.parks.iter()
    }

    /// The store `rename` writes for the target prefix: this store's park and escalation
    /// entries with their ids re-prefixed, and no claims. Pure; nothing on disk changes.
    pub fn carried_renamed_text(&self, source: &str, target: &str) -> Result<String> {
        let rekey = |key: &String, kind: &str| -> Result<String> {
            let id = TaskId::parse(key)?;
            if id.prefix != source {
                return Err(Error::Config(format!(
                    "{kind} entry {key} does not belong to prefix {source:?}"
                )));
            }
            Ok(format!("{target}-{}", id.hex))
        };
        let mut parks = BTreeMap::new();
        for (key, park) in &self.parks {
            parks.insert(rekey(key, "park")?, park.clone());
        }
        let mut escalations = BTreeMap::new();
        for (key, escalation) in &self.escalations {
            escalations.insert(rekey(key, "escalation")?, escalation.clone());
        }
        Ok(toml::to_string(&StoreFile {
            adopted_from: None,
            claims: BTreeMap::new(),
            parks,
            escalations,
        })
        .expect("claim store serializes"))
    }

    /// A target store may already have consumed carried parks while the registry is old.
    /// Keep its claims, and write only when no carried park or escalation is present yet.
    pub fn adoption_target_text(
        &self,
        source: &str,
        target: &str,
        destination: &ClaimStore,
    ) -> Result<Option<String>> {
        if let Some(previous) = &destination.adopted_from {
            if previous == source {
                return Ok(None);
            }
            return Err(Error::Validation(format!(
                "target store was adopted from {previous:?}, not {source:?}"
            )));
        }
        let mut carried: StoreFile = toml::from_str(&self.carried_renamed_text(source, target)?)
            .expect("carried store serializes");
        for (id, park) in &destination.parks {
            if carried.parks.get(id) != Some(park) {
                return Err(Error::Validation(format!(
                    "target park {id} conflicts with adoption"
                )));
            }
        }
        for (id, escalation) in &destination.escalations {
            if carried.escalations.get(id) != Some(escalation) {
                return Err(Error::Validation(format!(
                    "target escalation {id} conflicts with adoption"
                )));
            }
        }
        if !destination.parks.is_empty() || !destination.escalations.is_empty() {
            return Ok(None);
        }
        if carried.parks.is_empty() && carried.escalations.is_empty() {
            return Ok(None);
        }
        if let Some(id) = carried
            .parks
            .keys()
            .find(|id| destination.claims.contains_key(*id))
        {
            return Err(Error::Validation(format!(
                "target claim {id} conflicts with carried park"
            )));
        }
        carried.claims = destination.claims.clone();
        carried.adopted_from = Some(source.into());
        Ok(Some(
            toml::to_string(&carried).expect("claim store serializes"),
        ))
    }

    pub fn prune_dead(&mut self) {
        self.prune_with(|claim| !liveness(claim).is_stale());
    }
}

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

/// Hours a claim whose liveness cannot be established stays live. It applies *only* on the
/// unverifiable path: a confirmed-live process outlives it, and a confirmed-dead one gets
/// no grace at all.
pub const TTL_HOURS: i64 = 4;

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

/// Every claim in scope, with its liveness verdict, and every park entry, read once per
/// command.
#[derive(Debug, Default)]
pub struct ClaimSnapshot {
    by_id: BTreeMap<String, (Claim, Liveness)>,
    parks: BTreeMap<String, Park>,
    escalations: BTreeMap<String, Escalation>,
}

impl ClaimSnapshot {
    pub fn load<'a>(prefixes: impl Iterator<Item = &'a str>) -> Result<ClaimSnapshot> {
        let paths = prefixes
            .map(ClaimStore::path_for)
            .collect::<Result<Vec<_>>>()?;
        Self::load_from_paths(paths.into_iter())
    }

    /// The path form, so a test can point at a file without setting the environment.
    pub fn load_from_paths(paths: impl Iterator<Item = PathBuf>) -> Result<ClaimSnapshot> {
        let mut by_id = BTreeMap::new();
        let mut parks = BTreeMap::new();
        let mut escalations = BTreeMap::new();
        for path in paths {
            let store = ClaimStore::load_from(&path)?;
            for (id, claim) in store.iter() {
                by_id.insert(id.clone(), (claim.clone(), liveness(claim)));
            }
            for (id, park) in store.parks() {
                parks.insert(id.clone(), park.clone());
            }
            for (id, escalation) in store.escalations() {
                escalations.insert(id.clone(), escalation.clone());
            }
        }
        Ok(ClaimSnapshot {
            by_id,
            parks,
            escalations,
        })
    }

    /// The literal form, for tests that need a snapshot without files.
    #[cfg(test)]
    pub fn from_parts(
        by_id: BTreeMap<String, (Claim, Liveness)>,
        parks: BTreeMap<String, Park>,
        escalations: BTreeMap<String, Escalation>,
    ) -> ClaimSnapshot {
        ClaimSnapshot {
            by_id,
            parks,
            escalations,
        }
    }

    pub fn park(&self, id: &TaskId) -> Option<&Park> {
        self.parks.get(&id.to_string())
    }

    pub fn parks(&self) -> impl Iterator<Item = (&String, &Park)> {
        self.parks.iter()
    }

    pub fn escalation(&self, id: &TaskId) -> Option<&Escalation> {
        self.escalations.get(&id.to_string())
    }

    pub fn get(&self, id: &TaskId) -> Option<&(Claim, Liveness)> {
        self.by_id.get(&id.to_string())
    }

    /// The claim on `id` unless it is stale: `Live` and `Unverifiable` both hold (§4).
    pub fn held(&self, id: &TaskId) -> Option<&Claim> {
        match self.get(id) {
            Some((claim, liveness)) if !liveness.is_stale() => Some(claim),
            _ => None,
        }
    }

    /// Every claim with its liveness verdict, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &(Claim, Liveness))> {
        self.by_id.iter()
    }

    pub fn stale(&self) -> impl Iterator<Item = (&String, &Claim, &String)> {
        self.by_id
            .iter()
            .filter_map(|(id, (claim, live))| match live {
                Liveness::Stale(why) => Some((id, claim, why)),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcStat {
    NotFound,
    /// The process may exist; we could not tell. Not evidence of death.
    Unreadable,
    Found {
        state: char,
        starttime: u64,
    },
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
/// the recorded evidence unless it reports a different pid (§2's table), or that evidence
/// already reads stale from this view: keeping a dead pid would write the owner's own claim
/// stale, free for anyone to take (final review, Important 1).
pub fn evidence(
    me: &Identity,
    own: Option<&Claim>,
    view: Option<&crate::pidview::PidView>,
    boot_id: Option<String>,
    now: time::OffsetDateTime,
    stat: impl Fn(u32) -> ProcStat,
) -> Evidence {
    let boot_id_now = boot_id.clone();
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
        Some(own)
            if fresh.pid_view.is_none()
                && (me.pid.is_none() || me.pid == own.pid)
                && !liveness_with(own, now, boot_id_now.as_deref(), view, &stat).is_stale() =>
        {
            Evidence::of(own)
        }
        _ => fresh,
    }
}

pub fn liveness(claim: &Claim) -> Liveness {
    liveness_with(
        claim,
        time::OffsetDateTime::now_utc(),
        boot_id().as_deref(),
        crate::pidview::current().as_ref(),
        proc_stat,
    )
}

pub fn liveness_with(
    claim: &Claim,
    now: time::OffsetDateTime,
    boot_id: Option<&str>,
    view: Option<&crate::pidview::PidView>,
    stat: impl Fn(u32) -> ProcStat,
) -> Liveness {
    let ttl = || match crate::time::parse(&claim.seen) {
        Ok(seen) if now - seen <= time::Duration::hours(TTL_HOURS) => Liveness::Live,
        Ok(seen) => Liveness::Stale(format!(
            "not seen for {}h",
            (now - seen).whole_hours().max(0)
        )),
        Err(_) => Liveness::Live,
    };

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
    match stat(pid) {
        ProcStat::NotFound => Liveness::Stale(format!("pid {pid} is gone")),
        ProcStat::Unreadable => ttl(),
        ProcStat::Found { state: 'Z', .. } => Liveness::Stale(format!("pid {pid} is a zombie")),
        ProcStat::Found { starttime, .. } => match claim.pid_start {
            Some(recorded) if recorded != starttime => {
                Liveness::Stale(format!("pid {pid} was reused by another process"))
            }
            Some(_) => Liveness::Live,
            None => ttl(),
        },
    }
}

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
        root_session: claim.root_session.clone(),
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
        pid_source: None,
    }
}

/// Direct a held-claim resolution error to explicit owner recovery without exposing IDs.
/// A stored session may equal the other native input in a malformed-value diagnostic.
pub fn name_the_claim(error: Error, held: Option<&str>) -> Error {
    match held {
        Some(_) => Error::Config(format!(
            "{error}; inspect tasks show for the existing claim's exact session identity"
        )),
        None => error,
    }
}

/// Spec §6.2: does this caller own `claim`, proved from the claim's own recorded process
/// handle and the caller's ancestry, with no registry read?
pub fn proves_ownership(
    claim: &Claim,
    scope: &crate::relay::ancestry::Scope,
    host: &str,
    boot_id: Option<&str>,
    get: &impl Fn(&str) -> Option<OsString>,
) -> bool {
    let crate::relay::ancestry::Scope::Harness(boundary) = scope else {
        return false;
    };
    let nearest = &boundary.entry;
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
    if boundary.harness == "codex" {
        if crate::relay::resolve::hint_for("codex", get).is_err() {
            return false;
        }
        return match crate::native::value(get, "CODEX_THREAD_ID") {
            Ok(Some(thread)) => {
                crate::relay::resolve::same_session(&claim.session, "codex", &thread)
            }
            _ => false,
        };
    }
    // 3. a scoped hint must not contradict the claim, so that deferring a resolution error
    //    cannot turn a contradicted session into an accepted owner.
    match crate::relay::resolve::hint_for(boundary.harness, get) {
        Ok(Some(hint)) => {
            crate::relay::resolve::same_session(&claim.session, boundary.harness, &hint)
        }
        Ok(None) => true,
        Err(_) => false,
    }
}

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
            pid_view: Some(INIT.to_string()),
            host: "h".into(),
            worktree: "/tmp/wt".into(),
            started: "2026-09-05T08:00:00Z".into(),
            seen: "2026-09-05T08:00:00Z".into(),
            holds: Vec::new(),
            root_session: None,
        }
    }

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

    use crate::relay::ancestry::{Boundary, HARNESS_COMMS, ProcEntry, Scope};

    const PROOF_BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";

    fn claim_of(session: &str, pid: Option<u32>, pid_start: Option<u64>) -> Claim {
        Claim {
            owner: "tester".into(),
            session: session.into(),
            pid,
            pid_start,
            boot_id: Some(PROOF_BOOT.into()),
            pid_view: None,
            host: "testhost".into(),
            worktree: "/w".into(),
            started: "2026-09-22T00:00:00Z".into(),
            seen: "2026-09-22T00:00:00Z".into(),
            holds: Vec::new(),
            root_session: None,
        }
    }

    fn boundary(comm: &str, harness: &'static str, pid: u32, start: u64) -> Boundary {
        Boundary {
            entry: ProcEntry {
                pid,
                ppid: 1,
                comm: comm.into(),
                start,
            },
            harness,
        }
    }

    fn nearest(comm: &str, pid: u32, start: u64) -> Scope {
        let &(_, harness) = HARNESS_COMMS.iter().find(|(c, _)| *c == comm).unwrap();
        Scope::Harness(boundary(comm, harness, pid, start))
    }

    fn no_env() -> impl Fn(&str) -> Option<OsString> {
        |_| None
    }

    #[test]
    fn delegated_relay_proof_requires_a_concrete_matching_thread() {
        let claim = claim_of("codex:worker", Some(42), Some(900));
        for (vars, expected) in [
            (
                vec![("CODEX_SESSION_ID", "root"), ("CODEX_THREAD_ID", "worker")],
                true,
            ),
            (vec![("CODEX_THREAD_ID", "sibling")], false),
            (vec![("CODEX_SESSION_ID", "worker")], false),
            (vec![], false),
            (
                vec![
                    ("CODEX_SESSION_ID", "private\nroot"),
                    ("CODEX_THREAD_ID", "worker"),
                ],
                false,
            ),
        ] {
            let get = |key: &str| {
                vars.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| OsString::from(v))
            };
            assert_eq!(
                proves_ownership(
                    &claim,
                    &nearest("codex", 42, 900),
                    "testhost",
                    Some(PROOF_BOOT),
                    &get
                ),
                expected,
                "{vars:?}"
            );
        }
        assert!(proves_ownership(
            &claim_of("claude-code:owner", Some(42), Some(900)),
            &nearest("claude", 42, 900),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_establishes_the_owner_from_the_claims_own_contents() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some(PROOF_BOOT),
            &env_of(&[("CODEX_THREAD_ID", "s1")])
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
            &env_of(&[("CODEX_THREAD_ID", "claude-code:c1")])
        ));
    }

    #[test]
    fn a_proof_is_defeated_by_a_contradicted_hint() {
        let claim = claim_of("claude-code:c1", Some(42), Some(900));
        let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "other".into());
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
            let env = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "c1".into());
            assert!(
                proves_ownership(
                    &claim,
                    &nearest("claude", 42, 900),
                    "testhost",
                    Some(PROOF_BOOT),
                    &env
                ),
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
            &env_of(&[("CODEX_THREAD_ID", "s1")])
        ));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "testhost",
            Some("22222222-2222-4222-8222-222222222222"),
            &env_of(&[("CODEX_THREAD_ID", "s1")])
        ));
        assert!(!proves_ownership(
            &claim,
            &nearest("codex", 42, 900),
            "elsewhere",
            Some(PROOF_BOOT),
            &env_of(&[("CODEX_THREAD_ID", "s1")])
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
            &env_of(&[("CODEX_THREAD_ID", "s1")])
        ));
    }

    #[test]
    fn a_proof_needs_an_established_harness_boundary() {
        let claim = claim_of("codex:s1", Some(42), Some(900));
        assert!(!proves_ownership(
            &claim,
            &Scope::Outside,
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
        assert!(!proves_ownership(
            &claim,
            &Scope::Unknown {
                pid: 4,
                file: "stat"
            },
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_is_never_established_by_a_hosting_process() {
        let claim = claim_of("claude-code:c1", Some(300), Some(3000));
        assert!(!proves_ownership(
            &claim,
            &Scope::Hosting(boundary("claude", "claude-code", 300, 3000), "daemon"),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_at_a_version_boundary_is_its_own_not_the_outer_sessions() {
        // The outer claude(500) holds the claim; a nested 2.1.280(600) runs under it, with
        // the outer session's id inherited. Its boundary is 600, so it proves nothing.
        let outer = claim_of("claude-code:c1", Some(500), Some(5000));
        let inherited = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "c1".into());
        let nested = Scope::Harness(boundary("2.1.280", "claude-code", 600, 6000));
        assert!(!proves_ownership(
            &outer,
            &nested,
            "testhost",
            Some(PROOF_BOOT),
            &inherited
        ));
        // A claim that the version process itself holds is proved, and its hint is the
        // Claude Code variable.
        let own = claim_of("claude-code:bg1", Some(600), Some(6000));
        let hint = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "bg1".into());
        assert!(proves_ownership(
            &own,
            &nested,
            "testhost",
            Some(PROOF_BOOT),
            &hint
        ));
        let contradicted = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "other".into());
        assert!(!proves_ownership(
            &own,
            &nested,
            "testhost",
            Some(PROOF_BOOT),
            &contradicted
        ));
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
        assert!(
            continuation_identity(&claim_of("codex:s1", None, None))
                .proof
                .is_none()
        );
    }

    fn relay_off() -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
        || Ok(None)
    }

    fn relay_resolves(
        session: &str,
    ) -> impl FnOnce() -> Result<Option<crate::relay::resolve::Resolved>> {
        let session = session.to_string();
        move || {
            Ok(Some(crate::relay::resolve::Resolved {
                session,
                root_session: None,
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
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(id.session, "explicit");
        assert_eq!(id.pid, Some(7));
    }

    #[test]
    fn a_codex_session_is_a_level_below_claude_and_above_the_unix_fallback() {
        let mut warnings = Vec::new();
        let codex = identity_from(
            env_of(&[
                ("CODEX_SESSION_ID", "thread-1"),
                ("CODEX_THREAD_ID", "thread-1"),
            ]),
            Some(11),
            relay_off(),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(codex.session, "thread-1", "the raw session matches claims");
        assert_eq!(
            codex.tagged, "codex:thread-1",
            "the provenance key's scheme"
        );
        assert_eq!(
            codex.pid, None,
            "Codex exports no pid: the claim lives by the TTL, not by the command's session leader"
        );
        assert!(warnings.is_empty(), "{warnings:?}");

        let thread_only = identity_from(
            env_of(&[("CODEX_THREAD_ID", "thread-2")]),
            Some(11),
            relay_off(),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(thread_only.tagged, "codex:thread-2");

        let claude = identity_from(
            env_of(&[
                ("CLAUDE_CODE_SESSION_ID", "claude"),
                ("CODEX_SESSION_ID", "thread-1"),
            ]),
            Some(11),
            relay_off(),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(
            claude.tagged, "claude:claude",
            "the Claude level is read first"
        );
    }

    #[test]
    fn delegated_native_identity_is_the_thread() {
        let mut warnings = Vec::new();
        let id = identity_from(
            env_of(&[("CODEX_SESSION_ID", "root"), ("CODEX_THREAD_ID", "worker")]),
            Some(11),
            relay_off(),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(id.session, "worker");
        assert_eq!(id.tagged, "codex:worker");
        assert_eq!(id.pid, None);
        assert!(id.proof.is_none());
        assert!(warnings.is_empty());
    }

    #[test]
    fn delegated_native_values_are_validated_without_leaking() {
        for name in ["CODEX_SESSION_ID", "CODEX_THREAD_ID"] {
            for bad in ["private\nvalue", "private\tvalue", "private\u{7f}value"] {
                let get = |key: &str| match key {
                    k if k == name => Some(OsString::from(bad)),
                    "CODEX_THREAD_ID" => Some("worker".into()),
                    _ => None,
                };
                let error = identity_from(get, Some(11), relay_off(), &mut Vec::new())
                    .unwrap_err()
                    .to_string();
                assert!(
                    error.contains(name) && error.contains("TASKS_SESSION"),
                    "{error}"
                );
                assert!(!error.contains("private"), "{error}");
                let overridden = |key: &str| {
                    if key == "TASKS_SESSION" {
                        Some("owner".into())
                    } else {
                        get(key)
                    }
                };
                assert_eq!(
                    identity_from(overridden, Some(11), relay_off(), &mut Vec::new())
                        .unwrap()
                        .session,
                    "owner"
                );
                let claude = |key: &str| {
                    if key == "CLAUDE_CODE_SESSION_ID" {
                        Some("claude".into())
                    } else {
                        get(key)
                    }
                };
                assert_eq!(
                    identity_from(claude, Some(11), relay_off(), &mut Vec::new())
                        .unwrap()
                        .session,
                    "claude"
                );
            }
        }
        for vars in [
            vec![],
            vec![("CODEX_SESSION_ID", ""), ("CODEX_THREAD_ID", "")],
        ] {
            assert_eq!(
                identity_from(env_of(&vars), Some(11), relay_off(), &mut Vec::new())
                    .unwrap()
                    .session,
                "sid:11"
            );
        }
        assert_eq!(
            identity_from(
                env_of(&[("CODEX_SESSION_ID", "root")]),
                Some(11),
                relay_off(),
                &mut Vec::new()
            )
            .unwrap()
            .session,
            "root"
        );
    }

    #[cfg(unix)]
    #[test]
    fn delegated_native_non_unicode_refuses() {
        use std::os::unix::ffi::OsStringExt;
        for name in ["CODEX_SESSION_ID", "CODEX_THREAD_ID"] {
            let error = identity_from(
                |key| {
                    if key == name {
                        Some(OsString::from_vec(vec![255]))
                    } else if key == "CODEX_THREAD_ID" {
                        Some("worker".into())
                    } else {
                        None
                    }
                },
                Some(11),
                relay_off(),
                &mut Vec::new(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(name)
                    && error.contains("not Unicode")
                    && error.contains("TASKS_SESSION")
            );
        }
    }

    #[test]
    fn a_level_never_borrows_another_levels_pid() {
        let id = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "claude")]),
            Some(11),
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(id.session, "claude");
        assert_eq!(
            id.pid, None,
            "a session without its own pid must not be welded to the fallback pid"
        );
    }

    #[test]
    fn falls_back_to_the_unix_session_id() {
        let id = identity_from(env_of(&[]), Some(11), relay_off(), &mut Vec::new()).unwrap();
        assert_eq!(id.session, "sid:11");
        assert_eq!(id.pid, Some(11));
    }

    #[test]
    fn an_empty_variable_does_not_count_as_set() {
        let id = identity_from(
            env_of(&[("TASKS_SESSION", "")]),
            Some(11),
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            id.session, "sid:11",
            "emptiness is filtered inside the helper"
        );
    }

    #[test]
    fn unresolvable_identity_is_an_error_not_a_shared_placeholder() {
        let error = identity_from(env_of(&[]), None, relay_off(), &mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(
            error.to_string().contains("TASKS_SESSION"),
            "the message names the way out: {error}"
        );
    }

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

    #[test]
    fn delegated_root_store_roundtrip_and_validation() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();
        for root in [None, Some("codex:root"), Some("codex:codex:x")] {
            let mut store = ClaimStore::load_from(&path).unwrap();
            let mut claim = sample();
            claim.root_session = root.map(String::from);
            store.insert(&id, claim.clone());
            store.save().unwrap();
            let loaded = ClaimStore::load_from(&path).unwrap();
            assert_eq!(loaded.get(&id), Some(&claim));
            assert_eq!(
                continuation_identity(loaded.get(&id).unwrap()).root_session,
                root.map(String::from)
            );
            if root.is_none() {
                assert!(
                    !std::fs::read_to_string(&path)
                        .unwrap()
                        .contains("root_session")
                );
            }
        }
        for root in [
            "codex:",
            "claude:private-root",
            "codex:private\nroot",
            "codex:private\u{7f}root",
        ] {
            let mut store = ClaimStore::load_from(&path).unwrap();
            let mut claim = sample();
            claim.root_session = Some(root.into());
            store.insert(&id, claim);
            store.save().unwrap();
            let error = ClaimStore::load_from(&store.path).unwrap_err().to_string();
            assert!(
                error.contains("root_session") && error.contains(&id.to_string()),
                "{error}"
            );
            assert!(!error.contains("private"), "{error}");
            std::fs::remove_file(&store.path).unwrap();
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
    fn an_unreadable_store_parent_is_an_io_error() {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::tempdir().unwrap();
        let parent = home.path().join("claims");
        let path = parent.join("sci.toml");
        std::fs::create_dir(&parent).unwrap();
        std::fs::write(&path, "claims = {}").unwrap();
        let original = std::fs::metadata(&parent).unwrap().permissions();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = ClaimStore::load_from(&path);
        std::fs::set_permissions(&parent, original).unwrap();

        assert_eq!(result.unwrap_err().kind(), "io");
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
        store.insert(
            &go,
            Claim {
                pid: Some(43),
                ..sample()
            },
        );
        store.prune_with(|claim| claim.pid == Some(42));
        assert!(store.get(&keep).is_some());
        assert!(store.get(&go).is_none());
    }

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
                Some(&INIT),
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
                Some(&INIT),
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
                Some(&INIT),
                found(12345)
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn a_missing_recorded_start_time_takes_the_ttl_path_not_the_reuse_path() {
        let no_evidence = Claim {
            pid_start: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        assert_eq!(
            liveness_with(
                &no_evidence,
                at("2026-09-05T01:00:00Z"),
                Some("boot-a"),
                Some(&INIT),
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
                    Some(&INIT),
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
                    Some(&INIT),
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
                Some(&INIT),
                |_| ProcStat::Found {
                    state: 'Z',
                    starttime: 999
                }
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
                Some(&INIT),
                |_| ProcStat::Unreadable
            ),
            Liveness::Live,
            "a permission failure is not proof of death"
        );
        assert_eq!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T04:00:00Z"),
                Some("boot-a"),
                Some(&INIT),
                |_| ProcStat::Unreadable
            ),
            Liveness::Live,
            "exactly four hours is still inside the TTL"
        );
        assert!(matches!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T04:00:01Z"),
                Some("boot-a"),
                Some(&INIT),
                |_| ProcStat::Unreadable
            ),
            Liveness::Stale(_)
        ));
    }

    #[test]
    fn an_unreadable_boot_id_or_a_pidless_claim_falls_to_ttl() {
        assert_eq!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T01:00:00Z"),
                None,
                Some(&INIT),
                found(999)
            ),
            Liveness::Live
        );
        let pidless = Claim {
            pid: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        assert!(matches!(
            liveness_with(
                &pidless,
                at("2026-09-05T05:00:00Z"),
                Some("boot-a"),
                Some(&INIT),
                found(999)
            ),
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
            parse_proc_stat(
                "7 (weird ) name) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 4242 x"
            ),
            ProcStat::Found {
                state: 'S',
                starttime: 4242
            },
            "fields are counted from after the LAST ')'"
        );
        assert_eq!(parse_proc_stat("garbage"), ProcStat::Unreadable);
    }

    const A_CLAIM: &str = "[claims.\"sci-000001\"]\nowner = \"o\"\nsession = \"a\"\nhost = \"h\"\n\
        worktree = \"/w\"\nstarted = \"2026-09-09T20:00:00Z\"\nseen = \"2026-09-09T20:00:00Z\"\n";
    const A_PARK: &str = "[parks.\"sci-000002\"]\nowner = \"o\"\nsession = \"claude:x\"\nhost = \"h\"\n\
        worktree = \"/w\"\nat = \"2026-09-09T21:00:00Z\"\nnext_step = \"write §3\"\n\
        waiting_on = \"agent\"\ntitle = \"T\"\n";

    fn store_from(text: &str) -> (tempfile::TempDir, ClaimStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        std::fs::write(&path, text).unwrap();
        let store = ClaimStore::load_from(&path).unwrap();
        (dir, store)
    }

    fn sample_claim() -> Claim {
        Claim {
            owner: "o".into(),
            session: "a".into(),
            pid: None,
            pid_start: None,
            boot_id: None,
            pid_view: None,
            host: "h".into(),
            worktree: "/w".into(),
            started: "2026-09-09T20:00:00Z".into(),
            seen: "2026-09-09T20:00:00Z".into(),
            holds: Vec::new(),
            root_session: None,
        }
    }

    fn sample_park() -> Park {
        Park {
            owner: "o".into(),
            session: "claude:x".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-09-09T21:00:00Z".into(),
            next_step: "write §3".into(),
            waiting_on: WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "T".into(),
        }
    }

    #[test]
    fn identity_tags_the_session_by_the_level_that_resolved_it() {
        let explicit = identity_from(
            env_of(&[("TASKS_SESSION", "mine:7")]),
            Some(11),
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(explicit.tagged, "mine:7", "verbatim; the caller tags it");
        let claude = identity_from(
            env_of(&[("CLAUDE_CODE_SESSION_ID", "abc-123")]),
            Some(11),
            relay_off(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            claude.session, "abc-123",
            "the raw session still matches claims"
        );
        assert_eq!(claude.tagged, "claude:abc-123");
        let unix = identity_from(env_of(&[]), Some(11), relay_off(), &mut Vec::new()).unwrap();
        assert_eq!(unix.tagged, "sid:11");
    }

    #[test]
    fn waiting_on_parses_its_two_values_only() {
        assert_eq!(WaitingOn::parse("user").unwrap(), WaitingOn::User);
        assert_eq!(WaitingOn::parse("agent").unwrap(), WaitingOn::Agent);
        assert!(matches!(
            WaitingOn::parse("nobody"),
            Err(Error::Validation(_))
        ));
    }

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
            describe_stop(
                WaitingOn::User,
                Some(Reason::Quiet),
                Some(Needs::Headless),
                None
            ),
            "user, quiet; headless"
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
        assert_eq!(store.park(&two).unwrap().needs, None);
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
        assert_eq!(park.reason, None);
        assert_eq!(park.needs, Some(Needs::Idle));
        assert_eq!(park.minutes, Some(20));
    }

    #[test]
    fn a_park_without_a_reason_loads_and_a_park_with_one_writes_the_key() {
        let (dir, mut store) = store_from(A_PARK);
        let two = TaskId::parse("sci-000002").unwrap();
        assert_eq!(
            store.park(&two).unwrap().reason,
            None,
            "pre-reason store files load"
        );

        let three = TaskId::parse("sci-000003").unwrap();
        store.insert_park(
            &three,
            Park {
                owner: "o".into(),
                session: "a".into(),
                host: "h".into(),
                worktree: "/w".into(),
                at: "2026-09-11T10:00:00Z".into(),
                next_step: "open the sheet".into(),
                waiting_on: WaitingOn::User,
                reason: Some(Reason::Review),
                needs: None,
                minutes: None,
                title: "T".into(),
            },
        );
        store.save().unwrap();
        let path = dir.path().join("sci.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("reason = \"review\""), "{text}");
        assert_eq!(
            text.matches("reason =").count(),
            1,
            "the reason-less park writes no key: {text}"
        );
        let reloaded = ClaimStore::load_from(&path).unwrap();
        assert_eq!(reloaded.park(&three).unwrap().reason, Some(Reason::Review));
    }

    #[test]
    fn inserting_a_park_displaces_a_claim_on_the_same_id() {
        let (_dir, mut store) = store_from(A_CLAIM);
        let id = TaskId::parse("sci-000001").unwrap();
        store.insert_park(
            &id,
            Park {
                owner: "o".into(),
                session: "a".into(),
                host: "h".into(),
                worktree: "/w".into(),
                at: "2026-09-09T23:00:00Z".into(),
                next_step: "x".into(),
                waiting_on: WaitingOn::User,
                reason: None,
                needs: None,
                minutes: None,
                title: "T".into(),
            },
        );
        assert!(store.get(&id).is_none(), "one entry per task");
        assert_eq!(store.parks().count(), 1);
    }

    #[test]
    fn park_and_remove_park_read_and_clear_one_entry() {
        let (_dir, mut store) = store_from(A_PARK);
        let id = TaskId::parse("sci-000002").unwrap();
        assert_eq!(
            store.park(&id).map(|p| p.next_step.as_str()),
            Some("write §3")
        );
        assert!(store.remove_park(&id).is_some());
        assert!(store.park(&id).is_none());
        assert!(store.remove_park(&id).is_none());
    }

    #[test]
    fn escalations_round_trip_and_are_independent_of_claims_and_parks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        let id = TaskId::parse("sci-4f2a9c").unwrap();
        let mut store = ClaimStore::load_from(&path).unwrap();
        assert!(store.carries_nothing());
        store.insert_escalation(
            &id,
            Escalation {
                level: crate::model::Complexity::High,
                at: "2026-09-12T10:00:00Z".into(),
                session: "s:agent-a".into(),
            },
        );
        assert!(!store.carries_nothing());
        store.save().unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[escalations.sci-4f2a9c]"), "{text}");
        assert!(text.contains("level = \"high\""), "{text}");

        let mut store = ClaimStore::load_from(&path).unwrap();
        assert_eq!(
            store.escalation(&id).map(|e| e.level),
            Some(crate::model::Complexity::High)
        );
        // A claim on the same id leaves the escalation alone; so does a park.
        store.insert(&id, sample_claim());
        assert!(store.escalation(&id).is_some());
        store.remove(&id);
        store.insert_park(&id, sample_park());
        assert!(store.escalation(&id).is_some());
        assert!(store.remove_escalation(&id).is_some());
        assert!(store.remove_escalation(&id).is_none());

        let bad =
            "[escalations.sci-4f2a9c]\nlevel = \"high\"\nat = \"yesterday\"\nsession = \"s\"\n";
        std::fs::write(&path, bad).unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err().to_string();
        assert!(
            error.contains("escalation sci-4f2a9c has an unreadable at"),
            "{error}"
        );
    }

    #[test]
    fn a_store_loads_and_saves_claims_and_parks_side_by_side() {
        let (dir, store) = store_from(&format!("{A_CLAIM}{A_PARK}"));
        let claimed = TaskId::parse("sci-000001").unwrap();
        let parked = TaskId::parse("sci-000002").unwrap();
        assert_eq!(store.get(&claimed).map(|c| c.session.as_str()), Some("a"));
        let park = store
            .parks()
            .find(|(id, _)| *id == "sci-000002")
            .map(|(_, park)| park)
            .unwrap();
        assert_eq!(park.next_step, "write §3");
        assert_eq!(park.waiting_on, WaitingOn::Agent);
        assert_eq!(park.title, "T");
        assert!(store.get(&parked).is_none(), "a park is not a claim");

        store.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join("sci.toml")).unwrap();
        assert!(text.contains("[parks.sci-000002]"), "{text}");
        assert!(text.contains("[claims.sci-000001]"), "{text}");
        let reloaded = ClaimStore::load_from(&dir.path().join("sci.toml")).unwrap();
        assert_eq!(reloaded.parks().count(), 1);
        assert_eq!(reloaded.iter().count(), 1);
    }

    #[test]
    fn an_empty_kind_is_absent_from_the_file() {
        let (dir, store) = store_from(A_PARK);
        store.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join("sci.toml")).unwrap();
        assert!(
            !text.contains("claims"),
            "an empty claims map must not write [claims]: {text}"
        );
    }

    #[test]
    fn carried_renamed_text_re_prefixes_ids_and_drops_claims() {
        let (_dir, store) = store_from(&format!(
            "{}{}",
            A_CLAIM.replace("sci-", "old-"),
            A_PARK.replace("sci-", "old-")
        ));
        let text = store.carried_renamed_text("old", "new").unwrap();
        assert!(text.contains("[parks.new-000002]"), "{text}");
        assert!(!text.contains("claims"), "{text}");
        assert!(!text.contains("old-"), "{text}");
        assert!(matches!(
            store.carried_renamed_text("sci", "new"),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn inserting_a_claim_displaces_a_park_on_the_same_id() {
        let (_dir, mut store) = store_from(A_PARK);
        let id = TaskId::parse("sci-000002").unwrap();
        store.insert(
            &id,
            Claim {
                owner: "o".into(),
                session: "b".into(),
                pid: None,
                pid_start: None,
                boot_id: None,
                pid_view: None,
                host: "h".into(),
                worktree: "/w".into(),
                started: "2026-09-09T22:00:00Z".into(),
                seen: "2026-09-09T22:00:00Z".into(),
                holds: Vec::new(),
                root_session: None,
            },
        );
        assert_eq!(store.parks().count(), 0, "one entry per task");
        assert_eq!(store.get(&id).map(|c| c.session.as_str()), Some("b"));
    }

    #[test]
    fn loading_rejects_an_id_in_both_maps_and_a_bad_park() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        std::fs::write(
            &path,
            format!("{A_CLAIM}{}", A_PARK.replace("sci-000002", "sci-000001")),
        )
        .unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err().to_string();
        assert!(error.contains("both a claim and a park"), "{error}");

        std::fs::write(&path, A_PARK.replace("2026-09-09T21:00:00Z", "yesterday")).unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err().to_string();
        assert!(error.contains("unreadable at"), "{error}");

        std::fs::write(&path, A_PARK.replace("write §3", "")).unwrap();
        let error = ClaimStore::load_from(&path).unwrap_err().to_string();
        assert!(error.contains("next_step"), "{error}");
    }

    #[test]
    fn the_snapshot_carries_parks() {
        let (dir, _store) = store_from(&format!("{A_CLAIM}{A_PARK}"));
        let snapshot =
            ClaimSnapshot::load_from_paths(std::iter::once(dir.path().join("sci.toml"))).unwrap();
        let parked = TaskId::parse("sci-000002").unwrap();
        assert_eq!(
            snapshot.park(&parked).map(|p| p.session.as_str()),
            Some("claude:x")
        );
        assert!(
            snapshot
                .park(&TaskId::parse("sci-000001").unwrap())
                .is_none()
        );
        assert!(snapshot.held(&parked).is_none());
    }

    #[test]
    fn a_claim_written_before_holds_loads_holding_nothing_and_saves_without_the_key() {
        let (dir, store) = store_from(A_CLAIM);
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(store.get(&id).unwrap().holds.is_empty());
        store.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join("sci.toml")).unwrap();
        assert!(!text.contains("holds"), "{text}");
    }

    #[test]
    fn holds_round_trip_on_a_claim_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sci.toml");
        let id = TaskId::parse("sci-000001").unwrap();
        let mut store = ClaimStore::load_from(&path).unwrap();
        store.insert(
            &id,
            Claim {
                holds: vec!["quiet".into()],
                ..sample_claim()
            },
        );
        store.save().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("holds = [\"quiet\"]"), "{text}");
        assert_eq!(
            ClaimStore::load_from(&path)
                .unwrap()
                .get(&id)
                .unwrap()
                .holds,
            ["quiet"]
        );
    }

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

    /// Just after `sample()`'s `seen`, inside the TTL.
    fn t0() -> time::OffsetDateTime {
        at("2026-09-05T08:00:01Z")
    }

    fn boot() -> Option<String> {
        Some("boot-a".into())
    }

    #[test]
    fn an_exported_pid_found_in_the_initial_namespace_records_the_view() {
        let got = evidence(
            &ident(Some(42), Some(PidSource::Exported)),
            None,
            Some(&INIT),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got.pid_view, Some(INIT.to_string()));
        assert_eq!((got.pid, got.pid_start), (Some(42), Some(999)));
    }

    #[test]
    fn an_exported_pid_records_no_view_outside_the_initial_namespace() {
        // Spec round 2: a colliding ancestor is found, and still establishes nothing.
        let got = evidence(
            &ident(Some(2), Some(PidSource::Exported)),
            None,
            Some(&CHILD),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got.pid_view, None);
        assert_eq!((got.pid, got.pid_start), (Some(2), Some(999)));
    }

    #[test]
    fn an_exported_pid_not_found_or_in_an_unknown_view_records_no_view() {
        let missing = evidence(
            &ident(Some(42), Some(PidSource::Exported)),
            None,
            Some(&INIT),
            boot(),
            t0(),
            |_| ProcStat::NotFound,
        );
        assert_eq!((missing.pid_view, missing.pid_start), (None, None));
        let unknown = evidence(
            &ident(Some(42), Some(PidSource::Exported)),
            None,
            None,
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(unknown.pid_view, None);
    }

    #[test]
    fn a_found_session_id_records_the_view_in_any_known_view() {
        let got = evidence(
            &ident(Some(7), Some(PidSource::UnixSession)),
            None,
            Some(&CHILD),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got.pid_view, Some(CHILD.to_string()));
    }

    #[test]
    fn a_zero_session_id_establishes_nothing() {
        let got = evidence(
            &ident(Some(0), Some(PidSource::UnixSession)),
            None,
            Some(&INIT),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got.pid_view, None);
    }

    #[test]
    fn a_relay_match_records_the_view_from_its_proof() {
        let me = Identity {
            proof: Some(Proof {
                pid_start: 555,
                boot_id: "boot-r".into(),
                host: "h".into(),
            }),
            ..ident(Some(42), Some(PidSource::Relay))
        };
        let got = evidence(&me, None, Some(&CHILD), boot(), t0(), |_| {
            ProcStat::NotFound
        });
        assert_eq!(got.pid_view, Some(CHILD.to_string()));
        assert_eq!(
            (got.pid_start, got.boot_id.as_deref()),
            (Some(555), Some("boot-r"))
        );
    }

    fn recorded() -> Claim {
        Claim {
            pid_view: Some(INIT.to_string()),
            ..sample()
        }
    }

    #[test]
    fn an_established_heartbeat_replaces_the_evidence() {
        let got = evidence(
            &ident(Some(43), Some(PidSource::Exported)),
            Some(&recorded()),
            Some(&INIT),
            boot(),
            t0(),
            found(1000),
        );
        assert_eq!((got.pid, got.pid_start), (Some(43), Some(1000)));
        assert_eq!(got.pid_view, Some(INIT.to_string()));
    }

    #[test]
    fn an_unestablished_heartbeat_of_the_same_pid_keeps_the_evidence() {
        // The sandbox heartbeat: same exported pid, child view, so nothing is established.
        let own = recorded();
        let got = evidence(
            &ident(own.pid, Some(PidSource::Exported)),
            Some(&own),
            Some(&CHILD),
            boot(),
            t0(),
            |_| ProcStat::NotFound,
        );
        assert_eq!(got, Evidence::of(&own));
    }

    #[test]
    fn an_unestablished_heartbeat_of_a_new_pid_records_it_without_a_view() {
        let got = evidence(
            &ident(Some(77), Some(PidSource::Exported)),
            Some(&recorded()),
            Some(&CHILD),
            boot(),
            t0(),
            found(31),
        );
        assert_eq!(
            (got.pid, got.pid_start, got.pid_view),
            (Some(77), Some(31), None)
        );
    }

    #[test]
    fn a_heartbeat_without_a_pid_keeps_the_recorded_evidence() {
        let own = recorded();
        let got = evidence(
            &ident(None, None),
            Some(&own),
            Some(&INIT),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got, Evidence::of(&own));
    }

    #[test]
    fn a_proof_continuation_keeps_the_claims_evidence() {
        let own = recorded();
        let got = evidence(
            &continuation_identity(&own),
            Some(&own),
            Some(&INIT),
            boot(),
            t0(),
            found(999),
        );
        assert_eq!(got, Evidence::of(&own));
    }

    #[test]
    fn a_claim_from_another_view_is_unverifiable_however_it_would_read() {
        for stat in [
            ProcStat::NotFound,
            ProcStat::Found {
                state: 'S',
                starttime: 12345,
            },
        ] {
            assert_eq!(
                liveness_with(
                    &seen("2026-09-05T00:00:00Z"),
                    at("2026-09-05T00:00:01Z"),
                    Some("boot-a"),
                    Some(&CHILD),
                    |_| stat.clone()
                ),
                Liveness::Unverifiable("recorded in another PID view".into())
            );
        }
    }

    #[test]
    fn an_unverifiable_claim_never_ages_into_stale() {
        assert!(
            matches!(
                liveness_with(
                    &seen("2026-09-01T00:00:00Z"),
                    at("2026-09-05T00:00:00Z"),
                    Some("boot-a"),
                    Some(&CHILD),
                    |_| ProcStat::Unreadable
                ),
                Liveness::Unverifiable(_)
            ),
            "a live owner past four hours keeps its claim against a sandboxed caller"
        );
    }

    #[test]
    fn a_legacy_pid_claim_is_unverifiable_even_when_its_pid_is_gone() {
        let legacy = Claim {
            pid_view: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        assert_eq!(
            liveness_with(
                &legacy,
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                Some(&INIT),
                |_| ProcStat::NotFound
            ),
            Liveness::Unverifiable("PID view not established when recorded".into())
        );
    }

    #[test]
    fn an_unknown_current_view_is_unverifiable() {
        assert_eq!(
            liveness_with(
                &seen("2026-09-05T00:00:00Z"),
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                None,
                |_| ProcStat::NotFound
            ),
            Liveness::Unverifiable("this command cannot establish its PID view".into())
        );
    }

    #[test]
    fn a_malformed_recorded_view_is_unverifiable() {
        let odd = Claim {
            pid_view: Some("garbage".into()),
            ..seen("2026-09-05T00:00:00Z")
        };
        assert!(matches!(
            liveness_with(
                &odd,
                at("2026-09-05T00:00:01Z"),
                Some("boot-a"),
                Some(&INIT),
                |_| ProcStat::NotFound
            ),
            Liveness::Unverifiable(_)
        ));
    }

    #[test]
    fn a_reboot_is_stale_whatever_the_view() {
        for view in [Some(&INIT), Some(&CHILD), None] {
            let legacy = Claim {
                pid_view: None,
                ..seen("2026-09-05T00:00:00Z")
            };
            assert!(matches!(
                liveness_with(
                    &legacy,
                    at("2026-09-05T00:00:01Z"),
                    Some("boot-b"),
                    view,
                    found(999)
                ),
                Liveness::Stale(_)
            ));
        }
    }

    #[test]
    fn a_pidless_claim_takes_the_ttl_whatever_the_view() {
        let pidless = Claim {
            pid: None,
            pid_view: None,
            ..seen("2026-09-05T00:00:00Z")
        };
        for view in [Some(&INIT), Some(&CHILD), None] {
            assert!(matches!(
                liveness_with(
                    &pidless,
                    at("2026-09-05T05:00:00Z"),
                    Some("boot-a"),
                    view,
                    found(999)
                ),
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
        store.insert(
            &id,
            Claim {
                pid_view: Some("0:1".into()),
                pid: Some(4_194_303),
                boot_id: boot_id(),
                ..sample()
            },
        );
        store.prune_dead();
        assert!(store.get(&id).is_some());
    }

    #[test]
    fn a_heartbeat_does_not_keep_evidence_that_reads_stale_here() {
        // Final review, Important 1: keeping a dead pid makes the owner's own new claim
        // stale, so another session could take it without --force.
        let own = recorded();
        let gone = evidence(
            &ident(None, None),
            Some(&own),
            Some(&INIT),
            boot(),
            t0(),
            |_| ProcStat::NotFound,
        );
        assert_eq!(
            (gone.pid, gone.pid_view),
            (None, None),
            "a pid-less heartbeat after death"
        );
        let rebooted = evidence(
            &ident(None, None),
            Some(&own),
            Some(&INIT),
            Some("boot-b".into()),
            t0(),
            found(999),
        );
        assert_eq!(
            rebooted.pid, None,
            "evidence from an earlier boot is not kept"
        );
        // Evidence this view cannot judge is still kept: the sandbox heartbeat.
        let elsewhere = evidence(
            &ident(None, None),
            Some(&own),
            Some(&CHILD),
            boot(),
            t0(),
            |_| ProcStat::NotFound,
        );
        assert_eq!(elsewhere, Evidence::of(&own));
    }
}
