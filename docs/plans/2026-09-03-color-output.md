# Color Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in ANSI color to human-readable output without changing default or JSON output.

**Architecture:** A dependency-free `style` module resolves the CLI/environment policy and paints semantic roles with basic ANSI sequences. `main` creates separate stdout and stderr painters from one resolved mode, then passes them into the existing pure output renderers. Existing output data stays typed; styling happens only after table fields are padded.

**Tech Stack:** Rust 2024 edition, clap derive, serde, `std::io::IsTerminal`, literal ANSI SGR sequences. Tests use the existing in-module unit tests and `assert_cmd` end-to-end harness.

**Spec:** `docs/specs/2026-09-03-color-output-design.md` (read it first; each task cites its sections).

## Global Constraints

- Gates before every commit: `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `tasks check`.
- Rebuild and reinstall after CLI changes: `cargo install --path .`.
- JSON output is byte-for-byte unchanged and never contains ANSI escapes, including with `--color always`.
- Color is reachable only with `--pretty`; unset color mode is `never`.
- Precedence is explicit `--color`, then non-empty `NO_COLOR`, then `TASKS_COLOR`, then `never`; `TASKS_COLOR` is validated whenever present.
- `auto` tests the stream being written, so stdout and stderr receive separate painters.
- Use only basic ANSI colors plus bold and dim. Add no dependency and no palette configuration.
- Pad table fields before painting them. ANSI bytes must not affect visible alignment.
- Do not color graph output or the serialized task text at the start of `show --pretty`.
- Each task below is one tracker task; close it with `tasks done <id> "<what landed>"` in its commit.
- The final task updates the design status and closes parent goal `tasks-4737b6` in the same commit.
- Conventional commits; no AI-attribution trailers.

---

### Task 1: Add color policy and stream-specific painters

Tracker: `tasks-f412d4`. Spec §2, §2.1, §3 severity, §4 stream handling, §5 mode tests.

**Files:**
- Create: `src/style.rs`
- Modify: `src/main.rs`, `src/cli.rs`, `src/output.rs`, `tests/common/mod.rs`
- Test: `src/style.rs`, `tests/cli.rs`

**Interfaces:**
- Produces: `style::ColorMode::{Auto, Always, Never}`; `ColorMode::parse(value: &str) -> Result<ColorMode>`; `ColorMode::resolve(flag: Option<&str>, configured: Option<&str>, no_color: bool) -> Result<ColorMode>`; `style::Style::{Status, Chrome, Emphasis, Error, Ok, Warning}`; `Painter::new(mode: ColorMode, format: Format, stream_is_terminal: bool) -> Painter`; `Painter::paint(&self, style: Style, text: &str) -> String`.
- Changes: `output::render(out: &Output, format: Format, painter: &Painter) -> String`; `output::pretty_warnings(warnings: &[String], painter: &Painter) -> String`.
- Preserves: `output::render_error` stays plain JSON and does not take a painter.

- [ ] **Step 1: Claim the tracker task**

Run:

```bash
tasks start tasks-f412d4
```

Expected: the task becomes `doing` with the current worktree owner.

- [ ] **Step 2: Write failing unit tests for policy and painting**

Create `src/style.rs`, declare it with `mod style;` in `src/main.rs`, and start with tests that establish the complete policy and palette:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_precedence_and_validates_config_even_when_overridden() {
        assert_eq!(ColorMode::resolve(None, None, false).unwrap(), ColorMode::Never);
        assert_eq!(ColorMode::resolve(None, Some("auto"), false).unwrap(), ColorMode::Auto);
        assert_eq!(ColorMode::resolve(None, Some("always"), true).unwrap(), ColorMode::Never);
        assert_eq!(
            ColorMode::resolve(Some("always"), Some("never"), true).unwrap(),
            ColorMode::Always
        );
        assert_eq!(
            ColorMode::resolve(Some("chartreuse"), None, false)
                .unwrap_err()
                .kind(),
            "validation"
        );
        assert_eq!(
            ColorMode::resolve(Some("never"), Some("chartreuse"), false)
                .unwrap_err()
                .kind(),
            "config"
        );
    }

    #[test]
    fn painter_obeys_format_mode_stream_and_roles() {
        let plain = Painter::new(ColorMode::Always, Format::Json, true);
        assert_eq!(plain.paint(Style::Error, "error"), "error");

        let redirected = Painter::new(ColorMode::Auto, Format::Pretty, false);
        assert_eq!(redirected.paint(Style::Warning, "warning:"), "warning:");

        let terminal = Painter::new(ColorMode::Auto, Format::Pretty, true);
        assert_eq!(terminal.paint(Style::Warning, "warning:"), "\x1b[33mwarning:\x1b[0m");

        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        for (style, code) in [
            (Style::Status(Status::Idea), "34"),
            (Style::Status(Status::Doing), "33"),
            (Style::Status(Status::Blocked), "31"),
            (Style::Status(Status::Done), "2;32"),
            (Style::Status(Status::Dropped), "2;31"),
            (Style::Chrome, "2"),
            (Style::Emphasis, "1"),
            (Style::Error, "31"),
            (Style::Ok, "32"),
            (Style::Warning, "33"),
        ] {
            let painted = colored.paint(style, "x");
            assert_eq!(painted, format!("\x1b[{code}mx\x1b[0m"));
            assert!(painted.ends_with("\x1b[0m"));
        }
        assert_eq!(colored.paint(Style::Status(Status::Todo), "todo"), "todo");
    }
}
```

