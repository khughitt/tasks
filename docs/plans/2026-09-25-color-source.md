# Configurable Color Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users choose where the pretty scales' truecolors come from — the terminal theme (today's only source) or a built-in palette tasks ships — via `TASKS_THEME=terminal|default`, so a generated theme with an olive slot 5 or a query-averse setup gets a stable, distinct look with no OSC query.

**Architecture:** `src/palette.rs` gains a `ThemeSource` (`terminal` | `default`) parser and a `Palette::BUILTIN` constant with the four colors the specs and tests have always used as the representative palette. `src/main.rs` parses `TASKS_THEME` unconditionally (a bad value exits even when `TASKS_PALETTE` would win) and resolves in the order: `TASKS_PALETTE`, then `default` (no query), then the query. Everything past resolution — recency, priority scale, warnings — is shared and unchanged.

**Tech Stack:** Rust 2024; no new dependencies.

**Spec:** `docs/specs/2026-09-25-color-source-design.md`. Read it before starting; this plan argues from it. It extends `docs/specs/2026-09-25-priority-color-design.md` and `docs/specs/2026-09-25-date-recency-color-design.md`, whose code this plan touches only in `src/main.rs`'s resolution.

## Global Constraints

- `TASKS_THEME` accepts exactly `terminal` and `default`; anything else exits 1 with `TASKS_THEME must be terminal or default, got <value>`, and a non-UTF-8 value exits 1 with `TASKS_THEME must be valid UTF-8, got <value:?>`. Validation happens even when `TASKS_PALETTE` is set and would win.
- Precedence for the palette source: `TASKS_PALETTE` (complete, validated) > `TASKS_THEME=default` (built-in, no query) > terminal query. `TASKS_COLOR` / `NO_COLOR` precedence is untouched: `--color`, then non-empty `NO_COLOR`, then `TASKS_COLOR`.
- Built-in palette: `fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7` — identical to `tests/common/mod.rs`'s `TEST_PALETTE`. Its SGRs (already asserted elsewhere in the suite): P0 `1;38;2;215;95;215`, P1 `38;2;215;95;215`, P2 `38;2;163;88;159`, P3 `38;2;112;78;106`, P4 `38;2;64;64;56`, today's date `38;2;0;215;255`, a 2020-01-01 date `38;2;125;125;115`.
- Under `default` no query is sent and no theme warning is produced; a three-key `TASKS_PALETTE` override still warns `priority colors off: ...` exactly as today.
- Error kind for a bad `TASKS_THEME` value is `Error::Config` (it is environment configuration, like a bad `TASKS_COLOR` value).
- Test hygiene: `tests/common/mod.rs` injects `TASKS_PALETTE=TEST_PALETTE` into every child, and the suite's SGR expectations already match the built-in colors — a built-in-path test that leaves them set proves nothing. Every helper adds `.env_remove("TASKS_THEME")`; the built-in-path tests run through a command with `TASKS_PALETTE` removed.
- The JSON contract is untouched; color only ever affects `--pretty`.
- Commands: `just test-fast [<name>]` while working, `just check` before each commit (the pre-commit hook runs it), `just gate` before finishing. Never run `cargo test` directly.
- Snippets are not guaranteed rustfmt-exact: run `cargo fmt` before `just check`.
- Commits: conventional, no AI attribution. Never edit `tasks/*.md` by hand.

## Review Focus

1. **A bad `TASKS_THEME` with `TASKS_PALETTE` set** — the losing value is still parsed, so this exits 1 rather than silently winning with the palette. Test: Task 2, `an_invalid_theme_fails_even_when_a_palette_would_win`.
2. **Redirected stdout under `default` with color forced on** — paints with the built-ins instead of today's `theme colors off` warning, and no query is attempted (a query against the harness's stdin would hang or eat keys). Test: Task 2, `theme_default_paints_the_builtin_palette_without_a_query`.
3. **`TASKS_PALETTE` overrides `default`** — the palette's magenta, not the built-in's, paints priorities. Test: Task 2, `the_palette_beats_the_builtin_theme`.
4. **The built-in test cannot pass by accident** — helpers inject exactly the proposed colors as `TASKS_PALETTE`; the built-in test removes it inline (the pattern of `redirected_stdout_without_a_palette_skips_the_query_and_warns`, tests/cli.rs:16841) and the helpers clear inherited `TASKS_THEME`. Test: Task 2, `theme_default_paints_the_builtin_palette`.

---

### Task 1: `ThemeSource` and the built-in palette

**Files:**
- Modify: `src/palette.rs` (`ThemeSource`, `Palette::BUILTIN`, unit tests)

**Interfaces:**
- Produces:
  - `pub enum ThemeSource { Terminal, BuiltIn }`
  - `ThemeSource::parse(value: &str) -> crate::error::Result<ThemeSource>` — `terminal` → `Terminal`, `default` → `BuiltIn`, anything else `Error::Config("TASKS_THEME must be terminal or default, got {value:?}")`.
  - `pub const BUILTIN: Palette` — `fg #e5e3d7`, `bg #13140d`, `cyan #00d7ff`, `magenta Some(#d75fd7)`.

