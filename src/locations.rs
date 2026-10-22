//! Per-host location history and path resolution
//! (docs/specs/2026-10-06-project-resolution-design.md §2, §4).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One registered project that declares a `[checkout]` (linked-checkouts §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub prefix: String,
    pub checkout: PathBuf,
}

/// Every registered project on this host that declares a checkout, the color of every one
/// that sets one, and every registered project whose config could not be read. A project
/// whose root is gone or holds no config is in none: it cannot be opened here, so nothing
/// can be linked to it and it has no color to report.
#[derive(Debug, Default)]
pub struct CheckoutScan {
    pub linked: Vec<Linked>,
    pub colors: BTreeMap<String, crate::palette::Rgb>,
    pub failed: Vec<(String, crate::error::Error)>,
}

impl CheckoutScan {
    /// The linked projects, or the first failure, naming its project (§4.1a: errors stay
    /// errors for every caller that opens a project).
    pub fn strict(self) -> crate::error::Result<Vec<Linked>> {
        match self.failed.into_iter().next() {
            Some((prefix, error)) => {
                Err(error.with_suffix(&format!(" (registered project {prefix})")))
            }
            None => Ok(self.linked),
        }
    }
}

pub fn scan_checkouts(registry: &crate::registry::Registry) -> CheckoutScan {
    let mut scan = CheckoutScan::default();
    for (prefix, root) in &registry.projects {
        let opened = crate::scope::is_reachable(root).and_then(|reachable| {
            reachable
                .then(|| {
                    crate::scope::open_registered(registry, prefix, crate::scope::Origin::Prefix)
                })
                .transpose()
        });
        match opened {
            Ok(Some(project)) => {
                if let Some(color) = project.color {
                    scan.colors.insert(prefix.clone(), color);
                }
                if project.kind != crate::repo::Kind::InTree {
                    scan.linked.push(Linked {
                        prefix: prefix.clone(),
                        checkout: project.checkout,
                    });
                }
            }
            Ok(None) => {}
            Err(error) => scan.failed.push((prefix.clone(), error)),
        }
    }
    scan
}

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

