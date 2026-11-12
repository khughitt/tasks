mod common;

use common::{TestEnv, as_agent, git, id_of, repo_with_worktree};
use serde_json::Value;
use std::path::Path;

fn missing(env: &TestEnv, root: &Path, args: &[&str]) -> String {
    let out = env.cmd(root).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "task_not_found", "{value}");
    assert_eq!(value["error"].as_object().unwrap().len(), 2);
    value["error"]["detail"].as_str().unwrap().to_owned()
}

#[test]
fn missing_recovery_reads_both_directions() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let branch_id = id_of(env.json(&side, &["add", "Branch only"]));
    let main_id = id_of(env.json(&main, &["add", "Main only"]));
    for (here, there, id) in [(&main, &side, branch_id), (&side, &main, main_id)] {
        let path = there.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&path).unwrap();
        let detail = missing(&env, here, &["show", &id]);
        assert!(detail.contains(there.to_str().unwrap()), "{detail}");
        assert!(detail.contains(&format!("show {id}")), "{detail}");
        assert!(detail.contains("observed"), "{detail}");
        assert!(!here.join(format!("tasks/{id}.md")).exists());
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert_eq!(env.json(there, &["show", &id])["task"]["id"], id);
    }
}

#[test]
fn missing_recovery_zero_and_ambiguous() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let detail = missing(&env, &main, &["show", "sci-000000"]);
    assert!(detail.contains("no matching record file"), "{detail}");
    let other = main.join("another");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "another",
            other.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &other);
    let id = id_of(env.json(&side, &["add", "Same bytes in two branches"]));
    std::fs::copy(
        side.join(format!("tasks/{id}.md")),
        other.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("ambiguous"), "{detail}");
    for path in [&side, &other] {
        assert!(detail.contains(path.to_str().unwrap()), "{detail}");
    }
    assert_eq!(detail.matches(&format!("show {id}")).count(), 2, "{detail}");
}

#[test]
fn missing_recovery_doc_never_fetches() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Document remains unselected"]));
    let record = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&record).unwrap();
    std::fs::write(
        &record,
        raw.replacen(
            "---\n",
            "---\nspec: \"probe:spec\"\nplan: \"probe:plan\"\n",
            1,
        ),
    )
    .unwrap();
    let sentinel = env.home.path().join("provider-called");
    let command = format!("touch {}; printf document", sentinel.display());
    std::fs::write(
        env.home.path().join(".config/tasks/config.toml"),
        format!("[docs.schemes]\nprobe = {command:?}\n"),
    )
    .unwrap();
    for target in ["--spec", "--plan", "remote"] {
        let detail = missing(&env, &main, &["doc", &id, target]);
        assert!(detail.contains(&format!("show {id}")), "{detail}");
        assert!(!sentinel.exists());
    }
}

#[test]
fn missing_recovery_physical_archive_only() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Physical record with missing history"]));
    let file = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        file,
        raw.replacen(
            "---\n",
            &format!("---\nnote_archives: [\"v1:{}:1\"]\n", "a".repeat(64)),
            1,
        ),
    )
    .unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains(side.to_str().unwrap()), "{detail}");
    assert!(detail.contains("observed"), "{detail}");
    let out = env.cmd(&side).args(["show", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_ne!(common::err_kind(&out), "task_not_found");
}

#[test]
fn missing_recovery_incomplete_and_malformed() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Broken file"]));
    let file = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read(&file).unwrap();
    for bytes in [
        b"not a task".to_vec(),
        String::from_utf8(raw.clone())
            .unwrap()
            .replace(&id, "sci-000000")
            .into_bytes(),
    ] {
        std::fs::write(&file, bytes).unwrap();
        let detail = missing(&env, &main, &["show", &id]);
        assert!(detail.contains("incomplete"), "{detail}");
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(!detail.contains("Run it there:"), "{detail}");
    }
    std::fs::write(&file, &raw).unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(main.join("tasks/.config.toml"), &file).unwrap();
    assert!(missing(&env, &main, &["show", &id]).contains("incomplete"));
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir_all(&side).unwrap();
    assert!(missing(&env, &main, &["show", &id]).contains("incomplete"));
}

#[test]
fn missing_recovery_local_and_recorded_precedence() {
    let mut env = TestEnv::new();
    let (main, side, shared) = repo_with_worktree(&mut env);
    as_agent(&env, &side, "other")
        .args(["start", &shared])
        .assert()
        .success();
    assert_eq!(
        env.json(&main, &["show", &shared])["task"]["status"],
        "todo"
    );
    let id = id_of(env.json(&side, &["add", "Claimed task"]));
    as_agent(&env, &side, "other")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(env.json(&main, &["show", &id])["task"]["id"], id);
    std::fs::remove_file(side.join(format!("tasks/{id}.md"))).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("claimed in") && detail.contains("unavailable"),
        "{detail}"
    );
    assert!(!detail.contains("observed"), "{detail}");
    std::fs::write(main.join(format!("tasks/{shared}.md")), "malformed").unwrap();
    let out = env.cmd(&main).args(["show", &shared]).output().unwrap();
    assert_ne!(common::err_kind(&out), "task_not_found");
}

