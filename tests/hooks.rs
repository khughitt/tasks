mod common;
use common::hooks::{claim_root, envelope, facts_view, guard, install_facts};
use common::{TestEnv, write_claim};
use serde_json::json;

#[test]
fn hooks_entry_is_independent() {
    for harness in ["codex", "claude-code", "agy"] {
        for held in [false, true] {
            let mut env = TestEnv::new();
            let root = env.init("sci");
            if held {
                write_claim(
                    &env,
                    "sci",
                    "sci-000001",
                    &format!("{harness}:controller"),
                    true,
                );
            }
            let event = envelope(harness, "Stop", "controller", &root);
            std::fs::write(
                env.home.path().join(".config/tasks/config.toml"),
                "not [toml",
            )
            .unwrap();
            for flags in [
                vec![],
                vec![
                    "--json",
                    "hook",
                    "claim-guard",
                    "--pretty",
                    "--color",
                    "bad",
                ],
                vec![
                    "--headers",
                    "hook",
                    "--no-headers",
                    "claim-guard",
                    "-C",
                    "missing",
                ],
                vec!["-Cmissing", "--color=bad", "hook", "claim-guard"],
                vec![
                    "--json",
                    "--json",
                    "hook",
                    "claim-guard",
                    "--pretty",
                    "--pretty",
                    "--color=bad",
                    "--color=worse",
                    "-Cfirst",
                    "-Csecond",
                ],
            ] {
                let flags = if flags.is_empty() {
                    vec!["hook", "claim-guard"]
                } else {
                    flags
                };
                let out = env
                    .cmd(&root)
                    .args(flags)
                    .env("TASKS_FORMAT", "bad")
                    .env("TASKS_COLOR", "bad")
                    .env("TASKS_HEADERS", "bad")
                    .env("TASKS_PALETTE", "bad")
                    .env("TASKS_THEME", "bad")
                    .env("COLUMNS", "bad")
                    .write_stdin(event.to_string())
                    .output()
                    .unwrap();
                assert_eq!(
                    out.status.code(),
                    Some(0),
                    "{harness} {held}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                let reply: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(reply["decision"], if held { "deny" } else { "allow" });
            }
            let out = env
                .cmd(&root)
                .args(["hook", "claim-guard"])
                .env("TASKS_COMPLETE", "bash")
                .write_stdin(event.to_string())
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0));
            let reply: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(reply["decision"], if held { "deny" } else { "allow" });
            let ordinary = env.cmd(&root).arg("claims").output().unwrap();
            assert_eq!(ordinary.status.code(), Some(1));
            assert!(ordinary.stdout.is_empty());
        }
    }
}

#[test]
fn hooks_routing_consumes_global_value_slots() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let event = envelope("codex", "Stop", "controller", &root);
    for args in [
        vec!["-C", "hook", "hook", "claim-guard"],
        vec!["--color", "hook", "hook", "claim-guard"],
    ] {
        let out = env
            .cmd(&root)
            .args(args)
            .write_stdin(event.to_string())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["decision"],
            "allow"
        );
    }
    let out = env
        .cmd(&root)
        .args(["--color", "hook", "claims"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let out = env.cmd(&root).args(["hook", "--help"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("claim-guard")
    );
    for args in [
        vec!["hook", "missing"],
        vec!["hook", "claim-guard", "--unknown"],
        vec!["hook", "claim-guard", "-C"],
    ] {
        let out = env.cmd(&root).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn hooks_claim_inventory_is_complete() {
    let mut env = TestEnv::new();
    let root = env.init("one");
    env.init("two");
    write_claim(&env, "one", "one-000001", "claude-code:controller", true);
    std::fs::write(env.claim_store("two"), "not [toml").unwrap();
    let mut event = envelope("claude-code", "Stop", "controller", &root);
    event["event"]["private"]["backgroundTasks"] =
        json!({"status":"valid","value":[{"type":"subagent","status":"running"}]});
    let marker = install_facts(
        &env,
        &facts_view(
            "claude-code",
            true,
            &["shell", "subagent"],
            &["running", "pending"],
        ),
    );
    let reply = guard(&env, &root, &event);
    assert_eq!(reply["decision"], "deny");
    assert!(
        reply["reason"]
            .as_str()
            .unwrap()
            .contains("claims could not be read")
    );
    assert!(!marker.exists());
}

#[test]
fn hooks_codex_worker_matrix() {
    for native in ["Stop", "SubagentStop"] {
        for (root_held, worker_held) in [(true, false), (false, true), (true, true)] {
            let mut env = TestEnv::new();
            let root = env.init("sci");
            if root_held {
                write_claim(&env, "sci", "sci-000001", "codex:root", true);
            }
            if worker_held {
                write_claim(&env, "sci", "sci-000002", "codex:worker", true);
                claim_root(&env, "sci", "sci-000002", "codex:root");
            }
            let marker = install_facts(
                &env,
                &facts_view("codex", true, &["shell", "subagent"], &["running"]),
            );
            let mut event = envelope("codex", native, "worker", &root);
            event["event"]["identity"]["value"]["root"] = json!("codex:root");
            let reply = guard(&env, &root, &event);
            assert_eq!(
                reply["decision"],
                if worker_held { "deny" } else { "allow" }
            );
            if worker_held {
                let reason = reply["reason"].as_str().unwrap();
                assert!(reason.contains("sci-000002"));
                assert!(!reason.contains("sci-000001"));
                assert!(reason.contains("If you are a subagent"));
            }
            assert!(!marker.exists());
        }
    }
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let event = envelope("codex", "SubagentStop", "worker", &root);
    let reply = guard(&env, &root, &event);
    assert_eq!(reply["decision"], "deny");
    assert!(reply["reason"].as_str().unwrap().contains("root"));
}

#[test]
fn hooks_agy_exemption_matrix() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        "not [toml",
    )
    .unwrap();
    for reason in [
        "ERROR",
        "USER_CANCELED",
        "MAX_INVOCATIONS",
        "MAX_FORCED_INVOCATIONS",
        "MAX_TOKEN_BUDGET_EXCEEDED",
        "TERMINAL_CUSTOM_HOOK",
    ] {
        for count in [
            json!({"status":"missing"}),
            json!({"status":"invalid"}),
            json!({"status":"valid","value":"bad"}),
        ] {
            let mut event = envelope("agy", "Stop", "controller", &root);
            event["event"]["facts"]["terminationReason"]["value"] = json!(reason);
            event["event"]["facts"]["executionNum"] = count;
            event["event"]["identity"] = json!({"status":"invalid"});
            assert_eq!(guard(&env, &root, &event)["decision"], "allow");
        }
    }
    for harness in ["agy", "codex", "claude-code"] {
        let mut event = envelope(harness, "Stop", "controller", &root);
        event["event"]["identity"] = json!({"status":"missing"});
        if harness == "agy" {
            event["event"]["facts"]["terminationReason"] = json!({"status":"invalid"});
            event["event"]["facts"]["executionNum"] = json!({"status":"valid","value":1});
        } else {
            event["event"]["facts"]["stopHookActive"] = json!({"status":"valid","value":true});
        }
        assert_eq!(guard(&env, &root, &event)["decision"], "allow");
    }
}
