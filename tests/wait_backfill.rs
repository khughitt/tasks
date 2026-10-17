mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn tool() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/wait-backfill")
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

fn snapshot(state: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(state.join("sci.json")).unwrap()).unwrap()
}

fn decide(state: &Path, id: &str, decision: serde_json::Value) {
    let mut snap = snapshot(state);
    for entry in snap["entries"].as_array_mut().unwrap() {
        if entry["id"] == id {
            entry["decision"] = decision.clone();
        }
    }
    std::fs::write(
        state.join("sci.json"),
        serde_json::to_string_pretty(&snap).unwrap(),
    )
    .unwrap();
}

struct Fixture {
    dir: PathBuf,
    one: String, // names exactly one open task: auto edge
    follow_up: String,
    several: String, // names two open tasks: judge
    none: String,    // names nothing: judge
    closed: String,  // names only a closed task: judge
    waiting: String, // has an open edge: not stranded, not a candidate
}

fn fixture(env: &mut TestEnv) -> Fixture {
    let dir = env.init("sci");
    let add_idea =
        |env: &TestEnv, title: &str| id_of(env.json(&dir, &["add", title, "--status", "idea"]));
    let follow_up = id_of(env.json(&dir, &["add", "F"]));
    let second = id_of(env.json(&dir, &["add", "G"]));
    let finished = id_of(env.json(&dir, &["add", "H"]));
    env.json(&dir, &["done", &finished]);
    let one = add_idea(env, "One");
    env.json(
        &dir,
        &[
            "note",
            &one,
            "--reviewed",
            &format!("scope: briefed; waits on {follow_up}"),
        ],
    );
    let several = add_idea(env, "Several");
    env.json(
        &dir,
        &[
            "note",
            &several,
            "--reviewed",
            &format!("scope: briefed; see {follow_up} and {second}"),
        ],
    );
    let none = add_idea(env, "None");
    env.json(
        &dir,
        &[
            "note",
            &none,
            "--reviewed",
            "scope: question; once Q1 is answered",
        ],
    );
    let closed = add_idea(env, "Closed");
    env.json(
        &dir,
        &[
            "note",
            &closed,
            "--reviewed",
            &format!("scope: briefed; waits on {finished}"),
        ],
    );
    let waiting = add_idea(env, "Waiting");
    env.json(&dir, &["dep", &waiting, "--on", &second]);
    env.json(&dir, &["note", &waiting, "--reviewed", "scope: briefed"]);
    Fixture {
        dir,
        one,
        follow_up,
        several,
        none,
        closed,
        waiting,
    }
}

#[test]
fn freeze_classifies_stranded_ideas_by_their_latest_scope_note() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    let snap = snapshot(state.path());
    let entry = |id: &str| {
        snap["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .cloned()
    };
    let one = entry(&f.one).unwrap();
    assert_eq!(one["class"], "auto");
    assert_eq!(one["decision"], serde_json::json!({"dep": [f.follow_up]}));
    for id in [&f.several, &f.none, &f.closed] {
        let e = entry(id).unwrap();
        assert_eq!(e["class"], "judge", "{e}");
        assert!(e["decision"].is_null(), "{e}");
    }
    assert!(
        entry(&f.waiting).is_none(),
        "an idea with an open edge is not stranded"
    );
}

#[test]
fn freeze_follows_moves_and_reports_unresolved_references() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    env.init("wali");
    // A follow-up moved to another project is followed to its open successor.
    let moving = id_of(env.json(&f.dir, &["add", "M"]));
    let moved_idea = id_of(env.json(&f.dir, &["add", "Moved", "--status", "idea"]));
    env.json(
        &f.dir,
        &[
            "note",
            &moved_idea,
            "--reviewed",
            &format!("scope: briefed; waits on {moving}"),
        ],
    );
    let successor = id_of(env.json(&f.dir, &["move", &moving, "--to", "wali"]));
    // A note naming one open task and one that cannot be read is never automatic.
    let partial = id_of(env.json(&f.dir, &["add", "Partial", "--status", "idea"]));
    env.json(
        &f.dir,
        &[
            "note",
            &partial,
            "--reviewed",
            &format!("scope: briefed; see {} and sci-ffffff", f.follow_up),
        ],
    );
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    let snap = snapshot(state.path());
    let entry = |id: &str| {
        snap["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .cloned()
            .unwrap()
    };
    let moved = entry(&moved_idea);
    assert_eq!(moved["class"], "auto", "{moved}");
    assert_eq!(moved["decision"], serde_json::json!({"dep": [successor]}));
    let partial = entry(&partial);
    assert_eq!(partial["class"], "judge", "{partial}");
    assert!(partial["decision"].is_null(), "{partial}");
    assert_eq!(partial["named_open"], serde_json::json!([f.follow_up]));
    assert!(
        partial["unresolved"].to_string().contains("sci-ffffff"),
        "{partial}"
    );
}

