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

fn transaction_state(
    env: &TestEnv,
    projects: &[&Path],
) -> std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn collect(
        path: &Path,
        state: &mut std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>>,
    ) {
        if path.is_dir() {
            state.insert(path.to_path_buf(), None);
            for entry in std::fs::read_dir(path).unwrap() {
                collect(&entry.unwrap().path(), state);
            }
        } else if path.exists() {
            state.insert(path.to_path_buf(), Some(std::fs::read(path).unwrap()));
        }
    }
    let mut state = std::collections::BTreeMap::new();
    for project in projects {
        collect(&project.join("tasks"), &mut state);
    }
    for relative in [
        ".config/tasks/projects.toml",
        ".local/state/tasks/move",
        ".local/state/tasks/rename",
    ] {
        collect(&env.home.path().join(relative), &mut state);
    }
    state
}

fn refuse_review_write(env: &TestEnv, project: &Path, args: &[&str]) {
    let output = env.cmd(project).args(args).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("only an idea carries reviewed")
    );
}

fn attach_review_fixture(env: &TestEnv, project: &Path, id: &str) {
    let file = project.join("payload.txt");
    std::fs::write(&file, b"preserve these bytes").unwrap();
    env.json(project, &["attach", id, file.to_str().unwrap()]);
}

fn review_digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn fresh_move_refuses_reviewed_todo_without_transaction_writes() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let destination = env.init("wali");
    let id = id_of(env.json(&source, &["add", "T"]));
    attach_review_fixture(&env, &source, &id);
    seed_reviewed(&source, &id);
    assert!(reviewed(&env, &source, &id)); // Invalid records remain readable.
    let before = transaction_state(&env, &[&source, &destination]);
    refuse_review_write(&env, &source, &["move", &id, "--to", "wali"]);
    assert_eq!(transaction_state(&env, &[&source, &destination]), before);
}

#[test]
fn resumed_move_refuses_reviewed_todo_without_transaction_writes() {
    for stop in ["journal", "destination"] {
        let mut env = TestEnv::new();
        let source = env.init("dots");
        let destination = env.init("wali");
        let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
        attach_review_fixture(&env, &source, &id);
        seed_reviewed(&source, &id);
        env.cmd(&source)
            .env("TASKS_MOVE_STOP_AFTER", stop)
            .args(["move", &id, "--to", "wali"])
            .assert()
            .success();
        // Model a coherent journal published by a writer without the review invariant.
        let path = env
            .home
            .path()
            .join(format!(".local/state/tasks/move/{id}.toml"));
        let mut journal: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for key in ["source_text", "target_text"] {
            journal[key] = toml::Value::String(
                journal[key]
                    .as_str()
                    .unwrap()
                    .replace("status: idea", "status: todo"),
            );
        }
        journal["source_digest"] =
            toml::Value::String(review_digest(journal["source_text"].as_str().unwrap()));
        std::fs::write(
            source.join(format!("tasks/{id}.md")),
            journal["source_text"].as_str().unwrap(),
        )
        .unwrap();
        let target = journal["target"].as_str().unwrap();
        if stop == "destination" {
            std::fs::write(
                destination.join(format!("tasks/{target}.md")),
                journal["target_text"].as_str().unwrap(),
            )
            .unwrap();
        }
        std::fs::write(&path, toml::to_string(&journal).unwrap()).unwrap();
        let before = transaction_state(&env, &[&source, &destination]);
        refuse_review_write(&env, &source, &["move", &id, "--to", "wali"]);
        assert_eq!(
            transaction_state(&env, &[&source, &destination]),
            before,
            "{stop}"
        );
    }
}

#[test]
fn fresh_rename_refuses_reviewed_todo_without_transaction_writes() {
    let mut env = TestEnv::new();
    let source = env.init("dots");
    let id = id_of(env.json(&source, &["add", "T"]));
    attach_review_fixture(&env, &source, &id);
    seed_reviewed(&source, &id);
    let before = transaction_state(&env, &[&source]);
    refuse_review_write(&env, &source, &["rename", "dots", "wali"]);
    assert_eq!(transaction_state(&env, &[&source]), before);
}

