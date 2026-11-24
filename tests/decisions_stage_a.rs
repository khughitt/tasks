//! Stage A pending decisions: atomic drop/suggestion lifecycle, scope fingerprint v1,
//! actor/source/historical imports, per-kind holds, and list/search --pending (Tasks 3–4).

mod common;

use common::{TestEnv, id_of};
use serde_json::{Value, json};
use std::path::Path;

const D1: &str = "6f76bdab-e417-4ae7-81ce-09fc70d6487a";
const D2: &str = "1b4e28ba-2fa1-4d2f-9c83-2d3f9c0a1b2c";
const D3: &str = "3c5e7a90-1234-4abc-8def-0123456789ab";

fn write_f1(dir: &Path, id: &str, status: &str, extra_fm: &str, body: &str) {
    let depends = if extra_fm.contains("depends:") {
        ""
    } else {
        "depends: []\n"
    };
    let text = format!(
        "---\nid: {id}\ntitle: Task {id}\nstatus: {status}\npriority: 2\n\
         created: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\n\
         {extra_fm}{depends}tags: []\n---\n{body}"
    );
    std::fs::write(dir.join(format!("tasks/{id}.md")), text).unwrap();
}

fn err_json(env: &TestEnv, dir: &Path, args: &[&str]) -> Value {
    let out = env.cmd(dir).args(args).output().unwrap();
    assert!(
        !out.status.success(),
        "expected failure for {args:?}, got stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    serde_json::from_slice(&out.stderr).unwrap_or_else(|e| {
        panic!(
            "stderr not json ({e}): {}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

#[test]
fn stage_a_cli_rejects_stage_b_syntax_and_pending_conflicts() {
    let mut env = TestEnv::new();
    let dir = env.init("sa");
    let id = id_of(env.json(&dir, &["add", "Sample task"]));

    // Stage A must reject Stage B kinds, --reply, artifact pins, and park --decision (usage exit 2).
    for args in [
        vec![
            "decision", "open", &id, "--kind", "question", "--origin", "scope", "--text", "Q?",
        ],
        vec![
            "decision", "open", &id, "--kind", "approval", "--origin", "workflow", "--text", "A?",
        ],
        vec![
            "decision",
            "open",
            &id,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "D",
            "--artifact-path",
            "docs/p.md",
        ],
        vec!["decision", "answer", &id, D1, "--reply", "Answer"],
        vec![
            "park",
            &id,
            "next",
            "--waiting-on",
            "user",
            "--reason",
            "decision",
            "--decision",
            D1,
        ],
    ] {
        let out = env.cmd(&dir).args(&args).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "Stage A must reject {args:?} as usage error (exit 2), got {:?} stderr={}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // list --pending conflicts with explicit sort, reverse, parked, periodic, deferred, actions.
    for conflict in [
        vec!["list", "--pending", "--sort", "updated"],
        vec!["list", "--pending", "--reverse"],
        vec!["list", "--pending", "--parked"],
        vec!["list", "--pending", "--periodic"],
        vec!["list", "--pending", "--deferred"],
        vec!["list", "--pending", "--actions"],
        vec!["list", "--pending", "--waiting-on", "user"],
    ] {
        let out = env.cmd(&dir).args(&conflict).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "expected usage conflict for {conflict:?}"
        );
    }

    // Bare list --pending succeeds (clap default sort does not conflict).
    let pending = env.json(&dir, &["list", "--pending"]);
    assert_eq!(pending["items"], json!([]));
    assert_eq!(pending["coverage"]["complete"], true);
    assert_eq!(pending["coverage"]["unavailable_projects"], json!([]));
    assert!(pending["observed_at"].as_str().is_some());
    assert!(pending["warnings"].is_array());
}

#[test]
fn decision_open_replay_actor_source_and_historical_import() {
    let mut env = TestEnv::new();
    let dir = env.init("im");
    write_f1(
        &dir,
        "im-000001",
        "todo",
        "process: direct\n",
        "\nBody text.\n",
    );

    // 1. Direct caller omitting --by resolves owner and records source = "direct"; auto UUIDv4.
    let opened = env.json(
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Close as delivered",
        ],
    );
    assert_eq!(opened["action"], "opened");
    assert_eq!(opened["replayed"], false);
    let auto_id = opened["decision"]["id"].as_str().unwrap().to_string();
    assert_eq!(auto_id.len(), 36);
    assert_eq!(opened["decision"]["data"]["source"], "direct");
    assert!(
        opened["decision"]["by"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    );
    let auto_at = opened["decision"]["at"].as_str().unwrap().to_string();
    let auto_by = opened["decision"]["by"].as_str().unwrap().to_string();

    // Unrelated note does not break replay idempotency.
    env.json(&dir, &["note", "im-000001", "unrelated progress note"]);
    let replayed = env.json(
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--id",
            &auto_id,
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Close as delivered",
            "--by",
            &auto_by,
        ],
    );
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["decision"]["at"], auto_at);
    assert_eq!(replayed["decision"]["id"], auto_id);

    // Mismatched text on same UUID conflicts.
    let conflict = err_json(
        &env,
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--id",
            &auto_id,
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Different text",
            "--by",
            &auto_by,
        ],
    );
    assert_eq!(conflict["error"]["kind"], "decision_conflict", "{conflict}");

    // 2. --source requires explicit --by; bare --opening-scope without --opened-at refused;
    //    --opened-at without both --by and --source refused.
    for bad in [
        vec![
            "decision",
            "open",
            "im-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "drop",
            "--source",
            "note:1",
        ],
        vec![
            "decision",
            "open",
            "im-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "drop",
            "--opening-scope",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ],
        vec![
            "decision",
            "open",
            "im-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "drop",
            "--opened-at",
            "2026-11-10T08:00:00Z",
            "--by",
            "scope",
        ],
    ] {
        let e = err_json(&env, &dir, &bad);
        assert_eq!(e["error"]["kind"], "validation", "{bad:?}: {e}");
    }

    // 3. Historical import with --opened-at + --by + --source (no --opening-scope) stores {"scope": null}
    //    and reports scope_changed_since_open: null even when current scope is readable.
    let hist = env.json(
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Historical drop",
            "--by",
            "scope",
            "--source",
            "note:2026-11-10T08:00:00Z",
            "--opened-at",
            "2026-11-10T08:00:00Z",
        ],
    );
    assert_eq!(hist["decision"]["subject"], json!({"scope": null}));
    assert_eq!(hist["decision"]["scope_changed_since_open"], json!(null));
    assert!(hist["decision"]["seen_scope"].as_str().is_some());
    assert_eq!(hist["decision"]["at"], "2026-11-10T08:00:00Z");

    // Different historical time on replay conflicts.
    let hist_conflict = err_json(
        &env,
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Historical drop",
            "--by",
            "scope",
            "--source",
            "note:2026-11-10T08:00:00Z",
            "--opened-at",
            "2026-11-11T08:00:00Z",
        ],
    );
    assert_eq!(hist_conflict["error"]["kind"], "decision_conflict");

    // Historical import with explicit --opening-scope stores that digest and detects change when title differs.
    let old_digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let hist2 = env.json(
        &dir,
        &[
            "decision",
            "open",
            "im-000001",
            "--id",
            D2,
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Old suggestion",
            "--by",
            "curate",
            "--source",
            "note:2026-11-09T08:00:00Z",
            "--opened-at",
            "2026-11-09T08:00:00Z",
            "--opening-scope",
            old_digest,
        ],
    );
    assert_eq!(hist2["decision"]["subject"], json!({"scope": old_digest}));
    assert_eq!(hist2["decision"]["scope_changed_since_open"], true);

    // Format-1 awaiting_user: true refuses both drop and suggestion before saving anything.
    write_f1(
        &dir,
        "im-000002",
        "idea",
        "reviewed: true\nawaiting_user: true\nreviewed_at: 2026-11-20T00:00:00Z\n",
        "\nQuestion body.\n",
    );
    let raw_before = std::fs::read_to_string(dir.join("tasks/im-000002.md")).unwrap();
    for kind in ["drop", "suggestion"] {
        let e = err_json(
            &env,
            &dir,
            &[
                "decision",
                "open",
                "im-000002",
                "--kind",
                kind,
                "--origin",
                "scope",
                "--text",
                "Try open",
            ],
        );
        assert_eq!(e["error"]["kind"], "validation", "{kind}: {e}");
        assert!(
            e["error"]["detail"]
                .as_str()
                .unwrap()
                .contains("awaiting_user"),
            "{e}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("tasks/im-000002.md")).unwrap(),
        raw_before
    );
}

