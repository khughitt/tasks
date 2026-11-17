//! Writer contracts for declared goal pieces: `--piece`, `--rm-piece`, `--no-pieces` and
//! the editor (spec 2026-11-17-goal-pieces-design §5.2).
mod common;
use common::{TestEnv, editor_script, findings_of, id_of};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn add(env: &TestEnv, dir: &Path, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["add", title];
    args.extend_from_slice(extra);
    id_of(env.json(dir, &args))
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

fn task(env: &TestEnv, dir: &Path, id: &str) -> Value {
    env.json(dir, &["show", id])["task"].clone()
}

fn warnings(value: &Value) -> Vec<String> {
    value["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect()
}

/// The command fails with exit 1, leaves `id`'s file unchanged, and returns stderr.
fn refused(env: &TestEnv, dir: &Path, id: &str, args: &[&str]) -> String {
    let path = format!("tasks/{id}.md");
    let before = env.read(dir, &path);
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{args:?} should fail: {out:?}");
    assert_eq!(env.read(dir, &path), before, "{args:?} wrote the record");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn pieces_writer_add_and_edit_piece() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x]);
    let t = task(&env, &aa, &hub);
    assert_eq!(t["pieces"], json!([x]), "{t}");
    assert_eq!(t["depends"], json!([x]), "{t}");

    let fresh = add(&env, &aa, "Fresh", &[]);
    env.json(&aa, &["edit", &fresh, "--piece", &x]);
    let t = task(&env, &aa, &fresh);
    assert_eq!(t["pieces"], json!([x]), "{t}");
    assert_eq!(t["depends"], json!([x]), "{t}");
}

#[test]
fn pieces_writer_promotion_is_silent() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let hub = add(&env, &aa, "Hub", &["--depends", &x, "--depends", &y]);
    let out = env.json(&aa, &["edit", &hub, "--piece", &x]);
    assert!(
        !warnings(&out).iter().any(|w| w.contains("already depends")),
        "{out}"
    );
    let t = task(&env, &aa, &hub);
    assert_eq!(t["pieces"], json!([x]), "{t}");
    assert_eq!(t["depends"], json!([x, y]), "{t}");

    let again = env.json(&aa, &["edit", &hub, "--piece", &x]);
    assert!(
        warnings(&again).contains(&format!("{hub} already has piece {x}; unchanged")),
        "{again}"
    );
    // Fields are unchanged; `updated` moves on any edit, as for `--depends`.
    let after = task(&env, &aa, &hub);
    assert_eq!(
        (&after["pieces"], &after["depends"]),
        (&t["pieces"], &t["depends"])
    );

    // Under an alias spelling after the piece's project is renamed.
    env.json(&bb, &["rename", "bb", "cc"]);
    let renamed = y.replacen("bb-", "cc-", 1);
    let out = env.json(&aa, &["edit", &hub, "--piece", &renamed]);
    assert!(
        !warnings(&out).iter().any(|w| w.contains("already depends")),
        "{out}"
    );
    let t = task(&env, &aa, &hub);
    assert_eq!(t["depends"], json!([x, y]), "{t}");
    assert_eq!(t["pieces"].as_array().unwrap().len(), 2, "{t}");
}

#[test]
fn pieces_writer_rm_piece_keeps_dependency() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let hub = add(&env, &aa, "Hub", &["--depends", &y, "--piece", &x]);
    env.json(&aa, &["edit", &hub, "--rm-piece", &x]);
    let t = task(&env, &aa, &hub);
    assert!(t.get("pieces").is_none(), "{t}");
    assert_eq!(t["depends"], json!([y, x]), "{t}");
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--rm-piece", &y]);
    assert!(err.contains(&format!("{hub} has no piece {y}")), "{err}");
}

#[test]
fn pieces_writer_no_pieces_and_conflicts() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x, "--piece", &y]);
    env.usage(&aa, &["edit", &hub, "--no-pieces", "--piece", &x]);
    let err = refused(
        &env,
        &aa,
        &hub,
        &["edit", &hub, "--piece", &x, "--rm-piece", &x],
    );
    assert!(
        err.contains(&format!("{x} is named by both --piece and --rm-piece")),
        "{err}"
    );
    env.json(&aa, &["edit", &hub, "--no-pieces"]);
    let t = task(&env, &aa, &hub);
    assert!(t.get("pieces").is_none(), "{t}");
    assert_eq!(t["depends"], json!([x, y]), "{t}");
}

#[test]
fn pieces_writer_no_depends_with_clear() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x]);
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--no-depends"]);
    assert!(
        err.contains(&format!(
            "{hub} declares pieces ({x}); --no-depends would leave them outside depends; name \
             them again with --depends, or clear them with --no-pieces or --rm-piece"
        )),
        "{err}"
    );
    env.json(&aa, &["edit", &hub, "--no-depends", "--no-pieces"]);
    let t = task(&env, &aa, &hub);
    assert!(
        t.get("pieces").is_none() && t.get("depends").is_none(),
        "{t}"
    );
}

