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
