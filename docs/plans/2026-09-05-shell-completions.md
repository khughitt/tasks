# Shell Completions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tasks` completes subcommands, flags, fixed value sets, project prefixes, and task ids in bash and zsh.

**Architecture:** `clap_complete`'s `CompleteEnv` puts a small stub in the user's shell rc that re-invokes `tasks` on every TAB. A new `src/complete.rs` supplies candidates: fixed sets from the constants the parser already validates against, and task ids from a filesystem scan whose project is chosen per argument by one of six scopes. Because `ArgValueCompleter` is handed only the value being completed, `complete.rs` also recovers `-C`, `--project`, and the subject id by walking the transport argv itself.

**Tech Stack:** Rust 2024, clap 4.6 (derive), clap_complete 4.6 (`unstable-dynamic`), `tests/cli.rs` end-to-end tests via `assert_cmd`.

**Spec:** `docs/specs/2026-09-05-shell-completions-design.md`

## Global Constraints

- Dependency: `clap_complete = { version = "4.6", features = ["unstable-dynamic"] }`. No other new dependency.
- The activating environment variable is `TASKS_COMPLETE`, never the default `COMPLETE`.
- Nothing in `src/complete.rs` returns `Result`. Every failure path yields an empty `Vec<CompletionCandidate>`. This is the spec's documented exception to the repo's fail-early rule and is confined to this module; no command path may call into it.
- No `tasks completions` subcommand. The rc-sourced stub is the whole integration.
- JSON output shapes are unchanged. Completion never alters parsing: the only edits to argument definitions are `#[arg(add = …)]`.
- Every commit must pass `just check` (the pre-commit hook runs it). Run `just gate` before the final commit of each task.
- Conventional commits, no AI-attribution trailers.
- Task-id candidate order is open tasks before closed, each by id ascending.
- Descriptions are `"<status>  <title>"` (two spaces). Bash drops them; zsh shows them.

---

### Task 1: Wire CompleteEnv and complete the fixed value sets

Delivers working completion for subcommands, flags, `--status`, `--size`, `--sort`, `--color`, `--category`, and project prefixes. No task ids yet.

**Files:**
- Modify: `Cargo.toml`
- Create: `src/complete.rs`
- Modify: `src/main.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/feedback.rs` (widen one function's visibility)
- Modify: `tests/common/mod.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `crate::model::{Status, Size}`, `crate::registry::Registry`, `crate::commands::feedback::CATEGORIES`.
- Produces: `complete::statuses()`, `complete::add_statuses()`, `complete::sizes()`, `complete::sorts()`, `complete::colors()`, `complete::categories()`, `complete::prefixes()` — each `fn() -> Vec<clap_complete::CompletionCandidate>`. `TestEnv::complete(&self, dir: &Path, shell: &str, index: usize, words: &[&str]) -> Vec<String>`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]`, after the `clap` line:

```toml
clap_complete = { version = "4.6", features = ["unstable-dynamic"] }
```

- [ ] **Step 2: Add the test helper**

In `tests/common/mod.rs`, add `.env_remove("TASKS_COMPLETE")` to the chains in **both** `cmd` and `raw` (put it next to `.env_remove("TASKS_FORMAT")`), so an ambient variable cannot alter an ordinary test. Then add this method to `impl TestEnv`:

```rust
    /// One completion request over the `CompleteEnv` transport. The shell invokes
    /// `tasks -- <words…>` with the cursor on `index`; `words[0]` is the binary name, so
    /// `complete(dir, "bash", 2, &["tasks", "show", "sci-"])` completes `sci-`.
    /// Returns one string per candidate; under `"zsh"` each is `value:description`.
    pub fn complete(&self, dir: &Path, shell: &str, index: usize, words: &[&str]) -> Vec<String> {
        let out = self
            .cmd(dir)
            .env("TASKS_COMPLETE", shell)
            .env("_CLAP_COMPLETE_INDEX", index.to_string())
            .arg("--")
            .args(words)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "completion for {words:?} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "completion wrote to stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }
```

- [ ] **Step 3: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn completion_offers_the_fixed_value_sets_and_registry_prefixes() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");

    // every status on edit, but only the two `add` accepts
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "edit", "sci-000001", "--status", ""]),
        ["idea", "todo", "doing", "blocked", "done", "dropped"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--status", ""]),
        ["idea", "todo"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "ready", "--size", ""]),
        ["xs", "s", "m", "l", "xl"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--sort", ""]),
        ["priority", "updated", "created"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", "list", "--color", ""]),
        ["auto", "always", "never"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "feedback", "S", "--category", ""]),
        ["friction", "gap", "idea", "positive"]
    );

    // registry prefixes, in registry order
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--project", ""]),
        ["fam", "sci"]
    );
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "unregister", ""]),
        ["fam", "sci"]
    );

    // subcommands come from the derive, and a prefix narrows them
    let subs = env.complete(&sci, "bash", 1, &["tasks", "re"]);
    assert!(subs.contains(&"ready".to_string()), "{subs:?}");
}

