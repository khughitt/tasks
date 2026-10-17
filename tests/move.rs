mod common;
use common::TestEnv;

#[test]
fn external_doc_move_without_step_does_not_fetch() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let docs = common::docs::DocFixture::new(&env, "# Document\n");
    let id = id_of(env.json(
        &source,
        &[
            "add",
            "External",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
        ],
    ));
    let calls = docs.calls();
    docs.disconnect();
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    assert_eq!(docs.calls(), calls);
    let shown = env.json(&destination, &["show", &moved]);
    assert_eq!(shown["task"]["spec"], "stub:spec");
    assert_eq!(shown["task"]["plan"], "stub:canonical");
}

#[test]
fn external_doc_move_step_failure_is_atomic() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let docs = common::docs::DocFixture::new(&env, "### Task 1: one\n");
    let id = id_of(env.json(
        &source,
        &[
            "add",
            "External",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    ));
    let record = source.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    docs.set_body("# no heading\n");
    assert_eq!(
        env.fail(&source, &["move", &id, "--to", "wali"]),
        "validation"
    );
    assert_eq!(std::fs::read(&record).unwrap(), before);
    docs.disconnect();
    assert_eq!(env.fail(&source, &["move", &id, "--to", "wali"]), "config");
    assert_eq!(std::fs::read(record).unwrap(), before);
    assert!(
        env.json(&destination, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn show_moved_record_keeps_missing_historical_plan_and_move_pointers() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let plan = "docs/plans/history.md";
    for dir in [&source, &destination] {
        std::fs::create_dir_all(dir.join("docs/plans")).unwrap();
        std::fs::write(dir.join(plan), "### Task 1: A\n").unwrap();
    }
    let id = id_of(env.json(
        &source,
        &["add", "History", "--plan", "history", "--step", "Task 1: A"],
    ));
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    let before = env.json(&source, &["show", &id]);
    std::fs::remove_file(source.join(plan)).unwrap();

    env.check(&source);
    let shown = env.json(&source, &["show", &id]);
    assert_eq!(shown["task"], before["task"]);
    assert_eq!(shown["task"]["moved_to"], moved);
    assert_eq!(shown["step_found"], false);
    assert!(shown["warnings"].as_array().unwrap().iter().any(|w| {
        let w = w.as_str().unwrap();
        w.contains(plan) && w.contains("unavailable")
    }));
    assert_eq!(
        env.json(&destination, &["show", &moved])["task"]["moved_from"],
        id
    );
    std::fs::remove_file(destination.join(plan)).unwrap();
    assert_eq!(env.fail(&destination, &["show", &moved]), "io");
}

#[test]
fn moved_status_cannot_be_set_by_edit() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let id = id_of(env.json(&dots, &["add", "T", "-p", "2"]));
    assert_eq!(
        env.fail(&dots, &["edit", &id, "--status", "moved"]),
        "validation"
    );
    let listed = env.json(&dots, &["list", "--status", "moved"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn a_flag_edit_keeps_moved_from() {
    let mut env = TestEnv::new();
    let wali = env.init("wali");
    let n = id_of(env.json(&wali, &["add", "T", "-p", "2"]));
    let path = wali.join(format!("tasks/{n}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replacen("tags: []\n", "tags: []\nmoved_from: dots-00000a\n", 1),
    )
    .unwrap();
    env.json(&wali, &["edit", &n, "--title", "U", "-p", "1"]);
    assert_eq!(
        env.json(&wali, &["show", &n])["task"]["moved_from"],
        "dots-00000a"
    );
    let pretty = env.pretty(&wali, &["show", &n]);
    assert!(
        pretty
            .lines()
            .any(|line| line.starts_with("moved from:") && line.ends_with("dots-00000a")),
        "{pretty}"
    );
}

/// Writes the source tombstone before the move command exists.
fn tombstone_by_hand(dir: &std::path::Path, id: &str, to: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen("status: todo", "status: moved", 1).replacen(
        "status: moved\n",
        &format!("status: moved\nmoved_to: {to}\n"),
        1,
    );
    std::fs::write(path, text).unwrap();
}

fn fail_detail(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> (String, String) {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{args:?}: {out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    (
        v["error"]["kind"].as_str().unwrap().to_string(),
        v["error"]["detail"].as_str().unwrap().to_string(),
    )
}

#[test]
fn every_write_to_a_tombstone_refuses_and_names_the_new_id() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let path = dots.join(format!("tasks/{a}.md"));
    let before = std::fs::read_to_string(&path).unwrap();
    for args in [
        vec!["note", a.as_str(), "x"],
        vec!["edit", a.as_str(), "-p", "1"],
        vec!["start", a.as_str()],
        vec!["park", a.as_str(), "next"],
        vec!["shelve", a.as_str(), "later"],
        vec!["drop", a.as_str(), "why"],
        vec!["done", a.as_str()],
        vec!["block", a.as_str(), "why"],
        vec!["unblock", a.as_str()],
        vec!["unshelve", a.as_str()],
        vec!["dep", a.as_str(), "--on", n.as_str()],
        vec!["attach", a.as_str(), "missing.png"],
        vec!["detach", a.as_str(), "missing.png", "why"],
    ] {
        let (kind, detail) = fail_detail(&env, &dots, &args);
        assert_eq!(kind, "moved", "{args:?}");
        let mut redirected = args.clone();
        redirected[1] = &n;
        assert!(
            detail.contains(&format!("tasks {}", redirected.join(" "))),
            "{detail}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }
    let out = env
        .cmd(&dots)
        .env("EDITOR", "true")
        .args(["edit", &a])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "moved", "{out:?}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    let shown = env.json(&dots, &["show", &a]);
    assert_eq!(shown["task"]["status"], "moved");
    assert_eq!(shown["task"]["moved_to"], n);
    let pretty = env.pretty(&dots, &["show", &a]);
    assert!(
        pretty
            .lines()
            .any(|line| line.starts_with("moved to:") && line.ends_with(&n)),
        "{pretty}"
    );
}

#[test]
fn new_references_to_a_tombstone_refuse() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    for (args, role) in [
        (vec!["dep", &b, "--on", &a], "dependency"),
        (vec!["edit", &b, "--depends", &a], "dependency"),
        (vec!["edit", &b, "--parent", &a], "parent"),
        (vec!["add", "C", "--depends", &a], "dependency"),
        (vec!["add", "C", "--parent", &a], "parent"),
    ] {
        let (kind, detail) = fail_detail(&env, &dots, &args);
        assert_eq!(kind, "moved", "{args:?}");
        assert_eq!(detail, format!("{role} {a} moved to {n}; name {n} instead"));
    }
    assert!(
        env.json(&dots, &["show", &b])["task"]
            .get("depends")
            .is_none()
    );
}

#[test]
fn editor_refuses_a_new_tombstone_dependency_but_saves_an_existing_one() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2"]));
    let c = id_of(env.json(&dots, &["add", "C", "-p", "2", "--depends", &a]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let script = dots.join("editor.sh");
    std::fs::write(
        &script,
        format!("sed -i 's/depends: \\[\\]/depends: [{a}]/' \"$1\"\n"),
    )
    .unwrap();
    let out = env
        .cmd(&dots)
        .env("EDITOR", format!("sh {}", script.display()))
        .args(["edit", &b])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "moved", "{out:?}");
    let detail = v["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!("dependency {a} moved to {n}")),
        "{detail}"
    );
    let kept = detail
        .split(" (edit kept at ")
        .nth(1)
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert!(
        std::fs::read_to_string(kept)
            .unwrap()
            .contains(&format!("depends: [{a}]"))
    );
    assert!(
        env.json(&dots, &["show", &b])["task"]
            .get("depends")
            .is_none()
    );
    std::fs::write(&script, "sed -i 's/title: C/title: Edited/' \"$1\"\n").unwrap();
    let out = env
        .cmd(&dots)
        .env("EDITOR", format!("sh {}", script.display()))
        .args(["edit", &c])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let shown = env.json(&dots, &["show", &c]);
    assert_eq!(shown["task"]["title"], "Edited");
    assert_eq!(shown["task"]["depends"], serde_json::json!([a]));
}

#[test]
fn tombstone_retry_replaces_bare_and_alias_spellings() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"]
        .as_table_mut()
        .unwrap()
        .insert("old".into(), toml::Value::String("dots".into()));
    std::fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    for spelling in [
        a.split_once('-').unwrap().1.to_string(),
        a.replacen("dots-", "old-", 1),
    ] {
        let (kind, detail) = fail_detail(&env, &dots, &["note", &spelling, &spelling]);
        assert_eq!(kind, "moved");
        assert!(detail.contains(&format!("tasks note {n} {n}")), "{detail}");
    }
}

#[test]
fn feedback_recurrence_to_a_tombstone_refuses_before_mutating() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let config = dots.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        config,
        format!("{text}\n[feedback]\nscope = \"shared tooling\"\n"),
    )
    .unwrap();
    let a = id_of(env.json(&dots, &["add", "Report", "-p", "2", "--tag", "feedback"]));
    let n = id_of(env.json(&wali, &["add", "Report", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let path = dots.join(format!("tasks/{a}.md"));
    let before = std::fs::read_to_string(&path).unwrap();
    let (kind, detail) = fail_detail(
        &env,
        &dots,
        &[
            "feedback",
            "--project",
            "dots",
            "again",
            "--category",
            "gap",
            "--recur",
            &a,
        ],
    );
    assert_eq!(kind, "moved", "{detail}");
    assert!(detail.contains(&n), "{detail}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), before);
}

/// Parse the displayed command in a real shell, then run those words with TestEnv's isolation.
fn retry_words(dir: &std::path::Path, detail: &str, number: usize) -> Vec<String> {
    let line = detail.split('`').nth(2 * number + 1).unwrap();
    let words = line.strip_prefix("tasks ").unwrap();
    let out = std::process::Command::new("sh")
        .current_dir(dir)
        .arg("-c")
        .arg(format!("set -- {words}; printf '%s\\0' \"$@\""))
        .output()
        .unwrap();
    assert!(out.status.success(), "{line}: {out:?}");
    String::from_utf8(out.stdout)
        .unwrap()
        .split_terminator('\0')
        .map(str::to_string)
        .collect()
}

fn feedback_owner(dir: &std::path::Path) {
    let path = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        format!("{text}\n[feedback]\nscope = \"shared tooling\"\n"),
    )
    .unwrap();
}

#[test]
fn tombstone_retry_preserves_multiword_arguments() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let (kind, detail) = fail_detail(&env, &dots, &["note", &a, "two words"]);
    assert_eq!(kind, "moved");
    let words = retry_words(&dots, &detail, 0);
    assert_eq!(words, ["note", &n, "two words"]);
    let args: Vec<_> = words.iter().map(String::as_str).collect();
    env.json(&dots, &args);
    assert_eq!(
        env.json(&wali, &["show", &n])["task"]["notes"][0]["text"],
        "two words"
    );
}

#[test]
fn tombstone_retry_keeps_quotes_and_shell_metacharacters_literal() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    for text in ["it's quoted", "two \"quoted\" words; $(touch injected)"] {
        let (kind, detail) = fail_detail(&env, &dots, &["note", &a, text]);
        assert_eq!(kind, "moved");
        let words = retry_words(&dots, &detail, 0);
        assert_eq!(words, ["note", &n, text]);
        assert!(
            !dots.join("injected").exists(),
            "payload must remain literal"
        );
        let args: Vec<_> = words.iter().map(String::as_str).collect();
        env.json(&dots, &args);
        let shown = env.json(&wali, &["show", &n]);
        assert_eq!(
            shown["task"]["notes"].as_array().unwrap().last().unwrap()["text"],
            text
        );
    }
}

#[test]
fn feedback_retry_retargets_the_owner_for_plain_alias_and_equals_selectors() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    feedback_owner(&dots);
    feedback_owner(&wali);
    let a = id_of(env.json(&dots, &["add", "Report", "-p", "2", "--tag", "feedback"]));
    let n = id_of(env.json(&wali, &["add", "Report", "-p", "2", "--tag", "feedback"]));
    tombstone_by_hand(&dots, &a, &n);
    let source_path = dots.join(format!("tasks/{a}.md"));
    let before = std::fs::read_to_string(&source_path).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"]
        .as_table_mut()
        .unwrap()
        .insert("old".into(), toml::Value::String("dots".into()));
    std::fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let alias = a.replacen("dots-", "old-", 1);
    let recur_equals = format!("--recur={alias}");
    for args in [
        vec![
            "feedback",
            "--project",
            "dots",
            "it's still broken",
            "--category",
            "gap",
            "--recur",
            &a,
        ],
        vec![
            "feedback",
            "--project",
            "old",
            "it's still broken",
            "--category",
            "gap",
            "--recur",
            &alias,
        ],
        vec![
            "feedback",
            "--project=dots",
            "it's still broken",
            "--category=gap",
            &recur_equals,
        ],
        vec![
            "feedback",
            "--project=old",
            "it's still broken",
            "--category=gap",
            &recur_equals,
        ],
    ] {
        let (kind, detail) = fail_detail(&env, &dots, &args);
        assert_eq!(kind, "moved");
        let words = retry_words(&dots, &detail, 0);
        let retry: Vec<_> = words.iter().map(String::as_str).collect();
        let result = env.json(&dots, &retry);
        assert_eq!(result["id"], n);
        assert_eq!(result["action"], "recurred");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), before);
    }
    let destination = env.json(&wali, &["show", &n]);
    let notes = destination["task"]["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 4);
    assert!(
        notes.iter().all(|note| note["by"] == "feedback"
            && note["text"] == "feedback from dots: it's still broken")
    );
}

#[test]
fn feedback_retry_offers_an_ordinary_note_when_destination_is_not_an_owner() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    feedback_owner(&dots);
    let a = id_of(env.json(&dots, &["add", "Report", "-p", "2", "--tag", "feedback"]));
    let n = id_of(env.json(&wali, &["add", "Report", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let summary = "it's a report; $(touch injected)";
    let (kind, detail) = fail_detail(
        &env,
        &dots,
        &[
            "feedback",
            "--project",
            "dots",
            summary,
            "--category",
            "gap",
            "--recur",
            &a,
        ],
    );
    assert_eq!(kind, "moved");
    assert!(detail.contains("does not accept feedback"), "{detail}");
    let words = retry_words(&dots, &detail, 0);
    let retry: Vec<_> = words.iter().map(String::as_str).collect();
    assert_eq!(
        fail_detail(&env, &dots, &retry).0,
        "validation",
        "fixed-owner refusal remains"
    );
    let words = retry_words(&dots, &detail, 1);
    assert_eq!(words, ["note", &n, "--", summary]);
    assert!(!dots.join("injected").exists());
    let retry: Vec<_> = words.iter().map(String::as_str).collect();
    env.json(&dots, &retry);
    assert_eq!(
        env.json(&wali, &["show", &n])["task"]["notes"][0]["text"],
        summary
    );
}

#[test]
fn ordinary_tombstone_retry_keeps_feedback_and_flag_words_as_payload() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    for text in [
        "feedback",
        "--recur",
        "--project=dots",
        "feedback --recur --project=dots",
    ] {
        let (kind, detail) = fail_detail(&env, &dots, &["note", &a, "--", text]);
        assert_eq!(kind, "moved");
        let words = retry_words(&dots, &detail, 0);
        assert_eq!(words, ["note", &n, "--", text]);
        assert!(!detail.contains("does not accept feedback"), "{detail}");
    }
}

#[test]
fn cycle_through_a_tombstone_is_refused_and_reported() {
    // Spec §6 case 8.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2", "--depends", &a]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let (kind, detail) = fail_detail(&env, &wali, &["dep", &n, "--on", &b]);
    assert_eq!(kind, "cycle");
    assert_eq!(
        detail,
        format!("dependency cycle: {n} -> {b} -> {a} -> {n}")
    );
    let path = wali.join(format!("tasks/{n}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replacen("depends: []", &format!("depends: [{b}]"), 1),
    )
    .unwrap();
    let out = env.cmd(&wali).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "cycle" && f["detail"].as_str().unwrap().contains(&a)),
        "{report}"
    );
}

#[test]
fn local_dependent_follows_the_pointer() {
    // Review Focus 2: the dependent and the tombstone share a project.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2", "--depends", &a]));
    let n = id_of(env.json(&wali, &["add", "A", "-p", "2"]));
    tombstone_by_hand(&dots, &a, &n);
    let ready = |env: &TestEnv| -> Vec<String> {
        env.json(&dots, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(
        !ready(&env).contains(&b),
        "a moved dependency is not a closed one"
    );
    assert_eq!(env.fail(&dots, &["done", &b]), "open_dependencies");
    env.json(&wali, &["done", &n]);
    assert!(ready(&env).contains(&b));
}

/// A completed move written by hand: `id` in `from` becomes a tombstone, and a copy
/// carrying `moved_from` is created in `to_dir`. Returns the new id.
fn moved_pair_by_hand(
    env: &TestEnv,
    from: &std::path::Path,
    id: &str,
    to_dir: &std::path::Path,
) -> String {
    let n = id_of(env.json(to_dir, &["add", "copy", "-p", "2"]));
    let path = to_dir.join(format!("tasks/{n}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replacen("tags: []\n", &format!("tags: []\nmoved_from: {id}\n"), 1),
    )
    .unwrap();
    tombstone_by_hand(from, id, &n);
    n
}

fn kinds(report: &serde_json::Value, side: &str) -> Vec<String> {
    report[side]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn check_warns_moved_dep_with_the_swap() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let ops = env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let x = id_of(env.json(&ops, &["add", "X", "-p", "2", "--depends", &a]));
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    let report = env.check(&ops);
    let finding = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "moved_dep")
        .expect("moved_dep");
    assert_eq!(finding["id"], x);
    assert!(
        finding["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("tasks dep {x} --on {n} --rm {a}"))
    );
    assert!(kinds(&env.check(&dots), "errors").is_empty());
    assert!(kinds(&env.check(&wali), "errors").is_empty());
}

#[test]
fn check_skips_moved_dep_on_a_tombstone_dependent() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2", "--depends", &a]));
    let c = id_of(env.json(&dots, &["add", "C", "-p", "2", "--depends", &a]));
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    env.json(&dots, &["move", &b, "--to", "wali"]);
    // b's tombstone keeps its frozen depends on a's tombstone, which is history (spec:
    // Dependency evaluation), and refuses the swap; only the live c can act on it.
    let report = env.check(&dots);
    let moved: Vec<&serde_json::Value> = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["kind"] == "moved_dep")
        .collect();
    assert_eq!(moved.len(), 1, "{report}");
    assert_eq!(moved[0]["id"], c);
    assert!(
        moved[0]["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("tasks dep {c} --on {n} --rm {a}"))
    );
}

#[test]
fn half_a_move_is_incomplete_but_an_unreachable_counterpart_only_warns() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2", "--depends", &a]));
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    // Unregistering the destination: a warning, never move_incomplete (spec §4.4, case 7).
    env.json(&dots, &["unregister", "wali"]);
    let ready = env.json(&dots, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"] != b),
        "{ready}"
    );
    assert!(
        ready["warnings"].to_string().contains("unreachable"),
        "{ready}"
    );
    let report = env.check(&dots);
    assert!(
        kinds(&report, "warnings").contains(&"move_counterpart_unreachable".into()),
        "{report}"
    );
    assert!(
        !kinds(&report, "errors").contains(&"move_incomplete".into()),
        "{report}"
    );
    env.json(&wali, &["init", "--prefix", "wali", "--force"]);
    assert!(
        env.check(&dots)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["kind"] != "move_counterpart_unreachable")
    );
    // A reachable destination whose record is missing: move_incomplete.
    std::fs::remove_file(wali.join(format!("tasks/{n}.md"))).unwrap();
    let out = env.cmd(&dots).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        kinds(&report, "errors").contains(&"move_incomplete".into()),
        "{report}"
    );
}

#[test]
fn chained_moves_follow_to_the_end() {
    // Review Focus 3: the middle record is both a tombstone and a destination.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let ops = env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2", "--depends", &a]));
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    let m = moved_pair_by_hand(&env, &wali, &n, &ops);
    for dir in [&dots, &wali, &ops] {
        assert!(
            kinds(&env.check(dir), "errors").is_empty(),
            "{}",
            dir.display()
        );
    }
    // The swap names the end of the chain, and running it works: `dep --on` refuses
    // the middle tombstone, so naming it would be a repair that fails.
    let report = env.check(&dots);
    let detail = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "moved_dep")
        .expect("moved_dep")["detail"]
        .as_str()
        .unwrap()
        .to_string();
    let swap = format!("tasks dep {b} --on {m} --rm {a}");
    assert!(detail.contains(&swap), "{detail}");
    assert!(!detail.contains(&format!("--on {n}")), "{detail}");
    env.json(&dots, &["dep", &b, "--on", &m, "--rm", &a]);
    assert!(
        kinds(&env.check(&dots), "warnings")
            .iter()
            .all(|k| k != "moved_dep")
    );
    assert_eq!(env.fail(&dots, &["done", &b]), "open_dependencies");
    env.json(&ops, &["done", &m]);
    env.json(&dots, &["done", &b]);
}

#[test]
fn renaming_either_project_after_a_move_keeps_the_pair_whole() {
    // A rename rewrites only its own ids, so `moved_from: dots-…` outlives `dots`.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let ops = env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let x = id_of(env.json(&ops, &["add", "X", "-p", "2", "--depends", &a]));
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    let clean = |env: &TestEnv| {
        for dir in [&dots, &wali] {
            let report = env.check(dir);
            assert!(kinds(&report, "errors").is_empty(), "{report}");
            assert!(
                !kinds(&report, "warnings").contains(&"move_counterpart_unreachable".into()),
                "{report}"
            );
        }
    };
    env.json(&dots, &["rename", "dots", "dx"]);
    clean(&env);
    env.json(&wali, &["rename", "wali", "wx"]);
    clean(&env);
    let renamed = n.replacen("wali-", "wx-", 1);
    let report = env.check(&ops);
    let moved = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "moved_dep")
        .expect("moved_dep");
    assert!(
        moved["detail"]
            .as_str()
            .unwrap()
            .contains(&format!("--on {renamed}")),
        "{report}"
    );
    assert_eq!(env.fail(&ops, &["done", &x]), "open_dependencies");
}

