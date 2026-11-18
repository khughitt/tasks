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
    assert_eq!(
        ids(&prime["closeout"]),
        std::slice::from_ref(&hub),
        "{prime}"
    );

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
    assert_eq!(
        ids(&prime["closeout"]),
        std::slice::from_ref(&hub),
        "{prime}"
    );
    let out = env.json(&aa, &["goals"]);
    let row = goal_row(&out, &hub);
    assert_eq!(row["pieces"], json!({"done": 0, "total": 0}), "{row}");
    assert!(row.get("dependencies").is_none(), "{row}");
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
    // Writer stage: an editor save may change pieces, but never break the subset rule.
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let z = add(&env, &bb, "Not a dependency", &[]);
    let hub = add(&env, &aa, "Hub", &[]);
    env.json(&aa, &["dep", &hub, "--on", &x, "--on", &y]);
    raw(&env, &aa, &hub, "pieces", &[&x]);

    let added = editor_fail(
        &env,
        &aa,
        &hub,
        &format!("s/^pieces: .*/pieces: [{x}, {z}]/"),
    );
    assert!(
        added.contains(&format!(
            "piece {z} is not a dependency of {hub}; a piece must stay a dependency"
        )),
        "{added}"
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

// ---- goals: membership in every goal number (spec §6) ----

fn goal_row<'a>(out: &'a Value, id: &str) -> &'a Value {
    out["goals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no goal row for {id}: {out}"))
}

/// Rewrites the `<field>: ` frontmatter line of `tasks/<id>.md`, or inserts it before the
/// closing `---` when absent.
fn set_field(dir: &Path, id: &str, field: &str, value: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("{field}:");
    let mut lines: Vec<String> = Vec::new();
    let (mut fence, mut found) = (0, false);
    for line in text.lines() {
        if line == "---" {
            fence += 1;
            if fence == 2 && !found {
                lines.push(format!("{field}: {value}"));
                found = true;
            }
            lines.push(line.to_string());
            continue;
        }
        if fence == 1 && line.starts_with(&prefix) {
            found = true;
            lines.push(format!("{field}: {value}"));
            continue;
        }
        lines.push(line.to_string());
    }
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

fn ago(days: i64) -> String {
    let at = time::OffsetDateTime::now_utc() - time::Duration::days(days);
    at.replace_nanosecond(0)
        .unwrap()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn line_of<'a>(text: &'a str, id: &str) -> &'a str {
    text.lines()
        .find(|line| line.split_whitespace().next() == Some(id))
        .unwrap_or_else(|| panic!("no line for {id}: {text}"))
}

/// Goal `aa-g` (user) with done child `aa-c`, done piece `bb-p` with one open child
/// `bb-q`, and open prerequisite `cc-r` that is not a piece.
struct Fixture {
    env: TestEnv,
    aa: PathBuf,
    bb: PathBuf,
    cc: PathBuf,
    g: String,
    p: String,
    q: String,
}

fn fixture() -> Fixture {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    let p = add(&env, &bb, "Piece", &[]);
    let q = add(&env, &bb, "Piece child", &["--parent", &p]);
    let r = add(&env, &cc, "Prerequisite", &[]);
    let g = hub(&env, &aa, "Goal", &["--origin", "user"], &[&p]);
    env.json(&aa, &["dep", &g, "--on", &r]);
    let c = add(&env, &aa, "Child", &["--parent", &g]);
    env.json(&aa, &["done", &c, "done"]);
    set_field(&bb, &p, "status", "done");
    Fixture {
        env,
        aa,
        bb,
        cc,
        g,
        p,
        q,
    }
}

#[test]
fn pieces_reader_goals_progress_and_prereqs() {
    let f = fixture();
    let out = f.env.json(&f.aa, &["goals"]);
    let row = goal_row(&out, &f.g);
    assert_eq!(row["pieces"], json!({"done": 2, "total": 3}), "{row}");
    assert_eq!(row["dependencies"], json!({"open": 1, "total": 1}), "{row}");
}

