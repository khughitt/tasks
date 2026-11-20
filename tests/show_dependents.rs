//! `show` lists the tasks whose `depends` names the shown task: from the scan it already
//! holds by default, and from every reachable registered project under `--all-projects`.
mod common;
use common::{TestEnv, git, id_of};
use serde_json::Value;

fn dependents(shown: &Value) -> Vec<String> {
    shown["dependents"]
        .as_array()
        .unwrap_or_else(|| panic!("no dependents in {shown}"))
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn show_dependents_local_open_and_closed_in_ready_order() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let q = id_of(e.json(&aa, &["add", "Prerequisite", "-p", "0"]));
    let lone = id_of(e.json(&aa, &["add", "Lone"]));
    let open = id_of(e.json(&aa, &["add", "Open", "-p", "1"]));
    let done = id_of(e.json(&aa, &["add", "Done", "-p", "2"]));
    for dependent in [&done, &open] {
        e.json(&aa, &["dep", dependent, "--on", &q]);
    }
    e.json(&aa, &["done", &done, "--force", "test fixture"]);
    let shown = e.json(&aa, &["show", &q]);
    assert_eq!(dependents(&shown), vec![open.clone(), done.clone()]);
    assert_eq!(shown["dependents"][0]["title"], "Open");
    assert_eq!(shown["dependents"][0]["status"], "todo");
    assert_eq!(shown["dependents"][1]["status"], "done");
    assert!(shown["dependents"][0]["updated"].is_string());
    assert!(e.json(&aa, &["show", &lone]).get("dependents").is_none());
    assert!(e.json(&aa, &["show", &open]).get("dependents").is_none());

    let next = e.json(&aa, &["next"]);
    assert_eq!(next["next"]["task"]["id"], q);
    assert_eq!(next["next"]["dependents"], shown["dependents"]);

    let pretty = e.cmd(&aa).args(["--pretty", "show", &q]).output().unwrap();
    assert!(pretty.status.success());
    let text = String::from_utf8(pretty.stdout).unwrap();
    let block = text.split_once("── links ").unwrap().1;
    assert!(block.contains(&format!("dependent  {open}")), "{block}");
}

#[test]
fn show_dependents_foreign_only_with_all_projects() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Piece", "-p", "0"]));
    let local = id_of(e.json(&aa, &["add", "Local dependent"]));
    e.json(&aa, &["dep", &local, "--on", &q]);
    let hub = id_of(e.json(&bb, &["add", "Hub goal"]));
    e.json(&bb, &["dep", &hub, "--on", &q]);

    e.json(&aa, &["edit", &q, "--related", &hub]);

    for (args, role) in [
        (vec!["show", &q], "related"),
        (vec!["show", &q, "--all-projects"], "dependent"),
        (vec!["next", "--all-projects"], "dependent"),
    ] {
        let pretty = e.pretty(&aa, &args);
        let rows: Vec<_> = pretty.lines().filter(|line| line.contains(&hub)).collect();
        assert_eq!(rows.len(), 1, "{pretty}");
        assert!(rows[0].starts_with(role), "{pretty}");
    }

    let plain = e.json(&aa, &["show", &q]);
    assert_eq!(dependents(&plain), vec![local.clone()]);
    let wide = e.json(&aa, &["show", &q, "--all-projects"]);
    let rows = dependents(&wide);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows.contains(&local) && rows.contains(&hub));
    // Widening reads more projects without duplicating this one's records.
    assert_eq!(wide["children"], plain["children"]);
    assert_eq!(wide["parent"], plain["parent"]);

    let next = e.json(&aa, &["next", "--all-projects"]);
    assert_eq!(next["next"]["task"]["id"], q);
    assert_eq!(dependents(&next["next"]).len(), 2);
}

#[test]
fn show_dependents_all_projects_widens_followed_by_and_keeps_children() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Earlier"]));
    let child = id_of(e.json(&aa, &["add", "Child", "--parent", &q]));
    let follower = id_of(e.json(&bb, &["add", "Foreign follower", "--follows", &q]));
    let plain = e.json(&aa, &["show", &q]);
    assert!(plain.get("followed_by").is_none());
    let wide = e.json(&aa, &["show", &q, "--all-projects"]);
    assert_eq!(wide["followed_by"][0]["id"], follower);
    assert_eq!(wide["children"].as_array().unwrap().len(), 1);
    assert_eq!(wide["children"][0]["id"], child);
}

#[test]
fn show_dependents_counts_an_alias_spelling() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Renamed piece"]));
    let hub = id_of(e.json(&bb, &["add", "Hub"]));
    e.json(&bb, &["dep", &hub, "--on", &q]);
    e.json(&aa, &["rename", "aa", "cc"]);
    let renamed = q.replacen("aa-", "cc-", 1);
    // The dependent still stores the retired spelling.
    assert!(e.read(&bb, &format!("tasks/{hub}.md")).contains(&q));
    let wide = e.json(&aa, &["show", &renamed, "--all-projects"]);
    assert_eq!(dependents(&wide), vec![hub]);
}

#[test]
fn show_dependents_skips_moved_tombstones() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let _bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Prerequisite"]));
    let dependent = id_of(e.json(&aa, &["add", "Dependent"]));
    e.json(&aa, &["dep", &dependent, "--on", &q]);
    let moved = e.json(&aa, &["move", &dependent, "--to", "bb"]);
    let successor = moved["id"].as_str().unwrap().to_string();
    assert_eq!(
        dependents(&e.json(&aa, &["show", &q, "--all-projects"])),
        vec![successor]
    );
    assert!(e.json(&aa, &["show", &q]).get("dependents").is_none());
}

#[test]
fn show_dependents_all_projects_from_a_worktree_keeps_its_copy_once() {
    let mut e = TestEnv::new();
    let main = e.init("aa");
    let _bb = e.init("bb");
    git(&main, &["init", "-q", "-b", "main"]);
    let q = id_of(e.json(&main, &["add", "Prerequisite"]));
    let dependent = id_of(e.json(&main, &["add", "Dependent"]));
    e.json(&main, &["dep", &dependent, "--on", &q]);
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
    // Lower priority sorts the worktree copy after a stale registered copy, so a
    // duplicate that slipped past the own-project filter would win the dedupe.
    e.json(
        &side,
        &["edit", &dependent, "-p", "4", "--title", "Edited here"],
    );
    e.json(&side, &["done", &dependent, "--force", "worktree only"]);
    let wide = e.json(&side, &["show", &q, "--all-projects"]);
    assert_eq!(dependents(&wide), vec![dependent.clone()]);
    assert_eq!(wide["dependents"][0]["status"], "done");
    assert_eq!(wide["dependents"][0]["title"], "Edited here");
    assert_eq!(
        e.json(&main, &["show", &q, "--all-projects"])["dependents"][0]["status"],
        "todo"
    );
}

#[test]
fn show_dependents_all_projects_warns_about_unreachable_projects() {
    let mut e = TestEnv::new();
    let aa = e.init("aa");
    let bb = e.init("bb");
    let q = id_of(e.json(&aa, &["add", "Prerequisite"]));
    std::fs::remove_file(bb.join("tasks/.config.toml")).unwrap();
    let wide = e.json(&aa, &["show", &q, "--all-projects"]);
    let warnings: Vec<_> = wide["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("project bb at ") && w.ends_with(" is unreachable")),
        "{warnings:?}"
    );
    assert!(
        e.json(&aa, &["show", &q])["warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
