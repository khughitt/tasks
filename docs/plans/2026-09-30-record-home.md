# Record Home Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop task records diverging across checkouts. A write from a copy behind another
checkout's refuses with `stale_copy` and says what to do. A claim follows the checkout its
holder writes in. `show` finds a worktree-only task through its claim or park.

**Architecture:** `src/stale.rs` compares the loaded copy with its siblings and formats
the refusal. `commands::refuse_stale_copy` picks the remedy, because only it can read the
claim store and the caller's identity. The write commands' loader (`commands::load`), the
`$EDITOR` path, and feedback's `recur_into` call it under the mutation lock before any other
input is read. A helper, `commands::follow_holder`, moves the holder's claim after a save.
`show` falls back to `parked::open_recorded`, the park design §5.3 resolution that `prime`
already uses.

**Tech Stack:** Rust (clap, serde, thiserror), and end-to-end tests through `assert_cmd` in
`tests/cli.rs` against git worktrees in temp dirs.

**Spec:** `docs/specs/2026-09-30-record-home-design.md` (round 5 accept, 2026-09-30).

## Global Constraints

- JSON output is the contract, and no shapes change. The only addition is the error kind
  `stale_copy`, which uses the existing object `{"error": {"kind", "detail"}}` and exits 1.
- There is no override flag, and a write is never routed to another checkout on its own.
- A sibling copy that cannot be read, or a git failure listing worktrees, is a warning,
  never a refusal.
- Only a strictly newer stamp refuses, or an equal stamp with different bytes. A sibling
  that is behind is never compared by content.
- The remedy is chosen by first match (spec §3.2): another session works there, then same
  stamp, then a handoff from main, then the retry. A feedback recurrence never gets `-C`.
- Tests: `just test-one <runner args>` while working, `just test-fast` before each commit.
  Never run `cargo test` directly.
- Conventional commits. No AI attribution trailers.
- Each step task closes with `tasks done <id> "<what landed>"` in the same commit as its
  code, and `tasks check` runs before every commit.
- After the last task, `cargo install --path .`.

## Review Focus

- **Two newer siblings:** the newest is named and the other listed in `(also newer in: …)`
  (test in Task 2).
- **A worktree deleted from disk without `git worktree remove`:** it must not refuse or fail
  a write in main (test in Task 2).
- **Equal stamps with identical bytes:** the write succeeds (test in Task 2).
- **A retry argument containing a single quote or non-ASCII text:** it round-trips through a
  POSIX shell unchanged (tests in Tasks 1 and 2).
- **`show` of a task present locally while its claimed copy elsewhere is newer:** the local
  copy is shown with no fallback warning (test in Task 6).

---

### Task 1: The comparison and the refusal text

**Files:**
- Create: `src/stale.rs`
- Modify: `src/main.rs` (add `mod stale;` between `mod similarity;` and `mod style;`),
  `src/error.rs` (variant, `with_suffix`, `kind`), and `src/repo.rs` (`SiblingCopy::Found`
  gains `raw`)
- Test: unit tests in `src/stale.rs`

**Interfaces:**
- Produces: `crate::stale::Newer { root: PathBuf, theirs: String, same_stamp: bool, others: Vec<PathBuf> }`,
  `crate::stale::Remedy<'a> { Occupied(Vec<(String, String)>), Merge, Handoff(&'a [String]), Rerun(&'a [String]), New }`,
  `crate::stale::compare(project: &Project, id: &TaskId, loaded: &str, raw: &str) -> (Option<Newer>, Vec<String>)`,
  `crate::stale::refusal(id: &TaskId, newer: &Newer, ours: &str, remedy: Remedy) -> String`,
  `crate::stale::rerun_line(root: &Path, args: &[String]) -> String`,
  `Error::StaleCopy(String)` (kind `stale_copy`), and
  `SiblingCopy::Found { root: PathBuf, updated: String, raw: String }`.

- [ ] **Step 1: Write the failing unit tests**

Create `src/stale.rs` holding only this test module, and add `mod stale;` to `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    fn newer(same_stamp: bool, others: &[&str]) -> Newer {
        Newer {
            root: PathBuf::from("/wt"),
            theirs: "2026-09-07T10:00:00Z".into(),
            same_stamp,
            others: others.iter().map(PathBuf::from).collect(),
        }
    }

    fn id() -> TaskId {
        TaskId::parse("sci-1a2b3c").unwrap()
    }

    #[test]
    fn rerun_line_quotes_only_what_a_shell_would_split() {
        let line = rerun_line(
            Path::new("/w t"),
            &args(&["note", "sci-1", "it's done", "--pretty", ""]),
        );
        assert_eq!(line, r"tasks -C '/w t' note sci-1 'it'\''s done' --pretty ''");
    }

    #[test]
    fn rerun_line_drops_every_spelling_of_an_earlier_dash_c() {
        for given in [
            &["-C", "/old", "note", "sci-1", "x"][..],
            &["-C/old", "note", "sci-1", "x"],
            &["-C=/old", "note", "sci-1", "x"],
        ] {
            assert_eq!(
                rerun_line(Path::new("/new"), &args(given)),
                "tasks -C /new note sci-1 x"
            );
        }
    }

    #[test]
    fn rerun_line_keeps_everything_after_a_double_dash() {
        assert_eq!(
            rerun_line(Path::new("/new"), &args(&["note", "sci-1", "--", "-C"])),
            "tasks -C /new note sci-1 -- -C"
        );
    }

    #[test]
    fn non_ascii_is_quoted() {
        assert_eq!(quote("naïve"), "'naïve'");
    }

    #[test]
    fn the_retry_remedy_names_the_command_and_stdin() {
        let given = args(&["attach", "sci-1a2b3c", "-", "--name", "x"]);
        assert_eq!(
            refusal(&id(), &newer(false, &["/b"]), "2026-09-05T09:00:00Z", Remedy::Rerun(&given)),
            "tasks/sci-1a2b3c.md in /wt is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here) (also newer in: /b); nothing was written. Run it \
             there: tasks -C /wt attach sci-1a2b3c - --name x; supply the same input on stdin"
        );
    }

    #[test]
    fn the_handoff_remedy_leads_with_the_merge() {
        let given = args(&["start", "sci-1a2b3c"]);
        assert_eq!(
            refusal(&id(), &newer(false, &[]), "2026-09-05T09:00:00Z", Remedy::Handoff(&given)),
            "tasks/sci-1a2b3c.md in /wt is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here); nothing was written. Commit tasks/sci-1a2b3c.md in \
             /wt if it has changes, merge it into this branch, then rerun here; the merge may \
             conflict where both copies changed. Or, to write in the main checkout instead: \
             tasks -C /wt start sci-1a2b3c"
        );
    }

    #[test]
    fn the_occupied_remedy_names_the_work_and_offers_no_retry() {
        let text = refusal(
            &id(),
            &newer(false, &[]),
            "2026-09-05T09:00:00Z",
            Remedy::Occupied(vec![("sci-999999".into(), "agent-b".into())]),
        );
        assert!(
            text.ends_with(
                "nothing was written. /wt is where another session works: sci-999999 \
                 (session agent-b). Wait for that branch to merge, or merge its copy of \
                 tasks/sci-1a2b3c.md into this checkout, then rerun here"
            ),
            "{text}"
        );
        assert!(!text.contains("tasks -C"), "{text}");
    }

    #[test]
    fn the_same_stamp_remedy_is_the_merge_alone() {
        assert_eq!(
            refusal(&id(), &newer(true, &[]), "2026-09-05T09:00:00Z", Remedy::Merge),
            "tasks/sci-1a2b3c.md in /wt has the same stamp as this copy \
             (2026-09-05T09:00:00Z) but different content, so both were written in the same \
             second; nothing was written. Merge that copy of tasks/sci-1a2b3c.md into this \
             checkout, then rerun here"
        );
    }

    #[test]
    fn the_feedback_remedy_offers_new() {
        assert!(
            refusal(&id(), &newer(false, &[]), "2026-09-05T09:00:00Z", Remedy::New)
                .ends_with("nothing was written. Rerun with --new to file a separate entry")
        );
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --bin tasks stale::`
Expected: compile errors for the missing `Newer`, `Remedy`, `refusal`, `rerun_line`, `quote`.

