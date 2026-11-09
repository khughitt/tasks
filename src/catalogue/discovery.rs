use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use crate::scope::{Origin, config_dir, open_registered_config};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum EntryState {
    Read(Box<Project>),
    Error(String),
}

pub struct Catalogue {
    pub entries: BTreeMap<String, EntryState>,
    pub stores: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

impl Catalogue {
    pub fn build(registry: &Registry) -> Catalogue {
        let mut catalogue = Catalogue {
            entries: BTreeMap::new(),
            stores: Vec::new(),
            warnings: Vec::new(),
        };
        let mut stores = BTreeSet::new();
        for prefix in registry.projects.keys() {
            let dir = config_dir(registry, prefix).expect("registered config directory");
            if let Some(parent) = dir.parent()
                && let Ok(parent) = parent.canonicalize()
                && (parent.join(".git").is_file() || parent.join(".git").is_dir())
            {
                stores.insert(parent);
            }
            let entry = match open_registered_config(registry, prefix, Origin::Prefix) {
                Ok(project) => EntryState::Read(Box::new(project)),
                Err(error) => EntryState::Error(format!("{}: {error}", dir.display())),
            };
            catalogue.entries.insert(prefix.clone(), entry);
        }
        catalogue.stores = stores.into_iter().collect();
        let mut discovered: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for store in &catalogue.stores {
            let children = match std::fs::read_dir(store) {
                Ok(children) => children,
                Err(error) => {
                    catalogue
                        .warnings
                        .push(format!("{}: {error}", store.display()));
                    continue;
                }
            };
            for child in children {
                let child = match child {
                    Ok(child) => child,
                    Err(error) => {
                        catalogue
                            .warnings
                            .push(format!("{}: {error}", store.display()));
                        continue;
                    }
                };
                let path = child.path();
                match std::fs::metadata(path.join(CONFIG_REL)) {
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) =>
                    {
                        continue;
                    }
                    _ => {}
                }
                discovered
                    .entry(child.file_name().to_string_lossy().into_owned())
                    .or_default()
                    .push(path);
            }
        }
        for (prefix, mut paths) in discovered {
            paths.sort();
            if let Some(registered) = config_dir(registry, &prefix) {
                for path in paths {
                    let same = path == registered
                        || path
                            .canonicalize()
                            .ok()
                            .zip(registered.canonicalize().ok())
                            .is_some_and(|(path, registered)| path == registered);
                    if !same {
                        catalogue.warnings.push(format!(
                            "entry_conflict: {prefix}: registered entry {} wins over {}; re-pair with `tasks init --prefix {prefix} --records {} --force` in the checkout",
                            registered.display(), path.display(), path.parent().expect("store child").display()
                        ));
                    }
                }
                continue;
            }
            let entry = if paths.len() > 1 {
                let detail = format!(
                    "{prefix}: multiple records entries: {}",
                    paths
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                catalogue.warnings.push(detail.clone());
                EntryState::Error(detail)
            } else {
                let path = &paths[0];
                match Project::open(path) {
                    Ok(project) if project.prefix == prefix => EntryState::Read(Box::new(project)),
                    Ok(project) => EntryState::Error(format!(
                        "{}: directory prefix {prefix:?} disagrees with config prefix {:?}",
                        path.display(),
                        project.prefix
                    )),
                    Err(error) => EntryState::Error(format!("{}: {error}", path.display())),
                }
            };
            catalogue.entries.insert(prefix, entry);
        }
        catalogue
    }

    pub fn store_of(&self, path: &Path) -> Option<&Path> {
        let path = path.canonicalize().ok()?;
        self.stores
            .iter()
            .find(|store| path.starts_with(store))
            .map(PathBuf::as_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::CONFIG_REL;
    use std::fs;
    use tempfile::TempDir;

    fn store(parent: &Path, name: &str) -> PathBuf {
        let path = parent.join(name);
        fs::create_dir_all(path.join(".git")).unwrap();
        path.canonicalize().unwrap()
    }

    fn entry(store: &Path, directory: &str, prefix: &str) -> PathBuf {
        let path = store.join(directory);
        fs::create_dir_all(path.join("tasks")).unwrap();
        fs::write(path.join(CONFIG_REL), format!("prefix = {prefix:?}\n")).unwrap();
        path
    }

    fn register(registry: &mut Registry, prefix: &str, path: &Path) {
        registry.projects.insert(prefix.into(), path.into());
    }

    fn read<'a>(catalogue: &'a Catalogue, prefix: &str) -> &'a Project {
        match &catalogue.entries[prefix] {
            EntryState::Read(project) => project,
            other => panic!("expected readable entry: {other:?}"),
        }
    }

