mod common;
use common::{TestEnv, id_of, write_doc};
use serde_json::{Value, json};
use std::path::Path;

/// One project holding a document of every kind, plus files the listing must skip, and
/// three tasks linking them through `spec`, `plan` and a `--ref` entry.
fn fixture(env: &mut TestEnv) -> (std::path::PathBuf, String, String, String) {
    let dir = env.init("pa");
    write_doc(
        &dir,
        "docs/specs/2026-01-01-alpha-design.md",
        "# Alpha design\n\nThe Widget Contract lives here.\n",
    );
    write_doc(
        &dir,
        "docs/plans/2026-01-02-alpha.md",
        "# Alpha plan\n\n### Task 1: Build it\n\nwidget contract steps\n",
    );
    write_doc(
        &dir,
        "docs/notes/2026-01-03-alpha-brief.md",
        "# Alpha brief\n",
    );
    write_doc(
        &dir,
        "docs/notes/2026-01-04-alpha-handoff.md",
        "# Alpha handoff\n",
    );
    write_doc(&dir, "docs/notes/scratch.md", "no heading here\n");
    write_doc(&dir, "docs/notes/data.txt", "# not markdown\n");
    write_doc(&dir, "docs/reference.md", "# A reference document\n");
    let spec = id_of(env.json(
        &dir,
        &[
            "add",
            "specced",
            "--spec",
            "docs/specs/2026-01-01-alpha-design.md",
        ],
    ));
    let plan = id_of(env.json(
        &dir,
        &[
            "add",
            "planned",
            "--plan",
            "docs/plans/2026-01-02-alpha.md",
            "--step",
            "Task 1: Build it",
        ],
    ));
    let referenced = id_of(env.json(
        &dir,
        &[
            "add",
            "referenced",
            "--spec",
            "docs/specs/2026-01-01-alpha-design.md",
            "--ref",
            "brief (input): docs/notes/2026-01-03-alpha-brief.md",
        ],
    ));
    (dir, spec, plan, referenced)
}

