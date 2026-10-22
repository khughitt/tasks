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

/// A detached project named `prefix`: config written by hand and committed in the records
/// repository. Nothing is registered.
pub fn layout(env: &mut TestEnv, prefix: &str) -> Layout {
    let home = env.home.path().to_path_buf();
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let records = repo(&home, &base, "records");
    let code = repo(&home, &base, "code");
    std::fs::create_dir_all(base.join("storage/code")).unwrap();
    std::os::unix::fs::symlink("../storage/code", code.join(".worktrees")).unwrap();
    let project = records.join(prefix);
    std::fs::create_dir_all(project.join("tasks")).unwrap();
    std::fs::write(
        project.join("tasks/.config.toml"),
        format!(
            "prefix = \"{prefix}\"\n\n[git]\ncommit = true\n\n[checkout]\npath = \"../../code\"\nkind = \"detached\"\n"
        ),
    )
    .unwrap();
    git(&home, &records, &["add", "-A"]);
    git(&home, &records, &["commit", "-qm", "project"]);
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

/// Registers `prefix` as a detached project by writing the registry file directly:
/// [projects] prefix = code, [records] prefix = project.
pub fn register_by_hand(env: &TestEnv, l: &Layout, prefix: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            "[projects]\n{prefix} = {:?}\n\n[records]\n{prefix} = {:?}\n",
            l.code.display().to_string(),
            l.project.display().to_string()
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
    register_by_hand(&env, &l, "det");
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
    register_by_hand(&env, &l, "det");
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
    register_by_hand(&env, &l, "det");
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
    register_by_hand(&env, &l, "det");
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
    register_by_hand(&env, &l, "det");
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
    register_by_hand(&env, &l, "det");
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
}
