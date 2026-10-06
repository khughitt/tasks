mod common;
use common::TestEnv;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn registry(env: &TestEnv) -> toml::Value {
    let text =
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    toml::from_str(&text).unwrap()
}

/// A checkout at `base/<name>` whose `.worktrees` links to `base/work/<name>/.worktrees`,
/// the shape the work-link tool gives a synced checkout.
fn linked_checkout(base: &Path, name: &str) -> (PathBuf, PathBuf) {
    let root = base.join(name);
    let storage = base.join(format!("work/{name}/.worktrees"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&storage).unwrap();
    symlink(&storage, root.join(".worktrees")).unwrap();
    (root, storage)
}

/// Moves a linked checkout to `new` and relinks it as work-link would. With
/// `move_storage` false the storage stays put, so the new link dangles: the second
/// host's state between the synced move and its own storage move.
fn move_checkout(base: &Path, old: &str, new: &str, move_storage: bool) -> (PathBuf, PathBuf) {
    std::fs::rename(base.join(old), base.join(new)).unwrap();
    if move_storage {
        std::fs::rename(
            base.join(format!("work/{old}")),
            base.join(format!("work/{new}")),
        )
        .unwrap();
    }
    let root = base.join(new);
    let storage = base.join(format!("work/{new}/.worktrees"));
    std::fs::remove_file(root.join(".worktrees")).unwrap();
    symlink(&storage, root.join(".worktrees")).unwrap();
    (root, storage)
}

/// The cutover on the renaming host: rename at the old root, move, init --force.
fn cutover(env: &TestEnv, base: &Path) -> (String, PathBuf, PathBuf, PathBuf, PathBuf) {
    let (old_root, old_storage) = linked_checkout(base, "tack");
    env.json(&old_root, &["init", "--prefix", "tack"]);
    let id = id_of(env.json(&old_root, &["add", "Task"]));
    env.json(&old_root, &["rename", "tack", "hq"]);
    let (root, storage) = move_checkout(base, "tack", "hq", true);
    env.json(&root, &["init", "--prefix", "hq", "--force"]);
    (id, old_root, old_storage, root, storage)
}

#[test]
fn a_cutover_records_the_former_root_with_its_storage() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let (_, old_root, old_storage, _, storage) = cutover(&env, &base);
    let hq = &registry(&env)["locations"]["hq"];
    assert_eq!(hq["storage"].as_str().unwrap(), storage.to_str().unwrap());
    let former = hq["former"].as_array().unwrap();
    assert_eq!(former.len(), 1);
    assert_eq!(
        former[0]["root"].as_str().unwrap(),
        old_root.to_str().unwrap()
    );
    assert_eq!(
        former[0]["storage"].as_str().unwrap(),
        old_storage.to_str().unwrap()
    );
}

#[test]
fn an_init_without_a_worktrees_link_leaves_the_registry_shape_alone() {
    let mut env = TestEnv::new();
    env.init("sci");
    assert!(registry(&env).get("locations").is_none());
}

fn resolve(env: &TestEnv, dir: &Path, inputs: &[&str]) -> Vec<serde_json::Value> {
    let mut args = vec!["resolve"];
    args.extend_from_slice(inputs);
    env.json(dir, &args)["results"].as_array().unwrap().clone()
}

#[test]
fn every_id_and_path_form_resolves_through_a_cutover() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let (id, old_root, old_storage, root, _) = cutover(&env, &base);
    let hex = id.rsplit_once('-').unwrap().1;
    let gone = old_storage.join("gone/src/a.rs");
    let results = resolve(
        &env,
        &base,
        &[
            &id,
            "tack",
            old_root.join("tasks").to_str().unwrap(),
            gone.to_str().unwrap(),
        ],
    );
    assert_eq!(results[0]["id"], format!("hq-{hex}"));
    assert_eq!(results[0]["alias"], "tack");
    assert_eq!(results[0]["root"], root.to_str().unwrap());
    assert_eq!(results[1]["kind"], "prefix");
    assert_eq!(results[1]["prefix"], "hq");
    assert_eq!(results[2]["via"], "former_root");
    assert_eq!(results[2]["relpath"], "tasks");
    assert_eq!(results[3]["via"], "former_storage");
    assert_eq!(
        results[3]["worktree"],
        old_storage.join("gone").to_str().unwrap()
    );
    assert_eq!(results[3]["relpath"], "src/a.rs");
    for r in &results {
        assert_eq!(r["status"], "resolved", "{r}");
        assert_eq!(r["prefix"], "hq", "{r}");
    }
}

