mod common;
use common::{
    TestEnv, alias_registry, as_agent, declare_retired, editor_script, err_detail, err_kind,
    error_detail, error_of, feedback_env, git, hold_project_lock, id_of, reap, write_claim,
    write_park,
};
use std::fs::File;
use std::time::Duration;

#[test]
fn rename_rewrites_the_project_and_keeps_inbound_refs_resolving() {
    let mut env = TestEnv::new();
    let dots = env.init("dot");
    let ops = env.init("ops");
    git(&dots, &["init", "-q", "-b", "main"]);
    let mine = id_of(env.json(&dots, &["add", "Mine", "-p", "2"]));
    let theirs = id_of(env.json(&ops, &["add", "Theirs", "-p", "2"]));
    env.json(&ops, &["dep", &theirs, "--on", &mine]);
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);

    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["prefix"], "dots");
    assert_eq!(v["previous"], "dot");
    assert_eq!(v["tasks"], 1);
    assert_eq!(v["recovery"], "fresh");
    assert_eq!(v["aliases"], serde_json::json!(["dot"]));

    let moved = format!("dots-{}", mine.split_once('-').unwrap().1);
    assert!(dots.join(format!("tasks/{moved}.md")).is_file());
    assert!(!dots.join(format!("tasks/{mine}.md")).exists());
    assert_eq!(env.json(&dots, &["show", &moved])["task"]["id"], moved);

    // The other project was not written to, and its stored ref still resolves.
    assert_eq!(
        env.json(&ops, &["show", &theirs])["task"]["depends"][0],
        mine
    );
    assert_eq!(env.json(&ops, &["show", &mine])["task"]["id"], moved);

    // A completed re-run reports Complete rather than tripping a fresh-operation refusal.
    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["recovery"], "complete");

    // --explain writes nothing and reports the same verdict.
    let before =
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert_eq!(
        env.json(&dots, &["rename", "dot", "dots", "--explain"])["recovery"],
        "complete"
    );
    assert_eq!(
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
        before
    );
}

#[test]
fn an_add_waiting_through_a_rename_never_writes_the_old_prefix() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let seed = id_of(env.json(&dir, &["add", "Seed", "-p", "2"]));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    let moved_seed = format!("dots-{}", seed.split_once('-').unwrap().1);

    // This tests revalidation, not the rename executor (covered separately above).
    // Hold the lock and establish a completed-rename fixture while the add waits.
    let lock_path = env.claim_store("dot").with_file_name("dot.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();

    let mut adder = env
        .raw(&dir)
        .args(["add", "Late", "-p", "2"])
        .spawn()
        .unwrap();
    // The kernel listing the child as a waiter on this lock proves it constructed its
    // old-prefix context and reached lock acquisition. A sleep alone could leave it not
    // yet started. Its descriptor table cannot prove it: spawn can return while the
    // child is still inside execve, before our own close-on-exec lock descriptor leaves it.
    let inode = std::os::unix::fs::MetadataExt::ino(&held.metadata().unwrap()).to_string();
    let pid = adder.id().to_string();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(
            adder.try_wait().unwrap().is_none(),
            "the add exited before acquiring its lock"
        );
        // A blocked waiter: `N: -> FLOCK  ADVISORY  WRITE <pid> <major:minor:inode> 0 EOF`.
        let waiting = std::fs::read_to_string("/proc/locks")
            .unwrap()
            .lines()
            .any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                fields.len() > 6
                    && fields[1] == "->"
                    && fields[2] == "FLOCK"
                    && fields[5] == pid
                    && fields[6].rsplit(':').next() == Some(inode.as_str())
            });
        if waiting {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the add never waited on its lock"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Only this one seed task exists. Install its renamed file, config and registry
    // directly while the lock is held; invoking rename here would wait on our lock too.
    let source = dir.join(format!("tasks/{seed}.md"));
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(
        dir.join(format!("tasks/{moved_seed}.md")),
        text.replacen(&format!("id: {seed}\n"), &format!("id: {moved_seed}\n"), 1),
    )
    .unwrap();
    std::fs::remove_file(source).unwrap();
    let config_path = dir.join("tasks/.config.toml");
    let mut config: toml::Value =
        toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["prefix"] = toml::Value::String("dots".into());
    std::fs::write(config_path, toml::to_string(&config).unwrap()).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    let projects = registry["projects"].as_table_mut().unwrap();
    let root = projects.remove("dot").unwrap();
    projects.insert("dots".into(), root);
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    alias_registry(&env, "dot", "dots");
    assert!(dir.join(format!("tasks/{moved_seed}.md")).is_file());

    drop(held);
    let out = adder.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let added: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let late = added["id"].as_str().unwrap();
    assert!(
        late.starts_with("dots-"),
        "the queued add must use the refreshed prefix: {added}"
    );
    assert_eq!(env.json(&dir, &["show", late])["task"]["title"], "Late");

    // No later rename can mask a stale write: only the queued add ran after the fixture.
    let stale: Vec<_> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("dot-"))
        .collect();
    assert!(
        stale.is_empty(),
        "an old-prefix file survived the rename: {stale:?}"
    );
    assert_eq!(
        env.json(&dir, &["show", &moved_seed])["task"]["id"],
        moved_seed
    );
    assert_eq!(
        env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn an_add_waiting_through_a_pending_rename_is_refused() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "Seed", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // An interrupted rename leaves the freeze in place; a waiter that wakes into it is
    // refused rather than writing under either name.
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();
    let out = env
        .raw(&dir)
        .args(["add", "Late", "-p", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
}

#[test]
fn a_pending_rename_freezes_the_project_including_the_p4_window() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "T", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // Stop after the config write: files and config renamed, registry untouched. This is
    // the window where a lookup by resolved prefix would miss rename/dot.toml entirely.
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();

    assert_eq!(env.fail(&dir, &["add", "New", "-p", "2"]), "validation");
    assert_eq!(env.fail(&dir, &["unregister", "dot"]), "validation");
    let config_before = std::fs::read(dir.join("tasks/.config.toml")).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let registry_before = std::fs::read(&registry_path).unwrap();
    let baseline_path = env.home.path().join(".local/state/tasks/rename/dot.toml");
    let baseline_before = std::fs::read(&baseline_path).unwrap();
    let files_before: std::collections::BTreeMap<_, _> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect();
    assert_eq!(
        env.fail(&dir, &["add", "Explicit", "--project", "dot", "-p", "2"]),
        "config"
    );
    assert_eq!(
        std::fs::read(dir.join("tasks/.config.toml")).unwrap(),
        config_before
    );
    assert_eq!(std::fs::read(registry_path).unwrap(), registry_before);
    assert_eq!(std::fs::read(baseline_path).unwrap(), baseline_before);
    let files_after: std::collections::BTreeMap<_, _> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect();
    assert_eq!(files_after, files_before);

    assert!(
        env.json(&dir, &["list"])["tasks"].is_array(),
        "reads still work"
    );

    // The target is reserved even though it is in no registry yet.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(fresh.path(), &["init", "--prefix", "dots"]),
        "config"
    );

    // And the rename completes on a re-run.
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots"])["recovery"],
        "resume_registry"
    );
    assert!(
        env.json(&dir, &["add", "New", "-p", "2"])["id"]
            .as_str()
            .unwrap()
            .starts_with("dots-")
    );
}

#[test]
fn rename_refuses_a_dirty_tree_a_live_claim_and_a_second_worktree() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation"); // uncommitted
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "claimed");
}

#[test]
fn rename_collapses_aliases_and_accepts_an_older_alias_as_source() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.json(&dir, &["rename", "dot", "dots"]);
    let v = env.json(&dir, &["rename", "dot", "config"]);
    assert_eq!(v["previous"], "dots");
    assert_eq!(v["aliases"], serde_json::json!(["dot", "dots"]));
    let registry: toml::Value =
        toml::from_str(&env.read(&env.home.path().join(".config"), "tasks/projects.toml")).unwrap();
    assert_eq!(registry["aliases"]["dot"].as_str(), Some("config"));
    assert_eq!(registry["aliases"]["dots"].as_str(), Some("config"));
    assert!(
        env.json(&dir, &["show", &id])["task"]["id"]
            .as_str()
            .unwrap()
            .starts_with("config-")
    );
}

#[test]
fn rename_preflight_refusals_leave_no_inventory_or_changed_files() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.init("taken");
    alias_registry(&env, "retired", "taken");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let path = dir.join(format!("tasks/{id}.md"));
    let original = std::fs::read_to_string(&path).unwrap();
    for target in ["../bad", "dot", "taken", "retired"] {
        let out = env
            .raw(&dir)
            .args(["rename", "dot", target])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(!env.home.path().join(".local/state/tasks/rename").exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    for invalid in [
        original.replace("priority: 2", "priority: 9"),
        original.replace(&format!("id: {id}"), "id: dot-ffffff"),
    ] {
        std::fs::write(&path, invalid).unwrap();
        assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "parse");
        assert!(!env.home.path().join(".local/state/tasks/rename").exists());
    }
}

