//! Commits the files a tasks command wrote (docs/project-operations.md
//!  and): a plain commit over an index holding nothing else, under a lock every
//! tasks writer in the repository shares, unstaging only its own paths when it fails and
//! reporting whatever it could not put back.
use crate::claims::MutationLock;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Output;

const SHOWN: usize = 5;
const INDEX_LOCK_BACKOFF_MS: &[u64] = &[5, 10, 20, 40, 80, 120, 160, 200];

#[derive(Debug)]
pub struct Committed {
    pub toplevel: PathBuf,
    pub commit: String,
    pub paths: BTreeSet<PathBuf>,
}

fn commit_identity(stdout: &[u8]) -> Option<String> {
    std::str::from_utf8(stdout).ok()?.lines().find_map(|line| {
        let rest = line.strip_prefix('[')?;
        let (header, _) = rest.split_once("] ")?;
        if header.contains(']') {
            return None;
        }
        let sha = header.rsplit_once(' ')?.1;
        ((sha.len() == 40 || sha.len() == 64)
            && sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| sha.to_string())
    })
}

fn path_blobs(
    repo: &Repo,
    commit: Option<&str>,
    paths: &[String],
) -> Result<BTreeMap<String, String>, String> {
    let mut args = match commit {
        Some(commit) => vec!["ls-tree", "-z", commit, "--"],
        None => vec!["ls-files", "--stage", "-z", "--"],
    };
    args.extend(paths.iter().map(String::as_str));
    let output = (repo.git)(&repo.toplevel, &args).map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(cause_of(args[0], &output));
    }
    let mut blobs = BTreeMap::new();
    for entry in output
        .stdout
        .split(|b| *b == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry).map_err(|error| error.to_string())?;
        let (metadata, path) = entry.split_once('\t').ok_or("missing Git path metadata")?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.len() != 3 {
            return Err("invalid Git path metadata".into());
        }
        blobs.insert(
            path.to_string(),
            format!(
                "{} {}",
                fields[0],
                fields[if commit.is_some() { 2 } else { 1 }]
            ),
        );
    }
    Ok(blobs)
}

fn state_home() -> Result<PathBuf, String> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state"))
        .ok_or_else(|| "neither XDG_STATE_HOME nor HOME is set".to_string())
}

