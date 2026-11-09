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

fn git_text(env: &TestEnv, root: &Path, args: &[&str]) -> String {
    let out = common::git_fixture::command(env.home.path(), root)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap().trim().into()
}

fn linked_snapshot(env: &TestEnv, layout: &StoreLayout) {
    let overlay = layout.store.join("aa");
    let checkout = layout.code.join("aa");
    fs::write(
        overlay.join("identity.toml"),
        "name = 'Overlay'\npurpose = 'Owns records'\n",
    )
    .unwrap();
    git(&layout.store, &["add", "-A"]);
    git(&layout.store, &["commit", "-qm", "overlay identity"]);
    let commit = git_text(env, &overlay, &["rev-parse", "HEAD"]);
    let blob = git_text(env, &overlay, &["rev-parse", "HEAD:./identity.toml"]);
    fs::write(overlay.join("tasks/.config.toml"), format!(
        "prefix = 'aa'\n[identity]\nname = 'Overlay'\npurpose = 'Owns records'\nsource = 'head'\ncommit = {commit:?}\nblob = {blob:?}\nrefreshed = '2026-11-09T16:00:00Z'\n[checkout]\npath = {:?}\n", checkout.to_str().unwrap()
    )).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry["projects"]["aa"] = overlay.to_str().unwrap().into();
    registry["records"].as_table_mut().unwrap().remove("aa");
    fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    git(&checkout, &["init", "-q", "-b", "main"]);
    fs::write(
        checkout.join("identity.toml"),
        "name = 'Foreign'\npurpose = 'Other code'\n",
    )
    .unwrap();
    git(&checkout, &["add", "-A"]);
    git(&checkout, &["commit", "-qm", "foreign identity"]);
}

#[test]
fn linked_freshness_compares_the_overlay_identity_not_the_foreign_checkout() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    linked_snapshot(&env, &layout);
    let out = resolve(&env, &layout.code, &["aa", "--freshness"]);
    assert_eq!(
        out["results"][0]["entry"]["checkout"],
        layout.code.join("aa").to_str().unwrap()
    );
    assert_eq!(out["results"][0]["entry"]["freshness"], "current");
}

#[test]
fn linked_freshness_stays_current_when_the_foreign_checkout_is_missing() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    linked_snapshot(&env, &layout);
    fs::remove_dir_all(layout.code.join("aa")).unwrap();
    let out = resolve(&env, &layout.code, &["aa", "--freshness"]);
    assert_eq!(out["results"][0]["entry"]["freshness"], "current");
}

#[test]
fn resolve_all_keeps_discovered_alias_entries_and_their_errors_separate() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[
            (
                "aa",
                EntrySpec {
                    registered: false,
                    extra: IDENTITY,
                },
            ),
            ("bb", registered()),
        ],
    );
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry.as_table_mut().unwrap().insert(
        "aliases".into(),
        toml::Value::Table(
            [(String::from("aa"), toml::Value::String("bb".into()))]
                .into_iter()
                .collect(),
        ),
    );
    fs::write(registry_path, toml::to_string(&registry).unwrap()).unwrap();
    for malformed in [false, true] {
        if malformed {
            fs::write(layout.store.join("aa/tasks/.config.toml"), "prefix = 1").unwrap();
        }
        let out = resolve(&env, &layout.code, &["--all"]);
        let rows = out["results"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["input"], "aa");
        assert_eq!(rows[0]["prefix"], "aa");
        assert_eq!(rows[0]["status"], "unregistered");
        assert!(rows[0].get("alias").is_none());
        if malformed {
            assert!(rows[0]["entry_error"].is_string());
            assert!(rows[0].get("entry").is_none());
        } else {
            assert_eq!(
                rows[0]["entry"]["records"],
                layout.store.join("aa").to_str().unwrap()
            );
        }
        assert_eq!(rows[1]["prefix"], "bb");
        assert_eq!(
            rows[1]["entry"]["records"],
            layout.store.join("bb").to_str().unwrap()
        );
        let explicit = resolve(&env, &layout.code, &["aa"]);
        assert_eq!(explicit["results"][0]["prefix"], "bb");
        assert_eq!(explicit["results"][0]["alias"], "aa");
        assert_eq!(
            explicit["results"][0]["entry"]["records"],
            layout.store.join("bb").to_str().unwrap()
        );
    }
}

