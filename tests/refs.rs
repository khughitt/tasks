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
