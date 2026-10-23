mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Git with an environment built key by key: nothing a hook exports reaches the fixture,
/// and `-C` is the only thing that chooses the repository.
fn git(home: &Path, dir: &Path, args: &[&str]) -> String {
    let mut command = std::process::Command::new("git");
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .env("USER", "tester")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t");
    for key in ["GIT_CEILING_DIRECTORIES", "TMPDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let out = command.arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// `git init -b main`, then, before the first write, the assertion that this repository is
/// the fixture: top level == dir and the git directory inside it. One README commit.
fn repo(home: &Path, base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    git(home, &dir, &["init", "-q", "-b", "main"]);
    let top = git(home, &dir, &["rev-parse", "--show-toplevel"]);
    assert_eq!(Path::new(top.trim()), dir);
    let git_dir = git(home, &dir, &["rev-parse", "--absolute-git-dir"]);
    assert!(Path::new(git_dir.trim()).starts_with(&dir));
    std::fs::write(dir.join("README"), "x\n").unwrap();
    git(home, &dir, &["add", "README"]);
    git(home, &dir, &["commit", "-qm", "init"]);
    dir
}

pub struct Layout {
    _base: TempDir,
    pub base: PathBuf,
    /// The test home, as every git call takes it.
    pub home: PathBuf,
    /// The records root, a git repository.
    pub records: PathBuf,
    /// `<records>/<prefix>`, the project's records directory.
    pub project: PathBuf,
    /// The code checkout, a git repository with `.worktrees` linked to `base/storage/code`.
    pub code: PathBuf,
    /// A linked worktree of `code` on branch `wt`.
    pub wt: PathBuf,
}

impl Layout {
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        git(&self.home, dir, args)
    }

    pub fn porcelain(&self, dir: &Path) -> String {
        self.git(dir, &["status", "--porcelain", "--ignored"])
    }
}

fn hook_cmd(env: &TestEnv, dir: &Path, code: &Path) -> assert_cmd::Command {
    let mut cmd = env.cmd(dir);
    cmd.env("GIT_DIR", code.join(".git"))
        .env("GIT_COMMON_DIR", code.join(".git"))
        .env("GIT_WORK_TREE", code)
        .env("GIT_INDEX_FILE", code.join(".git/index"));
    cmd
}

#[test]
fn hook_git_selectors_do_not_redirect_detached_record_commits() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    std::fs::write(l.code.join("README"), "staged code\n").unwrap();
    l.git(&l.code, &["add", "README"]);
    let code_head = l.git(&l.code, &["rev-parse", "HEAD"]);
    let code_index = std::fs::read(l.code.join(".git/index")).unwrap();
    let out = hook_cmd(&env, &l.code, &l.code)
        .args(["add", "from a code hook"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let added: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = added["id"].as_str().unwrap();
    assert_eq!(
        l.git(&l.records, &["show", "--name-only", "--format=", "HEAD"])
            .trim(),
        format!("det/tasks/{id}.md"),
        "the record commits despite staged code: {added}"
    );
    assert_eq!(l.git(&l.records, &["status", "--porcelain"]), "");
    assert_eq!(l.git(&l.code, &["rev-parse", "HEAD"]), code_head);
    assert_eq!(
        std::fs::read(l.code.join(".git/index")).unwrap(),
        code_index
    );
}

#[test]
fn hook_git_selectors_do_not_hide_uncommitted_detached_records() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let config = l.project.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("commit = true", "commit = false")).unwrap();
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "manual record commits"]);
    let added = env.json(&l.code, &["add", "uncommitted"]);
    let id = added["id"].as_str().unwrap();
    let out = hook_cmd(&env, &l.code, &l.code)
        .arg("prime")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let prime: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        prime["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| { w.as_str() == Some(&format!("uncommitted task files: tasks/{id}.md")) }),
        "{prime}"
    );
}

#[test]
fn hook_git_selectors_do_not_redirect_explicit_project_paths() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let foreign = repo(&l.home, &l.base, "foreign");
    for (dir, holds_records) in [(&l.code, false), (&l.project, true)] {
        let out = hook_cmd(&env, dir, &foreign).arg("where").output().unwrap();
        assert!(out.status.success(), "{out:?}");
        let found: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(found["project"]["prefix"], "det", "{found}");
        assert_eq!(found["holds_records"], holds_records, "{found}");
    }
}

#[test]
fn hook_alternate_index_still_guards_the_detached_migration() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    env.json(&l.code, &["init", "--prefix", "det"]);
    l.git(&l.code, &["add", "tasks"]);
    l.git(&l.code, &["commit", "-qm", "in-tree records"]);
    register_by_hand(&env, &l, "det");
    let index = l.base.join("hook-index");
    let index_output = format!("--index-output={}", index.display());
    l.git(&l.code, &["read-tree", &index_output, "HEAD"]);
    l.git(&l.code, &["rm", "-r", "-q", "tasks"]);

    let run = || {
        hook_cmd(&env, &l.code, &l.code)
            .env("GIT_INDEX_FILE", &index)
            .arg("check")
            .output()
            .unwrap()
    };
    let out = run();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "records_moved", "{error}");

    l.git(&l.code, &["read-tree", &index_output, "--empty"]);
    let out = run();
    assert!(out.status.success(), "{out:?}");
}

/// The repositories of a detached project named `prefix`: the records root and the code
/// checkout with a worktree. Nothing is written under `<records>/<prefix>` and nothing is
/// registered: `register` (or `register_by_hand`) does that.
pub fn layout(env: &mut TestEnv, prefix: &str) -> Layout {
    let home = env.home.path().to_path_buf();
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let records = repo(&home, &base, "records");
    let code = repo(&home, &base, "code");
    std::fs::create_dir_all(base.join("storage/code")).unwrap();
    std::os::unix::fs::symlink("../storage/code", code.join(".worktrees")).unwrap();
    let project = records.join(prefix);
    git(
        &home,
        &code,
        &["worktree", "add", "-q", "-b", "wt", ".worktrees/wt"],
    );
    let wt = code.join(".worktrees/wt").canonicalize().unwrap();
    Layout {
        _base: tmp,
        base,
        home,
        records,
        project,
        code,
        wt,
    }
}

#[test]
fn opened_from_its_records_directory_a_detached_project_writes_and_commits_there() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let before = l.porcelain(&l.code);
    let added = env.json(&l.project, &["add", "first"]);
    let id = added["id"].as_str().unwrap().to_string();
    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    assert!(!l.code.join("tasks").exists());
    env.json(&l.project, &["note", &id, "n1"]);
    let doc = l.project.join("docs/specs/x-design.md");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, "# x\n").unwrap();
    // A record commit commits only what the command wrote.
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "spec"]);
    env.json(
        &l.project,
        &["edit", &id, "--spec", "docs/specs/x-design.md"],
    );
    let shown = env.json(&l.project, &["show", &id]);
    assert_eq!(shown["spec_path"], doc.display().to_string());
    assert_eq!(
        l.git(&l.records, &["log", "--format=%s", "-n", "1"]).trim(),
        format!("chore(tasks): edit {id}")
    );
    assert_eq!(
        l.git(&l.records, &["show", "--stat", "--format=", "HEAD"])
            .trim()
            .lines()
            .count(),
        2,
        "the record alone, and the summary line"
    );
    assert_eq!(l.porcelain(&l.records), "");
    assert_eq!(l.porcelain(&l.code), before);
}

