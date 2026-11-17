use super::{model::*, publish, state};
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
    let s = state::replay(&project(), &t, &basis(), None).unwrap();
    assert!(s.complete);
    assert_eq!(s.items[0].source.as_ref().unwrap().ordinal, 2);
    assert_eq!(s.items[0].record.as_ref().unwrap().purpose, "Changed");
}
#[test]
fn retained_malformed_scope() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t, "t", "retained: {\"version\":2}").unwrap();
    let s = state::replay(&project(), &t, &basis(), None).unwrap();
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
    let s = state::replay(&project(), &t, &basis(), Some("h")).unwrap();
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
    let s = state::replay(&project(), &t, &basis(), Some("h")).unwrap();
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
    assert!(
        !state::replay(&project(), &t, &basis(), None)
            .unwrap()
            .complete
    );
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
    let s = state::replay(&project(), &t, &basis(), None).unwrap();
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

#[test]
fn retained_local_request_locator() {
    let mut op = operation("intent", json!({}));
    op["request"] = json!({"type":"attachment","task":"sci-000001","name":"retained-op-00000000000000000000000000000000.request.json","sha256":"a".repeat(64)});
    assert!(decode_operation(&op.to_string()).is_ok());
}
#[test]
fn retained_operation_nested_contract() {
    for (phase, out) in [
        (
            "items_published",
            json!({"items":[0],"artifacts":[],"resolutions":[]}),
        ),
        (
            "completed",
            json!({"outcome":"invented","publications":[],"checkpoint":null}),
        ),
        (
            "incomplete",
            json!({"attempt":0,"previous_phase":"intent","checkpoint":null,"action":"Retry"}),
        ),
        (
            "sender_released",
            json!({"items":[1],"receiver_basis":{ "project":"sci","layout":"in-tree","records_subdir":".","commit":"bad","task_path":"tasks/sci-000001.md","retained_ref":"refs/heads/main"},"mapping":[]}),
        ),
    ] {
        assert!(
            decode_operation(&operation(phase, out).to_string()).is_err(),
            "{phase}"
        );
    }
}

#[test]
fn retained_operation_mapping_and_actions_contract() {
    let identity = json!({"host":"h","kind":"evidence","path":"/data/a"});
    for side in ["sender", "receiver"] {
        for field in ["host", "kind", "path"] {
            let mut mapping = json!({"sender":identity,"receiver":identity});
            mapping[side][field] = json!("");
            let n = operation(
                "receiver_published",
                json!({"items":[1],"artifacts":[],"mapping":[mapping]}),
            );
            assert!(decode_operation(&n.to_string()).is_err(), "{side}.{field}");
        }
    }
    let locator =
        json!({"type":"git","basis":basis(),"path":"request.json","sha256":"a".repeat(64)});
    for (phase, outputs) in [
        (
            "reconcile_ready",
            json!({"directive":locator,"reconciliation_id":"1".repeat(32),"items":[],"unresolved":[""]}),
        ),
        (
            "reconciled",
            json!({"directive":locator,"reconciliation_id":"1".repeat(32),"ready_bases":[],"unresolved":[""]}),
        ),
    ] {
        assert!(
            decode_operation(&operation(phase, outputs).to_string()).is_err(),
            "{phase}"
        );
    }
}

#[test]
fn retained_known_other_host_does_not_block_local_scope() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t, "t", r#"retained: {"host":"other","version":2}"#).unwrap();
    assert!(
        !state::replay(&project(), &t, &basis(), None)
            .unwrap()
            .complete
    );
    let local = state::replay(&project(), &t, &basis(), Some("h")).unwrap();
    assert!(local.complete);
    assert!(local.items[0].record.is_some());
}

