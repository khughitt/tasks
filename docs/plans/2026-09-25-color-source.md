# Configurable Color Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users choose where the pretty scales' truecolors come from — the terminal theme (today's only source) or a built-in palette tasks ships — via `TASKS_THEME=terminal|default`, so a generated theme with an olive slot 5 or a query-averse setup gets a stable, distinct look with no OSC query.

**Architecture:** `src/palette.rs` gains a `ThemeSource` (`terminal` | `default`) parser and a `Palette::BUILTIN` constant with the four colors the specs and tests have always used as the representative palette. `src/main.rs` parses `TASKS_THEME` unconditionally (a bad value exits even when `TASKS_PALETTE` would win) and resolves in the order: `TASKS_PALETTE`, then `default` (no query), then the query. Everything past resolution — recency, priority scale, warnings — is shared and unchanged. One implementation commit: a split would leave the parser unused between commits, and the pre-commit hook runs clippy with `-D warnings`, which fails on dead code.

**Tech Stack:** Rust 2024; no new dependencies — the PTY test uses `libc::openpty`, the pattern already in `src/palette.rs`'s tests.

**Spec:** `docs/specs/2026-09-25-color-source-design.md`. Read it before starting; this plan argues from it. It extends `docs/specs/2026-09-25-priority-color-design.md` and `docs/specs/2026-09-25-date-recency-color-design.md`, whose code this plan touches only in `src/main.rs`'s resolution.

## Global Constraints

- `TASKS_THEME` accepts exactly `terminal` and `default`; anything else exits 1 with `TASKS_THEME must be terminal or default, got <value>`, and a non-UTF-8 value exits 1 with `TASKS_THEME must be valid UTF-8, got <value:?>`. Validation happens even when `TASKS_PALETTE` is set and would win.
- Precedence for the palette source: `TASKS_PALETTE` (complete, validated) > `TASKS_THEME=default` (built-in, no query) > terminal query. `TASKS_COLOR` / `NO_COLOR` precedence is untouched: `--color`, then non-empty `NO_COLOR`, then `TASKS_COLOR`.
- Built-in palette: `fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7` — identical to `tests/common/mod.rs`'s `TEST_PALETTE`. Its SGRs (already asserted elsewhere in the suite): P0 `1;38;2;215;95;215`, P1 `38;2;215;95;215`, P2 `38;2;163;88;159`, P3 `38;2;112;78;106`, P4 `38;2;64;64;56`, today's date `38;2;0;215;255`, a 2020-01-01 date `38;2;125;125;115`.
- Under `default` no query is sent and no theme warning is produced; a three-key `TASKS_PALETTE` override still warns `priority colors off: ...` exactly as today.
- Error kind for a bad `TASKS_THEME` value is `Error::Config` (it is environment configuration, like a bad `TASKS_COLOR` value).
- Test hygiene: `tests/common/mod.rs` injects `TASKS_PALETTE=TEST_PALETTE` into every child, and the suite's SGR expectations already match the built-in colors — a built-in-path test that leaves them set proves nothing. Every helper adds `.env_remove("TASKS_THEME")`; built-in-path tests run with `TASKS_PALETTE` removed (`env_remove` inline, as `redirected_stdout_without_a_palette_skips_the_query_and_warns` does; the PTY helper removes it itself).
- The JSON contract is untouched; color only ever affects `--pretty`.
- Commands: `just test-fast [<name>]` while working, `just check` before each commit (the pre-commit hook runs it), `just gate` before finishing. Never run `cargo test` directly.
- Snippets are not guaranteed rustfmt-exact: run `cargo fmt` before `just check`.
- Commits: conventional, no AI attribution. Never edit `tasks/*.md` by hand.

## Review Focus