#[test]
fn start_and_park_record_the_code_checkout_as_the_resume_root() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let id = env.json(&l.project, &["add", "first"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.project, &["start", &id]);
    env.json(&l.project, &["park", &id, "next", "--waiting-on", "agent"]);
    let store = std::fs::read_to_string(env.claim_store("det")).unwrap();
    assert!(
        store.contains(&l.code.display().to_string()),
        "the claim names the code checkout:\n{store}"
    );
    assert!(!store.contains(&l.project.display().to_string()), "{store}");
}

#[test]
fn commit_from_a_detached_project_commits_in_the_records_repository() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let config = l.project.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("commit = true", "commit = false")).unwrap();
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "no auto commit"]);
    let before = l.porcelain(&l.code);
    let commits = l.git(&l.records, &["rev-list", "--count", "HEAD"]);
    let id = env.json(&l.project, &["add", "first"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        l.git(&l.records, &["rev-list", "--count", "HEAD"]),
        commits,
        "commit = false commits nothing on its own"
    );
    env.json(&l.project, &["commit"]);
    let after: u32 = l
        .git(&l.records, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap();
    assert_eq!(after, commits.trim().parse::<u32>().unwrap() + 1);
    assert!(
        l.git(&l.records, &["show", "--name-only", "--format=", "HEAD"])
            .contains(&format!("tasks/{id}.md"))
    );
    assert_eq!(l.porcelain(&l.code), before);
}

/// Registers `prefix` the way a person does: `init --records`, then the records commit.
pub fn register(env: &TestEnv, l: &Layout, prefix: &str) {
    env.json(
        &l.code,
        &[
            "init",
            "--prefix",
            prefix,
            "--records",
            l.records.to_str().unwrap(),
        ],
    );
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "init"]);
}

/// Registers `prefix` as a detached project without `init --records`, for a state that
/// command refuses: the config written by hand and committed in the records repository,
/// then the registry file written directly: [projects] prefix = code, [records] prefix =
/// project.
pub fn register_by_hand(env: &TestEnv, l: &Layout, prefix: &str) {
    let project = l.records.join(prefix);
    std::fs::create_dir_all(project.join("tasks")).unwrap();
    std::fs::write(
        project.join("tasks/.config.toml"),
        format!(
            "prefix = \"{prefix}\"\n\n[git]\ncommit = true\n\n[checkout]\npath = \"../../code\"\nkind = \"detached\"\n"
        ),
    )
    .unwrap();
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "project"]);
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            "[projects]\n{prefix} = {:?}\n\n[records]\n{prefix} = {:?}\n",
            l.code.display().to_string(),
            project.display().to_string()
        ),
    )
    .unwrap();
}

/// The registry file `register_by_hand` writes, with arbitrary text.
fn write_registry(env: &TestEnv, text: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
}

/// The `error.detail` of a failing command, after asserting its kind.
fn fail_detail(env: &TestEnv, dir: &Path, args: &[&str], kind: &str) -> String {
    assert_eq!(env.fail(dir, args), kind, "{args:?}");
    let out = env.raw(dir).args(args).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    v["error"]["detail"].as_str().unwrap().to_string()
}

#[test]
fn a_detached_project_opens_from_its_checkout_its_worktree_and_its_records_directory() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let id = env.json(&l.code, &["add", "from code"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    assert!(!l.code.join("tasks").exists());
    let from_wt = env.json(&l.wt, &["show", &id]);
    assert_eq!(from_wt["task"]["title"], "from code");
    std::fs::create_dir_all(l.code.join("src")).unwrap();
    let from_deep = env.json(&l.code.join("src"), &["list"]);
    assert_eq!(from_deep["tasks"].as_array().unwrap().len(), 1);
    env.json(&l.project, &["note", &id, "from records"]);
    // start from the worktree records the worktree as the resume root
    env.json(&l.wt, &["start", &id]);
    let store = std::fs::read_to_string(env.claim_store("det")).unwrap();
    assert!(
        store.contains(&format!("worktree = {:?}", l.wt.display().to_string())),
        "{store}"
    );
}

#[test]
fn a_branch_that_still_tracks_the_config_is_records_moved() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    std::fs::create_dir_all(l.wt.join("tasks")).unwrap();
    std::fs::write(l.wt.join("tasks/.config.toml"), "prefix = \"det\"\n").unwrap();
    l.git(&l.wt, &["add", "tasks"]);
    l.git(&l.wt, &["commit", "-qm", "old in-tree records"]);
    let detail = fail_detail(&env, &l.wt, &["list"], "records_moved");
    assert!(
        detail.contains(&l.project.display().to_string()),
        "{detail}"
    );
    assert!(detail.contains("rebase or merge"), "{detail}");
    assert_eq!(
        env.fail(&l.wt, &["note", "det-000000", "x"]),
        "records_moved"
    );
    // the main checkout, which holds no config, still opens the detached project
    env.json(&l.code, &["list"]);
}

#[test]
fn a_registered_root_without_config_or_pairing_is_records_unpaired() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    write_registry(
        &env,
        &format!("[projects]\ndet = {:?}\n", l.code.display().to_string()),
    );
    let detail = fail_detail(&env, &l.code, &["list"], "records_unpaired");
    assert!(
        detail.contains("tasks init --prefix det --records"),
        "{detail}"
    );
}

#[test]
fn an_in_tree_project_reached_from_a_worktree_without_its_tasks_dir_stays_no_project() {
    let env = TestEnv::new();
    let home = env.home.path().to_path_buf();
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let itr = repo(&home, &base, "itr");
    env.json(&itr, &["init", "--prefix", "itr"]);
    git(&home, &itr, &["add", "-A"]);
    git(&home, &itr, &["commit", "-qm", "records"]);
    let orphan = base.join("orphan");
    git(
        &home,
        &itr,
        &[
            "worktree",
            "add",
            "-q",
            "--orphan",
            "-b",
            "orphan",
            orphan.to_str().unwrap(),
        ],
    );
    assert!(!orphan.join("tasks").exists());
    assert_eq!(env.fail(&orphan, &["list"]), "no_project");
}

#[test]
fn a_retirement_declared_in_the_registered_records_binds_an_older_records_worktree() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let old = l.base.join("records-old");
    l.git(
        &l.records,
        &["worktree", "add", "-q", "-b", "old", old.to_str().unwrap()],
    );
    let old_project = old.join("det");
    assert!(old_project.join("tasks/.config.toml").is_file());
    let code = l.code.to_str().unwrap();
    env.json(&l.base, &["-C", code, "retire", "--reason", "over"]);
    assert!(
        std::fs::read_to_string(l.project.join("tasks/.config.toml"))
            .unwrap()
            .contains("[retired]"),
        "the registered records directory holds the retirement"
    );
    assert_eq!(env.fail(&old_project, &["add", "late"]), "retired_project");
    assert_eq!(env.fail(&l.code, &["add", "late"]), "retired_project");
    let detail = fail_detail(
        &env,
        &l.base,
        &[
            "-C",
            old_project.to_str().unwrap(),
            "retire",
            "--reason",
            "again",
        ],
        "not_registered_checkout",
    );
    assert!(
        detail.contains(&l.project.display().to_string()),
        "{detail}"
    );
}

#[test]
fn a_registry_pairing_a_prefix_the_projects_table_lacks_is_a_config_error() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register_by_hand(&env, &l, "det");
    write_registry(
        &env,
        &format!("[records]\ndet = {:?}\n", l.project.display().to_string()),
    );
    assert_eq!(env.fail(&l.project, &["list"]), "config");
}