- [ ] **Step 3: Implement the module**

In `src/repo.rs`, give `SiblingCopy::Found` the bytes, and document the field:

```rust
    /// The project root the copy was found in, the `updated` stamp it carries there, and
    /// its bytes, which a same-stamp comparison needs (record-home spec §3.1).
    Found {
        root: PathBuf,
        updated: String,
        raw: String,
    },
```

In `sibling_task_copies`, build it with the bytes it already read:

```rust
            match parse_task(&raw, &file) {
                Ok(task) => copies.push(SiblingCopy::Found {
                    root,
                    updated: task.updated,
                    raw,
                }),
```

In `src/error.rs`, add `#[error("{0}")] StaleCopy(String),` after `Claimed`, then
`Error::StaleCopy(detail) => Error::StaleCopy(detail + suffix),` in `with_suffix`, and
`Error::StaleCopy(_) => "stale_copy",` in `kind`.

Above the test module in `src/stale.rs`:

```rust
//! A write from a copy that is behind another checkout's refuses (record-home spec §3).

use crate::model::TaskId;
use crate::repo::{Project, SiblingCopy};
use std::path::{Path, PathBuf};

/// The sibling copy a write must not leave behind.
pub struct Newer {
    pub root: PathBuf,
    /// Its `updated` stamp.
    pub theirs: String,
    /// The stamps are equal but the bytes differ: a same-second fork (§3.1).
    pub same_stamp: bool,
    /// Every other sibling that also refuses, named after `root`.
    pub others: Vec<PathBuf>,
}

/// What a refusal tells the caller to do (§3.2). The caller chooses it, because only the
/// caller can read the claim store and its own identity.
pub enum Remedy<'a> {
    /// Rule 1: another session works in the newer checkout, as `(task id, session)` pairs.
    Occupied(Vec<(String, String)>),
    /// Rule 2: the same stamp with different bytes.
    Merge,
    /// Rule 3: a linked worktree behind the main checkout; the arguments for the alternative.
    Handoff(&'a [String]),
    /// Rule 4: this invocation's arguments, rerun there.
    Rerun(&'a [String]),
    /// A feedback recurrence where rule 4 would apply: `-C` would change the reporter.
    New,
}

/// Compares the copy this command loaded (its `loaded` stamp and `raw` bytes) with every
/// other worktree's copy of `id`. `Some` proves the write must refuse: a strictly newer
/// stamp, or else the same stamp with different bytes. The warnings name what could not
/// be compared: an unreadable copy, or a failure to list the worktrees.
pub fn compare(
    project: &Project,
    id: &TaskId,
    loaded: &str,
    raw: &str,
) -> (Option<Newer>, Vec<String>) {
    let copies = match project.sibling_task_copies(id) {
        Ok(Some(copies)) => copies,
        Ok(None) => return (None, Vec::new()),
        Err(error) => {
            return (
                None,
                vec![format!(
                    "could not check other checkouts for a newer copy of {id} ({error})"
                )],
            );
        }
    };
    let mut warnings = Vec::new();
    let mut newer: Vec<(PathBuf, String)> = Vec::new();
    let mut forked: Vec<PathBuf> = Vec::new();
    for copy in copies {
        match copy {
            SiblingCopy::Found { root, updated, .. } if updated.as_str() > loaded => {
                newer.push((root, updated));
            }
            SiblingCopy::Found {
                root,
                updated,
                raw: theirs,
            } if updated == loaded && theirs != raw => forked.push(root),
            SiblingCopy::Found { .. } => {}
            SiblingCopy::Unreadable { root, detail } => warnings.push(format!(
                "tasks/{id}.md in {} could not be read ({detail}); whether that copy has \
                 diverged from this one is unknown",
                root.display()
            )),
        }
    }
    // Newest first, ties in path order, so the message is stable.
    newer.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    forked.sort();
    let found = if !newer.is_empty() {
        let (root, theirs) = newer.remove(0);
        Some(Newer {
            root,
            theirs,
            same_stamp: false,
            others: newer.into_iter().map(|(root, _)| root).collect(),
        })
    } else if !forked.is_empty() {
        let root = forked.remove(0);
        Some(Newer {
            root,
            theirs: loaded.to_string(),
            same_stamp: true,
            others: forked,
        })
    } else {
        None
    };
    (found, warnings)
}

/// The `stale_copy` detail (§3.2).
pub fn refusal(id: &TaskId, newer: &Newer, ours: &str, remedy: Remedy) -> String {
    let root = newer.root.display();
    let head = if newer.same_stamp {
        format!(
            "tasks/{id}.md in {root} has the same stamp as this copy ({ours}) but different \
             content, so both were written in the same second"
        )
    } else {
        format!(
            "tasks/{id}.md in {root} is newer than this copy ({} there, {ours} here)",
            newer.theirs
        )
    };
    let also = if newer.others.is_empty() {
        String::new()
    } else {
        let others: Vec<String> = newer
            .others
            .iter()
            .map(|root| root.display().to_string())
            .collect();
        format!(" (also newer in: {})", others.join(", "))
    };
    let next = match remedy {
        Remedy::Occupied(work) => {
            let work: Vec<String> = work
                .iter()
                .map(|(task, session)| format!("{task} (session {session})"))
                .collect();
            format!(
                "{root} is where another session works: {}. Wait for that branch to merge, \
                 or merge its copy of tasks/{id}.md into this checkout, then rerun here",
                work.join(", ")
            )
        }
        Remedy::Merge => {
            format!("Merge that copy of tasks/{id}.md into this checkout, then rerun here")
        }
        Remedy::Handoff(args) => format!(
            "Commit tasks/{id}.md in {root} if it has changes, merge it into this branch, then \
             rerun here; the merge may conflict where both copies changed. Or, to write in \
             the main checkout instead: {}{}",
            rerun_line(&newer.root, args),
            stdin_hint(args)
        ),
        Remedy::Rerun(args) => format!(
            "Run it there: {}{}",
            rerun_line(&newer.root, args),
            stdin_hint(args)
        ),
        Remedy::New => "Rerun with --new to file a separate entry".to_string(),
    };
    format!("{head}{also}; nothing was written. {next}")
}

/// `-` is how every command here names stdin, which a printed line cannot carry.
fn stdin_hint(args: &[String]) -> &'static str {
    if args.iter().any(|arg| arg == "-") {
        "; supply the same input on stdin"
    } else {
        ""
    }
}

/// `tasks -C <root> <args>`, with any `-C` the invocation carried removed, quoted so the line
/// pastes into a POSIX shell. `-C` only chooses the project and the process keeps its
/// working directory, so a relative path in `args` resolves the same way in the retry.
pub fn rerun_line(root: &Path, args: &[String]) -> String {
    let mut words = vec![
        "tasks".to_string(),
        "-C".to_string(),
        quote(&root.display().to_string()),
    ];
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            words.push(quote(arg));
            words.extend(rest.by_ref().map(|arg| quote(arg)));
            break;
        }
        if arg == "-C" {
            rest.next();
            continue;
        }
        if arg.starts_with("-C") {
            continue;
        }
        words.push(quote(arg));
    }
    words.join(" ")
}

fn quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}
```

Nothing calls this module until Task 2, and `just check` (the pre-commit gate) runs clippy
with `-D warnings`. For this commit only, write the module line in `src/main.rs` as
`#[allow(dead_code)] mod stale;`, and put `#[allow(dead_code)]` on `Error::StaleCopy` and on
the new `raw` field. Task 2 removes all three attributes.

- [ ] **Step 4: Run the tests to see them pass**

