mod common;
use common::retained::HistoricalScenario;
use common::retained_migration::migration_fixture;
fn records(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| serde_json::json!({"note":v["note"],"ordinal":v["source"]["ordinal"]}))
        .collect()
}

#[test]
fn migration_original_publications() {
    for scenario in [
        HistoricalScenario::CheckpointCompleted,
        HistoricalScenario::Reconciled,
        HistoricalScenario::TransferSenderReleased,
    ] {
        let f = migration_fixture(scenario);
        let current = f.f.state();
        assert_eq!(current["complete"], true);
        assert_eq!(
            records(&current["operations"]),
            records(&f.original["operations"])
        );
        assert_eq!(
            records(&current["checkpoints"]),
            records(&f.original["checkpoints"])
        );
    }
}
#[test]
fn migration_archived_prefix() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let preview = f.f.json(&["archive-notes", &f.f.id]);
    assert_eq!(preview["eligible"], true);
    f.f.json(&[
        "archive-notes",
        &f.f.id,
        "--expect",
        preview["source_sha256"].as_str().unwrap(),
    ]);
    let current = f.f.state();
    assert_eq!(
        records(&current["operations"]),
        records(&f.original["operations"])
    );
}
#[test]
fn migration_rename_refuses_before_intent() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let out = f.f.cmd().args(["rename", "sci", "abc"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("recovery"),
        "{out:?}"
    );
}
#[test]
fn migration_boundary_rejections() {
    for case in [
        "config",
        "task",
        "mode",
        "tree",
        "missing_trailer",
        "duplicate_trailer",
        "body_trailer",
        "rule",
    ] {
        let mut f = migration_fixture(HistoricalScenario::CheckpointCompleted);
        f.replace_import(case);
        let out =
            f.f.cmd()
                .args(["retained", "state", &f.f.id])
                .output()
                .unwrap();
        assert!(!out.status.success(), "accepted {case}: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("recovery"),
            "{case}: {out:?}"
        );
    }
}
#[test]
fn migration_wrong_repository() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let source_head = f.git(&f.source, &["rev-parse", "HEAD"]);
    let destination_head = f.git(&f.destination, &["rev-parse", "HEAD"]);
    let out =
        f.f.cmd()
            .args(["retained", "state", &f.f.id, "--at", &f.cutover])
            .output()
            .unwrap();
    assert!(
        !out.status.success(),
        "source revision resolved in destination"
    );
    let before_import = f.git(&f.destination, &["rev-parse", &format!("{}^1", f.import)]);
    let out =
        f.f.cmd()
            .args(["retained", "state", &f.f.id, "--at", &before_import])
            .output()
            .unwrap();
    assert!(!out.status.success(), "pre-import revision must refuse");
    assert_eq!(f.git(&f.source, &["rev-parse", "HEAD"]), source_head);
    assert_eq!(
        f.git(&f.destination, &["rev-parse", "HEAD"]),
        destination_head
    );
}
#[test]
fn migration_retention_anchors() {
    for case in ["lost_ref", "bad_endpoint", "lost_source"] {
        let mut f = migration_fixture(HistoricalScenario::CheckpointCompleted);
        match case {
            "lost_ref" => {
                f.declaration["source_ref"] = serde_json::json!("refs/heads/missing");
                f.write_declaration(&f.declaration);
            }
            "bad_endpoint" => {
                f.declaration["refs"][0]["endpoint"] =
                    serde_json::json!(f.git(&f.source, &["rev-parse", "HEAD"]));
                f.write_declaration(&f.declaration);
            }
            "lost_source" => {
                std::fs::rename(f.source.join(".git"), f.source.join("git-unavailable")).unwrap()
            }
            _ => unreachable!(),
        }
        let out =
            f.f.cmd()
                .args(["retained", "state", &f.f.id])
                .output()
                .unwrap();
        assert!(!out.status.success(), "accepted {case}: {out:?}");
    }
}
#[test]
fn migration_prefix_provenance_refuses() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let path = f.f.records.join(format!("tasks/{}.md", f.f.id));
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("(fixture):"));
    std::fs::write(path, raw.replacen("(fixture):", "(forged-author):", 1)).unwrap();
    f.git(&f.destination, &["add", "."]);
    f.git(&f.destination, &["commit", "-qm", "alter provenance"]);
    let out =
        f.f.cmd()
            .args(["retained", "state", &f.f.id])
            .output()
            .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("prefix"),
        "{out:?}"
    );
}
#[test]
fn migration_mixed_epoch_refuses() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let current = f.f.state();
    let mut note = current["operations"][0]["note"].clone();
    note["input_bases"][0]["basis"] = current["basis"].clone();
    f.f.seed_note(&format!("retention operation: {note}"));
    let out =
        f.f.cmd()
            .args(["retained", "state", &f.f.id])
            .output()
            .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("epochs"),
        "{out:?}"
    );
}
use common::retained_migration::{manual_manifest, migration_fixture_with};
#[test]
fn migration_generated_manifest() {
    let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
    f.bind_manifests();
    let head = f.git(&f.source, &["rev-parse", "HEAD"]);
    let original = f.f.state()["items"][0]["record"].clone();
    let out = f.checkpoint();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "verified", "{result}");
    let current = f.f.state();
    assert_eq!(current["basis"]["layout"], "detached");
    assert_eq!(
        current["items"][0]["record"]["manifest"],
        serde_json::json!(f.f.records.join(format!(
                "tasks/files/{}/{}",
                f.f.id,
                f.declaration["manifest_bindings"][0]["attachment"]
                    .as_str()
                    .unwrap()
            )))
    );
    assert_ne!(
        current["items"][0]["record"]["manifest"],
        original["manifest"]
    );
    assert_eq!(f.git(&f.source, &["rev-parse", "HEAD"]), head);
}
#[test]
fn migration_manual_attachment_binding() {
    let mut f = migration_fixture_with(HistoricalScenario::CheckpointCompleted, |f| {
        manual_manifest(f, false)
    });
    f.bind_manifests();
    let out = f.checkpoint();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "verified", "{result}");
}
#[test]
fn migration_external_manifest_literal() {
    let f = migration_fixture_with(HistoricalScenario::CheckpointCompleted, |f| {
        manual_manifest(f, true)
    });
    let out = f.checkpoint();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "verified", "{result}");
}
#[test]
fn migration_new_checkpoint() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let out = f.checkpoint();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "verified", "{result}");
    let state = f.f.state();
    let operations = state["operations"].as_array().unwrap();
    let new = operations
        .iter()
        .filter(|o| o["note"]["key"]["operation_id"] == "d".repeat(32))
        .collect::<Vec<_>>();
    assert_eq!(new.len(), 3);
    for operation in new {
        assert_eq!(
            operation["note"]["input_bases"][0]["basis"]["layout"],
            "detached"
        );
    }
}
#[test]
fn migration_new_transfer() {
    let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
    f.bind_manifests();
    let to = f.f.json(&["add", "New custodian"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.f.json(&["start", &to]);
    let request = serde_json::json!({"version":1,"operation_id":"e".repeat(32),"host":common::retained::host(),"inputs":[{"task":f.f.id,"basis":f.f.state()["basis"]},{"task":to,"basis":f.f.json(&["retained","state",&to])["basis"]}],"items":[f.original["items"][0]["identity"]],"purpose":"Continue review","cleanup_trigger":"Review complete"});
    let path = f.f.request("new-transfer.json", &request);
    let head = f.git(&f.source, &["rev-parse", "HEAD"]);
    let out =
        f.f.cmd()
            .args([
                "retained",
                "transfer",
                &f.f.id,
                &to,
                "--request",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "transferred", "{result}");
    assert_eq!(f.git(&f.source, &["rev-parse", "HEAD"]), head);
    assert_eq!(f.f.state()["items"][0]["record"]["state"], "released");
    let receiver = f.f.json(&["retained", "state", &to]);
    assert_eq!(receiver["items"][0]["record"]["verification"], "verified");
}
#[test]
fn migration_missing_manifest() {
    for case in ["missing", "changed", "unbound"] {
        let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
        if case != "unbound" {
            f.bind_manifests();
        }
        let path = f.f.records.join(format!(
            "tasks/files/{}/{}",
            f.f.id,
            std::path::Path::new(
                f.original["items"][0]["record"]["manifest"]
                    .as_str()
                    .unwrap()
            )
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
        ));
        match case {
            "missing" => std::fs::remove_file(path).unwrap(),
            "changed" => std::fs::write(path, b"changed").unwrap(),
            _ => {}
        }
        let before = f.f.state();
        let out = f.checkpoint();
        assert!(!out.status.success(), "accepted {case}: {out:?}");
        assert_eq!(
            f.f.state()["operations"],
            before["operations"],
            "refusal published a phase for {case}"
        );
    }
}
#[test]
fn migration_suffix_and_config_change() {
    let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
    f.bind_manifests();
    let mut record = f.original["items"][0]["record"].clone();
    record["purpose"] = serde_json::json!("Later source-style reference");
    let mut basis = f.f.state()["basis"].clone();
    f.f.note(record);
    let head = f.git(&f.destination, &["rev-parse", "HEAD"]);
    basis["commit"] = serde_json::json!(head);
    let out =
        f.f.cmd()
            .args(["retained", "state", &f.f.id])
            .output()
            .unwrap();
    assert!(!out.status.success());
    let request = serde_json::json!({"version":1,"operation_id":"d".repeat(32),"host":common::retained::host(),"inputs":[{"task":f.f.id,"basis":basis}]});
    let request = f.f.request("suffix.json", &request);
    let out =
        f.f.cmd()
            .args([
                "retained",
                "checkpoint",
                &f.f.id,
                "--request",
                request.to_str().unwrap(),
            ])
            .output()
            .unwrap();
    assert!(!out.status.success(), "{out:?}");
    assert_eq!(f.git(&f.destination, &["rev-parse", "HEAD"]), head);
}
#[test]
fn migration_check_tree_identity() {
    for case in ["mode", "tree", "config"] {
        let mut f = migration_fixture(HistoricalScenario::CheckpointCompleted);
        let out = f.f.cmd().args(["check"]).output().unwrap();
        assert!(out.status.success(), "initial {out:?}");
        f.replace_import(case);
        let out = f.f.cmd().args(["check"]).output().unwrap();
        assert!(!out.status.success(), "accepted changed {case}: {out:?}");
        let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            result["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "retention_recovery"),
            "{result}"
        );
    }
}
#[test]
fn migration_check_live_prefix() {
    for case in ["provenance", "source_style_suffix"] {
        let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
        f.bind_manifests();
        let out = f.f.cmd().args(["check"]).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        if case == "provenance" {
            let path = f.f.records.join(format!("tasks/{}.md", f.f.id));
            let raw = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, raw.replacen("(fixture):", "(forged):", 1)).unwrap();
        } else {
            let mut record = f.original["items"][0]["record"].clone();
            record["purpose"] = serde_json::json!("new source-style suffix");
            f.f.note(record);
        }
        let out = f.f.cmd().args(["check"]).output().unwrap();
        assert!(!out.status.success(), "accepted {case}: {out:?}");
        let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            result["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "retention_recovery"),
            "{result}"
        );
    }
}
#[test]
fn migration_config_roundtrip_and_rename_guard() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let config = f.f.records.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    f.f.json(&[
        "init",
        "--prefix",
        "sci",
        "--records",
        f.destination.to_str().unwrap(),
        "--force",
    ]);
    assert_eq!(std::fs::read(&config).unwrap(), before);
    let head = f.git(&f.destination, &["rev-parse", "HEAD"]);
    let out = f.f.cmd().args(["rename", "sci", "abc"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(f.git(&f.destination, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read(config).unwrap(), before);
}
#[test]
fn migration_check_recovers_peer_context() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    let (records, to) = common::retained::cross_receiver(&f.f, false);
    let request = serde_json::json!({"version":1,"operation_id":"f".repeat(32),"host":common::retained::host(),"inputs":[{"task":f.f.id,"basis":f.f.state()["basis"]},{"task":to,"basis":f.f.json(&["retained","state",&to])["basis"]}],"items":[f.original["items"][0]["identity"]],"purpose":"Peer review","cleanup_trigger":"Review complete"});
    let path = f.f.request("peer-transfer.json", &request);
    let out =
        f.f.cmd()
            .args([
                "retained",
                "transfer",
                &f.f.id,
                &to,
                "--request",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(f.f.json(&["retained", "state", &to])["complete"], true);
    let out = f.f.env.cmd(&records).args(["check"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
}
#[test]
fn migration_check_source_archive() {
    let mut f = migration_fixture_with(HistoricalScenario::CheckpointCompleted, |f| {
        let preview = f.json(&["archive-notes", &f.id]);
        assert_eq!(preview["eligible"], true);
        f.json(&[
            "archive-notes",
            &f.id,
            "--expect",
            preview["source_sha256"].as_str().unwrap(),
        ]);
    });
    f.declaration["refs"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"original_ref":"refs/heads/disposable","endpoint":f.cutover}));
    f.write_declaration(&f.declaration);
    let out = f.f.cmd().args(["check"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(f.f.state()["complete"], true);
}
#[test]
fn migration_actual_current_pairing_paths() {
    let mut f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    f.git(&f.destination, &["mv", "sci", "recovered-sci"]);
    f.git(
        &f.destination,
        &["commit", "-qm", "fixture current pairing"],
    );
    f.f.records = f.destination.join("recovered-sci");
    let registry = f.f.env.home.path().join(".config/tasks/projects.toml");
    let mut value: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry).unwrap()).unwrap();
    value["records"]["sci"] = toml::Value::String(f.f.records.to_str().unwrap().into());
    std::fs::write(registry, toml::to_string(&value).unwrap()).unwrap();
    let state = f.f.state();
    assert_eq!(state["complete"], true);
    assert_eq!(state["basis"]["records_subdir"], "recovered-sci");
    assert_eq!(
        records(&state["operations"]),
        records(&f.original["operations"])
    );
    let out = f.f.cmd().args(["check"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
}
#[test]
fn migration_destination_named_ref_with_detached_head() {
    let f = migration_fixture(HistoricalScenario::CheckpointCompleted);
    f.git(&f.destination, &["checkout", "--detach", "-q", "HEAD"]);
    let state = f.f.json(&["retained", "state", &f.f.id, "--at", "HEAD"]);
    assert_eq!(state["complete"], true);
    assert_eq!(state["basis"]["retained_ref"], "refs/heads/main");
    let out = f.f.cmd().args(["check"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
}
#[test]
fn migration_missing_payload_remains_accounted_unknown() {
    let mut f = migration_fixture(HistoricalScenario::CheckpointDirectory);
    f.bind_manifests();
    let previous = f.f.state()["items"][0]["record"].clone();
    std::fs::remove_dir_all(f.source.join("payload")).unwrap();
    let out = f.checkpoint();
    assert!(out.status.success(), "{out:?}");
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["outcome"], "accounted_unknown");
    let state = f.f.state();
    let record = &state["items"][0]["record"];
    assert_eq!(record["verification"], "unknown");
    assert_eq!(record["sha256"], serde_json::Value::Null);
    let expected = f.f.records.join(format!(
        "tasks/files/{}/{}",
        f.f.id,
        std::path::Path::new(previous["manifest"].as_str().unwrap())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
    ));
    assert_eq!(record["manifest"], serde_json::json!(expected));
}