- [ ] **Step 3: Write failing end-to-end tests for public color selection**

First extend `TestEnv::cmd` in `tests/common/mod.rs` so every test begins from a deterministic environment:

```rust
.env_remove("TASKS_COLOR")
.env_remove("NO_COLOR")
```

Then add focused tests to `tests/cli.rs`:

```rust
fn has_ansi(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|pair| pair == b"\x1b[")
}

#[test]
fn color_is_opt_in_and_never_reaches_json() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    for args in [
        &["--pretty", "check"][..],
        &["--pretty", "--color", "auto", "check"][..],
    ] {
        let out = env.cmd(&dir).args(args).output().unwrap();
        assert!(out.status.success());
        assert!(!has_ansi(&out.stdout), "{args:?}");
    }

    let colored = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "check"])
        .output()
        .unwrap();
    assert!(has_ansi(&colored.stdout));

    let json = env
        .cmd(&dir)
        .args(["--color", "always", "check"])
        .output()
        .unwrap();
    assert!(!has_ansi(&json.stdout));
    serde_json::from_slice::<serde_json::Value>(&json.stdout).unwrap();
}

#[test]
fn no_color_suppresses_config_but_an_explicit_flag_wins() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");

    let suppressed = env
        .cmd(&dir)
        .env("TASKS_COLOR", "always")
        .env("NO_COLOR", "1")
        .args(["--pretty", "check"])
        .output()
        .unwrap();
    assert!(!has_ansi(&suppressed.stdout));

    let overridden = env
        .cmd(&dir)
        .env("TASKS_COLOR", "never")
        .env("NO_COLOR", "1")
        .args(["--pretty", "--color", "always", "check"])
        .output()
        .unwrap();
    assert!(has_ansi(&overridden.stdout));
}

#[test]
fn tasks_color_is_always_validated_and_warnings_use_the_stderr_painter() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [
        &["check"][..],
        &["--pretty", "check"][..],
        &["--pretty", "--color", "never", "check"][..],
    ] {
        let out = env
            .cmd(&dir)
            .env("TASKS_COLOR", "chartreuse")
            .env("NO_COLOR", "1")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config");
    }

    let fresh = tempfile::tempdir().unwrap();
    let warned = env
        .cmd(fresh.path())
        .args(["--pretty", "--color", "always", "init", "--prefix", "warn"])
        .output()
        .unwrap();
    assert!(warned.status.success());
    assert!(
        String::from_utf8_lossy(&warned.stderr).starts_with("\x1b[33mwarning:\x1b[0m ")
    );
}
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
cargo test style::tests
cargo test color_is_opt_in_and_never_reaches_json
```

Expected: compilation fails because `ColorMode`, `Style`, `Painter`, and `--color` are not implemented.

- [ ] **Step 5: Implement the policy, painter, and stream plumbing**

