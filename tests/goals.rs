//! `tasks goals`: the goals the user asked for, across projects, with progress, recent
//! activity and attention flags (records spec docs/user-guide.md). Every fixture
//! runs under TestEnv's isolated environment.

mod common;
use common::{TestEnv, id_of, write_claim};
use serde_json::Value;
use std::path::Path;

fn add(env: &TestEnv, dir: &Path, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["add", title];
    args.extend_from_slice(extra);
    id_of(env.json(dir, &args))
}

fn goals(env: &TestEnv, dir: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["goals"];
    args.extend_from_slice(extra);
    env.json(dir, &args)
}

fn row<'a>(out: &'a Value, id: &str) -> &'a Value {
    out["goals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row for {id}: {out}"))
}

fn ids(out: &Value) -> Vec<String> {
    out["goals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

/// Rewrites the `<field>: ` frontmatter line of `tasks/<id>.md`, inserts it before the
/// closing `---` when absent, or deletes it when `value` is None.
fn set_field(dir: &Path, id: &str, field: &str, value: Option<&str>) {
    let path = dir.join(format!("tasks/{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("{field}:");
    let mut lines: Vec<String> = Vec::new();
    let mut fence = 0;
    let mut found = false;
    for line in text.lines() {
        if line == "---" {
            fence += 1;
            if fence == 2 && !found {
                if let Some(value) = value {
                    lines.push(format!("{field}: {value}"));
                }
                found = true;
            }
            lines.push(line.to_string());
            continue;
        }
        if fence == 1 && line.starts_with(&prefix) {
            found = true;
            if let Some(value) = value {
                lines.push(format!("{field}: {value}"));
            }
            continue;
        }
        lines.push(line.to_string());
    }
    let mut out = lines.join("\n");
    out.push('\n');
    std::fs::write(&path, out).unwrap();
}

fn backdate(dir: &Path, ids: &[&str], stamp: &str) {
    for id in ids {
        set_field(dir, id, "updated", Some(stamp));
    }
}

fn ago(days: i64) -> String {
    let at = time::OffsetDateTime::now_utc() - time::Duration::days(days);
    at.replace_nanosecond(0)
        .unwrap()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

fn goal_with_child(env: &TestEnv, dir: &Path, title: &str) -> (String, String) {
    let goal = add(env, dir, title, &["--origin", "user"]);
    let child = add(env, dir, &format!("{title} piece"), &["--parent", &goal]);
    (goal, child)
}

#[test]
fn goals_activity_mapping_distinguishes_scores_with_fixed_ids() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = \"sci\"\ncolor = \"#123456\"\n",
    )
    .unwrap();
    let (hot, _) = goal_with_child(&env, &dir, "Hot goal");
    let (cold, child) = goal_with_child(&env, &dir, "Cold goal");
    for id in [&cold, &child] {
        for field in ["created", "updated"] {
            set_field(&dir, id, field, Some("2020-01-01T00:00:00Z"));
        }
    }
    let (closed, piece) = goal_with_child(&env, &dir, "Closed goal");
    env.json(&dir, &["done", &piece]);
    env.json(&dir, &["done", &closed]);
    let output = env
        .cmd(&dir)
        .args(["goals", "--closed", "--pretty", "--color", "always"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    // Independent B references: the old contrast-limited mapping left both titles dark.
    assert!(
        text.contains("\x1b[38;2;198;225;255mHot goal\x1b[0m"),
        "{text}"
    );
    assert!(
        text.contains("\x1b[38;2;95;131;170mCold goal\x1b[0m"),
        "{text}"
    );
    for id in [&hot, &cold] {
        assert!(
            text.contains(&format!("\x1b[38;2;84;108;135m{id}\x1b[0m")),
            "{text}"
        );
    }
    assert!(text.contains("\x1b[2mClosed goal\x1b[0m"), "{text}");
    let mut visible = String::new();
    let mut sgr = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            sgr = true;
        } else if sgr {
            if ch == 'm' {
                sgr = false;
            }
        } else {
            visible.push(ch);
        }
    }
    assert_eq!(
        visible,
        env.pretty(&dir, &["goals", "--closed", "--color", "never"])
    );
    let json = goals(&env, &dir, &["--closed"]);
    for id in [&hot, &cold, &closed] {
        assert_eq!(row(&json, id)["color"], "#123456");
        assert!(row(&json, id).get("activity").is_none());
    }
}

#[test]
fn lists_user_and_mixed_goals_only() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let mut made = Vec::new();
    for origin in ["user", "mixed", "agent", ""] {
        let extra: Vec<&str> = if origin.is_empty() {
            vec![]
        } else {
            vec!["--origin", origin]
        };
        let goal = add(&env, &aa, &format!("Goal {origin}"), &extra);
        add(&env, &aa, "piece", &["--parent", &goal]);
        made.push(goal);
    }
    let mut listed = ids(&goals(&env, &aa, &[]));
    listed.sort();
    let mut expected = vec![made[0].clone(), made[1].clone()];
    expected.sort();
    assert_eq!(listed, expected);
}

#[test]
fn childless_task_with_cross_project_dependency_is_not_a_goal() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let leaf = add(&env, &aa, "Leaf", &["--origin", "user"]);
    let other = add(&env, &bb, "Other", &[]);
    env.json(&aa, &["dep", &leaf, "--on", &other]);
    assert!(ids(&goals(&env, &aa, &[])).is_empty());
}

#[test]
fn user_lane_without_children_lists() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let lane = add(&env, &aa, "Lane", &["--lane", "--origin", "user"]);
    let out = goals(&env, &aa, &[]);
    assert_eq!(ids(&out), std::slice::from_ref(&lane));
    assert_eq!(row(&out, &lane)["pieces"]["total"], 0);
    assert_eq!(row(&out, &lane)["lane"], true);
}

