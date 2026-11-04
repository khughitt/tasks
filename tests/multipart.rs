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

#[test]
fn scratch_identity_checkpoint_publishes_and_recovers() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let scratch = env.home.path().join("scratch");
    let runtime = env.home.path().join("tmp");
    std::fs::create_dir(&scratch).unwrap();
    std::fs::create_dir(&runtime).unwrap();
    let entries: Vec<_> = (0..1024)
        .map(|i| {
            let path = format!("evidence-{i:04}.txt");
            std::fs::write(scratch.join(&path), b"scratch evidence\n").unwrap();
            serde_json::json!({"path": path, "type": "file", "sha256": digest(b"scratch evidence\n")})
        })
        .collect();
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "version": 1, "entries": entries
    }))
    .unwrap();
    assert!(manifest.len() > 65536);
    let out = env
        .cmd(&dir)
        .env("TMPDIR", &runtime)
        .args(["attach", &id, "-", "--name", "manifest.json", "--multipart"])
        .write_stdin(manifest.clone())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let attached: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(attached["publication"]["state"], "published", "{attached}");
    let commit = attached["publication"]["commit"].as_str().unwrap();
    assert_eq!(commit, git_text(&dir, &["rev-parse", "HEAD"]));

    let clone = tempfile::tempdir().unwrap();
    common::git(clone.path(), &["clone", "-q", dir.to_str().unwrap(), "."]);
    common::git_fixture::assert_repo(env.home.path(), clone.path());
    common::git(clone.path(), &["checkout", "-q", commit]);
    let shown = env.json(clone.path(), &["show", &id]);
    let declaration = &shown["multipart"][0];
    assert_eq!(declaration["name"], "manifest.json");
    assert_eq!(declaration["sha256"], digest(&manifest));
    assert!(declaration["parts"].as_u64().unwrap() > 1);
    let dest = clone.path().join("recovered-manifest.json");
    let out = env
        .cmd(clone.path())
        .env("TMPDIR", &runtime)
        .args([
            "export",
            &id,
            "manifest.json",
            "--at",
            commit,
            "-o",
            dest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let exported: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(exported["commit"], commit);
    assert_eq!(exported["sha256"], declaration["sha256"]);
    assert_eq!(exported["integrity"]["passed"], true);
    assert_eq!(std::fs::read(&dest).unwrap(), manifest);

    let mut pending = vec![
        dir,
        clone.path().to_path_buf(),
        env.home.path().to_path_buf(),
    ];
    let mut whole_files = Vec::new();
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            pending.extend(
                std::fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if metadata.is_file()
            && metadata.len() == manifest.len() as u64
            && std::fs::read(&path).unwrap() == manifest
        {
            whole_files.push(path);
        }
    }
    assert_eq!(whole_files, vec![dest]);
    assert!(std::fs::read_dir(runtime).unwrap().next().is_none());
}

#[test]
fn export_rebuilds_from_the_working_tree_and_from_a_commit_in_a_clean_clone() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let bytes = "abc\né日\n".repeat(20000).into_bytes();
    multipart_attach(&env, &dir, &id, "evidence.txt", &bytes);
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let dest = dir.join("rebuilt");
    let report = env.json(
        &dir,
        &["export", &id, "evidence.txt", "-o", dest.to_str().unwrap()],
    );
    assert_eq!(report["bytes"], bytes.len());
    assert_eq!(report["sha256"], digest(&bytes));
    assert_eq!(report["integrity"]["passed"], true);
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    let clone = tempfile::tempdir().unwrap();
    common::git(clone.path(), &["clone", "-q", dir.to_str().unwrap(), "."]);
    std::fs::remove_file(stored(clone.path(), &id, "evidence.txt.part-0001")).unwrap();
    for format in ["--json", "--pretty"] {
        let out = env
            .cmd(clone.path())
            .args([
                format,
                "--color",
                "always",
                "export",
                &id,
                "evidence.txt",
                "--at",
                &commit,
                "-o",
                "-",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, bytes);
    }
}

#[test]
fn export_refuses_on_any_mismatch_and_writes_nothing() {
    for member in [
        "evidence.txt.part-0001",
        "missing",
        "evidence.txt.catalog.json",
    ] {
        let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
        multipart_attach(&env, &dir, &id, "evidence.txt", &vec![b'x'; 140000]);
        if member == "missing" {
            std::fs::remove_file(stored(&dir, &id, "evidence.txt.part-0001")).unwrap();
        } else {
            std::fs::write(stored(&dir, &id, member), "altered").unwrap();
        }
        let dest = dir.join("rebuilt");
        env.fail(
            &dir,
            &["export", &id, "evidence.txt", "-o", dest.to_str().unwrap()],
        );
        assert!(!dest.exists());
        std::fs::write(&dest, "keep").unwrap();
        env.fail(
            &dir,
            &["export", &id, "evidence.txt", "-o", dest.to_str().unwrap()],
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"keep");
        env.fail(&dir, &["export", &id, "evidence.txt", "-o", "-"]);
        assert_eq!(
            std::fs::read_dir(&dir)
                .unwrap()
                .filter(|e| e
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("export-"))
                .count(),
            0
        );
    }
}

#[test]
fn export_at_a_commit_survives_lowered_and_zeroed_limits() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    let bytes = vec![b'x'; 140000];
    multipart_attach(&env, &dir, &id, "evidence.txt", &bytes);
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let config = dir.join("tasks/.config.toml");
    for limit in [1, 0] {
        let text = std::fs::read_to_string(&config).unwrap();
        let mut parsed: toml::Value = toml::from_str(&text).unwrap();
        parsed["attachments"]
            .as_table_mut()
            .unwrap()
            .insert("multipart_max_bytes".into(), limit.into());
        parsed["attachments"]
            .as_table_mut()
            .unwrap()
            .insert("multipart_max_parts".into(), 1.into());
        std::fs::write(&config, toml::to_string(&parsed).unwrap()).unwrap();
        common::git(&dir, &["add", "tasks/.config.toml"]);
        common::git(&dir, &["commit", "-qm", "lower limits"]);
        assert_eq!(env.check(&dir)["errors"], serde_json::json!([]));
        for args in [
            vec!["export", &id, "evidence.txt", "-o", "-"],
            vec!["export", &id, "evidence.txt", "--at", &commit, "-o", "-"],
        ] {
            let out = env.cmd(&dir).args(args).output().unwrap();
            assert!(out.status.success(), "{out:?}");
            assert_eq!(out.stdout, bytes);
        }
    }
}

