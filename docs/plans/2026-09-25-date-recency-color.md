# Date Recency Color Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In colored `--pretty` output, paint every date-column value on a continuous scale from the terminal theme's cyan (today) to a dimmed foreground (two years or more away).

**Architecture:** A new `src/palette.rs` owns RGB values, OKLab mixing, the `TASKS_PALETTE` parser and the terminal color query. `src/style.rs` gains a `Style::Date` role whose color a `Recency` (today plus the two ends) computes. `src/main.rs` resolves the palette after the command runs (the environment variable, else the query when stdout is a terminal, else a warning) and hands the painter a `Recency`. `src/output.rs` paints its date cells with the new role.

**Tech Stack:** Rust 2024, `time` 0.3, new `terminal-trx` 0.2 (controlling terminal and raw mode), `xterm-color` 1 (X11 color string parsing), `libc` (poll).

**Spec:** `docs/specs/2026-09-25-date-recency-color-design.md`. Read it before starting; this plan argues from it.

## Global Constraints

- Scale: `t = min(1, ln(1 + age) / ln(1 + 730))`, `age = |today - date|` in whole days, UTC.
- Recent end = palette cyan; old end = foreground mixed `0.45` toward background, in OKLab.
- Date SGR is `38;2;r;g;b`; every other role keeps its existing code.
- Query: `OSC 10;?`, `OSC 11;?`, `OSC 4;6;?` each ST-terminated, then DA1 `CSI c`; timeout 300 ms.
- Query only when: pretty format, stdout painter enabled, the output has a date column, `TASKS_PALETTE` unset, and stdout is a terminal.
- The query reads and writes the controlling terminal through `terminal-trx`, never stdout.
- `TASKS_PALETTE="fg=#rrggbb bg=#rrggbb cyan=#rrggbb"`: all three keys, any order, space separated; validated whenever set; malformed is a `config` error before any work.
- Warning text: `date colors off: the terminal did not report its colors (<reason>); set TASKS_PALETTE to supply them`.
- Reasons: `stdout is not a terminal`, `timed out after 300 ms`, `answered without the colors`, `unparsable reply …`, `no terminal to ask`.
- `TERM` is never consulted.
- Commands: `just test-fast [<name>]` while working, `just check` before each commit (the pre-commit hook runs it), `just gate` before finishing. Never run `cargo test` directly.
- Commits: conventional, no AI attribution. Never edit `tasks/*.md` by hand.

## Review Focus

1. **A reply split across reads** — a terminal (or ssh) delivers `ESC ] 1 0 ; rgb:…` in several chunks; parsing must not depend on one read returning a whole reply. Test: Task 3's one-byte-per-read fake.
2. **Two-digit and four-digit reply components** — terminals answer `rgb:e5/e3/d7` as well as `rgb:e5e5/e3e3/d7d7`; both must give `#e5e3d7`. Test: Task 3.
3. **`now` and `-` in the due column** — `now` is today (full cyan); `-` is not a date and stays unpainted. Test: Task 1's output unit test.
4. **Future dates** — a deferral 10 days ahead takes the same color as a date 10 days ago. Test: Task 1's `Recency` unit test.
5. **`TASKS_PALETTE` set while color is off** — no escapes appear, JSON is unchanged, and a malformed value still fails as `config`. Test: Task 1's end-to-end tests.

---

### Task 1: `TASKS_PALETTE` colors the list date column end to end

**Files:**
- Create: `src/palette.rs`
- Modify: `src/main.rs` (module list; `TASKS_PALETTE` validation after `TASKS_COLOR`; recency attachment after `commands::run`)
- Modify: `src/time.rs` (add `calendar_day`)
- Modify: `src/style.rs` (`When`, `Recency`, `fade`, `Style::Date`, `Painter::with_recency`, `Painter::enabled`)
- Modify: `src/output.rs` (`has_date_column`, `date_role`, `table`'s date cell, unit tests)
- Modify: `tests/common/mod.rs` (both helpers set `TASKS_PALETTE`)
- Modify: `tests/cli.rs` (`strip_ansi` becomes generic; new end-to-end tests)

**Interfaces:**
- Produces (`src/palette.rs`):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }`
  - `Rgb::parse_hex(hex: &str) -> Option<Rgb>` (exactly `#rrggbb`)
  - `Rgb::mix(self, other: Rgb, t: f64) -> Rgb` (OKLab; exact ends at `t <= 0` and `t >= 1`)
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Palette { pub fg: Rgb, pub bg: Rgb, pub cyan: Rgb }`
  - `Palette::parse(value: &str) -> crate::error::Result<Palette>` (errors are `Error::Config`)
- Produces (`src/style.rs`):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum When { Today, On(time::Date) }`
  - `Style::Date(When)`
  - `pub fn fade(age_days: u64) -> f64`
  - `#[derive(Debug, Clone, Copy)] pub struct Recency { today, recent, old }` with `Recency::new(today: time::Date, palette: &Palette) -> Recency` and `Recency::color(&self, when: When) -> Rgb`
  - `Painter::with_recency(self, recency: Recency) -> Painter`, `Painter::enabled(&self) -> bool`
- Produces (`src/time.rs`): `pub fn calendar_day(day: &str) -> Result<time::Date>`
- Produces (`src/output.rs`): `pub fn has_date_column(out: &Output) -> bool`, `fn date_role(timestamp: &str) -> Style`

- [ ] **Step 1: Write the palette unit tests**

Create `src/palette.rs` with only the tests module and the type skeleton the tests name, so the failure is an assertion or a missing function, not a missing file:

