//! What this host has not taken up from its records stores: discovered entries it has not
//! registered, and renames another host made that this registry has not adopted
//! (docs/project-operations.md#taking-up-synced-records).
use super::discovery::{Catalogue, EntryState};
use crate::registry::Registry;
use crate::repo::{Kind, Project};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One unregistered entry `tasks init` can pair on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub prefix: String,
    pub kind: Kind,
    pub records: PathBuf,
    pub checkout: PathBuf,
    /// Where init runs: the code checkout for a detached entry, the records directory
    /// otherwise.
    pub dir: PathBuf,
    /// `--records <store>` for a detached entry, `--checkout <code>` for a linked one.
    pub records_root: Option<PathBuf>,
    pub linked_checkout: Option<PathBuf>,
}

impl Registration {
    fn of(project: &Project) -> Registration {
        let (dir, records_root, linked_checkout) = match project.kind {
            Kind::Detached => (
                project.checkout.clone(),
                project.records.parent().map(Path::to_path_buf),
                None,
            ),
            Kind::Linked => (
                project.records.clone(),
                None,
                Some(project.checkout.clone()),
            ),
            Kind::InTree => (project.records.clone(), None, None),
        };
        Registration {
            prefix: project.prefix.clone(),
            kind: project.kind,
            records: project.records.clone(),
            checkout: project.checkout.clone(),
            dir,
            records_root,
            linked_checkout,
        }
    }

    /// The exact command a person runs to register this entry by hand.
    pub fn command(&self) -> String {
        let mut words = vec![
            "tasks".to_string(),
            "-C".into(),
            self.dir.display().to_string(),
            "init".into(),
            "--prefix".into(),
            self.prefix.clone(),
        ];
        if let Some(root) = &self.records_root {
            words.extend(["--records".into(), root.display().to_string()]);
        }
        if let Some(checkout) = &self.linked_checkout {
            words.extend(["--checkout".into(), checkout.display().to_string()]);
        }
        shell_words(&words)
    }
}

/// One rename another host made that this host's registry still names by its old prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adoption {
    pub old: String,
    pub new: String,
    /// Where `rename --adopt` runs: the renamed project's checkout.
    pub dir: PathBuf,
}

impl Adoption {
    pub fn command(&self) -> String {
        shell_words(&[
            "tasks".into(),
            "-C".into(),
            self.dir.display().to_string(),
            "rename".into(),
            self.old.clone(),
            self.new.clone(),
            "--adopt".into(),
        ])
    }
}

/// Why a catalogued, unregistered entry is not a registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    Retired,
    CheckoutAbsent(PathBuf),
    RenameTarget(String),
    Unreadable(String),
}

impl std::fmt::Display for Skip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Skip::Retired => write!(
                f,
                "it is retired; register it with an explicit `tasks init`"
            ),
            Skip::CheckoutAbsent(path) => {
                write!(f, "its checkout {} is not on this host", path.display())
            }
            Skip::RenameTarget(old) => write!(
                f,
                "it is the new name of {old}, registered here; adopt the rename instead"
            ),
            Skip::Unreadable(detail) => write!(f, "its entry cannot be read: {detail}"),
        }
    }
}

pub struct Pending {
    pub registrations: Vec<Registration>,
    pub adoptions: Vec<Adoption>,
    /// Every other unregistered catalogued prefix, with the reason it is not registered.
    pub skipped: BTreeMap<String, Skip>,
}

