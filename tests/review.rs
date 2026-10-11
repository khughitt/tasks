mod common;
use common::TestEnv;
use std::path::Path;

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

/// Writes `reviewed: true` into a record by hand: reaches states the CLI writes from
/// Task 2 on, or (on a non-idea) never writes at all.
fn seed_reviewed(dir: &Path, id: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let seeded = text.replacen("\npriority: ", "\nreviewed: true\npriority: ", 1);
    assert_ne!(seeded, text, "no priority line in {id}");
    std::fs::write(&path, seeded).unwrap();
}

fn reviewed(env: &TestEnv, dir: &Path, id: &str) -> bool {
    env.json(dir, &["show", id])["task"]["reviewed"]
        .as_bool()
        .unwrap()
}

fn editor_script(dir: &Path, body: &str) -> String {
    let p = dir.join("editor.sh");
    std::fs::write(&p, format!("{body}\n")).unwrap();
    format!("sh '{}'", p.display())
}

#[test]
fn every_status_transition_clears_reviewed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [
        vec!["edit", "ID", "--status", "todo"],
        vec!["shelve", "ID", "later"],
        vec!["drop", "ID"],
    ] {
        let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
        seed_reviewed(&dir, &id);
        assert!(reviewed(&env, &dir, &id));
        let args: Vec<&str> = args
            .iter()
            .map(|a| if *a == "ID" { id.as_str() } else { a })
            .collect();
        env.json(&dir, &args);
        assert!(!reviewed(&env, &dir, &id), "{args:?} keeps reviewed");
        assert!(
            !env.read(&dir, &format!("tasks/{id}.md"))
                .contains("reviewed")
        );
    }
    // Back to idea arrives unreviewed.
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed_reviewed(&dir, &id);
    env.json(&dir, &["shelve", &id, "later"]);
    env.json(&dir, &["unshelve", &id]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "idea");
    assert!(!reviewed(&env, &dir, &id));
}

#[test]
fn a_moved_idea_leaves_an_unreviewed_tombstone_and_a_reviewed_destination() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    env.init("wali");
    let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
    seed_reviewed(&source, &id);
    let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
    let tombstone = env.json(&source, &["show", &id]);
    assert_eq!(tombstone["task"]["status"], "moved");
    assert_eq!(tombstone["task"]["reviewed"], false);
    assert_eq!(
        env.json(&source, &["show", &moved])["task"]["reviewed"],
        true
    );
}

#[test]
fn an_editor_save_follows_the_idea_only_rule() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let add_line = editor_script(
        &dir,
        "sed -i 's/^priority: /reviewed: true\\npriority: /' \"$1\"",
    );
    // Same status, non-idea: refused, file unchanged.
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    let out = env
        .cmd(&dir)
        .env("EDITOR", &add_line)
        .args(["edit", &todo])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("only an idea carries reviewed"));
    assert_eq!(env.read(&dir, &file), before);
    // Same status, idea: allowed, then removable.
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.cmd(&dir)
        .env("EDITOR", &add_line)
        .args(["edit", &idea])
        .assert()
        .success();
    assert!(reviewed(&env, &dir, &idea));
    let drop_line = editor_script(&dir, "sed -i '/^reviewed: /d' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &drop_line)
        .args(["edit", &idea])
        .assert()
        .success();
    assert!(!reviewed(&env, &dir, &idea));
    // Status change from a reviewed idea: the transition drops the flag.
    seed_reviewed(&dir, &idea);
    let to_todo = editor_script(&dir, "sed -i 's/^status: idea/status: todo/' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &to_todo)
        .args(["edit", &idea])
        .assert()
        .success();
    assert!(!reviewed(&env, &dir, &idea));
}

#[test]
fn check_reports_reviewed_on_a_non_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    seed_reviewed(&dir, &todo);
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<&str> = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"review_status"), "{report}");
}

#[test]
fn feedback_recurrence_refuses_reviewed_on_a_non_idea_without_rewriting_it() {
    let mut env = TestEnv::new();
    let target = env.init("tasks");
    let config = env.read(&target, "tasks/.config.toml");
    std::fs::write(
        target.join("tasks/.config.toml"),
        format!("{config}\n[feedback]\nscope = \"The task tracker.\"\n"),
    )
    .unwrap();
    let reporter = env.init("sci");
    let id = id_of(env.json(&target, &["add", "T", "--tag", "feedback"]));
    seed_reviewed(&target, &id);
    let file = format!("tasks/{id}.md");
    let before = env.read(&target, &file);
    let out = env
        .cmd(&reporter)
        .args([
            "feedback",
            "--project",
            "tasks",
            "T again",
            "--category",
            "gap",
            "--recur",
            &id,
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert_eq!(
        error["error"]["detail"],
        format!("{id} is todo; only an idea carries reviewed")
    );
    assert_eq!(env.read(&target, &file), before);
}
