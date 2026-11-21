mod common;
use common::TestEnv;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const THREAD_ID: &str = "019a0000-0000-7000-8000-000000000001";
const VERIFIED_WITH_EFFORT: &str = r#"{"verified":true,"harness":"codex","agent":"codex/mock-model@low","model":"mock-model","effort":"low","reason":null,"source":{"nearest":"codex"}}"#;
const VERIFIED_WITHOUT_EFFORT: &str = r#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"nearest":"codex"}}"#;

fn resolve_relay_on_test_path(env: &TestEnv) -> String {
    let out = Command::new("sh")
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .args(["-c", "command -v relay || echo none"])
        .output()
        .unwrap();
    assert!(out.status.success(), "sh failed: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn calls_file(env: &TestEnv) -> PathBuf {
    env.home.path().join("relay-calls")
}

fn call_lines(env: &TestEnv) -> Vec<String> {
    match std::fs::read_to_string(calls_file(env)) {
        Ok(text) => text.lines().map(ToString::to_string).collect(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("read relay-calls: {err}"),
    }
}

fn install_canned_relay(env: &TestEnv, body: &str, exit_code: i32) {
    let calls = calls_file(env);
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nprintf '%s\\n' '{}'\nexit {exit_code}\n",
        calls.display(),
        body.replace('\'', "'\\''")
    );
    env.fake_relay(&script);
}

fn codex_cmd(env: &TestEnv, dir: &Path, poisoned: bool) -> assert_cmd::Command {
    let mut cmd = env.cmd(dir);
    cmd.env("CODEX_THREAD_ID", THREAD_ID);
    if poisoned {
        cmd.env("TASKS_MODEL", "poison-model")
            .env("TASKS_AGENT", "poison-agent");
    }
    cmd
}

fn run_json(mut cmd: assert_cmd::Command) -> (serde_json::Value, String, String) {
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "command failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("invalid json: {e}\nstdout: {stdout}"));
    (json, stdout, stderr)
}

fn assert_no_poison(env: &TestEnv, dir: &Path, id: &str, raw_out: &str, raw_err: &str) {
    for needle in ["poison-model", "poison-agent"] {
        assert!(
            !raw_out.contains(needle),
            "stdout leaked {needle}: {raw_out}"
        );
        assert!(
            !raw_err.contains(needle),
            "stderr leaked {needle}: {raw_err}"
        );
        let shown = env.json(dir, &["show", id]).to_string();
        assert!(
            !shown.contains(needle),
            "shown record leaked {needle}: {shown}"
        );
        let file_text =
            std::fs::read_to_string(dir.join("tasks").join(format!("{id}.md"))).unwrap();
        assert!(
            !file_text.contains(needle),
            "task file leaked {needle}: {file_text}"
        );
    }
}

fn prepare_two_close_migration(env: &mut TestEnv) -> (PathBuf, String, String, PathBuf) {
    let forge = env.init("forge");
    env.init("forge2");
    let a = env.json(&forge, &["add", "Close A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&forge, &["add", "Close B"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let list = env.home.path().join("list.toml");
    env.json(
        &forge,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ],
    );
    let draft = std::fs::read_to_string(&list).unwrap();
    let decided = draft.replace(
        "decision = \"\"",
        "decision = \"close\"\nreason = \"finished elsewhere\"",
    );
    std::fs::write(&list, decided).unwrap();
    (forge, a, b, list)
}

struct PidCleanupGuard {
    pid_file: PathBuf,
}

impl PidCleanupGuard {
    fn kill_now(&self) {
        if let Ok(text) = std::fs::read_to_string(&self.pid_file)
            && let Ok(pid) = text.trim().parse::<i32>()
            && pid > 0
        {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

impl Drop for PidCleanupGuard {
    fn drop(&mut self) {
        self.kill_now();
    }
}

fn process_alive(pid: i32) -> bool {
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[test]
fn host_relay_is_not_on_test_path() {
    let env = TestEnv::new();
    assert_eq!(resolve_relay_on_test_path(&env), "none");

    let script = env.fake_relay("#!/bin/sh\necho fake\n");
    assert_eq!(script, env.fake_bin().join("relay"));
    assert_eq!(
        resolve_relay_on_test_path(&env),
        script.display().to_string()
    );
}

#[test]
fn no_thread_no_call() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut add = env.cmd(&dir);
    add.env("TASKS_AGENT", "claude-code/m")
        .env("TASKS_MODEL", "m")
        .args(["add", "No thread"]);
    let (added, _, _) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = env.cmd(&dir);
    done.env("TASKS_AGENT", "claude-code/m")
        .env("TASKS_MODEL", "m")
        .args(["done", &id, "landed"]);
    let (closed, _, _) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "claude-code/m");
    assert_eq!(task["model"], "m");
    assert!(!calls_file(&env).exists());
}

#[test]
fn empty_thread_no_call() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut add = env.cmd(&dir);
    add.env("CODEX_THREAD_ID", "")
        .env("TASKS_AGENT", "claude-code/m")
        .env("TASKS_MODEL", "m")
        .args(["add", "Empty thread"]);
    let (added, _, _) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = env.cmd(&dir);
    done.env("CODEX_THREAD_ID", "")
        .env("TASKS_AGENT", "claude-code/m")
        .env("TASKS_MODEL", "m")
        .args(["done", &id, "landed"]);
    let (closed, _, _) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "claude-code/m");
    assert_eq!(task["model"], "m");
    assert!(!calls_file(&env).exists());
}

#[test]
fn explicit_agent_no_call() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Explicit", "--agent", "codex/x"]);
    let (added, _, _) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut show = codex_cmd(&env, &dir, true);
    show.args(["show", &id]);
    let (shown, _, _) = run_json(show);
    assert_eq!(shown["task"]["agent"], "codex/x");
    assert!(!calls_file(&env).exists());
}

#[test]
fn verified_with_effort() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Verified effort"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex/mock-model@low");
    assert_eq!(task["model"], "mock-model");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
    assert_eq!(
        call_lines(&env),
        vec![
            "model --harness codex --json",
            "model --harness codex --json"
        ]
    );
}

