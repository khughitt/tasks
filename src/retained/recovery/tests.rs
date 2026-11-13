use super::*;
use serde_json::json;
fn declaration() -> serde_json::Value {
    json!({"version":1,"source_cutover":"a".repeat(40),"destination_import":"b".repeat(40),"source_ref":"refs/heads/main","tasks":["sci-123456"],"refs":[{"original_ref":"refs/heads/main","endpoint":"a".repeat(40)}],"manifest_bindings":[]})
}
#[test]
fn migration_declaration_shape() {
    let good: RecoveryDeclaration = serde_json::from_value(declaration()).unwrap();
    validate_shape("sci", Kind::Detached, &good).unwrap();
    for kind in [Kind::InTree, Kind::Linked] {
        assert!(validate_shape("sci", kind, &good).is_err());
    }
    for field in ["tasks", "refs"] {
        let mut d = declaration();
        let a = d[field].as_array_mut().unwrap();
        a.push(a[0].clone());
        let d = serde_json::from_value(d).unwrap();
        assert!(validate_shape("sci", Kind::Detached, &d).is_err());
    }
    for (field, value) in [
        ("version", json!(2)),
        ("tasks", json!(["other-123456"])),
        ("source_cutover", json!("abc")),
        ("source_ref", json!("main")),
        (
            "manifest_bindings",
            json!([{"task":"sci-123456","ordinal":0,"attachment":"manifest.json"}]),
        ),
    ] {
        let mut d = declaration();
        d[field] = value;
        let d = serde_json::from_value(d).unwrap();
        assert!(
            validate_shape("sci", Kind::Detached, &d).is_err(),
            "{field}"
        );
    }
}
#[test]
fn migration_declaration_strict_decoder() {
    for field in ["version", "tasks", "manifest_bindings"] {
        let mut d = declaration();
        d.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<RecoveryDeclaration>(d).is_err());
    }
    let mut d = declaration();
    d["unexpected"] = json!(true);
    assert!(serde_json::from_value::<RecoveryDeclaration>(d).is_err());
}

use crate::retained::read::tests::{fixture, git, isolated};
fn paired_fixture() -> (tempfile::TempDir, tempfile::TempDir, Project, Basis, Basis) {
    let (source, s) = fixture();
    let task = "---\nid: sci-123456\ntitle: Recovery\nstatus: idea\npriority: 2\ncreated: 2026-11-10T00:00:00Z\nupdated: 2026-11-10T00:00:00Z\ndepends: []\ntags: []\n---\n\n## Notes\n\n- 2026-11-10T00:00:00Z (fixture): source history\n";
    std::fs::write(source.path().join("tasks/sci-123456.md"), task).unwrap();
    git(source.path(), &["add", "."]);
    git(source.path(), &["commit", "-qm", "source task"]);
    let cutover = git(source.path(), &["rev-parse", "HEAD"]);
    let (destination, _) = fixture();
    let seed = git(destination.path(), &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(destination.path().join("sci/tasks")).unwrap();
    std::fs::copy(
        source.path().join("tasks/.config.toml"),
        destination.path().join("sci/tasks/.config.toml"),
    )
    .unwrap();
    std::fs::write(destination.path().join("sci/tasks/sci-123456.md"), task).unwrap();
    git(destination.path(), &["add", "."]);
    git(destination.path(), &["commit", "-qm", "export"]);
    let export = git(destination.path(), &["rev-parse", "HEAD"]);
    let tree = git(destination.path(), &["rev-parse", "HEAD^{tree}"]);
    let message = format!(
        "import\n\nRecords-Migrate-Prefix: sci\nRecords-Migrate-Source: {cutover}\nRecords-Migrate-Paths: tasks docs/specs docs/plans docs/notes docs/superpowers docs/designs docs/reports\nRecords-Migrate-Citations: 2"
    );
    let import = git(
        destination.path(),
        &[
            "commit-tree",
            &tree,
            "-p",
            &seed,
            "-p",
            &export,
            "-m",
            &message,
        ],
    );
    git(destination.path(), &["reset", "--hard", &import]);
    let mut d = declaration();
    d["source_cutover"] = json!(cutover);
    d["destination_import"] = json!(import);
    d["refs"] = json!([{"original_ref":"refs/heads/disposable","endpoint":cutover}]);
    let declaration: RecoveryDeclaration = serde_json::from_value(d).unwrap();
    let config = format!(
        "prefix = 'sci'\n[checkout]\nkind='detached'\npath={}\n{}",
        serde_json::to_string(s.root.to_str().unwrap()).unwrap(),
        toml::to_string(&std::collections::BTreeMap::from([(
            "retention_recovery",
            declaration
        )]))
        .unwrap()
    );
    let records = destination.path().join("sci");
    std::fs::write(records.join("tasks/.config.toml"), config).unwrap();
    git(destination.path(), &["add", "."]);
    git(destination.path(), &["commit", "-qm", "pair and declare"]);
    let p = read::start(&Project::open(&records).unwrap()).unwrap();
    let original = source_basis("sci", "sci-123456", &cutover, "refs/heads/disposable");
    let current = current_basis(
        &p,
        &TaskId::parse("sci-123456").unwrap(),
        git(destination.path(), &["rev-parse", "HEAD"]),
        "refs/heads/main".into(),
    )
    .unwrap();
    (source, destination, p, original, current)
}
#[test]
fn migration_context_selection() {
    if isolated("retained::recovery::tests::migration_context_selection") {
        return;
    }
    let (source, destination, p, old, current) = paired_fixture();
    let records = p.records.clone();
    let id = TaskId::parse("sci-123456").unwrap();
    let selected = select(&p, &id, &old).unwrap().unwrap();
    assert_eq!(selected.epoch, Epoch::Source);
    assert_eq!(selected.context.root(), source.path());
    assert_eq!(selected.publication_tip, old.commit);
    let selected = select(&p, &id, &current).unwrap().unwrap();
    assert_eq!(selected.epoch, Epoch::Destination);
    assert_eq!(selected.context.root(), destination.path());
    assert_eq!(p.records, records);
}
#[test]
fn migration_mixed_epoch_refuses() {
    if isolated("retained::recovery::tests::migration_mixed_epoch_refuses") {
        return;
    }
    let (_source, _destination, p, old, current) = paired_fixture();
    let trace = std::env::var_os("GIT_TRACE2_EVENT").unwrap();
    let before = std::fs::read(&trace).unwrap();
    assert!(same_epoch(&p, &TaskId::parse("sci-123456").unwrap(), &old, &current).is_err());
    assert_eq!(
        std::fs::read(&trace).unwrap(),
        before,
        "mixed epoch comparison invoked Git"
    );
}
#[test]
fn migration_wrong_repository() {
    if isolated("retained::recovery::tests::migration_wrong_repository") {
        return;
    }
    let (_source, _destination, mut p, old, _current) = paired_fixture();
    let (wrong, _) = fixture();
    p.checkout = wrong.path().into();
    assert!(select(&p, &TaskId::parse("sci-123456").unwrap(), &old).is_err());
}
