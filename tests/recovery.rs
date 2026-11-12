mod common;

use common::{TestEnv, as_agent, git, id_of, repo_with_worktree};
use serde_json::Value;
use std::path::Path;

fn missing(env: &TestEnv, root: &Path, args: &[&str]) -> String {
    let out = env.cmd(root).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "task_not_found", "{value}");
    assert_eq!(value["error"].as_object().unwrap().len(), 2);
    value["error"]["detail"].as_str().unwrap().to_owned()
}

fn write_missing(env: &TestEnv, root: &Path, args: &[&str]) -> String {
    let out = as_agent(env, root, "caller").args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    assert_eq!(common::err_kind(&out), "task_not_found", "{out:?}");
    common::err_detail(&out)
}

#[test]
fn missing_recovery_writes_unique_and_main_handoff() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Only in side"]));
    for args in [
        vec!["note", &id, "it's literal"],
        vec!["edit", &id, "--title", "New title"],
        vec!["start", &id],
        vec!["edit", &id, "--body", "-"],
    ] {
        let before = std::fs::read(side.join(format!("tasks/{id}.md"))).unwrap();
        let detail = write_missing(&env, &main, &args);
        assert!(detail.contains("Run it there:"), "{detail}");
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert_eq!(
            std::fs::read(side.join(format!("tasks/{id}.md"))).unwrap(),
            before
        );
    }
    let detail = write_missing(&env, &main, &["note", &id, "it's literal"]);
    assert!(detail.contains("'it'\\''s literal'"), "{detail}");
    let id = id_of(env.json(&main, &["add", "Only in main"]));
    for command in ["note", "start"] {
        let args = if command == "note" {
            vec![command, &id, "hello"]
        } else {
            vec![command, &id]
        };
        let detail = write_missing(&env, &side, &args);
        assert!(
            detail.contains("Commit") && detail.contains("merge it into this branch"),
            "{detail}"
        );
        assert!(detail.find("Commit").unwrap() < detail.rfind(&format!("{command} {id}")).unwrap());
    }
}

#[test]
fn missing_recovery_writes_occupied_parent_note_and_main() {
    for occupied_main in [false, true] {
        let mut env = TestEnv::new();
        let (main, side, parent) = repo_with_worktree(&mut env);
        let (here, there) = if occupied_main {
            (&side, &main)
        } else {
            (&main, &side)
        };
        as_agent(&env, there, "holder")
            .args(["start", &parent])
            .assert()
            .success();
        let id = id_of(env.json(there, &["add", "Unclaimed child", "--parent", &parent]));
        let store = std::fs::read(env.claim_store("sci")).unwrap();
        let before = std::fs::read(there.join(format!("tasks/{id}.md"))).unwrap();
        let note = write_missing(&env, here, &["note", &id, "safe append"]);
        assert!(
            note.contains("holder") && note.contains(&format!("note {id}")),
            "{note}"
        );
        assert!(!note.contains("Wait for that branch to merge"), "{note}");
        let edit = write_missing(&env, here, &["edit", &id, "--title", "Not yet"]);
        assert!(
            edit.contains("holder") && edit.contains(&format!("show {id}")),
            "{edit}"
        );
        assert!(!edit.contains(&format!("edit {id}")), "{edit}");
        assert_eq!(
            std::fs::read(there.join(format!("tasks/{id}.md"))).unwrap(),
            before
        );
        assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), store);
        as_agent(&env, there, "caller")
            .args(["note", &id, "safe append"])
            .assert()
            .success();
        assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), store);
        let shown = env.json(there, &["show", &id]);
        assert_eq!(
            shown["task"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["text"] == "safe append")
                .count(),
            1
        );
    }
}