1. **A bad `TASKS_THEME` with `TASKS_PALETTE` set** — the losing value is still parsed, so this exits 1 rather than silently winning with the palette. Test: Task 1, `an_invalid_theme_fails_even_when_a_palette_would_win`.
2. **No query on a real terminal under `default`** — piped-stdout tests cannot see the query; the PTY test observes the pty stream itself and proves both directions: no OSC bytes and no warning under `default`, and — as a control that the observation works — query bytes and the timeout warning without `TASKS_THEME`. Test: Task 1, `theme_default_sends_no_osc_query_on_a_terminal`.
3. **Redirected stdout under `default` with color forced on** — paints with the built-ins instead of today's `theme colors off` warning. Test: Task 1, `theme_default_paints_the_builtin_palette`.
4. **`TASKS_PALETTE` overrides `default`** — the palette's magenta, not the built-in's, paints priorities. This test passes before the change (ignoring `TASKS_THEME` still resolves the explicit palette); it stays as a regression test. Test: Task 1, `the_palette_beats_the_builtin_theme`.
5. **The built-in test cannot pass by accident** — helpers inject exactly the proposed colors as `TASKS_PALETTE`; the built-in tests remove it inline and the helpers clear inherited `TASKS_THEME`. Test: Task 1, `theme_default_paints_the_builtin_palette`.

---

### Task 1: Implement `TASKS_THEME` end to end

**Files:**
- Modify: `src/palette.rs` (`ThemeSource`, `Palette::BUILTIN`, unit tests)
- Modify: `src/main.rs` (env read beside `TASKS_COLOR`/`TASKS_PALETTE`, resolution block)
- Modify: `tests/common/mod.rs` (helpers clear inherited `TASKS_THEME`)
- Modify: `tests/cli.rs` (four new tests beside the existing color tests)
- Modify: `README.md` (color section)

**Interfaces:**
- Produces: `pub enum ThemeSource { Terminal, BuiltIn }`; `ThemeSource::parse(value: &str) -> crate::error::Result<ThemeSource>` (`terminal` → `Terminal`, `default` → `BuiltIn`, anything else `Error::Config("TASKS_THEME must be terminal or default, got {value:?}")`); `pub const BUILTIN: Palette` (`fg #e5e3d7`, `bg #13140d`, `cyan #00d7ff`, `magenta Some(#d75fd7)`). The resolution lives in `main`; nothing else is exported.

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

In `tests/common/mod.rs`, add `.env_remove("TASKS_THEME")` beside each `.env_remove("TASKS_COLOR")` in `cmd`, `raw` and the third helper (around lines 41, 71 and 341).

In `tests/cli.rs`, beside the existing color tests (near `redirected_stdout_without_a_palette_skips_the_query_and_warns`, tests/cli.rs:16834), four new tests plus one helper. The three piped tests use that test's and `colored_list_paints_dates_by_recency`'s (tests/cli.rs:16694) exact shapes:

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

The PTY test observes the query on the child's own terminal, which piped stdout cannot see. One helper, two assertions per side:

