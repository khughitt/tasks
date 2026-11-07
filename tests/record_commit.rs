mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

#[test]
fn record_commit_git_fixtures_ignore_hostile_environment() {
    if common::git_fixture::hostile_parent("record_commit_git_fixtures_ignore_hostile_environment")
    {
        return;
    }
    add_note_and_done_each_commit_their_record();
    a_linked_checkout_commits_only_its_overlay_records();
    start_from_a_worktree_commits_the_halt_in_its_authority_root();
}

fn git(dir: &Path, args: &[&str]) -> String {
    let home = tempfile::tempdir().unwrap();
    let out = common::git_fixture::command(home.path(), dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    if args.first() == Some(&"init") {
        common::git_fixture::assert_repo(home.path(), dir);
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "t"),
        ("user.email", "t@t"),
        ("commit.gpgsign", "false"),
    ] {
        git(dir, &["config", key, value]);
    }
}

/// A tasks project at the top of a fresh repository, its init committed. `commit`
/// writes `[git] commit = true` into the config first.
fn project(env: &mut TestEnv, prefix: &str, commit: bool) -> PathBuf {
    let root = env.init(prefix);
    git_init(&root);
    if commit {
        let config = root.join("tasks/.config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "init"]);
    root
}

fn subjects(root: &Path) -> Vec<String> {
    git(root, &["log", "--format=%s"])
        .lines()
        .map(String::from)
        .collect()
}

fn last_files(root: &Path) -> Vec<String> {
    git(root, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(String::from)
        .collect()
}

fn add(env: &TestEnv, root: &Path, title: &str) -> String {
    env.json(root, &["add", title])["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn tasks_commit_sweeps_the_backlog_into_one_commit_even_without_the_switch() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let a = add(&env, &root, "one");
    let b = add(&env, &root, "two");
    std::fs::write(root.join("tasks/tst-ffffff.md.tmp"), "transient").unwrap();
    let out = env.json(&root, &["commit"]);
    assert_eq!(out["committed"], 2, "{out}");
    assert_eq!(subjects(&root)[0], "chore(tasks): commit 2 records");
    let files = last_files(&root);
    assert!(
        files.contains(&format!("tasks/{a}.md")) && files.contains(&format!("tasks/{b}.md")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|file| file.ends_with(".tmp")),
        "{files:?}"
    );
}

#[test]
fn tasks_commit_with_nothing_to_commit_says_so() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let out = env.json(&root, &["commit"]);
    assert_eq!(out["committed"], 0, "{out}");
    assert_eq!(subjects(&root), vec!["init"]);
}

#[test]
fn tasks_commit_refuses_beside_staged_work_and_exits_one() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    add(&env, &root, "one");
    std::fs::write(root.join("draft.txt"), "x").unwrap();
    git(&root, &["add", "draft.txt"]);
    let out = env.cmd(&root).arg("commit").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("commit_deferred"),
        "{error}"
    );
}

#[test]
fn tasks_commit_takes_a_project() {
    let mut env = TestEnv::new();
    let here = project(&mut env, "aaa", false);
    let there = project(&mut env, "bbb", false);
    add(&env, &there, "theirs");
    let out = env.json(&here, &["commit", "--project", "bbb"]);
    assert_eq!(out["committed"], 1, "{out}");
    assert_eq!(subjects(&there)[0], "chore(tasks): commit 1 records");
}

#[test]
fn without_the_switch_nothing_is_committed() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    add(&env, &root, "a task");
    assert_eq!(subjects(&root), vec!["init"]);
    assert!(!clean(&root));
}

#[test]
fn add_note_and_done_each_commit_their_record() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    assert_eq!(subjects(&root)[0], format!("chore(tasks): add {id} a task"));
    assert_eq!(last_files(&root), vec![format!("tasks/{id}.md")]);
    env.json(&root, &["note", &id, "a note"]);
    assert_eq!(subjects(&root)[0], format!("chore(tasks): note {id}"));
    env.json(&root, &["start", &id]);
    env.json(&root, &["done", &id, "landed"]);
    assert_eq!(subjects(&root)[0], format!("chore(tasks): done {id}"));
    assert!(clean(&root));
}

#[test]
fn attach_and_detach_commit_the_file_with_the_record() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    let source = root.join("shot.png");
    std::fs::write(&source, "png").unwrap();
    env.json(&root, &["attach", &id, source.to_str().unwrap()]);
    std::fs::remove_file(&source).unwrap();
    let files = last_files(&root);
    assert!(
        files.contains(&format!("tasks/{id}.md"))
            && files.contains(&format!("tasks/files/{id}/shot.png")),
        "{files:?}"
    );
    env.json(&root, &["detach", &id, "shot.png", "wrong one"]);
    assert!(last_files(&root).contains(&format!("tasks/files/{id}/shot.png")));
    assert!(clean(&root));
}