```rust
//! The terminal theme's colors: RGB values, OKLab mixing, and where they come from
//! (`TASKS_PALETTE`, or a query to the terminal). Design:
//! docs/specs/2026-09-25-date-recency-color-design.md.

use crate::error::{Error, Result};

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(value: &str) -> Rgb {
        Rgb::parse_hex(value).unwrap()
    }

    #[test]
    fn hex_parses_exactly_rrggbb() {
        assert_eq!(hex("#e5e3d7"), Rgb { r: 0xe5, g: 0xe3, b: 0xd7 });
        assert_eq!(hex("#00D7FF"), Rgb { r: 0, g: 0xd7, b: 0xff });
        for bad in ["e5e3d7", "#e5e3d", "#e5e3d7f", "#g5e3d7", "#", "", "#+5e3d7"] {
            assert_eq!(Rgb::parse_hex(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn mix_returns_the_exact_ends_and_darkens_toward_the_background() {
        let fg = hex("#e5e3d7");
        let bg = hex("#13140d");
        assert_eq!(fg.mix(bg, 0.0), fg);
        assert_eq!(fg.mix(bg, 1.0), bg);
        assert_eq!(fg.mix(bg, -0.5), fg);
        assert_eq!(fg.mix(bg, 1.5), bg);
        // The spec's old end for this theme, from a reference OKLab implementation.
        assert_eq!(fg.mix(bg, 0.45), Rgb { r: 125, g: 125, b: 115 });
    }

    #[test]
    fn palette_parses_three_keys_in_any_order() {
        let palette = Palette::parse("cyan=#00d7ff fg=#e5e3d7  bg=#13140d").unwrap();
        assert_eq!(
            palette,
            Palette {
                fg: hex("#e5e3d7"),
                bg: hex("#13140d"),
                cyan: hex("#00d7ff"),
            }
        );
    }

    #[test]
    fn palette_rejects_missing_unknown_duplicate_and_malformed_entries() {
        for (value, needle) in [
            ("fg=#e5e3d7 bg=#13140d", "missing cyan"),
            ("", "missing fg, bg, cyan"),
            ("fg=#e5e3d7 bg=#13140d cyan=#00d7ff red=#ff0000", "unknown key \"red\""),
            ("fg=#e5e3d7 fg=#e5e3d7 bg=#13140d cyan=#00d7ff", "fg is given twice"),
            ("fg=e5e3d7 bg=#13140d cyan=#00d7ff", "fg: \"e5e3d7\" is not #rrggbb"),
            ("fg bg=#13140d cyan=#00d7ff", "\"fg\" is not key=#rrggbb"),
        ] {
            let error = Palette::parse(value).unwrap_err();
            assert_eq!(error.kind(), "config", "{value:?}");
            let text = error.to_string();
            assert!(text.contains("TASKS_PALETTE"), "{value:?}: {text}");
            assert!(text.contains(needle), "{value:?}: {text}");
        }
    }
}
```

Add `mod palette;` to the module list in `src/main.rs`, between `mod output;` and `mod periodic;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `just test-fast palette::`
Expected: compile errors naming `Rgb` and `Palette` as not found.

- [ ] **Step 3: Implement `Rgb` and `Palette::parse`**

Add above the tests module in `src/palette.rs`:

```rust
/// One 8-bit sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Exactly `#rrggbb`, either case; anything else is `None`.
    pub fn parse_hex(hex: &str) -> Option<Rgb> {
        let digits = hex.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Rgb {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
        })
    }

    /// The color `t` of the way from `self` to `other`, interpolated in OKLab so equal
    /// steps of `t` look like equal steps of change. The ends are returned exactly, not
    /// round-tripped through floating point.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        let from = to_oklab(self);
        let to = to_oklab(other);
        from_oklab([
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
            from[2] + (to[2] - from[2]) * t,
        ])
    }
}

// OKLab, after Björn Ottosson, "A perceptual color space for image processing" (2020).

