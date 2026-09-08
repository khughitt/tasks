#![allow(dead_code)] // Consumed by the recovery classifier and rename command in Tasks 10-11.

use crate::error::{Error, Result};
use crate::model::{TaskId, is_valid_prefix};
use crate::registry::Registry;
use crate::rename::inventory::{Inventory, digest, task_paths};
use crate::repo::CONFIG_REL;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub source: String,
    pub target: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Absent,
    Present(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryState {
    pub hex: String,
    pub source: FileState,
    pub dest: FileState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryState {
    pub old_key: Option<PathBuf>,
    pub new_key: Option<PathBuf>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigState {
    pub prefix: String,
    pub digest: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub invocation: Invocation,
    pub registry: RegistryState,
    pub config: Option<ConfigState>,
    pub inventory: Option<Inventory>,
    pub entries: Vec<EntryState>,
    pub named: Named,
    pub strays: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct ConfigPrefix {
    prefix: String,
}

pub fn observe(
    registry: &Registry,
    invocation: &Invocation,
    inventory: Option<Inventory>,
) -> Result<Snapshot> {
    validate_invocation(invocation)?;
    if let Some(inventory) = &inventory {
        inventory.validate(Path::new("inventory"))?;
    }

    let mut entries: Vec<EntryState> = inventory
        .as_ref()
        .map(|inventory| {
            inventory
                .entries
                .iter()
                .map(|entry| EntryState {
                    hex: entry.hex.clone(),
                    source: FileState::Absent,
                    dest: FileState::Absent,
                })
                .collect()
        })
        .unwrap_or_default();
    let by_hex: BTreeMap<String, usize> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.hex.clone(), index))
        .collect();
    let mut named = Named::default();
    let mut strays = Vec::new();

    for path in task_paths(&invocation.root.join("tasks"))? {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Parse {
                file: path.display().to_string(),
                detail: "filename is not valid UTF-8".into(),
            })?;
        let id =
            TaskId::parse(name.strip_suffix(".md").expect("task path has .md")).map_err(|_| {
                Error::Parse {
                    file: format!("tasks/{name}"),
                    detail: "filename is not a valid task id".into(),
                }
            })?;
        if id.prefix == invocation.source {
            named.source += 1;
        } else if id.prefix == invocation.target {
            named.target += 1;
        }

        let Some(index) = by_hex
            .get(id.hex.as_str())
            .copied()
            .filter(|_| id.prefix == invocation.source || id.prefix == invocation.target)
        else {
            if inventory.is_some() {
                strays.push(path);
            }
            continue;
        };
        let state = FileState::Present(digest(&std::fs::read(&path)?));
        if id.prefix == invocation.source {
            entries[index].source = state;
        } else {
            entries[index].dest = state;
        }
    }
    strays.sort();

    Ok(Snapshot {
        invocation: invocation.clone(),
        registry: RegistryState {
            old_key: registry.projects.get(&invocation.source).cloned(),
            new_key: registry.projects.get(&invocation.target).cloned(),
            alias: registry.aliases.get(&invocation.source).cloned(),
        },
        config: observe_config(invocation)?,
        inventory,
        entries,
        named,
        strays,
    })
}

fn observe_config(invocation: &Invocation) -> Result<Option<ConfigState>> {
    let path = invocation.root.join(CONFIG_REL);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
    let config: ConfigPrefix =
        toml::from_str(text).map_err(|error| Error::Config(format!("{CONFIG_REL}: {error}")))?;
    if !is_valid_prefix(&config.prefix) {
        return Err(Error::Config(format!(
            "{CONFIG_REL}: bad prefix {:?}",
            config.prefix
        )));
    }
    Ok(Some(ConfigState {
        prefix: config.prefix,
        digest: digest(&bytes),
    }))
}

fn validate_invocation(invocation: &Invocation) -> Result<()> {
    for prefix in [&invocation.source, &invocation.target] {
        if !is_valid_prefix(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rename::inventory::{Inventory, digest};
    use std::path::{Path, PathBuf};

    const MINIMAL_TASK: &str = "---\nid: dot-a00088\ntitle: T\nstatus: todo\npriority: 2\n\
created: 2026-09-01T00:00:00Z\nupdated: 2026-09-01T00:00:00Z\ndepends: []\ntags: []\n---\n";

    const TASK_WITH_PARENT: &str = "---\nid: dot-a00088\ntitle: T\nstatus: todo\npriority: 2\n\
created: 2026-09-01T00:00:00Z\nupdated: 2026-09-01T00:00:00Z\ndepends: []\n\
parent: dot-b11111\ntags: []\n---\n";

    fn invocation(project: &crate::repo::Project) -> Invocation {
        Invocation {
            source: "dot".into(),
            target: "dots".into(),
            root: project.root.clone(),
        }
    }

    #[test]
    fn observes_an_untouched_project_against_its_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        std::fs::write(project.tasks_dir().join("dot-a00088.md"), TASK_WITH_PARENT).unwrap();

        let inventory = Inventory::build(&project, "dots").unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(inventory.entries[0].hex, "a00088");
        assert_ne!(inventory.entries[0].from, inventory.entries[0].to);
        assert_eq!(
            inventory.entries[0].to,
            "554f553215c76a8dbd255549667021347314c9ed53a37eb004ae156d956c5613",
            "the inventory transformation includes the local parent rewrite"
        );

        let snap = observe(
            &crate::registry::Registry::default(),
            &invocation(&project),
            Some(inventory.clone()),
        )
        .unwrap();
        assert_eq!(snap.named.source, 1);
        assert_eq!(snap.named.target, 0);
        assert!(snap.strays.is_empty());
        assert!(matches!(
            snap.entries[0].source,
            FileState::Present(ref d) if *d == inventory.entries[0].from
        ));
        assert!(matches!(snap.entries[0].dest, FileState::Absent));
    }

    #[test]
    fn a_task_file_outside_the_inventory_is_a_stray() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let inventory = Inventory::build(&project, "dots").unwrap();
        std::fs::write(project.tasks_dir().join("dot-ffffff.md"), "---\n").unwrap();
        let snap = observe(
            &crate::registry::Registry::default(),
            &invocation(&project),
            Some(inventory),
        )
        .unwrap();
        assert_eq!(snap.strays, [project.tasks_dir().join("dot-ffffff.md")]);
    }

    #[test]
    fn observing_without_an_inventory_still_counts_files_by_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        std::fs::write(project.tasks_dir().join("dot-a00088.md"), MINIMAL_TASK).unwrap();
        let snap = observe(
            &crate::registry::Registry::default(),
            &invocation(&project),
            None,
        )
        .unwrap();
        assert!(snap.inventory.is_none());
        assert!(snap.entries.is_empty(), "no baseline means no entries");
        assert_eq!(snap.named.source, 1);
        assert_eq!(snap.named.target, 0);
        assert!(
            snap.strays.is_empty(),
            "without a baseline nothing is a stray"
        );
    }

    #[test]
    fn observes_registry_config_and_absent_config_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.projects.insert("dot".into(), project.root.clone());
        registry
            .projects
            .insert("dots".into(), PathBuf::from("/tmp/foreign"));
        registry.aliases.insert("dot".into(), "dots".into());

        let snap = observe(&registry, &invocation(&project), None).unwrap();
        assert_eq!(snap.registry.old_key, Some(project.root.clone()));
        assert_eq!(snap.registry.new_key, Some(PathBuf::from("/tmp/foreign")));
        assert_eq!(snap.registry.alias.as_deref(), Some("dots"));
        assert_eq!(snap.config.as_ref().unwrap().prefix, "dot");
        assert_eq!(
            snap.config.as_ref().unwrap().digest,
            digest(&std::fs::read(project.root.join(crate::repo::CONFIG_REL)).unwrap())
        );

        std::fs::remove_file(project.root.join(crate::repo::CONFIG_REL)).unwrap();
        assert!(
            observe(&registry, &invocation(&project), None)
                .unwrap()
                .config
                .is_none()
        );
    }

    #[test]
    fn inventory_roundtrips_and_pending_rejects_a_traversal_name() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let inventory = Inventory::build(&project, "dots").unwrap();
        let state = dir.path().join("state/tasks/rename");
        let path = state.join("dot.toml");

        inventory.save_to(&path).unwrap();
        assert_eq!(
            Inventory::load_from(&path).unwrap(),
            Some(inventory.clone())
        );
        assert_eq!(
            Inventory::pending_in(&state).unwrap(),
            std::slice::from_ref(&inventory)
        );
        Inventory::remove_from(&path).unwrap();
        assert_eq!(Inventory::load_from(&path).unwrap(), None);
        Inventory::remove_from(&path).unwrap();

        std::fs::write(
            state.join("bad.toml"),
            "source = \"../bad\"\ntarget = \"dots\"\nroot = \"/tmp/x\"\nconfig_from = \"00\"\nconfig_to = \"00\"\nentries = []\n",
        )
        .unwrap();
        assert_eq!(Inventory::pending_in(&state).unwrap_err().kind(), "config");
    }

    #[test]
    fn state_path_uses_xdg_then_home_and_rejects_invalid_names() {
        let xdg = |key: &str| (key == "XDG_STATE_HOME").then(|| "/state".into());
        assert_eq!(
            Inventory::path_with("dot", xdg).unwrap(),
            Path::new("/state/tasks/rename/dot.toml")
        );
        let home = |key: &str| (key == "HOME").then(|| "/home/person".into());
        assert_eq!(
            Inventory::path_with("dot", home).unwrap(),
            Path::new("/home/person/.local/state/tasks/rename/dot.toml")
        );
        assert_eq!(
            Inventory::path_with("../dot", |_| None).unwrap_err().kind(),
            "config"
        );
    }

    #[test]
    fn sha256_digest_is_lowercase_hex() {
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(unix)]
    #[test]
    fn invalid_utf8_task_filename_is_not_skipped() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let name = std::ffi::OsString::from_vec(vec![0xff, b'.', b'm', b'd']);
        std::fs::write(project.tasks_dir().join(name), "---\n").unwrap();

        let error = Inventory::build(&project, "dots").unwrap_err();
        assert_eq!(error.kind(), "parse");
        assert!(error.to_string().contains("valid UTF-8"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn hidden_invalid_utf8_markdown_filename_is_ignored() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let name = std::ffi::OsString::from_vec(vec![b'.', 0xff, b'.', b'm', b'd']);
        std::fs::write(project.tasks_dir().join(name), "---\n").unwrap();

        let inventory = Inventory::build(&project, "dots").unwrap();
        assert!(inventory.entries.is_empty());
    }
}
