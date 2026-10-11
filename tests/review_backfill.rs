mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn tool() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/review-backfill")
}

/// Runs the tool against `dir` with the suite's isolated HOME and the built binary.
fn backfill(env: &TestEnv, dir: &Path, state: &Path, extra: &[&str]) -> (i32, serde_json::Value) {
    backfill_with(
        env,
        dir,
        state,
        &assert_cmd::cargo::cargo_bin("tasks"),
        extra,
    )
}

/// As `backfill`, running `tasks` as `bin`.
fn backfill_with(
    env: &TestEnv,
    dir: &Path,
    state: &Path,
    bin: &Path,
    extra: &[&str],
) -> (i32, serde_json::Value) {
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(tool())
        .args([
            "--state",
            state.to_str().unwrap(),
            "--root",
            dir.to_str().unwrap(),
        ])
        .args(["--tasks", bin.to_str().unwrap()])
        .args(extra)
        .env("HOME", env.home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_FORMAT")
        .env_remove("TASKS_SESSION")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .output()
        .unwrap();
    let code = out.status.code().unwrap();
    let report = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    if report.is_null() {
        eprintln!("stderr: {}", String::from_utf8_lossy(&out.stderr));
    }
    (code, report)
}

fn reviewed(env: &TestEnv, dir: &Path, id: &str) -> bool {
    env.json(dir, &["show", id])["task"]["reviewed"]
        .as_bool()
        .unwrap()
}

fn updated(env: &TestEnv, dir: &Path, id: &str) -> String {
    env.json(dir, &["show", id])["task"]["updated"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Ideas in each state the eligibility rule distinguishes.
struct Fixture {
    dir: PathBuf,
    briefed: String,
    proposed: String,
    readmitted: String,
    untouched: String,
    no_notes: String,
}

fn fixture(env: &mut TestEnv) -> Fixture {
    let dir = env.init("sci");
    let idea =
        |env: &TestEnv, title: &str| id_of(env.json(&dir, &["add", title, "--status", "idea"]));
    let briefed = idea(env, "Briefed");
    env.json(&dir, &["note", &briefed, "scope: briefed; brief: x"]);
    let proposed = idea(env, "Proposed");
    env.json(&dir, &["note", &proposed, "curate: stale; proposal: drop"]);
    let readmitted = idea(env, "Readmitted");
    env.json(&dir, &["note", &readmitted, "scope: briefed"]);
    env.json(&dir, &["note", &readmitted, "finding from research"]);
    let untouched = idea(env, "Untouched");
    env.json(&dir, &["note", &untouched, "just a comment"]);
    let no_notes = idea(env, "No notes");
    Fixture {
        dir,
        briefed,
        proposed,
        readmitted,
        untouched,
        no_notes,
    }
}

#[test]
fn freezes_today_s_rule_and_sets_only_its_candidates() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    let mut set: Vec<String> = report["set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    set.sort();
    let mut want = vec![f.briefed.clone(), f.proposed.clone()];
    want.sort();
    assert_eq!(set, want);
    assert!(reviewed(&env, &f.dir, &f.briefed));
    assert!(reviewed(&env, &f.dir, &f.proposed));
    for id in [&f.readmitted, &f.untouched, &f.no_notes] {
        assert!(!reviewed(&env, &f.dir, id), "{id}");
    }
}

#[test]
fn a_rerun_rewrites_nothing_and_reads_no_notes() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    let file = format!("tasks/{}.md", f.briefed);
    let before = env.read(&f.dir, &file);
    // Readmitted after the first run, by note and by bare edit (latest note still scope:).
    env.json(
        &f.dir,
        &["note", &f.proposed, "--no-reviewed", "new finding"],
    );
    env.json(&f.dir, &["edit", &f.briefed, "--no-reviewed"]);
    let briefed_after = env.read(&f.dir, &file);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["set"].as_array().unwrap().is_empty(), "{report}");
    assert!(!reviewed(&env, &f.dir, &f.proposed));
    assert!(!reviewed(&env, &f.dir, &f.briefed));
    assert_eq!(env.read(&f.dir, &file), briefed_after);
    assert_ne!(before, briefed_after);
}

#[test]
fn an_interrupted_run_or_a_later_scope_pass_is_reported_not_reset() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    // Interrupted: the edit landed, the entry was never marked done; then readmitted.
    env.json(&f.dir, &["edit", &f.briefed, "--reviewed"]);
    env.json(&f.dir, &["edit", &f.briefed, "--no-reviewed"]);
    // An ordinary scope pass after the freeze, then readmitted.
    env.json(
        &f.dir,
        &["note", &f.proposed, "--reviewed", "scope: briefed"],
    );
    env.json(&f.dir, &["edit", &f.proposed, "--no-reviewed"]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["set"].as_array().unwrap().is_empty(), "{report}");
    let changed: Vec<&str> = report["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        changed.contains(&f.briefed.as_str()) && changed.contains(&f.proposed.as_str()),
        "{report}"
    );
    assert!(!reviewed(&env, &f.dir, &f.briefed));
    assert!(!reviewed(&env, &f.dir, &f.proposed));
    // Reported once: a third run reports nothing.
    let (_, third) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert!(third["changed"].as_array().unwrap().is_empty(), "{third}");
}

