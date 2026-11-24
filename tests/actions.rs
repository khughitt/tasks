mod common;

use common::{TestEnv, id_of};
use serde_json::{Value, json};
use std::path::Path;

fn record(dir: &Path, id: &str, status: &str, priority: u8, fields: &str) {
    let depends = if fields.contains("depends:") {
        ""
    } else {
        "depends: []\n"
    };
    std::fs::write(
        dir.join(format!("tasks/{id}.md")),
        format!(
            "---\nid: {id}\ntitle: Fixture {id}\nstatus: {status}\npriority: {priority}\n\
             created: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\n{fields}{depends}tags: []\n---\n"
        ),
    )
    .unwrap();
}

fn action<'a>(row: &'a Value, name: &str) -> &'a Value {
    row["actions"]
        .as_array()
        .unwrap_or_else(|| panic!("actions absent from {row}"))
        .iter()
        .find(|entry| entry["action"] == name)
        .unwrap_or_else(|| panic!("action {name} absent from {row}"))
}

fn ids(rows: &Value) -> Vec<&str> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect()
}

#[test]
fn format_one_selection_characterization() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    record(&dir, "aa-000001", "todo", 2, "process: direct\n");
    record(&dir, "aa-000002", "todo", 2, "process: direct\n");
    record(&dir, "aa-000003", "todo", 1, "depends: [aa-000001]\n");
    record(&dir, "aa-000004", "idea", 3, "");
    record(&dir, "aa-000005", "todo", 4, "process: direct\n");
    record(
        &dir,
        "aa-000007",
        "done",
        2,
        "parent: aa-000005\nclosed: 2020-01-02T00:00:00Z\ncompleted: 2020-01-02T00:00:00Z\n",
    );
    record(
        &dir,
        "aa-000006",
        "done",
        1,
        "every: 1d\ncompleted: 2020-01-02T00:00:00Z\nclosed: 2020-01-02T00:00:00Z\n",
    );

    let ready = env.json(&dir, &["ready"]);
    assert_eq!(
        ids(&ready["tasks"]),
        ["aa-000006", "aa-000001", "aa-000002"]
    );
    let next = env.json(&dir, &["next", "--no-fetch"]);
    assert_eq!(next["next"]["task"]["id"], "aa-000006");
    assert!(next["next"].get("actions").is_none());

    let review = env.json(&dir, &["list", "--needs-review"]);
    assert_eq!(ids(&review["tasks"]), ["aa-000003", "aa-000004"]);
    assert_eq!(review["tasks"][0]["review_reason"], "unassessed");
    assert_eq!(review["tasks"][1]["review_reason"], "unreviewed");

    let sample = env.json(&dir, &["sample", "--seed", "4", "--limit", "20"]);
    let mut sampled = ids(&sample["tasks"]);
    sampled.sort_unstable();
    assert_eq!(
        sampled,
        [
            "aa-000001",
            "aa-000002",
            "aa-000003",
            "aa-000004",
            "aa-000005"
        ]
    );
    assert_eq!(
        sample["warnings"],
        json!([
            "asked for 20; the pool holds 5 (open, unclaimed, not pending, not updated in 7 days)"
        ])
    );
    let prime = env.json(&dir, &["prime"]);
    assert_eq!(ids(&prime["closeout"]), ["aa-000005"]);
}

#[test]
fn actions_show_and_list_explain_picker_holds() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    record(&dir, "aa-000001", "todo", 2, "process: direct\n");
    record(&dir, "aa-000002", "todo", 2, "depends: [aa-000001]\n");
    let show = env.json(&dir, &["show", "aa-000002", "--no-fetch"]);
    assert_eq!(
        action(&show, "pick_up"),
        &json!({"action":"pick_up","actor":"agent","available":false,
                "reasons":[{"code":"dependency_open","evidence":{"task":"aa-000001"}}],
                "signals":[]})
    );
    let curate = action(&show, "curate");
    assert_eq!(curate["reasons"], json!([]));
    assert_eq!(curate["window"], json!({"older_than_days":7}));
    assert!(curate.get("subject").is_none());
    assert!(action(&show, "pick_up").get("window").is_none());
    let full = env.pretty(&dir, &["show", "aa-000002"]);
    assert!(full.contains("pick_up (agent)  held  dependency_open aa-000001"));
    let listed_pretty = env.pretty(&dir, &["search", "aa-000002", "--actions"]);
    assert!(listed_pretty.contains("\naa-000002\n  pick_up (agent)"));
    let summary = env.json(&dir, &["show", "aa-000002", "--summary", "--no-fetch"]);
    assert_eq!(summary["actions"], show["actions"]);

    let listed = env.json(&dir, &["list", "--actions"]);
    let row = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "aa-000002")
        .unwrap();
    assert_eq!(row["actions"], show["actions"]);
    let filtered = env.json(
        &dir,
        &["search", "aa-000002", "--actions", "--status", "todo"],
    );
    assert_eq!(filtered["tasks"][0]["actions"], show["actions"]);
    for args in [&["list"][..], &["search", "Fixture"][..]] {
        let out = env.json(&dir, args);
        assert!(
            out["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row.get("actions").is_none())
        );
    }
}