#[test]
fn records_store_identifies_missing_tails_under_entries_stores_and_symlinks() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let alias = layout._temp.path().join("alias");
    std::os::unix::fs::symlink(&layout.store, &alias).unwrap();
    for (path, status) in [
        (layout.store.join("aa/tasks/new-file.md"), "resolved"),
        (
            layout.store.join("new-entry/tasks/new-file.md"),
            "unregistered",
        ),
        (alias.join("aa/tasks/new-file.md"), "resolved"),
        (alias.join("new-entry/tasks/new-file.md"), "unregistered"),
    ] {
        let out = resolve(&env, &layout.code, &[path.to_str().unwrap()]);
        assert_eq!(out["results"][0]["status"], status);
        assert_eq!(
            out["results"][0]["records_store"],
            layout.store.to_str().unwrap()
        );
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

fn root_identity(env: &TestEnv, layout: &StoreLayout, prefix: &str, text: &str) -> String {
    let root = layout.code.join(prefix);
    git(&root, &["init", "-q", "-b", "main"]);
    fs::write(root.join("identity.toml"), text).unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "identity"]);
    git_text(env, &root, &["rev-parse", "HEAD"])
}

fn refresh(env: &TestEnv, dir: &Path, args: &[&str], code: i32) -> Value {
    let out = env
        .cmd(dir)
        .args(["identity", "refresh"])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(code), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn config(layout: &StoreLayout, prefix: &str) -> PathBuf {
    layout.store.join(prefix).join("tasks/.config.toml")
}

fn snapshot(layout: &StoreLayout, prefix: &str) -> toml::Value {
    let doc: toml::Value =
        toml::from_str(&fs::read_to_string(config(layout, prefix)).unwrap()).unwrap();
    doc["identity"].clone()
}

#[test]
fn freshness_current_behind_unknown() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'A'\npurpose = 'First'\n");
    let root = layout.code.join("aa");
    refresh(&env, &root, &[], 0);
    assert_eq!(
        resolve(&env, &root, &["aa", "--freshness"])["results"][0]["entry"]["freshness"],
        "current"
    );
    fs::write(
        root.join("identity.toml"),
        "name = 'B'\npurpose = 'Second'\n",
    )
    .unwrap();
    git(&root, &["add", "identity.toml"]);
    git(&root, &["commit", "-qm", "new identity"]);
    assert_eq!(
        resolve(&env, &root, &["aa", "--freshness"])["results"][0]["entry"]["freshness"],
        "behind"
    );
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        resolve(&env, &layout.code, &["aa", "--freshness"])["results"][0]["entry"]["freshness"],
        "unknown"
    );
}

#[test]
fn a_stale_snapshot_warns_and_never_refuses() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'A'\npurpose = 'First'\n");
    let root = layout.code.join("aa");
    refresh(&env, &root, &[], 0);
    assert!(
        !env.check(&root)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "identity_behind")
    );
    fs::write(
        root.join("identity.toml"),
        "name = 'B'\npurpose = 'Second'\n",
    )
    .unwrap();
    git(&root, &["add", "identity.toml"]);
    git(&root, &["commit", "-qm", "new identity"]);
    let before = inventory(&layout.store);
    let out = env.check(&root);
    assert_eq!(out["errors"], json!([]));
    let warning = out["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "identity_behind")
        .expect("missing stale-snapshot warning");
    assert!(
        warning["detail"]
            .as_str()
            .unwrap()
            .contains("tasks identity refresh aa")
    );
    assert_eq!(inventory(&layout.store), before);
}