#[test]
fn a_config_whose_checkout_disagrees_with_the_registry_is_a_config_error() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register_by_hand(&env, &l, "det");
    let id = env.json(&l.project, &["add", "first"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = repo(&l.home, &l.base, "other");
    write_registry(
        &env,
        &format!(
            "[projects]\ndet = {:?}\n\n[records]\ndet = {:?}\n",
            other.display().to_string(),
            l.project.display().to_string()
        ),
    );
    let detail = fail_detail(&env, &l.base, &["show", &id], "config");
    assert!(detail.contains("declares checkout"), "{detail}");
    // The code checkout is no registered root, so the pairing does not reach it; the
    // linked scan then opens every registered project and the mismatch stays an error.
    assert_eq!(env.fail(&l.code, &["list"]), "config");
    // From the records directory the walk opens it.
    env.json(&l.project, &["show", &id]);
}

#[test]
fn a_move_from_an_older_records_worktree_refuses_and_names_the_registered_records() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    env.init("dest");
    let id = env.json(&l.code, &["add", "movable"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let old = l.base.join("records-old");
    l.git(
        &l.records,
        &["worktree", "add", "-q", "-b", "old", old.to_str().unwrap()],
    );
    let old_project = old.join("det");
    assert!(old_project.join(format!("tasks/{id}.md")).is_file());
    let detail = fail_detail(
        &env,
        &l.base,
        &[
            "-C",
            old_project.to_str().unwrap(),
            "move",
            &id,
            "--to",
            "dest",
        ],
        "validation",
    );
    assert!(
        detail.contains(&format!("tasks -C {}", l.project.display())),
        "{detail}"
    );
    assert!(!env.home.path().join(".local/state/tasks/move").exists());
    let moved = env.json(
        &l.base,
        &["-C", l.code.to_str().unwrap(), "move", &id, "--to", "dest"],
    );
    assert_eq!(moved["recovery"], "fresh");
}

#[test]
fn the_lifecycle_from_the_code_checkout_and_its_worktree_writes_only_under_records() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let code_before = l.porcelain(&l.code);
    let wt_before = l.porcelain(&l.wt);
    let spec = l.project.join("docs/specs/topic.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# Topic\n").unwrap();
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "spec"]);
    let shot = l.base.join("shot.png");
    std::fs::write(&shot, b"\x89PNG\r\n\x1a\n").unwrap();
    let elsewhere = l.base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();

    let id = env.json(&l.code, &["add", "Lifecycle"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.code, &["start", &id]);
    env.json(&l.code, &["note", &id, "a note"]);
    env.json(&l.code, &["edit", &id, "--spec", "topic"]);
    env.json(
        &l.code,
        &["attach", &id, shot.to_str().unwrap(), "--caption", "a shot"],
    );
    env.json(&l.code, &["park", &id, "next", "--waiting-on", "agent"]);
    env.json(&l.wt, &["start", &id]);
    env.json(&l.wt, &["note", &id, "from the worktree"]);
    env.json(&l.wt, &["done", &id, "landed"]);

    env.json(&elsewhere, &["quiet", "--project", "det"]);
    env.json(&elsewhere, &["next", "--all-projects"]);
    env.json(&elsewhere, &["show", &id]);
    env.json(&elsewhere, &["prime", "--project", "det"]);

    let second = env.json(&l.code, &["add", "Second"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.code, &["block", &second, "waiting"]);
    env.json(&l.code, &["unblock", &second]);
    env.json(&l.code, &["shelve", &second, "later"]);
    env.json(&l.code, &["unshelve", &second]);
    env.json(&l.code, &["drop", &second, "no longer needed"]);

    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    assert!(
        l.project.join(format!("tasks/files/{id}")).is_dir(),
        "attachments live under the records directory"
    );
    assert_eq!(
        l.porcelain(&l.code),
        code_before,
        "nothing written into the code checkout"
    );
    assert_eq!(
        l.porcelain(&l.wt),
        wt_before,
        "nothing written into the code worktree"
    );
    assert_eq!(
        l.porcelain(&l.records),
        "",
        "every record write was committed in the records repository"
    );
}

#[test]
fn stale_copy_never_fires_across_code_worktrees_and_fires_across_records_worktrees() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    l.git(
        &l.code,
        &["worktree", "add", "-q", "-b", "wt2", ".worktrees/wt2"],
    );
    let wt2 = l.code.join(".worktrees/wt2").canonicalize().unwrap();
    let id = env.json(&l.code, &["add", "Stale"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.wt, &["note", &id, "from wt"]);
    env.json(&wt2, &["note", &id, "from wt2"]);
    env.json(&l.wt, &["note", &id, "from wt again"]);

    let side = l.base.join("records-side");
    l.git(
        &l.records,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    env.json(&side.join("det"), &["note", &id, "from the side copy"]);
    assert_eq!(
        env.fail(&l.code, &["note", &id, "stale write"]),
        "stale_copy"
    );
    let out = hook_cmd(&env, &l.code, &l.code)
        .args(["note", &id, "stale write from hook"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
}

/// The registry file as text.
fn registry_text(env: &TestEnv) -> String {
    std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap()
}

/// `init --prefix <prefix> --records <records>` run in `dir`.
fn init_records<'a>(prefix: &'a str, records: &'a Path) -> [&'a str; 5] {
    [
        "init",
        "--prefix",
        prefix,
        "--records",
        records.to_str().unwrap(),
    ]
}

#[test]
fn init_records_derives_the_directory_writes_the_config_and_pairs_the_registry() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    let before = l.porcelain(&l.code);
    let out = env.json(&l.code, &init_records("det", &l.records));
    assert_eq!(out["records"], l.project.display().to_string());
    assert_eq!(out["root"], l.code.display().to_string());
    let config = std::fs::read_to_string(l.project.join("tasks/.config.toml")).unwrap();
    assert!(config.contains("prefix = \"det\""), "{config}");
    assert!(config.contains("kind = \"detached\""), "{config}");
    assert!(config.contains("path = \"../../code\""), "{config}");
    assert!(config.contains("commit = true"), "{config}");
    assert!(l.project.join("docs/specs").is_dir() && l.project.join("docs/plans").is_dir());
    assert!(!l.code.join("tasks").exists());
    assert_eq!(
        l.porcelain(&l.code),
        before,
        "nothing written in the checkout"
    );
    let registry = registry_text(&env);
    assert!(registry.contains("[records]"), "{registry}");
    assert!(
        registry.contains(&format!("det = {:?}", l.project.display().to_string())),
        "{registry}"
    );
    // the pairing opens it from the code checkout
    assert_eq!(env.json(&l.code, &["where"])["project"]["prefix"], "det");
    // A plain init omits the key.
    let plain = env.init("pln");
    let out = env.json(&plain, &["init", "--prefix", "pln"]);
    assert!(out.get("records").is_none(), "{out}");
}

#[test]
fn init_records_refuses_while_the_config_is_in_the_tree_and_allows_a_staged_deletion() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    env.json(&l.code, &["init", "--prefix", "det"]);
    l.git(&l.code, &["add", "tasks"]);
    l.git(&l.code, &["commit", "-qm", "in-tree records"]);
    let args = init_records("det", &l.records);
    // (a) in the working tree
    let detail = fail_detail(&env, &l.code, &args, "records_in_tree");
    assert!(detail.contains("git rm -r tasks"), "{detail}");
    assert!(!l.project.exists(), "a refused init writes nothing");
    // (b) gone from the working tree, still in the index
    std::fs::remove_file(l.code.join("tasks/.config.toml")).unwrap();
    assert_eq!(env.fail(&l.code, &args), "records_in_tree");
    // (c) the index deletes it while HEAD still tracks it: allowed, and no --force is
    // needed since the prefix is registered to this same root
    l.git(&l.code, &["rm", "-r", "-q", "tasks"]);
    let out = env.json(&l.code, &args);
    let warnings = out["warnings"].to_string();
    assert!(!warnings.contains("was registered to"), "{warnings}");
    // (d) the removal committed: the detached project opens from the checkout
    l.git(&l.code, &["commit", "-qm", "detach"]);
    let id = env.json(&l.code, &["add", "after"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    assert_eq!(
        env.json(&l.code, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn init_records_force_keeps_aliases_groups_and_location_history() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    let old = l.base.join("old");
    std::fs::create_dir_all(&old).unwrap();
    env.json(&old, &["init", "--prefix", "olp"]);
    env.json(&old, &["rename", "olp", "det"]);
    env.json(&old, &["group", "set", "g", "det"]);
    let detail = fail_detail(&env, &l.code, &init_records("det", &l.records), "config");
    assert!(detail.contains("--force"), "{detail}");
    let mut args = init_records("det", &l.records).to_vec();
    args.push("--force");
    let out = env.json(&l.code, &args);
    assert!(
        out["warnings"]
            .to_string()
            .contains(&format!("was registered to {}", old.display())),
        "{out}"
    );
    let registry: toml::Value = toml::from_str(&registry_text(&env)).unwrap();
    assert_eq!(registry["projects"]["det"].as_str(), l.code.to_str());
    assert_eq!(registry["records"]["det"].as_str(), l.project.to_str());
    assert_eq!(registry["aliases"]["olp"].as_str(), Some("det"));
    assert_eq!(
        registry["groups"]["g"].as_array().unwrap(),
        &vec![toml::Value::String("det".into())]
    );
    let former = registry["locations"]["det"]["former"].as_array().unwrap();
    assert_eq!(former[0]["root"].as_str(), old.to_str());
    let resolved = env.json(&l.base, &["resolve", "olp"]);
    assert!(resolved.to_string().contains("\"det\""), "{resolved}");
}

#[test]
fn init_records_on_an_existing_records_config_adds_checkout_and_enables_commits() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    let config = l.project.join("tasks/.config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let args = init_records("det", &l.records);
    // Each case is a host meeting the config for the first time: no pairing yet.
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let fresh_host = || {
        if registry.exists() {
            std::fs::remove_file(&registry).unwrap();
        }
    };
    // no [checkout], no [git]
    std::fs::write(
        &config,
        "# moved here\nprefix = \"det\"\n\n[tags]\nx = \"y\"\n",
    )
    .unwrap();
    let out = env.json(&l.code, &args);
    assert_eq!(out["records"], l.project.display().to_string());
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(
        text.starts_with("# moved here\nprefix = \"det\"\n"),
        "{text}"
    );
    assert!(text.contains("[tags]\nx = \"y\"\n"), "{text}");
    assert!(text.contains("[checkout]"), "{text}");
    assert!(text.contains("path = \"../../code\""), "{text}");
    assert!(text.contains("kind = \"detached\""), "{text}");
    assert!(text.contains("[git]\ncommit = true"), "{text}");
    assert!(l.project.join("docs/specs").is_dir() && l.project.join("docs/plans").is_dir());
    // [git] commit = false becomes true; the comment survives
    fresh_host();
    std::fs::write(
        &config,
        "# moved here\nprefix = \"det\"\n\n[git]\n# keep\ncommit = false\n",
    )
    .unwrap();
    env.json(&l.code, &args);
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.starts_with("# moved here\n"), "{text}");
    assert!(text.contains("# keep\ncommit = true"), "{text}");
    assert!(!text.contains("commit = false"), "{text}");
    // commit = true and the right [checkout] already present: the bytes are unchanged
    fresh_host();
    let settled = "# moved here\nprefix = \"det\"\n\n[git]\ncommit = true # yes\n\n[checkout]\npath = \"../../code\"\nkind = \"detached\"\n";
    std::fs::write(&config, settled).unwrap();
    env.json(&l.code, &args);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), settled);
    // a prefix mismatch is a config error
    fresh_host();
    std::fs::write(&config, "prefix = \"oth\"\n").unwrap();
    let detail = fail_detail(&env, &l.code, &args, "config");
    assert!(detail.contains("\"oth\""), "{detail}");
    // a [checkout] naming another path is a config error, and nothing changes
    fresh_host();
    let other = "prefix = \"det\"\n\n[checkout]\npath = \"../../elsewhere\"\nkind = \"detached\"\n";
    std::fs::write(&config, other).unwrap();
    let detail = fail_detail(&env, &l.code, &args, "config");
    assert!(detail.contains("does not relink"), "{detail}");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), other);
    // so is a linked [checkout]
    fresh_host();
    std::fs::write(
        &config,
        "prefix = \"det\"\n\n[checkout]\npath = \"../../code\"\n",
    )
    .unwrap();
    assert_eq!(env.fail(&l.code, &args), "config");
}

