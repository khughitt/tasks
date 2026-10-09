mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A code repository with one commit, at `base/name`.
fn code_repo(base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    std::fs::write(dir.join("README"), "code\n").unwrap();
    git(&dir, &["add", "README"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    dir.canonicalize().unwrap()
}

/// A project at `overlay/prefix`, registered, whose config declares `checkout` as written.
fn project(env: &TestEnv, overlay: &Path, prefix: &str, checkout: Option<&str>) -> PathBuf {
    let dir = overlay.join(prefix);
    std::fs::create_dir_all(&dir).unwrap();
    env.json(&dir, &["init", "--prefix", prefix]);
    if let Some(path) = checkout {
        let config = dir.join("tasks/.config.toml");
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str(&format!("\n[checkout]\npath = {path:?}\n"));
        std::fs::write(&config, text).unwrap();
    }
    dir.canonicalize().unwrap()
}

/// The standard layout: an overlay git repository holding `nexc` linked to `code`, with a
/// worktree of `code` under a `.worktrees` symlink into separate storage.
struct Layout {
    _base: tempfile::TempDir,
    base: PathBuf,
    overlay: PathBuf,
    nexc: PathBuf,
    code: PathBuf,
    wt: PathBuf,
}

fn layout(env: &TestEnv) -> Layout {
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let overlay = base.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    git(&overlay, &["init", "-q"]);
    let code = code_repo(&base, "code");
    let nexc = project(env, &overlay, "nexc", Some("../../code"));
    git(&overlay, &["add", "-A"]);
    git(&overlay, &["commit", "-q", "-m", "overlay"]);
    let storage = base.join("storage/code");
    std::fs::create_dir_all(&storage).unwrap();
    std::os::unix::fs::symlink(&storage, code.join(".worktrees")).unwrap();
    git(
        &code,
        &["worktree", "add", "-q", "-b", "wt", ".worktrees/wt"],
    );
    let wt = code.join(".worktrees/wt").canonicalize().unwrap();
    Layout {
        _base: holder,
        base,
        overlay,
        nexc,
        code,
        wt,
    }
}

#[test]
fn a_linked_checkout_opens_its_overlay_project_from_every_tree() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();
    let before = git(&l.code, &["status", "--porcelain", "--ignored"]);
    let wt_before = git(&l.wt, &["status", "--porcelain", "--ignored"]);
    for (n, dir) in [&l.code, &sub, &l.wt].into_iter().enumerate() {
        let added = env.json(dir, &["add", &format!("From tree {n}")]);
        let id = added["id"].as_str().unwrap().to_string();
        assert!(id.starts_with("nexc-"), "{dir:?} opened {id}");
        assert!(
            l.nexc.join(format!("tasks/{id}.md")).is_file(),
            "record lands in the overlay"
        );
    }
    let listed = env.json(&l.wt, &["list"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(
        git(&l.code, &["status", "--porcelain", "--ignored"]),
        before
    );
    assert_eq!(
        git(&l.wt, &["status", "--porcelain", "--ignored"]),
        wt_before
    );
}

#[test]
fn feedback_from_a_linked_checkout_carries_the_linked_prefix() {
    let mut env = TestEnv::new();
    let owner = env.init("tasks");
    let config = owner.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[feedback]\nscope = \"The tasks CLI.\"\n");
    std::fs::write(&config, text).unwrap();
    let l = layout(&env);
    let before = git(&l.code, &["status", "--porcelain", "--ignored"]);
    for dir in [&l.code, &l.wt] {
        let out = env.json(
            dir,
            &[
                "feedback",
                "--project",
                "tasks",
                "report from a code tree",
                "--category",
                "gap",
                "--new",
            ],
        );
        let shown = env.json(&owner, &["show", out["id"].as_str().unwrap()]);
        assert_eq!(
            shown["task"]["tags"],
            serde_json::json!(["feedback", "gap", "from:nexc"]),
            "{dir:?}"
        );
    }
    assert_eq!(
        git(&l.code, &["status", "--porcelain", "--ignored"]),
        before
    );
}

#[test]
fn feedback_from_an_unregistered_git_checkout_reports_as_unregistered() {
    let mut env = TestEnv::new();
    let owner = env.init("tasks");
    let config = owner.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[feedback]\nscope = \"The tasks CLI.\"\n");
    std::fs::write(&config, text).unwrap();
    // A linked project elsewhere makes the miss scan the registry's checkouts.
    let l = layout(&env);
    let clone = code_repo(&l.base, "clone");
    let before = git(&clone, &["status", "--porcelain", "--ignored"]);
    let out = env.json(
        &clone,
        &[
            "feedback",
            "--project",
            "tasks",
            "report from a team clone",
            "--category",
            "gap",
        ],
    );
    let shown = env.json(&owner, &["show", out["id"].as_str().unwrap()]);
    assert_eq!(
        shown["task"]["tags"],
        serde_json::json!(["feedback", "gap", "from:unregistered"])
    );
    assert_eq!(git(&clone, &["status", "--porcelain", "--ignored"]), before);

    // A registered config the checkout scan cannot read is an error, never unregistered.
    std::fs::write(l.nexc.join("tasks/.config.toml"), "prefix = [\n").unwrap();
    assert_eq!(
        env.fail(
            &clone,
            &[
                "feedback",
                "--project",
                "tasks",
                "report from a team clone",
                "--category",
                "gap"
            ]
        ),
        "config"
    );
}

#[test]
fn two_projects_declaring_one_checkout_is_a_conflict_naming_both() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "dup", Some(l.code.to_str().unwrap()));
    let out = env.cmd(&l.code).args(["list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "checkout_conflict");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains("nexc") && detail.contains("dup"),
        "{detail}"
    );
}