Run: `just test-one --bin tasks stale::`, then `just test-fast`
Expected: 9 passed, then the whole suite passes. The existing sibling-warning code in
`commands/mod.rs` matches `SiblingCopy::Found { root, updated }`; change that pattern to
`{ root, updated, .. }` so it compiles.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-2cbfad "stale.rs: sibling comparison with the same-stamp content rule, and the refusal text for each remedy"
tasks check
git add src/stale.rs src/main.rs src/error.rs src/repo.rs src/commands/mod.rs tasks/
git commit -m "feat(stale): compare sibling copies and format stale_copy refusals"
```

### Task 2: Refuse at load, choosing the remedy

**Files:**
- Modify: `src/commands/mod.rs` (`load`, new `refuse_stale_copy`, `Writer`, `occupants`;
  `save` loses `warn_on_newer_sibling_copies`), `src/commands/edit.rs` (first check on the
  editor path), and every `load(&ctx, &id)` caller in `park.rs`, `edit.rs`, `attach.rs` (2),
  `dep.rs`, and `status.rs` (6)
- Test: `tests/cli.rs`. Convert `a_write_warns_when_another_checkout_holds_a_newer_copy`,
  `a_project_below_the_repository_root_finds_its_sibling_copies` and
  `a_hand_edited_updated_stamp_cannot_suppress_the_warning`, and add new tests.

**Interfaces:**
- Consumes: everything Task 1 produces; `Ctx::ownership(&mut self, &Claim, &Resolution) -> Result<Ownership>`;
  `crate::claims::resolve_identity(&mut Vec<String>) -> Resolution`; `Resolution::identity() -> Option<&Identity>`
  (`Identity { session, tagged, .. }`); `ClaimSnapshot::load`, `iter()`, `parks()`; and
  `Project::main_checkout_root() -> Result<Option<PathBuf>>`.
- Produces: `commands::load(ctx: &mut Ctx, id: &str) -> Result<Task>`,
  `commands::Writer { Command, Feedback }`,
  `commands::refuse_stale_copy(ctx: &mut Ctx, task: &Task, raw: &str, writer: Writer) -> Result<()>`.
  Test helpers: `stale_detail`, `retry_words`, `started_then_branched`.

- [ ] **Step 1: Write the failing tests**

In `tests/cli.rs`, after `warnings_of`, add the helpers:

```rust
/// The `detail` of a `stale_copy` refusal from running `args` in `dir`.
fn stale_detail(env: &TestEnv, dir: &std::path::Path, args: &[&str]) -> String {
    let error = error_of(env, dir, args);
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    error["error"]["detail"].as_str().unwrap().to_string()
}

/// The `tasks -C …` line printed in a refusal's detail, split into words the way a POSIX
/// shell splits it, without the program name.
fn retry_words(detail: &str) -> Vec<String> {
    let at = detail.find("tasks -C ").expect("a retry");
    let line = &detail[at..];
    let line = line
        .strip_suffix("; supply the same input on stdin")
        .unwrap_or(line);
    let rest = line.strip_prefix("tasks ").unwrap();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("for word in {rest}; do printf '%s\\0' \"$word\"; done"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout)
        .unwrap()
        .split_terminator('\0')
        .map(str::to_string)
        .collect()
}

/// A task started by `agent-a` in the main checkout, committed, and only then branched into
/// a worktree (the prescribed order). Both copies are stamped equal and old, so the next
/// write in either checkout is the newer one.
fn started_then_branched(
    env: &mut TestEnv,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    as_agent(env, &main, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "start"]);
    let side = main.join("wt");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()],
    );
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-02T00:00:00Z");
    }
    (main, side, id)
}
```

Replace `a_write_warns_when_another_checkout_holds_a_newer_copy` with:

```rust
#[test]
fn a_write_from_a_copy_behind_another_checkout_refuses_and_prints_the_retry() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    // Explicit stamps: `updated` has second precision, and a real-clock race would make
    // the test flaky rather than wrong.
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let file = format!("tasks/{id}.md");
    let (main_before, side_before) = (env.read(&main, &file), env.read(&side, &file));

    for args in [
        vec!["note", id.as_str(), "it's naïve"],
        vec!["edit", id.as_str(), "-p", "1"],
        vec!["start", id.as_str()],
        vec!["done", id.as_str(), "landed"],
    ] {
        let detail = stale_detail(&env, &main, &args);
        let head = format!(
            "tasks/{id}.md in {} is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here); nothing was written. Run it there: ",
            side.display()
        );
        assert!(detail.starts_with(&head), "{detail}");
        let mut expected = vec!["-C".to_string(), side.display().to_string()];
        expected.extend(args.iter().map(|arg| arg.to_string()));
        assert_eq!(retry_words(&detail), expected);
    }
    assert_eq!(env.read(&main, &file), main_before);
    assert_eq!(env.read(&side, &file), side_before);
    let store = env.claim_store("sci");
    assert!(!store.exists() || !std::fs::read_to_string(&store).unwrap().contains(&id));

    // An invocation that already named a checkout gets that `-C` replaced, not doubled.
    let detail = stale_detail(&env, &side, &["-C", main.to_str().unwrap(), "note", &id, "x"]);
    assert_eq!(
        retry_words(&detail),
        ["-C", side.to_str().unwrap(), "note", &id, "x"]
    );

    // The printed retry, run verbatim from where it was refused, lands in the newer copy.
    let detail = stale_detail(&env, &main, &["note", &id, "it's naïve"]);
    env.cmd(&main).args(retry_words(&detail)).assert().success();
    assert!(env.read(&side, &file).contains("it's naïve"));
}

#[test]
fn a_linked_worktree_behind_the_main_checkout_leads_with_the_merge() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    let detail = stale_detail(&env, &side, &["note", &id, "x"]);
    let remedy = format!(
        "nothing was written. Commit tasks/{id}.md in {main} if it has changes, merge it into \
         this branch, then rerun here; the merge may conflict where both copies changed. Or, \
         to write in the main checkout instead: tasks -C {main} note {id} x",
        main = main.display()
    );
    assert!(detail.ends_with(&remedy), "{detail}");
}

#[test]
fn another_sessions_work_in_the_newer_checkout_withholds_the_retry() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let other = id_of(env.json(&side, &["add", "Other work", "-p", "2"]));
    as_agent(&env, &side, "agent-b")
        .args(["start", &other])
        .assert()
        .success();
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");

    // Someone else's claim, on a different task, names the newer checkout.
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert!(detail.contains(&other) && detail.contains("agent-b"), "{detail}");
    assert!(detail.contains("Wait for that branch to merge"), "{detail}");
    assert!(!detail.contains("tasks -C"), "{detail}");

    // The claim's own holder, whose shell reset to main, keeps the retry.
    let out = as_agent(&env, &main, "agent-b")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(
        error["error"]["detail"].as_str().unwrap().contains("Run it there: tasks -C"),
        "{error}"
    );

    // A park by another session withholds it too.
    as_agent(&env, &side, "agent-b")
        .args(["park", &other, "later"])
        .assert()
        .success();
    let out = as_agent(&env, &main, "agent-c")
        .args(["note", &id, "x"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains(&other) && !detail.contains("tasks -C"), "{detail}");
}

#[test]
fn equal_stamps_with_different_bytes_refuse_with_the_merge_only() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    let sibling = side.join(format!("tasks/{id}.md"));
    let forked = std::fs::read_to_string(&sibling)
        .unwrap()
        .replace("title: T\n", "title: Forked\n");
    std::fs::write(&sibling, forked).unwrap();
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert_eq!(
        detail,
        format!(
            "tasks/{id}.md in {} has the same stamp as this copy (2026-09-05T09:00:00Z) but \
             different content, so both were written in the same second; nothing was \
             written. Merge that copy of tasks/{id}.md into this checkout, then rerun here",
            side.display()
        )
    );
}

#[test]
fn the_first_write_in_a_worktree_behind_main_leads_with_the_merge() {
    // tasks-142d2f without the protocol step: a note in main after branching, then a write
    // in the worktree by the claim's own holder.
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &main, "agent-a")
        .args(["note", &id, "from main"])
        .assert()
        .success();
    let out = as_agent(&env, &side, "agent-a")
        .args(["note", &id, "here"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!("nothing was written. Commit tasks/{id}.md in {}", main.display())),
        "{detail}"
    );
}