#[test]
fn export_at_reads_a_record_with_archived_notes() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    multipart_attach(&env, &dir, &id, "evidence.txt", b"archived evidence\n");
    let preview = env.json(&dir, &["archive-notes", &id]);
    env.json(
        &dir,
        &[
            "archive-notes",
            &id,
            "--expect",
            preview["source_sha256"].as_str().unwrap(),
        ],
    );
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    std::fs::remove_file(dir.join(format!("tasks/{id}.md"))).unwrap();
    let out = env
        .cmd(&dir)
        .args(["export", &id, "evidence.txt", "--at", &commit, "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"archived evidence\n");
}

#[test]
fn export_verify_reports_current_runtime_basis() {
    use std::os::unix::fs::PermissionsExt;
    let (env, dir, id) = attach_fixture(
        "max_bytes = 65536\nmultipart_validators = [{match='*',command=['./validator']}] ",
    );
    enable_git(&dir);
    let script = b"#!/bin/sh\ncat >/dev/null\nexit 0\n";
    std::fs::write(dir.join("validator"), script).unwrap();
    std::fs::set_permissions(
        dir.join("validator"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let changed = b"#!/bin/sh\ncat >/dev/null\ntest -f schema\n";
    std::fs::write(dir.join("validator"), changed).unwrap();
    for passed in [false, true] {
        if passed {
            std::fs::write(dir.join("schema"), "available").unwrap();
        }
        let out = env.json(
            &dir,
            &["export", &id, "evidence.txt", "--at", &commit, "--verify"],
        );
        assert_eq!(out["integrity"]["passed"], true);
        assert_eq!(out["validation"]["basis"], "current_runtime");
        assert_eq!(out["validation"]["passed"], passed);
        assert_eq!(
            out["validation"]["validators"][0]["program_sha256"],
            digest(changed)
        );
        assert_eq!(out["commit"], commit);
    }
}

#[test]
fn export_recovery_ignores_validators() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap().replace(
        "multipart_validators = []",
        "multipart_validators = [{match='*',command=['/bin/false']}]",
    );
    std::fs::write(&config, text).unwrap();
    assert_eq!(
        env.json(&dir, &["export", &id, "evidence.txt", "--verify"])["validation"]["passed"],
        false
    );
    let out = env
        .cmd(&dir)
        .args(["export", &id, "evidence.txt", "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"evidence\n");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("multipart_validators"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&config, text).unwrap();
    let out = env
        .cmd(&dir)
        .args(["export", &id, "evidence.txt", "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"evidence\n");
}

#[test]
fn export_copies_an_ordinary_attachment() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    let bytes = b"\x00\xff\x80\x1b[31mbinary\r\n";
    let source = dir.join("raw.bin");
    std::fs::write(&source, bytes).unwrap();
    env.json(
        &dir,
        &["attach", &id, source.to_str().unwrap(), "--caption", "raw"],
    );
    enable_git(&dir);
    for at in [false, true] {
        let mut args = vec!["export", &id, "raw.bin", "-o", "-"];
        if at {
            args.extend(["--at", "HEAD"]);
        }
        let out = env.cmd(&dir).args(args).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, bytes);
    }
    env.usage(&dir, &["export", &id, "raw.bin"]);
    env.usage(&dir, &["export", &id, "raw.bin", "-o", "-", "--verify"]);
}

#[test]
fn export_refuses_an_unnoted_extra_part_in_the_working_tree() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    std::fs::write(stored(&dir, &id, "evidence.txt.part-0002"), "unnoted").unwrap();
    broken(&env, &dir, "evidence.txt", "evidence.txt.part-0002");
    assert_eq!(
        env.fail(&dir, &["export", &id, "evidence.txt", "-o", "-"]),
        "multipart_invalid_content"
    );
}

#[test]
fn export_refuses_committed_unnoted_extra_parts_at_the_same_basis() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    std::fs::write(stored(&dir, &id, "evidence.txt.part-0002"), "unnoted").unwrap();
    common::git(&dir, &["add", "tasks"]);
    common::git(&dir, &["commit", "-qm", "unnoted extra"]);
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let clone = tempfile::tempdir().unwrap();
    common::git(clone.path(), &["clone", "-q", dir.to_str().unwrap(), "."]);
    for root in [dir.as_path(), clone.path()] {
        for at in [false, true] {
            let mut args = vec!["export", &id, "evidence.txt"];
            if at {
                args.extend(["--at", &commit]);
            }
            let mut stdout = args.clone();
            stdout.extend(["-o", "-"]);
            assert_eq!(env.fail(root, &stdout), "multipart_invalid_content");
            let destination = root.join("recovered");
            args.extend(["-o", destination.to_str().unwrap()]);
            env.fail(root, &args);
            assert!(!destination.exists());
            std::fs::write(&destination, "preserve").unwrap();
            env.fail(root, &args);
            assert_eq!(std::fs::read(&destination).unwrap(), b"preserve");
            std::fs::remove_file(destination).unwrap();
        }
    }
}

#[test]
fn export_pretty_verification_reports_the_current_runtime_verdict_and_commit() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    enable_git(&dir);
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    let commit = git_text(&dir, &["rev-parse", "HEAD"]);
    let config = dir.join("tasks/.config.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for (command, verdict, detail) in [
        ("['/bin/true']", "passed", None),
        (
            "['/bin/sh','-c','echo schema-invalid >&2; exit 7']",
            "failed",
            Some("schema-invalid"),
        ),
        (
            "['tasks-test-missing-validator-486d']",
            "unavailable",
            Some("tasks-test-missing-validator-486d"),
        ),
    ] {
        std::fs::write(
            &config,
            original.replace(
                "multipart_validators = []",
                &format!("multipart_validators = [{{match='*',command={command}}}]"),
            ),
        )
        .unwrap();
        let text = env.pretty(
            &dir,
            &["export", &id, "evidence.txt", "--at", &commit, "--verify"],
        );
        assert!(text.contains("integrity: passed"), "{text}");
        assert!(
            text.contains(&format!("validation: {verdict} (current_runtime)")),
            "{text}"
        );
        assert!(text.contains(&format!("commit: {commit}")), "{text}");
        if let Some(detail) = detail {
            assert!(text.contains(detail), "{text}");
        }
        if verdict == "failed" {
            assert!(text.contains("/bin/sh"), "{text}");
            assert!(text.contains(": rejected, exit 7"), "{text}");
            assert!(text.lines().any(|line| line == "schema-invalid"), "{text}");
        }
    }
}

#[test]
fn export_pretty_file_preflights_width_before_touching_any_destination() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    multipart_attach(&env, &dir, &id, "evidence.txt", b"evidence\n");
    for existing in [false, true] {
        let destination = dir.join("output");
        if existing {
            std::fs::write(&destination, "sentinel").unwrap();
        }
        let out = env
            .cmd(&dir)
            .env("COLUMNS", "wide")
            .args([
                "--pretty",
                "export",
                &id,
                "evidence.txt",
                "-o",
                destination.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config");
        if existing {
            assert_eq!(std::fs::read(&destination).unwrap(), b"sentinel");
        } else {
            assert!(
                !destination.exists(),
                "output was created despite failed preflight"
            );
        }
    }
}

#[test]
fn export_pretty_raw_stdout_ignores_width_and_preserves_binary_bytes() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    let id = task(&env, &dir);
    let bytes = b"\x00\xff\x80raw\r\n";
    let source = dir.join("raw.bin");
    std::fs::write(&source, bytes).unwrap();
    env.json(
        &dir,
        &["attach", &id, source.to_str().unwrap(), "--caption", "raw"],
    );
    let out = env
        .cmd(&dir)
        .env("COLUMNS", "wide")
        .args([
            "--pretty", "--color", "always", "export", &id, "raw.bin", "-o", "-",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, bytes);
}

fn reference_fixture(relation: &str) -> (TestEnv, std::path::PathBuf, String, Vec<u8>) {
    let (env, dir, id) = attach_fixture("max_bytes = 1024\nmultipart_validators = []");
    let bytes = vec![b'x'; 1500];
    multipart_attach(&env, &dir, &id, "evidence.txt", &bytes);
    let record = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&record).unwrap();
    let section = format!("## References\n\n- whole ({relation}): files:evidence.txt\n\n");
    std::fs::write(
        &record,
        text.replacen("## Notes", &(section + "## Notes"), 1),
    )
    .unwrap();
    (env, dir, id, bytes)
}

#[test]
fn show_lists_members_and_the_logical_artifact() {
    let (env, dir, id, bytes) = reference_fixture("input");
    let source = dir.join("ordinary.txt");
    std::fs::write(&source, "ordinary").unwrap();
    let attach = env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    assert!(attach["files"][0].get("member_of").is_none());
    let shown = env.json(&dir, &["show", &id]);
    let logical = &shown["multipart"][0];
    assert_eq!(logical["name"], "evidence.txt");
    assert_eq!(logical["bytes"], bytes.len());
    assert_eq!(logical["parts"], 2);
    assert_eq!(logical["sha256"], digest(&bytes));
    assert_eq!(logical["catalog"], "evidence.txt.catalog.json");
    assert_eq!(logical["caption"], "whole document");
    let files = shown["files"].as_array().unwrap();
    assert_eq!(files.len(), 4);
    for file in files {
        if file["name"] == "ordinary.txt" {
            assert!(file.get("member_of").is_none());
        } else {
            assert_eq!(file["member_of"], "evidence.txt");
        }
    }
    let next = env.json(&dir, &["next"]);
    assert_eq!(next["next"]["multipart"], shown["multipart"]);
    assert_eq!(next["next"]["files"], shown["files"]);
}

#[test]
fn inventory_row_points_at_the_catalog_and_reports_a_broken_set_missing() {
    let (env, dir, id, _) = reference_fixture("output");
    let shown = env.json(&dir, &["show", &id]);
    let row = shown["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["handle"] == "whole")
        .unwrap();
    assert_eq!(row["location"], "attachment");
    assert_eq!(row["status"], "present");
    assert_eq!(
        row["path"],
        stored(&dir, &id, "evidence.txt.catalog.json")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        row["multipart"],
        serde_json::json!({"name": "evidence.txt", "parts": 2})
    );
    std::fs::write(stored(&dir, &id, "evidence.txt.part-0002"), "wrong").unwrap();
    let shown = env.json(&dir, &["show", &id]);
    let row = shown["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["handle"] == "whole")
        .unwrap();
    assert_eq!(row["status"], "missing");
    assert_eq!(row["multipart"]["name"], "evidence.txt");
}

#[test]
fn doc_returns_the_rebuilt_whole() {
    let (env, dir, id, bytes) = reference_fixture("input");
    let out = env.cmd(&dir).args(["doc", &id, "whole"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, bytes);
    std::fs::write(stored(&dir, &id, "evidence.txt.part-0001"), "bad").unwrap();
    let out = env.cmd(&dir).args(["doc", &id, "whole"]).output().unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn refs_refuse_a_member_name() {
    let (env, dir, id, _) = reference_fixture("input");
    for name in [
        "evidence.txt.part-0001",
        "evidence.txt.catalog.json",
        "evidence.txt.part-9999",
    ] {
        for relation in ["input", "output", "context"] {
            let reference = format!("member ({relation}): files:{name}");
            assert_eq!(
                env.fail(&dir, &["edit", &id, "--ref", &reference]),
                "multipart_member"
            );
        }
    }
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--rm-ref",
            "whole",
            "--ref",
            "context (context): files:evidence.txt",
        ],
    );
}

#[test]
fn done_accepts_a_declared_artifact_as_an_output() {
    for edit in [false, true] {
        let (env, dir, id, _) = reference_fixture("output");
        env.json(&dir, &["start", &id]);
        if edit {
            env.json(&dir, &["edit", &id, "--status", "done"]);
        } else {
            env.json(&dir, &["done", &id, "evidence saved"]);
        }
    }
    let (env, dir, id, _) = reference_fixture("output");
    env.json(&dir, &["start", &id]);
    std::fs::remove_file(stored(&dir, &id, "evidence.txt.part-0001")).unwrap();
    assert_eq!(
        env.fail(&dir, &["done", &id, "broken"]),
        "ref_output_missing"
    );
}

#[test]
fn detach_refuses_while_a_reference_cites_it() {
    let (env, dir, id, _) = reference_fixture("input");
    let before = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    assert_eq!(
        env.fail(&dir, &["detach", &id, "evidence.txt", "obsolete"]),
        "ref_cites_attachment"
    );
    assert!(stored(&dir, &id, "evidence.txt.part-0001").exists());
    assert_eq!(
        std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(),
        before
    );
    env.json(&dir, &["edit", &id, "--rm-ref", "whole"]);
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
}

#[test]
fn doc_survives_lowered_and_zeroed_limits() {
    let (env, dir, id, bytes) = reference_fixture("input");
    let config = dir.join("tasks/.config.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for limit in [1, 0] {
        let mut parsed: toml::Value = toml::from_str(&original).unwrap();
        let attachments = parsed["attachments"].as_table_mut().unwrap();
        attachments.insert("multipart_max_bytes".into(), limit.into());
        attachments.insert("multipart_max_parts".into(), 1.into());
        attachments.remove("multipart_validators");
        std::fs::write(&config, toml::to_string(&parsed).unwrap()).unwrap();
        let out = env.cmd(&dir).args(["doc", &id, "whole"]).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, bytes);
        let shown = env.json(&dir, &["show", &id]);
        assert_eq!(shown["inventory"][0]["status"], "present");
    }
}

#[test]
fn eligibility_rows_for_members_are_unchanged() {
    let (env, dir, id, _) = reference_fixture("input");
    let path = format!("tasks/files/{id}/evidence.txt.part-0001");
    let rows = env.json(&dir, &["attachments", &path]);
    assert_eq!(rows["version"], 1);
    assert_eq!(
        rows["attachments"][0]["file_sha256"],
        digest(&std::fs::read(dir.join(&path)).unwrap())
    );
    assert!(rows["attachments"][0].get("multipart").is_none());
}

#[test]
fn refs_allow_future_ordinary_output_after_complete_multipart_removal() {
    let (env, dir, id) = detach_fixture();
    env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    for name in [
        "evidence.txt.part-0002",
        "evidence.txt.part-9999",
        "evidence.txt.catalog.json",
        "evidence.txt",
    ] {
        env.json(
            &dir,
            &[
                "edit",
                &id,
                "--ref",
                &format!("future (output): files:{name}"),
            ],
        );
        let source = dir.join("ordinary");
        std::fs::write(&source, "new ordinary bytes").unwrap();
        env.json(
            &dir,
            &["attach", &id, source.to_str().unwrap(), "--name", name],
        );
        let shown = env.json(&dir, &["show", &id]);
        let row = shown["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .unwrap();
        assert!(row.get("member_of").is_none(), "{shown}");
        assert!(shown.get("multipart").is_none(), "{shown}");
        let row = shown["inventory"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["handle"] == "future")
            .unwrap();
        assert!(row.get("multipart").is_none(), "{row}");
        assert_eq!(row["status"], "present");
        let out = env.cmd(&dir).args(["doc", &id, "future"]).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, b"new ordinary bytes");
        env.json(&dir, &["edit", &id, "--rm-ref", "future"]);
    }
}

#[test]
fn refs_editor_and_unchanged_entries_refuse_members() {
    let (env, dir, id, _) = reference_fixture("input");
    let record = dir.join(format!("tasks/{id}.md"));
    let original = std::fs::read_to_string(&record).unwrap();
    let editor = common::editor_script(
        env.home.path(),
        "sed -i 's/files:evidence.txt$/files:evidence.txt.part-0001/' \"$1\"",
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &editor)
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "multipart_member");
    assert_eq!(std::fs::read_to_string(&record).unwrap(), original);
    std::fs::write(
        &record,
        original.replace("files:evidence.txt\n", "files:evidence.txt.part-0001\n"),
    )
    .unwrap();
    assert_eq!(
        env.fail(&dir, &["edit", &id, "-p", "1"]),
        "multipart_member"
    );
}

#[test]
fn doc_and_reference_presence_ignore_an_unrelated_broken_artifact() {
    let (env, dir, id, bytes) = reference_fixture("input");
    multipart_attach(&env, &dir, &id, "other.txt", b"other whole");
    std::fs::write(stored(&dir, &id, "other.txt.part-0001"), "bad").unwrap();
    let shown = env.json(&dir, &["show", &id]);
    let row = shown["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["handle"] == "whole")
        .unwrap();
    assert_eq!(row["status"], "present", "{shown}");
    let out = env.cmd(&dir).args(["doc", &id, "whole"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, bytes);
}

#[test]
fn refs_closed_names_stay_reserved_until_published_removal() {
    let (env, dir, id, _) = reference_fixture("input");
    env.json(&dir, &["edit", &id, "--rm-ref", "whole"]);
    let detached = env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
    assert_eq!(detached["publication"]["state"], "local");
    for name in ["evidence.txt.part-0001", "evidence.txt.catalog.json"] {
        assert_eq!(
            env.fail(
                &dir,
                &[
                    "edit",
                    &id,
                    "--ref",
                    &format!("future (output): files:{name}")
                ]
            ),
            "multipart_member"
        );
    }
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--ref",
            "future (output): files:evidence.txt.part-9999",
        ],
    );
}

#[test]
fn refs_over_ceiling_declarations_protect_member_names_without_expansion() {
    let (env, dir, id) = attach_fixture("max_bytes = 1024\nmultipart_validators = []");
    let hash = "0".repeat(64);
    env.json(&dir, &["note", &id, &format!("multipart: oversized.txt (1 bytes, 257 parts, sha256 {hash}, catalog sha256 {hash})")]);
    env.json(&dir, &["note", &id, "detached: oversized.txt: closed"]);
    assert_eq!(
        env.fail(
            &dir,
            &[
                "edit",
                &id,
                "--ref",
                "future (output): files:oversized.txt.part-9999"
            ]
        ),
        "multipart_member"
    );
}

#[test]
fn done_refuses_a_manually_recorded_member_output() {
    let (env, dir, id, _) = reference_fixture("output");
    env.json(&dir, &["start", &id]);
    let record = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&record).unwrap();
    std::fs::write(
        &record,
        text.replace("files:evidence.txt\n", "files:evidence.txt.part-0001\n"),
    )
    .unwrap();
    assert_eq!(
        env.fail(&dir, &["done", &id, "fragment is not the output"]),
        "multipart_member"
    );
}

