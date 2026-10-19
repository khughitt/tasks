mod common;
use common::{TestEnv, check_findings, findings_of, id_of, worktree_with_excluded_docs, write_doc};
use std::time::Duration;

fn set_refs(env: &TestEnv, dir: &std::path::Path, id: &str, entries: &str) {
    let rel = format!("tasks/{id}.md");
    let text = env.read(dir, &rel);
    let section = format!("## References\n\n{entries}\n\n");
    let text = if text.contains("## Notes") {
        text.replacen("## Notes", &(section + "## Notes"), 1)
    } else {
        format!("{text}\n{section}")
    };
    std::fs::write(dir.join(rel), text).unwrap();
}

fn bounded_check(env: &TestEnv, dir: &std::path::Path) -> (i32, serde_json::Value) {
    let out = env
        .cmd(dir)
        .arg("check")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let findings = if out.stdout.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&out.stdout).unwrap()
    };
    (out.status.code().unwrap(), findings)
}

#[test]
fn check_reports_reference_audit_findings_on_open_tasks_only() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/x.md", "x");
    let id = id_of(env.json(&dir, &["add", "A"]));
    // Hand-write the body and section: the CLI writers arrive in Task 5.
    let rel = format!("tasks/{id}.md");
    let text = env.read(&dir, &rel).replacen("\n---\n", "\n---\n\nSee [ref:gone] and [ref:Bad].\n\n## References\n\n- a (context): docs/x.md\n- b (input): docs/x.md\n\n", 1);
    std::fs::write(dir.join(&rel), text).unwrap();
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 1, "{v}");
    for kind in [
        "ref_duplicate",
        "ref_citation_dangling",
        "ref_citation_malformed",
    ] {
        assert_eq!(findings_of(&v, "errors", kind).len(), 1, "{kind}: {v}");
    }
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["done", &id, "x"]);
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn check_reports_missing_main_checkout_and_special_targets_on_open_tasks() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);
    write_doc(&side, "scratch/shot.png", "png");
    env.json(&side, &["attach", &id, "scratch/shot.png"]);
    let stored = side.join(format!("tasks/files/{id}/shot.png"));
    std::fs::remove_file(&stored).unwrap();
    set_refs(
        &env,
        &side,
        &id,
        "- i (input): docs/specs/x-design.md\n- o (output): docs/later.md\n- f (context): files:gone.png\n- s (input): files:shot.png\n- e (input): unconfigured:external",
    );
    let (code, v) = check_findings(&env, &side);
    assert_eq!(code, 1, "{v}");
    let warnings = findings_of(&v, "warnings", "ref_in_main_checkout");
    assert_eq!(warnings.len(), 1, "{v}");
    assert_eq!(warnings[0]["id"], id);
    let detail = warnings[0]["detail"].as_str().unwrap();
    assert!(detail.contains("reference i docs/specs/x-design.md"), "{v}");
    assert!(detail.contains(main.to_str().unwrap()), "{v}");
    assert!(
        detail.contains(main.join("docs/specs/x-design.md").to_str().unwrap()),
        "{v}"
    );
    let missing = findings_of(&v, "errors", "ref_missing");
    assert_eq!(missing.len(), 2, "{v}");
    for (handle, name) in [("f", "gone.png"), ("s", "shot.png")] {
        assert!(
            missing.iter().any(|f| f["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("reference {handle} files:{name}"))
                && f["detail"].as_str().unwrap().contains(
                    side.join(format!("tasks/files/{id}/{name}"))
                        .to_str()
                        .unwrap()
                )),
            "{v}"
        );
    }
    assert_eq!(
        findings_of(&v, "errors", "attachment_missing").len(),
        1,
        "{v}"
    );

    std::fs::remove_file(main.join("docs/specs/x-design.md")).unwrap();
    let (code, v) = check_findings(&env, &side);
    assert_eq!(code, 1, "{v}");
    assert!(
        findings_of(&v, "warnings", "ref_in_main_checkout").is_empty(),
        "{v}"
    );
    let missing = findings_of(&v, "errors", "ref_missing");
    assert_eq!(missing.len(), 3, "{v}");
    assert!(
        missing.iter().any(|f| f["detail"]
            .as_str()
            .unwrap()
            .contains("reference i docs/specs/x-design.md")
            && f["detail"]
                .as_str()
                .unwrap()
                .contains(side.join("docs/specs/x-design.md").to_str().unwrap())),
        "{v}"
    );

    std::fs::create_dir_all(side.join("docs")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(side.join("docs/pipe"))
            .status()
            .unwrap()
            .success()
    );
    let rel = format!("tasks/{id}.md");
    let text = env.read(&side, &rel).replace(
        "- o (output): docs/later.md",
        "- o (output): docs/later.md\n- p (context): docs/pipe\n- q (output): docs/pipe-out",
    );
    std::fs::write(side.join(rel), text).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(side.join("docs/pipe-out"))
            .status()
            .unwrap()
            .success()
    );
    let (code, v) = bounded_check(&env, &side);
    assert_eq!(code, 1, "{v}");
    let specials = findings_of(&v, "errors", "special_file");
    assert_eq!(specials.len(), 2, "{v}");
    for (handle, path) in [("p", "docs/pipe"), ("q", "docs/pipe-out")] {
        assert!(
            specials.iter().any(|f| f["id"] == id
                && f["detail"].as_str().unwrap().contains(&format!(
                    "reference {handle}: {}",
                    side.join(path).display()
                ))),
            "{v}"
        );
    }
    env.cmd(&side)
        .args(["start", &id])
        .timeout(Duration::from_secs(10))
        .assert()
        .success();
    env.cmd(&side)
        .args(["done", &id, "x"])
        .timeout(Duration::from_secs(10))
        .assert()
        .success();
    let (code, v) = bounded_check(&env, &side);
    assert_eq!(code, 1, "{v}"); // The existing attachment audit also covers closed records.
    assert_eq!(
        findings_of(&v, "errors", "attachment_missing").len(),
        1,
        "{v}"
    );
    for kind in [
        "ref_missing",
        "ref_in_main_checkout",
        "special_file",
        "doc_missing",
    ] {
        assert!(findings_of(&v, "errors", kind).is_empty(), "{v}");
        assert!(findings_of(&v, "warnings", kind).is_empty(), "{v}");
    }
}

