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
#[test]
fn retained_checkpoint_recovery() {
    for boundary in [
        "intent",
        "artifact:items_published",
        "save:items_published",
        "items_published",
        "save:completed",
        "commit:completed",
    ] {
        let f = RetainedEnv::new("in-tree");
        f.json(&["start", &f.id]);
        f.check_hook();
        let tree = f.dir.join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("report"), "payload").unwrap();
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .unwrap()
            .trim()
            .to_string();
        let request = json!({"version":1,"operation_id":"1".repeat(32),"host":host,"inputs":[{"task":f.id,"basis":f.state()["basis"]}],"declarations":[{"kind":"scratch","path":tree.to_str().unwrap(),"purpose":"Review","cleanup_trigger":"Review completes"}]});
        let path = f.request("request.json", &request);
        let args = [
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ];
        let out = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", boundary)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(86), "{boundary}: {out:?}");
        let result = f.json(&args);
        assert_eq!(result["outcome"], "verified", "{boundary}");
        let state = f.state();
        assert_eq!(
            state["checkpoints"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["note"]["complete"] == true)
                .count(),
            1
        );
        let name = std::path::Path::new(state["items"][0]["record"]["manifest"].as_str().unwrap())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args([
                "cat-file",
                "-e",
                &format!("HEAD:tasks/files/{}/{name}", f.id),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{boundary}: manifest was not committed: {out:?}"
        );
    }
}

#[test]
fn retained_checkpoint_saved_completion_rechecks() {
    let f = RetainedEnv::new("in-tree");
    f.json(&["start", &f.id]);
    f.check_hook();
    let file = f.dir.join("payload");
    std::fs::write(&file, "payload").unwrap();
    let request = json!({"version":1,"operation_id":"2".repeat(32),"host":std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap().trim(),"inputs":[{"task":f.id,"basis":f.state()["basis"]}],"declarations":[{"kind":"evidence","path":file.to_str().unwrap(),"purpose":"Review","cleanup_trigger":"Review completes"}]});
    let path = f.request("request.json", &request);
    let args = [
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ];
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "save:completed")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(86), "{out:?}");
    std::fs::remove_file(file).unwrap();
    let result = f.json(&args);
    assert_eq!(result["outcome"], "accounted_unknown");
    let state = f.state();
    assert_eq!(state["items"][0]["record"]["verification"], "unknown");
    assert_eq!(
        state["checkpoints"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["note"]["complete"] == true)
            .count(),
        1
    );
    let entries = std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("retained-checkpoint-")
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let backup: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&entries[0]).unwrap()).unwrap();
    assert!(
        backup["raw_task"]
            .as_str()
            .unwrap()
            .contains("retention checkpoint:")
    );
}
