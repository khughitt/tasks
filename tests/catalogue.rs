mod common;
use common::{TestEnv, git};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

struct EntrySpec {
    registered: bool,
    extra: &'static str,
}

const IDENTITY: &str = "profile = 'personal'\nshorthand = ['cat']\ncolor = '#123456'\n[identity]\nname = 'Catalogue'\naliases = []\npurpose = 'Keeps identities'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'\n";

struct StoreLayout {
    _temp: tempfile::TempDir,
    store: PathBuf,
    code: PathBuf,
}

fn store_env(env: &TestEnv, entries: &[(&str, EntrySpec)]) -> StoreLayout {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let store = base.join("records");
    let code = base.join("code");
    fs::create_dir(&store).unwrap();
    fs::create_dir(&code).unwrap();
    git(&store, &["init", "-q", "-b", "main"]);
    let mut registry = toml::map::Map::new();
    let mut projects = toml::map::Map::new();
    let mut records = toml::map::Map::new();
    for (prefix, spec) in entries {
        let root = code.join(prefix);
        let entry = store.join(prefix);
        fs::create_dir(&root).unwrap();
        fs::create_dir_all(entry.join("tasks")).unwrap();
        fs::write(
            entry.join("tasks/.config.toml"),
            format!(
                "prefix = {prefix:?}\n{}\n[checkout]\npath = {:?}\nkind = 'detached'\n",
                spec.extra,
                root.to_str().unwrap()
            ),
        )
        .unwrap();
        if spec.registered {
            projects.insert(prefix.to_string(), root.to_str().unwrap().into());
            records.insert(prefix.to_string(), entry.to_str().unwrap().into());
        }
    }
    registry.insert("projects".into(), projects.into());
    registry.insert("records".into(), records.into());
    let config = env.home.path().join(".config/tasks");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("projects.toml"),
        toml::to_string(&registry).unwrap(),
    )
    .unwrap();
    git(&store, &["add", "-A"]);
    git(&store, &["commit", "-qm", "entries"]);
    StoreLayout {
        _temp: temp,
        store,
        code,
    }
}

fn registered() -> EntrySpec {
    EntrySpec {
        registered: true,
        extra: IDENTITY,
    }
}

fn resolve(env: &TestEnv, dir: &Path, args: &[&str]) -> Value {
    let mut command = vec!["resolve"];
    command.extend_from_slice(args);
    env.json(dir, &command)
}

#[test]
fn unregistered_entry_appears_in_resolve_all_and_by_prefix() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[
            ("aa", registered()),
            (
                "bb",
                EntrySpec {
                    registered: false,
                    extra: IDENTITY,
                },
            ),
        ],
    );
    let out = resolve(&env, &layout.code, &["--all"]);
    assert_eq!(
        out["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["prefix"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["aa", "bb"]
    );
    let bb = &out["results"][1];
    assert_eq!(bb["input"], "bb");
    assert_eq!(bb["kind"], "prefix");
    assert_eq!(bb["status"], "unregistered");
    assert!(bb.get("root").is_none());
    assert_eq!(
        bb["entry"],
        json!({
            "config": layout.store.join("bb/tasks/.config.toml"),
            "records": layout.store.join("bb"), "checkout": layout.code.join("bb"),
            "kind": "detached", "name": "Catalogue", "aliases": [],
            "purpose": "Keeps identities", "snapshot": {"source":"seeded","refreshed":"2026-11-09T16:00:00Z"},
            "shorthand": ["cat"], "profile":"personal", "color":"#123456"
        })
    );
    assert_eq!(resolve(&env, &layout.code, &["bb"])["results"][0], *bb);
    assert!(
        env.pretty(&layout.code, &["resolve", "bb"])
            .contains("Catalogue")
    );
    for args in [
        vec!["resolve", "--all", "aa"],
        vec!["resolve", "--all", "--stdin"],
    ] {
        env.usage(&layout.code, &args);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(inventory(&path));
        } else {
            files.insert(
                path.clone(),
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(&path).unwrap().modified().unwrap(),
                ),
            );
        }
    }
    files
}

#[test]
fn resolve_all_reports_an_invalid_directory_name_as_a_prefix_entry_error() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    fs::create_dir_all(layout.store.join(".broken/tasks")).unwrap();
    fs::write(
        layout.store.join(".broken/tasks/.config.toml"),
        "prefix = 1",
    )
    .unwrap();
    let out = resolve(&env, &layout.code, &["--all"]);
    assert_eq!(out["results"][0]["input"], ".broken");
    assert_eq!(out["results"][0]["kind"], "prefix");
    assert_eq!(out["results"][0]["status"], "unregistered");
    assert!(out["results"][0]["entry_error"].is_string());
}