#[test]
fn attach_batch_commits_all_files_and_notes_once() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    std::fs::write(&a, "one").unwrap();
    std::fs::write(&b, "two").unwrap();
    let adopted = root.join(format!("tasks/files/{id}/a.txt"));
    std::fs::create_dir_all(adopted.parent().unwrap()).unwrap();
    std::fs::copy(&a, adopted).unwrap();
    let count = subjects(&root).len();
    env.json(
        &root,
        &["attach", &id, a.to_str().unwrap(), b.to_str().unwrap()],
    );
    assert_eq!(subjects(&root).len(), count + 1);
    let files = last_files(&root);
    for path in [
        format!("tasks/{id}.md"),
        format!("tasks/files/{id}/a.txt"),
        format!("tasks/files/{id}/b.txt"),
    ] {
        assert!(files.contains(&path), "{files:?}");
    }
    assert_eq!(
        git(&root, &["show", &format!("HEAD:tasks/files/{id}/a.txt")]),
        "one"
    );
    assert_eq!(
        git(&root, &["show", &format!("HEAD:tasks/files/{id}/b.txt")]),
        "two"
    );
}

#[test]
fn interrupted_attach_commits_adopted_bytes_with_the_record() {
    for tracked in [false, true] {
        let mut env = TestEnv::new();
        let root = project(&mut env, "tst", true);
        let id = add(&env, &root, "a task");
        let record = format!("tasks/{id}.md");
        let attachment = format!("tasks/files/{id}/shot.png");
        let target = root.join(&attachment);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        if tracked {
            std::fs::write(&target, "earlier bytes").unwrap();
            git(&root, &["add", &attachment]);
            git(&root, &["commit", "-qm", "earlier payload"]);
        }
        let bytes = b"recovered\0bytes\r\n";
        std::fs::write(&target, bytes).unwrap();
        let source = env.home.path().join("shot.png");
        std::fs::write(&source, bytes).unwrap();

        let out = env.json(&root, &["attach", &id, source.to_str().unwrap()]);
        assert_eq!(out["warnings"], serde_json::json!([]), "{out}");
        let files = last_files(&root);
        assert!(
            files.contains(&record) && files.contains(&attachment),
            "{files:?}"
        );
        assert_eq!(
            git(&root, &["show", &format!("HEAD:{attachment}")]).as_bytes(),
            bytes
        );
        let committed_record = git(&root, &["show", &format!("HEAD:{record}")]);
        assert_eq!(
            committed_record,
            std::fs::read_to_string(root.join(record)).unwrap()
        );
        assert!(committed_record.contains("attached: shot.png (17 bytes)"));
        assert!(clean(&root));
    }
}

#[test]
fn a_staged_file_defers_the_commit_with_a_warning_and_exit_zero() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    std::fs::write(root.join("draft.txt"), "x").unwrap();
    git(&root, &["add", "draft.txt"]);
    let out = env.json(&root, &["add", "a task"]);
    let warnings = out["warnings"].to_string();
    assert!(warnings.contains("commit_deferred"), "{warnings}");
    assert_eq!(subjects(&root), vec!["init"]);
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]).trim(),
        "draft.txt"
    );
}

#[test]
fn a_routed_note_commits_in_the_target_project() {
    let mut env = TestEnv::new();
    let home = project(&mut env, "aaa", true);
    let other = project(&mut env, "bbb", true);
    let id = add(&env, &other, "theirs");
    env.json(&home, &["note", &id, "from aaa"]);
    assert_eq!(subjects(&other)[0], format!("chore(tasks): note {id}"));
    assert_eq!(subjects(&home), vec!["init"]);
}

#[test]
fn move_commits_the_tombstone_and_the_new_record_in_their_own_roots() {
    let mut env = TestEnv::new();
    let source = project(&mut env, "aaa", true);
    let target = project(&mut env, "bbb", true);
    let id = add(&env, &source, "travelling");
    let out = env.json(&source, &["move", &id, "--to", "bbb"]);
    let new = out["id"].as_str().unwrap().to_string();
    assert_eq!(subjects(&source)[0], format!("chore(tasks): move {id}"));
    assert!(last_files(&source).contains(&format!("tasks/{id}.md")));
    assert_eq!(subjects(&target)[0], format!("chore(tasks): move {new}"));
    assert!(last_files(&target).contains(&format!("tasks/{new}.md")));
    assert!(clean(&source) && clean(&target));
}

#[test]
fn feedback_commits_in_the_owner_project() {
    let mut env = TestEnv::new();
    let reporter = project(&mut env, "aaa", true);
    let owner = project(&mut env, "own", true);
    let config = owner.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("[git]", "[feedback]\nscope = \"the tool\"\n\n[git]"),
    )
    .unwrap();
    git(&owner, &["commit", "-q", "-am", "accept feedback"]);
    let out = env.json(
        &reporter,
        &[
            "feedback",
            "--project",
            "own",
            "a rough edge",
            "--category",
            "friction",
        ],
    );
    let id = out["id"].as_str().unwrap();
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains("committed there") && !warnings.contains("uncommitted"),
        "{warnings}"
    );
    assert!(
        subjects(&owner)[0].starts_with(&format!("chore(tasks): feedback {id} a rough edge")),
        "{:?}",
        subjects(&owner)
    );
    assert!(clean(&owner));
}

