mod common;
use common::{
    REAP, TestEnv, declare_retired, err_kind, error_detail, git, hold_project_lock, id_of,
    owners_env, reap, repo_with_worktree_as, wait_bounded, warnings_of,
};
use std::time::Duration;

#[test]
fn retired_table_check_reports_an_open_task() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.json(&old, &["add", "T", "-p", "2"]);
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!("{text}\n[retired]\ndate = \"2026-10-04\"\nreason = \"r\"\n"),
    )
    .unwrap();
    let out = env.cmd(&old).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "open_in_retired"),
        "{report}"
    );
}

#[test]
fn retired_write_every_command_is_refused_in_the_registered_checkout() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    let id2 = id_of(env.json(&old, &["add", "U", "-p", "2"]));
    let blocked = id_of(env.json(&old, &["add", "B", "-p", "2"]));
    env.json(&old, &["block", &blocked, "why"]);
    let shelved = id_of(env.json(&old, &["add", "S", "-p", "2"]));
    env.json(&old, &["shelve", &shelved, "--defer", "30d", "later"]);
    let claimed = id_of(env.json(&old, &["add", "C", "-p", "2"]));
    env.json(&old, &["start", &claimed]);
    declare_retired(&old, Some("new"));
    let writes: Vec<Vec<&str>> = vec![
        vec!["add", "N", "-p", "2"],
        vec!["note", &id, "x"],
        vec!["edit", &id, "--title", "y"],
        vec!["start", &id],
        vec!["park", &claimed, "next"],
        vec!["done", &id, "m"],
        vec!["drop", &id, "m"],
        vec!["block", &id, "m"],
        vec!["unblock", &blocked],
        vec!["shelve", &id, "--defer", "30d", "m"],
        vec!["unshelve", &shelved],
        vec!["dep", &id, "--on", &id2],
    ];
    for args in &writes {
        assert_eq!(env.fail(&old, args), "retired_project", "{args:?}");
    }
}

#[test]
fn retired_write_attach_and_detach_are_refused() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    let file = old.join("shot.txt");
    std::fs::write(&file, "x").unwrap();
    env.json(&old, &["attach", &id, file.to_str().unwrap()]);
    declare_retired(&old, None);
    // A second name: re-attaching the same one fails `attachment_exists` before the lock.
    let second = old.join("shot2.txt");
    std::fs::write(&second, "y").unwrap();
    assert_eq!(
        env.fail(&old, &["attach", &id, second.to_str().unwrap()]),
        "retired_project"
    );
    assert_eq!(
        env.fail(&old, &["detach", &id, "shot.txt", "why"]),
        "retired_project"
    );
}

#[test]
fn retired_write_a_worktree_made_before_retirement_cannot_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&main, None); // only the registered checkout knows
    assert!(
        !std::fs::read_to_string(side.join("tasks/.config.toml"))
            .unwrap()
            .contains("[retired]")
    );
    for args in [
        vec!["add", "N", "-p", "2"],
        vec!["note", &id, "x"],
        vec!["park", &id, "next"],
        vec!["done", &id, "m"],
    ] {
        assert_eq!(env.fail(&side, &args), "retired_project", "{args:?}");
    }
}

#[test]
fn retired_write_an_unreadable_registered_checkout_refuses_a_worktree_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree_as(&mut env, "old");
    let config = main.join("tasks/.config.toml");
    let good = std::fs::read(&config).unwrap();
    let record = side.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    for broken in [
        &b"prefix = \"old\"\n[retired]\nreason = 3\n"[..],
        &b"prefix = \"other\"\n"[..],
    ] {
        std::fs::write(&config, broken).unwrap();
        assert_eq!(env.fail(&side, &["note", &id, "x"]), "config");
        assert_eq!(std::fs::read(&record).unwrap(), before);
    }
    std::fs::write(&config, &good).unwrap();
    env.json(&side, &["note", &id, "x"]);
}