#[test]
fn pieces_reader_goals_piece_prereq_enters_count() {
    let f = fixture();
    let s = add(&f.env, &f.cc, "Piece prerequisite", &[]);
    f.env.json(&f.bb, &["dep", &f.q, "--on", &s]);
    let out = f.env.json(&f.aa, &["goals"]);
    let row = goal_row(&out, &f.g);
    assert_eq!(row["dependencies"], json!({"open": 2, "total": 2}), "{row}");
}

#[test]
fn pieces_reader_goals_unresolved_piece_in_unres() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let g = hub(&env, &aa, "Goal", &["--origin", "user"], &[&x]);
    let before = goal_row(&env.json(&aa, &["goals"]), &g)["pieces"].clone();
    let missing = format!("bb-{}", if x.ends_with('0') { "fffff1" } else { "fffff0" });
    set_field(&aa, &g, "depends", &format!("[{x}, {missing}, zz-000001]"));
    set_field(&aa, &g, "pieces", &format!("[{x}, {missing}, zz-000001]"));
    let out = env.json(&aa, &["goals"]);
    let row = goal_row(&out, &g);
    assert_eq!(row["dependencies"]["unresolved"], 2, "{row}");
    assert_eq!(row["pieces"], before, "{row}");
    assert!(
        warnings(&out).contains(&"project zz is not registered on this host".to_string()),
        "{out}"
    );
    let text = env.pretty(&aa, &["goals", "--color", "never"]);
    assert!(line_of(&text, &g).contains("?2"), "{text}");
}

#[test]
fn pieces_reader_goals_subgoal_pieces_count_for_umbrella() {
    let Two { env, aa, bb } = two();
    let p = add(&env, &bb, "Piece", &[]);
    let u = add(&env, &aa, "Umbrella", &["--origin", "user"]);
    let s = hub(&env, &aa, "Sub-goal", &["--parent", &u], &[&p]);
    let out = env.json(&aa, &["goals"]);
    let row = goal_row(&out, &u);
    // The sub-goal and its piece: both open.
    assert_eq!(row["pieces"], json!({"done": 0, "total": 2}), "{row}");
    assert!(row.get("dependencies").is_none(), "{row}");
    let _ = s;
}

#[test]
fn pieces_reader_goals_scope_narrowing_stable() {
    let f = fixture();
    let parked = add(&f.env, &f.bb, "Parked on user", &["--parent", &f.p]);
    let old = add(&f.env, &f.bb, "Old doing", &["--parent", &f.p]);
    f.env.json(&f.bb, &["edit", &old, "--status", "doing"]);
    let _ = std::fs::remove_file(f.env.claim_store("bb"));
    set_field(&f.bb, &old, "updated", &ago(10));
    common::write_park(&f.env, "bb", &parked, "s1", "user", "/w");
    let narrow = f.env.json(&f.aa, &["goals", "--project", "aa"]);
    let wide = f.env.json(&f.aa, &["goals", "--all-projects"]);
    let (narrow, wide) = (goal_row(&narrow, &f.g), goal_row(&wide, &f.g));
    for key in ["pieces", "dependencies", "last_activity", "attention"] {
        assert_eq!(narrow[key], wide[key], "{key}: {narrow} vs {wide}");
    }
    assert_eq!(wide["attention"]["you"]["first"], json!(parked), "{wide}");
    assert_eq!(wide["attention"]["stalled"]["first"], json!(old), "{wide}");
}

