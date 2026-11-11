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

fn wake_by_hand(dir: &Path, id: &str, ids: &[&str]) {
    let path = dir.join("tasks").join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "\ntags:",
        &format!("\nwake_on: [{}]\ntags:", ids.join(", ")),
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
fn add_and_edit_related_link_local_and_foreign_ids() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");

    let local = id_of(env.json(&aa, &["add", "Local Target"]));
    let foreign = id_of(env.json(&bb, &["add", "Foreign Target"]));
    let to_rename = id_of(env.json(&cc, &["add", "Rename Target"]));
    env.json(&cc, &["rename", "cc", "newcc"]);
    let retired_spelling = to_rename.clone();
    let canonical_renamed = to_rename.replacen("cc-", "newcc-", 1);
    let suffix_target = id_of(env.json(&aa, &["add", "Suffix Target"]));
    let bare_suffix = &suffix_target[3..];

    // add --related with local and foreign
    let t1 = id_of(env.json(
        &aa,
        &["add", "T1", "--related", &local, "--related", &foreign],
    ));
    let shown1 = env.json(&aa, &["show", &t1]);
    assert_eq!(shown1["task"]["related"], json!([local, foreign]));

    // edit --related with retired prefix spelling and bare six-hex suffix
    env.json(
        &aa,
        &[
            "edit",
            &t1,
            "--related",
            &retired_spelling,
            "--related",
            bare_suffix,
        ],
    );
    let shown2 = env.json(&aa, &["show", &t1]);
    assert_eq!(
        shown2["task"]["related"],
        json!([local, foreign, canonical_renamed, suffix_target])
    );

    // edit --rm-related removes by canonical id
    env.json(&aa, &["edit", &t1, "--rm-related", &local]);
    let shown3 = env.json(&aa, &["show", &t1]);
    assert_eq!(
        shown3["task"]["related"],
        json!([foreign, canonical_renamed, suffix_target])
    );

    // edit --no-related clears the list
    env.json(&aa, &["edit", &t1, "--no-related"]);
    let shown4 = env.json(&aa, &["show", &t1]);
    assert!(shown4["task"].get("related").is_none());
}

#[test]
fn related_refusals_name_their_cause() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let _bb = env.init("bb");
    let t = id_of(env.json(&aa, &["add", "Task"]));
    let moved_task = id_of(env.json(&aa, &["add", "Moved"]));
    let moved_successor = id_of(env.json(&aa, &["move", &moved_task, "--to", "bb"]));

    // 1. Missing id fails with error kind unresolvable_id (add and edit)
    let out = env
        .cmd(&aa)
        .args(["add", "New", "--related", "aa-ffffff"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "unresolvable_id");

    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--related", "aa-ffffff"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "unresolvable_id");

    // 2. Tombstone refuses as moved, naming the successor (add and edit)
    let out = env
        .cmd(&aa)
        .args(["add", "New", "--related", &moved_task])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "moved");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&moved_successor),
        "stderr missing successor: {stderr}"
    );

    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--related", &moved_task])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "moved");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&moved_successor),
        "stderr missing successor: {stderr}"
    );

    // 3. Self link refuses
    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--related", &t])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("{t} cannot relate to itself")),
        "stderr missing self refusal: {stderr}"
    );

    // 4. --related X --rm-related X refuses
    let target = id_of(env.json(&aa, &["add", "Target"]));
    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--related", &target, "--rm-related", &target])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!(
            "{target} is named by both --related and --rm-related"
        )),
        "stderr missing both refusal: {stderr}"
    );

    // 5. --no-related --related X and --no-related --rm-related X are usage errors (exit 2)
    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--no-related", "--related", &target])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));

    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--no-related", "--rm-related", &target])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));

    // 6. --rm-related of an absent id refuses
    let out = env
        .cmd(&aa)
        .args(["edit", &t, "--rm-related", &target])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("{t} is not related to {target}")),
        "stderr missing absent refusal: {stderr}"
    );
}

#[test]
fn repeated_and_alias_spellings_deduplicate_with_a_warning() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let b = id_of(env.json(&aa, &["add", "B"]));
    let x = id_of(env.json(&bb, &["add", "X"]));
    env.json(&bb, &["rename", "bb", "newbb"]);
    let retired_x = x.clone();
    let canonical_x = x.replacen("bb-", "newbb-", 1);

    // add with repeated --related within the same command: deduplicated silently, no warning
    let out = env.json(&aa, &["add", "A", "--related", &b, "--related", &b]);
    let a = id_of(out.clone());
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings.is_empty(),
        "expected no warnings on repeated add: {warnings:?}"
    );
    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(shown_a["task"]["related"], json!([b]));

    // edit --related with an already-related id: leaves list unchanged and warns
    let out = env.json(&aa, &["edit", &a, "--related", &b]);
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        format!("{a} is already related to {b}; unchanged")
    );
    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(shown_a["task"]["related"], json!([b]));

    // edit --related with canonical_x
    env.json(&aa, &["edit", &a, "--related", &canonical_x]);
    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(shown_a["task"]["related"], json!([b, canonical_x]));

    // edit --related with alias spelling of canonical_x: deduplicates and warns
    let out = env.json(&aa, &["edit", &a, "--related", &retired_x]);
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        format!("{a} is already related to {canonical_x}; unchanged")
    );
    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(shown_a["task"]["related"], json!([b, canonical_x]));

    // edit --related with repeats within one call when id was already present: exactly one warning
    let out = env.json(&aa, &["edit", &a, "--related", &b, "--related", &b]);
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        format!("{a} is already related to {b}; unchanged")
    );
}

