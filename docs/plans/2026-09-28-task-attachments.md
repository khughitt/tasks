# Task Attachments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A task carries files under `tasks/files/<id>/`, recorded by ledger notes, that `show` lists, `check` audits, and `rename` moves.

**Architecture:**
- **Storage and the ledger:** one new module, `src/attachments.rs`, owns the storage rules (names, symlink-safe directory checks, atomic writes), the ledger, and the audit. A task's `attached:` and `detached:` notes are its ledger.
- **Commands:** `src/commands/attach.rs` holds the `attach` and `detach` commands. `src/clipboard.rs` wraps `wl-paste`.
- **Show and check:** `show`/`next` (through `describe`) and `check` call the audit.
- **Rename:** its inventory records a per-entry `attachments` baseline. Its snapshot observes the source and destination directory states, and its classifier adds R9–R11.

**Tech Stack:** Rust 2024, clap 4, serde/toml, thiserror; tests with assert_cmd and tempfile.

**Spec:** `docs/specs/2026-09-28-task-attachments-design.md` (reviewed; approved at e76dcc7).

## Global Constraints

- **Names:** `[A-Za-z0-9._-]`, 1–128 bytes, not starting with `.`.
- **Layout:** `tasks/files/<id>/<name>`. `tasks/files` and `tasks/files/<id>` are inspected with `symlink_metadata`, never followed, and created with `create_dir`, never `create_dir_all`.
- **Cap:** `[attachments] max_bytes` defaults to `2097152`. Zero or a negative value is `Error::Config`, and the table is `deny_unknown_fields`.
- **Ledger note forms, exactly:**
  - `attached: <name> (<n> bytes)`
  - `attached: <name> (<n> bytes): <caption>`
  - `detached: <name>: <why>`

  They are plain notes: no provenance, no lifecycle marker.
- **New error kinds, exactly:**
  - `invalid_attachment_name`
  - `attachment_exists`
  - `attachment_missing`
  - `attachment_unsafe`
  - `attachment_too_large`
  - `clipboard_unavailable`
  - `clipboard_no_image`
- **New check kinds:**
  - Errors: `attachment_unsafe`, `attachment_orphan`, `attachment_unnoted`, `attachment_missing`.
  - Warnings: `attachment_invalid`, `attachment_too_large`.
- **Rename refusal codes:** R1–R8 keep their meanings. The new codes are R9 (unexpected destination attachments), R10 (attachments missing from both sides), and R11 (source attachments appeared after the inventory).
- **JSON:** the changes are additive only.
  - `show`/`next` gain `files` (omitted when empty).
  - `attach` and `detach` outputs are new.
  - No existing field changes.
- **Rules:** fail early with a typed error, no silent fallbacks, conventional commits, no AI-attribution trailers.
- **Running tests:**
  - Use `just test-fast <filter>` while working and `just check` before each commit. Never run `cargo test` directly.
  - The exhaustive rename test is `#[ignore]`d; run it with `just test`.
- **Installing:** do not `cargo install --path .` from the worktree, because that repoints the host's `tasks` at worktree code. Reinstall from the main checkout after merge.

## Review Focus

These are inputs the spec implies but no spec test names; each gets a test in its owning task.

1. **The source path is a directory or unreadable:** `attach` fails `io` and writes nothing, with no directory and no note. (Task 1)
2. **The id has no record:** `attach` fails `task_not_found` and creates no `tasks/files/` directory. (Task 1)
3. **The caption contains a line break:** `attach` fails `validation` before writing any file. (Task 1)
4. **An empty source file:** a 0-byte attach succeeds and records `(0 bytes)`. (Task 1)
5. **A leftover temp file `.<name>.tmp-<pid>` in a task's directory:** `show` does not list it and warns `attachment_invalid`; `check` reports the same warning. (Task 2)

---

### Task 1: Storage, config, and the attach and detach commands

**Files:**
- Create: `src/attachments.rs`, `src/clipboard.rs`, `src/commands/attach.rs`, `tests/attachments.rs`
- Modify: `src/main.rs` (module list), `src/error.rs`, `src/repo.rs` (`Config`, `Project`), `src/cli.rs` (`Command`), `src/commands/mod.rs` (module list, dispatch), `src/output.rs` (`FileInfo`, `AttachOut`, `DetachOut`, `Output`, `pretty`, `warnings_of`)

**Interfaces:**
- Consumes (existing):
  - `commands::{Ctx, load, owner_name, append_note, save, open_id_write_ctx}`
  - `Ctx::resolve_for_guard`, `Ctx::claims_mut`
  - `format::validate_note_text`, `time::now`
- Produces (later tasks rely on these exact names):
  - `attachments::FILES_DIR: &str` (`"files"`), `attachments::DEFAULT_MAX_BYTES: u64`
  - `attachments::validate_name(&str) -> Result<()>`
  - `attachments::files_root(&Project) -> PathBuf`, `attachments::task_dir(&Project, &TaskId) -> PathBuf`
  - `attachments::DirState { Absent, Directory, Unsafe(String) }` and `attachments::dir_state(&Path) -> Result<DirState>`
  - `attachments::EntryState { Absent, File(u64), Unsafe(String) }` and `attachments::entry_state(&Path) -> Result<EntryState>`
  - `attachments::Ledger { Attached, Detached }`, `attachments::ledger(&Task) -> BTreeMap<String, Ledger>`
  - `Project::attachments_max_bytes: u64`
  - `output::FileInfo { name: String, path: String, bytes: u64 }`
  - `Error::{InvalidAttachmentName, AttachmentExists, AttachmentMissing, AttachmentUnsafe, AttachmentTooLarge, ClipboardUnavailable, ClipboardNoImage}(String)`

- [ ] **Step 1: Write the failing end-to-end tests**

Create `tests/attachments.rs`:

```rust
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
        &["attach", &id, source.to_str().unwrap(), "--caption", "the stale row"],
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
    env.json(&dir, &["attach", &id, source.to_str().unwrap(), "--name", "empty.txt"]);
    assert_eq!(notes(&env, &dir, &id), ["attached: empty.txt (0 bytes)"]);
}

#[test]
fn attach_from_stdin_needs_a_name() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&dir, &["attach", &id, "-"]), "invalid_attachment_name");
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
            env.fail(&dir, &["attach", &id, source.to_str().unwrap(), "--name", name]),
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
        env.fail(&dir, &["attach", &id, dir.join("scratch/folder").to_str().unwrap(), "--name", "f.png"]),
        "io"
    );
    assert_eq!(
        env.fail(&dir, &["attach", "dot-ffffff", source.to_str().unwrap()]),
        "task_not_found"
    );
    assert_eq!(
        env.fail(&dir, &["attach", &id, source.to_str().unwrap(), "--caption", "two\nlines"]),
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
    assert_eq!(std::fs::read(stored(&dir, &id, "b.png")).unwrap(), b"unrecorded");
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
    assert_eq!(env.fail(&dir, &["attach", &id, source.to_str().unwrap()]), "attachment_unsafe");
    assert_eq!(env.fail(&dir, &["detach", &id, "a.png", "why"]), "attachment_unsafe");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    std::fs::remove_file(dir.join("tasks/files")).unwrap();

    // tasks/files/<id> is a symlink to a directory holding a file detach could delete.
    std::fs::create_dir(dir.join("tasks/files")).unwrap();
    write(&outside.path().join("a.png"), b"keep me");
    std::os::unix::fs::symlink(outside.path(), dir.join("tasks/files").join(&id)).unwrap();
    assert_eq!(env.fail(&dir, &["attach", &id, source.to_str().unwrap()]), "attachment_unsafe");
    assert_eq!(env.fail(&dir, &["detach", &id, "a.png", "why"]), "attachment_unsafe");
    assert_eq!(std::fs::read(outside.path().join("a.png")).unwrap(), b"keep me");
    std::fs::remove_file(dir.join("tasks/files").join(&id)).unwrap();

    // tasks/files is a regular file.
    std::fs::remove_dir(dir.join("tasks/files")).unwrap();
    write(&dir.join("tasks/files"), b"not a directory");
    assert_eq!(env.fail(&dir, &["attach", &id, source.to_str().unwrap()]), "attachment_unsafe");
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
    assert_eq!(env.fail(&dir, &["attach", &id, source.to_str().unwrap()]), "attachment_unsafe");
    let status = std::process::Command::new("mkfifo")
        .arg(stored(&dir, &id, "b.png"))
        .status()
        .unwrap();
    assert!(status.success());
    let other = write(&dir.join("scratch/b.png"), b"same");
    // A FIFO would block a read forever; the refusal must come from its metadata.
    assert_eq!(env.fail(&dir, &["attach", &id, other.to_str().unwrap()]), "attachment_unsafe");
    assert!(notes(&env, &dir, &id).is_empty());
    assert_eq!(env.fail(&dir, &["detach", &id, "a.png", "why"]), "attachment_unsafe");
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
        ["attached: a.png (1 bytes)", "detached: a.png: wrong screenshot"]
    );
    assert_eq!(env.fail(&dir, &["detach", &id, "a.png", "again"]), "attachment_missing");
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
    assert_eq!(notes(&env, &dir, &id), ["detached: stray.png: interrupted attach"]);
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
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH").unwrap());
    let run = |types: &str, args: &[&str]| {
        env.cmd(&dir)
            .env("PATH", &path)
            .env("STUB_TYPES", types)
            .env("STUB_IMAGE", &image)
            .args(args)
            .output()
            .unwrap()
    };
    let out = run("text/plain\\nimage/jpeg\\nimage/png\\n", &["attach", &id, "--clipboard"]);
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let name = v["file"]["name"].as_str().unwrap().to_string();
    assert!(name.starts_with("clipboard-") && name.ends_with("Z.png"), "{name}");
    assert_eq!(std::fs::read(stored(&dir, &id, &name)).unwrap(), b"clipboard png");

    let out = run("text/plain\\n", &["attach", &id, "--clipboard"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("clipboard_no_image"));

    // Retry of an interrupted clipboard attach: the file exists unrecorded, --name finishes it.
    write(&stored(&dir, &id, "clipboard-retry.png"), b"clipboard png");
    let out = run("image/png\\n", &["attach", &id, "--clipboard", "--name", "clipboard-retry.png"]);
    assert!(out.status.success(), "{out:?}");
    assert!(notes(&env, &dir, &id).contains(&"attached: clipboard-retry.png (13 bytes)".to_string()));
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
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH").unwrap());
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
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast attach`
Expected: compile of `tests/attachments.rs` succeeds, and the tests fail with clap's "unrecognized subcommand 'attach'" (exit 2 where the test expects 1 or 0).

