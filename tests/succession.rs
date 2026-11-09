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

struct Handoff {
    env: TestEnv,
    _base: tempfile::TempDir,
    old: PathBuf,
    new: PathBuf,
    records: PathBuf,
}

impl Handoff {
    fn new() -> Self {
        let env = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("forge");
        let new = base.path().join("forge2");
        let records = base.path().join("records");
        for path in [&old, &new, &records] {
            repo(&env, path);
        }
        for (path, prefix) in [(&old, "forge"), (&new, "forge2")] {
            env.json(
                path,
                &[
                    "init",
                    "--prefix",
                    prefix,
                    "--records",
                    records.to_str().unwrap(),
                ],
            );
        }
        write_task(&records.join("forge"), "forge-0e720e", "");
        write_task(&records.join("forge2"), "forge2-18dae2", "");
        env.json(
            &old,
            &["retire", "--reason", "superseded", "--successor", "forge2"],
        );
        git(&env, &records, &["add", "-A"]);
        git(&env, &records, &["commit", "-qm", "seed records"]);
        Self {
            env,
            _base: base,
            old,
            new,
            records,
        }
    }
    fn step1(&self) {
        self.env.json(&self.old, &["rename", "forge", "forge1"]);
        self.env.json(&self.old, &["commit", "--project", "forge1"]);
    }
}

#[test]
fn retired_project_can_be_renamed_and_preserves_retirement() {
    let f = Handoff::new();
    let retired = config(&f.records.join("forge"))["retired"].clone();
    f.step1();
    assert_eq!(config(&f.records.join("forge1"))["retired"], retired);
    assert_eq!(git(&f.env, &f.records, &["status", "--porcelain"]), "");
    assert_eq!(f.env.fail(&f.old, &["add", "forbidden"]), "retired_project");
}