#[test]
fn editor_saves_may_change_related_and_are_validated() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    let c = id_of(env.json(&dir, &["add", "C"]));

    let path_a = dir.join("tasks").join(format!("{a}.md"));

    // Case 1: An editor save adding a valid id succeeds
    let add_script = editor_script(&dir, &format!("sed -i '/^tags:/i related: [{b}]' \"$1\""));
    env.cmd(&dir)
        .env("EDITOR", &add_script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    let shown = env.json(&dir, &["show", &a]);
    assert_eq!(shown["task"]["related"], json!([b]));
    assert!(
        env.read(&dir, &format!("tasks/{a}.md"))
            .contains(&format!("related: [{b}]"))
    );

    // Case 2: Adding a missing id refuses without writing
    let bytes_before = std::fs::read(&path_a).unwrap();
    let bad_script = editor_script(&dir, "sed -i 's/related: .*/related: [aa-ffffff]/' \"$1\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &bad_script)
        .args(["edit", &a, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert_eq!(std::fs::read(&path_a).unwrap(), bytes_before);

    // Case 3: A removal succeeds
    let remove_script = editor_script(&dir, "sed -i '/^related:/d' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &remove_script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    let shown = env.json(&dir, &["show", &a]);
    assert!(shown["task"].get("related").is_none());
    assert!(
        !env.read(&dir, &format!("tasks/{a}.md"))
            .contains("related:")
    );

    // Case 4: Multiple related and removing one succeeds
    relate_by_hand(&dir, &a, &[&b, &c]);
    let remove_one_script = editor_script(
        &dir,
        &format!("sed -i 's/related: \\[{b}, {c}\\]/related: [{c}]/' \"$1\""),
    );
    env.cmd(&dir)
        .env("EDITOR", &remove_one_script)
        .args(["edit", &a, "--editor"])
        .assert()
        .success();
    let shown = env.json(&dir, &["show", &a]);
    assert_eq!(shown["task"]["related"], json!([c]));
}

#[test]
fn show_resolves_related_wake_and_source_rows() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let s = id_of(env.json(&dir, &["add", "S"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    let w = id_of(env.json(&dir, &["add", "W"]));
    env.json(&dir, &["done", &b]);
    let a = id_of(env.json(&dir, &["add", "A", "--source", &s]));
    env.json(
        &dir,
        &["shelve", &a, "wait on W", "--defer", "30d", "--wake-on", &w],
    );
    relate_by_hand(&dir, &a, &[&b]);

    let shown = env.json(&dir, &["show", &a]);
    let b_updated = env.json(&dir, &["show", &b])["task"]["updated"].clone();
    let w_updated = env.json(&dir, &["show", &w])["task"]["updated"].clone();
    let s_updated = env.json(&dir, &["show", &s])["task"]["updated"].clone();
    assert_eq!(
        shown["related_to"],
        json!([{"id": b, "title": "B", "status": "done", "updated": b_updated, "resolved": true}])
    );
    assert_eq!(
        shown["wake_targets"],
        json!([{"id": w, "title": "W", "status": "todo", "updated": w_updated, "resolved": true}])
    );
    assert_eq!(
        shown["source_task"],
        json!({"id": s, "title": "S", "status": "todo", "updated": s_updated, "resolved": true})
    );
}

#[test]
fn rows_resolve_across_projects_without_a_full_scan() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let a = id_of(env.json(&aa, &["add", "A"]));
    let x = id_of(env.json(&bb, &["add", "X"]));
    let x_updated = env.json(&bb, &["show", &x])["task"]["updated"].clone();
    relate_by_hand(&aa, &a, &[&x]);

    let shown = env.json(&aa, &["show", &a]);
    assert_eq!(shown["related_to"][0]["id"], x);
    assert_eq!(shown["related_to"][0]["title"], "X");
    assert_eq!(shown["related_to"][0]["status"], "todo");
    assert_eq!(shown["related_to"][0]["updated"], x_updated);
    assert_eq!(shown["related_to"][0]["resolved"], true);
}

#[test]
fn unresolved_rows_warn_once_each() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let a = id_of(env.json(&aa, &["add", "A", "--source", "aa-eeeeee"]));
    relate_by_hand(&aa, &a, &["aa-ffffff"]);

    let shown = env.json(&aa, &["show", &a]);
    assert_eq!(shown["related_to"][0]["resolved"], false);
    assert_eq!(shown["related_to"][0]["id"], "aa-ffffff");
    assert!(shown["related_to"][0].get("updated").is_none());
    assert_eq!(shown["source_task"]["resolved"], false);
    assert_eq!(shown["source_task"]["id"], "aa-eeeeee");
    assert!(shown["source_task"].get("updated").is_none());

    let warnings = shown["warnings"].as_array().unwrap();
    assert_eq!(
        warnings
            .iter()
            .filter(|w| w.as_str() == Some("related task aa-ffffff is unreachable"))
            .count(),
        1
    );
    assert_eq!(
        warnings
            .iter()
            .filter(|w| w.as_str() == Some("source task aa-eeeeee is unreachable"))
            .count(),
        1
    );

    let unshelved = id_of(env.json(&aa, &["add", "Unshelved"]));
    wake_by_hand(&aa, &unshelved, &["aa-cccccc"]);
    let shown_unshelved = env.json(&aa, &["show", &unshelved]);
    assert_eq!(shown_unshelved["wake_targets"][0]["resolved"], false);
    let u_warnings = shown_unshelved["warnings"].as_array().unwrap();
    assert_eq!(
        u_warnings
            .iter()
            .filter(|w| w.as_str() == Some("wake target aa-cccccc is unreachable"))
            .count(),
        1
    );

    let d = id_of(env.json(&aa, &["add", "D"]));
    let shelved = id_of(env.json(&aa, &["add", "Shelved"]));
    env.json(
        &aa,
        &[
            "shelve",
            &shelved,
            "reason",
            "--defer",
            "30d",
            "--wake-on",
            &d,
        ],
    );
    std::fs::remove_file(aa.join("tasks").join(format!("{d}.md"))).unwrap();
    let shown_shelved = env.json(&aa, &["show", &shelved]);
    assert_eq!(shown_shelved["wake_targets"][0]["resolved"], false);
    let s_warnings = shown_shelved["warnings"].as_array().unwrap();
    assert_eq!(
        s_warnings
            .iter()
            .filter(|w| w.as_str()
                == Some(&format!(
                    "{shelved}: wake target {d} is unreachable; repair the shelf condition"
                )))
            .count(),
        1
    );
    assert_eq!(
        s_warnings
            .iter()
            .filter(|w| w.as_str() == Some(&format!("wake target {d} is unreachable")))
            .count(),
        0
    );
}

#[test]
fn moved_rows_show_the_tombstone() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let _bb = env.init("bb");
    let a = id_of(env.json(&aa, &["add", "A"]));
    let b = id_of(env.json(&aa, &["add", "B"]));
    let c = id_of(env.json(&aa, &["add", "C"]));
    env.json(&aa, &["dep", &c, "--on", &b]);
    relate_by_hand(&aa, &a, &[&b]);
    env.json(&aa, &["move", &b, "--to", "bb"]);

    let b_updated = env.json(&aa, &["show", &b])["task"]["updated"].clone();
    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(
        shown_a["related_to"][0],
        json!({"id": b, "title": "B", "status": "moved", "updated": b_updated, "resolved": true})
    );

    let shown_c = env.json(&aa, &["show", &c]);
    assert_eq!(
        shown_c["depends_on"][0],
        json!({"id": b, "title": "B", "status": "moved", "updated": b_updated, "resolved": true})
    );
}

#[test]
fn a_source_with_surrounding_text_stays_opaque() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let s = id_of(env.json(&aa, &["add", "S"]));
    for source in [
        format!("from {s}"),
        format!(" {s}"),
        format!("{s}/x"),
        "zz-123456".to_string(),
    ] {
        let t = id_of(env.json(&aa, &["add", "T", "--source", &source]));
        let shown = env.json(&aa, &["show", &t]);
        assert!(
            shown.get("source_task").is_none(),
            "expected no source_task for {source:?}, got {:?}",
            shown.get("source_task")
        );
        let warnings = shown["warnings"].as_array().unwrap();
        assert_eq!(
            warnings.len(),
            0,
            "expected no warnings for {source:?}, got {warnings:?}"
        );
    }
}

#[test]
fn related_links_change_no_execution_view() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let b = id_of(env.json(&dir, &["add", "B"]));
    let a = id_of(env.json(&dir, &["add", "A"]));

    let ready_before = env.json(&dir, &["ready"]);
    let next_before = env.json(&dir, &["next"]);
    let mut blockers_before = env.json(&dir, &["blockers"]);
    let review_before = env.json(&dir, &["show", &a])["review_reason"].clone();
    let lanes_before = env.json(&dir, &["lanes"]);

    relate_by_hand(&dir, &a, &[&b]);

    let ready_after = env.json(&dir, &["ready"]);
    let next_after = env.json(&dir, &["next"]);
    let mut blockers_after = env.json(&dir, &["blockers"]);
    let review_after = env.json(&dir, &["show", &a])["review_reason"].clone();
    let lanes_after = env.json(&dir, &["lanes"]);

    blockers_before
        .as_object_mut()
        .unwrap()
        .remove("observed_at");
    blockers_after
        .as_object_mut()
        .unwrap()
        .remove("observed_at");

    assert_eq!(ready_before, ready_after);
    assert_eq!(
        next_before["next"]["task"]["id"],
        next_after["next"]["task"]["id"]
    );
    assert_eq!(blockers_before, blockers_after);
    assert_eq!(review_before, review_after);
    assert_eq!(lanes_before, lanes_after);

    env.cmd(&dir).args(["done", &a]).assert().success();
}

#[test]
fn next_embeds_the_same_rows() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let s = id_of(env.json(&dir, &["add", "S"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    let w = id_of(env.json(&dir, &["add", "W"]));
    let a = id_of(env.json(&dir, &["add", "A", "-p", "1", "--source", &s]));
    relate_by_hand(&dir, &a, &[&b]);
    wake_by_hand(&dir, &a, &[&w]);

    let next_out = env.json(&dir, &["next"]);
    let show_out = env.json(&dir, &["show", &a]);
    let b_updated = env.json(&dir, &["show", &b])["task"]["updated"].clone();
    let w_updated = env.json(&dir, &["show", &w])["task"]["updated"].clone();
    let s_updated = env.json(&dir, &["show", &s])["task"]["updated"].clone();

    let next_task = &next_out["next"];
    assert_eq!(next_task["task"]["id"], a);
    assert_eq!(
        next_task["related_to"],
        json!([{"id": b, "title": "B", "status": "todo", "updated": b_updated, "resolved": true}])
    );
    assert_eq!(
        next_task["wake_targets"],
        json!([{"id": w, "title": "W", "status": "todo", "updated": w_updated, "resolved": true}])
    );
    assert_eq!(
        next_task["source_task"],
        json!({"id": s, "title": "S", "status": "todo", "updated": s_updated, "resolved": true})
    );

    assert_eq!(next_task["related_to"], show_out["related_to"]);
    assert_eq!(next_task["wake_targets"], show_out["wake_targets"]);
    assert_eq!(next_task["source_task"], show_out["source_task"]);
}

#[test]
fn pretty_related_section_lists_every_link_in_order() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let p = id_of(env.json(&dir, &["add", "Parent"]));
    let dep = id_of(env.json(&dir, &["add", "Dep"]));
    let s = id_of(env.json(&dir, &["add", "S"]));
    let b = id_of(env.json(&dir, &["add", "B"]));
    let w = id_of(env.json(&dir, &["add", "W"]));
    env.json(&dir, &["done", &b]);
    let a = id_of(env.json(&dir, &["add", "A", "--parent", &p, "--source", &s]));
    env.json(&dir, &["dep", &a, "--on", &dep]);
    env.json(
        &dir,
        &["shelve", &a, "wait on W", "--defer", "30d", "--wake-on", &w],
    );
    let c = id_of(env.json(&dir, &["add", "Child", "--parent", &a]));
    relate_by_hand(&dir, &a, &[&b, "aa-ffffff"]);

    let pretty_a = env.pretty(&dir, &["show", &a]);

    // The overview has no "wake on:" line and no "source:" line.
    assert!(
        !pretty_a.contains("wake on:"),
        "unexpected wake on in overview: {pretty_a}"
    );
    assert!(
        !pretty_a.contains("source:"),
        "unexpected source in overview: {pretty_a}"
    );

    // The `── related ──` lines start, in order, with "parent", "depends on", "wake on", "related",
    // "source", "child"; each carries the id, a status and the title; an unresolved related id
    // prints "(unresolved)".
    let related_section = pretty_a
        .split_once("── related ")
        .expect("related section present")
        .1
        .split_once("\n── ")
        .map(|(s, _)| s)
        .unwrap_or_else(|| pretty_a.split_once("── related ").unwrap().1);
    let rows: Vec<&str> = related_section
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    assert_eq!(rows.len(), 7, "expected 7 related rows: {rows:?}");
    assert!(
        rows[0].starts_with("parent")
            && rows[0].contains(&p)
            && rows[0].contains("todo")
            && rows[0].contains("Parent"),
        "row 0 mismatch: {:?}",
        rows[0]
    );
    assert!(
        rows[1].starts_with("depends on")
            && rows[1].contains(&dep)
            && rows[1].contains("todo")
            && rows[1].contains("Dep"),
        "row 1 mismatch: {:?}",
        rows[1]
    );
    assert!(
        rows[2].starts_with("wake on")
            && rows[2].contains(&w)
            && rows[2].contains("todo")
            && rows[2].contains("W"),
        "row 2 mismatch: {:?}",
        rows[2]
    );
    assert!(
        rows[3].starts_with("related")
            && rows[3].contains(&b)
            && rows[3].contains("done")
            && rows[3].contains("B"),
        "row 3 mismatch: {:?}",
        rows[3]
    );
    assert!(
        rows[4].starts_with("related")
            && rows[4].contains("aa-ffffff")
            && rows[4].contains("?")
            && rows[4].contains("(unresolved)"),
        "row 4 mismatch: {:?}",
        rows[4]
    );
    assert!(
        rows[5].starts_with("source")
            && rows[5].contains(&s)
            && rows[5].contains("todo")
            && rows[5].contains("S"),
        "row 5 mismatch: {:?}",
        rows[5]
    );
    assert!(
        rows[6].starts_with("child")
            && rows[6].contains(&c)
            && rows[6].contains("todo")
            && rows[6].contains("Child"),
        "row 6 mismatch: {:?}",
        rows[6]
    );

    // A task whose source is a URL keeps "source:".
    let u = id_of(env.json(
        &dir,
        &["add", "URL Task", "--source", "https://example.com/item"],
    ));
    let pretty_u = env.pretty(&dir, &["show", &u]);
    assert!(
        pretty_u
            .lines()
            .any(|l| l.starts_with("source:") && l.contains("https://example.com/item")),
        "URL task missing source in overview: {pretty_u}"
    );
}

#[test]
fn pretty_related_shows_target_updated_dates_recency_colors_and_next() {
    fn set_dates(dir: &Path, id: &str, created: &str, updated: &str) {
        let path = dir.join("tasks").join(format!("{id}.md"));
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| {
                if line.starts_with("created: ") {
                    format!("created: {created}")
                } else if line.starts_with("updated: ") {
                    format!("updated: {updated}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(&path, text).unwrap();
    }

    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let p = id_of(env.json(&aa, &["add", "Parent"]));
    let dep = id_of(env.json(&aa, &["add", "Dep"]));
    let w = id_of(env.json(&aa, &["add", "W"]));
    let x = id_of(env.json(&bb, &["add", "Foreign Rel"]));
    let s = id_of(env.json(&aa, &["add", "S"]));
    env.json(&aa, &["done", &dep]);
    let a = id_of(env.json(
        &aa,
        &["add", "Subject", "-p", "1", "--parent", &p, "--source", &s],
    ));
    env.json(&aa, &["dep", &a, "--on", &dep]);
    let c = id_of(env.json(&aa, &["add", "Child", "--parent", &a]));
    relate_by_hand(&aa, &a, &[&x, "bb-ffffff"]);
    wake_by_hand(&aa, &a, &[&w]);

    let today = env.json(&aa, &["show", &s])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    set_dates(&aa, &p, "2019-01-01T00:00:00Z", "2026-01-10T00:00:00Z");
    set_dates(&aa, &dep, "2019-02-02T00:00:00Z", "2026-02-11T00:00:00Z");
    set_dates(&aa, &w, "2019-03-03T00:00:00Z", "2026-03-12T00:00:00Z");
    set_dates(&bb, &x, "2019-04-04T00:00:00Z", "2020-01-01T00:00:00Z");
    set_dates(
        &aa,
        &s,
        "2019-05-05T00:00:00Z",
        &format!("{today}T12:00:00Z"),
    );
    set_dates(&aa, &c, "2019-06-06T00:00:00Z", "2026-04-13T00:00:00Z");
    set_dates(&aa, &a, "2022-07-07T00:00:00Z", "2026-05-14T00:00:00Z");

    let shown = env.json(&aa, &["show", &a]);
    assert_eq!(shown["parent"]["updated"], "2026-01-10T00:00:00Z");
    assert_eq!(shown["depends_on"][0]["updated"], "2026-02-11T00:00:00Z");
    assert_eq!(shown["wake_targets"][0]["updated"], "2026-03-12T00:00:00Z");
    assert_eq!(shown["related_to"][0]["updated"], "2020-01-01T00:00:00Z");
    assert!(shown["related_to"][1].get("updated").is_none());
    assert_eq!(
        shown["source_task"]["updated"],
        format!("{today}T12:00:00Z")
    );
    assert_eq!(shown["children"][0]["updated"], "2026-04-13T00:00:00Z");

    let pretty = env.pretty(&aa, &["show", &a]);
    assert!(
        !pretty.contains('\x1b'),
        "color-off output must not contain ANSI escapes: {pretty:?}"
    );
    let related_section = pretty
        .split_once("── related ")
        .expect("related section present")
        .1;
    let lines: Vec<&str> = related_section
        .lines()
        .skip(1)
        .take_while(|l| !l.is_empty())
        .collect();
    assert_eq!(
        lines,
        vec![
            format!("parent      {p}  todo     2026-01-10  Parent"),
            format!("depends on  {dep}  done     2026-02-11  Dep"),
            format!("wake on     {w}  todo     2026-03-12  W"),
            format!("related     {x}  todo     2020-01-01  Foreign Rel"),
            "related     bb-ffffff  ?                    (unresolved)".to_string(),
            format!("source      {s}  todo     {today}  S"),
            format!("child       {c}  todo     2026-04-13  Child"),
        ],
        "{related_section}"
    );
    for forbidden in [
        "2019-01-01",
        "2019-02-02",
        "2019-03-03",
        "2019-04-04",
        "2019-05-05",
        "2019-06-06",
        "2026-05-14",
    ] {
        assert!(
            !related_section.contains(forbidden),
            "related rows must use target updated date, not {forbidden}: {related_section}"
        );
    }

    let colored_out = env
        .cmd(&aa)
        .args(["--pretty", "--color", "always", "show", &a])
        .output()
        .unwrap();
    assert!(colored_out.status.success());
    let colored = String::from_utf8(colored_out.stdout).unwrap();
    assert!(
        colored.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m  S")),
        "recent related date should be painted full cyan: {colored:?}"
    );
    assert!(
        colored.contains("\x1b[38;2;125;125;115m2020-01-01\x1b[0m  Foreign Rel"),
        "old related date should be painted dim: {colored:?}"
    );
    assert!(
        colored
            .contains("\x1b[2mbb-ffffff\x1b[0m  \x1b[2m?      \x1b[0m              (unresolved)"),
        "unresolved row keeps chrome styling and blank 10-char date cell: {colored:?}"
    );

    env.json(&aa, &["edit", &c, "--no-parent"]);
    let next_pretty = env.pretty(&aa, &["next"]);
    assert!(
        next_pretty.contains(&format!(
            "related     {x}  todo     2020-01-01  Foreign Rel"
        )) && next_pretty.contains(&format!("source      {s}  todo     {today}  S")),
        "next --pretty must share related date rendering: {next_pretty}"
    );
}

#[test]
fn check_refuses_related_on_a_tombstone() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    env.init("bb");
    let t = id_of(env.json(&dir, &["add", "Task"]));
    let other = id_of(env.json(&dir, &["add", "Other"]));
    env.json(&dir, &["move", &t, "--to", "bb"]);

    relate_by_hand(&dir, &t, &[&other]);

    let (rc, findings) = common::check_findings(&env, &dir);
    assert_eq!(rc, 1);
    let errors = findings["errors"].as_array().unwrap();
    let error_kinds: Vec<_> = errors.iter().map(|f| f["kind"].as_str().unwrap()).collect();
    assert_eq!(
        error_kinds
            .iter()
            .filter(|k| **k == "related_status")
            .count(),
        1,
        "{findings}"
    );
    assert!(!error_kinds.contains(&"parse"), "{findings}");
    let warnings = findings["warnings"].as_array().unwrap();
    let warning_kinds: Vec<_> = warnings
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    assert!(!warning_kinds.contains(&"parse"), "{findings}");

    let path = dir.join(format!("tasks/{t}.md"));
    let before = std::fs::read(&path).unwrap();
    assert_eq!(env.fail(&dir, &["note", &t, "x"]), "moved");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn check_warns_dangling_related_only_for_local_ids() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    common::configure_color(&dir);
    let _other_dir = env.init("bb");
    common::configure_color(&_other_dir);
    let a = id_of(env.json(&dir, &["add", "Task A"]));
    let moved_src = id_of(env.json(&dir, &["add", "Moved Source"]));
    env.json(&dir, &["move", &moved_src, "--to", "bb"]);

    let missing_local = "aa-ffffff";
    let unreachable_foreign = "cc-111111";

    relate_by_hand(&dir, &a, &[missing_local, unreachable_foreign, &moved_src]);

    let (rc, findings) = common::check_findings(&env, &dir);
    assert_eq!(rc, 0, "check should exit 0 with only warnings: {findings}");
    let errors = findings
        .get("errors")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(errors.is_empty(), "expected no errors: {errors:?}");

    let warnings = findings
        .get("warnings")
        .and_then(|w| w.as_array())
        .cloned()
        .unwrap_or_default();
    let warning_kinds: Vec<_> = warnings
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        warning_kinds
            .iter()
            .filter(|k| **k == "dangling_related")
            .count(),
        1,
        "{findings}"
    );
    assert_eq!(
        warning_kinds.len(),
        1,
        "expected only dangling_related warning: {warnings:?}"
    );
}

#[test]
fn renewing_a_shelf_keeps_dropped_targets_as_related() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let x = id_of(env.json(&dir, &["add", "X"]));
    let y = id_of(env.json(&dir, &["add", "Y"]));
    let s = id_of(env.json(&dir, &["add", "S"]));
    env.json(
        &dir,
        &[
            "shelve",
            &s,
            "wait on X and Y",
            "--defer",
            "30d",
            "--wake-on",
            &x,
            "--wake-on",
            &y,
        ],
    );
    env.json(&dir, &["done", &x]);

    env.json(
        &dir,
        &[
            "shelve",
            &s,
            "wait on Y only",
            "--defer",
            "30d",
            "--wake-on",
            &y,
        ],
    );
    let shown = env.json(&dir, &["show", &s]);
    assert_eq!(shown["task"]["related"], json!([x]));
    assert_eq!(shown["task"]["wake_on"], json!([y]));

    let control = id_of(env.json(&dir, &["add", "Control"]));
    env.json(
        &dir,
        &[
            "shelve",
            &control,
            "wait on Y only",
            "--defer",
            "30d",
            "--wake-on",
            &y,
        ],
    );
    let control_shown = env.json(&dir, &["show", &control]);
    assert_eq!(shown["review_reason"], control_shown["review_reason"]);
}

#[test]
fn leaving_a_shelf_carries_every_target() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let x = id_of(env.json(&dir, &["add", "X"]));
    let y = id_of(env.json(&dir, &["add", "Y"]));

    // unshelve
    let s1 = id_of(env.json(&dir, &["add", "S1"]));
    env.json(
        &dir,
        &[
            "shelve",
            &s1,
            "wait",
            "--defer",
            "30d",
            "--wake-on",
            &x,
            "--wake-on",
            &y,
        ],
    );
    env.json(&dir, &["unshelve", &s1]);
    let shown1 = env.json(&dir, &["show", &s1]);
    assert_eq!(shown1["task"]["related"], json!([x, y]));
    assert!(shown1["task"].get("wake_on").is_none() || shown1["task"]["wake_on"] == json!([]));

    // drop
    let s2 = id_of(env.json(&dir, &["add", "S2"]));
    env.json(
        &dir,
        &[
            "shelve",
            &s2,
            "wait",
            "--defer",
            "30d",
            "--wake-on",
            &x,
            "--wake-on",
            &y,
        ],
    );
    env.json(&dir, &["drop", &s2]);
    let shown2 = env.json(&dir, &["show", &s2]);
    assert_eq!(shown2["task"]["related"], json!([x, y]));
    assert!(shown2["task"].get("wake_on").is_none() || shown2["task"]["wake_on"] == json!([]));

    // done
    let s3 = id_of(env.json(&dir, &["add", "S3"]));
    env.json(
        &dir,
        &[
            "shelve",
            &s3,
            "wait",
            "--defer",
            "30d",
            "--wake-on",
            &x,
            "--wake-on",
            &y,
        ],
    );
    env.json(&dir, &["done", &s3]);
    let shown3 = env.json(&dir, &["show", &s3]);
    assert_eq!(shown3["task"]["related"], json!([x, y]));
    assert!(shown3["task"].get("wake_on").is_none() || shown3["task"]["wake_on"] == json!([]));
}

#[test]
fn a_target_already_related_is_not_duplicated() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");

    // Case 1: the same spelling
    let x = id_of(env.json(&aa, &["add", "X"]));
    let y = id_of(env.json(&aa, &["add", "Y"]));
    let s1 = id_of(env.json(&aa, &["add", "S1"]));
    relate_by_hand(&aa, &s1, &[&x]);
    env.json(
        &aa,
        &[
            "shelve",
            &s1,
            "wait",
            "--defer",
            "30d",
            "--wake-on",
            &x,
            "--wake-on",
            &y,
        ],
    );
    env.json(&aa, &["unshelve", &s1]);
    let shown1 = env.json(&aa, &["show", &s1]);
    assert_eq!(shown1["task"]["related"], json!([x, y]));

    // Case 2: an alias spelling
    let target = id_of(env.json(&bb, &["add", "Target"]));
    let s2 = id_of(env.json(&aa, &["add", "S2"]));
    env.json(
        &aa,
        &[
            "shelve",
            &s2,
            "wait",
            "--defer",
            "30d",
            "--wake-on",
            &target,
        ],
    );
    env.json(&bb, &["rename", "bb", "bc"]);
    let canonical_target = target.replacen("bb-", "bc-", 1);
    relate_by_hand(&aa, &s2, &[&canonical_target]);
    env.json(&aa, &["unshelve", &s2]);
    let shown2 = env.json(&aa, &["show", &s2]);
    assert_eq!(shown2["task"]["related"], json!([canonical_target]));
}

