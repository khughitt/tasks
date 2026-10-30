mod common;

use common::{TestEnv, docs::DocFixture};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture() -> (TestEnv, PathBuf, String, String) {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    let id = env.json(&root, &["add", "Evidence"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let source = root.join("receipt.txt");
    std::fs::write(&source, b"receipt").unwrap();
    env.json(&root, &["attach", &id, source.to_str().unwrap()]);
    let path = format!("tasks/files/{id}/receipt.txt");
    (env, root, id, path)
}

fn query(env: &TestEnv, root: &Path, paths: &[&str]) -> Value {
    let mut args = vec!["attachments"];
    args.extend_from_slice(paths);
    env.json(root, &args)
}

fn denied(row: &Value, kind: &str, severity: &str) {
    assert_eq!(row["eligible"], false, "{row}");
    assert!(row.get("record_sha256").is_none(), "{row}");
    assert!(row.get("file_sha256").is_none(), "{row}");
    assert!(
        row["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| { p["kind"] == kind && p["severity"] == severity && p["file"].is_string() }),
        "{row}"
    );
}

#[test]
fn eligibility_returns_exact_local_snapshot_without_registry_or_provider_reads() {
    let (env, root, id, path) = fixture();
    let docs = DocFixture::new(&env, "# Evidence\n");
    env.json(&root, &["edit", &id, "--spec", "stub:evidence"]);
    let calls = docs.calls();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        "not toml [",
    )
    .unwrap();
    let record = root.join(format!("tasks/{id}.md"));
    let config = root.join("tasks/.config.toml");
    let record_bytes = std::fs::read(&record).unwrap();
    let config_bytes = std::fs::read(&config).unwrap();
    let claims = std::fs::read(env.claim_store("dot")).ok();
    let out = query(&env, &root, &[&path]);
    assert_eq!(out["version"], 1);
    assert_eq!(out["records"], root.to_str().unwrap());
    assert_eq!(out["prefix"], "dot");
    assert_eq!(out["config_sha256"], digest(&config_bytes));
    assert_eq!(out["attachments"].as_array().unwrap().len(), 1);
    let row = &out["attachments"][0];
    assert_eq!(row["path"], path);
    assert_eq!(row["eligible"], true);
    assert_eq!(row["record_sha256"], digest(&record_bytes));
    assert_eq!(row["file_sha256"], digest(b"receipt"));
    assert_eq!(row["problems"], serde_json::json!([]));
    assert_eq!(docs.calls(), calls);
    assert_eq!(std::fs::read(&record).unwrap(), record_bytes);
    assert_eq!(std::fs::read(&config).unwrap(), config_bytes);
    assert_eq!(std::fs::read(env.claim_store("dot")).ok(), claims);
}

#[test]
fn eligibility_follows_latest_ledger_and_keeps_closed_retained_evidence() {
    let (env, root, id, path) = fixture();
    env.json(&root, &["detach", &id, "receipt.txt", "old"]);
    std::fs::create_dir_all(root.join(&path).parent().unwrap()).unwrap();
    std::fs::write(root.join(&path), b"receipt").unwrap();
    denied(
        &query(&env, &root, &[&path])["attachments"][0],
        "attachment_unnoted",
        "error",
    );
    env.json(&root, &["attach", &id, "receipt.txt"]);
    env.json(&root, &["done", &id]);
    env.json(&root, &["retire", "--reason", "fixture"]);
    assert_eq!(
        query(&env, &root, &[&path])["attachments"][0]["eligible"],
        true
    );
}

#[test]
fn eligibility_denies_unnoted_missing_orphaned_and_invalid_names() {
    let (env, root, id, path) = fixture();
    let unnoted = format!("tasks/files/{id}/unnoted.txt");
    std::fs::write(root.join(&unnoted), b"unnoted").unwrap();
    let invalid = format!("tasks/files/{id}/.hidden");
    std::fs::write(root.join(&invalid), b"hidden").unwrap();
    let missing = format!("tasks/files/{id}/missing.txt");
    let out = query(&env, &root, &[&unnoted, &invalid, &missing, &path]);
    assert_eq!(out["attachments"][0]["path"], unnoted);
    denied(&out["attachments"][0], "attachment_unnoted", "error");
    denied(&out["attachments"][1], "attachment_invalid", "warning");
    denied(&out["attachments"][2], "attachment_missing", "error");
    assert_eq!(out["attachments"][3]["eligible"], true);
    std::fs::remove_file(root.join(format!("tasks/{id}.md"))).unwrap();
    denied(
        &query(&env, &root, &[&path])["attachments"][0],
        "attachment_orphan",
        "error",
    );
}

#[test]
fn eligibility_enforces_cap_and_ignores_unrelated_bad_records_and_siblings() {
    let (env, root, id, path) = fixture();
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text + "\n[attachments]\nmax_bytes = 7\n").unwrap();
    std::fs::create_dir(root.join(format!("tasks/files/{id}/nested"))).unwrap();
    let unrelated = if id == "dot-000000" {
        "dot-000001"
    } else {
        "dot-000000"
    };
    std::fs::write(root.join(format!("tasks/{unrelated}.md")), "broken record").unwrap();
    assert_eq!(
        query(&env, &root, &[&path])["attachments"][0]["eligible"],
        true
    );
    std::fs::write(root.join(&path), b"receipt+").unwrap();
    denied(
        &query(&env, &root, &[&path])["attachments"][0],
        "attachment_too_large",
        "warning",
    );
}

#[test]
fn eligibility_never_borrows_owner_from_registered_or_claimed_checkout() {
    let (mut env, root, id, path) = fixture();
    env.json(&root, &["start", &id]);
    let other = env.init_forced("dot");
    assert_eq!(
        query(&env, &root, &[&path])["attachments"][0]["eligible"],
        true
    );
    denied(
        &query(&env, &other, &[&path])["attachments"][0],
        "attachment_orphan",
        "error",
    );
}

#[test]
fn eligibility_reads_detached_records_without_a_paired_code_checkout() {
    let (env, root, _, path) = fixture();
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!(
            "{text}\n[checkout]\npath = '{}'\nkind = 'detached'\n",
            root.join("missing-code").display()
        ),
    )
    .unwrap();
    assert_eq!(
        query(&env, &root, &[&path])["attachments"][0]["eligible"],
        true
    );
}

