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
fn retained_state_historical_relative_revision() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let old = f.basis();
    let mut next = f.record();
    next["purpose"] = json!("Second review");
    f.note(next);
    let path = f.records.join(format!("tasks/{}.md", f.id));
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, format!("{bytes}\n")).unwrap();
    let current = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
    assert_eq!(current.status.code(), Some(1));
    for revision in ["HEAD~1", "main~1"] {
        let state = f.json(&["retained", "state", &f.id, "--at", revision]);
        assert_eq!(state["basis"]["commit"], old);
        assert_eq!(state["items"][0]["record"]["purpose"], "Review");
    }
}

#[test]
fn retained_state_historical_named_revisions() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let old = f.basis();
    common::git_fixture::assert_repo(f.env.home.path(), &f.records);
    for args in [["branch", "past", &old], ["tag", "retained-tag", &old]] {
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
    let mut next = f.record();
    next["purpose"] = json!("Second review");
    f.note(next);
    let current = f.basis();
    let path = f.records.join(format!("tasks/{}.md", f.id));
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, format!("{bytes}\n")).unwrap();
    for revision in ["past", "retained-tag", &old[..12], "main"] {
        let (expected, purpose) = if revision == "main" {
            (&current, "Second review")
        } else {
            (&old, "Review")
        };
        let state = f.json(&["retained", "state", &f.id, "--at", revision]);
        assert_eq!(state["basis"]["commit"], *expected);
        assert_eq!(state["items"][0]["record"]["purpose"], purpose);
    }
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
    f.seed_note("retained: {\"version\":2}");
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
    let file = f.records.join("payload");
    std::fs::write(&file, "old").unwrap();
    common::git(&f.records, &["add", "payload"]);
    common::git(&f.records, &["commit", "-qm", "old payload"]);
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    let old = f.state();
    let identity = old["items"][0]["identity"].clone();
    let prior = json!({"type":"git","basis":old["basis"],"path":format!("{}/payload",old["basis"]["records_subdir"].as_str().unwrap()),"sha256":old["items"][0]["record"]["sha256"]});
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
    let request = checkpoint_request(&f, 1, json!([declaration(&f.records, "scratch")]));
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
    for layout in ["detached", "linked"] {
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

#[test]
fn retained_checkpoint_foreign_claim_hints_and_identity_precedence() {
    let f = RetainedEnv::new("detached");
    std::fs::write(f.dir.join(".gitignore"), ".worktrees/\nrequest.json\n").unwrap();
    common::git(&f.dir, &["add", ".gitignore"]);
    common::git(&f.dir, &["commit", "-qm", "ignore worktrees"]);
    let side = f.dir.join(".worktrees/side");
    common::git(
        &f.dir,
        &["worktree", "add", "-qb", "side", side.to_str().unwrap()],
    );
    common::git_fixture::assert_worktree(f.env.home.path(), &f.dir, &side);

    // 1. Same-worktree foreign Live claim: continuation hint, no release hint, state unchanged.
    f.env
        .cmd(&f.dir)
        .env("TASKS_SESSION", "holder")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", &f.id])
        .assert()
        .success();
    let request = checkpoint_request(&f, 1, json!([]));
    let req_path = f.request("request.json", &request);
    let basis_before = f.basis();
    let claims_before = std::fs::read_to_string(f.env.claim_store("sci")).unwrap();

    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(common::err_kind(&out), "claimed");
    let detail = common::err_detail(&out);
    assert!(
        detail.contains(&format!(
            "tasks start --force {} --reason \"continued from session holder\"",
            f.id
        )),
        "{detail}"
    );
    assert!(detail.contains("before retrying"), "{detail}");
    assert!(!detail.contains("tasks release"), "{detail}");
    assert_eq!(f.basis(), basis_before);
    assert_eq!(
        std::fs::read_to_string(f.env.claim_store("sci")).unwrap(),
        claims_before
    );

    // 2. Same-worktree foreign Unverifiable claim: both continuation and release hints.
    let store_path = f.env.claim_store("sci");
    let mut store: toml::Value =
        toml::from_str(&std::fs::read_to_string(&store_path).unwrap()).unwrap();
    store["claims"][&f.id]
        .as_table_mut()
        .unwrap()
        .insert("pid_view".into(), toml::Value::String("0:1".into()));
    std::fs::write(&store_path, toml::to_string(&store).unwrap()).unwrap();

    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "claimed");
    let detail = common::err_detail(&out);
    assert!(
        detail.contains("--reason \"continued from session holder\"` before retrying"),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("tasks release {} \"<why>\"", f.id)),
        "{detail}"
    );
    assert_eq!(f.basis(), basis_before);

    // 3. Identity-error precedence: when relay identity fails without proof, surface the
    //    identity error rather than a `claimed` refusal that could suggest `tasks release`.
    let host_cfg = f.env.home.path().join(".config/tasks/config.toml");
    std::fs::create_dir_all(host_cfg.parent().unwrap()).unwrap();
    std::fs::write(&host_cfg, "[identity]\nrelay = true\n").unwrap();
    let out = common::harness_shim(
        &f.dir,
        f.env.home.path(),
        "codex",
        &format!(
            "\"$TASKS_BIN\" retained checkpoint {} --request '{}'",
            f.id,
            req_path.display()
        ),
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(common::err_kind(&out), "config", "{out:?}");
    let id_detail = common::err_detail(&out);
    assert!(
        id_detail.contains("inspect tasks show for the existing claim's exact session identity"),
        "{id_detail}"
    );
    assert!(!id_detail.contains("tasks release"), "{id_detail}");
    std::fs::remove_file(&host_cfg).unwrap();

    // 4. Cross-worktree foreign Live vs Unverifiable claim: omits `continued from`, includes
    //    `tasks release` only when Unverifiable.
    f.env
        .cmd(&side)
        .env("TASKS_SESSION", "side-holder")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", "--force", &f.id, "--reason", "move to side"])
        .assert()
        .success();
    let request = checkpoint_request(&f, 2, json!([]));
    let req_path = f.request("request.json", &request);
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "claimed");
    let detail = common::err_detail(&out);
    assert!(!detail.contains("continued from"), "{detail}");
    assert!(!detail.contains("tasks release"), "{detail}");

    let mut store: toml::Value =
        toml::from_str(&std::fs::read_to_string(&store_path).unwrap()).unwrap();
    store["claims"][&f.id]
        .as_table_mut()
        .unwrap()
        .insert("pid_view".into(), toml::Value::String("0:1".into()));
    std::fs::write(&store_path, toml::to_string(&store).unwrap()).unwrap();
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "claimed");
    let detail = common::err_detail(&out);
    assert!(!detail.contains("continued from"), "{detail}");
    assert!(
        detail.contains(&format!("tasks release {} \"<why>\"", f.id)),
        "{detail}"
    );

    // 5. Explicit takeover followed by retry of `retained checkpoint` succeeds.
    f.json(&[
        "start",
        "--force",
        &f.id,
        "--reason",
        "continued from session side-holder",
    ]);
    let request = checkpoint_request(&f, 3, json!([]));
    assert_eq!(checkpoint(&f, &request)["outcome"], "no_local_items");
}

#[test]
fn retained_state_working_input_scope() {
    let f = RetainedEnv::new("in-tree");
    f.note(f.record());
    let payload = f.dir.join("payload");
    std::fs::write(&payload, "original").unwrap();
    f.json(&[
        "attach",
        &f.id,
        payload.to_str().unwrap(),
        "--name",
        "evidence",
    ]);
    let before = f.state();
    std::fs::write(
        f.records.join(format!("tasks/files/{}/evidence", f.id)),
        "dirty",
    )
    .unwrap();
    assert_eq!(
        f.state(),
        before,
        "ordinary attachments remain pinned history"
    );
    let path = f.records.join(format!("tasks/{}.md", f.id));
    let bytes = std::fs::read(&path).unwrap();
    let mut changed = bytes.clone();
    changed.push(b'\n');
    std::fs::write(&path, changed).unwrap();
    common::git(&f.records, &["add", path.to_str().unwrap()]);
    std::fs::write(&path, bytes).unwrap();
    assert!(
        !f.cmd()
            .args(["retained", "state", &f.id])
            .output()
            .unwrap()
            .status
            .success(),
        "staged changes must refuse even with original working bytes"
    );
    assert_eq!(
        f.json(&[
            "retained",
            "state",
            &f.id,
            "--at",
            before["basis"]["commit"].as_str().unwrap()
        ]),
        before
    );
}