#[test]
fn check_warns_moved_dep_on_closed_and_shelved_records() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let closed = id_of(env.json(&dots, &["add", "Closed", "-p", "2", "--depends", &a]));
    let shelved = id_of(env.json(&dots, &["add", "Shelved", "-p", "2", "--depends", &a]));
    env.json(&dots, &["drop", &closed, "no longer needed"]);
    env.json(&dots, &["shelve", &shelved, "later"]);
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    let report = env.check(&dots);
    for id in [closed, shelved] {
        let finding = report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["kind"] == "moved_dep" && f["id"] == id)
            .unwrap_or_else(|| panic!("missing moved_dep for {id}: {report}"));
        assert!(
            finding["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("tasks dep {id} --on {n} --rm {a}"))
        );
    }
}

#[test]
fn check_checks_both_move_counterparts_without_following_past_them() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = moved_pair_by_hand(&env, &dots, &a, &wali);
    let source_path = dots.join(format!("tasks/{a}.md"));
    let destination_path = wali.join(format!("tasks/{n}.md"));
    let source = std::fs::read_to_string(&source_path).unwrap();
    let destination = std::fs::read_to_string(&destination_path).unwrap();
    for (path, original, pointer, replacement, check_dir) in [
        (
            &destination_path,
            &destination,
            a.as_str(),
            "dots-000000",
            &dots,
        ),
        (&source_path, &source, n.as_str(), "wali-000000", &wali),
    ] {
        std::fs::write(path, original.replace(pointer, replacement)).unwrap();
        let out = env.cmd(check_dir).args(["check"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            kinds(&report, "errors").contains(&"move_incomplete".into()),
            "{report}"
        );
        std::fs::write(path, original).unwrap();
        std::fs::write(path, "unparsable").unwrap();
        let report = env.check(check_dir);
        assert!(
            kinds(&report, "warnings").contains(&"move_counterpart_unreachable".into()),
            "{report}"
        );
        assert!(kinds(&report, "errors").is_empty(), "{report}");
        std::fs::remove_file(path).unwrap();
        let out = env.cmd(check_dir).args(["check"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            kinds(&report, "errors").contains(&"move_incomplete".into()),
            "{report}"
        );
        std::fs::write(path, original).unwrap();
    }
    env.json(&wali, &["unregister", "dots"]);
    let report = env.check(&wali);
    assert!(
        kinds(&report, "warnings").contains(&"move_counterpart_unreachable".into()),
        "{report}"
    );
    assert!(kinds(&report, "errors").is_empty(), "{report}");
}

fn accept_feedback(dir: &std::path::Path, scope: &str) {
    let path = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!("\n[feedback]\nscope = \"{scope}\"\n"));
    std::fs::write(path, text).unwrap();
}