#[test]
fn retained_operation_item_role_and_mapping_binding() {
    for (phase, outputs) in [
        (
            "sender_released",
            json!({"items":[2],"receiver_basis":basis(),"mapping":[]}),
        ),
        (
            "receiver_published",
            json!({"items":[2],"artifacts":[],"mapping":[]}),
        ),
    ] {
        let mut t = task();
        crate::commands::append_note(
            &mut t,
            "t",
            &format!("retention operation: {}", operation("intent", json!({}))),
        )
        .unwrap();
        crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
        crate::commands::append_note(
            &mut t,
            "t",
            &format!("retention operation: {}", operation(phase, outputs)),
        )
        .unwrap();
        assert!(
            !state::replay(&project(), &t, &basis(), None)
                .unwrap()
                .complete,
            "{phase}"
        );
    }
}
#[test]
fn retained_operation_nullable_contract() {
    let mut n = operation("intent", json!({}));
    n["observer"] = json!("");
    assert!(decode_operation(&n.to_string()).is_err());
    let mut n = operation("intent", json!({}));
    n["diagnostics"] = json!([{"code":"gap","scope":"task","host":null,"identity":null,"detail":"Missing basis","action":"Restore"}]);
    assert!(decode_operation(&n.to_string()).is_err());
    let mut n = operation("intent", json!({}));
    n["request"] = json!({"type":"attachment","task":"sci-000001","name":"request.json","basis":null,"sha256":"a".repeat(64)});
    assert!(decode_operation(&n.to_string()).is_err());
}
#[test]
fn retained_unknown_empty_host() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t, "t", r#"retained: {"host":""}"#).unwrap();
    assert!(
        !state::replay(&project(), &t, &basis(), Some("h"))
            .unwrap()
            .complete
    );
}
#[test]
fn retained_complete_host_resolution() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t, "t", r#"retained: {"host":"h","path":42}"#).unwrap();
    crate::commands::append_note(
        &mut t,
        "t",
        &format!("retention operation: {}", operation("intent", json!({}))),
    )
    .unwrap();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    let output = json!({"items":[4],"artifacts":[],"resolutions":[{"bad":{"basis":basis(),"ordinal":2},"successors":[4],"reason":"Complete declared host inventory"}]});
    crate::commands::append_note(
        &mut t,
        "t",
        &format!(
            "retention operation: {}",
            operation("items_published", output)
        ),
    )
    .unwrap();
    assert!(
        state::replay(&project(), &t, &basis(), Some("h"))
            .unwrap()
            .complete
    );
}
#[test]
fn retained_task_resolution_cannot_hide_other_hosts() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    let mut remote = record();
    remote["host"] = json!("other");
    remote["path"] = json!("/data/remote");
    crate::commands::append_note(&mut t, "t", &format!("retained: {remote}")).unwrap();
    crate::commands::append_note(&mut t, "t", r#"retained: {"version":2}"#).unwrap();
    crate::commands::append_note(
        &mut t,
        "t",
        &format!("retention operation: {}", operation("intent", json!({}))),
    )
    .unwrap();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    let output = json!({"items":[5],"artifacts":[],"resolutions":[{"bad":{"basis":basis(),"ordinal":3},"successors":[5],"reason":"Incomplete current-host declaration"}]});
    crate::commands::append_note(
        &mut t,
        "t",
        &format!(
            "retention operation: {}",
            operation("items_published", output)
        ),
    )
    .unwrap();
    assert!(
        !state::replay(&project(), &t, &basis(), Some("h"))
            .unwrap()
            .complete
    );
}

#[test]
fn retained_replay_rejects_missing_phase_items() {
    let mut t = task();
    crate::commands::append_note(&mut t, "t", &format!("retained: {}", record())).unwrap();
    for note in [
        operation("intent", json!({})),
        operation(
            "items_published",
            json!({"items":[999],"artifacts":[],"resolutions":[]}),
        ),
    ] {
        crate::commands::append_note(&mut t, "t", &format!("retention operation: {note}")).unwrap();
    }
    let state = state::replay(&project(), &t, &basis(), None).unwrap();
    assert!(
        !state.complete,
        "a phase cannot bind a missing retained source ordinal"
    );
    assert!(state.items[0].record.is_none());
}

fn project() -> crate::repo::Project {
    crate::repo::Project::from_config(std::path::PathBuf::from("/fixture"), "prefix = 'sci'")
        .unwrap()
}

