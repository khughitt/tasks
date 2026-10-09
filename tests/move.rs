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