- [ ] **Step 3: Add the error variants**

In `src/error.rs`, add to `enum Error` (after `Editor`):

```rust
    #[error("{0}")]
    InvalidAttachmentName(String),
    #[error("{0}")]
    AttachmentExists(String),
    #[error("{0}")]
    AttachmentMissing(String),
    #[error("{0}")]
    AttachmentUnsafe(String),
    #[error("{0}")]
    AttachmentTooLarge(String),
    #[error("{0}")]
    ClipboardUnavailable(String),
    #[error("{0}")]
    ClipboardNoImage(String),
```

In `with_suffix`, add these arms before the `Io` arm:

```rust
            Error::InvalidAttachmentName(detail) => Error::InvalidAttachmentName(detail + suffix),
            Error::AttachmentExists(detail) => Error::AttachmentExists(detail + suffix),
            Error::AttachmentMissing(detail) => Error::AttachmentMissing(detail + suffix),
            Error::AttachmentUnsafe(detail) => Error::AttachmentUnsafe(detail + suffix),
            Error::AttachmentTooLarge(detail) => Error::AttachmentTooLarge(detail + suffix),
            Error::ClipboardUnavailable(detail) => Error::ClipboardUnavailable(detail + suffix),
            Error::ClipboardNoImage(detail) => Error::ClipboardNoImage(detail + suffix),
```

In `kind`, add these arms:

```rust
            Error::InvalidAttachmentName(_) => "invalid_attachment_name",
            Error::AttachmentExists(_) => "attachment_exists",
            Error::AttachmentMissing(_) => "attachment_missing",
            Error::AttachmentUnsafe(_) => "attachment_unsafe",
            Error::AttachmentTooLarge(_) => "attachment_too_large",
            Error::ClipboardUnavailable(_) => "clipboard_unavailable",
            Error::ClipboardNoImage(_) => "clipboard_no_image",
```

- [ ] **Step 4: Add the storage module with its unit tests**

Create `src/attachments.rs`:

```rust
//! Files a task carries under `tasks/files/<id>/`, and the ledger notes that say which
//! record owns each one. See docs/specs/2026-09-28-task-attachments-design.md.

use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const FILES_DIR: &str = "files";
pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 128;

/// One path component of `[A-Za-z0-9._-]`, at most 128 bytes, not starting with `.`. No
/// space or colon, so a name parses unambiguously out of a ledger note.
pub fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidAttachmentName(format!(
            "attachment name {name:?} must be 1-{MAX_NAME_BYTES} bytes of [A-Za-z0-9._-], not starting with '.'"
        )))
    }
}

pub fn files_root(project: &Project) -> PathBuf {
    project.tasks_dir().join(FILES_DIR)
}

pub fn task_dir(project: &Project, id: &TaskId) -> PathBuf {
    files_root(project).join(id.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirState {
    Absent,
    Directory,
    Unsafe(String),
}

/// A storage directory's state, read without following a symlink.
pub fn dir_state(path: &Path) -> Result<DirState> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(DirState::Directory),
        Ok(meta) => Ok(DirState::Unsafe(format!(
            "{} is a {}, not a directory",
            path.display(),
            kind_of(&meta)
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(DirState::Absent),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryState {
    Absent,
    /// A regular file of this many bytes.
    File(u64),
    Unsafe(String),
}

/// An attachment's state, read without following a symlink or opening the entry.
pub fn entry_state(path: &Path) -> Result<EntryState> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(EntryState::File(meta.len())),
        Ok(meta) => Ok(EntryState::Unsafe(format!(
            "{} is a {}, not a regular file",
            path.display(),
            kind_of(&meta)
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EntryState::Absent),
        Err(error) => Err(error.into()),
    }
}

fn kind_of(meta: &std::fs::Metadata) -> &'static str {
    let kind = meta.file_type();
    if kind.is_symlink() {
        "symlink"
    } else if kind.is_dir() {
        "directory"
    } else if kind.is_file() {
        "regular file"
    } else {
        "special file"
    }
}

/// Checks both storage directories without following links. `Ok(true)` when the task's
/// directory exists; an unsafe one is `attachment_unsafe`.
pub fn check_storage(project: &Project, id: &TaskId) -> Result<bool> {
    for path in [files_root(project), task_dir(project, id)] {
        match dir_state(&path)? {
            DirState::Directory => {}
            DirState::Absent => return Ok(false),
            DirState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    Ok(true)
}

/// Creates whichever storage directories are missing. `create_dir` fails on an existing
/// symlink rather than writing through it. `Ok(true)` when the task directory was created.
pub fn ensure_task_dir(project: &Project, id: &TaskId) -> Result<bool> {
    let mut created = false;
    for path in [files_root(project), task_dir(project, id)] {
        match dir_state(&path)? {
            DirState::Directory => {}
            DirState::Absent => {
                std::fs::create_dir(&path)?;
                created = true;
            }
            DirState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    Ok(created)
}

/// Reads at most `max + 1` bytes, so a source over the cap costs no more memory than the
/// cap; one byte over is `attachment_too_large`.
pub fn read_capped(reader: impl Read, max: u64, what: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::AttachmentTooLarge(format!(
            "{what} is more than {max} bytes, the cap ([attachments] max_bytes in tasks/.config.toml)"
        )));
    }
    Ok(bytes)
}

/// Writes `bytes` to `path` through a `create_new` temp beside it, fsynced, then renamed.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("attachment paths end in a validated name");
    let temp = path.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written.and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ledger {
    Attached,
    Detached,
}

pub fn attached_note(name: &str, bytes: u64, caption: Option<&str>) -> String {
    match caption {
        Some(caption) => format!("attached: {name} ({bytes} bytes): {caption}"),
        None => format!("attached: {name} ({bytes} bytes)"),
    }
}

pub fn detached_note(name: &str, why: &str) -> String {
    format!("detached: {name}: {why}")
}

/// The ledger entry a note text records, if it is one.
pub fn parse_ledger_note(text: &str) -> Option<(Ledger, &str)> {
    if let Some(rest) = text.strip_prefix("attached: ") {
        let (name, rest) = rest.split_once(' ')?;
        let (count, rest) = rest.strip_prefix('(')?.split_once(" bytes)")?;
        let counted = !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit());
        let tail = rest.is_empty() || rest.starts_with(": ");
        return (counted && tail && validate_name(name).is_ok()).then_some((Ledger::Attached, name));
    }
    let (name, why) = text.strip_prefix("detached: ")?.split_once(": ")?;
    (!why.is_empty() && validate_name(name).is_ok()).then_some((Ledger::Detached, name))
}

/// Each name's latest ledger entry in `task`'s notes. A name is live when it is `Attached`.
pub fn ledger(task: &Task) -> BTreeMap<String, Ledger> {
    let mut state = BTreeMap::new();
    for note in &task.notes {
        if let Some((entry, name)) = parse_ledger_note(&note.text) {
            state.insert(name.to_string(), entry);
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_storage_rule() {
        for good in ["a.png", "Shot_2026-09-28.PNG", "x", &"a".repeat(128)] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", ".a", "a/b", "a b", "a:b", "é.png", &"a".repeat(129)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ledger_notes_round_trip_and_reject_near_misses() {
        assert_eq!(
            parse_ledger_note(&attached_note("a.png", 12, None)),
            Some((Ledger::Attached, "a.png"))
        );
        assert_eq!(
            parse_ledger_note(&attached_note("a.png", 12, Some("x (3 bytes): y"))),
            Some((Ledger::Attached, "a.png"))
        );
        assert_eq!(
            parse_ledger_note(&detached_note("a.png", "wrong: it was old")),
            Some((Ledger::Detached, "a.png"))
        );
        for text in [
            "attached: a.png",
            "attached: a.png (x bytes)",
            "attached: a.png (12 bytes)trailing",
            "attached: .a (1 bytes)",
            "detached: a.png",
            "detached: a.png: ",
            "note about attached: a.png (1 bytes)",
        ] {
            assert_eq!(parse_ledger_note(text), None, "{text}");
        }
    }
}
```

