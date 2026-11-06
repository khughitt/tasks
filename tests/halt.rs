mod common;
use common::{
    TestEnv, editor_script, error_of, grouped_projects, id_of, relay_on, repo_with_worktree,
    shim_env, warnings_of, write_claim,
};

#[test]
fn halt_start_uncommitted_registered_incident_blocks_existing_worktree_immediately() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert!(main.join(format!("tasks/{halt}.md")).exists());
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_takeover_hints_include_reason_in_ready_and_prime() {
    let mut env = TestEnv::new();
    let (main, side, live) = repo_with_worktree(&mut env);
    let stale = id_of(env.json(&side, &["add", "Stale", "-p", "2"]));
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    write_claim(&env, "sci", &live, "live-agent", true);
    write_claim(&env, "sci", &stale, "dead-agent", false);

    let ready = warnings_of(&env.json(&side, &["ready"])).join("\n");
    let live_hint = format!("`tasks start --force {live} --reason \"...\"`");
    assert!(ready.contains(&live_hint), "{ready}");
    let next = warnings_of(&env.json(&side, &["next"])).join("\n");
    assert!(next.contains(&live_hint), "{next}");
    let prime = warnings_of(&env.json(&side, &["prime"])).join("\n");
    assert!(
        prime.contains(&format!("`tasks start --force {stale} --reason \"...\"`")),
        "{prime}"
    );

    std::fs::remove_file(main.join(format!("tasks/{halt}.md"))).unwrap();
    let ready = warnings_of(&env.json(&side, &["ready"])).join("\n");
    assert!(
        ready.contains(&format!("`tasks start --force {live}`")),
        "{ready}"
    );
    let next = warnings_of(&env.json(&side, &["next"])).join("\n");
    assert!(
        next.contains(&format!("`tasks start --force {live}`")),
        "{next}"
    );
    let prime = warnings_of(&env.json(&side, &["prime"])).join("\n");
    assert!(
        prime.contains(&format!("`tasks start --force {stale}`")),
        "{prime}"
    );
}

#[test]
fn halt_edit_status_doing_cannot_bypass_registered_halt_even_with_force() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert_eq!(
        env.fail(&side, &["edit", &target, "--status", "doing"]),
        "halted"
    );
    assert_eq!(
        env.fail(&side, &["edit", &target, "--status", "doing", "--force"]),
        "validation"
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_editor_status_doing_cannot_bypass_registered_halt() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    let editor = editor_script(&side, "sed -i 's/status: todo/status: doing/' \"$1\"");
    let out = env
        .cmd(&side)
        .env("EDITOR", editor)
        .args(["edit", &target, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "halted");
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_start_side_closure_does_not_lift_until_registered_record_closes() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    std::fs::copy(
        main.join(format!("tasks/{halt}.md")),
        side.join(format!("tasks/{halt}.md")),
    )
    .unwrap();
    env.json(&side, &["done", &halt, "local resolution"]);
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(
        !std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains(&target)
    );
    // The record-home guard now refuses to close an older sibling copy. Remove the
    // side fixture after proving its closure did not lift the registered halt.
    std::fs::remove_file(side.join(format!("tasks/{halt}.md"))).unwrap();
    env.json(&main, &["done", &halt, "registered resolution"]);
    env.json(&side, &["start", &target]);
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "doing"
    );
    assert!(
        std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains(&target)
    );
}

#[test]
fn halt_start_unreadable_registered_checkout_refuses_force() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    std::fs::remove_file(main.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]),
        "config"
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_start_malformed_registered_config_refuses_before_claim_or_task_write() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    std::fs::write(main.join("tasks/.config.toml"), "not toml = [").unwrap();
    let before = env.read(&side, &format!("tasks/{target}.md"));
    let kind = env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]);
    assert!(matches!(kind.as_str(), "config" | "parse"), "{kind}");
    assert_eq!(env.read(&side, &format!("tasks/{target}.md")), before);
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_start_equal_priority_halts_sort_by_id_and_allow_equal_target() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let first = id_of(env.json(&main, &["add", "First", "-p", "1", "--tag", "halt"]));
    let second = id_of(env.json(&main, &["add", "Second", "-p", "1", "--tag", "halt"]));
    let mut expected = vec![first, second];
    expected.sort();
    let ids: Vec<String> = env.json(&side, &["ready"])["halts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, expected);
    env.json(&side, &["edit", &target, "-p", "1"]);
    env.json(&side, &["start", &target]);
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "doing"
    );
}