#[test]
fn atomic_drop_accept_scope_token_guards_replay_and_dependents() {
    let mut env = TestEnv::new();
    let dir = env.init("dr");
    let dep_proj = env.init("dp");
    let unreach = env.init("un");

    write_f1(
        &dir,
        "dr-000001",
        "todo",
        "process: direct\n",
        "\nOriginal body.\n",
    );
    write_f1(
        &dep_proj,
        "dp-000001",
        "todo",
        "process: direct\ndepends: [dr-000001]\n",
        "",
    );
    // Make `un` unreachable to test incomplete dependent coverage.
    std::fs::remove_file(unreach.join("tasks/.config.toml")).unwrap();

    // Open a drop decision.
    let opened = env.json(
        &dir,
        &[
            "decision",
            "open",
            "dr-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Source disappeared",
        ],
    );
    let token1 = opened["decision"]["seen_scope"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(opened["decision"]["scope_changed_since_open"], false);
    assert_eq!(opened["decision"]["open_dependents"]["complete"], false);
    assert_eq!(
        opened["decision"]["open_dependents"]["unavailable_projects"],
        json!(["un"])
    );
    assert_eq!(
        opened["decision"]["open_dependents"]["tasks"],
        json!([{"id": "dp-000001", "title": "Task dp-000001"}])
    );

    // Note and attachment do NOT stale seen_scope!
    env.json(&dir, &["note", "dr-000001", "Evidence arrived"]);
    let att = dir.join("evidence.txt");
    std::fs::write(&att, "evidence\n").unwrap();
    env.json(
        &dir,
        &[
            "attach",
            "dr-000001",
            att.to_str().unwrap(),
            "--caption",
            "proof",
        ],
    );
    let show1 = env.json(&dir, &["show", "dr-000001"]);
    assert_eq!(show1["decisions"][0]["seen_scope"], token1);
    assert_eq!(show1["decisions"][0]["scope_changed_since_open"], false);

    // Missing --seen-scope refuses drop accept, leaving status and bytes intact.
    let raw_before = std::fs::read_to_string(dir.join("tasks/dr-000001.md")).unwrap();
    let e_missing = err_json(
        &env,
        &dir,
        &["decision", "answer", "dr-000001", D1, "--accept", "Yes"],
    );
    assert_eq!(e_missing["error"]["kind"], "validation", "{e_missing}");
    assert_eq!(
        std::fs::read_to_string(dir.join("tasks/dr-000001.md")).unwrap(),
        raw_before
    );

    // Title edit after presentation stales token1!
    env.json(&dir, &["edit", "dr-000001", "--title", "Updated title"]);
    let e_stale = err_json(
        &env,
        &dir,
        &[
            "decision",
            "answer",
            "dr-000001",
            D1,
            "--accept",
            "Yes",
            "--seen-scope",
            &token1,
        ],
    );
    assert_eq!(e_stale["error"]["kind"], "stale_scope", "{e_stale}");
    let show2 = env.json(&dir, &["show", "dr-000001"]);
    assert_eq!(show2["task"]["status"], "todo");
    assert_eq!(show2["decisions"][0]["scope_changed_since_open"], true);
    let token2 = show2["decisions"][0]["seen_scope"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(token1, token2);

    // Open descendant refuses drop accept leaving both status and decision open.
    write_f1(&dir, "dr-000002", "todo", "parent: dr-000001\n", "");
    let e_desc = err_json(
        &env,
        &dir,
        &[
            "decision",
            "answer",
            "dr-000001",
            D1,
            "--accept",
            "Yes",
            "--seen-scope",
            &token2,
        ],
    );
    assert_eq!(e_desc["error"]["kind"], "open_descendants", "{e_desc}");
    env.json(&dir, &["done", "dr-000002", "closed child"]);

    // Fresh acceptance with token2 atomically drops task, removes pointer, appends resolution with seen_scope.
    let ans = env.json(
        &dir,
        &[
            "decision",
            "answer",
            "dr-000001",
            D1,
            "--accept",
            "Yes, drop it",
            "--seen-scope",
            &token2,
            "--by",
            "keith",
        ],
    );
    assert_eq!(ans["action"], "answered");
    assert_eq!(ans["replayed"], false);
    assert_eq!(ans["resolution"]["data"]["outcome"], "accept");
    assert_eq!(ans["resolution"]["data"]["seen_scope"], token2);
    let show3 = env.json(&dir, &["show", "dr-000001"]);
    assert_eq!(show3["task"]["status"], "dropped");
    assert!(show3["task"]["closed"].as_str().is_some());
    assert_eq!(show3["decisions"][0]["state"], "resolved");
    assert!(
        !show3["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"] == "answer")
    );

    // Matching replay after drop succeeds idempotently even though task is now dropped!
    let replay_ans = env.json(
        &dir,
        &[
            "decision",
            "answer",
            "dr-000001",
            D1,
            "--accept",
            "Yes, drop it",
            "--seen-scope",
            &token2,
            "--by",
            "keith",
        ],
    );
    assert_eq!(replay_ans["replayed"], true);
    assert_eq!(replay_ans["resolution"]["id"], ans["resolution"]["id"]);

    // Conflicting second answer (decline) conflicts!
    let conflict = err_json(
        &env,
        &dir,
        &[
            "decision",
            "answer",
            "dr-000001",
            D1,
            "--decline",
            "Actually keep",
            "--by",
            "keith",
        ],
    );
    assert_eq!(conflict["error"]["kind"], "decision_conflict", "{conflict}");
}

#[test]
fn proposal_holds_start_refusal_scope_verdict_and_pending_queue() {
    let mut env = TestEnv::new();
    let dir = env.init("hl");

    // 1. Idea with combined --scope-verdict + drop: marks reviewed, holds assess/curate/pickup/resume.
    write_f1(&dir, "hl-000001", "idea", "", "\nIdea body.\n");
    let opened_idea = env.json(
        &dir,
        &[
            "decision",
            "open",
            "hl-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop redundant idea",
            "--scope-verdict",
            "scope: drop; covered elsewhere",
        ],
    );
    assert_eq!(opened_idea["replayed"], false);
    let show_idea = env.json(&dir, &["show", "hl-000001"]);
    assert_eq!(show_idea["task"]["reviewed"], true);
    assert_eq!(show_idea["needs_review"], false);
    // Replay matching --scope-verdict does not duplicate the note!
    let note_count = show_idea["task"]["notes"].as_array().unwrap().len();
    let by = opened_idea["decision"]["by"].as_str().unwrap().to_string();
    let rep = env.json(
        &dir,
        &[
            "decision",
            "open",
            "hl-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop redundant idea",
            "--scope-verdict",
            "scope: drop; covered elsewhere",
            "--by",
            &by,
        ],
    );
    assert_eq!(rep["replayed"], true);
    let show_idea2 = env.json(&dir, &["show", "hl-000001"]);
    assert_eq!(
        show_idea2["task"]["notes"].as_array().unwrap().len(),
        note_count
    );

    // 2. Unassessed todo with drop: holds assess (fixing tasks-36485f!), curate, ready, next, AND refuses ALL starts!
    write_f1(
        &dir,
        "hl-000002",
        "todo",
        "",
        "\nTodo body matching keyword alpha.\n",
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "hl-000002",
            "--id",
            D2,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop todo beta",
            "--scope-verdict",
            "scope: drop; premise gone",
        ],
    );
    let show_todo = env.json(&dir, &["show", "hl-000002"]);
    assert_eq!(show_todo["task"]["status"], "todo");
    assert!(show_todo["task"].get("process").is_none());
    assert_eq!(show_todo["needs_review"], false);
    let ready = env.json(&dir, &["ready"]);
    assert!(
        !ready["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "hl-000002")
    );
    for args in [
        vec!["start", "hl-000002"],
        vec!["start", "hl-000002", "--force", "--reason", "try force"],
        vec!["edit", "hl-000002", "--status", "doing"],
    ] {
        let e = err_json(&env, &dir, &args);
        assert_eq!(e["error"]["kind"], "decision_held", "{args:?}: {e}");
    }

    // 3. Doing task gets stop-and-park warning when drop opens, AND repeat start by current owner is refused!
    write_f1(
        &dir,
        "hl-000003",
        "todo",
        "process: direct\n",
        "\nActive work.\n",
    );
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:owner")
        .args(["start", "hl-000003"])
        .assert()
        .success();
    let drop_doing = env.json(
        &dir,
        &[
            "decision",
            "open",
            "hl-000003",
            "--id",
            D3,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Stop this work",
        ],
    );
    assert!(
        drop_doing["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("park")),
        "drop on doing must warn to wind down and park: {drop_doing}"
    );
    let repeat_start = env
        .cmd(&dir)
        .env("TASKS_SESSION", "test:owner")
        .args(["start", "hl-000003"])
        .output()
        .unwrap();
    assert!(
        !repeat_start.status.success(),
        "repeat start by owner must be refused"
    );
    // Withdraw with multiline reason clears the hold!
    let wd = env.json(
        &dir,
        &[
            "decision",
            "withdraw",
            "hl-000003",
            D3,
            "--reason",
            "Line one\nLine two",
            "--by",
            "keith",
        ],
    );
    assert_eq!(wd["action"], "withdrawn");
    assert_eq!(wd["resolution"]["data"]["outcome"], "withdraw");
    assert_eq!(wd["resolution"]["data"]["text"], "Line one\nLine two");

    // 4. Suggestion holds curate only, NOT assess or start.
    write_f1(&dir, "hl-000004", "todo", "", "\nSuggestion target.\n");
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "hl-000004",
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Close as delivered?",
        ],
    );
    let show_sug = env.json(&dir, &["show", "hl-000004"]);
    assert_eq!(
        show_sug["needs_review"], true,
        "suggestion must not hold assess"
    );
    let sample = env.json(&dir, &["sample", "--older-than", "0d", "--limit", "10"]);
    assert!(
        !sample["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "hl-000004"),
        "suggestion must hold curate/sample"
    );
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:sug")
        .args(["start", "hl-000004"])
        .assert()
        .success();

    // 5. list --pending and search --pending wire contract + legacy waits + title/body-only search.
    write_f1(
        &dir,
        "hl-000005",
        "idea",
        "reviewed: true\nawaiting_user: true\nreviewed_at: 2020-01-02T00:00:00Z\n",
        "\n## Open questions\n\nWhich mode?\n",
    );
    write_f1(
        &dir,
        "hl-000006",
        "idea",
        "reviewed: true\n",
        "\nIdentifier control.\n\n## Notes\n\n- 2020-01-03T00:00:00Z (scope): scope: checked proposal::pending\n",
    );
    let pending = env.json(&dir, &["list", "--pending"]);
    let items = pending["items"].as_array().unwrap();
    // hl-000005 (legacy awaiting_user at 2020-01-02) must appear first (oldest), hl-000006 must NOT appear!
    assert_eq!(items[0]["type"], "legacy_wait");
    assert_eq!(items[0]["source"], "awaiting_user");
    assert_eq!(items[0]["task"]["id"], "hl-000005");
    assert_eq!(items[0]["host"], json!(null));
    assert_eq!(items[0]["reason"], json!(null));
    assert!(
        !items.iter().any(|i| i["task"]["id"] == "hl-000006"),
        "identifier proposal::pending must not produce a legacy_wait: {items:?}"
    );
    // search --pending matches title/body ("alpha") -> finds hl-000002; decision-only text ("beta") -> 0 rows!
    let s_hit = env.json(&dir, &["search", "alpha", "--pending"]);
    assert_eq!(s_hit["items"].as_array().unwrap().len(), 1);
    assert_eq!(s_hit["items"][0]["task"]["id"], "hl-000002");
    let s_miss = env.json(&dir, &["search", "beta", "--pending"]);
    assert_eq!(s_miss["items"].as_array().unwrap().len(), 0);
    // --status done yields 0 pending items.
    let s_closed = env.json(&dir, &["list", "--pending", "--status", "done"]);
    assert_eq!(s_closed["items"].as_array().unwrap().len(), 0);
}

#[test]
fn foreign_claim_concurrent_answers_replay_cleanup_decline_holds_and_overtaken_reopen() {
    let mut env = TestEnv::new();
    let dir = env.init("eg");

    // 1. Foreign live claim refuses drop accept, leaving status and decision intact.
    write_f1(
        &dir,
        "eg-000001",
        "todo",
        "process: direct\n",
        "\nClaimed target.\n",
    );
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:foreign")
        .args(["start", "eg-000001"])
        .assert()
        .success();
    let opened = env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop while claimed",
        ],
    );
    let token = opened["decision"]["seen_scope"]
        .as_str()
        .unwrap()
        .to_string();
    let e_claimed = err_json(
        &env,
        &dir,
        &[
            "decision",
            "answer",
            "eg-000001",
            D1,
            "--accept",
            "Drop it",
            "--seen-scope",
            &token,
            "--by",
            "keith",
        ],
    );
    assert_eq!(e_claimed["error"]["kind"], "claimed", "{e_claimed}");
    assert_eq!(
        env.json(&dir, &["show", "eg-000001"])["task"]["status"],
        "doing"
    );

    // Same session answers drop -> succeeds atomically and clears local claim.
    let ans = env
        .cmd(&dir)
        .env("TASKS_SESSION", "test:foreign")
        .args([
            "decision",
            "answer",
            "eg-000001",
            D1,
            "--accept",
            "Drop it",
            "--seen-scope",
            &token,
            "--by",
            "keith",
        ])
        .output()
        .unwrap();
    assert!(ans.status.success(), "{ans:?}");
    assert!(
        env.json(&dir, &["claims"])["claims"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Simulate interrupted local cleanup after durable drop save: re-inject claim, replay exact answer -> clears claim!
    common::write_claim_in_view(&env, "eg", "eg-000001", "test:foreign", "0:1");
    assert_eq!(
        env.json(&dir, &["claims"])["claims"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let replay = env.json(
        &dir,
        &[
            "decision",
            "answer",
            "eg-000001",
            D1,
            "--accept",
            "Drop it",
            "--seen-scope",
            &token,
            "--by",
            "keith",
        ],
    );
    assert_eq!(replay["replayed"], true);
    assert!(
        env.json(&dir, &["claims"])["claims"]
            .as_array()
            .unwrap()
            .is_empty(),
        "replay of accepted drop must recover local claim cleanup"
    );

    // 2. Concurrent incompatible answers race under mutation lock: one succeeds, one gets decision_conflict.
    write_f1(
        &dir,
        "eg-000002",
        "todo",
        "process: direct\n",
        "\nRace target.\n",
    );
    let opened2 = env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000002",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Race drop",
        ],
    );
    let token2 = opened2["decision"]["seen_scope"]
        .as_str()
        .unwrap()
        .to_string();
    let bin = assert_cmd::cargo::cargo_bin("tasks");
    let home = env.home.path().to_path_buf();
    let d_a = dir.clone();
    let d_b = dir.clone();
    let b_a = bin.clone();
    let b_b = bin.clone();
    let h_a = home.clone();
    let h_b = home.clone();
    let tok_a = token2.clone();
    let t_accept = std::thread::spawn(move || {
        std::process::Command::new(b_a)
            .current_dir(&d_a)
            .env("HOME", &h_a)
            .env("XDG_CONFIG_HOME", h_a.join(".config"))
            .env("XDG_STATE_HOME", h_a.join(".local/state"))
            .env("TASKS_SESSION", "test:race-a")
            .args([
                "decision",
                "answer",
                "eg-000002",
                D1,
                "--accept",
                "Accept",
                "--seen-scope",
                &tok_a,
                "--by",
                "keith",
            ])
            .output()
            .unwrap()
    });
    let t_decline = std::thread::spawn(move || {
        std::process::Command::new(b_b)
            .current_dir(&d_b)
            .env("HOME", &h_b)
            .env("XDG_CONFIG_HOME", h_b.join(".config"))
            .env("XDG_STATE_HOME", h_b.join(".local/state"))
            .env("TASKS_SESSION", "test:race-b")
            .args([
                "decision",
                "answer",
                "eg-000002",
                D1,
                "--decline",
                "Decline",
                "--by",
                "keith",
            ])
            .output()
            .unwrap()
    });
    let r_a = t_accept.join().unwrap();
    let r_b = t_decline.join().unwrap();
    assert_ne!(
        r_a.status.success(),
        r_b.status.success(),
        "exactly one concurrent incompatible answer must succeed"
    );
    let loser = if r_a.status.success() { r_b } else { r_a };
    let loser_err: Value = serde_json::from_slice(&loser.stderr).unwrap();
    assert_eq!(loser_err["error"]["kind"], "decision_conflict");
    // Record stays valid.
    assert!(
        env.cmd(&dir)
            .args(["check"])
            .output()
            .unwrap()
            .status
            .success()
    );

    // 3. Decline clears only that decision; second open drop + open dependency still hold!
    write_f1(
        &dir,
        "eg-000003",
        "todo",
        "process: direct\n",
        "\nPrereq.\n",
    );
    write_f1(
        &dir,
        "eg-000004",
        "todo",
        "depends: [eg-000003]\n",
        "\nTwo drops.\n",
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000004",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "First drop",
        ],
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000004",
            "--id",
            D2,
            "--kind",
            "drop",
            "--origin",
            "curate",
            "--text",
            "Second drop",
        ],
    );
    env.json(
        &dir,
        &[
            "decision",
            "answer",
            "eg-000004",
            D1,
            "--decline",
            "Keep for now",
            "--by",
            "keith",
        ],
    );
    // Second open drop still holds assess, ready, and start!
    let show4 = env.json(&dir, &["show", "eg-000004"]);
    assert_eq!(show4["needs_review"], false);
    assert_eq!(
        err_json(&env, &dir, &["start", "eg-000004"])["error"]["kind"],
        "decision_held"
    );
    // Decline second drop -> now assess is available (needs_review true), but dependency still blocks ready!
    env.json(
        &dir,
        &[
            "decision",
            "answer",
            "eg-000004",
            D2,
            "--decline",
            "Still keep",
            "--by",
            "keith",
        ],
    );
    let show4b = env.json(&dir, &["show", "eg-000004"]);
    assert_eq!(show4b["needs_review"], true);
    assert!(
        !env.json(&dir, &["ready"])["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "eg-000004"),
        "open dependency must still block ready after drops declined"
    );

    // 4. Suggestion acceptance records intent without closing; unrelated close hides unanswered decision
    //    until reopened!
    write_f1(
        &dir,
        "eg-000005",
        "todo",
        "process: direct\n",
        "\nSuggestion + overtake.\n",
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000005",
            "--id",
            D1,
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Close as delivered",
        ],
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000005",
            "--id",
            D2,
            "--kind",
            "suggestion",
            "--origin",
            "curate",
            "--text",
            "Second suggestion",
        ],
    );
    env.json(
        &dir,
        &[
            "decision",
            "answer",
            "eg-000005",
            D1,
            "--accept",
            "Agreed, will close separately",
            "--by",
            "keith",
        ],
    );
    let show5 = env.json(&dir, &["show", "eg-000005"]);
    assert_eq!(
        show5["task"]["status"], "todo",
        "accepting suggestion must not close task"
    );
    // Unrelated close hides D2 from pending queue & show answer actions, keeps it as overtaken in show.decisions.
    env.json(&dir, &["done", "eg-000005", "delivered"]);
    let show5_closed = env.json(&dir, &["show", "eg-000005"]);
    assert!(
        !show5_closed["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"] == "answer")
    );
    assert_eq!(show5_closed["decisions"][1]["state"], "overtaken");
    let pend_closed = env.json(&dir, &["list", "--pending"]);
    assert!(
        !pend_closed["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["task"]["id"] == "eg-000005")
    );
    // Reopen exposes D2 again!
    env.json(&dir, &["edit", "eg-000005", "--status", "todo"]);
    let pend_reopen = env.json(&dir, &["list", "--pending"]);
    assert!(
        pend_reopen["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["task"]["id"] == "eg-000005" && i["decision"]["id"] == D2)
    );

    // 5. Shelved task with open decision appears in list --pending, and --limit truncates after sort.
    write_f1(
        &dir,
        "eg-000006",
        "shelved",
        "defer: 2099-01-01\n",
        "\nShelved with decision.\n",
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "eg-000006",
            "--id",
            D3,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop shelved",
        ],
    );
    let pend_all = env.json(&dir, &["list", "--pending"]);
    assert!(
        pend_all["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["task"]["id"] == "eg-000006")
    );
    let pend_lim = env.json(&dir, &["list", "--pending", "-n", "1"]);
    assert_eq!(pend_lim["items"].as_array().unwrap().len(), 1);
    assert_eq!(pend_lim["items"][0], pend_all["items"][0]);
}