Implement `src/style.rs` with no dependency:

```rust
use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn parse(value: &str) -> Result<ColorMode> {
        match value {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            other => Err(Error::Validation(format!(
                "color mode must be auto, always, or never, got {other:?}"
            ))),
        }
    }

    pub fn resolve(
        flag: Option<&str>,
        configured: Option<&str>,
        no_color: bool,
    ) -> Result<ColorMode> {
        let flag = flag.map(ColorMode::parse).transpose()?;
        let configured = configured
            .map(|value| {
                ColorMode::parse(value).map_err(|_| {
                    Error::Config(format!(
                        "TASKS_COLOR must be auto, always, or never, got {value:?}"
                    ))
                })
            })
            .transpose()?;
        Ok(match (flag, no_color, configured) {
            (Some(mode), _, _) => mode,
            (None, true, _) => ColorMode::Never,
            (None, false, Some(mode)) => mode,
            (None, false, None) => ColorMode::Never,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Style {
    Status(Status),
    Chrome,
    Emphasis,
    Error,
    Ok,
    Warning,
}

pub struct Painter {
    enabled: bool,
}

impl Painter {
    pub fn new(mode: ColorMode, format: Format, stream_is_terminal: bool) -> Painter {
        Painter {
            enabled: format == Format::Pretty
                && match mode {
                    ColorMode::Auto => stream_is_terminal,
                    ColorMode::Always => true,
                    ColorMode::Never => false,
                },
        }
    }

    pub fn paint(&self, style: Style, text: &str) -> String {
        let code = match style {
            Style::Status(Status::Idea) => "34",
            Style::Status(Status::Todo) => return text.into(),
            Style::Status(Status::Doing) => "33",
            Style::Status(Status::Blocked) => "31",
            Style::Status(Status::Done) => "2;32",
            Style::Status(Status::Dropped) => "2;31",
            Style::Chrome => "2",
            Style::Emphasis => "1",
            Style::Error => "31",
            Style::Ok => "32",
            Style::Warning => "33",
        };
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.into()
        }
    }
}
```

Add `color: Option<String>` as a global clap argument beside `pretty`. In `main`, read `TASKS_COLOR` before `commands::run`; map `VarError::NotUnicode` to `Error::Config` instead of treating it as absent. Resolve the mode and render a typed error exactly like the existing `TASKS_FORMAT` branch. Detect non-empty `NO_COLOR` without requiring Unicode:

```rust
let tasks_color = match std::env::var("TASKS_COLOR") {
    Ok(value) => Some(value),
    Err(std::env::VarError::NotPresent) => None,
    Err(std::env::VarError::NotUnicode(value)) => {
        let error = error::Error::Config(format!(
            "TASKS_COLOR must be valid UTF-8, got {value:?}"
        ));
        eprintln!("{}", output::render_error(&error));
        std::process::exit(1);
    }
};
let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
let color_mode = match style::ColorMode::resolve(
    cli.color.as_deref(),
    tasks_color.as_deref(),
    no_color,
) {
    Ok(mode) => mode,
    Err(error) => {
        eprintln!("{}", output::render_error(&error));
        std::process::exit(1);
    }
};
```

Import `std::io::IsTerminal` and construct:

```rust
let stdout_painter = style::Painter::new(color_mode, format, std::io::stdout().is_terminal());
let stderr_painter = style::Painter::new(color_mode, format, std::io::stderr().is_terminal());
```

Render configuration failures through the existing `output::render_error`. Pass the stdout painter into `output::render` and the stderr painter into `output::pretty_warnings`. In `output.rs`, color only `check`'s complete `error:` lines and `ok` line, and only the literal `warning:` prefix on stderr. Leave command errors plain JSON.

- [ ] **Step 6: Run focused and full verification**

Run:

```bash
cargo test style::tests
cargo test color_is_opt_in_and_never_reaches_json
cargo test no_color_suppresses_config_but_an_explicit_flag_wins
cargo test tasks_color_is_always_validated_and_warnings_use_the_stderr_painter
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo install --path .
tasks check
```

Expected: every command exits 0 and all tests pass.

- [ ] **Step 7: Close and commit Task 1**

