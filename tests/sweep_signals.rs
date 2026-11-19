mod common;
use common::{TestEnv, git};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const COLUMNS: &str = "id status updated reviewed awaiting_user deferred_until parked \
    dead_paths cited_closed deps_closed touched_commits twins triggers cluster verdict \
    action evidence decided";

type Row = HashMap<String, String>;

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/sweep-signals")
}

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

/// Runs the script on `dir` with the suite's isolated environment plus `extra`; returns the
/// exit code, the rows keyed by header, and standard error.
fn ledger_with(env: &TestEnv, dir: &Path, extra: &[(&str, OsString)]) -> (i32, Vec<Row>, String) {
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", dir.to_str().unwrap()])
        .args([
            "--tasks",
            assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap(),
        ])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .envs(extra.iter().map(|(k, v)| (*k, v)))
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.lines();
    let header: Vec<&str> = match lines.next() {
        Some(h) => h.split('\t').collect(),
        None => {
            return (
                out.status.code().unwrap(),
                Vec::new(),
                stderr_of(&out.stderr),
            );
        }
    };
    let rows = lines
        .map(|l| {
            let cells: Vec<&str> = l.split('\t').collect();
            assert_eq!(cells.len(), header.len(), "ragged row {l:?}");
            header
                .iter()
                .zip(cells)
                .map(|(h, c)| (h.to_string(), c.to_string()))
                .collect()
        })
        .collect();
    (out.status.code().unwrap(), rows, stderr_of(&out.stderr))
}

fn stderr_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn ledger(env: &TestEnv, dir: &Path) -> (i32, Vec<Row>, String) {
    ledger_with(env, dir, &[])
}

/// Rewrites specified frontmatter timestamp lines on `<dir>/tasks/<id>.md`.
fn set_stamps(dir: &Path, id: &str, stamps: &[(&str, &str)]) {
    let path = dir.join("tasks").join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let rewritten: Vec<String> = text
        .lines()
        .map(|l| {
            for (field, value) in stamps {
                let prefix = format!("{field}:");
                if l.starts_with(&prefix) {
                    return format!("{field}: {value}");
                }
            }
            l.to_string()
        })
        .collect();
    std::fs::write(&path, rewritten.join("\n") + "\n").unwrap();
}

/// Rewrites the record's `updated:` line to an early date.
fn backdate(dir: &Path, id: &str) {
    set_stamps(dir, id, &[("updated", "2026-01-01T00:00:00Z")]);
}

fn add(env: &TestEnv, dir: &Path, title: &str, body: &str) -> String {
    id_of(env.json(dir, &["add", title, "--status", "todo", "--body", body]))
}

fn init_git(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
    ] {
        git(dir, &["config", key, value]);
    }
}

fn row<'a>(rows: &'a [Row], id: &str) -> &'a Row {
    rows.iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no row for {id}"))
}

