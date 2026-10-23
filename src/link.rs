//! Linked checkouts: a path's git facts and the registered project whose `[checkout]`
//! names its repository (ops docs/specs/2026-10-01-linked-checkouts-design.md §4.1), and
//! the detached project whose registered root is that repository's main checkout
//! (records-detached spec §3.2).

use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::{Kind, Project};
use std::path::{Path, PathBuf};

/// A path git printed, kept as the bytes git wrote: a path need not be UTF-8, and a lossy
/// decode names a different, usually missing, path.
pub fn path_from_git(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// git's view of a directory, canonical: the repository's common git directory.
#[derive(Debug, Clone, PartialEq)]
pub struct GitTree {
    pub top: PathBuf,
    pub common: PathBuf,
    pub git_dir: PathBuf,
}

impl GitTree {
    /// `None` when git says `dir` is not inside a repository and no ancestor git
    /// examined (below its ceilings and filesystem boundary) has a `.git` marker, or
    /// there is no git executable; broken metadata is an `io` error naming the nearest
    /// marker.
    /// `LC_ALL=C` pins the message matched, as `Project::git_toplevel` does. One line per requested fact, in
    /// the order of the flags.
    pub fn of(dir: &Path) -> Result<Option<GitTree>> {
        let output = match crate::git::at(dir)
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-common-dir",
                "--absolute-git-dir",
            ])
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let failed = format!(
                "git rev-parse in {} failed ({}): {}",
                dir.display(),
                output.status,
                stderr.trim()
            );
            if stderr.contains("not a git repository") {
                // A failed discovery names its scope `(or any …)`; a rejected gitdir
                // target names its path directly, without a discovery boundary.
                let discovery = stderr.contains("not a git repository (or any");
                let start = dir.canonicalize()?;
                let mut stops = Vec::new();
                if discovery {
                    stops.extend(ceilings(&start));
                    stops.extend(mount_point(&stderr));
                }
                // Only the directories git examined: those strictly below every stop.
                let examined = start.ancestors().take_while(|ancestor| {
                    stops
                        .iter()
                        .all(|stop| ancestor.starts_with(stop) && ancestor != stop)
                });
                let mut marker = None;
                for ancestor in examined {
                    let candidate = ancestor.join(".git");
                    match candidate.symlink_metadata() {
                        Ok(metadata) => {
                            marker = Some((candidate, metadata));
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                let Some((marker, metadata)) = marker else {
                    return Ok(None);
                };
                if discovery {
                    return Err(Error::Io(format!(
                        "{failed}; {}",
                        obstruction(&marker, &metadata)
                    )));
                }
            }
            return Err(Error::Io(failed));
        }
        let mut lines = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| path_from_git(line).canonicalize());
        let mut next = || -> Result<PathBuf> {
            lines
                .next()
                .ok_or_else(|| {
                    Error::Io(format!(
                        "git rev-parse in {} printed too little",
                        dir.display()
                    ))
                })?
                .map_err(Error::from)
        };
        Ok(Some(GitTree {
            top: next()?,
            common: next()?,
            git_dir: next()?,
        }))
    }

    /// This working tree is the main checkout, not a linked worktree.
    pub fn is_main(&self) -> bool {
        self.git_dir == self.common
    }

    /// Every working tree of the repository, canonical where it still exists, main first.
    pub fn worktrees(&self) -> Result<Vec<PathBuf>> {
        let output = crate::git::at(&self.top)
            .args(["worktree", "list", "--porcelain", "-z"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git worktree list in {} failed: {}",
                self.top.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(output
            .stdout
            .split(|byte| *byte == 0)
            .filter_map(|field| field.strip_prefix(b"worktree "))
            .map(|listed| {
                let listed = path_from_git(listed);
                listed.canonicalize().unwrap_or(listed)
            })
            .collect())
    }

    /// The repository's main checkout: the common directory's parent when that is a
    /// `.git` directory. `None` for a bare repository or a separate git dir.
    pub fn main_checkout(&self) -> Option<PathBuf> {
        if self.common.file_name()? != ".git" {
            return None;
        }
        self.common.parent().map(Path::to_path_buf)
    }
}

/// `GIT_CEILING_DIRECTORIES` as git's discovery reads it (setup.c): absolute entries,
/// resolved through symlinks until an empty entry and taken as written after it, where
/// git compares bytes and forgives one trailing `/`. Git skips an entry it cannot
/// resolve, and one that is not a proper ancestor of `start` bounds nothing, so neither
/// is a stop.
fn ceilings(start: &Path) -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let Some(list) = std::env::var_os("GIT_CEILING_DIRECTORIES") else {
        return Vec::new();
    };
    let mut resolve = true;
    let mut stops = Vec::new();
    for entry in std::env::split_paths(&list) {
        if entry.as_os_str().is_empty() {
            resolve = false;
            continue;
        }
        if !entry.is_absolute() {
            continue;
        }
        let ceiling = if resolve {
            match entry.canonicalize() {
                Ok(ceiling) => ceiling,
                Err(_) => continue,
            }
        } else {
            let written = entry.as_os_str().as_bytes();
            let written = match written.strip_suffix(b"/") {
                Some(trimmed) if !trimmed.is_empty() => trimmed,
                _ => written,
            };
            let under = start.as_os_str().as_bytes();
            let proper = written == b"/"
                || (under.starts_with(written) && under.get(written.len()) == Some(&b'/'));
            if !proper {
                continue;
            }
            PathBuf::from(std::ffi::OsStr::from_bytes(written))
        };
        if start.starts_with(&ceiling) && start != ceiling {
            stops.push(ceiling);
        }
    }
    stops
}

/// The filesystem boundary git stopped at, from its `LC_ALL=C` message
/// `not a git repository (or any parent up to mount point <dir>)`; git never examined
/// `<dir>` or anything above it.
fn mount_point(stderr: &str) -> Option<PathBuf> {
    let (_, rest) = stderr.split_once("(or any parent up to mount point ")?;
    let line = rest.lines().next()?;
    line.strip_suffix(')').map(PathBuf::from)
}

/// What to do about the `.git` marker git would not read. Only a directory confirmed
/// empty is offered for removal; anything else may hold repository data, so it is
/// named for inspection and never treated as absent.
fn obstruction(marker: &Path, metadata: &std::fs::Metadata) -> String {
    let parent = marker.parent().unwrap_or(marker);
    let inspect = format!(
        "inspect it with `git -C {} status` and repair it; tasks will not treat it as absent",
        parent.display()
    );
    let empty = if metadata.is_dir() {
        std::fs::read_dir(marker).map(|mut entries| entries.next().is_none())
    } else {
        Ok(false)
    };
    match empty {
        Ok(true) => format!(
            "the nearest .git, {0}, is an empty directory, which git does not take for a \
             repository; if no repository belongs at {1}, remove it with `rmdir {0}`",
            marker.display(),
            parent.display()
        ),
        Ok(false) => format!(
            "the nearest .git, {}, is not a repository git can read; {inspect}",
            marker.display()
        ),
        Err(error) => format!(
            "the nearest .git, {}, could not be read ({error}); {inspect}",
            marker.display()
        ),
    }
}

/// Spec §3.2 step 2: the project registered at `tree`'s main checkout (compared
/// canonically), when `[records]` pairs it, opened from its records directory with
/// `worktree` set to `tree`'s top level. A registered root that holds a config is an
/// in-tree project reached from a worktree without the file, `Ok(None)` as before; one
/// with neither a config nor a pairing is `records_unpaired`.
pub fn paired_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    let Some(main) = tree.main_checkout() else {
        return Ok(None);
    };
    for (prefix, root) in &registry.projects {
        if crate::rename::root_identity(root)? != main {
            continue;
        }
        if registry.records_dir(prefix).is_some() {
            let mut project =
                crate::scope::open_registered(registry, prefix, crate::scope::Origin::Prefix)?;
            project.worktree = Some(tree.top.clone());
            return Ok(Some(project));
        }
        if crate::scope::has_config(root)? {
            return Ok(None);
        }
        return Err(records_unpaired(prefix, root));
    }
    Ok(None)
}

