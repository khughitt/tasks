mod common;

use common::TestEnv;
use serde_json::Value;
use std::path::Path;

fn record(root: &Path, id: &str, status: &str, body: &str, notes: &[&str]) -> usize {
    let mut raw = format!(
        "---\nid: dot-{id}\ntitle: Growth\nstatus: {status}\npriority: 2\ndepends: []\ntags: []\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\n---\n\n{body}\n\n## Notes\n\n"
    );
    for text in notes {
        raw.push_str(&format!("- 2026-10-01T00:00:00Z (tester): {text}\n"));
    }
    std::fs::write(root.join(format!("tasks/dot-{id}.md")), &raw).unwrap();
    raw.len()
}

fn config(root: &Path, extra: &str) {
    std::fs::write(
        root.join("tasks/.config.toml"),
        format!("prefix = \"dot\"\n{extra}"),
    )
    .unwrap();
}

fn growth_warnings(report: &Value) -> Vec<&Value> {
    report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == "record_growth")
        .collect()
}

#[test]
fn growth_boundary_config_and_output() {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    assert_eq!(
        env.json(&root, &["check", "--growth"])["growth"],
        serde_json::json!([])
    );
    env.cmd(&root).arg("check").assert().success().stdout("");
    let completion = env
        .cmd(&root)
        .env("TASKS_COMPLETE", "zsh")
        .env("_CLAP_COMPLETE_INDEX", "2")
        .args(["--", "tasks", "check", "--gr"])
        .output()
        .unwrap();
    assert!(completion.status.success());
    assert!(String::from_utf8_lossy(&completion.stdout).contains("--growth"));

    let bytes = record(&root, "a00001", "todo", "é", &["same", "same"]);
    config(&root, &format!("[growth]\nwarn_bytes = {bytes}\n"));
    let at = env.json(&root, &["check", "--growth"]);
    assert!(growth_warnings(&at).is_empty());
    assert_eq!(at["growth"][0]["record_bytes"], bytes);
    assert_eq!(at["growth"][0]["body_bytes"], 2);
    assert_eq!(at["growth"][0]["note_count"], 2);
    assert_eq!(at["growth"][0]["note_text_bytes"], 8);
    assert_eq!(at["growth"][0]["duplicate_note_text_bytes"], 4);
    assert_eq!(at["growth"][0]["archived_bytes"], 0);
    config(&root, &format!("[growth]\nwarn_bytes = {}\n", bytes - 1));
    let over = env.json(&root, &["check", "--growth"]);
    let warnings = growth_warnings(&over);
    assert_eq!(warnings.len(), 1);
    let detail = warnings[0]["detail"].as_str().unwrap();
    assert!(detail.contains(&bytes.to_string()), "{detail}");
    assert!(detail.contains(&(bytes - 1).to_string()), "{detail}");
    assert!(detail.contains("attach"), "{detail}");
    assert!(!detail.contains("edit"), "{detail}");
    assert!(
        detail.contains("tasks archive-notes dot-a00001"),
        "{detail}"
    );
    assert!(!detail.contains("--expect"), "{detail}");
    assert!(env.check(&root).get("growth").is_none());
    let pretty = env.pretty(&root, &["check", "--growth"]);
    for label in [
        "record bytes",
        "body bytes",
        "notes",
        "note text bytes",
        "duplicate bytes",
        "archived bytes",
    ] {
        assert!(pretty.contains(label), "{pretty}");
    }
    // Ordinary lifecycle writes remain usable above the threshold.
    env.json(&root, &["start", "dot-a00001"]);
    env.json(&root, &["park", "dot-a00001", "continue"]);
    env.json(&root, &["done", "dot-a00001", "done"]);

    for extra in [
        "[growth]\nwarn_bytes = 0",
        "[growth]\nwarn_bytes = -1",
        "[growth]\nwarn_bytes = 1.5",
        "[growth]\nwarn_bytes = \"1\"",
        "[growth]\nunknown = 1",
    ] {
        config(&root, extra);
        assert_eq!(env.fail(&root, &["check"]), "config");
    }
    config(
        &root,
        "[attachments]\nmax_bytes = 1\n[growth]\nwarn_bytes = 999999\n",
    );
    assert!(growth_warnings(&env.json(&root, &["check", "--growth"])).is_empty());
    env.json(&root, &["init", "--prefix", "dot", "--force"]);
    assert!(
        std::fs::read_to_string(root.join("tasks/.config.toml"))
            .unwrap()
            .contains("999999")
    );
    env.json(&root, &["rename", "dot", "new"]);
    assert!(
        std::fs::read_to_string(root.join("tasks/.config.toml"))
            .unwrap()
            .contains("999999")
    );
}