Mark this task's completed checkboxes `[x]`, then run:

```bash
tasks done tasks-f412d4 "added color policy, ANSI roles, per-stream painters, and isolated mode tests"
git add src/style.rs src/main.rs src/cli.rs src/output.rs tests/common/mod.rs tests/cli.rs docs/plans/2026-09-03-color-output.md tasks/tasks-f412d4.md
git commit -m "feat: add color policy and painters"
```

---

### Task 2: Color tables, trees, and prime output

Tracker: `tasks-1bdfed`. Depends on Task 1. Spec §3, §4 padding rule, §5 layout test.

**Files:**
- Modify: `src/output.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Painter::paint(&self, Style, &str) -> String`; `Style::{Status, Chrome, Emphasis}`; the painter-aware `output::render` from Task 1.
- Changes: `table(rows: &[TaskSummary], painter: &Painter) -> String`; `tree_text(nodes: &[TreeNode], depth: usize, painter: &Painter) -> String`.
- Preserves: visible table text and spacing after ANSI SGR sequences are removed.

- [ ] **Step 1: Claim the tracker task**

Run:

```bash
tasks start tasks-1bdfed
```

Expected: the task becomes `doing`.

- [ ] **Step 2: Write failing table and prime tests**

Add this dependency-free test helper to `tests/cli.rs` beside `has_ansi`:

```rust
fn strip_ansi(text: &str) -> String {
    [
        "\x1b[0m", "\x1b[1m", "\x1b[2m", "\x1b[31m", "\x1b[32m", "\x1b[33m",
        "\x1b[34m", "\x1b[2;31m", "\x1b[2;32m",
    ]
    .into_iter()
    .fold(text.to_string(), |text, code| text.replace(code, ""))
}
```

Add one end-to-end test that exercises every status and verifies layout identity:

```rust
#[test]
fn colored_tables_use_semantic_roles_without_changing_layout() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Idea", "--status", "idea", "--tag", "later"]);
    env.json(&dir, &["add", "Todo", "-p", "0", "--tag", "now"]);
    let doing = id_of(env.json(&dir, &["add", "Doing"]));
    env.json(&dir, &["start", &doing]);
    let blocked = id_of(env.json(&dir, &["add", "Blocked"]));
    env.json(&dir, &["block", &blocked]);
    let done = id_of(env.json(&dir, &["add", "Done"]));
    env.json(&dir, &["done", &done]);
    let dropped = id_of(env.json(&dir, &["add", "Dropped"]));
    env.json(&dir, &["drop", &dropped]);

    let statuses = [
        "--status", "idea", "--status", "todo", "--status", "doing", "--status",
        "blocked", "--status", "done", "--status", "dropped",
    ];
    let plain = env
        .cmd(&dir)
        .arg("--pretty")
        .arg("list")
        .args(statuses)
        .output()
        .unwrap();
    let colored = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list"])
        .args(statuses)
        .output()
        .unwrap();
    let plain = String::from_utf8(plain.stdout).unwrap();
    let colored = String::from_utf8(colored.stdout).unwrap();
    assert_eq!(strip_ansi(&colored), plain);
    for code in ["\x1b[34m", "\x1b[33m", "\x1b[31m", "\x1b[2;32m", "\x1b[2;31m"] {
        assert!(colored.contains(code), "missing {code:?}: {colored:?}");
    }
    assert!(colored.contains("\x1b[1mP0\x1b[0m"));
    assert!(colored.contains("\x1b[2m [now]\x1b[0m"));

    let plain_ready = env.cmd(&dir).args(["--pretty", "ready"]).output().unwrap();
    let colored_ready = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "ready"])
        .output()
        .unwrap();
    assert_eq!(
        strip_ansi(&String::from_utf8(colored_ready.stdout).unwrap()),
        String::from_utf8(plain_ready.stdout).unwrap()
    );
}
```

Extend the existing `prime_shows_roadmap_and_closeout` test with a colored invocation and assert that `closeout:`, `roadmap:`, `ready:`, and `doing:` are each wrapped in `\x1b[1m...\x1b[0m`. The existing pretty tree test remains the regression check that indentation precedes the painted row.

