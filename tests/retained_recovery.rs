mod common;
use common::id_of;
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
    let name = format!("retained-op-{}.request.json", "a".repeat(32));
    let p = f.dir.join(&name);
    let b = f.state()["basis"].clone();
    let req = json!({
        "version": 1,
        "operation_id": "a".repeat(32),
        "host": "fixture-host",
        "inputs": [{"task": f.id, "basis": b}],
        "declarations": []
    });
    let req_bytes = serde_json::to_vec(&req).unwrap();
    std::fs::write(&p, &req_bytes).unwrap();
    f.json(&["attach", &f.id, p.to_str().unwrap(), "--name", &name]);
    use sha2::{Digest, Sha256};
    let sha = format!("{:x}", Sha256::digest(&req_bytes));
    let note = json!({"version":1,"key":{"coordinator":f.id,"operation_id":"a".repeat(32)},"host":"fixture-host","request":{"type":"attachment","task":f.id,"name":name,"sha256":sha},"input_bases":[{"task":f.id,"basis":b}],"phase":"intent","at":"2026-11-03T00:00:00Z","observer":null,"diagnostics":[],"outputs":{}});
    f.seed_note(&format!("retention operation: {note}"));
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
fn pending_operation_defers_citation_links() {
    let f = RetainedEnv::new("in-tree");
    pending(&f);
    let initial_state = f.state();

    let y = id_of(f.json(&["add", "Task Y"]));

    // note <id> "see Y" succeeds
    let note_out = f.json(&["note", &f.id, &format!("see {y}")]);
    // start <id> succeeds
    let start_out = f.json(&["start", &f.id]);
    // park <id> "next: Y" succeeds
    let park_out = f.json(&["park", &f.id, &format!("next: {y}")]);

    // related stays absent
    let shown = f.json(&["show", &f.id]);
    assert!(shown["task"].get("related").is_none());

    // the warnings of note and park each carry the deferral text naming Y
    let expected_warning = format!(
        "{} has a pending retained operation; not linked: {y}; link them with tasks edit {} --related {y} once it completes",
        f.id, f.id
    );
    let note_warnings = note_out["warnings"].as_array().unwrap();
    assert!(
        note_warnings
            .iter()
            .any(|w| w.as_str() == Some(&expected_warning)),
        "note warnings missing deferral text: {note_warnings:?}"
    );

    let park_warnings = park_out["warnings"].as_array().unwrap();
    assert!(
        park_warnings
            .iter()
            .any(|w| w.as_str() == Some(&expected_warning)),
        "park warnings missing deferral text: {park_warnings:?}"
    );

    // start supplies no text, so its warnings carry no deferral and it scans no old prose
    let start_warnings = start_out["warnings"].as_array().unwrap();
    assert!(
        !start_warnings.iter().any(|w| w
            .as_str()
            .unwrap_or("")
            .contains("pending retained operation")),
        "start warnings should not carry deferral: {start_warnings:?}"
    );

    // f.state() after the writes reports the same operation phases and diagnostics as before
    let current_state = f.state();
    let initial_op = &initial_state["operations"][0]["note"];
    let current_op = &current_state["operations"][0]["note"];
    assert_eq!(current_op["phase"], initial_op["phase"]);
    assert_eq!(current_op["diagnostics"], initial_op["diagnostics"]);

    // done still refuses, as in retained_pending_guards
    let out = f
        .cmd()
        .args(["done", &f.id, "Discard nothing"])
        .output()
        .unwrap();
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
    let fam = f.env.init("fam");
    let moved = f
        .cmd()
        .args(["move", &f.id, "--to", "fam"])
        .output()
        .unwrap();
    assert!(!moved.status.success(), "{moved:?}");
    let moved_err: serde_json::Value = serde_json::from_slice(&moved.stderr).unwrap();
    assert_eq!(moved_err["error"]["kind"], "retained_history_blocked");
    assert!(f.records.join(format!("tasks/{}.md", f.id)).exists());
    let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();
    assert!(!out.status.success(), "{out:?}");

    // Standalone `retention resolution: ` note (with zero `retention operation:` notes and zero
    // protected attachments) must refuse both `tasks move` and `tasks migrate` with
    // `retained_history_blocked` before mutating either project.
    let mut f_res = RetainedEnv::new("in-tree");
    let fam_res = f_res.env.init("fam");
    f_res.seed_note("retained: accidental prose");
    let state_bad = f_res.state();
    assert_eq!(state_bad["complete"], false);
    let resolution = json!({
        "version": 1,
        "bad": state_bad["diagnostics"][0]["source"],
        "successors": [],
        "reason": "Accidental prose on zero-item task"
    });
    f_res.json(&[
        "note",
        &f_res.id,
        &format!("retention resolution: {resolution}"),
    ]);
    let state_repaired = f_res.state();
    assert_eq!(state_repaired["complete"], true);
    assert!(state_repaired["operations"].as_array().unwrap().is_empty());
    assert!(
        !f_res
            .records
            .join(format!("tasks/files/{}", f_res.id))
            .exists()
    );

    let before_sci = f_res.basis();
    let move_res = f_res
        .cmd()
        .args(["move", &f_res.id, "--to", "fam"])
        .output()
        .unwrap();
    assert_eq!(move_res.status.code(), Some(1), "{move_res:?}");
    let move_res_err: serde_json::Value = serde_json::from_slice(&move_res.stderr).unwrap();
    assert_eq!(
        move_res_err["error"]["kind"], "retained_history_blocked",
        "{move_res_err}"
    );
    assert_eq!(f_res.basis(), before_sci);
    assert!(
        f_res
            .records
            .join(format!("tasks/{}.md", f_res.id))
            .exists()
    );
    assert!(
        f_res.env.json(&fam_res, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let list_path = f_res.env.home.path().join("migrate-list.toml");
    f_res.json(&[
        "migrate",
        "--to",
        "fam",
        "--draft",
        list_path.to_str().unwrap(),
    ]);
    let list_text = std::fs::read_to_string(&list_path)
        .unwrap()
        .replace("decision = \"\"", "decision = \"carry\"");
    std::fs::write(&list_path, list_text).unwrap();
    for migrate_args in [
        vec!["migrate", list_path.to_str().unwrap(), "--explain"],
        vec!["migrate", list_path.to_str().unwrap()],
    ] {
        let mig = f_res.cmd().args(&migrate_args).output().unwrap();
        assert_eq!(mig.status.code(), Some(1), "{migrate_args:?}: {mig:?}");
        let mig_err: serde_json::Value = serde_json::from_slice(&mig.stderr).unwrap();
        assert_eq!(
            mig_err["error"]["kind"], "retained_history_blocked",
            "{migrate_args:?}: {mig_err}"
        );
        assert_eq!(f_res.basis(), before_sci);
        assert!(
            f_res.env.json(&fam_res, &["list"])["tasks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    // JSON `retention checkpoint:` note and protected `multipart:` declaration also block `move`
    // before mutating either project, while non-JSON prose `retention checkpoint:` remains movable.
    let cp_id = id_of(f.json(&["add", "JSON checkpoint envelope task"]));
    let cp_path = f.records.join(format!("tasks/{cp_id}.md"));
    let mut cp_raw = std::fs::read_to_string(&cp_path).unwrap();
    if !cp_raw.contains("\n## Notes\n") {
        cp_raw.push_str("\n## Notes\n\n");
    }
    cp_raw.push_str("- 2026-11-03T00:00:00Z (fixture): retention checkpoint: {\"version\":1}\n");
    std::fs::write(&cp_path, cp_raw).unwrap();
    common::git(&f.records, &["add", cp_path.to_str().unwrap()]);
    common::git(&f.records, &["commit", "-qm", "seed json checkpoint note"]);

    let mp_id = id_of(f.json(&["add", "Protected multipart declaration task"]));
    let mp_path = f.records.join(format!("tasks/{mp_id}.md"));
    let mut mp_raw = std::fs::read_to_string(&mp_path).unwrap();
    if !mp_raw.contains("\n## Notes\n") {
        mp_raw.push_str("\n## Notes\n\n");
    }
    mp_raw.push_str(&format!(
        "- 2026-11-03T00:00:00Z (fixture): multipart: retained-manifest-{}.json (123 bytes, 2 parts, sha256 {}, catalog sha256 {})\n",
        "a".repeat(64),
        "a".repeat(64),
        "b".repeat(64)
    ));
    std::fs::write(&mp_path, mp_raw).unwrap();
    common::git(&f.records, &["add", mp_path.to_str().unwrap()]);
    common::git(
        &f.records,
        &["commit", "-qm", "seed protected multipart declaration"],
    );

    let before_f = f.basis();
    for blocked_id in [&cp_id, &mp_id] {
        let out = f
            .cmd()
            .args(["move", blocked_id, "--to", "fam"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{blocked_id}: {out:?}");
        let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(
            err["error"]["kind"], "retained_history_blocked",
            "{blocked_id}: {err}"
        );
        assert_eq!(f.basis(), before_f);
        assert!(
            f.env.json(&fam, &["list"])["tasks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    let prose_cp_id = id_of(f.json(&["add", "Prose checkpoint note task"]));
    f.json(&["note", &prose_cp_id, "retention checkpoint: manual prose"]);
    let moved_prose = f.json(&["move", &prose_cp_id, "--to", "fam"]);
    assert!(moved_prose["id"].as_str().unwrap().starts_with("fam-"));
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
        let f = RetainedEnv::new("detached");
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
                &format!("HEAD:sci/tasks/files/{}/{name}", f.id),
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
    let f = RetainedEnv::new("detached");
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
fn retained_reconcile_omitted_participant_is_typed_refusal() {
    let (f, to, file) = receiving_only();
    let before = [f.state(), f.json(&["retained", "state", &to])];
    let mut req = reconcile_request(&f, &to);
    req["inputs"].as_array_mut().unwrap().pop();
    let path = f.request("reconcile.json", &req);
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
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "retained_input");
    assert_eq!(f.state(), before[0]);
    assert_eq!(f.json(&["retained", "state", &to]), before[1]);
    assert_eq!(std::fs::read_to_string(file).unwrap(), "payload");
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
            .args(["show", &format!("HEAD:sci/tasks/files/{}/{name}", f.id)])
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

#[test]
fn retained_phase_freshness_checkpoint_restart() {
    for case in [
        "unrelated",
        "participant",
        "ABA",
        "note",
        "park_start",
        "archived_note",
        "metadata_ABA",
        "note_then_ABA",
        "attachment_ABA",
        "reserved",
    ] {
        let f = checkpoint_fixture();
        let payload = f.dir.join("attachment");
        std::fs::write(&payload, "original").unwrap();
        f.json(&[
            "attach",
            &f.id,
            payload.to_str().unwrap(),
            "--name",
            "evidence",
        ]);
        if case == "archived_note" {
            f.json(&["note", &f.id, &"Prelude one ".repeat(80)]);
            f.json(&["note", &f.id, &"Prelude two ".repeat(80)]);
            let preview = f.json(&["archive-notes", &f.id]);
            assert_eq!(preview["eligible"], true, "{preview}");
            f.json(&[
                "archive-notes",
                &f.id,
                "--expect",
                preview["source_sha256"].as_str().unwrap(),
            ]);
        }
        let request = checkpoint_request(&f, 94, json!([]));
        let path = f.request("request.json", &request);
        let args = [
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ];
        let stopped = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", "intent")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");
        if matches!(
            case,
            "note" | "archived_note" | "metadata_ABA" | "note_then_ABA" | "attachment_ABA"
        ) {
            f.json(&["note", &f.id, "Ordinary progress"]);
        }
        if case == "park_start" {
            f.json(&["park", &f.id, "Resume the original checkpoint"]);
            f.json(&["start", &f.id]);
        }
        if case == "reserved" {
            f.note(f.record());
        }
        if matches!(case, "unrelated" | "participant" | "ABA" | "note_then_ABA") {
            let target = if case == "unrelated" {
                f.records.join("unrelated")
            } else {
                f.records.join(format!("tasks/{}.md", f.id))
            };
            let old = std::fs::read(&target).unwrap_or_default();
            let mut new = old.clone();
            new.push(b'\n');
            std::fs::write(&target, new).unwrap();
            common::git(&f.records, &["add", target.to_str().unwrap()]);
            common::git(&f.records, &["commit", "-qm", "between invocations"]);
            if case == "ABA" || case == "note_then_ABA" {
                std::fs::write(&target, old).unwrap();
                common::git(&f.records, &["add", target.to_str().unwrap()]);
                common::git(&f.records, &["commit", "-qm", "revert"]);
            }
        }
        if case == "metadata_ABA" {
            let path = f.records.join(format!("tasks/{}.md", f.id));
            let old = std::fs::read_to_string(&path).unwrap();
            let owner = old
                .lines()
                .find(|line| line.starts_with("owner: "))
                .unwrap();
            let edited = old.replacen(owner, "owner: foreign", 1);
            for raw in [&edited, &old] {
                std::fs::write(&path, raw).unwrap();
                common::git(&f.records, &["add", path.to_str().unwrap()]);
                common::git(&f.records, &["commit", "-qm", "unbound metadata edit"]);
            }
        }
        if case == "attachment_ABA" {
            use std::os::unix::fs::PermissionsExt;
            let file = f.records.join(format!("tasks/files/{}/evidence", f.id));
            for mode in [0o755, 0o644] {
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
                common::git(&f.records, &["add", file.to_str().unwrap()]);
                common::git(&f.records, &["commit", "-qm", "attachment mode"]);
            }
        }
        let retry = f.cmd().args(args).output().unwrap();
        assert_eq!(
            retry.status.success(),
            matches!(case, "unrelated" | "note" | "park_start" | "archived_note"),
            "{case}: {retry:?}"
        );
        assert_eq!(
            f.state()["operations"][0]["note"]["input_bases"],
            request["inputs"]
        );
    }
}

#[test]
fn retained_historical_in_tree_operations() {
    use common::retained::{HistoricalScenario as S, seed_historical_operation};
    for scenario in [
        S::CheckpointPending,
        S::CheckpointCompleted,
        S::TransferReceiverPublished,
        S::TransferSenderReleased,
        S::Reconciled,
    ] {
        let f = RetainedEnv::new("in-tree");
        let seeded = seed_historical_operation(&f, scenario);
        if let Some(directive) = seeded.reconciliation {
            let path = f.request("reconcile.json", &directive);
            let out = f.json(&[
                "retained",
                "reconcile",
                &f.id,
                "--request",
                path.to_str().unwrap(),
            ]);
            assert_eq!(out["outcome"], "reconciled");
        } else if let Some(to) = seeded.receiver {
            let out = transfer(&f, &to, &seeded.request);
            assert_eq!(out["outcome"], "transferred");
        } else {
            let out = checkpoint(&f, &seeded.request);
            assert_eq!(out["outcome"], "verified");
        }
        assert_eq!(f.state()["complete"], true);
    }
}

#[test]
fn retained_phase_freshness_transfer_restart() {
    for cross in [false, true] {
        for case in ["unrelated", "ABA", "note", "park_start"] {
            let (f, mut to, _) = transfer_fixture();
            let receiver_records = if cross {
                let (root, id) = cross_receiver(&f, false);
                to = id;
                root
            } else {
                f.records.clone()
            };
            let request = transfer_request(&f, &to);
            let path = f.request("transfer.json", &request);
            let args = [
                "retained",
                "transfer",
                &f.id,
                &to,
                "--request",
                path.to_str().unwrap(),
            ];
            let stopped = f
                .cmd()
                .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
                .args(args)
                .output()
                .unwrap();
            assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");
            if case == "note" {
                for id in [&f.id, &to] {
                    f.json(&["note", id, "Ordinary progress"]);
                }
            } else if case == "park_start" {
                for id in [&f.id, &to] {
                    f.json(&["park", id, "Resume original transfer"]);
                    f.json(&["start", id]);
                }
            } else {
                let path = if case == "ABA" {
                    receiver_records.join(format!("tasks/{to}.md"))
                } else {
                    receiver_records.join("unrelated")
                };
                let old = std::fs::read(&path).unwrap_or_default();
                let mut new = old.clone();
                new.push(b'\n');
                std::fs::write(&path, new).unwrap();
                common::git(&receiver_records, &["add", path.to_str().unwrap()]);
                common::git(
                    &receiver_records,
                    &["commit", "-qm", "between transfer invocations"],
                );
                if case == "ABA" {
                    std::fs::write(&path, old).unwrap();
                    common::git(&receiver_records, &["add", path.to_str().unwrap()]);
                    common::git(&receiver_records, &["commit", "-qm", "revert participant"]);
                }
            }
            let result = f.cmd().args(args).output().unwrap();
            assert_eq!(
                result.status.success(),
                case != "ABA",
                "cross={cross}, {case}: {result:?}"
            );
            if case == "ABA" {
                assert_eq!(f.state()["items"][0]["record"]["state"], "kept");
            }
        }
    }
}

fn configure_attachments_recovery(records: &std::path::Path, attachments_toml: &str) {
    let config_path = records.join("tasks/.config.toml");
    let raw = std::fs::read_to_string(&config_path).unwrap();
    let mut doc: toml::Value = toml::from_str(&raw).unwrap();
    let att: toml::Value = toml::from_str(attachments_toml).unwrap();
    doc.as_table_mut()
        .unwrap()
        .insert("attachments".into(), att);
    std::fs::write(&config_path, toml::to_string(&doc).unwrap()).unwrap();
    common::git(records, &["add", config_path.to_str().unwrap()]);
    common::git(records, &["commit", "-qm", "configure attachments"]);
}

fn build_approx_5k_container_recovery(root: &std::path::Path) -> (Vec<u8>, String) {
    use sha2::{Digest, Sha256};
    std::fs::create_dir_all(root).unwrap();
    let mut entries = Vec::new();
    for i in 0..42 {
        let rel = format!("member-{i:03}-payload.txt");
        let content = format!("payload-{i:03}");
        std::fs::write(root.join(&rel), content.as_bytes()).unwrap();
        let file_sha = format!("{:x}", Sha256::digest(content.as_bytes()));
        entries.push(format!(
            "{{\"type\":\"file\",\"path\":\"{rel}\",\"sha256\":\"{file_sha}\"}}"
        ));
    }
    let manifest_bytes =
        format!("{{\"version\":1,\"entries\":[{}]}}", entries.join(",")).into_bytes();
    assert!(
        (4800..6000).contains(&manifest_bytes.len()),
        "unexpected manifest size: {}",
        manifest_bytes.len()
    );
    let manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));
    (manifest_bytes, manifest_sha)
}

fn build_small_container_recovery(
    root: &std::path::Path,
    filename: &str,
    content: &str,
) -> (Vec<u8>, String) {
    use sha2::{Digest, Sha256};
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join(filename), content.as_bytes()).unwrap();
    let file_sha = format!("{:x}", Sha256::digest(content.as_bytes()));
    let manifest_bytes = format!(
        "{{\"version\":1,\"entries\":[{{\"type\":\"file\",\"path\":\"{filename}\",\"sha256\":\"{file_sha}\"}}]}}"
    )
    .into_bytes();
    assert!(manifest_bytes.len() < 512);
    let manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));
    (manifest_bytes, manifest_sha)
}

#[test]
fn publish_batch_pass2_fail_after_rolls_back_only_created_files_and_surfaces_cleanup_errors() {
    // 1. Clean committed-only reuse fixture: reused committed item A (`create_new == false`)
    //    + new multipart item B + new single-file item C (`create_new == true`).
    //    On `TASKS_RETAINED_FAIL_AFTER="artifact:items_published"`, rollback removes all newly
    //    created files, preserves item A's committed file, and lets `incomplete` commit cleanly.
    let f_clean = checkpoint_fixture();
    f_clean.check_hook();
    configure_attachments_recovery(
        &f_clean.records,
        "max_bytes = 2048\nmultipart_validators = []\n",
    );

    let clean_a = f_clean.dir.join("item-a");
    let (clean_a_bytes, clean_a_sha) = build_small_container_recovery(&clean_a, "a.txt", "item-a");
    let clean_a_name = format!("retained-manifest-{clean_a_sha}.json");
    assert_eq!(
        checkpoint(
            &f_clean,
            &checkpoint_request(&f_clean, 701, json!([declaration(&clean_a, "scratch")])),
        )["outcome"],
        "verified"
    );
    let clean_b = f_clean.dir.join("item-b");
    let (_, clean_b_sha) = build_approx_5k_container_recovery(&clean_b);
    let clean_b_name = format!("retained-manifest-{clean_b_sha}.json");
    let clean_c = f_clean.dir.join("item-c");
    let (_, clean_c_sha) = build_small_container_recovery(&clean_c, "c.txt", "item-c");
    let clean_c_name = format!("retained-manifest-{clean_c_sha}.json");

    let mut decl_a_updated = declaration(&clean_a, "scratch");
    decl_a_updated["purpose"] = json!("Updated review purpose so item A re-enters batch");
    let clean_req = checkpoint_request(
        &f_clean,
        702,
        json!([
            decl_a_updated,
            declaration(&clean_b, "scratch"),
            declaration(&clean_c, "scratch"),
        ]),
    );
    let clean_req_path = f_clean.request("clean-fail-after.json", &clean_req);
    let clean_args = [
        "retained",
        "checkpoint",
        &f_clean.id,
        "--request",
        clean_req_path.to_str().unwrap(),
    ];
    let out_clean = f_clean
        .cmd()
        .env("TASKS_RETAINED_FAIL_AFTER", "artifact:items_published")
        .args(clean_args)
        .output()
        .unwrap();
    assert_eq!(out_clean.status.code(), Some(1), "{out_clean:?}");
    let err_clean: serde_json::Value = serde_json::from_slice(&out_clean.stderr).unwrap();
    let detail_clean = err_clean["error"]["detail"].as_str().unwrap();
    assert!(
        detail_clean.contains("artifact:items_published"),
        "got: {detail_clean}"
    );
    assert!(
        !detail_clean.contains("failure receipt could not publish"),
        "clean committed-reuse rollback must allow incomplete to commit cleanly: {detail_clean}"
    );
    let clean_files_dir = f_clean.records.join(format!("tasks/files/{}", f_clean.id));
    assert_eq!(
        std::fs::read(clean_files_dir.join(&clean_a_name)).unwrap(),
        clean_a_bytes,
        "item A's committed file must be preserved byte-for-byte"
    );
    assert!(!clean_files_dir.join(&clean_c_name).exists());
    assert!(
        !clean_files_dir
            .join(format!("{clean_b_name}.part-0001"))
            .exists()
    );
    assert!(
        !clean_files_dir
            .join(format!("{clean_b_name}.catalog.json"))
            .exists()
    );
    let clean_state = f_clean.state();
    assert_eq!(
        clean_state["operations"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["note"]["phase"],
        "incomplete"
    );
    assert_eq!(f_clean.json(&["check"])["errors"], json!([]));

    // 2. Mixed fixture combining:
    //    (a) reused already-committed `Ledger::Attached` manifest on item A (`create_new == false`),
    //    (b) pre-staged byte-identical unledgered `.part-0001` from a prior
    //        `TASKS_RETAINED_STOP_AFTER="artifact:items_published"` run on item B (`create_new == false`),
    //    (c) newly created `.part-0002`, `.part-0003`, `.catalog.json`, and item C's single-file
    //        manifest (`create_new == true`).
    let f = checkpoint_fixture();
    f.check_hook();
    configure_attachments_recovery(&f.records, "max_bytes = 2048\nmultipart_validators = []\n");

    let item_a = f.dir.join("item-a");
    let (item_a_bytes, item_a_sha) = build_small_container_recovery(&item_a, "a.txt", "item-a");
    let item_a_name = format!("retained-manifest-{item_a_sha}.json");
    assert_eq!(
        checkpoint(
            &f,
            &checkpoint_request(&f, 710, json!([declaration(&item_a, "scratch")])),
        )["outcome"],
        "verified"
    );

    let item_b = f.dir.join("item-b");
    let (_, item_b_sha) = build_approx_5k_container_recovery(&item_b);
    let item_b_name = format!("retained-manifest-{item_b_sha}.json");

    let item_c = f.dir.join("item-c");
    let (_, item_c_sha) = build_small_container_recovery(&item_c, "c.txt", "item-c");
    let item_c_name = format!("retained-manifest-{item_c_sha}.json");

    let mut decl_a = declaration(&item_a, "scratch");
    decl_a["purpose"] = json!("Updated review purpose on item A");
    let req = checkpoint_request(
        &f,
        711,
        json!([
            decl_a,
            declaration(&item_b, "scratch"),
            declaration(&item_c, "scratch"),
        ]),
    );
    let req_path = f.request("mixed-fail-after.json", &req);
    let args = [
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        req_path.to_str().unwrap(),
    ];

    // Pre-stage `.part-0001` on item B via `TASKS_RETAINED_STOP_AFTER="artifact:items_published"`.
    let stopped = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "artifact:items_published")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");

    let files_dir = f.records.join(format!("tasks/files/{}", f.id));
    let part1_path = files_dir.join(format!("{item_b_name}.part-0001"));
    let part2_path = files_dir.join(format!("{item_b_name}.part-0002"));
    let part3_path = files_dir.join(format!("{item_b_name}.part-0003"));
    let cat_path = files_dir.join(format!("{item_b_name}.catalog.json"));
    let item_c_path = files_dir.join(&item_c_name);
    assert!(part1_path.is_file());
    assert!(part2_path.is_file());
    assert!(part3_path.is_file());
    assert!(cat_path.is_file());
    assert!(item_c_path.is_file());

    let part1_bytes = std::fs::read(&part1_path).unwrap();
    for p in [&part2_path, &part3_path, &cat_path, &item_c_path] {
        std::fs::remove_file(p).unwrap();
    }
    let task_md_path = f.records.join(format!("tasks/{}.md", f.id));
    let task_md_before = std::fs::read(&task_md_path).unwrap();

    // Run with `TASKS_RETAINED_FAIL_AFTER="artifact:items_published"`:
    // Pass 2 creates `.part-0002`, `.part-0003`, `.catalog.json`, and `item_c_name`, hits the
    // failure injection before save, unlinks all `create_new == true` files, and preserves
    // item A's committed file and item B's adopted `.part-0001` byte-for-byte.
    let failed = f
        .cmd()
        .env("TASKS_RETAINED_FAIL_AFTER", "artifact:items_published")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1), "{failed:?}");
    let err_failed: serde_json::Value = serde_json::from_slice(&failed.stderr).unwrap();
    assert!(
        err_failed["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("artifact:items_published"),
        "{err_failed}"
    );
    assert_eq!(
        std::fs::read(files_dir.join(&item_a_name)).unwrap(),
        item_a_bytes
    );
    assert_eq!(std::fs::read(&part1_path).unwrap(), part1_bytes);
    assert!(!part2_path.exists());
    assert!(!part3_path.exists());
    assert!(!cat_path.exists());
    assert!(!item_c_path.exists());
    assert_eq!(
        std::fs::read(&task_md_path).unwrap(),
        task_md_before,
        "zero partial ledger or phase notes may be written before save"
    );

    // Unset `TASKS_RETAINED_FAIL_AFTER` and rerun the original request: retry adopts `.part-0001`
    // and completes `completed` cleanly.
    let res = f.json(&args);
    assert_eq!(res["outcome"], "verified", "{res}");
    assert_eq!(res["counts"]["kept_verified"], 3, "{res}");
    let final_state = f.state();
    assert_eq!(final_state["complete"], true, "{final_state}");
    assert_eq!(
        final_state["operations"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["note"]["phase"],
        "completed"
    );
    assert!(part1_path.is_file());
    assert!(part2_path.is_file());
    assert!(part3_path.is_file());
    assert!(cat_path.is_file());
    assert!(item_c_path.is_file());
    assert_eq!(f.json(&["check"])["errors"], json!([]));
}

#[test]
fn multipart_checkpoint_transfer_noop_reuse_and_stop_after_save_recovery() {
    let f = checkpoint_fixture();
    f.check_hook();
    configure_attachments_recovery(&f.records, "max_bytes = 2048\nmultipart_validators = []\n");

    let container = f.dir.join("multipart-container");
    let (_, manifest_sha) = build_approx_5k_container_recovery(&container);
    let manifest_name = format!("retained-manifest-{manifest_sha}.json");

    let cp_req = checkpoint_request(&f, 720, json!([declaration(&container, "scratch")]));
    let cp_path = f.request("cp-multipart.json", &cp_req);
    let cp_args = [
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        cp_path.to_str().unwrap(),
    ];

    // 1. Interrupt at `TASKS_RETAINED_STOP_AFTER="save:items_published"` and recover via `recover_saved` + `validate_since`.
    let stopped_save = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "save:items_published")
        .args(cp_args)
        .output()
        .unwrap();
    assert_eq!(stopped_save.status.code(), Some(86), "{stopped_save:?}");

    let cp_res = f.json(&cp_args);
    assert_eq!(cp_res["outcome"], "verified", "{cp_res}");
    let files_dir = f.records.join(format!("tasks/files/{}", f.id));
    assert!(!files_dir.join(&manifest_name).exists());
    assert!(
        files_dir
            .join(format!("{manifest_name}.part-0001"))
            .is_file()
    );
    assert!(
        files_dir
            .join(format!("{manifest_name}.part-0002"))
            .is_file()
    );
    assert!(
        files_dir
            .join(format!("{manifest_name}.part-0003"))
            .is_file()
    );
    assert!(
        files_dir
            .join(format!("{manifest_name}.catalog.json"))
            .is_file()
    );

    let shown_1 = f.json(&["show", &f.id]);
    let multipart_notes_1 = shown_1["task"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["text"].as_str().unwrap().starts_with("multipart: "))
        .count();
    assert_eq!(multipart_notes_1, 1);
    assert_eq!(f.state()["complete"], true);
    assert_eq!(f.json(&["check"])["errors"], json!([]));

    // 2. Second no-op `tasks retained checkpoint` reuses the live multipart manifest without re-attaching.
    let noop_req = checkpoint_request(&f, 721, json!([]));
    let noop_res = checkpoint(&f, &noop_req);
    assert_eq!(noop_res["outcome"], "verified", "{noop_res}");
    let shown_2 = f.json(&["show", &f.id]);
    let multipart_notes_2 = shown_2["task"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["text"].as_str().unwrap().starts_with("multipart: "))
        .count();
    assert_eq!(
        multipart_notes_2, 1,
        "no-op checkpoint must reuse live multipart manifest without re-attaching"
    );
    assert_eq!(f.json(&["check"])["errors"], json!([]));

    // 3. Native `tasks retained transfer` interrupted at `save:receiver_published` and `receiver_published`.
    let to = f.json(&["add", "Multipart receiver"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &to]);
    let tx_req = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 722u64),
        "host": host(),
        "inputs": [
            {"task": f.id, "basis": f.state()["basis"]},
            {"task": to, "basis": f.json(&["retained", "state", &to])["basis"]}
        ],
        "items": [f.state()["items"][0]["identity"]],
        "purpose": "Continue multipart custody",
        "cleanup_trigger": "Disposition complete"
    });
    let tx_path = f.request("tx-multipart.json", &tx_req);
    let tx_args = [
        "retained",
        "transfer",
        &f.id,
        &to,
        "--request",
        tx_path.to_str().unwrap(),
    ];

    // 3a. Transfer `f -> to` interrupted at `save:receiver_published`: `recover_saved` recovers
    //     the staged multipart receiver notes/files and completes the transfer cleanly.
    let stopped_tx_save = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "save:receiver_published")
        .args(tx_args)
        .output()
        .unwrap();
    assert_eq!(
        stopped_tx_save.status.code(),
        Some(86),
        "{stopped_tx_save:?}"
    );

    let tx_res = f.json(&tx_args);
    assert_eq!(tx_res["outcome"], "transferred", "{tx_res}");
    assert_eq!(f.state()["items"][0]["record"]["state"], "released");
    let to_state = f.json(&["retained", "state", &to]);
    assert_eq!(to_state["complete"], true);
    assert_eq!(to_state["items"][0]["record"]["state"], "kept");
    assert_eq!(to_state["items"][0]["record"]["verification"], "verified");
    let to_files = f.records.join(format!("tasks/files/{to}"));
    assert!(!to_files.join(&manifest_name).exists());
    assert!(
        to_files
            .join(format!("{manifest_name}.part-0001"))
            .is_file()
    );
    assert!(
        to_files
            .join(format!("{manifest_name}.catalog.json"))
            .is_file()
    );

    // 3b. Transfer `to -> to2` interrupted at `receiver_published`: `validate_since` validates
    //     the committed multipart `attached:` + `multipart:` notes on `to2` and completes cleanly.
    let to2 = f.json(&["add", "Second multipart receiver"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &to2]);
    let tx2_req = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 723u64),
        "host": host(),
        "inputs": [
            {"task": to, "basis": f.json(&["retained", "state", &to])["basis"]},
            {"task": to2, "basis": f.json(&["retained", "state", &to2])["basis"]}
        ],
        "items": [to_state["items"][0]["identity"]],
        "purpose": "Continue multipart custody on second hop",
        "cleanup_trigger": "Final disposition"
    });
    let tx2_path = f.request("tx2-multipart.json", &tx2_req);
    let tx2_args = [
        "retained",
        "transfer",
        &to,
        &to2,
        "--request",
        tx2_path.to_str().unwrap(),
    ];
    let stopped_tx_recv = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
        .args(tx2_args)
        .output()
        .unwrap();
    assert_eq!(
        stopped_tx_recv.status.code(),
        Some(86),
        "{stopped_tx_recv:?}"
    );
    let tx2_res = f.json(&tx2_args);
    assert_eq!(tx2_res["outcome"], "transferred", "{tx2_res}");
    assert_eq!(
        f.json(&["retained", "state", &to])["items"][0]["record"]["state"],
        "released"
    );
    let to2_state = f.json(&["retained", "state", &to2]);
    assert_eq!(to2_state["complete"], true);
    assert_eq!(to2_state["items"][0]["record"]["state"], "kept");
    assert_eq!(to2_state["items"][0]["record"]["verification"], "verified");
    assert_eq!(f.json(&["check"])["errors"], json!([]));
}

