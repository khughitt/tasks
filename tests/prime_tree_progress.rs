//! Integration coverage for `prime` and `tree` goal progress metadata, format routing,
//! warning deduplication, and pretty rendering (spec 2026-11-17-prime-tree-goal-progress-design).

mod common;

use common::{TestEnv, id_of, write_claim};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn add(env: &TestEnv, dir: &Path, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["add", title];
    args.extend_from_slice(extra);
    id_of(env.json(dir, &args))
}

/// Reusable claimed-checkout fixture seeded in Task 2 and rendered end-to-end in Task 3:
/// `main_aa` is the registered `"aa"` checkout; `other_wt` is an unmerged `"aa"` worktree
/// holding a live claimed doing goal with an extra child not present in `main_aa`.
pub struct ClaimedCheckoutFixture {
    pub env: TestEnv,
    pub main_aa: PathBuf,
    pub bb: PathBuf,
    pub other_wt: PathBuf,
    pub shared_goal: String,
    pub claimed_goal: String,
}

pub fn build_claimed_checkout_fixture() -> ClaimedCheckoutFixture {
    let mut env = TestEnv::new();
    let main_aa = env.init("aa");
    let bb = env.init("bb");
    let bb_done = add(&env, &bb, "Foreign done piece", &[]);
    env.json(&bb, &["done", &bb_done]);

    let shared_goal = add(&env, &main_aa, "Shared goal", &["--origin", "user"]);
    let done_step = add(&env, &main_aa, "Done step", &["--parent", &shared_goal]);
    env.json(&main_aa, &["done", &done_step]);
    let _open_step = add(
        &env,
        &main_aa,
        "Main open step",
        &["--parent", &shared_goal],
    );

    // Copy main_aa into a separate checkout directory without changing the registered root.
    let other_tmp = tempfile::tempdir().unwrap();
    let other_wt = other_tmp.path().join("other-aa");
    std::fs::create_dir_all(other_wt.join("tasks")).unwrap();
    for entry in std::fs::read_dir(main_aa.join("tasks")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), other_wt.join("tasks").join(entry.file_name())).unwrap();
        }
    }
    let other_wt = other_wt.canonicalize().unwrap();
    // Keep tempdir alive inside TestEnv's home so it lives as long as env.
    let persisted = env.home.path().join("other-aa");
    std::fs::rename(&other_wt, &persisted).unwrap();
    let other_wt = persisted.canonicalize().unwrap();

    // In other_wt, create a branch-only doing goal with local children, an extra child on
    // shared_goal, and a cross-project piece edit not present in main_aa.
    let claimed_goal = add(
        &env,
        &other_wt,
        "Claimed branch goal",
        &["--origin", "user"],
    );
    let branch_done = add(&env, &other_wt, "Branch done", &["--parent", &claimed_goal]);
    env.json(&other_wt, &["done", &branch_done]);
    let _branch_todo_1 = add(
        &env,
        &other_wt,
        "Branch todo 1",
        &["--parent", &claimed_goal],
    );
    let _branch_todo_2 = add(
        &env,
        &other_wt,
        "Branch todo 2",
        &["--parent", &claimed_goal],
    );

    let _extra_shared = add(
        &env,
        &other_wt,
        "Branch-only shared step",
        &["--parent", &shared_goal],
    );
    env.json(
        &other_wt,
        &["edit", &shared_goal, "--parent", &claimed_goal],
    );
    env.json(&other_wt, &["edit", &claimed_goal, "--piece", &bb_done]);

    write_claim(&env, "aa", &claimed_goal, "codex:other-session", true);
    let store_path = env.claim_store("aa");
    let mut store: toml::Value =
        toml::from_str(&std::fs::read_to_string(&store_path).unwrap()).unwrap();
    store["claims"][&claimed_goal]
        .as_table_mut()
        .unwrap()
        .insert(
            "worktree".into(),
            toml::Value::String(other_wt.display().to_string()),
        );
    std::fs::write(&store_path, toml::to_string(&store).unwrap()).unwrap();

    ClaimedCheckoutFixture {
        env,
        main_aa,
        bb,
        other_wt,
        shared_goal,
        claimed_goal,
    }
}

fn assert_summary_has_no_work_keys(summary: &Value) {
    let obj = summary.as_object().expect("TaskSummary object");
    assert!(
        !obj.contains_key("pieces"),
        "TaskSummary must not serialize pieces: {summary}"
    );
    assert!(
        !obj.contains_key("goal_work"),
        "TaskSummary must not serialize goal_work: {summary}"
    );
}