fn paths(out: &Value) -> Vec<String> {
    out["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn docs_lists_each_document_with_its_kind_title_and_linking_tasks() {
    let mut env = TestEnv::new();
    let (dir, spec, plan, referenced) = fixture(&mut env);
    let out = env.json(&dir, &["docs"]);
    assert_eq!(out["warnings"], json!([]));
    let docs = out["docs"].as_array().unwrap();
    // Spec, plan and notes directories only, Markdown only, in path order.
    assert_eq!(
        paths(&out),
        [
            "docs/notes/2026-01-03-alpha-brief.md",
            "docs/notes/2026-01-04-alpha-handoff.md",
            "docs/notes/scratch.md",
            "docs/plans/2026-01-02-alpha.md",
            "docs/specs/2026-01-01-alpha-design.md",
        ]
    );
    let kinds: Vec<&str> = docs
        .iter()
        .map(|row| row["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["brief", "handoff", "note", "plan", "spec"]);
    for row in docs {
        assert_eq!(row["project"], "pa");
        let file = Path::new(row["file"].as_str().unwrap());
        assert!(file.is_absolute() && file.is_file(), "{row}");
        assert_eq!(file, dir.join(row["path"].as_str().unwrap()));
    }
    assert_eq!(docs[0]["title"], "Alpha brief");
    // No level-one heading: the file's stem stands in.
    assert_eq!(docs[2]["title"], "scratch");
    assert_eq!(
        docs[0]["tasks"],
        json!([{"id": referenced, "status": "todo", "via": "ref", "handle": "brief"}])
    );
    assert_eq!(docs[1]["tasks"], json!([]));
    assert_eq!(
        docs[3]["tasks"],
        json!([{"id": plan, "status": "todo", "via": "plan"}])
    );
    let mut specced = [spec.clone(), referenced.clone()];
    specced.sort();
    assert_eq!(
        docs[4]["tasks"],
        json!([
            {"id": specced[0], "status": "todo", "via": "spec"},
            {"id": specced[1], "status": "todo", "via": "spec"},
        ])
    );
}

#[test]
fn docs_counts_links_from_closed_tasks_too() {
    let mut env = TestEnv::new();
    let (dir, _, plan, _) = fixture(&mut env);
    env.json(&dir, &["start", &plan]);
    env.json(&dir, &["done", &plan, "landed"]);
    let out = env.json(&dir, &["docs", "--kind", "plan"]);
    assert_eq!(
        out["docs"][0]["tasks"],
        json!([{"id": plan, "status": "done", "via": "plan"}])
    );
}

#[test]
fn docs_kind_filters_and_repeats_widen() {
    let mut env = TestEnv::new();
    let (dir, ..) = fixture(&mut env);
    let out = env.json(&dir, &["docs", "--kind", "plan"]);
    assert_eq!(paths(&out), ["docs/plans/2026-01-02-alpha.md"]);
    let out = env.json(&dir, &["docs", "--kind", "brief", "--kind", "spec"]);
    assert_eq!(
        paths(&out),
        [
            "docs/notes/2026-01-03-alpha-brief.md",
            "docs/specs/2026-01-01-alpha-design.md",
        ]
    );
    let err = env.usage(&dir, &["docs", "--kind", "design"]);
    assert!(err.contains("--kind") && err.contains("design"), "{err}");
}

#[test]
fn docs_search_matches_title_or_body_case_insensitively() {
    let mut env = TestEnv::new();
    let (dir, ..) = fixture(&mut env);
    let out = env.json(&dir, &["docs", "--search", "WIDGET contract"]);
    assert_eq!(
        paths(&out),
        [
            "docs/plans/2026-01-02-alpha.md",
            "docs/specs/2026-01-01-alpha-design.md",
        ]
    );
    let out = env.json(&dir, &["docs", "--search", "handoff"]);
    assert_eq!(paths(&out), ["docs/notes/2026-01-04-alpha-handoff.md"]);
    let out = env.json(&dir, &["docs", "--kind", "plan", "--search", "widget"]);
    assert_eq!(paths(&out), ["docs/plans/2026-01-02-alpha.md"]);
    let out = env.json(&dir, &["docs", "--search", "no such phrase"]);
    assert_eq!(out["docs"], json!([]));
    assert_eq!(env.fail(&dir, &["docs", "--search", "  "]), "validation");
}

#[test]
fn docs_follows_configured_spec_and_plan_dirs() {
    let mut env = TestEnv::new();
    let dir = env.init("pc");
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace(
            "prefix = \"pc\"\n",
            "prefix = \"pc\"\nspec_dirs = [\"design\"]\nplan_dirs = [\"design/plans\"]\n",
        ),
    )
    .unwrap();
    write_doc(&dir, "design/x.md", "# X\n");
    write_doc(&dir, "design/plans/y.md", "# Y\n");
    write_doc(&dir, "docs/specs/ignored.md", "# Not a spec dir here\n");
    let out = env.json(&dir, &["docs"]);
    let rows: Vec<(String, String)> = out["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["path"].as_str().unwrap().into(),
                row["kind"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("design/plans/y.md".to_string(), "plan".to_string()),
            ("design/x.md".to_string(), "spec".to_string()),
        ]
    );
}

#[test]
fn docs_reads_another_project_or_every_project() {
    let mut env = TestEnv::new();
    let (dir, ..) = fixture(&mut env);
    let other = env.init("pb");
    write_doc(&other, "docs/plans/2026-02-01-beta.md", "# Beta plan\n");
    let out = env.json(&dir, &["docs", "--project", "pb"]);
    assert_eq!(paths(&out), ["docs/plans/2026-02-01-beta.md"]);
    assert_eq!(out["docs"][0]["project"], "pb");
    let out = env.json(&other, &["docs", "--all-projects", "--kind", "plan"]);
    let rows: Vec<(&str, &str)> = out["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["project"].as_str().unwrap(),
                row["path"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("pa", "docs/plans/2026-01-02-alpha.md"),
            ("pb", "docs/plans/2026-02-01-beta.md"),
        ]
    );
}

#[test]
fn docs_warns_about_a_document_that_is_not_utf8_and_lists_it() {
    let mut env = TestEnv::new();
    let dir = env.init("pd");
    let path = dir.join("docs/notes/binary.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, [0xff, 0xfe, b'#']).unwrap();
    let out = env.json(&dir, &["docs"]);
    assert_eq!(paths(&out), ["docs/notes/binary.md"]);
    assert_eq!(out["docs"][0]["title"], "binary");
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0]
            .as_str()
            .unwrap()
            .contains("docs/notes/binary.md"),
        "{warnings:?}"
    );
}

#[test]
fn docs_pretty_shows_kind_path_title_and_the_linking_tasks() {
    let mut env = TestEnv::new();
    let (dir, _, plan, _) = fixture(&mut env);
    let text = env.pretty(&dir, &["docs", "--kind", "plan"]);
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(
        first.starts_with("plan")
            && first.contains("docs/plans/2026-01-02-alpha.md")
            && first.contains("Alpha plan"),
        "{text}"
    );
    let detail = lines.next().unwrap();
    assert!(
        detail.contains("tasks:") && detail.contains(&format!("{plan} plan")),
        "{text}"
    );
    // Under a wide scope each path names its project.
    let text = env.pretty(&dir, &["docs", "--all-projects", "--kind", "plan"]);
    assert!(text.contains("pa:docs/plans/2026-01-02-alpha.md"), "{text}");
}

#[test]
fn docs_lists_nothing_for_a_project_without_docs_directories() {
    let mut env = TestEnv::new();
    let dir = env.init("pe");
    let out = env.json(&dir, &["docs"]);
    assert_eq!(out, json!({"docs": [], "warnings": []}));
}

#[test]
fn docs_names_the_unreadable_file_in_its_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let dir = env.init("pf");
    write_doc(&dir, "docs/notes/locked.md", "# Locked\n");
    let path = dir.join("docs/notes/locked.md");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&path).is_ok() {
        // Running as root: permissions do not bind, so there is nothing to observe.
        return;
    }
    let out = env.cmd(&dir).args(["docs"]).output().unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "io", "{err}");
    assert!(
        err["error"]["detail"]
            .as_str()
            .unwrap()
            .contains(path.to_str().unwrap()),
        "{err}"
    );
}

#[test]
fn docs_search_says_a_non_utf8_document_was_left_out() {
    let mut env = TestEnv::new();
    let dir = env.init("pg");
    let path = dir.join("docs/notes/binary.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, [0xff, 0xfe, b'#']).unwrap();
    let out = env.json(&dir, &["docs", "--search", "binary"]);
    assert_eq!(out["docs"], json!([]));
    let warning = out["warnings"][0].as_str().unwrap();
    assert!(
        warning.contains("docs/notes/binary.md") && warning.contains("not listed"),
        "{out}"
    );
}

#[test]
fn docs_warns_about_a_dangling_markdown_symlink() {
    let mut env = TestEnv::new();
    let dir = env.init("ph");
    std::fs::create_dir_all(dir.join("docs/plans")).unwrap();
    std::os::unix::fs::symlink("gone.md", dir.join("docs/plans/broken.md")).unwrap();
    let out = env.json(&dir, &["docs"]);
    assert_eq!(out["docs"], json!([]));
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{out}");
    assert!(
        warnings[0]
            .as_str()
            .unwrap()
            .contains("docs/plans/broken.md"),
        "{out}"
    );
}