#[test]
fn check_freshness_uses_the_registered_overlay_even_from_foreign_code() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    linked_snapshot(&env, &layout);
    let checkout = layout.code.join("aa");
    assert!(
        !env.check(&checkout)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "identity_behind")
    );
    fs::write(
        layout.store.join("aa/identity.toml"),
        "name = 'Changed overlay'\npurpose = 'Owns records'\n",
    )
    .unwrap();
    git(&layout.store, &["add", "aa/identity.toml"]);
    git(&layout.store, &["commit", "-qm", "changed overlay"]);
    assert!(
        env.check(&checkout)["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "identity_behind")
    );
    fs::remove_dir_all(&checkout).unwrap();
    let out = env.check(&layout.store.join("aa"));
    for kind in ["identity_behind", "checkout_missing"] {
        assert!(
            out["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["kind"] == kind),
            "{out}"
        );
    }
}

#[test]
fn projects_rows_carry_name_and_purpose() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[
            ("aa", registered()),
            (
                "bb",
                EntrySpec {
                    registered: true,
                    extra: "",
                },
            ),
            ("cc", registered()),
        ],
    );
    fs::remove_dir_all(layout.store.join("cc")).unwrap();
    let out = env.json(&layout.code, &["projects"]);
    let rows = out["projects"].as_array().unwrap();
    assert_eq!(rows[0]["name"], "Catalogue");
    assert_eq!(rows[0]["purpose"], "Keeps identities");
    assert!(rows[0].get("aliases").is_none());
    for row in &rows[1..] {
        assert!(row.get("name").is_none());
        assert!(row.get("purpose").is_none());
    }
}

#[test]
fn init_refreshes_the_snapshot() {
    for kind in ["in-tree", "detached", "linked"] {
        let env = TestEnv::new();
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("root");
        let records = base.join("records");
        let foreign = base.join("foreign");
        for dir in [&root, &records, &foreign] {
            fs::create_dir(dir).unwrap();
            git(dir, &["init", "-q", "-b", "main"]);
            git(dir, &["commit", "--allow-empty", "-qm", "initial"]);
        }
        fs::write(
            root.join("identity.toml"),
            "prefix = 'aa'\nname = 'Root'\npurpose = 'Registered authority'\n",
        )
        .unwrap();
        git(&root, &["add", "identity.toml"]);
        git(&root, &["commit", "-qm", "identity"]);
        fs::write(
            foreign.join("identity.toml"),
            "prefix = 'bb'\nname = 'Foreign'\npurpose = 'Other code'\n",
        )
        .unwrap();
        git(&foreign, &["add", "identity.toml"]);
        git(&foreign, &["commit", "-qm", "foreign"]);
        let mut args = vec!["init", "--prefix", "aa"];
        match kind {
            "detached" => args.extend(["--records", records.to_str().unwrap()]),
            "linked" => args.extend(["--checkout", foreign.to_str().unwrap()]),
            _ => {}
        }
        let config = if kind == "detached" {
            records.join("aa/tasks/.config.toml")
        } else {
            root.join("tasks/.config.toml")
        };
        for force in [false, true] {
            let head = git_text(&env, &root, &["rev-parse", "HEAD"]);
            let records_head = git_text(&env, &records, &["rev-parse", "HEAD"]);
            let out = env.json(&root, &args);
            let doc: toml::Value = toml::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
            assert_eq!(
                doc["identity"]["name"].as_str(),
                Some(if force { "Changed" } else { "Root" }),
                "{kind}: {out}"
            );
            assert_eq!(doc["identity"]["commit"].as_str(), Some(head.as_str()));
            assert_eq!(git_text(&env, &root, &["rev-parse", "HEAD"]), head);
            assert_eq!(
                git_text(&env, &records, &["rev-parse", "HEAD"]),
                records_head
            );
            if !force {
                args.push("--force");
                fs::write(
                    root.join("identity.toml"),
                    "prefix = 'aa'\nname = 'Changed'\npurpose = 'Registered authority'\n",
                )
                .unwrap();
                git(&root, &["add", "identity.toml"]);
                git(&root, &["commit", "-qm", "changed identity"]);
            }
        }
    }
}