#[test]
fn a_new_note_citation_links_the_task() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let target = id_of(env.json(&dir, &["add", "Target"]));
    let citer = id_of(env.json(&dir, &["add", "Citer"]));

    // Older prose (a note) mentioning target before it was linked as related
    let path = dir.join("tasks").join(format!("{citer}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "\nnotes:",
        &format!("\nnotes:\n  - 2026-09-09T00:00:00Z (test): older prose mentioning {target}"),
        1,
    );
    std::fs::write(&path, text).unwrap();

    // Confirm related is absent initially
    let shown_before = env.json(&dir, &["show", &citer]);
    assert!(shown_before["task"].get("related").is_none());

    // A new note citing the target links it
    env.json(
        &dir,
        &["note", &citer, &format!("new note citing {target}")],
    );
    let shown_after = env.json(&dir, &["show", &citer]);
    assert_eq!(shown_after["task"]["related"], json!([target]));
}

#[test]
fn held_child_and_own_ids_are_not_linked() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let dep = id_of(env.json(&dir, &["add", "Dep"]));
    let wake_target = id_of(env.json(&dir, &["add", "Wake Target"]));
    let source_task = id_of(env.json(&dir, &["add", "Source Task"]));
    let parent = id_of(env.json(
        &dir,
        &["add", "Parent", "--depends", &dep, "--source", &source_task],
    ));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &parent]));
    env.json(&dir, &["shelve", &child, "child waiting", "--defer", "30d"]);
    env.json(
        &dir,
        &[
            "shelve",
            &parent,
            "waiting",
            "--defer",
            "30d",
            "--wake-on",
            &wake_target,
        ],
    );

    let text = format!("citing {parent}, {child}, {dep}, {wake_target}, {source_task}");
    env.json(&dir, &["note", &parent, &text]);

    let shown = env.json(&dir, &["show", &parent]);
    assert!(shown["task"].get("related").is_none());
}