fn to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f64) -> u8 {
    let c = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_oklab(color: Rgb) -> [f64; 3] {
    let (r, g, b) = (to_linear(color.r), to_linear(color.g), to_linear(color.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

fn from_oklab([l, a, b]: [f64; 3]) -> Rgb {
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Rgb {
        r: from_linear(4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_),
        g: from_linear(-1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_),
        b: from_linear(-0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_),
    }
}

/// The three theme colors the date scale needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cyan: Rgb,
}

impl Palette {
    /// Parses `TASKS_PALETTE`: `fg=#rrggbb bg=#rrggbb cyan=#rrggbb`, any order.
    pub fn parse(value: &str) -> Result<Palette> {
        let bad = |detail: String| {
            Error::Config(format!(
                "TASKS_PALETTE must be \"fg=#rrggbb bg=#rrggbb cyan=#rrggbb\": {detail}"
            ))
        };
        let (mut fg, mut bg, mut cyan) = (None, None, None);
        for entry in value.split_whitespace() {
            let (key, hex) = entry
                .split_once('=')
                .ok_or_else(|| bad(format!("{entry:?} is not key=#rrggbb")))?;
            let slot = match key {
                "fg" => &mut fg,
                "bg" => &mut bg,
                "cyan" => &mut cyan,
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
            (Some(fg), Some(bg), Some(cyan)) => Ok(Palette { fg, bg, cyan }),
            _ => {
                let missing: Vec<&str> = [("fg", fg), ("bg", bg), ("cyan", cyan)]
                    .into_iter()
                    .filter(|(_, color)| color.is_none())
                    .map(|(key, _)| key)
                    .collect();
                Err(bad(format!("missing {}", missing.join(", "))))
            }
        }
    }
}
```

Check `Error::kind()` and `Display` exist on `crate::error::Error` (they do: `kind()` returns `"config"` for `Error::Config`, as `tests/cli.rs` relies on). If a channel of the `0.45` mix comes out one off from `125, 125, 115`, do not loosen the test: compare `to_oklab`/`from_oklab` constants against the spec reference line by line first.

- [ ] **Step 4: Run the palette tests**

Run: `just test-fast palette::`
Expected: 4 passed. `just check` still fails on dead code; that is expected until Step 12.

- [ ] **Step 5: Write the style unit tests**

Append to `src/style.rs`'s `tests` module:

```rust
    use crate::palette::{Palette, Rgb};
    use ::time::{Date, Month};

    fn day(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).unwrap()
    }

    fn palette() -> Palette {
        Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap()
    }

    #[test]
    fn fade_is_logarithmic_and_clipped_at_two_years() {
        for (age, expected) in [
            (0, 0.0),
            (1, 0.105),
            (7, 0.315),
            (28, 0.511),
            (91, 0.684),
            (365, 0.895),
            (730, 1.0),
            (5000, 1.0),
        ] {
            assert!((fade(age) - expected).abs() < 0.001, "age {age}: {}", fade(age));
        }
    }

    #[test]
    fn recency_runs_from_cyan_to_a_dimmed_foreground_by_absolute_distance() {
        let today = day(2026, Month::September, 25);
        let recency = Recency::new(today, &palette());
        let cyan = Rgb { r: 0, g: 0xd7, b: 0xff };
        let old = Rgb { r: 125, g: 125, b: 115 };
        assert_eq!(recency.color(When::Today), cyan);
        assert_eq!(recency.color(When::On(today)), cyan);
        assert_eq!(recency.color(When::On(day(2023, Month::January, 1))), old);
        let past = recency.color(When::On(day(2026, Month::September, 15)));
        let future = recency.color(When::On(day(2026, Month::October, 5)));
        assert_eq!(past, future, "ten days either side is the same distance");
        assert_ne!(past, cyan);
        assert_ne!(past, old);
    }

    #[test]
    fn the_date_role_paints_truecolor_only_with_a_recency() {
        let today = day(2026, Month::September, 25);
        let bare = Painter::new(ColorMode::Always, Format::Pretty, false);
        assert_eq!(bare.paint(Style::Date(When::Today), "2026-09-25"), "2026-09-25");
        let dated = bare.with_recency(Recency::new(today, &palette()));
        assert_eq!(
            dated.paint(Style::Date(When::Today), "2026-09-25"),
            "\x1b[38;2;0;215;255m2026-09-25\x1b[0m"
        );
        let off = Painter::new(ColorMode::Never, Format::Pretty, true)
            .with_recency(Recency::new(today, &palette()));
        assert_eq!(off.paint(Style::Date(When::Today), "2026-09-25"), "2026-09-25");
        assert!(dated.enabled());
        assert!(!off.enabled());
    }
```

- [ ] **Step 6: Run them to see them fail**

Run: `just test-fast style::`
Expected: compile errors naming `fade`, `Recency`, `When`, `Style::Date`, `with_recency`, `enabled`.

- [ ] **Step 7: Implement the date role in `src/style.rs`**

Add at the top, after the existing `use` lines:

```rust
use crate::palette::{Palette, Rgb};
```

Add after the `Style` enum, and add the `Date(When)` variant to `Style` with this doc line:

```rust
    /// A date, painted by its distance from today (docs/specs/2026-09-25-date-recency-color-design.md).
    Date(When),
```

```rust
/// Which day a date cell shows. `Today` is the due column's `now`, which names no date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Today,
    On(time::Date),
}

/// Days beyond which every date looks the same.
const HORIZON_DAYS: f64 = 730.0;

/// How far along the scale a date `age_days` from today sits: 0 is today, 1 is the
/// horizon or beyond. Logarithmic, so the first weeks spread across half the range.
pub fn fade(age_days: u64) -> f64 {
    ((1.0 + age_days as f64).ln() / (1.0 + HORIZON_DAYS).ln()).min(1.0)
}

/// How far the old end sits from the foreground toward the background.
const OLD_TOWARD_BACKGROUND: f64 = 0.45;

/// Today and the two ends of the date scale, fixed for one run.
#[derive(Debug, Clone, Copy)]
pub struct Recency {
    today: time::Date,
    recent: Rgb,
    old: Rgb,
}

impl Recency {
    pub fn new(today: time::Date, palette: &Palette) -> Recency {
        Recency {
            today,
            recent: palette.cyan,
            old: palette.fg.mix(palette.bg, OLD_TOWARD_BACKGROUND),
        }
    }

    /// The color for `when`, by its absolute distance from today.
    pub fn color(&self, when: When) -> Rgb {
        let day = match when {
            When::Today => self.today,
            When::On(day) => day,
        };
        let age = (self.today - day).whole_days().unsigned_abs();
        self.recent.mix(self.old, fade(age))
    }
}
```

Change `Painter` and its `impl`:

```rust
pub struct Painter {
    enabled: bool,
    recency: Option<Recency>,
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
            recency: None,
        }
    }

    /// Dates paint only with a recency; without one `Style::Date` is the identity.
    pub fn with_recency(self, recency: Recency) -> Painter {
        Painter {
            recency: Some(recency),
            ..self
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Wraps `text` without changing its visible width, so callers pad first and paint
    /// last. `text` must not contain a newline: the reset has to land before any break.
    pub fn paint(&self, style: Style, text: &str) -> String {
        debug_assert!(
            !text.contains('\n'),
            "paint spans a newline; the reset would land after the break: {text:?}"
        );
        let code: String = match style {
            Style::Date(when) => match &self.recency {
                Some(recency) => {
                    let color = recency.color(when);
                    format!("38;2;{};{};{}", color.r, color.g, color.b)
                }
                None => return text.into(),
            },
            Style::Status(Status::Idea) => "34".into(),
            Style::Status(Status::Todo) => return text.into(),
            Style::Status(Status::Doing) => "33".into(),
            Style::Status(Status::Blocked) => "31".into(),
            Style::Status(Status::Shelved) => "2;34".into(),
            Style::Status(Status::Done) => "2;32".into(),
            Style::Status(Status::Dropped) => "2;31".into(),
            Style::Chrome => "2".into(),
            Style::Emphasis => "1".into(),
            Style::Error => "31".into(),
            Style::Ok => "32".into(),
            Style::Warning => "33".into(),
        };
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.into()
        }
    }
}
```

Inside `src/style.rs`, `time::Date` resolves to the `time` crate: `crate::time` is not imported into `style.rs`. The existing `painter_obeys_format_mode_stream_and_roles` test lists fixed codes per role; `Style::Date` has no fixed code, so it stays out of that list and is covered by `the_date_role_paints_truecolor_only_with_a_recency`.

- [ ] **Step 8: Run the style tests**

Run: `just test-fast style::`
Expected: all style tests pass.

- [ ] **Step 9: Write the output unit tests**

Add `calendar_day` to `src/time.rs`, after `day`:

```rust
/// The calendar date of a `YYYY-MM-DD` day, as `day` returns it.
pub fn calendar_day(day: &str) -> Result<time::Date> {
    time::Date::parse(day, &time::format_description::well_known::Iso8601::DATE)
        .map_err(|e| Error::Validation(format!("bad date {day:?}: {e}")))
}
```

Append to `src/output.rs`'s `tests` module:

```rust
    fn dated() -> Painter {
        let today = ::time::Date::from_calendar_date(2026, ::time::Month::September, 6).unwrap();
        let palette = crate::palette::Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false)
            .with_recency(crate::style::Recency::new(today, &palette))
    }

    #[test]
    fn the_date_column_carries_the_recency_role() {
        // row() is updated 2026-09-06, the painter's today: full cyan.
        let rows = [row("xx-000001", false)];
        let text = table(&rows, DateColumn::Updated, &dated(), 0, false, false);
        assert!(text.contains("\x1b[38;2;0;215;255m2026-09-06\x1b[0m"), "{text:?}");
    }

    #[test]
    fn a_due_column_paints_now_as_today_and_leaves_a_dash_plain() {
        let due_now = recurring("xx-000001");
        let undated = row("xx-000002", false);
        let text = table(&[due_now, undated], DateColumn::Due, &dated(), 0, false, false);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("\x1b[38;2;0;215;255mnow\x1b[0m"), "{:?}", lines[0]);
        assert!(lines[1].contains("todo    -  "), "{:?}", lines[1]);
        assert!(!lines[1].contains("38;2"), "{:?}", lines[1]);
    }

    #[test]
    fn date_columns_are_the_outputs_that_query() {
        let list = Output::List(ListOut {
            tasks: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        assert!(has_date_column(&list));
        let id = Output::Id(IdOut {
            id: "xx-000001".into(),
            warnings: vec![],
        });
        assert!(!has_date_column(&id));
    }
```

- [ ] **Step 10: Run them to see them fail**

Run: `just test-fast output::tests`
Expected: compile errors for `has_date_column`; the date assertions fail once it compiles.

- [ ] **Step 11: Paint `table`'s date cell and add `has_date_column`**

In `src/output.rs`, import the role type beside the existing style import: `use crate::style::When;` (keep the existing `Style` and `Painter` imports).

Add near `table`:

```rust
/// The date role for a validated timestamp or `YYYY-MM-DD` day.
fn date_role(timestamp: &str) -> Style {
    let day = crate::time::calendar_day(crate::time::day(timestamp))
        .expect("records and park entries carry validated dates");
    Style::Date(When::On(day))
}

/// Whether a pretty rendering of `out` has a date column, and so whether the stdout
/// painter needs the terminal's colors (spec §2.3, §3.1).
pub fn has_date_column(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Tree(_)
            | Output::Parked(_)
            | Output::Quiet(_)
            | Output::Projects(_)
    )
}
```

In `table`, replace the `let date = match date { … };` block with one that also yields the role, and paint it:

```rust
        let (date, role) = match date {
            DateColumn::Updated => (
                crate::time::day(&row.updated).to_string(),
                Some(date_role(&row.updated)),
            ),
            DateColumn::Created => (
                crate::time::day(&row.created).to_string(),
                Some(date_role(&row.created)),
            ),
            DateColumn::Due => match (&row.periodic, &row.deferred) {
                (Some(periodic), _) => match (&periodic.due, periodic.due_now) {
                    (Some(due), _) => (crate::time::day(due).to_string(), Some(date_role(due))),
                    (None, true) => ("now".into(), Some(Style::Date(When::Today))),
                    (None, false) => ("-".into(), None),
                },
                (None, Some(deferred)) => {
                    (deferred.until.clone(), Some(date_role(&deferred.until)))
                }
                (None, None) => ("-".into(), None),
            },
        };
        let date = match role {
            Some(role) => painter.paint(role, &date),
            None => date,
        };
```

- [ ] **Step 12: Resolve `TASKS_PALETTE` in `main.rs` and attach the recency**

In `src/main.rs`, directly after the `color_mode` resolution, validate the variable whenever it is set (spec §3.3):

```rust
    let palette = match std::env::var("TASKS_PALETTE") {
        Ok(value) => match palette::Palette::parse(&value) {
            Ok(palette) => Some(palette),
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
                    "TASKS_PALETTE must be valid UTF-8, got {value:?}"
                )))
            ));
            std::process::exit(1);
        }
    };
