#![allow(dead_code)]
use assert_cmd::Command;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestEnv {
    pub home: TempDir,
    dirs: Vec<TempDir>,
}

impl TestEnv {
    pub fn new() -> TestEnv {
        TestEnv {
            home: tempfile::tempdir().unwrap(),
            dirs: Vec::new(),
        }
    }

    pub fn cmd(&self, dir: &Path) -> Command {
        let mut c = Command::cargo_bin("tasks").unwrap();
        c.env("HOME", self.home.path())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("TASKS_FORMAT")
            .env_remove("TASKS_OWNER")
            .env("USER", "tester")
            .current_dir(dir);
        c
    }

    /// New temp project directory, `tasks init --prefix <prefix>` already run.
    pub fn init(&mut self, prefix: &str) -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        self.dirs.push(dir);
        self.json(&path, &["init", "--prefix", prefix]);
        path
    }

    pub fn json(&self, dir: &Path, args: &[&str]) -> serde_json::Value {
        let out = self.cmd(dir).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "tasks {:?} failed:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "bad json from {args:?}: {e}\n{}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    pub fn fail(&self, dir: &Path, args: &[&str]) -> String {
        let out = self.cmd(dir).args(args).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(1),
            "tasks {:?} should exit 1:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stdout.is_empty(), "stdout must be empty on error");
        let v: serde_json::Value =
            serde_json::from_slice(&out.stderr).expect("json error on stderr");
        v["error"]["kind"].as_str().expect("error.kind").to_string()
    }

    pub fn read(&self, dir: &Path, rel: &str) -> String {
        std::fs::read_to_string(dir.join(rel)).unwrap()
    }
}
