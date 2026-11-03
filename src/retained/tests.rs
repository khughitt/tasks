use super::{model::*, state};
use crate::model::Task;
fn task() -> Task {
    crate::format::parse_physical("---\nid: sci-000001\ntitle: T\nstatus: todo\npriority: 2\ndepends: []\ntags: []\ncreated: 2026-11-03T00:00:00Z\nupdated: 2026-11-03T00:00:00Z\n---\n\n## Notes\n", "fixture").unwrap().assemble(Vec::new())
}
use serde_json::{Value, json};
fn record() -> Value {
    json!({"version":1,"host":"h","kind":"evidence","path":"/data/a","state":"kept","verification":"verified","purpose":"Review","next_owner":"task:sci-000001","cleanup_trigger":"After review","sha256":"a".repeat(64)})
}
fn basis() -> Basis {
    serde_json::from_value(json!({"project":"sci","layout":"in-tree","records_subdir":".","commit":"a".repeat(40),"task_path":"tasks/sci-000001.md","retained_ref":"refs/heads/main"})).unwrap()
}
#[test]
fn retained_contract_vectors() {
    let cases: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/retention/cases.json")).unwrap();
    for c in cases.as_array().unwrap() {
        let raw = c["raw"].as_str().unwrap();
        let valid = if c["kind"] == "record" {
            decode_record(raw).is_ok_and(|r| r.version == Some(1))
        } else {
            decode_manifest(raw).is_ok()
        };
        assert_eq!(
            valid,
            c["structural_valid"].as_bool().unwrap(),
            "{}",
            c["name"]
        );
    }
}
#[test]
fn retained_anchored_patterns() {
    for key in ["path", "sha256"] {
        let mut r = record();
        r[key] = json!(format!("{}\n", r[key].as_str().unwrap()));
        assert!(decode_record(&r.to_string()).is_err());
    }
    assert!(decode_record(&record().to_string()).is_ok());
    let raw = record().to_string().replacen("{", "{\"version\":1,", 1);
    assert!(decode_record(&raw).is_err());
}
#[test]
fn retained_same_second_replay() {
    let mut t = task();
    let r = record();
    crate::commands::append_note(&mut t, "t", &format!("retained: {r}")).unwrap();
    let mut r = r;
    r["purpose"] = json!("Changed");
    crate::commands::append_note(&mut t, "t", &format!("retained: {r}")).unwrap();
    let s = state::replay(&t, &basis(), None).unwrap();
    assert!(s.complete);
    assert_eq!(s.items[0].source.as_ref().unwrap().ordinal, 2);
    assert_eq!(s.items[0].record.as_ref().unwrap().purpose, "Changed");
}
#[test]
fn retained_malformed_scope() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t, "t", "retained: {\"version\":2}").unwrap();
    let s = state::replay(&t, &basis(), None).unwrap();
    assert!(!s.complete);
    assert!(s.items[0].record.is_none());
    assert_eq!(s.diagnostics[0].code, "unsupported_version");
}
#[test]
fn retained_phase_shapes() {
    assert!(
        decode_operation(r#"{"version":1,"phase":"intent","outputs":{"unexpected":1}}"#).is_err()
    );
    assert!(decode_checkpoint(r#"{"version":2}"#).is_err());
}
#[test]
fn retained_resolution_coverage() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", "retained: {\"host\":\"h\",\"path\":4}").unwrap();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    let s = state::replay(&t, &basis(), Some("h")).unwrap();
    assert!(!s.complete);
    assert!(s.items[0].record.is_none());
}

#[test]
fn retained_duplicate_host_blocks_both_scopes() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(
        &mut t,
        "t",
        r#"retained: {"host":"h","host":"other","version":2}"#,
    )
    .unwrap();
    let s = state::replay(&t, &basis(), Some("h")).unwrap();
    assert!(!s.complete);
    assert!(s.items[0].record.is_none());
}

#[test]
fn retained_request_shapes() {
    let input = json!({"task":"sci-000001","basis":basis()});
    let good =
        json!({"version":1,"operation_id":"0".repeat(32),"host":"h","inputs":[input.clone()]});
    let r: CheckpointRequest = decode_request(good.to_string().as_bytes()).unwrap();
    assert!(r.validate().is_ok());
    let mut bad = good.clone();
    bad["version"] = json!(2);
    let r: CheckpointRequest = decode_request(bad.to_string().as_bytes()).unwrap();
    assert!(r.validate().is_err());
    let transfer = json!({"version":1,"operation_id":"1".repeat(32),"host":"h","inputs":[input.clone(),input.clone()],"items":[{"host":"h","kind":"evidence","path":"/data/a"}],"purpose":"Review","cleanup_trigger":"After review"});
    let r: TransferRequest = decode_request(transfer.to_string().as_bytes()).unwrap();
    assert!(r.validate().is_err());
    let reconciliation = json!({"version":1,"operation":{"coordinator":"sci-000001","operation_id":"0".repeat(32)},"reconciliation_id":"1".repeat(32),"inputs":[input],"reason":"Restore accounting"});
    let r: ReconcileRequest = decode_request(reconciliation.to_string().as_bytes()).unwrap();
    assert!(r.validate().is_ok());
}

fn operation(phase: &str, outputs: Value) -> Value {
    json!({"version":1,"key":{"coordinator":"sci-000001","operation_id":"0".repeat(32)},"phase":phase,
        "request":{"type":"git","basis":basis(),"path":"request.json","sha256":"a".repeat(64)},
        "input_bases":[{"task":"sci-000001","basis":basis()}],"host":"h","at":"2026-11-03T00:00:00Z","observer":null,"outputs":outputs,"diagnostics":[]})
}
#[test]
fn retained_record_conditional_nulls() {
    for key in ["handoff", "verification_action"] {
        let mut r = record();
        r[key] = Value::Null;
        assert!(decode_record(&r.to_string()).is_err(), "{key}");
    }
}
#[test]
fn retained_phase_history() {
    let intent = operation("intent", json!({}));
    assert!(decode_operation(&intent.to_string()).is_ok());
    let mut bad = intent.clone();
    bad["observer"] = json!(false);
    assert!(decode_operation(&bad.to_string()).is_err());
    let mut bad = intent.clone();
    bad.as_object_mut().unwrap().remove("observer");
    assert!(decode_operation(&bad.to_string()).is_err());
    let release = operation(
        "sender_released",
        json!({"items":[1],"receiver_basis":basis(),"mapping":[]}),
    );
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retention operation: {release}")).unwrap();
    assert!(!state::replay(&t, &basis(), None).unwrap().complete);
}
#[test]
fn retained_explicit_identity_resolution() {
    let mut t = task();
    let mut bad = record();
    bad["sha256"] = json!("broken");
    crate::commands::append_note(&mut t, "t", &format!("retained: {bad}")).unwrap();
    crate::commands::append_note(
        &mut t,
        "t",
        &format!("retention operation: {}", operation("intent", json!({}))),
    )
    .unwrap();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    let receipt = operation(
        "items_published",
        json!({"items":[3],"artifacts":[],"resolutions":[{"bad":{"basis":basis(),"ordinal":1},"successors":[3],"reason":"Recovered exact history"}]}),
    );
    crate::commands::append_note(&mut t, "t", &format!("retention operation: {receipt}")).unwrap();
    let s = state::replay(&t, &basis(), None).unwrap();
    assert!(s.complete);
    assert_eq!(s.diagnostics.len(), 1);
    assert!(s.diagnostics[0].resolved_by.is_some());
    assert!(s.items[0].record.is_some());
}

#[test]
fn retained_all_phase_outputs_are_closed() {
    let locator =
        json!({"type":"git","basis":basis(),"path":"request.json","sha256":"a".repeat(64)});
    for (phase, outputs) in [
        ("intent", json!({})),
        (
            "items_published",
            json!({"items":[],"artifacts":[],"resolutions":[]}),
        ),
        (
            "receiver_published",
            json!({"items":[],"artifacts":[],"mapping":[]}),
        ),
        (
            "sender_released",
            json!({"items":[],"receiver_basis":basis(),"mapping":[]}),
        ),
        (
            "completed",
            json!({"outcome":"verified","publications":[],"checkpoint":null}),
        ),
        (
            "incomplete",
            json!({"attempt":1,"previous_phase":"intent","checkpoint":null,"action":"Inspect missing bytes"}),
        ),
        (
            "reconcile_intent",
            json!({"directive":locator,"reconciliation_id":"1".repeat(32)}),
        ),
        (
            "reconcile_ready",
            json!({"directive":locator,"reconciliation_id":"1".repeat(32),"items":[],"unresolved":[]}),
        ),
        (
            "reconciled",
            json!({"directive":locator,"reconciliation_id":"1".repeat(32),"ready_bases":[],"unresolved":[]}),
        ),
    ] {
        let valid = operation(phase, outputs);
        assert!(decode_operation(&valid.to_string()).is_ok(), "{phase}");
        let mut bad = valid;
        bad["outputs"]["unexpected"] = json!(1);
        assert!(decode_operation(&bad.to_string()).is_err(), "{phase}");
    }
}