#[test]
fn review_existing_output_member_blocks_admission_before_writes() {
    let (env, dir, id) = attach_fixture("max_bytes = 65536\nmultipart_validators = []");
    env.json(
        &dir,
        &["edit", &id, "--ref", "part (output): files:a.txt.part-0001"],
    );
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    let source = dir.join("source");
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
        "multipart_member"
    );
    assert_eq!(std::fs::read(record).unwrap(), before);
    assert!(
        !stored(&dir, &id, "a.txt.part-0001")
            .parent()
            .unwrap()
            .exists()
    );
}

#[test]
fn review_doc_refuses_a_stored_member_reference() {
    let (env, dir, id, _) = reference_fixture("output");
    let record = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&record).unwrap();
    std::fs::write(
        record,
        text.replace("files:evidence.txt\n", "files:evidence.txt.part-0001\n"),
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["doc", &id, "whole"]), "multipart_member");
}

#[test]
fn review_check_reports_a_stored_member_reference() {
    let (env, dir, id, _) = reference_fixture("output");
    let record = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&record).unwrap();
    std::fs::write(
        record,
        text.replace("files:evidence.txt\n", "files:evidence.txt.part-0001\n"),
    )
    .unwrap();
    broken(&env, &dir, "evidence.txt", "evidence.txt.part-0001");
}