#[test]
fn nested_goals_each_get_a_row() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let s = add(&env, &aa, "S", &["--origin", "user", "--parent", &g]);
    add(&env, &aa, "leaf", &["--parent", &s]);
    let out = goals(&env, &aa, &[]);
    assert_eq!(row(&out, &g)["pieces"]["total"], 2);
    assert_eq!(row(&out, &s)["pieces"]["total"], 1);
}

#[test]
fn progress_counts_committed_descendants() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let child = |title: &str, extra: &[&str]| {
        let mut args = vec!["--parent", g.as_str()];
        args.extend_from_slice(extra);
        add(&env, &aa, title, &args)
    };
    let done = child("done", &[]);
    env.json(&aa, &["done", &done]);
    child("todo", &[]);
    let doing = child("doing", &[]);
    env.json(&aa, &["edit", &doing, "--status", "doing"]);
    let blocked = child("blocked", &[]);
    env.json(&aa, &["block", &blocked, "why"]);
    child("idea", &["--status", "idea"]);
    let shelved = child("shelved", &[]);
    env.json(&aa, &["shelve", &shelved, "--defer", "2099-01-01", "later"]);
    let dropped = child("dropped", &[]);
    env.json(&aa, &["drop", &dropped, "no"]);
    let out = goals(&env, &aa, &[]);
    assert_eq!(
        row(&out, &g)["pieces"],
        serde_json::json!({"done": 1, "total": 4, "ideas": 1})
    );
}

#[test]
fn moved_piece_leaves_goal_progress() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    env.init("bb");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    add(&env, &aa, "A", &["--parent", &g]);
    let b = add(&env, &aa, "B", &["--parent", &g]);
    env.json(&aa, &["edit", &b, "--no-parent"]);
    env.json(&aa, &["move", &b, "--to", "bb"]);
    let out = goals(&env, &aa, &[]);
    assert_eq!(row(&out, &g)["pieces"]["total"], 1);
}

// Open dependencies : reported apart from progress, never counted as pieces.

#[test]
fn dependencies_count_targets_outside_the_subtree_once() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let finished = add(&env, &aa, "finished", &["--parent", &g]);
    let open = add(&env, &aa, "open", &["--parent", &g]);
    let dropped = add(&env, &aa, "dropped", &["--parent", &g]);
    let outside = add(&env, &aa, "outside", &[]);
    let x = add(&env, &bb, "X", &[]);
    let y = add(&env, &bb, "Y", &[]);
    let z = add(&env, &bb, "Z", &[]);
    let w = add(&env, &bb, "W", &[]);
    // The goal record: one open, one done, one dropped target.
    env.json(&aa, &["dep", &g, "--on", &x, "--on", &y, "--on", &z]);
    // A done piece: the same open target again, and a sibling inside the subtree.
    env.json(&aa, &["dep", &finished, "--on", &x, "--on", &open]);
    // An open piece: a same-project task outside the subtree.
    env.json(&aa, &["dep", &open, "--on", &outside]);
    // A dropped piece is abandoned work: its dependencies are not counted.
    env.json(&aa, &["dep", &dropped, "--on", &w]);
    env.json(&aa, &["done", &finished, "--force"]);
    env.json(&aa, &["drop", &dropped, "no"]);
    env.json(&bb, &["done", &y]);
    env.json(&bb, &["drop", &z, "no"]);
    let out = goals(&env, &aa, &[]);
    let goal = row(&out, &g);
    assert_eq!(goal["pieces"], serde_json::json!({"done": 1, "total": 2}));
    assert_eq!(
        goal["dependencies"],
        serde_json::json!({"open": 2, "total": 4}),
        "{goal}"
    );
    // Narrowing the scope never changes a listed goal's numbers.
    let narrow = goals(&env, &bb, &["--project", "aa"]);
    assert_eq!(row(&narrow, &g)["dependencies"], goal["dependencies"]);
}

