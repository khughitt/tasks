# Priority Magenta Scale Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Amended after the kitty check (2026-09-25):** the steps and end below were replaced by
> spec §2 / §2.4: P2 1/3, P3 2/3, P4 1, toward the foreground mixed 0.75 toward the
> background (not `Palette::old`). Test palette SGRs are now P2 `38;2;163;88;159`,
> P3 `38;2;112;78;106`, P4 `38;2;64;64;56`. The rest of the plan stands as executed.

**Goal:** In colored `--pretty` output, paint each priority on a scale from the terminal theme's magenta (P0 bold, P1) to the date scale's dimmed foreground (P4), keeping today's bold P0/P1 wherever the theme's colors are unknown.

**Architecture:** `src/palette.rs` gains an optional `magenta` (from `TASKS_PALETTE`, or always from the terminal query, which now asks for slot 5) and owns the shared old end as `Palette::old`. `src/style.rs` gains a `Style::Priority(u8)` role and a `PriorityScale` the painter may carry; without one the role renders today's bold P0/P1. `src/output.rs` paints every priority through the role, and `src/main.rs` attaches the scale when the palette has a magenta, warning when a view that shows priorities cannot have it.

**Tech Stack:** Rust 2024, the existing `terminal-trx` / `xterm-color` / `libc` query stack; no new dependencies.

**Spec:** `docs/specs/2026-09-25-priority-color-design.md`. Read it before starting; this plan argues from it. It amends `docs/specs/2026-09-25-date-recency-color-design.md`, whose code this plan extends.

## Global Constraints