Add `mod attachments;` and `mod clipboard;` to `src/main.rs` in alphabetical order (`attachments` before `claims`, and `clipboard` after `cli`).

- [ ] **Step 5: Add the clipboard module**

Create `src/clipboard.rs`:

```rust
//! An image from the Wayland clipboard through `wl-paste`.

use crate::error::{Error, Result};
use std::io::Read;
use std::process::{Command, Stdio};

/// Preferred MIME types, first match wins, with the extension each is stored under.
const PREFERRED: [(&str, &str); 4] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

pub struct Image {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

pub fn choose(offered: &[String]) -> Option<(&'static str, &'static str)> {
    PREFERRED
        .iter()
        .find(|(mime, _)| offered.iter().any(|type_| type_ == mime))
        .copied()
}

/// `clipboard-<yyyymmddThhmmssZ>.<ext>` from an RFC 3339 UTC stamp such as `time::now()`.
pub fn default_name(now: &str, extension: &str) -> String {
    let compact: String = now.chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("clipboard-{compact}.{extension}")
}

fn unavailable(args: &[&str], stderr: &[u8]) -> Error {
    Error::ClipboardUnavailable(format!(
        "wl-paste {}: {}",
        args.join(" "),
        String::from_utf8_lossy(stderr).trim()
    ))
}

/// The offered types. A listing is a few lines, so it is read whole.
fn list_types() -> Result<Vec<u8>> {
    let args = ["--list-types"];
    let output = Command::new("wl-paste")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| Error::ClipboardUnavailable(format!("wl-paste: {error}")))?;
    if !output.status.success() {
        return Err(unavailable(&args, &output.stderr));
    }
    Ok(output.stdout)
}

/// The image's bytes, streamed through `attachments::read_capped`: at most `max + 1`
/// bytes are read, and a larger image kills and reaps `wl-paste` rather than draining it.
fn paste(mime: &str, max: u64) -> Result<Vec<u8>> {
    let args = ["--no-newline", "--type", mime];
    let mut child = Command::new("wl-paste")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::ClipboardUnavailable(format!("wl-paste: {error}")))?;
    // stderr drains on its own thread, so a chatty child cannot block on a full pipe
    // while this thread waits on stdout.
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let drain = std::thread::spawn(move || {
        let mut text = Vec::new();
        stderr.read_to_end(&mut text).map(|_| text)
    });
    let stdout = child.stdout.take().expect("stdout is piped");
    let bytes = match crate::attachments::read_capped(stdout, max, "the clipboard image") {
        Ok(bytes) => bytes,
        Err(error) => {
            // Not yet waited on, so the child is running or a zombie and kill succeeds.
            child.kill()?;
            child.wait()?;
            return Err(error);
        }
    };
    let status = child.wait()?;
    let stderr = drain.join().expect("the stderr reader does not panic")?;
    if !status.success() {
        return Err(unavailable(&args, &stderr));
    }
    Ok(bytes)
}

/// The preferred image on the clipboard, refused as `attachment_too_large` past `max`.
pub fn read(max: u64) -> Result<Image> {
    let listed = list_types()?;
    let offered: Vec<String> = String::from_utf8_lossy(&listed)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    let (mime, extension) = choose(&offered).ok_or_else(|| {
        Error::ClipboardNoImage(if offered.is_empty() {
            "the clipboard offers nothing".into()
        } else {
            format!("the clipboard offers no image: {}", offered.join(", "))
        })
    })?;
    Ok(Image {
        bytes: paste(mime, max)?,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_wins_over_jpeg_and_text_is_never_chosen() {
        let offered = |types: &[&str]| types.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(choose(&offered(&["text/plain", "image/jpeg", "image/png"])), Some(("image/png", "png")));
        assert_eq!(choose(&offered(&["image/jpeg"])), Some(("image/jpeg", "jpg")));
        assert_eq!(choose(&offered(&["text/plain", "image/bmp"])), None);
    }

    #[test]
    fn default_names_compact_the_stamp() {
        assert_eq!(default_name("2026-09-28T10:39:21Z", "png"), "clipboard-20260928T103921Z.png");
    }
}
```

- [ ] **Step 6: Add the config table**

In `src/repo.rs`:

- Add `pub attachments_max_bytes: u64,` to `Project`, after `feedback`, with the doc comment `/// The per-file cap from `[attachments] max_bytes`; `attachments::DEFAULT_MAX_BYTES` when unset.`
- Add the table to `Config`, after `feedback`:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attachments: Option<AttachmentsConfig>,
```

- Add the struct and validator after `feedback_scope`:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_bytes: Option<u64>,
}

/// The per-file attachment cap: positive, or the default when the table or key is absent.
/// A negative value fails in the TOML parse; zero fails here.
fn attachments_max_bytes(raw: Option<AttachmentsConfig>) -> Result<u64> {
    match raw.and_then(|table| table.max_bytes) {
        None => Ok(crate::attachments::DEFAULT_MAX_BYTES),
        Some(0) => Err(Error::Config(format!(
            "{CONFIG_REL}: [attachments] max_bytes must be a positive integer"
        ))),
        Some(bytes) => Ok(bytes),
    }
}
```

- In `Project::init`'s `Config { .. }` literal, add `attachments: None,`.
- In `Project::open`'s `Project { .. }` literal, add `attachments_max_bytes: attachments_max_bytes(config.attachments)?,`.

- [ ] **Step 7: Add the output types**

In `src/output.rs`, after `DeferredInfo` (anywhere among the payload structs):

```rust
/// One attachment as `show`, `next`, and `attach` report it. `path` is absolute.
#[derive(Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Serialize)]
pub struct AttachOut {
    pub id: String,
    pub file: FileInfo,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DetachOut {
    pub id: String,
    pub name: String,
    pub path: String,
    /// False when the ledger alone recorded the detach: no file was present.
    pub removed: bool,
    pub warnings: Vec<String>,
}
```

Then make three edits in the same file:

- Add `Attach(AttachOut),` and `Detach(DetachOut),` to `enum Output`, after `Feedback`.
- In `pretty`, add:

```rust
        Output::Attach(o) => o.file.path.clone(),
        Output::Detach(o) if o.removed => format!(
            "removed {}; git history keeps committed bytes, uncommitted ones are gone",
            o.path
        ),
        Output::Detach(o) => format!("recorded {} as detached; no file was present", o.name),
```

- In `warnings_of`, add:

```rust
        Output::Attach(o) => o.warnings.clone(),
        Output::Detach(o) => o.warnings.clone(),
```

- [ ] **Step 8: Add the commands**

Create `src/commands/attach.rs`:

