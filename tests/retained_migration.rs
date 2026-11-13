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
