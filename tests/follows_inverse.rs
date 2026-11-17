//! Incoming continuation discovery uses the captured scope, including closed work.
mod common;
use common::{TestEnv, id_of};
use serde_json::Value;

fn inject(e: &TestEnv, dir: &std::path::Path, id: &str, targets: &[&str]) {
    let path = format!("tasks/{id}.md");
    let text = e.read(dir, &path);
    std::fs::write(
        dir.join(path),
        text.replace(
            "\ntags:",
            &format!("\nfollows: [{}]\ntags:", targets.join(", ")),
        ),
    )
    .unwrap();
}
fn incoming(shown: &Value) -> Vec<String> {
    shown["followed_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn follows_inverse_local_closed_order_duplicate_and_self() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let q = id_of(e.json(&aa, &["add", "Earlier", "-p", "0"]));
    assert!(e.json(&aa, &["show", &q]).get("followed_by").is_none());
    let open = id_of(e.json(&aa, &["add", "Open", "-p", "1"]));
    let done = id_of(e.json(&aa, &["add", "Done", "-p", "2"]));
    let dropped = id_of(e.json(&aa, &["add", "Dropped", "-p", "3"]));
    inject(&e, &aa, &open, &[&q, &q]);
    inject(&e, &aa, &done, &[&q]);
    inject(&e, &aa, &dropped, &[&q]);
    inject(&e, &aa, &q, &[&q]);
    e.json(&aa, &["done", &done]);
    e.json(&aa, &["drop", &dropped, "test"]);
    let shown = e.json(&aa, &["show", &q]);
    assert_eq!(incoming(&shown), vec![open.clone(), done, dropped]);
    assert_eq!(shown["followed_by"][0]["status"], "todo");
    assert_eq!(
        e.json(&aa, &["show", &open])["follows_tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let next = e.json(&aa, &["next"]);
    assert_eq!(next["next"]["task"]["id"], q);
    assert_eq!(next["next"]["followed_by"], shown["followed_by"]);
    let pretty = e.cmd(&aa).args(["--pretty", "show", &q]).output().unwrap();
    assert!(pretty.status.success());
    assert!(
        String::from_utf8(pretty.stdout)
            .unwrap()
            .contains("followed by")
    );
}

#[test]
fn follows_inverse_next_uses_existing_wide_and_group_scope() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Earlier", "-p", "0"]));
    let follower = id_of(e.json(&bb, &["add", "Foreign"]));
    inject(&e, &bb, &follower, &[&q]);
    let unrelated = id_of(e.json(&aa, &["add", "Broken unrelated"]));
    inject(&e, &aa, &unrelated, &["missing-000001"]);
    let shown = e.json(&aa, &["show", &q]);
    assert!(shown.get("followed_by").is_none());
    assert!(shown["warnings"].as_array().unwrap().is_empty());
    let next = e.json(&aa, &["next", "--all-projects"]);
    assert_eq!(incoming(&next["next"]), vec![follower.clone()]);
    e.json(&aa, &["group", "set", "pair", "aa", "bb"]);
    assert_eq!(
        incoming(&e.json(&aa, &["next", "--group", "pair"])["next"]),
        vec![follower]
    );
}

#[test]
fn follows_inverse_moves_preserve_identity_and_scope() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let cc = e.init("cc");
    let q = id_of(e.json(&aa, &["add", "Earlier", "-p", "0"]));
    let oldf = id_of(e.json(&aa, &["add", "Old follower"]));
    inject(&e, &aa, &oldf, &[&q]);
    let moved = e.json(&aa, &["move", &q, "--to", "bb"]);
    let _ = moved;
    let target = e.json(&aa, &["show", &q])["task"]["moved_to"]
        .as_str()
        .unwrap()
        .to_string();
    e.json(&bb, &["move", &target, "--to", "cc"]);
    let terminal = e.json(&bb, &["show", &target])["task"]["moved_to"]
        .as_str()
        .unwrap()
        .to_string();
    let newf = id_of(e.json(&cc, &["add", "New follower"]));
    inject(&e, &cc, &newf, &[&terminal]);
    // Invalid self-history assertion must never turn a successor into its own follower.
    inject(&e, &cc, &terminal, &[&q]);
    assert_eq!(incoming(&e.json(&aa, &["show", &q])), vec![oldf.clone()]);
    assert_eq!(
        incoming(&e.json(&cc, &["show", &terminal])),
        vec![newf.clone()]
    );
    let wide = e.json(&cc, &["next", "--all-projects"]);
    assert_eq!(wide["next"]["task"]["id"], terminal);
    let rows = incoming(&wide["next"]);
    assert_eq!(rows.len(), 2);
    assert!(rows.contains(&oldf) && rows.contains(&newf));
    e.json(&aa, &["move", &oldf, "--to", "bb"]);
    assert!(e.json(&aa, &["show", &q]).get("followed_by").is_none());
    let wide = e.json(&cc, &["next", "--all-projects"]);
    assert_eq!(incoming(&wide["next"]).len(), 2);
    assert!(!incoming(&wide["next"]).contains(&oldf));
}

#[test]
fn follows_inverse_incomplete_history_warns_once_and_keeps_direct_rows() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Earlier"]));
    let f = id_of(e.json(&aa, &["add", "Follower"]));
    inject(&e, &aa, &f, &[&q]);
    let unrelated = id_of(e.json(&aa, &["add", "Unrelated broken"]));
    inject(&e, &aa, &unrelated, &["zz-000001"]);
    e.json(&aa, &["move", &q, "--to", "bb"]);
    let target = e.json(&aa, &["show", &q])["task"]["moved_to"]
        .as_str()
        .unwrap()
        .to_string();
    std::fs::remove_file(bb.join(format!("tasks/{target}.md"))).unwrap();
    let shown = e.json(&aa, &["show", &q]);
    assert_eq!(incoming(&shown), vec![f]);
    assert_eq!(shown["warnings"].as_array().unwrap().len(), 1);
    assert!(
        shown["warnings"][0]
            .as_str()
            .unwrap()
            .contains("coverage is incomplete")
    );
}
