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
        .env("GIT_TRACE2_EVENT", home.path().join("git-trace.jsonl"))
        .env("GIT_AUTHOR_DATE", "2026-11-10T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-11-10T00:00:00Z")
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

fn starts(verb: &str) -> usize {
    let path = std::env::var_os("GIT_TRACE2_EVENT").unwrap();
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| {
            let r: serde_json::Value = serde_json::from_str(line).unwrap();
            r["event"] == "start" && r["argv"].as_array().unwrap().iter().any(|a| a == verb)
        })
        .count()
}
#[test]
fn validation_read_reuses_positive_and_negative_ancestry() {
    if isolated("retained::read::tests::validation_read_reuses_positive_and_negative_ancestry") {
        return;
    }
    let (dir, p) = fixture();
    let main = git(dir.path(), &["rev-parse", "HEAD"]);
    git(dir.path(), &["checkout", "--orphan", "other"]);
    git(dir.path(), &["commit", "-qm", "unrelated"]);
    let other = git(dir.path(), &["rev-parse", "HEAD"]);
    let p = start(&p).unwrap();
    let before = starts("merge-base");
    for _ in 0..3 {
        assert!(ancestor(&p, &main, &main).unwrap());
        assert!(!ancestor(&p, &other, &main).unwrap());
    }
    assert!(starts("merge-base") - before <= 2);
}
#[test]
fn validation_read_errors_are_not_cached() {
    if isolated("retained::read::tests::validation_read_errors_are_not_cached") {
        return;
    }
    let (_dir, p) = fixture();
    let p = start(&p).unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    let before = starts("merge-base");
    for _ in 0..2 {
        assert!(ancestor(&p, &"0".repeat(40), &head).is_err());
    }
    assert_eq!(starts("merge-base") - before, 2);
}
#[test]
fn validation_read_session_ends_after_failure() {
    if isolated("retained::read::tests::validation_read_session_ends_after_failure") {
        return;
    }
    let (_dir, original) = fixture();
    let p = start(&original).unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    let before = starts("cat-file");
    for _ in 0..2 {
        basis::blob(&p, &head, "tasks/.config.toml", None, &mut 0).unwrap();
    }
    assert!(ancestor(&p, &"0".repeat(40), &head).is_err());
    let weak = Arc::downgrade(&p.retention_reads.as_ref().unwrap().0);
    drop(p);
    assert!(weak.upgrade().is_none());
    let next = start(&original).unwrap();
    basis::blob(&next, &head, "tasks/.config.toml", None, &mut 0).unwrap();
    assert!(starts("cat-file") - before <= 2);
}
#[test]
fn validation_read_budget_falls_back_to_reads() {
    if isolated("retained::read::tests::validation_read_budget_falls_back_to_reads") {
        return;
    }
    for (entries, bytes) in [(0, usize::MAX), (usize::MAX, 0)] {
        let (_dir, p) = fixture();
        let p = start(&p).unwrap();
        {
            let mut s = p.retention_reads.as_ref().unwrap().lock().unwrap();
            s.max_entries = entries;
            s.max_bytes = bytes;
        }
        let head = basis::resolve(&p, "HEAD").unwrap();
        let before = starts("cat-file");
        for _ in 0..2 {
            assert_eq!(
                basis::blob(&p, &head, "tasks/.config.toml", None, &mut 0).unwrap(),
                b"prefix = \"sci\"\n"
            );
        }
        assert_eq!(starts("cat-file") - before, 2);
    }
}
#[test]
fn validation_read_separates_repositories() {
    if isolated("retained::read::tests::validation_read_separates_repositories") {
        return;
    }
    let (a, p) = fixture();
    let (b, q) = fixture();
    let head = git(a.path(), &["rev-parse", "HEAD"]);
    assert_eq!(git(b.path(), &["rev-parse", "HEAD"]), head);
    let p = start(&p).unwrap();
    let q = inherit(&p, q).unwrap();
    basis::blob(&p, &head, "tasks/.config.toml", None, &mut 0).unwrap();
    let oid = git(b.path(), &["rev-parse", "HEAD:tasks/.config.toml"]);
    std::fs::remove_file(
        b.path()
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..]),
    )
    .unwrap();
    assert!(basis::blob(&q, &head, "tasks/.config.toml", None, &mut 0).is_err());
}
#[test]
fn validation_read_cache_preserves_caps_and_archive_integrity() {
    if isolated("retained::read::tests::validation_read_cache_preserves_caps_and_archive_integrity")
    {
        return;
    }
    use sha2::{Digest, Sha256};
    let (dir, p) = fixture();
    let bytes = b"## Notes\n\n- 2026-11-10T00:00:00Z (fixture): original evidence\n";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let name = format!("notes-{hash}.notes");
    let header = format!(
        "---\nid: sci-000001\ntitle: Tiny\nstatus: idea\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\nnote_archives: [\"v1:{hash}:1\"]\n---\n\n## Notes\n\n- 2026-11-10T00:00:00Z (fixture): {}\n",
        crate::attachments::attached_note(&name, bytes.len() as u64, None)
    );
    let files = dir.path().join("tasks/files/sci-000001");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::write(files.join(&name), bytes).unwrap();
    std::fs::write(dir.path().join("tasks/sci-000001.md"), &header).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "archived"]);
    let p = start(&p).unwrap();
    let head = basis::resolve(&p, "HEAD").unwrap();
    let id = crate::model::TaskId::parse("sci-000001").unwrap();
    let b = crate::retained::model::Basis {
        project: "sci".into(),
        layout: "in-tree".into(),
        records_subdir: ".".into(),
        commit: head.clone(),
        task_path: "tasks/sci-000001.md".into(),
        retained_ref: "refs/heads/main".into(),
    };
    crate::history::begin_metrics();
    let mut fetched = 0;
    for _ in 0..2 {
        assert_eq!(
            basis::task_at(&p, &id, &b, &mut fetched)
                .unwrap()
                .notes
                .len(),
            2
        );
    }
    assert_eq!(fetched, (header.len() + bytes.len()) as u64);
    assert_eq!(crate::history::metadata_bytes_read(), fetched);
    let mut small = p.clone();
    small.attachments_max_bytes = 1;
    assert!(basis::task_at(&small, &id, &b, &mut 0).is_err());
    let raw = Project::open(dir.path()).unwrap();
    crate::history::begin_metrics();
    for _ in 0..2 {
        basis::task_at(&raw, &id, &b, &mut 0).unwrap();
    }
    assert_eq!(
        crate::history::metadata_bytes_read(),
        2 * (header.len() + bytes.len()) as u64
    );
    // A distinct pinned task with an incorrect count must not reuse the parsed task.
    std::fs::write(
        dir.path().join("tasks/sci-000001.md"),
        header.replace(&format!("v1:{hash}:1"), &format!("v1:{hash}:2")),
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "invalid count"]);
    let mut bad = b.clone();
    bad.commit = git(dir.path(), &["rev-parse", "HEAD"]);
    assert!(basis::task_at(&p, &id, &bad, &mut 0).is_err());
    std::fs::write(files.join(&name), b"corrupt").unwrap();
    std::fs::write(dir.path().join("tasks/sci-000001.md"), header).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "invalid digest"]);
    bad.commit = git(dir.path(), &["rev-parse", "HEAD"]);
    assert!(basis::task_at(&p, &id, &bad, &mut 0).is_err());
    let unsafe_path = dir.path().join("tasks/unsafe");
    std::fs::write(&unsafe_path, "regular").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "regular payload"]);
    let regular = git(dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(
        basis::blob(&p, &regular, "tasks/unsafe", None, &mut 0).unwrap(),
        b"regular"
    );
    std::fs::remove_file(&unsafe_path).unwrap();
    std::os::unix::fs::symlink("missing", &unsafe_path).unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "unsafe payload"]);
    let unsafe_commit = git(dir.path(), &["rev-parse", "HEAD"]);
    assert!(basis::blob(&p, &unsafe_commit, "tasks/unsafe", None, &mut 0).is_err());
}

#[test]
fn validation_read_poisoned_session_is_typed() {
    if isolated("retained::read::tests::validation_read_poisoned_session_is_typed") {
        return;
    }
    let (_dir, p) = fixture();
    let p = start(&p).unwrap();
    let session = p.retention_reads.as_ref().unwrap().clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = session.0.lock().unwrap();
            panic!("fixture poisons lock");
        })
        .join()
        .is_err()
    );
    assert!(matches!(
        basis::resolve(&p, "HEAD"),
        Err(crate::error::Error::RetainedHistoryBlocked(_))
    ));
}
