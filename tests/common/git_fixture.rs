use std::path::Path;
use std::process::Command;

pub fn command(home: &Path, dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .env_clear()
        .envs(super::isolated_env(home))
        .current_dir(dir);
    command
}

/// Check both boundaries before any fixture mutation beyond initialization.
pub fn assert_repo(home: &Path, dir: &Path) {
    let read = |arg| {
        let out = command(home, dir)
            .args(["rev-parse", arg])
            .output()
            .unwrap();
        assert!(out.status.success(), "git rev-parse {arg}: {out:?}");
        Path::new(OsStr::from_bytes(
            out.stdout.strip_suffix(b"\n").expect("git path ends in LF"),
        ))
        .canonicalize()
        .unwrap()
    };
    let dir = dir.canonicalize().unwrap();
    assert_eq!(read("--show-toplevel"), dir);
    assert!(read("--absolute-git-dir").starts_with(&dir));
}

/// Run the named test again with hook selectors pointing only at owned foreign state.
/// Returns false in that child so the caller exercises its ordinary fixture helpers.
pub fn hostile_parent(test: &str) -> bool {
    if std::env::var_os("TASKS_GIT_FIXTURE_CHILD").is_some() {
        return false;
    }
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    let foreign = scratch.path().join("foreign");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&foreign).unwrap();
    let git = |args: &[&str]| {
        let out = command(&home, &foreign).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        out.stdout
    };
    git(&["init", "-q", "-b", "main"]);
    assert_repo(&home, &foreign);
    std::fs::write(foreign.join("README"), "foreign committed\n").unwrap();
    git(&["add", "README"]);
    git(&["commit", "-qm", "foreign"]);
    std::fs::write(foreign.join("README"), "foreign staged\n").unwrap();
    git(&["add", "README"]);
    let head = git(&["rev-parse", "HEAD"]);
    let index = std::fs::read(foreign.join(".git/index")).unwrap();
    let config = home.join(".gitconfig");
    // A host-style config-based hook (git 2.54+): a fixture that inherits global config
    // runs it at commit and fails, as a host identity hook did against moved records.
    let config_bytes = b"[user]\nname = foreign\nemail = foreign@example.invalid\n\
        [hook \"hostile\"]\nevent = pre-commit\ncommand = false\n";
    std::fs::write(&config, config_bytes).unwrap();

    let out = Command::new(std::env::current_exe().unwrap())
        .env_clear()
        .envs(super::isolated_env(&home))
        .env("TASKS_GIT_FIXTURE_CHILD", "1")
        .env("GIT_DIR", foreign.join(".git"))
        .env("GIT_COMMON_DIR", foreign.join(".git"))
        .env("GIT_WORK_TREE", &foreign)
        .env("GIT_INDEX_FILE", foreign.join(".git/index"))
        .env("GIT_CONFIG_GLOBAL", &config)
        .args(["--exact", test, "--nocapture"])
        .output()
        .unwrap();
    assert_eq!(
        git(&["rev-parse", "HEAD"]),
        head,
        "foreign HEAD changed: {out:?}"
    );
    assert_eq!(
        std::fs::read(foreign.join(".git/index")).unwrap(),
        index,
        "foreign index changed: {out:?}"
    );
    assert_eq!(
        std::fs::read(&config).unwrap(),
        config_bytes,
        "foreign global config changed: {out:?}"
    );
    assert!(out.status.success(), "fixture child: {out:?}");
    true
}
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