#[test]
fn verified_without_effort() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITHOUT_EFFORT, 0);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Verified no effort"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex/mock-model");
    assert_eq!(task["model"], "mock-model");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn other_harness_matching() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"claude-code"}}"#,
        0,
    );

    let mut add = codex_cmd(&env, &dir, false);
    add.env("TASKS_AGENT", "claude-code/claude-x")
        .env("TASKS_MODEL", "claude-x")
        .args(["add", "Claude under Codex"]);
    let (added, _, _) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, false);
    done.env("TASKS_AGENT", "claude-code/claude-x")
        .env("TASKS_MODEL", "claude-x")
        .args(["done", &id, "landed"]);
    let (closed, _, _) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "claude-code/claude-x");
    assert_eq!(task["model"], "claude-x");
    assert_eq!(call_lines(&env).len(), 2);
}

#[test]
fn other_harness_matching_with_non_unicode_agent_suffix() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"claude-code"}}"#,
        0,
    );
    let agent = OsStr::from_bytes(b"claude-code/\xff");
    let seed = env.json(&dir, &["add", "Seed"]);
    let id = seed["id"].as_str().unwrap();

    let mut done = codex_cmd(&env, &dir, false);
    done.env("TASKS_AGENT", agent)
        .env("TASKS_MODEL", "claude-x")
        .args(["done", id, "landed"]);
    let (closed, _, _) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));
    assert_eq!(env.json(&dir, &["show", id])["task"]["model"], "claude-x");

    let mut add = codex_cmd(&env, &dir, false);
    let output = add
        .env("TASKS_AGENT", agent)
        .args(["add", "Malformed agent"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let failure: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(failure["error"]["kind"], "validation");
    assert_eq!(
        failure["error"]["detail"],
        "TASKS_AGENT is not valid Unicode"
    );
}

#[test]
fn other_harness_absent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"claude-code"}}"#,
        0,
    );

    let mut add = codex_cmd(&env, &dir, false);
    add.env("TASKS_MODEL", "poison-model")
        .args(["add", "Claude absent env"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model unverified: other-harness"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, false);
    done.env("TASKS_MODEL", "poison-model")
        .args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model unverified: other-harness"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "claude-code");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn other_harness_mismatched() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"agy"}}"#,
        0,
    );

    let mut add = codex_cmd(&env, &dir, false);
    add.env("TASKS_AGENT", "codex")
        .env("TASKS_MODEL", "poison-model")
        .args(["add", "Agy from Codex"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model unverified: other-harness"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, false);
    done.env("TASKS_AGENT", "codex")
        .env("TASKS_MODEL", "poison-model")
        .args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model unverified: other-harness"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "agy");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn no_harness() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":null}}"#,
        0,
    );

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Reparented"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model unverified: no-harness"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model unverified: no-harness"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn unverified_reasons() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    for reason in [
        "no-rollout",
        "ambiguous-rollout",
        "no-turn-context",
        "scan-limit",
        "unbound-call",
        "earlier-turn",
        "turn-boundary",
        "unstampable",
    ] {
        install_canned_relay(
            &env,
            &format!(
                r#"{{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"{reason}","source":{{"nearest":"codex"}}}}"#
            ),
            0,
        );
        let expected_warning = format!("model unverified: {reason}");

        let mut add = codex_cmd(&env, &dir, true);
        add.args(["add", &format!("Reason {reason}")]);
        let (added, out_a, err_a) = run_json(add);
        assert_eq!(added["warnings"], serde_json::json!([expected_warning]));
        let id = added["id"].as_str().unwrap().to_string();

        let mut done = codex_cmd(&env, &dir, true);
        done.args(["done", &id, "landed"]);
        let (closed, out_d, err_d) = run_json(done);
        assert_eq!(closed["warnings"], serde_json::json!([expected_warning]));

        let task = &env.json(&dir, &["show", &id])["task"];
        assert_eq!(task["agent"], "codex");
        assert!(task.get("model").is_none(), "{task}");
        assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
    }
}