#[test]
fn a_fifo_at_a_linked_plan_path_is_refused_not_opened() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/plans/x.md", "### Task 1: A\n");
    let id = id_of(env.json(&dir, &["add", "T", "--plan", "x", "--step", "Task 1: A"]));
    let path = dir.join("docs/plans/x.md");
    std::fs::remove_file(&path).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let out = env
        .cmd(&dir)
        .args(["doc", &id, "--plan"])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "special_file", "{error}");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(path.to_str().unwrap()),
        "{error}"
    );
    let (code, v) = bounded_check(&env, &dir);
    assert_eq!(code, 1, "{v}");
    let findings = findings_of(&v, "errors", "special_file");
    assert_eq!(findings.len(), 1, "{v}");
    assert_eq!(findings[0]["id"], id);
    assert!(
        findings[0]["detail"]
            .as_str()
            .unwrap()
            .contains(path.to_str().unwrap()),
        "{v}"
    );
    assert!(findings_of(&v, "errors", "doc_missing").is_empty(), "{v}");
}

#[test]
fn main_only_plan_fifo_is_refused_and_main_regular_file_remains_readable() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);
    let rel = "docs/plans/x.md";
    let expected = std::fs::read(main.join(rel)).unwrap();
    std::fs::create_dir_all(side.join(rel)).unwrap();
    let out = env
        .cmd(&side)
        .args(["doc", &id, "--plan"])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, expected);
    std::fs::remove_dir(side.join(rel)).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(side.join(rel))
            .status()
            .unwrap()
            .success()
    );
    let out = env
        .cmd(&side)
        .args(["doc", &id, "--plan"])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, expected);
    let (code, v) = bounded_check(&env, &side);
    assert_eq!(code, 0, "{v}");
    assert!(findings_of(&v, "errors", "special_file").is_empty(), "{v}");
    std::fs::remove_file(side.join(rel)).unwrap();
    std::fs::remove_file(main.join(rel)).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(main.join(rel))
            .status()
            .unwrap()
            .success()
    );
    let out = env
        .cmd(&side)
        .args(["doc", &id, "--plan"])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "special_file", "{error}");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(main.join(rel).to_str().unwrap()),
        "{error}"
    );
    let (code, v) = bounded_check(&env, &side);
    assert_eq!(code, 1, "{v}");
    let specials = findings_of(&v, "errors", "special_file");
    assert_eq!(specials.len(), 1, "{v}");
    assert!(
        specials[0]["detail"]
            .as_str()
            .unwrap()
            .contains(main.join(rel).to_str().unwrap()),
        "{v}"
    );
    assert!(findings_of(&v, "errors", "doc_missing").is_empty(), "{v}");
}