/// The second host: its own paths, a synced checkout already renamed, then --adopt.
/// `storage_first` moves this host's storage before adopting. tack's runbook adopts
/// first, which leaves the new link dangling.
fn second_host(
    base: &Path,
    pre_register_target: bool,
    storage_first: bool,
) -> (TestEnv, String, PathBuf, PathBuf, serde_json::Value) {
    let env = TestEnv::new();
    let (old_root, old_storage) = linked_checkout(base, "tack");
    env.json(&old_root, &["init", "--prefix", "tack"]);
    let old_id = id_of(env.json(&old_root, &["add", "Task"]));
    // What sync delivers: the files renamed by the first host.
    let new_id = old_id.replacen("tack-", "hq-", 1);
    let task = old_root.join(format!("tasks/{old_id}.md"));
    let text = std::fs::read_to_string(&task).unwrap();
    std::fs::write(
        old_root.join(format!("tasks/{new_id}.md")),
        text.replacen(&old_id, &new_id, 1),
    )
    .unwrap();
    std::fs::remove_file(task).unwrap();
    let config = old_root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("prefix = \"tack\"", "prefix = \"hq\""),
    )
    .unwrap();
    let (root, _) = move_checkout(base, "tack", "hq", storage_first);
    if pre_register_target {
        env.json(&root, &["init", "--prefix", "hq", "--force"]);
        let path = env.home.path().join(".config/tasks/projects.toml");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("\n[[locations.hq.former]]\nroot = \"/elsewhere\"\nuntil = \"2026-01-01\"\n");
        std::fs::write(&path, text).unwrap();
    }
    let out = env.json(&root, &["rename", "tack", "hq", "--adopt"]);
    (env, old_id, old_root, old_storage, out)
}

#[test]
fn a_host_that_adopts_gets_the_same_answers_with_its_own_storage() {
    for (pre_register_target, storage_first) in
        [(false, true), (true, true), (false, false), (true, false)]
    {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (env, old_id, old_root, old_storage, out) =
            second_host(&base, pre_register_target, storage_first);
        let gone = old_storage.join("wt/f");
        let results = resolve(
            &env,
            &base,
            &[
                &old_id,
                old_root.join("x").to_str().unwrap(),
                gone.to_str().unwrap(),
            ],
        );
        assert_eq!(results[0]["id"], old_id.replacen("tack-", "hq-", 1));
        assert_eq!(results[1]["via"], "former_root");
        assert_eq!(
            results[2]["via"], "former_storage",
            "pre={pre_register_target}"
        );
        let former = registry(&env)["locations"]["hq"]["former"]
            .as_array()
            .unwrap()
            .clone();
        let roots: Vec<_> = former
            .iter()
            .map(|f| f["root"].as_str().unwrap().to_string())
            .collect();
        if pre_register_target {
            assert!(roots.contains(&"/elsewhere".to_string()), "{roots:?}");
        }
        assert!(
            roots.contains(&old_root.to_str().unwrap().to_string()),
            "{roots:?}"
        );
        let old = former
            .iter()
            .find(|f| f["root"].as_str() == old_root.to_str())
            .unwrap();
        assert_eq!(
            old["storage"].as_str(),
            old_storage.to_str(),
            "the old storage lives under former"
        );
        let hq = &registry(&env)["locations"]["hq"];
        if storage_first {
            assert_eq!(
                hq["storage"].as_str(),
                base.join("work/hq/.worktrees").to_str()
            );
        } else {
            assert!(
                hq.get("storage").is_none(),
                "a dangling link leaves no current record: {hq}"
            );
            let warnings = out["warnings"].as_array().unwrap();
            assert!(
                warnings
                    .iter()
                    .any(|w| w.as_str().unwrap().contains("points at missing storage")),
                "{warnings:?}"
            );
        }
    }
}

#[test]
fn adoption_without_a_storage_record_keeps_the_root_and_warns() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    // A synced checkout already renamed to hq, and a registry that knew tack only at an
    // old root that is gone, with no storage record: the host skipped the pre-move step.
    let (root, _) = linked_checkout(&base, "hq");
    std::fs::create_dir_all(root.join("tasks")).unwrap();
    std::fs::write(root.join("tasks/.config.toml"), "prefix = \"hq\"\n").unwrap();
    let gone = base.join("tack");
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!("[projects]\ntack = {:?}\n", gone.to_str().unwrap()),
    )
    .unwrap();
    let out = env.json(&root, &["rename", "tack", "hq", "--adopt"]);
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("no known worktree storage")),
        "{warnings:?}"
    );
    let former = &registry(&env)["locations"]["hq"]["former"][0];
    assert_eq!(former["root"].as_str().unwrap(), gone.to_str().unwrap());
    assert!(former.get("storage").is_none());
}