```

In the `Ok(out)` arm of `match commands::run(cli)`, before the warnings are printed, attach the recency when a palette is set (Task 3 adds the query and the warning for the unset case):

```rust
            let stdout_painter = match palette {
                Some(palette) if stdout_painter.enabled() && output::has_date_column(&out) => {
                    stdout_painter.with_recency(style::Recency::new(
                        ::time::OffsetDateTime::now_utc().date(),
                        &palette,
                    ))
                }
                _ => stdout_painter,
            };
```

`::time` names the crate: `main.rs` declares `mod time;`, which shadows it.

- [ ] **Step 13: Isolate the test environment and generalize `strip_ansi`**

In `tests/common/mod.rs`, add to **both** `cmd` and `raw`, right after `.env_remove("NO_COLOR")`:

```rust
            // Never query the terminal of whoever runs the suite, and never inherit a
            // malformed value: TASKS_PALETTE is validated whenever it is set.
            .env("TASKS_PALETTE", TEST_PALETTE)
```

and at the top of the file, after the `use` lines:

```rust
/// The palette every test child sees; its cyan paints today as `38;2;0;215;255`.
pub const TEST_PALETTE: &str = "fg=#e5e3d7 bg=#13140d cyan=#00d7ff";
```

In `tests/cli.rs`, replace `strip_ansi` with a generic SGR stripper, since dates now carry `38;2;r;g;b` codes:

```rust
fn strip_ansi(text: &str) -> String {
    let mut plain = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("\x1b[") {
        plain.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('m').expect("an SGR sequence ends in m");
        rest = &after[end + 1..];
    }
    plain.push_str(rest);
    plain
}
```

- [ ] **Step 14: Write the end-to-end tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn colored_list_paints_dates_by_recency() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let fresh = env.json(&dir, &["add", "Fresh"]);
    let today = fresh["updated"].as_str().unwrap()[..10].to_string();
    write_doc(
        &dir,
        "tasks/sci-a00001.md",
        "---\nid: sci-a00001\ntitle: Ancient\nstatus: todo\npriority: 2\ncreated: 2020-01-01T00:00:00Z\nupdated: 2020-01-01T00:00:00Z\ndepends: []\ntags: []\n---\n",
    );
    let out = env
        .cmd(&dir)
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
        String::from_utf8(out.stderr).unwrap().is_empty(),
        "a palette from the environment needs no warning"
    );
}

#[test]
fn a_palette_without_color_changes_nothing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Fresh"]);
    let pretty = env.cmd(&dir).args(["--pretty", "list"]).output().unwrap();
    assert!(!String::from_utf8(pretty.stdout).unwrap().contains('\x1b'));
    let json = env
        .cmd(&dir)
        .args(["--color", "always", "list"])
        .output()
        .unwrap();
    assert!(!String::from_utf8(json.stdout).unwrap().contains('\x1b'));
}

#[test]
fn a_malformed_palette_is_a_config_error_whenever_set() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    for args in [&["list"][..], &["--pretty", "--color", "never", "list"][..]] {
        let out = env
            .cmd(&dir)
            .env("TASKS_PALETTE", "fg=#e5e3d7 bg=#13140d")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config", "{args:?}");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(
            detail.contains("TASKS_PALETTE") && detail.contains("missing cyan"),
            "{args:?}: {detail}"
        );
    }
}
```