#[test]
fn init_over_a_mismatched_prefix_pairs_warns_and_writes_no_snapshot() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[(
            "aa",
            EntrySpec {
                registered: false,
                extra: "",
            },
        )],
    );
    root_identity(
        &env,
        &layout,
        "aa",
        "prefix = 'bb'\nname = 'Other'\npurpose = 'Mismatch'\n",
    );
    let root = layout.code.join("aa");
    let out = env.json(
        &root,
        &[
            "init",
            "--prefix",
            "aa",
            "--records",
            layout.store.to_str().unwrap(),
        ],
    );
    assert!(
        out["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("identity_prefix_mismatch")
            && w.as_str().unwrap().contains("tasks identity refresh aa")),
        "{out}"
    );
    let doc: toml::Value =
        toml::from_str(&fs::read_to_string(config(&layout, "aa")).unwrap()).unwrap();
    assert!(doc.get("identity").is_none());
    assert_eq!(
        resolve(&env, &root, &["aa"])["results"][0]["status"],
        "resolved"
    );
}

#[test]
fn init_refresh_failures_warn_and_preserve_the_previous_snapshot() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let root = layout.code.join("aa");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["commit", "--allow-empty", "-qm", "no identity"]);
    let before = snapshot(&layout, "aa");
    let args = [
        "init",
        "--prefix",
        "aa",
        "--records",
        layout.store.to_str().unwrap(),
        "--force",
    ];
    let out = env.json(&root, &args);
    assert!(
        !out["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("tasks identity refresh")),
        "{out}"
    );
    assert_eq!(snapshot(&layout, "aa"), before);
    fs::write(root.join("identity.toml"), "name = 1\n").unwrap();
    git(&root, &["add", "identity.toml"]);
    git(&root, &["commit", "-qm", "invalid identity"]);
    for kind in ["identity_invalid", "root_detached"] {
        if kind == "root_detached" {
            git(&root, &["checkout", "-q", "--detach"]);
        }
        let out = env.json(&root, &args);
        assert!(
            out["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str().unwrap().contains(kind)
                    && w.as_str().unwrap().contains("tasks identity refresh aa")),
            "{out}"
        );
        assert_eq!(snapshot(&layout, "aa"), before);
    }
}

#[test]
fn refresh_reads_the_registered_roots_head() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let commit = root_identity(
        &env,
        &layout,
        "aa",
        "prefix = 'aa'\nname = 'A'\npurpose = 'Committed'\naliases = ['Alpha']\n",
    );
    let root = layout.code.join("aa");
    let blob = git_text(&env, &root, &["rev-parse", "HEAD:identity.toml"]);
    let wt = layout.code.join("worktree");
    git(
        &root,
        &["worktree", "add", "-q", "-b", "other", wt.to_str().unwrap()],
    );
    common::git_fixture::assert_worktree(env.home.path(), &root, &wt);
    fs::write(
        wt.join("identity.toml"),
        "name = 'C'\npurpose = 'Other branch'\n",
    )
    .unwrap();
    git(&wt, &["add", "identity.toml"]);
    git(&wt, &["commit", "-qm", "other"]);
    fs::write(
        root.join("identity.toml"),
        "name = 'B'\npurpose = 'Dirty'\n",
    )
    .unwrap();
    let before = fs::read_to_string(config(&layout, "aa")).unwrap();
    let out = refresh(&env, &wt, &[], 0);
    let row = &out["results"][0];
    assert_eq!(row["prefix"], "aa");
    assert_eq!(row["outcome"], "refreshed");
    assert_eq!(row["commit"], commit);
    assert_eq!(row["snapshot"]["blob"], blob);
    let after = snapshot(&layout, "aa");
    assert_eq!(after["name"].as_str(), Some("A"));
    assert_eq!(after["purpose"].as_str(), Some("Committed"));
    assert_eq!(after["aliases"][0].as_str(), Some("Alpha"));
    assert_eq!(after["commit"].as_str(), Some(commit.as_str()));
    let after_text = fs::read_to_string(config(&layout, "aa")).unwrap();
    assert_eq!(
        before.split("[identity]").next(),
        after_text.split("[identity]").next()
    );
    assert!(
        env.pretty(&root, &["identity", "refresh", "aa"])
            .contains("unchanged")
    );
}