#[test]
fn apply_needs_the_operator_s_confirmation_and_no_live_claim() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, _) = backfill(&env, &f.dir, state.path(), &[]);
    assert_eq!(code, 2, "no mode is a usage error");
    let stamp = updated(&env, &f.dir, &f.briefed);
    // A live claim in the project refuses the pass before any edit.
    env.json(&f.dir, &["start", &f.untouched]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    assert_eq!(updated(&env, &f.dir, &f.briefed), stamp);
    // Once the claim is released, the next pass sets the untouched candidate (spec §8 case 14).
    env.json(&f.dir, &["park", &f.untouched, "later"]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(reviewed(&env, &f.dir, &f.briefed));
}

#[test]
fn a_claim_refusal_reports_pending_snapshot_entries_without_editing_them() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    let briefed_stamp = updated(&env, &f.dir, &f.briefed);
    let proposed_stamp = updated(&env, &f.dir, &f.proposed);

    env.json(&f.dir, &["start", &f.untouched]);
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["remaining"], 2, "{report}");
    assert!(report["set"].as_array().unwrap().is_empty(), "{report}");
    assert_eq!(updated(&env, &f.dir, &f.briefed), briefed_stamp);
    assert_eq!(updated(&env, &f.dir, &f.proposed), proposed_stamp);
}

#[test]
fn an_invalid_snapshot_refuses_before_any_edit() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let stamp = updated(&env, &f.dir, &f.briefed);
    let valid = |id: &str, updated: &str| serde_json::json!({"id": id, "updated": updated, "done": false, "outcome": null});
    for (name, body) in [
        ("invalid json", "{not json".to_string()),
        (
            "malformed second entry",
            serde_json::json!({"prefix": "sci", "frozen_at": "2026-10-11T00:00:00Z",
                "entries": [valid(&f.briefed, &stamp), {}]})
            .to_string(),
        ),
        (
            "another project's snapshot",
            serde_json::json!({"prefix": "lab", "frozen_at": "2026-10-11T00:00:00Z",
                "entries": [valid(&f.briefed, &stamp)]})
            .to_string(),
        ),
        (
            "a malformed id after a valid candidate",
            serde_json::json!({"prefix": "sci", "frozen_at": "2026-10-11T00:00:00Z",
                "entries": [valid(&f.briefed, &stamp), valid("sci-bad-id", &stamp)]})
            .to_string(),
        ),
        (
            "an entry outside the project",
            serde_json::json!({"prefix": "sci", "frozen_at": "2026-10-11T00:00:00Z",
                "entries": [valid("lab-000001", &stamp)]})
            .to_string(),
        ),
    ] {
        let state = tempfile::tempdir().unwrap();
        std::fs::write(state.path().join("sci.json"), body).unwrap();
        let (code, _) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
        assert_eq!(code, 2, "{name}");
        assert_eq!(
            updated(&env, &f.dir, &f.briefed),
            stamp,
            "{name}: edited before refusing"
        );
    }
}

/// A `tasks` wrapper that fails `show` for one id the way an I/O error would: exit 1 with
/// a non-`task_not_found` error. Everything else runs the real binary.
fn flaky_tasks(dir: &Path, fail_id: &str) -> PathBuf {
    let path = dir.join("flaky-tasks");
    let real = assert_cmd::cargo::cargo_bin("tasks");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh
case \"$*\" in *\"show {fail_id}\"*) \
             echo '{{\"error\":{{\"kind\":\"io\",\"detail\":\"disk hiccup\"}}}}' >&2; exit 1;; esac\n\
             exec '{}' \"$@\"\n",
            real.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    path
}

#[test]
fn a_read_failure_stays_pending_and_a_missing_record_is_a_change() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    let flaky = flaky_tasks(state.path(), &f.briefed);
    std::fs::remove_file(f.dir.join(format!("tasks/{}.md", f.proposed))).unwrap();
    let (code, report) = backfill_with(&env, &f.dir, state.path(), &flaky, &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    let failed: Vec<&str> = report["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(failed, [f.briefed.as_str()], "{report}");
    let changed: Vec<&str> = report["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(changed, [f.proposed.as_str()], "{report}");
    assert_eq!(report["remaining"], 1);
    // The real binary on the next run sets it.
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(reviewed(&env, &f.dir, &f.briefed));
}

#[test]
fn a_deferred_commit_is_reported_and_not_rewritten() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    git(&f.dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
    ] {
        git(&f.dir, &["config", key, value]);
    }
    let config = f.dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    git(&f.dir, &["add", "-A"]);
    git(&f.dir, &["commit", "-q", "-m", "init"]);
    // A staged change elsewhere defers every record commit (record-commit design §3.3).
    std::fs::write(f.dir.join("other.txt"), "x").unwrap();
    git(&f.dir, &["add", "other.txt"]);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    let uncommitted: Vec<&str> = report["uncommitted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert!(uncommitted.contains(&f.briefed.as_str()), "{report}");
    assert!(
        report["uncommitted"][0]["warning"]
            .as_str()
            .unwrap()
            .starts_with("commit_deferred:")
    );
    assert!(reviewed(&env, &f.dir, &f.briefed));
    let file = format!("tasks/{}.md", f.briefed);
    let after = env.read(&f.dir, &file);
    let (_, rerun) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert!(
        rerun["set"].as_array().unwrap().is_empty()
            && rerun["uncommitted"].as_array().unwrap().is_empty(),
        "{rerun}"
    );
    assert_eq!(
        env.read(&f.dir, &file),
        after,
        "a rerun rewrote a record it had set"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