fn fixture_and_assert(extra: &[(&str, OsString)]) {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let other = env.init("bb");
    init_git(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/kept.rs"), "fn main() {}\n").unwrap();
    git(&dir, &["add", "src"]);
    git(&dir, &["commit", "-q", "-m", "init"]);

    let plain = add(&env, &dir, "Plain task", "nothing named");
    let dead = add(
        &env,
        &dir,
        "Dead paths",
        "see `src/gone.rs`, `src/kept.rs`, src/kept.rs:12, [k](src/kept.rs) and src/kept.rs.",
    );

    let cited = add(&env, &dir, "Cites others", "placeholder");
    let second = add(&env, &dir, "Second", "to be dropped");
    env.json(
        &dir,
        &[
            "edit",
            &cited,
            "--body",
            &format!("see {second}, {cited} and zz-000001"),
        ],
    );
    backdate(&dir, &cited);
    env.json(&dir, &["drop", &second, "curate: test"]);

    let moved_open = add(&env, &dir, "Moves away open", "x");
    let moved_closed = add(&env, &dir, "Moves away closed", "x");
    let moved_open_deps = id_of(env.json(
        &dir,
        &[
            "add",
            "Depends on moved open",
            "--status",
            "todo",
            "--depends",
            &moved_open,
        ],
    ));
    let moved_closed_deps = id_of(env.json(
        &dir,
        &[
            "add",
            "Depends on moved closed",
            "--status",
            "todo",
            "--depends",
            &moved_closed,
        ],
    ));
    backdate(&dir, &moved_closed_deps);
    let new_open = id_of(env.json(&dir, &["move", &moved_open, "--to", "bb"]));
    let new_closed = id_of(env.json(&dir, &["move", &moved_closed, "--to", "bb"]));
    let _ = new_open;
    env.json(&other, &["drop", &new_closed, "curate: test"]);
    let moved_cite = add(&env, &dir, "Cites moved open", &format!("see {moved_open}"));
    backdate(&dir, &moved_cite);
    let moved_closed_cite = add(
        &env,
        &dir,
        "Cites moved closed",
        &format!("see {moved_closed}"),
    );
    backdate(&dir, &moved_closed_cite);

    let dep = add(&env, &dir, "A dependency", "x");
    let deps = id_of(env.json(
        &dir,
        &["add", "Has deps", "--status", "todo", "--depends", &dep],
    ));
    backdate(&dir, &deps);
    env.json(&dir, &["done", &dep, "--force", "finished"]);

    let touched = add(&env, &dir, "Touches", "names src/kept.rs");
    backdate(&dir, &touched);
    std::fs::write(dir.join("src/kept.rs"), "fn main() { 1; }\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "change"]);

    let twin_a = add(&env, &dir, "Rename the widget cache loader", "x");
    let twin_b = add(&env, &dir, "Rename widget cache loader module", "x");
    let started = add(&env, &dir, "Started elsewhere", "x");
    env.json(&dir, &["start", &started]);
    let closed = add(&env, &dir, "Closed", "x");
    env.json(&dir, &["done", &closed, "--force", "finished"]);

    let (code, rows, stderr) = ledger_with(&env, &dir, extra);
    assert_eq!(code, 0, "{stderr}");
    let mut ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str()).collect();
    let sorted = {
        let mut v = ids.clone();
        v.sort();
        v
    };
    assert_eq!(ids, sorted, "rows are sorted by id");
    ids.retain(|i| *i == started || *i == closed || *i == second);
    assert!(
        ids.is_empty(),
        "claimed, done and dropped tasks have no row"
    );
    for r in &rows {
        for col in ["cluster", "verdict", "action", "evidence", "decided"] {
            assert_eq!(r[col], "", "{col} is left for the controller: {r:?}");
        }
    }

    let r = row(&rows, &plain);
    assert_eq!(r["dead_paths"], "");
    assert_eq!(r["cited_closed"], "");
    assert_eq!(r["deps_closed"], "false");
    assert_eq!(r["touched_commits"], "0");
    assert_eq!(r["triggers"], "0");

    let r = row(&rows, &dead);
    assert_eq!(r["dead_paths"], "src/gone.rs");
    assert_eq!(r["triggers"], "1");

    assert_eq!(row(&rows, &cited).get("cited_closed").unwrap(), &second);
    assert_eq!(row(&rows, &moved_cite)["cited_closed"], "");
    assert_eq!(row(&rows, &moved_closed_cite)["cited_closed"], moved_closed);

    let r = row(&rows, &deps);
    assert_eq!(r["deps_closed"], "true");
    assert_eq!(r["triggers"], "1");

    let r = row(&rows, &moved_open_deps);
    assert_eq!(r["deps_closed"], "false");
    assert_eq!(r["triggers"], "0");

    let r = row(&rows, &moved_closed_deps);
    assert_eq!(r["deps_closed"], "true");
    assert_eq!(r["triggers"], "1");

    let r = row(&rows, &touched);
    assert!(r["touched_commits"].parse::<u32>().unwrap() >= 1, "{r:?}");
    assert_eq!(r["triggers"], "0");

    assert_eq!(row(&rows, &twin_a)["twins"], twin_b);
    assert_eq!(row(&rows, &twin_b)["twins"], twin_a);
}

#[test]
fn sweep_signals_reports_each_signal() {
    fixture_and_assert(&[]);
}

#[test]
fn sweep_signals_header_is_the_column_list() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    add(&env, &dir, "One", "x");
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", dir.to_str().unwrap()])
        .args([
            "--tasks",
            assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap(),
        ])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let want: Vec<&str> = COLUMNS.split_whitespace().collect();
    assert_eq!(
        text.lines().next().unwrap().split('\t').collect::<Vec<_>>(),
        want
    );
}

