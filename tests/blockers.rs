//! `tasks blockers`: the ranked downstream-impact read
//! (docs/specs/2026-10-25-blocker-ranking-design.md). Every fixture runs under TestEnv's
//! isolated environment; git runs only through `common::git_fixture`.

mod common;
use common::{TestEnv, id_of};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn add(env: &TestEnv, dir: &Path, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["add", title];
    args.extend_from_slice(extra);
    id_of(env.json(dir, &args))
}

fn blockers(env: &TestEnv, dir: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["blockers"];
    args.extend_from_slice(extra);
    env.json(dir, &args)
}

fn row<'a>(out: &'a Value, id: &str) -> &'a Value {
    out["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row for {id}: {out}"))
}

fn has_row(out: &Value, id: &str) -> bool {
    out["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id)
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}

fn sorted(ids: &[&str]) -> Vec<String> {
    let mut ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    ids.sort();
    ids
}

fn warns(out: &Value, needle: &str) -> bool {
    out["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains(needle))
}

/// direct work, transitive work, direct goals, transitive goals, cleared, own open, own ?
fn counts(row: &Value) -> [u64; 7] {
    [
        "direct_work_count",
        "transitive_work_count",
        "direct_goal_count",
        "transitive_goal_count",
        "dependencies_cleared_direct_count",
        "unmet_dependency_count",
        "unresolved_dependency_count",
    ]
    .map(|key| {
        row[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{key} missing: {row}"))
    })
}

/// Every file under each root, by path, with its bytes.
fn snapshot(roots: &[&Path]) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending: Vec<PathBuf> = roots.iter().map(|root| root.to_path_buf()).collect();
    while let Some(path) = pending.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                pending.push(entry.unwrap().path());
            }
        } else {
            files.insert(path.clone(), std::fs::read(&path).unwrap());
        }
    }
    files
}

/// Commits `dir` as a git repository and adds a worktree of it at `at`, every git run
/// under the isolated fixture environment, checked against both boundaries first.
fn isolated_worktree(env: &TestEnv, dir: &Path, at: &Path) {
    let git = |args: &[&str]| {
        let out = common::git_fixture::command(env.home.path(), dir)
            .args(["-c", "init.defaultBranch=main"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    git(&["init", "-q"]);
    common::git_fixture::assert_repo(env.home.path(), dir);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "seed"]);
    git(&["worktree", "add", "-q", "-b", "wt", at.to_str().unwrap()]);
}

/// A claim written straight into the store: held by this test process (`live`), recorded
/// in a PID view no process here can judge (`unverifiable`), or long dead (`stale`).
fn write_claim(env: &TestEnv, prefix: &str, id: &str, session: &str, state: &str) {
    let path = env.claim_store(prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (pid, pid_start, boot) = if state == "stale" {
        (0, 1, "not-this-boot".to_string())
    } else {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let rest = stat.rsplit_once(") ").unwrap().1.to_string();
        let start: u64 = rest.split_whitespace().nth(19).unwrap().parse().unwrap();
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
        (std::process::id(), start, boot.trim().to_string())
    };
    let view = match state {
        "unverifiable" => "pid_view = \"0:1\"\n".to_string(),
        _ => own_pid_view()
            .map(|view| format!("pid_view = \"{view}\"\n"))
            .unwrap_or_default(),
    };
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!(
        "[claims.\"{id}\"]\nowner = \"someone\"\nsession = \"{session}\"\npid = {pid}\n\
         pid_start = {pid_start}\nboot_id = \"{boot}\"\nhost = \"h\"\n\
         worktree = \"/elsewhere\"\nstarted = \"2026-01-01T00:00:00Z\"\n\
         seen = \"2026-01-01T00:00:00Z\"\n{view}"
    ));
    std::fs::write(&path, text).unwrap();
}

/// This process's PID view, when it is the only namespace level (as tests/cli.rs).
fn own_pid_view() -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let nspid = status
        .lines()
        .find_map(|line| line.strip_prefix("NSpid:"))?;
    let mut entries = nspid.split_whitespace();
    let pid: u32 = entries.next()?.parse().ok()?;
    if pid == 0 || entries.next().is_some() {
        return None;
    }
    let ns = std::fs::metadata("/proc/self/ns/pid").ok()?;
    Some(format!("{}:{}", ns.dev(), ns.ino()))
}