#[test]
fn an_unresolvable_citation_warns_and_is_not_linked() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let t = id_of(env.json(&dir, &["add", "Task"]));

    let out = env.json(&dir, &["note", &t, "see aa-ffffff"]);
    let warnings = out["warnings"].as_array().unwrap();
    let expected = format!("{t} cites aa-ffffff, which resolves to no task here; not linked");
    assert!(
        warnings.iter().any(|w| w.as_str() == Some(&expected)),
        "expected unresolvable warning, got: {warnings:?}"
    );

    let shown = env.json(&dir, &["show", &t]);
    assert!(shown["task"].get("related").is_none());
}

#[test]
fn removed_links_stay_removed_until_cited_again() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let target = id_of(env.json(&dir, &["add", "Target"]));
    let t = id_of(env.json(&dir, &["add", "Task", "--related", &target]));
    assert_eq!(
        env.json(&dir, &["show", &t])["task"]["related"],
        json!([target])
    );

    env.json(&dir, &["edit", &t, "--rm-related", &target]);
    assert!(
        env.json(&dir, &["show", &t])["task"]
            .get("related")
            .is_none()
    );

    env.json(&dir, &["note", &t, "unrelated progress"]);
    assert!(
        env.json(&dir, &["show", &t])["task"]
            .get("related")
            .is_none()
    );

    env.json(&dir, &["note", &t, &format!("revisiting {target}")]);
    assert_eq!(
        env.json(&dir, &["show", &t])["task"]["related"],
        json!([target])
    );
}