#[test]
fn refresh_refuses_a_mismatched_prefix_and_writes_nothing() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let commit = root_identity(
        &env,
        &layout,
        "aa",
        "prefix = 'bb'\nname = 'Wrong'\npurpose = 'P'\n",
    );
    let before = fs::read(config(&layout, "aa")).unwrap();
    let out = refresh(&env, &layout.code, &["aa"], 1);
    assert_eq!(out["results"][0]["outcome"], "error");
    assert!(
        out["results"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("identity_prefix_mismatch")
    );
    assert_eq!(out["results"][0]["commit"], commit);
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
}

#[test]
fn refresh_from_seeds_and_refuses_when_committed() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let root = layout.code.join("aa");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["commit", "-qm", "empty", "--allow-empty"]);
    let seed = layout.code.join("seed.toml");
    fs::write(&seed, "name = 'Seed'\npurpose = 'Initial'\n").unwrap();
    let args = ["aa", "--from", seed.to_str().unwrap()];
    let out = refresh(&env, &layout.code, &args, 0);
    assert_eq!(out["results"][0]["outcome"], "seeded");
    assert!(out["results"][0].get("branch").is_none());
    assert!(out["results"][0].get("commit").is_none());
    assert_eq!(snapshot(&layout, "aa")["source"].as_str(), Some("seeded"));
    // Same data is unchanged even when its old timestamp predates this invocation.
    let path = config(&layout, "aa");
    let old = fs::read_to_string(&path).unwrap();
    let timestamp = snapshot(&layout, "aa")["refreshed"]
        .as_str()
        .unwrap()
        .to_owned();
    fs::write(&path, old.replace(&timestamp, "2020-01-01T00:00:00Z")).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        refresh(&env, &layout.code, &args, 0)["results"][0]["outcome"],
        "unchanged"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    root_identity(
        &env,
        &layout,
        "aa",
        "name = 'Committed'\npurpose = 'Authority'\n",
    );
    assert!(
        refresh(&env, &layout.code, &args, 1)["results"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("identity_committed")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    // A malformed committed identity is still committed authority: seeding cannot hide it.
    root_identity(&env, &layout, "aa", "name = 1\n");
    assert!(
        refresh(&env, &layout.code, &args, 1)["results"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("identity_committed")
    );
    env.usage(
        &layout.code,
        &[
            "identity",
            "refresh",
            "--all",
            "--from",
            seed.to_str().unwrap(),
        ],
    );
    env.usage(
        &layout.code,
        &[
            "identity",
            "refresh",
            "aa",
            "bb",
            "--from",
            seed.to_str().unwrap(),
        ],
    );
}

#[test]
fn refresh_error_row_carries_branch_and_commit() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered()), ("bb", registered())]);
    let commit = root_identity(&env, &layout, "aa", "name = 1\npurpose = 'Malformed'\n");
    root_identity(&env, &layout, "bb", "name = 'Valid'\npurpose = 'P'\n");
    let before = fs::read(config(&layout, "aa")).unwrap();
    let out = refresh(&env, &layout.code, &["aa", "bb"], 1);
    let row = &out["results"][0];
    assert_eq!(row["outcome"], "error");
    assert_eq!(row["branch"], "main");
    assert_eq!(row["commit"], commit);
    assert!(row["detail"].as_str().unwrap().contains("identity_invalid"));
    assert!(row["detail"].as_str().unwrap().contains(&commit));
    assert_eq!(out["results"][1]["outcome"], "refreshed");
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
}

#[test]
fn refresh_rows_name_branch_and_commit() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let commit = root_identity(&env, &layout, "aa", "name = 'Feature'\npurpose = 'P'\n");
    let root = layout.code.join("aa");
    git(&root, &["checkout", "-qb", "feature"]);
    for outcome in ["refreshed", "unchanged"] {
        let out = refresh(&env, &layout.code, &["aa"], 0);
        assert_eq!(out["results"][0]["outcome"], outcome);
        assert_eq!(out["results"][0]["branch"], "feature");
        assert_eq!(out["results"][0]["commit"], commit);
    }
    let before = fs::read(config(&layout, "aa")).unwrap();
    git(&root, &["rm", "-q", "identity.toml"]);
    git(&root, &["commit", "-qm", "remove"]);
    let commit = git_text(&env, &root, &["rev-parse", "HEAD"]);
    let out = refresh(&env, &layout.code, &["aa"], 0);
    assert_eq!(out["results"][0]["outcome"], "no_identity");
    assert_eq!(out["results"][0]["branch"], "feature");
    assert_eq!(out["results"][0]["commit"], commit);
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
}