#[test]
fn rename_explain_never_creates_locks_or_inventory_and_skips_authorization() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let registry = std::fs::read(&registry_path).unwrap();
    let config = std::fs::read(dir.join("tasks/.config.toml")).unwrap();
    // init creates the registry lock and, taking the project lock first, the state
    // directory with dot's lock; remove both so explain must prove no lock creation.
    std::fs::remove_file(registry_path.with_file_name("projects.lock")).unwrap();
    std::fs::remove_dir_all(env.home.path().join(".local/state/tasks")).unwrap();
    let value = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(value["recovery"], "fresh");
    assert!(!registry_path.with_file_name("projects.lock").exists());
    assert!(!env.home.path().join(".local/state/tasks").exists());
    assert_eq!(std::fs::read(&registry_path).unwrap(), registry);
    assert_eq!(
        std::fs::read(dir.join("tasks/.config.toml")).unwrap(),
        config
    );

    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
        "fresh"
    );
}

#[test]
fn rename_refuses_a_second_worktree_even_with_no_task_copies() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    let other = tempfile::tempdir().unwrap();
    git(
        &dir,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            other.path().to_str().unwrap(),
        ],
    );
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("worktree"));
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_pending_target_is_reserved_against_another_rename() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let other = env.init("other");
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(env.fail(&other, &["rename", "other", "dots"]), "config");
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/other.toml")
            .exists()
    );
}

#[test]
fn rename_explain_reports_refusal_without_touching_the_baseline() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    let inventory_path = env.home.path().join(".local/state/tasks/rename/dot.toml");
    let before = std::fs::read(&inventory_path).unwrap();
    std::fs::remove_file(dir.join(format!("tasks/{id}.md"))).unwrap();
    let out = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(out["recovery"], "refuse");
    assert!(out["warnings"][0].as_str().unwrap().contains("R4"));
    assert_eq!(std::fs::read(inventory_path).unwrap(), before);
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
}

#[test]
fn rename_an_older_alias_resumes_cleanup_using_the_real_source_inventory() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.json(&dir, &["rename", "dot", "dots"]);
    let stopped = env
        .raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "registry")
        .args(["rename", "dot", "config"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let baseline = env.home.path().join(".local/state/tasks/rename/dots.toml");
    assert!(baseline.is_file());
    let out = env.json(&dir, &["rename", "dot", "config"]);
    assert_eq!(out["recovery"], "resume_cleanup");
    assert_eq!(out["previous"], "dots");
    assert!(!baseline.exists());
    assert!(
        env.json(&dir, &["add", "After", "-p", "2"])["id"]
            .as_str()
            .unwrap()
            .starts_with("config-")
    );
}

#[test]
fn rename_reports_only_destination_files_written_by_this_invocation() {
    for (boundary, written, resumed, verdict) in [
        ("inventory", 0, 2, "resume_files"),
        ("file:0", 1, 1, "resume_files"),
        ("files", 2, 0, "resume_files"),
        ("config", 2, 0, "resume_registry"),
        ("registry", 2, 0, "resume_cleanup"),
        ("claims", 2, 0, "resume_cleanup"),
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        env.json(&dir, &["add", "One", "-p", "2"]);
        env.json(&dir, &["add", "Two", "-p", "2"]);
        let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
        assert_eq!(explain["tasks"], 0, "fresh explain at {boundary}");
        let output = env
            .raw(&dir)
            .env("TASKS_RENAME_STOP_AFTER", boundary)
            .args(["rename", "dot", "dots"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{boundary}: {output:?}");
        let stopped: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(stopped["tasks"], written, "stop {boundary}");
        let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
        assert_eq!(explain["tasks"], 0, "pending explain at {boundary}");
        let recovered = env.json(&dir, &["rename", "dot", "dots"]);
        assert_eq!(recovered["recovery"], verdict, "resume {boundary}");
        assert_eq!(recovered["tasks"], resumed, "resume {boundary}");
        let completed = env.json(&dir, &["rename", "dot", "dots"]);
        assert_eq!(completed["recovery"], "complete");
        assert_eq!(completed["tasks"], 0, "complete after {boundary}");
        assert_eq!(
            env.json(&dir, &["rename", "dot", "dots", "--explain"])["tasks"],
            0
        );
    }
}

#[test]
fn rename_migrates_park_entries_to_the_target_store() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let one = id_of(env.json(&dir, &["add", "One", "-p", "2"]));
    let two = id_of(env.json(&dir, &["add", "Two", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["park", &one, "a"])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["park", &two, "b", "--waiting-on", "user"])
        .assert()
        .success();
    write_claim(&env, "dot", "dot-ffffff", "ghost", false);

    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["parks"],
        2
    );
    let out = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(out["parks"], 2);
    assert!(
        !env.claim_store("dot").exists(),
        "the source store is removed"
    );
    let target = std::fs::read_to_string(env.claim_store("dots")).unwrap();
    assert!(
        target.contains(&format!("[parks.dots-{}]", &one[4..])),
        "{target}"
    );
    assert!(!target.contains("dot-"), "{target}");
    assert!(!target.contains("claims"), "{target}");

    let prime = env.json(&dir, &["prime"]);
    let ids: Vec<&str> = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| id.starts_with("dots-")), "{ids:?}");
    let completed = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(completed["recovery"], "complete");
    assert_eq!(completed["parks"], 2);
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["parks"],
        2
    );
}

