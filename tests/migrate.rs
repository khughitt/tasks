mod common;
use common::TestEnv;
use sha2::{Digest, Sha256};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn fail_detail(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> (String, String) {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{args:?}: {out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    (
        v["error"]["kind"].as_str().unwrap().to_string(),
        v["error"]["detail"].as_str().unwrap().to_string(),
    )
}

fn git(env: &TestEnv, dir: &std::path::Path, args: &[&str]) {
    let mut cmd = std::process::Command::new("git");
    cmd.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", env.home.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .current_dir(dir)
        .args(args);
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

#[test]
fn draft_lists_every_open_task_in_tree_order_with_digests() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let c = id_of(env.json(&forge, &["add", "Child C", "--parent", &g]));
    let l = id_of(env.json(&forge, &["add", "Loose L", "-p", "2"]));
    let d = id_of(env.json(&forge, &["add", "Done D", "-p", "2"]));
    env.json(&forge, &["done", &d]);
    let r = id_of(env.json(&forge, &["add", "Recur R", "-p", "3", "--every", "7d"]));
    env.json(&forge, &["done", &r]);

    let list = forge.join("list.toml");
    let out = env.json(
        &forge,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ],
    );
    assert_eq!(out["path"], list.to_str().unwrap());
    assert_eq!(out["tasks"], 4);
    assert_eq!(out["warnings"], serde_json::json!([]));

    let text = std::fs::read_to_string(&list).unwrap();
    let doc: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(doc["version"].as_integer(), Some(1));
    assert_eq!(doc["source"].as_str(), Some("forge"));
    assert_eq!(doc["successor"].as_str(), Some("forge2"));
    let entries = doc["task"].as_array().unwrap();
    let ids: Vec<_> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [g.as_str(), c.as_str(), l.as_str(), r.as_str()]);
    for entry in entries {
        let id = entry["id"].as_str().unwrap();
        let bytes = std::fs::read(forge.join(format!("tasks/{id}.md"))).unwrap();
        assert_eq!(
            entry["digest"].as_str().unwrap(),
            format!("sha256:{:x}", Sha256::digest(&bytes))
        );
        assert_eq!(entry["decision"].as_str(), Some(""));
    }
    assert!(!text.contains(&d));
    let above = |id: &str| {
        let at = text.find(&format!("id = \"{id}\"")).unwrap();
        let head = text[..at].rfind("[[task]]").unwrap();
        let prev = text[..head].rfind("id = \"").unwrap_or(0);
        text[prev..head].to_string() + &text[head..at]
    };
    let g_head = above(&g);
    assert!(g_head.contains("Goal G"), "{g_head}");
    assert!(g_head.contains(&format!("children: {c}")), "{g_head}");
    assert!(above(&c).contains(&format!("parent: {g}")), "{}", above(&c));
}

#[test]
fn draft_refuses_an_existing_path() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    env.json(&forge, &["add", "A"]);
    let list = forge.join("list.toml");
    let args = [
        "migrate",
        "--to",
        "forge2",
        "--draft",
        list.to_str().unwrap(),
    ];
    env.json(&forge, &args);
    let before = std::fs::read(&list).unwrap();
    assert_eq!(env.fail(&forge, &args), "validation");
    assert_eq!(std::fs::read(&list).unwrap(), before);
}

#[test]
fn migrate_usage_errors() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.usage(&forge, &["migrate"]);
    env.usage(&forge, &["migrate", "--draft", "x.toml"]);
    env.usage(
        &forge,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            "x.toml",
            "--explain",
        ],
    );
    env.usage(
        &forge,
        &[
            "migrate",
            "list.toml",
            "--to",
            "forge2",
            "--draft",
            "x.toml",
        ],
    );
}

#[test]
fn migrate_open_refusals_match_move() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    env.json(&forge, &["add", "A"]);
    let list = forge.join("list.toml");
    let l = list.to_str().unwrap();
    assert_eq!(
        fail_detail(&env, &forge, &["migrate", "--to", "forge", "--draft", l]).0,
        "config"
    );
    assert_eq!(
        fail_detail(&env, &forge, &["migrate", "--to", "unknown", "--draft", l]).0,
        "config"
    );
    git(&env, &forge, &["init", "-q", "-b", "main"]);
    git(&env, &forge, &["add", "-A"]);
    git(&env, &forge, &["commit", "-qm", "seed"]);
    let side = forge.join("wt");
    git(
        &env,
        &forge,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let (kind, detail) = fail_detail(&env, &side, &["migrate", "--to", "forge2", "--draft", l]);
    assert_eq!(kind, "validation");
    assert!(
        detail.contains(&format!("tasks -C {}", forge.display())),
        "{detail}"
    );
    assert!(detail.contains("migrate"), "{detail}");
    assert!(!list.exists());
}

fn comment_above(text: &str, id: &str) -> String {
    let at = text.find(&format!("id = \"{id}\"")).unwrap();
    let head = text[..at].rfind("[[task]]").unwrap();
    let start = text[..head]
        .rfind("\n\n")
        .map(|n| n + 2)
        .unwrap_or_default();
    text[start..head].to_string()
}

fn draft(env: &TestEnv, dir: &std::path::Path, name: &str) -> (String, String) {
    let list = dir.join(name);
    let out = env.json(
        dir,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ],
    );
    assert_eq!(out["warnings"], serde_json::json!([]));
    (std::fs::read_to_string(&list).unwrap(), name.to_string())
}

#[test]
fn draft_comments_carry_claims_parks_dependents_and_unresolved_links() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let ops_dir = env.init("ops");
    for dir in ["docs/specs", "docs/plans", "docs/notes"] {
        std::fs::create_dir_all(forge.join(dir)).unwrap();
    }
    std::fs::write(forge.join("docs/specs/design.md"), "# Design\n").unwrap();
    std::fs::write(
        forge.join("docs/plans/work.md"),
        "# Plan\n\n### Task 1: one\n",
    )
    .unwrap();
    std::fs::write(forge.join("docs/notes/b.md"), "brief\n").unwrap();
    let claimed = id_of(env.json(&forge, &["add", "Claimed one"]));
    env.json(&forge, &["start", &claimed]);
    let parked = id_of(env.json(&forge, &["add", "Parked one"]));
    env.json(&forge, &["start", &parked]);
    env.json(&forge, &["park", &parked, "resume the parser"]);
    let needed = id_of(env.json(&forge, &["add", "Needed one"]));
    let linked = id_of(env.json(
        &forge,
        &[
            "add",
            "Linked one",
            "--spec",
            "docs/specs/design.md",
            "--plan",
            "docs/plans/work.md",
            "--step",
            "Task 1: one",
            "--body",
            "Read [ref:brief] first.",
            "--ref",
            "brief (input): docs/notes/b.md",
        ],
    ));
    let dependent = id_of(env.json(&ops_dir, &["add", "Waits on forge", "--depends", &needed]));

    let (text, _) = draft(&env, &forge, "list.toml");
    let claim = comment_above(&text, &claimed);
    assert!(claim.contains("LIVE CLAIM"), "{claim}");
    let park = comment_above(&text, &parked);
    assert!(
        park.contains("parked by") && park.contains("resume the parser"),
        "{park}"
    );
    let need = comment_above(&text, &needed);
    assert!(
        need.contains(&format!("open dependent in another project: {dependent}")),
        "{need}"
    );
    // spec, plan, step and ref all resolve in the successor only if present there.
    let missing = comment_above(&text, &linked);
    assert!(
        missing.contains("spec docs/specs/design.md does not resolve in forge2"),
        "{missing}"
    );
    assert!(
        missing.contains("plan docs/plans/work.md does not resolve in forge2"),
        "{missing}"
    );
    assert!(
        missing
            .contains("ref brief docs/notes/b.md does not exist in forge2 (cited as [ref:brief])"),
        "{missing}"
    );

    // Once the successor holds them, the draft says nothing about them.
    for dir in ["docs/specs", "docs/plans", "docs/notes"] {
        std::fs::create_dir_all(forge2.join(dir)).unwrap();
    }
    std::fs::write(forge2.join("docs/specs/design.md"), "# Design\n").unwrap();
    std::fs::write(
        forge2.join("docs/plans/work.md"),
        "# Plan\n\n### Task 1: one\n",
    )
    .unwrap();
    std::fs::write(forge2.join("docs/notes/b.md"), "brief\n").unwrap();
    let (text, _) = draft(&env, &forge, "again.toml");
    assert!(
        !comment_above(&text, &linked).contains("does not"),
        "{}",
        comment_above(&text, &linked)
    );
    // A plan present but without the step is reported as a missing step.
    std::fs::write(forge2.join("docs/plans/work.md"), "# Plan\n").unwrap();
    let (text, _) = draft(&env, &forge, "step.toml");
    assert!(
        comment_above(&text, &linked)
            .contains("step \"Task 1: one\" of docs/plans/work.md not found")
    );
}

#[test]
fn draft_stops_on_a_resolver_error_instead_of_commenting_it_away() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    std::fs::create_dir_all(forge.join("docs/notes")).unwrap();
    std::fs::write(forge.join("docs/notes/b.md"), "brief\n").unwrap();
    env.json(
        &forge,
        &["add", "Linked", "--ref", "brief (input): docs/notes/b.md"],
    );
    // A symlink loop in the successor is not "absent": looking at it fails with ELOOP.
    std::fs::create_dir_all(forge2.join("docs/notes")).unwrap();
    std::os::unix::fs::symlink("b.md", forge2.join("docs/notes/b.md")).unwrap();
    let list = forge.join("list.toml");
    let out = env
        .cmd(&forge)
        .args([
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(!list.exists(), "a failed draft writes nothing");
}

/// A draft written outside the project, so a run's snapshot of the project stays whole.
fn draft_home(env: &TestEnv, dir: &std::path::Path) -> (String, String) {
    let list = env.home.path().join("list.toml");
    let out = env.json(
        dir,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ],
    );
    assert_eq!(out["warnings"], serde_json::json!([]));
    (std::fs::read_to_string(&list).unwrap(), "list.toml".into())
}

/// Sets the decision line of one drafted entry, with extra lines after it.
fn decide(text: &str, id: &str, decision: &str, extra: &str) -> String {
    let at = text.find(&format!("id = \"{id}\"")).unwrap();
    let line = at + text[at..].find("decision = \"\"\n").unwrap();
    let end = line + "decision = \"\"\n".len();
    format!(
        "{}decision = \"{decision}\"\n{extra}{}",
        &text[..line],
        &text[end..]
    )
}

fn explain(env: &TestEnv, dir: &std::path::Path, list: &std::path::Path) -> serde_json::Value {
    env.json(dir, &["migrate", list.to_str().unwrap(), "--explain"])
}

fn rules(out: &serde_json::Value) -> Vec<(String, String)> {
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

fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().display().to_string(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn explain_reports_every_refusal_at_once() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let b = id_of(env.json(&forge, &["add", "B"]));
    let c = id_of(env.json(&forge, &["add", "C"]));
    let (text, _) = draft_home(&env, &forge);
    env.json(&forge, &["edit", &b, "--title", "B changed"]);
    let e = id_of(env.json(&forge, &["add", "E"]));
    let mut text = decide(&text, &a, "carry", "");
    text = decide(&text, &b, "carry", "");
    // c stays undecided; a is listed a second time.
    text.push_str(&format!(
        "\n[[task]]\nid = \"{a}\"\ndigest = \"sha256:00\"\ndecision = \"carry\"\n"
    ));
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();

    let before = (snapshot(&forge), snapshot(&forge2));
    let out = explain(&env, &forge, &list);
    assert_eq!((snapshot(&forge), snapshot(&forge2)), before);
    assert_eq!(out["recovery"], "refuse");
    assert_eq!(out["source"], "forge");
    assert_eq!(out["successor"], "forge2");
    assert!(out["list"].as_str().unwrap().starts_with("sha256:"));
    let found = rules(&out);
    for want in [
        ("undecided", c.as_str()),
        ("undecided", e.as_str()),
        ("stale_digest", b.as_str()),
        ("listed_twice", a.as_str()),
    ] {
        assert!(
            found.contains(&(want.0.to_string(), want.1.to_string())),
            "{want:?} missing from {found:?}"
        );
    }
    for row in out["refusals"].as_array().unwrap() {
        assert!(!row["settle"].as_str().unwrap().is_empty(), "{row}");
    }
}

#[test]
fn alias_ids_resolve_and_bare_suffixes_refuse() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    env.json(&forge, &["add", "A"]);
    env.json(&forge, &["add", "B"]);
    env.json(&forge, &["rename", "forge", "forgex"]);
    let list = env.home.path().join("list.toml");
    let out = env.json(
        &forge,
        &[
            "migrate",
            "--to",
            "forge2",
            "--draft",
            list.to_str().unwrap(),
        ],
    );
    assert_eq!(out["tasks"], 2);
    let mut text = std::fs::read_to_string(&list).unwrap();
    let doc: toml::Table = toml::from_str(&text).unwrap();
    let ids: Vec<String> = doc["task"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.iter().all(|id| id.starts_with("forgex-")), "{ids:?}");
    let hex = |id: &str| id.rsplit_once('-').unwrap().1.to_string();
    for id in &ids {
        text = decide(&text, id, "carry", "");
    }
    // Both spellings of the first id are accepted; the second is a bare suffix.
    let retired = format!("forge-{}", hex(&ids[0]));
    let accepted = text.replace(
        &format!("id = \"{}\"", ids[0]),
        &format!("id = \"{retired}\""),
    );
    std::fs::write(&list, &accepted).unwrap();
    let out = explain(&env, &forge, &list);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");

    let bare = accepted.replace(
        &format!("id = \"{}\"", ids[1]),
        &format!("id = \"{}\"", hex(&ids[1])),
    );
    std::fs::write(&list, bare).unwrap();
    let out = explain(&env, &forge, &list);
    assert_eq!(out["recovery"], "refuse");
    let rows = out["refusals"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{out}");
    assert!(
        rows[0]["settle"].as_str().unwrap().contains(&ids[1]),
        "{out}"
    );
}

#[test]
fn key_shaped_unknown_reason_warns_but_is_accepted() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ids: Vec<String> = ["A", "B", "C", "D", "E", "F", "G"]
        .iter()
        .map(|t| id_of(env.json(&forge, &["add", t])))
        .collect();
    let (text, _) = draft_home(&env, &forge);
    let mut text = text.replacen("[reasons]\n", "[reasons]\nsuperseded = \"replaced\"\n", 1);
    let reasons = [
        "superseeded",
        "obsolete",
        "done in forge2-abc123",
        "superseded",
        "Superseded",
        "a_b",
        "2fast",
    ];
    for (id, reason) in ids.iter().zip(reasons) {
        text = decide(&text, id, "drop", &format!("reason = \"{reason}\"\n"));
    }
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let out = explain(&env, &forge, &list);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
    let warnings: Vec<String> = out["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect();
    for word in ["superseeded", "obsolete"] {
        assert!(
            warnings
                .iter()
                .any(|w| w.contains(word) && w.contains("[reasons]")),
            "{word}: {warnings:?}"
        );
    }
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

#[test]
fn drop_and_close_need_reasons_and_keys_belong_to_their_decision() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|t| id_of(env.json(&forge, &["add", t])))
        .collect();
    let (text, _) = draft_home(&env, &forge);
    let mut text = decide(&text, &ids[0], "drop", "");
    text = decide(
        &text,
        &ids[1],
        "drop",
        "reason = \"gone\"\nparent = \"none\"\n",
    );
    text = decide(&text, &ids[2], "carry", "release = [\"ops-000001\"]\n");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let found = rules(&explain(&env, &forge, &list));
    for want in [
        ("missing_reason", &ids[0]),
        ("bad_key", &ids[1]),
        ("bad_key", &ids[2]),
    ] {
        assert!(
            found.contains(&(want.0.to_string(), want.1.clone())),
            "{want:?} missing from {found:?}"
        );
    }
}