#[test]
fn retained_admission_ancestor() {
    let f = checkpoint_fixture();
    let request = checkpoint_request(&f, 91, json!([]));
    std::fs::write(f.records.join("unrelated"), "unrelated").unwrap();
    common::git(&f.records, &["add", "unrelated"]);
    common::git(&f.records, &["commit", "-qm", "unrelated records"]);
    let result = checkpoint(&f, &request);
    assert_eq!(result["outcome"], "no_local_items");
    let state = f.state();
    assert_eq!(
        state["operations"][0]["note"]["input_bases"],
        request["inputs"]
    );
}

#[test]
fn retained_admission_changed_inputs() {
    use std::os::unix::fs::PermissionsExt;
    for case in [
        "record",
        "note",
        "ABA",
        "content",
        "mode",
        "extra",
        "ignored",
        "symlink",
        "empty_directory",
        "hidden_mode",
    ] {
        let f = checkpoint_fixture();
        let file = f.dir.join("payload");
        std::fs::write(&file, "original").unwrap();
        f.json(&[
            "attach",
            &f.id,
            file.to_str().unwrap(),
            "--name",
            "evidence",
        ]);
        let request = checkpoint_request(&f, 92, json!([]));
        let files = f.records.join(format!("tasks/files/{}", f.id));
        match case {
            "note" => {
                f.json(&["note", &f.id, "Progress after request"]);
            }
            "record" | "ABA" => {
                let path = f.records.join(format!("tasks/{}.md", f.id));
                let old = std::fs::read(&path).unwrap();
                let mut new = old.clone();
                new.push(b'\n');
                std::fs::write(&path, new).unwrap();
                common::git(&f.records, &["add", "tasks"]);
                common::git(&f.records, &["commit", "-qm", "participant edit"]);
                if case == "ABA" {
                    std::fs::write(&path, old).unwrap();
                    common::git(&f.records, &["add", "tasks"]);
                    common::git(&f.records, &["commit", "-qm", "participant revert"]);
                }
            }
            "mode" => std::fs::set_permissions(
                files.join("evidence"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap(),
            "content" => std::fs::write(files.join("evidence"), "changed").unwrap(),
            "extra" => std::fs::write(files.join("extra"), "extra").unwrap(),
            "empty_directory" => std::fs::create_dir(files.join("empty")).unwrap(),
            "hidden_mode" => {
                common::git(
                    &f.records,
                    &[
                        "update-index",
                        "--assume-unchanged",
                        files.join("evidence").to_str().unwrap(),
                    ],
                );
                std::fs::set_permissions(
                    files.join("evidence"),
                    std::fs::Permissions::from_mode(0o755),
                )
                .unwrap();
            }
            "ignored" => {
                let gitdir = common::git_fixture::command(f.env.home.path(), &f.records)
                    .args(["rev-parse", "--absolute-git-dir"])
                    .output()
                    .unwrap();
                let gitdir =
                    std::path::PathBuf::from(String::from_utf8(gitdir.stdout).unwrap().trim());
                std::fs::write(gitdir.join("info/exclude"), "ignored\n").unwrap();
                std::fs::write(files.join("ignored"), "extra").unwrap();
            }
            "symlink" => {
                std::fs::remove_file(files.join("evidence")).unwrap();
                std::os::unix::fs::symlink(&file, files.join("evidence")).unwrap();
            }
            _ => unreachable!(),
        }
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
        assert!(!out.status.success(), "{case} admitted: {out:?}");
        let shown = f.json(&["show", &f.id]);
        assert!(
            !shown["task"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("retention operation:")),
            "{case}: intent written"
        );
        assert!(
            !files
                .join(format!(
                    "retained-op-{}.request.json",
                    request["operation_id"].as_str().unwrap()
                ))
                .exists()
        );
    }
}

#[test]
fn retained_admission_in_tree() {
    for worktree in [false, true] {
        let mut f = RetainedEnv::new("in-tree");
        if worktree {
            let wt = f.dir.parent().unwrap().join("worktree");
            common::git(
                &f.dir,
                &["worktree", "add", "-qb", "feature", wt.to_str().unwrap()],
            );
            common::git_fixture::assert_worktree(f.env.home.path(), &f.dir, &wt);
            f.dir = wt;
            f.records = f.dir.clone();
        }
        f.json(&["start", &f.id]);
        let before = f.basis();
        let request = checkpoint_request(&f, 93, json!([]));
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
        assert!(!out.status.success(), "in-tree admitted: {out:?}");
        let error = String::from_utf8(out.stderr).unwrap();
        assert!(
            error.contains("detached") && error.contains("manual") && error.contains("migration"),
            "{error}"
        );
        assert_eq!(f.basis(), before, "no intent or artifact publication");
    }
}

#[test]
fn retained_admission_transfer_checks_both_participants() {
    for sender_in_tree in [false, true] {
        let f = RetainedEnv::new(if sender_in_tree {
            "in-tree"
        } else {
            "detached"
        });
        f.json(&["start", &f.id]);
        let payload = f.dir.join("payload");
        std::fs::write(&payload, "payload").unwrap();
        use sha2::{Digest, Sha256};
        let mut record = f.record();
        record["host"] = json!(host());
        record["path"] = json!(payload);
        record["sha256"] = json!(format!("{:x}", Sha256::digest(b"payload")));
        f.note(record);
        let (_root, to) = common::retained::cross_receiver_layout(&f, false, !sender_in_tree);
        let request = transfer_request(&f, &to);
        let path = f.request("transfer.json", &request);
        let before = f.basis();
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
        assert!(
            !out.status.success(),
            "sender_in_tree={sender_in_tree}: {out:?}"
        );
        assert!(String::from_utf8(out.stderr).unwrap().contains("in-tree"));
        assert_eq!(f.basis(), before);
        for id in [&f.id, &to] {
            let shown = f.json(&["show", id]);
            assert!(
                !shown["task"]["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n["text"]
                        .as_str()
                        .unwrap()
                        .starts_with("retention operation:")),
                "{id}: intent written"
            );
        }
    }
}

#[test]
fn retained_admission_staged_in_tree_intent() {
    let f = RetainedEnv::new("in-tree");
    let seeded = common::retained::seed_historical_operation(
        &f,
        common::retained::HistoricalScenario::CheckpointPending,
    );
    let input = seeded.request["inputs"][0]["basis"]["commit"]
        .as_str()
        .unwrap();
    common::git(&f.records, &["reset", "--mixed", input]);
    let path = f.request("request.json", &seeded.request);
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
    assert!(
        String::from_utf8(out.stderr).unwrap().contains("in-tree"),
        "staged phase cannot bypass location guard"
    );
    assert_eq!(f.basis(), input);
}

#[test]
fn retained_state_shared_reads() {
    let f = checkpoint_fixture();
    let request = checkpoint_request(&f, 987, json!([]));
    checkpoint(&f, &request);
    let trace = f.env.home.path().join("state-git-trace.jsonl");
    let out = f
        .cmd()
        .env("GIT_TRACE2_EVENT", &trace)
        .args(["retained", "state", &f.id])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let state: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(state["complete"], true);
    assert!(state.get("metrics").is_none());
    let mut blobs = std::collections::BTreeSet::new();
    for line in std::fs::read_to_string(trace).unwrap().lines() {
        let event: serde_json::Value = serde_json::from_str(line).unwrap();
        if event["event"] != "start" {
            continue;
        }
        let argv = event["argv"].as_array().unwrap();
        if let Some(index) = argv.iter().position(|a| a == "cat-file")
            && argv.get(index + 1).is_some_and(|a| a == "blob")
        {
            assert!(
                blobs.insert(argv[index + 2].as_str().unwrap().to_string()),
                "state reread an immutable blob: {argv:?}"
            );
        }
    }
    assert!(!blobs.is_empty());
    assert_eq!(state, f.state());
}

#[test]
fn note_and_edit_reject_malformed_unversioned_and_prose_retained_notes() {
    let f = RetainedEnv::new("in-tree");
    let task_file = f.records.join(format!("tasks/{}.md", f.id));
    let initial_notes = f.json(&["show", &f.id])["task"]["notes"].clone();
    let parse_retained_input = |out: &std::process::Output| -> String {
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "stdout must be empty on error");
        let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(err["error"]["kind"], "retained_input", "{err}");
        err["error"]["detail"].as_str().unwrap().to_string()
    };

    // 1. Prose under reserved 'retained:' prefix via `tasks note`
    let out = f
        .cmd()
        .args(["note", &f.id, "retained: created scratch dir"])
        .output()
        .unwrap();
    let detail = parse_retained_input(&out);
    assert!(detail.contains("'retained:'"), "got: {detail}");
    assert!(detail.contains("'Custody note: ...'"), "got: {detail}");

    // 2. Unversioned otherwise-valid JSON record via `tasks note`
    let unversioned = format!(
        r#"retained: {{"host":"h","kind":"worktree","path":".worktrees/x","state":"kept","verification":"verified","purpose":"p","next_owner":"task:{}","cleanup_trigger":"c"}}"#,
        f.id
    );
    let out = f
        .cmd()
        .args(["note", &f.id, &unversioned])
        .output()
        .unwrap();
    let detail = parse_retained_input(&out);
    assert!(
        detail.contains(r#"newly written retained: records require integer "version": 1"#),
        "got: {detail}"
    );

    // 3. Invalid v1 semantics (`state: "released"` with `next_owner: "task:<id>"`)
    let bad_released = format!(
        r#"retained: {{"version":1,"host":"h","kind":"worktree","path":".worktrees/x","state":"released","verification":"verified","purpose":"p","next_owner":"task:{}","cleanup_trigger":"none"}}"#,
        f.id
    );
    let out = f
        .cmd()
        .args(["note", &f.id, &bad_released])
        .output()
        .unwrap();
    let _ = parse_retained_input(&out);

    // 4. `tasks note <id> --stdin` fails atomically with 1-based line number and zero notes written
    let out = f
        .cmd()
        .args(["note", &f.id, "--stdin"])
        .write_stdin("Custody note: ok\nretained: bad prose\n")
        .output()
        .unwrap();
    let detail = parse_retained_input(&out);
    assert!(detail.contains("stdin line 2:"), "got: {detail}");
    assert!(detail.contains("'retained:'"), "got: {detail}");
    assert!(detail.contains("'Custody note: ...'"), "got: {detail}");
    assert_eq!(
        f.json(&["show", &f.id])["task"]["notes"],
        initial_notes,
        "failed --stdin batch must write zero notes"
    );

    // 5. Lifecycle note writers (`block`) also fail with `retained_input`, while `edit --editor`
    //    cannot append notes and CAN trim trailing spaces on historical malformed `retained:` notes.
    let before_raw = std::fs::read_to_string(&task_file).unwrap();
    let out = f
        .cmd()
        .args(["block", &f.id, "retained: blocked prose"])
        .output()
        .unwrap();
    let detail = parse_retained_input(&out);
    assert!(detail.contains("'retained:'"), "got: {detail}");
    assert!(detail.contains("'Custody note: ...'"), "got: {detail}");
    assert_eq!(std::fs::read_to_string(&task_file).unwrap(), before_raw);

    let raw_with_historical =
        format!("{before_raw}- 2026-11-03T00:00:00Z (fixture): retained: historical bad prose \n");
    std::fs::write(&task_file, &raw_with_historical).unwrap();
    let trim_editor = common::editor_script(
        f.env.home.path(),
        r#"sed -i 's/historical bad prose $/historical bad prose/' "$1""#,
    );
    let out = f
        .cmd()
        .env("EDITOR", &trim_editor)
        .args(["edit", &f.id, "--editor"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "trimming historical malformed note via edit --editor must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn closed_done_task_repairs_legacy_files_manifest_via_retention_resolution() {
    use sha2::{Digest, Sha256};

    let f = RetainedEnv::new("detached");
    let manifest_src = f.dir.join("manifest-src.json");
    let manifest_bytes = serde_json::to_vec(&json!({
        "version": 1,
        "entries": [{"type": "file", "path": "entry", "sha256": "b".repeat(64)}]
    }))
    .unwrap();
    std::fs::write(&manifest_src, &manifest_bytes).unwrap();
    let manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));

    f.json(&[
        "attach",
        &f.id,
        manifest_src.to_str().unwrap(),
        "--name",
        "manifest.json",
    ]);
    f.json(&["start", &f.id]);
    f.json(&["done", &f.id, "Completed task"]);
    assert_eq!(f.json(&["show", &f.id])["task"]["status"], "done");

    let mut bad = f.record();
    bad["kind"] = json!("scratch");
    bad["path"] = json!("$WORK_ROOT/sci/scratch-dir");
    bad["sha256"] = json!(manifest_sha);
    bad["manifest"] = json!("files:manifest.json");
    bad["next_owner"] = json!("user");
    bad["handoff"] = json!("Retained scratch artifacts for review");
    f.note(bad.clone());

    let before = f.state();
    assert_eq!(before["complete"], false);
    assert_eq!(before["diagnostics"].as_array().unwrap().len(), 1);
    let detail = before["diagnostics"][0]["detail"].as_str().unwrap();
    assert!(
        detail.contains("manifest locator must start with / or $WORK_ROOT/"),
        "got: {detail}"
    );
    assert_eq!(
        before["diagnostics"][0]["action"],
        "Recover exact history, append valid version: 1 'retained:' successor note(s) for each affected identity (or none when no identities exist; use <records>/tasks/files/<id>/<name> for an existing attached manifest, including on closed tasks without reopening), then append 'retention resolution: {\"version\":1,\"bad\":<source>,\"successors\":[...],\"reason\":\"...\"}' or pass resolutions to tasks retained checkpoint"
    );

    // Repair on closed `done` task via `tasks note` without `tasks start` or reopening.
    let canonical_manifest = f
        .records
        .join(format!("tasks/files/{}/manifest.json", f.id));
    let mut fixed = bad;
    fixed["manifest"] = json!(canonical_manifest.to_str().unwrap());
    f.json(&["note", &f.id, &format!("retained: {fixed}")]);
    let succ_ordinal = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();

    let resolution = json!({
        "version": 1,
        "bad": before["diagnostics"][0]["source"],
        "successors": [succ_ordinal],
        "reason": "Migrated legacy files: manifest locator to canonical attached path"
    });
    f.json(&[
        "note",
        &f.id,
        &format!("retention resolution: {resolution}"),
    ]);

    assert_eq!(f.json(&["show", &f.id])["task"]["status"], "done");
    let after = f.state();
    assert_eq!(after["complete"], true, "{after}");
    assert_eq!(
        after["diagnostics"][0]["resolved_by"]["ordinal"],
        succ_ordinal + 1
    );
    assert_eq!(
        after["items"][0]["record"]["manifest"],
        canonical_manifest.to_str().unwrap()
    );
}

#[test]
fn resolution_covers_released_other_host_and_zero_item_prose_across_host_filter() {
    let f = RetainedEnv::new("detached");
    let local_host = host();

    // 1. Historical malformed `released` note (`next_owner != "none"`) repaired via `tasks note`.
    let bad_released = json!({
        "version": 1,
        "host": local_host,
        "kind": "evidence",
        "path": "/data/released-item",
        "state": "released",
        "verification": "verified",
        "purpose": "Released evidence",
        "next_owner": format!("task:{}", f.id),
        "cleanup_trigger": "none"
    });
    f.note(bad_released);
    let s1 = f.state();
    assert_eq!(s1["complete"], false);
    let valid_released = json!({
        "version": 1,
        "host": local_host,
        "kind": "evidence",
        "path": "/data/released-item",
        "state": "released",
        "verification": "verified",
        "purpose": "Released evidence",
        "next_owner": "none",
        "cleanup_trigger": "none"
    });
    f.json(&["note", &f.id, &format!("retained: {valid_released}")]);
    let rel_succ = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    let rel_res = json!({
        "version": 1,
        "bad": s1["diagnostics"][0]["source"],
        "successors": [rel_succ],
        "reason": "Corrected released record next_owner to none"
    });
    f.json(&["note", &f.id, &format!("retention resolution: {rel_res}")]);
    assert_eq!(f.state()["complete"], true);

    // 2. Historical malformed note on `"other-host"` repaired by valid `"other-host"` successor
    //    + `retention resolution:`; both unfiltered and `--host <local>` report `complete: true`.
    f.seed_note(r#"retained: {"host":"other-host","path":42}"#);
    let s2 = f.state();
    assert_eq!(s2["complete"], false);
    let bad_other_source = s2["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["resolved_by"].is_null())
        .unwrap()["source"]
        .clone();
    let mut valid_other = f.record();
    valid_other["host"] = json!("other-host");
    valid_other["path"] = json!("/data/other-host-item");
    f.json(&["note", &f.id, &format!("retained: {valid_other}")]);
    let other_succ = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    let other_res = json!({
        "version": 1,
        "bad": bad_other_source,
        "successors": [other_succ],
        "reason": "Recovered other-host inventory"
    });
    f.json(&["note", &f.id, &format!("retention resolution: {other_res}")]);
    let unfiltered = f.state();
    assert_eq!(unfiltered["complete"], true, "{unfiltered}");
    let filtered = f.json(&["retained", "state", &f.id, "--host", &local_host]);
    assert_eq!(filtered["complete"], true, "{filtered}");
    assert!(
        filtered["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| !d["resolved_by"].is_null()),
        "no false unmatched-resolution diagnostic under --host: {filtered}"
    );

    // 3a. 0-item accidental `"retained: prose"` repaired via `"successors": []` in `retention resolution:`.
    let prose_note_id = f.json(&["add", "Zero-item prose note repair"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let prose_note_path = f.records.join(format!("tasks/{prose_note_id}.md"));
    let mut raw = std::fs::read_to_string(&prose_note_path).unwrap();
    if !raw.contains("\n## Notes\n") {
        raw.push_str("\n## Notes\n\n");
    }
    raw.push_str("- 2026-11-03T00:00:00Z (fixture): retained: prose\n");
    std::fs::write(&prose_note_path, raw).unwrap();
    common::git(&f.records, &["add", prose_note_path.to_str().unwrap()]);
    common::git(&f.records, &["commit", "-qm", "seed 0-item prose note"]);

    let s_prose = f.json(&["retained", "state", &prose_note_id]);
    assert_eq!(s_prose["complete"], false);
    let prose_res = json!({
        "version": 1,
        "bad": s_prose["diagnostics"][0]["source"],
        "successors": [],
        "reason": "Accidental prose on zero-item task"
    });
    f.json(&[
        "note",
        &prose_note_id,
        &format!("retention resolution: {prose_res}"),
    ]);
    let s_prose_after = f.json(&["retained", "state", &prose_note_id]);
    assert_eq!(s_prose_after["complete"], true, "{s_prose_after}");
    assert!(s_prose_after["items"].as_array().unwrap().is_empty());
    assert!(!s_prose_after["diagnostics"][0]["resolved_by"].is_null());

    // 3b. Sibling 0-item task repaired via 0-item `CheckpointRequest.resolutions` (`declarations: []`).
    let cp_id = f.json(&["add", "Zero-item checkpoint resolution"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &cp_id]);
    let cp_path = f.records.join(format!("tasks/{cp_id}.md"));
    let mut raw_cp = std::fs::read_to_string(&cp_path).unwrap();
    raw_cp.push_str("- 2026-11-03T00:00:05Z (fixture): retained: prose\n");
    std::fs::write(&cp_path, raw_cp).unwrap();
    common::git(&f.records, &["add", cp_path.to_str().unwrap()]);
    common::git(
        &f.records,
        &["commit", "-qm", "seed 0-item checkpoint prose"],
    );

    let s_cp_before = f.json(&["retained", "state", &cp_id]);
    assert_eq!(s_cp_before["complete"], false);
    let cp_req = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 77u64),
        "host": local_host,
        "inputs": [{"task": cp_id, "basis": s_cp_before["basis"]}],
        "declarations": [],
        "resolutions": [{
            "bad": s_cp_before["diagnostics"][0]["source"],
            "declarations": [],
            "reason": "Zero local items exist; accidental prose cleared"
        }]
    });
    let req_file = f.request("zero-item-res.json", &cp_req);
    let cp_out = f.json(&[
        "retained",
        "checkpoint",
        &cp_id,
        "--request",
        req_file.to_str().unwrap(),
    ]);
    assert_eq!(cp_out["outcome"], "no_local_items", "{cp_out}");
    assert_eq!(cp_out["counts"]["kept_verified"], 0);
    assert_eq!(cp_out["counts"]["kept_unknown"], 0);
    assert_eq!(cp_out["counts"]["released"], 0);

    let s_cp_after = f.json(&["retained", "state", &cp_id]);
    assert_eq!(s_cp_after["complete"], true, "{s_cp_after}");
    assert!(!s_cp_after["diagnostics"][0]["resolved_by"].is_null());
    assert!(s_cp_after["checkpoints"].as_array().unwrap().is_empty());
    let ops = s_cp_after["operations"].as_array().unwrap();
    let phases: Vec<&str> = ops
        .iter()
        .map(|o| o["note"]["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases, vec!["intent", "items_published", "completed"]);
    assert_eq!(
        ops[1]["note"]["outputs"]["resolutions"][0]["successors"],
        json!([])
    );
    assert_eq!(ops[2]["note"]["outputs"]["outcome"], "no_local_items");
    assert!(ops[2]["note"]["outputs"]["checkpoint"].is_null());
}

#[test]
fn resolution_rejects_forged_basis_operation_envelope_duplicate_identities_and_self_ordinal() {
    let f = RetainedEnv::new("detached");
    let assert_rejected_write = |out: &std::process::Output| {
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "stdout must be empty on error");
        let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        let kind = err["error"]["kind"].as_str().unwrap();
        assert!(
            matches!(kind, "retained_input" | "retained_history_blocked"),
            "unexpected error kind: {err}"
        );
    };

    // 1. Targeting a `retention operation:` ordinal (or malformed `retention checkpoint:` ordinal)
    //    is rejected before saving even when no operation is pending.
    common::retained::seed_historical_operation(
        &f,
        common::retained::HistoricalScenario::CheckpointCompleted,
    );
    let seeded_state = f.state();
    let op_ord = seeded_state["operations"][0]["source"]["ordinal"]
        .as_u64()
        .unwrap();
    let op_basis = seeded_state["basis"].clone();
    f.seed_note(r#"retention checkpoint: {"version":2}"#);
    let s_cp = f.state();
    assert_eq!(s_cp["complete"], false);
    let cp_bad_source = s_cp["diagnostics"][0]["source"].clone();
    let task_file = f.records.join(format!("tasks/{}.md", f.id));
    let raw_before = std::fs::read_to_string(&task_file).unwrap();

    for bad_target in [json!({"basis": op_basis, "ordinal": op_ord}), cp_bad_source] {
        let res_targeting_envelope = json!({
            "version": 1,
            "bad": bad_target,
            "successors": [],
            "reason": "Try to target operation or checkpoint envelope"
        });
        let out = f
            .cmd()
            .args([
                "note",
                &f.id,
                &format!("retention resolution: {res_targeting_envelope}"),
            ])
            .output()
            .unwrap();
        assert_rejected_write(&out);
        assert_eq!(std::fs::read_to_string(&task_file).unwrap(), raw_before);
    }

    // 2. Self-ordinal (`bad.ordinal == res_ordinal`) and not-yet-persisted ordinal (`> old.notes.len()`).
    let current_len = f.json(&["show", &f.id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    for bad_ord in [current_len + 1, current_len + 2] {
        let self_or_future = json!({
            "version": 1,
            "bad": {"basis": s_cp["basis"], "ordinal": bad_ord},
            "successors": [],
            "reason": "Self or future ordinal"
        });
        let out = f
            .cmd()
            .args([
                "note",
                &f.id,
                &format!("retention resolution: {self_or_future}"),
            ])
            .output()
            .unwrap();
        assert_rejected_write(&out);
        assert_eq!(std::fs::read_to_string(&task_file).unwrap(), raw_before);
    }

    // Not-yet-persisted ordinal via `--stdin` batch (line 1 adds ordinal `current_len + 1`,
    // line 2 tries to resolve `current_len + 1` in the same uncommitted save).
    let unpersisted_batch_res = json!({
        "version": 1,
        "bad": {"basis": s_cp["basis"], "ordinal": current_len + 1},
        "successors": [],
        "reason": "Unpersisted note in same batch"
    });
    let out = f
        .cmd()
        .args(["note", &f.id, "--stdin"])
        .write_stdin(format!(
            "Custody note: ok\nretention resolution: {unpersisted_batch_res}\n"
        ))
        .output()
        .unwrap();
    assert_rejected_write(&out);
    assert_eq!(std::fs::read_to_string(&task_file).unwrap(), raw_before);

    // 3. Forged `bad.basis.commit` and duplicate successor identities `[4, 5]` on a 2-identity host diagnostic.
    let dup_id = f.json(&["add", "Duplicate identity and forged basis checks"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dup_file = f.records.join(format!("tasks/{dup_id}.md"));
    // Start with 0 notes on `dup_id` so ordinals are literally 1..=5.
    let initial_raw = std::fs::read_to_string(&dup_file).unwrap();
    let header_only = format!(
        "{}\n## Notes\n\n",
        initial_raw.split("\n## Notes").next().unwrap()
    );
    std::fs::write(&dup_file, header_only).unwrap();
    common::git(&f.records, &["add", dup_file.to_str().unwrap()]);
    common::git(&f.records, &["commit", "-qm", "empty notes on dup task"]);
    let dup_early_basis = f.json(&["retained", "state", &dup_id])["basis"].clone();

    let mut rec_a = f.record();
    rec_a["host"] = json!("h");
    rec_a["path"] = json!("/data/a");
    rec_a["next_owner"] = json!(format!("task:{dup_id}"));
    let mut rec_b = rec_a.clone();
    rec_b["path"] = json!("/data/b");

    let append_committed = |line: &str| {
        let mut raw = std::fs::read_to_string(&dup_file).unwrap();
        raw.push_str(&format!("- 2026-11-03T00:00:10Z (fixture): {line}\n"));
        std::fs::write(&dup_file, raw).unwrap();
        common::git(&f.records, &["add", dup_file.to_str().unwrap()]);
        common::git(&f.records, &["commit", "-qm", "seed dup history"]);
    };
    append_committed(&format!("retained: {rec_a}")); // ordinal 1 (item A)
    append_committed(&format!("retained: {rec_b}")); // ordinal 2 (item B)
    append_committed(r#"retained: {"host":"h","path":42}"#); // ordinal 3 (bad host-scoped)
    let bad_state = f.json(&["retained", "state", &dup_id]);
    let real_bad_source = bad_state["diagnostics"][0]["source"].clone();
    assert_eq!(real_bad_source["ordinal"], 3);
    append_committed(&format!("retained: {rec_a}")); // ordinal 4 (item A)
    append_committed(&format!("retained: {rec_a}")); // ordinal 5 (duplicate item A!)
    let dup_raw_before = std::fs::read_to_string(&dup_file).unwrap();

    // 3a. Forged unreachable commit ("0".repeat(40)) and forged earlier commit (`dup_early_basis`).
    let mut zero_basis = real_bad_source["basis"].clone();
    zero_basis["commit"] = json!("0".repeat(40));
    for forged_basis in [zero_basis, dup_early_basis] {
        let forged_res = json!({
            "version": 1,
            "bad": {"basis": forged_basis, "ordinal": 3},
            "successors": [4],
            "reason": "Forged basis commit"
        });
        let out = f
            .cmd()
            .args([
                "note",
                &dup_id,
                &format!("retention resolution: {forged_res}"),
            ])
            .output()
            .unwrap();
        assert_rejected_write(&out);
        assert_eq!(std::fs::read_to_string(&dup_file).unwrap(), dup_raw_before);
    }

    // 4. Duplicate identities across two successor ordinals `[4, 5]` on a 2-identity host diagnostic.
    let dup_succ_res = json!({
        "version": 1,
        "bad": real_bad_source,
        "successors": [4, 5],
        "reason": "Two successor ordinals naming item A twice"
    });
    let out = f
        .cmd()
        .args([
            "note",
            &dup_id,
            &format!("retention resolution: {dup_succ_res}"),
        ])
        .output()
        .unwrap();
    assert_rejected_write(&out);
    assert_eq!(std::fs::read_to_string(&dup_file).unwrap(), dup_raw_before);

    // 5. Tampering a committed task file on a branch to hold a `retention resolution:` with a
    //    mismatched `bad.basis` commit fails BOTH `tasks retained state` and `tasks retained checkpoint`
    //    with `retained_history_blocked`.
    let tamper_id = f.json(&["add", "Tampered committed resolution"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &tamper_id]);
    let tamper_early = f.json(&["retained", "state", &tamper_id])["basis"].clone();
    let tamper_notes_before = f.json(&["show", &tamper_id])["task"]["notes"]
        .as_array()
        .unwrap()
        .len();
    let tamper_file = f.records.join(format!("tasks/{tamper_id}.md"));
    let mut raw_t = std::fs::read_to_string(&tamper_file).unwrap();
    raw_t.push_str("- 2026-11-03T00:00:20Z (fixture): retained: prose\n");
    std::fs::write(&tamper_file, &raw_t).unwrap();
    common::git(&f.records, &["add", tamper_file.to_str().unwrap()]);
    common::git(&f.records, &["commit", "-qm", "tamper prose"]);

    let tampered_res = json!({
        "version": 1,
        "bad": {"basis": tamper_early, "ordinal": tamper_notes_before + 1},
        "successors": [],
        "reason": "Tampered bad.basis commit on branch"
    });
    raw_t.push_str(&format!(
        "- 2026-11-03T00:00:21Z (fixture): retention resolution: {tampered_res}\n"
    ));
    std::fs::write(&tamper_file, &raw_t).unwrap();
    common::git(&f.records, &["add", tamper_file.to_str().unwrap()]);
    common::git(
        &f.records,
        &["commit", "-qm", "tamper forged resolution commit"],
    );

    let out_state = f
        .cmd()
        .args(["retained", "state", &tamper_id])
        .output()
        .unwrap();
    assert_eq!(out_state.status.code(), Some(1));
    let err_state: serde_json::Value = serde_json::from_slice(&out_state.stderr).unwrap();
    assert_eq!(
        err_state["error"]["kind"], "retained_history_blocked",
        "{err_state}"
    );

    let mut head_basis = tamper_early;
    head_basis["commit"] = json!(f.basis());
    let cp_req = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 88u64),
        "host": host(),
        "inputs": [{"task": tamper_id, "basis": head_basis}],
        "declarations": []
    });
    let req_path = f.request("tampered-cp.json", &cp_req);
    let out_cp = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &tamper_id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_cp.status.code(), Some(1));
    let err_cp: serde_json::Value = serde_json::from_slice(&out_cp.stderr).unwrap();
    assert_eq!(
        err_cp["error"]["kind"], "retained_history_blocked",
        "{err_cp}"
    );
}

#[test]
fn checkpoint_whole_scope_resolution_must_cover_newly_declared_items() {
    let f = checkpoint_fixture();
    let payload = f.dir.join("payload");
    std::fs::write(&payload, "payload").unwrap();
    f.seed_note("retained: prose");

    let before_state = f.state();
    assert_eq!(before_state["complete"], false);
    assert_eq!(before_state["diagnostics"][0]["scope"], "task");
    let before_head = f.basis();
    let task_file = f.records.join(format!("tasks/{}.md", f.id));
    let before_raw = std::fs::read_to_string(&task_file).unwrap();

    // 1. `declarations: [item]` + `resolutions: [{ bad, declarations: [] }]` must fail in `inventory`
    //    before `publish::intent` writes any note or request attachment.
    let mut bad_req = checkpoint_request(&f, 91, json!([declaration(&payload, "evidence")]));
    bad_req["resolutions"] = json!([{
        "bad": before_state["diagnostics"][0]["source"],
        "declarations": [],
        "reason": "Omitted newly declared item from resolution"
    }]);
    let bad_path = f.request("bad-whole-scope.json", &bad_req);
    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            bad_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "retained_input", "{err}");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("resolution needs the complete blocked inventory on the executing host"),
        "got: {err}"
    );
    assert_eq!(
        f.basis(),
        before_head,
        "no commit may be written before intent"
    );
    assert_eq!(
        std::fs::read_to_string(&task_file).unwrap(),
        before_raw,
        "task file must stay untouched before intent"
    );
    assert!(
        !f.records.join(format!("tasks/files/{}", f.id)).exists(),
        "no request attachment may be written before intent"
    );

    // 2. Moving `item` into `resolutions[0].declarations` covers the full local inventory and succeeds.
    let mut good_req = checkpoint_request(&f, 92, json!([]));
    good_req["resolutions"] = json!([{
        "bad": before_state["diagnostics"][0]["source"],
        "declarations": [declaration(&payload, "evidence")],
        "reason": "Complete local inventory declared in resolution"
    }]);
    let res = checkpoint(&f, &good_req);
    assert_eq!(res["outcome"], "verified", "{res}");
    assert_eq!(res["counts"]["kept_verified"], 1);
    let after = f.state();
    assert_eq!(after["complete"], true, "{after}");
    assert!(!after["diagnostics"][0]["resolved_by"].is_null());

    // 3. Prior item A + newly declared item B in `request.declarations` with host resolution
    //    naming only `[A]` is also rejected before `publish::intent`, while `[A, B]` succeeds.
    let extra = f.dir.join("extra");
    std::fs::write(&extra, "extra").unwrap();
    f.seed_note(&format!(
        "retained: {}",
        json!({"host": host(), "path": 42})
    ));
    let s2 = f.state();
    assert_eq!(s2["complete"], false);
    let bad_host_source = s2["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["resolved_by"].is_null())
        .unwrap()["source"]
        .clone();
    let head2 = f.basis();
    let raw2 = std::fs::read_to_string(&task_file).unwrap();

    let mut partial_req = checkpoint_request(&f, 93, json!([declaration(&extra, "evidence")]));
    partial_req["resolutions"] = json!([{
        "bad": bad_host_source,
        "declarations": [declaration(&payload, "evidence")],
        "reason": "Omitted newly declared extra item"
    }]);
    let partial_path = f.request("partial-host.json", &partial_req);
    let out2 = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            partial_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(1), "{out2:?}");
    let err2: serde_json::Value = serde_json::from_slice(&out2.stderr).unwrap();
    assert_eq!(err2["error"]["kind"], "retained_input", "{err2}");
    assert_eq!(f.basis(), head2);
    assert_eq!(std::fs::read_to_string(&task_file).unwrap(), raw2);

    let mut full_req = checkpoint_request(&f, 94, json!([]));
    full_req["resolutions"] = json!([{
        "bad": bad_host_source,
        "declarations": [
            declaration(&payload, "evidence"),
            declaration(&extra, "evidence")
        ],
        "reason": "Both prior and newly declared items included"
    }]);
    assert_eq!(checkpoint(&f, &full_req)["outcome"], "verified");
    assert_eq!(f.state()["complete"], true);
}

fn configure_attachments(records: &std::path::Path, attachments_toml: &str) {
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

fn build_approx_5k_container(root: &std::path::Path) -> (Vec<u8>, String) {
    use sha2::{Digest, Sha256};
    std::fs::create_dir_all(root).unwrap();
    let mut entries = Vec::new();
    for i in 0..42 {
        let rel = format!("member-{i:03}-payload.txt");
        let content = format!("payload-{i:03}");
        std::fs::write(root.join(&rel), content.as_bytes()).unwrap();
        let file_sha = format!("{:x}", Sha256::digest(content.as_bytes()));
        entries.push(json!({"type": "file", "path": rel, "sha256": file_sha}));
    }
    let manifest_bytes = serde_json::to_vec(&json!({
        "version": 1,
        "entries": entries,
    }))
    .unwrap();
    assert!(
        (4800..6000).contains(&manifest_bytes.len()),
        "unexpected manifest size: {}",
        manifest_bytes.len()
    );
    let manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));
    (manifest_bytes, manifest_sha)
}

#[test]
fn pre_intent_admission_refuses_undeclared_over_max_and_oversized_catalog_without_notes() {
    let f = checkpoint_fixture();
    let (receiver_records, to) = cross_receiver(&f, false);
    let container = f.dir.join("container-5k");
    let (manifest_bytes, manifest_sha) = build_approx_5k_container(&container);
    let off_container_manifest = f.dir.join("manual-off-container-manifest.json");
    std::fs::write(&off_container_manifest, &manifest_bytes).unwrap();

    // Give sender task `s_id` a valid kept scratch item backed by the off-container manifest
    // while sender (`sci`) has permissive multipart settings during transfer tests, so transfer
    // specifically tests admission against receiver project `to` (`fam`) before `publish::intent`.
    let s_id = f.json(&["add", "Sender with container"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &s_id]);
    let sender_item = json!({
        "version": 1,
        "host": host(),
        "kind": "scratch",
        "path": container.to_str().unwrap(),
        "state": "kept",
        "verification": "verified",
        "purpose": "Review container",
        "next_owner": format!("task:{s_id}"),
        "cleanup_trigger": "Review completes",
        "sha256": manifest_sha,
        "manifest": off_container_manifest.to_str().unwrap(),
    });
    f.json(&["note", &s_id, &format!("retained: {sender_item}")]);

    let assert_zero_side_effects = |task_ids: &[(&std::path::Path, &str)], before_head: &str| {
        assert_eq!(
            f.basis(),
            before_head,
            "HEAD must stay untouched before intent"
        );
        for (rec, tid) in task_ids {
            let raw = std::fs::read_to_string(rec.join(format!("tasks/{tid}.md"))).unwrap();
            assert!(
                !raw.contains("retention operation:"),
                "task {tid} must have zero retention operation notes:\n{raw}"
            );
            assert!(
                !rec.join(format!("tasks/files/{tid}")).exists(),
                "tasks/files/{tid} must not exist"
            );
        }
    };

    // 1. `attachments.max_bytes = 2048` with `multipart_validators` undeclared -> `MultipartValidatorsUndeclared`.
    configure_attachments(&f.records, "max_bytes = 2048\n");
    configure_attachments(&receiver_records, "max_bytes = 2048\n");
    let head1 = f.basis();
    let cp_req1 = checkpoint_request(&f, 101, json!([declaration(&container, "scratch")]));
    let cp_path1 = f.request("cp-undeclared.json", &cp_req1);
    let out_cp1 = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            cp_path1.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_cp1.status.code(), Some(1), "{out_cp1:?}");
    let err_cp1: serde_json::Value = serde_json::from_slice(&out_cp1.stderr).unwrap();
    assert_eq!(
        err_cp1["error"]["kind"], "multipart_validators_undeclared",
        "{err_cp1}"
    );
    let detail_cp1 = err_cp1["error"]["detail"].as_str().unwrap();
    assert!(
        detail_cp1.contains(&format!(
            "Container {} manifest is {} bytes (exceeds 2048 byte attachment cap) and [attachments] multipart_validators is undeclared in sci/tasks/.config.toml",
            container.display(),
            manifest_bytes.len()
        )),
        "got: {detail_cp1}"
    );
    assert_zero_side_effects(&[(&f.records, &f.id)], &head1);

    // Make sender permissive (`multipart_validators = []`) while receiver (`fam`) stays undeclared,
    // proving `transfer::preflight` checks receiver `to` before `publish::intent`.
    configure_attachments(&f.records, "max_bytes = 2048\nmultipart_validators = []\n");
    let head1_tx = f.basis();
    let s_state = f.json(&["retained", "state", &s_id]);
    let to_state = f.json(&["retained", "state", &to]);
    let tx_req1 = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 201u64),
        "host": host(),
        "inputs": [
            {"task": s_id, "basis": s_state["basis"]},
            {"task": to, "basis": to_state["basis"]}
        ],
        "items": [s_state["items"][0]["identity"]],
        "purpose": "Transfer container",
        "cleanup_trigger": "After review"
    });
    let tx_path1 = f.request("tx-undeclared.json", &tx_req1);
    let out_tx1 = f
        .cmd()
        .args([
            "retained",
            "transfer",
            &s_id,
            &to,
            "--request",
            tx_path1.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_tx1.status.code(), Some(1), "{out_tx1:?}");
    let err_tx1: serde_json::Value = serde_json::from_slice(&out_tx1.stderr).unwrap();
    assert_eq!(
        err_tx1["error"]["kind"], "multipart_validators_undeclared",
        "{err_tx1}"
    );
    assert!(
        err_tx1["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("in fam/tasks/.config.toml"),
        "got: {err_tx1}"
    );
    assert_zero_side_effects(&[(&f.records, &s_id), (&receiver_records, &to)], &head1_tx);

    // 2. `multipart_validators = []` with `multipart_max_bytes = 4096` (and `multipart_max_parts = 2`) -> `AttachmentTooLarge`.
    configure_attachments(
        &f.records,
        "max_bytes = 2048\nmultipart_max_bytes = 4096\nmultipart_validators = []\n",
    );
    configure_attachments(
        &receiver_records,
        "max_bytes = 2048\nmultipart_max_bytes = 4096\nmultipart_validators = []\n",
    );
    let head2 = f.basis();
    let cp_req2 = checkpoint_request(&f, 102, json!([declaration(&container, "scratch")]));
    let cp_path2 = f.request("cp-over-max.json", &cp_req2);
    let out_cp2 = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            cp_path2.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_cp2.status.code(), Some(1), "{out_cp2:?}");
    let err_cp2: serde_json::Value = serde_json::from_slice(&out_cp2.stderr).unwrap();
    assert_eq!(
        err_cp2["error"]["kind"], "attachment_too_large",
        "{err_cp2}"
    );
    assert!(
        err_cp2["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("exceeds configured multipart_max_bytes = 4096 in sci/tasks/.config.toml"),
        "got: {err_cp2}"
    );
    assert_zero_side_effects(&[(&f.records, &f.id)], &head2);

    // Keep sender permissive (`multipart_max_bytes = 16384`) so transfer checks receiver's 4096 cap.
    configure_attachments(
        &f.records,
        "max_bytes = 2048\nmultipart_max_bytes = 16384\nmultipart_validators = []\n",
    );
    let head2_tx = f.basis();
    let s_state2 = f.json(&["retained", "state", &s_id]);
    let to_state2 = f.json(&["retained", "state", &to]);
    let tx_req2 = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 202u64),
        "host": host(),
        "inputs": [
            {"task": s_id, "basis": s_state2["basis"]},
            {"task": to, "basis": to_state2["basis"]}
        ],
        "items": [s_state2["items"][0]["identity"]],
        "purpose": "Transfer container",
        "cleanup_trigger": "After review"
    });
    let tx_path2 = f.request("tx-over-max.json", &tx_req2);
    let out_tx2 = f
        .cmd()
        .args([
            "retained",
            "transfer",
            &s_id,
            &to,
            "--request",
            tx_path2.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_tx2.status.code(), Some(1), "{out_tx2:?}");
    let err_tx2: serde_json::Value = serde_json::from_slice(&out_tx2.stderr).unwrap();
    assert_eq!(
        err_tx2["error"]["kind"], "attachment_too_large",
        "{err_tx2}"
    );
    assert!(
        err_tx2["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("exceeds configured multipart_max_bytes = 4096 in fam/tasks/.config.toml"),
        "got: {err_tx2}"
    );
    assert_zero_side_effects(&[(&f.records, &s_id), (&receiver_records, &to)], &head2_tx);

    // 2b. `multipart_max_parts = 2` on 3-part ~5 KiB manifest -> `AttachmentTooLarge`.
    configure_attachments(
        &f.records,
        "max_bytes = 2048\nmultipart_max_bytes = 16384\nmultipart_max_parts = 2\nmultipart_validators = []\n",
    );
    let head_parts = f.basis();
    let cp_req_parts = checkpoint_request(&f, 103, json!([declaration(&container, "scratch")]));
    let cp_path_parts = f.request("cp-over-parts.json", &cp_req_parts);
    let out_parts = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            cp_path_parts.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_parts.status.code(), Some(1), "{out_parts:?}");
    let err_parts: serde_json::Value = serde_json::from_slice(&out_parts.stderr).unwrap();
    assert_eq!(
        err_parts["error"]["kind"], "attachment_too_large",
        "{err_parts}"
    );
    assert!(
        err_parts["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("exceeds configured multipart_max_parts = 2 in sci/tasks/.config.toml"),
        "got: {err_parts}"
    );
    assert_zero_side_effects(&[(&f.records, &f.id)], &head_parts);

    // 3. `attachments.max_bytes = 400` and `multipart_validators = []` so `.catalog.json` exceeds `max_bytes`.
    // Run checkpoint on `s_id` (where `container` is already in `kept` inventory so the request JSON
    // itself is < 300 bytes <= 400) and transfer from `s_id` (`max_bytes = 2048`) to `to` (`max_bytes = 400`).
    configure_attachments(
        &f.records,
        "max_bytes = 400\nmultipart_max_bytes = 16384\nmultipart_max_parts = 64\nmultipart_validators = []\n",
    );
    let head3 = f.basis();
    let s_state3 = f.json(&["retained", "state", &s_id]);
    let cp_req3 = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 104u64),
        "host": host(),
        "inputs": [{"task": s_id, "basis": s_state3["basis"]}]
    });
    let cp_path3 = f.request("cp-catalog-cap.json", &cp_req3);
    assert!(
        std::fs::metadata(&cp_path3).unwrap().len() <= 400,
        "request JSON itself must fit within 400 bytes"
    );
    let out_cp3 = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &s_id,
            "--request",
            cp_path3.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_cp3.status.code(), Some(1), "{out_cp3:?}");
    let err_cp3: serde_json::Value = serde_json::from_slice(&out_cp3.stderr).unwrap();
    assert_eq!(
        err_cp3["error"]["kind"], "attachment_too_large",
        "{err_cp3}"
    );
    let detail_cp3 = err_cp3["error"]["detail"].as_str().unwrap();
    assert!(
        detail_cp3.contains(".catalog.json") && detail_cp3.contains("400"),
        "got: {detail_cp3}"
    );
    assert_zero_side_effects(&[(&f.records, &s_id)], &head3);

    configure_attachments(
        &f.records,
        "max_bytes = 2048\nmultipart_max_bytes = 16384\nmultipart_max_parts = 64\nmultipart_validators = []\n",
    );
    configure_attachments(
        &receiver_records,
        "max_bytes = 400\nmultipart_max_bytes = 16384\nmultipart_max_parts = 64\nmultipart_validators = []\n",
    );
    let head3_tx = f.basis();
    let s_state3_tx = f.json(&["retained", "state", &s_id]);
    let to_state3_tx = f.json(&["retained", "state", &to]);
    let tx_req3 = json!({
        "version": 1,
        "operation_id": format!("{:032x}", 204u64),
        "host": host(),
        "inputs": [
            {"task": s_id, "basis": s_state3_tx["basis"]},
            {"task": to, "basis": to_state3_tx["basis"]}
        ],
        "items": [s_state3_tx["items"][0]["identity"]],
        "purpose": "Transfer container",
        "cleanup_trigger": "After review"
    });
    let tx_path3 = f.request("tx-catalog-cap.json", &tx_req3);
    let out_tx3 = f
        .cmd()
        .args([
            "retained",
            "transfer",
            &s_id,
            &to,
            "--request",
            tx_path3.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_tx3.status.code(), Some(1), "{out_tx3:?}");
    let err_tx3: serde_json::Value = serde_json::from_slice(&out_tx3.stderr).unwrap();
    assert_eq!(
        err_tx3["error"]["kind"], "attachment_too_large",
        "{err_tx3}"
    );
    let detail_tx3 = err_tx3["error"]["detail"].as_str().unwrap();
    assert!(
        detail_tx3.contains(".catalog.json") && detail_tx3.contains("400"),
        "got: {detail_tx3}"
    );
    assert_zero_side_effects(&[(&f.records, &s_id), (&receiver_records, &to)], &head3_tx);
}