#[test]
fn rename_refuses_a_target_store_that_holds_parks_before_any_mutation() {
    let mut env = TestEnv::new();
    let new = env.init("new");
    let theirs = id_of(env.json(&new, &["add", "Theirs", "-p", "2"]));
    as_agent(&env, &new, "agent-a")
        .args(["park", &theirs, "keep"])
        .assert()
        .success();
    env.json(&new, &["unregister", "new"]);
    let old = env.init("old");
    let mine = id_of(env.json(&old, &["add", "Mine", "-p", "2"]));
    as_agent(&env, &old, "agent-a")
        .args(["park", &mine, "move"])
        .assert()
        .success();
    let before_new = std::fs::read_to_string(env.claim_store("new")).unwrap();
    let before_old = std::fs::read_to_string(env.claim_store("old")).unwrap();

    let out = env
        .cmd(&old)
        .args(["rename", "old", "new"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains("target store holds 1 parked task"),
        "{}",
        err_detail(&out)
    );
    assert!(err_detail(&out).contains(&theirs), "{}", err_detail(&out));
    assert_eq!(
        std::fs::read_to_string(env.claim_store("new")).unwrap(),
        before_new
    );
    assert_eq!(
        std::fs::read_to_string(env.claim_store("old")).unwrap(),
        before_old
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/old.toml")
            .exists(),
        "no inventory"
    );
    assert!(
        old.join(format!("tasks/{mine}.md")).is_file(),
        "no file moved"
    );
}

#[test]
fn rename_carries_an_escalation_only_store_and_refuses_one_at_the_target() {
    let mut env = TestEnv::new();
    let dot = env.init("dot");
    let id = id_of(env.json(&dot, &["add", "T", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &dot, "agent-a")
        .args([
            "park",
            &id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    // Resume and finish the session so the store holds an escalation and nothing else.
    as_agent(&env, &dot, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &dot, "agent-a")
        .args(["edit", &id, "--status", "todo"])
        .assert()
        .success();
    let store = std::fs::read_to_string(env.claim_store("dot")).unwrap();
    assert!(
        store.contains("[escalations.") && !store.contains("[parks."),
        "{store}"
    );

    let v = env.json(&dot, &["rename", "dot", "dots"]);
    assert_eq!(v["escalations"], 1, "{v}");
    assert_eq!(v["parks"], 0);
    assert!(!env.claim_store("dot").exists());
    let store = std::fs::read_to_string(env.claim_store("dots")).unwrap();
    assert!(store.contains("[escalations.dots-"), "{store}");
    let new_id = id.replace("dot-", "dots-");
    let v = env.json(&dot, &["show", &new_id]);
    assert_eq!(v["escalation"]["level"], "high");
    let v = env.json(&dot, &["ready", "--max-complexity", "mid"]);
    assert!(
        v["tasks"].as_array().unwrap().is_empty(),
        "still hidden after the rename: {v}"
    );

    // A target store holding only an escalation is a destination conflict.
    let other = env.init("fam");
    let other_id = id_of(env.json(&other, &["add", "F", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &other, "agent-b")
        .args([
            "park",
            &other_id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    as_agent(&env, &other, "agent-b")
        .args(["start", &other_id])
        .assert()
        .success();
    as_agent(&env, &other, "agent-b")
        .args(["edit", &other_id, "--status", "todo"])
        .assert()
        .success();
    // Leave `fam` registered but make its store the target of a rename from `dots`.
    env.json(&other, &["unregister", "fam"]);
    let out = env
        .cmd(&dot)
        .args(["rename", "dots", "fam"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("target store holds"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rename_interrupted_after_the_store_write_resumes_and_a_tampered_destination_refuses() {
    for tamper in [false, true] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
        as_agent(&env, &dir, "agent-a")
            .args(["park", &id, "a"])
            .assert()
            .success();
        let escalated = id_of(env.json(&dir, &["add", "E", "-p", "2", "--complexity", "low"]));
        as_agent(&env, &dir, "agent-a")
            .args([
                "park",
                &escalated,
                "stuck",
                "--reason",
                "capability",
                "--complexity",
                "high",
            ])
            .assert()
            .success();
        // Resume and finish the session so this task's only surviving entry is the escalation.
        as_agent(&env, &dir, "agent-a")
            .args(["start", &escalated])
            .assert()
            .success();
        as_agent(&env, &dir, "agent-a")
            .args(["edit", &escalated, "--status", "todo"])
            .assert()
            .success();
        let stopped = env
            .raw(&dir)
            .env("TASKS_RENAME_STOP_AFTER", "store")
            .args(["rename", "dot", "dots"])
            .output()
            .unwrap();
        assert!(stopped.status.success(), "{stopped:?}");
        assert!(env.claim_store("dots").is_file(), "destination written");
        assert!(env.claim_store("dot").is_file(), "source still present");
        let dest = std::fs::read_to_string(env.claim_store("dots")).unwrap();
        assert!(
            dest.contains(&format!("[escalations.dots-{}]", &escalated[4..])),
            "{dest}"
        );
        if tamper {
            write_park(&env, "dots", "dots-ffffff", "someone", "agent", "/x");
            let out = env
                .cmd(&dir)
                .args(["rename", "dot", "dots"])
                .output()
                .unwrap();
            assert_eq!(err_kind(&out), "validation");
            assert!(
                err_detail(&out).contains("disagrees with inventory"),
                "{}",
                err_detail(&out)
            );
            assert!(
                env.claim_store("dot").is_file(),
                "the source is left in place"
            );
        } else {
            let resumed = env.json(&dir, &["rename", "dot", "dots"]);
            assert_eq!(resumed["recovery"], "resume_cleanup");
            assert_eq!(resumed["parks"], 1);
            assert_eq!(resumed["escalations"], 1);
            assert!(!env.claim_store("dot").exists());
            assert_eq!(
                env.json(&dir, &["prime"])["parked"][0]["id"],
                format!("dots-{}", &id[4..])
            );
            assert_eq!(
                env.json(&dir, &["show", &format!("dots-{}", &escalated[4..])])["escalation"]["level"],
                "high"
            );
        }
    }

    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["park", &id, "a"])
        .assert()
        .success();
    let escalated = id_of(env.json(&dir, &["add", "E", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &dir, "agent-a")
        .args([
            "park",
            &escalated,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["start", &escalated])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["edit", &escalated, "--status", "todo"])
        .assert()
        .success();
    let stopped = env
        .raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "claims")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(!env.claim_store("dot").exists(), "source removed");
    let explained = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explained["parks"], 1);
    assert_eq!(explained["escalations"], 1);
    let resumed = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(resumed["recovery"], "resume_cleanup");
    assert_eq!(resumed["parks"], 1);
    assert_eq!(resumed["escalations"], 1);
}

#[test]
fn rename_taken_live_and_alias_targets_are_config_errors() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.init("taken");
    alias_registry(&env, "retired", "taken");
    for target in ["taken", "retired"] {
        assert_eq!(
            env.fail(&dir, &["rename", "dot", target]),
            "config",
            "target {target}"
        );
        assert!(
            !env.home
                .path()
                .join(".local/state/tasks/rename/dot.toml")
                .exists()
        );
    }
}

fn rename_fixture(
    env: &mut TestEnv,
    count: usize,
    in_git: bool,
) -> (std::path::PathBuf, Vec<String>) {
    let dir = env.init("dot");
    let ids = (0..count)
        .map(|n| id_of(env.json(&dir, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();
    if in_git {
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "seed"]);
    }
    (dir, ids)
}

fn adopt_cmd(env: &TestEnv, dir: &std::path::Path) -> assert_cmd::Command {
    let mut command = env.cmd(dir);
    command
        .env("XDG_CONFIG_HOME", env.home.path().join("config"))
        .env("XDG_STATE_HOME", env.home.path().join("state"));
    command
}

fn adopt_json(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = adopt_cmd(env, dir).args(args).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn adopt_fixture(env: &TestEnv) -> (std::path::PathBuf, String) {
    let dir = env.home.path().join("checkout");
    std::fs::create_dir(&dir).unwrap();
    adopt_json(env, &dir, &["init", "--prefix", "old"]);
    let old_id = id_of(adopt_json(env, &dir, &["add", "Task"]));
    let old_path = dir.join(format!("tasks/{old_id}.md"));
    let new_id = old_id.replacen("old-", "new-", 1);
    let new_path = dir.join(format!("tasks/{new_id}.md"));
    let text = std::fs::read_to_string(&old_path).unwrap();
    std::fs::write(&new_path, text.replacen(&old_id, &new_id, 1)).unwrap();
    std::fs::remove_file(old_path).unwrap();
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(config, text.replace("prefix = \"old\"", "prefix = \"new\"")).unwrap();
    (dir, old_id)
}

fn adopt_old_park(env: &TestEnv, old_id: &str) {
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("old.toml"),
        format!(
            "[parks.\"{old_id}\"]\nowner = \"tester\"\nsession = \"codex:old\"\nhost = \"host\"\nworktree = \"/old\"\nat = \"2026-09-27T00:00:00Z\"\nnext_step = \"resume\"\nwaiting_on = \"agent\"\ntitle = \"Task\"\n"
        ),
    )
    .unwrap();
}

#[test]
fn adopt_registers_synced_prefix_without_touching_checkout() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    let path = dir.join(format!("tasks/{new_id}.md"));
    let before = std::fs::read(&path).unwrap();
    let out = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(out["mode"], "adopt");
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(out["tasks"], 0);
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        adopt_json(&env, &dir, &["show", &old_id])["task"]["id"],
        new_id
    );
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "complete"
    );
}

#[test]
fn adopt_preserves_target_claims_when_carrying_an_old_park() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let state = env.home.path().join("state/tasks/claims");
    adopt_old_park(&env, &old_id);
    let stale_claim = "[claims.\"new-ffffff\"]\nowner = \"tester\"\nsession = \"old\"\nhost = \"host\"\nworktree = \"/old\"\nstarted = \"2020-01-01T00:00:00Z\"\nseen = \"2020-01-01T00:00:00Z\"\n";
    std::fs::write(state.join("new.toml"), stale_claim).unwrap();
    let out = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(out["parks"], 1);
    let target = std::fs::read_to_string(state.join("new.toml")).unwrap();
    assert!(target.contains("[claims.new-ffffff]"), "{target}");
    assert!(
        target.contains(&format!("[parks.{}]", old_id.replacen("old-", "new-", 1))),
        "{target}"
    );
}

#[test]
fn adopt_retry_after_local_start_consumes_the_only_park() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_park(&env, &old_id);
    let stopped = adopt_cmd(&env, &dir)
        .env("TASKS_RENAME_STOP_AFTER", "store")
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let started = adopt_cmd(&env, &dir)
        .env("CODEX_SESSION_ID", "test-thread")
        .args(["start", &new_id])
        .output()
        .unwrap();
    assert!(started.status.success(), "{started:?}");
    let claimed = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&claimed), "claimed");
    let target_path = env.home.path().join("state/tasks/claims/new.toml");
    let text = std::fs::read_to_string(&target_path).unwrap();
    std::fs::write(
        &target_path,
        text.replace("seen = \"2026-", "seen = \"2020-"),
    )
    .unwrap();
    let resumed = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(resumed["recovery"], "resume_registry");
    assert_eq!(resumed["parks"], 0);
    let target = std::fs::read_to_string(target_path).unwrap();
    assert!(target.contains(&format!("[claims.{new_id}]")), "{target}");
    assert!(!target.contains(&format!("[parks.{new_id}]")), "{target}");
}

#[test]
fn adopt_same_prefix_refuses_without_waiting_on_the_lock_twice() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let mut child = env
        .raw(&dir)
        .env("XDG_CONFIG_HOME", env.home.path().join("config"))
        .env("XDG_STATE_HOME", env.home.path().join("state"))
        .args(["rename", "new", "new", "--adopt"])
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    if status.is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
    }
    assert_eq!(status.unwrap().code(), Some(1));
}

#[test]
fn adopt_explain_refuses_foreign_old_project_without_writes() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let foreign = env.home.path().join("foreign");
    std::fs::create_dir_all(foreign.join("tasks")).unwrap();
    std::fs::write(foreign.join("tasks/.config.toml"), "prefix = \"old\"\n").unwrap();
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["old"] = toml::Value::String(foreign.display().to_string());
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let before = std::fs::read(&registry_path).unwrap();
    let explained = adopt_json(
        &env,
        &dir,
        &["rename", "old", "new", "--adopt", "--explain"],
    );
    assert_eq!(explained["recovery"], "refuse");
    assert_eq!(std::fs::read(registry_path).unwrap(), before);
    assert!(!env.home.path().join("state/tasks/claims/new.toml").exists());
    std::fs::remove_file(foreign.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
}

#[test]
fn adopt_explain_refuses_target_park_or_older_alias_before_registry_switch() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_json(&env, &dir, &["park", &new_id, "new host work"]);
    let before = std::fs::read(env.home.path().join("state/tasks/claims/new.toml")).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    assert_eq!(
        std::fs::read(env.home.path().join("state/tasks/claims/new.toml")).unwrap(),
        before
    );

    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"] = toml::toml! { older = "old" }.into();
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "older", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
}

#[test]
fn adopt_retired_target_refuses_before_writing_target_store() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    adopt_old_park(&env, &old_id);
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"] = toml::toml! { new = "old" }.into();
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let registry_before = std::fs::read(&registry_path).unwrap();
    let old_path = env.home.path().join("state/tasks/claims/old.toml");
    let old_before = std::fs::read(&old_path).unwrap();
    let target_path = env.home.path().join("state/tasks/claims/new.toml");
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    let refused = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(!target_path.exists());
    assert_eq!(std::fs::read(old_path).unwrap(), old_before);
    assert_eq!(std::fs::read(registry_path).unwrap(), registry_before);
}