#[test]
fn close_on_a_recurrence_refuses() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let r = id_of(env.json(&forge, &["add", "R", "--every", "7d"]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &r, "close", "reason = \"done elsewhere\"\n");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let out = explain(&env, &forge, &list);
    let row = &out["refusals"][0];
    assert_eq!(row["rule"], "close_recurring", "{out}");
    assert_eq!(row["id"], r.as_str());
    let settle = row["settle"].as_str().unwrap();
    assert!(
        settle.contains("drop") && settle.contains("carry"),
        "{settle}"
    );
}

#[test]
fn live_claim_and_other_checkout_refuse() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    git(&env, &forge, &["init", "-q", "-b", "main"]);
    git(&env, &forge, &["add", "-A"]);
    git(&env, &forge, &["commit", "-qm", "seed"]);
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &a, "carry", "");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    assert_eq!(rules(&explain(&env, &forge, &list)), vec![]);

    common::write_claim(&env, "forge", &a, "sess", true);
    let out = explain(&env, &forge, &list);
    assert_eq!(
        rules(&out),
        vec![("claimed".to_string(), a.clone())],
        "{out}"
    );
    std::fs::remove_file(env.claim_store("forge")).unwrap();

    // A worktree holding only an identical copy of a listed task still blocks `retire`.
    let side = forge.join("wt");
    git(
        &env,
        &forge,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let out = explain(&env, &forge, &list);
    assert_eq!(
        rules(&out),
        vec![("other_checkout".to_string(), a.clone())],
        "{out}"
    );
    assert!(
        out["refusals"][0]["settle"]
            .as_str()
            .unwrap()
            .contains("wt"),
        "{out}"
    );
    // An extra open record there is named too.
    let extra = id_of(env.json(&side, &["add", "Extra"]));
    let found = rules(&explain(&env, &forge, &list));
    assert!(
        found.contains(&("other_checkout".to_string(), extra)),
        "{found:?}"
    );

    git(
        &env,
        &forge,
        &["worktree", "remove", "--force", side.to_str().unwrap()],
    );
    let out = explain(&env, &forge, &list);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
}

#[test]
fn explain_refuses_a_retired_successor() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let (text, _) = draft_home(&env, &forge);
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, decide(&text, &a, "carry", "")).unwrap();
    env.json(&forge2, &["retire", "--reason", "done"]);
    let (kind, _) = fail_detail(
        &env,
        &forge,
        &["migrate", list.to_str().unwrap(), "--explain"],
    );
    assert_eq!(kind, "retired_project");
}

#[test]
fn migrate_list_takes_no_to() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    env.usage(
        &forge,
        &["migrate", "list.toml", "--to", "forge2", "--explain"],
    );
}

fn setting(out: &serde_json::Value, rule: &str) -> Vec<(String, String)> {
    out["refusals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["rule"] == rule)
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_string(),
                r["settle"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn header_not_open_foreign_and_decision_text_refusals() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let b = id_of(env.json(&forge, &["add", "B"]));
    let closed = id_of(env.json(&forge, &["add", "Closed"]));
    env.json(&forge, &["done", &closed]);
    let (text, _) = draft_home(&env, &forge);
    let mut text = decide(&text, &b, "keep", "");
    text = text.replace("source = \"forge\"", "source = \"elsewhere\"");
    for id in [closed.as_str(), "forge2-abc123", "forge-abcdef"] {
        text.push_str(&format!(
            "\n[[task]]\nid = \"{id}\"\ndigest = \"sha256:00\"\ndecision = \"carry\"\n"
        ));
    }
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let out = explain(&env, &forge, &list);
    assert_eq!(out["recovery"], "refuse");
    assert_eq!(setting(&out, "header_mismatch")[0].0, "list");
    assert_eq!(setting(&out, "header_mismatch").len(), 1, "{out}");
    assert_eq!(setting(&out, "not_open")[0].0, closed);
    let foreign: Vec<String> = setting(&out, "foreign_id")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(foreign, ["forge2-abc123", "forge-abcdef"], "{out}");
    assert_eq!(setting(&out, "bad_key")[0].0, b);
    assert!(
        setting(&out, "bad_key")[0]
            .1
            .contains("carry, drop or close")
    );
    // A is still the draft's empty decision.
    assert_eq!(setting(&out, "undecided")[0].0, a);
}

#[test]
fn a_pending_move_is_a_refusal_and_an_unreadable_inventory_is_not() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let (text, _) = draft_home(&env, &forge);
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, decide(&text, &a, "carry", "")).unwrap();
    env.cmd(&forge)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &a, "--to", "forge2"])
        .assert()
        .success();
    let out = explain(&env, &forge, &list);
    assert_eq!(out["recovery"], "refuse");
    let pending = setting(&out, "pending_operation");
    assert_eq!(pending.len(), 1, "{out}");
    assert!(pending[0].1.contains("pending"), "{out}");

    // An unreadable rename inventory is an error of its own, not a pending operation.
    let journals = env.home.path().join(".local/state/tasks/move");
    std::fs::remove_dir_all(&journals).unwrap();
    let inventories = env.home.path().join(".local/state/tasks/rename");
    std::fs::create_dir_all(&inventories).unwrap();
    std::fs::write(inventories.join("zz.toml"), "not = [valid").unwrap();
    let (kind, _) = fail_detail(
        &env,
        &forge,
        &["migrate", list.to_str().unwrap(), "--explain"],
    );
    assert_eq!(kind, "config");
}

/// Writes the list beside the scratch HOME and returns `--explain`'s output.
fn explain_text(env: &TestEnv, dir: &std::path::Path, text: &str) -> serde_json::Value {
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    explain(env, dir, &list)
}

fn carried_row<'a>(out: &'a serde_json::Value, from: &str) -> &'a serde_json::Value {
    out["carried"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["from"] == from)
        .unwrap_or_else(|| panic!("{from} not carried in {out}"))
}

fn rewrites_of(out: &serde_json::Value, from: &str) -> Vec<String> {
    carried_row(out, from)["rewrites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn carried_child_of_a_dropped_goal_needs_a_parent() {
    // Spec §5 case 1, validation half.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let b = id_of(env.json(&forge, &["add", "B", "--parent", &g]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &g, "--depends", &b]));
    let c = id_of(env.json(&forge, &["add", "C", "--parent", &g]));
    let s = id_of(env.json(&forge2, &["add", "Successor goal S"]));
    let (text, _) = draft_home(&env, &forge);
    let mut text = decide(&text, &g, "drop", "reason = \"superseded\"\n");
    text = decide(&text, &c, "close", "reason = \"done in forge2\"\n");
    let bare = decide(&decide(&text, &a, "carry", ""), &b, "carry", "");
    let out = explain_text(&env, &forge, &bare);
    assert_eq!(out["recovery"], "refuse");
    let mut found = rules(&out);
    found.sort();
    let mut want = vec![
        ("parent_required".to_string(), a.clone()),
        ("parent_required".to_string(), b.clone()),
    ];
    want.sort();
    assert_eq!(found, want, "{out}");
    for (_, settle) in setting(&out, "parent_required") {
        assert!(settle.contains("`parent"), "{settle}");
    }

    text = decide(&text, &a, "carry", "parent = \"none\"\n");
    text = decide(&text, &b, "carry", &format!("parent = \"{s}\"\n"));
    let out = explain_text(&env, &forge, &text);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
    let row_b = carried_row(&out, &b);
    assert_eq!(row_b["parent"], s.as_str(), "{out}");
    assert!(
        row_b.get("id").is_none(),
        "a fresh verdict assigns no ids: {out}"
    );
    assert_eq!(carried_row(&out, &a)["parent"], serde_json::Value::Null);
    let rewrites_a = rewrites_of(&out, &a);
    assert!(
        rewrites_a.contains(&format!("parent {g} not carried; set to none")),
        "{rewrites_a:?}"
    );
    assert!(
        rewrites_a.contains(&format!("dependency {b} carried as new:{b}")),
        "{rewrites_a:?}"
    );
    assert!(
        rewrites_of(&out, &b).contains(&format!("parent {g} not carried; set to {s}")),
        "{out}"
    );
}

#[test]
fn carried_subtree_keeps_its_shape() {
    // Spec §5 case 2: validation runs after the rewrites, so A and B are valid under G'.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &g]));
    let b = id_of(env.json(&forge, &["add", "B", "--parent", &g]));
    let (mut text, _) = draft_home(&env, &forge);
    for id in [&g, &a, &b] {
        text = decide(&text, id, "carry", "");
    }
    let out = explain_text(&env, &forge, &text);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
    assert_eq!(carried_row(&out, &g)["parent"], serde_json::Value::Null);
    for child in [&a, &b] {
        assert_eq!(
            carried_row(&out, child)["parent"],
            format!("new:{g}"),
            "{out}"
        );
        assert!(
            rewrites_of(&out, child).contains(&format!("parent {g} carried as new:{g}")),
            "{out}"
        );
    }
}

#[test]
fn carried_parent_override_must_name_carried_or_open_successor_work() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let d = id_of(env.json(&forge, &["add", "D"]));
    let shut = id_of(env.json(&forge2, &["add", "Closed successor"]));
    env.json(&forge2, &["done", &shut]);
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &d, "drop", "reason = \"gone\"\n");
    for parent in [d.as_str(), shut.as_str(), "ops-abc123", "forge2-abcdef"] {
        let out = explain_text(
            &env,
            &forge,
            &decide(&text, &a, "carry", &format!("parent = \"{parent}\"\n")),
        );
        assert_eq!(
            rules(&out),
            vec![("parent_invalid".to_string(), a.clone())],
            "{parent}: {out}"
        );
    }
}

#[test]
fn dependency_on_dropped_work_needs_rm_depends() {
    // Spec §5 case 3.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let k = id_of(env.json(&forge, &["add", "K"]));
    env.json(&forge, &["done", &k]);
    let a = id_of(env.json(&forge, &["add", "A", "--depends", &d, "--depends", &k]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &d, "drop", "reason = \"gone\"\n");
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", ""));
    assert_eq!(
        rules(&out),
        vec![("depends_on_dropped".to_string(), a.clone())],
        "{out}"
    );
    let settle = &setting(&out, "depends_on_dropped")[0].1;
    assert!(
        settle.contains("rm_depends") && settle.contains(&d),
        "{settle}"
    );

    // rm_depends may name only a dependency this list drops.
    let wrong = decide(
        &text,
        &a,
        "carry",
        &format!("rm_depends = [\"{d}\", \"{k}\"]\n"),
    );
    let out = explain_text(&env, &forge, &wrong);
    assert_eq!(
        rules(&out),
        vec![("bad_key".to_string(), a.clone())],
        "{out}"
    );

    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &a, "carry", &format!("rm_depends = [\"{d}\"]\n")),
    );
    assert_eq!(rules(&out), vec![], "{out}");
    let rewrites = rewrites_of(&out, &a);
    assert_eq!(
        rewrites,
        [format!("removed dependency {d} (dropped)")],
        "{out}"
    );
}