#[test]
fn sweep_signals_git_fixtures_ignore_hostile_environment() {
    if common::git_fixture::hostile_parent("sweep_signals_git_fixtures_ignore_hostile_environment")
    {
        return;
    }
    // env_clear() in the helper drops the hostile selectors, so hand them over explicitly.
    let selectors: Vec<(&str, OsString)> = [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ]
    .into_iter()
    .filter_map(|k| std::env::var_os(k).map(|v| (k, v)))
    .collect();
    fixture_and_assert(&selectors);
}

#[test]
fn sweep_signals_empty_pool_prints_header_only() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", dir.to_str().unwrap()])
        .args([
            "--tasks",
            assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap(),
        ])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(String::from_utf8(out.stdout).unwrap().lines().count(), 1);
}

#[test]
fn sweep_signals_fails_without_partial_output() {
    let env = TestEnv::new();
    let empty = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", empty.path().to_str().unwrap()])
        .args([
            "--tasks",
            assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap(),
        ])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

#[test]
fn sweep_signals_non_git_checkout_fails() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/kept.rs"), "fn main() {}\n").unwrap();
    add(&env, &dir, "Names a file", "see src/kept.rs");
    let (code, rows, stderr) = ledger(&env, &dir);
    assert_eq!(code, 1, "{stderr}");
    assert!(rows.is_empty());
    assert!(stderr.contains("git"), "{stderr}");
}