#[test]
fn retired_write_a_worktree_that_declares_it_refuses() {
    let mut env = TestEnv::new();
    let (_main, side, _id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&side, None);
    assert_eq!(env.fail(&side, &["add", "N", "-p", "2"]), "retired_project");
}

#[test]
fn retired_write_feedback_is_refused() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[feedback]\nscope = \"s\"\n")).unwrap();
    declare_retired(&old, None);
    let here = env.init("here");
    assert_eq!(
        env.fail(
            &here,
            &[
                "feedback",
                "--project",
                "old",
                "the tool line could be clearer",
                "--category",
                "gap"
            ]
        ),
        "retired_project"
    );
}

#[test]
fn retired_write_refusal_names_the_successor_and_the_way_back() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, Some("new"));
    let out = env
        .cmd(&old)
        .args(["add", "N", "-p", "2"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("new") && detail.contains("tasks unretire"),
        "{detail}"
    );
}

#[test]
fn retired_write_editor_save_is_refused_after_retirement_lands() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    // The editor declares retirement, then changes the title, so the re-lock sees it.
    let script = old.join("editor.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '\\n[retired]\\ndate = \"2026-10-04\"\\nreason = \"r\"\\n' >> {}\nsed -i 's/^title: .*/title: changed/' \"$1\"\n",
            old.join("tasks/.config.toml").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = env
        .cmd(&old)
        .env("EDITOR", &script)
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("retired_project"));
    let record = std::fs::read_to_string(old.join(format!("tasks/{id}.md"))).unwrap();
    assert!(!record.contains("title: changed"));
}

#[test]
fn retired_lock_init_waits_for_the_project_lock() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let held = hold_project_lock(&env, "old");
    let mut command = env.raw(&old);
    command
        .args(["init", "--prefix", "old", "--force"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    let finished_early = wait_bounded(&mut child, Duration::from_millis(500));
    drop(held);
    let out = reap(child, REAP);
    assert!(
        !finished_early,
        "init --force must wait for the project lock"
    );
    assert!(out.expect("reaped").status.success());
}

#[test]
fn retired_lock_init_force_and_rename_refuse() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    assert_eq!(
        env.fail(&old, &["init", "--prefix", "old", "--force"]),
        "retired_project"
    );
    // Re-pointing the prefix at a fresh root is refused while the registered one is retired.
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(elsewhere.path(), &["init", "--prefix", "old", "--force"]),
        "retired_project"
    );
    assert_eq!(
        env.fail(&old, &["rename", "old", "older"]),
        "retired_project"
    );
}

#[test]
fn retired_lock_unregister_still_works() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    declare_retired(&old, None);
    let here = env.init("here");
    env.json(&here, &["unregister", "old"]);
}

/// A project that is its own git repository with one *closed* task committed, plus a
/// linked worktree branched after the close, so every checkout's copy is closed.
fn closed_repo_with_worktree(
    env: &mut TestEnv,
    prefix: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let main = env.init(prefix);
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    env.json(&main, &["drop", &id, "seed"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    (main, side)
}

fn spawn_piped(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> std::process::Child {
    let mut command = env.raw(dir);
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command.spawn().unwrap()
}

fn kind_of(output: &std::process::Output) -> String {
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    error["error"]["kind"].as_str().unwrap().to_string()
}

#[test]
fn retire_cmd_writes_the_table_and_unretire_removes_it_keeping_comments() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.init("new");
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("# keep me\n{text}")).unwrap();
    let out = env.json(
        &old,
        &["retire", "--successor", "new", "--reason", "superseded"],
    );
    assert_eq!(out["retired"]["successor"], "new");
    let written = std::fs::read_to_string(&config).unwrap();
    assert!(written.contains("# keep me") && written.contains("[retired]"));
    assert_eq!(env.fail(&old, &["add", "N", "-p", "2"]), "retired_project");
    env.json(&old, &["unretire"]);
    let restored = std::fs::read_to_string(&config).unwrap();
    assert!(restored.contains("# keep me") && !restored.contains("[retired]"));
    env.json(&old, &["add", "N", "-p", "2"]);
}

#[test]
fn retire_cmd_unretire_on_an_active_project_changes_nothing() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let before = std::fs::read(old.join("tasks/.config.toml")).unwrap();
    let out = env.json(&old, &["unretire"]);
    assert!(out["retired"].is_null());
    assert_eq!(
        std::fs::read(old.join("tasks/.config.toml")).unwrap(),
        before
    );
}