#[test]
fn pieces_reader_goals_paused_lane_dedup() {
    let Two { env, aa, bb } = two();
    let lane_b = add(&env, &bb, "B lane", &["--lane"]);
    let m1 = add(&env, &bb, "M1", &["--parent", &lane_b]);
    let m2 = add(&env, &bb, "M2", &["--parent", &lane_b]);
    let lane_a = add(&env, &aa, "A lane", &["--lane"]);
    let g = hub(
        &env,
        &aa,
        "Goal",
        &["--origin", "user", "--parent", &lane_a],
        &[&m1, &m2],
    );
    env.json(&bb, &["block", &lane_b, "why"]);
    env.json(&aa, &["block", &lane_a, "why"]);
    set_field(&aa, &lane_a, "updated", &ago(10));
    set_field(&bb, &lane_b, "updated", &ago(5));
    let out = env.json(&aa, &["goals"]);
    let row = goal_row(&out, &g);
    assert_eq!(row["attention"]["blocked"]["count"], 2, "{row}");
    let text = env.pretty(&aa, &["goals", "--color", "never"]);
    assert!(
        squash(&text).contains(&format!("2 blocked ({lane_a}, paused lane)")),
        "{text}"
    );
}

#[test]
fn pieces_reader_goals_last_activity_from_piece() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let g = hub(&env, &aa, "Goal", &["--origin", "user"], &[&x]);
    set_field(&aa, &g, "updated", &ago(20));
    let stamp = ago(1);
    set_field(&bb, &x, "updated", &stamp);
    let out = env.json(&aa, &["goals"]);
    assert_eq!(goal_row(&out, &g)["last_activity"], json!(stamp), "{out}");
}

#[test]
fn pieces_reader_foreign_scan_error_names_project() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    hub(&env, &aa, "Goal", &["--origin", "user"], &[&x]);
    std::fs::write(
        bb.join("tasks/bb-ffffff.md"),
        "---\nid: bb-ffffff\ntitle: [\n---\n",
    )
    .unwrap();
    let err = stderr_of(&env, &aa, &["goals", "--project", "aa"]);
    assert!(err.contains("project bb"), "{err}");
}

#[test]
fn pieces_reader_alias_spelling_counts_once() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    assert!(x.starts_with("bb-"), "{x}");
    let g = hub(&env, &aa, "Goal", &["--origin", "user"], &[&x]);
    env.json(&bb, &["done", &x, "done"]);
    env.json(&bb, &["rename", "bb", "cc"]);
    let stored = env.read(&aa, &format!("tasks/{g}.md"));
    assert!(stored.contains(&format!("depends: [{x}]\n")), "{stored}");
    assert!(stored.contains(&format!("pieces: [{x}]\n")), "{stored}");
    let out = env.json(&aa, &["goals"]);
    let row = goal_row(&out, &g);
    assert_eq!(row["pieces"], json!({"done": 1, "total": 1}), "{row}");
    assert!(row.get("dependencies").is_none(), "{row}");
}

#[test]
fn pieces_reader_goals_skip_unshown_goals() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let mm = env.init("mm");
    let m = add(&env, &mm, "Piece", &[]);
    let mut goals = Vec::new();
    for title in ["Dropped", "Shelved", "Old done"] {
        let g = add(&env, &aa, title, &["--origin", "user"]);
        set_field(&aa, &g, "depends", &format!("[{m}, zz-000001]"));
        set_field(&aa, &g, "pieces", &format!("[{m}, zz-000001]"));
        goals.push(g);
    }
    set_field(&aa, &goals[0], "status", "dropped");
    set_field(&aa, &goals[1], "status", "shelved");
    set_field(&aa, &goals[1], "defer", "2099-01-01");
    set_field(&aa, &goals[2], "status", "done");
    set_field(&aa, &goals[2], "completed", &ago(30));
    set_field(&aa, &goals[2], "closed", &ago(30));
    std::fs::write(
        mm.join("tasks/mm-ffffff.md"),
        "---\nid: mm-ffffff\ntitle: [\n---\n",
    )
    .unwrap();
    for args in [
        &["goals", "--project", "aa"][..],
        &["goals", "--project", "aa", "--closed"],
    ] {
        let out = env.json(&aa, args);
        assert!(
            !warnings(&out)
                .iter()
                .any(|w| w.contains("project zz") || w.contains("project mm")),
            "{args:?}: {out}"
        );
    }
}