#[test]
fn completion_stub_is_emitted_and_the_hook_is_otherwise_inert() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    // no words after `--`: the registration stub, naming the binary
    let out = env
        .cmd(&sci)
        .env("TASKS_COMPLETE", "bash")
        .arg("--")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stub = String::from_utf8_lossy(&out.stdout);
    assert!(stub.contains("complete "), "{stub}");
    assert!(stub.contains("TASKS_COMPLETE"), "{stub}");

    // an unsupported shell name is an error, not a silent stub
    let bad = env
        .cmd(&sci)
        .env("TASKS_COMPLETE", "notashell")
        .arg("--")
        .output()
        .unwrap();
    assert!(!bad.status.success());

    // with the variable unset, an ordinary argv runs the ordinary command
    let v = env.json(&sci, &["list"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test --test cli -- completion_offers completion_stub`
Expected: FAIL to compile (`no method named complete`) or, once the helper exists, FAIL with empty candidate lists.

- [ ] **Step 5: Create the candidate module**

Create `src/complete.rs`:

```rust
//! Completion candidates for the `CompleteEnv` protocol.
//!
//! Nothing here is reachable from a command path, and nothing here returns `Result`.
//! Completion runs on every TAB with no error channel — the bash stub discards
//! `COMPREPLY` when the completer exits non-zero, and a write to stderr corrupts the
//! prompt — so every failure yields an empty candidate list instead. This is the
//! deliberate exception to the repo's fail-early rule recorded in
//! `docs/specs/2026-09-05-shell-completions-design.md`.

use clap_complete::CompletionCandidate;

use crate::model::{Size, Status};
use crate::registry::Registry;

fn plain(values: impl IntoIterator<Item = impl Into<std::ffi::OsString>>) -> Vec<CompletionCandidate> {
    values.into_iter().map(CompletionCandidate::new).collect()
}

/// Every status. `edit --status`, `list --status`, `tags --status`.
pub fn statuses() -> Vec<CompletionCandidate> {
    plain(Status::ALL.iter().map(|status| status.as_str()))
}

/// The two `add` accepts; it rejects the rest with a validation error.
pub fn add_statuses() -> Vec<CompletionCandidate> {
    plain([Status::Idea.as_str(), Status::Todo.as_str()])
}

pub fn sizes() -> Vec<CompletionCandidate> {
    plain(Size::ALL.iter().map(|size| size.as_str()))
}

/// The keys `query::SortKey::parse` accepts.
pub fn sorts() -> Vec<CompletionCandidate> {
    plain(["priority", "updated", "created"])
}

/// The modes `style::ColorMode::resolve` accepts.
pub fn colors() -> Vec<CompletionCandidate> {
    plain(["auto", "always", "never"])
}

pub fn categories() -> Vec<CompletionCandidate> {
    plain(crate::commands::feedback::CATEGORIES)
}

/// Registered prefixes, in registry order. An unreadable registry offers nothing.
pub fn prefixes() -> Vec<CompletionCandidate> {
    let Ok(registry) = Registry::load() else {
        return Vec::new();
    };
    plain(registry.projects.keys().cloned().collect::<Vec<_>>())
}
```

- [ ] **Step 6: Install the hook**

In `src/main.rs`, add `mod complete;` to the module list (alphabetically, after `mod commands;`). Change the `use clap::Parser;` line to:

```rust
use clap::{CommandFactory, Parser};
```

Then make the first statement of `fn main()`:

```rust
    // Must run before anything writes to stdout. Returns immediately unless
    // TASKS_COMPLETE is set, so an ordinary run pays one getenv.
    clap_complete::CompleteEnv::with_factory(cli::Cli::command)
        .var("TASKS_COMPLETE")
        .complete();
```

- [ ] **Step 7: Widen the feedback helpers**

In `src/commands/feedback.rs`, change `fn is_open_feedback(task: &Task) -> bool` to `pub fn is_open_feedback(task: &Task) -> bool`. `CATEGORIES` and `TARGET_PREFIX` are already `pub`. Task 4 uses all three; sharing them is what keeps completion and execution from drifting.

- [ ] **Step 8: Attach the fixed sets**

In `src/cli.rs`, add the import:

```rust
use clap_complete::ArgValueCandidates;
```

Then add `add = …` to these existing `#[arg(...)]` attributes (keep every other attribute on each line unchanged):

- `Cli::color` → `add = ArgValueCandidates::new(crate::complete::colors)`
- `FieldArgs::size` → `add = ArgValueCandidates::new(crate::complete::sizes)`
- `EditArgs::status` → `add = ArgValueCandidates::new(crate::complete::statuses)`
- `Command::Add::status` → `add = ArgValueCandidates::new(crate::complete::add_statuses)`
- `Command::Add::project` → `add = ArgValueCandidates::new(crate::complete::prefixes)`
- `Command::List::statuses` → `add = ArgValueCandidates::new(crate::complete::statuses)`
- `Command::List::sort` → `add = ArgValueCandidates::new(crate::complete::sorts)`
- `Command::Ready::size` → `add = ArgValueCandidates::new(crate::complete::sizes)`
- `Command::Tags::statuses` → `add = ArgValueCandidates::new(crate::complete::statuses)`
- `Command::Feedback::category` → `add = ArgValueCandidates::new(crate::complete::categories)`

`Command::Unregister::prefix` has no attribute today; give it one:

```rust
    Unregister {
        #[arg(add = ArgValueCandidates::new(crate::complete::prefixes))]
        prefix: String,
    },
```

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test --test cli -- completion_offers completion_stub`
Expected: PASS, 2 tests.

- [ ] **Step 10: Run the gate and commit**

Run: `just gate`
Expected: exit 0, all tests pass.

```bash
git add Cargo.toml Cargo.lock src/complete.rs src/main.rs src/cli.rs src/commands/feedback.rs tests/common/mod.rs tests/cli.rs
git commit -m "feat(complete): wire CompleteEnv and complete the fixed value sets"
```

---

### Task 2: Walk the command line to recover -C, --project, and the subject id

`ArgValueCompleter` receives only the value being completed. This task recovers the rest from the transport argv. Pure functions over a word list, unit-tested in-module; no argument is wired to it yet.

> **Landed with Task 3.** Nothing calls this code until Task 3 attaches the completers, and
> `cargo clippy --all-targets -- -D warnings` rejects it as dead code in the meantime —
> `pub` does not exempt an item in a binary-only crate, and `#[cfg(test)]` callers do not
> either. The two tasks therefore share one commit. Do not add `#[allow(dead_code)]`.

**Files:**
- Modify: `src/complete.rs`

**Interfaces:**
- Consumes: `crate::cli::Cli`, `crate::model::TaskId`.
- Produces: `struct Line { subcommand: Option<String>, dir: Option<PathBuf>, project: Option<String>, subject: Option<TaskId>, all_projects: bool }`; `fn line() -> Line` (reads the process argv); `fn walk(words: &[Option<&str>]) -> Line` (pure).

- [ ] **Step 1: Write the failing tests**

Append to `src/complete.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The words a shell sends, with `None` marking the cursor.
    fn walked(words: &[&str], cursor: usize) -> Line {
        let mut words: Vec<Option<&str>> = words.iter().map(|word| Some(*word)).collect();
        if cursor < words.len() {
            words[cursor] = None;
        }
        walk(&words)
    }

    #[test]
    fn the_subject_is_the_first_positional_of_an_id_taking_subcommand() {
        let line = walked(&["tasks", "edit", "sci-000001", "--parent", ""], 4);
        assert_eq!(line.subcommand.as_deref(), Some("edit"));
        assert_eq!(line.subject.map(|id| id.to_string()), Some("sci-000001".into()));
    }

    #[test]
    fn adds_title_is_not_a_subject_even_when_it_parses_as_an_id() {
        let line = walked(&["tasks", "add", "fam-000001", "--parent", ""], 4);
        assert_eq!(line.subcommand.as_deref(), Some("add"));
        assert_eq!(line.subject, None, "add's first positional is a title");
    }

    #[test]
    fn option_values_are_not_positionals() {
        let line = walked(
            &["tasks", "edit", "--body", "fam-000001", "sci-abcdef", "--parent", ""],
            6,
        );
        assert_eq!(line.subject.map(|id| id.to_string()), Some("sci-abcdef".into()));
    }

    #[test]
    fn a_multi_value_option_consumes_every_following_word() {
        let line = walked(
            &["tasks", "dep", "sci-000001", "--on", "fam-1", "fam-2", "--rm", ""],
            7,
        );
        assert_eq!(line.subject.map(|id| id.to_string()), Some("sci-000001".into()));
    }

    #[test]
    fn dir_is_read_in_every_form_and_the_last_one_wins() {
        for words in [
            vec!["tasks", "-C", "/w", "show", ""],
            vec!["tasks", "-C/w", "show", ""],
            vec!["tasks", "-C=/w", "show", ""],
        ] {
            let cursor = words.len() - 1;
            assert_eq!(walked(&words, cursor).dir, Some(PathBuf::from("/w")), "{words:?}");
        }
        let line = walked(&["tasks", "-C", "/a", "-C", "/b", "show", ""], 6);
        assert_eq!(line.dir, Some(PathBuf::from("/b")));
    }

    #[test]
    fn project_and_all_projects_are_recorded() {
        let line = walked(&["tasks", "add", "T", "--project", "fam", "--parent", ""], 6);
        assert_eq!(line.project.as_deref(), Some("fam"));
        let line = walked(&["tasks", "add", "T", "--project=fam", "--parent", ""], 5);
        assert_eq!(line.project.as_deref(), Some("fam"));
        let line = walked(&["tasks", "list", "--all-projects", "--parent", ""], 4);
        assert!(line.all_projects);
        assert!(!walked(&["tasks", "list", "--parent", ""], 3).all_projects);
    }

    #[test]
    fn ambiguity_yields_no_context() {
        // an unrecognized subcommand
        assert_eq!(walked(&["tasks", "frobnicate", ""], 2).subcommand, None);
        // the subcommand itself is under the cursor
        assert_eq!(walked(&["tasks", ""], 1).subcommand, None);
        // -C's value is the word being completed
        assert_eq!(walked(&["tasks", "-C", ""], 2).dir, None);
        // a first positional that is not an id
        assert_eq!(walked(&["tasks", "show", "nonsense", "--"], 3).subject, None);
    }

    #[test]
    fn a_user_typed_double_dash_makes_the_rest_positional() {
        let line = walked(&["tasks", "note", "--", "sci-000001", "text"], 4);
        assert_eq!(line.subject.map(|id| id.to_string()), Some("sci-000001".into()));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks -- complete::tests`
Expected: FAIL to compile — `walk`, `Line`, and `PathBuf` are not defined.

- [ ] **Step 3: Implement the walk**

Add to the imports at the top of `src/complete.rs`:

```rust
use std::path::PathBuf;

use clap::CommandFactory;

use crate::cli::Cli;
use crate::model::TaskId;
```

Then add, above the `#[cfg(test)]` block:

```rust
/// Subcommands whose first positional is a task id. `add`'s is a title, so it has no
/// subject; guessing one there would read `tasks add fam-000001 --parent <TAB>` as an
/// invocation against `fam`.
const ID_FIRST: [&str; 11] = [
    "show", "root", "tree", "edit", "note", "start", "done", "drop", "block", "unblock", "dep",
];

/// What the words on the command line say about the invocation. Only the parts the
/// candidate scopes need; everything else is discarded.
#[derive(Debug, Default, PartialEq)]
pub struct Line {
    pub subcommand: Option<String>,
    pub dir: Option<PathBuf>,
    pub project: Option<String>,
    pub subject: Option<TaskId>,
    pub all_projects: bool,
}

/// The user's command line, from this process's own argv.
///
/// The transport is `tasks -- tasks -C dir show tasks-`: everything after the first `--`
/// is what the shell had, `words[0]` being the binary name. `_CLAP_COMPLETE_INDEX` names
/// the word under the cursor, which is the fragment being completed rather than context,
/// so it is blanked instead of removed — every other word keeps its own index.
pub fn line() -> Line {
    let argv: Vec<String> = std::env::args().collect();
    let Some(dashes) = argv.iter().skip(1).position(|word| word == "--") else {
        return Line::default();
    };
    let words: Vec<String> = argv[dashes + 2..].to_vec();
    let cursor: usize = std::env::var("_CLAP_COMPLETE_INDEX")
        .ok()
        .and_then(|index| index.parse().ok())
        .unwrap_or(usize::MAX);
    let mut refs: Vec<Option<&str>> = words.iter().map(|word| Some(word.as_str())).collect();
    if cursor < refs.len() {
        refs[cursor] = None;
    }
    walk(&refs)
}

/// Classify each word the way clap will. Value-taking is read off the built `Command`
/// rather than a hardcoded list, so a new flag cannot desynchronize this walk.
fn walk(words: &[Option<&str>]) -> Line {
    let root = Cli::command();
    let mut line = Line::default();
    let mut sub: Option<clap::Command> = None;
    let mut positionals: Vec<&str> = Vec::new();
    let mut escaped = false;
    let mut index = 1; // words[0] is the binary name

    while index < words.len() {
        let Some(word) = words[index] else {
            // The cursor. Never context: not a subcommand, not a positional.
            if sub.is_none() {
                return Line::default();
            }
            index += 1;
            continue;
        };
        index += 1;

        if !escaped && word == "--" {
            escaped = true;
            continue;
        }
        if escaped || !word.starts_with('-') || word == "-" {
            match &sub {
                Some(_) => positionals.push(word),
                None => match root.get_subcommands().find(|c| c.get_name() == word) {
                    Some(found) => {
                        line.subcommand = Some(word.to_string());
                        sub = Some(found.clone());
                    }
                    None => return Line::default(),
                },
            }
            continue;
        }

        let (arg, attached) = match word.strip_prefix("--") {
            Some(rest) => match rest.split_once('=') {
                Some((name, value)) => (find_long(&root, sub.as_ref(), name), Some(value)),
                None => (find_long(&root, sub.as_ref(), rest), None),
            },
            None => {
                let rest = &word[1..];
                let Some(short) = rest.chars().next() else {
                    continue;
                };
                let tail = &rest[short.len_utf8()..];
                let attached = (!tail.is_empty()).then(|| tail.strip_prefix('=').unwrap_or(tail));
                (find_short(&root, sub.as_ref(), short), attached)
            }
        };
        let Some(arg) = arg else { continue };

        let mut values: Vec<&str> = Vec::new();
        if arg.get_action().takes_values() {
            match attached {
                Some(value) => values.push(value),
                None => {
                    let max = arg.get_num_args().map_or(1, |range| range.max_values());
                    while values.len() < max && index < words.len() {
                        let Some(next) = words[index] else {
                            // The option's value is the word being completed. That is the
                            // ordinary case for every option-valued completion, not
                            // ambiguity: stop consuming, and keep everything the walk has
                            // already learned. `-C <cursor>` records no directory and
                            // falls back to the process's own, which is what the spec asks.
                            break;
                        };
                        if next.starts_with('-') && next != "-" {
                            break;
                        }
                        values.push(next);
                        index += 1;
                    }
                }
            }
        }
        record(&mut line, arg, &values);
    }

    if let Some(name) = &line.subcommand
        && ID_FIRST.contains(&name.as_str())
        && let Some(first) = positionals.first()
    {
        line.subject = TaskId::parse(first).ok();
    }
    line
}

/// The subcommand's arg, else the root's — where the globals `-C`, `--pretty`, `--color`
/// are declared.
fn find_long<'a>(
    root: &'a clap::Command,
    sub: Option<&'a clap::Command>,
    name: &str,
) -> Option<&'a clap::Arg> {
    let matches = |arg: &&clap::Arg| arg.get_long() == Some(name);
    sub.and_then(|sub| sub.get_arguments().find(matches))
        .or_else(|| root.get_arguments().find(matches))
}

fn find_short<'a>(
    root: &'a clap::Command,
    sub: Option<&'a clap::Command>,
    short: char,
) -> Option<&'a clap::Arg> {
    let matches = |arg: &&clap::Arg| arg.get_short() == Some(short);
    sub.and_then(|sub| sub.get_arguments().find(matches))
        .or_else(|| root.get_arguments().find(matches))
}

fn record(line: &mut Line, arg: &clap::Arg, values: &[&str]) {
    match arg.get_id().as_str() {
        "dir" => {
            if let Some(value) = values.last() {
                line.dir = Some(PathBuf::from(value));
            }
        }
        "project" => {
            if let Some(value) = values.last() {
                line.project = Some((*value).to_string());
            }
        }
        "all_projects" => line.all_projects = true,
        _ => {}
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin tasks -- complete::tests`
Expected: PASS, 8 tests.

- [ ] **Step 5: Run the gate and commit**

Run: `just gate`
Expected: exit 0.

```bash
git add src/complete.rs
git commit -m "feat(complete): walk the transport argv for -C, --project, and the subject id"
```

---

### Task 3: Task ids for show, root, and the id-taking writes

Adds the scan-and-present path and the `IdDirected` scope: local first, foreign by typed prefix.

**Files:**
- Modify: `src/complete.rs`
- Modify: `src/cli.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Line`/`line()` from Task 2; `crate::repo::Project`, `crate::scope::{is_reachable, open_registered, Origin}`, `crate::model::is_valid_prefix`.
- Produces: `complete::id_directed(&OsStr) -> Vec<CompletionCandidate>`; internal `open_local(&Line)`, `open_prefix(&Registry, &str)`, `typed_prefix(&str)`, `local_or_foreign(&Registry, Option<Project>, &str)`, `described(&str, Option<&Task>)`, `candidates(Vec<Task>, &str)` for Task 4. `TestEnv::complete_values(&self, dir: &Path, shell: &str, index: usize, words: &[&str]) -> Vec<String>`.

- [ ] **Step 1: Add the value-only test helper**

Completing a **bare positional with an empty word** also offers the still-unused global
flags — `-C`, `--pretty`, `--color`, `--help` — after the positional's own candidates.
That is correct behavior (`tasks show --help` is a real command line) and it does not
happen when completing a flag's value, nor when the word is non-empty, because the typed
text filters the flags out. Every assertion in this task is about the id candidates, so
add a helper that drops flag-shaped candidates. Put it in `tests/common/mod.rs`, right
after `complete`:

```rust
    /// `complete`, minus the flag candidates. Completing a bare positional with an empty
    /// word also offers the flags still available on that command line, which is correct
    /// and irrelevant to every assertion about ids: a task id never starts with `-`.
    pub fn complete_values(
        &self,
        dir: &Path,
        shell: &str,
        index: usize,
        words: &[&str],
    ) -> Vec<String> {
        self.complete(dir, shell, index, words)
            .into_iter()
            .filter(|candidate| !candidate.starts_with('-'))
            .collect()
    }
```

Then replace the inline filter Task 1 left in
`completion_offers_the_fixed_value_sets_and_registry_prefixes` (the `unregister` case,
which hit this first) with a call to the new helper, so one mechanism covers every site:

```rust
    assert_eq!(
        env.complete_values(&sci, "bash", 2, &["tasks", "unregister", ""]),
        ["fam", "sci"]
    );
```

- [ ] **Step 2: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn completion_offers_task_ids_open_first_with_descriptions() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let open = id_of(env.json(&sci, &["add", "Still open"]));
    let closed = id_of(env.json(&sci, &["add", "Finished"]));
    env.json(&sci, &["done", &closed, "landed"]);

    // open before closed, whatever the ids sort to
    let ids = env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]);
    assert_eq!(ids, [open.clone(), closed.clone()], "open task must come first");

    // the prefix filters
    assert_eq!(
        env.complete(&sci, "bash", 2, &["tasks", "show", &open[..5]]),
        [open.clone()]
    );

    // zsh carries `value:description`; bash emits bare values
    let described = env.complete_values(&sci, "zsh", 2, &["tasks", "show", ""]);
    assert_eq!(described[0], format!("{open}:todo  Still open"));
    assert_eq!(described[1], format!("{closed}:done  Finished"));

    // every id-taking write uses the same source
    for command in ["edit", "note", "start", "done", "drop", "block", "unblock", "dep", "root"] {
        let ids = env.complete_values(&sci, "bash", 2, &["tasks", command, ""]);
        assert!(ids.contains(&open), "{command}: {ids:?}");
    }
}

