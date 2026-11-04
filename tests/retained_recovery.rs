mod common;
use common::retained::{RetainedEnv, host, transfer, transfer_fixture, transfer_request};
use common::retained::{
    checkpoint, checkpoint_fixture, checkpoint_request, cross_receiver, declaration,
};
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
    let f = checkpoint_fixture();
    let payload = f.dir.join("payload");
    std::fs::write(&payload, "immutable").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&payload, "evidence")])),
    );
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

fn reconcile_request(f: &RetainedEnv, to: &str) -> serde_json::Value {
    json!({"version":1,"operation":{"coordinator":f.id,"operation_id":"a".repeat(32)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]},{"task":to,"basis":f.json(&["retained","state",to])["basis"]}],"reason":"Payload is irrecoverable; preserve both custodies and end the reservation"})
}
fn reconcile(f: &RetainedEnv, request: &serde_json::Value) -> serde_json::Value {
    let path = f.request("reconcile.json", request);
    f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ])
}
fn receiving_only() -> (RetainedEnv, String, std::path::PathBuf) {
    let (f, to, file) = transfer_fixture();
    f.check_hook();
    let request = transfer_request(&f, &to);
    let path = f.request("transfer.json", &request);
    assert!(
        !f.cmd()
            .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
            .args([
                "retained",
                "transfer",
                &f.id,
                &to,
                "--request",
                path.to_str().unwrap()
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    (f, to, file)
}
#[test]
fn retained_reconcile_irrecoverable() {
    let (f, to, file) = receiving_only();
    std::fs::remove_file(file).unwrap();
    let req = reconcile_request(&f, &to);
    let result = reconcile(&f, &req);
    assert_eq!(result["outcome"], "reconciled");
    assert!(!result["actions"].as_array().unwrap().is_empty());
    for id in [&f.id, &to] {
        let state = f.json(&["retained", "state", id]);
        assert_eq!(state["items"][0]["record"]["state"], "kept");
        assert_eq!(state["items"][0]["record"]["verification"], "unknown");
        assert!(state["items"][0]["record"]["sha256"].is_null());
        f.json(&["edit", id, "--title", "Recovered reservation"]);
    }
    let replay = reconcile(&f, &req);
    assert_eq!(replay["publications"], result["publications"]);
    let next = json!({"version":1,"operation_id":"d".repeat(32),"host":host(),"inputs":[{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}]});
    let path = f.request("repair.json", &next);
    assert_eq!(
        f.json(&[
            "retained",
            "checkpoint",
            &to,
            "--request",
            path.to_str().unwrap()
        ])["outcome"],
        "accounted_unknown"
    );
}
#[test]
fn retained_reconcile_after_release() {
    let (f, to, _) = transfer_fixture();
    let original = transfer_request(&f, &to);
    transfer(&f, &to, &original);
    let req = reconcile_request(&f, &to);
    let path = f.request("reconcile.json", &req);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "reconcile",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(f.basis(), before);
}
#[test]
fn retained_reconcile_retry() {
    for boundary in [
        "artifact:reconcile_intent",
        "save:reconcile_intent",
        "commit:reconcile_intent",
        "reconcile_intent",
        "save:reconcile_ready",
        "commit:reconcile_ready",
        "reconcile_ready",
        "save:reconciled",
        "commit:reconciled",
        "reconciled",
    ] {
        let (f, to, file) = receiving_only();
        std::fs::remove_file(file).unwrap();
        let req = reconcile_request(&f, &to);
        let path = f.request("reconcile.json", &req);
        let out = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", boundary)
            .args([
                "retained",
                "reconcile",
                &f.id,
                "--request",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(86),
            "{boundary}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if !boundary.ends_with("reconciled") {
            for id in [&f.id, &to] {
                assert!(
                    !f.cmd()
                        .args(["edit", id, "--title", "Premature release"])
                        .output()
                        .unwrap()
                        .status
                        .success(),
                    "{boundary} {id}"
                );
            }
        }
        let result = reconcile(&f, &req);
        assert_eq!(result["outcome"], "reconciled", "{boundary}");
        for id in [&f.id, &to] {
            f.json(&["edit", id, "--title", "Reservation ended"]);
        }
    }
}
#[test]
fn retained_reconcile_directive_conflict() {
    let (f, to, _) = receiving_only();
    let req = reconcile_request(&f, &to);
    let path = f.request("reconcile.json", &req);
    assert!(
        !f.cmd()
            .env("TASKS_RETAINED_STOP_AFTER", "reconcile_intent")
            .args([
                "retained",
                "reconcile",
                &f.id,
                "--request",
                path.to_str().unwrap()
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    for new_id in [false, true] {
        let mut other = req.clone();
        if new_id {
            other["reconciliation_id"] = json!("d".repeat(32));
        } else {
            other["reason"] = json!("Different intent");
        }
        let path = f.request("other.json", &other);
        let before = f.basis();
        assert!(
            !f.cmd()
                .args([
                    "retained",
                    "reconcile",
                    &f.id,
                    "--request",
                    path.to_str().unwrap()
                ])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(f.basis(), before);
    }
    assert_eq!(reconcile(&f, &req)["outcome"], "reconciled");
}
#[test]
fn retained_reconcile_malformed() {
    let (f, to, _) = receiving_only();
    f.seed_note(&format!(
        "retained: {{\"host\":\"{}\",\"version\":2}}",
        host()
    ));
    let req = reconcile_request(&f, &to);
    let result = reconcile(&f, &req);
    assert_eq!(result["outcome"], "reconciled");
    assert!(!result["actions"].as_array().unwrap().is_empty());
    let state = f.state();
    assert_eq!(state["complete"], false);
    assert!(state["items"][0]["record"].is_null());
    f.json(&[
        "edit",
        &f.id,
        "--title",
        "Reservation ended; history still blocked",
    ]);
}

#[test]
fn retained_reconcile_cross_project_and_named_branch() {
    for feature in [false, true] {
        let (f, _, file) = transfer_fixture();
        let (_, to) = cross_receiver(&f, false);
        let source = if feature {
            std::fs::write(f.dir.join(".gitignore"), ".worktrees/\n").unwrap();
            common::git(&f.dir, &["add", ".gitignore"]);
            common::git(&f.dir, &["commit", "-qm", "ignore fixture worktrees"]);
            let tree = f.dir.join(".worktrees/holder");
            common::git(
                &f.dir,
                &["worktree", "add", "-qb", "holder", tree.to_str().unwrap()],
            );
            common::git_fixture::assert_worktree(f.env.home.path(), &f.dir, &tree);
            f.json(&["-C", tree.to_str().unwrap(), "start", &f.id]);
            tree
        } else {
            f.dir.clone()
        };
        let mut original = transfer_request(&f, &to);
        original["inputs"][0]["basis"] =
            f.json(&["-C", source.to_str().unwrap(), "retained", "state", &f.id])["basis"].clone();
        let path = f.request("transfer.json", &original);
        let stopped = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
            .args([
                "-C",
                source.to_str().unwrap(),
                "retained",
                "transfer",
                &f.id,
                &to,
                "--request",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");
        std::fs::remove_file(file).unwrap();
        let mut req = reconcile_request(&f, &to);
        req["inputs"][0]["basis"] =
            f.json(&["-C", source.to_str().unwrap(), "retained", "state", &f.id])["basis"].clone();
        let path = f.request("reconcile.json", &req);
        assert_eq!(
            f.json(&[
                "-C",
                source.to_str().unwrap(),
                "retained",
                "reconcile",
                &f.id,
                "--request",
                path.to_str().unwrap()
            ])["outcome"],
            "reconciled"
        );
        f.json(&[
            "edit",
            &to,
            "--title",
            "Ready follows exact coordinator retained branch",
        ]);
        let next = json!({"version":1,"operation_id":"e".repeat(32),"host":host(),"inputs":[{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}]});
        let path = f.request("next.json", &next);
        assert_eq!(
            f.json(&[
                "retained",
                "checkpoint",
                &to,
                "--request",
                path.to_str().unwrap()
            ])["outcome"],
            "accounted_unknown"
        );
    }
}

#[test]
fn retained_reconcile_release_in_earlier_phase_still_refuses() {
    let (f, to, _) = transfer_fixture();
    let original = transfer_request(&f, &to);
    let path = f.request("transfer.json", &original);
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "sender_released")
        .args([
            "retained",
            "transfer",
            &f.id,
            &to,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(86));
    let state = f.state();
    let mut operation = state["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["note"]["phase"] == "sender_released")
        .unwrap()["note"]
        .clone();
    operation["phase"] = json!("incomplete");
    operation["outputs"] = json!({"attempt":1,"previous_phase":"sender_released","checkpoint":null,"action":"Finish original final receipts"});
    f.seed_note(&format!("retention operation: {operation}"));
    let req = reconcile_request(&f, &to);
    let path = f.request("reconcile.json", &req);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "reconcile",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(f.basis(), before);
    assert!(String::from_utf8_lossy(&out.stderr).contains("Sender release is committed"));
    assert_eq!(transfer(&f, &to, &original)["outcome"], "transferred");
}
#[test]
fn retained_reconcile_checkpoint_manifest_recovery() {
    use common::retained::{checkpoint_fixture, checkpoint_request, declaration};
    for boundary in ["artifact:reconcile_ready", "save:reconcile_ready"] {
        let f = checkpoint_fixture();
        f.check_hook();
        let tree = f.dir.join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("report"), "kept").unwrap();
        let original = checkpoint_request(&f, 10, json!([declaration(&tree, "scratch")]));
        let path = f.request("checkpoint.json", &original);
        assert_eq!(
            f.cmd()
                .env("TASKS_RETAINED_STOP_AFTER", "intent")
                .args([
                    "retained",
                    "checkpoint",
                    &f.id,
                    "--request",
                    path.to_str().unwrap()
                ])
                .output()
                .unwrap()
                .status
                .code(),
            Some(86)
        );
        let mut req = reconcile_request(&f, &f.id);
        req["operation"]["operation_id"] = original["operation_id"].clone();
        req["inputs"] = json!([{"task":f.id,"basis":f.state()["basis"]}]);
        let path = f.request("reconcile.json", &req);
        let out = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", boundary)
            .args([
                "retained",
                "reconcile",
                &f.id,
                "--request",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(86), "{boundary}: {out:?}");
        assert_eq!(reconcile(&f, &req)["outcome"], "reconciled");
        let state = f.state();
        assert_eq!(state["items"][0]["record"]["state"], "kept");
        assert_eq!(state["items"][0]["record"]["verification"], "verified");
        let path = std::path::Path::new(state["items"][0]["record"]["manifest"].as_str().unwrap());
        let name = path.file_name().unwrap().to_str().unwrap();
        let blob = common::git_fixture::command(f.env.home.path(), &f.records)
            .args(["show", &format!("HEAD:tasks/files/{}/{name}", f.id)])
            .output()
            .unwrap();
        assert!(blob.status.success());
        assert_eq!(blob.stdout, std::fs::read(path).unwrap());
        f.json(&[
            "edit",
            &f.id,
            "--title",
            "Fresh checkpoint can follow reconciliation",
        ]);
        let old = f.request("old-checkpoint.json", &original);
        assert!(
            !f.cmd()
                .args([
                    "retained",
                    "checkpoint",
                    &f.id,
                    "--request",
                    old.to_str().unwrap()
                ])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

#[test]
fn retained_reconcile_rejects_unbound_receiver_mapping() {
    let (f, to, file) = receiving_only();
    std::fs::remove_file(file).unwrap();
    let path = f.records.join(format!("tasks/{to}.md"));
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut changed = false;
    let raw = raw
        .lines()
        .map(|line| {
            if let Some((head, body)) = line.split_once("retention operation: ") {
                let mut note: serde_json::Value = serde_json::from_str(body).unwrap();
                if note["phase"] == "receiver_published" {
                    note["outputs"]["mapping"][0]["receiver"]["path"] = json!("/unbound/identity");
                    changed = true;
                    return format!("{head}retention operation: {note}");
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(changed);
    std::fs::write(&path, raw).unwrap();
    common::git(&f.records, &["add", "tasks"]);
    common::git(
        &f.records,
        &["commit", "-qm", "inconsistent receiver history"],
    );
    let req = reconcile_request(&f, &to);
    let path = f.request("reconcile.json", &req);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "reconcile",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "an inconsistent receiver mapping is not a recoverable payload failure"
    );
    assert_eq!(f.basis(), before);
}

#[test]
fn retained_reconcile_corrupt_terminal_stays_blocked() {
    let (f, to, file) = receiving_only();
    std::fs::remove_file(file).unwrap();
    let req = reconcile_request(&f, &to);
    reconcile(&f, &req);
    let path = f.records.join(format!("tasks/{}.md", f.id));
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut changed = false;
    let raw = raw
        .lines()
        .map(|line| {
            if let Some((head, body)) = line.split_once("retention operation: ") {
                let mut n: serde_json::Value = serde_json::from_str(body).unwrap();
                if n["phase"] == "reconciled" {
                    n["outputs"]["ready_bases"] = json!([]);
                    changed = true;
                    return format!("{head}retention operation: {n}");
                }
            }
            line.into()
        })
        .collect::<Vec<String>>()
        .join("\n")
        + "\n";
    assert!(changed);
    std::fs::write(path, raw).unwrap();
    common::git(&f.records, &["add", "tasks"]);
    common::git(&f.records, &["commit", "-qm", "invalid terminal proof"]);
    for id in [&f.id, &to] {
        assert!(
            !f.cmd()
                .args(["edit", id, "--title", "Must stay blocked"])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert!(
        !f.cmd()
            .args(["retained", "state", &f.id])
            .output()
            .unwrap()
            .status
            .success()
    );
}
#[test]
fn retained_reconcile_consumer_fixture() {
    use sha2::{Digest, Sha256};
    let (f, to, file) = receiving_only();
    std::fs::remove_file(file).unwrap();
    let request = reconcile_request(&f, &to);
    let result = reconcile(&f, &request);
    let fixture = json!({"format":"tasks-retained-generated-fixture-1","producer_reconcile_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src/retained/reconcile.rs"))),"request":request,"result":result,"sender":f.json(&["show",&f.id])["task"],"receiver":f.json(&["show",&to])["task"],"sender_state":f.state(),"receiver_state":f.json(&["retained","state",&to]),"expected":{"outcome":"reconciled","sender_custody":"kept","receiver_custody":"kept","observations":"unknown","reservation":"ended","complete_host_checkpoint":false,"bytes_inspected_by_consumer":false}});
    assert_eq!(fixture["result"]["outcome"], "reconciled");
    if let Some(dir) = std::env::var_os("TASKS_RETAINED_RECONCILE_FIXTURE_OUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir(&dir).expect("fixture export needs a new explicit directory");
        std::fs::write(
            dir.join("reconcile-generated.json"),
            serde_json::to_vec_pretty(&fixture).unwrap(),
        )
        .unwrap();
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args([
                "bundle",
                "create",
                dir.join("reconcile-records.bundle").to_str().unwrap(),
                "refs/heads/main",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
}