- [ ] **Step 3: Run the focused test and verify it fails**

Run:

```bash
cargo test colored_tables_use_semantic_roles_without_changing_layout
```

Expected: FAIL because table rows do not yet contain the status, chrome, or emphasis sequences.

- [ ] **Step 4: Apply styles after padding**

Change `table` to format the width-sensitive fields before painting:

```rust
let id = painter.paint(Style::Chrome, &row.id);
let priority = format!("P{}", row.priority);
let priority = if row.priority <= 1 {
    painter.paint(Style::Emphasis, &priority)
} else {
    priority
};
let status = painter.paint(
    Style::Status(row.status),
    &format!("{:<7}", row.status.as_str()),
);
let tags = if row.tags.is_empty() {
    String::new()
} else {
    painter.paint(Style::Chrome, &format!(" [{}]", row.tags.join(", ")))
};
let owner = row
    .owner
    .as_ref()
    .map(|owner| painter.paint(Style::Chrome, &format!(" @{owner}")))
    .unwrap_or_default();
rendered.push_str(&format!(
    "{id}  {priority} {size:<2} {status} {}{tags}{owner}\n",
    row.title
));
```

Thread `&Painter` through every `table` and `tree_text` call. Paint only the four `prime` section header words with `Style::Emphasis`; keep their surrounding newlines plain. Do not paint `project`, counts, the childless-root summary, task titles, sizes, or indentation.

- [ ] **Step 5: Run focused and full verification**

Run:

```bash
cargo test colored_tables_use_semantic_roles_without_changing_layout
cargo test prime_shows_roadmap_and_closeout
cargo test tree_nests_prunes_and_orders
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo install --path .
tasks check
```

Expected: every command exits 0; stripped colored tables exactly equal uncolored tables.

- [ ] **Step 6: Close and commit Task 2**

Mark this task's completed checkboxes `[x]`, then run:

```bash
tasks done tasks-1bdfed "colored tables, trees, and prime headings without changing layout"
git add src/output.rs tests/cli.rs docs/plans/2026-09-03-color-output.md tasks/tasks-1bdfed.md
git commit -m "feat: color task tables and trees"
```

---

### Task 3: Color show footers and document the public interface

Tracker: `tasks-8eb927`. Depends on Task 2. Spec §3 show rules, §4 typed statuses, §6; original design §5.1.

**Files:**
- Modify: `src/output.rs`, `src/commands/show.rs`, `tests/cli.rs`
- Modify: `README.md`, `docs/specs/2026-08-29-tasks-design.md`, `docs/specs/2026-09-03-color-output-design.md`, `docs/plans/2026-09-03-color-output.md`
- Modify through CLI: `tasks/tasks-8eb927.md`, `tasks/tasks-4737b6.md`

**Interfaces:**
- Changes internally: `DepInfo.status: Option<Status>` and `Related.status: Status`; construction sites use `Some(task.status)` and `task.status`.
- Preserves publicly: serde still emits lowercase JSON strings because `Status` derives `Serialize` with `rename_all = "lowercase"`.
- Consumes: `Painter`, `Style::Status`, `Style::Chrome`, and `Style::Error` from Task 1.

- [ ] **Step 1: Claim the tracker task**

Run:

```bash
tasks start tasks-8eb927
```

Expected: the task becomes `doing`.

- [ ] **Step 2: Write failing show tests while retaining JSON assertions**

Keep the existing assertions that `depends_on[*].status`, `parent.status`, and `children[*].status` are lowercase JSON strings. Extend `show_reports_parent_and_children_and_list_filters_by_parent` to invoke:

```rust
let out = env
    .cmd(&dir)
    .args(["--pretty", "--color", "always", "show", &goal])
    .output()
    .unwrap();
let text = String::from_utf8(out.stdout).unwrap();
assert!(text.contains(&format!("\x1b[2m{one}\x1b[0m [todo] One")), "{text:?}");
assert!(text.contains(&format!("\x1b[2m{two}\x1b[0m [todo] Two")), "{text:?}");
```

Then block `two`, rerun the colored `show`, and assert its footer contains `\x1b[2m{two}\x1b[0m [\x1b[31mblocked\x1b[0m] Two`, proving that footer statuses use the same semantic status role as tables.

