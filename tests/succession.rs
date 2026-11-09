mod common;
use common::{TestEnv, git_fixture, id_of};
use std::path::{Path, PathBuf};

fn git(env: &TestEnv, dir: &Path, args: &[&str]) -> String {
    let out = git_fixture::command(env.home.path(), dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    if args.first() == Some(&"init") {
        git_fixture::assert_repo(env.home.path(), dir);
    }
    String::from_utf8(out.stdout).unwrap()
}

fn repo(env: &TestEnv, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(env, dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("README"), "fixture\n").unwrap();
    git(env, dir, &["add", "README"]);
    git(env, dir, &["commit", "-qm", "seed"]);
}

fn config(path: &Path) -> toml::Value {
    toml::from_str(&std::fs::read_to_string(path.join("tasks/.config.toml")).unwrap()).unwrap()
}

fn registry(env: &TestEnv) -> toml::Value {
    toml::from_str(&env.read(env.home.path(), ".config/tasks/projects.toml")).unwrap()
}

#[test]
fn repointing_retired_project_succeeds_when_retirement_is_preserved() {
    for detached in [false, true] {
        let mut env = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let code = base.path().join("code_v1");
        let records = base.path().join("records");
        repo(&env, &code);
        let project = if detached {
            repo(&env, &records);
            env.json(
                &code,
                &[
                    "init",
                    "--prefix",
                    "forge",
                    "--records",
                    records.to_str().unwrap(),
                ],
            );
            records.join("forge")
        } else {
            env.json(&code, &["init", "--prefix", "forge"]);
            code.clone()
        };
        let successor = env.init("forge2");
        assert!(successor.exists());
        let id = id_of(env.json(&code, &["add", "finished"]));
        env.json(&code, &["done", &id, "done"]);
        env.json(
            &code,
            &["retire", "--reason", "superseded", "--successor", "forge2"],
        );
        let retired = config(&project)["retired"].clone();
        let archived = base.path().join("archive/familiar/forge/v1");
        std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
        std::fs::rename(&code, &archived).unwrap();
        let mut args = vec!["init", "--prefix", "forge", "--force"];
        if detached {
            args.extend(["--records", records.to_str().unwrap()]);
        }
        env.json(&archived, &args);
        assert_eq!(
            registry(&env)["projects"]["forge"].as_str(),
            archived.to_str()
        );
        let project = if detached { project } else { archived };
        assert_eq!(config(&project)["retired"], retired);
        if !detached {
            assert!(
                registry(&env)
                    .get("records")
                    .and_then(|r| r.get("forge"))
                    .is_none()
            );
        }
    }
}

#[test]
fn repointing_retired_project_refuses_when_target_config_alters_or_drops_retirement() {
    for mutation in ["drop", "date", "invent"] {
        let mut env = TestEnv::new();
        let old = env.init("old");
        let target = tempfile::tempdir().unwrap();
        if mutation != "invent" {
            env.json(&old, &["retire", "--reason", "done"]);
        }
        let mut cfg = config(&old);
        match mutation {
            "drop" => {
                cfg.as_table_mut().unwrap().remove("retired");
            }
            "date" => {
                cfg["retired"]["date"] = "2000-01-01".into();
            }
            _ => {
                cfg.as_table_mut().unwrap().insert(
                    "retired".into(),
                    toml::toml! { date = "2000-01-01" reason = "invented" }.into(),
                );
            }
        }
        std::fs::create_dir(target.path().join("tasks")).unwrap();
        std::fs::write(
            target.path().join("tasks/.config.toml"),
            toml::to_string(&cfg).unwrap(),
        )
        .unwrap();
        assert_eq!(
            env.fail(target.path(), &["init", "--prefix", "old", "--force"]),
            "retired_project"
        );
        assert_eq!(registry(&env)["projects"]["old"].as_str(), old.to_str());
    }
}

#[test]
fn retired_project_commits_config_and_docs_but_refuses_task_records() {
    let mut env = TestEnv::new();
    let root = env.init("old");
    repo(&env, &root);
    let id = id_of(env.json(&root, &["add", "finished"]));
    env.json(&root, &["done", &id, "done"]);
    git(&env, &root, &["add", "tasks"]);
    git(&env, &root, &["commit", "-qm", "records"]);
    env.json(&root, &["retire", "--reason", "done"]);
    assert_eq!(
        env.json(
            &root,
            &["commit", "tasks/.config.toml", "-m", "chore(tasks): retire"]
        )["committed"],
        1
    );
    common::write_doc(&root, "docs/notes/retro.md", "retro\n");
    assert_eq!(
        env.json(
            &root,
            &["commit", "docs/notes/retro.md", "-m", "docs: add retro"]
        )["committed"],
        1
    );
    let task = root.join(format!("tasks/{id}.md"));
    let original = std::fs::read_to_string(&task).unwrap();
    std::fs::write(&task, format!("{original}\nchanged\n")).unwrap();
    for target in [
        id.clone(),
        format!("tasks/{id}.md"),
        task.display().to_string(),
        "tasks".into(),
        "tasks/".into(),
        "tasks/files".into(),
    ] {
        assert_eq!(
            env.fail(&root, &["commit", &target, "-m", "chore: forbidden"]),
            "retired_project",
            "{target}"
        );
    }
    assert_eq!(env.fail(&root, &["commit"]), "retired_project");
}

fn renamed(
    detached: bool,
) -> (
    TestEnv,
    tempfile::TempDir,
    PathBuf,
    PathBuf,
    String,
    PathBuf,
) {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let code = base.path().join("code");
    repo(&env, &code);
    let records = if detached {
        let records = base.path().join("records");
        repo(&env, &records);
        env.json(
            &code,
            &[
                "init",
                "--prefix",
                "dot",
                "--records",
                records.to_str().unwrap(),
            ],
        );
        records
    } else {
        env.json(&code, &["init", "--prefix", "dot"]);
        code.clone()
    };
    let id = id_of(env.json(&code, &["add", "one"]));
    let project = if detached {
        records.join("dot")
    } else {
        code.clone()
    };
    let file = base.path().join("evidence.txt");
    std::fs::write(&file, "evidence\n").unwrap();
    env.json(&code, &["attach", &id, file.to_str().unwrap()]);
    git(&env, &records, &["add", "-A"]);
    git(&env, &records, &["commit", "-qm", "records"]);
    assert!(project.exists());
    env.json(&code, &["rename", "dot", "dots"]);
    let project = if detached {
        records.join("dots")
    } else {
        code.clone()
    };
    let receipt = env
        .home
        .path()
        .join(".local/state/tasks/rename-commit/dots.toml");
    assert!(project.exists());
    (
        env,
        base,
        code,
        records,
        id.replacen("dot-", "dots-", 1),
        receipt,
    )
}

#[test]
fn commit_rename_receipt_commits_detached_rename_and_clears_receipt() {
    for detached in [false, true] {
        let (env, _base, code, records, _, receipt) = renamed(detached);
        assert!(receipt.exists());
        env.json(&code, &["commit", "--project", "dots"]);
        assert_eq!(git(&env, &records, &["status", "--porcelain"]), "");
        assert_eq!(
            git(&env, &records, &["log", "-1", "--format=%s"]).trim(),
            "chore(tasks): rename dot -> dots"
        );
        assert!(!receipt.exists());
    }
}

#[test]
fn commit_rename_receipt_refuses_when_file_digest_changed_after_rename() {
    for changed in ["record", "config", "source", "missing", "old-source"] {
        let (env, _base, code, records, id, receipt) = renamed(true);
        let project = records.join("dots");
        let path = match changed {
            "record" | "missing" => project.join(format!("tasks/{id}.md")),
            "old-source" => {
                records.join(format!("dot/tasks/{}.md", id.replacen("dots-", "dot-", 1)))
            }
            "config" => project.join("tasks/.config.toml"),
            _ => project.join(format!("tasks/{}.md", id.replacen("dots-", "dot-", 1))),
        };
        if changed == "missing" {
            std::fs::remove_file(&path).unwrap();
        } else {
            let bytes = if changed == "source" || changed == "old-source" {
                "source".into()
            } else {
                format!("{}\n", std::fs::read_to_string(&path).unwrap())
            };
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        let head = git(&env, &records, &["rev-parse", "HEAD"]);
        let out = env
            .cmd(&code)
            .args(["commit", "--project", "dots"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert_eq!(common::err_kind(&out), "validation");
        if changed == "record" || changed == "config" {
            assert!(
                common::error_detail(&out).contains("digest mismatch"),
                "{out:?}"
            );
        }
        assert!(receipt.exists());
        assert_eq!(git(&env, &records, &["rev-parse", "HEAD"]), head);
    }
}

#[test]
fn commit_rename_receipt_survives_commit_failure_and_excludes_other_projects() {
    use std::os::unix::fs::PermissionsExt;
    let (env, _base, code, records, _, receipt) = renamed(true);
    std::fs::write(records.join("unrelated.txt"), "other project\n").unwrap();
    let hook = records.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        env.fail(&code, &["commit", "--project", "dots"]),
        "validation"
    );
    assert!(receipt.exists());
    std::fs::remove_file(hook).unwrap();
    env.json(
        &code,
        &["commit", "--project", "dots", "-m", "chore: publish rename"],
    );
    assert!(!receipt.exists());
    assert_eq!(
        git(&env, &records, &["status", "--porcelain"]).trim(),
        "?? unrelated.txt"
    );
    assert_eq!(
        git(&env, &records, &["log", "-1", "--format=%s"]).trim(),
        "chore: publish rename"
    );
}

#[test]
fn commit_rename_receipt_includes_both_sides_of_staged_git_renames() {
    for detached in [false, true] {
        let (env, _base, code, records, _, receipt) = renamed(detached);
        git(&env, &records, &["add", "-A"]);
        env.json(&code, &["commit", "--project", "dots"]);
        assert_eq!(git(&env, &records, &["status", "--porcelain"]), "");
        assert!(!receipt.exists());
    }
}

fn predecessor(env: &TestEnv, current: &Path, old: &str, suffixes: &[&str]) {
    let mut cfg = config(current);
    cfg.as_table_mut().unwrap().insert(
        "predecessor".into(),
        toml::Value::try_from(serde_json::json!({"prefix": old, "suffixes": suffixes})).unwrap(),
    );
    std::fs::write(
        current.join("tasks/.config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    env.json(current, &["init", "--prefix", "forge", "--force"]);
}

fn write_task(root: &Path, id: &str, extra: &str) {
    std::fs::write(root.join(format!("tasks/{id}.md")), format!("---\nid: {id}\ntitle: fixture\nstatus: done\npriority: 2\ncreated: 2026-11-01T00:00:00Z\nupdated: 2026-11-01T00:00:00Z\ndepends: []\ntags: []\n{extra}---\n")).unwrap();
}

#[test]
fn succession_routes_literal_ids_and_keeps_bare_suffixes_local() {
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    write_task(&old, "forge1-0e720e", "");
    predecessor(&env, &current, "forge1", &["0e720e"]);
    let mut reg = registry(&env);
    reg["aliases"] = toml::toml! { forge2 = "forge" }.into();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        toml::to_string(&reg).unwrap(),
    )
    .unwrap();
    assert_eq!(
        env.json(&current, &["show", "forge-0e720e"])["task"]["id"],
        "forge1-0e720e"
    );
    assert_eq!(env.fail(&current, &["show", "0e720e"]), "task_not_found");
    assert_eq!(
        env.fail(&current, &["show", "forge2-0e720e"]),
        "task_not_found"
    );
    for value in ["0e720e", "forge2-0e720e"] {
        assert_eq!(
            env.fail(&current, &["add", "invalid reference", "--depends", value]),
            "unresolvable_id"
        );
        assert_eq!(
            env.fail(&current, &["add", "invalid reference", "--related", value]),
            "unresolvable_id"
        );
    }
    let rows = env.json(&current, &["resolve", "forge-0e720e", "forge2-0e720e"]);
    assert_eq!(rows["results"][0]["prefix"], "forge1", "{rows}");
    assert_eq!(rows["results"][1]["prefix"], "forge", "{rows}");
}

#[test]
fn check_reports_retired_successor_and_predecessor_errors() {
    for retired in [false, true] {
        let mut env = TestEnv::new();
        let old = env.init("forge1");
        let current = env.init("forge");
        if retired {
            env.json(&current, &["retire", "--reason", "done"]);
        }
        let successor = if retired { "forge" } else { "missing" };
        std::fs::write(old.join("tasks/.config.toml"), format!("prefix = \"forge1\"\n[retired]\ndate = \"2026-11-01\"\nreason = \"done\"\nsuccessor = \"{successor}\"\n")).unwrap();
        let out = env.cmd(&old).arg("check").output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let kind = if retired {
            "retired_successor_retired"
        } else {
            "retired_successor_invalid"
        };
        assert!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == kind),
            "{report}"
        );
    }
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    write_task(&old, "forge1-0e720e", "");
    predecessor(&env, &current, "forge1", &["0e720e", "123456"]);
    write_task(&current, "forge-0e720e", "");
    let out = env.cmd(&current).arg("check").output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for kind in [
        "predecessor_suffix_collision",
        "predecessor_suffix_mismatch",
        "predecessor_not_retired_to_successor",
    ] {
        assert!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == kind),
            "{report}"
        );
    }
    std::fs::remove_file(current.join("tasks/forge-0e720e.md")).unwrap();
    predecessor(&env, &current, "forge1", &["0e720e"]);
    write_task(&old, "forge1-0e720e", "moved_to: forge-18dae2\n");
    let path = old.join("tasks/forge1-0e720e.md");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace("status: done", "status: moved"),
    )
    .unwrap();
    std::fs::write(old.join("tasks/.config.toml"), "prefix = \"forge1\"\n[retired]\ndate = \"2026-11-01\"\nreason = \"done\"\nsuccessor = \"forge\"\n").unwrap();
    write_task(&current, "forge-18dae2", "moved_from: forge-0e720e\n");
    assert!(env.check(&current)["errors"].as_array().unwrap().is_empty());
}

