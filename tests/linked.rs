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