#[test]
fn prefix_handoff_refuses_uncommitted_step1_non_retired_wrong_successor_or_colliding_suffix() {
    for case in [
        "receipt",
        "dirty",
        "deleted-source",
        "not-retired",
        "wrong-successor",
        "collision",
        "predecessor",
        "park",
    ] {
        let f = Handoff::new();
        f.env.json(&f.old, &["rename", "forge", "forge1"]);
        if case != "receipt" {
            f.env.json(&f.old, &["commit", "--project", "forge1"]);
        }
        let pred = f.records.join("forge1");
        match case {
            "dirty" => std::fs::write(pred.join("tasks/uncommitted"), "dirty").unwrap(),
            "deleted-source" => {
                std::fs::create_dir_all(f.records.join("forge")).unwrap();
                std::fs::write(f.records.join("forge/uncommitted"), "dirty").unwrap();
            }
            "not-retired" | "wrong-successor" | "predecessor" => {
                let mut cfg = config(&pred);
                if case == "not-retired" {
                    cfg.as_table_mut().unwrap().remove("retired");
                } else if case == "wrong-successor" {
                    cfg["retired"]["successor"] = "other".into();
                } else {
                    cfg.as_table_mut().unwrap().insert(
                        "predecessor".into(),
                        toml::toml! { prefix = "older" suffixes = [] }.into(),
                    );
                }
                std::fs::write(
                    pred.join("tasks/.config.toml"),
                    toml::to_string(&cfg).unwrap(),
                )
                .unwrap();
                git(&f.env, &f.records, &["add", "-A"]);
                git(&f.env, &f.records, &["commit", "-qm", "alter predecessor"]);
            }
            "collision" => {
                write_task(&f.records.join("forge2"), "forge2-0e720e", "");
                git(&f.env, &f.records, &["add", "-A"]);
                git(&f.env, &f.records, &["commit", "-qm", "collision"]);
            }
            "park" => common::write_park(
                &f.env,
                "forge1",
                "forge1-0e720e",
                "test",
                "agent",
                f.old.to_str().unwrap(),
            ),
            _ => (),
        }
        let before = registry(&f.env);
        let out = f
            .env
            .cmd(&f.new)
            .args(["rename", "forge2", "forge"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "{case}: {out:?}");
        let detail = String::from_utf8_lossy(&out.stderr);
        assert!(
            detail.contains("forge1") || detail.contains("forge"),
            "{case}: {out:?}"
        );
        assert_eq!(registry(&f.env), before);
        assert!(f.records.join("forge2/tasks/forge2-18dae2.md").exists());
    }
}

#[test]
fn prefix_handoff_resumes_after_stopping_at_each_stage() {
    for (stage, recovery) in [
        ("inventory", "resume_files"),
        ("files", "resume_files"),
        ("config", "resume_registry"),
        ("records", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("store", "resume_cleanup"),
        ("claims", "resume_cleanup"),
    ] {
        let f = Handoff::new();
        f.step1();
        common::write_park(
            &f.env,
            "forge2",
            "forge2-18dae2",
            "test",
            "agent",
            f.new.to_str().unwrap(),
        );
        f.env
            .cmd(&f.new)
            .env("TASKS_RENAME_STOP_AFTER", stage)
            .args(["rename", "forge2", "forge"])
            .assert()
            .success();
        assert_eq!(
            f.env
                .json(&f.new, &["rename", "forge2", "forge", "--explain"])["recovery"],
            recovery,
            "{stage}"
        );
        f.env.json(&f.new, &["rename", "forge2", "forge"]);
        assert_eq!(
            config(&f.records.join("forge1"))["retired"]["successor"].as_str(),
            Some("forge")
        );
        assert_eq!(
            config(&f.records.join("forge"))["predecessor"]["prefix"].as_str(),
            Some("forge1")
        );
        f.env.json(&f.new, &["commit", "--project", "forge"]);
        assert_eq!(
            git(&f.env, &f.records, &["status", "--porcelain"]),
            "",
            "{stage}"
        );
        assert_eq!(
            f.env.json(&f.new, &["show", "forge-0e720e"])["task"]["id"],
            "forge1-0e720e"
        );
        assert_eq!(
            f.env.json(&f.new, &["show", "forge2-18dae2"])["task"]["id"],
            "forge-18dae2"
        );
        assert_eq!(
            f.env.fail(&f.new, &["show", "forge2-0e720e"]),
            "task_not_found"
        );
        assert_eq!(f.env.fail(&f.new, &["show", "0e720e"]), "task_not_found");
    }
}

#[test]
fn prefix_handoff_commit_verifies_predecessor_config_and_retains_failed_receipt() {
    let f = Handoff::new();
    f.step1();
    f.env.json(&f.new, &["rename", "forge2", "forge"]);
    let path = f.records.join("forge1/tasks/.config.toml");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, [original.as_slice(), b"\n"].concat()).unwrap();
    assert_eq!(
        f.env.fail(&f.new, &["commit", "--project", "forge"]),
        "validation"
    );
    assert!(
        f.env
            .home
            .path()
            .join(".local/state/tasks/rename-commit/forge.toml")
            .exists()
    );
    std::fs::write(path, original).unwrap();
    f.env.json(&f.new, &["commit", "--project", "forge"]);
    assert_eq!(git(&f.env, &f.records, &["status", "--porcelain"]), "");
}

fn assert_handoff_routing(env: &TestEnv, current: &Path, archived: &Path) {
    assert_eq!(
        env.json(current, &["show", "forge-0e720e"])["task"]["id"],
        "forge1-0e720e"
    );
    assert_eq!(
        env.json(current, &["show", "forge2-18dae2"])["task"]["id"],
        "forge-18dae2"
    );
    assert_eq!(
        env.fail(current, &["show", "forge2-0e720e"]),
        "task_not_found"
    );
    assert_eq!(env.fail(current, &["show", "0e720e"]), "task_not_found");
    assert_eq!(env.fail(archived, &["add", "forbidden"]), "retired_project");
    env.check(current);
    env.check(archived);
}

#[test]
fn multi_host_succession_rehearsal_primary_secondary_and_fresh_init() {
    for early_adoption in [true, false] {
        let a = TestEnv::new();
        let b = TestEnv::new();
        let c = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("forge");
        let new = base.path().join("forge2");
        let records = base.path().join("records");
        for path in [&old, &new, &records] {
            repo(&a, path);
        }
        for host in [&a, &b] {
            for (path, prefix) in [(&old, "forge"), (&new, "forge2")] {
                host.json(
                    path,
                    &[
                        "init",
                        "--prefix",
                        prefix,
                        "--records",
                        records.to_str().unwrap(),
                    ],
                );
            }
        }
        write_task(&records.join("forge"), "forge-0e720e", "");
        write_task(&records.join("forge2"), "forge2-18dae2", "");
        git(&a, &records, &["add", "-A"]);
        git(&a, &records, &["commit", "-qm", "seed records"]);
        let carried = id_of(a.json(&old, &["add", "carry this work"]));
        let list = base.path().join("migration.toml");
        a.json(
            &old,
            &[
                "migrate",
                "--to",
                "forge2",
                "--draft",
                list.to_str().unwrap(),
            ],
        );
        let text = std::fs::read_to_string(&list)
            .unwrap()
            .replace("decision = \"\"", "decision = \"carry\"");
        std::fs::write(&list, text).unwrap();
        a.json(&old, &["migrate", list.to_str().unwrap()]);
        a.json(
            &old,
            &["retire", "--successor", "forge2", "--reason", "superseded"],
        );
        let archived = base.path().join("archive/forge-v1");
        std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
        std::fs::rename(&old, &archived).unwrap();
        a.json(
            &archived,
            &[
                "init",
                "--prefix",
                "forge",
                "--records",
                records.to_str().unwrap(),
                "--force",
            ],
        );
        a.json(
            &archived,
            &["commit", "tasks/.config.toml", "-m", "chore: archive"],
        );
        a.json(&new, &["commit"]);
        a.json(&archived, &["rename", "forge", "forge1"]);
        a.json(&archived, &["commit", "--project", "forge1"]);
        if early_adoption {
            b.json(&archived, &["rename", "forge", "forge1", "--adopt"]);
        }
        a.json(&new, &["rename", "forge2", "forge"]);
        a.json(&new, &["commit", "--project", "forge"]);
        std::fs::rename(&new, &old).unwrap();
        a.json(
            &old,
            &[
                "init",
                "--prefix",
                "forge",
                "--records",
                records.to_str().unwrap(),
                "--force",
            ],
        );
        a.json(
            &old,
            &["commit", "tasks/.config.toml", "-m", "chore: repoint"],
        );
        eprintln!("checking primary");
        assert_handoff_routing(&a, &old, &archived);
        assert_eq!(
            a.json(&archived, &["show", &carried])["task"]["status"],
            "moved"
        );
        if !early_adoption {
            let out = b
                .cmd(&old)
                .args(["rename", "forge2", "forge", "--adopt"])
                .output()
                .unwrap();
            assert!(!out.status.success());
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("forge -> forge1"),
                "{out:?}"
            );
            b.json(&archived, &["rename", "forge", "forge1", "--adopt"]);
        }
        b.json(&old, &["rename", "forge2", "forge", "--adopt"]);
        b.json(
            &old,
            &[
                "init",
                "--prefix",
                "forge",
                "--records",
                records.to_str().unwrap(),
                "--force",
            ],
        );
        eprintln!("checking secondary");
        assert_handoff_routing(&b, &old, &archived);
        if early_adoption {
            continue;
        }
        c.json(
            &old,
            &[
                "init",
                "--prefix",
                "forge",
                "--records",
                records.to_str().unwrap(),
            ],
        );
        let out = c.cmd(&old).args(["show", "forge-0e720e"]).output().unwrap();
        assert!(!out.status.success());
        let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_ne!(err["error"]["kind"], "task_not_found");
        assert!(err["error"]["detail"].as_str().unwrap().contains("forge1"));
        let added = id_of(c.json(&old, &["add", "new host work"]));
        let reserved = config(&records.join("forge"))["predecessor"]["suffixes"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            !reserved
                .iter()
                .any(|s| added.ends_with(s.as_str().unwrap()))
        );
        c.json(
            &archived,
            &[
                "init",
                "--prefix",
                "forge1",
                "--records",
                records.to_str().unwrap(),
            ],
        );
        // A fresh host has no historical forge2 alias until that generation is adopted.
        assert_eq!(
            c.json(&old, &["show", "forge-0e720e"])["task"]["id"],
            "forge1-0e720e"
        );
        eprintln!("checking fresh host");
        c.check(&old);
        c.check(&archived);
    }
}

