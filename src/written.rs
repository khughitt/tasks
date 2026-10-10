//! The files under a project root this command wrote, which the record commit commits
//! (docs/specs/2026-10-10-record-commit-design.md §3.2). Process-wide: the writes happen in
//! `Project`, `attachments` and `moving`, far below the command that finishes them, and a
//! command is one process.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Written {
    /// The verb the commit message names; `None` for a command that commits nothing.
    pub verb: Option<&'static str>,
    /// Canonical project root -> root-relative paths written under it.
    pub roots: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

static JOURNAL: Mutex<Written> = Mutex::new(Written {
    verb: None,
    roots: BTreeMap::new(),
});

fn journal() -> std::sync::MutexGuard<'static, Written> {
    JOURNAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn begin(verb: Option<&'static str>) {
    let mut journal = journal();
    journal.verb = verb;
    journal.roots.clear();
}

pub fn record(root: &Path, path: &Path) {
    let relative = path
        .strip_prefix(root)
        .unwrap_or_else(|_| panic!("{} is not under {}", path.display(), root.display()))
        .to_path_buf();
    journal()
        .roots
        .entry(root.to_path_buf())
        .or_default()
        .insert(relative);
}

pub fn take() -> Written {
    let mut journal = journal();
    Written {
        verb: journal.verb,
        roots: std::mem::take(&mut journal.roots),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_paths_relative_to_their_root_and_take_drains() {
        // The journal is process-wide and other unit tests write records concurrently, so
        // this asserts only on a root no other test uses.
        let root = Path::new("/written-test-root");
        record(root, &root.join("tasks/tst-000001.md"));
        record(root, &root.join("tasks/files/tst-000001/a b.png"));
        record(root, &root.join("tasks/tst-000001.md"));
        let written = take();
        let paths: Vec<_> = written.roots[root].iter().cloned().collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("tasks/files/tst-000001/a b.png"),
                PathBuf::from("tasks/tst-000001.md"),
            ]
        );
        assert!(!take().roots.contains_key(root));
    }
}
