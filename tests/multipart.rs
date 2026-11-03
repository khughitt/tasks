mod common;
use common::TestEnv;
use std::path::Path;

fn configure(dir: &Path, attachments: &str) {
    let config = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!("\n[attachments]\n{attachments}\n"));
    std::fs::write(config, text).unwrap();
}

#[test]
fn config_rejects_limits_above_the_recovery_ceiling() {
    for (key, value, ceiling) in [
        ("multipart_max_bytes", "268435457", "268435456"),
        ("multipart_max_parts", "257", "256"),
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        configure(&dir, &format!("{key} = {value}"));
        let output = env.cmd(&dir).arg("list").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(detail.contains(key), "{detail}");
        assert!(detail.contains(ceiling), "{detail}");
    }
}

#[test]
fn config_accepts_zero_bytes_and_an_empty_validator_list() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    configure(&dir, "multipart_max_bytes = 0\n multipart_validators = []");
    env.json(&dir, &["list"]);
}

#[test]
fn config_rejects_a_validator_without_command() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    configure(&dir, "multipart_validators = [{ match = \"*.md\" }]");
    let output = env.cmd(&dir).arg("list").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("command")
    );
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn stored(dir: &Path, id: &str, name: &str) -> std::path::PathBuf {
    dir.join("tasks/files").join(id).join(name)
}

fn declare(env: &TestEnv, dir: &Path, id: &str, catalog: &serde_json::Value) {
    let name = catalog["name"].as_str().unwrap();
    let bytes = std::fs::read(stored(dir, id, &format!("{name}.catalog.json"))).unwrap();
    env.json(
        dir,
        &[
            "note",
            id,
            &format!(
                "multipart: {name} ({} bytes, {} parts, sha256 {}, catalog sha256 {})",
                catalog["bytes"],
                catalog["parts"].as_array().unwrap().len(),
                catalog["sha256"].as_str().unwrap(),
                digest(&bytes)
            ),
        ],
    );
}

fn seed_artifact(
    env: &TestEnv,
    dir: &Path,
    id: &str,
    name: &str,
    whole: &[u8],
    cap: usize,
) -> serde_json::Value {
    configure(dir, &format!("max_bytes = {cap}"));
    let scratch = dir.join("seed");
    std::fs::create_dir_all(&scratch).unwrap();
    let chunks: Vec<_> = whole.chunks(cap).collect();
    let mut parts = Vec::new();
    for (index, bytes) in chunks.iter().enumerate() {
        let member = format!("{name}.part-{:04}", index + 1);
        let source = scratch.join(&member);
        std::fs::write(&source, bytes).unwrap();
        env.json(
            dir,
            &[
                "attach",
                id,
                source.to_str().unwrap(),
                "--caption",
                &format!("part {}/{} of {name}", index + 1, chunks.len()),
            ],
        );
        parts.push(
            serde_json::json!({"name": member, "bytes": bytes.len(), "sha256": digest(bytes)}),
        );
    }
    let catalog = serde_json::json!({"version": 1, "name": name, "bytes": whole.len(), "sha256": digest(whole), "parts": parts});
    let source = scratch.join(format!("{name}.catalog.json"));
    let bytes = serde_json::to_vec(&catalog).unwrap();
    assert!(bytes.len() <= cap);
    std::fs::write(&source, bytes).unwrap();
    env.json(
        dir,
        &[
            "attach",
            id,
            source.to_str().unwrap(),
            "--caption",
            &format!("catalog of {name}"),
        ],
    );
    declare(env, dir, id, &catalog);
    catalog
}

fn task(env: &TestEnv, dir: &Path) -> String {
    env.json(dir, &["add", "Evidence"])["id"]
        .as_str()
        .unwrap()
        .into()
}

fn broken(env: &TestEnv, dir: &Path, name: &str, member: &str) -> serde_json::Value {
    let out = env.cmd(dir).arg("check").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"].as_array().unwrap().iter().any(|finding| {
            finding["kind"] == "multipart_broken"
                && finding["detail"].as_str().unwrap().contains(name)
                && finding.to_string().contains(member)
        }),
        "{report}"
    );
    report
}

#[test]
fn check_is_clean_for_an_intact_artifact() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    seed_artifact(&env, &dir, &id, "evidence.txt", &vec![b'x'; 2050], 1024);
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
}

#[test]
fn check_reports_missing_extra_detached_and_altered_parts() {
    for case in [
        "missing", "extra", "detached", "altered", "oversize", "symlink",
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        let id = task(&env, &dir);
        seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
        let member = "evidence.txt.part-0001";
        let path = stored(&dir, &id, member);
        match case {
            "missing" => std::fs::remove_file(path).unwrap(),
            "extra" => {
                let source = dir.join("evidence.txt.part-0002");
                std::fs::write(&source, b"extra").unwrap();
                env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
            }
            "detached" => {
                env.json(
                    &dir,
                    &["note", &id, &format!("detached: {member}: manual drift")],
                );
            }
            "altered" => std::fs::write(path, b"abcdeg").unwrap(),
            "oversize" => std::fs::write(path, vec![b'x'; 1025]).unwrap(),
            "symlink" => {
                std::fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink(dir.join("seed").join(member), path).unwrap();
            }
            _ => unreachable!(),
        }
        broken(
            &env,
            &dir,
            "evidence.txt",
            if case == "extra" {
                "evidence.txt.part-0002"
            } else {
                member
            },
        );
    }
}