#[test]
fn refresh_on_detached_head_is_root_detached() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'N'\npurpose = 'P'\n");
    git(&layout.code.join("aa"), &["checkout", "-q", "--detach"]);
    let before = fs::read(config(&layout, "aa")).unwrap();
    let row = &refresh(&env, &layout.code, &["aa"], 1)["results"][0];
    assert_eq!(row["outcome"], "error");
    assert!(row["detail"].as_str().unwrap().contains("root_detached"));
    assert!(row.get("branch").is_none());
    assert!(row.get("commit").is_none());
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
}

#[test]
fn refresh_all_commits_once_per_repository() {
    let env = TestEnv::new();
    let spec = || EntrySpec {
        registered: true,
        extra: "[git]\ncommit = true\n",
    };
    let layout = store_env(&env, &[("aa", spec()), ("bb", spec())]);
    root_identity(&env, &layout, "aa", "name = 'A'\npurpose = 'P'\n");
    root_identity(&env, &layout, "bb", "name = 'B'\npurpose = 'P'\n");
    let before = git_text(&env, &layout.store, &["rev-parse", "HEAD"]);
    let out = refresh(&env, &layout.code, &["--all"], 0);
    assert_eq!(out["results"].as_array().unwrap().len(), 2);
    assert_eq!(out["warnings"], json!([]));
    let range = format!("{before}..HEAD");
    assert_eq!(
        git_text(&env, &layout.store, &["rev-list", "--count", &range]),
        "1"
    );
    assert_eq!(
        git_text(
            &env,
            &layout.store,
            &["show", "--format=%s", "--name-only", "HEAD"]
        ),
        "chore(tasks): refresh identity snapshot for aa, bb\n\naa/tasks/.config.toml\nbb/tasks/.config.toml"
    );
    assert!(git_text(&env, &layout.store, &["status", "--porcelain"]).is_empty());
}

#[test]
fn refresh_all_reports_a_gone_root_as_unreachable() {
    let env = TestEnv::new();
    let layout = store_env(
        &env,
        &[(
            "aa",
            EntrySpec {
                registered: true,
                extra: "[retired]\nreason = 'Retired'\nat = '2026-01-01T00:00:00Z'\n",
            },
        )],
    );
    fs::remove_dir(layout.code.join("aa")).unwrap();
    let out = refresh(&env, &layout.code, &["--all"], 0);
    assert_eq!(out["results"][0]["prefix"], "aa");
    assert_eq!(out["results"][0]["outcome"], "unreachable");
    assert!(out["results"][0].get("branch").is_none());
}

#[test]
fn refresh_a_linked_project_from_its_overlay() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    linked_snapshot(&env, &layout);
    let overlay = layout.store.join("aa");
    fs::write(
        overlay.join("identity.toml"),
        "name = 'Updated overlay'\npurpose = 'Authority'\n",
    )
    .unwrap();
    git(&layout.store, &["add", "aa/identity.toml"]);
    git(&layout.store, &["commit", "-qm", "overlay update"]);
    let out = refresh(&env, &layout.code.join("aa"), &[], 0);
    assert_eq!(out["results"][0]["outcome"], "refreshed");
    assert_eq!(
        snapshot(&layout, "aa")["name"].as_str(),
        Some("Updated overlay")
    );
    assert_eq!(
        snapshot(&layout, "aa")["blob"].as_str(),
        Some(git_text(&env, &overlay, &["rev-parse", "HEAD:./identity.toml"]).as_str())
    );
}

#[test]
fn refresh_without_git_commit_writes_and_leaves_it_uncommitted() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'Uncommitted'\npurpose = 'P'\n");
    let before = git_text(&env, &layout.store, &["rev-parse", "HEAD"]);
    refresh(&env, &layout.code, &["aa"], 0);
    assert_eq!(
        snapshot(&layout, "aa")["name"].as_str(),
        Some("Uncommitted")
    );
    assert_eq!(
        git_text(&env, &layout.store, &["rev-parse", "HEAD"]),
        before
    );
    assert_eq!(
        git_text(&env, &layout.store, &["status", "--porcelain"]).trim(),
        "M aa/tasks/.config.toml"
    );
}

