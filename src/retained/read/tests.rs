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

#[test]
fn validation_read_live_views_share_immutable_entries() {
    if isolated("retained::read::tests::validation_read_live_views_share_immutable_entries") {
        return;
    }
    let (dir, original) = fixture();
    let pinned = start(&original).unwrap();
    let live = live(&pinned);
    let pinned_head_before = basis::resolve(&pinned, "HEAD").unwrap();
    let object = format!("{pinned_head_before}:tasks/.config.toml");
    let first_blob = run(&pinned, &["cat-file", "blob", &object]).unwrap();
    assert!(first_blob.bytes_read > 0);
    let cached_blob = run(&live, &["cat-file", "blob", &object]).unwrap();
    assert_eq!(cached_blob.bytes_read, 0);
    std::fs::write(dir.path().join("unrelated"), "next").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "next"]);
    let live_head_after = basis::resolve(&live, "HEAD").unwrap();
    let pinned_head_after = basis::resolve(&pinned, "HEAD").unwrap();
    assert_ne!(live_head_after, pinned_head_before);
    assert_eq!(pinned_head_after, pinned_head_before);
    let alias_dir = tempfile::tempdir().unwrap();
    let alias_path = alias_dir.path().join("records");
    std::os::unix::fs::symlink(dir.path(), &alias_path).unwrap();
    let mut alias = original.clone();
    alias.records = alias_path;
    let alias = inherit(&live, alias).unwrap();
    assert_eq!(basis::resolve(&alias, "HEAD").unwrap(), live_head_after);
    let before = starts("cat-file");
    assert_eq!(
        run(&alias, &["cat-file", "blob", &object])
            .unwrap()
            .bytes_read,
        0
    );
    assert_eq!(starts("cat-file"), before);
    let (other_dir, other) = fixture();
    let other = inherit(&live, other).unwrap();
    let oid = git(other_dir.path(), &["rev-parse", "HEAD:tasks/.config.toml"]);
    std::fs::remove_file(
        other_dir
            .path()
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..]),
    )
    .unwrap();
    assert!(
        basis::blob(
            &other,
            &pinned_head_before,
            "tasks/.config.toml",
            None,
            &mut 0
        )
        .is_err()
    );
    git(dir.path(), &["branch", "-m", "renamed"]);
    assert_eq!(
        basis::git(&live, &["symbolic-ref", "-q", "HEAD"]).unwrap(),
        b"refs/heads/renamed\n"
    );
    assert!(basis::git(&live, &["show-ref", "--verify", "refs/heads/main"]).is_err());
    remember_lineage(&pinned).unwrap();
    assert!(lineage_cached(&pinned).unwrap());
    assert!(!lineage_cached(&live).unwrap());
    refresh(&live).unwrap();
    assert!(!lineage_cached(&pinned).unwrap());
    assert_eq!(basis::resolve(&pinned, "HEAD").unwrap(), pinned_head_before);
    assert_eq!(
        run(&live, &["cat-file", "blob", &object])
            .unwrap()
            .bytes_read,
        0
    );
    let separate = start_live(&original).unwrap();
    assert_eq!(basis::resolve(&separate, "HEAD").unwrap(), live_head_after);
    assert!(super::live(&original).retention_reads.is_none());
    std::fs::write(
        dir.path().join("tasks/.config.toml"),
        "prefix = \"other\"\n",
    )
    .unwrap();
    assert!(crate::retained::freshness::context(&live).is_err());
}

