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

fn warnings(v: &serde_json::Value) -> Vec<String> {
    v["warnings"]
        .as_array()
        .map(|w| w.iter().map(|s| s.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

#[test]
fn note_sets_and_clears_reviewed_in_the_same_write() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed; x"]);
    let shown = env.json(&dir, &["show", &id]);
    assert_eq!(shown["task"]["reviewed"], true);
    assert_eq!(shown["task"]["notes"][0]["text"], "scope: briefed; x");
    env.json(&dir, &["note", &id, "--no-reviewed", "finding"]);
    assert!(!reviewed(&env, &dir, &id));
    // A second pass sets it again with a second audit note.
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed; y"]);
    let notes = env.json(&dir, &["show", &id])["task"]["notes"].clone();
    let texts: Vec<&str> = notes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["scope: briefed; x", "finding", "scope: briefed; y"]);
    assert!(reviewed(&env, &dir, &id));
}

#[test]
fn a_plain_note_keeps_reviewed_and_warns_once() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let quiet = env.json(&dir, &["note", &id, "before review"]);
    assert!(warnings(&quiet).is_empty(), "{quiet}");
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let out = env.json(&dir, &["note", &id, "curate: tidy"]);
    assert_eq!(
        warnings(&out),
        [format!(
            "{id} stays reviewed; add --no-reviewed to readmit it for scope"
        )]
    );
    assert!(reviewed(&env, &dir, &id));
}

#[test]
fn a_pending_drop_proposal_is_reviewed_and_still_pending_in_sample() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &id,
            "--reviewed",
            "scope: drop; landed; proposal: drop as covered",
        ],
    );
    assert!(reviewed(&env, &dir, &id));
    let sample = env.json(&dir, &["sample", "--older-than", "0d"]);
    assert!(
        warnings(&sample)
            .iter()
            .any(|w| w.starts_with(&format!("{id} pending:"))),
        "{sample}"
    );
}

#[test]
fn setting_reviewed_on_a_non_idea_is_refused_and_writes_nothing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    for args in [
        vec!["note", todo.as_str(), "--reviewed", "x"],
        vec!["edit", todo.as_str(), "--reviewed"],
        vec!["edit", todo.as_str(), "--status", "todo", "--reviewed"],
    ] {
        assert_eq!(env.fail(&dir, &args), "validation", "{args:?}");
        assert_eq!(env.read(&dir, &file), before, "{args:?}");
    }
    // Clearing on a non-idea is a harmless no-op (Review Focus 2).
    env.json(&dir, &["edit", &todo, "--no-reviewed"]);
    assert!(!reviewed(&env, &dir, &todo));
    // An idea promoted in the same edit refuses the flag; a todo demoted to idea takes it
    // (Review Focus 1).
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    assert_eq!(
        env.fail(&dir, &["edit", &idea, "--status", "todo", "--reviewed"]),
        "validation"
    );
    env.json(&dir, &["edit", &todo, "--status", "idea", "--reviewed"]);
    assert!(reviewed(&env, &dir, &todo));
    env.json(&dir, &["edit", &todo, "--no-reviewed"]);
    assert!(!reviewed(&env, &dir, &todo));
}

#[test]
fn the_flag_forms_conflict() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.usage(&dir, &["note", &id, "--reviewed", "--no-reviewed", "x"]);
    env.usage(&dir, &["edit", &id, "--reviewed", "--no-reviewed"]);
}

#[test]
fn reviewed_writes_follow_the_plain_writes_under_a_foreign_live_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let plain = id_of(env.json(&dir, &["add", "P", "--status", "idea"]));
    let flagged = id_of(env.json(&dir, &["add", "F", "--status", "idea"]));
    write_claim(&env, "sci", &plain, "someone-else");
    write_claim(&env, "sci", &flagged, "someone-else");
    let a = env.cmd(&dir).args(["note", &plain, "x"]).output().unwrap();
    let b = env
        .cmd(&dir)
        .args(["note", &flagged, "--reviewed", "x"])
        .output()
        .unwrap();
    assert_eq!(a.status.code(), b.status.code());
    assert_eq!(reviewed(&env, &dir, &flagged), b.status.success());
    let c = env
        .cmd(&dir)
        .args(["edit", &plain, "-p", "1"])
        .output()
        .unwrap();
    let d = env
        .cmd(&dir)
        .args(["edit", &flagged, "--no-reviewed"])
        .output()
        .unwrap();
    assert_eq!(c.status.code(), d.status.code());
}

/// A live claim held by another session, written straight into the store.
fn write_claim(env: &TestEnv, prefix: &str, id: &str, session: &str) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let rest = stat.rsplit_once(") ").unwrap().1.to_string();
    let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {}\n\
         pid_start = {start}\nboot_id = \"{}\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n",
        std::process::id(),
        boot.trim()
    ));
    std::fs::write(&path, text).unwrap();
}
