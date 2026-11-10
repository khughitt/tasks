use super::*;
use crate::retained::{basis, lineage};
use std::{path::Path, process::Command};

fn isolated(name: &str) -> bool {
    if std::env::var_os("TASKS_READ_FIXTURE_CHILD").is_some() {
        return false;
    }
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home.path())
        .env("USER", "fixture")
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_STATE_HOME", home.path().join("state"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("TASKS_READ_FIXTURE_CHILD", "1")
        .args(["--exact", name, "--nocapture"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    true
}
fn git(dir: &Path, args: &[&str]) -> String {
    let out = crate::git::at(dir).args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
fn fixture() -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-qb", "main"]);
    assert_eq!(
        Path::new(&git(dir.path(), &["rev-parse", "--show-toplevel"])),
        dir.path().canonicalize().unwrap()
    );
    assert!(
        Path::new(&git(dir.path(), &["rev-parse", "--absolute-git-dir"])).starts_with(dir.path())
    );
    std::fs::create_dir(dir.path().join("tasks")).unwrap();
    std::fs::write(dir.path().join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "seed"]);
    let project = Project::open(dir.path()).unwrap();
    (dir, project)
}
fn branches(p: &Project, commit: &str) -> String {
    String::from_utf8(
        basis::git(
            p,
            &[
                "for-each-ref",
                "--contains",
                commit,
                "--format=%(refname)",
                "refs/heads/",
            ],
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn validation_read_project_remains_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Project>();
}
#[test]
fn validation_read_snapshot_survives_ref_changes() {
    if isolated("retained::read::tests::validation_read_snapshot_survives_ref_changes") {
        return;
    }
    let (dir, p) = fixture();
    git(dir.path(), &["branch", "old-side"]);
    let p = start(&p).unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    let initial = branches(&p, &head);
    std::fs::write(dir.path().join("unrelated"), "next").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "unrelated"]);
    git(dir.path(), &["branch", "aaa-new"]);
    git(dir.path(), &["branch", "-D", "old-side"]);
    assert_eq!(basis::resolve(&p, "HEAD").unwrap(), head);
    assert_eq!(branches(&p, &head), initial);
    assert!(basis::git(&p, &["show-ref", "--verify", "refs/heads/old-side"]).is_ok());
    assert!(basis::git(&p, &["show-ref", "--verify", "refs/heads/aaa-new"]).is_err());
    let next = start(&Project::open(dir.path()).unwrap()).unwrap();
    assert_ne!(basis::resolve(&next, "HEAD").unwrap(), head);
    assert_eq!(
        branches(&next, &head),
        "refs/heads/aaa-new\nrefs/heads/main\n"
    );
}
#[test]
fn validation_read_snapshot_detached_and_missing() {
    if isolated("retained::read::tests::validation_read_snapshot_detached_and_missing") {
        return;
    }
    let (dir, p) = fixture();
    git(dir.path(), &["checkout", "--detach", "-q"]);
    let p = start(&p).unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    git(dir.path(), &["checkout", "-qb", "other"]);
    assert!(
        !run(&p, &["symbolic-ref", "-q", "HEAD"])
            .unwrap()
            .output
            .status
            .success()
    );
    assert_eq!(basis::resolve(&p, "HEAD").unwrap(), head);
    assert!(basis::git(&p, &["show-ref", "--verify", "refs/heads/missing"]).is_err());
    assert_eq!(
        run(&p, &["show-ref", "--verify", "refs/heads/missing"])
            .unwrap()
            .output
            .status
            .code(),
        Some(128)
    );
    assert_eq!(
        run(
            &p,
            &["show-ref", "--verify", "--quiet", "refs/heads/missing"]
        )
        .unwrap()
        .output
        .status
        .code(),
        Some(1)
    );
}
#[test]
fn validation_read_snapshot_skips_empty_lineage() {
    if isolated("retained::read::tests::validation_read_snapshot_skips_empty_lineage") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("tasks")).unwrap();
    std::fs::write(dir.path().join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();
    let p = Project::open(dir.path()).unwrap();
    lineage::check_live(&p, &[]).unwrap();
}

#[test]
fn validation_read_snapshot_branch_preference_and_config_isolation() {
    if isolated(
        "retained::read::tests::validation_read_snapshot_branch_preference_and_config_isolation",
    ) {
        return;
    }
    let (dir, _) = fixture();
    let original = "---\nid: sci-000001\ntitle: Tiny\nstatus: idea\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n";
    std::fs::write(dir.path().join("tasks/sci-000001.md"), original).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "task"]);
    let source = git(dir.path(), &["rev-parse", "HEAD"]);
    let edge = lineage::RenameEdge {
        version: 1,
        from_prefix: "sci".into(),
        to_prefix: "new".into(),
        layout: "in-tree".into(),
        from_subdir: ".".into(),
        to_subdir: ".".into(),
        commit: source,
        retained_ref: "refs/heads/main".into(),
    };
    let config = serde_json::json!({"prefix":"new", "retention_lineage":[edge]});
    std::fs::write(
        dir.path().join("tasks/.config.toml"),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(dir.path().join("tasks/sci-000001.md")).unwrap();
    std::fs::write(
        dir.path().join("tasks/new-000001.md"),
        original.replace("sci-000001", "new-000001"),
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "rename"]);
    git(dir.path(), &["branch", "aaa"]);
    let p = start(&Project::open(dir.path()).unwrap()).unwrap();
    let id = crate::model::TaskId::parse("new-000001").unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    assert_eq!(
        basis::read(&p, &id, Some("HEAD"))
            .unwrap()
            .basis
            .retained_ref,
        "refs/heads/main"
    );
    assert_eq!(
        lineage::epoch_at(&p, &id, &head).unwrap().retained_ref,
        "refs/heads/aaa"
    );
    let mut different = p.clone();
    different.retention_lineage[0].commit = "0".repeat(40);
    assert!(lineage::validate(&different).is_err());
}

#[test]
fn validation_read_snapshot_checks_unknown_commit() {
    if isolated("retained::read::tests::validation_read_snapshot_checks_unknown_commit") {
        return;
    }
    let (_dir, p) = fixture();
    let p = start(&p).unwrap();
    assert!(basis::resolve(&p, &"0".repeat(40)).is_err());
}
