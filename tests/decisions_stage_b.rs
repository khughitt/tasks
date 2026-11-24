//! Stage B pending decisions: questions, revision-pinned approvals, confirmed awaiting_user
//! conversion, linked human-request parks, and two-host crash/repair handoff matrix (Task 5).

mod common;

use common::TestEnv;
use serde_json::{Value, json};
use std::path::Path;

const D1: &str = "6f76bdab-e417-4ae7-81ce-09fc70d6487a";
const D2: &str = "1b4e28ba-2fa1-4d2f-9c83-2d3f9c0a1b2c";
const D3: &str = "3c5e7a90-1234-4abc-8def-0123456789ab";
const D4: &str = "4d6f8b01-2345-4bcd-9ef0-123456789abc";
const D5: &str = "5e709c12-3456-4cde-8f01-23456789abcd";
const COMMIT1: &str = "0123456789abcdef0123456789abcdef01234567";
const SHA1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
fn questions_approvals_pins_and_confirmed_awaiting_user_conversion() {
    let mut env = TestEnv::new();
    let dir = env.init("sb");

    // 1. Format-1 awaiting_user: true idea requires --replace-awaiting-user with --kind question.
    write_f1(
        &dir,
        "sb-000001",
        "idea",
        "reviewed: true\nawaiting_user: true\nreviewed_at: 2026-11-20T00:00:00Z\n",
        "\n## Open questions\n\nInclude withdrawn rows?\n",
    );
    // Ordinary open without --replace-awaiting-user refuses.
    let e_ord = err_json(
        &env,
        &dir,
        &[
            "decision",
            "open",
            "sb-000001",
            "--id",
            D1,
            "--kind",
            "question",
            "--origin",
            "scope",
            "--text",
            "Include withdrawn rows?",
        ],
    );
    assert_eq!(e_ord["error"]["kind"], "validation", "{e_ord}");
    // --replace-awaiting-user with non-question kind refuses.
    let e_drop = err_json(
        &env,
        &dir,
        &[
            "decision",
            "open",
            "sb-000001",
            "--id",
            D1,
            "--kind",
            "drop",
            "--origin",
            "scope",
            "--text",
            "Drop instead",
            "--replace-awaiting-user",
        ],
    );
    assert_eq!(e_drop["error"]["kind"], "validation", "{e_drop}");

    // Confirmed conversion removes physical awaiting_user, opens named question, and keeps derived awaiting_user = true.
    let conv = env.json(
        &dir,
        &[
            "decision",
            "open",
            "sb-000001",
            "--id",
            D1,
            "--kind",
            "question",
            "--origin",
            "scope",
            "--text",
            "Include withdrawn rows?",
            "--replace-awaiting-user",
            "--by",
            "scope",
            "--source",
            "note:2026-11-20T00:00:00Z",
        ],
    );
    assert_eq!(conv["replayed"], false);
    assert_eq!(conv["decision"]["data"]["kind"], "question");
    let raw = std::fs::read_to_string(dir.join("tasks/sb-000001.md")).unwrap();
    assert!(raw.starts_with("---\nformat: 2\n"));
    assert!(
        !raw.contains("awaiting_user:"),
        "physical format-2 file must not contain awaiting_user: {raw}"
    );
    assert!(
        env.cmd(&dir)
            .args(["check"])
            .output()
            .unwrap()
            .status
            .success()
    );

    // Retry after conversion returns the exact same opening receipt!
    let retry = env.json(
        &dir,
        &[
            "decision",
            "open",
            "sb-000001",
            "--id",
            D1,
            "--kind",
            "question",
            "--origin",
            "scope",
            "--text",
            "Include withdrawn rows?",
            "--replace-awaiting-user",
            "--by",
            "scope",
            "--source",
            "note:2026-11-20T00:00:00Z",
        ],
    );
    assert_eq!(retry["replayed"], true);
    assert_eq!(retry["decision"]["at"], conv["decision"]["at"]);

    // Read projection derives awaiting_user = true while open, and rejects contradictory legacy setters!
    let show_open = env.json(&dir, &["show", "sb-000001"]);
    assert_eq!(show_open["task"]["awaiting_user"], true);
    assert_eq!(show_open["needs_review"], false);
    assert!(
        !show_open["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"] == "answer" && a["subject"]["source"] == "awaiting_user"),
        "format-2 task must not emit duplicate legacy awaiting_user answer action: {show_open}"
    );
    let e_legacy = err_json(&env, &dir, &["edit", "sb-000001", "--no-awaiting-user"]);
    assert_eq!(e_legacy["error"]["kind"], "validation", "{e_legacy}");
    let e_legacy2 = err_json(
        &env,
        &dir,
        &["note", "sb-000001", "hi", "--reviewed", "--awaiting-user"],
    );
    assert_eq!(e_legacy2["error"]["kind"], "validation", "{e_legacy2}");

    // Question refuses --accept / --decline, succeeds with --reply, and clears derived awaiting_user.
    assert_eq!(
        err_json(
            &env,
            &dir,
            &["decision", "answer", "sb-000001", D1, "--accept", "yes"]
        )["error"]["kind"],
        "validation"
    );
    let replied = env.json(
        &dir,
        &[
            "decision",
            "answer",
            "sb-000001",
            D1,
            "--reply",
            "Include withdrawn rows.",
            "--by",
            "keith",
        ],
    );
    assert_eq!(replied["resolution"]["data"]["outcome"], "reply");
    let show_done = env.json(&dir, &["show", "sb-000001"]);
    assert_eq!(show_done["task"]["awaiting_user"], false);
    assert_eq!(show_done["needs_review"], true);

    // 2. Approval requires exact one complete pin form (commit or digest) and survives artifact edits!
    write_f1(
        &dir,
        "sb-000002",
        "todo",
        "process: direct\n",
        "\nPlan task.\n",
    );
    std::fs::create_dir_all(dir.join("docs/plans")).unwrap();
    std::fs::write(dir.join("docs/plans/p.md"), "Version 1\n").unwrap();

    // Missing pin or mixed/incomplete pins refused.
    for bad in [
        vec![
            "decision",
            "open",
            "sb-000002",
            "--kind",
            "approval",
            "--origin",
            "workflow",
            "--text",
            "Approve?",
        ],
        vec![
            "decision",
            "open",
            "sb-000002",
            "--kind",
            "approval",
            "--origin",
            "workflow",
            "--text",
            "Approve?",
            "--artifact-path",
            "docs/plans/p.md",
        ],
        vec![
            "decision",
            "open",
            "sb-000002",
            "--kind",
            "approval",
            "--origin",
            "workflow",
            "--text",
            "Approve?",
            "--artifact-repo",
            "records",
            "--artifact-path",
            "docs/plans/p.md",
            "--artifact-commit",
            COMMIT1,
            "--artifact-sha256",
            SHA1,
        ],
    ] {
        let e = err_json(&env, &dir, &bad);
        assert_eq!(e["error"]["kind"], "validation", "{bad:?}: {e}");
    }

    // Valid commit-pinned approval opens; holds curate only (NOT assess, NOT start!).
    let appr = env.json(
        &dir,
        &[
            "decision",
            "open",
            "sb-000002",
            "--id",
            D2,
            "--kind",
            "approval",
            "--origin",
            "workflow",
            "--text",
            "Approve plan rev A?",
            "--artifact-repo",
            "records",
            "--artifact-path",
            "tasks/docs/plans/p.md",
            "--artifact-commit",
            COMMIT1,
        ],
    );
    assert_eq!(
        appr["decision"]["subject"],
        json!({"repo": "records", "path": "tasks/docs/plans/p.md", "commit": COMMIT1})
    );
    assert_eq!(appr["decision"]["seen_scope"], json!(null));
    assert_eq!(appr["decision"]["scope_changed_since_open"], json!(null));

    // Modify artifact on disk -> approval stays open for its original pinned revision and can still be accepted!
    std::fs::write(dir.join("docs/plans/p.md"), "Version 2\n").unwrap();
    let ans = env.json(
        &dir,
        &[
            "decision",
            "answer",
            "sb-000002",
            D2,
            "--accept",
            "Approved rev A",
            "--by",
            "keith",
        ],
    );
    assert_eq!(ans["resolution"]["data"]["outcome"], "accept");
    assert_eq!(
        ans["decision"]["subject"],
        json!({"repo": "records", "path": "tasks/docs/plans/p.md", "commit": COMMIT1})
    );
}

