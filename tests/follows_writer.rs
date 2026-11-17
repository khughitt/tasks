//! Explicit continuation writes must remain atomic and independent of scheduling.
mod common;
use common::{TestEnv, editor_script, id_of};
use serde_json::json;

#[test]
fn follows_writer_append_remove_clear_and_citations() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let a = id_of(e.json(&aa, &["add", "Earlier"]));
    let b = id_of(e.json(&bb, &["add", "Foreign"]));
    let t = id_of(e.json(
        &aa,
        &[
            "add",
            "More",
            "--follows",
            &a,
            "--follows",
            &a,
            "--body",
            &a,
        ],
    ));
    let shown = e.json(&aa, &["show", &t]);
    assert_eq!(shown["task"]["follows"], json!([a]));
    assert!(shown["task"].get("related").is_none());
    e.json(&aa, &["edit", &t, "--follows", &b]);
    assert_eq!(e.json(&aa, &["show", &t])["task"]["follows"], json!([a, b]));
    let repeated = e.json(&aa, &["edit", &t, "--follows", &a]);
    assert!(!repeated["warnings"].as_array().unwrap().is_empty());
    e.json(&aa, &["note", &t, &format!("still follows {a}")]);
    assert!(e.json(&aa, &["show", &t])["task"].get("related").is_none());
    e.json(&aa, &["edit", &t, "--body", "plain"]);
    e.json(
        &aa,
        &[
            "edit",
            &t,
            "--rm-follows",
            &a,
            "--body",
            &format!("new {a} {b}"),
        ],
    );
    assert!(e.json(&aa, &["show", &t])["task"].get("related").is_none());
    e.json(&aa, &["note", &t, &format!("later {a}")]);
    assert_eq!(e.json(&aa, &["show", &t])["task"]["related"], json!([a]));
    e.json(&aa, &["edit", &t, "--body", "plain"]);
    e.json(
        &aa,
        &[
            "edit",
            &t,
            "--no-follows",
            "--body",
            &format!("cleared {b}"),
        ],
    );
    assert_eq!(e.json(&aa, &["show", &t])["task"]["related"], json!([a]));
    e.json(&aa, &["edit", &t, "--follows", &b, "--related", &b]);
    e.json(&aa, &["edit", &t, "--no-follows"]);
    let shown = e.json(&aa, &["show", &t]);
    assert!(shown["task"].get("follows").is_none());
    assert_eq!(shown["task"]["related"], json!([a, b]));
}

#[test]
fn follows_writer_failures_are_atomic() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let a = id_of(e.json(&aa, &["add", "Earlier"]));
    let t = id_of(e.json(&aa, &["add", "More"]));
    for args in [
        vec!["edit", &t, "--follows", &t],
        vec!["edit", &t, "--follows", &a, "--follows", "aa-ffffff"],
        vec!["edit", &t, "--rm-follows", &a],
        vec!["edit", &t, "--follows", &a, "--rm-follows", &a],
        vec!["edit", &t, "--follows", &a, "--no-follows"],
        vec!["edit", &t, "--rm-follows", &a, "--no-follows"],
        vec!["edit", &t, "--editor", "--follows", &a],
    ] {
        let before = e.read(&aa, &format!("tasks/{t}.md"));
        e.cmd(&aa).args(args).assert().failure();
        assert_eq!(e.read(&aa, &format!("tasks/{t}.md")), before);
    }
    e.json(&aa, &["edit", &t, "--follows", &a]);
    let before = e.read(&aa, &format!("tasks/{t}.md"));
    // Invalid stored lists remain readable and survive unrelated writes, but flag writes validate the result.
    std::fs::write(
        aa.join(format!("tasks/{t}.md")),
        before.replace(&format!("follows: [{a}]"), &format!("follows: [{a}, {a}]")),
    )
    .unwrap();
    e.json(&aa, &["note", &t, "unrelated"]);
    e.cmd(&aa)
        .args(["edit", &t, "--follows", &a])
        .assert()
        .failure();
}

#[test]
fn follows_writer_alias_shorthand_and_moved_target() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let a = id_of(e.json(&bb, &["add", "Earlier"]));
    e.json(&bb, &["rename", "bb", "newbb"]);
    let canonical = a.replacen("bb-", "newbb-", 1);
    let t = id_of(e.json(
        &aa,
        &["add", "More", "--follows", &a, "--follows", &canonical],
    ));
    assert_eq!(
        e.json(&aa, &["show", &t])["task"]["follows"],
        json!([canonical])
    );
    let local = id_of(e.json(&aa, &["add", "Local"]));
    let bare = local.split_once('-').unwrap().1;
    e.json(&aa, &["edit", &t, "--follows", bare]);
    e.json(&aa, &["edit", &t, "--rm-follows", &a]);
    e.json(&bb, &["move", &canonical, "--to", "aa"]);
    e.cmd(&aa)
        .args(["edit", &t, "--follows", &canonical])
        .assert()
        .failure();
}

