# Record Commit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In a project that sets `[git] commit = true`, every tasks write commits the files
it wrote, and `tasks commit` commits whatever is left under `tasks/`.

**Architecture:** Record writes register their paths in a process-wide journal
(`src/written.rs`). A write command's finishing point drains the journal while its mutation
lock is still held, and hands each opted-in root to the commit engine
(`src/record_commit.rs`). The engine runs the spec's §3.3 procedure under a repository lock
keyed by the git common directory (§3.7). `tasks commit` calls the engine directly.

**Tech Stack:** Rust (edition as in `Cargo.toml`), the `git` executable through
`std::process::Command`, `sha2` (already a dependency), `assert_cmd` and `tempfile` in
tests.

**Spec:** `docs/specs/2026-10-10-record-commit-design.md` (approved, review round 3)

## Global Constraints

- Opt-in: `[git]` with `commit = true` in `tasks/.config.toml`. Without it, behaviour is
  unchanged. Any other key in `[git]` is a config error.
- Commit message: `chore(tasks): <verb> <id>[ <id>…]`. For `add` and `feedback`, the title
  follows, truncated so the subject is at most 72 characters. No trailer, ever.
- Never `--no-verify`. Never a pathspec on `git commit` (a pathspec means `--only`).
- Defer when *anything* is staged before the command stages its own paths.
- On failure, unstage only the command's own paths. Never run a bare `git reset`.
- Every git call sets `LC_ALL=C`.
- The repository commit lock is innermost: taken after the project mutation lock (and the
  holds lock when held), and no other lock is acquired while it is held.
- A write command still exits 0 when its commit is skipped or fails. The warning carries
  the cause. `tasks commit` exits 1 when it cannot commit, because it is an explicit
  request.
- Warning prefixes, exactly: `commit_unavailable:`, `commit_deferred:`, `commit_failed:`.
  A failure that leaves other paths staged adds `cleanup needs attention:`.
- Tests: `just test-one <name>` while working, `just test-fast` before each commit. Never
  call cargo directly.
- Commits: conventional, no AI attribution.

## Review Focus

- **A repository with no commits yet (unborn HEAD).** A person runs `git init`, then
  `tasks init`, then `tasks add`. The add should commit, and a failed commit should
  unstage through `git rm --cached` (there is no HEAD to reset to). Test in Task 3.
- **A project nested below the repository top level** (`repo/sub/tasks/`). Paths git
  reports are relative to the top level, the journal's to the project root. Mixing them up
  either commits nothing or defers forever. Test in Task 3.
- **A path with a space**, such as an attachment named `a b.png`. A `-z`/argv mistake
  splits it into two. Test in Task 3.
- **A linked git worktree whose main checkout is mid-merge.** Operation detection must
  read the worktree's own git dir, not the common dir, or a merge in the main checkout
  blocks every task branch. Test in Task 3.
- **`tasks/files/` ignored by `.gitignore`.** `git add` exits 1 for an explicit ignored
  path. That must surface as `commit_failed`, with the record unstaged and nothing else
  touched. Test in Task 3.

---

### Task 1: The `[git] commit` config key

**Files:**
- Modify: `src/repo.rs` (struct `Config` at ~117, `Project` at ~68, `Project::init` literal at ~367, `Project::open` at ~403, unit tests at ~1062)

**Interfaces:**
- Produces: `pub commit_records: bool` on `Project`. It is `true` only when the config holds `[git] commit = true`.