#[test]
fn refresh_keeps_the_snapshot_when_only_head_changes() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'N'\npurpose = 'P'\n");
    refresh(&env, &layout.code, &["aa"], 0);
    let path = config(&layout, "aa");
    let old = fs::read_to_string(&path).unwrap();
    let timestamp = snapshot(&layout, "aa")["refreshed"]
        .as_str()
        .unwrap()
        .to_owned();
    fs::write(&path, old.replace(&timestamp, "2020-01-01T00:00:00Z")).unwrap();
    let before = fs::read(&path).unwrap();
    let root = layout.code.join("aa");
    git(&root, &["commit", "-qm", "unrelated", "--allow-empty"]);
    let out = refresh(&env, &layout.code, &["aa"], 0);
    assert_eq!(out["results"][0]["outcome"], "unchanged");
    assert_eq!(
        out["results"][0]["commit"],
        git_text(&env, &root, &["rev-parse", "HEAD"])
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn refresh_from_resolves_a_relative_file_under_chdir() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    let root = layout.code.join("aa");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["commit", "-qm", "empty", "--allow-empty"]);
    fs::write(
        root.join("seed.toml"),
        "name = 'Local seed'\npurpose = 'P'\n",
    )
    .unwrap();
    let out = env.json(
        &layout.code,
        &[
            "-C",
            root.to_str().unwrap(),
            "identity",
            "refresh",
            "--from",
            "seed.toml",
        ],
    );
    assert_eq!(out["results"][0]["outcome"], "seeded");
    assert_eq!(snapshot(&layout, "aa")["name"].as_str(), Some("Local seed"));
}

#[test]
fn refresh_waits_for_the_project_mutation_lock_before_writing() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'Updated'\npurpose = 'P'\n");
    let path = config(&layout, "aa");
    let before = fs::read(&path).unwrap();
    let lock = common::hold_project_lock(&env, "aa");
    let mut child = env
        .raw(&layout.code)
        .args(["identity", "refresh", "aa"])
        .spawn()
        .unwrap();
    let finished_while_locked = common::wait_bounded(&mut child, Duration::from_millis(500));
    let while_locked = fs::read(&path).unwrap();
    drop(lock);
    let out = common::reap(child, Duration::from_secs(5))
        .expect("refresh did not resume after releasing project lock");
    assert!(
        !finished_while_locked,
        "refresh ignored the held project lock: {out:?}"
    );
    assert_eq!(
        while_locked, before,
        "snapshot changed under another writer's project lock"
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(snapshot(&layout, "aa")["name"].as_str(), Some("Updated"));
}

