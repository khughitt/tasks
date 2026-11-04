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

const DETACH_MEMBERS: [&str; 4] = [
    "evidence.txt.part-0001",
    "evidence.txt.part-0002",
    "evidence.txt.part-0003",
    "evidence.txt.catalog.json",
];

fn detach_fixture() -> (TestEnv, std::path::PathBuf, String) {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let out = multipart_attach(&env, &dir, &id, "evidence.txt", &vec![b'x'; 140000]);
    assert_eq!(out["publication"]["state"], "published");
    (env, dir, id)
}

fn commit_switch(dir: &Path, enabled: bool) {
    let path = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        text.replace(
            &format!("commit = {}", !enabled),
            &format!("commit = {enabled}"),
        ),
    )
    .unwrap();
}

fn close_notes(env: &TestEnv, dir: &Path, id: &str) {
    let mut notes: Vec<_> = DETACH_MEMBERS
        .iter()
        .map(|member| format!("detached: {member}: part of evidence.txt"))
        .collect();
    notes.push("detached: evidence.txt: obsolete".into());
    env.cmd(dir)
        .args(["note", id, "--stdin"])
        .write_stdin(format!("{}\n", notes.join("\n")))
        .assert()
        .success();
}

fn assert_removal_clone(dir: &Path, id: &str, commit: &str) {
    let clone = tempfile::tempdir().unwrap();
    common::git(clone.path(), &["clone", "-q", dir.to_str().unwrap(), "."]);
    common::git(clone.path(), &["checkout", "-q", commit]);
    let env = TestEnv::new();
    let task = env.json(clone.path(), &["show", id]);
    assert!(
        task["task"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note["text"]
                .as_str()
                .unwrap()
                .starts_with("detached: evidence.txt: "))
    );
    for member in DETACH_MEMBERS {
        assert!(!stored(clone.path(), id, member).exists(), "{member}");
    }
    let check = env.check(clone.path());
    assert_eq!(check["errors"], serde_json::json!([]), "{check}");
    assert_eq!(check["warnings"], serde_json::json!([]), "{check}");
}

