mod common;

use common::{TestEnv, as_agent, err_detail, err_kind, git, id_of};

#[test]
fn equal_stamp_git_resolution_requires_identical_siblings_before_retry() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T"]));
    env.json(&main, &["note", &id, "common history"]);
    git(&main, &["add", "tasks"]);
    git(&main, &["commit", "-qm", "common record"]);
    git(&main, &["switch", "-qc", "local"]);
    env.json(&main, &["note", &id, "local history"]);
    let record = format!("tasks/{id}.md");
    let local = env.read(&main, &record);
    git(&main, &["commit", "-qam", "local history"]);
    git(&main, &["switch", "main"]);
    env.json(&main, &["note", &id, "source history"]);
    env.json(&main, &["note", &id, "source second note"]);
    git(&main, &["commit", "-qam", "source history"]);
    let source = env.read(&main, &record);
    let source_task = env.json(&main, &["show", &id])["task"].clone();

    let side = main.join("wt");
    git(
        &main,
        &["worktree", "add", "-q", side.to_str().unwrap(), "local"],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let merge = common::git_fixture::command(env.home.path(), &side)
        .args(["merge", "--no-commit", "--no-ff", "main"])
        .output()
        .unwrap();
    assert_eq!(merge.status.code(), Some(1), "{merge:?}");
    assert!(env.read(&side, &record).contains("<<<<<<<"));

    // Resolve the real Git conflict with the source header and both CLI-authored histories.
    let local_note = local.rsplit_once("\n- ").unwrap().1;
    let resolved = format!("{source}\n- {local_note}");
    std::fs::write(side.join(&record), &resolved).unwrap();
    git(&side, &["add", &record]);
    git(&side, &["commit", "-qm", "resolve both histories"]);
    let resolved_task = env.json(&side, &["show", &id])["task"].clone();
    assert_eq!(resolved_task["updated"], source_task["updated"]);
    let notes = resolved_task["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 5);
    assert_eq!(&notes[..4], source_task["notes"].as_array().unwrap());
    assert_eq!(notes[4]["text"], "local history");

    let retry = ["note", id.as_str(), "retry"];
    let out = env.cmd(&side).args(retry).output().unwrap();
    assert_eq!(err_kind(&out), "stale_copy");
    let detail = err_detail(&out);
    assert!(detail.contains("Git conflict resolution"), "{detail}");
    assert!(detail.contains("every equal-stamp sibling"), "{detail}");
    assert_eq!(env.read(&side, &record), resolved);
    assert_eq!(env.read(&main, &record), source);

    let other = id_of(env.json(&main, &["add", "Other work"]));
    as_agent(&env, &main, "other-owner")
        .args(["start", &other])
        .assert()
        .success();
    let claims = std::fs::read(env.claim_store("sci")).unwrap();
    let out = as_agent(&env, &side, "recovering-owner")
        .args(retry)
        .output()
        .unwrap();
    assert_eq!(err_kind(&out), "stale_copy");
    let detail = err_detail(&out);
    assert!(detail.contains("other-owner"), "{detail}");
    assert!(
        detail.contains("Coordinate with the named session owners"),
        "{detail}"
    );
    assert!(detail.contains("every equal-stamp sibling"), "{detail}");
    assert!(!detail.contains("tasks -C"), "{detail}");
    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), claims);
    assert_eq!(env.read(&side, &record), resolved);
    assert_eq!(env.read(&main, &record), source);

    // The source owner integrates the committed resolution before the CLI retries.
    git(&main, &["merge", "-q", "--ff-only", "local"]);
    assert_eq!(env.read(&main, &record), resolved);
    env.json(&side, &retry);
    let recovered = env.json(&side, &["show", &id])["task"].clone();
    assert!(recovered["updated"].as_str() > resolved_task["updated"].as_str());
    let recovered_notes = recovered["notes"].as_array().unwrap();
    assert_eq!(&recovered_notes[..5], notes);
    assert_eq!(recovered_notes[5]["text"], "retry");
    assert_eq!(env.read(&main, &record), resolved);
    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), claims);
}