#[test]
fn an_in_tree_project_never_scans_the_registry() {
    let env = TestEnv::new();
    let l = layout(&env);
    // Two registered projects claim the in-tree project's own repository: if locate
    // scanned on a hit, every command here would be a checkout_conflict.
    let intree = code_repo(&l.base, "intree");
    env.json(&intree, &["init", "--prefix", "own"]);
    project(&env, &l.overlay, "aaa", Some(intree.to_str().unwrap()));
    project(&env, &l.overlay, "bbb", Some(intree.to_str().unwrap()));
    let added = env.json(&intree, &["add", "Here"]);
    assert!(added["id"].as_str().unwrap().starts_with("own-"));
}

#[test]
fn a_missing_declared_checkout_never_matches() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "gone", Some("../../nowhere"));
    let other = code_repo(&l.base, "other");
    assert_eq!(env.fail(&other, &["list"]), "no_project");
}

#[test]
fn a_malformed_config_in_a_registered_project_is_an_error_not_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::write(
        l.nexc.join("tasks/.config.toml"),
        "prefix = \"nexc\"\n[checkout]\npath = 7\n",
    )
    .unwrap();
    assert_eq!(env.fail(&l.code, &["list"]), "config");
}

#[test]
fn outside_git_a_miss_is_still_no_project_without_reading_the_registry() {
    let env = TestEnv::new();
    let plain = tempfile::tempdir().unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    std::fs::write(&registry, "not toml = [").unwrap();
    assert_eq!(env.fail(plain.path(), &["list"]), "no_project");
}

#[test]
fn an_absolute_checkout_path_matches_like_a_relative_one() {
    let env = TestEnv::new();
    let l = layout(&env);
    let abs = code_repo(&l.base, "abs");
    project(&env, &l.overlay, "absp", Some(abs.to_str().unwrap()));
    let added = env.json(&abs, &["add", "Absolute"]);
    assert!(added["id"].as_str().unwrap().starts_with("absp-"));
}

#[test]
fn a_checkout_declared_through_a_symlink_matches_its_real_path() {
    let env = TestEnv::new();
    let l = layout(&env);
    let real = code_repo(&l.base, "real");
    let alias = l.base.join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    project(&env, &l.overlay, "sym", Some(alias.to_str().unwrap()));
    let added = env.json(&real, &["add", "Through the alias"]);
    assert!(added["id"].as_str().unwrap().starts_with("sym-"));
}

#[test]
fn an_overlay_missing_on_this_host_is_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::rename(&l.nexc, l.base.join("moved-away")).unwrap();
    assert_eq!(env.fail(&l.code, &["list"]), "no_project");
}

fn porcelain(dir: &Path) -> String {
    git(dir, &["status", "--porcelain", "--ignored"])
}

fn string(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap()
}