- [ ] **Step 1: Write the failing test** (in `src/repo.rs`'s `mod tests`, beside `tag_dictionary_is_optional_and_its_entries_are_validated`)

```rust
#[test]
fn git_commit_is_off_by_default_and_its_table_is_validated() {
    let (_dir, project) = project_with_config("prefix = \"tst\"\n");
    assert!(!project.unwrap().commit_records);

    let (_dir, project) = project_with_config("prefix = \"tst\"\n\n[git]\ncommit = true\n");
    assert!(project.unwrap().commit_records);

    let (_dir, project) = project_with_config("prefix = \"tst\"\n\n[git]\ncommit = false\n");
    assert!(!project.unwrap().commit_records);

    let (_dir, project) = project_with_config("prefix = \"tst\"\n\n[git]\npush = true\n");
    assert!(matches!(project, Err(Error::Config(_))), "{project:?}");

    let (_dir, project) = project_with_config("prefix = \"tst\"\n\n[git]\ncommit = \"yes\"\n");
    assert!(matches!(project, Err(Error::Config(_))), "{project:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `just test-one git_commit_is_off_by_default`
Expected: compile error, `no field commit_records on type Project`.

- [ ] **Step 3: Implement**

In `src/repo.rs`, add beside `CheckoutConfig`:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GitConfig {
    #[serde(default)]
    commit: bool,
}
```

Add to `struct Config`:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    git: Option<GitConfig>,
```

Add `git: None,` to the `Config { … }` literal in `Project::init`. Add to `Project`:

```rust
    /// The `[git] commit` switch: commit each record write as it lands. See
    /// docs/specs/2026-10-10-record-commit-design.md §3.1.
    pub commit_records: bool,
```

In `Project::open`'s struct literal: `commit_records: config.git.is_some_and(|git| git.commit),`. Fix every other `Project { … }` literal the compiler reports (`cargo` errors list them) by adding `commit_records: false`. A literal that spreads `..Project::open(…)` needs nothing.

- [ ] **Step 4: Run test to verify it passes**

Run: `just test-one git_commit_is_off_by_default`
Expected: PASS

- [ ] **Step 5: Run the fast suite and commit**

Run: `just test-fast`, expected all pass.

```bash
git add src/repo.rs
git commit -m "feat(config): read the [git] commit switch"
```

---

### Task 2: The written-paths journal

**Files:**
- Create: `src/written.rs`
- Modify: `src/main.rs` (add `mod written;` beside the other `mod` lines), `src/repo.rs` (`write_task` ~597, `publish_task_file` ~934), `src/commands/attach.rs` (~138, ~148, ~206), `src/moving/mod.rs` (~226, ~270, ~281)

**Interfaces:**
- Produces:
  - `pub fn written::begin(verb: Option<&'static str>)` clears the journal and sets the verb.
  - `pub fn written::record(root: &Path, path: &Path)` takes a canonical project root and an absolute path under it.
  - `pub fn written::take() -> written::Written` drains the journal.
  - `pub struct Written { pub verb: Option<&'static str>, pub roots: BTreeMap<PathBuf, BTreeSet<PathBuf>> }`, where each root maps to root-relative paths.

- [ ] **Step 1: Write the failing test** (`src/written.rs`, at the bottom)

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `just test-one records_paths_relative_to_their_root`
Expected: compile error (module missing).

- [ ] **Step 3: Implement `src/written.rs`**

```rust
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
    JOURNAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `just test-one records_paths_relative_to_their_root`
Expected: PASS

- [ ] **Step 5: Record every record and attachment write**

Each call goes right after the write succeeds:

- `src/repo.rs` `write_task`:
  ```rust
  let path = self.task_path(&task.id);
  atomic_write(&path, serialize_task(task).as_bytes())?;
  crate::written::record(&self.root, &path);
  Ok(())
  ```
- `src/repo.rs` `publish_task_file`: after the `hard_link` succeeds, `crate::written::record(&self.root, &self.task_path(&task.id));`. Read the function first: record only on the success path, never on the collision retry.
- `src/commands/attach.rs`: after `attachments::write_new(&target, &bytes)?;` (~138), record `&target` under the project root. After the detach `std::fs::remove_file(&target)?;` (~206), record `&target` too, since a deletion is also a write. On the cleanup path at ~148 (a failed attach removing its own file), record nothing.
- `src/moving/mod.rs`: record the copied attachment after `attachments::write_new` (~226) under the target root. Record the tombstone after `atomic_write(&source_path, …)` (~270) under the source root. Record each removed source attachment after `remove_file(path)?` (~281) under the source root. `create_task_exact` records through `publish_task_file`. Read it first and confirm it calls `publish_task_file`; if it does not, record there.

- [ ] **Step 6: Run the fast suite and commit**

Run: `just test-fast`, expected all pass. The journal only collects at this point.

```bash
git add src/written.rs src/main.rs src/repo.rs src/commands/attach.rs src/moving/mod.rs
git commit -m "feat(written): journal the files each command writes"
```

---

### Task 3: The commit engine

**Files:**
- Create: `src/record_commit.rs`
- Modify: `src/main.rs` (`mod record_commit;`)

**Interfaces:**
- Consumes: `crate::claims::{ClaimStore, MutationLock}`: `ClaimStore::path_for(&str) -> Result<PathBuf>` and `MutationLock::acquire_at(&Path) -> Result<MutationLock>`.
- Produces:
  - `pub fn commit_paths(root: &Path, paths: &[PathBuf], message: &str) -> Vec<String>`. `paths` are root-relative. It returns warnings, and an empty vector means committed or nothing to commit.
  - `pub(crate) fn commit_paths_in(root: &Path, paths: &[PathBuf], message: &str, state_home: &Path, before_commit: &mut dyn FnMut()) -> Vec<String>`, the same function with the state directory passed in and a test seam, `before_commit`, that runs after staging and before the final staged-set check. `commit_paths` resolves the state home (`XDG_STATE_HOME`, else `$HOME/.local/state`, as `ClaimStore::path_with` does) and calls it.
  - `pub fn message(verb: &str, ids: &[String], title: Option<&str>) -> String`.

- [ ] **Step 1: Write the failing tests** (`src/record_commit.rs`, `mod tests`)

```rust
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
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
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
        commit_paths_in(root, paths, "chore(tasks): note tst-000001", state, &mut || {})
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
        assert_eq!(git(root, &["log", "-1", "--format=%s"]).trim(), "chore(tasks): note tst-000001");
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
        assert_eq!(git(root, &["show", ":tasks/tst-000001.md"]), "staged version");
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
        assert!(warnings[0].starts_with("commit_deferred:") && warnings[0].contains("merge"), "{warnings:?}");
        std::fs::remove_file(root.join(".git/MERGE_HEAD")).unwrap();
        git(root, &["checkout", "-q", "--detach"]);
        let warnings = commit(root, &[record], state.path());
        assert!(warnings[0].starts_with("commit_deferred:") && warnings[0].contains("detached"), "{warnings:?}");
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
        assert!(files.contains("derived.txt") && files.contains("tasks/tst-000001.md"), "{files}");
        assert_eq!(staged(root), "");
    }

    #[test]
    fn a_refusing_hook_unstages_only_the_records_and_keeps_outside_staging() {
        let dir = repo(true);
        let root = dir.path();
        let state = state_home();
        // The hook stands in for a separate git process staging while the hook runs: the
        // CLI cannot tell the two apart, which is the point of the test.
        hook(root, "echo refused >&2; echo x > other.txt; git add other.txt; exit 1");
        let record = write(root, "tasks/tst-000001.md", "a");
        let warnings = commit(root, &[record], state.path());
        assert!(warnings[0].starts_with("commit_failed:"), "{warnings:?}");
        assert!(warnings[0].contains("refused"), "{warnings:?}");
        assert!(warnings[0].contains("cleanup needs attention") && warnings[0].contains("other.txt"), "{warnings:?}");
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
        assert!(warnings[0].starts_with("commit_failed:") && warnings[0].contains("index locked"), "{warnings:?}");
        assert_eq!(staged(root), "");
    }

    #[test]
    fn outside_git_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_home();
        let record = write(dir.path(), "tasks/tst-000001.md", "a");
        let warnings = commit(dir.path(), &[record], state.path());
        assert!(warnings[0].starts_with("commit_unavailable:"), "{warnings:?}");
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
        assert_eq!(commit(root, &[kept, spaced], state.path()), Vec::<String>::new());
        let files = last_files(root);
        assert!(files.contains("tasks/files/tst-000001/a b.png") && files.contains("gone.png"), "{files}");
        assert_eq!(git(root, &["status", "--porcelain"]), "");
    }

    #[test]
    fn a_linked_worktree_ignores_a_merge_in_its_main_checkout() {
        let dir = repo(true);
        let main = dir.path();
        let state = state_home();
        let wt = main.join("wt");
        git(main, &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()]);
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
                        let record = write(&root, &format!("tasks/tst-00000{n}.md"), &format!("{i}"));
                        let message = format!("chore(tasks): note tst-00000{n}");
                        let warnings = commit_paths_in(&root, &[record], &message, &state, &mut || {});
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
    fn the_message_names_verb_and_ids_and_truncates_a_title_to_72() {
        assert_eq!(message("note", &["tst-000001".into()], None), "chore(tasks): note tst-000001");
        let long = "x".repeat(100);
        let subject = message("add", &["tst-000001".into()], Some(&long));
        assert_eq!(subject.chars().count(), 72);
        assert!(subject.starts_with("chore(tasks): add tst-000001 xxx") && subject.ends_with('…'));
        assert_eq!(
            message("add", &["tst-000001".into()], Some("Short")),
            "chore(tasks): add tst-000001 Short"
        );
    }
}
```

The thread test runs two writers in one process through the repository lock. `flock` locks are per open file description, so two threads opening the lock file separately do exclude each other.

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-one record_commit`
Expected: compile errors (module missing).

