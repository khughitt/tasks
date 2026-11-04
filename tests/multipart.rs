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
                // The writer refuses this drift; seed corruption for the auditor directly.
                std::fs::write(stored(&dir, &id, "evidence.txt.part-0002"), b"extra").unwrap();
                env.json(
                    &dir,
                    &[
                        "note",
                        &id,
                        "attached: evidence.txt.part-0002 (5 bytes): manual drift",
                    ],
                );
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

fn attach_fixture(config: &str) -> (TestEnv, std::path::PathBuf, String) {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    configure(&dir, config);
    let id = common::id_of(env.json(&dir, &["add", "Evidence"]));
    (env, dir, id)
}
fn multipart_attach(
    env: &TestEnv,
    dir: &Path,
    id: &str,
    name: &str,
    bytes: &[u8],
) -> serde_json::Value {
    let source = dir.join("source");
    std::fs::write(&source, bytes).unwrap();
    env.json(
        dir,
        &[
            "attach",
            id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            name,
            "--caption",
            "whole document",
        ],
    )
}
fn git_text(dir: &Path, args: &[&str]) -> String {
    let home = tempfile::tempdir().unwrap();
    let out = common::git_fixture::command(home.path(), dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn enable_git(dir: &Path) {
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    common::git(dir, &["init", "-q", "-b", "main"]);
    common::git(dir, &["add", "tasks"]);
    common::git(dir, &["commit", "-qm", "fixture"]);
}
#[test]
fn multipart_attach_round_trips_exactly() {
    for bytes in [
        "abc\né日\n".repeat(15000).into_bytes(),
        "é日".repeat(30000).into_bytes(),
    ] {
        let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
        let out = multipart_attach(&env, &dir, &id, "evidence.txt", &bytes);
        assert_eq!(out["multipart"]["sha256"], digest(&bytes));
        assert_eq!(out["multipart"]["caption"], "whole document");
        let cat: serde_json::Value = serde_json::from_slice(
            &std::fs::read(stored(&dir, &id, "evidence.txt.catalog.json")).unwrap(),
        )
        .unwrap();
        let rebuilt: Vec<u8> = cat["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| std::fs::read(stored(&dir, &id, p["name"].as_str().unwrap())).unwrap())
            .collect();
        assert_eq!(rebuilt, bytes);
        assert!(out["validation"]["passed"].as_bool().unwrap());
        assert!(env.check(&dir)["errors"].as_array().unwrap().is_empty());
    }
}
#[test]
fn multipart_attach_is_one_save_and_one_commit() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let before: usize = git_text(&dir, &["rev-list", "--count", "HEAD"])
        .parse()
        .unwrap();
    let out = multipart_attach(&env, &dir, &id, "evidence.txt", &vec![b'a'; 140000]);
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(out["publication"]["commit"], commit);
    assert_eq!(
        git_text(&dir, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    let files = git_text(&dir, &["show", "--pretty=", "--name-only", &commit]);
    assert_eq!(files.lines().count(), 5);
    for file in out["files"].as_array().unwrap() {
        assert!(files.contains(file["name"].as_str().unwrap()));
    }
}
#[test]
fn multipart_admits_exactly_the_byte_limit() {
    let (env, dir, id) =
        attach_fixture("max_bytes = 65536\nmultipart_max_bytes = 1000\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "exact.txt", &vec![b'a'; 1000]);
    let before = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    let source = dir.join("source");
    std::fs::write(&source, vec![b'a'; 1001]).unwrap();
    let out = env
        .cmd(&dir)
        .args([
            "attach",
            &id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            "over.txt",
        ])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "multipart_too_large");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("multipart_max_bytes")
    );
    assert_eq!(
        before,
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap()
    );
    assert!(!stored(&dir, &id, "over.txt.part-0001").exists());
}
#[test]
fn multipart_refusals_write_nothing() {
    for (config, name, bytes, kind) in [
        (
            "max_bytes = 65536\nmultipart_validators = []",
            "a.txt",
            vec![255],
            "multipart_not_text",
        ),
        (
            "max_bytes = 65536\nmultipart_validators = []",
            "a.json",
            b"{bad}".to_vec(),
            "multipart_invalid_content",
        ),
        (
            "max_bytes = 65536",
            "a.txt",
            b"abc".to_vec(),
            "multipart_validators_undeclared",
        ),
        (
            "max_bytes = 1\nmultipart_validators = []",
            "a.txt",
            "é".as_bytes().to_vec(),
            "multipart_uncuttable",
        ),
        (
            "max_bytes = 1\nmultipart_validators = []",
            "a.txt",
            b"abc".to_vec(),
            "attachment_too_large",
        ),
        (
            "max_bytes = 65536\nmultipart_max_bytes = 0\nmultipart_validators = []",
            "a.txt",
            vec![],
            "multipart_too_large",
        ),
        (
            "max_bytes = 65536\nmultipart_max_parts = 0\nmultipart_validators = []",
            "a.txt",
            vec![],
            "multipart_too_large",
        ),
        (
            "max_bytes = 65536\nmultipart_max_parts = 1\nmultipart_validators = []",
            "a.txt",
            vec![b'x'; 65537],
            "multipart_too_large",
        ),
        (
            "max_bytes = 65536\nmultipart_validators = [{match='*',command=['sh','-c','! grep -q TOKEN']}]",
            "a.txt",
            [vec![b'a'; 65534], b"TOKEN".to_vec()].concat(),
            "multipart_invalid_content",
        ),
    ] {
        let (env, dir, id) = attach_fixture(config);
        let source = dir.join("source");
        std::fs::write(&source, bytes).unwrap();
        let record = dir.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&record).unwrap();
        assert_eq!(
            env.fail(
                &dir,
                &[
                    "attach",
                    &id,
                    source.to_str().unwrap(),
                    "--multipart",
                    "--name",
                    name
                ]
            ),
            kind,
            "{config}"
        );
        assert_eq!(std::fs::read(record).unwrap(), before);
        assert!(!dir.join(format!("tasks/files/{id}")).exists());
    }
}
#[test]
fn multipart_source_constraints() {
    let (env, dir, id) = attach_fixture("multipart_validators = []");
    env.usage(&dir, &["attach", &id, "--multipart", "--clipboard"]);
    assert_eq!(
        env.fail(&dir, &["attach", &id, "one", "two", "--multipart"]),
        "validation"
    );
    assert!(!dir.join(format!("tasks/files/{id}")).exists());
}
#[test]
fn one_part_artifact_when_it_fits() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    assert_eq!(out["multipart"]["parts"], 1);
    assert_eq!(out["files"].as_array().unwrap().len(), 2);
    assert_eq!(out["publication"]["cause"], "commit_disabled");
    assert!(
        out["publication"]["next"]
            .as_str()
            .unwrap()
            .contains("tasks commit")
    );
}
#[test]
fn ordinary_attach_refuses_a_declared_member_name() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    let source = dir.join("source");
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--name",
                "a.txt.part-0001"
            ]
        ),
        "multipart_member"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--multipart",
                "--name",
                "a.txt"
            ]
        ),
        "attachment_exists"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--multipart",
                "--name",
                "a.txt.part-0001"
            ]
        ),
        "multipart_member"
    );
}
#[test]
fn oversize_without_flag_names_multipart() {
    let (env, dir, id) = attach_fixture("max_bytes = 1");
    let source = dir.join("source");
    std::fs::write(&source, "ab").unwrap();
    let out = env
        .cmd(&dir)
        .args(["attach", &id, source.to_str().unwrap()])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "attachment_too_large");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--multipart")
    );
}
#[test]
fn publication_local_for_each_cause() {
    for cause in [
        "commit_deferred",
        "commit_failed",
        "validation_failed",
        "validation_unavailable",
        "commit_unavailable",
    ] {
        let validators = if cause.starts_with("validation_") {
            "multipart_validators = [{match='*', command=['sh','-c','if test -e ran; then exit 3; fi; touch ran']}]"
        } else {
            "multipart_validators = []"
        };
        let (env, dir, id) = attach_fixture(&format!("max_bytes = 65536\n{validators}"));
        if cause == "commit_unavailable" {
            let config = dir.join("tasks/.config.toml");
            let text = std::fs::read_to_string(&config).unwrap();
            std::fs::write(config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
        } else {
            enable_git(&dir);
            if cause == "commit_deferred" {
                std::fs::write(dir.join("unrelated"), "x").unwrap();
                common::git(&dir, &["add", "unrelated"]);
            }
            if cause == "commit_failed" || cause == "validation_unavailable" {
                use std::os::unix::fs::PermissionsExt;
                let hook = dir.join(".git/hooks/pre-commit");
                let body = if cause == "commit_failed" {
                    "#!/bin/sh\nexit 1\n"
                } else {
                    "#!/bin/sh\nrm -f validator\n"
                };
                std::fs::write(&hook, body).unwrap();
                std::fs::set_permissions(hook, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        if cause == "validation_unavailable" {
            let config = dir.join("tasks/.config.toml");
            let text = std::fs::read_to_string(&config).unwrap().replace(
                "command=['sh','-c','if test -e ran; then exit 3; fi; touch ran']",
                "command=['./validator']",
            );
            std::fs::write(config, text).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(dir.join("validator"), "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(
                dir.join("validator"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
        assert_eq!(out["publication"]["state"], "local", "{cause}: {out}");
        assert_eq!(out["publication"]["cause"], cause, "{out}");
        if cause == "commit_unavailable" {
            assert!(
                out["publication"]["next"]
                    .as_str()
                    .unwrap()
                    .contains("Git work tree")
            );
        }
    }
}
#[test]
fn interrupted_attach_reruns_adopt_and_commit_every_member() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let bytes = vec![b'x'; 140000];
    let member = stored(&dir, &id, "a.txt.part-0001");
    std::fs::create_dir_all(member.parent().unwrap()).unwrap();
    std::fs::write(member, &bytes[..65536]).unwrap();
    std::fs::write(stored(&dir, &id, "a.txt.part-0002"), &bytes[65536..131072]).unwrap();
    let out = multipart_attach(&env, &dir, &id, "a.txt", &bytes);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    let tree = git_text(&dir, &["ls-tree", "-r", "--name-only", "HEAD"]);
    for file in out["files"].as_array().unwrap() {
        assert!(tree.contains(file["name"].as_str().unwrap()));
    }
}

fn hook(dir: &Path, name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(format!(".git/hooks/{name}"));
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn publication_binds_current_validators_to_exact_config_bytes() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    hook(
        &dir,
        "post-commit",
        r#"sed -i "s/multipart_validators = \[\]/multipart_validators = [{match='*', command=['sh','-c','exit 3']}] /" tasks/.config.toml"#,
    );
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    assert_eq!(out["publication"]["cause"], "validation_failed", "{out}");
    assert_eq!(
        out["validation"]["config_sha256"],
        digest(&std::fs::read(dir.join("tasks/.config.toml")).unwrap())
    );
    assert_eq!(out["validation"]["validators"][0]["exit"], 3);
}

#[test]
fn publication_invalid_runtime_config_is_unavailable() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    hook(
        &dir,
        "post-commit",
        "printf 'broken = [' > tasks/.config.toml",
    );
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    assert_eq!(
        out["publication"]["cause"], "validation_unavailable",
        "{out}"
    );
    assert_ne!(out["publication"]["state"], "published");
    assert!(
        out.get("validation").is_none(),
        "setup failure must not retain an earlier passing report: {out}"
    );
}

#[test]
fn publication_commit_guard_detects_hook_corruption() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    hook(
        &dir,
        "pre-commit",
        &format!(
            "printf altered > tasks/files/{id}/a.txt.part-0001\ngit add tasks/files/{id}/a.txt.part-0001"
        ),
    );
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    assert_eq!(out["publication"]["cause"], "commit_failed", "{out}");
}

#[test]
fn publication_validates_committed_bytes_after_working_bytes_change() {
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['sh','-c','test \"$(cat)\" = hello']}]",
    );
    enable_git(&dir);
    hook(
        &dir,
        "post-commit",
        &format!("printf altered > tasks/files/{id}/a.txt.part-0001"),
    );
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(
        std::fs::read(stored(&dir, &id, "a.txt.part-0001")).unwrap(),
        b"altered"
    );
    assert_eq!(
        git_text(
            &dir,
            &["show", &format!("HEAD:tasks/files/{id}/a.txt.part-0001")]
        ),
        "hello"
    );
}

#[test]
fn multipart_collisions_refuse_without_writing_members_or_record() {
    for live in [false, true] {
        let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
        let source = dir.join("source");
        std::fs::write(&source, "different").unwrap();
        if live {
            env.json(
                &dir,
                &[
                    "attach",
                    &id,
                    source.to_str().unwrap(),
                    "--name",
                    "a.txt.part-0001",
                ],
            );
        } else {
            std::fs::create_dir_all(stored(&dir, &id, "a.txt.part-0001").parent().unwrap())
                .unwrap();
            std::fs::write(stored(&dir, &id, "a.txt.part-0001"), "different").unwrap();
        }
        let record = dir.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&record).unwrap();
        std::fs::write(&source, vec![b'x'; 70000]).unwrap();
        assert_eq!(
            env.fail(
                &dir,
                &[
                    "attach",
                    &id,
                    source.to_str().unwrap(),
                    "--multipart",
                    "--name",
                    "a.txt"
                ]
            ),
            "attachment_exists"
        );
        assert_eq!(std::fs::read(record).unwrap(), before);
        assert_eq!(
            std::fs::read_dir(stored(&dir, &id, "a.txt.part-0001").parent().unwrap())
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn multipart_part_limit_error_names_the_setting() {
    let (env, dir, id) =
        attach_fixture("max_bytes = 65536\nmultipart_max_parts = 1\nmultipart_validators = []");
    let source = dir.join("source");
    std::fs::write(&source, vec![b'x'; 65537]).unwrap();
    let out = env
        .cmd(&dir)
        .args(["attach", &id, source.to_str().unwrap(), "--multipart"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "multipart_too_large");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("multipart_max_parts")
    );
    assert!(!dir.join(format!("tasks/files/{id}")).exists());
}

#[test]
fn multipart_stdin_uses_the_same_admission_and_storage() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    let out = env
        .cmd(&dir)
        .args(["attach", &id, "-", "--name", "a.txt", "--multipart"])
        .write_stdin("hello\n")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        std::fs::read(stored(&dir, &id, "a.txt.part-0001")).unwrap(),
        b"hello\n"
    );
    assert_eq!(
        env.fail(&dir, &["attach", &id, "-", "--multipart"]),
        "invalid_attachment_name"
    );
}

#[test]
fn multipart_save_failure_rolls_back_new_members_and_keeps_adopted_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    let source = dir.join("source");
    let bytes = vec![b'x'; 70000];
    std::fs::write(&source, &bytes).unwrap();
    let first = stored(&dir, &id, "a.txt.part-0001");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, &bytes[..65536]).unwrap();
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    let tasks = dir.join("tasks");
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o555)).unwrap();
    let out = env
        .cmd(&dir)
        .args([
            "attach",
            &id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            "a.txt",
        ])
        .output()
        .unwrap();
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "io");
    assert_eq!(std::fs::read(record).unwrap(), before);
    assert_eq!(std::fs::read(first).unwrap(), bytes[..65536]);
    assert!(!stored(&dir, &id, "a.txt.part-0002").exists());
    assert!(!stored(&dir, &id, "a.txt.catalog.json").exists());
}