/// Absolute, with the longest existing ancestor canonicalized and the rest appended
/// lexically (§4.1). `..` after a missing component cannot be resolved and is refused.
pub fn normalize(path: &Path, base: &Path) -> std::result::Result<PathBuf, String> {
    use std::path::Component;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut out = PathBuf::from("/");
    let mut missing = false;
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir if missing => {
                return Err(format!(
                    "{}: `..` follows a component that does not exist",
                    path.display()
                ));
            }
            // `out` is canonical while nothing is missing, so its lexical parent is real.
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => {
                out.push(name);
                if !missing {
                    match out.canonicalize() {
                        Ok(real) => out = real,
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                            ) =>
                        {
                            missing = true
                        }
                        Err(e) => return Err(format!("{}: {e}", out.display())),
                    }
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Root,
    Storage,
    Checkout,
    FormerRoot,
    FormerStorage,
    Git,
}

impl Via {
    fn current(self) -> bool {
        matches!(self, Via::Root | Via::Storage | Via::Checkout)
    }
    fn storage(self) -> bool {
        matches!(self, Via::Storage | Via::FormerStorage)
    }
}

#[derive(Debug)]
struct Location {
    prefix: String,
    via: Via,
    path: PathBuf,
}

#[derive(Debug, PartialEq)]
pub struct Match {
    pub prefix: String,
    pub via: Via,
    pub location: PathBuf,
    pub worktree: Option<PathBuf>,
    pub relpath: PathBuf,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub prefix: String,
    pub via: Via,
    pub location: String,
}

#[derive(Debug, PartialEq)]
pub enum Answer {
    Resolved(Match),
    Ambiguous(Vec<Candidate>),
    Unregistered,
}

/// Every registered project's locations on this host (§4.2), built once per call.
pub struct Index {
    locations: Vec<Location>,
    colors: BTreeMap<String, crate::palette::Rgb>,
    pub warnings: Vec<String>,
}

impl Index {
    pub fn build(registry: &crate::registry::Registry) -> Index {
        let mut locations = Vec::new();
        let mut warnings = Vec::new();
        let mut add = |prefix: &str, via: Via, path: &Path, warnings: &mut Vec<String>| {
            let path = normalize(path, Path::new("/")).unwrap_or_else(|problem| {
                warnings.push(format!("{prefix}: {problem}; matched as written"));
                path.to_path_buf()
            });
            locations.push(Location {
                prefix: prefix.into(),
                via,
                path,
            });
        };
        for (prefix, root) in &registry.projects {
            add(prefix, Via::Root, root, &mut warnings);
            let history = registry.locations.get(prefix);
            let record = history.and_then(|h| h.storage.as_deref());
            match storage_state(root) {
                Ok(StorageState::Link(real)) => add(prefix, Via::Storage, &real, &mut warnings),
                Ok(_) => {
                    if let Some(record) = record {
                        add(prefix, Via::Storage, record, &mut warnings);
                    }
                }
                Err(e) => {
                    warnings.push(format!(
                        "{prefix}: cannot read {}/.worktrees ({e}); using its storage record",
                        root.display()
                    ));
                    if let Some(record) = record {
                        add(prefix, Via::Storage, record, &mut warnings);
                    }
                }
            }
            for former in history.map(|h| h.former.as_slice()).unwrap_or_default() {
                add(prefix, Via::FormerRoot, &former.root, &mut warnings);
                if let Some(storage) = &former.storage {
                    add(prefix, Via::FormerStorage, storage, &mut warnings);
                }
            }
        }
        let scan = scan_checkouts(registry);
        for linked in scan.linked {
            add(
                &linked.prefix,
                Via::Checkout,
                &linked.checkout,
                &mut warnings,
            );
            // The code checkout's worktrees live where its own `.worktrees` points, so a
            // removed one still resolves while the link stands. There is no record to
            // fall back on: the registry's storage record describes the root.
            match storage_state(&linked.checkout) {
                Ok(StorageState::Link(real)) => {
                    add(&linked.prefix, Via::Storage, &real, &mut warnings)
                }
                Ok(_) => {}
                Err(e) => warnings.push(format!(
                    "{}: cannot read {}/.worktrees ({e}); its worktree storage is not matched",
                    linked.prefix,
                    linked.checkout.display()
                )),
            }
        }
        for (prefix, error) in scan.failed {
            warnings.push(format!(
                "{prefix}: its checkout is not matched and its color is not reported ({error})"
            ));
        }
        Index {
            locations,
            colors: scan.colors,
            warnings,
        }
    }

    /// The color `prefix`'s config sets, when it was read and sets one.
    pub fn color(&self, prefix: &str) -> Option<crate::palette::Rgb> {
        self.colors.get(prefix).copied()
    }

    /// The project `path` (already normalized) belongs to (§4.3).
    pub fn resolve(&self, path: &Path, warnings: &mut Vec<String>) -> Answer {
        match self.pick(path) {
            Answer::Unregistered if path.exists() => self
                .through_git(path, warnings)
                .unwrap_or(Answer::Unregistered),
            answer => answer,
        }
    }

    fn pick(&self, path: &Path) -> Answer {
        let found: Vec<&Location> = self
            .locations
            .iter()
            .filter(|l| path.starts_with(&l.path))
            .collect();
        let Some(depth) = found.iter().map(|l| l.path.components().count()).max() else {
            return Answer::Unregistered;
        };
        let deepest: Vec<&Location> = found
            .into_iter()
            .filter(|l| l.path.components().count() == depth)
            .collect();
        let pool: Vec<&Location> = if deepest.iter().any(|l| l.via.current()) {
            deepest.into_iter().filter(|l| l.via.current()).collect()
        } else {
            deepest
        };
        let mut owners: Vec<&str> = pool.iter().map(|l| l.prefix.as_str()).collect();
        owners.sort();
        owners.dedup();
        if owners.len() > 1 {
            let mut candidates: Vec<Candidate> = pool
                .iter()
                .map(|l| Candidate {
                    prefix: l.prefix.clone(),
                    via: l.via,
                    location: l.path.display().to_string(),
                })
                .collect();
            candidates.sort_by(|a, b| (&a.prefix, a.via).cmp(&(&b.prefix, b.via)));
            return Answer::Ambiguous(candidates);
        }
        let best = pool
            .into_iter()
            .min_by_key(|l| l.via)
            .expect("a deepest location");
        let rest = path
            .strip_prefix(&best.path)
            .expect("an ancestor")
            .to_path_buf();
        let (worktree, relpath) = match (best.via.storage(), rest.components().next()) {
            (true, Some(first)) => (
                Some(best.path.join(first)),
                rest.strip_prefix(first)
                    .expect("its own first component")
                    .to_path_buf(),
            ),
            _ => (None, rest),
        };
        Answer::Resolved(Match {
            prefix: best.prefix.clone(),
            via: best.via,
            location: best.path.clone(),
            worktree,
            relpath,
        })
    }

    /// §4.3 step 5: map an existing path into its main checkout, keeping its offset in
    /// the worktree, and match again.
    fn through_git(&self, path: &Path, warnings: &mut Vec<String>) -> Option<Answer> {
        let dir = if path.is_dir() { path } else { path.parent()? };
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-common-dir",
            ])
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                let warning =
                    format!("git is unavailable ({e}); worktrees outside storage are not resolved");
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                return None;
            }
        };
        if !output.status.success() {
            return None;
        }
        let mut lines = output.stdout.split(|byte| *byte == b'\n');
        let top = crate::link::path_from_git(lines.next()?)
            .canonicalize()
            .ok()?;
        let common = crate::link::path_from_git(lines.next()?);
        if common.file_name()? != ".git" {
            return None;
        }
        let main = common.parent()?.canonicalize().ok()?;
        if top == main {
            return None;
        }
        let offset = path.strip_prefix(&top).ok()?.to_path_buf();
        match self.pick(&main.join(&offset)) {
            Answer::Resolved(m) => Some(Answer::Resolved(Match {
                via: Via::Git,
                worktree: Some(top),
                relpath: offset,
                ..m
            })),
            Answer::Ambiguous(candidates) => Some(Answer::Ambiguous(candidates)),
            Answer::Unregistered => None,
        }
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
    use crate::registry::Registry;
    use std::path::Path;

    fn reg(projects: &[(&str, &Path)], history: &[(&str, Locations)]) -> Registry {
        let mut r = Registry::default();
        for (prefix, root) in projects {
            r.projects.insert((*prefix).into(), root.to_path_buf());
        }
        for (prefix, h) in history {
            r.locations.insert((*prefix).into(), h.clone());
        }
        r
    }

    fn resolved(answer: Answer) -> Match {
        match answer {
            Answer::Resolved(m) => m,
            other => panic!("expected a match, got {other:?}"),
        }
    }

    #[test]
    fn normalize_resolves_a_symlinked_ancestor_of_a_missing_tail() {
        let base = tempfile::tempdir().unwrap();
        let real = base.path().canonicalize().unwrap().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = base.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert_eq!(
            normalize(&alias.join("gone/x"), Path::new("/")).unwrap(),
            real.join("gone/x")
        );
        assert_eq!(
            normalize(Path::new("gone/./x"), &alias).unwrap(),
            real.join("gone/x"),
            "relative to base, `.` dropped"
        );
        assert_eq!(
            normalize(&alias.join("../real/y"), Path::new("/")).unwrap(),
            real.join("y")
        );
        assert!(
            normalize(&alias.join("gone/../x"), Path::new("/"))
                .unwrap_err()
                .contains("..")
        );
    }

    #[test]
    fn matching_is_component_wise_and_the_deepest_location_wins() {
        let r = reg(
            &[
                ("parent", Path::new("/p")),
                ("child", Path::new("/p/sub/child")),
            ],
            &[],
        );
        let index = Index::build(&r);
        let mut w = Vec::new();
        assert!(matches!(
            index.resolve(Path::new("/pp/x"), &mut w),
            Answer::Unregistered
        ));
        let m = resolved(index.resolve(Path::new("/p/sub/child/src/a.rs"), &mut w));
        assert_eq!((m.prefix.as_str(), m.via), ("child", Via::Root));
        assert_eq!(m.relpath, Path::new("src/a.rs"));
        let m = resolved(index.resolve(Path::new("/p/sub"), &mut w));
        assert_eq!(
            (m.prefix.as_str(), m.relpath.as_path()),
            ("parent", Path::new("sub"))
        );
    }

    #[test]
    fn current_beats_former_and_two_former_owners_are_ambiguous() {
        let gone = |root: &str| Locations {
            storage: None,
            former: vec![Former {
                root: root.into(),
                storage: None,
                until: "2026-01-01".into(),
            }],
        };
        let r = reg(
            &[
                ("new", Path::new("/x")),
                ("a", Path::new("/a")),
                ("b", Path::new("/b")),
            ],
            &[("a", gone("/x")), ("b", gone("/y"))],
        );
        let index = Index::build(&r);
        let mut w = Vec::new();
        assert_eq!(
            resolved(index.resolve(Path::new("/x/f"), &mut w)).prefix,
            "new"
        );
        let r = reg(
            &[("a", Path::new("/a")), ("b", Path::new("/b"))],
            &[("a", gone("/y")), ("b", gone("/y"))],
        );
        match Index::build(&r).resolve(Path::new("/y/f"), &mut w) {
            Answer::Ambiguous(c) => {
                let names: Vec<_> = c.iter().map(|c| c.prefix.as_str()).collect();
                assert_eq!(names, ["a", "b"]);
            }
            other => panic!("{other:?}"),
        }
        // One project as both current and former at the same path is not ambiguous.
        let r = reg(&[("a", Path::new("/y"))], &[("a", gone("/y"))]);
        assert_eq!(
            resolved(Index::build(&r).resolve(Path::new("/y/f"), &mut w)).via,
            Via::Root
        );
    }

    #[test]
    fn storage_match_splits_the_worktree() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let root = base.join("hq");
        let storage = base.join("work/hq/.worktrees");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(storage.join("wt/src")).unwrap();
        std::os::unix::fs::symlink(&storage, root.join(".worktrees")).unwrap();
        let index = Index::build(&reg(&[("hq", &root)], &[]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(&storage.join("wt/src/a.rs"), &mut w));
        assert_eq!(m.via, Via::Storage);
        assert_eq!(m.worktree, Some(storage.join("wt")));
        assert_eq!(m.relpath, Path::new("src/a.rs"));
        let m = resolved(index.resolve(&storage, &mut w));
        assert_eq!((m.worktree, m.relpath), (None, PathBuf::new()));
    }

    #[test]
    fn former_storage_resolves_a_worktree_that_no_longer_exists() {
        let h = Locations {
            storage: None,
            former: vec![Former {
                root: "/d/ai".into(),
                storage: Some("/w/ai/.worktrees".into()),
                until: "2026-09-27".into(),
            }],
        };
        let index = Index::build(&reg(&[("tack", Path::new("/d/tack"))], &[("tack", h)]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(Path::new("/w/ai/.worktrees/gone/f"), &mut w));
        assert_eq!((m.prefix.as_str(), m.via), ("tack", Via::FormerStorage));
        assert_eq!(m.worktree, Some(PathBuf::from("/w/ai/.worktrees/gone")));
        let m = resolved(index.resolve(Path::new("/d/ai/x"), &mut w));
        assert_eq!(m.via, Via::FormerRoot);
    }

    #[test]
    fn index_uses_the_record_when_the_live_link_dangles() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let root = base.join("hq");
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(base.join("not-yet"), root.join(".worktrees")).unwrap();
        let h = Locations {
            storage: Some("/w/old/.worktrees".into()),
            former: vec![],
        };
        let index = Index::build(&reg(&[("hq", &root)], &[("hq", h)]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(Path::new("/w/old/.worktrees/wt/f"), &mut w));
        assert_eq!(m.via, Via::Storage);
    }
    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    #[test]
    fn git_fallback_keeps_the_offset_into_a_nested_project() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let main = base.join("m");
        std::fs::create_dir_all(main.join("sub/nested")).unwrap();
        std::fs::write(main.join("sub/nested/f"), "").unwrap();
        git(&main, &["init", "-q"]);
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-qm", "init"]);
        let top = base.join("t");
        git(&main, &["worktree", "add", "-q", top.to_str().unwrap()]);
        std::fs::write(top.join("sub/nested/x"), "").unwrap();
        let mut w = Vec::new();
        for parent in [true, false] {
            let mut projects = vec![("nested", main.join("sub/nested"))];
            if parent {
                projects.push(("parent", main.clone()));
            }
            let pairs: Vec<(&str, &Path)> =
                projects.iter().map(|(p, r)| (*p, r.as_path())).collect();
            let index = Index::build(&reg(&pairs, &[]));
            let m = resolved(index.resolve(&top.join("sub/nested/x"), &mut w));
            assert_eq!(m.prefix, "nested", "parent={parent}");
            assert_eq!(m.via, Via::Git);
            assert_eq!(m.location, main.join("sub/nested"));
            assert_eq!(m.worktree, Some(top.clone()));
            assert_eq!(m.relpath, Path::new("sub/nested/x"));
        }
    }
}
