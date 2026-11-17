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

/// Rewrites the record's `updated:` line to an early date.
fn backdate(dir: &Path, id: &str) {
    let path = dir.join("tasks").join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let rewritten: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("updated:") {
                "updated: 2026-01-01T00:00:00Z".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&path, rewritten.join("\n") + "\n").unwrap();
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