#[test]
fn check_preserves_attachment_ledger_and_storage_safety() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "T"]));
    write_doc(&dir, "scratch/source.png", "png");
    for name in ["valid.png", "pipe.png", "dir.png", "link.png"] {
        env.json(&dir, &["attach", &id, "scratch/source.png", "--name", name]);
    }
    let storage = dir.join(format!("tasks/files/{id}"));
    std::fs::write(storage.join("unrecorded.png"), "png").unwrap();
    set_refs(
        &env,
        &dir,
        &id,
        "- v (input): files:valid.png\n- p (context): files:pipe.png\n- d (input): files:dir.png\n- l (context): files:link.png\n- u (input): files:unrecorded.png\n- o (output): files:absent.png",
    );
    let (_, v) = check_findings(&env, &dir);
    let missing = findings_of(&v, "errors", "ref_missing");
    assert_eq!(missing.len(), 1, "{v}");
    assert!(
        missing[0]["detail"]
            .as_str()
            .unwrap()
            .contains("reference u files:unrecorded.png"),
        "{v}"
    );
    for name in ["pipe.png", "dir.png", "link.png"] {
        std::fs::remove_file(storage.join(name)).unwrap();
    }
    assert!(
        std::process::Command::new("mkfifo")
            .arg(storage.join("pipe.png"))
            .status()
            .unwrap()
            .success()
    );
    std::fs::create_dir(storage.join("dir.png")).unwrap();
    std::os::unix::fs::symlink("valid.png", storage.join("link.png")).unwrap();
    let (code, v) = bounded_check(&env, &dir);
    assert_eq!(code, 1, "{v}");
    let specials = findings_of(&v, "errors", "special_file");
    assert_eq!(specials.len(), 1, "{v}");
    assert!(
        specials[0]["detail"]
            .as_str()
            .unwrap()
            .contains(storage.join("pipe.png").to_str().unwrap()),
        "{v}"
    );
    let missing = findings_of(&v, "errors", "ref_missing");
    assert_eq!(missing.len(), 3, "{v}");
    for handle in ["d", "l"] {
        assert!(
            missing.iter().any(|f| f["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("reference {handle}:"))
                && f["detail"]
                    .as_str()
                    .unwrap()
                    .contains("is not a usable attachment")),
            "{v}"
        );
    }
    assert_eq!(
        findings_of(&v, "warnings", "attachment_invalid").len(),
        3,
        "{v}"
    );
    assert_eq!(
        findings_of(&v, "errors", "attachment_missing").len(),
        3,
        "{v}"
    );
    assert!(
        findings_of(&v, "errors", "attachment_unsafe").is_empty(),
        "{v}"
    );

    let moved = dir.join("moved-storage");
    std::fs::rename(&storage, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &storage).unwrap();
    let (code, v) = bounded_check(&env, &dir);
    assert_eq!(code, 1, "{v}");
    assert_eq!(
        findings_of(&v, "errors", "attachment_unsafe").len(),
        1,
        "{v}"
    );
    assert!(
        findings_of(&v, "warnings", "attachment_invalid").is_empty(),
        "{v}"
    );
    assert!(findings_of(&v, "errors", "special_file").is_empty(), "{v}");
    let missing = findings_of(&v, "errors", "ref_missing");
    assert_eq!(missing.len(), 5, "{v}");
    assert!(
        missing.iter().all(|f| f["detail"]
            .as_str()
            .unwrap()
            .contains("is not a usable attachment")
            && f["detail"].as_str().unwrap().contains("symlink")),
        "{v}"
    );
}

#[test]
fn add_and_edit_write_entries_that_show_returns() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/notes/b.md", "brief");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "T",
            "--body",
            "See [ref:brief].",
            "--ref",
            "brief (input, required, kind=brief): docs/notes/b.md — the brief",
            "--ref",
            r#"rb (output): "docs/run books/a \"v2\" \\ — x.md""#,
        ],
    ));
    let shown = env.json(&dir, &["show", &id]);
    let refs = &shown["task"]["refs"];
    assert_eq!(
        refs[0],
        serde_json::json!({"handle":"brief","relation":"input","required":true,"kind":"brief","reference":"docs/notes/b.md","description":"the brief"})
    );
    assert_eq!(
        refs[1],
        serde_json::json!({"handle":"rb","relation":"output","required":false,"reference":"docs/run books/a \"v2\" \\ — x.md"})
    );
    assert!(env.read(&dir, &format!("tasks/{id}.md")).contains(
        "## References\n\n- brief (input, required, kind=brief): docs/notes/b.md — the brief\n"
    ));
    let marker = env.home.path().join("editor-ran");
    let stub = common::write_editor_stub(&env, &marker);
    for args in [
        vec![
            "edit",
            &id,
            "--ref",
            "brief (context): docs/notes/b.md",
            "--ref",
            "c (context): https://example.org/x",
        ],
        vec!["edit", &id, "--rm-ref", "rb"],
        vec!["edit", &id, "--no-refs", "--body", "Nothing cited."],
    ] {
        env.cmd(&dir)
            .env("EDITOR", &stub)
            .args(&args)
            .timeout(Duration::from_secs(10))
            .assert()
            .success();
        assert!(!marker.exists());
        if args.contains(&"--ref") {
            let refs = env.json(&dir, &["show", &id])["task"]["refs"].clone();
            let handles: Vec<_> = refs
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["handle"].as_str().unwrap())
                .collect();
            assert_eq!(handles, ["brief", "rb", "c"]);
            assert_eq!(refs[0]["relation"], "context");
        }
    }
    assert!(env.json(&dir, &["show", &id])["task"].get("refs").is_none());
    // Clearing alone must also use the non-editor path.
    env.cmd(&dir)
        .env("EDITOR", stub)
        .args(["edit", &id, "--no-refs"])
        .timeout(Duration::from_secs(10))
        .assert()
        .success();
    assert!(!marker.exists());
}

