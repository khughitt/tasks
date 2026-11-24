//! `tasks check --staged` behind the records repository's pre-commit hook: what a commit
//! publishes decides the verdict, whatever the working tree holds, and an ordinary
//! appended note on an accepted tree skips the full audit.

mod common;
use common::{TestEnv, git_fixture};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

const PREFIX: &str = "st";

/// A records repository holding one detached project, with the candidate records hook
/// installed and the test binary first on the hook's PATH as `tasks`.
struct Fx {
    env: TestEnv,
    _base: tempfile::TempDir,
    records: PathBuf,
    project: PathBuf,
    trace: PathBuf,
}

fn repo(home: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let git = |args: &[&str]| {
        let out = git_fixture::command(home, dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    git_fixture::assert_repo(home, dir);
    std::fs::write(dir.join("README"), "x\n").unwrap();
    git(&["add", "README"]);
    git(&["commit", "-qm", "init"]);
}

impl Fx {
    fn new() -> Fx {
        Fx::with(true)
    }

    /// `commit_config`: commit the new project before the hook is installed, so the
    /// first hooked commit has a HEAD holding the project.
    fn with(commit_config: bool) -> Fx {
        let env = TestEnv::new();
        let base = tempfile::tempdir().unwrap();
        let root = base.path().canonicalize().unwrap();
        let records = root.join("records");
        let code = root.join("code");
        repo(env.home.path(), &records);
        repo(env.home.path(), &code);
        let out = env
            .cmd(&code)
            .args(["init", "--prefix", PREFIX, "--records"])
            .arg(&records)
            .output()
            .unwrap();
        assert!(out.status.success(), "init: {out:?}");
        let project = records.join(PREFIX);
        let fx = Fx {
            trace: root.join("trace2.json"),
            env,
            _base: base,
            records,
            project,
        };
        if commit_config {
            fx.git(&["add", "-A"]);
            fx.git(&["commit", "-qm", "project"]);
        }
        let hooks = root.join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("pre-commit");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/records-pre-commit"),
            &hook,
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(
            assert_cmd::cargo::cargo_bin("tasks"),
            fx.env.fake_bin().join("tasks"),
        )
        .unwrap();
        fx.git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
        fx
    }

    fn git_output(&self, args: &[&str]) -> Output {
        git_fixture::command(self.env.home.path(), &self.records)
            .env("GIT_TRACE2_EVENT", &self.trace)
            .args(args)
            .output()
            .unwrap()
    }

    fn git(&self, args: &[&str]) -> String {
        let out = self.git_output(args);
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    /// Commits the index through the hook; `Ok` when the hook accepted it.
    fn commit(&self, extra: &[&str]) -> Result<(), String> {
        let _ = std::fs::remove_file(&self.trace);
        let mut args = vec!["commit", "-qm", "staged"];
        args.extend_from_slice(extra);
        let out = self.git_output(&args);
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }

    /// Whether the last commit's hook ran the full staged audit, which materializes the
    /// staged tree with `git checkout-index`.
    fn full_audit_ran(&self) -> bool {
        std::fs::read_to_string(&self.trace)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .any(|event| {
                event["event"] == "start"
                    && event["argv"]
                        .as_array()
                        .is_some_and(|argv| argv.iter().any(|a| a == "checkout-index"))
            })
    }

    fn add(&self, title: &str, extra: &[&str]) -> String {
        let mut args = vec!["add", title];
        args.extend_from_slice(extra);
        common::id_of(self.env.json(&self.project, &args))
    }

    fn record(&self, id: &str) -> PathBuf {
        self.project.join(format!("tasks/{id}.md"))
    }

    fn rel(&self, id: &str) -> String {
        format!("{PREFIX}/tasks/{id}.md")
    }

    fn read(&self, id: &str) -> String {
        std::fs::read_to_string(self.record(id)).unwrap()
    }

    fn write(&self, id: &str, text: &str) {
        std::fs::write(self.record(id), text).unwrap();
    }

    /// Appends one plain note to `id`'s record by hand, as an editor or raw Git user would.
    fn append_note(&self, id: &str, text: &str) {
        let raw = self.read(id);
        self.write(id, &format!("{raw}- 2026-11-21T00:00:00Z (main): {text}\n"));
    }

    fn forget_accepted_trees(&self) {
        let dir = self
            .env
            .home
            .path()
            .join(".local/state/tasks/staged-checks");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn malformed_staged_bytes_are_refused_behind_a_valid_working_copy() {
    let fx = Fx::new();
    let id = fx.add("one", &[]);
    let valid = fx.read(&id);
    fx.write(&id, "not a task record\n");
    fx.git(&["add", &fx.rel(&id)]);
    fx.write(&id, &valid);
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("parse"), "{refused}");
}

#[test]
fn unstaged_breakage_outside_the_commit_does_not_block_it() {
    let fx = Fx::new();
    let a = fx.add("a", &[]);
    let b = fx.add("b", &[]);
    fx.write(&b, "not a task record\n");
    fx.append_note(&a, "valid staged change");
    fx.git(&["add", &fx.rel(&a)]);
    fx.commit(&[]).unwrap();
    assert_eq!(
        fx.git(&["status", "--porcelain"]).trim(),
        format!("M {}", fx.rel(&b))
    );
}

#[test]
fn a_malformed_staged_config_is_refused_behind_a_valid_working_copy() {
    let fx = Fx::new();
    let config = fx.project.join("tasks/.config.toml");
    let valid = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{valid}\nnot toml [[[\n")).unwrap();
    fx.git(&["add", &format!("{PREFIX}/tasks/.config.toml")]);
    std::fs::write(&config, &valid).unwrap();
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains(".config.toml"), "{refused}");
}

#[test]
fn a_staged_attachment_without_a_ledger_entry_is_refused_after_its_file_is_removed() {
    let fx = Fx::new();
    let id = fx.add("owner", &[]);
    let dir = fx.project.join(format!("tasks/files/{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("loose.txt"), "unledgered\n").unwrap();
    fx.git(&["add", &format!("{PREFIX}/tasks/files/{id}/loose.txt")]);
    std::fs::remove_dir_all(&dir).unwrap();
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("attachment_unnoted"), "{refused}");
}

#[test]
fn cross_record_rules_still_judge_the_staged_tree() {
    let fx = Fx::new();
    let parent = fx.add("parent", &[]);
    let _child = fx.add("child", &["--parent", &parent]);
    fx.git(&["rm", "-q", &fx.rel(&parent)]);
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("dangling_parent"), "{refused}");
    fx.git(&["reset", "-q", "--hard"]);

    let a = fx.add("a", &[]);
    let b = fx.add("b", &["--depends", &a]);
    let raw = fx.read(&a);
    fx.write(
        &a,
        &raw.replacen("depends: []", &format!("depends: [{b}]"), 1),
    );
    fx.git(&["add", &fx.rel(&a)]);
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("cycle"), "{refused}");
}

#[test]
fn a_pathspec_commit_is_checked_against_its_temporary_index() {
    let fx = Fx::new();
    let a = fx.add("a", &[]);
    let b = fx.add("b", &[]);
    // A malformed record staged but left out of the commit does not block it...
    fx.write(&b, "not a task record\n");
    fx.git(&["add", &fx.rel(&b)]);
    fx.append_note(&a, "committed by pathspec");
    fx.commit(&["--", &fx.rel(&a)]).unwrap();
    // ...and the commit holds only the pathspec.
    let changed = fx.git(&["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(changed.trim(), fx.rel(&a));
    // A malformed record named by the pathspec is refused although the index holds a
    // valid copy.
    fx.git(&["reset", "-q", "--hard"]);
    let raw = fx.read(&a);
    fx.append_note(&a, "valid in the index");
    fx.git(&["add", &fx.rel(&a)]);
    fx.write(&a, &format!("{raw}not a note line\n"));
    let refused = fx.commit(&["--", &fx.rel(&a)]).unwrap_err();
    assert!(refused.contains("parse"), "{refused}");
}

#[test]
fn an_ordinary_note_on_an_accepted_tree_skips_the_full_audit() {
    let fx = Fx::new();
    let id = fx.add("noted", &[]);
    // `add` committed through the hook with the full audit, which accepted the tree
    // HEAD now holds.
    fx.append_note(&id, "first plain note");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(!fx.full_audit_ran(), "an ordinary note took the full audit");
    fx.append_note(&id, "second plain note");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(
        !fx.full_audit_ran(),
        "a chained ordinary note took the full audit"
    );
}

#[test]
fn anything_but_an_ordinary_appended_note_takes_the_full_audit() {
    let fx = Fx::new();
    let a = fx.add("a", &[]);
    let b = fx.add("b", &[]);
    type Change<'a> = Box<dyn Fn() + 'a>;
    let cases: Vec<(&str, Change)> = vec![
        (
            "ledger note",
            Box::new(|| fx.append_note(&a, "detached: gone.txt: no longer needed")),
        ),
        (
            "retention note",
            Box::new(|| fx.append_note(&a, "retained: {}")),
        ),
        (
            "field change",
            Box::new(|| {
                let raw = fx.read(&a);
                fx.write(&a, &raw.replacen("priority: 2", "priority: 3", 1));
            }),
        ),
        (
            "two records",
            Box::new(|| {
                fx.append_note(&a, "one");
                fx.append_note(&b, "two");
            }),
        ),
        (
            "rewritten note",
            Box::new(|| {
                fx.append_note(&a, "kept");
                fx.git(&["add", "-A"]);
                fx.commit(&[]).unwrap();
                let raw = fx.read(&a);
                fx.write(&a, &raw.replace("kept", "rewritten"));
            }),
        ),
        (
            "deleted record",
            Box::new(|| {
                std::fs::remove_file(fx.record(&b)).unwrap();
            }),
        ),
    ];
    for (name, change) in cases {
        change();
        fx.git(&["add", "-A"]);
        let verdict = fx.commit(&[]);
        assert!(
            fx.full_audit_ran(),
            "{name}: took the note route ({verdict:?})"
        );
        fx.git(&["reset", "-q", "--hard"]);
    }
}

#[test]
fn the_note_route_needs_a_tree_this_binary_accepted() {
    let fx = Fx::new();
    let id = fx.add("noted", &[]);
    fx.forget_accepted_trees();
    fx.append_note(&id, "after forgetting");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(
        fx.full_audit_ran(),
        "an unaccepted base took the note route"
    );
    fx.append_note(&id, "after the full audit accepted HEAD");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(!fx.full_audit_ran());
}

#[test]
fn the_note_route_and_the_full_audit_reach_the_same_verdict() {
    // Each case stages one change on an accepted tree; with and without the record of
    // accepted trees the hook must agree. Only the accepting cases may take the route.
    type Change<'a> = Box<dyn Fn(&Fx, &str, &str) + 'a>;
    let cases: Vec<(&str, bool, Change)> = vec![
        (
            "ordinary note",
            true,
            Box::new(|fx, a, _| fx.append_note(a, "plain")),
        ),
        (
            "note citing a task",
            true,
            Box::new(|fx, a, b| fx.append_note(a, &format!("see {b}"))),
        ),
        (
            "malformed appended note",
            false,
            Box::new(|fx, a, _| {
                let raw = fx.read(a);
                fx.write(a, &format!("{raw}- not a timestamped note\n"));
            }),
        ),
        (
            "note with unledgered attachment",
            false,
            Box::new(|fx, a, _| {
                fx.append_note(a, "plain");
                let dir = fx.project.join(format!("tasks/files/{a}"));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("loose.txt"), "x").unwrap();
            }),
        ),
        (
            "ledger note without its file",
            false,
            Box::new(|fx, a, _| {
                fx.append_note(a, "attached: missing.txt (3 bytes)");
            }),
        ),
    ];
    for (name, accepts, change) in cases {
        let mut verdicts = Vec::new();
        for primed in [true, false] {
            let fx = Fx::new();
            let a = fx.add("a", &[]);
            let b = fx.add("b", &[]);
            if !primed {
                fx.forget_accepted_trees();
            }
            change(&fx, &a, &b);
            fx.git(&["add", "-A"]);
            let verdict = fx.commit(&[]);
            assert_eq!(
                verdict.is_ok(),
                accepts,
                "{name} (primed {primed}): {verdict:?}"
            );
            assert_eq!(
                fx.full_audit_ran(),
                !(primed && accepts),
                "{name} (primed {primed}): route"
            );
            verdicts.push(verdict.is_ok());
        }
        assert_eq!(verdicts[0], verdicts[1], "{name}");
    }
}

#[test]
fn tasks_auto_commit_passes_the_staged_hook_beside_unrelated_breakage() {
    let fx = Fx::new();
    let a = fx.add("a", &[]);
    let b = fx.add("b", &[]);
    fx.write(&b, "not a task record\n");
    let out = fx
        .env
        .cmd(&fx.project)
        .args(["note", &a, "written by the tracker"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("commit_failed"), "{stderr}");
    let changed = fx.git(&["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(changed.trim(), fx.rel(&a));
}

#[test]
fn the_first_commit_of_a_project_takes_the_full_audit() {
    let fx = Fx::with(false);
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(fx.full_audit_ran());
    let id = fx.add("after", &[]);
    fx.append_note(&id, "plain");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(!fx.full_audit_ran());
}

#[test]
fn a_staged_check_outside_a_hook_reads_the_repository_index() {
    let fx = Fx::new();
    let id = fx.add("one", &[]);
    let valid = fx.read(&id);
    fx.write(&id, "not a task record\n");
    fx.git(&["add", &fx.rel(&id)]);
    fx.write(&id, &valid);
    // Plain `check` in a Git-backed records checkout also enforces the changed-index
    // record guard so legacy pre-commit hooks calling plain `check` cannot admit a
    // malformed or unsupported staged task blob behind a valid working copy.
    let plain = fx.env.cmd(&fx.project).args(["check"]).output().unwrap();
    assert_eq!(plain.status.code(), Some(1), "{plain:?}");
    let staged = fx
        .env
        .cmd(&fx.project)
        .args(["check", "--staged"])
        .output()
        .unwrap();
    assert_eq!(staged.status.code(), Some(1), "{staged:?}");
    let report: serde_json::Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(report["errors"][0]["kind"], "parse", "{report}");
    // Findings name the records directory, never the private copy they were read from.
    assert!(
        !String::from_utf8_lossy(&staged.stdout).contains("tasks-staged-"),
        "{report}"
    );
    // Non-record staged breakage (a staged spec removal kept in the working tree)
    // still distinguishes working-tree `check` from index-only `check --staged`.
    fx.git(&["reset", "-q", "HEAD", "--", &fx.rel(&id)]);
    let spec = fx.project.join("docs/specs/kept.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# Spec\n").unwrap();
    assert!(
        fx.env
            .cmd(&fx.project)
            .args(["edit", &id, "--spec", "docs/specs/kept.md"])
            .output()
            .unwrap()
            .status
            .success()
    );
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    fx.git(&[
        "rm",
        "-q",
        "--cached",
        &format!("{PREFIX}/docs/specs/kept.md"),
    ]);
    let plain_doc = fx.env.cmd(&fx.project).args(["check"]).output().unwrap();
    assert!(plain_doc.status.success(), "{plain_doc:?}");
    let staged_doc = fx
        .env
        .cmd(&fx.project)
        .args(["check", "--staged"])
        .output()
        .unwrap();
    assert_eq!(staged_doc.status.code(), Some(1), "{staged_doc:?}");
}

#[test]
fn the_hook_keeps_its_layout_rules() {
    let fx = Fx::new();
    fx.add("kept", &[]);
    // Removing only the config leaves records no project owns.
    fx.git(&[
        "rm",
        "-q",
        "--cached",
        &format!("{PREFIX}/tasks/.config.toml"),
    ]);
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("no tasks/.config.toml"), "{refused}");
    fx.git(&["reset", "-q"]);
    // Removing the whole project directory leaves nothing to check.
    fx.git(&["rm", "-q", "-r", "--cached", PREFIX]);
    fx.commit(&[]).unwrap();
}

#[test]
fn a_staged_document_removal_is_judged_although_the_working_copy_keeps_it() {
    let fx = Fx::new();
    let spec = fx.project.join("docs/specs/design.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# Design\n").unwrap();
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    fx.add("linked", &["--spec", "docs/specs/design.md"]);
    fx.git(&[
        "rm",
        "-q",
        "--cached",
        &format!("{PREFIX}/docs/specs/design.md"),
    ]);
    assert!(spec.is_file());
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("doc_missing"), "{refused}");
}

#[test]
fn a_commit_spanning_two_projects_checks_each_staged_tree() {
    let fx = Fx::new();
    let code = fx._base.path().canonicalize().unwrap().join("code2");
    repo(fx.env.home.path(), &code);
    let out = fx
        .env
        .cmd(&code)
        .args(["init", "--prefix", "su", "--records"])
        .arg(&fx.records)
        .output()
        .unwrap();
    assert!(out.status.success(), "init: {out:?}");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    let other = fx.records.join("su");
    let a = fx.add("a", &[]);
    let b = common::id_of(fx.env.json(&other, &["add", "b"]));
    let b_record = other.join(format!("tasks/{b}.md"));
    let b_valid = std::fs::read_to_string(&b_record).unwrap();
    // A malformed staged record in the second project refuses the whole commit, though
    // its working copy is valid and the first project's change is fine.
    fx.append_note(&a, "fine");
    std::fs::write(&b_record, "not a task record\n").unwrap();
    fx.git(&["add", "-A"]);
    std::fs::write(&b_record, &b_valid).unwrap();
    let refused = fx.commit(&[]).unwrap_err();
    assert!(refused.contains("tasks check failed for su"), "{refused}");
    // Valid staged changes in both are accepted.
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
}

#[test]
fn a_record_from_another_version_is_replaced_not_refused() {
    let fx = Fx::new();
    let id = fx.add("noted", &[]);
    let dir = fx.env.home.path().join(".local/state/tasks/staged-checks");
    let files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    std::fs::write(&files[0], "version = 99\nkey = \"future\"\npassed = []\n").unwrap();
    fx.append_note(&id, "after a newer binary wrote the record");
    fx.git(&["add", "-A"]);
    fx.commit(&[]).unwrap();
    assert!(fx.full_audit_ran());
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.starts_with("version = 1\n"), "{text}");
}