```rust
/// Runs `--pretty --color always list` with stdout and stderr on a fresh pty that is
/// also the child's controlling terminal, and returns everything the child wrote,
/// bounded by a ten-second deadline. Built from `env.raw` so the child keeps the
/// helpers' environment isolation; `TASKS_THEME` is removed first and set only when
/// asked, so a `TASKS_THEME` in the suite's own environment cannot defeat the control.
/// `TASKS_PALETTE` is always removed. Nothing answers the query from the master side,
/// so the control case exercises the query's timeout path.
fn list_on_pty(env: &TestEnv, dir: &std::path::Path, theme: Option<&str>) -> String {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let mut master_fd: libc::c_int = 0;
    let mut slave_fd: libc::c_int = 0;
    // SAFETY: two valid out-pointers; default attributes.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // Both descriptors are owned from here on, so every path below — panics
    // included — closes them.
    let slave = unsafe { OwnedFd::from_raw_fd(slave_fd) };
    // SAFETY: `slave` is ours; CLOEXEC affects only the child's post-exec image,
    // and the pre-exec closure below runs before it.
    unsafe {
        libc::fcntl(
            slave.as_raw_fd(),
            libc::F_SETFD,
            libc::fcntl(slave.as_raw_fd(), libc::F_GETFD) | libc::FD_CLOEXEC,
        )
    };
    let master = unsafe { OwnedFd::from_raw_fd(master_fd) };
    // Non-blocking before spawn: a failure here involves no child to clean up, and
    // an undetected failure would leave the first read blocking past the deadline.
    // SAFETY: `master` is ours.
    assert_ne!(
        unsafe {
            libc::fcntl(
                master.as_raw_fd(),
                libc::F_SETFL,
                libc::fcntl(master.as_raw_fd(), libc::F_GETFL) | libc::O_NONBLOCK,
            )
        },
        -1,
        "making the master non-blocking failed"
    );
    let mut command = env.raw(dir);
    // `try_clone` checks the duplication and owns each descriptor immediately; no
    // unchecked raw duplicate can slip a `-1` into `from_raw_fd`.
    command
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave.try_clone().unwrap()))
        .env_remove("TASKS_PALETTE")
        .env_remove("TASKS_THEME");
    if let Some(theme) = theme {
        command.env("TASKS_THEME", theme);
    }
    // The closure takes the fd number, not the `OwnedFd`: the descriptor must stay
    // open in the parent until after `spawn`, and the child needs it during
    // pre-exec, before CLOEXEC takes effect.
    let child_slave = slave.as_raw_fd();
    // SAFETY: runs once in the forked child before exec.
    unsafe {
        command.pre_exec(move || {
            // SAFETY: libc calls in the forked child, before exec.
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: `child_slave` is a valid descriptor in the child.
            if libc::ioctl(child_slave, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.args(["--pretty", "--color", "always", "list"]);
    let mut child = command.spawn().unwrap();
    // The child has inherited what it needs; the parent's copy of `slave` must go,
    // and `command` still owns the cloned slave descriptors, so it goes too —
    // otherwise the master never sees EOF and the loop only ends at the deadline.
    drop(slave);
    drop(command);

    let deadline = Instant::now() + Duration::from_secs(10);
    // The deadline is checked every iteration, and the loop ends only when both the
    // output is done (EOF or EIO on the master) and the child has been seen to
    // exit, because either alone can lie: a child can close its stdio and live on,
    // and Linux reads EIO on a master whose slave side is gone.
    type Outcome = Result<(Vec<u8>, Option<std::process::ExitStatus>), String>;
    let outcome =
        (|child: &mut std::process::Child, master: &OwnedFd| -> Outcome {
            let mut bytes = Vec::new();
            let mut output_done = false;
            let mut status = None;
            loop {
                if Instant::now() >= deadline {
                    return Err(format!("the pty child did not finish; wrote {bytes:?}"));
                }
                if !output_done {
                    let mut buf = [0u8; 4096];
                    // SAFETY: `master` is ours and `buf` outlives the call.
                    let n = unsafe {
                        libc::read(master.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len())
                    };
                    if n > 0 {
                        bytes.extend_from_slice(&buf[..usize::try_from(n).unwrap()]);
                        continue;
                    } else if n == 0 {
                        output_done = true;
                    } else {
                        let error = std::io::Error::last_os_error();
                        match error.raw_os_error() {
                            Some(libc::EIO) => output_done = true,
                            Some(libc::EAGAIN) => {
                                std::thread::sleep(Duration::from_millis(20))
                            }
                            _ => return Err(format!("reading the master failed: {error}")),
                        }
                    }
                }
                if status.is_none() {
                    status = child.try_wait().map_err(|error| error.to_string())?;
                }
                if output_done && status.is_some() {
                    return Ok((bytes, status));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })(&mut child, &master);
    let (bytes, status) = match outcome {
        Ok(pair) => pair,
        Err(message) => {
            // Every post-spawn failure terminates and reaps the child before
            // reporting; `master` and `slave` close through their own drops.
            let _ = child.kill();
            let _ = child.wait();
            panic!("{message}");
        }
    };
    let status = status.unwrap();
    assert!(status.success(), "the pty child failed");
    String::from_utf8(bytes).unwrap()
}

#[test]
fn theme_default_sends_no_osc_query_on_a_terminal() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "One", "-p", "1"]);

    let theme = list_on_pty(&env, &dir, Some("default"));
    assert!(
        theme.contains("\x1b[38;2;215;95;215mP1\x1b[0m"),
        "painted from the built-ins: {theme:?}"
    );
    assert!(!theme.contains("theme colors off"), "{theme:?}");
    for query in ["\x1b]10;?", "\x1b]11;?", "\x1b]4;5;?", "\x1b]4;6;?", "\x1b[c"] {
        assert!(!theme.contains(query), "query sent: {query:?}");
    }

    // The control proves the observation can see a query: without `TASKS_THEME` the
    // child asks the terminal (nothing answers) and warns after the timeout.
    let terminal = list_on_pty(&env, &dir, None);
    assert!(
        terminal.contains("\x1b]10;?"),
        "the control queried: {terminal:?}"
    );
    assert!(terminal.contains("theme colors off"), "{terminal:?}");
}
```