#[test]
fn missing_recovery_writes_shorthand_and_ambiguous_inspection_only() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Only there"]));
    let other = env.init("oth");
    let outside = tempfile::tempdir().unwrap();
    for here in [&other, outside.path()] {
        let detail = write_missing(&env, here, &["edit", &id, "--related", "abcdef"]);
        assert!(detail.contains(&format!("show {id}")), "{detail}");
        assert!(!detail.contains(&format!("edit {id}")), "{detail}");
        assert!(detail.contains("shorthand"), "{detail}");
    }
    let suffix = id.strip_prefix("sci-").unwrap();
    assert!(
        write_missing(&env, &main, &["note", suffix, "abcdef is text"]).contains("Run it there:")
    );
    let second = main.join("second");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "second",
            second.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &second);
    std::fs::copy(
        side.join(format!("tasks/{id}.md")),
        second.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    let detail = write_missing(&env, &main, &["note", &id, "no selection"]);
    assert!(detail.contains("ambiguous"), "{detail}");
    assert!(!detail.contains(&format!("note {id}")), "{detail}");
}

fn linked_fixture(
    env: &mut TestEnv,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
) {
    let (main, side, parent) = repo_with_worktree(env);
    let code = env.home.path().join("foreign-code");
    std::fs::create_dir(&code).unwrap();
    git(&code, &["init", "-q", "-b", "main"]);
    common::git_fixture::assert_repo(env.home.path(), &code);
    std::fs::write(code.join("README"), "code").unwrap();
    git(&code, &["add", "README"]);
    git(&code, &["commit", "-qm", "seed"]);
    let code_side = code.join("code-side");
    git(
        &code,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "code-side",
            code_side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &code, &code_side);
    for root in [&main, &side] {
        let path = root.join("tasks/.config.toml");
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            format!(
                "{raw}\n[checkout]\npath = {:?}\n",
                code.display().to_string()
            ),
        )
        .unwrap();
    }
    (main, side, code, code_side, parent)
}

#[test]
fn missing_recovery_writes_linked_code_claim_maps_to_registered_overlay() {
    for code_worktree in [false, true] {
        for unverifiable in [false, true] {
            let mut env = TestEnv::new();
            let (main, side, code, code_side, parent) = linked_fixture(&mut env);
            let resume = if code_worktree { &code_side } else { &code };
            as_agent(&env, resume, "holder")
                .args(["start", &parent])
                .assert()
                .success();
            if unverifiable {
                common::set_pid_view(&env, "sci", &parent, "0:0");
            }
            let main_id = id_of(env.json(&main, &["add", "Registered overlay record"]));
            let side_id = id_of(env.json(&side, &["add", "Other overlay branch record"]));
            std::fs::write(
                main.join("tasks/sci-badbad.md"),
                "unrelated malformed history",
            )
            .unwrap();
            let detail = write_missing(&env, &side, &["edit", &main_id, "--title", "No"]);
            assert!(
                detail.contains("holder") && detail.contains(resume.to_str().unwrap()),
                "{detail}"
            );
            assert!(!detail.contains(&format!("edit {main_id}")), "{detail}");
            let detail = write_missing(&env, &side, &["note", &main_id, "append"]);
            assert!(detail.contains(&format!("note {main_id}")), "{detail}");
            let detail = write_missing(&env, &main, &["edit", &side_id, "--title", "Elsewhere"]);
            assert!(
                detail.contains("Run it there:") && detail.contains(&format!("edit {side_id}")),
                "{detail}"
            );
        }
    }
}