#[test]
fn validate_record_splits_missing_scratch_manifest_from_invalid_locator() {
    let mut scratch = record();
    scratch["kind"] = json!("scratch");
    let err = decode_record(&scratch.to_string()).unwrap_err().to_string();
    assert!(err.contains("missing manifest"), "got: {err}");

    let mut scratch_null = scratch.clone();
    scratch_null["manifest"] = Value::Null;
    let err = decode_record(&scratch_null.to_string())
        .unwrap_err()
        .to_string();
    assert!(err.contains("missing manifest"), "got: {err}");

    for kind in ["scratch", "evidence"] {
        for bad_locator in ["files:foo.json", "rel/path.json"] {
            let mut r = record();
            r["kind"] = json!(kind);
            r["manifest"] = json!(bad_locator);
            let err = decode_record(&r.to_string()).unwrap_err().to_string();
            assert!(
                err.contains("manifest locator must start with / or $WORK_ROOT/"),
                "{kind} {bad_locator}: got {err}"
            );
        }
    }

    let mut evidence_null = record();
    evidence_null["manifest"] = Value::Null;
    let err = decode_record(&evidence_null.to_string())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("manifest locator must start with / or $WORK_ROOT/"),
        "evidence null manifest: got {err}"
    );

    for ok_locator in ["/records/manifest.json", "$WORK_ROOT/sci/manifest.json"] {
        let mut r = scratch.clone();
        r["manifest"] = json!(ok_locator);
        assert!(decode_record(&r.to_string()).is_ok(), "{ok_locator}");
    }

    let mut unknown = json!({
        "version": 1,
        "host": "h",
        "kind": "scratch",
        "path": "$WORK_ROOT/sci/dir",
        "state": "kept",
        "verification": "unknown",
        "purpose": "Review",
        "next_owner": "task:sci-000001",
        "cleanup_trigger": "After review",
        "sha256": null,
        "manifest": null,
        "verification_action": "Inspect missing directory"
    });
    assert!(decode_record(&unknown.to_string()).is_ok());

    unknown["manifest"] = json!("files:foo.json");
    let err = decode_record(&unknown.to_string()).unwrap_err().to_string();
    assert!(
        err.contains("unknown manifest locator must be null or start with / or $WORK_ROOT/"),
        "unknown bad manifest: got {err}"
    );
}

#[test]
fn decode_resolution_rejects_bad_ordinals() {
    let valid = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 3},
        "successors": [4],
        "reason": "fixed"
    });
    let parsed = decode_resolution(&valid.to_string()).expect("valid single successor");
    assert_eq!(
        parsed,
        ResolutionNote {
            version: 1,
            bad: Source {
                basis: basis(),
                ordinal: 3,
            },
            successors: vec![4],
            reason: "fixed".into(),
        }
    );

    let mut empty_succ = valid.clone();
    empty_succ["successors"] = json!([]);
    assert!(
        decode_resolution(&empty_succ.to_string()).is_ok(),
        "empty successors allowed"
    );

    let mut v2 = valid.clone();
    v2["version"] = json!(2);
    let err = decode_resolution(&v2.to_string()).unwrap_err().to_string();
    assert!(err.contains("unsupported_version"), "v2 full: got {err}");
    let err = decode_resolution(r#"{"version":2}"#)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unsupported_version"), "v2 minimal: got {err}");

    let mut zero_bad = valid.clone();
    zero_bad["bad"]["ordinal"] = json!(0);
    assert!(decode_resolution(&zero_bad.to_string()).is_err());

    for bad_succ in [json!([3]), json!([2]), json!([4, 4])] {
        let mut s = valid.clone();
        s["successors"] = bad_succ.clone();
        assert!(
            decode_resolution(&s.to_string()).is_err(),
            "bad successors {bad_succ} must fail"
        );
    }

    for bad_reason in ["", "line1\nline2", "line1\rline2", "line1\0line2"] {
        let mut s = valid.clone();
        s["reason"] = json!(bad_reason);
        assert!(
            decode_resolution(&s.to_string()).is_err(),
            "bad reason {bad_reason:?} must fail"
        );
    }

    let mut unknown_field = valid.clone();
    unknown_field["extra"] = json!(1);
    assert!(decode_resolution(&unknown_field.to_string()).is_err());

    let dup_key = valid.to_string().replacen("{", "{\"version\":1,", 1);
    assert!(decode_resolution(&dup_key).is_err());

    assert!(publish::reserved("retention resolution: {}"));
    assert!(publish::reserved("multipart: {}"));
    assert!(publish::validate_appended_note_syntax("Custody note: ok").is_ok());
    assert!(
        publish::validate_appended_note_syntax(&format!("retention resolution: {valid}")).is_ok()
    );
    assert!(publish::validate_appended_note_syntax("retention resolution:").is_err());
    assert!(publish::validate_appended_note_syntax("retention resolution: {}").is_err());
    assert!(publish::validate_appended_note_syntax("retained:no-space").is_err());
    assert!(publish::validate_appended_note_syntax("retained: [1, 2]").is_err());
}