```rust
use super::{Ctx, append_note, load, owner_name, save};
use crate::attachments::{self, EntryState, Ledger};
use crate::error::{Error, Result};
use crate::output::{AttachOut, DetachOut, FileInfo, Output};
use std::path::{Path, PathBuf};

pub enum Source {
    Path(PathBuf),
    Stdin,
    Clipboard,
}

impl Source {
    pub fn from_args(source: Option<String>, clipboard: bool) -> Result<Source> {
        match (source, clipboard) {
            (None, true) => Ok(Source::Clipboard),
            (Some(source), false) if source == "-" => Ok(Source::Stdin),
            (Some(source), false) => Ok(Source::Path(source.into())),
            _ => Err(Error::Validation(
                "give exactly one of a path, `-`, or --clipboard".into(),
            )),
        }
    }
}

fn basename(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(String::from)
        .ok_or_else(|| {
            Error::InvalidAttachmentName(format!("{} has no usable file name; pass --name", path.display()))
        })
}

/// Spec, "attach": the file is written before the ledger, and a rerun with the same
/// bytes under the same name finishes an interrupted attach.
pub fn attach(
    mut ctx: Ctx,
    id: String,
    source: Source,
    name: Option<String>,
    caption: Option<String>,
) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    let max = ctx.project.attachments_max_bytes;
    // Storage safety comes before any read of the source.
    attachments::check_storage(&ctx.project, &task.id)?;
    let (name, bytes) = match source {
        Source::Path(path) => {
            let name = match name {
                Some(name) => name,
                None => basename(&path)?,
            };
            attachments::validate_name(&name)?;
            let file = std::fs::File::open(&path)
                .map_err(|error| Error::Io(format!("{}: {error}", path.display())))?;
            let bytes = attachments::read_capped(file, max, &path.display().to_string())
                .map_err(|error| match error {
                    Error::Io(detail) => Error::Io(format!("{}: {detail}", path.display())),
                    other => other,
                })?;
            (name, bytes)
        }
        Source::Stdin => {
            let name = name.ok_or_else(|| {
                Error::InvalidAttachmentName("an attachment from stdin needs --name".into())
            })?;
            attachments::validate_name(&name)?;
            (name, attachments::read_capped(std::io::stdin().lock(), max, "stdin")?)
        }
        Source::Clipboard => {
            let image = crate::clipboard::read(max)?;
            let name = name.unwrap_or_else(|| {
                crate::clipboard::default_name(&crate::time::now(), image.extension)
            });
            attachments::validate_name(&name)?;
            (name, image.bytes)
        }
    };
    let note = attachments::attached_note(&name, bytes.len() as u64, caption.as_deref());
    crate::format::validate_note_text(&note)?;

    let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
    let live = attachments::ledger(&task).get(&name) == Some(&Ledger::Attached);
    let state = attachments::entry_state(&target)?;
    if live && state != EntryState::Absent {
        return Err(Error::AttachmentExists(format!("{} already has {name}", task.id)));
    }
    let write = match state {
        EntryState::Absent => true,
        EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        EntryState::File(size) => {
            if size != bytes.len() as u64 || std::fs::read(&target)? != bytes {
                return Err(Error::AttachmentExists(format!(
                    "{} holds an unrecorded {name} with different bytes; detach it or choose another --name",
                    task.id
                )));
            }
            false // an interrupted attach left these bytes; this run adds only the note
        }
    };

    // Identity and the claim store resolve before any write, as for `note`.
    ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    let created_dir = if write {
        let created = attachments::ensure_task_dir(&ctx.project, &task.id)?;
        attachments::write_new(&target, &bytes)?;
        created
    } else {
        false
    };
    append_note(&mut task, &owner, &note)?;
    if let Err(error) = save(&mut ctx, &mut task) {
        if !write {
            return Err(error);
        }
        let mut cleanup = std::fs::remove_file(&target);
        if cleanup.is_ok() && created_dir {
            cleanup = std::fs::remove_dir(target.parent().expect("attachment has a directory"));
        }
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup) => error.with_suffix(&format!(
                "; also could not remove {}: {cleanup}",
                target.display()
            )),
        });
    }
    Ok(Output::Attach(AttachOut {
        id: task.id.to_string(),
        file: FileInfo {
            name,
            path: target.display().to_string(),
            bytes: bytes.len() as u64,
        },
        warnings: ctx.warnings,
    }))
}

/// Spec, "detach": the ledger is written before the file is deleted, and a rerun
/// finishes an interrupted detach without a second note.
pub fn detach(mut ctx: Ctx, id: String, name: String, why: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    attachments::validate_name(&name)?;
    if why.trim().is_empty() {
        return Err(Error::Validation("detach needs a reason".into()));
    }
    let note = attachments::detached_note(&name, &why);
    crate::format::validate_note_text(&note)?;
    let dir_exists = attachments::check_storage(&ctx.project, &task.id)?;
    let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
    let present = dir_exists
        && match attachments::entry_state(&target)? {
            EntryState::File(_) => true,
            EntryState::Absent => false,
            EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        };
    let recorded = attachments::ledger(&task).get(&name).copied();
    if !present && recorded != Some(Ledger::Attached) {
        return Err(Error::AttachmentMissing(format!("{} has no attachment {name}", task.id)));
    }
    ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    if recorded != Some(Ledger::Detached) {
        append_note(&mut task, &owner, &note)?;
        save(&mut ctx, &mut task)?;
    }
    if present {
        std::fs::remove_file(&target)?;
        let dir = target.parent().expect("attachment has a directory");
        if std::fs::read_dir(dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
        }
    }
    Ok(Output::Detach(DetachOut {
        id: task.id.to_string(),
        name,
        path: target.display().to_string(),
        removed: present,
        warnings: ctx.warnings,
    }))
}
```

- [ ] **Step 9: Wire the CLI**

In `src/cli.rs`, add after the `Note` variant:

```rust
    /// Copy a file into the task's tasks/files/<id>/ and record it in a ledger note.
    #[command(
        after_help = "Examples:\n  tasks attach sci-4f2a9c ~/Pictures/before.png --caption \"the stale row\"\n  grim - | tasks attach sci-4f2a9c - --name after.png\n  tasks attach sci-4f2a9c --clipboard"
    )]
    Attach {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// The file to copy, or `-` for stdin (which needs --name).
        #[arg(required_unless_present = "clipboard", conflicts_with = "clipboard")]
        source: Option<String>,
        /// Read an image from the Wayland clipboard through wl-paste.
        #[arg(long)]
        clipboard: bool,
        /// The stored name: [A-Za-z0-9._-], at most 128 bytes, not starting with '.'.
        #[arg(long)]
        name: Option<String>,
        /// One line saying what the file shows, kept in the ledger note.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Remove one attachment and record why. Git history keeps committed bytes; an
    /// uncommitted file is gone for good.
    Detach {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        name: String,
        why: String,
    },
```

In `src/commands/mod.rs`, add `pub mod attach;` to the module list, and add these arms to `run` after `Command::Note`:

```rust
        Command::Attach {
            id,
            source,
            clipboard,
            name,
            caption,
        } => attach::attach(
            open_id_write_ctx(dir, &id)?,
            id,
            attach::Source::from_args(source, clipboard)?,
            name,
            caption,
        ),
        Command::Detach { id, name, why } => {
            attach::detach(open_id_write_ctx(dir, &id)?, id, name, why)
        }
```

- [ ] **Step 10: Run the tests to verify they pass**

Run: `just test-fast attach`, then `just test-fast attachments`, then `just test-fast clipboard`, then `just test-fast config_rejects`.
Expected: all PASS. If `attach_removes_its_file_when_the_save_fails_and_keeps_an_earlier_one` fails with a different kind than `io`, read the error: the save's temp write into the read-only `tasks/` must be what fails. Do not loosen the assertion.

- [ ] **Step 11: Run the whole fast suite and the checks**

Run: `just test-fast` and then `just check`.
Expected: PASS, with no clippy warnings. The existing `Output` and `Error` matches are exhaustive, so a missed arm fails compilation, not a test.

- [ ] **Step 12: Commit**

```bash
git add src/attachments.rs src/clipboard.rs src/commands/attach.rs src/error.rs src/repo.rs \
  src/cli.rs src/commands/mod.rs src/output.rs src/main.rs tests/attachments.rs
tasks done tasks-483846 "attach and detach with symlink-safe storage, the ledger, the size cap, and clipboard input"
git add tasks/
git commit -m "feat(attach): store task files under tasks/files with a ledger"
```

---

### Task 2: The show/next files field and the check pass

**Files:**
- Modify: `src/attachments.rs` (audit), `src/commands/show.rs` (`describe`), `src/output.rs` (`ShowFields.files`, `show_text`), `src/commands/check.rs` (`run`), `tests/attachments.rs`

**Interfaces:**
- Consumes (from Task 1):
  - `attachments::{files_root, task_dir, dir_state, DirState, validate_name, ledger, Ledger, FILES_DIR}`
  - `Project::attachments_max_bytes`
  - `output::FileInfo`
- Produces:
  - `attachments::Severity { Error, Warning }`
  - `attachments::Problem { severity: Severity, kind: &'static str, id: Option<TaskId>, file: String, detail: String }`
  - `attachments::Attachment { name: String, path: PathBuf, bytes: u64 }`
  - `attachments::audit_task(&Project, &Task) -> Result<(Vec<Attachment>, Vec<Problem>)>`
  - `attachments::audit(&Project, &[Task]) -> Result<Vec<Problem>>`
  - `attachments::human_size(u64) -> String`
  - `ShowFields.files: Vec<FileInfo>`

