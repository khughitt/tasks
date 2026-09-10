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
    assert_eq!(
        env.fail(&sci, &["park", &id, "x", "--waiting-on", "nobody"]),
        "validation"
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
    // The setup add took the lock; reads must not create it again.
    std::fs::remove_file(&lock).unwrap();

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
        serde_json::json!([{ "tag": "fam-only", "count": 1, "projects": { "fam": 1 } }])
    );
}

#[test]
fn project_and_all_projects_conflict_on_every_read_command() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    for command in ["list", "ready", "next", "prime", "tree", "tags"] {
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
    env.json(&sci, &["start", &id]);
    let path = sci.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("status: doing", "status: done")
            .replace("updated: ", "last_done: 2026-01-01T00:00:00Z\nupdated: "),
    )
    .unwrap();

    let value = env.json(&sci, &["done", &id, "landed"]);
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

    let v = env.json(&sci, &["check"]);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            w["kind"] == "retired_prefix" && w["detail"].as_str().unwrap().contains(&far)
        }),
        "{v}"
    );
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    assert_eq!(
        env.json(&fam, &["check"])["warnings"],
        serde_json::json!([])
    );
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
        env.json(&sci, &["show", &a])["task"]["depends"]
            .as_array()
            .unwrap()
            .is_empty()
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
        "project  idea  todo  doing  blocked  total  activity"
    );
    // header-width columns, two-space gutters, counts right-aligned under their labels
    assert_eq!(
        lines.next().unwrap(),
        format!(
            "{:<7}  {:>4}  {:>4}  {:>5}  {:>7}  {:>5}  {day}",
            "sci", 1, 0, 0, 0, 2
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
            "{:<7}  {:>4}  {:>4}  {:>5}  {:>7}  {:>5}  unreachable",
            "fam", "-", "-", "-", "-", "-"
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
        "project  idea  todo  doing  blocked  done  dropped  total  activity"
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
        "idea 0  todo 1  doing 0  blocked 0  total 2"
    );

    let opened = env.pretty(&sci, &["prime", "--closed"]);
    assert_eq!(
        opened.lines().nth(1).unwrap(),
        "idea 0  todo 1  doing 0  blocked 0  done 1  dropped 0  total 2"
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
    // total and activity carry no status, so nothing after the last reset is painted
    let tail = row.rsplit("\u{1b}[0m").next().unwrap();
    assert!(tail.starts_with("      1  2"), "{row:?}");
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
    assert_eq!(
        env.fail(nowhere.path(), &["projects", "--sort", "updated"]),
        "validation"
    );
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
    assert_eq!(row["depends"], serde_json::json!([]));
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
    let main = env.init("sci");
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

#[test]
fn a_write_warns_when_another_checkout_holds_a_newer_copy() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);

    // The reported sequence: the main checkout moved on after the worktree branched, so
    // the worktree is about to close a record that is missing what main already wrote.
    // Explicit stamps -- `updated` has second precision, and a real-clock race here would
    // make the test flaky rather than wrong.
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");

    let v = env.json(&side, &["done", &id, "landed"]);
    let expected = format!("tasks/{id}.md in {} is newer", main.display());
    let warnings = warnings_of(&v);
    assert!(
        warnings.iter().any(|w| w.starts_with(&expected)
            && w.contains("2026-09-07T10:00:00Z")
            && w.contains("2026-09-05T09:00:00Z")
            && w.contains("reconcile")),
        "{warnings:?}"
    );
    // Advisory only: the write still lands.
    assert_eq!(env.json(&side, &["show", &id])["task"]["status"], "done");
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
    let v = env.json(&side.join("sub"), &["note", &id, "in the worktree"]);
    let expected = format!("tasks/{id}.md in {} is newer", sub.display());
    let warnings = warnings_of(&v);
    assert!(
        warnings.iter().any(|w| w.starts_with(&expected)),
        "{warnings:?}"
    );
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
fn a_hand_edited_updated_stamp_cannot_suppress_the_warning() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");

    // The baseline is the stamp of the copy on disk, not whatever the editor left behind:
    // a far-future `updated:` would otherwise make every sibling look stale.
    let editor = editor_script(
        &main,
        "sed -i 's/^updated: .*/updated: 2030-01-01T00:00:00Z/; s/^title: T$/title: Edited/' \"$1\"",
    );
    let out = env
        .cmd(&main)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let warnings = warnings_of(&v);
    assert!(
        warnings.iter().any(|w| w.contains("is newer")),
        "{warnings:?}"
    );
    let saved = env.json(&main, &["show", &id]);
    assert_eq!(saved["task"]["title"], "Edited");
    assert_ne!(saved["task"]["updated"], "2030-01-01T00:00:00Z");
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
    // and a local project is still required
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.fail(nowhere.path(), &["note", &piece, "x"]),
        "no_project"
    );
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
    assert_eq!(tags(&env, &id), serde_json::json!([]));

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

    // absent is null, never a missing key
    let plain = id_of(env.json(&dir, &["add", "Plain"]));
    let shown = env.json(&dir, &["show", &plain]);
    assert!(shown["task"].get("source").is_some(), "{shown}");
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
    let check = env.json(&dir, &["check"]);
    assert_eq!(check["errors"], serde_json::json!([]), "{check}");

    // the flag completes; nothing in complete.rs mentions it
    let flags = env.complete(&dir, "bash", 3, &["tasks", "add", "T", "--sou"]);
    assert_eq!(flags, ["--source"]);
    let flags = env.complete(&dir, "bash", 3, &["tasks", "edit", &plain, "--no-s"]);
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
    assert_eq!(plain_row["source"], serde_json::Value::Null);
    let ready = env.json(&dir, &["ready"]);
    assert!(
        ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r.get("source").is_some()),
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

    // every status on edit, but only the two `add` accepts
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
            4,
            &["tasks", "feedback", "S", "--category", ""]
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
    let sci = env.init("sci");
    let open = id_of(env.json(&upstream, &["add", "Open report", "--tag", "feedback"]));
    let closed = id_of(env.json(&upstream, &["add", "Closed report", "--tag", "feedback"]));
    env.json(&upstream, &["done", &closed, "fixed"]);
    let untagged = id_of(env.json(&upstream, &["add", "Not feedback"]));

    let ids = env.complete(&sci, "bash", 4, &["tasks", "feedback", "S", "--recur", ""]);
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
        4,
        &["tasks", "feedback", "S", "--recur", ""],
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
            &["feedback", "Original report", "--category", "gap", "--new"],
        );
        let id = original["id"].as_str().unwrap();
        let original_raw = env.read(&target, &format!("tasks/{id}.md"));
        let source_held = hold_project_lock(&env, "sci");
        let held = hold_project_lock(&env, "tasks");
        let mut command = env.raw(&source);
        command.args(["feedback", "Another report", "--category", "gap"]);
        if recur {
            command.args(["--recur", id]);
        } else {
            command.arg("--new");
        }
        let mut child = command.spawn().unwrap();
        let blocked = !wait_bounded(&mut child, Duration::from_millis(300));
        let new_id = id.replacen("tasks-", "tracker-", 1);
        std::fs::write(target.join("tasks/.config.toml"), "prefix = \"tracker\"\n").unwrap();
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
            &["feedback", "Third report", "--category", "gap", "--new"],
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
    // Seeing its lock descriptor proves the child constructed its old-prefix context
    // and reached lock acquisition. A sleep alone could leave it not yet started.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(
            adder.try_wait().unwrap().is_none(),
            "the add exited before acquiring its lock"
        );
        let opened = std::fs::read_dir(format!("/proc/{}/fd", adder.id()))
            .unwrap()
            .any(|entry| {
                // Other descriptors can close while /proc is being inspected.
                std::fs::read_link(entry.unwrap().path()).is_ok_and(|path| path == lock_path)
            });
        if opened {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the add never opened its lock"
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
fn rename_interrupted_after_the_store_write_resumes_and_a_tampered_destination_refuses() {
    for tamper in [false, true] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
        as_agent(&env, &dir, "agent-a")
            .args(["park", &id, "a"])
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
            assert!(!env.claim_store("dot").exists());
            assert_eq!(
                env.json(&dir, &["prime"])["parked"][0]["id"],
                format!("dots-{}", &id[4..])
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
    let stopped = env
        .raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "claims")
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(!env.claim_store("dot").exists(), "source removed");
    assert_eq!(
        env.json(&dir, &["rename", "dot", "dots", "--explain"])["parks"],
        1
    );
    let resumed = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(resumed["recovery"], "resume_cleanup");
    assert_eq!(resumed["parks"], 1);
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
                    env.json(&dir, &["check"])["errors"]
                        .as_array()
                        .unwrap()
                        .is_empty(),
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
            &["feedback", "Original", "--category", "gap", "--new"],
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
            vec!["feedback", "Late", "--category", "gap", "--new"],
            vec!["feedback", "Late", "--category", "gap", "--recur", &current],
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
    assert!(
        env.json(&dir, &["check"])["errors"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let check = env.json(&foreign, &["check"]);
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

    let v = env.json(&dir, &["sample", "-n", "10", "--older-than", "0"]);
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
    for bad in ["36501", "10000000", "18446744073709551615", "-1"] {
        let out = env
            .cmd(&dir)
            .args(["sample", "--older-than", bad])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "--older-than {bad} must be a parse error"
        );
    }
    let v = env.json(&dir, &["sample", "--older-than", "36500"]);
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

    // no cadence: null everywhere
    let plain = id_of(env.json(&sci, &["add", "One off"]));
    assert_eq!(
        env.json(&sci, &["show", &plain]).get("periodic"),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        env.json(&sci, &["list"])["tasks"][0].get("periodic"),
        Some(&serde_json::Value::Null)
    );
    // close it so it cannot compete for the head of `ready` below
    env.json(&sci, &["done", &plain]);

    // open with a cadence: present, but no pending recurrence (spec §4.1)
    let open = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    let v = env.json(&sci, &["show", &open]);
    assert_eq!(v["periodic"]["every"], "30d");
    assert!(v["periodic"]["last_done"].is_null());
    assert!(v["periodic"]["due"].is_null());
    assert_eq!(v["periodic"]["due_now"], false);

    // closed and anchored: a computable date, not yet due
    env.json(&sci, &["start", &open]);
    env.json(&sci, &["done", &open]);
    let v = env.json(&sci, &["show", &open]);
    assert!(v["periodic"]["last_done"].is_string());
    assert!(v["periodic"]["due"].as_str().unwrap().ends_with('Z'));
    assert_eq!(v["periodic"]["due_now"], false);

    // closed and unanchored: due now, with no computable date -- the case `due: null`
    // alone cannot express (spec §5.4)
    let due = id_of(env.json(&sci, &["add", "Overdue"]));
    env.json(&sci, &["done", &due]);
    env.json(&sci, &["edit", &due, "--every", "30d"]);
    let v = env.json(&sci, &["show", &due]);
    assert!(v["periodic"]["due"].is_null());
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
    let v = env.json(&sci, &["check"]);
    let kinds: Vec<&str> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["kind"].as_str().unwrap())
        .collect();
    assert!(!kinds.contains(&"open_child_of_closed_parent"), "{v}");

    // An ordinary open child of a closed parent is still warned about.
    let plain = id_of(env.json(&sci, &["add", "Plain", "--parent", &goal]));
    let v = env.json(&sci, &["check"]);
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