#[test]
fn writes_refuse_broken_entries_and_citations() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/b.md", "b");
    assert_eq!(
        env.fail(&dir, &["add", "T", "--ref", "a (input): docs/none.md"]),
        "ref_not_found"
    );
    assert_eq!(
        env.fail(&dir, &["add", "T", "--body", "[ref:gone]"]),
        "ref_citation_dangling"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "add",
                "T",
                "--ref",
                "a (context): docs/b.md",
                "--ref",
                "b (input): docs/b.md"
            ]
        ),
        "ref_duplicate"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "add",
                "T",
                "--ref",
                "a (context): docs/b.md",
                "--ref",
                "a (input): docs/b.md"
            ]
        ),
        "validation"
    );
    env.json(
        &dir,
        &[
            "add",
            "T",
            "--ref",
            "o (output): docs/later.md",
            "--ref",
            "s (output): files:after.png",
        ],
    );
    env.json(
        &dir,
        &[
            "add",
            "T",
            "--body",
            "Write `[ref:x]` to cite.\nLiteral `example\n[ref:sample]` here.\n```\n[ref:y]\n```",
        ],
    );
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "T",
            "--body",
            "See [ref:a].",
            "--ref",
            "a (context): docs/b.md",
        ],
    ));
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--rm-ref", "a"]),
        "ref_citation_dangling"
    );
    env.json(
        &dir,
        &["edit", &id, "--rm-ref", "a", "--body", "Nothing cited."],
    );
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--rm-ref", "a"]),
        "validation"
    );
    for flag in ["--ref", "--rm-ref"] {
        let value = if flag == "--ref" {
            "a (output): docs/a.md"
        } else {
            "a"
        };
        let error = env.usage(&dir, &["edit", &id, "--no-refs", flag, value]);
        assert!(
            error.contains("--no-refs") && error.contains(flag),
            "{error}"
        );
    }
}

#[test]
fn an_attachment_input_needs_its_bytes_not_just_its_ledger_entry() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "T"]));
    write_doc(&dir, "shot.png", "png");
    env.json(&dir, &["attach", &id, "shot.png"]);
    std::fs::remove_file(dir.join(format!("tasks/files/{id}/shot.png"))).unwrap();
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--ref", "s (input): files:shot.png"]),
        "ref_not_found"
    );
}

#[test]
fn an_unrelated_edit_ignores_an_input_that_has_since_vanished() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/b.md", "b");
    let id = id_of(env.json(&dir, &["add", "T", "--ref", "a (input): docs/b.md"]));
    std::fs::remove_file(dir.join("docs/b.md")).unwrap();
    env.json(&dir, &["edit", &id, "-p", "1"]);
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--ref",
            "a (input, required): docs/b.md — still valid",
        ],
    );
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--ref", "a (context): docs/b.md"]),
        "ref_not_found"
    );
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--ref", "a (input): docs/c.md"]),
        "ref_not_found"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["priority"], 1);
}

#[test]
fn editor_saves_validate_added_entries() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/b.md", "b");
    for notes in [false, true] {
        let id = id_of(env.json(&dir, &["add", "T"]));
        if notes {
            env.json(&dir, &["note", &id, "A note"]);
        }
        for (entry, expected) in [
            ("x (input): docs/none.md", Some("ref_not_found")),
            ("x (input): docs/b.md", None),
        ] {
            let section = format!("## References\\n\\n- {entry}\\n\\n");
            let body = if notes {
                format!("sed -i '/^## Notes/i {section}' \"$1\"")
            } else {
                format!("printf '\\n{section}' >> \"$1\"")
            };
            let editor = common::editor_script(env.home.path(), &body);
            let out = env
                .cmd(&dir)
                .env("EDITOR", editor)
                .args(["edit", &id])
                .timeout(Duration::from_secs(10))
                .output()
                .unwrap();
            if let Some(kind) = expected {
                assert_eq!(out.status.code(), Some(1), "{out:?}");
                let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
                assert_eq!(error["error"]["kind"], kind);
                let detail = error["error"]["detail"].as_str().unwrap();
                let kept = detail
                    .split(" (edit kept at ")
                    .nth(1)
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                assert!(std::fs::read_to_string(kept).unwrap().contains(entry));
                assert!(
                    !env.read(&dir, &format!("tasks/{id}.md"))
                        .contains("## References")
                );
            } else {
                assert!(out.status.success(), "{out:?}");
                assert_eq!(
                    env.json(&dir, &["show", &id])["task"]["refs"][0]["handle"],
                    "x"
                );
            }
        }
    }
}