- [ ] **Step 1: Write the failing tests**

In `src/palette.rs`'s `tests` module:

```rust
    #[test]
    fn theme_source_parses_terminal_and_default() {
        assert!(matches!(
            ThemeSource::parse("terminal").unwrap(),
            ThemeSource::Terminal
        ));
        assert!(matches!(
            ThemeSource::parse("default").unwrap(),
            ThemeSource::BuiltIn
        ));
    }

    #[test]
    fn theme_source_rejects_anything_else() {
        let error = ThemeSource::parse("chartreuse").unwrap_err();
        assert!(
            error.to_string().contains("TASKS_THEME must be terminal or default"),
            "{error}"
        );
        assert!(ThemeSource::parse("").is_err());
    }

    #[test]
    fn the_builtin_palette_is_the_representative_test_palette() {
        assert_eq!(
            Palette::BUILTIN,
            Palette::parse(TEST_PALETTE_EQUIVALENT).unwrap()
        );
    }
```

with, beside the test module's other helpers:

```rust
    const TEST_PALETTE_EQUIVALENT: &str = "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7";
```

(An equality against a `Palette::parse` call is deliberate: it pins the constant to the same four colors the rest of the suite assumes, without a second hand-written `Rgb` list to drift.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast theme_source`
Expected: FAIL — `ThemeSource` and `Palette::BUILTIN` do not exist.

- [ ] **Step 3: Implement**

In `src/palette.rs`, above `Palette`:

```rust
/// Where the pretty scales' colors come from, chosen by `TASKS_THEME`
/// (docs/specs/2026-09-25-color-source-design.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    /// The terminal theme: `TASKS_PALETTE` if set, else the OSC query.
    Terminal,
    /// The palette tasks ships; no query, and `TASKS_PALETTE` still wins over it.
    BuiltIn,
}

impl ThemeSource {
    pub fn parse(value: &str) -> crate::error::Result<ThemeSource> {
        match value {
            "terminal" => Ok(ThemeSource::Terminal),
            "default" => Ok(ThemeSource::BuiltIn),
            other => Err(Error::Config(format!(
                "TASKS_THEME must be terminal or default, got {other:?}"
            ))),
        }
    }
}
```

and, directly under the `Palette` struct's definition:

```rust
impl Palette {
    /// The palette tasks ships for `TASKS_THEME=default`: the representative palette of
    /// the color designs and tests. Fixed for dark backgrounds — its mixed endpoints
    /// (the old-date end `#7d7d73`, the priority end `#404038`) recede on a dark
    /// terminal and lose contrast on a light one.
    pub const BUILTIN: Palette = Palette {
        fg: Rgb {
            r: 0xe5,
            g: 0xe3,
            b: 0xd7,
        },
        bg: Rgb {
            r: 0x13,
            g: 0x14,
            b: 0x0d,
        },
        cyan: Rgb {
            r: 0x00,
            g: 0xd7,
            b: 0xff,
        },
        magenta: Some(Rgb {
            r: 0xd7,
            g: 0x5f,
            b: 0xd7,
        }),
    };
```

(merged into the existing `impl Palette` block, not a second one.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast theme_source && just test-fast the_builtin_palette`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/palette.rs
git commit -m "feat(color): add TASKS_THEME source parser and built-in palette"
```

---

### Task 2: Wire `TASKS_THEME` into `main`, test helpers, CLI tests, README

**Files:**
- Modify: `src/main.rs` (env read beside `TASKS_COLOR`/`TASKS_PALETTE`, resolution block)
- Modify: `tests/common/mod.rs` (helpers clear inherited `TASKS_THEME`)- Modify: `tests/cli.rs` (three new tests beside the existing color tests)
- Modify: `README.md` (color section)

**Interfaces:**
- Consumes: `ThemeSource`, `Palette::BUILTIN` from Task 1.
- Produces: nothing exported; the resolution lives in `main`.

- [ ] **Step 1: Write the failing CLI tests**

In `tests/common/mod.rs`, add `.env_remove("TASKS_THEME")` beside each `.env_remove("TASKS_COLOR")` in `cmd`, `raw` and the third helper (around lines 41, 71 and 341).

In `tests/cli.rs`, beside the existing color tests (near `redirected_stdout_without_a_palette_skips_the_query_and_warns`, tests/cli.rs:16834), three new tests using that test's and `colored_list_paints_dates_by_recency`'s (tests/cli.rs:16694) exact shapes:

```rust
#[test]
fn theme_default_paints_the_builtin_palette() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let fresh = id_of(env.json(&dir, &["add", "Fresh", "-p", "1"]));
    let today = env.json(&dir, &["show", &fresh])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    write_doc(
        &dir,
        "tasks/sci-a00001.md",
        "---\nid: sci-a00001\ntitle: Ancient\nstatus: todo\npriority: 2\ncreated: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\ndepends: []\ntags: []\n---\n",
    );
    // The helpers set `TASKS_PALETTE` to the built-in's exact colors, so it comes off
    // here: the SGRs below prove the built-in path, not the injected palette.
    let out = env
        .cmd(&dir)
        .env_remove("TASKS_PALETTE")
        .env("TASKS_THEME", "default")
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "today is full cyan: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;125;125;115m2020-01-01\x1b[0m"),
        "past the horizon is the old end: {text:?}"
    );
    assert!(
        text.contains("\x1b[38;2;215;95;215mP1\x1b[0m"),
        "P1 is the built-in magenta: {text:?}"
    );
    assert!(
        String::from_utf8(out.stderr).unwrap().is_empty(),
        "the built-in palette needs no warning"
    );
}