- [ ] **Step 3: Implement `src/record_commit.rs`**

```rust
//! Commits the files a tasks command wrote (docs/specs/2026-10-10-record-commit-design.md
//! §3.3 and §3.7): a plain commit over an index holding nothing else, under a lock every
//! tasks writer in the repository shares, unstaging only its own paths when it fails.
use crate::claims::MutationLock;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SHOWN: usize = 5;
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

fn state_home() -> Result<PathBuf, String> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state"))
        .ok_or_else(|| "neither XDG_STATE_HOME nor HOME is set".to_string())
}

pub fn commit_paths(root: &Path, paths: &[PathBuf], message: &str) -> Vec<String> {
    match state_home() {
        Ok(state) => commit_paths_in(root, paths, message, &state, &mut || {}),
        Err(error) => vec![failed(root, paths, &error, &[])],
    }
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git").args(args).env("LC_ALL", "C").current_dir(dir).output()
}

fn stderr_tail(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].join(" / ")
}

struct Repo {
    toplevel: PathBuf,
    git_dir: PathBuf,
    common: PathBuf,
}

/// `Ok(None)` when the root is outside git or there is no git executable.
fn discover(root: &Path) -> Result<Option<Repo>, String> {
    let output = match git(root, &["rev-parse", "--show-toplevel", "--absolute-git-dir", "--git-common-dir"]) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !output.status.success() {
        let tail = stderr_tail(&output);
        return if tail.contains("not a git repository") { Ok(None) } else { Err(tail) };
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let mut next = || lines.next().map(PathBuf::from).ok_or("git rev-parse printed too little");
    let toplevel = next()?;
    let git_dir = next()?;
    let common = root.join(next()?); // relative to the cwd when git prints it relative
    let canonical = |path: PathBuf| path.canonicalize().map_err(|error| format!("{}: {error}", path.display()));
    Ok(Some(Repo {
        toplevel: canonical(toplevel)?,
        git_dir: canonical(git_dir)?,
        common: canonical(common)?,
    }))
}

fn lock_for(common: &Path, state: &Path) -> Result<MutationLock, String> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(common.as_os_str().as_encoded_bytes());
    let hex: String = digest[..8].iter().map(|byte| format!("{byte:02x}")).collect();
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
    let output = git(&repo.toplevel, &["diff", "--cached", "--name-only", "--no-renames", "-z"])
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
    git(&repo.toplevel, &["rev-parse", "-q", "--verify", "HEAD"]).is_ok_and(|output| output.status.success())
}

/// Puts the index back to HEAD for `ours`, and only for `ours`.
fn unstage(repo: &Repo, ours: &[String]) {
    let mut args: Vec<&str> = if head_exists(repo) {
        vec!["reset", "-q", "--"]
    } else {
        vec!["rm", "--cached", "-q", "-r", "--ignore-unmatch", "--"]
    };
    args.extend(ours.iter().map(String::as_str));
    let _ = git(&repo.toplevel, &args);
}

fn listed(paths: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let all: Vec<String> = paths.into_iter().map(|path| path.as_ref().to_string()).collect();
    let mut shown = all.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
    if all.len() > SHOWN {
        shown.push_str(&format!(" and {} more", all.len() - SHOWN));
    }
    shown
}

fn rel(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| path.display().to_string()).collect()
}

fn deferred(root: &Path, paths: &[PathBuf], cause: &str) -> String {
    format!(
        "commit_deferred: {}: {cause}; left uncommitted: {}; run `tasks commit` there once that is cleared",
        root.display(),
        listed(rel(paths))
    )
}

fn failed(root: &Path, paths: &[PathBuf], cause: &str, still_staged: &[String]) -> String {
    let mut text = format!(
        "commit_failed: {}: {cause}; left uncommitted: {}; for a refusing hook see the record-commit routes in the tasks skill, then run `tasks commit` there",
        root.display(),
        listed(rel(paths))
    );
    if !still_staged.is_empty() {
        text.push_str(&format!("; cleanup needs attention: still staged: {}", listed(still_staged)));
    }
    text
}

fn cause_of(output: &Output) -> String {
    let tail = stderr_tail(output);
    if tail.contains("index.lock") { format!("index locked ({tail})") } else { tail }
}

pub(crate) fn commit_paths_in(
    root: &Path,
    paths: &[PathBuf],
    message: &str,
    state: &Path,
    before_commit: &mut dyn FnMut(),
) -> Vec<String> {
    let repo = match discover(root) {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return vec![format!(
                "commit_unavailable: {} is not in a git repository (or git is missing); left uncommitted: {}",
                root.display(),
                listed(rel(paths))
            )];
        }
        Err(error) => return vec![failed(root, paths, &error, &[])],
    };
    let _lock = match lock_for(&repo.common, state) {
        Ok(lock) => lock,
        Err(error) => return vec![failed(root, paths, &error, &[])],
    };
    if let Some(what) = in_progress(&repo.git_dir) {
        return vec![deferred(root, paths, &format!("{what} is in progress"))];
    }
    if !git(&repo.toplevel, &["symbolic-ref", "-q", "HEAD"]).is_ok_and(|output| output.status.success()) {
        return vec![deferred(root, paths, "HEAD is detached")];
    }
    match staged(&repo) {
        Ok(already) if !already.is_empty() => {
            return vec![deferred(root, paths, &format!("staged changes: {}", listed(&already)))];
        }
        Ok(_) => {}
        Err(error) => return vec![failed(root, paths, &error, &[])],
    }
    let Ok(prefix) = root.canonicalize().map(|root| root.strip_prefix(&repo.toplevel).map(Path::to_path_buf)) else {
        return vec![failed(root, paths, "cannot canonicalize the project root", &[])];
    };
    let Ok(prefix) = prefix else {
        return vec![failed(root, paths, "the project root is not under the repository top level", &[])];
    };
    let ours: Vec<String> = paths.iter().map(|path| prefix.join(path).display().to_string()).collect();
    let (present, missing): (Vec<&String>, Vec<&String>) =
        ours.iter().partition(|path| repo.toplevel.join(path).exists());
    for (base, group) in [
        (vec!["add", "-A", "--"], present),
        (vec!["rm", "--cached", "-q", "-r", "--ignore-unmatch", "--"], missing),
    ] {
        if group.is_empty() {
            continue;
        }
        let mut args = base;
        args.extend(group.iter().map(|path| path.as_str()));
        match git(&repo.toplevel, &args) {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                unstage(&repo, &ours);
                return vec![failed(root, paths, &cause_of(&output), &[])];
            }
            Err(error) => {
                unstage(&repo, &ours);
                return vec![failed(root, paths, &error.to_string(), &[])];
            }
        }
    }
    before_commit();
    let now = match staged(&repo) {
        Ok(now) => now,
        Err(error) => {
            unstage(&repo, &ours);
            return vec![failed(root, paths, &error, &[])];
        }
    };
    if now.is_empty() {
        return Vec::new(); // the files already match HEAD
    }
    let foreign: Vec<&String> = now.iter().filter(|path| !ours.contains(path)).collect();
    if !foreign.is_empty() {
        unstage(&repo, &ours);
        return vec![deferred(root, paths, &format!("staged changes appeared: {}", listed(&foreign)))];
    }
    match git(&repo.toplevel, &["commit", "-q", "-m", message]) {
        Ok(output) if output.status.success() => Vec::new(),
        result => {
            let cause = match &result {
                Ok(output) => cause_of(output),
                Err(error) => error.to_string(),
            };
            unstage(&repo, &ours);
            let still: Vec<String> = staged(&repo).unwrap_or_default().into_iter().collect();
            vec![failed(root, paths, &cause, &still)]
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `just test-one record_commit`
Expected: all PASS. If `a_held_index_lock_fails…` reports a different cause string, read git's stderr in the failure and adjust `cause_of`. The warning must still contain `index locked`.

- [ ] **Step 5: Run the fast suite and commit**

```bash
just test-fast
git add src/record_commit.rs src/main.rs
git commit -m "feat(record-commit): commit written paths under a repository lock"
```

---

### Task 4: Commit at the end of every write command

**Files:**
- Modify: `src/cli.rs` (`impl Command`, beside `pub enum Command` at ~360), `src/commands/mod.rs` (`run` at ~1757, `id_out` at ~1226), `src/commands/add.rs` (~99, ~110), `src/commands/feedback.rs` (~228), `src/commands/attach.rs` (~168, ~217), `src/commands/move_.rs` (~91), `src/record_commit.rs`
- Test: `tests/record_commit.rs` (create)

**Interfaces:**
- Consumes: `written::{begin, take}`, `record_commit::{commit_paths, message}`, `Project::commit_records`, `Project::open`, `TaskId::parse(&str) -> Result<TaskId>`, `Project::read_task`.
- Produces:
  - `pub fn Command::commit_verb(&self) -> Option<&'static str>`.
  - `pub fn record_commit::commit_written(warnings: &mut Vec<String>)`, which drains the journal and commits each opted-in root.
  - `pub fn commands::finish(ctx: &mut Ctx)`, which calls `commit_written(&mut ctx.warnings)`.