#[test]
fn multipart_namespace_collision_before_declaration_writes_nothing() {
    for state in ["live", "unnoted", "missing-live"] {
        let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
        let source = dir.join("source");
        std::fs::write(&source, "hello").unwrap();
        let extra = stored(&dir, &id, "a.txt.part-0002");
        if state == "unnoted" {
            std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
            std::fs::write(&extra, "existing").unwrap();
        } else {
            env.json(
                &dir,
                &[
                    "attach",
                    &id,
                    source.to_str().unwrap(),
                    "--name",
                    "a.txt.part-0002",
                ],
            );
            if state == "missing-live" {
                std::fs::remove_file(&extra).unwrap();
            }
        }
        let record = dir.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&record).unwrap();
        assert_eq!(
            env.fail(
                &dir,
                &[
                    "attach",
                    &id,
                    source.to_str().unwrap(),
                    "--multipart",
                    "--name",
                    "a.txt"
                ]
            ),
            "attachment_exists",
            "{state}"
        );
        assert_eq!(std::fs::read(record).unwrap(), before);
        assert!(!stored(&dir, &id, "a.txt.part-0001").exists());
        assert!(!stored(&dir, &id, "a.txt.catalog.json").exists());
        assert_eq!(extra.exists(), state != "missing-live");
        if state != "missing-live" {
            assert_eq!(
                std::fs::read(extra).unwrap(),
                if state == "unnoted" {
                    b"existing".as_slice()
                } else {
                    b"hello".as_slice()
                }
            );
        }
    }
}

