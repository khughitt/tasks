mod common;
use common::TestEnv;

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
fn tasks_format_env_must_be_valid() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let out = env
        .cmd(&dir)
        .env("TASKS_FORMAT", "xml")
        .args(["list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

fn write_doc(dir: &std::path::Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn id_of(value: serde_json::Value) -> String {
    value["id"].as_str().unwrap().to_string()
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
    assert_eq!(s["task"]["notes"], serde_json::json!([]));
    assert_eq!(s["spec_path"], serde_json::Value::Null);
    assert_eq!(s["step_found"], serde_json::Value::Null);
    assert_eq!(s["depends_on"], serde_json::json!([]));
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
    assert_eq!(env.fail(&dir, &["add", "x", "-p", "9"]), "validation");
    assert_eq!(
        env.fail(&dir, &["add", "x", "--size", "huge"]),
        "validation"
    );
    assert_eq!(
        env.fail(&dir, &["add", "x", "--status", "done"]),
        "validation"
    );
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
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"].as_array().unwrap().len(), 0, "{check}");
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
    assert_eq!(env.fail(&dir, &["list", "--status", "weird"]), "validation");
    let summary = &v["tasks"][0];
    for key in [
        "id", "title", "status", "priority", "size", "owner", "updated", "tags", "depends",
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
    assert!(
        roadmap.contains("1 open task(s) without children are listed under ready"),
        "{roadmap}"
    );
}

fn editor_script(dir: &std::path::Path, body: &str) -> String {
    let p = dir.join("editor.sh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.display().to_string()
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
    assert_eq!(
        env.json(&dir, &["show", &c])["task"]["depends"],
        serde_json::json!([])
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
    assert_eq!(env.fail(&dir, &["graph", "--format", "png"]), "validation");
}

#[test]
fn check_passes_clean_repo_and_reports_drift() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    write_doc(&dir, "docs/plans/2026-08-29-p.md", "### Task 1: one\n");
    let a = env.json(&dir, &["add", "A", "--plan", "p", "--step", "Task 1: one"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = env.json(&dir, &["add", "B", "--depends", &a])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let v = env.json(&dir, &["check"]);
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
    env.json(&dir, &["add", "A", "--plan", "p", "--step", "Task 1: one"]);
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warnings = check["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{check}");
    assert_eq!(warnings[0]["kind"], "unlinked_step");
    assert_eq!(warnings[0]["file"], "docs/plans/2026-09-03-p.md");
    assert_eq!(warnings[0]["id"], serde_json::Value::Null);
    assert!(
        warnings[0]["detail"]
            .as_str()
            .unwrap()
            .contains("Task 2: two")
    );
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
    let v = env.json(&sci, &["check"]); // exit 0: warnings only
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
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]));
    let warning = &check["warnings"].as_array().unwrap()[0];
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
    assert_eq!(shown["children"], serde_json::json!([]));
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
}