#[test]
fn resumed_rename_refuses_reviewed_todo_without_transaction_writes() {
    for stop in ["config", "inventory", "file:0", "files", "registry"] {
        let mut env = TestEnv::new();
        let source = env.init("dots");
        let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
        attach_review_fixture(&env, &source, &id);
        seed_reviewed(&source, &id);
        env.cmd(&source)
            .env("TASKS_RENAME_STOP_AFTER", stop)
            .args(["rename", "dots", "wali"])
            .assert()
            .success();
        let old = source.join(format!("tasks/{id}.md"));
        let new_id = id.replace("dots-", "wali-");
        let new = source.join(format!("tasks/{new_id}.md"));
        let text = std::fs::read_to_string(if old.exists() { &old } else { &new }).unwrap();
        let old_text = text
            .replace("status: idea", "status: todo")
            .replace(&format!("id: {new_id}"), &format!("id: {id}"));
        let new_text = old_text.replace(&format!("id: {id}"), &format!("id: {new_id}"));
        if old.exists() {
            std::fs::write(&old, &old_text).unwrap();
        }
        if new.exists() {
            std::fs::write(&new, &new_text).unwrap();
        }
        // Keep the old writer's inventory consistent with its invalid records.
        let path = env.home.path().join(".local/state/tasks/rename/dots.toml");
        let mut inventory: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        inventory["entries"][0]["from"] = toml::Value::String(review_digest(&old_text));
        inventory["entries"][0]["to"] = toml::Value::String(review_digest(&new_text));
        std::fs::write(&path, toml::to_string(&inventory).unwrap()).unwrap();
        let before = transaction_state(&env, &[&source]);
        refuse_review_write(&env, &source, &["rename", "dots", "wali"]);
        assert_eq!(transaction_state(&env, &[&source]), before, "{stop}");
    }
}

#[test]
fn a_moved_idea_leaves_an_unreviewed_tombstone_and_a_reviewed_destination() {
    for stop in [
        None,
        Some("journal"),
        Some("destination"),
        Some("tombstone"),
    ] {
        let mut env = TestEnv::new();
        let source = env.init("dots");
        let destination = env.init("wali");
        let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
        attach_review_fixture(&env, &source, &id);
        seed_reviewed(&source, &id);
        if let Some(stop) = stop {
            env.cmd(&source)
                .env("TASKS_MOVE_STOP_AFTER", stop)
                .args(["move", &id, "--to", "wali"])
                .assert()
                .success();
        }
        let moved = id_of(env.json(&source, &["move", &id, "--to", "wali"]));
        let tombstone = env.json(&source, &["show", &id]);
        assert_eq!(tombstone["task"]["status"], "moved");
        assert_eq!(tombstone["task"]["reviewed"], false);
        assert!(reviewed(&env, &destination, &moved));
        assert_eq!(
            env.read(&destination, &format!("tasks/files/{moved}/payload.txt")),
            "preserve these bytes"
        );
        env.check(&source);
        env.check(&destination);
    }
}

#[test]
fn a_renamed_idea_keeps_reviewed() {
    for stop in [
        None,
        Some("inventory"),
        Some("file:0"),
        Some("config"),
        Some("registry"),
    ] {
        let mut env = TestEnv::new();
        let source = env.init("dots");
        let id = id_of(env.json(&source, &["add", "I", "--status", "idea"]));
        attach_review_fixture(&env, &source, &id);
        seed_reviewed(&source, &id);
        if let Some(stop) = stop {
            env.cmd(&source)
                .env("TASKS_RENAME_STOP_AFTER", stop)
                .args(["rename", "dots", "wali"])
                .assert()
                .success();
        }
        env.json(&source, &["rename", "dots", "wali"]);
        let renamed = id.replace("dots-", "wali-");
        assert!(reviewed(&env, &source, &renamed));
        assert_eq!(
            env.read(&source, &format!("tasks/files/{renamed}/payload.txt")),
            "preserve these bytes"
        );
        env.check(&source);
    }
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

fn rows(v: &serde_json::Value) -> Vec<(String, bool)> {
    v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_string(),
                r["needs_review"].as_bool().unwrap(),
            )
        })
        .collect()
}