#[test]
fn a_root_outside_git_warns_unavailable() {
    let mut env = TestEnv::new();
    let root = env.init("tst");
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
    let out = env.json(&root, &["add", "a task"]);
    assert!(
        out["warnings"].to_string().contains("commit_unavailable"),
        "{out}"
    );
}

#[test]
fn start_past_a_halt_commits_both_records() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let halt = env.json(&root, &["add", "stop the line", "-p", "0", "--tag", "halt"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = env.json(&root, &["add", "other work", "-p", "2"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&root, &["start", &id, "--force", "--reason", "unrelated"]);
    let files = last_files(&root);
    assert!(
        files.contains(&format!("tasks/{id}.md")) && files.contains(&format!("tasks/{halt}.md")),
        "{files:?}"
    );
    assert!(clean(&root));
}

#[test]
fn two_projects_in_one_repository_commit_concurrently_without_mixing() {
    let mut env = TestEnv::new();
    let (_dir, top, roots) = shared_repo(&mut env, &["aaa", "bbb"]);
    let ids: Vec<String> = roots.iter().map(|root| add(&env, root, "work")).collect();
    let mut children = Vec::new();
    for (root, id) in roots.iter().zip(&ids) {
        for n in 0..4 {
            children.push(
                env.raw(root)
                    .args(["note", id, &format!("note {n}")])
                    .spawn()
                    .unwrap(),
            );
        }
    }
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // Every note commit names one id and touches only that project's directory.
    let log = git(&top, &["log", "--format=@%s", "--name-only"]);
    let mut notes = 0;
    for block in log
        .split('@')
        .filter(|block| block.starts_with("chore(tasks): note "))
    {
        let mut lines = block.lines().filter(|line| !line.is_empty());
        let id = lines
            .next()
            .unwrap()
            .trim_start_matches("chore(tasks): note ");
        let project = &id[..3];
        let files: Vec<_> = lines.collect();
        assert_eq!(files, vec![format!("{project}/tasks/{id}.md")], "{log}");
        notes += 1;
    }
    assert_eq!(notes, 8, "{log}");
    assert!(clean(&top));
}

fn clean(root: &Path) -> bool {
    git(root, &["status", "--porcelain"]).is_empty()
}

/// One repository holding a project per prefix in its own subdirectory, every project
/// opted in and the whole tree committed.
fn shared_repo(env: &mut TestEnv, prefixes: &[&str]) -> (tempfile::TempDir, PathBuf, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    git_init(&top);
    let mut roots = Vec::new();
    for prefix in prefixes {
        let root = top.join(prefix);
        std::fs::create_dir_all(&root).unwrap();
        env.json(&root, &["init", "--prefix", prefix]);
        let config = root.join("tasks/.config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[git]\ncommit = true\n")).unwrap();
        roots.push(root);
    }
    git(&top, &["add", "-A"]);
    git(&top, &["commit", "-q", "-m", "init"]);
    (dir, top, roots)
}

#[test]
fn a_move_within_one_repository_is_one_commit_holding_both_records() {
    let mut env = TestEnv::new();
    let (_dir, top, roots) = shared_repo(&mut env, &["aaa", "bbb"]);
    let id = add(&env, &roots[0], "travelling");
    let out = env.json(&roots[0], &["move", &id, "--to", "bbb"]);
    let new = out["id"].as_str().unwrap().to_string();
    assert_eq!(subjects(&top)[0], format!("chore(tasks): move {id} {new}"));
    let files = last_files(&top);
    assert!(
        files.contains(&format!("aaa/tasks/{id}.md"))
            && files.contains(&format!("bbb/tasks/{new}.md")),
        "{files:?}"
    );
    assert!(clean(&top));
}

#[test]
fn start_does_not_ask_to_commit_a_record_it_just_committed() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let side_dir = tempfile::tempdir().unwrap();
    let side = side_dir.path().join("side");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let id = add(&env, &root, "a task");
    let out = env.json(&root, &["start", &id]);
    let warnings = out["warnings"].to_string();
    assert!(!warnings.contains("before branching"), "{warnings}");
    assert_eq!(subjects(&root)[0], format!("chore(tasks): start {id}"));
}