#[test]
fn validation_read_memo_budget() {
    if isolated("retained::read::tests::validation_read_memo_budget") {
        return;
    }
    let (_dir, original) = fixture();
    let p = start(&original).unwrap();
    let permits = (0..2049)
        .filter_map(|_| reserve_memo(&p, 1).unwrap())
        .collect::<Vec<_>>();
    {
        let state = p.retention_reads.as_ref().unwrap().lock().unwrap();
        let entries = state.memo_entries;
        let charged_bytes = state.used_bytes;
        assert!(entries <= 2048 && charged_bytes <= 32 * 1024 * 1024);
        assert_eq!(entries, 2048);
        assert_eq!(charged_bytes, 2048 * 65);
    }
    assert!(reserve_memo(&p, 1).unwrap().is_none());
    let head = basis::resolve(&p, "HEAD").unwrap();
    let object = format!("{head}:tasks/.config.toml");
    for _ in 0..2 {
        let out = run(&p, &["cat-file", "blob", &object]).unwrap();
        assert_eq!(out.output.stdout, b"prefix = \"sci\"\n");
        assert!(out.bytes_read > 0);
        assert!(basis::resolve(&p, &"0".repeat(40)).is_err());
    }
    drop(permits);
    assert_eq!(
        p.retention_reads
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .used_bytes,
        0
    );
    let permit = reserve_memo(&p, 32 * 1024 * 1024 - 64).unwrap().unwrap();
    assert!(reserve_memo(&p, 0).unwrap().is_none());
    drop(permit);
    assert!(reserve_memo(&p, usize::MAX).unwrap().is_none());
    assert!(reserve_memo(&original, 1).unwrap().is_none());
    p.retention_reads
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .max_entries = 2;
    let permit = reserve_memo(&p, 1).unwrap().unwrap();
    run(&p, &["cat-file", "blob", &object]).unwrap();
    let charged = p
        .retention_reads
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .used_bytes;
    assert_eq!(
        run(&p, &["cat-file", "blob", &object]).unwrap().bytes_read,
        0
    );
    assert_eq!(
        p.retention_reads
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .used_bytes,
        charged
    );
    assert!(reserve_memo(&p, 1).unwrap().is_none());
    drop(permit);
    assert!(reserve_memo(&p, 1).unwrap().is_some());
}

#[test]
fn validation_read_exact_queries_and_errors() {
    if isolated("retained::read::tests::validation_read_exact_queries_and_errors") {
        return;
    }
    let (dir, original) = fixture();
    let old = git(dir.path(), &["rev-parse", "HEAD"]);
    std::fs::write(dir.path().join("unrelated"), "next").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "next"]);
    let new = git(dir.path(), &["rev-parse", "HEAD"]);
    let p = start_live(&original).unwrap();
    let range = format!("{old}..{new}");
    let commit = format!("{new}^{{commit}}");
    let bad = "0".repeat(40);
    let missing_commit = format!("{bad}^{{commit}}");
    let queries = [
        vec!["rev-parse", "--verify", "--end-of-options", &commit],
        vec![
            "diff",
            "--quiet",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            &old,
            &new,
            "--",
            "unrelated",
        ],
        vec![
            "diff",
            "--quiet",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            &old,
            &new,
            "--",
            "tasks",
        ],
        vec!["rev-list", "--parents", &range],
        vec!["rev-list", "--parents", &new],
    ];
    for args in queries {
        let before = starts(args[0]);
        let first = run(&p, &args).unwrap().output;
        let second = run(&p, &args).unwrap().output;
        assert_eq!(first.status, second.status);
        assert_eq!(first.stdout, second.stdout);
        assert_eq!(starts(args[0]) - before, 1, "{args:?}");
    }
    for args in [
        vec!["rev-parse", "--verify", "--end-of-options", &bad],
        vec!["rev-parse", "--verify", "--end-of-options", &missing_commit],
        vec![
            "diff",
            "--quiet",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            &bad,
            &new,
            "--",
            "tasks",
        ],
        vec!["rev-list", "--parents", &bad],
        vec!["rev-list", "--parents", "HEAD"],
        vec!["rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"],
    ] {
        let before = starts(args[0]);
        for _ in 0..2 {
            run(&p, &args).unwrap();
        }
        assert_eq!(starts(args[0]) - before, 2, "{args:?}");
    }
    for args in [
        vec!["rev-list", "--parents", "HEAD"],
        vec!["rev-list", "--parents", "HEAD..main"],
        vec!["rev-list", "--parents", &format!("{old}...{new}")],
        vec!["rev-list", "--parents", &format!("{new}^1")],
        vec!["diff", "--quiet", &old, &new, "--", "tasks"],
    ] {
        assert!(!cacheable(&args), "{args:?}");
    }
}