#[test]
fn adopt_refuses_a_live_old_claim_with_missing_registered_root() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("old.toml"),
        format!("[claims.\"{old_id}\"]\nowner = \"tester\"\nsession = \"old-session\"\nhost = \"host\"\nworktree = \"/old\"\nstarted = \"2099-01-01T00:00:00Z\"\nseen = \"2099-01-01T00:00:00Z\"\n"),
    )
    .unwrap();
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["old"] =
        toml::Value::String(env.home.path().join("missing").display().to_string());
    std::fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    assert!(String::from_utf8_lossy(&out.stderr).contains("old-session"));
    let claims = adopt_json(&env, &dir, &["claims"]);
    assert_eq!(claims["claims"][0]["id"], old_id);
    let old_store = state.join("old.toml");
    let text = std::fs::read_to_string(&old_store).unwrap();
    std::fs::write(&old_store, text.replace("2099-", "2020-")).unwrap();
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
    assert!(!old_store.exists());
}

#[test]
fn adopt_explain_refuses_pending_inventory_or_mismatched_task_file() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let inventory_dir = env.home.path().join("state/tasks/rename");
    std::fs::create_dir_all(&inventory_dir).unwrap();
    std::fs::write(
        inventory_dir.join("old.toml"),
        format!("source = \"old\"\ntarget = \"new\"\nroot = \"{}\"\nconfig_from = \"{}\"\nconfig_to = \"{}\"\nentries = []\n", dir.display(), "0".repeat(64), "1".repeat(64)),
    )
    .unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    std::fs::remove_file(inventory_dir.join("old.toml")).unwrap();
    let new_id = old_id.replacen("old-", "new-", 1);
    let path = dir.join(format!("tasks/{new_id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen(&new_id, &old_id, 1)).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
}

#[test]
fn adopt_two_hosts_after_primary_rename_and_optional_partial_init() {
    for partial_init in [false, true] {
        let host_a = TestEnv::new();
        let host_b = TestEnv::new();
        let parent = tempfile::tempdir().unwrap();
        let old_root = parent.path().join("old-root");
        std::fs::create_dir(&old_root).unwrap();
        adopt_json(&host_a, &old_root, &["init", "--prefix", "old"]);
        let old_id = id_of(adopt_json(
            &host_a,
            &old_root,
            &["add", "T", "--complexity", "mid"],
        ));
        adopt_json(&host_b, &old_root, &["init", "--prefix", "old"]);
        adopt_json(&host_b, &old_root, &["park", &old_id, "continue here"]);
        let b_store = host_b.home.path().join("state/tasks/claims/old.toml");
        let mut text = std::fs::read_to_string(&b_store).unwrap();
        text.push_str(&format!(
            "\n[escalations.\"{old_id}\"]\nlevel = \"high\"\nat = \"2026-09-27T00:00:00Z\"\nsession = \"old-session\"\n"
        ));
        std::fs::write(&b_store, text).unwrap();

        adopt_json(&host_a, &old_root, &["rename", "old", "new"]);
        let new_root = parent.path().join("new-root");
        std::fs::rename(&old_root, &new_root).unwrap();
        adopt_json(&host_a, &new_root, &["init", "--prefix", "new", "--force"]);
        if partial_init {
            adopt_json(&host_b, &new_root, &["init", "--prefix", "new", "--force"]);
        }
        let new_id = old_id.replacen("old-", "new-", 1);
        let config = new_root.join("tasks/.config.toml");
        let task = new_root.join(format!("tasks/{new_id}.md"));
        let config_before = std::fs::read(&config).unwrap();
        let task_before = std::fs::read(&task).unwrap();
        let adopted = adopt_json(&host_b, &new_root, &["rename", "old", "new", "--adopt"]);
        assert_eq!(adopted["mode"], "adopt");
        assert_eq!(std::fs::read(config).unwrap(), config_before);
        assert_eq!(std::fs::read(task).unwrap(), task_before);
        assert_eq!(
            adopt_json(&host_b, &new_root, &["show", &old_id])["task"]["id"],
            new_id
        );
        let registry: toml::Value = toml::from_str(
            &std::fs::read_to_string(host_b.home.path().join("config/tasks/projects.toml"))
                .unwrap(),
        )
        .unwrap();
        assert!(
            registry["projects"]
                .as_table()
                .unwrap()
                .get("old")
                .is_none()
        );
        assert_eq!(registry["aliases"]["old"].as_str(), Some("new"));
        assert_eq!(registry["projects"]["new"].as_str(), new_root.to_str());
        let b_new_store =
            std::fs::read_to_string(host_b.home.path().join("state/tasks/claims/new.toml"))
                .unwrap();
        assert!(
            b_new_store.contains(&format!("[parks.{new_id}]")),
            "{b_new_store}"
        );
        assert!(
            b_new_store.contains(&format!("[escalations.{new_id}]")),
            "{b_new_store}"
        );
        assert!(!b_store.exists());
    }
}

#[test]
fn adopt_replays_each_state_boundary_and_skips_store_without_parks() {
    for (boundary, expected) in [
        ("store", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("claims", "complete"),
    ] {
        let env = TestEnv::new();
        let (dir, old_id) = adopt_fixture(&env);
        adopt_old_park(&env, &old_id);
        let stopped = adopt_cmd(&env, &dir)
            .env("TASKS_RENAME_STOP_AFTER", boundary)
            .args(["rename", "old", "new", "--adopt"])
            .output()
            .unwrap();
        assert!(stopped.status.success(), "{boundary}: {stopped:?}");
        let explained = adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"],
        );
        assert_eq!(explained["recovery"], expected, "{boundary}: {explained}");
        if boundary == "registry" {
            let new_id = old_id.replacen("old-", "new-", 1);
            let started = adopt_cmd(&env, &dir)
                .env("CODEX_SESSION_ID", "post-registry")
                .args(["start", &new_id])
                .output()
                .unwrap();
            assert!(started.status.success(), "{started:?}");
        }
        assert_eq!(
            adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
            expected
        );
        assert_eq!(
            adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
            "complete"
        );
    }
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let stopped = adopt_cmd(&env, &dir)
        .env("TASKS_RENAME_STOP_AFTER", "store")
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "complete"
    );
}

#[test]
fn adopt_carries_an_orphaned_park_and_reports_it() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    adopt_old_park(&env, "old-ffffff");
    let adopted = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert!(adopted["warnings"].to_string().contains("new-ffffff"));
    let parked = adopt_json(&env, &dir, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["id"], "new-ffffff");
    assert!(parked["tasks"][0]["status"].is_null());
}

fn rename_stop(env: &TestEnv, dir: &std::path::Path, old: &str, new: &str, stop: &str) {
    let output = env
        .raw(dir)
        .env("TASKS_RENAME_STOP_AFTER", stop)
        .args(["rename", old, new])
        .output()
        .unwrap();
    assert!(output.status.success(), "{stop}: {output:?}");
}

// Include directories as well as bytes: diagnosis must not create even an empty state dir.
fn rename_disk_state(
    env: &TestEnv,
    dir: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn collect(
        path: &std::path::Path,
        state: &mut std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>>,
    ) {
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
    collect(&dir.join("tasks"), &mut state);
    collect(env.home.path(), &mut state);
    state
}

#[cfg(unix)]
fn set_registry_root(env: &TestEnv, prefix: &str, root: &std::path::Path) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    registry["projects"].as_table_mut().unwrap().insert(
        prefix.into(),
        toml::Value::String(root.display().to_string()),
    );
    std::fs::write(path, toml::to_string(&registry).unwrap()).unwrap();
}

#[cfg(unix)]
/// A claim on `old_id` in `old`'s store under the adopt fixture's state directory. With
/// `view`, it carries this test process's live pid recorded in that view; without, it is
/// a pid-less claim seen in 2099, which reads live by its TTL.
fn adopt_old_claim(env: &TestEnv, old_id: &str, session: &str, view: Option<&str>) {
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    let process = match view {
        Some(view) => {
            let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
            let rest = stat.rsplit_once(") ").unwrap().1.to_string();
            let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
            let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
            format!(
                "pid = {}\npid_start = {start}\nboot_id = \"{}\"\npid_view = \"{view}\"\n",
                std::process::id(),
                boot.trim()
            )
        }
        None => String::new(),
    };
    std::fs::write(
        state.join("old.toml"),
        format!(
            "[claims.\"{old_id}\"]\nowner = \"tester\"\nsession = \"{session}\"\nhost = \"host\"\n\
             worktree = \"/old\"\nstarted = \"2099-01-01T00:00:00Z\"\n\
             seen = \"2099-01-01T00:00:00Z\"\n{process}"
        ),
    )
    .unwrap();
}