- [ ] **Step 1: Write the failing integration tests** (`tests/record_commit.rs`)

```rust
mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [("user.name", "t"), ("user.email", "t@t"), ("commit.gpgsign", "false")] {
        git(dir, &["config", key, value]);
    }
}

/// A tasks project at the top of a fresh repository, its init committed. `commit`
/// writes `[git] commit = true` into the config first.
fn project(env: &mut TestEnv, prefix: &str, commit: bool) -> PathBuf {
    let root = env.init(prefix);
    git_init(&root);
    if commit {
        let config = root.join("tasks/.config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "init"]);
    root
}

fn subjects(root: &Path) -> Vec<String> {
    git(root, &["log", "--format=%s"]).lines().map(String::from).collect()
}

fn last_files(root: &Path) -> Vec<String> {
    git(root, &["show", "--name-only", "--format=", "HEAD"]).lines().map(String::from).collect()
}

fn clean(root: &Path) -> bool {
    git(root, &["status", "--porcelain"]).is_empty()
}

fn add(env: &TestEnv, root: &Path, title: &str) -> String {
    env.json(root, &["add", title])["id"].as_str().unwrap().to_string()
}

#[test]
fn without_the_switch_nothing_is_committed() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    add(&env, &root, "a task");
    assert_eq!(subjects(&root), vec!["init"]);
    assert!(!clean(&root));
}

#[test]
fn add_note_and_done_each_commit_their_record() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    assert_eq!(subjects(&root)[0], format!("chore(tasks): add {id} a task"));
    assert_eq!(last_files(&root), vec![format!("tasks/{id}.md")]);
    env.json(&root, &["note", &id, "a note"]);
    assert_eq!(subjects(&root)[0], format!("chore(tasks): note {id}"));
    env.json(&root, &["start", &id]);
    env.json(&root, &["done", &id, "landed"]);
    assert_eq!(subjects(&root)[0], format!("chore(tasks): done {id}"));
    assert!(clean(&root));
}

#[test]
fn attach_and_detach_commit_the_file_with_the_record() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    let source = root.join("shot.png");
    std::fs::write(&source, "png").unwrap();
    env.json(&root, &["attach", &id, source.to_str().unwrap()]);
    std::fs::remove_file(&source).unwrap();
    let files = last_files(&root);
    assert!(files.contains(&format!("tasks/{id}.md")) && files.contains(&format!("tasks/files/{id}/shot.png")), "{files:?}");
    env.json(&root, &["detach", &id, "shot.png", "wrong one"]);
    assert!(last_files(&root).contains(&format!("tasks/files/{id}/shot.png")));
    assert!(clean(&root));
}

#[test]
fn a_staged_file_defers_the_commit_with_a_warning_and_exit_zero() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    std::fs::write(root.join("draft.txt"), "x").unwrap();
    git(&root, &["add", "draft.txt"]);
    let out = env.json(&root, &["add", "a task"]);
    let warnings = out["warnings"].to_string();
    assert!(warnings.contains("commit_deferred"), "{warnings}");
    assert_eq!(subjects(&root), vec!["init"]);
    assert_eq!(git(&root, &["diff", "--cached", "--name-only"]).trim(), "draft.txt");
}

#[test]
fn a_routed_note_commits_in_the_target_project() {
    let mut env = TestEnv::new();
    let home = project(&mut env, "aaa", true);
    let other = project(&mut env, "bbb", true);
    let id = add(&env, &other, "theirs");
    env.json(&home, &["note", &id, "from aaa"]);
    assert_eq!(subjects(&other)[0], format!("chore(tasks): note {id}"));
    assert_eq!(subjects(&home), vec!["init"]);
}

#[test]
fn move_commits_the_tombstone_and_the_new_record_in_their_own_roots() {
    let mut env = TestEnv::new();
    let source = project(&mut env, "aaa", true);
    let target = project(&mut env, "bbb", true);
    let id = add(&env, &source, "travelling");
    let out = env.json(&source, &["move", &id, "--to", "bbb"]);
    let new = out["id"].as_str().unwrap().to_string();
    assert_eq!(subjects(&source)[0], format!("chore(tasks): move {id}"));
    assert!(last_files(&source).contains(&format!("tasks/{id}.md")));
    assert_eq!(subjects(&target)[0], format!("chore(tasks): move {new}"));
    assert!(last_files(&target).contains(&format!("tasks/{new}.md")));
    assert!(clean(&source) && clean(&target));
}

#[test]
fn feedback_commits_in_the_owner_project() {
    let mut env = TestEnv::new();
    let reporter = project(&mut env, "aaa", true);
    let owner = project(&mut env, "own", true);
    let config = owner.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("[git]", "[feedback]\nscope = \"the tool\"\n\n[git]")).unwrap();
    git(&owner, &["commit", "-q", "-am", "accept feedback"]);
    let out = env.json(&reporter, &["feedback", "--project", "own", "a rough edge", "--category", "friction"]);
    let id = out["id"].as_str().unwrap();
    assert!(subjects(&owner)[0].starts_with(&format!("chore(tasks): feedback {id} a rough edge")), "{:?}", subjects(&owner));
    assert!(clean(&owner));
}

#[test]
fn a_root_outside_git_warns_unavailable() {
    let mut env = TestEnv::new();
    let root = env.init("tst");
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    let out = env.json(&root, &["add", "a task"]);
    assert!(out["warnings"].to_string().contains("commit_unavailable"), "{out}");
}

#[test]
fn start_past_a_halt_commits_both_records() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let halt = env.json(&root, &["add", "stop the line", "-p", "0", "--tag", "halt"])["id"].as_str().unwrap().to_string();
    let id = env.json(&root, &["add", "other work", "-p", "2"])["id"].as_str().unwrap().to_string();
    env.json(&root, &["start", &id, "--force", "--reason", "unrelated"]);
    let files = last_files(&root);
    assert!(files.contains(&format!("tasks/{id}.md")) && files.contains(&format!("tasks/{halt}.md")), "{files:?}");
    assert!(clean(&root));
}

#[test]
fn two_projects_in_one_repository_commit_concurrently_without_mixing() {
    let mut env = TestEnv::new();
    let top = tempfile::tempdir().unwrap();
    let top = top.path().canonicalize().unwrap();
    git_init(&top);
    let mut roots = Vec::new();
    for prefix in ["aaa", "bbb"] {
        let root = top.join(prefix);
        std::fs::create_dir_all(&root).unwrap();
        env.json(&root, &["init", "--prefix", prefix]);
        let config = root.join("tasks/.config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
        roots.push(root);
    }
    git(&top, &["add", "-A"]);
    git(&top, &["commit", "-q", "-m", "init"]);
    let ids: Vec<String> = roots.iter().map(|root| add(&env, root, "work")).collect();
    let mut children = Vec::new();
    for (root, id) in roots.iter().zip(&ids) {
        for n in 0..4 {
            children.push(env.raw(root).args(["note", id, &format!("note {n}")]).spawn().unwrap());
        }
    }
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    // Every note commit names one id and touches only that project's directory.
    let log = git(&top, &["log", "--format=@%s", "--name-only"]);
    let mut notes = 0;
    for block in log.split('@').filter(|block| block.starts_with("chore(tasks): note ")) {
        let mut lines = block.lines().filter(|line| !line.is_empty());
        let id = lines.next().unwrap().trim_start_matches("chore(tasks): note ");
        let project = &id[..3];
        let files: Vec<_> = lines.collect();
        assert_eq!(files, vec![format!("{project}/tasks/{id}.md")], "{log}");
        notes += 1;
    }
    assert_eq!(notes, 8, "{log}");
    assert!(clean(&top));
}
```

