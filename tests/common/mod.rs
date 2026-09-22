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
            .env_remove("XDG_STATE_HOME")
            .env_remove("RELAY_STATE_DIR")
            .env_remove("TASKS_FORMAT")
            .env_remove("TASKS_COMPLETE")
            .env_remove("TASKS_OWNER")
            .env_remove("TASKS_SESSION")
            .env_remove("TASKS_SESSION_PID")
            .env_remove("TASKS_MODEL")
            .env_remove("TASKS_AGENT")
            .env_remove("TASKS_MAX_COMPLEXITY")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("CLAUDE_PID")
            .env_remove("CODEX_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("TASKS_COLOR")
            .env_remove("NO_COLOR")
            .env("USER", "tester")
            .current_dir(dir);
        c
    }

    pub fn raw(&self, dir: &Path) -> std::process::Command {
        let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin("tasks"));
        c.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("HOME", self.home.path())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_STATE_HOME")
            .env_remove("RELAY_STATE_DIR")
            .env_remove("TASKS_FORMAT")
            .env_remove("TASKS_COMPLETE")
            .env_remove("TASKS_OWNER")
            .env_remove("TASKS_SESSION")
            .env_remove("TASKS_SESSION_PID")
            .env_remove("TASKS_MODEL")
            .env_remove("TASKS_AGENT")
            .env_remove("TASKS_MAX_COMPLEXITY")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("CLAUDE_PID")
            .env_remove("CODEX_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("TASKS_COLOR")
            .env_remove("NO_COLOR")
            .env("USER", "tester")
            .current_dir(dir);
        c
    }

    pub fn claim_store(&self, prefix: &str) -> PathBuf {
        self.home
            .path()
            .join(format!(".local/state/tasks/claims/{prefix}.toml"))
    }

    /// New temp project directory, `tasks init --prefix <prefix>` already run.
    pub fn init(&mut self, prefix: &str) -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        self.dirs.push(dir);
        self.json(&path, &["init", "--prefix", prefix]);
        path
    }

    /// A second project root under an already-registered prefix — a worktree, as far as a
    /// prefix-keyed claim store is concerned.
    pub fn init_forced(&mut self, prefix: &str) -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        self.dirs.push(dir);
        self.json(&path, &["init", "--prefix", prefix, "--force"]);
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

    /// `tasks check` as its report. A clean check prints nothing, which is the empty
    /// report; findings print as JSON. Exit 1 (errors) is the caller's to assert on `cmd`.
    pub fn check(&self, dir: &Path) -> serde_json::Value {
        let out = self.cmd(dir).args(["check"]).output().unwrap();
        assert!(
            out.status.success(),
            "tasks check failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.stdout.is_empty() {
            return serde_json::json!({"errors": [], "warnings": []});
        }
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "bad json from check: {e}\n{}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    pub fn pretty(&self, dir: &Path, args: &[&str]) -> String {
        let mut all = vec!["--pretty"];
        all.extend_from_slice(args);
        let out = self.cmd(dir).args(&all).output().unwrap();
        assert!(
            out.status.success(),
            "tasks --pretty {:?} failed:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
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

    /// A usage error: exit 2, nothing on stdout, and the problem plus at most the usage
    /// line on stderr. Returns stderr for the caller to check the option and value named.
    pub fn usage(&self, dir: &Path, args: &[&str]) -> String {
        let out = self.cmd(dir).args(args).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "tasks {:?} should exit 2:\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stdout.is_empty(),
            "stdout must be empty on a usage error"
        );
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(
            err.lines().count() <= 2,
            "a usage error is the problem and the usage line: {err}"
        );
        err
    }

    pub fn read(&self, dir: &Path, rel: &str) -> String {
        std::fs::read_to_string(dir.join(rel)).unwrap()
    }

    /// One completion request over the `CompleteEnv` transport. The shell invokes
    /// `tasks -- <words…>` with the cursor on `index`; `words[0]` is the binary name, so
    /// `complete(dir, "bash", 2, &["tasks", "show", "sci-"])` completes `sci-`.
    /// Returns one string per candidate; under `"zsh"` each is `value:description`.
    pub fn complete(&self, dir: &Path, shell: &str, index: usize, words: &[&str]) -> Vec<String> {
        let out = self
            .cmd(dir)
            .env("TASKS_COMPLETE", shell)
            .env("_CLAP_COMPLETE_INDEX", index.to_string())
            .arg("--")
            .args(words)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "completion for {words:?} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "completion wrote to stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Like `complete`, but drops the still-available global flags (`-C`, `--json`,
    /// `--pretty`, `--color`, `--help`) that clap_complete appends after the real candidates when
    /// completing a bare positional with an empty word. Use this whenever the assertion
    /// checks the candidate list itself (equality, emptiness) rather than membership.
    pub fn complete_values(
        &self,
        dir: &Path,
        shell: &str,
        index: usize,
        words: &[&str],
    ) -> Vec<String> {
        self.complete(dir, shell, index, words)
            .into_iter()
            .filter(|candidate| !candidate.starts_with('-'))
            .collect()
    }
}
/// Run `script` under a process whose `comm` is `comm`, so the `tasks` it launches has a
/// recognized harness ancestor. A symlink to `/bin/sh` supplies the comm: the kernel takes
/// `comm` from the basename of the path passed to `execve`, not from the resolved target,
/// so a link named `codex` runs `sh` under the comm `codex`. It must be a link rather than
/// a copy — a copy opens an executable for writing, and a sibling test thread that forks
/// between the copy's open and its close inherits that writable descriptor, which makes the
/// `execve` here fail with `ETXTBSY`. Nothing is `exec`ed from the script either: `exec`
/// would replace the shim with `tasks`, which would inherit the shim's pid and its parent,
/// destroying the ancestry under test.
pub fn harness_shim(dir: &Path, home: &Path, comm: &str, script: &str) -> std::process::Output {
    let shim = home.join(comm);
    if !shim.exists() {
        std::os::unix::fs::symlink("/bin/sh", &shim).unwrap();
    }
    std::process::Command::new(&shim)
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_FORMAT")
        .env_remove("TASKS_OWNER")
        .env_remove("TASKS_SESSION")
        .env_remove("TASKS_SESSION_PID")
        .env_remove("TASKS_MODEL")
        .env_remove("TASKS_AGENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_PID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .env("TASKS_BIN", assert_cmd::cargo::cargo_bin("tasks"))
        .output()
        .unwrap()
}

/// Shell function that writes a one-agent registry naming the *shim's own* process, with
/// the private mode relay requires, then leaves `$TASKS_BIN` ready to run.
///
/// The start token must come from `/proc/$$/stat`, the shim's own stat file. Reading
/// `/proc/self/stat` inside a `$(…)` substitution reads the *substituting* process — a
/// different process with a different start token — and pairing that with `$$` produces a
/// handle that matches only if two processes happened to start within one clock tick.
pub const WRITE_REGISTRY: &str = r#"
write_registry() {
  start=$(awk '{print $22}' "/proc/$$/stat")
  boot=$(cat /proc/sys/kernel/random/boot_id)
  host=$(cat /proc/sys/kernel/hostname)
  mkdir -p "$RELAY_STATE_DIR"
  chmod 700 "$RELAY_STATE_DIR"
  cat > "$RELAY_STATE_DIR/agents.json" <<EOF
{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,
 "agents":{"$AGENT_ID":{"id":"$AGENT_ID","harness":"$HARNESS","sessionId":"$SESSION",
  "scope":"session","cwd":"/w","repoRoot":null,"remote":null,"projectKey":"k",
  "project":"p","state":"idle","updatedAt":1,
  "process":{"platform":"linux","host":"$host","bootId":"$boot","pid":$$,"start":"$start"}}}}
EOF
  chmod 600 "$RELAY_STATE_DIR/agents.json"
}
"#;
