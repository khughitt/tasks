use crate::error::{Error, Result};
use crate::model::{TaskId, is_valid_prefix};
use crate::registry::Registry;
use crate::rename::inventory::{Inventory, digest, task_paths};
use crate::repo::CONFIG_REL;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A rename keeps two paths apart: `root` is the project's identity, the registered root
/// `[projects]` holds (the code checkout of a detached project), and `records` is where
/// `tasks/` and the config are now, which a detached rename moves (§5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub source: String,
    pub target: String,
    pub root: PathBuf,
    pub records: PathBuf,
}

/// Which of a detached inventory's `records_from` and `records_to` exist. `Old` for every
/// project whose records do not move, and before an inventory exists while the current
/// records directory does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordsDir {
    Old,
    New,
    Both,
    Neither,
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
    pub dirs: AttachmentDirs,
}

/// Which of `tasks/files/<source>-<hex>` and `tasks/files/<target>-<hex>` are directories,
/// read without following a symlink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttachmentDirs {
    pub source: bool,
    pub dest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryState {
    pub old_key: Option<PathBuf>,
    pub new_key: Option<PathBuf>,
    pub alias: Option<String>,
    /// The target names a group in the registry: refusal R12, in every stage.
    pub target_group: bool,
    /// `[records]` pairs the source with the inventory's `records_from` (both absent
    /// counts as agreeing).
    pub records_old: bool,
    /// `[records]` pairs the target with the inventory's `records_to`, likewise.
    pub records_new: bool,
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
    pub target_dirs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub invocation: Invocation,
    pub registry: RegistryState,
    pub records: RecordsDir,
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
    let inventory = inventory
        .map(|mut inventory| {
            inventory.root = super::root_identity(&inventory.root)?;
            Ok::<_, Error>(inventory)
        })
        .transpose()?;
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
                    dirs: AttachmentDirs::default(),
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
    let records = observe_records(registry, invocation, inventory.as_ref())?;
    // With neither directory there is nothing to read; the classifier refuses.
    let tasks = (records != RecordsDir::Neither)
        .then(|| task_paths(&invocation.records.join("tasks")))
        .transpose()?
        .unwrap_or_default();

    for path in tasks {
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
    let files_root = invocation
        .records
        .join("tasks")
        .join(crate::attachments::FILES_DIR);
    match crate::attachments::dir_state(&files_root)? {
        crate::attachments::DirState::Absent => {}
        crate::attachments::DirState::Unsafe(detail) => {
            return Err(Error::AttachmentUnsafe(detail));
        }
        crate::attachments::DirState::Directory => {
            for entry in std::fs::read_dir(&files_root)? {
                let entry = entry?;
                // `DirEntry::file_type` does not follow a symlink.
                let is_dir = entry.file_type()?.is_dir();
                let id = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| TaskId::parse(name).ok())
                    .filter(|_| is_dir);
                if let Some(id) = &id
                    && id.prefix == invocation.target
                {
                    named.target_dirs += 1;
                }
                let index = id
                    .as_ref()
                    .filter(|id| id.prefix == invocation.source || id.prefix == invocation.target)
                    .and_then(|id| by_hex.get(id.hex.as_str()).copied());
                match (index, id) {
                    (Some(index), Some(id)) if id.prefix == invocation.source => {
                        entries[index].dirs.source = true;
                    }
                    (Some(index), Some(_)) => entries[index].dirs.dest = true,
                    _ if inventory.is_some() => strays.push(entry.path()),
                    _ => {}
                }
            }
        }
    }
    strays.sort();
    let records_from = inventory
        .as_ref()
        .and_then(|inventory| inventory.records_from.as_deref());
    let records_to = inventory
        .as_ref()
        .and_then(|inventory| inventory.records_to.as_deref());

    Ok(Snapshot {
        invocation: invocation.clone(),
        registry: RegistryState {
            old_key: registry
                .projects
                .get(&invocation.source)
                .map(|root| super::root_identity(root))
                .transpose()?,
            new_key: registry
                .projects
                .get(&invocation.target)
                .map(|root| super::root_identity(root))
                .transpose()?,
            alias: registry.aliases.get(&invocation.source).cloned(),
            target_group: registry.groups.contains_key(&invocation.target),
            records_old: same_dir(registry.records_dir(&invocation.source), records_from)?,
            records_new: same_dir(registry.records_dir(&invocation.target), records_to)?,
        },
        config: if records == RecordsDir::Neither {
            None
        } else {
            observe_config(invocation)?
        },
        records,
        inventory,
        entries,
        named,
        strays,
    })
}

/// `RecordsDir` from the existence of the inventory's two directories. Before an
/// inventory, a detached project (one `[records]` pairs) is `Both` when the directory it
/// would move to is already taken, as a destination task file refuses a fresh rename.
fn observe_records(
    registry: &Registry,
    invocation: &Invocation,
    inventory: Option<&Inventory>,
) -> Result<RecordsDir> {
    let (from, to) = match inventory {
        Some(inventory) => match (&inventory.records_from, &inventory.records_to) {
            (Some(from), Some(to)) => (from.clone(), to.clone()),
            _ => return Ok(RecordsDir::Old),
        },
        None => {
            let live = registry.canonical_prefix(&invocation.source);
            if registry.records_dir(live).is_none() {
                return Ok(RecordsDir::Old);
            }
            let to = invocation
                .records
                .parent()
                .map(|parent| parent.join(&invocation.target))
                .unwrap_or_else(|| invocation.records.clone());
            (invocation.records.clone(), to)
        }
    };
    Ok(match (present(&from)?, from != to && present(&to)?) {
        (true, false) => RecordsDir::Old,
        (false, true) => RecordsDir::New,
        (true, true) => RecordsDir::Both,
        (false, false) => RecordsDir::Neither,
    })
}

/// Whether anything is at `path`, a symlink included.
pub(crate) fn present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Two optional directories compared by identity; a missing directory compares by its
/// parent's identity and its name, so a moved directory still matches its old spelling.
fn same_dir(left: Option<&Path>, right: Option<&Path>) -> Result<bool> {
    Ok(match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => super::dir_identity(left)? == super::dir_identity(right)?,
        _ => false,
    })
}

fn observe_config(invocation: &Invocation) -> Result<Option<ConfigState>> {
    let path = invocation.records.join(CONFIG_REL);
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
            records: project.records.clone(),
        }
    }

    #[test]
    fn observes_an_untouched_project_against_its_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
    fn observes_whether_the_target_names_a_group() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.projects.insert("dot".into(), project.root.clone());
        let observed = |registry: &crate::registry::Registry| {
            observe(registry, &invocation(&project), None)
                .unwrap()
                .registry
                .target_group
        };
        assert!(!observed(&registry));
        registry.groups.insert("dot-set".into(), vec!["dot".into()]);
        assert!(
            !observed(&registry),
            "only a group named like the target counts"
        );
        registry.groups.insert("dots".into(), vec!["dot".into()]);
        assert!(observed(&registry));
    }

    #[test]
    fn inventory_roundtrips_and_pending_rejects_a_traversal_name() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
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
        let project = crate::repo::Project::init(dir.path(), "dot", None).unwrap();
        let name = std::ffi::OsString::from_vec(vec![b'.', 0xff, b'.', b'm', b'd']);
        std::fs::write(project.tasks_dir().join(name), "---\n").unwrap();

        let inventory = Inventory::build(&project, "dots").unwrap();
        assert!(inventory.entries.is_empty());
    }
}
