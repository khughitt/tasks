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
fn migrate_list_takes_no_to_and_applying_is_refused_until_it_exists() {
    let mut env = TestEnv::new();
    let forge = env.init("forge");
    env.init("forge2");
    env.usage(
        &forge,
        &["migrate", "list.toml", "--to", "forge2", "--explain"],
    );
    let (_, _) = draft_home(&env, &forge);
    let list = env.home.path().join("list.toml");
    let (kind, detail) = fail_detail(&env, &forge, &["migrate", list.to_str().unwrap()]);
    assert_eq!(kind, "validation");
    assert!(detail.contains("--explain"), "{detail}");
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