#[test]
fn working_preimage_names_specific_unledgered_file_path() {
    let f = checkpoint_fixture();
    let payload = f.dir.join("payload.txt");
    std::fs::write(&payload, "payload").unwrap();
    let req = checkpoint_request(&f, 301, json!([declaration(&payload, "evidence")]));
    let req_path = f.request("cp-orphan.json", &req);

    // 1. Fresh checkpoint with unledgered file in tasks/files/<id>/.
    let files_dir = f.records.join(format!("tasks/files/{}", f.id));
    std::fs::create_dir_all(&files_dir).unwrap();
    let orphan = files_dir.join("stray-orphan.json");
    std::fs::write(&orphan, b"{\"orphan\":true}").unwrap();

    let out = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "retained_stale_basis", "{err}");
    let detail = err["error"]["detail"].as_str().unwrap();
    let expected = format!(
        "Protected attachment directory tasks/files/{} contains unledgered file stray-orphan.json; preserve and verify a copy outside tasks/files/{} if needed, remove the unledgered file to restore the committed preimage, and retry or reconcile",
        f.id, f.id
    );
    assert_eq!(detail, expected);
    assert!(
        orphan.exists(),
        "orphan file must never be silently deleted"
    );

    // 2. Remove the unledgered file, stop after `intent`, drop the orphan again, and verify
    //    `working_preimage` during phase retry emits the exact same diagnostic.
    std::fs::remove_file(&orphan).unwrap();
    std::fs::remove_dir(&files_dir).unwrap();
    let stopped = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "intent")
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(stopped.status.code(), Some(86), "{stopped:?}");
    std::fs::write(&orphan, b"{\"orphan\":true}").unwrap();

    let out_retry = f
        .cmd()
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            req_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out_retry.status.code(), Some(1), "{out_retry:?}");
    let err_retry: serde_json::Value = serde_json::from_slice(&out_retry.stderr).unwrap();
    assert_eq!(
        err_retry["error"]["kind"], "retained_stale_basis",
        "{err_retry}"
    );
    assert!(
        err_retry["error"]["detail"]
            .as_str()
            .unwrap()
            .starts_with(&expected),
        "got: {err_retry}"
    );
    assert!(orphan.exists());

    // Removing the unledgered file restores the committed preimage so retry succeeds.
    std::fs::remove_file(&orphan).unwrap();
    assert_eq!(checkpoint(&f, &req)["outcome"], "verified");
}