#[test]
fn succession_citations_and_stored_dependencies_do_not_route_aliases_twice() {
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    write_task(&old, "forge1-0e720e", "");
    predecessor(&env, &current, "forge1", &["0e720e"]);
    let mut reg = registry(&env);
    reg["aliases"] = toml::toml! { forge2 = "forge" }.into();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        toml::to_string(&reg).unwrap(),
    )
    .unwrap();
    let id = id_of(env.json(&current, &["add", "citation", "-b", "forge2-0e720e"]));
    let shown = env.json(&current, &["show", &id]);
    assert!(shown["task"].get("related").is_none(), "{shown}");
    let id = id_of(env.json(&current, &["add", "wake target"]));
    assert_eq!(
        env.fail(
            &current,
            &[
                "shelve",
                &id,
                "wake",
                "--defer",
                "1d",
                "--wake-on",
                "0e720e"
            ]
        ),
        "unresolvable_id"
    );
    let mut raw = std::fs::read_to_string(current.join(format!("tasks/{id}.md"))).unwrap();
    raw = raw.replace("depends: []", "depends: [forge2-0e720e]");
    std::fs::write(current.join(format!("tasks/{id}.md")), raw).unwrap();
    let shown = env.json(&current, &["show", &id]);
    assert_eq!(shown["depends_on"][0]["resolved"], false, "{shown}");
}