/// One of each kind spec §4 distinguishes, returned as (marked ids, unmarked ids).
fn mixed_project(env: &mut TestEnv) -> (std::path::PathBuf, Vec<String>, Vec<String>) {
    let dir = env.init("sci");
    let new_idea = id_of(env.json(&dir, &["add", "New", "--status", "idea"]));
    let briefed = id_of(env.json(&dir, &["add", "Briefed", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &briefed,
            "--reviewed",
            "--awaiting-user",
            "scope: question",
        ],
    );
    let raw_todo = id_of(env.json(&dir, &["add", "Raw"]));
    let assessed = id_of(env.json(&dir, &["add", "Assessed", "--process", "direct"]));
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(
        &dir,
        &["add", "Child", "--parent", &goal, "--process", "direct"],
    ));
    let blocked = id_of(env.json(&dir, &["add", "Blocked"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    let doing = id_of(env.json(&dir, &["add", "Doing"]));
    env.json(&dir, &["start", &doing]);
    (
        dir,
        vec![new_idea, raw_todo, blocked],
        vec![briefed, assessed, goal, child, doing],
    )
}

#[test]
fn needs_review_marks_exactly_spec_section_4_in_json_and_the_filter() {
    let mut env = TestEnv::new();
    let (dir, marked, unmarked) = mixed_project(&mut env);
    let json = env.json(&dir, &["list"]);
    let listed = rows(&json);
    for row in json["tasks"].as_array().unwrap() {
        assert_eq!(row["reviewed"], row["id"] == unmarked[0]);
    }
    for id in &marked {
        assert!(
            listed.contains(&(id.clone(), true)),
            "{id} should need review: {listed:?}"
        );
    }
    for id in &unmarked {
        assert!(
            listed.contains(&(id.clone(), false)),
            "{id} should not: {listed:?}"
        );
    }
    let filtered: Vec<String> = rows(&env.json(&dir, &["list", "--needs-review"]))
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let mut want = marked.clone();
    want.sort();
    let mut got = filtered.clone();
    got.sort();
    assert_eq!(got, want);
    // `ready` never shows ideas, so its filter narrows to the unassessed todo.
    let ready: Vec<String> = rows(&env.json(&dir, &["ready", "--needs-review"]))
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ready, [marked[1].clone()]);
    // `show` carries the stored flag.
    assert_eq!(
        env.json(&dir, &["show", &unmarked[0]])["task"]["reviewed"],
        true
    );
}

/// A park entry written straight into the store, for a task or for an id with no record.
fn write_park(env: &TestEnv, prefix: &str, id: &str) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[parks.\"{id}\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"next\"\n\
         waiting_on = \"user\"\ntitle = \"T\"\n"
    ));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn parked_rows_carry_needs_review_and_filter_on_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let unreviewed = id_of(env.json(&dir, &["add", "U", "--status", "idea"]));
    let reviewed_idea = id_of(env.json(&dir, &["add", "R", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &reviewed_idea,
            "--reviewed",
            "--awaiting-user",
            "scope: question",
        ],
    );
    write_park(&env, "sci", &unreviewed);
    write_park(&env, "sci", &reviewed_idea);
    write_park(&env, "sci", "sci-abcdef");
    let parked = env.json(&dir, &["list", "--parked"]);
    let by_id = |id: &str| {
        parked["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} missing: {parked}"))
    };
    assert_eq!(by_id(&unreviewed)["needs_review"], true);
    assert_eq!(by_id(&unreviewed)["reviewed"], false);
    assert_eq!(by_id(&reviewed_idea)["needs_review"], false);
    assert_eq!(by_id(&reviewed_idea)["reviewed"], true);
    assert!(by_id("sci-abcdef").get("needs_review").is_none());
    assert!(by_id("sci-abcdef").get("reviewed").is_none());
    let filtered: Vec<String> = env.json(&dir, &["list", "--parked", "--needs-review"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(filtered, [unreviewed]);
}

/// The first two characters of each pretty row that names `id`.
fn prefix_of(pretty: &str, id: &str) -> String {
    let line = pretty
        .lines()
        .find(|l| l.contains(id))
        .unwrap_or_else(|| panic!("{id} not in:\n{pretty}"));
    line.chars().take(2).collect()
}

#[test]
fn pretty_list_marks_exactly_the_rows_json_marks() {
    let mut env = TestEnv::new();
    let (dir, marked, unmarked) = mixed_project(&mut env);
    let pretty = env.pretty(&dir, &["list"]);
    for id in &marked {
        assert_eq!(prefix_of(&pretty, id), "* ", "{id}:\n{pretty}");
    }
    for id in &unmarked {
        assert_eq!(prefix_of(&pretty, id), "  ", "{id}:\n{pretty}");
    }
    let ready = env.pretty(&dir, &["ready"]);
    assert_eq!(prefix_of(&ready, &marked[1]), "* ");
    let tree = env.pretty(&dir, &["tree"]);
    assert_eq!(prefix_of(&tree, &marked[0]), "* ");
}

#[test]
fn a_fully_reviewed_listing_renders_without_the_prefix() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A", "--process", "direct"]));
    let pretty = env.pretty(&dir, &["list"]);
    assert!(pretty.starts_with(&id), "{pretty}");
}

#[test]
fn the_mark_stays_left_of_a_tree_indent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &goal]));
    let tree = env.pretty(&dir, &["tree"]);
    let line = tree.lines().find(|l| l.contains(&child)).unwrap();
    assert!(line.starts_with(&format!("*   {child}")), "{tree}");
    assert!(
        tree.lines()
            .find(|l| l.contains(&goal))
            .unwrap()
            .starts_with(&format!("  {goal}"))
    );
}