/// Commits `paths` (relative to `dir`, any directory inside the working tree) in one
/// commit. Returns `(refused, warnings)`: an empty `refused` means committed, or nothing
/// differed from HEAD.
pub fn commit_paths(dir: &Path, paths: &[PathBuf], message: &str) -> (Vec<String>, Vec<String>) {
    match state_home() {
        Ok(state) => commit_paths_in(dir, paths, message, &state, &mut || {}, &mut || {}, git),
        Err(error) => (vec![failed(dir, paths, &error, None)], Vec::new()),
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct Recovery {
    pub unstaged: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StagedLedger {
    version: u32,
    toplevel: PathBuf,
    git_dir: PathBuf,
    entries: Vec<StagedEntry>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct StagedEntry {
    path: String,
    staged_blob: Option<Option<String>>,
}

fn staged_ledger_path(git_dir: &Path, state: &Path) -> PathBuf {
    let digest = crate::history::sha256(git_dir.as_os_str().as_encoded_bytes());
    let hex = &digest[..16];
    state.join(format!("tasks/claims/.commit-{hex}.staged"))
}

fn write_staged_ledger(path: &Path, ledger: &StagedLedger) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension("staged.tmp");
    let bytes = serde_json::to_vec(ledger).map_err(|error| error.to_string())?;
    std::fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

fn remove_staged_ledger(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn relative_to_dir(prefix: &Path, toplevel_rel: &str) -> PathBuf {
    Path::new(toplevel_rel)
        .strip_prefix(prefix)
        .unwrap_or(Path::new(toplevel_rel))
        .to_path_buf()
}

pub fn recovery_warning(left_uncommitted: &[PathBuf]) -> Option<String> {
    if left_uncommitted.is_empty() {
        return None;
    }
    let set: BTreeSet<PathBuf> = left_uncommitted.iter().cloned().collect();
    let ids = ids_of(&set);
    let retry = if ids.is_empty() {
        "tasks commit".to_string()
    } else {
        format!("tasks commit {}", ids.join(" "))
    };
    Some(format!(
        "recovered staged changes from an earlier interrupted commit: {}; left uncommitted in the working tree; run `{retry}` to commit them",
        listed(rel(left_uncommitted))
    ))
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
    crate::git::at(dir)
        .arg("--literal-pathspecs")
        .args(args)
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
    let digest = crate::history::sha256(common.as_os_str().as_encoded_bytes());
    let hex = &digest[..16];
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
    match git_mutating(repo, &args) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(cause_of(args[0], &output)),
        Err(error) => Err(error.to_string()),
    }
}

fn git_mutating(repo: &Repo, args: &[&str]) -> std::io::Result<Output> {
    let mut result = (repo.git)(&repo.toplevel, args);
    for ms in INDEX_LOCK_BACKOFF_MS {
        let should_retry = match &result {
            Ok(output) => !output.status.success() && stderr_tail(output).contains("index.lock"),
            Err(_) => false,
        };
        if !should_retry {
            break;
        }
        #[cfg(not(test))]
        std::thread::sleep(std::time::Duration::from_millis(*ms));
        #[cfg(test)]
        {
            let _ = ms;
            std::thread::yield_now();
        }
        result = (repo.git)(&repo.toplevel, args);
    }
    result
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
            "commit_failed: {}: {cause}; left uncommitted: {}; for a refusing hook see Record-commit recovery routes in the tasks skill, then run `tasks commit` there",
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
    after_commit: &mut dyn FnMut(),
    git: Git,
) -> (Vec<String>, Vec<String>) {
    let (refused, warnings, _) =
        commit_paths_report_in(dir, paths, message, state, before_commit, after_commit, git);
    (refused, warnings)
}

fn commit_paths_report_in(
    dir: &Path,
    paths: &[PathBuf],
    message: &str,
    state: &Path,
    before_commit: &mut dyn FnMut(),
    after_commit: &mut dyn FnMut(),
    git: Git,
) -> (Vec<String>, Vec<String>, Option<Committed>) {
    let repo = match discover(dir, git) {
        Ok(Some(repo)) => repo,
        Ok(None) => return (vec![unavailable(dir, paths)], Vec::new(), None),
        Err(error) => return (vec![failed(dir, paths, &error, None)], Vec::new(), None),
    };
    let _lock = match lock_for(&repo.common, state) {
        Ok(lock) => lock,
        Err(error) => return (vec![failed(dir, paths, &error, None)], Vec::new(), None),
    };
    commit_under_lock(
        &repo,
        dir,
        paths,
        message,
        state,
        before_commit,
        after_commit,
    )
}

fn commit_under_lock(
    repo: &Repo,
    dir: &Path,
    paths: &[PathBuf],
    message: &str,
    state: &Path,
    before_commit: &mut dyn FnMut(),
    after_commit: &mut dyn FnMut(),
) -> (Vec<String>, Vec<String>, Option<Committed>) {
    let git = repo.git;
    let mut committed = None;
    let mut warnings = Vec::new();
    let refused = (|| {
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
        let ledger_path = staged_ledger_path(&repo.git_dir, state);
        let had_ledger = ledger_path.exists();
        match recover_staged(repo, state) {
            Ok((unstaged, ledger_warnings)) => {
                warnings.extend(ledger_warnings);
                let left_uncommitted: Vec<PathBuf> = unstaged
                    .into_iter()
                    .filter(|p| !ours.contains(p))
                    .map(|p| relative_to_dir(&prefix, &p))
                    .collect();
                if let Some(w) = recovery_warning(&left_uncommitted) {
                    warnings.push(w);
                }
            }
            Err(error) => return vec![failed(dir, paths, &error, None)],
        }
        match staged(repo) {
            Ok(already) if !already.is_empty() => {
                let already_vec: Vec<String> = already.iter().cloned().collect();
                let mechanism_b_ok = if !had_ledger
                    && !ours.is_empty()
                    && already.iter().all(|p| ours.contains(p))
                {
                    match path_blobs(repo, None, &already_vec)
                        .and_then(|blobs| working_tree_matches_index(repo, &already_vec, &blobs))
                    {
                        Ok(ok) => ok,
                        Err(error) => return vec![failed(dir, paths, &error, None)],
                    }
                } else {
                    false
                };
                if !mechanism_b_ok {
                    return vec![deferred(
                        dir,
                        paths,
                        &format!("staged changes: {}", listed(&already)),
                        None,
                    )];
                }
            }
            Ok(_) => {}
            Err(error) => return vec![failed(dir, paths, &error, None)],
        }
        if ours.is_empty() {
            return Vec::new();
        }
        if let Err(error) = write_staged_ledger(
            &ledger_path,
            &StagedLedger {
                version: 1,
                toplevel: repo.toplevel.clone(),
                git_dir: repo.git_dir.clone(),
                entries: ours
                    .iter()
                    .map(|path| StagedEntry {
                        path: path.clone(),
                        staged_blob: None,
                    })
                    .collect(),
            },
        ) {
            return vec![failed(dir, paths, &error, None)];
        }
        let (present, missing): (Vec<&String>, Vec<&String>) = ours
            .iter()
            .partition(|path| repo.toplevel.join(path).exists());
        let mut staged_groups: Vec<String> = Vec::new();
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
            let cause = match git_mutating(repo, &args) {
                Ok(output) if output.status.success() => {
                    staged_groups.extend(group.into_iter().cloned());
                    match path_blobs(repo, None, &staged_groups) {
                        Ok(blobs) => {
                            let _ = write_staged_ledger(
                                &ledger_path,
                                &StagedLedger {
                                    version: 1,
                                    toplevel: repo.toplevel.clone(),
                                    git_dir: repo.git_dir.clone(),
                                    entries: staged_groups
                                        .iter()
                                        .map(|path| StagedEntry {
                                            path: path.clone(),
                                            staged_blob: Some(blobs.get(path).cloned()),
                                        })
                                        .collect(),
                                },
                            );
                        }
                        Err(error) => {
                            let cl = cleanup(repo, &staged_groups);
                            if cl.is_none() {
                                let _ = remove_staged_ledger(&ledger_path);
                            }
                            return vec![failed(dir, paths, &error, cl)];
                        }
                    }
                    continue;
                }
                Ok(output) => cause_of(args[0], &output),
                Err(error) => error.to_string(),
            };
            if staged_groups.is_empty() && cause.starts_with("index locked") {
                let _ = remove_staged_ledger(&ledger_path);
                return vec![failed(dir, paths, &cause, None)];
            }
            let cleanup_targets: Vec<String> = if staged_groups.is_empty() {
                ours.clone()
            } else {
                staged_groups.clone()
            };
            let write_note = match path_blobs(repo, None, &cleanup_targets).and_then(|blobs| {
                write_staged_ledger(
                    &ledger_path,
                    &StagedLedger {
                        version: 1,
                        toplevel: repo.toplevel.clone(),
                        git_dir: repo.git_dir.clone(),
                        entries: cleanup_targets
                            .iter()
                            .filter(|path| blobs.contains_key(*path))
                            .map(|path| StagedEntry {
                                path: path.clone(),
                                staged_blob: Some(blobs.get(path).cloned()),
                            })
                            .collect(),
                    },
                )
            }) {
                Ok(()) => String::new(),
                Err(write_err) => format!("; could not update staged ledger ({write_err})"),
            };
            let cl = cleanup(repo, &cleanup_targets);
            if cl.is_none() {
                let _ = remove_staged_ledger(&ledger_path);
            }
            return vec![failed(dir, paths, &format!("{cause}{write_note}"), cl)];
        }
        before_commit();
        let now = match staged(repo) {
            Ok(now) => now,
            Err(error) => {
                let cl = cleanup(repo, &ours);
                if cl.is_none() {
                    let _ = remove_staged_ledger(&ledger_path);
                }
                return vec![failed(dir, paths, &error, cl)];
            }
        };
        if now.is_empty() {
            let _ = remove_staged_ledger(&ledger_path);
            return Vec::new(); // the files already match HEAD
        }
        let foreign: Vec<&String> = now.iter().filter(|path| !ours.contains(path)).collect();
        if !foreign.is_empty() {
            let cause = format!("staged changes appeared: {}", listed(&foreign));
            let cl = cleanup(repo, &ours);
            if cl.is_none() {
                let _ = remove_staged_ledger(&ledger_path);
            }
            return vec![deferred(dir, paths, &cause, cl)];
        }
        let expected = match path_blobs(repo, None, &ours) {
            Ok(blobs) => blobs,
            Err(error) => {
                let cl = cleanup(repo, &ours);
                if cl.is_none() {
                    let _ = remove_staged_ledger(&ledger_path);
                }
                return vec![failed(dir, paths, &error, cl)];
            }
        };
        let output = git_mutating(
            repo,
            &[
                "-c",
                "core.abbrev=no",
                // Admission may make us a subreaper; detached maintenance becomes our child.
                "-c",
                "maintenance.auto=false",
                "-c",
                "gc.auto=0",
                "commit",
                "-m",
                message,
            ],
        );
        after_commit();
        let output = match output {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                let cl = cleanup(repo, &ours);
                if cl.is_none() {
                    let _ = remove_staged_ledger(&ledger_path);
                }
                return vec![failed(dir, paths, &cause_of("commit", &output), cl)];
            }
            Err(error) => {
                let cl = cleanup(repo, &ours);
                if cl.is_none() {
                    let _ = remove_staged_ledger(&ledger_path);
                }
                return vec![failed(dir, paths, &error.to_string(), cl)];
            }
        };
        let _ = remove_staged_ledger(&ledger_path);
        let identified = commit_identity(&output.stdout).filter(|sha| {
            git(&repo.toplevel, &["cat-file", "-t", sha])
                .is_ok_and(|out| out.status.success() && out.stdout == b"commit\n")
                && path_blobs(repo, Some(sha), &ours).is_ok_and(|blobs| blobs == expected)
        });
        match identified {
            Some(commit) => {
                committed = Some(Committed {
                    toplevel: repo.toplevel.clone(),
                    commit,
                    paths: ours.iter().map(|path| repo.toplevel.join(path)).collect(),
                });
                Vec::new()
            }
            None => vec![format!(
                "commit_failed: {}: commit made but not identified; inspect the commit before retrying",
                dir.display()
            )],
        }
    })();
    (refused, warnings, committed)
}

fn working_tree_matches_index(
    repo: &Repo,
    paths: &[String],
    index_blobs: &BTreeMap<String, String>,
) -> Result<bool, String> {
    if paths.is_empty() {
        return Ok(true);
    }
    let mut args = vec!["diff", "--name-only", "-z", "--"];
    args.extend(paths.iter().map(String::as_str));
    let output = (repo.git)(&repo.toplevel, &args).map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(cause_of("diff", &output));
    }
    if !output.stdout.is_empty() {
        return Ok(false);
    }
    for path in paths {
        if !index_blobs.contains_key(path) && repo.toplevel.join(path).exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn recover_staged(repo: &Repo, state: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let ledger_path = staged_ledger_path(&repo.git_dir, state);
    if !ledger_path.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    if in_progress(&repo.git_dir).is_some()
        || !(repo.git)(&repo.toplevel, &["symbolic-ref", "-q", "HEAD"])
            .is_ok_and(|output| output.status.success())
    {
        return Ok((Vec::new(), Vec::new()));
    }
    let raw = match std::fs::read(&ledger_path) {
        Ok(raw) => raw,
        Err(error) => {
            let _ = remove_staged_ledger(&ledger_path);
            return Ok((
                Vec::new(),
                vec![format!(
                    "discarded unreadable or mismatched staged-commit ledger at {}: {error}; inspect `git status` if staged changes remain",
                    ledger_path.display()
                )],
            ));
        }
    };
    let ledger: StagedLedger = match serde_json::from_slice(&raw) {
        Ok(ledger) => ledger,
        Err(error) => {
            let _ = remove_staged_ledger(&ledger_path);
            return Ok((
                Vec::new(),
                vec![format!(
                    "discarded unreadable or mismatched staged-commit ledger at {}: {error}; inspect `git status` if staged changes remain",
                    ledger_path.display()
                )],
            ));
        }
    };
    if ledger.version != 1 || ledger.toplevel != repo.toplevel || ledger.git_dir != repo.git_dir {
        let _ = remove_staged_ledger(&ledger_path);
        return Ok((
            Vec::new(),
            vec![format!(
                "discarded unreadable or mismatched staged-commit ledger at {}: mismatched repository or version; inspect `git status` if staged changes remain",
                ledger_path.display()
            )],
        ));
    }
    let currently_staged = staged(repo)?;
    let entry_paths: Vec<String> = ledger.entries.iter().map(|e| e.path.clone()).collect();
    let current_blobs = if entry_paths.is_empty() {
        BTreeMap::new()
    } else {
        path_blobs(repo, None, &entry_paths)?
    };
    let mut to_unstage_entries = Vec::new();
    for entry in ledger.entries {
        if !currently_staged.contains(&entry.path) {
            continue;
        }
        if entry.staged_blob == Some(current_blobs.get(&entry.path).cloned()) {
            to_unstage_entries.push(entry);
        } else if entry.staged_blob.is_none()
            && working_tree_matches_index(repo, std::slice::from_ref(&entry.path), &current_blobs)?
        {
            to_unstage_entries.push(entry);
        }
    }
    let to_unstage_paths: Vec<String> =
        to_unstage_entries.iter().map(|e| e.path.clone()).collect();
    if !to_unstage_paths.is_empty() {
        if let Err(error) = unstage(repo, &to_unstage_paths) {
            let suffix = match write_staged_ledger(
                &ledger_path,
                &StagedLedger {
                    version: 1,
                    toplevel: repo.toplevel.clone(),
                    git_dir: repo.git_dir.clone(),
                    entries: to_unstage_entries,
                },
            ) {
                Ok(()) => String::new(),
                Err(write_err) => format!("; could not update staged ledger ({write_err})"),
            };
            return Err(format!(
                "could not unstage {} ({error}){suffix}",
                listed(&to_unstage_paths)
            ));
        }
    }
    let _ = remove_staged_ledger(&ledger_path);
    Ok((to_unstage_paths, Vec::new()))
}

/// The top level of the working tree `dir` lies in, canonical; `None` outside git or
/// without a git executable. Record commits group roots by it (docs/project-operations.md).
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

/// A migration's record commit message: a subject naming the two projects and how many
/// tasks the list settles, however many that is, then the ids this working tree's commit
/// touches, wrapped to the subject limit, in the body.
pub fn migration_message(source: &str, successor: &str, tasks: usize, ids: &[String]) -> String {
    let noun = if tasks == 1 { "task" } else { "tasks" };
    let mut message = format!("chore(tasks): migrate {source} to {successor} ({tasks} {noun})");
    let mut line = String::new();
    let mut body = Vec::new();
    for id in ids {
        if !line.is_empty() && line.chars().count() + 1 + id.chars().count() > SUBJECT_MAX {
            body.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(id);
    }
    if !line.is_empty() {
        body.push(line);
    }
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body.join("\n"));
    }
    message
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
        if let [.., tasks, file] = parts.as_slice()
            && tasks == "tasks"
            && let Some(id) = file.strip_suffix(".md")
            && crate::model::TaskId::parse(id).is_ok()
        {
            ids.insert(id.to_string());
            continue;
        }
        for (idx, window) in parts.windows(3).enumerate() {
            if window[0] == "tasks"
                && window[1] == "files"
                && idx + 3 < parts.len()
                && crate::model::TaskId::parse(&window[2]).is_ok()
            {
                ids.insert(window[2].clone());
                break;
            }
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
/// commit (docs/project-operations.md). Called by a write command's finishing point while its locks
/// are still held. Returns the absolute paths it committed.
pub fn commit_written(warnings: &mut Vec<String>) -> BTreeSet<PathBuf> {
    commit_written_result(warnings).1
}

pub fn commit_written_report(warnings: &mut Vec<String>) -> Vec<Committed> {
    commit_written_result(warnings).0
}

fn commit_written_result(warnings: &mut Vec<String>) -> (Vec<Committed>, BTreeSet<PathBuf>) {
    commit_drained(warnings, |verb, ids, projects| {
        let title = match verb {
            "add" | "feedback" => ids
                .first()
                .and_then(|id| crate::model::TaskId::parse(id).ok())
                .and_then(|id| {
                    let project = projects
                        .iter()
                        .find(|project| project.prefix == id.prefix)?;
                    project.read_task(&id).ok()
                })
                .map(|task| task.title),
            _ => None,
        };
        message(verb, ids, title.as_deref())
    })
}

/// `commit_written` for `migrate`, whose commit names the two projects and the count of
/// listed tasks rather than every id in its subject (`migration_message`).
pub fn commit_migration(
    warnings: &mut Vec<String>,
    source: &str,
    successor: &str,
    tasks: usize,
) -> BTreeSet<PathBuf> {
    commit_drained(warnings, |_, ids, _| {
        migration_message(source, successor, tasks, ids)
    })
    .1
}

/// Drains the journal and commits each working tree's share with the message `compose`
/// writes from the verb, the tree's ids and its projects.
fn commit_drained(
    warnings: &mut Vec<String>,
    compose: impl Fn(&str, &[String], &[crate::repo::Project]) -> String,
) -> (Vec<Committed>, BTreeSet<PathBuf>) {
    let written = crate::written::take();
    let mut committed = Vec::new();
    let mut paths = BTreeSet::new();
    let Some(verb) = written.verb else {
        return (committed, paths);
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
        let relative: Vec<PathBuf> = tree
            .absolute
            .iter()
            .map(|path| {
                path.strip_prefix(&top)
                    .expect("a root lies under its working tree")
                    .to_path_buf()
            })
            .collect();
        match state_home() {
            Ok(state) => {
                let (refused, recovery_warnings, report) = commit_paths_report_in(
                    &top,
                    &relative,
                    &compose(verb, &ids, &tree.projects),
                    &state,
                    &mut || {},
                    &mut || {},
                    git,
                );
                if refused.is_empty() {
                    paths.extend(tree.absolute);
                }
                warnings.extend(recovery_warnings);
                warnings.extend(refused);
                committed.extend(report);
            }
            Err(error) => warnings.push(failed(&top, &relative, &error, None)),
        }
    }
    (committed, paths)
}

pub struct CommitLock {
    common: PathBuf,
    state: PathBuf,
    _guard: MutationLock,
}
pub fn common_dir(dir: &Path) -> Result<PathBuf, String> {
    discover(dir, git)?
        .map(|r| r.common)
        .ok_or_else(|| "retained publication requires Git records".into())
}
#[allow(dead_code)]
pub fn try_lock(dir: &Path) -> Result<Option<CommitLock>, String> {
    let Some(repo) = discover(dir, git)? else {
        return Ok(None);
    };
    let state = state_home()?;
    let guard = lock_for(&repo.common, &state)?;
    Ok(Some(CommitLock {
        common: repo.common,
        state,
        _guard: guard,
    }))
}
pub fn lock(dir: &Path) -> Result<CommitLock, String> {
    let common = common_dir(dir)?;
    let state = state_home()?;
    let guard = lock_for(&common, &state)?;
    Ok(CommitLock {
        common,
        state,
        _guard: guard,
    })
}
impl CommitLock {
    #[allow(dead_code)]
    pub fn recover(&self, dir: &Path) -> Result<Recovery, String> {
        let repo = match discover(dir, git)? {
            Some(r) if r.common == self.common => r,
            _ => {
                return Err("held commit lock no longer matches records repository".to_string());
            }
        };
        let prefix = dir
            .canonicalize()
            .map_err(|error| error.to_string())?
            .strip_prefix(&repo.toplevel)
            .map_err(|_| "not under the repository top level".to_string())?
            .to_path_buf();
        let (unstaged, warnings) = recover_staged(&repo, &self.state)?;
        Ok(Recovery {
            unstaged: unstaged
                .into_iter()
                .map(|p| relative_to_dir(&prefix, &p))
                .collect(),
            warnings,
        })
    }

    pub fn commit_paths(
        &self,
        dir: &Path,
        paths: &[PathBuf],
        message: &str,
    ) -> (Vec<String>, Vec<String>) {
        let repo = match discover(dir, git) {
            Ok(Some(r)) if r.common == self.common => r,
            _ => {
                return (
                    vec![failed(
                        dir,
                        paths,
                        "held commit lock no longer matches records repository",
                        None,
                    )],
                    Vec::new(),
                );
            }
        };
        let (refused, warnings, _) = commit_under_lock(
            &repo,
            dir,
            paths,
            message,
            &self.state,
            &mut || {},
            &mut || {},
        );
        (refused, warnings)
    }
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
    fn a_migration_message_keeps_its_subject_short_and_wraps_ids_in_the_body() {
        assert_eq!(
            migration_message("forge", "forge2", 1, &["forge-000001".into()]),
            "chore(tasks): migrate forge to forge2 (1 task)\n\nforge-000001"
        );
        let ids: Vec<String> = (0..64).map(|n| format!("forge-{n:06x}")).collect();
        let message = migration_message("forge", "forge2", 64, &ids);
        let mut lines = message.lines();
        assert_eq!(
            lines.next(),
            Some("chore(tasks): migrate forge to forge2 (64 tasks)")
        );
        assert_eq!(lines.next(), Some(""));
        let body: Vec<&str> = lines.collect();
        assert!(body.iter().all(|line| line.chars().count() <= SUBJECT_MAX));
        assert_eq!(body.join(" ").split(' ').collect::<Vec<_>>(), ids);
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
        let (refused, warnings) = commit_paths_in(
            root,
            paths,
            "chore(tasks): note tst-000001",
            state,
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        refused
    }

    fn report(
        root: &Path,
        paths: &[PathBuf],
        after: &mut dyn FnMut(),
    ) -> (Vec<String>, Option<Committed>) {
        let state = state_home();
        let (refused, warnings, committed) = commit_paths_report_in(
            root,
            paths,
            "chore(tasks): note tst-000001",
            state.path(),
            &mut || {},
            after,
            isolated_git,
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        (refused, committed)
    }

    #[test]
    fn commit_report_does_not_substitute_head_for_missing_commit_output() {
        fn without_identity(root: &Path, args: &[&str]) -> std::io::Result<Output> {
            let mut output = isolated_git(root, args)?;
            if args.contains(&"commit") && output.status.success() {
                output.stdout.clear();
            }
            Ok(output)
        }
        let dir = repo(true);
        let root = dir.path();
        let path = write(root, "tasks/tst-000001.md", "record");
        let state = state_home();
        let (warnings, recovery_warnings, result) = commit_paths_report_in(
            root,
            &[path],
            "m",
            state.path(),
            &mut || {},
            &mut || {},
            without_identity,
        );
        assert!(recovery_warnings.is_empty());
        assert!(result.is_none());
        assert!(warnings[0].starts_with("commit_failed:"));
        assert!(warnings[0].contains("commit made but not identified"));
        assert_eq!(git(root, &["show", "HEAD:tasks/tst-000001.md"]), "record");
    }

    #[test]
    fn commit_report_refuses_a_commit_with_changed_registered_bytes() {
        let dir = repo(true);
        let root = dir.path();
        let path = write(root, "tasks/tst-000001.md", "record");
        hook(
            root,
            "echo different > tasks/tst-000001.md; git add tasks/tst-000001.md",
        );
        let (warnings, result) = report(root, &[path], &mut || {});
        assert!(result.is_none());
        assert!(warnings[0].contains("commit made but not identified"));
    }

    #[test]
    fn commit_report_noop_keeps_existing_committed_path_status() {
        if std::env::var_os("TASKS_COMMIT_NOOP_TEST").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap())
                .env("HOME", home.path())
                .env("XDG_STATE_HOME", home.path().join("state"))
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("TASKS_COMMIT_NOOP_TEST", "1")
                .args([
                    "--exact",
                    "record_commit::tests::commit_report_noop_keeps_existing_committed_path_status",
                    "--nocapture",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        write(
            &root,
            "tasks/.config.toml",
            "prefix = \"tst\"\n[git]\ncommit = true\n",
        );
        let rel = write(&root, "tasks/tst-000001.md", "already committed");
        git(&root, &["add", "tasks"]);
        git(&root, &["commit", "-q", "-m", "records"]);
        let before = git(&root, &["rev-parse", "HEAD"]);
        let path = root.join(rel);
        crate::written::begin(Some("note"));
        crate::written::record(&root, &path);
        let mut warnings = Vec::new();
        // Feedback uses this membership to tell the user whether the record is committed.
        assert!(commit_written(&mut warnings).contains(&path));
        crate::written::record(&root, &path);
        assert!(commit_written_report(&mut warnings).is_empty());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(git(&root, &["rev-parse", "HEAD"]), before);
    }

    #[test]
    fn commit_report_returns_the_commit_it_made() {
        let dir = repo(true);
        let root = dir.path();
        let path = write(root, "tasks/tst-000001.md", "record");
        let (warnings, result) = report(root, std::slice::from_ref(&path), &mut || {});
        assert!(warnings.is_empty(), "{warnings:?}");
        let result = result.unwrap();
        assert_eq!(result.commit, git(root, &["rev-parse", "HEAD"]).trim());
        assert_eq!(result.toplevel, root.canonicalize().unwrap());
        assert_eq!(result.paths, [root.join(path)].into_iter().collect());
    }

    #[test]
    fn commit_report_names_its_own_commit_when_head_moves_after_it() {
        let dir = repo(true);
        let root = dir.path();
        let path = write(root, "tasks/tst-000001.md", "record");
        let (warnings, result) = report(root, &[path], &mut || {
            write(root, "other", "another writer");
            git(root, &["add", "other"]);
            git(root, &["commit", "-q", "-m", "other writer"]);
        });
        assert!(warnings.is_empty(), "{warnings:?}");
        let result = result.unwrap();
        assert_eq!(result.commit, git(root, &["rev-parse", "HEAD^"]).trim());
        assert_eq!(
            git(
                root,
                &["show", &format!("{}:tasks/tst-000001.md", result.commit)]
            ),
            "record"
        );
    }

    #[test]
    fn commit_report_keeps_hook_behaviour() {
        let dir = repo(true);
        let root = dir.path();
        let path = write(root, "tasks/tst-000001.md", "record");
        hook(root, "echo generated > generated; git add generated");
        let (warnings, result) = report(root, std::slice::from_ref(&path), &mut || {});
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            git(
                root,
                &["show", "--name-only", "--format=", &result.unwrap().commit]
            )
            .contains("generated")
        );
        write(root, "tasks/tst-000001.md", "changed");
        hook(root, "echo hook-refused >&2; exit 1");
        let (warnings, result) = report(root, &[path], &mut || {});
        assert!(result.is_none());
        assert!(warnings[0].contains("hook-refused"), "{warnings:?}");
    }

    #[test]
    fn commit_report_identifies_a_root_commit() {
        let dir = repo(false);
        let path = write(dir.path(), "tasks/tst-000001.md", "record");
        let (warnings, result) = report(dir.path(), &[path], &mut || {});
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            result.unwrap().commit,
            git(dir.path(), &["rev-parse", "HEAD"]).trim()
        );
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
            let (warnings, recovery_warnings) = commit_paths_in(
                root,
                &[record],
                "m",
                state.path(),
                &mut || {
                    write(root, "tasks/ab.txt", "foreign");
                    git(root, &["add", "tasks/ab.txt"]);
                },
                &mut || {},
                isolated_git,
            );
            assert!(recovery_warnings.is_empty());
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
            warnings[0].contains("see Record-commit recovery routes in the tasks skill"),
            "{warnings:?}"
        );
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
        let (warnings, recovery_warnings) = commit_paths_in(
            root,
            &[record],
            "m",
            state.path(),
            &mut || {
                std::fs::write(root.join("other.txt"), "x").unwrap();
                git(root, &["add", "other.txt"]);
            },
            &mut || {},
            isolated_git,
        );
        assert!(recovery_warnings.is_empty());
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
                        let (warnings, recovery_warnings) = commit_paths_in(
                            &root,
                            &[record],
                            &message,
                            &state,
                            &mut || {},
                            &mut || {},
                            isolated_git,
                        );
                        assert_eq!(warnings, Vec::<String>::new());
                        assert_eq!(recovery_warnings, Vec::<String>::new());
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
        let (warnings, recovery_warnings) = commit_paths_in(
            root,
            &[record],
            "m",
            state.path(),
            &mut || {
                std::fs::write(&lock, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        assert!(recovery_warnings.is_empty());
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

    #[test]
    fn transient_index_lock_retries_and_succeeds() {
        use std::cell::Cell;
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        thread_local! {
            static ADD_FAILS: Cell<usize> = const { Cell::new(0) };
        }

        fn flaky_git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
            if args.first().copied() == Some("add") {
                let n = ADD_FAILS.with(|c| {
                    let v = c.get();
                    c.set(v + 1);
                    v
                });
                if n < 3 {
                    return Ok(Output {
                        status: ExitStatus::from_raw(128 << 8),
                        stdout: Vec::new(),
                        stderr: b"fatal: Unable to create '/repo/.git/index.lock': File exists.\n"
                            .to_vec(),
                    });
                }
            }
            isolated_git(dir, args)
        }

        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "a");
        ADD_FAILS.with(|c| c.set(0));
        let (warnings, recovery_warnings) = commit_paths_in(
            root,
            &[record],
            "chore(tasks): note tst-000001",
            state.path(),
            &mut || {},
            &mut || {},
            flaky_git,
        );
        assert!(warnings.is_empty() && recovery_warnings.is_empty(), "{warnings:?}");
        assert_eq!(ADD_FAILS.with(|c| c.get()), 4);
        assert_eq!(last_files(root).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn persistent_index_lock_exhausts_retries_without_unlinking_lock() {
        use std::cell::Cell;

        thread_local! {
            static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
        }

        fn counting_git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
            if args.first().copied() == Some("add") {
                ATTEMPTS.with(|c| c.set(c.get() + 1));
            }
            isolated_git(dir, args)
        }

        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let lock = root.join(".git/index.lock");
        std::fs::write(&lock, "").unwrap();
        let record = write(root, "tasks/tst-000001.md", "a");
        ATTEMPTS.with(|c| c.set(0));
        let (warnings, recovery_warnings) = commit_paths_in(
            root,
            &[record],
            "chore(tasks): note tst-000001",
            state.path(),
            &mut || {},
            &mut || {},
            counting_git,
        );
        assert!(recovery_warnings.is_empty());
        assert!(lock.exists(), ".git/index.lock must never be unlinked");
        std::fs::remove_file(&lock).unwrap();
        assert_eq!(
            ATTEMPTS.with(|c| c.get()),
            1 + INDEX_LOCK_BACKOFF_MS.len()
        );
        assert!(
            warnings[0].starts_with("commit_failed:") && warnings[0].contains("index locked"),
            "{warnings:?}"
        );
    }

    fn repo_git_dir(root: &Path) -> PathBuf {
        PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]).trim())
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn recovers_self_staged_entries_left_by_failed_commit_and_unstage() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "v1");
        let lock = root.join(".git/index.lock");
        let (refused, warnings) = commit_paths_in(
            root,
            std::slice::from_ref(&record),
            "m",
            state.path(),
            &mut || {
                std::fs::write(&lock, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        assert!(!refused.is_empty());
        assert!(warnings.is_empty());
        std::fs::remove_file(&lock).unwrap();
        let ledger_path = staged_ledger_path(&repo_git_dir(root), state.path());
        assert!(ledger_path.exists());

        let (refused, warnings) = commit_paths_in(
            root,
            &[record],
            "chore(tasks): note tst-000001",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused.is_empty(), "{refused:?}");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!ledger_path.exists());
        assert_eq!(last_files(root).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn recovers_stranded_same_task_after_new_working_tree_write_and_commits_updated_bytes() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "v1");
        let lock = root.join(".git/index.lock");
        let (refused, _) = commit_paths_in(
            root,
            std::slice::from_ref(&record),
            "m1",
            state.path(),
            &mut || {
                std::fs::write(&lock, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        assert!(!refused.is_empty());
        std::fs::remove_file(&lock).unwrap();

        // Simulate Task::save() writing new bytes ("v2") in a subsequent `tasks note` on the same task (`MM` state).
        write(root, "tasks/tst-000001.md", "v2");
        let (refused, warnings) = commit_paths_in(
            root,
            &[record],
            "chore(tasks): note tst-000001",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused.is_empty(), "{refused:?}");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(git(root, &["show", "HEAD:tasks/tst-000001.md"]), "v2");
        assert_eq!(staged(root), "");
        assert!(!staged_ledger_path(&repo_git_dir(root), state.path()).exists());
    }

    #[test]
    fn recovers_stranded_task_a_when_task_b_commits_and_warns_without_refusing() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let task_a = write(root, "tasks/tst-000001.md", "task a");
        let lock = root.join(".git/index.lock");
        let (refused, _) = commit_paths_in(
            root,
            &[task_a],
            "m1",
            state.path(),
            &mut || {
                std::fs::write(&lock, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        assert!(!refused.is_empty());
        std::fs::remove_file(&lock).unwrap();

        let task_b = write(root, "tasks/tst-000002.md", "task b");
        let (refused, warnings) = commit_paths_in(
            root,
            &[task_b],
            "chore(tasks): note tst-000002",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(last_files(root).trim(), "tasks/tst-000002.md");
        assert_eq!(staged(root), "");
        assert!(root.join("tasks/tst-000001.md").exists());
        assert!(!staged_ledger_path(&repo_git_dir(root), state.path()).exists());
        assert_eq!(
            warnings,
            vec![
                "recovered staged changes from an earlier interrupted commit: tasks/tst-000001.md; left uncommitted in the working tree; run `tasks commit tst-000001` to commit them".to_string()
            ]
        );
    }

    #[test]
    fn recover_does_not_unstage_during_merge_rebase_or_detached_head() {
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        let state = state_home();
        let record = write(&root, "tasks/tst-000001.md", "v1");
        let lock_file = root.join(".git/index.lock");
        let _ = commit_paths_in(
            &root,
            std::slice::from_ref(&record),
            "m1",
            state.path(),
            &mut || {
                std::fs::write(&lock_file, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        std::fs::remove_file(&lock_file).unwrap();
        let ledger_path = staged_ledger_path(&repo_git_dir(&root), state.path());
        assert!(ledger_path.exists());

        let head = git(&root, &["rev-parse", "HEAD"]);
        // 1. MERGE_HEAD
        std::fs::write(root.join(".git/MERGE_HEAD"), &head).unwrap();
        let (refused, warnings) = commit_paths_in(
            &root,
            std::slice::from_ref(&record),
            "m2",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused[0].starts_with("commit_deferred:"));
        assert!(warnings.is_empty());
        assert_eq!(staged(&root).trim(), "tasks/tst-000001.md");
        assert!(ledger_path.exists());
        std::fs::remove_file(root.join(".git/MERGE_HEAD")).unwrap();

        // 2. rebase-merge directory
        std::fs::create_dir_all(root.join(".git/rebase-merge")).unwrap();
        let (refused, warnings) = commit_paths_in(
            &root,
            std::slice::from_ref(&record),
            "m2",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused[0].starts_with("commit_deferred:"));
        assert!(warnings.is_empty());
        assert_eq!(staged(&root).trim(), "tasks/tst-000001.md");
        assert!(ledger_path.exists());
        std::fs::remove_dir_all(root.join(".git/rebase-merge")).unwrap();

        // 3. detached HEAD
        git(&root, &["checkout", "-q", "--detach"]);
        let (refused, warnings) = commit_paths_in(
            &root,
            &[record],
            "m2",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused[0].starts_with("commit_deferred:"));
        assert!(warnings.is_empty());
        assert_eq!(staged(&root).trim(), "tasks/tst-000001.md");
        assert!(ledger_path.exists());
        let commit_lock = CommitLock {
            common: repo_git_dir(&root),
            state: state.path().to_path_buf(),
            _guard: lock_for(&repo_git_dir(&root), state.path()).unwrap(),
        };
        let recovery = commit_lock.recover(&root).unwrap();
        assert!(recovery.unstaged.is_empty() && recovery.warnings.is_empty());
        assert_eq!(staged(&root).trim(), "tasks/tst-000001.md");
        assert!(ledger_path.exists());
    }

    #[test]
    fn failed_recovery_unstage_retains_only_to_unstage_entries_in_ledger() {
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        let state = state_home();
        let _a = write(&root, "tasks/tst-000001.md", "a");
        git(&root, &["add", "tasks/tst-000001.md"]);
        git(&root, &["commit", "-q", "-m", "commit a"]);
        let b = write(&root, "tasks/tst-000002.md", "b");
        git(&root, &["add", "tasks/tst-000002.md"]);
        let r = discover(&root, isolated_git).unwrap().unwrap();
        let blobs = path_blobs(&r, None, &["tasks/tst-000002.md".to_string()]).unwrap();
        let ledger_path = staged_ledger_path(&r.git_dir, state.path());
        write_staged_ledger(
            &ledger_path,
            &StagedLedger {
                version: 1,
                toplevel: r.toplevel.clone(),
                git_dir: r.git_dir.clone(),
                entries: vec![
                    StagedEntry {
                        path: "tasks/tst-000001.md".to_string(),
                        staged_blob: Some(Some("100644 deadbeef".to_string())),
                    },
                    StagedEntry {
                        path: "tasks/tst-000002.md".to_string(),
                        staged_blob: Some(blobs.get("tasks/tst-000002.md").cloned()),
                    },
                ],
            },
        )
        .unwrap();

        let lock_file = root.join(".git/index.lock");
        std::fs::write(&lock_file, "").unwrap();
        let (refused, _) = commit_paths_in(
            &root,
            &[b],
            "m",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        std::fs::remove_file(&lock_file).unwrap();
        assert!(
            refused[0].starts_with("commit_failed:") && refused[0].contains("index locked"),
            "{refused:?}"
        );
        let updated: StagedLedger =
            serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
        assert_eq!(updated.entries.len(), 1);
        assert_eq!(updated.entries[0].path, "tasks/tst-000002.md");
    }

    #[test]
    fn failed_first_git_add_removes_ledger_without_leaving_null_entries() {
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        let state = state_home();
        let record = write(&root, "tasks/tst-000001.md", "v1");
        let lock_file = root.join(".git/index.lock");
        std::fs::write(&lock_file, "").unwrap();
        let (refused, _) = commit_paths_in(
            &root,
            std::slice::from_ref(&record),
            "m",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        std::fs::remove_file(&lock_file).unwrap();
        assert!(refused[0].starts_with("commit_failed:"));
        let ledger_path = staged_ledger_path(&repo_git_dir(&root), state.path());
        assert!(!ledger_path.exists());

        // User partially stages v1 and writes v2 in working tree (`MM`); next commit must defer and keep v1 staged.
        git(&root, &["add", "tasks/tst-000001.md"]);
        write(&root, "tasks/tst-000001.md", "v2");
        let (refused, _) = commit_paths_in(
            &root,
            &[record],
            "m2",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused[0].starts_with("commit_deferred:"), "{refused:?}");
        assert_eq!(git(&root, &["show", ":tasks/tst-000001.md"]), "v1");
    }

    #[test]
    fn null_staged_blob_from_sigkill_unstages_only_when_index_matches_working_tree() {
        // Subcase (a): index matches working tree -> recovery unstages and commits cleanly.
        {
            let dir = repo(true);
            let root = dir.path().canonicalize().unwrap();
            let state = state_home();
            let record = write(&root, "tasks/tst-000001.md", "v1");
            git(&root, &["add", "tasks/tst-000001.md"]);
            let r = discover(&root, isolated_git).unwrap().unwrap();
            let ledger_path = staged_ledger_path(&r.git_dir, state.path());
            write_staged_ledger(
                &ledger_path,
                &StagedLedger {
                    version: 1,
                    toplevel: r.toplevel.clone(),
                    git_dir: r.git_dir.clone(),
                    entries: vec![StagedEntry {
                        path: "tasks/tst-000001.md".to_string(),
                        staged_blob: None,
                    }],
                },
            )
            .unwrap();
            let (refused, warnings) = commit_paths_in(
                &root,
                &[record],
                "m",
                state.path(),
                &mut || {},
                &mut || {},
                isolated_git,
            );
            assert!(refused.is_empty() && warnings.is_empty(), "{refused:?}");
            assert!(!ledger_path.exists());
            assert_eq!(git(&root, &["show", "HEAD:tasks/tst-000001.md"]), "v1");
        }

        // Subcase (b): index has v1 while working tree has v2 (`MM`) -> drops null entry as foreign and defers.
        {
            let dir = repo(true);
            let root = dir.path().canonicalize().unwrap();
            let state = state_home();
            let record = write(&root, "tasks/tst-000001.md", "v1");
            git(&root, &["add", "tasks/tst-000001.md"]);
            write(&root, "tasks/tst-000001.md", "v2");
            let r = discover(&root, isolated_git).unwrap().unwrap();
            let ledger_path = staged_ledger_path(&r.git_dir, state.path());
            write_staged_ledger(
                &ledger_path,
                &StagedLedger {
                    version: 1,
                    toplevel: r.toplevel.clone(),
                    git_dir: r.git_dir.clone(),
                    entries: vec![StagedEntry {
                        path: "tasks/tst-000001.md".to_string(),
                        staged_blob: None,
                    }],
                },
            )
            .unwrap();
            let (refused, _) = commit_paths_in(
                &root,
                &[record],
                "m",
                state.path(),
                &mut || {},
                &mut || {},
                isolated_git,
            );
            assert!(refused[0].starts_with("commit_deferred:"), "{refused:?}");
            assert!(!ledger_path.exists());
            assert_eq!(git(&root, &["show", ":tasks/tst-000001.md"]), "v1");
        }
    }

    #[test]
    fn externally_restaged_ledger_entry_is_dropped_as_foreign_and_defers() {
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        let state = state_home();
        let record = write(&root, "tasks/tst-000001.md", "restaged");
        git(&root, &["add", "tasks/tst-000001.md"]);
        let r = discover(&root, isolated_git).unwrap().unwrap();
        let ledger_path = staged_ledger_path(&r.git_dir, state.path());
        write_staged_ledger(
            &ledger_path,
            &StagedLedger {
                version: 1,
                toplevel: r.toplevel.clone(),
                git_dir: r.git_dir.clone(),
                entries: vec![StagedEntry {
                    path: "tasks/tst-000001.md".to_string(),
                    staged_blob: Some(Some(
                        "100644 0000000000000000000000000000000000000000".to_string(),
                    )),
                }],
            },
        )
        .unwrap();
        let (refused, _) = commit_paths_in(
            &root,
            &[record],
            "m",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused[0].starts_with("commit_deferred:"), "{refused:?}");
        assert!(!ledger_path.exists());
        assert_eq!(git(&root, &["show", ":tasks/tst-000001.md"]), "restaged");
    }

    #[test]
    fn mechanism_b_proceeds_when_staged_subset_of_ours_matches_working_tree() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "cleanly staged");
        git(root, &["add", "tasks/tst-000001.md"]);
        assert_eq!(commit(root, &[record], state.path()), Vec::<String>::new());
        assert_eq!(
            git(root, &["show", "HEAD:tasks/tst-000001.md"]),
            "cleanly staged"
        );
    }

    #[test]
    fn mechanism_b_defers_when_a_staged_deletion_is_recreated_in_the_working_tree() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        let record = write(root, "tasks/tst-000001.md", "v1");
        git(root, &["add", "tasks/tst-000001.md"]);
        git(root, &["commit", "-q", "-m", "seed"]);
        git(root, &["rm", "-q", "tasks/tst-000001.md"]);
        write(root, "tasks/tst-000001.md", "recreated in working tree");
        let refused = commit(root, &[record], state.path());
        assert!(refused[0].starts_with("commit_deferred:"), "{refused:?}");
        assert_eq!(staged(root).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn linked_worktree_commit_does_not_touch_another_worktrees_staged_ledger() {
        let dir = repo(true);
        let main = dir.path().canonicalize().unwrap();
        let state = state_home();
        let wt_dir = tempfile::tempdir().unwrap();
        let wt = wt_dir.path().join("wt");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        let main_record = write(&main, "tasks/tst-000001.md", "main stranded");
        let main_lock = main.join(".git/index.lock");
        let (refused, _) = commit_paths_in(
            &main,
            &[main_record],
            "m1",
            state.path(),
            &mut || {
                std::fs::write(&main_lock, "").unwrap();
            },
            &mut || {},
            isolated_git,
        );
        assert!(!refused.is_empty());
        std::fs::remove_file(&main_lock).unwrap();
        let main_ledger = staged_ledger_path(&repo_git_dir(&main), state.path());
        assert!(main_ledger.exists());

        let wt_record = write(&wt, "tasks/tst-000002.md", "wt record");
        assert_eq!(commit(&wt, &[wt_record], state.path()), Vec::<String>::new());
        assert!(main_ledger.exists(), "main's ledger must not be touched by wt");
        assert_eq!(staged(&main).trim(), "tasks/tst-000001.md");
    }

    #[test]
    fn corrupt_or_mismatched_staged_ledger_warns_and_is_removed() {
        let dir = repo(true);
        let root = dir.path().canonicalize().unwrap();
        let state = state_home();
        let ledger_path = staged_ledger_path(&repo_git_dir(&root), state.path());
        std::fs::create_dir_all(ledger_path.parent().unwrap()).unwrap();
        std::fs::write(&ledger_path, "{not json").unwrap();

        let record = write(&root, "tasks/tst-000001.md", "v1");
        let (refused, warnings) = commit_paths_in(
            &root,
            &[record],
            "m",
            state.path(),
            &mut || {},
            &mut || {},
            isolated_git,
        );
        assert!(refused.is_empty(), "{refused:?}");
        assert!(!ledger_path.exists());
        assert!(
            warnings
                .first()
                .is_some_and(|w| w.contains("discarded unreadable or mismatched staged-commit ledger at")),
            "{warnings:?}"
        );
    }
}
