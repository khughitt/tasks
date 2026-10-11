mod common;
use common::{TestEnv, docs::DocFixture};

#[test]
fn external_doc_check_reads_shared_plan_once_and_reports_drift() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n### Task 2: two\n");
    for step in ["Task 1: one", "Task 2: two"] {
        env.json(
            &dir,
            &[
                "add",
                step,
                "--plan",
                "stub:plan",
                "--step",
                step,
                "--complexity",
                "low",
            ],
        );
    }
    let before = docs.calls().len();
    assert!(env.check(&dir)["warnings"].as_array().unwrap().is_empty());
    assert_eq!(docs.calls().len(), before + 1);
    docs.set_body("### Task 1: one\n### Task 3: unlinked\n");
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "step_missing")
    );
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "unlinked_step")
    );
}

#[test]
fn external_doc_check_unreachable_is_a_warning_and_failure_is_cached() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    for title in ["A", "B"] {
        env.json(
            &dir,
            &[
                "add",
                title,
                "--plan",
                "stub:plan",
                "--step",
                "Task 1: one",
                "--complexity",
                "low",
            ],
        );
    }
    let original = std::fs::read_to_string(&docs.script).unwrap();
    std::fs::write(
        &docs.script,
        original + "\nprintf 'provider down' >&2; exit 7\n",
    )
    .unwrap();
    let before = docs.calls().len();
    let failed = env.check(&dir);
    assert_eq!(docs.calls().len(), before + 1);
    assert!(failed["errors"].as_array().unwrap().is_empty());
    assert_eq!(failed["warnings"].as_array().unwrap().len(), 2);
    for item in failed["warnings"].as_array().unwrap() {
        assert_eq!(item["kind"], "doc_unreachable");
        assert!(item["detail"].as_str().unwrap().contains("provider down"));
    }
    docs.disconnect();
    let unavailable = env.check(&dir);
    assert_eq!(unavailable["warnings"].as_array().unwrap().len(), 2);
    assert!(
        unavailable["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["kind"] == "doc_unreachable")
    );
}

#[test]
fn external_doc_check_closed_only_failure_has_no_drift_finding() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Plan\n");
    let id = env.json(&dir, &["add", "Closed", "--plan", "stub:plan"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.json(&dir, &["done", &id]);
    docs.disconnect();
    assert!(env.check(&dir)["warnings"].as_array().unwrap().is_empty());
}

#[test]
fn external_doc_check_findings_ignore_worker_completion_order() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let mut ids = Vec::new();
    for name in ["a", "b", "c", "d"] {
        ids.push(
            env.json(
                &dir,
                &[
                    "add",
                    name,
                    "--plan",
                    &format!("stub:{name}"),
                    "--step",
                    "Task 1: one",
                    "--complexity",
                    "low",
                ],
            )["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    std::fs::write(&docs.script, "case \"$1\" in a) sleep 0.08;; b) sleep 0.06;; c) sleep 0.04;; d) sleep 0.02;; esac\nprintf '{\"id\":\"%s\",\"body\":\"\"}' \"$1\"\n").unwrap();
    ids.sort();
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let errors = report["errors"].as_array().unwrap();
    assert!(errors.iter().all(|item| item["kind"] == "step_missing"));
    assert_eq!(
        errors
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn external_doc_check_malformed_host_config_is_fatal_before_fetch() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Plan\n");
    env.json(&dir, &["add", "Open", "--plan", "stub:plan"]);
    let calls = docs.calls();
    std::fs::write(
        env.home.path().join(".config/tasks/config.toml"),
        "[docs]\nunknown = true\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["check"]), "config");
    assert_eq!(docs.calls(), calls);
}

#[test]
#[ignore = "production five-second budget; short deadline matrix is in resolver units"]
fn external_doc_check_production_deadline() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Plan\n");
    for name in ["a", "b", "c", "d", "e", "f"] {
        env.json(&dir, &["add", name, "--plan", &format!("stub:{name}")]);
    }
    let local = dir.join("docs/plans/local.md");
    std::fs::write(&local, "# Local\n").unwrap();
    env.json(&dir, &["add", "Local", "--plan", "local"]);
    std::fs::remove_file(local).unwrap();
    std::fs::write(&docs.script, "sleep 60\n").unwrap();
    let started = std::time::Instant::now();
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "doc_missing")
    );
    let warnings = report["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["kind"], "doc_unchecked");
    for name in ["a", "b", "c", "d", "e", "f"] {
        assert!(
            warnings[0]["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("stub:{name}"))
        );
    }
}

#[test]
fn external_doc_unrelated_edit_does_not_fetch() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let calls = docs.calls();
    docs.disconnect();
    env.json(&dir, &["edit", &id, "--title", "After", "-p", "1"]);
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--spec",
            "stub:spec",
            "--plan",
            "stub:canonical",
            "--step",
            "Task 1: one",
        ],
    );
    assert_eq!(docs.calls(), calls);
    let rows = env.json(&dir, &["list"]);
    assert_eq!(rows["tasks"][0]["title"], "After");
    assert_eq!(rows["tasks"][0]["priority"], 1);
    env.json(&dir, &["edit", &id, "--no-spec"]);
    env.json(&dir, &["edit", &id, "--no-step", "--no-plan"]);
    assert_eq!(docs.calls(), calls);
}