#[test]
fn unresolved_plan_link_needs_links_drop() {
    // Spec §5 case 6: valid only because validation runs on the rewritten record.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    std::fs::create_dir_all(forge.join("docs/plans")).unwrap();
    std::fs::write(forge.join("docs/plans/x.md"), "# Plan\n\n### Task 1: one\n").unwrap();
    let a = id_of(env.json(
        &forge,
        &[
            "add",
            "A",
            "--plan",
            "docs/plans/x.md",
            "--step",
            "Task 1: one",
        ],
    ));
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", ""));
    assert_eq!(
        rules(&out),
        vec![("links_unresolved".to_string(), a.clone())],
        "{out}"
    );
    let settle = &setting(&out, "links_unresolved")[0].1;
    assert!(
        settle.contains("links = \"drop\"") && settle.contains("docs/plans/x.md"),
        "{settle}"
    );

    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &a, "carry", "links = \"drop\"\n"),
    );
    assert_eq!(rules(&out), vec![], "{out}");
    let rewrites = rewrites_of(&out, &a);
    assert!(
        rewrites.contains(&"plan link docs/plans/x.md dropped (not in forge2)".to_string()),
        "{rewrites:?}"
    );
    assert!(
        rewrites.contains(&"step \"Task 1: one\" dropped with its plan".to_string()),
        "{rewrites:?}"
    );

    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &a, "carry", "links = \"keep\"\n"),
    );
    assert!(
        rules(&out).contains(&("bad_key".to_string(), a.clone())),
        "{out}"
    );
}

#[test]
fn cited_reference_needs_a_replacement_body() {
    // Spec §5 case 7.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    std::fs::create_dir_all(forge.join("docs/notes")).unwrap();
    std::fs::write(forge.join("docs/notes/b.md"), "brief\n").unwrap();
    let a = id_of(env.json(
        &forge,
        &[
            "add",
            "A",
            "--body",
            "Intro.\nSee [ref:brief] for the question.\nOutro.",
            "--ref",
            "brief (input): docs/notes/b.md",
        ],
    ));
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &a, "carry", "links = \"drop\"\n"),
    );
    assert_eq!(
        rules(&out),
        vec![("citation_dangling".to_string(), a.clone())],
        "{out}"
    );
    let settle = &setting(&out, "citation_dangling")[0].1;
    assert!(
        settle.contains("[ref:brief]") && settle.contains("`body`"),
        "{settle}"
    );

    // A replacement that still cites the dropped handle refuses too.
    let still = "links = \"drop\"\nbody = \"\"\"\nSee [ref:brief] again.\n\"\"\"\n";
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", still));
    assert_eq!(
        rules(&out),
        vec![("citation_dangling".to_string(), a.clone())],
        "{out}"
    );

    let good = "links = \"drop\"\nbody = \"\"\"\nIntro.\nThe question is settled in forge2.\nOutro.\n\"\"\"\n";
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", good));
    assert_eq!(rules(&out), vec![], "{out}");
    let rewrites = rewrites_of(&out, &a);
    assert!(
        rewrites.contains(&"ref brief docs/notes/b.md dropped (not in forge2)".to_string()),
        "{rewrites:?}"
    );
    assert!(
        rewrites
            .iter()
            .any(|r| r.starts_with("body replaced") && r.contains("brief")),
        "{rewrites:?}"
    );
    let changes = out["body_changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{out}");
    assert_eq!(changes[0]["id"], a.as_str());
    let diff = changes[0]["diff"].as_str().unwrap();
    assert_eq!(
        diff,
        " Intro.\n-See [ref:brief] for the question.\n+The question is settled in forge2.\n Outro.\n"
    );
}

#[test]
fn replacement_body_round_trips_exactly() {
    // Plan Review Focus 4: the projected body is the list's body after the record
    // format's normalisation (trailing whitespace trimmed), non-ASCII intact.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A", "--body", "Old body."]));
    let (text, _) = draft_home(&env, &forge);
    let body = "body = \"\"\"\nLine é\n\n- item\n\"\"\"\n";
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", body));
    assert_eq!(rules(&out), vec![], "{out}");
    let diff = out["body_changes"][0]["diff"].as_str().unwrap();
    assert_eq!(diff, "-Old body.\n+Line é\n+\n+- item\n");
    assert!(
        rewrites_of(&out, &a).contains(&"body replaced".to_string()),
        "{out}"
    );

    // A body with a reserved delimiter is refused rather than mangled.
    let bad = "body = \"\"\"\nx\n## Notes\ny\n\"\"\"\n";
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", bad));
    assert_eq!(
        rules(&out),
        vec![("bad_key".to_string(), a.clone())],
        "{out}"
    );
}

#[test]
fn parent_override_cycle_refuses() {
    // Spec §5 case 14.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let b = id_of(env.json(&forge, &["add", "B"]));
    let (text, _) = draft_home(&env, &forge);
    let mut cyclic = decide(&text, &a, "carry", &format!("parent = \"{b}\"\n"));
    cyclic = decide(&cyclic, &b, "carry", &format!("parent = \"{a}\"\n"));
    let out = explain_text(&env, &forge, &cyclic);
    let found = rules(&out);
    assert_eq!(found.len(), 1, "{out}");
    assert_eq!(found[0].0, "parent_cycle", "{out}");
    let settle = &setting(&out, "parent_cycle")[0].1;
    assert!(settle.contains(&a) && settle.contains(&b), "{settle}");

    // A dependency cycle through the successor: S depends on A, written by hand, and A
    // depends on S; after the run A's tombstone points at A'.
    let s = id_of(env.json(&forge2, &["add", "S"]));
    let c = id_of(env.json(&forge, &["add", "C", "--depends", &s]));
    let path = forge2.join(format!("tasks/{s}.md"));
    let record = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        record.replacen("depends: []", &format!("depends: [{c}]"), 1),
    )
    .unwrap();
    let (text, _) = {
        std::fs::remove_file(env.home.path().join("list.toml")).unwrap();
        draft_home(&env, &forge)
    };
    let mut list = decide(&text, &a, "carry", "");
    list = decide(&list, &b, "carry", "");
    list = decide(&list, &c, "carry", "");
    let out = explain_text(&env, &forge, &list);
    assert_eq!(
        rules(&out),
        vec![("dependency_cycle".to_string(), c.clone())],
        "{out}"
    );
    assert!(setting(&out, "dependency_cycle")[0].1.contains(&s), "{out}");
}

#[test]
fn done_recurrence_carries_with_its_anchor() {
    // Spec §5 case 9, recurrence half: a recurrence whose occurrence is done carries
    // (move's open-status rule does not apply to it).
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let r = id_of(env.json(&forge, &["add", "R", "--every", "7d"]));
    env.json(&forge, &["done", &r]);
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(&env, &forge, &decide(&text, &r, "carry", ""));
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
    let row = carried_row(&out, &r);
    assert_eq!(row["rewrites"], serde_json::json!([]), "{out}");
    assert_eq!(row["park"], false);
}

#[test]
fn carried_need_must_be_declared_in_the_successor() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let config = forge.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[needs.gpu]\nmeaning = \"a GPU host\"\n");
    std::fs::write(&config, text).unwrap();
    let a = id_of(env.json(&forge, &["add", "A", "--need", "gpu"]));
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(&env, &forge, &decide(&text, &a, "carry", ""));
    assert_eq!(
        rules(&out),
        vec![("destination_invalid".to_string(), a.clone())],
        "{out}"
    );
    assert!(
        setting(&out, "destination_invalid")[0].1.contains("gpu"),
        "{out}"
    );
}

#[test]
fn carried_lane_under_a_successor_lane_refuses_and_a_carried_lane_tree_is_valid() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let l = id_of(env.json(&forge, &["add", "Lane L", "--lane", "--origin", "user"]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &l]));
    let outer = id_of(env.json(&forge2, &["add", "Outer", "--lane", "--origin", "user"]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &a, "carry", "");
    // Carried with its lane: valid, the child under the projected lane.
    let out = explain_text(&env, &forge, &decide(&text, &l, "carry", ""));
    assert_eq!(rules(&out), vec![], "{out}");
    // The lane re-parented under a successor lane: nested.
    let nested = decide(&text, &l, "carry", &format!("parent = \"{outer}\"\n"));
    let out = explain_text(&env, &forge, &nested);
    assert_eq!(
        rules(&out),
        vec![("destination_invalid".to_string(), l.clone())],
        "{out}"
    );
    assert!(
        setting(&out, "destination_invalid")[0].1.contains("lane"),
        "{out}"
    );
}

fn row<'a>(out: &'a serde_json::Value, key: &str, id: &str) -> &'a serde_json::Value {
    out[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} in {out}"))
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("{id} not in {key} of {out}"))
}

#[test]
fn drop_with_an_outside_dependent_needs_release() {
    // Spec §5 case 4: an open dependent in another project must be released by name.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let x = id_of(env.json(&ops, &["add", "X", "--depends", &d]));
    let z = id_of(env.json(&ops, &["add", "Z"]));
    // A closed dependent, and a dependent this list also drops, need no release.
    let gone = id_of(env.json(&ops, &["add", "Gone", "--depends", &d]));
    env.json(&ops, &["drop", &gone]);
    let e = id_of(env.json(&forge, &["add", "E", "--depends", &d]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &e, "drop", "reason = \"gone\"\n");

    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &d, "drop", "reason = \"superseded\"\n"),
    );
    assert_eq!(
        rules(&out),
        vec![("release_mismatch".to_string(), d.clone())],
        "{out}"
    );
    let settle = &setting(&out, "release_mismatch")[0].1;
    assert!(
        settle.contains(&x) && settle.contains("`release"),
        "{settle}"
    );
    assert!(!settle.contains(&e) && !settle.contains(&gone), "{settle}");

    let extra = format!("reason = \"superseded\"\nrelease = [\"{x}\", \"{z}\"]\n");
    let out = explain_text(&env, &forge, &decide(&text, &d, "drop", &extra));
    assert_eq!(
        rules(&out),
        vec![("release_mismatch".to_string(), d.clone())],
        "{out}"
    );
    assert!(setting(&out, "release_mismatch")[0].1.contains(&z), "{out}");

    let before = snapshot(&ops);
    let exact = format!("reason = \"superseded\"\nrelease = [\"{x}\"]\n");
    let out = explain_text(&env, &forge, &decide(&text, &d, "drop", &exact));
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(out["recovery"], "fresh");
    let dropped = row(&out, "dropped", &d);
    assert_eq!(dropped["reason"], "superseded");
    assert_eq!(dropped["released"], serde_json::json!([x]));
    assert_eq!(row(&out, "dropped", &e)["released"], serde_json::json!([]));
    assert_eq!(snapshot(&ops), before);
}

#[test]
fn close_releases_carried_dependents_by_v1_id() {
    // Spec §4.3: a closed task's carried dependents keep the edge, now satisfied, and are
    // released by their v1 ids; its open dependencies do not block the close.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let y = id_of(env.json(&ops, &["add", "Y"]));
    let c = id_of(env.json(&forge, &["add", "C", "--depends", &y]));
    let a = id_of(env.json(&forge, &["add", "A", "--depends", &c]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &a, "carry", "");
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &c, "close", "reason = \"done in forge2\"\n"),
    );
    assert_eq!(
        rules(&out),
        vec![("release_mismatch".to_string(), c.clone())],
        "{out}"
    );
    assert!(setting(&out, "release_mismatch")[0].1.contains(&a), "{out}");

    let exact = format!("reason = \"done in forge2\"\nrelease = [\"{a}\"]\n");
    let out = explain_text(&env, &forge, &decide(&text, &c, "close", &exact));
    assert_eq!(rules(&out), vec![], "{out}");
    let closed = row(&out, "closed", &c);
    assert_eq!(closed["reason"], "done in forge2");
    assert_eq!(closed["released"], serde_json::json!([a]));
    assert_eq!(closed["open_depends"], serde_json::json!([y]));
}

#[test]
fn dropped_reviewed_idea_and_shelf_are_clean() {
    // Spec §5 case 8, projection half: each record renders as `drop` would write it and
    // passes the write validation (the rendered text is pinned by the unit tests).
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let w = id_of(env.json(&ops, &["add", "W"]));
    let idea = id_of(env.json(&forge, &["add", "I", "--status", "idea"]));
    env.json(&forge, &["edit", &idea, "--reviewed"]);
    let shelf = id_of(env.json(&forge, &["add", "S"]));
    env.json(
        &forge,
        &[
            "shelve",
            &shelf,
            "when W lands",
            "--defer",
            "2099-01-01",
            "--wake-on",
            &w,
        ],
    );
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &idea, "drop", "reason = \"gone\"\n");
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &shelf, "drop", "reason = \"gone\"\n"),
    );
    assert_eq!(rules(&out), vec![], "{out}");
    for id in [&idea, &shelf] {
        assert_eq!(row(&out, "dropped", id)["reason"], "gone");
    }
    assert_eq!(out["woken"], serde_json::json!([]), "{out}");
}

#[test]
fn close_with_a_missing_declared_output_refuses() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let c = id_of(env.json(
        &forge,
        &[
            "add",
            "C",
            "--ref",
            "report (output, kind=report): docs/notes/r.md",
        ],
    ));
    let (text, _) = draft_home(&env, &forge);
    let close = decide(&text, &c, "close", "reason = \"done\"\n");
    let out = explain_text(&env, &forge, &close);
    assert_eq!(
        rules(&out),
        vec![("close_outputs".to_string(), c.clone())],
        "{out}"
    );
    assert!(
        setting(&out, "close_outputs")[0]
            .1
            .contains("docs/notes/r.md"),
        "{out}"
    );
    // A drop declares nothing finished.
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &c, "drop", "reason = \"gone\"\n"),
    );
    assert_eq!(rules(&out), vec![], "{out}");

    std::fs::create_dir_all(forge.join("docs/notes")).unwrap();
    std::fs::write(forge.join("docs/notes/r.md"), "report\n").unwrap();
    let out = explain_text(&env, &forge, &close);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(
        row(&out, "closed", &c)["open_depends"],
        serde_json::json!([])
    );
}

#[test]
fn done_recurrence_drop_ends_the_cadence() {
    // Spec §5 case 9, drop half (the record itself is pinned by the unit tests).
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let r = id_of(env.json(&forge, &["add", "R", "--every", "7d"]));
    env.json(&forge, &["done", &r]);
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &r, "drop", "reason = \"cadence ends\"\n"),
    );
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(row(&out, "dropped", &r)["reason"], "cadence ends");
}