#[test]
fn check_write_validates_appended_retained_when_prior_note_edited() {
    let p = project();
    let reg = crate::registry::Registry::default();
    let mut old = task();
    crate::commands::append_note(&mut old, "t", "initial").unwrap();
    old.notes[0].text = "initial ".into();

    let mut new = old.clone();
    new.notes[0].text = "initial".into();
    crate::commands::append_note(&mut new, "t", "retained: created scratch dir").unwrap();

    let err = publish::check_write(&p, &reg, &old, &new, publish::WritePermission::Ordinary)
        .expect_err("prose retained: note after prior-note trim must fail");
    match err {
        crate::error::Error::RetainedInput(msg) => {
            assert!(
                msg.contains(
                    "Note prefix 'retained:' is reserved for Retention-1 JSON item records"
                ),
                "got: {msg}"
            );
            assert!(msg.contains("Custody note: ..."), "got: {msg}");
        }
        other => panic!("expected Error::RetainedInput, got: {other:?}"),
    }
}

#[test]
fn check_write_preserves_trimmed_historical_malformed_retained_note() {
    let p = project();
    let reg = crate::registry::Registry::default();
    let mut old = task();
    crate::commands::append_note(&mut old, "t", "retained: historical bad prose").unwrap();
    old.notes[0].text = "retained: historical bad prose ".into();

    let mut new = old.clone();
    new.notes[0].text = "retained: historical bad prose".into();
    crate::commands::append_note(&mut new, "t", "Custody note: ok").unwrap();
    assert_eq!(old.notes.len(), 1);
    assert_eq!(new.notes.len(), 2);

    publish::check_write(&p, &reg, &old, &new, publish::WritePermission::Ordinary)
        .expect("existing historical malformed note at ..old.notes.len() must not be re-validated");

    let mut bad_new = old.clone();
    bad_new.notes[0].text = "retained: historical bad prose".into();
    crate::commands::append_note(&mut bad_new, "t", "retained: new bad prose").unwrap();
    let err = publish::check_write(&p, &reg, &old, &bad_new, publish::WritePermission::Ordinary)
        .expect_err("newly appended malformed retained: note must still fail");
    assert!(
        matches!(err, crate::error::Error::RetainedInput(_)),
        "expected Error::RetainedInput, got: {err:?}"
    );
}

const EXPECTED_RETAINED_ACTION: &str = "Recover exact history, append valid version: 1 'retained:' successor note(s) for each affected identity (or none when no identities exist; use <records>/tasks/files/<id>/<name> for an existing attached manifest, including on closed tasks without reopening), then append 'retention resolution: {\"version\":1,\"bad\":<source>,\"successors\":[...],\"reason\":\"...\"}' or pass resolutions to tasks retained checkpoint";
const EXPECTED_ENVELOPE_ACTION: &str =
    "Publish an explicit source-bound correction after recovering exact history";

