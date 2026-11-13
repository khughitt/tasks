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
fn explicit_context_reads_selected_repository_without_repointing_owner() {
    if isolated(
        "retained::read::tests::explicit_context_reads_selected_repository_without_repointing_owner",
    ) {
        return;
    }
    let (_destination, owner) = fixture();
    let (source_dir, source) = fixture();
    std::fs::write(
        source_dir.path().join("source-only"),
        b"original source bytes",
    )
    .unwrap();
    git(source_dir.path(), &["add", "."]);
    git(source_dir.path(), &["commit", "-qm", "source payload"]);
    let owner = start(&owner).unwrap();
    let destination_records = owner.records.clone();
    let context = at(&owner, &source.records).unwrap();
    let commit = git(source_dir.path(), &["rev-parse", "HEAD"]);
    let object = format!("{commit}:source-only");
    let read = context.run(&["cat-file", "blob", &object]).unwrap();
    assert!(read.output.status.success());
    assert_eq!(read.output.stdout, b"original source bytes");
    assert_eq!(context.root(), source_dir.path().canonicalize().unwrap());
    assert_eq!(owner.records, destination_records);
}

#[test]
fn explicit_context_does_not_reuse_equal_object_from_owner_repository() {
    if isolated(
        "retained::read::tests::explicit_context_does_not_reuse_equal_object_from_owner_repository",
    ) {
        return;
    }
    let (destination_dir, owner) = fixture();
    let (source_dir, source) = fixture();
    let owner = start(&owner).unwrap();
    let destination = at(&owner, &owner.records).unwrap();
    let commit = git(destination_dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(commit, git(source_dir.path(), &["rev-parse", "HEAD"]));
    let object = format!("{commit}:tasks/.config.toml");
    assert!(
        destination
            .run(&["cat-file", "blob", &object])
            .unwrap()
            .output
            .status
            .success()
    );
    let blob = git(source_dir.path(), &["rev-parse", "HEAD:tasks/.config.toml"]);
    std::fs::remove_file(
        source_dir
            .path()
            .join(".git/objects")
            .join(&blob[..2])
            .join(&blob[2..]),
    )
    .unwrap();
    let context = at(&owner, &source.records).unwrap();
    assert!(
        !context
            .run(&["cat-file", "blob", &object])
            .unwrap()
            .output
            .status
            .success()
    );
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
        vec!["rev-list", "--first-parent", &new],
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

fn range_fixture() -> (
    tempfile::TempDir,
    Project,
    crate::model::TaskId,
    basis::PinnedTask,
) {
    let (dir, p) = fixture();
    let raw = "---\nid: sci-000001\ntitle: Tiny\nstatus: idea\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n";
    std::fs::write(dir.path().join("tasks/sci-000001.md"), raw).unwrap();
    std::fs::create_dir_all(dir.path().join("tasks/files/sci-000001")).unwrap();
    std::fs::write(
        dir.path().join("tasks/files/sci-000001/evidence"),
        "original",
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "participant"]);
    let id = crate::model::TaskId::parse("sci-000001").unwrap();
    let pinned = basis::read(&p, &id, None).unwrap();
    (dir, start_live(&p).unwrap(), id, pinned)
}
fn range_tip(p: &Project, b: &crate::retained::model::Basis) -> crate::retained::model::Basis {
    let mut b = b.clone();
    b.commit = basis::resolve(p, "HEAD").unwrap();
    b
}
fn commit_file(dir: &Path, path: &str, bytes: &str) {
    std::fs::write(dir.join(path), bytes).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "change"]);
}
#[test]
fn retained_range_warm_delta() {
    if isolated("retained::read::tests::retained_range_warm_delta") {
        return;
    }
    use crate::retained::freshness::{ProgressPolicy::Exact, check, warm};
    let (dir, p, id, pinned) = range_fixture();
    commit_file(dir.path(), "unrelated", "H0");
    let h0 = range_tip(&p, &pinned.basis);
    let proof = warm(&p, &id, &pinned.basis, &h0, &pinned.task, Exact)
        .unwrap()
        .unwrap();
    for advance in [false, true] {
        if advance {
            commit_file(dir.path(), "unrelated", "H");
        }
        let h = range_tip(&p, &h0);
        let diff_starts = starts("diff");
        let warm_verdict = check(
            &p,
            &id,
            &pinned.basis,
            &h,
            &pinned.task,
            Exact,
            Some(&proof),
        )
        .map_err(|e| e.to_string());
        let new_diffs = starts("diff") - diff_starts;
        // Same-tip endpoints are already immutable cached queries; an advance checks its endpoint and new edge.
        assert_eq!(new_diffs, if advance { 2 } else { 0 });
        let full_verdict = check(
            &Project::open(dir.path()).unwrap(),
            &id,
            &pinned.basis,
            &h,
            &pinned.task,
            Exact,
            None,
        )
        .map_err(|e| e.to_string());
        assert_eq!(warm_verdict, full_verdict);
        assert!(warm_verdict.is_ok());
        let range_starts = starts("rev-list");
        check(
            &p,
            &id,
            &pinned.basis,
            &h,
            &pinned.task,
            Exact,
            Some(&proof),
        )
        .unwrap();
        assert_eq!(
            starts("rev-list"),
            range_starts,
            "full-OID delta enumeration is reusable"
        );
    }
    let raw = Project::open(dir.path()).unwrap();
    assert!(
        warm(&raw, &id, &pinned.basis, &h0, &pinned.task, Exact)
            .unwrap()
            .is_none()
    );
    p.retention_reads
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .max_entries = 0;
    assert!(
        warm(&p, &id, &pinned.basis, &h0, &pinned.task, Exact)
            .unwrap()
            .is_none()
    );
    check(
        &p,
        &id,
        &pinned.basis,
        &range_tip(&p, &h0),
        &pinned.task,
        Exact,
        None,
    )
    .unwrap();
    commit_file(dir.path(), "tasks/files/sci-000001/evidence", "changed");
    assert!(
        warm(
            &p,
            &id,
            &pinned.basis,
            &range_tip(&p, &h0),
            &pinned.task,
            Exact
        )
        .is_err(),
        "saturation must not turn failed validation into None"
    );
}
#[test]
fn retained_range_warm_merge_edges() {
    if isolated("retained::read::tests::retained_range_warm_merge_edges") {
        return;
    }
    use crate::retained::freshness::{ProgressPolicy::Exact, check, warm};
    let (dir, p, id, pinned) = range_fixture();
    commit_file(dir.path(), "unrelated", "H0");
    let h0 = range_tip(&p, &pinned.basis);
    let proof = warm(&p, &id, &pinned.basis, &h0, &pinned.task, Exact)
        .unwrap()
        .unwrap();
    // Every parent edge counts, even when that parent is excluded as an old H0 ancestor.
    let old_parent = git(
        dir.path(),
        &["rev-parse", &format!("{}^", pinned.basis.commit)],
    );
    let tree = git(
        dir.path(),
        &["rev-parse", &format!("{}^{{tree}}", h0.commit)],
    );
    let merge = git(
        dir.path(),
        &[
            "commit-tree",
            &tree,
            "-p",
            &h0.commit,
            "-p",
            &old_parent,
            "-m",
            "boundary parent",
        ],
    );
    git(dir.path(), &["reset", "--hard", &merge]);
    let boundary = range_tip(&p, &h0);
    let warm_verdict = check(
        &p,
        &id,
        &pinned.basis,
        &boundary,
        &pinned.task,
        Exact,
        Some(&proof),
    )
    .map_err(|e| e.to_string());
    let full_verdict = check(&p, &id, &pinned.basis, &boundary, &pinned.task, Exact, None)
        .map_err(|e| e.to_string());
    assert_eq!(warm_verdict, full_verdict);
    assert!(
        warm_verdict.is_err(),
        "old parent at the boundary must still be compared"
    );
    for revert in [false, true] {
        git(dir.path(), &["reset", "--hard", &h0.commit]);
        git(
            dir.path(),
            &["checkout", "-B", "side", &pinned.basis.commit],
        );
        commit_file(dir.path(), "tasks/files/sci-000001/evidence", "changed");
        if revert {
            commit_file(dir.path(), "tasks/files/sci-000001/evidence", "original");
        }
        git(dir.path(), &["checkout", "main"]);
        git(
            dir.path(),
            &["merge", "--no-ff", "-s", "ours", "side", "-m", "merge"],
        );
        let h = range_tip(&p, &h0);
        let warm_verdict = check(
            &p,
            &id,
            &pinned.basis,
            &h,
            &pinned.task,
            Exact,
            Some(&proof),
        )
        .map_err(|e| e.to_string());
        let full_verdict =
            check(&p, &id, &pinned.basis, &h, &pinned.task, Exact, None).map_err(|e| e.to_string());
        assert_eq!(warm_verdict, full_verdict);
        assert!(
            warm_verdict.is_err(),
            "newly reachable side history and boundary parents must be checked"
        );
    }
}
#[test]
fn retained_range_warm_invalidation() {
    if isolated("retained::read::tests::retained_range_warm_invalidation") {
        return;
    }
    use crate::retained::freshness::{
        ProgressPolicy::{Exact, PendingNotes},
        check, warm,
    };
    let (dir, p, id, pinned) = range_fixture();
    let h0 = pinned.basis.clone();
    let proof = warm(&p, &id, &h0, &h0, &pinned.task, Exact)
        .unwrap()
        .unwrap();
    for path in ["tasks/sci-000001.md", "tasks/files/sci-000001/evidence"] {
        let original = std::fs::read_to_string(dir.path().join(path)).unwrap();
        commit_file(dir.path(), path, &format!("{original}\n"));
        commit_file(dir.path(), path, &original);
        let h = range_tip(&p, &h0);
        assert!(
            check(&p, &id, &h0, &h, &pinned.task, Exact, Some(&proof)).is_err(),
            "ABA {path}"
        );
        git(dir.path(), &["reset", "--hard", &h0.commit]);
    }
    use std::os::unix::fs::PermissionsExt;
    for path in ["tasks/sci-000001.md", "tasks/files/sci-000001/evidence"] {
        std::fs::set_permissions(
            dir.path().join(path),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "mode change"]);
        assert!(
            check(
                &p,
                &id,
                &h0,
                &range_tip(&p, &h0),
                &pinned.task,
                PendingNotes,
                Some(&proof)
            )
            .is_err()
        );
        git(dir.path(), &["reset", "--hard", &h0.commit]);
    }
    let mut appended = pinned.task.clone();
    appended.notes.push(crate::model::Note {
        at: "2026-11-10T00:00:00Z".into(),
        by: "fixture".into(),
        text: "ordinary progress".into(),
        provenance: None,
        occurrence: Default::default(),
    });
    let physical = crate::history::physical_for_save(&appended, |_| unreachable!()).unwrap();
    let serialized = crate::format::serialize_physical(&physical);
    commit_file(dir.path(), "tasks/sci-000001.md", &serialized);
    let pending = range_tip(&p, &h0);
    let progress = warm(&p, &id, &h0, &pending, &pinned.task, PendingNotes)
        .unwrap()
        .unwrap();
    check(
        &p,
        &id,
        &h0,
        &pending,
        &pinned.task,
        PendingNotes,
        Some(&progress),
    )
    .unwrap();
    assert!(
        check(&p, &id, &h0, &pending, &pinned.task, Exact, Some(&progress)).is_err(),
        "changed policy must check full old range"
    );
    // A parsed-equal byte edit is not writer output, even under PendingNotes.
    commit_file(
        dir.path(),
        "tasks/sci-000001.md",
        &format!("{serialized}\n"),
    );
    assert!(
        check(
            &p,
            &id,
            &h0,
            &range_tip(&p, &pending),
            &pinned.task,
            PendingNotes,
            Some(&progress)
        )
        .is_err()
    );
    git(dir.path(), &["reset", "--hard", &h0.commit]);
    let mut changed = h0.clone();
    changed.task_path = "tasks/sci-000002.md".into();
    assert!(check(&p, &id, &h0, &changed, &pinned.task, Exact, Some(&proof)).is_err());
    changed = h0.clone();
    changed.retained_ref = "refs/heads/other".into();
    assert!(check(&p, &id, &h0, &changed, &pinned.task, Exact, Some(&proof)).is_err());
    // Policy mismatch must fall back to exact validation of its requested policy.
    check(&p, &id, &h0, &h0, &pinned.task, PendingNotes, Some(&proof)).unwrap();
    let mut task = pinned.task.clone();
    task.title = "changed input".into();
    check(&p, &id, &h0, &h0, &task, Exact, Some(&proof)).unwrap();
    // Rebinding P cannot reuse a proof that starts after a protected ABA.
    commit_file(dir.path(), "tasks/files/sci-000001/evidence", "changed");
    commit_file(dir.path(), "tasks/files/sci-000001/evidence", "original");
    let later = range_tip(&p, &h0);
    let later_proof = warm(&p, &id, &later, &later, &pinned.task, Exact)
        .unwrap()
        .unwrap();
    assert!(
        check(
            &p,
            &id,
            &h0,
            &later,
            &pinned.task,
            Exact,
            Some(&later_proof)
        )
        .is_err()
    );
    git(dir.path(), &["reset", "--hard", &h0.commit]);
    // H0 on a divergent branch is not a proof of the new tip's history.
    commit_file(dir.path(), "unrelated", "divergent warm");
    let side = range_tip(&p, &h0);
    let side_proof = warm(&p, &id, &h0, &side, &pinned.task, Exact)
        .unwrap()
        .unwrap();
    git(dir.path(), &["reset", "--hard", &h0.commit]);
    commit_file(dir.path(), "tasks/files/sci-000001/evidence", "changed");
    commit_file(dir.path(), "tasks/files/sci-000001/evidence", "original");
    assert!(
        check(
            &p,
            &id,
            &h0,
            &range_tip(&p, &h0),
            &pinned.task,
            Exact,
            Some(&side_proof)
        )
        .is_err()
    );
    git(dir.path(), &["reset", "--hard", &h0.commit]);
    git(dir.path(), &["checkout", "-qb", "other"]);
    assert!(check(&p, &id, &h0, &h0, &pinned.task, Exact, Some(&proof)).is_err());
    git(dir.path(), &["checkout", "main"]);
    std::fs::write(
        dir.path().join("tasks/.config.toml"),
        "prefix = \"other\"\n",
    )
    .unwrap();
    assert!(check(&p, &id, &h0, &h0, &pinned.task, Exact, Some(&proof)).is_err());
}