fn journal_path(env: &TestEnv, id: &str) -> std::path::PathBuf {
    env.home
        .path()
        .join(format!(".local/state/tasks/move/{id}.toml"))
}

/// A journal as `move` writes one after M2, naming `source` in `from` and `target` in
/// `to`, carrying coherent records without publishing them.
fn journal_by_hand(
    env: &TestEnv,
    from: &std::path::Path,
    source: &str,
    to: &std::path::Path,
    target: &str,
) {
    use sha2::{Digest, Sha256};
    let source_text = std::fs::read_to_string(from.join(format!("tasks/{source}.md"))).unwrap();
    let moved_text = source_text
        .lines()
        .map(|line| {
            if line.starts_with("updated:") {
                "updated: 2099-01-01T00:00:00Z"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let note = |mut text: String, message: &str| {
        if !text.contains("\n## Notes\n") {
            text.push_str("\n## Notes\n\n");
        }
        text.push_str(&format!("- 2099-01-01T00:00:00Z (owner): {message}\n"));
        text
    };
    let target_text = note(
        moved_text.replacen(
            &format!("id: {source}\n"),
            &format!("id: {target}\nmoved_from: {source}\n"),
            1,
        ),
        &format!("moved from {source}"),
    );
    let original_status = moved_text
        .lines()
        .find(|line| line.starts_with("status:"))
        .unwrap()
        .to_string()
        + "\n";
    let tombstone_text = note(
        moved_text.replacen(
            &original_status,
            &format!("status: moved\nmoved_to: {target}\n"),
            1,
        ),
        &format!("moved to {target}"),
    );
    let mut journal = toml::Table::new();
    for (key, value) in [
        ("source", source.to_string()),
        ("target", target.to_string()),
        ("source_root", from.display().to_string()),
        ("target_root", to.display().to_string()),
        (
            "source_digest",
            format!("{:x}", Sha256::digest(source_text.as_bytes())),
        ),
        ("source_text", source_text),
        ("target_text", target_text),
        ("tombstone_text", tombstone_text),
    ] {
        journal.insert(key.into(), toml::Value::String(value));
    }
    let path = journal_path(env, source);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, toml::to_string(&journal).unwrap()).unwrap();
}

#[test]
fn a_pending_journal_freezes_both_ids_in_every_writer() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    accept_feedback(&dots, "*");
    let a = id_of(env.json(
        &wali,
        &[
            "feedback",
            "--project",
            "dots",
            "move freeze probe",
            "--category",
            "gap",
        ],
    ));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    journal_by_hand(&env, &dots, &a, &wali, &n);
    assert_eq!(
        fail_detail(&env, &dots, &["note", &a, "x"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &wali, &["note", &n, "x"]).0,
        "move_pending"
    );
    let out = env
        .cmd(&dots)
        .env("EDITOR", "true")
        .args(["edit", &a])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "move_pending", "{out:?}");
    let (kind, _) = fail_detail(
        &env,
        &wali,
        &[
            "feedback",
            "--project",
            "dots",
            "move freeze probe",
            "--category",
            "gap",
            "--recur",
            &a,
        ],
    );
    assert_eq!(kind, "move_pending");
    let warnings = env.json(&dots, &["show", &a])["warnings"].to_string();
    assert!(warnings.contains("pending"), "{warnings}");
    let unrelated = id_of(env.json(&dots, &["add", "Unrelated", "-p", "2"]));
    env.json(&dots, &["note", &unrelated, "allowed"]);
    let unrelated_target = id_of(env.json(&wali, &["add", "Unrelated", "-p", "2"]));
    env.json(&wali, &["note", &unrelated_target, "allowed"]);
    // Pending wins over the tombstone: the same refusal whichever state the move reached.
    tombstone_by_hand(&dots, &a, &n);
    assert_eq!(
        fail_detail(&env, &dots, &["note", &a, "x"]).0,
        "move_pending"
    );
}

#[test]
fn a_pending_journal_freezes_both_projects_lifecycle() {
    // Spec §6 case 9, with the journal written by hand.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    journal_by_hand(&env, &dots, &a, &wali, &n);
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let before = std::fs::read(&registry).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(
        fail_detail(&env, &dots, &["retire", "--reason", "done"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &wali, &["retire", "--reason", "done"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["unregister", "wali"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["unregister", "dots"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(
            &env,
            elsewhere.path(),
            &["init", "--prefix", "wali", "--force"]
        )
        .0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["rename", "dots", "dx"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["rename", "wali", "wx"]).0,
        "move_pending"
    );
    assert_eq!(fail_detail(&env, &dots, &["unretire"]).0, "move_pending");
    assert_eq!(fail_detail(&env, &wali, &["unretire"]).0, "move_pending");
    // The new side of a rename is reserved too, even when the old project is unrelated.
    assert_eq!(std::fs::read(&registry).unwrap(), before);
    let ops = env.init("ops");
    let with_ops = std::fs::read(&registry).unwrap();
    assert_eq!(
        fail_detail(&env, &ops, &["rename", "ops", "dots"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &ops, &["rename", "ops", "wali"]).0,
        "move_pending"
    );
    for (dir, old, new) in [(&dots, "dots", "dx"), (&ops, "ops", "wali")] {
        let out = env.json(dir, &["rename", old, new, "--explain"]);
        assert_eq!(out["recovery"], "refuse");
        assert!(out["warnings"].to_string().contains("pending"));
    }
    // Initializing ops is the only registry mutation in this fixture.
    assert_ne!(with_ops, before);
    assert_eq!(std::fs::read(&registry).unwrap(), with_ops);
    // Retire refuses any open task once the freeze lifts; close A first.
    std::fs::remove_file(journal_path(&env, &a)).unwrap();
    env.json(&dots, &["done", &a]);
    env.json(&dots, &["retire", "--reason", "done"]);
}

#[test]
fn a_pending_journal_stands_in_for_move_incomplete() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    journal_by_hand(&env, &dots, &a, &wali, &n);
    tombstone_by_hand(&dots, &a, &n); // M6 done; `n` lacks moved_from, as before M4
    for dir in [&dots, &wali] {
        let report = env.check(dir);
        assert!(
            kinds(&report, "warnings").contains(&"move_pending".into()),
            "{report}"
        );
        assert!(kinds(&report, "errors").is_empty(), "{report}");
    }
    // The same records on a host without the journal: the error stands.
    std::fs::remove_file(journal_path(&env, &a)).unwrap();
    let out = env.cmd(&dots).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("move_incomplete"));
}

#[test]
fn a_pending_journal_freezes_both_adopt_prefixes() {
    for frozen_old in [true, false] {
        let mut env = TestEnv::new();
        let dots = env.init("dots");
        let wali = env.init("wali");
        let ops = env.init("ops");
        let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
        let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
        let old = if frozen_old { "dots" } else { "ops" };
        let new = if frozen_old { "ops" } else { "dots" };
        let adopted = if frozen_old { &ops } else { &dots };
        // Adoption is reached from the synced checkout already carrying the new prefix.
        journal_by_hand(&env, &dots, &a, &wali, &n);
        let registry = env.home.path().join(".config/tasks/projects.toml");
        let before = std::fs::read(&registry).unwrap();
        assert_eq!(
            fail_detail(&env, adopted, &["rename", old, new, "--adopt"]).0,
            "move_pending"
        );
        let out = env.json(adopted, &["rename", old, new, "--adopt", "--explain"]);
        assert_eq!(out["recovery"], "refuse");
        assert!(out["warnings"].to_string().contains("pending"));
        assert_eq!(std::fs::read(&registry).unwrap(), before);
    }
}

#[test]
fn a_pending_journal_does_not_hide_a_different_incomplete_pair() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2"]));
    let m = id_of(env.json(&wali, &["add", "M", "-p", "2"]));
    journal_by_hand(&env, &dots, &a, &wali, &n);
    tombstone_by_hand(&dots, &a, &n);
    tombstone_by_hand(&dots, &b, &m);
    let out = env.cmd(&dots).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        kinds(&report, "warnings").contains(&"move_pending".into()),
        "{report}"
    );
    let incomplete: Vec<_> = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["kind"] == "move_incomplete")
        .collect();
    assert_eq!(incomplete.len(), 1, "{report}");
    assert_eq!(incomplete[0]["id"], b);
    // Sharing only one id is not the journal's pair either.
    let path = dots.join(format!("tasks/{a}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        text.replace(&format!("moved_to: {n}"), &format!("moved_to: {m}")),
    )
    .unwrap();
    let out = env.cmd(&dots).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["kind"] == "move_incomplete")
            .count(),
        2,
        "{report}"
    );
}

#[test]
fn a_pending_journal_freezes_retired_projects_before_unretire() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    journal_by_hand(&env, &dots, &a, &wali, &n);
    let saved = std::fs::read(journal_path(&env, &a)).unwrap();
    std::fs::remove_file(journal_path(&env, &a)).unwrap();
    env.json(&dots, &["done", &a]);
    env.json(&wali, &["done", &n]);
    for dir in [&dots, &wali] {
        env.json(dir, &["retire", "--reason", "done"]);
    }
    std::fs::write(journal_path(&env, &a), saved).unwrap();
    for dir in [&dots, &wali] {
        let path = dir.join("tasks/.config.toml");
        let before = std::fs::read(&path).unwrap();
        assert_eq!(fail_detail(&env, dir, &["unretire"]).0, "move_pending");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    std::fs::remove_file(journal_path(&env, &a)).unwrap();
    for dir in [&dots, &wali] {
        env.json(dir, &["unretire"]);
        assert!(
            !std::fs::read_to_string(dir.join("tasks/.config.toml"))
                .unwrap()
                .contains("[retired]")
        );
    }
}

#[test]
fn a_pending_journal_freezes_alias_id_and_lifecycle_spellings() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let old = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&wali, &["add", "N", "-p", "2"]));
    env.json(&dots, &["rename", "dots", "dx"]);
    let a = old.replacen("dots-", "dx-", 1);
    journal_by_hand(&env, &dots, &a, &wali, &n);
    assert_eq!(
        fail_detail(&env, &dots, &["note", &old, "x"]).0,
        "move_pending"
    );
    assert!(
        env.json(&dots, &["show", &old])["warnings"]
            .to_string()
            .contains("pending")
    );
    assert_eq!(
        fail_detail(&env, &dots, &["unregister", "dots"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["rename", "dots", "ops", "--adopt"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["rename", "ops", "dots", "--adopt"]).0,
        "move_pending"
    );
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        fail_detail(&env, other.path(), &["init", "--prefix", "dots", "--force"]).0,
        "move_pending"
    );
}

