mod common;
use common::{TestEnv, docs::DocFixture};

#[test]
fn external_doc_add_canonicalizes_and_reuses_body_for_step() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let created = env.json(
        &dir,
        &[
            "add",
            "External",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    );
    let id = created["id"].as_str().unwrap();
    assert_eq!(docs.calls(), ["alias"]);
    let shown = env.json(&dir, &["show", id]);
    assert_eq!(shown["task"]["plan"], "stub:canonical");
    assert_eq!(shown["plan_path"], "stub:canonical");
    assert_eq!(shown["step_found"], true);
    docs.disconnect();
    let listed = env.json(&dir, &["list"]);
    assert_eq!(listed["tasks"][0]["id"], id);
}

#[test]
fn external_doc_fetch_failure_leaves_no_record() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "");
    std::fs::write(&docs.script, "printf 'provider unavailable' >&2; exit 7\n").unwrap();
    let out = env
        .cmd(&dir)
        .args(["add", "External", "--spec", "stub:alias"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "doc_not_found");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("provider unavailable"),
        "{error}"
    );
    assert!(
        env.json(&dir, &["list"])["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