#[test]
fn parked_views_mark_the_same_rows_as_their_json() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let unreviewed = id_of(env.json(&dir, &["add", "U", "--status", "idea"]));
    let reviewed_idea = id_of(env.json(&dir, &["add", "R", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &reviewed_idea,
            "--reviewed",
            "--awaiting-user",
            "scope: question",
        ],
    );
    write_park(&env, "sci", &unreviewed);
    write_park(&env, "sci", &reviewed_idea);
    write_park(&env, "sci", "sci-abcdef");
    let parked = env.pretty(&dir, &["list", "--parked"]);
    assert_eq!(prefix_of(&parked, &unreviewed), "* ");
    assert_eq!(prefix_of(&parked, &reviewed_idea), "  ");
    assert_eq!(prefix_of(&parked, "sci-abcdef"), "  ");
    let prime = env.pretty(&dir, &["prime"]);
    let section = prime.split("parked:").nth(1).unwrap();
    assert_eq!(prefix_of(section, &unreviewed), "* ");
}

#[test]
fn show_names_a_reviewed_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    assert!(!env.pretty(&dir, &["show", &id]).contains("reviewed"));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let shown = env.pretty(&dir, &["show", &id]);
    assert!(
        shown
            .lines()
            .any(|l| l.starts_with("reviewed") && l.contains("yes (")),
        "{shown}"
    );
}

/// Inserts frontmatter lines into a record by hand, before its priority line: states the
/// writers never produce, or stamps a test needs at a fixed instant.
fn seed(dir: &Path, id: &str, lines: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let seeded = text.replacen("\npriority: ", &format!("\n{lines}\npriority: "), 1);
    assert_ne!(seeded, text, "no priority line in {id}");
    std::fs::write(&path, seeded).unwrap();
}

/// Rewrites a stored stamp line in place, so a test can age it.
fn restamp(dir: &Path, id: &str, key: &str, value: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("\n{key}: ");
    let start = text
        .find(&prefix)
        .unwrap_or_else(|| panic!("no {key} in {id}"))
        + 1;
    let end = start + text[start..].find('\n').unwrap();
    let out = format!("{}{key}: {value}{}", &text[..start], &text[end..]);
    std::fs::write(&path, out).unwrap();
}

fn task(env: &TestEnv, dir: &Path, id: &str) -> serde_json::Value {
    env.json(dir, &["show", id])["task"].clone()
}

fn check_kinds(env: &TestEnv, dir: &Path) -> Vec<String> {
    let out = env.cmd(dir).args(["check"]).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn done_and_drop_stamp_closed_and_reopening_clears_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let done = id_of(env.json(&dir, &["add", "D"]));
    env.json(&dir, &["done", &done]);
    let shown = task(&env, &dir, &done);
    assert!(shown["closed"].is_string(), "{shown}");
    assert_eq!(shown["closed"], shown["completed"]);
    let dropped = id_of(env.json(&dir, &["add", "X"]));
    env.json(&dir, &["drop", &dropped]);
    let shown = task(&env, &dir, &dropped);
    assert!(shown["closed"].is_string(), "{shown}");
    assert!(shown.get("completed").is_none(), "{shown}");
    env.json(&dir, &["edit", &done, "--status", "todo"]);
    assert!(task(&env, &dir, &done).get("closed").is_none());
    assert!(
        !env.read(&dir, &format!("tasks/{done}.md"))
            .contains("closed:")
    );
    // A recurring completion stamps it, a reopen clears it, the next completion stamps it again.
    let every = id_of(env.json(&dir, &["add", "R", "--every", "7d"]));
    env.json(&dir, &["start", &every]);
    env.json(&dir, &["done", &every]);
    assert!(task(&env, &dir, &every)["closed"].is_string());
    env.json(&dir, &["start", &every]);
    assert!(task(&env, &dir, &every).get("closed").is_none());
    env.json(&dir, &["done", &every]);
    assert!(task(&env, &dir, &every)["closed"].is_string());
}