fn store(env: &TestEnv, prefix: &str) -> toml::Value {
    std::fs::read_to_string(env.claim_store(prefix))
        .map(|text| toml::from_str(&text).unwrap())
        .unwrap_or(toml::Value::Table(Default::default()))
}

#[test]
fn moving_a_task_with_an_inbound_dependency_and_an_attachment() {
    // Spec §6 case 1.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let ops = env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2", "-b", "why"]));
    env.json(&dots, &["note", &a, "first"]);
    let shot = dots.join("scratch/shot.png");
    std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
    std::fs::write(&shot, b"png bytes").unwrap();
    env.json(&dots, &["attach", &a, shot.to_str().unwrap()]);
    let x = id_of(env.json(&ops, &["add", "X", "-p", "2", "--depends", &a]));
    let before = env.json(&dots, &["show", &a])["task"].clone();
    let ops_before = std::fs::read(ops.join(format!("tasks/{x}.md"))).unwrap();

    let out = env.json(&dots, &["move", &a, "--to", "wali"]);
    let n = out["id"].as_str().unwrap().to_string();
    assert!(n.starts_with("wali-"));
    assert_eq!(out["from"], a);
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(out["attachments"], serde_json::json!(["shot.png"]));
    assert_eq!(out["inbound"], serde_json::json!([x]));

    let moved = env.json(&wali, &["show", &n])["task"].clone();
    assert_eq!(moved["moved_from"], a);
    assert_eq!(moved["created"], before["created"]);
    assert_eq!(moved["body"], before["body"]);
    let carried = &moved["notes"].as_array().unwrap()[..before["notes"].as_array().unwrap().len()];
    assert_eq!(
        carried,
        before["notes"].as_array().unwrap().as_slice(),
        "notes carry verbatim"
    );
    assert_eq!(
        moved["notes"].as_array().unwrap().last().unwrap()["text"],
        format!("moved from {a}")
    );
    assert_eq!(
        std::fs::read(wali.join(format!("tasks/files/{n}/shot.png"))).unwrap(),
        b"png bytes"
    );

    let tomb = env.json(&dots, &["show", &a])["task"].clone();
    assert_eq!(tomb["status"], "moved");
    assert_eq!(tomb["moved_to"], n);
    let texts: Vec<&str> = tomb["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["text"].as_str().unwrap())
        .collect();
    assert!(
        texts.contains(&format!("detached: shot.png: moved to {n}").as_str()),
        "{texts:?}"
    );
    assert!(!dots.join(format!("tasks/files/{a}/shot.png")).exists());

    assert_eq!(
        std::fs::read(ops.join(format!("tasks/{x}.md"))).unwrap(),
        ops_before
    );
    let ready: Vec<String> = env.json(&ops, &["ready"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!ready.contains(&x));
    assert!(
        env.check(&ops)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "moved_dep")
    );
    assert!(env.check(&dots)["errors"].as_array().unwrap().is_empty());
    assert!(env.check(&wali)["errors"].as_array().unwrap().is_empty());
    env.json(&wali, &["done", &n]);
    let ready: Vec<String> = env.json(&ops, &["ready"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ready.contains(&x));

    let again = env.json(&dots, &["move", &a, "--to", "wali"]);
    assert_eq!(again["recovery"], "complete");
    assert_eq!(again["id"], n);
}

#[test]
fn parented_and_parent_tasks_refuse_before_any_write() {
    // Spec §6 case 2.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let g = id_of(env.json(&dots, &["add", "G", "-p", "2"]));
    let c = id_of(env.json(&dots, &["add", "C", "-p", "2", "--parent", &g]));
    let (kind, detail) = fail_detail(&env, &dots, &["move", &c, "--to", "wali"]);
    assert_eq!(kind, "validation");
    assert!(
        detail.contains(&g) && detail.contains(&format!("tasks edit {c} --no-parent")),
        "{detail}"
    );
    let (kind, detail) = fail_detail(&env, &dots, &["move", &g, "--to", "wali"]);
    assert_eq!(kind, "validation");
    assert!(detail.contains(&c), "{detail}");
    assert!(!env.home.path().join(".local/state/tasks/move").exists());
    env.json(&dots, &["edit", &c, "--no-parent"]);
    env.json(&dots, &["move", &c, "--to", "wali"]);
}

#[test]
fn live_claims_refuse_and_a_park_travels_to_the_destination_checkout() {
    // Spec §6 case 3.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.cmd(&dots)
        .env("CODEX_SESSION_ID", "other")
        .env("CODEX_THREAD_ID", "other")
        .args(["start", &a])
        .assert()
        .success();
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
        "claimed"
    );
    env.cmd(&dots)
        .env("CODEX_SESSION_ID", "other")
        .env("CODEX_THREAD_ID", "other")
        .args(["park", &a, "resume the parser"])
        .assert()
        .success();
    let out = env.json(&dots, &["move", &a, "--to", "wali"]);
    let n = out["id"].as_str().unwrap().to_string();
    assert_eq!(out["park"], true);
    let parks = store(&env, "wali");
    let park = &parks["parks"][n.as_str()];
    assert_eq!(park["next_step"].as_str(), Some("resume the parser"));
    assert_eq!(park["worktree"].as_str(), Some(wali.to_str().unwrap()));
    assert!(
        store(&env, "dots")
            .get("parks")
            .and_then(|p| p.get(&a))
            .is_none()
    );
    let parked = env.json(&wali, &["list", "--parked"]);
    assert!(parked.to_string().contains(&n));
    env.json(&wali, &["start", &n]);
}

#[test]
fn the_callers_own_claim_refuses_naming_park() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_SESSION", "me")
        .args(["start", &a])
        .assert()
        .success();
    let out = env
        .cmd(&dots)
        .env("TASKS_SESSION", "me")
        .args(["move", &a, "--to", "wali"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "claimed");
    assert!(
        v["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("tasks park")
    );
}

fn unverifiable(env: &TestEnv, prefix: &str, id: &str) {
    let path = env.claim_store(prefix);
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    store["claims"][id]
        .as_table_mut()
        .unwrap()
        .insert("pid_view".into(), toml::Value::String("0:1".into()));
    std::fs::write(&path, toml::to_string(&store).unwrap()).unwrap();
}

#[test]
fn a_foreign_claim_refuses_move_with_its_sessions_remedy() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let live = id_of(env.json(&dots, &["add", "Live", "-p", "2"]));
    let gone = id_of(env.json(&dots, &["add", "Gone", "-p", "2"]));
    for id in [&live, &gone] {
        env.cmd(&dots)
            .env("TASKS_SESSION", "other")
            .env("TASKS_SESSION_PID", std::process::id().to_string())
            .args(["start", id])
            .assert()
            .success();
    }
    unverifiable(&env, "dots", &gone);

    let (kind, detail) = fail_detail(&env, &dots, &["move", &live, "--to", "wali"]);
    assert_eq!(kind, "claimed");
    assert!(
        detail.contains("that session must park it first"),
        "{detail}"
    );
    assert!(!detail.contains("tasks park"), "{detail}");

    let (kind, detail) = fail_detail(&env, &dots, &["move", &gone, "--to", "wali"]);
    assert_eq!(kind, "claimed");
    assert!(
        detail.contains(&format!("tasks release {gone} \"<why>\"")),
        "{detail}"
    );
    assert!(!detail.contains("tasks park"), "{detail}");
}

#[test]
fn an_earlier_refile_in_the_destination_refuses() {
    // Spec §6 case 4.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let r = id_of(env.json(&wali, &["add", "A", "-p", "2", "--source", &a]));
    let (kind, detail) = fail_detail(&env, &dots, &["move", &a, "--to", "wali"]);
    assert_eq!(kind, "validation");
    assert!(detail.contains(&r), "{detail}");
    assert_eq!(env.json(&dots, &["show", &a])["task"]["status"], "todo");
    assert_eq!(
        std::fs::read_dir(wali.join("tasks"))
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "md"))
            .count(),
        1
    );
}