#[test]
fn pieces_reader_goals_no_duplicate_unreachable_warning() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let nn = env.init("nn");
    let g = add(&env, &aa, "Goal", &["--origin", "user"]);
    set_field(&aa, &g, "depends", "[nn-000001]");
    set_field(&aa, &g, "pieces", "[nn-000001]");
    std::fs::remove_dir_all(&nn).unwrap();
    let out = env.json(&aa, &["goals", "--all-projects"]);
    let about: Vec<String> = warnings(&out)
        .into_iter()
        .filter(|w| w.contains("project nn"))
        .collect();
    assert_eq!(about.len(), 1, "{out}");
    assert_eq!(goal_row(&out, &g)["dependencies"]["unresolved"], 1, "{out}");
}

#[test]
fn pieces_reader_goals_local_unregistered_and_empty_projects() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let _ee = env.init("ee");
    let xx = env.init("xx");
    // Unregister xx: its checkout stays, the registry forgets it.
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    registry["projects"].as_table_mut().unwrap().remove("xx");
    std::fs::write(&path, toml::to_string(&registry).unwrap()).unwrap();
    let g = add(&env, &aa, "Goal", &["--origin", "user"]);
    set_field(&aa, &g, "depends", "[ee-000001]");
    set_field(&aa, &g, "pieces", "[ee-000001]");
    for dir in [&aa, &xx] {
        let out = env.json(dir, &["goals"]);
        assert!(
            !warnings(&out)
                .iter()
                .any(|w| w.contains("is not registered on this host")),
            "{out}"
        );
        assert_eq!(goal_row(&out, &g)["dependencies"]["unresolved"], 1, "{out}");
    }

    // Running prime/tree --pretty inside unregistered xx walks a goal in xx through
    // Membership::new's local-seed path without treating xx as an unregistered foreign prefix.
    let foreign_done = add(&env, &aa, "Foreign done piece", &[]);
    env.json(&aa, &["done", &foreign_done, "done"]);
    let local_goal = hub(
        &env,
        &xx,
        "Local goal",
        &["--origin", "user"],
        &[&foreign_done],
    );
    let _local_child = add(&env, &xx, "Local open child", &["--parent", &local_goal]);
    for cmd in ["prime", "tree"] {
        let out = env
            .cmd(&xx)
            .args([cmd, "--pretty", "--color", "never"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{cmd}: {out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("is not registered on this host"),
            "{cmd} emitted unregistered warning for local seed:\n{stderr}"
        );
        assert!(
            line_of(&stdout, &local_goal).contains("1/2"),
            "{cmd} missing 1/2 work cell:\n{stdout}"
        );
    }
}

#[test]
fn pieces_reader_goals_legend() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    hub(&env, &aa, "Goal", &["--origin", "user"], &[&x]);
    let text = env.pretty(&aa, &["goals", "--color", "never"]);
    assert!(
        squash(&text).contains("work: done/total descendants and declared pieces"),
        "{text}"
    );
}

// ---- show, check, move and migrate (spec §7) ----

#[test]
fn pieces_reader_show_labels_piece_rows() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Prerequisite", &[]);
    let hub = hub(&env, &aa, "Hub", &[], &[&x]);
    env.json(&aa, &["dep", &hub, "--on", &y]);
    let labelled = |env: &TestEnv| {
        let text = env.pretty(&aa, &["show", &hub, "--color", "never"]);
        let piece = text
            .lines()
            .any(|line| line.starts_with("piece") && line.contains(&x));
        let dep = text
            .lines()
            .any(|line| line.starts_with("depends on") && line.contains(&y));
        assert!(piece && dep, "{text}");
    };
    labelled(&env);
    let shown = env.json(&aa, &["show", &hub]);
    for row in shown["depends_on"].as_array().unwrap() {
        assert!(row.get("piece").is_none(), "{row}");
    }
    env.json(&bb, &["rename", "bb", "cc"]);
    let stored = env.read(&aa, &format!("tasks/{hub}.md"));
    assert!(x.starts_with("bb-"), "{x}");
    assert!(stored.contains(&format!("pieces: [{x}]\n")), "{stored}");
    labelled(&env);
}

