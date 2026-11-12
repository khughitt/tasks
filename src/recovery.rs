//! Bounded physical-record observations for missing-task diagnostics, never read authority.
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::{Kind, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(crate) const CONFIG_CAP: usize = 64 * 1024;
const RECORD_CAP: usize = 1024 * 1024;
const TOTAL_CAP: usize = 8 * 1024 * 1024;
const LOCATION_CAP: usize = 64;

#[derive(Debug)]
pub(crate) struct Candidate {
    pub root: PathBuf,
    pub kind: Kind,
    pub checkout: PathBuf,
}
#[derive(Debug)]
pub(crate) struct Gap {
    pub root: Option<PathBuf>,
    pub detail: String,
}
#[derive(Debug)]
pub(crate) enum SearchUnavailable {
    Detached,
    NoGit,
}
#[derive(Default, Debug)]
pub(crate) struct Scan {
    pub matches: Vec<Candidate>,
    pub gaps: Vec<Gap>,
    pub unavailable: Option<SearchUnavailable>,
    pub budget: Budget,
}
impl Scan {
    pub(crate) fn complete(&self) -> bool {
        self.gaps.is_empty() && self.unavailable.is_none()
    }
    fn gap(&mut self, root: Option<PathBuf>, error: impl std::fmt::Display) {
        self.gaps.push(Gap {
            root,
            detail: error.to_string(),
        });
    }
}

#[derive(Default, Debug)]
pub(crate) struct Budget {
    locations: BTreeSet<PathBuf>,
    bytes: usize,
    configs: BTreeMap<PathBuf, Vec<u8>>,
}
impl Budget {
    pub(crate) fn admit(&mut self, location: &Path) -> Result<()> {
        let location = location
            .canonicalize()
            .unwrap_or_else(|_| location.to_path_buf());
        if !self.locations.contains(&location) && self.locations.len() == LOCATION_CAP {
            return Err(Error::Validation(
                "64-location diagnostic allowance exhausted".into(),
            ));
        }
        self.locations.insert(location);
        Ok(())
    }

    pub(crate) fn read_regular(&mut self, path: &Path, cap: usize) -> Result<Vec<u8>> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let before = file.metadata()?;
        let remaining = TOTAL_CAP - self.bytes;
        if !before.is_file() {
            return Err(Error::Validation(
                "diagnostic input is not a regular file".into(),
            ));
        }
        if before.len() > cap as u64 || before.len() > remaining as u64 || remaining == 0 {
            return Err(Error::Validation(
                "diagnostic file or 8 MiB byte allowance exceeded".into(),
            ));
        }
        let limit = (before.len() as usize)
            .saturating_add(1)
            .min(cap)
            .min(remaining);
        let mut bytes = Vec::new();
        let read = (&mut file).take(limit as u64).read_to_end(&mut bytes);
        self.bytes += bytes.len();
        read?;
        let after = file.metadata()?;
        if fingerprint(&before) != fingerprint(&after) || bytes.len() as u64 != before.len() {
            return Err(Error::Validation(
                "diagnostic file changed while reading".into(),
            ));
        }
        Ok(bytes)
    }

    pub(crate) fn project(&mut self, root: &Path) -> Result<Project> {
        self.admit(root)?;
        if !self.configs.contains_key(root) {
            let bytes = self.read_regular(&root.join(crate::repo::CONFIG_REL), CONFIG_CAP)?;
            self.configs.insert(root.to_path_buf(), bytes);
        }
        let text = std::str::from_utf8(&self.configs[root])
            .map_err(|error| Error::Validation(error.to_string()))?;
        Project::from_config(root.to_path_buf(), text)
            .map_err(|e| Error::Validation(format!("invalid project configuration ({})", e.kind())))
    }
}