#[test]
fn retire_cmd_an_invalid_reason_leaves_the_config_unchanged() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let config = old.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    for reason in ["", "   ", "two\nlines", "tab\there"] {
        assert_eq!(
            env.fail(&old, &["retire", "--reason", reason]),
            "config",
            "{reason:?}"
        );
        assert_eq!(std::fs::read(&config).unwrap(), before, "{reason:?}");
    }
    env.json(&old, &["add", "still writable", "-p", "2"]);
}

#[test]
fn retire_cmd_refuses_an_open_task() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "open_tasks");
    env.json(&old, &["drop", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
}

#[test]
fn retire_cmd_refuses_its_own_prefix_an_unregistered_or_a_retired_successor() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let mid = env.init("mid");
    env.json(&mid, &["retire", "--reason", "r"]);
    for bad in ["old", "nowhere", "mid"] {
        assert_eq!(
            env.fail(&old, &["retire", "--successor", bad, "--reason", "r"]),
            "config",
            "{bad}"
        );
    }
}

#[test]
fn retire_cmd_refuses_a_live_claim_on_the_prefix() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["start", &id]);
    // Close the record without releasing the claim, the way a sync can leave it.
    let path = old.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("status: doing", "status: done");
    std::fs::write(&path, text).unwrap();
    assert_eq!(env.fail(&old, &["retire", "--reason", "r"]), "claimed");
}

#[test]
fn retire_cmd_refuses_an_open_task_that_exists_only_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let only_here = id_of(env.json(&side, &["add", "W", "-p", "2"])); // `add` takes no claim
    let out = env
        .cmd(&main)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(kind_of(&out), "open_tasks");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&only_here) && detail.contains(side.to_str().unwrap()),
        "{detail}"
    );
}

