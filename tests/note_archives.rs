mod common;

use common::{TestEnv, archive_inline};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[test]
fn archived_notes_keep_the_multipart_declaration() {
    let (env, root, id) = archivable();
    let bytes = common::multipart::seed(&env, &root, &id, "evidence.txt");
    let before = env.json(&root, &["show", &id]);
    let preview = env.json(&root, &["archive-notes", &id]);
    assert_eq!(preview["eligible"], true, "{preview}");
    let archived = env.json(
        &root,
        &[
            "archive-notes",
            &id,
            "--expect",
            preview["source_sha256"].as_str().unwrap(),
        ],
    );
    assert_eq!(archived["action"], "archived");
    assert!(
        !env.read(&root, &format!("tasks/{id}.md"))
            .contains("multipart: ")
    );
    let after = env.json(&root, &["show", &id]);
    assert_eq!(after["multipart"], before["multipart"]);
    common::multipart::assert_intact(&env, &root, &id, "evidence.txt", &bytes);
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn raw_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("tasks/{id}.md"))
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
    assert_eq!(report["errors"][0]["file"], format!("tasks/{id}.md"));
    let detail = report["errors"][0]["detail"].as_str().unwrap();
    assert!(detail.contains(&format!("tasks/{id}.md")), "{detail}");
    assert!(
        detail.contains(&format!("tasks/files/{id}/{name}")),
        "{detail}"
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
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(
            detail.contains(&format!("tasks/{id}.md")),
            "{failure}: {detail}"
        );
        if matches!(
            failure,
            "hash"
                | "utf8"
                | "grammar"
                | "note-owner"
                | "note-text"
                | "ledger"
                | "count"
                | "file-symlink"
                | "dir-symlink"
        ) {
            assert!(
                detail.contains(&format!("tasks/files/{id}/notes-")),
                "{failure}: {detail}"
            );
        }
        assert_eq!(std::fs::read_to_string(&record).unwrap(), raw, "{failure}");
    }
}

fn editor(root: &Path, body: &str) -> String {
    let path = root.join("editor.py");
    let capture = serde_json::to_string(root.join("editor-input.txt").to_str().unwrap()).unwrap();
    std::fs::write(&path, format!("from pathlib import Path\nimport sys\np = Path(sys.argv[1])\ntext = p.read_text()\nPath({capture}).write_text(text)\n{body}\n")).unwrap();
    format!(
        "python3 '{}'",
        path.display().to_string().replace('\'', "'\\''")
    )
}

