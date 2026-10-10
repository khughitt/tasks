//! Commits the files a tasks command wrote (docs/specs/2026-10-10-record-commit-design.md
//! §3.3 and §3.7): a plain commit over an index holding nothing else, under a lock every
//! tasks writer in the repository shares, unstaging only its own paths when it fails and
//! reporting whatever it could not put back.
use crate::claims::MutationLock;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SHOWN: usize = 5;

fn state_home() -> Result<PathBuf, String> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state"))
        .ok_or_else(|| "neither XDG_STATE_HOME nor HOME is set".to_string())
}

/// Commits `paths` (relative to `dir`, any directory inside the working tree) in one
/// commit. An empty result means committed, or nothing differed from HEAD.
pub fn commit_paths(dir: &Path, paths: &[PathBuf], message: &str) -> Vec<String> {
    match state_home() {
        Ok(state) => commit_paths_in(dir, paths, message, &state, &mut || {}),
        Err(error) => vec![failed(dir, paths, &error, None)],
    }
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git")
        .args(args)
        .env("LC_ALL", "C")
        .current_dir(dir)
        .output()
}

fn stderr_tail(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(3)..].join(" / ")
}

struct Repo {
    toplevel: PathBuf,
    git_dir: PathBuf,
    common: PathBuf,
}

/// `Ok(None)` when `dir` is outside git or there is no git executable.
fn discover(dir: &Path) -> Result<Option<Repo>, String> {
    let output = match git(
        dir,
        &[
            "rev-parse",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
        ],
    ) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !output.status.success() {
        let tail = stderr_tail(&output);
        return if tail.contains("not a git repository") {
            Ok(None)
        } else {
            Err(tail)
        };
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let mut next = || {
        lines
            .next()
            .map(PathBuf::from)
            .ok_or("git rev-parse printed too little")
    };
    let toplevel = next()?;
    let git_dir = next()?;
    let common = dir.join(next()?); // relative to the cwd when git prints it relative
    let canonical = |path: PathBuf| {
        path.canonicalize()
            .map_err(|error| format!("{}: {error}", path.display()))
    };
    Ok(Some(Repo {
        toplevel: canonical(toplevel)?,
        git_dir: canonical(git_dir)?,
        common: canonical(common)?,
    }))
}

fn lock_for(common: &Path, state: &Path) -> Result<MutationLock, String> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(common.as_os_str().as_encoded_bytes());
    let hex: String = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let path = state.join(format!("tasks/claims/.commit-{hex}.lock"));
    MutationLock::acquire_at(&path).map_err(|error| error.to_string())
}

fn in_progress(git_dir: &Path) -> Option<&'static str> {
    [
        ("MERGE_HEAD", "a merge"),
        ("CHERRY_PICK_HEAD", "a cherry-pick"),
        ("REVERT_HEAD", "a revert"),
        ("BISECT_LOG", "a bisect"),
        ("rebase-merge", "a rebase"),
        ("rebase-apply", "a rebase"),
    ]
    .into_iter()
    .find(|(name, _)| git_dir.join(name).exists())
    .map(|(_, what)| what)
}

/// Top-level-relative paths with a staged change.
fn staged(repo: &Repo) -> Result<BTreeSet<String>, String> {
    let output = git(
        &repo.toplevel,
        &["diff", "--cached", "--name-only", "--no-renames", "-z"],
    )
    .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(stderr_tail(&output));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect())
}

fn head_exists(repo: &Repo) -> bool {
    git(&repo.toplevel, &["rev-parse", "-q", "--verify", "HEAD"])
        .is_ok_and(|output| output.status.success())
}

/// Puts the index back to HEAD for `ours`, and only for `ours`.
fn unstage(repo: &Repo, ours: &[String]) -> Result<(), String> {
    let mut args: Vec<&str> = if head_exists(repo) {
        vec!["reset", "-q", "--"]
    } else {
        vec!["rm", "--cached", "-q", "-r", "--ignore-unmatch", "--"]
    };
    args.extend(ours.iter().map(String::as_str));
    match git(&repo.toplevel, &args) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(stderr_tail(&output)),
        Err(error) => Err(error.to_string()),
    }
}