#[test]
fn blockers_cli_surface_and_limits() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // An empty graph keeps every collection and an explicit zero.
    let out = blockers(&env, &sci, &[]);
    for (key, want) in [("total", 0), ("shown", 0), ("omitted", 0), ("limit", 20)] {
        assert_eq!(out[key], want, "{key}: {out}");
    }
    for key in ["blockers", "unresolved", "cycles"] {
        assert_eq!(out[key], serde_json::json!([]), "{key}: {out}");
    }
    assert_eq!(out["coverage"]["mode"], "local");
    assert_eq!(strings(&out["coverage"]["requested_projects"]), ["sci"]);
    assert_eq!(strings(&out["coverage"]["scanned_projects"]), ["sci"]);
    assert_eq!(out["coverage"]["complete"], true);

    for n in 0..22 {
        let blocker = add(&env, &sci, &format!("Blocker {n}"), &["-p", "2"]);
        add(
            &env,
            &sci,
            &format!("Consumer {n}"),
            &["--depends", &blocker],
        );
    }
    let out = blockers(&env, &sci, &[]);
    assert_eq!(
        (&out["total"], &out["shown"], &out["omitted"], &out["limit"]),
        (&22.into(), &20.into(), &2.into(), &20.into()),
        "{out}"
    );
    let ranks: Vec<u64> = out["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["rank"].as_u64().unwrap())
        .collect();
    assert_eq!(ranks, (1..=20).collect::<Vec<u64>>());

    // -n 0 keeps the totals, coverage and diagnostics; a limit past the total shows all.
    let none = blockers(&env, &sci, &["-n", "0"]);
    assert_eq!(none["blockers"], serde_json::json!([]));
    assert_eq!((&none["total"], &none["omitted"]), (&22.into(), &22.into()));
    assert!(none["coverage"]["complete"].as_bool().unwrap());
    let all = blockers(&env, &sci, &["--limit", "50"]);
    assert_eq!((&all["shown"], &all["omitted"]), (&22.into(), &0.into()));
    assert_eq!(all["limit"], 50);

    // No undocumented filters or sorts; negative and non-integer limits are usage errors;
    // the scope flags keep their existing conflicts and errors.
    for args in [
        &["blockers", "-n", "-1"][..],
        &["blockers", "--limit", "x"],
        &["blockers", "--status", "todo"],
        &["blockers", "--sort", "impact"],
        &["blockers", "--project", "sci", "--all-projects"],
        &["blockers", "--group", "g", "--project", "sci"],
    ] {
        env.usage(&sci, args);
    }
    assert_eq!(
        env.fail(&sci, &["blockers", "--group", "nope"]),
        "unknown_group"
    );
}