#[test]
fn refresh_rejects_a_pending_rename_without_breaking_its_inventory() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered())]);
    root_identity(&env, &layout, "aa", "name = 'Updated'\npurpose = 'P'\n");
    let stopped = env
        .raw(&layout.code.join("aa"))
        .env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "aa", "bb"])
        .output()
        .unwrap();
    assert!(stopped.status.success(), "{stopped:?}");
    let inventory = env.home.path().join(".local/state/tasks/rename/aa.toml");
    let inventory_before = fs::read(&inventory).expect("rename saved its inventory");
    let before = fs::read(config(&layout, "aa")).unwrap();
    let out = env
        .cmd(&layout.code)
        .args(["identity", "refresh", "aa"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = &value["results"][0];
    assert_eq!(row["outcome"], "error");
    assert!(
        row["detail"].as_str().unwrap().contains("validation:"),
        "{row}"
    );
    assert!(
        row["detail"].as_str().unwrap().contains("unfinished"),
        "{row}"
    );
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
    assert_eq!(fs::read(&inventory).unwrap(), inventory_before);
    let resumed = env.json(&layout.code.join("aa"), &["rename", "aa", "bb"]);
    assert_eq!(resumed["prefix"], "bb");
    assert!(!inventory.exists(), "rename did not finish recovery");
    assert!(config(&layout, "bb").exists());
}

#[test]
fn refresh_refuses_an_alias_that_changed_while_waiting_for_its_project_lock() {
    let env = TestEnv::new();
    let layout = store_env(&env, &[("aa", registered()), ("bb", registered())]);
    root_identity(&env, &layout, "aa", "name = 'A'\npurpose = 'P'\n");
    root_identity(&env, &layout, "bb", "name = 'B'\npurpose = 'P'\n");
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry
        .as_table_mut()
        .unwrap()
        .insert("aliases".into(), toml::toml! { old = "aa" }.into());
    fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    let before_a = fs::read(config(&layout, "aa")).unwrap();
    let before_b = fs::read(config(&layout, "bb")).unwrap();
    let lock = common::hold_project_lock(&env, "aa");
    let lock_path = env.claim_store("aa").with_file_name("aa.lock");
    let mut child = env
        .raw(&layout.code)
        .args(["identity", "refresh", "old"])
        .spawn()
        .unwrap();
    // Observing its open lock fd makes the registration change deterministic:
    // the child has resolved old -> aa and is waiting on our held lock.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let reached_lock = loop {
        let awaiting = fs::read_dir(format!("/proc/{}/fd", child.id())).is_ok_and(|fds| {
            fds.filter_map(std::result::Result::ok)
                .any(|fd| fs::read_link(fd.path()).is_ok_and(|path| path == lock_path))
        });
        if awaiting {
            break true;
        }
        if child.try_wait().unwrap().is_some() || std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if reached_lock {
        registry["aliases"]["old"] = "bb".into();
        fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    }
    drop(lock);
    let out =
        common::reap(child, Duration::from_secs(5)).expect("refresh did not leave its stale lock");
    assert!(
        reached_lock,
        "refresh never waited on the resolved project lock: {out:?}"
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = &value["results"][0];
    assert_eq!(row["outcome"], "error");
    assert!(row["detail"].as_str().unwrap().contains("config:"), "{row}");
    assert!(
        row["detail"]
            .as_str()
            .unwrap()
            .contains("changed while acquiring"),
        "{row}"
    );
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before_a);
    assert_eq!(fs::read(config(&layout, "bb")).unwrap(), before_b);
}

#[test]
fn refresh_reports_a_project_lock_error_and_refreshes_other_prefixes() {
    let env = TestEnv::new();
    let spec = || EntrySpec {
        registered: true,
        extra: "[git]\ncommit = true\n",
    };
    let layout = store_env(&env, &[("aa", spec()), ("bb", spec())]);
    root_identity(&env, &layout, "aa", "name = 'A'\npurpose = 'P'\n");
    root_identity(&env, &layout, "bb", "name = 'B'\npurpose = 'P'\n");
    let before = fs::read(config(&layout, "aa")).unwrap();
    let lock_path = env.claim_store("aa").with_file_name("aa.lock");
    fs::create_dir_all(&lock_path).unwrap();
    let out = env
        .cmd(&layout.code)
        .args(["identity", "refresh", "aa", "bb"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout)
        .expect("a project lock failure must return per-prefix results");
    assert_eq!(value["results"].as_array().unwrap().len(), 2);
    assert_eq!(value["results"][0]["prefix"], "aa");
    assert_eq!(value["results"][0]["outcome"], "error");
    assert!(
        value["results"][0]["detail"]
            .as_str()
            .unwrap()
            .starts_with("io:")
    );
    assert!(value["results"][0].get("commit").is_none());
    assert_eq!(fs::read(config(&layout, "aa")).unwrap(), before);
    assert_eq!(value["results"][1]["prefix"], "bb");
    assert_eq!(value["results"][1]["outcome"], "refreshed");
    assert_eq!(snapshot(&layout, "bb")["name"].as_str(), Some("B"));
    assert_eq!(value["warnings"], json!([]));
    assert_eq!(
        git_text(
            &env,
            &layout.store,
            &["show", "--format=", "--name-only", "HEAD"]
        ),
        "bb/tasks/.config.toml"
    );
    assert!(git_text(&env, &layout.store, &["status", "--porcelain"]).is_empty());
}
