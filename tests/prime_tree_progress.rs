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

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for t in chars.by_ref() {
                if t.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[test]
fn progress_display_members_match_goals() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");

    let f_done = add(&env, &bb, "Foreign done piece", &[]);
    env.json(&bb, &["done", &f_done]);
    let f_idea = add(&env, &bb, "Foreign idea piece", &["--status", "idea"]);

    let root = add(
        &env,
        &aa,
        "Umbrella goal",
        &["--origin", "user", "--piece", &f_done, "--piece", &f_idea],
    );
    let hidden_done = add(&env, &aa, "Hidden completed child", &["--parent", &root]);
    env.json(&aa, &["done", &hidden_done]);

    let closed_anc = add(
        &env,
        &aa,
        "Closed ancestor goal",
        &["--origin", "user", "--parent", &root],
    );
    let open_grand = add(&env, &aa, "Open grandchild", &["--parent", &closed_anc]);
    env.json(&aa, &["done", &open_grand]);
    env.json(&aa, &["done", &closed_anc]);
    env.json(&aa, &["edit", &open_grand, "--status", "todo"]);

    let goals_out = env.json(&aa, &["goals", "--all-projects", "--closed"]);
    let goal_cell = |id: &str| -> String {
        let g = goals_out["goals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let p = &g["pieces"];
        let done = p["done"].as_u64().unwrap();
        let total = p["total"].as_u64().unwrap();
        let ideas = p.get("ideas").and_then(Value::as_u64).unwrap_or(0);
        let mut cell = if total == 0 {
            "-".to_string()
        } else {
            format!("{done}/{total}")
        };
        if ideas > 0 {
            cell.push_str(&format!(" +{ideas}"));
        }
        cell
    };
    let expected_root = goal_cell(&root);
    let expected_anc = goal_cell(&closed_anc);
    // root has: f_done (Done), f_idea (Idea), hidden_done (Done), closed_anc (Done), open_grand (Todo) -> 3/4 +1
    assert_eq!(expected_root, "3/4 +1");
    // closed_anc has: open_grand (Todo) -> 0/1
    assert_eq!(expected_anc, "0/1");

    let prime_pretty = env.pretty(&aa, &["prime"]);
    let root_line = prime_pretty.lines().find(|l| l.contains(&root)).unwrap();
    let anc_line = prime_pretty
        .lines()
        .find(|l| l.contains(&closed_anc))
        .unwrap();
    assert!(root_line.contains(&expected_root), "{root_line}");
    assert!(anc_line.contains(&expected_anc), "{anc_line}");
    // Hidden completed child was filtered from tree display, so closed_anc is the only displayed
    // child of root (`└─ `, not `├─ `), preceded by the 2-cell review prefix reserved in prime.
    assert!(
        anc_line.starts_with(&format!("  └─ {closed_anc}")),
        "{anc_line}"
    );

    let tree_default = env.pretty(&aa, &["tree"]);
    assert!(
        tree_default
            .lines()
            .find(|l| l.contains(&root))
            .unwrap()
            .contains(&expected_root),
        "{tree_default}"
    );
    let tree_all = env.pretty(&aa, &["tree", "--all"]);
    assert!(
        tree_all
            .lines()
            .find(|l| l.contains(&root))
            .unwrap()
            .contains(&expected_root),
        "{tree_all}"
    );

    // Selected root starts at depth zero (`""` tree prefix, after the 2-cell review column)
    // and keeps identical work counts!
    let selected = env.pretty(&aa, &["tree", &closed_anc]);
    let sel_line = selected.lines().find(|l| l.contains(&closed_anc)).unwrap();
    assert!(
        sel_line.starts_with(&format!("  {closed_anc}")),
        "{sel_line}"
    );
    assert!(sel_line.contains(&expected_anc), "{sel_line}");
    assert!(
        selected
            .lines()
            .any(|l| l.starts_with(&format!("* └─ {open_grand}"))),
        "{selected}"
    );
}

#[test]
fn progress_display_goal_kinds() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let unreg = env.init("unreg");

    // 1. Childless lane -> `-`
    let empty_lane = add(&env, &aa, "Empty lane", &["--lane"]);

    // 2. Childless piece hub with missing piece -> `- ?1`, and ideas + missing piece -> `- +3 ?1`
    let missing_piece = add(&env, &unreg, "Vanishing piece", &[]);
    let missing_hub = add(
        &env,
        &aa,
        "Missing piece hub",
        &["--origin", "agent", "--piece", &missing_piece],
    );
    let ideas_missing_goal = add(
        &env,
        &aa,
        "Ideas and missing goal",
        &["--origin", "mixed", "--piece", &missing_piece],
    );
    for idx in 1..=3 {
        add(
            &env,
            &aa,
            &format!("Idea {idx}"),
            &["--status", "idea", "--parent", &ideas_missing_goal],
        );
    }
    std::fs::remove_file(unreg.join(format!("tasks/{missing_piece}.md"))).unwrap();

    // 3. Excluded statuses (shelved, dropped, moved) do not enter done/total/ideas -> `- +1`
    let cc = env.init("cc");
    let mv_piece = add(&env, &bb, "Moved foreign piece", &[]);
    let excluded_goal = add(&env, &aa, "Excluded children goal", &["--piece", &mv_piece]);
    let sh = add(&env, &aa, "Shelved kid", &["--parent", &excluded_goal]);
    env.json(&aa, &["shelve", &sh, "later", "--defer", "30d"]);
    let dr = add(&env, &aa, "Dropped kid", &["--parent", &excluded_goal]);
    env.json(&aa, &["drop", &dr, "not needed"]);
    let moved_out = env.json(&bb, &["move", &mv_piece, "--to", "cc"]);
    let successor = moved_out["id"].as_str().unwrap();
    env.json(&cc, &["drop", successor, "abandoned after move"]);
    let _open_keep = add(
        &env,
        &aa,
        "Idea only kid",
        &["--status", "idea", "--parent", &excluded_goal],
    );

    // 4. Closeout + Doing goal occurrences in prime
    let close_goal = add(&env, &aa, "Closeout ready goal", &["--origin", "user"]);
    let close_kid = add(&env, &aa, "Done kid", &["--parent", &close_goal]);
    env.json(&aa, &["done", &close_kid]);

    let doing_goal = add(&env, &aa, "Doing active goal", &[]);
    let doing_kid = add(&env, &aa, "Open doing kid", &["--parent", &doing_goal]);
    let _ = doing_kid;
    env.json(&aa, &["start", &doing_goal]);

    let _ = bb;
    let prime_pretty = env.pretty(&aa, &["prime"]);

    // Verify exact cells across sections:
    assert!(
        prime_pretty
            .lines()
            .any(|l| l.contains(&empty_lane) && l.contains(" - ")),
        "{prime_pretty}"
    );
    assert!(
        prime_pretty
            .lines()
            .any(|l| l.contains(&missing_hub) && l.contains("- ?1")),
        "{prime_pretty}"
    );
    assert!(
        prime_pretty
            .lines()
            .any(|l| l.contains(&ideas_missing_goal) && l.contains("- +3 ?1")),
        "{prime_pretty}"
    );
    assert!(
        prime_pretty
            .lines()
            .any(|l| l.contains(&excluded_goal) && l.contains("- +1")),
        "{prime_pretty}"
    );
    // close_goal in closeout: 1/1; doing_goal in both roadmap and doing: 0/1
    let close_hits: Vec<&str> = prime_pretty
        .lines()
        .filter(|l| l.contains(&close_goal) && l.contains("1/1"))
        .collect();
    assert!(!close_hits.is_empty(), "{prime_pretty}");
    let doing_hits: Vec<&str> = prime_pretty
        .lines()
        .filter(|l| l.contains(&doing_goal) && l.contains("0/1"))
        .collect();
    assert!(
        doing_hits.len() >= 2,
        "doing goal should show 0/1 in both roadmap and doing:\n{prime_pretty}"
    );
}

