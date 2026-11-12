mod common;

use common::{TestEnv, as_agent, err_detail, err_kind, git, id_of, isolated_worktree};

#[test]
fn foreign_dependency_refusal_explains_publication_without_changing_records() {
    let mut env = TestEnv::new();
    let upstream = env.init("up");
    let side = upstream.join("wt");
    isolated_worktree(&env, &upstream, &side);
    let producer = id_of(env.json(&side, &["add", "Producer"]));
    let downstream = env.init("down");
    let consumer = id_of(env.json(&downstream, &["add", "Consumer"]));
    let path = downstream.join(format!("tasks/{consumer}.md"));
    let before = std::fs::read(&path).unwrap();

    for args in [
        vec!["dep", &consumer, "--on", &producer],
        vec!["edit", &consumer, "--depends", &producer],
        vec!["add", "Another consumer", "--depends", &producer],
    ] {
        let out = env.cmd(&downstream).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(err_kind(&out), "unresolvable_id");
        let detail = err_detail(&out);
        assert!(detail.contains("registered records"), "{detail}");
        assert!(detail.contains(upstream.to_str().unwrap()), "{detail}");
        assert!(detail.contains("publish"), "{detail}");
        assert!(detail.contains("even when"), "{detail}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            env.json(&downstream, &["list"])["tasks"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    // The invocation can itself be in the upstream worktree while add routes away.
    let out = env
        .cmd(&side)
        .args([
            "add",
            "Consumer",
            "--project",
            "down",
            "--depends",
            &producer,
        ])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert!(err_detail(&out).contains("registered records"));

    let out = env
        .cmd(&downstream)
        .args(["dep", &consumer, "--on", "up-ffffff"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert!(err_detail(&out).contains("Check the ID; if"));

    let out = env
        .cmd(&downstream)
        .args(["dep", &consumer, "--on", "zzz-000001"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert!(err_detail(&out).contains("not registered"));
    assert!(!err_detail(&out).contains("publish"));

    std::fs::remove_file(upstream.join("tasks/.config.toml")).unwrap();
    let out = env
        .cmd(&downstream)
        .args(["dep", &consumer, "--on", &producer])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert!(err_detail(&out).contains("unavailable"));
    assert!(!err_detail(&out).contains("publish"));
    assert_eq!(std::fs::read(&path).unwrap(), before);

    std::fs::write(upstream.join("tasks/.config.toml"), "not valid = [").unwrap();
    let out = env
        .cmd(&downstream)
        .args(["dep", &consumer, "--on", &producer])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "config");
    assert!(!err_detail(&out).contains("publish"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn detached_upstream_worktrees_publish_directly_to_shared_records() {
    let mut env = TestEnv::new();
    let code = tempfile::tempdir().unwrap();
    let records = tempfile::tempdir().unwrap();
    git(records.path(), &["init", "-q", "-b", "main"]);
    std::fs::write(records.path().join("README"), "records\n").unwrap();
    git(records.path(), &["add", "README"]);
    git(records.path(), &["commit", "-qm", "seed"]);
    git(code.path(), &["init", "-q", "-b", "main"]);
    std::fs::write(code.path().join("runtime"), "live\n").unwrap();
    env.json(
        code.path(),
        &[
            "init",
            "--prefix",
            "up",
            "--records",
            records.path().to_str().unwrap(),
        ],
    );
    let side = code.path().join("wt");
    isolated_worktree(&env, code.path(), &side);
    let producer = id_of(env.json(&side, &["add", "Producer"]));
    let downstream = env.init("down");
    let consumer = id_of(env.json(&downstream, &["add", "Consumer", "--depends", &producer]));
    assert!(
        env.json(&downstream, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    env.json(&side, &["done", &producer]);
    assert_eq!(
        env.json(code.path(), &["show", &producer])["task"]["status"],
        "done"
    );
    assert_eq!(
        env.json(&downstream, &["ready"])["tasks"][0]["id"],
        consumer
    );
    let record = format!("tasks/{producer}.md");
    assert!(!code.path().join(&record).exists());
    assert!(!side.join(&record).exists());
    let out = common::git_fixture::command(env.home.path(), records.path())
        .args(["show", &format!("HEAD:up/{record}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("status: done")
    );
    assert_eq!(
        std::fs::read_to_string(code.path().join("runtime")).unwrap(),
        "live\n"
    );
}

#[test]
fn record_publication_controls_readiness_and_survives_the_later_branch_merge() {
    // Identical published bytes merge cleanly; a later registered note needs a union.
    for divergent in [false, true] {
        let mut env = TestEnv::new();
        let upstream = env.init("up");
        std::fs::write(upstream.join("runtime"), "live\n").unwrap();
        let side = upstream.join("wt");
        isolated_worktree(&env, &upstream, &side);
        let producer = id_of(env.json(&side, &["add", "Producer"]));
        std::fs::write(side.join("runtime"), "candidate\n").unwrap();
        let out = as_agent(&env, &side, "producer-owner")
            .args(["start", &producer])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let downstream = env.init("down");
        let consumer = id_of(env.json(&downstream, &["add", "Consumer"]));
        assert_eq!(
            env.json(&downstream, &["show", &producer])["task"]["status"],
            "doing"
        );
        assert_eq!(
            env.fail(&downstream, &["dep", &consumer, "--on", &producer]),
            "unresolvable_id"
        );
        let record = format!("tasks/{producer}.md");
        git(&side, &["add", "--", &record]);
        git(&side, &["commit", "-qm", "record producer"]);
        git(&upstream, &["restore", "--source=wt", "--", &record]);
        git(&upstream, &["add", "--", &record]);
        git(&upstream, &["commit", "-qm", "publish producer"]);
        env.json(&downstream, &["dep", &consumer, "--on", &producer]);
        assert!(
            env.json(&downstream, &["ready"])["tasks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let out = as_agent(&env, &side, "other-owner")
            .args(["done", &producer])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "claimed");
        let out = as_agent(&env, &side, "producer-owner")
            .args(["done", &producer])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        git(&side, &["add", "--", &record]);
        git(&side, &["commit", "-qm", "complete producer"]);
        assert!(
            env.json(&downstream, &["ready"])["tasks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        git(&upstream, &["restore", "--source=wt", "--", &record]);
        git(&upstream, &["add", "--", &record]);
        git(&upstream, &["commit", "-qm", "publish completion"]);
        assert_eq!(
            env.json(&downstream, &["ready"])["tasks"][0]["id"],
            consumer
        );
        assert_eq!(
            std::fs::read_to_string(upstream.join("runtime")).unwrap(),
            "live\n"
        );

        if divergent {
            env.json(&upstream, &["note", &producer, "Registered history"]);
            git(&upstream, &["add", "--", &record]);
            git(&upstream, &["commit", "-qm", "registered note"]);
        }
        git(&side, &["add", "runtime"]);
        git(&side, &["commit", "-qm", "runtime candidate"]);
        let out = common::git_fixture::command(env.home.path(), &upstream)
            .args(["merge", "--no-edit", "wt"])
            .output()
            .unwrap();
        assert_eq!(out.status.success(), !divergent, "{out:?}");
        if divergent {
            // Keep the authoritative superset, not the branch's older completed copy.
            git(
                &upstream,
                &[
                    "restore",
                    "--source=HEAD",
                    "--staged",
                    "--worktree",
                    "--",
                    &record,
                ],
            );
            git(
                &upstream,
                &["commit", "-qm", "merge retaining published history"],
            );
            assert!(
                std::fs::read_to_string(upstream.join(&record))
                    .unwrap()
                    .contains("Registered history")
            );
        }
        env.json(&upstream, &["note", &producer, "After merge"]);
        assert_eq!(
            env.json(&downstream, &["show", &producer])["task"]["status"],
            "done"
        );
    }
}
