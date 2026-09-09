use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    identity_from(|key| std::env::var_os(key), unix_session_id())
}

/// Session and pid resolve as a pair from a single level. A level that yields a session
/// but no usable pid yields `pid: None` and falls to the TTL path; it is never welded to
/// an unrelated fallback pid.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitingOn {
    User,
    Agent,
}

impl WaitingOn {
    pub fn as_str(self) -> &'static str {
        match self {
            WaitingOn::User => "user",
            WaitingOn::Agent => "agent",
        }
    }
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
    /// Snapshot at park time, for rows whose task file is unreachable (spec §5.3).
    pub title: String,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StoreFile {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    claims: BTreeMap<String, Claim>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parks: BTreeMap<String, Park>,
}

#[derive(Debug)]
pub struct ClaimStore {
    path: PathBuf,
    claims: BTreeMap<String, Claim>,
    parks: BTreeMap<String, Park>,
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
        let StoreFile { claims, parks } = file;
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
        Ok(ClaimStore {
            path: path.to_path_buf(),
            claims,
            parks,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = StoreFile {
            claims: self.claims.clone(),
            parks: self.parks.clone(),
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

    pub fn remove(&mut self, id: &TaskId) -> Option<Claim> {
        self.claims.remove(&id.to_string())
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

    pub fn prune_dead(&mut self) {
        self.prune_with(|claim| liveness(claim) == Liveness::Live);
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
}

/// Every claim in scope, with its liveness verdict, and every park entry, read once per
/// command.
#[derive(Debug, Default)]
pub struct ClaimSnapshot {
    by_id: BTreeMap<String, (Claim, Liveness)>,
    parks: BTreeMap<String, Park>,
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
        for path in paths {
            let store = ClaimStore::load_from(&path)?;
            for (id, claim) in store.iter() {
                by_id.insert(id.clone(), (claim.clone(), liveness(claim)));
            }
            for (id, park) in store.parks() {
                parks.insert(id.clone(), park.clone());
            }
        }
        Ok(ClaimSnapshot { by_id, parks })
    }

    pub fn park(&self, id: &TaskId) -> Option<&Park> {
        self.parks.get(&id.to_string())
    }

    pub fn get(&self, id: &TaskId) -> Option<&(Claim, Liveness)> {
        self.by_id.get(&id.to_string())
    }

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
        Err(_) => Liveness::Live,
    };

    let Some(pid) = claim.pid else { return ttl() };
    match (boot_id, &claim.boot_id) {
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
        ProcStat::Found { starttime, .. } => match claim.pid_start {
            Some(recorded) if recorded != starttime => {
                Liveness::Stale(format!("pid {pid} was reused by another process"))
            }
            Some(_) => Liveness::Live,
            None => ttl(),
        },
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
            host: "h".into(),
            worktree: "/tmp/wt".into(),
            started: "2026-09-05T08:00:00Z".into(),
            seen: "2026-09-05T08:00:00Z".into(),
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
        assert_eq!(
            id.session, "sid:11",
            "emptiness is filtered inside the helper"
        );
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
                host: "h".into(),
                worktree: "/w".into(),
                started: "2026-09-09T22:00:00Z".into(),
                seen: "2026-09-09T22:00:00Z".into(),
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
        assert!(snapshot.live(&parked).is_none());
    }
}