#[test]
fn retained_range_warm_query_admission() {
    if isolated("retained::read::tests::retained_range_warm_query_admission") {
        return;
    }
    let (dir, p, _, pinned) = range_fixture();
    commit_file(dir.path(), "unrelated", "tip");
    let head = basis::resolve(&p, "HEAD").unwrap();
    let args = [
        "rev-list",
        "--parents",
        &head,
        "--not",
        &pinned.basis.commit,
    ];
    let before = starts("rev-list");
    let first = run(&p, &args).unwrap();
    assert!(first.output.status.success());
    assert_eq!(run(&p, &args).unwrap().output.stdout, first.output.stdout);
    assert_eq!(starts("rev-list") - before, 1);
    let mutable_args = [
        "rev-list",
        "--parents",
        "HEAD",
        "--not",
        &pinned.basis.commit,
    ];
    let before = starts("rev-list");
    let initial = run(&p, &mutable_args).unwrap().output.stdout;
    commit_file(dir.path(), "unrelated", "advance");
    assert_ne!(run(&p, &mutable_args).unwrap().output.stdout, initial);
    assert_eq!(
        starts("rev-list") - before,
        2,
        "mutable refs are never query cache keys"
    );
    let missing = "0".repeat(40);
    let args = ["rev-list", "--parents", &head, "--not", &missing];
    let before = starts("rev-list");
    for _ in 0..2 {
        assert!(!run(&p, &args).unwrap().output.status.success());
    }
    assert_eq!(
        starts("rev-list") - before,
        2,
        "failed enumerations are not cached"
    );
}