#[test]
fn explicit_changes_win_over_citations() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let x = id_of(env.json(&dir, &["add", "X"]));
    let y = id_of(env.json(&dir, &["add", "Y"]));
    let z = id_of(env.json(&dir, &["add", "Z"]));

    // 1. --rm-related X together with a body citing X
    let t1 = id_of(env.json(&dir, &["add", "T1", "--related", &x]));
    env.json(
        &dir,
        &[
            "edit",
            &t1,
            "--rm-related",
            &x,
            "--body",
            &format!("body cites {x}"),
        ],
    );
    assert!(
        env.json(&dir, &["show", &t1])["task"]
            .get("related")
            .is_none()
    );

    // 2. --no-related --body "... X", which leaves related empty
    let t2 = id_of(env.json(&dir, &["add", "T2", "--related", &x]));
    env.json(
        &dir,
        &[
            "edit",
            &t2,
            "--no-related",
            "--body",
            &format!("body cites {x}"),
        ],
    );
    assert!(
        env.json(&dir, &["show", &t2])["task"]
            .get("related")
            .is_none()
    );

    // 3. Editor save that changes related and adds a body citation, which keeps exactly the saved list
    let t3 = id_of(env.json(&dir, &["add", "T3", "--related", &x]));
    let script3 = format!(
        "sed -i 's/related: \\[{x}\\]/related: \\[{y}\\]/' \"$1\"\n\
         sed -i 's/^## Notes/cites {z}\\n\\n## Notes/' \"$1\""
    );
    let ed3 = editor_script(&dir, &script3);
    env.cmd(&dir)
        .env("EDITOR", &ed3)
        .args(["edit", &t3, "--editor"])
        .output()
        .unwrap();
    let shown3 = env.json(&dir, &["show", &t3]);
    assert_eq!(shown3["task"]["related"], json!([y]));

    // 4. Editor save that leaves related as stored, which links its new body citation
    let t4 = id_of(env.json(&dir, &["add", "T4", "--related", &x]));
    let script4 = format!("sed -i 's/^## Notes/cites {z}\\n\\n## Notes/' \"$1\"");
    let ed4 = editor_script(&dir, &script4);
    env.cmd(&dir)
        .env("EDITOR", &ed4)
        .args(["edit", &t4, "--editor"])
        .output()
        .unwrap();
    let shown4 = env.json(&dir, &["show", &t4]);
    assert_eq!(shown4["task"]["related"], json!([x, z]));
}