#[test]
fn release_adopting_follows_the_adoption_hint_end_to_end() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));

    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    let detail = err_detail(&out);
    let hint = format!("tasks release {new_id} \"<why>\" --adopting old");
    assert!(detail.contains(&hint), "{detail}");

    // Run what the hint prints, with a reason in place of the placeholder.
    let out = adopt_cmd(&env, &dir)
        .env("TASKS_SESSION", "releaser")
        .args(["release", &new_id, "sandbox exited", "--adopting", "old"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let shown = adopt_json(&env, &dir, &["show", &new_id]);
    assert!(
        shown.to_string().contains(
            "released session sandboxed (owner tester, unverifiable: recorded in another PID \
             view); reason: sandbox exited"
        ),
        "{shown}"
    );

    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
}

#[test]
fn release_adopting_refuses_own_and_live_claims_without_ordinary_remedies() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    let record = std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap();
    let old_store = env.home.path().join("state/tasks/claims/old.toml");

    for (session, kind, expected) in [
        (
            "me",
            "validation",
            "is this session's, recorded before the rename",
        ),
        (
            "someone-else",
            "claimed",
            "was recorded before the rename. Once session someone-else ends",
        ),
    ] {
        adopt_old_claim(&env, &old_id, session, None);
        let before = std::fs::read(&old_store).unwrap();
        let out = adopt_cmd(&env, &dir)
            .env("TASKS_SESSION", "me")
            .args(["release", &new_id, "x", "--adopting", "old"])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), kind, "{session}");
        let detail = err_detail(&out);
        assert!(detail.contains(expected), "{detail}");
        assert!(detail.contains("tasks rename old new --adopt"), "{detail}");
        assert!(detail.contains("if it reads unverifiable"), "{detail}");
        assert!(!detail.contains("it clears when"), "{detail}");
        for remedy in ["tasks park", "tasks done", "start --force"] {
            assert!(!detail.contains(remedy), "{remedy} in {detail}");
        }
        assert_eq!(std::fs::read(&old_store).unwrap(), before);
        assert_eq!(
            std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap(),
            record
        );
    }
}

#[test]
fn release_adopting_refusals_write_nothing() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));
    let record = std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap();

    let claims = env.home.path().join("state/tasks/claims");
    let locks = |dir: &std::path::Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".lock"))
            .collect();
        names.sort();
        names
    };
    let before = locks(&claims);
    for old in ["new", "nope", "../claims/new", "Old", ""] {
        let out = adopt_cmd(&env, &dir)
            .env("TASKS_SESSION", "releaser")
            .args(["release", &new_id, "x", "--adopting", old])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "validation", "--adopting {old:?}");
        assert!(
            err_detail(&out).contains(&format!("--adopting {old}")),
            "{out:?}"
        );
    }
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap(),
        record
    );
    assert_eq!(locks(&claims), before);

    // After adoption completes, the old prefix is no longer adoptable here.
    std::fs::remove_file(env.home.path().join("state/tasks/claims/old.toml")).unwrap();
    adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    let out = adopt_cmd(&env, &dir)
        .env("TASKS_SESSION", "releaser")
        .args(["release", &new_id, "x", "--adopting", "old"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
}

#[test]
fn release_adopting_takes_an_own_viewless_claim_after_its_session_ended() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();
    // A pid with no boot and no view, as an older binary or an unproven export records it:
    // it reads unverifiable in every view, even with its process gone.
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("old.toml"),
        format!(
            "[claims.\"{old_id}\"]\nowner = \"tester\"\nsession = \"me\"\nhost = \"host\"\n\
             worktree = \"/old\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
             seen = \"2026-01-01T00:00:00Z\"\npid = {pid}\npid_start = 1\n"
        ),
    )
    .unwrap();
    let claims = adopt_json(&env, &dir, &["claims"]);
    assert_eq!(
        claims["claims"][0]["unverified"],
        "PID view not established when recorded"
    );

    let out = adopt_cmd(&env, &dir)
        .env("TASKS_SESSION", "later")
        .args([
            "release",
            &new_id,
            "my earlier session ended",
            "--adopting",
            "old",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
}

#[test]
fn release_adopting_rechecks_eligibility_once_both_locks_are_held() {
    use std::os::unix::fs::MetadataExt;
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));
    let other = env.home.path().join("other");
    std::fs::create_dir(&other).unwrap();
    adopt_json(&env, &other, &["init", "--prefix", "other"]);
    let old_store = env.home.path().join("state/tasks/claims/old.toml");
    let store = std::fs::read(&old_store).unwrap();
    let record = std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap();

    // Hold `old`'s lock, as `init --force` does while it repoints the prefix. "new" sorts
    // first, so release takes it, passes its first check, and blocks here.
    let lock_path = env.home.path().join("state/tasks/claims/old.lock");
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();
    let ino = std::fs::metadata(&lock_path).unwrap().ino();
    let child = env
        .raw(&dir)
        .env("XDG_CONFIG_HOME", env.home.path().join("config"))
        .env("XDG_STATE_HOME", env.home.path().join("state"))
        .env("TASKS_SESSION", "releaser")
        .args(["release", &new_id, "gone", "--adopting", "old"])
        .spawn()
        .unwrap();
    // /proc/locks lists a blocked flock waiter as a `->` line naming the inode.
    let blocked = || {
        std::fs::read_to_string("/proc/locks")
            .unwrap()
            .lines()
            .any(|line| {
                line.contains("->")
                    && line
                        .split_whitespace()
                        .any(|field| field.ends_with(&format!(":{ino}")))
            })
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !blocked() {
        assert!(
            std::time::Instant::now() < deadline,
            "release never blocked on old.lock"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // Repoint `old` at another project while release waits, then let it through.
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["old"] = toml::Value::String(other.display().to_string());
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    drop(held);

    let out = child.wait_with_output().unwrap();
    assert_eq!(err_kind(&out), "validation", "{out:?}");
    assert!(
        err_detail(&out).contains("still holds a tasks project"),
        "{out:?}"
    );
    assert_eq!(std::fs::read(&old_store).unwrap(), store);
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{new_id}.md"))).unwrap(),
        record
    );
}

#[test]
fn adopt_hints_plain_release_for_an_unverifiable_claim_in_the_new_store() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_claim(&env, &old_id, "sandboxed", Some("0:1"));
    let state = env.home.path().join("state/tasks/claims");
    std::fs::rename(state.join("old.toml"), state.join("new.toml")).unwrap();
    let text = std::fs::read_to_string(state.join("new.toml")).unwrap();
    std::fs::write(state.join("new.toml"), text.replace(&old_id, &new_id)).unwrap();
    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    let detail = err_detail(&out);
    assert!(
        detail.contains(&format!("tasks release {new_id} \"<why>\"`")),
        "{detail}"
    );
    assert!(!detail.contains("--adopting"), "{detail}");
}

