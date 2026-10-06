//! Per-host location history and path resolution
//! (docs/specs/2026-10-06-project-resolution-design.md §2, §4).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One project's history on this host. `storage` is the record of where the current
/// root's `.worktrees` resolved when a command last wrote this project's entry (§2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Locations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub former: Vec<Former>,
}

/// A root a command moved the project off, with the storage record kept for it: the
/// fallback when the root is gone and its link cannot be read (§2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Displaced {
    pub root: PathBuf,
    pub storage: Option<PathBuf>,
}

/// A location the project left. Identity is (`root`, `storage`): a root can recur with
/// other storage, or be current again, and no storage it used is ever dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Former {
    pub root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<PathBuf>,
    pub until: String,
}

impl Locations {
    pub fn is_empty(&self) -> bool {
        self.storage.is_none() && self.former.is_empty()
    }

    /// Adds `entry`; an existing (root, storage) pair only moves its `until` forward.
    pub fn push_former(&mut self, entry: Former) {
        match self
            .former
            .iter_mut()
            .find(|f| f.root == entry.root && f.storage == entry.storage)
        {
            Some(existing) => {
                if entry.until > existing.until {
                    existing.until = entry.until;
                }
            }
            None => self.former.push(entry),
        }
        self.former
            .sort_by(|a, b| (&a.until, &a.root).cmp(&(&b.until, &b.root)));
    }

    /// The adoption merge (§2.2): the union of both `former` lists. The storage record
    /// starts empty: neither table's record describes the checkout's root, and
    /// `Registry::note_root_change` refreshes it from the live link. The displaced
    /// table's record travels separately, in `Displaced`.
    pub fn merge(mut self, displaced: Locations) -> Locations {
        for entry in displaced.former {
            self.push_former(entry);
        }
        self.storage = None;
        self
    }

    /// What load validation refuses (§2), or `None`.
    pub fn problem(&self) -> Option<String> {
        let absolute = |what: &str, path: &std::path::Path| {
            (!path.is_absolute())
                .then(|| format!("{what} {} must be an absolute path", path.display()))
        };
        if let Some(storage) = &self.storage
            && let Some(problem) = absolute("storage", storage)
        {
            return Some(problem);
        }
        for (i, entry) in self.former.iter().enumerate() {
            if let Some(problem) = absolute("former root", &entry.root) {
                return Some(problem);
            }
            if let Some(storage) = &entry.storage
                && let Some(problem) = absolute("former storage", storage)
            {
                return Some(problem);
            }
            if entry.until.len() != 10 || crate::time::calendar_day(&entry.until).is_err() {
                return Some(format!(
                    "former root {} has until {:?}; write YYYY-MM-DD",
                    entry.root.display(),
                    entry.until
                ));
            }
            if self.former[..i]
                .iter()
                .any(|f| f.root == entry.root && f.storage == entry.storage)
            {
                return Some(format!(
                    "former root {} is listed twice with the same storage",
                    entry.root.display()
                ));
            }
        }
        None
    }
}

/// What a root's `.worktrees` is on this host right now (§2.1, §4.2).
#[derive(Debug, Clone, PartialEq)]
pub enum StorageState {
    /// A symlink that resolves: the storage path, canonical.
    Link(PathBuf),
    /// A symlink whose target is missing.
    Dangling,
    /// A real directory inside the root, already covered by the root.
    InTree,
    /// No `.worktrees`, or no root.
    Absent,
}

pub fn storage_state(root: &std::path::Path) -> std::io::Result<StorageState> {
    let link = root.join(".worktrees");
    let meta = match std::fs::symlink_metadata(&link) {
        Ok(meta) => meta,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(StorageState::Absent);
        }
        Err(e) => return Err(e),
    };
    if !meta.file_type().is_symlink() {
        return Ok(StorageState::InTree);
    }
    match link.canonicalize() {
        Ok(real) => Ok(StorageState::Link(real)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StorageState::Dangling),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn former(root: &str, storage: Option<&str>, until: &str) -> Former {
        Former {
            root: root.into(),
            storage: storage.map(PathBuf::from),
            until: until.into(),
        }
    }

    #[test]
    fn push_former_keys_entries_by_root_and_storage() {
        let mut l = Locations::default();
        l.push_former(former("/a", Some("/s1"), "2026-01-01"));
        l.push_former(former("/b", None, "2026-02-01"));
        l.push_former(former("/a", Some("/s2"), "2026-03-01"));
        l.push_former(former("/a", Some("/s1"), "2026-04-01"));
        assert_eq!(
            l.former,
            [
                former("/b", None, "2026-02-01"),
                former("/a", Some("/s2"), "2026-03-01"),
                former("/a", Some("/s1"), "2026-04-01"),
            ],
            "the same pair moves its until forward; another storage is another entry"
        );
    }

    #[test]
    fn merge_unions_both_lists_and_copies_no_record() {
        let mut target = Locations {
            storage: Some("/target-store".into()),
            former: vec![former("/x", None, "2026-01-01")],
        };
        target.push_former(former("/shared", Some("/s"), "2026-02-01"));
        let old = Locations {
            storage: Some("/old-store".into()),
            former: vec![
                former("/shared", Some("/s"), "2026-05-01"),
                former("/y", Some("/ys"), "2026-03-01"),
            ],
        };
        let merged = target.merge(old);
        assert_eq!(
            merged.storage, None,
            "neither table's record is copied (spec §2.2)"
        );
        assert_eq!(
            merged.former,
            [
                former("/x", None, "2026-01-01"),
                former("/y", Some("/ys"), "2026-03-01"),
                former("/shared", Some("/s"), "2026-05-01"),
            ]
        );
    }

    #[test]
    fn problem_names_each_invalid_entry() {
        for (l, word) in [
            (
                Locations {
                    storage: Some("rel".into()),
                    former: vec![],
                },
                "absolute",
            ),
            (
                Locations {
                    storage: None,
                    former: vec![former("~/d/ai", None, "2026-01-01")],
                },
                "absolute",
            ),
            (
                Locations {
                    storage: None,
                    former: vec![former("/a", Some("s"), "2026-01-01")],
                },
                "absolute",
            ),
            (
                Locations {
                    storage: None,
                    former: vec![former("/a", None, "2026-1-1")],
                },
                "YYYY-MM-DD",
            ),
            (
                Locations {
                    storage: None,
                    former: vec![
                        former("/a", None, "2026-01-01"),
                        former("/a", None, "2026-02-01"),
                    ],
                },
                "twice",
            ),
        ] {
            let problem = l.problem().expect("invalid");
            assert!(problem.contains(word), "{problem}");
        }
        assert_eq!(Locations::default().problem(), None);
    }
}