#[test]
fn unresolved_dependency_is_counted_apart_and_never_satisfied() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "G");
    set_field(&aa, &g, "depends", Some("[aa-ffffff]"));
    let out = goals(&env, &aa, &[]);
    assert_eq!(
        row(&out, &g)["dependencies"],
        serde_json::json!({"open": 0, "total": 1, "unresolved": 1})
    );
}

#[test]
fn dependency_on_a_piece_moved_out_of_reach_is_unresolved() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let moved = add(&env, &aa, "moved", &["--parent", &g]);
    let sibling = add(&env, &aa, "sibling", &["--parent", &g]);
    env.json(&aa, &["dep", &sibling, "--on", &moved]);
    env.json(&aa, &["edit", &moved, "--no-parent"]);
    env.json(&aa, &["move", &moved, "--to", "bb"]);
    // Within reach, the moved record is an outside prerequisite like any other.
    let out = goals(&env, &aa, &["--project", "aa"]);
    assert_eq!(
        row(&out, &g)["dependencies"],
        serde_json::json!({"open": 1, "total": 1})
    );
    // Out of reach, no record answers: unresolved, never dropped as inside the subtree,
    // even when the tombstone still names the goal as its parent (an older or hand-edited
    // record; the CLI unparents before a move).
    set_field(&aa, &moved, "parent", Some(&g));
    std::fs::remove_dir_all(&bb).unwrap();
    let out = goals(&env, &aa, &["--project", "aa"]);
    assert_eq!(
        row(&out, &g)["dependencies"],
        serde_json::json!({"open": 0, "total": 1, "unresolved": 1})
    );
}

#[test]
fn closed_goal_reports_open_dependencies_too() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (g, piece) = goal_with_child(&env, &aa, "G");
    let x = add(&env, &bb, "X", &[]);
    env.json(&aa, &["dep", &g, "--on", &x]);
    env.json(&aa, &["done", &piece]);
    env.json(&aa, &["done", &g, "--force"]);
    let out = goals(&env, &aa, &["--closed"]);
    assert_eq!(
        row(&out, &g)["dependencies"],
        serde_json::json!({"open": 1, "total": 1})
    );
}

#[test]
fn idea_only_goal_lists_with_zero_total() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    add(&env, &aa, "idea", &["--parent", &g, "--status", "idea"]);
    let out = goals(&env, &aa, &[]);
    assert_eq!(row(&out, &g)["pieces"]["total"], 0);
    assert_eq!(row(&out, &g)["pieces"]["ideas"], 1);
}

#[test]
fn shelved_and_dropped_goals_never_list() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (shelved, child) = goal_with_child(&env, &aa, "Shelved");
    env.json(&aa, &["shelve", &child, "--defer", "2099-01-01", "later"]);
    env.json(&aa, &["shelve", &shelved, "--defer", "2099-01-01", "later"]);
    let (dropped, piece) = goal_with_child(&env, &aa, "Dropped");
    env.json(&aa, &["drop", &piece, "no"]);
    env.json(&aa, &["drop", &dropped, "no"]);
    assert!(ids(&goals(&env, &aa, &[])).is_empty());
}

#[test]
fn idea_goal_lists() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user", "--status", "idea"]);
    add(&env, &aa, "piece", &["--parent", &g, "--status", "idea"]);
    assert_eq!(row(&goals(&env, &aa, &[]), &g)["status"], "idea");
}

#[test]
fn orders_by_subtree_activity() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g1, c1) = goal_with_child(&env, &aa, "G1");
    let (g2, c2) = goal_with_child(&env, &aa, "G2");
    let (g3, c3) = goal_with_child(&env, &aa, "G3");
    backdate(&aa, &[&g1, &c1], &ago(1));
    backdate(&aa, &[&g2], &ago(10));
    backdate(&aa, &[&g3, &c3], &ago(5));
    let out = goals(&env, &aa, &[]);
    assert_eq!(ids(&out), [g2.clone(), g1, g3]);
    let child = env.json(&aa, &["show", &c2]);
    let child_updated = child["updated"]
        .as_str()
        .or_else(|| child["task"]["updated"].as_str())
        .unwrap()
        .to_string();
    assert_eq!(row(&out, &g2)["last_activity"], child_updated.as_str());
}

#[test]
fn ties_break_on_total_then_id() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (a, a1) = goal_with_child(&env, &aa, "A");
    let (b, b1) = goal_with_child(&env, &aa, "B");
    let b2 = add(&env, &aa, "B second", &["--parent", &b]);
    let (c, c1) = goal_with_child(&env, &aa, "C");
    let stamp = ago(2);
    backdate(&aa, &[&a, &a1, &b, &b1, &b2, &c, &c1], &stamp);
    let (low, high) = if a < c { (a, c) } else { (c, a) };
    assert_eq!(ids(&goals(&env, &aa, &[])), [b, low, high]);
}