#[test]
fn retire_cmd_refuses_an_unreadable_linked_worktree() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(side.join("tasks"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let kind = env.fail(&main, &["retire", "--reason", "r"]);
    std::fs::set_permissions(side.join("tasks"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(kind, "unreadable_checkout");
    assert!(
        !std::fs::read_to_string(main.join("tasks/.config.toml"))
            .unwrap()
            .contains("[retired]")
    );
}

#[test]
fn retire_cmd_and_unretire_refuse_in_a_linked_worktree() {
    let mut env = TestEnv::new();
    let (_main, side) = closed_repo_with_worktree(&mut env, "old");
    assert_eq!(
        env.fail(&side, &["retire", "--reason", "r"]),
        "not_registered_checkout"
    );
    assert_eq!(env.fail(&side, &["unretire"]), "not_registered_checkout");
}

#[test]
fn retire_cmd_and_unretire_respect_a_pending_rename() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.json(&dir, &["drop", &id, "m"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();
    let config = dir.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    // Same kind a write gets during the freeze (a_pending_rename_freezes_the_project_*).
    assert_eq!(env.fail(&dir, &["retire", "--reason", "r"]), "validation");
    assert_eq!(env.fail(&dir, &["unretire"]), "validation");
    assert_eq!(std::fs::read(&config).unwrap(), before);
}

/// Retire takes the project lock: it waits behind a holder and finishes after release.
#[test]
fn retire_cmd_waits_for_the_project_lock() {
    let mut env = TestEnv::new();
    let (main, _side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut retire = spawn_piped(&env, &main, &["retire", "--reason", "r"]);
    let finished_early = wait_bounded(&mut retire, Duration::from_millis(500));
    drop(held);
    let out = reap(retire, REAP);
    assert!(!finished_early, "retire must wait for the project lock");
    assert!(out.expect("reaped").status.success());
}

/// Writer first: a stale-worktree write that holds the lock lands, and the retire that
/// follows it through the lock sees the new task and refuses.
#[test]
fn retire_cmd_after_a_worktree_writer_sees_its_task() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut writer = spawn_piped(&env, &side, &["add", "W", "-p", "2"]);
    let writer_waited = !wait_bounded(&mut writer, Duration::from_millis(300));
    drop(held);
    let writer = reap(writer, REAP).expect("writer reaped");
    assert!(writer_waited && writer.status.success());
    let retire = env
        .cmd(&main)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(kind_of(&retire), "open_tasks");
}

/// Retire first: once it holds the lock and lands, a writer queued behind it in a stale
/// worktree (whose own config has no [retired]) is refused.
#[test]
fn retire_cmd_before_a_worktree_writer_refuses_it() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    let held = hold_project_lock(&env, "old");
    let mut retire = spawn_piped(&env, &main, &["retire", "--reason", "r"]);
    let retire_waited = !wait_bounded(&mut retire, Duration::from_millis(300));
    drop(held);
    let retire = reap(retire, REAP).expect("retire reaped");
    assert!(retire_waited && retire.status.success());
    assert!(
        !std::fs::read_to_string(side.join("tasks/.config.toml"))
            .unwrap()
            .contains("[retired]")
    );
    let writer = env
        .cmd(&side)
        .args(["add", "W", "-p", "2"])
        .output()
        .unwrap();
    assert_eq!(kind_of(&writer), "retired_project");
    assert!(std::fs::read_dir(side.join("tasks")).unwrap().all(|e| {
        let path = e.unwrap().path();
        path.extension().is_none_or(|x| x != "md")
            || !std::fs::read_to_string(&path).unwrap().contains("title: W")
    }));
}

#[test]
fn retired_read_wide_reads_skip_it_without_a_warning() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let closed = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &closed, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    env.json(&live, &["add", "L", "-p", "2"]);
    for args in [
        vec!["list", "--all-projects", "--status", "dropped"],
        vec!["ready", "--all-projects"],
        vec!["prime", "--all-projects"],
        vec!["tags", "--all-projects"],
        vec!["sample", "--all-projects"],
        vec!["quiet"],
    ] {
        let out = env.json(&live, &args);
        let text = out.to_string();
        assert!(!text.contains("\"old"), "{args:?}: {text}");
        assert!(
            warnings_of(&out).iter().all(|w| !names_word(w, "old")),
            "{args:?}"
        );
    }
    env.json(&live, &["group", "set", "pair", "live"]);
    assert_eq!(
        env.fail(&live, &["group", "set", "both", "live", "old"]),
        "retired_project"
    );
}

#[test]
fn retired_read_a_group_made_before_retirement_skips_the_member() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    env.json(&live, &["group", "set", "pair", "live", "old"]);
    let closed = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &closed, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    env.json(&live, &["add", "L", "-p", "2"]);
    for args in [
        vec!["ready", "--group", "pair"],
        vec!["list", "--group", "pair", "--status", "dropped"],
    ] {
        let out = env.json(&live, &args);
        assert!(!out.to_string().contains("\"old-"), "{args:?}");
        assert!(
            warnings_of(&out).iter().all(|w| !names_word(w, "old")),
            "{args:?}"
        );
    }
}

#[test]
fn retired_read_a_dependency_on_a_retired_task_resolves_and_never_blocks() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let dep = id_of(env.json(&old, &["add", "D", "-p", "2"]));
    let waiter = id_of(env.json(&live, &["add", "W", "-p", "2"]));
    env.json(&live, &["dep", &waiter, "--on", &dep]);
    env.json(&old, &["done", &dep, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    let ready = env.json(&live, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == waiter.as_str())
    );
    let report = env.check(&live);
    assert!(report.as_object().is_none_or(|r| {
        r.get("warnings")
            .and_then(|w| w.as_array())
            .is_none_or(|w| w.iter().all(|f| f["kind"] != "unreachable_dep"))
    }));
}

#[test]
fn retired_read_single_project_reads_and_show_warn() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let live = env.init("live");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["drop", &id, "m"]);
    env.json(&old, &["retire", "--successor", "live", "--reason", "r"]);
    let shown = env.json(&live, &["show", &id]);
    assert_eq!(shown["task"]["id"], id.as_str());
    assert!(
        warnings_of(&shown)
            .iter()
            .any(|w| w.contains("old is retired") && w.contains("successor live"))
    );
    let listed = env.json(&live, &["list", "--project", "old", "--status", "dropped"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert!(
        warnings_of(&listed)
            .iter()
            .any(|w| w.contains("old is retired"))
    );
}

#[test]
fn retired_read_a_gone_or_retired_successor_does_not_break_reads() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let mid = env.init("mid");
    env.json(&old, &["retire", "--successor", "mid", "--reason", "r"]);
    env.json(&mid, &["retire", "--reason", "r"]);
    let here = env.init("here");
    env.json(&here, &["projects"]);
    env.json(&here, &["unregister", "mid"]);
    let rows = env.json(&here, &["projects"]);
    let row = rows["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["prefix"] == "old")
        .unwrap();
    assert_eq!(row["retired"]["successor"], "mid");
}

#[test]
fn retired_read_projects_reports_retirement_and_keeps_counts() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    env.json(&old, &["done", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
    let rows = env.json(&old, &["projects"]);
    let row = rows["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["prefix"] == "old")
        .unwrap();
    assert_eq!(row["retired"]["reason"], "r");
    assert_eq!(row["counts"]["done"], 1);
    assert!(env.pretty(&old, &["projects"]).contains("retired"));
}

/// Whether `text` contains `word` as a whole word: "old" is in "old-1a2b3c" and
/// "old is retired", not in "holds".
fn names_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric())
        .any(|w| w == word)
}

#[test]
fn retire_cmd_ignores_a_kept_edit_temp_file() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let id = id_of(env.json(&old, &["add", "T", "-p", "2"]));
    // A failed edit keeps tasks/.<id>.<hex>.edit.md holding the record as it was (open).
    let out = env
        .cmd(&old)
        .env("EDITOR", "false")
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        std::fs::read_dir(old.join("tasks")).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with('.'))
    );
    env.json(&old, &["done", &id, "m"]);
    env.json(&old, &["retire", "--reason", "r"]);
}

