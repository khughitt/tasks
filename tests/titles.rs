//! Concise titles: `long_title` and `tldr` on records, their CLI flags, matching and
//! JSON rows (records spec docs/specs/2026-11-26-titles-and-headings-design.md).

mod common;
use common::{TestEnv, editor_script, feedback_env, id_of};
use std::path::Path;

/// The record format that admits `long_title` and `tldr`.
const TITLES_FORMAT: &str = "format: 3";

fn record(dir: &Path, id: &str) -> String {
    std::fs::read_to_string(dir.join(format!("tasks/{id}.md"))).unwrap()
}

#[test]
fn add_with_tldr_and_long_title_writes_titles_format() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(
        &sci,
        &[
            "add",
            "Goal headings",
            "--tldr",
            "Scaled headings",
            "--long-title",
            "Make goals stand out",
        ],
    ));
    let task = &env.json(&sci, &["show", &id])["task"];
    assert_eq!(task["tldr"], "Scaled headings");
    assert_eq!(task["long_title"], "Make goals stand out");
    let text = record(&sci, &id);
    assert_eq!(text.lines().nth(1), Some(TITLES_FORMAT), "{text}");
}

#[test]
fn add_without_new_keys_stays_format_1() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Plain"]));
    let text = record(&sci, &id);
    assert!(!text.contains("\nformat:"), "{text}");
    let task = &env.json(&sci, &["show", &id])["task"];
    assert!(
        task.get("tldr").is_none() && task.get("long_title").is_none(),
        "{task}"
    );
}

#[test]
fn edit_no_tldr_keeps_format() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T"]));
    env.json(&sci, &["edit", &id, "--tldr", "x"]);
    assert!(record(&sci, &id).contains(TITLES_FORMAT));
    env.json(&sci, &["edit", &id, "--no-tldr"]);
    let text = record(&sci, &id);
    assert!(text.contains(TITLES_FORMAT), "{text}");
    assert!(!text.contains("tldr:"), "{text}");
}

#[test]
fn edit_title_unbounded() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T"]));
    let long = "w".repeat(120);
    env.json(&sci, &["edit", &id, "--title", &long]);
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["title"],
        long.as_str()
    );
}

#[test]
fn editor_promotes_on_new_keys() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T"]));
    let editor = editor_script(env.home.path(), "sed -i '/^title: /a tldr: x' \"$1\"");
    env.cmd(&sci)
        .args(["edit", &id, "--editor"])
        .env("EDITOR", &editor)
        .assert()
        .success();
    let text = record(&sci, &id);
    assert_eq!(text.lines().nth(1), Some(TITLES_FORMAT), "{text}");
    assert_eq!(env.json(&sci, &["show", &id])["task"]["tldr"], "x");
}

#[test]
fn editor_refuses_entries_heading_on_promotion() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-b", "Intro\n\n## Entries\n\nLiteral."]));
    let before = record(&sci, &id);
    let editor = editor_script(env.home.path(), "sed -i '/^title: /a tldr: x' \"$1\"");
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--editor"])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("## Entries"), "{stderr}");
    assert_eq!(record(&sci, &id), before);
}

#[test]
fn edit_editor_flag_conflicts_with_tldr() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T"]));
    for flag in ["--tldr", "--long-title"] {
        let out = env
            .cmd(&sci)
            .args(["edit", &id, "--editor", flag, "x"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{flag}");
    }
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--editor", "--no-tldr"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn editor_manual_format_change_still_refused() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T"]));
    let editor = editor_script(
        env.home.path(),
        "sed -i '2i format: 3' \"$1\"; sed -i '/^title: /a tldr: x' \"$1\"",
    );
    let out = env
        .cmd(&sci)
        .args(["edit", &id, "--editor"])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("format is managed by tasks"), "{stderr}");
}

#[test]
fn feedback_tldr_on_create_promotes() {
    let (env, target, reporter) = feedback_env();
    let id = id_of(env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "tasks",
            "The listing is slow on big projects",
            "--category",
            "friction",
            "--tldr",
            "Listing bottleneck",
        ],
    ));
    let task = &env.json(&target, &["show", &id])["task"];
    assert_eq!(task["title"], "The listing is slow on big projects");
    assert_eq!(task["tldr"], "Listing bottleneck");
    assert_eq!(record(&target, &id).lines().nth(1), Some(TITLES_FORMAT));
}