#[test]
fn closed_window_includes_recent_done_goals_after_open_ones() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (open, _) = goal_with_child(&env, &aa, "Open");
    let (recent, r1) = goal_with_child(&env, &aa, "Recent");
    let (old, o1) = goal_with_child(&env, &aa, "Old");
    for (goal, piece) in [(&recent, &r1), (&old, &o1)] {
        env.json(&aa, &["done", piece]);
        env.json(&aa, &["done", goal]);
    }
    set_field(&aa, &recent, "closed", Some(&ago(13)));
    set_field(&aa, &old, "closed", Some(&ago(15)));
    // The open goal is older than the closed one's stamps: closed rows still come last.
    backdate(&aa, &[&open], &ago(30));
    let out = goals(&env, &aa, &["--closed"]);
    assert_eq!(ids(&out), [open, recent.clone()]);
    let closed = row(&out, &recent);
    assert!(closed.get("closed").is_some(), "{closed}");
    assert!(closed.get("attention").is_none(), "{closed}");
}

#[test]
fn closed_goals_are_hidden_without_the_flag() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (open, _) = goal_with_child(&env, &aa, "Open");
    let (recent, piece) = goal_with_child(&env, &aa, "Recent");
    env.json(&aa, &["done", &piece]);
    env.json(&aa, &["done", &recent]);
    assert_eq!(ids(&goals(&env, &aa, &[])), std::slice::from_ref(&open));
    assert_eq!(ids(&goals(&env, &aa, &["--closed"])), [open, recent]);
}

#[test]
fn closed_window_falls_back_to_updated() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, piece) = goal_with_child(&env, &aa, "Legacy");
    env.json(&aa, &["done", &piece]);
    env.json(&aa, &["done", &g]);
    set_field(&aa, &g, "closed", None);
    set_field(&aa, &g, "completed", None);
    set_field(&aa, &g, "updated", Some(&ago(2)));
    assert_eq!(ids(&goals(&env, &aa, &["--closed"])), [g]);
}

#[test]
fn project_scope_narrows_listing() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (ga, _) = goal_with_child(&env, &aa, "In aa");
    add(&env, &aa, "second", &["--parent", &ga]);
    goal_with_child(&env, &bb, "In bb");
    let wide = goals(&env, &aa, &[]);
    assert_eq!(wide["goals"].as_array().unwrap().len(), 2);
    let narrow = goals(&env, &bb, &["--project", "aa"]);
    assert_eq!(ids(&narrow), std::slice::from_ref(&ga));
    assert_eq!(row(&narrow, &ga)["pieces"], row(&wide, &ga)["pieces"]);
}

#[test]
fn unreachable_project_warns_and_others_list() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "G");
    let bb = env.init("bb");
    std::fs::remove_dir_all(&bb).unwrap();
    let out = goals(&env, &aa, &[]);
    assert_eq!(ids(&out), [g]);
    let warnings = out["warnings"].to_string();
    assert!(warnings.contains("bb"), "{warnings}");
}

#[test]
fn retired_project_needs_explicit_scope() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    // Retirement needs every task closed, so bb's goal is a recently closed one.
    let (g, piece) = goal_with_child(&env, &bb, "Finished");
    env.json(&bb, &["done", &piece]);
    env.json(&bb, &["done", &g]);
    env.json(&bb, &["retire", "--reason", "fixture"]);
    assert!(ids(&goals(&env, &aa, &["--closed"])).is_empty());
    let out = goals(&env, &aa, &["--project", "bb", "--closed"]);
    assert_eq!(ids(&out), [g]);
    let warnings = out["warnings"].to_string();
    assert!(warnings.contains("retired"), "{warnings}");
}

#[test]
fn json_omits_unset_fields() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "G");
    let out = goals(&env, &aa, &[]);
    let goal = row(&out, &g);
    for key in ["closed", "attention", "color", "dependencies"] {
        assert!(goal.get(key).is_none(), "{key} in {goal}");
    }
    assert!(out["observed_at"].is_string());
    assert_eq!(out["park_evidence"]["scope"], "host_local");
}

#[test]
fn json_carries_project_color() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let config = aa.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("color = \"#7AA2F7\"\n{text}")).unwrap();
    let (g, _) = goal_with_child(&env, &aa, "G");
    assert_eq!(row(&goals(&env, &aa, &[]), &g)["color"], "#7aa2f7");
}

// Attention flags.

/// Makes `id` doing, then drops every claim the CLI's short-lived process wrote, so no
/// claim can suppress a stall.
fn doing(env: &TestEnv, dir: &Path, prefix: &str, id: &str) {
    env.json(dir, &["edit", id, "--status", "doing"]);
    let _ = std::fs::remove_file(env.claim_store(prefix));
}

/// Starts and parks `id` with the given park flags.
fn park(env: &TestEnv, dir: &Path, id: &str, flags: &[&str]) {
    env.json(dir, &["start", id]);
    let mut args = vec!["park", id, "the next step"];
    args.extend_from_slice(flags);
    env.json(dir, &args);
}