#[test]
fn closed_unportable_and_misdirected_moves_refuse() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let done = id_of(env.json(&dots, &["add", "D", "-p", "2"]));
    env.json(&dots, &["done", &done]);
    assert_eq!(
        fail_detail(&env, &dots, &["move", &done, "--to", "wali"]).0,
        "validation"
    );
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "dots"]).0,
        "config"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "nope"]).0,
        "config"
    );
    // A need the destination does not declare: settled by editing the source first.
    let config = dots.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[needs.quiet]\nmeaning = \"an idle host\"\n");
    std::fs::write(&config, text).unwrap();
    let q = id_of(env.json(&dots, &["add", "Q", "-p", "2", "--need", "quiet"]));
    assert_eq!(
        fail_detail(&env, &dots, &["move", &q, "--to", "wali"]).0,
        "unknown_need"
    );
    // A retired destination.
    let gone = env.init("gone");
    env.json(&gone, &["retire", "--reason", "over"]);
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "gone"]).0,
        "retired_project"
    );
    assert!(
        !env.home.path().join(".local/state/tasks/move").exists(),
        "no refusal writes a journal"
    );
}

/// A retired-prefix alias, as `tests/cli.rs::alias_registry` writes one.
fn alias_registry(env: &TestEnv, alias: &str, target: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    registry["aliases"]
        .as_table_mut()
        .unwrap()
        .insert(alias.into(), toml::Value::String(target.into()));
    std::fs::write(&path, toml::to_string(&registry).unwrap()).unwrap();
}

#[test]
fn move_accepts_alias_and_suffix_spellings() {
    // Review Focus 1.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    alias_registry(&env, "old", "dots");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&dots, &["add", "B", "-p", "2"]));
    let hex = a.trim_start_matches("dots-").to_string();
    let n = id_of(env.json(&dots, &["move", &hex, "--to", "wali"]));
    let (kind, detail) = fail_detail(&env, &dots, &["note", &hex, "x"]);
    assert_eq!(kind, "moved");
    assert!(detail.contains(&format!("tasks note {n} x")), "{detail}");
    let old_b = b.replacen("dots-", "old-", 1);
    let m = env.json(&dots, &["move", &old_b, "--to", "wali"]);
    assert_eq!(
        m["from"], b,
        "the journal and output use the canonical source"
    );
    let (kind, detail) = fail_detail(&env, &dots, &["note", &old_b, "x"]);
    assert_eq!(kind, "moved");
    assert!(detail.contains(m["id"].as_str().unwrap()), "{detail}");
}

#[test]
fn moving_back_mints_a_fresh_id() {
    // Review Focus 4.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    let back = id_of(env.json(&wali, &["move", &n, "--to", "dots"]));
    assert!(back.starts_with("dots-") && back != a);
    assert_eq!(env.json(&dots, &["show", &back])["task"]["moved_from"], n);
}

/// Run git in `dir` under a fixed identity, asserting success (as `tests/cli.rs::git`).
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

#[test]
fn move_from_a_source_worktree_refuses() {
    // Review Focus 5. Foreign readers and `check` read the registered checkout, so a
    // tombstone written only in a worktree would not be seen until integration.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    git(&dots, &["init", "-q", "-b", "main"]);
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);
    let side = dots.join("wt");
    git(
        &dots,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let (kind, detail) = fail_detail(&env, &side, &["move", &a, "--to", "wali"]);
    assert_eq!(kind, "validation");
    assert!(
        detail.contains(&format!("tasks -C {}", dots.display())),
        "{detail}"
    );
    assert!(!env.home.path().join(".local/state/tasks/move").exists());
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali"])["recovery"],
        "fresh"
    );
}

#[test]
fn an_unsafe_attachment_entry_refuses_before_any_write() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let shot = dots.join("scratch/shot.png");
    std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
    std::fs::write(&shot, b"png bytes").unwrap();
    env.json(&dots, &["attach", &a, shot.to_str().unwrap()]);
    let stored = dots.join(format!("tasks/files/{a}/shot.png"));
    std::fs::remove_file(&stored).unwrap();
    std::os::unix::fs::symlink(&shot, &stored).unwrap();
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
        "attachment_unsafe"
    );
    assert!(!env.home.path().join(".local/state/tasks/move").exists());
    assert!(!wali.join("tasks/files").exists());
}

#[test]
fn a_journal_naming_an_outside_path_is_never_acted_on() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let shot = dots.join("scratch/shot.png");
    std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
    std::fs::write(&shot, b"png bytes").unwrap();
    env.json(&dots, &["attach", &a, shot.to_str().unwrap()]);
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "attachments")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let sentinel = env.home.path().join("keep.txt");
    std::fs::write(&sentinel, b"keep").unwrap();
    let path = journal_path(&env, &a);
    let mut journal: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    journal["attachments"][0]["name"] = toml::Value::String(sentinel.display().to_string());
    std::fs::write(&path, toml::to_string(&journal).unwrap()).unwrap();
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
        "config"
    );
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
}

#[test]
fn an_interrupted_move_resumes_with_the_same_id() {
    // Spec §6 case 5.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "destination")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    assert!(journal_path(&env, &a).is_file());
    let n = std::fs::read_dir(wali.join("tasks"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .find(|f| f.ends_with(".md"))
        .unwrap()
        .trim_end_matches(".md")
        .to_string();
    assert_eq!(env.json(&dots, &["show", &a])["task"]["status"], "todo");
    assert_eq!(
        fail_detail(&env, &dots, &["note", &a, "x"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &wali, &["note", &n, "x"]).0,
        "move_pending"
    );
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "resume"
    );
    let checked = env.check(&dots);
    assert!(
        checked["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "move_pending")
    );
    assert_eq!(
        fail_detail(&env, &wali, &["retire", "--reason", "x"]).0,
        "move_pending"
    );
    // The same records with scratch state: no journal on "this host".
    let saved = std::fs::read(journal_path(&env, &a)).unwrap();
    std::fs::remove_file(journal_path(&env, &a)).unwrap();
    let out = env.cmd(&wali).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("move_incomplete"));
    std::fs::write(journal_path(&env, &a), saved).unwrap();

    let out = env.json(&dots, &["move", &a, "--to", "wali"]);
    assert_eq!(out["recovery"], "resumed");
    assert_eq!(out["id"], n);
    assert!(!journal_path(&env, &a).exists());
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali"])["recovery"],
        "complete"
    );
}

#[test]
fn a_hand_edit_mid_move_makes_the_rerun_refuse() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let path = dots.join(format!("tasks/{a}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen("title: A", "title: B", 1)).unwrap();
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
        "move_pending"
    );
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "refuse"
    );
}

