use crate::attachments::validate_name;
use crate::claims::{Escalation, Park};
use crate::error::{Error, Result};
use crate::moving::journal::JournalAttachment;
use crate::repo::atomic_write;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub trait StateRecord: Serialize + DeserializeOwned {
    fn validate_loaded(&self, path: &Path) -> Result<()> {
        self.validate(path)
    }

    fn validate(&self, path: &Path) -> Result<()>;
}

pub struct StateStore<T> {
    subdir: &'static str,
    _marker: PhantomData<fn() -> T>,
}

impl<T: StateRecord> StateStore<T> {
    pub const fn new(subdir: &'static str) -> Self {
        Self {
            subdir,
            _marker: PhantomData,
        }
    }

    pub fn dir(&self) -> Result<PathBuf> {
        self.dir_with(|key| std::env::var_os(key))
    }

    pub fn dir_with(&self, get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf> {
        let base = if let Some(state) = get("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join("tasks").join(self.subdir))
    }

    pub fn path_with(&self, key: &str, get: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf> {
        Ok(self.dir_with(get)?.join(format!("{key}.toml")))
    }

    pub fn load_from(&self, path: &Path) -> Result<Option<T>> {
        self.load_from_with(path, T::validate_loaded)
    }

    pub fn load_from_with(
        &self,
        path: &Path,
        validate: impl FnOnce(&T, &Path) -> Result<()>,
    ) -> Result<Option<T>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let record: T = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        validate(&record, path)?;
        Ok(Some(record))
    }

    pub fn save_to(&self, record: &T, path: &Path) -> Result<()> {
        record.validate(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(record)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        atomic_write(path, text.as_bytes())
    }

    pub fn remove_from(&self, path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn pending(&self) -> Result<Vec<T>> {
        self.pending_in(&self.dir()?)
    }

    pub fn pending_in(&self, directory: &Path) -> Result<Vec<T>> {
        let read_dir = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut paths = Vec::new();
        for entry in read_dir {
            let path = entry?.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
                paths.push(path);
            }
        }
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                self.load_from(&path)?.ok_or_else(|| {
                    Error::Config(format!("{} disappeared while being read", path.display()))
                })
            })
            .collect()
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn validate_digest(path: &Path, field: &str, value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{}: {field} is not a lowercase SHA-256 digest",
            path.display()
        )))
    }
}

pub fn state_error(path: &Path, error: Error) -> Error {
    Error::Config(format!("{}: {error}", path.display()))
}

pub fn check_stem(path: &Path, source: &str) -> Result<()> {
    if path.file_stem().and_then(|stem| stem.to_str()) != Some(source) {
        return Err(Error::Config(format!(
            "{}: filename does not match source {source:?}",
            path.display()
        )));
    }
    Ok(())
}

pub fn validate_source_digest(path: &Path, text: &str, sha: &str, message: &str) -> Result<()> {
    validate_digest(path, "source_digest", sha)?;
    if digest(text.as_bytes()) != sha {
        return Err(Error::Config(format!("{}: {message}", path.display())));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn validate_carried_fields(
    path: &Path,
    entry_label: Option<&str>,
    source_text: &str,
    source_digest: &str,
    attachments: &[JournalAttachment],
    source_park: Option<&Park>,
    target_park: Option<&Park>,
    source_escalation: Option<&Escalation>,
    target_escalation: Option<&Escalation>,
) -> Result<()> {
    let digest_msg = if entry_label.is_some() {
        "a source_text does not digest to its source_digest"
    } else {
        "source_text does not digest to source_digest"
    };
    validate_source_digest(path, source_text, source_digest, digest_msg)?;

    let prefix = match entry_label {
        Some(id) => format!("{}: {id}: ", path.display()),
        None => format!("{}: ", path.display()),
    };
    let mut seen = BTreeSet::new();
    for attachment in attachments {
        validate_name(&attachment.name).map_err(|error| state_error(path, error))?;
        if !seen.insert(&attachment.name) {
            return Err(Error::Config(match entry_label {
                Some(id) => format!(
                    "{}: duplicate attachment {:?} on {id}",
                    path.display(),
                    attachment.name
                ),
                None => format!(
                    "{}: duplicate attachment {:?}",
                    path.display(),
                    attachment.name
                ),
            }));
        }
        validate_digest(path, "attachment sha256", &attachment.sha256)?;
    }
    if source_park.is_some() != target_park.is_some() {
        return Err(Error::Config(format!(
            "{prefix}source_park and target_park must both be present or absent"
        )));
    }
    if source_escalation != target_escalation {
        return Err(Error::Config(format!(
            "{prefix}source_escalation and target_escalation must match"
        )));
    }
    if let (Some(source), Some(target)) = (source_park, target_park) {
        let mut original = target.clone();
        original.worktree.clone_from(&source.worktree);
        if &original != source {
            return Err(Error::Config(format!(
                "{prefix}target_park must match source_park except for worktree"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct DummyRecord {
        source: String,
        digest: String,
    }

    impl StateRecord for DummyRecord {
        fn validate_loaded(&self, path: &Path) -> Result<()> {
            self.validate(path)?;
            check_stem(path, &self.source)
        }

        fn validate(&self, path: &Path) -> Result<()> {
            validate_digest(path, "digest", &self.digest)
        }
    }

    const STORE: StateStore<DummyRecord> = StateStore::new("dummy");

    #[test]
    fn store_resolves_xdg_and_home_paths_and_round_trips_sorted_pending_records() {
        let state = tempfile::tempdir().unwrap();
        let path_a = STORE
            .path_with("a", |key| {
                (key == "XDG_STATE_HOME").then(|| state.path().as_os_str().to_owned())
            })
            .unwrap();
        assert_eq!(path_a, state.path().join("tasks/dummy/a.toml"));
        assert_eq!(
            STORE
                .path_with("a", |key| (key == "HOME").then(|| "/home/u".into()))
                .unwrap(),
            Path::new("/home/u/.local/state/tasks/dummy/a.toml")
        );
        assert_eq!(STORE.path_with("a", |_| None).unwrap_err().kind(), "config");

        let rec_b = DummyRecord {
            source: "b".into(),
            digest: digest(b"b"),
        };
        let rec_a = DummyRecord {
            source: "a".into(),
            digest: digest(b"a"),
        };
        let dir = state.path().join("tasks/dummy");
        STORE.save_to(&rec_b, &dir.join("b.toml")).unwrap();
        STORE.save_to(&rec_a, &dir.join("a.toml")).unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

        assert_eq!(
            STORE.pending_in(&dir).unwrap(),
            vec![rec_a.clone(), rec_b.clone()]
        );
        STORE.remove_from(&dir.join("a.toml")).unwrap();
        assert_eq!(STORE.load_from(&dir.join("a.toml")).unwrap(), None);
        STORE.remove_from(&dir.join("a.toml")).unwrap();
    }
}