/// Rewrites the `at = "…"` line of `id`'s park in `prefix`'s claim store.
fn set_park_at(env: &TestEnv, prefix: &str, id: &str, stamp: &str) {
    let path = env.claim_store(prefix);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut inside = false;
    let mut found = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with('[') {
                inside = line.contains("parks") && line.contains(id);
            }
            if inside && line.starts_with("at = ") {
                found = true;
                format!("at = \"{stamp}\"")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert!(found, "no park for {id} in {text}");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

fn attention(out: &Value, id: &str) -> Value {
    row(out, id)
        .get("attention")
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn doing_leaf_untouched_for_three_days_is_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, old) = goal_with_child(&env, &aa, "G");
    let recent = add(&env, &aa, "recent", &["--parent", &g]);
    doing(&env, &aa, "aa", &old);
    doing(&env, &aa, "aa", &recent);
    let stamp = ago(4);
    set_field(&aa, &old, "updated", Some(&stamp));
    set_field(&aa, &recent, "updated", Some(&ago(2)));
    let out = goals(&env, &aa, &[]);
    assert_eq!(
        attention(&out, &g)["stalled"],
        serde_json::json!({"count": 1, "first": old, "since": stamp})
    );
}

#[test]
fn live_claim_suppresses_stall() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    doing(&env, &aa, "aa", &leaf);
    set_field(&aa, &leaf, "updated", Some(&ago(4)));
    write_claim(&env, "aa", &leaf, "someone-else", true);
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn user_park_counts_as_you_not_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    park(&env, &aa, &leaf, &["--waiting-on", "user"]);
    let stamp = ago(4);
    set_park_at(&env, "aa", &leaf, &stamp);
    set_field(&aa, &leaf, "updated", Some(&stamp));
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert!(flags.get("stalled").is_none(), "{flags}");
    assert_eq!(
        flags["you"],
        serde_json::json!({"count": 1, "first": leaf, "since": stamp})
    );
}

#[test]
fn dependency_and_quiet_parks_never_stall() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, waits) = goal_with_child(&env, &aa, "G");
    let quiet = add(&env, &aa, "quiet", &["--parent", &g]);
    park(&env, &aa, &waits, &["--reason", "dependency"]);
    park(&env, &aa, &quiet, &["--reason", "quiet", "--minutes", "30"]);
    let stamp = ago(5);
    for id in [&waits, &quiet] {
        set_park_at(&env, "aa", id, &stamp);
        set_field(&aa, id, "updated", Some(&stamp));
    }
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn agent_park_stalls_by_park_age() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    park(&env, &aa, &leaf, &[]);
    let stamp = ago(4);
    set_park_at(&env, "aa", &leaf, &stamp);
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert_eq!(
        flags["stalled"],
        serde_json::json!({"count": 1, "first": leaf, "since": stamp})
    );
}

#[test]
fn old_doing_parent_with_recent_child_is_not_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let s = add(&env, &aa, "S", &["--parent", &g]);
    let child = add(&env, &aa, "child", &["--parent", &s]);
    doing(&env, &aa, "aa", &s);
    set_field(&aa, &s, "updated", Some(&ago(5)));
    set_field(&aa, &child, "updated", Some(&ago(0)));
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn old_doing_parent_with_quiet_subtree_is_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let s = add(&env, &aa, "S", &["--parent", &g]);
    let child = add(&env, &aa, "child", &["--parent", &s]);
    doing(&env, &aa, "aa", &s);
    let newest = ago(5);
    set_field(&aa, &s, "updated", Some(&newest));
    set_field(&aa, &child, "updated", Some(&ago(6)));
    set_field(&aa, &g, "updated", Some(&ago(6)));
    assert_eq!(
        attention(&goals(&env, &aa, &[]), &g)["stalled"],
        serde_json::json!({"count": 1, "first": s, "since": newest})
    );
}

#[test]
fn held_claim_in_subtree_keeps_old_parent_from_stalling() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let s = add(&env, &aa, "S", &["--parent", &g]);
    let child = add(&env, &aa, "child", &["--parent", &s]);
    doing(&env, &aa, "aa", &s);
    for id in [&g, &s, &child] {
        set_field(&aa, id, "updated", Some(&ago(6)));
    }
    write_claim(&env, "aa", &child, "someone-else", true);
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn blocked_descendant_flags_blocked() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    env.json(&aa, &["block", &leaf, "why"]);
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert_eq!(flags["blocked"]["count"], 1);
    assert_eq!(flags["blocked"]["first"], leaf.as_str());
}

#[test]
fn goal_inside_paused_lane_names_the_lane() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let lane = add(&env, &aa, "L", &["--lane", "--origin", "agent"]);
    let g = add(&env, &aa, "G", &["--origin", "user", "--parent", &lane]);
    add(&env, &aa, "piece", &["--parent", &g]);
    env.json(&aa, &["block", &lane, "why"]);
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert_eq!(flags["blocked"]["count"], 1);
    assert_eq!(flags["blocked"]["first"], lane.as_str());
}

