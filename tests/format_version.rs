//! Mixed-version record reading, format-2 preservation, editor tampering guards,
//! and staged/plain check index guards (Task 2, reader-only boundary).

mod common;

use common::{TestEnv, git_fixture};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const D1: &str = "6f76bdab-e417-4ae7-81ce-09fc70d6487a";
const D2: &str = "1b4e28ba-2fa1-4d2f-9c83-2d3f9c0a1b2c";
const R1: &str = "9a3c4d5e-6f70-4812-a345-6789abcdef01";
const HASH1: &str = "ff13e9569c2bc118b1f6e76f89b964557b3eae8d66ff8ad3174849517704145f";
const COMMIT1: &str = "0123456789abcdef0123456789abcdef01234567";

fn write_f1(dir: &Path, id: &str, status: &str, extra_fm: &str, body: &str) {
    let depends = if extra_fm.contains("depends:") {
        ""
    } else {
        "depends: []\n"
    };
    let text = format!(
        "---\nid: {id}\ntitle: Task {id}\nstatus: {status}\npriority: 2\n\
         created: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\n\
         {extra_fm}{depends}tags: []\n---\n{body}"
    );
    std::fs::write(dir.join(format!("tasks/{id}.md")), text).unwrap();
}

fn write_f2_sample(dir: &Path, id: &str) -> String {
    let text = format!(
        "---\nformat: 2\nid: {id}\ntitle: Format two task\nstatus: todo\npriority: 2\n\
         open_decisions: [{D1}]\n\
         created: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\ndepends: []\ntags: []\n---\n\n\
         Body with literal Unicode: café — naïve.\n\n\
         ## References\n\n\
         - brief (input): docs/notes/b.md — brief\n\n\
         ## Entries\n\n\
         {D1} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"scope\",\"subject\":{{\"scope\":\"{HASH1}\"}},\"data\":{{\"kind\":\"drop\",\"origin\":\"scope\",\"text\":\"Line one\\nLine two — café\"}},\"provenance\":{{\"harness_session\":\"codex:sess-1\",\"harness_session_source\":\"CODEX_THREAD_ID\"}}}}\n\
         {D2} {{\"type\":\"decision\",\"at\":\"2026-11-21T08:00:00Z\",\"by\":\"author\",\"subject\":{{\"repo\":\"records\",\"path\":\"tasks/docs/plans/p.md\",\"commit\":\"{COMMIT1}\"}},\"data\":{{\"kind\":\"approval\",\"origin\":\"workflow\",\"text\":\"Approve?\"}}}}\n\
         {R1} {{\"type\":\"resolution\",\"at\":\"2026-11-21T09:00:00Z\",\"by\":\"human\",\"subject\":{{\"entry\":\"{D2}\"}},\"data\":{{\"outcome\":\"accept\",\"text\":\"Approved\",\"source\":\"direct\"}}}}\n\n\
         ## Notes\n\n\
         - 2026-11-20T00:00:00Z (author): created\n"
    );
    std::fs::write(dir.join(format!("tasks/{id}.md")), &text).unwrap();
    text
}