#[test]
fn exit_one() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 1);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Exit one"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver failed"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model resolver failed"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn exit_two() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 2);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Exit two"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver failed"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model resolver failed"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn missing_relay() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Missing relay"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver missing"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model resolver missing"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}

#[test]
fn invalid_answers() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let oversize = format!(
        r#"{{"verified":true,"harness":"codex","agent":"codex/m","model":"m","effort":null,"reason":null,"source":{{"nearest":"codex","pad":"{}"}}}}"#,
        "a".repeat(70 * 1024)
    );
    let cases: Vec<(&str, String)> = vec![
        (
            "wrong harness",
            r#"{"verified":true,"harness":"agy","agent":"codex/m","model":"m","effort":null,"reason":null,"source":{"nearest":"codex"}}"#.into(),
        ),
        (
            "agent mismatch",
            r#"{"verified":true,"harness":"codex","agent":"codex/other","model":"m","effort":null,"reason":null,"source":{"nearest":"codex"}}"#.into(),
        ),
        (
            "unknown reason",
            r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"sideways","source":{"nearest":"codex"}}"#.into(),
        ),
        ("non-json", "not json at all".into()),
        (
            "nearest missing",
            r#"{"verified":true,"harness":"codex","agent":"codex/m","model":"m","effort":null,"reason":null,"source":{}}"#.into(),
        ),
        (
            "nearest pi",
            r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"pi"}}"#.into(),
        ),
        (
            "verified claude-code",
            r#"{"verified":true,"harness":"codex","agent":"codex/m","model":"m","effort":null,"reason":null,"source":{"nearest":"claude-code"}}"#.into(),
        ),
        (
            "not-under-codex codex",
            r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"codex"}}"#.into(),
        ),
        ("70 KiB valid-prefix json", oversize),
    ];

    for (idx, (label, payload)) in cases.iter().enumerate() {
        let payload_file = env.home.path().join(format!("payload-{idx}.txt"));
        std::fs::write(&payload_file, payload).unwrap();
        let script = format!("#!/bin/sh\ncat '{}'\nexit 0\n", payload_file.display());
        env.fake_relay(&script);

        let mut add = codex_cmd(&env, &dir, true);
        add.args(["add", &format!("Invalid {idx}")]);
        let (added, out_a, err_a) = run_json(add);
        assert_eq!(
            added["warnings"],
            serde_json::json!(["model resolver failed"]),
            "case {label}"
        );
        let id = added["id"].as_str().unwrap().to_string();

        let mut done = codex_cmd(&env, &dir, true);
        done.args(["done", &id, "landed"]);
        let (closed, out_d, err_d) = run_json(done);
        assert_eq!(
            closed["warnings"],
            serde_json::json!(["model resolver failed"]),
            "case {label}"
        );

        let task = &env.json(&dir, &["show", &id])["task"];
        assert_eq!(task["agent"], "codex", "case {label}");
        assert!(task.get("model").is_none(), "case {label}: {task}");
        assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
    }
}

