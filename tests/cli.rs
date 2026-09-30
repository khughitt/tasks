mod common;
use common::TestEnv;

#[test]
fn lifecycle_provenance_survives_claim_and_park_release() {
    for (source, key) in [
        ("CLAUDE_CODE_SESSION_ID", "claude-code:native-a"),
        ("CODEX_SESSION_ID", "codex:native-a"),
        ("CODEX_THREAD_ID", "codex:native-a"),
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("sci");
        let id = id_of(env.json(&dir, &["add", "Provenance"]));
        let run = |args: &[&str]| {
            let out = env
                .cmd(&dir)
                .env(source, "native-a")
                .env("CLAUDE_PID", std::process::id().to_string())
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["start", &id]);
        let first = env.json(&dir, &["show", &id]);
        let claim = &first["claim"];
        assert_eq!(claim["live"], true);
        assert_eq!(
            claim["session"], "native-a",
            "{source} is a claim identity level"
        );
        assert_eq!(first["task"]["notes"][0]["text"], "started");
        run(&["start", &id]);
        let refreshed = env.json(&dir, &["show", &id]);
        assert_eq!(refreshed["task"]["started"], first["task"]["started"]);
        assert_eq!(refreshed["claim"]["started"], claim["started"]);
        run(&["park", &id, "continue here"]);
        let parked = env.json(&dir, &["show", &id]);
        assert!(parked["claim"].is_null());
        assert!(!parked["park"].is_null());
        let earlier = parked["task"]["notes"].as_array().unwrap().clone();
        run(&["start", &id]);
        run(&["done", &id, "landed"]);
        let closed = env.json(&dir, &["show", &id]);
        assert!(closed["claim"].is_null() && closed["park"].is_null());
        let notes = closed["task"]["notes"].as_array().unwrap();
        assert_eq!(&notes[..earlier.len()], earlier);
        assert_eq!(
            notes
                .iter()
                .map(|n| n["text"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "started",
                "resumed",
                "parked (waiting on agent): continue here",
                "resumed",
                "done",
                "landed"
            ]
        );
        for note in notes {
            assert_eq!(note["harness_session"], key);
            assert_eq!(note["harness_session_source"], source);
        }
        run(&["done", &id]);
        assert_eq!(
            env.json(&dir, &["show", &id])["task"]["notes"],
            closed["task"]["notes"]
        );
    }
}

#[test]
fn a_codex_claim_outlives_the_command_that_made_it() {
    // Codex runs each command as its own session leader, so a `sid:<pid>` claim would be
    // dead the moment `start` returned. The thread id is the session instead, and the
    // claim lives by the TTL: `show` here is a later command, and so is the rival start.
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let as_codex = |thread: &str| {
        let mut cmd = env.cmd(&sci);
        cmd.env("CODEX_SESSION_ID", thread)
            .env("CODEX_THREAD_ID", thread);
        cmd
    };

    as_codex("thread-a").args(["start", &id]).assert().success();
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["claim"]["session"], "thread-a");
    assert_eq!(shown["claim"]["live"], true);
    assert!(shown["claim"]["pid"].is_null(), "{}", shown["claim"]);
    assert_eq!(
        shown["task"]["notes"][0]["harness_session"], "codex:thread-a",
        "provenance and the claim name the same thread"
    );

    let out = as_codex("thread-b").args(["start", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "claimed");
    assert!(
        err_detail(&out).contains("thread-a"),
        "{}",
        err_detail(&out)
    );

    as_codex("thread-a")
        .args(["park", &id, "continue here"])
        .assert()
        .success();
    let parked = env.json(&sci, &["show", &id]);
    assert_eq!(
        parked["park"]["session"], "codex:thread-a",
        "the park entry carries the provenance scheme"
    );

    let out = as_codex("thread-a")
        .env("CODEX_THREAD_ID", "thread-z")
        .args(["start", &id])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("CODEX_SESSION_ID conflicts")),
        "{warnings:?}"
    );
    assert!(
        env.json(&sci, &["show", &id])["claim"]["session"]
            .as_str()
            .unwrap()
            .starts_with("sid:"),
        "a conflict falls to the Unix session"
    );
}

#[test]
fn lifecycle_provenance_covers_editor_and_flag_transitions() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Edited", "--status", "todo"]));
    let editor = editor_script(&dir, "sed -i 's/status: todo/status: doing/' \"$1\"");
    env.cmd(&dir)
        .env("CODEX_SESSION_ID", "native-a")
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .assert()
        .success();
    env.cmd(&dir)
        .env("CODEX_SESSION_ID", "native-a")
        .args(["park", &id, "continue here"])
        .assert()
        .success();
    let parked = env.json(&dir, &["show", &id]);
    env.cmd(&dir)
        .env("CODEX_SESSION_ID", "native-a")
        .args(["edit", &id, "--status", "doing"])
        .assert()
        .success();
    let same = env.json(&dir, &["show", &id]);
    assert_eq!(same["park"], parked["park"]);
    assert_eq!(same["task"]["notes"], parked["task"]["notes"]);
    env.cmd(&dir)
        .env("CODEX_SESSION_ID", "native-a")
        .args(["edit", &id, "--status", "done"])
        .assert()
        .success();
    let shown = env.json(&dir, &["show", &id]);
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0]["text"], "started");
    assert_eq!(notes[2]["text"], "done");
    assert!(
        notes
            .iter()
            .all(|n| n["harness_session"] == "codex:native-a")
    );

    for (close, marker, every) in [
        ("drop", "dropped", false),
        ("done", "done", false),
        ("done", "completed; next due ", true),
    ] {
        let id = id_of(env.json(&dir, &["add", "Close"]));
        if every {
            env.json(&dir, &["edit", &id, "--every", "30d"]);
        }
        env.cmd(&dir)
            .env("CODEX_SESSION_ID", "native-a")
            .args([close, &id])
            .assert()
            .success();
        let shown = env.json(&dir, &["show", &id]);
        let notes = shown["task"]["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 1);
        assert!(notes[0]["text"].as_str().unwrap().starts_with(marker));
        assert_eq!(notes[0]["harness_session"], "codex:native-a");
    }
}

#[test]
fn lifecycle_provenance_conflicts_warn_without_becoming_notes() {
    for pretty in [false, true] {
        let mut env = TestEnv::new();
        let dir = env.init("sci");
        let id = id_of(env.json(&dir, &["add", "Conflict"]));
        let mut cmd = env.cmd(&dir);
        cmd.env("CODEX_SESSION_ID", "secret-a")
            .env("CODEX_THREAD_ID", "secret-b");
        if pretty {
            cmd.arg("--pretty");
        }
        let out = cmd.args(["start", &id]).output().unwrap();
        assert!(out.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            combined.contains("unknown harness provenance"),
            "{combined}"
        );
        assert!(!combined.contains("secret-a") && !combined.contains("secret-b"));
        let shown = env.json(&dir, &["show", &id]);
        let notes = shown["task"]["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0]["text"], "started");
        assert!(notes[0].get("harness_session").is_none());
        env.json(&dir, &["park", &id, "unknown still works"]);
        let shown = env.json(&dir, &["show", &id]);
        assert!(shown["task"]["notes"][1].get("harness_session").is_none());
    }
}

#[test]
fn lifecycle_provenance_does_not_change_override_claims_or_plain_notes() {
    for with_pid in [false, true] {
        let mut env = TestEnv::new();
        let dir = env.init("sci");
        let id = id_of(env.json(&dir, &["add", "Claims"]));
        let run = |args: &[&str], native: bool| {
            let mut cmd = env.cmd(&dir);
            cmd.env("TASKS_SESSION", "worker");
            if with_pid {
                cmd.env("TASKS_SESSION_PID", std::process::id().to_string());
            }
            if native {
                cmd.env("CODEX_SESSION_ID", "native-a");
            }
            cmd.args(args).assert().success();
        };
        run(&["start", &id], false);
        let before: toml::Value =
            toml::from_str(&env.read(env.home.path(), ".local/state/tasks/claims/sci.toml"))
                .unwrap();
        run(&["start", &id], true);
        let after: toml::Value =
            toml::from_str(&env.read(env.home.path(), ".local/state/tasks/claims/sci.toml"))
                .unwrap();
        assert_eq!(before["claims"][&id].get("pid").is_some(), with_pid);
        assert_eq!(before["claims"][&id].get("pid_start").is_some(), with_pid);
        for field in ["session", "pid", "pid_start", "boot_id", "started"] {
            assert_eq!(
                before["claims"][&id].get(field),
                after["claims"][&id].get(field),
                "{field}"
            );
        }
        assert_eq!(env.json(&dir, &["show", &id])["claim"]["live"], true);
        for native in [false, true] {
            run(&["note", &id, "heartbeat"], native);
        }
        let shown = env.json(&dir, &["show", &id]);
        let notes = shown["task"]["notes"].as_array().unwrap();
        assert_eq!(notes[1]["harness_session"], "codex:native-a");
        assert!(
            notes[2..]
                .iter()
                .all(|n| n.get("harness_session").is_none())
        );
        let before_denied = env.read(&dir, &format!("tasks/{id}.md"));
        let out = as_agent(&env, &dir, "foreign")
            .env("CODEX_SESSION_ID", "native-b")
            .args(["done", &id])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "claimed");
        assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), before_denied);
        run(&["done", &id], with_pid);
        assert!(env.json(&dir, &["show", &id])["claim"].is_null());
    }
}

#[test]
fn list_help_shows_sort_values_and_examples() {
    let env = TestEnv::new();
    let out = env
        .cmd(env.home.path())
        .args(["list", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Filter by status (repeatable)"), "{text}");
    assert!(text.contains("Filter by tag (repeatable)"), "{text}");
    assert!(text.contains("Only tasks owned by this value"), "{text}");
    assert!(text.contains("--sort <SORT>"), "{text}");
    assert!(
        text.contains("priority (then last activity), updated, or created"),
        "{text}"
    );
    assert!(text.contains("[default: priority]"), "{text}");
    assert!(text.contains("Examples:"), "{text}");
    assert!(text.contains("tasks list --sort updated"), "{text}");
    assert!(
        text.contains("tasks list --status todo --tag cli"),
        "{text}"
    );
}

#[test]
fn init_creates_layout_and_registers() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    assert!(dir.join("tasks/.config.toml").is_file());
    assert!(dir.join("docs/specs").is_dir());
    assert!(dir.join("docs/plans").is_dir());
    let reg = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert!(reg.contains("sci = "), "{reg}");
    env.json(&dir, &["init", "--prefix", "sci"]);
    assert_eq!(env.fail(&dir, &["init", "--prefix", "fam"]), "config");
}

#[test]
fn next_is_the_head_of_ready_in_show_shape() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();

    let empty = env.json(nowhere.path(), &["next", "--all-projects"]);
    assert_eq!(empty["next"], serde_json::Value::Null);
    assert_eq!(empty["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "next", "--all-projects"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "nothing ready");

    // Top and Piece are both P1; Top is sized and Piece is not, so Top sorts first
    // whatever the clock says (timestamps have second precision; ids are random).
    let dep = id_of(env.json(&sci, &["add", "Dep", "-p", "3"]));
    let top = id_of(env.json(
        &fam,
        &[
            "add",
            "Top",
            "-p",
            "1",
            "--size",
            "s",
            "-b",
            "do the thing",
            "--depends",
            &dep,
        ],
    ));
    let goal = id_of(env.json(&fam, &["add", "Goal", "-p", "0"]));
    let piece = id_of(env.json(&fam, &["add", "Piece", "-p", "1", "--parent", &goal]));

    // locally, while Top is still blocked on Dep: Piece, with its parent resolved
    let v = env.json(&fam, &["next"]);
    assert_eq!(v["next"]["task"]["id"], piece, "{v}");
    assert_eq!(v["next"]["parent"]["id"], goal);
    assert!(
        v["next"].get("warnings").is_none(),
        "warnings live at the top: {v}"
    );

    // once Dep closes, Top is the head across projects, and its dependency is described
    env.json(&sci, &["done", &dep]);
    let v = env.json(nowhere.path(), &["next", "--all-projects"]);
    assert_eq!(v["next"]["task"]["id"], top, "{v}");
    assert_eq!(v["next"]["task"]["body"], "do the thing");
    assert_eq!(v["next"]["depends_on"][0]["id"], dep);
    assert_eq!(v["next"]["depends_on"][0]["status"], "done");
    assert_eq!(v["next"]["spec_path"], serde_json::Value::Null);
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "next", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("title: Top") && text.contains("# depends on"),
        "{text}"
    );
}

#[test]
fn init_refuses_prefix_registered_elsewhere() {
    let mut env = TestEnv::new();
    env.init("sci");
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(other.path(), &["init", "--prefix", "sci"]),
        "config"
    );
}

#[test]
fn init_warns_when_no_skill_installed_and_pretty_prints_prefix() {
    let env = TestEnv::new();
    let fresh = tempfile::tempdir().unwrap();
    let v = env.json(fresh.path(), &["init", "--prefix", "fam"]);
    assert_eq!(v["prefix"], "fam");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("skill"))
    );
    let fresh2 = tempfile::tempdir().unwrap();
    let out = env
        .cmd(fresh2.path())
        .args(["--pretty", "init", "--prefix", "niri"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "niri");
}

#[test]
fn no_project_is_an_error_and_usage_errors_exit_2() {
    let env = TestEnv::new();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(env.fail(dir.path(), &["list"]), "no_project");
    let out = env.cmd(dir.path()).args(["frobnicate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn local_read_prefers_no_project_to_a_malformed_registry() {
    let env = TestEnv::new();
    let dir = tempfile::tempdir().unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    std::fs::write(registry, "not toml = [").unwrap();

    assert_eq!(env.fail(dir.path(), &["list"]), "no_project");
}

fn write_dictionary(dir: &std::path::Path, prefix: &str, entries: &[(&str, &str)]) {
    let mut text = format!("prefix = \"{prefix}\"\n\n[tags]\n");
    for (tag, meaning) in entries {
        text.push_str(&format!("{tag} = \"{meaning}\"\n"));
    }
    std::fs::write(dir.join("tasks/.config.toml"), text).unwrap();
}

/// Opt a test project into feedback by appending a `[feedback]` table to its config.
fn accept_feedback(dir: &std::path::Path, scope: &str) {
    let path = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!("\n[feedback]\nscope = \"{scope}\"\n"));
    std::fs::write(path, text).unwrap();
}

/// The invalid `[feedback]` tables, as TOML text after the `prefix` line. ops's
/// `tests/test_ops_projects.py` (`INVALID_FEEDBACK`) carries the same list: the two
/// readers must refuse exactly the same configs (Task 6).
const INVALID_FEEDBACK: [(&str, &str); 11] = [
    ("[feedback]\n", "missing scope"),
    ("[feedback]\nscope = \"\"\n", "empty scope"),
    ("[feedback]\nscope = \"   \"\n", "blank scope"),
    ("[feedback]\nscope = \"a\\nb\"\n", "line feed"),
    ("[feedback]\nscope = \"a\\rb\"\n", "carriage return"),
    ("[feedback]\nscope = \"a\\tb\"\n", "tab"),
    ("[feedback]\nscope = \"a\\u001fb\"\n", "unit separator"),
    ("[feedback]\nscope = 3\n", "not a string"),
    ("[feedback]\nscop = \"Owns x.\"\n", "misspelled key"),
    (
        "[feedback]\nscope = \"Owns x.\"\nextra = 1\n",
        "unknown key",
    ),
    ("feedback = \"Owns x.\"\n", "not a table"),
];

#[test]
fn a_feedback_table_needs_a_one_line_scope_and_no_other_keys() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let config = sci.join("tasks/.config.toml");
    for (table, why) in INVALID_FEEDBACK {
        std::fs::write(&config, format!("prefix = \"sci\"\n\n{table}")).unwrap();
        assert_eq!(env.fail(&sci, &["list"]), "config", "{why}");
    }
    // Non-ASCII text and inner spaces are fine; only control characters and blankness are not.
    std::fs::write(
        &config,
        "prefix = \"sci\"\n\n[feedback]\nscope = \"Owns x \u{2014} and y.\"\n",
    )
    .unwrap();
    env.json(&sci, &["list"]);
}

#[test]
fn check_holds_feedback_tags_defined_on_feedback_records_in_an_accepting_project() {
    let mut env = TestEnv::new();
    let ai = env.init("ai");
    write_dictionary(&ai, "ai", &[("rules", "The instruction files.")]);
    // A report synced from a host where its source is registered; here it is not.
    let args = [
        "add",
        "Report",
        "--status",
        "idea",
        "--tag",
        "feedback",
        "--tag",
        "friction",
        "--tag",
        "from:gone",
    ];
    env.json(&ai, &args);
    // Without [feedback] the dictionary binds everything: three findings.
    assert_eq!(env.check(&ai)["warnings"].as_array().unwrap().len(), 3);

    accept_feedback(&ai, "Agent instructions.");
    let clean = env.check(&ai);
    assert!(clean["warnings"].as_array().unwrap().is_empty(), "{clean}");

    // Any other tag on a feedback record is still held to the dictionary ...
    env.json(
        &ai,
        &[
            "add",
            "Report two",
            "--status",
            "idea",
            "--tag",
            "feedback",
            "--tag",
            "perf",
        ],
    );
    // ... and so is the feedback vocabulary on a record that is not feedback.
    env.json(
        &ai,
        &["add", "Plain", "--tag", "friction", "--tag", "from:ai"],
    );
    let warnings = env.check(&ai)["warnings"].clone();
    let details: Vec<String> = warnings
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["detail"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(details.len(), 3, "{details:?}");
    for tag in ["\"perf\"", "\"friction\"", "\"from:ai\""] {
        assert!(
            details.iter().any(|d| d.contains(tag)),
            "{tag} in {details:?}"
        );
    }
}

#[test]
fn tags_carry_the_dictionary_meaning_and_check_holds_open_work_to_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "A", "--tag", "testing", "--tag", "perf"]);
    env.json(&fam, &["add", "C", "--tag", "testing"]);

    // No dictionary: meanings are null and check says nothing about tags.
    assert!(env.json(&sci, &["tags"])["tags"][0]["meaning"].is_null());
    assert!(env.check(&sci)["warnings"].as_array().unwrap().is_empty());

    write_dictionary(&sci, "sci", &[("testing", "Tests, gates, and CI.")]);
    let local = env.json(&sci, &["tags"]);
    let row = |name: &str| {
        local["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["tag"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(row("testing")["meaning"], "Tests, gates, and CI.");
    assert!(row("perf")["meaning"].is_null(), "perf has no entry");
    let text = env.pretty(&sci, &["tags"]);
    assert!(text.contains("testing  Tests, gates, and CI."), "{text}");

    // The open task carrying the undefined tag is a finding; a closed one is not.
    let warnings = env.check(&sci)["warnings"].clone();
    let warnings = warnings.as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["kind"], "undefined_tag");
    assert!(warnings[0]["detail"].as_str().unwrap().contains("\"perf\""));
    let id = warnings[0]["id"].as_str().unwrap().to_string();
    env.json(&sci, &["done", &id, "landed"]);
    assert!(env.check(&sci)["warnings"].as_array().unwrap().is_empty());

    // Across projects the first registered dictionary that defines the tag wins.
    write_dictionary(&fam, "fam", &[("testing", "fam's reading.")]);
    let nowhere = tempfile::tempdir().unwrap();
    let wide = env.json(nowhere.path(), &["tags", "--all-projects"]);
    let testing = wide["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["tag"] == "testing")
        .unwrap();
    assert_eq!(
        testing["meaning"], "fam's reading.",
        "fam registers before sci"
    );

    // A malformed entry is a config error, not a silent skip.
    std::fs::write(
        sci.join("tasks/.config.toml"),
        "prefix = \"sci\"\n\n[tags]\nx = \"\"\n",
    )
    .unwrap();
    let out = env.cmd(&sci).args(["tags"]).output().unwrap();
    assert_eq!(err_kind(&out), "config");
}

#[test]
fn tags_counts_per_project_and_filters_by_status() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "A", "--tag", "testing", "--tag", "perf"]);
    // A repeated tag counts the task once.
    env.json(&sci, &["add", "B", "--tag", "testing", "--tag", "testing"]);
    env.json(&fam, &["add", "C", "--tag", "testing"]);
    let old = id_of(env.json(&fam, &["add", "D", "--tag", "legacy"]));
    env.json(&fam, &["done", &old]);

    let local = env.json(&sci, &["tags"]);
    assert_eq!(
        local["tags"],
        serde_json::json!([
            { "tag": "testing", "meaning": null, "count": 2, "projects": { "sci": 2 } },
            { "tag": "perf", "meaning": null, "count": 1, "projects": { "sci": 1 } }
        ])
    );

    let nowhere = tempfile::tempdir().unwrap();
    let wide = env.json(nowhere.path(), &["tags", "--all-projects"]);
    assert_eq!(wide["tags"][0]["tag"], "testing");
    assert_eq!(wide["tags"][0]["count"], 3);
    assert_eq!(
        wide["tags"][0]["projects"],
        serde_json::json!({ "fam": 1, "sci": 2 })
    );
    assert_eq!(
        wide["tags"].as_array().unwrap().len(),
        2,
        "legacy is on a done task"
    );

    let closed = env.json(
        nowhere.path(),
        &["tags", "--all-projects", "--status", "done"],
    );
    assert_eq!(
        closed["tags"],
        serde_json::json!([{ "tag": "legacy", "meaning": null, "count": 1, "projects": { "fam": 1 } }])
    );

    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "tags", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("testing") && text.contains("fam 1, sci 2"),
        "{text}"
    );
}

#[test]
fn tasks_format_env_must_be_valid() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // A flag overrides a valid TASKS_FORMAT, never the check on an invalid one.
    for args in [
        &["list"][..],
        &["--pretty", "list"][..],
        &["--json", "list"][..],
    ] {
        let out = env
            .cmd(&dir)
            .env("TASKS_FORMAT", "xml")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(
            text.contains("\"kind\":\"config\"")
                && text.contains("TASKS_FORMAT must be json or pretty"),
            "{args:?}: {text}"
        );
    }
}

fn write_doc(dir: &std::path::Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn id_of(value: serde_json::Value) -> String {
    value["id"].as_str().unwrap().to_string()
}

/// Register `alias` as a retired prefix of `target` by writing the registry directly.
/// Until Task 11 there is no command that does this.
fn alias_registry(env: &TestEnv, alias: &str, target: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    if !text.contains("[aliases]") {
        text.push_str("\n[aliases]\n");
    }
    text.push_str(&format!("{alias} = {target:?}\n"));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn a_checkout_still_using_a_retired_prefix_refuses() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    // A second root under the retired name: what a branch predating the rename looks like.
    let stale = env.init_forced("dots");
    std::fs::write(stale.join("tasks/.config.toml"), "prefix = \"dot\"\n").unwrap();
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text = text.replace(
        &format!("dots = {:?}", stale.to_str().unwrap()),
        &format!("dots = {:?}", dots.to_str().unwrap()),
    );
    std::fs::write(&path, text).unwrap();
    alias_registry(&env, "dot", "dots");

    // Both a read and a write refuse, rather than routing or minting an old-prefix file.
    assert_eq!(env.fail(&stale, &["list"]), "config");
    assert_eq!(env.fail(&stale, &["add", "New", "-p", "2"]), "config");
}

/// Writes a claim straight into the store.
fn write_claim(env: &TestEnv, prefix: &str, id: &str, session: &str, live: bool) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (pid, pid_start, boot) = if live {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let rest = stat.rsplit_once(") ").unwrap().1.to_string();
        let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
        (std::process::id(), start, boot.trim().to_string())
    } else {
        (0, 1, "not-this-boot".to_string())
    };
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {pid}\n\
         pid_start = {pid_start}\nboot_id = \"{boot}\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n"
    ));
    std::fs::write(&path, text).unwrap();
}

/// Writes a park entry straight into the store.
fn write_park(
    env: &TestEnv,
    prefix: &str,
    id: &str,
    session: &str,
    waiting_on: &str,
    worktree: &str,
) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[parks.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\nhost = \"h\"\n\
         worktree = \"{worktree}\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"finish §3\"\n\
         waiting_on = \"{waiting_on}\"\ntitle = \"T\"\n"
    ));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn shelved_is_hidden_from_default_views_but_counted() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);

    let ids = |value: &serde_json::Value| -> Vec<String> {
        value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(ids(&env.json(&sci, &["list"])).is_empty());
    assert_eq!(
        ids(&env.json(&sci, &["list", "--status", "shelved"])),
        std::slice::from_ref(&id)
    );
    assert!(ids(&env.json(&sci, &["ready"])).is_empty());
    assert!(env.json(&sci, &["next"])["next"].is_null());
    let eligible = id_of(env.json(&sci, &["add", "Eligible", "--status", "idea"]));
    assert_eq!(
        ids(&env.json(
            &sci,
            &["sample", "-n", "5", "--seed", "1", "--older-than", "0d"]
        )),
        [eligible]
    );

    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["counts"]["shelved"], 1);
    assert_eq!(prime["counts"]["idea"], 1);
    assert!(env.pretty(&sci, &["prime"]).contains("shelved 1"));
}

#[test]
fn tree_shows_shelved_children_but_roadmap_hides_every_shelved_node() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&sci, &["add", "Child", "--parent", &goal]));
    let subgoal = id_of(env.json(&sci, &["add", "Subgoal", "--parent", &goal]));
    let leaf = id_of(env.json(&sci, &["add", "Leaf", "--parent", &subgoal]));
    let root = id_of(env.json(&sci, &["add", "Root", "--status", "idea"]));
    let root_child = id_of(env.json(&sci, &["add", "Root child", "--parent", &root]));
    env.json(&sci, &["shelve", &child, "later"]);
    env.json(&sci, &["shelve", &leaf, "later"]);
    env.json(&sci, &["shelve", &subgoal, "later"]);
    env.json(&sci, &["shelve", &root_child, "later"]);
    env.json(&sci, &["shelve", &root, "later"]);

    let tree = env.json(&sci, &["tree"]);
    assert_eq!(tree["nodes"].as_array().unwrap().len(), 1, "{tree}");
    assert_eq!(tree["nodes"][0]["id"], goal);
    let children = tree["nodes"][0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert!(children.iter().any(|node| node["id"] == child));
    let subgoal_node = children.iter().find(|node| node["id"] == subgoal).unwrap();
    assert_eq!(subgoal_node["children"][0]["id"], leaf);
    assert_eq!(
        env.json(&sci, &["tree", "--all"])["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["roadmap"].as_array().unwrap().len(), 1, "{prime}");
    assert_eq!(prime["roadmap"][0]["id"], goal);
    fn assert_no_shelves(nodes: &[serde_json::Value]) {
        for node in nodes {
            assert_ne!(node["status"], "shelved", "{node}");
            if let Some(children) = node["children"].as_array() {
                assert_no_shelves(children);
            }
        }
    }
    assert_no_shelves(prime["roadmap"].as_array().unwrap());

    let shown = env.json(&sci, &["show", &goal]);
    assert_eq!(
        shown["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == child)
            .unwrap()["status"],
        "shelved"
    );
    assert_eq!(
        env.json(&sci, &["projects"])["projects"][0]["counts"]["shelved"],
        5
    );

    let error = error_of(&env, &sci, &["done", &goal]);
    assert_eq!(error["error"]["kind"], "open_descendants");
    assert!(error["error"]["detail"].as_str().unwrap().contains(&leaf));
    assert_eq!(env.json(&sci, &["show", &goal])["task"]["status"], "todo");

    env.json(&sci, &["unshelve", &child]);
    env.json(&sci, &["unshelve", &root_child]);
    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["roadmap"].as_array().unwrap().len(), 1, "{prime}");
    assert_eq!(prime["roadmap"][0]["id"], goal);
    assert_eq!(prime["roadmap"][0]["children"][0]["id"], child);
    assert!(
        prime["roadmap"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node["id"] != root)
    );
    assert_eq!(
        env.json(&sci, &["tree"])["nodes"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn parked_shelved_task_is_visible_but_never_a_next_candidate() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);
    write_park(
        &env,
        "sci",
        &id,
        "agent-a",
        "agent",
        &sci.display().to_string(),
    );

    assert!(env.json(&sci, &["next"])["next"].is_null());
    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["parked"][0]["status"], "shelved");
    let parked = env.json(&sci, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["status"], "shelved");
}

#[test]
fn check_warns_when_open_work_depends_on_shelved_work() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let dep = id_of(env.json(&sci, &["add", "Dep", "-p", "2"]));
    let work = id_of(env.json(&sci, &["add", "Work", "-p", "2", "--depends", &dep]));
    env.json(&sci, &["shelve", &dep, "later"]);

    assert!(
        env.json(&sci, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["id"] != work)
    );

    let check = env.check(&sci);
    let warning = check["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|warning| warning["kind"] == "shelved_dep")
        .unwrap_or_else(|| panic!("{check}"));
    assert_eq!(warning["id"], work);
    assert_eq!(
        warning["detail"],
        format!("depends on shelved {dep}: unshelve it or drop the dependency")
    );

    env.json(&sci, &["shelve", &work, "later"]);
    assert!(
        !env.check(&sci)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["kind"] == "shelved_dep")
    );
}

#[test]
fn park_appears_in_show_and_list_json_and_pretty_show() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    assert!(env.json(&sci, &["show", &id])["park"].is_null());
    assert!(env.json(&sci, &["list"])["tasks"][0]["park"].is_null());

    write_park(&env, "sci", &id, "claude:abc", "user", "/elsewhere");
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["next_step"], "finish §3");
    assert_eq!(v["park"]["waiting_on"], "user");
    assert_eq!(v["park"]["session"], "claude:abc");
    assert_eq!(v["park"]["worktree"], "/elsewhere");
    assert_eq!(v["park"]["at"], "2026-01-02T00:00:00Z");
    assert!(v["claim"].is_null(), "a park is not a claim");
    assert_eq!(
        env.json(&sci, &["list"])["tasks"][0]["park"]["waiting_on"],
        "user"
    );
    let text = env.pretty(&sci, &["show", &id]);
    assert!(text.contains("# parked"), "{text}");
    assert!(text.contains("waiting on user"), "{text}");
    assert!(text.contains("finish §3"), "{text}");
}

#[test]
fn park_records_the_entry_and_a_note_and_re_parking_replaces() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "write §3 of the spec"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo", "status is untouched");
    assert_eq!(v["park"]["next_step"], "write §3 of the spec");
    assert_eq!(v["park"]["waiting_on"], "agent", "the default");
    assert_eq!(
        v["park"]["session"], "agent-a",
        "TASKS_SESSION is written verbatim"
    );
    assert_eq!(v["park"]["owner"], "tester");
    assert_eq!(v["park"]["worktree"], sci.to_str().unwrap());
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("parked (waiting on agent): write §3 of the spec"),
        "{raw}"
    );
    assert!(!raw.contains("next_step"), "no frontmatter field: {raw}");

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "decide the store shape",
            "--waiting-on",
            "user",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["next_step"], "decide the store shape");
    assert_eq!(v["park"]["waiting_on"], "user");
    assert_eq!(
        v["task"]["notes"].as_array().unwrap().len(),
        2,
        "the notes accumulate"
    );
}

#[test]
fn park_replaces_the_callers_claim_and_follows_the_claim_rules_for_others() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "next"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["claim"].is_null(), "the caller's own claim is replaced");
    assert_eq!(v["park"]["session"], "agent-a");
    assert_eq!(v["task"]["status"], "doing");

    // A foreign live claim refuses; there is no --force.
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let out = as_agent(&env, &sci, "agent-b")
        .args(["park", &id, "steal"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "claimed");
    assert!(err_detail(&out).contains("agent-a"));
    let out = as_agent(&env, &sci, "agent-b")
        .args(["park", "--force", &id, "steal"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "no --force flag exists");

    // A foreign stale claim is replaced with the takeover warning.
    let other = id_of(env.json(&sci, &["add", "U", "-p", "2"]));
    write_claim(&env, "sci", &other, "ghost", false);
    let out = as_agent(&env, &sci, "agent-b")
        .args(["park", &other, "carry on"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("took over")),
        "{v}"
    );
    assert_eq!(
        env.json(&sci, &["show", &other])["park"]["session"],
        "agent-b"
    );

    // Any session may re-park.
    as_agent(&env, &sci, "agent-c")
        .args(["park", &other, "again"])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &other])["park"]["session"],
        "agent-c"
    );
}

#[test]
fn start_resumes_a_parked_task_and_closing_removes_the_entry() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "next"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "doing");

    as_agent(&env, &sci, "agent-b")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"].is_null(), "resumed");
    assert_eq!(v["claim"]["session"], "agent-b");
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(!raw.contains("took over"), "a park is free to take: {raw}");

    as_agent(&env, &sci, "agent-b")
        .args(["park", &id, "next"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["done", &id, "landed"])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["park"].is_null(),
        "done removes it"
    );

    let dropped = id_of(env.json(&sci, &["add", "D", "-p", "2"]));
    as_agent(&env, &sci, "agent-b")
        .args(["park", &dropped, "x"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["drop", &dropped, "no"])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &dropped])["park"].is_null(),
        "drop removes it"
    );
}

#[test]
fn block_unblock_note_and_dep_leave_a_park_entry_alone() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let other = id_of(env.json(&sci, &["add", "O", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "next"])
        .assert()
        .success();

    as_agent(&env, &sci, "agent-a")
        .args(["block", &id, "waiting"])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["park"]["next_step"],
        "next",
        "blocked, still parked"
    );
    as_agent(&env, &sci, "agent-a")
        .args(["unblock", &id])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["park"]["next_step"], "next");
    as_agent(&env, &sci, "agent-a")
        .args(["note", &id, "a thought"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["dep", &id, "--on", &other])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["edit", &id, "--tag", "x"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["park"]["next_step"], "next");

    write_claim(&env, "sci", &other, "agent-z", true);
    as_agent(&env, &sci, "agent-a")
        .args(["note", &other, "fine"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &other]);
    assert_eq!(v["claim"]["session"], "agent-z");
    assert_eq!(
        v["claim"]["seen"], "2026-01-01T00:00:00Z",
        "a foreign claim is never refreshed"
    );
}

#[test]
fn status_changing_edits_clear_a_park_and_unchanged_saves_keep_it() {
    use std::os::unix::fs::MetadataExt;

    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let closed = id_of(env.json(&sci, &["add", "C", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &closed, "x"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["edit", &closed, "--status", "done"])
        .assert()
        .success();
    assert!(env.json(&sci, &["show", &closed])["park"].is_null());

    let taken = id_of(env.json(&sci, &["add", "D", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &taken, "x"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["edit", &taken, "--status", "doing"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &taken]);
    assert!(v["park"].is_null(), "a change into doing is a resume");
    assert_eq!(v["claim"]["session"], "agent-a");

    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "resume here"])
        .assert()
        .success();
    let stale = id_of(env.json(&sci, &["add", "S", "-p", "2"]));
    write_claim(&env, "sci", &stale, "dead-agent", false);
    let store = env.claim_store("sci");
    let before = std::fs::read_to_string(&store).unwrap();
    let before_ino = std::fs::metadata(&store).unwrap().ino();
    let editor = editor_script(&sci, "sed -i '/^## Notes/i edited body' \"$1\"");
    as_agent(&env, &sci, "agent-a")
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(
        v["park"]["next_step"], "resume here",
        "an unchanged-status save keeps the park"
    );
    assert!(v["claim"].is_null(), "and acquires nothing");
    assert!(v["task"]["body"].as_str().unwrap().contains("edited body"));
    assert_eq!(
        std::fs::read_to_string(&store).unwrap(),
        before,
        "the store was not written"
    );
    assert_eq!(std::fs::metadata(&store).unwrap().ino(), before_ino);
    as_agent(&env, &sci, "agent-a")
        .args(["edit", &id, "--status", "doing"])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["park"]["next_step"],
        "resume here",
        "edit --status <same> is not a transition"
    );
    assert_eq!(std::fs::read_to_string(&store).unwrap(), before);
    assert_eq!(std::fs::metadata(&store).unwrap().ino(), before_ino);
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["park"].is_null(),
        "explicit start clears"
    );

    let held = id_of(env.json(&sci, &["add", "H", "-p", "2"]));
    write_claim(&env, "sci", &held, "agent-z", true);
    let out = as_agent(&env, &sci, "agent-a")
        .env("EDITOR", &editor)
        .args(["edit", &held])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    let out = as_agent(&env, &sci, "agent-a")
        .args(["edit", &held, "--status", "todo"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
}

#[test]
fn park_refuses_a_closed_task_and_validates_its_arguments() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let idea = id_of(env.json(&sci, &["add", "I", "--status", "idea"]));

    assert_eq!(env.fail(&sci, &["park", &id, ""]), "validation");
    assert_eq!(env.fail(&sci, &["park", &id, "two\nlines"]), "validation");
    let err = env.usage(&sci, &["park", &id, "x", "--waiting-on", "nobody"]);
    assert!(
        err.contains("--waiting-on") && err.contains("nobody"),
        "{err}"
    );
    assert!(
        env.json(&sci, &["show", &id])["park"].is_null(),
        "nothing landed"
    );

    env.json(&sci, &["done", &id, "landed"]);
    let out = env.cmd(&sci).args(["park", &id, "x"]).output().unwrap();
    assert_eq!(err_kind(&out), "invalid_transition");
    assert!(err_detail(&out).contains("parked"), "{}", err_detail(&out));

    // An idea needs no start: parking it writes the entry that makes it visible.
    as_agent(&env, &sci, "agent-a")
        .args(["park", &idea, "write the problem statement"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &idea]);
    assert_eq!(v["task"]["status"], "idea");
    assert_eq!(v["park"]["next_step"], "write the problem statement");
}

#[test]
fn park_reason_rides_the_entry_the_note_and_every_park_view() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "open the round-02 sheet",
            "--waiting-on",
            "user",
            "--reason",
            "review",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "review");
    assert_eq!(v["park"]["waiting_on"], "user");
    assert_eq!(
        env.json(&sci, &["list", "--parked"])["tasks"][0]["park"]["reason"],
        "review"
    );
    assert_eq!(
        env.json(&sci, &["prime"])["parked"][0]["park"]["reason"],
        "review"
    );
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("parked (waiting on user, review): open the round-02 sheet"),
        "{raw}"
    );
    let text = env.pretty(&sci, &["show", &id]);
    assert!(text.contains("waiting on user, review since"), "{text}");
    let table = env.pretty(&sci, &["list", "--parked"]);
    assert!(table.contains("waits on user, review"), "{table}");

    // `next` hands back agent-parked work with the same block.
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "rerun after the restart",
            "--reason",
            "environment",
        ])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], id);
    assert_eq!(next["next"]["park"]["reason"], "environment");
    assert_eq!(next["next"]["park"]["waiting_on"], "agent");
}

#[test]
fn park_without_a_reason_records_none_and_re_parking_drops_a_previous_one() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "write §3"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"]["reason"].is_null(), "{v}");
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("parked (waiting on agent): write §3"),
        "old note form: {raw}"
    );

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "decide the shape",
            "--waiting-on",
            "user",
            "--reason",
            "decision",
        ])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["park"]["reason"], "decision");
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "decide the shape", "--waiting-on", "user"])
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["park"]["reason"].is_null(),
        "a re-park without the flag records none"
    );

    let err = env.usage(&sci, &["park", &id, "x", "--reason", "boredom"]);
    assert!(err.contains("--reason") && err.contains("boredom"), "{err}");
    assert!(
        env.json(&sci, &["show", &id])["park"]["reason"].is_null(),
        "nothing landed"
    );
}

#[test]
fn ordinary_pretty_tables_mark_a_quiet_park_until_it_is_resumed_or_reparked() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let idle = "waits on user, quiet; idle, 40 min";
    let park = |args: &[&str]| {
        as_agent(&env, &sci, "agent-a")
            .args(["park", &id, "rerun the preflight"])
            .args(args)
            .assert()
            .success();
    };
    let row = |text: String| -> String {
        text.lines()
            .find(|line| line.starts_with(&id))
            .unwrap_or_else(|| panic!("no row for {id} in {text}"))
            .to_string()
    };

    park(&[
        "--waiting-on",
        "user",
        "--reason",
        "quiet",
        "--minutes",
        "40",
    ]);
    assert!(row(env.pretty(&sci, &["list"])).contains(idle));
    let prime = env.pretty(&sci, &["prime"]);
    let roadmap = &prime[prime.find("roadmap:").unwrap()..prime.find("parked:").unwrap()];
    assert!(row(roadmap.to_string()).contains(idle), "{prime}");
    // JSON keeps its shape: the park object carries the recipe, and nothing else is added.
    let listed = &env.json(&sci, &["list"])["tasks"][0];
    assert_eq!(listed["park"]["needs"], "idle", "{listed}");
    assert_eq!(listed["park"]["minutes"], 40, "{listed}");
    // A user-waiting park stays out of ready.
    assert!(!env.pretty(&sci, &["ready"]).contains(&id));

    // Waiting on the agent, the quiet park stays eligible and carries the marker there too.
    park(&[
        "--reason",
        "quiet",
        "--needs",
        "headless",
        "--minutes",
        "50",
    ]);
    assert!(row(env.pretty(&sci, &["ready"])).contains("waits on agent, quiet; headless, 50 min"));

    // A park for another reason, and a resume, both drop the marker.
    park(&["--waiting-on", "user", "--reason", "review"]);
    let listed = row(env.pretty(&sci, &["list"]));
    assert!(
        !listed.contains("quiet") && !listed.contains("waits on"),
        "{listed}"
    );
    park(&[
        "--waiting-on",
        "user",
        "--reason",
        "quiet",
        "--minutes",
        "40",
    ]);
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let listed = row(env.pretty(&sci, &["list"]));
    assert!(!listed.contains("waits on"), "{listed}");
}

#[test]
fn a_quiet_park_records_its_recipe_in_the_entry_the_note_and_every_park_view() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "rerun the readiness preflight, then capture",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "50",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "quiet");
    assert_eq!(v["park"]["needs"], "idle");
    assert_eq!(v["park"]["minutes"], 50);
    assert_eq!(
        env.json(&sci, &["list", "--parked"])["tasks"][0]["park"]["minutes"],
        50
    );
    assert_eq!(
        env.json(&sci, &["prime"])["parked"][0]["park"]["needs"],
        "idle"
    );
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("parked (waiting on user, quiet; idle, 50 min): rerun the readiness preflight, then capture"), "{raw}");
    assert!(
        env.pretty(&sci, &["show", &id])
            .contains("waiting on user, quiet; idle, 50 min since")
    );
    assert!(
        env.pretty(&sci, &["list", "--parked"])
            .contains("waits on user, quiet; idle, 50 min")
    );

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "log out, then run the power capture",
            "--reason",
            "quiet",
            "--needs",
            "headless",
            "--minutes",
            "90",
        ])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], id);
    assert_eq!(next["next"]["park"]["needs"], "headless");
    assert_eq!(next["next"]["park"]["minutes"], 90);

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "read the sheet",
            "--waiting-on",
            "user",
            "--reason",
            "review",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["park"]["reason"], "review");
    assert!(
        v["park"]["needs"].is_null() && v["park"]["minutes"].is_null(),
        "{v}"
    );

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "write §3"])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(
        v["park"]["needs"].is_null() && v["park"]["minutes"].is_null(),
        "{v}"
    );
}

#[test]
fn quiet_park_flags_are_validated_together() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let err = error_of(&env, &sci, &["park", &id, "x", "--reason", "quiet"]);
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--reason quiet needs --minutes <n>"),
        "{err}"
    );
    let err = error_of(&env, &sci, &["park", &id, "x", "--minutes", "10"]);
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--minutes on park needs --reason quiet"),
        "{err}"
    );
    let err = error_of(&env, &sci, &["park", &id, "x", "--needs", "idle"]);
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--needs on park needs --reason quiet"),
        "{err}"
    );
    let err = error_of(
        &env,
        &sci,
        &[
            "park",
            &id,
            "x",
            "--reason",
            "environment",
            "--minutes",
            "10",
        ],
    );
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--minutes on park needs --reason quiet"),
        "{err}"
    );
    let err = env.usage(
        &sci,
        &[
            "park",
            &id,
            "x",
            "--reason",
            "quiet",
            "--minutes",
            "10",
            "--needs",
            "sometimes",
        ],
    );
    assert!(
        err.contains("--needs") && err.contains("idle, headless"),
        "{err}"
    );
    let err = error_of(
        &env,
        &sci,
        &[
            "park",
            &id,
            "x",
            "--reason",
            "quiet",
            "--minutes",
            "10",
            "--complexity",
            "high",
        ],
    );
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("--complexity on park needs --reason capability"),
        "{err}"
    );
    for bad in ["0", "1441"] {
        let out = env
            .cmd(&sci)
            .args(["park", &id, "x", "--reason", "quiet", "--minutes", bad])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "--minutes {bad}");
    }
    assert!(env.json(&sci, &["show", &id])["park"].is_null());
}

#[test]
fn start_on_a_quiet_park_removes_the_entry_and_its_recipe() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "capture",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["park"].is_null(), "{v}");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(!store.contains("minutes"), "{store}");
}

#[test]
fn claim_appears_in_show_and_list_json() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());

    write_claim(&env, "sci", &id, "agent-a", true);
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["claim"]["session"], "agent-a");
    assert_eq!(v["claim"]["live"], true);
    assert_eq!(v["claim"]["worktree"], "/elsewhere");
    assert_eq!(v["claim"]["pid"], std::process::id());
    assert_eq!(
        env.json(&sci, &["list"])["tasks"][0]["claim"]["session"],
        "agent-a"
    );
}

#[test]
fn a_dead_claim_is_reported_as_not_live() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "ghost", false);
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["live"], false);
}

#[test]
fn pretty_rows_name_the_claim_holder_not_the_local_owner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "agent-a", true);

    let out = env.cmd(&sci).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("someone"), "the claim's own owner: {text}");
}

#[test]
fn pretty_tables_wrap_at_columns_but_piped_output_stays_line_oriented() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let long_title = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu";
    id_of(env.json(&sci, &["add", long_title, "-p", "2"]));

    let wrapped = env
        .cmd(&sci)
        .env("COLUMNS", "80")
        .args(["--pretty", "list"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&wrapped.stdout).to_string();
    let lines: Vec<&str> = text.trim_end().lines().collect();
    assert!(lines.len() > 1, "{text:?}");
    let title_start = lines[0].find("alpha").expect("title on the first line");
    for line in &lines[1..] {
        let content = line.trim_start();
        assert!(!content.is_empty(), "{text:?}");
        assert_eq!(line.len() - content.len(), title_start, "{text:?}");
    }
    for line in &lines {
        assert!(line.chars().count() <= 80, "{text:?}");
    }

    let piped = env.cmd(&sci).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8_lossy(&piped.stdout).to_string();
    assert_eq!(text.trim_end().lines().count(), 1, "{text:?}");

    // 60 columns leave fewer than 20 for the title: the row overflows unwrapped rather
    // than rendering one word per line.
    let narrow = env
        .cmd(&sci)
        .env("COLUMNS", "60")
        .args(["--pretty", "list"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&narrow.stdout).to_string();
    assert_eq!(text.trim_end().lines().count(), 1, "{text:?}");

    let json_with = env
        .cmd(&sci)
        .env("COLUMNS", "80")
        .args(["--json", "list"])
        .output()
        .unwrap();
    let json_without = env.cmd(&sci).args(["--json", "list"]).output().unwrap();
    assert_eq!(json_with.stdout, json_without.stdout);
}

#[test]
fn an_invalid_columns_value_fails_as_config() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let out = env
        .cmd(&sci)
        .env("COLUMNS", "wide")
        .args(["--pretty", "list"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        text.contains("COLUMNS must be a positive integer"),
        "{text}"
    );
}

#[test]
fn columns_only_applies_to_pretty_tables() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "A task"]));
    for args in [vec!["--json", "list"], vec!["--pretty", "show", &id]] {
        let out = env
            .cmd(&sci)
            .env("COLUMNS", "wide")
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn read_commands_do_not_take_the_mutation_lock() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let lock = env.claim_store("sci").with_file_name("sci.lock");
    // The setup add took the lock; reads must not create it again.
    std::fs::remove_file(&lock).unwrap();

    env.json(&sci, &["show", &id]);
    env.check(&sci);
    env.json(&sci, &["graph"]);
    assert!(!lock.exists(), "read commands must not create the lock");

    env.json(&sci, &["note", &id, "hello"]);
    assert!(lock.exists(), "a write command takes it");
}

#[test]
fn add_project_creates_in_the_named_project_from_anywhere() {
    let mut env = TestEnv::new();
    let ops = env.init("ops");
    let fam = env.init("fam");
    write_doc(&fam, "docs/specs/fam-thing.md", "# Fam thing\n");
    let goal = id_of(env.json(&ops, &["add", "Cross-cutting goal"]));
    let groundwork = id_of(env.json(&ops, &["add", "Groundwork"]));
    let fam_parent = id_of(env.json(&fam, &["add", "Fam goal"]));
    let nowhere = tempfile::tempdir().unwrap();

    // a foreign --depends resolves through the registry from the target's point of view
    let id = id_of(env.json(
        nowhere.path(),
        &[
            "add",
            "Fam piece",
            "--project",
            "fam",
            "--parent",
            &fam_parent,
            "--spec",
            "fam-thing",
            "--depends",
            &groundwork,
            "--tag",
            "audit",
        ],
    ));
    assert!(id.starts_with("fam-"), "{id}");
    let shown = env.json(&fam, &["show", &id]);
    assert_eq!(shown["task"]["parent"], fam_parent);
    assert_eq!(shown["task"]["spec"], "docs/specs/fam-thing.md");
    assert_eq!(shown["task"]["depends"][0], groundwork);
    assert_eq!(shown["task"]["tags"][0], "audit");
    assert_eq!(shown["task"]["status"], "todo");

    // validated against the target, not the caller: ops has no such spec or parent
    assert_eq!(
        env.fail(&ops, &["add", "x", "--project", "fam", "--spec", "nope"]),
        "doc_not_found"
    );
    assert_eq!(
        env.fail(&ops, &["add", "x", "--project", "fam", "--parent", &goal]),
        "validation"
    );
    // an explicit prefix targets the registry root, not a displaced checkout with the
    // same prefix
    let displaced = tempfile::tempdir().unwrap();
    std::fs::create_dir(displaced.path().join("tasks")).unwrap();
    std::fs::write(
        displaced.path().join("tasks/.config.toml"),
        "prefix = \"ops\"\n",
    )
    .unwrap();
    let registered = id_of(env.json(displaced.path(), &["add", "Local", "--project", "ops"]));
    assert!(registered.starts_with("ops-"));
    assert!(ops.join(format!("tasks/{registered}.md")).is_file());
    assert!(
        !displaced
            .path()
            .join(format!("tasks/{registered}.md"))
            .exists()
    );
    // an unknown prefix is config, since a person typed it
    assert_eq!(
        env.fail(nowhere.path(), &["add", "x", "--project", "zzz"]),
        "config"
    );
    // the wire-up: the goal depends on the piece, leaves ready while it is open, and
    // returns once it closes
    env.json(&ops, &["dep", &goal, "--on", &id]);
    let in_ready = |env: &TestEnv| {
        env.json(&ops, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == goal)
    };
    assert!(!in_ready(&env));
    env.json(&ops, &["done", &groundwork]);
    env.json(&fam, &["done", &id]);
    assert!(in_ready(&env), "the goal is the verify-and-close step now");
}

#[test]
fn parent_is_validated_persisted_and_clearable() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.init("fam");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child", "--parent", &goal]));
    let raw = env.read(&dir, &format!("tasks/{child}.md"));
    assert!(
        raw.contains(&format!("depends: []\nparent: {goal}\ntags: []\n")),
        "{raw}"
    );
    assert_eq!(env.json(&dir, &["show", &child])["task"]["parent"], goal);
    assert_eq!(
        env.json(&dir, &["show", &goal])["task"]["parent"],
        serde_json::Value::Null
    );

    assert_eq!(
        env.fail(&dir, &["add", "x", "--parent", "sci-ffffff"]),
        "unresolvable_id"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--parent", "fam-000001"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["edit", &goal, "--parent", &child]),
        "cycle"
    );
    assert_eq!(env.fail(&dir, &["edit", &goal, "--parent", &goal]), "cycle");
    let grandchild = id_of(env.json(&dir, &["add", "Grandchild", "--parent", &child]));
    assert_eq!(
        env.fail(&dir, &["edit", &goal, "--parent", &grandchild]),
        "cycle"
    );
    let files = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "md")
        })
        .count();
    assert_eq!(
        files, 3,
        "rejected adds wrote nothing: goal, child, grandchild only"
    );

    env.json(&dir, &["edit", &child, "--no-parent"]);
    assert_eq!(
        env.json(&dir, &["show", &child])["task"]["parent"],
        serde_json::Value::Null
    );
}

#[test]
fn editor_path_validates_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let child = id_of(env.json(&dir, &["add", "Child"]));
    let set = editor_script(
        &dir,
        &format!("sed -i 's/^depends: \\[\\]$/depends: []\\nparent: {goal}/' \"$1\""),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &set)
        .args(["edit", &child])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(env.json(&dir, &["show", &child])["task"]["parent"], goal);

    let loop_back = editor_script(
        &dir,
        &format!("sed -i 's/^depends: \\[\\]$/depends: []\\nparent: {child}/' \"$1\""),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &loop_back)
        .args(["edit", &goal])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "cycle");
}

#[test]
fn check_reports_parent_problems() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    let c = id_of(env.json(&dir, &["add", "C"]));
    let d = id_of(env.json(&dir, &["add", "D"]));
    let e = id_of(env.json(&dir, &["add", "E"]));
    let f = id_of(env.json(&dir, &["add", "F", "--parent", &a]));
    // a -> b loop with f as a tail into it, c dangling, d foreign, e its own parent;
    // written by hand to simulate drift
    let set_parent = |id: &str, parent: &str| {
        let raw = env.read(&dir, &format!("tasks/{id}.md"));
        std::fs::write(
            dir.join(format!("tasks/{id}.md")),
            raw.replace("depends: []\n", &format!("depends: []\nparent: {parent}\n")),
        )
        .unwrap();
    };
    set_parent(&a, &b);
    set_parent(&c, "sci-ffffff");
    set_parent(&d, "fam-000001");
    set_parent(&e, &e);
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<(String, String)> = check["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["kind"].as_str().unwrap().to_string(),
                f["id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        kinds.contains(&("parent_cycle".into(), a.clone().min(b.clone()))),
        "{check}"
    );
    assert!(
        kinds.contains(&("dangling_parent".into(), c.clone())),
        "{check}"
    );
    assert!(
        kinds.contains(&("foreign_parent".into(), d.clone())),
        "{check}"
    );
    assert!(
        kinds.contains(&("parent_cycle".into(), e.clone())),
        "self-edge: {check}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|(kind, _)| kind == "parent_cycle")
            .count(),
        2,
        "each cycle is reported once, at its lowest member, even with the tail f: {check}"
    );
    assert!(
        !kinds.iter().any(|(_, id)| id == &f),
        "the tail is not a cycle member: {check}"
    );
    assert!(
        !kinds.iter().any(|(kind, _)| kind == "parse"),
        "a self-edge is a hierarchy finding, not a parse error: {check}"
    );
}

#[test]
fn check_reports_deferred_goals_and_deferrals_on_the_wrong_status() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "--defer", "2099-01-02"]));
    let kid = id_of(env.json(&sci, &["add", "Kid"]));
    // Reachable only by hand: both writers refuse the pair (§3.3).
    let path = sci.join(format!("tasks/{kid}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("depends: []\n", &format!("depends: []\nparent: {goal}\n")),
    )
    .unwrap();
    let closed = id_of(env.json(&sci, &["add", "Closed"]));
    env.json(&sci, &["done", &closed, "x"]);
    let path = sci.join(format!("tasks/{closed}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("priority: 2\n", "priority: 2\ndefer: 2099-01-02\n"),
    )
    .unwrap();

    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<(&str, &str)> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["kind"].as_str().unwrap(), e["id"].as_str().unwrap()))
        .collect();
    assert!(kinds.contains(&("deferred_goal", goal.as_str())), "{v}");
    assert!(kinds.contains(&("defer_status", closed.as_str())), "{v}");
    assert!(
        !kinds.iter().any(|(kind, _)| *kind == "parse"),
        "the status rule is not parsing's: {v}"
    );

    // Parsing still owns the shape rules: defer beside every fails the scan.
    let both = id_of(env.json(&sci, &["add", "Both", "--every", "30d"]));
    let path = sci.join(format!("tasks/{both}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("every: 30d\n", "every: 30d\ndefer: 2099-01-02\n"),
    )
    .unwrap();
    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "parse" && e["file"].as_str().unwrap().contains(&both)),
        "{v}"
    );

    let malformed = id_of(env.json(&sci, &["add", "Malformed"]));
    let path = sci.join(format!("tasks/{malformed}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("priority: 2\n", "priority: 2\ndefer: not-a-date\n"),
    )
    .unwrap();
    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "parse" && e["file"].as_str().unwrap().contains(&malformed)),
        "{v}"
    );
}

#[test]
fn add_writes_a_valid_file_and_show_reads_it_back() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let v = env.json(
        &dir,
        &[
            "add",
            "Bank the ledger",
            "-p",
            "1",
            "--size",
            "m",
            "--tag",
            "ledger",
            "--tag",
            "cut-12",
            "-b",
            "Body text",
        ],
    );
    let id = v["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("sci-") && id.len() == 10, "{id}");
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    assert!(
        raw.starts_with(&format!(
            "---\nid: {id}\ntitle: Bank the ledger\nstatus: todo\npriority: 1\nsize: m\n"
        )),
        "{raw}"
    );
    assert!(raw.contains("tags: [ledger, cut-12]\n"));
    assert!(raw.ends_with("---\n\nBody text\n"));
    let s = env.json(&dir, &["show", &id]);
    assert_eq!(s["task"]["title"], "Bank the ledger");
    assert_eq!(s["task"]["body"], "Body text");
    assert_eq!(s["task"]["size"], "m");
    assert_eq!(s["task"]["owner"], serde_json::Value::Null);
    assert!(s["task"].get("notes").is_none());
    assert_eq!(s["spec_path"], serde_json::Value::Null);
    assert_eq!(s["step_found"], serde_json::Value::Null);
    assert!(s.get("depends_on").is_none());
    assert_eq!(s["warnings"], serde_json::json!([]));
}

#[test]
fn body_leading_whitespace_survives_routine_writes() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let body = "\n    indented first line\n\n  indented second line";
    let id = env.json(&dir, &["add", "Indented", "--body", body])["id"]
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(env.json(&dir, &["show", &id])["task"]["body"], body);
    env.json(&dir, &["note", &id, "routine write"]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["body"], body);
}

#[test]
fn add_validates_before_writing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // a value outside a closed set is a usage error, refused before anything runs
    let err = env.usage(&dir, &["add", "x", "-p", "9"]);
    assert!(err.contains("--priority") && err.contains("'9'"), "{err}");
    let err = env.usage(&dir, &["add", "x", "--size", "huge"]);
    assert!(err.contains("--size") && err.contains("huge"), "{err}");
    let err = env.usage(&dir, &["add", "x", "--status", "done"]);
    assert!(err.contains("--status") && err.contains("done"), "{err}");
    assert_eq!(
        env.fail(&dir, &["add", "x", "--depends", "sci-ffffff"]),
        "unresolvable_id"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--depends", "zzz-ffffff"]),
        "unresolvable_id"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--spec", "nothing"]),
        "doc_not_found"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--step", "Task 1"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "-b", "a\n## Notes\nb"]),
        "validation"
    );
    let n = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "md")
        })
        .count();
    assert_eq!(n, 0, "no task files should have been written");
}

#[test]
fn add_resolves_spec_plan_and_step() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(
        &dir,
        "docs/specs/2026-08-24-holdings-design.md",
        "# Holdings\n",
    );
    write_doc(
        &dir,
        "docs/specs/2026-08-25-holdings-v2-design.md",
        "# Holdings v2\n",
    );
    write_doc(
        &dir,
        "docs/plans/2026-08-24-holdings.md",
        "# Plan\n\n### Task 1: emit rows\n\n### Task 2: verify\n",
    );
    assert_eq!(env.fail(&dir, &["add", "x", "--plan", ""]), "validation");
    assert_eq!(
        env.fail(&dir, &["add", "x", "--spec", "holdings"]),
        "ambiguous"
    );
    assert_eq!(
        env.fail(
            &dir,
            &["add", "x", "--plan", "holdings", "--step", "Task 9: nope"]
        ),
        "validation"
    );
    assert_eq!(
        env.fail(
            &dir,
            &["add", "x", "--spec", "docs/plans/2026-08-24-holdings.md"]
        ),
        "validation"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "add",
                "x",
                "--spec",
                "docs/specs/../plans/2026-08-24-holdings.md"
            ]
        ),
        "validation"
    );
    let v = env.json(
        &dir,
        &[
            "add",
            "x",
            "--spec",
            "holdings-v2",
            "--plan",
            "holdings",
            "--step",
            "Task 1: emit rows",
        ],
    );
    let id = v["id"].as_str().unwrap();
    let s = env.json(&dir, &["show", id]);
    assert_eq!(
        s["task"]["spec"],
        "docs/specs/2026-08-25-holdings-v2-design.md"
    );
    assert_eq!(s["task"]["plan"], "docs/plans/2026-08-24-holdings.md");
    assert_eq!(s["step_found"], true);
    assert!(
        s["spec_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/specs/2026-08-25-holdings-v2-design.md")
    );
    write_doc(
        &dir,
        "docs/plans/2026-08-24-holdings.md",
        "# Plan\n\n### Task 1: emit the rows\n",
    );
    let s = env.json(&dir, &["show", id]);
    assert_eq!(s["step_found"], false);

    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "show", id])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("\n\x1b[31m# step MISSING\x1b[0m\n"),
        "no painted step marker: {text:?}"
    );
}

#[test]
fn add_resolves_specs_from_supported_directories() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for (topic, rel) in [
        ("root-specs", "docs/specs/2026-09-02-root-specs-design.md"),
        (
            "root-designs",
            "docs/designs/2026-09-02-root-designs-design.md",
        ),
        (
            "superpowers-specs",
            "docs/superpowers/specs/2026-09-02-superpowers-specs-design.md",
        ),
        (
            "superpowers-designs",
            "docs/superpowers/designs/2026-09-02-superpowers-designs-design.md",
        ),
    ] {
        write_doc(&dir, rel, "# Design\n");
        let id = env.json(&dir, &["add", topic, "--spec", topic])["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(env.json(&dir, &["show", &id])["task"]["spec"], rel);
    }
}

#[test]
fn add_resolves_plans_from_supported_directories() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for (topic, rel) in [
        ("root-plans", "docs/plans/2026-09-04-root-plans.md"),
        (
            "superpowers-plans",
            "docs/superpowers/plans/2026-09-04-superpowers-plans.md",
        ),
    ] {
        write_doc(&dir, rel, "# Plan\n\n### Task 1: bank\n");
        let id = env.json(
            &dir,
            &["add", topic, "--plan", topic, "--step", "Task 1: bank"],
        )["id"]
            .as_str()
            .unwrap()
            .to_string();
        let shown = env.json(&dir, &["show", &id]);
        assert_eq!(shown["task"]["plan"], rel);
        assert_eq!(shown["step_found"], true);
    }
}

#[test]
fn bare_spec_names_are_ambiguous_across_supported_directories() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/specs/2026-09-02-shared-design.md", "# Specs\n");
    write_doc(
        &dir,
        "docs/designs/2026-09-02-shared-design.md",
        "# Designs\n",
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--spec", "shared"]),
        "ambiguous"
    );
}

#[test]
fn configured_doc_roots_replace_the_defaults() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\nspec_dirs = [\"design/\", \"rfcs\"]\nplan_dirs = [\"planning\"]\n",
    )
    .unwrap();
    write_doc(&dir, "design/2026-09-03-ledger-design.md", "# Design\n");
    write_doc(&dir, "rfcs/0001-index.md", "# RFC\n");
    write_doc(
        &dir,
        "planning/2026-09-03-ledger.md",
        "# Plan\n\n### Task 1: bank\n",
    );
    write_doc(&dir, "docs/specs/2026-09-03-old-design.md", "# Old\n");

    let id = env.json(
        &dir,
        &[
            "add",
            "Bank",
            "--spec",
            "ledger",
            "--plan",
            "ledger",
            "--step",
            "Task 1: bank",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let shown = env.json(&dir, &["show", &id]);
    assert_eq!(shown["task"]["spec"], "design/2026-09-03-ledger-design.md");
    assert_eq!(shown["task"]["plan"], "planning/2026-09-03-ledger.md");
    assert_eq!(shown["step_found"], true);

    let id = env.json(&dir, &["add", "Index", "--spec", "rfcs/0001-index.md"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["spec"],
        "rfcs/0001-index.md"
    );

    assert_eq!(
        env.fail(
            &dir,
            &["add", "x", "--spec", "docs/specs/2026-09-03-old-design.md"]
        ),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--spec", "old"]),
        "doc_not_found"
    );
    let check = env.check(&dir);
    assert_eq!(check["errors"].as_array().unwrap().len(), 0, "{check}");
}

#[test]
fn doc_root_rejections_name_the_config_key_that_sets_the_roots() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/plans/2026-09-24-ledger-design.md", "# Design\n");
    write_doc(&dir, "docs/specs/2026-09-24-ledger-plan.md", "# Plan\n");
    let id = env.json(&dir, &["add", "Ledger"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    for (args, kind, key) in [
        (
            ["--spec", "docs/plans/2026-09-24-ledger-design.md"],
            "validation",
            "spec_dirs",
        ),
        (["--spec", "missing"], "doc_not_found", "spec_dirs"),
        (
            ["--plan", "docs/specs/2026-09-24-ledger-plan.md"],
            "validation",
            "plan_dirs",
        ),
        (["--plan", "missing"], "doc_not_found", "plan_dirs"),
    ] {
        let error = error_of(&env, &dir, &[&["edit", &id][..], &args[..]].concat());
        assert_eq!(error["error"]["kind"], kind, "{error}");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(
            detail.contains(key) && detail.contains("tasks/.config.toml"),
            "{args:?}: {detail}"
        );
    }

    // Following the hint attaches the document the rejection refused.
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\nspec_dirs = [\"docs/specs\", \"docs/plans\"]\n",
    )
    .unwrap();
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--spec",
            "docs/plans/2026-09-24-ledger-design.md",
        ],
    );
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["spec"],
        "docs/plans/2026-09-24-ledger-design.md"
    );
}

#[test]
fn check_reports_links_outside_the_configured_roots() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/specs/2026-09-03-ledger-design.md", "# Design\n");
    let id = env.json(&dir, &["add", "Bank", "--spec", "ledger"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\nspec_dirs = [\"design\"]\n",
    )
    .unwrap();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let check: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let errors = check["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{check}");
    assert_eq!(errors[0]["file"], format!("tasks/{id}.md"));
    assert_eq!(errors[0]["kind"], "parse");
    assert!(
        errors[0]["detail"]
            .as_str()
            .unwrap()
            .contains("under design/"),
        "{check}"
    );
    assert_eq!(env.fail(&dir, &["show", &id]), "parse");
}

#[test]
fn malformed_doc_roots_are_a_config_error() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for config in [
        "prefix = \"sci\"\nspec_dirs = []\n",
        "prefix = \"sci\"\nplan_dirs = [\"../plans\"]\n",
        "prefix = \"sci\"\nspec_dirs = [\"/abs/specs\"]\n",
        "prefix = \"sci\"\nspec_dirs = \"docs/specs\"\n",
    ] {
        std::fs::write(dir.join("tasks/.config.toml"), config).unwrap();
        assert_eq!(env.fail(&dir, &["list"]), "config", "{config}");
    }
}

#[test]
fn init_creates_the_first_configured_roots() {
    let env = TestEnv::new();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("tasks")).unwrap();
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\nspec_dirs = [\"design\", \"rfcs\"]\nplan_dirs = [\"planning\"]\n",
    )
    .unwrap();
    env.json(&dir, &["init", "--prefix", "sci"]);
    assert!(dir.join("design").is_dir());
    assert!(dir.join("planning").is_dir());
    assert!(!dir.join("rfcs").exists());
    assert!(!dir.join("docs").exists());
}

#[test]
fn show_resolves_local_and_foreign_dependencies() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let f = env.json(&fam, &["add", "Foreign dep"]);
    let fid = f["id"].as_str().unwrap().to_string();
    let a = env.json(&sci, &["add", "Local dep"]);
    let aid = a["id"].as_str().unwrap().to_string();
    let b = env.json(&sci, &["add", "Main", "--depends", &aid, "--depends", &fid]);
    let bid = b["id"].as_str().unwrap().to_string();
    let s = env.json(&sci, &["show", &bid]);
    let deps = s["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0]["id"], aid);
    assert_eq!(deps[0]["title"], "Local dep");
    assert_eq!(deps[0]["resolved"], true);
    assert_eq!(deps[1]["id"], fid);
    assert_eq!(deps[1]["title"], "Foreign dep");

    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "always", "show", &bid])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("\x1b[2m{aid}\x1b[0m [todo] Local dep")),
        "{text:?}"
    );
    assert!(
        text.contains(&format!("\x1b[2m{fid}\x1b[0m [todo] Foreign dep")),
        "{text:?}"
    );

    std::fs::remove_file(fam.join(format!("tasks/{fid}.md"))).unwrap();
    let s = env.json(&sci, &["show", &bid]);
    assert_eq!(s["depends_on"][1]["resolved"], false);
    assert_eq!(s["depends_on"][1]["title"], serde_json::Value::Null);
    assert_eq!(s["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(env.fail(&sci, &["show", "sci-000000"]), "task_not_found");
    assert_eq!(env.fail(&sci, &["show", "bogus"]), "invalid_id");
}

#[test]
fn list_all_projects_walks_registry() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "S"]);
    env.json(&fam, &["add", "F"]);
    let v = env.json(&sci, &["list", "--all-projects"]);
    let mut titles: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    titles.sort();
    assert_eq!(titles, ["F", "S"]);
}

#[test]
fn list_all_projects_warns_for_missing_registered_config() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();

    let v = env.json(&sci, &["list", "--all-projects"]);

    assert_eq!(v["tasks"].as_array().unwrap().len(), 0);
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
}

#[test]
fn list_all_projects_errors_for_malformed_registered_config() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::write(fam.join("tasks/.config.toml"), "not toml = [").unwrap();

    assert_eq!(env.fail(&sci, &["list", "--all-projects"]), "config");
}

#[test]
fn all_projects_needs_no_local_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "S"]);
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["title"], "S");
    assert_eq!(v["warnings"], serde_json::json!([]));
}

#[test]
fn all_projects_warns_on_empty_registry_and_unregistered_current_project() {
    let env = TestEnv::new();
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!(["registry is empty"]));

    // a project that was initialised under another home is not in this registry
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lone = tempfile::tempdir().unwrap();
    let other = TestEnv::new();
    other.json(lone.path(), &["init", "--prefix", "lon"]);
    env.json(&sci, &["add", "S"]);
    let v = env.json(lone.path(), &["list", "--all-projects"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(
        v["warnings"],
        serde_json::json!(["current project lon is not registered"])
    );
}

#[test]
fn all_projects_rejects_a_prefix_mismatch() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::write(fam.join("tasks/.config.toml"), "prefix = \"zzz\"\n").unwrap();
    assert_eq!(env.fail(&sci, &["list", "--all-projects"]), "config");
}

#[test]
fn project_reads_one_named_registered_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "S"]);
    env.json(&fam, &["add", "F"]);

    // from inside another project, and from no project at all
    for dir in [sci.as_path(), tempfile::tempdir().unwrap().path()] {
        let v = env.json(dir, &["list", "--project", "fam"]);
        let tasks = v["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1, "{v}");
        assert_eq!(tasks[0]["title"], "F");
        assert_eq!(v["warnings"], serde_json::json!([]));
    }
}

#[test]
fn project_scope_covers_every_read_command() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&sci, &["add", "S", "--tag", "sci-only"]);
    let goal = id_of(env.json(&fam, &["add", "F goal"]));
    let child = id_of(env.json(
        &fam,
        &["add", "F child", "--parent", &goal, "--tag", "fam-only"],
    ));

    let v = env.json(&sci, &["ready", "--project", "fam"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["tasks"][0]["id"], child, "the goal has a child");

    let v = env.json(&sci, &["next", "--project", "fam"]);
    assert_eq!(v["next"]["task"]["id"], child, "{v}");

    let v = env.json(&sci, &["prime", "--project", "fam"]);
    assert_eq!(v["prefix"], "fam", "a named project is a local scope");
    assert_eq!(v["projects"], serde_json::json!(["fam"]));
    assert_eq!(v["counts"]["todo"], 2, "{v}");

    let v = env.json(&sci, &["tree", "--project", "fam"]);
    let nodes = v["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "{v}");
    assert_eq!(nodes[0]["id"], goal);
    assert_eq!(nodes[0]["children"][0]["id"], child);

    // an id the local project has never heard of, read from the project that owns it --
    // named explicitly, or left to the id's own prefix to route
    let v = env.json(&sci, &["tree", "--project", "fam", &goal]);
    assert_eq!(v["nodes"][0]["id"], goal, "{v}");
    let v = env.json(&sci, &["tree", &goal]);
    assert_eq!(v["nodes"][0]["id"], goal, "{v}");

    let v = env.json(&sci, &["tags", "--project", "fam"]);
    assert_eq!(
        v["tags"],
        serde_json::json!([{ "tag": "fam-only", "meaning": null, "count": 1, "projects": { "fam": 1 } }])
    );
}

#[test]
fn project_and_all_projects_conflict_on_every_read_command() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    for command in ["list", "ready", "next", "prime", "tree", "tags", "quiet"] {
        let out = env
            .cmd(&sci)
            .args([command, "--project", "fam", "--all-projects"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{command}: --project and --all-projects conflict"
        );
    }
}

#[test]
fn project_scope_reports_an_unusable_prefix_as_config() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    assert_eq!(env.fail(&sci, &["list", "--project", "zzz"]), "config");

    std::fs::write(fam.join("tasks/.config.toml"), "prefix = \"nope\"\n").unwrap();
    assert_eq!(env.fail(&sci, &["list", "--project", "fam"]), "config");

    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.fail(&sci, &["list", "--project", "fam"]),
        "config",
        "unreachable is a warning registry-wide, but an error when named"
    );
}

#[test]
fn project_scope_takes_the_registered_root_over_a_worktree_of_the_same_prefix() {
    let mut env = TestEnv::new();
    let fam = env.init("fam");
    env.json(&fam, &["add", "Registered"]);
    // a second checkout of fam that the registry does not point at
    let worktree = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(worktree.path().join("tasks")).unwrap();
    std::fs::write(
        worktree.path().join("tasks/.config.toml"),
        "prefix = \"fam\"\n",
    )
    .unwrap();
    env.json(worktree.path(), &["add", "Worktree"]);

    let v = env.json(worktree.path(), &["list"]);
    assert_eq!(v["tasks"][0]["title"], "Worktree", "the default is local");
    let v = env.json(worktree.path(), &["list", "--project", "fam"]);
    let tasks = v["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1, "{v}");
    assert_eq!(tasks[0]["title"], "Registered", "--project names a root");
}

#[test]
fn project_scope_never_looks_at_the_current_directory() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    env.json(&fam, &["add", "F"]);
    std::fs::write(sci.join("tasks/.config.toml"), "not toml = [").unwrap();

    let v = env.json(&sci, &["list", "--project", "fam"]);
    assert_eq!(v["tasks"][0]["title"], "F", "{v}");
}

#[test]
fn completion_follows_the_named_project_scope() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    let foreign = id_of(env.json(&fam, &["add", "Foreign"]));

    assert_eq!(
        env.complete_values(&sci, "bash", 4, &["tasks", "tree", "--project", "fam", ""]),
        [foreign.as_str()],
        "tree's id comes from the named project"
    );
    assert_eq!(
        env.complete(
            &sci,
            "bash",
            5,
            &["tasks", "list", "--project", "fam", "--parent", ""]
        ),
        [foreign.as_str()],
        "list --parent follows the same scope"
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--parent", ""]),
        [local.as_str()],
        "and the default is still local"
    );
    assert!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--project", ""])
            .contains(&"fam".to_string()),
        "the prefix itself completes"
    );
}

#[test]
fn list_warns_about_unreachable_dependencies() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let dep = env.json(&fam, &["add", "F"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&sci, &["add", "S", "--depends", &dep]);
    std::fs::remove_file(fam.join(format!("tasks/{dep}.md"))).unwrap();
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
    assert!(v["warnings"][0].as_str().unwrap().contains(&dep));
}

#[test]
fn note_appends_single_line_entries() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = env.json(&dir, &["add", "A", "-b", "Body"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["note", &id, "first"]);
    env.cmd(&dir)
        .env("TASKS_OWNER", "agent-7")
        .args(["note", &id, "second"])
        .assert()
        .success();
    assert_eq!(env.fail(&dir, &["note", &id, "a\nb"]), "validation");
    assert_eq!(env.fail(&dir, &["note", &id, ""]), "validation");
    let out = env
        .cmd(&dir)
        .env("TASKS_OWNER", "bad owner)")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = env
        .cmd(&dir)
        .env_remove("USER")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "no owner source must be an error, not \"unknown\""
    );
    assert_eq!(env.fail(&dir, &["add", "x", "--tag", "a\nb"]), "validation");
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    assert!(raw.contains("\nBody\n\n## Notes\n\n- 20"), "{raw}");
    assert!(raw.contains("(tester): first\n- 20"), "{raw}");
    assert!(raw.ends_with("(agent-7): second\n"), "{raw}");
    let s = env.json(&dir, &["show", &id]);
    assert_eq!(s["task"]["notes"].as_array().unwrap().len(), 2);
    assert_eq!(s["task"]["notes"][1]["by"], "agent-7");
}

#[test]
fn new_notes_lose_trailing_spaces_and_tabs() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A"]));
    env.json(&dir, &["note", &id, "  keep  inner \t \t"]);
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["done", &id, "landed \t"]);
    let notes = env.json(&dir, &["show", &id])["task"]["notes"].clone();
    let texts: Vec<&str> = notes
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["  keep  inner", "started", "done", "landed"]);
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    assert!(
        raw.lines().all(|line| !line.ends_with([' ', '\t'])),
        "{raw:?}"
    );
    for blank in [" ", "\t", " \t "] {
        assert_eq!(env.fail(&dir, &["note", &id, blank]), "validation");
    }
    assert_eq!(env.fail(&dir, &["note", &id, "a\n "]), "validation");
    assert_eq!(env.fail(&dir, &["note", &id, "a \r"]), "validation");
}

#[test]
fn the_editor_may_only_strip_trailing_whitespace_from_notes() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A", "-b", "Body"]));
    env.cmd(&dir)
        .env("CLAUDE_CODE_SESSION_ID", "native-a")
        .env("CLAUDE_PID", std::process::id().to_string())
        .args(["start", &id])
        .assert()
        .success();
    env.json(&dir, &["note", &id, "  n1"]);
    // Records written before insertion trimmed, or by hand, can still carry the whitespace.
    let path = dir.join(format!("tasks/{id}.md"));
    let seeded = env
        .read(&dir, &format!("tasks/{id}.md"))
        .replace("): started\n", "): started \t\n")
        .replace("):   n1\n", "):   n1  \n");
    std::fs::write(&path, &seeded).unwrap();
    let before = env.json(&dir, &["show", &id])["task"]["notes"].clone();
    assert_eq!(before[0]["text"], "started \t");
    assert_eq!(before[1]["text"], "  n1  ");

    let edit = |script: &str| {
        let editor = editor_script(&dir, script);
        env.cmd(&dir)
            .env("CLAUDE_CODE_SESSION_ID", "native-a")
            .env("CLAUDE_PID", std::process::id().to_string())
            .env("EDITOR", &editor)
            .args(["edit", &id])
            .output()
            .unwrap()
    };
    for (script, why) in [
        (
            "sed -i 's/): started[ \\t]*$/): starte/' \"$1\"",
            "text beyond the whitespace",
        ),
        ("sed -i 's/):   n1  $/): n1/' \"$1\"", "leading whitespace"),
        (
            "sed -i 's/):   n1  $/):   n1   /' \"$1\"",
            "added whitespace",
        ),
        (
            "sed -i 's/[ \\t]*$//; /^  provenance: /d' \"$1\"",
            "provenance",
        ),
        (
            "sed -i 's/[ \\t]*$//; s/^- 20[^ ]* (tester): started/- 2000-01-01T00:00:00Z (tester): started/' \"$1\"",
            "timestamp",
        ),
        (
            "sed -i 's/[ \\t]*$//; /): started$/,+1d' \"$1\"",
            "a removed note",
        ),
    ] {
        let out = edit(script);
        assert_eq!(out.status.code(), Some(1), "{why} must be refused");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("append-only"),
            "{why}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), seeded);

    let out = edit("sed -i 's/[ \\t]*$//' \"$1\"");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = env.json(&dir, &["show", &id])["task"]["notes"].clone();
    assert_eq!(after.as_array().unwrap().len(), 2);
    assert_eq!(after[0]["text"], "started");
    assert_eq!(after[1]["text"], "  n1");
    for (old, new) in before
        .as_array()
        .unwrap()
        .iter()
        .zip(after.as_array().unwrap())
    {
        for key in ["at", "by", "harness_session", "harness_session_source"] {
            assert_eq!(old[key], new[key], "{key}");
        }
    }
    assert_eq!(after[0]["harness_session"], "claude-code:native-a");
}

#[test]
fn start_done_drop_block_unblock_transitions() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let dep = env.json(&dir, &["add", "Dep"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = env.json(&dir, &["add", "Main", "--depends", &dep])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.cmd(&dir)
        .env("TASKS_OWNER", "me")
        .args(["start", &id])
        .assert()
        .success();
    let s = env.json(&dir, &["show", &id]);
    assert_eq!(s["task"]["status"], "doing");
    assert_eq!(s["task"]["owner"], "me");
    assert_eq!(
        env.fail(&dir, &["done", &id, "too soon"]),
        "open_dependencies"
    );
    env.json(&dir, &["block", &id, "waiting on dep"]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "blocked");
    env.json(&dir, &["unblock", &id]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "todo");
    env.json(&dir, &["done", &id, "forced", "--force"]);
    let s = env.json(&dir, &["show", &id]);
    assert_eq!(s["task"]["status"], "done");
    assert_eq!(
        s["task"]["notes"].as_array().unwrap().last().unwrap()["text"],
        "forced"
    );
    assert_eq!(env.fail(&dir, &["drop", &id]), "invalid_transition");
    assert_eq!(env.fail(&dir, &["start", &id]), "invalid_transition");
    env.json(&dir, &["done", &dep]);
    assert_eq!(
        env.fail(&dir, &["drop", &dep, "nope"]),
        "invalid_transition"
    ); // done -> dropped is not allowed
}

#[test]
fn start_warns_when_a_repo_with_several_worktrees_leaves_the_claim_uncommitted() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&sci)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "seed"]);

    let v = env.json(&sci, &["start", &id]);
    assert!(
        !v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("worktree")),
        "a single-worktree repo has nothing to warn about: {v}"
    );
    git(&["commit", "-qam", "start"]);

    git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "side",
        sci.join("wt").to_str().unwrap(),
    ]);
    let other = id_of(env.json(&sci, &["add", "U", "-p", "2"]));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "add U"]);

    let v = env.json(&sci, &["start", &other]);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&other) && w.contains("worktree")
        }),
        "{v}"
    );
}

#[test]
fn start_reports_a_git_worktree_inspection_failure_as_a_warning() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&sci)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "seed"]);

    let bin = tempfile::tempdir().unwrap();
    let fake_git = bin.path().join("git");
    std::fs::write(
        &fake_git,
        "#!/bin/sh\nif [ \"$1 $2\" = \"worktree list\" ]; then echo broken >&2; exit 42; fi\nPATH=${PATH#*:} exec git \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );

    let output = env
        .cmd(&sci)
        .env("PATH", path)
        .args(["start", &id])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("worktree list failed") && w.contains("broken")
        }),
        "{v}"
    );
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn list_defaults_to_open_and_filters() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A", "-p", "3", "--tag", "x"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "-p", "0"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let c = env.json(&dir, &["add", "C", "--status", "idea", "--tag", "x"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["drop", &a, "obsolete"]);
    let v = env.json(&dir, &["list"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [b.as_str(), c.as_str()]);
    let v = env.json(&dir, &["list", "--status", "dropped"]);
    assert_eq!(v["tasks"][0]["id"], a);
    let v = env.json(
        &dir,
        &[
            "list", "--status", "idea", "--status", "dropped", "--tag", "x",
        ],
    );
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2);
    let err = env.usage(&dir, &["list", "--status", "weird"]);
    assert!(err.contains("--status") && err.contains("weird"), "{err}");
    let summary = &v["tasks"][0];
    for key in [
        "id", "title", "status", "priority", "created", "updated", "tags",
    ] {
        assert!(summary.get(key).is_some(), "summary missing {key}");
    }
    assert!(summary.get("body").is_none());
}

#[test]
fn ready_excludes_ideas_doing_and_open_deps() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "--depends", &a, "-p", "0"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["add", "I", "--status", "idea", "-p", "0"]);
    let d = env.json(&dir, &["add", "D", "-p", "0"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["start", &d]);
    let v = env.json(&dir, &["ready"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [a.as_str()]);
    env.json(&dir, &["done", &a]);
    let v = env.json(&dir, &["ready"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [b.as_str()]);
    let v = env.json(&dir, &["ready", "-n", "0"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn ready_all_projects_orders_across_projects() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "3"]));
    let high = id_of(env.json(&fam, &["add", "High", "-p", "1", "--size", "s"]));
    let mid = id_of(env.json(&sci, &["add", "Mid", "-p", "1", "--size", "m"]));
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["ready", "--all-projects"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [high.as_str(), mid.as_str(), low.as_str()]);
    let v = env.json(nowhere.path(), &["ready", "--all-projects", "-n", "1"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
}

#[test]
fn prime_all_projects_reports_scope_and_per_project_uncommitted_files() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&fam)
        .status()
        .unwrap();
    assert!(status.success());
    let s = id_of(env.json(&sci, &["add", "S"]));
    let f = id_of(env.json(&fam, &["add", "F"]));
    env.json(&sci, &["start", &s]);

    let local = env.json(&sci, &["prime"]);
    assert_eq!(local["prefix"], "sci");
    assert_eq!(local["projects"], serde_json::json!(["sci"]));

    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["prime", "--all-projects"]);
    assert_eq!(v["prefix"], serde_json::Value::Null);
    assert_eq!(v["projects"], serde_json::json!(["fam", "sci"]));
    assert_eq!(v["counts"]["todo"], 1);
    assert_eq!(v["counts"]["doing"], 1);
    assert_eq!(v["ready"][0]["id"], f);
    assert_eq!(v["doing"][0]["id"], s);
    assert_eq!(v["roadmap"].as_array().unwrap().len(), 2);
    assert_eq!(
        v["warnings"],
        serde_json::json!([format!(
            "fam: uncommitted task files: tasks/.config.toml, tasks/{f}.md"
        )])
    );
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "prime", "--all-projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("projects fam, sci\n"), "{text}");
}

#[test]
fn prime_reports_counts_ready_and_doing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["add", "B"]);
    env.json(&dir, &["start", &a]);
    let v = env.json(&dir, &["prime"]);
    assert_eq!(v["prefix"], "sci");
    assert_eq!(v["counts"]["todo"], 1);
    assert_eq!(v["counts"]["doing"], 1);
    assert_eq!(v["counts"]["done"], 0);
    assert_eq!(v["ready"].as_array().unwrap().len(), 1);
    assert_eq!(v["doing"][0]["id"], a);
    assert_eq!(v["doing"][0]["owner"], "tester");
    let out = env.cmd(&dir).args(["--pretty", "prime"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("sci") && text.contains(&a), "{text}");
}

#[test]
fn prime_shows_roadmap_and_closeout() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // goal gets an explicit priority so it sorts before loner deterministically: both are
    // otherwise tied on priority/size/created (same second) and the id tie-break is random hex.
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "1"]));
    let leaf = id_of(env.json(&dir, &["add", "Leaf", "--parent", &goal]));
    let loner = id_of(env.json(&dir, &["add", "Loner"]));
    env.json(&dir, &["start", &goal]);

    let prime = env.json(&dir, &["prime"]);
    assert_eq!(prime["closeout"], serde_json::json!([]));
    let roadmap = prime["roadmap"].as_array().unwrap();
    assert_eq!(roadmap.len(), 2, "{prime}");
    assert_eq!(roadmap[0]["id"], goal);
    assert_eq!(roadmap[0]["children"][0]["id"], leaf);
    assert_eq!(roadmap[1]["id"], loner);
    let ready: Vec<&str> = prime["ready"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(!ready.contains(&goal.as_str()));

    env.json(&dir, &["done", &leaf]);
    let prime = env.json(&dir, &["prime"]);
    assert_eq!(
        prime["closeout"][0]["id"], goal,
        "a doing parent surfaces: {prime}"
    );
    assert!(
        prime["ready"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"] != goal)
    );

    let parked = id_of(env.json(&dir, &["add", "Parked", "--status", "idea"]));
    let kid = id_of(env.json(&dir, &["add", "Kid", "--parent", &parked]));
    env.json(&dir, &["done", &kid]);
    let prime = env.json(&dir, &["prime"]);
    assert!(
        prime["closeout"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"] != parked),
        "an idea is never a close-out candidate: {prime}"
    );

    let stuck = id_of(env.json(&dir, &["add", "Stuck"]));
    env.json(&dir, &["block", &stuck, "waiting"]);
    let out = env.cmd(&dir).args(["--pretty", "prime"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\ncloseout:\n"), "{text}");
    let roadmap = text
        .split("\nroadmap:\n")
        .nth(1)
        .unwrap()
        .split("\nready:\n")
        .next()
        .unwrap();
    assert!(
        roadmap.contains(&goal),
        "roots with children print as subtrees: {roadmap}"
    );
    assert!(
        roadmap.contains(&parked),
        "an idea with children is still a subtree: {roadmap}"
    );
    assert!(
        roadmap.contains(&stuck),
        "a childless root absent from ready is printed: {roadmap}"
    );
    assert!(
        !roadmap.contains(&loner),
        "a childless root present in ready is only counted: {roadmap}"
    );
    let colored = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "prime"])
        .output()
        .unwrap();
    let colored = String::from_utf8_lossy(&colored.stdout);
    for header in ["closeout:", "roadmap:", "ready:", "doing:"] {
        assert!(
            colored.contains(&format!("\n\x1b[1m{header}\x1b[0m\n")),
            "{header} must be emphasised with the newlines outside the span: {colored:?}"
        );
    }

    assert!(
        roadmap.contains("1 childless root(s) are listed under ready"),
        "{roadmap}"
    );
}

/// An EDITOR value that has sh read the script rather than exec it: executing a file this
/// process just wrote races sibling test threads, whose forks can still hold the write
/// descriptor, into ETXTBSY.
fn editor_script(dir: &std::path::Path, body: &str) -> String {
    let p = dir.join("editor.sh");
    std::fs::write(&p, format!("{body}\n")).unwrap();
    format!("sh '{}'", p.display())
}

/// Writes `last_done: <stamp>` into a task record, inserting the line if it is absent and
/// replacing it if it is not. Fixture-only: it reaches states the CLI produces later, or
/// (in Task 3) does not produce at all.
fn seed_anchor(dir: &std::path::Path, id: &str, stamp: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let seeded = if text.contains("\nlast_done: ") {
        text.lines()
            .map(|line| {
                if line.starts_with("last_done: ") {
                    format!("last_done: {stamp}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    } else {
        text.replace("updated: ", &format!("last_done: {stamp}\nupdated: "))
    };
    std::fs::write(&path, seeded).unwrap();
}

#[test]
fn done_stamps_model_from_tasks_model_and_absence_records_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let stamped = id_of(env.json(&sci, &["add", "Stamped", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &stamped, "landed"])
        .env("TASKS_MODEL", "claude-fable-5-1")
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &stamped])["task"]["model"],
        "claude-fable-5-1"
    );

    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    env.json(&sci, &["done", &plain, "landed"]);
    assert!(env.json(&sci, &["show", &plain])["task"]["model"].is_null());
}

#[test]
fn recompletion_without_the_variable_clears_the_stamp() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Rework", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "first pass"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "A");
    env.json(&sci, &["edit", &id, "--status", "todo"]);
    env.json(&sci, &["done", &id, "second pass"]);
    assert!(
        env.json(&sci, &["show", &id])["task"]["model"].is_null(),
        "a fresh completion with no TASKS_MODEL records unknown, not the stale stamp"
    );
}

#[test]
fn recurring_restamps_and_recovery_preserves_the_stamp() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.cmd(&sci)
        .args(["done", &id, "first"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    env.json(&sci, &["start", &id]);
    env.cmd(&sci)
        .args(["done", &id, "second"])
        .env("TASKS_MODEL", "B")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");

    // A bare repeat with no claim or park errors before any write.
    env.cmd(&sci)
        .args(["done", &id, "third"])
        .env("TASKS_MODEL", "C")
        .assert()
        .failure();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");

    // An interrupted completion whose claim cleanup failed: the record says done
    // while this checkout still holds the claim. The retry takes the claim-release
    // branch, which is not a completion and never restamps.
    env.json(&sci, &["start", &id]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: doing", "status: done")).unwrap();
    let out = env
        .cmd(&sci)
        .args(["done", &id, "cleanup"])
        .env("TASKS_MODEL", "C")
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(
        value["warnings"].to_string().contains("already completed"),
        "{value}"
    );
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");
}

#[test]
fn edit_status_done_and_editor_flips_stamp_like_done() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let via_flag = id_of(env.json(&sci, &["add", "Flag", "-p", "2"]));
    env.cmd(&sci)
        .args(["edit", &via_flag, "--status", "done"])
        .env("TASKS_MODEL", "E")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &via_flag])["task"]["model"], "E");

    let via_editor = id_of(env.json(&sci, &["add", "Editor", "-p", "2"]));
    let editor = editor_script(&sci, "sed -i 's/^status: todo$/status: done/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &via_editor])
        .env("EDITOR", &editor)
        .env("TASKS_MODEL", "F")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &via_editor])["task"]["model"], "F");
}

#[test]
fn start_stamps_started_once_and_later_starts_leave_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    assert!(env.json(&sci, &["show", &id])["task"]["started"].is_null());
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let first = env.json(&sci, &["show", &id])["task"]["started"].clone();
    assert!(first.is_string(), "{first}");
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace(
            &format!("started: {}", first.as_str().unwrap()),
            "started: 2000-01-01T00:00:00Z",
        ),
    )
    .unwrap();
    let first = "2000-01-01T00:00:00Z";
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "resume later"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["started"],
        first,
        "resume"
    );
    as_agent(&env, &sci, "agent-b")
        .args(["start", &id, "--force"])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["started"],
        first,
        "takeover"
    );
    as_agent(&env, &sci, "agent-b")
        .args(["edit", &id, "--status", "todo"])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["edit", &id, "--status", "doing"])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["started"],
        first,
        "edit --status doing"
    );
    assert_eq!(env.json(&sci, &["list"])["tasks"][0]["started"], first);
    assert!(env.json(&sci, &["list"])["tasks"][0]["completed"].is_null());
    as_agent(&env, &sci, "agent-b")
        .args(["park", &id, "later", "--waiting-on", "user"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["prime"])["parked"][0]["started"], first);
}

#[test]
fn recurring_completion_stamps_both_and_recovery_stamps_neither() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.json(&sci, &["done", &id, "first"]);
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(
        v["task"]["completed"], v["periodic"]["last_done"],
        "one instant for both stamps: {v}"
    );
    let first = v["task"]["completed"].clone();
    env.json(&sci, &["start", &id]);
    let v = env.json(&sci, &["show", &id]);
    assert!(v["task"]["completed"].is_null(), "{v}");
    assert_eq!(
        v["periodic"]["last_done"], first,
        "the anchor survives: {v}"
    );
    assert!(v["task"]["started"].is_string());
    std::thread::sleep(std::time::Duration::from_millis(1100));
    env.json(&sci, &["done", &id, "second"]);
    let second = env.json(&sci, &["show", &id])["task"]["completed"].clone();
    assert_ne!(second, first);
    env.json(&sci, &["start", &id]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: doing", "status: done")).unwrap();
    assert!(env.json(&sci, &["show", &id])["task"]["completed"].is_null());
    let out = env
        .cmd(&sci)
        .args(["done", &id, "cleanup"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(
        value["warnings"].to_string().contains("already completed"),
        "{value}"
    );
    assert!(env.json(&sci, &["show", &id])["task"]["completed"].is_null());
}

#[test]
fn done_stamps_completed_and_reopening_clears_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["start", &id]);
    let started = env.json(&sci, &["show", &id])["task"]["started"].clone();
    assert!(started.is_string(), "{started}");
    env.json(&sci, &["done", &id, "first pass"]);
    let v = env.json(&sci, &["show", &id]);
    let first = v["task"]["completed"].clone();
    assert!(first.is_string(), "{v}");
    assert_eq!(v["task"]["started"], started);
    env.json(&sci, &["edit", &id, "--status", "todo"]);
    let v = env.json(&sci, &["show", &id]);
    assert!(v["task"]["completed"].is_null());
    assert_eq!(v["task"]["started"], started);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    env.json(&sci, &["start", &id]);
    env.json(&sci, &["done", &id, "second pass"]);
    assert_ne!(env.json(&sci, &["show", &id])["task"]["completed"], first);
}

#[test]
fn editor_reopen_keeps_the_completed_stamp_and_transition_clears_it() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["done", &id, "landed"]);
    let editor = editor_script(&sci, "sed -i 's/^status: done$/status: todo/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo");
    assert!(v["task"]["completed"].is_null(), "{v}");
}

#[test]
fn editor_refuses_to_set_move_or_clear_a_stamp() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fresh = id_of(env.json(&sci, &["add", "Fresh", "-p", "2"]));
    let editor = editor_script(
        &sci,
        r#"sed -i 's/^updated: \(.*\)$/updated: \1\nstarted: 2026-09-01T00:00:00Z/' "$1""#,
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &fresh])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains("started is stamped by starting the task; it cannot be edited")
    );
    assert!(env.json(&sci, &["show", &fresh])["task"]["started"].is_null());

    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["start", &id]);
    let started = env.json(&sci, &["show", &id])["task"]["started"].clone();
    for (body, expected) in [
        (
            "sed -i 's/^updated: \\(.*\\)$/updated: \\1\\ncompleted: 2026-09-01T00:00:00Z/' \"$1\"",
            "completed is stamped by completing the task; it cannot be edited",
        ),
        (
            "sed -i 's/^started: .*$/started: 2020-01-01T00:00:00Z/' \"$1\"",
            "started is stamped by starting the task; it cannot be edited",
        ),
        (
            "sed -i '/^started: /d' \"$1\"",
            "started is stamped by starting the task; it cannot be edited",
        ),
    ] {
        let editor = editor_script(&sci, body);
        let out = env
            .cmd(&sci)
            .args(["edit", &id])
            .env("EDITOR", &editor)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert_eq!(err_kind(&out), "validation");
        assert!(err_detail(&out).contains(expected));
        assert_eq!(env.json(&sci, &["show", &id])["task"]["started"], started);
    }

    env.json(&sci, &["done", &id, "landed"]);
    let completed = env.json(&sci, &["show", &id])["task"]["completed"].clone();
    assert!(completed.is_string());
    for body in [
        "sed -i 's/^completed: .*$/completed: 2020-01-01T00:00:00Z/' \"$1\"",
        "sed -i '/^completed: /d' \"$1\"",
    ] {
        let editor = editor_script(&sci, body);
        let out = env
            .cmd(&sci)
            .args(["edit", &id])
            .env("EDITOR", &editor)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert_eq!(err_kind(&out), "validation");
        assert!(
            err_detail(&out)
                .contains("completed is stamped by completing the task; it cannot be edited")
        );
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["completed"],
            completed
        );
    }
}

#[test]
fn check_reports_completed_stamp_on_an_open_record() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["done", &id, "landed"]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: done", "status: todo")).unwrap();
    let v = env.check(&sci);
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    let warnings = v["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{v}");
    assert_eq!(warnings[0]["kind"], "completed_stamp_on_open_task");
    assert_eq!(warnings[0]["id"], id);

    let dropped = id_of(env.json(&sci, &["add", "Dropped", "-p", "2"]));
    env.json(&sci, &["done", &dropped, "landed"]);
    let path = sci.join(format!("tasks/{dropped}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("status: done", "status: dropped")).unwrap();
    let v = env.check(&sci);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|warning| {
            warning["kind"] == "completed_stamp_on_open_task" && warning["id"] == dropped
        }),
        "{v}"
    );
}
#[test]
fn non_unicode_tasks_model_fails_the_completion() {
    use std::os::unix::ffi::OsStrExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Bytes", "-p", "2"]));
    let out = env
        .cmd(&sci)
        .args(["done", &id, "x"])
        .env("TASKS_MODEL", std::ffi::OsStr::from_bytes(b"\xff\xfe"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains("TASKS_MODEL is not valid Unicode"),
        "{}",
        err_detail(&out)
    );
    assert!(
        env.json(&sci, &["show", &id])["task"]["model"].is_null(),
        "a failed completion writes nothing"
    );
}

#[test]
fn edit_model_replaces_and_no_model_clears() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Fix", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    env.json(&sci, &["edit", &id, "--model", "B"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "B");
    env.json(&sci, &["edit", &id, "--no-model"]);
    assert!(env.json(&sci, &["show", &id])["task"]["model"].is_null());
    assert_eq!(env.fail(&sci, &["edit", &id, "--model", ""]), "validation");
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--model", "x", "--no-model"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "conflicting flags must fail");
}

#[test]
fn a_completing_edit_stamps_last_over_a_same_invocation_correction() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    for extra in [&["--model", "B"][..], &["--no-model"][..]] {
        let id = id_of(env.json(&sci, &["add", "Both", "-p", "2"]));
        env.cmd(&sci)
            .args(["edit", &id, "--status", "done"])
            .args(extra)
            .env("TASKS_MODEL", "A")
            .assert()
            .success();
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["model"],
            "A",
            "the fresh completion's stamp wins over {extra:?}"
        );
        env.json(&sci, &["edit", &id, "--model", "B"]);
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["model"],
            "B",
            "the correction holds in a following, non-completing edit"
        );
    }

    // The editor equivalent: flipping the status and the model line together still
    // stamps the completion.
    let id = id_of(env.json(&sci, &["add", "BothInEditor", "-p", "2"]));
    let editor = editor_script(
        &sci,
        "sed -i -e 's/^status: todo$/status: done/' -e 's/^tags: \\[\\]$/tags: []\\nmodel: C/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["show", &id])["task"]["model"], "A");
}

#[test]
fn summary_rows_and_parked_rows_carry_model() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let done_id = id_of(env.json(&sci, &["add", "Done", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &done_id, "landed"])
        .env("TASKS_MODEL", "A")
        .assert()
        .success();
    let rows = env.json(&sci, &["list", "--status", "done"]);
    let row = rows["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == done_id)
        .unwrap()
        .clone();
    assert_eq!(row["model"], "A", "list rows carry the stamp");

    let open_id = id_of(env.json(&sci, &["add", "Open", "-p", "2"]));
    let open = env.json(&sci, &["list"]);
    let row = open["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == open_id)
        .unwrap()
        .clone();
    assert!(
        row["model"].is_null(),
        "the key is present and null when absent"
    );

    // A parked row for a previously completed task keeps its attribution.
    env.json(&sci, &["edit", &done_id, "--status", "todo"]);
    env.json(&sci, &["park", &done_id, "resume here"]);
    let prime = env.json(&sci, &["prime"]);
    let parked = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == done_id)
        .unwrap()
        .clone();
    assert_eq!(parked["model"], "A");
}

#[test]
fn show_pretty_prints_the_model_line() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Pretty", "-p", "2"]));
    env.cmd(&sci)
        .args(["done", &id, "landed"])
        .env("TASKS_MODEL", "claude-fable-5-1")
        .assert()
        .success();
    let out = env
        .cmd(&sci)
        .args(["--pretty", "show", &id])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(text.contains("model: claude-fable-5-1\n"), "{text}");
}

#[test]
fn completing_a_recurrence_anchors_and_notes_every_completion_path() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    for (title, path) in [("Command", "done"), ("Flag", "flag"), ("Editor", "editor")] {
        let id = id_of(env.json(&sci, &["add", title, "--every", "30d"]));
        env.json(&sci, &["start", &id]);
        match path {
            "done" => {
                env.json(&sci, &["done", &id, "landed"]);
            }
            "flag" => {
                env.json(&sci, &["edit", &id, "--status", "done"]);
            }
            _ => {
                let editor = editor_script(&sci, "sed -i 's/^status: doing$/status: done/' \"$1\"");
                env.cmd(&sci)
                    .env("EDITOR", editor)
                    .args(["edit", &id])
                    .assert()
                    .success();
            }
        }
        let file = env.read(&sci, &format!("tasks/{id}.md"));
        assert!(file.contains("last_done: 2"), "{path}: {file}");
        assert_eq!(
            file.matches("completed; next due ").count(),
            1,
            "{path}: {file}"
        );
        if path == "done" {
            assert!(file.find("completed; next due ").unwrap() < file.find("landed").unwrap());
        }
    }
}

#[test]
fn ordinary_completion_paths_and_same_status_editor_do_not_create_an_anchor() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    for (title, path) in [("Command", "done"), ("Flag", "flag"), ("Editor", "editor")] {
        let id = id_of(env.json(&sci, &["add", title]));
        if path == "done" {
            env.json(&sci, &["done", &id]);
        } else if path == "flag" {
            env.json(&sci, &["edit", &id, "--status", "done"]);
        } else {
            let editor = editor_script(&sci, "sed -i 's/^status: todo$/status: done/' \"$1\"");
            env.cmd(&sci)
                .env("EDITOR", editor)
                .args(["edit", &id])
                .assert()
                .success();
        }
        let file = env.read(&sci, &format!("tasks/{id}.md"));
        assert!(
            !file.contains("last_done:") && !file.contains("completed; next due"),
            "{file}"
        );
    }

    let id = id_of(env.json(&sci, &["add", "Periodic", "--every", "30d"]));
    seed_anchor(&sci, &id, "2026-01-02T03:04:05Z");
    let before = env.read(&sci, &format!("tasks/{id}.md"));
    let editor = editor_script(&sci, "sed -i 's/^title: Periodic$/title: Renamed/' \"$1\"");
    env.cmd(&sci)
        .env("EDITOR", editor)
        .args(["edit", &id])
        .assert()
        .success();
    let after = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(after.contains("last_done: 2026-01-02T03:04:05Z"));
    assert_eq!(
        after.matches("completed; next due ").count(),
        before.matches("completed; next due ").count()
    );
}

#[test]
fn recurrence_records_each_cycle_and_refuses_an_unclaimed_reclose() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    for cycle in 0..3 {
        env.json(&sci, &["start", &id]);
        env.json(&sci, &["done", &id]);
        if cycle < 2 {
            env.json(&sci, &["edit", &id, "--status", "todo"]);
        }
    }
    let file = env.read(&sci, &format!("tasks/{id}.md"));
    assert_eq!(file.matches("completed; next due ").count(), 3, "{file}");
    let before = file;
    assert_eq!(env.fail(&sci, &["done", &id]), "validation");
    assert_eq!(env.read(&sci, &format!("tasks/{id}.md")), before);
    env.json(&sci, &["edit", &id, "--status", "done", "-p", "1"]);
    let after = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(after.contains("priority: 1"), "{after}");
    assert_eq!(after.matches("completed; next due ").count(), 3, "{after}");
}

#[test]
fn cleanup_retry_releases_only_entries_owned_by_this_checkout() {
    let mut env = TestEnv::new();
    for (pending, prefix) in [("claim", "sci"), ("park", "fam")] {
        let a = env.init(prefix);
        let b = env.init_forced(prefix);
        let id = id_of(env.json(&a, &["add", pending, "--every", "30d"]));
        as_agent(&env, &a, "same")
            .args(["start", &id])
            .assert()
            .success();
        as_agent(&env, &a, "same")
            .args(["done", &id])
            .assert()
            .success();
        std::fs::copy(
            a.join(format!("tasks/{id}.md")),
            b.join(format!("tasks/{id}.md")),
        )
        .unwrap();
        env.json(&a, &["edit", &id, "--status", "todo"]);
        as_agent(&env, &a, "same")
            .args(["start", &id])
            .assert()
            .success();
        if pending == "park" {
            as_agent(&env, &a, "same")
                .args(["park", &id, "continue"])
                .assert()
                .success();
        }
        let record_before = std::fs::read(b.join(format!("tasks/{id}.md"))).unwrap();
        let store_before = std::fs::read(env.claim_store(prefix)).unwrap();
        let out = as_agent(&env, &b, "same")
            .args(["done", &id])
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "validation");
        assert!(String::from_utf8_lossy(&out.stderr).contains(a.to_str().unwrap()));
        assert_eq!(
            std::fs::read(b.join(format!("tasks/{id}.md"))).unwrap(),
            record_before
        );
        assert_eq!(
            std::fs::read(env.claim_store(prefix)).unwrap(),
            store_before
        );
    }
}

#[test]
fn cleanup_retry_releases_this_checkouts_claim_without_a_second_occurrence() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.cmd(&sci)
        .env("CODEX_SESSION_ID", "native-a")
        .args(["start", &id])
        .assert()
        .success();
    let notes_before = env.json(&sci, &["show", &id])["task"]["notes"].clone();
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("status: doing", "status: done")
            .replace("updated: ", "last_done: 2026-01-01T00:00:00Z\nupdated: "),
    )
    .unwrap();

    // The same Codex session retries: the thread id is its claim identity.
    let out = env
        .cmd(&sci)
        .env("CODEX_SESSION_ID", "native-a")
        .args(["done", &id, "landed"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        value["warnings"].to_string().contains("already completed"),
        "{value}"
    );
    assert!(
        !std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains(&id)
    );
    let file = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(file.contains("last_done: 2026-01-01T00:00:00Z"), "{file}");
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["notes"],
        notes_before
    );
    assert!(
        !file.contains("completed; next due") && !file.contains("landed"),
        "{file}"
    );
}

#[test]
fn every_sets_and_no_every_clears_the_cadence() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let id = id_of(env.json(&sci, &["add", "Curation sweep", "--every", "30d"]));
    let file = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(file.contains("every: 30d\n"), "{file}");
    assert!(
        !file.contains("last_done:"),
        "no anchor before a completion: {file}"
    );

    env.json(&sci, &["edit", &id, "--every", "2w"]);
    assert!(
        env.read(&sci, &format!("tasks/{id}.md"))
            .contains("every: 2w\n")
    );

    assert_eq!(
        env.fail(&sci, &["edit", &id, "--every", "30m"]),
        "validation"
    );
    assert_eq!(
        env.fail(&sci, &["edit", &id, "--every", "0d"]),
        "validation"
    );
    assert!(
        env.read(&sci, &format!("tasks/{id}.md"))
            .contains("every: 2w\n")
    );

    seed_anchor(&sci, &id, "2026-01-02T03:04:05Z");
    let anchored = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        anchored.contains("last_done: 2026-01-02"),
        "seeded: {anchored}"
    );
    env.json(&sci, &["edit", &id, "--no-every"]);
    let cleared = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        !cleared.contains("every:") && !cleared.contains("last_done:"),
        "{cleared}"
    );

    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--every", "30d", "--no-every"])
        .output()
        .unwrap();
    assert!(!out.status.success());

    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--every", ""]),
        vec!["7d", "14d", "30d", "90d"]
    );
}

#[test]
fn an_editor_save_cannot_set_or_move_the_anchor() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    seed_anchor(&sci, &id, "2026-01-02T03:04:05Z");
    let before = env.read(&sci, &format!("tasks/{id}.md"));

    let move_it = editor_script(
        &sci,
        "sed -i 's/^last_done: .*/last_done: 2020-01-01T00:00:00Z/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .env("EDITOR", &move_it)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(env.read(&sci, &format!("tasks/{id}.md")), before);

    let recadence = editor_script(&sci, "sed -i 's/^every: 30d$/every: 7d/' \"$1\"");
    env.cmd(&sci)
        .env("EDITOR", &recadence)
        .args(["edit", &id])
        .assert()
        .success();
    let after = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(after.contains("every: 7d\n"), "{after}");
    assert!(
        after.contains("last_done: 2026-01-02T03:04:05Z"),
        "anchor preserved: {after}"
    );

    let clear = editor_script(&sci, "sed -i '/^every: /d; /^last_done: /d' \"$1\"");
    env.cmd(&sci)
        .env("EDITOR", &clear)
        .args(["edit", &id])
        .assert()
        .success();
    let cleared = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        !cleared.contains("every:") && !cleared.contains("last_done:"),
        "{cleared}"
    );

    let orphan = editor_script(&sci, "sed -i '/^every: /d' \"$1\"");
    let id2 = id_of(env.json(&sci, &["add", "Second sweep", "--every", "30d"]));
    seed_anchor(&sci, &id2, "2026-01-02T03:04:05Z");
    let out = env
        .cmd(&sci)
        .env("EDITOR", &orphan)
        .args(["edit", &id2])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn edit_flags_update_fields_and_enforce_rules() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let dep = env.json(&dir, &["add", "Dep"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = env.json(&dir, &["add", "A", "--depends", &dep])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &dir,
        &[
            "edit", &id, "--title", "A2", "-p", "0", "--size", "xl", "--tag", "t1",
        ],
    );
    let s = env.json(&dir, &["show", &id]);
    assert_eq!(s["task"]["title"], "A2");
    assert_eq!(s["task"]["priority"], 0);
    assert_eq!(s["task"]["size"], "xl");
    assert_eq!(env.fail(&dir, &["edit", &id, "--force"]), "validation");
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--status", "todo", "--force"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--status", "done"]),
        "open_dependencies"
    );
    let dep2 = env.json(&dir, &["add", "Dep2"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["done", &dep]);
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--status", "done", "--depends", &dep2]),
        "open_dependencies"
    );
    env.json(&dir, &["edit", &id, "--status", "done", "--force"]);
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--status", "doing"]),
        "invalid_transition"
    );
    env.json(&dir, &["edit", &id, "--status", "todo"]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "todo");
    env.cmd(&dir)
        .args(["edit", &id, "--body", "-"])
        .write_stdin("New body\n")
        .assert()
        .success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["body"], "New body");
    write_doc(&dir, "docs/plans/2026-08-24-one.md", "### Task 1: keep\n");
    write_doc(
        &dir,
        "docs/plans/2026-08-25-other.md",
        "### Task 7: unrelated\n",
    );
    let linked = env.json(
        &dir,
        &["add", "Linked", "--plan", "one", "--step", "Task 1: keep"],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        env.fail(&dir, &["edit", &linked, "--plan", "other"]),
        "validation"
    );
}

#[test]
fn edit_editor_path_validates_and_is_atomic() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = env.json(&dir, &["add", "A", "-b", "Body"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["note", &id, "n1"]);
    let before = env.read(&dir, &format!("tasks/{id}.md"));

    let bad = editor_script(&dir, "echo garbage > \"$1\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &bad)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "parse");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(".edit.md")
    );
    assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), before);
    let kept = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".edit.md")
        })
        .count();
    assert_eq!(kept, 1);

    let tamper = editor_script(
        &dir,
        "sed -i 's/^created: .*/created: 2000-01-01T00:00:00Z/' \"$1\"",
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &tamper)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));

    let notes = editor_script(&dir, "sed -i 's/): n1$/): hacked/' \"$1\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &notes)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));

    let good = editor_script(&dir, "sed -i 's/^title: A$/title: Edited/' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &good)
        .args(["edit", &id])
        .assert()
        .success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["title"], "Edited");
    let kept = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".edit.md")
        })
        .count();
    assert_eq!(
        kept, 3,
        "the three failed edits keep their temp files; the good one removes its own"
    );

    let fail_save = editor_script(&dir, "chmod 555 \"$(dirname \"$1\")\"");
    let out = env
        .cmd(&dir)
        .env("EDITOR", &fail_save)
        .args(["edit", &id])
        .output()
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    let tasks_dir = dir.join("tasks");
    let mut permissions = std::fs::metadata(&tasks_dir).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&tasks_dir, permissions).unwrap();
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "io");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(".edit.md")
    );

    let task_path = dir.join(format!("tasks/{id}.md")).display().to_string();
    let racy = editor_script(
        &dir,
        &format!(
            "sed -i 's/^title: .*/title: Racer/' \"{task_path}\"; sed -i 's/^title: .*/title: Loser/' \"$1\""
        ),
    );
    let out = env
        .cmd(&dir)
        .env("EDITOR", &racy)
        .args(["edit", &id])
        .output()
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "concurrent_modification");
    assert_eq!(env.json(&dir, &["show", &id])["task"]["title"], "Racer");

    let out = env
        .cmd(&dir)
        .env_remove("EDITOR")
        .args(["edit", &id])
        .output()
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "editor");
}

#[test]
fn edit_reports_deleted_original_as_concurrent_modification_and_keeps_edit() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = env.json(&dir, &["add", "Original"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task_path = dir.join(format!("tasks/{id}.md"));
    let editor = editor_script(
        &dir,
        &format!(
            "rm \"{}\"\nsed -i 's/^title: Original$/title: Edited/' \"$1\"",
            task_path.display()
        ),
    );

    let out = env
        .cmd(&dir)
        .env("EDITOR", editor)
        .args(["edit", &id])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "concurrent_modification");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(".edit.md")
    );
    assert!(!task_path.exists());
    let kept = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".edit.md"))
        .unwrap();
    assert!(
        std::fs::read_to_string(kept)
            .unwrap()
            .contains("title: Edited")
    );
}

#[test]
fn dep_add_remove_and_local_cycle() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let c = env.json(&dir, &["add", "C"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["dep", &b, "--on", &a]);
    env.json(&dir, &["dep", &c, "--on", &b]);
    assert_eq!(env.fail(&dir, &["dep", &a, "--on", &c]), "cycle");
    assert_eq!(env.fail(&dir, &["dep", &a, "--on", &a]), "cycle");
    assert_eq!(
        env.fail(&dir, &["dep", &a, "--on", "sci-ffffff"]),
        "unresolvable_id"
    );
    assert_eq!(
        env.json(&dir, &["show", &c])["task"]["depends"],
        serde_json::json!([b])
    );
    env.json(&dir, &["dep", &c, "--rm", &b]);
    assert!(
        env.json(&dir, &["show", &c])["task"]
            .get("depends")
            .is_none()
    );
    assert_eq!(env.fail(&dir, &["dep", &c, "--rm", &b]), "validation");
    env.json(&dir, &["dep", &b, "--on", &a]);
    assert_eq!(
        env.json(&dir, &["show", &b])["task"]["depends"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn cross_project_cycle_is_rejected_and_unreachable_blocks_link() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let s1 = env.json(&sci, &["add", "S1"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let f1 = env.json(&fam, &["add", "F1", "--depends", &s1])["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(env.fail(&sci, &["dep", &s1, "--on", &f1]), "cycle");
    let s2 = env.json(&sci, &["add", "S2"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let raw = env.read(&fam, &format!("tasks/{f1}.md"));
    std::fs::write(
        fam.join(format!("tasks/{f1}.md")),
        raw.replace(&format!("depends: [{s1}]"), "depends: [zzz-000001]"),
    )
    .unwrap();
    assert_eq!(
        env.fail(&sci, &["dep", &s2, "--on", &f1]),
        "unresolvable_id"
    );
}

#[test]
fn graph_renders_open_tasks() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "--depends", &a])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["done", &a]);
    let value = env.json(&dir, &["graph"]);
    assert_eq!(value["format"], "mermaid");
    let text = value["text"].as_str().unwrap();
    assert!(text.contains(&b) && !text.contains(&a), "{text}");
    let value = env.json(&dir, &["graph", "--all", "--format", "dot"]);
    assert!(value["text"].as_str().unwrap().contains(&a));
    let err = env.usage(&dir, &["graph", "--format", "png"]);
    assert!(err.contains("--format") && err.contains("png"), "{err}");
}

#[test]
fn check_passes_clean_repo_and_reports_drift() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/plans/2026-08-29-p.md", "### Task 1: one\n");
    let a = env.json(
        &dir,
        &[
            "add",
            "A",
            "--plan",
            "p",
            "--step",
            "Task 1: one",
            "--complexity",
            "low",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "--depends", &a])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let v = env.check(&dir);
    assert_eq!(v["errors"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!([]));

    // drift: heading renamed; dangling dep; garbage file; foreign unreachable dep
    write_doc(&dir, "docs/plans/2026-08-29-p.md", "### Task 1: uno\n");
    let raw = env.read(&dir, &format!("tasks/{b}.md"));
    std::fs::write(
        dir.join(format!("tasks/{b}.md")),
        raw.replace(
            &format!("depends: [{a}]"),
            &format!("depends: [{a}, sci-ffffff, zzz-000001]"),
        ),
    )
    .unwrap();
    std::fs::write(dir.join("tasks/sci-bad.md"), "nope").unwrap();
    std::fs::write(
        dir.join("tasks/sci-abcdef.md"),
        "---\nnot: frontmatter\n---\n",
    )
    .unwrap();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<&str> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"step_missing"), "{kinds:?}");
    assert!(kinds.contains(&"dangling_dep"), "{kinds:?}");
    assert_eq!(
        kinds.iter().filter(|k| **k == "parse").count(),
        2,
        "{kinds:?}"
    );
    let wkinds: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(wkinds.contains(&"unreachable_dep"), "{wkinds:?}");
    assert!(wkinds.contains(&"unlinked_step"), "{wkinds:?}");
}

#[test]
fn check_holds_only_open_records_to_their_doc_links() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/specs/2026-08-29-s.md", "# S\n");
    write_doc(
        &dir,
        "docs/plans/2026-08-29-p.md",
        "### Task 1: one\n### Task 2: two\n### Task 3: three\n",
    );
    let linked = |env: &TestEnv, title: &str, step: &str| {
        id_of(env.json(
            &dir,
            &[
                "add",
                title,
                "--spec",
                "s",
                "--plan",
                "p",
                "--step",
                step,
                "--complexity",
                "low",
            ],
        ))
    };
    let dropped = linked(&env, "Dropped", "Task 1: one");
    let done = linked(&env, "Done", "Task 2: two");
    let shelved = linked(&env, "Shelved", "Task 3: three");
    env.json(&dir, &["drop", &dropped, "merged away"]);
    env.json(&dir, &["done", &done, "landed"]);
    env.json(&dir, &["shelve", &shelved, "later"]);

    // The plan revision merges every heading away and the spec is retired.
    write_doc(&dir, "docs/plans/2026-08-29-p.md", "### Task 1: all\n");
    std::fs::remove_file(dir.join("docs/specs/2026-08-29-s.md")).unwrap();

    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let flagged: Vec<(&str, &str)> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["id"].as_str().unwrap(), e["kind"].as_str().unwrap()))
        .collect();
    // Closed records are history; the shelved one is still open work.
    assert_eq!(
        flagged,
        vec![
            (shelved.as_str(), "doc_missing"),
            (shelved.as_str(), "step_missing"),
        ],
        "{flagged:?}"
    );
}

#[test]
fn edit_clears_spec_plan_and_step_links() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/specs/2026-08-29-s.md", "# S\n");
    write_doc(
        &dir,
        "docs/plans/2026-08-29-p.md",
        "### Task 1: one\n### Task 2: two\n",
    );
    let linked = |env: &TestEnv, title: &str, step: &str| {
        id_of(env.json(
            &dir,
            &["add", title, "--spec", "s", "--plan", "p", "--step", step],
        ))
    };
    let id = linked(&env, "Goal", "Task 1: one");
    let task = |env: &TestEnv, id: &str| env.json(&dir, &["show", id])["task"].clone();

    // A plan cannot go while a step still points into it; nothing is written.
    let before = env.read(&dir, &format!("tasks/{id}.md"));
    assert_eq!(env.fail(&dir, &["edit", &id, "--no-plan"]), "validation");
    assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), before);

    env.json(&dir, &["edit", &id, "--no-step"]);
    let shown = task(&env, &id);
    assert!(shown.get("step").is_none(), "{shown}");
    assert_eq!(shown["plan"], "docs/plans/2026-08-29-p.md");

    env.json(&dir, &["edit", &id, "--no-plan", "--no-spec"]);
    let shown = task(&env, &id);
    assert!(shown.get("plan").is_none(), "{shown}");
    assert!(shown.get("spec").is_none(), "{shown}");
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    for key in ["spec:", "plan:", "step:"] {
        assert!(!raw.contains(key), "{raw}");
    }

    // The escape hatch for a link whose heading is already gone, open or closed.
    let open = linked(&env, "Open", "Task 1: one");
    let dropped = linked(&env, "Dropped", "Task 2: two");
    env.json(&dir, &["drop", &dropped, "merged away"]);
    write_doc(&dir, "docs/plans/2026-08-29-p.md", "### Task 1: all\n");
    env.json(&dir, &["edit", &open, "--no-step"]);
    env.json(&dir, &["edit", &dropped, "--no-plan", "--no-step"]);
    assert!(task(&env, &open).get("step").is_none());
    assert!(task(&env, &dropped).get("plan").is_none());
    let check = env.check(&dir);
    assert_eq!(check["errors"], serde_json::json!([]), "{check}");

    // clap rejects each setter beside its clearer before any command runs
    for pair in [
        ["--spec", "s", "--no-spec"],
        ["--plan", "p", "--no-plan"],
        ["--step", "Task 1: all", "--no-step"],
    ] {
        env.cmd(&dir)
            .args(["edit", &open].iter().copied().chain(pair))
            .assert()
            .code(2);
    }
}

#[test]
fn check_prints_nothing_on_a_clean_repo_and_everything_otherwise() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    // pretty mode is for a person who cannot see the exit status
    let out = env.cmd(&dir).args(["--pretty", "check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");

    // warnings alone still exit 0, so they must print or they are lost
    write_doc(&dir, "docs/plans/2026-09-19-p.md", "### Task 1: nobody\n");
    env.json(
        &dir,
        &["edit", &a, "--plan", "p", "--step", "Task 1: nobody"],
    );
    write_doc(
        &dir,
        "docs/plans/2026-09-19-p.md",
        "### Task 1: nobody\n\n### Task 2: unlinked\n",
    );
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"], serde_json::json!([]));
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unlinked_step"),
        "{v}"
    );

    // errors print and fail
    std::fs::write(dir.join("tasks/sci-bad.md"), "nope").unwrap();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"].as_array().unwrap().len(), 1, "{v}");
}

#[test]
fn check_warns_on_plan_headings_without_a_task() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(
        &dir,
        "docs/plans/2026-09-03-p.md",
        "# Plan\n\n## Overview\n\n### Task 1: one\n\n### Task 2: two\n\n### Notes on Task 3\n",
    );
    write_doc(
        &dir,
        "docs/plans/2026-09-03-unlinked.md",
        "### Task 1: nobody\n",
    );
    let a = env.json(&dir, &["add", "A", "--plan", "p", "--step", "Task 1: one"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let check = env.check(&dir);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warnings = check["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 2, "{check}");

    // Verify the unlinked_step finding matches the original test's exact assertions
    let unlinked = warnings
        .iter()
        .find(|w| w["kind"] == "unlinked_step")
        .expect("unlinked_step warning missing");
    assert_eq!(unlinked["kind"], "unlinked_step");
    assert_eq!(unlinked["file"], "docs/plans/2026-09-03-p.md");
    assert_eq!(unlinked["id"], serde_json::Value::Null);
    assert!(unlinked["detail"].as_str().unwrap().contains("Task 2: two"));

    // Verify the unrated_step finding for the plan step task
    let unrated = warnings
        .iter()
        .find(|w| w["kind"] == "unrated_step")
        .expect("unrated_step warning missing");
    assert_eq!(unrated["kind"], "unrated_step");
    assert_eq!(unrated["id"], a);
}

#[test]
fn check_reports_unparsable_foreign_dependency_as_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let f = env.json(&fam, &["add", "F"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&sci, &["add", "S", "--depends", &f]);
    std::fs::write(fam.join(format!("tasks/{f}.md")), "garbage").unwrap();
    let v = env.check(&sci); // exit 0: warnings only
    assert_eq!(v["errors"], serde_json::json!([]));
    let wkinds: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(wkinds.contains(&"foreign_unparsable"), "{wkinds:?}");
    assert!(wkinds.contains(&"cycle_unverifiable"), "{wkinds:?}");
}

#[test]
fn check_nudges_a_depends_naming_a_retired_prefix() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let far = id_of(env.json(&fam, &["add", "Far", "-p", "2"]));
    let here = id_of(env.json(&sci, &["add", "Here", "-p", "2"]));
    env.json(&sci, &["dep", &here, "--on", &far]);
    alias_registry(&env, "old", "fam");

    let path = sci.join(format!("tasks/{here}.md"));
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("fam-", "old-");
    std::fs::write(&path, text).unwrap();

    let v = env.check(&sci);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            w["kind"] == "retired_prefix" && w["detail"].as_str().unwrap().contains(&far)
        }),
        "{v}"
    );
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    assert_eq!(env.check(&fam)["warnings"], serde_json::json!([]));
}

#[test]
fn check_does_not_call_an_existing_malformed_local_dependency_dangling() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let dependency = env.json(&dir, &["add", "Dependency"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&dir, &["add", "Dependent", "--depends", &dependency]);
    std::fs::write(dir.join(format!("tasks/{dependency}.md")), "malformed").unwrap();

    let out = env.cmd(&dir).args(["check"]).output().unwrap();

    assert_eq!(out.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let errors = value["errors"].as_array().unwrap();
    assert!(errors.iter().any(|finding| finding["kind"] == "parse"));
    assert!(
        !errors
            .iter()
            .any(|finding| finding["kind"] == "dangling_dep")
    );
    assert!(
        value["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["kind"] == "cycle_unverifiable")
    );
}

#[test]
fn check_reports_cycles_once() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "--depends", &a])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let raw = env.read(&dir, &format!("tasks/{a}.md"));
    std::fs::write(
        dir.join(format!("tasks/{a}.md")),
        raw.replace("depends: []", &format!("depends: [{b}]")),
    )
    .unwrap();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let cycles: Vec<&serde_json::Value> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "cycle")
        .collect();
    assert_eq!(cycles.len(), 1, "{:?}", v["errors"]);
    // pretty mode lists findings one per line
    let out = env.cmd(&dir).args(["--pretty", "check"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("cycle"));
}

#[test]
fn check_finds_cycle_after_missing_dependency() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = env.json(&dir, &["add", "A"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let raw = env.read(&dir, &format!("tasks/{a}.md"));
    std::fs::write(
        dir.join(format!("tasks/{a}.md")),
        raw.replace("depends: []", &format!("depends: [sci-ffffff, {b}]")),
    )
    .unwrap();
    let raw = env.read(&dir, &format!("tasks/{b}.md"));
    std::fs::write(
        dir.join(format!("tasks/{b}.md")),
        raw.replace("depends: []", &format!("depends: [{a}]")),
    )
    .unwrap();

    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let errors = value["errors"].as_array().unwrap();
    assert!(
        errors
            .iter()
            .any(|finding| finding["kind"] == "dangling_dep")
    );
    assert!(errors.iter().any(|finding| finding["kind"] == "cycle"));
    assert!(
        value["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["kind"] == "cycle_unverifiable")
    );
}

#[test]
fn check_reports_a_cycle_at_its_lowest_member() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let mut ids = [
        env.json(&dir, &["add", "A"])["id"]
            .as_str()
            .unwrap()
            .to_string(),
        env.json(&dir, &["add", "B"])["id"]
            .as_str()
            .unwrap()
            .to_string(),
        env.json(&dir, &["add", "C"])["id"]
            .as_str()
            .unwrap()
            .to_string(),
    ];
    ids.sort();
    for (id, dependency) in [(&ids[0], &ids[1]), (&ids[1], &ids[2]), (&ids[2], &ids[1])] {
        let raw = env.read(&dir, &format!("tasks/{id}.md"));
        std::fs::write(
            dir.join(format!("tasks/{id}.md")),
            raw.replace("depends: []", &format!("depends: [{dependency}]")),
        )
        .unwrap();
    }

    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let cycle = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == "cycle")
        .unwrap();
    assert_eq!(cycle["id"], ids[1]);
}

#[test]
fn closing_rules_walk_descendants() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    let c = id_of(env.json(&dir, &["add", "C", "--parent", &b]));
    assert_eq!(env.fail(&dir, &["done", &a]), "open_descendants");
    assert_eq!(env.fail(&dir, &["drop", &a]), "open_descendants");
    env.json(&dir, &["done", &b, "forced past c", "--force"]);
    assert_eq!(
        env.fail(&dir, &["done", &a]),
        "open_descendants",
        "c is still open under the force-closed b"
    );
    let out = env
        .cmd(&dir)
        .args(["drop", &a, "--force"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "drop has no override flag");
    env.json(&dir, &["done", &c]);
    env.json(&dir, &["done", &a]);
    assert_eq!(env.json(&dir, &["show", &a])["task"]["status"], "done");
}

#[test]
fn ready_never_lists_a_task_with_children_and_summaries_carry_counts() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let leaf = id_of(env.json(&dir, &["add", "Leaf", "--parent", &goal]));
    let ready = env.json(&dir, &["ready"]);
    let ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [leaf.as_str()]);
    let list = env.json(&dir, &["list"]);
    let goal_row = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == goal)
        .unwrap();
    assert_eq!(goal_row["parent"], serde_json::Value::Null);
    assert_eq!(goal_row["child_count"], 1);
    assert_eq!(goal_row["open_descendant_count"], 1);
    let leaf_row = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == leaf)
        .unwrap();
    assert_eq!(leaf_row["parent"], goal);
    assert_eq!(leaf_row["child_count"], 0);
    env.json(&dir, &["done", &leaf]);
    let ready = env.json(&dir, &["ready"]);
    assert_eq!(
        ready["tasks"].as_array().unwrap().len(),
        0,
        "a parent is never ready"
    );
}

#[test]
fn check_warns_on_open_child_of_closed_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A"]));
    let b = id_of(env.json(&dir, &["add", "B", "--parent", &a]));
    env.json(&dir, &["done", &b]);
    env.json(&dir, &["done", &a]);
    env.json(&dir, &["edit", &b, "--status", "todo"]);
    let check = env.check(&dir);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warnings = check["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{check}");
    let warning = &warnings[0];
    assert_eq!(warning["kind"], "open_child_of_closed_parent");
    assert_eq!(warning["id"], b);
}

#[test]
fn tree_nests_prunes_and_orders() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "1"]));
    let big = id_of(env.json(&dir, &["add", "Big", "--parent", &goal, "--size", "l"]));
    let small = id_of(env.json(&dir, &["add", "Small", "--parent", &goal, "--size", "xs"]));
    let deep = id_of(env.json(&dir, &["add", "Deep", "--parent", &big]));
    let loner = id_of(env.json(&dir, &["add", "Loner", "-p", "3"]));
    env.json(&dir, &["done", &small]);

    let tree = env.json(&dir, &["tree"]);
    let nodes = tree["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["id"], goal);
    assert_eq!(nodes[1]["id"], loner);
    let goal_children = nodes[0]["children"].as_array().unwrap();
    assert_eq!(goal_children.len(), 1, "closed Small is pruned: {tree}");
    assert_eq!(goal_children[0]["id"], big);
    assert_eq!(goal_children[0]["children"][0]["id"], deep);
    assert_eq!(nodes[0]["child_count"], 2);
    assert_eq!(nodes[0]["open_descendant_count"], 2);

    let all = env.json(&dir, &["tree", "--all"]);
    let goal_children = all["nodes"][0]["children"].as_array().unwrap();
    let ids: Vec<&str> = goal_children
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [small.as_str(), big.as_str()],
        "ready order: xs before l"
    );

    let sub = env.json(&dir, &["tree", &big]);
    assert_eq!(sub["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(sub["nodes"][0]["id"], big);
    assert_eq!(env.fail(&dir, &["tree", "sci-ffffff"]), "task_not_found");

    // an open task under a closed parent stays visible with its closed ancestor
    env.json(&dir, &["done", &deep]);
    env.json(&dir, &["done", &big]);
    env.json(&dir, &["edit", &deep, "--status", "todo"]);
    let tree = env.json(&dir, &["tree"]);
    let big_node = &tree["nodes"][0]["children"][0];
    assert_eq!(big_node["id"], big);
    assert_eq!(big_node["status"], "done");
    assert_eq!(big_node["children"][0]["id"], deep);

    let out = env.cmd(&dir).args(["--pretty", "tree"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("{goal}  P1")), "{text}");
    assert!(
        text.contains(&format!("  {big}  P2")),
        "children are indented: {text}"
    );
}

#[test]
fn tree_all_projects_groups_by_project_in_registry_order() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    // sci's root outranks fam's, but registry order (fam before sci) wins
    let s_goal = id_of(env.json(&sci, &["add", "S goal", "-p", "0"]));
    let s_child = id_of(env.json(&sci, &["add", "S child", "--parent", &s_goal]));
    let f_goal = id_of(env.json(&fam, &["add", "F goal", "-p", "3"]));
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["tree", "--all-projects"]);
    let nodes = v["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "{v}");
    assert_eq!(nodes[0]["id"], f_goal);
    assert_eq!(nodes[1]["id"], s_goal);
    assert_eq!(nodes[1]["children"][0]["id"], s_child);

    let out = env
        .cmd(nowhere.path())
        .args(["tree", &s_goal, "--all-projects"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "id and --all-projects conflict");
}

#[test]
fn orphaned_tasks_are_roots_in_tree_and_roadmap() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let mid = id_of(env.json(&dir, &["add", "Mid", "--parent", &goal]));
    let kid = id_of(env.json(&dir, &["add", "Kid", "--parent", &mid]));
    std::fs::remove_file(dir.join(format!("tasks/{goal}.md"))).unwrap();
    let tree = env.json(&dir, &["tree"]);
    assert_eq!(tree["nodes"][0]["id"], mid, "{tree}");
    assert_eq!(tree["nodes"][0]["children"][0]["id"], kid, "{tree}");
    let prime = env.json(&dir, &["prime"]);
    assert_eq!(prime["roadmap"][0]["id"], mid, "{prime}");
    // kid's own parent (mid) still exists; the walk only runs into the missing
    // grandparent (goal), so the write still succeeds.
    env.json(&dir, &["note", &kid, "still writable"]);
}

#[test]
fn show_reports_parent_and_children_and_list_filters_by_parent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    // priorities pin the order: children are reported in ready order, not id order
    let two = id_of(env.json(&dir, &["add", "Two", "--parent", &goal, "-p", "2"]));
    let one = id_of(env.json(&dir, &["add", "One", "--parent", &goal, "-p", "1"]));
    let other = id_of(env.json(&dir, &["add", "Other"]));
    let shown = env.json(&dir, &["show", &one]);
    assert_eq!(shown["parent"]["id"], goal);
    assert_eq!(shown["parent"]["title"], "Goal");
    assert_eq!(shown["parent"]["status"], "todo");
    assert!(shown.get("children").is_none());
    let shown = env.json(&dir, &["show", &goal]);
    assert_eq!(shown["parent"], serde_json::Value::Null);
    let kids: Vec<&str> = shown["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(kids, [one.as_str(), two.as_str()]);
    let filtered = env.json(&dir, &["list", "--parent", &goal]);
    let ids: Vec<&str> = filtered["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&other.as_str()));
    assert_eq!(
        env.fail(&dir, &["list", "--parent", "sci-ffffff"]),
        "task_not_found"
    );

    let colored = |id: &str| {
        let out = env
            .cmd(&dir)
            .args(["--pretty", "--color", "always", "show", id])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    };
    let text = colored(&goal);
    assert!(
        text.contains(&format!("\x1b[2m{one}\x1b[0m [todo] One")),
        "{text:?}"
    );
    assert!(
        text.contains(&format!("\x1b[2m{two}\x1b[0m [todo] Two")),
        "{text:?}"
    );

    env.json(&dir, &["block", &two, "waiting"]);
    let text = colored(&goal);
    assert!(
        text.contains(&format!("\x1b[2m{two}\x1b[0m [\x1b[31mblocked\x1b[0m] Two")),
        "footer statuses use the same role as tables: {text:?}"
    );
    let text = colored(&one);
    assert!(
        text.contains(&format!("\x1b[2m{goal}\x1b[0m [todo] Goal")),
        "{text:?}"
    );
}

#[test]
fn show_warns_when_the_parent_is_missing_from_the_scan() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal"]));
    let kid = id_of(env.json(&dir, &["add", "Kid", "--parent", &goal]));
    std::fs::remove_file(dir.join(format!("tasks/{goal}.md"))).unwrap();
    let out = env.cmd(&dir).args(["show", &kid]).output().unwrap();
    let shown: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(shown["parent"], serde_json::Value::Null, "{shown}");
    assert_eq!(
        shown["warnings"],
        serde_json::json!([format!("parent {goal} not found")]),
        "{shown}"
    );
}

fn feedback_env() -> (TestEnv, std::path::PathBuf, std::path::PathBuf) {
    let mut env = TestEnv::new();
    let target = env.init("tasks");
    accept_feedback(&target, "The tasks CLI.");
    let reporter = env.init("sci");
    (env, target, reporter)
}

#[test]
fn feedback_creates_an_idea_in_the_registered_tasks_project() {
    let (env, target, reporter) = feedback_env();
    let out = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects a spec outside the roots",
            "--category",
            "friction",
            "-b",
            "expected a hint naming the configured roots",
        ],
    );
    assert_eq!(out["action"], "created");
    let id = out["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("tasks-"), "{id}");
    let path = out["path"].as_str().unwrap();
    assert!(std::path::Path::new(path).is_file());
    assert!(path.starts_with(target.to_str().unwrap()), "{path}");
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings[0].as_str().unwrap().contains("uncommitted"),
        "{out}"
    );

    let shown = env.json(&target, &["show", &id]);
    assert_eq!(shown["task"]["status"], "idea");
    assert_eq!(
        shown["task"]["title"],
        "check rejects a spec outside the roots"
    );
    assert_eq!(
        shown["task"]["body"],
        "expected a hint naming the configured roots"
    );
    assert_eq!(shown["task"]["priority"], 2);
    assert_eq!(shown["task"]["size"], serde_json::Value::Null);
    assert_eq!(
        shown["task"]["tags"],
        serde_json::json!(["feedback", "friction", "from:sci"])
    );
    assert_eq!(
        env.json(&reporter, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let out = env.json(
        &target,
        &[
            "feedback",
            "--project",
            "tasks",
            "prime is fast",
            "--category",
            "positive",
        ],
    );
    let shown = env.json(&target, &["show", out["id"].as_str().unwrap()]);
    assert_eq!(shown["task"]["tags"][2], "from:tasks");

    let out = env
        .cmd(&target)
        .args([
            "--pretty",
            "feedback",
            "--project",
            "tasks",
            "pretty check",
            "--category",
            "idea",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("created tasks-"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("warning: ") && stderr.contains("uncommitted"),
        "{stderr}"
    );
}

#[test]
fn feedback_fails_early_without_a_target_or_a_reporter() {
    let mut env = TestEnv::new();
    let reporter = env.init("sci");
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap"
            ]
        ),
        "config"
    );
    let target = env.init("tasks");
    std::fs::remove_file(target.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap"
            ]
        ),
        "config"
    );
    assert!(
        std::fs::read_dir(target.join("tasks"))
            .unwrap()
            .next()
            .is_none()
    );
    std::fs::write(target.join("tasks/.config.toml"), "prefix = \"other\"\n").unwrap();
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap"
            ]
        ),
        "config",
        "a registry entry pointing at a project with another prefix is refused"
    );
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(
            nowhere.path(),
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap"
            ]
        ),
        "no_project"
    );
    let err = env.usage(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "probe summary",
            "--category",
            "rant",
        ],
    );
    assert!(err.contains("--category") && err.contains("rant"), "{err}");
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap",
                "-b",
                "a\n## Notes\nb"
            ]
        ),
        "validation"
    );
}

#[test]
fn show_resolves_a_foreign_id_read_only() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    write_doc(&fam, "docs/specs/2026-09-03-far-design.md", "# Far\n");
    let f = id_of(env.json(&fam, &["add", "Far", "--spec", "far"]));
    env.json(&fam, &["note", &f, "seen from afar"]);
    let shown = env.json(&sci, &["show", &f]);
    assert_eq!(shown["task"]["id"], f);
    assert_eq!(shown["task"]["notes"][0]["text"], "seen from afar");
    let spec_path = shown["spec_path"].as_str().unwrap();
    assert!(spec_path.starts_with(fam.to_str().unwrap()), "{spec_path}");
    assert_eq!(env.fail(&sci, &["show", "zzz-000001"]), "unresolvable_id");
    assert_eq!(env.fail(&sci, &["show", "fam-ffffff"]), "task_not_found");

    // relationships are read from the foreign project too
    let kid = id_of(env.json(&fam, &["add", "Kid", "--parent", &f]));
    let shown = env.json(&sci, &["show", &f]);
    assert_eq!(shown["children"][0]["id"], kid);
    assert_eq!(env.json(&sci, &["show", &kid])["parent"]["id"], f);

    let registry = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&registry).unwrap();
    text.push_str(&format!("zzz = {:?}\n", fam.to_str().unwrap()));
    std::fs::write(&registry, text).unwrap();
    assert_eq!(env.fail(&sci, &["show", "zzz-000001"]), "config");
}

#[test]
fn prime_warns_about_uncommitted_task_files_only_in_a_git_checkout() {
    let mut env = TestEnv::new();
    let plain = env.init("sci");
    env.json(&plain, &["add", "Loose"]);
    assert_eq!(
        env.json(&plain, &["prime"])["warnings"],
        serde_json::json!([])
    );

    let repo = env.init("fam");
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(status.success());
    let id = id_of(env.json(&repo, &["add", "Unfiled"]));
    let text = env.json(&repo, &["prime"])["warnings"][0]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        text,
        format!("uncommitted task files: tasks/.config.toml, tasks/{id}.md"),
        "the config counts too; it is a changed file under tasks/"
    );

    // a project nested inside a larger repository reports project-relative paths
    let outer = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(outer.path())
        .status()
        .unwrap();
    assert!(status.success());
    // the directory name has a space: without -z git would quote the whole path
    std::fs::create_dir_all(outer.path().join("sub space")).unwrap();
    let nested = outer.path().join("sub space").canonicalize().unwrap();
    env.json(&nested, &["init", "--prefix", "nst"]);
    let id = id_of(env.json(&nested, &["add", "Deep"]));
    let text = env.json(&nested, &["prime"])["warnings"][0]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        text,
        format!("uncommitted task files: tasks/.config.toml, tasks/{id}.md"),
        "project-relative, unquoted: not \"sub space/tasks/…\""
    );

    // a staged rename record carries two paths; only the new one is reported. The file
    // renamed is a dotfile, which the task scanner skips, because renaming a task file
    // would break id == filename and fail prime's scan before the warning is built
    std::fs::write(nested.join("tasks/.keep"), "").unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&nested)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "-m",
            "seed",
        ])
        .current_dir(&nested)
        .status()
        .unwrap();
    assert!(commit.success());
    let renamed = std::process::Command::new("git")
        .args(["mv", "tasks/.keep", "tasks/.kept"])
        .current_dir(&nested)
        .status()
        .unwrap();
    assert!(renamed.success());
    let text = env.json(&nested, &["prime"])["warnings"][0]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(text, "uncommitted task files: tasks/.kept", "{text}");

    // a broken repository is an error, not a silent skip: a corrupt index makes
    // `git status` fail after discovery succeeded
    std::fs::write(outer.path().join(".git/index"), "garbage").unwrap();
    assert_eq!(env.fail(&nested, &["prime"]), "io");
}

#[test]
fn feedback_recurs_on_exact_titles_and_refuses_to_guess_on_similar_ones() {
    let (mut env, target, reporter) = feedback_env();
    let other = env.init("mnd");
    let first = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing spec",
            "--category",
            "friction",
        ],
    );
    let id = first["id"].as_str().unwrap().to_string();

    let again = env.json(
        &other,
        &[
            "feedback",
            "--project",
            "tasks",
            "Check rejects MISSING spec!",
            "--category",
            "gap",
            "-b",
            "same here",
        ],
    );
    assert_eq!(again["action"], "recurred");
    assert_eq!(again["id"], id);
    let shown = env.json(&target, &["show", &id]);
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2, "{shown}");
    assert_eq!(
        notes[0]["text"],
        "feedback from mnd: Check rejects MISSING spec!"
    );
    assert_eq!(notes[1]["text"], "detail from mnd: same here");
    assert_eq!(
        notes[0]["by"], "feedback",
        "never the reporter's owner name"
    );
    assert_eq!(notes[1]["by"], "feedback");
    assert_eq!(
        shown["task"]["tags"],
        serde_json::json!(["feedback", "friction", "from:sci", "from:mnd", "gap"])
    );
    let files = std::fs::read_dir(target.join("tasks")).unwrap().count();
    assert_eq!(files, 2, "config plus one task file");

    // three shared tokens of six is below the threshold: a new entry, no ambiguity
    let below = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects a missing plan file",
            "--category",
            "friction",
        ],
    );
    assert_eq!(below["action"], "created");

    let entries = || std::fs::read_dir(target.join("tasks")).unwrap().count();
    let before = entries();
    let out = env
        .cmd(&reporter)
        .args([
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "ambiguous");
    assert!(
        err["error"]["detail"].as_str().unwrap().contains(&id),
        "{err}"
    );
    assert_eq!(entries(), before, "an ambiguous request writes nothing");

    let forced = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
            "--new",
        ],
    );
    assert_eq!(forced["action"], "created");
    assert_ne!(forced["id"], id);

    // a single exact title recurs on its own; once --new has made a second one, an
    // automatic report must ask rather than pick the lower id
    let auto = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
        ],
    );
    assert_eq!(auto["action"], "recurred");
    assert_eq!(auto["id"], forced["id"]);
    let twin = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
            "--new",
        ],
    );
    assert_eq!(twin["action"], "created");
    let out = env
        .cmd(&reporter)
        .args([
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "ambiguous");
    let detail = err["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(forced["id"].as_str().unwrap()), "{detail}");
    assert!(detail.contains(twin["id"].as_str().unwrap()), "{detail}");

    let explicit = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
            "--recur",
            &id,
        ],
    );
    assert_eq!(explicit["action"], "recurred");
    assert_eq!(explicit["id"], id);

    // explicit requests do not scan the target, so an unrelated malformed file there
    // blocks neither --recur nor --new (it would fail an automatic report's scan)
    std::fs::write(target.join("tasks/tasks-bad.md"), "nope").unwrap();
    let again = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing plan",
            "--category",
            "friction",
            "--recur",
            &id,
        ],
    );
    assert_eq!(again["action"], "recurred");
    let isolated = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "isolated new entry",
            "--category",
            "gap",
            "--new",
        ],
    );
    assert_eq!(isolated["action"], "created");
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "another automatic report",
                "--category",
                "gap"
            ]
        ),
        "parse"
    );
    std::fs::remove_file(target.join("tasks/tasks-bad.md")).unwrap();

    let unrelated = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "prime output is delightful",
            "--category",
            "positive",
        ],
    );
    assert_eq!(unrelated["action"], "created");
    let unrelated_id = unrelated["id"].as_str().unwrap().to_string();

    // a closed feedback entry is neither matched automatically nor accepted by --recur
    env.json(&target, &["done", &unrelated_id, "triaged"]);
    let refiled = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "prime output is delightful",
            "--category",
            "positive",
        ],
    );
    assert_eq!(refiled["action"], "created");
    assert_ne!(refiled["id"], unrelated_id);
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap",
                "--recur",
                &unrelated_id
            ]
        ),
        "validation"
    );

    // a summary with no usable tokens can match nothing and is refused outright, before
    // the target is even looked up; every other summary in these tests has tokens so that
    // the assertion it carries fails for its own reason and not for this one
    assert_eq!(
        env.fail(
            &reporter,
            &["feedback", "--project", "tasks", "a !", "--category", "gap"]
        ),
        "validation"
    );

    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap",
                "--recur",
                "tasks-ffffff"
            ]
        ),
        "validation"
    );
    let plain = id_of(env.json(&target, &["add", "Not feedback"]));
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap",
                "--recur",
                &plain
            ]
        ),
        "validation"
    );
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "probe summary",
                "--category",
                "gap",
                "--recur",
                &id,
                "-b",
                "two\nlines"
            ]
        ),
        "validation"
    );
}

#[test]
fn feedback_refuses_a_multiline_body_on_recurrence_and_names_the_flag() {
    let (mut env, target, reporter) = feedback_env();
    let other = env.init("mnd");
    let first = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "check rejects missing spec",
            "--category",
            "friction",
        ],
    );
    let id = first["id"].as_str().unwrap().to_string();
    let path = first["path"].as_str().unwrap().to_string();

    // An automatic exact-title match refuses the multiline body with a message that
    // names --body and explains the single-line note, and writes nothing.
    let before = std::fs::read(&path).unwrap();
    let out = env
        .cmd(&other)
        .args([
            "feedback",
            "--project",
            "tasks",
            "Check rejects MISSING spec!",
            "--category",
            "gap",
            "-b",
            "line one\nline two",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "validation", "{err}");
    let detail = err["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("--body"), "{detail}");
    assert!(detail.contains("single line"), "{detail}");
    assert_eq!(std::fs::read(&path).unwrap(), before, "target unchanged");

    // An explicit --recur refuses with the same actionable message.
    let out = env
        .cmd(&other)
        .args([
            "feedback",
            "--project",
            "tasks",
            "a different summary entirely",
            "--category",
            "gap",
            "--recur",
            &id,
            "-b",
            "line one\nline two",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "validation", "{err}");
    assert!(err["error"]["detail"].as_str().unwrap().contains("--body"));
    assert_eq!(std::fs::read(&path).unwrap(), before, "target unchanged");

    // A single-line body still recurs; a multiline body still creates.
    let recurred = env.json(
        &other,
        &[
            "feedback",
            "--project",
            "tasks",
            "Check rejects MISSING spec!",
            "--category",
            "gap",
            "-b",
            "one line here",
        ],
    );
    assert_eq!(recurred["action"], "recurred");
    let shown = env.json(&target, &["show", &id]);
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(
        notes.last().unwrap()["text"],
        "detail from mnd: one line here"
    );

    let multiline = env.json(
        &other,
        &[
            "feedback",
            "--project",
            "tasks",
            "multiline bodies still create",
            "--category",
            "gap",
            "-b",
            "line one\nline two",
        ],
    );
    assert_eq!(multiline["action"], "created");
    let shown = env.json(&target, &["show", multiline["id"].as_str().unwrap()]);
    assert_eq!(shown["task"]["body"], "line one\nline two");
}

#[test]
fn feedback_recurrence_serializes_against_concurrent_recurrences() {
    let (env, target, reporter) = feedback_env();
    let id = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "the thing is slow",
            "--category",
            "friction",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_string();

    let held = hold_project_lock(&env, "tasks");
    let mut children = Vec::new();
    for n in 0..4 {
        // Include source == target: taking its lock twice would deadlock.
        let source = if n % 2 == 0 { &reporter } else { &target };
        let mut cmd = env.raw(source);
        cmd.args([
            "feedback",
            "--project",
            "tasks",
            "the thing is slow",
            "--category",
            "friction",
            "--recur",
            &id,
            "-b",
            &format!("detail {n}"),
        ]);
        children.push(cmd.spawn().unwrap());
    }
    std::thread::sleep(Duration::from_millis(300));
    // Timing heuristic: a heavily loaded unlocked writer can also remain unfinished.
    let blocked: Vec<_> = children.iter_mut().map(|c| c.try_wait()).collect();
    drop(held);
    let reaped: Vec<_> = children.into_iter().map(|c| reap(c, REAP)).collect();
    assert!(
        blocked.into_iter().all(|r| r.unwrap().is_none()),
        "feedback must wait on the target mutation lock"
    );
    assert!(reaped.iter().all(Option::is_some), "feedback never exited");
    for out in reaped.into_iter().flatten() {
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let raw = std::fs::read_to_string(target.join(format!("tasks/{id}.md"))).unwrap();
    for n in 0..4 {
        assert!(
            raw.contains(&format!("detail {n}")),
            "update {n} was lost: {raw}"
        );
    }
}

fn owners_env() -> (
    TestEnv,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let mut env = TestEnv::new();
    let ai = env.init("ai");
    accept_feedback(&ai, "Agent instructions.");
    let ops = env.init("ops");
    accept_feedback(&ops, "Shared tooling and hooks.");
    let sci = env.init("sci");
    (env, ai, ops, sci)
}

#[test]
fn feedback_lands_in_the_owner_named_by_project() {
    let (env, ai, ops, sci) = owners_env();
    let summary = "the guard blocks a clean commit";
    for (owner, root) in [("ai", &ai), ("ops", &ops)] {
        let out = env.json(
            &sci,
            &[
                "feedback",
                "--project",
                owner,
                summary,
                "--category",
                "friction",
            ],
        );
        // the same summary in another owner is not a match: each owner triages its own
        assert_eq!(out["action"], "created", "{out}");
        let id = out["id"].as_str().unwrap();
        assert!(id.starts_with(&format!("{owner}-")), "{id}");
        assert!(
            out["path"]
                .as_str()
                .unwrap()
                .starts_with(root.to_str().unwrap()),
            "{out}"
        );
        let shown = env.json(root, &["show", id]);
        assert_eq!(
            shown["task"]["tags"],
            serde_json::json!(["feedback", "friction", "from:sci"])
        );
    }
    let again = env.json(
        &sci,
        &[
            "feedback",
            "--project",
            "ops",
            summary,
            "--category",
            "friction",
        ],
    );
    assert_eq!(again["action"], "recurred");
    assert!(again["id"].as_str().unwrap().starts_with("ops-"), "{again}");
}

#[test]
fn feedback_refuses_an_owner_that_does_not_accept_and_lists_those_that_do() {
    let (mut env, _ai, _ops, sci) = owners_env();
    let dots = env.init("dots");
    // A malformed task file in an unrelated project must not hide the owners.
    std::fs::write(dots.join("tasks/dots-000001.md"), "not a record\n").unwrap();
    // Nor may another project's unparsable config; it is named, not fatal.
    let bad = env.init("bad");
    std::fs::write(bad.join("tasks/.config.toml"), "prefix = \n").unwrap();

    let out = env
        .cmd(&sci)
        .args([
            "feedback",
            "--project",
            "dots",
            "x is slow",
            "--category",
            "friction",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("\"dots\" does not accept feedback"),
        "{detail}"
    );
    assert!(detail.contains("ai (Agent instructions.)"), "{detail}");
    assert!(
        detail.contains("ops (Shared tooling and hooks.)"),
        "{detail}"
    );
    assert!(detail.contains("bad (unreadable:"), "{detail}");
    assert!(
        std::fs::read_dir(dots.join("tasks")).unwrap().count() == 2,
        "nothing written beside the config and the malformed file"
    );

    assert_eq!(
        env.fail(
            &sci,
            &[
                "feedback",
                "--project",
                "zzz",
                "x is slow",
                "--category",
                "gap"
            ]
        ),
        "config"
    );
    let usage = env.usage(&sci, &["feedback", "x is slow", "--category", "gap"]);
    assert!(usage.contains("--project"), "{usage}");
}

// A permission error reading a project's config (not a parse error) must not be read as
// "unreachable" and silently dropped from the refusal's list of who does accept.
#[test]
fn feedback_lists_a_permission_denied_owner_as_unreadable_not_dropped() {
    let (mut env, _ai, _ops, sci) = owners_env();
    let locked = env.init("lck");
    let mut perms = std::fs::metadata(locked.join("tasks"))
        .unwrap()
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o000);
    std::fs::set_permissions(locked.join("tasks"), perms.clone()).unwrap();
    let out = env
        .cmd(&sci)
        .args([
            "feedback",
            "--project",
            "sci",
            "permission denied owner",
            "--category",
            "friction",
        ])
        .output()
        .unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(locked.join("tasks"), perms).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("lck (unreadable:"), "{detail}");
    assert!(detail.contains("ai (Agent instructions.)"), "{detail}");
}

#[test]
fn feedback_rechecks_acceptance_after_waiting_for_the_target_lock() {
    for recur in [false, true] {
        let (env, target, source) = feedback_env();
        let original = env.json(
            &source,
            &[
                "feedback",
                "--project",
                "tasks",
                "Original report",
                "--category",
                "gap",
                "--new",
            ],
        );
        let id = original["id"].as_str().unwrap().to_string();
        let original_raw = env.read(&target, &format!("tasks/{id}.md"));
        let listing = || {
            let mut names: Vec<_> = std::fs::read_dir(target.join("tasks"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let before = listing();

        let held = hold_project_lock(&env, "tasks");
        let mut command = env.raw(&source);
        command.args([
            "feedback",
            "--project",
            "tasks",
            "Another report",
            "--category",
            "gap",
        ]);
        if recur {
            command.args(["--recur", &id]);
        } else {
            command.arg("--new");
        }
        let mut child = command.spawn().unwrap();
        let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
        // The owner opts out while the report waits for its lock; same prefix, same root.
        std::fs::write(target.join("tasks/.config.toml"), "prefix = \"tasks\"\n").unwrap();
        drop(held);
        let out = reap(child, REAP).unwrap();

        assert!(blocked, "feedback ignored the target lock");
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "validation", "{error}");
        assert!(
            error["error"]["detail"]
                .as_str()
                .unwrap()
                .contains("does not accept feedback"),
            "{error}"
        );
        assert_eq!(listing(), before, "recur={recur}: no file created");
        assert_eq!(
            env.read(&target, &format!("tasks/{id}.md")),
            original_raw,
            "recur={recur}: no note appended"
        );
    }
}

#[test]
fn feedback_recur_must_name_open_feedback_in_the_chosen_owner() {
    let (env, _ai, _ops, sci) = owners_env();
    let filed = env.json(
        &sci,
        &[
            "feedback",
            "--project",
            "ai",
            "skill says x",
            "--category",
            "gap",
        ],
    );
    let id = filed["id"].as_str().unwrap();
    assert_eq!(
        env.fail(
            &sci,
            &[
                "feedback",
                "--project",
                "ops",
                "skill says x",
                "--category",
                "gap",
                "--recur",
                id
            ]
        ),
        "validation"
    );
    let joined = env.json(
        &sci,
        &[
            "feedback",
            "--project",
            "ai",
            "skill says x again",
            "--category",
            "gap",
            "--recur",
            id,
        ],
    );
    assert_eq!(joined["action"], "recurred");
}

#[test]
fn feedback_completion_offers_only_owners_and_their_open_reports() {
    let (env, ai, _ops, sci) = owners_env();
    let owners = env.complete(&sci, "bash", 3, &["tasks", "feedback", "--project", ""]);
    assert_eq!(owners, ["ai", "ops"]);
    let open = id_of(env.json(&ai, &["add", "Open report", "--tag", "feedback"]));
    env.json(&ai, &["add", "Not feedback"]);
    let ids = env.complete(
        &sci,
        "bash",
        6,
        &["tasks", "feedback", "--project", "ai", "S", "--recur", ""],
    );
    assert_eq!(ids, [open.as_str()]);
    let none = env.complete(&sci, "bash", 4, &["tasks", "feedback", "S", "--recur", ""]);
    assert!(none.is_empty(), "no --project, no candidates: {none:?}");
}

fn has_ansi(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|pair| pair == b"\x1b[")
}

#[test]
fn color_is_opt_in_and_never_reaches_json() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    for args in [
        &["--pretty", "check"][..],
        &["--pretty", "--color", "auto", "check"][..],
    ] {
        let out = env.cmd(&dir).args(args).output().unwrap();
        assert!(out.status.success());
        assert!(!has_ansi(&out.stdout), "{args:?}");
    }

    let colored = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "check"])
        .output()
        .unwrap();
    assert!(has_ansi(&colored.stdout));

    // a finding, so the JSON report is non-empty and can be checked for escapes
    std::fs::write(dir.join("tasks/sci-bad.md"), "nope").unwrap();
    let json = env
        .cmd(&dir)
        .args(["--color", "always", "check"])
        .output()
        .unwrap();
    assert!(!has_ansi(&json.stdout));
    let report = serde_json::from_slice::<serde_json::Value>(&json.stdout).unwrap();
    assert_eq!(report["errors"].as_array().unwrap().len(), 1);
}

#[test]
fn no_color_suppresses_config_but_an_explicit_flag_wins() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let suppressed = env
        .cmd(&dir)
        .env("TASKS_COLOR", "always")
        .env("NO_COLOR", "1")
        .args(["--pretty", "check"])
        .output()
        .unwrap();
    assert!(!has_ansi(&suppressed.stdout));

    let overridden = env
        .cmd(&dir)
        .env("TASKS_COLOR", "never")
        .env("NO_COLOR", "1")
        .args(["--pretty", "--color", "always", "check"])
        .output()
        .unwrap();
    assert!(has_ansi(&overridden.stdout));
}

#[test]
fn tasks_color_is_always_validated_and_warnings_use_the_stderr_painter() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [
        &["check"][..],
        &["--pretty", "check"][..],
        &["--pretty", "--color", "never", "check"][..],
    ] {
        let out = env
            .cmd(&dir)
            .env("TASKS_COLOR", "chartreuse")
            .env("NO_COLOR", "1")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config");
    }

    let fresh = tempfile::tempdir().unwrap();
    let warned = env
        .cmd(fresh.path())
        .args(["--pretty", "--color", "always", "init", "--prefix", "warn"])
        .output()
        .unwrap();
    assert!(warned.status.success());
    assert!(String::from_utf8_lossy(&warned.stderr).starts_with("\x1b[33mwarning:\x1b[0m "));
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("\x1b[") {
        plain.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('m').expect("an SGR sequence ends in m");
        rest = &after[end + 1..];
    }
    plain.push_str(rest);
    plain
}

#[test]
fn colored_tables_use_semantic_roles_without_changing_layout() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Idea", "--status", "idea", "--tag", "later"]);
    env.json(&dir, &["add", "Todo", "-p", "0", "--tag", "now"]);
    let doing = id_of(env.json(&dir, &["add", "Doing"]));
    env.json(&dir, &["start", &doing]);
    let blocked = id_of(env.json(&dir, &["add", "Blocked"]));
    env.json(&dir, &["block", &blocked]);
    let done = id_of(env.json(&dir, &["add", "Done"]));
    env.json(&dir, &["done", &done]);
    let dropped = id_of(env.json(&dir, &["add", "Dropped"]));
    env.json(&dir, &["drop", &dropped]);

    let statuses = [
        "--status", "idea", "--status", "todo", "--status", "doing", "--status", "blocked",
        "--status", "done", "--status", "dropped",
    ];
    let plain = env
        .cmd(&dir)
        .arg("--pretty")
        .arg("list")
        .args(statuses)
        .output()
        .unwrap();
    let colored = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list"])
        .args(statuses)
        .output()
        .unwrap();
    let plain = String::from_utf8(plain.stdout).unwrap();
    let colored = String::from_utf8(colored.stdout).unwrap();
    assert_eq!(strip_ansi(&colored), plain);
    for code in [
        "\x1b[34m",
        "\x1b[33m",
        "\x1b[31m",
        "\x1b[2;32m",
        "\x1b[2;31m",
    ] {
        assert!(colored.contains(code), "missing {code:?}: {colored:?}");
    }
    assert!(
        colored.contains("\x1b[1;38;2;215;95;215mP0\x1b[0m"),
        "{colored:?}"
    );
    assert!(colored.contains("\x1b[2m [now]\x1b[0m"));

    let plain_ready = env.cmd(&dir).args(["--pretty", "ready"]).output().unwrap();
    let colored_ready = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "ready"])
        .output()
        .unwrap();
    assert_eq!(
        strip_ansi(&String::from_utf8(colored_ready.stdout).unwrap()),
        String::from_utf8(plain_ready.stdout).unwrap()
    );
}

#[test]
fn colored_show_keeps_bold_priority_without_a_warning() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Urgent", "-p", "1"]));
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "show", &id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("priority: \x1b[1m1\x1b[0m\n"), "{text:?}");
    assert!(String::from_utf8(out.stderr).unwrap().is_empty());
}

#[test]
fn colored_list_paints_priorities_on_the_magenta_scale() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for (title, priority) in [("One", "1"), ("Two", "2"), ("Three", "3"), ("Four", "4")] {
        env.json(&dir, &["add", title, "-p", priority]);
    }
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    for code in [
        "\x1b[38;2;215;95;215mP1\x1b[0m",
        "\x1b[38;2;163;88;159mP2\x1b[0m",
        "\x1b[38;2;112;78;106mP3\x1b[0m",
        "\x1b[38;2;64;64;56mP4\x1b[0m",
    ] {
        assert!(text.contains(code), "missing {code:?}: {text:?}");
    }
    assert!(String::from_utf8(out.stderr).unwrap().is_empty());
}

#[test]
fn a_palette_without_magenta_warns_only_where_priorities_show() {
    const THREE_KEYS: &str = "fg=#e5e3d7 bg=#13140d cyan=#00d7ff";
    const WARNING: &str = "priority colors off: TASKS_PALETTE has no magenta; add magenta=#rrggbb";
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Urgent", "-p", "1"]));
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["park", &id, "resume the thing"]);

    let list = env
        .cmd(&dir)
        .env("TASKS_PALETTE", THREE_KEYS)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let text = String::from_utf8(list.stdout).unwrap();
    assert!(text.contains("\x1b[1mP1\x1b[0m"), "bold look: {text:?}");
    assert!(
        text.contains("\x1b[38;2;0;215;255m"),
        "dates still paint: {text:?}"
    );
    let stderr = String::from_utf8(list.stderr).unwrap();
    assert_eq!(stderr.matches(WARNING).count(), 1, "{stderr:?}");

    for args in [&["projects"][..], &["list", "--parked"][..]] {
        let out = env
            .cmd(&dir)
            .env("TASKS_PALETTE", THREE_KEYS)
            .args(["--pretty", "--color", "always"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .contains("\x1b[38;2;0;215;255m"),
            "{args:?}: dates paint"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(
            !stderr.contains("priority colors off"),
            "{args:?}: {stderr:?}"
        );
    }
}

#[test]
fn a_malformed_magenta_is_a_config_error_whenever_set() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let out = env
        .cmd(&dir)
        .env(
            "TASKS_PALETTE",
            "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=purple",
        )
        .args(["list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("magenta: \"purple\" is not #rrggbb")
    );
}

#[test]
fn colored_show_paints_frontmatter_values_with_table_roles() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let dep = id_of(env.json(&dir, &["add", "Dep"]));
    let parent = id_of(env.json(&dir, &["add", "Goal"]));
    let id = id_of(env.json(
        &dir,
        &[
            "add",
            "Painted",
            "-p",
            "0",
            "--size",
            "m",
            "--tag",
            "now",
            "--parent",
            &parent,
            "-b",
            // the words a naive whole-text substitution would repaint
            "Body mentioning doing and todo and tags.",
        ],
    ));
    env.json(&dir, &["dep", &id, "--on", &dep]);
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["note", &id, "doing the work"]);
    let task = env.json(&dir, &["show", &id]);
    let owner = task["task"]["owner"].as_str().unwrap().to_string();
    let created = task["task"]["created"].as_str().unwrap().to_string();

    let show = |args: &[&str]| {
        let out = env.cmd(&dir).args(args).output().unwrap();
        assert!(out.status.success(), "{args:?} failed");
        String::from_utf8(out.stdout).unwrap()
    };
    let plain = show(&["--pretty", "show", &id]);
    let colored = show(&["--pretty", "--color", "always", "show", &id]);

    // color is decoration only: the same visible text either way
    assert_eq!(strip_ansi(&colored), plain);
    assert!(
        !plain.contains("\x1b["),
        "--pretty alone stays plain: {plain:?}"
    );
    assert!(
        !show(&["--color", "always", "show", &id]).contains("\x1b["),
        "JSON never carries escapes"
    );

    // painted values take the roles the list table gives the same fields
    for span in [
        format!("id: \x1b[2m{id}\x1b[0m\n"),
        "status: \x1b[33mdoing\x1b[0m\n".to_string(),
        "priority: \x1b[1m0\x1b[0m\n".to_string(),
        format!("owner: \x1b[2m{owner}\x1b[0m\n"),
        format!("parent: \x1b[2m{parent}\x1b[0m\n"),
        format!("depends: \x1b[2m[{dep}]\x1b[0m\n"),
        "tags: \x1b[2m[now]\x1b[0m\n".to_string(),
    ] {
        assert!(colored.contains(&span), "missing {span:?}: {colored:?}");
    }

    // the rest of the file text stays plain
    for span in [
        "title: Painted\n".to_string(),
        "size: m\n".to_string(),
        format!("created: {created}\n"),
    ] {
        assert!(
            colored.contains(&span),
            "{span:?} must stay plain: {colored:?}"
        );
    }
    let (_, after) = colored.split_once("\n---\n").unwrap();
    let body = after.split("\n# ").next().unwrap();
    assert!(
        !body.contains("\x1b["),
        "body and notes stay plain: {body:?}"
    );

    // emphasis is for P0/P1 only, exactly as in the table
    let ordinary = show(&["--pretty", "--color", "always", "show", &dep]);
    assert!(
        ordinary.contains("priority: 2\n"),
        "an ordinary priority stays plain: {ordinary:?}"
    );
}

#[test]
fn init_force_repoints_a_prefix_and_unregister_frees_it() {
    let mut env = TestEnv::new();
    let first = env.init("sci");
    let second = tempfile::tempdir().unwrap();
    let second = second.path().canonicalize().unwrap();

    // the conflict is an error that names both remedies
    let out = env
        .cmd(&second)
        .args(["init", "--prefix", "sci"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(first.to_str().unwrap()), "{detail}");
    assert!(detail.contains("--force"), "{detail}");
    assert!(detail.contains("tasks unregister sci"), "{detail}");
    assert!(
        !second.join("tasks").exists(),
        "a refused init writes nothing"
    );

    // --force re-points and warns with the displaced root
    let forced = env.json(&second, &["init", "--prefix", "sci", "--force"]);
    assert_eq!(forced["prefix"], "sci");
    assert_eq!(forced["root"], second.to_str().unwrap());
    let warnings = forced["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains(first.to_str().unwrap())),
        "{forced}"
    );

    // the re-point took effect: a foreign id now resolves through the new root
    let moved = id_of(env.json(&second, &["add", "Moved"]));
    let other = env.init("fam");
    let shown = env.json(&other, &["show", &moved]);
    assert_eq!(shown["task"]["title"], "Moved");

    // re-pointing at the same root displaces nothing, so there is nothing to warn about
    let again = env.json(&second, &["init", "--prefix", "sci", "--force"]);
    assert!(
        !again["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("registered")),
        "{again}"
    );

    // unregister works from outside any project and frees the prefix
    let nowhere = tempfile::tempdir().unwrap();
    let dropped = env.json(nowhere.path(), &["unregister", "sci"]);
    assert_eq!(dropped["prefix"], "sci");
    assert_eq!(dropped["root"], second.to_str().unwrap());
    assert_eq!(
        env.fail(nowhere.path(), &["unregister", "sci"]),
        "config",
        "removing an absent prefix is an error, not a silent no-op"
    );
    assert_eq!(env.fail(&other, &["show", &moved]), "unresolvable_id");
    assert!(
        second.join("tasks/.config.toml").is_file(),
        "unregister edits the registry only; project files are untouched"
    );

    // and the prefix is claimable again without --force
    env.json(&second, &["init", "--prefix", "sci"]);
    assert_eq!(
        env.json(&other, &["show", &moved])["task"]["title"],
        "Moved"
    );

    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "unregister", "sci"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "sci");
}

#[test]
fn unregister_takes_the_aliases_with_it_and_init_respects_them() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    alias_registry(&env, "old", "fam");

    // A retired name is taken: init cannot claim it.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(fresh.path(), &["init", "--prefix", "old"]),
        "config"
    );
    assert_eq!(
        env.fail(fresh.path(), &["init", "--prefix", "old", "--force"]),
        "config"
    );

    // Unregistering the alias itself is refused, pointing at the live name.
    assert_eq!(env.fail(&sci, &["unregister", "old"]), "config");

    // Unregistering the project drops its aliases and says so.
    let value = env.json(&sci, &["unregister", "fam"]);
    assert_eq!(value["aliases"], serde_json::json!(["old"]));
    // The registry is loadable afterwards: no dangling alias.
    assert_eq!(env.json(&sci, &["list"])["tasks"], serde_json::json!([]));
}

#[test]
fn root_prints_the_registered_root_of_an_id() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["root", "sci-000000"]);
    assert_eq!(v["prefix"], "sci");
    assert_eq!(v["root"], sci.to_str().unwrap());
    assert_eq!(v["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "root", "sci-000000"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        sci.to_str().unwrap()
    );
    assert_eq!(
        env.fail(nowhere.path(), &["root", "zzz-000000"]),
        "unresolvable_id"
    );
    assert_eq!(env.fail(nowhere.path(), &["root", "bogus"]), "invalid_id");

    let mut other_env = TestEnv::new();
    let unregistered = other_env.init("lon");
    let v = env.json(&unregistered, &["root", "sci-000000"]);
    assert_eq!(
        v["warnings"],
        serde_json::json!(["current project lon is not registered"])
    );
}

#[test]
fn a_retired_prefix_resolves_to_its_live_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&fam, &["add", "Far", "-p", "2"]));
    alias_registry(&env, "old", "fam");

    let retired = format!("old-{}", id.split_once('-').unwrap().1);
    // `root` resolves the *project* from the prefix, which is all this task delivers.
    assert_eq!(env.json(&sci, &["root", &retired])["prefix"], "fam");

    // --project takes a retired name too: it is a name of the project.
    let v = env.json(&sci, &["list", "--project", "old"]);
    assert_eq!(v["tasks"][0]["id"], id, "{v}");
}

#[test]
fn a_retired_id_is_one_task_for_routing_dedup_and_removal() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let a = id_of(env.json(&sci, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&sci, &["add", "B", "-p", "2"]));
    alias_registry(&env, "old", "sci");
    let retired_a = format!("old-{}", a.split_once('-').unwrap().1);
    let retired_b = format!("old-{}", b.split_once('-').unwrap().1);

    assert_eq!(env.json(&sci, &["show", &retired_a])["task"]["id"], a);
    assert_eq!(
        env.json(&sci, &["list", "--parent", &retired_a])["tasks"],
        serde_json::json!([])
    );

    env.json(
        &sci,
        &["note", &retired_a, "written through the retired name"],
    );
    let shown = env.json(&sci, &["show", &a]);
    assert_eq!(
        shown["task"]["notes"][0]["text"],
        "written through the retired name"
    );

    env.json(&sci, &["dep", &a, "--on", &retired_b]);
    assert_eq!(env.json(&sci, &["show", &a])["task"]["depends"][0], b);

    env.json(&sci, &["dep", &a, "--on", &b]);
    assert_eq!(
        env.json(&sci, &["show", &a])["task"]["depends"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    assert_eq!(env.fail(&sci, &["dep", &a, "--on", &retired_a]), "cycle");

    env.json(&sci, &["dep", &a, "--rm", &retired_b]);
    assert!(
        env.json(&sci, &["show", &a])["task"]
            .get("depends")
            .is_none()
    );
}

#[test]
fn stored_retired_references_resolve_detect_cycles_and_keep_their_spelling() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&sci, &["add", "Child", "-p", "2", "--parent", &goal]));
    let a = id_of(env.json(&sci, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&sci, &["add", "B", "-p", "2"]));
    env.json(&sci, &["dep", &a, "--on", &b]);
    alias_registry(&env, "old", "sci");

    let retired_goal = goal.replacen("sci-", "old-", 1);
    let retired_child = child.replacen("sci-", "old-", 1);
    let retired_b = b.replacen("sci-", "old-", 1);
    let child_path = sci.join("tasks").join(format!("{child}.md"));
    let a_path = sci.join("tasks").join(format!("{a}.md"));
    std::fs::write(
        &child_path,
        std::fs::read_to_string(&child_path).unwrap().replace(
            &format!("parent: {goal}"),
            &format!("parent: {retired_goal}"),
        ),
    )
    .unwrap();
    std::fs::write(
        &a_path,
        std::fs::read_to_string(&a_path).unwrap().replace(
            &format!("depends: [{b}]"),
            &format!("depends: [{retired_b}]"),
        ),
    )
    .unwrap();

    let shown = env.json(&sci, &["show", &child]);
    assert_eq!(shown["parent"]["id"], goal);
    assert_eq!(env.json(&sci, &["show", &goal])["children"][0]["id"], child);
    assert_eq!(
        env.json(&sci, &["list", "--parent", &goal])["tasks"][0]["id"],
        child
    );
    assert_eq!(
        env.json(&sci, &["tree", &goal])["nodes"][0]["children"][0]["id"],
        child
    );

    env.json(&sci, &["note", &child, "unrelated"]);
    assert!(
        std::fs::read_to_string(&child_path)
            .unwrap()
            .contains(&format!("parent: {retired_goal}"))
    );

    assert_eq!(
        env.fail(&sci, &["edit", &goal, "--parent", &retired_child]),
        "cycle"
    );
    assert_eq!(env.fail(&sci, &["dep", &b, "--on", &a]), "cycle");
}

#[test]
fn projects_lists_the_registry_with_reachability_and_counts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["add", "B", "--status", "idea"]);
    env.json(&sci, &["done", &a]);
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let nowhere = tempfile::tempdir().unwrap();
    let v = env.json(nowhere.path(), &["projects"]);
    let rows = v["projects"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["prefix"], "fam");
    assert_eq!(rows[0]["reachable"], false);
    assert_eq!(rows[0]["counts"], serde_json::Value::Null);
    assert_eq!(rows[1]["prefix"], "sci");
    assert_eq!(rows[1]["root"], sci.to_str().unwrap());
    assert_eq!(rows[1]["reachable"], true);
    assert_eq!(rows[1]["counts"]["idea"], 1);
    assert_eq!(rows[1]["counts"]["done"], 1);
    assert_eq!(v["warnings"], serde_json::json!([]));
    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("fam") && text.contains("unreachable"),
        "{text}"
    );
    assert!(
        text.lines().next().unwrap().starts_with("project"),
        "{text}"
    );
    assert_eq!(
        text.matches("idea").count(),
        1,
        "one header, not one per row: {text}"
    );

    std::fs::write(fam.join("tasks/.config.toml"), "not toml = [").unwrap();
    assert_eq!(env.fail(nowhere.path(), &["projects"]), "config");

    // the shared registry warnings apply here too
    let fresh = TestEnv::new();
    let v = fresh.json(nowhere.path(), &["projects"]);
    assert_eq!(v["projects"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!(["registry is empty"]));
}

#[test]
fn projects_total_and_activity_count_closed_tasks() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["done", &a]);
    let closed = env.json(&sci, &["show", &a]);
    let nowhere = tempfile::tempdir().unwrap();

    // the project's only task is done: a total or an activity date that skipped closed
    // tasks would report 0 and null here.
    let v = env.json(nowhere.path(), &["projects"]);
    let row = &v["projects"][0];
    assert_eq!(row["total"], 1, "{v}");
    assert_eq!(row["last_activity"], closed["task"]["updated"], "{v}");
}

#[test]
fn projects_total_and_activity_report_absence_apart_from_emptiness() {
    let mut env = TestEnv::new();
    let empty = env.init("fam");
    let gone = env.init("sci");
    std::fs::remove_file(gone.join("tasks/.config.toml")).unwrap();
    let nowhere = tempfile::tempdir().unwrap();

    let v = env.json(nowhere.path(), &["projects"]);
    let rows = v["projects"].as_array().unwrap();
    assert_eq!(rows[0]["prefix"], "fam");
    assert_eq!(rows[0]["root"], empty.to_str().unwrap());
    // registered and scanned, holding nothing: a real zero and no activity yet
    assert_eq!(rows[0]["total"], 0, "{v}");
    assert_eq!(rows[0]["last_activity"], serde_json::Value::Null, "{v}");
    // never scanned: absent data, reported the way `counts` already reports it
    assert_eq!(rows[1]["prefix"], "sci");
    assert_eq!(rows[1]["reachable"], false);
    assert_eq!(rows[1]["total"], serde_json::Value::Null, "{v}");
    assert_eq!(rows[1]["last_activity"], serde_json::Value::Null, "{v}");
}

#[test]
fn projects_pretty_prints_one_header_and_aligned_columns() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "B", "--status", "idea"]);
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["done", &a]);
    let day = env.json(&sci, &["show", &a])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    let nowhere = tempfile::tempdir().unwrap();

    let text = env.pretty(nowhere.path(), &["projects"]);
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "project  idea  todo  doing  blocked  shelved  total  activity"
    );
    // header-width columns, two-space gutters, counts right-aligned under their labels
    assert_eq!(
        lines.next().unwrap(),
        format!(
            "{:<7}  {:>4}  {:>4}  {:>5}  {:>7}  {:>7}  {:>5}  {day}",
            "sci", 1, 0, 0, 0, 0, 2
        )
    );
}

#[test]
fn projects_pretty_marks_an_unreachable_row_without_breaking_the_grid() {
    let mut env = TestEnv::new();
    env.init("sci");
    let gone = env.init("fam");
    std::fs::remove_file(gone.join("tasks/.config.toml")).unwrap();
    let nowhere = tempfile::tempdir().unwrap();

    let text = env.pretty(nowhere.path(), &["projects"]);
    let row = text.lines().find(|l| l.starts_with("fam")).unwrap();
    assert_eq!(
        row,
        format!(
            "{:<7}  {:>4}  {:>4}  {:>5}  {:>7}  {:>7}  {:>5}  unreachable",
            "fam", "-", "-", "-", "-", "-", "-"
        ),
        "{text}"
    );
}

#[test]
fn projects_pretty_reveals_closed_columns_on_request() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["done", &a]);
    let nowhere = tempfile::tempdir().unwrap();

    let plain = env.pretty(nowhere.path(), &["projects"]);
    assert!(
        !plain.contains("done"),
        "closed columns are hidden: {plain}"
    );
    assert!(!plain.contains("dropped"), "{plain}");
    // the one done task is still accounted for, in total
    assert!(plain.lines().nth(1).unwrap().contains(" 1"), "{plain}");

    let opened = env.pretty(nowhere.path(), &["projects", "--closed"]);
    assert_eq!(
        opened.lines().next().unwrap(),
        "project  idea  todo  doing  blocked  shelved  done  dropped  total  activity"
    );
}

#[test]
fn projects_pretty_reveals_roots_on_request() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let nowhere = tempfile::tempdir().unwrap();

    let plain = env.pretty(nowhere.path(), &["projects"]);
    assert!(!plain.contains(sci.to_str().unwrap()), "{plain}");

    let with_paths = env.pretty(nowhere.path(), &["projects", "--paths"]);
    assert!(
        with_paths.lines().next().unwrap().ends_with("root"),
        "{with_paths}"
    );
    assert!(
        with_paths
            .lines()
            .nth(1)
            .unwrap()
            .ends_with(sci.to_str().unwrap()),
        "{with_paths}"
    );
}

#[test]
fn prime_counts_line_uses_the_same_columns_as_projects() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "Open"]);
    let a = id_of(env.json(&sci, &["add", "A"]));
    env.json(&sci, &["done", &a]);

    // prime is a single row, so it keeps label-value pairs rather than a header - but the
    // columns, their order, and the closed-by-default rule come from one definition.
    let text = env.pretty(&sci, &["prime"]);
    assert_eq!(
        text.lines().nth(1).unwrap(),
        "idea 0  todo 1  doing 0  blocked 0  shelved 0  total 2"
    );

    let opened = env.pretty(&sci, &["prime", "--closed"]);
    assert_eq!(
        opened.lines().nth(1).unwrap(),
        "idea 0  todo 1  doing 0  blocked 0  shelved 0  done 1  dropped 0  total 2"
    );
}

#[test]
fn projects_pretty_paints_counts_by_status_and_dims_zeros() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "B", "--status", "idea"]);
    let nowhere = tempfile::tempdir().unwrap();

    let out = env
        .cmd(nowhere.path())
        .args(["--pretty", "--color", "always", "projects"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let row = text.lines().nth(1).unwrap();
    // idea is 1: painted in the idea role, the same blue `list` and `show` use
    assert!(row.contains("\u{1b}[34m   1\u{1b}[0m"), "{row:?}");
    // blocked is 0: dimmed whatever the column, so a red 0 does not read as an alarm
    assert!(row.contains("\u{1b}[2m      0\u{1b}[0m"), "{row:?}");
    // total carries no status; activity is a date and takes the recency role
    assert!(row.contains("      1  \x1b[38;2;0;215;255m"), "{row:?}");
    assert!(row.ends_with("\x1b[0m"), "{row:?}");
}

#[test]
fn projects_sort_is_command_level_and_reorders_the_json() {
    let mut env = TestEnv::new();
    let small = env.init("aaa");
    let big = env.init("zzz");
    env.json(&small, &["add", "one"]);
    for title in ["one", "two", "three"] {
        env.json(&big, &["add", title]);
    }
    let nowhere = tempfile::tempdir().unwrap();
    let prefixes = |v: &serde_json::Value| -> Vec<String> {
        v["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["prefix"].as_str().unwrap().to_string())
            .collect()
    };

    let v = env.json(nowhere.path(), &["projects"]);
    assert_eq!(prefixes(&v), ["aaa", "zzz"], "prefix is the default order");
    // the reorder reaches JSON, not just the table: this is a command-level option
    let v = env.json(nowhere.path(), &["projects", "--sort", "size"]);
    assert_eq!(prefixes(&v), ["zzz", "aaa"]);
    let v = env.json(nowhere.path(), &["projects", "--sort", "size", "--reverse"]);
    assert_eq!(prefixes(&v), ["aaa", "zzz"]);

    // the task-shaped keys are not project keys
    let err = env.usage(nowhere.path(), &["projects", "--sort", "updated"]);
    assert!(err.contains("--sort") && err.contains("updated"), "{err}");
}

#[test]
fn defer_stores_an_absolute_date_from_either_form_and_clears() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fixed = id_of(env.json(&sci, &["add", "Fixed", "--defer", "2099-01-02"]));
    let v = env.json(&sci, &["show", &fixed]);
    assert_eq!(v["task"]["defer"], "2099-01-02");
    let text = std::fs::read_to_string(sci.join(format!("tasks/{fixed}.md"))).unwrap();
    assert!(text.contains("\ndefer: 2099-01-02\n"), "{text}");

    let relative = id_of(env.json(
        &sci,
        &["add", "Relative", "--status", "idea", "--defer", "60d"],
    ));
    let stored = env.json(&sci, &["show", &relative])["task"]["defer"].clone();
    let today = time::OffsetDateTime::now_utc().date();
    let expected = today + time::Duration::days(60);
    assert_eq!(
        stored,
        format!(
            "{:04}-{:02}-{:02}",
            expected.year(),
            expected.month() as u8,
            expected.day()
        )
    );

    env.json(&sci, &["edit", &fixed, "--defer", "8w"]);
    assert_ne!(
        env.json(&sci, &["show", &fixed])["task"]["defer"],
        "2099-01-02"
    );
    env.json(&sci, &["edit", &fixed, "--no-defer"]);
    assert!(env.json(&sci, &["show", &fixed])["task"]["defer"].is_null());
    env.json(&sci, &["edit", &fixed, "--defer", "2099-01-02", "-p", "1"]);
    let v = env.json(&sci, &["show", &fixed]);
    assert_eq!(v["task"]["defer"], "2099-01-02");
    assert_eq!(v["task"]["priority"], 1);
}

#[test]
fn defer_refuses_bad_values_and_incompatible_records() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let today = time::OffsetDateTime::now_utc().date();
    let today = format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        today.month() as u8,
        today.day()
    );
    for bad in [today.as_str(), "2020-01-01", "0d", "soon", "2099-13-01"] {
        let error = error_of(&env, &sci, &["add", "Bad", "--defer", bad]);
        assert_eq!(error["error"]["kind"], "validation", "{bad}: {error}");
    }
    let error = error_of(&env, &sci, &["add", "Bad", "--defer", &today]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("must be after today")
    );

    let sweep = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    let error = error_of(&env, &sci, &["edit", &sweep, "--defer", "30d"]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("defer and every cannot both be set")
    );
    let later = id_of(env.json(&sci, &["add", "Later", "--defer", "30d"]));
    let error = error_of(&env, &sci, &["edit", &later, "--every", "30d"]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("defer and every cannot both be set")
    );
    let error = error_of(
        &env,
        &sci,
        &["add", "Both", "--every", "30d", "--defer", "30d"],
    );
    assert_eq!(error["error"]["kind"], "validation");

    let goal = id_of(env.json(&sci, &["add", "Goal"]));
    env.json(&sci, &["add", "Kid", "--parent", &goal]);
    let error = error_of(&env, &sci, &["edit", &goal, "--defer", "30d"]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("cannot be deferred"),
        "{error}"
    );
    let error = error_of(&env, &sci, &["add", "Orphan", "--parent", &later]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("is deferred and cannot have children"),
        "{error}"
    );
    let loose = id_of(env.json(&sci, &["add", "Loose"]));
    let error = error_of(&env, &sci, &["edit", &loose, "--parent", &later]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("is deferred and cannot have children"),
        "{error}"
    );

    let before = env.read(&sci, &format!("tasks/{loose}.md"));
    let reparent = editor_script(
        &sci,
        &format!("sed -i '/^priority:/a parent: {later}' \"$1\""),
    );
    let out = env
        .cmd(&sci)
        .env("EDITOR", &reparent)
        .args(["edit", &loose])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("is deferred and cannot have children"),
        "{error}"
    );
    assert_eq!(env.read(&sci, &format!("tasks/{loose}.md")), before);

    let busy = id_of(env.json(&sci, &["add", "Busy"]));
    env.json(&sci, &["start", &busy]);
    let error = error_of(&env, &sci, &["edit", &busy, "--defer", "30d"]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("is doing"),
        "{error}"
    );
    let closed = id_of(env.json(&sci, &["add", "Closed"]));
    env.json(&sci, &["done", &closed, "x"]);
    let error = error_of(&env, &sci, &["edit", &closed, "--defer", "30d"]);
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("is done"),
        "{error}"
    );
}

#[test]
fn defer_flag_conflicts_are_usage_errors() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "--status", "idea"]));
    for args in [
        vec!["edit", id.as_str(), "--defer", "30d", "--no-defer"],
        vec!["edit", id.as_str(), "--status", "todo", "--defer", "30d"],
    ] {
        env.cmd(&sci).args(&args).assert().code(2);
    }
    assert!(env.json(&sci, &["show", &id])["task"]["defer"].is_null());
}

#[test]
fn every_status_transition_spends_a_deferral() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("todo", vec!["start"]),
        ("todo", vec!["done", "landed"]),
        ("todo", vec!["drop", "no"]),
        ("todo", vec!["shelve", "later"]),
        ("idea", vec!["edit", "--status", "todo"]),
    ];
    for (status, action) in cases {
        let id = id_of(env.json(&sci, &["add", "T", "--status", status, "--defer", "30d"]));
        let mut args: Vec<&str> = vec![action[0], &id];
        args.extend(&action[1..]);
        env.json(&sci, &args);
        let value = env.json(&sci, &["show", &id]);
        assert!(value["task"]["defer"].is_null(), "{action:?} left {value}");
    }

    let kept = id_of(env.json(&sci, &["add", "Kept", "--defer", "30d"]));
    env.json(&sci, &["edit", &kept, "-p", "0"]);
    env.json(&sci, &["note", &kept, "still later"]);
    assert!(env.json(&sci, &["show", &kept])["task"]["defer"].is_string());
    env.json(&sci, &["edit", &kept, "--status", "todo"]);
    assert!(
        env.json(&sci, &["show", &kept])["task"]["defer"].is_string(),
        "same status is not a transition"
    );

    let blocked = id_of(env.json(&sci, &["add", "Blocked", "--defer", "2099-01-02"]));
    env.json(&sci, &["block", &blocked, "blocked"]);
    env.json(&sci, &["edit", &blocked, "--defer", "2099-01-02"]);
    env.json(&sci, &["block", &blocked, "still blocked"]);
    let v = env.json(&sci, &["show", &blocked]);
    assert_eq!(v["task"]["status"], "blocked");
    assert_eq!(v["task"]["defer"], "2099-01-02", "{v}");
}

#[test]
fn editor_saves_follow_the_one_thing_per_save_rule_for_defer() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "--defer", "30d"]));

    let editor = editor_script(&sci, "sed -i 's/^status: todo$/status: doing/' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let value = env.json(&sci, &["show", &id]);
    assert_eq!(value["task"]["status"], "doing");
    assert!(value["task"]["defer"].is_null(), "{value}");

    let editor = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\ndefer: 2099-01-02/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains("is doing"),
        "{}",
        err_detail(&out)
    );
    assert!(env.json(&sci, &["show", &id])["task"]["defer"].is_null());

    let both = id_of(env.json(
        &sci,
        &["add", "Both", "--status", "idea", "--defer", "2099-01-02"],
    ));
    let editor = editor_script(
        &sci,
        "sed -i -e 's/^status: idea$/status: todo/' -e 's/^defer: 2099-01-02$/defer: 2099-06-01/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &both])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        err_detail(&out).contains("cannot also change defer"),
        "{}",
        err_detail(&out)
    );
    let value = env.json(&sci, &["show", &both]);
    assert_eq!(value["task"]["status"], "idea");
    assert_eq!(value["task"]["defer"], "2099-01-02");

    let plain = id_of(env.json(&sci, &["add", "Plain", "--status", "idea"]));
    let editor = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\ndefer: 2026-01-01/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &plain])
        .env("EDITOR", &editor)
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &plain])["task"]["defer"],
        "2026-01-01"
    );

    env.json(&sci, &["edit", &plain, "--status", "todo"]);
    env.json(&sci, &["edit", &plain, "--defer", "30d"]);
    let value = env.json(&sci, &["show", &plain]);
    assert_eq!(value["task"]["status"], "todo");
    assert!(value["task"]["defer"].is_string());
}

/// Two project roots sharing one prefix: what a main checkout and a worktree look like to a
/// store keyed by prefix.
fn two_roots(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf) {
    (env.init("sci"), env.init_forced("sci"))
}

/// Run `tasks` as a named agent with a live pid.
fn as_agent(env: &TestEnv, dir: &std::path::Path, session: &str) -> assert_cmd::Command {
    let mut cmd = env.cmd(dir);
    cmd.env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    cmd
}

/// The parsed `{"error": {"kind", "detail"}}` of a command expected to exit 1.
fn error_of(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stderr).unwrap()
}

#[test]
fn prime_lists_parked_before_ready_and_ready_omits_user_parked_work() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    write_doc(&sci, "docs/specs/x-design.md", "# x\n");
    let a = id_of(env.json(&sci, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(
        &sci,
        &["add", "B", "-p", "1", "--spec", "docs/specs/x-design.md"],
    ));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &b, "decide the shape", "--waiting-on", "user"])
        .assert()
        .success();
    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["parked"].as_array().unwrap().len(), 1);
    assert_eq!(prime["parked"][0]["id"], b);
    assert_eq!(prime["parked"][0]["phase"], "planning");
    assert_eq!(prime["parked"][0]["status"], "todo");
    assert_eq!(prime["parked"][0]["park"]["waiting_on"], "user");
    let ready: Vec<&str> = prime["ready"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ready, vec![a.as_str()], "B waits on the user");
    let warnings = prime["warnings"].to_string();
    assert!(
        warnings.contains(&format!(
            "{b} omitted: parked waiting on the user: decide the shape"
        )),
        "{warnings}"
    );
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], a);
    let text = env.pretty(&sci, &["prime"]);
    assert!(
        text.find("parked:").unwrap() < text.find("ready:").unwrap(),
        "{text}"
    );
    assert!(
        text.contains("planning") && text.contains("decide the shape"),
        "{text}"
    );
}

#[test]
fn next_prefers_the_most_recently_parked_candidate_and_applies_the_eligibility_rules() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let urgent = id_of(env.json(&sci, &["add", "Urgent", "-p", "0", "--size", "xs"]));
    let work = id_of(env.json(&sci, &["add", "Work", "-p", "3"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &work, "continue"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], work);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let idea = id_of(env.json(&sci, &["add", "Idea", "--status", "idea"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &idea, "write the problem statement"])
        .assert()
        .success();
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], idea);
    env.json(&sci, &["drop", &work, "x"]);
    env.json(&sci, &["drop", &idea, "x"]);
    let blocked = id_of(env.json(&sci, &["add", "Blocked", "-p", "1"]));
    env.json(&sci, &["block", &blocked, "why"]);
    as_agent(&env, &sci, "agent-a")
        .args(["park", &blocked, "x"])
        .assert()
        .success();
    let open_dep = id_of(env.json(&sci, &["add", "OpenDep", "-p", "1"]));
    let held = id_of(env.json(&sci, &["add", "Held", "-p", "1", "--depends", &open_dep]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &held, "x"])
        .assert()
        .success();
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "1"]));
    env.json(&sci, &["add", "Child", "-p", "1", "--parent", &goal]);
    as_agent(&env, &sci, "agent-a")
        .args(["park", &goal, "x"])
        .assert()
        .success();
    let foreign = id_of(env.json(&fam, &["add", "F", "-p", "2"]));
    let unreachable = id_of(env.json(
        &sci,
        &["add", "Unreachable", "-p", "1", "--depends", &foreign],
    ));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &unreachable, "x"])
        .assert()
        .success();
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], urgent, "{next}");
    assert!(
        next["warnings"].to_string().contains("unreachable"),
        "{next}"
    );
    assert_eq!(
        env.json(&sci, &["prime"])["parked"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn park_in_one_worktree_start_and_done_in_another_leaves_nothing_parked() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    as_agent(&env, &a, "agent-a")
        .args(["park", &id, "next"])
        .assert()
        .success();
    assert_eq!(env.json(&b, &["prime"])["parked"][0]["id"], id);
    as_agent(&env, &b, "agent-b")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &b, "agent-b")
        .args(["done", &id, "landed"])
        .assert()
        .success();
    let prime = env.json(&a, &["prime"]);
    assert!(prime["parked"].as_array().unwrap().is_empty(), "{prime}");
    assert!(env.json(&a, &["show", &id])["park"].is_null());
}

#[test]
fn a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next() {
    let mut env = TestEnv::new();
    let (main, wt) = two_roots(&mut env);
    let parent = id_of(env.json(&wt, &["add", "Parent", "-p", "2"]));
    env.json(&wt, &["add", "Child", "-p", "2", "--parent", &parent]);
    as_agent(&env, &wt, "agent-a")
        .args(["park", &parent, "plan the pieces"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let goal = id_of(env.json(&wt, &["add", "Goal", "-p", "2"]));
    let kid = id_of(env.json(&wt, &["add", "Kid", "-p", "2", "--parent", &goal]));
    as_agent(&env, &wt, "agent-a")
        .args(["park", &kid, "finish the kid"])
        .assert()
        .success();
    let prime = env.json(&main, &["prime"]);
    let rows = prime["parked"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let row = rows.iter().find(|r| r["id"] == parent).unwrap();
    assert_eq!(row["status"], "todo");
    assert_eq!(row["phase"], "implementing");
    assert_eq!(row["child_count"], 1, "{row}");
    assert!(
        prime["warnings"]
            .to_string()
            .contains("resume it from that checkout"),
        "{prime}"
    );
    let next = env.json(&main, &["next"]);
    assert!(next["next"].is_null());
    assert!(
        next["warnings"]
            .to_string()
            .contains("resume it from that checkout"),
        "{next}"
    );
    let by_parent = env.json(&main, &["list", "--parked", "--parent", &goal]);
    assert_eq!(by_parent["tasks"][0]["id"], kid);
    std::fs::remove_dir_all(wt.join("tasks")).unwrap();
    let prime = env.json(&main, &["prime"]);
    let row = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == parent)
        .unwrap()
        .clone();
    assert_eq!(row["title"], "Parent");
    assert!(row["status"].is_null() && row["phase"].is_null() && row["priority"].is_null());
    assert!(row.get("depends").is_none());
    assert_eq!(row["parallel"], false);
    assert_eq!(row["park"]["next_step"], "plan the pieces");
    assert!(
        prime["warnings"]
            .to_string()
            .contains("which is unavailable"),
        "{prime}"
    );
    let next = env.json(&main, &["next"]);
    assert!(next["next"].is_null());
    assert!(
        next["warnings"]
            .to_string()
            .contains("which is unavailable"),
        "{next}"
    );
    assert_eq!(
        env.json(&main, &["list", "--parked"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let filtered = env.json(&main, &["list", "--parked", "--tag", "x"]);
    assert!(filtered["tasks"].as_array().unwrap().is_empty());
    assert!(filtered["warnings"].to_string().contains("unavailable"));
    assert_eq!(
        env.fail(&main, &["list", "--parked", "--parent", &goal]),
        "task_not_found"
    );
}

#[test]
fn a_task_claimed_only_in_another_checkout_is_doing_in_prime_and_never_ready() {
    let mut env = TestEnv::new();
    let (main, wt) = two_roots(&mut env);
    // The registry names the main checkout, as it does for a real worktree.
    env.json(&main, &["init", "--prefix", "sci", "--force"]);
    let id = id_of(env.json(&wt, &["add", "Worktree only", "-p", "2"]));
    as_agent(&env, &wt, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    for args in [&["prime"][..], &["prime", "--all-projects"][..]] {
        let prime = env.json(&main, args);
        let doing = prime["doing"].as_array().unwrap();
        assert_eq!(doing.len(), 1, "{args:?}: {prime}");
        assert_eq!(doing[0]["id"], id);
        assert_eq!(doing[0]["title"], "Worktree only");
        assert_eq!(doing[0]["status"], "doing");
        assert_eq!(doing[0]["claim"]["worktree"], wt.to_str().unwrap());
        let expected = format!(
            "{id} is claimed in {}; resume it from that checkout",
            wt.display()
        );
        assert!(
            prime["warnings"].to_string().contains(&expected),
            "{args:?}: {prime}"
        );
    }
    let ready = env.json(&main, &["ready"]);
    assert!(ready["tasks"].as_array().unwrap().is_empty(), "{ready}");
    assert!(env.json(&main, &["next"])["next"].is_null());

    std::fs::remove_dir_all(wt.join("tasks")).unwrap();
    let prime = env.json(&main, &["prime"]);
    assert!(prime["doing"].as_array().unwrap().is_empty(), "{prime}");
    let expected = format!("{id} is claimed in {}, which is unavailable", wt.display());
    assert!(prime["warnings"].to_string().contains(&expected), "{prime}");
}

fn unregistered_checkout(prefix: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(path.join("tasks")).unwrap();
    std::fs::write(
        path.join("tasks/.config.toml"),
        format!("prefix = \"{prefix}\"\n"),
    )
    .unwrap();
    (dir, path)
}

#[test]
fn a_park_whose_task_file_was_deleted_in_its_own_checkout_still_warns_unavailable() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "finish",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ])
        .assert()
        .success();
    std::fs::remove_file(sci.join(format!("tasks/{id}.md"))).unwrap();

    for args in [vec!["list", "--parked"], vec!["prime"], vec!["quiet"]] {
        let v = env.json(&sci, &args);
        let rows = if args[0] == "prime" {
            &v["parked"]
        } else {
            &v["tasks"]
        };
        let row = &rows.as_array().unwrap()[0];
        assert_eq!(row["id"], id, "{args:?}: {v}");
        assert!(row["status"].is_null(), "store-only: {v}");
        assert_eq!(row["title"], "T");
        assert!(
            v["warnings"]
                .to_string()
                .contains("which is unavailable; the row shows the park entry only"),
            "{args:?}: {v}"
        );
    }
}

#[test]
fn quiet_lists_quiet_parks_across_projects_by_priority_then_park_time() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.json(nowhere.path(), &["quiet"]),
        serde_json::json!({"tasks": [], "warnings": []})
    );
    assert_eq!(env.pretty(nowhere.path(), &["quiet"]).trim(), "");

    let later = id_of(env.json(&sci, &["add", "Later capture", "-p", "2"]));
    let review = id_of(env.json(&sci, &["add", "Sheet", "-p", "0"]));
    let first = id_of(env.json(&fam, &["add", "First capture", "-p", "2"]));
    let urgent = id_of(env.json(&fam, &["add", "Urgent sweep", "-p", "1"]));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &review,
            "look",
            "--waiting-on",
            "user",
            "--reason",
            "review",
        ])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args([
            "park",
            &first,
            "run it",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "30",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &later,
            "run it too",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "45",
            "--needs",
            "headless",
        ])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args([
            "park",
            &urgent,
            "sweep",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ])
        .assert()
        .success();

    let v = env.json(nowhere.path(), &["quiet"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [urgent.as_str(), first.as_str(), later.as_str()],
        "{v}"
    );
    assert_eq!(v["warnings"], serde_json::json!([]));
    assert_eq!(v["tasks"][2]["park"]["needs"], "headless");
    assert_eq!(v["tasks"][2]["park"]["minutes"], 45);
    assert_eq!(
        v["tasks"][2]["park"]["worktree"],
        sci.to_string_lossy().as_ref()
    );
    let one = env.json(nowhere.path(), &["quiet", "-n", "1"]);
    assert_eq!(one["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(one["tasks"][0]["id"], urgent);
    let only_sci = env.json(nowhere.path(), &["quiet", "--project", "sci"]);
    assert_eq!(only_sci["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(only_sci["tasks"][0]["id"], later);
    let explicit = env.json(&sci, &["quiet", "--all-projects"]);
    assert_eq!(explicit["tasks"].as_array().unwrap().len(), 3);
    let text = env.pretty(nowhere.path(), &["quiet"]);
    let brief = text.lines().collect::<Vec<_>>();
    assert!(
        brief[0].starts_with(&urgent)
            && brief[0].contains("P1")
            && brief[0].contains("idle")
            && brief[0].contains("5 min")
            && brief[0].contains("Urgent sweep"),
        "{text}"
    );
    assert!(brief[1].trim_start().starts_with("next: sweep"), "{text}");
    assert!(
        brief[2].trim_start().starts_with("in:") && brief[2].contains(&*fam.to_string_lossy()),
        "{text}"
    );
    assert_eq!(brief[3], "", "one blank line between briefs: {text}");
    assert!(
        text.contains("headless") && text.contains("45 min"),
        "{text}"
    );
}

#[test]
fn quiet_resolves_from_the_recorded_checkout_before_the_registered_copy() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    let (_keep, wt) = unregistered_checkout("sci");
    let id = id_of(env.json(&main, &["add", "Capture", "-p", "3"]));
    std::fs::copy(
        main.join(format!("tasks/{id}.md")),
        wt.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    env.json(&main, &["done", &id, "closed on main"]);
    env.json(&wt, &["edit", &id, "-p", "1"]);
    as_agent(&env, &wt, "agent-a")
        .args([
            "park",
            &id,
            "capture",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "20",
        ])
        .assert()
        .success();
    let parked = env.json(&main, &["list", "--parked"]);
    assert!(parked["tasks"].as_array().unwrap().is_empty(), "{parked}");
    let v = env.json(&main, &["quiet"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["tasks"][0]["id"], id);
    assert_eq!(v["tasks"][0]["status"], "todo");
    assert_eq!(v["tasks"][0]["priority"], 1);
    assert_eq!(
        v["tasks"][0]["park"]["worktree"],
        wt.to_string_lossy().as_ref()
    );
    assert!(
        v["warnings"]
            .to_string()
            .contains("resume it from that checkout"),
        "{v}"
    );
    std::fs::remove_dir_all(wt.join("tasks")).unwrap();
    let v = env.json(&main, &["quiet"]);
    assert!(v["tasks"].as_array().unwrap().is_empty(), "{v}");
    assert!(
        v["warnings"]
            .to_string()
            .contains("showing the registered copy"),
        "{v}"
    );
}

#[test]
fn quiet_keeps_scan_warnings_when_the_queue_is_empty() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let v = env.json(&sci, &["quiet"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1, "{v}");
    assert!(v["warnings"][0].as_str().unwrap().contains("fam"), "{v}");
    let out = env.cmd(&sci).args(["--pretty", "quiet"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
    assert!(String::from_utf8_lossy(&out.stderr).contains("unreachable"));
}

#[test]
fn quiet_includes_a_store_only_entry_last() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let real = id_of(env.json(&sci, &["add", "Real", "-p", "3"]));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &real,
            "run",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "10",
        ])
        .assert()
        .success();
    let path = env.claim_store("sci");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("[parks.\"sci-0000ff\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\nworktree = \"/gone\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"finish\"\nwaiting_on = \"user\"\nreason = \"quiet\"\nneeds = \"idle\"\nminutes = 15\ntitle = \"Ghost\"\n");
    std::fs::write(&path, text).unwrap();
    let v = env.json(&sci, &["quiet", "--project", "sci"]);
    let rows = v["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{v}");
    assert_eq!(rows[0]["id"], real);
    assert_eq!(rows[1]["id"], "sci-0000ff");
    assert!(rows[1]["priority"].is_null());
    assert_eq!(rows[1]["title"], "Ghost");
    assert_eq!(rows[1]["park"]["minutes"], 15);
    let text = env.pretty(&sci, &["quiet", "--project", "sci"]);
    assert!(text.contains("sci-0000ff  P-"), "{text}");
}

#[test]
fn quiet_ignores_unavailable_non_quiet_parks_but_parked_lists_warn() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let quiet = id_of(env.json(&sci, &["add", "Quiet", "-p", "1"]));
    let review = id_of(env.json(&sci, &["add", "Review", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &quiet,
            "run",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ])
        .assert()
        .success();
    let (_keep, wt) = unregistered_checkout("sci");
    std::fs::copy(
        sci.join(format!("tasks/{review}.md")),
        wt.join(format!("tasks/{review}.md")),
    )
    .unwrap();
    as_agent(&env, &wt, "agent-a")
        .args([
            "park",
            &review,
            "review",
            "--waiting-on",
            "user",
            "--reason",
            "review",
        ])
        .assert()
        .success();
    std::fs::remove_file(sci.join(format!("tasks/{review}.md"))).unwrap();
    std::fs::remove_file(wt.join(format!("tasks/{review}.md"))).unwrap();

    let quiet_out = env.json(&sci, &["quiet"]);
    assert_eq!(
        quiet_out["tasks"].as_array().unwrap().len(),
        1,
        "{quiet_out}"
    );
    assert_eq!(quiet_out["tasks"][0]["id"], quiet);
    assert!(
        !quiet_out["warnings"].to_string().contains(&review),
        "{quiet_out}"
    );

    let parked = env.json(&sci, &["list", "--parked"]);
    assert!(
        parked["warnings"]
            .to_string()
            .contains("which is unavailable"),
        "{parked}"
    );
}

#[test]
fn list_parked_orders_by_park_time_and_conflicts_with_sort() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let first = id_of(env.json(&sci, &["add", "First", "-p", "1"]));
    let second = id_of(env.json(&sci, &["add", "Second", "-p", "3"]));
    env.json(&sci, &["add", "Neither", "-p", "0"]);
    as_agent(&env, &sci, "agent-a")
        .args(["park", &first, "a"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &second, "b", "--waiting-on", "user"])
        .assert()
        .success();
    let listed = env.json(&sci, &["list", "--parked"]);
    let ids: Vec<&str> = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![second.as_str(), first.as_str()]);
    assert_eq!(listed["tasks"][0]["phase"], "implementing");
    assert_eq!(
        env.json(&sci, &["list", "--parked", "--status", "todo"])["tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for flag in ["--sort", "--reverse"] {
        let mut args = vec!["list", "--parked", flag];
        if flag == "--sort" {
            args.push("updated");
        }
        assert_eq!(
            env.cmd(&sci).args(&args).output().unwrap().status.code(),
            Some(2)
        );
    }
}

#[test]
fn ready_omits_a_task_claimed_from_another_root_and_says_why() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--size", "s"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();

    assert_eq!(env.json(&b, &["ready"])["tasks"][0]["id"], id);

    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    for v in [
        env.json(&b, &["ready"]),
        serde_json::from_slice(
            &as_agent(&env, &b, "agent-a")
                .args(["ready"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap(),
    ] {
        assert_eq!(v["tasks"].as_array().unwrap().len(), 0, "{v}");
        assert!(
            v["warnings"].as_array().unwrap().iter().any(|w| {
                let w = w.as_str().unwrap();
                w.contains(&id) && w.contains("agent-a")
            }),
            "a silent omission is worse than an explained one: {v}"
        );
    }
    let v: serde_json::Value = serde_json::from_slice(
        &as_agent(&env, &b, "agent-a")
            .args(["next"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(v["next"], serde_json::Value::Null, "{v}");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("agent-a")),
        "{v}"
    );
}

#[test]
fn ready_parallel_filters_and_still_honours_limit() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // C outranks both marked tasks, so it heads the unfiltered ready list. That is what
    // makes the -n 1 assertion below able to catch a truncate-before-filter regression:
    // with the filter in the wrong place, `--parallel -n 1` truncates to [C] and then
    // filters to nothing. Give them distinct priorities — equal priority and size would
    // fall through to `created`, and whenever a marked task happened to sort first the
    // broken order would still pass.
    env.json(&dir, &["add", "C", "-p", "0"]);
    let a = id_of(env.json(&dir, &["add", "A", "-p", "1", "--parallel"]));
    let b = id_of(env.json(&dir, &["add", "B", "-p", "2", "--parallel"]));

    let ids = |v: serde_json::Value| -> Vec<String> {
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };

    assert_eq!(ids(env.json(&dir, &["ready"])).len(), 3);
    assert_eq!(
        ids(env.json(&dir, &["ready", "--parallel"])),
        [a.clone(), b.clone()],
        "marked only, in the usual ready order"
    );
    assert_eq!(
        ids(env.json(&dir, &["ready", "--parallel", "-n", "1"])),
        [a],
        "the limit applies after the filter"
    );

    // A doing task is not ready, so it never joins the marked set.
    env.json(&dir, &["start", &b]);
    assert_eq!(ids(env.json(&dir, &["ready", "--parallel"])).len(), 1);
}

#[test]
fn prime_shows_a_claim_made_in_another_root_and_warns_about_divergence() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();

    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    let v = env.json(&b, &["prime"]);
    assert!(
        v["doing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == id.as_str()),
        "a claim made in another worktree shows as doing here: {v}"
    );
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("conflict")
        }),
        "the divergent copies are called out: {v}"
    );
}

#[test]
fn prime_warns_about_a_stale_claim_over_a_local_todo() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "dead-agent", false);

    let v = env.json(&sci, &["prime"]);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains("dead-agent") && w.contains(&id)
        }),
        "{v}"
    );
}

#[test]
fn one_prime_never_contradicts_itself_about_a_claim() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--size", "s"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    let v = env.json(&b, &["prime"]);
    let row = v["doing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id.as_str())
        .unwrap();
    assert_eq!(row["claim"]["live"], true, "{v}");
    assert!(
        !v["ready"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == id.as_str()),
        "and the ready list agrees: {v}"
    );
}

#[test]
fn prime_keeps_a_live_claim_on_a_locally_closed_task() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    env.cmd(&b)
        .args(["done", &id, "closed here"])
        .assert()
        .success();
    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    let v = env.json(&b, &["prime"]);
    let row = v["doing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id.as_str())
        .unwrap();
    assert_eq!(row["status"], "done", "{v}");
    assert_eq!(row["claim"]["live"], true, "{v}");
}

// The error shape is {"error": {"kind", "detail"}} — there is no `message` field.
fn err_kind(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    v["error"]["kind"].as_str().unwrap().to_string()
}

fn err_detail(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    v["error"]["detail"].as_str().unwrap().to_string()
}

#[test]
fn a_live_claim_from_another_session_refuses_start() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "claimed");
    assert!(err_detail(&out).contains("agent-a"), "{}", err_detail(&out));
}

#[test]
fn force_takeover_records_a_note_naming_the_displaced_session() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["start", "--force", &id])
        .assert()
        .success();

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("agent-a"),
        "the takeover is recorded in the notes: {raw}"
    );
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-b"
    );
}

/// Machine details no task record may carry: a claim naming them, whose pid is either this
/// test process (live) or one no process holds on this boot (stale, "pid … is gone").
const LEAK_HOST: &str = "leakhost-q7z";
const LEAK_WORKTREE: &str = "/sync-root-x9k/proj/.worktrees/leak";
const GONE_PID: u32 = 4_194_303;

fn write_detailed_claim(env: &TestEnv, prefix: &str, id: &str, session: &str, live: bool) {
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let (pid, pid_start) = if live {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let rest = stat.rsplit_once(") ").unwrap().1.to_string();
        let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
        (std::process::id(), start)
    } else {
        (GONE_PID, 1)
    };
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {pid}\n\
         pid_start = {pid_start}\nboot_id = \"{}\"\nhost = \"{LEAK_HOST}\"\n\
         worktree = \"{LEAK_WORKTREE}\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n",
        boot.trim()
    ));
    std::fs::write(&path, text).unwrap();
}

fn warnings_text(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    warnings_of(&v).join("\n")
}

fn assert_no_machine_details(raw: &str, pid: u32) {
    for leak in [
        LEAK_HOST,
        LEAK_WORKTREE,
        &format!("pid {pid}"),
        "host ",
        "worktree ",
    ] {
        assert!(!raw.contains(leak), "the record carries {leak:?}: {raw}");
    }
}

#[test]
fn a_forced_takeover_note_names_the_session_without_machine_details() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_detailed_claim(&env, "sci", &id, "agent-a", true);

    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", "--force", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let warnings = warnings_text(&out);
    let pid = std::process::id();
    for detail in [LEAK_HOST, LEAK_WORKTREE, &format!("pid {pid}")] {
        assert!(
            warnings.contains(detail),
            "the warning keeps {detail:?}: {warnings}"
        );
    }

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("took over session agent-a (owner someone, live, forced)"),
        "{raw}"
    );
    assert_no_machine_details(&raw, pid);
    let claim = &env.json(&sci, &["show", &id])["claim"];
    assert_eq!(claim["session"], "agent-b");
    assert!(!claim["host"].as_str().unwrap().is_empty(), "{claim}");
    assert!(!claim["worktree"].as_str().unwrap().is_empty(), "{claim}");
}

#[test]
fn a_stale_takeover_note_names_the_session_without_machine_details() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_detailed_claim(&env, "sci", &id, "agent-a", false);

    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let warnings = warnings_text(&out);
    let gone = format!("pid {GONE_PID} is gone");
    for detail in [LEAK_HOST, LEAK_WORKTREE, gone.as_str()] {
        assert!(
            warnings.contains(detail),
            "the warning keeps {detail:?}: {warnings}"
        );
    }

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("took over session agent-a (owner someone, stale)"),
        "{raw}"
    );
    assert_no_machine_details(&raw, GONE_PID);
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-b"
    );
}

#[test]
fn a_park_over_a_stale_claim_keeps_machine_details_out_of_its_note() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_detailed_claim(&env, "sci", &id, "agent-a", false);

    let out = as_agent(&env, &sci, "agent-b")
        .args(["park", &id, "carry on"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let warnings = warnings_text(&out);
    for detail in [LEAK_HOST, LEAK_WORKTREE] {
        assert!(
            warnings.contains(detail),
            "the warning keeps {detail:?}: {warnings}"
        );
    }

    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    assert!(raw.contains("parked (waiting on agent): carry on"), "{raw}");
    assert!(!raw.contains("took over"), "{raw}");
    assert_no_machine_details(&raw, GONE_PID);
    let park = &env.json(&sci, &["show", &id])["park"];
    assert_eq!(park["session"], "agent-b");
    assert!(!park["host"].as_str().unwrap().is_empty(), "{park}");
    assert!(!park["worktree"].as_str().unwrap().is_empty(), "{park}");
}

#[test]
fn a_displaced_session_cannot_close_the_task_it_lost() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-b")
        .args(["start", "--force", &id])
        .assert()
        .success();

    for args in [
        vec!["done", &id, "landed"],
        vec!["drop", &id, "nope"],
        vec!["block", &id, "waiting"],
        vec!["edit", &id, "--status", "done"],
    ] {
        let out = as_agent(&env, &sci, "agent-a")
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "A must not close via {args:?}");
        assert_eq!(err_kind(&out), "claimed", "{args:?}");
    }
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-b"
    );
}

#[test]
fn release_follows_the_claim_not_the_local_doing_status() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    // Root B's copy is the pre-claim file: still `todo`, the ordinary cross-worktree case.
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();

    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    // The same session closes it from root B, where the local status was never `doing`.
    as_agent(&env, &b, "agent-a")
        .args(["done", &id, "landed"])
        .assert()
        .success();

    assert!(
        env.json(&a, &["show", &id])["claim"].is_null(),
        "released even though this checkout never left doing"
    );
}

#[test]
fn one_checkouts_closed_copy_does_not_prune_a_live_claim() {
    let mut env = TestEnv::new();
    let (a, b) = two_roots(&mut env);
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));
    std::fs::copy(
        a.join(format!("tasks/{id}.md")),
        b.join(format!("tasks/{id}.md")),
    )
    .unwrap();

    // Both branch states are established *before* the claim exists. Otherwise A's own close
    // is refused — correctly — and the test never reaches what it is about.
    env.json(&a, &["edit", &id, "--status", "done"]);
    as_agent(&env, &b, "agent-b")
        .args(["start", &id])
        .assert()
        .success();

    // `note` is the right probe from A: never refused, and it touches the store.
    as_agent(&env, &a, "agent-a")
        .args(["note", &id, "still here"])
        .assert()
        .success();

    assert_eq!(
        env.json(&b, &["show", &id])["claim"]["session"],
        "agent-b",
        "one checkout's view cannot establish that a shared claim is obsolete"
    );
}

#[test]
fn done_retries_a_failed_release_after_a_status_edit() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let store = env.claim_store("sci");

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "continue here"])
        .assert()
        .success();
    use std::os::unix::fs::PermissionsExt;
    // The existing lock is writable, but a new atomic store temp file cannot be created.
    let state_dir = store.parent().unwrap();
    let original = std::fs::metadata(state_dir).unwrap().permissions();
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-a")
        .env("CODEX_SESSION_ID", "native-a")
        .args(["edit", &id, "--status", "done"])
        .output();
    std::fs::set_permissions(state_dir, original).unwrap();
    let out = out.unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        result["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&format!("run `tasks done {id}"))
        }),
        "{result}"
    );
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "done");
    assert_eq!(shown["park"]["next_step"], "continue here");
    let notes_before = shown["task"]["notes"].as_array().unwrap().clone();
    assert_eq!(notes_before.last().unwrap()["text"], "done");
    assert_eq!(
        notes_before.last().unwrap()["harness_session"],
        "codex:native-a"
    );

    // `start --force` cannot recover this: can_transition rejects done -> doing.
    let out = as_agent(&env, &sci, "agent-a")
        .args(["start", "--force", &id])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "invalid_transition");

    // The advertised status command retries the release even though the task is already done.
    as_agent(&env, &sci, "agent-a")
        .args(["done", &id, "landed"])
        .assert()
        .success();
    let shown = env.json(&sci, &["show", &id]);
    assert!(shown["claim"].is_null(), "{shown}");
    assert!(shown["park"].is_null(), "{shown}");
    assert_eq!(
        &shown["task"]["notes"].as_array().unwrap()[..notes_before.len()],
        notes_before
    );
    assert_eq!(
        shown["task"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["text"] == "done")
            .count(),
        1
    );
}

#[test]
fn a_stale_claim_is_taken_over_without_force_but_with_a_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    write_claim(&env, "sci", &id, "dead-agent", false);

    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", &id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("dead-agent")),
        "taking over a stale claim names the displaced holder: {v}"
    );
}

#[test]
fn a_repeated_start_by_the_owner_keeps_the_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-a"
    );
}

#[test]
fn note_fails_before_appending_when_identity_cannot_be_resolved() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let before = env.read(&sci, &format!("tasks/{id}.md"));

    // A corrupt store is the reachable version of "the heartbeat cannot proceed".
    let path = env.claim_store("sci");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not valid toml = [").unwrap();

    let out = env.cmd(&sci).args(["note", &id, "hello"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        env.read(&sci, &format!("tasks/{id}.md")),
        before,
        "a note that reports failure must not have landed; a retry would duplicate it"
    );
}

#[test]
fn closure_force_never_authorizes_a_foreign_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T"]));
    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let before = env.read(&dir, &format!("tasks/{id}.md"));
    for args in [
        vec!["done", "--force", &id],
        vec!["edit", &id, "--force", "--status", "done"],
        vec!["edit", &id, "--status", "doing"],
    ] {
        let out = as_agent(&env, &dir, "agent-b")
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(err_kind(&out), "claimed", "{args:?}");
    }
    let out = as_agent(&env, &dir, "agent-b")
        .args(["edit", &id, "--force", "--status", "doing"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
    let editor = editor_script(&dir, "sed -i 's/^status: doing$/status: done/' \"$1\"");
    let out = as_agent(&env, &dir, "agent-b")
        .env("EDITOR", editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), before);
    assert_eq!(
        env.json(&dir, &["show", &id])["claim"]["session"],
        "agent-a"
    );
}

#[test]
fn ordinary_locked_writes_prune_stale_claims_without_reviving_them() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let stale = id_of(env.json(&dir, &["add", "Stale"]));
    let other = id_of(env.json(&dir, &["add", "Other"]));
    for args in [
        vec!["note", stale.as_str(), "heartbeat"],
        vec!["note", other.as_str(), "unrelated"],
        vec!["edit", other.as_str(), "--title", "Edited"],
        vec!["dep", other.as_str(), "--on", stale.as_str()],
    ] {
        write_claim(&env, "sci", &stale, "dead-agent", false);
        as_agent(&env, &dir, "dead-agent")
            .args(&args)
            .assert()
            .success();
        assert!(
            env.json(&dir, &["show", &stale])["claim"].is_null(),
            "{args:?}"
        );
    }
}

#[test]
fn rejected_close_and_editor_do_not_persist_claim_intent() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let dependency = id_of(env.json(&dir, &["add", "Dependency"]));
    let id = id_of(env.json(&dir, &["add", "T", "--depends", &dependency]));
    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let store = env.claim_store("sci");
    let before = std::fs::read_to_string(&store).unwrap();
    let out = as_agent(&env, &dir, "agent-a")
        .args(["done", &id])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "open_dependencies");
    assert_eq!(std::fs::read_to_string(&store).unwrap(), before);
    let editor = editor_script(
        &dir,
        &format!(
            "sed -i 's/^title: .*/title: Racer/' tasks/{dependency}.md\nsed -i 's/^status: todo$/status: doing/' \"$1\""
        ),
    );
    let out = as_agent(&env, &dir, "agent-a")
        .env("EDITOR", editor)
        .args(["edit", &dependency])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "concurrent_modification");
    assert_eq!(std::fs::read_to_string(&store).unwrap(), before);
}

#[test]
fn closing_releases_and_unblock_does_not_acquire() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for close in ["done", "drop", "block"] {
        let id = id_of(env.json(&dir, &["add", "T"]));
        as_agent(&env, &dir, "agent-a")
            .args(["start", &id])
            .assert()
            .success();
        assert_eq!(
            env.json(&dir, &["show", &id])["claim"]["session"],
            "agent-a"
        );
        as_agent(&env, &dir, "agent-a")
            .args([close, &id])
            .assert()
            .success();
        assert!(env.json(&dir, &["show", &id])["claim"].is_null());
        if close == "block" {
            as_agent(&env, &dir, "agent-a")
                .args(["unblock", &id])
                .assert()
                .success();
            assert!(env.json(&dir, &["show", &id])["claim"].is_null());
        }
    }
}

#[test]
fn invalid_parent_does_not_persist_acquire_or_stale_pruning() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T"]));
    write_claim(&env, "sci", "sci-ffffff", "dead-agent", false);
    let store = env.claim_store("sci");
    let before = std::fs::read_to_string(&store).unwrap();
    let out = as_agent(&env, &dir, "agent-a")
        .args(["edit", &id, "--status", "doing", "--parent", "sci-ffffff"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "unresolvable_id");
    assert_eq!(std::fs::read_to_string(store).unwrap(), before);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "todo");
}

use std::fs::File;
use std::time::{Duration, Instant};

/// Hold the project's mutation lock from the test process itself.
fn hold_project_lock(env: &TestEnv, prefix: &str) -> File {
    let path = env
        .claim_store(prefix)
        .with_file_name(format!("{prefix}.lock"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .unwrap();
    file.lock().unwrap();
    file
}

/// Wait for a child, but never forever: a regression that makes a command block must fail
/// the assertion, not hang the suite while still holding the lock.
fn wait_bounded(child: &mut std::process::Child, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Collect a child's output, killing it if it outlives `limit`. `None` means it had to be
/// killed.
///
/// Every *final* wait goes through this too, not just the ones being measured. Releasing a
/// handshake or dropping the test's lock only removes the blocker the test knows about; a
/// command that deadlocks for some other reason — an editor path that kept the lock and then
/// tries to reacquire it, say — would still park a bare `wait()` forever and hang the suite
/// with the failure invisible.
fn reap(mut child: std::process::Child, limit: Duration) -> Option<std::process::Output> {
    if wait_bounded(&mut child, limit) {
        return Some(child.wait_with_output().expect("already exited"));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

const REAP: Duration = Duration::from_secs(30);

// Reap every child *before* asserting or unwrapping anything. A panic partway through
// leaves the children behind it running, which outlives the test and can wedge whatever
// runs next.

#[test]
fn a_write_command_waits_for_the_project_lock_and_a_read_command_does_not() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let held = hold_project_lock(&env, "sci");

    let mut writer = env.raw(&sci);
    writer
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let mut writing = writer.spawn().unwrap();

    // Spawned, not called synchronously: if a regression made reads take the lock, a
    // synchronous call here would deadlock against the lock this test is holding.
    let mut reader = env.raw(&sci);
    reader.args(["show", &id]);
    let mut reading = reader.spawn().unwrap();

    // Observations only — no assertions while the lock is held.
    let read_finished = wait_bounded(&mut reading, Duration::from_secs(10));
    // Timing-based, and deliberately so: this says the writer has not finished, which a
    // slow unlocked writer would also satisfy. It fails reliably against an implementation
    // that takes no lock, which is what it is for.
    let writer_still_blocked = !wait_bounded(&mut writing, Duration::from_millis(300));

    drop(held);

    let read = reap(reading, REAP);
    let wrote = reap(writing, REAP);

    assert!(
        read_finished,
        "read commands must not take the mutation lock"
    );
    assert!(
        read.expect("the read command never exited")
            .status
            .success()
    );
    let wrote = wrote.expect("the write command never exited after the lock was released");
    assert!(
        writer_still_blocked,
        "a write command must wait while the project lock is held"
    );
    assert!(
        wrote.status.success(),
        "{}",
        String::from_utf8_lossy(&wrote.stderr)
    );
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-a"
    );
}

#[test]
fn simultaneous_starts_produce_exactly_one_winner() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // Spawn under the held lock to force contention; scheduling still determines
    // whether every child reaches acquisition before release.
    let held = hold_project_lock(&env, "sci");
    let children: Vec<_> = (0..6)
        .map(|n| {
            let mut cmd = env.raw(&sci);
            cmd.args(["start", &id])
                .env("TASKS_SESSION", format!("agent-{n}"))
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    // Reap them all first: a panic inside the map would strand the children behind it.
    let reaped: Vec<_> = children.into_iter().map(|c| reap(c, REAP)).collect();
    assert!(
        reaped.iter().all(Option::is_some),
        "a queued start never exited"
    );
    let outs: Vec<_> = reaped.into_iter().flatten().collect();
    assert_eq!(
        outs.iter().filter(|o| o.status.success()).count(),
        1,
        "exactly one session may hold the claim"
    );
    for out in outs.iter().filter(|o| !o.status.success()) {
        assert_eq!(err_kind(out), "claimed");
    }
    assert_eq!(env.json(&sci, &["show", &id])["claim"]["live"], true);
}

#[test]
fn concurrent_claims_on_different_tasks_are_all_kept() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let ids: Vec<String> = (0..6)
        .map(|n| id_of(env.json(&sci, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();

    let held = hold_project_lock(&env, "sci");
    let children: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(n, id)| {
            let mut cmd = env.raw(&sci);
            cmd.args(["start", id])
                .env("TASKS_SESSION", format!("agent-{n}"))
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    let reaped: Vec<_> = children.into_iter().map(|c| reap(c, REAP)).collect();
    assert!(
        reaped.iter().all(Option::is_some),
        "a queued start never exited"
    );
    for out in reaped.into_iter().flatten() {
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // The store is one whole file per prefix, so an unserialized writer drops the claims it
    // never read.
    for id in &ids {
        assert_eq!(
            env.json(&sci, &["show", id])["claim"]["live"],
            true,
            "{id} lost its claim to a concurrent write"
        );
    }
}

#[test]
fn concurrent_notes_and_a_status_change_lose_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    let held = hold_project_lock(&env, "sci");
    let mut children: Vec<_> = (0..5)
        .map(|n| {
            let mut cmd = env.raw(&sci);
            cmd.args(["note", &id, &format!("line {n}")])
                .env("TASKS_SESSION", "agent-a")
                .env("TASKS_SESSION_PID", std::process::id().to_string());
            cmd.spawn().unwrap()
        })
        .collect();
    let mut status = env.raw(&sci);
    status
        .args(["start", &id])
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    children.push(status.spawn().unwrap());
    std::thread::sleep(Duration::from_millis(300));
    drop(held);

    let reaped: Vec<_> = children.into_iter().map(|c| reap(c, REAP)).collect();
    assert!(
        reaped.iter().all(Option::is_some),
        "a queued write never exited"
    );
    for out in reaped.into_iter().flatten() {
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // `note` rewrites the whole markdown file, so an unserialized note clobbers whatever
    // landed between its read and its write.
    let raw = env.read(&sci, &format!("tasks/{id}.md"));
    for n in 0..5 {
        assert!(
            raw.contains(&format!("line {n}")),
            "note {n} was lost: {raw}"
        );
    }
    assert!(
        raw.contains("status: doing"),
        "the status change was lost: {raw}"
    );
}

#[test]
fn a_concurrent_edit_during_an_interactive_edit_is_rejected_and_leaks_no_claim() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // A handshake, not a sleep: the editor announces that it is inside its unlocked window
    // and waits to be released, so the test never depends on how fast the machine is.
    let ready = sci.join("editor-ready");
    let go = sci.join("editor-go");
    let script = editor_script(
        &sci,
        &format!(
            "touch '{}'\nwhile [ ! -e '{}' ]; do sleep 0.02; done\nsed -i 's/^status: todo/status: doing/' \"$1\"",
            ready.display(),
            go.display()
        ),
    );

    let mut editing = env.raw(&sci);
    editing
        .args(["edit", &id])
        .env("EDITOR", &script)
        .env("TASKS_SESSION", "agent-a")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let child = editing.spawn().unwrap();

    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready.exists() {
        if Instant::now() >= deadline {
            // Release and reap before failing, so a stuck editor cannot outlive the test.
            // Bounded, because writing `go` releases this test's handshake but cannot
            // guarantee the command exits.
            std::fs::write(&go, "").unwrap();
            reap(child, REAP);
            panic!("the editor never started");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // The editor is holding no lock now, so this must succeed rather than block — but it is
    // spawned and bounded anyway, because if a regression made it block, a synchronous call
    // would hang here with the editor child still parked on its handshake.
    let mut noting = env.raw(&sci);
    noting
        .args(["note", &id, "landed first"])
        .env("TASKS_SESSION", "agent-b")
        .env("TASKS_SESSION_PID", std::process::id().to_string());
    let mut noting = noting.spawn().unwrap();
    let note_finished = wait_bounded(&mut noting, REAP);

    // Release the editor whatever happened, so the child is always reaped. Bounded, because
    // the handshake is the only blocker this test controls: an editor path that kept the
    // lock and then tried to reacquire it would deadlock past the `go` file.
    std::fs::write(&go, "").unwrap();
    // Both children are reaped before anything can panic. An `expect` on the first would
    // abandon the second, leaving a live process behind for the rest of the suite.
    let edited = reap(child, REAP);
    let noted = reap(noting, REAP);

    assert!(
        note_finished,
        "the concurrent note blocked; the editor holds no lock here"
    );
    let out = edited.expect("the editor never exited after the handshake");
    let note_out = noted.expect("the concurrent note never exited");
    assert!(
        note_out.status.success(),
        "{}",
        String::from_utf8_lossy(&note_out.stderr)
    );
    assert_eq!(err_kind(&out), "concurrent_modification");
    // The editor's `transition` ran before the comparison. Because no claim is persisted
    // until `save`, the rejected edit must not have left one behind.
    assert!(
        env.json(&sci, &["show", &id])["claim"].is_null(),
        "a rejected edit acquired a claim"
    );
}

#[test]
fn a_failed_task_write_leaves_no_claim_behind() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let before = env.read(&sci, &format!("tasks/{id}.md"));

    // Read still works; `atomic_write` cannot create its temp file.
    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-a")
        .env("CODEX_SESSION_ID", "native-a")
        .args(["start", &id])
        .output();
    std::fs::set_permissions(&tasks_dir, original).unwrap();
    let out = out.unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(env.read(&sci, &format!("tasks/{id}.md")), before);

    assert!(
        env.json(&sci, &["show", &id])["claim"].is_null(),
        "acquire is rolled back when the task write fails"
    );
}

#[test]
fn a_failed_takeover_restores_the_previous_owners_claim() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", "--force", &id])
        .output();
    std::fs::set_permissions(&tasks_dir, original).unwrap();
    let out = out.unwrap();
    assert_eq!(out.status.code(), Some(1));

    // Rollback restores what was there; a blanket removal would unclaim A's live work.
    assert_eq!(
        env.json(&sci, &["show", &id])["claim"]["session"],
        "agent-a",
        "a failed takeover must not unclaim the previous holder"
    );
}

#[test]
fn a_failed_start_restores_the_displaced_park() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "continue here", "--waiting-on", "user"])
        .assert()
        .success();
    let before = std::fs::read(env.claim_store("sci")).unwrap();

    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-b")
        .args(["start", &id])
        .output();
    std::fs::set_permissions(&tasks_dir, original).unwrap();
    assert_eq!(out.unwrap().status.code(), Some(1));

    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), before);
    let shown = env.json(&sci, &["show", &id]);
    assert!(shown["claim"].is_null(), "{shown}");
    assert_eq!(shown["park"]["next_step"], "continue here", "{shown}");
    assert_eq!(shown["park"]["waiting_on"], "user", "{shown}");
}

#[test]
fn the_reported_sequence_start_then_create_the_worktree() {
    let mut env = TestEnv::new();
    let a = env.init("sci");
    let id = id_of(env.json(&a, &["add", "T", "-p", "2"]));

    // The bytes a later worktree would branch from: captured *before* the claim exists.
    let committed = env.read(&a, &format!("tasks/{id}.md"));
    as_agent(&env, &a, "agent-a")
        .args(["start", &id])
        .assert()
        .success();

    // Only now does the second worktree come into being, from the pre-start state.
    let b = env.init_forced("sci");
    std::fs::write(b.join(format!("tasks/{id}.md")), &committed).unwrap();

    let v = env.json(&b, &["prime"]);
    assert!(
        v["doing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == id.as_str()),
        "the claim is visible in a worktree created after the start: {v}"
    );
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&id) && w.contains("conflict")
        }),
        "and the divergence is called out: {v}"
    );
}
/// Run git in `dir` under a fixed identity, asserting success.
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

/// A project that is its own git repository, with one task committed, plus a second
/// worktree branched from that commit. Returns the two project roots and the task id.
fn repo_with_worktree(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf, String) {
    repo_with_worktree_as(env, "sci")
}

/// `repo_with_worktree` for a project with the given prefix.
fn repo_with_worktree_as(
    env: &mut TestEnv,
    prefix: &str,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let main = env.init(prefix);
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(
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
    (main, side, id)
}

fn warnings_of(v: &serde_json::Value) -> Vec<String> {
    v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect()
}

/// The `detail` of a `stale_copy` refusal from running `args` in `dir`.
fn stale_detail(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> String {
    let error = error_of(env, dir, args);
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    error["error"]["detail"].as_str().unwrap().to_string()
}

/// The `tasks -C …` line printed in a refusal's detail, split into words the way a POSIX
/// shell splits it, without the program name.
fn retry_words(detail: &str) -> Vec<String> {
    let at = detail.find("tasks -C ").expect("a retry");
    let line = &detail[at..];
    let line = line
        .strip_suffix("; supply the same input on stdin")
        .unwrap_or(line);
    let rest = line.strip_prefix("tasks ").unwrap();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "for word in {rest}; do printf '%s\\0' \"$word\"; done"
        ))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout)
        .unwrap()
        .split_terminator('\0')
        .map(str::to_string)
        .collect()
}

/// A task started by `agent-a` in the main checkout, committed, and only then branched into
/// a worktree (the prescribed order). Both copies are stamped equal and old, so the next
/// write in either checkout is the newer one.
fn started_then_branched(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    as_agent(env, &main, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "start"]);
    let side = main.join("wt");
    git(
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
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-02T00:00:00Z");
    }
    (main, side, id)
}

#[test]
fn halt_start_uncommitted_registered_incident_blocks_existing_worktree_immediately() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert!(main.join(format!("tasks/{halt}.md")).exists());
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_start_side_closure_does_not_lift_until_registered_record_closes() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    std::fs::copy(
        main.join(format!("tasks/{halt}.md")),
        side.join(format!("tasks/{halt}.md")),
    )
    .unwrap();
    env.json(&side, &["done", &halt, "local resolution"]);
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(
        !std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains(&target)
    );
    // The record-home guard now refuses to close an older sibling copy. Remove the
    // side fixture after proving its closure did not lift the registered halt.
    std::fs::remove_file(side.join(format!("tasks/{halt}.md"))).unwrap();
    env.json(&main, &["done", &halt, "registered resolution"]);
    env.json(&side, &["start", &target]);
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "doing"
    );
    assert!(
        std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains(&target)
    );
}

#[test]
fn halt_start_unreadable_registered_checkout_refuses_force() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    std::fs::remove_file(main.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]),
        "config"
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert!(!env.claim_store("sci").exists());
}

#[test]
fn halt_start_unregistered_checkout_uses_local_authority() {
    let mut env = TestEnv::new();
    let project = env.init("sci");
    let target = id_of(env.json(&project, &["add", "Ordinary", "-p", "2"]));
    std::fs::remove_file(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    env.json(&project, &["start", &target]);
    assert_eq!(
        env.json(&project, &["show", &target])["task"]["status"],
        "doing"
    );
}

#[test]
fn halt_override_records_authority_and_target_before_claimed_start() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(
        &side,
        &["start", &target, "--force", "--reason", "emergency"],
    );
    let authority = env.json(&main, &["show", &halt]);
    let local = env.json(&side, &["show", &target]);
    assert_eq!(local["task"]["status"], "doing");
    assert!(
        authority
            .to_string()
            .contains(&format!("halt override: attempted {target}"))
    );
    assert!(
        local
            .to_string()
            .contains(&format!("halt override: started past {halt}"))
    );
    assert!(env.claim_store("sci").exists());
}

#[test]
fn halt_start_equal_priority_is_allowed_and_force_reason_is_validated() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Incident", "-p", "1", "--tag", "halt"]);
    let equal = id_of(env.json(&dir, &["add", "Equal", "-p", "1"]));
    let lower = id_of(env.json(&dir, &["add", "Lower", "-p", "3"]));
    env.json(&dir, &["start", &equal]);
    assert_eq!(env.fail(&dir, &["start", &lower]), "halted");
    assert_eq!(env.fail(&dir, &["start", &lower, "--force"]), "validation");
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--reason", "urgent"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--force", "--reason", "  "]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["start", &lower, "--force", "--reason", "line\ntwo"]),
        "validation"
    );
    assert_eq!(env.json(&dir, &["show", &lower])["task"]["status"], "todo");
    env.json(&dir, &["start", &lower, "--force", "--reason", "urgent"]);
    assert_eq!(env.json(&dir, &["show", &lower])["task"]["status"], "doing");
}

#[test]
fn halt_start_multihalt_error_names_urgent_blockers_in_order() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let p2 = id_of(env.json(&dir, &["add", "P2", "-p", "2", "--tag", "halt"]));
    let p0 = id_of(env.json(&dir, &["add", "P0", "-p", "0", "--tag", "halt"]));
    let target = id_of(env.json(&dir, &["add", "P3", "-p", "3"]));
    let error = error_of(&env, &dir, &["start", &target]);
    assert_eq!(error["error"]["kind"], "halted");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.find(&p0).unwrap() < detail.find(&p2).unwrap(),
        "{detail}"
    );
    assert!(detail.contains("--force --reason"));
}

#[test]
fn halt_override_invalid_transition_leaves_no_attempted_note() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&side, &["drop", &target, "no longer wanted"]);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert_eq!(
        env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]),
        "invalid_transition"
    );
    assert!(
        !env.json(&main, &["show", &halt])
            .to_string()
            .contains("halt override:")
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "dropped"
    );
}

#[test]
fn halt_override_failed_target_write_keeps_attempted_note_and_previous_claim() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf, std::fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    env.json(&side, &["start", &target]);
    let task_path = side.join(format!("tasks/{target}.md"));
    let raw = std::fs::read_to_string(&task_path)
        .unwrap()
        .replace("status: doing", "status: todo");
    std::fs::write(&task_path, raw).unwrap();
    let previous_claim = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    let tasks_dir = side.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let restore = Restore(tasks_dir, original);
    let kind = env.fail(&side, &["start", &target, "--force", "--reason", "urgent"]);
    drop(restore);
    assert_eq!(kind, "io");
    assert!(
        env.json(&main, &["show", &halt])
            .to_string()
            .contains(&format!("halt override: attempted {target}"))
    );
    assert_eq!(
        env.json(&side, &["show", &target])["task"]["status"],
        "todo"
    );
    assert_eq!(
        std::fs::read_to_string(env.claim_store("sci")).unwrap(),
        previous_claim
    );
}

#[test]
fn halt_start_unused_reason_warns_and_multiline_reason_is_refused_without_halt() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let target = id_of(env.json(&dir, &["add", "Ordinary"]));
    assert_eq!(
        env.fail(
            &dir,
            &["start", &target, "--force", "--reason", "two\nlines"]
        ),
        "validation"
    );
    let started = env.json(
        &dir,
        &["start", &target, "--force", "--reason", "prepared override"],
    );
    assert!(
        warnings_of(&started)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
}

#[test]
fn halt_start_force_resume_requires_reason_even_when_allowed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let target = id_of(env.json(&dir, &["add", "Ordinary"]));
    env.json(&dir, &["start", &target]);
    env.json(&dir, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert_eq!(env.fail(&dir, &["start", &target, "--force"]), "validation");
    let resumed = env.json(&dir, &["start", &target, "--force", "--reason", "resuming"]);
    assert!(
        warnings_of(&resumed)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
}

#[test]
fn halt_start_blocks_blocked_and_recurring_done_new_starts() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let blocked = id_of(env.json(&dir, &["add", "Blocked", "-p", "3"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    let recurring = id_of(env.json(&dir, &["add", "Sweep", "-p", "3", "--every", "1d"]));
    env.json(&dir, &["start", &recurring]);
    env.json(&dir, &["done", &recurring, "first pass"]);
    env.json(&dir, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert_eq!(env.fail(&dir, &["start", &blocked]), "halted");
    assert_eq!(env.fail(&dir, &["start", &recurring]), "halted");
    assert_eq!(
        env.json(&dir, &["show", &blocked])["task"]["status"],
        "blocked"
    );
    assert_eq!(
        env.json(&dir, &["show", &recurring])["task"]["status"],
        "done"
    );
}

#[test]
fn halt_start_work_for_less_urgent_halt_is_allowed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(
        &dir,
        &["add", "Urgent incident", "-p", "0", "--tag", "halt"],
    );
    let p2 = id_of(env.json(&dir, &["add", "Other incident", "-p", "2", "--tag", "halt"]));
    let remedy = id_of(env.json(&dir, &["add", "Remedy", "-p", "3", "--parent", &p2]));
    env.json(&dir, &["start", &remedy]);
    assert_eq!(
        env.json(&dir, &["show", &remedy])["task"]["status"],
        "doing"
    );
}

#[test]
fn halt_start_prepared_override_warns_when_authority_halt_lifts() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    assert_eq!(env.fail(&side, &["start", &target]), "halted");
    env.json(&main, &["done", &halt, "fixed"]);
    let started = env.json(
        &side,
        &["start", &target, "--force", "--reason", "prepared"],
    );
    assert!(
        warnings_of(&started)
            .iter()
            .any(|warning| warning.contains("--reason was unused"))
    );
    assert!(
        !env.json(&main, &["show", &halt])
            .to_string()
            .contains("halt override:")
    );
}

#[test]
fn halt_views_registered_incident_filters_entry_rows_and_names_missing_halt() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    for command in ["prime", "ready", "next"] {
        let output = env.json(&side, &[command]);
        assert_eq!(output["halts"][0]["id"], halt, "{command}: {output}");
        let warnings = warnings_of(&output);
        assert_eq!(
            warnings
                .iter()
                .filter(|warning| warning.contains("hidden by halt"))
                .count(),
            1,
            "{command}: {warnings:?}"
        );
        assert!(
            env.pretty(&side, &[command]).starts_with("halt:"),
            "{command}"
        );
        if command == "next" {
            assert!(output["next"].is_null(), "{output}");
        } else if command == "ready" {
            assert!(
                output["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["id"] != target)
            );
        } else {
            assert!(
                output["ready"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["id"] != target)
            );
        }
    }
    assert!(env.pretty(&side, &["next"]).contains("registered checkout"));
}

#[test]
fn halt_views_unhalted_json_omits_metadata() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Ordinary"]);
    for command in ["prime", "ready", "next"] {
        assert!(
            env.json(&dir, &[command]).get("halts").is_none(),
            "{command}"
        );
    }
}

#[test]
fn halt_views_all_projects_filter_only_the_halted_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let sci_task = id_of(env.json(&sci, &["add", "Sci work", "-p", "2"]));
    let fam_task = id_of(env.json(&fam, &["add", "Fam work", "-p", "2"]));
    let halt = id_of(env.json(&sci, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    env.json(&sci, &["shelve", &halt, "pending"]);
    for command in ["ready", "prime", "next"] {
        let output = env.json(&fam, &[command, "--all-projects"]);
        assert_eq!(output["halts"][0]["id"], halt, "{output}");
        let rows = if command == "prime" {
            &output["ready"]
        } else if command == "ready" {
            &output["tasks"]
        } else {
            &output["next"]
        };
        assert!(rows.to_string().contains(&fam_task), "{output}");
        assert!(!rows.to_string().contains(&sci_task), "{output}");
    }
}

#[test]
fn halt_views_unreadable_authority_warns_and_keeps_local_rows() {
    let mut env = TestEnv::new();
    let (main, side, target) = repo_with_worktree(&mut env);
    std::fs::remove_file(main.join("tasks/.config.toml")).unwrap();
    for command in ["ready", "prime", "next"] {
        let output = env.json(&side, &[command]);
        assert!(output.get("halts").is_none());
        assert!(output.to_string().contains(&target), "{output}");
        assert!(
            warnings_of(&output)
                .iter()
                .any(|warning| warning.contains("halt state unknown"))
        );
    }
}

#[test]
fn halt_views_claimed_shelved_and_deferred_halts_stay_visible() {
    let mut env = TestEnv::new();
    let (main, side, _target) = repo_with_worktree(&mut env);
    let claimed = id_of(env.json(&main, &["add", "Claimed", "-p", "0", "--tag", "halt"]));
    let shelved = id_of(env.json(&main, &["add", "Shelved", "-p", "1", "--tag", "halt"]));
    let deferred = id_of(env.json(
        &main,
        &[
            "add",
            "Deferred",
            "-p",
            "2",
            "--tag",
            "halt",
            "--defer",
            "2099-01-01",
        ],
    ));
    env.json(&main, &["start", &claimed]);
    env.json(&main, &["shelve", &shelved, "later"]);
    let ids: Vec<String> = env.json(&side, &["prime"])["halts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![claimed, shelved, deferred]);
}

#[test]
fn halt_views_parked_todo_is_hidden_but_parked_doing_resumes() {
    let mut env = TestEnv::new();
    let (main, side, todo) = repo_with_worktree(&mut env);
    env.json(&side, &["park", &todo, "continue"]);
    env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]);
    assert!(env.json(&side, &["next"])["next"].is_null());
    let doing = id_of(env.json(&side, &["add", "Already doing", "-p", "2"]));
    env.json(&side, &["start", &doing, "--force", "--reason", "initial"]);
    env.json(&side, &["park", &doing, "resume"]);
    assert_eq!(env.json(&side, &["next"])["next"]["task"]["id"], doing);
}

#[test]
fn halt_views_ready_limit_applies_after_halt_filter() {
    let mut env = TestEnv::new();
    let (main, side, hidden) = repo_with_worktree(&mut env);
    let halt = id_of(env.json(&main, &["add", "Incident", "-p", "0", "--tag", "halt"]));
    let local_halt = side.join(format!("tasks/{halt}.md"));
    std::fs::copy(main.join(format!("tasks/{halt}.md")), &local_halt).unwrap();
    let remedy = id_of(env.json(&side, &["add", "Remedy", "-p", "3", "--parent", &halt]));
    std::fs::remove_file(local_halt).unwrap();
    let output = env.json(&side, &["ready", "-n", "1"]);
    assert_eq!(output["tasks"][0]["id"], remedy);
    assert_eq!(output["tasks"].as_array().unwrap().len(), 1);
    assert!(!output["tasks"].to_string().contains(&hidden));
    assert!(
        warnings_of(&output)
            .iter()
            .any(|warning| warning.starts_with("1 ready task(s) hidden by halt"))
    );
}

#[test]
fn halt_views_all_projects_unreachable_project_reports_unknown_state() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let output = env.json(&sci, &["ready", "--all-projects"]);
    assert!(output["tasks"].to_string().contains(&local));
    assert!(
        warnings_of(&output)
            .iter()
            .any(|warning| warning.contains("fam: halt state unknown"))
    );
}

#[test]
fn a_write_from_a_copy_behind_another_checkout_refuses_and_prints_the_retry() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    // Explicit stamps: `updated` has second precision, and a real-clock race would make
    // the test flaky rather than wrong.
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let file = format!("tasks/{id}.md");
    let (main_before, side_before) = (env.read(&main, &file), env.read(&side, &file));

    for args in [
        vec!["note", id.as_str(), "it's naïve"],
        vec!["edit", id.as_str(), "-p", "1"],
        vec!["start", id.as_str()],
        vec!["done", id.as_str(), "landed"],
    ] {
        let detail = stale_detail(&env, &main, &args);
        let head = format!(
            "tasks/{id}.md in {} is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here); nothing was written. Run it there: ",
            side.display()
        );
        assert!(detail.starts_with(&head), "{detail}");
        let mut expected = vec!["-C".to_string(), side.display().to_string()];
        expected.extend(args.iter().map(|arg| arg.to_string()));
        assert_eq!(retry_words(&detail), expected);
    }
    assert_eq!(env.read(&main, &file), main_before);
    assert_eq!(env.read(&side, &file), side_before);
    let store = env.claim_store("sci");
    assert!(!store.exists() || !std::fs::read_to_string(&store).unwrap().contains(&id));

    // An invocation that already named a checkout gets that `-C` replaced, not doubled.
    let detail = stale_detail(
        &env,
        &side,
        &["-C", main.to_str().unwrap(), "note", &id, "x"],
    );
    assert_eq!(
        retry_words(&detail),
        ["-C", side.to_str().unwrap(), "note", &id, "x"]
    );

    // The printed retry, run verbatim from where it was refused, lands in the newer copy.
    let detail = stale_detail(&env, &main, &["note", &id, "it's naïve"]);
    env.cmd(&main).args(retry_words(&detail)).assert().success();
    assert!(env.read(&side, &file).contains("it's naïve"));
}

#[test]
fn a_linked_worktree_behind_the_main_checkout_leads_with_the_merge() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    let detail = stale_detail(&env, &side, &["note", &id, "x"]);
    let remedy = format!(
        "nothing was written. Commit tasks/{id}.md in {main} if it has changes, merge it into \
         this branch, then rerun here; the merge may conflict where both copies changed. Or, \
         to write in the main checkout instead: tasks -C {main} note {id} x",
        main = main.display()
    );
    assert!(detail.ends_with(&remedy), "{detail}");
}

#[test]
fn another_sessions_work_in_the_newer_checkout_withholds_the_retry() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let other = id_of(env.json(&side, &["add", "Other work", "-p", "2"]));
    as_agent(&env, &side, "agent-b")
        .args(["start", &other])
        .assert()
        .success();
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");

    // Someone else's claim, on a different task, names the newer checkout.
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert!(
        detail.contains(&other) && detail.contains("agent-b"),
        "{detail}"
    );
    assert!(detail.contains("Wait for that branch to merge"), "{detail}");
    assert!(!detail.contains("tasks -C"), "{detail}");

    // The claim's own holder, whose shell reset to main, keeps the retry.
    let out = as_agent(&env, &main, "agent-b")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("Run it there: tasks -C"),
        "{error}"
    );

    // A park by another session withholds it too.
    as_agent(&env, &side, "agent-b")
        .args(["park", &other, "later"])
        .assert()
        .success();
    let out = as_agent(&env, &main, "agent-c")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&other) && !detail.contains("tasks -C"),
        "{detail}"
    );
}

#[test]
fn a_stale_refusal_reports_an_identity_resolution_failure() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let state = relay_on(&env);
    // Under a real harness ancestor, relay identity cannot resolve without its registry.
    let script = format!(
        "RELAY_STATE_DIR={}\nexport RELAY_STATE_DIR\n\"$TASKS_BIN\" note {id} x\n",
        state.display()
    );
    let out = common::harness_shim(&main, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("whether another session works there is unknown:")
            && detail.contains("TASKS_SESSION"),
        "{detail}"
    );
    assert!(detail.contains("Run it there: tasks -C"), "{detail}");
    assert_eq!(
        retry_words(detail),
        ["-C", side.to_str().unwrap(), "note", &id, "x"]
    );
}

#[test]
fn a_stale_refusal_reports_a_claim_store_failure_and_keeps_the_retry_valid() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let store = env.claim_store("sci");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&store, "not a claim store").unwrap();
    let detail = stale_detail(&env, &main, &["note", &id, "it's naïve"]);
    assert!(
        detail.contains("whether another session works there is unknown:")
            && detail.contains("claim store"),
        "{detail}"
    );
    assert_eq!(
        retry_words(&detail),
        ["-C", side.to_str().unwrap(), "note", &id, "it's naïve"]
    );
}

#[test]
fn equal_stamps_with_different_bytes_refuse_with_the_merge_only() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    let sibling = side.join(format!("tasks/{id}.md"));
    let forked = std::fs::read_to_string(&sibling)
        .unwrap()
        .replace("title: T\n", "title: Forked\n");
    std::fs::write(&sibling, forked).unwrap();
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert_eq!(
        detail,
        format!(
            "tasks/{id}.md in {} has the same stamp as this copy (2026-09-05T09:00:00Z) but \
             different content, so both were written in the same second; nothing was \
             written. Merge that copy of tasks/{id}.md into this checkout, then rerun here",
            side.display()
        )
    );
}

#[test]
fn a_write_in_the_same_second_as_the_loaded_stamp_still_moves_the_stamp_forward() {
    // tasks-cff04e: start in main, branch, and start again in the worktree within one
    // second. A stamp ahead of the clock stands in for "the same second": the worktree's
    // write must still land strictly after the copy it loaded, or both copies share a stamp
    // with different bytes and every later write refuses as a same-second fork.
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2099-01-01T00:00:00Z");
    }
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&side, &["show", &id]);
    assert_eq!(v["task"]["updated"], "2099-01-01T00:00:01Z", "{v}");
    as_agent(&env, &side, "agent-a")
        .args(["note", &id, "here"])
        .assert()
        .success();
    let detail = stale_detail(&env, &main, &["note", &id, "from main"]);
    assert!(
        detail.starts_with(&format!(
            "tasks/{id}.md in {} is newer than this copy",
            side.display()
        )),
        "{detail}"
    );
}

#[test]
fn the_first_write_in_a_worktree_behind_main_leads_with_the_merge() {
    // tasks-142d2f without the protocol step: a note in main after branching, then a write
    // in the worktree by the claim's own holder.
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &main, "agent-a")
        .args(["note", &id, "from main"])
        .assert()
        .success();
    let out = as_agent(&env, &side, "agent-a")
        .args(["note", &id, "here"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!(
            "nothing was written. Commit tasks/{id}.md in {}",
            main.display()
        )),
        "{detail}"
    );
}

#[test]
fn a_start_left_uncommitted_before_branching_leads_with_the_merge() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    as_agent(&env, &main, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let side = main.join("wt");
    git(
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
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-01T00:00:00Z");
    let out = as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!(
            "nothing was written. Commit tasks/{id}.md in {}",
            main.display()
        )),
        "{detail}"
    );
    assert!(!detail.contains("Run it there"), "{detail}");
}

#[test]
fn a_refusal_names_the_newest_sibling_and_lists_the_other_newer_ones() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let third = main.join("wt2");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "third",
            third.to_str().unwrap(),
        ],
    );
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&third, &id, "2026-09-01T00:00:00Z", "2026-09-06T10:00:00Z");
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert!(
        detail.starts_with(&format!("tasks/{id}.md in {} is newer", side.display())),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("(also newer in: {})", third.display())),
        "{detail}"
    );
}

#[test]
fn a_worktree_deleted_from_disk_does_not_block_a_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2030-01-01T00:00:00Z");
    // Gone from disk but still listed by git: nothing there to be newer.
    std::fs::remove_dir_all(&side).unwrap();
    env.json(&main, &["note", &id, "still lands"]);
    assert!(
        env.read(&main, &format!("tasks/{id}.md"))
            .contains("still lands")
    );
}

#[test]
fn equal_stamps_with_equal_bytes_do_not_refuse() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    let v = env.json(&main, &["note", &id, "same record"]);
    assert!(!warnings_of(&v).iter().any(|w| w.contains("newer")), "{v}");
}

#[test]
fn a_checkout_that_is_merely_behind_is_not_worth_a_warning() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");

    // Writing in the *newer* checkout: the other copy holds nothing this one lacks, which
    // is the normal state of any long-lived worktree and must stay silent.
    let v = env.json(&main, &["note", &id, "onward"]);
    let warnings = warnings_of(&v);
    assert!(
        !warnings.iter().any(|w| w.contains("is newer")),
        "{warnings:?}"
    );
}

#[test]
fn a_project_below_the_repository_root_finds_its_sibling_copies() {
    let env = TestEnv::new();
    let held = tempfile::tempdir().unwrap();
    let repo = held.path().canonicalize().unwrap();
    let sub = repo.join("sub");
    std::fs::create_dir(&sub).unwrap();
    env.json(&sub, &["init", "--prefix", "sci"]);

    git(&repo, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&sub, &["add", "T", "-p", "2"]));
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "seed"]);
    let side = repo.join("wt");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );

    stamp(&sub, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(
        &side.join("sub"),
        &id,
        "2026-09-01T00:00:00Z",
        "2026-09-05T09:00:00Z",
    );

    // The sibling's project root is the worktree root plus the project's path below the
    // repository top level, not the worktree root itself.
    let detail = stale_detail(&env, &side.join("sub"), &["note", &id, "in the worktree"]);
    let expected = format!("tasks/{id}.md in {} is newer", sub.display());
    assert!(detail.starts_with(&expected), "{detail}");
}

#[test]
fn a_sibling_copy_that_cannot_be_read_is_a_warning_and_a_missing_one_is_silent() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let sibling = side.join(format!("tasks/{id}.md"));

    std::fs::write(&sibling, "not a task file").unwrap();
    let v = env.json(&main, &["note", &id, "first"]);
    let warnings = warnings_of(&v);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains(side.to_str().unwrap()) && w.contains("could not be read")),
        "a copy we cannot compare is not a copy we know to agree: {warnings:?}"
    );

    // A worktree branched before the task existed simply has nothing to say.
    std::fs::remove_file(&sibling).unwrap();
    let v = env.json(&main, &["note", &id, "second"]);
    let warnings = warnings_of(&v);
    assert!(
        !warnings
            .iter()
            .any(|w| w.contains("could not be read") || w.contains("is newer")),
        "{warnings:?}"
    );
}

#[test]
fn the_editor_does_not_open_on_a_copy_that_is_behind() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let opened = main.join("editor-opened");
    let editor = editor_script(&main, &format!("touch '{}'", opened.display()));
    let out = env
        .cmd(&main)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    assert!(!opened.exists(), "the editor opened on a stale copy");
}

#[test]
fn a_failure_to_inspect_other_checkouts_is_a_warning_not_a_refusal() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = TestEnv::new();
    let (main, _side, id) = repo_with_worktree(&mut env);

    let bin = tempfile::tempdir().unwrap();
    let fake_git = bin.path().join("git");
    std::fs::write(
        &fake_git,
        "#!/bin/sh\nif [ \"$1 $2\" = \"worktree list\" ]; then echo broken >&2; exit 42; fi\nPATH=${PATH#*:} exec git \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );

    let out = env
        .cmd(&main)
        .env("PATH", path)
        .args(["note", &id, "still lands"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let warnings = warnings_of(&v);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains(&id) && w.contains("other checkouts") && w.contains("broken")),
        "{warnings:?}"
    );
    assert_eq!(
        env.json(&main, &["show", &id])["task"]["notes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Pin a task's clocks so date order is deterministic (the binary stamps real time).
fn stamp(dir: &std::path::Path, id: &str, created: &str, updated: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let text: String = text
        .lines()
        .map(|line| {
            if line.starts_with("created: ") {
                format!("created: {created}\n")
            } else if line.starts_with("updated: ") {
                format!("updated: {updated}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    std::fs::write(path, text).unwrap();
}

#[test]
fn list_sorts_by_priority_updated_or_created_and_prints_the_date() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let a = id_of(env.json(&dir, &["add", "A", "-p", "1"]));
    let b = id_of(env.json(&dir, &["add", "B", "-p", "2"]));
    let c = id_of(env.json(&dir, &["add", "C", "-p", "3"]));
    let (a, b, c) = (a.as_str(), b.as_str(), c.as_str());
    // priority order is A B C; updated desc is C A B; created desc is B C A
    stamp(&dir, a, "2026-01-01T00:00:00Z", "2026-02-02T00:00:00Z");
    stamp(&dir, b, "2026-03-03T00:00:00Z", "2026-01-01T00:00:00Z");
    stamp(&dir, c, "2026-02-02T00:00:00Z", "2026-03-03T00:00:00Z");
    let ids = |v: serde_json::Value| -> Vec<String> {
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };

    assert_eq!(ids(env.json(&dir, &["list"])), [a, b, c]);
    assert_eq!(
        ids(env.json(&dir, &["list", "--sort", "priority"])),
        [a, b, c]
    );
    assert_eq!(
        ids(env.json(&dir, &["list", "--sort", "updated"])),
        [c, a, b]
    );
    let v = env.json(&dir, &["list", "--sort", "created"]);
    assert_eq!(v["tasks"][0]["created"], "2026-03-03T00:00:00Z");
    assert_eq!(ids(v), [b, c, a]);
    assert_eq!(ids(env.json(&dir, &["list", "--reverse"])), [c, b, a]);
    assert_eq!(
        ids(env.json(&dir, &["list", "--sort", "created", "--reverse"])),
        [a, c, b]
    );
    let err = env.usage(&dir, &["list", "--sort", "weird"]);
    assert!(err.contains("--sort") && err.contains("weird"), "{err}");

    // pretty rows carry the day of last activity, or of creation when sorting by it
    let pretty = |args: &[&str]| -> String {
        let out = env.cmd(&dir).arg("--pretty").args(args).output().unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let text = pretty(&["list"]);
    assert!(text.contains("todo    2026-02-02  A\n"), "{text}");
    let text = pretty(&["list", "--sort", "created"]);
    assert!(text.contains("todo    2026-01-01  A\n"), "{text}");
    let text = pretty(&["ready"]);
    assert!(text.contains("todo    2026-02-02  A\n"), "{text}");
}

#[test]
fn write_commands_follow_the_id_prefix_across_projects() {
    let mut env = TestEnv::new();
    let ops = env.init("ops");
    let fam = env.init("fam");
    let groundwork = id_of(env.json(&ops, &["add", "Groundwork"]));
    let piece = id_of(env.json(&ops, &["add", "Fam piece", "--project", "fam"]));

    // every id-taking write runs from the hub, against the project the prefix names
    env.json(&ops, &["note", &piece, "from the hub"]);
    env.json(&ops, &["dep", &piece, "--on", &groundwork]);
    env.json(&ops, &["edit", &piece, "--tag", "audit"]);
    env.json(&ops, &["start", &piece]);
    env.json(&ops, &["block", &piece, "waiting"]);
    env.json(&ops, &["unblock", &piece]);

    let shown = env.json(&fam, &["show", &piece]);
    assert_eq!(shown["task"]["tags"][0], "audit");
    assert_eq!(shown["task"]["depends"][0], groundwork);
    assert_eq!(shown["task"]["notes"][0]["text"], "from the hub");
    assert_eq!(shown["task"]["status"], "todo");

    // the claim landed in the target project's store, not the caller's
    env.json(&ops, &["start", &piece]);
    let claims = std::fs::read_to_string(env.claim_store("fam")).unwrap();
    assert!(claims.contains(&piece), "{claims}");
    assert!(!env.claim_store("ops").exists());
    env.json(&ops, &["done", &groundwork]);
    env.json(&ops, &["done", &piece, "landed"]);
    assert_eq!(env.json(&fam, &["show", &piece])["task"]["status"], "done");

    // a matching prefix uses the local checkout, not the registry root
    let displaced = env.init_forced("ops");
    let local = id_of(env.json(&displaced, &["add", "Local"]));
    env.json(&displaced, &["note", &local, "stays put"]);
    assert!(displaced.join(format!("tasks/{local}.md")).is_file());
    assert!(!ops.join(format!("tasks/{local}.md")).exists());

    // an id whose prefix no project claims is unresolvable, not task_not_found
    assert_eq!(
        env.fail(&ops, &["note", "zzz-000001", "x"]),
        "unresolvable_id"
    );
    // a foreign id is still resolved through the registry, so a missing file is that
    assert_eq!(
        env.fail(&ops, &["note", "fam-000001", "x"]),
        "task_not_found"
    );
    // with no local project, the prefix still names the target
    let nowhere = tempfile::tempdir().unwrap();
    env.json(nowhere.path(), &["note", &piece, "from outside"]);
    assert_eq!(
        env.json(&fam, &["show", &piece])["task"]["notes"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["text"],
        "from outside"
    );
}

#[test]
fn id_commands_route_from_outside_every_project() {
    let mut env = TestEnv::new();
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();
    let id = id_of(env.json(&fam, &["add", "Available everywhere"]));

    for command in ["show", "note", "start"] {
        assert_eq!(
            env.complete(nowhere.path(), "bash", 2, &["tasks", command, "fam-"]),
            [id.as_str()]
        );
    }
    assert_eq!(env.json(nowhere.path(), &["show", &id])["task"]["id"], id);
    assert_eq!(
        env.json(nowhere.path(), &["tree", &id])["nodes"][0]["id"],
        id
    );
    env.json(nowhere.path(), &["start", &id]);
    let shown = env.json(&fam, &["show", &id]);
    assert_eq!(shown["task"]["status"], "doing");
    assert!(shown["claim"].is_object(), "{shown}");
    assert!(
        env.read(&fam, &format!("tasks/{id}.md"))
            .contains("status: doing")
    );
    assert!(!nowhere.path().join("tasks").exists());

    alias_registry(&env, "old", "fam");
    let retired = id.replacen("fam-", "old-", 1);
    env.json(nowhere.path(), &["note", &retired, "via retired prefix"]);
    assert_eq!(
        env.json(nowhere.path(), &["show", &retired])["task"]["id"],
        id
    );

    for command in ["show", "tree", "start"] {
        assert_eq!(
            env.fail(nowhere.path(), &[command, "zzz-000001"]),
            "unresolvable_id"
        );
        assert_eq!(
            env.fail(nowhere.path(), &[command, "fam-000001"]),
            "task_not_found"
        );
    }
    assert_eq!(env.fail(nowhere.path(), &["tree"]), "no_project");
    assert_eq!(
        env.fail(
            nowhere.path(),
            &[
                "feedback",
                "--project",
                "tasks",
                "Example",
                "--category",
                "gap"
            ]
        ),
        "no_project"
    );

    // A broken local config is an error, even when the id names a healthy registry root.
    write_doc(nowhere.path(), "tasks/.config.toml", "not valid toml");
    for command in ["show", "tree", "start"] {
        assert_eq!(env.fail(nowhere.path(), &[command, &id]), "config");
    }
}

#[test]
fn edit_tags_append_and_remove_instead_of_replacing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let tags = |env: &TestEnv, id: &str| env.json(&sci, &["show", id])["task"]["tags"].clone();
    let id = id_of(env.json(
        &sci,
        &["add", "Triage me", "--tag", "feedback", "--tag", "gap"],
    ));
    assert_eq!(tags(&env, &id), serde_json::json!(["feedback", "gap"]));

    // the reported bug: adding one tag kept the provenance tags
    env.json(&sci, &["edit", &id, "--tag", "cli"]);
    assert_eq!(
        tags(&env, &id),
        serde_json::json!(["feedback", "gap", "cli"])
    );
    // appending is idempotent
    env.json(&sci, &["edit", &id, "--tag", "cli", "--tag", "gap"]);
    assert_eq!(
        tags(&env, &id),
        serde_json::json!(["feedback", "gap", "cli"])
    );

    // removal is explicit, and naming a tag the task lacks is an error
    env.json(&sci, &["edit", &id, "--rm-tag", "gap"]);
    assert_eq!(tags(&env, &id), serde_json::json!(["feedback", "cli"]));
    assert_eq!(
        env.fail(&sci, &["edit", &id, "--rm-tag", "gap"]),
        "validation"
    );

    // --no-tags clears, and pairs with --tag to reproduce a wholesale replace
    env.json(&sci, &["edit", &id, "--no-tags", "--tag", "cli"]);
    assert_eq!(tags(&env, &id), serde_json::json!(["cli"]));
    env.json(&sci, &["edit", &id, "--no-tags"]);
    assert!(env.json(&sci, &["show", &id])["task"].get("tags").is_none());

    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--no-tags", "--rm-tag", "cli"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "--no-tags and --rm-tag conflict"
    );
}

#[test]
fn source_is_set_by_add_replaced_and_cleared_by_edit() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let id = id_of(env.json(
        &dir,
        &["add", "From mail", "--source", "mail:<42@example.org>"],
    ));
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        "mail:<42@example.org>"
    );
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    assert!(
        raw.contains("tags: []\nsource: \"mail:<42@example.org>\"\n"),
        "{raw}"
    );
    let pretty = env.pretty(&dir, &["show", &id]);
    assert!(
        pretty.contains("source: \"mail:<42@example.org>\""),
        "{pretty}"
    );

    env.json(
        &dir,
        &["edit", &id, "--source", "https://example.org/issues/7"],
    );
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        "https://example.org/issues/7"
    );

    env.json(&dir, &["edit", &id, "--no-source"]);
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["source"],
        serde_json::Value::Null
    );
    assert!(
        !env.read(&dir, &format!("tasks/{id}.md"))
            .contains("source:")
    );

    // Unset source is omitted.
    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    let shown = env.json(&dir, &["show", &plain]);
    assert!(shown["task"].get("source").is_none(), "{shown}");
    assert_eq!(shown["task"]["source"], serde_json::Value::Null);

    // empty and multi-line are validation errors on both write paths
    assert_eq!(
        env.fail(&dir, &["add", "Bad", "--source", ""]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["add", "Bad", "--source", "a\nb"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["edit", &plain, "--source", ""]),
        "validation"
    );
    // clap rejects the conflicting pair before any command runs
    env.cmd(&dir)
        .args(["edit", &plain, "--source", "x", "--no-source"])
        .assert()
        .code(2);

    // a repository with sourced tasks passes check
    let check = env.check(&dir);
    assert_eq!(check["errors"], serde_json::json!([]), "{check}");

    // the flag completes; nothing in complete.rs mentions it
    let flags = env.complete(&dir, "bash", 3, &["tasks", "add", "T", "--sou"]);
    assert_eq!(flags, ["--source"]);
    let flags = env.complete(&dir, "bash", 3, &["tasks", "edit", &plain, "--no-so"]);
    assert_eq!(flags, ["--no-source"]);

    // summary rows carry it too, so `list` can answer "what came from this reference"
    let sourced = id_of(env.json(&dir, &["add", "Sourced", "--source", "note:abc"]));
    let rows = env.json(&dir, &["list"]);
    let row = rows["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == sourced)
        .unwrap();
    assert_eq!(row["source"], "note:abc");
    let plain_row = rows["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == plain)
        .unwrap();
    assert!(plain_row.get("source").is_none());
    let ready = env.json(&dir, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| if r["id"] == sourced {
                r["source"] == "note:abc"
            } else {
                r.get("source").is_none()
            }),
        "{ready}"
    );
}

#[test]
fn a_sourced_add_reuses_the_record_that_origin_and_title_already_made() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let first = env.json(&dir, &["add", "Ship it", "--source", "note:abc", "-p", "1"]);
    assert_eq!(first["action"], "created", "{first}");
    let id = id_of(first);

    // same origin, same title: the existing id comes back and nothing is written
    let again = env.json(&dir, &["add", "Ship it", "--source", "note:abc", "-p", "4"]);
    assert_eq!(again["action"], "reused", "{again}");
    assert_eq!(again["id"], id);
    assert!(
        again["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("already carries this source")),
        "{again}"
    );
    // reuse is not a merge: the second call's fields are ignored, not applied
    assert_eq!(env.json(&dir, &["show", &id])["task"]["priority"], 1);
    assert_eq!(
        env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
        1
    );

    // a closed task still counts; refiling what is already done is the duplicate
    env.json(&dir, &["done", &id, "shipped"]);
    let after_done = env.json(&dir, &["add", "Ship it", "--source", "note:abc"]);
    assert_eq!(after_done["action"], "reused", "{after_done}");
    assert_eq!(after_done["id"], id);

    // either half differing files afresh, and an unsourced add never dedupes
    let other_source = env.json(&dir, &["add", "Ship it", "--source", "note:xyz"]);
    assert_eq!(other_source["action"], "created", "{other_source}");
    let other_title = env.json(&dir, &["add", "Ship it later", "--source", "note:abc"]);
    assert_eq!(other_title["action"], "created", "{other_title}");
    let plain = id_of(env.json(&dir, &["add", "Twice"]));
    let plain_again = env.json(&dir, &["add", "Twice"]);
    assert_eq!(plain_again["action"], "created", "{plain_again}");
    assert_ne!(plain_again["id"], plain);

    // the check follows `--project` to the project actually written
    let fam = env.init("fam");
    let piece = env.json(
        &dir,
        &["add", "Piece", "--project", "fam", "--source", "note:abc"],
    );
    assert_eq!(piece["action"], "created", "{piece}");
    let piece_again = env.json(
        &dir,
        &["add", "Piece", "--project", "fam", "--source", "note:abc"],
    );
    assert_eq!(piece_again["action"], "reused", "{piece_again}");
    assert_eq!(piece_again["id"], piece["id"]);
    assert_eq!(
        env.json(&fam, &["list"])["tasks"].as_array().unwrap().len(),
        1
    );

    // pretty: the id on stdout as ever, the reuse on stderr, exit 0 either way
    let out = env
        .cmd(&dir)
        .args(["--pretty", "add", "Ship it", "--source", "note:abc"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), id);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(stderr.contains("reused it, wrote nothing"), "{stderr}");
}

#[test]
fn list_filters_by_exact_source() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let fam = env.init("fam");

    let one = id_of(env.json(&dir, &["add", "One", "--source", "note:abc", "--tag", "x"]));
    let two = id_of(env.json(&dir, &["add", "Two", "--source", "note:abc"]));
    env.json(&dir, &["add", "Three", "--source", "note:xyz"]);
    env.json(&dir, &["add", "Plain"]);
    let far = id_of(env.json(&fam, &["add", "Far", "--source", "note:abc"]));

    let ids = |v: serde_json::Value| -> Vec<String> {
        let mut ids: Vec<String> = v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        ids
    };
    let mut want = vec![one.clone(), two.clone()];
    want.sort();
    assert_eq!(ids(env.json(&dir, &["list", "--source", "note:abc"])), want);

    // exact: never a prefix, a substring, or a case-folded match
    for near in ["note:", "abc", "note:ABC", "note:abcd"] {
        let v = env.json(&dir, &["list", "--source", near]);
        assert!(v["tasks"].as_array().unwrap().is_empty(), "{near}: {v}");
    }

    // composes with the other filters, with the statuses, and with the read scopes
    assert_eq!(
        ids(env.json(&dir, &["list", "--source", "note:abc", "--tag", "x"])),
        vec![one.clone()]
    );
    env.json(&dir, &["done", &one, "landed"]);
    assert_eq!(
        ids(env.json(&dir, &["list", "--source", "note:abc"])),
        vec![two.clone()]
    );
    assert_eq!(
        ids(env.json(&dir, &["list", "--source", "note:abc", "--status", "done"])),
        vec![one.clone()]
    );
    assert_eq!(
        ids(env.json(&fam, &["list", "--project", "sci", "--source", "note:abc"])),
        vec![two.clone()]
    );
    let mut across = vec![two.clone(), far.clone()];
    across.sort();
    assert_eq!(
        ids(env.json(&dir, &["list", "--all-projects", "--source", "note:abc"])),
        across
    );

    // the flag completes; nothing in complete.rs mentions it
    let flags = env.complete(&dir, "bash", 2, &["tasks", "list", "--sour"]);
    assert_eq!(flags, ["--source"]);
}

#[test]
fn completion_offers_the_fixed_value_sets_and_registry_prefixes() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");

    // every status but shelved on edit (`shelve` is the way in), only the two `add` accepts
    assert_eq!(
        env.complete(
            &sci,
            "bash",
            4,
            &["tasks", "edit", "sci-000001", "--status", ""]
        ),
        ["idea", "todo", "doing", "blocked", "done", "dropped"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--status", ""]),
        [
            "idea", "todo", "doing", "blocked", "shelved", "done", "dropped"
        ]
    );
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--status", ""]),
        ["idea", "todo"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "ready", "--size", ""]),
        ["xs", "s", "m", "l", "xl"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--sort", ""]),
        ["priority", "updated", "created"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--color", ""]),
        ["auto", "always", "never"]
    );
    assert_eq!(
        env.complete(
            &sci,
            "bash",
            6,
            &[
                "tasks",
                "feedback",
                "--project",
                "tasks",
                "S",
                "--category",
                ""
            ]
        ),
        ["friction", "gap", "idea", "positive"]
    );

    // registry prefixes, in registry order
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--project", ""]),
        ["fam", "sci"]
    );
    // `unregister`'s prefix is a bare positional with nothing preceding it, so
    // clap_complete also offers the still-unused global flags alongside it;
    // `complete_values` drops those to check the prefix candidates specifically.
    assert_eq!(
        env.complete_values(&sci, "bash", 2, &["tasks", "unregister", ""]),
        ["fam", "sci"]
    );

    // subcommands come from the derive, and a prefix narrows them
    let subs = env.complete(&sci, "bash", 1, &["tasks", "re"]);
    assert!(subs.contains(&"ready".to_string()), "{subs:?}");
}

#[test]
fn completion_offers_task_ids_open_first_with_descriptions() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let open = id_of(env.json(&sci, &["add", "Still open"]));
    let closed = id_of(env.json(&sci, &["add", "Finished"]));
    env.json(&sci, &["done", &closed, "landed"]);

    // open before closed, whatever the ids sort to
    let ids = env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]);
    assert_eq!(
        ids,
        [open.clone(), closed.clone()],
        "open task must come first"
    );

    // the prefix filters, tested at the two ends that do not depend on the random hex:
    // a prefix both ids share narrows nothing, and one only the open id has narrows to it.
    // `&open[..5]` used to stand in for the second case, but that is `sci-` plus a single
    // hex digit, which the closed id also matches one run in sixteen.
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "show", "sci-"]),
        [open.as_str(), closed.as_str()]
    );
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "show", &open]),
        [open.as_str()]
    );

    // zsh carries `value:description`; bash emits bare values
    let described = env.complete(&sci, "zsh", 2, &["tasks", "show", ""]);
    assert_eq!(described[0], format!("{open}:todo  Still open"));
    assert_eq!(described[1], format!("{closed}:done  Finished"));

    // every id-taking write uses the same source
    for command in [
        "edit", "note", "start", "done", "drop", "block", "unblock", "dep", "root",
    ] {
        let ids = env.complete_values(&sci, "bash", 2, &["tasks", command, ""]);
        assert!(ids.contains(&open), "{command}: {ids:?}");
    }
}

#[test]
fn completion_follows_a_typed_prefix_and_prefers_the_local_checkout() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    let foreign = id_of(env.json(&fam, &["add", "Foreign"]));

    // a foreign prefix reaches that project
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "note", "fam-"]),
        [foreign.as_str()]
    );
    // the local one stays local
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "note", "sci-"]),
        [local.as_str()]
    );

    // a second root under the same prefix holds different tasks; standing in it, the
    // local checkout wins over the registered root
    let displaced = env.init_forced("sci");
    let displaced_id = id_of(env.json(&displaced, &["add", "Displaced"]));
    assert_eq!(
        env.complete(&displaced, "bash", 2, &["tasks", "note", "sci-"]),
        [displaced_id.as_str()]
    );

    // -C selects the project, overriding the process's directory
    let dir = displaced.to_str().unwrap();
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "-C", dir, "note", "sci-"]),
        [displaced_id.as_str()]
    );
    assert_eq!(
        env.complete(
            &sci,
            "bash",
            3,
            &["tasks", &format!("-C{dir}"), "note", "sci-"]
        ),
        [displaced_id]
    );

    // `root` resolves through the registry and needs no local project
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.complete(nowhere.path(), "bash", 2, &["tasks", "root", "fam-"]),
        [foreign]
    );
}

#[test]
fn completion_is_silent_when_anything_is_wrong() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fine = id_of(env.json(&sci, &["add", "Fine"]));
    let nowhere = tempfile::tempdir().unwrap();

    // outside every project
    assert!(
        env.complete_values(nowhere.path(), "bash", 2, &["tasks", "show", ""])
            .is_empty()
    );
    // an unregistered prefix
    assert!(
        env.complete(&sci, "bash", 2, &["tasks", "show", "zzz-"])
            .is_empty()
    );
    // a malformed task file does not wipe the rest of the menu: the good id is still
    // offered, the bad file is silently skipped
    std::fs::write(sci.join("tasks/sci-bad001.md"), "not a task").unwrap();
    assert_eq!(
        env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]),
        [fine.as_str()],
        "a malformed sibling file must not hide a valid id"
    );
    // an unreadable tasks directory (registered before the registry below is corrupted,
    // since `init` itself needs a readable registry)
    let locked = env.init("lck");
    let mut perms = std::fs::metadata(locked.join("tasks"))
        .unwrap()
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o000);
    std::fs::set_permissions(locked.join("tasks"), perms.clone()).unwrap();
    let out = env.complete_values(&locked, "bash", 2, &["tasks", "show", ""]);
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(locked.join("tasks"), perms).unwrap();
    assert!(out.is_empty(), "{out:?}");
    // a malformed registry does not disable the local project: it is opened by walking
    // up from the effective directory, not through the registry, so its ids are still
    // offered even though the registry itself cannot be read
    std::fs::write(
        env.home.path().join(".config/tasks/projects.toml"),
        "not toml = [",
    )
    .unwrap();
    assert_eq!(
        env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]),
        [fine.as_str()],
        "a malformed registry must not disable the local project"
    );
}

#[test]
fn completion_stub_is_emitted_and_the_hook_is_otherwise_inert() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // no words after `--`: the registration stub, naming the binary
    let out = env
        .cmd(&sci)
        .env("TASKS_COMPLETE", "bash")
        .arg("--")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stub = String::from_utf8_lossy(&out.stdout);
    assert!(stub.contains("complete "), "{stub}");
    assert!(stub.contains("TASKS_COMPLETE"), "{stub}");

    // an unsupported shell name is an error, not a silent stub
    let bad = env
        .cmd(&sci)
        .env("TASKS_COMPLETE", "notashell")
        .arg("--")
        .output()
        .unwrap();
    assert!(!bad.status.success());

    // with the variable unset, an ordinary argv runs the ordinary command
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
}

#[test]
fn completion_scopes_ids_to_what_each_argument_accepts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    let foreign = id_of(env.json(&fam, &["add", "Foreign"]));

    // Scoped: tree and list --parent are local, until --all-projects widens list
    assert!(
        env.complete_values(&sci, "bash", 2, &["tasks", "tree", "fam-"])
            .is_empty()
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--parent", ""]),
        [local.as_str()]
    );
    let mut wide = env.complete(
        &sci,
        "bash",
        4,
        &["tasks", "list", "--all-projects", "--parent", ""],
    );
    wide.sort();
    let mut both = [foreign.clone(), local.clone()];
    both.sort();
    assert_eq!(
        wide, both,
        "--all-projects widens the scope to the registry"
    );

    // a second task in each project — enough to prove a scope excludes the subject's
    // own id while still offering everything else
    let sci_peer = id_of(env.json(&sci, &["add", "Sci peer"]));
    let fam_peer = id_of(env.json(&fam, &["add", "Fam peer"]));
    let mut fam_ids = [foreign.clone(), fam_peer.clone()];
    fam_ids.sort();
    let mut sci_ids = [local.clone(), sci_peer.clone()];
    sci_ids.sort();

    // Destination: --parent follows --project, and the subject id on edit — but never
    // the subject's own id, which `apply_fields` rejects as a task's own parent
    let mut via_project = env.complete(
        &sci,
        "bash",
        6,
        &["tasks", "add", "T", "--project", "fam", "--parent", ""],
    );
    via_project.sort();
    assert_eq!(via_project, fam_ids);
    assert_eq!(
        env.complete(
            &sci,
            "bash",
            4,
            &["tasks", "edit", &foreign, "--parent", ""]
        ),
        [fam_peer.as_str()],
        "a task must not be offered as its own parent"
    );
    // add's title is not a subject: --parent stays local
    let mut via_add_title =
        env.complete(&sci, "bash", 4, &["tasks", "add", &foreign, "--parent", ""]);
    via_add_title.sort();
    assert_eq!(via_add_title, sci_ids);

    // Resolvable: --depends and dep --on reach any registered project — but never offer
    // the subject as its own dependency, which `dep`/`apply_fields` reject as a cycle
    let mut via_depends =
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--depends", "fam-"]);
    via_depends.sort();
    assert_eq!(via_depends, fam_ids);
    let mut via_dep_on = env.complete(&sci, "bash", 4, &["tasks", "dep", &local, "--on", "fam-"]);
    via_dep_on.sort();
    assert_eq!(via_dep_on, fam_ids);
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "dep", &local, "--on", ""]),
        [sci_peer.as_str()],
        "a task must not be offered as its own dependency"
    );
}

#[test]
fn resolvable_starts_from_the_destination_project() {
    let mut env = TestEnv::new();
    let fam = env.init("fam");
    let registered = id_of(env.json(&fam, &["add", "In the registered root"]));

    // a second `fam` root with different tasks: `add --project fam` validates against the
    // registered root, so completion must offer that one's ids, not this checkout's
    let worktree = env.init_forced("fam");
    env.json(&worktree, &["add", "Only in the worktree"]);
    // `init --force` repointed the registry; put it back so `fam` names the first root
    env.json(&fam, &["init", "--prefix", "fam", "--force"]);

    assert_eq!(
        env.complete(
            &worktree,
            "bash",
            6,
            &["tasks", "add", "T", "--project", "fam", "--depends", "fam-"]
        ),
        [registered.as_str()]
    );

    // and it works with no local project at all
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.complete(
            nowhere.path(),
            "bash",
            6,
            &["tasks", "add", "T", "--project", "fam", "--depends", "fam-"]
        ),
        [registered]
    );
}

#[test]
fn dep_rm_offers_only_current_dependencies_including_unreachable_ones() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let subject = id_of(env.json(&sci, &["add", "Subject"]));
    let other = id_of(env.json(&sci, &["add", "Not a dependency"]));
    let reachable = id_of(env.json(&fam, &["add", "Reachable dep"]));
    env.json(&sci, &["dep", &subject, "--on", &reachable]);

    let ids = env.complete(&sci, "bash", 4, &["tasks", "dep", &subject, "--rm", ""]);
    assert_eq!(ids, [reachable.as_str()]);
    assert!(!ids.contains(&other), "{ids:?}");

    // unregister fam: the dependency is now unreachable but still removable, so it stays
    // a candidate — described in zsh only while its task can be read
    let zsh = env.complete(&sci, "zsh", 4, &["tasks", "dep", &subject, "--rm", ""]);
    assert_eq!(zsh, [format!("{reachable}:todo  Reachable dep")]);
    env.json(&sci, &["unregister", "fam"]);
    assert_eq!(
        env.complete(&sci, "zsh", 4, &["tasks", "dep", &subject, "--rm", ""]),
        [reachable],
        "an unreachable dependency is offered bare, not dropped"
    );
}

#[test]
fn dependencies_are_ordered_open_before_closed_or_unresolvable_each_by_id() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let unr = env.init("unr");
    let subject = id_of(env.json(&sci, &["add", "Subject"]));
    let open = id_of(env.json(&sci, &["add", "Open dep"]));
    let closed = id_of(env.json(&fam, &["add", "Closed dep"]));
    env.json(&fam, &["done", &closed, "fixed"]);
    let unreachable = id_of(env.json(&unr, &["add", "Unreachable dep"]));
    // insertion order is deliberately the reverse of the expected output order, so a
    // completer that merely echoes `depends` back cannot pass by accident
    env.json(
        &sci,
        &["dep", &subject, "--on", &unreachable, &closed, &open],
    );
    // unregister last: the dependency is still offered, just unresolvable and sorted
    // with the closed ones — "fam" < "unr" so this also proves the id-ascending
    // tiebreak within that group, not just the open/closed split
    env.json(&sci, &["unregister", "unr"]);

    let ids = env.complete(&sci, "bash", 4, &["tasks", "dep", &subject, "--rm", ""]);
    assert_eq!(ids, [open.as_str(), closed.as_str(), unreachable.as_str()]);
}

#[test]
fn feedback_recur_offers_open_feedback_from_the_registered_tasks_root() {
    let mut env = TestEnv::new();
    let upstream = env.init("tasks");
    accept_feedback(&upstream, "The tasks CLI.");
    let sci = env.init("sci");
    let open = id_of(env.json(&upstream, &["add", "Open report", "--tag", "feedback"]));
    let closed = id_of(env.json(&upstream, &["add", "Closed report", "--tag", "feedback"]));
    env.json(&upstream, &["done", &closed, "fixed"]);
    let untagged = id_of(env.json(&upstream, &["add", "Not feedback"]));

    let ids = env.complete(
        &sci,
        "bash",
        6,
        &[
            "tasks",
            "feedback",
            "--project",
            "tasks",
            "S",
            "--recur",
            "",
        ],
    );
    assert_eq!(ids, [open.as_str()]);
    assert!(
        !ids.contains(&closed) && !ids.contains(&untagged),
        "{ids:?}"
    );

    // a worktree of the upstream does not leak its own records
    let worktree = env.init_forced("tasks");
    let only_here = id_of(env.json(&worktree, &["add", "Local only", "--tag", "feedback"]));
    env.json(&upstream, &["init", "--prefix", "tasks", "--force"]);
    let ids = env.complete(
        &worktree,
        "bash",
        6,
        &[
            "tasks",
            "feedback",
            "--project",
            "tasks",
            "S",
            "--recur",
            "",
        ],
    );
    assert_eq!(ids, [open]);
    assert!(!ids.contains(&only_here), "{ids:?}");
}

#[test]
fn parallel_is_set_by_flag_and_cleared_by_no_parallel() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    assert_eq!(env.json(&dir, &["show", &plain])["task"]["parallel"], false);

    let marked = id_of(env.json(&dir, &["add", "Marked", "--parallel"]));
    assert_eq!(env.json(&dir, &["show", &marked])["task"]["parallel"], true);
    assert!(
        env.read(&dir, &format!("tasks/{marked}.md"))
            .contains("\nparallel: true\n"),
        "the key is written unquoted"
    );

    // An unrelated edit must not disturb the flag.
    env.json(&dir, &["edit", &marked, "-p", "1"]);
    assert_eq!(env.json(&dir, &["show", &marked])["task"]["parallel"], true);

    env.json(&dir, &["edit", &marked, "--no-parallel"]);
    assert_eq!(
        env.json(&dir, &["show", &marked])["task"]["parallel"],
        false
    );
    assert!(
        !env.read(&dir, &format!("tasks/{marked}.md"))
            .contains("parallel"),
        "the key is dropped, not written false"
    );

    env.json(&dir, &["edit", &plain, "--parallel"]);
    assert_eq!(env.json(&dir, &["show", &plain])["task"]["parallel"], true);
}

#[test]
fn parallel_and_no_parallel_conflict() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "A"]));
    let out = env
        .cmd(&dir)
        .args(["edit", &id, "--parallel", "--no-parallel"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "clap must reject the pair");
}

#[test]
fn pretty_rows_show_the_parallel_marker_only_when_something_is_marked() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Plain", "-p", "1"]);

    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        !text.contains("||"),
        "no column when nothing is marked:\n{text}"
    );

    env.json(&dir, &["add", "Marked", "-p", "0", "--parallel"]);
    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    // `table` ends each row with '\n' and `println!` adds one more, so pretty output
    // always carries a trailing blank line; trim it before counting rows.
    let lines: Vec<&str> = text.trim_end().lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].contains("|| "), "{}", lines[0]);
    assert!(!lines[1].contains("||"), "{}", lines[1]);
    assert_eq!(
        lines[0].find("Marked"),
        lines[1].find("Plain"),
        "titles must start in the same column:\n{text}"
    );

    // The JSON key rides on every summary.
    let v = env.json(&dir, &["list"]);
    let flags: Vec<bool> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["parallel"].as_bool().unwrap())
        .collect();
    assert_eq!(flags, [true, false]);
}

#[test]
fn pretty_rows_show_the_type_letter_only_when_something_recurs() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let plain = id_of(env.json(&dir, &["add", "Plain", "-p", "1"]));

    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        !text.contains(" p "),
        "no column when nothing recurs:\n{text}"
    );

    let sweep = id_of(env.json(&dir, &["add", "Sweep", "-p", "0", "--every", "30d"]));
    let out = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.trim_end().lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    let sweep_line = lines
        .iter()
        .find(|line| line.contains("Sweep"))
        .unwrap_or_else(|| panic!("recurring task missing:\n{text}"));
    let plain_line = lines
        .iter()
        .find(|line| line.contains("Plain"))
        .unwrap_or_else(|| panic!("plain task missing:\n{text}"));
    assert!(sweep_line.contains(" p "), "{sweep_line}");
    assert!(!plain_line.contains(" p "), "{plain_line}");
    assert_eq!(
        sweep_line.find("Sweep"),
        plain_line.find("Plain"),
        "titles must start in the same column:\n{text}"
    );

    // The letter reads the existing `periodic` object; no `type` key reaches JSON.
    let v = env.json(&dir, &["list"]);
    let sweeps: Vec<&serde_json::Value> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|task| task["id"].as_str() == Some(sweep.as_str()))
        .collect();
    assert_eq!(sweeps.len(), 1, "{v}");
    assert!(sweeps[0].get("type").is_none(), "{:?}", sweeps[0]);
    assert_eq!(sweeps[0]["periodic"]["every"], "30d");
    assert!(
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"].as_str() == Some(plain.as_str())),
        "{v}"
    );
}

#[test]
fn prime_aligns_the_type_column_across_all_its_blocks() {
    // Like the `||` column, the decision to reserve the type column is made once for all
    // of prime's blocks, so a recurrence in `ready` does not shift dates in `doing`.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let sweep = id_of(env.json(&dir, &["add", "Sweep", "--every", "30d"]));
    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    env.json(&dir, &["start", &plain]);

    let out = env.cmd(&dir).args(["--pretty", "prime"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();

    let ready_block = text
        .split("\nready:\n")
        .nth(1)
        .unwrap_or_else(|| panic!("no ready section:\n{text}"))
        .split("\ndoing:\n")
        .next()
        .unwrap();
    let doing_block = text
        .split("\ndoing:\n")
        .nth(1)
        .unwrap_or_else(|| panic!("no doing section:\n{text}"));

    let ready_line = ready_block
        .lines()
        .find(|line| line.contains(&sweep))
        .unwrap_or_else(|| panic!("recurring task missing from ready:\n{text}"));
    let doing_line = doing_block
        .lines()
        .find(|line| line.contains(&plain))
        .unwrap_or_else(|| panic!("plain task missing from doing:\n{text}"));

    assert!(ready_line.contains(" p "), "{ready_line}");
    assert!(!doing_line.contains(" p "), "{doing_line}");

    let v = env.json(&dir, &["prime"]);
    let ready_date = &v["ready"][0]["updated"].as_str().unwrap()[..10];
    let doing_date = &v["doing"][0]["updated"].as_str().unwrap()[..10];
    assert_eq!(
        ready_line.find(ready_date),
        doing_line.find(doing_date),
        "dates must start in the same column across prime's blocks:\n{text}"
    );
}

#[test]
fn prime_aligns_the_parallel_column_across_all_its_blocks() {
    // prime's ready and doing blocks are rendered by separate `table` calls, but the
    // decision to reserve the `||` column is made once for all of prime's blocks together
    // (src/output.rs, the `Output::Prime` arm) so a mark in one block doesn't shift dates
    // out of alignment with an unmarked row in another block.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let marked = id_of(env.json(&dir, &["add", "Marked", "--parallel"]));
    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    env.json(&dir, &["start", &plain]);

    let out = env.cmd(&dir).args(["--pretty", "prime"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();

    let ready_block = text
        .split("\nready:\n")
        .nth(1)
        .unwrap_or_else(|| panic!("no ready section:\n{text}"))
        .split("\ndoing:\n")
        .next()
        .unwrap();
    let doing_block = text
        .split("\ndoing:\n")
        .nth(1)
        .unwrap_or_else(|| panic!("no doing section:\n{text}"));

    let ready_line = ready_block
        .lines()
        .find(|line| line.contains(&marked))
        .unwrap_or_else(|| panic!("marked task missing from ready:\n{text}"));
    let doing_line = doing_block
        .lines()
        .find(|line| line.contains(&plain))
        .unwrap_or_else(|| panic!("plain task missing from doing:\n{text}"));

    assert!(ready_line.contains("|| "), "{ready_line}");
    assert!(!doing_line.contains("||"), "{doing_line}");

    // Each row's own date (the first 10 chars of its `updated` timestamp) must start in
    // the same column in both blocks.
    let v = env.json(&dir, &["prime"]);
    let ready_date = &v["ready"][0]["updated"].as_str().unwrap()[..10];
    let doing_date = &v["doing"][0]["updated"].as_str().unwrap()[..10];
    assert_eq!(
        ready_line.find(ready_date),
        doing_line.find(doing_date),
        "dates must start in the same column across prime's blocks:\n{text}"
    );
}

/// A pipe whose read end is closed before the child writes: the first write gets EPIPE
/// whatever the output's size. Piping to `head` is the same situation with a race in it.
fn closed_pipe() -> (std::io::PipeReader, std::process::Stdio) {
    let (reader, writer) = std::io::pipe().unwrap();
    (reader, std::process::Stdio::from(writer))
}

#[test]
fn a_closed_stdout_reader_ends_the_command_quietly() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));

    let (reader, stdout) = closed_pipe();
    let child = env
        .raw(&dir)
        .args(["show", &id, "--pretty"])
        .stdout(stdout)
        .spawn()
        .unwrap();
    drop(reader);
    let out = child.wait_with_output().unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(out.status.code(), Some(0), "{stderr}");
}

#[test]
fn a_closed_stderr_reader_does_not_panic_either() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    // The error path writes to stderr and exits 1. With nothing reading it, the exit code
    // must still be the one the command earned -- not 101 from a panic.
    let (reader, stderr) = closed_pipe();
    let mut child = env
        .raw(&dir)
        .args(["show", "sci-abcdef"])
        .stderr(stderr)
        .spawn()
        .unwrap();
    drop(reader);
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(1));
}

#[test]
fn a_closed_stdout_reader_does_not_hide_what_check_found() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    std::fs::write(dir.join("tasks/sci-abcdef.md"), "garbage").unwrap();

    // Falling through rather than exiting on the broken pipe is what keeps this 1: the
    // findings are real whether or not anyone read the report.
    let (reader, stdout) = closed_pipe();
    let child = env
        .raw(&dir)
        .args(["check"])
        .stdout(stdout)
        .spawn()
        .unwrap();
    drop(reader);
    let out = child.wait_with_output().unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(out.status.code(), Some(1), "{stderr}");
}

#[test]
fn closeout_omits_a_goal_its_dependencies_still_hold_and_says_why() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&dir, &["add", "Child", "-p", "2", "--parent", &goal]));
    let blocker = id_of(env.json(&dir, &["add", "Blocker", "-p", "2"]));
    env.json(&dir, &["dep", &goal, "--on", &blocker]);
    env.json(&dir, &["done", &child, "done"]);

    // closeout means "done will accept this". While the dependency is open it will not:
    // `done` answers open_dependencies, so an invitation here is one the tool refuses.
    let v = env.json(&dir, &["prime"]);
    let closeout: Vec<&str> = v["closeout"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(closeout.is_empty(), "{closeout:?}");
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&goal) && w.contains(&blocker)
        }),
        "the goal must not go silent: {v}"
    );
    assert_eq!(env.fail(&dir, &["done", &goal, "x"]), "open_dependencies");

    // Once the dependency closes the goal is genuinely closeable, and the notice goes away.
    env.json(&dir, &["done", &blocker, "done"]);
    let v = env.json(&dir, &["prime"]);
    let closeout: Vec<&str> = v["closeout"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(closeout, [goal.as_str()], "{v}");
    assert!(
        !v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains(&goal)),
        "{v}"
    );
    env.json(&dir, &["done", &goal, "met"]);
}

#[test]
fn tree_routes_a_bare_id_by_its_prefix_like_show() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let goal = id_of(env.json(&fam, &["add", "Goal", "-p", "2"]));
    let kid = id_of(env.json(&fam, &["add", "Kid", "-p", "2", "--parent", &goal]));

    // The subtree of an id is read from that id's own project (multi-project design §4),
    // the way `show` and every write command already route it.
    let v = env.json(&sci, &["tree", &goal]);
    assert_eq!(v["nodes"][0]["id"], goal, "{v}");
    assert_eq!(v["nodes"][0]["children"][0]["id"], kid, "{v}");

    // The errors `show` gives for the same ids, rather than a misleading task_not_found
    // for a prefix that names no project at all.
    assert_eq!(env.fail(&sci, &["tree", "zzz-000001"]), "unresolvable_id");
    assert_eq!(env.fail(&sci, &["tree", "fam-ffffff"]), "task_not_found");

    // An explicit --project is the caller naming the scope, and wins over the prefix.
    assert_eq!(
        env.fail(&sci, &["tree", "--project", "sci", &goal]),
        "task_not_found"
    );

    // A local id still reads locally.
    let here = id_of(env.json(&sci, &["add", "Here", "-p", "2"]));
    let v = env.json(&sci, &["tree", &here]);
    assert_eq!(v["nodes"][0]["id"], here, "{v}");
}

#[test]
fn concurrent_registry_writes_do_not_lose_each_other() {
    let env = TestEnv::new();
    let dirs: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();
    let lock_path = env.home.path().join(".config/tasks/projects.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .unwrap();
    held.lock().unwrap();
    let mut children: Vec<_> = ["aaa", "bbb"]
        .iter()
        .zip(&dirs)
        .map(|(prefix, dir)| {
            env.raw(dir.path())
                .args(["init", "--prefix", prefix])
                .spawn()
                .unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let blocked = children
        .iter_mut()
        .all(|child| child.try_wait().unwrap().is_none());
    drop(held);
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| reap(child, REAP))
        .collect();
    assert!(blocked, "a registry writer ran without the lock");
    for output in outputs {
        assert!(output.unwrap().status.success());
    }
    let text = env.read(env.home.path(), ".config/tasks/projects.toml");
    assert!(text.contains("aaa = ") && text.contains("bbb = "), "{text}");
}

#[test]
fn unregister_reloads_the_registry_after_waiting_for_its_lock() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let held = File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(env.home.path().join(".config/tasks/projects.lock"))
        .unwrap();
    held.lock().unwrap();
    let mut child = env.raw(&sci).args(["unregister", "sci"]).spawn().unwrap();
    let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
    alias_registry(&env, "family", "fam");
    drop(held);
    let out = reap(child, REAP).unwrap();
    assert!(blocked, "unregister ignored the registry lock");
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        env.json(&fam, &["root", "family-000001"])["root"],
        fam.to_str().unwrap()
    );
}

#[test]
fn add_revalidates_local_config_and_registered_routing_after_locking() {
    for (registered, sourced) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut env = TestEnv::new();
        let local = env.init("sci");
        let root = env.init_forced("sci");
        let held = hold_project_lock(&env, "sci");
        let mut command = env.raw(&local);
        command.args(["add", "Waiting"]);
        if sourced {
            command.args(["--source", "test"]);
        }
        if registered {
            command.args(["--project", "sci"]);
        }
        let mut child = command.spawn().unwrap();
        let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
        let target = if registered { &root } else { &local };
        std::fs::write(target.join("tasks/.config.toml"), "prefix = \"fresh\"\n").unwrap();
        if registered {
            let path = env.home.path().join(".config/tasks/projects.toml");
            std::fs::write(
                path,
                format!(
                    "[projects]\nfresh = {:?}\n[aliases]\nsci = \"fresh\"\n",
                    root.to_str().unwrap()
                ),
            )
            .unwrap();
        }
        drop(held);
        let out = reap(child, REAP).unwrap();
        assert!(blocked, "add ignored the project lock");
        assert!(out.status.success(), "{out:?}");
        let output: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let id = output["id"].as_str().unwrap();
        assert!(id.starts_with("fresh-"), "{output}");
        assert!(target.join(format!("tasks/{id}.md")).exists());
        let other = if registered { &local } else { &root };
        assert!(!other.join(format!("tasks/{id}.md")).exists());
    }
}

#[test]
fn feedback_create_and_recur_revalidate_only_the_registered_target() {
    for recur in [false, true] {
        let (env, target, source) = feedback_env();
        let original = env.json(
            &source,
            &[
                "feedback",
                "--project",
                "tasks",
                "Original report",
                "--category",
                "gap",
                "--new",
            ],
        );
        let id = original["id"].as_str().unwrap();
        let original_raw = env.read(&target, &format!("tasks/{id}.md"));
        let source_held = hold_project_lock(&env, "sci");
        let held = hold_project_lock(&env, "tasks");
        let mut command = env.raw(&source);
        command.args([
            "feedback",
            "--project",
            "tasks",
            "Another report",
            "--category",
            "gap",
        ]);
        if recur {
            command.args(["--recur", id]);
        } else {
            command.arg("--new");
        }
        let mut child = command.spawn().unwrap();
        let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
        let new_id = id.replacen("tasks-", "tracker-", 1);
        std::fs::write(
            target.join("tasks/.config.toml"),
            "prefix = \"tracker\"\n\n[feedback]\nscope = \"The tasks CLI.\"\n",
        )
        .unwrap();
        std::fs::rename(
            target.join(format!("tasks/{id}.md")),
            target.join(format!("tasks/{new_id}.md")),
        )
        .unwrap();
        std::fs::write(
            target.join(format!("tasks/{new_id}.md")),
            original_raw.replacen(id, &new_id, 1),
        )
        .unwrap();
        let path = env.home.path().join(".config/tasks/projects.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("tasks = ", "tracker = ");
        std::fs::write(&path, text).unwrap();
        alias_registry(&env, "tasks", "tracker");
        drop(held);
        let out = reap(child, REAP);
        drop(source_held);
        assert!(blocked, "feedback ignored the target lock");
        let out = out.expect("feedback took the source lock or deadlocked");
        assert!(out.status.success(), "{out:?}");
        let output: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let result_id = output["id"].as_str().unwrap();
        assert!(result_id.starts_with("tracker-"), "{output}");
        let raw = env.read(&target, &format!("tasks/{result_id}.md"));
        assert!(raw.contains("from:sci"), "{raw}");
        if recur {
            assert!(raw.contains("feedback from sci: Another report"), "{raw}");
        }
        // Starting with the retired target name must also work.
        env.json(
            &source,
            &[
                "feedback",
                "--project",
                "tasks",
                "Third report",
                "--category",
                "gap",
                "--new",
            ],
        );
    }
}

#[test]
fn editor_revalidation_keeps_the_edit_when_the_local_prefix_is_retired() {
    let mut env = TestEnv::new();
    let local = env.init("sci");
    let live = env.init("fresh");
    let id = id_of(env.json(&local, &["add", "Original"]));
    let original = env.read(&local, &format!("tasks/{id}.md"));
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let editor = editor_script(
        &local,
        &format!(
            "sed -i 's/^title: Original$/title: Edited/' \"$1\"\nprintf '[projects]\\nfresh = \"{}\"\\n[aliases]\\nsci = \"fresh\"\\n' > \"{}\"",
            live.display(),
            registry.display()
        ),
    );
    let out = env
        .cmd(&local)
        .env("EDITOR", editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "editor wrote after its prefix was retired"
    );
    assert_eq!(err_kind(&out), "config");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(".edit.md"),
        "{error}"
    );
    assert_eq!(env.read(&local, &format!("tasks/{id}.md")), original);
    assert!(
        std::fs::read_dir(local.join("tasks"))
            .unwrap()
            .any(|entry| {
                let path = entry.unwrap().path();
                path.to_string_lossy().ends_with(".edit.md")
                    && std::fs::read_to_string(path)
                        .unwrap()
                        .contains("title: Edited")
            })
    );
}

#[test]
fn id_writer_revalidates_doc_roots_even_when_identity_is_unchanged() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Original"]));
    std::fs::create_dir(dir.join("changed")).unwrap();
    std::fs::write(dir.join("changed/new-spec.md"), "# New spec\n").unwrap();
    let held = hold_project_lock(&env, "sci");
    let mut child = env
        .raw(&dir)
        .args(["edit", &id, "--spec", "new-spec"])
        .spawn()
        .unwrap();
    let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\nspec_dirs = [\"changed\"]\n",
    )
    .unwrap();
    drop(held);
    let out = reap(child, REAP).unwrap();
    assert!(blocked);
    assert!(
        out.status.success(),
        "writer used stale document roots: {out:?}"
    );
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["spec"],
        "changed/new-spec.md"
    );
}

#[test]
fn rename_rewrites_the_project_and_keeps_inbound_refs_resolving() {
    let mut env = TestEnv::new();
    let dots = env.init("dot");
    let ops = env.init("ops");
    git(&dots, &["init", "-q", "-b", "main"]);
    let mine = id_of(env.json(&dots, &["add", "Mine", "-p", "2"]));
    let theirs = id_of(env.json(&ops, &["add", "Theirs", "-p", "2"]));
    env.json(&ops, &["dep", &theirs, "--on", &mine]);
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);

    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["prefix"], "dots");
    assert_eq!(v["previous"], "dot");
    assert_eq!(v["tasks"], 1);
    assert_eq!(v["recovery"], "fresh");
    assert_eq!(v["aliases"], serde_json::json!(["dot"]));

    let moved = format!("dots-{}", mine.split_once('-').unwrap().1);
    assert!(dots.join(format!("tasks/{moved}.md")).is_file());
    assert!(!dots.join(format!("tasks/{mine}.md")).exists());
    assert_eq!(env.json(&dots, &["show", &moved])["task"]["id"], moved);

    // The other project was not written to, and its stored ref still resolves.
    assert_eq!(
        env.json(&ops, &["show", &theirs])["task"]["depends"][0],
        mine
    );
    assert_eq!(env.json(&ops, &["show", &mine])["task"]["id"], moved);

    // A completed re-run reports Complete rather than tripping a fresh-operation refusal.
    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["recovery"], "complete");

    // --explain writes nothing and reports the same verdict.
    let before =
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert_eq!(
        env.json(&dots, &["rename", "dot", "dots", "--explain"])["recovery"],
        "complete"
    );
    assert_eq!(
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
        before
    );
}

#[test]
fn an_add_waiting_through_a_rename_never_writes_the_old_prefix() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let seed = id_of(env.json(&dir, &["add", "Seed", "-p", "2"]));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    let moved_seed = format!("dots-{}", seed.split_once('-').unwrap().1);

    // This tests revalidation, not the rename executor (covered separately above).
    // Hold the lock and establish a completed-rename fixture while the add waits.
    let lock_path = env.claim_store("dot").with_file_name("dot.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();

    let mut adder = env
        .raw(&dir)
        .args(["add", "Late", "-p", "2"])
        .spawn()
        .unwrap();
    // The kernel listing the child as a waiter on this lock proves it constructed its
    // old-prefix context and reached lock acquisition. A sleep alone could leave it not
    // yet started. Its descriptor table cannot prove it: spawn() can return while the
    // child is still inside execve, before our own close-on-exec lock descriptor leaves it.
    let inode = std::os::unix::fs::MetadataExt::ino(&held.metadata().unwrap()).to_string();
    let pid = adder.id().to_string();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(
            adder.try_wait().unwrap().is_none(),
            "the add exited before acquiring its lock"
        );
        // A blocked waiter: `N: -> FLOCK  ADVISORY  WRITE <pid> <major:minor:inode> 0 EOF`.
        let waiting = std::fs::read_to_string("/proc/locks")
            .unwrap()
            .lines()
            .any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                fields.len() > 6
                    && fields[1] == "->"
                    && fields[2] == "FLOCK"
                    && fields[5] == pid
                    && fields[6].rsplit(':').next() == Some(inode.as_str())
            });
        if waiting {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the add never waited on its lock"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Only this one seed task exists. Install its renamed file, config and registry
    // directly while the lock is held; invoking rename here would wait on our lock too.
    let source = dir.join(format!("tasks/{seed}.md"));
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(
        dir.join(format!("tasks/{moved_seed}.md")),
        text.replacen(&format!("id: {seed}\n"), &format!("id: {moved_seed}\n"), 1),
    )
    .unwrap();
    std::fs::remove_file(source).unwrap();
    let config_path = dir.join("tasks/.config.toml");
    let mut config: toml::Value =
        toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["prefix"] = toml::Value::String("dots".into());
    std::fs::write(config_path, toml::to_string(&config).unwrap()).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    let projects = registry["projects"].as_table_mut().unwrap();
    let root = projects.remove("dot").unwrap();
    projects.insert("dots".into(), root);
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    alias_registry(&env, "dot", "dots");
    assert!(dir.join(format!("tasks/{moved_seed}.md")).is_file());

    drop(held);
    let out = adder.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let added: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let late = added["id"].as_str().unwrap();
    assert!(
        late.starts_with("dots-"),
        "the queued add must use the refreshed prefix: {added}"
    );
    assert_eq!(env.json(&dir, &["show", late])["task"]["title"], "Late");

    // No later rename can mask a stale write: only the queued add ran after the fixture.
    let stale: Vec<_> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("dot-"))
        .collect();
    assert!(
        stale.is_empty(),
        "an old-prefix file survived the rename: {stale:?}"
    );
    assert_eq!(
        env.json(&dir, &["show", &moved_seed])["task"]["id"],
        moved_seed
    );
    assert_eq!(
        env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn an_add_waiting_through_a_pending_rename_is_refused() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "Seed", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // An interrupted rename leaves the freeze in place; a waiter that wakes into it is
    // refused rather than writing under either name.
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();
    let out = env
        .raw(&dir)
        .args(["add", "Late", "-p", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
}

#[test]
fn a_pending_rename_freezes_the_project_including_the_p4_window() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "T", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // Stop after the config write: files and config renamed, registry untouched. This is
    // the window where a lookup by resolved prefix would miss rename/dot.toml entirely.
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();

    assert_eq!(env.fail(&dir, &["add", "New", "-p", "2"]), "validation");
    assert_eq!(env.fail(&dir, &["unregister", "dot"]), "validation");
    let config_before = std::fs::read(dir.join("tasks/.config.toml")).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let registry_before = std::fs::read(&registry_path).unwrap();
    let baseline_path = env.home.path().join(".local/state/tasks/rename/dot.toml");
    let baseline_before = std::fs::read(&baseline_path).unwrap();
    let files_before: std::collections::BTreeMap<_, _> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect();
    assert_eq!(
        env.fail(&dir, &["add", "Explicit", "--project", "dot", "-p", "2"]),
        "config"
    );
    assert_eq!(
        std::fs::read(dir.join("tasks/.config.toml")).unwrap(),
        config_before
    );
    assert_eq!(std::fs::read(registry_path).unwrap(), registry_before);
    assert_eq!(std::fs::read(baseline_path).unwrap(), baseline_before);
    let files_after: std::collections::BTreeMap<_, _> = std::fs::read_dir(dir.join("tasks"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect();
    assert_eq!(files_after, files_before);

    assert!(
        env.json(&dir, &["list"])["tasks"].is_array(),
        "reads still work"
    );

    // The target is reserved even though it is in no registry yet.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(fresh.path(), &["init", "--prefix", "dots"]),
        "config"
    );

    // And the rename completes on a re-run.
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots"])["recovery"],
        "resume_registry"
    );
    assert!(
        env.json(&dir, &["add", "New", "-p", "2"])["id"]
            .as_str()
            .unwrap()
            .starts_with("dots-")
    );
}

#[test]
fn rename_refuses_a_dirty_tree_a_live_claim_and_a_second_worktree() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation"); // uncommitted
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "claimed");
}

#[test]
fn rename_collapses_aliases_and_accepts_an_older_alias_as_source() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.json(&dir, &["rename", "dot", "dots"]);
    let v = env.json(&dir, &["rename", "dot", "config"]);
    assert_eq!(v["previous"], "dots");
    assert_eq!(v["aliases"], serde_json::json!(["dot", "dots"]));
    let registry: toml::Value =
        toml::from_str(&env.read(&env.home.path().join(".config"), "tasks/projects.toml")).unwrap();
    assert_eq!(registry["aliases"]["dot"].as_str(), Some("config"));
    assert_eq!(registry["aliases"]["dots"].as_str(), Some("config"));
    assert!(
        env.json(&dir, &["show", &id])["task"]["id"]
            .as_str()
            .unwrap()
            .starts_with("config-")
    );
}

#[test]
fn rename_preflight_refusals_leave_no_inventory_or_changed_files() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.init("taken");
    alias_registry(&env, "retired", "taken");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let path = dir.join(format!("tasks/{id}.md"));
    let original = std::fs::read_to_string(&path).unwrap();
    for target in ["../bad", "dot", "taken", "retired"] {
        let out = env
            .raw(&dir)
            .args(["rename", "dot", target])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(!env.home.path().join(".local/state/tasks/rename").exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    for invalid in [
        original.replace("priority: 2", "priority: 9"),
        original.replace(&format!("id: {id}"), "id: dot-ffffff"),
    ] {
        std::fs::write(&path, invalid).unwrap();
        assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "parse");
        assert!(!env.home.path().join(".local/state/tasks/rename").exists());
    }
}

#[test]
fn rename_explain_never_creates_locks_or_inventory_and_skips_authorization() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let registry = std::fs::read(&registry_path).unwrap();
    let config = std::fs::read(dir.join("tasks/.config.toml")).unwrap();
    // init creates only the registry lock; remove it so explain must prove no lock creation.
    std::fs::remove_file(registry_path.with_file_name("projects.lock")).unwrap();
    let value = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(value["recovery"], "fresh");
    assert!(!registry_path.with_file_name("projects.lock").exists());
    assert!(!env.home.path().join(".local/state/tasks").exists());
    assert_eq!(std::fs::read(&registry_path).unwrap(), registry);
    assert_eq!(
        std::fs::read(dir.join("tasks/.config.toml")).unwrap(),
        config
    );

    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
        "fresh"
    );
}

#[test]
fn rename_refuses_a_second_worktree_even_with_no_task_copies() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    let other = tempfile::tempdir().unwrap();
    git(
        &dir,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            other.path().to_str().unwrap(),
        ],
    );
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("worktree"));
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_pending_target_is_reserved_against_another_rename() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let other = env.init("other");
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(env.fail(&other, &["rename", "other", "dots"]), "config");
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/other.toml")
            .exists()
    );
}

#[test]
fn rename_explain_reports_refusal_without_touching_the_baseline() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    let inventory_path = env.home.path().join(".local/state/tasks/rename/dot.toml");
    let before = std::fs::read(&inventory_path).unwrap();
    std::fs::remove_file(dir.join(format!("tasks/{id}.md"))).unwrap();
    let out = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(out["recovery"], "refuse");
    assert!(out["warnings"][0].as_str().unwrap().contains("R4"));
    assert_eq!(std::fs::read(inventory_path).unwrap(), before);
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
}

#[test]
fn rename_an_older_alias_resumes_cleanup_using_the_real_source_inventory() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.json(&dir, &["rename", "dot", "dots"]);
    let stopped = env
        .raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "registry")
        .args(["rename", "dot", "config"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let baseline = env.home.path().join(".local/state/tasks/rename/dots.toml");
    assert!(baseline.is_file());
    let out = env.json(&dir, &["rename", "dot", "config"]);
    assert_eq!(out["recovery"], "resume_cleanup");
    assert_eq!(out["previous"], "dots");
    assert!(!baseline.exists());
    assert!(
        env.json(&dir, &["add", "After", "-p", "2"])["id"]
            .as_str()
            .unwrap()
            .starts_with("config-")
    );
}

#[test]
fn rename_reports_only_destination_files_written_by_this_invocation() {
    for (boundary, written, resumed, verdict) in [
        ("inventory", 0, 2, "resume_files"),
        ("file:0", 1, 1, "resume_files"),
        ("files", 2, 0, "resume_files"),
        ("config", 2, 0, "resume_registry"),
        ("registry", 2, 0, "resume_cleanup"),
        ("claims", 2, 0, "resume_cleanup"),
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        env.json(&dir, &["add", "One", "-p", "2"]);
        env.json(&dir, &["add", "Two", "-p", "2"]);
        let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
        assert_eq!(explain["tasks"], 0, "fresh explain at {boundary}");
        let output = env
            .raw(&dir)
            .env("TASKS_RENAME_STOP_AFTER", boundary)
            .args(["rename", "dot", "dots"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{boundary}: {output:?}");
        let stopped: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(stopped["tasks"], written, "stop {boundary}");
        let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
        assert_eq!(explain["tasks"], 0, "pending explain at {boundary}");
        let recovered = env.json(&dir, &["rename", "dot", "dots"]);
        assert_eq!(recovered["recovery"], verdict, "resume {boundary}");
        assert_eq!(recovered["tasks"], resumed, "resume {boundary}");
        let completed = env.json(&dir, &["rename", "dot", "dots"]);
        assert_eq!(completed["recovery"], "complete");
        assert_eq!(completed["tasks"], 0, "complete after {boundary}");
        assert_eq!(
            env.json(&dir, &["rename", "dot", "dots", "--explain"])["tasks"],
            0
        );
    }
}

#[test]
fn rename_migrates_park_entries_to_the_target_store() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let one = id_of(env.json(&dir, &["add", "One", "-p", "2"]));
    let two = id_of(env.json(&dir, &["add", "Two", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["park", &one, "a"])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["park", &two, "b", "--waiting-on", "user"])
        .assert()
        .success();
    write_claim(&env, "dot", "dot-ffffff", "ghost", false);

    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["parks"],
        2
    );
    let out = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(out["parks"], 2);
    assert!(
        !env.claim_store("dot").exists(),
        "the source store is removed"
    );
    let target = std::fs::read_to_string(env.claim_store("dots")).unwrap();
    assert!(
        target.contains(&format!("[parks.dots-{}]", &one[4..])),
        "{target}"
    );
    assert!(!target.contains("dot-"), "{target}");
    assert!(!target.contains("claims"), "{target}");

    let prime = env.json(&dir, &["prime"]);
    let ids: Vec<&str> = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| id.starts_with("dots-")), "{ids:?}");
    let completed = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(completed["recovery"], "complete");
    assert_eq!(completed["parks"], 2);
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["parks"],
        2
    );
}

#[test]
fn rename_refuses_a_target_store_that_holds_parks_before_any_mutation() {
    let mut env = TestEnv::new();
    let new = env.init("new");
    let theirs = id_of(env.json(&new, &["add", "Theirs", "-p", "2"]));
    as_agent(&env, &new, "agent-a")
        .args(["park", &theirs, "keep"])
        .assert()
        .success();
    env.json(&new, &["unregister", "new"]);
    let old = env.init("old");
    let mine = id_of(env.json(&old, &["add", "Mine", "-p", "2"]));
    as_agent(&env, &old, "agent-a")
        .args(["park", &mine, "move"])
        .assert()
        .success();
    let before_new = std::fs::read_to_string(env.claim_store("new")).unwrap();
    let before_old = std::fs::read_to_string(env.claim_store("old")).unwrap();

    let out = env
        .cmd(&old)
        .args(["rename", "old", "new"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "validation");
    assert!(
        err_detail(&out).contains("target store holds 1 parked task"),
        "{}",
        err_detail(&out)
    );
    assert!(err_detail(&out).contains(&theirs), "{}", err_detail(&out));
    assert_eq!(
        std::fs::read_to_string(env.claim_store("new")).unwrap(),
        before_new
    );
    assert_eq!(
        std::fs::read_to_string(env.claim_store("old")).unwrap(),
        before_old
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/old.toml")
            .exists(),
        "no inventory"
    );
    assert!(
        old.join(format!("tasks/{mine}.md")).is_file(),
        "no file moved"
    );
}

#[test]
fn rename_carries_an_escalation_only_store_and_refuses_one_at_the_target() {
    let mut env = TestEnv::new();
    let dot = env.init("dot");
    let id = id_of(env.json(&dot, &["add", "T", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &dot, "agent-a")
        .args([
            "park",
            &id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    // Resume and finish the session so the store holds an escalation and nothing else.
    as_agent(&env, &dot, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &dot, "agent-a")
        .args(["edit", &id, "--status", "todo"])
        .assert()
        .success();
    let store = std::fs::read_to_string(env.claim_store("dot")).unwrap();
    assert!(
        store.contains("[escalations.") && !store.contains("[parks."),
        "{store}"
    );

    let v = env.json(&dot, &["rename", "dot", "dots"]);
    assert_eq!(v["escalations"], 1, "{v}");
    assert_eq!(v["parks"], 0);
    assert!(!env.claim_store("dot").exists());
    let store = std::fs::read_to_string(env.claim_store("dots")).unwrap();
    assert!(store.contains("[escalations.dots-"), "{store}");
    let new_id = id.replace("dot-", "dots-");
    let v = env.json(&dot, &["show", &new_id]);
    assert_eq!(v["escalation"]["level"], "high");
    let v = env.json(&dot, &["ready", "--max-complexity", "mid"]);
    assert!(
        v["tasks"].as_array().unwrap().is_empty(),
        "still hidden after the rename: {v}"
    );

    // A target store holding only an escalation is a destination conflict.
    let other = env.init("fam");
    let other_id = id_of(env.json(&other, &["add", "F", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &other, "agent-b")
        .args([
            "park",
            &other_id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    as_agent(&env, &other, "agent-b")
        .args(["start", &other_id])
        .assert()
        .success();
    as_agent(&env, &other, "agent-b")
        .args(["edit", &other_id, "--status", "todo"])
        .assert()
        .success();
    // Leave `fam` registered but make its store the target of a rename from `dots`.
    env.json(&other, &["unregister", "fam"]);
    let out = env
        .cmd(&dot)
        .args(["rename", "dots", "fam"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("target store holds"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rename_interrupted_after_the_store_write_resumes_and_a_tampered_destination_refuses() {
    for tamper in [false, true] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
        as_agent(&env, &dir, "agent-a")
            .args(["park", &id, "a"])
            .assert()
            .success();
        let escalated = id_of(env.json(&dir, &["add", "E", "-p", "2", "--complexity", "low"]));
        as_agent(&env, &dir, "agent-a")
            .args([
                "park",
                &escalated,
                "stuck",
                "--reason",
                "capability",
                "--complexity",
                "high",
            ])
            .assert()
            .success();
        // Resume and finish the session so this task's only surviving entry is the escalation.
        as_agent(&env, &dir, "agent-a")
            .args(["start", &escalated])
            .assert()
            .success();
        as_agent(&env, &dir, "agent-a")
            .args(["edit", &escalated, "--status", "todo"])
            .assert()
            .success();
        let stopped = env
            .raw(&dir)
            .env("TASKS_RENAME_STOP_AFTER", "store")
            .args(["rename", "dot", "dots"])
            .output()
            .unwrap();
        assert!(stopped.status.success(), "{stopped:?}");
        assert!(env.claim_store("dots").is_file(), "destination written");
        assert!(env.claim_store("dot").is_file(), "source still present");
        let dest = std::fs::read_to_string(env.claim_store("dots")).unwrap();
        assert!(
            dest.contains(&format!("[escalations.dots-{}]", &escalated[4..])),
            "{dest}"
        );
        if tamper {
            write_park(&env, "dots", "dots-ffffff", "someone", "agent", "/x");
            let out = env
                .cmd(&dir)
                .args(["rename", "dot", "dots"])
                .output()
                .unwrap();
            assert_eq!(err_kind(&out), "validation");
            assert!(
                err_detail(&out).contains("disagrees with inventory"),
                "{}",
                err_detail(&out)
            );
            assert!(
                env.claim_store("dot").is_file(),
                "the source is left in place"
            );
        } else {
            let resumed = env.json(&dir, &["rename", "dot", "dots"]);
            assert_eq!(resumed["recovery"], "resume_cleanup");
            assert_eq!(resumed["parks"], 1);
            assert_eq!(resumed["escalations"], 1);
            assert!(!env.claim_store("dot").exists());
            assert_eq!(
                env.json(&dir, &["prime"])["parked"][0]["id"],
                format!("dots-{}", &id[4..])
            );
            assert_eq!(
                env.json(&dir, &["show", &format!("dots-{}", &escalated[4..])])["escalation"]["level"],
                "high"
            );
        }
    }

    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    as_agent(&env, &dir, "agent-a")
        .args(["park", &id, "a"])
        .assert()
        .success();
    let escalated = id_of(env.json(&dir, &["add", "E", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &dir, "agent-a")
        .args([
            "park",
            &escalated,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["start", &escalated])
        .assert()
        .success();
    as_agent(&env, &dir, "agent-a")
        .args(["edit", &escalated, "--status", "todo"])
        .assert()
        .success();
    let stopped = env
        .raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "claims")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(!env.claim_store("dot").exists(), "source removed");
    let explained = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explained["parks"], 1);
    assert_eq!(explained["escalations"], 1);
    let resumed = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(resumed["recovery"], "resume_cleanup");
    assert_eq!(resumed["parks"], 1);
    assert_eq!(resumed["escalations"], 1);
}

#[test]
fn rename_taken_live_and_alias_targets_are_config_errors() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    env.init("taken");
    alias_registry(&env, "retired", "taken");
    for target in ["taken", "retired"] {
        assert_eq!(
            env.fail(&dir, &["rename", "dot", target]),
            "config",
            "target {target}"
        );
        assert!(
            !env.home
                .path()
                .join(".local/state/tasks/rename/dot.toml")
                .exists()
        );
    }
}

fn rename_fixture(
    env: &mut TestEnv,
    count: usize,
    in_git: bool,
) -> (std::path::PathBuf, Vec<String>) {
    let dir = env.init("dot");
    let ids = (0..count)
        .map(|n| id_of(env.json(&dir, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();
    if in_git {
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "seed"]);
    }
    (dir, ids)
}

fn adopt_cmd(env: &TestEnv, dir: &std::path::Path) -> assert_cmd::Command {
    let mut command = env.cmd(dir);
    command
        .env("XDG_CONFIG_HOME", env.home.path().join("config"))
        .env("XDG_STATE_HOME", env.home.path().join("state"));
    command
}

fn adopt_json(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = adopt_cmd(env, dir).args(args).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn adopt_fixture(env: &TestEnv) -> (std::path::PathBuf, String) {
    let dir = env.home.path().join("checkout");
    std::fs::create_dir(&dir).unwrap();
    adopt_json(env, &dir, &["init", "--prefix", "old"]);
    let old_id = id_of(adopt_json(env, &dir, &["add", "Task"]));
    let old_path = dir.join(format!("tasks/{old_id}.md"));
    let new_id = old_id.replacen("old-", "new-", 1);
    let new_path = dir.join(format!("tasks/{new_id}.md"));
    let text = std::fs::read_to_string(&old_path).unwrap();
    std::fs::write(&new_path, text.replacen(&old_id, &new_id, 1)).unwrap();
    std::fs::remove_file(old_path).unwrap();
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(config, text.replace("prefix = \"old\"", "prefix = \"new\"")).unwrap();
    (dir, old_id)
}

fn adopt_old_park(env: &TestEnv, old_id: &str) {
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("old.toml"),
        format!(
            "[parks.\"{old_id}\"]\nowner = \"tester\"\nsession = \"codex:old\"\nhost = \"host\"\nworktree = \"/old\"\nat = \"2026-09-27T00:00:00Z\"\nnext_step = \"resume\"\nwaiting_on = \"agent\"\ntitle = \"Task\"\n"
        ),
    )
    .unwrap();
}

#[test]
fn adopt_registers_synced_prefix_without_touching_checkout() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    let path = dir.join(format!("tasks/{new_id}.md"));
    let before = std::fs::read(&path).unwrap();
    let out = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(out["mode"], "adopt");
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(out["tasks"], 0);
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        adopt_json(&env, &dir, &["show", &old_id])["task"]["id"],
        new_id
    );
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "complete"
    );
}

#[test]
fn adopt_preserves_target_claims_when_carrying_an_old_park() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let state = env.home.path().join("state/tasks/claims");
    adopt_old_park(&env, &old_id);
    let stale_claim = "[claims.\"new-ffffff\"]\nowner = \"tester\"\nsession = \"old\"\nhost = \"host\"\nworktree = \"/old\"\nstarted = \"2020-01-01T00:00:00Z\"\nseen = \"2020-01-01T00:00:00Z\"\n";
    std::fs::write(state.join("new.toml"), stale_claim).unwrap();
    let out = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(out["parks"], 1);
    let target = std::fs::read_to_string(state.join("new.toml")).unwrap();
    assert!(target.contains("[claims.new-ffffff]"), "{target}");
    assert!(
        target.contains(&format!("[parks.{}]", old_id.replacen("old-", "new-", 1))),
        "{target}"
    );
}

#[test]
fn adopt_retry_after_local_start_consumes_the_only_park() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_old_park(&env, &old_id);
    let stopped = adopt_cmd(&env, &dir)
        .env("TASKS_RENAME_STOP_AFTER", "store")
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let started = adopt_cmd(&env, &dir)
        .env("CODEX_SESSION_ID", "test-thread")
        .args(["start", &new_id])
        .output()
        .unwrap();
    assert!(started.status.success(), "{started:?}");
    let claimed = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&claimed), "claimed");
    let target_path = env.home.path().join("state/tasks/claims/new.toml");
    let text = std::fs::read_to_string(&target_path).unwrap();
    std::fs::write(
        &target_path,
        text.replace("seen = \"2026-", "seen = \"2020-"),
    )
    .unwrap();
    let resumed = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert_eq!(resumed["recovery"], "resume_registry");
    assert_eq!(resumed["parks"], 0);
    let target = std::fs::read_to_string(target_path).unwrap();
    assert!(target.contains(&format!("[claims.{new_id}]")), "{target}");
    assert!(!target.contains(&format!("[parks.{new_id}]")), "{target}");
}

#[test]
fn adopt_same_prefix_refuses_without_waiting_on_the_lock_twice() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let mut child = env
        .raw(&dir)
        .env("XDG_CONFIG_HOME", env.home.path().join("config"))
        .env("XDG_STATE_HOME", env.home.path().join("state"))
        .args(["rename", "new", "new", "--adopt"])
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    if status.is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
    }
    assert_eq!(status.unwrap().code(), Some(1));
}

#[test]
fn adopt_explain_refuses_foreign_old_project_without_writes() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let foreign = env.home.path().join("foreign");
    std::fs::create_dir_all(foreign.join("tasks")).unwrap();
    std::fs::write(foreign.join("tasks/.config.toml"), "prefix = \"old\"\n").unwrap();
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["old"] = toml::Value::String(foreign.display().to_string());
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let before = std::fs::read(&registry_path).unwrap();
    let explained = adopt_json(
        &env,
        &dir,
        &["rename", "old", "new", "--adopt", "--explain"],
    );
    assert_eq!(explained["recovery"], "refuse");
    assert_eq!(std::fs::read(registry_path).unwrap(), before);
    assert!(!env.home.path().join("state/tasks/claims/new.toml").exists());
    std::fs::remove_file(foreign.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
}

#[test]
fn adopt_explain_refuses_target_park_or_older_alias_before_registry_switch() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let new_id = old_id.replacen("old-", "new-", 1);
    adopt_json(&env, &dir, &["park", &new_id, "new host work"]);
    let before = std::fs::read(env.home.path().join("state/tasks/claims/new.toml")).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    assert_eq!(
        std::fs::read(env.home.path().join("state/tasks/claims/new.toml")).unwrap(),
        before
    );

    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"] = toml::toml! { older = "old" }.into();
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "older", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
}

#[test]
fn adopt_retired_target_refuses_before_writing_target_store() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    adopt_old_park(&env, &old_id);
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["aliases"] = toml::toml! { new = "old" }.into();
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let registry_before = std::fs::read(&registry_path).unwrap();
    let old_path = env.home.path().join("state/tasks/claims/old.toml");
    let old_before = std::fs::read(&old_path).unwrap();
    let target_path = env.home.path().join("state/tasks/claims/new.toml");
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    let refused = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(!target_path.exists());
    assert_eq!(std::fs::read(old_path).unwrap(), old_before);
    assert_eq!(std::fs::read(registry_path).unwrap(), registry_before);
}

#[test]
fn adopt_refuses_a_live_old_claim_with_missing_registered_root() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let state = env.home.path().join("state/tasks/claims");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("old.toml"),
        format!("[claims.\"{old_id}\"]\nowner = \"tester\"\nsession = \"old-session\"\nhost = \"host\"\nworktree = \"/old\"\nstarted = \"2099-01-01T00:00:00Z\"\nseen = \"2099-01-01T00:00:00Z\"\n"),
    )
    .unwrap();
    let registry_path = env.home.path().join("config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["old"] =
        toml::Value::String(env.home.path().join("missing").display().to_string());
    std::fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let out = adopt_cmd(&env, &dir)
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "claimed");
    assert!(String::from_utf8_lossy(&out.stderr).contains("old-session"));
    let claims = adopt_json(&env, &dir, &["claims"]);
    assert_eq!(claims["claims"][0]["id"], old_id);
    let old_store = state.join("old.toml");
    let text = std::fs::read_to_string(&old_store).unwrap();
    std::fs::write(&old_store, text.replace("2099-", "2020-")).unwrap();
    assert_eq!(
        adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
        "fresh"
    );
    assert!(!old_store.exists());
}

#[test]
fn adopt_explain_refuses_pending_inventory_or_mismatched_task_file() {
    let env = TestEnv::new();
    let (dir, old_id) = adopt_fixture(&env);
    let inventory_dir = env.home.path().join("state/tasks/rename");
    std::fs::create_dir_all(&inventory_dir).unwrap();
    std::fs::write(
        inventory_dir.join("old.toml"),
        format!("source = \"old\"\ntarget = \"new\"\nroot = \"{}\"\nconfig_from = \"{}\"\nconfig_to = \"{}\"\nentries = []\n", dir.display(), "0".repeat(64), "1".repeat(64)),
    )
    .unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
    std::fs::remove_file(inventory_dir.join("old.toml")).unwrap();
    let new_id = old_id.replacen("old-", "new-", 1);
    let path = dir.join(format!("tasks/{new_id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen(&new_id, &old_id, 1)).unwrap();
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "refuse"
    );
}

#[test]
fn adopt_two_hosts_after_primary_rename_and_optional_partial_init() {
    for partial_init in [false, true] {
        let host_a = TestEnv::new();
        let host_b = TestEnv::new();
        let parent = tempfile::tempdir().unwrap();
        let old_root = parent.path().join("old-root");
        std::fs::create_dir(&old_root).unwrap();
        adopt_json(&host_a, &old_root, &["init", "--prefix", "old"]);
        let old_id = id_of(adopt_json(
            &host_a,
            &old_root,
            &["add", "T", "--complexity", "mid"],
        ));
        adopt_json(&host_b, &old_root, &["init", "--prefix", "old"]);
        adopt_json(&host_b, &old_root, &["park", &old_id, "continue here"]);
        let b_store = host_b.home.path().join("state/tasks/claims/old.toml");
        let mut text = std::fs::read_to_string(&b_store).unwrap();
        text.push_str(&format!(
            "\n[escalations.\"{old_id}\"]\nlevel = \"high\"\nat = \"2026-09-27T00:00:00Z\"\nsession = \"old-session\"\n"
        ));
        std::fs::write(&b_store, text).unwrap();

        adopt_json(&host_a, &old_root, &["rename", "old", "new"]);
        let new_root = parent.path().join("new-root");
        std::fs::rename(&old_root, &new_root).unwrap();
        adopt_json(&host_a, &new_root, &["init", "--prefix", "new", "--force"]);
        if partial_init {
            adopt_json(&host_b, &new_root, &["init", "--prefix", "new", "--force"]);
        }
        let new_id = old_id.replacen("old-", "new-", 1);
        let config = new_root.join("tasks/.config.toml");
        let task = new_root.join(format!("tasks/{new_id}.md"));
        let config_before = std::fs::read(&config).unwrap();
        let task_before = std::fs::read(&task).unwrap();
        let adopted = adopt_json(&host_b, &new_root, &["rename", "old", "new", "--adopt"]);
        assert_eq!(adopted["mode"], "adopt");
        assert_eq!(std::fs::read(config).unwrap(), config_before);
        assert_eq!(std::fs::read(task).unwrap(), task_before);
        assert_eq!(
            adopt_json(&host_b, &new_root, &["show", &old_id])["task"]["id"],
            new_id
        );
        let registry: toml::Value = toml::from_str(
            &std::fs::read_to_string(host_b.home.path().join("config/tasks/projects.toml"))
                .unwrap(),
        )
        .unwrap();
        assert!(
            registry["projects"]
                .as_table()
                .unwrap()
                .get("old")
                .is_none()
        );
        assert_eq!(registry["aliases"]["old"].as_str(), Some("new"));
        assert_eq!(registry["projects"]["new"].as_str(), new_root.to_str());
        let b_new_store =
            std::fs::read_to_string(host_b.home.path().join("state/tasks/claims/new.toml"))
                .unwrap();
        assert!(
            b_new_store.contains(&format!("[parks.{new_id}]")),
            "{b_new_store}"
        );
        assert!(
            b_new_store.contains(&format!("[escalations.{new_id}]")),
            "{b_new_store}"
        );
        assert!(!b_store.exists());
    }
}

#[test]
fn adopt_replays_each_state_boundary_and_skips_store_without_parks() {
    for (boundary, expected) in [
        ("store", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("claims", "complete"),
    ] {
        let env = TestEnv::new();
        let (dir, old_id) = adopt_fixture(&env);
        adopt_old_park(&env, &old_id);
        let stopped = adopt_cmd(&env, &dir)
            .env("TASKS_RENAME_STOP_AFTER", boundary)
            .args(["rename", "old", "new", "--adopt"])
            .output()
            .unwrap();
        assert!(stopped.status.success(), "{boundary}: {stopped:?}");
        let explained = adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"],
        );
        assert_eq!(explained["recovery"], expected, "{boundary}: {explained}");
        if boundary == "registry" {
            let new_id = old_id.replacen("old-", "new-", 1);
            let started = adopt_cmd(&env, &dir)
                .env("CODEX_SESSION_ID", "post-registry")
                .args(["start", &new_id])
                .output()
                .unwrap();
            assert!(started.status.success(), "{started:?}");
        }
        assert_eq!(
            adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
            expected
        );
        assert_eq!(
            adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"])["recovery"],
            "complete"
        );
    }
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    let stopped = adopt_cmd(&env, &dir)
        .env("TASKS_RENAME_STOP_AFTER", "store")
        .args(["rename", "old", "new", "--adopt"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    assert_eq!(
        adopt_json(
            &env,
            &dir,
            &["rename", "old", "new", "--adopt", "--explain"]
        )["recovery"],
        "complete"
    );
}

#[test]
fn adopt_carries_an_orphaned_park_and_reports_it() {
    let env = TestEnv::new();
    let (dir, _) = adopt_fixture(&env);
    adopt_old_park(&env, "old-ffffff");
    let adopted = adopt_json(&env, &dir, &["rename", "old", "new", "--adopt"]);
    assert!(adopted["warnings"].to_string().contains("new-ffffff"));
    let parked = adopt_json(&env, &dir, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["id"], "new-ffffff");
    assert!(parked["tasks"][0]["status"].is_null());
}

fn rename_stop(env: &TestEnv, dir: &std::path::Path, old: &str, new: &str, stop: &str) {
    let output = env
        .raw(dir)
        .env("TASKS_RENAME_STOP_AFTER", stop)
        .args(["rename", old, new])
        .output()
        .unwrap();
    assert!(output.status.success(), "{stop}: {output:?}");
}

// Include directories as well as bytes: diagnosis must not create even an empty state dir.
fn rename_disk_state(
    env: &TestEnv,
    dir: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn collect(
        path: &std::path::Path,
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
    collect(&dir.join("tasks"), &mut state);
    collect(env.home.path(), &mut state);
    state
}

#[cfg(unix)]
fn set_registry_root(env: &TestEnv, prefix: &str, root: &std::path::Path) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    registry["projects"].as_table_mut().unwrap().insert(
        prefix.into(),
        toml::Value::String(root.display().to_string()),
    );
    std::fs::write(path, toml::to_string(&registry).unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn rename_recovers_through_noncanonical_registry_roots_before_and_after_registry_write() {
    for spelling in ["symlink", "dotdot"] {
        for (stop, verdict) in [("files", "resume_files"), ("registry", "resume_cleanup")] {
            let env = TestEnv::new();
            let outer = tempfile::tempdir().unwrap();
            let real = outer.path().join("real");
            std::fs::create_dir(&real).unwrap();
            env.json(&real, &["init", "--prefix", "dot"]);
            let id = id_of(env.json(&real, &["add", "One", "-p", "2"]));
            let root = if spelling == "symlink" {
                let link = outer.path().join("link");
                std::os::unix::fs::symlink(&real, &link).unwrap();
                link
            } else {
                real.join("..").join("real")
            };
            set_registry_root(&env, "dot", &root);
            let label = format!("{spelling} stop={stop}");

            assert_eq!(
                env.json(&real, &["rename", "dot", "dots", "--explain"])["recovery"],
                "fresh",
                "{label}"
            );
            rename_stop(&env, &real, "dot", "dots", stop);
            let moved = id.replacen("dot-", "dots-", 1);
            assert!(!real.join(format!("tasks/{id}.md")).exists(), "{label}");
            assert!(real.join(format!("tasks/{moved}.md")).is_file(), "{label}");
            let before = rename_disk_state(&env, &real);
            assert_eq!(
                env.json(&real, &["rename", "dot", "dots", "--explain"])["recovery"],
                verdict,
                "{label}"
            );
            assert_eq!(rename_disk_state(&env, &real), before, "{label}");
            assert_eq!(
                env.json(&real, &["rename", "dot", "dots"])["recovery"],
                verdict,
                "{label}"
            );
            assert_eq!(env.json(&real, &["show", &id])["task"]["id"], moved);
            assert_eq!(env.json(&real, &["show", &moved])["task"]["id"], moved);
            assert!(
                !env.home
                    .path()
                    .join(".local/state/tasks/rename/dot.toml")
                    .exists(),
                "{label}"
            );
            let registry: toml::Value = toml::from_str(
                &std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml"))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(registry["projects"]["dots"].as_str(), root.to_str());
            assert_eq!(registry["aliases"]["dot"].as_str(), Some("dots"));
        }
    }
}

#[cfg(unix)]
#[test]
fn rename_older_alias_replays_and_freezes_by_canonical_root() {
    let env = TestEnv::new();
    let outer = tempfile::tempdir().unwrap();
    let real = outer.path().join("real");
    let link = outer.path().join("link");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    env.json(&real, &["init", "--prefix", "dot"]);
    let id = id_of(env.json(&real, &["add", "One", "-p", "2"]));
    env.json(&real, &["rename", "dot", "dots"]);
    set_registry_root(&env, "dots", &link);
    rename_stop(&env, &real, "dot", "config", "registry");
    let moved = id.replacen("dot-", "config-", 1);
    assert!(
        !real
            .join("tasks")
            .join(id.replacen("dot-", "dots-", 1))
            .with_extension("md")
            .exists()
    );
    assert!(
        real.join("tasks")
            .join(&moved)
            .with_extension("md")
            .is_file()
    );

    assert_eq!(
        env.json(&real, &["rename", "dot", "config", "--explain"])["recovery"],
        "resume_cleanup"
    );
    let frozen = env.raw(&real).args(["unregister", "dot"]).output().unwrap();
    assert_eq!(frozen.status.code(), Some(1), "{frozen:?}");
    assert!(
        String::from_utf8_lossy(&frozen.stderr).contains("unfinished"),
        "{frozen:?}"
    );
    assert_eq!(
        env.json(&real, &["rename", "dot", "config"])["recovery"],
        "resume_cleanup"
    );
    assert_eq!(env.json(&real, &["show", &id])["task"]["id"], moved);
    assert_eq!(
        env.json(&real, &["show", &id.replacen("dot-", "dots-", 1)])["task"]["id"],
        moved
    );
    assert!(
        !env.home
            .path()
            .join(".local/state/tasks/rename/dots.toml")
            .exists()
    );
    let registry: toml::Value = toml::from_str(
        &std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(registry["projects"]["config"].as_str(), link.to_str());
    assert_eq!(registry["aliases"]["dot"].as_str(), Some("config"));
    assert_eq!(registry["aliases"]["dots"].as_str(), Some("config"));
}

#[test]
fn rename_explains_a_missing_foreign_registry_root_as_r8() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, false);
    rename_stop(&env, &dir, "dot", "dots", "config");
    let missing = dir.with_file_name("missing-rename-root");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let projects = registry["projects"].as_table_mut().unwrap();
    projects.remove("dot");
    projects.insert(
        "dots".into(),
        toml::Value::String(missing.display().to_string()),
    );
    std::fs::write(path, toml::to_string(&registry).unwrap()).unwrap();

    let before = rename_disk_state(&env, &dir);
    let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explain["recovery"], "refuse");
    let warning = explain["warnings"][0].as_str().unwrap();
    assert!(warning.starts_with("R8:"), "{warning}");
    assert!(warning.contains(missing.to_str().unwrap()), "{warning}");
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "config");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

#[test]
fn rename_every_mutation_boundary_resumes_inside_and_outside_git_including_empty_projects() {
    for in_git in [true, false] {
        for count in [0, 3] {
            for (stop, want) in [
                ("inventory", "resume_files"),
                ("file:0", "resume_files"),
                ("file:1", "resume_files"),
                ("file:2", "resume_files"),
                ("files", "resume_files"),
                ("config", "resume_registry"),
                ("registry", "resume_cleanup"),
                ("claims", "resume_cleanup"),
            ] {
                if count == 0 && stop.starts_with("file:") {
                    continue;
                }
                let mut env = TestEnv::new();
                let (dir, ids) = rename_fixture(&mut env, count, in_git);
                let inventory = env.home.path().join(".local/state/tasks/rename/dot.toml");
                let claims = env.claim_store("dot");
                std::fs::create_dir_all(claims.parent().unwrap()).unwrap();
                std::fs::write(&claims, "[claims]\n").unwrap();
                let label = format!("git={in_git} count={count} stop={stop}");
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    "fresh",
                    "{label}"
                );
                assert!(!inventory.exists());
                rename_stop(&env, &dir, "dot", "dots", stop);
                assert!(
                    inventory.is_file(),
                    "hook must leave a pending inventory: {label}"
                );
                let named = |prefix: &str| {
                    std::fs::read_dir(dir.join("tasks"))
                        .unwrap()
                        .map(|entry| entry.unwrap())
                        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
                        .count()
                };
                let (sources, destinations) = match stop {
                    "inventory" => (count, 0),
                    "file:0" => (3, 1),
                    "file:1" => (2, 2),
                    "file:2" => (1, 3),
                    _ => (0, count),
                };
                assert_eq!(
                    (named("dot-"), named("dots-")),
                    (sources, destinations),
                    "{label}"
                );
                let config: toml::Value =
                    toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
                assert_eq!(
                    config["prefix"].as_str().unwrap(),
                    if matches!(stop, "config" | "registry" | "claims") {
                        "dots"
                    } else {
                        "dot"
                    },
                    "{label}"
                );
                let registry: toml::Value = toml::from_str(
                    &env.read(&env.home.path().join(".config"), "tasks/projects.toml"),
                )
                .unwrap();
                if matches!(stop, "registry" | "claims") {
                    assert_eq!(
                        registry["projects"]["dots"].as_str(),
                        dir.to_str(),
                        "{label}"
                    );
                    assert_eq!(registry["aliases"]["dot"].as_str(), Some("dots"), "{label}");
                } else {
                    assert_eq!(
                        registry["projects"]["dot"].as_str(),
                        dir.to_str(),
                        "{label}"
                    );
                    assert!(registry["projects"].get("dots").is_none(), "{label}");
                }
                assert_eq!(claims.exists(), stop != "claims", "{label}");
                let before = rename_disk_state(&env, &dir);
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    want,
                    "{label}"
                );
                assert_eq!(
                    rename_disk_state(&env, &dir),
                    before,
                    "explain changed disk: {label}"
                );
                let resumed = env.json(&dir, &["rename", "dot", "dots"]);
                assert_eq!(resumed["recovery"], want, "{label}");
                if !in_git {
                    assert!(
                        resumed["warnings"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|w| w.as_str().unwrap().contains("outside git")),
                        "{label}"
                    );
                }
                assert_eq!((named("dot-"), named("dots-")), (0, count), "{label}");
                for id in ids {
                    let moved = id.replacen("dot-", "dots-", 1);
                    assert_eq!(
                        env.json(&dir, &["show", &id])["task"]["id"],
                        moved,
                        "{label}"
                    );
                    assert_eq!(
                        env.json(&dir, &["show", &moved])["task"]["id"],
                        moved,
                        "{label}"
                    );
                }
                assert!(!inventory.exists(), "{label}");
                assert!(!claims.exists(), "{label}");
                assert!(
                    claims.with_extension("lock").is_file(),
                    "old lock must survive: {label}"
                );
                assert_eq!(
                    env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                    "complete",
                    "{label}"
                );
                assert!(
                    env.check(&dir)["errors"].as_array().unwrap().is_empty(),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn rename_each_recovery_refusal_fires_independently_and_preserves_disk() {
    for rule in 1..=8 {
        let mut env = TestEnv::new();
        let (dir, ids) = rename_fixture(&mut env, 1, true);
        let foreign = env.init("foreign");
        rename_stop(&env, &dir, "dot", "dots", "file:0");
        let source = dir.join(format!("tasks/{}.md", ids[0]));
        let dest = dir.join(format!("tasks/{}.md", ids[0].replacen("dot-", "dots-", 1)));
        let registry_path = env.home.path().join(".config/tasks/projects.toml");
        let mut target = "dots";
        match rule {
            1 => target = "other",
            2 => std::fs::write(
                &source,
                std::fs::read_to_string(&source)
                    .unwrap()
                    .replace("title: T0", "title: Edited"),
            )
            .unwrap(),
            3 => std::fs::write(
                &dest,
                std::fs::read_to_string(&dest)
                    .unwrap()
                    .replace("title: T0", "title: Conflict"),
            )
            .unwrap(),
            4 => {
                std::fs::remove_file(&source).unwrap();
                std::fs::remove_file(&dest).unwrap();
            }
            5 => std::fs::write(dir.join("tasks/dot-ffffff.md"), "unrecorded task").unwrap(),
            6 => {
                let config = dir.join("tasks/.config.toml");
                std::fs::write(
                    &config,
                    format!(
                        "{}\n# unrelated edit\n",
                        std::fs::read_to_string(&config).unwrap()
                    ),
                )
                .unwrap();
            }
            7 | 8 => {
                let mut registry: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
                let projects = registry["projects"].as_table_mut().unwrap();
                projects.remove("foreign");
                projects.insert(
                    "dots".into(),
                    toml::Value::String(foreign.display().to_string()),
                );
                if rule == 8 {
                    projects.remove("dot");
                }
                std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        // R1's alternate target lock exists before taking the byte-for-byte snapshot.
        let held = hold_project_lock(&env, target);
        drop(held);
        let before = rename_disk_state(&env, &dir);
        let explain = env.json(&dir, &["rename", "dot", target, "--explain"]);
        assert_eq!(explain["recovery"], "refuse", "R{rule}: {explain}");
        let reason = explain["warnings"][0].as_str().unwrap();
        assert!(reason.starts_with(&format!("R{rule}:")), "{reason}");
        if rule == 3 {
            assert!(
                reason.contains(source.to_str().unwrap()),
                "both paths required: {reason}"
            );
            assert!(
                reason.contains(dest.to_str().unwrap()),
                "both paths required: {reason}"
            );
        }
        assert_eq!(
            rename_disk_state(&env, &dir),
            before,
            "R{rule}: explain changed disk"
        );
        let out = env
            .raw(&dir)
            .args(["rename", "dot", target])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "R{rule}: {out:?}");
        assert_eq!(
            err_kind(&out),
            if rule >= 7 { "config" } else { "validation" }
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(&format!("R{rule}:")),
            "{out:?}"
        );
        assert_eq!(
            rename_disk_state(&env, &dir),
            before,
            "R{rule}: refusal changed disk"
        );
    }
}

#[test]
fn rename_same_name_refusal_names_the_obstacle() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, true);
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dot"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("source and target prefixes are both"),
        "{detail}"
    );
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_resumes_require_claim_and_worktree_authorization_at_every_stage() {
    for (stop, verdict) in [
        ("inventory", "resume_files"),
        ("files", "resume_files"),
        ("config", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("claims", "resume_cleanup"),
    ] {
        for obstacle in ["old claim", "new claim", "worktree"] {
            let mut env = TestEnv::new();
            let (dir, ids) = rename_fixture(&mut env, 1, true);
            rename_stop(&env, &dir, "dot", "dots", stop);
            let other = tempfile::tempdir().unwrap();
            let claim_prefix = if obstacle == "old claim" {
                "dot"
            } else {
                "dots"
            };
            if obstacle == "worktree" {
                git(
                    &dir,
                    &[
                        "worktree",
                        "add",
                        "-q",
                        "--detach",
                        other.path().to_str().unwrap(),
                    ],
                );
            } else {
                let id = ids[0].replacen("dot-", &format!("{claim_prefix}-"), 1);
                write_claim(&env, claim_prefix, &id, "agent-a", true);
            }
            let before = rename_disk_state(&env, &dir);
            // Authorization is separate from classification, including after P5/P6.
            assert_eq!(
                env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"],
                verdict,
                "{stop}: {obstacle}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before);
            assert_eq!(
                env.fail(&dir, &["rename", "dot", "dots"]),
                if obstacle == "worktree" {
                    "validation"
                } else {
                    "claimed"
                },
                "{stop}: {obstacle}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {obstacle}");
            if obstacle == "worktree" {
                git(
                    &dir,
                    &["worktree", "remove", other.path().to_str().unwrap()],
                );
            } else {
                assert!(
                    env.claim_store(claim_prefix).is_file(),
                    "live store must survive refused cleanup"
                );
                std::fs::remove_file(env.claim_store(claim_prefix)).unwrap();
            }
            assert_eq!(
                env.json(&dir, &["rename", "dot", "dots"])["recovery"],
                verdict
            );
            assert!(
                !env.home
                    .path()
                    .join(".local/state/tasks/rename/dot.toml")
                    .exists()
            );
        }
    }
}

#[test]
fn rename_pending_explain_does_not_acquire_any_existing_lock() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, true);
    rename_stop(&env, &dir, "dot", "dots", "registry");
    write_claim(
        &env,
        "dots",
        &ids[0].replacen("dot-", "dots-", 1),
        "agent-a",
        true,
    );
    let old_lock = hold_project_lock(&env, "dot");
    let new_lock = hold_project_lock(&env, "dots");
    let registry_lock = File::options()
        .write(true)
        .open(env.home.path().join(".config/tasks/projects.lock"))
        .unwrap();
    registry_lock.lock().unwrap();
    let before = rename_disk_state(&env, &dir);
    let child = env
        .raw(&dir)
        .args(["rename", "dot", "dots", "--explain"])
        .spawn()
        .unwrap();
    let out = reap(child, Duration::from_secs(3));
    drop((old_lock, new_lock, registry_lock));
    let out = out.expect("explain waited on a lock");
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["recovery"], "resume_cleanup");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

#[test]
fn rename_freeze_covers_all_writers_feedback_and_registry_names() {
    for stop in ["inventory", "config", "registry", "claims"] {
        let (env, dir, reporter) = feedback_env();
        let id = id_of(env.json(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                "Original",
                "--category",
                "gap",
                "--new",
            ],
        ));
        let editor_ran = dir.join("editor-ran");
        let editor = editor_script(&dir, &format!("touch '{}'; exit 99", editor_ran.display()));
        rename_stop(&env, &dir, "tasks", "tracker", stop);
        let current = if stop == "inventory" {
            id.clone()
        } else {
            id.replacen("tasks-", "tracker-", 1)
        };
        let before = rename_disk_state(&env, &dir);
        let commands: &[&[&str]] = &[
            &["add", "Late", "-p", "2"],
            &["add", "Late", "--source", "same-source"],
            &["start", &current],
            &["done", &current],
            &["drop", &current, "Reason"],
            &["block", &current, "Reason"],
            &["note", &current, "Late note"],
            &["edit", &current, "--title", "Late title"],
            &["edit", &current],
            &["dep", &current, "--on", &current],
        ];
        for args in commands {
            let out = env
                .raw(&dir)
                .env("EDITOR", &editor)
                .args(*args)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
            assert_eq!(err_kind(&out), "validation", "{stop}: {args:?}: {out:?}");
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                "{stop}: {args:?}: {out:?}"
            );
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
        }
        assert!(
            !editor_ran.exists(),
            "a frozen edit must not launch the editor"
        );
        for args in [
            vec![
                "feedback",
                "--project",
                "tasks",
                "Late",
                "--category",
                "gap",
                "--new",
            ],
            vec![
                "feedback",
                "--project",
                "tasks",
                "Late",
                "--category",
                "gap",
                "--recur",
                &current,
            ],
        ] {
            let out = env.raw(&reporter).args(&args).output().unwrap();
            assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
            assert_eq!(
                err_kind(&out),
                if stop == "config" {
                    "config"
                } else {
                    "validation"
                }
            );
            if stop != "config" {
                assert!(
                    String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                    "{out:?}"
                );
            }
            assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
        }
        let unrelated = tempfile::tempdir().unwrap();
        for prefix in ["tasks", "tracker"] {
            for (root, args) in [
                (dir.as_path(), vec!["unregister", prefix]),
                (dir.as_path(), vec!["init", "--prefix", prefix, "--force"]),
                (
                    unrelated.path(),
                    vec!["init", "--prefix", prefix, "--force"],
                ),
            ] {
                let out = env.raw(root).args(&args).output().unwrap();
                assert_eq!(out.status.code(), Some(1), "{stop}: {args:?}: {out:?}");
                assert!(
                    String::from_utf8_lossy(&out.stderr).contains("unfinished"),
                    "{stop}: {args:?}: {out:?}"
                );
                assert!(!unrelated.path().join("tasks").exists());
                assert_eq!(rename_disk_state(&env, &dir), before, "{stop}: {args:?}");
            }
        }
        assert_eq!(env.json(&dir, &["show", &current])["task"]["id"], current);
        assert_eq!(
            env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
            1
        );
    }
}

#[test]
fn rename_freeze_is_rechecked_when_an_editor_returns() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, false);
    let original = env.read(&dir, &format!("tasks/{}.md", ids[0]));
    let editor = editor_script(
        &dir,
        &format!(
            "sed -i 's/^title: T0$/title: Edited/' \"$1\"\nTASKS_RENAME_STOP_AFTER=inventory '{}' rename dot dots > /dev/null",
            assert_cmd::cargo::cargo_bin("tasks").display()
        ),
    );
    let out = env
        .raw(&dir)
        .env("EDITOR", editor)
        .args(["edit", &ids[0]])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(err_kind(&out), "validation");
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("unfinished") && detail.contains(".edit.md"),
        "{detail}"
    );
    assert_eq!(env.read(&dir, &format!("tasks/{}.md", ids[0])), original);
    assert!(std::fs::read_dir(dir.join("tasks")).unwrap().any(|entry| {
        let path = entry.unwrap().path();
        path.to_string_lossy().ends_with(".edit.md")
            && std::fs::read_to_string(path)
                .unwrap()
                .contains("title: Edited")
    }));
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots"])["recovery"],
        "resume_files"
    );
}

#[test]
fn rename_rewrites_own_refs_and_preserves_hand_written_body_notes_and_foreign_bytes() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let foreign = env.init("ops");
    let foreign_id = id_of(env.json(&foreign, &["add", "Foreign"]));
    let parent = "---\nid: dot-a00001\ntitle: Parent\nstatus: todo\npriority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\ndepends: []\ntags: []\n---\n\nParent body.\n";
    let child = format!(
        "---\nid: dot-a00002\ntitle: Child\nstatus: todo\npriority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\nparent: dot-a00001\ndepends: [dot-a00001, {foreign_id}]\ntags: [dot-a00001]\nsource: dot-a00001\n---\n\n\nBody   with  odd    spacing mentions dot-a00001.\n\n\n## Notes\n\n- 2026-09-01T00:00:00Z (keith):   dot-a00001 stays in prose\n\n"
    );
    std::fs::write(dir.join("tasks/dot-a00001.md"), parent).unwrap();
    std::fs::write(dir.join("tasks/dot-a00002.md"), &child).unwrap();
    let inbound = id_of(env.json(&foreign, &["add", "Inbound", "--depends", "dot-a00002"]));
    let foreign_before = env.read(&foreign, &format!("tasks/{inbound}.md"));
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(
        env.read(&dir, "tasks/dots-a00001.md"),
        parent
            .replacen("id: dot-a00001", "id: dots-a00001", 1)
            .replace("priority: 2", "priority: \"2\"")
    );
    let expected = child
        .replacen("id: dot-a00002", "id: dots-a00002", 1)
        .replacen("parent: dot-a00001", "parent: dots-a00001", 1)
        .replacen("depends: [dot-a00001", "depends: [dots-a00001", 1)
        .replace("priority: 2", "priority: \"2\"");
    assert_eq!(env.read(&dir, "tasks/dots-a00002.md"), expected);
    assert_eq!(
        env.read(&foreign, &format!("tasks/{inbound}.md")),
        foreign_before
    );
    let shown = env.json(&dir, &["show", "dot-a00002"]);
    assert_eq!(shown["task"]["parent"], "dots-a00001");
    assert_eq!(
        shown["task"]["depends"],
        serde_json::json!(["dots-a00001", foreign_id])
    );
    assert!(env.check(&dir)["errors"].as_array().unwrap().is_empty());
    let check = env.check(&foreign);
    assert!(
        check["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "retired_prefix")
    );
    env.json(&foreign, &["edit", &inbound, "--title", "Unrelated edit"]);
    assert_eq!(
        env.json(&foreign, &["show", &inbound])["task"]["depends"][0],
        "dot-a00002"
    );
}

#[test]
fn rename_fresh_destination_collision_names_the_obstacle_and_keeps_both_files() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, 1, false);
    let dest = dir.join(format!("tasks/{}.md", ids[0].replacen("dot-", "dots-", 1)));
    std::fs::write(&dest, "unrelated destination").unwrap();
    let original = env.read(&dir, &format!("tasks/{}.md", ids[0]));
    let out = env
        .raw(&dir)
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_kind(&out), "validation");
    let detail = String::from_utf8_lossy(&out.stderr);
    assert!(
        detail.contains("destination") && detail.contains("dots"),
        "{detail}"
    );
    assert_eq!(env.read(&dir, &format!("tasks/{}.md", ids[0])), original);
    assert_eq!(
        std::fs::read_to_string(dest).unwrap(),
        "unrelated destination"
    );
    assert!(!env.home.path().join(".local/state/tasks/rename").exists());
}

#[test]
fn rename_explains_a_missing_config_and_refuses_recovery_without_writes() {
    let mut env = TestEnv::new();
    let (dir, _) = rename_fixture(&mut env, 1, false);
    rename_stop(&env, &dir, "dot", "dots", "inventory");
    std::fs::remove_file(dir.join("tasks/.config.toml")).unwrap();
    let before = rename_disk_state(&env, &dir);
    let explain = env.json(&dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(explain["recovery"], "refuse");
    assert!(explain["warnings"][0].as_str().unwrap().starts_with("R6:"));
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
    assert_eq!(rename_disk_state(&env, &dir), before);
}

/// A task backdated far enough to be in the default sample pool.
fn old_task(env: &TestEnv, dir: &std::path::Path, title: &str, status_args: &[&str]) -> String {
    let mut args = vec!["add", title, "-p", "2"];
    args.extend_from_slice(status_args);
    let id = id_of(env.json(dir, &args));
    stamp(dir, &id, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    id
}

fn sampled_ids(v: &serde_json::Value) -> Vec<String> {
    v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn sample_draws_only_from_the_curable_pool() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let idea = old_task(&env, &dir, "Idea", &["--status", "idea"]);
    let todo = old_task(&env, &dir, "Todo", &[]);
    let blocked = id_of(env.json(&dir, &["add", "Blocked", "-p", "2"]));
    env.json(&dir, &["block", &blocked, "waiting"]);
    stamp(
        &dir,
        &blocked,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    let doing = old_task(&env, &dir, "Doing", &[]);
    env.json(&dir, &["start", &doing]);
    stamp(&dir, &doing, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let done = old_task(&env, &dir, "Done", &[]);
    env.json(&dir, &["done", &done]);
    stamp(&dir, &done, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let dropped = old_task(&env, &dir, "Dropped", &[]);
    env.json(&dir, &["drop", &dropped, "not needed"]);
    stamp(
        &dir,
        &dropped,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    let recent = id_of(env.json(&dir, &["add", "Recent", "-p", "2"]));
    let live = old_task(&env, &dir, "Live claim", &[]);
    write_claim(&env, "sci", &live, "other-session", true);
    // a curate note whose proposal the human has not answered keeps the task out; a
    // curate note without one, or any later note, does not
    let pending = old_task(&env, &dir, "Pending", &[]);
    env.json(
        &dir,
        &[
            "note",
            &pending,
            "curate: stale; body tightened; proposal: drop, landed in abc1234",
        ],
    );
    stamp(
        &dir,
        &pending,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    let kept = old_task(&env, &dir, "Kept", &[]);
    env.json(&dir, &["note", &kept, "curate: keep; nothing to change"]);
    stamp(&dir, &kept, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
    let answered = old_task(&env, &dir, "Answered", &[]);
    env.json(
        &dir,
        &["note", &answered, "curate: duplicate; proposal: merge"],
    );
    env.json(&dir, &["note", &answered, "declined: they differ in scope"]);
    stamp(
        &dir,
        &answered,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    // last: `note` prunes dead claims from the store, so a stale claim written earlier
    // would be swept before the draw
    let stale = old_task(&env, &dir, "Stale claim", &[]);
    write_claim(&env, "sci", &stale, "gone-session", false);

    let v = env.json(&dir, &["sample", "-n", "10", "--seed", "1"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![
        idea.clone(),
        todo.clone(),
        blocked.clone(),
        stale.clone(),
        kept.clone(),
        answered.clone(),
    ];
    expected.sort();
    assert_eq!(ids, expected, "{v}");
    for absent in [&doing, &done, &dropped, &recent, &live, &pending] {
        assert!(!ids.contains(absent), "{absent} must not be drawn: {v}");
    }
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&live) && w.contains("omitted") && w.contains("other-session")
        }),
        "{v}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap() == format!("{pending} pending: drop, landed in abc1234")),
        "{v}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("pool holds 6")),
        "{v}"
    );
    // rows are list rows: the same keys, with the claim reported on the stale one
    let row = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == stale)
        .unwrap();
    assert_eq!(row["claim"]["live"], false, "{row}");
    assert!(row.get("child_count").is_some(), "{row}");
}

#[test]
fn sample_treats_a_scope_proposal_like_a_curate_proposal() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let pending = old_task(&env, &dir, "Pending", &["--status", "idea"]);
    env.json(
        &dir,
        &[
            "note",
            &pending,
            "scope: drop; landed in abc1234; proposal: drop, abc1234",
        ],
    );
    stamp(
        &dir,
        &pending,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    let readmitted = old_task(&env, &dir, "Readmitted", &["--status", "idea"]);
    env.json(
        &dir,
        &["note", &readmitted, "scope: drop; proposal: drop, dup of x"],
    );
    env.json(&dir, &["note", &readmitted, "declined: keep it"]);
    stamp(
        &dir,
        &readmitted,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );
    let briefed = old_task(&env, &dir, "Briefed", &["--status", "idea"]);
    env.json(
        &dir,
        &[
            "note",
            &briefed,
            "scope: briefed; brief: docs/notes/x-brief.md",
        ],
    );
    stamp(
        &dir,
        &briefed,
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );

    let v = env.json(&dir, &["sample", "-n", "10", "--seed", "1"]);
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![readmitted.clone(), briefed.clone()];
    expected.sort();
    assert_eq!(
        ids, expected,
        "a scope verdict without a proposal stays in the pool"
    );
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap() == format!("{pending} pending: drop, abc1234")),
        "{warnings:?}"
    );
}

#[test]
fn sample_older_than_zero_admits_fresh_tasks_and_goals_stay_in() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let goal = id_of(env.json(&dir, &["add", "Goal", "-p", "2"]));
    let child = id_of(env.json(&dir, &["add", "Child", "-p", "2", "--parent", &goal]));
    // clock skew: a record stamped in the future is "within" every positive window
    let future = id_of(env.json(&dir, &["add", "Future", "-p", "2"]));
    stamp(
        &dir,
        &future,
        "2026-01-01T00:00:00Z",
        "2999-01-01T00:00:00Z",
    );

    let v = env.json(&dir, &["sample", "-n", "10"]);
    assert_eq!(sampled_ids(&v), Vec::<String>::new(), "{v}");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("not updated in 7 days")),
        "{v}"
    );

    let v = env.json(&dir, &["sample", "-n", "10", "--older-than", "0d"]);
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("any age")),
        "{v}"
    );
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![goal, child, future];
    expected.sort();
    assert_eq!(ids, expected, "{v}");
}

#[test]
fn sample_bounds_older_than_at_the_cli() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    old_task(&env, &dir, "T", &[]);
    for bad in [
        "7",
        "36501d",
        "5215w",
        "10000000d",
        "18446744073709551615d",
        "0w",
    ] {
        let err = env.usage(&dir, &["sample", "--older-than", bad]);
        assert!(err.contains("--older-than") && err.contains(bad), "{err}");
    }
    // Zero has one spelling; the other zeros name it, and no other rejection does.
    for zero in ["0", "0w"] {
        let err = env.usage(&dir, &["sample", "--older-than", zero]);
        assert!(err.contains("0d skips the age check"), "{err}");
    }
    let err = env.usage(&dir, &["sample", "--older-than", "7"]);
    assert!(!err.contains("0d skips the age check"), "{err}");
    // a leading minus reads as a flag unless attached to the option
    let err = env.usage(&dir, &["sample", "--older-than=-1d"]);
    assert!(err.contains("--older-than") && err.contains("-1d"), "{err}");
    let v = env.json(&dir, &["sample", "--older-than", "36500d"]);
    assert!(sampled_ids(&v).is_empty(), "{v}");
}

/// Pins the seeded draw so a change of generator or generator width is caught. The
/// expected indices were computed with fastrand 2.5.0 by the same algorithm
/// (`Rng::with_seed(seed)`, then `rng.u64(i..n)` for each slot) over a pool of twenty
/// ids sorted lexically; on a 32-bit target `Rng::usize` would give different values.
#[test]
fn sample_seeded_draw_is_a_known_answer() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for n in 0..20u32 {
        let id = format!("sci-{n:06x}");
        std::fs::write(
            dir.join(format!("tasks/{id}.md")),
            format!(
                "---\nid: {id}\ntitle: T{n}\nstatus: todo\npriority: 2\n\
                 created: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
        )
        .unwrap();
    }
    let v = env.json(&dir, &["sample", "--seed", "7"]);
    assert_eq!(
        sampled_ids(&v),
        vec!["sci-00000f", "sci-000005", "sci-00000e"],
        "{v}"
    );
    let v = env.json(&dir, &["sample", "--seed", "8"]);
    assert_eq!(
        sampled_ids(&v),
        vec!["sci-000005", "sci-000002", "sci-000000"],
        "{v}"
    );
    let v = env.json(&dir, &["sample", "-n", "5", "--seed", "7"]);
    assert_eq!(
        sampled_ids(&v),
        vec![
            "sci-00000f",
            "sci-000005",
            "sci-00000e",
            "sci-00000b",
            "sci-000001"
        ],
        "{v}"
    );
}

#[test]
fn sample_is_reproducible_by_seed_and_defaults_to_three() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for n in 0..20 {
        old_task(&env, &dir, &format!("T{n}"), &[]);
    }
    let a = sampled_ids(&env.json(&dir, &["sample", "--seed", "7"]));
    let b = sampled_ids(&env.json(&dir, &["sample", "--seed", "7"]));
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
    let c = sampled_ids(&env.json(&dir, &["sample", "--seed", "8"]));
    assert_ne!(
        a, c,
        "two seeds over twenty tasks should not draw the same three in order"
    );
    let five = sampled_ids(&env.json(&dir, &["sample", "-n", "5", "--seed", "7"]));
    assert_eq!(five.len(), 5);
    let mut unique = five.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5, "without replacement");
}

#[test]
fn sample_of_an_empty_pool_is_empty_with_a_warning() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let v = env.json(&dir, &["sample"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("pool holds 0")),
        "{v}"
    );
    let out = env.cmd(&dir).args(["--pretty", "sample"]).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn sample_scopes_like_the_other_read_commands() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();
    let s = old_task(&env, &sci, "S", &[]);
    let f = old_task(&env, &fam, "F", &[]);

    let v = env.json(&sci, &["sample", "-n", "10", "--project", "fam"]);
    assert_eq!(sampled_ids(&v), vec![f.clone()], "{v}");

    let v = env.json(
        nowhere.path(),
        &["sample", "-n", "10", "--all-projects", "--seed", "1"],
    );
    let mut ids = sampled_ids(&v);
    ids.sort();
    let mut expected = vec![s.clone(), f.clone()];
    expected.sort();
    assert_eq!(ids, expected, "{v}");

    let out = env
        .cmd(&sci)
        .args(["sample", "--project", "fam", "--all-projects"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "clap conflict");

    let out = env
        .cmd(&sci)
        .args(["--pretty", "sample", "-n", "10", "--project", "fam"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&f) && text.contains("F"), "{text}");
}

#[test]
fn a_recurrence_reopens_straight_to_doing_and_parks_only_once_open() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    env.json(&sci, &["start", &id]);
    env.json(&sci, &["done", &id]);

    // Seed a distinctly older anchor so that "the anchor moved" is provable. Timestamps
    // have second precision, so comparing two same-second stamps would prove nothing.
    seed_anchor(&sci, &id, "2020-03-04T05:06:07Z");

    // An overdue recurrence reopens directly.
    let v = env.json(&sci, &["start", &id]);
    assert_eq!(v["id"], id);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "doing");

    // Running early re-anchors from that close, replacing the seeded value.
    env.json(&sci, &["done", &id]);
    let after = env.json(&sci, &["show", &id])["task"]["last_done"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(after, "2020-03-04T05:06:07Z", "the anchor was replaced");
    assert!(after.starts_with("20"), "{after}");
    assert!(
        after.as_str() > "2020-03-04T05:06:07Z",
        "the anchor moved forward: {after}"
    );

    // A closed recurrence cannot be parked; reopen first (spec §4.8).
    assert_eq!(
        env.fail(&sci, &["park", &id, "ask the user"]),
        "invalid_transition"
    );
    // Freshly completed: not yet due, but early reopening is allowed too.
    env.json(&sci, &["start", &id]);
    env.json(&sci, &["park", &id, "ask the user", "--waiting-on", "user"]);
    assert_eq!(env.json(&sci, &["list", "--parked"])["tasks"][0]["id"], id);

    // An ordinary closed task still cannot reopen straight to doing.
    let plain = id_of(env.json(&sci, &["add", "One off"]));
    env.json(&sci, &["done", &plain]);
    assert_eq!(env.fail(&sci, &["start", &plain]), "invalid_transition");
}

#[test]
fn a_due_recurrence_is_ready_and_an_anchored_one_is_not() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // not due: anchored at this moment by its own completion
    let anchored = id_of(env.json(&sci, &["add", "Fresh sweep", "-p", "1", "--every", "30d"]));
    env.json(&sci, &["start", &anchored]);
    env.json(&sci, &["done", &anchored]);

    // due: closed with no anchor, then given a cadence (spec §3.2)
    let due = id_of(env.json(&sci, &["add", "Overdue sweep", "-p", "0"]));
    env.json(&sci, &["done", &due]);
    env.json(&sci, &["edit", &due, "--every", "30d"]);

    let ready = env.json(&sci, &["ready"]);
    let ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![due.as_str()], "{ready}");
    assert_eq!(ready["tasks"][0]["status"], "done", "the record is closed");
    assert_eq!(env.json(&sci, &["next"])["next"]["task"]["id"], due);

    // A due record satisfies a dependent, because it is closed (spec §4.5).
    let dependent = id_of(env.json(&sci, &["add", "Follows", "-p", "2", "--depends", &due]));
    let ids: Vec<String> = env.json(&sci, &["ready"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&dependent), "{ids:?}");

    // Once started it is open, and holds the dependent (spec §4.5).
    env.json(&sci, &["start", &due]);
    let ids: Vec<String> = env.json(&sci, &["ready"])["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!ids.contains(&dependent), "{ids:?}");
}

#[test]
fn a_due_recurrence_keeps_priority_order_and_respects_another_checkouts_claim() {
    let mut env = TestEnv::new();
    let a = env.init("sci");
    let b = env.init_forced("sci");
    let due = id_of(env.json(&a, &["add", "Due", "-p", "2"]));
    env.json(&a, &["done", &due]);
    env.json(&a, &["edit", &due, "--every", "30d"]);
    let urgent = id_of(env.json(&a, &["add", "Urgent", "-p", "1"]));
    let ready = env.json(&a, &["ready"]);
    assert_eq!(ready["tasks"][0]["id"], urgent);
    assert_eq!(ready["tasks"][1]["id"], due);
    std::fs::copy(
        a.join(format!("tasks/{due}.md")),
        b.join(format!("tasks/{due}.md")),
    )
    .unwrap();
    as_agent(&env, &b, "other")
        .args(["start", &due])
        .assert()
        .success();
    let ready = env.json(&a, &["ready"]);
    assert_eq!(ready["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(ready["tasks"][0]["id"], urgent);
    assert!(
        ready["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&due) && w.contains("claimed") && w.contains("--force")
        }),
        "{ready}"
    );
}

#[test]
fn the_periodic_object_is_the_json_contract() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // No cadence: periodic is omitted everywhere.
    let plain = id_of(env.json(&sci, &["add", "One off"]));
    assert_eq!(env.json(&sci, &["show", &plain]).get("periodic"), None);
    assert_eq!(env.json(&sci, &["list"])["tasks"][0].get("periodic"), None);
    // close it so it cannot compete for the head of `ready` below
    env.json(&sci, &["done", &plain]);

    // open with a cadence: present, but no pending recurrence (spec §4.1)
    let open = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    let v = env.json(&sci, &["show", &open]);
    assert_eq!(v["periodic"]["every"], "30d");
    assert!(v["periodic"].get("last_done").is_none());
    assert!(v["periodic"].get("due").is_none());
    assert_eq!(v["periodic"]["due_now"], false);

    // closed and anchored: a computable date, not yet due
    env.json(&sci, &["start", &open]);
    env.json(&sci, &["done", &open]);
    let v = env.json(&sci, &["show", &open]);
    assert!(v["periodic"]["last_done"].is_string());
    assert!(v["periodic"]["due"].as_str().unwrap().ends_with('Z'));
    assert_eq!(v["periodic"]["due_now"], false);

    // Closed and unanchored: due now, with no computable date -- an absent `due`
    // alone cannot express (spec §5.4)
    let due = id_of(env.json(&sci, &["add", "Overdue"]));
    env.json(&sci, &["done", &due]);
    env.json(&sci, &["edit", &due, "--every", "30d"]);
    let v = env.json(&sci, &["show", &due]);
    assert!(v["periodic"].get("due").is_none());
    assert_eq!(v["periodic"]["due_now"], true);

    // the same object rides on list rows and on next; `due` is the only ready task
    let rows = env.json(&sci, &["ready"]);
    assert_eq!(rows["tasks"].as_array().unwrap().len(), 1, "{rows}");
    assert_eq!(rows["tasks"][0]["id"], due);
    assert_eq!(rows["tasks"][0]["periodic"]["due_now"], true);
    assert_eq!(
        env.json(&sci, &["next"])["next"]["periodic"]["due_now"],
        true
    );
    // Reopening retains history but suspends dueness; dropping ends it.
    env.json(&sci, &["start", &open]);
    let reopened = env.json(&sci, &["show", &open]);
    assert!(reopened["periodic"]["last_done"].is_string());
    assert!(reopened["periodic"]["due"].is_null());
    assert_eq!(reopened["periodic"]["due_now"], false);
    env.json(&sci, &["drop", &open]);
    let dropped = env.json(&sci, &["show", &open]);
    assert_eq!(dropped["periodic"], reopened["periodic"]);
}

#[test]
fn deferred_object_rides_show_list_next_and_parked_rows() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Later", "--defer", "2099-01-02"]));
    let plain = id_of(env.json(&sci, &["add", "Now"]));
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["deferred"]["until"], "2099-01-02");
    assert_eq!(v["deferred"]["due"], false);
    assert!(env.json(&sci, &["show", &plain])["deferred"].is_null());
    let list = env.json(&sci, &["list"]);
    let row = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == id)
        .unwrap();
    assert_eq!(row["deferred"]["until"], "2099-01-02");
    assert_eq!(row["deferred"]["due"], false);

    // A due record, through the editor (§8).
    let due = id_of(env.json(&sci, &["add", "Due", "--status", "idea"]));
    let editor = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\ndefer: 2026-01-01/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &due])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let v = env.json(&sci, &["show", &due]);
    assert_eq!(v["deferred"]["until"], "2026-01-01");
    assert_eq!(v["deferred"]["due"], true);

    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "later"])
        .assert()
        .success();
    let parked = env.json(&sci, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["id"], id);
    assert_eq!(parked["tasks"][0]["deferred"]["until"], "2099-01-02");
}

#[test]
fn a_due_row_shows_its_cadence_and_due_date() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let due = id_of(env.json(&sci, &["add", "Overdue sweep"]));
    env.json(&sci, &["done", &due]);
    env.json(&sci, &["edit", &due, "--every", "30d"]);

    let text = env.pretty(&sci, &["ready"]);
    assert!(text.contains("done"), "the status column is honest: {text}");
    assert!(text.contains("every 30d, due now"), "{text}");
    assert!(text.is_ascii(), "pretty output is ASCII only: {text}");

    let shown = env.pretty(&sci, &["show", &due]);
    assert!(shown.contains("every: 30d"), "{shown}");
    assert!(shown.contains("due: now"), "{shown}");

    // An anchored, not-yet-due recurrence is not in ready at all, and its show page
    // carries a real date.
    let anchored = id_of(env.json(&sci, &["add", "Fresh sweep", "--every", "2w"]));
    env.json(&sci, &["start", &anchored]);
    env.json(&sci, &["done", &anchored]);
    let shown = env.pretty(&sci, &["show", &anchored]);
    assert!(shown.contains("every: 2w"), "{shown}");
    assert!(shown.contains("due: 2"), "{shown}");
    assert!(
        !env.pretty(&sci, &["ready"]).contains(&anchored),
        "not due yet"
    );
}

#[test]
fn list_periodic_shows_the_series_in_due_order() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let open = id_of(env.json(&sci, &["add", "Open sweep", "--every", "90d"]));
    let soon = id_of(env.json(&sci, &["add", "Soon", "--every", "7d"]));
    env.json(&sci, &["start", &soon]);
    env.json(&sci, &["done", &soon]);
    let later = id_of(env.json(&sci, &["add", "Later", "--every", "90d"]));
    env.json(&sci, &["start", &later]);
    env.json(&sci, &["done", &later]);
    let overdue = id_of(env.json(&sci, &["add", "Overdue"]));
    env.json(&sci, &["done", &overdue]);
    env.json(&sci, &["edit", &overdue, "--every", "30d"]);
    let plain = id_of(env.json(&sci, &["add", "Not periodic"]));

    let rows = env.json(&sci, &["list", "--periodic"]);
    let ids: Vec<&str> = rows["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    // unanchored due-now first, then anchored by due ascending, then everything else
    assert_eq!(
        ids,
        vec![
            overdue.as_str(),
            soon.as_str(),
            later.as_str(),
            open.as_str()
        ]
    );
    assert!(!ids.contains(&plain.as_str()), "{ids:?}");

    // It bypasses the default open-status filter: most of a healthy series is closed.
    assert_eq!(rows["tasks"][1]["status"], "done");

    let text = env.pretty(&sci, &["list", "--periodic"]);
    assert!(
        text.contains("now"),
        "the unanchored due cell reads now: {text}"
    );
    assert!(
        text.contains(" -  "),
        "an open row's due cell is a dash: {text}"
    );

    // Ordinary filters intersect, and validation survives: a bad parent still errors.
    env.json(&sci, &["edit", &overdue, "--tag", "sweep"]);
    let tagged = env.json(&sci, &["list", "--periodic", "--tag", "sweep"]);
    assert_eq!(tagged["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(
        env.fail(&sci, &["list", "--periodic", "--parent", "sci-ffffff"]),
        "task_not_found"
    );
    // An explicit --status still narrows.
    let closed = env.json(&sci, &["list", "--periodic", "--status", "done"]);
    assert_eq!(closed["tasks"].as_array().unwrap().len(), 3);

    // The ordering flags conflict, as they do for --parked.
    for flag in [
        ["--sort", "created"].as_slice(),
        ["--reverse"].as_slice(),
        ["--parked"].as_slice(),
    ] {
        let mut args = vec!["list", "--periodic"];
        args.extend_from_slice(flag);
        let out = env.cmd(&sci).args(&args).output().unwrap();
        assert!(
            !out.status.success(),
            "{flag:?} must conflict with --periodic"
        );
    }
    // Periodic filtering keeps the ordinary dependency diagnostics.
    let path = sci.join(format!("tasks/{open}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, text.replace("depends: []", "depends: [sci-ffffff]")).unwrap();
    let rows = env.json(&sci, &["list", "--periodic"]);
    assert!(
        rows["warnings"].as_array().unwrap().iter().any(|w| {
            let w = w.as_str().unwrap();
            w.contains(&open) && w.contains("sci-ffffff") && w.contains("unreachable")
        }),
        "{rows}"
    );
}

#[test]
fn prime_counts_what_is_scheduled_but_not_yet_due() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let empty = env.json(&sci, &["prime"]);
    assert_eq!(empty["periodic"]["scheduled"], 0);
    assert!(empty["periodic"]["next_due"].is_null());
    assert!(!env.pretty(&sci, &["prime"]).contains("periodic:"));

    let soon = id_of(env.json(&sci, &["add", "Soon", "--every", "7d"]));
    env.json(&sci, &["start", &soon]);
    env.json(&sci, &["done", &soon]);
    let later = id_of(env.json(&sci, &["add", "Later", "--every", "90d"]));
    env.json(&sci, &["start", &later]);
    env.json(&sci, &["done", &later]);
    // A due record is work, not schedule: it is in ready, and out of the count.
    let due = id_of(env.json(&sci, &["add", "Overdue"]));
    env.json(&sci, &["done", &due]);
    env.json(&sci, &["edit", &due, "--every", "30d"]);

    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["periodic"]["scheduled"], 2);
    let next = v["periodic"]["next_due"].as_str().unwrap().to_string();
    let soon_due = env.json(&sci, &["show", &soon])["periodic"]["due"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(next, soon_due, "the earliest of the scheduled");
    assert!(v["periodic"].get("in_days").is_none(), "pretty-only: {v}");

    let text = env.pretty(&sci, &["prime"]);
    assert!(text.contains("periodic: 2 scheduled, next due "), "{text}");
    // The figure itself is pinned by the unit test above; here only its presence and form.
    assert!(text.contains("(in ") && text.contains("d)"), "{text}");
    assert!(text.is_ascii(), "{text}");
}

#[test]
fn a_goal_is_never_periodic_in_either_direction() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let goal = id_of(env.json(&sci, &["add", "Goal"]));
    let child = id_of(env.json(&sci, &["add", "Piece", "--parent", &goal]));

    // A task that already has children cannot be given a cadence: `is_ready` excludes
    // goals, so it could never fire (spec §4.7).
    assert_eq!(
        env.fail(&sci, &["edit", &goal, "--every", "30d"]),
        "validation"
    );
    assert!(
        !env.read(&sci, &format!("tasks/{goal}.md"))
            .contains("every:")
    );

    // And a recurrence cannot acquire children, through add or through edit.
    let sweep = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    assert_eq!(
        env.fail(&sci, &["edit", &child, "--parent", &sweep]),
        "validation"
    );
    assert_eq!(
        env.fail(&sci, &["add", "Under a sweep", "--parent", &sweep]),
        "validation"
    );

    // The editor save goes through the same choke point.
    let before = env.read(&sci, &format!("tasks/{child}.md"));
    let reparent = editor_script(
        &sci,
        &format!("sed -i 's/^parent: .*/parent: {sweep}/' \"$1\""),
    );
    let out = env
        .cmd(&sci)
        .env("EDITOR", &reparent)
        .args(["edit", &child])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(error["error"]["detail"].as_str().unwrap().contains(&sweep));
    assert_eq!(env.read(&sci, &format!("tasks/{child}.md")), before);

    let cadence = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\nevery: 30d/' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .env("EDITOR", &cadence)
        .args(["edit", &goal])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(error["error"]["detail"].as_str().unwrap().contains(&goal));
    assert!(
        !env.read(&sci, &format!("tasks/{goal}.md"))
            .contains("every:")
    );

    // Dropping the cadence lets it be a goal again.
    env.json(&sci, &["edit", &sweep, "--no-every"]);
    env.json(&sci, &["add", "Under a former sweep", "--parent", &sweep]);
}

#[test]
fn check_reports_what_parsing_cannot_see_about_cadences() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // A periodic child reopened under a goal that has since closed is intentional, and
    // must not be warned about (spec §6).
    let goal = id_of(env.json(&sci, &["add", "Goal"]));
    let sweep = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d", "--parent", &goal]));
    env.json(&sci, &["start", &sweep]);
    env.json(&sci, &["done", &sweep]);
    env.json(&sci, &["done", &goal]);
    env.json(&sci, &["start", &sweep]);
    let v = env.check(&sci);
    let kinds: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["kind"].as_str().unwrap())
        .collect();
    assert!(!kinds.contains(&"open_child_of_closed_parent"), "{v}");

    // An ordinary open child of a closed parent is still warned about.
    let plain = id_of(env.json(&sci, &["add", "Plain", "--parent", &goal]));
    let v = env.check(&sci);
    let warned: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "open_child_of_closed_parent")
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(warned, vec![plain.as_str()], "{v}");

    // A combination only a merge or a hand edit can produce: a cadence on a task that has
    // children. Both writes go straight to disk, bypassing write_task.
    let orphan = id_of(env.json(&sci, &["add", "Hand edited"]));
    let kid = id_of(env.json(&sci, &["add", "Kid"]));
    let path = sci.join(format!("tasks/{orphan}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("priority: 2\n", "priority: 2\nevery: 30d\n"),
    )
    .unwrap();
    let kid_path = sci.join(format!("tasks/{kid}.md"));
    let kid_text = std::fs::read_to_string(&kid_path).unwrap();
    std::fs::write(
        &kid_path,
        kid_text.replace("depends: []\n", &format!("depends: []\nparent: {orphan}\n")),
    )
    .unwrap();

    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "check exits 1 on errors");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<&str> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"periodic_goal"), "{v}");

    // An anchor with no cadence does not load at all, so it arrives as a parse finding
    // rather than one of its own (spec §6).
    let bare = id_of(env.json(&sci, &["add", "Bare anchor"]));
    let bare_path = sci.join(format!("tasks/{bare}.md"));
    let bare_text = std::fs::read_to_string(&bare_path).unwrap();
    std::fs::write(
        &bare_path,
        bare_text.replace("updated: ", "last_done: 2026-01-01T00:00:00Z\nupdated: "),
    )
    .unwrap();
    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let parse_findings: Vec<&str> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "parse")
        .map(|e| e["file"].as_str().unwrap())
        .collect();
    assert!(
        parse_findings.iter().any(|file| file.contains(&bare)),
        "{v}"
    );
    // An unrepresentable anchor plus cadence also arrives through the parse channel.
    let overflow = id_of(env.json(&sci, &["add", "Overflow", "--every", "1d"]));
    seed_anchor(&sci, &overflow, "9999-12-31T00:00:00Z");
    let out = env.cmd(&sci).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "parse" && e["file"].as_str().unwrap().contains(&overflow)),
        "{v}"
    );
}

#[test]
fn complexity_is_set_cleared_listed_and_completed() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Rate me", "-p", "2", "--complexity", "mid"]));
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "mid");
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"][0]["complexity"], "mid");
    let text = std::fs::read_to_string(sci.join(format!("tasks/{id}.md"))).unwrap();
    assert!(text.contains("\ncomplexity: mid\n"), "{text}");

    let pretty = env.pretty(&sci, &["list"]);
    assert!(pretty.contains(" mid "), "{pretty}");

    env.json(&sci, &["edit", &id, "--complexity", "high"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["complexity"], "high");
    env.json(&sci, &["edit", &id, "--no-complexity"]);
    assert!(env.json(&sci, &["show", &id])["task"]["complexity"].is_null());
    let pretty = env.pretty(&sci, &["list"]);
    assert!(
        pretty.contains(" -    "),
        "unassessed shows a dash: {pretty}"
    );

    let err = env.usage(&sci, &["add", "Bad", "--complexity", "medium"]);
    assert!(
        err.contains("--complexity") && err.contains("medium"),
        "{err}"
    );
    let err = env.usage(&sci, &["edit", &id, "--complexity", "5"]);
    assert!(err.contains("--complexity") && err.contains("'5'"), "{err}");
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--complexity", "low", "--no-complexity"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "clap conflict is a usage error");

    let values = env.complete_values(&sci, "bash", 4, &["tasks", "add", "T", "--complexity", ""]);
    assert_eq!(values, vec!["low", "mid", "high"]);
    let values = env.complete_values(&sci, "zsh", 4, &["tasks", "edit", &id, "--complexity", ""]);
    assert!(
        values.iter().any(|value| value.starts_with("high")),
        "{values:?}"
    );
    let values = env.complete_values(&sci, "bash", 3, &["tasks", "ready", "--max-complexity", ""]);
    assert_eq!(values, vec!["low", "mid", "high"]);
    let values = env.complete_values(&sci, "bash", 3, &["tasks", "next", "--max-complexity", ""]);
    assert_eq!(values, vec!["low", "mid", "high"]);
}

#[test]
fn ready_and_next_hide_above_cutoff_and_unassessed_with_counts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "1", "--complexity", "low"]));
    let mid = id_of(env.json(&sci, &["add", "Mid", "-p", "2", "--complexity", "mid"]));
    let high = id_of(env.json(&sci, &["add", "High", "-p", "0", "--complexity", "high"]));
    let none = id_of(env.json(&sci, &["add", "Unassessed", "-p", "0"]));

    let v = env.json(&sci, &["ready"]);
    assert_eq!(
        v["tasks"].as_array().unwrap().len(),
        4,
        "no cutoff, nothing hidden"
    );
    assert!(v["warnings"].as_array().unwrap().is_empty());

    let v = env.json(&sci, &["ready", "--max-complexity", "mid"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![low.as_str(), mid.as_str()],
        "priority order kept: {v}"
    );
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(
        warnings,
        vec![
            "max-complexity mid: 1 above cutoff hidden",
            "max-complexity mid: 1 unassessed hidden"
        ]
    );

    let v = env.json(&sci, &["ready", "--max-complexity", "low", "-n", "1"]);
    assert_eq!(v["tasks"][0]["id"], low);
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(
        warnings,
        vec![
            "max-complexity low: 2 above cutoff hidden",
            "max-complexity low: 1 unassessed hidden"
        ]
    );

    let v = env.json(&sci, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], low, "{v}");

    let v = env.json(&sci, &["ready", "--max-complexity", "high"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 3);
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(warnings, vec!["max-complexity high: 1 unassessed hidden"]);

    // The cutoff composes with --size and --parallel, and its counts are the cutoff's
    // alone: the size and parallel filters run after it and are not counted (spec §4.1).
    env.json(&sci, &["edit", &low, "--size", "s", "--parallel"]);
    env.json(&sci, &["edit", &mid, "--size", "m"]);
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--size", "s"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(
        warnings,
        vec![
            "max-complexity mid: 1 above cutoff hidden",
            "max-complexity mid: 1 unassessed hidden"
        ]
    );
    let v = env.json(&sci, &["ready", "--max-complexity", "mid", "--parallel"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let v = env.json(
        &sci,
        &[
            "ready",
            "--max-complexity",
            "mid",
            "--parallel",
            "--size",
            "m",
        ],
    );
    assert!(v["tasks"].as_array().unwrap().is_empty());

    // Across projects the counts are one total for the scope, not one line per project.
    let fam = env.init("fam");
    env.json(&fam, &["add", "Fam unassessed", "-p", "2"]);
    env.json(
        &fam,
        &["add", "Fam high", "-p", "2", "--complexity", "high"],
    );
    let v = env.json(
        &sci,
        &["ready", "--all-projects", "--max-complexity", "mid"],
    );
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2, "{v}");
    let warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(
        warnings,
        vec![
            "max-complexity mid: 2 above cutoff hidden",
            "max-complexity mid: 2 unassessed hidden"
        ]
    );
    let v = env.json(&fam, &["next", "--all-projects", "--max-complexity", "mid"]);
    assert_eq!(
        v["next"]["task"]["id"], low,
        "priority order across the scope"
    );

    let err = env.usage(&sci, &["ready", "--max-complexity", "huge"]);
    assert!(
        err.contains("--max-complexity") && err.contains("huge"),
        "{err}"
    );
    let _ = (high, none);
}

#[test]
fn the_cutoff_variable_drives_ready_next_and_prime_and_the_flag_wins() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "2", "--complexity", "low"]));
    let high = id_of(env.json(&sci, &["add", "High", "-p", "0", "--complexity", "high"]));
    let with_env = |args: &[&str], value: &str| -> serde_json::Value {
        let out = env
            .cmd(&sci)
            .env("TASKS_MAX_COMPLEXITY", value)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let v = with_env(&["ready"], "low");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(v["tasks"][0]["id"], low);
    let v = with_env(&["next"], "low");
    assert_eq!(v["next"]["task"]["id"], low);
    let v = with_env(&["prime"], "low");
    assert_eq!(v["ready"].as_array().unwrap().len(), 1, "{v}");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "max-complexity low: 1 above cutoff hidden")
    );
    let v = with_env(&["ready"], "");
    assert_eq!(
        v["tasks"].as_array().unwrap().len(),
        2,
        "empty means no cutoff"
    );

    // The flag wins over the variable.
    let v = with_env(&["ready", "--max-complexity", "high"], "low");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2);
    // An invalid variable fails even when the flag is given.
    let out = env
        .cmd(&sci)
        .env("TASKS_MAX_COMPLEXITY", "huge")
        .args(["ready", "--max-complexity", "high"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let out = env
        .cmd(&sci)
        .env("TASKS_MAX_COMPLEXITY", "huge")
        .args(["prime"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let _ = high;
}

#[test]
fn next_skips_parked_work_above_the_cutoff_and_falls_through() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let parked = id_of(env.json(
        &sci,
        &["add", "Parked high", "-p", "0", "--complexity", "high"],
    ));
    let low = id_of(env.json(&sci, &["add", "Low", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &parked, "continue"])
        .assert()
        .success();

    let v = env.json(&sci, &["next"]);
    assert_eq!(
        v["next"]["task"]["id"], parked,
        "parked work waiting on the agent comes first"
    );
    let v = env.json(&sci, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], low, "{v}");
    let v = env.json(&sci, &["next", "--max-complexity", "low"]);
    assert_eq!(v["next"]["task"]["id"], low);
    env.json(&sci, &["edit", &low, "--complexity", "high"]);
    let v = env.json(&sci, &["next", "--max-complexity", "low"]);
    assert!(v["next"].is_null(), "{v}");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "max-complexity low: 2 above cutoff hidden"),
        "{v}"
    );
}

#[test]
fn prime_closeout_is_filtered_by_the_goals_own_rating() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2", "-b", "committed"]));
    let child = id_of(env.json(&sci, &["add", "Child", "--parent", &goal]));
    env.json(&sci, &["done", &child, "landed"]);
    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["closeout"][0]["id"], goal);
    let out = env
        .cmd(&sci)
        .env("TASKS_MAX_COMPLEXITY", "mid")
        .arg("prime")
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["closeout"].as_array().unwrap().is_empty(),
        "unrated goal hidden: {v}"
    );
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "max-complexity mid: 1 unassessed hidden"),
        "{v}"
    );
    env.json(&sci, &["edit", &goal, "--complexity", "low"]);
    let out = env
        .cmd(&sci)
        .env("TASKS_MAX_COMPLEXITY", "mid")
        .arg("prime")
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["closeout"][0]["id"], goal);
}

#[test]
fn capability_park_validates_the_level_and_records_the_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "low"]));

    assert_eq!(
        env.fail(&sci, &["park", &id, "stuck", "--complexity", "high"]),
        "validation"
    );
    assert_eq!(
        env.fail(&sci, &["park", &id, "stuck", "--reason", "capability"]),
        "validation"
    );
    let err = env.usage(
        &sci,
        &[
            "park",
            &id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "medium",
        ],
    );
    assert!(
        err.contains("--complexity") && err.contains("medium"),
        "{err}"
    );
    // Never below the effective rating.
    env.json(&sci, &["edit", &id, "--complexity", "mid"]);
    assert_eq!(
        env.fail(
            &sci,
            &[
                "park",
                &id,
                "stuck",
                "--reason",
                "capability",
                "--complexity",
                "low"
            ]
        ),
        "validation"
    );

    // Under a cutoff the level must exceed it.
    let under = |value: &str, level: &str| {
        as_agent(&env, &sci, "agent-a")
            .env("TASKS_MAX_COMPLEXITY", value)
            .args([
                "park",
                &id,
                "needs a decision the plan leaves open",
                "--reason",
                "capability",
                "--complexity",
                level,
            ])
            .output()
            .unwrap()
    };
    let out = under("mid", "mid");
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = under("high", "high");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--waiting-on user"));

    // Under a high cutoff, omitting --complexity still reports that no level above the
    // cutoff exists -- not the generic "needs --complexity <level>" -- and names the
    // --waiting-on user route.
    let out = as_agent(&env, &sci, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "high")
        .args([
            "park",
            &id,
            "needs a decision the plan leaves open",
            "--reason",
            "capability",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--waiting-on user"));

    let out = under("mid", "high");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "high");
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(
        v["escalation"]["session"], "agent-a",
        "TASKS_SESSION is written verbatim, per park_reason_rides_the_entry_the_note_and_every_park_view"
    );
    assert_eq!(v["park"]["reason"], "capability");
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"][0]["escalation"]["level"], "high");
    let pretty = env.pretty(&sci, &["show", &id]);
    assert!(pretty.contains("# escalation"), "{pretty}");
    assert!(pretty.contains("agent, capability"), "{pretty}");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("[escalations."), "{store}");

    // A record already high accepts high under a lower or absent cutoff (the retry shape).
    let out = under("mid", "high");
    assert!(out.status.success());
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "again",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
}

#[test]
fn capability_park_waiting_on_the_user_records_no_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "high"]));
    as_agent(&env, &sci, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "high")
        .args([
            "park",
            &id,
            "decompose this",
            "--reason",
            "capability",
            "--waiting-on",
            "user",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert!(v["escalation"].is_null(), "{v}");
    assert_eq!(v["park"]["waiting_on"], "user");
    let v = env.json(&sci, &["ready"]);
    assert!(
        v["tasks"].as_array().unwrap().is_empty(),
        "parked on the user is omitted"
    );
    // --complexity is optional here but still never lowers.
    assert_eq!(
        env.fail(
            &sci,
            &[
                "park",
                &id,
                "x",
                "--reason",
                "capability",
                "--waiting-on",
                "user",
                "--complexity",
                "low"
            ]
        ),
        "validation"
    );
}

#[test]
fn capability_park_fails_loudly_when_the_store_write_fails_and_the_rerun_succeeds() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--complexity", "low"]));
    let plain = id_of(env.json(&sci, &["add", "P", "-p", "2"]));
    let store = env.claim_store("sci");
    // Create the store directory and the lock so only the atomic temp file fails.
    as_agent(&env, &sci, "agent-a")
        .args(["park", &plain, "warm up"])
        .assert()
        .success();
    use std::os::unix::fs::PermissionsExt;
    let state_dir = store.parent().unwrap();
    let original = std::fs::metadata(state_dir).unwrap().permissions();
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let capability = as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .output()
        .unwrap();
    let ordinary = as_agent(&env, &sci, "agent-a")
        .args(["park", &plain, "later", "--reason", "session"])
        .output()
        .unwrap();
    std::fs::set_permissions(state_dir, original).unwrap();

    assert_eq!(
        capability.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&capability.stdout)
    );
    let error: serde_json::Value = serde_json::from_slice(&capability.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    let message = error["error"]["detail"].as_str().unwrap();
    assert!(
        message.contains("rerun the same `tasks park` command"),
        "{message}"
    );
    assert!(
        message.contains(&format!("escalation of {id} to high")),
        "{message}"
    );
    assert!(
        ordinary.status.success(),
        "an ordinary park keeps the warning contract"
    );

    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "high", "the record write landed");
    assert!(v["escalation"].is_null(), "the store write did not");

    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &id,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    let v = env.json(&sci, &["show", &id]);
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(v["park"]["reason"], "capability");
}

/// A second checkout of `sci` holding a copy of `id`'s record as it was before the escalation.
fn second_checkout(_env: &mut TestEnv, first: &std::path::Path, id: &str) -> std::path::PathBuf {
    let second = tempfile::tempdir().unwrap();
    let path = second.path().canonicalize().unwrap();
    std::fs::create_dir_all(path.join("tasks")).unwrap();
    std::fs::write(path.join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();
    std::fs::copy(
        first.join(format!("tasks/{id}.md")),
        path.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    std::mem::forget(second);
    path
}

#[test]
fn an_escalation_governs_every_checkout_through_resume_and_reparking() {
    let mut env = TestEnv::new();
    // A is the registered root and stays stale throughout; B and C are unregistered
    // checkouts holding the record as it was before the escalation.
    let a = env.init("sci");
    let id = id_of(env.json(&a, &["add", "T", "-p", "2", "--complexity", "low"]));
    let b = second_checkout(&mut env, &a, &id);
    let c = second_checkout(&mut env, &a, &id);

    // The escalation is made from B, so the registered root never sees the raised record.
    as_agent(&env, &b, "agent-b")
        .env("TASKS_MAX_COMPLEXITY", "mid")
        .args([
            "park",
            &id,
            "interacting behaviour outside the assessed scope",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["task"]["complexity"], "low", "A's record is stale");
    assert_eq!(v["escalation"]["level"], "high", "the store is shared");

    for dir in [&a, &b, &c] {
        let v = env.json(dir, &["next", "--max-complexity", "mid"]);
        assert!(v["next"].is_null(), "{}: {v}", dir.display());
        let v = env.json(dir, &["ready", "--max-complexity", "mid"]);
        assert!(v["tasks"].as_array().unwrap().is_empty());
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w == "max-complexity mid: 1 above cutoff hidden"),
            "{v}"
        );
    }
    let v = env.json(&a, &["next"]);
    assert_eq!(
        v["next"]["task"]["id"], id,
        "an unrestricted session still gets it"
    );

    // --all-projects reads the registered root's stale record with the shared store.
    let v = env.json(&c, &["next", "--all-projects", "--max-complexity", "mid"]);
    assert!(v["next"].is_null(), "{v}");
    let v = env.json(&b, &["ready", "--all-projects", "--max-complexity", "mid"]);
    assert!(v["tasks"].as_array().unwrap().is_empty());
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "max-complexity mid: 1 above cutoff hidden"),
        "{v}"
    );

    // A stronger session resumes in C and parks again for the session.
    as_agent(&env, &c, "agent-c")
        .args(["start", &id])
        .assert()
        .success();
    as_agent(&env, &c, "agent-c")
        .args(["park", &id, "half done", "--reason", "session"])
        .assert()
        .success();
    for dir in [&a, &b, &c] {
        let v = env.json(dir, &["next", "--max-complexity", "mid"]);
        assert!(
            v["next"].is_null(),
            "{}: escalation must survive start and re-park: {v}",
            dir.display()
        );
    }
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["escalation"]["level"], "high");
    assert_eq!(v["park"]["reason"], "session");

    // A stale checkout cannot lower it through another capability park.
    let out = as_agent(&env, &a, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "low")
        .args([
            "park",
            &id,
            "x",
            "--reason",
            "capability",
            "--complexity",
            "mid",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("effective rating high"));
    as_agent(&env, &a, "agent-a")
        .env("TASKS_MAX_COMPLEXITY", "low")
        .args([
            "park",
            &id,
            "x",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    let v = env.json(&a, &["show", &id]);
    assert_eq!(v["park"]["reason"], "capability", "A's park replaced C's");

    // A combined status-and-rating edit whose task write fails must leave the escalation
    // and the previous park standing: the acquire path saves the store first and rolls
    // back on failure. C must be `todo` so the edit is a transition into `doing`.
    as_agent(&env, &c, "agent-c")
        .args(["edit", &id, "--status", "todo"])
        .assert()
        .success();
    let v = env.json(&c, &["show", &id]);
    assert_eq!(
        v["task"]["status"], "todo",
        "precondition: the edit below acquires"
    );
    assert!(v["claim"].is_null(), "precondition: no claim to displace");
    assert_eq!(
        v["park"]["reason"], "capability",
        "precondition: a park to restore"
    );
    use std::os::unix::fs::PermissionsExt;
    let c_tasks = c.join("tasks");
    let original = std::fs::metadata(&c_tasks).unwrap().permissions();
    std::fs::set_permissions(&c_tasks, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &c, "agent-c")
        .args(["edit", &id, "--status", "doing", "--complexity", "mid"])
        .output()
        .unwrap();
    std::fs::set_permissions(&c_tasks, original).unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let v = env.json(&a, &["show", &id]);
    assert_eq!(
        v["escalation"]["level"], "high",
        "rolled back with the park: {v}"
    );
    assert_eq!(
        v["park"]["reason"], "capability",
        "the previous park is back: {v}"
    );
    assert!(
        v["claim"].is_null(),
        "the acquired claim was rolled back: {v}"
    );
    let v = env.json(&c, &["show", &id]);
    assert_eq!(
        v["task"]["complexity"], "low",
        "the record write never landed"
    );

    // Explicit reassessment from C clears it, with a warning naming what was overridden.
    let v = env.json(&c, &["edit", &id, "--complexity", "mid"]);
    let warning = v["warnings"][0].as_str().unwrap();
    assert!(
        warning.contains("cleared the escalation of") && warning.contains("to high"),
        "{warning}"
    );
    let v = env.json(&c, &["show", &id]);
    assert!(v["escalation"].is_null(), "{v}");
    let v = env.json(&c, &["next", "--max-complexity", "mid"]);
    assert_eq!(v["next"]["task"]["id"], id, "C offers it at its own mid");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(!store.contains("[escalations."), "{store}");
}

#[test]
fn reassessment_clears_on_every_edit_shape_and_closing_clears_too() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let escalate = |id: &str| {
        as_agent(&env, &sci, "agent-a")
            .args([
                "park",
                id,
                "stuck",
                "--reason",
                "capability",
                "--complexity",
                "high",
            ])
            .assert()
            .success();
    };
    let has_escalation = |id: &str| !env.json(&sci, &["show", id])["escalation"].is_null();

    let same_status = id_of(env.json(&sci, &["add", "Same", "-p", "2", "--complexity", "low"]));
    escalate(&same_status);
    env.json(&sci, &["note", &same_status, "still there"]);
    env.json(&sci, &["edit", &same_status, "--size", "s"]);
    assert!(
        has_escalation(&same_status),
        "notes and unrelated edits leave it"
    );
    env.json(
        &sci,
        &[
            "edit",
            &same_status,
            "--status",
            "todo",
            "--complexity",
            "low",
        ],
    );
    assert!(
        !has_escalation(&same_status),
        "a same-status edit still persists the clear"
    );

    let no_level = id_of(env.json(&sci, &["add", "Clear", "-p", "2", "--complexity", "low"]));
    escalate(&no_level);
    env.json(&sci, &["edit", &no_level, "--no-complexity"]);
    assert!(!has_escalation(&no_level));

    let transition = id_of(env.json(
        &sci,
        &["add", "Scoped", "--status", "idea", "--complexity", "low"],
    ));
    escalate(&transition);
    env.json(
        &sci,
        &[
            "edit",
            &transition,
            "--status",
            "todo",
            "-p",
            "2",
            "--complexity",
            "mid",
        ],
    );
    assert!(!has_escalation(&transition));

    let closed = id_of(env.json(&sci, &["add", "Done", "-p", "2", "--complexity", "low"]));
    escalate(&closed);
    env.json(&sci, &["done", &closed, "landed"]);
    assert!(!has_escalation(&closed));
    let dropped = id_of(env.json(&sci, &["add", "Dropped", "-p", "2", "--complexity", "low"]));
    escalate(&dropped);
    env.json(&sci, &["drop", &dropped, "no longer needed"]);
    assert!(!has_escalation(&dropped));
}

/// A failed store save during a reassessment must not claim the clear happened, and the
/// recovery it names must be the actual retry -- not the generic status-cleanup hint.
/// Covers both branches of `save` that remove an escalation: `ClearEscalation` (a
/// same-status edit) and `Release` (a status change away from `doing`).
#[test]
fn reassessment_names_the_retry_when_the_store_save_fails() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let escalate = |id: &str| {
        as_agent(&env, &sci, "agent-a")
            .args([
                "park",
                id,
                "stuck",
                "--reason",
                "capability",
                "--complexity",
                "high",
            ])
            .assert()
            .success();
    };
    let has_escalation = |id: &str| !env.json(&sci, &["show", id])["escalation"].is_null();
    let warnings_of = |out: &std::process::Output| -> Vec<String> {
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_str().unwrap().to_string())
            .collect()
    };
    use std::os::unix::fs::PermissionsExt;
    let state_dir = env.claim_store("sci").parent().unwrap().to_path_buf();
    let chmod = |mode: u32| {
        std::fs::set_permissions(&state_dir, std::fs::Permissions::from_mode(mode)).unwrap();
    };

    // ClearEscalation: a same-status edit that only reassesses the rating.
    let same_status = id_of(env.json(&sci, &["add", "Same", "-p", "2", "--complexity", "low"]));
    escalate(&same_status);
    let original = std::fs::metadata(&state_dir).unwrap().permissions();
    chmod(0o500);
    let out = as_agent(&env, &sci, "agent-a")
        .args(["edit", &same_status, "--complexity", "mid"])
        .output()
        .unwrap();
    std::fs::set_permissions(&state_dir, original.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let warnings = warnings_of(&out);
    assert!(
        !warnings
            .iter()
            .any(|w| w.contains("cleared the escalation")),
        "{warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains(&format!(
            "rerun `tasks edit {same_status} --complexity mid`"
        ))),
        "{warnings:?}"
    );
    assert!(
        has_escalation(&same_status),
        "the store write failed, so the escalation still stands"
    );

    let v = env.json(&sci, &["edit", &same_status, "--complexity", "mid"]);
    let retry_warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert!(
        retry_warnings
            .iter()
            .any(|w| w.contains("cleared the escalation")),
        "the retry clears it once the store can be written again: {retry_warnings:?}"
    );
    assert!(!has_escalation(&same_status));

    // Release: a status change away from `doing`, combined with a rating.
    let releasing = id_of(env.json(
        &sci,
        &["add", "Releasing", "-p", "2", "--complexity", "low"],
    ));
    escalate(&releasing);
    as_agent(&env, &sci, "agent-a")
        .args(["start", &releasing])
        .assert()
        .success();
    chmod(0o500);
    let out = as_agent(&env, &sci, "agent-a")
        .args([
            "edit",
            &releasing,
            "--status",
            "todo",
            "--complexity",
            "mid",
        ])
        .output()
        .unwrap();
    std::fs::set_permissions(&state_dir, original).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let warnings = warnings_of(&out);
    assert!(
        !warnings
            .iter()
            .any(|w| w.contains("cleared the escalation")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains(&format!("rerun `tasks edit {releasing} --complexity mid`"))),
        "{warnings:?}"
    );
    assert!(
        has_escalation(&releasing),
        "the store write failed, so the escalation still stands"
    );

    let v = env.json(&sci, &["edit", &releasing, "--complexity", "mid"]);
    let retry_warnings: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert!(
        retry_warnings
            .iter()
            .any(|w| w.contains("cleared the escalation")),
        "the retry clears it once the store can be written again: {retry_warnings:?}"
    );
    assert!(!has_escalation(&releasing));
}

#[test]
fn check_warns_on_an_open_plan_step_without_a_rating() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    write_doc(&sci, "docs/plans/x.md", "# x\n\n### Task 1: do it\n");
    let step = id_of(env.json(
        &sci,
        &[
            "add",
            "Step",
            "-p",
            "2",
            "--plan",
            "x",
            "--step",
            "Task 1: do it",
        ],
    ));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    let v = env.check(&sci);
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w["kind"] == "unrated_step" && w["id"] == step),
        "{v}"
    );
    assert!(
        !warnings.iter().any(|w| w["id"] == plain),
        "missing complexity elsewhere is not a finding"
    );
    env.json(&sci, &["edit", &step, "--complexity", "low"]);
    let v = env.check(&sci);
    assert!(
        !v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unrated_step"),
        "{v}"
    );
    env.json(&sci, &["edit", &step, "--no-complexity"]);
    env.json(&sci, &["done", &step, "landed"]);
    let v = env.check(&sci);
    assert!(
        !v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unrated_step"),
        "closed steps are silent: {v}"
    );
}

#[test]
fn shelve_writes_the_status_and_the_note_and_unshelve_returns_to_idea() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2", "--size", "s"]));
    let v = env.json(&sci, &["shelve", &id, "when profiles have two consumers"]);
    assert_eq!(v["id"], id);
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "shelved");
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(
        notes.last().unwrap()["text"],
        "shelved: when profiles have two consumers"
    );
    assert_eq!(shown["task"]["size"], "s", "fields survive the shelf");

    env.json(&sci, &["unshelve", &id]);
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "idea");
    let notes = shown["task"]["notes"].as_array().unwrap();
    assert_eq!(notes.last().unwrap()["text"], "unshelved");
}

#[test]
fn shelve_requires_a_wake_condition_and_unshelve_requires_shelved() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let out = env.cmd(&sci).args(["shelve", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "usage error, nothing written");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    let err = error_of(&env, &sci, &["unshelve", &id]);
    assert_eq!(err["error"]["kind"], "invalid_transition");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("unshelve requires shelved")
    );
}

#[test]
fn shelve_follows_the_claim_rules_and_clears_a_park_and_its_escalation() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let held = id_of(env.json(&sci, &["add", "Held", "-p", "2"]));
    write_claim(&env, "sci", &held, "agent-z", true);
    let err = as_agent(&env, &sci, "agent-a")
        .args(["shelve", &held, "later"])
        .output()
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&err.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "claimed");

    let parked = id_of(env.json(&sci, &["add", "Parked", "-p", "2", "--complexity", "low"]));
    as_agent(&env, &sci, "agent-a")
        .args(["start", &parked])
        .assert()
        .success();
    as_agent(&env, &sci, "agent-a")
        .args([
            "park",
            &parked,
            "stuck",
            "--reason",
            "capability",
            "--complexity",
            "high",
        ])
        .assert()
        .success();
    assert!(!env.json(&sci, &["show", &parked])["escalation"].is_null());

    let v: serde_json::Value = {
        let out = as_agent(&env, &sci, "agent-a")
            .args(["shelve", &parked, "after the rack lands"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with(&format!("cleared the escalation of {parked} to high"))),
        "{warnings:?}"
    );
    let shown = env.json(&sci, &["show", &parked]);
    assert!(shown["park"].is_null(), "shelving clears the park entry");
    assert!(shown["escalation"].is_null(), "and its escalation");
    assert!(shown["claim"].is_null(), "and releases the claim");
}

#[test]
fn shelve_refuses_a_goal_with_unshelved_open_descendants() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal", "-p", "2"]));
    let a = id_of(env.json(&sci, &["add", "A", "--parent", &goal]));
    let b = id_of(env.json(&sci, &["add", "B", "--parent", &goal]));
    let err = error_of(&env, &sci, &["shelve", &goal, "someday"]);
    assert_eq!(err["error"]["kind"], "open_descendants");
    let detail = err["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(&a) && detail.contains(&b), "{detail}");
    assert_eq!(env.json(&sci, &["show", &goal])["task"]["status"], "todo");

    env.json(&sci, &["shelve", &a, "someday"]);
    env.json(&sci, &["done", &b, "landed"]);
    env.json(&sci, &["shelve", &goal, "someday"]);
    assert_eq!(
        env.json(&sci, &["show", &goal])["task"]["status"],
        "shelved"
    );
}

#[test]
fn start_and_park_refuse_a_shelved_task_and_name_unshelve() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    env.json(&sci, &["shelve", &id, "later"]);
    for args in [
        vec!["start", id.as_str()],
        vec!["park", id.as_str(), "next"],
    ] {
        let err = error_of(&env, &sci, &args);
        assert_eq!(err["error"]["kind"], "invalid_transition", "{args:?}");
        assert!(
            err["error"]["detail"]
                .as_str()
                .unwrap()
                .contains("tasks unshelve"),
            "{args:?}: {err}"
        );
    }
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "shelved");
    assert!(env.json(&sci, &["show", &id])["park"].is_null());
}

#[test]
fn edit_refuses_a_transition_into_shelved_and_allows_edits_of_a_shelved_record() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));

    // shelved is outside the set `edit --status` accepts; `shelve` is the way in
    let err = env.usage(&sci, &["edit", &id, "--status", "shelved"]);
    assert!(err.contains("--status") && err.contains("shelved"), "{err}");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    let into = editor_script(&sci, "sed -i 's/^status: todo$/status: shelved/' \"$1\"");
    let out = env
        .cmd(&sci)
        .env("EDITOR", &into)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "validation");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["status"], "todo");

    env.json(&sci, &["shelve", &id, "later"]);
    let retitle = editor_script(&sci, "sed -i 's/^title: T$/title: Renamed/' \"$1\"");
    let out = env
        .cmd(&sci)
        .env("EDITOR", &retitle)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["title"], "Renamed");
    assert_eq!(
        shown["task"]["status"], "shelved",
        "an edit keeps the shelf"
    );

    env.json(&sci, &["edit", &id, "--status", "todo"]);
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["status"],
        "todo",
        "explicit reopen"
    );
}

#[test]
fn process_round_trips_and_rejects_invalid_edits() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T", "--process", "direct"]));
    let path = dir.join(format!("tasks/{id}.md"));
    assert_eq!(env.json(&dir, &["show", &id])["task"]["process"], "direct");
    env.json(&dir, &["edit", &id, "--process", "planned"]);
    env.json(&dir, &["note", &id, "retain choice"]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["process"], "planned");
    let before = std::fs::read(&path).unwrap();
    let err = env.usage(&dir, &["edit", &id, "--process", "auto"]);
    assert!(err.contains("--process") && err.contains("auto"), "{err}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let out = env
        .cmd(&dir)
        .args(["edit", &id, "--process", "direct", "--no-process"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    env.json(&dir, &["edit", &id, "--no-process"]);
    env.json(&dir, &["note", &id, "retain absence"]);
    let task = env.json(&dir, &["show", &id]);
    assert!(task["task"].get("process").is_none());
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("\nprocess:")
    );
    let err = env.usage(&dir, &["add", "Bad", "--process", "auto"]);
    assert!(err.contains("--process") && err.contains("auto"), "{err}");
    assert_eq!(
        env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        env.complete_values(&dir, "bash", 4, &["tasks", "add", "T", "--process", ""]),
        vec!["direct", "planned"]
    );
    assert!(
        env.complete_values(&dir, "zsh", 4, &["tasks", "edit", &id, "--process", ""])
            .iter()
            .any(|value| value.starts_with("planned"))
    );

    let editor = editor_script(&dir, "sed -i '/^priority:/a process: direct' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .assert()
        .success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["process"], "direct");
    let before = std::fs::read(&path).unwrap();
    let editor = editor_script(&dir, "sed -i 's/process: direct/process: auto/' \"$1\"");
    env.cmd(&dir)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .assert()
        .failure();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn process_survives_ready_and_parked_next() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T", "--process", "planned"]));
    assert_eq!(env.json(&dir, &["ready"])["tasks"][0]["process"], "planned");
    assert_eq!(env.json(&dir, &["prime"])["ready"][0]["process"], "planned");
    assert_eq!(
        env.json(&dir, &["next"])["next"]["task"]["process"],
        "planned"
    );
    assert!(env.pretty(&dir, &["ready"]).contains("planned"));
    assert!(env.pretty(&dir, &["next"]).contains("Process: planned"));
    env.json(&dir, &["park", &id, "resume here"]);
    let parked = env.json(&dir, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["process"], "planned");
    assert_eq!(parked["tasks"][0]["phase"], "implementing");
    assert_eq!(
        env.json(&dir, &["next"])["next"]["task"]["process"],
        "planned"
    );
    assert!(env.pretty(&dir, &["list", "--parked"]).contains("planned"));
    env.json(&dir, &["edit", &id, "--no-process"]);
    assert!(
        env.pretty(&dir, &["show", &id])
            .contains("Process: unassessed")
    );
    assert!(
        env.json(&dir, &["list", "--parked"])["tasks"][0]
            .get("process")
            .is_none()
    );
    assert_eq!(env.json(&dir, &["next"])["next"]["task"]["id"], id);

    // A surviving park with no record also omits unset fields.
    std::fs::remove_file(dir.join(format!("tasks/{id}.md"))).unwrap();
    let unresolved = env.json(&dir, &["list", "--parked"]);
    assert!(unresolved["tasks"][0].get("process").is_none());
}

#[test]
fn process_missing_warns_only_while_doing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T"]));
    let missing = |v: &serde_json::Value| {
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "process_missing")
    };
    assert!(!missing(&env.check(&dir)));
    env.json(&dir, &["start", &id]);
    assert!(missing(&env.check(&dir)));
    env.json(&dir, &["edit", &id, "--process", "direct"]);
    assert!(!missing(&env.check(&dir)));
    env.json(&dir, &["edit", &id, "--no-process"]);
    assert!(missing(&env.check(&dir)));
    env.json(&dir, &["done", &id]);
    assert!(!missing(&env.check(&dir)));

    let idea = id_of(env.json(&dir, &["add", "Idea", "--status", "idea"]));
    let shelved = id_of(env.json(&dir, &["add", "Shelf"]));
    env.json(&dir, &["shelve", &shelved, "not now"]);
    let dropped = id_of(env.json(&dir, &["add", "Drop"]));
    env.json(&dir, &["drop", &dropped, "unneeded"]);
    assert!(!missing(&env.check(&dir)));
    assert_eq!(env.json(&dir, &["show", &idea])["task"]["status"], "idea");

    let goal = id_of(env.json(&dir, &["add", "Goal", "--process", "planned"]));
    write_doc(&dir, "docs/plans/x.md", "# x\n\n### Task 1: do it\n");
    let step = id_of(env.json(
        &dir,
        &[
            "add",
            "Step",
            "--parent",
            &goal,
            "--plan",
            "x",
            "--step",
            "Task 1: do it",
            "--complexity",
            "low",
        ],
    ));
    assert!(
        env.json(&dir, &["show", &step])["task"]
            .get("process")
            .is_none()
    );
    assert!(!missing(&env.check(&dir)));
    env.json(&dir, &["edit", &goal, "--no-process"]);
    for active in [&goal, &step] {
        env.json(&dir, &["start", active]);
    }
    let findings = env.check(&dir);
    let ids: Vec<_> = findings["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "process_missing")
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&goal.as_str()) && ids.contains(&step.as_str()));
}

#[test]
fn process_does_not_change_selection_or_starting() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let first = id_of(env.json(&dir, &["add", "First", "-p", "1"]));
    let second = id_of(env.json(&dir, &["add", "Second", "-p", "2"]));
    for process in ["planned", "direct"] {
        env.json(&dir, &["edit", &second, "--process", process]);
        let ready = env.json(&dir, &["ready"]);
        let ids: Vec<_> = ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, [first.as_str(), second.as_str()]);
        assert!(ready["tasks"][0].get("process").is_none());
        assert_eq!(env.json(&dir, &["next"])["next"]["task"]["id"], first);
    }
    env.json(&dir, &["start", &first]);
    assert!(
        env.json(&dir, &["show", &first])["task"]
            .get("process")
            .is_none()
    );
}

#[test]
fn add_stamps_agent_from_the_flag_then_the_variable_and_absence_records_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let from_env = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Env", "-p", "2"])
                .env("TASKS_AGENT", "crush/kimi-k3")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert_eq!(
        env.json(&sci, &["show", &from_env])["task"]["agent"],
        "crush/kimi-k3"
    );
    assert!(
        env.read(&sci, &format!("tasks/{from_env}.md"))
            .contains("\nagent: crush/kimi-k3\n")
    );

    let from_flag = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Flag", "-p", "2", "--agent", "codex/gpt-6"])
                .env("TASKS_AGENT", "crush/kimi-k3")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert_eq!(
        env.json(&sci, &["show", &from_flag])["task"]["agent"],
        "codex/gpt-6"
    );

    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    let shown = env.json(&sci, &["show", &plain]);
    assert!(
        shown["task"].get("agent").is_none(),
        "unset agent must be omitted: {shown}"
    );
    assert!(shown["task"]["agent"].is_null());

    let empty = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Empty", "-p", "2"])
                .env("TASKS_AGENT", "")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert!(env.json(&sci, &["show", &empty])["task"]["agent"].is_null());
}

#[test]
fn invalid_tasks_agent_fails_add_unless_the_flag_names_a_valid_agent() {
    use std::os::unix::ffi::OsStrExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let before = std::fs::read_dir(sci.join("tasks")).unwrap().count();

    let out = env
        .cmd(&sci)
        .args(["add", "Bad", "-p", "2"])
        .env("TASKS_AGENT", std::ffi::OsStr::from_bytes(b"\xff\xfe"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        err_detail(&out).contains("TASKS_AGENT is not valid Unicode"),
        "{}",
        err_detail(&out)
    );

    let out = env
        .cmd(&sci)
        .args(["add", "Bad", "-p", "2"])
        .env("TASKS_AGENT", "a\nb")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        err_detail(&out).contains("TASKS_AGENT must be a single line"),
        "{}",
        err_detail(&out)
    );
    assert_eq!(
        std::fs::read_dir(sci.join("tasks")).unwrap().count(),
        before,
        "a failed add writes nothing"
    );

    for bad in [
        std::ffi::OsStr::from_bytes(b"\xff\xfe"),
        std::ffi::OsStr::new("a\nb"),
    ] {
        let id = id_of(
            serde_json::from_slice(
                &env.cmd(&sci)
                    .args(["add", "Good", "-p", "2", "--agent", "codex/gpt-6"])
                    .env("TASKS_AGENT", bad)
                    .assert()
                    .success()
                    .get_output()
                    .stdout,
            )
            .unwrap(),
        );
        assert_eq!(
            env.json(&sci, &["show", &id])["task"]["agent"],
            "codex/gpt-6"
        );
    }
}

#[test]
fn feedback_stamps_agent_from_the_variable_only() {
    let (env, _target, reporter) = feedback_env();
    let out: serde_json::Value = serde_json::from_slice(
        &env.cmd(&reporter)
            .args([
                "feedback",
                "--project",
                "tasks",
                "the flag is hard to find",
                "--category",
                "friction",
            ])
            .env("TASKS_AGENT", "claude-code/claude-opus-5")
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let id = out["id"].as_str().unwrap();
    assert_eq!(
        env.json(&reporter, &["show", id])["task"]["agent"],
        "claude-code/claude-opus-5"
    );
    // feedback has no --agent flag
    env.cmd(&reporter)
        .args([
            "feedback",
            "--project",
            "tasks",
            "x",
            "--category",
            "friction",
            "--agent",
            "codex",
        ])
        .assert()
        .code(2);
}

#[test]
fn edit_never_reads_tasks_agent_and_agent_flags_replace_and_clear() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));

    env.cmd(&sci)
        .args(["edit", &id, "--title", "Renamed"])
        .env("TASKS_AGENT", "crush/kimi-k3")
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["task"]["agent"].is_null(),
        "an edit under the variable does not claim creation"
    );

    env.json(&sci, &["edit", &id, "--agent", "codex/gpt-6"]);
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["agent"],
        "codex/gpt-6"
    );
    env.json(&sci, &["edit", &id, "--no-agent"]);
    assert!(env.json(&sci, &["show", &id])["task"]["agent"].is_null());
    assert!(!env.read(&sci, &format!("tasks/{id}.md")).contains("agent:"));

    assert_eq!(env.fail(&sci, &["edit", &id, "--agent", ""]), "validation");
    assert_eq!(
        env.fail(&sci, &["edit", &id, "--agent", "a\nb"]),
        "validation"
    );
    env.cmd(&sci)
        .args(["edit", &id, "--agent", "x", "--no-agent"])
        .assert()
        .code(2);

    // an editor save is validated through the record parser: an empty value is refused
    // and the record is left as it was
    env.json(&sci, &["edit", &id, "--agent", "codex/gpt-6"]);
    let editor = editor_script(&sci, "sed -i 's|^agent: .*$|agent: \"\"|' \"$1\"");
    let out = env
        .cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success(), "{}", err_detail(&out));
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["agent"],
        "codex/gpt-6"
    );

    // a value with reserved characters is quoted on disk and round-trips byte for byte
    env.json(
        &sci,
        &["edit", &id, "--agent", "crush/kimi-k3 [nightly]: b"],
    );
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["agent"],
        "crush/kimi-k3 [nightly]: b"
    );
    assert!(
        env.read(&sci, &format!("tasks/{id}.md"))
            .contains("\nagent: \"crush/kimi-k3 [nightly]: b\"\n")
    );
}

#[test]
fn summary_rows_and_parked_rows_carry_agent() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let stamped = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Stamped", "-p", "2"])
                .env("TASKS_AGENT", "codex/gpt-6")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));

    for view in [vec!["list"], vec!["ready"]] {
        let rows = env.json(&sci, &view);
        let rows = rows["tasks"].as_array().unwrap();
        let row = rows.iter().find(|row| row["id"] == stamped).unwrap();
        assert_eq!(row["agent"], "codex/gpt-6", "{view:?}");
        let row = rows.iter().find(|row| row["id"] == plain).unwrap();
        assert!(row.get("agent").is_none(), "{view:?}");
    }

    env.json(&sci, &["park", &stamped, "resume here"]);
    let prime = env.json(&sci, &["prime"]);
    let parked = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == stamped)
        .unwrap()
        .clone();
    assert_eq!(parked["agent"], "codex/gpt-6");

    let out = env
        .cmd(&sci)
        .args(["--pretty", "show", &stamped])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(text.contains("agent: codex/gpt-6\n"), "{text}");
    let out = env.cmd(&sci).args(["--pretty", "list"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(
        !text.contains("codex/gpt-6"),
        "tables gain no column: {text}"
    );
}

#[test]
fn deferred_work_is_hidden_from_ready_next_and_sample_with_one_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let soon = id_of(env.json(&sci, &["add", "Soon", "-p", "0", "--defer", "2099-01-02"]));
    let later = id_of(env.json(&sci, &["add", "Later", "-p", "0", "--defer", "2099-06-01"]));
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "3"]));
    let idea = id_of(env.json(
        &sci,
        &["add", "Idea", "--status", "idea", "--defer", "2099-01-02"],
    ));
    let ready = env.json(&sci, &["ready"]);
    let ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![plain.as_str()], "{ready}");
    assert_eq!(ready["warnings"].as_array().unwrap().len(), 1, "{ready}");
    assert_eq!(
        ready["warnings"][0],
        "2 deferred tasks omitted, next due 2099-01-02; `tasks list --deferred` shows them"
    );
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], plain);
    assert!(
        next["warnings"]
            .to_string()
            .contains("2 deferred tasks omitted"),
        "{next}"
    );
    for seed in ["1", "2", "3", "4", "5"] {
        let sample = env.json(
            &sci,
            &["sample", "-n", "10", "--older-than", "0d", "--seed", seed],
        );
        let ids: Vec<&str> = sample["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![plain.as_str()], "seed {seed}: {sample}");
    }
    let _ = (&soon, &later, &idea);
}

#[test]
fn deferral_omissions_count_only_what_would_otherwise_be_handed_out() {
    let mut env = TestEnv::new();
    let (sci, other) = two_roots(&mut env);
    let counted = id_of(env.json(
        &sci,
        &[
            "add",
            "Counted",
            "-p",
            "1",
            "--size",
            "s",
            "--complexity",
            "low",
            "--defer",
            "2099-01-02",
        ],
    ));
    let waiting = id_of(env.json(&sci, &["add", "Waiting", "--defer", "2099-01-02"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &waiting, "ask", "--waiting-on", "user"])
        .assert()
        .success();
    let claimed = id_of(env.json(&sci, &["add", "Claimed", "--defer", "2099-01-02"]));
    std::fs::copy(
        sci.join(format!("tasks/{claimed}.md")),
        other.join(format!("tasks/{claimed}.md")),
    )
    .unwrap();
    as_agent(&env, &other, "agent-b")
        .args(["start", &claimed])
        .assert()
        .success();
    let gate = id_of(env.json(&sci, &["add", "Gate"]));
    env.json(
        &sci,
        &["add", "Held", "--defer", "2099-01-02", "--depends", &gate],
    );
    let big = id_of(env.json(
        &sci,
        &[
            "add",
            "Big",
            "--size",
            "l",
            "--complexity",
            "high",
            "--defer",
            "2099-01-02",
        ],
    ));
    let ready = env.json(&sci, &["ready"]);
    assert!(
        ready["warnings"]
            .to_string()
            .contains("2 deferred tasks omitted"),
        "counted and big: {ready}"
    );
    let ready = env.json(&sci, &["ready", "--size", "s"]);
    assert!(
        ready["warnings"]
            .to_string()
            .contains("1 deferred task omitted"),
        "{ready}"
    );
    let ready = env.json(&sci, &["ready", "--parallel"]);
    assert!(
        !ready["warnings"].to_string().contains("deferred task"),
        "{ready}"
    );
    let ready = env.json(&sci, &["ready", "--max-complexity", "low"]);
    assert!(
        ready["warnings"]
            .to_string()
            .contains("1 deferred task omitted"),
        "{ready}"
    );
    let next = env.json(&sci, &["next", "--max-complexity", "low"]);
    assert!(
        next["warnings"]
            .to_string()
            .contains("1 deferred task omitted"),
        "{next}"
    );
    let _ = (&counted, &big);
}

#[test]
fn a_due_deferral_is_back_in_ready_next_and_sample() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let due = id_of(env.json(&sci, &["add", "Due", "-p", "0"]));
    let editor = editor_script(
        &sci,
        "sed -i 's/^priority: 0$/priority: 0\\ndefer: 2026-01-01/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &due])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let ready = env.json(&sci, &["ready"]);
    assert_eq!(ready["tasks"][0]["id"], due);
    assert_eq!(ready["tasks"][0]["deferred"]["due"], true);
    assert!(ready["warnings"].as_array().unwrap().is_empty(), "{ready}");
    let next = env.json(&sci, &["next"]);
    assert_eq!(next["next"]["task"]["id"], due);
    assert_eq!(next["next"]["deferred"]["due"], true, "{next}");
    let sample = env.json(
        &sci,
        &["sample", "-n", "10", "--older-than", "0d", "--seed", "1"],
    );
    assert_eq!(sample["tasks"][0]["id"], due);
}

#[test]
fn a_deferred_parked_idea_is_omitted_from_next_with_the_warning() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let idea = id_of(env.json(
        &sci,
        &["add", "Idea", "--status", "idea", "--defer", "2099-01-02"],
    ));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &idea, "scope it"])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert!(next["next"].is_null(), "{next}");
    assert!(
        next["warnings"]
            .to_string()
            .contains("1 deferred task omitted, next due 2099-01-02"),
        "{next}"
    );
    let prime = env.json(&sci, &["prime"]);
    assert_eq!(prime["parked"][0]["id"], idea, "{prime}");
    let todo = id_of(env.json(&sci, &["add", "Todo", "--defer", "2099-01-02"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &todo, "resume"])
        .assert()
        .success();
    let next = env.json(&sci, &["next"]);
    assert!(
        next["warnings"]
            .to_string()
            .contains("2 deferred tasks omitted"),
        "{next}"
    );
}

#[test]
fn quiet_judges_the_copy_it_would_hand_out() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    let (_keep, wt) = unregistered_checkout("sci");
    let copy = |id: &str| {
        std::fs::copy(
            main.join(format!("tasks/{id}.md")),
            wt.join(format!("tasks/{id}.md")),
        )
        .unwrap()
    };
    let park = |env: &TestEnv, id: &str| {
        as_agent(env, &wt, "agent-a")
            .args([
                "park",
                id,
                "bench",
                "--waiting-on",
                "user",
                "--reason",
                "quiet",
                "--minutes",
                "20",
            ])
            .assert()
            .success()
    };
    let a = id_of(env.json(&main, &["add", "A", "-p", "1"]));
    copy(&a);
    env.json(&wt, &["edit", &a, "--defer", "2099-01-02"]);
    park(&env, &a);
    let b = id_of(env.json(&main, &["add", "B", "-p", "1"]));
    copy(&b);
    env.json(&main, &["edit", &b, "--defer", "2099-01-02"]);
    park(&env, &b);
    let v = env.json(&main, &["quiet"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![b.as_str()], "{v}");
    assert!(v["tasks"][0]["deferred"].is_null(), "{v}");
    assert_eq!(
        v["tasks"][0]["park"]["worktree"],
        wt.to_string_lossy().as_ref()
    );
}

#[test]
fn a_deferred_todo_still_holds_its_dependents() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let gate = id_of(env.json(&sci, &["add", "Gate", "--defer", "2099-01-02"]));
    let after = id_of(env.json(&sci, &["add", "After", "--depends", &gate]));
    let ready = env.json(&sci, &["ready"]);
    assert!(ready["tasks"].as_array().unwrap().is_empty(), "{ready}");
    env.json(&sci, &["start", &gate]);
    env.json(&sci, &["done", &gate, "opened"]);
    assert_eq!(env.json(&sci, &["ready"])["tasks"][0]["id"], after);
}

#[test]
fn list_deferred_orders_by_date_with_due_first_and_conflicts_with_orderings() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let later = id_of(env.json(
        &sci,
        &["add", "Later", "--defer", "2099-06-01", "--tag", "x"],
    ));
    let soon = id_of(env.json(
        &sci,
        &["add", "Soon", "--status", "idea", "--defer", "2099-01-02"],
    ));
    let due = id_of(env.json(&sci, &["add", "Due", "--tag", "x"]));
    let editor = editor_script(
        &sci,
        "sed -i 's/^priority: 2$/priority: 2\\ndefer: 2026-01-01/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &due])
        .env("EDITOR", &editor)
        .assert()
        .success();
    env.json(&sci, &["add", "Plain"]);
    let v = env.json(&sci, &["list", "--deferred"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![due.as_str(), soon.as_str(), later.as_str()],
        "{v}"
    );
    let v = env.json(&sci, &["list", "--deferred", "--tag", "x"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![due.as_str(), later.as_str()], "{v}");
    let pretty = env.pretty(&sci, &["list", "--deferred"]);
    assert!(pretty.contains("2099-06-01"), "{pretty}");
    assert!(pretty.contains("defer 2099-06-01"), "{pretty}");
    assert!(pretty.contains("due 2026-01-01"), "{pretty}");
    for extra in [
        vec!["--sort", "updated"],
        vec!["--reverse"],
        vec!["--parked"],
        vec!["--periodic"],
    ] {
        let mut args = vec!["list", "--deferred"];
        args.extend(extra);
        env.cmd(&sci).args(&args).assert().code(2);
    }
}

#[test]
fn prime_carries_the_deferred_line_and_aggregate() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let empty = env.json(&sci, &["prime"]);
    assert_eq!(empty["deferred"]["waiting"], 0);
    assert_eq!(empty["deferred"]["due"], 0);
    assert!(empty["deferred"]["next"].is_null());
    assert!(!env.pretty(&sci, &["prime"]).contains("deferred:"));

    let later = id_of(env.json(&sci, &["add", "Later", "--defer", "2099-06-01"]));
    let idea = id_of(env.json(
        &sci,
        &["add", "Soon", "--status", "idea", "--defer", "2099-01-02"],
    ));
    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["deferred"]["waiting"], 2);
    assert_eq!(v["deferred"]["next"], "2099-01-02");
    assert!(v["deferred"]["in_days"].as_i64().unwrap() > 0, "{v}");
    assert_eq!(v["deferred"]["due"], 0);
    let pretty = env.pretty(&sci, &["prime"]);
    let line = pretty
        .lines()
        .find(|l| l.starts_with("deferred:"))
        .unwrap_or_else(|| panic!("{pretty}"));
    assert!(
        line.starts_with("deferred: 2 waiting, next 2099-01-02 (in "),
        "{line}"
    );
    assert!(!line.contains("due"), "{line}");
    assert!(
        pretty.contains("defer 2099-01-02"),
        "roadmap rows carry the marker: {pretty}"
    );

    let editor = editor_script(
        &sci,
        "sed -i 's/^defer: 2099-01-02$/defer: 2026-01-01/' \"$1\"",
    );
    env.cmd(&sci)
        .args(["edit", &idea])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["deferred"]["waiting"], 1);
    assert_eq!(v["deferred"]["next"], "2099-06-01");
    assert_eq!(v["deferred"]["due"], 1);
    let pretty = env.pretty(&sci, &["prime"]);
    let line = pretty.lines().find(|l| l.starts_with("deferred:")).unwrap();
    assert!(line.ends_with("; 1 due"), "{line}");
    assert!(pretty.contains("due 2026-01-01"), "{pretty}");

    env.json(&sci, &["edit", &later, "--no-defer"]);
    let v = env.json(&sci, &["prime"]);
    assert_eq!(v["deferred"]["waiting"], 0);
    assert!(v["deferred"]["next"].is_null());
    assert!(v["deferred"]["in_days"].is_null());
    assert_eq!(v["deferred"]["due"], 1);
    let pretty = env.pretty(&sci, &["prime"]);
    let line = pretty.lines().find(|l| l.starts_with("deferred:")).unwrap();
    assert_eq!(line, "deferred: 0 waiting; 1 due");
    assert!(!line.contains("next"), "{line}");

    let show = env.pretty(&sci, &["show", &idea]);
    assert!(
        show.contains("# deferred\nuntil: 2026-01-01 (due)\n"),
        "{show}"
    );
}

#[test]
fn sparse_json_omits_unset_task_fields_but_preserves_values_and_envelopes() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let empty = env.json(&dir, &["list"]);
    assert_eq!(empty, serde_json::json!({"tasks": [], "warnings": []}));
    let empty_next = env.json(&dir, &["next"]);
    assert_eq!(empty_next.get("next"), Some(&serde_json::Value::Null));

    let id = id_of(env.json(&dir, &["add", "Sparse", "-p", "0"]));
    let raw = env.read(&dir, &format!("tasks/{id}.md"));
    for command in ["list", "ready"] {
        let out = env.json(&dir, &[command]);
        let row = &out["tasks"][0];
        assert_eq!(row["id"], id);
        assert_eq!(row["priority"], 0);
        assert_eq!(row["parallel"], false);
        assert_eq!(row["child_count"], 0);
        assert_eq!(row["open_descendant_count"], 0);
        for key in [
            "size", "process", "owner", "claim", "park", "tags", "depends",
        ] {
            assert!(row.get(key).is_none(), "{command}: {key}: {row}");
        }
        assert_eq!(out["warnings"], serde_json::json!([]));
    }
    let prime = env.json(&dir, &["prime"]);
    assert!(prime["ready"][0].get("size").is_none());
    assert!(prime["roadmap"][0].get("children").is_none());
    assert_eq!(prime["doing"], serde_json::json!([]));
    let tree = env.json(&dir, &["tree"]);
    assert!(tree["nodes"][0].get("children").is_none());
    let show = env.json(&dir, &["show", &id]);
    for key in ["size", "tags", "depends", "notes"] {
        assert!(show["task"].get(key).is_none(), "{key}: {show}");
    }
    for key in [
        "claim",
        "parent",
        "spec_path",
        "step_found",
        "depends_on",
        "children",
    ] {
        assert!(show.get(key).is_none(), "{key}: {show}");
    }
    assert_eq!(show["task"]["body"], "");
    let next = env.json(&dir, &["next"]);
    assert_eq!(next["next"]["task"], show["task"]);
    assert!(next["next"].get("depends_on").is_none());
    assert_eq!(env.read(&dir, &format!("tasks/{id}.md")), raw);

    env.json(
        &dir,
        &["edit", &id, "--tag", "cli", "--size", "s", "--every", "30d"],
    );
    let row = &env.json(&dir, &["list"])["tasks"][0];
    assert_eq!(row["tags"], serde_json::json!(["cli"]));
    assert_eq!(row["size"], "s");
    assert_eq!(row["periodic"]["every"], "30d");
    assert_eq!(row["periodic"]["due_now"], false);
    assert!(row["periodic"].get("due").is_none());
    assert!(row["periodic"].get("last_done").is_none());
    env.json(&dir, &["park", &id, "Resume here"]);
    let parked = env.json(&dir, &["list", "--parked"]);
    let row = &parked["tasks"][0];
    assert!(row.get("depends").is_none());
    assert!(row.get("parent").is_none());
    assert_eq!(row["park"]["next_step"], "Resume here");
    assert!(row["park"].get("reason").is_none());
    assert_eq!(row["parallel"], false);
}

// ---- The shared CLI vocabulary: the behaviour half of the conformance test (the
// structural half is `surface::tests::parser_surface_equals_table`). The vocabulary and
// its design live with the table vendored as tools/cli.toml; see its header.

fn table_commands() -> Vec<Vec<String>> {
    let doc: toml::Value = toml::from_str(include_str!("../tools/cli.toml")).unwrap();
    doc["cli"]["tasks"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            c["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

#[test]
fn cli_vocabulary_help_version_usage() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let mut paths = table_commands();
    paths.push(vec![]);
    for path in &paths {
        for flag in ["--help", "-h"] {
            let mut args = path.clone();
            args.push(flag.to_string());
            let out = env.cmd(&dir).args(&args).output().unwrap();
            assert!(
                out.status.success() && !out.stdout.is_empty() && out.stderr.is_empty(),
                "{path:?} {flag}"
            );
        }
        if !path.is_empty() {
            let mut via_help = vec!["help".to_string()];
            via_help.extend(path.iter().cloned());
            let mut direct = path.clone();
            direct.push("--help".into());
            let a = env.cmd(&dir).args(&via_help).output().unwrap();
            let b = env.cmd(&dir).args(&direct).output().unwrap();
            assert_eq!(a.stdout, b.stdout, "help routing for {path:?}");
        }
    }
    for flag in ["--version", "-V"] {
        let out = env.cmd(&dir).args([flag]).output().unwrap();
        assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("tasks "));
    }
    for args in [
        vec!["bogus"],
        vec!["list", "--bogus"],
        vec!["show"],
        vec!["help", "bogus"],
    ] {
        let out = env.cmd(&dir).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            out.stdout.is_empty()
                && !out.stderr.is_empty()
                && String::from_utf8_lossy(&out.stderr).lines().count() <= 2,
            "{args:?}"
        );
    }
}

#[test]
fn cli_vocabulary_enum_baselines_cover_every_enum_row() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // `feedback` files into the registered `tasks` project; here that is a fixture
    let target = env.init("tasks");
    accept_feedback(&target, "The tasks CLI.");
    let id = id_of(env.json(&dir, &["add", "Baseline"]));
    let i = id.as_str();
    // (path, binding) -> a real success; the value that must be replaced is the word after the binding, or the binding itself for a positional
    type Baseline<'a> = ((Vec<&'a str>, &'a str), Vec<&'a str>);
    let baselines: Vec<Baseline> = vec![
        (
            (vec!["add"], "--status"),
            vec!["add", "B1", "--status", "idea"],
        ),
        ((vec!["add"], "--size"), vec!["add", "B2", "--size", "s"]),
        (
            (vec!["add"], "--complexity"),
            vec!["add", "B3", "--complexity", "low"],
        ),
        (
            (vec!["add"], "--process"),
            vec!["add", "B4", "--process", "direct"],
        ),
        (
            (vec!["add"], "--priority"),
            vec!["add", "B5", "--priority", "1"],
        ),
        (
            (vec!["edit"], "--status"),
            vec!["edit", i, "--status", "todo"],
        ),
        ((vec!["edit"], "--size"), vec!["edit", i, "--size", "m"]),
        (
            (vec!["edit"], "--complexity"),
            vec!["edit", i, "--complexity", "mid"],
        ),
        (
            (vec!["edit"], "--process"),
            vec!["edit", i, "--process", "planned"],
        ),
        (
            (vec!["edit"], "--priority"),
            vec!["edit", i, "--priority", "2"],
        ),
        ((vec!["list"], "--status"), vec!["list", "--status", "todo"]),
        ((vec!["list"], "--sort"), vec!["list", "--sort", "updated"]),
        ((vec!["tags"], "--status"), vec!["tags", "--status", "todo"]),
        (
            (vec!["projects"], "--sort"),
            vec!["projects", "--sort", "size"],
        ),
        ((vec!["ready"], "--size"), vec!["ready", "--size", "m"]),
        (
            (vec!["ready"], "--max-complexity"),
            vec!["ready", "--max-complexity", "low"],
        ),
        (
            (vec!["next"], "--max-complexity"),
            vec!["next", "--max-complexity", "low"],
        ),
        (
            (vec!["graph"], "--format"),
            vec!["graph", "--format", "dot"],
        ),
        (
            (vec!["park"], "--waiting-on"),
            vec!["park", i, "step", "--waiting-on", "user"],
        ),
        (
            (vec!["park"], "--reason"),
            vec!["park", i, "step", "--reason", "review"],
        ),
        (
            (vec!["park"], "--complexity"),
            vec![
                "park",
                i,
                "step",
                "--reason",
                "capability",
                "--complexity",
                "high",
            ],
        ),
        (
            (vec!["park"], "--needs"),
            vec![
                "park",
                i,
                "step",
                "--reason",
                "quiet",
                "--waiting-on",
                "user",
                "--minutes",
                "5",
                "--needs",
                "idle",
            ],
        ),
        (
            (vec!["feedback"], "--category"),
            vec![
                "feedback",
                "--project",
                "tasks",
                "the tool works",
                "--category",
                "positive",
            ],
        ),
    ];
    let table = tasks_surface_enum_rows(); // (path, binding) for every enum option/arg row of `tasks` in tools/cli.toml
    let covered: std::collections::BTreeSet<_> = baselines
        .iter()
        .map(|((p, b), _)| {
            (
                p.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                b.to_string(),
            )
        })
        .collect();
    assert_eq!(covered, table, "every enum row needs a baseline");
    for ((_, binding), argv) in &baselines {
        let ok = env.cmd(&dir).args(argv).output().unwrap();
        assert!(
            ok.status.success(),
            "baseline {argv:?}: {}",
            String::from_utf8_lossy(&ok.stderr)
        );
        let pos = argv.iter().position(|w| w == binding).unwrap() + 1;
        let mut bad = argv.clone();
        bad[pos] = "__not_in_set__";
        let out = env.cmd(&dir).args(&bad).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{bad:?}");
        assert!(
            err.contains(binding) && err.contains("__not_in_set__"),
            "{bad:?}: {err}"
        );
    }
}

fn tasks_surface_enum_rows() -> std::collections::BTreeSet<(Vec<String>, String)> {
    let doc: toml::Value = toml::from_str(include_str!("../tools/cli.toml")).unwrap();
    let mut rows = std::collections::BTreeSet::new();
    for cmd in doc["cli"]["tasks"]["commands"].as_array().unwrap() {
        let path: Vec<String> = cmd["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        for opt in cmd
            .get("options")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
        {
            if opt["value"].as_str() == Some("enum") {
                let name = match opt.get("shared") {
                    Some(k) => doc["vocabulary"]["options"][k.as_str().unwrap()]["names"][0]
                        .as_str()
                        .unwrap()
                        .to_string(),
                    None => opt["names"][0].as_str().unwrap().to_string(),
                };
                rows.insert((path.clone(), name));
            }
        }
        for arg in cmd
            .get("args")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
        {
            if arg["value"].as_str() == Some("enum") {
                rows.insert((path.clone(), arg["name"].as_str().unwrap().to_string()));
            }
        }
    }
    rows
}

#[test]
fn cli_vocabulary_routing_output() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // global routing: both placements, and the conflict
    for args in [
        vec!["--json", "list"],
        vec!["list", "--json"],
        vec!["--pretty", "list"],
        vec!["list", "--pretty"],
    ] {
        assert!(
            env.cmd(&dir).args(&args).output().unwrap().status.success(),
            "{args:?}"
        );
    }
    assert_eq!(
        env.cmd(&dir)
            .args(["--json", "list", "--pretty"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    // json failure: error object on stderr, nothing on stdout
    let out = env.cmd(&dir).args(["show", "sci-000000"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(err["error"]["kind"].is_string() && err["error"]["detail"].is_string());
    // output precedence: default json; TASKS_FORMAT=pretty flips it; --json wins over the variable
    assert!(
        serde_json::from_slice::<serde_json::Value>(
            &env.cmd(&dir).args(["list"]).output().unwrap().stdout
        )
        .is_ok()
    );
    let out = env
        .cmd(&dir)
        .env("TASKS_FORMAT", "pretty")
        .args(["list"])
        .output()
        .unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&out.stdout).is_err());
    let out = env
        .cmd(&dir)
        .env("TASKS_FORMAT", "pretty")
        .args(["--json", "list"])
        .output()
        .unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&out.stdout).is_ok());
    // color: accepted both sides, refused outside its set, honoured from the variable
    assert!(
        env.cmd(&dir)
            .args(["--color", "never", "--pretty", "list"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        env.cmd(&dir)
            .args(["--pretty", "list", "--color", "never"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        env.cmd(&dir)
            .args(["--color", "sometimes", "list"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert!(
        env.cmd(&dir)
            .env("TASKS_COLOR", "always")
            .args(["--pretty", "list"])
            .output()
            .unwrap()
            .status
            .success()
    );
    // check: clean prints nothing; findings containing an error print a JSON object and exit 1
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert!(out.status.success() && out.stdout.is_empty());
    std::fs::write(
        dir.join("tasks/sci-ffffff.md"),
        "---\nid: sci-ffffff\ntitle: Broken\nstatus: nonsense\n---\n",
    )
    .unwrap();
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .map(|v| v.is_object())
            .unwrap_or(false)
    );
}

#[test]
fn cli_vocabulary_completion() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let complete = |words: &[&str], index: usize| {
        let out = env
            .cmd(&dir)
            .env("TASKS_COMPLETE", "zsh")
            .env("_CLAP_COMPLETE_INDEX", index.to_string())
            .env("_CLAP_IFS", "\n")
            .arg("--")
            .args(words)
            .output()
            .unwrap();
        // clap's zsh callback prints `value:description` (its bash form prints bare values)
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.split(':').next().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    };
    let root = complete(&["tasks", ""], 1);
    for name in ["add", "list", "sample", "prime", "quiet", "help"] {
        assert!(root.contains(&name.to_string()), "{root:?}");
    }
    assert!(
        complete(&["tasks", "list", "--"], 2)
            .iter()
            .any(|w| w == "--sort")
    );
    let sorts = complete(&["tasks", "list", "--sort", ""], 3);
    for v in ["priority", "updated", "created"] {
        assert!(sorts.contains(&v.to_string()), "{sorts:?}");
    }
    // the script registers the completer when sourced
    let script = env
        .cmd(&dir)
        .env("TASKS_COMPLETE", "zsh")
        .output()
        .unwrap()
        .stdout;
    let path = dir.join("_tasks");
    std::fs::write(&path, &script).unwrap();
    let z = std::process::Command::new("zsh")
        .args([
            "-f",
            "-c",
            &format!(
                "autoload -Uz compinit; compinit -D -u; source {}; print -r -- ${{_comps[tasks]}}",
                path.display()
            ),
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&z.stdout).trim(),
        "_clap_dynamic_completer_tasks"
    );
    let script = env
        .cmd(&dir)
        .env("TASKS_COMPLETE", "bash")
        .output()
        .unwrap()
        .stdout;
    std::fs::write(dir.join("tasks.bash"), &script).unwrap();
    let b = std::process::Command::new("bash")
        .args([
            "-c",
            &format!(
                "source {}; complete -p tasks",
                dir.join("tasks.bash").display()
            ),
        ])
        .output()
        .unwrap();
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
}
#[test]
fn a_continuity_repeated_start_by_the_owner_keeps_one_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    for _ in 0..2 {
        env.cmd(&dir)
            .env("TASKS_SESSION", "owner")
            .args(["start", &id])
            .assert()
            .success();
    }
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(store.matches("session = \"owner\"").count(), 1, "{store}");
}

// NOTE: the explicit-mismatch case lives in Task 8 as an acceptance test. It has to run
// under a harness shim with relay enabled and a matching boundary, or removing the
// explicit-identity guard from `Ctx::ownership` would leave it passing — proof would never
// have been consulted in the first place.

#[test]
fn a_continuity_park_and_close_by_the_owner_still_work() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let session = || ("TASKS_SESSION", "owner");
    env.cmd(&dir)
        .env(session().0, session().1)
        .args(["start", &id])
        .assert()
        .success();
    env.cmd(&dir)
        .env(session().0, session().1)
        .args(["park", &id, "next"])
        .assert()
        .success();
    env.cmd(&dir)
        .env(session().0, session().1)
        .args(["start", &id])
        .assert()
        .success();
    env.cmd(&dir)
        .env(session().0, session().1)
        .args(["done", &id, "landed"])
        .assert()
        .success();
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "done");
}
/// Enable relay identity for this test's HOME and return its relay state directory.
fn relay_on(env: &TestEnv) -> std::path::PathBuf {
    let config = env.home.path().join(".config/tasks/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[identity]\nrelay = true\n").unwrap();
    env.home.path().join("relay-state")
}

/// The preamble every shim script shares: exports, then the registry writer.
fn shim_env(state: &std::path::Path, harness: &str, session: &str) -> String {
    format!(
        "RELAY_STATE_DIR={}\nHARNESS={harness}\nSESSION={session}\nAGENT_ID={harness}:{session}\n\
         export RELAY_STATE_DIR HARNESS SESSION AGENT_ID TASKS_BIN\n{}",
        state.display(),
        common::WRITE_REGISTRY
    )
}
#[test]
fn a_note_lands_when_relay_identity_cannot_resolve() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // In scope under a live codex ancestor, but the registry holds no agents — the real
    // Codex case before its first SessionStart. Identity cannot resolve; the note must
    // land anyway, and must say why the heartbeat was skipped.
    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":2,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n\
         \"$TASKS_BIN\" note {id} 'still lands'\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);

    // `start` is acquisition and must fail; `note` must still succeed.
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("\"warnings\""),
        "note should have produced output: {text}"
    );
    let shown = env.json(&dir, &["show", &id]);
    assert!(
        shown["task"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note["text"].as_str().unwrap().contains("still lands")),
        "the note must land even though relay identity could not resolve"
    );
}

#[test]
fn a_note_warns_when_ownership_evidence_is_unavailable() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Claim under a working registry, then remove it and note with a contradicted hint so
    // neither identity nor proof can establish ownership.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         CODEX_SESSION_ID=someone-else \"$TASKS_BIN\" note {id} 'orphan note'\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("heartbeat") && text.contains("not refreshed"),
        "an unprovable claim must say the heartbeat was skipped: {text}"
    );
}

#[test]
fn a_note_from_a_foreign_session_leaves_the_claim_alone() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    env.cmd(&dir)
        .env("TASKS_SESSION", "owner")
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", &id])
        .assert()
        .success();
    let before = std::fs::read_to_string(env.claim_store("sci")).unwrap();

    let out = env
        .cmd(&dir)
        .env("TASKS_SESSION", "stranger")
        .args(["note", &id, "from elsewhere"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let after = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(
        before, after,
        "a foreign note must not refresh or alter the claim"
    );
    // An ordinary foreign note resolved its identity fine, so it must not warn.
    assert!(!String::from_utf8_lossy(&out.stdout).contains("heartbeat"));
}

#[test]
fn a_note_with_relay_off_is_unchanged_under_a_harness_ancestor() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // No config file: relay is off, so the harness ancestor is irrelevant and the native
    // ladder resolves as it always has.
    let script = format!("\"$TASKS_BIN\" start {id}\n\"$TASKS_BIN\" note {id} 'native'\n");
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains("heartbeat"));
}

#[test]
fn an_acceptance_relay_claim_is_keyed_by_the_agent_id_with_its_proof() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"codex:s1\""), "{store}");
    assert!(store.contains("pid_start"), "{store}");
    assert!(store.contains("boot_id"), "{store}");
}

#[test]
fn an_acceptance_owner_can_park_and_close_after_the_registry_is_removed() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let parked = id_of(env.json(&dir, &["add", "To park", "-p", "2"]));
    let closed = id_of(env.json(&dir, &["add", "To close", "-p", "2"]));
    let state = relay_on(&env);

    // Two *separately held* claims. Park and close are both releases by the owner, but
    // `Store::insert_park` removes the claim along with its proof, so a task cannot be
    // parked and then resumed on the same run: resumption is acquisition again and needs
    // either a restored registry or the explicit override.
    let script = format!(
        "{}\nwrite_registry\n\
         \"$TASKS_BIN\" start {parked}\n\
         \"$TASKS_BIN\" start {closed}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" park {parked} 'next step'\n\
         \"$TASKS_BIN\" done {closed} landed\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the owner must park and close with the registry gone: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(env.json(&dir, &["show", &closed])["task"]["status"], "done");
}

#[test]
fn an_acceptance_resuming_a_parked_task_needs_identity_again() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Parking released the claim and its proof with it, so the resume is a fresh
    // acquisition. With the registry still gone it must refuse rather than quietly
    // claim under some other identity.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" park {id} 'next step'\n\
         \"$TASKS_BIN\" start {id} && echo RESUMED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("RESUMED"),
        "a parked task cannot be resumed without a resolvable identity"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("TASKS_SESSION"));
}

#[test]
fn an_acceptance_repeated_start_after_registry_loss_keeps_the_held_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert_eq!(
        store.matches("session = \"codex:s1\"").count(),
        1,
        "{store}"
    );
}

#[test]
fn an_acceptance_nested_harness_cannot_close_the_outer_sessions_task() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // A claude shim claims the task, then runs a codex shim beneath itself which tries to
    // close it. The inner session's nearest boundary is the codex process, so its
    // ownership proof against the claude claim must fail. `exit $?` closes the inner
    // script for the reason `harness_shim` documents: without it `sh -c` would `execve`
    // `tasks` in place of the inner shim, leaving the outer claude process as the nearest
    // boundary — which is the owner, so the close would land and the test would pass
    // while proving nothing.
    // A symlink, not a copy, for the reason `harness_shim` documents.
    let inner = env.home.path().join("codex");
    std::os::unix::fs::symlink("/bin/sh", &inner).unwrap();

    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         \"{}\" -c '\"$TASKS_BIN\" done {id} landed; exit $?' && echo INNER_CLOSED\n",
        shim_env(&state, "claude-code", "c1"),
        inner.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        !text.contains("INNER_CLOSED"),
        "a session nested under the owner must not close its task: {text}"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_contradicted_hint_is_refused_at_acquisition() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\nCODEX_SESSION_ID=someone-else \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("someone-else") && text.contains("codex:s1"),
        "{text}"
    );
}

#[test]
fn an_acceptance_contradicted_hint_is_refused_on_a_held_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // The claim is held and carries proof; the registry is then removed, so ownership can
    // only come from proof. A contradicted hint must defeat it — a deferred resolution
    // error must not become an accepted owner.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         CODEX_SESSION_ID=someone-else \"$TASKS_BIN\" done {id} landed\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_empty_registry_refuses_rather_than_using_terminal_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":2,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("TASKS_SESSION"));
    let store = env.claim_store("sci");
    assert!(
        !store.exists() || !std::fs::read_to_string(&store).unwrap().contains("sid:"),
        "terminal identity must never be used in scope"
    );
}

#[test]
fn an_acceptance_world_readable_registry_is_refused() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\nchmod 644 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("privately owned"));
}

#[test]
fn an_acceptance_schema_one_registry_is_superseded() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // A correct row for this very process, but in a schema-1 file: refused, not matched.
    let script = format!(
        "{}\nwrite_registry\n\
         sed -i 's/\"schema\":2,/\"schema\":1,/' \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("superseded"), "{text}");
    assert!(text.contains("relay reap"), "{text}");
    assert!(text.contains("TASKS_SESSION"), "{text}");
    assert!(
        !env.claim_store("sci").exists()
            || !std::fs::read_to_string(env.claim_store("sci"))
                .unwrap()
                .contains("codex:s1")
    );
}

#[test]
fn an_acceptance_background_session_claims_as_its_versioned_process() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    let script = format!(
        "{}\nwrite_registry\necho \"SHIM=$$\"\n\"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "claude-code", "bg1")
    );
    let out = common::harness_shim_at(&bin, &dir, env.home.path(), &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let shim = stdout
        .lines()
        .find_map(|line| line.strip_prefix("SHIM="))
        .unwrap();
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"claude-code:bg1\""), "{store}");
    assert!(store.contains(&format!("pid = {shim}\n")), "{store}");
}

#[test]
fn an_acceptance_nested_version_session_cannot_continue_the_outer_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    // The outer claude shim claims the task under its own row, then runs a nested 2.1.280
    // twice: once with no hint, once with the outer session's id inherited. Each nested
    // `start`, `park` and `done` must fail. `exit $?` in the inner scripts keeps `tasks` a
    // child of the nested process, for the reason `harness_shim` documents.
    let nested = |prefix: &str| {
        format!(
            "{prefix}\"{bin}\" -c '\"$TASKS_BIN\" start {id} && echo NESTED_STARTED; \
             \"$TASKS_BIN\" park {id} next && echo NESTED_PARKED; \
             \"$TASKS_BIN\" done {id} landed && echo NESTED_CLOSED; exit 0'\n",
            bin = bin.display()
        )
    };
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id} || exit 70\n{}{}",
        shim_env(&state, "claude-code", "c1"),
        nested(""),
        nested("CLAUDE_CODE_SESSION_ID=c1 "),
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for marker in ["NESTED_STARTED", "NESTED_PARKED", "NESTED_CLOSED"] {
        assert!(!stdout.contains(marker), "{marker}: {stdout}");
    }
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"claude-code:c1\""), "{store}");
}

#[test]
fn an_acceptance_version_session_under_claude_never_claims_as_the_outer_session() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    // The review chain: tasks -> 2.1.280 -> claude, with a correct row for the outer claude
    // and no hint. The nearest harness is the version process, which has no row.
    let script = format!(
        "{}\nwrite_registry\n\"{}\" -c 'echo \"VERSION=$$\"; \"$TASKS_BIN\" start {id}; exit $?'\n",
        shim_env(&state, "claude-code", "outer"),
        bin.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_ne!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid = stdout
        .lines()
        .find_map(|line| line.strip_prefix("VERSION="))
        .unwrap_or_else(|| panic!("no VERSION= line in stdout: {stdout}"));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("2.1.280"), "{text}");
    assert!(text.contains("TASKS_SESSION"), "{text}");
    assert!(text.contains(&format!("pid {pid}")), "{text}");
    let store = env.claim_store("sci");
    assert!(
        !store.exists()
            || !std::fs::read_to_string(&store)
                .unwrap()
                .contains("claude-code:outer"),
        "the nested session must never claim as the outer one"
    );
}

#[test]
fn an_acceptance_hosting_process_refuses_before_reading_the_registry() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // Relay on, and deliberately no registry at all: a refusal that names the role, not a
    // missing file, shows the registry was never opened.
    relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");
    let claude = env.home.path().join("claude");
    std::os::unix::fs::symlink("/bin/sh", &claude).unwrap();

    for (program, role) in [(&bin, "bg-pty-host"), (&claude, "daemon")] {
        let script = format!("\"$TASKS_BIN\" start {id}\n");
        let out = common::shim_with_argument(program, role, &dir, env.home.path(), &script);
        assert_ne!(out.status.code(), Some(0), "{role}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(
            text.contains(role) && text.contains("not a session"),
            "{role}: {text}"
        );
        assert!(!text.contains("does not exist"), "{role}: {text}");
    }
}

#[test]
fn an_acceptance_explicit_pair_works_under_a_harness_with_no_registry() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "RELAY_STATE_DIR={} TASKS_SESSION=explicit \"$TASKS_BIN\" start {id}\n",
        state.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::fs::read_to_string(env.claim_store("sci"))
            .unwrap()
            .contains("explicit")
    );
}

#[test]
fn an_acceptance_explicit_mismatch_stays_foreign_under_one_harness() {
    // Two workers beneath the *same* shim, so the ancestry, host and boot all agree and
    // the claim's proof names their shared harness process. Only TASKS_SESSION tells them
    // apart. If `Ctx::ownership` stopped honouring the explicit pair, worker-b's proof
    // would succeed and this close would land — which is exactly the bypass to catch.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    let script = format!(
        "{}\nwrite_registry\n\
         TASKS_SESSION=worker-a TASKS_SESSION_PID=$$ \"$TASKS_BIN\" start {id}\n\
         TASKS_SESSION=worker-b TASKS_SESSION_PID=$$ \"$TASKS_BIN\" done {id} 'not mine' \
           && echo FOREIGN_CLOSED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("FOREIGN_CLOSED"),
        "an explicit session mismatch must stay foreign however the ancestry looks"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
}

#[test]
fn an_acceptance_mode_change_continues_a_natively_held_claim() {
    // Acquisition, the configuration change, and the continuation all happen inside **one**
    // shim. A second `harness_shim` would be a different process whose ancestry cannot
    // prove anything about the first one's claim, so the close would succeed through
    // ordinary stale takeover and the test would pass with proof-based continuity broken.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = env.home.path().join("relay-state");
    let after_start = env.home.path().join("after-start.toml");

    // Claim natively with a pid, so the claim carries proof and is keyed `c1`. Then enable
    // relay mid-run: identity now resolves to `claude-code:c1`, which does *not* equal the
    // claim's session, so only proof can establish ownership.
    // `set -e`: every command here must succeed, and a failed repeated `start` must not be
    // masked by a `done` that then takes the claim over.
    let script = format!(
        "set -e\n{}\nwrite_registry\n\
         CLAUDE_CODE_SESSION_ID=c1 CLAUDE_PID=$$ \"$TASKS_BIN\" start {id}\n\
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"$HOME/native.toml\"\n\
         mkdir -p \"$HOME/.config/tasks\"\n\
         printf '[identity]\\nrelay = true\\n' > \"$HOME/.config/tasks/config.toml\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" start {id}\n\
         cp \"$HOME/.local/state/tasks/claims/sci.toml\" \"{}\"\n\
         CLAUDE_CODE_SESSION_ID=c1 \"$TASKS_BIN\" done {id} landed\n",
        shim_env(&state, "claude-code", "c1"),
        after_start.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a natively-held claim carrying proof must be continued: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let native = std::fs::read_to_string(env.home.path().join("native.toml")).unwrap();
    assert!(
        native.contains("session = \"c1\""),
        "claimed natively: {native}"
    );

    // The repeated `start` under relay kept the claim's own key: no re-keying to
    // `claude-code:c1`, and one claim rather than a takeover of a foreign one.
    let refreshed = std::fs::read_to_string(&after_start).unwrap();
    assert!(
        refreshed.contains("session = \"c1\""),
        "re-keyed: {refreshed}"
    );
    assert!(
        !refreshed.contains("claude-code:c1"),
        "re-keyed: {refreshed}"
    );
    assert_eq!(refreshed.matches("session = ").count(), 1, "{refreshed}");

    // A continuation is not a takeover, so nothing may have warned about one.
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        !text.contains("took over"),
        "continuation must not take over: {text}"
    );
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "done");
}

#[test]
fn an_acceptance_force_cannot_take_over_without_a_resolved_identity() {
    // A stale foreign claim and no resolvable identity. `--force` may displace an owner,
    // but a takeover records a new owner and there is none to record.
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // Claim as an unrelated session whose pid is long gone, so the claim reads stale.
    env.cmd(&dir)
        .env("TASKS_SESSION", "departed")
        .env("TASKS_SESSION_PID", "999999")
        .args(["start", &id])
        .assert()
        .success();

    let script = format!(
        "{}\nmkdir -p \"$RELAY_STATE_DIR\"\nchmod 700 \"$RELAY_STATE_DIR\"\n\
         printf '%s' '{{\"schema\":2,\"generation\":\"11111111-2222-4333-8444-555555555555\",\"revision\":1,\"agents\":{{}}}}' > \"$RELAY_STATE_DIR/agents.json\"\n\
         chmod 600 \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start --force {id} && echo FORCED\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("FORCED"),
        "--force must not substitute for an unresolvable identity"
    );
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("TASKS_SESSION"), "{text}");
    // The held error names the claim it was raised against, per §§5 and 6.5.
    assert!(text.contains("departed"), "{text}");
}

#[test]
fn an_acceptance_relay_off_keeps_a_harness_session_on_the_native_ladder() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // No config file: relay is off, and the registry below is never consulted.
    let state = env.home.path().join("relay-state");

    let script = format!(
        "{}\nwrite_registry\nCODEX_SESSION_ID=s1 \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(
        store.contains("session = \"s1\"") && !store.contains("codex:s1"),
        "relay off must key the claim by the native raw id: {store}"
    );
}

#[test]
fn an_unreadable_registry_fails_instead_of_reading_empty() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let dir = env.home.path().join(".config/tasks");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads through any mode; the case cannot be staged there.
    let readable = std::fs::read_to_string(dir.join("projects.toml")).is_ok();
    let out = env
        .cmd(&root)
        .args(["prime", "--all-projects"])
        .output()
        .unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        return;
    }
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "io");
}

fn start_as(env: &TestEnv, dir: &std::path::Path, id: &str, session: &str) {
    env.cmd(dir)
        .env("TASKS_SESSION", session)
        .env("TASKS_SESSION_PID", std::process::id().to_string())
        .args(["start", id])
        .assert()
        .success();
}

#[test]
fn claims_lists_a_claim_whose_task_file_the_registered_checkout_lacks() {
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let id = env.json(&root, &["add", "worktree only"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    start_as(&env, &root, &id, "probe-session");
    // A task created on a worktree branch has no file in the registered checkout.
    std::fs::remove_file(root.join(format!("tasks/{id}.md"))).unwrap();
    let prime = env.json(&root, &["prime", "--all-projects"]);
    assert!(prime["doing"].as_array().unwrap().is_empty());

    let out = env.json(&root, &["claims"]);
    let claims = out["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["id"], id.as_str());
    assert_eq!(claims[0]["prefix"], "zz");
    assert_eq!(claims[0]["session"], "probe-session");
    assert_eq!(claims[0]["live"], true);
    assert!(claims[0]["host"].is_string());
    assert!(claims[0]["worktree"].is_string());
    assert!(out.get("warnings").is_none());
}

#[test]
fn claims_reads_the_store_of_a_project_whose_checkout_is_gone() {
    let mut env = TestEnv::new();
    let here = env.init("zz");
    let gone = env.init("yy");
    let id = env.json(&gone, &["add", "elsewhere"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    start_as(&env, &gone, &id, "probe-session");
    std::fs::remove_dir_all(&gone).unwrap();

    let out = env.json(&here, &["claims", "--all-projects"]);
    let claims = out["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["id"], id.as_str());
    assert_eq!(claims[0]["prefix"], "yy");
}

#[test]
fn claims_fails_when_the_registry_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let dir = env.home.path().join(".config/tasks");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::read_to_string(dir.join("projects.toml")).is_ok();
    let out = env.cmd(&root).args(["claims"]).output().unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        return;
    }
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn claims_is_empty_when_no_store_exists() {
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let out = env.json(&root, &["claims"]);
    assert_eq!(out, serde_json::json!({ "claims": [] }));
}

#[test]
fn claims_fails_on_a_corrupt_store() {
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let store = env.claim_store("zz");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&store, "claims = [not toml").unwrap();
    assert_eq!(env.fail(&root, &["claims"]), "config");
}

#[test]
fn claims_reports_a_stale_claim_as_not_live() {
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let store = env.claim_store("zz");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    // No pid, seen long ago: the TTL path, stale.
    std::fs::write(
        &store,
        "[claims.\"zz-000001\"]\nowner = \"tester\"\nsession = \"old\"\nhost = \"h\"\n\
         worktree = \"/x\"\nstarted = \"2026-01-01T00:00:00Z\"\nseen = \"2026-01-01T00:00:00Z\"\n",
    )
    .unwrap();
    let out = env.json(&root, &["claims"]);
    assert_eq!(out["claims"][0]["id"], "zz-000001");
    assert_eq!(out["claims"][0]["live"], false);
}

#[test]
fn claims_agrees_with_prime_on_a_live_claim() {
    let mut env = TestEnv::new();
    let root = env.init("zz");
    let id = env.json(&root, &["add", "both see it"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    start_as(&env, &root, &id, "probe-session");
    let prime = env.json(&root, &["prime", "--all-projects"]);
    let claims = env.json(&root, &["claims"]);
    assert_eq!(
        prime["doing"][0]["claim"]["live"],
        claims["claims"][0]["live"]
    );
    assert_eq!(
        prime["doing"][0]["claim"]["session"],
        claims["claims"][0]["session"]
    );
}

#[test]
fn colored_list_paints_dates_by_recency() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // `add` answers {id, action, warnings}; the timestamp is on the record.
    let fresh = id_of(env.json(&dir, &["add", "Fresh"]));
    let today = env.json(&dir, &["show", &fresh])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    write_doc(
        &dir,
        "tasks/sci-a00001.md",
        "---\nid: sci-a00001\ntitle: Ancient\nstatus: todo\npriority: 2\ncreated: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\ndepends: []\ntags: []\n---\n",
    );
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "today is full cyan: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;125;125;115m2020-01-01\x1b[0m"),
        "past the horizon is the old end: {text:?}"
    );
    assert!(
        String::from_utf8(out.stderr).unwrap().is_empty(),
        "a palette from the environment needs no warning"
    );
}

#[test]
fn colored_quiet_paints_the_park_date_by_recency() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Sweep"]));
    as_agent(&env, &dir, "agent-a")
        .args([
            "park",
            &id,
            "run the sweep",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "40",
        ])
        .assert()
        .success();
    let today = env.json(&dir, &["show", &id])["park"]["at"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "quiet"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "a park minutes old is full cyan: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;163;88;159mP2\x1b[0m"),
        "the brief's priority takes the scale: {text:?}"
    );
    assert!(
        String::from_utf8(out.stderr).unwrap().is_empty(),
        "a palette from the environment needs no warning"
    );
}

#[test]
fn a_palette_without_color_changes_nothing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Fresh"]);
    let pretty = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    assert!(!String::from_utf8(pretty.stdout).unwrap().contains('\x1b'));
    let json = env
        .cmd(&dir)
        .args(["--color", "always", "list"])
        .output()
        .unwrap();
    assert!(!String::from_utf8(json.stdout).unwrap().contains('\x1b'));
}

#[test]
fn a_malformed_palette_is_a_config_error_whenever_set() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [&["list"][..], &["--pretty", "--color", "never", "list"][..]] {
        let out = env
            .cmd(&dir)
            .env("TASKS_PALETTE", "fg=#e5e3d7 bg=#13140d")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config", "{args:?}");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(
            detail.contains("TASKS_PALETTE") && detail.contains("missing cyan"),
            "{args:?}: {detail}"
        );
    }
}

#[test]
fn colored_parked_paints_the_park_date() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Paused"]));
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["park", &id, "resume the thing"]);
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list", "--parked"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let today = env.json(&dir, &["show", &id])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "{text:?}"
    );
}

#[test]
fn redirected_stdout_without_a_palette_skips_the_query_and_warns() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Fresh", "-p", "1"]);
    // The harness pipes stdout, which is exactly the redirect the policy covers.
    let out = env
        .cmd(&dir)
        .env_remove("TASKS_PALETTE")
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("38;2"), "no truecolor: {text:?}");
    assert!(text.contains("\x1b[1mP1\x1b[0m"), "P1 stays bold: {text:?}");
    for query in ["\x1b]10", "\x1b]11", "\x1b]4;", "\x1b[c"] {
        assert!(!text.contains(query), "no query bytes {query:?}: {text:?}");
    }
    let warning = String::from_utf8(out.stderr).unwrap();
    assert!(
        warning.contains(
            "theme colors off: the terminal did not report its colors (stdout is not a terminal); set TASKS_PALETTE to supply them"
        ),
        "{warning:?}"
    );
    assert!(!warning.contains("priority colors off"), "{warning:?}");
}

#[test]
fn theme_default_paints_the_builtin_palette() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let fresh = id_of(env.json(&dir, &["add", "Fresh", "-p", "1"]));
    let today = env.json(&dir, &["show", &fresh])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    write_doc(
        &dir,
        "tasks/sci-a00001.md",
        "---\nid: sci-a00001\ntitle: Ancient\nstatus: todo\npriority: 2\ncreated: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\ndepends: []\ntags: []\n---\n",
    );
    // The helpers set `TASKS_PALETTE` to the built-in's exact colors, so it comes off
    // here: the SGRs below prove the built-in path, not the injected palette.
    let out = env
        .cmd(&dir)
        .env_remove("TASKS_PALETTE")
        .env("TASKS_THEME", "default")
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "today is full cyan: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;125;125;115m2020-01-01\x1b[0m"),
        "past the horizon is the old end: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;215;95;215mP1\x1b[0m"),
        "P1 is the built-in magenta: {text:?}"
    );
    assert!(
        String::from_utf8(out.stderr).unwrap().is_empty(),
        "the built-in palette needs no warning"
    );
}

#[test]
fn the_palette_beats_the_builtin_theme() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "One", "-p", "1"]);
    let out = env
        .cmd(&dir)
        .env("TASKS_THEME", "default")
        .env(
            "TASKS_PALETTE",
            "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#ff0000",
        )
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\x1b[38;2;255;0;0mP1\x1b[0m"), "{text:?}");
    assert!(
        !text.contains("215;95;215"),
        "the built-in magenta is not used: {text:?}"
    );
}

#[test]
fn an_invalid_theme_fails_even_when_a_palette_would_win() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // `cmd` already sets the winning `TASKS_PALETTE`; the bad value must still exit 1.
    let out = env
        .cmd(&dir)
        .env("TASKS_THEME", "chartreuse")
        .args(["list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("TASKS_THEME must be terminal or default"),
        "{error}"
    );
}

/// Runs `--pretty --color always list` with stdout and stderr on a fresh pty that is
/// also the child's controlling terminal, and returns everything the child wrote,
/// bounded by a ten-second deadline. Built from `env.raw` so the child keeps the
/// helpers' environment isolation; `TASKS_THEME` is removed first and set only when
/// asked, so a `TASKS_THEME` in the suite's own environment cannot defeat the control.
/// `TASKS_PALETTE` is always removed. Nothing answers the query from the master side,
/// so the control case exercises the query's timeout path.
fn list_on_pty(env: &TestEnv, dir: &std::path::Path, theme: Option<&str>) -> String {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let mut master_fd: libc::c_int = 0;
    let mut slave_fd: libc::c_int = 0;
    // SAFETY: two valid out-pointers; default attributes.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // Both descriptors are owned from here on, so every path below — panics
    // included — closes them.
    let slave = unsafe { OwnedFd::from_raw_fd(slave_fd) };
    // SAFETY: `slave` is ours; CLOEXEC affects only the child's post-exec image,
    // and the pre-exec closure below runs before it.
    unsafe {
        libc::fcntl(
            slave.as_raw_fd(),
            libc::F_SETFD,
            libc::fcntl(slave.as_raw_fd(), libc::F_GETFD) | libc::FD_CLOEXEC,
        )
    };
    let master = unsafe { OwnedFd::from_raw_fd(master_fd) };
    // Non-blocking before spawn: a failure here involves no child to clean up, and
    // an undetected failure would leave the first read blocking past the deadline.
    // SAFETY: `master` is ours.
    assert_ne!(
        unsafe {
            libc::fcntl(
                master.as_raw_fd(),
                libc::F_SETFL,
                libc::fcntl(master.as_raw_fd(), libc::F_GETFL) | libc::O_NONBLOCK,
            )
        },
        -1,
        "making the master non-blocking failed"
    );
    let mut command = env.raw(dir);
    // `try_clone` checks the duplication and owns each descriptor immediately; no
    // unchecked raw duplicate can slip a `-1` into `from_raw_fd`.
    command
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave.try_clone().unwrap()))
        .env_remove("TASKS_PALETTE")
        .env_remove("TASKS_THEME");
    if let Some(theme) = theme {
        command.env("TASKS_THEME", theme);
    }
    // The closure takes the fd number, not the `OwnedFd`: the descriptor must stay
    // open in the parent until after `spawn`, and the child needs it during
    // pre-exec, before CLOEXEC takes effect.
    let child_slave = slave.as_raw_fd();
    // SAFETY: runs once in the forked child before exec.
    unsafe {
        command.pre_exec(move || {
            // SAFETY: libc calls in the forked child, before exec.
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: `child_slave` is a valid descriptor in the child.
            if libc::ioctl(child_slave, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.args(["--pretty", "--color", "always", "list"]);
    let mut child = command.spawn().unwrap();
    // The child has inherited what it needs; the parent's copy of `slave` must go,
    // and `command` still owns the cloned slave descriptors, so it goes too —
    // otherwise the master never sees EOF and the loop only ends at the deadline.
    drop(slave);
    drop(command);

    let deadline = Instant::now() + Duration::from_secs(10);
    // The deadline is checked every iteration, and the loop ends only when both the
    // output is done (EOF or EIO on the master) and the child has been seen to
    // exit, because either alone can lie: a child can close its stdio and live on,
    // and Linux reads EIO on a master whose slave side is gone.
    type Outcome = Result<(Vec<u8>, Option<std::process::ExitStatus>), String>;
    let outcome = (|child: &mut std::process::Child, master: &OwnedFd| -> Outcome {
        let mut bytes = Vec::new();
        let mut output_done = false;
        let mut status = None;
        loop {
            if Instant::now() >= deadline {
                return Err(format!("the pty child did not finish; wrote {bytes:?}"));
            }
            if !output_done {
                let mut buf = [0u8; 4096];
                // SAFETY: `master` is ours and `buf` outlives the call.
                let n =
                    unsafe { libc::read(master.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
                if n > 0 {
                    bytes.extend_from_slice(&buf[..usize::try_from(n).unwrap()]);
                    continue;
                } else if n == 0 {
                    output_done = true;
                } else {
                    let error = std::io::Error::last_os_error();
                    match error.raw_os_error() {
                        Some(libc::EIO) => output_done = true,
                        Some(libc::EAGAIN) => std::thread::sleep(Duration::from_millis(20)),
                        _ => return Err(format!("reading the master failed: {error}")),
                    }
                }
            }
            if status.is_none() {
                status = child.try_wait().map_err(|error| error.to_string())?;
            }
            if output_done && status.is_some() {
                return Ok((bytes, status));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })(&mut child, &master);
    let (bytes, status) = match outcome {
        Ok(pair) => pair,
        Err(message) => {
            // Every post-spawn failure terminates and reaps the child before
            // reporting; `master` and `slave` close through their own drops.
            let _ = child.kill();
            let _ = child.wait();
            panic!("{message}");
        }
    };
    let status = status.unwrap();
    assert!(status.success(), "the pty child failed");
    String::from_utf8(bytes).unwrap()
}

#[test]
fn theme_default_sends_no_osc_query_on_a_terminal() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "One", "-p", "1"]);

    let theme = list_on_pty(&env, &dir, Some("default"));
    assert!(
        theme.contains("\x1b[38;2;215;95;215mP1\x1b[0m"),
        "painted from the built-ins: {theme:?}"
    );
    assert!(!theme.contains("theme colors off"), "{theme:?}");
    for query in [
        "\x1b]10;?",
        "\x1b]11;?",
        "\x1b]4;5;?",
        "\x1b]4;6;?",
        "\x1b[c",
    ] {
        assert!(!theme.contains(query), "query sent: {query:?}");
    }

    // The control proves the observation can see a query: without `TASKS_THEME` the
    // child asks the terminal (nothing answers) and warns after the timeout.
    let terminal = list_on_pty(&env, &dir, None);
    assert!(
        terminal.contains("\x1b]10;?"),
        "the control queried: {terminal:?}"
    );
    assert!(terminal.contains("theme colors off"), "{terminal:?}");
}

/// Runs `check` in `dir` and returns its exit code and parsed findings (`null` when clean).
fn check_findings(env: &TestEnv, dir: &std::path::Path) -> (i32, serde_json::Value) {
    let out = env.cmd(dir).args(["check"]).output().unwrap();
    let findings = if out.stdout.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&out.stdout).unwrap()
    };
    (out.status.code().unwrap(), findings)
}

fn findings_of<'a>(v: &'a serde_json::Value, key: &str, kind: &str) -> Vec<&'a serde_json::Value> {
    v[key]
        .as_array()
        .map(|all| all.iter().filter(|f| f["kind"] == kind).collect())
        .unwrap_or_default()
}

/// A project whose specs and plans are kept out of git, one task linked to a spec, a
/// plan, and its `Task 1: A` step, and a linked worktree at `<main>/wt` that git therefore
/// gave no docs: `(main, side, id)`.
fn worktree_with_excluded_docs(
    env: &mut TestEnv,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    // Kept out of git, as a project whose profile does not commit its specs does.
    std::fs::write(main.join(".git/info/exclude"), "docs/\n").unwrap();
    write_doc(&main, "docs/specs/x-design.md", "# X\n");
    write_doc(
        &main,
        "docs/plans/x.md",
        "# X plan\n\n### Task 1: A\n\n### Task 2: B\n",
    );
    let id = id_of(env.json(
        &main,
        &[
            "add",
            "T",
            "-p",
            "2",
            "--spec",
            "x",
            "--plan",
            "x",
            "--step",
            "Task 1: A",
        ],
    ));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(
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
    assert!(!side.join("docs").exists(), "git carried the excluded docs");
    (main, side, id)
}

#[test]
fn check_in_a_worktree_reads_git_excluded_docs_from_the_main_checkout() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);

    // Found in main: a warning naming it, never an error, and the plan's steps are read
    // from main's copy, so an unlinked heading there is still reported.
    let (code, v) = check_findings(&env, &side);
    assert_eq!(code, 0, "{v}");
    assert!(findings_of(&v, "errors", "doc_missing").is_empty(), "{v}");
    let found = findings_of(&v, "warnings", "doc_in_main_checkout");
    assert_eq!(found.len(), 2, "{v}");
    for finding in &found {
        assert_eq!(finding["id"], id.as_str());
        let detail = finding["detail"].as_str().unwrap();
        assert!(detail.contains(main.to_str().unwrap()), "{detail}");
    }
    let unlinked = findings_of(&v, "warnings", "unlinked_step");
    assert_eq!(unlinked.len(), 1, "{v}");
    assert!(
        unlinked[0]["detail"]
            .as_str()
            .unwrap()
            .contains("Task 2: B")
    );

    // The main checkout itself behaves as before: no finding about a checkout it is.
    let (code, v) = check_findings(&env, &main);
    assert_eq!(code, 0, "{v}");
    assert!(
        findings_of(&v, "warnings", "doc_in_main_checkout").is_empty(),
        "{v}"
    );

    // A step that main's plan no longer has is still an error from the worktree.
    write_doc(&main, "docs/plans/x.md", "# X plan\n\n### Task 2: B\n");
    let (code, v) = check_findings(&env, &side);
    assert_eq!(code, 1, "{v}");
    assert_eq!(findings_of(&v, "errors", "step_missing").len(), 1, "{v}");

    // Absent from both checkouts: the error it always was.
    std::fs::remove_file(main.join("docs/specs/x-design.md")).unwrap();
    let (code, v) = check_findings(&env, &side);
    assert_eq!(code, 1, "{v}");
    let missing = findings_of(&v, "errors", "doc_missing");
    assert_eq!(missing.len(), 1, "{v}");
    assert!(
        missing[0]["detail"]
            .as_str()
            .unwrap()
            .contains("docs/specs/x-design.md"),
        "{v}"
    );
}

/// The warnings in `v` that say a document was read from the main checkout at `main`.
fn main_checkout_warnings(v: &serde_json::Value, main: &std::path::Path) -> Vec<String> {
    v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w.as_str())
        .filter(|w| w.contains("main checkout") && w.contains(main.to_str().unwrap()))
        .map(str::to_string)
        .collect()
}

#[test]
fn show_add_and_edit_in_a_worktree_read_git_excluded_docs_from_the_main_checkout() {
    let mut env = TestEnv::new();
    let (main, side, id) = worktree_with_excluded_docs(&mut env);

    // show reads the step from main's plan and points at main's copies.
    let v = env.json(&side, &["show", &id]);
    assert_eq!(v["step_found"], true, "{v}");
    for key in ["spec_path", "plan_path"] {
        let path = v[key].as_str().unwrap();
        assert!(path.starts_with(main.to_str().unwrap()), "{key}: {path}");
        assert!(std::path::Path::new(path).is_file(), "{key}: {path}");
    }
    assert_eq!(main_checkout_warnings(&v, &main).len(), 2, "{v}");

    // Keep the copies equal and older than the first write; real-clock stamps can otherwise
    // make that write look like a same-second fork of main's unchanged record.
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }

    // An edit that revalidates the step, and one that moves it, both succeed.
    let v = env.json(&side, &["edit", &id, "-p", "1"]);
    assert_eq!(main_checkout_warnings(&v, &main).len(), 1, "{v}");
    let v = env.json(&side, &["edit", &id, "--step", "Task 2: B"]);
    assert_eq!(main_checkout_warnings(&v, &main).len(), 1, "{v}");
    let v = env.json(&side, &["show", &id]);
    assert_eq!(v["task"]["step"], "Task 2: B", "{v}");
    assert_eq!(v["step_found"], true, "{v}");

    // add links by bare name and by path, finding both only in main.
    let v = env.json(
        &side,
        &[
            "add",
            "U",
            "--spec",
            "x",
            "--plan",
            "docs/plans/x.md",
            "--step",
            "Task 1: A",
        ],
    );
    assert_eq!(main_checkout_warnings(&v, &main).len(), 2, "{v}");
    let added = env.json(&side, &["show", &id_of(v)]);
    assert_eq!(added["task"]["spec"], "docs/specs/x-design.md", "{added}");
    assert_eq!(added["task"]["plan"], "docs/plans/x.md", "{added}");

    // A step main's plan lacks is still refused, and so is a doc absent from both.
    assert_eq!(
        env.fail(&side, &["edit", &id, "--step", "Task 9: Z"]),
        "validation"
    );
    assert_eq!(
        env.fail(&side, &["edit", &id, "--plan", "nosuch"]),
        "doc_not_found"
    );
    assert_eq!(
        env.fail(&side, &["edit", &id, "--plan", "docs/plans/nosuch.md"]),
        "doc_not_found"
    );

    // The main checkout itself warns about nothing.
    let v = env.json(&main, &["show", &id]);
    assert!(main_checkout_warnings(&v, &main).is_empty(), "{v}");
}

#[test]
fn attach_from_a_copy_that_is_behind_reads_nothing_and_creates_nothing() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    std::fs::write(side.join("shot.png"), b"png").unwrap();

    let by_path = stale_detail(&env, &side, &["attach", &id, "shot.png"]);
    assert_eq!(
        retry_words(&by_path),
        ["-C", main.to_str().unwrap(), "attach", &id, "shot.png"]
    );

    let out = env
        .cmd(&side)
        .args(["attach", &id, "-", "--name", "in.txt"])
        .write_stdin("hello")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy");
    let by_stdin = error["error"]["detail"].as_str().unwrap().to_string();
    assert!(
        by_stdin.ends_with("; supply the same input on stdin"),
        "{by_stdin}"
    );
    assert!(!side.join(format!("tasks/files/{id}")).exists());

    // Both retries, run verbatim from the same directory, land in the newer checkout: the
    // relative path still resolves, and stdin is supplied again.
    env.cmd(&side)
        .args(retry_words(&by_path))
        .assert()
        .success();
    env.cmd(&side)
        .args(retry_words(&by_stdin))
        .write_stdin("hello")
        .assert()
        .success();
    let files = main.join(format!("tasks/files/{id}"));
    assert_eq!(std::fs::read(files.join("shot.png")).unwrap(), b"png");
    assert_eq!(std::fs::read(files.join("in.txt")).unwrap(), b"hello");
}

#[test]
fn edit_body_from_stdin_on_a_copy_that_is_behind_asks_for_the_same_input() {
    let mut missing_hints = Vec::new();
    for body_args in [&["--body", "-"][..], &["--body=-"], &["-b-"]] {
        let mut env = TestEnv::new();
        let (main, side, id) = repo_with_worktree(&mut env);
        stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
        stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
        let out = env
            .cmd(&side)
            .args(["edit", &id])
            .args(body_args)
            .write_stdin("new body\n")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        let detail = error["error"]["detail"].as_str().unwrap();
        if !detail.ends_with("; supply the same input on stdin") {
            missing_hints.push(body_args);
        }
        env.cmd(&side)
            .args(retry_words(detail))
            .write_stdin("new body\n")
            .assert()
            .success();
        let shown = env.json(&main, &["show", &id]);
        assert!(
            shown["task"]["body"].as_str().unwrap().contains("new body"),
            "{shown}"
        );
    }
    assert!(
        missing_hints.is_empty(),
        "missing stdin hints: {missing_hints:?}"
    );
}

#[test]
fn detach_of_a_leftover_file_from_a_copy_that_is_behind_keeps_the_file() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    std::fs::write(main.join("shot.png"), b"png").unwrap();
    env.json(&main, &["attach", &id, "shot.png"]);
    env.json(&main, &["detach", &id, "shot.png", "wrong file"]);
    // An interrupted detach leaves the ledger saying "detached" with the file still there;
    // a rerun only removes the file and never saves the record.
    let leftover = main.join(format!("tasks/files/{id}/shot.png"));
    std::fs::create_dir_all(leftover.parent().unwrap()).unwrap();
    std::fs::write(&leftover, b"png").unwrap();
    let created = "2026-09-01T00:00:00Z";
    stamp(&main, &id, created, "2026-09-05T09:00:00Z");
    stamp(&side, &id, created, "2026-09-07T10:00:00Z");
    stale_detail(&env, &main, &["detach", &id, "shot.png", "wrong file"]);
    assert!(leftover.exists(), "the refusal removed the file");
}

#[test]
fn a_sibling_that_moves_ahead_while_the_editor_is_open_refuses_and_keeps_the_edit() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    // While the editor is open, the worktree writes. The edit also forges a far-future
    // stamp in its own copy, which must not count: the baseline is what was loaded.
    let sibling = side.join(format!("tasks/{id}.md"));
    let editor = editor_script(
        &main,
        &format!(
            "sed -i 's/^updated: .*/updated: 2030-01-01T00:00:00Z/' '{}' && \
             sed -i 's/^updated: .*/updated: 2031-01-01T00:00:00Z/; s/^title: T$/title: Edited/' \"$1\"",
            sibling.display()
        ),
    );
    let out = env
        .cmd(&main)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("fresh editor"), "{detail}");
    let retry = retry_words(detail);
    assert_eq!(retry, ["-C", side.to_str().unwrap(), "edit", &id]);
    let kept = detail
        .split("edit kept at ")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("the kept file is named");
    assert!(
        std::fs::read_to_string(kept)
            .unwrap()
            .contains("title: Edited")
    );
    assert_eq!(env.json(&main, &["show", &id])["task"]["title"], "T");
    let recovery = editor_script(&main, &format!("cp '{kept}' \"$1\""));
    env.cmd(&main)
        .env("EDITOR", recovery)
        .args(retry)
        .assert()
        .success();
    assert_eq!(env.json(&side, &["show", &id])["task"]["title"], "Edited");
    assert!(std::path::Path::new(kept).is_file());
}

#[test]
fn a_feedback_recurrence_onto_a_copy_behind_the_owners_worktree_refuses_both_ways() {
    let mut env = TestEnv::new();
    let owner = env.init("own");
    accept_feedback(&owner, "the own tool");
    git(&owner, &["init", "-q", "-b", "main"]);
    let reporter = env.init("rep");
    let report = [
        "feedback",
        "--project",
        "own",
        "slow startup",
        "--category",
        "friction",
    ];
    let id = env.json(&reporter, &report)["id"]
        .as_str()
        .unwrap()
        .to_string();
    git(&owner, &["add", "-A"]);
    git(&owner, &["commit", "-qm", "seed"]);
    let side = owner.join("wt");
    git(
        &owner,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    stamp(&owner, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let file = format!("tasks/{id}.md");
    let before = env.read(&owner, &file);

    let mut explicit = report.to_vec();
    explicit.extend(["--recur", id.as_str()]);
    // The same title again is the automatic match; --recur names it outright.
    for args in [report.to_vec(), explicit] {
        let detail = stale_detail(&env, &reporter, &args);
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(
            detail.ends_with("nothing was written. Rerun with --new to file a separate entry"),
            "{detail}"
        );
        assert_eq!(env.read(&owner, &file), before);
    }
}

fn claim_worktree(env: &TestEnv, dir: &std::path::Path, id: &str) -> String {
    env.json(dir, &["show", id])["claim"]["worktree"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_holders_writes_move_its_claim_to_the_checkout_they_land_in() {
    let mut failures = Vec::new();
    for command in ["note", "edit", "editor", "dep", "attach", "detach"] {
        let mut env = TestEnv::new();
        let (main, side, id) = started_then_branched(&mut env);
        let before = env.json(&main, &["show", &id])["claim"].clone();
        assert_eq!(claim_worktree(&env, &main, &id), main.display().to_string());
        let mut cmd = as_agent(&env, &side, "agent-a");
        match command {
            "note" => {
                cmd.args(["note", &id, "working here"]);
            }
            "edit" => {
                cmd.args(["edit", &id, "-p", "1"]);
            }
            "editor" => {
                let editor = editor_script(&side, "sed -i 's/^priority: 2$/priority: 1/' \"$1\"");
                cmd.env("EDITOR", editor).args(["edit", &id]);
            }
            "dep" => {
                let dependency = id_of(env.json(&side, &["add", "Dependency"]));
                cmd.args(["dep", &id, "--on", &dependency]);
            }
            "attach" => {
                cmd.args(["attach", &id, "-", "--name", "shot.png"])
                    .write_stdin("image");
            }
            "detach" => {
                as_agent(&env, &side, "agent-b")
                    .args(["attach", &id, "-", "--name", "shot.png"])
                    .write_stdin("image")
                    .assert()
                    .success();
                cmd.args(["detach", &id, "shot.png", "finished"]);
            }
            _ => unreachable!(),
        }
        cmd.assert().success();
        let after = env.json(&main, &["show", &id])["claim"].clone();

        let mut expected = before;
        expected["worktree"] = side.display().to_string().into();
        expected["seen"] = after["seen"].clone();
        if after != expected {
            failures.push(format!("{command}: expected {expected}, got {after}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn another_sessions_write_leaves_the_claim_where_it_is() {
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &side, "agent-b")
        .args(["note", &id, "passing through"])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &main, &id), main.display().to_string());
}

#[test]
fn a_restart_in_the_new_worktree_makes_a_later_write_in_main_refuse() {
    // tasks-142d2f with the protocol step: the note in main now refuses.
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &side, &id), side.display().to_string());
    // Another session writing from main is told who works there, with no retry.
    let detail = stale_detail(&env, &main, &["note", &id, "from main"]);
    assert!(
        detail.starts_with(&format!("tasks/{id}.md in {} is newer", side.display())),
        "{detail}"
    );
    assert!(
        detail.contains("agent-a") && !detail.contains("tasks -C"),
        "{detail}"
    );
}

#[test]
fn a_holders_feedback_recurrence_moves_the_claim_to_the_owners_checkout() {
    let mut env = TestEnv::new();
    let owner = env.init("own");
    accept_feedback(&owner, "the own tool");
    git(&owner, &["init", "-q", "-b", "main"]);
    let reporter = env.init("rep");
    let report = [
        "feedback",
        "--project",
        "own",
        "slow startup",
        "--category",
        "friction",
    ];
    let id = env.json(&reporter, &report)["id"]
        .as_str()
        .unwrap()
        .to_string();
    git(&owner, &["add", "-A"]);
    git(&owner, &["commit", "-qm", "seed"]);
    let side = owner.join("wt");
    git(
        &owner,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &side, &id), side.display().to_string());
    // The owner's main checkout holds the newer copy, so the recurrence may land there.
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&owner, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let mut explicit = report.to_vec();
    explicit.extend(["--recur", id.as_str()]);
    as_agent(&env, &reporter, "agent-a")
        .args(&explicit)
        .assert()
        .success();
    assert_eq!(
        claim_worktree(&env, &owner, &id),
        owner.display().to_string()
    );
}

#[test]
fn a_claim_held_by_proof_moves_with_its_holder() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "Thing", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    let state = relay_on(&env);
    // Start under relay identity, commit, branch, then lose the registry: ownership can
    // only come from the claim's recorded proof.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         git add -A && git -c user.name=t -c user.email=t@e commit -qm start\n\
         git worktree add -q -b side '{}'\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" -C '{}' note {id} 'by proof'\n",
        shim_env(&state, "codex", "s1"),
        side.display(),
        side.display()
    );
    let out = common::harness_shim(&main, env.home.path(), "codex", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(claim_worktree(&env, &main, &id), side.display().to_string());
}

#[test]
fn show_reads_a_worktree_only_task_from_the_checkout_its_claim_or_park_names() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let claimed = id_of(env.json(&side, &["add", "Claimed there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &claimed])
        .assert()
        .success();
    let v = env.json(&main, &["show", &claimed]);
    assert_eq!(v["task"]["title"], "Claimed there", "{v}");
    assert!(
        warnings_of(&v).contains(&format!(
            "{claimed} exists only in {}; shown from that checkout",
            side.display()
        )),
        "{v}"
    );

    let parked = id_of(env.json(&side, &["add", "Parked there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &parked])
        .assert()
        .success();
    as_agent(&env, &side, "agent-a")
        .args(["park", &parked, "pick it up"])
        .assert()
        .success();
    let v = env.json(&main, &["show", &parked]);
    assert_eq!(v["task"]["title"], "Parked there", "{v}");
    assert_eq!(v["park"]["next_step"], "pick it up", "{v}");
}

#[test]
fn show_of_another_projects_worktree_only_task_falls_back_through_its_claims() {
    let mut env = TestEnv::new();
    let here = env.init("sci");
    let (_, side, _) = repo_with_worktree_as(&mut env, "oth");
    let id = id_of(env.json(&side, &["add", "Over there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&here, &["show", &id]);
    assert_eq!(v["task"]["title"], "Over there", "{v}");
    assert!(
        warnings_of(&v).iter().any(|w| w.contains("exists only in")),
        "{v}"
    );
}

#[test]
fn show_names_the_checkout_when_the_one_holding_the_task_is_gone() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Lost", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    git(
        &main,
        &["worktree", "remove", "--force", side.to_str().unwrap()],
    );
    let error = error_of(&env, &main, &["show", &id]);
    assert_eq!(error["error"]["kind"], "task_not_found", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!(
            "claimed in {}, which is unavailable",
            side.display()
        )),
        "{detail}"
    );
}

#[test]
fn show_reads_the_local_copy_even_when_the_claimed_one_is_newer() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&main, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo", "{v}");
    assert!(
        !warnings_of(&v).iter().any(|w| w.contains("exists only in")),
        "{v}"
    );
}