#[test]
fn external_doc_changed_values_fetch_only_needed_documents() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n### Task 2: two\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.json(&dir, &["edit", &id, "--spec", "stub:new-spec"]);
    assert_eq!(docs.calls(), ["spec", "alias", "new-spec"]);
    env.json(&dir, &["edit", &id, "--step", "Task 2: two"]);
    assert_eq!(docs.calls(), ["spec", "alias", "new-spec", "canonical"]);
    env.json(&dir, &["edit", &id, "--plan", "stub:new-plan"]);
    assert_eq!(
        docs.calls(),
        ["spec", "alias", "new-spec", "canonical", "new-plan"]
    );
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    docs.set_body("# no steps\n");
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--step", "Task 3: absent"]),
        "validation"
    );
    assert_eq!(std::fs::read(record).unwrap(), before);
}

#[test]
fn external_doc_editor_skips_unchanged_links_and_canonicalizes_changes() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let editor = env.home.path().join("edit.sh");
    std::fs::write(
        &editor,
        "sed -i 's/^spec:.*/spec: \"stub:alias\"/' \"$1\"\n",
    )
    .unwrap();
    let command = format!("sh '{}'", editor.display());
    let out = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(docs.calls(), ["spec", "alias", "alias"]);
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["spec"],
        "stub:canonical"
    );
    let calls = docs.calls();
    docs.disconnect();
    std::fs::write(&editor, "printf '\\nBody edit\\n' >> \"$1\"\n").unwrap();
    let out = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(docs.calls(), calls);
    let record = dir.join(format!("tasks/{id}.md"));
    assert!(
        std::fs::read_to_string(&record)
            .unwrap()
            .contains("\nBody edit\n")
    );
    let before = std::fs::read(&record).unwrap();
    std::fs::write(
        &editor,
        "sed -i 's/^step:.*/step: \"Task 2: absent\"/' \"$1\"\n",
    )
    .unwrap();
    let refused = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("edit kept at"));
    assert_eq!(std::fs::read(&record).unwrap(), before);
    assert!(std::fs::read_dir(dir.join("tasks")).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".edit.md")
    }));
}

#[test]
fn external_doc_add_canonicalizes_and_reuses_body_for_step() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let created = env.json(
        &dir,
        &[
            "add",
            "External",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    );
    let id = created["id"].as_str().unwrap();
    assert_eq!(docs.calls(), ["alias"]);
    let shown = env.json(&dir, &["show", id]);
    assert_eq!(shown["task"]["plan"], "stub:canonical");
    assert_eq!(shown["plan_path"], "stub:canonical");
    assert_eq!(shown["step_found"], true);
    docs.disconnect();
    let listed = env.json(&dir, &["list"]);
    assert_eq!(listed["tasks"][0]["id"], id);
}

#[test]
fn external_doc_fetch_failure_leaves_no_record() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "");
    std::fs::write(&docs.script, "printf 'provider unavailable' >&2; exit 7\n").unwrap();
    let out = env
        .cmd(&dir)
        .args(["add", "External", "--spec", "stub:alias"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "doc_not_found");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("provider unavailable"),
        "{error}"
    );
    assert!(
        env.json(&dir, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