#[test]
fn completion_follows_a_typed_prefix_and_prefers_the_local_checkout() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    let foreign = id_of(env.json(&fam, &["add", "Foreign"]));

    // a foreign prefix reaches that project
    assert_eq!(env.complete(&sci, "bash", 2, &["tasks", "note", "fam-"]), [foreign.clone()]);
    // the local one stays local
    assert_eq!(env.complete(&sci, "bash", 2, &["tasks", "note", "sci-"]), [local.clone()]);

    // a second root under the same prefix holds different tasks; standing in it, the
    // local checkout wins over the registered root
    let displaced = env.init_forced("sci");
    let displaced_id = id_of(env.json(&displaced, &["add", "Displaced"]));
    assert_eq!(
        env.complete(&displaced, "bash", 2, &["tasks", "note", "sci-"]),
        [displaced_id.clone()]
    );

    // -C selects the project, overriding the process's directory
    let dir = displaced.to_str().unwrap();
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "-C", dir, "note", "sci-"]),
        [displaced_id.clone()]
    );
    assert_eq!(
        env.complete(&sci, "bash", 3, &["tasks", &format!("-C{dir}"), "note", "sci-"]),
        [displaced_id]
    );

    // `root` resolves through the registry and needs no local project
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.complete(nowhere.path(), "bash", 2, &["tasks", "root", "fam-"]),
        [foreign]
    );
}