#[test]
fn woken_shelves_are_reported_not_required() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let lost = env.init("lost");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let y = id_of(env.json(&ops, &["add", "Y"]));
    env.json(
        &ops,
        &[
            "shelve",
            &y,
            "when D lands",
            "--defer",
            "2099-01-01",
            "--wake-on",
            &d,
        ],
    );
    let (text, _) = draft_home(&env, &forge);
    std::fs::remove_dir_all(&lost).unwrap();
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &d, "drop", "reason = \"gone\"\n"),
    );
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(
        out["woken"],
        serde_json::json!([{"id": d, "shelves": [y]}]),
        "{out}"
    );
    assert_eq!(out["unread_projects"], serde_json::json!(["lost"]), "{out}");
    assert_eq!(row(&out, "dropped", &d)["released"], serde_json::json!([]));
}

#[test]
fn inbound_lists_dependents_of_carried_tasks_outside_the_carried_set() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let b = id_of(env.json(&forge, &["add", "B", "--depends", &a]));
    let x = id_of(env.json(&ops, &["add", "X", "--depends", &a]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&decide(&text, &a, "carry", ""), &b, "carry", "");
    let out = explain_text(&env, &forge, &text);
    assert_eq!(rules(&out), vec![], "{out}");
    assert_eq!(
        out["inbound"],
        serde_json::json!([{"id": a, "dependents": [x]}]),
        "{out}"
    );
}

#[test]
fn explain_joins_list_and_projection_refusals() {
    // Ruling R6: list checks and projections report together; a header mismatch skips the
    // projection.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let g = id_of(env.json(&forge, &["add", "G"]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &g]));
    let b = id_of(env.json(&forge, &["add", "B"]));
    let d = id_of(env.json(&forge, &["add", "D"]));
    env.json(&ops, &["add", "X", "--depends", &d]);
    let (text, _) = draft_home(&env, &forge);
    env.json(&forge, &["edit", &b, "--title", "B changed"]);
    let mut text = decide(&text, &g, "drop", "reason = \"gone\"\n");
    text = decide(&text, &a, "carry", "");
    text = decide(&text, &b, "carry", "");
    text = decide(&text, &d, "drop", "reason = \"gone\"\n");
    let out = explain_text(&env, &forge, &text);
    let mut found = rules(&out);
    found.sort();
    let mut want = vec![
        ("parent_required".to_string(), a.clone()),
        ("release_mismatch".to_string(), d.clone()),
        ("stale_digest".to_string(), b.clone()),
    ];
    want.sort();
    assert_eq!(found, want, "{out}");

    let out = explain_text(
        &env,
        &forge,
        &text.replace("source = \"forge\"", "source = \"elsewhere\""),
    );
    let found: Vec<String> = rules(&out).into_iter().map(|(rule, _)| rule).collect();
    assert!(found.contains(&"header_mismatch".to_string()), "{out}");
    assert!(
        !found
            .iter()
            .any(|r| r == "parent_required" || r == "release_mismatch"),
        "{out}"
    );
}

#[test]
fn refusals_name_unassigned_ids_by_their_v1_id() {
    // Ruling R8: a fresh run assigns no ids, so no refusal names a random new id.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let l = id_of(env.json(&forge, &["add", "Lane L", "--lane", "--origin", "user"]));
    let outer = id_of(env.json(&forge2, &["add", "Outer", "--lane", "--origin", "user"]));
    let (text, _) = draft_home(&env, &forge);
    let out = explain_text(
        &env,
        &forge,
        &decide(&text, &l, "carry", &format!("parent = \"{outer}\"\n")),
    );
    let settle = &setting(&out, "destination_invalid")[0].1;
    assert!(settle.contains(&format!("new:{l}")), "{settle}");
    let rest = settle.replace(&outer, "").replace(&format!("new:{l}"), "");
    assert!(!rest.contains("forge2-"), "{settle}");
}

// ---- Apply (plan Task 5): journal, phases G2-G10, resume, commits, freeze ----

const PHASES: [&str; 8] = [
    "journal",
    "attachments",
    "destination",
    "store",
    "tombstones",
    "closures",
    "cleanup",
    "commit",
];

fn migration_journal(env: &TestEnv, prefix: &str) -> std::path::PathBuf {
    env.home
        .path()
        .join(format!(".local/state/tasks/migrate/{prefix}.toml"))
}