fn finding_on(check: &Value, level: &str, kind: &str, id: &str) -> Option<Value> {
    findings_of(check, level, kind)
        .into_iter()
        .find(|f| f["id"] == id)
        .cloned()
}

#[test]
fn pieces_reader_check_findings() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let local = add(&env, &aa, "Local", &[]);

    let not_dep = add(&env, &aa, "Not a dependency", &[]);
    raw(&env, &aa, &not_dep, "pieces", &[&x]);
    let own = add(&env, &aa, "Own project piece", &[]);
    env.json(&aa, &["dep", &own, "--on", &local]);
    raw(&env, &aa, &own, "pieces", &[&local]);
    let lane = add(&env, &aa, "Lane", &["--lane"]);
    env.json(&aa, &["dep", &lane, "--on", &x]);
    raw(&env, &aa, &lane, "pieces", &[&x]);
    let selfish = add(&env, &aa, "Self", &[]);
    raw(&env, &aa, &selfish, "pieces", &[&selfish]);
    let twice = add(&env, &aa, "Twice", &[]);
    env.json(&aa, &["dep", &twice, "--on", &x]);
    raw(&env, &aa, &twice, "pieces", &[&x, &x]);
    let gone = add(&env, &aa, "Moved away", &[]);
    env.json(&aa, &["move", &gone, "--to", "bb"]);
    raw(&env, &aa, &gone, "pieces", &[&x]);

    let (_, check) = common::check_findings(&env, &aa);
    for (level, kind, id) in [
        ("errors", "piece_not_dependency", &not_dep),
        ("errors", "local_piece", &own),
        ("errors", "lane_pieces", &lane),
        ("warnings", "self_piece", &selfish),
        ("warnings", "duplicate_piece", &twice),
        ("errors", "piece_status", &gone),
    ] {
        assert!(
            finding_on(&check, level, kind, id).is_some(),
            "{level} {kind} on {id}: {check}"
        );
    }

    // A foreign piece that moves into the goal's own project.
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let p = add(&env, &bb, "Piece", &[]);
    let goal = hub(&env, &aa, "Goal", &[], &[&p]);
    let done_goal = hub(&env, &aa, "Done goal", &[], &[&p]);
    let other = add(&env, &bb, "Other", &[]);
    env.json(&bb, &["done", &other, "done"]);
    set_field(&aa, &done_goal, "status", "done");
    let new = id_of(env.json(&bb, &["move", &p, "--to", "aa"]));
    let (_, check) = common::check_findings(&env, &aa);
    let found = finding_on(&check, "warnings", "piece_now_local", &goal)
        .unwrap_or_else(|| panic!("no piece_now_local: {check}"));
    let detail = found["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!("tasks dep {goal} --rm {p}")),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("tasks edit {new} --parent {goal}")),
        "{detail}"
    );
    assert!(
        finding_on(&check, "warnings", "moved_dep", &goal).is_none(),
        "{check}"
    );
    for kind in ["piece_now_local", "moved_dep"] {
        assert!(
            finding_on(&check, "warnings", kind, &done_goal).is_none(),
            "{check}"
        );
    }
}

#[test]
fn pieces_reader_move_carries_and_refuses() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    let x = add(&env, &bb, "Piece", &[]);
    let h = hub(&env, &aa, "Hub", &[], &[&x]);

    let before = env.read(&aa, &format!("tasks/{h}.md"));
    let err = stderr_of(&env, &aa, &["move", &h, "--to", "bb"]);
    assert!(
        err.contains(&format!("tasks edit {h} --rm-piece {x}")),
        "{err}"
    );
    assert!(err.contains("--parent"), "{err}");
    assert_eq!(env.read(&aa, &format!("tasks/{h}.md")), before);

    let new = id_of(env.json(&aa, &["move", &h, "--to", "cc"]));
    let moved = env.json(&cc, &["show", &new])["task"].clone();
    assert_eq!(moved["pieces"], json!([x]), "{moved}");
    assert_eq!(moved["depends"], json!([x]), "{moved}");
    let tombstone = env.json(&aa, &["show", &h])["task"].clone();
    assert!(tombstone.get("pieces").is_none(), "{tombstone}");
}