#[test]
fn retired_lock_init_force_refuses_when_the_retired_root_config_does_not_parse() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.json(&old, &["retire", "--reason", "r"]);
    let config = old.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\ngarbage = [\n")).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let before = std::fs::read(&registry_path).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(elsewhere.path(), &["init", "--prefix", "old", "--force"]),
        "config"
    );
    assert_eq!(std::fs::read(&registry_path).unwrap(), before);
}

#[test]
fn retired_write_every_command_is_refused_from_a_worktree_made_before_retirement() {
    let mut env = TestEnv::new();
    let main = env.init("old");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    let id2 = id_of(env.json(&main, &["add", "U", "-p", "2"]));
    let blocked = id_of(env.json(&main, &["add", "B", "-p", "2"]));
    env.json(&main, &["block", &blocked, "why"]);
    let shelved = id_of(env.json(&main, &["add", "S", "-p", "2"]));
    env.json(&main, &["shelve", &shelved, "--defer", "30d", "later"]);
    let attached = id_of(env.json(&main, &["add", "A", "-p", "2"]));
    let file = main.join("shot.txt");
    std::fs::write(&file, "x").unwrap();
    env.json(&main, &["attach", &attached, file.to_str().unwrap()]);
    let config = main.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[feedback]\nscope = \"s\"\n")).unwrap();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    declare_retired(&main, None); // only the registered checkout knows
    let second = side.join("shot2.txt");
    std::fs::write(&second, "y").unwrap();
    let editor = side.join("editor.sh");
    std::fs::write(
        &editor,
        "#!/bin/sh\nsed -i 's/^title: .*/title: changed/' \"$1\"\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    let writes: Vec<Vec<&str>> = vec![
        vec!["add", "N", "-p", "2"],
        vec!["note", &id, "x"],
        vec!["edit", &id, "--title", "y"],
        vec!["start", &id],
        vec!["done", &id, "m"],
        vec!["drop", &id, "m"],
        vec!["block", &id, "m"],
        vec!["unblock", &blocked],
        vec!["shelve", &id, "--defer", "30d", "m"],
        vec!["unshelve", &shelved],
        vec!["dep", &id, "--on", &id2],
        vec!["attach", &attached, second.to_str().unwrap()],
        vec!["detach", &attached, "shot.txt", "why"],
    ];
    for args in &writes {
        assert_eq!(env.fail(&side, args), "retired_project", "{args:?}");
    }
    let edited = env
        .cmd(&side)
        .env("EDITOR", &editor)
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(edited.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&edited.stderr).contains("retired_project"));
    let here = env.init("here");
    assert_eq!(
        env.fail(
            &here,
            &[
                "feedback",
                "--project",
                "old",
                "the tool line could be clearer",
                "--category",
                "gap"
            ]
        ),
        "retired_project"
    );
}

