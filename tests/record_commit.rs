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