/// Spec §3.2: a registered root that holds no config while `[records]` pairs the prefix
/// with nothing (the second host between the sync and its own `init --records`). Never
/// opened as anything; the remedy that pairs it comes first, then the one for a root that
/// truly keeps no records anywhere.
pub fn records_unpaired(prefix: &str, root: &Path) -> Error {
    Error::RecordsUnpaired(format!(
        "{prefix} is registered at {}, which holds no {CONFIG_PATH} and is paired with no \
         records directory; pair it with `tasks init --prefix {prefix} --records \
         <records-root>` run there, or, if {prefix} keeps no records anywhere yet, run \
         `tasks init --prefix {prefix}` there",
        root.display()
    ))
}

/// The project a code tree without a config of its own belongs to: the detached project
/// paired with its main checkout, else the linked project declaring it.
pub fn project_of_tree(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    match paired_project(registry, tree)? {
        Some(project) => Ok(Some(project)),
        None => linked_project(registry, tree),
    }
}

/// The `records_moved` refusal for `prefix`, whose records live at `dir`.
pub fn records_moved(prefix: &str, dir: &Path) -> Error {
    Error::RecordsMoved(format!(
        "{prefix}'s records live at {}; this tree's tasks/ is read-only history: rebase or \
         merge the branch onto the main branch",
        dir.display()
    ))
}

