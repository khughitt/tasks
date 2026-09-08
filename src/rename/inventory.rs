#![allow(dead_code)] // Persisted by the rename command in Task 11.

use crate::error::{Error, Result};
use crate::model::{TaskId, is_valid_prefix};
use crate::repo::{CONFIG_REL, Project, atomic_write};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub hex: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub source: String,
    pub target: String,
    pub root: PathBuf,
    pub config_from: String,
    pub config_to: String,
    pub entries: Vec<InventoryEntry>,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Rewrite the parsed config prefix using the same serialization P4 will write.
pub fn rewrite_config_prefix(text: &str, target: &str) -> Result<String> {
    validate_prefix(target)?;
    let mut config: toml::Value =
        toml::from_str(text).map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
    let table = config
        .as_table_mut()
        .ok_or_else(|| Error::Config(format!("{CONFIG_REL}: expected a table")))?;
    let prefix = table
        .get_mut("prefix")
        .ok_or_else(|| Error::Config(format!("{CONFIG_REL}: missing prefix")))?;
    if !prefix.is_str() {
        return Err(Error::Config(format!(
            "{CONFIG_REL}: prefix must be a string"
        )));
    }
    *prefix = toml::Value::String(target.into());
    toml::to_string(&config).map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))
}

impl Inventory {
    pub fn build(project: &Project, target: &str) -> Result<Inventory> {
        validate_prefix(&project.prefix)?;
        validate_prefix(target)?;
        if project.prefix == target {
            return Err(Error::Validation(format!(
                "source and target prefixes are both {target:?}"
            )));
        }

        let config_bytes = std::fs::read(project.root.join(CONFIG_REL))?;
        let config_text = std::str::from_utf8(&config_bytes)
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
        let config_to = rewrite_config_prefix(config_text, target)?;
        let mut entries = Vec::new();
        for path in task_paths(&project.tasks_dir())? {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| Error::Parse {
                    file: path.display().to_string(),
                    detail: "filename is not valid UTF-8".into(),
                })?;
            let id = TaskId::parse(name.strip_suffix(".md").expect("task path has .md")).map_err(
                |_| Error::Parse {
                    file: format!("tasks/{name}"),
                    detail: "filename is not a valid task id".into(),
                },
            )?;
            if id.prefix != project.prefix {
                return Err(Error::Parse {
                    file: format!("tasks/{name}"),
                    detail: format!(
                        "prefix {:?} does not match project prefix {:?}",
                        id.prefix, project.prefix
                    ),
                });
            }
            let source = std::fs::read_to_string(&path)?;
            let rewritten = super::rewrite::rewrite_prefix(&source, &project.prefix, target)
                .map_err(|error| error.with_suffix(&format!(" in tasks/{name}")))?;
            entries.push(InventoryEntry {
                hex: id.hex,
                from: digest(source.as_bytes()),
                to: digest(rewritten.as_bytes()),
            });
        }
        entries.sort_by(|left, right| left.hex.cmp(&right.hex));

        Ok(Inventory {
            source: project.prefix.clone(),
            target: target.into(),
            root: project.root.clone(),
            config_from: digest(&config_bytes),
            config_to: digest(config_to.as_bytes()),
            entries,
        })
    }

    pub fn path(source: &str) -> Result<PathBuf> {
        Self::path_with(source, |key| std::env::var_os(key))
    }

    pub(crate) fn path_with(
        source: &str,
        get: impl Fn(&str) -> Option<OsString>,
    ) -> Result<PathBuf> {
        validate_prefix(source)?;
        let base = if let Some(state) = get("XDG_STATE_HOME") {
            PathBuf::from(state)
        } else if let Some(home) = get("HOME") {
            PathBuf::from(home).join(".local/state")
        } else {
            return Err(Error::Config(
                "neither XDG_STATE_HOME nor HOME is set".into(),
            ));
        };
        Ok(base.join(format!("tasks/rename/{source}.toml")))
    }

    pub fn load(source: &str) -> Result<Option<Inventory>> {
        Self::load_from(&Self::path(source)?)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Option<Inventory>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let inventory: Inventory = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        inventory.validate(path)?;
        if path.file_stem().and_then(|stem| stem.to_str()) != Some(&inventory.source) {
            return Err(Error::Config(format!(
                "{}: filename does not match source {:?}",
                path.display(),
                inventory.source
            )));
        }
        Ok(Some(inventory))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path(&self.source)?)
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        self.validate(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(
            path,
            toml::to_string(self)
                .expect("validated inventory serializes")
                .as_bytes(),
        )
    }

    pub fn remove(&self) -> Result<()> {
        Self::remove_from(&Self::path(&self.source)?)
    }

    pub(crate) fn remove_from(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn pending() -> Result<Vec<Inventory>> {
        let directory = Self::path("aa")?
            .parent()
            .expect("inventory path has a parent")
            .to_path_buf();
        Self::pending_in(&directory)
    }

    pub(crate) fn pending_in(directory: &Path) -> Result<Vec<Inventory>> {
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
                Self::load_from(&path)?.ok_or_else(|| {
                    Error::Config(format!("{} disappeared while being read", path.display()))
                })
            })
            .collect()
    }

    pub(crate) fn validate(&self, path: &Path) -> Result<()> {
        validate_prefix(&self.source).map_err(|error| inventory_error(path, error))?;
        validate_prefix(&self.target).map_err(|error| inventory_error(path, error))?;
        if self.source == self.target {
            return Err(Error::Config(format!(
                "{}: source and target prefixes are both {:?}",
                path.display(),
                self.source
            )));
        }
        if !self.root.is_absolute() {
            return Err(Error::Config(format!(
                "{}: root must be absolute",
                path.display()
            )));
        }
        validate_digest(path, "config_from", &self.config_from)?;
        validate_digest(path, "config_to", &self.config_to)?;
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            TaskId::parse(&format!("{}-{}", self.source, entry.hex))
                .map_err(|error| inventory_error(path, error))?;
            if !seen.insert(&entry.hex) {
                return Err(Error::Config(format!(
                    "{}: duplicate entry {:?}",
                    path.display(),
                    entry.hex
                )));
            }
            validate_digest(path, "entry from", &entry.from)?;
            validate_digest(path, "entry to", &entry.to)?;
        }
        Ok(())
    }
}

pub(crate) fn task_paths(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension() != Some(std::ffi::OsStr::new("md")) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Parse {
                file: path.display().to_string(),
                detail: "filename is not valid UTF-8".into(),
            })?;
        if !name.starts_with('.') {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn validate_prefix(prefix: &str) -> Result<()> {
    if is_valid_prefix(prefix) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
        )))
    }
}

fn validate_digest(path: &Path, field: &str, value: &str) -> Result<()> {
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

fn inventory_error(path: &Path, error: Error) -> Error {
    Error::Config(format!("{}: {error}", path.display()))
}