#[test]
fn sweep_signals_missing_git_reports_command_without_traceback() {
    let env = TestEnv::new();
    let empty_bin = tempfile::tempdir().unwrap();
    let python = {
        let lookup = std::process::Command::new("sh")
            .args(["-c", "command -v python3"])
            .env_clear()
            .envs(common::isolated_env(env.home.path()))
            .output()
            .unwrap();
        assert!(lookup.status.success());
        PathBuf::from(String::from_utf8(lookup.stdout).unwrap().trim())
    };
    let out = std::process::Command::new(python)
        .arg("-I")
        .arg(script())
        .args(["--root", env.home.path().to_str().unwrap()])
        .args([
            "--tasks",
            assert_cmd::cargo::cargo_bin("tasks").to_str().unwrap(),
        ])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .env("PATH", empty_bin.path())
        .output()
        .unwrap();
    let stderr = stderr_of(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert!(
        stderr.starts_with("sweep-signals: git rev-parse --local-env-vars:"),
        "{stderr}"
    );
    assert!(!stderr.contains("Traceback"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn sweep_signals_unexpected_json_shape_reports_command_without_traceback() {
    use std::os::unix::fs::PermissionsExt;

    let env = TestEnv::new();
    let stub_dir = tempfile::tempdir().unwrap();
    let stub = stub_dir.path().join("bad-tasks");
    std::fs::write(&stub, "#!/bin/sh\nprintf '{}\\n'\n").unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", stub_dir.path().to_str().unwrap()])
        .args(["--tasks", stub.to_str().unwrap()])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .output()
        .unwrap();
    let stderr = stderr_of(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert!(stderr.starts_with("sweep-signals:"), "{stderr}");
    assert!(stderr.contains("where"), "{stderr}");
    assert!(!stderr.contains("Traceback"), "{stderr}");

    // A list row missing required `updated` must also fail closed rather than default to "".
    let root = stub_dir.path().display();
    let missing_updated = stub_dir.path().join("missing-updated-tasks");
    std::fs::write(
        &missing_updated,
        format!(
            "#!/bin/sh\ncase \"$3\" in\n  where) printf '{{\"project\":{{\"prefix\":\"aa\",\"checkout\":\"{root}\",\"records\":\"{root}\"}}}}\\n' ;;\n  list) printf '{{\"tasks\":[{{\"id\":\"aa-000001\",\"status\":\"todo\",\"title\":\"No updated\"}}]}}\\n' ;;\n  show) printf '{{\"task\":{{\"title\":\"No updated\",\"body\":\"\"}}}}\\n' ;;\nesac\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&missing_updated, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(script())
        .args(["--root", stub_dir.path().to_str().unwrap()])
        .args(["--tasks", missing_updated.to_str().unwrap()])
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .output()
        .unwrap();
    let stderr = stderr_of(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert!(stderr.starts_with("sweep-signals:"), "{stderr}");
    assert!(stderr.contains("list"), "{stderr}");
    assert!(!stderr.contains("Traceback"), "{stderr}");
}

#[test]
fn sweep_signals_deps_closed_requires_closing_stamp_after_updated() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let other = env.init("bb");

    let dep_old = add(&env, &dir, "Old closed dep", "x");
    env.json(&dir, &["done", &dep_old, "--force", "finished"]);
    // Closed at 02-01, then gained a post-close note bumping updated to 04-01.
    set_stamps(
        &dir,
        &dep_old,
        &[
            ("closed", "2026-02-01T00:00:00Z"),
            ("completed", "2026-02-01T00:00:00Z"),
            ("updated", "2026-04-01T00:00:00Z"),
        ],
    );

    let dep_new = add(&env, &dir, "New closed dep", "x");
    env.json(&dir, &["drop", &dep_new, "curate: test"]);
    set_stamps(
        &dir,
        &dep_new,
        &[
            ("closed", "2026-03-15T00:00:00Z"),
            ("updated", "2026-03-15T00:00:00Z"),
        ],
    );

    let dep_open = add(&env, &dir, "Open dep", "x");

    let stale_refined = id_of(env.json(
        &dir,
        &[
            "add",
            "Refined after dep closed",
            "--status",
            "todo",
            "--depends",
            &dep_old,
        ],
    ));
    set_stamps(&dir, &stale_refined, &[("updated", "2026-03-01T00:00:00Z")]);

    let same_second = id_of(env.json(
        &dir,
        &[
            "add",
            "Updated at exact dep close",
            "--status",
            "todo",
            "--depends",
            &dep_old,
        ],
    ));
    set_stamps(&dir, &same_second, &[("updated", "2026-02-01T00:00:00Z")]);

    let multi_max_newer = id_of(env.json(
        &dir,
        &[
            "add",
            "Last dep closed after update",
            "--status",
            "todo",
            "--depends",
            &dep_old,
            "--depends",
            &dep_new,
        ],
    ));
    set_stamps(
        &dir,
        &multi_max_newer,
        &[("updated", "2026-03-01T00:00:00Z")],
    );

    let multi_one_open = id_of(env.json(
        &dir,
        &[
            "add",
            "Still waiting on open dep",
            "--status",
            "todo",
            "--depends",
            &dep_new,
            "--depends",
            &dep_open,
        ],
    ));
    set_stamps(
        &dir,
        &multi_one_open,
        &[("updated", "2026-01-01T00:00:00Z")],
    );

    let fdep = add(&env, &other, "Foreign closed dep", "x");
    env.json(&other, &["done", &fdep, "--force", "finished"]);
    set_stamps(
        &other,
        &fdep,
        &[
            ("closed", "2026-02-01T00:00:00Z"),
            ("completed", "2026-02-01T00:00:00Z"),
            ("updated", "2026-04-01T00:00:00Z"),
        ],
    );

    let foreign_refined = id_of(env.json(
        &dir,
        &[
            "add",
            "Refined after foreign dep closed",
            "--status",
            "todo",
            "--depends",
            &fdep,
        ],
    ));
    set_stamps(
        &dir,
        &foreign_refined,
        &[("updated", "2026-03-01T00:00:00Z")],
    );

    let foreign_woken = id_of(env.json(
        &dir,
        &[
            "add",
            "Woken by foreign dep",
            "--status",
            "todo",
            "--depends",
            &fdep,
        ],
    ));
    set_stamps(&dir, &foreign_woken, &[("updated", "2026-01-01T00:00:00Z")]);

    let (code, rows, stderr) = ledger(&env, &dir);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(row(&rows, &stale_refined)["deps_closed"], "false");
    assert_eq!(row(&rows, &stale_refined)["triggers"], "0");
    assert_eq!(row(&rows, &same_second)["deps_closed"], "false");
    assert_eq!(row(&rows, &multi_max_newer)["deps_closed"], "true");
    assert_eq!(row(&rows, &multi_max_newer)["triggers"], "1");
    assert_eq!(row(&rows, &multi_one_open)["deps_closed"], "false");
    assert_eq!(row(&rows, &foreign_refined)["deps_closed"], "false");
    assert_eq!(row(&rows, &foreign_woken)["deps_closed"], "true");
}

#[test]
fn sweep_signals_cited_closed_matches_registered_foreign_ids_and_follows_moves() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    let dd = env.init("dd");

    let bb_closed = add(&env, &bb, "Foreign done", "x");
    env.json(&bb, &["done", &bb_closed, "--force", "finished"]);
    set_stamps(
        &bb,
        &bb_closed,
        &[
            ("closed", "2026-03-01T00:00:00Z"),
            ("completed", "2026-03-01T00:00:00Z"),
            ("updated", "2026-03-01T00:00:00Z"),
        ],
    );

    let bb_stale = add(&env, &bb, "Foreign closed earlier", "x");
    env.json(&bb, &["done", &bb_stale, "--force", "finished"]);
    set_stamps(
        &bb,
        &bb_stale,
        &[
            ("closed", "2026-01-01T00:00:00Z"),
            ("completed", "2026-01-01T00:00:00Z"),
            ("updated", "2026-04-01T00:00:00Z"),
        ],
    );

    let bb_open = add(&env, &bb, "Foreign open", "x");

    let bb_moved_closed = add(&env, &bb, "Foreign moved then closed", "x");
    let cc_closed = id_of(env.json(&bb, &["move", &bb_moved_closed, "--to", "cc"]));
    env.json(&cc, &["drop", &cc_closed, "curate: test"]);
    set_stamps(
        &cc,
        &cc_closed,
        &[
            ("closed", "2026-03-01T00:00:00Z"),
            ("updated", "2026-03-01T00:00:00Z"),
        ],
    );

    let bb_moved_open = add(&env, &bb, "Foreign moved still open", "x");
    let _cc_open = id_of(env.json(&bb, &["move", &bb_moved_open, "--to", "cc"]));

    // Retired alias prefix: create under `dd`, rename `dd` -> `dd2`, close in `dd2`.
    let dd_old_id = add(&env, &dd, "Renamed project task", "x");
    let dd_suffix = dd_old_id.split_once('-').unwrap().1;
    env.json(&dd, &["rename", "dd", "dd2"]);
    let dd2_id = format!("dd2-{dd_suffix}");
    env.json(&dd, &["done", &dd2_id, "--force", "finished"]);
    set_stamps(
        &dd,
        &dd2_id,
        &[
            ("closed", "2026-03-01T00:00:00Z"),
            ("completed", "2026-03-01T00:00:00Z"),
            ("updated", "2026-03-01T00:00:00Z"),
        ],
    );

    // 6-hop move chain (> MAX_HOPS = 5) ending at a closed task must not count.
    let mut hop_dirs = Vec::new();
    for p in ["h0", "h1", "h2", "h3", "h4", "h5", "h6"] {
        hop_dirs.push((p, env.init(p)));
    }
    let first_hop = add(&env, &hop_dirs[0].1, "Six hops", "x");
    let mut cur_id = first_hop.clone();
    for w in hop_dirs.windows(2) {
        cur_id = id_of(env.json(&w[0].1, &["move", &cur_id, "--to", w[1].0]));
    }
    env.json(&hop_dirs[6].1, &["done", &cur_id, "--force", "finished"]);
    set_stamps(
        &hop_dirs[6].1,
        &cur_id,
        &[
            ("closed", "2026-03-01T00:00:00Z"),
            ("completed", "2026-03-01T00:00:00Z"),
            ("updated", "2026-03-01T00:00:00Z"),
        ],
    );

    let cites_foreign = add(
        &env,
        &dir,
        "Cites foreign tasks",
        &format!(
            "see {bb_closed}, {bb_stale}, {bb_open}, bb-deadbe, zz-000001, foo-{bb_closed} and {bb_closed}-bar."
        ),
    );
    backdate(&dir, &cites_foreign);

    let cites_moved = add(
        &env,
        &dir,
        "Cites foreign moved",
        &format!("see {bb_moved_closed} and {bb_moved_open}"),
    );
    backdate(&dir, &cites_moved);

    let cites_alias = add(
        &env,
        &dir,
        "Cites retired alias",
        &format!("see {dd_old_id}"),
    );
    backdate(&dir, &cites_alias);

    let cites_too_many_hops = add(&env, &dir, "Cites six hops", &format!("see {first_hop}"));
    backdate(&dir, &cites_too_many_hops);

    let (code, rows, stderr) = ledger(&env, &dir);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(row(&rows, &cites_foreign)["cited_closed"], bb_closed);
    assert_eq!(row(&rows, &cites_foreign)["triggers"], "1");
    assert_eq!(row(&rows, &cites_moved)["cited_closed"], bb_moved_closed);
    assert_eq!(row(&rows, &cites_alias)["cited_closed"], dd_old_id);
    assert_eq!(row(&rows, &cites_too_many_hops)["cited_closed"], "");
}

#[test]
fn sweep_signals_scans_promoted_refs_spec_plan_and_existing_root_files() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");
    init_git(&dir);

    for rel in [
        "src/kept.rs",
        "docs/specs/kept-spec.md",
        "docs/specs/gone-spec.md",
        "docs/plans/kept-plan.md",
        "docs/plans/gone-plan.md",
        "docs/notes/kept-brief.md",
        "docs/notes/gone-brief.md",
        "justfile",
        "README.md",
        "AGENTS.md",
        "identity.toml",
        ".tt.toml",
        "GONE_ROOT.md",
    ] {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, "initial\n").unwrap();
    }
    let snap_src = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(snap_src.path(), "attachment bytes\n").unwrap();
    git(
        &dir,
        &[
            "add",
            "src",
            "docs",
            "justfile",
            "README.md",
            "AGENTS.md",
            "identity.toml",
            ".tt.toml",
            "GONE_ROOT.md",
        ],
    );
    common::git_fixture::assert_repo(env.home.path(), &dir);
    let commit_at = |stamp: &str, msg: &str| {
        let out = common::git_fixture::command(env.home.path(), &dir)
            .env("GIT_AUTHOR_DATE", stamp)
            .env("GIT_COMMITTER_DATE", stamp)
            .args(["commit", "-q", "-m", msg])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    };
    commit_at("2026-01-01T00:00:00Z", "init");

    let dead_refs = id_of(env.json(
        &dir,
        &[
            "add",
            "Promoted targets later deleted",
            "--status",
            "todo",
            "--spec",
            "docs/specs/gone-spec.md",
            "--plan",
            "docs/plans/gone-plan.md",
            "--ref",
            "brief (input): docs/notes/gone-brief.md",
            "--ref",
            "rootdoc (context): GONE_ROOT.md",
            "--ref",
            "report (output): docs/notes/future-report.md",
            "--ref",
            "ext (context): https://example.com/missing/doc.md",
            "--body",
            "see [ref:brief], [ref:rootdoc], [ref:report] and [ref:ext]",
        ],
    ));
    env.json(
        &dir,
        &[
            "attach",
            &dead_refs,
            snap_src.path().to_str().unwrap(),
            "--name",
            "snap.txt",
        ],
    );
    env.json(
        &dir,
        &["edit", &dead_refs, "--ref", "snap (input): files:snap.txt"],
    );

    for gone in [
        "docs/specs/gone-spec.md",
        "docs/plans/gone-plan.md",
        "docs/notes/gone-brief.md",
        "GONE_ROOT.md",
    ] {
        std::fs::remove_file(dir.join(gone)).unwrap();
    }

    let touched_refs = id_of(env.json(
        &dir,
        &[
            "add",
            "Promoted targets modified in git",
            "--status",
            "todo",
            "--spec",
            "docs/specs/kept-spec.md",
            "--plan",
            "docs/plans/kept-plan.md",
            "--ref",
            "brief (input): docs/notes/kept-brief.md",
            "--body",
            "see [ref:brief] only",
        ],
    ));
    set_stamps(&dir, &touched_refs, &[("updated", "2026-06-01T00:00:00Z")]);

    let touched_roots = id_of(env.json(
        &dir,
        &[
            "add",
            "Root files modified in git",
            "--status",
            "todo",
            "--ref",
            "jf (context): justfile",
            "--body",
            "update `justfile` ([ref:jf]), README.md., AGENTS.md:12, (identity.toml) and .tt.toml; mention sweep.md",
        ],
    ));
    set_stamps(&dir, &touched_roots, &[("updated", "2026-06-01T00:00:00Z")]);

    let subpath_only = add(
        &env,
        &dir,
        "Subpath and bare non-root word",
        "mentions src/justfile and sweep.md only",
    );
    set_stamps(&dir, &subpath_only, &[("updated", "2026-06-01T00:00:00Z")]);

    for rel in [
        "docs/specs/kept-spec.md",
        "docs/plans/kept-plan.md",
        "docs/notes/kept-brief.md",
    ] {
        std::fs::write(dir.join(rel), "changed\n").unwrap();
        git(&dir, &["add", rel]);
        commit_at("2026-09-01T00:00:00Z", &format!("touch {rel}"));
    }
    for rel in [
        "justfile",
        "README.md",
        "AGENTS.md",
        "identity.toml",
        ".tt.toml",
    ] {
        std::fs::write(dir.join(rel), "changed\n").unwrap();
        git(&dir, &["add", rel]);
        commit_at("2026-09-01T00:00:00Z", &format!("touch {rel}"));
    }

    let (code, rows, stderr) = ledger(&env, &dir);
    assert_eq!(code, 0, "{stderr}");

    assert_eq!(
        row(&rows, &dead_refs)["dead_paths"],
        "GONE_ROOT.md,docs/notes/gone-brief.md,docs/plans/gone-plan.md,docs/specs/gone-spec.md"
    );
    assert_eq!(row(&rows, &dead_refs)["triggers"], "1");

    assert_eq!(row(&rows, &touched_refs)["dead_paths"], "");
    assert_eq!(row(&rows, &touched_refs)["touched_commits"], "3");
    assert_eq!(row(&rows, &touched_refs)["triggers"], "0");

    assert_eq!(row(&rows, &touched_roots)["dead_paths"], "");
    assert_eq!(row(&rows, &touched_roots)["touched_commits"], "5");
    assert_eq!(row(&rows, &touched_roots)["triggers"], "0");

    assert_eq!(row(&rows, &subpath_only)["dead_paths"], "src/justfile");
    assert_eq!(row(&rows, &subpath_only)["touched_commits"], "0");
}

#[test]
fn sweep_signals_twins_excludes_direct_parent_child_pairs() {
    let mut env = TestEnv::new();
    let dir = env.init("aa");

    let goal = add(
        &env,
        &dir,
        "Close out publication persistence certification",
        "goal body",
    );
    let child_a = id_of(env.json(
        &dir,
        &[
            "add",
            "Certify publication persistence certification",
            "--status",
            "todo",
            "--parent",
            &goal,
            "--body",
            "child a",
        ],
    ));
    let child_b = id_of(env.json(
        &dir,
        &[
            "add",
            "Verify publication persistence certification",
            "--status",
            "todo",
            "--parent",
            &goal,
            "--body",
            "child b",
        ],
    ));

    let (code, rows, stderr) = ledger(&env, &dir);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(row(&rows, &goal)["twins"], "");
    assert_eq!(row(&rows, &child_a)["twins"], child_b);
    assert_eq!(row(&rows, &child_b)["twins"], child_a);
}
