mod common;
use common::TestEnv;

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
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
