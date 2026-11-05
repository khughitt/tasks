mod common;
use common::retained::{
    RetainedEnv, checkpoint, checkpoint_fixture, checkpoint_request, cross_receiver, declaration,
    host, transfer, transfer_fixture, transfer_request,
};
use serde_json::json;

#[test]
fn retained_metrics_diagnostic_counts_readonly_calls() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let out = f
        .cmd()
        .env("TASKS_RETAINED_TEST_METRICS", "1")
        .args(["retained", "state", &f.id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let raw = stderr
        .lines()
        .find_map(|line| line.strip_prefix("retained test command metrics: "))
        .expect("explicit test-only command metric diagnostic");
    let metrics: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert!(metrics["history_bytes_read"].as_u64().unwrap() > 0);
    assert!(metrics["wall_ns"].as_u64().unwrap() > 0);
    assert!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .unwrap()
            .get("metrics")
            .is_none()
    );
}

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
    f.seed_note(&format!("retention checkpoint: {}", cp(&f, basis)));
    let s = f.state();
    assert_eq!(s["complete"], false);
    assert_eq!(s["items"][0]["record"], serde_json::Value::Null);
}
#[test]
fn retained_state_checkpoint_intervening_change() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let basis = f.state()["basis"].clone();
    f.seed_note(&format!(
        "retention operation: {}",
        op(&f, basis.clone(), "intent", json!({}))
    ));
    // This deliberately malformed request is not evidence for a successful state query.
    let mut basis = basis;
    basis["commit"] = json!(f.basis());
    let mut r = f.record();
    r["purpose"] = json!("Changed after observation");
    f.note(r);
    f.seed_note(&format!("retention checkpoint: {}", cp(&f, basis)));
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
    f.seed_note(&format!(
        "retention operation: {}",
        op(&f, basis.clone(), "intent", json!({}))
    ));
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
    f.seed_note(&format!("retention operation: {receipt}"));
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