Add one test for a linked project (overlay repository). Copy the overlay fixture setup from `tests/linked.rs`: its helper that builds an overlay directory with `[checkout] path` and a code checkout. Set `[git] commit = true` in the overlay config, make the overlay a git repository with an initial commit, run `tasks add` from inside the code checkout, and assert that the overlay's HEAD subject is `chore(tasks): add <id> …` and the code checkout's log is unchanged. Before writing it, read `tests/linked.rs` to reuse its helper names exactly.

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-one --test record_commit`
Expected: `without_the_switch…` and `a_root_outside_git…` may pass or fail; every commit-asserting test FAILS (subjects stay `["init"]`).

- [ ] **Step 3: Implement the verb mapping** (`src/cli.rs`)

```rust
impl Command {
    /// The verb a record commit names for this command, or `None` for a command that never
    /// writes a record (record-commit spec §3.2). `init`, `rename`, `retire`, `unretire` and
    /// `unregister` keep their own procedures and commit nothing.
    pub fn commit_verb(&self) -> Option<&'static str> {
        Some(match self {
            Command::Add { .. } => "add",
            Command::Edit { .. } => "edit",
            Command::Note { .. } => "note",
            Command::Dep { .. } => "dep",
            Command::Start { .. } => "start",
            Command::Park { .. } => "park",
            Command::Done { .. } => "done",
            Command::Drop { .. } => "drop",
            Command::Block { .. } => "block",
            Command::Unblock { .. } => "unblock",
            Command::Shelve { .. } => "shelve",
            Command::Unshelve { .. } => "unshelve",
            Command::Attach { .. } => "attach",
            Command::Detach { .. } => "detach",
            Command::Feedback { .. } => "feedback",
            Command::Move { .. } => "move",
            _ => return None,
        })
    }
}
```

- [ ] **Step 4: Implement `commit_written`** (append to `src/record_commit.rs`)

```rust
/// Drains the journal and commits each root whose config opts in. Called by a write
/// command's finishing point while its locks are still held (spec §3.2, §3.7).
pub fn commit_written(warnings: &mut Vec<String>) {
    let written = crate::written::take();
    let Some(verb) = written.verb else {
        return;
    };
    for (root, paths) in written.roots {
        let project = match crate::repo::Project::open(&root) {
            Ok(project) => project,
            Err(error) => {
                warnings.push(format!("commit_failed: {}: cannot read its config ({error})", root.display()));
                continue;
            }
        };
        if !project.commit_records {
            continue;
        }
        let ids = ids_of(&paths);
        let title = match verb {
            "add" | "feedback" => ids
                .first()
                .and_then(|id| crate::model::TaskId::parse(id).ok())
                .and_then(|id| project.read_task(&id).ok())
                .map(|task| task.title),
            _ => None,
        };
        let paths: Vec<PathBuf> = paths.into_iter().collect();
        warnings.extend(commit_paths(&root, &paths, &message(verb, &ids, title.as_deref())));
    }
}

