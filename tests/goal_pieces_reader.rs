//! Reader-release contracts for declared goal pieces: raw fixtures stand in for the
//! writers, which are not enabled yet (spec 2026-11-17-goal-pieces-design).
mod common;
use common::{TestEnv, editor_script, findings_of, id_of};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Inserts `key: [values]` before the record's `tags` line, as a hand edit would.
fn raw(env: &TestEnv, dir: &Path, id: &str, key: &str, values: &[&str]) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = env.read(dir, &format!("tasks/{id}.md"));
    std::fs::write(
        path,
        text.replacen(
            "\ntags:",
            &format!("\n{key}: [{}]\ntags:", values.join(", ")),
            1,
        ),
    )
    .unwrap();
}

/// Inserts `line` before the record's `depends` line.
fn raw_line(env: &TestEnv, dir: &Path, id: &str, line: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = env.read(dir, &format!("tasks/{id}.md"));
    std::fs::write(
        path,
        text.replacen("\ndepends:", &format!("\n{line}\ndepends:"), 1),
    )
    .unwrap();
}

fn add(env: &TestEnv, dir: &Path, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["add", title];
    args.extend_from_slice(extra);
    id_of(env.json(dir, &args))
}

/// A hub in `aa` that depends on `pieces` and declares them.
fn hub(env: &TestEnv, aa: &Path, title: &str, extra: &[&str], pieces: &[&str]) -> String {
    let hub = add(env, aa, title, extra);
    for piece in pieces {
        env.json(aa, &["dep", &hub, "--on", piece]);
    }
    raw(env, aa, &hub, "pieces", pieces);
    hub
}

struct Two {
    env: TestEnv,
    aa: PathBuf,
    bb: PathBuf,
}

fn two() -> Two {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    Two { env, aa, bb }
}

fn ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|row| {
            row.get("id")
                .or_else(|| row.get("task").and_then(|task| task.get("id")))
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("no id: {row}"))
                .to_string()
        })
        .collect()
}