/// Spec §3.3: an in-tree config whose prefix `[records]` pairs with another directory is
/// a branch that predates the detach. Its `tasks/` is history, never written.
pub fn refuse_moved(registry: &Registry, project: &Project) -> Result<()> {
    if project.kind != Kind::InTree {
        return Ok(());
    }
    match registry.records_dir(&project.prefix) {
        Some(dir) if dir != project.records => Err(records_moved(&project.prefix, dir)),
        _ => Ok(()),
    }
}

/// The registered linked project whose `[checkout]` is `tree`'s main checkout. Two such
/// projects are `checkout_conflict`. Opened with `worktree` set to `tree`'s top level.
pub fn linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    let Some(main) = tree.main_checkout() else {
        return Ok(None);
    };
    let matches: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.kind == Kind::Linked && linked.checkout == main)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [one] => {
            let mut project =
                crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)?;
            project.worktree = Some(tree.top.clone());
            Ok(Some(project))
        }
        many => Err(Error::CheckoutConflict(format!(
            "{} is declared as the checkout of {}; keep one project's [checkout]",
            main.display(),
            many.iter()
                .map(|l| l.prefix.as_str())
                .collect::<Vec<_>>()
                .join(" and ")
        ))),
    }
}

/// Linked-checkouts decision 5 and §8.1: the repository holding `tree` keeps our records
/// when a registered project's config directory (`scope::config_dir`: a detached
/// project's records directory, else its root) is in it (the same common git directory),
/// or its top level holds a `tasks/.config.toml` tracked in `HEAD` as a regular file. Only
/// directories inside one of the repository's working trees can share its common
/// directory, so git runs only for those.
pub fn holds_records(registry: &Registry, tree: &GitTree) -> Result<bool> {
    let worktrees = tree.worktrees()?;
    let dirs = registry
        .projects
        .keys()
        .filter_map(|prefix| crate::scope::config_dir(registry, prefix));
    for dir in dirs {
        if !dir.try_exists()? {
            continue;
        }
        let dir = dir.canonicalize()?;
        if !worktrees.iter().any(|worktree| dir.starts_with(worktree)) {
            continue;
        }
        if GitTree::of(&dir)?.is_some_and(|theirs| theirs.common == tree.common) {
            return Ok(true);
        }
    }
    tracked_config(&tree.top)
}