#[test]
fn the_lifecycle_in_a_code_worktree_keeps_records_in_the_overlay() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join(".keep"), "").unwrap();
    git(&l.code, &["add", "src/deep/.keep"]);
    git(&l.code, &["commit", "-q", "-m", "sub"]);
    let before = porcelain(&l.code);
    let before_wt = porcelain(&l.wt);
    std::fs::create_dir_all(l.nexc.join("docs/specs")).unwrap();
    std::fs::write(l.nexc.join("docs/specs/topic.md"), "# Topic\n").unwrap();
    let shot = l.base.join("shot.png");
    std::fs::write(&shot, b"\x89PNG\r\n\x1a\n").unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    // From the main checkout, a subdirectory of it, and the worktree: the claim and park
    // name the working tree's top level, and the owner is that tree's branch.
    for (n, (tree, top, branch)) in [
        (&l.code, &l.code, "main"),
        (&sub, &l.code, "main"),
        (&l.wt, &l.wt, "wt"),
    ]
    .into_iter()
    .enumerate()
    {
        let top = top.to_str().unwrap();
        let title = format!("Linked work {n}");
        let id = env.json(tree, &["add", &title])["id"]
            .as_str()
            .unwrap()
            .to_string();
        env.json(tree, &["start", &id]);
        let shown = env.json(tree, &["show", &id]);
        assert_eq!(
            shown["claim"]["worktree"], top,
            "{n}: the claim names the code tree"
        );
        assert_eq!(
            shown["task"]["owner"], branch,
            "{n}: the owner reads the code branch"
        );
        env.json(tree, &["note", &id, "a note"]);
        env.json(tree, &["edit", &id, "--spec", "topic"]);
        env.json(
            tree,
            &["attach", &id, shot.to_str().unwrap(), "--caption", "a shot"],
        );

        // An agent-waiting park: next and prime list it with the code tree to open.
        env.json(tree, &["park", &id, "resume here"]);
        for view in [env.json(&l.code, &["next"]), env.json(&l.code, &["prime"])] {
            let text = string(&view);
            assert!(text.contains(&id) && text.contains(top), "{n}: {text}");
            assert!(
                !string(&view["warnings"]).contains("unavailable"),
                "{n}: {text}"
            );
        }
        let found = env.json(elsewhere.path(), &["show", &id]);
        assert_eq!(found["park"]["worktree"], top, "{n}");
        assert_eq!(found["task"]["spec"], "docs/specs/topic.md", "{n}");

        // A quiet park, read from outside every project: quiet resolves the recorded code
        // tree through the link (`open_recorded`), so the row carries the record's title
        // and no warning calls the checkout unavailable.
        env.json(tree, &["start", &id]);
        env.json(
            tree,
            &[
                "park",
                &id,
                "run it",
                "--reason",
                "quiet",
                "--waiting-on",
                "user",
                "--minutes",
                "5",
            ],
        );
        let quiet = env.json(elsewhere.path(), &["quiet", "--project", "nexc"]);
        let text = string(&quiet);
        assert!(
            text.contains(&id) && text.contains(top) && text.contains(&title),
            "{n}: {text}"
        );
        assert!(
            !string(&quiet["warnings"]).contains("unavailable"),
            "{n}: {text}"
        );

        env.json(tree, &["start", &id]);
        env.json(tree, &["done", &id, "landed"]);
        assert!(l.nexc.join(format!("tasks/{id}.md")).is_file(), "{n}");
        assert!(
            l.nexc.join(format!("tasks/files/{id}")).is_dir(),
            "{n}: attachments in the overlay"
        );
    }
    assert_eq!(
        porcelain(&l.code),
        before,
        "nothing written into the code repository"
    );
    assert_eq!(
        porcelain(&l.wt),
        before_wt,
        "nothing written into the code worktree"
    );
}