fn migrate_list(env: &TestEnv, dir: &Path, to: &str, carry: &[&str]) -> PathBuf {
    let list = env.home.path().join("list.toml");
    let _ = std::fs::remove_file(&list);
    env.json(
        dir,
        &["migrate", "--to", to, "--draft", list.to_str().unwrap()],
    );
    let mut text = std::fs::read_to_string(&list).unwrap();
    for id in carry {
        let at = text.find(&format!("id = \"{id}\"")).unwrap();
        let line = at + text[at..].find("decision = \"\"\n").unwrap();
        let end = line + "decision = \"\"\n".len();
        text = format!("{}decision = \"carry\"\n{}", &text[..line], &text[end..]);
    }
    // Everything else is dropped so the list is complete.
    text = text.replace(
        "decision = \"\"\n",
        "decision = \"drop\"\nreason = \"out of scope\"\n",
    );
    std::fs::write(&list, text).unwrap();
    list
}

fn refusal_rules(out: &Value) -> Vec<(String, String)> {
    out["refusals"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    (
                        r["rule"].as_str().unwrap().to_string(),
                        r["id"].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn pieces_reader_migrate_refuses_into_piece_project() {
    // A carried hub whose destination is its piece's project.
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let h = hub(&env, &aa, "Hub", &[], &[&x]);
    let list = migrate_list(&env, &aa, "bb", &[&h]);
    let out = env.json(&aa, &["migrate", list.to_str().unwrap(), "--explain"]);
    assert!(
        refusal_rules(&out).contains(&("local_piece".to_string(), h.clone())),
        "{out}"
    );

    // A carried record with a corrupt same-project piece (carried or already closed).
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    env.init("cc");
    let local = add(&env, &aa, "Local", &[]);
    let closed_local = add(&env, &aa, "Closed local", &[]);
    env.json(&aa, &["done", &closed_local, "done"]);
    let h = add(&env, &aa, "Hub", &[]);
    env.json(&aa, &["dep", &h, "--on", &local]);
    raw(&env, &aa, &h, "pieces", &[&local]);
    let h_closed_piece = add(&env, &aa, "Hub with closed local piece", &[]);
    env.json(&aa, &["dep", &h_closed_piece, "--on", &closed_local]);
    raw(&env, &aa, &h_closed_piece, "pieces", &[&closed_local]);
    let list = migrate_list(&env, &aa, "cc", &[&h, &local, &h_closed_piece]);
    let out = env.json(&aa, &["migrate", list.to_str().unwrap(), "--explain"]);
    for (goal_id, piece_id) in [(&h, &local), (&h_closed_piece, &closed_local)] {
        let refusal = out["refusals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["rule"] == "local_piece" && r["id"] == *goal_id)
            .unwrap_or_else(|| panic!("missing local_piece on {goal_id}: {out}"));
        let settle = refusal["settle"].as_str().unwrap();
        assert!(
            settle.contains(&format!("tasks dep {goal_id} --rm {piece_id}")),
            "{settle}"
        );
        assert!(
            settle.contains(&format!("tasks edit {piece_id} --parent {goal_id}")),
            "{settle}"
        );
        assert!(!settle.contains("in cc"), "{settle}");
        assert!(!settle.contains("move it"), "{settle}");
    }

    // A piece migrated into an uncarried goal's project is listed as inbound.
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let x = add(&env, &bb, "Piece", &[]);
    let g = hub(&env, &aa, "Goal", &[], &[&x]);
    let list = migrate_list(&env, &bb, "aa", &[&x]);
    let out = env.json(&bb, &["migrate", list.to_str().unwrap(), "--explain"]);
    assert_eq!(
        out["inbound"],
        json!([{"id": x, "dependents": [g]}]),
        "{out}"
    );
}
