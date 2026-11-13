//! Reader-release contracts: raw fixtures stand in for writers that are not enabled.
mod common;
use common::{TestEnv, editor_script, id_of};
use serde_json::json;
use std::path::Path;

fn raw(env: &TestEnv, dir: &Path, id: &str, key: &str, values: &[&str]) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = env.read(dir, &format!("tasks/{id}.md"));
    std::fs::write(
        path,
        text.replacen(
            "\ntags:",
            &format!("\n{key}: [{}]\ntags:", values.join(", ")),
            1,
        ),
    )
    .unwrap();
}

#[test]
fn follows_reader_forward_roundtrip_and_projection() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    common::configure_color(&bb);
    let first = id_of(env.json(&aa, &["add", "First"]));
    let foreign = id_of(env.json(&bb, &["add", "Foreign"]));
    let task = id_of(env.json(
        &aa,
        &[
            "add",
            "Continuation",
            "--source",
            &first,
            "--related",
            &first,
        ],
    ));
    assert!(
        env.json(&aa, &["show", &task])
            .get("follows_tasks")
            .is_none()
    );
    assert!(
        env.json(&aa, &["show", &task])["task"]
            .get("follows")
            .is_none()
    );
    raw(
        &env,
        &aa,
        &task,
        "follows",
        &[&first, &first, &foreign, "zz-000001"],
    );
    let shown = env.json(&aa, &["show", &task]);
    assert_eq!(
        shown["task"]["follows"],
        json!([first, first, foreign, "zz-000001"])
    );
    let colored = env
        .cmd(&aa)
        .args(["--pretty", "--color", "always", "show", &task])
        .output()
        .unwrap();
    assert!(colored.status.success());
    let colored = String::from_utf8(colored.stdout).unwrap();
    // Project id colors mix 30% white in OKLab into the target project's declared RGB.
    assert!(
        colored.contains(&format!("\x1b[38;2;121;188;200m{foreign}\x1b[0m")),
        "{colored:?}"
    );
    let rows = shown["follows_tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["title"], "First");
    assert_eq!(rows[2]["title"], "Foreign");
    assert_eq!(rows[3]["resolved"], false);
    assert!(rows[3].get("title").is_none());
    assert!(
        shown["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("zz-000001"))
    );
    env.json(&aa, &["note", &task, "ordinary note"]);
    let noted = env.json(&aa, &["show", &task]);
    assert_eq!(noted["task"]["follows"], shown["task"]["follows"]);
    assert_eq!(noted["task"]["related"], json!([first]));
    assert_eq!(noted["task"]["source"], first);
    let list = env.json(&aa, &["list"]);
    assert!(
        list["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == task)
            .unwrap()
            .get("follows")
            .is_none()
    );
    env.json(&aa, &["done", &first]);
    let next = env.json(&aa, &["next"]);
    assert_eq!(next["next"]["task"]["follows"], shown["task"]["follows"]);
}

#[test]
fn follows_reader_empty_malformed_and_unknown_keys() {
    for (key, values, valid) in [
        ("follows", vec![], true),
        ("follows", vec!["bad-id"], false),
        ("arbitrary_unknown", vec![], false),
    ] {
        let mut env = TestEnv::new();
        let aa = env.init("aa");
        let a = id_of(env.json(&aa, &["add", "A"]));
        raw(&env, &aa, &a, key, &values);
        let out = env.cmd(&aa).args(["show", &a]).output().unwrap();
        assert_eq!(out.status.success(), valid, "{out:?}");
        if valid {
            env.json(&aa, &["note", &a, "save"]);
            assert!(!env.read(&aa, &format!("tasks/{a}.md")).contains("follows:"));
        }
    }
}

#[test]
fn follows_reader_semantic_findings_remain_readable() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let target = id_of(env.json(&bb, &["add", "Target"]));
    env.json(&bb, &["rename", "bb", "cc"]);
    let renamed = target.replacen("bb-", "cc-", 1);
    let a = id_of(env.json(&aa, &["add", "A"]));
    raw(
        &env,
        &aa,
        &a,
        "follows",
        &[&a, &target, &renamed, &target, "aa-ffffff"],
    );
    env.json(&aa, &["show", &a]);
    env.json(&aa, &["list"]);
    env.json(&aa, &["ready"]);
    let check = env.check(&aa);
    for kind in ["self_follows", "duplicate_follows", "dangling_follows"] {
        assert!(
            check["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["kind"] == kind),
            "{check}"
        );
    }
    assert_eq!(check["errors"], json!([]));
    let script = editor_script(&aa, "sed -i 's/title: A/title: Edited/' \"$1\"");
    env.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    assert_eq!(
        env.json(&aa, &["show", &a])["task"]["follows"],
        json!([a, target, renamed, target, "aa-ffffff"])
    );
}

#[test]
fn follows_reader_editor_changes_refused_atomically() {
    for edit in [
        "sed -i '/^tags:/i follows: [aa-000001]' \"$1\"",
        "sed -i '/^follows:/d' \"$1\"",
        "sed -i 's/follows: .*/follows: [aa-000002, aa-000001]/' \"$1\"",
    ] {
        let mut env = TestEnv::new();
        let aa = env.init("aa");
        let a = id_of(env.json(&aa, &["add", "A"]));
        if !edit.contains("/^tags:/") {
            raw(&env, &aa, &a, "follows", &["aa-000001", "aa-000002"]);
        }
        let before = env.read(&aa, &format!("tasks/{a}.md"));
        let script = editor_script(&aa, edit);
        let out = env
            .cmd(&aa)
            .env("EDITOR", script)
            .args(["edit", &a, "--editor"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "{out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("follows"));
        assert_eq!(env.read(&aa, &format!("tasks/{a}.md")), before);
    }
}

#[test]
fn follows_reader_lifecycle_move_rename_and_held_citations() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    let target = id_of(env.json(&aa, &["add", "Target"]));
    let a = id_of(env.json(&aa, &["add", "A", "--source", "capture:original"]));
    raw(&env, &aa, &a, "follows", &[&target]);
    env.json(&aa, &["rename", "aa", "newaa"]);
    let a = a.replacen("aa-", "newaa-", 1);
    let renamed = target.replacen("aa-", "newaa-", 1);
    env.json(&aa, &["note", &a, &format!("citation {target}")]);
    assert!(
        env.json(&aa, &["show", &a])["task"]
            .get("related")
            .is_none()
    );
    let moved_target = id_of(env.json(&aa, &["move", &renamed, "--to", "bb"]));
    // A stored live successor also holds a citation using the old moved spelling.
    let held = id_of(env.json(&cc, &["add", "Held"]));
    raw(&env, &cc, &held, "follows", &[&moved_target]);
    env.json(&cc, &["note", &held, &format!("citation {renamed}")]);
    assert!(
        env.json(&cc, &["show", &held])["task"]
            .get("related")
            .is_none()
    );
    let shown = env.json(&aa, &["show", &a]);
    assert_eq!(shown["follows_tasks"][0]["status"], "moved");
    let b = id_of(env.json(&aa, &["move", &a, "--to", "bb"]));
    let c = id_of(env.json(&bb, &["move", &b, "--to", "cc"]));
    assert_eq!(
        env.json(&cc, &["show", &c])["task"]["follows"],
        json!([renamed])
    );
    assert!(
        env.json(&aa, &["show", &a])["task"]
            .get("follows")
            .is_none()
    );
    assert!(
        env.json(&bb, &["show", &b])["task"]
            .get("follows")
            .is_none()
    );
    assert_eq!(
        env.json(&cc, &["show", &c])["task"]["source"],
        "capture:original"
    );
    raw(&env, &aa, &a, "follows", &[&renamed]);
    let out = env.cmd(&aa).arg("check").output().unwrap();
    assert!(!out.status.success());
    let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        check["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "follows_status")
    );
}