/// `tasks/.config.toml` is tracked in `HEAD` at `top` as a regular file: `ls-tree` names a
/// `blob` of mode `100644` or `100755`. A tree at that path (which `git cat-file -e`
/// accepts) or a symlink (`120000`) is not a config. A genuinely unborn branch tracks
/// nothing; every other failure to read `HEAD` (a malformed loose ref, a corrupt
/// `packed-refs`) is an `io` error, never "no records" (plan review round 2).
fn tracked_config(top: &Path) -> Result<bool> {
    let git = |args: &[&str]| crate::git::at(top).args(args).output();
    let listed = git(&["ls-tree", "-z", "HEAD", "--", CONFIG_PATH])?;
    if !listed.status.success() {
        if unborn(top)? {
            return Ok(false);
        }
        return Err(Error::Io(format!(
            "git ls-tree HEAD in {} failed: {}",
            top.display(),
            String::from_utf8_lossy(&listed.stderr).trim()
        )));
    }
    // `<mode> SP <type> SP <object> TAB <path> NUL`, or nothing when the path is untracked.
    let entry = String::from_utf8_lossy(&listed.stdout);
    let Some((meta, path)) = entry.trim_end_matches('\0').split_once('\t') else {
        return Ok(false);
    };
    let mut fields = meta.split(' ');
    Ok(path == CONFIG_PATH
        && matches!(
            (fields.next(), fields.next()),
            (Some("100644" | "100755"), Some("blob"))
        ))
}

const CONFIG_PATH: &str = crate::repo::CONFIG_REL;

/// `tasks/.config.toml` is in the index at `top` (records-detached spec §5.1): `git
/// ls-files --cached` names it. A staged deletion takes it out, whatever `HEAD` tracks. A
/// failing git run is an `io` error, never "absent". Keep the invoking hook's index:
/// this checks the code commit being made, not the records repository.
pub fn config_in_index(top: &Path) -> Result<bool> {
    let listed = std::process::Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["ls-files", "-z", "--cached", "--", CONFIG_PATH])
        .env("LC_ALL", "C")
        .output()?;
    if !listed.status.success() {
        return Err(Error::Io(format!(
            "git ls-files in {} failed: {}",
            top.display(),
            String::from_utf8_lossy(&listed.stderr).trim()
        )));
    }
    Ok(!listed.stdout.is_empty())
}

/// `HEAD` names a branch that does not exist yet, and nothing is wrong: `symbolic-ref`
/// resolves `HEAD` to a ref name, and `for-each-ref` on that name lists nothing and says
/// nothing. A malformed loose ref fails `symbolic-ref` ("No such ref: HEAD"), and a
/// corrupt `packed-refs` fails both, so neither reads as unborn (both reproduced in
/// scratch repositories while planning).
fn unborn(top: &Path) -> Result<bool> {
    let git = |args: &[&str]| crate::git::at(top).args(args).output();
    let head = git(&["symbolic-ref", "-q", "HEAD"])?;
    if !head.status.success() {
        return Ok(false);
    }
    let name = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let listed = git(&["for-each-ref", "--format=%(refname)", &name])?;
    Ok(listed.status.success() && listed.stdout.is_empty() && listed.stderr.is_empty())
}

/// `to` relative to `from`, both absolute and canonical: the shared prefix dropped, one
/// `..` per remaining component of `from`, then the rest of `to`.
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in shared..from.len() {
        out.push("..");
    }
    for component in &to[shared..] {
        out.push(component.as_os_str());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mount_point_comes_from_gits_boundary_message_only() {
        let stopped = "fatal: not a git repository (or any parent up to mount point /mnt/a b)\n\
                       Stopping at filesystem boundary (GIT_DISCOVERY_ACROSS_FILESYSTEM not set).\n";
        assert_eq!(mount_point(stopped), Some(PathBuf::from("/mnt/a b")));
        let searched = "fatal: not a git repository (or any of the parent directories): .git\n";
        assert_eq!(mount_point(searched), None);
    }

    #[test]
    fn main_checkout_is_the_parent_of_a_dot_git_common_dir_only() {
        let tree = |common: &str| GitTree {
            top: "/r".into(),
            common: common.into(),
            git_dir: common.into(),
        };
        assert_eq!(tree("/r/.git").main_checkout(), Some(PathBuf::from("/r")));
        assert_eq!(
            tree("/srv/r.git").main_checkout(),
            None,
            "a bare repository has none"
        );
    }

    #[test]
    fn relative_path_climbs_then_descends() {
        assert_eq!(
            relative_path(Path::new("/d/o/nexc"), Path::new("/d/code")),
            PathBuf::from("../../code")
        );
        assert_eq!(
            relative_path(Path::new("/d/o"), Path::new("/d/o/x")),
            PathBuf::from("x")
        );
    }

    #[test]
    fn outside_a_repository_there_is_no_tree() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitTree::of(dir.path()).unwrap(), None);
    }
}