#[test]
fn list_and_search_filter_by_reference_byte_for_byte() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/b.md", "b");
    write_doc(&dir, "docs/c.md", "c");
    let id = id_of(env.json(&dir, &["add", "T one", "--ref", "b (context): docs/b.md"]));
    let other = id_of(env.json(&dir, &["add", "T two", "--ref", "c (context): docs/c.md"]));
    for parked in [false, true] {
        if parked {
            for task in [&id, &other] {
                env.json(&dir, &["start", task]);
                env.json(&dir, &["park", task, "resume"]);
            }
        }
        for command in [vec!["list"], vec!["search", "T"]] {
            for (reference, count) in [("docs/b.md", 1), ("docs/b", 0)] {
                let mut args = command.clone();
                args.extend(["--ref", reference]);
                if parked {
                    args.push("--parked");
                }
                let result = env.json(&dir, &args);
                let rows = result["tasks"].as_array().unwrap();
                assert_eq!(rows.len(), count, "{result}");
                if count == 1 {
                    assert_eq!(rows[0]["id"], id);
                }
            }
        }
    }
    std::fs::remove_file(dir.join(format!("tasks/{id}.md"))).unwrap();
    assert_eq!(
        env.json(&dir, &["list", "--parked"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        env.json(&dir, &["list", "--parked", "--ref", "docs/b.md"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn doc_reads_an_entry_by_handle() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/b.md", "body\n");
    write_doc(&dir, "docs/folder/child", "child");
    std::fs::write(dir.join("docs/binary"), [0xff, 0xfe]).unwrap();
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "T",
            "--ref",
            "text (context): docs/b.md",
            "--ref",
            "folder (input): docs/folder",
            "--ref",
            "bytes (input): docs/binary",
            "--ref",
            "later (output): docs/later.md",
            "--ref",
            "external (context): stub:x",
        ],
    ));
    for (handle, kind) in [
        ("folder", "ref_not_text"),
        ("bytes", "ref_not_text"),
        ("later", "doc_not_found"),
        ("unknown", "doc_not_found"),
    ] {
        assert_eq!(env.fail(&dir, &["doc", &id, handle]), kind);
    }
    env.cmd(&dir)
        .args(["doc", &id, "text"])
        .assert()
        .success()
        .stdout("body\n");
    env.json(&dir, &["attach", &id, "docs/b.md"]);
    env.json(
        &dir,
        &["edit", &id, "--ref", "attachment (input): files:b.md"],
    );
    env.cmd(&dir)
        .args(["doc", &id, "attachment"])
        .assert()
        .success()
        .stdout("body\n");
    let fixture = common::docs::DocFixture::new(&env, "external body\n");
    env.cmd(&dir)
        .args(["doc", &id, "external"])
        .assert()
        .success()
        .stdout("external body\n");
    assert_eq!(
        env.fail(&dir, &["doc", &id, "external", "--no-fetch"]),
        "doc_not_found"
    );
    assert_eq!(fixture.calls(), ["x"]);
    for flag in ["--plan", "--spec"] {
        env.usage(&dir, &["doc", &id, "text", flag]);
    }
}

#[test]
fn a_fifo_is_refused_without_being_opened() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(dir.join("docs/pipe"))
            .status()
            .unwrap()
            .success()
    );
    let id = id_of(env.json(&dir, &["add", "T"]));
    set_refs(&env, &dir, &id, "- p (context): docs/pipe");
    for args in [
        vec!["add", "T", "--ref", "p (context): docs/pipe"],
        vec!["doc", &id, "p"],
    ] {
        let out = env
            .cmd(&dir)
            .args(&args)
            .timeout(Duration::from_secs(10))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "special_file");
    }
}

#[test]
fn entry_targets_use_main_checkout_but_prefer_local_directories() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);
    let result = env.json(
        &side,
        &[
            "edit",
            &id,
            "--ref",
            "brief (input): docs/specs/x-design.md",
        ],
    );
    let warning = format!(
        "reference brief docs/specs/x-design.md is not in this checkout; read from the main checkout at {}",
        main.display()
    );
    assert!(
        result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == &warning)
    );
    let out = env.cmd(&side).args(["doc", &id, "brief"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"# X\n");
    assert!(String::from_utf8(out.stderr).unwrap().contains(&warning));
    std::fs::create_dir_all(side.join("docs/specs/x-design.md")).unwrap();
    assert_eq!(env.fail(&side, &["doc", &id, "brief"]), "ref_not_text");
}

#[test]
fn editor_body_audit_keeps_the_edit_and_unrelated_changes_ignore_existing_prose() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "T", "--body", "Original body."]));
    let editor = common::editor_script(
        env.home.path(),
        "sed -i 's/Original body./See [ref:missing]./' \"$1\"",
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", editor)
        .args(["edit", &id])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "ref_citation_dangling");
    let detail = error["error"]["detail"].as_str().unwrap();
    let kept = detail
        .split(" (edit kept at ")
        .nth(1)
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert!(
        std::fs::read_to_string(kept)
            .unwrap()
            .contains("See [ref:missing].")
    );
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["body"],
        "Original body."
    );
    let record = dir.join(format!("tasks/{id}.md"));
    std::fs::write(
        &record,
        env.read(&dir, &format!("tasks/{id}.md"))
            .replace("Original body.", "See [ref:missing]."),
    )
    .unwrap();
    env.json(&dir, &["edit", &id, "-p", "1"]);
    let editor = common::editor_script(
        env.home.path(),
        "sed -i 's/^priority: 1$/priority: 2/' \"$1\"",
    );
    env.cmd(&dir)
        .env("EDITOR", editor)
        .args(["edit", &id])
        .timeout(Duration::from_secs(10))
        .assert()
        .success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["priority"], 2);
}