#[test]
fn undo_from_the_journal_restores_the_original_park() {
    // Spec §6 case 10: the §4.4 manual undo, performed by the test from the journal.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.json(&dots, &["park", &a, "pick up the parser"]);
    let code_worktree = dots.join(".worktrees/x").display().to_string();
    let store_path = env.claim_store("dots");
    let text = std::fs::read_to_string(&store_path).unwrap();
    let mut parks: toml::Value = toml::from_str(&text).unwrap();
    parks["parks"][a.as_str()]["worktree"] = toml::Value::String(code_worktree.clone());
    std::fs::write(&store_path, toml::to_string(&parks).unwrap()).unwrap();
    let original = parks["parks"][a.as_str()].clone();

    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "store")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let journal: toml::Value =
        toml::from_str(&std::fs::read_to_string(journal_path(&env, &a)).unwrap()).unwrap();
    assert_eq!(
        journal["source_park"]["worktree"].as_str(),
        Some(code_worktree.as_str())
    );
    assert_eq!(
        journal["target_park"]["worktree"].as_str(),
        Some(wali.to_str().unwrap())
    );
    let n = journal["target"].as_str().unwrap().to_string();
    assert_eq!(
        store(&env, "wali")["parks"][n.as_str()],
        journal["target_park"]
    );

    // Undo, restore first and delete last.
    std::fs::write(
        dots.join(format!("tasks/{a}.md")),
        journal["source_text"].as_str().unwrap(),
    )
    .unwrap();
    let mut source_store = store(&env, "dots");
    source_store["parks"]
        .as_table_mut()
        .unwrap()
        .insert(a.clone(), journal["source_park"].clone());
    std::fs::write(&store_path, toml::to_string(&source_store).unwrap()).unwrap();
    assert_eq!(store(&env, "dots")["parks"][a.as_str()], original);
    let mut target_store = store(&env, "wali");
    target_store["parks"].as_table_mut().unwrap().remove(&n);
    std::fs::write(
        env.claim_store("wali"),
        toml::to_string(&target_store).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(wali.join(format!("tasks/{n}.md"))).unwrap();
    std::fs::remove_file(journal_path(&env, &a)).unwrap();

    assert_eq!(env.json(&dots, &["show", &a])["task"]["status"], "todo");
    assert!(env.check(&dots)["errors"].as_array().unwrap().is_empty());
    assert!(env.check(&wali)["errors"].as_array().unwrap().is_empty());
}

#[test]
fn lost_attachment_bytes_block_recovery_until_restored() {
    // Spec §6 case 11.
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let shot = dots.join("scratch/shot.png");
    std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
    std::fs::write(&shot, b"png bytes").unwrap();
    env.json(&dots, &["attach", &a, shot.to_str().unwrap()]);
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "cleanup")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let journal: toml::Value =
        toml::from_str(&std::fs::read_to_string(journal_path(&env, &a)).unwrap()).unwrap();
    let n = journal["target"].as_str().unwrap().to_string();
    let copy = wali.join(format!("tasks/files/{n}/shot.png"));
    std::fs::remove_file(&copy).unwrap();
    let (kind, detail) = fail_detail(&env, &dots, &["move", &a, "--to", "wali"]);
    assert_eq!(kind, "move_pending");
    assert!(detail.contains("shot.png"), "{detail}");
    let explained = env.json(&dots, &["move", &a, "--to", "wali", "--explain"]);
    assert_eq!(explained["recovery"], "refuse");
    assert!(explained["warnings"].to_string().contains("shot.png"));
    assert!(journal_path(&env, &a).is_file());
    assert_eq!(
        fail_detail(&env, &wali, &["note", &n, "x"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &dots, &["retire", "--reason", "x"]).0,
        "move_pending"
    );
    std::fs::write(&copy, b"png bytes").unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali"])["recovery"],
        "resumed"
    );
}

#[test]
fn destination_documents_and_step_must_validate_before_journaling() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    for root in [&dots, &wali] {
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::create_dir_all(root.join("docs/plans")).unwrap();
    }
    std::fs::write(dots.join("docs/specs/design.md"), "# Design\n").unwrap();
    std::fs::write(
        dots.join("docs/plans/plan.md"),
        "# Plan\n### Task 1: Work\n",
    )
    .unwrap();
    let a = id_of(env.json(
        &dots,
        &[
            "add",
            "A",
            "-p",
            "2",
            "--spec",
            "docs/specs/design.md",
            "--plan",
            "docs/plans/plan.md",
            "--step",
            "Task 1: Work",
        ],
    ));
    let before = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
    for remedy in ["--no-spec", "--no-plan", "--no-plan"] {
        let (kind, detail) = fail_detail(&env, &dots, &["move", &a, "--to", "wali"]);
        assert_eq!(kind, "validation");
        assert!(
            detail.contains(&format!("tasks edit {a}")) && detail.contains(remedy),
            "{detail}"
        );
        assert!(!journal_path(&env, &a).exists());
        assert_eq!(
            std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
            before
        );
        if remedy == "--no-spec" {
            std::fs::write(wali.join("docs/specs/design.md"), "# Design\n").unwrap();
        } else {
            std::fs::write(wali.join("docs/plans/plan.md"), "# Plan\n").unwrap();
        }
    }
    std::fs::write(wali.join("docs/plans/plan.md"), "### Task 1: Work\n").unwrap();
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    let moved = env.json(&wali, &["show", &n]);
    assert_eq!(moved["task"]["spec"], "docs/specs/design.md");
    assert_eq!(moved["task"]["plan"], "docs/plans/plan.md");
    assert_eq!(moved["task"]["step"], "Task 1: Work");
}

#[test]
fn overlapping_project_operations_refuse_fresh_and_resume() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let ops = env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&ops, &["add", "B", "-p", "2"]));
    for (from, target) in [(&dots, "wali-111111"), (&ops, "wali-111111")] {
        let other = id_of(env.json(from, &["add", "other", "-p", "2"]));
        journal_by_hand(&env, from, &other, &wali, target);
        assert_eq!(
            fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
            "move_pending"
        );
        assert!(!journal_path(&env, &a).exists());
        std::fs::remove_file(journal_path(&env, &other)).unwrap();
    }
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let own = std::fs::read(journal_path(&env, &a)).unwrap();
    journal_by_hand(&env, &ops, &b, &wali, "wali-111111");
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "wali"]).0,
        "move_pending"
    );
    assert_eq!(std::fs::read(journal_path(&env, &a)).unwrap(), own);
    std::fs::remove_file(journal_path(&env, &b)).unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali"])["recovery"],
        "resumed"
    );
}

#[test]
fn completion_requires_the_requested_immediate_destination() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    env.init("ops");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    env.json(&wali, &["move", &n, "--to", "ops"]);
    assert_eq!(
        fail_detail(&env, &dots, &["move", &a, "--to", "ops"]).0,
        "moved"
    );
    alias_registry(&env, "alias", "wali");
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "alias"])["recovery"],
        "complete"
    );
}

#[test]
fn destination_doc_roots_refuse_with_an_actionable_remedy() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    std::fs::write(dots.join("docs/specs/design.md"), "# Design\n").unwrap();
    let a = id_of(env.json(
        &dots,
        &["add", "A", "-p", "2", "--spec", "docs/specs/design.md"],
    ));
    let config = wali.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\nspec_dirs = [\"other/specs\"]\n");
    std::fs::write(config, text).unwrap();
    let (kind, detail) = fail_detail(&env, &dots, &["move", &a, "--to", "wali"]);
    assert_eq!(kind, "validation");
    assert!(
        detail.contains(&format!("tasks edit {a} --no-spec")),
        "{detail}"
    );
    assert!(!journal_path(&env, &a).exists());
}

#[test]
fn every_move_phase_resumes_and_explain_preserves_records() {
    for phase in [
        "journal",
        "attachments",
        "destination",
        "store",
        "tombstone",
        "cleanup",
    ] {
        let mut env = TestEnv::new();
        let dots = env.init("dots");
        let wali = env.init("wali");
        let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
        env.json(&dots, &["park", &a, "continue"]);
        std::fs::write(dots.join("image.png"), b"bytes").unwrap();
        env.json(&dots, &["attach", &a, "image.png"]);
        let before = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
        let preview = env.json(&dots, &["move", &a, "--to", "wali", "--explain"]);
        assert_eq!(preview["recovery"], "fresh");
        assert!(!journal_path(&env, &a).exists());
        assert_eq!(
            std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
            before
        );
        env.cmd(&dots)
            .env("TASKS_MOVE_STOP_AFTER", phase)
            .args(["move", &a, "--to", "wali"])
            .assert()
            .success();
        let saved = std::fs::read(journal_path(&env, &a)).unwrap();
        assert_eq!(
            env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
            "resume",
            "{phase}"
        );
        assert_eq!(std::fs::read(journal_path(&env, &a)).unwrap(), saved);
        let out = env.json(&dots, &["move", &a, "--to", "wali"]);
        assert_eq!(out["recovery"], "resumed", "{phase}");
        let n = out["id"].as_str().unwrap();
        assert_eq!(
            std::fs::read(wali.join(format!("tasks/files/{n}/image.png"))).unwrap(),
            b"bytes"
        );
        assert!(!journal_path(&env, &a).exists());
        assert!(!dots.join(format!("tasks/files/{a}")).exists());
        assert!(env.check(&dots)["errors"].as_array().unwrap().is_empty());
        assert!(env.check(&wali)["errors"].as_array().unwrap().is_empty());
    }
}

#[test]
fn pending_renames_on_either_move_project_refuse() {
    for side in ["dots", "wali"] {
        let mut env = TestEnv::new();
        let dots = env.init("dots");
        let wali = env.init("wali");
        let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
        let root = if side == "dots" { &dots } else { &wali };
        env.cmd(root)
            .env("TASKS_RENAME_STOP_AFTER", "inventory")
            .args(["rename", side, "changed"])
            .assert()
            .success();
        let (kind, detail) = fail_detail(&env, &dots, &["move", &a, "--to", "wali"]);
        assert_eq!(kind, "validation");
        assert!(
            detail.contains("rename") && detail.contains("unfinished"),
            "{detail}"
        );
        assert!(!journal_path(&env, &a).exists());
    }
}

#[test]
fn move_recovery_ignores_an_orphaned_publication_temp() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let journal: toml::Value =
        toml::from_str(&std::fs::read_to_string(journal_path(&env, &a)).unwrap()).unwrap();
    let n = journal["target"].as_str().unwrap();
    let orphan = wali.join(format!("tasks/.{n}.new.tmp"));
    std::fs::write(&orphan, b"partial publication").unwrap();
    let out = env.json(&dots, &["move", &a, "--to", "wali"]);
    assert_eq!(out["recovery"], "resumed");
    assert_eq!(out["id"], n);
    assert_eq!(
        std::fs::read(&orphan).unwrap(),
        b"partial publication",
        "unverified orphan bytes are retained"
    );
    assert_eq!(env.json(&wali, &["show", n])["task"]["moved_from"], a);
    assert!(!journal_path(&env, &a).exists());
}