#[test]
fn missing_recovery_linked_overlay_and_foreign_route() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let code = tempfile::tempdir().unwrap();
    for root in [&main, &side] {
        let path = root.join("tasks/.config.toml");
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            format!(
                "{raw}\n[checkout]\npath = {:?}\n",
                code.path().display().to_string()
            ),
        )
        .unwrap();
    }
    let id = id_of(env.json(&side, &["add", "Linked overlay only"]));
    let foreign = env.init("oth");
    for here in [&main, &foreign] {
        let detail = missing(&env, here, &["show", &id]);
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(!detail.contains(code.path().to_str().unwrap()), "{detail}");
    }
}

#[test]
fn missing_recovery_non_utf8_and_quoted_paths() {
    use std::os::unix::ffi::OsStringExt;
    let mut env = TestEnv::new();
    let (main, _, _) = repo_with_worktree(&mut env);
    let side = main.join(std::ffi::OsString::from_vec(b"bad-\xff".to_vec()));
    let out = common::git_fixture::command(env.home.path(), &main)
        .args(["worktree", "add", "-q", "-b", "bytes"])
        .arg(&side)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let id = id_of(env.json(&side, &["add", "Byte path"]));
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("incomplete"), "{detail}");
    assert!(!detail.contains(&format!("show {id}")), "{detail}");
}

#[test]
fn missing_recovery_nested_project_offset() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let main = base.path().canonicalize().unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    common::git_fixture::assert_repo(env.home.path(), &main);
    let project = main.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    env.json(&project, &["init", "--prefix", "sci"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("side");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let nested = side.join("nested/project");
    let id = id_of(env.json(&nested, &["add", "Nested only"]));
    let detail = missing(&env, &project, &["show", &id]);
    assert!(detail.contains(nested.to_str().unwrap()), "{detail}");
}

#[test]
fn missing_recovery_detached_never_resurrects_code_records() {
    let mut env = TestEnv::new();
    let (main, _historical, id) = repo_with_worktree(&mut env);
    git(&main, &["rm", "-r", "-q", "tasks"]);
    let records = tempfile::tempdir().unwrap();
    env.json(
        &main,
        &[
            "init",
            "--prefix",
            "sci",
            "--records",
            records.path().to_str().unwrap(),
        ],
    );
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("shared records home"), "{detail}");
    assert!(!detail.contains(&format!("show {id}")), "{detail}");
}

#[test]
fn missing_recovery_no_git_is_explicit() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let bin = tempfile::tempdir().unwrap();
    let out = env
        .cmd(&root)
        .env("PATH", bin.path())
        .args(["show", "sci-000000"])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "task_not_found");
    assert!(common::err_detail(&out).contains("search unavailable"));
}

#[test]
fn missing_recovery_excludes_wrong_layout_and_bounds_inputs() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Wrong project"]));
    let config = side.join("tasks/.config.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for wrong in [
        original.replace("\"sci\"", "\"oth\""),
        format!(
            "{original}\n[checkout]\nkind = \"detached\"\npath = {:?}\n",
            main.display().to_string()
        ),
    ] {
        std::fs::write(&config, wrong).unwrap();
        assert!(missing(&env, &main, &["show", &id]).contains("no matching record file"));
    }
    std::fs::write(&config, "x".repeat(65_537)).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("incomplete") && detail.contains("allowance"),
        "{detail}"
    );
    std::fs::write(&config, original).unwrap();
    let file = side.join(format!("tasks/{id}.md"));
    std::fs::write(&file, "PRIVATE_RECORD_CONTENT".repeat(70_000)).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("incomplete") && detail.contains("allowance"),
        "{detail}"
    );
    assert!(!detail.contains("PRIVATE_RECORD_CONTENT"));
    std::fs::write(&file, "PRIVATE_RECORD_CONTENT").unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("invalid physical record"), "{detail}");
    assert!(!detail.contains("PRIVATE_RECORD_CONTENT"));
}

#[test]
fn missing_recovery_shell_special_retry_is_executable() {
    let mut env = TestEnv::new();
    let (main, _, _) = repo_with_worktree(&mut env);
    let side = main.join("quotes ' and spaces; literal");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "quoted",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let id = id_of(env.json(&side, &["add", "Quoted directory"]));
    let detail = missing(&env, &main, &["show", &id]);
    let command = &detail[detail.find("tasks -C ").expect("explicit retry")..];
    let command = command.split("; location hints only;").next().unwrap();
    let bin = env.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(assert_cmd::cargo::cargo_bin("tasks"), bin.join("tasks")).unwrap();
    let out = std::process::Command::new("sh")
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .current_dir(&main)
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(out.status.success(), "{command}: {out:?}");
    let shown: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(shown["task"]["id"], id);
}