#[test]
fn init_records_refuses_a_checkout_that_overlaps_another_registered_project() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    // A repository nested under an in-tree project.
    let outer = repo(&l.home, &l.base, "outer");
    env.json(&outer, &["init", "--prefix", "out"]);
    let inner = repo(&l.home, &outer, "inner");
    let detail = fail_detail(&env, &inner, &init_records("inn", &l.records), "config");
    assert!(
        detail.contains("overlaps registered project out"),
        "{detail}"
    );
    assert!(detail.contains("outer"), "{detail}");
    // A repository containing a registered root.
    let holder = repo(&l.home, &l.base, "holder");
    let nested = holder.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    env.json(&nested, &["init", "--prefix", "nes"]);
    let detail = fail_detail(&env, &holder, &init_records("hol", &l.records), "config");
    assert!(
        detail.contains("overlaps registered project nes"),
        "{detail}"
    );
    // A main checkout whose worktree, stored outside it, holds a registered project.
    env.json(&l.wt, &["init", "--prefix", "wtp"]);
    let detail = fail_detail(&env, &l.code, &init_records("det", &l.records), "config");
    assert!(
        detail.contains("overlaps registered project wtp"),
        "{detail}"
    );
    assert!(!inner.join("tasks").exists() && !holder.join("tasks").exists());
    assert!(!l.code.join("tasks").exists());
    assert!(!l.records.join("inn").exists() && !l.records.join("hol").exists());
    assert!(!l.project.exists(), "a refused init writes nothing");
    assert!(!registry_text(&env).contains("[records]"));
}

#[test]
fn init_records_is_idempotent_on_the_second_host() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let config = l.project.join("tasks/.config.toml");
    let bytes = std::fs::read(&config).unwrap();
    std::fs::remove_file(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    let mut args = init_records("det", &l.records).to_vec();
    args.push("--force");
    env.json(&l.code, &args);
    assert_eq!(std::fs::read(&config).unwrap(), bytes);
    assert_eq!(l.porcelain(&l.records), "");
    let registry = registry_text(&env);
    assert!(
        registry.contains(&format!("det = {:?}", l.project.display().to_string())),
        "{registry}"
    );
    assert_eq!(env.json(&l.code, &["where"])["project"]["prefix"], "det");
}