#[test]
fn follows_writer_editor_validates_changes_and_suppresses_removed_citations() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let a = id_of(e.json(&aa, &["add", "Earlier"]));
    let t = id_of(e.json(&aa, &["add", "More"]));
    let script = editor_script(&aa, &format!("sed -i '/^tags:/i follows: [{a}]' \"$1\""));
    e.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &t, "--editor"])
        .assert()
        .success();
    assert_eq!(e.json(&aa, &["show", &t])["task"]["follows"], json!([a]));
    for value in [t.clone(), format!("{a}, {a}"), "aa-ffffff".into()] {
        let before = e.read(&aa, &format!("tasks/{t}.md"));
        let script = editor_script(
            &aa,
            &format!("sed -i 's/^follows:.*/follows: [{value}]/' \"$1\""),
        );
        e.cmd(&aa)
            .env("EDITOR", script)
            .args(["edit", &t, "--editor"])
            .assert()
            .failure();
        assert_eq!(e.read(&aa, &format!("tasks/{t}.md")), before);
    }
    let script = editor_script(
        &aa,
        &format!("sed -i '/^follows:/d; /^## Notes/i new citation {a}' \"$1\""),
    );
    e.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &t, "--editor"])
        .assert()
        .success();
    let shown = e.json(&aa, &["show", &t]);
    assert!(shown["task"].get("follows").is_none());
    assert!(shown["task"].get("related").is_none());
}

#[test]
fn follows_writer_source_reuse_no_inference_and_non_gating() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let a = id_of(e.json(&aa, &["add", "Earlier"]));
    let t = id_of(e.json(&aa, &["add", "More", "--source", "capture:one"]));
    let reused = e.json(
        &aa,
        &["add", "More", "--source", "capture:one", "--follows", &a],
    );
    assert_eq!(reused["action"], "reused");
    assert!(e.json(&aa, &["show", &t])["task"].get("follows").is_none());
    e.json(
        &aa,
        &["note", &t, &format!("concerns: {a} extension — more work")],
    );
    assert!(e.json(&aa, &["show", &t])["task"].get("follows").is_none());
    let ready = e.json(&aa, &["ready"]);
    let next = e.json(&aa, &["next"]);
    let blockers = e.json(&aa, &["blockers"]);
    e.json(&aa, &["edit", &t, "--follows", &a]);
    let ids = |v: &serde_json::Value| {
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&e.json(&aa, &["ready"])), ids(&ready));
    assert_eq!(
        e.json(&aa, &["next"])["next"]["task"]["id"],
        next["next"]["task"]["id"]
    );
    assert_eq!(e.json(&aa, &["blockers"]), blockers);
    let shown = e.json(&aa, &["show", &t]);
    assert!(shown["task"].get("depends").is_none());
    assert!(shown["task"].get("parent").is_none());
    assert_eq!(shown["task"]["reviewed"], false);
    e.json(&aa, &["done", &t]); // Earlier remains open.
}

#[test]
fn follows_writer_completion_and_unchanged_unreachable_editor_list() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let a = id_of(e.json(&aa, &["add", "Earlier"]));
    let subject = id_of(e.json(&aa, &["add", "Other"]));
    for (args, index) in [
        (vec!["tasks", "add", "More", "--follows", "aa-"], 4),
        (
            vec!["tasks", "edit", subject.as_str(), "--follows", "aa-"],
            4,
        ),
        (
            vec!["tasks", "edit", subject.as_str(), "--rm-follows", "aa-"],
            4,
        ),
    ] {
        assert!(e.complete_values(&aa, "bash", index, &args).contains(&a));
    }
    let path = aa.join(format!("tasks/{a}.md"));
    std::fs::write(
        &path,
        e.read(&aa, &format!("tasks/{a}.md"))
            .replace("\ntags:", "\nfollows: [zz-000001]\ntags:"),
    )
    .unwrap();
    let script = editor_script(&aa, "sed -i 's/title: Earlier/title: Changed/' \"$1\"");
    e.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    assert_eq!(
        e.json(&aa, &["show", &a])["task"]["follows"],
        json!(["zz-000001"])
    );
}