#[test]
fn stdin_batch_keeps_order_and_reports_each_item() {
    let mut env = TestEnv::new();
    let dir = env.init("abcdef");
    env.init("other");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for prefix in ["abcdef", "other"] {
        text.push_str(&format!(
            "\n[[locations.{prefix}.former]]\nroot = \"/shared-gone\"\nuntil = \"2026-01-01\"\n"
        ));
    }
    std::fs::write(&path, text).unwrap();
    let out = env
        .cmd(&dir)
        .args(["resolve", "--stdin"])
        .write_stdin(
            "abcdef\r\n4f2a9c\n\nzz-123456\nbeef00\n/nowhere/at/all\n/shared-gone/x\nnot an id\n",
        )
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["results"].as_array().unwrap();
    let got: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| {
            (
                r["input"].as_str().unwrap(),
                r["kind"].as_str().unwrap(),
                r["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("abcdef", "prefix", "resolved"),
            ("4f2a9c", "invalid", "invalid"),
            ("", "invalid", "invalid"),
            ("zz-123456", "id", "unregistered"),
            ("beef00", "prefix", "unregistered"),
            ("/nowhere/at/all", "path", "unregistered"),
            ("/shared-gone/x", "path", "ambiguous"),
            ("not an id", "invalid", "invalid"),
        ]
    );
    assert_eq!(rows[3]["prefix"], "zz");
    assert_eq!(
        rows[6]["candidates"],
        serde_json::json!([
            {"prefix": "abcdef", "via": "former_root", "location": "/shared-gone"},
            {"prefix": "other", "via": "former_root", "location": "/shared-gone"}
        ])
    );
}

#[test]
fn a_path_in_an_outside_worktree_resolves_through_git_with_its_offset() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let main = base.join("m");
    std::fs::create_dir_all(main.join("sub/nested")).unwrap();
    env.json(&main.join("sub/nested"), &["init", "--prefix", "nested"]);
    env.json(&main, &["init", "--prefix", "parent"]);
    git(&main, &["init", "-q"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "init"]);
    let top = base.join("t");
    git(&main, &["worktree", "add", "-q", top.to_str().unwrap()]);
    std::fs::write(top.join("sub/nested/x"), "").unwrap();
    let r = &resolve(&env, &base, &[top.join("sub/nested/x").to_str().unwrap()])[0];
    assert_eq!(r["prefix"], "nested");
    assert_eq!(r["via"], "git");
    assert_eq!(r["location"], main.join("sub/nested").to_str().unwrap());
    assert_eq!(r["worktree"], top.to_str().unwrap());
    assert_eq!(r["relpath"], "sub/nested/x");
}

#[test]
fn a_malformed_locations_table_is_a_config_error() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[locations.gone]\nstorage = \"/s\"\n");
    std::fs::write(&path, text).unwrap();
    let out = env.cmd(&dir).args(["resolve", "sci"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "config");
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

#[test]
fn unavailable_cwd_only_invalidates_paths_that_need_it() {
    use std::os::unix::process::CommandExt;
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let gone = tempfile::tempdir().unwrap();
    let gone_name = std::ffi::CString::new(gone.path().as_os_str().as_encoded_bytes()).unwrap();
    let mut command = env.raw(gone.path());
    command.args([
        "resolve",
        "sci",
        "sci-123456",
        root.to_str().unwrap(),
        "./x",
    ]);
    // SAFETY: rmdir is async-signal-safe; the CString lives in the child closure.
    unsafe {
        command.pre_exec(move || {
            if libc::rmdir(gone_name.as_ptr()) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let out = command.output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["results"].as_array().unwrap();
    for row in &rows[..3] {
        assert_eq!(row["status"], "resolved");
    }
    assert_eq!(rows[3]["status"], "invalid");
    assert!(
        rows[3]["detail"]
            .as_str()
            .unwrap()
            .contains("working directory")
    );
}

#[test]
fn relative_directory_override_resolves_against_cwd() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("project");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    env.json(&root, &["init", "--prefix", "sci"]);
    let value = env.json(base.path(), &["-C", "project/sub", "resolve", "./x"]);
    assert_eq!(value["results"][0]["status"], "resolved");
    assert_eq!(value["results"][0]["relpath"], "sub/x");
}

#[test]
fn malformed_stdin_line_preserves_valid_neighbors() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let out = env
        .cmd(&dir)
        .args(["resolve", "--stdin"])
        .write_stdin(b"sci\n\xff\nzz-123456\n")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["results"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["status"], "resolved");
    assert_eq!(rows[1]["input"], "\u{fffd}");
    assert_eq!(rows[1]["status"], "invalid");
    assert!(rows[1]["detail"].as_str().unwrap().contains("UTF-8"));
    assert_eq!(rows[2]["status"], "unregistered");
}