#[test]
fn blockers_scopes_and_read_only() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    // The spec's verification table, plus a lane the bb side depends on.
    let b = add(&env, &aa, "B", &["-p", "2"]);
    let x = add(&env, &aa, "X", &["-p", "2"]);
    let l = add(&env, &aa, "L", &["-p", "3", "--depends", &b]);
    let g = add(&env, &aa, "G", &["-p", "0", "--depends", &b]);
    let child = add(&env, &aa, "G child", &["--parent", &g]);
    env.json(&aa, &["done", &child, "closed child"]);
    let lane = add(&env, &aa, "Lane", &["--lane", "-p", "2"]);
    let r = add(&env, &bb, "R", &["-p", "1", "--depends", &b]);
    add(
        &env,
        &bb,
        "D",
        &["-p", "1", "--depends", &l, "--depends", &r],
    );
    add(
        &env,
        &bb,
        "M",
        &["-p", "2", "--depends", &b, "--depends", &x],
    );
    add(&env, &bb, "H", &["-p", "0", "--depends", &b]);
    add(&env, &bb, "Q", &["--depends", &lane]);
    env.json(&bb, &["group", "set", "both", "aa", "bb"]);

    let owned = [
        aa.join("tasks"),
        bb.join("tasks"),
        env.home.path().to_path_buf(),
    ];
    let owned: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let before = snapshot(&owned);

    // Local bb: B, L, X and the lane are foreign boundary rows; only L has dependencies.
    let local = blockers(&env, &bb, &[]);
    let coverage = &local["coverage"];
    assert_eq!(coverage["mode"], "local");
    assert_eq!(strings(&coverage["requested_projects"]), ["bb"]);
    assert_eq!(
        strings(&coverage["boundary_targets"]),
        sorted(&[&b, &l, &x, &lane])
    );
    assert_eq!(
        strings(&coverage["unfollowed_targets"]),
        std::slice::from_ref(&l)
    );
    assert_eq!(coverage["complete"], false);
    assert_eq!(counts(row(&local, &b)), [3, 4, 0, 0, 2, 0, 0]);
    assert_eq!(row(&local, &b)["goal"], Value::Null);
    assert_eq!(row(&local, &lane)["goal"], true);
    assert!(warns(&local, "partial") && warns(&local, &l), "{local}");
    let none = blockers(&env, &bb, &["-n", "0"]);
    assert_eq!(none["coverage"], local["coverage"]);
    assert_eq!(none["unresolved"], local["unresolved"]);
    assert!(warns(&none, "partial"), "{none}");

    // --project reads the registered root of another project; groups and all-projects
    // restore the whole diamond.
    let project = blockers(&env, &bb, &["--project", "aa"]);
    assert_eq!(project["coverage"]["mode"], "project");
    assert_eq!(strings(&project["coverage"]["requested_projects"]), ["aa"]);
    assert_eq!(counts(row(&project, &b)), [1, 1, 1, 1, 1, 0, 0]);
    assert_eq!(project["coverage"]["complete"], true);
    for (args, mode) in [
        (&["--group", "both"][..], "group"),
        (&["--all-projects"], "all_projects"),
    ] {
        let wide = blockers(&env, &bb, args);
        assert_eq!(wide["coverage"]["mode"], mode);
        assert_eq!(strings(&wide["coverage"]["scanned_projects"]), ["aa", "bb"]);
        assert_eq!(wide["coverage"]["complete"], true, "{wide}");
        assert_eq!(counts(row(&wide, &b)), [4, 5, 1, 1, 3, 0, 0]);
        assert_eq!(row(&wide, &b)["rank"], 1);
        assert_eq!(row(&wide, &b)["goal"], false);
    }
    assert_eq!(snapshot(&owned), before, "blockers must write nothing");

    // A local worktree read keeps the worktree's copy; --project reads the registered one.
    let scratch = tempfile::tempdir().unwrap();
    let tree = scratch.path().join("aa-tree");
    isolated_worktree(&env, &aa, &tree);
    env.json(&tree, &["done", &b, "closed in the worktree"]);
    assert!(!has_row(&blockers(&env, &tree, &[]), &b));
    assert!(has_row(&blockers(&env, &tree, &["--project", "aa"]), &b));

    // An unregistered target is a diagnostic, an unreachable project a partial scan.
    let cc = env.init("cc");
    let gone = add(&env, &cc, "Gone", &[]);
    let waits = add(&env, &bb, "Waits on gone", &["--depends", &gone]);
    env.json(&bb, &["unregister", "cc"]);
    let dd = env.init("dd");
    std::fs::remove_dir_all(&dd).unwrap();
    let wide = blockers(&env, &bb, &["--all-projects"]);
    assert_eq!(strings(&wide["coverage"]["unreachable_projects"]), ["dd"]);
    assert!(strings(&wide["coverage"]["requested_projects"]).contains(&"dd".to_string()));
    assert_eq!(wide["coverage"]["complete"], false);
    assert!(!has_row(&wide, &gone));
    let diagnostic = wide["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["dependent"] == waits)
        .unwrap_or_else(|| panic!("{wide}"));
    assert_eq!(diagnostic["reference"], gone);
    assert_eq!(diagnostic["reason"], "unresolved");
    assert!(warns(&wide, "unreachable"), "{wide}");

    // Malformed stores, records and configs are typed failures, never a partial report.
    let store = env.claim_store("bb");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&store, "not toml = [").unwrap();
    assert_eq!(env.fail(&bb, &["blockers"]), "config");
    std::fs::remove_file(&store).unwrap();
    std::fs::write(bb.join(format!("tasks/{r}.md")), "not a task\n").unwrap();
    assert_eq!(env.fail(&bb, &["blockers"]), "parse");
    std::fs::write(aa.join("tasks/.config.toml"), "not toml = [").unwrap();
    assert_eq!(env.fail(&aa, &["blockers"]), "config");
}