#[test]
fn sourced_add_matches_given_long_title_against_old_title() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let old = id_of(env.json(&sci, &["add", "Paste line one", "--source", "S"]));
    let out = env.json(
        &sci,
        &[
            "add",
            "Different concise",
            "--source",
            "S",
            "--long-title",
            "Paste line one",
        ],
    );
    assert_eq!(out["action"], "reused", "{out}");
    assert_eq!(out["id"], old.as_str());
}

#[test]
fn sourced_add_matches_given_title_against_existing_long_title() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let old = id_of(env.json(
        &sci,
        &[
            "add",
            "Concise",
            "--source",
            "S",
            "--long-title",
            "Paste line one",
        ],
    ));
    let out = env.json(&sci, &["add", "Paste line one", "--source", "S"]);
    assert_eq!(out["action"], "reused", "{out}");
    assert_eq!(out["id"], old.as_str());
}

#[test]
fn feedback_recurrence_matches_long_title() {
    let (env, target, reporter) = feedback_env();
    let report = "Listing output is slow on large projects";
    let file = |env: &TestEnv| {
        env.json(
            &reporter,
            &[
                "feedback",
                "--project",
                "tasks",
                report,
                "--category",
                "friction",
            ],
        )
    };
    let id = id_of(file(&env));
    env.json(
        &target,
        &["edit", &id, "--title", "Short", "--long-title", report],
    );
    let again = file(&env);
    assert_eq!(again["id"], id.as_str(), "{again}");
    assert_ne!(again["action"], "created", "{again}");
}

#[test]
fn search_matches_long_title_and_tldr() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let by_long = id_of(env.json(&sci, &["add", "A", "--long-title", "Make goals stand out"]));
    let by_tldr = id_of(env.json(&sci, &["add", "B", "--tldr", "Headings that stand out"]));
    env.json(&sci, &["add", "C"]);
    let out = env.json(&sci, &["search", "stand out"]);
    let mut ids: Vec<&str> = out["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    ids.sort();
    let mut want = vec![by_long.as_str(), by_tldr.as_str()];
    want.sort();
    assert_eq!(ids, want, "{out}");
}

/// Every row in `out` (recursively) whose `id` is `id`.
fn rows_for<'a>(out: &'a serde_json::Value, id: &str, found: &mut Vec<&'a serde_json::Value>) {
    match out {
        serde_json::Value::Object(map) => {
            if map.get("id").and_then(|v| v.as_str()) == Some(id) && map.contains_key("title") {
                found.push(out);
            }
            for value in map.values() {
                rows_for(value, id, found);
            }
        }
        serde_json::Value::Array(items) => {
            for value in items {
                rows_for(value, id, found);
            }
        }
        _ => {}
    }
}

fn assert_keys(out: &serde_json::Value, id: &str, what: &str) {
    let mut found = Vec::new();
    rows_for(out, id, &mut found);
    assert!(!found.is_empty(), "{what}: no row for {id}: {out}");
    for row in found {
        assert_eq!(row["tldr"], "Short tldr", "{what}: {row}");
        assert_eq!(row["long_title"], "Long sentence title", "{what}: {row}");
    }
}

const KEYS: [&str; 4] = [
    "--tldr",
    "Short tldr",
    "--long-title",
    "Long sentence title",
];

fn with_keys<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut all = args.to_vec();
    all.extend_from_slice(&KEYS);
    all
}