#[test]
fn a_start_left_uncommitted_before_branching_leads_with_the_merge() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    as_agent(&env, &main, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let side = main.join("wt");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()],
    );
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-01T00:00:00Z");
    let out = as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!("nothing was written. Commit tasks/{id}.md in {}", main.display())),
        "{detail}"
    );
    assert!(!detail.contains("Run it there"), "{detail}");
}
```

In `a_project_below_the_repository_root_finds_its_sibling_copies`, replace everything from
`let v = env.json(&side.join("sub"), …` to the end of the test with:

```rust
    let detail = stale_detail(&env, &side.join("sub"), &["note", &id, "in the worktree"]);
    let expected = format!("tasks/{id}.md in {} is newer", sub.display());
    assert!(detail.starts_with(&expected), "{detail}");
```

Replace `a_hand_edited_updated_stamp_cannot_suppress_the_warning` with:

```rust
#[test]
fn the_editor_does_not_open_on_a_copy_that_is_behind() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let opened = main.join("editor-opened");
    let editor = editor_script(&main, &format!("touch '{}'", opened.display()));
    let out = env
        .cmd(&main)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    assert!(!opened.exists(), "the editor opened on a stale copy");
}
```

The Review Focus tests:

```rust
#[test]
fn a_refusal_names_the_newest_sibling_and_lists_the_other_newer_ones() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    let third = main.join("wt2");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "third", third.to_str().unwrap()],
    );
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&third, &id, "2026-09-01T00:00:00Z", "2026-09-06T10:00:00Z");
    let detail = stale_detail(&env, &main, &["note", &id, "x"]);
    assert!(
        detail.starts_with(&format!("tasks/{id}.md in {} is newer", side.display())),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("(also newer in: {})", third.display())),
        "{detail}"
    );
}

#[test]
fn a_worktree_deleted_from_disk_does_not_block_a_write() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2030-01-01T00:00:00Z");
    // Gone from disk but still listed by git: nothing there to be newer.
    std::fs::remove_dir_all(&side).unwrap();
    env.json(&main, &["note", &id, "still lands"]);
    assert!(env.read(&main, &format!("tasks/{id}.md")).contains("still lands"));
}

#[test]
fn equal_stamps_with_equal_bytes_do_not_refuse() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    let v = env.json(&main, &["note", &id, "same record"]);
    assert!(!warnings_of(&v).iter().any(|w| w.contains("newer")), "{v}");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --test cli copy_behind_another_checkout`
Expected: FAIL. The note exits 0 and prints the old `is newer` warning.

- [ ] **Step 3: Choose the remedy and refuse at load**

In `src/commands/mod.rs`, delete `warn_on_newer_sibling_copies` and its doc comment, drop
`SiblingCopy` from the `use crate::repo::…` line, and start `save` with:

```rust
pub fn save(ctx: &mut Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    ctx.project.validate_docs(task)?;
    crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, task)?;
    let clear_escalation = std::mem::take(&mut ctx.clear_escalation);
```

Replace `load` and add the rest:

```rust
/// Reads `id` for a write and refuses when another checkout's copy must not be left behind
/// (record-home spec §3.1). Every write command loads through here, under the mutation lock
/// and before it reads any other input, so a refusal consumes nothing and creates nothing.
pub fn load(ctx: &mut Ctx, id: &str) -> Result<Task> {
    let (task, raw) = ctx
        .project
        .read_task_with_raw(&parse_id(&ctx.registry, id)?)?;
    refuse_stale_copy(ctx, &task, &raw, Writer::Command)?;
    Ok(task)
}

/// Who is writing, which decides whether a `-C` retry can be offered (spec §3.2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Writer {
    Command,
    /// A feedback recurrence: `-C` would change the reporting project.
    Feedback,
}

/// The record-home §3 check for `task` as loaded (`raw` is the bytes it was read from).
/// Its warnings are added once, however many times one command checks.
pub fn refuse_stale_copy(ctx: &mut Ctx, task: &Task, raw: &str, writer: Writer) -> Result<()> {
    let (newer, warnings) = crate::stale::compare(&ctx.project, &task.id, &task.updated, raw);
    for warning in warnings {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    let Some(newer) = newer else {
        return Ok(());
    };
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let (work, unknown) = match occupants(ctx, &newer.root) {
        Ok(work) => (work, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    // The sibling lookup just ran the same git query, so a failure here is not expected. If
    // it happens, only the handoff ordering is lost; the retry it falls back to is valid.
    let main = ctx.project.main_checkout_root().ok().flatten();
    let remedy = if !work.is_empty() {
        crate::stale::Remedy::Occupied(work)
    } else if newer.same_stamp {
        crate::stale::Remedy::Merge
    } else if writer == Writer::Feedback {
        crate::stale::Remedy::New
    } else if main.as_ref() == Some(&newer.root) {
        crate::stale::Remedy::Handoff(&args)
    } else {
        crate::stale::Remedy::Rerun(&args)
    };
    let mut detail = crate::stale::refusal(&task.id, &newer, &task.updated, remedy);
    if let Some(error) = unknown {
        // A refusal prints only its error object, so this cannot be a warning (spec §3.2).
        detail.push_str(&format!(
            " (whether another session works there is unknown: {error})"
        ));
    }
    Err(Error::StaleCopy(detail))
}

/// Spec §3.2 rule 1: every task that a session other than the caller holds live in `root`,
/// or parked there. Claims are compared by `Ctx::ownership`, parks by their tagged session.
/// An unresolvable identity makes every park someone else's.
fn occupants(ctx: &mut Ctx, root: &Path) -> Result<Vec<(String, String)>> {
    let snapshot =
        crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let mine = me.identity().map(|identity| identity.tagged.clone());
    let mut work = Vec::new();
    for (task, (claim, liveness)) in snapshot.iter() {
        if *liveness == crate::claims::Liveness::Live
            && Path::new(&claim.worktree) == root
            && ctx.ownership(claim, &me)? == Ownership::Foreign
        {
            work.push((task.clone(), claim.session.clone()));
        }
    }
    for (task, park) in snapshot.parks() {
        if Path::new(&park.worktree) == root && mine.as_deref() != Some(park.session.as_str()) {
            work.push((task.clone(), park.session.clone()));
        }
    }
    Ok(work)
}
```

Remove the three `#[allow(dead_code)]` attributes that Task 1 added (on `mod stale;`,
`Error::StaleCopy`, and `SiblingCopy::Found`'s `raw`). Make sure
`Path` is imported (`use std::path::Path;`) and that `Ownership` derives `PartialEq`, which
`status.rs` already relies on.

Change every caller:

```bash
sed -i 's/load(&ctx, &id)/load(\&mut ctx, \&id)/' src/commands/park.rs src/commands/edit.rs \
  src/commands/attach.rs src/commands/dep.rs src/commands/status.rs
```

In `src/commands/edit.rs` `editor`, check right after the read, before the temp file exists:

```rust
    let (original, original_raw) = ctx.project.read_task_with_raw(&id)?;
    super::refuse_stale_copy(&mut ctx, &original, &original_raw, super::Writer::Command)?;
```

Confirm every `save` caller loads through `load` or the editor path:

Run: `grep -n 'save(&mut ctx' src/commands/*.rs`
Expected: callers only in `status.rs`, `park.rs`, `edit.rs`, `dep.rs` and `attach.rs`.
Each takes its task from `load(&mut ctx, …)`, or in `editor` from `read_task_with_raw`
followed by the check.

- [ ] **Step 4: Run the tests to see them pass**

Run `just test-one --test cli <filter>` with each of these filters:
`copy_behind_another_checkout`, `leads_with_the_merge`, `withholds_the_retry`,
`different_bytes`, `sibling`, `editor_does_not_open`, `newest_sibling`,
`deleted_from_disk`, `equal_bytes`.
Expected: PASS. `a_sibling_copy_that_cannot_be_read_is_a_warning_and_a_missing_one_is_silent`,
`a_checkout_that_is_merely_behind_is_not_worth_a_warning` and
`a_failure_to_inspect_other_checkouts_is_a_warning_not_a_refusal` pass unchanged.

- [ ] **Step 5: Run the fast suite**

Run: `just test-fast`
Expected: PASS. If an older test wrote from a behind copy by accident, fix its setup (for
example, stamp both copies equal). Never loosen the rule.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-608fdb "stale_copy refusal at load with the §3.2 remedy ladder"
tasks check
git add src/stale.rs src/commands tests/cli.rs tasks/
git commit -m "feat(stale): refuse a write from a copy behind another checkout"
```

### Task 3: Input, attachments, and the editor

**Files:**
- Modify: `src/commands/edit.rs` (second check after `lock_and_revalidate` on the editor path)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `commands::refuse_stale_copy`, `commands::Writer`, `Error::with_suffix`, and the
  test helpers `stale_detail` and `retry_words` (Task 2).

- [ ] **Step 1: Write the tests**

```rust
#[test]
fn attach_from_a_copy_that_is_behind_reads_nothing_and_creates_nothing() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    std::fs::write(side.join("shot.png"), b"png").unwrap();

    let by_path = stale_detail(&env, &side, &["attach", &id, "shot.png"]);
    assert_eq!(
        retry_words(&by_path),
        ["-C", main.to_str().unwrap(), "attach", &id, "shot.png"]
    );

    let out = env
        .cmd(&side)
        .args(["attach", &id, "-", "--name", "in.txt"])
        .write_stdin("hello")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy");
    let by_stdin = error["error"]["detail"].as_str().unwrap().to_string();
    assert!(by_stdin.ends_with("; supply the same input on stdin"), "{by_stdin}");
    assert!(!side.join(format!("tasks/files/{id}")).exists());

    // Both retries, run verbatim from the same directory, land in the newer checkout: the
    // relative path still resolves, and stdin is supplied again.
    env.cmd(&side).args(retry_words(&by_path)).assert().success();
    env.cmd(&side)
        .args(retry_words(&by_stdin))
        .write_stdin("hello")
        .assert()
        .success();
    let files = main.join(format!("tasks/files/{id}"));
    assert_eq!(std::fs::read(files.join("shot.png")).unwrap(), b"png");
    assert_eq!(std::fs::read(files.join("in.txt")).unwrap(), b"hello");
}

#[test]
fn edit_body_from_stdin_on_a_copy_that_is_behind_asks_for_the_same_input() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    stamp(&main, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    let out = env
        .cmd(&side)
        .args(["edit", &id, "--body", "-"])
        .write_stdin("new body\n")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.ends_with("; supply the same input on stdin"), "{detail}");
    env.cmd(&side)
        .args(retry_words(detail))
        .write_stdin("new body\n")
        .assert()
        .success();
    let shown = env.json(&main, &["show", &id]);
    assert!(shown["task"]["body"].as_str().unwrap().contains("new body"), "{shown}");
}

#[test]
fn detach_of_a_leftover_file_from_a_copy_that_is_behind_keeps_the_file() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    std::fs::write(main.join("shot.png"), b"png").unwrap();
    env.json(&main, &["attach", &id, "shot.png"]);
    env.json(&main, &["detach", &id, "shot.png", "wrong file"]);
    // An interrupted detach leaves the ledger saying "detached" with the file still there;
    // a rerun only removes the file and never saves the record.
    let leftover = main.join(format!("tasks/files/{id}/shot.png"));
    std::fs::create_dir_all(leftover.parent().unwrap()).unwrap();
    std::fs::write(&leftover, b"png").unwrap();
    let created = "2026-09-01T00:00:00Z";
    stamp(&main, &id, created, "2026-09-05T09:00:00Z");
    stamp(&side, &id, created, "2026-09-07T10:00:00Z");
    stale_detail(&env, &main, &["detach", &id, "shot.png", "wrong file"]);
    assert!(leftover.exists(), "the refusal removed the file");
}

