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