#[test]
fn multipart_stop_after_artifact_adopts_unchanged_and_flags_changed_member() {
    let f = checkpoint_fixture();
    f.check_hook();
    configure_attachments_recovery(&f.records, "max_bytes = 2048\nmultipart_validators = []\n");

    let container = f.dir.join("mutated-container");
    let (_, sha_a) = build_approx_5k_container_recovery(&container);
    let manifest_a = format!("retained-manifest-{sha_a}.json");

    let req = checkpoint_request(&f, 730, json!([declaration(&container, "scratch")]));
    let req_path = f.request("cp-stop-artifact.json", &req);
    let args = [
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        req_path.to_str().unwrap(),
    ];

    let stopped = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "artifact:items_published")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");

    let files_dir = f.records.join(format!("tasks/files/{}", f.id));
    let part1_a = files_dir.join(format!("{manifest_a}.part-0001"));
    assert!(part1_a.is_file());
    for suffix in ["part-0002", "part-0003", "catalog.json"] {
        std::fs::remove_file(files_dir.join(format!("{manifest_a}.{suffix}"))).unwrap();
    }

    // Mutate a member inside the scratch directory so the new manifest digest `sha_b != sha_a`.
    let member0 = container.join("member-000-payload.txt");
    let original_member0 = std::fs::read(&member0).unwrap();
    std::fs::write(&member0, b"mutated-payload-000").unwrap();

    let out = f.cmd().args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "retained_stale_basis", "{err}");
    let expected_prefix = format!(
        "Protected attachment directory tasks/files/{} contains unledgered file {manifest_a}.part-0001; preserve and verify a copy outside tasks/files/{} if needed, remove the unledgered file to restore the committed preimage, and retry or reconcile",
        f.id, f.id
    );
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .starts_with(&expected_prefix),
        "got: {err}"
    );
    assert!(
        part1_a.is_file(),
        "leftover unledgered member must never be silently deleted"
    );

    // Restoring the exact scratch member lets retry adopt `.part-0001` and complete cleanly.
    std::fs::write(&member0, &original_member0).unwrap();
    let res = f.json(&args);
    assert_eq!(res["outcome"], "verified", "{res}");
    assert_eq!(f.state()["complete"], true);
    assert_eq!(f.json(&["check"])["errors"], json!([]));
}

