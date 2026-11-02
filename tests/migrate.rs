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
    let ids: Vec<String> = ["A", "B", "C", "D"]
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