#[test]
fn moved_citations_link_live_destinations_once() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");

    // Case 1: X moved to Y, with Y already related: nothing appended
    let x = id_of(env.json(&aa, &["add", "X"]));
    let y = id_of(env.json(&aa, &["move", &x, "--to", "bb"]));
    let t1 = id_of(env.json(&aa, &["add", "T1", "--related", &y]));
    env.json(&aa, &["note", &t1, &format!("see {x}")]);
    assert_eq!(env.json(&aa, &["show", &t1])["task"]["related"], json!([y]));

    // Case 2: two ids whose chains converge on Y: Y appended once
    let a_chain = id_of(env.json(&aa, &["add", "Chain A"]));
    let b_chain = id_of(env.json(&aa, &["move", &a_chain, "--to", "bb"]));
    let c_chain = id_of(env.json(&bb, &["move", &b_chain, "--to", "cc"]));
    let t2 = id_of(env.json(&aa, &["add", "T2"]));
    env.json(&aa, &["note", &t2, &format!("see {a_chain} and {b_chain}")]);
    assert_eq!(
        env.json(&aa, &["show", &t2])["task"]["related"],
        json!([c_chain])
    );

    // Case 3: A to B to C, with a note on C citing A: nothing appended, exit 0
    let out3 = env.json(&cc, &["note", &c_chain, &format!("see {a_chain}")]);
    assert!(out3["warnings"].as_array().unwrap().is_empty());
    assert!(
        env.json(&cc, &["show", &c_chain])["task"]
            .get("related")
            .is_none()
    );

    // Case 4: a cited X held in depends as a tombstone, whose successor is not held: nothing appended
    let x_tomb = id_of(env.json(&aa, &["add", "XTomb"]));
    let t4 = id_of(env.json(&aa, &["add", "T4", "--depends", &x_tomb]));
    let y_succ = id_of(env.json(&aa, &["move", &x_tomb, "--to", "bb"]));
    env.json(&aa, &["note", &t4, &format!("see {x_tomb}")]);
    assert!(
        env.json(&aa, &["show", &t4])["task"]
            .get("related")
            .is_none()
    );

    // Case 5: a newly cited moved id with nothing held: the successor appended
    let t5 = id_of(env.json(&aa, &["add", "T5"]));
    env.json(&aa, &["note", &t5, &format!("see {x_tomb}")]);
    assert_eq!(
        env.json(&aa, &["show", &t5])["task"]["related"],
        json!([y_succ])
    );
}