#[test]
fn detach_by_logical_name_removes_the_set_in_one_commit() {
    let (env, dir, id) = detach_fixture();
    let before = git_text(&dir, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(out["publication"]["commit"], commit);
    assert_eq!(
        git_text(&dir, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    assert_eq!(
        git_text(&dir, &["show", "--pretty=", "--name-only", "HEAD"])
            .lines()
            .count(),
        5
    );
    assert_removal_clone(&dir, &id, &commit);
}

#[test]
fn detaching_a_member_refuses_naming_the_artifact() {
    let (env, dir, id) = detach_fixture();
    for member in DETACH_MEMBERS {
        let out = env
            .cmd(&dir)
            .args(["detach", &id, member, "obsolete"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "multipart_member");
        assert!(
            error["error"]["detail"]
                .as_str()
                .unwrap()
                .contains("evidence.txt")
        );
    }
    assert!(git_text(&dir, &["status", "--porcelain", "--", "tasks"]).is_empty());
}

#[test]
fn detach_interrupted_at_each_point_recovers() {
    for removed in [vec![], vec![0], vec![3], vec![0, 1, 2, 3]] {
        let (env, dir, id) = detach_fixture();
        commit_switch(&dir, false);
        close_notes(&env, &dir, &id);
        for index in removed {
            std::fs::remove_file(stored(&dir, &id, DETACH_MEMBERS[index])).unwrap();
        }
        let record = dir.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&record).unwrap();
        let count = git_text(&dir, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap();
        commit_switch(&dir, true);
        let out = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
        assert_eq!(out["publication"]["state"], "published", "{out}");
        assert_eq!(std::fs::read(&record).unwrap(), before);
        assert_eq!(
            git_text(&dir, &["rev-list", "--count", "HEAD"])
                .parse::<usize>()
                .unwrap(),
            count + 1
        );
        assert_removal_clone(&dir, &id, out["publication"]["commit"].as_str().unwrap());
    }
}

#[test]
fn rerun_after_a_complete_detach_returns_published_without_writing() {
    let (env, dir, id) = detach_fixture();
    let first = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    let record = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    let second = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
    assert_eq!(second["publication"], first["publication"]);
    assert_eq!(second["removed"], false);
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(),
        record
    );
    assert_eq!(
        git_text(&dir, &["rev-parse", "HEAD"]),
        first["publication"]["commit"]
    );
}

#[test]
fn reuse_before_committed_removal_is_refused() {
    let (env, dir, id) = detach_fixture();
    commit_switch(&dir, false);
    let detached = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(detached["publication"]["cause"], "commit_disabled");
    assert_eq!(detached["publication"]["reuse"], "blocked");
    let record = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    let source = dir.join("small.txt");
    std::fs::write(&source, "small").unwrap();
    for name in ["evidence.txt", "evidence.txt.part-0002"] {
        for multipart in [false, true] {
            let mut args = vec!["attach", &id, source.to_str().unwrap(), "--name", name];
            if multipart {
                args.push("--multipart");
            }
            assert_eq!(env.fail(&dir, &args), "multipart_removal_unpublished");
        }
    }
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(),
        record
    );
    assert!(!stored(&dir, &id, "evidence.txt.part-0001").exists());
    commit_switch(&dir, true);
    env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
    let out = multipart_attach(&env, &dir, &id, "evidence.txt", b"small");
    assert_eq!(out["publication"]["state"], "published", "{out}");
    let tree = git_text(&dir, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(!tree.contains("evidence.txt.part-0002"));
    assert!(!tree.contains("evidence.txt.part-0003"));
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
    let clone = tempfile::tempdir().unwrap();
    common::git(clone.path(), &["clone", "-q", dir.to_str().unwrap(), "."]);
    common::git(
        clone.path(),
        &[
            "checkout",
            "-q",
            out["publication"]["commit"].as_str().unwrap(),
        ],
    );
    assert!(!stored(clone.path(), &id, "evidence.txt.part-0002").exists());
    assert!(!stored(clone.path(), &id, "evidence.txt.part-0003").exists());
    let clone_env = TestEnv::new();
    assert_eq!(
        clone_env.check(clone.path())["errors"],
        serde_json::json!([])
    );
    assert_eq!(
        clone_env.check(clone.path())["warnings"],
        serde_json::json!([])
    );
}

#[test]
fn reuse_blocked_under_commit_unavailable_names_the_git_route() {
    let (env, dir, id) = attach_fixture("max_bytes = 1024\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "evidence.txt", b"hello");
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(out["publication"]["cause"], "commit_unavailable", "{out}");
    assert_eq!(out["publication"]["reuse"], "blocked");
    assert!(
        out["publication"]["next"]
            .as_str()
            .unwrap()
            .contains("Git work tree")
    );
    let out = env
        .cmd(&dir)
        .args(["attach", &id, "source", "--name", "evidence.txt"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "multipart_removal_unpublished");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("Git work tree")
    );
}

#[test]
fn check_warns_when_removal_is_not_yet_in_head() {
    let (env, dir, id) = detach_fixture();
    commit_switch(&dir, false);
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    let check = env.check(&dir);
    assert!(
        check["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "multipart_removal_unpublished"),
        "{check}"
    );
    commit_switch(&dir, true);
    hook(
        &dir,
        "pre-commit",
        &format!(
            "'{}' check >check-result",
            assert_cmd::cargo::cargo_bin("tasks").display()
        ),
    );
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert!(
        std::fs::read_to_string(dir.join("check-result"))
            .unwrap()
            .contains("multipart_removal_unpublished")
    );
    assert_eq!(env.check(&dir)["warnings"], serde_json::json!([]));
}

#[test]
fn reuse_ordinary_member_survives_later_logical_detach_recovery() {
    let (env, dir, id) = detach_fixture();
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    let member = "evidence.txt.part-0002";
    let source = dir.join("ordinary");
    std::fs::write(&source, "new ordinary bytes").unwrap();
    env.json(
        &dir,
        &["attach", &id, source.to_str().unwrap(), "--name", member],
    );
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
    assert_eq!(env.check(&dir)["warnings"], serde_json::json!([]));
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let record = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    std::fs::write(stored(&dir, &id, member), "uncommitted ordinary edit").unwrap();
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "retry old removal"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(
        std::fs::read(stored(&dir, &id, member)).unwrap(),
        b"uncommitted ordinary edit"
    );
    assert_eq!(git_text(&dir, &["rev-parse", "HEAD"]), commit);
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(),
        record
    );
    env.json(&dir, &["detach", &id, member, "ordinary detach"]);
    std::fs::create_dir_all(stored(&dir, &id, member).parent().unwrap()).unwrap();
    std::fs::write(
        stored(&dir, &id, member),
        "later interrupted ordinary cleanup",
    )
    .unwrap();
    env.json(&dir, &["detach", &id, "evidence.txt", "retry old removal"]);
    assert_eq!(
        std::fs::read(stored(&dir, &id, member)).unwrap(),
        b"later interrupted ordinary cleanup"
    );
}

#[test]
fn reuse_ordinary_logical_name_can_detach_as_ordinary() {
    let (env, dir, id) = detach_fixture();
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    let source = dir.join("ordinary");
    std::fs::write(&source, "ordinary").unwrap();
    env.json(
        &dir,
        &[
            "attach",
            &id,
            source.to_str().unwrap(),
            "--name",
            "evidence.txt",
        ],
    );
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "ordinary detach"]);
    assert!(out.get("publication").is_none(), "{out}");
    assert_eq!(out["removed"], true);
    assert!(!stored(&dir, &id, "evidence.txt").exists());
    assert_eq!(env.check(&dir)["warnings"], serde_json::json!([]));
    assert_eq!(
        env.fail(&dir, &["detach", &id, "evidence.txt", "ordinary retry"]),
        "attachment_missing"
    );
    let target = stored(&dir, &id, "evidence.txt");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "interrupted ordinary removal").unwrap();
    let retry = env.json(&dir, &["detach", &id, "evidence.txt", "ordinary retry"]);
    assert!(retry.get("publication").is_none(), "{retry}");
    assert!(!target.exists());
}

#[test]
fn detach_published_commit_still_blocks_reuse_when_a_hook_restores_local_members() {
    let (env, dir, id) = detach_fixture();
    hook(
        &dir,
        "post-commit",
        &format!(
            "mkdir -p tasks/files/{id}\nprintf restored > tasks/files/{id}/evidence.txt.part-0001"
        ),
    );
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(out["publication"]["reuse"], "blocked", "{out}");
    assert!(
        out["publication"]["next"]
            .as_str()
            .unwrap()
            .contains("tasks detach")
    );
    assert_removal_clone(&dir, &id, out["publication"]["commit"].as_str().unwrap());
    let check = env.cmd(&dir).arg("check").output().unwrap();
    assert_eq!(check.status.code(), Some(1));
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert!(
        check["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|problem| problem["kind"] == "multipart_leftover")
    );
    let source = dir.join("small.txt");
    std::fs::write(&source, "small").unwrap();
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--name",
                "evidence.txt"
            ]
        ),
        "multipart_removal_unpublished"
    );
    std::fs::remove_file(dir.join(".git/hooks/post-commit")).unwrap();
    let retry = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
    assert_eq!(retry["publication"]["state"], "published", "{retry}");
    assert!(retry["publication"].get("reuse").is_none());
    assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
    assert_eq!(env.check(&dir)["warnings"], serde_json::json!([]));
    env.json(
        &dir,
        &[
            "attach",
            &id,
            source.to_str().unwrap(),
            "--name",
            "evidence.txt",
        ],
    );
}