#[test]
fn open_recorded_reads_a_recorded_code_tree_through_the_link() {
    // The unit seam under quiet, next, prime, and show's fallback: with the overlay root
    // never named, a recorded code worktree still opens the overlay project.
    let env = TestEnv::new();
    let l = layout(&env);
    let id = env.json(&l.wt, &["add", "Recorded"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.wt, &["start", &id]);
    env.json(
        &l.wt,
        &[
            "park",
            &id,
            "run it",
            "--reason",
            "quiet",
            "--waiting-on",
            "user",
            "--minutes",
            "5",
        ],
    );
    let elsewhere = tempfile::tempdir().unwrap();
    let quiet = env.json(elsewhere.path(), &["quiet"]);
    let text = string(&quiet);
    assert!(text.contains(&id) && text.contains("Recorded"), "{text}");
    assert!(
        !string(&quiet["warnings"]).contains("unavailable"),
        "{text}"
    );
}

#[test]
fn a_holders_write_moves_its_one_claim_between_code_trees() {
    let env = TestEnv::new();
    let l = layout(&env);
    let id = env.json(&l.code, &["add", "Moving"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&l.code, &["start", &id]);
    assert_eq!(
        env.json(&l.code, &["show", &id])["claim"]["worktree"],
        l.code.to_str().unwrap()
    );
    env.json(&l.wt, &["note", &id, "from the worktree"]);
    let moved = env.json(&l.code, &["show", &id]);
    assert_eq!(
        moved["claim"]["worktree"],
        l.wt.to_str().unwrap(),
        "the claim follows its holder"
    );
    env.json(&l.code, &["note", &id, "back in the main checkout"]);
    assert_eq!(
        env.json(&l.wt, &["show", &id])["claim"]["worktree"],
        l.code.to_str().unwrap()
    );
    let claims = string(&env.json(&l.code, &["claims"]));
    assert_eq!(
        claims.matches(&format!("\"{id}\"")).count(),
        1,
        "one claim, moved: {claims}"
    );
}

#[test]
fn prime_warns_only_about_this_projects_directory_in_the_overlay() {
    let env = TestEnv::new();
    let l = layout(&env);
    let other = project(&env, &l.overlay, "oth", None);
    env.json(&other, &["add", "Uncommitted elsewhere"]);
    env.json(&l.wt, &["add", "Uncommitted here"]);
    let warnings = string(&env.json(&l.wt, &["prime"])["warnings"]);
    assert!(warnings.contains("nexc-"), "{warnings}");
    assert!(!warnings.contains("oth-"), "{warnings}");
}

#[test]
fn where_answers_linked_in_tree_and_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    let linked = env.json(&l.wt, &["where", "--json"]);
    assert_eq!(linked["project"]["prefix"], "nexc");
    assert_eq!(linked["project"]["root"], l.nexc.to_str().unwrap());
    assert_eq!(linked["project"]["checkout"], l.code.to_str().unwrap());
    assert_eq!(linked["project"]["linked"], true);
    assert_eq!(
        linked["holds_records"], false,
        "the code repository keeps no records"
    );
    assert_eq!(linked["warnings"], serde_json::json!([]));

    let intree = env.json(&l.nexc, &["where"]);
    assert_eq!(intree["project"]["linked"], false);
    assert!(
        intree["project"]
            .as_object()
            .unwrap()
            .contains_key("checkout")
    );
    assert!(intree["project"]["checkout"].is_null());
    assert_eq!(
        intree["holds_records"], true,
        "the overlay holds a registered root"
    );

    let plain = tempfile::tempdir().unwrap();
    let none = env.json(plain.path(), &["where"]);
    assert!(none["project"].is_null());
    assert_eq!(none["holds_records"], false);
}

#[test]
fn where_takes_a_path_a_file_and_refuses_a_missing_one() {
    let env = TestEnv::new();
    let l = layout(&env);
    let elsewhere = tempfile::tempdir().unwrap();
    let by_path = env.json(elsewhere.path(), &["where", l.wt.to_str().unwrap()]);
    assert_eq!(by_path["project"]["prefix"], "nexc");
    let by_file = env.json(&l.code, &["where", "README"]);
    assert_eq!(
        by_file["project"]["prefix"], "nexc",
        "a relative file answers for its directory"
    );
    assert_eq!(env.fail(&l.code, &["where", "no/such/path"]), "io");
}

#[test]
fn where_never_crosses_into_an_enclosing_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    let parent = code_repo(&l.base, "parent");
    env.json(&parent, &["init", "--prefix", "par"]);
    git(&parent, &["add", "-A"]);
    git(&parent, &["commit", "-q", "-m", "records"]);
    let nested = code_repo(&parent, "vendor/clone");
    let inside = env.json(&nested, &["where"]);
    assert!(
        inside["project"].is_null(),
        "a nested foreign clone is no project: {inside}"
    );
    assert_eq!(inside["holds_records"], false);
    let sub = parent.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    assert_eq!(env.json(&sub, &["where"])["project"]["prefix"], "par");
}

#[test]
fn holds_records_needs_a_config_tracked_in_head() {
    let env = TestEnv::new();
    let l = layout(&env);
    let repo = code_repo(&l.base, "unregistered");
    std::fs::create_dir_all(repo.join("tasks")).unwrap();
    std::fs::write(repo.join("tasks/.config.toml"), "prefix = \"unr\"\n").unwrap();
    assert_eq!(
        env.json(&repo, &["where"])["holds_records"],
        false,
        "untracked"
    );
    git(&repo, &["add", "tasks/.config.toml"]);
    assert_eq!(
        env.json(&repo, &["where"])["holds_records"],
        false,
        "staged only"
    );
    git(&repo, &["commit", "-q", "-m", "records"]);
    assert_eq!(
        env.json(&repo, &["where"])["holds_records"],
        true,
        "tracked in HEAD"
    );
}

#[test]
fn holds_records_refuses_a_tracked_tree_or_symlink_at_the_config_path() {
    let env = TestEnv::new();
    let l = layout(&env);
    // A directory named like the config: `git cat-file -e` would accept its tree object.
    let tree = code_repo(&l.base, "tree");
    std::fs::create_dir_all(tree.join("tasks/.config.toml")).unwrap();
    std::fs::write(tree.join("tasks/.config.toml/inner"), "x\n").unwrap();
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "a tree at the config path"]);
    assert_eq!(
        env.json(&tree, &["where"])["holds_records"],
        false,
        "a tree is not a config"
    );
    let link = code_repo(&l.base, "link");
    std::fs::create_dir_all(link.join("tasks")).unwrap();
    std::fs::write(link.join("real.toml"), "prefix = \"lnk\"\n").unwrap();
    std::os::unix::fs::symlink("../real.toml", link.join("tasks/.config.toml")).unwrap();
    git(&link, &["add", "-A"]);
    git(
        &link,
        &["commit", "-q", "-m", "a symlink at the config path"],
    );
    assert_eq!(
        env.json(&link, &["where"])["holds_records"],
        false,
        "a symlink is not a config"
    );
    let unborn = l.base.join("unborn");
    std::fs::create_dir_all(&unborn).unwrap();
    git(&unborn, &["init", "-q"]);
    assert_eq!(
        env.json(&unborn, &["where"])["holds_records"],
        false,
        "no HEAD tracks nothing"
    );
}

