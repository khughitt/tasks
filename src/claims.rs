// Removed in Task 7, whose guard is the first consumer of the whole surface. Until then
// this is a binary crate's unreachable `pub` API, which `clippy -D warnings` rejects — and
// the pre-commit hook runs `just check`, so without this the intermediate commits could not
// land. Task 7 deletes the attribute and proves clippy is clean without it.
#![allow(dead_code)]
// compile_error!("TDD RED probe");

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

    pub fn prune_with(&mut self, keep: impl Fn(&Claim) -> bool) {
        self.claims.retain(|_, claim| keep(claim));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Claim)> {
        self.claims.iter()
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
}