#[test]
fn state_replay_standalone_resolution_scopes_and_rejections() {
    // 1. Diagnostic.action distinguishes retained: vs operation/checkpoint envelopes,
    //    and 0-item prose resolves via empty successors in standalone resolution.
    let mut t = task();
    crate::commands::append_note(&mut t, "t", "retained: accidental prose").unwrap();
    let s = state::replay(&project(), &t, &basis(), None).unwrap();
    assert!(!s.complete);
    assert_eq!(s.diagnostics[0].action, EXPECTED_RETAINED_ACTION);
    let res = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 1},
        "successors": [],
        "reason": "Accidental prose with zero retained items"
    });
    crate::commands::append_note(&mut t, "t", &format!("retention resolution: {res}")).unwrap();
    let s = state::replay(&project(), &t, &basis(), None).unwrap();
    assert!(s.complete, "{s:?}");
    assert_eq!(s.diagnostics[0].resolved_by.as_ref().unwrap().ordinal, 2);

    // 2. Released item diagnostic resolved via valid released successor + retention resolution.
    let mut t_rel = task();
    let bad_rel = json!({
        "version": 1,
        "host": "h",
        "kind": "evidence",
        "path": "/data/a",
        "state": "released",
        "verification": "verified",
        "purpose": "Released item",
        "next_owner": "task:sci-000001",
        "cleanup_trigger": "none"
    });
    crate::commands::append_note(&mut t_rel, "t", &format!("retained: {bad_rel}")).unwrap();
    let ok_rel = json!({
        "version": 1,
        "host": "h",
        "kind": "evidence",
        "path": "/data/a",
        "state": "released",
        "verification": "verified",
        "purpose": "Released item",
        "next_owner": "none",
        "cleanup_trigger": "none"
    });
    crate::commands::append_note(&mut t_rel, "t", &format!("retained: {ok_rel}")).unwrap();
    let res_rel = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 1},
        "successors": [2],
        "reason": "Fixed released next_owner"
    });
    crate::commands::append_note(&mut t_rel, "t", &format!("retention resolution: {res_rel}"))
        .unwrap();
    let s = state::replay(&project(), &t_rel, &basis(), None).unwrap();
    assert!(s.complete, "{s:?}");
    assert_eq!(s.items[0].record.as_ref().unwrap().state, "released");

    // 3. Other-host diagnostic resolved without false unmatched diagnostic under --host h.
    let mut t_other = task();
    crate::commands::append_note(&mut t_other, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(
        &mut t_other,
        "t",
        r#"retained: {"host":"other-host","path":42}"#,
    )
    .unwrap();
    let mut ok_other = record();
    ok_other["host"] = json!("other-host");
    ok_other["path"] = json!("/data/other");
    crate::commands::append_note(&mut t_other, "t", &format!("retained: {ok_other}")).unwrap();
    let res_other = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 2},
        "successors": [3],
        "reason": "Recovered other-host inventory"
    });
    crate::commands::append_note(
        &mut t_other,
        "t",
        &format!("retention resolution: {res_other}"),
    )
    .unwrap();
    let full = state::replay(&project(), &t_other, &basis(), None).unwrap();
    assert!(full.complete, "{full:?}");
    let local = state::replay(&project(), &t_other, &basis(), Some("h")).unwrap();
    assert!(local.complete, "{local:?}");
    assert!(local.diagnostics.is_empty(), "{:?}", local.diagnostics);

    // 4. Rejections in state::replay emit task-scoped diagnostics with envelope action.
    let mut t_bad_op = task();
    crate::commands::append_note(&mut t_bad_op, "t", "retention operation: {\"bad\":1}").unwrap();
    let s_op = state::replay(&project(), &t_bad_op, &basis(), None).unwrap();
    assert_eq!(s_op.diagnostics[0].action, EXPECTED_ENVELOPE_ACTION);
    let try_res_op = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 1},
        "successors": [],
        "reason": "Attempted to suppress operation envelope"
    });
    crate::commands::append_note(
        &mut t_bad_op,
        "t",
        &format!("retention resolution: {try_res_op}"),
    )
    .unwrap();
    let s_op = state::replay(&project(), &t_bad_op, &basis(), None).unwrap();
    assert!(!s_op.complete);
    assert_eq!(s_op.diagnostics.len(), 2);
    assert!(s_op.diagnostics[0].resolved_by.is_none());
    assert_eq!(s_op.diagnostics[1].scope, "task");
    assert_eq!(s_op.diagnostics[1].code, "malformed_record");
    assert_eq!(s_op.diagnostics[1].action, EXPECTED_ENVELOPE_ACTION);

    // Unsupported version on retention resolution: emits code == "unsupported_version".
    let mut t_v2 = task();
    crate::commands::append_note(&mut t_v2, "t", r#"retention resolution: {"version":2}"#).unwrap();
    let s_v2 = state::replay(&project(), &t_v2, &basis(), Some("h")).unwrap();
    assert!(!s_v2.complete);
    assert_eq!(s_v2.diagnostics[0].code, "unsupported_version");
    assert_eq!(s_v2.diagnostics[0].scope, "task");

    // Duplicate successor identities [4, 5] on a 2-identity host diagnostic fail coverage.
    let mut t_dup = task();
    let mut item_b = record();
    item_b["path"] = json!("/data/b");
    crate::commands::append_note(&mut t_dup, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t_dup, "t", &format!("retained: {item_b}")).unwrap();
    crate::commands::append_note(&mut t_dup, "t", r#"retained: {"host":"h","path":42}"#).unwrap();
    crate::commands::append_note(&mut t_dup, "t", &format!("retained: {}", record())).unwrap();
    crate::commands::append_note(&mut t_dup, "t", &format!("retained: {}", record())).unwrap();
    let dup_res = json!({
        "version": 1,
        "bad": {"basis": basis(), "ordinal": 3},
        "successors": [4, 5],
        "reason": "Duplicate item A twice instead of A and B"
    });
    crate::commands::append_note(&mut t_dup, "t", &format!("retention resolution: {dup_res}"))
        .unwrap();
    let s_dup = state::replay(&project(), &t_dup, &basis(), None).unwrap();
    assert!(!s_dup.complete);
    assert!(s_dup.diagnostics[0].resolved_by.is_none());
    assert_eq!(s_dup.diagnostics[1].source.as_ref().unwrap().ordinal, 6);
}

#[test]
fn proof_and_basis_reject_forged_resolution_bad_basis() {
    if crate::retained::read::tests::isolated(
        "retained::tests::proof_and_basis_reject_forged_resolution_bad_basis",
    ) {
        return;
    }
    let (dir, project) = crate::retained::read::tests::fixture();
    let id = crate::model::TaskId::parse("sci-000001").unwrap();
    let mut t = task();
    crate::commands::append_note(&mut t, "t", "Custody note: initial").unwrap();
    let write_task = |proj: &crate::repo::Project, task_val: &Task| {
        let raw = crate::format::serialize_physical(&proj.physical_for_save(task_val).unwrap());
        std::fs::write(proj.task_path(&task_val.id), raw).unwrap();
    };
    write_task(&project, &t);
    crate::retained::read::tests::git(dir.path(), &["add", "."]);
    crate::retained::read::tests::git(dir.path(), &["commit", "-qm", "initial task"]);
    let early = super::basis::read(&project, &id, None).unwrap().basis;

    crate::commands::append_note(&mut t, "t", "retained: accidental prose").unwrap();
    write_task(&project, &t);
    crate::retained::read::tests::git(dir.path(), &["add", "."]);
    crate::retained::read::tests::git(dir.path(), &["commit", "-qm", "bad retained"]);

    // Tamper a resolution note pointing bad.basis at `early` (before ordinal 2 existed).
    let forged = json!({
        "version": 1,
        "bad": {"basis": early, "ordinal": 2},
        "successors": [],
        "reason": "Forged earlier basis commit"
    });
    crate::commands::append_note(&mut t, "t", &format!("retention resolution: {forged}")).unwrap();
    write_task(&project, &t);
    crate::retained::read::tests::git(dir.path(), &["add", "."]);
    crate::retained::read::tests::git(dir.path(), &["commit", "-qm", "forged resolution"]);
    let head_commit = crate::retained::read::tests::git(dir.path(), &["rev-parse", "HEAD"]);

    // 1. `basis::read` rejects the forged `retention resolution:` with RetainedHistoryBlocked.
    let err = super::basis::read(&project, &id, None)
        .err()
        .expect("basis::read must reject forged bad.basis");
    assert!(
        matches!(err, crate::error::Error::RetainedHistoryBlocked(_)),
        "expected RetainedHistoryBlocked from basis::read, got: {err:?}"
    );

    // 2. `proof::validate_history` independently rejects the forged `retention resolution:`
    //    even when loaded via `basis::read_exact` (both unfiltered and host-filtered).
    let mut head_basis = early.clone();
    head_basis.commit = head_commit;
    let exact = super::basis::read_exact(&project, &id, &head_basis).unwrap();
    for filter in [None, Some("h")] {
        let err = super::proof::validate_history(&project, &exact, filter)
            .expect_err("proof::validate_history must reject forged bad.basis");
        assert!(
            matches!(err, crate::error::Error::RetainedHistoryBlocked(_)),
            "expected RetainedHistoryBlocked from proof::validate_history ({filter:?}), got: {err:?}"
        );
    }

    // 2b. `proof::validate_history(..., Some("h"))` also rejects a forged `bad.basis` inside
    //     an other-host `ItemsOutput.resolutions` entry when loaded via `basis::read_exact`.
    let (op_dir, op_proj) = crate::retained::read::tests::fixture();
    let mut op_t = task();
    crate::commands::append_note(&mut op_t, "t", "Custody note: initial").unwrap();
    write_task(&op_proj, &op_t);
    crate::retained::read::tests::git(op_dir.path(), &["add", "."]);
    crate::retained::read::tests::git(op_dir.path(), &["commit", "-qm", "initial op task"]);
    let op_early = super::basis::read(&op_proj, &id, None).unwrap().basis;

    crate::commands::append_note(&mut op_t, "t", "retained: accidental prose").unwrap();
    write_task(&op_proj, &op_t);
    crate::retained::read::tests::git(op_dir.path(), &["add", "."]);
    crate::retained::read::tests::git(op_dir.path(), &["commit", "-qm", "bad prose"]);

    let mut op_intent = operation("intent", json!({}));
    op_intent["host"] = json!("other-host");
    op_intent["input_bases"] = json!([{"task": "sci-000001", "basis": op_early}]);
    op_intent["request"]["basis"] = json!(op_early);
    let mut op_items = operation(
        "items_published",
        json!({
            "items": [],
            "artifacts": [],
            "resolutions": [{
                "bad": {"basis": op_early, "ordinal": 2},
                "successors": [],
                "reason": "Forged earlier basis commit on other-host ItemsOutput"
            }]
        }),
    );
    op_items["host"] = json!("other-host");
    op_items["input_bases"] = json!([{"task": "sci-000001", "basis": op_early}]);
    op_items["request"]["basis"] = json!(op_early);
    assert!(decode_operation(&op_items.to_string()).is_ok());

    crate::commands::append_note(&mut op_t, "t", &format!("retention operation: {op_intent}"))
        .unwrap();
    crate::commands::append_note(&mut op_t, "t", &format!("retention operation: {op_items}"))
        .unwrap();
    write_task(&op_proj, &op_t);
    crate::retained::read::tests::git(op_dir.path(), &["add", "."]);
    crate::retained::read::tests::git(
        op_dir.path(),
        &["commit", "-qm", "forged other-host items_published"],
    );
    let mut op_head_basis = op_early.clone();
    op_head_basis.commit = crate::retained::read::tests::git(op_dir.path(), &["rev-parse", "HEAD"]);
    let op_exact = super::basis::read_exact(&op_proj, &id, &op_head_basis).unwrap();
    let err = super::proof::validate_history(&op_proj, &op_exact, Some("h")).expect_err(
        "proof::validate_history(..., Some(\"h\")) must reject forged bad.basis on other-host ItemsOutput",
    );
    assert!(
        matches!(err, crate::error::Error::RetainedHistoryBlocked(_)),
        "expected RetainedHistoryBlocked for other-host ItemsOutput.resolutions, got: {err:?}"
    );

    // 3. `basis::read` checkpoint-gap guard rejects intervening `retention resolution: ` and `multipart: ` notes.
    for gap_note in [
        r#"retention resolution: {"not_decoded":1}"#,
        r#"multipart: {"not_decoded":1}"#,
    ] {
        let (gap_dir, gap_proj) = crate::retained::read::tests::fixture();
        let mut gap_t = task();
        crate::commands::append_note(&mut gap_t, "t", &format!("retained: {}", record())).unwrap();
        write_task(&gap_proj, &gap_t);
        crate::retained::read::tests::git(gap_dir.path(), &["add", "."]);
        crate::retained::read::tests::git(gap_dir.path(), &["commit", "-qm", "seed"]);
        let cp_basis = super::basis::read(&gap_proj, &id, None).unwrap().basis;

        crate::commands::append_note(&mut gap_t, "t", gap_note).unwrap();
        let cp_json = json!({
            "version": 1,
            "operation": {"coordinator": "sci-000001", "operation_id": "0".repeat(32)},
            "observed_at": "2026-11-03T00:00:00Z",
            "host": "h",
            "observer": null,
            "basis": cp_basis,
            "scope": {"mode": "all_known_kept_on_host", "host": "h", "other_hosts": "outside_observation"},
            "counts": {"kept_verified": 1, "kept_unknown": 0, "released": 0},
            "changed": 0,
            "actions": [],
            "complete": true
        });
        crate::commands::append_note(&mut gap_t, "t", &format!("retention checkpoint: {cp_json}"))
            .unwrap();
        write_task(&gap_proj, &gap_t);
        crate::retained::read::tests::git(gap_dir.path(), &["add", "."]);
        crate::retained::read::tests::git(gap_dir.path(), &["commit", "-qm", "gap cp"]);
        let err = super::basis::read(&gap_proj, &id, None)
            .err()
            .unwrap_or_else(|| panic!("gap note {gap_note} must block checkpoint basis"));
        assert!(
            matches!(err, crate::error::Error::RetainedHistoryBlocked(_)),
            "{gap_note}: got {err:?}"
        );
    }
}