fn retained_notes(f: &RetainedEnv) -> usize {
    f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["text"].as_str().unwrap().starts_with("retained: "))
        .count()
}
#[test]
fn retained_checkpoint_unchanged() {
    let f = checkpoint_fixture();
    let tree = f.dir.join("tree");
    std::fs::create_dir(&tree).unwrap();
    std::fs::write(tree.join("report"), "payload").unwrap();
    let first = checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&tree, "scratch")])),
    );
    assert_eq!(first["outcome"], "verified");
    let before = f.state();
    let count = retained_notes(&f);
    let manifests = || {
        std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .starts_with("retained-manifest-")
            })
            .count()
    };
    assert_eq!(manifests(), 1);
    let second = checkpoint(&f, &checkpoint_request(&f, 2, json!([])));
    assert_eq!(second["counts"]["kept_verified"], 1);
    assert_eq!(retained_notes(&f), count);
    assert_eq!(manifests(), 1);
    assert_eq!(
        f.state()["items"][0]["record"],
        before["items"][0]["record"]
    );
    assert!(second["metrics"]["bytes_hashed"].as_u64().unwrap() >= 7);
    assert!(second["metrics"]["history_bytes_read"].as_u64().unwrap() > 0);
    assert_eq!(
        f.state()["checkpoints"].as_array().unwrap().last().unwrap()["note"]["changed"],
        0
    );
}
#[test]
fn retained_checkpoint_unknown() {
    let f = checkpoint_fixture();
    let missing = f.dir.join("missing");
    let result = checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&missing, "evidence")])),
    );
    assert_eq!(result["outcome"], "accounted_unknown");
    assert_eq!(result["counts"]["kept_unknown"], 1);
    let state = f.state();
    assert_eq!(state["items"][0]["record"]["state"], "kept");
    assert!(state["items"][0]["record"]["sha256"].is_null());
    assert!(!result["actions"].as_array().unwrap().is_empty());
    assert_eq!(state["checkpoints"][0]["note"]["complete"], true);
}
#[test]
fn retained_checkpoint_changed() {
    let f = checkpoint_fixture();
    let file = f.dir.join("payload");
    std::fs::write(&file, "old").unwrap();
    common::git(&f.dir, &["add", "payload"]);
    common::git(&f.dir, &["commit", "-qm", "old payload"]);
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    let old = f.state();
    let identity = old["items"][0]["identity"].clone();
    let prior = json!({"type":"git","basis":old["basis"],"path":"payload","sha256":old["items"][0]["record"]["sha256"]});
    std::fs::write(&file, "new").unwrap();
    let mut request = checkpoint_request(&f, 2, json!([]));
    request["preserved_previous"] = json!([{"identity":identity,"artifact":prior}]);
    assert_eq!(checkpoint(&f, &request)["outcome"], "verified");
    let new = f.state();
    assert_ne!(
        new["items"][0]["record"]["sha256"],
        old["items"][0]["record"]["sha256"]
    );
    f.json(&[
        "attach",
        &f.id,
        file.to_str().unwrap(),
        "--name",
        "prior-new.txt",
    ]);
    let prior = json!({"type":"attachment","task":f.id,"name":"prior-new.txt","basis":f.state()["basis"],"sha256":new["items"][0]["record"]["sha256"]});
    std::fs::write(&file, "old").unwrap();
    let mut request = checkpoint_request(&f, 3, json!([]));
    request["preserved_previous"] = json!([{"identity":identity,"artifact":prior}]);
    assert_eq!(checkpoint(&f, &request)["outcome"], "verified");
    assert_eq!(
        f.state()["items"][0]["record"]["sha256"],
        old["items"][0]["record"]["sha256"]
    );
    std::fs::write(&file, "unexpected").unwrap();
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 4, json!([])))["outcome"],
        "accounted_unknown"
    );
}
#[test]
fn retained_checkpoint_resolution() {
    let f = checkpoint_fixture();
    let file = f.dir.join("payload");
    std::fs::write(&file, "data").unwrap();
    let mut bad = f.record();
    bad["host"] = json!(host());
    bad["path"] = json!(file.to_str().unwrap());
    bad["sha256"] = json!("broken");
    f.note(bad);
    let state = f.state();
    assert_eq!(state["complete"], false);
    let mut request = checkpoint_request(&f, 1, json!([]));
    request["resolutions"] = json!([{"bad":state["diagnostics"][0]["source"],"declarations":[declaration(&file,"evidence")],"reason":"Recovered exact identity and complete terms"}]);
    assert_eq!(checkpoint(&f, &request)["outcome"], "verified");
    let state = f.state();
    assert_eq!(state["complete"], true);
    assert!(!state["diagnostics"][0]["resolved_by"].is_null());
}
#[test]
fn retained_checkpoint_remote_only() {
    let f = checkpoint_fixture();
    f.note(f.record());
    let result = checkpoint(&f, &checkpoint_request(&f, 1, json!([])));
    assert_eq!(result["outcome"], "no_local_items");
    assert!(f.state()["checkpoints"].as_array().unwrap().is_empty());
    assert_eq!(f.state()["items"][0]["record"]["host"], "fixture-host");
}
#[test]
fn retained_checkpoint_unsupported() {
    let f = checkpoint_fixture();
    let mut request = checkpoint_request(&f, 1, json!([]));
    request["host"] = json!("another-host");
    let path = f.request("request.json", &request);
    let head = f.basis();
    assert!(
        !f.cmd()
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
            .success()
    );
    assert_eq!(f.basis(), head);
    let config = f.records.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("commit = true", "commit = false");
    std::fs::write(config, text).unwrap();
    let request = checkpoint_request(&f, 2, json!([]));
    let path = f.request("request.json", &request);
    assert!(
        !f.cmd()
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
            .success()
    );
    assert_eq!(f.basis(), head);
}
#[test]
fn retained_checkpoint_retry() {
    let f = checkpoint_fixture();
    let file = f.dir.join("payload");
    std::fs::write(&file, "payload").unwrap();
    let request = checkpoint_request(&f, 1, json!([declaration(&file, "evidence")]));
    let first = checkpoint(&f, &request);
    let head = f.basis();
    f.json(&["note", &f.id, "Ordinary later progress"]);
    let later = f.basis();
    assert_ne!(head, later);
    let path = f.request("retry.json", &request);
    std::fs::write(&path, serde_json::to_string_pretty(&request).unwrap()).unwrap();
    std::fs::remove_file(&file).unwrap();
    let replay = f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(f.basis(), later);
    assert_eq!(replay["publications"], first["publications"]);
    assert_eq!(replay["counts"], first["counts"]);
    assert_eq!(replay["outcome"], first["outcome"]);
}
#[test]
fn retained_checkpoint_historical_and_whole_host() {
    let f = checkpoint_fixture();
    for name in ["one", "two"] {
        let path = f.dir.join(name);
        std::fs::write(&path, "payload").unwrap();
        let mut r = f.record();
        r.as_object_mut().unwrap().remove("version");
        r["host"] = json!(host());
        r["path"] = json!(path.to_str().unwrap());
        r["verification"] = json!("unknown");
        r["sha256"] = serde_json::Value::Null;
        r["verification_action"] = json!("Inspect");
        f.note(r);
    }
    let result = checkpoint(&f, &checkpoint_request(&f, 1, json!([])));
    assert_eq!(result["counts"]["kept_verified"], 2);
    for item in f.state()["items"].as_array().unwrap() {
        assert_eq!(item["record"]["version"], 1);
    }
    assert_eq!(retained_notes(&f), 4);
}
#[test]
fn retained_checkpoint_stale_and_conflicting() {
    let f = checkpoint_fixture();
    let request = checkpoint_request(&f, 1, json!([]));
    f.json(&["note", &f.id, "Changed after basis"]);
    let path = f.request("request.json", &request);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(f.basis(), before);
    let mut request = checkpoint_request(&f, 2, json!([]));
    checkpoint(&f, &request);
    let before = f.basis();
    request["declarations"] = json!([declaration(&f.dir.join("new"), "evidence")]);
    let path = f.request("request.json", &request);
    assert!(
        !f.cmd()
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
            .success()
    );
    assert_eq!(f.basis(), before);
}
#[test]
fn retained_checkpoint_formats_and_nongit() {
    let f = checkpoint_fixture();
    let root = f.dir.join("tree");
    std::fs::create_dir(&root).unwrap();
    let manifest = f.dir.join("old.json");
    std::fs::write(&manifest, b"{\"version\":2,\"entries\":[]}").unwrap();
    use sha2::{Digest, Sha256};
    let mut record = f.record();
    record["host"] = json!(host());
    record["kind"] = json!("scratch");
    record["path"] = json!(root.to_str().unwrap());
    record["manifest"] = json!(manifest.to_str().unwrap());
    record["sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(std::fs::read(&manifest).unwrap())
    ));
    f.note(record);
    let request = checkpoint_request(&f, 1, json!([]));
    let path = f.request("request.json", &request);
    let before = f.basis();
    assert!(
        !f.cmd()
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
            .success()
    );
    assert_eq!(f.basis(), before);
    let task = f.records.join(format!("tasks/{}.md", f.id));
    let before = std::fs::read(&task).unwrap();
    std::fs::remove_dir_all(f.dir.join(".git")).unwrap();
    assert!(
        !f.cmd()
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
            .success()
    );
    assert_eq!(std::fs::read(task).unwrap(), before);
}
#[test]
fn retained_checkpoint_recursive_metadata() {
    let f = checkpoint_fixture();
    let request = checkpoint_request(&f, 1, json!([declaration(&f.dir, "scratch")]));
    let path = f.request("request.json", &request);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
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
fn retained_checkpoint_scope_resolution() {
    for scope in ["host", "task"] {
        let f = checkpoint_fixture();
        let files = [f.dir.join("one"), f.dir.join("two")];
        for file in &files {
            std::fs::write(file, "data").unwrap();
            let mut record = f.record();
            record["host"] = json!(host());
            record["path"] = json!(file.to_str().unwrap());
            f.note(record);
        }
        let raw = if scope == "host" {
            json!({"host":host(),"path":42})
        } else {
            json!({"host":42})
        };
        f.seed_note(&format!("retained: {raw}"));
        let state = f.state();
        assert_eq!(state["diagnostics"][0]["scope"], scope);
        let mut request = checkpoint_request(&f, 1, json!([]));
        request["resolutions"] = json!([{"bad":state["diagnostics"][0]["source"],"declarations":[declaration(&files[0],"evidence")],"reason":"Incomplete inventory"}]);
        let path = f.request("request.json", &request);
        let before = f.basis();
        assert!(
            !f.cmd()
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
                .success()
        );
        assert_eq!(f.basis(), before);
        request["resolutions"][0]["declarations"] = json!([
            declaration(&files[0], "evidence"),
            declaration(&files[1], "evidence")
        ]);
        assert_eq!(checkpoint(&f, &request)["outcome"], "verified");
        assert_eq!(f.state()["complete"], true);
    }
}
#[test]
fn retained_checkpoint_layouts() {
    for layout in ["in-tree", "detached", "linked"] {
        let f = RetainedEnv::new(layout);
        f.json(&["start", &f.id]);
        f.check_hook();
        let file = f.dir.join("payload");
        std::fs::write(&file, "payload").unwrap();
        let result = checkpoint(
            &f,
            &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
        );
        assert_eq!(result["outcome"], "verified", "{layout}");
        assert!(
            result["publications"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["basis"]["layout"] == layout)
        );
    }
}
#[test]
fn retained_checkpoint_unknown_keeps_prior_version_requirement() {
    let f = checkpoint_fixture();
    let file = f.dir.join("payload");
    std::fs::write(&file, "old").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    f.json(&[
        "attach",
        &f.id,
        file.to_str().unwrap(),
        "--name",
        "old-payload.txt",
    ]);
    let old = f.state();
    let artifact = json!({"type":"attachment","task":f.id,"name":"old-payload.txt","basis":old["basis"],"sha256":old["items"][0]["record"]["sha256"]});
    std::fs::write(&file, "new").unwrap();
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 2, json!([])))["outcome"],
        "accounted_unknown"
    );
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 3, json!([])))["outcome"],
        "accounted_unknown",
        "a later observation must not forget the missing prior version"
    );
    let mut request = checkpoint_request(&f, 4, json!([]));
    request["preserved_previous"] =
        json!([{"identity":old["items"][0]["identity"],"artifact":artifact}]);
    assert_eq!(checkpoint(&f, &request)["outcome"], "verified");
}