#[test]
fn review_logical_ordinary_collision_is_broken() {
    let (env, dir, id) = attach_fixture("multipart_validators = []");
    multipart_attach(&env, &dir, &id, "a.txt", b"whole");
    std::fs::write(stored(&dir, &id, "a.txt"), b"other").unwrap();
    env.json(&dir, &["note", &id, "attached: a.txt (5 bytes)"]);
    broken(&env, &dir, "a.txt", "a.txt");
    assert_eq!(
        env.fail(&dir, &["export", &id, "a.txt", "-o", "-"]),
        "multipart_invalid_content"
    );
}

#[test]
fn review_physical_logical_path_blocks_admission() {
    for case in ["different", "identical", "symlink", "directory"] {
        let (env, dir, id) = attach_fixture("multipart_validators = []");
        let path = stored(&dir, &id, "a.txt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = dir.join("source");
        std::fs::write(&source, b"whole").unwrap();
        match case {
            "different" => std::fs::write(&path, b"original").unwrap(),
            "identical" => std::fs::write(&path, b"whole").unwrap(),
            "symlink" => std::os::unix::fs::symlink(&source, &path).unwrap(),
            "directory" => std::fs::create_dir(&path).unwrap(),
            _ => unreachable!(),
        }
        let record = dir.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&record).unwrap();
        let kind = if matches!(case, "symlink" | "directory") {
            "attachment_unsafe"
        } else {
            "attachment_exists"
        };
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
            kind,
            "{case}"
        );
        assert_eq!(std::fs::read(record).unwrap(), before);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        match case {
            "different" => assert_eq!(std::fs::read(path).unwrap(), b"original"),
            "identical" => assert_eq!(std::fs::read(path).unwrap(), b"whole"),
            "symlink" => assert_eq!(std::fs::read_link(path).unwrap(), source),
            "directory" => assert!(path.is_dir()),
            _ => unreachable!(),
        }
    }
}