#[test]
fn validation_read_query_admission_observer() {
    if isolated("retained::read::tests::validation_read_query_admission_observer") {
        return;
    }
    use std::{
        io::{BufRead, BufReader, Read},
        os::unix::net::UnixListener,
    };
    let (dir, p) = fixture();
    let p = start(&p).unwrap();
    let session = p.retention_reads.as_ref().unwrap();
    let snapshot = session.snapshot(&p.records).unwrap();
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let socket = dir.path().join("query.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    // isolated() runs this exact test alone in its own credential-free subprocess.
    unsafe {
        std::env::set_var("TASKS_RETAINED_TEST_SOCKET", &socket);
    }
    let object = format!("{head}:tasks/.config.toml");
    let cached = ["cat-file", "blob", object.as_str()];
    assert!(
        session
            .query(&snapshot, &cached)
            .unwrap()
            .output
            .status
            .success()
    );
    assert!(
        session
            .query(&snapshot, &cached)
            .unwrap()
            .output
            .status
            .success()
    );
    session.lock().unwrap().max_entries = 0;
    assert!(
        session
            .query(&snapshot, &cached)
            .unwrap()
            .output
            .status
            .success()
    );
    let uncached = ["ls-tree", "-z", head.as_str(), "--", "tasks/.config.toml"];
    for _ in 0..2 {
        assert!(
            session
                .query(&snapshot, &uncached)
                .unwrap()
                .output
                .status
                .success()
        );
    }
    let missing = format!("{}:missing", "0".repeat(40));
    assert!(
        !session
            .query(&snapshot, &["cat-file", "blob", &missing])
            .unwrap()
            .output
            .status
            .success()
    );
    unsafe {
        std::env::remove_var("TASKS_RETAINED_TEST_SOCKET");
    }
    let mut frames = Vec::new();
    loop {
        let (stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("{e}"),
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream)
            .take(4096)
            .read_line(&mut line)
            .unwrap();
        frames.push(serde_json::from_str::<serde_json::Value>(&line).unwrap());
    }
    let admissions = frames
        .iter()
        .filter(|f| f["event"] == "query_admitted")
        .collect::<Vec<_>>();
    assert_eq!(
        admissions.len(),
        1,
        "only the completed retained query is admitted: {frames:?}"
    );
    let signature: serde_json::Value =
        serde_json::from_str(admissions[0]["phase"].as_str().unwrap()).unwrap();
    assert_eq!(
        signature,
        serde_json::json!({"source":"immutable_read","repository":snapshot.root,"argv":cached})
    );
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["event"] == "cache_saturated")
            .count(),
        2
    );
    assert_eq!(
        frames.iter().filter(|f| f["event"] == "git_start").count(),
        4,
        "cache hit adds no Git start, failed/unadmitted queries do"
    );
}
