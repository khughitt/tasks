mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn write(path: &Path, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

fn notes(env: &TestEnv, dir: &Path, id: &str) -> Vec<String> {
    env.json(dir, &["show", id])["task"]["notes"]
        .as_array()
        .map(|notes| {
            notes
                .iter()
                .map(|note| note["text"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn set_cap(dir: &Path, bytes: u64) {
    let config = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!("\n[attachments]\nmax_bytes = {bytes}\n"));
    std::fs::write(config, text).unwrap();
}

fn stored(dir: &Path, id: &str, name: &str) -> PathBuf {
    dir.join("tasks/files").join(id).join(name)
}

#[test]
fn attach_copies_a_file_and_records_a_ledger_note() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/before.png"), b"png bytes");
    let out = env.json(
        &dir,
        &[
            "attach",
            &id,
            source.to_str().unwrap(),
            "--caption",
            "the stale row",
        ],
    );
    assert_eq!(out["id"], id);
    assert_eq!(out["file"]["name"], "before.png");
    assert_eq!(out["file"]["bytes"], 9);
    let path = stored(&dir, &id, "before.png");
    assert_eq!(out["file"]["path"], path.display().to_string());
    assert_eq!(std::fs::read(&path).unwrap(), b"png bytes");
    assert_eq!(
        notes(&env, &dir, &id),
        ["attached: before.png (9 bytes): the stale row"]
    );
}

#[test]
fn attach_accepts_an_empty_file_and_names_it_with_name() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/empty"), b"");
    env.json(
        &dir,
        &[
            "attach",
            &id,
            source.to_str().unwrap(),
            "--name",
            "empty.txt",
        ],
    );
    assert_eq!(notes(&env, &dir, &id), ["attached: empty.txt (0 bytes)"]);
}

#[test]
fn attach_from_stdin_needs_a_name() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    assert_eq!(
        env.fail(&dir, &["attach", &id, "-"]),
        "invalid_attachment_name"
    );
    let out = env
        .cmd(&dir)
        .args(["attach", &id, "-", "--name", "piped.png"])
        .write_stdin(b"from stdin".to_vec())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        std::fs::read(stored(&dir, &id, "piped.png")).unwrap(),
        b"from stdin"
    );
}

#[test]
fn attach_rejects_invalid_names() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"x");
    let long = "a".repeat(129);
    for name in [".hidden", "a/b", "sp ace", "colon:png", "", long.as_str()] {
        assert_eq!(
            env.fail(
                &dir,
                &["attach", &id, source.to_str().unwrap(), "--name", name]
            ),
            "invalid_attachment_name",
            "{name:?}"
        );
    }
    assert!(!dir.join("tasks/files").exists());
}

#[test]
fn attach_fails_before_writing_on_bad_input() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"x");
    // A directory as the source, an unknown id, and a multi-line caption.
    std::fs::create_dir_all(dir.join("scratch/folder")).unwrap();
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                dir.join("scratch/folder").to_str().unwrap(),
                "--name",
                "f.png"
            ]
        ),
        "io"
    );
    assert_eq!(
        env.fail(&dir, &["attach", "dot-ffffff", source.to_str().unwrap()]),
        "task_not_found"
    );
    assert_eq!(
        env.fail(
            &dir,
            &[
                "attach",
                &id,
                source.to_str().unwrap(),
                "--caption",
                "two\nlines"
            ]
        ),
        "validation"
    );
    assert!(!dir.join("tasks/files").exists());
    assert!(notes(&env, &dir, &id).is_empty());
}

#[test]
fn attach_refuses_a_live_name_and_an_unrecorded_name_with_different_bytes() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"one");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap()]),
        "attachment_exists"
    );
    write(&stored(&dir, &id, "b.png"), b"unrecorded");
    let other = write(&dir.join("scratch/b.png"), b"different");
    assert_eq!(
        env.fail(&dir, &["attach", &id, other.to_str().unwrap()]),
        "attachment_exists"
    );
    assert_eq!(
        std::fs::read(stored(&dir, &id, "b.png")).unwrap(),
        b"unrecorded"
    );
}

