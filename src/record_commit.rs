//! Commits the files a tasks command wrote (docs/specs/2026-10-10-record-commit-design.md
//! §3.3 and §3.7): a plain commit over an index holding nothing else, under a lock every
//! tasks writer in the repository shares, unstaging only its own paths when it fails and
//! reporting whatever it could not put back.
use crate::claims::MutationLock;
use std::collections::{BTreeMap, BTreeSet};
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
        Ok(state) => commit_paths_in(dir, paths, message, &state, &mut || {}, git),
        Err(error) => vec![failed(dir, paths, &error, None)],
    }
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git")
        .arg("--literal-pathspecs")
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

type Git = fn(&Path, &[&str]) -> std::io::Result<Output>;

struct Repo {
    toplevel: PathBuf,
    git_dir: PathBuf,
    common: PathBuf,
    git: Git,
}

/// `Ok(None)` when `dir` is outside git or there is no git executable.
fn discover(dir: &Path, git: Git) -> Result<Option<Repo>, String> {
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
            Err(cause_of("rev-parse", &output))
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
        git,
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
        ("sequencer", "a cherry-pick or revert sequence"),
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
    let output = (repo.git)(
        &repo.toplevel,
        &["diff", "--cached", "--name-only", "--no-renames", "-z"],
    )
    .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(cause_of("diff --cached", &output));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect())
}