#[test]
fn completion_is_silent_when_anything_is_wrong() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.json(&sci, &["add", "Fine"]);
    let nowhere = tempfile::tempdir().unwrap();

    // outside every project
    assert!(env.complete_values(nowhere.path(), "bash", 2, &["tasks", "show", ""]).is_empty());
    // an unregistered prefix
    assert!(env.complete_values(&sci, "bash", 2, &["tasks", "show", "zzz-"]).is_empty());
    // a malformed task file
    std::fs::write(sci.join("tasks/sci-bad001.md"), "not a task").unwrap();
    env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]);
    // a malformed registry
    std::fs::write(env.home.path().join(".config/tasks/projects.toml"), "not toml = [").unwrap();
    env.complete_values(&sci, "bash", 2, &["tasks", "show", ""]);
    // an unreadable tasks directory
    let locked = env.init("lck");
    let mut perms = std::fs::metadata(locked.join("tasks")).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o000);
    std::fs::set_permissions(locked.join("tasks"), perms.clone()).unwrap();
    let out = env.complete_values(&locked, "bash", 2, &["tasks", "show", ""]);
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(locked.join("tasks"), perms).unwrap();
    assert!(out.is_empty(), "{out:?}");
}
```

`TestEnv::complete` already asserts exit 0 and an empty stderr, so every case above proves silence rather than an error.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --test cli -- completion_offers_task_ids completion_follows_a_typed completion_is_silent`
Expected: FAIL — empty candidate lists (nothing is wired to an id completer yet).