#[test]
fn attach_finishes_an_interrupted_attach_with_identical_bytes() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    write(&stored(&dir, &id, "a.png"), b"same");
    let source = write(&dir.join("scratch/a.png"), b"same");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    assert_eq!(notes(&env, &dir, &id), ["attached: a.png (4 bytes)"]);
}

#[test]
fn attach_enforces_the_configured_cap() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    set_cap(&dir, 8);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let fits = write(&dir.join("scratch/fits.bin"), &[7; 8]);
    let over = write(&dir.join("scratch/over.bin"), &[7; 9]);
    env.json(&dir, &["attach", &id, fits.to_str().unwrap()]);
    assert_eq!(
        env.fail(&dir, &["attach", &id, over.to_str().unwrap()]),
        "attachment_too_large"
    );
    assert!(!stored(&dir, &id, "over.bin").exists());
}

#[test]
fn config_rejects_a_zero_cap_and_unknown_keys() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    set_cap(&dir, 0);
    assert_eq!(env.fail(&dir, &["list"]), "config");
    let config = dir.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("max_bytes = 0", "max_size = 5");
    std::fs::write(&config, text).unwrap();
    assert_eq!(env.fail(&dir, &["list"]), "config");
}

#[test]
fn attach_routes_to_the_project_the_id_names() {
    let mut env = TestEnv::new();
    let here = env.init("dot");
    let there = env.init("ops");
    let id = id_of(env.json(&there, &["add", "T", "-p", "2"]));
    let source = write(&here.join("scratch/a.png"), b"x");
    env.json(&here, &["attach", &id, source.to_str().unwrap()]);
    assert!(stored(&there, &id, "a.png").is_file());
    assert!(!here.join("tasks/files").exists());
}

#[cfg(unix)]
#[test]
fn attach_removes_its_file_when_the_save_fails_and_keeps_an_earlier_one() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let earlier = write(&stored(&dir, &id, "earlier.png"), b"earlier");
    let source = write(&dir.join("scratch/new.png"), b"new");
    let tasks = dir.join("tasks");
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o555)).unwrap();
    let kind = env.fail(&dir, &["attach", &id, source.to_str().unwrap()]);
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(kind, "io");
    assert!(!stored(&dir, &id, "new.png").exists());
    assert!(earlier.is_file());
}

#[cfg(unix)]
#[test]
fn storage_symlinks_are_refused_and_nothing_outside_is_touched() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let outside = tempfile::tempdir().unwrap();
    let source = write(&dir.join("scratch/a.png"), b"x");

    // tasks/files itself is a symlink.
    std::os::unix::fs::symlink(outside.path(), dir.join("tasks/files")).unwrap();
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap()]),
        "attachment_unsafe"
    );
    assert_eq!(
        env.fail(&dir, &["detach", &id, "a.png", "why"]),
        "attachment_unsafe"
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    std::fs::remove_file(dir.join("tasks/files")).unwrap();

    // tasks/files/<id> is a symlink to a directory holding a file detach could delete.
    std::fs::create_dir(dir.join("tasks/files")).unwrap();
    write(&outside.path().join("a.png"), b"keep me");
    std::os::unix::fs::symlink(outside.path(), dir.join("tasks/files").join(&id)).unwrap();
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap()]),
        "attachment_unsafe"
    );
    assert_eq!(
        env.fail(&dir, &["detach", &id, "a.png", "why"]),
        "attachment_unsafe"
    );
    assert_eq!(
        std::fs::read(outside.path().join("a.png")).unwrap(),
        b"keep me"
    );
    std::fs::remove_file(dir.join("tasks/files").join(&id)).unwrap();

    // tasks/files is a regular file.
    std::fs::remove_dir(dir.join("tasks/files")).unwrap();
    write(&dir.join("tasks/files"), b"not a directory");
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap()]),
        "attachment_unsafe"
    );
}