#[test]
fn missing_recovery_writes_linked_unknown_mapping_withholds_notes() {
    for broken in ["gone", "git", "not-directory", "conflict", "budget"] {
        let mut env = TestEnv::new();
        let (main, side, code, code_side, parent) = linked_fixture(&mut env);
        as_agent(&env, &code_side, "holder")
            .args(["start", &parent])
            .assert()
            .success();
        let id = id_of(env.json(&main, &["add", "Only on registered overlay"]));
        match broken {
            "gone" => std::fs::remove_dir_all(&code_side).unwrap(),
            "not-directory" => {
                std::fs::remove_dir_all(&code_side).unwrap();
                std::fs::write(&code_side, "not a directory").unwrap();
            }
            "git" => std::fs::write(
                code_side.join(".git"),
                format!("gitdir: {}\n", code_side.join("missing-gitdir").display()),
            )
            .unwrap(),
            "conflict" => {
                let root = env.init("oth");
                let config = root.join("tasks/.config.toml");
                let raw = std::fs::read_to_string(&config).unwrap();
                std::fs::write(
                    config,
                    format!(
                        "{raw}\n[checkout]\npath = {:?}\n",
                        code.display().to_string()
                    ),
                )
                .unwrap();
            }
            "budget" => {
                let path = env.home.path().join(".config/tasks/projects.toml");
                let mut registry: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                for n in 0..65 {
                    let prefix = format!("p{n:02}");
                    let root = env.home.path().join(&prefix);
                    std::fs::create_dir_all(root.join("tasks")).unwrap();
                    std::fs::write(
                        root.join("tasks/.config.toml"),
                        format!("prefix = {prefix:?}\n"),
                    )
                    .unwrap();
                    registry["projects"]
                        .as_table_mut()
                        .unwrap()
                        .insert(prefix, toml::Value::String(root.display().to_string()));
                }
                std::fs::write(path, toml::to_string(&registry).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        let detail = write_missing(&env, &side, &["note", &id, "no retry"]);
        assert!(
            detail.contains("occupancy") && detail.contains("unknown"),
            "{broken}: {detail}"
        );
        assert!(
            !detail.contains(&format!("note {id}")),
            "{broken}: {detail}"
        );
        assert!(detail.contains(&format!("show {id}")), "{broken}: {detail}");
    }
}

#[test]
fn missing_recovery_writes_unknown_occupancy_limits_are_explicit() {
    let mut env = TestEnv::new();
    let (main, side, code, _, parent) = linked_fixture(&mut env);
    as_agent(&env, &code, "holder")
        .args(["start", &parent])
        .assert()
        .success();
    let id = id_of(env.json(&main, &["add", "Unknown occupants"]));
    let path = env.claim_store("sci");
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let template = store["claims"][&parent].clone();
    store["claims"].as_table_mut().unwrap().clear();
    for n in 1..=9 {
        let mut claim = template.clone();
        claim["worktree"] = toml::Value::String(
            env.home
                .path()
                .join(format!("missing-code-{n}"))
                .display()
                .to_string(),
        );
        store["claims"]
            .as_table_mut()
            .unwrap()
            .insert(format!("sci-{n:06x}"), claim);
    }
    std::fs::write(path, toml::to_string(&store).unwrap()).unwrap();
    let detail = write_missing(&env, &side, &["note", &id, "no retry"]);
    assert!(
        detail.contains("1 further unresolved occupancy location omitted"),
        "{detail}"
    );
    assert!(!detail.contains(&format!("note {id}")), "{detail}");
}

#[test]
fn missing_recovery_writes_own_stale_and_parked_are_not_foreign_occupants() {
    for mode in ["own", "stale", "park"] {
        let mut env = TestEnv::new();
        let (main, side, parent) = repo_with_worktree(&mut env);
        let holder = if mode == "own" { "caller" } else { "holder" };
        as_agent(&env, &side, holder)
            .args(["start", &parent])
            .assert()
            .success();
        let id = id_of(env.json(&side, &["add", "Unclaimed record"]));
        if mode == "park" {
            as_agent(&env, &side, holder)
                .args(["park", &parent, "later"])
                .assert()
                .success();
        } else if mode == "stale" {
            let path = env.claim_store("sci");
            let mut store: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            store["claims"][&parent]["boot_id"] = toml::Value::String("different-boot".into());
            std::fs::write(path, toml::to_string(&store).unwrap()).unwrap();
        }
        let detail = write_missing(&env, &main, &["edit", &id, "--title", "Allowed retry"]);
        assert!(detail.contains("Run it there:"), "{mode}: {detail}");
    }
}

#[test]
fn missing_recovery_writes_unknown_identity_is_inspection_only() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Unknown caller"]));
    let out = env
        .cmd(&main)
        .env("CODEX_THREAD_ID", "invalid\nthread")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "task_not_found");
    let detail = common::err_detail(&out);
    assert!(detail.contains("occupancy unknown"), "{detail}");
    assert!(!detail.contains(&format!("note {id}")), "{detail}");
    assert!(detail.contains(&format!("show {id}")), "{detail}");
}

#[test]
fn missing_recovery_writes_known_occupants_remain_visible_with_uncertainty() {
    let mut env = TestEnv::new();
    let (main, side, code, _, parent) = linked_fixture(&mut env);
    as_agent(&env, &code, "known-holder")
        .args(["start", &parent])
        .assert()
        .success();
    let id = id_of(env.json(&main, &["add", "Known and unknown"]));
    let path = env.claim_store("sci");
    let mut store: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut unknown = store["claims"][&parent].clone();
    unknown["session"] = toml::Value::String("unknown-holder".into());
    unknown["worktree"] =
        toml::Value::String(env.home.path().join("missing-code").display().to_string());
    store["claims"]
        .as_table_mut()
        .unwrap()
        .insert("sci-dead00".into(), unknown);
    std::fs::write(path, toml::to_string(&store).unwrap()).unwrap();
    let detail = write_missing(&env, &side, &["note", &id, "no retry"]);
    assert!(
        detail.contains("occupancy unknown") && detail.contains("known-holder"),
        "{detail}"
    );
    assert!(!detail.contains(&format!("note {id}")), "{detail}");
}

#[test]
fn missing_recovery_writes_direct_overlay_claims_occupy_their_own_home() {
    let mut env = TestEnv::new();
    let (main, side, _, _, parent) = linked_fixture(&mut env);
    as_agent(&env, &side, "holder")
        .args(["start", &parent])
        .assert()
        .success();
    let main_id = id_of(env.json(&main, &["add", "Unoccupied registered home"]));
    let side_id = id_of(env.json(&side, &["add", "Occupied overlay branch"]));
    let detail = write_missing(&env, &side, &["edit", &main_id, "--title", "main"]);
    assert!(
        detail.contains("Commit") && detail.contains(&format!("edit {main_id}")),
        "{detail}"
    );
    let detail = write_missing(&env, &main, &["edit", &side_id, "--title", "branch"]);
    assert!(
        detail.contains("holder") && !detail.contains(&format!("edit {side_id}")),
        "{detail}"
    );
}

#[test]
fn missing_recovery_writes_different_overlay_binding_still_has_a_known_home() {
    let mut env = TestEnv::new();
    let (main, side, code, _, parent) = linked_fixture(&mut env);
    let third = main.join("third-overlay");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "third-overlay",
            third.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &third);
    let config = third.join("tasks/.config.toml");
    let raw = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        config,
        format!(
            "{raw}\n[checkout]\npath = {:?}\n",
            code.display().to_string()
        ),
    )
    .unwrap();
    let alternate_code = env.home.path().join("alternate-code");
    std::fs::create_dir(&alternate_code).unwrap();
    let config = side.join("tasks/.config.toml");
    let raw = std::fs::read_to_string(&config).unwrap().replace(
        &code.display().to_string(),
        &alternate_code.display().to_string(),
    );
    std::fs::write(config, raw).unwrap();
    as_agent(&env, &side, "holder")
        .args(["start", &parent])
        .assert()
        .success();
    let id = id_of(env.json(&third, &["add", "Known unrelated home"]));
    let detail = write_missing(&env, &main, &["edit", &id, "--title", "retry"]);
    assert!(detail.contains("Run it there:"), "{detail}");
    assert!(!detail.contains("occupancy unknown"), "{detail}");
}