- [ ] **Step 4: Implement the scan and presentation**

Add to the imports in `src/complete.rs`:

```rust
use std::ffi::OsStr;

use crate::model::Task;
use crate::repo::Project;
use crate::scope::Origin;
```

Add above the `#[cfg(test)]` block:

```rust
/// The project at the effective directory, if there is one.
fn open_local(line: &Line) -> Option<Project> {
    let dir = match &line.dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir().ok()?,
    };
    Project::locate(&dir).ok()
}

/// A registered project, if the registry knows it and it is reachable.
fn open_prefix(registry: &Registry, prefix: &str) -> Option<Project> {
    let root = registry.project_root(prefix)?;
    if !crate::scope::is_reachable(root).unwrap_or(false) {
        return None;
    }
    crate::scope::open_registered(registry, prefix, Origin::Prefix).ok()
}

/// The prefix the user has typed ahead of `-`, when it is a well-formed one.
fn typed_prefix(current: &str) -> Option<&str> {
    let (prefix, _) = current.split_once('-')?;
    crate::model::is_valid_prefix(prefix).then_some(prefix)
}

/// Local first, foreign by typed prefix — the write-side rule in §6 of
/// `docs/specs/2026-08-29-tasks-design.md`. A prefix equal to `base`'s completes from
/// `base`, so a worktree beats the registered root of the same project.
fn local_or_foreign(registry: &Registry, base: Option<Project>, current: &str) -> Vec<Task> {
    if let Some(prefix) = typed_prefix(current)
        && base.as_ref().is_none_or(|project| project.prefix != prefix)
        && let Some(foreign) = open_prefix(registry, prefix)
    {
        return foreign.scan().unwrap_or_default();
    }
    base.map(|project| project.scan().unwrap_or_default())
        .unwrap_or_default()
}

/// An id, described when its task can be read. `dep --rm` offers dependencies that may
/// be unreachable, so a missing task drops the description, never the candidate.
fn described(id: &str, task: Option<&Task>) -> CompletionCandidate {
    let help = task.map(|task| {
        clap::builder::StyledStr::from(format!("{}  {}", task.status.as_str(), task.title))
    });
    CompletionCandidate::new(id).help(help)
}

/// Filter to the typed fragment, open tasks first, each group by id.
fn candidates(tasks: Vec<Task>, current: &str) -> Vec<CompletionCandidate> {
    let mut matching: Vec<Task> = tasks
        .into_iter()
        .filter(|task| task.id.to_string().starts_with(current))
        .collect();
    matching.sort_by(|a, b| {
        b.status
            .is_open()
            .cmp(&a.status.is_open())
            .then_with(|| a.id.cmp(&b.id))
    });
    matching
        .iter()
        .map(|task| described(&task.id.to_string(), Some(task)))
        .collect()
}

/// `show`, `root`, and every id-taking write.
pub fn id_directed(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    candidates(local_or_foreign(&registry, open_local(&line), current), current)
}
```

