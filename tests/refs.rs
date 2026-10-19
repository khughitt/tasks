mod common;
use common::{TestEnv, check_findings, findings_of, id_of, write_doc};

#[test]
fn check_reports_reference_audit_findings_on_open_tasks_only() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/x.md", "x");
    let id = id_of(env.json(&dir, &["add", "A"]));
    // Hand-write the body and section: the CLI writers arrive in Task 5.
    let rel = format!("tasks/{id}.md");
    let text = env.read(&dir, &rel).replacen("\n---\n", "\n---\n\nSee [ref:gone] and [ref:Bad].\n\n## References\n\n- a (context): docs/x.md\n- b (input): docs/x.md\n\n", 1);
    std::fs::write(dir.join(&rel), text).unwrap();
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 1, "{v}");
    for kind in [
        "ref_duplicate",
        "ref_citation_dangling",
        "ref_citation_malformed",
    ] {
        assert_eq!(findings_of(&v, "errors", kind).len(), 1, "{kind}: {v}");
    }
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["done", &id, "x"]);
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 0, "{v}");
}
