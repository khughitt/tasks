mod common;
use common::retained::RetainedEnv;
use serde_json::json;

#[test]
fn retained_state_exact_basis() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let old = f.basis();
    let mut r = f.record();
    r["purpose"] = json!("Second review");
    f.note(r);
    let s = f.state();
    assert!(s["complete"].as_bool().unwrap());
    assert_eq!(s["items"][0]["record"]["purpose"], "Second review");
    assert_eq!(s["basis"]["commit"], f.basis());
    let old = f.json(&["retained", "state", &f.id, "--at", &old]);
    assert_eq!(old["items"][0]["record"]["purpose"], "Review");
    assert_eq!(
        s["items"][0]["source"]["ordinal"],
        old["items"][0]["source"]["ordinal"].as_u64().unwrap() + 1
    );
}
#[test]
fn retained_state_dirty_current() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let old = f.basis();
    let p = f.records.join(format!("tasks/{}.md", f.id));
    let bytes = std::fs::read_to_string(&p).unwrap();
    std::fs::write(p, format!("{bytes}\n")).unwrap();
    let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let s = f.json(&["retained", "state", &f.id, "--at", &old]);
    assert_eq!(s["basis"]["commit"], old);
}
#[test]
fn retained_state_layouts() {
    for layout in ["in-tree", "detached", "linked"] {
        let f = RetainedEnv::new(layout);
        f.note(f.record());
        let s = f.state();
        assert_eq!(s["basis"]["layout"], layout);
        assert!(
            s["basis"]["task_path"]
                .as_str()
                .unwrap()
                .ends_with(&format!("tasks/{}.md", f.id))
        );
        assert_eq!(
            s["items"][0]["record"]["next_owner"],
            format!("task:{}", f.id)
        );
    }
}
#[test]
fn retained_state_unknown_host() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    f.json(&["note", &f.id, "retained: {\"version\":2}"]);
    let s = f.state();
    assert_eq!(s["complete"], false);
    assert_eq!(s["items"][0]["record"], serde_json::Value::Null);
    let s = f.json(&["retained", "state", &f.id, "--host", "fixture-host"]);
    assert_eq!(s["complete"], false);
    let out = f
        .cmd()
        .args(["retained", "state", &f.id, "--at=--help"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
#[test]
fn retained_state_archives() {
    let f = RetainedEnv::new("in-tree");
    for i in 0..4 {
        let mut r = f.record();
        r["purpose"] = json!(format!("Review {i}"));
        f.note(r);
    }
    let before = f.state();
    let preview = f.json(&["archive-notes", &f.id]);
    assert_eq!(preview["eligible"], true);
    f.json(&[
        "archive-notes",
        &f.id,
        "--expect",
        preview["source_sha256"].as_str().unwrap(),
    ]);
    let after = f.state();
    assert_eq!(before["items"][0]["record"], after["items"][0]["record"]);
    assert_eq!(
        before["items"][0]["source"]["ordinal"],
        after["items"][0]["source"]["ordinal"]
    );
    let archive = std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
        .unwrap()
        .map(|v| v.unwrap().path())
        .find(|p| p.extension().is_some_and(|v| v == "notes"))
        .unwrap();
    std::fs::write(archive, "tampered").unwrap();
    let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

fn op(
    f: &RetainedEnv,
    basis: serde_json::Value,
    phase: &str,
    outputs: serde_json::Value,
) -> serde_json::Value {
    json!({"version":1,"key":{"coordinator":f.id,"operation_id":"0".repeat(32)},"phase":phase,
        "request":{"type":"git","basis":basis,"path":"README.md","sha256":"a".repeat(64)},
        "input_bases":[{"task":f.id,"basis":basis}],"host":"fixture-host","at":"2026-11-03T00:00:00Z","observer":null,"outputs":outputs,"diagnostics":[]})
}
fn cp(f: &RetainedEnv, basis: serde_json::Value) -> serde_json::Value {
    json!({"version":1,"operation":{"coordinator":f.id,"operation_id":"0".repeat(32)},"observed_at":"2026-11-03T00:00:00Z","host":"fixture-host","observer":null,"basis":basis,"scope":{"mode":"all_known_kept_on_host","host":"fixture-host","other_hosts":"outside_observation"},"counts":{"kept_verified":1,"kept_unknown":0,"released":0},"changed":0,"actions":[],"complete":true})
}
#[test]
fn retained_state_checkpoint_requires_intent() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let basis = f.state()["basis"].clone();
    f.json(&[
        "note",
        &f.id,
        &format!("retention checkpoint: {}", cp(&f, basis)),
    ]);
    let s = f.state();
    assert_eq!(s["complete"], false);
    assert_eq!(s["items"][0]["record"], serde_json::Value::Null);
}
#[test]
fn retained_state_checkpoint_intervening_change() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let basis = f.state()["basis"].clone();
    f.json(&[
        "note",
        &f.id,
        &format!(
            "retention operation: {}",
            op(&f, basis, "intent", json!({}))
        ),
    ]);
    let basis = f.state()["basis"].clone();
    let mut r = f.record();
    r["purpose"] = json!("Changed after observation");
    f.note(r);
    f.json(&[
        "note",
        &f.id,
        &format!("retention checkpoint: {}", cp(&f, basis)),
    ]);
    let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}
#[test]
fn retained_state_repair_requires_exact_bad_source() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let mut bad = f.record();
    bad["sha256"] = json!("damaged");
    f.note(bad);
    let basis = f.state()["basis"].clone();
    let ordinal = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    f.json(&[
        "note",
        &f.id,
        &format!(
            "retention operation: {}",
            op(&f, basis.clone(), "intent", json!({}))
        ),
    ]);
    f.note(f.record());
    let successor = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    let mut fake = basis.clone();
    fake["commit"] = json!("0".repeat(40));
    let receipt = op(
        &f,
        basis,
        "items_published",
        json!({"items":[successor],"artifacts":[],"resolutions":[{"bad":{"basis":fake,"ordinal":ordinal},"successors":[successor],"reason":"Claimed repair"}]}),
    );
    f.json(&["note", &f.id, &format!("retention operation: {receipt}")]);
    let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn retained_state_completion() {
    let f = RetainedEnv::new("in-tree");
    assert_eq!(
        f.env
            .complete_values(&f.dir, "bash", 3, &["tasks", "retained", "state", "sci-"]),
        [f.id.as_str()]
    );
    assert_eq!(
        f.env.complete(
            &f.dir,
            "bash",
            4,
            &["tasks", "retained", "state", &f.id, "--ho"]
        ),
        ["--host"]
    );
}