#[test]
fn prefix_handoff_separate_repositories_name_the_required_predecessor_commit() {
    let mut env = TestEnv::new();
    let old = env.init("forge");
    let new = env.init("forge2");
    for path in [&old, &new] {
        repo(&env, path);
    }
    write_task(&old, "forge-0e720e", "");
    write_task(&new, "forge2-18dae2", "");
    env.json(
        &old,
        &["retire", "--successor", "forge2", "--reason", "superseded"],
    );
    for path in [&old, &new] {
        git(&env, path, &["add", "-A"]);
        git(&env, path, &["commit", "-qm", "seed records"]);
    }
    env.json(&old, &["rename", "forge", "forge1"]);
    env.json(&old, &["commit", "--project", "forge1"]);
    env.json(&new, &["rename", "forge2", "forge"]);
    let out = env.json(&new, &["commit", "--project", "forge"]);
    assert!(
        out["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("tasks commit --project forge1 tasks/.config.toml")),
        "{out}"
    );
    assert_eq!(git(&env, &new, &["status", "--porcelain"]), "");
    assert!(!git(&env, &old, &["status", "--porcelain"]).is_empty());
    env.json(
        &old,
        &[
            "commit",
            "--project",
            "forge1",
            "tasks/.config.toml",
            "-m",
            "chore: update successor",
        ],
    );
    assert_eq!(git(&env, &old, &["status", "--porcelain"]), "");
    assert_handoff_routing(&env, &new, &old);
}

#[test]
fn prefix_handoff_refuses_a_later_generation_rename_without_losing_historical_routing() {
    let f = Handoff::new();
    f.step1();
    f.env.json(&f.new, &["rename", "forge2", "forge"]);
    f.env.json(&f.new, &["commit", "--project", "forge"]);
    let before = registry(&f.env);
    for (path, source, target) in [(&f.new, "forge", "newname"), (&f.old, "forge1", "archive")] {
        let out = f
            .env
            .cmd(path)
            .args(["rename", source, target])
            .output()
            .unwrap();
        assert!(!out.status.success(), "{out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("predecessor"),
            "{out:?}"
        );
        assert_eq!(registry(&f.env), before);
    }
    assert_handoff_routing(&f.env, &f.new, &f.old);
}