(The helper was compiled and run as a standalone probe before this plan was finalized,
and its failure paths were probe-tested: a child writing continuously and a child that
closes its stdio and lives on both end at the deadline with the child killed and reaped,
while a normal child returns promptly with its output.)

- [ ] **Step 2: Run the tests to verify the state of the world**

Run: `just test-fast theme_source`, then `just test-fast theme_default_paints_the_builtin_palette`, then `just test-fast the_palette_beats_the_builtin_theme`, then `just test-fast an_invalid_theme_fails_even_when_a_palette_would_win`, then `just test-fast theme_default_sends_no_osc_query_on_a_terminal`.

Expected: the unit tests and `an_invalid_theme...` FAIL (nothing exists yet); `theme_default_paints_the_builtin_palette` FAIL (stdout is piped, so today's `NotAsked` path leaves it unpainted and warns); `theme_default_sends_no_osc_query_on_a_terminal` FAIL (its `default` half warns instead of painting). `the_palette_beats_the_builtin_theme` PASSES today — ignoring `TASKS_THEME` still resolves the explicit palette — and stays as the regression test.

- [ ] **Step 3: Implement in `src/palette.rs` and `src/main.rs`**

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

and, merged into the existing `impl Palette` block (not a second one):

```rust
    /// The palette tasks ships for `TASKS_THEME=default`: the representative palette of
    /// the color designs and tests. Fixed colors intended for dark backgrounds; a fixed
    /// palette cannot guarantee contrast against an arbitrary background (spec §2.1),
    /// so the look is confirmed visually, not by the suite.
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

In `src/main.rs`, read the variable beside the others (after the `TASKS_PALETTE` block, same shape):

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

Run: `just test-fast theme_source`, then `just test-fast the_builtin_palette`, then `just test-fast theme_default_paints_the_builtin_palette`, then `just test-fast the_palette_beats_the_builtin_theme`, then `just test-fast an_invalid_theme_fails_even_when_a_palette_would_win`, then `just test-fast theme_default_sends_no_osc_query_on_a_terminal`.
Expected: PASS.

- [ ] **Step 5: Document in `README.md`**

In the color paragraph (around lines 362-376), after the `TASKS_PALETTE` sentence, add:

```markdown
By default the colors come from the terminal itself. `TASKS_THEME=default` instead uses
the palette tasks ships (`fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7`), which
sends no query and is meant for dark backgrounds; a set `TASKS_PALETTE` still wins over
it. `TASKS_THEME=terminal` is the default behavior, spelled out.
```

- [ ] **Step 6: Run the full fast suite and commit**

Run: `just test-fast`
Expected: PASS.

```bash
git add src/palette.rs src/main.rs tests/common/mod.rs tests/cli.rs README.md
git commit -m "feat(color): add TASKS_THEME for a built-in color source"
```

---

### Task 2: Final verification

**Files:** none modified.

- [ ] **Step 1:** ~~`cargo install --path .`~~ refused here: the harness that ran this
  session does not permit `cargo install` (it writes outside the repository), so the
  host's `~/.cargo/bin/tasks` was left untouched and the smoke below ran the worktree
  binary by explicit path. `cargo build --release` supplied it. Reinstall on the host
  when the branch merges.
- [ ] **Step 2:** `just gate` — fmt, clippy, `tasks check`, the whole suite including the ignored exhaustive enumeration.
- [ ] **Step 3:** Manual smoke from the main checkout, with `TASKS_PALETTE` explicitly unset (an inherited palette would win over the built-ins) and `--pretty` forcing the colored path (`--color always` alone leaves JSON unpainted):

        env -u TASKS_PALETTE TASKS_THEME=default \
          .worktrees/color-source/target/release/tasks --pretty --color always list
        env -u TASKS_PALETTE TASKS_THEME=chartreuse \
          .worktrees/color-source/target/release/tasks list

    Expected: painted output, no theme warning; the bad value exits 1 naming `TASKS_THEME`. The spec leaves the built-in look to a visual check — confirm the columns read as distinct in your own terminal.
- [ ] **Step 4:** `tasks done tasks-ee6ca2 "<what landed>"` in the same commit as the final code, after the spec's Status line is updated from draft to implemented with the plan reference.