#[test]
fn unsafe_attachment_storage_is_refused_by_writes_and_doc() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "T"]));
    write_doc(&dir, "shot.png", "png");
    env.json(&dir, &["attach", &id, "shot.png"]);
    env.json(
        &dir,
        &["edit", &id, "--ref", "shot (output): files:shot.png"],
    );
    let storage = dir.join(format!("tasks/files/{id}"));
    let moved = dir.join("moved-storage");
    std::fs::rename(&storage, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &storage).unwrap();
    assert_eq!(
        env.fail(
            &dir,
            &["edit", &id, "--ref", "shot (input): files:shot.png"]
        ),
        "attachment_unsafe"
    );
    assert_eq!(env.fail(&dir, &["doc", &id, "shot"]), "attachment_unsafe");
}

fn inventory_fixture(env: &mut TestEnv) -> (std::path::PathBuf, String) {
    let dir = env.init("rf");
    write_doc(&dir, "docs/specs/x-design.md", "# X");
    write_doc(&dir, "docs/plans/x.md", "### Task 1: A");
    write_doc(&dir, "docs/notes/b.md", "brief");
    for name in ["cited.png", "shot.png"] {
        write_doc(&dir, &format!("scratch/{name}"), "png");
    }
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "Inventory",
            "--spec",
            "x",
            "--plan",
            "x",
            "--step",
            "Task 1: A",
            "--source",
            "user:2026-10-19 chat",
            "--ref",
            "brief (input, required, kind=brief): docs/notes/b.md — the brief",
            "--ref",
            "ctx (context): https://e.org/x",
            "--ref",
            "out (output, required): docs/out.md",
        ],
    ));
    for name in ["cited.png", "shot.png"] {
        env.json(&dir, &["attach", &id, &format!("scratch/{name}")]);
    }
    env.json(
        &dir,
        &["edit", &id, "--ref", "c (context): files:cited.png"],
    );
    (dir, id)
}

fn bounded_show(env: &TestEnv, dir: &std::path::Path, id: &str) -> serde_json::Value {
    let out = env
        .cmd(dir)
        .args(["show", id])
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn fifo(path: &std::path::Path) {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn the_inventory_lists_everything_in_order_with_origins() {
    let mut env = TestEnv::new();
    let (dir, id) = inventory_fixture(&mut env);
    let v = env.json(&dir, &["show", &id]);
    let rows = v["inventory"].as_array().unwrap();
    let summary: Vec<(&str, &str, bool)> = rows
        .iter()
        .map(|r| {
            (
                r["origin"].as_str().unwrap(),
                r["status"].as_str().unwrap(),
                r["required"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("spec", "present", true),
            ("plan", "present", true),
            ("ref", "present", true),
            ("ref", "unchecked", false),
            ("ref", "pending", true),
            ("ref", "present", false),
            ("source", "unchecked", false),
            ("attachment", "present", false)
        ]
    );
    assert_eq!(rows[1]["step"], "Task 1: A");
    assert_eq!(rows[1]["step_found"], true);
    assert_eq!(rows[6]["location"], "opaque");
    assert_eq!(rows[5]["location"], "attachment");
    assert_eq!(rows[7]["reference"], "files:shot.png");
    for key in [
        "handle",
        "relation",
        "kind",
        "description",
        "step",
        "step_found",
        "cause",
    ] {
        assert!(rows[0].get(key).is_none(), "{key}: {v}");
    }
    assert!(
        rows[2]["path"]
            .as_str()
            .unwrap()
            .ends_with("docs/notes/b.md")
    );
    assert!(rows[3].get("path").is_none());
    let bare = id_of(env.json(&dir, &["add", "Bare"]));
    assert!(env.json(&dir, &["show", &bare]).get("inventory").is_none());
}

#[test]
fn attachment_rows_come_from_the_ledger() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "A"]));
    for name in ["b.png", "a.png", "detached.png"] {
        write_doc(&dir, &format!("scratch/{name}"), "png");
        env.json(&dir, &["attach", &id, &format!("scratch/{name}")]);
    }
    env.json(&dir, &["detach", &id, "detached.png", "not needed"]);
    std::fs::remove_file(dir.join(format!("tasks/files/{id}/b.png"))).unwrap();
    write_doc(&dir, &format!("tasks/files/{id}/stray.png"), "stray");
    let v = env.json(&dir, &["show", &id]);
    let rows = v["inventory"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{v}");
    assert_eq!(rows[0]["reference"], "files:a.png");
    assert_eq!(rows[0]["status"], "present");
    assert_eq!(rows[1]["reference"], "files:b.png");
    assert_eq!(rows[1]["status"], "missing");
    let warnings = v["warnings"].to_string();
    assert!(
        warnings.contains("[attachment_missing]") && warnings.contains("b.png"),
        "{v}"
    );
    assert!(warnings.contains("stray.png"), "{v}");
}

#[test]
fn a_missing_stepped_plan_on_an_open_task_is_a_missing_row() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/plans/x.md", "### Task 1: A");
    let id = id_of(env.json(&dir, &["add", "A", "--plan", "x", "--step", "Task 1: A"]));
    std::fs::remove_file(dir.join("docs/plans/x.md")).unwrap();
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][0]["status"], "missing");
    assert!(v.get("step_found").is_none());
    assert!(v["inventory"][0].get("step_found").is_none());
    std::fs::create_dir(dir.join("docs/plans/x.md")).unwrap();
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][0]["status"], "missing");
    assert!(v.get("step_found").is_none());
    assert!(
        v["warnings"]
            .to_string()
            .contains("is not a regular file; its step is not checked")
    );
}