- [ ] **Step 1: Write the failing tests**

Append to `tests/attachments.rs`:

```rust
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
    assert_eq!(env.json(&dir, &["next"])["next"]["files"][0]["name"], "a.png");
    let pretty = env.pretty(&dir, &["show", &id]);
    assert!(pretty.contains("# files"), "{pretty}");
    assert!(pretty.contains(&format!("{} (2 B)", stored(&dir, &id, "b.png").display())), "{pretty}");
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
        show["warnings"][0].as_str().unwrap().contains("attachment_invalid"),
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
    assert!(show["warnings"][0].as_str().unwrap().contains("attachment_unsafe"));
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
    std::fs::rename(stored(&dir, &loser, "b.png"), stored(&dir, &winner, "b.png")).unwrap();
    std::fs::remove_dir(dir.join("tasks/files").join(&loser)).unwrap();
    let mut errors = check_kinds(&env, &dir, "errors");
    errors.sort();
    assert_eq!(errors, ["attachment_missing", "attachment_unnoted"]);
    // Recovery step 1: move each name live only in the loser's ledger.
    std::fs::create_dir(dir.join("tasks/files").join(&loser)).unwrap();
    std::fs::rename(stored(&dir, &winner, "b.png"), stored(&dir, &loser, "b.png")).unwrap();
    assert!(check_kinds(&env, &dir, "errors").is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast show_and_next_list_files`, then `just test-fast check_reports_each`.
Expected: FAIL. `show` has no `files` key, and `check` reports no attachment findings.

- [ ] **Step 3: Add the audit to `src/attachments.rs`**

Append this above the `#[cfg(test)]` module:

```rust
/// `812 B`, `1.4 KiB`, `1.9 MiB`.
pub fn human_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KIB {
        format!("{bytes} B")
    } else if value < KIB * KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{:.1} MiB", value / (KIB * KIB))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One `check` finding about attachments, also shown as a `show` warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub severity: Severity,
    pub kind: &'static str,
    pub id: Option<TaskId>,
    /// Repo-relative, like every other finding's `file`.
    pub file: String,
    pub detail: String,
}

impl Problem {
    /// The form `show` warnings use: `file [kind] detail`, as `check --pretty` prints.
    pub fn line(&self) -> String {
        format!("{} [{}] {}", self.file, self.kind, self.detail)
    }
}

/// A valid regular file in a task's directory, noted or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

fn rel_root() -> String {
    format!("tasks/{FILES_DIR}")
}

/// What `show` lists for `task`, and every problem `check` would report about it,
/// including an unsafe `tasks/files`.
pub fn audit_task(project: &Project, task: &Task) -> Result<(Vec<Attachment>, Vec<Problem>)> {
    if let DirState::Unsafe(detail) = dir_state(&files_root(project))? {
        let problem = Problem {
            severity: Severity::Error,
            kind: "attachment_unsafe",
            id: None,
            file: rel_root(),
            detail,
        };
        return Ok((Vec::new(), vec![problem]));
    }
    audit_task_dir(project, task)
}

/// Every attachment problem in the project: each record's ledger against its directory,
/// then orphans under `tasks/files`.
pub fn audit(project: &Project, tasks: &[Task]) -> Result<Vec<Problem>> {
    let root = files_root(project);
    let root_state = dir_state(&root)?;
    if let DirState::Unsafe(detail) = root_state {
        return Ok(vec![Problem {
            severity: Severity::Error,
            kind: "attachment_unsafe",
            id: None,
            file: rel_root(),
            detail,
        }]);
    }
    let mut problems = Vec::new();
    for task in tasks {
        problems.extend(audit_task_dir(project, task)?.1);
    }
    if root_state == DirState::Directory {
        let mut entries: Vec<_> = std::fs::read_dir(&root)?.collect::<std::io::Result<_>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let raw = entry.file_name();
            let shown = format!("{}/{}", rel_root(), raw.to_string_lossy());
            // An entry named after a live record is that record's to judge: `audit_task_dir`
            // reports it as `attachment_unsafe` when it is not a real directory.
            let owner = raw
                .to_str()
                .and_then(|name| TaskId::parse(name).ok())
                .filter(|id| project.task_path(id).is_file());
            if owner.is_none() {
                problems.push(Problem {
                    severity: Severity::Error,
                    kind: "attachment_orphan",
                    id: None,
                    file: shown,
                    detail: format!(
                        "{} names no task record; attachments live in tasks/files/<id>/ beside tasks/<id>.md",
                        raw.to_string_lossy()
                    ),
                });
            }
        }
    }
    Ok(problems)
}

/// `task`'s own directory against its ledger; assumes `tasks/files` itself is safe.
fn audit_task_dir(project: &Project, task: &Task) -> Result<(Vec<Attachment>, Vec<Problem>)> {
    let dir = task_dir(project, &task.id);
    let rel = format!("{}/{}", rel_root(), task.id);
    let ledger = ledger(task);
    let mut files = Vec::new();
    let mut problems = Vec::new();
    let problem = |severity, kind, file: String, detail: String| Problem {
        severity,
        kind,
        id: Some(task.id.clone()),
        file,
        detail,
    };
    match dir_state(&dir)? {
        DirState::Unsafe(detail) => {
            problems.push(problem(Severity::Error, "attachment_unsafe", rel, detail));
            return Ok((files, problems));
        }
        DirState::Absent => {}
        DirState::Directory => {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)?.collect::<std::io::Result<_>>()?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let raw = entry.file_name();
                let shown = format!("{rel}/{}", raw.to_string_lossy());
                let meta = std::fs::symlink_metadata(entry.path())?;
                let name = raw.to_str().filter(|name| validate_name(name).is_ok());
                let Some(name) = name.filter(|_| meta.is_file()) else {
                    problems.push(problem(
                        Severity::Warning,
                        "attachment_invalid",
                        shown,
                        format!("a {} that is not an attachment", kind_of(&meta)),
                    ));
                    continue;
                };
                if meta.len() > project.attachments_max_bytes {
                    problems.push(problem(
                        Severity::Warning,
                        "attachment_too_large",
                        shown.clone(),
                        format!("{} bytes is above the {}-byte cap", meta.len(), project.attachments_max_bytes),
                    ));
                }
                if ledger.get(name) != Some(&Ledger::Attached) {
                    problems.push(problem(
                        Severity::Error,
                        "attachment_unnoted",
                        shown,
                        format!("{name} is not attached in {}'s notes; attach it again with the same bytes, or detach it", task.id),
                    ));
                }
                files.push(Attachment {
                    name: name.to_string(),
                    path: entry.path(),
                    bytes: meta.len(),
                });
            }
        }
    }
    for (name, entry) in &ledger {
        if *entry == Ledger::Attached && !files.iter().any(|file| &file.name == name) {
            problems.push(problem(
                Severity::Error,
                "attachment_missing",
                format!("{rel}/{name}"),
                format!("{}'s notes attach {name} but no file is there", task.id),
            ));
        }
    }
    Ok((files, problems))
}
```

Add this test to the module's `tests`:

```rust
    #[test]
    fn human_sizes() {
        assert_eq!(human_size(812), "812 B");
        assert_eq!(human_size(1434), "1.4 KiB");
        assert_eq!(human_size(2_000_000), "1.9 MiB");
    }
```

- [ ] **Step 4: Add `files` to `ShowFields` and render it**

In `src/output.rs`, add this to `ShowFields`, after `deferred`:

```rust
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileInfo>,
```

In `show_text`, add this after the `# children` block and before `# parked`:

```rust
    if !o.files.is_empty() {
        rendered.push_str("\n# files\n");
        for file in &o.files {
            rendered.push_str(&format!(
                "- {} ({})\n",
                file.path,
                crate::attachments::human_size(file.bytes)
            ));
        }
    }
```

In `src/commands/show.rs` `describe`, before the final `Ok(ShowFields { .. })`:

```rust
    let (attached, problems) = crate::attachments::audit_task(project, &task)?;
    warnings.extend(problems.iter().map(crate::attachments::Problem::line));
    let files = attached
        .into_iter()
        .map(|file| crate::output::FileInfo {
            name: file.name,
            path: file.path.display().to_string(),
            bytes: file.bytes,
        })
        .collect();
```

Then add `files,` to the `ShowFields` literal.

- [ ] **Step 5: Report the audit from `check`**

In `src/commands/check.rs` `run`, immediately before `Ok(Output::Check(CheckOut { errors, warnings }))`:

