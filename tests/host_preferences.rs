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