#[test]
fn awaiting_user_idea_counts_as_you() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let idea = add(&env, &aa, "idea", &["--parent", &g, "--status", "idea"]);
    env.json(
        &aa,
        &["note", &idea, "asked", "--reviewed", "--awaiting-user"],
    );
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert_eq!(flags["you"]["count"], 1);
    assert_eq!(flags["you"]["first"], idea.as_str());
}

#[test]
fn shelved_descendant_raises_no_flag() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    add(&env, &aa, "other", &["--parent", &g]);
    park(&env, &aa, &leaf, &["--waiting-on", "user"]);
    // Shelving clears the park; restore it, as a park left behind on this host would be.
    let store = std::fs::read_to_string(env.claim_store("aa")).unwrap();
    env.json(&aa, &["shelve", &leaf, "--defer", "2099-01-01", "later"]);
    std::fs::write(env.claim_store("aa"), store).unwrap();
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn closed_goal_has_no_attention() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    env.json(&aa, &["block", &leaf, "why"]);
    env.json(&aa, &["done", &g, "closed anyway", "--force"]);
    let out = goals(&env, &aa, &["--closed"]);
    assert!(row(&out, &g).get("closed").is_some());
    assert_eq!(attention(&out, &g), Value::Null);
}

#[test]
fn dependency_wait_is_not_flagged() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, first) = goal_with_child(&env, &aa, "G");
    add(&env, &aa, "second", &["--parent", &g, "--depends", &first]);
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

// Pretty output.

fn pretty(env: &TestEnv, dir: &Path) -> String {
    env.pretty(dir, &["goals", "--color", "never"])
}

fn pretty_closed(env: &TestEnv, dir: &Path) -> String {
    env.pretty(dir, &["goals", "--closed", "--color", "never"])
}

fn line_of<'a>(text: &'a str, id: &str) -> &'a str {
    text.lines()
        .find(|line| line.split_whitespace().next() == Some(id))
        .unwrap_or_else(|| panic!("no line for {id}: {text}"))
}

#[test]
fn pretty_row_shows_status_progress_and_flags() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let g = add(&env, &aa, "G", &["--origin", "user"]);
    let done = add(&env, &aa, "done", &["--parent", &g]);
    env.json(&aa, &["done", &done]);
    add(&env, &aa, "todo", &["--parent", &g]);
    let leaf = add(&env, &aa, "stalled", &["--parent", &g]);
    for title in ["idea one", "idea two"] {
        add(&env, &aa, title, &["--parent", &g, "--status", "idea"]);
    }
    doing(&env, &aa, "aa", &g);
    doing(&env, &aa, "aa", &leaf);
    set_field(&aa, &leaf, "updated", Some(&ago(4)));
    let text = pretty(&env, &aa);
    let line = line_of(&text, &g);
    assert!(line.contains(" doing "), "{line}");
    assert!(line.contains("1/3") && line.contains("+2"), "{line}");
    assert!(line.contains(&format!("1 stalled ({leaf}, 4d)")), "{line}");
}

#[test]
fn pretty_progress_names_satisfied_dependencies() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (g, piece) = goal_with_child(&env, &aa, "G");
    env.json(&aa, &["done", &piece]);
    let x = add(&env, &bb, "X", &[]);
    let y = add(&env, &bb, "Y", &[]);
    env.json(&aa, &["dep", &g, "--on", &x, "--on", &y]);
    let text = pretty(&env, &aa);
    assert!(
        line_of(&text, &g).contains("1/1") && line_of(&text, &g).contains("0/2"),
        "{text}"
    );
    assert!(text.contains("deps: satisfied/total"));
    env.json(&bb, &["done", &x]);
    env.json(&bb, &["done", &y]);
    let text = pretty(&env, &aa);
    let line = line_of(&text, &g);
    assert!(
        line.contains("2/2"),
        "satisfied dependency progress remains visible: {line}"
    );
}

#[test]
fn pretty_names_unresolved_dependencies() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "G");
    set_field(&aa, &g, "depends", Some("[aa-ffffff]"));
    let text = pretty(&env, &aa);
    assert!(
        line_of(&text, &g).contains("0/1") && line_of(&text, &g).contains("?1"),
        "{text}"
    );
}

#[test]
fn pretty_shows_dash_for_zero_total() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let lane = add(&env, &aa, "Lane", &["--lane", "--origin", "user"]);
    let text = pretty(&env, &aa);
    let cells: Vec<&str> = line_of(&text, &lane).split_whitespace().collect();
    assert_eq!(cells[3], "-", "{text}");
}

#[test]
fn pretty_closed_section_follows_open_rows() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (open, _) = goal_with_child(&env, &aa, "Open");
    let (closed, piece) = goal_with_child(&env, &aa, "Closed");
    env.json(&aa, &["done", &piece]);
    env.json(&aa, &["done", &closed]);
    let hidden = pretty(&env, &aa);
    assert!(!hidden.contains(&closed), "{hidden}");
    assert!(!hidden.contains("── closed"), "{hidden}");
    let text = pretty_closed(&env, &aa);
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("no {needle}: {text}"))
    };
    let separator = at("── closed in the last 14 days ──");
    assert!(at(&open) < separator, "{text}");
    assert!(separator < at(&closed), "{text}");
}