#[test]
fn growth_default_and_attachment_cap_boundaries() {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    for (extra, threshold) in [
        ("", 65536),
        ("[attachments]\nmax_bytes = 100", 25),
        ("[attachments]\nmax_bytes = 1", 1),
    ] {
        config(&root, extra);
        let base = record(&root, "a00001", "todo", "", &[]);
        if threshold >= base {
            record(&root, "a00001", "todo", &"x".repeat(threshold - base), &[]);
            assert!(growth_warnings(&env.json(&root, &["check", "--growth"])).is_empty());
            record(
                &root,
                "a00001",
                "todo",
                &"x".repeat(threshold + 1 - base),
                &[],
            );
        }
        let report = env.json(&root, &["check", "--growth"]);
        let warnings = growth_warnings(&report);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("threshold {threshold} bytes")),
            "{report}"
        );
    }
}

#[test]
fn growth_duplicate_key() {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    record(
        &root,
        "a00001",
        "todo",
        "",
        &["é \t", "é", "é\t", " é", "x y", "x  y"],
    );
    let before = env.json(&root, &["check", "--growth"]);
    assert_eq!(before["growth"][0]["duplicate_note_text_bytes"], 5);
    let note = env.json(&root, &["note", "dot-a00001", "é \t"]);
    let advisory = note["warnings"].as_array().unwrap();
    assert_eq!(advisory.len(), 1, "{note}");
    let text = advisory[0].as_str().unwrap();
    assert!(text.contains("note 1"), "{text}");
    assert!(text.contains("2 bytes"), "{text}");
    assert!(text.contains("landed"), "{text}");
    assert!(!text.contains('é'), "{text}");
    let after = env.json(&root, &["show", "dot-a00001"]);
    assert_eq!(after["task"]["notes"].as_array().unwrap().len(), 7);
    assert!(
        env.json(&root, &["note", "dot-a00001", "changed"])["warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        env.json(&root, &["note", "dot-a00001", " é"])["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("note 4"))
    );
    // Automatic lifecycle notes do not use the explicit-note advisory.
    env.json(&root, &["start", "dot-a00001"]);
    let park = env.json(&root, &["park", "dot-a00001", "repeat"]);
    env.json(&root, &["start", "dot-a00001"]);
    let second = env.json(&root, &["park", "dot-a00001", "repeat"]);
    assert_eq!(park["warnings"], second["warnings"]);
}

#[test]
fn growth_unique_and_invalid_records() {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    config(&root, "[growth]\nwarn_bytes = 1");
    record(&root, "a00001", "done", "", &[&"é".repeat(2000)]);
    record(&root, "a00002", "shelved", &"body".repeat(2000), &[]);
    record(&root, "a00003", "todo", "", &[]);
    record(&root, "a00004", "todo", "", &[]);
    let report = env.json(&root, &["check", "--growth"]);
    let rows = report["growth"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["dot-a00002", "dot-a00001", "dot-a00003", "dot-a00004"]
    );
    assert_eq!(rows[1]["duplicate_note_text_bytes"], 0);
    assert_eq!(rows[1]["note_text_bytes"], 4000);
    let warnings = growth_warnings(&report);
    assert_eq!(warnings.len(), 4);
    let body_dominated = warnings[0]["detail"].as_str().unwrap();
    assert_eq!(warnings[0]["id"], "dot-a00002");
    assert!(
        body_dominated.contains("tasks attach")
            && body_dominated.contains("tasks archive-notes dot-a00002")
            && body_dominated.contains(
                "preserve exact body bytes before using `tasks edit dot-a00002 --body` to shorten a large body"
            ),
        "{body_dominated}"
    );
    let note_dominated = warnings[1]["detail"].as_str().unwrap();
    assert_eq!(warnings[1]["id"], "dot-a00001");
    assert!(
        note_dominated.contains("tasks attach")
            && note_dominated.contains("tasks archive-notes dot-a00001")
            && !note_dominated.contains("--body")
            && !note_dominated.contains("shorten a large body"),
        "{note_dominated}"
    );
    // Exact half-record boundary: body_bytes * 2 < record_bytes omits --body; >= includes it.
    let base = record(&root, "a00003", "todo", "", &[]);
    record(&root, "a00003", "todo", &"x".repeat(base - 1), &[]);
    record(&root, "a00004", "todo", &"x".repeat(base), &[]);
    let boundary = env.json(&root, &["check", "--growth"]);
    let boundary_warnings = growth_warnings(&boundary);
    let under_half = boundary_warnings
        .iter()
        .find(|w| w["id"] == "dot-a00003")
        .unwrap()["detail"]
        .as_str()
        .unwrap();
    let at_half = boundary_warnings
        .iter()
        .find(|w| w["id"] == "dot-a00004")
        .unwrap()["detail"]
        .as_str()
        .unwrap();
    assert!(!under_half.contains("--body"), "{under_half}");
    assert!(
        at_half.contains("tasks edit dot-a00004 --body"),
        "{at_half}"
    );
    std::fs::write(root.join("tasks/dot-a00005.md"), "broken record").unwrap();
    let out = env.cmd(&root).args(["check", "--growth"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let broken: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(broken["growth"].as_array().unwrap().len(), 4);
    assert!(!broken["errors"].as_array().unwrap().is_empty());
}