#[test]
fn detach_same_declaration_again_needs_its_new_closing_history_published() {
    let (env, dir, id) = detach_fixture();
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    commit_switch(&dir, false);
    multipart_attach(&env, &dir, &id, "evidence.txt", &vec![b'x'; 140000]);
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(out["publication"]["state"], "local", "{out}");
    assert_eq!(out["publication"]["reuse"], "blocked");
    commit_switch(&dir, true);
    let retry = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
    assert_eq!(retry["publication"]["state"], "published", "{retry}");
    assert_removal_clone(&dir, &id, retry["publication"]["commit"].as_str().unwrap());
}

#[test]
fn detach_local_commit_failures_keep_names_reserved_until_recovery() {
    for cause in ["commit_deferred", "commit_failed"] {
        let (env, dir, id) = detach_fixture();
        if cause == "commit_deferred" {
            std::fs::write(dir.join("unrelated"), "staged").unwrap();
            common::git(&dir, &["add", "unrelated"]);
        } else {
            hook(&dir, "pre-commit", "exit 1");
        }
        let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
        assert_eq!(out["publication"]["cause"], cause, "{out}");
        assert_eq!(out["publication"]["reuse"], "blocked");
        if cause == "commit_deferred" {
            common::git(&dir, &["reset", "--", "unrelated"]);
        } else {
            std::fs::remove_file(dir.join(".git/hooks/pre-commit")).unwrap();
        }
        let retry = env.json(&dir, &["detach", &id, "evidence.txt", "retry"]);
        assert_eq!(retry["publication"]["state"], "published", "{retry}");
        assert_removal_clone(&dir, &id, retry["publication"]["commit"].as_str().unwrap());
    }
}

#[test]
fn reuse_of_members_from_an_over_ceiling_closed_declaration_refuses_before_writing() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    env.json(
        &dir,
        &[
            "note",
            &id,
            &format!(
                "multipart: evidence.txt (1 bytes, {} parts, sha256 {}, catalog sha256 {})",
                usize::MAX,
                "a".repeat(64),
                "b".repeat(64)
            ),
        ],
    );
    env.json(&dir, &["note", &id, "detached: evidence.txt: obsolete"]);
    let record = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    let source = dir.join("source");
    std::fs::write(&source, "ordinary").unwrap();
    for name in [
        "evidence.txt",
        "evidence.txt.part-0001",
        "evidence.txt.catalog.json",
    ] {
        assert_eq!(
            env.fail(
                &dir,
                &["attach", &id, source.to_str().unwrap(), "--name", name]
            ),
            "multipart_removal_unpublished"
        );
    }
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(),
        record
    );
}

#[test]
fn detach_publication_keeps_its_own_commit_when_head_moves() {
    let (env, dir, id) = detach_fixture();
    hook(
        &dir,
        "post-commit",
        "git -c core.hooksPath=/dev/null -c maintenance.auto=false -c gc.auto=0 commit -qm later --allow-empty",
    );
    let out = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(out["publication"]["state"], "published", "{out}");
    assert_eq!(
        out["publication"]["commit"],
        git_text(&dir, &["rev-parse", "HEAD^"])
    );
    assert_ne!(
        out["publication"]["commit"],
        git_text(&dir, &["rev-parse", "HEAD"])
    );
    assert_removal_clone(&dir, &id, out["publication"]["commit"].as_str().unwrap());
}