#[test]
fn pretty_paused_lane_is_named() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let lane = add(&env, &aa, "L", &["--lane", "--origin", "agent"]);
    let g = add(&env, &aa, "G", &["--origin", "user", "--parent", &lane]);
    add(&env, &aa, "piece", &["--parent", &g]);
    env.json(&aa, &["block", &lane, "why"]);
    let text = pretty(&env, &aa);
    assert!(
        line_of(&text, &g).contains(&format!("1 blocked ({lane}, paused lane)")),
        "{text}"
    );
}

#[test]
fn pretty_empty_says_why() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    add(&env, &aa, "Not a goal", &[]);
    assert_eq!(
        pretty(&env, &aa).trim_end(),
        "no goals you asked for; goals without an origin are not shown \
         (tasks edit <id> --origin user)"
    );
}

// Final.

#[test]
fn doing_goal_waiting_on_user_park_is_not_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    doing(&env, &aa, "aa", &g);
    park(
        &env,
        &aa,
        &leaf,
        &["--waiting-on", "user", "--reason", "review"],
    );
    let stamp = ago(4);
    set_park_at(&env, "aa", &leaf, &stamp);
    for id in [&g, &leaf] {
        set_field(&aa, id, "updated", Some(&stamp));
    }
    let flags = attention(&goals(&env, &aa, &[]), &g);
    assert!(flags.get("stalled").is_none(), "{flags}");
    assert_eq!(flags["you"]["first"], leaf.as_str());
}

#[test]
fn doing_goal_waiting_on_quiet_park_is_not_stalled() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, leaf) = goal_with_child(&env, &aa, "G");
    doing(&env, &aa, "aa", &g);
    park(&env, &aa, &leaf, &["--reason", "quiet", "--minutes", "30"]);
    let stamp = ago(4);
    set_park_at(&env, "aa", &leaf, &stamp);
    for id in [&g, &leaf] {
        set_field(&aa, id, "updated", Some(&stamp));
    }
    assert_eq!(attention(&goals(&env, &aa, &[]), &g), Value::Null);
}

#[test]
fn pretty_title_uses_a_wide_terminal() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let title = "A deliberately long goal title that runs well past fifty columns on one line";
    let g = add(&env, &aa, title, &["--origin", "user"]);
    add(&env, &aa, "piece", &["--parent", &g]);
    let out = env
        .cmd(&aa)
        .env("COLUMNS", "200")
        .args(["--pretty", "goals", "--color", "never"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(line_of(&text, &g).contains(title), "{text}");
}

#[test]
fn goals_sort_project_default() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (a, ac) = goal_with_child(&env, &aa, "Older A");
    let (b, bc) = goal_with_child(&env, &bb, "Newer B");
    backdate(&aa, &[&a, &ac], &ago(5));
    backdate(&bb, &[&b, &bc], &ago(1));
    assert_eq!(ids(&goals(&env, &aa, &[])), [a.clone(), b.clone()]);
    assert_eq!(
        ids(&goals(&env, &aa, &["--sort", "project"])),
        [a.clone(), b.clone()]
    );
    assert_eq!(ids(&goals(&env, &aa, &["--project", "aa"])), [a]);
}

#[test]
fn goals_sort_activity_and_priority() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (a, ac) = goal_with_child(&env, &aa, "Older priority zero");
    let (b, bc) = goal_with_child(&env, &bb, "New priority two");
    env.json(&aa, &["edit", &a, "--priority", "0"]);
    backdate(&aa, &[&a, &ac], &ago(5));
    backdate(&bb, &[&b, &bc], &ago(1));
    assert_eq!(
        ids(&goals(&env, &aa, &["--sort", "activity"])),
        [b.clone(), a.clone()]
    );
    assert_eq!(
        ids(&goals(&env, &aa, &["--sort", "priority"])),
        [a.clone(), b.clone()]
    );
    assert_eq!(
        ids(&goals(&env, &aa, &["--reverse"])),
        [b.clone(), a.clone()]
    );
    assert_eq!(
        ids(&goals(&env, &aa, &["--sort", "priority", "--reverse"])),
        [b, a]
    );
}