#[test]
fn init_records_refusals() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    let args = init_records("det", &l.records);
    // from a linked worktree: names the main checkout
    let detail = fail_detail(&env, &l.wt, &args, "config");
    assert!(detail.contains(&l.code.display().to_string()), "{detail}");
    assert!(detail.contains("linked worktree"), "{detail}");
    // not the top level of its repository
    std::fs::create_dir_all(l.code.join("src")).unwrap();
    assert_eq!(env.fail(&l.code.join("src"), &args), "config");
    // not a git repository
    let plain = l.base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(env.fail(&plain, &init_records("pla", &l.records)), "config");
    // a records root that does not exist
    let missing = l.base.join("missing");
    let detail = fail_detail(&env, &l.code, &init_records("det", &missing), "config");
    assert!(detail.contains("does not exist"), "{detail}");
    assert!(!missing.exists());
    // a records directory inside the checkout
    assert_eq!(env.fail(&l.code, &init_records("det", &l.code)), "config");
    assert!(!l.code.join("det").exists());
    // a bad prefix never becomes a path
    assert_eq!(
        env.fail(&l.code, &init_records("../x", &l.records)),
        "config"
    );
    // with --checkout: a usage error
    let mut both = args.to_vec();
    both.extend(["--checkout", l.code.to_str().unwrap()]);
    env.usage(&l.code, &both);
    // another prefix linked to this checkout
    let overlay = l.base.join("overlay/lnk");
    std::fs::create_dir_all(&overlay).unwrap();
    env.json(&overlay, &["init", "--prefix", "lnk"]);
    let linked_config = overlay.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&linked_config).unwrap();
    text.push_str(&format!(
        "\n[checkout]\npath = {:?}\n",
        l.code.display().to_string()
    ));
    std::fs::write(&linked_config, text).unwrap();
    let detail = fail_detail(&env, &l.code, &args, "checkout_conflict");
    assert!(detail.contains("lnk"), "{detail}");
    assert!(!l.project.exists(), "a refused init writes nothing");
    env.json(&l.base, &["unregister", "lnk"]);
    // another prefix already paired with this checkout
    register(&env, &l, "det");
    let detail = fail_detail(
        &env,
        &l.code,
        &init_records("oth", &l.records),
        "checkout_conflict",
    );
    assert!(detail.contains("det"), "{detail}");
    assert!(!l.records.join("oth").exists());
    // plain init in a paired checkout: the records moved
    let detail = fail_detail(&env, &l.code, &["init", "--prefix", "det"], "records_moved");
    assert!(
        detail.contains(&format!(
            "tasks init --prefix det --records {} --force",
            l.records.display()
        )),
        "{detail}"
    );
    assert!(
        detail.contains(&l.project.display().to_string()),
        "{detail}"
    );
    assert_eq!(
        env.fail(&l.wt, &["init", "--prefix", "det", "--force"]),
        "records_moved"
    );
    assert!(!l.code.join("tasks").exists() && !l.wt.join("tasks").exists());
}

#[test]
fn a_plain_init_repoint_drops_a_stale_records_pairing() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let elsewhere = repo(&l.home, &l.base, "elsewhere");
    let out = env.json(&elsewhere, &["init", "--prefix", "det", "--force"]);
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains(&format!(
            "prefix \\\"det\\\" was paired with {}; the pairing is removed",
            l.project.display()
        )),
        "{warnings}"
    );
    let registry: toml::Value = toml::from_str(&registry_text(&env)).unwrap();
    assert!(registry.get("records").is_none(), "{registry}");
    assert_eq!(registry["projects"]["det"].as_str(), elsewhere.to_str());
    env.json(&elsewhere, &["list"]);
    // A plain init that pairs nothing warns about nothing.
    let out = env.json(&elsewhere, &["init", "--prefix", "det"]);
    assert!(!out["warnings"].to_string().contains("paired"), "{out}");
}

#[test]
fn a_plain_init_in_a_paired_checkout_refuses_while_its_records_are_off_this_host() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    std::fs::rename(&l.records, l.base.join("records-away")).unwrap();
    assert_eq!(
        env.fail(&l.code, &["init", "--prefix", "det", "--force"]),
        "records_moved"
    );
    assert!(!l.code.join("tasks").exists());
    assert!(registry_text(&env).contains("[records]"));
}

#[test]
fn init_records_force_refuses_a_retired_records_config() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    env.json(&l.code, &["retire", "--reason", "over"]);
    let config = l.project.join("tasks/.config.toml");
    let bytes = std::fs::read(&config).unwrap();
    let mut args = init_records("det", &l.records).to_vec();
    args.push("--force");
    // re-pairing on this host
    let detail = fail_detail(&env, &l.code, &args, "retired_project");
    assert!(detail.contains("tasks unretire"), "{detail}");
    // a second host meeting the retired config for the first time
    std::fs::remove_file(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert_eq!(env.fail(&l.code, &args), "retired_project");
    assert_eq!(std::fs::read(&config).unwrap(), bytes);
    assert!(!env.home.path().join(".config/tasks/projects.toml").exists());
}

#[test]
fn where_reports_kind_records_and_checkout_for_every_shape() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    std::fs::create_dir_all(l.code.join("src")).unwrap();
    for dir in [
        l.code.clone(),
        l.wt.clone(),
        l.code.join("src"),
        l.project.clone(),
    ] {
        let out = env.json(&dir, &["where"]);
        let p = &out["project"];
        assert_eq!(p["prefix"], "det", "{dir:?}: {out}");
        assert_eq!(p["root"], l.code.to_str().unwrap(), "{dir:?}");
        assert_eq!(p["checkout"], l.code.to_str().unwrap(), "{dir:?}");
        assert_eq!(p["records"], l.project.to_str().unwrap(), "{dir:?}");
        assert_eq!(p["kind"], "detached", "{dir:?}");
        assert_eq!(p["linked"], false, "{dir:?}");
        assert_eq!(out["holds_records"], dir == l.project, "{dir:?}");
    }
    let above = env.json(&l.records, &["where"]);
    assert!(above["project"].is_null(), "{above}");
    let pretty = env.pretty(&l.code, &["where"]);
    assert_eq!(
        pretty.trim(),
        format!(
            "det {} (records at {})",
            l.code.display(),
            l.project.display()
        )
    );
}

#[test]
fn projects_rows_carry_the_three_locations_and_the_kind() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let row = |env: &TestEnv| {
        env.json(&l.code, &["projects"])["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["prefix"] == "det")
            .unwrap()
            .clone()
    };
    let r = row(&env);
    assert_eq!(r["root"], l.code.to_str().unwrap());
    assert_eq!(r["checkout"], l.code.to_str().unwrap());
    assert_eq!(r["records"], l.project.to_str().unwrap());
    assert_eq!(r["kind"], "detached");
    let paths = env.pretty(&l.code, &["projects", "--paths"]);
    assert!(
        paths.contains(&format!("{} → {}", l.code.display(), l.project.display())),
        "{paths}"
    );
    std::fs::remove_dir_all(&l.project).unwrap();
    let gone = env.json(&l.base, &["projects"])["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["prefix"] == "det")
        .unwrap()
        .clone();
    assert_eq!(gone["reachable"], false);
    assert_eq!(gone["records"], l.project.to_str().unwrap());
    let object = gone.as_object().unwrap();
    assert!(
        !object.contains_key("checkout") && !object.contains_key("kind"),
        "{gone}"
    );
}

#[test]
fn resolve_matches_a_path_under_records_via_records_and_under_the_checkout_via_root() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let resolve = |input: &Path| {
        env.json(&l.base, &["resolve", input.to_str().unwrap()])["results"][0].clone()
    };
    let r = resolve(&l.project.join("tasks/det-000000.md"));
    assert_eq!(r["prefix"], "det", "{r}");
    assert_eq!(r["via"], "records", "{r}");
    assert_eq!(r["records"], l.project.to_str().unwrap());
    assert_eq!(r["relpath"], "tasks/det-000000.md");
    let r = resolve(&l.code.join("src/main.rs"));
    assert_eq!(r["via"], "root", "{r}");
    assert_eq!(r["records"], l.project.to_str().unwrap());
    let r = resolve(&l.wt.join("x"));
    assert_eq!(r["via"], "storage", "{r}");
    assert_eq!(
        r["worktree"],
        l.base.join("storage/code/wt").to_str().unwrap()
    );
    let r = env.json(&l.base, &["resolve", "det"])["results"][0].clone();
    assert_eq!(r["root"], l.code.to_str().unwrap());
    assert_eq!(r["records"], l.project.to_str().unwrap());
}