#[test]
fn linked_parks_two_host_repair_newer_handoff_and_legacy_drain() {
    let mut env = TestEnv::new();
    let dir = env.init("lp");

    // 1. Unlinked user review/decision/approval park is REFUSED; non-request parks still work!
    write_f1(
        &dir,
        "lp-000001",
        "todo",
        "process: direct\n",
        "\nPark target.\n",
    );
    for reason in ["review", "decision", "approval"] {
        let e = err_json(
            &env,
            &dir,
            &[
                "park",
                "lp-000001",
                "Next step",
                "--waiting-on",
                "user",
                "--reason",
                reason,
            ],
        );
        assert_eq!(e["error"]["kind"], "validation", "unlinked {reason}: {e}");
    }
    // Non-request user park (environment) succeeds without --decision, omits decision_id.
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:host-a")
        .args([
            "park",
            "lp-000001",
            "Fix env",
            "--waiting-on",
            "user",
            "--reason",
            "environment",
        ])
        .assert()
        .success();
    let show_env = env.json(&dir, &["show", "lp-000001"]);
    assert!(show_env["park"].get("decision_id").is_none());

    // 2. Inline-open linked park creates decision + park in one durable write; list --pending has NO duplicate legacy park row!
    let next_step = "Resume implementation after approval";
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:host-a")
        .args([
            "park",
            "lp-000001",
            next_step,
            "--waiting-on",
            "user",
            "--reason",
            "approval",
            "--decision",
            D1,
            "--decision-kind",
            "approval",
            "--decision-origin",
            "workflow",
            "--decision-text",
            "Approve spec?",
            "--artifact-path",
            "docs/specs/s.md",
            "--artifact-sha256",
            SHA1,
        ])
        .assert()
        .success();

    let show_linked = env.json(&dir, &["show", "lp-000001"]);
    assert_eq!(show_linked["park"]["decision_id"], D1);
    assert_eq!(show_linked["park"]["waiting_on"], "user");
    let parked = env.json(&dir, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["park"]["decision_id"], D1);
    let pending = env.json(&dir, &["list", "--pending"]);
    assert_eq!(
        pending["items"].as_array().unwrap().len(),
        1,
        "linked park must not emit a duplicate legacy_wait park row: {pending}"
    );
    assert_eq!(pending["items"][0]["type"], "decision");
    assert_eq!(pending["items"][0]["decision"]["id"], D1);

    // 3. Simulate second independent host store (Host B) holding the SAME linked park on D1.
    let host_b_state = tempfile::tempdir().unwrap();
    let store_a = env.claim_store("lp");
    let store_b_dir = host_b_state.path().join("tasks/claims");
    std::fs::create_dir_all(&store_b_dir).unwrap();
    std::fs::copy(&store_a, store_b_dir.join("lp.toml")).unwrap();

    // Host A answers D1 -> Host A's park immediately hands off to waiting_on = agent with IDENTICAL next_step!
    env.json(
        &dir,
        &[
            "decision",
            "answer",
            "lp-000001",
            D1,
            "--accept",
            "Approved",
            "--by",
            "keith",
        ],
    );
    let show_a = env.json(&dir, &["show", "lp-000001"]);
    assert_eq!(show_a["park"]["waiting_on"], "agent");
    assert_eq!(show_a["park"]["next_step"], next_step);
    assert!(show_a["park"].get("decision_id").is_none());

    // Host B reads show with its own stale XDG_STATE_HOME -> derives agent handoff AND repairs its local store!
    let out_b = env
        .cmd(&dir)
        .env("XDG_STATE_HOME", host_b_state.path())
        .args(["show", "lp-000001"])
        .output()
        .unwrap();
    assert!(out_b.status.success(), "{out_b:?}");
    let show_b: Value = serde_json::from_slice(&out_b.stdout).unwrap();
    assert_eq!(show_b["park"]["waiting_on"], "agent");
    assert_eq!(show_b["park"]["next_step"], next_step);
    assert!(show_b["park"].get("decision_id").is_none());

    // 4. Newer handoff survives a late answer/repair for an older decision!
    write_f1(
        &dir,
        "lp-000002",
        "todo",
        "process: direct\n",
        "\nTwo handoffs.\n",
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "lp-000002",
            "--id",
            D1,
            "--kind",
            "question",
            "--origin",
            "scope",
            "--text",
            "Old question?",
        ],
    );
    env.json(
        &dir,
        &[
            "decision",
            "open",
            "lp-000002",
            "--id",
            D2,
            "--kind",
            "question",
            "--origin",
            "scope",
            "--text",
            "Newer question?",
        ],
    );
    // Park explicitly linked to the NEWER decision D2.
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:host-a")
        .args([
            "park",
            "lp-000002",
            "Wait for newer question",
            "--waiting-on",
            "user",
            "--reason",
            "decision",
            "--decision",
            D2,
        ])
        .assert()
        .success();
    // Late answer arrives for OLD decision D1 -> MUST NOT flip the newer D2 park to agent!
    env.json(
        &dir,
        &[
            "decision",
            "answer",
            "lp-000002",
            D1,
            "--reply",
            "Late reply to D1",
            "--by",
            "keith",
        ],
    );
    let show2 = env.json(&dir, &["show", "lp-000002"]);
    assert_eq!(show2["park"]["waiting_on"], "user");
    assert_eq!(show2["park"]["decision_id"], D2);
    assert_eq!(show2["park"]["next_step"], "Wait for newer question");

    // 5. Existing unlinked legacy user park remains visible in list --pending and drains on start/done
    //    without synthesizing any decision!
    write_f1(
        &dir,
        "lp-000003",
        "todo",
        "process: direct\n",
        "\nLegacy park.\n",
    );
    // Inject a pre-Stage-B unlinked user review park directly into the host-local store.
    let mut store_text = std::fs::read_to_string(&store_a).unwrap();
    store_text.push_str(
        "\n[parks.\"lp-000003\"]\nowner = \"tester\"\nsession = \"test:old\"\nhost = \"testhost\"\nworktree = \"",
    );
    store_text.push_str(dir.to_str().unwrap());
    store_text.push_str(
        "\"\nat = \"2026-11-01T00:00:00Z\"\nnext_step = \"Old unlinked review\"\nwaiting_on = \"user\"\nreason = \"review\"\ntitle = \"Task lp-000003\"\n",
    );
    std::fs::write(&store_a, store_text).unwrap();

    let raw_before = std::fs::read_to_string(dir.join("tasks/lp-000003.md")).unwrap();
    let pend3 = env.json(&dir, &["list", "--pending"]);
    assert!(
        pend3["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "legacy_wait"
                && i["source"] == "park"
                && i["task"]["id"] == "lp-000003"),
        "legacy unlinked park must remain visible in list --pending: {pend3}"
    );
    // Plain read never migrates or opens a decision.
    assert_eq!(
        std::fs::read_to_string(dir.join("tasks/lp-000003.md")).unwrap(),
        raw_before
    );
    // Drains cleanly via start + done without creating any decision or upgrading format.
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:drain")
        .args(["start", "lp-000003"])
        .assert()
        .success();
    env.cmd(&dir)
        .env("TASKS_SESSION", "test:drain")
        .args(["done", "lp-000003", "drained"])
        .assert()
        .success();
    let raw_after = std::fs::read_to_string(dir.join("tasks/lp-000003.md")).unwrap();
    assert!(!raw_after.contains("format: 2"));
    let _ = D3;
}