#[test]
fn tool_generated_notes_and_unchanged_bodies_link_nothing() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let target = id_of(env.json(&aa, &["add", "Target"]));

    // Move note: moving a task generates a note in the destination with the source id
    let moved_succ = id_of(env.json(&aa, &["move", &target, "--to", "bb"]));
    let shown_succ = env.json(&bb, &["show", &moved_succ]);
    assert!(shown_succ["task"].get("related").is_none());

    // Body reworded with the same ids
    let t = id_of(env.json(&aa, &["add", "T", "--body", &format!("see {moved_succ}")]));
    assert_eq!(
        env.json(&aa, &["show", &t])["task"]["related"],
        json!([moved_succ])
    );
    env.json(&aa, &["edit", &t, "--rm-related", &moved_succ]);
    assert!(
        env.json(&aa, &["show", &t])["task"]
            .get("related")
            .is_none()
    );
    env.json(
        &aa,
        &[
            "edit",
            &t,
            "--body",
            &format!("please see {moved_succ} again"),
        ],
    );
    assert!(
        env.json(&aa, &["show", &t])["task"]
            .get("related")
            .is_none()
    );

    // Migrate note
    let mig = id_of(env.json(&bb, &["add", "Migrate Target"]));
    env.json(&bb, &["rename", "bb", "bx"]);
    let shown_mig = env.json(&bb, &["show", &mig.replacen("bb-", "bx-", 1)]);
    assert!(shown_mig["task"].get("related").is_none());
}

