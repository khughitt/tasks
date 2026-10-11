mod common;
use common::{TestEnv, docs::DocFixture};

#[test]
fn external_doc_export_is_byte_exact_in_every_format() {
    for body in ["", "No final newline", "### Task 1: one\n"] {
        let mut env = TestEnv::new();
        let dir = env.init("sci");
        let _docs = DocFixture::new(&env, body);
        let id = env.json(&dir, &["add", "Doc", "--plan", "stub:alias"])["id"]
            .as_str()
            .unwrap()
            .to_owned();
        for flags in [
            vec![],
            vec!["--json"],
            vec!["--pretty"],
            vec!["--pretty", "--color=always", "--headers"],
        ] {
            let out = env
                .cmd(&dir)
                .args(flags)
                .args(["doc", &id, "--plan"])
                .env("COLUMNS", "invalid")
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            assert_eq!(out.stdout, body.as_bytes());
        }
        let out = env
            .cmd(&dir)
            .env("TASKS_FORMAT", "pretty")
            .args(["doc", &id, "--plan"])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, body.as_bytes());
    }
}

#[test]
fn external_doc_export_selects_only_requested_link_and_validates_selectors() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Spec\n");
    let id = env.json(
        &dir,
        &["add", "Doc", "--spec", "stub:spec", "--plan", "stub:plan"],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let calls = docs.calls().len();
    let original = std::fs::read_to_string(&docs.script).unwrap();
    std::fs::write(
        &docs.script,
        "[ \"$1\" != plan ] || { printf 'plan down' >&2; exit 7; }\n".to_owned() + &original,
    )
    .unwrap();
    let out = env.cmd(&dir).args(["doc", &id, "--spec"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"# Spec\n");
    assert_eq!(docs.calls().len(), calls + 1);
    assert_eq!(env.fail(&dir, &["doc", &id, "--plan"]), "doc_not_found");
    env.usage(&dir, &["doc", &id]);
    env.usage(&dir, &["doc", &id, "--plan", "--spec"]);
    let plain = env.json(&dir, &["add", "Plain"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(env.fail(&dir, &["doc", &plain, "--plan"]), "doc_not_found");
    docs.disconnect();
    assert_eq!(env.fail(&dir, &["doc", &id, "--spec"]), "config");
}

#[test]
fn external_doc_filter_matches_exact_plan_across_parents_and_carries_steps() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n### Task 2: two\n");
    let mut completed = Vec::new();
    for (title, step) in [("Goal A", "Task 1: one"), ("Goal B", "Task 2: two")] {
        let parent = env.json(&dir, &["add", title])["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let child = env.json(
            &dir,
            &[
                "add",
                "Completed step",
                "--parent",
                &parent,
                "--plan",
                "stub:plan",
                "--step",
                step,
                "-p",
                "1",
            ],
        )["id"]
            .as_str()
            .unwrap()
            .to_owned();
        env.json(&dir, &["done", &child]);
        completed.push(child);
    }
    let open = env.json(
        &dir,
        &[
            "add",
            "Open step",
            "--plan",
            "stub:plan",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let unstepped = env.json(&dir, &["add", "Unstepped", "--plan", "stub:plan"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let calls = docs.calls();
    docs.disconnect();
    let listed = env.json(
        &dir,
        &["list", "--plan", "stub:plan", "--status", "done", "-p", "1"],
    );
    let rows = listed["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(
        |row| completed.contains(&row["id"].as_str().unwrap().to_owned())
            && row["step"].as_str().is_some()
    ));
    assert!(
        env.json(&dir, &["list", "--plan", "stub:alias"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        env.json(&dir, &["search", "Open", "--plan", "stub:plan"])["tasks"][0]["id"],
        open
    );
    env.json(&dir, &["park", &unstepped, "later"]);
    let parked = env.json(&dir, &["list", "--parked", "--plan", "stub:plan"]);
    assert_eq!(parked["tasks"][0]["id"], unstepped);
    assert!(parked["tasks"][0].get("step").is_none());
    assert_eq!(docs.calls(), calls);
}

#[test]
fn external_doc_plan_filter_composes_with_needs_review() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Plan\n");
    let mut wanted = Vec::new();
    for (title, plan, flags, needs_review) in [
        (
            "Target pending idea",
            "stub:plan",
            vec!["--status", "idea"],
            true,
        ),
        (
            "Target reviewed idea",
            "stub:plan",
            vec!["--status", "idea"],
            false,
        ),
        (
            "Target other idea",
            "stub:other",
            vec!["--status", "idea"],
            false,
        ),
        ("Target unassessed todo", "stub:plan", vec![], true),
        (
            "Target assessed todo",
            "stub:plan",
            vec!["--process", "direct"],
            false,
        ),
    ] {
        let mut args = vec!["add", title, "--plan", plan];
        args.extend(flags);
        let id = env.json(&dir, &args)["id"].as_str().unwrap().to_owned();
        if title == "Target reviewed idea" {
            env.json(&dir, &["note", &id, "--reviewed", "reviewed"]);
        }
        env.json(&dir, &["park", &id, "later"]);
        if needs_review {
            wanted.push(id);
        }
    }
    wanted.sort();
    docs.disconnect();
    for mut args in [
        vec!["list"],
        vec!["search", "Target"],
        vec!["list", "--parked"],
    ] {
        args.extend(["--plan", "stub:plan", "--needs-review"]);
        let listed = env.json(&dir, &args);
        let mut ids: Vec<_> = listed["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        assert_eq!(ids, wanted, "{args:?}");
    }
}

#[test]
fn external_doc_export_completion_does_not_fetch() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Doc\n");
    let id = env.json(&dir, &["add", "Doc", "--plan", "stub:alias"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = docs.calls();
    assert!(
        env.complete_values(&dir, "bash", 2, &["tasks", "doc", "sci-"])
            .contains(&id)
    );
    assert_eq!(docs.calls(), before);
}

#[test]
fn external_doc_export_closed_pipe_is_successful() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let _docs = DocFixture::new(&env, &"x".repeat(100000));
    let id = env.json(&dir, &["add", "Doc", "--plan", "stub:alias"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut child = env.raw(&dir).args(["doc", &id, "--plan"]).spawn().unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty());
}

#[test]
fn external_doc_show_and_next_tolerate_unavailable_open_step() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Open",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&docs.script, "printf 'provider down' >&2; exit 7\n").unwrap();
    for missing in [false, true] {
        if missing {
            docs.disconnect();
        }
        let shown = env.json(&dir, &["show", &id]);
        assert_eq!(shown["plan_path"], "stub:canonical");
        assert!(shown.get("step_found").is_none());
        assert!(!shown["warnings"].as_array().unwrap().is_empty());
        let next = env.json(&dir, &["next"]);
        assert_eq!(next["next"]["task"]["id"], id);
        assert!(next["next"].get("step_found").is_none());
    }
}

#[test]
fn external_doc_show_history_reads_only_when_stepped() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let stepped = env.json(
        &dir,
        &[
            "add",
            "Stepped",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let plain = env.json(&dir, &["add", "Plain", "--plan", "stub:alias"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for id in [&stepped, &plain] {
        env.json(&dir, &["done", id]);
    }
    assert_eq!(env.json(&dir, &["show", &stepped])["step_found"], true);
    let count = docs.calls().len();
    let shown = env.json(&dir, &["show", &plain]);
    assert!(shown.get("step_found").is_none());
    assert!(shown["warnings"].as_array().unwrap().is_empty());
    assert_eq!(docs.calls().len(), count);
    docs.set_body("# No steps\n");
    assert_eq!(env.json(&dir, &["show", &stepped])["step_found"], false);
    docs.disconnect();
    let unavailable = env.json(&dir, &["show", &stepped]);
    assert_eq!(unavailable["step_found"], false);
    assert!(
        unavailable["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("historical plan"))
    );
    assert!(
        env.json(&dir, &["show", &plain])["warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn external_doc_show_malformed_config_stays_fatal() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let _docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Open",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(
        env.home.path().join(".config/tasks/config.toml"),
        "[docs]\nunknown = true\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["show", &id]), "config");
}

#[test]
#[ignore = "production five-second display budget; short budget coverage is in resolver units"]
fn external_doc_show_production_deadline() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Open",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&docs.script, "sleep 60\n").unwrap();
    let started = std::time::Instant::now();
    let shown = env.json(&dir, &["show", &id]);
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    assert!(shown.get("step_found").is_none());
    assert!(
        shown["warnings"][0]
            .as_str()
            .unwrap()
            .contains("budget exhausted")
    );
}

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

#[test]
#[ignore = "production thirty-second strict deadline; short supervisor cases run in the fast suite"]
fn external_doc_strict_production_deadline() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "# Plan\n");
    let id = env.json(&dir, &["add", "Strict", "--plan", "stub:plan"])["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&docs.script, "sleep 60\n").unwrap();
    let started = std::time::Instant::now();
    let out = env.cmd(&dir).args(["doc", &id, "--plan"]).output().unwrap();
    let elapsed = started.elapsed();
    assert!(elapsed >= std::time::Duration::from_secs(29), "{elapsed:?}");
    assert!(elapsed < std::time::Duration::from_secs(35), "{elapsed:?}");
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "doc_not_found");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("deadline exceeded")
    );
}