#[test]
fn succession_pairing_mismatch_and_unreachable_predecessor_are_reported() {
    let mut env = TestEnv::new();
    let current = env.init("forge");
    let mut cfg = config(&current);
    cfg.as_table_mut().unwrap().insert(
        "predecessor".into(),
        toml::toml! { prefix = "forge1" suffixes = ["0e720e"] }.into(),
    );
    std::fs::write(
        current.join("tasks/.config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    assert_eq!(env.fail(&current, &["add", "stale pairing"]), "config");
    let out = env.cmd(&current).arg("check").output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "predecessor_registry_mismatch"),
        "{report}"
    );
    env.json(&current, &["init", "--prefix", "forge", "--force"]);
    let report = env.check(&current);
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "predecessor_unreachable"),
        "{report}"
    );
    write_task(&current, "forge-18dae2", "moved_from: forge-abcdef\n");
    assert_eq!(env.fail(&current, &["show", "forge-18dae2"]), "parse");
}

#[test]
fn succession_commit_routes_full_ids_once_and_bare_suffixes_locally() {
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    repo(&env, &old);
    repo(&env, &current);
    write_task(&old, "forge1-0e720e", "");
    write_task(&current, "forge-18dae2", "");
    predecessor(&env, &current, "forge1", &["0e720e"]);
    let mut reg = registry(&env);
    reg["aliases"] = toml::toml! { forge2 = "forge" }.into();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        toml::to_string(&reg).unwrap(),
    )
    .unwrap();
    assert_eq!(
        env.json(&current, &["commit", "forge-0e720e"])["committed"],
        1
    );
    assert!(git(&env, &old, &["show", "HEAD:tasks/forge1-0e720e.md"]).contains("fixture"));
    assert_eq!(
        env.json(&current, &["commit", "forge2-18dae2"])["committed"],
        1
    );
    let before = git(&env, &old, &["rev-parse", "HEAD"]);
    write_task(&old, "forge1-0e720e", "source: changed\n");
    for value in ["0e720e", "forge2-0e720e"] {
        assert_eq!(env.fail(&current, &["commit", value]), "task_not_found");
    }
    assert_eq!(git(&env, &old, &["rev-parse", "HEAD"]), before);
}