Extend `show_resolves_local_and_foreign_dependencies` before deleting the foreign task:

```rust
let out = env
    .cmd(&sci)
    .args(["--pretty", "--color", "always", "show", &bid])
    .output()
    .unwrap();
let text = String::from_utf8(out.stdout).unwrap();
assert!(text.contains(&format!("\x1b[2m{aid}\x1b[0m [todo] Local dep")), "{text:?}");
assert!(text.contains(&format!("\x1b[2m{fid}\x1b[0m [todo] Foreign dep")), "{text:?}");
```

Extend `add_resolves_spec_plan_and_step` after it observes `step_found == false`: invoke `--pretty --color always show <id>` and assert it contains `\x1b[31m# step MISSING\x1b[0m`. Locate `"\n\x1b[31m# step MISSING"` and assert the output slice before that marker contains no `\x1b[`, proving the serialized task text stayed plain.

- [ ] **Step 3: Run the focused tests and verify they fail**

Run:

```bash
cargo test show_reports_parent_and_children_and_list_filters_by_parent
cargo test show_resolves_local_and_foreign_dependencies
cargo test add_resolves_spec_plan_and_step
```

Expected: the pretty-output assertions fail because `show` footers are still plain.

- [ ] **Step 4: Keep statuses typed and paint only footer roles**

In `src/output.rs`, change the internal fields:

```rust
pub struct DepInfo {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<Status>,
    pub resolved: bool,
}

pub struct Related {
    pub id: String,
    pub title: String,
    pub status: Status,
}
```

In `src/commands/show.rs`, replace the two string conversions with the typed values:

```rust
status: Some(task.status),
status: task.status,
```

For each resolved dependency, parent, and child, paint the id with `Style::Chrome` and the lowercase status text with `Style::Status(status)`. Keep unresolved dependency status `?`, titles, and section headers plain. For `step_found == Some(false)`, paint the complete `# step MISSING` line with `Style::Error`; keep `# step found` plain. Continue appending all footers after the untouched `serialize_task` result.

- [ ] **Step 5: Update public documentation and implementation status**

In `README.md`, add one human-facing example and the three-mode contract immediately after the existing `--pretty` example:

```text
tasks --pretty --color auto ready # color when stdout is a terminal
```

State concisely that color is off unless `--color` or `TASKS_COLOR` selects `auto` or `always`, and that non-empty `NO_COLOR` disables ambient color while an explicit flag overrides it.

In `docs/specs/2026-08-29-tasks-design.md` §5 and §5.1, retain JSON as the default and record that the later color design adds explicit, pretty-only styling with no default TTY inference. Link `docs/specs/2026-09-03-color-output-design.md` instead of duplicating its palette.

Change the color design status to:

```markdown
**Status:** implemented (2026-09-03); see docs/plans/2026-09-03-color-output.md.
```

Then search for stale user-facing claims:

```bash
rg -n "TTY detection|--pretty|TASKS_COLOR|--color|NO_COLOR" README.md skills docs/specs
```

Expected: every statement agrees that output format is never inferred, color is off by default, and `auto` probes each output stream only after explicit selection.

- [ ] **Step 6: Run final verification**

Run:

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo install --path .
tasks check
git diff --check
```

Then manually verify both output contracts:

```bash
tasks --color always ready | jq -e '.tasks | type == "array"'
tasks --pretty --color always ready
```

Expected: all gates exit 0; the first command is plain valid JSON and the second visibly colors the pretty table.

- [ ] **Step 7: Close the child and parent tasks, then commit**

Mark this task's completed checkboxes `[x]`, then run:

```bash
tasks done tasks-8eb927 "typed and colored show footers and documented the color interface"
tasks done tasks-4737b6 "implemented opt-in, stream-aware color across pretty output"
tasks check
git add src/output.rs src/commands/show.rs tests/cli.rs README.md docs/specs/2026-08-29-tasks-design.md docs/specs/2026-09-03-color-output-design.md docs/plans/2026-09-03-color-output.md tasks/tasks-8eb927.md tasks/tasks-4737b6.md
git commit -m "feat: finish color output support"
```
