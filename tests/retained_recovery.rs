mod common;
use common::retained::RetainedEnv;
use serde_json::json;
fn intent(f: &RetainedEnv) -> serde_json::Value {
    let b = f.state()["basis"].clone();
    json!({"version":1,"key":{"coordinator":f.id,"operation_id":"a".repeat(32)},"host":"fixture-host","request":{"type":"git","basis":b,"path":"README.md","sha256":"a".repeat(64)},"input_bases":[{"task":f.id,"basis":b}],"phase":"intent","at":"2026-11-03T00:00:00Z","observer":null,"diagnostics":[],"outputs":{}})
}
fn pending(f: &RetainedEnv) {
    f.seed_note(&format!("retention operation: {}", intent(f)));
}
#[test]
fn retained_pending_guards() {
    let f = RetainedEnv::new("in-tree");
    pending(&f);
    f.json(&["note", &f.id, "Ordinary progress"]);
    f.json(&["start", &f.id]);
    f.json(&["park", &f.id, "Resume this operation"]);
    for args in [
        vec!["done", &f.id, "Discard nothing"],
        vec!["edit", &f.id, "--title", "Changed"],
        vec!["note", &f.id, "retained: {}"],
    ] {
        let out = f.cmd().args(&args).output().unwrap();
        assert!(!out.status.success(), "{args:?}: {out:?}");
    }
    let out = f.cmd().args(["archive-notes", &f.id]).output().unwrap();
    assert!(!out.status.success());
}
#[test]
fn retained_protected_artifacts() {
    let f = RetainedEnv::new("in-tree");
    let p = f.dir.join("payload");
    std::fs::write(&p, "immutable").unwrap();
    let name = "retained-op-a.request.json";
    f.json(&["attach", &f.id, p.to_str().unwrap(), "--name", name]);
    let out = f
        .cmd()
        .args(["detach", &f.id, name, "disposal"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        f.records
            .join(format!("tasks/files/{}/{name}", f.id))
            .exists()
    );
}
#[test]
fn retained_protected_move() {
    let mut f = RetainedEnv::new("in-tree");
    let p = f.dir.join("payload");
    std::fs::write(&p, "immutable").unwrap();
    f.json(&[
        "attach",
        &f.id,
        p.to_str().unwrap(),
        "--name",
        "retained-manifest-a.json",
    ]);
    f.env.init("fam");
    let moved = f
        .cmd()
        .args(["move", &f.id, "--to", "fam"])
        .output()
        .unwrap();
    assert!(!moved.status.success(), "{moved:?}");
    assert!(f.records.join(format!("tasks/{}.md", f.id)).exists());
    let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();
    assert!(!out.status.success(), "{out:?}");
}
#[test]
fn retained_reserved_forgery() {
    let f = RetainedEnv::new("in-tree");
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "note",
            &f.id,
            &format!("retention operation: {}", intent(&f)),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(f.basis(), before);
}

#[test]
fn retained_terminal_archive() {
    let f = RetainedEnv::new("in-tree");
    let mut n = intent(&f);
    f.note(f.record());
    f.seed_note(&format!("retention operation: {n}"));
    n["phase"] = json!("completed");
    n["outputs"] = json!({"outcome":"verified","publications":[],"checkpoint":null});
    f.seed_note(&format!("retention operation: {n}"));
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
    assert_eq!(
        before["items"][0]["source"]["ordinal"],
        after["items"][0]["source"]["ordinal"]
    );
    assert_eq!(
        before["operations"][0]["note"],
        after["operations"][0]["note"]
    );
}