fn warnings(value: &Value) -> Vec<String> {
    value["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect()
}

fn stderr_of(env: &TestEnv, dir: &Path, args: &[&str]) -> String {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{args:?} should fail: {out:?}");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn editor_fail(env: &TestEnv, dir: &Path, id: &str, sed: &str) -> String {
    let script = editor_script(dir, &format!("sed -i '{sed}' \"$1\""));
    let out = env
        .cmd(dir)
        .env("EDITOR", script)
        .args(["edit", id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "editor save should fail: {out:?}"
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn pieces_reader_roundtrip() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let plain = add(&env, &aa, "Plain", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    assert_eq!(env.json(&aa, &["show", &hub])["task"]["pieces"], json!([x]));
    assert!(
        env.json(&aa, &["show", &plain])["task"]
            .get("pieces")
            .is_none()
    );

    let line = format!("pieces: [{x}]\n");
    env.json(&aa, &["edit", &hub, "-p", "1"]);
    assert!(env.read(&aa, &format!("tasks/{hub}.md")).contains(&line));

    raw(&env, &aa, &plain, "pieces", &["bad-id"]);
    let out = env.cmd(&aa).args(["show", &plain]).output().unwrap();
    assert!(!out.status.success(), "{out:?}");
}

#[test]
fn pieces_reader_hub_leaves_ready_and_unassessed() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let ready = ids(&env.json(&aa, &["ready"])["tasks"]);
    assert!(!ready.contains(&hub), "{ready:?}");
    let review = ids(&env.json(&aa, &["list", "--needs-review"])["tasks"]);
    assert!(!review.contains(&hub), "{review:?}");
    let next = env.json(&aa, &["next"]);
    assert_ne!(next["next"]["task"]["id"], json!(hub), "{next}");

    // Without pieces the same record is an ordinary leaf, as today.
    let path = aa.join(format!("tasks/{hub}.md"));
    let text = env.read(&aa, &format!("tasks/{hub}.md"));
    std::fs::write(&path, text.replace(&format!("pieces: [{x}]\n"), "")).unwrap();
    env.json(&bb, &["done", &x, "done"]);
    let ready = ids(&env.json(&aa, &["ready"])["tasks"]);
    assert!(ready.contains(&hub), "{ready:?}");
    let review = ids(&env.json(&aa, &["list", "--needs-review"])["tasks"]);
    assert!(review.contains(&hub), "{review:?}");
}

#[test]
fn pieces_reader_closeout_waits_for_pieces() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Prerequisite", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);

    let prime = env.json(&aa, &["prime"]);
    assert!(!ids(&prime["closeout"]).contains(&hub), "{prime}");
    assert!(
        !warnings(&prime).iter().any(|w| w.contains(&hub)),
        "{prime}"
    );

    env.json(&bb, &["done", &x, "done"]);
    let prime = env.json(&aa, &["prime"]);
    assert_eq!(ids(&prime["closeout"]), [hub.clone()], "{prime}");

    env.json(&aa, &["dep", &hub, "--on", &y]);
    let prime = env.json(&aa, &["prime"]);
    assert!(!ids(&prime["closeout"]).contains(&hub), "{prime}");
    let expected = format!(
        "{hub} has no open children or pieces but is held by open prerequisites: {y}; it joins \
         closeout when they close"
    );
    assert!(warnings(&prime).contains(&expected), "{prime}");
}

#[test]
fn pieces_reader_childless_lane_stays_out_of_closeout() {
    let Two { env, aa, .. } = two();
    let lane = add(&env, &aa, "Lane", &[]);
    env.json(&aa, &["edit", &lane, "--lane"]);
    let prime = env.json(&aa, &["prime"]);
    assert!(!ids(&prime["closeout"]).contains(&lane), "{prime}");
}

#[test]
fn pieces_reader_shelved_piece_holds_silently_check_names_it() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    env.json(&bb, &["shelve", &x, "--defer", "30d", "later"]);

    let prime = env.json(&aa, &["prime"]);
    assert!(!ids(&prime["closeout"]).contains(&hub), "{prime}");
    assert!(
        !warnings(&prime).iter().any(|w| w.contains(&hub)),
        "{prime}"
    );
    let (_, check) = common::check_findings(&env, &aa);
    let shelved: Vec<&Value> = ["errors", "warnings"]
        .iter()
        .flat_map(|key| findings_of(&check, key, "shelved_dep"))
        .collect();
    assert!(
        shelved
            .iter()
            .any(|f| f.to_string().contains(&hub) && f.to_string().contains(&x)),
        "{check}"
    );
}

#[test]
fn pieces_reader_all_dropped_hub_closes() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &["--origin", "user"], &[&x]);
    env.json(&bb, &["drop", &x, "no longer needed"]);
    let prime = env.json(&aa, &["prime"]);
    assert_eq!(ids(&prime["closeout"]), [hub.clone()], "{prime}");
    env.json(&aa, &["done", &hub, "met"]);
}

#[test]
fn pieces_reader_dep_rm_unmarks_piece() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let out = env.json(&aa, &["dep", &hub, "--rm", &x]);
    let expected =
        format!("{x} was a piece of {hub}; removing the dependency unmarked it as a piece");
    assert!(warnings(&out).contains(&expected), "{out}");
    let task = &env.json(&aa, &["show", &hub])["task"];
    assert!(task.get("pieces").is_none(), "{task}");
    assert!(task.get("depends").is_none(), "{task}");
}

#[test]
fn pieces_reader_no_depends_refuses_stranding() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let before = env.read(&aa, &format!("tasks/{hub}.md"));
    let err = stderr_of(&env, &aa, &["edit", &hub, "--no-depends"]);
    assert!(
        err.contains(&format!(
            "declares pieces ({x}); --no-depends would leave them outside depends"
        )),
        "{err}"
    );
    assert_eq!(env.read(&aa, &format!("tasks/{hub}.md")), before);
    env.json(&aa, &["edit", &hub, "--no-depends", "--depends", &x]);
    assert_eq!(env.json(&aa, &["show", &hub])["task"]["pieces"], json!([x]));
}

#[test]
fn pieces_reader_edit_lane_refuses() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let err = stderr_of(&env, &aa, &["edit", &hub, "--lane"]);
    assert!(
        err.contains(&format!(
            "{hub} declares pieces ({x}); a lane holds only local children, so it cannot carry pieces"
        )),
        "{err}"
    );
}