#[test]
fn a_sibling_that_moves_ahead_while_the_editor_is_open_refuses_and_keeps_the_edit() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    for dir in [&main, &side] {
        stamp(dir, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    }
    // While the editor is open, the worktree writes. The edit also forges a far-future
    // stamp in its own copy, which must not count: the baseline is what was loaded.
    let sibling = side.join(format!("tasks/{id}.md"));
    let editor = editor_script(
        &main,
        &format!(
            "sed -i 's/^updated: .*/updated: 2030-01-01T00:00:00Z/' '{}' && \
             sed -i 's/^updated: .*/updated: 2031-01-01T00:00:00Z/; s/^title: T$/title: Edited/' \"$1\"",
            sibling.display()
        ),
    );
    let out = env
        .cmd(&main)
        .env("EDITOR", &editor)
        .args(["edit", &id])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "stale_copy", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("fresh editor"), "{detail}");
    let kept = detail
        .split("edit kept at ")
        .nth(1)
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("the kept file is named");
    assert!(std::fs::read_to_string(kept).unwrap().contains("title: Edited"));
    assert_eq!(env.json(&main, &["show", &id])["task"]["title"], "T");
}
```

- [ ] **Step 2: Run them**

Run `just test-one --test cli <filter>` with `reads_nothing_and_creates_nothing`,
`asks_for_the_same_input` and `leftover_file`.
Expected: PASS already. Task 2's load-time check covers all three, and these tests pin that
down.

Run: `just test-one --test cli moves_ahead_while_the_editor`
Expected: FAIL. The edit saves, exits 0, and the title becomes `Edited`.

- [ ] **Step 3: Add the second check on the editor path**

In `src/commands/edit.rs` `editor`, directly after the block that follows
`super::lock_and_revalidate(&mut ctx, &routing).map_err(keep)?;` (the identity-changed
check), add:

```rust
    // Spec record-home §3.1: the lock was released while the editor was open, so another
    // checkout may have written since the first check.
    super::refuse_stale_copy(&mut ctx, &original, &original_raw, super::Writer::Command)
        .map_err(|error| {
            keep(error.with_suffix(
                "; a rerun opens a fresh editor on the newer copy, so copy your changes over \
                 from the kept file",
            ))
        })?;
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `just test-one --test cli moves_ahead_while_the_editor`, then `just test-fast`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-dc5d46 "editor rechecks after re-locking; attach, stdin, and leftover-detach refusals touch nothing"
tasks check
git add src/commands/edit.rs tests/cli.rs tasks/
git commit -m "feat(edit): recheck for a newer copy after the editor closes"
```

### Task 4: Feedback recurrence, explicit and automatic

**Files:**
- Modify: `src/commands/feedback.rs` (`recur_into`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `commands::refuse_stale_copy(ctx, task, raw, Writer::Feedback)` (Task 2).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_feedback_recurrence_onto_a_copy_behind_the_owners_worktree_refuses_both_ways() {
    let mut env = TestEnv::new();
    let owner = env.init("own");
    accept_feedback(&owner, "the own tool");
    git(&owner, &["init", "-q", "-b", "main"]);
    let reporter = env.init("rep");
    let report = ["feedback", "--project", "own", "slow startup", "--category", "friction"];
    let id = env.json(&reporter, &report)["id"].as_str().unwrap().to_string();
    git(&owner, &["add", "-A"]);
    git(&owner, &["commit", "-qm", "seed"]);
    let side = owner.join("wt");
    git(
        &owner,
        &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()],
    );
    stamp(&owner, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let file = format!("tasks/{id}.md");
    let before = env.read(&owner, &file);

    let mut explicit = report.to_vec();
    explicit.extend(["--recur", id.as_str()]);
    // The same title again is the automatic match; --recur names it outright.
    for args in [report.to_vec(), explicit] {
        let detail = stale_detail(&env, &reporter, &args);
        assert!(detail.contains(side.to_str().unwrap()), "{detail}");
        assert!(
            detail.ends_with("nothing was written. Rerun with --new to file a separate entry"),
            "{detail}"
        );
        assert_eq!(env.read(&owner, &file), before);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `just test-one --test cli feedback_recurrence_onto_a_copy_behind`
Expected: FAIL, because the recurrence lands and exits 0.

- [ ] **Step 3: Check in `recur_into`**

In `src/commands/feedback.rs` `recur_into`, which both the explicit and the automatic path
reach, add directly before `let mut claims = crate::claims::ClaimStore::load(&ctx.project.prefix)?;`:

```rust
    // Record-home spec §3.1: under the target's lock, before anything is appended.
    let (loaded, raw) = ctx.project.read_task_with_raw(id)?;
    super::refuse_stale_copy(ctx, &loaded, &raw, super::Writer::Feedback)?;