/// A code repository with one commit, at `base/name` (the linked fixture).
fn code_repo(base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git_init(&dir);
    std::fs::write(dir.join("README"), "code\n").unwrap();
    git(&dir, &["add", "README"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    dir.canonicalize().unwrap()
}

#[test]
fn a_linked_checkout_commits_only_its_overlay_records() {
    let env = TestEnv::new();
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let overlay = base.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    git_init(&overlay);
    let code = code_repo(&base, "code");
    let nexc = overlay.join("nexc");
    std::fs::create_dir_all(&nexc).unwrap();
    env.json(&nexc, &["init", "--prefix", "nexc"]);
    let config = nexc.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!("{text}\n[checkout]\npath = \"../../code\"\n\n[git]\ncommit = true\n"),
    )
    .unwrap();
    git(&overlay, &["add", "-A"]);
    git(&overlay, &["commit", "-q", "-m", "overlay"]);
    let before = git(&code, &["log", "--format=%H"]);
    let id = add(&env, &code, "linked task");
    assert_eq!(
        subjects(&overlay)[0],
        format!("chore(tasks): add {id} linked task")
    );
    assert_eq!(last_files(&overlay), vec![format!("nexc/tasks/{id}.md")]);
    assert_eq!(git(&code, &["log", "--format=%H"]), before);
    assert!(clean(&overlay) && clean(&code));
}

#[test]
fn start_from_a_worktree_commits_the_halt_in_its_authority_root() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let halt = env.json(&root, &["add", "stop the line", "-p", "0", "--tag", "halt"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = env.json(&root, &["add", "other work", "-p", "2"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let side_dir = tempfile::tempdir().unwrap();
    let side = side_dir.path().join("side");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    let out = env.json(&side, &["start", &id, "--force", "--reason", "unrelated"]);
    assert_eq!(subjects(&root)[0], format!("chore(tasks): start {halt}"));
    assert_eq!(last_files(&root), vec![format!("tasks/{halt}.md")]);
    assert_eq!(subjects(&side)[0], format!("chore(tasks): start {id}"));
    assert_eq!(last_files(&side), vec![format!("tasks/{id}.md")]);
    assert!(
        !out["warnings"].to_string().contains("before branching"),
        "{out}"
    );
    assert!(clean(&root) && clean(&side));
}

#[test]
fn move_commits_copied_and_removed_attachments_in_both_roots() {
    let mut env = TestEnv::new();
    let source = project(&mut env, "aaa", true);
    let target = project(&mut env, "bbb", true);
    let id = add(&env, &source, "travelling");
    let shot = source.join("shot.png");
    std::fs::write(&shot, "png").unwrap();
    env.json(&source, &["attach", &id, shot.to_str().unwrap()]);
    std::fs::remove_file(&shot).unwrap();
    let out = env.json(&source, &["move", &id, "--to", "bbb"]);
    let new = out["id"].as_str().unwrap();
    assert!(last_files(&source).contains(&format!("tasks/files/{id}/shot.png")));
    assert!(last_files(&target).contains(&format!("tasks/files/{new}/shot.png")));
    assert_eq!(
        std::fs::read(target.join(format!("tasks/files/{new}/shot.png"))).unwrap(),
        b"png"
    );
    assert!(clean(&source) && clean(&target));
}

#[test]
fn archive_record_commit_and_interrupted_retry_include_both_paths() {
    for boundary in [None, Some("after-record-switch")] {
        let mut env = TestEnv::new();
        let root = project(&mut env, "arc", true);
        let id = add(&env, &root, "Archive");
        env.json(&root, &["note", &id, &"retained history ".repeat(300)]);
        let p = env.json(&root, &["archive-notes", &id]);
        let args = [
            "archive-notes",
            &id,
            "--expect",
            p["source_sha256"].as_str().unwrap(),
        ];
        if let Some(boundary) = boundary {
            let out = env
                .cmd(&root)
                .env("TASKS_ARCHIVE_STOP_AFTER", boundary)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(86));
        }
        let out = env.json(&root, &args);
        assert_eq!(
            out["action"],
            if boundary.is_some() {
                "already_archived"
            } else {
                "archived"
            }
        );
        assert!(out["warnings"].as_array().unwrap().is_empty(), "{out}");
        let mut files = last_files(&root);
        files.sort();
        let mut expected = vec![
            format!("tasks/{id}.md"),
            format!("tasks/files/{id}/{}", p["archive_name"].as_str().unwrap()),
        ];
        expected.sort();
        assert_eq!(files, expected);
        assert!(git(&root, &["status", "--porcelain"]).is_empty());
    }
}

#[test]
fn archive_commit_failure_keeps_landed_history_and_retries_without_duplicate_notes() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let root = project(&mut env, "arc", true);
    let id = add(&env, &root, "Archive");
    env.json(&root, &["note", &id, &"retained history ".repeat(300)]);
    let p = env.json(&root, &["archive-notes", &id]);
    let args = [
        "archive-notes",
        &id,
        "--expect",
        p["source_sha256"].as_str().unwrap(),
    ];
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = env.json(&root, &args);
    assert_eq!(out["action"], "archived");
    assert!(!out["warnings"].as_array().unwrap().is_empty(), "{out}");
    let before = env.json(&root, &["show", &id])["task"].clone();
    assert!(
        root.join(format!(
            "tasks/files/{id}/{}",
            p["archive_name"].as_str().unwrap()
        ))
        .is_file()
    );
    std::fs::remove_file(hook).unwrap();
    let retry = env.json(&root, &args);
    assert_eq!(retry["action"], "already_archived");
    assert!(retry["warnings"].as_array().unwrap().is_empty(), "{retry}");
    assert_eq!(env.json(&root, &["show", &id])["task"], before);
    assert!(git(&root, &["status", "--porcelain"]).is_empty());
}

fn assert_required_history_in_head(root: &Path, id: &str) {
    let record = git(root, &["show", &format!("HEAD:tasks/{id}.md")]);
    let line = record
        .lines()
        .find_map(|line| line.strip_prefix("note_archives: "))
        .unwrap();
    let descriptors: Vec<String> = serde_json::from_str(line).unwrap();
    for descriptor in descriptors {
        let sha = descriptor.split(':').nth(1).unwrap();
        let path = format!("tasks/files/{id}/notes-{sha}.notes");
        assert_eq!(
            git(root, &["show", &format!("HEAD:{path}")]).as_bytes(),
            std::fs::read(root.join(path)).unwrap()
        );
    }
}

fn archive_recovery_publication(route: &str) {
    let mut env = TestEnv::new();
    let root = project(&mut env, "arc", true);
    let id = add(&env, &root, "Archive recovery");
    env.json(&root, &["note", &id, &"history A ".repeat(400)]);
    let a = env.json(&root, &["archive-notes", &id]);
    let a_hash = a["source_sha256"].as_str().unwrap();
    let mut interrupted_hash = a_hash.to_string();
    if route == "older-retry" {
        env.json(&root, &["archive-notes", &id, "--expect", a_hash]);
        env.json(&root, &["note", &id, &"history B ".repeat(400)]);
        interrupted_hash = env.json(&root, &["archive-notes", &id])["source_sha256"]
            .as_str()
            .unwrap()
            .to_string();
    }
    env.cmd(&root)
        .env("TASKS_ARCHIVE_STOP_AFTER", "after-record-switch")
        .args(["archive-notes", &id, "--expect", &interrupted_hash])
        .assert()
        .code(86);
    match route {
        "note" => {
            env.json(&root, &["note", &id, "ordinary write"]);
        }
        "older-retry" => {
            assert_eq!(
                env.json(&root, &["archive-notes", &id, "--expect", a_hash])["action"],
                "already_archived"
            );
        }
        "new-archive" => {
            let path = root.join(format!("tasks/{id}.md"));
            let raw = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                &path,
                format!(
                    "{raw}- 2026-10-31T00:00:00Z (fixture): {}\n",
                    "history B ".repeat(400)
                ),
            )
            .unwrap();
            let b = env.json(&root, &["archive-notes", &id]);
            env.json(
                &root,
                &[
                    "archive-notes",
                    &id,
                    "--expect",
                    b["source_sha256"].as_str().unwrap(),
                ],
            );
        }
        "move" => {
            let target = project(&mut env, "dst", true);
            let moved = env.json(&root, &["move", &id, "--to", "dst"]);
            assert_required_history_in_head(&target, moved["id"].as_str().unwrap());
        }
        _ => unreachable!(),
    }
    assert_required_history_in_head(&root, &id);
    assert!(git(&root, &["status", "--porcelain"]).is_empty(), "{route}");
}

#[test]
fn archive_publish_after_ordinary_note() {
    archive_recovery_publication("note");
}
#[test]
fn archive_publish_after_older_source_retry() {
    archive_recovery_publication("older-retry");
}
#[test]
fn archive_publish_after_new_archive() {
    archive_recovery_publication("new-archive");
}
#[test]
fn archive_publish_after_move() {
    archive_recovery_publication("move");
}

#[test]
fn prime_and_start_do_not_create_index_lock_when_stat_cache_is_dirty() {
    use std::os::unix::fs::MetadataExt;
    use std::time::Duration;

    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");
    let task_file = root.join(format!("tasks/{id}.md"));
    let old_mtime = std::fs::metadata(&task_file).unwrap().modified().unwrap();
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(&task_file)
        .unwrap();
    f.set_modified(old_mtime + Duration::from_secs(2)).unwrap();
    drop(f);

    let index_path = root.join(".git/index");
    let before = std::fs::metadata(&index_path).unwrap();
    let before_key = (before.ino(), before.modified().unwrap());

    let out = env.json(&root, &["prime"]);
    let warnings = out["warnings"].to_string();
    assert!(!warnings.contains("uncommitted task files"), "{out}");

    let after = std::fs::metadata(&index_path).unwrap();
    let after_key = (after.ino(), after.modified().unwrap());
    assert_eq!(
        after_key, before_key,
        "git status refreshed .git/index via optional index.lock"
    );
}

#[test]
fn tasks_note_recovers_after_failed_commit_and_unstage_on_same_task() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let id = add(&env, &root, "a task");

    // Pre-commit hook creates .git/index.lock and exits 1 so both `git commit` and `cleanup`'s `unstage` fail,
    // stranding the staged entry in `.git/index` and `.commit-<git_dir_hex>.staged`.
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\n: > .git/index.lock\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out1 = env.json(&root, &["note", &id, "first note"]);
    let warnings1 = out1["warnings"].to_string();
    assert!(warnings1.contains("commit_failed"), "{out1}");
    assert!(warnings1.contains("cleanup needs attention"), "{out1}");

    std::fs::remove_file(&hook).unwrap();
    std::fs::remove_file(root.join(".git/index.lock")).unwrap();
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]).trim(),
        format!("tasks/{id}.md")
    );

    // Subsequent `tasks note` on the same task modifies `tasks/<id>.md` on disk (`MM`),
    // recovers its own earlier staged entry via the ledger, and commits both notes cleanly.
    let out2 = env.json(&root, &["note", &id, "second note"]);
    assert_eq!(out2["warnings"], serde_json::json!([]), "{out2}");
    let head_body = git(&root, &["show", &format!("HEAD:tasks/{id}.md")]);
    assert!(
        head_body.contains("first note") && head_body.contains("second note"),
        "{head_body}"
    );
    assert!(clean(&root));
}