#[test]
fn missing_recovery_writes_guard_editor_and_body_stdin() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Inputs remain unread"]));
    let sentinel = env.home.path().join("editor-called");
    let out = as_agent(&env, &main, "caller")
        .env("EDITOR", format!("touch {}", sentinel.display()))
        .args(["edit", &id, "--editor"])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "task_not_found");
    assert!(!sentinel.exists());
    let mut child = env
        .raw(&main)
        .env("TASKS_SESSION", "caller")
        .args(["edit", &id, "--body", "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let pipe = child.stdin.take().unwrap();
    let out = common::reap(child, common::REAP).expect("refusal must not wait for stdin EOF");
    assert_eq!(common::err_kind(&out), "task_not_found");
    assert!(common::err_detail(&out).contains("supply the same input on stdin"));
    drop(pipe);
}

#[test]
fn missing_recovery_writes_feedback_stays_on_registered_records() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(
        &side,
        &[
            "add",
            "Feedback only in branch",
            "--status",
            "idea",
            "--tag",
            "feedback",
            "--tag",
            "gap",
            "--tag",
            "from:sci",
        ],
    ));
    let file = side.join(format!("tasks/{id}.md"));
    let before = std::fs::read(&file).unwrap();
    let out = env
        .cmd(&main)
        .args([
            "feedback",
            "--project",
            "sci",
            "Feedback only in branch",
            "--category",
            "gap",
            "--recur",
            &id,
        ])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "validation");
    assert!(!common::err_detail(&out).contains(side.to_str().unwrap()));
    assert_eq!(std::fs::read(file).unwrap(), before);
    assert!(!main.join(format!("tasks/{id}.md")).exists());
}