#[test]
fn every_status_transition_clears_the_review_stamp_and_marker() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(
        &dir,
        &id,
        "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\nawaiting_user: true",
    );
    env.json(&dir, &["edit", &id, "--status", "todo"]);
    let text = env.read(&dir, &format!("tasks/{id}.md"));
    for key in ["reviewed", "reviewed_at", "awaiting_user"] {
        assert!(!text.contains(&format!("\n{key}:")), "{key} kept: {text}");
    }
}

#[test]
fn check_reports_review_stamps_and_closed_out_of_place() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "T"]));
    seed(&dir, &todo, "awaiting_user: true");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(&dir, &idea, "reviewed_at: 2026-10-17T09:00:00Z");
    let open = id_of(env.json(&dir, &["add", "O"]));
    seed(&dir, &open, "closed: 2026-10-17T09:00:00Z");
    let kinds = check_kinds(&env, &dir);
    assert_eq!(
        kinds.iter().filter(|k| *k == "review_status").count(),
        2,
        "{kinds:?}"
    );
    assert_eq!(
        kinds.iter().filter(|k| *k == "closed_status").count(),
        1,
        "{kinds:?}"
    );
}

#[test]
fn the_editor_refuses_to_change_closed_or_reviewed_at() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let done = id_of(env.json(&dir, &["add", "D"]));
    env.json(&dir, &["done", &done]);
    let file = format!("tasks/{done}.md");
    let before = env.read(&dir, &file);
    let script = editor_script(
        &dir,
        "sed -i 's/^closed: .*/closed: 2020-01-01T00:00:00Z/' \"$1\"",
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &script)
        .args(["edit", &done])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("closed is stamped by closing the task; it cannot be edited")
    );
    assert_eq!(env.read(&dir, &file), before);
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    seed(
        &dir,
        &idea,
        "reviewed: true\nreviewed_at: 2026-10-17T09:00:00Z",
    );
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let script = editor_script(
        &dir,
        "sed -i 's/^reviewed_at: .*/reviewed_at: 2020-01-01T00:00:00Z/' \"$1\"",
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &script)
        .args(["edit", &idea])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("reviewed_at is stamped by reviewing the idea; it cannot be edited")
    );
    assert_eq!(env.read(&dir, &file), before);
}

#[test]
fn reviewing_stamps_reviewed_at_and_a_rereview_restamps_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed"]);
    let first = task(&env, &dir, &id)["reviewed_at"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(first.ends_with('Z'), "{first}");
    restamp(&dir, &id, "reviewed_at", "2026-01-01T00:00:00Z");
    env.json(&dir, &["note", &id, "--reviewed", "scope: briefed again"]);
    assert_ne!(task(&env, &dir, &id)["reviewed_at"], "2026-01-01T00:00:00Z");
    restamp(&dir, &id, "reviewed_at", "2026-01-01T00:00:00Z");
    env.json(&dir, &["edit", &id, "--reviewed"]);
    assert_ne!(task(&env, &dir, &id)["reviewed_at"], "2026-01-01T00:00:00Z");
}