#[test]
fn multipart_namespace_collision_after_declaration_writes_nothing() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "a.txt", b"hello");
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    for name in ["a.txt.part-0000", "a.txt.part-0002", "a.txt.part-9999"] {
        let source = dir.join("source");
        assert_eq!(
            env.fail(
                &dir,
                &["attach", &id, source.to_str().unwrap(), "--name", name]
            ),
            "multipart_member"
        );
        assert_eq!(std::fs::read(&record).unwrap(), before);
        assert!(!stored(&dir, &id, name).exists());
    }
    assert!(env.check(&dir)["errors"].as_array().unwrap().is_empty());
}

#[test]
fn publication_hashes_the_executable_selected_from_path() {
    use std::os::unix::fs::PermissionsExt;
    for case in [
        "repository",
        "external",
        "relative",
        "empty",
        "skip-non-executable",
    ] {
        let (env, dir, id) = attach_fixture(
            "max_bytes = 65536\nmultipart_validators = [{match='*',command=['check-doc']}]",
        );
        enable_git(&dir);
        let external = tempfile::tempdir().unwrap();
        let tools = dir.join("tools");
        std::fs::create_dir(&tools).unwrap();
        let selected = b"#!/bin/sh\nprintf selected > ran\n";
        let wrong = b"#!/bin/sh\nprintf wrong > ran\n";
        let local_program = if case == "empty" {
            dir.join("check-doc")
        } else {
            tools.join("check-doc")
        };
        std::fs::write(
            &local_program,
            if case == "external" {
                wrong.as_slice()
            } else {
                selected.as_slice()
            },
        )
        .unwrap();
        std::fs::set_permissions(&local_program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let other = external.path().join("check-doc");
        std::fs::write(
            &other,
            if case == "skip-non-executable" {
                wrong.as_slice()
            } else {
                selected.as_slice()
            },
        )
        .unwrap();
        std::fs::set_permissions(
            &other,
            std::fs::Permissions::from_mode(if case == "skip-non-executable" {
                0o644
            } else {
                0o755
            }),
        )
        .unwrap();
        let path = match case {
            "relative" => "tools:/usr/bin:/bin".into(),
            "empty" => ":/usr/bin:/bin".into(),
            "external" | "skip-non-executable" => format!(
                "{}:{}:/usr/bin:/bin",
                external.path().display(),
                tools.display()
            ),
            _ => format!("{}:/usr/bin:/bin", tools.display()),
        };
        let source = dir.join("source");
        std::fs::write(&source, "hello").unwrap();
        let out = env
            .cmd(&dir)
            .env("PATH", path)
            .args([
                "attach",
                &id,
                source.to_str().unwrap(),
                "--multipart",
                "--name",
                "a.txt",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{case}: {out:?}");
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            value["publication"]["state"], "published",
            "{case}: {value}"
        );
        assert_eq!(
            std::fs::read(dir.join("ran")).unwrap(),
            b"selected",
            "{case}"
        );
        let validation = &value["validation"]["validators"][0];
        assert_eq!(validation["command"][0], "check-doc");
        if case == "external" {
            assert_eq!(validation["program"], "external");
            assert!(validation.get("program_sha256").is_none());
        } else {
            assert_eq!(
                validation["program_sha256"],
                digest(selected),
                "{case}: {value}"
            );
            assert!(validation.get("program").is_none());
        }
    }
}

#[test]
fn multipart_missing_path_validator_fails_closed_before_writes() {
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['absent-validator-for-task-test']}]",
    );
    let source = dir.join("source");
    std::fs::write(&source, "hello").unwrap();
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--multipart",
                "--name",
                "a.txt"
            ]
        ),
        "multipart_invalid_content"
    );
    assert_eq!(std::fs::read(record).unwrap(), before);
    assert!(!stored(&dir, &id, "a.txt.part-0001").exists());
}