fn write_f3(dir: &Path, id: &str) -> String {
    let text = format!(
        "---\nformat: 3\nid: {id}\ntitle: Future record\nstatus: todo\npriority: 2\n\
         assessment: {D1}\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\n\
         depends: []\ntags: []\n---\n\nFuture body.\n"
    );
    std::fs::write(dir.join(format!("tasks/{id}.md")), &text).unwrap();
    text
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
fn format_two_roundtrips_and_ordinary_writes_preserve_version_and_entries() {
    let mut env = TestEnv::new();
    let dir = env.init("fm");
    std::fs::create_dir_all(dir.join("docs/notes")).unwrap();
    std::fs::write(dir.join("docs/notes/b.md"), "brief\n").unwrap();

    // Format 1 with reserved heading `## Entries` stays ordinary body on read/write.
    write_f1(
        &dir,
        "fm-000001",
        "todo",
        "",
        "\nIntro.\n\n## Entries\n\nNot a section in format 1.\n",
    );
    let shown1 = env.json(&dir, &["show", "fm-000001"]);
    assert!(
        shown1["task"]["body"]
            .as_str()
            .unwrap()
            .contains("## Entries"),
        "format-1 body must preserve ## Entries text: {shown1}"
    );
    let ok = env.json(&dir, &["note", "fm-000001", "ordinary note"]);
    assert_eq!(ok["id"], "fm-000001");
    let raw1 = std::fs::read_to_string(dir.join("tasks/fm-000001.md")).unwrap();
    assert!(
        !raw1.contains("format: 2"),
        "ordinary write must preserve format 1: {raw1}"
    );
    assert!(raw1.contains("## Entries\n\nNot a section in format 1."));

    // Format 2 round-trips entries, Unicode, escaped newlines, references, and preserves format 2 on ordinary writes.
    write_f2_sample(&dir, "fm-000002");
    let shown2 = env.json(&dir, &["show", "fm-000002"]);
    assert_eq!(shown2["task"]["id"], "fm-000002");
    let ok2 = env.json(&dir, &["edit", "fm-000002", "--tag", "checked"]);
    assert_eq!(ok2["id"], "fm-000002");
    let raw2 = std::fs::read_to_string(dir.join("tasks/fm-000002.md")).unwrap();
    assert!(raw2.starts_with("---\nformat: 2\n"), "{raw2}");
    assert!(raw2.contains(&format!("open_decisions: [{D1}]")), "{raw2}");
    assert!(raw2.contains("## References\n"), "{raw2}");
    assert!(raw2.contains("## Entries\n"), "{raw2}");
    assert!(raw2.contains("## Notes\n"), "{raw2}");
    assert!(
        raw2.find("## References").unwrap() < raw2.find("## Entries").unwrap()
            && raw2.find("## Entries").unwrap() < raw2.find("## Notes").unwrap(),
        "section order must be References -> Entries -> Notes: {raw2}"
    );
    assert!(
        raw2.contains("Line one\\nLine two — café"),
        "escaped newline and literal Unicode must round-trip: {raw2}"
    );

    // Format 2 with archived notes sidecar round-trips intact.
    let seg = b"## Notes\n\n- 2026-11-19T00:00:00Z (author): archived line\n";
    let sha = format!("{:x}", Sha256::digest(seg));
    let arc_name = format!("notes-{sha}.notes");
    std::fs::create_dir_all(dir.join("tasks/files/fm-000003")).unwrap();
    std::fs::write(dir.join(format!("tasks/files/fm-000003/{arc_name}")), seg).unwrap();
    let f2_arc = format!(
        "---\nformat: 2\nid: fm-000003\ntitle: Archived f2\nstatus: todo\npriority: 2\n\
         open_decisions: [{D1}]\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\n\
         depends: []\ntags: []\nnote_archives: [\"v1:{sha}:1\"]\n---\n\n\
         ## Entries\n\n\
         {D1} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"scope\",\"subject\":{{\"scope\":null}},\"data\":{{\"kind\":\"suggestion\",\"origin\":\"curate\",\"text\":\"Check\"}}}}\n\n\
         ## Notes\n\n\
         - 2026-11-20T00:00:00Z (author): attached: {arc_name} ({} bytes)\n",
        seg.len()
    );
    std::fs::write(dir.join("tasks/fm-000003.md"), &f2_arc).unwrap();
    let shown3 = env.json(&dir, &["show", "fm-000003"]);
    assert_eq!(shown3["task"]["notes"].as_array().unwrap().len(), 2);

    // `tasks check` passes on all valid format 1 + format 2 records.
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert!(
        out.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn mixed_version_collection_keeps_independent_rows_blocks_dependents_and_refuses_future_targets() {
    let mut env = TestEnv::new();
    let dir = env.init("mx");
    let other = env.init("ot");

    // 1. Independent runnable todo
    write_f1(&dir, "mx-000001", "todo", "process: direct\n", "");
    // 2. Dependent on future record mx-000009
    write_f1(
        &dir,
        "mx-000002",
        "todo",
        "process: direct\ndepends: [mx-000009]\n",
        "",
    );
    // 3. Goal with a future piece in `ot`
    let future_foreign = write_f3(&other, "ot-000009");
    write_f1(
        &dir,
        "mx-000003",
        "todo",
        "depends: [ot-000009]\npieces: [ot-000009]\n",
        "",
    );
    // 4. Redirect tombstone pointing to future foreign record
    write_f1(&dir, "mx-000004", "moved", "moved_to: ot-000009\n", "");
    write_f1(
        &dir,
        "mx-000005",
        "todo",
        "process: direct\ndepends: [mx-000004]\n",
        "",
    );
    // 5. Local future format-3 file
    let future_local = write_f3(&dir, "mx-000009");

    // `list` succeeds with warning naming id, path, and format.
    let listed = env.json(&dir, &["list"]);
    let ids: Vec<&str> = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"mx-000001"), "{listed}");
    assert!(!ids.contains(&"mx-000009"), "{listed}");
    let warns = listed["warnings"].as_array().unwrap();
    assert!(
        warns.iter().any(|w| {
            let s = w.as_str().unwrap();
            s.contains("mx-000009")
                && s.contains("tasks/mx-000009.md")
                && s.contains("3")
                && s.contains("unsupported_record_version")
        }),
        "list must warn about unsupported future record: {warns:?}"
    );

    // `ready` keeps mx-000001, excludes dependent mx-000002 and redirect-dependent mx-000005.
    let ready = env.json(&dir, &["ready"]);
    let ready_ids: Vec<&str> = ready["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ready_ids, vec!["mx-000001"], "ready: {ready}");

    // Single-record show/show --no-fetch/edit on future record refuse with unsupported_record_version.
    for args in [
        vec!["show", "mx-000009"],
        vec!["show", "mx-000009", "--no-fetch"],
        vec!["edit", "mx-000009", "--title", "Nope"],
        vec!["show", "ot-000009"],
    ] {
        let err = err_json(&env, &dir, &args);
        assert_eq!(
            err["error"]["kind"], "unsupported_record_version",
            "{args:?}: {err}"
        );
        let detail = err["error"]["detail"].as_str().unwrap();
        assert!(detail.contains("3"), "{detail}");
    }

    // Unsafe goal closure refuses when a declared piece or local sibling is unsupported.
    let close_err = err_json(&env, &dir, &["done", "mx-000003", "try close"]);
    assert!(
        matches!(
            close_err["error"]["kind"].as_str().unwrap(),
            "unsupported_record_version" | "unresolved_goal_members" | "open_dependencies"
        ),
        "must withhold unsafe closure: {close_err}"
    );

    // Bulk write (`rename`) refuses when unsupported record exists.
    let rename_err = err_json(&env, &dir, &["rename", "mx", "mx2"]);
    assert_eq!(
        rename_err["error"]["kind"], "unsupported_record_version",
        "{rename_err}"
    );

    // Future files stay byte-for-byte untouched.
    assert_eq!(
        std::fs::read_to_string(dir.join("tasks/mx-000009.md")).unwrap(),
        future_local
    );
    assert_eq!(
        std::fs::read_to_string(other.join("tasks/ot-000009.md")).unwrap(),
        future_foreign
    );

    // Malformed supported file still fails hard on list/ready.
    std::fs::write(
        dir.join("tasks/mx-000008.md"),
        "---\nformat: 2\nid: mx-000008\ntitle: Bad\nstatus: todo\npriority: 2\nawaiting_user: false\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\ndepends: []\ntags: []\n---\n",
    )
    .unwrap();
    let bad_err = err_json(&env, &dir, &["list"]);
    assert_eq!(bad_err["error"]["kind"], "parse", "{bad_err}");
}

#[test]
fn editor_refuses_entries_and_pointer_tampering_but_allows_ordinary_edits() {
    let mut env = TestEnv::new();
    let dir = env.init("ed");
    std::fs::create_dir_all(dir.join("docs/notes")).unwrap();
    std::fs::write(dir.join("docs/notes/b.md"), "brief\n").unwrap();
    write_f2_sample(&dir, "ed-000001");

    let run_editor = |script: &str| {
        let mut cmd = env.cmd(&dir);
        cmd.env("EDITOR", script);
        cmd.args(["edit", "ed-000001", "--editor"])
            .output()
            .unwrap()
    };

    // 1. Tamper: delete an entry line -> refused
    let out = run_editor(
        "python3 -c 'import sys; p=sys.argv[1]; t=open(p).read(); lines=[l for l in t.splitlines(True) if \"9a3c4d5e\" not in l]; open(p,\"w\").write(\"\".join(lines))'",
    );
    assert!(!out.status.success(), "deleting an entry must be refused");

    // 2. Tamper: reorder entries -> refused
    let out = run_editor(
        "python3 -c 'import sys; p=sys.argv[1]; t=open(p).read(); a,b=t.split(\"## Entries\\n\\n\"); e,n=b.split(\"\\n\\n## Notes\"); el=e.splitlines(); el[0],el[1]=el[1],el[0]; open(p,\"w\").write(a+\"## Entries\\n\\n\"+\"\\n\".join(el)+\"\\n\\n## Notes\"+n)'",
    );
    assert!(!out.status.success(), "reordering entries must be refused");

    // 3. Tamper: clear open_decisions pointer -> refused
    let out = run_editor(
        "python3 -c 'import sys; p=sys.argv[1]; t=open(p).read().replace(\"open_decisions: [6f76bdab-e417-4ae7-81ce-09fc70d6487a]\\n\", \"\"); open(p,\"w\").write(t)'",
    );
    assert!(
        !out.status.success(),
        "changing open_decisions must be refused"
    );

    // 4. Ordinary title edit preserves entries and pointers
    let out = run_editor(
        "python3 -c 'import sys; p=sys.argv[1]; t=open(p).read().replace(\"Format two task\", \"Edited title\"); open(p,\"w\").write(t)'",
    );
    assert!(
        out.status.success(),
        "ordinary editor edit should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = std::fs::read_to_string(dir.join("tasks/ed-000001.md")).unwrap();
    assert!(after.contains("title: Edited title"));
    assert!(after.contains(&format!("open_decisions: [{D1}]")));
}

fn init_git(home: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "test"],
        vec!["config", "user.email", "test@example.invalid"],
    ] {
        let out = git_fixture::command(home, dir)
            .args(&args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }
    git_fixture::assert_repo(home, dir);
}

#[test]
fn check_and_staged_check_enforce_changed_index_guard_and_warn_on_untouched_future_siblings() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let root = base.path().canonicalize().unwrap();
    let records = root.join("records");
    let code_a = root.join("code-a");
    let code_b = root.join("code-b");
    init_git(env.home.path(), &records);
    init_git(env.home.path(), &code_a);
    init_git(env.home.path(), &code_b);

    for (code, prefix) in [(&code_a, "pa"), (&code_b, "pb")] {
        let out = env
            .cmd(code)
            .args(["init", "--prefix", prefix, "--records"])
            .arg(&records)
            .output()
            .unwrap();
        assert!(out.status.success(), "init {prefix}: {out:?}");
    }
    let pa = records.join("pa");
    let pb = records.join("pb");

    // Seed pa with one valid f1 task, one untouched future f3 task (plus its attachment dir),
    // and commit everything as initial HEAD (before enabling hook).
    write_f1(&pa, "pa-000001", "todo", "", "");
    let f3_bytes = write_f3(&pa, "pa-000009");
    std::fs::create_dir_all(pa.join("tasks/files/pa-000009")).unwrap();
    std::fs::write(
        pa.join("tasks/files/pa-000009/blob.txt"),
        "future attachment\n",
    )
    .unwrap();
    write_f1(&pb, "pb-000001", "todo", "", "");

    let git = |args: &[&str]| {
        git_fixture::command(env.home.path(), &records)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(git(&["add", "-A"]).status.success());
    assert!(git(&["commit", "-qm", "seed"]).status.success());

    // Install a real pre-commit hook that runs plain `tasks check -C pa` (matching today's records hook).
    let hooks = root.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::os::unix::fs::symlink(
        assert_cmd::cargo::cargo_bin("tasks"),
        env.fake_bin().join("tasks"),
    )
    .unwrap();
    let hook_path = hooks.join("pre-commit");
    std::fs::write(
        &hook_path,
        format!(
            "#!/bin/sh\nset -eu\nexport PATH=\"{}:$PATH\"\nexport XDG_CONFIG_HOME=\"{}\"\nexport HOME=\"{}\"\nexec tasks -C \"{}\" check\n",
            env.fake_bin().display(),
            env.home.path().join(".config").display(),
            env.home.path().display(),
            pa.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        git(&["config", "core.hooksPath", hooks.to_str().unwrap()])
            .status
            .success()
    );

    // 1. Edit supported sibling beside untouched future file -> both `check` and `check --staged` pass with warning,
    //    no orphan attachment error for pa-000009, and real pre-commit commits cleanly.
    let noted = env.json(&code_a, &["note", "pa-000001", "first note"]);
    assert_eq!(noted["id"], "pa-000001");
    let out_plain = env.cmd(&pa).args(["check"]).output().unwrap();
    assert!(
        out_plain.status.success(),
        "plain check failed: {}",
        String::from_utf8_lossy(&out_plain.stdout)
    );
    let plain_json: Value = serde_json::from_slice(&out_plain.stdout).unwrap();
    assert!(
        plain_json["errors"].as_array().unwrap().is_empty(),
        "{plain_json}"
    );
    assert!(
        plain_json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unsupported_record_version" && w["id"] == "pa-000009"),
        "{plain_json}"
    );
    assert!(
        !plain_json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "orphan_attachment"),
        "must suppress orphan attachment for known future id: {plain_json}"
    );

    // First full staged audit (records accepted tree)
    let out_staged1 = env.cmd(&pa).args(["check", "--staged"]).output().unwrap();
    assert!(out_staged1.status.success(), "{out_staged1:?}");
    let staged1_json: Value = serde_json::from_slice(&out_staged1.stdout).unwrap();
    assert!(
        staged1_json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unsupported_record_version" && w["id"] == "pa-000009"),
        "first staged audit must keep future warning: {staged1_json}"
    );

    // Second note on pa-000001 (fast-path candidate after HEAD matches accepted tree):
    // stage a plain appended note and run check --staged again; untouched future warning stays!
    let mut pa1 = std::fs::read_to_string(pa.join("tasks/pa-000001.md")).unwrap();
    pa1.push_str("- 2026-11-24T12:00:00Z (author): fast path note\n");
    let pa1_updated = pa1
        .lines()
        .map(|l| {
            if l.starts_with("updated: ") {
                "updated: 2026-11-24T12:00:00Z"
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(pa.join("tasks/pa-000001.md"), &pa1_updated).unwrap();
    assert!(git(&["add", "pa/tasks/pa-000001.md"]).status.success());
    let out_staged2 = env.cmd(&pa).args(["check", "--staged"]).output().unwrap();
    assert!(out_staged2.status.success(), "{out_staged2:?}");
    let staged2_json: Value = serde_json::from_slice(&out_staged2.stdout).unwrap();
    assert!(
        staged2_json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["kind"] == "unsupported_record_version" && w["id"] == "pa-000009"),
        "fast-path staged audit must keep untouched future warning: {staged2_json}"
    );
    assert!(git(&["commit", "-qm", "fast path note"]).status.success());

    // 2. Stage a NEW future file, then overwrite working tree with valid f1 bytes ->
    //    BOTH `check --staged` AND plain `check` AND real pre-commit MUST FAIL!
    write_f3(&pa, "pa-00000a");
    assert!(git(&["add", "pa/tasks/pa-00000a.md"]).status.success());
    write_f1(&pa, "pa-00000a", "todo", "", ""); // working tree now looks valid!

    for args in [vec!["check", "--staged"], vec!["check"]] {
        let out = env.cmd(&pa).args(&args).output().unwrap();
        assert!(
            !out.status.success(),
            "{args:?} must refuse staged future blob even when working copy is valid"
        );
        let json: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            json["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "unsupported_record_version"
                    && e["file"] == "tasks/pa-00000a.md"),
            "{args:?}: {json}"
        );
    }
    let hook_fail = git(&["commit", "-qm", "should fail"]);
    assert!(
        !hook_fail.status.success(),
        "pre-commit hook must refuse staged future blob"
    );

    // Reset pa-00000a
    assert!(
        git(&["reset", "-q", "HEAD", "--", "pa/tasks/pa-00000a.md"])
            .status
            .success()
    );
    std::fs::remove_file(pa.join("tasks/pa-00000a.md")).unwrap();

    // 3. Stage supported malformed format-2 record (duplicate JSON key), overwrite working tree with valid bytes -> failure
    let dup_json = format!(
        "---\nformat: 2\nid: pa-00000b\ntitle: Dup\nstatus: todo\npriority: 2\n\
         open_decisions: [{D1}]\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\n\
         depends: []\ntags: []\n---\n\n## Entries\n\n\
         {D1} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"scope\",\"subject\":{{\"scope\":\"{HASH1}\",\"scope\":\"{HASH1}\"}},\"data\":{{\"kind\":\"drop\",\"origin\":\"scope\",\"text\":\"x\"}}}}\n"
    );
    std::fs::write(pa.join("tasks/pa-00000b.md"), &dup_json).unwrap();
    assert!(git(&["add", "pa/tasks/pa-00000b.md"]).status.success());
    write_f1(&pa, "pa-00000b", "todo", "", "");
    let out_dup = env.cmd(&pa).args(["check", "--staged"]).output().unwrap();
    assert!(
        !out_dup.status.success(),
        "staged duplicate JSON key must fail"
    );
    let out_dup_plain = env.cmd(&pa).args(["check"]).output().unwrap();
    assert!(
        !out_dup_plain.status.success(),
        "plain check must also refuse staged duplicate JSON key"
    );
    assert!(
        git(&["reset", "-q", "HEAD", "--", "pa/tasks/pa-00000b.md"])
            .status
            .success()
    );
    std::fs::remove_file(pa.join("tasks/pa-00000b.md")).unwrap();

    // 3b. Stage supported malformed format-2 record with dangling open_decisions pointer -> failure
    let dangling = format!(
        "---\nformat: 2\nid: pa-00000c\ntitle: Dangling\nstatus: todo\npriority: 2\n\
         open_decisions: [{D1}]\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\n\
         depends: []\ntags: []\n---\n"
    );
    std::fs::write(pa.join("tasks/pa-00000c.md"), &dangling).unwrap();
    assert!(git(&["add", "pa/tasks/pa-00000c.md"]).status.success());
    write_f1(&pa, "pa-00000c", "todo", "", "");
    assert!(
        !env.cmd(&pa)
            .args(["check", "--staged"])
            .output()
            .unwrap()
            .status
            .success(),
        "staged dangling open_decisions pointer must fail"
    );
    assert!(
        !env.cmd(&pa)
            .args(["check"])
            .output()
            .unwrap()
            .status
            .success(),
        "plain check must also refuse staged dangling open_decisions pointer"
    );
    assert!(
        git(&["reset", "-q", "HEAD", "--", "pa/tasks/pa-00000c.md"])
            .status
            .success()
    );
    std::fs::remove_file(pa.join("tasks/pa-00000c.md")).unwrap();

    // 3c. Stage a rename whose destination is a future record (`git mv` of untouched f3) -> failure
    assert!(
        git(&["mv", "pa/tasks/pa-000009.md", "pa/tasks/pa-00000d.md"])
            .status
            .success()
    );
    // Update the id inside the staged destination to match the new filename, still format: 3.
    let moved_f3 = f3_bytes.replace("pa-000009", "pa-00000d");
    std::fs::write(pa.join("tasks/pa-00000d.md"), &moved_f3).unwrap();
    assert!(git(&["add", "pa/tasks/pa-00000d.md"]).status.success());
    write_f1(&pa, "pa-00000d", "todo", "", ""); // working tree looks valid
    for args in [vec!["check", "--staged"], vec!["check"]] {
        let out = env.cmd(&pa).args(&args).output().unwrap();
        assert!(
            !out.status.success(),
            "staged rename destination to future record must fail for {args:?}"
        );
    }
    assert!(
        git(&[
            "reset",
            "-q",
            "HEAD",
            "--",
            "pa/tasks/pa-000009.md",
            "pa/tasks/pa-00000d.md"
        ])
        .status
        .success()
    );
    assert!(
        git(&["checkout", "-q", "--", "pa/tasks/pa-000009.md"])
            .status
            .success()
    );
    let _ = std::fs::remove_file(pa.join("tasks/pa-00000d.md"));

    // 4. Stage a future file in UNRELATED project pb -> checking pa ignores pb (outside scope)!
    write_f3(&pb, "pb-000009");
    assert!(git(&["add", "pb/tasks/pb-000009.md"]).status.success());
    let out_pa = env.cmd(&pa).args(["check"]).output().unwrap();
    assert!(
        out_pa.status.success(),
        "unrelated project pb's index change must be outside pa scope: {}",
        String::from_utf8_lossy(&out_pa.stdout)
    );
    let out_pa_staged = env.cmd(&pa).args(["check", "--staged"]).output().unwrap();
    assert!(
        out_pa_staged.status.success(),
        "staged check on pa must ignore unrelated pb: {}",
        String::from_utf8_lossy(&out_pa_staged.stdout)
    );

    // Untouched future file bytes never changed.
    assert_eq!(
        std::fs::read_to_string(pa.join("tasks/pa-000009.md")).unwrap(),
        f3_bytes
    );
}

#[test]
fn unborn_head_and_linked_checkout_and_editor_concurrent_edit_guards() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let root = base.path().canonicalize().unwrap();

    // 1. Unborn HEAD: staging a future file must fail on both `check` and `check --staged`.
    let unborn = root.join("unborn");
    init_git(env.home.path(), &unborn);
    std::fs::create_dir_all(unborn.join("tasks")).unwrap();
    std::fs::write(
        unborn.join("tasks/.config.toml"),
        "prefix = \"ub\"\nstage = \"experimental\"\n",
    )
    .unwrap();
    write_f3(&unborn, "ub-000009");
    let out = git_fixture::command(env.home.path(), &unborn)
        .args(["add", "-A"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    write_f1(&unborn, "ub-000009", "todo", "", ""); // working copy valid, index has f3!
    for args in [vec!["check", "--staged"], vec!["check"]] {
        let res = env.cmd(&unborn).args(&args).output().unwrap();
        assert!(
            !res.status.success(),
            "unborn HEAD {args:?} must refuse staged f3"
        );
        let json: Value = serde_json::from_slice(&res.stdout).unwrap();
        assert!(
            json["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "unsupported_record_version"),
            "unborn HEAD {args:?}: {json}"
        );
    }

    // 2. Linked checkout route & registered --project route with mixed-version records.
    let foreign_code = root.join("foreign-code");
    init_git(env.home.path(), &foreign_code);
    let overlay = root.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    let init_lnk = env
        .cmd(&overlay)
        .args(["init", "--prefix", "lk", "--checkout"])
        .arg(&foreign_code)
        .output()
        .unwrap();
    assert!(init_lnk.status.success(), "{init_lnk:?}");
    write_f1(&overlay, "lk-000001", "todo", "process: direct\n", "");
    write_f3(&overlay, "lk-000009");

    // From linked code checkout: list/ready succeed with warning, show future refuses.
    let listed_linked = env.json(&foreign_code, &["list"]);
    assert_eq!(listed_linked["tasks"].as_array().unwrap().len(), 1);
    assert!(
        listed_linked["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("unsupported_record_version")),
        "{listed_linked}"
    );
    let err_linked = err_json(&env, &foreign_code, &["show", "lk-000009"]);
    assert_eq!(err_linked["error"]["kind"], "unsupported_record_version");

    // Via --project lk from outside any project: same behavior.
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let listed_reg = env.json(&outside, &["list", "--project", "lk"]);
    assert_eq!(listed_reg["tasks"].as_array().unwrap().len(), 1);
    assert!(
        listed_reg["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("unsupported_record_version")),
        "{listed_reg}"
    );

    // 3. Editor concurrent modification on a format-2 record is refused under lock.
    std::fs::create_dir_all(overlay.join("docs/notes")).unwrap();
    std::fs::write(overlay.join("docs/notes/b.md"), "brief\n").unwrap();
    write_f2_sample(&overlay, "lk-000002");
    let rec_path = overlay.join("tasks/lk-000002.md");
    let mut cmd = env.cmd(&overlay);
    cmd.env(
        "EDITOR",
        format!(
            "python3 -c 'import sys; p=sys.argv[1]; r={:?}; rt=open(r).read(); open(r,\"w\").write(rt.replace(\"Format two task\",\"Disk race\")); pt=open(p).read(); open(p,\"w\").write(pt.replace(\"Format two task\",\"Editor edit\"))'",
            rec_path.to_str().unwrap()
        ),
    );
    let race = cmd
        .args(["edit", "lk-000002", "--editor"])
        .output()
        .unwrap();
    assert!(!race.status.success(), "concurrent edit must fail");
    let race_err: Value = serde_json::from_slice(&race.stderr).unwrap();
    assert_eq!(
        race_err["error"]["kind"], "concurrent_modification",
        "{race_err}"
    );

    // 4. Same-host concurrent updates on a format-2 record preserve both updates and all entries.
    let bin = assert_cmd::cargo::cargo_bin("tasks");
    let home_path = env.home.path().to_path_buf();
    let ov_a = overlay.clone();
    let ov_b = overlay.clone();
    let bin_a = bin.clone();
    let bin_b = bin.clone();
    let h_a = home_path.clone();
    let h_b = home_path.clone();
    let t1 = std::thread::spawn(move || {
        std::process::Command::new(bin_a)
            .current_dir(&ov_a)
            .env("HOME", &h_a)
            .env("XDG_CONFIG_HOME", h_a.join(".config"))
            .env("XDG_STATE_HOME", h_a.join(".local/state"))
            .env("TASKS_ACTOR", "worker-a")
            .args(["note", "lk-000002", "concurrent note A"])
            .output()
            .unwrap()
    });
    let t2 = std::thread::spawn(move || {
        std::process::Command::new(bin_b)
            .current_dir(&ov_b)
            .env("HOME", &h_b)
            .env("XDG_CONFIG_HOME", h_b.join(".config"))
            .env("XDG_STATE_HOME", h_b.join(".local/state"))
            .env("TASKS_ACTOR", "worker-b")
            .args(["note", "lk-000002", "concurrent note B"])
            .output()
            .unwrap()
    });
    assert!(t1.join().unwrap().status.success());
    assert!(t2.join().unwrap().status.success());
    let after_conc = std::fs::read_to_string(overlay.join("tasks/lk-000002.md")).unwrap();
    assert!(after_conc.contains("concurrent note A"), "{after_conc}");
    assert!(after_conc.contains("concurrent note B"), "{after_conc}");
    assert!(
        after_conc.contains(&format!("open_decisions: [{D1}]")),
        "{after_conc}"
    );
    assert!(after_conc.contains(D1) && after_conc.contains(D2) && after_conc.contains(R1));

    // 5. Sync-conflict sibling file is surfaced as an error rather than silently overwritten or lost.
    let conflict_path = overlay.join("tasks/lk-000002 (host conflicted copy 2026-11-24).md");
    std::fs::write(&conflict_path, &after_conc).unwrap();
    let chk = env.cmd(&overlay).args(["check"]).output().unwrap();
    assert!(
        !chk.status.success(),
        "sync-conflict sibling copy must be reported by check so both copies are preserved for recovery"
    );
    assert!(conflict_path.exists(), "conflict copy must stay untouched");
    std::fs::remove_file(&conflict_path).unwrap();
}

#[test]
fn mixed_version_allows_projects_show_next_start_edit_parked_and_marks_blockers_incomplete() {
    let mut env = TestEnv::new();
    let dir = env.init("mv");
    let other = env.init("ot");

    // Independent task that also links to a local format-3 record via `related`.
    write_f1(
        &dir,
        "mv-000001",
        "todo",
        "process: direct\nrelated: [mv-000009]\n",
        "",
    );
    // Task that depends on a local format-3 record.
    write_f1(
        &dir,
        "mv-000002",
        "todo",
        "process: direct\ndepends: [mv-000009]\n",
        "",
    );
    // Task to test `edit --status doing` alongside a format-3 sibling.
    write_f1(&dir, "mv-000003", "todo", "process: direct\n", "");
    write_f3(&dir, "mv-000009");
    write_f3(&other, "ot-000009");

    // 1. `projects` succeeds across reachable projects with format-3 siblings and warns.
    let projs = env.json(&dir, &["projects"]);
    assert_eq!(projs["projects"].as_array().unwrap().len(), 2, "{projs}");
    assert!(
        projs["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("mv-000009")
            && w.as_str().unwrap().contains("unsupported_record_version")),
        "projects must warn on future record: {projs}"
    );

    // 2. `show` on a supported task referencing a format-3 dependency succeeds with resolved=false.
    let shown_dep = env.json(&dir, &["show", "mv-000002"]);
    assert_eq!(shown_dep["depends_on"][0]["id"], "mv-000009");
    assert_eq!(shown_dep["depends_on"][0]["resolved"], false);

    // 3. `next` succeeds when the picked task references a format-3 record via `related`.
    let next = env.json(&dir, &["next"]);
    assert_eq!(next["next"]["task"]["id"], "mv-000001", "{next}");
    assert_eq!(next["next"]["related_to"][0]["resolved"], false);

    // 4. `blockers` succeeds beside a format-3 sibling and marks coverage incomplete.
    let blockers = env.json(&dir, &["blockers"]);
    assert_eq!(
        blockers["coverage"]["complete"], false,
        "blockers must mark coverage incomplete when future records are skipped: {blockers}"
    );
    assert!(
        blockers["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("mv-000009")),
        "{blockers}"
    );

    // 5. `start` and `edit --status doing` succeed on unrelated supported tasks beside format-3.
    let started = env.json(&dir, &["start", "mv-000001"]);
    assert_eq!(started["id"], "mv-000001", "{started}");
    let edited = env.json(&dir, &["edit", "mv-000003", "--status", "doing"]);
    assert_eq!(edited["id"], "mv-000003", "{edited}");

    // 6. Parked worktree with a format-3 sibling still opens supported parked task cleanly.
    let wt = env.home.path().join("mv-wt");
    std::fs::create_dir_all(wt.join("tasks")).unwrap();
    std::fs::copy(
        dir.join("tasks/.config.toml"),
        wt.join("tasks/.config.toml"),
    )
    .unwrap();
    write_f1(&wt, "mv-000004", "doing", "process: direct\n", "");
    write_f3(&wt, "mv-000008");
    let parked = env.json(&wt, &["park", "mv-000004", "continue later"]);
    assert_eq!(parked["id"], "mv-000004", "{parked}");
    let show_elsewhere = env.json(&dir, &["show", "mv-000004"]);
    assert_eq!(
        show_elsewhere["task"]["id"], "mv-000004",
        "{show_elsewhere}"
    );
    let list_parked = env.json(&dir, &["list", "--parked"]);
    assert!(
        list_parked["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "mv-000004"),
        "{list_parked}"
    );
}