#[test]
fn halt_start_unregistered_checkout_uses_local_authority() {
    let mut env = TestEnv::new();
    let project = env.init("sci");
    let target = id_of(env.json(&project, &["add", "Ordinary", "-p", "2"]));
    std::fs::remove_file(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    env.json(&project, &["start", &target]);
    assert_eq!(
        env.json(&project, &["show", &target])["task"]["status"],
        "doing"
    );
}

#[test]
fn halt_override_records_authority_and_target_before_claimed_start() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(
        &side,
        &["start", &target, "--force", "--reason", "emergency"],
    );
    let authority = env.json(&main, &["show", &halt]);
    let local = env.json(&side, &["show", &target]);
    assert_eq!(local["task"]["status"], "doing");
    assert!(
        authority
            .to_string()
            .contains(&format!("halt override: attempted {target}"))
    );
    assert!(
        local
            .to_string()
            .contains(&format!("halt override: started past {halt}"))
    );
    assert!(env.claim_store("sci").exists());
}

#[test]
fn halt_start_equal_priority_is_allowed_and_force_reason_is_validated() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Incident", "-p", "1", "--tag", "halt"]);
    let equal = id_of(env.json(&dir, &["add", "Equal", "-p", "1"]));
    let lower = id_of(env.json(&dir, &["add", "Lower", "-p", "3"]));
    env.json(&dir, &["start", &equal]);
    assert_eq!(env.fail(&dir, &["start", &lower]), "halted");
    assert_eq!(env.fail(&dir, &["start", &lower, "--force"]), "validation");
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--reason", "urgent"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--force", "--reason", "  "]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--force", "--reason", "line\ntwo"]),
        "validation"
    );
    assert_eq!(env.json(&dir, &["show", &lower])["task"]["status"], "todo");
    env.json(&dir, &["start", &lower, "--force", "--reason", "urgent"]);
    assert_eq!(env.json(&dir, &["show", &lower])["task"]["status"], "doing");
}

#[test]
fn halt_start_multihalt_error_names_urgent_blockers_in_order() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let p2 = id_of(env.json(&dir, &["add", "P2", "-p", "2", "--tag", "halt"]));
    let p0 = id_of(env.json(&dir, &["add", "P0", "-p", "0", "--tag", "halt"]));
    let target = id_of(env.json(&dir, &["add", "P3", "-p", "3"]));
    let error = error_of(&env, &dir, &["start", &target]);
    assert_eq!(error["error"]["kind"], "halted");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.find(&p0).unwrap() < detail.find(&p2).unwrap(),
        "{detail}"
    );
    assert!(detail.contains("--force --reason"));
}

#[test]
fn halt_override_invalid_transition_leaves_no_attempted_note() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&side, &["drop", &target, "no longer wanted"]);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert_eq!(
        env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]),
        "invalid_transition"
    );
    assert!(
        !env.json(&main, &["show", &halt])
            .to_string()
            .contains("halt override:")
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "dropped"
    );
}

#[test]
fn halt_override_identity_failure_leaves_no_attempted_note() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    let state = relay_on(&env);
    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":2,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {target} --force --reason urgent\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&side, env.home.path(), "codex", &script);
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    assert!(
        !env.json(&main, &["show", &halt])
            .to_string()
            .contains("halt override:")
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_override_failed_target_write_keeps_attempted_note_and_previous_claim() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf, std::fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&side, &["start", &target]);
    let task_path = side.join(format!("tasks/{target}.md"));
    let raw = std::fs::read_to_string(&task_path)
        .unwrap()
        .replace("status: doing", "status: todo");
    std::fs::write(&task_path, raw).unwrap();
    let previous_claim = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    let tasks_dir = side.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let restore = Restore(tasks_dir, original);
    let kind = env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]);
    drop(restore);
    assert_eq!(kind, "io");
    assert!(
        env.json(&main, &["show", &halt])
            .to_string()
            .contains(&format!("halt override: attempted {target}"))
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert_eq!(
        std::fs::read_to_string(env.claim_store("sci")).unwrap(),
        previous_claim
    );
}

#[test]
fn halt_start_unused_reason_warns_and_multiline_reason_is_refused_without_halt() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let target = id_of(env.json(&dir, &["add", "Ordinary"]));
    assert_eq!(
        env.fail(
            &dir,
            &["start", &target, "--force", "--reason", "two\nlines"]
        ),
        "validation"
    );
    let started = env.json(
        &dir,
        &["start", &target, "--force", "--reason", "prepared override"],
    );
    assert!(
        warnings_of(&started)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
}

