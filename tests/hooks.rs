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

#[test]
fn hooks_halt_authority_matrix() {
    use common::hooks::{fingerprint, notice, repository};
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let halt = common::id_of(env.json(&root, &["add", "Stop work", "--tag", "halt"]));
    let paths = vec![
        root.join("tasks"),
        env.home.path().join(".config/tasks/projects.toml"),
        env.home.path().join(".local/state/tasks"),
    ];
    write_claim(&env, "sci", "sci-000001", "codex:controller", true);
    let before = fingerprint(&paths);
    assert!(
        notice(&env, &root, &root)["context"]
            .as_str()
            .unwrap()
            .contains(&halt)
    );
    assert_eq!(before, fingerprint(&paths));
    let other = tempfile::tempdir().unwrap();
    assert_eq!(notice(&env, &root, other.path()), json!({}));
    for cwd in [root.join("missing"), root.join("tasks/.config.toml")] {
        assert!(
            notice(&env, &root, &cwd)["context"]
                .as_str()
                .unwrap()
                .contains("cannot tell which project")
        );
    }
    let config = root.join("tasks/.config.toml");
    let original = std::fs::read(&config).unwrap();
    for broken in ["broken [toml", "[git]\ncommit=false\n"] {
        std::fs::write(&config, broken).unwrap();
        assert!(
            notice(&env, &root, &root)["context"]
                .as_str()
                .unwrap()
                .contains("cannot tell which project")
        );
    }
    std::fs::write(&config, original).unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let saved = std::fs::read(&registry).unwrap();
    std::fs::write(&registry, "[projects]\n").unwrap();
    for in_git in [false, true] {
        if in_git {
            repository(&env, &root);
        }
        let text = notice(&env, &root, &root)["context"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            text.contains("unregistered") && text.contains("sci") && text.contains("tasks where"),
            "{text}"
        );
    }
    std::fs::write(&registry, &saved).unwrap();
    std::fs::write(root.join(format!("tasks/{halt}.md")), "corrupt task").unwrap();
    let out = env
        .cmd(&root)
        .args(["hook", "halt-notice"])
        .write_stdin(envelope("codex", "SessionStart", "c", &root).to_string())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

#[test]
fn hooks_halt_registered_git_authorities_and_historical_tree() {
    use common::hooks::{notice, repository};
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let code = base.path().join("code");
    let records = base.path().join("records");
    repository(&env, &code);
    repository(&env, &records);
    env.json(
        &code,
        &[
            "init",
            "--prefix",
            "det",
            "--records",
            records.to_str().unwrap(),
        ],
    );
    let authority = records.join("det");
    let halt = common::id_of(env.json(&authority, &["add", "Detached halt", "--tag", "halt"]));
    let worktree = base.path().join("worktree");
    common::git(
        &code,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            worktree.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &code, &worktree);
    for dir in [&code, &worktree, &authority] {
        assert!(
            notice(&env, &code, dir)["context"]
                .as_str()
                .unwrap()
                .contains(&halt)
        );
    }
    std::fs::create_dir_all(worktree.join("tasks")).unwrap();
    std::fs::write(worktree.join("tasks/.config.toml"), "prefix='det'\n").unwrap();
    assert!(
        notice(&env, &code, &worktree)["context"]
            .as_str()
            .unwrap()
            .contains(&halt)
    );
    // Ordinary commands retain records_moved; the subscriber follows verified authority.
    assert_eq!(env.fail(&worktree, &["list"]), "records_moved");
    let foreign = base.path().join("foreign");
    repository(&env, &foreign);
    std::fs::create_dir_all(foreign.join("tasks")).unwrap();
    std::fs::write(foreign.join("tasks/.config.toml"), "prefix='det'\n").unwrap();
    assert!(
        notice(&env, &code, &foreign)["context"]
            .as_str()
            .unwrap()
            .contains("cannot tell which project")
    );
    std::fs::write(
        authority.join("tasks/.config.toml"),
        "prefix='det'\n[checkout]\nkind='detached'\npath='../../foreign'\n",
    )
    .unwrap();
    assert!(
        notice(&env, &code, &code)["context"]
            .as_str()
            .unwrap()
            .contains("cannot tell which project")
    );
}

#[test]
fn hooks_halt_linked_and_in_tree_worktrees_use_registered_records() {
    use common::hooks::{notice, repository};
    let mut env = TestEnv::new();
    let authority = env.init("lnk");
    let base = tempfile::tempdir().unwrap();
    let code = base.path().join("code");
    repository(&env, &code);
    let config = authority.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "\n[checkout]\npath={:?}\n",
        code.to_str().unwrap()
    ));
    std::fs::write(&config, text).unwrap();
    let halt = common::id_of(env.json(&authority, &["add", "Linked halt", "--tag", "halt"]));
    let wt = base.path().join("linked-wt");
    common::git(
        &code,
        &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
    );
    common::git_fixture::assert_worktree(env.home.path(), &code, &wt);
    for dir in [&code, &wt] {
        assert!(
            notice(&env, &authority, dir)["context"]
                .as_str()
                .unwrap()
                .contains(&halt)
        );
    }
    let root = env.init("itr");
    repository(&env, &root);
    let halt = common::id_of(env.json(&root, &["add", "In tree halt", "--tag", "halt"]));
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-qm", "records"]);
    let side = base.path().join("in-tree-wt");
    common::git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &root, &side);
    std::fs::remove_file(side.join(format!("tasks/{halt}.md"))).unwrap();
    assert!(
        notice(&env, &root, &side)["context"]
            .as_str()
            .unwrap()
            .contains(&halt)
    );
}