#[test]
fn archive_editor_compact_buffer() {
    let (env, root, id, before, name, bytes) = fixture();
    let active = std::fs::read_to_string(raw_path(&root, &id)).unwrap();
    let command = editor(
        &root,
        "p.write_text(text.replace('\\n---\\n', '\\n---\\n\\nEdited body\\n', 1))",
    );
    env.cmd(&root)
        .env("EDITOR", command)
        .args(["edit", &id, "--editor"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(root.join("editor-input.txt")).unwrap(),
        active
    );
    assert!(!active.contains("archived first sentinel"));
    let shown = env.json(&root, &["show", &id]);
    assert_eq!(shown["task"]["body"], "Edited body");
    assert_eq!(
        &shown["task"]["notes"].as_array().unwrap()[..3],
        before["notes"].as_array().unwrap()
    );
    assert_eq!(
        std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
        bytes
    );
}

#[test]
fn archive_editor_stale_and_protected() {
    // Different byte segments may represent identical Notes: history equivalence alone
    // must not let an editor reorder their protected descriptors.
    let mut env = TestEnv::new();
    let root = env.init("dot");
    let id = env.json(&root, &["add", "Equivalent"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let original = std::fs::read_to_string(raw_path(&root, &id)).unwrap();
    let prefix = &original[..original.find("## Notes").unwrap()];
    let mut descriptors = Vec::new();
    let mut suffix = String::from("## Notes\n\n");
    std::fs::create_dir_all(root.join(format!("tasks/files/{id}"))).unwrap();
    for ending in ["\n", "\r\n"] {
        let bytes = format!(
            "## Notes{ending}{ending}- 2026-10-01T00:00:00Z (fixture): equivalent-history-sentinel{ending}"
        );
        let hash = digest(bytes.as_bytes());
        let name = format!("notes-{hash}.notes");
        descriptors.push(format!("\"v1:{hash}:1\""));
        std::fs::write(root.join(format!("tasks/files/{id}/{name}")), &bytes).unwrap();
        suffix.push_str(&format!(
            "- 2026-10-02T00:00:00Z (fixture): attached: {name} ({} bytes)\n",
            bytes.len()
        ));
    }
    let line = format!("note_archives: [{}]", descriptors.join(", "));
    let active = format!(
        "{}{suffix}",
        prefix.replacen("\n---\n", &format!("\n{line}\n---\n"), 1)
    );
    std::fs::write(raw_path(&root, &id), &active).unwrap();
    env.json(&root, &["show", &id]);
    descriptors.reverse();
    let changed = format!("note_archives: [{}]", descriptors.join(", "));
    let command = editor(
        &root,
        &format!(
            "p.write_text(text.replace({}, {}))",
            serde_json::to_string(&line).unwrap(),
            serde_json::to_string(&changed).unwrap()
        ),
    );
    let out = env
        .cmd(&root)
        .env("EDITOR", command)
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "descriptor reorder must fail even when logical Notes are equal"
    );
    assert_eq!(
        std::fs::read_to_string(raw_path(&root, &id)).unwrap(),
        active
    );
    for entry in std::fs::read_dir(root.join("tasks")).unwrap() {
        let path = entry.unwrap().path();
        if path.to_string_lossy().ends_with(".edit.md") {
            assert!(
                !std::fs::read_to_string(path)
                    .unwrap()
                    .contains("equivalent-history-sentinel")
            );
        }
    }

    for tamper in ["segment", "source"] {
        let (env, root, id, _, name, _) = fixture();
        let active = std::fs::read_to_string(raw_path(&root, &id)).unwrap();
        let body = if tamper == "segment" {
            format!(
                "Path({}).write_bytes(b'changed')",
                serde_json::to_string(
                    root.join(format!("tasks/files/{id}/{name}"))
                        .to_str()
                        .unwrap()
                )
                .unwrap()
            )
        } else {
            format!(
                "Path({}).write_text(text.replace('title: Archived', 'title: Concurrent'))",
                serde_json::to_string(raw_path(&root, &id).to_str().unwrap()).unwrap()
            )
        };
        let command = editor(&root, &body);
        let out = env
            .cmd(&root)
            .env("EDITOR", command)
            .args(["edit", &id, "--editor"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{tamper}");
        assert_eq!(
            std::fs::read_to_string(root.join("editor-input.txt")).unwrap(),
            active
        );
        for entry in std::fs::read_dir(root.join("tasks")).unwrap() {
            let path = entry.unwrap().path();
            if path.to_string_lossy().ends_with(".edit.md") {
                assert!(
                    !std::fs::read_to_string(path)
                        .unwrap()
                        .contains("archived first sentinel")
                );
            }
        }
    }
}

#[test]
fn archive_move_pinned_journal() {
    for boundary in [
        "journal",
        "attachments",
        "destination",
        "tombstone",
        "cleanup",
    ] {
        let (mut env, root, id, _, first, first_bytes) = fixture();
        let target = env.init("dst");
        let ordinary = root.join("ordinary.txt");
        std::fs::write(&ordinary, b"ordinary evidence").unwrap();
        env.json(&root, &["attach", &id, ordinary.to_str().unwrap()]);
        // The ordinary attach ledger entry is now archived too.
        let (second, second_bytes) = archive_inline(&root, &id, false);
        let before = env.json(&root, &["show", &id])["task"]["notes"]
            .as_array()
            .unwrap()
            .clone();
        let out = env
            .cmd(&root)
            .env("TASKS_MOVE_STOP_AFTER", boundary)
            .args(["move", &id, "--to", "dst"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{boundary}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let moved: Value = serde_json::from_slice(&out.stdout).unwrap();
        let new_id = moved["id"].as_str().unwrap();
        let journal = env
            .home
            .path()
            .join(format!(".local/state/tasks/move/{id}.toml"));
        let journal_text = std::fs::read_to_string(&journal).unwrap();
        let value: toml::Value = toml::from_str(&journal_text).unwrap();
        let entries = value["attachments"].as_array().unwrap();
        let names: std::collections::BTreeSet<_> = entries
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            std::collections::BTreeSet::from([first.as_str(), second.as_str(), "ordinary.txt"])
        );
        for entry in entries {
            assert_eq!(
                entry["retain_source"].as_bool(),
                Some(entry["name"].as_str() != Some("ordinary.txt"))
            );
        }
        assert_eq!(value["source_records"].as_str(), root.to_str());
        if boundary == "journal" {
            assert!(!raw_path(&target, new_id).exists());
            for corruption in [
                "missing",
                "extra",
                "disposition",
                "detach",
                "prefix",
                "unpinned",
            ] {
                let mut bad = value.clone();
                match corruption {
                    "missing" => {
                        bad["attachments"].as_array_mut().unwrap().remove(0);
                    }
                    "extra" => {
                        let mut extra = bad["attachments"][0].clone();
                        extra["name"] = toml::Value::String("extra.txt".into());
                        extra["retain_source"] = toml::Value::Boolean(false);
                        bad["attachments"].as_array_mut().unwrap().push(extra);
                    }
                    "disposition" => {
                        bad["attachments"][0]["retain_source"] = toml::Value::Boolean(false);
                    }
                    "detach" => {
                        bad["tombstone_text"] = toml::Value::String(format!(
                            "{}- 2026-10-31T00:00:00Z (fixture): detached: {first}: forged\n",
                            bad["tombstone_text"].as_str().unwrap()
                        ));
                    }
                    "prefix" => {
                        let changed = format!(
                            "{}- 2026-10-31T00:00:00Z (fixture): changed history\n",
                            bad["source_text"].as_str().unwrap()
                        );
                        bad["source_digest"] = toml::Value::String(digest(changed.as_bytes()));
                        bad["source_text"] = toml::Value::String(changed);
                    }
                    "unpinned" => {
                        bad.as_table_mut().unwrap().remove("source_records");
                    }
                    _ => unreachable!(),
                }
                std::fs::write(&journal, toml::to_string(&bad).unwrap()).unwrap();
                assert!(
                    !env.cmd(&root)
                        .args(["move", &id, "--to", "dst"])
                        .output()
                        .unwrap()
                        .status
                        .success(),
                    "{corruption}"
                );
                assert_eq!(
                    std::fs::read(root.join(format!("tasks/files/{id}/{first}"))).unwrap(),
                    first_bytes
                );
            }
            std::fs::write(&journal, &journal_text).unwrap();
        }
        if boundary == "attachments" {
            let source_archive = root.join(format!("tasks/files/{id}/{first}"));
            assert!(
                target
                    .join(format!("tasks/files/{new_id}/{first}"))
                    .exists()
            );
            std::fs::remove_file(&source_archive).unwrap();
            assert!(
                !env.cmd(&root)
                    .args(["move", &id, "--to", "dst"])
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            assert!(!raw_path(&target, new_id).exists());
            std::fs::write(source_archive, &first_bytes).unwrap();
        }
        env.json(&root, &["move", &id, "--to", "dst"]);
        assert!(!journal.exists());
        for (name, bytes) in [(&first, &first_bytes), (&second, &second_bytes)] {
            assert_eq!(
                &std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
                bytes
            );
            assert_eq!(
                &std::fs::read(target.join(format!("tasks/files/{new_id}/{name}"))).unwrap(),
                bytes
            );
        }
        assert!(!root.join(format!("tasks/files/{id}/ordinary.txt")).exists());
        let source = env.json(&root, &["show", &id]);
        let destination = env.json(&target, &["show", new_id]);
        for shown in [&source, &destination] {
            assert_eq!(
                &shown["task"]["notes"].as_array().unwrap()[..before.len()],
                before
            );
        }
        let detach_notes: Vec<_> = source["task"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str().filter(|t| t.starts_with("detached:")))
            .collect();
        assert_eq!(detach_notes.len(), 1);
        assert!(detach_notes[0].contains("ordinary.txt"));
        // Each later tombstone retains its independent immutable copies as well.
        let third = env.init("last");
        let next = env.json(&target, &["move", new_id, "--to", "last"]);
        let next_id = next["id"].as_str().unwrap();
        assert_eq!(
            std::fs::read(third.join(format!("tasks/files/{next_id}/{first}"))).unwrap(),
            first_bytes
        );
        assert_eq!(
            std::fs::read(target.join(format!("tasks/files/{new_id}/{first}"))).unwrap(),
            first_bytes
        );
    }
}

#[test]
fn archive_detach_rename_eligibility() {
    let (env, root, id, _, name, bytes) = fixture();
    let before = std::fs::read(raw_path(&root, &id)).unwrap();
    let out = env
        .cmd(&root)
        .args(["detach", &id, &name, "remove"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("protected"),
        "{error}"
    );
    assert_eq!(std::fs::read(raw_path(&root, &id)).unwrap(), before);
    assert_eq!(
        std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
        bytes
    );
    let path = format!("tasks/files/{id}/{name}");
    assert_eq!(
        env.json(&root, &["attachments", &path])["attachments"][0]["eligible"],
        true
    );
    env.json(&root, &["rename", "dot", "new"]);
    let new_id = id.replacen("dot-", "new-", 1);
    assert_eq!(
        std::fs::read(root.join(format!("tasks/files/{new_id}/{name}"))).unwrap(),
        bytes
    );
    env.json(&root, &["rename", "dot", "new", "--adopt"]);
    env.json(&root, &["show", &new_id]);
    std::fs::write(
        root.join(format!("tasks/files/{new_id}/{name}")),
        b"tampered",
    )
    .unwrap();
    assert!(
        !env.cmd(&root)
            .args(["attachments", &format!("tasks/files/{new_id}/{name}")])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn archive_metadata_only_other_checkout() {
    let (env, root, id, _, name, _) = fixture();
    env.json(&root, &["done", &id, "finished"]);
    let sibling = env.home.path().join("sibling");
    common::isolated_worktree(&env, &root, &sibling);
    std::fs::remove_file(sibling.join(format!("tasks/files/{id}/{name}"))).unwrap();
    // Stale-copy inspection needs the sibling's physical metadata, not its archive.
    env.json(&root, &["note", &id, "current observation"]);
    // Retirement inspects closed status in every checkout without requiring archives.
    env.json(&root, &["retire", "--reason", "complete"]);
}

#[test]
fn archive_linked_and_detached_storage() {
    for kind in ["linked", "detached"] {
        let (mut env, root, id, _, name, bytes) = fixture();
        let code = env.home.path().join("code");
        std::fs::create_dir_all(&code).unwrap();
        let out = common::git_fixture::command(env.home.path(), &code)
            .args(["init", "-q", "-b", "main"])
            .output()
            .unwrap();
        assert!(out.status.success());
        common::git_fixture::assert_repo(env.home.path(), &code);
        let records = if kind == "linked" {
            let config = root.join("tasks/.config.toml");
            let text = std::fs::read_to_string(&config).unwrap();
            std::fs::write(
                &config,
                format!(
                    "{text}\n[checkout]\npath = {}\n",
                    serde_json::to_string(code.to_str().unwrap()).unwrap()
                ),
            )
            .unwrap();
            env.json(
                &root,
                &[
                    "init",
                    "--prefix",
                    "dot",
                    "--checkout",
                    code.to_str().unwrap(),
                    "--force",
                ],
            );
            root.clone()
        } else {
            let base = env.home.path().join("records");
            let records = base.join("dot");
            std::fs::create_dir_all(&records).unwrap();
            std::fs::rename(root.join("tasks"), records.join("tasks")).unwrap();
            env.json(
                &code,
                &[
                    "init",
                    "--prefix",
                    "dot",
                    "--records",
                    base.to_str().unwrap(),
                    "--force",
                ],
            );
            records
        };
        env.json(&code, &["show", &id]);
        env.json(&code, &["note", &id, "from the code checkout"]);
        let destination = env.init("dst");
        let moved = env.json(&code, &["move", &id, "--to", "dst"]);
        let new_id = moved["id"].as_str().unwrap();
        assert!(
            moved["paths"]
                .as_object()
                .unwrap()
                .contains_key(records.to_str().unwrap()),
            "{kind}: paths must resolve from the records root: {moved}"
        );
        assert_eq!(
            std::fs::read(records.join(format!("tasks/files/{id}/{name}"))).unwrap(),
            bytes,
            "{kind}"
        );
        assert_eq!(
            std::fs::read(destination.join(format!("tasks/files/{new_id}/{name}"))).unwrap(),
            bytes,
            "{kind}"
        );
    }
}

#[test]
fn archive_ordinary_inflight_journal_defaults() {
    let mut env = TestEnv::new();
    let root = env.init("dot");
    let target = env.init("dst");
    let id = env.json(&root, &["add", "Ordinary move"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let ordinary = root.join("ordinary.txt");
    std::fs::write(&ordinary, b"ordinary").unwrap();
    env.json(&root, &["attach", &id, ordinary.to_str().unwrap()]);
    let out = env
        .cmd(&root)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &id, "--to", "dst"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let moved: Value = serde_json::from_slice(&out.stdout).unwrap();
    let new_id = moved["id"].as_str().unwrap();
    let journal = env
        .home
        .path()
        .join(format!(".local/state/tasks/move/{id}.toml"));
    let text = std::fs::read_to_string(&journal).unwrap();
    assert!(!text.contains("source_records"));
    std::fs::write(&journal, text.replace("retain_source = false\n", "")).unwrap();
    env.json(&root, &["move", &id, "--to", "dst"]);
    assert_eq!(
        std::fs::read(target.join(format!("tasks/files/{new_id}/ordinary.txt"))).unwrap(),
        b"ordinary"
    );
}

#[test]
fn archive_rename_interruption_recovery() {
    for phase in [
        "inventory",
        "file:0",
        "attachments",
        "files",
        "config",
        "registry",
        "store",
        "claims",
    ] {
        let (env, root, id, before, name, bytes) = fixture();
        let boundary = if phase == "attachments" {
            format!("attachments:{}", id.split_once('-').unwrap().1)
        } else {
            phase.into()
        };
        let out = env
            .cmd(&root)
            .env("TASKS_RENAME_STOP_AFTER", &boundary)
            .args(["rename", "dot", "new"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{phase}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        env.json(&root, &["rename", "dot", "new"]);
        env.json(&root, &["rename", "dot", "new", "--adopt"]);
        let new_id = id.replacen("dot-", "new-", 1);
        let shown = env.json(&root, &["show", &new_id]);
        assert_eq!(
            &shown["task"]["notes"].as_array().unwrap()[..3],
            before["notes"].as_array().unwrap()
        );
        assert_eq!(
            std::fs::read(root.join(format!("tasks/files/{new_id}/{name}"))).unwrap(),
            bytes,
            "{phase}"
        );
    }
}

fn archivable() -> (TestEnv, PathBuf, String) {
    let mut env = TestEnv::new();
    let root = env.init("arc");
    let id = env.json(&root, &["add", "Archive candidate"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.cmd(&root)
        .env("CODEX_THREAD_ID", "source")
        .args(["note", &id, &"é exact history ".repeat(400), "--stamp"])
        .assert()
        .success();
    (env, root, id)
}

#[test]
fn archive_preview_apply_exact() {
    let (env, root, id) = archivable();
    let path = raw_path(&root, &id);
    // A body mention is not a note-section header; CRLF and trailing bytes are exact evidence.
    env.json(
        &root,
        &[
            "edit",
            &id,
            "--body",
            "Mention ## Notes within this sentence",
        ],
    );
    let original = std::fs::read_to_string(&path).unwrap();
    let split = original.find("\n## Notes\n").unwrap() + 1;
    let raw = format!(
        "{}{}",
        &original[..split],
        original[split..].replace('\n', "\r\n")
    );
    std::fs::write(&path, &raw).unwrap();
    let before = env.json(&root, &["show", &id])["task"].clone();
    let preview = env.json(&root, &["archive-notes", &id]);
    assert_eq!(preview["source_sha256"], digest(raw.as_bytes()));
    assert_eq!(preview["eligible"], true, "{preview}");
    assert_eq!(preview.as_object().unwrap().len(), 9);
    assert_eq!(std::fs::read(&path).unwrap(), raw.as_bytes());
    assert!(!root.join("tasks/files").exists());
    let hash = preview["source_sha256"].as_str().unwrap();
    let out = env.json(&root, &["archive-notes", &id, "--expect", hash]);
    assert_eq!(out["action"], "archived");
    assert_eq!(out["record_bytes_before"], raw.len());
    assert_eq!(
        out["record_bytes_after"],
        std::fs::metadata(&path).unwrap().len()
    );
    let section = &raw.as_bytes()[raw.find("\n## Notes\r\n").unwrap() + 1..];
    let name = preview["archive_name"].as_str().unwrap();
    assert_eq!(preview["archive_bytes"], section.len());
    assert_eq!(
        std::fs::read(root.join(format!("tasks/files/{id}/{name}"))).unwrap(),
        section
    );
    let after = env.json(&root, &["show", &id])["task"].clone();
    let n = before["notes"].as_array().unwrap().len();
    assert_eq!(
        &after["notes"].as_array().unwrap()[..n],
        before["notes"].as_array().unwrap()
    );
    assert_eq!(after["notes"].as_array().unwrap().len(), n + 2);
    let mut expected = before;
    expected["notes"] = after["notes"].clone();
    expected["updated"] = after["updated"].clone();
    assert_eq!(after, expected);
}

#[test]
fn archive_retry_after_append() {
    let (env, root, id) = archivable();
    let preview = env.json(&root, &["archive-notes", &id]);
    let hash = preview["source_sha256"].as_str().unwrap();
    env.json(&root, &["archive-notes", &id, "--expect", hash]);
    env.json(&root, &["note", &id, &"later history ".repeat(300)]);
    let raw = std::fs::read(raw_path(&root, &id)).unwrap();
    let out = env.json(&root, &["archive-notes", &id, "--expect", hash]);
    assert_eq!(out["action"], "already_archived");
    assert_eq!(out["record_bytes_before"], raw.len());
    assert_eq!(out["record_bytes_after"], raw.len());
    assert_eq!(std::fs::read(raw_path(&root, &id)).unwrap(), raw);
    let second = env.json(&root, &["archive-notes", &id]);
    env.json(
        &root,
        &[
            "archive-notes",
            &id,
            "--expect",
            second["source_sha256"].as_str().unwrap(),
        ],
    );
    assert_eq!(
        env.json(&root, &["archive-notes", &id, "--expect", hash])["action"],
        "already_archived"
    );
    assert_eq!(
        std::fs::read_dir(root.join(format!("tasks/files/{id}")))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn archive_interruption_recovery() {
    for boundary in [
        "before-segment-publish",
        "after-segment-publish",
        "before-record-switch",
        "after-record-switch",
    ] {
        let (env, root, id) = archivable();
        let path = raw_path(&root, &id);
        let raw = std::fs::read(&path).unwrap();
        let preview = env.json(&root, &["archive-notes", &id]);
        let hash = preview["source_sha256"].as_str().unwrap();
        let name = preview["archive_name"].as_str().unwrap();
        let segment = root.join(format!("tasks/files/{id}/{name}"));
        let out = env
            .cmd(&root)
            .env("TASKS_ARCHIVE_STOP_AFTER", boundary)
            .args(["archive-notes", &id, "--expect", hash])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(86), "{boundary}: {out:?}");
        assert_eq!(segment.exists(), boundary != "before-segment-publish");
        if boundary != "after-record-switch" {
            assert_eq!(std::fs::read(&path).unwrap(), raw);
        }
        let out = env.json(&root, &["archive-notes", &id, "--expect", hash]);
        assert_eq!(
            out["action"],
            if boundary == "after-record-switch" {
                "already_archived"
            } else {
                "archived"
            }
        );
        let notes = env.json(&root, &["show", &id])["task"]["notes"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(notes.len(), 4);
        assert_eq!(
            notes
                .iter()
                .filter(|n| n["text"].as_str().unwrap().starts_with("archived-notes:"))
                .count(),
            1
        );
        assert_eq!(
            std::fs::read_dir(segment.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        let split = raw.windows(9).position(|b| b == b"## Notes\n").unwrap();
        assert_eq!(std::fs::read(&segment).unwrap(), raw[split..]);
    }
}

#[test]
fn archive_eligibility_boundaries_and_storage() {
    let (env, root, id) = archivable();
    let path = raw_path(&root, &id);
    let initial = env.json(&root, &["archive-notes", &id]);
    let config = root.join("tasks/.config.toml");
    let old = std::fs::read_to_string(&config).unwrap();
    let cap = initial["archive_bytes"].as_u64().unwrap();
    for (limit, eligible) in [(cap, true), (cap - 1, false)] {
        std::fs::write(
            &config,
            format!("{old}\n[attachments]\nmax_bytes = {limit}\n"),
        )
        .unwrap();
        let preview = env.json(&root, &["archive-notes", &id]);
        assert_eq!(preview["eligible"], eligible, "{preview}");
        if !eligible {
            let raw = std::fs::read(&path).unwrap();
            assert_eq!(
                env.fail(
                    &root,
                    &[
                        "archive-notes",
                        &id,
                        "--expect",
                        preview["source_sha256"].as_str().unwrap()
                    ]
                ),
                "validation"
            );
            assert_eq!(std::fs::read(&path).unwrap(), raw);
        }
    }
    std::fs::write(&config, old).unwrap();
    let name = initial["archive_name"].as_str().unwrap();
    let dir = root.join(format!("tasks/files/{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join(name);
    std::fs::write(&target, b"conflict").unwrap();
    assert_eq!(env.json(&root, &["archive-notes", &id])["eligible"], false);
    assert_eq!(
        env.fail(
            &root,
            &[
                "archive-notes",
                &id,
                "--expect",
                initial["source_sha256"].as_str().unwrap()
            ]
        ),
        "validation"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"conflict");
    std::fs::remove_file(&target).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path().join("missing"), &target).unwrap();
    assert_eq!(env.json(&root, &["archive-notes", &id])["eligible"], false);
    assert!(!outside.path().join("missing").exists());
    let tiny = env.json(&root, &["add", "Tiny"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        env.json(&root, &["archive-notes", &tiny])["eligible"],
        false
    );
    let tiny_path = raw_path(&root, &tiny);
    let raw = std::fs::read_to_string(&tiny_path).unwrap();
    let split = raw.find("## Notes").unwrap();
    std::fs::write(&tiny_path, &raw[..split]).unwrap();
    let p = env.json(&root, &["archive-notes", &tiny]);
    assert_eq!(p["note_count"], 0);
    assert_eq!(p["eligible"], false);
}

#[test]
fn archive_stale_claim_and_policy() {
    let (env, root, id) = archivable();
    let preview = env.json(&root, &["archive-notes", &id]);
    let hash = preview["source_sha256"].as_str().unwrap();
    env.json(&root, &["note", &id, "concurrent append"]);
    let raw = std::fs::read(raw_path(&root, &id)).unwrap();
    assert_eq!(
        env.fail(&root, &["archive-notes", &id, "--expect", hash]),
        "concurrent_modification"
    );
    assert_eq!(std::fs::read(raw_path(&root, &id)).unwrap(), raw);
    assert!(!root.join("tasks/files").exists());
    for state in ["live", "unverifiable"] {
        env.cmd(&root)
            .env("TASKS_SESSION", "other")
            .env("TASKS_SESSION_PID", std::process::id().to_string())
            .args(["start", &id])
            .assert()
            .success();
        if state == "unverifiable" {
            let path = env.claim_store("arc");
            let mut store: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            store["claims"][&id]
                .as_table_mut()
                .unwrap()
                .insert("pid_view".into(), toml::Value::String("0:1".into()));
            std::fs::write(path, toml::to_string(&store).unwrap()).unwrap();
        }
        let p = env.json(&root, &["archive-notes", &id]);
        assert_eq!(
            env.fail(
                &root,
                &[
                    "archive-notes",
                    &id,
                    "--expect",
                    p["source_sha256"].as_str().unwrap()
                ]
            ),
            "claimed"
        );
    }
    std::fs::remove_file(env.claim_store("arc")).unwrap();
    for status in ["done", "shelved"] {
        if status == "done" {
            env.json(&root, &["done", &id]);
        } else {
            env.json(&root, &["edit", &id, "--status", "todo"]);
            env.json(&root, &["shelve", &id, "wake", "--defer", "2030-01-01"]);
        }
        env.json(&root, &["note", &id, &"suffix ".repeat(300)]);
        let p = env.json(&root, &["archive-notes", &id]);
        env.json(
            &root,
            &[
                "archive-notes",
                &id,
                "--expect",
                p["source_sha256"].as_str().unwrap(),
            ],
        );
        assert_eq!(env.json(&root, &["show", &id])["task"]["status"], status);
    }
}

#[test]
fn archive_receipts_require_the_exact_boundary() {
    for failure in ["malformed", "unbound", "count", "source-repeat", "position"] {
        let (env, root, id) = archivable();
        let p = env.json(&root, &["archive-notes", &id]);
        let hash = p["source_sha256"].as_str().unwrap();
        if failure == "malformed" || failure == "unbound" {
            let text = if failure == "malformed" {
                "archived-notes: invalid".into()
            } else {
                format!(
                    "archived-notes: v1 source={hash} segment={} count=2",
                    p["archive_name"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("notes-")
                        .trim_end_matches(".notes")
                )
            };
            env.json(&root, &["note", &id, &text]);
        } else {
            env.json(&root, &["archive-notes", &id, "--expect", hash]);
            let path = raw_path(&root, &id);
            let raw = std::fs::read_to_string(&path).unwrap();
            let raw = match failure {
                "count" => raw.replace(" count=2", " count=1"),
                "source-repeat" => format!("{raw}{}\n", raw.lines().last().unwrap()),
                "position" => raw.replacen(
                    "## Notes\n",
                    "## Notes\n\n- 2026-10-31T00:00:00Z (fixture): shifted\n",
                    1,
                ),
                _ => unreachable!(),
            };
            std::fs::write(path, raw).unwrap();
        }
        let bytes = std::fs::read(raw_path(&root, &id)).unwrap();
        assert_eq!(
            env.fail(&root, &["archive-notes", &id]),
            "validation",
            "{failure}"
        );
        assert_eq!(
            env.fail(&root, &["archive-notes", &id, "--expect", hash]),
            "validation",
            "{failure}"
        );
        assert_eq!(std::fs::read(raw_path(&root, &id)).unwrap(), bytes);
    }
}

#[test]
fn archive_growth_and_append_cost() {
    for body_dominant in [false, true] {
        let (env, root, id) = archivable();
        if body_dominant {
            env.json(&root, &["edit", &id, "--body", &"body ".repeat(14000)]);
        }
        env.json(&root, &["note", &id, &"unique evidence ".repeat(5000)]);
        for text in [
            "review: impl round 1 — verdict: accept; findings: none; reviewer: human",
            "concerns: arc-a00001 extension — synthetic follow-up",
            "run: synthetic completed with 1 result",
            "retained: {\"host\":\"fixture-a\",\"kind\":\"scratch\",\"path\":\"$WORK_ROOT/fixture\",\"state\":\"kept\",\"verification\":\"unknown\",\"sha256\":null,\"manifest\":null,\"purpose\":\"synthetic\",\"next_owner\":\"task:arc-a00001\",\"cleanup_trigger\":\"fixture complete\",\"verification_action\":\"inspect fixture\"}",
            "retained: {\"host\":\"fixture-a\",\"kind\":\"scratch\",\"path\":\"$WORK_ROOT/fixture\",\"state\":\"released\",\"verification\":\"verified\",\"purpose\":\"synthetic removal\",\"next_owner\":\"none\",\"cleanup_trigger\":\"none\"}",
            "retention checkpoint: observed 2026-10-30T00:00:00Z on fixture-a; basis synthetic; retained by main; this host: kept verified 0, kept unknown 0, released 1; changed 1.",
            "retention checkpoint: incomplete; observed 2026-10-30T00:00:00Z on fixture-b; basis unavailable: synthetic; Next: inspect fixture.",
        ] {
            env.cmd(&root)
                .env("CODEX_THREAD_ID", "history")
                .args(["note", &id, text, "--stamp"])
                .assert()
                .success();
        }
        let source = root.join("evidence.txt");
        std::fs::write(&source, "exact attachment").unwrap();
        env.json(&root, &["attach", &id, source.to_str().unwrap()]);
        let before = env.json(&root, &["show", &id])["task"].clone();
        let growth_before = env.json(&root, &["check", "--growth"]);
        assert!(!growth_before["warnings"].as_array().unwrap().is_empty());
        let p = env.json(&root, &["archive-notes", &id]);
        env.json(
            &root,
            &[
                "archive-notes",
                &id,
                "--expect",
                p["source_sha256"].as_str().unwrap(),
            ],
        );
        let segment = root.join(format!(
            "tasks/files/{id}/{}",
            p["archive_name"].as_str().unwrap()
        ));
        let segment_bytes = std::fs::read(&segment).unwrap();
        let active_before_append = std::fs::read(raw_path(&root, &id)).unwrap();
        assert!(
            active_before_append.len()
                < growth_before["growth"][0]["record_bytes"].as_u64().unwrap() as usize
        );
        let after = env.json(&root, &["show", &id])["task"].clone();
        let n = before["notes"].as_array().unwrap().len();
        assert_eq!(
            &after["notes"].as_array().unwrap()[..n],
            before["notes"].as_array().unwrap()
        );
        let growth_after = env.json(&root, &["check", "--growth"]);
        assert_eq!(
            growth_after["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["kind"] == "record_growth"),
            body_dominant
        );
        env.json(&root, &["note", &id, "later append"]);
        let active_after_append = std::fs::read(raw_path(&root, &id)).unwrap();
        assert!(active_after_append.len() > active_before_append.len());
        assert!(
            active_after_append.len()
                < growth_before["growth"][0]["record_bytes"].as_u64().unwrap() as usize
        );
        assert_eq!(std::fs::read(segment).unwrap(), segment_bytes);
        assert!(env.check(&root)["errors"].as_array().unwrap().is_empty());
    }
}

#[test]
fn archive_consumer_paths() {
    let (env, root, id) = archivable();
    let before = env.json(&root, &["show", &id])["task"]["notes"]
        .as_array()
        .unwrap()
        .clone();
    let p = env.json(&root, &["archive-notes", &id]);
    env.json(
        &root,
        &[
            "archive-notes",
            &id,
            "--expect",
            p["source_sha256"].as_str().unwrap(),
        ],
    );
    let check = env.json(&root, &["check", "--growth"]);
    assert_eq!(check["growth"].as_array().unwrap().len(), 1);
    assert_eq!(check["growth"][0]["id"], id);
    assert_eq!(check["growth"][0]["note_count"], before.len() + 2);
    assert!(check["errors"].as_array().unwrap().is_empty());
    assert_eq!(
        env.json(&root, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let shown = env.json(&root, &["show", &id]);
    assert_eq!(
        &shown["task"]["notes"].as_array().unwrap()[..before.len()],
        before
    );
    assert_eq!(shown["files"].as_array().unwrap().len(), 1);
    let completion = env
        .cmd(&root)
        .env("TASKS_COMPLETE", "zsh")
        .env("_CLAP_COMPLETE_INDEX", "1")
        .args(["--", "tasks", "archive-n"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&completion.stdout).contains("archive-notes"));
}

#[test]
fn archive_segment_errors_name_record_and_project_across_read_paths() {
    let (mut env, dot, id, _, name, _) = fixture();
    let sci = env.init("sci");
    let sci_id = env.json(&sci, &["add", "Depends on dot", "--depends", &id])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&sci, &["group", "set", "both", "dot", "sci"]);

    let segment_rel = format!("tasks/files/{id}/{name}");
    let record_rel = format!("tasks/{id}.md");
    let mut tampered = std::fs::read(dot.join(&segment_rel)).unwrap();
    tampered.push(b'\n');
    std::fs::write(dot.join(&segment_rel), tampered).unwrap();

    // Single-project reads carry both the record path and the segment path.
    for args in [&["show", id.as_str()][..], &["list"][..], &["ready"][..]] {
        let out = env.cmd(&dot).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let error: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "parse", "{args:?}: {error}");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(detail.contains(&record_rel), "{args:?}: {detail}");
        assert!(detail.contains(&segment_rel), "{args:?}: {detail}");
        assert!(detail.contains("SHA-256 mismatch"), "{args:?}: {detail}");
    }

    // Cross-project reads carry the project prefix, record path, and segment path.
    for args in [
        &["projects"][..],
        &["list", "--all-projects"][..],
        &["ready", "--all-projects"][..],
        &["tree", "--all-projects"][..],
        &["list", "--group", "both"][..],
    ] {
        let out = env.cmd(&sci).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let error: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "parse", "{args:?}: {error}");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(detail.contains("project dot:"), "{args:?}: {detail}");
        assert!(detail.contains(&record_rel), "{args:?}: {detail}");
        assert!(detail.contains(&segment_rel), "{args:?}: {detail}");
    }

    // Foreign dependency resolution in `check` also names the project, record, and segment.
    let check = env.check(&sci);
    let warning = check["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "foreign_unparsable" && w["id"] == sci_id)
        .unwrap_or_else(|| panic!("{check}"));
    let detail = warning["detail"].as_str().unwrap();
    assert!(detail.contains("project dot:"), "{detail}");
    assert!(detail.contains(&record_rel), "{detail}");
    assert!(detail.contains(&segment_rel), "{detail}");
}