#[test]
fn reconcile_degrades_over_cap_pending_manifest_to_kept_unknown() {
    let f = checkpoint_fixture();
    f.check_hook();

    let container = f.dir.join("over-cap-reconcile");
    let (manifest_bytes, _) = build_approx_5k_container_recovery(&container);

    let cp_req = checkpoint_request(&f, 740, json!([declaration(&container, "scratch")]));
    let cp_path = f.request("cp-pending-over-cap.json", &cp_req);
    let stopped = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "intent")
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            cp_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");

    // Tighten single-file cap to 2048 while keeping `multipart_validators` undeclared.
    configure_attachments_recovery(&f.records, "max_bytes = 2048\n");

    let rec_req = json!({
        "version": 1,
        "operation": {
            "coordinator": f.id,
            "operation_id": cp_req["operation_id"],
        },
        "reconciliation_id": format!("{:032x}", 741u64),
        "inputs": [{"task": f.id, "basis": f.state()["basis"]}],
        "reason": "Reconcile pending over-cap scratch container without multipart configured"
    });
    let res = reconcile(&f, &rec_req);
    assert_eq!(res["outcome"], "reconciled", "{res}");
    assert_eq!(res["counts"]["kept_unknown"], 1, "{res}");
    assert_eq!(res["counts"]["kept_verified"], 0, "{res}");

    let expected_detail = format!(
        "generated manifest ({} bytes) exceeds 2048 byte single-file attachment cap during reconciliation; custody retained as unknown — configure [attachments] multipart_validators and run a fresh tasks retained checkpoint, or use the manual off-container manifest route",
        manifest_bytes.len()
    );
    let expected_action = format!("Restore and inspect exact retained bytes: {expected_detail}");

    let state = f.state();
    assert_eq!(state["complete"], true, "{state}");
    assert_eq!(state["items"][0]["record"]["state"], "kept");
    assert_eq!(state["items"][0]["record"]["verification"], "unknown");
    assert!(state["items"][0]["record"]["sha256"].is_null());
    assert_eq!(
        state["items"][0]["record"]["verification_action"],
        expected_action
    );
    assert_eq!(
        res["actions"],
        json!([format!("{}: {expected_action}", f.id)])
    );

    f.json(&[
        "edit",
        &f.id,
        "--title",
        "Reservation ended cleanly after cap-safe reconcile",
    ]);
    assert_eq!(f.json(&["check"])["errors"], json!([]));
}