- [ ] **Step 5: Attach the completer**

In `src/cli.rs`, add `ArgValueCompleter` to the import:

```rust
use clap_complete::{ArgValueCandidates, ArgValueCompleter};
```

Give the `id` positional of `Show`, `Root`, `Edit`, `Note`, `Start`, `Done`, `Drop`, `Block`, and `Unblock`, and of `Dep`, this attribute (these positionals have no attribute today; `Edit`'s and `Dep`'s sit above their existing fields):

```rust
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
```

For the ones declared inline as `Show { id: String }` and `Root { id: String }`, expand them to the braced form with the attribute on `id`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --test cli -- completion_offers_task_ids completion_follows_a_typed completion_is_silent`
Expected: PASS, 3 tests.

- [ ] **Step 7: Run the gate and commit**

Run: `just gate`
Expected: exit 0.

```bash
git add src/complete.rs src/cli.rs tests/cli.rs
git commit -m "feat(complete): complete task ids, local first and foreign by typed prefix"
```

---

### Task 4: The remaining five scopes

`Scoped`, `Destination`, `Resolvable`, `Dependencies`, and `UpstreamFeedback`, each on the arguments whose execution rules they match.

**Files:**
- Modify: `src/complete.rs`
- Modify: `src/cli.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: everything Task 3 produced.
- Produces: `complete::scoped`, `complete::destination_ids`, `complete::resolvable`, `complete::dependencies`, `complete::upstream_feedback` — each `fn(&OsStr) -> Vec<CompletionCandidate>`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn completion_scopes_ids_to_what_each_argument_accepts() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let local = id_of(env.json(&sci, &["add", "Local"]));
    let foreign = id_of(env.json(&fam, &["add", "Foreign"]));

    // Scoped: tree and list --parent are local, until --all-projects widens list
    assert!(env.complete_values(&sci, "bash", 2, &["tasks", "tree", "fam-"]).is_empty());
    assert_eq!(env.complete(&sci, "bash", 3, &["tasks", "list", "--parent", ""]), [local.clone()]);
    let mut wide = env.complete(&sci, "bash", 4, &["tasks", "list", "--all-projects", "--parent", ""]);
    wide.sort();
    let mut both = [foreign.clone(), local.clone()];
    both.sort();
    assert_eq!(wide, both, "--all-projects widens the scope to the registry");

    // Destination: --parent follows --project, and the subject id on edit
    assert_eq!(
        env.complete(&sci, "bash", 6, &["tasks", "add", "T", "--project", "fam", "--parent", ""]),
        [foreign.clone()]
    );
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "edit", &foreign, "--parent", ""]),
        [foreign.clone()]
    );
    // add's title is not a subject: --parent stays local
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", &foreign, "--parent", ""]),
        [local.clone()]
    );

    // Resolvable: --depends and dep --on reach any registered project
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "add", "T", "--depends", "fam-"]),
        [foreign.clone()]
    );
    assert_eq!(
        env.complete(&sci, "bash", 4, &["tasks", "dep", &local, "--on", "fam-"]),
        [foreign.clone()]
    );
}

#[test]
fn resolvable_starts_from_the_destination_project() {
    let mut env = TestEnv::new();
    let fam = env.init("fam");
    let registered = id_of(env.json(&fam, &["add", "In the registered root"]));

    // a second `fam` root with different tasks: `add --project fam` validates against the
    // registered root, so completion must offer that one's ids, not this checkout's
    let worktree = env.init_forced("fam");
    env.json(&worktree, &["add", "Only in the worktree"]);
    // `init --force` repointed the registry; put it back so `fam` names the first root
    env.json(&fam, &["init", "--prefix", "fam", "--force"]);

    assert_eq!(
        env.complete(&worktree, "bash", 6, &["tasks", "add", "T", "--project", "fam", "--depends", "fam-"]),
        [registered.clone()]
    );

    // and it works with no local project at all
    let nowhere = tempfile::tempdir().unwrap();
    assert_eq!(
        env.complete(nowhere.path(), "bash", 6, &["tasks", "add", "T", "--project", "fam", "--depends", "fam-"]),
        [registered]
    );
}