#[test]
fn rename_recovers_through_noncanonical_registry_roots_before_and_after_registry_write() {
    for spelling in ["symlink", "dotdot"] {
        for (stop, verdict) in [("files", "resume_files"), ("registry", "resume_cleanup")] {
            let env = TestEnv::new();
            let outer = tempfile::tempdir().unwrap();
            let real = outer.path().join("real");
            std::fs::create_dir(&real).unwrap();
            env.json(&real, &["init", "--prefix", "dot"]);
            let id = id_of(env.json(&real, &["add", "One", "-p", "2"]));
            let root = if spelling == "symlink" {
                let link = outer.path().join("link");
                std::os::unix::fs::symlink(&real, &link).unwrap();
                link
            } else {
                real.join("..").join("real")
            };
            set_registry_root(&env, "dot", &root);
            let label = format!("{spelling} stop={stop}");

            assert_eq!(
                env.json(&real, &["rename", "dot", "dots", "--explain"])["recovery"],
                "fresh",
                "{label}"
            );
            rename_stop(&env, &real, "dot", "dots", stop);
            let moved = id.replacen("dot-", "dots-", 1);
            assert!(!real.join(format!("tasks/{id}.md")).exists(), "{label}");
            assert!(real.join(format!("tasks/{moved}.md")).is_file(), "{label}");
            let before = rename_disk_state(&env, &real);
            assert_eq!(
                env.json(&real, &["rename", "dot", "dots", "--explain"])["recovery"],
                verdict,
                "{label}"
            );
            assert_eq!(rename_disk_state(&env, &real), before, "{label}");
            assert_eq!(
                env.json(&real, &["rename", "dot", "dots"])["recovery"],
                verdict,
                "{label}"
            );
            assert_eq!(env.json(&real, &["show", &id])["task"]["id"], moved);
            assert_eq!(env.json(&real, &["show", &moved])["task"]["id"], moved);
            assert!(
                !env.home
                    .path()
                    .join(".local/state/tasks/rename/dot.toml")
                    .exists(),
                "{label}"
            );
            let registry: toml::Value = toml::from_str(
                &std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml"))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(registry["projects"]["dots"].as_str(), root.to_str());
            assert_eq!(registry["aliases"]["dot"].as_str(), Some("dots"));
        }
    }
}

#[cfg(unix)]
#[test]
fn rename_older_alias_replays_and_freezes_by_canonical_root() {
    let env = TestEnv::new();
    let outer = tempfile::tempdir().unwrap();
    let real = outer.path().join("real");
    let link = outer.path().join("link");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    env.json(&real, &["init", "--prefix", "dot"]);
    let id = id_of(env.json(&real, &["add", "One", "-p", "2"]));
    env.json(&real, &["rename", "dot", "dots"]);
    set_registry_root(&env, "dots", &link);
    rename_stop(&env, &real, "dot", "config", "registry");
    let moved = id.replacen("dot-", "config-", 1);
    assert!(
        !real
            .join("tasks")
            .join(id.replacen("dot-", "dots-", 1))
            .with_extension("md")
            .exists()
    );
    assert!(
        real.join("tasks")
            .join(&moved)
            .with_extension("md")
            .is_file()
    );

    assert_eq!(
        env.json(&real, &["rename", "dot", "config", "--explain"])["recovery"],
        "resume_cleanup"
    );
    let frozen = env.raw(&real).args(["unregister", "dot"]).output().unwrap();
    assert_eq!(frozen.status.code(), Some(1), "{frozen:?}");
    assert!(
        String::from_utf8_lossy(&frozen.stderr).contains("unfinished"),
        "{frozen:?}"
    );
    assert_eq!(
        env.json(&real, &["rename", "dot", "config"])["recovery"],
        "resume_cleanup"
    );
    assert_eq!(env.json(&real, &["show", &id])["task"]["id"], moved);
    assert_eq!(
        env.json(&real, &["show", &id.replacen("dot-", "dots-", 1)])["task"]["id"],
        moved
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/dots.toml")
            .exists()
    );
    let registry: toml::Value = toml::from_str(
        &std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(registry["projects"]["config"].as_str(), link.to_str());
    assert_eq!(registry["aliases"]["dot"].as_str(), Some("config"));
    assert_eq!(registry["aliases"]["dots"].as_str(), Some("config"));
}

#[test]
fn rename_explains_a_missing_foreign_registry_root_as_r8() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, false);
    rename_stop(&env, &dir, "dot", "dots", "config");
    let missing = dir.with_file_name("missing-rename-root");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let projects = registry["projects"].as_table_mut().unwrap();
    projects.remove("dot");
    projects.insert(
        "dots".into(),
        toml::Value::String(missing.display().to_string()),
    );
    std::fs::write(path, toml::to_string(&registry).unwrap()).unwrap();

    let before = rename_disk_state(&env, &dir);
    let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explain["recovery"], "refuse");
    let warning = explain["warnings"][0].as_str().unwrap();
    assert!(warning.starts_with("R8:"), "{warning}");
    assert!(warning.contains(missing.to_str().unwrap()), "{warning}");
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "config");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

