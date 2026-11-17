//! `show --all-projects` feeds its widened scan to the followed-by filter: foreign
//! followers appear only with the flag, moved focal ids agree over one scope, and
//! `next`'s existing widening is unchanged.
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
    let mut ids: Vec<String> = shown["followed_by"]
        .as_array()
        .unwrap_or_else(|| panic!("no followed_by in {shown}"))
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

fn warnings(shown: &Value) -> Vec<String> {
    shown["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn follows_wide_show_foreign_follower_only_with_flag_and_honest_warnings() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let dd = e.init("dd");
    let q = id_of(e.json(&aa, &["add", "Earlier", "-p", "0"]));
    let follower = id_of(e.json(&bb, &["add", "Foreign follower"]));
    inject(&e, &bb, &follower, &[&q]);
    // A broken follows edge in another project is not this task's concern.
    let broken = id_of(e.json(&bb, &["add", "Unrelated broken"]));
    inject(&e, &bb, &broken, &["zz-000001"]);

    let plain = e.json(&aa, &["show", &q]);
    assert!(plain.get("followed_by").is_none());
    assert!(warnings(&plain).is_empty());
    let wide = e.json(&aa, &["show", &q, "--all-projects"]);
    assert_eq!(incoming(&wide), vec![follower.clone()]);
    assert!(warnings(&wide).is_empty(), "{:?}", warnings(&wide));
    // Outgoing fields stay as they were.
    assert_eq!(wide["follows_tasks"], plain["follows_tasks"]);

    // next's existing widening is unchanged and agrees with show's.
    let next = e.json(&aa, &["next", "--all-projects"]);
    assert_eq!(next["next"]["task"]["id"], q);
    assert_eq!(next["next"]["followed_by"], wide["followed_by"]);

    // An unreachable project is named, and reachable followers are still listed.
    std::fs::remove_file(dd.join("tasks/.config.toml")).unwrap();
    let partial = e.json(&aa, &["show", &q, "--all-projects"]);
    assert_eq!(incoming(&partial), vec![follower]);
    let named = warnings(&partial);
    assert_eq!(named.len(), 1, "{named:?}");
    assert!(
        named[0].starts_with("project dd at ") && named[0].ends_with(" is unreachable"),
        "{named:?}"
    );
}

#[test]
fn follows_wide_show_moved_focal_ids_agree_over_one_scope() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let cc = e.init("cc");
    let q = id_of(e.json(&aa, &["add", "Earlier"]));
    e.json(&aa, &["move", &q, "--to", "bb"]);
    let successor = e.json(&aa, &["show", &q])["task"]["moved_to"]
        .as_str()
        .unwrap()
        .to_string();
    // One follower cites the old id, the other the successor, both outside aa and bb.
    let old_cite = id_of(e.json(&cc, &["add", "Cites old id"]));
    inject(&e, &cc, &old_cite, &[&q]);
    let new_cite = id_of(e.json(&cc, &["add", "Cites successor"]));
    inject(&e, &cc, &new_cite, &[&successor]);

    assert!(e.json(&aa, &["show", &q]).get("followed_by").is_none());
    assert!(
        e.json(&bb, &["show", &successor])
            .get("followed_by")
            .is_none()
    );
    let mut both = vec![old_cite, new_cite];
    both.sort();
    let from_old = e.json(&aa, &["show", &q, "--all-projects"]);
    let from_new = e.json(&bb, &["show", &successor, "--all-projects"]);
    assert_eq!(incoming(&from_old), both);
    assert_eq!(incoming(&from_new), both);
    assert!(warnings(&from_old).is_empty() && warnings(&from_new).is_empty());
}