- [ ] **Step 15: Run the tests and the check**

Run: `just test-fast`
Expected: all pass. `projects`, `parked` and `quiet` are not painted until Task 2, so their existing tests are unchanged here; any failure is a regression to fix.

Run: `just check`
Expected: pass (no dead code: every new item is reachable from `main`).

- [ ] **Step 16: Commit**

```bash
git add src/palette.rs src/style.rs src/time.rs src/output.rs src/main.rs tests/common/mod.rs tests/cli.rs
git commit -m "feat(style): recency color for the list date column from TASKS_PALETTE"
```

---

### Task 2: The remaining date columns take the role

**Files:**
- Modify: `src/output.rs` (`parked_table`, `quiet_briefs`, `projects`' activity cell)
- Modify: `tests/cli.rs` (`projects_pretty_paints_counts_by_status_and_dims_zeros`; a parked test)

**Interfaces:**
- Consumes: `date_role(timestamp: &str) -> Style` from Task 1; `TEST_PALETTE` from `tests/common/mod.rs`.

- [ ] **Step 1: Write the failing tests**

In `tests/cli.rs`, change the last two lines of `projects_pretty_paints_counts_by_status_and_dims_zeros` from the "nothing after the last reset is painted" assertion to:

```rust
    // total carries no status; activity is a date and takes the recency role
    assert!(row.contains("      1  \x1b[38;2;0;215;255m"), "{row:?}");
    assert!(row.ends_with("\x1b[0m"), "{row:?}");
```

Append a parked test:

```rust
#[test]
fn colored_parked_paints_the_park_date() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Paused"]));
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["park", &id, "resume the thing"]);
    let out = env
        .cmd(&dir)
        .args(["--pretty", "--color", "always", "list", "--parked"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let today = env.json(&dir, &["show", &id])["task"]["updated"]
        .as_str()
        .unwrap()[..10]
        .to_string();
    assert!(
        text.contains(&format!("\x1b[38;2;0;215;255m{today}\x1b[0m")),
        "{text:?}"
    );
}
```

`start` then `park` needs no session setup under `TestEnv`; `list --parked` renders through `parked_table`.

- [ ] **Step 2: Run them to see them fail**

Run: `just test-fast projects_pretty_paints && just test-fast colored_parked`
Expected: both fail on the missing `38;2` sequence.

- [ ] **Step 3: Paint the three cells**

In `parked_table`, paint the park date before formatting:

```rust
        let parked = painter.paint(date_role(&park.at), crate::time::day(&park.at));
        rendered.push_str(&format!(
            "{id}  {status} {process:<7} {phase} waits on {:<18} {parked}  {}\n",
            crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes),
            row.title
        ));
```

In `quiet_briefs`:

```rust
        let parked = painter.paint(date_role(&park.at), crate::time::day(&park.at));
        rendered.push_str(&format!(
            "{id}  {priority}  {needs:<8}  {minutes:>8}  parked {parked}  {}\n",
            row.title
        ));
```

In the `Output::Projects` arm, the activity cell:

```rust
                    (Some(_), Some(at)) => {
                        cell(crate::time::day(at), Align::Left, Some(date_role(at)))
                    }
```

`grid_text` pads before it paints, so the column stays aligned.

- [ ] **Step 4: Run the tests**

Run: `just test-fast`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/output.rs tests/cli.rs
git commit -m "feat(output): park and activity dates take the recency role"
```

---

### Task 3: Query the terminal when `TASKS_PALETTE` is unset

**Files:**
- Modify: `Cargo.toml` (dependencies)
- Modify: `src/palette.rs` (`QueryError`, `QUERY_TIMEOUT`, `exchange`, `query`, `Timed`)
- Modify: `src/main.rs` (the unset path and the warning)
- Modify: `tests/cli.rs` (redirect-policy test)
- Modify: `README.md` (the color paragraph)

**Interfaces:**
- Consumes: `Rgb`, `Palette` from Task 1.
- Produces (`src/palette.rs`):
  - `pub const QUERY_TIMEOUT: Duration = Duration::from_millis(300);`
  - `#[derive(Debug)] pub enum QueryError { NotAsked, NoTerminal, TimedOut(Duration), Unsupported, Unparsable(String), Io(io::Error) }` with `Display` giving the spec's reason text
  - `pub fn exchange<T: Read + Write>(tty: &mut T, timeout: Duration) -> Result<Palette, QueryError>`
  - `Palette::query(timeout: Duration) -> Result<Palette, QueryError>`

- [ ] **Step 1: Add the dependencies**

In `Cargo.toml` under `[dependencies]`, add:

```toml
libc = "0.2"
terminal-trx = "0.2"
xterm-color = "1"
```

Run: `cargo build` (a build, not a test; it only resolves and compiles the new crates).
Expected: success; `Cargo.lock` gains `terminal-trx` and `xterm-color`.

- [ ] **Step 2: Write the exchange tests**

Append to `src/palette.rs`'s `tests` module:

```rust
    use std::io::{self, Read, Write};
    use std::time::Duration;

    /// A terminal that answers with `replies`, `chunk` bytes per read, then times out.
    struct Fake {
        replies: Vec<u8>,
        at: usize,
        chunk: usize,
        sent: Vec<u8>,
    }

    impl Fake {
        fn new(replies: &[u8], chunk: usize) -> Fake {
            Fake {
                replies: replies.to_vec(),
                at: 0,
                chunk,
                sent: Vec::new(),
            }
        }
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.at == self.replies.len() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no reply"));
            }
            let n = self.chunk.min(buf.len()).min(self.replies.len() - self.at);
            buf[..n].copy_from_slice(&self.replies[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const DA1: &[u8] = b"\x1b[?62;22c";
    const T: Duration = Duration::from_millis(300);

    fn replies(fg: &str, bg: &str, cyan: &str, end: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (prefix, color) in [("10;", fg), ("11;", bg), ("4;6;", cyan)] {
            bytes.extend_from_slice(format!("\x1b]{prefix}{color}{end}").as_bytes());
        }
        bytes.extend_from_slice(DA1);
        bytes
    }

    fn expected() -> Palette {
        Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap()
    }

    #[test]
    fn exchange_sends_the_three_queries_and_the_fence() {
        let mut tty = Fake::new(
            &replies("rgb:e5e5/e3e3/d7d7", "rgb:1313/1414/0d0d", "rgb:0000/d7d7/ffff", "\x1b\\"),
            64,
        );
        assert_eq!(exchange(&mut tty, T).unwrap(), expected());
        assert_eq!(
            tty.sent,
            b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c".to_vec()
        );
        assert_eq!(tty.at, tty.replies.len(), "the fence reply is consumed too");
    }

    #[test]
    fn exchange_accepts_bel_two_digit_components_and_one_byte_reads() {
        let bytes = replies("rgb:e5/e3/d7", "rgb:13/14/0d", "rgb:00/d7/ff", "\x07");
        assert_eq!(exchange(&mut Fake::new(&bytes, 1), T).unwrap(), expected());
    }

    #[test]
    fn a_fence_before_the_colors_means_unsupported() {
        let err = exchange(&mut Fake::new(DA1, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
        let mut partial = b"\x1b]10;rgb:e5/e3/d7\x07\x1b]11;rgb:13/14/0d\x07".to_vec();
        partial.extend_from_slice(DA1);
        let err = exchange(&mut Fake::new(&partial, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
        assert_eq!(err.to_string(), "answered without the colors");
    }

    #[test]
    fn a_garbled_reply_is_unparsable_and_silence_is_a_timeout() {
        let bytes = replies("rgb:zz/e3/d7", "rgb:13/14/0d", "rgb:00/d7/ff", "\x07");
        let err = exchange(&mut Fake::new(&bytes, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unparsable(_)), "{err:?}");
        assert!(err.to_string().starts_with("unparsable reply"), "{err}");

        let err = exchange(&mut Fake::new(b"", 64), T).unwrap_err();
        assert!(matches!(err, QueryError::TimedOut(_)), "{err:?}");
        assert_eq!(err.to_string(), "timed out after 300 ms");
    }

    #[test]
    fn the_reasons_read_as_the_spec_words_them() {
        assert_eq!(QueryError::NotAsked.to_string(), "stdout is not a terminal");
        assert_eq!(QueryError::NoTerminal.to_string(), "no terminal to ask");
    }
```

- [ ] **Step 3: Run them to see them fail**

Run: `just test-fast palette::`
Expected: compile errors naming `exchange` and `QueryError`.

- [ ] **Step 4: Implement the exchange and the query**

Add to `src/palette.rs` (below `Palette`'s `impl`; put the `use` lines at the top of the file):

```rust
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

/// How long the whole exchange may take before the terminal counts as silent.
pub const QUERY_TIMEOUT: Duration = Duration::from_millis(300);

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// Foreground, background and palette slot 6, each ST-terminated, then primary device
/// attributes (DA1) as a fence: terminals answer in order, so the fence arriving before
/// a color reply means the terminal does not answer color queries.
const QUERY: &[u8] = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c";

/// Why the terminal's colors are unavailable. `Display` is the reason the warning names.
#[derive(Debug)]
pub enum QueryError {
    /// stdout is redirected, so the terminal is not asked (spec §3.2).
    NotAsked,
    NoTerminal,
    TimedOut(Duration),
    Unsupported,
    Unparsable(String),
    Io(io::Error),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::NotAsked => f.write_str("stdout is not a terminal"),
            QueryError::NoTerminal => f.write_str("no terminal to ask"),
            QueryError::TimedOut(after) => write!(f, "timed out after {} ms", after.as_millis()),
            QueryError::Unsupported => f.write_str("answered without the colors"),
            QueryError::Unparsable(reply) => write!(f, "unparsable reply {reply:?}"),
            QueryError::Io(error) => write!(f, "terminal error: {error}"),
        }
    }
}

/// Runs the query over `tty` (a raw-mode terminal in production, a fake in tests). A
/// read that fails with `TimedOut` becomes `QueryError::TimedOut(timeout)`.
pub fn exchange<T: Read + Write>(tty: &mut T, timeout: Duration) -> Result<Palette, QueryError> {
    let timed_out = |error: io::Error| match error.kind() {
        io::ErrorKind::TimedOut => QueryError::TimedOut(timeout),
        _ => QueryError::Io(error),
    };
    tty.write_all(QUERY).map_err(QueryError::Io)?;
    tty.flush().map_err(QueryError::Io)?;
    let mut reader = BufReader::with_capacity(64, tty);
    let mut reply = |prefix: &[u8]| -> Result<Rgb, QueryError> {
        read_reply(&mut reader, prefix).map_err(|error| match error {
            QueryError::Io(io) => timed_out(io),
            other => other,
        })
    };
    let fg = reply(b"10;")?;
    let bg = reply(b"11;")?;
    let cyan = reply(b"4;6;")?;
    // The fence's reply is still in flight; read it so it never reaches the shell.
    skip_fence(&mut reader, true).map_err(timed_out)?;
    Ok(Palette { fg, bg, cyan })
}

fn read_byte(reader: &mut impl BufRead) -> io::Result<u8> {
    let mut byte = [0u8];
    reader.read_exact(&mut byte)?;
    Ok(byte[0])
}

/// One `ESC ] <prefix><color> (BEL | ESC \)` reply, or `Unsupported` if the fence
/// (`ESC [ … c`) comes first.
fn read_reply(reader: &mut impl BufRead, prefix: &[u8]) -> Result<Rgb, QueryError> {
    while read_byte(reader).map_err(QueryError::Io)? != ESC {}
    match read_byte(reader).map_err(QueryError::Io)? {
        b']' => {}
        b'[' => {
            skip_fence(reader, false).map_err(QueryError::Io)?;
            return Err(QueryError::Unsupported);
        }
        other => return Err(QueryError::Unparsable(format!("ESC {:?}", other as char))),
    }
    let mut body = Vec::new();
    loop {
        match read_byte(reader).map_err(QueryError::Io)? {
            BEL => break,
            ESC => {
                if read_byte(reader).map_err(QueryError::Io)? != b'\\' {
                    return Err(QueryError::Unparsable(String::from_utf8_lossy(&body).into()));
                }
                break;
            }
            byte => body.push(byte),
        }
    }
    let unparsable = || QueryError::Unparsable(String::from_utf8_lossy(&body).into_owned());
    let color = body.strip_prefix(prefix).ok_or_else(unparsable)?;
    let color = xterm_color::Color::parse(color).map_err(|_| unparsable())?;
    Ok(Rgb {
        r: (color.red >> 8) as u8,
        g: (color.green >> 8) as u8,
        b: (color.blue >> 8) as u8,
    })
}

/// Reads through the DA1 reply's final `c`; `from_start` also skips to its `ESC [`.
fn skip_fence(reader: &mut impl BufRead, from_start: bool) -> io::Result<()> {
    if from_start {
        while read_byte(reader)? != ESC {}
    }
    while read_byte(reader)? != b'c' {}
    Ok(())
}

impl Palette {
    /// Asks the controlling terminal (never stdout) for its colors, in raw mode, bounded
    /// by `timeout`. The raw-mode guard restores the terminal when it drops, on every path.
    pub fn query(timeout: Duration) -> Result<Palette, QueryError> {
        let mut terminal = terminal_trx::terminal().map_err(|_| QueryError::NoTerminal)?;
        if !terminal.has_connected_stdio_stream() {
            return Err(QueryError::NoTerminal);
        }
        let mut lock = terminal.lock();
        let raw = lock.enable_raw_mode().map_err(QueryError::Io)?;
        let mut timed = Timed {
            inner: raw,
            deadline: Instant::now() + timeout,
        };
        exchange(&mut timed, timeout)
    }
}

/// A terminal whose reads fail with `TimedOut` once the deadline passes, so a silent
/// terminal cannot hang the run.
struct Timed<T> {
    inner: T,
    deadline: Instant,
}

impl<T: Read + std::os::fd::AsRawFd> Read for Timed<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        let millis = left.as_millis().min(i32::MAX as u128) as libc::c_int;
        let mut fd = libc::pollfd {
            fd: self.inner.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `fd` is one valid pollfd for the duration of the call, and the count
        // passed is 1.
        let ready = unsafe { libc::poll(&mut fd, 1, millis) };
        match ready {
            0 => Err(io::Error::new(io::ErrorKind::TimedOut, "terminal did not answer")),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => self.inner.read(buf),
        }
    }
}

impl<T: Write> Write for Timed<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
```

`terminal_trx::RawModeGuard` implements `Read`, `Write` and (on Unix) `AsRawFd`. `tasks` builds for Unix only; the `libc::poll` path is Unix code.

- [ ] **Step 5: Run the exchange tests**

Run: `just test-fast palette::`
Expected: all pass. `just check` fails on dead code for `query` until Step 7.

- [ ] **Step 6: Write the redirect-policy end-to-end test**

Append to `tests/cli.rs`:

```rust
#[test]
fn redirected_stdout_without_a_palette_skips_the_query_and_warns() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    env.json(&dir, &["add", "Fresh"]);
    // The harness pipes stdout, which is exactly the redirect the policy covers.
    let out = env
        .cmd(&dir)
        .env_remove("TASKS_PALETTE")
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("38;2"), "no date colors: {text:?}");
    for query in ["\x1b]10", "\x1b]11", "\x1b]4;", "\x1b[c"] {
        assert!(!text.contains(query), "no query bytes {query:?}: {text:?}");
    }
    let warning = String::from_utf8(out.stderr).unwrap();
    assert!(
        warning.contains(
            "date colors off: the terminal did not report its colors (stdout is not a terminal); set TASKS_PALETTE to supply them"
        ),
        "{warning:?}"
    );
}
```

Run: `just test-fast redirected_stdout`
Expected: FAIL (no warning yet).

- [ ] **Step 7: Resolve the unset palette in `main.rs`**

Replace the Task 1 recency block in the `Ok(out)` arm with the full resolution, and feed the warning into the pretty warnings:

```rust
            let mut date_warning = None;
            let stdout_painter = if stdout_painter.enabled() && output::has_date_column(&out) {
                // Redirected stdout is never queried: a pager may be reading the same
                // terminal (spec §3.2).
                let resolved = match palette {
                    Some(palette) => Ok(palette),
                    None if !std::io::stdout().is_terminal() => {
                        Err(palette::QueryError::NotAsked)
                    }
                    None => palette::Palette::query(palette::QUERY_TIMEOUT),
                };
                match resolved {
                    Ok(palette) => stdout_painter.with_recency(style::Recency::new(
                        ::time::OffsetDateTime::now_utc().date(),
                        &palette,
                    )),
                    Err(reason) => {
                        date_warning = Some(format!(
                            "date colors off: the terminal did not report its colors ({reason}); set TASKS_PALETTE to supply them"
                        ));
                        stdout_painter
                    }
                }
            } else {
                stdout_painter
            };
```

and change the warnings call in the `format == Format::Pretty` block to:

```rust
                let mut warnings = output::warnings_of(&out);
                warnings.extend(date_warning);
                to_stderr(&output::pretty_warnings(&warnings, &stderr_painter));
```

`IsTerminal` is already imported in `main.rs` (it calls `std::io::stdout().is_terminal()`); if not, add `use std::io::IsTerminal;`.

- [ ] **Step 8: Run the suite and the check**

Run: `just test-fast`
Expected: all pass.

Run: `just check`
Expected: pass.

- [ ] **Step 9: Document `TASKS_PALETTE` in the README**

In `README.md`, after the paragraph that ends "and an explicit `--color` overrides it.", add:

```markdown
With color on, date columns fade by age: today in the terminal theme's cyan, older dates
toward a dimmed foreground, anything two years or more away the same. The colors are read
from the terminal itself (foreground, background and palette slot 6), and only when stdout
is a terminal, since a pager reading the same terminal would race the query. For piped
output, a terminal that does not answer, or ends of your own, set
`TASKS_PALETTE="fg=#rrggbb bg=#rrggbb cyan=#rrggbb"`; otherwise dates print plain with one
warning saying why.
```

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml Cargo.lock src/palette.rs src/main.rs tests/cli.rs README.md
git commit -m "feat(palette): read date colors from the terminal when stdout is a terminal"
```

---

### Task 4: Gate, verify in a real terminal, close

**Files:**
- Modify: `docs/specs/2026-09-25-date-recency-color-design.md` (status line)
- Modify: `tasks/tasks-142d2f.md` (through the binary only)

- [ ] **Step 1: Run the whole gate**

Run: `just gate`
Expected: pass, including the ignored exhaustive tests.

- [ ] **Step 2: Check the no-answer path in a pseudo-terminal**

From `.worktrees/date-colors`, `script` gives the run a pseudo-terminal that nothing answers:

```bash
time env -u TASKS_PALETTE script -qec './target/debug/tasks --pretty --color always list' /dev/null | tail -3
```

Expected: dates uncolored, a `date colors off: … (timed out after 300 ms)` warning, and a wall time a little over 0.3 s. (The query bytes appear in `script`'s own output here: they were written to its pseudo-terminal, which is what `script` records. That is not stdout of `tasks`.)

- [ ] **Step 3: The user checks kitty**

Hand these to the user to run in their own terminal from the worktree (an agent has no terminal):

```bash
.worktrees/date-colors/target/debug/tasks --pretty --color always list
.worktrees/date-colors/target/debug/tasks --pretty --color always list | head -5
.worktrees/date-colors/target/debug/tasks --pretty --color always list | less -R
TASKS_PALETTE="fg=#e5e3d7 bg=#13140d cyan=#00d7ff" .worktrees/date-colors/target/debug/tasks --pretty --color always list | head -5
```

Expected: the first paints dates from the live theme; the second and third print plain dates plus the warning, with no stray characters in `less` or at the next prompt; the fourth paints the piped dates. The user reports what they saw; fix anything they report before Step 4.

- [ ] **Step 4: Mark the spec implemented and close the task in one commit**

Change the spec's status line to:

```markdown
**Status:** implemented (2026-09-25), tasks-142d2f; see docs/plans/2026-09-25-date-recency-color.md.
```

Then, from `.worktrees/date-colors`:

```bash
tasks done tasks-142d2f "Pretty date columns fade by age from the terminal's cyan to a dimmed foreground, read by OSC query when stdout is a terminal or from TASKS_PALETTE"
tasks check
git add docs/specs/2026-09-25-date-recency-color-design.md tasks/tasks-142d2f.md
git commit -m "docs(spec): date recency color implemented"
```

After merging to `main`, reinstall so the tracker is the code under test: `cargo install --path .`