```

- [ ] **Step 4: Run it to see it pass, then the fast suite**

Run: `just test-one --test cli feedback_recurrence_onto_a_copy_behind`, then `just test-fast`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
tasks done tasks-e5c16d "both feedback recurrence paths refuse onto a copy behind the owner's worktree, offering --new"
tasks check
git add src/commands/feedback.rs tests/cli.rs tasks/
git commit -m "feat(feedback): refuse a recurrence onto a copy behind another checkout"
```

### Task 5: The claim follows its holder

**Files:**
- Modify: `src/commands/mod.rs` (new `follow_holder`), `src/commands/status.rs` (`note`:
  replace the heartbeat block), `src/commands/edit.rs` (both save paths),
  `src/commands/dep.rs`, `src/commands/attach.rs` (`attach` and `detach`), and
  `src/commands/feedback.rs` (`run`, after a recurrence)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Ctx::ownership`, `crate::claims::resolve_identity`, `Ctx::claims_mut`, and the
  test helpers `started_then_branched` and `stale_detail` (Task 2).
- Produces: `commands::follow_holder(ctx: &mut Ctx, id: &TaskId, me: Option<&crate::claims::Resolution>, landed: &str)`.

- [ ] **Step 1: Write the failing tests**

```rust
fn claim_worktree(env: &TestEnv, dir: &std::path::Path, id: &str) -> String {
    env.json(dir, &["show", id])["claim"]["worktree"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_holders_writes_move_its_claim_to_the_checkout_they_land_in() {
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    assert_eq!(claim_worktree(&env, &main, &id), main.display().to_string());
    as_agent(&env, &side, "agent-a")
        .args(["note", &id, "working here"])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &main, &id), side.display().to_string());

    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &side, "agent-a")
        .args(["edit", &id, "-p", "1"])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &main, &id), side.display().to_string());
}

#[test]
fn another_sessions_write_leaves_the_claim_where_it_is() {
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &side, "agent-b")
        .args(["note", &id, "passing through"])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &main, &id), main.display().to_string());
}

#[test]
fn a_restart_in_the_new_worktree_makes_a_later_write_in_main_refuse() {
    // tasks-142d2f with the protocol step: the note in main now refuses.
    let mut env = TestEnv::new();
    let (main, side, id) = started_then_branched(&mut env);
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &side, &id), side.display().to_string());
    // Another session writing from main is told who works there, with no retry.
    let detail = stale_detail(&env, &main, &["note", &id, "from main"]);
    assert!(
        detail.starts_with(&format!("tasks/{id}.md in {} is newer", side.display())),
        "{detail}"
    );
    assert!(detail.contains("agent-a") && !detail.contains("tasks -C"), "{detail}");
}

#[test]
fn a_holders_feedback_recurrence_moves_the_claim_to_the_owners_checkout() {
    let mut env = TestEnv::new();
    let owner = env.init("own");
    accept_feedback(&owner, "the own tool");
    git(&owner, &["init", "-q", "-b", "main"]);
    let reporter = env.init("rep");
    let report = ["feedback", "--project", "own", "slow startup", "--category", "friction"];
    let id = env.json(&reporter, &report)["id"].as_str().unwrap().to_string();
    git(&owner, &["add", "-A"]);
    git(&owner, &["commit", "-qm", "seed"]);
    let side = owner.join("wt");
    git(
        &owner,
        &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()],
    );
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &side, &id), side.display().to_string());
    // The owner's main checkout holds the newer copy, so the recurrence may land there.
    stamp(&side, &id, "2026-09-01T00:00:00Z", "2026-09-05T09:00:00Z");
    stamp(&owner, &id, "2026-09-01T00:00:00Z", "2026-09-07T10:00:00Z");
    let mut explicit = report.to_vec();
    explicit.extend(["--recur", id.as_str()]);
    as_agent(&env, &reporter, "agent-a")
        .args(&explicit)
        .assert()
        .success();
    assert_eq!(claim_worktree(&env, &owner, &id), owner.display().to_string());
}