#[test]
fn missing_recovery_reads_both_directions() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let branch_id = id_of(env.json(&side, &["add", "Branch only"]));
    let main_id = id_of(env.json(&main, &["add", "Main only"]));
    for (here, there, id) in [(&main, &side, branch_id), (&side, &main, main_id)] {
        let path = there.join(format!("tasks/{id}.md"));
        let before = std::fs::read(&path).unwrap();
        let detail = missing(&env, here, &["show", &id]);
        assert!(detail.contains(there.to_str().unwrap()), "{detail}");
        assert!(detail.contains(&format!("show {id}")), "{detail}");
        assert!(detail.contains("observed"), "{detail}");
        assert!(!here.join(format!("tasks/{id}.md")).exists());
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert_eq!(env.json(there, &["show", &id])["task"]["id"], id);
    }
}

#[test]
fn missing_recovery_zero_and_ambiguous() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let detail = missing(&env, &main, &["show", "sci-000000"]);
    assert!(detail.contains("no matching record file"), "{detail}");
    let other = main.join("another");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "another",
            other.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &other);
    let id = id_of(env.json(&side, &["add", "Same bytes in two branches"]));
    std::fs::copy(
        side.join(format!("tasks/{id}.md")),
        other.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("ambiguous"), "{detail}");
    for path in [&side, &other] {
        assert!(detail.contains(path.to_str().unwrap()), "{detail}");
    }
    assert_eq!(detail.matches(&format!("show {id}")).count(), 2, "{detail}");
}

#[test]
fn missing_recovery_doc_never_fetches() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Document remains unselected"]));
    let record = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&record).unwrap();
    std::fs::write(
        &record,
        raw.replacen(
            "---\n",
            "---\nspec: \"probe:spec\"\nplan: \"probe:plan\"\n",
            1,
        ),
    )
    .unwrap();
    let sentinel = env.home.path().join("provider-called");
    let command = format!("touch {}; printf document", sentinel.display());
    std::fs::write(
        env.home.path().join(".config/tasks/config.toml"),
        format!("[docs.schemes]\nprobe = {command:?}\n"),
    )
    .unwrap();
    for target in ["--spec", "--plan", "remote"] {
        let detail = missing(&env, &main, &["doc", &id, target]);
        assert!(detail.contains(&format!("show {id}")), "{detail}");
        assert!(!sentinel.exists());
    }
}