#[test]
fn an_entry_equal_to_the_spec_merges_once_targets_agree() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "A",
            "--ref",
            "s (output, kind=spec): docs/specs/y-design.md",
            "--body",
            "[ref:s]",
        ],
    ));
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][0]["status"], "pending");
    write_doc(&dir, "docs/specs/y-design.md", "# Y");
    env.json(&dir, &["edit", &id, "--spec", "docs/specs/y-design.md"]);
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 1);
    let row = &v["inventory"][0];
    assert_eq!(row["origin"], "spec");
    assert_eq!(row["handle"], "s");
    assert_eq!(row["relation"], "output");
    assert_eq!(row["kind"], "spec");
    assert_eq!(row["required"], true);
    env.json(&dir, &["edit", &id, "--body", "[ref:s] again"]);
    std::fs::remove_file(dir.join("docs/specs/y-design.md")).unwrap();
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 1);
    assert_eq!(v["inventory"][0]["status"], "missing");
}

#[test]
fn merge_keeps_separate_rows_when_targets_differ() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);
    std::fs::create_dir_all(side.join("docs/specs/x-design.md")).unwrap();
    set_refs(&env, &side, &id, "- d (context): docs/specs/x-design.md");
    let v = env.json(&side, &["show", &id]);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 3);
    assert_eq!(
        v["inventory"][0]["path"],
        main.join("docs/specs/x-design.md").to_str().unwrap()
    );
    assert_eq!(
        v["inventory"][2]["path"],
        side.join("docs/specs/x-design.md").to_str().unwrap()
    );
    assert_eq!(v["inventory"][2]["status"], "present");
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains("reference d")
                && w.contains(main.join("docs/specs/x-design.md").to_str().unwrap())
                && w.contains(side.join("docs/specs/x-design.md").to_str().unwrap())
        }),
        "{v}"
    );
    std::fs::remove_file(main.join("docs/specs/x-design.md")).unwrap();
    let v = env.json(&side, &["show", &id]);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 3);
    assert_eq!(v["inventory"][0]["status"], "missing");
    assert_eq!(v["inventory"][2]["status"], "present");
}

#[test]
fn external_plan_rows_report_the_fetch_outcome() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let fixture = common::docs::DocFixture::new(&env, "### Task 1: A");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "A",
            "--spec",
            "stub:s",
            "--plan",
            "stub:p",
            "--step",
            "Task 1: A",
            "--ref",
            "e (input): stub:entry",
        ],
    ));
    let count = fixture.calls().len();
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][0]["status"], "unchecked");
    assert_eq!(v["inventory"][1]["status"], "available");
    assert_eq!(v["inventory"][1]["step_found"], true);
    assert_eq!(v["inventory"][2]["status"], "unchecked");
    assert_eq!(&fixture.calls()[count..], ["p"]);
    let v = env.json(&dir, &["show", &id, "--no-fetch"]);
    assert_eq!(v["inventory"][1]["status"], "unchecked");
    assert!(v["inventory"][1].get("step_found").is_none());
    fixture.disconnect();
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][1]["status"], "unavailable");
    assert!(v["inventory"][1]["cause"].as_str().is_some());
    assert!(v["inventory"][0].get("cause").is_none());
}

#[test]
fn show_stays_readable_with_special_files_and_reports_them() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/plans/x.md", "### Task 1: A");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "A",
            "--plan",
            "x",
            "--step",
            "Task 1: A",
            "--ref",
            "p (context): docs/plans/x.md",
            "--ref",
            "o (output): docs/out-pipe",
        ],
    ));
    std::fs::remove_file(dir.join("docs/plans/x.md")).unwrap();
    fifo(&dir.join("docs/plans/x.md"));
    fifo(&dir.join("docs/out-pipe"));
    let v = bounded_show(&env, &dir, &id);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 2);
    assert_eq!(v["inventory"][0]["handle"], "p");
    for (i, rel) in [(0, "docs/plans/x.md"), (1, "docs/out-pipe")] {
        assert_eq!(v["inventory"][i]["status"], "special");
        assert_eq!(v["inventory"][i]["path"], dir.join(rel).to_str().unwrap());
        assert!(
            v["warnings"]
                .to_string()
                .contains(dir.join(rel).to_str().unwrap()),
            "{v}"
        );
    }
    assert!(v.get("step_found").is_none());
    assert!(v["inventory"][0].get("step_found").is_none());
}

