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
            { "tag": "testing", "count": 2, "projects": { "sci": 2 } },
            { "tag": "perf", "count": 1, "projects": { "sci": 1 } }
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
        serde_json::json!([{ "tag": "legacy", "count": 1, "projects": { "fam": 1 } }])
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
fn read_commands_do_not_take_the_mutation_lock() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let lock = env.claim_store("sci").with_file_name("sci.lock");

    env.json(&sci, &["show", &id]);
    env.json(&sci, &["check"]);
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

    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "show", id])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let marker = "\n\x1b[31m# step MISSING\x1b[0m\n";
    let at = text
        .find(marker)
        .unwrap_or_else(|| panic!("no painted step marker: {text:?}"));
    assert!(
        !text[..at].contains("\x1b["),
        "the serialized task text stays plain: {:?}",
        &text[..at]
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
    assert_eq!(env.fail(&dir, &["list", "--status", "weird"]), "validation");
    let summary = &v["tasks"][0];
    for key in [
        "id", "title", "status", "priority", "size", "owner", "created", "updated", "tags",
        "depends",
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
        &["feedback", "prime is fast", "--category", "positive"],
    );
    let shown = env.json(&target, &["show", out["id"].as_str().unwrap()]);
    assert_eq!(shown["task"]["tags"][2], "from:tasks");

    let out = env
        .cmd(&target)
        .args(["--pretty", "feedback", "pretty check", "--category", "idea"])
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
            &["feedback", "probe summary", "--category", "gap"]
        ),
        "config"
    );
    let target = env.init("tasks");
    std::fs::remove_file(target.join("tasks/.config.toml")).unwrap();
    assert_eq!(
        env.fail(
            &reporter,
            &["feedback", "probe summary", "--category", "gap"]
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
            &["feedback", "probe summary", "--category", "gap"]
        ),
        "config",
        "a registry entry pointing at a project with another prefix is refused"
    );
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(
            nowhere.path(),
            &["feedback", "probe summary", "--category", "gap"]
        ),
        "no_project"
    );
    assert_eq!(
        env.fail(
            &reporter,
            &["feedback", "probe summary", "--category", "rant"]
        ),
        "validation"
    );
    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
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
            &["feedback", "another automatic report", "--category", "gap"]
        ),
        "parse"
    );
    std::fs::remove_file(target.join("tasks/tasks-bad.md")).unwrap();

    let unrelated = env.json(
        &reporter,
        &[
            "feedback",
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
        env.fail(&reporter, &["feedback", "a !", "--category", "gap"]),
        "validation"
    );

    assert_eq!(
        env.fail(
            &reporter,
            &[
                "feedback",
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
fn feedback_recurrence_serializes_against_concurrent_recurrences() {
    let (env, target, reporter) = feedback_env();
    let id = env.json(
        &reporter,
        &["feedback", "the thing is slow", "--category", "friction"],
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

    let json = env
        .cmd(&dir)
        .args(["--color", "always", "check"])
        .output()
        .unwrap();
    assert!(!has_ansi(&json.stdout));
    serde_json::from_slice::<serde_json::Value>(&json.stdout).unwrap();
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
    [
        "\x1b[0m",
        "\x1b[1m",
        "\x1b[2m",
        "\x1b[31m",
        "\x1b[32m",
        "\x1b[33m",
        "\x1b[34m",
        "\x1b[2;31m",
        "\x1b[2;32m",
    ]
    .into_iter()
    .fold(text.to_string(), |text, code| text.replace(code, ""))
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
    assert!(colored.contains("\x1b[1mP0\x1b[0m"));
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
    assert!(text.contains("sci") && text.contains("idea 1"), "{text}");

    std::fs::write(fam.join("tasks/.config.toml"), "not toml = [").unwrap();
    assert_eq!(env.fail(nowhere.path(), &["projects"]), "config");

    // the shared registry warnings apply here too
    let fresh = TestEnv::new();
    let v = fresh.json(nowhere.path(), &["projects"]);
    assert_eq!(v["projects"], serde_json::json!([]));
    assert_eq!(v["warnings"], serde_json::json!(["registry is empty"]));
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
fn re_running_a_close_retries_a_failed_release() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    let store = env.claim_store("sci");

    as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    use std::os::unix::fs::PermissionsExt;
    // The existing lock is writable, but a new atomic store temp file cannot be created.
    let state_dir = store.parent().unwrap();
    let original = std::fs::metadata(state_dir).unwrap().permissions();
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-a")
        .args(["done", &id, "landed"])
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
            w.contains("re-run the same command to retry the release")
        }),
        "{result}"
    );
    let shown = env.json(&sci, &["show", &id]);
    assert_eq!(shown["task"]["status"], "done");
    assert_eq!(shown["claim"]["session"], "agent-a");
    assert_eq!(shown["claim"]["live"], true);

    // `start --force` cannot recover this: can_transition rejects done -> doing.
    let out = as_agent(&env, &sci, "agent-a")
        .args(["start", "--force", &id])
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "invalid_transition");

    // Re-running the closing command can, because a same-status transition still releases.
    as_agent(&env, &sci, "agent-a")
        .args(["done", &id, "landed"])
        .assert()
        .success();
    assert!(env.json(&sci, &["show", &id])["claim"].is_null());
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

    // Read still works; `atomic_write` cannot create its temp file.
    let tasks_dir = sci.join("tasks");
    let original = std::fs::metadata(&tasks_dir).unwrap().permissions();
    std::fs::set_permissions(&tasks_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = as_agent(&env, &sci, "agent-a")
        .args(["start", &id])
        .output();
    std::fs::set_permissions(&tasks_dir, original).unwrap();
    let out = out.unwrap();
    assert_eq!(out.status.code(), Some(1));

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
    assert_eq!(env.fail(&dir, &["list", "--sort", "weird"]), "validation");

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
