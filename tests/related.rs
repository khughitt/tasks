//! Integration tests for the `related` frontmatter field and operations.

mod common;
use common::{TestEnv, editor_script, err_kind, id_of};
use serde_json::json;
use std::path::Path;

/// Writes `related: [...]` into a record by hand. Phase 1 has no writer, so tests stand in for the phase 2 writer.
fn relate_by_hand(dir: &Path, id: &str, ids: &[&str]) {
    let path = dir.join("tasks").join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "\ntags:",
        &format!("\nrelated: [{}]\ntags:", ids.join(", ")),
        1,
    );
    std::fs::write(&path, text).unwrap();
}

#[test]
fn related_round_trips_and_is_omitted_when_empty() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    assert!(
        env.json(&dir, &["show", &a])["task"]
            .get("related")
            .is_none()
    );
    relate_by_hand(&dir, &a, &[&b]);
    assert_eq!(env.json(&dir, &["show", &a])["task"]["related"], json!([b]));
    env.json(&dir, &["note", &a, "plain note"]); // an unrelated write keeps the list byte-for-byte
    assert!(
        env.read(&dir, &format!("tasks/{a}.md"))
            .contains(&format!("related: [{b}]"))
    );
}

#[test]
fn phase_one_editor_save_cannot_change_related() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    let c = id_of(env.json(&dir, &["add", "C"]));

    let path_a = dir.join("tasks").join(format!("{a}.md"));

    // Case 1: An editor script adds `related: [<b>]`; the edit fails, and the record bytes
    // match what was read before the edit.
    let bytes_before = std::fs::read(&path_a).unwrap();
    let add_script = editor_script(&dir, &format!("sed -i '/^tags:/i related: [{b}]' \"$1\""));
    let out = env
        .cmd(&dir)
        .env("EDITOR", &add_script)
        .args(["edit", &a, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("related cannot be changed in the editor by this version of tasks"),
        "stderr missing refusal: {stderr}"
    );
    assert_eq!(std::fs::read(&path_a).unwrap(), bytes_before);

    // Prepare a hand-written related list on `a`: [b, c]
    relate_by_hand(&dir, &a, &[&b, &c]);
    let bytes_with_related = std::fs::read(&path_a).unwrap();

    // Case 2: Removing a hand-written list fails the same way.
    let remove_script = editor_script(&dir, "sed -i '/^related:/d' \"$1\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &remove_script)
        .args(["edit", &a, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("related cannot be changed in the editor by this version of tasks"),
        "stderr missing refusal: {stderr}"
    );
    assert_eq!(std::fs::read(&path_a).unwrap(), bytes_with_related);

    // Case 3: Reordering a hand-written list fails the same way.
    let reorder_script = editor_script(
        &dir,
        &format!("sed -i 's/related: \\[{b}, {c}\\]/related: [{c}, {b}]/' \"$1\""),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &reorder_script)
        .args(["edit", &a, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("related cannot be changed in the editor by this version of tasks"),
        "stderr missing refusal: {stderr}"
    );
    assert_eq!(std::fs::read(&path_a).unwrap(), bytes_with_related);

    // Case 4: A save that changes only the title succeeds and keeps `related`.
    let retitle_script = editor_script(&dir, "sed -i 's/^title: A$/title: Renamed/' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &retitle_script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    let shown = env.json(&dir, &["show", &a]);
    assert_eq!(shown["task"]["title"], "Renamed");
    assert_eq!(shown["task"]["related"], json!([b, c]));
    assert!(
        env.read(&dir, &format!("tasks/{a}.md"))
            .contains(&format!("related: [{b}, {c}]"))
    );
}