#[test]
fn a_corrupt_head_ref_is_an_error_not_an_empty_repository() {
    let env = TestEnv::new();
    let l = layout(&env);
    // A malformed loose branch ref.
    let loose = code_repo(&l.base, "loose");
    std::fs::write(loose.join(".git/refs/heads/main"), "garbage\n").unwrap();
    assert_eq!(env.fail(&loose, &["where"]), "io", "a malformed loose ref");
    // A corrupt packed-refs entry for the branch.
    let packed = code_repo(&l.base, "packed");
    git(&packed, &["pack-refs", "--all"]);
    let refs = packed.join(".git/packed-refs");
    let text = std::fs::read_to_string(&refs).unwrap();
    let broken: String = text
        .lines()
        .map(|line| match line.split_once(' ') {
            Some((_, "refs/heads/main")) => "zzzz refs/heads/main".to_string(),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&refs, broken + "\n").unwrap();
    assert_eq!(
        env.fail(&packed, &["where"]),
        "io",
        "a corrupt packed-refs entry"
    );
}

/// `tasks <args>` in `dir` fails with an `io` error; its detail.
fn io_detail(env: &TestEnv, dir: &Path, args: &[&str]) -> String {
    io_error(&env.cmd(dir).args(args).output().unwrap())
}

/// A failed run's `io` error detail.
fn io_error(out: &std::process::Output) -> String {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "io", "{error}");
    error["error"]["detail"].as_str().unwrap().to_string()
}

#[test]
fn an_empty_ancestor_dot_git_is_named_with_how_to_clear_it() {
    let env = TestEnv::new();
    let scratch = tempfile::tempdir().unwrap();
    let base = scratch.path().canonicalize().unwrap();
    let marker = base.join(".git");
    std::fs::create_dir(&marker).unwrap();
    let child = base.join("a/b");
    std::fs::create_dir_all(&child).unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let registry_before = std::fs::read(&registry).ok();

    for args in [&["where"][..], &["init", "--prefix", "oops"]] {
        let detail = io_detail(&env, &child, args);
        assert!(
            detail.contains(&format!("{}, is an empty directory", marker.display())),
            "{args:?}: {detail}"
        );
        assert!(
            detail.contains(&format!("rmdir {}", marker.display())),
            "{args:?}: {detail}"
        );
    }
    assert!(!child.join("tasks").exists());
    assert_eq!(std::fs::read(&registry).ok(), registry_before);

    std::fs::remove_dir(&marker).unwrap();
    assert!(env.json(&child, &["where"])["project"].is_null());
    git(&child, &["init", "-q"]);
    assert!(
        env.json(&child, &["where"])["project"].is_null(),
        "a healthy unborn repository is no project"
    );
}

#[test]
fn a_broken_ancestor_dot_git_is_named_for_inspection_never_removal() {
    let env = TestEnv::new();
    let scratch = tempfile::tempdir().unwrap();
    let base = scratch.path().canonicalize().unwrap();
    let marker = base.join(".git");
    std::fs::create_dir(&marker).unwrap();
    std::fs::write(marker.join("config"), "kept\n").unwrap();
    let child = base.join("a/b");
    std::fs::create_dir_all(&child).unwrap();
    let inspect = format!("git -C {} status", base.display());

    let detail = io_detail(&env, &child, &["where"]);
    assert!(
        detail.contains(&format!(
            "{}, is not a repository git can read",
            marker.display()
        )),
        "{detail}"
    );
    assert!(detail.contains(&inspect), "{detail}");
    assert!(
        !detail.contains("empty") && !detail.contains("rmdir"),
        "{detail}"
    );

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o000)).unwrap();
    let out = env.cmd(&child).arg("where").output().unwrap();
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let detail = io_error(&out);
    assert!(
        detail.contains(&format!("{}, could not be read", marker.display())),
        "{detail}"
    );
    assert!(detail.contains(&inspect), "{detail}");
    assert!(
        !detail.contains("empty") && !detail.contains("rmdir"),
        "{detail}"
    );
    assert_eq!(std::fs::read(marker.join("config")).unwrap(), b"kept\n");
}

