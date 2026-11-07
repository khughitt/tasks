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
    assert_eq!(
        shown["related_to"],
        json!([{"id": b, "title": "B", "status": "done", "resolved": true}])
    );
    assert_eq!(
        shown["wake_targets"],
        json!([{"id": w, "title": "W", "status": "todo", "resolved": true}])
    );
    assert_eq!(
        shown["source_task"],
        json!({"id": s, "title": "S", "status": "todo", "resolved": true})
    );
}

#[test]
fn rows_resolve_across_projects_without_a_full_scan() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let a = id_of(env.json(&aa, &["add", "A"]));
    let x = id_of(env.json(&bb, &["add", "X"]));
    relate_by_hand(&aa, &a, &[&x]);

    let shown = env.json(&aa, &["show", &a]);
    assert_eq!(shown["related_to"][0]["id"], x);
    assert_eq!(shown["related_to"][0]["title"], "X");
    assert_eq!(shown["related_to"][0]["status"], "todo");
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
    assert_eq!(shown["source_task"]["resolved"], false);
    assert_eq!(shown["source_task"]["id"], "aa-eeeeee");

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

    let shown_a = env.json(&aa, &["show", &a]);
    assert_eq!(
        shown_a["related_to"][0],
        json!({"id": b, "title": "B", "status": "moved", "resolved": true})
    );

    let shown_c = env.json(&aa, &["show", &c]);
    assert_eq!(
        shown_c["depends_on"][0],
        json!({"id": b, "title": "B", "status": "moved", "resolved": true})
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

    let next_task = &next_out["next"];
    assert_eq!(next_task["task"]["id"], a);
    assert_eq!(
        next_task["related_to"],
        json!([{"id": b, "title": "B", "status": "todo", "resolved": true}])
    );
    assert_eq!(
        next_task["wake_targets"],
        json!([{"id": w, "title": "W", "status": "todo", "resolved": true}])
    );
    assert_eq!(
        next_task["source_task"],
        json!({"id": s, "title": "S", "status": "todo", "resolved": true})
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
    let _other_dir = env.init("bb");
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