#[test]
fn review_export_ordinary_reuse_at_each_basis() {
    let (env, dir, id) = detach_fixture();
    let closed =
        env.json(&dir, &["detach", &id, "evidence.txt", "obsolete"])["publication"]["commit"]
            .as_str()
            .unwrap()
            .to_string();
    let source = dir.join("new");
    std::fs::write(&source, b"new").unwrap();
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
    for at in [vec![], vec!["--at", "HEAD"]] {
        let out = env
            .cmd(&dir)
            .args(["export", &id, "evidence.txt", "-o", "-"])
            .args(at)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, b"new");
    }
    assert_eq!(
        env.fail(
            &dir,
            &["export", &id, "evidence.txt", "--at", &closed, "-o", "-"]
        ),
        "attachment_missing"
    );
}

#[test]
fn review_same_blob_symlink_is_never_published_or_recovered() {
    let (env, dir, id) = attach_fixture("multipart_validators = []");
    enable_git(&dir);
    let member = format!("tasks/files/{id}/a.txt.part-0001");
    hook(
        &dir,
        "pre-commit",
        &format!("rm {member}\nln -s payload {member}\ngit add {member}"),
    );
    let out = multipart_attach(&env, &dir, &id, "a.txt", b"payload");
    assert_eq!(out["publication"]["state"], "local", "{out}");
    assert!(git_text(&dir, &["ls-tree", "HEAD", "--", &member]).starts_with("120000"));
    broken(&env, &dir, "a.txt", "a.txt.part-0001");
    for at in [vec![], vec!["--at", "HEAD"]] {
        let out = env
            .cmd(&dir)
            .args(["export", &id, "a.txt", "-o", "-"])
            .args(at)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
    }
    assert_eq!(
        env.fail(&dir, &["export", &id, "a.txt", "--at", "HEAD", "--verify"]),
        "multipart_invalid_content"
    );
}