#[test]
fn resolve_returns_while_the_commit_lock_is_held_and_writes_nothing() {
    use sha2::{Digest, Sha256};
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let common = layout.store.join(".git").canonicalize().unwrap();
    let hash = format!("{:x}", Sha256::digest(common.to_string_lossy().as_bytes()));
    let lock_path = env
        .claim_store("aa")
        .with_file_name(format!(".commit-{}.lock", &hash[..16]));
    fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let lock = fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.lock().unwrap();
    let _project_lock = common::hold_project_lock(&env, "aa");
    let before = inventory(&layout.store);
    let before_home = inventory(env.home.path());
    let mut child = env
        .raw(&layout.code)
        .args(["resolve", "--all"])
        .spawn()
        .unwrap();
    let finished = common::wait_bounded(&mut child, Duration::from_secs(1));
    if !finished {
        child.kill().unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(finished, "resolve blocked on a mutation lock");
    assert!(out.status.success(), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["results"][0]["entry"]["name"], "Catalogue");
    assert_eq!(inventory(&layout.store), before);
    assert_eq!(inventory(env.home.path()), before_home);
    let status = common::git_fixture::command(env.home.path(), &layout.store)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(status.stdout.is_empty(), "{status:?}");
}

#[test]
fn a_malformed_task_record_leaves_resolve_intact() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    fs::write(
        layout.store.join("aa/tasks/aa-123456.md"),
        "---\nstatus: [broken\n",
    )
    .unwrap();
    let out = resolve(&env, &layout.code, &["--all"]);
    assert_eq!(out["results"][0]["entry"]["name"], "Catalogue");
    assert_eq!(out["warnings"], json!([]));
}

#[test]
fn a_malformed_entry_gives_entry_error_and_others_resolve() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered()), ("bb", registered())]);
    fs::write(layout.store.join("bb/tasks/.config.toml"), "prefix = 1").unwrap();
    let out = resolve(&env, &layout.code, &["--all"]);
    assert_eq!(out["results"][0]["entry"]["name"], "Catalogue");
    let bad = &out["results"][1];
    assert_eq!(bad["status"], "resolved");
    assert!(bad.get("entry").is_none());
    let error = bad["entry_error"].as_str().unwrap();
    assert!(
        error.contains("bb") && error.contains("tasks/.config.toml"),
        "{error}"
    );
    assert!(
        out["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains(error))
    );
}

#[test]
fn records_store_on_store_paths_only() {
    let mut env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let in_tree = env.init("cc");
    let alias = layout._temp.path().join("alias");
    std::os::unix::fs::symlink(&layout.store, &alias).unwrap();
    for path in [
        &layout.store,
        &layout.store.join("aa"),
        &alias,
        &alias.join("aa/tasks"),
    ] {
        let out = resolve(&env, &layout.code, &[path.to_str().unwrap()]);
        assert_eq!(
            out["results"][0]["records_store"],
            layout.store.to_str().unwrap()
        );
    }
    for path in [&layout.code, &in_tree] {
        let out = resolve(&env, &layout.code, &[path.to_str().unwrap()]);
        assert!(out["results"][0].get("records_store").is_none());
    }
}

#[test]
fn registered_entry_wins_with_entry_conflict_warning() {
    let mut env = TestEnv::new();
    let layout = store_env(
        &env,
        &[
            ("aa", registered()),
            (
                "bb",
                EntrySpec {
                    registered: false,
                    extra: IDENTITY,
                },
            ),
        ],
    );
    let selected = env.init("bb");
    let out = resolve(&env, &layout.code, &["bb"]);
    assert_eq!(
        out["results"][0]["entry"]["records"],
        selected.to_str().unwrap()
    );
    assert!(out["results"][0]["entry"].get("name").is_none());
    assert!(
        out["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("entry_conflict"))
    );
    fs::write(selected.join("tasks/.config.toml"), "prefix = 1").unwrap();
    let out = resolve(&env, &layout.code, &["bb"]);
    assert!(out["results"][0].get("entry").is_none());
    assert!(out["results"][0]["entry_error"].is_string());
}

#[test]
fn duplicate_unregistered_prefix_is_one_entry_error_row() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[
            ("aa", registered()),
            (
                "bb",
                EntrySpec {
                    registered: false,
                    extra: IDENTITY,
                },
            ),
        ],
    );
    let other = layout._temp.path().join("other");
    fs::create_dir_all(other.join(".git")).unwrap();
    for prefix in ["bb", "cc"] {
        fs::create_dir_all(other.join(prefix).join("tasks")).unwrap();
        fs::write(
            other.join(prefix).join("tasks/.config.toml"),
            format!("prefix = {prefix:?}\n"),
        )
        .unwrap();
    }
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]
        .as_table_mut()
        .unwrap()
        .insert("cc".into(), other.join("cc").to_str().unwrap().into());
    fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let out = resolve(&env, &layout.code, &["--all"]);
    let bb = out["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["prefix"] == "bb")
        .collect::<Vec<_>>();
    assert_eq!(bb.len(), 1);
    assert_eq!(bb[0]["status"], "unregistered");
    assert!(bb[0].get("entry").is_none());
    let error = bb[0]["entry_error"].as_str().unwrap();
    assert!(
        error.contains(layout.store.join("bb").to_str().unwrap())
            && error.contains(other.join("bb").to_str().unwrap())
    );
}

