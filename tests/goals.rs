//! `tasks goals`: the goals the user asked for, across projects, with progress, recent
//! activity and attention flags (records spec 2026-11-01-goals-view-design). Every fixture
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
    let out = goals(&env, &aa, &[]);
    assert_eq!(ids(&out), [open, recent.clone()]);
    let closed = row(&out, &recent);
    assert!(closed.get("closed").is_some(), "{closed}");
    assert!(closed.get("attention").is_none(), "{closed}");
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
    assert_eq!(ids(&goals(&env, &aa, &[])), [g]);
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
    assert!(ids(&goals(&env, &aa, &[])).is_empty());
    let out = goals(&env, &aa, &["--project", "bb"]);
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
    for key in ["closed", "attention", "color"] {
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

// Attention flags (§5.4).

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
    let out = goals(&env, &aa, &[]);
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