#[test]
fn review_git_catalog_symlink_refuses_but_executable_files_work() {
    use std::os::unix::fs::PermissionsExt;
    let (env, dir, id) = attach_fixture("multipart_validators = []");
    enable_git(&dir);
    multipart_attach(&env, &dir, &id, "a.txt", b"payload");
    let member = stored(&dir, &id, "a.txt.part-0001");
    std::fs::set_permissions(member, std::fs::Permissions::from_mode(0o755)).unwrap();
    common::git(&dir, &["add", "tasks"]);
    common::git(&dir, &["commit", "-qm", "executable regular member"]);
    let out = env
        .cmd(&dir)
        .args(["export", &id, "a.txt", "--at", "HEAD", "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"payload");
    let catalog = stored(&dir, &id, "a.txt.catalog.json");
    let bytes = std::fs::read_to_string(&catalog).unwrap();
    std::fs::remove_file(&catalog).unwrap();
    std::os::unix::fs::symlink(bytes, &catalog).unwrap();
    common::git(&dir, &["add", "tasks"]);
    common::git(&dir, &["commit", "-qm", "unsafe catalog"]);
    let out = env
        .cmd(&dir)
        .args(["export", &id, "a.txt", "--at", "HEAD", "-o", "-"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn review_pretty_detach_reports_publication_and_reuse() {
    for case in ["disabled", "deferred", "published", "leftover"] {
        let (env, dir, id) = detach_fixture();
        match case {
            "disabled" => commit_switch(&dir, false),
            "deferred" => {
                std::fs::write(dir.join("unrelated"), "staged").unwrap();
                common::git(&dir, &["add", "unrelated"]);
            }
            "leftover" => hook(
                &dir,
                "post-commit",
                &format!(
                    "mkdir -p tasks/files/{id}\nprintf restored > tasks/files/{id}/evidence.txt.part-0001"
                ),
            ),
            _ => {}
        }
        let text = env.pretty(&dir, &["detach", &id, "evidence.txt", "obsolete"]);
        if matches!(case, "disabled" | "deferred") {
            assert!(text.starts_with("local"), "{case}: {text}");
            assert!(text.contains(&format!("commit_{case}")), "{text}");
        } else {
            assert!(text.starts_with("published"), "{case}: {text}");
            assert!(
                text.contains(&git_text(&dir, &["rev-parse", "HEAD"])),
                "{text}"
            );
        }
        if case != "published" {
            assert!(text.contains("reuse: blocked"), "{case}: {text}");
            assert!(text.contains("tasks detach"), "{text}");
        }
    }
}