#[test]
fn progress_display_elsewhere_uses_source_scan() {
    let fixture = build_claimed_checkout_fixture();
    let prime_pretty = fixture.env.pretty(&fixture.main_aa, &["prime"]);

    // In main_aa's scan, shared_goal has 1 done + 1 todo = 1/2.
    let shared_line = prime_pretty
        .lines()
        .find(|l| l.contains(&fixture.shared_goal))
        .expect("shared_goal in roadmap");
    assert!(shared_line.contains("1/2"), "{shared_line}\n{prime_pretty}");

    // In other_wt's scan, claimed_goal has 3 done + 5 todo = 3/8 (including shared_goal's
    // branch-only extra child and claimed_goal's branch-only foreign piece).
    let claimed_line = prime_pretty
        .lines()
        .find(|l| l.contains(&fixture.claimed_goal))
        .expect("claimed_goal in doing");
    assert!(
        claimed_line.contains("3/8"),
        "{claimed_line}\n{prime_pretty}"
    );
}

#[test]
fn progress_display_elsewhere_shared_root_detached() {
    let mut env = TestEnv::new();
    let main_aa = env.init("aa");
    let shared_code = env.home.path().join("shared-code");
    std::fs::create_dir_all(&shared_code).unwrap();
    let shared_code = shared_code.canonicalize().unwrap();

    let det_1 = env.home.path().join("det-rec-1");
    let det_2 = env.home.path().join("det-rec-2");
    for d in [&det_1, &det_2] {
        std::fs::create_dir_all(d.join("tasks")).unwrap();
        std::fs::write(d.join("tasks/.config.toml"), "prefix = \"aa\"\n").unwrap();
    }

    // Populate det_1 with claimed goal_1 (1/2) and claimed goal_1b (0/1).
    let goal_1 = add(&env, &det_1, "Detached 1 goal", &["--origin", "user"]);
    let g1_done = add(&env, &det_1, "Det 1 done", &["--parent", &goal_1]);
    env.json(&det_1, &["done", &g1_done]);
    let _g1_todo = add(&env, &det_1, "Det 1 todo", &["--parent", &goal_1]);

    let goal_1b = add(
        &env,
        &det_1,
        "Detached 1 second goal",
        &["--origin", "user"],
    );
    let _g1b_todo = add(&env, &det_1, "Det 1b todo", &["--parent", &goal_1b]);

    // Populate det_2 with claimed goal_2 (2/3), which does NOT exist in det_1.
    let goal_2 = add(&env, &det_2, "Detached 2 goal", &["--origin", "user"]);
    let g2_done_a = add(&env, &det_2, "Det 2 done A", &["--parent", &goal_2]);
    let g2_done_b = add(&env, &det_2, "Det 2 done B", &["--parent", &goal_2]);
    env.json(&det_2, &["done", &g2_done_a]);
    env.json(&det_2, &["done", &g2_done_b]);
    let _g2_todo = add(&env, &det_2, "Det 2 todo", &["--parent", &goal_2]);

    // Now mark both det_1 and det_2 as Kind::Detached checkouts sharing [checkout] path = shared_code!
    let detached_cfg = format!(
        "prefix = \"aa\"\n\n[checkout]\npath = \"{}\"\nkind = \"detached\"\n",
        shared_code.display()
    );
    for d in [&det_1, &det_2] {
        std::fs::write(d.join("tasks/.config.toml"), &detached_cfg).unwrap();
    }

    let symlink_det_1 = env.home.path().join("symlink-det-1");
    std::os::unix::fs::symlink(det_1.canonicalize().unwrap(), &symlink_det_1).unwrap();

    for id in [&goal_1, &goal_1b, &goal_2] {
        write_claim(&env, "aa", id, "codex:other-session", true);
    }
    let store_path = env.claim_store("aa");
    let mut store: toml::Value =
        toml::from_str(&std::fs::read_to_string(&store_path).unwrap()).unwrap();
    for (id, wt) in [
        (&goal_1, &symlink_det_1),
        (&goal_1b, &symlink_det_1),
        (&goal_2, &det_2),
    ] {
        store["claims"][id].as_table_mut().unwrap().insert(
            "worktree".into(),
            toml::Value::String(wt.display().to_string()),
        );
    }
    std::fs::write(&store_path, toml::to_string(&store).unwrap()).unwrap();

    let prime_json = env.json(&main_aa, &["prime"]);
    let doing_ids: BTreeSet<&str> = prime_json["doing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert!(doing_ids.contains(goal_1.as_str()), "{prime_json}");
    assert!(doing_ids.contains(goal_1b.as_str()), "{prime_json}");
    assert!(doing_ids.contains(goal_2.as_str()), "{prime_json}");

    let prime_pretty = env.pretty(&main_aa, &["prime"]);
    let line_1 = prime_pretty.lines().find(|l| l.contains(&goal_1)).unwrap();
    let line_1b = prime_pretty.lines().find(|l| l.contains(&goal_1b)).unwrap();
    let line_2 = prime_pretty.lines().find(|l| l.contains(&goal_2)).unwrap();
    assert!(line_1.contains("1/2"), "{line_1}\n{prime_pretty}");
    assert!(line_1b.contains("0/1"), "{line_1b}\n{prime_pretty}");
    assert!(line_2.contains("2/3"), "{line_2}\n{prime_pretty}");
}

#[test]
fn progress_display_legends_and_width() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let unreg = env.init("unreg");

    // Empty tree stays byte-empty in output::render (`"\n"` after main's trailing newline, matching baseline).
    assert_eq!(env.pretty(&aa, &["tree"]), "\n");

    // Leaves-only views emit no legend.
    let leaf = add(&env, &aa, "Standalone leaf", &[]);
    let leaves_tree = env.pretty(&aa, &["tree"]);
    assert!(!leaves_tree.contains("work: done/total"), "{leaves_tree}");
    let leaves_prime = env.pretty(&aa, &["prime"]);
    assert!(!leaves_prime.contains("work: done/total"), "{leaves_prime}");

    // Base legend (goal with committed work only, no ideas, no missing pieces):
    let goal = add(&env, &aa, "Plain goal", &[]);
    env.json(&aa, &["edit", &leaf, "--parent", &goal]);
    let base = "work: done/total local descendants and declared pieces; tree: local containment";

    let tree_base = env.pretty(&aa, &["tree"]);
    assert!(tree_base.starts_with(&format!("{base}\n")), "{tree_base:?}");
    assert!(!tree_base.contains("+N: ideas"), "{tree_base}");
    assert!(
        !tree_base.contains("?N: unresolved declared pieces"),
        "{tree_base}"
    );

    let prime_base = env.pretty(&aa, &["prime"]);
    let lines: Vec<&str> = prime_base.lines().collect();
    assert_eq!(lines[0], "project aa");
    assert!(lines[1].starts_with("idea "), "{prime_base}");
    assert_eq!(lines[2], base);
    assert_eq!(lines[3], "");
    assert_eq!(lines[4], "closeout:");

    // Add an idea -> appends `; +N: ideas` only.
    let _idea = add(
        &env,
        &aa,
        "Idea child",
        &["--status", "idea", "--parent", &goal],
    );
    let with_ideas = env.pretty(&aa, &["tree"]);
    assert!(
        with_ideas.starts_with(&format!("{base}; +N: ideas\n")),
        "{with_ideas:?}"
    );

    // Add an unresolved piece -> appends `; ?N: unresolved declared pieces`, and wraps <= 80 cols!
    let vanishing = add(&env, &unreg, "Gone piece", &[]);
    env.json(&aa, &["edit", &goal, "--piece", &vanishing]);
    std::fs::remove_file(unreg.join(format!("tasks/{vanishing}.md"))).unwrap();

    for extra_args in [
        &["--pretty", "prime"][..],
        &["--pretty", "--color", "always", "prime"][..],
        &["--pretty", "tree"][..],
        &["--pretty", "--color", "always", "tree"][..],
    ] {
        let out = env
            .cmd(&aa)
            .env("COLUMNS", "80")
            .args(extra_args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let stdout = String::from_utf8(out.stdout).unwrap();
        let clean = strip_ansi(&stdout);
        assert!(clean.contains("+N: ideas"), "{clean}");
        assert!(clean.contains("?N: unresolved declared pieces"), "{clean}");
        for line in clean.lines() {
            assert!(
                line.chars().count() <= 80,
                "exceeded 80 cols ({}) in {extra_args:?}: {line:?}",
                line.chars().count()
            );
        }
    }

    // With `--headers`: headers do not suppress the legend, the legend still wraps within
    // COLUMNS=80 (and carry no header row of its own), and table headers appear on data tables.
    for cmd in ["prime", "tree"] {
        let out = env
            .cmd(&aa)
            .env("COLUMNS", "80")
            .args(["--pretty", "--headers", cmd])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let clean = strip_ansi(&String::from_utf8(out.stdout).unwrap());
        assert!(clean.contains("+N: ideas"), "{clean}");
        assert!(clean.contains("?N: unresolved declared pieces"), "{clean}");
        assert!(
            clean
                .lines()
                .any(|l| l.contains("status") && l.contains("work")),
            "{clean}"
        );
        for line in clean
            .lines()
            .filter(|l| l.starts_with("work:") || l.starts_with("containment") || l.contains("?N:"))
        {
            assert!(
                line.chars().count() <= 80,
                "legend line exceeded 80 cols: {line:?}"
            );
        }
    }
}

#[test]
fn progress_display_empty_fixed_floor() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let long_title =
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron";
    let leaf = add(&env, &aa, long_title, &[]);
    // Mark leaf `doing` so it is unadorned (`needs_review == false`, process `-`):
    // id(9) + P(2) + size(1) + cx(1) + process(1) + status("doing"=5) + work(max(0,1)=1) + date(10) + title(20) + 8*gap(16) = 66.
    env.json(&aa, &["start", &leaf]);
    assert_eq!(leaf.len(), 9);
    let computed_min = 9 + 2 + 1 + 1 + 1 + 5 + 1 + 10 + 20 + 8 * 2;
    assert_eq!(computed_min, 66);

    for width in [66usize, 80usize] {
        for cmd in ["tree", "prime"] {
            let out = env
                .cmd(&aa)
                .env("COLUMNS", width.to_string())
                .args(["--pretty", cmd])
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                text.lines().any(|l| l.contains("omicron")),
                "{cmd} at {width}: {text}"
            );
            for line in text.lines() {
                assert!(
                    line.chars().count() <= width,
                    "{cmd} at COLUMNS={width} overflowed ({} cols): {line:?}\n{text}",
                    line.chars().count()
                );
            }
        }
    }

    // One column below the computed minimum (65 < 66): deliberate Mode::Unwrapped single-line row!
    let unwrapped_tree = env
        .cmd(&aa)
        .env("COLUMNS", "65")
        .args(["--pretty", "tree"])
        .output()
        .unwrap();
    let tree_65 = String::from_utf8(unwrapped_tree.stdout).unwrap();
    assert_eq!(
        tree_65.trim_end().lines().count(),
        1,
        "below floor 66 should render unwrapped: {tree_65:?}"
    );
    assert!(tree_65.contains(long_title), "{tree_65:?}");
}