#[test]
fn oversize_is_prompt() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let pid_file = env.home.path().join("oversize-pid");
    let payload_file = env.home.path().join("oversize.bin");
    std::fs::write(&payload_file, vec![b'a'; 70 * 1024]).unwrap();
    let _guard = PidCleanupGuard {
        pid_file: pid_file.clone(),
    };
    let script = format!(
        "#!/bin/sh\necho $$ > '{}'\ncat '{}'\nexec sleep 10\n",
        pid_file.display(),
        payload_file.display()
    );
    env.fake_relay(&script);

    let started = Instant::now();
    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Oversize prompt"]);
    let (added, _, _) = run_json(add);
    let elapsed = started.elapsed();

    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver failed"])
    );
    assert!(
        elapsed < Duration::from_millis(1500),
        "oversize resolver took {elapsed:?}, expected < 1.5s"
    );
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(!process_alive(pid), "oversize child {pid} survived");
}

#[test]
fn timeout_kills_child() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let pid_file = env.home.path().join("pid");
    let _guard = PidCleanupGuard {
        pid_file: pid_file.clone(),
    };
    let script = format!(
        "#!/bin/sh\necho $$ > '{}'\nexec sleep 10\n",
        pid_file.display()
    );
    env.fake_relay(&script);

    let started = Instant::now();
    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Timeout child"]);
    let (added, _, _) = run_json(add);
    let elapsed = started.elapsed();

    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver timeout"])
    );
    assert!(
        elapsed >= Duration::from_secs(2) && elapsed < Duration::from_millis(3500),
        "timeout elapsed {elapsed:?} outside [2s, 3.5s)"
    );
    let id = added["id"].as_str().unwrap();
    assert_eq!(env.json(&dir, &["show", id])["task"]["agent"], "codex");

    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(!process_alive(pid), "timed-out child {pid} survived");
}

#[test]
fn grandchild_holds_stdout() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let grandchild_pid_file = env.home.path().join("grandchild-pid");
    let guard = PidCleanupGuard {
        pid_file: grandchild_pid_file.clone(),
    };
    let script = format!(
        "#!/bin/sh\nsleep 10 &\necho $! > '{}'\nprintf '%s\\n' '{}'\nexit 0\n",
        grandchild_pid_file.display(),
        VERIFIED_WITH_EFFORT.replace('\'', "'\\''")
    );
    env.fake_relay(&script);

    let started = Instant::now();
    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Grandchild stdout"]);
    let (added, _, _) = run_json(add);
    let elapsed = started.elapsed();

    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver timeout"])
    );
    assert!(
        elapsed >= Duration::from_secs(2) && elapsed < Duration::from_millis(3500),
        "grandchild timeout elapsed {elapsed:?} outside [2s, 3.5s)"
    );
    let id = added["id"].as_str().unwrap();
    let task = &env.json(&dir, &["show", id])["task"];
    assert_eq!(task["agent"], "codex");

    let gpid: i32 = std::fs::read_to_string(&grandchild_pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    drop(guard);
    for _ in 0..20 {
        if !process_alive(gpid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process_alive(gpid),
        "grandchild {gpid} survived guard cleanup"
    );
}

#[test]
fn stderr_never_leaks() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let secret = "/secret/path/rollout.jsonl";
    let unbound = r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"unbound-call","source":{"nearest":"codex"}}"#;
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' '{secret}' >&2\nprintf '%s\\n' '{}'\nexit 0\n",
        unbound.replace('\'', "'\\''")
    );
    env.fake_relay(&script);

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Stderr check"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model unverified: unbound-call"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    assert!(!out_a.contains(secret), "{out_a}");
    assert!(!err_a.contains(secret), "{err_a}");
    assert_eq!(env.json(&dir, &["show", &id])["task"]["agent"], "codex");

    fn assert_tree_clean(at: &Path, secret: &str) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                assert_tree_clean(&path, secret);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                let text = String::from_utf8_lossy(&bytes);
                assert!(
                    !text.contains(secret),
                    "file {} leaked secret: {text}",
                    path.display()
                );
            }
        }
    }
    assert_tree_clean(&dir, secret);
}

#[test]
fn one_call_per_process() {
    let mut env = TestEnv::new();
    let (forge, a, b, list) = prepare_two_close_migration(&mut env);
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut migrate = codex_cmd(&env, &forge, true);
    migrate.args(["migrate", list.to_str().unwrap()]);
    let (migrated, out_m, err_m) = run_json(migrate);
    assert_eq!(migrated["warnings"], serde_json::json!([]));
    assert_eq!(call_lines(&env).len(), 1);
    assert_eq!(
        env.json(&forge, &["show", &a])["task"]["model"],
        "mock-model"
    );
    assert_eq!(
        env.json(&forge, &["show", &b])["task"]["model"],
        "mock-model"
    );
    assert!(!out_m.contains("poison") && !err_m.contains("poison"));
}