fn head_exists(repo: &Repo) -> bool {
    (repo.git)(&repo.toplevel, &["rev-parse", "-q", "--verify", "HEAD"])
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
    match (repo.git)(&repo.toplevel, &args) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(cause_of(args[0], &output)),
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

fn cause_of(operation: &str, output: &Output) -> String {
    let tail = stderr_tail(output);
    if tail.is_empty() {
        format!("git {operation} failed ({})", output.status)
    } else if tail.contains("index.lock") {
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
    git: Git,
) -> Vec<String> {
    let repo = match discover(dir, git) {
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
            Ok(output) => cause_of(args[0], &output),
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
        Ok(output) => cause_of("commit", &output),
        Err(error) => error.to_string(),
    };
    vec![failed(dir, paths, &cause, cleanup(&repo, &ours))]
}

/// The top level of the working tree `dir` lies in, canonical; `None` outside git or
/// without a git executable. Record commits group roots by it (spec §3.7).
pub fn working_tree(dir: &Path) -> Result<Option<PathBuf>, String> {
    Ok(discover(dir, git)?.map(|repo| repo.toplevel))
}

const SUBJECT_MAX: usize = 72;

pub fn message(verb: &str, ids: &[String], title: Option<&str>) -> String {
    let subject = format!("chore(tasks): {verb} {}", ids.join(" "));
    let Some(title) = title else {
        return subject;
    };
    let room = SUBJECT_MAX.saturating_sub(subject.chars().count() + 1);
    if room < 2 {
        return subject;
    }
    let title = if title.chars().count() > room {
        format!("{}…", title.chars().take(room - 1).collect::<String>())
    } else {
        title.to_string()
    };
    format!("{subject} {title}")
}

/// The task ids a root's written paths belong to: `tasks/<id>.md` and
/// `tasks/files/<id>/…`, sorted and deduplicated. Paths are relative to the project root.
fn ids_of(paths: &BTreeSet<PathBuf>) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for path in paths {
        let parts: Vec<String> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        match parts.as_slice() {
            [tasks, file] if tasks == "tasks" => {
                if let Some(id) = file.strip_suffix(".md") {
                    ids.insert(id.to_string());
                }
            }
            [tasks, files, id, ..] if tasks == "tasks" && files == "files" => {
                ids.insert(id.clone());
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}

/// One working tree's share of a command's writes.
#[derive(Default)]
struct Tree {
    absolute: BTreeSet<PathBuf>,
    ids: BTreeSet<String>,
    projects: Vec<crate::repo::Project>,
}

/// Drains the journal and commits it: the opted-in roots grouped by git working tree, one
/// commit per working tree, so a move between two projects in one repository is one
/// commit (spec §3.2, §3.7). Called by a write command's finishing point while its locks
/// are still held. Returns the absolute paths it committed.
pub fn commit_written(warnings: &mut Vec<String>) -> BTreeSet<PathBuf> {
    let written = crate::written::take();
    let mut committed = BTreeSet::new();
    let Some(verb) = written.verb else {
        return committed;
    };
    let mut trees: BTreeMap<PathBuf, Tree> = BTreeMap::new();
    for (records, paths) in written.roots {
        let project = match crate::repo::Project::open(&records) {
            Ok(project) => project,
            Err(error) => {
                warnings.push(format!(
                    "commit_failed: {}: cannot read its config ({error})",
                    records.display()
                ));
                continue;
            }
        };
        if !project.commit_records {
            continue;
        }
        match working_tree(&records) {
            Ok(Some(top)) => {
                let tree = trees.entry(top).or_default();
                tree.ids.extend(ids_of(&paths));
                tree.absolute
                    .extend(paths.iter().map(|path| records.join(path)));
                tree.projects.push(project);
            }
            Ok(None) => {
                let paths: Vec<PathBuf> = paths.into_iter().collect();
                warnings.push(unavailable(&records, &paths));
            }
            Err(error) => warnings.push(format!("commit_failed: {}: {error}", records.display())),
        }
    }
    for (top, tree) in trees {
        let ids: Vec<String> = tree.ids.into_iter().collect();
        let title = match verb {
            "add" | "feedback" => ids
                .first()
                .and_then(|id| crate::model::TaskId::parse(id).ok())
                .and_then(|id| {
                    let project = tree
                        .projects
                        .iter()
                        .find(|project| project.prefix == id.prefix)?;
                    project.read_task(&id).ok()
                })
                .map(|task| task.title),
            _ => None,
        };
        let relative: Vec<PathBuf> = tree
            .absolute
            .iter()
            .map(|path| {
                path.strip_prefix(&top)
                    .expect("a root lies under its working tree")
                    .to_path_buf()
            })
            .collect();
        let refused = commit_paths(&top, &relative, &message(verb, &ids, title.as_deref()));
        if refused.is_empty() {
            committed.extend(tree.absolute);
        } else {
            warnings.extend(refused);
        }
    }
    committed
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_message_names_verb_and_ids_and_truncates_a_title_to_72() {
        assert_eq!(
            message("note", &["tst-000001".into()], None),
            "chore(tasks): note tst-000001"
        );
        let long = "x".repeat(100);
        let subject = message("add", &["tst-000001".into()], Some(&long));
        assert_eq!(subject.chars().count(), 72);
        assert!(subject.starts_with("chore(tasks): add tst-000001 xxx") && subject.ends_with('…'));
        assert_eq!(
            message("add", &["tst-000001".into()], Some("Short")),
            "chore(tasks): add tst-000001 Short"
        );
    }

    #[test]
    fn ids_of_reads_records_and_attachment_dirs() {
        let paths: BTreeSet<PathBuf> = [
            "tasks/tst-000002.md",
            "tasks/files/tst-000001/a b.png",
            "tasks/tst-000001.md",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            ids_of(&paths),
            vec!["tst-000001".to_string(), "tst-000002".to_string()]
        );
    }

    use super::*;
    use std::process::Command;

    fn isolated_git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
        Command::new("git")
            .arg("--literal-pathspecs")
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", dir)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CEILING_DIRECTORIES",
                std::env::temp_dir().canonicalize().unwrap(),
            )
            .output()
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = isolated_git(dir, args).unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// A repository with isolated Git execution and pinned local identity and signing.
    fn repo(initial_commit: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        let canonical = root.canonicalize().unwrap();
        assert_eq!(
            PathBuf::from(git(root, &["rev-parse", "--show-toplevel"]).trim()),
            canonical
        );
        assert!(
            PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]).trim())
                .starts_with(&canonical)
        );
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
            isolated_git,
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
    fn fixture_commits_ignore_a_hostile_global_hook() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("hostile.gitconfig");
        std::fs::write(
            &config,
            "[hook \"reject\"]\n\tevent = pre-commit\n\tcommand = \"echo hostile global hook >&2; exit 1\"\n",
        )
        .unwrap();
        let out = Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home.path())
            .env("GIT_CONFIG_GLOBAL", &config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CEILING_DIRECTORIES",
                std::env::temp_dir().canonicalize().unwrap(),
            )
            .args([
                "--exact",
                "record_commit::tests::commits_exactly_the_paths_and_leaves_unstaged_work_alone",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("1 passed;"));
    }

    #[test]
    fn a_literal_wildcard_path_commits_without_its_matching_foreign_file() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/a*.txt", "ours");
        write(root, "tasks/ab.txt", "foreign");
        assert_eq!(commit(root, &[record], state.path()), Vec::<String>::new());
        assert_eq!(last_files(root).trim(), "tasks/a*.txt");
        assert_eq!(staged(root), "");
        assert_eq!(
            git(root, &["status", "--porcelain"]).trim(),
            "?? tasks/ab.txt"
        );
    }

    #[test]
    fn literal_wildcard_cleanup_retains_matching_foreign_staging() {
        for initial_commit in [true, false] {
            let dir = repo(initial_commit);
            let root = dir.path();
            let state = state_home();
            let record = write(root, "tasks/a*.txt", "ours");
            let warnings = commit_paths_in(
                root,
                &[record],
                "m",
                state.path(),
                &mut || {
                    write(root, "tasks/ab.txt", "foreign");
                    git(root, &["add", "tasks/ab.txt"]);
                },
                isolated_git,
            );
            assert!(warnings[0].starts_with("commit_deferred:"), "{warnings:?}");
            assert_eq!(staged(root).trim(), "tasks/ab.txt");
            assert!(
                warnings[0].contains("cleanup needs attention"),
                "{warnings:?}"
            );
            assert!(root.join("tasks/a*.txt").exists());
        }
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
    fn defers_during_a_sequencer_after_a_manual_resolution_commit() {
        for operation in ["cherry-pick", "revert"] {
            let dir = repo(true);
            let root = dir.path();
            let state = state_home();
            git(root, &["checkout", "-q", "-b", "topic"]);
            write(root, "README", "topic\n");
            git(root, &["commit", "-q", "-am", "first topic change"]);
            let first = git(root, &["rev-parse", "HEAD"]);
            write(root, "other.txt", "second\n");
            git(root, &["add", "other.txt"]);
            git(root, &["commit", "-q", "-m", "second topic change"]);
            let second = git(root, &["rev-parse", "HEAD"]);
            git(root, &["checkout", "-q", "main"]);
            write(root, "README", "main\n");
            git(root, &["commit", "-q", "-am", "conflicting main change"]);
            let conflict = isolated_git(root, &[operation, first.trim(), second.trim()]).unwrap();
            assert!(!conflict.status.success());
            write(root, "README", "resolved\n");
            git(root, &["add", "README"]);
            git(root, &["commit", "-q", "-m", "manual resolution"]);
            let marker = if operation == "cherry-pick" {
                "CHERRY_PICK_HEAD"
            } else {
                "REVERT_HEAD"
            };
            assert!(!root.join(".git").join(marker).exists());
            let todo = root.join(".git/sequencer/todo");
            let pending = std::fs::read(&todo).unwrap();
            assert!(!pending.is_empty());
            assert_eq!(git(root, &["status", "--porcelain"]), "");
            let head = git(root, &["rev-parse", "HEAD"]);
            let record = write(root, "tasks/tst-000001.md", "a");
            let warnings = commit(root, &[record], state.path());
            assert!(
                warnings
                    .first()
                    .is_some_and(|warning| warning.starts_with("commit_deferred:")
                        && warning.contains(operation)),
                "{operation}: {warnings:?}"
            );
            assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
            assert_eq!(std::fs::read(&todo).unwrap(), pending);
            assert_eq!(staged(root), "");
            assert!(root.join("tasks/tst-000001.md").exists());
        }
    }

    #[test]
    fn silent_hook_failures_name_the_git_operation_and_exit_or_signal_status() {
        for (script, status) in [
            ("exit 1", "exit status: 1"),
            (r#"kill -TERM "$PPID""#, "signal: 15"),
        ] {
            let dir = repo(true);
            let root = dir.path();
            let state = state_home();
            let head = git(root, &["rev-parse", "HEAD"]);
            hook(root, script);
            let record = write(root, "tasks/tst-000001.md", "a");
            let warnings = commit(root, &[record], state.path());
            assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
            assert!(warnings[0].contains("git commit"), "{warnings:?}");
            assert!(warnings[0].contains(status), "{warnings:?}");
            assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
            assert_eq!(staged(root), "");
            assert!(root.join("tasks/tst-000001.md").exists());
        }
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
        let warnings = commit_paths_in(
            root,
            &[record],
            "m",
            state.path(),
            &mut || {
                std::fs::write(root.join("other.txt"), "x").unwrap();
                git(root, &["add", "other.txt"]);
            },
            isolated_git,
        );
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
                        let warnings = commit_paths_in(
                            &root,
                            &[record],
                            &message,
                            &state,
                            &mut || {},
                            isolated_git,
                        );
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
        let warnings = commit_paths_in(
            root,
            &[record],
            "m",
            state.path(),
            &mut || {
                std::fs::write(&lock, "").unwrap();
            },
            isolated_git,
        );
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