#[test]
fn check_in_the_code_checkout_refuses_an_index_held_config_and_passes_a_staged_deletion() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    env.json(&l.code, &["init", "--prefix", "det"]);
    l.git(&l.code, &["add", "tasks"]);
    l.git(&l.code, &["commit", "-qm", "in-tree records"]);
    register_by_hand(&env, &l, "det");
    // (a) gone from the working tree, still in the index: the pairing opens the detached
    // project, and check refuses like the walk does for a working-tree config
    std::fs::remove_file(l.code.join("tasks/.config.toml")).unwrap();
    let detail = fail_detail(&env, &l.code, &["check"], "records_moved");
    assert!(detail.contains(l.project.to_str().unwrap()), "{detail}");
    // opened by its records directory, the index check never runs
    assert_eq!(env.check(&l.project)["errors"].as_array().unwrap().len(), 0);
    // (b) the index deletes it while HEAD still tracks it: the removal commit's window
    l.git(&l.code, &["rm", "-r", "-q", "tasks"]);
    assert_eq!(env.check(&l.code)["errors"].as_array().unwrap().len(), 0);
    // (c) committed
    l.git(&l.code, &["commit", "-qm", "detach"]);
    assert_eq!(env.check(&l.code)["errors"].as_array().unwrap().len(), 0);
    assert_eq!(env.check(&l.project)["errors"].as_array().unwrap().len(), 0);
}

#[test]
fn check_warns_when_the_code_checkout_is_missing() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    l.git(
        &l.code,
        &["worktree", "remove", "--force", l.wt.to_str().unwrap()],
    );
    std::fs::remove_dir_all(&l.code).unwrap();
    std::fs::remove_dir_all(l.base.join("storage/code")).unwrap();
    env.json(&l.project, &["list"]);
    let out = env.check(&l.project);
    assert!(
        out["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "checkout_missing"),
        "{out}"
    );
}

fn write_file(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn a_spec_under_records_and_a_retained_spec_in_the_code_tree_both_resolve() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    write_file(&l.project.join("docs/specs/a-design.md"), "# A\n");
    write_file(&l.project.join("docs/plans/p.md"), "# P\n\n## Task 1: x\n");
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "docs"]);
    write_file(&l.code.join("docs/specs/kept-design.md"), "# Kept\n");
    l.git(&l.code, &["add", "-A"]);
    l.git(&l.code, &["commit", "-qm", "kept"]);

    let t1 = env.json(&l.code, &["add", "T1", "--spec", "docs/specs/a-design.md"]);
    let t2 = env.json(
        &l.code,
        &["add", "T2", "--spec", "docs/specs/kept-design.md"],
    );
    let t3 = env.json(
        &l.code,
        &[
            "add",
            "T3",
            "--plan",
            "docs/plans/p.md",
            "--step",
            "Task 1: x",
        ],
    );
    let (t1, t2, t3) = (common::id_of(t1), common::id_of(t2), common::id_of(t3));

    let shown = env.json(&l.code, &["show", &t1]);
    assert_eq!(
        shown["spec_path"],
        l.project.join("docs/specs/a-design.md").to_str().unwrap(),
        "{shown}"
    );
    let shown = env.json(&l.code, &["show", &t2]);
    assert_eq!(
        shown["spec_path"],
        l.code.join("docs/specs/kept-design.md").to_str().unwrap(),
        "{shown}"
    );
    let shown = env.json(&l.code, &["show", &t3]);
    assert_eq!(shown["step_found"], true, "{shown}");

    let check = env.check(&l.code);
    let text = check.to_string();
    assert!(!text.contains("in_main_checkout"), "{text}");
    assert_eq!(check["errors"].as_array().map_or(0, Vec::len), 0, "{text}");

    // The worktree is searched before the main checkout.
    write_file(&l.wt.join("docs/specs/wt-only.md"), "# Wt\n");
    let t4 = common::id_of(env.json(&l.wt, &["add", "T4", "--spec", "docs/specs/wt-only.md"]));
    let shown = env.json(&l.wt, &["show", &t4]);
    assert_eq!(
        shown["spec_path"],
        l.wt.join("docs/specs/wt-only.md").to_str().unwrap(),
        "{shown}"
    );
}

#[test]
fn an_output_ref_under_records_satisfies_the_completion_gate() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let a = common::id_of(env.json(
        &l.code,
        &["add", "A", "--ref", "rep (output): docs/notes/r.md"],
    ));
    env.json(&l.code, &["start", &a]);
    assert_eq!(env.fail(&l.code, &["done", &a, "x"]), "ref_output_missing");
    write_file(&l.project.join("docs/notes/r.md"), "r\n");
    env.json(&l.code, &["done", &a, "x"]);

    let b = common::id_of(env.json(
        &l.code,
        &["add", "B", "--ref", "rep (output): docs/notes/c.md"],
    ));
    env.json(&l.code, &["start", &b]);
    assert_eq!(env.fail(&l.code, &["done", &b, "x"]), "ref_output_missing");
    write_file(&l.code.join("docs/notes/c.md"), "c\n");
    env.json(&l.code, &["done", &b, "x"]);
}

#[test]
fn a_document_under_records_resolves_without_asking_git_about_the_main_checkout() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    write_file(&l.project.join("docs/specs/here.md"), "# Here\n");
    write_file(&l.project.join("docs/plans/p.md"), "# P\n\n## Task 1: x\n");
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "docs"]);
    let id = common::id_of(env.json(
        &l.project,
        &[
            "add",
            "T",
            "--spec",
            "docs/specs/here.md",
            "--plan",
            "docs/plans/p.md",
            "--step",
            "Task 1: x",
        ],
    ));
    // A `git` that fails every call: any lookup that consults git before records misses
    // surfaces as an error.
    let bin = l.base.join("nogit");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("git"), "#!/bin/sh\nexit 1\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = env.raw(&l.project);
    command.env("PATH", &bin).args(["show", &id]);
    let out = command.output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let shown: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(shown["step_found"], true, "{shown}");
    assert_eq!(
        shown["spec_path"],
        l.project.join("docs/specs/here.md").to_str().unwrap()
    );
}

/// A registered detached `det` with two committed tasks, one of them parked so the rename
/// carries a claim store: the ids.
fn rename_fixture(env: &mut TestEnv) -> (Layout, Vec<String>) {
    let l = layout(env, "det");
    register(env, &l, "det");
    let ids: Vec<String> = ["first", "second"]
        .iter()
        .map(|title| common::id_of(env.json(&l.project, &["add", title])))
        .collect();
    env.json(&l.project, &["start", &ids[0]]);
    env.json(
        &l.project,
        &["park", &ids[0], "next", "--waiting-on", "agent"],
    );
    assert_eq!(l.git(&l.records, &["status", "--porcelain"]), "");
    (l, ids)
}