#[test]
fn rename_every_mutation_boundary_resumes_inside_and_outside_git_including_empty_projects() {
    for in_git in [true, false] {
        for count in [0, 3] {
            for (stop, want) in [
                ("inventory", "resume_files"),
                ("file:0", "resume_files"),
                ("file:1", "resume_files"),
                ("file:2", "resume_files"),
                ("files", "resume_files"),
                ("config", "resume_registry"),
                ("registry", "resume_cleanup"),
                ("claims", "resume_cleanup"),
            ] {
                if count == 0 && stop.starts_with("file:") {
                    continue;
                }
                let mut env = TestEnv::new();
                let (dir, ids) = rename_fixture(&mut env, count, in_git);
                let inventory = env.home.path().join(".local/state/tasks/rename/dot.toml");
                let claims = env.claim_store("dot");
                std::fs::create_dir_all(claims.parent().unwrap()).unwrap();
                std::fs::write(&claims, "[claims]\n").unwrap();
                let label = format!("git={in_git} count={count} stop={stop}");
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    "fresh",
                    "{label}"
                );
                assert!(!inventory.exists());
                rename_stop(&env, &dir, "dot", "dots", stop);
                assert!(
                    inventory.is_file(),
                    "hook must leave a pending inventory: {label}"
                );
                let named = |prefix: &str| {
                    std::fs::read_dir(dir.join("tasks"))
                        .unwrap()
                        .map(|entry| entry.unwrap())
                        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
                        .count()
                };
                let (sources, destinations) = match stop {
                    "inventory" => (count, 0),
                    "file:0" => (3, 1),
                    "file:1" => (2, 2),
                    "file:2" => (1, 3),
                    _ => (0, count),
                };
                assert_eq!(
                    (named("dot-"), named("dots-")),
                    (sources, destinations),
                    "{label}"
                );
                let config: toml::Value =
                    toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
                assert_eq!(
                    config["prefix"].as_str().unwrap(),
                    if matches!(stop, "config" | "registry" | "claims") {
                        "dots"
                    } else {
                        "dot"
                    },
                    "{label}"
                );
                let registry: toml::Value = toml::from_str(
                    &env.read(&env.home.path().join(".config"), "tasks/projects.toml"),
                )
                .unwrap();
                if matches!(stop, "registry" | "claims") {
                    assert_eq!(
                        registry["projects"]["dots"].as_str(),
                        dir.to_str(),
                        "{label}"
                    );
                    assert_eq!(registry["aliases"]["dot"].as_str(), Some("dots"), "{label}");
                } else {
                    assert_eq!(
                        registry["projects"]["dot"].as_str(),
                        dir.to_str(),
                        "{label}"
                    );
                    assert!(registry["projects"].get("dots").is_none(), "{label}");
                }
                assert_eq!(claims.exists(), stop != "claims", "{label}");
                let before = rename_disk_state(&env, &dir);
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    want,
                    "{label}"
                );
                assert_eq!(
                    rename_disk_state(&env, &dir),
                    before,
                    "explain changed disk: {label}"
                );
                let resumed = env.json(&dir, &["rename", "dot", "dots"]);
                assert_eq!(resumed["recovery"], want, "{label}");
                if !in_git {
                    assert!(
                        resumed["warnings"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|w| w.as_str().unwrap().contains("outside git")),
                        "{label}"
                    );
                }
                assert_eq!((named("dot-"), named("dots-")), (0, count), "{label}");
                for id in ids {
                    let moved = id.replacen("dot-", "dots-", 1);
                    assert_eq!(
                        env.json(&dir, &["show", &id])["task"]["id"],
                        moved,
                        "{label}"
                    );
                    assert_eq!(
                        env.json(&dir, &["show", &moved])["task"]["id"],
                        moved,
                        "{label}"
                    );
                }
                assert!(!inventory.exists(), "{label}");
                assert!(!claims.exists(), "{label}");
                assert!(
                    claims.with_extension("lock").is_file(),
                    "old lock must survive: {label}"
                );
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    "complete",
                    "{label}"
                );
                assert!(
                    env.check(&dir)["errors"].as_array().unwrap().is_empty(),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn rename_each_recovery_refusal_fires_independently_and_preserves_disk() {
    for rule in 1..=8 {
        let mut env = TestEnv::new();
        let (dir, ids) = rename_fixture(&mut env, 1, true);
        let foreign = env.init("foreign");
        rename_stop(&env, &dir, "dot", "dots", "file:0");
        let source = dir.join(format!("tasks/{}.md", ids[0]));
        let dest = dir.join(format!("tasks/{}.md", ids[0].replacen("dot-", "dots-", 1)));
        let registry_path = env.home.path().join(".config/tasks/projects.toml");
        let mut target = "dots";
        match rule {
            1 => target = "other",
            2 => std::fs::write(
                &source,
                std::fs::read_to_string(&source)
                    .unwrap()
                    .replace("title: T0", "title: Edited"),
            )
            .unwrap(),
            3 => std::fs::write(
                &dest,
                std::fs::read_to_string(&dest)
                    .unwrap()
                    .replace("title: T0", "title: Conflict"),
            )
            .unwrap(),
            4 => {
                std::fs::remove_file(&source).unwrap();
                std::fs::remove_file(&dest).unwrap();
            }
            5 => std::fs::write(dir.join("tasks/dot-ffffff.md"), "unrecorded task").unwrap(),
            6 => {
                let config = dir.join("tasks/.config.toml");
                std::fs::write(
                    &config,
                    format!(
                        "{}\n# unrelated edit\n",
                        std::fs::read_to_string(&config).unwrap()
                    ),
                )
                .unwrap();
            }
            7 | 8 => {
                let mut registry: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
                let projects = registry["projects"].as_table_mut().unwrap();
                projects.remove("foreign");
                projects.insert(
                    "dots".into(),
                    toml::Value::String(foreign.display().to_string()),
                );
                if rule == 8 {
                    projects.remove("dot");
                }
                std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        // R1's alternate target lock exists before taking the byte-for-byte snapshot.
        let held = hold_project_lock(&env, target);
        drop(held);
        let before = rename_disk_state(&env, &dir);
        let explain = env.json(&dir, &["rename", "dot", target, "--explain"]);
        assert_eq!(explain["recovery"], "refuse", "R{rule}: {explain}");
        let reason = explain["warnings"][0].as_str().unwrap();
        assert!(reason.starts_with(&format!("R{rule}:")), "{reason}");
        if rule == 3 {
            assert!(
                reason.contains(source.to_str().unwrap()),
                "both paths required: {reason}"
            );
            assert!(
                reason.contains(dest.to_str().unwrap()),
                "both paths required: {reason}"
            );
        }
        assert_eq!(
            rename_disk_state(&env, &dir),
            before,
            "R{rule}: explain changed disk"
        );
        let out = env
            .raw(&dir)
            .args(["rename", "dot", target])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "R{rule}: {out:?}");
        assert_eq!(
            err_kind(&out),
            if rule >= 7 { "config" } else { "validation" }
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(&format!("R{rule}:")),
            "{out:?}"
        );
        assert_eq!(
            rename_disk_state(&env, &dir),
            before,
            "R{rule}: refusal changed disk"
        );
    }
}

#[test]
fn rename_same_name_refusal_names_the_obstacle() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, true);
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dot"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("source and target prefixes are both"),
        "{detail}"
    );
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_resumes_require_claim_and_worktree_authorization_at_every_stage() {
    for (stop, verdict) in [
        ("inventory", "resume_files"),
        ("files", "resume_files"),
        ("config", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("claims", "resume_cleanup"),
    ] {
        for obstacle in ["old claim", "new claim", "worktree"] {
            let mut env = TestEnv::new();
            let (dir, ids) = rename_fixture(&mut env, 1, true);
            rename_stop(&env, &dir, "dot", "dots", stop);
            let other = tempfile::tempdir().unwrap();
            let claim_prefix = if obstacle == "old claim" {
                "dot"
            } else {
                "dots"
            };
            if obstacle == "worktree" {
                git(
                    &dir,
                    &[
                        "worktree",
                        "add",
                        "-q",
                        "--detach",
                        other.path().to_str().unwrap(),
                    ],
                );
            } else {
                let id = ids[0].replacen("dot-", &format!("{claim_prefix}-"), 1);
                write_claim(&env, claim_prefix, &id, "agent-a", true);
            }
            let before = rename_disk_state(&env, &dir);
            // Authorization is separate from classification, including after P5/P6.
            assert_eq!(
                env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                verdict,
                "{stop}: {obstacle}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before);
            assert_eq!(
                env.fail(&dir, &["rename", "dot", "dots"]),
                if obstacle == "worktree" {
                    "validation"
                } else {
                    "claimed"
                },
                "{stop}: {obstacle}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {obstacle}");
            if obstacle == "worktree" {
                git(
                    &dir,
                    &["worktree", "remove", other.path().to_str().unwrap()],
                );
            } else {
                assert!(
                    env.claim_store(claim_prefix).is_file(),
                    "live store must survive refused cleanup"
                );
                std::fs::remove_file(env.claim_store(claim_prefix)).unwrap();
            }
            assert_eq!(
                env.json(&dir, &["rename", "dot", "dots"])["recovery"],
                verdict
            );
            assert!(
                !env.home
                    .path()
                    .join(".local/state/tasks/rename/dot.toml")
                    .exists()
            );
        }
    }
}

#[test]
fn rename_pending_explain_does_not_acquire_any_existing_lock() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, true);
    rename_stop(&env, &dir, "dot", "dots", "registry");
    write_claim(
        &env,
        "dots",
        &ids[0].replacen("dot-", "dots-", 1),
        "agent-a",
        true,
    );
    let old_lock = hold_project_lock(&env, "dot");
    let new_lock = hold_project_lock(&env, "dots");
    let registry_lock = File::options()
        .write(true)
        .open(env.home.path().join(".config/tasks/projects.lock"))
        .unwrap();
    registry_lock.lock().unwrap();
    let before = rename_disk_state(&env, &dir);
    let child = env
        .raw(&dir)
        .args(["rename", "dot", "dots", "--explain"])
        .spawn()
        .unwrap();
    let out = reap(child, Duration::from_secs(3));
    drop((old_lock, new_lock, registry_lock));
    let out = out.expect("explain waited on a lock");
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["recovery"], "resume_cleanup");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

#[test]
fn rename_freeze_covers_all_writers_feedback_and_registry_names() {
    for stop in ["inventory", "config", "registry", "claims"] {
        let (env, dir, reporter) = feedback_env();
        let id = id_of(env.json(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "Original",
                "--category",
                "gap",
                "--new",
            ],
        ));
        let editor_ran = dir.join("editor-ran");
        let editor = editor_script(&dir, &format!("touch '{}'; exit 99", editor_ran.display()));
        rename_stop(&env, &dir, "tasks", "tracker", stop);
        let current = if stop == "inventory" {
            id.clone()
        } else {
            id.replacen("tasks-", "tracker-", 1)
        };
        let before = rename_disk_state(&env, &dir);
        let commands: &[&[&str]] = &[
            &["add", "Late", "-p", "2"],
            &["add", "Late", "--source", "same-source"],
            &["start", &current],
            &["done", &current],
            &["drop", &current, "Reason"],
            &["block", &current, "Reason"],
            &["note", &current, "Late note"],
            &["edit", &current, "--title", "Late title"],
            &["edit", &current, "--editor"],
            &["dep", &current, "--on", &current],
        ];
        for args in commands {
            let out = env
                .raw(&dir)
                .env("EDITOR", &editor)
                .args(*args)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
            assert_eq!(err_kind(&out), "validation", "{stop}: {args:?}: {out:?}");
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                "{stop}: {args:?}: {out:?}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
        }
        assert!(
            !editor_ran.exists(),
            "a frozen edit must not launch the editor"
        );
        for args in [
            vec![
                "feedback",
                "--project",
                "tasks",
                "Late",
                "--category",
                "gap",
                "--new",
            ],
            vec![
                "feedback",
                "--project",
                "tasks",
                "Late",
                "--category",
                "gap",
                "--recur",
                &current,
            ],
        ] {
            let out = env.raw(&reporter).args(&args).output().unwrap();
            assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
            assert_eq!(
                err_kind(&out),
                if stop == "config" {
                    "config"
                } else {
                    "validation"
                }
            );
            if stop != "config" {
                assert!(
                    String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                    "{out:?}"
                );
            }
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
        }
        let unrelated = tempfile::tempdir().unwrap();
        for prefix in ["tasks", "tracker"] {
            for (root, args) in [
                (dir.as_path(), vec!["unregister", prefix]),
                (dir.as_path(), vec!["init", "--prefix", prefix, "--force"]),
                (
                    unrelated.path(),
                    vec!["init", "--prefix", prefix, "--force"],
                ),
            ] {
                let out = env.raw(root).args(&args).output().unwrap();
                assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
                assert!(
                    String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                    "{stop}: {args:?}: {out:?}"
                );
                assert!(!unrelated.path().join("tasks").exists());
                assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
            }
        }
        assert_eq!(env.json(&dir, &["show", &current])["task"]["id"], current);
        assert_eq!(
            env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
            1
        );
    }
}

#[test]
fn rename_freeze_is_rechecked_when_an_editor_returns() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, false);
    let original = env.read(&dir, &format!("tasks/{}.md", ids[0]));
    let editor = editor_script(
        &dir,
        &format!(
            "sed -i 's/^title: T0$/title: Edited/' \"$1\"\nTASKS_RENAME_STOP_AFTER=inventory '{}' rename dot dots > /dev/null",
            assert_cmd::cargo::cargo_bin("tasks").display()
        ),
    );
    let out = env
        .raw(&dir)
        .env("EDITOR", editor)
        .args(["edit", &ids[0], "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(err_kind(&out), "validation");
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("unfinished") && detail.contains(".edit.md"),
        "{detail}"
    );
    assert_eq!(env.read(&dir, &format!("tasks/{}.md", ids[0])), original);
    assert!(std::fs::read_dir(dir.join("tasks")).unwrap().any(|entry| {
        let path = entry.unwrap().path();
        path.to_string_lossy().ends_with(".edit.md")
            && std::fs::read_to_string(path)
                .unwrap()
                .contains("title: Edited")
    }));
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots"])["recovery"],
        "resume_files"
    );
}