#[test]
fn tasks_commit_id_commits_only_that_task_and_its_attachments_leaving_other_tasks_and_config_uncommitted()
 {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let a = add(&env, &root, "task a");
    let b = add(&env, &root, "task b");
    let shot = env.home.path().join("shot.png");
    std::fs::write(&shot, "png").unwrap();
    env.json(&root, &["attach", &a, shot.to_str().unwrap()]);
    let cfg = root.join("tasks/.config.toml");
    let cfg_text = std::fs::read_to_string(&cfg).unwrap();
    std::fs::write(&cfg, format!("{cfg_text}# edited\n")).unwrap();

    let out = env.json(&root, &["commit", &a]);
    assert_eq!(out["committed"], 2, "{out}");
    assert_eq!(subjects(&root)[0], format!("chore(tasks): commit {a}"));
    let files = last_files(&root);
    assert!(
        files.contains(&format!("tasks/{a}.md"))
            && files.contains(&format!("tasks/files/{a}/shot.png"))
            && files.len() == 2,
        "{files:?}"
    );
    let status = git(&root, &["status", "--porcelain"]);
    assert!(
        status.contains(&format!("tasks/{b}.md")) && status.contains("tasks/.config.toml"),
        "{status}"
    );

    let out2 = env.json(&root, &["commit", &a]);
    assert_eq!(out2["committed"], 0, "{out2}");
    assert_eq!(out2["paths"], serde_json::json!([]), "{out2}");
}