#[cfg(unix)]
#[test]
fn attach_refuses_an_existing_symlink_or_fifo_without_reading_it() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"same");
    let target = write(&dir.join("scratch/target.png"), b"same");
    std::fs::create_dir_all(dir.join("tasks/files").join(&id)).unwrap();
    std::os::unix::fs::symlink(&target, stored(&dir, &id, "a.png")).unwrap();
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap()]),
        "attachment_unsafe"
    );
    let status = std::process::Command::new("mkfifo")
        .arg(stored(&dir, &id, "b.png"))
        .status()
        .unwrap();
    assert!(status.success());
    let other = write(&dir.join("scratch/b.png"), b"same");
    // A FIFO would block a read forever; the refusal must come from its metadata.
    assert_eq!(
        env.fail(&dir, &["attach", &id, other.to_str().unwrap()]),
        "attachment_unsafe"
    );
    assert!(notes(&env, &dir, &id).is_empty());
    assert_eq!(
        env.fail(&dir, &["detach", &id, "a.png", "why"]),
        "attachment_unsafe"
    );
}

#[test]
fn detach_records_then_removes_and_prunes_the_directory() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"x");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    let out = env.json(&dir, &["detach", &id, "a.png", "wrong screenshot"]);
    assert_eq!(out["removed"], true);
    assert!(!dir.join("tasks/files").join(&id).exists());
    assert_eq!(
        notes(&env, &dir, &id),
        [
            "attached: a.png (1 bytes)",
            "detached: a.png: wrong screenshot"
        ]
    );
    assert_eq!(
        env.fail(&dir, &["detach", &id, "a.png", "again"]),
        "attachment_missing"
    );
}

#[cfg(unix)]
#[test]
fn detach_leaves_the_file_when_the_save_fails() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"x");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    let tasks = dir.join("tasks");
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o555)).unwrap();
    let kind = env.fail(&dir, &["detach", &id, "a.png", "why"]);
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(kind, "io");
    assert!(stored(&dir, &id, "a.png").is_file());
}

#[test]
fn detach_finishes_an_interrupted_detach_without_a_second_note() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"x");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    env.json(&dir, &["note", &id, "detached: a.png: wrong"]);
    env.json(&dir, &["detach", &id, "a.png", "wrong"]);
    assert!(!stored(&dir, &id, "a.png").exists());
    assert_eq!(notes(&env, &dir, &id).len(), 2);
}

#[test]
fn detach_clears_an_unrecorded_file() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    write(&stored(&dir, &id, "stray.png"), b"x");
    env.json(&dir, &["detach", &id, "stray.png", "interrupted attach"]);
    assert!(!stored(&dir, &id, "stray.png").exists());
    assert_eq!(
        notes(&env, &dir, &id),
        ["detached: stray.png: interrupted attach"]
    );
}