    fn error<'a>(catalogue: &'a Catalogue, prefix: &str) -> &'a str {
        match &catalogue.entries[prefix] {
            EntryState::Error(error) => error,
            other => panic!("expected entry error: {other:?}"),
        }
    }

    #[test]
    fn stores_are_parents_holding_git() {
        let temp = TempDir::new().unwrap();
        let first = store(temp.path(), "first");
        let second = temp.path().join("second");
        fs::create_dir(&second).unwrap();
        fs::write(second.join(".git"), "gitdir: elsewhere").unwrap();
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&first, "aa", "aa"));
        register(&mut registry, "bb", &entry(&first, "bb", "bb"));
        register(&mut registry, "cc", &entry(&second, "cc", "cc"));
        let catalogue = Catalogue::build(&registry);
        assert_eq!(
            catalogue.stores,
            vec![first, second.canonicalize().unwrap()]
        );
        assert_eq!(catalogue.entries.len(), 3);
        assert!(catalogue.warnings.is_empty());
    }

    #[test]
    fn children_without_a_config_are_skipped() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let registered = entry(&store, "aa", "aa");
        fs::write(store.join("README.md"), "Records").unwrap();
        fs::create_dir(store.join("empty")).unwrap();
        std::os::unix::fs::symlink(temp.path(), store.join(".worktrees")).unwrap();
        let mut registry = Registry::default();
        register(&mut registry, "aa", &registered);
        let catalogue = Catalogue::build(&registry);
        assert_eq!(catalogue.entries.keys().collect::<Vec<_>>(), vec!["aa"]);
        assert!(catalogue.warnings.is_empty());
    }

    #[test]
    fn an_unregistered_entry_is_catalogued() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        let unregistered = entry(&store, "bb", "bb");
        let catalogue = Catalogue::build(&registry);
        assert_eq!(read(&catalogue, "bb").records, unregistered);
        assert_eq!(read(&catalogue, "bb").prefix, "bb");
        assert!(catalogue.warnings.is_empty());
    }

    #[test]
    fn directory_name_must_equal_prefix() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        entry(&store, "bb", "wrong");
        let catalogue = Catalogue::build(&registry);
        let error = error(&catalogue, "bb");
        assert!(error.contains("wrong") && error.contains("bb"), "{error}");
        assert!(!catalogue.entries.contains_key("wrong"));
    }

    #[test]
    fn registered_entry_wins_with_entry_conflict() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        let registered = entry(temp.path(), "bb", "bb");
        register(&mut registry, "bb", &registered);
        let duplicate = entry(&store, "bb", "bb");
        let catalogue = Catalogue::build(&registry);
        assert_eq!(read(&catalogue, "bb").records, registered);
        assert_eq!(catalogue.warnings.len(), 1);
        let warning = &catalogue.warnings[0];
        for text in [
            "entry_conflict",
            "tasks init",
            registered.to_str().unwrap(),
            duplicate.to_str().unwrap(),
        ] {
            assert!(warning.contains(text), "{warning}");
        }
    }

    #[test]
    fn unreadable_registered_entry_is_an_error_not_the_store_copy() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        let registered = temp.path().join("missing");
        register(&mut registry, "bb", &registered);
        entry(&store, "bb", "bb");
        let catalogue = Catalogue::build(&registry);
        assert!(error(&catalogue, "bb").contains(registered.to_str().unwrap()));
        assert!(
            catalogue
                .warnings
                .iter()
                .any(|warning| warning.contains("entry_conflict"))
        );
    }

    #[test]
    fn unregistered_prefix_in_two_stores_is_an_error() {
        let temp = TempDir::new().unwrap();
        let first = store(temp.path(), "first");
        let second = store(temp.path(), "second");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&first, "aa", "aa"));
        register(&mut registry, "cc", &entry(&second, "cc", "cc"));
        let copies = [entry(&first, "bb", "bb"), entry(&second, "bb", "bb")];
        let catalogue = Catalogue::build(&registry);
        let error = error(&catalogue, "bb");
        assert_eq!(catalogue.warnings.len(), 1);
        for copy in copies {
            assert!(error.contains(copy.to_str().unwrap()), "{error}");
            assert!(catalogue.warnings[0].contains(copy.to_str().unwrap()));
        }
        assert_eq!(catalogue.entries.len(), 3);
    }

    #[test]
    fn store_of_matches_through_a_symlinked_alias() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&store, &alias).unwrap();
        let catalogue = Catalogue::build(&registry);
        assert_eq!(
            catalogue.store_of(&alias.join("aa/tasks")),
            Some(store.as_path())
        );
        assert_eq!(catalogue.store_of(&alias), Some(store.as_path()));
        assert_eq!(catalogue.store_of(temp.path()), None);
        assert_eq!(catalogue.store_of(&alias.join("missing")), None);
    }

    #[test]
    fn in_tree_repository_is_not_a_store() {
        let temp = TempDir::new().unwrap();
        let root = store(temp.path(), "code");
        entry(temp.path(), "code", "aa");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &root);
        let catalogue = Catalogue::build(&registry);
        assert!(catalogue.stores.is_empty());
        assert_eq!(read(&catalogue, "aa").records, root);
        assert!(catalogue.warnings.is_empty());
    }

    #[test]
    fn malformed_entry_does_not_hide_other_projects() {
        let temp = TempDir::new().unwrap();
        let store = store(temp.path(), "store");
        let mut registry = Registry::default();
        register(&mut registry, "aa", &entry(&store, "aa", "aa"));
        let broken = entry(&store, "bb", "bb");
        fs::write(broken.join(CONFIG_REL), "prefix = 1").unwrap();
        let catalogue = Catalogue::build(&registry);
        assert!(!error(&catalogue, "bb").is_empty());
        assert_eq!(read(&catalogue, "aa").prefix, "aa");
    }
}
