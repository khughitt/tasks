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
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
    ] {
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
    git(root, &["log", "--format=%s"])
        .lines()
        .map(String::from)
        .collect()
}

fn last_files(root: &Path) -> Vec<String> {
    git(root, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(String::from)
        .collect()
}

fn add(env: &TestEnv, root: &Path, title: &str) -> String {
    env.json(root, &["add", title])["id"]
        .as_str()
        .unwrap()
        .to_string()
}

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
    assert!(
        files.contains(&format!("tasks/{a}.md")) && files.contains(&format!("tasks/{b}.md")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|file| file.ends_with(".tmp")),
        "{files:?}"
    );
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
    let out = env.cmd(&root).arg("commit").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("commit_deferred"),
        "{error}"
    );
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
