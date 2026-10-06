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
