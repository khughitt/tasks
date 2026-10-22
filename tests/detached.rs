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
