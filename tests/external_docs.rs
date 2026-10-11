mod common;
use common::{TestEnv, docs::DocFixture};

#[test]
fn external_doc_unrelated_edit_does_not_fetch() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let calls = docs.calls();
    docs.disconnect();
    env.json(&dir, &["edit", &id, "--title", "After", "-p", "1"]);
    env.json(
        &dir,
        &[
            "edit",
            &id,
            "--spec",
            "stub:spec",
            "--plan",
            "stub:canonical",
            "--step",
            "Task 1: one",
        ],
    );
    assert_eq!(docs.calls(), calls);
    let rows = env.json(&dir, &["list"]);
    assert_eq!(rows["tasks"][0]["title"], "After");
    assert_eq!(rows["tasks"][0]["priority"], 1);
    env.json(&dir, &["edit", &id, "--no-spec"]);
    env.json(&dir, &["edit", &id, "--no-step", "--no-plan"]);
    assert_eq!(docs.calls(), calls);
}

#[test]
fn external_doc_changed_values_fetch_only_needed_documents() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n### Task 2: two\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.json(&dir, &["edit", &id, "--spec", "stub:new-spec"]);
    assert_eq!(docs.calls(), ["spec", "alias", "new-spec"]);
    env.json(&dir, &["edit", &id, "--step", "Task 2: two"]);
    assert_eq!(docs.calls(), ["spec", "alias", "new-spec", "canonical"]);
    env.json(&dir, &["edit", &id, "--plan", "stub:new-plan"]);
    assert_eq!(
        docs.calls(),
        ["spec", "alias", "new-spec", "canonical", "new-plan"]
    );
    let record = dir.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&record).unwrap();
    docs.set_body("# no steps\n");
    assert_eq!(
        env.fail(&dir, &["edit", &id, "--step", "Task 3: absent"]),
        "validation"
    );
    assert_eq!(std::fs::read(record).unwrap(), before);
}

#[test]
fn external_doc_editor_skips_unchanged_links_and_canonicalizes_changes() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let id = env.json(
        &dir,
        &[
            "add",
            "Before",
            "--spec",
            "stub:spec",
            "--plan",
            "stub:alias",
            "--step",
            "Task 1: one",
        ],
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let editor = env.home.path().join("edit.sh");
    std::fs::write(
        &editor,
        "sed -i 's/^spec:.*/spec: \"stub:alias\"/' \"$1\"\n",
    )
    .unwrap();
    let command = format!("sh '{}'", editor.display());
    let out = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(docs.calls(), ["spec", "alias", "alias"]);
    assert_eq!(
        env.json(&dir, &["show", &id])["task"]["spec"],
        "stub:canonical"
    );
    let calls = docs.calls();
    docs.disconnect();
    std::fs::write(&editor, "printf '\\nBody edit\\n' >> \"$1\"\n").unwrap();
    let out = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(docs.calls(), calls);
    let record = dir.join(format!("tasks/{id}.md"));
    assert!(
        std::fs::read_to_string(&record)
            .unwrap()
            .contains("\nBody edit\n")
    );
    let before = std::fs::read(&record).unwrap();
    std::fs::write(
        &editor,
        "sed -i 's/^step:.*/step: \"Task 2: absent\"/' \"$1\"\n",
    )
    .unwrap();
    let refused = env
        .cmd(&dir)
        .env("EDITOR", &command)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("edit kept at"));
    assert_eq!(std::fs::read(&record).unwrap(), before);
    assert!(std::fs::read_dir(dir.join("tasks")).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".edit.md")
    }));
}

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