#[test]
fn succession_check_reports_predecessor_pairing_mismatch_from_code_checkout() {
    for detached in [false, true] {
        let env = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let code = base.path().join("code");
        let records = base.path().join("records");
        repo(&env, &code);
        repo(&env, &records);
        let project = if detached {
            env.json(
                &code,
                &[
                    "init",
                    "--prefix",
                    "forge",
                    "--records",
                    records.to_str().unwrap(),
                ],
            );
            records.join("forge")
        } else {
            env.json(
                &records,
                &[
                    "init",
                    "--prefix",
                    "forge",
                    "--checkout",
                    code.to_str().unwrap(),
                ],
            );
            records.clone()
        };
        let mut cfg = config(&project);
        cfg.as_table_mut().unwrap().insert(
            "predecessor".into(),
            toml::toml! { prefix = "forge1" suffixes = ["0e720e"] }.into(),
        );
        std::fs::write(
            project.join("tasks/.config.toml"),
            toml::to_string(&cfg).unwrap(),
        )
        .unwrap();
        assert_eq!(env.fail(&code, &["add", "stale pairing"]), "config");
        let out = env.cmd(&code).arg("check").output().unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|_| panic!("{out:?}"));
        assert!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "predecessor_registry_mismatch"),
            "{report}"
        );
    }
}