#[test]
fn format_two_show_assess_uses_per_kind_holds_without_legacy_awaiting_user_and_drop_warns_with_decision_id()
 {
    let mut env = TestEnv::new();
    let dir = env.init("as");

    // 4 unreviewed ideas (so assess is otherwise available unless held by decision kind).
    for (id, kind, entry_id, extra) in [
        ("as-000001", "question", D1, vec![]),
        ("as-000002", "drop", D2, vec![]),
        ("as-000003", "suggestion", D3, vec![]),
        (
            "as-000004",
            "approval",
            D4,
            vec!["--artifact-path", "docs/p.md", "--artifact-sha256", SHA1],
        ),
    ] {
        write_f1(&dir, id, "idea", "reviewed: true\n", "\nIdea body.\n");
        let mut args = vec![
            "decision", "open", id, "--id", entry_id, "--kind", kind, "--origin", "scope",
            "--text", "Check me", "--by", "scope",
        ];
        args.extend(extra);
        env.json(&dir, &args);
    }

    let find_assess = |id: &str| {
        let shown = env.json(&dir, &["show", id]);
        assert_eq!(shown["task"]["awaiting_user"], true, "{id}: {shown}");
        shown["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["action"] == "assess")
            .cloned()
            .expect("assess action present")
    };

    // question and drop hold assess via named entry only — never duplicate legacy awaiting_user!
    for (id, expected_entry, expected_kind) in
        [("as-000001", D1, "question"), ("as-000002", D2, "drop")]
    {
        let assess = find_assess(id);
        assert_eq!(assess["available"], false, "{id}: {assess}");
        let reasons = assess["reasons"].as_array().unwrap();
        assert_eq!(
            reasons.len(),
            1,
            "no duplicate awaiting_user blocker: {assess}"
        );
        assert_eq!(reasons[0]["code"], "decision_open");
        assert_eq!(reasons[0]["evidence"]["entry"], expected_entry);
        assert_eq!(reasons[0]["evidence"]["kind"], expected_kind);
    }

    // suggestion and approval do NOT hold assess on format-2 ideas!
    for id in ["as-000003", "as-000004"] {
        let assess = find_assess(id);
        let reasons = assess["reasons"].as_array().unwrap();
        assert!(
            !reasons.iter().any(|r| r["code"] == "decision_open"),
            "{id}: suggestion/approval must not block assess via awaiting_user: {assess}"
        );
    }

    // Opening a drop on a doing task warns with Stage B `--decision <entry_id>`.
    write_f1(
        &dir,
        "as-000005",
        "doing",
        "process: direct\n",
        "\nActive.\n",
    );
    let opened = env.json(
        &dir,
        &[
            "decision",
            "open",
            "as-000005",
            "--id",
            D5,
            "--kind",
            "drop",
            "--origin",
            "curate",
            "--text",
            "Should drop?",
            "--by",
            "curate",
        ],
    );
    let expected_flag = format!("--decision {D5}");
    assert!(
        opened["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains(&expected_flag)),
        "drop-on-doing warning must include `--decision <entry_id>`: {opened}"
    );
}