```rust
    for problem in crate::attachments::audit(&ctx.project, &tasks)? {
        let finding = Finding {
            id: problem.id.map(|id| id.to_string()),
            file: problem.file,
            kind: problem.kind.into(),
            detail: problem.detail,
        };
        match problem.severity {
            crate::attachments::Severity::Error => errors.push(finding),
            crate::attachments::Severity::Warning => warnings.push(finding),
        }
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `just test-fast show_and_next_list_files`, then `just test-fast check_`, then `just test-fast show_warns`, then `just test-fast collision`, then `just test-fast human_sizes`.
Expected: PASS.

- [ ] **Step 7: Run the whole fast suite and the checks**

Run: `just test-fast` and then `just check`.
Expected: PASS. `tasks check` on this repository stays clean, since it has no `tasks/files`.

- [ ] **Step 8: Commit**

```bash
git add src/attachments.rs src/commands/show.rs src/output.rs src/commands/check.rs tests/attachments.rs
tasks done tasks-53fa40 "show/next list attachments; check audits ledgers against directories"
git add tasks/
git commit -m "feat(attach): list attachments in show and audit them in check"
```

---

### Task 3: Rename moves attachment directories

**Files:**
- Modify:
  - `src/rename/inventory.rs` (`InventoryEntry`, `build`)
  - `src/rename/snapshot.rs` (`EntryState`, `Named`, `observe`)
  - `src/rename/classify.rs` (`classify`, `refusal`, tests)
  - `src/rename/mod.rs` (the entry loop)
  - `tests/attachments.rs`

**Interfaces:**
- Consumes (from Task 1): `attachments::{dir_state, DirState, files_root, task_dir, FILES_DIR}`, `Error::AttachmentUnsafe`
- Consumes (from Task 2): the `check_kinds` test helper in `tests/attachments.rs`, and the attachment check kinds its rename test expects to be clean. Task 3 therefore runs after Task 2.
- Produces:
  - `InventoryEntry.attachments: bool` (required in TOML)
  - `snapshot::AttachmentDirs { source: bool, dest: bool }` and `EntryState.dirs: AttachmentDirs`
  - `Named.target_dirs: usize`
  - the `TASKS_RENAME_STOP_AFTER` boundary `attachments:<hex>`

- [ ] **Step 1: Write the failing end-to-end tests**

Append to `tests/attachments.rs`:

```rust
fn git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

/// A committed project `dot` with one task carrying `a.png` and one without files.
fn committed_project(env: &mut TestEnv) -> (PathBuf, String, String) {
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let with = id_of(env.json(&dir, &["add", "With", "-p", "2"]));
    let without = id_of(env.json(&dir, &["add", "Without", "-p", "2"]));
    let source = write(&dir.join("scratch/a.png"), b"a");
    env.json(&dir, &["attach", &with, source.to_str().unwrap()]);
    std::fs::remove_dir_all(dir.join("scratch")).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    (dir, with, without)
}

fn renamed(id: &str) -> String {
    format!("dots-{}", id.split_once('-').unwrap().1)
}

fn hex(id: &str) -> &str {
    id.split_once('-').unwrap().1
}

fn explain_warning(env: &TestEnv, dir: &Path) -> String {
    let out = env.json(dir, &["rename", "dot", "dots", "--explain"]);
    assert_eq!(out["recovery"], "refuse", "{out}");
    out["warnings"][0].as_str().unwrap().to_string()
}