#[test]
fn succession_blockers_keep_missing_alias_dependency_unresolved() {
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    write_task(&old, "forge1-0e720e", "");
    std::fs::write(old.join("tasks/.config.toml"), "prefix = \"forge1\"\n[retired]\ndate = \"2026-11-01\"\nreason = \"done\"\nsuccessor = \"forge\"\n").unwrap();
    predecessor(&env, &current, "forge1", &["0e720e"]);
    let mut reg = registry(&env);
    reg["aliases"] = toml::toml! { forge2 = "forge" }.into();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        toml::to_string(&reg).unwrap(),
    )
    .unwrap();
    let id = id_of(env.json(&current, &["add", "blocked work"]));
    let path = current.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&path)
        .unwrap()
        .replace("depends: []", "depends: [forge2-0e720e]");
    std::fs::write(&path, &raw).unwrap();
    let report = env.json(&current, &["blockers"]);
    assert_eq!(report["coverage"]["complete"], false, "{report}");
    assert_eq!(
        report["unresolved"].as_array().unwrap().len(),
        1,
        "{report}"
    );
    assert_eq!(report["unresolved"][0]["dependent"], id, "{report}");
    assert_eq!(
        report["unresolved"][0]["reference"], "forge2-0e720e",
        "{report}"
    );
    assert_eq!(
        report["unresolved"][0]["canonical"], "forge-0e720e",
        "{report}"
    );
    std::fs::write(&path, raw.replace("forge2-0e720e", "forge-0e720e")).unwrap();
    let report = env.json(&current, &["blockers"]);
    assert!(
        report["unresolved"].as_array().unwrap().is_empty(),
        "{report}"
    );
    assert_eq!(report["coverage"]["complete"], true, "{report}");
}

#[test]
fn succession_halt_exception_requires_a_genuine_predecessor_edge() {
    let mut env = TestEnv::new();
    let old = env.init("forge1");
    let current = env.init("forge");
    write_task(&old, "forge1-0e720e", "moved_to: forge-18dae2\n");
    let path = old.join("tasks/forge1-0e720e.md");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace("status: done", "status: moved"),
    )
    .unwrap();
    std::fs::write(old.join("tasks/.config.toml"), "prefix = \"forge1\"\n[retired]\ndate = \"2026-11-01\"\nreason = \"done\"\nsuccessor = \"forge\"\n").unwrap();
    predecessor(&env, &current, "forge1", &["0e720e"]);
    let mut reg = registry(&env);
    reg["aliases"] = toml::toml! { forge2 = "forge" }.into();
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        toml::to_string(&reg).unwrap(),
    )
    .unwrap();
    write_task(&current, "forge-18dae2", "moved_from: forge-0e720e\n");
    let carried = current.join("tasks/forge-18dae2.md");
    std::fs::write(
        &carried,
        std::fs::read_to_string(&carried)
            .unwrap()
            .replace("status: done", "status: todo"),
    )
    .unwrap();
    write_task(&current, "forge-aaaaaa", "");
    let halt = current.join("tasks/forge-aaaaaa.md");
    let raw = std::fs::read_to_string(&halt)
        .unwrap()
        .replace("status: done", "status: todo")
        .replace("priority: 2", "priority: 0")
        .replace("tags: []", "tags: [halt]")
        .replace("depends: []", "depends: [forge2-0e720e]");
    std::fs::write(&halt, &raw).unwrap();
    let ready = env.json(&current, &["ready"]);
    assert!(
        !ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "forge-18dae2"),
        "{ready}"
    );
    assert_eq!(env.fail(&current, &["start", "forge-18dae2"]), "halted");
    std::fs::write(&halt, raw.replace("forge2-0e720e", "forge-0e720e")).unwrap();
    let ready = env.json(&current, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "forge-18dae2"),
        "{ready}"
    );
    assert_eq!(
        env.json(&current, &["start", "forge-18dae2"])["id"],
        "forge-18dae2"
    );
}