#[test]
fn blockers_attention_claims_and_resources() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let config = sci.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[needs.paid-calls]\nmeaning = \"spends money on a provider\"\n");
    std::fs::write(&config, text).unwrap();
    let blocker_of = |title: &str, extra: &[&str]| {
        let id = add(&env, &sci, title, extra);
        add(
            &env,
            &sci,
            &format!("Consumer of {title}"),
            &["--depends", &id],
        );
        id
    };

    // A prose-only approval gate is unknown; an explicit user park says so, rank unchanged.
    let gate = blocker_of(
        "Prepare bounded calls",
        &[
            "-p",
            "2",
            "-b",
            "Needs approval before any paid call is made.",
        ],
    );
    let before = blockers(&env, &sci, &[]);
    let unparked = row(&before, &gate);
    assert_eq!(unparked["attention"], "unknown");
    assert!(unparked.get("park").is_none() && unparked.get("claim").is_none());
    assert_eq!(unparked["awaiting_user"], false);
    assert_eq!(unparked["task_needs"], serde_json::json!([]));
    let step = "Obtain authorization for the prepared bounded calls";
    env.json(
        &sci,
        &[
            "park",
            &gate,
            step,
            "--waiting-on",
            "user",
            "--reason",
            "approval",
        ],
    );
    let after = blockers(&env, &sci, &[]);
    let parked = row(&after, &gate);
    assert_eq!(parked["attention"], "user");
    assert_eq!(parked["park"]["waiting_on"], "user");
    assert_eq!(parked["park"]["reason"], "approval");
    assert_eq!(parked["park"]["next_step"], step);
    assert_eq!(parked["rank"], unparked["rank"]);
    assert_eq!(counts(parked), counts(unparked));
    assert!(
        after["park_evidence"]["host"]
            .as_str()
            .is_some_and(|h| !h.is_empty())
    );
    assert_eq!(after["park_evidence"]["scope"], "host_local");

    // Another host's store has no park: unknown again.
    let elsewhere = tempfile::tempdir().unwrap();
    let out = env
        .cmd(&sci)
        .env("XDG_STATE_HOME", elsewhere.path())
        .args(["blockers"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let other: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(row(&other, &gate)["attention"], "unknown");
    assert!(row(&other, &gate).get("park").is_none());

    // A declared need names a resource, not an approval; a park's host requirement is a
    // separate field beside it.
    let needy = blocker_of("Needs money", &["--need", "paid-calls"]);
    let out = blockers(&env, &sci, &[]);
    assert_eq!(row(&out, &needy)["attention"], "unknown");
    assert_eq!(strings(&row(&out, &needy)["task_needs"]), ["paid-calls"]);
    env.json(
        &sci,
        &[
            "park",
            &needy,
            "run it overnight",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
            "--needs",
            "headless",
        ],
    );
    let out = blockers(&env, &sci, &[]);
    assert_eq!(strings(&row(&out, &needy)["task_needs"]), ["paid-calls"]);
    assert_eq!(row(&out, &needy)["park"]["needs"], "headless");
    assert_eq!(row(&out, &needy)["attention"], "user");

    // A reviewed idea waiting on the user is user attention without a park; an ordinary
    // todo cannot carry that marker through a valid write.
    let idea = blocker_of("Idea waiting on the user", &["--status", "idea"]);
    env.json(
        &sci,
        &[
            "note",
            &idea,
            "asked the user",
            "--reviewed",
            "--awaiting-user",
        ],
    );
    let out = blockers(&env, &sci, &[]);
    assert_eq!(row(&out, &idea)["attention"], "user");
    assert_eq!(row(&out, &idea)["awaiting_user"], true);
    assert!(row(&out, &idea).get("park").is_none());
    assert_eq!(
        env.fail(&sci, &["note", &gate, "x", "--reviewed", "--awaiting-user"]),
        "validation"
    );

    // A park waiting on an agent is evidence, not authorization.
    let handed = blocker_of("Handed to an agent", &[]);
    env.json(&sci, &["park", &handed, "pick it up"]);
    let out = blockers(&env, &sci, &[]);
    assert_eq!(row(&out, &handed)["attention"], "unknown");
    assert_eq!(row(&out, &handed)["park"]["waiting_on"], "agent");

    // Held claims are shown, with uncertainty; stale ones are omitted.
    let live = blocker_of("Held live", &[]);
    let unsure = blocker_of("Held unverifiable", &[]);
    let stale = blocker_of("Held stale", &[]);
    write_claim(&env, "sci", &live, "live-session", "live");
    write_claim(&env, "sci", &unsure, "far-session", "unverifiable");
    write_claim(&env, "sci", &stale, "dead-session", "stale");
    let out = blockers(&env, &sci, &[]);
    assert_eq!(row(&out, &live)["claim"]["session"], "live-session");
    assert!(row(&out, &live)["claim"].get("unverified").is_none());
    assert_eq!(row(&out, &unsure)["claim"]["live"], true);
    assert!(row(&out, &unsure)["claim"]["unverified"].is_string());
    assert!(row(&out, &stale).get("claim").is_none());
    for id in [&live, &unsure, &stale] {
        assert_eq!(row(&out, id)["attention"], "unknown");
        assert_eq!(counts(row(&out, id))[0], 1);
    }
}

#[test]
fn blockers_pretty_contract_and_existing_reads() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let config = sci.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("color = \"#00d7ff\"\n{text}")).unwrap();
    let title = "Ünïcode blocker — 長いタイトル with a deliberately long title that has to wrap \
                 across several lines in a narrow terminal";
    let blocker = add(&env, &sci, title, &["-p", "1"]);
    let mut consumers = Vec::new();
    for name in ["shelved", "user-held", "claimed", "deferred"] {
        let mut extra = vec!["--depends", blocker.as_str()];
        if name == "deferred" {
            extra.extend(["--defer", "30d"]);
        }
        consumers.push(add(&env, &sci, &format!("Consumer {name}"), &extra));
    }
    env.json(&sci, &["shelve", &consumers[0], "later"]);
    env.json(
        &sci,
        &["park", &consumers[1], "ask", "--waiting-on", "user"],
    );
    write_claim(&env, "sci", &consumers[2], "holder", "live");

    let ready = env.json(&sci, &["ready"]);
    let next = env.json(&sci, &["next"]);
    let out = blockers(&env, &sci, &[]);
    assert_eq!(env.json(&sci, &["ready"]), ready);
    assert_eq!(env.json(&sci, &["next"]), next);

    // Relief counts all four, though none of them is ready.
    let top = row(&out, &blocker);
    assert_eq!(counts(top), [4, 4, 0, 0, 4, 0, 0]);
    let ready_ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(consumers.iter().all(|id| !ready_ids.contains(&id.as_str())));
    let map = serde_json::json!({"idea": 0, "todo": 3, "doing": 0, "blocked": 0, "shelved": 1});
    assert_eq!(top["direct_work_by_status"], map);
    assert_eq!(top["transitive_work_by_status"], map);

    // The JSON contract: exact top-level keys, every row key, optional ones omitted.
    let mut keys: Vec<&str> = out
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "blockers",
            "coverage",
            "cycles",
            "goal_detection",
            "limit",
            "observed_at",
            "omitted",
            "park_evidence",
            "ranking",
            "shown",
            "total",
            "unresolved",
            "warnings"
        ]
    );
    assert_eq!(out["goal_detection"], "structural_only");
    assert_eq!(
        out["ranking"],
        "transitive_work,direct_work,priority,created,id"
    );
    let mut row_keys: Vec<&str> = top
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    row_keys.sort();
    assert_eq!(
        row_keys,
        [
            "attention",
            "awaiting_user",
            "cycle",
            "dependencies_cleared_direct_count",
            "direct_examples",
            "direct_goal_count",
            "direct_work_by_status",
            "direct_work_count",
            "goal",
            "id",
            "priority",
            "rank",
            "status",
            "task_needs",
            "title",
            "transitive_goal_count",
            "transitive_work_by_status",
            "transitive_work_count",
            "unmet_dependency_count",
            "unresolved_dependency_count"
        ]
    );
    let mut examples = consumers.clone();
    examples.sort();
    assert_eq!(strings(&top["direct_examples"]), examples[..3]);
    assert!(warns(&out, "history"), "{out}");

    // Pretty: complete IDs and counts at every width, wrapped titles, every label.
    let pretty = |args: &[&str], columns: &str| -> String {
        let mut all = vec!["--pretty"];
        all.extend_from_slice(args);
        all.push("blockers");
        let out = env
            .cmd(&sci)
            .env("COLUMNS", columns)
            .args(&all)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    for columns in ["120", "60"] {
        for headers in [&["--headers"][..], &[]] {
            let mut args = headers.to_vec();
            args.extend(["--color", "never"]);
            let text = pretty(&args, columns);
            assert!(text.contains(&blocker), "{text}");
            assert!(text.contains("Ünïcode"), "{text}");
            assert!(text.contains("coverage complete"), "{text}");
            for label in [
                "cleared 4",
                "goals 0/0",
                "attention unknown",
                "todo 3",
                "shelved 1",
            ] {
                assert!(text.contains(label), "{label}: {text}");
            }
            for footer in [
                "structural only",
                "not attributed to a goal's children",
                "host-local",
                "unknown",
                "does not mean ready",
            ] {
                assert!(text.contains(footer), "{footer}: {text}");
            }
            if columns == "120" {
                assert!(
                    text.lines().all(|line| line.chars().count() <= 120),
                    "{text}"
                );
            }
            let header = text
                .lines()
                .any(|line| line.trim_start().starts_with("rank") && line.contains("title"));
            assert_eq!(header, !headers.is_empty(), "{text}");
        }
    }
    assert!(pretty(&["--color", "always"], "120").contains("\u{1b}["));
    assert!(!pretty(&["--color", "never"], "120").contains("\u{1b}["));

    // Every continuation label on a ranked row: a user park's reason and step, its host
    // requirement beside the task's needs, an agent park, a reviewed idea awaiting the
    // user, a hand-seeded cycle (the CLI refuses one) and a future deferral.
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[needs.paid-calls]\nmeaning = \"spends money on a provider\"\n");
    std::fs::write(&config, text).unwrap();
    let blocker_of = |title: &str, extra: &[&str]| {
        let id = add(&env, &sci, title, extra);
        add(
            &env,
            &sci,
            &format!("Consumer of {title}"),
            &["--depends", &id],
        );
        id
    };
    let quiet = blocker_of("Quiet run", &["--need", "paid-calls"]);
    env.json(
        &sci,
        &[
            "park",
            &quiet,
            "run it overnight",
            "--waiting-on",
            "user",
            "--reason",
            "quiet",
            "--minutes",
            "5",
            "--needs",
            "headless",
        ],
    );
    let handed = blocker_of("Handed over", &[]);
    env.json(&sci, &["park", &handed, "pick it up"]);
    let idea = blocker_of("Waiting idea", &["--status", "idea"]);
    env.json(
        &sci,
        &[
            "note",
            &idea,
            "asked the user",
            "--reviewed",
            "--awaiting-user",
        ],
    );
    blocker_of("Later", &["--defer", "30d"]);
    let first = add(&env, &sci, "Cycle first", &[]);
    let second = add(&env, &sci, "Cycle second", &["--depends", &first]);
    let record = sci.join(format!("tasks/{first}.md"));
    let seeded = std::fs::read_to_string(&record)
        .unwrap()
        .replace("depends: []", &format!("depends: [{second}]"));
    std::fs::write(&record, seeded).unwrap();
    let text = pretty(&["--color", "never"], "120");
    for line in [
        "user:   quiet: run it overnight",
        "host:   park host requirement headless",
        "needs:  paid-calls",
        "park:   waiting on agent: pick it up",
        "user:   reviewed idea awaiting the user",
        "cycle:  investigate cycle",
        "defer:  deferred until ",
        "cycles (1):",
    ] {
        assert!(text.contains(line), "{line}: {text}");
    }

    // A claim shows its holder; a cut path makes the heading partial even at -n 0.
    let claimed = add(&env, &sci, "Held blocker", &[]);
    add(&env, &sci, "Waits on held", &["--depends", &claimed]);
    write_claim(&env, "sci", &claimed, "far-session", "unverifiable");
    let text = pretty(&["--color", "never"], "120");
    assert!(
        text.contains("someone") && text.contains("unverifiable"),
        "{text}"
    );
    let other = env.init("fam");
    let foreign_dep = add(&env, &other, "Foreign dependency", &[]);
    let foreign = add(&env, &other, "Foreign", &["--depends", &foreign_dep]);
    add(&env, &sci, "Waits on foreign", &["--depends", &foreign]);
    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "never", "blockers", "-n", "0"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("coverage partial"), "{text}");
    assert!(text.contains("ranks may change"), "{text}");
    assert_eq!(
        blockers(&env, &sci, &["-n", "0"])["coverage"]["complete"],
        false
    );
}