#[test]
fn tasks_commit_id_commits_pending_task_and_attachment_deletions() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let a = add(&env, &root, "task a");
    let shot = env.home.path().join("shot.png");
    std::fs::write(&shot, "png").unwrap();
    env.json(&root, &["attach", &a, shot.to_str().unwrap()]);

    std::fs::remove_file(root.join(format!("tasks/{a}.md"))).unwrap();
    std::fs::remove_dir_all(root.join(format!("tasks/files/{a}"))).unwrap();

    let out = env.json(&root, &["commit", &a]);
    assert_eq!(out["committed"], 2, "{out}");
    assert_eq!(subjects(&root)[0], format!("chore(tasks): commit {a}"));
    assert!(clean(&root));

    let missing = env
        .cmd(&root)
        .args(["commit", "tst-deadbe"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let err: serde_json::Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "task_not_found", "{err}");
}

#[test]
fn tasks_commit_path_commits_document_with_explicit_message_and_accepts_absolute_path_under_records()
 {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let a = add(&env, &root, "task a");
    let spec = root.join("docs/specs/2026-11-07-example.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# spec\n").unwrap();
    let plan = root.join("docs/plans/2026-11-07-plan.md");
    std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
    std::fs::write(&plan, "# plan\n").unwrap();

    let out1 = env.json(
        &root,
        &[
            "commit",
            "docs/specs/2026-11-07-example.md",
            "-m",
            "docs(tasks): draft spec",
        ],
    );
    assert_eq!(out1["committed"], 1, "{out1}");
    assert_eq!(
        out1["paths"],
        serde_json::json!(["docs/specs/2026-11-07-example.md"])
    );
    assert_eq!(subjects(&root)[0], "docs(tasks): draft spec");
    assert_eq!(
        last_files(&root),
        vec!["docs/specs/2026-11-07-example.md".to_string()]
    );

    let out2 = env.json(
        &root,
        &[
            "commit",
            plan.to_str().unwrap(),
            "-m",
            "docs(tasks): draft plan",
        ],
    );
    assert_eq!(out2["committed"], 1, "{out2}");
    assert_eq!(
        out2["paths"],
        serde_json::json!(["docs/plans/2026-11-07-plan.md"])
    );
    assert_eq!(subjects(&root)[0], "docs(tasks): draft plan");
    assert_eq!(
        last_files(&root),
        vec!["docs/plans/2026-11-07-plan.md".to_string()]
    );

    let status = git(&root, &["status", "--porcelain"]);
    assert!(status.contains(&format!("tasks/{a}.md")), "{status}");
}