#[test]
fn a_git_dir_override_is_not_blamed_on_a_healthy_dot_git() {
    let env = TestEnv::new();
    let l = layout(&env);
    let missing = l.base.join("missing-git-dir");
    let out = env
        .cmd(&l.code)
        .arg("where")
        .env("GIT_DIR", &missing)
        .output()
        .unwrap();
    let detail = io_error(&out);
    assert!(detail.contains(missing.to_str().unwrap()), "{detail}");
    assert!(!detail.contains("the nearest .git"), "{detail}");
}

#[test]
fn where_reports_resolver_errors_as_errors() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "dup", Some(l.code.to_str().unwrap()));
    assert_eq!(env.fail(&l.wt, &["where", "--json"]), "checkout_conflict");
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::write(
        l.nexc.join("tasks/.config.toml"),
        "prefix = \"nexc\"\n[checkout]\nbogus = 1\n",
    )
    .unwrap();
    assert_eq!(env.fail(&l.code, &["where"]), "config");
}

#[test]
fn where_with_the_overlay_missing_is_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::rename(&l.nexc, l.base.join("moved-away")).unwrap();
    assert!(env.json(&l.code, &["where"])["project"].is_null());
}

#[test]
fn init_checkout_writes_a_relative_link_outside_the_code_tree() {
    let env = TestEnv::new();
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let code = code_repo(&base, "my code");
    let before = porcelain(&code);
    let dir = base.join("overlay/mine");
    std::fs::create_dir_all(&dir).unwrap();
    env.json(
        &dir,
        &["init", "--prefix", "mine", "--checkout", "../../my code"],
    );
    let config = std::fs::read_to_string(dir.join("tasks/.config.toml")).unwrap();
    assert!(
        config.contains("[checkout]") && config.contains("path = \"../../my code\""),
        "{config}"
    );
    assert!(dir.join("docs/specs").is_dir() && dir.join("docs/plans").is_dir());
    assert_eq!(
        porcelain(&code),
        before,
        "init wrote nothing into the checkout"
    );
    assert_eq!(env.json(&code, &["where"])["project"]["prefix"], "mine");
}