#[test]
fn pieces_reader_editor_guard() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let hub = add(&env, &aa, "Hub", &[]);
    env.json(&aa, &["dep", &hub, "--on", &x, "--on", &y]);
    raw(&env, &aa, &hub, "pieces", &[&x]);

    let added = editor_fail(
        &env,
        &aa,
        &hub,
        &format!("s/^pieces: .*/pieces: [{x}, {y}]/"),
    );
    assert!(
        added.contains("pieces writers are not enabled yet"),
        "{added}"
    );
    let removed = editor_fail(&env, &aa, &hub, "/^pieces: /d");
    assert!(
        removed.contains("pieces writers are not enabled yet"),
        "{removed}"
    );
    let unlinked = editor_fail(&env, &aa, &hub, &format!("s/^depends: .*/depends: [{y}]/"));
    assert!(
        unlinked.contains(&format!(
            "piece {x} is not a dependency of {hub}; a piece must stay a dependency"
        )),
        "{unlinked}"
    );
}

#[test]
fn pieces_reader_editor_body_save_keeps_pieces() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let line = format!("pieces: [{x}]\n");
    let script = editor_script(
        &aa,
        "perl -0pi -e 's/\\n---\\n/\\n---\\n\\nMore body.\\n/' \"$1\"",
    );
    env.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &hub, "--editor"])
        .assert()
        .success();
    let text = env.read(&aa, &format!("tasks/{hub}.md"));
    assert!(text.contains(&line), "{text}");
    assert!(text.contains("More body."), "{text}");
}

#[test]
fn pieces_reader_goal_refusals() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let err = stderr_of(&env, &aa, &["edit", &hub, "--every", "1w"]);
    assert!(
        err.contains(&format!(
            "{hub} declares pieces ({x}) and cannot be a recurrence; a task with pieces is a goal, \
             and a goal is never ready"
        )),
        "{err}"
    );
    let err = stderr_of(&env, &aa, &["edit", &hub, "--defer", "3d"]);
    assert!(
        err.contains(&format!(
            "{hub} declares pieces ({x}) and cannot be deferred; a task with pieces is a goal, \
             and a goal is never ready"
        )),
        "{err}"
    );

    raw_line(&env, &aa, &hub, "every: 7d");
    let (_, check) = common::check_findings(&env, &aa);
    let periodic = findings_of(&check, "errors", "periodic_goal");
    assert!(
        periodic.iter().any(|f| f["detail"]
            == format!(
                "has a cadence and pieces ({x}); a goal is never ready, so the cadence can never fire"
            )),
        "{check}"
    );

    let other = hub_with_defer(&env, &aa, &x);
    let (_, check) = common::check_findings(&env, &aa);
    let deferred = findings_of(&check, "errors", "deferred_goal");
    assert!(
        deferred.iter().any(|f| f["detail"]
            == format!(
                "is deferred and has pieces ({x}); a goal is never ready, so the deferral hides nothing"
            )
            && f.to_string().contains(&other)),
        "{check}"
    );
}

fn hub_with_defer(env: &TestEnv, aa: &Path, x: &str) -> String {
    let other = hub(env, aa, "Deferred hub", &[], &[x]);
    raw_line(env, aa, &other, "defer: 2099-01-01");
    other
}

#[test]
fn pieces_reader_blockers_classify_hub_as_goal() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    let x = add(&env, &bb, "Piece", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    let waiter = add(&env, &cc, "Waiter", &[]);
    env.json(&cc, &["dep", &waiter, "--on", &hub]);

    // Out of scope from cc: the hub is a boundary row.
    let local = env.json(&cc, &["blockers"]);
    let row = local["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == hub)
        .unwrap_or_else(|| panic!("no hub row: {local}"));
    assert_eq!(row["goal"], true, "{row}");

    // In scope: the hub counts as a goal, not work, against its piece.
    let wide = env.json(&aa, &["blockers", "--all-projects"]);
    let piece = wide["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == x)
        .unwrap_or_else(|| panic!("no piece row: {wide}"));
    assert_eq!(piece["direct_work_count"], 0, "{piece}");
    assert_eq!(piece["direct_goal_count"], 1, "{piece}");

    let text = env.pretty(&aa, &["blockers", "--all-projects"]);
    // The plan's one footer sentence, split at its semicolon to fit 120 columns.
    for line in [
        "goals: structural only (lanes, records with children, and records that declare pieces)\n",
        "\na childless cross-project hub without declared pieces counts as work\n",
    ] {
        assert!(text.contains(line), "{line}: {text}");
    }
}
