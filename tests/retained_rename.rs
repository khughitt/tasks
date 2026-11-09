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
        if let Some(t) = v.get_mut(key).and_then(|v| v.as_table_mut())
            && let Some(old) = t.remove(&old_prefix)
        {
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

fn renamed_live(f: &mut RetainedEnv, target: &str) {
    let detached = f.state()["basis"]["layout"] == "detached";
    let prefix = f.id.split('-').next().unwrap().to_string();
    f.json(&["rename", &prefix, target]);
    f.id = format!("{target}-{}", f.id.split_once('-').unwrap().1);
    if detached {
        f.records = f.records.with_file_name(target);
    }
}
fn commit_rename(f: &RetainedEnv, source: &str) {
    let detached = std::fs::read_to_string(f.records.join("tasks/.config.toml"))
        .unwrap()
        .contains("kind = \"detached\"");
    if detached {
        git(
            f.records.parent().unwrap(),
            &[
                "add",
                "--all",
                "--",
                source,
                f.records.file_name().unwrap().to_str().unwrap(),
            ],
        );
    } else {
        git(&f.records, &["add", "--all", "--", "tasks"]);
    }
    let out = common::git_fixture::command(f.env.home.path(), &f.records)
        .args(["commit", "-qm", "retention rename"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn retention_rename_command_hook() {
    for layout in ["in-tree", "detached", "linked"] {
        let mut f = observed(layout);
        let original = f.basis();
        let old = f.id.clone();
        let bytes = std::fs::read(f.records.join(format!("tasks/{old}.md"))).unwrap();
        // The hook follows the stable code checkout, not the detached records directory being moved.
        f.check_hook();
        renamed_live(&mut f, "new");
        assert_eq!(f.basis(), original);
        let config: toml::Value =
            toml::from_str(&std::fs::read_to_string(f.records.join("tasks/.config.toml")).unwrap())
                .unwrap();
        assert_eq!(
            config["retention_lineage"][0]["commit"].as_str(),
            Some(original.as_str())
        );
        let check = f.cmd().args(["check"]).output().unwrap();
        assert!(
            check.status.success(),
            "{layout}: {}",
            String::from_utf8_lossy(&check.stdout)
        );
        commit_rename(&f, "sci");
        assert_eq!(f.state()["complete"], true);
        let basis = f.json(&["retained", "state", &f.id, "--at", &original]);
        assert_eq!(basis["task"], old);
        let sub = if layout == "detached" { "sci/" } else { "" };
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args(["show", &format!("{original}:{sub}tasks/{old}.md")])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, bytes);
    }
}
#[test]
fn retention_rename_command_dirty_config_refuses_before_intent() {
    for staged in [false, true] {
        let f = observed("in-tree");
        let p = f.records.join("tasks/.config.toml");
        let mut config = std::fs::read_to_string(&p).unwrap();
        config.push_str("\n# dirty source config\n");
        std::fs::write(p, config).unwrap();
        if staged {
            git(&f.records, &["add", "tasks/.config.toml"]);
        }
        let before = f.basis();
        let registry = f.env.home.path().join(".config/tasks/projects.toml");
        let registry_bytes = std::fs::read(&registry).unwrap();
        assert!(
            !f.cmd()
                .args(["rename", "sci", "new"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(registry).unwrap(), registry_bytes);
        assert!(
            !f.env
                .home
                .path()
                .join(".local/state/tasks/rename/sci.toml")
                .exists()
        );
        assert!(f.records.join(format!("tasks/{}.md", f.id)).exists());
    }
}

#[test]
fn retention_rename_command_recovery() {
    for layout in ["in-tree", "detached"] {
        for boundary in [
            "inventory",
            "file:0",
            "attachments",
            "files",
            "config",
            "records",
            "registry",
            "store",
            "claims",
        ] {
            if layout != "detached" && boundary == "records" {
                continue;
            }
            let mut f = observed(layout);
            let before = f.basis();
            let old = f.id.clone();
            let body = std::fs::read_to_string(f.records.join(format!("tasks/{old}.md")))
                .unwrap()
                .split_once("\n---\n")
                .unwrap()
                .1
                .to_string();
            let files = std::fs::read_dir(f.records.join(format!("tasks/files/{old}")))
                .unwrap()
                .map(|e| {
                    let p = e.unwrap().path();
                    (
                        p.file_name().unwrap().to_os_string(),
                        std::fs::read(&p).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let stop = if boundary == "attachments" {
                format!("attachments:{}", old.split_once('-').unwrap().1)
            } else {
                boundary.into()
            };
            let out = f
                .cmd()
                .env("TASKS_RENAME_STOP_AFTER", stop)
                .args(["rename", "sci", "new"])
                .output()
                .unwrap();
            assert!(out.status.success(), "{layout}/{boundary}: {out:?}");
            assert!(
                f.env
                    .home
                    .path()
                    .join(".local/state/tasks/rename/sci.toml")
                    .exists()
            );
            f.json(&["rename", "sci", "new"]);
            f.id = format!("new-{}", old.split_once('-').unwrap().1);
            if layout == "detached" {
                f.records = f.records.with_file_name("new");
            }
            let config: toml::Value = toml::from_str(
                &std::fs::read_to_string(f.records.join("tasks/.config.toml")).unwrap(),
            )
            .unwrap();
            assert_eq!(config["retention_lineage"].as_array().unwrap().len(), 1);
            assert_eq!(
                config["retention_lineage"][0]["commit"].as_str(),
                Some(before.as_str())
            );
            let raw =
                std::fs::read_to_string(f.records.join(format!("tasks/{}.md", f.id))).unwrap();
            assert_eq!(raw.split_once("\n---\n").unwrap().1, body);
            for (name, bytes) in files {
                assert_eq!(
                    std::fs::read(f.records.join(format!("tasks/files/{}", f.id)).join(name))
                        .unwrap(),
                    bytes
                );
            }
            assert!(f.cmd().args(["check"]).output().unwrap().status.success());
            commit_rename(&f, "sci");
            assert_eq!(f.state()["complete"], true);
        }
    }
}
#[test]
fn retention_rename_command_repeat() {
    let mut f = observed("detached");
    renamed_live(&mut f, "new");
    commit_rename(&f, "sci");
    renamed_live(&mut f, "third");
    commit_rename(&f, "new");
    assert_eq!(f.state()["complete"], true);
    let config: toml::Value =
        toml::from_str(&std::fs::read_to_string(f.records.join("tasks/.config.toml")).unwrap())
            .unwrap();
    assert_eq!(config["retention_lineage"].as_array().unwrap().len(), 2);
}
#[test]
fn retention_rename_command_corrupt_payload_refuses() {
    let f = observed("in-tree");
    f.cmd()
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "new"])
        .assert()
        .success();
    let path = f.env.home.path().join(".local/state/tasks/rename/sci.toml");
    let mut v: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    v["config_payload"] = toml::Value::String("prefix = 'other'".into());
    std::fs::write(&path, toml::to_string(&v).unwrap()).unwrap();
    assert!(
        !f.cmd()
            .args(["rename", "sci", "new"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(f.records.join(format!("tasks/{}.md", f.id)).exists());
    assert!(path.exists());
}

#[test]
fn retention_rename_command_missing_or_malformed_lineage_refuses() {
    use sha2::{Digest, Sha256};
    for mutation in ["missing", "empty", "malformed"] {
        let f = observed("in-tree");
        f.cmd()
            .env("TASKS_RENAME_STOP_AFTER", "inventory")
            .args(["rename", "sci", "new"])
            .assert()
            .success();
        let path = f.env.home.path().join(".local/state/tasks/rename/sci.toml");
        let mut inventory: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut config: toml::Value =
            toml::from_str(inventory["config_payload"].as_str().unwrap()).unwrap();
        match mutation {
            "missing" => {
                config.as_table_mut().unwrap().remove("retention_lineage");
            }
            "empty" => {
                config["retention_lineage"] = toml::Value::Array(vec![]);
            }
            _ => {
                config["retention_lineage"] = "malformed".into();
            }
        }
        let payload = toml::to_string(&config).unwrap();
        inventory["config_to"] = format!("{:x}", Sha256::digest(payload.as_bytes())).into();
        inventory["config_payload"] = payload.into();
        let tampered = toml::to_string(&inventory).unwrap();
        std::fs::write(&path, &tampered).unwrap();
        let task_path = f.records.join(format!("tasks/{}.md", f.id));
        let before = std::fs::read(&task_path).unwrap();
        let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();
        assert!(!out.status.success(), "{mutation}: {out:?}");
        assert_eq!(std::fs::read(task_path).unwrap(), before);
        assert_eq!(std::fs::read_to_string(path).unwrap(), tampered);
    }
}

#[test]
fn retention_rename_command_adopt() {
    for layout in ["in-tree", "detached"] {
        let mut f = observed(layout);
        let second = common::TestEnv::new();
        let records_root = f.records.parent().unwrap().to_path_buf();
        let mut args = vec!["init", "--prefix", "sci", "--force"];
        if layout == "detached" {
            args.extend(["--records", records_root.to_str().unwrap()]);
        }
        second.json(&f.dir, &args);
        renamed_live(&mut f, "new");
        commit_rename(&f, "sci");
        let before = f.basis();
        let config = std::fs::read(f.records.join("tasks/.config.toml")).unwrap();
        let registry = second.home.path().join(".config/tasks/projects.toml");
        let v: toml::Value = toml::from_str(&std::fs::read_to_string(&registry).unwrap()).unwrap();
        assert!(v.get("aliases").and_then(|v| v.get("sci")).is_none());
        second.json(&f.dir, &["rename", "sci", "new", "--adopt"]);
        assert_eq!(f.basis(), before);
        assert_eq!(
            std::fs::read(f.records.join("tasks/.config.toml")).unwrap(),
            config
        );
        assert_eq!(
            second.json(&f.dir, &["retained", "state", &f.id])["complete"],
            true
        );
    }
}
#[test]
fn retention_rename_command_pending_or_malformed_refuses() {
    for malformed in [false, true] {
        let f = observed("in-tree");
        if malformed {
            f.seed_note("retention operation: {\"phase\":\"reconciled\"}");
        } else {
            f.json(&["start", &f.id]);
            let request = checkpoint_request(&f, 2, json!([]));
            let path = f.request("pending.json", &request);
            let out = f
                .cmd()
                .env("TASKS_RETAINED_STOP_AFTER", "intent")
                .args([
                    "retained",
                    "checkpoint",
                    &f.id,
                    "--request",
                    path.to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(86));
            f.json(&["park", &f.id, "recover"]);
        }
        let before = f.basis();
        assert!(
            !f.cmd()
                .args(["rename", "sci", "new"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(f.basis(), before);
        assert!(
            !f.env
                .home
                .path()
                .join(".local/state/tasks/rename/sci.toml")
                .exists()
        );
    }
}
// Plain retained: item diagnostics survive a rename byte for byte, so they never block it;
// a malformed checkpoint envelope still does.
#[test]
fn retention_rename_command_plain_item_diagnostics_do_not_block() {
    for layout in ["in-tree", "detached", "linked"] {
        let mut f = observed(layout);
        f.seed_note(
            r#"retained: {"schema":1,"id":"wt-x","kind":"worktree","state":"released","host":"host-a","path":"/abs/wt-x","created":"2026-11-05T16:38:00Z"}"#,
        );
        f.seed_note(
            r#"retained: {"version":1,"host":"host-a","kind":"worktree","path":".worktrees/x","state":"released","verification":"verified","purpose":"merged","next_owner":"none","cleanup_trigger":"after merge"}"#,
        );
        let before = f.state();
        assert_eq!(before["complete"], false, "{layout}: {before}");
        assert_eq!(before["diagnostics"].as_array().unwrap().len(), 2);
        renamed_live(&mut f, "new");
        let check = f.cmd().args(["check"]).output().unwrap();
        assert!(
            check.status.success(),
            "{layout}: {}",
            String::from_utf8_lossy(&check.stdout)
        );
        commit_rename(&f, "sci");
        let after = f.state();
        assert_eq!(after["task"], f.id);
        let summary = |v: &serde_json::Value| {
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| {
                    (
                        d["code"].clone(),
                        d["source"]["ordinal"].clone(),
                        d["detail"].clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(summary(&after), summary(&before), "{layout}");
        assert_eq!(after["operations"].as_array().unwrap().len(), 3);
        let check = f.cmd().args(["check"]).output().unwrap();
        assert!(
            check.status.success(),
            "{layout}: {}",
            String::from_utf8_lossy(&check.stdout)
        );
    }
    // A newer-version item note means an older reader is renaming: refuse, as for a bad envelope.
    for note in [
        r#"retention checkpoint: {"version":1}"#,
        r#"retained: {"version":2,"host":"host-a","kind":"worktree","path":".worktrees/x","state":"kept"}"#,
    ] {
        let f = observed("in-tree");
        f.seed_note(note);
        let before = f.basis();
        let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();
        assert!(!out.status.success(), "{note}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("incomplete retention history"),
            "{note}: {out:?}"
        );
        assert_eq!(f.basis(), before);
    }
}
#[test]
fn retention_rename_command_bad_live_protected_bytes_refuse() {
    let f = observed("in-tree");
    let file = std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
        .unwrap()
        .map(|v| v.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".request.json")
        })
        .unwrap();
    std::fs::write(file, b"{}").unwrap();
    git(&f.records, &["add", "tasks"]);
    git(&f.records, &["commit", "-qm", "corrupt current attachment"]);
    assert!(
        !f.cmd()
            .args(["rename", "sci", "new"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !f.env
            .home
            .path()
            .join(".local/state/tasks/rename/sci.toml")
            .exists()
    );
}
#[test]
fn retention_rename_command_nongit_boundary() {
    for protected in [false, true] {
        let mut env = common::TestEnv::new();
        let dir = env.init("sci");
        let id = env.json(&dir, &["add", "Fixture"])["id"]
            .as_str()
            .unwrap()
            .to_string();
        if protected {
            let p = dir.join("manifest");
            std::fs::write(&p, "opaque").unwrap();
            env.json(
                &dir,
                &[
                    "attach",
                    &id,
                    p.to_str().unwrap(),
                    "--name",
                    "retained-manifest-a.json",
                ],
            );
        }
        let out = env
            .cmd(&dir)
            .args(["rename", "sci", "new"])
            .output()
            .unwrap();
        assert_eq!(out.status.success(), !protected, "{out:?}");
    }
}

#[test]
fn retention_rename_command_edge_safety() {
    let f = observed("in-tree");
    f.cmd()
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "sci", "new"])
        .assert()
        .success();
    let file = std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
        .unwrap()
        .map(|v| v.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".request.json")
        })
        .unwrap();
    std::fs::write(file, b"{}").unwrap();
    assert!(
        !f.cmd()
            .args(["rename", "sci", "new"])
            .output()
            .unwrap()
            .status
            .success(),
        "resume must verify protected bytes before moving them"
    );
}
#[test]
fn retention_rename_command_git_root_boundary() {
    let f = RetainedEnv::new("detached");
    git(&f.records, &["init", "-q", "-b", "main"]);
    git(&f.records, &["add", "tasks"]);
    git(&f.records, &["commit", "-qm", "owned records git root"]);
    f.json(&["start", &f.id]);
    let file = f.dir.join("payload");
    std::fs::write(&file, "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    f.json(&["park", &f.id, "rename"]);
    let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();

    assert!(
        !out.status.success(),
        "records Git root relocation needs an explicit route"
    );
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "retained_publication_unsupported");
    assert!(f.records.exists());
}

#[test]
fn retention_rename_command_ready_manifest_is_pinned() {
    let mut f = RetainedEnv::new("detached");
    f.json(&["start", &f.id]);
    let directory = f.dir.join("scratch");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("member"), "payload").unwrap();
    let request = checkpoint_request(&f, 1, json!([declaration(&directory, "scratch")]));
    let path = f.request("intent.json", &request);
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "intent")
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(86));
    let request = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":format!("{:032x}",1)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]}],"reason":"Keep declared directory"});
    let path = f.request("reconcile.json", &request);
    f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    f.json(&["park", &f.id, "rename"]);
    let manifest = f.state()["items"][0]["record"]["manifest"]
        .as_str()
        .unwrap()
        .to_string();
    let name = std::path::Path::new(&manifest)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let old = f.id.clone();
    let commit = f.basis();
    renamed_live(&mut f, "new");
    commit_rename(&f, "sci");
    assert_eq!(f.state()["complete"], true);
    let git = |args: &[&str]| {
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let oid = git(&[
        "rev-parse",
        &format!("{commit}:sci/tasks/files/{old}/{name}"),
    ]);
    let dir = std::path::PathBuf::from(git(&["rev-parse", "--absolute-git-dir"]));
    common::git_fixture::assert_repo(f.env.home.path(), f.records.parent().unwrap());
    assert!(dir.starts_with(f.records.parent().unwrap()));
    let object = dir.join(format!("objects/{}/{}", &oid[..2], &oid[2..]));
    assert!(object.is_file());
    std::fs::remove_file(object).unwrap();
    assert!(
        !f.cmd()
            .args(["retained", "state", &f.id])
            .output()
            .unwrap()
            .status
            .success(),
        "missing original ready manifest cannot be replaced by the working attachment"
    );
}

#[test]
fn retention_rename_command_config_version_is_strict() {
    let mut f = observed("in-tree");
    renamed_live(&mut f, "new");
    commit_rename(&f, "sci");
    let p = f.records.join("tasks/.config.toml");
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    for (field, value) in [
        ("version", toml::Value::Integer(2)),
        ("to_prefix", toml::Value::String("other".into())),
        ("to_subdir", toml::Value::String("../escape".into())),
    ] {
        let mut changed = config.clone();
        changed["retention_lineage"][0][field] = value;
        std::fs::write(&p, toml::to_string(&changed).unwrap()).unwrap();
        let out = f.cmd().args(["list"]).output().unwrap();
        assert!(
            !out.status.success(),
            "list accepted invalid lineage {field}"
        );
    }
}

#[test]
fn retention_rename_lineage_source_config_is_strict() {
    let mut f = observed("linked");
    let p = f.records.join("tasks/.config.toml");
    let valid = std::fs::read_to_string(&p).unwrap();
    let mut bad: toml::Value = toml::from_str(&valid).unwrap();
    bad["checkout"] = toml::Value::Integer(7);
    std::fs::write(&p, toml::to_string(&bad).unwrap()).unwrap();
    git(&f.records, &["add", "tasks/.config.toml"]);
    git(
        &f.records,
        &["commit", "-qm", "invalid historical source config"],
    );
    // A fabricated lineage must not bless an invalid committed source through a valid live config.
    std::fs::write(&p, valid).unwrap();
    seed_rename(&mut f, "new");
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
fn retention_rename_operations_current_branch_ref() {
    let mut f = observed("in-tree");
    seed_rename(&mut f, "new");
    f.json(&["start", &f.id]);
    git(&f.records, &["branch", "a-preserved"]);
    let result = checkpoint(&f, &checkpoint_request(&f, 2, json!([])));
    assert_eq!(result["outcome"], "verified");
    assert_eq!(f.state()["basis"]["retained_ref"], "refs/heads/main");
}

#[test]
fn retention_rename_command_pilot() {
    let mut f = RetainedEnv::new("detached");
    println!(
        "pilot executable: {}",
        std::path::Path::new(f.cmd().get_program())
            .canonicalize()
            .unwrap()
            .display()
    );
    let version = f.cmd().arg("--version").output().unwrap();
    assert!(version.status.success());
    println!(
        "pilot version: {}",
        String::from_utf8_lossy(&version.stdout).trim()
    );
    f.json(&["start", &f.id]);
    let file = f.dir.join("payload");
    std::fs::write(&file, "preserved").unwrap();
    let request = checkpoint_request(&f, 1, json!([declaration(&file, "evidence")]));
    let old = f.request("original.json", &request);
    let result = f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        old.to_str().unwrap(),
    ]);
    assert_eq!(result["outcome"], "verified");
    f.json(&["park", &f.id, "rename"]);
    f.check_hook();
    renamed_live(&mut f, "new");
    commit_rename(&f, "sci");
    assert_eq!(f.state()["complete"], true);
    f.json(&["start", &f.id]);
    assert_eq!(
        checkpoint(&f, &checkpoint_request(&f, 2, json!([])))["outcome"],
        "verified"
    );
    let before = f.basis();
    let replay = f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        old.to_str().unwrap(),
    ]);
    assert_eq!(replay["publications"], result["publications"]);
    assert_eq!(replay["counts"], result["counts"]);
    assert_eq!(f.basis(), before);
    println!("pilot verdict: passed");
}

fn cross_reconciled(f: &RetainedEnv, to: &str) -> (serde_json::Value, serde_json::Value) {
    use common::retained::transfer_request;
    let path = f.request("transfer.json", &transfer_request(f, to));
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "receiver_published")
        .args([
            "retained",
            "transfer",
            &f.id,
            to,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(86));
    std::fs::remove_file(f.dir.join("payload")).unwrap();
    let request = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":"a".repeat(32)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]},{"task":to,"basis":f.json(&["retained","state",to])["basis"]}],"reason":"Keep both custodies"});
    let path = f.request("reconcile.json", &request);
    let result = f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    (request, result)
}

#[test]
fn retention_rename_review_receiver_reconcile_retry() {
    use common::retained::cross_receiver;
    let mut f = observed("in-tree");
    f.json(&["start", &f.id]);
    let (other, to) = cross_receiver(&f, false);
    let (request, original) = cross_reconciled(&f, &to);
    f.json(&["park", &f.id, "rename"]);
    f.json(&["park", &to, "rename"]);
    let saved = (f.dir.clone(), f.records.clone(), f.id.clone());
    f.dir = other.clone();
    f.records = other;
    f.id = to;
    renamed_live(&mut f, "other");
    commit_rename(&f, "fam");
    let new_to = f.id.clone();
    (f.dir, f.records, f.id) = saved;
    f.json(&["start", &f.id]);
    f.json(&["start", &new_to]);
    let before = f.basis();
    let receiver_before = f.json(&["retained", "state", &new_to])["basis"].clone();
    let path = f.request("reconcile-retry.json", &request);
    let replay = f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(replay["publications"], original["publications"]);
    assert_eq!(replay["counts"], original["counts"]);
    assert_eq!(f.basis(), before);
    assert_eq!(
        f.json(&["retained", "state", &new_to])["basis"],
        receiver_before
    );
}

#[test]
fn retention_rename_review_cross_project_adopt() {
    use common::retained::{cross_receiver, transfer, transfer_request};
    for reconciled in [false, true] {
        for layout in ["in-tree", "detached"] {
            let mut f = observed(layout);
            f.json(&["start", &f.id]);
            let (other, to) = cross_receiver(&f, false);
            if reconciled {
                cross_reconciled(&f, &to);
            } else {
                transfer(&f, &to, &transfer_request(&f, &to));
            }
            f.json(&["park", &f.id, "rename"]);
            f.json(&["park", &to, "rename"]);
            let second = common::TestEnv::new();
            let registry = second.home.path().join(".config/tasks/projects.toml");
            std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
            std::fs::copy(
                f.env.home.path().join(".config/tasks/projects.toml"),
                &registry,
            )
            .unwrap();
            renamed_live(&mut f, "new");
            commit_rename(&f, "sci");
            let before = f.basis();
            let config = std::fs::read(f.records.join("tasks/.config.toml")).unwrap();
            second.json(&f.dir, &["rename", "sci", "new", "--adopt"]);
            assert_eq!(f.basis(), before);
            assert_eq!(
                std::fs::read(f.records.join("tasks/.config.toml")).unwrap(),
                config
            );
            assert_eq!(
                second.json(&f.dir, &["retained", "state", &f.id])["complete"],
                true
            );
            assert_eq!(
                second.json(&other, &["retained", "state", &to])["complete"],
                true
            );
        }
    }
}

#[test]
fn retention_rename_review_unledgered_protected_refuses() {
    for history in [false, true] {
        for name in [
            "retained-op-orphan.request.json",
            "retained-manifest-orphan.json",
            "retained-checkpoint-orphan.json",
        ] {
            let f = if history {
                observed("in-tree")
            } else {
                RetainedEnv::new("in-tree")
            };
            let dir = f.records.join(format!("tasks/files/{}", f.id));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(name), "uncertain evidence").unwrap();
            git(&f.records, &["add", "tasks"]);
            git(&f.records, &["commit", "-qm", "unledgered evidence"]);
            let before = f.basis();
            let config = std::fs::read(f.records.join("tasks/.config.toml")).unwrap();
            let registry = f.env.home.path().join(".config/tasks/projects.toml");
            let original = std::fs::read(&registry).unwrap();
            let out = f.cmd().args(["rename", "sci", "new"]).output().unwrap();
            assert!(
                !out.status.success(),
                "unledgered protected evidence was renamed: {name}"
            );
            assert_eq!(f.basis(), before);
            assert_eq!(std::fs::read(registry).unwrap(), original);
            assert_eq!(
                std::fs::read(f.records.join("tasks/.config.toml")).unwrap(),
                config
            );
            assert!(dir.join(name).is_file());
            assert!(f.records.join(format!("tasks/{}.md", f.id)).is_file());
            assert!(
                !f.env
                    .home
                    .path()
                    .join(".local/state/tasks/rename/sci.toml")
                    .exists()
            );
        }
    }
}

#[test]
fn retention_rename_review_ready_manifest_other_root() {
    let mut f = RetainedEnv::new("detached");
    f.json(&["start", &f.id]);
    let directory = f.dir.join("scratch");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("member"), "payload").unwrap();
    let request = checkpoint_request(&f, 1, json!([declaration(&directory, "scratch")]));
    let path = f.request("intent.json", &request);
    let out = f
        .cmd()
        .env("TASKS_RETAINED_STOP_AFTER", "intent")
        .args([
            "retained",
            "checkpoint",
            &f.id,
            "--request",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(86));
    let request = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":format!("{:032x}",1)},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":f.state()["basis"]}],"reason":"Keep declared directory"});
    let path = f.request("reconcile.json", &request);
    f.json(&[
        "retained",
        "reconcile",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ]);
    f.json(&["park", &f.id, "rename"]);
    let manifest = f.state()["items"][0]["record"]["manifest"]
        .as_str()
        .unwrap()
        .to_string();
    let name = std::path::Path::new(&manifest)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let old = f.id.clone();
    let commit = f.basis();
    renamed_live(&mut f, "new");
    commit_rename(&f, "sci");
    assert_eq!(f.state()["complete"], true);
    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &dest);
            } else {
                std::fs::copy(entry.path(), dest).unwrap();
            }
        }
    }
    let paired = tempfile::tempdir().unwrap();
    copy_tree(f.dir.parent().unwrap(), paired.path());
    f.dir = paired.path().join("code");
    f.records = paired.path().join("records/new");
    let second = common::TestEnv::new();
    second.json(
        &f.dir,
        &[
            "init",
            "--prefix",
            "new",
            "--records",
            paired.path().join("records").to_str().unwrap(),
            "--force",
        ],
    );
    f.env = second;
    assert_eq!(
        f.state()["complete"],
        true,
        "intact evidence remains portable"
    );
    let git = |args: &[&str]| {
        let out = common::git_fixture::command(f.env.home.path(), &f.records)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let oid = git(&[
        "rev-parse",
        &format!("{commit}:sci/tasks/files/{old}/{name}"),
    ]);
    let dir = std::path::PathBuf::from(git(&["rev-parse", "--absolute-git-dir"]));
    common::git_fixture::assert_repo(f.env.home.path(), f.records.parent().unwrap());
    assert!(dir.starts_with(f.records.parent().unwrap()));
    let object = dir.join(format!("objects/{}/{}", &oid[..2], &oid[2..]));
    assert!(object.is_file());
    std::fs::remove_file(object).unwrap();
    assert!(
        !f.cmd()
            .args(["retained", "state", &f.id])
            .output()
            .unwrap()
            .status
            .success(),
        "missing original ready manifest cannot be replaced by the working attachment"
    );
}