impl Pending {
    pub fn find(registry: &Registry, catalogue: &Catalogue) -> Pending {
        let reopened = reopened(registry, catalogue);
        let entries: Vec<&Project> = catalogue
            .entries
            .values()
            .filter_map(|state| match state {
                EntryState::Read(project) => Some(project.as_ref()),
                EntryState::Error(_) => None,
            })
            .chain(&reopened)
            .collect();
        let adoptions = adoptions(registry, catalogue, &entries);
        let mut registrations = Vec::new();
        let mut skipped = BTreeMap::new();
        for (prefix, state) in &catalogue.entries {
            if registry.projects.contains_key(prefix) {
                continue;
            }
            let project = match state {
                EntryState::Read(project) => project,
                EntryState::Error(detail) => {
                    skipped.insert(prefix.clone(), Skip::Unreadable(detail.clone()));
                    continue;
                }
            };
            let skip = if let Some(a) = adoptions.iter().find(|a| &a.new == prefix) {
                Some(Skip::RenameTarget(a.old.clone()))
            } else if project.retired.is_some() {
                Some(Skip::Retired)
            } else if !project.checkout.is_dir() {
                Some(Skip::CheckoutAbsent(project.checkout.clone()))
            } else {
                None
            };
            match skip {
                Some(skip) => {
                    skipped.insert(prefix.clone(), skip);
                }
                None => registrations.push(Registration::of(project)),
            }
        }
        Pending {
            registrations,
            adoptions,
            skipped,
        }
    }
}

/// The raw config at each registered prefix's config directory whose registered open
/// failed: a prefix renamed in place, or a pairing that no longer matches, still names its
/// new prefix there. With the catalogue's readable entries these are every entry on disk.
fn reopened(registry: &Registry, catalogue: &Catalogue) -> Vec<Project> {
    catalogue
        .entries
        .iter()
        .filter(|(prefix, state)| {
            matches!(state, EntryState::Error(_)) && registry.projects.contains_key(*prefix)
        })
        .filter_map(|(prefix, _)| crate::scope::config_dir(registry, prefix))
        .filter_map(|dir| Project::open(&dir).ok())
        .collect()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Where a project's own commands run: the code checkout of a detached project, the
/// registered root (records directory) otherwise.
fn run_dir(project: &Project) -> PathBuf {
    match project.kind {
        Kind::Detached => project.checkout.clone(),
        Kind::InTree | Kind::Linked => project.records.clone(),
    }
}

fn adoptions(registry: &Registry, catalogue: &Catalogue, entries: &[&Project]) -> Vec<Adoption> {
    let mut found: Vec<Adoption> = Vec::new();
    let mut push = |adoption: Adoption| {
        if !found
            .iter()
            .any(|a| a.old == adoption.old && a.new == adoption.new)
        {
            found.push(adoption);
        }
    };
    // A registered prefix that no longer reads as itself, and an entry under another
    // prefix at its registered root: the records moved to the new prefix (detached), or
    // the config there names it (in-tree, linked).
    for (old, root) in &registry.projects {
        if !matches!(catalogue.entries.get(old), Some(EntryState::Error(_))) {
            continue;
        }
        let root = canonical(root);
        for entry in entries {
            if entry.prefix == *old
                || registry
                    .project_root(&entry.prefix)
                    .is_some_and(|r| canonical(r) == root)
            {
                continue;
            }
            if canonical(&entry.root) == root || canonical(&entry.checkout) == root {
                push(Adoption {
                    old: old.clone(),
                    new: entry.prefix.clone(),
                    dir: run_dir(entry),
                });
            }
        }
    }
    // A prefix handoff (docs/project-operations.md#reclaiming-a-retired-predecessors-prefix)
    // this host registered before cutover: its predecessor table is not imported yet, and
    // it still registers the successor under its former prefix. The predecessor's rename
    // is adopted first.
    for entry in entries {
        let Some(pred) = &entry.predecessor else {
            continue;
        };
        let live = &entry.prefix;
        if registry.predecessors.get(live) == Some(pred)
            || !registry.projects.contains_key(&pred.successor_alias)
        {
            continue;
        }
        if registry.projects.contains_key(live)
            && !registry.projects.contains_key(&pred.prefix)
            && let Some(predecessor) = entries.iter().find(|e| e.prefix == pred.prefix)
        {
            push(Adoption {
                old: live.clone(),
                new: pred.prefix.clone(),
                dir: run_dir(predecessor),
            });
        }
        push(Adoption {
            old: pred.successor_alias.clone(),
            new: live.clone(),
            dir: run_dir(entry),
        });
    }
    // A rename onto a prefix still registered here waits for that prefix's own rename.
    found.sort_by_key(|a| registry.projects.contains_key(&a.new));
    found
}

fn shell_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word| crate::stale::quote(word))
        .collect::<Vec<_>>()
        .join(" ")
}