#[test]
fn multipart_path_identity_never_falls_through_to_another_executable() {
    use std::os::unix::fs::PermissionsExt;
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['check-doc']}]",
    );
    let first = dir.join("first");
    let second = dir.join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    for (path, bytes) in [
        (
            first.join("check-doc"),
            b"#!/missing-interpreter-for-task-validator\n".as_slice(),
        ),
        (
            second.join("check-doc"),
            b"#!/bin/sh\nprintf wrongly-ran > second-ran\n".as_slice(),
        ),
    ] {
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let source = dir.join("source");
    std::fs::write(&source, "hello").unwrap();
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    let out = env
        .cmd(&dir)
        .env(
            "PATH",
            format!("{}:{}:/usr/bin:/bin", first.display(), second.display()),
        )
        .args([
            "attach",
            &id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            "a.txt",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "multipart_invalid_content");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("validation_unavailable")
    );
    assert!(!dir.join("second-ran").exists());
    assert_eq!(std::fs::read(record).unwrap(), before);
    assert!(!stored(&dir, &id, "a.txt.part-0001").exists());
}

#[test]
fn publication_path_selection_preserves_argv_zero() {
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['argv-check','-c','printf %s \"$0\" > ran']}]",
    );
    enable_git(&dir);
    let tools = dir.join("tools");
    std::fs::create_dir(&tools).unwrap();
    std::os::unix::fs::symlink("/bin/sh", tools.join("argv-check")).unwrap();
    let source = dir.join("source");
    std::fs::write(&source, "hello").unwrap();
    let out = env
        .cmd(&dir)
        .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
        .args([
            "attach",
            &id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            "a.txt",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["publication"]["state"], "published", "{value}");
    assert_eq!(std::fs::read(dir.join("ran")).unwrap(), b"argv-check");
    assert_eq!(
        value["validation"]["validators"][0]["command"][0],
        "argv-check"
    );
}