#[test]
fn existing_fields_and_statuses_are_unchanged() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let root = layout.code.join("aa");
    let out = resolve(
        &env,
        &layout.code,
        &["aa-123456", "aa", root.to_str().unwrap(), "unknown", "?"],
    );
    for (i, kind) in ["id", "prefix", "path"].iter().enumerate() {
        let row = &out["results"][i];
        let old: Value = [
            "input", "kind", "status", "prefix", "alias", "root", "records", "color",
        ]
        .into_iter()
        .filter_map(|k| row.get(k).map(|v| (k.to_string(), v.clone())))
        .collect();
        let input = ["aa-123456", "aa", root.to_str().unwrap()][i];
        assert_eq!(
            old,
            json!({"input": input, "kind":kind, "status":"resolved", "prefix":"aa", "root":root,"records":layout.store.join("aa"),"color":"#123456"})
        );
    }
    assert_eq!(out["results"][3]["status"], "unregistered");
    assert_eq!(out["results"][4]["status"], "invalid");
}

#[test]
fn no_git_runs_without_freshness() {
    use std::os::unix::fs::PermissionsExt;
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    git(&layout.code.join("aa"), &["init", "-q", "-b", "main"]);
    git(
        &layout.code.join("aa"),
        &["commit", "--allow-empty", "-qm", "initial"],
    );
    let bin = layout._temp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let log = layout._temp.path().join("git.log");
    let shim = bin.join("git");
    fs::write(
        &shim,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_CALL_LOG\"\nexec /usr/bin/git \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    for args in [
        vec!["resolve", "--all"],
        vec!["resolve", "aa"],
        vec!["resolve", "--all", "--freshness"],
    ] {
        let out = env
            .cmd(&layout.code)
            .env("PATH", &path)
            .env("GIT_CALL_LOG", &log)
            .args(&args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        if args.contains(&"--freshness") {
            assert!(fs::metadata(&log).unwrap().len() > 0);
            assert_eq!(value["results"][0]["entry"]["freshness"], "current");
        } else {
            assert!(!log.exists(), "git ran without --freshness");
        }
    }
}

#[test]
fn check_reports_a_malformed_identity_table_as_a_config_error() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\n[identity]\npurpose = 'A catalogue'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["check"]), "config");
}

#[test]
fn profile_must_follow_the_tag_grammar() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\nprofile = 'Personal Work'\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["check"]), "config");
}

#[test]
fn shorthand_entries_are_nonempty_single_lines() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    for entry in ["", "Two\nlines"] {
        let entry = toml::Value::String(entry.into());
        std::fs::write(
            dir.join("tasks/.config.toml"),
            format!("prefix = 'cat'\nshorthand = [{entry}]\n"),
        )
        .unwrap();
        assert_eq!(env.fail(&dir, &["check"]), "config");
    }
}

#[test]
fn rename_carries_profile_shorthand_and_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\nprofile = 'personal'\nshorthand = ['x']\n\n[identity]\nname = 'Catalogue'\naliases = ['Public catalogue']\npurpose = 'Keeps project identities'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'\n",
    )
    .unwrap();
    let before: toml::Value = toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.json(&dir, &["rename", "cat", "cats"]);
    let after: toml::Value = toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
    assert_eq!(after["prefix"].as_str(), Some("cats"));
    for key in ["profile", "shorthand", "identity"] {
        assert_eq!(after[key], before[key], "rename changed {key}");
    }
}