#[test]
fn pieces_writer_refusals() {
    let Two { env, aa, bb } = two();
    let local = add(&env, &aa, "Local", &[]);
    let hub = add(&env, &aa, "Hub", &[]);
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--piece", &local]);
    assert!(
        err.contains("make it a child with --parent instead"),
        "{err}"
    );
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--piece", &hub]);
    assert!(
        err.contains(&format!("{hub} cannot be its own piece")),
        "{err}"
    );
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--piece", "bb-fffff0"]);
    assert!(err.contains("bb-fffff0"), "{err}");

    let gone = add(&env, &bb, "Gone", &[]);
    let successor = id_of(env.json(&bb, &["move", &gone, "--to", "aa"]));
    let err = refused(&env, &aa, &hub, &["edit", &hub, "--piece", &gone]);
    assert!(err.contains(&successor), "{err}");

    let x = add(&env, &bb, "Piece", &[]);
    let lane_message = "is a lane; a lane holds only local children, so it cannot carry pieces";
    let before = std::fs::read_dir(aa.join("tasks")).unwrap().count();
    let out = env
        .cmd(&aa)
        .args(["add", "Lane", "--lane", "--piece", &x])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(lane_message),
        "{out:?}"
    );
    assert_eq!(std::fs::read_dir(aa.join("tasks")).unwrap().count(), before);
    let lane = add(&env, &aa, "Lane", &["--lane"]);
    let err = refused(&env, &aa, &lane, &["edit", &lane, "--piece", &x]);
    assert!(err.contains(&format!("{lane} {lane_message}")), "{err}");
}

#[test]
fn pieces_writer_allows_closed_targets() {
    let Two { env, aa, bb } = two();
    let done = add(&env, &bb, "Done", &[]);
    env.json(&bb, &["done", &done, "done"]);
    let dropped = add(&env, &bb, "Dropped", &[]);
    env.json(&bb, &["drop", &dropped, "not needed"]);
    let hub = add(&env, &aa, "Hub", &["--piece", &done, "--piece", &dropped]);
    assert_eq!(task(&env, &aa, &hub)["pieces"], json!([done, dropped]));
}

#[test]
fn pieces_writer_editor_parity() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let local = add(&env, &aa, "Local", &[]);
    let hub = add(&env, &aa, "Hub", &["--depends", &x, "--depends", &local]);

    let script = editor_script(
        &aa,
        &format!("sed -i 's/^tags:/pieces: [{x}]\\ntags:/' \"$1\""),
    );
    env.cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &hub, "--editor"])
        .assert()
        .success();
    assert_eq!(task(&env, &aa, &hub)["pieces"], json!([x]));

    let script = editor_script(
        &aa,
        &format!("sed -i 's/^pieces: .*/pieces: [{x}, {local}]/' \"$1\""),
    );
    let out = env
        .cmd(&aa)
        .env("EDITOR", script)
        .args(["edit", &hub, "--editor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("make it a child with --parent instead"),
        "{out:?}"
    );
    assert_eq!(task(&env, &aa, &hub)["pieces"], json!([x]));
}

#[test]
fn pieces_writer_completion() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let y = add(&env, &bb, "Other", &[]);
    let local = add(&env, &aa, "Local", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x, "--depends", &y]);
    let foreign = env.complete_values(&aa, "bash", 4, &["tasks", "edit", &hub, "--piece", "bb-"]);
    assert!(foreign.contains(&x) && foreign.contains(&y), "{foreign:?}");
    let own = env.complete_values(&aa, "bash", 4, &["tasks", "edit", &hub, "--piece", "aa-"]);
    assert!(!own.contains(&local), "{own:?}");
    let removable = env.complete_values(
        &aa,
        "bash",
        4,
        &["tasks", "edit", &hub, "--rm-piece", "bb-"],
    );
    assert_eq!(removable, vec![x.clone()], "{removable:?}");
}

#[test]
fn pieces_writer_moved_dep_remedy() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    env.init("cc");
    let x = add(&env, &bb, "Piece", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x]);
    let new = id_of(env.json(&bb, &["move", &x, "--to", "cc"]));
    let (_, check) = common::check_findings(&env, &aa);
    let detail = findings_of(&check, "warnings", "moved_dep")
        .into_iter()
        .find(|f| f["id"] == hub)
        .unwrap_or_else(|| panic!("no moved_dep: {check}"))["detail"]
        .as_str()
        .unwrap()
        .to_string();
    let first = detail
        .find(&format!("tasks edit {hub} --piece {new}"))
        .unwrap_or_else(|| panic!("{detail}"));
    let second = detail
        .find(&format!("tasks dep {hub} --rm {x}"))
        .unwrap_or_else(|| panic!("{detail}"));
    assert!(first < second, "{detail}");
}

#[test]
fn pieces_writer_non_gating_unchanged() {
    let Two { env, aa, bb } = two();
    let x = add(&env, &bb, "Piece", &[]);
    let hub = add(&env, &aa, "Hub", &["--piece", &x]);
    let ready = env.json(&aa, &["ready"]);
    assert!(
        !ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == hub),
        "{ready}"
    );
    assert_eq!(env.fail(&aa, &["done", &hub, "x"]), "open_dependencies");
    let blockers = env.json(&aa, &["blockers", "--all-projects"]);
    let row = blockers["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == x)
        .unwrap_or_else(|| panic!("no row for the piece: {blockers}"));
    assert_eq!(row["direct_goal_count"], 1, "{row}");
}
