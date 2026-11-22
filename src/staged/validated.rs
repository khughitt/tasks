//! The host-local record of staged trees that passed a staged check.
//!
//! Each passing `check --staged` records the HEAD it ran against, HEAD's records tree,
//! and the staged tree it accepted. The ordinary-note route accepts a later commit only
//! when HEAD has since published one of these trees. A record belongs to one repository
//! and project and to the inputs that decided it: the running binary (its version and
//! file identity), the project registry and the host configuration. When any of them differs the record is
//! replaced, so a new build or a changed registry starts from a full check.

use crate::error::{Error, Result};
use crate::state_store::{StateRecord, StateStore, digest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const STORE: StateStore<Record> = StateStore::new("staged-checks");
const VERSION: u32 = 1;
/// Passing trees kept per project: enough for commits that were checked and then not
/// made, without growing.
const KEEP: usize = 16;

/// One staged tree a staged check accepted, with the HEAD it was checked against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Passed {
    pub(crate) head: Option<String>,
    pub(crate) head_tree: Option<String>,
    pub(crate) tree: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    version: u32,
    /// Digest of the inputs listed in the module comment; a different key replaces the
    /// record instead of extending it.
    key: String,
    passed: Vec<Passed>,
}

fn object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl StateRecord for Record {
    fn validate(&self, path: &Path) -> Result<()> {
        let bad = |what: &str| {
            Error::Config(format!(
                "{}: {what}; remove the file and the next staged check runs in full",
                path.display()
            ))
        };
        if self.version != VERSION {
            return Err(bad(&format!("unsupported version {}", self.version)));
        }
        if self.key.len() != 64 {
            return Err(bad("key is not a SHA-256 digest"));
        }
        if self.passed.len() > KEEP {
            return Err(bad("holds more passing trees than it keeps"));
        }
        for passed in &self.passed {
            let ids = [&passed.head, &passed.head_tree]
                .into_iter()
                .flatten()
                .chain(std::iter::once(&passed.tree));
            if ids.into_iter().any(|id| !object_id(id)) {
                return Err(bad("holds a value that is not a Git object id"));
            }
        }
        Ok(())
    }
}

/// Where one project's record lives, and the key its entries must carry.
pub(crate) struct Slot {
    path: PathBuf,
    key: String,
}

fn bytes_or_absent(path: &Path) -> Result<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut keyed = b"present\0".to_vec();
            keyed.extend(bytes);
            Ok(keyed)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(b"absent\0".to_vec()),
        Err(error) => Err(error.into()),
    }
}

/// The running binary: its version and the file's identity, which every build or install
/// changes. Cheaper than hashing the executable on each commit.
fn binary_identity() -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(std::env::current_exe()?)?;
    Ok(digest(
        format!(
            "{}\0{}\0{}\0{}\0{}.{}",
            env!("TASKS_VERSION"),
            meta.dev(),
            meta.ino(),
            meta.size(),
            meta.mtime(),
            meta.mtime_nsec()
        )
        .as_bytes(),
    ))
}

impl Slot {
    /// The slot for `prefix` in the repository whose common Git directory is `common`.
    pub(crate) fn open(common: &Path, prefix: &str) -> Result<Slot> {
        let location = digest(format!("{}\0{prefix}", common.display()).as_bytes());
        let binary = binary_identity()?;
        let registry = digest(&bytes_or_absent(&crate::registry::Registry::path()?)?);
        let host = digest(&bytes_or_absent(&crate::config::HostConfig::path()?)?);
        let key = digest(
            format!(
                "staged-check v{VERSION}\0{}\0{prefix}\0{binary}\0{registry}\0{host}",
                common.display()
            )
            .as_bytes(),
        );
        Ok(Slot {
            path: STORE.dir()?.join(format!("{location}.toml")),
            key,
        })
    }

    /// The passing trees recorded under this slot's key, newest first.
    pub(crate) fn passed(&self) -> Result<Vec<Passed>> {
        Ok(match STORE.load_from(&self.path)? {
            Some(record) if record.key == self.key => record.passed,
            _ => Vec::new(),
        })
    }

    /// Records `passed` as the newest passing tree.
    pub(crate) fn record(&self, passed: Passed) -> Result<()> {
        let mut trees = self.passed()?;
        trees.retain(|known| *known != passed);
        trees.insert(0, passed);
        trees.truncate(KEEP);
        STORE.save_to(
            &Record {
                version: VERSION,
                key: self.key.clone(),
                passed: trees,
            },
            &self.path,
        )
    }
}
