//! Isolated subscriber envelopes and facts-reader fixtures.
use super::TestEnv;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn envelope(harness: &str, native_event: &str, session: &str, cwd: &Path) -> Value {
    let kind = match native_event {
        "SessionStart" => "session-start",
        "SubagentStop" => "child-stop",
        _ => "turn-end",
    };
    let facts = if harness == "agy" && native_event == "Stop" {
        json!({"terminationReason":{"status":"valid","value":"NO_TOOL_CALL"},
            "executionNum":{"status":"valid","value":0},
            "fullyIdle":{"status":"valid","value":true}})
    } else if native_event == "Stop" || native_event == "SubagentStop" {
        json!({"stopHookActive":{"status":"valid","value":false}})
    } else {
        json!({})
    };
    let private = if harness == "claude-code" && native_event == "Stop" {
        json!({"backgroundTasks":{"status":"missing"},"sessionCrons":{"status":"missing"}})
    } else {
        json!({})
    };
    json!({"schema":2,"type":"event","subscriber":"fixture",
        "event":{"schema":2,"kind":kind,"harness":harness,"nativeEvent":native_event,
            "cwd":cwd,"identity":{"status":"valid","value":{
                "agentId":format!("{harness}:{session}"),"sessionId":session}},
            "facts":facts,"private":private,"tool":null},
        "registry":{"status":"not-requested"}})
}

pub fn guard(env: &TestEnv, cwd: &Path, event: &Value) -> Value {
    let out = env
        .cmd(cwd)
        .args(["hook", "claim-guard"])
        .write_stdin(event.to_string())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    let keys = reply.as_object().unwrap().len();
    match reply["decision"].as_str().unwrap() {
        "allow" => assert_eq!(keys, 1),
        "deny" => {
            assert_eq!(keys, 2);
            assert!(reply["reason"].as_str().unwrap().contains("owner: tasks"));
        }
        other => panic!("invalid guard decision {other}"),
    }
    reply
}

pub fn facts_view(harness: &str, wake: bool, kinds: &[&str], statuses: &[&str]) -> Value {
    let mut facts = serde_json::Map::new();
    for (name, kind, value) in [
        ("controller-wake", "boolean", json!(wake)),
        ("controller-wake-kinds", "set", json!(kinds)),
        ("stop-input-in-flight-statuses", "set", json!(statuses)),
    ] {
        facts.insert(
            name.into(),
            json!({"type":kind,"harness":{harness:{
            "type":kind,"status":"probed","value":value,"fact":name,"harness":harness}}}),
        );
    }
    json!({"schema":1,"harnesses":[harness],"facts":facts})
}

pub fn install_facts(env: &TestEnv, view: &Value) -> PathBuf {
    let file = env.home.path().join(".agents/bin/harness-facts");
    let marker = env.home.path().join("facts-called");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, format!(
        "#!/usr/bin/env python3\nimport pathlib, sys\nassert sys.argv[1:] == ['list']\npathlib.Path({:?}).write_text('called')\nprint({:?})\n",
        marker.to_str().unwrap(), view.to_string())).unwrap();
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    marker
}

pub fn claim_root(env: &TestEnv, prefix: &str, id: &str, root: &str) {
    let path = env.claim_store(prefix);
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    store["claims"][id]
        .as_table_mut()
        .unwrap()
        .insert("root_session".into(), root.into());
    std::fs::write(path, toml::to_string(&store).unwrap()).unwrap();
}

pub fn notice(env: &TestEnv, process_cwd: &Path, event_cwd: &Path) -> Value {
    let event = envelope("codex", "SessionStart", "controller", event_cwd);
    let out = env
        .cmd(process_cwd)
        .args(["hook", "halt-notice"])
        .write_stdin(event.to_string())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

pub fn repository(env: &TestEnv, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let run = |args: &[&str]| {
        let out = super::git_fixture::command(env.home.path(), dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
    };
    run(&["init", "-q", "-b", "main"]);
    super::git_fixture::assert_repo(env.home.path(), dir);
    std::fs::write(dir.join("README"), "fixture\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "fixture"]);
}

pub fn fingerprint(paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    use sha2::{Digest, Sha256};
    fn walk(path: &Path, found: &mut Vec<(PathBuf, String)>) {
        if path.is_dir() {
            for item in std::fs::read_dir(path).unwrap() {
                walk(&item.unwrap().path(), found);
            }
        } else if path.is_file() {
            found.push((
                path.to_path_buf(),
                format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
            ));
        }
    }
    let mut found = Vec::new();
    for path in paths {
        walk(path, &mut found);
    }
    found.sort();
    found
}