/// Unstages `ours`, then says what a person must look at: `None` only when the unstage
/// worked and the index then holds nothing staged. Every failure is reported, including a
/// failure to read the index afterwards.
fn cleanup(repo: &Repo, ours: &[String]) -> Option<String> {
    let mut problems = Vec::new();
    if let Err(error) = unstage(repo, ours) {
        problems.push(format!("could not unstage {} ({error})", listed(ours)));
    }
    match staged(repo) {
        Ok(still) if still.is_empty() => {}
        Ok(still) => problems.push(format!("still staged: {}", listed(&still))),
        Err(error) => problems.push(format!("could not read the index ({error})")),
    }
    (!problems.is_empty()).then(|| format!("cleanup needs attention: {}", problems.join("; ")))
}

fn listed(paths: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let all: Vec<String> = paths
        .into_iter()
        .map(|path| path.as_ref().to_string())
        .collect();
    let mut shown = all
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if all.len() > SHOWN {
        shown.push_str(&format!(" and {} more", all.len() - SHOWN));
    }
    shown
}

fn rel(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

fn with_cleanup(text: String, cleanup: Option<String>) -> String {
    match cleanup {
        Some(cleanup) => format!("{text}; {cleanup}"),
        None => text,
    }
}

fn deferred(dir: &Path, paths: &[PathBuf], cause: &str, cleanup: Option<String>) -> String {
    with_cleanup(
        format!(
            "commit_deferred: {}: {cause}; left uncommitted: {}; run `tasks commit` there once that is cleared",
            dir.display(),
            listed(rel(paths))
        ),
        cleanup,
    )
}

fn failed(dir: &Path, paths: &[PathBuf], cause: &str, cleanup: Option<String>) -> String {
    with_cleanup(
        format!(
            "commit_failed: {}: {cause}; left uncommitted: {}; for a refusing hook see the record-commit routes in the tasks skill, then run `tasks commit` there",
            dir.display(),
            listed(rel(paths))
        ),
        cleanup,
    )
}

pub(crate) fn unavailable(dir: &Path, paths: &[PathBuf]) -> String {
    format!(
        "commit_unavailable: {} is not in a git repository (or git is missing); left uncommitted: {}",
        dir.display(),
        listed(rel(paths))
    )
}

fn cause_of(output: &Output) -> String {
    let tail = stderr_tail(output);
    if tail.contains("index.lock") {
        format!("index locked ({tail})")
    } else {
        tail
    }
}

pub(crate) fn commit_paths_in(
    dir: &Path,
    paths: &[PathBuf],
    message: &str,
    state: &Path,
    before_commit: &mut dyn FnMut(),
) -> Vec<String> {
    let repo = match discover(dir) {
        Ok(Some(repo)) => repo,
        Ok(None) => return vec![unavailable(dir, paths)],
        Err(error) => return vec![failed(dir, paths, &error, None)],
    };
    let _lock = match lock_for(&repo.common, state) {
        Ok(lock) => lock,
        Err(error) => return vec![failed(dir, paths, &error, None)],
    };
    if let Some(what) = in_progress(&repo.git_dir) {
        return vec![deferred(
            dir,
            paths,
            &format!("{what} is in progress"),
            None,
        )];
    }
    if !git(&repo.toplevel, &["symbolic-ref", "-q", "HEAD"])
        .is_ok_and(|output| output.status.success())
    {
        return vec![deferred(dir, paths, "HEAD is detached", None)];
    }
    match staged(&repo) {
        Ok(already) if !already.is_empty() => {
            return vec![deferred(
                dir,
                paths,
                &format!("staged changes: {}", listed(&already)),
                None,
            )];
        }
        Ok(_) => {}
        Err(error) => return vec![failed(dir, paths, &error, None)],
    }
    let prefix = match dir.canonicalize() {
        Ok(dir) => match dir.strip_prefix(&repo.toplevel) {
            Ok(prefix) => prefix.to_path_buf(),
            Err(_) => {
                return vec![failed(
                    dir.as_path(),
                    paths,
                    "not under the repository top level",
                    None,
                )];
            }
        },
        Err(error) => return vec![failed(dir, paths, &error.to_string(), None)],
    };
    let ours: Vec<String> = paths
        .iter()
        .map(|path| prefix.join(path).display().to_string())
        .collect();
    let (present, missing): (Vec<&String>, Vec<&String>) = ours
        .iter()
        .partition(|path| repo.toplevel.join(path).exists());
    for (base, group) in [
        (vec!["add", "-A", "--"], present),
        (
            vec!["rm", "--cached", "-q", "-r", "--ignore-unmatch", "--"],
            missing,
        ),
    ] {
        if group.is_empty() {
            continue;
        }
        let mut args = base;
        args.extend(group.iter().map(|path| path.as_str()));
        let cause = match git(&repo.toplevel, &args) {
            Ok(output) if output.status.success() => continue,
            Ok(output) => cause_of(&output),
            Err(error) => error.to_string(),
        };
        return vec![failed(dir, paths, &cause, cleanup(&repo, &ours))];
    }
    before_commit();
    let now = match staged(&repo) {
        Ok(now) => now,
        Err(error) => return vec![failed(dir, paths, &error, cleanup(&repo, &ours))],
    };
    if now.is_empty() {
        return Vec::new(); // the files already match HEAD
    }
    let foreign: Vec<&String> = now.iter().filter(|path| !ours.contains(path)).collect();
    if !foreign.is_empty() {
        let cause = format!("staged changes appeared: {}", listed(&foreign));
        return vec![deferred(dir, paths, &cause, cleanup(&repo, &ours))];
    }
    let cause = match git(&repo.toplevel, &["commit", "-q", "-m", message]) {
        Ok(output) if output.status.success() => return Vec::new(),
        Ok(output) => cause_of(&output),
        Err(error) => error.to_string(),
    };
    vec![failed(dir, paths, &cause, cleanup(&repo, &ours))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// A repository whose local config pins identity, signing and hooks, so the
    /// developer's global config cannot change the outcome.
    fn repo(initial_commit: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.name", "t"),
            ("user.email", "t@t"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", ".git/hooks"),
        ] {
            git(root, &["config", key, value]);
        }
        std::fs::create_dir_all(root.join("tasks/files")).unwrap();
        if initial_commit {
            std::fs::write(root.join("README"), "r").unwrap();
            git(root, &["add", "README"]);
            git(root, &["commit", "-q", "-m", "init"]);
        }
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        PathBuf::from(rel)
    }

    fn hook(root: &Path, script: &str) {
        let path = root.join(".git/hooks/pre-commit");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn staged(root: &Path) -> String {
        git(root, &["diff", "--cached", "--name-only"])
    }

    fn last_files(root: &Path) -> String {
        git(root, &["show", "--name-only", "--format=", "HEAD"])
    }

    fn state_home() -> tempfile::TempDir {
        // The repository lock lives under XDG_STATE_HOME; every test sets its own, through
        // the seam below, never through the process environment.
        tempfile::tempdir().unwrap()
    }

    fn commit(root: &Path, paths: &[PathBuf], state: &Path) -> Vec<String> {
        commit_paths_in(
            root,
            paths,
            "chore(tasks): note tst-000001",
            state,
            &mut || {},
        )
    }

    #[test]
    fn commits_exactly_the_paths_and_leaves_unstaged_work_alone() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        write(root, "README", "edited, unstaged");
        let record = write(root, "tasks/tst-000001.md", "a");
        assert_eq!(commit(root, &[record], state.path()), Vec::<String>::new());
        assert_eq!(last_files(root).trim(), "tasks/tst-000001.md");
        assert_eq!(
            git(root, &["log", "-1", "--format=%s"]).trim(),
            "chore(tasks): note tst-000001"
        );
        assert_eq!(git(root, &["status", "--porcelain"]).trim(), "M README");
    }

    #[test]
    fn defers_when_anything_is_staged_even_its_own_record() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "staged version");
        git(root, &["add", "tasks/tst-000001.md"]);
        write(root, "tasks/tst-000001.md", "working version");
        let warnings = commit(root, &[record], state.path());
        assert!(warnings[0].starts_with("commit_deferred:"), "{warnings:?}");
        assert!(warnings[0].contains("staged changes"), "{warnings:?}");
        assert_eq!(
            git(root, &["show", ":tasks/tst-000001.md"]),
            "staged version"
        );
    }

    #[test]
    fn defers_during_a_merge_and_on_a_detached_head() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "a");
        let head = git(root, &["rev-parse", "HEAD"]);
        std::fs::write(root.join(".git/MERGE_HEAD"), &head).unwrap();
        let warnings = commit(root, std::slice::from_ref(&record), state.path());
        assert!(
            warnings[0].starts_with("commit_deferred:") && warnings[0].contains("merge"),
            "{warnings:?}"
        );
        std::fs::remove_file(root.join(".git/MERGE_HEAD")).unwrap();
        git(root, &["checkout", "-q", "--detach"]);
        let warnings = commit(root, &[record], state.path());
        assert!(
            warnings[0].starts_with("commit_deferred:") && warnings[0].contains("detached"),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_hook_that_stages_a_derived_file_lands_it_and_leaves_the_index_clean() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        hook(root, "date > derived.txt && git add derived.txt");
        let record = write(root, "tasks/tst-000001.md", "a");
        assert_eq!(commit(root, &[record], state.path()), Vec::<String>::new());
        let files = last_files(root);
        assert!(
            files.contains("derived.txt") && files.contains("tasks/tst-000001.md"),
            "{files}"
        );
        assert_eq!(staged(root), "");
    }

    #[test]
    fn a_refusing_hook_unstages_only_the_records_and_keeps_outside_staging() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        // The hook stands in for a separate git process staging while the hook runs: the
        // CLI cannot tell the two apart, which is the point of the test.
        hook(
            root,
            "echo refused >&2; echo x > other.txt; git add other.txt; exit 1",
        );
        let record = write(root, "tasks/tst-000001.md", "a");
        let warnings = commit(root, &[record], state.path());
        assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
        assert!(warnings[0].contains("refused"), "{warnings:?}");
        assert!(
            warnings[0].contains("cleanup needs attention") && warnings[0].contains("other.txt"),
            "{warnings:?}"
        );
        assert_eq!(staged(root).trim(), "other.txt");
        assert!(root.join("tasks/tst-000001.md").exists());
    }

    #[test]
    fn staging_that_appears_before_the_commit_defers_and_unstages_only_the_records() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "a");
        let warnings = commit_paths_in(root, &[record], "m", state.path(), &mut || {
            std::fs::write(root.join("other.txt"), "x").unwrap();
            git(root, &["add", "other.txt"]);
        });
        assert!(warnings[0].starts_with("commit_deferred:"), "{warnings:?}");
        assert_eq!(staged(root).trim(), "other.txt");
    }

    #[test]
    fn a_held_index_lock_fails_and_leaves_the_index_unchanged() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        std::fs::write(root.join(".git/index.lock"), "").unwrap();
        let record = write(root, "tasks/tst-000001.md", "a");
        let warnings = commit(root, &[record], state.path());
        std::fs::remove_file(root.join(".git/index.lock")).unwrap();
        assert!(
            warnings[0].starts_with("commit_failed:") && warnings[0].contains("index locked"),
            "{warnings:?}"
        );
        assert_eq!(staged(root), "");
    }

    #[test]
    fn outside_git_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_home();
        let record = write(dir.path(), "tasks/tst-000001.md", "a");
        let warnings = commit(dir.path(), &[record], state.path());
        assert!(
            warnings[0].starts_with("commit_unavailable:"),
            "{warnings:?}"
        );
    }

    #[test]
    fn an_unborn_head_commits_and_a_refused_commit_unstages_with_rm_cached() {
        let dir = repo(false);
        let root = dir.path();
        let state = state_home();
        hook(root, "exit 1");
        let record = write(root, "tasks/tst-000001.md", "a");
        let warnings = commit(root, std::slice::from_ref(&record), state.path());
        assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
        assert_eq!(staged(root), "");
        std::fs::remove_file(root.join(".git/hooks/pre-commit")).unwrap();
        assert_eq!(commit(root, &[record], state.path()), Vec::<String>::new());
        assert_eq!(last_files(root).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn a_project_nested_below_the_top_level_commits_its_paths() {
        let dir = repo(true);
        let top = dir.path();
        let root = top.join("sub");
        let state = state_home();
        let record = write(&root, "tasks/tst-000001.md", "a");
        assert_eq!(commit(&root, &[record], state.path()), Vec::<String>::new());
        assert_eq!(last_files(top).trim(), "sub/tasks/tst-000001.md");
    }

    #[test]
    fn a_path_with_a_space_and_a_deleted_file_commit_whole() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let kept = write(root, "tasks/files/tst-000001/gone.png", "g");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "seed"]);
        std::fs::remove_file(root.join(&kept)).unwrap();
        let spaced = write(root, "tasks/files/tst-000001/a b.png", "x");
        assert_eq!(
            commit(root, &[kept, spaced], state.path()),
            Vec::<String>::new()
        );
        let files = last_files(root);
        assert!(
            files.contains("tasks/files/tst-000001/a b.png") && files.contains("gone.png"),
            "{files}"
        );
        assert_eq!(git(root, &["status", "--porcelain"]), "");
    }

    #[test]
    fn a_linked_worktree_ignores_a_merge_in_its_main_checkout() {
        let dir = repo(true);
        let main = dir.path();
        let state = state_home();
        let wt = main.join("wt");
        git(
            main,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        let head = git(main, &["rev-parse", "HEAD"]);
        std::fs::write(main.join(".git/MERGE_HEAD"), &head).unwrap();
        let record = write(&wt, "tasks/tst-000001.md", "a");
        assert_eq!(commit(&wt, &[record], state.path()), Vec::<String>::new());
        assert_eq!(last_files(&wt).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn an_ignored_path_fails_and_unstages_the_rest() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        std::fs::write(root.join(".gitignore"), "tasks/files/\n").unwrap();
        git(root, &["add", ".gitignore"]);
        git(root, &["commit", "-q", "-m", "ignore"]);
        let record = write(root, "tasks/tst-000001.md", "a");
        let file = write(root, "tasks/files/tst-000001/a.png", "x");
        let warnings = commit(root, &[record, file], state.path());
        assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
        assert_eq!(staged(root), "");
    }

    #[test]
    fn two_writers_in_one_repository_commit_separately() {
        let dir = repo(true);
        let root = dir.path().to_path_buf();
        let state = state_home();
        let state_path = state.path().to_path_buf();
        let handles: Vec<_> = (0..2)
            .map(|n| {
                let root = root.clone();
                let state = state_path.clone();
                std::thread::spawn(move || {
                    for i in 0..5 {
                        let record =
                            write(&root, &format!("tasks/tst-00000{n}.md"), &format!("{i}"));
                        let message = format!("chore(tasks): note tst-00000{n}");
                        let warnings =
                            commit_paths_in(&root, &[record], &message, &state, &mut || {});
                        assert_eq!(warnings, Vec::<String>::new());
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let log = git(&root, &["log", "--format=%s", "--name-only", "-10"]);
        for block in log.split("chore(tasks): note ").skip(1) {
            let mut lines = block.lines().filter(|line| !line.is_empty());
            let id = lines.next().unwrap();
            let files: Vec<_> = lines.collect();
            assert_eq!(files, vec![format!("tasks/{id}.md")], "{log}");
        }
    }

    #[test]
    fn a_lock_taken_after_staging_is_reported_as_cleanup_needing_attention() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "a");
        // Another git process takes the index lock after the records are staged: the
        // commit fails, and so does the unstage, which must be reported, not swallowed.
        let lock = root.join(".git/index.lock");
        let warnings = commit_paths_in(root, &[record], "m", state.path(), &mut || {
            std::fs::write(&lock, "").unwrap();
        });
        std::fs::remove_file(&lock).unwrap();
        assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
        assert!(
            warnings[0].contains("cleanup needs attention"),
            "{warnings:?}"
        );
        assert!(warnings[0].contains("could not unstage"), "{warnings:?}");
        assert!(
            warnings[0].contains("still staged: tasks/tst-000001.md"),
            "{warnings:?}"
        );
        assert_eq!(staged(root).trim(), "tasks/tst-000001.md");
    }
}
