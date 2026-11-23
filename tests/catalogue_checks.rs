//! `tasks check` findings drawn from the catalogue: the project's stage, one stage per
//! Git repository, and words two entries claim (docs/configuration.md).
mod common;
use common::{TestEnv, check_findings, git};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn config(dir: &Path) -> std::path::PathBuf {
    dir.join("tasks/.config.toml")
}

/// Replaces the `stage = ...` line init wrote (or drops it with an empty `line`).
fn set_stage(dir: &Path, line: &str) {
    let text = fs::read_to_string(config(dir)).unwrap();
    let kept: Vec<_> = text.lines().filter(|l| !l.starts_with("stage =")).collect();
    fs::write(config(dir), format!("{line}\n{}\n", kept.join("\n"))).unwrap();
}

fn append(dir: &Path, text: &str) {
    let mut config_text = fs::read_to_string(config(dir)).unwrap();
    config_text.push_str(text);
    fs::write(config(dir), config_text).unwrap();
}

fn kinds(findings: &Value, side: &str) -> Vec<String> {
    findings[side]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|f| f["kind"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The project's own `word_collision` warnings (a fresh fixture also warns `color_missing`).
fn collisions(env: &TestEnv, dir: &Path) -> Vec<String> {
    let (code, findings) = check_findings(env, dir);
    assert_eq!(code, 0, "a collision is a warning: {findings}");
    findings["warnings"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|f| f["kind"] == "word_collision")
                .map(|f| f["detail"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn detail(findings: &Value, side: &str, kind: &str) -> String {
    findings[side]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} in {findings}"))["detail"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn an_active_project_must_declare_a_valid_stage() {
    let mut env = TestEnv::new();
    let dir = env.init("st");
    assert_eq!(check_findings(&env, &dir).0, 0, "init writes a stage");

    set_stage(&dir, "");
    let (code, findings) = check_findings(&env, &dir);
    assert_eq!(code, 1, "{findings}");
    assert_eq!(kinds(&findings, "errors"), ["stage_missing"]);
    assert!(detail(&findings, "errors", "stage_missing").contains("stage = \"experimental\""));

    for value in ["\"preview\"", "4"] {
        set_stage(&dir, &format!("stage = {value}"));
        let (code, findings) = check_findings(&env, &dir);
        assert_eq!(code, 1, "{findings}");
        assert_eq!(kinds(&findings, "errors"), ["stage_invalid"], "{value}");
    }

    // A retired project needs no stage, though a declared one must still be valid.
    set_stage(&dir, "");
    append(
        &dir,
        "\n[retired]\ndate = \"2026-11-01\"\nreason = \"ended\"\n",
    );
    assert_eq!(check_findings(&env, &dir).0, 0);
    set_stage(&dir, "stage = \"preview\"");
    assert_eq!(
        kinds(&check_findings(&env, &dir).1, "errors"),
        ["stage_invalid"]
    );
}

#[test]
fn a_malformed_retired_table_fails_check() {
    let mut env = TestEnv::new();
    let dir = env.init("rt");
    append(&dir, "\n[retired]\ndate = \"2026-02-30\"\nreason = \"r\"\n");
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("[retired] date must be YYYY-MM-DD")
    );
}

#[test]
fn projects_sharing_a_git_repository_declare_one_stage() {
    let env = TestEnv::new();
    let temp = tempfile::tempdir().unwrap();
    let shared = temp.path().canonicalize().unwrap().join("shared");
    fs::create_dir_all(shared.join("one")).unwrap();
    fs::create_dir_all(shared.join("two")).unwrap();
    git(&shared, &["init", "-q", "-b", "main"]);
    let apart = temp.path().canonicalize().unwrap().join("apart");
    fs::create_dir_all(&apart).unwrap();
    git(&apart, &["init", "-q", "-b", "main"]);
    let one = shared.join("one");
    let two = shared.join("two");
    for (dir, prefix) in [(&one, "one"), (&two, "two"), (&apart, "apart")] {
        env.json(dir, &["init", "--prefix", prefix]);
    }
    assert_eq!(check_findings(&env, &one).0, 0, "one stage, no conflict");

    set_stage(&two, "stage = \"supported\"");
    set_stage(&apart, "stage = \"pre-release\"");
    for (dir, other) in [(&one, "two"), (&two, "one")] {
        let (code, findings) = check_findings(&env, dir);
        assert_eq!(code, 1, "{findings}");
        assert_eq!(kinds(&findings, "errors"), ["stage_conflict"]);
        let text = detail(&findings, "errors", "stage_conflict");
        assert!(
            text.contains(other)
                && text.contains("experimental")
                && text.contains("supported")
                && text.contains(shared.join(".git").to_str().unwrap()),
            "{text}"
        );
        assert!(!text.contains("apart"), "{text}");
    }
    assert_eq!(check_findings(&env, &apart).0, 0, "its own repository");

    // A retired project no longer sets the repository's policy.
    append(
        &two,
        "\n[retired]\ndate = \"2026-11-01\"\nreason = \"ended\"\n",
    );
    assert_eq!(check_findings(&env, &one).0, 0);
    assert_eq!(check_findings(&env, &two).0, 0);
}

#[test]
fn a_word_two_entries_claim_warns_both_and_a_successor_inherits_its_predecessors() {
    let mut env = TestEnv::new();
    let aa = env.init("aa");
    let bb = env.init("bb");
    let cc = env.init("cc");
    assert!(collisions(&env, &aa).is_empty());

    // Case-insensitive: bb's shorthand claims aa's prefix.
    append(&bb, "shorthand = [\"AA\"]\n");
    for dir in [&aa, &bb] {
        let found = collisions(&env, dir);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("aa (prefix)") && found[0].contains("bb (shorthand)"),
            "{found:?}"
        );
    }
    assert!(collisions(&env, &cc).is_empty());

    // A retired project's words forward to its successor: no collision with it.
    let text = fs::read_to_string(config(&bb)).unwrap();
    fs::write(config(&bb), text.replace("shorthand = [\"AA\"]\n", "")).unwrap();
    append(&cc, "shorthand = [\"shared\"]\n");
    append(&aa, "shorthand = [\"shared\"]\n");
    append(
        &cc,
        "\n[retired]\ndate = \"2026-11-01\"\nsuccessor = \"aa\"\nreason = \"merged\"\n",
    );
    assert!(collisions(&env, &aa).is_empty());
    assert!(collisions(&env, &cc).is_empty());
    // Any third claimant still collides.
    append(&bb, "shorthand = [\"shared\"]\n");
    assert_eq!(collisions(&env, &aa).len(), 1);
}