#[test]
fn nested_shared_records_project_commits_id_path_and_recovers_with_records_relative_paths() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = TestEnv::new();
    let (_dir, top, roots) = shared_repo(&mut env, &["aaa", "bbb"]);
    let aaa = &roots[0];
    let config = aaa.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("commit = true", "commit = false");
    std::fs::write(&config, text).unwrap();
    git(&top, &["add", "-A"]);
    git(&top, &["commit", "-q", "-m", "manual aaa"]);

    let a1 = add(&env, aaa, "first");
    let a2 = add(&env, aaa, "second");
    let spec = aaa.join("docs/specs/s.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# s\n").unwrap();

    // 1. Commit task ID in nested project: paths in output are relative to `aaa`, commit in git is `aaa/tasks/<a1>.md`
    let out1 = env.json(aaa, &["commit", &a1]);
    assert_eq!(out1["committed"], 1, "{out1}");
    assert_eq!(out1["paths"], serde_json::json!([format!("tasks/{a1}.md")]));
    assert_eq!(last_files(&top), vec![format!("aaa/tasks/{a1}.md")]);

    // 2. Commit records-relative path in nested project
    let out2 = env.json(aaa, &["commit", "docs/specs/s.md", "-m", "docs(aaa): spec"]);
    assert_eq!(out2["committed"], 1, "{out2}");
    assert_eq!(out2["paths"], serde_json::json!(["docs/specs/s.md"]));
    assert_eq!(last_files(&top), vec!["aaa/docs/specs/s.md".to_string()]);

    // 3. Stranded staging of `a2` in nested project: committing `a1` unstages `a2` and warns with records-relative path `tasks/<a2>.md`;
    //    committing `a2` directly when `a2` is stranded in the ledger commits `a2` with no warning!
    let hook = top.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\n: > .git/index.lock\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let fail = env.cmd(aaa).args(["commit", &a2]).output().unwrap();
    assert!(!fail.status.success());
    std::fs::remove_file(&hook).unwrap();
    std::fs::remove_file(top.join(".git/index.lock")).unwrap();
    assert_eq!(
        git(&top, &["diff", "--cached", "--name-only"]).trim(),
        format!("aaa/tasks/{a2}.md")
    );

    env.json(aaa, &["note", &a1, "touch a1"]);
    let out3 = env.json(aaa, &["commit", &a1]);
    assert_eq!(out3["committed"], 1, "{out3}");
    let w3 = out3["warnings"].to_string();
    assert!(
        w3.contains(&format!(
            "recovered staged changes from an earlier interrupted commit: tasks/{a2}.md; left uncommitted in the working tree; run `tasks commit {a2}` to commit them"
        )),
        "{out3}"
    );

    // Now strand `a2` again and run `tasks commit <a2>`: `recovery.unstaged` (`tasks/<a2>.md`) matches `paths`, so `warnings` is empty!
    std::fs::write(&hook, "#!/bin/sh\n: > .git/index.lock\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let fail2 = env.cmd(aaa).args(["commit", &a2]).output().unwrap();
    assert!(!fail2.status.success());
    std::fs::remove_file(&hook).unwrap();
    std::fs::remove_file(top.join(".git/index.lock")).unwrap();

    let out4 = env.json(aaa, &["commit", &a2]);
    assert_eq!(out4["committed"], 1, "{out4}");
    assert_eq!(out4["warnings"], serde_json::json!([]), "{out4}");
    assert!(clean(&top));
}

#[test]
fn tasks_commit_path_and_concurrent_task_writes_serialize_without_absorption() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", true);
    let mut ids = Vec::new();
    for i in 0..4 {
        ids.push(add(&env, &root, &format!("task {i}")));
        let spec = root.join(format!("docs/specs/s{i}.md"));
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        std::fs::write(&spec, format!("spec {i}\n")).unwrap();
    }

    let mut children = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        children.push(
            env.raw(&root)
                .args([
                    "commit",
                    &format!("docs/specs/s{i}.md"),
                    "-m",
                    &format!("docs: spec {i}"),
                ])
                .spawn()
                .unwrap(),
        );
        children.push(
            env.raw(&root)
                .args(["note", id, &format!("note {i}")])
                .spawn()
                .unwrap(),
        );
    }
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(parsed["warnings"], serde_json::json!([]), "{parsed}");
    }

    let log = git(&root, &["log", "--format=@%s", "--name-only", "-8"]);
    for block in log.split('@').filter(|b| !b.trim().is_empty()) {
        let mut lines = block.lines().filter(|l| !l.is_empty());
        let subj = lines.next().unwrap();
        let files: Vec<_> = lines.collect();
        if let Some(i) = subj.strip_prefix("docs: spec ") {
            assert_eq!(files, vec![format!("docs/specs/s{i}.md")], "{log}");
        } else if let Some(id) = subj.strip_prefix("chore(tasks): note ") {
            assert_eq!(files, vec![format!("tasks/{id}.md")], "{log}");
        }
    }
    assert!(clean(&root));
}

#[test]
fn tasks_commit_path_without_message_or_with_whitespace_only_message_is_refused() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let spec = root.join("docs/specs/2026-11-07-example.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# spec\n").unwrap();

    for args in [
        vec!["commit", "docs/specs/2026-11-07-example.md"],
        vec!["commit", "docs/specs/2026-11-07-example.md", "-m", "   "],
    ] {
        let out = env.cmd(&root).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(err["error"]["kind"], "validation", "{err}");
    }
}

#[test]
fn tasks_commit_mixed_id_and_path_commits_both_in_one_commit() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let a = add(&env, &root, "task a");
    let plan = root.join("docs/plans/2026-11-07-plan.md");
    std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
    std::fs::write(&plan, "# plan\n").unwrap();

    let out = env.json(
        &root,
        &[
            "commit",
            &a,
            "docs/plans/2026-11-07-plan.md",
            "-m",
            "docs(tasks): link plan",
        ],
    );
    assert_eq!(out["committed"], 2, "{out}");
    assert_eq!(subjects(&root)[0], "docs(tasks): link plan");
    let files = last_files(&root);
    assert!(
        files.contains(&format!("tasks/{a}.md"))
            && files.contains(&"docs/plans/2026-11-07-plan.md".to_string()),
        "{files:?}"
    );
}