#[test]
fn freeze_skips_a_record_that_changes_between_list_and_show() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    // A `tasks` shim that, on the freeze's show of `one`, first lets another writer mark it.
    let real = assert_cmd::cargo::cargo_bin("tasks");
    let flag = f.dir.join("shim-fired");
    let shim = f.dir.join("tasks-shim");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nif [ \"$3\" = show ] && [ \"$4\" = {one} ] && [ ! -e '{flag}' ]; then\n  touch '{flag}'\n  '{real}' -C \"$2\" edit {one} --awaiting-user >/dev/null || exit 9\nfi\nexec '{real}' \"$@\"\n",
            one = f.one,
            flag = flag.display(),
            real = real.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&shim, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let state = tempfile::tempdir().unwrap();
    let (code, report) = backfill_with(&env, &f.dir, state.path(), &shim, &["--freeze-only"]);
    assert_eq!(code, 0, "{report}");
    assert!(flag.exists(), "the shim never ran");
    assert!(report["changed"].to_string().contains(&f.one), "{report}");
    let snap = snapshot(state.path());
    assert!(
        !snap["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == f.one.as_str()),
        "{snap}"
    );
    assert_eq!(
        env.json(&f.dir, &["show", &f.one])["task"]["awaiting_user"],
        true
    );
}

#[test]
fn apply_runs_decisions_only_on_unchanged_records() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    decide(state.path(), &f.none, serde_json::json!("awaiting_user"));
    decide(state.path(), &f.closed, serde_json::json!("leave"));
    // `several` stays undecided. Before the apply, a scope pass re-reviews `several`
    // and gives it an edge: its frozen assumptions no longer hold.
    env.json(&f.dir, &["dep", &f.several, "--on", &f.follow_up]);
    env.json(
        &f.dir,
        &[
            "note",
            &f.several,
            "--reviewed",
            "scope: briefed; new research",
        ],
    );
    decide(state.path(), &f.several, serde_json::json!("awaiting_user"));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    let show = |id: &str| env.json(&f.dir, &["show", id])["task"].clone();
    assert!(show(&f.one)["depends"].to_string().contains(&f.follow_up));
    assert_eq!(show(&f.none)["awaiting_user"], true);
    assert_eq!(
        show(&f.several)["awaiting_user"],
        false,
        "changed since the freeze: no marker"
    );
    assert!(
        report["changed"].to_string().contains(&f.several),
        "{report}"
    );
    let detail = report["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == f.several.as_str())
        .unwrap()["detail"]
        .as_str()
        .unwrap();
    assert!(
        detail.contains("review_reason null"),
        "waiting on an open edge, so not queued: {detail}"
    );
    assert!(report["left"].to_string().contains(&f.closed), "{report}");
    // The re-reviewed idea wakes when its new dependency closes.
    env.json(&f.dir, &["done", &f.follow_up]);
    let queue = env.json(&f.dir, &["list", "--needs-review"]).to_string();
    assert!(queue.contains(&f.several), "{queue}");
}

#[test]
fn rerun_rewrites_nothing_and_reads_no_notes() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    decide(state.path(), &f.none, serde_json::json!("awaiting_user"));
    backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    let before: Vec<String> = [&f.one, &f.none, &f.several, &f.closed]
        .iter()
        .map(|id| env.read(&f.dir, &format!("tasks/{id}.md")))
        .collect();
    // A later scope note would change the classification if notes were re-read.
    env.json(
        &f.dir,
        &[
            "note",
            &f.closed,
            "--reviewed",
            &format!("scope: briefed; waits on {}", f.follow_up),
        ],
    );
    let closed_after = env.read(&f.dir, &format!("tasks/{}.md", f.closed));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    for (id, text) in [&f.one, &f.none, &f.several].iter().zip(&before) {
        assert_eq!(&env.read(&f.dir, &format!("tasks/{id}.md")), text);
    }
    assert_eq!(
        env.read(&f.dir, &format!("tasks/{}.md", f.closed)),
        closed_after
    );
}

#[test]
fn an_interrupted_pass_reports_the_entry_changed_and_writes_nothing() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    // Simulate a pass that ran the auto edge and died before marking the entry done.
    env.json(&f.dir, &["dep", &f.one, "--on", &f.follow_up]);
    let before = env.read(&f.dir, &format!("tasks/{}.md", f.one));
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(report["changed"].to_string().contains(&f.one), "{report}");
    assert_eq!(env.read(&f.dir, &format!("tasks/{}.md", f.one)), before);
}

#[test]
fn a_live_claim_refuses_the_pass_and_a_later_pass_applies() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    backfill(&env, &f.dir, state.path(), &["--freeze-only"]);
    let other = id_of(env.json(&f.dir, &["add", "Busy"]));
    env.cmd(&f.dir)
        .env("TASKS_SESSION", "agent-x")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", &other])
        .assert()
        .success();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 1, "{report}");
    assert!(
        !env.json(&f.dir, &["show", &f.one])["task"]["depends"]
            .to_string()
            .contains(&f.follow_up)
    );
    env.cmd(&f.dir)
        .env("TASKS_SESSION", "agent-x")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["park", &other, "later"])
        .assert()
        .success();
    let (code, report) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 0, "{report}");
    assert!(
        env.json(&f.dir, &["show", &f.one])["task"]["depends"]
            .to_string()
            .contains(&f.follow_up)
    );
}

#[test]
fn a_corrupt_snapshot_refuses_before_any_write() {
    let mut env = TestEnv::new();
    let f = fixture(&mut env);
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("sci.json"), "{not json").unwrap();
    let before = env.read(&f.dir, &format!("tasks/{}.md", f.one));
    let (code, _) = backfill(&env, &f.dir, state.path(), &["--writers-paused"]);
    assert_eq!(code, 2);
    assert_eq!(env.read(&f.dir, &format!("tasks/{}.md", f.one)), before);
}