fn list_sha(path: &std::path::Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn shown(env: &TestEnv, dir: &std::path::Path, id: &str) -> serde_json::Value {
    env.json(dir, &["show", id])["task"].clone()
}

fn note_texts(task: &serde_json::Value) -> Vec<String> {
    task["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["text"].as_str().unwrap().to_string())
        .collect()
}

fn new_id(out: &serde_json::Value, from: &str) -> String {
    carried_row(out, from)["id"]
        .as_str()
        .unwrap_or_else(|| panic!("{from} has no new id in {out}"))
        .to_string()
}

/// Runs `migrate <list>` stopping after `phase`, asserting success.
fn apply_until(env: &TestEnv, dir: &std::path::Path, list: &std::path::Path, phase: &str) {
    env.cmd(dir)
        .env("TASKS_MIGRATE_STOP_AFTER", phase)
        .args(["migrate", list.to_str().unwrap()])
        .assert()
        .success();
}

fn assert_clean(env: &TestEnv, dir: &std::path::Path) {
    let report = env.check(dir);
    assert_eq!(report["errors"], serde_json::json!([]), "{report}");
    assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
}

struct Mixed {
    forge: std::path::PathBuf,
    forge2: std::path::PathBuf,
    g: String,
    a: String,
    b: String,
    c: String,
    s: String,
    r: String,
    d: String,
    list: std::path::PathBuf,
}

/// Spec §5 case 1, plus a done recurrence R carried and a task D carried with a
/// replacement body: G dropped, A carried with no parent, B carried under successor goal
/// S, C closed; A depends on B.
fn mixed(env: &mut TestEnv) -> Mixed {
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let b = id_of(env.json(&forge, &["add", "B", "--parent", &g]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &g, "--depends", &b]));
    let c = id_of(env.json(&forge, &["add", "C", "--parent", &g]));
    let r = id_of(env.json(&forge, &["add", "R", "--every", "7d"]));
    env.json(&forge, &["done", &r]);
    let d = id_of(env.json(&forge, &["add", "D", "--body", "Old body."]));
    let s = id_of(env.json(&forge2, &["add", "Goal S", "-p", "1"]));
    let (text, _) = draft_home(env, &forge);
    let mut text = decide(&text, &g, "drop", "reason = \"superseded by forge2\"\n");
    text = decide(&text, &a, "carry", "parent = \"none\"\n");
    text = decide(&text, &b, "carry", &format!("parent = \"{s}\"\n"));
    text = decide(&text, &c, "close", "reason = \"done in forge\"\n");
    text = decide(&text, &r, "carry", "");
    text = decide(
        &text,
        &d,
        "carry",
        "body = \"\"\"\nReplacement body é.\n\"\"\"\n",
    );
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    Mixed {
        forge,
        forge2,
        g,
        a,
        b,
        c,
        s,
        r,
        d,
        list,
    }
}

#[test]
fn mixed_list_applies_end_to_end_and_retire_follows() {
    // Spec §5 case 1, end to end.
    let mut env = TestEnv::new();
    let m = mixed(&mut env);
    let sha = list_sha(&m.list);
    let suffix = format!(" list: sha256:{sha}");
    let r_before = shown(&env, &m.forge, &m.r);
    let out = env.json(&m.forge, &["migrate", m.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "fresh", "{out}");
    assert_eq!(out["list"], format!("sha256:{sha}"));
    assert_eq!(out["source"], "forge");
    assert_eq!(out["successor"], "forge2");
    assert!(out.get("refusals").is_none(), "{out}");
    let a2 = new_id(&out, &m.a);
    let b2 = new_id(&out, &m.b);
    let r2 = new_id(&out, &m.r);
    let d2 = new_id(&out, &m.d);
    assert_eq!(carried_row(&out, &m.b)["parent"], m.s.as_str());
    assert_eq!(carried_row(&out, &m.a)["parent"], serde_json::Value::Null);
    assert_eq!(row(&out, "dropped", &m.g)["reason"], "superseded by forge2");
    assert_eq!(row(&out, "closed", &m.c)["reason"], "done in forge");
    assert!(!migration_journal(&env, "forge").exists());

    let a_new = shown(&env, &m.forge2, &a2);
    assert_eq!(a_new["parent"], serde_json::Value::Null, "{a_new}");
    assert_eq!(a_new["depends"], serde_json::json!([b2]), "{a_new}");
    assert_eq!(a_new["moved_from"], m.a.as_str());
    let b_new = shown(&env, &m.forge2, &b2);
    assert_eq!(b_new["parent"], m.s.as_str(), "{b_new}");
    for (old, new) in [(&m.a, &a2), (&m.b, &b2), (&m.r, &r2), (&m.d, &d2)] {
        let tomb = shown(&env, &m.forge, old);
        assert_eq!(tomb["status"], "moved", "{tomb}");
        assert_eq!(tomb["moved_to"], new.as_str(), "{tomb}");
    }
    let g = shown(&env, &m.forge, &m.g);
    assert_eq!(g["status"], "dropped");
    assert_eq!(
        note_texts(&g).last().unwrap(),
        &format!("dropped in migration to forge2: superseded by forge2{suffix}")
    );
    let c = shown(&env, &m.forge, &m.c);
    assert_eq!(c["status"], "done");
    assert_eq!(
        note_texts(&c).last().unwrap(),
        &format!("closed in migration to forge2: done in forge{suffix}")
    );
    // Every migration note, on both sides, carries the list's sha256.
    let mut seen = 0;
    for (dir, id) in [
        (&m.forge2, &a2),
        (&m.forge2, &b2),
        (&m.forge2, &r2),
        (&m.forge2, &d2),
        (&m.forge, &m.a),
        (&m.forge, &m.b),
        (&m.forge, &m.r),
        (&m.forge, &m.d),
    ] {
        for text in note_texts(&shown(&env, dir, id)) {
            if text.starts_with("moved from")
                || text.starts_with("moved to")
                || text.starts_with("migration:")
            {
                assert!(text.ends_with(&suffix), "{id}: {text}");
                seen += 1;
            }
        }
    }
    assert!(seen >= 10, "{seen}");
    // A carried done recurrence keeps its status, cadence and anchor.
    let r_new = shown(&env, &m.forge2, &r2);
    assert_eq!(r_new["status"], "done", "{r_new}");
    assert_eq!(r_new["every"], r_before["every"]);
    assert_eq!(r_new["last_done"], r_before["last_done"]);
    assert!(r_new["last_done"].is_string(), "{r_new}");
    // Review Focus 4 end to end: the replacement body reads back exactly.
    assert_eq!(shown(&env, &m.forge2, &d2)["body"], "Replacement body é.");
    assert_clean(&env, &m.forge);
    assert_clean(&env, &m.forge2);
    env.json(
        &m.forge,
        &["retire", "--successor", "forge2", "--reason", "migrated"],
    );
}

#[test]
fn parks_travel_or_go() {
    // Spec §5 case 10, park half.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let d = id_of(env.json(&forge, &["add", "D"]));
    env.json(&forge, &["park", &a, "resume the parser"]);
    env.json(&forge, &["park", &d, "never mind"]);
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &a, "carry", "");
    let text = decide(&text, &d, "drop", "reason = \"obsolete\"\n");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let out = env.json(&forge, &["migrate", list.to_str().unwrap()]);
    let a2 = new_id(&out, &a);
    assert_eq!(carried_row(&out, &a)["park"], true);
    let parked = env.json(&forge2, &["list", "--parked"]);
    assert!(parked.to_string().contains(&a2), "{parked}");
    let store: toml::Value =
        toml::from_str(&std::fs::read_to_string(env.claim_store("forge2")).unwrap()).unwrap();
    assert_eq!(
        store["parks"][a2.as_str()]["worktree"].as_str(),
        Some(forge2.to_str().unwrap())
    );
    assert_eq!(
        store["parks"][a2.as_str()]["next_step"].as_str(),
        Some("resume the parser")
    );
    let source = std::fs::read_to_string(env.claim_store("forge")).unwrap_or_default();
    assert!(!source.contains(&d) && !source.contains(&a), "{source}");
}

/// A carried task with an attachment and a park, a dropped task and a closed one.
struct Phased {
    forge: std::path::PathBuf,
    forge2: std::path::PathBuf,
    a: String,
    d: String,
    c: String,
    list: std::path::PathBuf,
}

fn phased(env: &mut TestEnv) -> Phased {
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    phased_in(env, forge, forge2)
}

fn phased_in(env: &mut TestEnv, forge: std::path::PathBuf, forge2: std::path::PathBuf) -> Phased {
    let a = id_of(env.json(&forge, &["add", "A"]));
    let shot = env.home.path().join("image.png");
    std::fs::write(&shot, b"bytes").unwrap();
    env.json(&forge, &["attach", &a, shot.to_str().unwrap()]);
    env.json(&forge, &["park", &a, "continue"]);
    let d = id_of(env.json(&forge, &["add", "D"]));
    let c = id_of(env.json(&forge, &["add", "C"]));
    let (text, _) = draft_home(env, &forge);
    let text = decide(&text, &a, "carry", "");
    let text = decide(&text, &d, "drop", "reason = \"obsolete\"\n");
    let text = decide(&text, &c, "close", "reason = \"finished\"\n");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    Phased {
        forge,
        forge2,
        a,
        d,
        c,
        list,
    }
}

#[test]
fn every_phase_resumes_to_the_same_ids() {
    // Spec §5 case 11, journal path.
    for phase in PHASES {
        let mut env = TestEnv::new();
        let p = phased(&mut env);
        let l = p.list.to_str().unwrap();
        apply_until(&env, &p.forge, &p.list, phase);
        let journal = migration_journal(&env, "forge");
        assert!(journal.exists(), "{phase}");
        let saved = std::fs::read(&journal).unwrap();
        let preview = env.json(&p.forge, &["migrate", l, "--explain"]);
        assert_eq!(preview["recovery"], "resume", "{phase}: {preview}");
        let planned = new_id(&preview, &p.a);
        assert_eq!(std::fs::read(&journal).unwrap(), saved, "{phase}");
        for (dir, id) in [
            (&p.forge, &p.a),
            (&p.forge, &p.d),
            (&p.forge, &p.c),
            (&p.forge2, &planned),
        ] {
            assert_eq!(
                fail_detail(&env, dir, &["note", id, "x"]).0,
                "move_pending",
                "{phase}: note {id}"
            );
        }
        assert_eq!(
            fail_detail(
                &env,
                &p.forge,
                &["retire", "--successor", "forge2", "--reason", "x"]
            )
            .0,
            "move_pending",
            "{phase}"
        );
        if phase == "journal" {
            // A pending run continues only with the list it started with.
            let original = std::fs::read(&p.list).unwrap();
            std::fs::write(&p.list, [original.as_slice(), b"\n# edited\n"].concat()).unwrap();
            let (kind, detail) = fail_detail(&env, &p.forge, &["migrate", l]);
            assert_eq!(kind, "move_pending", "{detail}");
            assert!(detail.contains("list"), "{detail}");
            std::fs::write(&p.list, &original).unwrap();
            // A hand edit to a listed record mid-run refuses.
            let path = p.forge.join(format!("tasks/{}.md", p.d));
            let record = std::fs::read(&path).unwrap();
            std::fs::write(&path, [record.as_slice(), b"\nhand edit\n"].concat()).unwrap();
            let (kind, detail) = fail_detail(&env, &p.forge, &["migrate", l]);
            assert_eq!(kind, "move_pending", "{detail}");
            assert!(detail.contains(&p.d), "{detail}");
            std::fs::write(&path, &record).unwrap();
            assert_eq!(std::fs::read(&journal).unwrap(), saved);
        }
        let out = env.json(&p.forge, &["migrate", l]);
        assert_eq!(out["recovery"], "resumed", "{phase}: {out}");
        let a2 = new_id(&out, &p.a);
        assert_eq!(a2, planned, "{phase}");
        assert!(!journal.exists(), "{phase}");
        assert_eq!(
            std::fs::read(p.forge2.join(format!("tasks/files/{a2}/image.png"))).unwrap(),
            b"bytes"
        );
        assert!(!p.forge.join(format!("tasks/files/{}", p.a)).exists());
        assert_eq!(shown(&env, &p.forge, &p.d)["status"], "dropped");
        assert_eq!(shown(&env, &p.forge, &p.c)["status"], "done");
        assert!(
            env.json(&p.forge2, &["list", "--parked"])
                .to_string()
                .contains(&a2),
            "{phase}"
        );
        assert_clean(&env, &p.forge);
        assert_clean(&env, &p.forge2);
    }
}

fn git_out(dir: &std::path::Path, args: &[&str]) -> String {
    let home = tempfile::tempdir().unwrap();
    let out = common::git_fixture::command(home.path(), dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    if args.first() == Some(&"init") {
        common::git_fixture::assert_repo(home.path(), dir);
    }
    String::from_utf8(out.stdout).unwrap()
}

/// A project at the top of its own repository with `[git] commit = true`, committed.
fn committed_project(env: &mut TestEnv, prefix: &str) -> std::path::PathBuf {
    let root = env.init(prefix);
    git_out(&root, &["init", "-q", "-b", "main"]);
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    git_out(&root, &["add", "-A"]);
    git_out(&root, &["commit", "-qm", "init"]);
    root
}

fn commits(dir: &std::path::Path) -> usize {
    git_out(dir, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap()
}

fn head_files(dir: &std::path::Path) -> Vec<String> {
    git_out(dir, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(String::from)
        .collect()
}

fn porcelain(dir: &std::path::Path) -> String {
    git_out(dir, &["status", "--porcelain"])
}

#[test]
fn commit_phase_resumes_from_the_journal_paths() {
    let mut env = TestEnv::new();
    let forge = committed_project(&mut env, "forge");
    let forge2 = committed_project(&mut env, "forge2");
    let p = phased_in(&mut env, forge.clone(), forge2.clone());
    assert_eq!(porcelain(&forge), "");
    let (before, before2) = (commits(&forge), commits(&forge2));
    apply_until(&env, &forge, &p.list, "cleanup");
    assert_eq!(commits(&forge), before, "nothing commits before G9");
    assert_eq!(commits(&forge2), before2);
    let out = env.json(&forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    let a2 = new_id(&out, &p.a);
    assert_eq!(commits(&forge), before + 1);
    assert_eq!(commits(&forge2), before2 + 1);
    let mut files = head_files(&forge);
    files.sort();
    let mut want = vec![
        format!("tasks/{}.md", p.a),
        format!("tasks/{}.md", p.c),
        format!("tasks/{}.md", p.d),
        format!("tasks/files/{}/image.png", p.a),
    ];
    want.sort();
    assert_eq!(files, want);
    let mut files2 = head_files(&forge2);
    files2.sort();
    assert_eq!(
        files2,
        vec![
            format!("tasks/files/{a2}/image.png"),
            format!("tasks/{a2}.md"),
        ]
    );
    assert_eq!(porcelain(&forge), "");
    assert_eq!(porcelain(&forge2), "");
    assert!(!migration_journal(&env, "forge").exists());
}

#[test]
fn a_failing_pre_commit_hook_keeps_the_journal() {
    let mut env = TestEnv::new();
    let forge = committed_project(&mut env, "forge");
    let forge2 = committed_project(&mut env, "forge2");
    let p = phased_in(&mut env, forge.clone(), forge2.clone());
    let hook = forge.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let before = commits(&forge);
    let (kind, detail) = fail_detail(&env, &forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains(&format!("tasks/{}.md", p.d)), "{detail}");
    assert!(detail.contains("commit_failed"), "{detail}");
    assert!(migration_journal(&env, "forge").exists());
    assert_eq!(commits(&forge), before);
    std::fs::remove_file(&hook).unwrap();
    let out = env.json(&forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    assert_eq!(commits(&forge), before + 1);
    assert_eq!(porcelain(&forge), "");
    assert_eq!(porcelain(&forge2), "");
    assert!(!migration_journal(&env, "forge").exists());
}

#[test]
fn shared_records_repository_gets_one_commit() {
    // Plan Review Focus 1: detached forge and forge2 whose records live in one repository.
    let mut env = TestEnv::new();
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let repo = |name: &str| {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        git_out(&dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("README"), "x\n").unwrap();
        git_out(&dir, &["add", "README"]);
        git_out(&dir, &["commit", "-qm", "init"]);
        dir
    };
    let records = repo("records");
    let code = repo("forge-code");
    let code2 = repo("forge2-code");
    for (dir, prefix) in [(&code, "forge"), (&code2, "forge2")] {
        let out = env.json(
            dir,
            &[
                "init",
                "--prefix",
                prefix,
                "--records",
                records.to_str().unwrap(),
            ],
        );
        assert_eq!(
            out["records"],
            records.join(prefix).display().to_string(),
            "{out}"
        );
    }
    git_out(&records, &["add", "-A"]);
    git_out(&records, &["commit", "-qm", "projects"]);
    let p = phased_in(&mut env, code.clone(), code2.clone());
    assert_eq!(porcelain(&records), "");
    let before = commits(&records);
    let out = env.json(&code, &["migrate", p.list.to_str().unwrap()]);
    let a2 = new_id(&out, &p.a);
    assert_eq!(
        commits(&records),
        before + 1,
        "one commit for one repository"
    );
    let files = head_files(&records);
    for path in [
        format!("forge/tasks/{}.md", p.a),
        format!("forge/tasks/{}.md", p.d),
        format!("forge/tasks/{}.md", p.c),
        format!("forge2/tasks/{a2}.md"),
        format!("forge2/tasks/files/{a2}/image.png"),
    ] {
        assert!(files.contains(&path), "{path} not in {files:?}");
    }
    assert_eq!(porcelain(&records), "");
    assert!(!migration_journal(&env, "forge").exists());
}

#[test]
fn a_pending_migration_freezes_move_and_lifecycle() {
    let mut env = TestEnv::new();
    let p = phased(&mut env);
    let wali = env.init("wali");
    let w = id_of(env.json(&wali, &["add", "W"]));
    apply_until(&env, &p.forge, &p.list, "journal");
    for to in ["forge", "forge2"] {
        assert_eq!(
            fail_detail(&env, &wali, &["move", &w, "--to", to]).0,
            "move_pending",
            "move to {to}"
        );
    }
    let (kind, detail) = fail_detail(&env, &p.forge, &["rename", "forge", "fx"]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains("migrat"), "{detail}");
    assert_eq!(
        fail_detail(&env, &p.forge, &["unregister", "forge2"]).0,
        "move_pending"
    );
    assert_eq!(
        fail_detail(&env, &p.forge2, &["retire", "--reason", "x"]).0,
        "move_pending"
    );
    // Another migration naming either project refuses while this one is pending.
    let wl = env.home.path().join("wali-list.toml");
    env.json(
        &wali,
        &["migrate", "--to", "forge2", "--draft", wl.to_str().unwrap()],
    );
    std::fs::write(
        &wl,
        decide(
            &std::fs::read_to_string(&wl).unwrap(),
            &w,
            "drop",
            "reason = \"x\"\n",
        ),
    )
    .unwrap();
    assert_eq!(
        fail_detail(&env, &wali, &["migrate", wl.to_str().unwrap()]).0,
        "move_pending"
    );
    let out = env.json(&p.forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "resumed");
    env.json(&wali, &["move", &w, "--to", "forge2"]);
}

#[test]
fn a_pending_move_refuses_the_apply() {
    let mut env = TestEnv::new();
    let p = phased(&mut env);
    let wali = env.init("wali");
    let w = id_of(env.json(&wali, &["add", "W"]));
    env.cmd(&wali)
        .env("TASKS_MOVE_STOP_AFTER", "journal")
        .args(["move", &w, "--to", "forge"])
        .assert()
        .success();
    let (kind, detail) = fail_detail(&env, &p.forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(!migration_journal(&env, "forge").exists());
}

#[test]
fn explain_of_a_refused_pending_run_assigns_no_ids() {
    // Spec §4.7: new ids are filled only when the verdict is `resume`.
    let mut env = TestEnv::new();
    let p = phased(&mut env);
    let l = p.list.to_str().unwrap();
    apply_until(&env, &p.forge, &p.list, "journal");
    let resume = env.json(&p.forge, &["migrate", l, "--explain"]);
    assert_eq!(resume["recovery"], "resume");
    assert!(carried_row(&resume, &p.a)["id"].is_string(), "{resume}");
    let original = std::fs::read(&p.list).unwrap();
    std::fs::write(&p.list, [original.as_slice(), b"\n# edited\n"].concat()).unwrap();
    let refused = env.json(&p.forge, &["migrate", l, "--explain"]);
    assert_eq!(refused["recovery"], "refuse", "{refused}");
    assert!(carried_row(&refused, &p.a).get("id").is_none(), "{refused}");
    assert_eq!(setting(&refused, "pending_operation").len(), 1, "{refused}");
    assert!(setting(&refused, "pending_operation")[0].1.contains("list"));
    std::fs::write(&p.list, &original).unwrap();
    assert_eq!(env.json(&p.forge, &["migrate", l])["recovery"], "resumed");
}

// ---- Completion without a journal (plan Task 6, spec §4.5, §4.6, §5 case 13) ----

fn store_of(env: &TestEnv, prefix: &str) -> toml::Value {
    let path = env.claim_store(prefix);
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap(),
        Err(_) => toml::Value::Table(toml::Table::new()),
    }
}

fn write_store(env: &TestEnv, prefix: &str, store: &toml::Value) {
    std::fs::write(env.claim_store(prefix), toml::to_string(store).unwrap()).unwrap();
}

/// Removes `id`'s entry from the `kind` table (`parks` or `escalations`) of `prefix`'s store.
fn remove_entry(env: &TestEnv, prefix: &str, kind: &str, id: &str) {
    let mut store = store_of(env, prefix);
    let table = store[kind].as_table_mut().unwrap();
    assert!(
        table.remove(id).is_some(),
        "{prefix} has no {kind} entry {id}"
    );
    write_store(env, prefix, &store);
}

fn add_escalation(env: &TestEnv, prefix: &str, id: &str) {
    let mut store = store_of(env, prefix);
    let table = store.as_table_mut().unwrap();
    let escalations = table
        .entry("escalations")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    escalations.as_table_mut().unwrap().insert(
        id.into(),
        toml::Value::Table(toml::Table::from_iter([
            ("level".into(), toml::Value::String("high".into())),
            (
                "at".into(),
                toml::Value::String("2026-10-09T00:00:00Z".into()),
            ),
            ("session".into(), toml::Value::String("codex:test".into())),
        ])),
    );
    write_store(env, prefix, &store);
}

/// Everything a rerun could write: both trees and both claim stores.
fn state(
    env: &TestEnv,
    dirs: &[&std::path::Path],
) -> Vec<std::collections::BTreeMap<String, Vec<u8>>> {
    let mut out: Vec<_> = dirs.iter().map(|dir| snapshot(dir)).collect();
    for prefix in ["forge", "forge2"] {
        let mut stores = std::collections::BTreeMap::new();
        if let Ok(bytes) = std::fs::read(env.claim_store(prefix)) {
            stores.insert(prefix.to_string(), bytes);
        }
        out.push(stores);
    }
    out
}

/// A rerun with no journal reports `complete` and writes nothing; so does `--explain`.
fn assert_complete(
    env: &TestEnv,
    dir: &std::path::Path,
    list: &std::path::Path,
    dirs: &[&std::path::Path],
) -> serde_json::Value {
    assert!(!migration_journal(env, "forge").exists());
    let before = state(env, dirs);
    let preview = explain(env, dir, list);
    assert_eq!(preview["recovery"], "complete", "{preview}");
    let out = env.json(dir, &["migrate", list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "complete", "{out}");
    assert_eq!(out["list"], format!("sha256:{}", list_sha(list)));
    assert_eq!(state(env, dirs), before, "a complete rerun writes nothing");
    assert!(!migration_journal(env, "forge").exists());
    out
}

fn refused(env: &TestEnv, dir: &std::path::Path, list: &std::path::Path) -> String {
    let (kind, detail) = fail_detail(env, dir, &["migrate", list.to_str().unwrap()]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(!migration_journal(env, "forge").exists());
    detail
}

/// A list deciding every open task of `forge` the same way.
fn uniform_list(
    env: &TestEnv,
    forge: &std::path::Path,
    ids: &[&str],
    decision: &str,
) -> std::path::PathBuf {
    let (mut text, _) = draft_home(env, forge);
    for id in ids {
        text = decide(&text, id, decision, "reason = \"finished elsewhere\"\n");
    }
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    list
}

#[test]
fn reruns_after_success_report_complete() {
    // Spec §5 case 13, first sentences; and case 11's "another rerun reports complete".
    let mut env = TestEnv::new();
    let m = mixed(&mut env);
    let digests_before = std::fs::read_to_string(&m.list).unwrap();
    let out = env.json(&m.forge, &["migrate", m.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "fresh");
    let again = assert_complete(&env, &m.forge, &m.list, &[&m.forge, &m.forge2]);
    // The new ids are read back from the tombstones.
    for from in [&m.a, &m.b, &m.r, &m.d] {
        assert_eq!(new_id(&again, from), new_id(&out, from), "{again}");
    }
    assert_eq!(carried_row(&again, &m.b)["parent"], m.s.as_str());
    assert_eq!(again["dropped"][0]["id"], m.g.as_str(), "{again}");
    assert_eq!(again["dropped"][0]["reason"], "superseded by forge2");
    assert_eq!(again["closed"][0]["id"], m.c.as_str(), "{again}");
    assert_eq!(again["closed"][0]["reason"], "done in forge");
    // Every listed digest changed, yet the rerun did not refuse as stale.
    for id in [&m.g, &m.a, &m.b, &m.c] {
        let raw = std::fs::read(m.forge.join(format!("tasks/{id}.md"))).unwrap();
        let now = format!("sha256:{:x}", Sha256::digest(&raw));
        assert!(!digests_before.contains(&now), "{id}");
    }

    for decision in ["drop", "close"] {
        let mut env = TestEnv::new();
        let forge = env.init("forge");
        let forge2 = env.init("forge2");
        let x = id_of(env.json(&forge, &["add", "X"]));
        let y = id_of(env.json(&forge, &["add", "Y"]));
        let list = uniform_list(&env, &forge, &[&x, &y], decision);
        let out = env.json(&forge, &["migrate", list.to_str().unwrap()]);
        assert_eq!(out["recovery"], "fresh", "{decision}: {out}");
        let again = assert_complete(&env, &forge, &list, &[&forge, &forge2]);
        let key = if decision == "drop" {
            "dropped"
        } else {
            "closed"
        };
        let rows: Vec<&str> = again[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect();
        let mut want = vec![x.as_str(), y.as_str()];
        want.sort();
        let mut rows = rows;
        rows.sort();
        assert_eq!(rows, want, "{decision}: {again}");
        assert_eq!(again["carried"], serde_json::json!([]), "{again}");
    }
}

/// An all-carry list: A with an attachment and a park, in two committed projects.
struct CarryOne {
    forge: std::path::PathBuf,
    forge2: std::path::PathBuf,
    a: String,
    list: std::path::PathBuf,
}

fn carry_one(env: &mut TestEnv) -> CarryOne {
    let forge = committed_project(env, "forge");
    let forge2 = committed_project(env, "forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    let shot = env.home.path().join("image.png");
    std::fs::write(&shot, b"bytes").unwrap();
    env.json(&forge, &["attach", &a, shot.to_str().unwrap()]);
    env.json(&forge, &["park", &a, "continue"]);
    let list = uniform_list(env, &forge, &[], "carry");
    let text = decide(&std::fs::read_to_string(&list).unwrap(), &a, "carry", "");
    std::fs::write(&list, text).unwrap();
    CarryOne {
        forge,
        forge2,
        a,
        list,
    }
}

/// The new id, read from the tombstone the run wrote.
fn moved_to(forge: &std::path::Path, id: &str) -> String {
    let raw = std::fs::read_to_string(forge.join(format!("tasks/{id}.md"))).unwrap();
    raw.lines()
        .find_map(|line| line.strip_prefix("moved_to: "))
        .unwrap_or_else(|| panic!("{id} is not a tombstone:\n{raw}"))
        .trim()
        .to_string()
}

fn commit_all(dir: &std::path::Path) {
    git_out(dir, &["add", "-A", "--", "tasks"]);
    git_out(dir, &["commit", "-qm", "finish the migration by hand"]);
}

#[test]
fn journal_lost_after_tombstones_refuses_until_finished_by_hand() {
    // Spec §5 case 13 with the manual finish of §4.6.
    let mut env = TestEnv::new();
    let c = carry_one(&mut env);
    apply_until(&env, &c.forge, &c.list, "tombstones");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let a2 = moved_to(&c.forge, &c.a);
    let source_file = c.forge.join(format!("tasks/files/{}/image.png", c.a));
    let before = state(&env, &[&c.forge, &c.forge2]);
    let detail = refused(&env, &c.forge, &c.list);
    assert_eq!(
        state(&env, &[&c.forge, &c.forge2]),
        before,
        "a refusal writes nothing"
    );
    assert!(
        detail.contains(&format!("tasks/files/{}/image.png", c.a)),
        "{detail}"
    );
    assert!(detail.contains(&format!("{} park", c.a)), "{detail}");
    assert!(
        detail.contains(&format!("tasks/{a2}.md")),
        "uncommitted: {detail}"
    );
    let preview = explain(&env, &c.forge, &c.list);
    assert_eq!(preview["recovery"], "refuse", "{preview}");

    // The manual finish: the attachment, verified against the destination copy.
    let copy = std::fs::read(c.forge2.join(format!("tasks/files/{a2}/image.png"))).unwrap();
    assert_eq!(std::fs::read(&source_file).unwrap(), copy);
    std::fs::remove_file(&source_file).unwrap();
    std::fs::remove_dir(source_file.parent().unwrap()).unwrap();
    // The park: the destination holds the expected entry, so the source entry goes.
    let source = store_of(&env, "forge");
    let mut expected = source["parks"][c.a.as_str()].clone();
    expected["worktree"] = toml::Value::String(c.forge2.display().to_string());
    assert_eq!(store_of(&env, "forge2")["parks"][a2.as_str()], expected);
    remove_entry(&env, "forge", "parks", &c.a);
    let detail = refused(&env, &c.forge, &c.list);
    assert!(!detail.contains("still in the source"), "{detail}");
    assert!(!detail.contains(&format!("{} park", c.a)), "{detail}");
    assert!(detail.contains(&format!("tasks/{}.md", c.a)), "{detail}");
    commit_all(&c.forge);
    commit_all(&c.forge2);
    let out = assert_complete(&env, &c.forge, &c.list, &[&c.forge, &c.forge2]);
    assert_eq!(new_id(&out, &c.a), a2);
    assert_clean(&env, &c.forge);
    assert_clean(&env, &c.forge2);

    // Lost after G8: only the commits remain, and they are named.
    let mut env = TestEnv::new();
    let c = carry_one(&mut env);
    apply_until(&env, &c.forge, &c.list, "cleanup");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let a2 = moved_to(&c.forge, &c.a);
    let detail = refused(&env, &c.forge, &c.list);
    for path in [
        c.forge.join(format!("tasks/{}.md", c.a)),
        c.forge.join(format!("tasks/files/{}/image.png", c.a)),
        c.forge2.join(format!("tasks/{a2}.md")),
        c.forge2.join(format!("tasks/files/{a2}/image.png")),
    ] {
        assert!(
            detail.contains(&path.display().to_string()),
            "{path:?}: {detail}"
        );
    }
    assert!(!detail.contains(&format!("{} park", c.a)), "{detail}");
    assert!(!detail.contains("still in the source"), "{detail}");
    commit_all(&c.forge);
    commit_all(&c.forge2);
    assert_complete(&env, &c.forge, &c.list, &[&c.forge, &c.forge2]);
}

#[test]
fn archived_history_stays_and_completes() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    env.json(&forge, &["note", &a, "archived sentinel"]);
    let (name, bytes) = common::archive_inline(&forge, &a, false);
    env.json(&forge, &["note", &a, "inline after the archive"]);
    let before = note_texts(&shown(&env, &forge, &a));
    let list = uniform_list(&env, &forge, &[], "carry");
    std::fs::write(
        &list,
        decide(&std::fs::read_to_string(&list).unwrap(), &a, "carry", ""),
    )
    .unwrap();
    let out = env.json(&forge, &["migrate", list.to_str().unwrap()]);
    let a2 = new_id(&out, &a);
    let segment = forge.join(format!("tasks/files/{a}/{name}"));
    assert_eq!(
        std::fs::read(&segment).unwrap(),
        bytes,
        "retained with its digest"
    );
    assert_eq!(
        std::fs::read(forge2.join(format!("tasks/files/{a2}/{name}"))).unwrap(),
        bytes
    );
    let tombstone = note_texts(&shown(&env, &forge, &a));
    assert_eq!(
        &tombstone[..before.len()],
        before.as_slice(),
        "history reads in full"
    );
    assert!(tombstone.contains(&"archived sentinel".to_string()));
    assert_complete(&env, &forge, &list, &[&forge, &forge2]);
    // A retained segment that goes missing is unfinished, never cleanup.
    std::fs::remove_file(&segment).unwrap();
    let detail = refused(&env, &forge, &list);
    assert!(detail.contains(&name), "{detail}");
    std::fs::write(&segment, &bytes).unwrap();
    assert_complete(&env, &forge, &list, &[&forge, &forge2]);
}

#[test]
fn conflicting_destination_park_is_kept() {
    // Park: resumed and parked again in the successor after the journal was lost.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    env.json(&forge, &["park", &a, "continue"]);
    let list = uniform_list(&env, &forge, &[], "carry");
    std::fs::write(
        &list,
        decide(&std::fs::read_to_string(&list).unwrap(), &a, "carry", ""),
    )
    .unwrap();
    apply_until(&env, &forge, &list, "tombstones");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let a2 = moved_to(&forge, &a);
    env.json(&forge2, &["start", &a2]);
    env.json(&forge2, &["park", &a2, "a newer next step"]);
    let newer = store_of(&env, "forge2")["parks"][a2.as_str()].clone();
    assert_eq!(newer["next_step"].as_str(), Some("a newer next step"));
    let detail = refused(&env, &forge, &list);
    assert!(detail.contains(&format!("{a} park")), "{detail}");
    // The manual finish stops at the conflict; the person removes the source entry.
    remove_entry(&env, "forge", "parks", &a);
    assert_complete(&env, &forge, &list, &[&forge, &forge2]);
    assert_eq!(store_of(&env, "forge2")["parks"][a2.as_str()], newer);

    // Escalation: its expected entry has no worktree; a differing one is kept the same way.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge, &["add", "A"]));
    env.json(&forge, &["park", &a, "continue"]);
    add_escalation(&env, "forge", &a);
    let list = uniform_list(&env, &forge, &[], "carry");
    std::fs::write(
        &list,
        decide(&std::fs::read_to_string(&list).unwrap(), &a, "carry", ""),
    )
    .unwrap();
    apply_until(&env, &forge, &list, "tombstones");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let a2 = moved_to(&forge, &a);
    let carried = store_of(&env, "forge2")["escalations"][a2.as_str()].clone();
    assert_eq!(carried, store_of(&env, "forge")["escalations"][a.as_str()]);
    assert!(carried.get("worktree").is_none(), "{carried}");
    let mut changed = store_of(&env, "forge2");
    changed["escalations"][a2.as_str()]["session"] = toml::Value::String("codex:newer".into());
    write_store(&env, "forge2", &changed);
    // The park matches its expected entry and is finished first.
    remove_entry(&env, "forge", "parks", &a);
    let detail = refused(&env, &forge, &list);
    assert!(detail.contains(&format!("{a} escalation")), "{detail}");
    assert!(!detail.contains(&format!("{a} park")), "{detail}");
    remove_entry(&env, "forge", "escalations", &a);
    assert_complete(&env, &forge, &list, &[&forge, &forge2]);
    assert_eq!(
        store_of(&env, "forge2")["escalations"][a2.as_str()]["session"].as_str(),
        Some("codex:newer")
    );
}

#[test]
fn all_drop_with_park_and_escalation_finishes_by_hand() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let x = id_of(env.json(&forge, &["add", "X"]));
    let y = id_of(env.json(&forge, &["add", "Y"]));
    env.json(&forge, &["park", &x, "continue"]);
    add_escalation(&env, "forge", &y);
    let list = uniform_list(&env, &forge, &[&x, &y], "drop");
    let store2 = std::fs::read(env.claim_store("forge2")).ok();
    apply_until(&env, &forge, &list, "closures");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let detail = refused(&env, &forge, &list);
    assert!(detail.contains(&format!("{x} park")), "{detail}");
    assert!(detail.contains(&format!("{y} escalation")), "{detail}");
    // The manual finish for dropped tasks: each record is dropped with this list's sha256.
    let suffix = format!(" list: sha256:{}", list_sha(&list));
    for id in [&x, &y] {
        let task = shown(&env, &forge, id);
        assert_eq!(task["status"], "dropped");
        assert!(note_texts(&task).last().unwrap().ends_with(&suffix));
    }
    remove_entry(&env, "forge", "parks", &x);
    remove_entry(&env, "forge", "escalations", &y);
    assert_complete(&env, &forge, &list, &[&forge, &forge2]);
    assert_eq!(std::fs::read(env.claim_store("forge2")).ok(), store2);
}

#[test]
fn uncommitted_paths_matter_only_where_commit_is_on() {
    // Plan Review Focus 5: forge opts in to record commits, forge2 does not.
    let forge2_repo = |env: &mut TestEnv| {
        let root = env.init("forge2");
        git_out(&root, &["init", "-q", "-b", "main"]);
        let config = root.join("tasks/.config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[git]\ncommit = false\n")).unwrap();
        git_out(&root, &["add", "-A"]);
        git_out(&root, &["commit", "-qm", "init"]);
        root
    };
    let setup = |env: &mut TestEnv| {
        let forge = committed_project(env, "forge");
        let forge2 = forge2_repo(env);
        phased_in(env, forge, forge2)
    };

    // A full run commits forge only, and its rerun is complete with forge2 still dirty.
    let mut env = TestEnv::new();
    let p = setup(&mut env);
    let before2 = commits(&p.forge2);
    let out = env.json(&p.forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "fresh", "{out}");
    assert_eq!(commits(&p.forge2), before2, "forge2 gets no commit");
    assert_eq!(porcelain(&p.forge), "");
    assert_ne!(porcelain(&p.forge2), "");
    assert_complete(&env, &p.forge, &p.list, &[&p.forge, &p.forge2]);

    // Lost after G8: only forge's paths are named; committing them completes the run.
    let mut env = TestEnv::new();
    let p = setup(&mut env);
    apply_until(&env, &p.forge, &p.list, "cleanup");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let a2 = moved_to(&p.forge, &p.a);
    let detail = refused(&env, &p.forge, &p.list);
    assert!(
        detail.contains(
            &p.forge
                .join(format!("tasks/{}.md", p.d))
                .display()
                .to_string()
        ),
        "{detail}"
    );
    assert!(!detail.contains(&a2), "{detail}");
    commit_all(&p.forge);
    assert_ne!(porcelain(&p.forge2), "");
    assert_complete(&env, &p.forge, &p.list, &[&p.forge, &p.forge2]);
}

#[test]
fn mixed_list_journal_lost_after_destination_refuses() {
    let mut env = TestEnv::new();
    let m = mixed(&mut env);
    apply_until(&env, &m.forge, &m.list, "destination");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let carriers: Vec<String> = std::fs::read_dir(m.forge2.join("tasks"))
        .unwrap()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let id = name.strip_suffix(".md")?.to_string();
            (id != m.s).then_some(id)
        })
        .collect();
    assert_eq!(carriers.len(), 4, "{carriers:?}");
    let before = state(&env, &[&m.forge, &m.forge2]);
    let detail = refused(&env, &m.forge, &m.list);
    assert_eq!(state(&env, &[&m.forge, &m.forge2]), before);
    let (carry, rest) = detail
        .split_once("not carrying")
        .unwrap_or_else(|| panic!("{detail}"));
    for id in &carriers {
        assert!(carry.contains(id.as_str()), "{id}: {detail}");
    }
    for id in [&m.g, &m.a, &m.b, &m.c, &m.r, &m.d] {
        assert!(rest.contains(id.as_str()), "{id}: {detail}");
    }
    assert_eq!(explain(&env, &m.forge, &m.list)["recovery"], "refuse");
}

#[test]
fn an_unreadable_successor_record_carrying_the_sha_is_never_fresh() {
    // Fix round 1: a corrupted new record is still evidence of a lost run.
    let mut env = TestEnv::new();
    let p = phased(&mut env);
    apply_until(&env, &p.forge, &p.list, "destination");
    std::fs::remove_file(migration_journal(&env, "forge")).unwrap();
    let records: Vec<_> = std::fs::read_dir(p.forge2.join("tasks"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    assert_eq!(records.len(), 1, "{records:?}");
    let a2 = records[0]
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let raw = std::fs::read_to_string(&records[0]).unwrap();
    std::fs::write(&records[0], raw.replacen("---\n", "", 1)).unwrap();
    let before = state(&env, &[&p.forge, &p.forge2]);
    let detail = refused(&env, &p.forge, &p.list);
    assert_eq!(state(&env, &[&p.forge, &p.forge2]), before);
    assert!(detail.contains(&format!("{a2} (unreadable")), "{detail}");
    assert_eq!(explain(&env, &p.forge, &p.list)["recovery"], "refuse");
}

// ---- The freeze, restoring writes and resume revalidation (plan Task 7, spec §4.5) ----

const PENDING: &str = "a migration of forge to forge2 is pending on this host";

fn refuses(env: &TestEnv, dir: &std::path::Path, args: &[&str]) {
    let (kind, detail) = fail_detail(env, dir, args);
    assert_eq!(kind, "move_pending", "{args:?}: {detail}");
}

/// Writes `record`'s file with `from` replaced by `to`, as another host's synced write
/// would arrive: no command runs.
fn hand_edit(dir: &std::path::Path, id: &str, from: &str, to: &str) {
    let path = dir.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains(from), "{id}: {raw}");
    std::fs::write(&path, raw.replacen(from, to, 1)).unwrap();
}

fn depends_of(env: &TestEnv, dir: &std::path::Path, id: &str) -> Vec<String> {
    shown(env, dir, id)["depends"]
        .as_array()
        .map(|deps| {
            deps.iter()
                .map(|d| d.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Forge D dropped with `release = [x]`, where ops-x depends on D; ops-y and ops-z are
/// plain; forge B carried under successor goal S. Stopped after `journal`.
struct Frozen {
    forge: std::path::PathBuf,
    forge2: std::path::PathBuf,
    ops: std::path::PathBuf,
    d: String,
    b: String,
    s: String,
    /// Another forge2 goal, not a parent target.
    p: String,
    x: String,
    y: String,
    z: String,
    list: std::path::PathBuf,
}

fn frozen(env: &mut TestEnv) -> Frozen {
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let ops = env.init("ops");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let b = id_of(env.json(&forge, &["add", "B"]));
    let s = id_of(env.json(&forge2, &["add", "Goal S"]));
    let p = id_of(env.json(&forge2, &["add", "Goal P"]));
    let x = id_of(env.json(&ops, &["add", "X", "--depends", &d]));
    let y = id_of(env.json(&ops, &["add", "Y"]));
    let z = id_of(env.json(&ops, &["add", "Z"]));
    let (text, _) = draft_home(env, &forge);
    let text = decide(
        &text,
        &d,
        "drop",
        &format!("reason = \"superseded\"\nrelease = [\"{x}\"]\n"),
    );
    let text = decide(&text, &b, "carry", &format!("parent = \"{s}\"\n"));
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    apply_until(env, &forge, &list, "journal");
    Frozen {
        forge,
        forge2,
        ops,
        d,
        b,
        s,
        p,
        x,
        y,
        z,
        list,
    }
}

#[test]
fn every_write_in_either_project_refuses_while_pending() {
    // Spec §5 case 11 (write refusals) and §4.5 freeze: the whole source and successor.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let f = id_of(env.json(&forge, &["add", "F"]));
    env.json(&forge, &["done", &f]);
    let t = id_of(env.json(&forge2, &["add", "T"]));
    let p = phased_in(&mut env, forge, forge2);
    apply_until(&env, &p.forge, &p.list, "journal");
    for (dir, id) in [(&p.forge, &f), (&p.forge2, &t)] {
        refuses(&env, dir, &["note", id, "x"]);
        refuses(&env, dir, &["edit", id, "--title", "renamed"]);
        refuses(&env, dir, &["add", "New"]);
        let show = env.json(dir, &["show", id]);
        assert!(show["warnings"].to_string().contains(PENDING), "{show}");
        let report = env.check(dir);
        assert!(
            report["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["kind"] == "move_pending"
                    && w["detail"].as_str().unwrap().contains(PENDING)),
            "{report}"
        );
        refuses(&env, dir, &["unretire"]);
    }
    refuses(&env, &p.forge2, &["start", &t]);
    let out = env.json(&p.forge, &["migrate", p.list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    env.json(&p.forge2, &["note", &t, "after the run"]);
    env.json(&p.forge, &["add", "After"]);
    assert_clean(&env, &p.forge2);
}

#[test]
fn edges_to_listed_ids_are_frozen_elsewhere() {
    // Spec §4.5 freeze, third projects: edges to listed ids move in neither direction.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let x = id_of(env.json(&ops, &["add", "X", "--depends", &d]));
    let y = id_of(env.json(&ops, &["add", "Y"]));
    let w = id_of(env.json(&ops, &["add", "W", "--depends", &d]));
    env.json(&ops, &["drop", &w]);
    let (text, _) = draft_home(&env, &forge);
    let text = decide(
        &text,
        &d,
        "drop",
        &format!("reason = \"superseded\"\nrelease = [\"{x}\"]\n"),
    );
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    apply_until(&env, &forge, &list, "journal");
    let before = snapshot(&ops);
    refuses(&env, &ops, &["dep", &y, "--on", &d]);
    refuses(&env, &ops, &["dep", &x, "--rm", &d]);
    refuses(&env, &ops, &["edit", &y, "--depends", &d]);
    refuses(&env, &ops, &["add", "New", "--depends", &d]);
    // Reopening a closed record that depends on D would make it a dependent.
    refuses(&env, &ops, &["edit", &w, "--status", "todo"]);
    assert_eq!(snapshot(&ops), before);
    // Writes that leave every edge to a listed id as it is go through.
    env.json(&ops, &["note", &x, "unrelated"]);
    env.json(&ops, &["dep", &y, "--on", &x]);
    env.json(&ops, &["add", "Other"]);
    let out = env.json(&forge, &["migrate", list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    env.json(&ops, &["dep", &y, "--on", &d]);
}

#[test]
fn hand_edits_are_repaired_one_command_at_a_time() {
    // Spec §5 case 12: other hosts' synced edits are caught by resume revalidation, and
    // each restoring command is admitted by the freeze, in either order.
    let mut env = TestEnv::new();
    let f = frozen(&mut env);
    let l = f.list.to_str().unwrap();
    refuses(&env, &f.forge2, &["edit", &f.s, "--every", "7d"]);
    refuses(&env, &f.ops, &["dep", &f.y, "--on", &f.d]);
    refuses(&env, &f.ops, &["dep", &f.x, "--rm", &f.d]);
    hand_edit(&f.ops, &f.y, "depends: []", &format!("depends: [{}]", f.d));
    hand_edit(&f.ops, &f.x, &format!("depends: [{}]", f.d), "depends: []");
    let rm_y = format!("`tasks dep {} --rm {}`", f.y, f.d);
    let on_x = format!("`tasks dep {} --on {}`", f.x, f.d);
    let (kind, detail) = fail_detail(&env, &f.forge, &["migrate", l]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains(&rm_y) && detail.contains(&on_x), "{detail}");
    let preview = explain(&env, &f.forge, &f.list);
    assert_eq!(preview["recovery"], "refuse", "{preview}");
    let settles: Vec<String> = setting(&preview, "release_changed")
        .into_iter()
        .map(|(_, settle)| settle)
        .collect();
    assert_eq!(settles.len(), 2, "{preview}");
    assert!(settles.iter().any(|s| s.contains(&rm_y)), "{preview}");
    assert!(settles.iter().any(|s| s.contains(&on_x)), "{preview}");
    refuses(&env, &f.ops, &["dep", &f.z, "--on", &f.d]);
    // Reversed order: the removed edge first.
    env.json(&f.ops, &["dep", &f.x, "--on", &f.d]);
    let (_, detail) = fail_detail(&env, &f.forge, &["migrate", l]);
    assert!(
        detail.contains(&rm_y) && !detail.contains(&on_x),
        "{detail}"
    );
    refuses(&env, &f.ops, &["dep", &f.z, "--on", &f.d]);
    // Not restoring: the edge is already as the journal expects.
    refuses(&env, &f.ops, &["dep", &f.x, "--rm", &f.d]);
    env.json(&f.ops, &["dep", &f.y, "--rm", &f.d]);
    refuses(&env, &f.ops, &["dep", &f.z, "--on", &f.d]);
    let out = env.json(&f.forge, &["migrate", l]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    assert_eq!(
        row(&out, "dropped", &f.d)["released"],
        serde_json::json!([f.x])
    );
    assert_eq!(depends_of(&env, &f.ops, &f.x), vec![f.d.clone()]);
    assert_eq!(depends_of(&env, &f.ops, &f.y), Vec::<String>::new());
    assert!(
        note_texts(&shown(&env, &f.forge, &f.d))
            .last()
            .unwrap()
            .contains(&format!("released: {}", f.x))
    );
    let b2 = new_id(&out, &f.b);
    assert_eq!(shown(&env, &f.forge2, &b2)["parent"], f.s.as_str());
}

#[test]
fn a_released_dependent_closed_elsewhere_is_reopened() {
    // A released dependent closed by another host is restored by reopening it.
    let mut env = TestEnv::new();
    let f = frozen(&mut env);
    let l = f.list.to_str().unwrap();
    hand_edit(&f.ops, &f.x, "status: todo", "status: dropped");
    let reopen = format!("`tasks edit {} --status todo`", f.x);
    let (_, detail) = fail_detail(&env, &f.forge, &["migrate", l]);
    assert!(detail.contains(&reopen), "{detail}");
    // Not the restoring write: closing it again, or dropping the edge.
    refuses(&env, &f.ops, &["dep", &f.x, "--rm", &f.d]);
    env.json(&f.ops, &["edit", &f.x, "--status", "todo"]);
    let out = env.json(&f.forge, &["migrate", l]);
    assert_eq!(out["recovery"], "resumed", "{out}");
}

#[test]
fn a_successor_dependent_is_repaired_inside_the_frozen_successor() {
    // A restoring write is judged before the project-wide refusal, so it is admitted in
    // the frozen successor; any other write there still refuses.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let t = id_of(env.json(&forge2, &["add", "T", "--depends", &d]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(
        &text,
        &d,
        "drop",
        &format!("reason = \"superseded\"\nrelease = [\"{t}\"]\n"),
    );
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let l = list.to_str().unwrap();
    apply_until(&env, &forge, &list, "journal");
    hand_edit(&forge2, &t, &format!("depends: [{d}]"), "depends: []");
    let restore = format!("`tasks dep {t} --on {d}`");
    let (kind, detail) = fail_detail(&env, &forge, &["migrate", l]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains(&restore), "{detail}");
    refuses(&env, &forge2, &["note", &t, "x"]);
    env.json(&forge2, &["dep", &t, "--on", &d]);
    refuses(&env, &forge2, &["note", &t, "x"]);
    let out = env.json(&forge, &["migrate", l]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    assert_eq!(row(&out, "dropped", &d)["released"], serde_json::json!([t]));
    assert_eq!(depends_of(&env, &forge2, &t), vec![d.clone()]);
    env.json(&forge2, &["note", &t, "x"]);
}

#[test]
fn a_parent_target_made_recurring_is_repaired() {
    // Spec §5 case 12, parent target half.
    let mut env = TestEnv::new();
    let f = frozen(&mut env);
    let l = f.list.to_str().unwrap();
    refuses(&env, &f.forge2, &["edit", &f.s, "--every", "7d"]);
    refuses(&env, &f.forge2, &["note", &f.s, "x"]);
    hand_edit(&f.forge2, &f.s, "\ndepends: ", "\nevery: 7d\ndepends: ");
    let restore = format!("`tasks edit {} --no-every`", f.s);
    let (kind, detail) = fail_detail(&env, &f.forge, &["migrate", l]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains(&restore), "{detail}");
    let preview = explain(&env, &f.forge, &f.list);
    assert_eq!(setting(&preview, "parent_target").len(), 1, "{preview}");
    // A write to S that leaves it recurring is not the restoring one.
    refuses(&env, &f.forge2, &["edit", &f.s, "--title", "Goal S2"]);
    // Nor is one that fixes the cadence and changes another field the parent check reads.
    refuses(
        &env,
        &f.forge2,
        &["edit", &f.s, "--no-every", "--parent", &f.p],
    );
    refuses(&env, &f.forge2, &["edit", &f.s, "--no-every", "--lane"]);
    refuses(
        &env,
        &f.forge2,
        &["edit", &f.s, "--no-every", "--status", "doing"],
    );
    refuses(
        &env,
        &f.forge2,
        &["edit", &f.s, "--no-every", "--title", "x"],
    );
    env.json(&f.forge2, &["edit", &f.s, "--no-every"]);
    refuses(&env, &f.forge2, &["note", &f.s, "x"]);
    let out = env.json(&f.forge, &["migrate", l]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    let b2 = new_id(&out, &f.b);
    assert_eq!(shown(&env, &f.forge2, &b2)["parent"], f.s.as_str());
    assert_clean(&env, &f.forge2);
}

#[test]
fn a_lane_made_above_a_parent_target_is_cleared_on_that_ancestor() {
    // Fix round 1: a carried lane under S, and an ancestor of S made a lane by another
    // host. The restore clears the ancestor's lane; moving S is not admitted.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let a = id_of(env.json(&forge2, &["add", "Goal A"]));
    let s = id_of(env.json(&forge2, &["add", "Goal S", "--parent", &a]));
    let l = id_of(env.json(&forge, &["add", "Lane L", "--lane", "--origin", "user"]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(&text, &l, "carry", &format!("parent = \"{s}\"\n"));
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let l_arg = list.to_str().unwrap();
    apply_until(&env, &forge, &list, "journal");
    refuses(&env, &forge2, &["edit", &a, "--lane", "--origin", "user"]);
    hand_edit(&forge2, &a, "\ndepends: ", "\nlane: true\ndepends: ");
    let restore = format!("`tasks edit {a} --no-lane`");
    let (kind, detail) = fail_detail(&env, &forge, &["migrate", l_arg]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(detail.contains(&restore), "{detail}");
    assert!(!detail.contains("--no-parent"), "{detail}");
    refuses(&env, &forge2, &["edit", &s, "--no-parent"]);
    refuses(&env, &forge2, &["edit", &a, "--no-lane", "--title", "x"]);
    env.json(&forge2, &["edit", &a, "--no-lane"]);
    let out = env.json(&forge, &["migrate", l_arg]);
    assert_eq!(out["recovery"], "resumed", "{out}");
    let l2 = new_id(&out, &l);
    assert_eq!(shown(&env, &forge2, &l2)["parent"], s.as_str());
    assert_eq!(shown(&env, &forge2, &s)["parent"], a.as_str());
}

#[test]
fn a_reopen_that_would_reactivate_another_listed_edge_is_not_named() {
    // Fix round 1: reopening a closed released dependent that also depends on a carried
    // task would reactivate that edge too, which the freeze refuses; the resume refusal
    // must not name it.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    let ops = env.init("ops");
    let d = id_of(env.json(&forge, &["add", "D"]));
    let c = id_of(env.json(&forge, &["add", "C"]));
    let x = id_of(env.json(&ops, &["add", "X", "--depends", &d, "--depends", &c]));
    let (text, _) = draft_home(&env, &forge);
    let text = decide(
        &text,
        &d,
        "drop",
        &format!("reason = \"superseded\"\nrelease = [\"{x}\"]\n"),
    );
    let text = decide(&text, &c, "carry", "");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let l = list.to_str().unwrap();
    apply_until(&env, &forge, &list, "journal");
    hand_edit(&ops, &x, "status: todo", "status: dropped");
    let reopen = format!("`tasks edit {x} --status todo`");
    let (kind, detail) = fail_detail(&env, &forge, &["migrate", l]);
    assert_eq!(kind, "move_pending", "{detail}");
    assert!(!detail.contains(&reopen), "{detail}");
    assert!(
        detail.contains("no single command") && detail.contains(&c),
        "{detail}"
    );
    refuses(&env, &ops, &["edit", &x, "--status", "todo"]);
}

/// The review the user reads: `migrate <list> --explain --pretty --color never`.
fn pretty_review(env: &TestEnv, dir: &std::path::Path, list: &std::path::Path) -> String {
    let out = env
        .cmd(dir)
        .args([
            "--pretty",
            "--color",
            "never",
            "migrate",
            list.to_str().unwrap(),
            "--explain",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn pretty_review_shows_sections_in_order() {
    // Spec §4.8: refusals, the successor tree, drops, closes, removed edges and dropped
    // links, then body diffs.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let ops = env.init("ops");
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let a = id_of(env.json(&forge, &["add", "A", "--parent", &g]));
    let k = id_of(env.json(&forge, &["add", "K"]));
    let x = id_of(env.json(&forge, &["add", "X", "--depends", &k]));
    let c = id_of(env.json(&forge, &["add", "C"]));
    let u = id_of(env.json(&forge, &["add", "U needs rm_depends", "--depends", &k]));
    let o = id_of(env.json(&ops, &["add", "O", "--depends", &k]));
    let s = id_of(env.json(&forge2, &["add", "Goal S", "-p", "1"]));
    let (text, _) = draft_home(&env, &forge);
    let mut text = decide(&text, &g, "carry", "");
    text = decide(&text, &a, "carry", "");
    text = decide(
        &text,
        &k,
        "drop",
        &format!("reason = \"gone\"\nrelease = [\"{o}\"]\n"),
    );
    text = decide(
        &text,
        &x,
        "carry",
        &format!("parent = \"{s}\"\nrm_depends = [\"{k}\"]\nbody = \"\"\"\nNew words.\n\"\"\"\n"),
    );
    text = decide(&text, &c, "close", "reason = \"done in forge\"\n");
    text = decide(&text, &u, "carry", "");
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let shown = pretty_review(&env, &forge, &list);
    let at = |needle: &str| {
        shown
            .find(needle)
            .unwrap_or_else(|| panic!("no {needle:?} in:\n{shown}"))
    };
    let refusals = at("refusals:");
    let tree = at("carried:");
    let dropped = at("dropped:");
    let closed = at("closed:");
    let removed = at("removed:");
    let body = at("body:");
    assert!(
        refusals < tree && tree < dropped && dropped < closed && closed < removed && removed < body,
        "{shown}"
    );
    // The tree nests A under G' and marks X's new parent; drops list released dependents.
    assert!(shown.contains(&format!("\n  {g}\n    {a}\n")), "{shown}");
    assert!(shown[tree..dropped].contains(&a), "{shown}");
    assert!(
        shown[tree..dropped].contains(&format!("under {s}")),
        "{shown}"
    );
    assert!(shown[dropped..closed].contains("gone"), "{shown}");
    assert!(
        shown[dropped..closed].contains(&format!("released: {o}")),
        "{shown}"
    );
    assert!(shown[closed..removed].contains("done in forge"), "{shown}");
    assert!(
        shown[removed..body].contains(&format!("removed dependency {k} (dropped)")),
        "{shown}"
    );
    let diff: Vec<&str> = shown[body..]
        .lines()
        .filter(|l| l.starts_with('+'))
        .collect();
    assert_eq!(diff, ["+New words."], "{shown}");
    // No refusals: the section is absent.
    let text = std::fs::read_to_string(&list).unwrap();
    let at_u = text.find(&format!("id = \"{u}\"")).unwrap();
    let line = at_u + text[at_u..].find("decision = \"carry\"\n").unwrap();
    let end = line + "decision = \"carry\"\n".len();
    let clean = format!(
        "{}decision = \"carry\"\nrm_depends = [\"{k}\"]\n{}",
        &text[..line],
        &text[end..]
    );
    std::fs::write(&list, clean).unwrap();
    let shown = pretty_review(&env, &forge, &list);
    assert!(!shown.contains("refusals:"), "{shown}");
}

#[test]
fn forge_shaped_fixture_migrates_in_one_list() {
    // Spec §5 case 16: a three-level tree, dependencies to a forge2 task, fleet-rollout
    // dependents in ops, a mix of carry, drop and close.
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    let forge2 = env.init("forge2");
    let ops = env.init("ops");
    let t = id_of(env.json(&forge2, &["add", "Successor task"]));
    let s = id_of(env.json(&forge2, &["add", "Goal S", "-p", "1"]));
    let g = id_of(env.json(&forge, &["add", "Goal G", "-p", "1"]));
    let m = id_of(env.json(&forge, &["add", "Milestone M", "--parent", &g]));
    let l1 = id_of(env.json(&forge, &["add", "Leaf 1", "--parent", &m]));
    let l2 = id_of(env.json(&forge, &["add", "Leaf 2", "--parent", &m, "--depends", &t]));
    let p = id_of(env.json(&forge, &["add", "Rollout prep"]));
    let q = id_of(env.json(&forge, &["add", "Finished elsewhere", "--parent", &g]));
    let h = id_of(env.json(&forge, &["add", "Harness", "--depends", &p]));
    let fleet: Vec<String> = ["fleet a", "fleet b"]
        .iter()
        .map(|title| id_of(env.json(&ops, &["add", title, "--depends", &p])))
        .collect();
    let (text, _) = draft_home(&env, &forge);
    let mut text = decide(&text, &g, "drop", "reason = \"replaced by forge2 goal\"\n");
    text = decide(&text, &m, "carry", &format!("parent = \"{s}\"\n"));
    text = decide(&text, &l1, "carry", "");
    text = decide(&text, &l2, "carry", "");
    text = decide(&text, &q, "close", "reason = \"done in forge\"\n");
    text = decide(
        &text,
        &p,
        "drop",
        &format!(
            "reason = \"rollout moved to forge2\"\nrelease = [\"{}\", \"{}\"]\n",
            fleet[0], fleet[1]
        ),
    );
    text = decide(
        &text,
        &h,
        "carry",
        &format!("parent = \"none\"\nrm_depends = [\"{p}\"]\n"),
    );
    let list = env.home.path().join("list.toml");
    std::fs::write(&list, text).unwrap();
    let out = explain(&env, &forge, &list);
    assert!(out.get("refusals").is_none(), "{out}");
    let out = env.json(&forge, &["migrate", list.to_str().unwrap()]);
    assert_eq!(out["recovery"], "fresh", "{out}");
    let l2_new = new_id(&out, &l2);
    let m_new = new_id(&out, &m);
    assert_eq!(shown(&env, &forge2, &l2_new)["parent"], m_new.as_str());
    assert_eq!(
        shown(&env, &forge2, &l2_new)["depends"],
        serde_json::json!([t])
    );
    assert_eq!(shown(&env, &forge2, &m_new)["parent"], s.as_str());
    for id in &fleet {
        // A dropped dependency no longer blocks; the edge itself is left alone.
        assert_eq!(shown(&env, &ops, id)["depends"], serde_json::json!([p]));
    }
    env.json(
        &forge,
        &["retire", "--successor", "forge2", "--reason", "migrated"],
    );
    for dir in [&forge, &forge2, &ops] {
        assert_clean(&env, dir);
    }
}