#[test]
fn one_warning_per_command() {
    let mut env = TestEnv::new();
    let (forge, a, b, list) = prepare_two_close_migration(&mut env);
    install_canned_relay(
        &env,
        r#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":null}}"#,
        0,
    );

    let mut migrate = codex_cmd(&env, &forge, true);
    migrate.args(["migrate", list.to_str().unwrap()]);
    let (migrated, _, _) = run_json(migrate);
    assert_eq!(call_lines(&env).len(), 1);
    assert_eq!(
        migrated["warnings"],
        serde_json::json!(["model unverified: no-harness"])
    );
    assert!(
        env.json(&forge, &["show", &a])["task"]
            .get("model")
            .is_none()
    );
    assert!(
        env.json(&forge, &["show", &b])["task"]
            .get("model")
            .is_none()
    );
}

#[test]
fn migrate_explain_no_call() {
    let mut env = TestEnv::new();
    let (forge, _, _, list) = prepare_two_close_migration(&mut env);
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut explain = codex_cmd(&env, &forge, true);
    explain.args(["migrate", list.to_str().unwrap(), "--explain"]);
    let (preview, _, _) = run_json(explain);
    assert_eq!(preview["recovery"], "fresh");
    assert_eq!(preview["warnings"], serde_json::json!([]));
    assert!(!calls_file(&env).exists());
}

#[test]
fn non_unicode_env_ignored() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut add = env.cmd(&dir);
    add.env("CODEX_THREAD_ID", THREAD_ID)
        .env("TASKS_MODEL", OsStr::from_bytes(b"\xff\xfe"))
        .env("TASKS_AGENT", OsStr::from_bytes(b"\xff\xfe"))
        .args(["add", "Non unicode"]);
    let (added, _, _) = run_json(add);
    assert_eq!(added["warnings"], serde_json::json!([]));
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = env.cmd(&dir);
    done.env("CODEX_THREAD_ID", THREAD_ID)
        .env("TASKS_MODEL", OsStr::from_bytes(b"\xff\xfe"))
        .env("TASKS_AGENT", OsStr::from_bytes(b"\xff\xfe"))
        .args(["done", &id, "landed"]);
    let (closed, _, _) = run_json(done);
    assert_eq!(closed["warnings"], serde_json::json!([]));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex/mock-model@low");
    assert_eq!(task["model"], "mock-model");
}

#[test]
fn feedback_stamps() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(&env, VERIFIED_WITH_EFFORT, 0);

    let mut fb = codex_cmd(&env, &dir, true);
    fb.args([
        "feedback",
        "--project",
        "sci",
        "Tool observation",
        "--category",
        "friction",
    ]);
    let (filed, out_f, err_f) = run_json(fb);
    let id = filed["id"].as_str().unwrap().to_string();
    let warnings = filed["warnings"].as_array().unwrap();
    assert_eq!(
        warnings.len(),
        1,
        "only the filed-as notice expected: {warnings:?}"
    );
    assert!(warnings[0].as_str().unwrap().contains("filed as"));

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex/mock-model@low");
    assert_no_poison(&env, &dir, &id, &out_f, &err_f);
}

#[test]
fn base_relay_degrades() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    install_canned_relay(
        &env,
        r#"{"verified":true,"agent":"codex/m","harness":"codex","model":"m","effort":null,"effortReason":"not-recorded","reason":null,"source":{"thread":"t","rollout":"r"}}"#,
        0,
    );

    let mut add = codex_cmd(&env, &dir, true);
    add.args(["add", "Base relay"]);
    let (added, out_a, err_a) = run_json(add);
    assert_eq!(
        added["warnings"],
        serde_json::json!(["model resolver failed"])
    );
    let id = added["id"].as_str().unwrap().to_string();

    let mut done = codex_cmd(&env, &dir, true);
    done.args(["done", &id, "landed"]);
    let (closed, out_d, err_d) = run_json(done);
    assert_eq!(
        closed["warnings"],
        serde_json::json!(["model resolver failed"])
    );

    let task = &env.json(&dir, &["show", &id])["task"];
    assert_eq!(task["agent"], "codex");
    assert!(task.get("model").is_none(), "{task}");
    assert_no_poison(&env, &dir, &id, &(out_a + &out_d), &(err_a + &err_d));
}
