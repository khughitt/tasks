mod common;

use common::TestEnv;
use std::fs;
use std::path::{Path, PathBuf};

fn config_path(env: &TestEnv) -> PathBuf {
    env.home.path().join(".config/tasks/config.toml")
}

fn write_config(env: &TestEnv, text: impl AsRef<[u8]>) -> PathBuf {
    let path = config_path(env);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn config_failure(env: &TestEnv, root: &Path, path: &Path) {
    let out = env.cmd(root).args(["--json", "projects"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(matches!(
        error["error"]["kind"].as_str(),
        Some("config" | "io")
    ));
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(path.to_str().unwrap())
    );
}

#[test]
fn host_preferences_file_boundaries() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    env.json(&root, &["projects"]); // Missing optional file.
    for bytes in [
        b"bad [toml".as_slice(),
        b"[unknown]\nvalue = true\n",
        b"\xff",
    ] {
        let path = write_config(&env, bytes);
        config_failure(&env, &root, &path);
    }
    let path = config_path(&env);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    config_failure(&env, &root, &path);
    fs::remove_dir(&path).unwrap();
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
    env.json(&root, &["projects"]); // Missing parent is optional too.

    #[cfg(unix)]
    {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let target = env.home.path().join("host.toml");
        fs::write(&target, "[identity]\nrelay = false\n").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        env.json(&root, &["projects"]);
        fs::remove_file(&target).unwrap();
        config_failure(&env, &root, &path);
    }
}

#[cfg(unix)]
#[test]
fn host_preferences_unreadable_file_fails() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let path = write_config(&env, "[identity]\nrelay = false\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0)).unwrap();
    if fs::read(&path).is_ok() {
        eprintln!("SKIP: this user bypasses file permissions (root or equivalent)");
        return;
    }
    config_failure(&env, &root, &path);
}

#[test]
fn host_preferences_early_and_local_paths_skip_config() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    write_config(&env, "bad [toml");
    for args in [vec!["--help"], vec!["--version"], vec!["list", "--help"]] {
        env.cmd(&root).args(args).assert().success();
    }
    assert!(!env.complete(&root, "bash", 1, &["tasks", ""]).is_empty());
    let out = env.json(&root, &["attachments", "tasks/files/sci-000000/a.txt"]);
    assert_eq!(out["version"], 1);
    assert_eq!(out["attachments"][0]["eligible"], false);
}

fn has_headers(text: &str) -> bool {
    text.lines()
        .any(|line| line.split_whitespace().next() == Some("id"))
}

#[test]
fn header_preferences_follow_cli_env_config_default() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    env.json(&root, &["add", "A task"]);
    for (config, mirror, flag, want) in [
        (None, None, None, false),
        (Some(true), None, None, true),
        (Some(false), Some("true"), None, true),
        (Some(true), Some("false"), None, false),
        (Some(false), Some("false"), Some("--headers"), true),
        (Some(true), Some("true"), Some("--no-headers"), false),
    ] {
        if let Some(value) = config {
            write_config(&env, format!("[output]\nheaders = {value}\n"));
        }
        let mut cmd = env.cmd(&root);
        cmd.args(["--pretty", "list"]);
        if let Some(value) = mirror {
            cmd.env("TASKS_HEADERS", value);
        }
        if let Some(flag) = flag {
            cmd.arg(flag);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(
            has_headers(std::str::from_utf8(&out.stdout).unwrap()),
            want,
            "config={config:?}, env={mirror:?}, flag={flag:?}: {out:?}"
        );
    }
}

#[test]
fn header_preferences_conflict_across_command_boundaries() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let count = fs::read_dir(root.join("tasks")).unwrap().count();
    for (first, second) in [("--headers", "--no-headers"), ("--no-headers", "--headers")] {
        let error = env.usage(&root, &[first, "add", "Must not be created", second]);
        assert!(error.contains("cannot be used with"), "{error}");
        assert!(
            error.contains("--headers") && error.contains("--no-headers"),
            "{error}"
        );
        assert_eq!(fs::read_dir(root.join("tasks")).unwrap().count(), count);
    }
}

#[test]
fn header_preferences_validate_before_writes() {
    use std::collections::BTreeMap;
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let id = env.json(&root, &["add", "Original"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let snapshot = || -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(root.join("tasks"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    };
    let before = snapshot();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let registry_before = fs::read(&registry).unwrap();
    for args in [
        vec!["add", "Blocked"],
        vec!["edit", &id, "--body", "Changed"],
    ] {
        for (config, mirror) in [
            ("[output]\nheaders = true\n", Some("yes")),
            ("[output]\nheaders = 'true'\n", None),
        ] {
            write_config(&env, config);
            let mut cmd = env.cmd(&root);
            cmd.arg("--headers").args(&args);
            if let Some(value) = mirror {
                cmd.env("TASKS_HEADERS", value);
            }
            let out = cmd.output().unwrap();
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stdout.is_empty());
            let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
            assert_eq!(error["error"]["kind"], "config");
            assert_eq!(snapshot(), before);
            assert_eq!(fs::read(&registry).unwrap(), registry_before);
        }
    }
}

#[test]
fn header_preferences_leave_json_and_nontables_unchanged() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let id = env.json(&root, &["add", "Original"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let json = env.json(&root, &["list"]);
    let text = env.pretty(&root, &["root", &id]);
    write_config(&env, "[output]\nheaders = true\n");
    assert_eq!(env.json(&root, &["list"]), json);
    assert_eq!(env.json(&root, &["--no-headers", "list"]), json);
    assert_eq!(env.pretty(&root, &["root", &id]), text);
    let out = env
        .cmd(&root)
        .env("TASKS_HEADERS", "yes")
        .args(["--json", "list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}
