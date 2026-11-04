mod common;
use common::git;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

fn sandbox() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    common::git_fixture::assert_repo(dir.path(), dir.path());
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let just = bin.join("just");
    std::fs::write(
        &just,
        r#"#!/bin/sh
if [ "$1" = --evaluate ]; then
    [ "$EVALUATE_FAIL" != 1 ] || exit 1
    case "$2" in
        docs_paths) printf %s "$DOCS_PATHS" ;;
        ci_suite_refs) printf %s "$CI_REFS" ;;
        ci_remote) printf %s origin ;;
        *) exit 1 ;;
    esac
else
    case "$1" in
        hook-pre-push*) [ -z "${GIT_DIR+x}${GIT_WORK_TREE+x}" ] || exit 9 ;;
    esac
    printf '%s\n' "$1"
    cat
fi
"#,
    )
    .unwrap();
    std::fs::set_permissions(just, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn stage(dir: &Path, path: &str) {
    let file = dir.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, "test\n").unwrap();
    git(dir, &["add", "--", path]);
}

fn hook(dir: &Path, name: &str, globs: &str, remote: &str, refs: &str, fail: bool) -> String {
    let mut child = Command::new("sh")
        .env_clear()
        .envs(common::isolated_env(dir))
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".githooks")
                .join(name),
        )
        .args([remote, "https://example.invalid/repo"])
        .current_dir(dir)
        .env("GIT_DIR", dir.join(".git"))
        .env("GIT_WORK_TREE", dir)
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.join("bin").display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("DOCS_PATHS", globs)
        .env("CI_REFS", globs)
        .env("EVALUATE_FAIL", if fail { "1" } else { "0" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(refs.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn front_door_commit_classifies_every_staged_path() {
    for (paths, globs, fail, want) in [
        (
            vec!["AGENTS.md", "tasks/tasks-a.md"],
            "AGENTS.md tasks/*.md",
            false,
            "hook-pre-commit-docs\n",
        ),
        (
            vec!["README.md"],
            "AGENTS.md tasks/*.md",
            false,
            "hook-pre-commit\n",
        ),
        (
            vec!["docs/example.md"],
            "AGENTS.md tasks/*.md",
            false,
            "hook-pre-commit\n",
        ),
        (
            vec!["tasks/a.md\nsrc/code.rs"],
            "AGENTS.md tasks/*.md",
            false,
            "hook-pre-commit\n",
        ),
        (
            vec!["AGENTS.md", "src/code.rs"],
            "AGENTS.md tasks/*.md",
            false,
            "hook-pre-commit\n",
        ),
        (vec!["AGENTS.md"], "", false, "hook-pre-commit\n"),
        (
            vec!["AGENTS.md"],
            "AGENTS.md tasks/*.md",
            true,
            "hook-pre-commit\n",
        ),
        (vec![], "AGENTS.md tasks/*.md", false, "hook-pre-commit\n"),
    ] {
        let dir = sandbox();
        for path in paths {
            stage(dir.path(), path);
        }
        assert_eq!(hook(dir.path(), "pre-commit", globs, "", "", fail), want);
    }
    let dir = sandbox();
    stage(dir.path(), "src/code.rs");
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "baseline",
        ],
    );
    std::fs::create_dir(dir.path().join("tasks")).unwrap();
    git(dir.path(), &["mv", "src/code.rs", "tasks/code.md"]);
    assert_eq!(
        hook(
            dir.path(),
            "pre-commit",
            "AGENTS.md tasks/*.md",
            "",
            "",
            false
        ),
        "hook-pre-commit\n"
    );
}

#[test]
fn front_door_push_requires_ci_coverage_and_preserves_refs() {
    let main = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        "1".repeat(40),
        "2".repeat(40)
    );
    let feature = main.replace("refs/heads/main", "refs/heads/feature/x");
    let deletion = format!(
        "(delete) {} refs/heads/feature/x {}\n",
        "0".repeat(40),
        "2".repeat(40)
    );
    for (refs, globs, remote, fail, want) in [
        (
            main.clone(),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push-fast",
        ),
        (
            feature.clone(),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push",
        ),
        (
            format!("{main}{feature}"),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push",
        ),
        (
            deletion.clone(),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push-fast",
        ),
        (deletion, "", "origin", false, "hook-pre-push"),
        (
            feature,
            "refs/heads/*",
            "origin",
            false,
            "hook-pre-push-fast",
        ),
        (
            main.clone(),
            "refs/heads/main",
            "mirror",
            false,
            "hook-pre-push",
        ),
        (
            main.clone(),
            "refs/heads/main",
            "origin",
            true,
            "hook-pre-push",
        ),
        (main, "", "origin", false, "hook-pre-push"),
        (
            String::new(),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push",
        ),
        (
            "malformed\n".into(),
            "refs/heads/main",
            "origin",
            false,
            "hook-pre-push",
        ),
    ] {
        let dir = sandbox();
        assert_eq!(
            hook(dir.path(), "pre-push", globs, remote, &refs, fail),
            format!("{want}\n{refs}")
        );
    }
}