#[test]
fn rename_rewrites_own_refs_and_preserves_hand_written_body_notes_and_foreign_bytes() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let foreign = env.init("ops");
    let foreign_id = id_of(env.json(&foreign, &["add", "Foreign"]));
    let parent = "---\nid: dot-a00001\ntitle: Parent\nstatus: todo\npriority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\ndepends: []\ntags: []\n---\n\nParent body.\n";
    let child = format!(
        "---\nid: dot-a00002\ntitle: Child\nstatus: todo\npriority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\nparent: dot-a00001\ndepends: [dot-a00001, {foreign_id}]\ntags: [dot-a00001]\nsource: dot-a00001\n---\n\n\nBody   with  odd    spacing mentions dot-a00001.\n\n\n## Notes\n\n- 2026-09-01T00:00:00Z (keith):   dot-a00001 stays in prose\n\n"
    );
    std::fs::write(dir.join("tasks/dot-a00001.md"), parent).unwrap();
    std::fs::write(dir.join("tasks/dot-a00002.md"), &child).unwrap();
    let inbound = id_of(env.json(&foreign, &["add", "Inbound", "--depends", "dot-a00002"]));
    let foreign_before = env.read(&foreign, &format!("tasks/{inbound}.md"));
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(
        env.read(&dir, "tasks/dots-a00001.md"),
        parent
            .replacen("id: dot-a00001", "id: dots-a00001", 1)
            .replace("priority: 2", "priority: \"2\"")
    );
    let expected = child
        .replacen("id: dot-a00002", "id: dots-a00002", 1)
        .replacen("parent: dot-a00001", "parent: dots-a00001", 1)
        .replacen("depends: [dot-a00001", "depends: [dots-a00001", 1)
        .replace("priority: 2", "priority: \"2\"");
    assert_eq!(env.read(&dir, "tasks/dots-a00002.md"), expected);
    assert_eq!(
        env.read(&foreign, &format!("tasks/{inbound}.md")),
        foreign_before
    );
    let shown = env.json(&dir, &["show", "dot-a00002"]);
    assert_eq!(shown["task"]["parent"], "dots-a00001");
    assert_eq!(
        shown["task"]["depends"],
        serde_json::json!(["dots-a00001", foreign_id])
    );
    assert!(env.check(&dir)["errors"].as_array().unwrap().is_empty());
    let check = env.check(&foreign);
    assert!(
        check["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "retired_prefix")
    );
    env.json(&foreign, &["edit", &inbound, "--title", "Unrelated edit"]);
    assert_eq!(
        env.json(&foreign, &["show", &inbound])["task"]["depends"][0],
        "dot-a00002"
    );
}

#[test]
fn rename_fresh_destination_collision_names_the_obstacle_and_keeps_both_files() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, false);
    let dest = dir.join(format!("tasks/{}.md", ids[0].replacen("dot-", "dots-", 1)));
    std::fs::write(&dest, "unrelated destination").unwrap();
    let original = env.read(&dir, &format!("tasks/{}.md", ids[0]));
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("destination") && detail.contains("dots"),
        "{detail}"
    );
    assert_eq!(env.read(&dir, &format!("tasks/{}.md", ids[0])), original);
    assert_eq!(
        std::fs::read_to_string(dest).unwrap(),
        "unrelated destination"
    );
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_explains_a_missing_config_and_refuses_recovery_without_writes() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, false);
    rename_stop(&env, &dir, "dot", "dots", "inventory");
    std::fs::remove_file(dir.join("tasks/.config.toml")).unwrap();
    let before = rename_disk_state(&env, &dir);
    let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explain["recovery"], "refuse");
    assert!(explain["warnings"][0].as_str().unwrap().starts_with("R6:"));
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

#[test]
fn group_set_refuses_a_name_an_unfinished_rename_reserves() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&sci, &["add", "S", "-p", "2"]));
    let stopped = env
        .raw(&sci)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "lab"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");

    // `lab` is not yet a prefix, so only the reservation stands between it and a group.
    // A group there would make the resume rewrite files and config, then fail at the
    // registry step on the collision.
    let error = error_of(&env, &fam, &["group", "set", "lab", "fam"]);
    assert_eq!(error["error"]["kind"], "validation");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("reserved by unfinished rename sci -> lab"),
        "{detail}"
    );
    assert_eq!(env.json(&fam, &["groups"])["groups"], serde_json::json!([]));

    // The name stayed free, so the rename resumes and finishes.
    assert_eq!(
        env.json(&sci, &["rename", "sci", "lab"])["recovery"],
        "resume_files"
    );
    assert!(sci.join(format!("tasks/lab-{}.md", &id[4..])).is_file());
    // Finished, `lab` is a live prefix and `sci` a retired one: still refused, now as
    // prefixes rather than reservations.
    for name in ["lab", "sci"] {
        assert_eq!(
            env.fail(&fam, &["group", "set", name, "fam"]),
            "validation",
            "{name}"
        );
    }
    env.json(&fam, &["group", "set", "vf", "lab", "fam"]);
}

#[test]
fn rename_rewrites_group_members_and_refuses_a_group_name_as_target() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    env.json(&sci, &["group", "set", "vf", "sci", "fam"]);
    env.json(&sci, &["group", "set", "science", "fam"]);
    assert_eq!(env.fail(&sci, &["rename", "sci", "science"]), "config");
    env.json(&sci, &["rename", "sci", "lab"]);
    assert_eq!(
        env.json(&sci, &["groups"])["groups"][1],
        serde_json::json!({"name": "vf", "members": [
            {"prefix": "fam", "reachable": true},
            {"prefix": "lab", "reachable": true},
        ]})
    );
    // The retired name resolves on set, as any alias does.
    assert_eq!(
        env.json(&sci, &["group", "set", "vf", "sci"])["members"],
        serde_json::json!(["lab"])
    );
}

#[test]
fn adopt_carries_group_membership_and_refuses_a_group_named_target() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    adopt_json(&env, &dir, &["group", "set", "new", "old"]);
    let refused = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    let error: serde_json::Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"].as_str().unwrap().contains("group"),
        "{error}"
    );

    adopt_json(&env, &dir, &["group", "rm", "new"]);
    adopt_json(&env, &dir, &["group", "set", "vf", "old"]);
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
    assert_eq!(
        adopt_json(&env, &dir, &["groups"])["groups"],
        serde_json::json!([{"name": "vf", "members": [{"prefix": "new", "reachable": true}]}])
    );
}

#[test]
fn rename_explain_and_execution_both_refuse_a_group_named_target() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&sci, &["add", "S", "-p", "2"]));
    env.json(&fam, &["group", "set", "vf", "fam"]);
    let state = env.home.path().join(".local/state/tasks/rename");

    // Fresh: explain predicts the refusal execution gives.
    let explained = env.json(&sci, &["rename", "sci", "vf", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"][0]
            .as_str()
            .unwrap()
            .starts_with("R12:"),
        "{explained}"
    );
    assert_eq!(env.fail(&sci, &["rename", "sci", "vf"]), "config");
    assert!(
        !state.join("sci.toml").exists(),
        "refused before the inventory"
    );

    // Mid-rename: a group that appears under the pending target blocks the resume before
    // any file moves. `group set` refuses the reserved name, so only a hand edit of the
    // registry gets there.
    let stopped = env
        .raw(&sci)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "lab"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[groups]\n"), "{text}");
    std::fs::write(
        &path,
        text.replace("[groups]\n", "[groups]\nlab = [\"fam\"]\n"),
    )
    .unwrap();
    let explained = env.json(&sci, &["rename", "sci", "lab", "--explain"]);
    assert_eq!(explained["recovery"], "refuse", "{explained}");
    assert!(
        explained["warnings"][0]
            .as_str()
            .unwrap()
            .starts_with("R12:"),
        "{explained}"
    );
    assert_eq!(env.fail(&sci, &["rename", "sci", "lab"]), "config");
    assert!(
        sci.join(format!("tasks/{id}.md")).is_file(),
        "no task file moved"
    );
    assert!(!sci.join(format!("tasks/lab-{}.md", &id[4..])).exists());
    assert!(
        state.join("sci.toml").is_file(),
        "the inventory waits for the resume"
    );

    env.json(&fam, &["group", "rm", "lab"]);
    assert_eq!(
        env.json(&sci, &["rename", "sci", "lab"])["recovery"],
        "resume_files"
    );
}

#[test]
fn adopt_refuses_a_target_registered_at_a_retired_root_elsewhere() {
    let env = TestEnv::new();
    let (dir, _old_id) = adopt_fixture(&env);
    let other = env.home.path().join("other");
    std::fs::create_dir(&other).unwrap();
    adopt_json(&env, &other, &["init", "--prefix", "new"]);
    adopt_json(&env, &other, &["retire", "--reason", "r"]);
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let before = std::fs::read(&registry_path).unwrap();
    let refused = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let detail = error_detail(&refused);
    assert!(detail.contains("is registered at"), "{detail}");
    assert_eq!(std::fs::read(registry_path).unwrap(), before);
}

#[test]
fn adopt_explain_refuses_a_retired_checkout_as_the_run_does() {
    let env = TestEnv::new();
    let (dir, _old_id) = adopt_fixture(&env);
    declare_retired(&dir, None);
    for args in [
        &["rename", "old", "new", "--adopt", "--explain"][..],
        &["rename", "old", "new", "--adopt"][..],
    ] {
        let out = adopt_cmd(&env, &dir).args(args).output().unwrap();
        assert_eq!(err_kind(&out), "retired_project", "{args:?}");
    }
}

#[test]
fn rename_explain_refuses_a_retired_project_as_the_run_does() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    for args in [
        &["rename", "old", "neu", "--explain"][..],
        &["rename", "old", "neu"][..],
    ] {
        let out = env.cmd(&old).args(args).output().unwrap();
        assert_eq!(err_kind(&out), "retired_project", "{args:?}");
    }
}