#[test]
fn retire_cmd_refuses_an_already_retired_project_and_keeps_its_declaration() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    env.json(&old, &["retire", "--reason", "first"]);
    let config = old.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    let out = env
        .cmd(&old)
        .args(["retire", "--reason", "second"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "retired_project");
    assert!(error_detail(&out).contains("tasks unretire"));
    assert_eq!(std::fs::read(&config).unwrap(), before);
}

#[test]
fn retire_cmd_stores_a_successor_alias_as_its_live_prefix() {
    let mut env = TestEnv::new();
    let old = env.init("old");
    let new = env.init("new");
    env.init("other");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    registry["aliases"] = toml::toml! { nu = "new" }.into();
    std::fs::write(&path, toml::to_string(&registry).unwrap()).unwrap();
    let out = env.json(&old, &["retire", "--successor", "nu", "--reason", "r"]);
    assert_eq!(out["retired"]["successor"], "new");
    assert!(
        std::fs::read_to_string(old.join("tasks/.config.toml"))
            .unwrap()
            .contains("successor = \"new\"")
    );
    // A refusal names the alias as typed and the prefix it now stands for.
    env.json(&old, &["unretire"]);
    env.json(&new, &["retire", "--successor", "other", "--reason", "r"]);
    let refused = env
        .cmd(&old)
        .args(["retire", "--successor", "nu", "--reason", "r"])
        .output()
        .unwrap();
    let detail = error_detail(&refused);
    assert!(detail.contains("\"nu\" (now \"new\")"), "{detail}");
}

#[test]
fn retired_read_feedback_owners_and_wide_id_completion_leave_it_out() {
    let (env, _ai, ops, sci) = owners_env();
    let gone = id_of(env.json(&ops, &["add", "Old"]));
    env.json(&ops, &["drop", &gone, "m"]);
    env.json(&ops, &["retire", "--reason", "r"]);
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "feedback", "--project", ""]),
        ["ai", "sci"]
    );
    let local = id_of(env.json(&sci, &["add", "Local"]));
    for scope in [vec!["--group", "all"], vec!["--all-projects"]] {
        if scope[0] == "--group" {
            env.json(&sci, &["group", "set", "all", "sci", "ai"]);
            // A group made before the retirement still names ops.
            let path = env.home.path().join(".config/tasks/projects.toml");
            let mut registry: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            registry["groups"]["all"] = toml::toml! { x = ["ai", "ops", "sci"] }["x"].clone();
            std::fs::write(&path, toml::to_string(&registry).unwrap()).unwrap();
            let members = env.json(&sci, &["groups"]).to_string();
            assert!(members.contains("\"ops\""), "{members}");
        }
        let mut words = vec!["tasks", "list"];
        words.extend(&scope);
        words.extend(["--parent", ""]);
        let wide = env.complete(&sci, "bash", words.len() - 1, &words);
        assert!(wide.contains(&local), "{scope:?}: {wide:?}");
        assert!(!wide.contains(&gone), "{scope:?}: {wide:?}");
    }
    assert_eq!(
        env.fail(
            &sci,
            &[
                "feedback",
                "--project",
                "ops",
                "x is slow",
                "--category",
                "gap",
            ]
        ),
        "retired_project"
    );
}