#[test]
fn move_reporting_errors_refuse_before_mutation() {
    for malformed in ["record", "config"] {
        let mut env = TestEnv::new();
        let dots = env.init("dots");
        let wali = env.init("wali");
        let ops = env.init("ops");
        let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
        env.json(&dots, &["park", &a, "continue"]);
        let raw = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
        let parked = std::fs::read(env.claim_store("dots")).unwrap();
        if malformed == "record" {
            std::fs::write(ops.join("tasks/ops-123456.md"), "invalid record").unwrap();
        } else {
            std::fs::write(ops.join("tasks/.config.toml"), "invalid config").unwrap();
        }
        let explain = env.json(&dots, &["move", &a, "--to", "wali", "--explain"]);
        assert_eq!(explain["recovery"], "refuse");
        let out = env
            .cmd(&dots)
            .args(["move", &a, "--to", "wali"])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(!journal_path(&env, &a).exists());
        assert_eq!(
            std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
            raw,
            "{malformed}"
        );
        assert_eq!(std::fs::read(env.claim_store("dots")).unwrap(), parked);
        assert!(
            !std::fs::read_dir(wali.join("tasks")).unwrap().any(|e| e
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "md"))
        );
    }
}

#[test]
fn move_explain_classifies_opening_and_journal_refusals() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    alias_registry(&env, "old", "dots");
    let alias = a.replacen("dots-", "old-", 1);
    let raw = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let registration = std::fs::read(&registry).unwrap();
    std::fs::rename(&wali, wali.with_extension("offline")).unwrap();
    for (to, fragment) in [
        ("dots", "both"),
        ("missing", "registered"),
        ("wali", "unreachable"),
    ] {
        let out = env.json(&dots, &["move", &alias, "--to", to, "--explain"]);
        assert_eq!(out["recovery"], "refuse");
        assert_eq!(out["from"], a);
        assert_eq!(out["id"], "");
        assert_eq!(out["paths"], serde_json::json!({}));
        assert!(out["warnings"].to_string().contains(fragment), "{out}");
        assert_eq!(
            fail_detail(&env, &dots, &["move", &alias, "--to", to]).0,
            "config"
        );
    }
    std::fs::rename(wali.with_extension("offline"), &wali).unwrap();
    assert!(!journal_path(&env, &a).exists());
    let path = journal_path(&env, &a);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "invalid journal").unwrap();
    let out = env.json(&dots, &["move", &alias, "--to", "wali", "--explain"]);
    assert_eq!(out["recovery"], "refuse");
    assert_eq!(out["from"], a);
    assert_eq!(out["id"], "");
    assert!(!out["warnings"].as_array().unwrap().is_empty());
    assert_eq!(
        fail_detail(&env, &dots, &["move", &alias, "--to", "wali"]).0,
        "config"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid journal");
    assert_eq!(std::fs::read(&registry).unwrap(), registration);
    assert_eq!(
        std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
        raw
    );
}

#[test]
fn move_explain_classifies_source_worktree_refusal() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    git(&dots, &["init", "-q", "-b", "main"]);
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);
    let side = dots.join("wt");
    git(
        &dots,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let raw = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
    let out = env.json(&side, &["move", &a, "--to", "wali", "--explain"]);
    assert_eq!(out["recovery"], "refuse");
    assert_eq!(out["from"], a);
    assert_eq!(out["id"], "");
    assert!(out["warnings"].to_string().contains("registered checkout"));
    assert!(!journal_path(&env, &a).exists());
    assert_eq!(
        std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
        raw
    );
    assert_eq!(
        std::fs::read(side.join(format!("tasks/{a}.md"))).unwrap(),
        raw
    );
}