#[test]
fn tasks_commit_directory_target_excludes_scratch_and_ignored_siblings_and_refuses_explicit_invalid_paths()
 {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    std::fs::write(root.join(".gitignore"), "docs/specs/*.ignored\n").unwrap();
    git(&root, &["add", ".gitignore"]);
    git(&root, &["commit", "-q", "-m", "gitignore"]);

    std::fs::create_dir_all(root.join("docs/specs")).unwrap();
    std::fs::write(root.join("docs/specs/valid.md"), "ok\n").unwrap();
    std::fs::write(root.join("docs/specs/draft.md.tmp"), "scratch\n").unwrap();
    std::fs::write(root.join("docs/specs/draft.edit.md"), "scratch\n").unwrap();
    std::fs::write(root.join("docs/specs/skip.ignored"), "ignored\n").unwrap();

    let out = env.json(&root, &["commit", "docs/specs", "-m", "docs: add specs"]);
    assert_eq!(out["committed"], 1, "{out}");
    assert_eq!(out["paths"], serde_json::json!(["docs/specs/valid.md"]));
    assert_eq!(last_files(&root), vec!["docs/specs/valid.md".to_string()]);
    assert!(root.join("docs/specs/draft.md.tmp").exists());
    assert!(root.join("docs/specs/draft.edit.md").exists());
    assert!(root.join("docs/specs/skip.ignored").exists());

    for bad in [
        "../outside.md",
        "docs/missing.md",
        "docs/specs/draft.md.tmp",
        "docs/specs/skip.ignored",
    ] {
        let res = env
            .cmd(&root)
            .args(["commit", bad, "-m", "m"])
            .output()
            .unwrap();
        assert_eq!(res.status.code(), Some(1), "expected failure for {bad}");
        let err: serde_json::Value = serde_json::from_slice(&res.stderr).unwrap();
        assert_eq!(err["error"]["kind"], "validation", "{bad}: {err}");
    }
}

#[test]
fn tasks_commit_routing_and_validation_are_order_independent() {
    let mut env = TestEnv::new();
    let aaa = project(&mut env, "aaa", false);
    let bbb = project(&mut env, "bbb", false);
    let a_id = add(&env, &aaa, "in aaa");
    let b_id = add(&env, &bbb, "in bbb");
    std::fs::create_dir_all(aaa.join("docs/specs")).unwrap();
    std::fs::write(aaa.join("docs/specs/x.md"), "x\n").unwrap();

    // 1. Trailing punctuation stripped and routes to bbb from aaa
    let out = env.json(&aaa, &["commit", &format!("{b_id}.")]);
    assert_eq!(out["committed"], 1, "{out}");
    assert_eq!(subjects(&bbb)[0], format!("chore(tasks): commit {b_id}"));

    // 2. Mixed project prefixes refused in both argument orders
    env.json(&bbb, &["note", &b_id, "edit b"]);
    for pair in [[&b_id, &a_id], [&a_id, &b_id]] {
        let res = env
            .cmd(&aaa)
            .args(["commit", pair[0], pair[1]])
            .output()
            .unwrap();
        assert_eq!(res.status.code(), Some(1));
        let err: serde_json::Value = serde_json::from_slice(&res.stderr).unwrap();
        assert_eq!(err["error"]["kind"], "validation", "{err}");
    }

    // 3. Cross-project ID + local path refused in both argument orders
    for pair in [
        vec!["commit", &b_id, "docs/specs/x.md", "-m", "m"],
        vec!["commit", "docs/specs/x.md", &b_id, "-m", "m"],
    ] {
        let res = env.cmd(&aaa).args(&pair).output().unwrap();
        assert_eq!(res.status.code(), Some(1));
        let err: serde_json::Value = serde_json::from_slice(&res.stderr).unwrap();
        assert_eq!(err["error"]["kind"], "validation", "{err}");
    }
}

#[test]
fn tasks_commit_expands_bare_hex_suffix_unless_file_exists_in_records() {
    let mut env = TestEnv::new();
    let root = project(&mut env, "tst", false);
    let id = add(&env, &root, "a task");
    let hex = id.strip_prefix("tst-").unwrap();

    // 1. No file named `<hex>` -> expands to `tst-<hex>`
    let out1 = env.json(&root, &["commit", hex]);
    assert_eq!(out1["committed"], 1, "{out1}");
    assert_eq!(out1["paths"], serde_json::json!([format!("tasks/{id}.md")]));
    assert_eq!(subjects(&root)[0], format!("chore(tasks): commit {id}"));

    // 2. File literally named `<hex>` exists under records -> treated as path target
    std::fs::write(root.join(hex), "literal hex file\n").unwrap();
    let out2 = env.json(&root, &["commit", hex, "-m", "chore: hex file"]);
    assert_eq!(out2["committed"], 1, "{out2}");
    assert_eq!(out2["paths"], serde_json::json!([hex]));
    assert_eq!(subjects(&root)[0], "chore: hex file");
}
