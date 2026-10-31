mod common;

use common::TestEnv;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn raw_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("tasks/{id}.md"))
}

// Construct synthetic physical records before the public archive producer exists.
fn archive_inline(root: &Path, id: &str, crlf: bool) -> (String, Vec<u8>) {
    let raw = std::fs::read_to_string(raw_path(root, id)).unwrap();
    let split = raw.find("## Notes").unwrap();
    let section = if crlf {
        raw[split..].replace('\n', "\r\n")
    } else {
        raw[split..].to_string()
    };
    let count = section
        .lines()
        .filter(|line| line.starts_with("- "))
        .count();
    let hash = digest(section.as_bytes());
    let name = format!("notes-{hash}.notes");
    let descriptor = format!("v1:{hash}:{count}");
    let mut prefix = raw[..split].to_string();
    if let Some(line) = prefix
        .lines()
        .find(|line| line.starts_with("note_archives: "))
    {
        let replacement = format!("{}, \"{descriptor}\"]", line.strip_suffix(']').unwrap());
        prefix = prefix.replacen(line, &replacement, 1);
    } else {
        prefix = prefix.replacen(
            "\n---\n",
            &format!("\nnote_archives: [\"{descriptor}\"]\n---\n"),
            1,
        );
    }
    let dir = root.join(format!("tasks/files/{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(&name), &section).unwrap();
    let active = format!(
        "{prefix}## Notes\n\n- 2026-10-31T00:00:00Z (fixture): attached: {name} ({} bytes)\n",
        section.len()
    );
    std::fs::write(raw_path(root, id), active).unwrap();
    (name, section.into_bytes())
}

fn fixture() -> (TestEnv, PathBuf, String, Value, String, Vec<u8>) {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    let id = env.json(
        &root,
        &[
            "add",
            "Archived",
            "--origin",
            "user",
            "--origin-session",
            "codex:source",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&root, &["note", &id, "archived first sentinel"]);
    env.cmd(&root)
        .env("CODEX_THREAD_ID", "archive-fixture")
        .args(["note", &id, "repeat é", "--stamp"])
        .assert()
        .success();
    let before = env.json(&root, &["show", &id])["task"].clone();
    let (name, bytes) = archive_inline(&root, &id, true);
    (env, root, id, before, name, bytes)
}

#[test]
fn archive_physical_and_logical_boundary() {
    let (env, root, id, before, name, bytes) = fixture();
    let shown = env.json(&root, &["show", &id]);
    assert!(shown["task"].get("note_archives").is_none());
    assert_eq!(
        &shown["task"]["notes"].as_array().unwrap()[..3],
        before["notes"].as_array().unwrap()
    );
    assert_eq!(shown["task"]["notes"].as_array().unwrap().len(), 4);
    assert_eq!(shown["files"][0]["name"], name);
    env.json(&root, &["note", &id, "repeat é"]);
    let metrics = env.json(&root, &["check", "--growth"]);
    assert_eq!(metrics["growth"][0]["note_count"], 5);
    assert_eq!(metrics["growth"][0]["duplicate_note_text_bytes"], 9);
    assert_eq!(metrics["growth"][0]["archived_bytes"], bytes.len());
    let again = env.json(&root, &["note", &id, "repeat é"]);
    assert!(
        again["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("logical note 3"))
    );
    // A missing required segment is an error, never an inline-only task or zero row.
    std::fs::remove_file(root.join(format!("tasks/files/{id}/{name}"))).unwrap();
    assert_eq!(env.fail(&root, &["show", &id]), "parse");
    let out = env.cmd(&root).args(["check", "--growth"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report["growth"].as_array().unwrap().is_empty());
    assert!(
        report["errors"][0]["detail"]
            .as_str()
            .unwrap()
            .contains(&name)
    );
}

#[test]
fn archive_roundtrip_exact_notes() {
    let (env, root, id, before, name, bytes) = fixture();
    env.json(&root, &["note", &id, "new suffix"]);
    env.json(&root, &["edit", &id, "--body", "edited body"]);
    let raw = std::fs::read_to_string(raw_path(&root, &id)).unwrap();
    assert!(!raw.contains("archived first sentinel"));
    assert!(raw.contains("note_archives: [\"v1:"));
    assert_eq!(
        std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
        bytes
    );
    let shown = env.json(&root, &["show", &id]);
    assert_eq!(
        &shown["task"]["notes"].as_array().unwrap()[..3],
        before["notes"].as_array().unwrap()
    );
    let before_second = shown["task"]["notes"].as_array().unwrap().clone();
    let (second, second_bytes) = archive_inline(&root, &id, false);
    let reloaded = env.json(&root, &["show", &id]);
    assert_eq!(
        &reloaded["task"]["notes"].as_array().unwrap()[..before_second.len()],
        before_second
    );
    env.json(&root, &["note", &id, "later"]);
    for (name, bytes) in [(name, bytes), (second, second_bytes)] {
        assert_eq!(
            std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
            bytes
        );
    }
}

#[test]
fn archive_invalid_segment() {
    for failure in [
        "hash",
        "utf8",
        "grammar",
        "note-owner",
        "note-text",
        "ledger",
        "count",
        "version",
        "zero",
        "overflow",
        "traversal",
        "case",
        "duplicate",
        "file-symlink",
        "dir-symlink",
    ] {
        let (env, root, id, _, name, bytes) = fixture();
        env.json(&root, &["show", &id]);
        let path = root.join(format!("tasks/files/{id}/{name}"));
        let record = raw_path(&root, &id);
        let mut raw = std::fs::read_to_string(&record).unwrap();
        let hash = digest(&bytes);
        match failure {
            "hash" => {
                std::fs::write(&path, b"tampered").unwrap();
            }
            "utf8" | "grammar" | "note-owner" | "note-text" => {
                let bad = if failure == "utf8" {
                    vec![255]
                } else if failure == "note-owner" {
                    String::from_utf8(bytes.clone())
                        .unwrap()
                        .replacen(
                            "(tester): archived first sentinel",
                            "(bad\rby): archived first sentinel",
                            1,
                        )
                        .into_bytes()
                } else if failure == "note-text" {
                    String::from_utf8(bytes.clone())
                        .unwrap()
                        .replacen("archived first sentinel", "archived\rfirst sentinel", 1)
                        .into_bytes()
                } else {
                    b"## Notes\n\nnot a note\n".to_vec()
                };
                let new_hash = digest(&bad);
                std::fs::write(path.with_file_name(format!("notes-{new_hash}.notes")), bad)
                    .unwrap();
                raw = raw.replace(&hash, &new_hash);
            }
            "ledger" => {
                raw = raw.replace("attached: ", "unrelated: ");
            }
            "count" => {
                raw = raw.replace(&format!("v1:{hash}:3"), &format!("v1:{hash}:4"));
            }
            "version" => {
                raw = raw.replace("v1:", "v2:");
            }
            "zero" => {
                raw = raw.replace(&format!("v1:{hash}:3"), &format!("v1:{hash}:0"));
            }
            "overflow" => {
                raw = raw.replace(
                    &format!("v1:{hash}:3"),
                    &format!("v1:{hash}:999999999999999999999999999999999"),
                );
            }
            "traversal" => {
                raw = raw.replace(&format!("v1:{hash}:3"), "v1:../../secret:3");
            }
            "case" => {
                raw = raw.replace(
                    &format!("v1:{hash}:3"),
                    &format!("v1:{}:3", hash.to_uppercase()),
                );
            }
            "duplicate" => {
                raw = raw.replace(
                    &format!("[\"v1:{hash}:3\"]"),
                    &format!("[\"v1:{hash}:3\", \"v1:{hash}:3\"]"),
                );
            }
            "file-symlink" => {
                let saved = path.with_extension("saved");
                std::fs::rename(&path, &saved).unwrap();
                std::os::unix::fs::symlink(saved, &path).unwrap();
            }
            "dir-symlink" => {
                let dir = path.parent().unwrap();
                let saved = dir.with_extension("saved");
                std::fs::rename(dir, &saved).unwrap();
                std::os::unix::fs::symlink(saved, dir).unwrap();
            }
            _ => unreachable!(),
        }
        std::fs::write(&record, &raw).unwrap();
        let out = env.cmd(&root).args(["show", &id]).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{failure}");
        assert!(out.stdout.is_empty(), "{failure}");
        let error: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "parse", "{failure}: {error}");
        assert_eq!(std::fs::read_to_string(&record).unwrap(), raw, "{failure}");
    }
}