#[test]
fn goals_sort_reverse_keeps_closed_last() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let (open, _) = goal_with_child(&env, &aa, "Open");
    let (a, ac) = goal_with_child(&env, &aa, "Closed A");
    let (b, bc) = goal_with_child(&env, &bb, "Closed B");
    for (dir, g, child) in [(&aa, &a, &ac), (&bb, &b, &bc)] {
        env.json(dir, &["done", child]);
        env.json(dir, &["done", g]);
    }
    set_field(&aa, &a, "closed", Some(&ago(1)));
    set_field(&bb, &b, "closed", Some(&ago(2)));
    assert_eq!(
        ids(&goals(&env, &aa, &["--closed"])),
        [open.clone(), a.clone(), b.clone()]
    );
    assert_eq!(
        ids(&goals(&env, &aa, &["--closed", "--reverse"])),
        [open.clone(), b.clone(), a.clone()]
    );
    assert_eq!(
        ids(&goals(&env, &aa, &["--closed", "--sort", "activity"])),
        [open.clone(), a.clone(), b.clone()]
    );
    assert_eq!(
        ids(&goals(
            &env,
            &aa,
            &["--closed", "--sort", "activity", "--reverse"]
        )),
        [open, b, a]
    );
}

#[test]
fn goals_sort_offsets_and_ties() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (a, ac) = goal_with_child(&env, &aa, "A");
    let (b, bc) = goal_with_child(&env, &aa, "B");
    // 10:00 UTC is newer than 11:00 at +02:00, despite the local date text.
    backdate(&aa, &[&a, &ac], "2026-10-01T10:00:00Z");
    backdate(&aa, &[&b, &bc], "2026-10-01T11:00:00+02:00");
    let rejected = env
        .cmd(&aa)
        .args(["goals", "--sort", "activity"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("must be UTC with Z suffix"));
    backdate(&aa, &[&b, &bc], "2026-10-01T09:00:00Z");
    assert_eq!(
        ids(&goals(&env, &aa, &["--sort", "activity"])),
        [a.clone(), b.clone()]
    );
    backdate(&aa, &[&a, &ac, &b, &bc], "2026-10-01T10:00:00Z");
    let mut ascending = vec![a.clone(), b.clone()];
    ascending.sort();
    assert_eq!(ids(&goals(&env, &aa, &[])), ascending);
    ascending.reverse();
    assert_eq!(ids(&goals(&env, &aa, &["--reverse"])), ascending);
    let extra = add(&env, &aa, "extra", &["--parent", &b]);
    backdate(&aa, &[&a, &ac, &b, &bc, &extra], "2026-10-01T10:00:00Z");
    assert_eq!(ids(&goals(&env, &aa, &[])), [b.clone(), a.clone()]);
    assert_eq!(ids(&goals(&env, &aa, &["--reverse"])), [a, b]);
}

#[test]
fn goals_sort_rejects_unknown() {
    let env = TestEnv::new();
    let dir = env.home.path();
    env.cmd(dir)
        .args(["goals", "--sort", "unknown"])
        .assert()
        .failure();
}

#[test]
fn goals_sort_json_shape() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "Goal");
    let out = goals(&env, &aa, &["--sort", "priority"]);
    let mut keys: Vec<_> = out
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["goals", "observed_at", "park_evidence", "warnings"]);
    assert_eq!(
        row(&out, &g)["pieces"],
        serde_json::json!({"done":0,"total":1})
    );
    assert!(row(&out, &g).get("order").is_none());
}

#[test]
fn goals_pretty_contract_closed_intro_names_closure_dates() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, c) = goal_with_child(&env, &aa, "Closed goal");
    env.json(&aa, &["done", &c]);
    env.json(&aa, &["done", &g]);
    let text = env.pretty(&aa, &["goals", "--closed", "--color", "never", "--headers"]);
    let intro = text.split(&g).next().unwrap();
    assert!(intro.contains("closure"), "{text}");
    assert!(intro.contains("sort:"), "{text}");
    assert_eq!(
        text.lines()
            .filter(|l| l.contains("status") && l.contains("title"))
            .count(),
        1
    );
}

#[test]
fn goals_pretty_contract_legend_keeps_relocated_categories() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let (g, _) = goal_with_child(&env, &aa, "A long title for the compact goals report");
    let idea = add(&env, &aa, "An idea", &["--parent", &g, "--status", "idea"]);
    set_field(&aa, &g, "depends", Some("[aa-ffffff]"));
    let run = || {
        let out = env
            .cmd(&aa)
            .env("COLUMNS", "60")
            .args(["goals", "--pretty", "--color", "never"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    let text = run();
    let intro = text.split(&g).next().unwrap();
    for category in ["+ideas", "deps:", "?unresolved"] {
        assert!(intro.contains(category), "{text}");
    }
    assert!(text.contains("progress:"), "{text}");
    env.json(&aa, &["drop", &idea, "not now"]);
    set_field(&aa, &g, "depends", Some("[]"));
    let text = run();
    let intro = text.split(&g).next().unwrap();
    for category in ["+ideas", "deps:", "?unresolved"] {
        assert!(!intro.contains(category), "{text}");
    }
}

#[test]
fn goals_pretty_contract_invalid_width_stays_an_error() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    goal_with_child(&env, &aa, "Goal");
    for width in ["0", "wide"] {
        env.cmd(&aa)
            .env("COLUMNS", width)
            .args(["goals", "--pretty"])
            .assert()
            .failure();
    }
}
