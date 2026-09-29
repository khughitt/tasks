use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

fn sandbox() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
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

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn stage(dir: &Path, path: &str) {
    let file = dir.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, "test\n").unwrap();
    git(dir, &["add", "--", path]);
}

fn hook(dir: &Path, name: &str, globs: &str, remote: &str, refs: &str, fail: bool) -> String {
    let mut child = Command::new("sh")
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