#[test]
fn actions_preserve_independent_park_and_dependency_evidence() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    record(&dir, "aa-000001", "todo", 2, "");
    record(
        &dir,
        "aa-000002",
        "todo",
        2,
        "depends: [aa-000001, zz-000001]\n",
    );
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:park")
        .args([
            "park",
            "aa-000002",
            "Choose the scope",
            "--waiting-on",
            "user",
            "--reason",
            "decision",
        ])
        .assert()
        .success();
    let path = dir.join("tasks/aa-000002.md");
    let task_bytes = std::fs::read(&path).unwrap();
    let store = env.claim_store("aa");
    let store_bytes = std::fs::read(&store).unwrap();

    let show = env.json(&dir, &["show", "aa-000002", "--no-fetch"]);
    let pick = action(&show, "pick_up");
    assert_eq!(pick["available"], false);
    let reasons = pick["reasons"].as_array().unwrap();
    assert_eq!(
        reasons
            .iter()
            .map(|r| r["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["dependency_open", "dependency_unknown", "user_handoff"]
    );
    assert_eq!(reasons[1]["evidence"]["task"], "zz-000001");
    assert!(reasons[2]["evidence"]["host"].as_str().is_some());
    assert!(reasons[2]["evidence"]["observed_at"].as_str().is_some());
    let answer = action(&show, "answer");
    assert_eq!(answer["reasons"], json!([]));
    assert_eq!(
        answer["subject"],
        json!({
            "source":"park", "task":"aa-000002", "host":show["park"]["host"],
            "at":show["park"]["at"], "next_step":"Choose the scope"
        })
    );
    assert!(answer.get("window").is_none());
    assert_eq!(std::fs::read(path).unwrap(), task_bytes);
    assert_eq!(std::fs::read(store).unwrap(), store_bytes);
}

#[test]
fn actions_show_unreachable_foreign_piece() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let piece = id_of(env.json(&bb, &["add", "Foreign piece"]));
    let goal = id_of(env.json(&aa, &["add", "Goal", "--piece", &piece]));
    let child = id_of(env.json(&aa, &["add", "Local child", "--parent", &goal]));
    std::fs::remove_file(bb.join("tasks/.config.toml")).unwrap();
    let show = env.json(&aa, &["show", &goal, "--no-fetch"]);
    assert_eq!(show["task"]["id"], goal);
    assert_eq!(
        show["warnings"],
        json!([format!("dependency {piece} is unreachable")])
    );
    let close = action(&show, "close_goal");
    assert_eq!(close["available"], false);
    assert!(
        close["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"code":"member_unknown","evidence":{"task":piece}}))
    );
    assert!(
        close["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"code":"member_open","evidence":{"task":child}}))
    );
    assert_eq!(env.fail(&aa, &["done", &goal]), "open_dependencies");
    std::fs::write(aa.join("tasks/aa-ffffff.md"), "---\nstatus: broken\n---\n").unwrap();
    assert_eq!(env.fail(&aa, &["show", &goal, "--no-fetch"]), "parse");
}

#[test]
fn actions_show_legacy_scope_and_sample_disagreement() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    for (id, status) in [
        ("aa-000001", "todo"),
        ("aa-000002", "blocked"),
        ("aa-000003", "idea"),
    ] {
        record(&dir, id, status, 2, "");
        let mut args = vec!["note", id, "scope: drop; proposal: keep or drop"];
        if status == "idea" {
            args.push("--reviewed");
        }
        env.json(&dir, &args);
        let show = env.json(&dir, &["show", id]);
        assert_eq!(action(&show, "assess")["available"], status != "idea");
        let curate = action(&show, "curate");
        assert_eq!(curate["available"], false);
        assert!(
            curate["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["code"] == "decision_open")
        );
        let subject = &action(&show, "answer")["subject"];
        assert_eq!(
            subject,
            &json!({"source":"latest_note", "task":id, "note_index":0})
        );
    }
    env.json(&dir, &["note", "aa-000003", "Incidental progress"]);
    let show = env.json(&dir, &["show", "aa-000003"]);
    assert_eq!(action(&show, "assess")["available"], true);
    assert!(
        !show["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"] == "answer")
    );
    env.json(
        &dir,
        &["note", "aa-000003", "scope: followed proposal::pending"],
    );
    let show = env.json(&dir, &["show", "aa-000003"]);
    assert_eq!(action(&show, "answer")["subject"]["note_index"], 2);
}

#[test]
fn actions_resume_and_reconsider_are_independent() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    record(&dir, "aa-000001", "todo", 2, "");
    record(&dir, "aa-000002", "idea", 2, "");
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:park")
        .args([
            "park",
            "aa-000002",
            "Assess the idea",
            "--waiting-on",
            "agent",
        ])
        .assert()
        .success();
    let next = env.json(&dir, &["next", "--no-fetch"]);
    assert_eq!(next["next"]["task"]["id"], "aa-000002");
    let idea = env.json(&dir, &["show", "aa-000002"]);
    assert_eq!(action(&idea, "resume")["available"], true);
    assert_eq!(action(&idea, "assess")["available"], true);
    record(&dir, "aa-000003", "todo", 2, "depends: [aa-000001]\n");
    let wake = id_of(env.json(&dir, &["add", "Wake target"]));
    env.json(
        &dir,
        &[
            "shelve",
            "aa-000003",
            "Wake target closes",
            "--wake-on",
            &wake,
            "--defer",
            "2099-01-01",
        ],
    );
    env.json(&dir, &["done", &wake]);
    let shelf = env.json(&dir, &["show", "aa-000003"]);
    let reconsider = action(&shelf, "reconsider");
    assert_eq!(reconsider["available"], true);
    assert!(
        reconsider["signals"]
            .as_array()
            .unwrap()
            .contains(&json!({"code":"wake_target_closed","evidence":{"task":wake}}))
    );
    let pick = action(&shelf, "pick_up");
    assert_eq!(pick["available"], false);
    assert!(
        pick["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!({"code":"dependency_open","evidence":{"task":"aa-000001"}}))
    );
}

#[test]
fn actions_claims_and_unresolved_parks_use_host_evidence() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    for id in ["aa-000001", "aa-000002", "aa-000003"] {
        record(&dir, id, "todo", 2, "");
    }
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:owner")
        .args(["start", "aa-000001"])
        .assert()
        .success();
    common::write_claim_in_view(&env, "aa", "aa-000002", "test:remote", "0:1");
    let live = env.json(&dir, &["show", "aa-000001"]);
    assert_eq!(
        action(&live, "resume")["reasons"][0]["code"],
        "already_active"
    );
    let foreign = env.json(&dir, &["show", "aa-000002"]);
    for kind in ["pick_up", "resume", "curate"] {
        let item = action(&foreign, kind);
        assert_eq!(item["available"], false);
        assert_eq!(item["reasons"][0]["code"], "claim_unknown");
        assert!(
            item["reasons"][0]["evidence"]["observed_at"]
                .as_str()
                .is_some()
        );
    }
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:parking")
        .args([
            "park",
            "aa-000003",
            "Need the idle host",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ])
        .assert()
        .success();
    let quiet = env.json(&dir, &["show", "aa-000003"]);
    assert_eq!(
        action(&quiet, "resume")["reasons"][0]["code"],
        "host_condition"
    );
    assert!(
        !quiet["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"] == "answer")
    );
    std::fs::remove_file(dir.join("tasks/aa-000003.md")).unwrap();
    let parked = env.json(&dir, &["list", "--parked", "--actions"]);
    assert_eq!(parked["tasks"][0]["id"], "aa-000003");
    assert!(parked["tasks"][0].get("actions").is_none());
    assert!(parked["warnings"].to_string().contains("park entry only"));
}