#[test]
fn missing_recovery_physical_archive_only() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Physical record with missing history"]));
    let file = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        file,
        raw.replacen(
            "---\n",
            &format!("---\nnote_archives: [\"v1:{}:1\"]\n", "a".repeat(64)),
            1,
        ),
    )
    .unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains(side.to_str().unwrap()), "{detail}");
    assert!(detail.contains("observed"), "{detail}");
    let out = env.cmd(&side).args(["show", &id]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_ne!(common::err_kind(&out), "task_not_found");
}

#[test]
fn missing_recovery_incomplete_and_malformed() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Broken file"]));
    let file = side.join(format!("tasks/{id}.md"));
    let raw = std::fs::read(&file).unwrap();
    for bytes in [
        b"not a task".to_vec(),
        String::from_utf8(raw.clone())
            .unwrap()
            .replace(&id, "sci-000000")
            .into_bytes(),
    ] {
        std::fs::write(&file, bytes).unwrap();
        let detail = missing(&env, &main, &["show", &id]);
        assert!(detail.contains("incomplete"), "{detail}");
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(!detail.contains("Run it there:"), "{detail}");
    }
    std::fs::write(&file, &raw).unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(main.join("tasks/.config.toml"), &file).unwrap();
    assert!(missing(&env, &main, &["show", &id]).contains("incomplete"));
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir_all(&side).unwrap();
    assert!(missing(&env, &main, &["show", &id]).contains("incomplete"));
}

#[test]
fn missing_recovery_local_and_recorded_precedence() {
    let mut env = TestEnv::new();
    let (main, side, shared) = repo_with_worktree(&mut env);
    as_agent(&env, &side, "other")
        .args(["start", &shared])
        .assert()
        .success();
    assert_eq!(
        env.json(&main, &["show", &shared])["task"]["status"],
        "todo"
    );
    let id = id_of(env.json(&side, &["add", "Claimed task"]));
    as_agent(&env, &side, "other")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(env.json(&main, &["show", &id])["task"]["id"], id);
    std::fs::remove_file(side.join(format!("tasks/{id}.md"))).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("claimed in") && detail.contains("unavailable"),
        "{detail}"
    );
    assert!(!detail.contains("observed"), "{detail}");
    std::fs::write(main.join(format!("tasks/{shared}.md")), "malformed").unwrap();
    let out = env.cmd(&main).args(["show", &shared]).output().unwrap();
    assert_ne!(common::err_kind(&out), "task_not_found");
}

#[test]
fn missing_recovery_linked_overlay_and_foreign_route() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let code = tempfile::tempdir().unwrap();
    for root in [&main, &side] {
        let path = root.join("tasks/.config.toml");
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            format!(
                "{raw}\n[checkout]\npath = {:?}\n",
                code.path().display().to_string()
            ),
        )
        .unwrap();
    }
    let id = id_of(env.json(&side, &["add", "Linked overlay only"]));
    let foreign = env.init("oth");
    for here in [&main, &foreign] {
        let detail = missing(&env, here, &["show", &id]);
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(!detail.contains(code.path().to_str().unwrap()), "{detail}");
    }
}

#[test]
fn missing_recovery_non_utf8_and_quoted_paths() {
    use std::os::unix::ffi::OsStringExt;
    let mut env = TestEnv::new();
    let (main, _, _) = repo_with_worktree(&mut env);
    let side = main.join(std::ffi::OsString::from_vec(b"bad-\xff".to_vec()));
    let out = common::git_fixture::command(env.home.path(), &main)
        .args(["worktree", "add", "-q", "-b", "bytes"])
        .arg(&side)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let id = id_of(env.json(&side, &["add", "Byte path"]));
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("incomplete"), "{detail}");
    assert!(!detail.contains(&format!("show {id}")), "{detail}");
}