- Steps: P0 `t = 0.0` plus bold, P1 `0.0`, P2 `0.5`, P3 `0.8`, P4 `1.0`; color is `magenta.mix(old, t)`.
- Old end: foreground mixed `0.45` toward background, in OKLab (`Palette::old`), shared with the date scale.
- Priority SGR with a scale: `38;2;r;g;b`, prefixed `1;` at P0. Without a scale: `1` at P0 and P1, nothing at P2–P4.
- Query: `OSC 10;?`, `OSC 11;?`, `OSC 4;5;?`, `OSC 4;6;?`, each ST-terminated, then DA1 `CSI c`; four replies required, fewer is `Unsupported`.
- `TASKS_PALETTE`: `fg`, `bg`, `cyan` required; `magenta` optional; same `#rrggbb`, once-only, any-order rules; error prefix `TASKS_PALETTE must be "fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]": `.
- Query failure warning: `theme colors off: the terminal did not report its colors (<reason>); set TASKS_PALETTE to supply them`.
- Missing magenta warning: `priority colors off: TASKS_PALETTE has no magenta; add magenta=#rrggbb`, only for outputs that show priorities (`list`, `ready`, `tree`, `prime`, `quiet`), never with the query failure warning.
- Test palette: `fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7`. Its SGRs: P0 `1;38;2;215;95;215`, P1 `38;2;215;95;215`, P2 `38;2;171;115;165`, P3 `38;2;144;122;135`, P4 `38;2;125;125;115`. (Computed with the reference mix; if Task 2's unit test shows a channel off by one, the Rust `Rgb::mix` is authoritative: use its value everywhere and say so in the commit.)
- The JSON contract is untouched.
- Commands: `just test-fast [<name>]` while working, `just check` before each commit (the pre-commit hook runs it), `just gate` before finishing. Never run `cargo test` directly.
- Snippets are not guaranteed rustfmt-exact: run `cargo fmt` before `just check`.
- Commits: conventional, no AI attribution. Never edit `tasks/*.md` by hand.

## Review Focus

1. **A three-key `TASKS_PALETTE` on `projects` or `parked`** — every color those views display works, so no warning may appear. Test: Task 2, `a_palette_without_magenta_warns_only_where_priorities_show`.
2. **A terminal that answers fg, bg and cyan but not slot 5** — the whole theme is off (`Unsupported`), not a half-painted table. Test: Task 1, `three_replies_are_no_longer_enough`.
3. **Redirected output with no `TASKS_PALETTE`** — P0/P1 stay bold, dates plain, one reworded warning, and no magenta warning beside it. Test: Task 2, the extended `redirected_stdout_without_a_palette_skips_the_query_and_warns`.
4. **Column alignment with truecolor on P0** — `1;38;2;…` must not change visible widths. Test: Task 2, the existing layout check in the colored `list` test, now asserting the new P0 code.
5. **`show` with color on** — `priority:` keeps bold at P0/P1 and prints no warning, because `show` never queries. Test: Task 2, `colored_show_keeps_bold_priority_without_a_warning`.

---

### Task 1: The palette reads magenta

**Files:**
- Modify: `src/palette.rs` (`Palette`, `Palette::parse`, `Palette::old`, `QUERY`, `parse_replies`, unit tests)
- Modify: `src/style.rs` (`Recency::new` uses `Palette::old`; drop `OLD_TOWARD_BACKGROUND`)

**Interfaces:**
- Produces:
  - `pub struct Palette { pub fg: Rgb, pub bg: Rgb, pub cyan: Rgb, pub magenta: Option<Rgb> }` (derives unchanged)
  - `Palette::old(&self) -> Rgb` — `self.fg.mix(self.bg, 0.45)`
  - `Palette::parse` accepts an optional `magenta` key; `Palette::query` / `exchange` always return `magenta: Some(_)`.

- [ ] **Step 1: Write the failing tests**

In `src/palette.rs`'s `tests` module, replace `palette_parses_three_keys_in_any_order` and extend the rejection table:

```rust
    #[test]
    fn palette_parses_three_or_four_keys_in_any_order() {
        let palette = Palette::parse("cyan=#00d7ff fg=#e5e3d7  bg=#13140d").unwrap();
        assert_eq!(
            palette,
            Palette {
                fg: hex("#e5e3d7"),
                bg: hex("#13140d"),
                cyan: hex("#00d7ff"),
                magenta: None,
            }
        );
        let palette =
            Palette::parse("magenta=#d75fd7 cyan=#00d7ff fg=#e5e3d7 bg=#13140d").unwrap();
        assert_eq!(palette.magenta, Some(hex("#d75fd7")));
    }

    #[test]
    fn the_old_end_is_the_foreground_dimmed_toward_the_background() {
        let palette = Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        assert_eq!(
            palette.old(),
            Rgb {
                r: 125,
                g: 125,
                b: 115
            }
        );
    }
```

Add these rows to the `for (value, needle) in [...]` table of `palette_rejects_missing_unknown_duplicate_and_malformed_entries`:

```rust
            ("fg=#e5e3d7 bg=#13140d magenta=#d75fd7", "missing cyan"),
            (
                "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7 magenta=#d75fd7",
                "magenta is given twice",
            ),
            (
                "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=d75fd7",
                "magenta: \"d75fd7\" is not #rrggbb",
            ),
```

and after the loop:

```rust
        let text = Palette::parse("").unwrap_err().to_string();
        assert!(
            text.contains("\"fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]\""),
            "{text}"
        );
```

Make the exchange helpers answer slot 5. Replace `replies` and `expected`:

```rust
    /// Replies in query order, with slot 5 fixed at `#d75fd7`.
    fn replies(fg: &str, bg: &str, cyan: &str, end: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (prefix, color) in [
            ("10;", fg),
            ("11;", bg),
            ("4;5;", "rgb:d7d7/5f5f/d7d7"),
            ("4;6;", cyan),
        ] {
            bytes.extend_from_slice(format!("\x1b]{prefix}{color}{end}").as_bytes());
        }
        bytes.extend_from_slice(DA1);
        bytes
    }

    fn expected() -> Palette {
        Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7").unwrap()
    }
```

In `exchange_sends_the_three_queries_and_the_fence`, rename it `exchange_sends_the_four_queries_and_the_fence` and change the expected bytes:

```rust
        assert_eq!(
            tty.sent,
            b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;5;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c".to_vec()
        );
```

Add:

```rust
    #[test]
    fn three_replies_are_no_longer_enough() {
        // A terminal that answers fg, bg and slot 6 but skips slot 5.
        let mut bytes = Vec::new();
        for (prefix, color) in [("10;", "rgb:e5/e3/d7"), ("11;", "rgb:13/14/0d"), ("4;6;", "rgb:00/d7/ff")] {
            bytes.extend_from_slice(format!("\x1b]{prefix}{color}\x07").as_bytes());
        }
        bytes.extend_from_slice(DA1);
        let err = exchange(&mut Fake::new(&bytes, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
    }
```

`typed_keys_neither_end_the_read_nor_outlast_the_fence` and the malformed-reply test keep working unchanged through the new `replies`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast palette`
Expected: compile errors (`Palette` has no field `magenta`, no method `old`).

- [ ] **Step 3: Implement**

In `src/palette.rs`:

```rust
/// How far the old end of both scales sits from the foreground toward the background.
const OLD_TOWARD_BACKGROUND: f64 = 0.45;

/// The theme colors the date and priority scales need. `magenta` is `None` only from a
/// `TASKS_PALETTE` that leaves it out; the terminal query always supplies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cyan: Rgb,
    pub magenta: Option<Rgb>,
}

impl Palette {
    /// Parses `TASKS_PALETTE`: `fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]`,
    /// any order.
    pub fn parse(value: &str) -> crate::error::Result<Palette> {
        let bad = |detail: String| {
            Error::Config(format!(
                "TASKS_PALETTE must be \"fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]\": {detail}"
            ))
        };
        let (mut fg, mut bg, mut cyan, mut magenta) = (None, None, None, None);
        for entry in value.split_whitespace() {
            let (key, hex) = entry
                .split_once('=')
                .ok_or_else(|| bad(format!("{entry:?} is not key=#rrggbb")))?;
            let slot = match key {
                "fg" => &mut fg,
                "bg" => &mut bg,
                "cyan" => &mut cyan,
                "magenta" => &mut magenta,
                other => return Err(bad(format!("unknown key {other:?}"))),
            };
            if slot.is_some() {
                return Err(bad(format!("{key} is given twice")));
            }
            *slot = Some(
                Rgb::parse_hex(hex).ok_or_else(|| bad(format!("{key}: {hex:?} is not #rrggbb")))?,
            );
        }
        match (fg, bg, cyan) {
            (Some(fg), Some(bg), Some(cyan)) => Ok(Palette {
                fg,
                bg,
                cyan,
                magenta,
            }),
            _ => {
                // Only the required keys can be missing.
                let missing: Vec<&str> = [("fg", fg), ("bg", bg), ("cyan", cyan)]
                    .into_iter()
                    .filter(|(_, color)| color.is_none())
                    .map(|(key, _)| key)
                    .collect();
                Err(bad(format!("missing {}", missing.join(", "))))
            }
        }
    }

    /// The faded end both scales run toward: the foreground dimmed toward the background.
    pub fn old(&self) -> Rgb {
        self.fg.mix(self.bg, OLD_TOWARD_BACKGROUND)
    }
}
```

Update the query constant and its comment:

```rust
/// Foreground, background and palette slots 5 and 6, each ST-terminated, then primary
/// device attributes (DA1) as a fence: terminals answer in order, so the fence arriving
/// before a color reply means the terminal does not answer color queries.
const QUERY: &[u8] = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;5;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c";
```

Replace `parse_replies`:

```rust
/// The four color replies among the bytes before the fence. Fewer than four means
/// the terminal skipped a query it does not support.
fn parse_replies(bytes: &[u8]) -> Result<Palette, QueryError> {
    let bodies = osc_bodies(bytes)?;
    match bodies.as_slice() {
        [fg, bg, magenta, cyan] => Ok(Palette {
            fg: reply_color(fg, b"10;")?,
            bg: reply_color(bg, b"11;")?,
            cyan: reply_color(cyan, b"4;6;")?,
            magenta: Some(reply_color(magenta, b"4;5;")?),
        }),
        fewer if fewer.len() < 4 => Err(QueryError::Unsupported),
        _ => Err(QueryError::Unparsable(format!(
            "{} replies to four queries",
            bodies.len()
        ))),
    }
}
```

Replace the module doc comment:

```rust
//! The terminal theme's colors: RGB values, OKLab mixing, and where they come from
//! (`TASKS_PALETTE`, or a query to the terminal). Design:
//! docs/specs/2026-09-25-date-recency-color-design.md and
//! docs/specs/2026-09-25-priority-color-design.md.
```

In `src/style.rs`, delete `OLD_TOWARD_BACKGROUND` and change `Recency::new`:

```rust
    pub fn new(today: time::Date, palette: &Palette) -> Recency {
        Recency {
            today,
            recent: palette.cyan,
            old: palette.old(),
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-fast palette` then `just test-fast recency`
Expected: PASS. The existing `recency_runs_from_cyan_to_a_dimmed_foreground_by_absolute_distance` still sees old `(125, 125, 115)`.

- [ ] **Step 5: Commit**

```bash
cargo fmt && just check
git add src/palette.rs src/style.rs
git commit -m "feat(palette): read magenta from TASKS_PALETTE and the terminal query"
```

---

### Task 2: The priority role, attached from the theme

One commit: `PriorityScale::new` and `Painter::with_priority_scale` have their only
production caller in `main.rs`, so committing the role before the wiring would fail
`just check`'s `-D warnings` on dead code.

**Files:**
- Modify: `src/palette.rs` (`#[cfg(test)] Rgb::lightness`)
- Modify: `src/style.rs` (`Style::Priority`, `PriorityScale`, `Painter::with_priority_scale`, unit tests)
- Modify: `src/output.rs` (`table`, `quiet_briefs`, `paint_field` use the role; `has_date_column` → `needs_theme`; add `shows_priority`; unit tests)
- Modify: `src/main.rs` (scale attachment, both warnings)
- Modify: `tests/common/mod.rs` (`TEST_PALETTE` gains magenta)
- Modify: `tests/cli.rs` (updated and new end-to-end tests)

**Interfaces:**
- Consumes: `Palette { magenta: Option<Rgb>, .. }`, `Palette::old`, `Rgb::mix` (Task 1).
- Produces:
  - `Style::Priority(u8)` (priority is validated 0–4 on every read path)
  - `#[derive(Debug, Clone, Copy)] pub struct PriorityScale { steps: [Rgb; 5] }`
  - `PriorityScale::new(magenta: Rgb, palette: &Palette) -> PriorityScale`
  - `PriorityScale::color(&self, priority: u8) -> Rgb`
  - `Painter::with_priority_scale(self, scale: PriorityScale) -> Painter`
  - `output::needs_theme(out: &Output) -> bool` (renamed from `has_date_column`, same outputs), `output::shows_priority(out: &Output) -> bool`

- [ ] **Step 1: Write the failing tests**

In `src/style.rs`'s `tests` module:

```rust
    fn magenta() -> Rgb {
        Rgb {
            r: 0xd7,
            g: 0x5f,
            b: 0xd7,
        }
    }

    #[test]
    fn the_priority_scale_samples_the_mix_at_fixed_steps() {
        let palette = palette();
        let scale = PriorityScale::new(magenta(), &palette);
        let old = palette.old();
        assert_eq!(scale.color(0), magenta());
        assert_eq!(scale.color(1), magenta());
        assert_eq!(scale.color(2), magenta().mix(old, 0.5));
        assert_eq!(scale.color(3), magenta().mix(old, 0.8));
        assert_eq!(scale.color(4), old);
        // The reference values the end-to-end tests pin.
        assert_eq!(scale.color(2), Rgb { r: 171, g: 115, b: 165 });
        assert_eq!(scale.color(3), Rgb { r: 144, g: 122, b: 135 });
    }

    #[test]
    fn with_this_fixture_the_steps_fall_in_lightness() {
        // A fixture property, not a guarantee: a theme whose magenta equals the old end
        // collapses the scale (spec §2.3).
        let scale = PriorityScale::new(magenta(), &palette());
        let lightness: Vec<f64> = (1..=4).map(|p| scale.color(p).lightness()).collect();
        assert!(
            lightness.windows(2).all(|pair| pair[0] > pair[1]),
            "{lightness:?}"
        );
    }

    #[test]
    fn the_priority_role_paints_the_scale_or_falls_back_to_bold() {
        let bare = Painter::new(ColorMode::Always, Format::Pretty, false);
        assert_eq!(bare.paint(Style::Priority(0), "P0"), "\x1b[1mP0\x1b[0m");
        assert_eq!(bare.paint(Style::Priority(1), "P1"), "\x1b[1mP1\x1b[0m");
        for p in 2..=4 {
            assert_eq!(bare.paint(Style::Priority(p), "Px"), "Px");
        }

        let scaled = Painter::new(ColorMode::Always, Format::Pretty, false)
            .with_priority_scale(PriorityScale::new(magenta(), &palette()));
        assert_eq!(
            scaled.paint(Style::Priority(0), "P0"),
            "\x1b[1;38;2;215;95;215mP0\x1b[0m"
        );
        assert_eq!(
            scaled.paint(Style::Priority(1), "P1"),
            "\x1b[38;2;215;95;215mP1\x1b[0m"
        );
        assert_eq!(
            scaled.paint(Style::Priority(4), "P4"),
            "\x1b[38;2;125;125;115mP4\x1b[0m"
        );

        let off = Painter::new(ColorMode::Never, Format::Pretty, true)
            .with_priority_scale(PriorityScale::new(magenta(), &palette()));
        assert_eq!(off.paint(Style::Priority(0), "P0"), "P0");
    }
```

In `src/output.rs`'s `tests` module, beside `dated()`:

```rust
    fn scaled() -> Painter {
        let palette = crate::palette::Palette::parse(
            "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7",
        )
        .unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false).with_priority_scale(
            crate::style::PriorityScale::new(palette.magenta.unwrap(), &palette),
        )
    }

    #[test]
    fn the_priority_column_carries_the_priority_role() {
        let mut urgent = row("xx-000001", false);
        urgent.priority = 0;
        let rows = [urgent, row("xx-000002", false)];
        let text = table(&rows, DateColumn::Updated, &scaled(), 0, false, false);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[1;38;2;215;95;215mP0\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(
            lines[1].contains("\x1b[38;2;171;115;165mP2\x1b[0m"),
            "{:?}",
            lines[1]
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-fast priority`
Expected: compile errors (`PriorityScale`, `Style::Priority`, `with_priority_scale` do not exist).

- [ ] **Step 3: Implement the role**

In `src/palette.rs`, beside `Rgb::mix`:

```rust
    /// OKLab lightness, for tests that check a scale's order.
    #[cfg(test)]
    pub fn lightness(self) -> f64 {
        to_oklab(self)[0]
    }
```

In `src/style.rs`, add the variant to `Style`:

```rust
    /// A priority, painted on the magenta scale when the painter has one
    /// (docs/specs/2026-09-25-priority-color-design.md), else bold at P0 and P1.
    Priority(u8),
```

After `Recency`'s impl:

```rust
/// How far each priority, P0 to P4, sits from magenta toward the old end. P0 and P1 share
/// a color; bold sets P0 apart. P2 is the default and the bulk, so it sits halfway.
const PRIORITY_STEPS: [f64; 5] = [0.0, 0.0, 0.5, 0.8, 1.0];

/// The five priority colors, fixed for one run.
#[derive(Debug, Clone, Copy)]
pub struct PriorityScale {
    steps: [Rgb; 5],
}

impl PriorityScale {
    pub fn new(magenta: Rgb, palette: &Palette) -> PriorityScale {
        let old = palette.old();
        PriorityScale {
            steps: PRIORITY_STEPS.map(|t| magenta.mix(old, t)),
        }
    }

    /// `priority` is validated 0-4 wherever a record is read.
    pub fn color(&self, priority: u8) -> Rgb {
        self.steps[usize::from(priority)]
    }
}
```

Give `Painter` the field and builder:

```rust
pub struct Painter {
    enabled: bool,
    recency: Option<Recency>,
    priority: Option<PriorityScale>,
}
```

(`Painter::new` sets `priority: None`.)

```rust
    /// Priorities paint on the scale only with one; without it P0 and P1 are bold.
    pub fn with_priority_scale(self, scale: PriorityScale) -> Painter {
        Painter {
            priority: Some(scale),
            ..self
        }
    }
```

In `paint`'s match, after the `Style::Date` arm:

```rust
            Style::Priority(priority) => match &self.priority {
                Some(scale) => {
                    let color = scale.color(priority);
                    let bold = if priority == 0 { "1;" } else { "" };
                    format!("{bold}38;2;{};{};{}", color.r, color.g, color.b)
                }
                None if priority <= 1 => "1".into(),
                None => return text.into(),
            },
```

- [ ] **Step 4: Paint every priority through the role**

In `src/output.rs`, `table`:

```rust
        let priority = painter.paint(Style::Priority(row.priority), &format!("P{}", row.priority));
```

replacing the `if row.priority <= 1 { … Emphasis … }` block. In `quiet_briefs`:

```rust
        let priority = match row.priority {
            Some(priority) => painter.paint(Style::Priority(priority), &format!("P{priority}")),
            None => "P-".into(),
        };
```

In `paint_field`:

```rust
        "priority" => Style::Priority(task.priority),
```

replacing `"priority" if task.priority <= 1 => Style::Emphasis,`.

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `just test-fast priority` then `just test-fast`
Expected: PASS, the existing `\x1b[1mP0\x1b[0m` list assertion and `show`'s `priority: \x1b[1m0\x1b[0m` included: `main` attaches no scale yet, so every view still renders the bold look. If the reference-value assertions in `the_priority_scale_samples_the_mix_at_fixed_steps` fail by one on a channel, see Global Constraints. Do not commit yet (see the note under the task heading).

- [ ] **Step 6: Write the failing end-to-end and gate tests**

In `tests/common/mod.rs`:

```rust
/// The palette every test child sees; its cyan paints today as `38;2;0;215;255` and its
/// magenta paints P1 as `38;2;215;95;215`.
pub const TEST_PALETTE: &str = "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7";
```

In `tests/cli.rs`, the colored `list` test (around the `assert!(colored.contains("\x1b[1mP0\x1b[0m"));` line) now expects the scale:

```rust
    assert!(colored.contains("\x1b[1;38;2;215;95;215mP0\x1b[0m"), "{colored:?}");
```

Its `strip_ansi(&colored) == plain` assertion stays: that is the layout check.

The colored `show` test keeps `priority: \x1b[1m0\x1b[0m`. Add beside it:

```rust
#[test]
fn colored_show_keeps_bold_priority_without_a_warning() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Urgent", "-p", "1"]));
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "show", &id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("priority: \x1b[1m1\x1b[0m\n"), "{text:?}");
    assert!(String::from_utf8(out.stderr).unwrap().is_empty());
}
```

New tests:

```rust
#[test]
fn colored_list_paints_priorities_on_the_magenta_scale() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for (title, priority) in [("One", "1"), ("Two", "2"), ("Three", "3"), ("Four", "4")] {
        env.json(&dir, &["add", title, "-p", priority]);
    }
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    for code in [
        "\x1b[38;2;215;95;215mP1\x1b[0m",
        "\x1b[38;2;171;115;165mP2\x1b[0m",
        "\x1b[38;2;144;122;135mP3\x1b[0m",
        "\x1b[38;2;125;125;115mP4\x1b[0m",
    ] {
        assert!(text.contains(code), "missing {code:?}: {text:?}");
    }
    assert!(String::from_utf8(out.stderr).unwrap().is_empty());
}

#[test]
fn a_palette_without_magenta_warns_only_where_priorities_show() {
    const THREE_KEYS: &str = "fg=#e5e3d7 bg=#13140d cyan=#00d7ff";
    const WARNING: &str =
        "priority colors off: TASKS_PALETTE has no magenta; add magenta=#rrggbb";
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Urgent", "-p", "1"]));
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["park", &id, "resume the thing"]);

    let list = env
        .cmd(&dir)
        .env("TASKS_PALETTE", THREE_KEYS)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let text = String::from_utf8(list.stdout).unwrap();
    assert!(text.contains("\x1b[1mP1\x1b[0m"), "bold look: {text:?}");
    assert!(text.contains("\x1b[38;2;0;215;255m"), "dates still paint: {text:?}");
    let stderr = String::from_utf8(list.stderr).unwrap();
    assert_eq!(stderr.matches(WARNING).count(), 1, "{stderr:?}");

    for args in [&["projects"][..], &["list", "--parked"][..]] {
        let out = env
            .cmd(&dir)
            .env("TASKS_PALETTE", THREE_KEYS)
            .args(["--pretty", "--color", "always"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
        assert!(
            String::from_utf8(out.stdout).unwrap().contains("\x1b[38;2;0;215;255m"),
            "{args:?}: dates paint"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(!stderr.contains("priority colors off"), "{args:?}: {stderr:?}");
    }
}

#[test]
fn a_malformed_magenta_is_a_config_error_whenever_set() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let out = env
        .cmd(&dir)
        .env("TASKS_PALETTE", "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=purple")
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
            .contains("magenta: \"purple\" is not #rrggbb")
    );
}
```

In `colored_quiet_paints_the_park_date_by_recency`, the task is added at the default P2; add after the date assertion:

```rust
    assert!(
        text.contains("\x1b[38;2;171;115;165mP2\x1b[0m"),
        "the brief's priority takes the scale: {text:?}"
    );
```

Replace the tail of `redirected_stdout_without_a_palette_skips_the_query_and_warns`. Give the test a P1 task (`env.json(&dir, &["add", "Fresh", "-p", "1"]);`), then:

```rust
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("38;2"), "no truecolor: {text:?}");
    assert!(text.contains("\x1b[1mP1\x1b[0m"), "P1 stays bold: {text:?}");
    for query in ["\x1b]10", "\x1b]11", "\x1b]4;", "\x1b[c"] {
        assert!(!text.contains(query), "no query bytes {query:?}: {text:?}");
    }
    let warning = String::from_utf8(out.stderr).unwrap();
    assert!(
        warning.contains(
            "theme colors off: the terminal did not report its colors (stdout is not a terminal); set TASKS_PALETTE to supply them"
        ),
        "{warning:?}"
    );
    assert!(!warning.contains("priority colors off"), "{warning:?}");
```

In `src/output.rs`, rename the unit test `date_columns_are_the_outputs_that_query` to `theme_gates_cover_dates_and_priorities` and make it:

```rust
    #[test]
    fn theme_gates_cover_dates_and_priorities() {
        let list = Output::List(ListOut {
            tasks: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        assert!(needs_theme(&list));
        assert!(shows_priority(&list));
        let parked = Output::Parked(ParkedOut {
            tasks: vec![],
            warnings: vec![],
        });
        assert!(needs_theme(&parked));
        assert!(!shows_priority(&parked));
        let id = Output::Id(IdOut {
            id: "xx-000001".into(),
            warnings: vec![],
        });
        assert!(!needs_theme(&id));
        assert!(!shows_priority(&id));
    }
```

- [ ] **Step 7: Run the tests to verify they fail**

Run: `just test-fast palette` then `just test-fast priorit` then `just test-fast theme`
Expected: the scale and warning tests fail (bold P0/P1, `date colors off` wording, no magenta warning); `theme_gates_cover_dates_and_priorities` fails to compile.

- [ ] **Step 8: Implement the gates**

In `src/output.rs`, rename `has_date_column` to `needs_theme` (updating its one caller in `main.rs`) and document it:

```rust
/// Whether a pretty rendering of `out` paints from the theme's colors, and so whether
/// the stdout painter needs them (date spec §3.1; priority spec §3.1). Every output with
/// a priority column also has a date column.
pub fn needs_theme(out: &Output) -> bool {
```

Add beside it:

```rust
/// Whether a pretty rendering of `out` shows priorities, and so whether a palette
/// without magenta costs it anything (priority spec §3.3).
pub fn shows_priority(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_) | Output::Prime(_) | Output::Tree(_) | Output::Quiet(_)
    )
}
```

- [ ] **Step 9: Attach the scale and warn**

In `src/main.rs`, replace the `date_warning` block:

```rust
            let mut theme_warning = None;
            let stdout_painter = if stdout_painter.enabled() && output::needs_theme(&out) {
                // Redirected stdout is never queried: a pager may be reading the same
                // terminal (date spec §3.2).
                let resolved = match palette {
                    Some(palette) => Ok(palette),
                    None if !std::io::stdout().is_terminal() => Err(palette::QueryError::NotAsked),
                    None => palette::Palette::query(palette::QUERY_TIMEOUT),
                };
                match resolved {
                    Ok(palette) => {
                        let painter = stdout_painter.with_recency(style::Recency::new(
                            ::time::OffsetDateTime::now_utc().date(),
                            &palette,
                        ));
                        match palette.magenta {
                            Some(magenta) => painter
                                .with_priority_scale(style::PriorityScale::new(magenta, &palette)),
                            None => {
                                if output::shows_priority(&out) {
                                    theme_warning = Some(
                                        "priority colors off: TASKS_PALETTE has no magenta; add magenta=#rrggbb"
                                            .to_string(),
                                    );
                                }
                                painter
                            }
                        }
                    }
                    Err(reason) => {
                        theme_warning = Some(format!(
                            "theme colors off: the terminal did not report its colors ({reason}); set TASKS_PALETTE to supply them"
                        ));
                        stdout_painter
                    }
                }
            } else {
                stdout_painter
            };
```

and change `warnings.extend(date_warning);` to `warnings.extend(theme_warning);`.

- [ ] **Step 10: Run the tests to verify they pass**

Run: `just test-fast`
Expected: PASS. Any other test still asserting `\x1b[1mP0\x1b[0m` or `\x1b[1mP1\x1b[0m` in a table or quiet brief under the default test palette now sees the scale's code: update it to `\x1b[1;38;2;215;95;215mP0\x1b[0m` / `\x1b[38;2;215;95;215mP1\x1b[0m`. Find them with `grep -n '1mP[01]' tests/cli.rs`.

- [ ] **Step 11: Commit**

```bash
cargo fmt && just check
git add src/palette.rs src/style.rs src/main.rs src/output.rs tests/common/mod.rs tests/cli.rs
git commit -m "feat(output): paint priorities on the theme's magenta scale"
```

---

### Task 3: Docs, gate, verify in kitty, close

**Files:**
- Modify: `README.md` (color paragraph)
- Modify: `docs/specs/2026-09-25-priority-color-design.md` (status line)
- Modify: `docs/specs/2026-09-25-date-recency-color-design.md` (status line notes the amendment)
- Modify: `docs/specs/2026-09-03-color-output-design.md` (one-line amendment note under its status)
- Modify: `tasks/tasks-006dfe.md`, `tasks/tasks-92757e.md` (through the binary only)

- [ ] **Step 1: README**

Replace the date paragraph in `README.md`'s color section (the one starting "With color on, date columns fade by age") with:

```markdown
With color on, date columns fade by age: today in the terminal theme's cyan, older dates
toward a dimmed foreground, anything two years or more away the same. Priorities take the
theme's magenta: P0 bold, P1 full, P2 and P3 fading, P4 at the same dimmed foreground. The
colors are read from the terminal itself (foreground, background and palette slots 5 and
6), and only when stdout is a terminal, since a pager reading the same terminal would race
the query. For piped output, a terminal that does not answer, or ends of your own, set
`TASKS_PALETTE="fg=#rrggbb bg=#rrggbb cyan=#rrggbb magenta=#rrggbb"` (`magenta` may be
left out, which keeps priorities bold at P0 and P1); otherwise dates print plain,
priorities bold at P0 and P1, with one warning saying why.
```

- [ ] **Step 2: Spec status lines**

- `docs/specs/2026-09-25-priority-color-design.md`: `**Status:** implemented (<date>), tasks-92757e; see docs/plans/2026-09-25-priority-color.md. Amends …` (keep the rest of the line).
- `docs/specs/2026-09-25-date-recency-color-design.md`: append to its status paragraph: `§3.1's query and §3.3's TASKS_PALETTE are extended by docs/specs/2026-09-25-priority-color-design.md (slot 5, optional magenta), and its warning is reworded there.`
- `docs/specs/2026-09-03-color-output-design.md`: under its status line, add `P0/P1 bold in the priority column is now the no-theme look; with the theme's colors, priorities paint on a magenta scale (docs/specs/2026-09-25-priority-color-design.md).`

- [ ] **Step 3: Gate**

Run: `just gate`
Expected: pass, including the ignored exhaustive tests.

- [ ] **Step 4: Commit docs**

```bash
git add README.md docs/specs
git commit -m "docs: priority magenta scale in the README and spec status lines"
```

- [ ] **Step 5: The user checks kitty**

Hand these to the user to run in their own terminal from the main checkout (an agent has no terminal):

```bash
env -u TASKS_PALETTE .worktrees/priority-color/target/debug/tasks --pretty --color always list
env -u TASKS_PALETTE .worktrees/priority-color/target/debug/tasks --pretty --color always list | less -R
TASKS_PALETTE="fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7" .worktrees/priority-color/target/debug/tasks --pretty --color always list | head -8
```

`env -u` matters: an exported `TASKS_PALETTE` would skip the query in the first and paint the pipe in the second.

Expected: the first paints priorities from the live theme's slot 5 (the olive `#c8cb88` today, so the steps show as lightness); the second queries nothing, keeps P0/P1 bold and warns `theme colors off`; the third paints a true magenta ramp. Park the task `--waiting-on user --reason review` while waiting.

- [ ] **Step 6: Close**

After the user confirms, close this step first: the tracker refuses to close a parent while a descendant is open.

```bash
tasks start tasks-006dfe
tasks done tasks-006dfe "README, spec status lines, gate and kitty check"
tasks done tasks-92757e "priority column paints on the theme's magenta scale (P0 bold … P4 dimmed fg); slot 5 in the query, optional magenta in TASKS_PALETTE, bold P0/P1 without the theme"
tasks check
git add tasks && git commit -m "chore(tasks): close priority magenta scale"
```

Then `cargo install --path .` from the merged main checkout so the installed tracker is the new code.
