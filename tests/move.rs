mod common;
use common::TestEnv;

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn moved_status_cannot_be_set_by_edit() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    let id = id_of(env.json(&dots, &["add", "T", "-p", "2"]));
    assert_eq!(
        env.fail(&dots, &["edit", &id, "--status", "moved"]),
        "validation"
    );
    let listed = env.json(&dots, &["list", "--status", "moved"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn a_flag_edit_keeps_moved_from() {
    let mut env = TestEnv::new();
    let wali = env.init("wali");
    let n = id_of(env.json(&wali, &["add", "T", "-p", "2"]));
    let path = wali.join(format!("tasks/{n}.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replacen("tags: []\n", "tags: []\nmoved_from: dots-00000a\n", 1),
    )
    .unwrap();
    env.json(&wali, &["edit", &n, "--title", "U", "-p", "1"]);
    assert_eq!(
        env.json(&wali, &["show", &n])["task"]["moved_from"],
        "dots-00000a"
    );
}