#[test]
fn a_claim_held_by_proof_moves_with_its_holder() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "Thing", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    let state = relay_on(&env);
    // Start under relay identity, commit, branch, then lose the registry: ownership can
    // only come from the claim's recorded proof.
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id}\n\
         git add -A && git -c user.name=t -c user.email=t@e commit -qm start\n\
         git worktree add -q -b side '{}'\n\
         rm \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" -C '{}' note {id} 'by proof'\n",
        shim_env(&state, "codex", "s1"),
        side.display(),
        side.display()
    );
    let out = common::harness_shim(&main, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(claim_worktree(&env, &main, &id), side.display().to_string());
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --test cli claim_to_the_checkout`,
`just test-one --test cli held_by_proof_moves` and
`just test-one --test cli feedback_recurrence_moves_the_claim`
Expected: FAIL, because the claim still names the checkout `start` ran in. `another_sessions_write_leaves` and
`a_restart_in_the_new_worktree` pass already, since `start` sets the worktree today.

- [ ] **Step 3: Add `follow_holder` to `src/commands/mod.rs`**

Place it after `occupants`:

```rust
/// Record-home spec §4: a write by the holder of a live claim moves the claim to this
/// checkout and refreshes its heartbeat. Holder means `Ctx::ownership` is not `Foreign`,
/// by identity or by proof. It runs after the record is saved and never fails the command.
/// `me` is the caller's identity when the command already resolved it; otherwise it is
/// resolved here, and only when a claim exists to follow.
pub(crate) fn follow_holder(
    ctx: &mut Ctx,
    id: &TaskId,
    me: Option<&crate::claims::Resolution>,
    landed: &str,
) {
    let claim = match ctx.claims_mut() {
        Ok(store) => {
            // A write must not revive a stale claim.
            store.prune_dead();
            match store.get(id) {
                Some(claim) => claim.clone(),
                None => return,
            }
        }
        Err(error) => {
            ctx.warnings.push(format!(
                "{landed}, but the claim on {id} could not be read ({error})"
            ));
            return;
        }
    };
    let resolved;
    let me = match me {
        Some(me) => me,
        None => {
            resolved = crate::claims::resolve_identity(&mut ctx.warnings);
            &resolved
        }
    };
    match ctx.ownership(&claim, me) {
        Ok(Ownership::Foreign) => return,
        Ok(Ownership::ByIdentity | Ownership::ByProof) => {}
        Err(error) => {
            ctx.warnings.push(format!(
                "{landed}, but whether this session holds the claim on {id} could not be \
                 established ({error})"
            ));
            return;
        }
    }
    let moved = crate::claims::Claim {
        worktree: ctx.project.root.display().to_string(),
        seen: crate::time::now(),
        ..claim
    };
    let saved = ctx.claims_mut().and_then(|store| {
        store.insert(id, moved);
        store.save()
    });
    if let Err(error) = saved {
        ctx.warnings.push(format!(
            "{landed}, but the claim heartbeat on {id} was not refreshed ({error}); the \
             claim may look stale to other sessions"
        ));
    }
}
```

- [ ] **Step 4: Call it from every write that keeps the claim**

In `src/commands/status.rs` `note`, delete from `let mine = existing.filter(|_| mine);` to
the end of the `if let Some(claim) = mine { … }` heartbeat block, and put this in its place:

```rust
    // The heartbeat, and only on our own claim: `note` never touches a foreign one. It
    // also moves the claim to this checkout (record-home spec §4).
    follow_holder(&mut ctx, &task.id, Some(&me), "the note landed");
```

Import it with `use super::{…, follow_holder};`. Keep the warning block above it, which
covers a claim whose ownership could not be established because identity did not resolve.

In `src/commands/edit.rs`, in the field-flag path, change the ending to:

```rust
    save(&mut ctx, &mut task)?;
    super::follow_holder(&mut ctx, &task.id, None, "the edit landed");
    Ok(id_out(ctx, &task))
```

In `editor`, after `save(&mut ctx, &mut edited).map_err(keep)?;`, add
`super::follow_holder(&mut ctx, &edited.id, None, "the edit landed");`.

In `src/commands/dep.rs` `run`, after `save(&mut ctx, &mut task)?;`, add
`super::follow_holder(&mut ctx, &task.id, None, "the dependency change landed");`.

In `src/commands/attach.rs` `attach`, change `ctx.resolve_for_guard()?;` to
`let me = ctx.resolve_for_guard()?;`, and add
`super::follow_holder(&mut ctx, &task.id, Some(&me), "the attachment landed");` directly
before `Ok(Output::Attach(…))`. In `detach`, make the same `let me` change. Inside
`if recorded != Some(Ledger::Detached) { … }`, after `save(…)?;`, add
`super::follow_holder(&mut ctx, &task.id, Some(&me), "the detach landed");`.

In `src/commands/feedback.rs` `run`, where a match recurs, call it after the recurrence
lands. `recur_into` has already saved its own pruned claim store by then, so this load
starts fresh and clobbers nothing:

```rust
    let (task, action) = match existing {
        Some((id, automatic)) => {
            let task = recur_into(
                &mut ctx, &id, automatic, &summary, &body, &category, &from, &prefix,
            )?;
            // Record-home spec §4: a holder's recurrence moves its claim to the owner's
            // registered root, where it landed.
            super::follow_holder(&mut ctx, &task.id, None, "the recurrence landed");
            (task, "recurred")
        }
```

A status change that releases the claim leaves nothing for `follow_holder` to find, so the
edit path's call is harmless after `--status done`.

- [ ] **Step 5: Run the tests to see them pass, then the fast suite**

Run: `just test-one --test cli claim_to_the_checkout`,
`just test-one --test cli held_by_proof_moves`,
`just test-one --test cli feedback_recurrence_moves_the_claim`,
`just test-one --test cli heartbeat`, then `just test-fast`
Expected: PASS. The existing heartbeat tests keep their exact warning text,
`the note landed, but the claim heartbeat on … was not refreshed`.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-e24e39 "a holder's note, edit, dep, attach, detach, and feedback recurrence move its claim to the checkout they land in"
tasks check
git add src/commands tests/cli.rs tasks/
git commit -m "feat(claims): move a claim to the checkout its holder writes in"
```

### Task 6: `show` finds a worktree-only task

**Files:**
- Modify: `src/commands/parked.rs` (`open_recorded`; `scan_recorded` delegates to it) and
  `src/commands/show.rs` (`run`)
- Test: `tests/cli.rs`. `repo_with_worktree` becomes a wrapper around a new
  `repo_with_worktree_as(env, prefix)`.

**Interfaces:**
- Produces: `parked::open_recorded(id: &TaskId, worktree: &Path) -> Result<Option<(Project, Task, Vec<Task>)>>`.
- Consumes: `ClaimSnapshot::live(&TaskId) -> Option<&Claim>`, `ClaimSnapshot::park(&TaskId) -> Option<&Park>`,
  and `show::describe` (signature unchanged).

- [ ] **Step 1: Write the failing tests**

Generalize the helper by replacing `repo_with_worktree` with the pair below. Every existing
caller stays as it is:

```rust
fn repo_with_worktree(env: &mut TestEnv) -> (std::path::PathBuf, std::path::PathBuf, String) {
    repo_with_worktree_as(env, "sci")
}

/// `repo_with_worktree` for a project with the given prefix.
fn repo_with_worktree_as(
    env: &mut TestEnv,
    prefix: &str,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let main = env.init(prefix);
    git(&main, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&main, &["add", "T", "-p", "2"]));
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "seed"]);
    let side = main.join("wt");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()],
    );
    (main, side, id)
}
```

Then the tests:

```rust
#[test]
fn show_reads_a_worktree_only_task_from_the_checkout_its_claim_or_park_names() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let claimed = id_of(env.json(&side, &["add", "Claimed there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &claimed])
        .assert()
        .success();
    let v = env.json(&main, &["show", &claimed]);
    assert_eq!(v["task"]["title"], "Claimed there", "{v}");
    assert!(
        warnings_of(&v).contains(&format!(
            "{claimed} exists only in {}; shown from that checkout",
            side.display()
        )),
        "{v}"
    );

    let parked = id_of(env.json(&side, &["add", "Parked there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &parked])
        .assert()
        .success();
    as_agent(&env, &side, "agent-a")
        .args(["park", &parked, "pick it up"])
        .assert()
        .success();
    let v = env.json(&main, &["show", &parked]);
    assert_eq!(v["task"]["title"], "Parked there", "{v}");
    assert_eq!(v["park"]["next_step"], "pick it up", "{v}");
}

#[test]
fn show_of_another_projects_worktree_only_task_falls_back_through_its_claims() {
    let mut env = TestEnv::new();
    let here = env.init("sci");
    let (_, side, _) = repo_with_worktree_as(&mut env, "oth");
    let id = id_of(env.json(&side, &["add", "Over there", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&here, &["show", &id]);
    assert_eq!(v["task"]["title"], "Over there", "{v}");
    assert!(
        warnings_of(&v).iter().any(|w| w.contains("exists only in")),
        "{v}"
    );
}

#[test]
fn show_names_the_checkout_when_the_one_holding_the_task_is_gone() {
    let mut env = TestEnv::new();
    let (main, side, _) = repo_with_worktree(&mut env);
    let id = id_of(env.json(&side, &["add", "Lost", "-p", "2"]));
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    git(
        &main,
        &["worktree", "remove", "--force", side.to_str().unwrap()],
    );
    let error = error_of(&env, &main, &["show", &id]);
    assert_eq!(error["error"]["kind"], "task_not_found", "{error}");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(
        detail.contains(&format!("claimed in {}, which is unavailable", side.display())),
        "{detail}"
    );
}

#[test]
fn show_reads_the_local_copy_even_when_the_claimed_one_is_newer() {
    let mut env = TestEnv::new();
    let (main, side, id) = repo_with_worktree(&mut env);
    as_agent(&env, &side, "agent-a")
        .args(["start", &id])
        .assert()
        .success();
    let v = env.json(&main, &["show", &id]);
    assert_eq!(v["task"]["status"], "todo", "{v}");
    assert!(
        !warnings_of(&v).iter().any(|w| w.contains("exists only in")),
        "{v}"
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run `just test-one --test cli <filter>` with `worktree_only_task`,
`another_projects_worktree_only` and `show_names_the_checkout`.
Expected: FAIL with `task_not_found` and no fallback. `show_reads_the_local_copy` passes
already, which pins down the park §5.3 rule.

- [ ] **Step 3: Add `open_recorded` in `src/commands/parked.rs`**

Replace `scan_recorded` with:

```rust
/// Opens the checkout a store entry recorded and reads `id` there, with that checkout's
/// whole scan so the task's children, dependencies, and documents resolve there too (park
/// design §5.3). `None` when the checkout is gone, belongs to another prefix, or lacks the
/// record.
pub fn open_recorded(
    id: &TaskId,
    worktree: &Path,
) -> Result<Option<(Project, Task, Vec<Task>)>> {
    if !crate::scope::has_config(worktree)? {
        return Ok(None);
    }
    let project = Project::open(worktree)?;
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let scan = project.scan()?;
    let Some(task) = scan.iter().find(|task| task.id == *id).cloned() else {
        return Ok(None);
    };
    Ok(Some((project, task, scan)))
}

/// `open_recorded` without the project, for rows that need only the scan.
pub fn scan_recorded(id: &TaskId, worktree: &Path) -> Result<Option<(Task, Vec<Task>)>> {
    Ok(open_recorded(id, worktree)?.map(|(_, task, scan)| (task, scan)))
}
```

- [ ] **Step 4: Fall back in `src/commands/show.rs`**

Replace `run` with the following, adding `use crate::error::Error;` and
`use std::path::Path;`. `ctx.project` is already the id's own project, because `show`
routes by prefix (`open_id_ctx`), so another project's id reads that project's claim
store:

```rust
pub fn run(mut ctx: Ctx, id: String) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &id)?;
    let claims =
        crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, all) = match ctx.project.read_task(&id) {
        Ok(task) => (None, task, ctx.project.scan()?),
        Err(Error::TaskNotFound(_)) => {
            let (project, task, all) = recorded_elsewhere(&id, &claims, &mut ctx.warnings)?;
            (Some(project), task, all)
        }
        Err(error) => return Err(error),
    };
    let project = recorded.as_ref().unwrap_or(&ctx.project);
    let now = crate::time::parse(&crate::time::now())?;
    let fields = describe(
        project,
        &ctx.registry,
        task,
        &all,
        Some(&claims),
        &mut ctx.warnings,
        now,
    )?;
    Ok(Output::Show(Box::new(ShowOut {
        fields,
        warnings: ctx.warnings,
    })))
}