#[test]
fn awaiting_user_is_set_with_a_review_and_cleared_by_readmission() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &id,
            "--reviewed",
            "--awaiting-user",
            "scope: question",
        ],
    );
    let shown = task(&env, &dir, &id);
    assert_eq!(shown["reviewed"], true);
    assert_eq!(shown["awaiting_user"], true);
    // A plain note keeps both and says so.
    let out = env.json(&dir, &["note", &id, "an aside"]);
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains(&format!(
            "{id} stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope"
        )),
        "{warnings}"
    );
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], true);
    // edit clears and sets the marker alone, without a warning.
    let out = env.json(&dir, &["edit", &id, "--no-awaiting-user"]);
    assert!(
        !out["warnings"].to_string().contains("stays reviewed"),
        "{out}"
    );
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], false);
    assert_eq!(task(&env, &dir, &id)["reviewed"], true);
    env.json(&dir, &["edit", &id, "--awaiting-user"]);
    assert_eq!(task(&env, &dir, &id)["awaiting_user"], true);
    // The answer readmits it: both flags and the stamp go.
    env.json(&dir, &["note", &id, "--no-reviewed", "answer: yes"]);
    let shown = task(&env, &dir, &id);
    assert_eq!(shown["reviewed"], false);
    assert_eq!(shown["awaiting_user"], false);
    assert!(shown.get("reviewed_at").is_none(), "{shown}");
}

#[test]
fn awaiting_user_is_refused_without_a_review_and_on_a_non_idea() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let out = env
        .cmd(&dir)
        .args(["note", &idea, "--awaiting-user", "q"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(&format!(
        "{idea} is not reviewed; only a reviewed idea carries awaiting_user"
    )));
    assert_eq!(env.read(&dir, &file), before);
    let todo = id_of(env.json(&dir, &["add", "T"]));
    let file = format!("tasks/{todo}.md");
    let before = env.read(&dir, &file);
    let out = env
        .cmd(&dir)
        .args(["edit", &todo, "--awaiting-user"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("only an idea carries reviewed"));
    assert_eq!(env.read(&dir, &file), before);
    // clap: the marker cannot ride on a readmission.
    env.usage(
        &dir,
        &["note", &idea, "--awaiting-user", "--no-reviewed", "x"],
    );
    env.usage(
        &dir,
        &["edit", &idea, "--awaiting-user", "--no-awaiting-user"],
    );
}

#[test]
fn the_editor_stamps_reviewed_at_with_reviewed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    let add = editor_script(
        &dir,
        "sed -i 's/^priority: /reviewed: true\\npriority: /' \"$1\"",
    );
    env.cmd(&dir)
        .env("EDITOR", &add)
        .args(["edit", &idea])
        .assert()
        .success();
    assert!(task(&env, &dir, &idea)["reviewed_at"].is_string());
    // Removing reviewed while the marker stays is refused, file unchanged.
    env.json(&dir, &["edit", &idea, "--awaiting-user"]);
    let file = format!("tasks/{idea}.md");
    let before = env.read(&dir, &file);
    let drop_reviewed = editor_script(&dir, "sed -i '/^reviewed: /d' \"$1\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &drop_reviewed)
        .args(["edit", &idea])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("only a reviewed idea carries awaiting_user")
    );
    assert_eq!(env.read(&dir, &file), before);
    // Removing both clears the stamp.
    env.json(&dir, &["edit", &idea, "--no-awaiting-user"]);
    env.cmd(&dir)
        .env("EDITOR", &drop_reviewed)
        .args(["edit", &idea])
        .assert()
        .success();
    assert!(task(&env, &dir, &idea).get("reviewed_at").is_none());
}

#[test]
fn show_prints_the_review_date_the_marker_and_closed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = id_of(env.json(&dir, &["add", "I", "--status", "idea"]));
    env.json(
        &dir,
        &[
            "note",
            &idea,
            "--reviewed",
            "--awaiting-user",
            "scope: question",
        ],
    );
    restamp(&dir, &idea, "reviewed_at", "2026-01-02T03:04:05Z");
    let shown = env.pretty(&dir, &["show", &idea]);
    assert!(
        shown.contains("reviewed") && shown.contains("yes (2026-01-02)"),
        "{shown}"
    );
    assert!(
        shown.contains("awaiting") && shown.contains("user"),
        "{shown}"
    );
    let legacy = id_of(env.json(&dir, &["add", "L", "--status", "idea"]));
    seed_reviewed(&dir, &legacy);
    let shown = env.pretty(&dir, &["show", &legacy]);
    assert!(shown.contains("yes") && !shown.contains("yes ("), "{shown}");
    let dropped = id_of(env.json(&dir, &["add", "X"]));
    env.json(&dir, &["drop", &dropped]);
    assert!(env.pretty(&dir, &["show", &dropped]).contains("closed"));
}