/// `rename det dtx` with `TASKS_RENAME_STOP_AFTER=stop`, run in the code checkout.
fn rename_stop(env: &TestEnv, l: &Layout, stop: &str) {
    let out = env
        .raw(&l.code)
        .env("TASKS_RENAME_STOP_AFTER", stop)
        .args(["rename", "det", "dtx"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{stop}: {out:?}");
}

/// Every path and its bytes under the records root (its `.git` aside) and the test home:
/// what `--explain` and a replay must leave alone.
fn disk_state(env: &TestEnv, l: &Layout) -> std::collections::BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn collect(path: &Path, state: &mut std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>) {
        if path.file_name().is_some_and(|name| name == ".git") {
            return;
        }
        if path.is_dir() {
            state.insert(path.to_path_buf(), None);
            for entry in std::fs::read_dir(path).unwrap() {
                collect(&entry.unwrap().path(), state);
            }
        } else if path.exists() {
            state.insert(path.to_path_buf(), Some(std::fs::read(path).unwrap()));
        }
    }
    let mut state = std::collections::BTreeMap::new();
    collect(&l.records, &mut state);
    collect(env.home.path(), &mut state);
    state
}

/// The end state of `det -> dtx`, however it was reached.
fn assert_renamed(env: &TestEnv, l: &Layout, ids: &[String], label: &str) {
    let moved = l.records.join("dtx");
    assert!(!l.project.exists(), "{label}: det is gone");
    for id in ids {
        let renamed = id.replacen("det-", "dtx-", 1);
        assert!(
            moved.join(format!("tasks/{renamed}.md")).is_file(),
            "{label}: {renamed}"
        );
        assert_eq!(
            env.json(&l.code, &["show", id])["task"]["id"],
            renamed,
            "{label}: the old id resolves through the alias"
        );
    }
    let registry: toml::Value = toml::from_str(&registry_text(env)).unwrap();
    assert_eq!(
        registry["projects"]["dtx"].as_str(),
        l.code.to_str(),
        "{label}"
    );
    assert!(registry["projects"].get("det").is_none(), "{label}");
    assert_eq!(
        registry["records"]["dtx"].as_str(),
        moved.to_str(),
        "{label}"
    );
    assert!(registry["records"].get("det").is_none(), "{label}");
    assert_eq!(registry["aliases"]["det"].as_str(), Some("dtx"), "{label}");
    let config: toml::Value =
        toml::from_str(&std::fs::read_to_string(moved.join("tasks/.config.toml")).unwrap())
            .unwrap();
    assert_eq!(config["prefix"].as_str(), Some("dtx"), "{label}");
    assert_eq!(
        config["checkout"]["path"].as_str(),
        Some("../../code"),
        "{label}"
    );
    let listed = env.json(&l.code, &["list"]);
    let listed: Vec<&str> = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed.len(), ids.len(), "{label}: {listed:?}");
    assert!(
        listed.iter().all(|id| id.starts_with("dtx-")),
        "{label}: {listed:?}"
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/det.toml")
            .exists(),
        "{label}: the inventory is removed"
    );
    assert!(!env.claim_store("det").exists(), "{label}");
    let store = std::fs::read_to_string(env.claim_store("dtx")).unwrap();
    assert!(
        store.contains(&ids[0].replacen("det-", "dtx-", 1)),
        "{label}: the park moved\n{store}"
    );
    let status = l.git(&l.records, &["status", "--porcelain"]);
    assert!(status.contains("?? dtx/"), "{label}: {status}");
    assert!(
        status.contains(" D det/tasks/.config.toml"),
        "{label}: the rename is the user's to commit\n{status}"
    );
    assert_eq!(
        env.json(&l.code, &["rename", "det", "dtx", "--explain"])["recovery"],
        "complete",
        "{label}"
    );
}

#[test]
fn rename_moves_the_records_directory_and_the_registry_pairing() {
    let mut env = TestEnv::new();
    let (l, ids) = rename_fixture(&mut env);
    // A directory already at the destination refuses, as a destination task file does.
    let taken = l.records.join("dtx");
    std::fs::create_dir(&taken).unwrap();
    let explained = env.json(&l.code, &["rename", "det", "dtx", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"][0]
            .as_str()
            .unwrap()
            .starts_with("both "),
        "{explained}"
    );
    assert!(
        fail_detail(&env, &l.code, &["rename", "det", "dtx"], "validation").starts_with("both ")
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/det.toml")
            .exists()
    );
    std::fs::remove_dir(&taken).unwrap();
    // The registry's [projects] names the code checkout, which is the invocation's identity.
    assert_eq!(
        env.json(&l.code, &["rename", "det", "dtx", "--explain"])["recovery"],
        "fresh"
    );
    let out = env.json(&l.code, &["rename", "det", "dtx"]);
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(out["tasks"], 2);
    assert_eq!(out["root"], l.code.display().to_string());
    assert_renamed(&env, &l, &ids, "uninterrupted");
}

#[test]
fn rename_resumes_across_the_records_boundary() {
    for (stop, want) in [
        ("inventory", "resume_files"),
        ("file:0", "resume_files"),
        ("files", "resume_files"),
        ("config", "resume_registry"),
        ("records", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("store", "resume_cleanup"),
        ("claims", "resume_cleanup"),
    ] {
        let mut env = TestEnv::new();
        let (l, ids) = rename_fixture(&mut env);
        rename_stop(&env, &l, stop);
        let moved = matches!(stop, "records" | "registry" | "store" | "claims");
        assert_eq!(l.project.exists(), !moved, "{stop}");
        assert_eq!(l.records.join("dtx").exists(), moved, "{stop}");
        let before = disk_state(&env, &l);
        let explained = env.json(&l.code, &["rename", "det", "dtx", "--explain"]);
        assert_eq!(explained["recovery"], want, "{stop}: {explained}");
        assert_eq!(disk_state(&env, &l), before, "{stop}: explain wrote");
        let resumed = env.json(&l.code, &["rename", "det", "dtx"]);
        assert_eq!(resumed["recovery"], want, "{stop}: {resumed}");
        assert_renamed(&env, &l, &ids, stop);
    }
}

#[test]
fn rename_replay_and_explain_after_the_inventory_is_gone_report_complete_without_writes() {
    let mut env = TestEnv::new();
    let (l, ids) = rename_fixture(&mut env);
    env.json(&l.code, &["rename", "det", "dtx"]);
    let registry = std::fs::read(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    let before = disk_state(&env, &l);
    let status = l.porcelain(&l.records);
    for args in [
        &["rename", "det", "dtx", "--explain"][..],
        &["rename", "det", "dtx"][..],
    ] {
        let out = env.json(&l.code, args);
        assert_eq!(out["recovery"], "complete", "{args:?}: {out}");
        assert_eq!(
            std::fs::read(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
            registry,
            "{args:?}"
        );
        let after = disk_state(&env, &l);
        let records: Vec<_> = before
            .keys()
            .filter(|path| path.starts_with(&l.records))
            .collect();
        assert_eq!(
            records,
            after
                .keys()
                .filter(|path| path.starts_with(&l.records))
                .collect::<Vec<_>>(),
            "{args:?}"
        );
        for path in records {
            assert_eq!(before[path], after[path], "{args:?}: {}", path.display());
        }
        assert_eq!(l.porcelain(&l.records), status, "{args:?}");
    }
    assert_renamed(&env, &l, &ids, "replayed");
}

#[test]
fn rename_refuses_a_second_worktree_of_the_records_repository_not_of_the_code_repository() {
    let mut env = TestEnv::new();
    let (l, _) = rename_fixture(&mut env);
    assert!(l.wt.is_dir(), "the code repository has a second worktree");
    env.json(&l.code, &["rename", "det", "dtx"]);
    l.git(&l.records, &["add", "-A"]);
    l.git(&l.records, &["commit", "-qm", "rename"]);
    let second = l.base.join("records-wt");
    l.git(
        &l.records,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "rw",
            second.to_str().unwrap(),
        ],
    );
    let detail = fail_detail(&env, &l.code, &["rename", "dtx", "det"], "validation");
    assert!(detail.contains("2 git worktrees"), "{detail}");
    assert!(l.records.join("dtx").is_dir());
}

#[test]
fn adopt_pairs_the_renamed_records_directory() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let id = common::id_of(env.json(&l.project, &["add", "first"]));
    let renamed = id.replacen("det-", "dtx-", 1);
    // What the sync delivers from the host that ran the rename.
    let moved = l.records.join("dtx");
    std::fs::rename(&l.project, &moved).unwrap();
    let old = moved.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&old).unwrap();
    std::fs::write(
        moved.join(format!("tasks/{renamed}.md")),
        text.replacen(&id, &renamed, 1),
    )
    .unwrap();
    std::fs::remove_file(old).unwrap();
    let config = moved.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("prefix = \"det\"", "prefix = \"dtx\""),
    )
    .unwrap();

    let registry = registry_text(&env);
    let explained = env.json(&l.code, &["rename", "det", "dtx", "--adopt", "--explain"]);
    assert_eq!(explained["recovery"], "fresh", "{explained}");
    assert_eq!(registry_text(&env), registry, "explain wrote the registry");
    let out = env.json(&l.code, &["rename", "det", "dtx", "--adopt"]);
    assert_eq!(out["mode"], "adopt");
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(out["root"], l.code.display().to_string());
    let registry: toml::Value = toml::from_str(&registry_text(&env)).unwrap();
    assert_eq!(registry["projects"]["dtx"].as_str(), l.code.to_str());
    assert_eq!(registry["records"]["dtx"].as_str(), moved.to_str());
    assert!(registry["records"].get("det").is_none(), "{registry}");
    assert_eq!(registry["aliases"]["det"].as_str(), Some("dtx"));
    let listed = env.json(&l.code, &["list"]);
    assert_eq!(listed["tasks"][0]["id"], renamed, "{listed}");
    assert_eq!(env.json(&l.code, &["show", &id])["task"]["id"], renamed);
    assert_eq!(
        env.json(&l.code, &["rename", "det", "dtx", "--adopt", "--explain"])["recovery"],
        "complete"
    );
}