#[test]
fn hooks_halt_selects_oldest_open_task() {
    use common::hooks::notice;
    for status in [
        "idea", "todo", "doing", "blocked", "shelved", "done", "dropped",
    ] {
        let mut env = TestEnv::new();
        let root = env.init("sci");
        let first = common::id_of(env.json(&root, &["add", "First", "--tag", "halt"]));
        let second = common::id_of(env.json(&root, &["add", "Second", "--tag", "halt"]));
        let mut ids = [first.clone(), second.clone()];
        ids.sort();
        for id in &ids {
            let path = root.join(format!("tasks/{id}.md"));
            let text = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .map(|line| {
                    if line.starts_with("created:") {
                        "created: 2026-01-01T00:00:00Z".into()
                    } else if line.starts_with("status:") {
                        format!("status: {}", if *id == ids[0] { status } else { "todo" })
                    } else {
                        line.into()
                    }
                })
                .collect::<Vec<String>>()
                .join("\n");
            std::fs::write(path, text).unwrap();
        }
        let expected = if matches!(status, "done" | "dropped") {
            &ids[1]
        } else {
            &ids[0]
        };
        assert!(
            notice(&env, &root, &root)["context"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{status}"
        );
    }
}

#[test]
fn hooks_halt_without_local_config_uses_registered_authority() {
    use common::hooks::{notice, repository};
    let mut failures = Vec::new();
    for nested in [false, true] {
        let env = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let main = base.path().join("main");
        repository(&env, &main);
        let project = if nested {
            main.join("nested")
        } else {
            main.clone()
        };
        std::fs::create_dir_all(&project).unwrap();
        env.json(&project, &["init", "--prefix", "sci"]);
        let halt = common::id_of(env.json(&project, &["add", "Registered halt", "--tag", "halt"]));
        common::git(&main, &["add", "-A"]);
        common::git(&main, &["commit", "-qm", "records"]);
        let wt = base.path().join("side");
        common::git(
            &main,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        common::git_fixture::assert_worktree(env.home.path(), &main, &wt);
        let cwd = if nested { wt.join("nested") } else { wt };
        std::fs::remove_dir_all(cwd.join("tasks")).unwrap();
        assert!(env.json(&cwd, &["where"])["project"].is_null());
        let reply = notice(&env, &main, &cwd);
        if !reply["context"].as_str().is_some_and(|s| s.contains(&halt)) {
            failures.push((nested, reply));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn hooks_halt_title_is_visibly_single_line() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let title = "Greek β\u{000b}\u{000c}\u{001b}[31m\u{0085}\u{2028}\u{2029}end";
    let id = common::id_of(env.json(&root, &["add", title, "--tag", "halt"]));
    let reply = common::hooks::notice(&env, &root, &root);
    let context = reply["context"].as_str().unwrap();
    assert!(context.contains("Greek β"));
    assert!(context.contains(&id));
    let text = context.strip_suffix('\n').unwrap();
    assert!(
        !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')),
        "{context:?}"
    );
    assert!(text.contains("\\u{b}") && text.contains("\\u{1b}") && text.contains("\\u{2028}"));
    assert!(text.contains("tasks start <id> --force --reason \"<why>\""));
}

#[test]
fn hooks_halt_config_free_lookup_respects_git_boundaries_and_ambiguity() {
    use common::hooks::{notice, repository};
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let main = base.path().join("main");
    repository(&env, &main);
    env.json(&main, &["init", "--prefix", "sci"]);
    let halt = common::id_of(env.json(&main, &["add", "Main halt", "--tag", "halt"]));
    common::git(&main, &["add", "-A"]);
    common::git(&main, &["commit", "-qm", "records"]);
    let side = base.path().join("side");
    common::git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    std::fs::remove_dir_all(side.join("tasks")).unwrap();
    let nested = main.join("nested");
    repository(&env, &nested);
    env.json(&nested, &["init", "--prefix", "other"]);
    env.json(&nested, &["add", "Nested repository halt", "--tag", "halt"]);
    let cwd = side.join("nested");
    std::fs::create_dir(&cwd).unwrap();
    let context = notice(&env, &main, &cwd)["context"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(context.contains(&halt));
    assert!(!context.contains("Nested repository halt"));
    let file = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let root = registry["projects"]["sci"].clone();
    registry["projects"]
        .as_table_mut()
        .unwrap()
        .insert("duplicate".into(), root);
    std::fs::write(file, toml::to_string(&registry).unwrap()).unwrap();
    let context = notice(&env, &main, &side)["context"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(context.contains("cannot tell which project") && context.contains("ambiguous"));
}

#[test]
fn hooks_notice_ignores_display_and_global_dir() {
    use common::hooks::notice;
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let other = env.init("other");
    let halt = common::id_of(env.json(&root, &["add", "Stop work", "--tag", "halt"]));
    std::fs::write(
        env.home.path().join(".config/tasks/config.toml"),
        "broken [toml",
    )
    .unwrap();
    for h in ["codex", "claude-code", "agy"] {
        let out = env
            .cmd(&other)
            .args([
                "--json",
                "hook",
                "halt-notice",
                "--pretty",
                "--headers",
                "--no-headers",
                "-Cmissing",
                "--color=bad",
            ])
            .env("TASKS_FORMAT", "bad")
            .env("TASKS_COLOR", "bad")
            .env("TASKS_HEADERS", "bad")
            .env("TASKS_THEME", "bad")
            .env("TASKS_PALETTE", "bad")
            .env("COLUMNS", "bad")
            .write_stdin(envelope(h, "SessionStart", "controller", &root).to_string())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert!(
            serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["context"]
                .as_str()
                .unwrap()
                .contains(&halt)
        );
    }
    // Enter an owned cwd that the child removes before exec: current_dir() must never be needed.
    let gone = root.join("gone");
    std::fs::create_dir(&gone).unwrap();
    let mut cmd = env.raw(&gone);
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::CommandExt;
    let gone_path = std::ffi::CString::new(gone.as_os_str().as_bytes()).unwrap();
    unsafe {
        cmd.pre_exec(move || {
            if libc::rmdir(gone_path.as_ptr()) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    cmd.args(["hook", "halt-notice"])
        .stdin(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            envelope("codex", "SessionStart", "c", &root)
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["context"]
            .as_str()
            .unwrap()
            .contains(&halt)
    );
    // Separate valid event cwd also works from every unrelated existing process directory.
    assert!(
        notice(&env, &other, &root)["context"]
            .as_str()
            .unwrap()
            .contains(&halt)
    );
}

#[test]
fn hooks_notice_discovery_avoids_records_admission_commands() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let root = env.init("sci");
    common::hooks::repository(&env, &root);
    let halt = common::id_of(env.json(&root, &["add", "Halt", "--tag", "halt"]));
    let bin = env.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .unwrap()
        .canonicalize()
        .unwrap();
    let wrapper = bin.join("git");
    let calls = env.home.path().join("git-calls");
    std::fs::write(&wrapper,format!("#!/usr/bin/env python3\nimport os,sys\nwith open({:?},'a') as f: f.write(' '.join(sys.argv[1:])+'\\n')\nif 'ls-tree' in sys.argv or ('worktree' in sys.argv and 'list' in sys.argv): sys.exit(88)\nos.execv({:?}, ['git',*sys.argv[1:]])\n",calls.to_str().unwrap(),git.to_str().unwrap())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = env
        .cmd(&root)
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .args(["hook", "halt-notice"])
        .write_stdin(envelope("codex", "SessionStart", "c", &root).to_string())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(String::from_utf8(out.stdout).unwrap().contains(&halt));
    let calls = std::fs::read_to_string(calls).unwrap();
    assert!(!calls.contains("ls-tree"));
    assert!(!calls.contains("worktree list"));
}

#[test]
fn hooks_replay_source_integrity() {
    assert_eq!(common::hooks::replay("integrity")["verified_sources"], 16);
}
#[test]
fn hooks_replay_smallest() {
    assert_eq!(common::hooks::replay("smallest")["count"], 5);
    let decimal = common::hooks::replay_case("agy-count-decimal-one");
    assert_eq!(decimal.native_decision, "deny");
    assert_eq!(decimal.subscriber_decision, "allow");
    assert!(
        decimal.evidence["native_text"]
            .as_str()
            .unwrap()
            .contains("1.0")
    );
    let large = common::hooks::replay_case("agy-count-above-safe-integer");
    assert_eq!(large.native_decision, "allow");
    assert_eq!(large.subscriber_decision, "deny");
    assert_eq!(
        large.evidence["envelope"]["event"]["facts"]["executionNum"]["status"],
        "invalid"
    );
}
#[test]
fn hooks_replay_authority_pilot() {
    for id in [
        "historical-codex",
        "linked-worktree-codex",
        "in-tree-worktree-codex",
        "pairing-mismatch-codex",
        "non-directory-codex",
    ] {
        let row = common::hooks::replay_case(id);
        assert_eq!(row.subscriber_decision, "context");
    }
}
#[test]
fn hooks_replay_corrective_pilot() {
    for id in [
        "in-tree-without-config-codex",
        "nested-without-config-codex",
        "title-controls-codex",
    ] {
        assert_eq!(
            common::hooks::replay_case(id).subscriber_decision,
            "context"
        );
    }
}
#[test]
fn hooks_replay_corpus() {
    assert_eq!(common::hooks::replay("corpus")["count"], 194);
}

#[test]
fn hooks_protocol_failures_are_explicit() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let event = envelope("codex", "Stop", "c", &root);
    let valid = event.to_string();
    let duplicate = valid.replacen(
        "\"schema\":2",
        "\"unknown\":{\"secret\":1,\"secret\":2},\"schema\":2",
        1,
    );
    let mut schema = event.clone();
    schema["schema"] = json!(1);
    for bytes in [
        b"secret not JSON".to_vec(),
        vec![0xff],
        duplicate.into_bytes(),
        schema.to_string().into_bytes(),
        vec![b' '; 4 * 1024 * 1024 + 1],
        b"{} {}".to_vec(),
    ] {
        for leaf in ["claim-guard", "halt-notice"] {
            let out = env
                .cmd(&root)
                .args(["hook", leaf])
                .write_stdin(bytes.clone())
                .output()
                .unwrap();
            assert!(!String::from_utf8_lossy(&out.stderr).contains("secret"));
            if leaf == "claim-guard" {
                assert_eq!(out.status.code(), Some(0));
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["decision"],
                    "deny"
                );
            } else {
                assert_eq!(out.status.code(), Some(1));
                assert!(out.stdout.is_empty());
            }
        }
    }
    let out = env
        .cmd(&root)
        .args(["hook", "halt-notice"])
        .write_stdin(valid)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    write_claim(&env, "sci", "sci-000001", "codex:c", true);
    let store = env.claim_store("sci");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&store).unwrap()).unwrap();
    doc["claims"]["sci-000001"]["worktree"] = toml::Value::String("\u{1f600}".repeat(5000));
    std::fs::write(store, toml::to_string(&doc).unwrap()).unwrap();
    let reply = guard(&env, &root, &event);
    let reason = reply["reason"].as_str().unwrap();
    assert!(reason.contains("reply limit") && reason.contains("tasks claims --all-projects"));
    assert!(reason.encode_utf16().count() <= 10000);
    env.json(&root, &["add", &"\u{1f600}".repeat(5000), "--tag", "halt"]);
    let out = env
        .cmd(&root)
        .args(["hook", "halt-notice"])
        .write_stdin(envelope("codex", "SessionStart", "c", &root).to_string())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("context exceeds")
    );
}