fn fingerprint(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn optional_metadata(result: std::io::Result<Metadata>) -> Result<Option<Metadata>> {
    match result {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn metadata(path: &Path) -> Result<Option<Metadata>> {
    optional_metadata(path.symlink_metadata())
}

fn listed_roots(bytes: &[u8]) -> Result<Vec<PathBuf>> {
    let bad = || Error::Validation("malformed Git worktree listing".into());
    if !bytes.ends_with(b"\0\0") {
        return Err(bad());
    }
    let mut roots = Vec::new();
    let mut inside = false;
    let mut fields = bytes.split(|b| *b == 0).peekable();
    while let Some(field) = fields.next() {
        if field.is_empty() {
            if !inside && fields.peek().is_some() {
                return Err(bad());
            }
            inside = false;
            continue;
        }
        if let Some(path) = field.strip_prefix(b"worktree ") {
            if inside || path.is_empty() {
                return Err(bad());
            }
            roots.push(crate::link::path_from_git(path));
            inside = true;
        } else {
            let mut pair = field.splitn(2, |b| *b == b' ');
            let key = pair.next().expect("nonempty field");
            let keyed = !key.is_empty()
                && key.iter().all(u8::is_ascii_alphabetic)
                && pair.next().is_some_and(|value| !value.is_empty());
            let flag = matches!(field, b"detached" | b"bare" | b"locked" | b"prunable");
            if !inside || !(keyed || flag) {
                return Err(bad());
            }
        }
    }
    if roots.is_empty() {
        return Err(bad());
    }
    Ok(roots)
}

pub(crate) fn discover(project: &Project, id: &TaskId) -> Scan {
    let mut scan = Scan::default();
    if project.kind == Kind::Detached {
        scan.unavailable = Some(SearchUnavailable::Detached);
        return scan;
    }
    let roots = (|| -> Result<Option<Vec<PathBuf>>> {
        let Some(tree) = crate::link::GitTree::of(&project.records)? else {
            return Ok(None);
        };
        let offset = project
            .records
            .strip_prefix(&tree.top)
            .map_err(|e| Error::Validation(e.to_string()))?;
        let output = crate::git::at(&tree.top)
            .args(["worktree", "list", "--porcelain", "-z"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git worktree list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(Some(
            listed_roots(&output.stdout)?
                .into_iter()
                .map(|p| p.join(offset))
                .collect(),
        ))
    })();
    match roots {
        Ok(Some(roots)) => collect(project, id, roots),
        Ok(None) => {
            scan.unavailable = Some(SearchUnavailable::NoGit);
            scan
        }
        Err(e) => {
            scan.gap(None, e);
            scan
        }
    }
}

fn eligible(active: &Project, other: &Project) -> bool {
    active.prefix == other.prefix
        && active.kind == other.kind
        && (active.kind != Kind::Linked || active.checkout == other.checkout)
}

struct Observation {
    candidate: Candidate,
    config: Vec<u8>,
    record: Vec<u8>,
}
fn observe(
    active: &Project,
    id: &TaskId,
    root: &Path,
    budget: &mut Budget,
) -> Result<Option<Observation>> {
    let m = root.metadata()?;
    if !m.is_dir() {
        return Err(Error::Validation(
            "listed project root is not a directory".into(),
        ));
    }
    if metadata(&root.join(crate::repo::CONFIG_REL))?.is_none() {
        root.metadata()?;
        return Ok(None);
    }
    let other = budget.project(root)?;
    if !eligible(active, &other) {
        return Ok(None);
    }
    let path = root.join(format!("tasks/{id}.md"));
    if metadata(&path)?.is_none() {
        if !root.join("tasks").metadata()?.is_dir() {
            return Err(Error::Validation("records directory disappeared".into()));
        }
        return Ok(None);
    }
    let bytes = budget.read_regular(&path, RECORD_CAP)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| Error::Validation(e.to_string()))?;
    let physical = crate::format::parse_physical(text, &path.display().to_string())
        .map_err(|e| Error::Validation(format!("invalid physical record ({})", e.kind())))?;
    if physical.id() != id {
        return Err(Error::Validation(
            "record ID does not match its filename".into(),
        ));
    }
    Ok(Some(Observation {
        candidate: Candidate {
            root: root.to_path_buf(),
            kind: other.kind,
            checkout: other.checkout,
        },
        config: budget.configs[root].clone(),
        record: bytes,
    }))
}

fn recheck(active: &Project, id: &TaskId, seen: &Observation, budget: &mut Budget) -> Result<()> {
    let config = budget.read_regular(
        &seen.candidate.root.join(crate::repo::CONFIG_REL),
        CONFIG_CAP,
    )?;
    let record = budget.read_regular(
        &seen.candidate.root.join(format!("tasks/{id}.md")),
        RECORD_CAP,
    )?;
    if config != seen.config || record != seen.record {
        return Err(Error::Validation(
            "record or config changed before reporting".into(),
        ));
    }
    let text = std::str::from_utf8(&config).map_err(|e| Error::Validation(e.to_string()))?;
    let fresh = Project::from_config(seen.candidate.root.clone(), text)?;
    if !eligible(active, &fresh)
        || fresh.kind != seen.candidate.kind
        || fresh.checkout != seen.candidate.checkout
    {
        return Err(Error::Validation("project eligibility changed".into()));
    }
    Ok(())
}

fn collect(project: &Project, id: &TaskId, mut roots: Vec<PathBuf>) -> Scan {
    let mut scan = Scan::default();
    roots.sort();
    roots.dedup();
    let mut seen = BTreeSet::new();
    let mut observations = Vec::new();
    for (index, path) in roots.iter().enumerate() {
        let root = path.canonicalize().unwrap_or_else(|_| path.clone());
        if root == project.records || !seen.insert(root.clone()) {
            continue;
        }
        if let Err(e) = scan.budget.admit(&root) {
            scan.gap(
                None,
                format!("{e}; {} locations not inspected", roots.len() - index),
            );
            break;
        }
        if root.to_str().is_none() {
            scan.gap(Some(root), "non-UTF-8 path requires manual inspection");
            continue;
        }
        match observe(project, id, &root, &mut scan.budget) {
            Ok(Some(observation)) => observations.push(observation),
            Ok(None) => {}
            Err(e) => scan.gap(Some(root), e),
        }
    }
    for observation in observations {
        match recheck(project, id, &observation, &mut scan.budget) {
            Ok(()) => scan.matches.push(observation.candidate),
            Err(e) => scan.gap(Some(observation.candidate.root), e),
        }
    }
    scan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(root: &Path) -> Project {
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::write(root.join(crate::repo::CONFIG_REL), "prefix = \"sci\"\n").unwrap();
        Project::open(root).unwrap()
    }
    fn record(root: &Path) -> TaskId {
        let id = TaskId::parse("sci-000001").unwrap();
        std::fs::write(root.join("tasks/sci-000001.md"), "---\nid: sci-000001\ntitle: Tiny\nstatus: idea\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n").unwrap();
        id
    }

    #[test]
    fn reads_enforce_file_and_total_byte_caps() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input");
        for (cap, size, allowed) in [
            (CONFIG_CAP, 65_536, true),
            (CONFIG_CAP, 65_537, false),
            (RECORD_CAP, 1_048_576, true),
            (RECORD_CAP, 1_048_577, false),
        ] {
            std::fs::write(&file, vec![b'x'; size]).unwrap();
            let mut budget = Budget::default();
            assert_eq!(budget.read_regular(&file, cap).is_ok(), allowed);
            assert!(budget.bytes <= size.min(cap));
        }
        std::fs::write(&file, vec![b'x'; 1_048_576]).unwrap();
        let mut budget = Budget::default();
        for _ in 0..8 {
            assert_eq!(
                budget.read_regular(&file, RECORD_CAP).unwrap().len(),
                1_048_576
            );
        }
        assert_eq!(budget.bytes, 8_388_608);
        assert!(budget.read_regular(&file, RECORD_CAP).is_err());
        assert_eq!(budget.bytes, 8_388_608);
    }

    #[test]
    fn location_limit_counts_distinct_aliases_once() {
        let dir = tempfile::tempdir().unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let mut budget = Budget::default();
        budget.admit(dir.path()).unwrap();
        budget.admit(&alias).unwrap();
        for n in 1..64 {
            budget.admit(&dir.path().join(format!("root-{n}"))).unwrap();
        }
        assert_eq!(budget.locations.len(), 64);
        assert!(budget.admit(&dir.path().join("sixty-fifth")).is_err());
    }

    #[test]
    fn malformed_git_output_cannot_establish_absence() {
        for bytes in [
            b"".as_slice(),
            b"worktree \0\0",
            b"HEAD x\0\0",
            b"worktree /a\0",
            b"worktree /a\0worktree /b\0\0",
            b"\0worktree /a\0\0",
            b"worktree /a\0HEAD\0\0",
            b"worktree /a\0\0\0",
        ] {
            assert!(listed_roots(bytes).is_err(), "{bytes:?}");
        }
        assert_eq!(
            listed_roots(b"worktree /a b\0HEAD abc\0locked reason\0\0worktree /c\0detached\0\0")
                .unwrap(),
            vec![PathBuf::from("/a b"), PathBuf::from("/c")]
        );
    }

    #[test]
    fn symlink_fifo_and_read_errors_are_unresolved() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file");
        std::fs::write(&target, "x").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(Budget::default().read_regular(&link, CONFIG_CAP).is_err());
        let fifo = dir.path().join("fifo");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(Budget::default().read_regular(&fifo, CONFIG_CAP).is_err());
        assert!(
            optional_metadata(Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied
            )))
            .is_err()
        );
        assert!(
            optional_metadata(Err(std::io::Error::from(std::io::ErrorKind::NotFound)))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn changed_match_and_mapping_reads_share_budget() {
        let main = tempfile::tempdir().unwrap();
        let side = tempfile::tempdir().unwrap();
        let active = project(main.path());
        project(side.path());
        let id = record(side.path());
        for change_config in [false, true] {
            project(side.path());
            record(side.path());
            let mut budget = Budget::default();
            let observation = observe(&active, &id, side.path(), &mut budget)
                .unwrap()
                .unwrap();
            let bytes_before = budget.bytes;
            let path = if change_config {
                side.path().join(crate::repo::CONFIG_REL)
            } else {
                side.path().join("tasks/sci-000001.md")
            };
            std::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .unwrap()
                .write_all(b"\n")
                .unwrap();
            assert!(recheck(&active, &id, &observation, &mut budget).is_err());
            assert!(budget.bytes > bytes_before);
        }
    }

    #[test]
    fn incomplete_locations_and_aliases_are_not_silent() {
        let main = tempfile::tempdir().unwrap();
        let side = tempfile::tempdir().unwrap();
        let active = project(main.path());
        project(side.path());
        let id = record(side.path());
        let alias = main.path().join("side-alias");
        std::os::unix::fs::symlink(side.path(), &alias).unwrap();
        let scan = collect(&active, &id, vec![side.path().to_path_buf(), alias]);
        assert!(scan.complete());
        assert_eq!(scan.matches.len(), 1);
        let gone = main.path().join("gone");
        let scan = collect(&active, &id, vec![side.path().to_path_buf(), gone]);
        assert!(!scan.complete());
        assert_eq!(scan.matches.len(), 1);
        assert_eq!(scan.gaps.len(), 1);
    }
    use std::io::Write;
}