#[test]
fn init_records_refuses_to_re_pair_with_another_records_root_without_force() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let other = repo(&l.home, &l.base, "other");
    let before = registry_text(&env);
    let detail = fail_detail(&env, &l.code, &init_records("det", &other), "config");
    assert!(
        detail.contains(&l.project.display().to_string()) && detail.contains("--force"),
        "{detail}"
    );
    assert!(!other.join("det").exists(), "the refusal created nothing");
    assert_eq!(registry_text(&env), before);
    // The same records root again is still idempotent without --force.
    env.json(&l.code, &init_records("det", &l.records));
    assert_eq!(registry_text(&env), before);
    let mut args = init_records("det", &other).to_vec();
    args.push("--force");
    let out = env.json(&l.code, &args);
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains(&format!(
            "prefix \\\"det\\\" was paired with {}",
            l.project.display()
        )),
        "{warnings}"
    );
    let registry: toml::Value = toml::from_str(&registry_text(&env)).unwrap();
    assert_eq!(
        registry["records"]["det"].as_str(),
        other.join("det").to_str()
    );
}

#[test]
fn a_plain_init_in_a_records_directory_refuses_and_names_init_records() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    let before = registry_text(&env);
    for args in [
        &["init", "--prefix", "det", "--force"][..],
        &["init", "--prefix", "det"][..],
    ] {
        let detail = fail_detail(&env, &l.project, args, "config");
        assert!(
            detail.contains(&format!(
                "tasks init --prefix det --records {}",
                l.records.display()
            )),
            "{args:?}: {detail}"
        );
        assert!(
            detail.contains(&l.code.display().to_string()),
            "{args:?}: {detail}"
        );
        assert_eq!(registry_text(&env), before, "{args:?}");
    }
    env.json(&l.code, &["list"]);
}

#[test]
fn a_registry_mapping_a_prefix_to_its_detached_records_without_a_pairing_is_a_config_error() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    write_registry(
        &env,
        &format!("[projects]\ndet = {:?}\n", l.project.display().to_string()),
    );
    for args in [&["show", "det-000000"][..], &["projects"][..]] {
        let detail = fail_detail(&env, &l.base, args, "config");
        assert!(detail.contains("detached"), "{args:?}: {detail}");
        assert!(
            detail.contains("tasks init --prefix det --records"),
            "{args:?}: {detail}"
        );
    }
}

#[test]
fn rename_without_adopt_after_another_host_renamed_names_adopt() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register(&env, &l, "det");
    // What the sync delivers from the host that ran the rename.
    let moved = l.records.join("dtx");
    std::fs::rename(&l.project, &moved).unwrap();
    let config = moved.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("prefix = \"det\"", "prefix = \"dtx\""),
    )
    .unwrap();
    let registry = registry_text(&env);
    let detail = fail_detail(&env, &l.code, &["rename", "det", "dtx"], "validation");
    assert!(detail.contains("tasks rename det dtx --adopt"), "{detail}");
    assert!(
        detail.contains(&l.project.display().to_string())
            && detail.contains(&moved.display().to_string()),
        "{detail}"
    );
    let explained = env.json(&l.code, &["rename", "det", "dtx", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"].to_string().contains("--adopt"),
        "{explained}"
    );
    assert_eq!(
        registry_text(&env),
        registry,
        "a refusal wrote the registry"
    );
    let out = env.json(&l.code, &["rename", "det", "dtx", "--adopt"]);
    assert_eq!(out["mode"], "adopt");
    let registry: toml::Value = toml::from_str(&registry_text(&env)).unwrap();
    assert_eq!(registry["records"]["dtx"].as_str(), moved.to_str());
}

#[test]
fn a_registered_root_without_config_or_pairing_is_records_unpaired_from_every_entry_point() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    write_registry(
        &env,
        &format!("[projects]\ndet = {:?}\n", l.code.display().to_string()),
    );
    for args in [
        &["show", "det-000000"][..],
        &["list", "--project", "det"][..],
    ] {
        let detail = fail_detail(&env, &l.base, args, "records_unpaired");
        assert!(
            detail.contains("tasks init --prefix det --records"),
            "{args:?}: {detail}"
        );
    }
    let other = env.init("oth");
    let id = common::id_of(env.json(&other, &["add", "first"]));
    let detail = fail_detail(
        &env,
        &other,
        &["move", &id, "--to", "det"],
        "records_unpaired",
    );
    assert!(
        detail.contains("tasks init --prefix det --records"),
        "{detail}"
    );
}