#[test]
fn retained_checkpoint_rechecks_preserved_directory_versions() {
    let f = checkpoint_fixture();
    let tree = f.dir.join("tree");
    let prior = f.dir.join("prior");
    let restored = f.dir.join("restored");
    for dir in [&tree, &prior, &restored] {
        std::fs::create_dir(dir).unwrap();
    }
    for dir in [&tree, &prior, &restored] {
        std::fs::write(dir.join("report"), "old").unwrap();
    }
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&tree, "scratch")])),
    );
    let old = f.state();
    let record = &old["items"][0]["record"];
    let manifest = std::path::Path::new(record["manifest"].as_str().unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let mut locator = json!({"type":"directory","host":host(),"path":prior.to_str().unwrap(),"manifest":{"type":"attachment","task":f.id,"name":manifest,"basis":old["basis"],"sha256":record["sha256"]}});
    let mut req = checkpoint_request(&f, 2, json!([]));
    req["preserved_previous"] =
        json!([{"identity":old["items"][0]["identity"],"artifact":locator}]);
    std::fs::write(tree.join("report"), "new").unwrap();
    assert_eq!(checkpoint(&f, &req)["outcome"], "verified");
    std::fs::remove_file(prior.join("report")).unwrap();
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 3, json!([])))["outcome"],
        "accounted_unknown",
        "an unchanged current tree cannot hide missing prior payloads"
    );
    locator["path"] = json!(restored.to_str().unwrap());
    let mut req = checkpoint_request(&f, 4, json!([]));
    req["preserved_previous"] =
        json!([{"identity":old["items"][0]["identity"],"artifact":locator}]);
    assert_eq!(checkpoint(&f, &req)["outcome"], "verified");
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 5, json!([])))["outcome"],
        "verified",
        "the explicitly verified replacement locator survives later requests"
    );
    std::fs::remove_file(restored.join("report")).unwrap();
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 6, json!([])))["outcome"],
        "accounted_unknown"
    );
}