#[test]
fn missing_recovery_nested_project_offset() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let main = base.path().canonicalize().unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    common::git_fixture::assert_repo(env.home.path(), &main);
    let project = main.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    env.json(&project, &["init", "--prefix", "sci"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("side");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let nested = side.join("nested/project");
    let id = id_of(env.json(&nested, &["add", "Nested only"]));
    let detail = missing(&env, &project, &["show", &id]);
    assert!(detail.contains(nested.to_str().unwrap()), "{detail}");
}

#[test]
fn missing_recovery_detached_never_resurrects_code_records() {
    let mut env = TestEnv::new();
    let (main, _historical, id) = repo_with_worktree(&mut env);
    git(&main, &["rm", "-r", "-q", "tasks"]);
    let records = tempfile::tempdir().unwrap();
    env.json(
        &main,
        &[
            "init",
            "--prefix",
            "sci",
            "--records",
            records.path().to_str().unwrap(),
        ],
    );
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("shared records home"), "{detail}");
    assert!(!detail.contains(&format!("show {id}")), "{detail}");
}

#[test]
fn missing_recovery_no_git_is_explicit() {
    let mut env = TestEnv::new();
    let root = env.init("sci");
    let bin = tempfile::tempdir().unwrap();
    let out = env
        .cmd(&root)
        .env("PATH", bin.path())
        .args(["show", "sci-000000"])
        .output()
        .unwrap();
    assert_eq!(common::err_kind(&out), "task_not_found");
    assert!(common::err_detail(&out).contains("search unavailable"));
}

#[test]
fn missing_recovery_excludes_wrong_layout_and_bounds_inputs() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Wrong project"]));
    let config = side.join("tasks/.config.toml");
    let original = std::fs::read_to_string(&config).unwrap();
    for wrong in [
        original.replace("\"sci\"", "\"oth\""),
        format!(
            "{original}\n[checkout]\nkind = \"detached\"\npath = {:?}\n",
            main.display().to_string()
        ),
    ] {
        std::fs::write(&config, wrong).unwrap();
        assert!(missing(&env, &main, &["show", &id]).contains("no matching record file"));
    }
    std::fs::write(&config, "x".repeat(65_537)).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("incomplete") && detail.contains("allowance"),
        "{detail}"
    );
    std::fs::write(&config, original).unwrap();
    let file = side.join(format!("tasks/{id}.md"));
    std::fs::write(&file, "PRIVATE_RECORD_CONTENT".repeat(70_000)).unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(
        detail.contains("incomplete") && detail.contains("allowance"),
        "{detail}"
    );
    assert!(!detail.contains("PRIVATE_RECORD_CONTENT"));
    std::fs::write(&file, "PRIVATE_RECORD_CONTENT").unwrap();
    let detail = missing(&env, &main, &["show", &id]);
    assert!(detail.contains("invalid physical record"), "{detail}");
    assert!(!detail.contains("PRIVATE_RECORD_CONTENT"));
}

#[test]
fn missing_recovery_shell_special_retry_is_executable() {
    let mut env = TestEnv::new();
    let (main, _, _) = repo_with_worktree(&mut env);
    let side = main.join("quotes ' and spaces; literal");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "quoted",
            side.to_str().unwrap(),
        ],
    );
    common::git_fixture::assert_worktree(env.home.path(), &main, &side);
    let id = id_of(env.json(&side, &["add", "Quoted directory"]));
    let detail = missing(&env, &main, &["show", &id]);
    let command = &detail[detail.find("tasks -C ").expect("explicit retry")..];
    let command = command.split("; location hints only;").next().unwrap();
    let bin = env.home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(assert_cmd::cargo::cargo_bin("tasks"), bin.join("tasks")).unwrap();
    let out = std::process::Command::new("sh")
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .current_dir(&main)
        .args(["-c", command])
        .output()
        .unwrap();
    assert!(out.status.success(), "{command}: {out:?}");
    let shown: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(shown["task"]["id"], id);
}