#[test]
fn halt_start_force_resume_requires_reason_even_when_allowed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let target = id_of(env.json(&dir, &["add", "Ordinary"]));
    env.json(&dir, &["start", &target]);
    env.json(&dir, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert_eq!(env.fail(&dir, &["start", &target, "--force"]), "validation");
    let resumed = env.json(&dir, &["start", &target, "--force", "--reason", "resuming"]);
    assert!(
        warnings_of(&resumed)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
}

#[test]
fn halt_start_blocks_blocked_and_recurring_done_new_starts() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let blocked = id_of(env.json(&dir, &["add", "Blocked", "-p", "3"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    let recurring = id_of(env.json(&dir, &["add", "Sweep", "-p", "3", "--every", "1d"]));
    env.json(&dir, &["start", &recurring]);
    env.json(&dir, &["done", &recurring, "first pass"]);
    env.json(&dir, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert_eq!(env.fail(&dir, &["start", &blocked]), "halted");
    assert_eq!(env.fail(&dir, &["start", &recurring]), "halted");
    assert_eq!(
        env.json(&dir, &["show", &blocked])["task"]["status"],
        "blocked"
    );
    assert_eq!(
        env.json(&dir, &["show", &recurring])["task"]["status"],
        "done"
    );
}

#[test]
fn halt_start_work_for_less_urgent_halt_is_allowed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(
        &dir,
        &["add", "Urgent incident", "-p", "0", "--tag", "halt"],
    );
    let p2 = id_of(env.json(&dir, &["add", "Other incident", "-p", "2", "--tag", "halt"]));
    let remedy = id_of(env.json(&dir, &["add", "Remedy", "-p", "3", "--parent", &p2]));
    env.json(&dir, &["start", &remedy]);
    assert_eq!(
        env.json(&dir, &["show", &remedy])["task"]["status"],
        "doing"
    );
}

#[test]
fn halt_start_prepared_override_warns_when_authority_halt_lifts() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    env.json(&main, &["done", &halt, "fixed"]);
    let started = env.json(
        &side,
        &["start", &target, "--force", "--reason", "prepared"],
    );
    assert!(
        warnings_of(&started)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
    assert!(
        !env.json(&main, &["show", &halt])
            .to_string()
            .contains("halt override:")
    );
}

#[test]
fn halt_views_registered_incident_filters_entry_rows_and_names_missing_halt() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    for command in ["prime", "ready", "next"] {
        let output = env.json(&side, &[command]);
        assert_eq!(output["halts"][0]["id"], halt, "{command}: {output}");
        let warnings = warnings_of(&output);
        assert_eq!(
            warnings
                .iter()
                .filter(|warning| warning.contains("hidden by halt"))
                .count(),
            1,
            "{command}: {warnings:?}"
        );
        assert!(
            env.pretty(&side, &[command]).starts_with("halt:"),
            "{command}"
        );
        if command == "next" {
            assert!(output["next"].is_null(), "{output}");
        } else if command == "ready" {
            assert!(
                output["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["id"] != target)
            );
        } else {
            assert!(
                output["ready"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["id"] != target)
            );
        }
    }
    assert!(env.pretty(&side, &["next"]).contains("registered checkout"));
}

#[test]
fn halt_views_unhalted_json_omits_metadata() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Ordinary"]);
    for command in ["prime", "ready", "next"] {
        assert!(
            env.json(&dir, &[command]).get("halts").is_none(),
            "{command}"
        );
    }
}

#[test]
fn halt_views_all_projects_filter_only_the_halted_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let sci_task = id_of(env.json(&sci, &["add", "Sci work", "-p", "2"]));
    let fam_task = id_of(env.json(&fam, &["add", "Fam work", "-p", "2"]));
    let halt = id_of(env.json(&sci, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(&sci, &["shelve", &halt, "--defer", "30d", "pending"]);
    for command in ["ready", "prime", "next"] {
        let output = env.json(&fam, &[command, "--all-projects"]);
        assert_eq!(output["halts"][0]["id"], halt, "{output}");
        let rows = if command == "prime" {
            &output["ready"]
        } else if command == "ready" {
            &output["tasks"]
        } else {
            &output["next"]
        };
        assert!(rows.to_string().contains(&fam_task), "{output}");
        assert!(!rows.to_string().contains(&sci_task), "{output}");
    }
}

#[test]
fn halt_views_unreadable_authority_warns_and_keeps_local_rows() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    std::fs::remove_file(main.join("tasks/.config.toml")).unwrap();
    for command in ["ready", "prime", "next"] {
        let output = env.json(&side, &[command]);
        assert!(output.get("halts").is_none());
        assert!(output.to_string().contains(&target), "{output}");
        assert!(
            warnings_of(&output)
                .iter()
                .any(|warning| warning.contains("halt state unknown"))
        );
    }
}

#[test]
fn halt_views_claimed_shelved_and_deferred_halts_stay_visible() {
    let mut env = TestEnv::new();
    let (main, side, _target) = repo_with_worktree(&mut env);
    let claimed = id_of(env.json(&main, &["add", "Claimed", "-p", "0", "--tag", "halt"]));
    let shelved = id_of(env.json(&main, &["add", "Shelved", "-p", "1", "--tag", "halt"]));
    let deferred = id_of(env.json(
        &main,
        &[
            "add",
            "Deferred",
            "-p",
            "2",
            "--tag",
            "halt",
            "--defer",
            "2099-01-01",
        ],
    ));
    env.json(&main, &["start", &claimed]);
    env.json(&main, &["shelve", &shelved, "--defer", "30d", "later"]);
    let ids: Vec<String> = env.json(&side, &["prime"])["halts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![claimed, shelved, deferred]);
}

#[test]
fn halt_views_parked_todo_is_hidden_but_parked_doing_resumes() {
    let mut env = TestEnv::new();
    let (main, side, todo) = repo_with_worktree(&mut env);
    env.json(&side, &["park", &todo, "continue"]);
    env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert!(env.json(&side, &["next"])["next"].is_null());
    let doing = id_of(env.json(&side, &["add", "Already doing", "-p", "2"]));
    env.json(&side, &["start", &doing, "--force", "--reason", "initial"]);
    env.json(&side, &["park", &doing, "resume"]);
    assert_eq!(env.json(&side, &["next"])["next"]["task"]["id"], doing);
}

#[test]
fn halt_views_ready_limit_applies_after_halt_filter() {
    let mut env = TestEnv::new();
    let (main, side, hidden) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    let local_halt = side.join(format!("tasks/{halt}.md"));
    std::fs::copy(main.join(format!("tasks/{halt}.md")), &local_halt).unwrap();
    let remedy = id_of(env.json(&side, &["add", "Remedy", "-p", "3", "--parent", &halt]));
    std::fs::remove_file(local_halt).unwrap();
    let output = env.json(&side, &["ready", "-n", "1"]);
    assert_eq!(output["tasks"][0]["id"], remedy);
    assert_eq!(output["tasks"].as_array().unwrap().len(), 1);
    assert!(!output["tasks"].to_string().contains(&hidden));
    assert!(
        warnings_of(&output)
            .iter()
            .any(|warning| warning.starts_with("1 ready task(s) hidden by halt"))
    );
}

#[test]
fn halt_views_all_projects_unreachable_project_reports_unknown_state() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let output = env.json(&sci, &["ready", "--all-projects"]);
    assert!(output["tasks"].to_string().contains(&local));
    assert!(
        warnings_of(&output)
            .iter()
            .any(|warning| warning.contains("fam: halt state unknown"))
    );
}

#[test]
fn group_halt_views_filter_members_and_warn_only_about_members() {
    let mut env = TestEnv::new();
    let (sci, fam, ops) = grouped_projects(&mut env);
    let sci_task = id_of(env.json(&sci, &["add", "Sci work", "-p", "2"]));
    let fam_task = id_of(env.json(&fam, &["add", "Fam work", "-p", "2"]));
    let halt = id_of(env.json(&sci, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(&sci, &["shelve", &halt, "--defer", "30d", "pending"]);
    std::fs::remove_file(ops.join("tasks/.config.toml")).unwrap();
    let about_ops = |w: &String| w.starts_with("ops:") || w.starts_with("project ops ");
    for command in ["ready", "prime", "next"] {
        let output = env.json(&fam, &[command, "--group", "vf"]);
        assert_eq!(output["halts"][0]["id"], halt, "{output}");
        let rows = match command {
            "prime" => &output["ready"],
            "ready" => &output["tasks"],
            _ => &output["next"],
        };
        assert!(rows.to_string().contains(&fam_task), "{output}");
        assert!(!rows.to_string().contains(&sci_task), "{output}");
        assert!(
            !warnings_of(&output).iter().any(about_ops),
            "{command}: {output}"
        );
        let wide = env.json(&fam, &[command, "--all-projects"]);
        assert!(
            warnings_of(&wide)
                .iter()
                .any(|w| w == "ops: halt state unknown (registered checkout unreachable)"),
            "{command}: {wide}"
        );
    }
}