/// The task ids a set of written paths belongs to: `tasks/<id>.md` and
/// `tasks/files/<id>/…`, sorted and deduplicated.
fn ids_of(paths: &BTreeSet<PathBuf>) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for path in paths {
        let parts: Vec<_> = path.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
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
```

Add a unit test in `record_commit`'s tests:

```rust
#[test]
fn ids_of_reads_records_and_attachment_dirs() {
    let paths: BTreeSet<PathBuf> = ["tasks/tst-000002.md", "tasks/files/tst-000001/a b.png", "tasks/tst-000001.md"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    assert_eq!(ids_of(&paths), vec!["tst-000001".to_string(), "tst-000002".to_string()]);
}
```

Confirm `TaskId::parse` and the `title` field name against `src/model.rs` before compiling.

- [ ] **Step 5: Wire the finishing points**

In `src/commands/mod.rs`:

```rust
/// Commits what this write command wrote, while its locks are still held (record-commit
/// spec §3.2, §3.7). Every write command's last step before building its output.
pub fn finish(ctx: &mut Ctx) {
    crate::record_commit::commit_written(&mut ctx.warnings);
}

pub fn id_out(mut ctx: Ctx, task: &Task) -> Output {
    finish(&mut ctx);
    Output::Id(crate::output::IdOut {
        id: task.id.to_string(),
        warnings: ctx.warnings,
    })
}
```

Rename the existing `pub fn run(cli: Cli) -> Result<Output>` to `fn dispatch(cli: Cli) -> Result<Output>`, and add:

```rust
pub fn run(cli: Cli) -> Result<Output> {
    crate::written::begin(cli.command.commit_verb());
    let result = dispatch(cli);
    // A command that failed after writing leaves its files uncommitted; `prime` lists
    // them. A successful one must have reached `finish`: anything left is a write path
    // that skipped it, which the debug build refuses so the test suite catches it.
    let left = crate::written::take();
    if result.is_ok() && left.verb.is_some() {
        debug_assert!(
            left.roots.is_empty(),
            "written but never committed: {:?}",
            left.roots
        );
    }
    result
}
```

Call `finish(&mut ctx);` immediately before each output literal that moves `ctx.warnings` in a write command: `src/commands/add.rs` (~99 and ~110), `src/commands/feedback.rs` (~228), and `src/commands/attach.rs` (~168 and ~217). Each of these functions owns `ctx` by value, so make the binding `mut` where needed.

In `src/commands/move_.rs` `run`, inside the locked loop, replace `return decide(&mut refreshed, false).map(Output::Move);` with:

```rust
let mut out = decide(&mut refreshed, false)?;
crate::record_commit::commit_written(&mut out.warnings);
return Ok(Output::Move(out));
```

- [ ] **Step 6: Run the tests**

Run: `just test-one --test record_commit`, expected all PASS.
Run: `just test-fast`, expected all PASS. If the `debug_assert` fires in an existing test, a write command returns without `finish`. Add the call at that command's output site, not a broader fallback.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/commands/ src/record_commit.rs tests/record_commit.rs
git commit -m "feat(record-commit): commit each write command's records"
```

---

### Task 5: `tasks commit`

**Files:**
- Modify (ops, first): `cli.toml` in the ops checkout
- Modify: `tools/cli.toml` (adopted, never hand-edited), `src/cli.rs` (enum `Command`), `src/commands/mod.rs` (dispatch), `src/output.rs` (new `CommitOut`)
- Create: `src/commands/commit.rs`
- Test: `tests/record_commit.rs`

**Interfaces:**
- Consumes: `Project::uncommitted_task_files() -> Result<Option<Vec<String>>>`, `record_commit::commit_paths`, `lock_and_revalidate`, `Routing`.
- Produces: `Command::Commit { project: Option<String> }`, `Output::Commit(CommitOut { committed: usize, paths: Vec<String>, warnings: Vec<String> })`.

- [ ] **Step 1: Land the cli.toml row in ops**

```bash
OPS=$(tasks root ops-0a6ba1 | python3 -c 'import json,sys;print(json.load(sys.stdin)["root"])')
cd "$OPS" && work-link --ensure .worktrees && git worktree add -q -b feat/tasks-commit-row .worktrees/tasks-commit-row
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/tasks-commit-row
```

In `.worktrees/tasks-commit-row/cli.toml`, after the `[[cli.tasks.commands]]` row with `path = ["check"]`, add:

```toml
[[cli.tasks.commands]]
path = ["commit"]
summary = "Commit every uncommitted file under tasks/ in one commit"
options = [
  { shared = "project", role = "select", value = "string", repeatable = false },
]
```

Run `just test-one tests.test_cli` there, then `just test-fast`. Commit with `git commit -m "feat(cli): add the tasks commit command row" -- cli.toml`. Merge into ops main (a personal repository, so a local merge is in scope): `git -C "$OPS" merge --no-ff feat/tasks-commit-row`. Run `tt-report`, then unlock and remove the ops worktree.

- [ ] **Step 2: Adopt it into tasks**

From this worktree: `python3 "$OPS/bin/vendored" adopt cli.toml`. Expected: `tools/cli.toml` changes by exactly the new row.

- [ ] **Step 3: Write the failing tests** (append to `tests/record_commit.rs`)

```rust
#[test]
fn tasks_commit_sweeps_the_backlog_into_one_commit_even_without_the_switch() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let a = add(&env, &root, "one");
    let b = add(&env, &root, "two");
    std::fs::write(root.join("tasks/tst-ffffff.md.tmp"), "transient").unwrap();
    let out = env.json(&root, &["commit"]);
    assert_eq!(out["committed"], 2, "{out}");
    assert_eq!(subjects(&root)[0], "chore(tasks): commit 2 records");
    let files = last_files(&root);
    assert!(files.contains(&format!("tasks/{a}.md")) && files.contains(&format!("tasks/{b}.md")), "{files:?}");
    assert!(!files.iter().any(|file| file.ends_with(".tmp")), "{files:?}");
}

#[test]
fn tasks_commit_with_nothing_to_commit_says_so() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let out = env.json(&root, &["commit"]);
    assert_eq!(out["committed"], 0, "{out}");
    assert_eq!(subjects(&root), vec!["init"]);
}

#[test]
fn tasks_commit_refuses_beside_staged_work_and_exits_one() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    add(&env, &root, "one");
    std::fs::write(root.join("draft.txt"), "x").unwrap();
    git(&root, &["add", "draft.txt"]);
    let error = env.fail(&root, &["commit"]);
    assert!(error.contains("commit_deferred"), "{error}");
}

#[test]
fn tasks_commit_takes_a_project() {
    let mut env = TestEnv::new();
    let here = project(&mut env, "aaa", false);
    let there = project(&mut env, "bbb", false);
    add(&env, &there, "theirs");
    let out = env.json(&here, &["commit", "--project", "bbb"]);
    assert_eq!(out["committed"], 1, "{out}");
    assert_eq!(subjects(&there)[0], "chore(tasks): commit 1 records");
}
```

`.tmp` files under `tasks/` are skipped by `uncommitted_task_files`, which ignores names ending in `.tmp` and `.edit.md`.

- [ ] **Step 4: Run tests to verify they fail**

Run: `just test-one --test record_commit tasks_commit`
Expected: FAIL (`unrecognized subcommand 'commit'`).

- [ ] **Step 5: Implement**

`src/cli.rs`, in `enum Command` after `Check`:

```rust
    /// Commit every uncommitted file under tasks/ in one commit
    Commit {
        /// One registered project, by prefix
        #[arg(long)]
        project: Option<String>,
    },
```

Match the doc-comment style of the neighbouring variants. The summary must equal the cli.toml row's `summary`; the conformance test in `tests/cli.rs` compares them.

`src/output.rs`: add beside `IdOut`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct CommitOut {
    pub committed: usize,
    pub paths: Vec<String>,
    pub warnings: Vec<String>,
}
```

Add the `Output::Commit(CommitOut)` variant, and handle it in every `match` on `Output` the compiler reports. In `warnings_of`, use `Output::Commit(o) => o.warnings.clone()`. The pretty render is `committed N records` when N > 0, else `nothing to commit`. JSON serializes the struct, as `IdOut` does. Follow `IdOut`'s arms for anything else.

`src/commands/commit.rs`:

```rust
//! `tasks commit`: every uncommitted file under tasks/ in one commit, through the record
//! commit's procedure (record-commit spec §3.5). An explicit request, so it ignores the
//! `[git] commit` switch and fails loudly when it cannot commit.
use super::Ctx;
use crate::error::{Error, Result};
use crate::output::{CommitOut, Output};
use std::path::PathBuf;

pub fn run(ctx: Ctx) -> Result<Output> {
    let Some(files) = ctx.project.uncommitted_task_files()? else {
        return Err(Error::Validation(format!(
            "commit_unavailable: {} is not in a git repository (or git is missing)",
            ctx.project.root.display()
        )));
    };
    if files.is_empty() {
        return Ok(Output::Commit(CommitOut { committed: 0, paths: Vec::new(), warnings: ctx.warnings }));
    }
    let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    let message = format!("chore(tasks): commit {} records", paths.len());
    let refused = crate::record_commit::commit_paths(&ctx.project.root, &paths, &message);
    if !refused.is_empty() {
        return Err(Error::Validation(refused.join("\n")));
    }
    Ok(Output::Commit(CommitOut { committed: files.len(), paths: files, warnings: ctx.warnings }))
}
```

Add `pub mod commit;` in `src/commands/mod.rs`. Dispatch it the way `Command::Add` builds its context: route to the named project, then take the lock.

```rust
        Command::Commit { project } => {
            let routing = project.clone().map_or(Routing::Local, Routing::Registered);
            let mut ctx = match project {
                Some(prefix) => {
                    let registry = Registry::load()?;
                    let project = crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
                    let shorthand = Shorthand::new(start_dir(dir)?);
                    Ctx::new(project, registry, routing.clone(), shorthand)
                }
                None => open_ctx(dir)?,
            };
            lock_and_revalidate(&mut ctx, &routing)?;
            commit::run(ctx)
        }
```

`commit_verb` returns `None` for `Commit`, so the journal stays empty and the `debug_assert` holds.

- [ ] **Step 6: Run the tests**

Run: `just test-one --test record_commit tasks_commit`, expected PASS.
Run: `just test-fast`, expected PASS, including the cli.toml conformance test in `tests/cli.rs`.

- [ ] **Step 7: Commit**

```bash
git add tools/cli.toml src/cli.rs src/commands/commit.rs src/commands/mod.rs src/output.rs tests/record_commit.rs
git commit -m "feat(commit): add tasks commit for the uncommitted backlog"
```

---

### Task 6: Instructions and README

**Files:**
- Modify: `skills/tasks/SKILL.md`, `skills/scope/SKILL.md`, `AGENTS.md`, `README.md`

Find every passage first, so none is missed:

```bash
grep -n -iE 'same commit|uncommitted|commit the task record|commit it before|commits neither' skills/tasks/SKILL.md skills/scope/SKILL.md AGENTS.md README.md
```

- [ ] **Step 1: `skills/tasks/SKILL.md`**
  - Session protocol step 6: replace "in the same commit as the code" with "on the task's branch before it merges (under `[git] commit` the CLI commits the record itself; otherwise commit it with the code)".
  - The linked-project paragraph ("the matching commit of `<prefix>/tasks/`…"): keep the overlay rule and drop "in the same commit". Records commit in the overlay repository, after the code commit, and under `[git] commit` the CLI makes that commit.
  - **Process and workspace**: after "commit the task record before creating one with `git worktree add`", add "(automatic under `[git] commit`: `add` and `start` commit it)".
  - Add a short subsection under **Recording work** titled `### Record commits`. It covers the `[git] commit` switch; that each write commits only the files it wrote; the three warning prefixes and what each means; that a write still exits 0 when its commit is skipped; `cleanup needs attention`; and `tasks commit [--project <p>]` as the recovery and backlog sweep. Keep it under 15 lines, in the skill's voice.
  - Feedback, both paragraphs. "The report lands as an uncommitted `idea`" becomes "lands as an `idea` … (committed there when the owner sets `[git] commit`)". In "In a project that accepts feedback", "Uncommitted files under `tasks/` tagged `feedback` are unreviewed reports … then commit" becomes "Ideas tagged `feedback` with no triage note are unreviewed reports: read each, and redact anything that describes a project rather than the tool with `tasks edit` (history keeps the original)".
  - The `move` bullet, "The move commits neither repository: commit each root its `paths` lists", becomes "Under `[git] commit` the move commits each root; otherwise commit each root its `paths` lists".
- [ ] **Step 2: `skills/scope/SKILL.md`** lines ~140 and ~169: "in the same commit as this result" and "in the same commit" become "before this result's branch merges".
- [ ] **Step 3: `AGENTS.md`** line ~12: "in the same commit as the code" becomes "on the task's branch before it merges". Line ~17: "in this repo, review uncommitted feedback files before committing them" becomes "in this repo, review feedback ideas without a triage note and redact with `tasks edit`".
- [ ] **Step 4: `README.md`**: make the same replacements at ~54 and ~74, and the feedback paragraph at ~766. Under the configuration section, document the `[git]` table, with the opt-in, what is committed, the warnings, and the rationale in one line each. Add a `tasks commit` entry beside `tasks check` in the command reference. Update the `move` text wherever it says the move commits neither repository.
- [ ] **Step 5: Verify and commit**

Run: `just check` (it runs `tasks check` and the guide checks), expected clean. Then run the grep from the top of this task again: every remaining hit must be deliberate (for example, "uncommitted" describing `prime`'s warning).

```bash
git add skills/tasks/SKILL.md skills/scope/SKILL.md AGENTS.md README.md
git commit -m "docs: record commits replace the same-commit and uncommitted-feedback rules"
```

---

### Task 7: File the rollout pieces

Task-record maintenance only, done after the branch merges into tasks main. No code.

- [ ] **Step 1: lore piece**

```bash
tasks add "Retire the same-commit and record-before-worktree rules in the instruction provenance for record commits" --project lore -p 1 --size s --complexity low --process direct --agent claude-code/claude-opus-5-5 -b "tasks-6939f6 landed [git] commit: the CLI commits each record write itself (tasks docs/specs/2026-10-10-record-commit-design.md §3.4, §3.6). Update docs/instruction-provenance.md rows T12 (done in the same commit as the code -> on the task's branch before it merges) and T21 (commit the record before git worktree add -> automatic under [git] commit), and any assembled instruction that repeats the same-commit rule or the uncommitted-feedback review."
```

- [ ] **Step 2: ops piece, depending on lore's**

```bash
tasks add "Turn on [git] commit in the personal projects and sweep each backlog with tasks commit" --project ops -p 1 --size m --complexity mid --process direct --agent claude-code/claude-opus-5-5 -b "After tasks-6939f6 and the lore provenance piece: add [git] commit = true to tasks/.config.toml in each personal project, run tasks commit once in each, and confirm ops's pre-commit guide regeneration commits cleanly under the record-commit procedure (ops-89d93b): a record commit that changes the goal list must leave git diff --cached empty."
tasks dep <ops-id> --on <lore-id>
```

- [ ] **Step 3:** `tasks note tasks-6939f6 "rollout filed: <lore-id>, <ops-id>"`, then commit the records in each repository by pathspec.