#[test]
fn retained_transfer_time_and_purpose() {
    for future in [false, true] {
        let (f, to, _file) = transfer_fixture();
        if future {
            let p = f.records.join(format!("tasks/{to}.md"));
            let mut bytes = std::fs::read_to_string(&p).unwrap();
            let start = bytes.find("updated: ").unwrap() + "updated: ".len();
            let end = start + bytes[start..].find('\n').unwrap();
            bytes.replace_range(start..end, "2100-01-01T00:00:00Z");
            std::fs::write(&p, bytes).unwrap();
            common::git(&f.records, &["add", "tasks"]);
            common::git(&f.records, &["commit", "-qm", "future receiver floor"]);
        }
        let old = f.json(&["show", &f.id])["task"]["updated"]
            .as_str()
            .unwrap()
            .to_string();
        let result = transfer(&f, &to, &transfer_request(&f, &to));
        assert_eq!(result["outcome"], "transferred");
        let sender = f.json(&["show", &f.id]);
        let receiver = f.json(&["show", &to]);
        let note = |task: &serde_json::Value, state: &str| {
            task["task"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .find(|n| {
                    n["text"].as_str().is_some_and(|t| {
                        t.starts_with("retained: ")
                            && serde_json::from_str::<serde_json::Value>(&t[10..]).unwrap()["state"]
                                == state
                    })
                })
                .unwrap()
                .clone()
        };
        let released = note(&sender, "released");
        let kept = note(&receiver, "kept");
        let parsed: serde_json::Value =
            serde_json::from_str(&released["text"].as_str().unwrap()[10..]).unwrap();
        assert_eq!(
            parsed["purpose"],
            "Transferred to sci-000002; custody transferred and bytes remain in place"
        );
        assert!(released["at"].as_str().unwrap() > kept["at"].as_str().unwrap());
        assert!(released["at"].as_str().unwrap() > old.as_str());
        assert!(sender["task"]["updated"].as_str().unwrap() >= released["at"].as_str().unwrap());
        assert_eq!(parsed["next_owner"], "none");
        assert!(parsed.get("sha256").is_none());
        if future && let Some(dir) = std::env::var_os("TASKS_RETAINED_FUTURE_FIXTURE_OUT") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir(&dir).expect("new explicit fixture directory");
            let fixture = json!({"format":"tasks-retained-generated-future-fixture-1","sender":sender["task"],"receiver":receiver["task"],"result":result});
            std::fs::write(
                dir.join("transfer-future-generated.json"),
                serde_json::to_vec_pretty(&fixture).unwrap(),
            )
            .unwrap();
        }
    }
}
#[test]
fn retained_transfer_same_repo() {
    let (f, to, file) = transfer_fixture();
    f.check_hook();
    let request = transfer_request(&f, &to);
    let result = transfer(&f, &to, &request);
    let publications = result["publications"].as_array().unwrap();
    let receiver = publications
        .iter()
        .find(|p| p["task"] == to && p["phase"] == "receiver_published")
        .unwrap();
    let sender = publications
        .iter()
        .find(|p| p["task"] == f.id && p["phase"] == "sender_released")
        .unwrap();
    assert_ne!(receiver["basis"]["commit"], sender["basis"]["commit"]);
    let out = common::git_fixture::command(f.env.home.path(), &f.records)
        .args([
            "merge-base",
            "--is-ancestor",
            receiver["basis"]["commit"].as_str().unwrap(),
            sender["basis"]["commit"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(std::fs::read(&file).unwrap(), b"payload");
    assert_eq!(f.state()["items"][0]["record"]["state"], "released");
    assert_eq!(
        f.json(&["retained", "state", &to])["items"][0]["record"]["next_owner"],
        format!("task:{to}")
    );
    let before = f.basis();
    std::fs::remove_file(file).unwrap();
    let replay = transfer(&f, &to, &request);
    assert_eq!(replay["publications"], result["publications"]);
    assert!(!replay["warnings"].as_array().unwrap().is_empty());
    assert_eq!(f.basis(), before);
}
#[test]
fn retained_transfer_conflict() {
    let (mut f, to, _) = transfer_fixture();
    let record = f.state()["items"][0]["record"].clone();
    let sender = f.id.clone();
    f.id = to.clone();
    f.note(record);
    f.id = sender;
    let request = transfer_request(&f, &to);
    let before = f.basis();
    let path = f.request("transfer.json", &request);
    let out = f
        .cmd()
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
    assert!(!out.status.success());
    assert_eq!(
        f.basis(),
        before,
        "conflicts are refused before durable intent"
    );
}
#[test]
fn retained_transfer_retry() {
    for boundary in [
        "intent",
        "save:intent",
        "commit:intent",
        "receiver_intent",
        "save:receiver_published",
        "commit:receiver_published",
        "receiver_published",
        "save:sender_released",
        "sender_released",
        "commit:sender_released",
        "commit:completed",
        "completed",
    ] {
        let (f, to, _) = transfer_fixture();
        f.check_hook();
        let request = transfer_request(&f, &to);
        let path = f.request("transfer.json", &request);
        let out = f
            .cmd()
            .env("TASKS_RETAINED_STOP_AFTER", boundary)
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
        assert!(!out.status.success(), "{boundary}");
        let result = transfer(&f, &to, &request);
        assert_eq!(result["outcome"], "transferred", "{boundary}");
        for (id, state) in [(&f.id, "released"), (&to, "kept")] {
            let task = f.json(&["show", id]);
            let count = task["task"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| {
                    n["text"].as_str().is_some_and(|t| {
                        t.starts_with("retained: ")
                            && serde_json::from_str::<serde_json::Value>(&t[10..]).unwrap()["state"]
                                == state
                    })
                })
                .count();
            assert_eq!(count, 1, "{boundary} {id}");
        }
    }
}
#[test]
fn retained_transfer_changed_before_release() {
    let (f, to, file) = transfer_fixture();
    let request = transfer_request(&f, &to);
    let path = f.request("transfer.json", &request);
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
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
    assert!(!out.status.success());
    std::fs::write(file, "tampered").unwrap();
    let out = f
        .cmd()
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
    assert!(!out.status.success());
    assert_eq!(f.state()["items"][0]["record"]["state"], "kept");
    assert_eq!(
        f.json(&["retained", "state", &to])["items"][0]["record"]["state"],
        "kept"
    );
}

#[test]
fn retained_transfer_cross_repo() {
    for shared in [false, true] {
        let (f, _, file) = transfer_fixture();
        let (_root, to) = cross_receiver(&f, shared);
        let result = transfer(&f, &to, &transfer_request(&f, &to));
        assert_eq!(result["outcome"], "transferred");
        assert_eq!(
            f.json(&["retained", "state", &to])["items"][0]["record"]["path"],
            file.to_str().unwrap()
        );
        assert_eq!(std::fs::read(file).unwrap(), b"payload");
    }
}
#[test]
fn retained_transfer_foreign_worktree() {
    let (f, _, _) = transfer_fixture();
    let (_root, to) = cross_receiver(&f, false);
    std::fs::write(f.dir.join(".gitignore"), ".worktrees/\n").unwrap();
    common::git(&f.dir, &["add", ".gitignore"]);
    common::git(&f.dir, &["commit", "-qm", "worktree fixture"]);
    let tree = f.dir.join(".worktrees/source");
    common::git(
        &f.dir,
        &["worktree", "add", "-qb", "source", tree.to_str().unwrap()],
    );
    common::git_fixture::assert_worktree(f.env.home.path(), &f.dir, &tree);
    let mut request = checkpoint_request(
        &f,
        2,
        json!([{"kind":"worktree","path":".worktrees/source","purpose":"Review branch","cleanup_trigger":"Branch integrated"}]),
    );
    checkpoint(&f, &request);
    request = transfer_request(&f, &to);
    let source = f.state()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["kind"] == "worktree")
        .unwrap()
        .clone();
    request["items"] = json!([source["identity"]]);
    assert_eq!(transfer(&f, &to, &request)["outcome"], "transferred");
    let state = f.json(&["retained", "state", &to]);
    assert_eq!(
        state["items"][0]["record"]["path"],
        tree.canonicalize().unwrap().to_str().unwrap()
    );
    assert!(state["items"][0]["record"].get("sha256").is_none());
}
#[test]
fn retained_transfer_exception() {
    let (f, to, _) = transfer_fixture();
    let tree = f.dir.join("tree");
    std::fs::create_dir(&tree).unwrap();
    let child = tree.join("report");
    std::fs::write(&child, "independent").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(
            &f,
            2,
            json!([
                declaration(&tree, "scratch"),
                declaration(&child, "evidence")
            ]),
        ),
    );
    let third = f.json(&["add", "Independent owner"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut independent = f.state()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["path"] == child.to_str().unwrap())
        .unwrap()["record"]
        .clone();
    independent["next_owner"] = json!(format!("task:{third}"));
    f.note(independent.clone());
    let mut request = transfer_request(&f, &to);
    request["items"] = json!([f.state()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["kind"] == "scratch")
        .unwrap()["identity"]]);
    assert_eq!(transfer(&f, &to, &request)["outcome"], "transferred");
    let state = f.state();
    let actual = &state["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["path"] == child.to_str().unwrap())
        .unwrap()["record"];
    assert_eq!(*actual, independent);
    assert_eq!(
        f.json(&["retained", "state", &to])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn retained_transfer_historical_counts() {
    let (mut f, to, _) = transfer_fixture();
    let request = transfer_request(&f, &to);
    let result = transfer(&f, &to, &request);
    let mut released = f.state()["items"][0]["record"].clone();
    released["purpose"] =
        json!("Transferred to sci-000003; custody transferred and bytes remain in place");
    let from = f.id.clone();
    f.id = to.clone();
    f.note(released);
    f.id = from;
    let replay = transfer(&f, &to, &request);
    assert_eq!(
        replay["counts"], result["counts"],
        "historical replay cannot use today's changed receiver custody"
    );
}
#[test]
fn retained_transfer_after_release_finishes_with_new_receiver_work() {
    let (f, to, file) = transfer_fixture();
    let request = transfer_request(&f, &to);
    let path = f.request("transfer.json", &request);
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "commit:completed")
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
    assert!(!out.status.success());
    assert_eq!(f.state()["items"][0]["record"]["state"], "released");
    let next = json!({"version":1,"operation_id":"b".repeat(32),"host":host(),"inputs":[{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}]});
    let next_path = f.request("receiver-checkpoint.json", &next);
    f.json(&[
        "retained",
        "checkpoint",
        &to,
        "--request",
        next_path.to_str().unwrap(),
    ]);
    std::fs::remove_file(file).unwrap();
    assert_eq!(
        transfer(&f, &to, &request)["outcome"],
        "transferred",
        "committed release needs only original final receipts"
    );
}

#[test]
fn retained_transfer_consumer_fixture() {
    use sha2::{Digest, Sha256};
    let (f, to, _) = transfer_fixture();
    f.check_hook();
    let request = transfer_request(&f, &to);
    let result = transfer(&f, &to, &request);
    let fixture = json!({"format":"tasks-retained-generated-fixture-1","producer_transfer_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src/retained/transfer.rs"))),"request":request,"result":result,"sender":f.json(&["show",&f.id])["task"],"receiver":f.json(&["show",&to])["task"],"sender_state":f.state(),"receiver_state":f.json(&["retained","state",&to]),"expected":{"outcome":"transferred","release_kind":"transfer","target":to,"forbidden_findings":["release_disposition_unknown","identity_release_unestablished"],"bytes_inspected_by_consumer":false}});
    assert_eq!(fixture["sender_state"]["complete"], true);
    assert_eq!(fixture["receiver_state"]["complete"], true);
    if let Some(dir) = std::env::var_os("TASKS_RETAINED_FIXTURE_OUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir(&dir).expect("fixture exports need a new explicit directory");
        std::fs::write(
            dir.join("transfer-generated.json"),
            serde_json::to_vec_pretty(&fixture).unwrap(),
        )
        .unwrap();
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args([
                "bundle",
                "create",
                dir.join("transfer-records.bundle").to_str().unwrap(),
                "refs/heads/main",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
}

#[test]
fn retained_checkpoint_retry_rejects_unbound_retention_note() {
    let f = checkpoint_fixture();
    let file = f.dir.join("payload");
    std::fs::write(&file, "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    let request = checkpoint_request(&f, 2, json!([]));
    let path = f.request("request.json", &request);
    assert!(
        !f.cmd()
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
            .success()
    );
    let mut foreign = f.state()["items"][0]["record"].clone();
    foreign["purpose"] = json!("Uncoordinated external change");
    f.note(foreign);
    let before = f.basis();
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "an unrelated reserved note cannot be adopted into the original intent"
    );
    assert_eq!(f.basis(), before);
}
#[test]
fn retained_transfer_reverifies_unknown_source() {
    let (f, to, _) = transfer_fixture();
    let mut unknown = f.state()["items"][0]["record"].clone();
    unknown["verification"] = json!("unknown");
    unknown["sha256"] = serde_json::Value::Null;
    unknown["verification_action"] = json!("Reinspect after temporary access failure");
    f.note(unknown);
    assert_eq!(
        transfer(&f, &to, &transfer_request(&f, &to))["outcome"],
        "transferred",
        "fresh full inspection can establish an unknown source's unchanged required version"
    );
}

#[test]
fn retained_transfer_inherits_prior_versions() {
    let (f, to, _) = transfer_fixture();
    let tree = f.dir.join("tree");
    let prior = f.dir.join("prior");
    for dir in [&tree, &prior] {
        std::fs::create_dir(dir).unwrap();
        std::fs::write(dir.join("report"), "old").unwrap();
    }
    checkpoint(
        &f,
        &checkpoint_request(&f, 2, json!([declaration(&tree, "scratch")])),
    );
    let old = f.state();
    let item = old["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["kind"] == "scratch")
        .unwrap();
    let name = std::path::Path::new(item["record"]["manifest"].as_str().unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let mut req = checkpoint_request(&f, 3, json!([]));
    req["preserved_previous"] = json!([{"identity":item["identity"],"artifact":{"type":"directory","host":host(),"path":prior.to_str().unwrap(),"manifest":{"type":"attachment","task":f.id,"basis":old["basis"],"name":name,"sha256":item["record"]["sha256"]}}}]);
    std::fs::write(tree.join("report"), "new").unwrap();
    assert_eq!(checkpoint(&f, &req)["outcome"], "verified");
    let mut req = transfer_request(&f, &to);
    req["items"] = json!([item["identity"]]);
    assert_eq!(transfer(&f, &to, &req)["outcome"], "transferred");
    std::fs::remove_file(prior.join("report")).unwrap();
    let req = json!({"version":1,"operation_id":"b".repeat(32),"host":host(),"inputs":[{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}]});
    let path = f.request("receiver-checkpoint.json", &req);
    assert_eq!(
        f.json(&[
            "retained",
            "checkpoint",
            &to,
            "--request",
            path.to_str().unwrap()
        ])["outcome"],
        "accounted_unknown",
        "transfer cannot sever the exact prior-payload obligations"
    );
}
#[test]
fn retained_transfer_dirty_receiver_evidence() {
    let (f, to, _) = transfer_fixture();
    let mut record = f.record();
    record["host"] = json!(host());
    record["kind"] = json!("worktree");
    record["path"] = json!(f.dir.to_str().unwrap());
    record.as_object_mut().unwrap().remove("sha256");
    f.note(record);
    let mut req = transfer_request(&f, &to);
    req["items"] = json!([f.state()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"]["kind"] == "worktree")
        .unwrap()["identity"]]);
    let path = f.request("transfer.json", &req);
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
    let state = f.json(&["retained", "state", &to]);
    let receipt = state["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["note"]["phase"] == "receiver_published")
        .unwrap();
    let name = receipt["note"]["outputs"]["artifacts"][0]["name"]
        .as_str()
        .unwrap();
    let artifact = f.records.join(format!("tasks/files/{to}/{name}"));
    let mut bytes = std::fs::read(&artifact).unwrap();
    bytes[0] = b'!';
    std::fs::write(artifact, bytes).unwrap();
    assert!(
        !f.cmd()
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
    let sender = f.state();
    assert_eq!(
        sender["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["identity"]["kind"] == "worktree")
            .unwrap()["record"]["state"],
        "kept"
    );
}

#[test]
fn retained_state_proves_immutable_operation_dependencies() {
    for corruption in ["request", "publication"] {
        let (f, to, _) = transfer_fixture();
        transfer(&f, &to, &transfer_request(&f, &to));
        let original = f.state();
        let task = f.records.join(format!("tasks/{}.md", f.id));
        let raw = std::fs::read_to_string(&task).unwrap();
        let changed = match corruption {
            "request" => raw.replace(
                original["operations"][0]["note"]["request"]["sha256"]
                    .as_str()
                    .unwrap(),
                &"0".repeat(64),
            ),
            "publication" => {
                let completed = &original["operations"][2]["note"];
                raw.replace(
                    completed["outputs"]["publications"][0]["basis"]["commit"]
                        .as_str()
                        .unwrap(),
                    completed["input_bases"][0]["basis"]["commit"]
                        .as_str()
                        .unwrap(),
                )
            }
            _ => unreachable!(),
        };
        assert_ne!(raw, changed);
        std::fs::write(&task, changed).unwrap();
        common::git(&f.records, &["add", "."]);
        common::git(
            &f.records,
            &["commit", "-qm", "corrupt immutable receipt dependency"],
        );
        let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
        assert!(
            !out.status.success(),
            "{corruption}: corrupt dependency reported complete"
        );
        assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
        f.json(&[
            "retained",
            "state",
            &f.id,
            "--at",
            original["basis"]["commit"].as_str().unwrap(),
        ]);
    }
}

#[test]
fn retained_state_uses_historical_artifacts_without_current_bytes() {
    let f = checkpoint_fixture();
    let tree = f.dir.join("payload");
    std::fs::create_dir(&tree).unwrap();
    std::fs::write(tree.join("report"), "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&tree, "scratch")])),
    );
    let state = f.state();
    let name = state["operations"][0]["note"]["request"]["name"]
        .as_str()
        .unwrap();
    std::fs::write(
        f.records.join(format!("tasks/files/{}/{name}", f.id)),
        "changed current request",
    )
    .unwrap();
    std::fs::remove_dir_all(tree).unwrap();
    common::git(&f.records, &["add", "."]);
    common::git(
        &f.records,
        &["commit", "-qm", "change only current artifact bytes"],
    );
    assert_eq!(f.state()["complete"], true);
}

#[test]
fn review_retained_checkpoint_refuses_multipart_artifact_name() {
    let f = checkpoint_fixture();
    let config = f.records.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        config,
        format!("{text}\n[attachments]\nmultipart_validators = []\n"),
    )
    .unwrap();
    let name =
        "retained-manifest-8bcae181d21321b79e1cee4976a6c9759a0f03769dde64b6f821ab5d22a33921.json";
    let source = f.dir.join("source");
    std::fs::write(&source, "{}").unwrap();
    let attached = f.json(&[
        "attach",
        &f.id,
        source.to_str().unwrap(),
        "--multipart",
        "--name",
        name,
    ]);
    assert_eq!(attached["publication"]["state"], "published");
    let scratch = f.dir.join("scratch");
    std::fs::create_dir(&scratch).unwrap();
    let request = checkpoint_request(&f, 1, json!([declaration(&scratch, "scratch")]));
    let path = f.request("request.json", &request);
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "attachment_exists");
    let stored = f.records.join(format!("tasks/files/{}/{name}", f.id));
    assert!(!stored.exists());
    let out = f
        .cmd()
        .args(["export", &f.id, name, "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"{}");
}