/// Record-home spec §6.1: a record this checkout lacks is read from the checkout named by
/// its live claim, or else by its park. A record present here is always read here.
fn recorded_elsewhere(
    id: &crate::model::TaskId,
    claims: &crate::claims::ClaimSnapshot,
    warnings: &mut Vec<String>,
) -> Result<(Project, Task, Vec<Task>)> {
    let named = claims
        .live(id)
        .map(|claim| ("claimed", claim.worktree.as_str()))
        .or_else(|| claims.park(id).map(|park| ("parked", park.worktree.as_str())));
    let Some((how, worktree)) = named else {
        return Err(Error::TaskNotFound(id.to_string()));
    };
    let unavailable = |why: String| {
        Error::TaskNotFound(id.to_string())
            .with_suffix(&format!(" ({how} in {worktree}, which is unavailable{why})"))
    };
    match super::parked::open_recorded(id, Path::new(worktree)) {
        Ok(Some(found)) => {
            warnings.push(format!(
                "{id} exists only in {worktree}; shown from that checkout"
            ));
            Ok(found)
        }
        Ok(None) => Err(unavailable(String::new())),
        Err(error) => Err(unavailable(format!(": {error}"))),
    }
}
```

`Error::TaskNotFound` displays as `task {0} not found`, so the detail reads
`task sci-1a2b3c (claimed in /path/wt, which is unavailable) not found`.

- [ ] **Step 5: Run the tests to see them pass, then the fast suite**

Run `just test-one --test cli <filter>` with `show_reads`, `another_projects_worktree_only`
and `show_names_the_checkout`, then `just test-fast`.
Expected: PASS. The `prime` and `list --parked` tests that use `scan_recorded` pass unchanged.

- [ ] **Step 6: Commit**

```bash
tasks done tasks-fea247 "show reads a worktree-only task, in this or another project, from the checkout its claim or park names"
tasks check
git add src/commands/parked.rs src/commands/show.rs tests/cli.rs tasks/
git commit -m "feat(show): read a worktree-only task from its claim's or park's checkout"
```

### Task 7: Documents, the protocol step, and rollout

**Files:**
- Modify: `docs/specs/2026-09-05-work-claims-design.md` (§Command behaviour `note`
  bullet, §Warnings first bullet), `docs/specs/2026-09-09-park-design.md` (§5.3),
  `skills/tasks/SKILL.md` (session protocol step 3, Process and workspace), `AGENTS.md`
  (Process and workspace), `README.md` (claims paragraph)

**Interfaces:**
- Consumes: the behaviour of Tasks 1–6, as the text below describes it.

- [ ] **Step 1: Work-claims design**

In §Command behaviour, in the `**note**` bullet, replace `It is never refused.` with:
`The claim guard never refuses it. Like every write, it refuses from a copy that another
checkout's copy has moved past (record-home spec §3), and a note by the claim's holder moves
the claim to its checkout (§4).`

Replace the whole first §Warnings bullet, `**A newer copy elsewhere** (tasks-76671b): …`
through `…the noise the newer-only rule exists to avoid.`, with:

```markdown
- **A newer copy elsewhere** (tasks-76671b; a refusal since the record-home spec,
  `docs/specs/2026-09-30-record-home-design.md` §3): every write of an existing record
  compares the copy it loaded against the same record in every other worktree, found
  through `git worktree list --porcelain -z` and read at the project's offset below the
  repository top level. The check runs at load, under the mutation lock, before any other
  input is read. A sibling with a *newer* stamp, or with the same stamp but different
  bytes (a same-second fork), refuses the write as `stale_copy`. The error names that
  checkout and says what to do. If another session works there, wait or merge. If this is
  a worktree behind the main checkout, merge it in. Otherwise, rerun with `tasks -C
  <root> …`. A sibling that is behind says nothing and is never compared by content. A
  copy that cannot be read or parsed, or a git failure, is a warning, never a refusal.
```

- [ ] **Step 2: Park design §5.3**

After the paragraph that ends `…where the parked copy has moved on.`, add:

```markdown
`show` resolves a record this checkout lacks the same way (record-home spec §6.1): from the
checkout its live claim names, or else its park, with the warning `<id> exists only in
<worktree>; shown from that checkout`. A record present here is always read here.
```

- [ ] **Step 3: The skill, the agent guide, and the README**

In `skills/tasks/SKILL.md`, session protocol step 3, after the sentence ending
`…records that in the task's notes.`, add:

```markdown
   A write refuses with `stale_copy` when another checkout holds a newer copy of the
   record, and the error says what to do: rerun there with the printed `tasks -C <root> …`,
   merge the main checkout's copy into your worktree, or leave the record to the session
   named as working there. There is no override. A write by the claim's holder moves the
   claim to the checkout it lands in, and `show` finds a task that exists only in the
   checkout its claim or park names.
```

In **Process and workspace**, after `…or reuse it on resume.`, add:
`After creating a worktree for a task you have started, run `tasks start <id>` there
before any other `tasks` command for it: the claim moves there, and a later write from the
main checkout refuses instead of forking the record.`

In `AGENTS.md` **Process and workspace**, after `…commit the task record before git
worktree add, then reuse its isolated worktree or create one under .worktrees/;`, add:
`in a new worktree, run tasks start <id> before any other tasks command for it;`

In `README.md`, append to the paragraph that begins `` `start` also writes a per-project claim ``:
`A write refuses with ``stale_copy`` when another worktree holds a newer copy of the record,
and says whether to rerun there, merge, or leave it to the session working there.`

- [ ] **Step 4: Report the protocol step to the global instructions' owner**

Supply `TASKS_AGENT=<harness>/<model>` on this invocation with the harness and model that
run this step, or the harness alone if that is all you know. When unsure, leave the
variable off; a wrong attribution is worse than none.

```bash
tasks feedback --project tack \
  "The global worktree rule should tell agents to run 'tasks start <id>' in a new worktree before any other tasks command for that task, now that tasks refuses writes from a copy behind another checkout" \
  --category idea
tasks note tasks-3b5e4c "filed the global-instructions step as tack feedback <returned id>"
```

- [ ] **Step 5: Verify, reinstall, and commit**

Run: `just check`, then `cargo install --path .`
Expected: clean.

```bash
tasks done tasks-3b5e4c "docs, skill, and agent guide describe stale_copy and its remedies, the claim move, the show fallback, and the new-worktree start"
tasks check
git add docs skills AGENTS.md README.md tasks/
git commit -m "docs: record-home refusal, claim move, and show fallback"
```

When all seven are done, close `tasks-9949f3`. Close tasks-2c0a1d and tasks-bb53e5 with
`tasks done`, naming the commits. Note on tasks-fbc32b that `show` landed and the `list`
half was declined, then `tasks done` it.