/// A `wl-paste` stand-in: lists `$STUB_TYPES` and prints `$STUB_IMAGE`.
#[cfg(unix)]
fn stub_wl_paste() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("wl-paste");
    std::fs::write(
        &script,
        "#!/bin/sh\ncase \"$1\" in\n  --list-types) printf '%b' \"$STUB_TYPES\" ;;\n  *) exec cat \"$STUB_IMAGE\" ;;\nesac\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

#[cfg(unix)]
#[test]
fn clipboard_attach_reads_the_preferred_image_type() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let bin = stub_wl_paste();
    let image = write(&dir.join("scratch/clip"), b"clipboard png");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let run = |types: &str, args: &[&str]| {
        env.cmd(&dir)
            .env("PATH", &path)
            .env("STUB_TYPES", types)
            .env("STUB_IMAGE", &image)
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(
        "text/plain\\nimage/jpeg\\nimage/png\\n",
        &["attach", &id, "--clipboard"],
    );
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let name = v["file"]["name"].as_str().unwrap().to_string();
    assert!(
        name.starts_with("clipboard-") && name.ends_with("Z.png"),
        "{name}"
    );
    assert_eq!(
        std::fs::read(stored(&dir, &id, &name)).unwrap(),
        b"clipboard png"
    );

    let out = run("text/plain\\n", &["attach", &id, "--clipboard"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("clipboard_no_image"));

    // Retry of an interrupted clipboard attach: the file exists unrecorded, --name finishes it.
    write(&stored(&dir, &id, "clipboard-retry.png"), b"clipboard png");
    let out = run(
        "image/png\\n",
        &[
            "attach",
            &id,
            "--clipboard",
            "--name",
            "clipboard-retry.png",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        notes(&env, &dir, &id).contains(&"attached: clipboard-retry.png (13 bytes)".to_string())
    );
}

#[cfg(unix)]
#[test]
fn an_endless_clipboard_stops_at_the_cap() {
    use std::io::Read;
    use std::time::{Duration, Instant};
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    set_cap(&dir, 1024);
    let bin = stub_wl_paste();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    // /dev/zero never ends, so a read that buffers the whole image before checking the
    // cap never returns; the deadline turns that hang into a failure.
    let mut child = env
        .raw(&dir)
        .env("PATH", &path)
        .env("STUB_TYPES", "image/png\\n")
        .env("STUB_IMAGE", "/dev/zero")
        .args(["attach", &id, "--clipboard"])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("attach kept reading an endless clipboard past the cap");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("attachment_too_large"), "{stderr}");
    assert!(!dir.join("tasks/files").join(&id).exists());
    assert!(notes(&env, &dir, &id).is_empty());
}

#[test]
fn clipboard_without_wl_paste_is_unavailable() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let empty = tempfile::tempdir().unwrap();
    let out = env
        .cmd(&dir)
        .env("PATH", empty.path())
        .args(["attach", &id, "--clipboard"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("clipboard_unavailable"));
}

/// The kinds `tasks check` reports at `level`. Errors exit 1 and warnings alone exit 0,
/// so this asserts the exit status the parsed errors call for, where `TestEnv::check`
/// would assert success and panic on the error cases these tests exist to see.
fn check_kinds(env: &TestEnv, dir: &Path, level: &str) -> Vec<String> {
    let out = env.cmd(dir).args(["check"]).output().unwrap();
    let findings: serde_json::Value = if out.stdout.is_empty() {
        serde_json::json!({"errors": [], "warnings": []})
    } else {
        serde_json::from_slice(&out.stdout).unwrap_or_else(|error| panic!("{error}: {out:?}"))
    };
    let errors = findings["errors"].as_array().unwrap().len();
    assert_eq!(out.status.code(), Some(i32::from(errors > 0)), "{out:?}");
    findings[level]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn show_and_next_list_files_with_absolute_paths() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/b.png"), b"bb");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    let source = write(&dir.join("scratch/a.png"), b"a");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    let show = env.json(&dir, &["show", &id]);
    assert_eq!(show["files"][0]["name"], "a.png");
    assert_eq!(show["files"][1]["name"], "b.png");
    assert_eq!(show["files"][1]["bytes"], 2);
    assert_eq!(
        show["files"][0]["path"],
        stored(&dir, &id, "a.png").display().to_string()
    );
    assert_eq!(
        env.json(&dir, &["next"])["next"]["files"][0]["name"],
        "a.png"
    );
    let pretty = env.pretty(&dir, &["show", &id]);
    assert!(pretty.contains("# files"), "{pretty}");
    assert!(
        pretty.contains(&format!("{} (2 B)", stored(&dir, &id, "b.png").display())),
        "{pretty}"
    );
    // No files: the key is absent.
    let bare = id_of(env.json(&dir, &["add", "U", "-p", "3"]));
    assert!(env.json(&dir, &["show", &bare]).get("files").is_none());
}

#[test]
fn show_warns_about_a_leftover_temp_file_and_does_not_list_it() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    write(&stored(&dir, &id, ".a.png.tmp-123"), b"partial");
    let show = env.json(&dir, &["show", &id]);
    assert!(show.get("files").is_none());
    assert!(
        show["warnings"][0]
            .as_str()
            .unwrap()
            .contains("attachment_invalid"),
        "{show}"
    );
    assert_eq!(check_kinds(&env, &dir, "warnings"), ["attachment_invalid"]);
}

#[test]
fn check_reports_each_attachment_drift_kind() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    set_cap(&dir, 4);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"a");
    env.json(&dir, &["attach", &id, source.to_str().unwrap()]);
    assert!(check_kinds(&env, &dir, "errors").is_empty());

    // Unnoted: a file no ledger entry covers.
    write(&stored(&dir, &id, "b.png"), b"b");
    // Missing: a live name with no file.
    std::fs::remove_file(stored(&dir, &id, "a.png")).unwrap();
    // Orphan: a directory for no record, and a stray file directly under tasks/files.
    write(&dir.join("tasks/files/dot-ffffff/x.png"), b"x");
    write(&dir.join("tasks/files/loose.png"), b"x");
    // Too large: above the configured cap of 4 bytes.
    write(&stored(&dir, &id, "big.png"), b"12345");
    let mut errors = check_kinds(&env, &dir, "errors");
    errors.sort();
    assert_eq!(
        errors,
        [
            "attachment_missing",
            "attachment_orphan",
            "attachment_orphan",
            "attachment_unnoted",
            "attachment_unnoted",
        ]
    );
    assert!(check_kinds(&env, &dir, "warnings").contains(&"attachment_too_large".to_string()));
    let out = env.cmd(&dir).args(["check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn check_and_show_report_an_unsafe_storage_directory_without_reading_through_it() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("a.png"), b"x");
    std::fs::create_dir(dir.join("tasks/files")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.join("tasks/files").join(&id)).unwrap();
    assert_eq!(check_kinds(&env, &dir, "errors"), ["attachment_unsafe"]);
    let show = env.json(&dir, &["show", &id]);
    assert!(show.get("files").is_none());
    assert!(
        show["warnings"][0]
            .as_str()
            .unwrap()
            .contains("attachment_unsafe")
    );
}

#[test]
fn check_catches_a_collision_split_by_name_before_and_after_recovery() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let winner = id_of(env.json(&dir, &["add", "Winner", "-p", "2"]));
    let loser = id_of(env.json(&dir, &["add", "Loser", "-p", "2"]));
    let a = write(&dir.join("scratch/a.png"), b"a");
    let b = write(&dir.join("scratch/b.png"), b"b");
    env.json(&dir, &["attach", &winner, a.to_str().unwrap()]);
    env.json(&dir, &["attach", &loser, b.to_str().unwrap()]);
    // A merged directory: the loser's file sits under the winner's id.
    std::fs::rename(
        stored(&dir, &loser, "b.png"),
        stored(&dir, &winner, "b.png"),
    )
    .unwrap();
    std::fs::remove_dir(dir.join("tasks/files").join(&loser)).unwrap();
    let mut errors = check_kinds(&env, &dir, "errors");
    errors.sort();
    assert_eq!(errors, ["attachment_missing", "attachment_unnoted"]);
    // Recovery step 1: move each name live only in the loser's ledger.
    std::fs::create_dir(dir.join("tasks/files").join(&loser)).unwrap();
    std::fs::rename(
        stored(&dir, &winner, "b.png"),
        stored(&dir, &loser, "b.png"),
    )
    .unwrap();
    assert!(check_kinds(&env, &dir, "errors").is_empty());
}
