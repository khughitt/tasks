//! `show` lists the tasks whose `depends` names the shown task: from the scan it already
//! holds by default, and from every reachable registered project under `--all-projects`.
mod common;
use common::{TestEnv, id_of};
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
    let block = text.split_once("── related ").unwrap().1;
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