#[test]
fn front_door_git_fixtures_ignore_hostile_environment() {
    if common::git_fixture::hostile_parent("front_door_git_fixtures_ignore_hostile_environment") {
        return;
    }
    front_door_commit_classifies_every_staged_path();
    let fixture = FrontDoorFixture::new(Some(false));
    let out = fixture.run(&["gate"], &[]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(fixture.markers(), ["check-hygiene"]);
}

struct FrontDoorFixture {
    dir: tempfile::TempDir,
}

fn set_variable(root: &Path, name: &str, value: &str) {
    let path = root.join("justfile");
    let source = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("{name} := ");
    let mut found = false;
    let mut lines: Vec<String> = source
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                found = true;
                format!("{prefix}{value}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !found {
        lines.push(format!("{prefix}{value}"));
    }
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

impl FrontDoorFixture {
    fn new(policy: Option<bool>) -> Self {
        let fixture = Self { dir: sandbox() };
        let root = fixture.dir.path();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in [
            "justfile",
            "tools/tt",
            ".githooks/pre-commit",
            ".githooks/pre-push",
        ] {
            let dest = root.join(path);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(source.join(path), dest).unwrap();
        }
        // Use the real just executable; sandbox's stub serves only the classifier tests.
        std::fs::remove_file(root.join("bin/just")).unwrap();
        for (name, marker) in [
            ("check_hygiene_cmd", "check-hygiene"),
            ("docs_check_cmd", "check-hygiene"),
            ("ci_hygiene_cmd", "ci-hygiene"),
            ("check_code_cmd", "check-code"),
            ("fast_cmd", "test-fast"),
            ("test_cmd", "test"),
            ("one_cmd", "test-one"),
        ] {
            set_variable(root, name, &format!("\"python3 bin/marker {marker}\""));
        }
        fixture.script("marker", r#"#!/usr/bin/env python3
import json, os, sys
with open('markers.jsonl', 'a') as stream:
    stream.write(json.dumps(dict(target=sys.argv[1], args=sys.argv[2:], stdin=sys.stdin.read(),
        git={k: os.environ[k] for k in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE') if k in os.environ})) + '\n')
sys.exit(7 if os.environ.get('FAIL_MARKER') == sys.argv[1] else 0)
"#);
        fixture.script(
            "ssh",
            "#!/bin/sh\nprintf 'serveraliveinterval %s\\n' \"${KEEPALIVE:-60}\"\n",
        );
        if let Some(enabled) = policy {
            std::fs::write(root.join(".tt.toml"), format!("automatic_broad = {enabled}\nbroad_targets = [\"test\", \"test-fast\", \"check-code\"]\n")).unwrap();
        }
        fixture
    }

    fn script(&self, name: &str, text: &str) {
        let path = self.dir.path().join("bin").join(name);
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn command(&self, program: &str) -> Command {
        let root = self.dir.path();
        let mut cmd = Command::new(program);
        cmd.env_clear()
            .envs(common::isolated_env(root))
            .current_dir(root);
        for (key, path) in [
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_DATA_HOME", ".local/share"),
            ("XDG_CACHE_HOME", ".cache"),
            ("XDG_STATE_HOME", ".local/state"),
            ("TT_LOG", "runs.jsonl"),
        ] {
            cmd.env(key, root.join(path));
        }
        cmd.env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap()
            ),
        );
        cmd
    }

    fn reset(&self) {
        for file in ["markers.jsonl", "runs.jsonl"] {
            let path = self.dir.path().join(file);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
        self.reset();
        self.command("just")
            .args(args)
            .envs(env.iter().copied())
            .output()
            .unwrap()
    }

    fn rows(&self, name: &str) -> Vec<serde_json::Value> {
        let path = self.dir.path().join(name);
        if !path.exists() {
            return vec![];
        }
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn markers(&self) -> Vec<String> {
        self.rows("markers.jsonl")
            .iter()
            .map(|row| row["target"].as_str().unwrap().to_owned())
            .collect()
    }
}

const ROUTES: &[(&str, &[&str], &[&str])] = &[
    (
        "check",
        &["check-hygiene", "check-code"],
        &["check-hygiene", "check-code"],
    ),
    (
        "gate",
        &["check-hygiene", "check-code", "test"],
        &["check-hygiene", "check-code", "test"],
    ),
    (
        "hook-pre-commit",
        &["check-hygiene", "check-code"],
        &["hook-pre-commit", "check-code"],
    ),
    (
        "hook-pre-push",
        &["check-hygiene", "check-code", "test"],
        &["hook-pre-push", "check-code", "test"],
    ),
    (
        "hook-pre-push-fast",
        &["check-hygiene", "check-code", "test-fast"],
        &["hook-pre-push-fast", "check-code", "test-fast"],
    ),
    ("ci-check-hygiene", &["ci-hygiene"], &["ci-check-hygiene"]),
];

#[test]
fn front_door_enabled_recipes_run_hygiene_and_broad_children_and_short_circuit_on_failure() {
    for policy in [None, Some(true)] {
        let fixture = FrontDoorFixture::new(policy);
        for &(recipe, markers, targets) in ROUTES {
            let out = fixture.run(&[recipe], &[]);
            assert!(out.status.success(), "{recipe}: {out:?}");
            assert_eq!(fixture.markers(), markers);
            let rows = fixture.rows("runs.jsonl");
            assert_eq!(
                rows.iter()
                    .map(|row| row["target"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                targets
            );
            assert!(rows.iter().all(|row| row["execution"] == "ran"));
            for fail in ["check-hygiene", "check-code"] {
                if let Some(index) = markers.iter().position(|&marker| marker == fail) {
                    let out = fixture.run(&[recipe], &[("FAIL_MARKER", fail)]);
                    assert_eq!(out.status.code(), Some(7), "{recipe}: {out:?}");
                    assert_eq!(fixture.markers(), markers[..=index]);
                }
            }
        }
    }
}

#[test]
fn front_door_policy_off_runs_hygiene_and_records_skipped_broad_children() {
    let fixture = FrontDoorFixture::new(Some(false));
    for &(recipe, _, targets) in ROUTES {
        let out = fixture.run(&[recipe], &[]);
        assert!(out.status.success(), "{recipe}: {out:?}");
        assert_eq!(
            fixture.markers(),
            if recipe == "ci-check-hygiene" {
                ["ci-hygiene"]
            } else {
                ["check-hygiene"]
            }
        );
        let rows = fixture.rows("runs.jsonl");
        assert_eq!(rows.len(), targets.len());
        for (row, target) in rows.iter().zip(targets) {
            assert_eq!(row["target"], *target);
            if ["check-code", "test", "test-fast"].contains(target) {
                assert_eq!(row["execution"], "skipped");
                assert_eq!(row["execution_reason"], "policy-disabled");
                for key in ["exit", "tests", "selection", "slowest"] {
                    assert_eq!(row.get(key), Some(&serde_json::Value::Null));
                }
            } else {
                assert_eq!(row["execution"], "ran");
            }
        }
    }
}

#[test]
fn front_door_test_one_preserves_arguments_and_test_full_forces_suite() {
    let fixture = FrontDoorFixture::new(Some(false));
    let args = ["--test", "cli", "two words", "it's", "$HOME", "glob *"];
    let out = fixture.run(&[&["test-one"][..], &args].concat(), &[]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fixture.rows("markers.jsonl")[0]["args"],
        serde_json::json!(args)
    );
    assert_eq!(fixture.rows("runs.jsonl")[0]["execution"], "ran");
    assert!(!fixture.run(&["test-one"], &[]).status.success());
    for (env, exit) in [(vec![], 0), (vec![("FAIL_MARKER", "test")], 7)] {
        let out = fixture.run(&["test-full"], &env);
        assert_eq!(out.status.code(), Some(exit), "{out:?}");
        assert_eq!(fixture.markers(), ["test"]);
        let rows = fixture.rows("runs.jsonl");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["target"], "test");
        assert_eq!(rows[0]["execution_reason"], "forced");
        assert_eq!(rows[0]["exit"], exit);
    }
}

#[test]
fn front_door_full_suite_merges_cargo_streams_before_tt_capture() {
    let fixture = FrontDoorFixture::new(None);
    fixture.script(
        "suite",
        r#"#!/usr/bin/env python3
import sys
sys.stderr.write('Running unittests src/main.rs (target/debug/deps/tasks-123)\n')
sys.stderr.flush()
sys.stdout.write('test result: ok. 2 passed; 0 failed; 0 ignored;\n')
"#,
    );
    set_variable(fixture.dir.path(), "test_cmd", "\"python3 bin/suite\"");
    let out = fixture.run(&["test"], &[]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Running unittests src/main.rs (target/debug/deps/tasks-123)\ntest result: ok. 2 passed; 0 failed; 0 ignored;\n"
    );
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
    assert_eq!(fixture.rows("runs.jsonl")[0]["tests"], 2);
}

#[test]
fn front_door_hosted_broad_targets_require_force_and_propagate_failure() {
    let fixture = FrontDoorFixture::new(Some(false));
    for recipe in [
        "test",
        "test-fast",
        "check-code",
        "check",
        "gate",
        "hook-pre-commit",
        "hook-pre-push",
        "hook-pre-push-fast",
    ] {
        let out = fixture.run(&[recipe], &[("GITHUB_ACTIONS", "true")]);
        assert_eq!(out.status.code(), Some(2), "{recipe}: {out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("TT_FORCE=1"));
        assert!(
            fixture
                .markers()
                .iter()
                .all(|marker| marker == "check-hygiene")
        );
        assert!(
            fixture
                .rows("runs.jsonl")
                .iter()
                .all(|row| row["execution"] == "ran"
                    && row["target"] != "check-code"
                    && row["target"] != "test"
                    && row["target"] != "test-fast")
        );
    }
    for (fail, exit, markers) in [
        ("", 0, vec!["check-hygiene", "check-code", "test"]),
        ("check-code", 7, vec!["check-hygiene", "check-code"]),
        ("test", 7, vec!["check-hygiene", "check-code", "test"]),
    ] {
        let out = fixture.run(
            &["gate"],
            &[
                ("GITHUB_ACTIONS", "true"),
                ("TT_FORCE", "1"),
                ("FAIL_MARKER", fail),
            ],
        );
        assert_eq!(out.status.code(), Some(exit), "{out:?}");
        assert_eq!(fixture.markers(), markers);
        assert!(
            fixture
                .rows("runs.jsonl")
                .iter()
                .all(|row| row["execution_reason"] == "forced")
        );
    }
}

#[test]
fn front_door_pre_push_policy_warning_and_invalid_policy_refusal() {
    let fixture = FrontDoorFixture::new(Some(false));
    let root = fixture.dir.path();
    let refs = format!(
        "refs/heads/feature/x {} refs/heads/feature/x {}\n\n",
        "1".repeat(40),
        "2".repeat(40)
    );
    let push = |remote: &str, url: &str, input: &str, env: &[(&str, &str)]| {
        fixture.reset();
        let mut child = fixture
            .command("sh")
            .args([".githooks/pre-push", remote, url])
            .env("GIT_DIR", root.join(".git"))
            .env("GIT_WORK_TREE", root)
            .env("GIT_INDEX_FILE", root.join(".git/index"))
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    for (covered, remote, force, warning, suite) in [
        (false, "origin", false, true, "test"),
        (true, "mirror", false, true, "test"),
        (true, "origin", false, false, "test-fast"),
        (false, "origin", true, false, "test"),
    ] {
        set_variable(
            root,
            "ci_suite_refs",
            if covered { "\"refs/heads/*\"" } else { "\"\"" },
        );
        // A single well-formed line is required for covered ref classification.
        let input = refs.trim_end_matches('\n').to_owned() + "\n";
        let out = push(
            remote,
            "https://example.invalid/repo",
            &input,
            if force { &[("TT_FORCE", "1")] } else { &[] },
        );
        assert!(out.status.success(), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(stderr.contains("pre-push: local test skipped by policy; remote full-suite coverage is not declared for this push. Use TT_FORCE=1 for deliberate local verification."), warning, "{stderr}");
        let rows = fixture.rows("markers.jsonl");
        assert_eq!(rows[0]["stdin"], input);
        assert_eq!(rows[0]["git"], serde_json::json!({}));
        assert_eq!(fixture.rows("runs.jsonl").last().unwrap()["target"], suite);
    }
    set_variable(root, "ci_suite_refs", "\"\"");
    let out = push("origin", "https://example.invalid/repo", &refs, &[]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(fixture.rows("markers.jsonl")[0]["stdin"], refs);
    std::fs::write(
        root.join(".tt.toml"),
        "automatic_broad = true\nbroad_targets = [\"test\", \"test-fast\", \"check-code\"]\n",
    )
    .unwrap();
    let out = push("origin", "https://example.invalid/repo", &refs, &[]);
    assert!(out.status.success(), "{out:?}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("local test skipped by policy"));
    std::fs::write(root.join(".tt.toml"), "automatic_broad = \"no\"\n").unwrap();
    let out = push("origin", "https://example.invalid/repo", &refs, &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("pre-push: cannot inspect test execution policy:")
    );
    assert!(fixture.rows("runs.jsonl").is_empty());
    let out = push(
        "origin",
        "git@example.invalid:repo",
        &refs,
        &[("KEEPALIVE", "0")],
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no SSH keepalive"));
    assert!(fixture.markers().is_empty());
}

fn workflow() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"))
        .unwrap()
}

fn workflow_run(source: &str, name: &str) -> String {
    let step = source
        .split(&format!("      - name: {name}\n"))
        .nth(1)
        .unwrap()
        .split("      - name:")
        .next()
        .unwrap();
    step.split("        run: |\n")
        .nth(1)
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn front_door_clean_home_ci_hygiene_passes_without_registry() {
    let fixture = FrontDoorFixture::new(None);
    for args in [vec!["tools/ops-check"], vec!["tools/ops-docs", "check"]] {
        let out = fixture
            .command("python3")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(&args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
        assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    }
}

#[test]
fn front_door_ci_workflow_structure_and_pins() {
    let source = workflow();
    for required in [
        "push:\n    branches: ['**']",
        "workflow_dispatch:",
        "contents: read",
        "group: ${{ github.workflow }}-${{ github.ref }}",
        "cancel-in-progress: true",
        "runs-on: ubuntu-24.04",
        "timeout-minutes: 25",
        "TT_FORCE: \"1\"",
        "GIT_CONFIG_NOSYSTEM: \"1\"",
        "CARGO_TERM_COLOR: \"never\"",
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "fetch-depth: 0",
        "persist-credentials: false",
        "extractions/setup-just@53165ef7e734c5c07cb06b3c8e7b647c5aa16db3",
        "just-version: 1.59.0",
        "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "if: always()",
        "retention-days: 7",
        "if-no-files-found: error",
        "rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy",
        "rustup default 1.99.1",
        "apt-get install --no-install-recommends -y zsh",
    ] {
        assert!(
            source.contains(required),
            "missing workflow contract: {required}"
        );
    }
    assert!(!source.contains("pull_request"));
    let mut previous = 0;
    for step in [
        "Prepare isolated Git config and evidence directory",
        "Checkout",
        "Install zsh and Rust toolchain",
        "Install just",
        "Record tool versions",
        "Run setup",
        "Run portable hygiene checks",
        "Run Rust format and lint checks",
        "Run full test suite and verify coverage",
        "Upload CI evidence",
    ] {
        let index = source.find(&format!("      - name: {step}\n")).unwrap();
        assert!(index > previous, "step order: {step}");
        previous = index;
    }
    assert!(workflow_run(&source, "Run setup").contains("just setup"));
    assert!(workflow_run(&source, "Run portable hygiene checks").contains("just ci-check-hygiene"));
    assert!(workflow_run(&source, "Run Rust format and lint checks").contains("just check-code"));
}

#[test]
fn front_door_ci_coverage_verifier_enforces_dynamic_metadata_targets() {
    let shell = workflow_run(&workflow(), "Run full test suite and verify coverage");
    let fixture = FrontDoorFixture::new(None);
    let root = fixture.dir.path();
    git(
        root,
        &["commit", "--allow-empty", "-qm", "coverage fixture"],
    );
    let evidence = root.join("evidence");
    std::fs::create_dir(&evidence).unwrap();
    let target = |name: &str, path: &str, kind: &str, test: bool| {
        serde_json::json!({
            "name": name, "src_path": root.join(path), "kind": [kind], "test": test, "doctest": false
        })
    };
    let metadata = serde_json::json!({"packages": [{"manifest_path": root.join("Cargo.toml"), "targets": [
        target("tasks", "src/main.rs", "bin", true), target("cli", "tests/cli.rs", "test", true),
        target("multipart", "tests/multipart.rs", "test", true), target("build-script-build", "build.rs", "custom-build", false)
    ]}]});
    std::fs::write(root.join("metadata.json"), metadata.to_string()).unwrap();
    fixture.script("cargo", "#!/bin/sh\n[ \"$*\" = 'metadata --no-deps --format-version 1' ] || exit 9\ncat metadata.json\n");
    fixture.script(
        "just",
        r#"#!/usr/bin/env python3
import os, pathlib, sys
assert sys.argv[1:] == ['test']
log = pathlib.Path.home() / '.local/share/ops/runs.jsonl'
log.parent.mkdir(parents=True, exist_ok=True)
log.write_text(pathlib.Path('tt-input.jsonl').read_text())
sys.stdout.write(pathlib.Path('cargo-output.log').read_text())
sys.exit(int(os.environ.get('SUITE_EXIT', '0')))
"#,
    );
    let log = "Running unittests src/main.rs (target/debug/deps/tasks-123)\nrunning 2 tests\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\nRunning tests/cli.rs (target/debug/deps/cli-123)\nrunning 3 tests\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\nRunning tests/multipart.rs (target/debug/deps/multipart-123)\nrunning 4 tests\ntest result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n";
    let row = serde_json::json!({"target": "test", "execution": "ran", "execution_reason": "forced", "exit": 0, "tests": 9});
    let run = |output: &str, record: Option<&serde_json::Value>, exit: &str| {
        let summary = evidence.join("coverage-summary.json");
        if summary.exists() {
            std::fs::remove_file(summary).unwrap();
        }
        std::fs::write(root.join("cargo-output.log"), output).unwrap();
        std::fs::write(
            root.join("tt-input.jsonl"),
            record.map(|r| r.to_string() + "\n").unwrap_or_default(),
        )
        .unwrap();
        fixture
            .command("bash")
            .args(["-e", "-o", "pipefail", "-c", &shell])
            .env("EVIDENCE_DIR", &evidence)
            .env("SUITE_EXIT", exit)
            .output()
            .unwrap()
    };
    let out = run(log, Some(&row), "0");
    assert!(out.status.success(), "{out:?}");
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence.join("coverage-summary.json")).unwrap())
            .unwrap();
    for (key, want) in [
        ("excluded_modules", 0),
        ("skipped_tests", 0),
        ("target_count", 3),
        ("executed_tests", 9),
    ] {
        assert_eq!(summary[key], want);
    }
    assert_eq!(
        summary["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["src/main.rs", "tests/cli.rs", "tests/multipart.rs"]
    );
    let out = run(log, Some(&row), "101");
    assert_eq!(out.status.code(), Some(101), "{out:?}");
    assert_eq!(
        std::fs::read_to_string(evidence.join("runs.jsonl")).unwrap(),
        row.to_string() + "\n"
    );
    assert!(!evidence.join("coverage-summary.json").exists());
    for bad in [
        log.split("Running tests/multipart.rs")
            .next()
            .unwrap()
            .to_owned(),
        log.replace("Running tests/cli.rs", "Running tests/extra.rs"),
        log.to_owned() + log,
        log.replace("running 3 tests", "Running tests/multipart.rs (extra)"),
        log.replacen(
            "test result: ok. 2 passed; 0 failed; 0 ignored;",
            "test result: ok. 2 passed; 0 failed; 1 ignored;",
            1,
        ),
        log.replacen("0 failed", "1 failed", 1),
        log.replace("2 passed", "0 passed")
            .replace("3 passed", "0 passed")
            .replace("4 passed", "0 passed"),
        log.replacen(
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n",
            "",
            1,
        ),
    ] {
        let out = run(&bad, Some(&row), "0");
        assert!(
            !out.status.success(),
            "accepted incomplete coverage: {bad}\n{out:?}"
        );
    }
    for (key, value) in [
        ("execution", serde_json::json!("skipped")),
        ("execution_reason", serde_json::json!("default")),
        ("exit", serde_json::json!(1)),
        ("tests", serde_json::json!(8)),
    ] {
        let mut bad = row.clone();
        bad[key] = value;
        assert!(
            !run(log, Some(&bad), "0").status.success(),
            "accepted bad tt {key}"
        );
    }
    assert!(!run(log, None, "0").status.success());
    let mut unsupported = metadata;
    unsupported["packages"][0]["targets"][0]["doctest"] = serde_json::json!(true);
    std::fs::write(root.join("metadata.json"), unsupported.to_string()).unwrap();
    assert!(!run(log, Some(&row), "0").status.success());
}

fn repo_policy_contract(root: &Path) -> bool {
    let fixture = FrontDoorFixture::new(None);
    let evaluate = |name| {
        let out = fixture
            .command("just")
            .current_dir(root)
            .args(["--evaluate", name])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    if evaluate("ci_remote") != "origin" {
        return false;
    }
    let policy = root.join(".tt.toml");
    let enabled = if policy.exists() {
        let document: toml::Value =
            toml::from_str(&std::fs::read_to_string(policy).unwrap()).unwrap();
        if document["automatic_broad"].as_bool() == Some(false) {
            if document["broad_targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
                != ["test", "test-fast", "check-code"]
            {
                return false;
            }
            false
        } else {
            true
        }
    } else {
        true
    };
    let refs = evaluate("ci_suite_refs");
    if enabled {
        refs.is_empty()
    } else {
        refs == "refs/heads/*"
            && std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
                .is_ok_and(|s| s.contains("TT_FORCE: \"1\""))
    }
}

#[test]
fn front_door_live_repo_policy_and_ci_coverage_contract() {
    assert!(repo_policy_contract(Path::new(env!("CARGO_MANIFEST_DIR"))));
    for policy in [None, Some(true), Some(false)] {
        let fixture = FrontDoorFixture::new(policy);
        let root = fixture.dir.path();
        set_variable(root, "ci_suite_refs", "\"\"");
        if policy == Some(false) {
            assert!(!repo_policy_contract(root));
            set_variable(root, "ci_suite_refs", "\"refs/heads/*\"");
            assert!(!repo_policy_contract(root));
            std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
            std::fs::write(root.join(".github/workflows/ci.yml"), workflow()).unwrap();
            assert!(repo_policy_contract(root));
            std::fs::write(
                root.join(".tt.toml"),
                "automatic_broad = false\nbroad_targets = [\"test\"]\n",
            )
            .unwrap();
            assert!(!repo_policy_contract(root));
        } else {
            assert!(repo_policy_contract(root));
            set_variable(root, "ci_suite_refs", "\"refs/heads/*\"");
            assert!(!repo_policy_contract(root));
        }
    }
}