#[test]
fn correction_i1_move_stamp_keeps_identical_sibling_stale() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    git(&dots, &["init", "-q", "-b", "main"]);
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let path = dots.join(format!("tasks/{a}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text
        .lines()
        .map(|line| {
            if line.starts_with("updated:") {
                "updated: 2099-01-01T00:00:00Z"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&path, text).unwrap();
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);
    let side = dots.join("wt");
    git(
        &dots,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    let moved = env.json(&dots, &["show", &a]);
    assert!(moved["task"]["updated"].as_str().unwrap() > "2099-01-01T00:00:00Z");
    assert_eq!(
        moved["task"]["updated"],
        env.json(&wali, &["show", &n])["task"]["updated"]
    );
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali"])["recovery"],
        "complete"
    );
    assert_eq!(env.fail(&side, &["note", &a, "stale write"]), "stale_copy");
}

#[test]
fn correction_i2_deferred_move_checks_cleanly_and_stays_immutable() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2", "--defer", "7d"]));
    let date = env.json(&dots, &["show", &a])["task"]["defer"].clone();
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    assert_eq!(env.json(&wali, &["show", &n])["task"]["defer"], date);
    assert_eq!(env.json(&dots, &["show", &a])["task"]["defer"], date);
    for root in [&dots, &wali] {
        assert!(env.check(root)["errors"].as_array().unwrap().is_empty());
    }
    assert_eq!(env.fail(&dots, &["edit", &a, "--no-defer"]), "moved");
}

fn pending_parent_refuses(phase: &str, reparent: bool) {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let child = id_of(env.json(&dots, &["add", "Child", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", phase)
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let args = if reparent {
        vec!["edit", &child, "--parent", &a]
    } else {
        vec!["add", "New child", "--parent", &a]
    };
    assert_eq!(env.fail(&dots, &args), "move_pending");
}

#[test]
fn correction_i3_add_before_destination() {
    pending_parent_refuses("journal", false);
}
#[test]
fn correction_i3_reparent_before_destination() {
    pending_parent_refuses("journal", true);
}
#[test]
fn correction_i3_add_after_destination() {
    pending_parent_refuses("destination", false);
}
#[test]
fn correction_i3_reparent_after_destination() {
    pending_parent_refuses("destination", true);
}

#[test]
fn correction_i3_external_child_refuses_resume_without_mutation() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let child = id_of(env.json(&dots, &["add", "Child", "-p", "2"]));
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let path = dots.join(format!("tasks/{child}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        text.replacen("tags:", &format!("parent: {a}\ntags:"), 1),
    )
    .unwrap();
    let before = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "refuse"
    );
    assert_eq!(env.fail(&dots, &["move", &a, "--to", "wali"]), "validation");
    assert_eq!(
        std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
        before
    );
    let j: toml::Value =
        toml::from_str(&std::fs::read_to_string(journal_path(&env, &a)).unwrap()).unwrap();
    assert!(
        !wali
            .join(format!("tasks/{}.md", j["target"].as_str().unwrap()))
            .exists()
    );
}

fn changed_attachment_refuses(phase: &str, changed_source: bool) {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let input = env.home.path().join("shot.png");
    std::fs::write(&input, b"original").unwrap();
    env.json(&dots, &["attach", &a, input.to_str().unwrap()]);
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", phase)
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let jp = journal_path(&env, &a);
    let before = std::fs::read_to_string(&jp).unwrap();
    let j: toml::Value = toml::from_str(&before).unwrap();
    let source = dots.join(format!("tasks/files/{a}/shot.png"));
    let target = wali.join(format!(
        "tasks/files/{}/shot.png",
        j["target"].as_str().unwrap()
    ));
    let (changed, kept) = if changed_source {
        (&source, &target)
    } else {
        (&target, &source)
    };
    std::fs::write(changed, b"unique new bytes").unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "refuse"
    );
    assert_eq!(
        env.fail(&dots, &["move", &a, "--to", "wali"]),
        "move_pending"
    );
    assert_eq!(std::fs::read(changed).unwrap(), b"unique new bytes");
    assert_eq!(std::fs::read(kept).unwrap(), b"original");
    assert_eq!(std::fs::read_to_string(jp).unwrap(), before);
}

#[test]
fn correction_i4_source_attachment_before_publish() {
    changed_attachment_refuses("attachments", true);
}

#[test]
fn correction_i4_source_attachment_after_tombstone() {
    changed_attachment_refuses("tombstone", true);
}

#[test]
fn correction_i4_destination_attachment() {
    changed_attachment_refuses("attachments", false);
}

fn changed_store_refuses(field: &str) {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.json(&dots, &["park", &a, "continue"]);
    let sp = env.claim_store("dots");
    let mut s = store(&env, "dots");
    s.as_table_mut().unwrap().insert(
        "escalations".into(),
        toml::Value::Table(toml::Table::from_iter([(
            a.clone(),
            toml::Value::Table(toml::Table::from_iter([
                ("level".into(), toml::Value::String("high".into())),
                (
                    "at".into(),
                    toml::Value::String("2026-10-09T00:00:00Z".into()),
                ),
                ("session".into(), toml::Value::String("codex:test".into())),
            ])),
        )])),
    );
    std::fs::write(&sp, toml::to_string(&s).unwrap()).unwrap();
    env.cmd(&dots)
        .env("TASKS_MOVE_STOP_AFTER", "tombstone")
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let mut changed = store(&env, "dots");
    let key = if field == "parks" {
        "next_step"
    } else {
        "session"
    };
    changed[field][a.as_str()][key] = toml::Value::String("unique changed value".into());
    let bytes = toml::to_string(&changed).unwrap();
    std::fs::write(&sp, &bytes).unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "refuse"
    );
    assert_eq!(
        env.fail(&dots, &["move", &a, "--to", "wali"]),
        "move_pending"
    );
    assert_eq!(std::fs::read_to_string(sp).unwrap(), bytes);
    assert!(journal_path(&env, &a).exists());
}

#[test]
fn correction_i4_source_park() {
    changed_store_refuses("parks");
}

#[test]
fn correction_i4_source_escalation() {
    changed_store_refuses("escalations");
}

fn incoherent_journal_refuses(change: &str) {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    let input = env.home.path().join("shot.png");
    std::fs::write(&input, b"retained source data").unwrap();
    env.json(&dots, &["attach", &a, input.to_str().unwrap()]);
    env.json(&dots, &["park", &a, "continue"]);
    let sp = env.claim_store("dots");
    let mut s = store(&env, "dots");
    s.as_table_mut().unwrap().insert(
        "escalations".into(),
        toml::Value::Table(toml::Table::from_iter([(
            a.clone(),
            toml::Value::Table(toml::Table::from_iter([
                ("level".into(), toml::Value::String("high".into())),
                (
                    "at".into(),
                    toml::Value::String("2026-10-09T00:00:00Z".into()),
                ),
                ("session".into(), toml::Value::String("codex:test".into())),
            ])),
        )])),
    );
    std::fs::write(&sp, toml::to_string(&s).unwrap()).unwrap();
    env.cmd(&dots)
        .env(
            "TASKS_MOVE_STOP_AFTER",
            if change == "tombstone" {
                "destination"
            } else {
                "journal"
            },
        )
        .args(["move", &a, "--to", "wali"])
        .assert()
        .success();
    let jp = journal_path(&env, &a);
    let mut j: toml::Value = toml::from_str(&std::fs::read_to_string(&jp).unwrap()).unwrap();
    let n = j["target"].as_str().unwrap().to_string();
    let target_path = wali.join(format!("tasks/{n}.md"));
    let target_before = std::fs::read(&target_path).ok();
    let target_attachment = wali.join(format!("tasks/files/{n}/shot.png"));
    let attachment_before = std::fs::read(&target_attachment).ok();
    match change {
        "missing_attachment" => {
            j.as_table_mut().unwrap().remove("attachments");
        }
        "missing_park" => {
            j.as_table_mut().unwrap().remove("target_park");
        }
        "missing_escalation" => {
            j.as_table_mut().unwrap().remove("target_escalation");
        }
        "tombstone" => j["tombstone_text"] = j["source_text"].clone(),
        "target_escalation" => {
            j["target_escalation"]["session"] = toml::Value::String("changed".into())
        }
        "target_back_pointer" => {
            j["target_text"] = toml::Value::String(
                j["target_text"]
                    .as_str()
                    .unwrap()
                    .replace(&format!("moved_from: {a}"), "moved_from: dots-ffffff"),
            )
        }
        "tombstone_forward_pointer" => {
            j["tombstone_text"] = toml::Value::String(
                j["tombstone_text"]
                    .as_str()
                    .unwrap()
                    .replace(&format!("moved_to: {n}"), "moved_to: wali-ffffff"),
            )
        }
        "target_body" => {
            j["target_text"] = toml::Value::String(
                j["target_text"]
                    .as_str()
                    .unwrap()
                    .replace("\n## Notes", "\nChanged body\n\n## Notes"),
            )
        }
        "tombstone_history" => {
            j["tombstone_text"] = toml::Value::String(
                j["tombstone_text"]
                    .as_str()
                    .unwrap()
                    .replace("continue", "lost original note"),
            )
        }
        "target_status" => {
            j["target_text"] = toml::Value::String(
                j["target_text"]
                    .as_str()
                    .unwrap()
                    .replace("status: todo", "status: blocked"),
            )
        }
        "target_title" => {
            j["target_text"] = toml::Value::String(
                j["target_text"]
                    .as_str()
                    .unwrap()
                    .replace("title: A", "title: Changed"),
            )
        }
        "target_note" => {
            j["target_text"] = toml::Value::String(
                j["target_text"]
                    .as_str()
                    .unwrap()
                    .replace("continue", "lost original note"),
            )
        }
        "target_worktree" => {
            j["target_park"]["worktree"] = toml::Value::String(dots.display().to_string())
        }
        "attachment_ledger" => {
            j.as_table_mut().unwrap().insert(
                "attachments".into(),
                toml::Value::Array(vec![toml::Value::Table(toml::Table::from_iter([
                    ("name".into(), toml::Value::String("invented.png".into())),
                    ("sha256".into(), toml::Value::String("a".repeat(64))),
                ]))]),
            );
        }
        _ => unreachable!(),
    }
    let bytes = toml::to_string(&j).unwrap();
    std::fs::write(&jp, &bytes).unwrap();
    let before = std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap();
    let store_before = std::fs::read(&sp).unwrap();
    assert_eq!(
        env.json(&dots, &["move", &a, "--to", "wali", "--explain"])["recovery"],
        "refuse",
        "{change}"
    );
    let out = env
        .cmd(&dots)
        .args(["move", &a, "--to", "wali"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "{change}: {out:?}");
    assert_eq!(
        std::fs::read(dots.join(format!("tasks/{a}.md"))).unwrap(),
        before
    );
    assert_eq!(std::fs::read(sp).unwrap(), store_before);
    assert_eq!(std::fs::read(target_path).ok(), target_before);
    assert_eq!(
        std::fs::read(dots.join(format!("tasks/files/{a}/shot.png"))).unwrap(),
        b"retained source data"
    );
    assert_eq!(std::fs::read(target_attachment).ok(), attachment_before);
    assert_eq!(std::fs::read_to_string(jp).unwrap(), bytes);
}

#[test]
fn correction_i5_missing_park() {
    incoherent_journal_refuses("missing_park");
}

#[test]
fn correction_i5_missing_escalation() {
    incoherent_journal_refuses("missing_escalation");
}

#[test]
fn correction_i5_tombstone() {
    incoherent_journal_refuses("tombstone");
}

#[test]
fn correction_i5_target_title() {
    incoherent_journal_refuses("target_title");
}

#[test]
fn correction_i5_target_note() {
    incoherent_journal_refuses("target_note");
}

#[test]
fn correction_i5_target_worktree() {
    incoherent_journal_refuses("target_worktree");
}

#[test]
fn correction_i5_attachment_ledger() {
    incoherent_journal_refuses("attachment_ledger");
}

#[test]
fn correction_i5_target_back_pointer() {
    incoherent_journal_refuses("target_back_pointer");
}

#[test]
fn correction_i5_tombstone_forward_pointer() {
    incoherent_journal_refuses("tombstone_forward_pointer");
}

#[test]
fn correction_i5_target_body() {
    incoherent_journal_refuses("target_body");
}

#[test]
fn correction_i5_tombstone_history() {
    incoherent_journal_refuses("tombstone_history");
}

#[test]
fn correction_i5_target_escalation() {
    incoherent_journal_refuses("target_escalation");
}

#[test]
fn correction_i5_target_status() {
    incoherent_journal_refuses("target_status");
}

#[test]
fn correction_i6_halt_follows_move_back_and_live_dependencies() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let old_dep = id_of(env.json(&dots, &["add", "Old dependency", "-p", "2"]));
    let new_dep = id_of(env.json(&dots, &["add", "New dependency", "-p", "2"]));
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2", "--depends", &old_dep]));
    env.json(
        &dots,
        &["add", "Halt", "-p", "0", "--tag", "halt", "--depends", &a],
    );
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    env.json(&wali, &["dep", &n, "--rm", &old_dep]);
    let back = id_of(env.json(&wali, &["move", &n, "--to", "dots"]));
    let ready = env.json(&dots, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == back),
        "{ready}"
    );
    env.json(&dots, &["start", &back]);
    env.json(&dots, &["park", &back, "continue"]);
    env.json(&dots, &["dep", &back, "--on", &new_dep]);
    assert_eq!(env.fail(&dots, &["start", &old_dep]), "halted");
    env.json(&dots, &["start", &new_dep]);
}

#[test]
fn correction_i6_foreign_project_keeps_its_own_halt_authority() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.json(
        &dots,
        &["add", "Halt", "-p", "0", "--tag", "halt", "--depends", &a],
    );
    env.json(
        &wali,
        &["add", "Independent halt", "-p", "0", "--tag", "halt"],
    );
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    assert_eq!(env.fail(&wali, &["start", &n]), "halted");
}

#[test]
fn correction_i6_halt_pointer_cycle_does_not_authorize_historical_dependencies() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let dep = id_of(env.json(&dots, &["add", "Dependency", "-p", "2"]));
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2", "--depends", &dep]));
    env.json(
        &dots,
        &["add", "Halt", "-p", "0", "--tag", "halt", "--depends", &a],
    );
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    tombstone_by_hand(&wali, &n, &a);
    assert_eq!(env.fail(&dots, &["start", &dep]), "halted");
}

#[test]
fn correction_i6_halt_canonicalizes_old_prefixes_along_pointer_chain() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&dots, &["add", "A", "-p", "2"]));
    env.json(
        &dots,
        &["add", "Halt", "-p", "0", "--tag", "halt", "--depends", &a],
    );
    let n = id_of(env.json(&dots, &["move", &a, "--to", "wali"]));
    let back = id_of(env.json(&wali, &["move", &n, "--to", "dots"]));
    env.json(&wali, &["rename", "wali", "other"]);
    env.json(&dots, &["start", &back]);
}

#[test]
fn correction_i5_missing_attachment() {
    incoherent_journal_refuses("missing_attachment");
}

#[test]
fn correction_i6_foreign_dependency_moved_local_is_ready_and_startable() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let wali = env.init("wali");
    let a = id_of(env.json(&wali, &["add", "Remedy", "-p", "2"]));
    env.json(
        &dots,
        &["add", "Halt", "-p", "0", "--tag", "halt", "--depends", &a],
    );
    let local = id_of(env.json(&wali, &["move", &a, "--to", "dots"]));
    let ready = env.json(&dots, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"] == local),
        "{ready}"
    );
    env.json(&dots, &["start", &local]);
}

#[test]
fn move_clears_review_stamps_on_the_tombstone_and_keeps_them_on_the_target() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
    let path = source.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap().replacen(
        "\npriority: ",
        "\nreviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\nawaiting_user: true\npriority: ",
        1,
    );
    std::fs::write(&path, text).unwrap();
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    let tombstone = env.read(&source, &format!("tasks/{id}.md"));
    for key in ["reviewed", "reviewed_at", "awaiting_user", "closed"] {
        assert!(
            !tombstone.contains(&format!("\n{key}:")),
            "{key} on tombstone: {tombstone}"
        );
    }
    let target = env.json(&destination, &["show", &moved])["task"].clone();
    assert_eq!(target["reviewed"], true);
    assert_eq!(target["reviewed_at"], "2026-10-17T09:00:00Z");
    assert_eq!(target["awaiting_user"], true);
}