fn stop_after(env: &TestEnv, dir: &Path, boundary: &str) {
    let out = env
        .raw(dir)
        .env("TASKS_RENAME_STOP_AFTER", boundary)
        .args(["rename", "dot", "dots"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
}

#[test]
fn rename_moves_attachment_directories_with_their_records() {
    let mut env = TestEnv::new();
    let (dir, with, without) = committed_project(&mut env);
    env.json(&dir, &["rename", "dot", "dots"]);
    assert!(!dir.join("tasks/files").join(&with).exists());
    assert!(stored(&dir, &renamed(&with), "a.png").is_file());
    assert!(!dir.join("tasks/files").join(renamed(&without)).exists());
    assert_eq!(env.json(&dir, &["show", &renamed(&with)])["files"][0]["name"], "a.png");
    assert!(check_kinds(&env, &dir, "errors").is_empty());
}

#[test]
fn rename_resumes_between_the_directory_move_and_the_source_removal() {
    let mut env = TestEnv::new();
    let (dir, with, _) = committed_project(&mut env);
    stop_after(&env, &dir, &format!("attachments:{}", hex(&with)));
    assert!(dir.join(format!("tasks/{with}.md")).is_file(), "source record not yet removed");
    assert!(stored(&dir, &renamed(&with), "a.png").is_file(), "directory already moved");
    assert!(!dir.join("tasks/files").join(&with).exists());
    let out = env.json(&dir, &["rename", "dot", "dots"]);
    assert_eq!(out["recovery"], "resume_files");
    assert!(!dir.join(format!("tasks/{with}.md")).exists());
    assert!(stored(&dir, &renamed(&with), "a.png").is_file());
}

#[test]
fn rename_refuses_a_destination_orphan_up_front_as_r9() {
    let mut env = TestEnv::new();
    let (dir, _, without) = committed_project(&mut env);
    // Git does not track an empty directory, so the tree stays clean.
    std::fs::create_dir(dir.join("tasks/files").join(renamed(&without))).unwrap();
    assert!(explain_warning(&env, &dir).starts_with("R9:"));
    let out = env.cmd(&dir).args(["rename", "dot", "dots"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("R9"));
    assert!(!env.home.path().join(".local/state/tasks/rename/dot.toml").exists());
}

#[test]
fn rename_refuses_a_stray_under_tasks_files_as_r5_before_a_fresh_inventory() {
    // An empty directory for no record, then a regular file. Git does not see the empty
    // directory and the file is committed, so neither trips the uncommitted-changes
    // refusal first; each reaches the inventory build.
    for stray in ["dir", "file"] {
        let mut env = TestEnv::new();
        let (dir, with, _) = committed_project(&mut env);
        let path = dir.join("tasks/files/dot-ffffff");
        match stray {
            "dir" => std::fs::create_dir(&path).unwrap(),
            _ => {
                write(&path, b"x");
                git(&dir, &["add", "-A"]);
                git(&dir, &["commit", "-qm", "stray"]);
            }
        }
        let out = env.cmd(&dir).args(["rename", "dot", "dots"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{stray}: {out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("R5") && stderr.contains("dot-ffffff"), "{stray}: {stderr}");
        assert!(!env.home.path().join(".local/state/tasks/rename/dot.toml").exists());
        assert!(dir.join(format!("tasks/{with}.md")).is_file());
        assert!(stored(&dir, &with, "a.png").is_file());
        assert!(!dir.join(format!("tasks/{}.md", renamed(&with))).exists());
    }
}

#[test]
fn rename_refuses_r10_r11_and_an_r5_stray_directory_after_the_inventory() {
    // R10: the baseline had attachments, and they vanished from both sides.
    let mut env = TestEnv::new();
    let (dir, with, _) = committed_project(&mut env);
    stop_after(&env, &dir, "inventory");
    std::fs::remove_dir_all(dir.join("tasks/files").join(&with)).unwrap();
    assert!(explain_warning(&env, &dir).starts_with("R10:"));

    // R11: attachments appeared for a task whose baseline had none.
    let mut env = TestEnv::new();
    let (dir, _, without) = committed_project(&mut env);
    stop_after(&env, &dir, "inventory");
    std::fs::create_dir(dir.join("tasks/files").join(&without)).unwrap();
    assert!(explain_warning(&env, &dir).starts_with("R11:"));

    // R5: a directory under tasks/files for no inventoried task.
    let mut env = TestEnv::new();
    let (dir, _, _) = committed_project(&mut env);
    stop_after(&env, &dir, "inventory");
    std::fs::create_dir(dir.join("tasks/files/dot-ffffff")).unwrap();
    assert!(explain_warning(&env, &dir).starts_with("R5:"));
}

#[test]
fn an_inventory_without_the_attachments_baseline_fails_to_load() {
    let mut env = TestEnv::new();
    let (dir, _, _) = committed_project(&mut env);
    stop_after(&env, &dir, "inventory");
    let path = env.home.path().join(".local/state/tasks/rename/dot.toml");
    let text: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("attachments"))
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(&path, text).unwrap();
    let out = env.cmd(&dir).args(["rename", "dot", "dots"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("\"config\"") && stderr.contains("attachments"), "{stderr}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast rename_moves_attachment`, then `just test-fast rename_refuses_r10`, then `just test-fast rename_refuses_a_stray`.
Expected: FAIL. The directory stays under the old id, no R9–R11 exists, and a fresh rename moves records past the stray.

- [ ] **Step 3: Record the baseline in the inventory**

In `src/rename/inventory.rs`, add this to `InventoryEntry`:

```rust
    /// Whether `tasks/files/<source>-<hex>/` existed when the inventory was built. No serde
    /// default: an inventory from a binary that predates attachments fails to load rather
    /// than guessing (spec, "Rename").
    pub attachments: bool,
```

In `Inventory::build`, before the `for path in task_paths(..)` loop:

```rust
        if let crate::attachments::DirState::Unsafe(detail) =
            crate::attachments::dir_state(&crate::attachments::files_root(project))?
        {
            return Err(Error::AttachmentUnsafe(detail));
        }
```

Inside the loop, before `entries.push`:

```rust
            let attachments =
                match crate::attachments::dir_state(&crate::attachments::task_dir(project, &id))? {
                    crate::attachments::DirState::Absent => false,
                    crate::attachments::DirState::Directory => true,
                    crate::attachments::DirState::Unsafe(detail) => {
                        return Err(Error::AttachmentUnsafe(detail));
                    }
                };
```

Then change the push to `entries.push(InventoryEntry { hex: id.hex, from: .., to: .., attachments });`.

After the loop, before `entries.sort_by(..)`, refuse anything under `tasks/files` that is not an inventoried task's directory. `observe` records these as R5 strays only once an inventory exists, so without this a fresh rename would save its inventory and move records past a stray the interrupted rename would refuse. It runs before `inventory.save()`, the first mutation, and uses the existing R5 wording:

```rust
        let files_root = crate::attachments::files_root(project);
        if crate::attachments::dir_state(&files_root)? == crate::attachments::DirState::Directory {
            let inventoried: BTreeSet<String> = entries
                .iter()
                .map(|entry| format!("{}-{}", project.prefix, entry.hex))
                .collect();
            let mut strays = Vec::new();
            for entry in std::fs::read_dir(&files_root)? {
                let entry = entry?;
                // `DirEntry::file_type` does not follow a symlink; an inventoried name that
                // is a symlink already failed the per-entry `dir_state` above.
                let owned = entry.file_type()?.is_dir()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| inventoried.contains(name));
                if !owned {
                    strays.push(entry.path());
                }
            }
            if !strays.is_empty() {
                strays.sort();
                return Err(Error::Validation(format!(
                    "R5: task files outside the inventory: {strays:?}"
                )));
            }
        }
```

`BTreeSet` is already imported in `inventory.rs`. A destination-prefix directory never reaches this walk: `classify` refuses it as R9 before the build.

- [ ] **Step 4: Observe the directories**

In `src/rename/snapshot.rs`, add after `EntryState`:

```rust
/// Which of `tasks/files/<source>-<hex>` and `tasks/files/<target>-<hex>` are directories,
/// read without following a symlink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttachmentDirs {
    pub source: bool,
    pub dest: bool,
}
```

Then:

- Add `pub dirs: AttachmentDirs,` to `EntryState`, and `pub target_dirs: usize,` to `Named`.
- In `observe`, set `dirs: AttachmentDirs::default()` in the `EntryState` literal.
- After the `for path in task_paths(..)` loop and before `strays.sort()`, add:

```rust
    let files_root = invocation
        .root
        .join("tasks")
        .join(crate::attachments::FILES_DIR);
    match crate::attachments::dir_state(&files_root)? {
        crate::attachments::DirState::Absent => {}
        crate::attachments::DirState::Unsafe(detail) => {
            return Err(Error::AttachmentUnsafe(detail));
        }
        crate::attachments::DirState::Directory => {
            for entry in std::fs::read_dir(&files_root)? {
                let entry = entry?;
                // `DirEntry::file_type` does not follow a symlink.
                let is_dir = entry.file_type()?.is_dir();
                let id = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| TaskId::parse(name).ok())
                    .filter(|_| is_dir);
                if let Some(id) = &id
                    && id.prefix == invocation.target
                {
                    named.target_dirs += 1;
                }
                let index = id
                    .as_ref()
                    .filter(|id| id.prefix == invocation.source || id.prefix == invocation.target)
                    .and_then(|id| by_hex.get(id.hex.as_str()).copied());
                match (index, id) {
                    (Some(index), Some(id)) if id.prefix == invocation.source => {
                        entries[index].dirs.source = true;
                    }
                    (Some(index), Some(_)) => entries[index].dirs.dest = true,
                    _ if inventory.is_some() => strays.push(entry.path()),
                    _ => {}
                }
            }
        }
    }
```

- [ ] **Step 5: Classify the directory states**

In `src/rename/classify.rs`:

1. In `classify`'s `None` branch, right after the existing `snap.named.target != 0` refusal:

```rust
                if snap.named.target_dirs != 0 {
                    return Recovery::Refuse(format!(
                        "R9: {} destination attachment directories with prefix {:?} already exist",
                        snap.named.target_dirs, invocation.target
                    ));
                }
```

2. In the `Some(inventory)` branch, make `files_done` also require that no source directory remains:

```rust
            let files_done = snap.entries.iter().all(|entry| {
                matches!(entry.source, FileState::Absent)
                    && matches!(entry.dest, FileState::Present(_))
                    && !entry.dirs.source
            });
```

3. In `refusal`, after the R4 loop and before the R5 stray check:

```rust
        for entry in &snap.entries {
            let had = baseline[&entry.hex].attachments;
            let source = format!("tasks/files/{}-{}", inventory.source, entry.hex);
            let dest = format!("tasks/files/{}-{}", inventory.target, entry.hex);
            match (had, entry.dirs.source, entry.dirs.dest) {
                (true, true, true) | (false, _, true) => {
                    return Some(format!("R9: unexpected destination attachments {dest}"));
                }
                (true, false, false) => {
                    return Some(format!(
                        "R10: attachments of {}-{} are missing from both {source} and {dest}",
                        inventory.source, entry.hex
                    ));
                }
                (false, true, false) => {
                    return Some(format!(
                        "R11: source attachments {source} appeared after the inventory"
                    ));
                }
                _ => {}
            }
        }
```

4. Update the test module:
   - Every `InventoryEntry { .. }` literal gains `attachments: false`: in `baseline`, and in case 6 of `enumerate_snapshots`.
   - Every `EntryState { .. }` literal gains `dirs: AttachmentDirs::default()`: in `inventory_only`, and in the `enumerate_snapshots` map.
   - Every `Named { source, target }` literal gains `target_dirs: 0`.
   - Import `AttachmentDirs` from `crate::rename::snapshot`.
5. In `each_refusal_fires_on_its_own_before_the_table`:
   - Loop `for rule in 1..=11`.
   - Add these match arms:

     ```rust
                9 => snap.entries[0].dirs.dest = true,
                10 => snap.inventory.as_mut().unwrap().entries[0].attachments = true,
                11 => snap.entries[0].dirs.source = true,
     ```

   - Change `if rule >= 7` to `if (7..=8).contains(&rule)`.
6. In the oracle `expected`, inside the per-entry loop after `done &= (source, dest) == (0, 1);`:

```rust
                    let dirs = (record.attachments, entry.dirs.source, entry.dirs.dest);
                    excluded |=
                        !matches!(dirs, (true, true, false) | (true, false, true) | (false, false, false));
                    done &= !entry.dirs.source;
```

   Also change the `Fresh` row to `(false, 1, 1) if snap.named.target == 0 && snap.named.target_dirs == 0 => Verdict::Fresh`.
7. Add a fast, non-ignored test beside the exhaustive one. The exhaustive enumeration keeps directories neutral; this covers every directory state against two file-pass stages:

```rust
    #[test]
    fn every_attachment_directory_state_gets_the_verdict_the_spec_names() {
        let mut counts = BTreeMap::new();
        for files in 0..81usize {
            for dirs in 0..64usize {
                for stage in 0..2 {
                    let mut snap = inventory_only();
                    let (mut file_digits, mut dir_digits) = (files, dirs);
                    for index in 0..2 {
                        let record = snap.inventory.as_ref().unwrap().entries[index].clone();
                        let pair = file_digits % 9;
                        file_digits /= 9;
                        snap.entries[index].source = match pair % 3 {
                            0 => FileState::Absent,
                            1 => FileState::Present(record.from.clone()),
                            _ => FileState::Present("edited".into()),
                        };
                        snap.entries[index].dest = match pair / 3 {
                            0 => FileState::Absent,
                            1 => FileState::Present(record.to.clone()),
                            _ => FileState::Present("conflict".into()),
                        };
                        let bits = dir_digits % 8;
                        dir_digits /= 8;
                        snap.inventory.as_mut().unwrap().entries[index].attachments = bits & 1 != 0;
                        snap.entries[index].dirs = AttachmentDirs {
                            source: bits & 2 != 0,
                            dest: bits & 4 != 0,
                        };
                    }
                    if stage == 1 {
                        snap.config = Some(ConfigState {
                            prefix: "new".into(),
                            digest: "config after".into(),
                        });
                    }
                    let got = verdict(classify(&snap));
                    assert_eq!(got, expected(&snap), "snapshot {snap:?}");
                    *counts.entry(got).or_insert(0usize) += 1;
                }
            }
        }
        for verdict in [Verdict::ResumeFiles, Verdict::ResumeRegistry, Verdict::Refuse] {
            assert!(counts.contains_key(&verdict), "{verdict:?} unreached: {counts:?}");
        }
        let mut fresh = inventory_only();
        fresh.inventory = None;
        fresh.entries.clear();
        assert_eq!(classify(&fresh), Recovery::Fresh);
        fresh.named.target_dirs = 1;
        let Recovery::Refuse(reason) = classify(&fresh) else {
            panic!("a destination attachment directory must refuse a fresh rename");
        };
        assert!(reason.starts_with("R9:"), "{reason}");
    }
```

- [ ] **Step 6: Move the directory in the entry loop**

In `src/rename/mod.rs`:

1. Import `InventoryEntry`: `use inventory::{Inventory, InventoryEntry, digest, rewrite_config_prefix};`.
2. Add this function after `verify_bytes`:

```rust
/// Moves `tasks/files/<source>-<hex>` to the target name when the baseline says it
/// exists and the move is pending; a settled pair is left alone. Any other state
/// disagrees with the inventory, which classification already refused.
fn finish_attachment_move(
    project: &Project,
    invocation: &Invocation,
    entry: &InventoryEntry,
) -> Result<()> {
    let root = crate::attachments::files_root(project);
    let source = root.join(format!("{}-{}", invocation.source, entry.hex));
    let dest = root.join(format!("{}-{}", invocation.target, entry.hex));
    let present = |path: &std::path::Path| -> Result<bool> {
        match crate::attachments::dir_state(path)? {
            crate::attachments::DirState::Absent => Ok(false),
            crate::attachments::DirState::Directory => Ok(true),
            crate::attachments::DirState::Unsafe(detail) => Err(Error::AttachmentUnsafe(detail)),
        }
    };
    match (entry.attachments, present(&source)?, present(&dest)?) {
        (false, false, false) | (true, false, true) => Ok(()),
        (true, true, false) => Ok(std::fs::rename(&source, &dest)?),
        (baseline, source, dest) => Err(Error::Validation(format!(
            "attachment directories for {} disagree with the inventory: baseline {baseline}, source {source}, destination {dest}",
            entry.hex
        ))),
    }
}
```

3. In the entry loop's `NotFound` arm, change the body to:

```rust
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    verify_bytes(&dest, &entry.to)?;
                    finish_attachment_move(&project, invocation, entry)?;
                    continue;
                }
```

4. Directly before `// A digest never authorizes deleting the only surviving unverified copy.`:

```rust
            finish_attachment_move(&project, invocation, entry)?;
            if stop_after(&format!("attachments:{}", entry.hex)) {
                return Ok(out);
            }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `just test-fast rename_`, then `just test-fast every_attachment_directory_state`, then `just test-fast each_refusal`, then `just test-fast inventory`.
Expected: PASS, and the existing rename tests are unchanged.

- [ ] **Step 8: Run the full suite with the exhaustive test**

Run: `just test`.
Expected: PASS, including `every_enumerated_snapshot_gets_the_verdict_the_spec_names` at its existing count of 1,572,480. The enumeration's directory states are neutral, so the count does not change.

- [ ] **Step 9: Commit**

```bash
git add src/rename tests/attachments.rs
tasks done tasks-e38ccd "rename moves attachment directories with an inventory baseline and R9-R11"
git add tasks/
git commit -m "feat(rename): move attachment directories with their records"
```

---

### Task 4: README, skill, and agent guide

**Files:**
- Modify: `README.md` (`## Use`, the rename paragraph), `skills/tasks/SKILL.md` (Task operations, id-collision recovery), `AGENTS.md` (Layout)

**Interfaces:**
- Consumes: the commands, finding kinds, and refusal codes from Tasks 1–3, exactly as named there.
- Produces: documentation only.

- [ ] **Step 1: Document the commands in the README**

In `README.md` `## Use`, after the `tasks note sci-4f2a9c "spec §4 no longer holds"` line:

```
    tasks attach sci-4f2a9c ~/Pictures/before.png --caption "the stale row"  # copy into tasks/files/<id>/
    tasks attach sci-4f2a9c --clipboard       # an image from wl-paste; --name to choose the name
    tasks detach sci-4f2a9c before.png "wrong screenshot"  # recorded; uncommitted bytes are gone
```

After the paragraph that ends "goals cannot recur.", add:

```markdown
Attachments live in `tasks/files/<id>/`, one directory per task, committed with the
record. Each file is capped at 2 MiB unless `tasks/.config.toml` sets
`[attachments] max_bytes`. A task's own `attached: <name> (<n> bytes)` and
`detached: <name>: <why>` notes are the ledger of which files it owns. `show` lists
the files with absolute paths. `check` reports these kinds:

- errors: `attachment_unsafe`, `attachment_orphan`, `attachment_unnoted`, and
  `attachment_missing`;
- warnings: `attachment_invalid` and `attachment_too_large`.

`tasks/files` and `tasks/files/<id>` must be real directories, not symlinks.
Binaries older than attachments ignore `tasks/files/` entirely. `feedback` never
carries attachments.

An interrupted `attach` leaves a file that `check` reports as `attachment_unnoted`.
Rerun it with the same bytes and `--name <name>` to finish, or `detach` the file. An
interrupted `detach` finishes when rerun.
```

In the rename paragraph that begins "Rename requires clean `tasks/`", append:

```markdown
Attachment directories move with their records. The inventory records which tasks had
attachments, and recovery refuses R9 (unexpected destination attachments), R10
(attachments missing from both sides), or R11 (source attachments appeared after the
inventory).

An inventory written by a binary older than attachments fails to load. Finish that
rename with the older binary, then `git mv tasks/files/<old>-<hex> tasks/files/<new>-<hex>`
for each task directory.
```

- [ ] **Step 2: Update the skill**

In `skills/tasks/SKILL.md` **Task operations**, after the `--source <ref>` sentence in the paragraph above that list, add this bullet to the list:

```markdown
- A screenshot or file for a task: `tasks attach <id> <path>` (or `-` with `--name`, or
  `--clipboard`), with `--caption "<what it shows>"`. `tasks show <id>` prints each
  file's absolute path; read the image from there. `tasks detach <id> <name> "<why>"`
  removes one. Never copy files into `tasks/files/` by hand: the record's notes are the
  ledger, and `check` fails on a file no ledger lists. `feedback` takes no attachments.
```

Replace the id-collision bullet with:

```markdown
- Id collision after a merge (git add/add conflict on the same `tasks/<id>.md`): keep one
  file, rename the other to a fresh id, fix its `id` field, then run `tasks check` and
  repair any `depends` it reports. If `tasks/files/<id>/` exists, split it by ledger
  before `git add` resolves the conflict:
  1. For each name live only in the loser's notes, move `tasks/files/<id>/<name>` to
     `tasks/files/<new>/<name>`.
  2. For a name both records attach, the record at `git show :2:tasks/<id>.md` came from
     the `HEAD` side, and the one at `:3:` from the merged side. Write each side's
     `git show :<stage>:tasks/files/<id>/<name>` into its own record's directory.
  3. Before staging, confirm `git hash-object <file>` equals
     `git rev-parse :<stage>:tasks/files/<id>/<name>`.
  4. Identical bytes (stage 0 only) are copied to both directories. If the merge is
     already committed, use `<merge>^1:` and `<merge>^2:` in place of `:2:` and `:3:`.

  `check` catches a file left under the wrong id, but not swapped bytes under a shared
  name.
```

- [ ] **Step 3: Update the agent guide's layout**

In `AGENTS.md` **Layout**, add this to the `src/` bullet, after the `claims.rs` clause:

```
`attachments.rs` (tasks/files/<id>/ storage, the attached:/detached: ledger, and the audit behind show and check),
`clipboard.rs` (wl-paste input for attach),
```

and add this after the `tests/cli.rs` bullet:

```markdown
- `tests/attachments.rs` — end-to-end tests for attach, detach, show, check, and rename of task files.
```

- [ ] **Step 4: Verify the documented commands match the binary**

Run: `cargo run -q -- attach --help` and `cargo run -q -- detach --help`, plus `just check`.
Expected: the flags printed match the README lines (`--clipboard`, `--name`, `--caption`, the positional `why`), and the check is clean.

- [ ] **Step 5: Commit**

```bash
git add README.md skills/tasks/SKILL.md AGENTS.md
tasks done tasks-061594 "README, skill, and agent guide document attachments"
git add tasks/
git commit -m "docs: document task attachments"
```