#[test]
fn publication_path_selection_keeps_the_selected_symlink_path() {
    use std::os::unix::fs::PermissionsExt;
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['check-doc']}]",
    );
    enable_git(&dir);
    let tools = dir.join("tools");
    let actual = dir.join("actual");
    std::fs::create_dir(&tools).unwrap();
    std::fs::create_dir(&actual).unwrap();
    let script = b"#!/bin/sh\nbase=${0%/*}\ncat \"$base/resource\" > ran\n";
    std::fs::write(actual.join("check-doc"), script).unwrap();
    std::fs::set_permissions(
        actual.join("check-doc"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::write(actual.join("resource"), "wrong canonical location").unwrap();
    std::fs::write(tools.join("resource"), "selected location").unwrap();
    std::os::unix::fs::symlink("../actual/check-doc", tools.join("check-doc")).unwrap();
    let source = dir.join("source");
    std::fs::write(&source, "hello").unwrap();
    let out = env
        .cmd(&dir)
        .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
        .args([
            "attach",
            &id,
            source.to_str().unwrap(),
            "--multipart",
            "--name",
            "a.txt",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["publication"]["state"], "published", "{value}");
    assert_eq!(
        std::fs::read(dir.join("ran")).unwrap(),
        b"selected location"
    );
    assert_eq!(
        value["validation"]["validators"][0]["program_sha256"],
        digest(script)
    );
}