#[test]
fn json_rows_carry_long_title_and_tldr() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lane = id_of(env.json(&sci, &with_keys(&["add", "Lane", "--lane"])));
    let goal = id_of(env.json(
        &sci,
        &with_keys(&["add", "Goal", "--parent", &lane, "--origin", "user"]),
    ));
    let leaf = id_of(env.json(&sci, &with_keys(&["add", "Leaf", "--parent", &goal])));
    let blocker = id_of(env.json(&sci, &with_keys(&["add", "Blocker"])));
    env.json(&sci, &["add", "Waits", "--depends", &blocker]);
    let parked = id_of(env.json(&sci, &with_keys(&["add", "Parked"])));
    env.json(&sci, &["start", &parked]);
    env.json(
        &sci,
        &[
            "park",
            &parked,
            "answer the question",
            "--waiting-on",
            "user",
            "--reason",
            "decision",
            "--decision-kind",
            "question",
            "--decision-origin",
            "workflow",
            "--decision-text",
            "Which?",
        ],
    );
    let quiet = id_of(env.json(&sci, &with_keys(&["add", "Quiet"])));
    env.json(&sci, &["start", &quiet]);
    env.json(
        &sci,
        &[
            "park",
            &quiet,
            "run it",
            "--reason",
            "quiet",
            "--minutes",
            "5",
        ],
    );

    assert_keys(&env.json(&sci, &["list"]), &leaf, "list");
    assert_keys(&env.json(&sci, &["tree"]), &goal, "tree");
    let prime = env.json(&sci, &["prime"]);
    assert_keys(&prime, &lane, "prime lanes");
    assert_keys(&prime, &goal, "prime roadmap");
    assert_keys(&env.json(&sci, &["lanes"]), &lane, "lanes");
    assert_keys(&env.json(&sci, &["ready"]), &leaf, "ready");
    assert_keys(
        &env.json(&sci, &["goals", "--project", "sci"]),
        &goal,
        "goals",
    );
    assert_keys(&env.json(&sci, &["list", "--pending"]), &parked, "pending");
    assert_keys(&env.json(&sci, &["list", "--parked"]), &parked, "parked");
    assert_keys(
        &env.json(&sci, &["quiet", "--project", "sci"]),
        &quiet,
        "quiet",
    );
    assert_keys(&env.json(&sci, &["blockers"]), &blocker, "blockers");
}

#[test]
fn json_rows_omit_absent_keys() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lane = id_of(env.json(&sci, &["add", "Lane", "--lane"]));
    env.json(&sci, &["add", "Leaf", "--parent", &lane]);
    for args in [
        vec!["list"],
        vec!["tree"],
        vec!["prime"],
        vec!["lanes"],
        vec!["ready"],
    ] {
        let out = env.json(&sci, &args);
        let text = out.to_string();
        assert!(
            !text.contains("\"tldr\"") && !text.contains("\"long_title\""),
            "{args:?}: {text}"
        );
    }
}

#[test]
fn prime_summary_omits_long_title_and_tldr() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lane = id_of(env.json(&sci, &with_keys(&["add", "Lane", "--lane"])));
    env.json(&sci, &with_keys(&["add", "Goal", "--parent", &lane]));
    let out = env.json(&sci, &["prime", "--summary"]);
    let text = out.to_string();
    assert!(
        !text.contains("\"tldr\"") && !text.contains("\"long_title\""),
        "{text}"
    );
}

#[test]
fn text_sizing_env_value_is_validated() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let out = env
        .cmd(&sci)
        .args(["list", "--pretty"])
        .env("TASKS_TEXT_SIZING", "big")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("TASKS_TEXT_SIZING"), "{stderr}");
}

#[test]
fn osc66_appears_only_under_the_gating_rules() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lane = id_of(env.json(&sci, &["add", "Lane", "--lane"]));
    env.json(&sci, &["add", "Goal", "--parent", &lane]);
    let run = |args: &[&str], sizing: Option<&str>| {
        let mut cmd = env.cmd(&sci);
        cmd.args(args);
        if let Some(mode) = sizing {
            cmd.env("TASKS_TEXT_SIZING", mode);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{args:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    let osc = "\x1b]66;";
    let tree = ["--pretty", "--color", "auto", "tree"];
    assert!(run(&tree, Some("always")).contains(osc));
    assert!(!run(&tree, None).contains(osc), "auto is off when piped");
    assert!(!run(&tree, Some("never")).contains(osc));
    assert!(!run(&["--pretty", "--color", "never", "tree"], Some("always")).contains(osc));
    assert!(
        !run(&["tree"], Some("always")).contains(osc),
        "never in JSON"
    );
    assert!(run(&["--pretty", "--color", "auto", "lanes"], Some("always")).contains(osc));
}