#[test]
fn dep_rm_offers_only_current_dependencies_including_unreachable_ones() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let subject = id_of(env.json(&sci, &["add", "Subject"]));
    let other = id_of(env.json(&sci, &["add", "Not a dependency"]));
    let reachable = id_of(env.json(&fam, &["add", "Reachable dep"]));
    env.json(&sci, &["dep", &subject, "--on", &reachable]);

    let ids = env.complete(&sci, "bash", 4, &["tasks", "dep", &subject, "--rm", ""]);
    assert_eq!(ids, [reachable.clone()]);
    assert!(!ids.contains(&other), "{ids:?}");

    // unregister fam: the dependency is now unreachable but still removable, so it stays
    // a candidate — described in zsh only while its task can be read
    let zsh = env.complete(&sci, "zsh", 4, &["tasks", "dep", &subject, "--rm", ""]);
    assert_eq!(zsh, [format!("{reachable}:todo  Reachable dep")]);
    env.json(&sci, &["unregister", "fam"]);
    assert_eq!(
        env.complete(&sci, "zsh", 4, &["tasks", "dep", &subject, "--rm", ""]),
        [reachable],
        "an unreachable dependency is offered bare, not dropped"
    );
}

#[test]
fn feedback_recur_offers_open_feedback_from_the_registered_tasks_root() {
    let mut env = TestEnv::new();
    let upstream = env.init("tasks");
    let sci = env.init("sci");
    let open = id_of(env.json(&upstream, &["add", "Open report", "--tag", "feedback"]));
    let closed = id_of(env.json(&upstream, &["add", "Closed report", "--tag", "feedback"]));
    env.json(&upstream, &["done", &closed, "fixed"]);
    let untagged = id_of(env.json(&upstream, &["add", "Not feedback"]));

    let ids = env.complete(&sci, "bash", 4, &["tasks", "feedback", "S", "--recur", ""]);
    assert_eq!(ids, [open.clone()]);
    assert!(!ids.contains(&closed) && !ids.contains(&untagged), "{ids:?}");

    // a worktree of the upstream does not leak its own records
    let worktree = env.init_forced("tasks");
    let only_here = id_of(env.json(&worktree, &["add", "Local only", "--tag", "feedback"]));
    env.json(&upstream, &["init", "--prefix", "tasks", "--force"]);
    let ids = env.complete(&worktree, "bash", 4, &["tasks", "feedback", "S", "--recur", ""]);
    assert_eq!(ids, [open]);
    assert!(!ids.contains(&only_here), "{ids:?}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli -- completion_scopes resolvable_starts dep_rm_offers feedback_recur_offers`
Expected: FAIL — these arguments have no completer, so every list is empty.

- [ ] **Step 3: Implement the scopes**

Add to `src/complete.rs`, above the `#[cfg(test)]` block:

```rust
/// The project a new or edited task lands in: `--project`, else the subject id's project,
/// else the effective directory's. A subject whose prefix matches the local project keeps
/// the local checkout, as the write path does.
fn destination(registry: &Registry, line: &Line) -> Option<Project> {
    if let Some(prefix) = &line.project {
        return open_prefix(registry, prefix);
    }
    let local = open_local(line);
    let Some(subject) = &line.subject else {
        return local;
    };
    if local
        .as_ref()
        .is_some_and(|project| project.prefix == subject.prefix)
    {
        return local;
    }
    open_prefix(registry, &subject.prefix)
}

/// `tree <id>` and `list --parent`: the scope the command itself scans, which
/// `--all-projects` widens to the registry (`list.rs` validates the parent against it).
pub fn scoped(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let mut tasks = Vec::new();
    if line.all_projects {
        for prefix in registry.projects.keys() {
            if let Some(project) = open_prefix(&registry, prefix) {
                tasks.extend(project.scan().unwrap_or_default());
            }
        }
    } else if let Some(project) = open_local(&line) {
        tasks.extend(project.scan().unwrap_or_default());
    }
    candidates(tasks, current)
}

/// `--parent`: a parent must live in the same project as its child.
pub fn destination_ids(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let tasks = destination(&registry, &line)
        .map(|project| project.scan().unwrap_or_default())
        .unwrap_or_default();
    candidates(tasks, current)
}

/// `--depends` and `dep --on`: whatever `Resolver` can reach *from the destination*.
/// `apply_fields` and `dep::run` both build their `Resolver` on the project being written
/// to, so a worktree's own ids are the ones `add --project` would reject.
pub fn resolvable(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let base = destination(&registry, &line);
    candidates(local_or_foreign(&registry, base, current), current)
}

/// `dep --rm`: only what the task already depends on. Removal does not resolve ids, so an
/// unreachable dependency stays a candidate and simply loses its description.
pub fn dependencies(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let (Some(subject), Some(project)) = (&line.subject, destination(&registry, &line)) else {
        return Vec::new();
    };
    let Ok(task) = project.read_task(subject) else {
        return Vec::new();
    };
    let resolver = crate::resolve::Resolver::new(&project, &registry);
    task.depends
        .iter()
        .filter(|id| id.to_string().starts_with(current))
        .map(|id| {
            let found = resolver.resolve_task(id).ok().flatten();
            described(&id.to_string(), found.as_ref())
        })
        .collect()
}

/// `feedback --recur`: open, `feedback`-tagged tasks in the project registered as
/// `tasks`, never the local directory — `feedback::locate_target` resolves the same way,
/// so a worktree of the upstream must not suggest records it has not pushed.
pub fn upstream_feedback(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let registry = Registry::load().unwrap_or_default();
    let Some(project) = open_prefix(&registry, crate::commands::feedback::TARGET_PREFIX) else {
        return Vec::new();
    };
    let tasks = project
        .scan()
        .unwrap_or_default()
        .into_iter()
        .filter(crate::commands::feedback::is_open_feedback)
        .collect();
    candidates(tasks, current)
}
```

`is_open_feedback` takes `&Task`, so `filter` receives `&&Task`; if the compiler objects, write `.filter(|task| crate::commands::feedback::is_open_feedback(task))`.

- [ ] **Step 4: Attach the completers**

In `src/cli.rs`:

- `FieldArgs::parent` → add `add = ArgValueCompleter::new(crate::complete::destination_ids)`
- `FieldArgs::depends` → add `add = ArgValueCompleter::new(crate::complete::resolvable)`
- `Command::List::parent` → add `add = ArgValueCompleter::new(crate::complete::scoped)`
- `Command::Tree::id` → give the positional `#[arg(add = ArgValueCompleter::new(crate::complete::scoped))]`
- `Command::Dep::on` → add `add = ArgValueCompleter::new(crate::complete::resolvable)`
- `Command::Dep::rm` → add `add = ArgValueCompleter::new(crate::complete::dependencies)`
- `Command::Feedback::recur` → add `add = ArgValueCompleter::new(crate::complete::upstream_feedback)`

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test cli -- completion_scopes resolvable_starts dep_rm_offers feedback_recur_offers`
Expected: PASS, 4 tests.

- [ ] **Step 6: Run the gate and commit**

Run: `just gate`
Expected: exit 0, all tests pass.

```bash
git add src/complete.rs src/cli.rs tests/cli.rs
git commit -m "feat(complete): scope id candidates to what each argument accepts"
```

---

### Task 5: Document the integration and close the task

**Files:**
- Modify: `README.md`
- Modify: `skills/tasks/SKILL.md`
- Modify: `docs/specs/2026-08-29-tasks-design.md`
- Modify: `docs/specs/2026-09-05-shell-completions-design.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add the README section**

In `README.md`, after the `Run \`tasks --help\` for the full command list.` line, add:

```markdown
## Completions

Bash and zsh complete subcommands, flags, `--status`/`--size`/`--sort`/`--color`
values, registered project prefixes, and task ids — `tasks show sci-4f<TAB>`. The id
candidates come from the project the command would actually act on, so `-C`,
`--project`, and a typed foreign prefix all steer them.

Bash, in `~/.bashrc`:

    source <(TASKS_COMPLETE=bash tasks)

Zsh, in `~/.zshrc`, **after** completion is initialized — the stub calls `compdef`, so
sourcing it before `compinit` fails with `command not found: compdef`:

    autoload -Uz compinit && compinit      # or your framework's own init
    source <(TASKS_COMPLETE=zsh tasks)

Under oh-my-zsh, prezto, or a plugin manager, the same rule applies: the stub goes after
that framework's initialization.

Re-source the stub or open a new shell after upgrading `tasks`; a running shell keeps the
function it loaded at startup. Fish, elvish, and powershell use the same mechanism with
their own syntax. `TASKS_COMPLETE=` or `TASKS_COMPLETE=0` disables completion.
```

- [ ] **Step 2: Add the SKILL.md line**

In `skills/tasks/SKILL.md`, in the "Recording work" list, after the `tasks tree [<id>]` bullet:

```markdown
- Tab completion for ids and flags: `source <(TASKS_COMPLETE=bash tasks)` in `~/.bashrc`,
  or the same with `zsh` in `~/.zshrc` after `compinit`. See the README.
```

- [ ] **Step 3: Note it in the design spec's CLI section**

In `docs/specs/2026-08-29-tasks-design.md`, at the end of the `## 6. Configuration and registry` section (just before `### 6.1 Cycle detection`), add:

```markdown
Shell completion is activated by `TASKS_COMPLETE=<shell> tasks`, which prints a stub for a
shell rc to source; the stub calls the binary back on each TAB. Candidates for an id
argument come from the project that argument's command would act on, which is not always
the local one. See docs/specs/2026-09-05-shell-completions-design.md.
```

- [ ] **Step 4: Mark the spec implemented**

In `docs/specs/2026-09-05-shell-completions-design.md`, change the header line
`Status: designed (2026-09-05)` to `Status: implemented (2026-09-05)`.

- [ ] **Step 5: Add the layout entry**

In `AGENTS.md`, in the `## Layout` list, extend the `src/` bullet by adding
`` `complete.rs` (shell completion candidates; best-effort, never errors), `` after
`` `resolve.rs` (spec/plan links), ``.

- [ ] **Step 6: Reinstall and smoke-test**

```bash
cargo install --path .
TASKS_COMPLETE=bash tasks | head -5
```

Expected: the bash stub, mentioning `TASKS_COMPLETE` and `complete -o nospace`.

- [ ] **Step 7: Run the gate and commit**

Run: `just gate`
Expected: exit 0.

```bash
tasks done tasks-ea07fb "bash/zsh completion for commands, flags, value sets, prefixes and task ids via clap_complete CompleteEnv"
git add -A
git commit -m "docs: document shell completions and close tasks-ea07fb"
```

---

## Self-Review

**Spec coverage.** Approach and Wiring → Task 1 (dependency, `.var("TASKS_COMPLETE")`, hook placement). Recovering context → Task 2 (all four walk rules, all three accessors, ambiguity). Candidate scopes → `IdDirected` in Task 3, the other five in Task 4; the "missing local project is never an error" rule is exercised by the `root` and `add --project` from-nowhere cases. What completes → the fixed-set table rows in Task 1, id rows in Tasks 3 and 4. Presenting ids → Task 3 (ordering, descriptions, bash/zsh split) and Task 4 (bare candidate for an unreadable task). Failure is silence → Task 3's silence test covers no project, unregistered prefix, malformed task, malformed registry, unreadable directory. Installation → Task 5, including the `compinit` prerequisite and the re-source-after-upgrade note. Testing → every listed case appears in Tasks 1, 3, or 4. Non-goals → no task adds `--tag`/`--spec`/`--plan`/`--step` completion or a `completions` subcommand.

**Type consistency.** `Line` and `walk`/`line()` are defined in Task 2 and used unchanged in Tasks 3 and 4. `open_local`, `open_prefix`, `typed_prefix`, `local_or_foreign`, `described`, and `candidates` are defined in Task 3 and consumed by Task 4's `destination`, `scoped`, `destination_ids`, `resolvable`, `dependencies`, and `upstream_feedback`. The public completer names in Task 4's `cli.rs` step match the function names in its implementation step. `is_open_feedback` is made `pub` in Task 1, ahead of its use in Task 4.

**Known risk.** `env.init_forced` repoints the registry at the new root, so both Task 4 tests that use it re-run `init --force` on the original to restore the mapping before asserting. If a future change makes `init --force` non-idempotent, those two tests are where it will show.