#[test]
fn the_palette_beats_the_builtin_theme() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "One", "-p", "1"]);
    let out = env
        .cmd(&dir)
        .env("TASKS_THEME", "default")
        .env(
            "TASKS_PALETTE",
            "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#ff0000",
        )
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\x1b[38;2;255;0;0mP1\x1b[0m"), "{text:?}");
    assert!(
        !text.contains("215;95;215"),
        "the built-in magenta is not used: {text:?}"
    );
}

#[test]
fn an_invalid_theme_fails_even_when_a_palette_would_win() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    // `cmd` already sets the winning `TASKS_PALETTE`; the bad value must still exit 1.
    let out = env
        .cmd(&dir)
        .env("TASKS_THEME", "chartreuse")
        .args(["list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("TASKS_THEME must be terminal or default"),
        "{error}"
    );
}
```

(The invalid-value shape mirrors `tasks_color_is_always_validated_and_warnings_use_the_stderr_painter`, tests/cli.rs:5857.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast theme_default_paints_the_builtin_palette`, then `just test-fast the_palette_beats_the_builtin_theme`, then `just test-fast an_invalid_theme_fails_even_when_a_palette_would_win`
Expected: FAIL — `default` currently falls through to the query/`NotAsked` path, so the first test's stdout is unpainted and its stderr warning assertion trips; the precedence test trips because `TASKS_THEME` is ignored.

- [ ] **Step 3: Implement in `src/main.rs`**

Read the variable beside the others (after the `TASKS_PALETTE` block, same shape):

```rust
    let theme = match std::env::var("TASKS_THEME") {
        Ok(value) => match palette::ThemeSource::parse(&value) {
            Ok(source) => Some(source),
            Err(error) => {
                to_stderr(&format!("{}\n", output::render_error(&error)));
                std::process::exit(1);
            }
        },
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(value)) => {
            to_stderr(&format!(
                "{}\n",
                output::render_error(&error::Error::Config(format!(
                    "TASKS_THEME must be valid UTF-8, got {value:?}"
                )))
            ));
            std::process::exit(1);
        }
    };
```

and replace the resolution inside the `needs_theme` branch:

```rust
                let resolved = match palette {
                    Some(palette) => Ok(palette),
                    None => match theme {
                        Some(palette::ThemeSource::BuiltIn) => Ok(palette::Palette::BUILTIN),
                        _ if !std::io::stdout().is_terminal() => {
                            Err(palette::QueryError::NotAsked)
                        }
                        _ => palette::Palette::query(palette::QUERY_TIMEOUT),
                    },
                };
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast theme_default_paints_the_builtin_palette`, then `just test-fast the_palette_beats_the_builtin_theme`, then `just test-fast an_invalid_theme_fails_even_when_a_palette_would_win`
Expected: PASS.

- [ ] **Step 5: Document in `README.md`**

In the color paragraph (around lines 362-376), after the `TASKS_PALETTE` sentence, add:

```markdown
By default the colors come from the terminal itself. `TASKS_THEME=default` instead uses
the palette tasks ships (`fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7`), which
sends no query and works on dark backgrounds; a set `TASKS_PALETTE` still wins over it.
`TASKS_THEME=terminal` is the default behavior, spelled out.
```

- [ ] **Step 6: Run the full fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src/main.rs tests/common/mod.rs tests/cli.rs README.md
git commit -m "feat(color): honor TASKS_THEME=default as the color source"
```

---

### Task 3: Final verification

**Files:** none modified.

- [ ] **Step 1:** `cargo install --path .` — the tracker used in sessions must be the code under test (repo rule).
- [ ] **Step 2:** `just gate` — fmt, clippy, `tasks check`, the whole suite including the ignored exhaustive enumeration.
- [ ] **Step 3:** Manual smoke in a real terminal with `TASKS_THEME=default tasks --color always list` and `TASKS_THEME=chartreuse tasks list`: painted output, no theme warning; the bad value exits 1. The spec leaves the built-in look to a visual check — confirm the columns read as distinct in your own terminal.
- [ ] **Step 4:** `tasks done tasks-ee6ca2 "<what landed>"` in the same commit as the final code, after the spec's Status line is updated from draft to implemented with the plan reference.