fn check_tree_nodes(nodes: &[Value]) {
    for node in nodes {
        assert_summary_has_no_work_keys(node);
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            check_tree_nodes(children);
        }
    }
}

#[test]
fn progress_metadata_json_shapes() {
    let fixture = build_claimed_checkout_fixture();
    let foreign_piece = add(&fixture.env, &fixture.bb, "Foreign piece", &[]);
    let hub = add(
        &fixture.env,
        &fixture.main_aa,
        "Piece hub",
        &["--origin", "user", "--piece", &foreign_piece],
    );
    let lane = add(
        &fixture.env,
        &fixture.main_aa,
        "Main lane",
        &["--lane", "--origin", "user"],
    );
    let _lane_step = add(
        &fixture.env,
        &fixture.main_aa,
        "Lane step",
        &["--parent", &lane],
    );
    let close_goal = add(&fixture.env, &fixture.main_aa, "Closeout goal", &[]);
    let close_child = add(
        &fixture.env,
        &fixture.main_aa,
        "Closeout child",
        &["--parent", &close_goal],
    );
    fixture.env.json(&fixture.main_aa, &["done", &close_child]);

    // Task record on show keeps `pieces`, and GoalRow on goals keeps `pieces`.
    assert_eq!(
        fixture.env.json(&fixture.main_aa, &["show", &hub])["task"]["pieces"],
        json!([foreign_piece])
    );
    let goals_out = fixture
        .env
        .json(&fixture.main_aa, &["goals", "--all-projects"]);
    let hub_goal = goals_out["goals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == hub)
        .unwrap();
    assert_eq!(hub_goal["pieces"], json!({"done": 0, "total": 1}));

    // Prime JSON: exact top-level keys, zero TaskSummary pieces/goal_work keys.
    let prime = fixture
        .env
        .json(&fixture.main_aa, &["prime", "--all-projects"]);
    let prime_keys: BTreeSet<&str> = prime
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        prime_keys,
        BTreeSet::from([
            "closeout", "counts", "deferred", "doing", "lanes", "parked", "periodic", "prefix",
            "projects", "ready", "roadmap", "warnings",
        ])
    );
    for section in ["ready", "doing", "closeout"] {
        for row in prime[section].as_array().unwrap() {
            assert_summary_has_no_work_keys(row);
        }
    }
    check_tree_nodes(prime["roadmap"].as_array().unwrap());
    for lane_row in prime["lanes"].as_array().unwrap() {
        assert_summary_has_no_work_keys(&lane_row["lane"]);
        if let Some(pick) = lane_row.get("pick") {
            assert_summary_has_no_work_keys(pick);
        }
    }

    // Tree JSON: exact top-level keys, zero TaskSummary pieces/goal_work keys.
    let tree = fixture
        .env
        .json(&fixture.main_aa, &["tree", "--all-projects", "--all"]);
    let tree_keys: BTreeSet<&str> = tree
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(tree_keys, BTreeSet::from(["nodes", "warnings"]));
    check_tree_nodes(tree["nodes"].as_array().unwrap());

    // Prime --summary size-limiting and recovery command behavior unchanged:
    // add tasks with long titles to exceed the 12,288-byte summary budget.
    for i in 0..35 {
        let _ = add(
            &fixture.env,
            &fixture.main_aa,
            &format!("Bulk summary leaf {i:02} {}", "x".repeat(220)),
            &[],
        );
    }
    let summary_out = fixture
        .env
        .cmd(&fixture.main_aa)
        .args(["prime", "--all-projects", "--summary"])
        .output()
        .unwrap();
    assert!(summary_out.status.success(), "{summary_out:?}");
    assert!(
        summary_out.stdout.len() <= 12_288,
        "len={} > 12_288",
        summary_out.stdout.len()
    );
    let summary_json: Value = serde_json::from_slice(&summary_out.stdout).unwrap();
    assert_eq!(summary_json["summary"]["byte_limit"], 12_288);
    assert_eq!(summary_json["summary"]["complete"], false);
    let full = summary_json["summary"]["full"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(full.iter().any(|arg| arg == "prime"));
    assert!(!full.iter().any(|arg| arg == "--summary"));
}

#[test]
fn progress_metadata_local_prereq_isolation() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bad = env.init("bad");
    let foreign_dep = add(&env, &bad, "Foreign prerequisite", &[]);

    let goal = add(&env, &aa, "Active goal", &["--origin", "user"]);
    let closed_child = add(&env, &aa, "Closed child", &["--parent", &goal]);
    env.json(&aa, &["dep", &closed_child, "--on", &foreign_dep]);
    env.json(&bad, &["done", &foreign_dep]);
    env.json(&aa, &["done", &closed_child]);
    let _open_child = add(&env, &aa, "Open child", &["--parent", &goal]);

    let paused_lane = add(&env, &aa, "Paused lane", &["--lane", "--origin", "user"]);
    let paused_step = add(&env, &aa, "Paused step", &["--parent", &paused_lane]);
    let bad_open = add(&env, &bad, "Second foreign prereq", &[]);
    env.json(&aa, &["dep", &paused_step, "--on", &bad_open]);
    env.json(&aa, &["block", &paused_step, "step blocked"]);
    env.json(&aa, &["block", &paused_lane, "waiting"]);

    // Corrupt foreign project config: ordinary prerequisites on closed descendants and paused
    // lanes must never be opened by local prime or tree in either JSON or pretty mode.
    std::fs::write(bad.join("tasks/.config.toml"), "not valid toml = [").unwrap();

    for args in [
        &["prime", "--json"][..],
        &["prime", "--pretty"][..],
        &["tree", "--json"][..],
        &["tree", "--pretty"][..],
    ] {
        let out = env.cmd(&aa).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "args={args:?} failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn progress_metadata_format_routing() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let unreg = env.init("unreg");
    let piece = add(&env, &unreg, "Piece in soon-unregistered project", &[]);
    let hub = add(
        &env,
        &aa,
        "Hub with unregistered piece",
        &["--origin", "user", "--piece", &piece],
    );
    // Keep an open local child so closeout does not walk depends in JSON mode; only
    // Membership (pretty mode) opens declared pieces!
    let _open_local = add(&env, &aa, "Open local child", &["--parent", &hub]);
    env.json(&aa, &["unregister", "unreg"]);

    // --json: no Membership traversal, so no unregistered-project warning.
    for cmd in ["prime", "tree"] {
        let json_out = env.json(&aa, &[cmd, "--json"]);
        let warns = json_out["warnings"].as_array().unwrap();
        assert!(
            !warns.iter().any(|w| w
                .as_str()
                .unwrap()
                .contains("project unreg is not registered")),
            "{cmd} --json should not run pretty membership: {warns:?}"
        );
    }

    // Explicit --pretty triggers Membership and emits the unregistered-project warning.
    for cmd in ["prime", "tree"] {
        let out = env.cmd(&aa).args([cmd, "--pretty"]).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("project unreg is not registered on this host"),
            "{cmd} --pretty missing membership warning: {stderr}"
        );
    }

    // TASKS_FORMAT=pretty without --pretty flag routes Format::Pretty identically!
    for cmd in ["prime", "tree"] {
        let out = env
            .cmd(&aa)
            .env("TASKS_FORMAT", "pretty")
            .args([cmd])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("project unreg is not registered on this host"),
            "TASKS_FORMAT=pretty {cmd} missing membership warning: {stderr}"
        );
    }

    // TASKS_FORMAT=pretty overridden by --json stays Format::Json (no membership read).
    let override_out = env
        .cmd(&aa)
        .env("TASKS_FORMAT", "pretty")
        .args(["prime", "--json"])
        .output()
        .unwrap();
    assert!(override_out.status.success(), "{override_out:?}");
    let parsed: Value = serde_json::from_slice(&override_out.stdout).unwrap();
    assert!(
        !parsed["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("project unreg is not registered")),
        "{parsed}"
    );
}

#[test]
fn progress_metadata_warnings_dedup() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let gone = env.init("gone");
    let foreign = add(&env, &gone, "Soon unreachable", &[]);
    let hub = add(
        &env,
        &aa,
        "Cross-project hub",
        &["--origin", "user", "--piece", &foreign],
    );
    let _keep_open = add(&env, &aa, "Local open child", &["--parent", &hub]);

    // Remove gone's config so both Scope::All and Membership encounter an unreachable project.
    std::fs::remove_file(gone.join("tasks/.config.toml")).unwrap();

    for cmd in ["prime", "tree"] {
        let out = env
            .cmd(&aa)
            .args([cmd, "--all-projects", "--pretty"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let needle = format!("project gone at {} is unreachable", gone.display());
        let occurrences = stderr.matches(&needle).count();
        assert_eq!(
            occurrences, 1,
            "{cmd} --all-projects --pretty should emit unreachable warning once, got {occurrences}:\n{stderr}"
        );
    }
}