#[test]
fn check_reports_a_missing_or_altered_catalog_against_the_declaration() {
    for missing in [true, false] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        let id = task(&env, &dir);
        seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
        let path = stored(&dir, &id, "evidence.txt.catalog.json");
        if missing {
            std::fs::remove_file(path).unwrap();
        } else {
            std::fs::write(path, b"{}").unwrap();
        }
        broken(&env, &dir, "evidence.txt", "evidence.txt.catalog.json");
    }
}

#[test]
fn check_reports_out_of_order_catalog_rows() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    let mut catalog = seed_artifact(&env, &dir, &id, "evidence.txt", &vec![b'x'; 2050], 1024);
    catalog["parts"].as_array_mut().unwrap().swap(0, 1);
    std::fs::write(
        stored(&dir, &id, "evidence.txt.catalog.json"),
        serde_json::to_vec(&catalog).unwrap(),
    )
    .unwrap();
    declare(&env, &dir, &id, &catalog);
    broken(&env, &dir, "evidence.txt", "evidence.txt.catalog.json");
}

#[test]
fn check_reports_invalid_json_in_the_rebuilt_whole() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    seed_artifact(&env, &dir, &id, "evidence.json", b"{invalid}", 1024);
    broken(&env, &dir, "evidence.json", "evidence.json");
}

#[test]
fn check_reports_leftovers_of_a_closed_declaration_naming_the_rerun() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
    env.json(&dir, &["note", &id, "detached: evidence.txt: obsolete"]);
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"].as_array().unwrap().iter().any(|finding| {
            finding["kind"] == "multipart_leftover"
                && finding["detail"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("tasks detach {id} evidence.txt"))
        }),
        "{report}"
    );
}

#[test]
fn check_ignores_an_artifact_above_current_admission_limits() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
    let config = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("multipart_max_bytes = 1\nmultipart_max_parts = 1\n");
    std::fs::write(config, text).unwrap();
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
}

#[test]
fn check_reports_a_declaration_beyond_the_recovery_ceiling() {
    for (bytes, parts) in [(268435457u64, 1usize), (1, 257), (1, usize::MAX)] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        let id = task(&env, &dir);
        env.json(&dir, &["note", &id, &format!("multipart: evidence.txt ({bytes} bytes, {parts} parts, sha256 {}, catalog sha256 {})", "a".repeat(64), "b".repeat(64))]);
        broken(&env, &dir, "evidence.txt", "evidence.txt");
    }
}

#[test]
fn check_reports_unnoted_members_without_a_declaration_as_attachment_unnoted() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    let path = stored(&dir, &id, "evidence.txt.part-0001");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"interrupted").unwrap();
    let out = env.cmd(&dir).arg("check").output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report["errors"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "attachment_unnoted"
            && finding["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("tasks attach {id}"))
    }));
}

#[test]
fn check_reports_catalog_totals_and_whole_digest_mismatches() {
    for case in ["total", "whole", "catalog-name", "catalog-count"] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        let id = task(&env, &dir);
        let mut catalog = seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
        match case {
            "total" => catalog["parts"][0]["bytes"] = serde_json::json!(7),
            "whole" => catalog["sha256"] = serde_json::json!("a".repeat(64)),
            "catalog-name" => catalog["name"] = serde_json::json!("other.txt"),
            "catalog-count" => catalog["parts"] = serde_json::json!([]),
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&catalog).unwrap();
        std::fs::write(stored(&dir, &id, "evidence.txt.catalog.json"), &bytes).unwrap();
        env.json(
            &dir,
            &[
                "note",
                &id,
                &format!(
                    "multipart: evidence.txt (6 bytes, 1 parts, sha256 {}, catalog sha256 {})",
                    catalog["sha256"].as_str().unwrap(),
                    digest(&bytes)
                ),
            ],
        );
        broken(&env, &dir, "evidence.txt", "evidence.txt");
    }
}

#[test]
fn check_accepts_a_later_declaration_after_logical_detach() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    let catalog = seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
    env.json(&dir, &["note", &id, "detached: evidence.txt: obsolete"]);
    declare(&env, &dir, &id, &catalog);
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
}

#[test]
fn check_ignores_members_detached_by_a_task_move() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let destination = env.init("dst");
    let id = task(&env, &dir);
    seed_artifact(&env, &dir, &id, "evidence.txt", b"abcdef", 1024);
    env.json(&dir, &["move", &id, "--to", "dst"]);
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
    assert_eq!(env.check(&destination)["errors"], serde_json::json!([]));
}