#[test]
fn artifact_location_and_rebuild_multipart_verify_logical_multipart_and_reject_tampering() {
    let f = checkpoint_fixture();
    configure_attachments(&f.records, "max_bytes = 2048\nmultipart_validators = []\n");

    let prior = f.dir.join("prior-container");
    let tree = f.dir.join("live-container");
    let (manifest_bytes, manifest_sha) = build_approx_5k_container(&prior);
    let _ = build_approx_5k_container(&tree);

    let manifest_name = format!("retained-manifest-{manifest_sha}.json");
    let tmp_manifest = f.dir.join("manifest-to-attach.json");
    std::fs::write(&tmp_manifest, &manifest_bytes).unwrap();
    let attach_out = f.json(&[
        "attach",
        &f.id,
        tmp_manifest.to_str().unwrap(),
        "--name",
        &manifest_name,
        "--multipart",
    ]);
    assert_eq!(
        attach_out["publication"]["state"], "published",
        "{attach_out}"
    );
    assert!(attach_out["multipart"]["parts"].as_u64().unwrap() >= 3);

    let recorded_manifest_path = f
        .records
        .join(format!("tasks/files/{}/{manifest_name}", f.id));
    assert!(
        !recorded_manifest_path.exists(),
        "multipart logical path has no single physical file"
    );
    let item = json!({
        "version": 1,
        "host": host(),
        "kind": "scratch",
        "path": tree.to_str().unwrap(),
        "state": "kept",
        "verification": "verified",
        "purpose": "Review container",
        "next_owner": format!("task:{}", f.id),
        "cleanup_trigger": "Review completes",
        "sha256": manifest_sha,
        "manifest": recorded_manifest_path.to_str().unwrap(),
    });
    f.json(&["note", &f.id, &format!("retained: {item}")]);
    let multipart_state = f.state();
    assert_eq!(multipart_state["complete"], true);
    let multipart_basis = multipart_state["basis"].clone();
    let identity = multipart_state["items"][0]["identity"].clone();

    // Shrink `tree` to a 1-file directory (single-file manifest <= 2048 bytes) while `prior`
    // preserves all 42 members of the prior multipart manifest.
    std::fs::remove_dir_all(&tree).unwrap();
    std::fs::create_dir_all(&tree).unwrap();
    std::fs::write(tree.join("summary.txt"), "condensed").unwrap();

    let attachment_locator = json!({
        "type": "attachment",
        "task": f.id,
        "name": manifest_name,
        "basis": multipart_basis,
        "sha256": manifest_sha,
    });
    let dir_locator = json!({
        "type": "directory",
        "host": host(),
        "path": prior.to_str().unwrap(),
        "manifest": attachment_locator,
    });
    let mut good_req = checkpoint_request(&f, 401, json!([]));
    good_req["preserved_previous"] = json!([{
        "identity": identity,
        "artifact": dir_locator,
    }]);
    let res = checkpoint(&f, &good_req);
    assert_eq!(res["outcome"], "verified", "{res}");
    assert_eq!(res["counts"]["kept_verified"], 1);
    assert_eq!(f.state()["complete"], true);

    // Negative tampered-basis cases via committed tampered tasks:
    let run_tampered_case = |op_n: u64, mutate_task_md: &dyn Fn(String) -> String| -> String {
        let tid = f.json(&["add", "Tampered multipart case"])["id"]
            .as_str()
            .unwrap()
            .to_string();
        f.json(&["start", &tid]);
        f.json(&[
            "attach",
            &tid,
            tmp_manifest.to_str().unwrap(),
            "--name",
            &manifest_name,
            "--multipart",
        ]);
        let task_md = f.records.join(format!("tasks/{tid}.md"));
        let before_raw = std::fs::read_to_string(&task_md).unwrap();
        let mutated = mutate_task_md(before_raw.clone());
        assert_ne!(mutated, before_raw, "tamper mutation must modify task file");
        std::fs::write(&task_md, mutated).unwrap();
        common::git(&f.records, &["add", task_md.to_str().unwrap()]);
        common::git(&f.records, &["commit", "-qm", "tamper multipart history"]);
        let bad_basis = f.json(&["retained", "state", &tid])["basis"].clone();

        let host_task = f.json(&["add", "Consumer of tampered locator"])["id"]
            .as_str()
            .unwrap()
            .to_string();
        f.json(&["start", &host_task]);
        let case_tree = f.dir.join(format!("case-tree-{op_n}"));
        std::fs::create_dir_all(&case_tree).unwrap();
        std::fs::write(case_tree.join("summary.txt"), "condensed").unwrap();
        let prior_item = json!({
            "version": 1,
            "host": host(),
            "kind": "scratch",
            "path": case_tree.to_str().unwrap(),
            "state": "kept",
            "verification": "verified",
            "purpose": "Review",
            "next_owner": format!("task:{host_task}"),
            "cleanup_trigger": "Review completes",
            "sha256": manifest_sha,
            "manifest": off_container_copy(&f, &manifest_bytes, op_n).to_str().unwrap(),
        });
        f.json(&["note", &host_task, &format!("retained: {prior_item}")]);
        let host_state = f.json(&["retained", "state", &host_task]);
        let req = json!({
            "version": 1,
            "operation_id": format!("{op_n:032x}"),
            "host": host(),
            "inputs": [{"task": host_task, "basis": host_state["basis"]}],
            "preserved_previous": [{
                "identity": host_state["items"][0]["identity"],
                "artifact": {
                    "type": "directory",
                    "host": host(),
                    "path": prior.to_str().unwrap(),
                    "manifest": {
                        "type": "attachment",
                        "task": tid,
                        "name": manifest_name,
                        "basis": bad_basis,
                        "sha256": manifest_sha,
                    }
                }
            }]
        });
        let path = f.request(&format!("tampered-{op_n}.json"), &req);
        let out = f.json(&[
            "retained",
            "checkpoint",
            &host_task,
            "--request",
            path.to_str().unwrap(),
        ]);
        assert_eq!(out["outcome"], "accounted_unknown", "{out}");
        out["actions"][0].as_str().unwrap().to_string()
    };

    fn off_container_copy(f: &RetainedEnv, bytes: &[u8], op_n: u64) -> std::path::PathBuf {
        let p = f.dir.join(format!("off-container-{op_n}.json"));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    // Case A: missing `multipart:` declaration note -> "preserved attachment lacks a live ledger entry".
    let action_missing_decl = run_tampered_case(410, &|raw| {
        raw.lines()
            .filter(|line| !line.contains(": multipart: "))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    });
    assert!(
        action_missing_decl.contains("preserved attachment lacks a live ledger entry"),
        "got: {action_missing_decl}"
    );

    // Case B: mismatched `declaration.sha256` -> "preserved multipart declaration digest mismatch".
    let action_bad_digest = run_tampered_case(411, &|raw| {
        raw.replace(
            &format!("sha256 {manifest_sha},"),
            &format!("sha256 {},", "b".repeat(64)),
        )
    });
    assert!(
        action_bad_digest.contains("preserved multipart declaration digest mismatch"),
        "got: {action_bad_digest}"
    );

    // Case C: missing `.part-0001` ledger entry -> "preserved multipart attachment lacks a live member ledger entry".
    let part1 = format!("{manifest_name}.part-0001");
    let action_missing_part = run_tampered_case(412, &|raw| {
        raw.lines()
            .filter(|line| !line.contains(&format!("attached: {part1}")))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    });
    assert!(
        action_missing_part
            .contains("preserved multipart attachment lacks a live member ledger entry"),
        "got: {action_missing_part}"
    );

    // Case D: detached `.catalog.json` ledger entry -> "preserved multipart attachment lacks a live member ledger entry".
    let cat = format!("{manifest_name}.catalog.json");
    let action_detached_cat = run_tampered_case(413, &|mut raw| {
        raw.push_str(&format!(
            "- 2099-11-16T00:00:59Z (fixture): detached: {cat}: tampered\n"
        ));
        raw
    });
    assert!(
        action_detached_cat
            .contains("preserved multipart attachment lacks a live member ledger entry"),
        "got: {action_detached_cat}"
    );
}