#[test]
fn eligibility_rejects_malformed_config_record_and_owner_prefix() {
    let (env, root, id, path) = fixture();
    let record = root.join(format!("tasks/{id}.md"));
    let original = std::fs::read(&record).unwrap();
    std::fs::write(&record, "broken record").unwrap();
    assert_eq!(env.fail(&root, &["attachments", &path]), "parse");
    std::fs::write(&record, original).unwrap();
    assert_eq!(
        env.fail(&root, &["attachments", "tasks/files/other-000000/a.txt"]),
        "validation"
    );
    std::fs::write(root.join("tasks/.config.toml"), "bad [toml").unwrap();
    assert_eq!(env.fail(&root, &["attachments", &path]), "config");
}

#[test]
fn eligibility_rejects_duplicate_absolute_traversal_and_nested_requests() {
    let (env, root, _, path) = fixture();
    assert_eq!(
        env.fail(&root, &["attachments", &path, &path]),
        "validation"
    );
    for bad in [
        root.join(&path).to_string_lossy().into_owned(),
        path.replace("receipt.txt", "../receipt.txt"),
        path.replace("receipt.txt", "nested/file.txt"),
        path.replace("tasks/files/", "tasks//files/"),
    ] {
        assert_eq!(env.fail(&root, &["attachments", &bad]), "validation");
    }
}

#[test]
fn eligibility_bounds_requests_and_renders_pretty_without_hiding_denials() {
    let (env, root, id, path) = fixture();
    let paths: Vec<_> = (0..128)
        .map(|n| format!("tasks/files/{id}/missing-{n}.txt"))
        .collect();
    let borrowed: Vec<_> = paths.iter().map(String::as_str).collect();
    assert_eq!(
        query(&env, &root, &borrowed)["attachments"]
            .as_array()
            .unwrap()
            .len(),
        128
    );
    let mut too_many = vec!["attachments"];
    too_many.extend_from_slice(&borrowed);
    too_many.push(&path);
    assert!(
        !env.cmd(&root)
            .args(too_many)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !env.cmd(&root)
            .arg("attachments")
            .output()
            .unwrap()
            .status
            .success()
    );
    let out = env
        .cmd(&root)
        .args(["--pretty", "attachments", &path, &paths[0]])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&path) && text.contains("eligible") && text.contains("attachment_missing"),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn eligibility_never_follows_symlink_in_file_record_storage_or_configuration() {
    use std::os::unix::fs::symlink;
    for component in ["entry", "record", "owner", "files", "config", "tasks"] {
        let (env, root, id, path) = fixture();
        let victim = match component {
            "entry" => root.join(&path),
            "record" => root.join(format!("tasks/{id}.md")),
            "owner" => root.join(format!("tasks/files/{id}")),
            "files" => root.join("tasks/files"),
            "config" => root.join("tasks/.config.toml"),
            _ => root.join("tasks"),
        };
        let moved = root.join("moved");
        std::fs::rename(&victim, &moved).unwrap();
        symlink(&moved, &victim).unwrap();
        let out = env
            .cmd(&root)
            .args(["attachments", &path])
            .output()
            .unwrap();
        if out.status.success() {
            let result: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(
                result["attachments"][0]["eligible"], false,
                "{component}: {result}"
            );
            assert!(
                !result["attachments"][0]["problems"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        } else {
            let error: Value = serde_json::from_slice(&out.stderr).unwrap();
            assert!(error["error"]["kind"].is_string(), "{component}: {error}");
        }
    }
}
