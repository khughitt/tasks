mod common;
use common::{
    git,
    retained::{RetainedEnv, checkpoint, checkpoint_request, declaration},
};
use serde_json::json;

// Simulate a reviewed cutover before the command's preservation route is enabled.
fn seed_rename(f: &mut RetainedEnv, target: &str) -> (String, String) {
    let old_id = f.id.clone();
    let old_prefix = old_id.split('-').next().unwrap().to_string();
    let commit = f.basis();
    let basis = f.state()["basis"].clone();
    let from_subdir = basis["records_subdir"].as_str().unwrap();
    let config = f.records.join("tasks/.config.toml");
    let mut value: toml::Value =
        toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    let detached = basis["layout"] == "detached";
    let to_subdir = if detached { target } else { from_subdir };
    let edge = json!({"version":1,"from_prefix":old_prefix,"to_prefix":target,"layout":basis["layout"],"from_subdir":from_subdir,"to_subdir":to_subdir,"commit":commit,"retained_ref":"refs/heads/main"});
    let table = value.as_table_mut().unwrap();
    table
        .entry("retention_lineage")
        .or_insert(toml::Value::Array(vec![]))
        .as_array_mut()
        .unwrap()
        .push(toml::Value::try_from(edge).unwrap());
    table.insert("prefix".into(), toml::Value::String(target.into()));
    std::fs::write(&config, toml::to_string(&value).unwrap()).unwrap();
    let paths: Vec<_> = std::fs::read_dir(f.records.join("tasks"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "md"))
        .collect();
    for path in paths {
        let old = path.file_stem().unwrap().to_str().unwrap();
        let new = format!("{target}-{}", old.split_once('-').unwrap().1);
        let raw = std::fs::read_to_string(&path).unwrap().replacen(
            &format!("id: {old}\n"),
            &format!("id: {new}\n"),
            1,
        );
        std::fs::write(path.with_file_name(format!("{new}.md")), raw).unwrap();
        std::fs::remove_file(&path).unwrap();
        let files = f.records.join(format!("tasks/files/{old}"));
        if files.exists() {
            std::fs::rename(&files, files.with_file_name(&new)).unwrap();
        }
    }
    let git_root = if detached {
        f.records.parent().unwrap().to_path_buf()
    } else {
        f.records.clone()
    };
    if detached {
        let dest = f.records.with_file_name(target);
        std::fs::rename(&f.records, &dest).unwrap();
        f.records = dest;
    }
    git(&git_root, &["add", "-A"]);
    git(&git_root, &["commit", "-qm", "fixture rename"]);
    let registry = f.env.home.path().join(".config/tasks/projects.toml");
    let mut v: toml::Value = toml::from_str(&std::fs::read_to_string(&registry).unwrap()).unwrap();
    for key in ["projects", "records"] {
        if let Some(t) = v.get_mut(key).and_then(|v| v.as_table_mut()) {
            if let Some(old) = t.remove(&old_prefix) {
                t.insert(
                    target.into(),
                    if key == "records" {
                        toml::Value::String(f.records.to_str().unwrap().into())
                    } else {
                        old
                    },
                );
            }
        }
    }
    if let Some(aliases) = v.get_mut("aliases").and_then(|v| v.as_table_mut()) {
        for (_, value) in aliases.iter_mut() {
            if value.as_str() == Some(old_prefix.as_str()) {
                *value = toml::Value::String(target.into());
            }
        }
    }
    v.as_table_mut()
        .unwrap()
        .entry("aliases")
        .or_insert(toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap()
        .insert(old_prefix, toml::Value::String(target.into()));
    std::fs::write(registry, toml::to_string(&v).unwrap()).unwrap();
    f.id = format!("{target}-{}", old_id.split_once('-').unwrap().1);
    (old_id, commit)
}
fn observed(layout: &str) -> RetainedEnv {
    let f = RetainedEnv::new(layout);
    f.json(&["start", &f.id]);
    let payload = f.dir.join("payload");
    std::fs::write(&payload, "original").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&payload, "evidence")])),
    );
    f.json(&["park", &f.id, "rename fixture"]);
    f
}
#[test]
fn retention_rename_lineage_reads() {
    for layout in ["in-tree", "detached", "linked"] {
        let mut f = observed(layout);
        let before = f.state();
        let envelopes = before["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["note"].clone())
            .collect::<Vec<_>>();
        let (old, commit) = seed_rename(&mut f, "new");
        let current = f.state();
        assert_eq!(current["complete"], true, "{current}");
        assert_eq!(current["task"], f.id);
        assert_eq!(
            current["operations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["note"].clone())
                .collect::<Vec<_>>(),
            envelopes
        );
        let historical = f.json(&["retained", "state", &f.id, "--at", &commit]);
        assert_eq!(historical["task"], old);
        assert_eq!(historical["basis"]["commit"], commit);
        assert_eq!(
            historical["basis"]["task_path"],
            before["basis"]["task_path"]
        );
        seed_rename(&mut f, "third");
        assert_eq!(f.state()["complete"], true);
    }
}
#[test]
fn retention_rename_lineage_rejects() {
    for field in ["commit", "retained_ref", "from_subdir", "layout", "version"] {
        let mut f = observed("in-tree");
        seed_rename(&mut f, "new");
        let p = f.records.join("tasks/.config.toml");
        let mut v: toml::Value = toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["retention_lineage"][0][field] = match field {
            "commit" => toml::Value::String("0".repeat(40)),
            "version" => toml::Value::Integer(2),
            "layout" => toml::Value::String("detached".into()),
            _ => toml::Value::String("missing".into()),
        };
        std::fs::write(p, toml::to_string(&v).unwrap()).unwrap();
        let out = f.cmd().args(["retained", "state", &f.id]).output().unwrap();
        assert!(!out.status.success(), "accepted bad {field}: {out:?}");
    }
}
#[test]
fn retention_rename_lineage_publications() {
    let mut f = observed("in-tree");
    let before = f.state();
    seed_rename(&mut f, "new");
    let after = f.state();
    assert_eq!(after["complete"], true);
    for (a, b) in after["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(before["operations"].as_array().unwrap())
    {
        assert_eq!(a["note"], b["note"]);
    }
    // A missing original artifact must not be replaced by the live moved file.
    let op = before["operations"][0]["note"]["request"]["name"]
        .as_str()
        .unwrap();
    let path = f.records.join(format!("tasks/files/{}/{op}", f.id));
    std::fs::write(path, b"corrupt live copy").unwrap();
    assert_eq!(f.state()["complete"], true);
}

#[test]
fn retention_rename_lineage_reads_archives() {
    let mut f = RetainedEnv::new("in-tree");
    for i in 0..4 {
        let mut r = f.record();
        r["purpose"] = json!(format!("Review {i}"));
        f.note(r);
    }
    let preview = f.json(&["archive-notes", &f.id]);
    f.json(&[
        "archive-notes",
        &f.id,
        "--expect",
        preview["source_sha256"].as_str().unwrap(),
    ]);
    let before = f.state();
    let (_, commit) = seed_rename(&mut f, "new");
    assert_eq!(
        f.state()["items"][0]["record"],
        before["items"][0]["record"]
    );
    assert_eq!(
        f.json(&["retained", "state", &f.id, "--at", &commit])["items"][0]["source"]["ordinal"],
        before["items"][0]["source"]["ordinal"]
    );
}

#[test]
fn retention_rename_lineage_reconciled() {
    use common::retained::{host, transfer_fixture, transfer_request};
    let (mut f, to, file) = transfer_fixture();
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
    assert_eq!(out.status.code(), Some(86));
    std::fs::remove_file(file).unwrap();
    let request = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":"a".repeat(32)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]},{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}],"reason":"Keep both custodies"});
    let path = f.request("reconcile.json", &request);
    f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    f.json(&["park", &f.id, "rename"]);
    f.json(&["park", &to, "rename"]);
    seed_rename(&mut f, "new");
    assert_eq!(f.state()["complete"], true);
    let receiver = format!("new-{}", to.split_once('-').unwrap().1);
    assert_eq!(f.json(&["retained", "state", &receiver])["complete"], true);
    let _ = host();
}

#[test]
fn retention_rename_operations_custody() {
    use common::retained::{transfer, transfer_request};
    let mut f = observed("in-tree");
    let old = f.id.clone();
    seed_rename(&mut f, "new");
    assert_eq!(
        f.state()["items"][0]["record"]["next_owner"],
        format!("task:{old}")
    );
    let to = f.json(&["add", "Receiver"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &f.id]);
    f.json(&["start", &to]);
    let request = transfer_request(&f, &to);
    let result = transfer(&f, &to, &request);
    assert_eq!(result["outcome"], "transferred");
    assert_eq!(f.state()["items"][0]["record"]["state"], "released");
}
#[test]
fn retention_rename_operations_manifest() {
    for layout in ["in-tree", "detached", "linked"] {
        let mut f = RetainedEnv::new(layout);
        f.json(&["start", &f.id]);
        let a = f.dir.join("scratch");
        let b = f.dir.join("evidence");
        for d in [&a, &b] {
            std::fs::create_dir(d).unwrap();
            std::fs::write(d.join("member"), "exact").unwrap();
        }
        checkpoint(
            &f,
            &checkpoint_request(
                &f,
                1,
                json!([declaration(&a, "scratch"), declaration(&b, "evidence")]),
            ),
        );
        f.json(&["park", &f.id, "rename"]);
        let before = f.state();
        let old_path =
            std::path::PathBuf::from(before["items"][0]["record"]["manifest"].as_str().unwrap());
        let bytes = std::fs::read(&old_path).unwrap();
        seed_rename(&mut f, "new");
        assert!(!old_path.exists());
        let new_path = f.records.join(format!(
            "tasks/files/{}/{}",
            f.id,
            old_path.file_name().unwrap().to_str().unwrap()
        ));
        assert_eq!(std::fs::read(&new_path).unwrap(), bytes);
        f.json(&["start", &f.id]);
        let result = checkpoint(&f, &checkpoint_request(&f, 2, json!([])));
        assert_eq!(result["outcome"], "verified", "{result}");
        let after = f.state();
        assert_eq!(after["complete"], true);
        for i in 0..2 {
            assert_eq!(
                after["items"][i]["record"]["path"],
                before["items"][i]["record"]["path"]
            );
            assert_eq!(
                after["items"][i]["record"]["next_owner"],
                before["items"][i]["record"]["next_owner"]
            );
        }
    }
}
#[test]
fn retention_rename_operations_retry() {
    let mut f = observed("in-tree");
    let request = std::fs::read(f.dir.join("request.json")).unwrap();
    f.json(&["start", &f.id]);
    let original = f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        f.dir.join("request.json").to_str().unwrap(),
    ]);
    f.json(&["park", &f.id, "rename"]);
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    std::fs::remove_file(f.dir.join("payload")).unwrap();
    let before = f.basis();
    let path = f.dir.join("retry.json");
    std::fs::write(&path, &request).unwrap();
    let result = f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(result["publications"], original["publications"]);
    assert_eq!(result["counts"], original["counts"]);
    assert_eq!(f.basis(), before);
    assert!(result["warnings"].as_array().unwrap().iter().any(|v|v=="Completed operation replay; current filesystem bytes were not inspected"));
    let mut changed: serde_json::Value = serde_json::from_slice(&request).unwrap();
    changed["declarations"][0]["purpose"] = json!("Changed");
    std::fs::write(&path, changed.to_string()).unwrap();
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
}

#[test]
fn retention_rename_operations_cross_project_retry() {
    use common::retained::{cross_receiver, transfer, transfer_request};
    let mut f = observed("in-tree");
    f.json(&["start", &f.id]);
    let (other, to) = cross_receiver(&f, false);
    f.json(&["-C", other.to_str().unwrap(), "start", &to]);
    let request = transfer_request(&f, &to);
    let original = transfer(&f, &to, &request);
    f.json(&["park", &f.id, "rename"]);
    f.json(&["-C", other.to_str().unwrap(), "park", &to, "rename"]);
    let saved = (f.dir.clone(), f.records.clone(), f.id.clone());
    f.dir = other.clone();
    f.records = other;
    f.id = to;
    seed_rename(&mut f, "other");
    let new_to = f.id.clone();
    let other = f.dir.clone();
    (f.dir, f.records, f.id) = saved;
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    f.json(&["-C", other.to_str().unwrap(), "start", &new_to]);
    let result = transfer(&f, &new_to, &request);
    assert_eq!(result["publications"], original["publications"]);
    assert_eq!(result["counts"], original["counts"]);
}
#[test]
fn retention_rename_operations_missing_manifest_refuses() {
    let mut f = RetainedEnv::new("detached");
    f.json(&["start", &f.id]);
    let directory = f.dir.join("scratch");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("file"), "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&directory, "scratch")])),
    );
    let manifest = f.state()["items"][0]["record"]["manifest"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["park", &f.id, "rename"]);
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    let target = f.records.join(format!(
        "tasks/files/{}/{}",
        f.id,
        std::path::Path::new(&manifest)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
    ));
    std::fs::write(target, b"corrupt").unwrap();
    let request = checkpoint_request(&f, 2, json!([]));
    let path = f.request("corrupt.json", &request);
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
fn retention_rename_operations_reconcile_carried_custody() {
    use common::retained::{host, transfer_request};
    let mut f = observed("in-tree");
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    let to = f.json(&["add", "Receiver"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &to]);
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
    assert_eq!(out.status.code(), Some(86));
    std::fs::remove_file(f.dir.join("payload")).unwrap();
    let request = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":"a".repeat(32)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]},{"task":to,"basis":f.json(&["retained","state",&to])["basis"]}],"reason":"Keep custody after loss"});
    let path = f.request("reconcile.json", &request);
    let original = f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(original["outcome"], "reconciled");
    f.json(&["park", &f.id, "rename"]);
    f.json(&["park", &to, "rename"]);
    seed_rename(&mut f, "third");
    let to = format!("third-{}", to.split_once('-').unwrap().1);
    f.json(&["start", &f.id]);
    f.json(&["start", &to]);
    let replay = f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(replay["publications"], original["publications"]);
    assert_eq!(replay["counts"], original["counts"]);
    let _ = host();
}

#[test]
fn retention_rename_operations_external_manifest_is_not_aliased() {
    let mut f = RetainedEnv::new("in-tree");
    f.json(&["start", &f.id]);
    let directory = f.dir.join("scratch");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("file"), "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&directory, "scratch")])),
    );
    let mut record = f.state()["items"][0]["record"].clone();
    let name = std::path::Path::new(record["manifest"].as_str().unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    record["manifest"] = json!(
        f.dir
            .join(format!("external/tasks/files/{}/{name}", f.id))
            .to_str()
            .unwrap()
    );
    f.note(record);
    f.json(&["park", &f.id, "rename"]);
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    assert!(
        f.records
            .join(format!("tasks/files/{}/{name}", f.id))
            .exists()
    );
    let result = checkpoint(&f, &checkpoint_request(&f, 2, json!([])));
    assert_eq!(result["outcome"], "accounted_unknown");
}