#[test]
fn special_and_symlinked_attachments_stay_readable() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "A"]));
    for name in ["a.png", "b.png", "c.png"] {
        write_doc(&dir, &format!("scratch/{name}"), "png");
        env.json(&dir, &["attach", &id, &format!("scratch/{name}")]);
    }
    env.json(&dir, &["edit", &id, "--ref", "s (input): files:a.png"]);
    for name in ["a.png", "b.png", "c.png"] {
        let path = dir.join(format!("tasks/files/{id}/{name}"));
        std::fs::remove_file(&path).unwrap();
        if name == "c.png" {
            std::os::unix::fs::symlink(dir.join("scratch/c.png"), &path).unwrap();
        } else {
            fifo(&path);
        }
    }
    let v = bounded_show(&env, &dir, &id);
    let rows = v["inventory"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["handle"], "s");
    assert_eq!(rows[1]["origin"], "attachment");
    for (i, name) in [(0, "a.png"), (1, "b.png"), (2, "c.png")] {
        assert_eq!(
            rows[i]["status"],
            if i == 2 { "missing" } else { "special" }
        );
        assert_eq!(
            rows[i]["path"],
            dir.join(format!("tasks/files/{id}/{name}"))
                .to_str()
                .unwrap()
        );
    }
    let warnings = v["warnings"].as_array().unwrap();
    for code in ["attachment_invalid", "attachment_missing"] {
        assert!(
            warnings
                .iter()
                .any(|w| w.as_str().unwrap().contains(&format!("[{code}]"))
                    && w.as_str().unwrap().contains("c.png")),
            "{v}"
        );
    }
    assert!(
        !v["warnings"].to_string().contains("[attachment_unsafe]"),
        "{v}"
    );
}

#[test]
fn main_only_special_governing_rows_keep_the_actual_path_and_merge() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);
    let path = main.join("docs/specs/x-design.md");
    std::fs::remove_file(&path).unwrap();
    fifo(&path);
    let v = bounded_show(&env, &side, &id);
    assert_eq!(v["inventory"][0]["status"], "special");
    assert_eq!(v["inventory"][0]["path"], path.to_str().unwrap());
    set_refs(&env, &side, &id, "- d (context): docs/specs/x-design.md");
    let v = bounded_show(&env, &side, &id);
    assert_eq!(v["inventory"].as_array().unwrap().len(), 2);
    assert_eq!(v["inventory"][0]["handle"], "d");
    assert_eq!(v["inventory"][0]["status"], "special");
    assert_eq!(v["inventory"][0]["path"], path.to_str().unwrap());
}

#[test]
fn pretty_show_has_a_references_section() {
    let mut env = TestEnv::new();
    let (dir, id) = inventory_fixture(&mut env);
    let out = env.pretty(&dir, &["show", &id]);
    assert!(out.contains("── references"), "{out}");
    assert!(out.contains("read first: spec, plan, brief, out"), "{out}");
    assert!(
        out.contains("- brief (input, required, kind=brief): docs/notes/b.md — the brief"),
        "{out}"
    );
    assert!(
        out.contains("- out (output, required): docs/out.md [pending]"),
        "{out}"
    );
    assert!(
        out.contains("- shot.png (attachment): files:shot.png"),
        "{out}"
    );
    assert!(
        out.contains("- plan (plan, required): docs/plans/x.md (step: Task 1: A)"),
        "{out}"
    );
}

#[test]
fn equal_external_links_merge_into_spec_and_an_unstepped_plan_is_not_fetched() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let fixture = common::docs::DocFixture::new(&env, "# Shared");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "A",
            "--spec",
            "stub:p",
            "--plan",
            "stub:p",
            "--ref",
            "shared (context): stub:p",
        ],
    ));
    let count = fixture.calls().len();
    let v = env.json(&dir, &["show", &id]);
    let rows = v["inventory"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["origin"], "spec");
    assert_eq!(rows[0]["handle"], "shared");
    assert_eq!(rows[0]["status"], "unchecked");
    assert_eq!(rows[0]["required"], true);
    assert_eq!(rows[1]["origin"], "plan");
    assert_eq!(rows[1]["status"], "unchecked");
    for key in ["handle", "step", "step_found", "path", "cause"] {
        assert!(rows[1].get(key).is_none(), "{key}: {v}");
    }
    assert_eq!(fixture.calls().len(), count);
}

#[test]
fn pretty_inventory_quotes_references_and_preserves_required_labels() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "A",
            "--source",
            "user:chat",
            "--ref",
            r#"draft (output, required): "docs/a \"quoted\" \\ draft.md" — draft"#,
        ],
    ));
    let out = env.pretty(&dir, &["show", &id]);
    assert!(out.contains("read first: draft"), "{out}");
    assert!(
        out.contains(
            r#"- draft (output, required): "docs/a \"quoted\" \\ draft.md" [pending] — draft"#
        ),
        "{out}"
    );
    assert!(
        out.contains("- source (source): user:chat [unchecked]"),
        "{out}"
    );
}

#[test]
fn missing_attachment_outputs_are_pending_but_missing_inputs_are_missing() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    let id = id_of(env.json(&dir, &["add", "A"]));
    set_refs(
        &env,
        &dir,
        &id,
        "- i (input): docs/gone.md\n- o (output): files:future.png",
    );
    let v = env.json(&dir, &["show", &id]);
    assert_eq!(v["inventory"][0]["status"], "missing");
    assert_eq!(v["inventory"][1]["status"], "pending");
    assert_eq!(
        v["inventory"][1]["path"],
        dir.join(format!("tasks/files/{id}/future.png"))
            .to_str()
            .unwrap()
    );
}