#[test]
fn citations_are_found_through_punctuation_markdown_and_paths() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let target = id_of(env.json(&dir, &["add", "Target"]));
    let t = id_of(env.json(&dir, &["add", "Task"]));

    // Own attachment path: tasks/files/{t}/x.png should NOT link {t} (own id skipped)
    // Wrapped forms: ({target}), {target}., `{target}`, **{target}**, tasks/files/{target}/x.png
    let text = format!(
        "own: tasks/files/{t}/x.png, wrapped: ({target}), {target}., `{target}`, **{target}**, tasks/files/{target}/x.png"
    );
    env.json(&dir, &["note", &t, &text]);

    let shown = env.json(&dir, &["show", &t]);
    assert_eq!(shown["task"]["related"], json!([target]));
}

#[test]
fn lookalike_tokens_are_not_citations() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let target = id_of(env.json(&dir, &["add", "Target"]));
    let t = id_of(env.json(&dir, &["add", "Task"]));

    let bare_hex = &target[3..];
    let upper = target.to_uppercase();
    let text = format!("foo-{target} {target}-x {upper} {bare_hex} 2026-11-07 zz-123456");
    let out = env.json(&dir, &["note", &t, &text]);

    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings.is_empty(),
        "expected no warnings for lookalikes, got: {warnings:?}"
    );

    let shown = env.json(&dir, &["show", &t]);
    assert!(shown["task"].get("related").is_none());
}

#[test]
fn a_stdin_batch_links_each_citation_once_in_order() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let x = id_of(env.json(&dir, &["add", "X"]));
    let y = id_of(env.json(&dir, &["add", "Y"]));
    let t = id_of(env.json(&dir, &["add", "Task"]));

    let stdin_input =
        format!("line 1 cites {x}\nline 2 cites {y} and {x}\nline 3 cites {y} and {x}\n");
    let out = env
        .cmd(&dir)
        .args(["note", &t, "--stdin"])
        .write_stdin(stdin_input)
        .output()
        .unwrap();
    assert!(out.status.success());

    let shown = env.json(&dir, &["show", &t]);
    assert_eq!(shown["task"]["related"], json!([x, y]));
}

#[test]
fn an_unreachable_citation_warns_and_the_note_lands() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let bb_target = id_of(env.json(&bb, &["add", "BB Target"]));
    let t = id_of(env.json(&aa, &["add", "Task"]));

    std::fs::remove_dir_all(&bb).unwrap();

    let out = env.json(&aa, &["note", &t, &format!("see {bb_target}")]);
    let warnings = out["warnings"].as_array().unwrap();
    let expected = format!("{t} cites {bb_target}, which resolves to no task here; not linked");
    assert!(
        warnings.iter().any(|w| w.as_str() == Some(&expected)),
        "expected unresolvable/unreachable warning, got: {warnings:?}"
    );

    let shown = env.json(&aa, &["show", &t]);
    assert!(shown["task"].get("related").is_none());
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert!(
        notes.last().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains(&bb_target)
    );
}

#[test]
fn every_supplied_text_hook_links() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let target = id_of(env.json(&dir, &["add", "Target"]));

    // 1. add -b
    let t1 = id_of(env.json(&dir, &["add", "T1", "-b", &format!("mentions {target}")]));
    assert_eq!(
        env.json(&dir, &["show", &t1])["task"]["related"],
        json!([target])
    );

    // 2. edit --body
    let t2 = id_of(env.json(&dir, &["add", "T2"]));
    env.json(
        &dir,
        &["edit", &t2, "--body", &format!("mentions {target}")],
    );
    assert_eq!(
        env.json(&dir, &["show", &t2])["task"]["related"],
        json!([target])
    );

    // 3. note
    let t3 = id_of(env.json(&dir, &["add", "T3"]));
    env.json(&dir, &["note", &t3, &format!("mentions {target}")]);
    assert_eq!(
        env.json(&dir, &["show", &t3])["task"]["related"],
        json!([target])
    );

    // 4. park
    let t4 = id_of(env.json(&dir, &["add", "T4"]));
    env.json(
        &dir,
        &[
            "park",
            &t4,
            &format!("next: inspect {target}"),
            "--waiting-on",
            "user",
        ],
    );
    assert_eq!(
        env.json(&dir, &["show", &t4])["task"]["related"],
        json!([target])
    );

    // 5. done
    let t5 = id_of(env.json(&dir, &["add", "T5"]));
    env.json(&dir, &["done", &t5, &format!("done, see {target}")]);
    assert_eq!(
        env.json(&dir, &["show", &t5])["task"]["related"],
        json!([target])
    );

    // 6. drop
    let t6 = id_of(env.json(&dir, &["add", "T6"]));
    env.json(&dir, &["drop", &t6, &format!("drop, see {target}")]);
    assert_eq!(
        env.json(&dir, &["show", &t6])["task"]["related"],
        json!([target])
    );

    // 7. block
    let t7 = id_of(env.json(&dir, &["add", "T7"]));
    env.json(&dir, &["block", &t7, &format!("blocked by {target}")]);
    assert_eq!(
        env.json(&dir, &["show", &t7])["task"]["related"],
        json!([target])
    );

    // 8. shelve
    let t8 = id_of(env.json(&dir, &["add", "T8"]));
    env.json(
        &dir,
        &[
            "shelve",
            &t8,
            &format!("wake when {target} arrives"),
            "--defer",
            "30d",
        ],
    );
    assert_eq!(
        env.json(&dir, &["show", &t8])["task"]["related"],
        json!([target])
    );
}
