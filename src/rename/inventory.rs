use crate::error::{Error, Result};
use crate::model::{TaskId, is_valid_prefix};
use crate::repo::{CONFIG_REL, Project};
pub use crate::state_store::digest;
use crate::state_store::{StateRecord, StateStore, check_stem, state_error, validate_digest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const STORE: StateStore<Inventory> = StateStore::new("rename");
const COMMIT_STORE: StateStore<Inventory> = StateStore::new("rename-commit");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub hex: String,
    pub from: String,
    pub to: String,
    /// Whether `tasks/files/<source>-<hex>/` existed when the inventory was built. No serde
    /// default: an inventory from a binary that predates attachments fails to load rather
    /// than guessing (spec, "Rename").
    pub attachments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredecessorHandoff {
    /// Exact pre-rename source config, bound by Inventory::config_from even after
    /// the config and detached records stages have replaced its original path.
    pub source_config: String,
    pub prefix: String,
    pub suffixes: BTreeSet<String>,
    pub config_path: PathBuf,
    pub config_from: String,
    pub config_to: String,
    pub config_payload: String,
}

impl PredecessorHandoff {
    pub fn predecessor(&self, source: &str) -> crate::repo::Predecessor {
        crate::repo::Predecessor {
            prefix: self.prefix.clone(),
            successor_alias: source.into(),
            suffixes: self.suffixes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub source: String,
    pub target: String,
    /// The project's identity, the registered root; for a detached project the code
    /// checkout, never its records directory.
    pub root: PathBuf,
    pub config_from: String,
    pub config_to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_handoff: Option<PredecessorHandoff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_payload: Option<String>,
    pub entries: Vec<InventoryEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parks_store: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_to: Option<String>,
    /// A detached project's records directory, `<records-root>/<source>`, and where the
    /// `records` stage moves it, `<records-root>/<target>`; both absent otherwise. An
    /// inventory from an older binary has neither, which means "records at `root`": what
    /// it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub records_from: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub records_to: Option<PathBuf>,
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

        let config_bytes = std::fs::read(project.records.join(CONFIG_REL))?;
        let config_text = std::str::from_utf8(&config_bytes)
            .map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
        let edge = crate::retained::lineage::prepare(project, target)?;
        let config_payload = edge
            .as_ref()
            .map(|e| rewrite_config_lineage(config_text, target, e))
            .transpose()?;
        let config_to = match &config_payload {
            Some(payload) => payload.clone(),
            None => rewrite_config_prefix(config_text, target)?,
        };
        let mut entries = Vec::new();
        if let crate::attachments::DirState::Unsafe(detail) =
            crate::attachments::dir_state(&crate::attachments::files_root(project))?
        {
            return Err(Error::AttachmentUnsafe(detail));
        }
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
            validate_task_file(&path, &source, &id)?;
            let rewritten = super::rewrite::rewrite_prefix(&source, &project.prefix, target)
                .map_err(|error| error.with_suffix(&format!(" in tasks/{name}")))?;
            let attachments =
                match crate::attachments::dir_state(&crate::attachments::task_dir(project, &id))? {
                    crate::attachments::DirState::Absent => false,
                    crate::attachments::DirState::Directory => true,
                    crate::attachments::DirState::Unsafe(detail) => {
                        return Err(Error::AttachmentUnsafe(detail));
                    }
                };
            entries.push(InventoryEntry {
                hex: id.hex,
                from: digest(source.as_bytes()),
                to: digest(rewritten.as_bytes()),
                attachments,
            });
        }
        let files_root = crate::attachments::files_root(project);
        if crate::attachments::dir_state(&files_root)? == crate::attachments::DirState::Directory {
            let inventoried: BTreeSet<String> = entries
                .iter()
                .map(|entry| format!("{}-{}", project.prefix, entry.hex))
                .collect();
            let mut strays = Vec::new();
            for entry in std::fs::read_dir(&files_root)? {
                let entry = entry?;
                // `DirEntry::file_type` does not follow a symlink; an inventoried name that
                // is a symlink already failed the per-entry `dir_state` above.
                let owned = entry.file_type()?.is_dir()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| inventoried.contains(name));
                if !owned {
                    strays.push(entry.path());
                }
            }
            if !strays.is_empty() {
                strays.sort();
                return Err(Error::Validation(format!(
                    "R5: task files outside the inventory: {strays:?}"
                )));
            }
        }
        entries.sort_by(|left, right| left.hex.cmp(&right.hex));
        let store = crate::claims::ClaimStore::load(&project.prefix)?;
        let (parks_store, store_to) = if store.carries_nothing() {
            (None, None)
        } else {
            let text = store.carried_renamed_text(&project.prefix, target)?;
            let digest = digest(text.as_bytes());
            (Some(text), Some(digest))
        };
        let (records_from, records_to) = match project.kind {
            crate::repo::Kind::Detached => {
                let parent = project.records.parent().ok_or_else(|| {
                    Error::Config(format!(
                        "records directory {} has no parent",
                        project.records.display()
                    ))
                })?;
                (Some(project.records.clone()), Some(parent.join(target)))
            }
            crate::repo::Kind::InTree | crate::repo::Kind::Linked => (None, None),
        };

        Ok(Inventory {
            source: project.prefix.clone(),
            target: target.into(),
            root: project.root.clone(),
            config_from: digest(&config_bytes),
            config_to: digest(config_to.as_bytes()),
            config_payload,
            predecessor_handoff: None,
            entries,
            parks_store,
            store_to,
            records_from,
            records_to,
        })
    }

    pub fn with_handoff(
        mut self,
        project: &Project,
        handoff: Option<PredecessorHandoff>,
    ) -> Result<Self> {
        if let Some(handoff) = handoff {
            let source = std::fs::read_to_string(project.records.join(CONFIG_REL))?;
            let payload = with_predecessor(
                &self.target_config(&source)?,
                &handoff.predecessor(&self.source),
            )?;
            self.config_to = digest(payload.as_bytes());
            self.config_payload = Some(payload);
            self.predecessor_handoff = Some(handoff);
        }
        Ok(self)
    }

    fn validate_edge_free_handoff(
        &self,
        source: &Project,
        handoff: &PredecessorHandoff,
        payload: &str,
    ) -> Result<()> {
        let missing =
            || Error::Config("handoff payload lacks required frozen retention lineage".into());
        if !source.retention_lineage.is_empty()
            || with_predecessor(
                &rewrite_config_prefix(&handoff.source_config, &self.target)?,
                &handoff.predecessor(&self.source),
            )? != payload
        {
            return Err(missing());
        }
        for entry in &self.entries {
            let old = TaskId::parse(&format!("{}-{}", self.source, entry.hex))?;
            let new = TaskId::parse(&format!("{}-{}", self.target, entry.hex))?;
            let mut found = false;
            for (id, expected) in [(&old, &entry.from), (&new, &entry.to)] {
                let path = source.tasks_dir().join(format!("{id}.md"));
                let raw = match std::fs::read_to_string(&path) {
                    Ok(raw) => raw,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                if digest(raw.as_bytes()) != *expected {
                    return Err(Error::Config(format!(
                        "handoff task differs from frozen inventory: {}",
                        path.display()
                    )));
                }
                found = true;
                let physical = crate::format::parse_physical(&raw, &path.display().to_string())?;
                if physical.id() != id {
                    return Err(Error::Config("handoff task identity mismatch".into()));
                }
                let task = crate::history::load_logical(physical, |archive| {
                    use crate::attachments::{DirState, dir_state, task_dir};
                    let archive_id = match (
                        dir_state(&task_dir(source, &old))?,
                        dir_state(&task_dir(source, &new))?,
                    ) {
                        (DirState::Directory, DirState::Absent) => &old,
                        (DirState::Absent, DirState::Directory) => &new,
                        _ => {
                            return Err(Error::Config(
                                "handoff archive location is missing or ambiguous".into(),
                            ));
                        }
                    };
                    source.read_archive(archive_id, archive)
                })?;
                if crate::retained::lineage::relevant(&task) {
                    return Err(missing());
                }
            }
            if !found {
                return Err(Error::Config(format!(
                    "handoff task {} is missing from frozen inventory paths",
                    entry.hex
                )));
            }
        }
        Ok(())
    }

    pub fn target_config(&self, source: &str) -> Result<String> {
        if digest(source.as_bytes()) != self.config_from {
            return Err(Error::Validation(
                "source config differs from rename inventory".into(),
            ));
        }
        match &self.config_payload {
            Some(payload) => Ok(payload.clone()),
            None => rewrite_config_prefix(source, &self.target),
        }
    }
    pub fn path(source: &str) -> Result<PathBuf> {
        Self::path_with(source, |key| std::env::var_os(key))
    }

    pub(crate) fn path_with(
        source: &str,
        get: impl Fn(&str) -> Option<OsString>,
    ) -> Result<PathBuf> {
        validate_prefix(source)?;
        STORE.path_with(source, get)
    }

    pub fn load(source: &str) -> Result<Option<Inventory>> {
        Self::load_from(&Self::path(source)?)
    }

    pub(crate) fn load_from(path: &Path) -> Result<Option<Inventory>> {
        STORE.load_from(path)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path(&self.source)?)
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        STORE.save_to(self, path)
    }

    pub fn remove(&self) -> Result<()> {
        Self::remove_from(&Self::path(&self.source)?)
    }

    pub(crate) fn remove_from(path: &Path) -> Result<()> {
        STORE.remove_from(path)
    }

    pub fn save_commit_receipt(&self) -> Result<()> {
        COMMIT_STORE.save_to(self, &Self::commit_receipt_path(&self.target)?)
    }

    pub fn load_commit_receipt(target: &str) -> Result<Option<Inventory>> {
        COMMIT_STORE.load_from_with(&Self::commit_receipt_path(target)?, |inventory, path| {
            inventory.validate(path)?;
            check_stem(path, &inventory.target)
        })
    }

    pub fn remove_commit_receipt(target: &str) -> Result<()> {
        COMMIT_STORE.remove_from(&Self::commit_receipt_path(target)?)
    }

    fn commit_receipt_path(target: &str) -> Result<PathBuf> {
        validate_prefix(target)?;
        COMMIT_STORE.path_with(target, |key| std::env::var_os(key))
    }

    pub fn pending() -> Result<Vec<Inventory>> {
        STORE.pending()
    }

    #[cfg(test)]
    pub(crate) fn pending_in(directory: &Path) -> Result<Vec<Inventory>> {
        STORE.pending_in(directory)
    }

    pub(crate) fn validate(&self, path: &Path) -> Result<()> {
        validate_prefix(&self.source).map_err(|error| state_error(path, error))?;
        validate_prefix(&self.target).map_err(|error| state_error(path, error))?;
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
        if self.predecessor_handoff.is_some() && self.config_payload.is_none() {
            return Err(Error::Config("handoff lacks frozen target config".into()));
        }
        if let Some(payload) = &self.config_payload {
            if digest(payload.as_bytes()) != self.config_to {
                return Err(Error::Config(
                    "rename config payload digest mismatch".into(),
                ));
            }
            let records = self
                .records_from
                .as_ref()
                .filter(|p| p.is_dir())
                .or_else(|| self.records_to.as_ref().filter(|p| p.is_dir()))
                .unwrap_or(&self.root);
            let source_project = if let Some(handoff) = &self.predecessor_handoff {
                if digest(handoff.source_config.as_bytes()) != self.config_from {
                    return Err(Error::Config(
                        "handoff source config digest mismatch".into(),
                    ));
                }
                let source = Project::from_config(records.canonicalize()?, &handoff.source_config)?;
                if source.prefix != self.source {
                    return Err(Error::Config(
                        "handoff source config prefix mismatch".into(),
                    ));
                }
                Some(source)
            } else {
                None
            };
            let project = Project::from_config(records.canonicalize()?, payload)?;
            if project.prefix != self.target
                || self
                    .predecessor_handoff
                    .as_ref()
                    .is_some_and(|h| project.predecessor != Some(h.predecessor(&self.source)))
            {
                return Err(Error::Config(
                    "rename payload disagrees with target or predecessor".into(),
                ));
            }
            if let Some(edge) = project.retention_lineage.last() {
                if project.prefix != self.target
                    || edge.from_prefix != self.source
                    || edge.to_prefix != self.target
                {
                    return Err(Error::Config(
                        "retention inventory edge disagrees with its endpoints".into(),
                    ));
                }
                let id = TaskId::parse(&format!("{}-000001", edge.from_prefix))?;
                crate::retained::model::Basis {
                    project: edge.from_prefix.clone(),
                    layout: edge.layout.clone(),
                    records_subdir: edge.from_subdir.clone(),
                    commit: edge.commit.clone(),
                    task_path: crate::retained::lineage::task_path(&edge.from_subdir, &id),
                    retained_ref: edge.retained_ref.clone(),
                }
                .validate()?;
                if edge.version != 1
                    || edge.layout != project.kind.as_str()
                    || !(edge.to_subdir == "."
                        || crate::retained::model::member_path(&edge.to_subdir))
                {
                    return Err(Error::Config("invalid frozen retention edge".into()));
                }
                crate::retained::basis::git(
                    &project,
                    &["show-ref", "--verify", &edge.retained_ref],
                )?;
                crate::retained::basis::git(
                    &project,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        &edge.commit,
                        &edge.retained_ref,
                    ],
                )?;
                let source_path = format!("{}/tasks/.config.toml", edge.from_subdir)
                    .trim_start_matches("./")
                    .to_string();
                let source = crate::retained::basis::blob(
                    &project,
                    &edge.commit,
                    &source_path,
                    None,
                    &mut 0,
                )?;
                if digest(&source) != self.config_from || {
                    let rewritten = rewrite_config_lineage(
                        std::str::from_utf8(&source).map_err(|e| Error::Config(e.to_string()))?,
                        &self.target,
                        edge,
                    )?;
                    match &self.predecessor_handoff {
                        Some(h) => with_predecessor(&rewritten, &h.predecessor(&self.source))?,
                        None => rewritten,
                    }
                } != *payload
                {
                    return Err(Error::Config(
                        "retention payload differs from anchored source config".into(),
                    ));
                }
            } else if let (Some(handoff), Some(source)) =
                (&self.predecessor_handoff, source_project)
            {
                self.validate_edge_free_handoff(&source, handoff, payload)?;
            } else {
                return Err(Error::Config(
                    "rename payload lacks retention lineage or predecessor handoff".into(),
                ));
            }
        }
        if let Some(h) = &self.predecessor_handoff {
            h.predecessor(&self.source).validate(&self.target)?;
            validate_digest(path, "predecessor config_from", &h.config_from)?;
            validate_digest(path, "predecessor config_to", &h.config_to)?;
            if !h.config_path.is_absolute()
                || !h.config_path.ends_with(CONFIG_REL)
                || digest(h.config_payload.as_bytes()) != h.config_to
            {
                return Err(Error::Config(
                    "invalid predecessor handoff config path or digest".into(),
                ));
            }
            let pred = Project::from_config(
                h.config_path
                    .parent()
                    .and_then(Path::parent)
                    .expect("absolute config root")
                    .to_path_buf(),
                &h.config_payload,
            )?;
            if pred.prefix != h.prefix
                || pred.predecessor.is_some()
                || pred.retired.as_ref().and_then(|r| r.successor.as_deref())
                    != Some(self.target.as_str())
            {
                return Err(Error::Config("invalid predecessor handoff payload".into()));
            }
        }

        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            TaskId::parse(&format!("{}-{}", self.source, entry.hex))
                .map_err(|error| state_error(path, error))?;
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
        match (&self.records_from, &self.records_to) {
            (None, None) => {}
            (Some(from), Some(to)) => {
                if !from.is_absolute() || !to.is_absolute() || from == to {
                    return Err(Error::Config(format!(
                        "{}: records_from and records_to must be two absolute paths",
                        path.display()
                    )));
                }
            }
            _ => {
                return Err(Error::Config(format!(
                    "{}: records_from and records_to must be present together",
                    path.display()
                )));
            }
        }
        match (&self.parks_store, &self.store_to) {
            (None, None) => {}
            (Some(text), Some(expected)) => {
                validate_digest(path, "store_to", expected)?;
                if digest(text.as_bytes()) != *expected {
                    return Err(Error::Config(format!(
                        "{}: parks_store does not digest to store_to",
                        path.display()
                    )));
                }
            }
            _ => {
                return Err(Error::Config(format!(
                    "{}: parks_store and store_to must be present together",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_task_file(path: &Path, text: &str, archive_owner: &TaskId) -> Result<()> {
    let file = path.display().to_string();
    let physical = crate::format::parse_physical(text, &file)?;
    let records = path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Error::Validation("task record has no explicit records root".into()))?;
    let task = crate::history::load_logical(physical, |archive| {
        crate::repo::Project::open(records)?.read_archive(archive_owner, archive)
    })?;
    if path.file_stem().and_then(|name| name.to_str()) != Some(task.id.to_string().as_str()) {
        return Err(Error::Parse {
            file,
            detail: format!("id field is {}", task.id),
        });
    }
    crate::review::validate(&task)?;
    crate::defer::validate(&task)
}

pub(crate) fn task_paths(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let Some(file_name) = path.file_name() else {
            continue;
        };
        if file_name.as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }
        if path.extension() != Some(std::ffi::OsStr::new("md")) {
            continue;
        }
        file_name.to_str().ok_or_else(|| Error::Parse {
            file: path.display().to_string(),
            detail: "filename is not valid UTF-8".into(),
        })?;
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

impl StateRecord for Inventory {
    fn validate_loaded(&self, path: &Path) -> Result<()> {
        self.validate(path)?;
        check_stem(path, &self.source)
    }

    fn validate(&self, path: &Path) -> Result<()> {
        Inventory::validate(self, path)
    }
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

fn rewrite_config_lineage(
    source: &str,
    target: &str,
    edge: &crate::retained::lineage::RenameEdge,
) -> Result<String> {
    let mut v: toml::Value = toml::from_str(&rewrite_config_prefix(source, target)?)
        .map_err(|e| Error::Config(e.to_string()))?;
    let lineage = v
        .as_table_mut()
        .expect("config table")
        .entry("retention_lineage")
        .or_insert(toml::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| Error::Config("retention lineage must be an array".into()))?;
    lineage.push(toml::Value::try_from(edge).map_err(|e| Error::Config(e.to_string()))?);
    toml::to_string(&v).map_err(|e| Error::Config(e.to_string()))
}

fn with_predecessor(text: &str, predecessor: &crate::repo::Predecessor) -> Result<String> {
    let mut config: toml::Value = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
    config.as_table_mut().expect("config table").insert(
        "predecessor".into(),
        toml::Value::try_from(predecessor).map_err(|e| Error::Config(e.to_string()))?,
    );
    toml::to_string(&config).map_err(|e| Error::Config(e.to_string()))
}