#[test]
fn retired_read_check_in_a_worktree_made_before_retirement_reports_open_work() {
    let mut env = TestEnv::new();
    let (main, side, _id) = repo_with_worktree_as(&mut env, "old");
    declare_retired(&main, None); // only the registered checkout knows
    let out = env.cmd(&side).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let finding = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "open_in_retired")
        .unwrap_or_else(|| panic!("{report}"))
        .clone();
    let detail = finding["detail"].as_str().unwrap();
    assert!(
        detail.contains(main.to_str().unwrap()) && detail.contains("stale"),
        "{detail}"
    );

    // A registered checkout that cannot be read is a finding, not an abort.
    let config = main.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("<<<<<<< HEAD\n{text}")).unwrap();
    let out = env.cmd(&side).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "retirement_unknown"),
        "{report}"
    );
}

#[test]
fn retire_cmd_names_git_worktree_prune_for_a_worktree_that_is_gone() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    std::fs::remove_dir_all(&side).unwrap();
    let out = env
        .cmd(&main)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unreadable_checkout");
    let detail = error_detail(&out);
    assert!(detail.contains("git worktree prune"), "{detail}");
    assert!(!detail.contains("unlock"), "{detail}");
    git(&main, &["worktree", "prune"]);
    env.json(&main, &["retire", "--reason", "r"]);
}

#[test]
fn retire_cmd_names_unlock_for_a_locked_worktree_that_is_gone() {
    let mut env = TestEnv::new();
    let (main, side) = closed_repo_with_worktree(&mut env, "old");
    git(
        &main,
        &[
            "worktree",
            "lock",
            "--reason",
            "kept",
            side.to_str().unwrap(),
        ],
    );
    std::fs::remove_dir_all(&side).unwrap();
    let out = env
        .cmd(&main)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unreadable_checkout");
    let detail = error_detail(&out);
    assert!(detail.contains("git worktree unlock"), "{detail}");
    git(&main, &["worktree", "unlock", side.to_str().unwrap()]);
    git(&main, &["worktree", "prune"]);
    env.json(&main, &["retire", "--reason", "r"]);
}

#[test]
fn retire_cmd_names_a_worktree_whose_commit_predates_a_nested_project() {
    let env = TestEnv::new();
    let repo = env.home.path().canonicalize().unwrap().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("README"), "r").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "before"]);
    let sub = repo.join("sub");
    std::fs::create_dir(&sub).unwrap();
    env.json(&sub, &["init", "--prefix", "old"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "project"]);
    let wt = repo.join("wt");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "old-side",
            wt.to_str().unwrap(),
            "HEAD~1",
        ],
    );
    let out = env
        .cmd(&sub)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unreadable_checkout");
    let detail = error_detail(&out);
    assert!(
        detail.contains("has no sub/tasks at its commit"),
        "{detail}"
    );
    assert!(!detail.contains("prune"), "{detail}");
}

#[test]
fn retire_cmd_names_a_worktree_whose_commit_predates_the_records() {
    let env = TestEnv::new();
    let repo = env.home.path().canonicalize().unwrap().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("README"), "r").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "before"]);
    env.json(&repo, &["init", "--prefix", "old"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "project"]);
    let wt = repo.join("wt");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "old-side",
            wt.to_str().unwrap(),
            "HEAD~1",
        ],
    );
    let out = env
        .cmd(&repo)
        .args(["retire", "--reason", "r"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unreadable_checkout");
    let detail = error_detail(&out);
    assert!(detail.contains("has no tasks at its commit"), "{detail}");
}