#[test]
fn plain_init_refuses_every_tree_of_a_linked_checkout() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let before = porcelain(&l.code);
    let before_wt = porcelain(&l.wt);
    for (n, dir) in [&l.code, &sub, &l.wt].into_iter().enumerate() {
        let out = env
            .cmd(dir)
            .args(["init", "--prefix", "intr"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{n}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config", "{n}");
        assert!(
            error["error"]["detail"].as_str().unwrap().contains("nexc"),
            "{n}: names the linked project"
        );
        assert!(!dir.join("tasks").exists(), "{n}: nothing created");
    }
    assert_eq!(porcelain(&l.code), before);
    assert_eq!(porcelain(&l.wt), before_wt);
    assert!(
        env.json(&l.nexc, &["projects"])["projects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["prefix"] != "intr")
    );
}

#[test]
fn init_checkout_refuses_what_it_cannot_link() {
    let env = TestEnv::new();
    let l = layout(&env);
    let fresh = |name: &str| {
        let dir = l.overlay.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let plain = l.base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let intree = code_repo(&l.base, "intree");
    env.json(&intree, &["init", "--prefix", "own"]);
    let inside = intree.join("inner");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(l.code.join("src")).unwrap();
    for (name, checkout, kind) in [
        ("p1", l.wt.to_str().unwrap(), "config"), // a worktree, not the main checkout
        ("p2", l.code.join("src").to_str().unwrap(), "config"), // not the top level
        ("p3", plain.to_str().unwrap(), "config"), // not a git repository
        ("p4", intree.to_str().unwrap(), "config"), // keeps records in-tree
        ("p5", l.code.to_str().unwrap(), "checkout_conflict"), // nexc already declares it
    ] {
        let dir = fresh(name);
        assert_eq!(
            env.fail(&dir, &["init", "--prefix", name, "--checkout", checkout]),
            kind,
            "{name}"
        );
        assert!(
            !dir.join("tasks").exists(),
            "{name}: a refused init writes nothing"
        );
    }
    // The project directory inside a worktree of the very repository passed as
    // --checkout, that worktree stored outside the checkout's tree: no project links the
    // repository yet, and containment misses it; repository identity does not.
    let other = code_repo(&l.base, "other");
    let other_store = l.base.join("storage/other");
    std::fs::create_dir_all(&other_store).unwrap();
    std::os::unix::fs::symlink(&other_store, other.join(".worktrees")).unwrap();
    git(
        &other,
        &["worktree", "add", "-q", "-b", "ow", ".worktrees/ow"],
    );
    let in_wt = other
        .join(".worktrees/ow")
        .canonicalize()
        .unwrap()
        .join("overlay-here");
    std::fs::create_dir_all(&in_wt).unwrap();
    assert!(
        !in_wt.starts_with(&other),
        "the worktree really is outside the checkout's tree"
    );
    assert_eq!(
        env.fail(
            &in_wt,
            &[
                "init",
                "--prefix",
                "p7",
                "--checkout",
                other.to_str().unwrap()
            ]
        ),
        "config",
        "a project directory in a worktree of its own checkout's repository"
    );
    assert!(!in_wt.join("tasks").exists());
    // An in-tree project registered from a worktree stored outside the repository's tree,
    // with no config in the main checkout: linking the main checkout would give one
    // repository two project identities.
    let split = code_repo(&l.base, "split");
    let split_store = l.base.join("storage/split");
    std::fs::create_dir_all(&split_store).unwrap();
    std::os::unix::fs::symlink(&split_store, split.join(".worktrees")).unwrap();
    git(
        &split,
        &["worktree", "add", "-q", "-b", "sw", ".worktrees/sw"],
    );
    let split_wt = split.join(".worktrees/sw").canonicalize().unwrap();
    env.json(&split_wt, &["init", "--prefix", "spl"]);
    assert!(
        !split.join("tasks/.config.toml").exists(),
        "main holds no config"
    );
    let dir = fresh("p8");
    let out = env
        .cmd(&dir)
        .args([
            "init",
            "--prefix",
            "p8",
            "--checkout",
            split.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"].as_str().unwrap().contains("spl"),
        "names the registered project"
    );
    assert!(!dir.join("tasks").exists());
    let nested = code_repo(&l.nexc, "nested");
    let dir = fresh("p6");
    assert_eq!(
        env.fail(
            &dir,
            &[
                "init",
                "--prefix",
                "p6",
                "--checkout",
                nested.to_str().unwrap()
            ]
        ),
        "config",
        "a checkout inside a registered project's tree"
    );
}

#[test]
fn init_checkout_refuses_a_head_tracked_config_deleted_locally() {
    let env = TestEnv::new();
    let l = layout(&env);
    let code = code_repo(&l.base, "records-deleted");
    std::fs::create_dir_all(code.join("tasks")).unwrap();
    let config = code.join("tasks/.config.toml");
    std::fs::write(&config, "prefix = \"old\"\n").unwrap();
    git(&code, &["add", "tasks/.config.toml"]);
    git(&code, &["commit", "-q", "-m", "records"]);
    std::fs::remove_file(config).unwrap();
    let located = env.json(&code, &["where"]);
    assert!(located["project"].is_null());
    assert_eq!(located["holds_records"], true);

    let overlay = l.overlay.join("refused");
    std::fs::create_dir_all(&overlay).unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    let registry_before = std::fs::read(&registry).unwrap();
    let code_before = porcelain(&code);
    assert_eq!(
        env.fail(
            &overlay,
            &[
                "init",
                "--prefix",
                "ref",
                "--checkout",
                code.to_str().unwrap()
            ]
        ),
        "config"
    );
    assert_eq!(std::fs::read_dir(&overlay).unwrap().count(), 0);
    assert_eq!(std::fs::read(registry).unwrap(), registry_before);
    assert_eq!(porcelain(&code), code_before);
}

#[test]
fn corrupt_head_discovery_errors_without_writing_into_linked_checkouts() {
    for worktree in [false, true] {
        let env = TestEnv::new();
        let l = layout(&env);
        let root = if worktree { &l.wt } else { &l.code };
        let sub = root.join("src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        let head =
            PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]).trim()).join("HEAD");
        let valid_head = std::fs::read(&head).unwrap();
        let before = porcelain(root);
        let registry = env.home.path().join(".config/tasks/projects.toml");
        let registry_before = std::fs::read(&registry).unwrap();
        std::fs::write(&head, "garbage\n").unwrap();

        for dir in [root, &sub] {
            let located = env.cmd(dir).arg("where").output().unwrap();
            let initialized = env
                .cmd(dir)
                .args(["init", "--prefix", "oops"])
                .output()
                .unwrap();
            assert!(
                located.status.code() == Some(1) && initialized.status.code() == Some(1),
                "where: {located:?}; init: {initialized:?}"
            );
            for out in [located, initialized] {
                assert!(out.stdout.is_empty());
                let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
                assert_eq!(error["error"]["kind"], "io");
                let detail = error["error"]["detail"].as_str().unwrap();
                // git names a broken gitfile itself; the main tree's marker is named by tasks.
                let named = if worktree {
                    format!("{}", root.join(".git").display())
                } else {
                    format!("{}, is not a repository", root.join(".git").display())
                };
                assert!(detail.contains(&named), "{detail}");
                assert!(!detail.contains("rmdir"), "{detail}");
            }
            assert!(!dir.join("tasks").exists());
            assert_eq!(std::fs::read(&registry).unwrap(), registry_before);
            assert_eq!(std::fs::read(&head).unwrap(), b"garbage\n");
        }
        std::fs::write(&head, valid_head).unwrap();
        assert_eq!(porcelain(root), before);
    }
}

#[test]
fn projects_rows_carry_checkout_only_when_linked() {
    let env = TestEnv::new();
    let l = layout(&env);
    let other = project(&env, &l.overlay, "oth", None);
    let rows = env.json(&other, &["projects"])["projects"]
        .as_array()
        .unwrap()
        .clone();
    let row = |p: &str| rows.iter().find(|r| r["prefix"] == p).unwrap().clone();
    assert_eq!(row("nexc")["checkout"], l.code.to_str().unwrap());
    assert!(!row("oth").as_object().unwrap().contains_key("checkout"));
}

#[test]
fn check_reports_a_declared_checkout_that_is_missing() {
    let env = TestEnv::new();
    let l = layout(&env);
    let gone = project(&env, &l.overlay, "gone", Some("../../nowhere"));
    let report = env.check(&gone);
    let text = serde_json::to_string(&report).unwrap();
    assert!(
        text.contains("checkout_missing") && text.contains("nowhere"),
        "{text}"
    );
    assert_eq!(env.check(&l.nexc)["warnings"], serde_json::json!([]));
}

#[test]
fn resolve_matches_a_linked_checkout() {
    let env = TestEnv::new();
    let l = layout(&env);
    let out = env.json(
        &l.base,
        &["resolve", l.code.join("README").to_str().unwrap()],
    );
    let text = serde_json::to_string(&out).unwrap();
    assert!(
        text.contains("\"nexc\"") && text.contains("\"checkout\""),
        "{text}"
    );
}

#[test]
fn resolve_follows_a_linked_checkouts_worktree_storage_after_removal() {
    let env = TestEnv::new();
    let l = layout(&env);
    let physical = l.base.join("storage/code/wt");
    let storage = l.base.join("storage/code");
    // A live worktree answers through the storage too, as a registered root's does.
    let live = env.json(&l.base, &["resolve", l.wt.join("README").to_str().unwrap()]);
    assert_eq!(live["results"][0]["via"], "storage", "{live}");
    assert_eq!(
        live["results"][0]["worktree"],
        physical.to_str().unwrap(),
        "{live}"
    );
    git(&l.code, &["worktree", "remove", ".worktrees/wt"]);
    assert!(!physical.exists());
    for path in [l.code.join(".worktrees/wt/src/x"), physical.join("src/x")] {
        let out = env.json(&l.base, &["resolve", path.to_str().unwrap()]);
        let row = &out["results"][0];
        assert_eq!(row["status"], "resolved", "{path:?}: {out}");
        assert_eq!(row["prefix"], "nexc", "{path:?}: {out}");
        assert_eq!(row["via"], "storage", "{path:?}: {out}");
        assert_eq!(row["relpath"], "src/x", "{path:?}: {out}");
        assert_eq!(
            row["location"],
            storage.to_str().unwrap(),
            "{path:?}: {out}"
        );
        assert_eq!(
            row["worktree"],
            physical.to_str().unwrap(),
            "{path:?}: {out}"
        );
    }
}

#[test]
fn resolve_warns_when_a_linked_checkouts_worktrees_cannot_be_read() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::remove_file(l.code.join(".worktrees")).unwrap();
    std::os::unix::fs::symlink(".worktrees", l.code.join(".worktrees")).unwrap();
    let out = env.json(
        &l.base,
        &["resolve", l.code.join("README").to_str().unwrap()],
    );
    assert_eq!(out["results"][0]["prefix"], "nexc", "{out}");
    let warnings = out["warnings"].to_string();
    assert!(
        warnings.contains("nexc: cannot read")
            && warnings.contains("worktree storage is not matched"),
        "{out}"
    );
}

#[test]
fn resolve_keeps_every_valid_checkout_when_one_project_is_broken() {
    let env = TestEnv::new();
    let l = layout(&env);
    let broken = project(&env, &l.overlay, "brk", None);
    std::fs::write(
        broken.join("tasks/.config.toml"),
        "prefix = \"brk\"\n[checkout]\npath = 7\n",
    )
    .unwrap();
    let out = env.json(
        &l.base,
        &["resolve", l.code.join("README").to_str().unwrap()],
    );
    let text = serde_json::to_string(&out).unwrap();
    assert!(
        text.contains("\"nexc\"") && text.contains("\"checkout\""),
        "nexc still resolves: {text}"
    );
    let warnings = serde_json::to_string(&out["warnings"]).unwrap();
    assert!(
        warnings.contains("brk"),
        "one warning names the broken project: {warnings}"
    );
    assert!(!warnings.contains("nexc"), "{warnings}");
}