#[test]
fn actions_follow_dependencies_from_the_selected_checkout() {
    let mut env = TestEnv::new();
    let main = env.init("aa");
    let other = env.init("bb");
    record(&main, "aa-000001", "todo", 2, "depends: [aa-000002]\n");
    record(&main, "aa-000002", "todo", 2, "");
    let side_home = tempfile::tempdir().unwrap();
    let side = side_home.path().join("side");
    common::isolated_worktree(&env, &main, &side);
    record(
        &side,
        "aa-000002",
        "done",
        2,
        "closed: 2020-01-02T00:00:00Z\ncompleted: 2020-01-02T00:00:00Z\n",
    );
    assert_eq!(
        action(
            &env.json(
                &side,
                &["show", "aa-000001", "--no-fetch", "--all-projects"]
            ),
            "pick_up"
        )["available"],
        true,
    );
    std::fs::copy(
        main.join("tasks/aa-000002.md"),
        side.join("tasks/aa-000002.md"),
    )
    .unwrap();
    let moved = env.json(&main, &["move", "aa-000002", "--to", "bb"]);
    let successor = moved["id"].as_str().unwrap();
    assert_eq!(
        action(&env.json(&main, &["show", "aa-000001"]), "pick_up")["reasons"],
        json!([{"code":"dependency_open","evidence":{"task":successor}}])
    );
    assert_eq!(
        env.json(&main, &["show", "aa-000002"])["actions"],
        json!([])
    );
    env.json(&other, &["done", successor]);
    let listed = env.json(
        &main,
        &["search", "aa-000001", "--all-projects", "--actions"],
    );
    assert_eq!(action(&listed["tasks"][0], "pick_up")["available"], true);
}
