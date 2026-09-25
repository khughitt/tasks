# Magenta scale for the pretty priority column — design

**Status:** implemented (2026-09-25), tasks-92757e; see docs/plans/2026-09-25-priority-color.md. Amends docs/specs/2026-09-03-color-output-design.md
§6 (P0 and P1 bold) and docs/specs/2026-09-25-date-recency-color-design.md §3.1 (the
query) and §3.3 (`TASKS_PALETTE`). §2's steps and end amended the same day after the
kitty check (§2.4).

## 1. Problem

The priority column marks P0 and P1 bold and leaves P2, P3 and P4 identical. That shows
urgent work, but it cannot show that a row sits below the norm, and it cannot tell P0
from P1 at a glance. Priority is ordinal, with five values, so a few distinct steps are
enough. The date column already reads the terminal's theme and mixes colors in OKLab
(the date spec §2.2), so a priority scale can reuse that machinery.

## 2. Decision

When color is on and the theme's colors are known, each priority cell is painted with a
step on a scale from the theme's magenta (palette slot 5) to a deeply dimmed foreground:

| priority | step `t` | extra |
|----------|----------|-------|
| P0       | 0        | bold  |
| P1       | 0        |       |
| P2       | 1/3      |       |
| P3       | 2/3      |       |
| P4       | 1        |       |

The color is `magenta.mix(end, t)`: the same `Rgb::mix` the date scale uses, toward its
own `end`, the foreground mixed 75% of the way toward the background. The steps are fixed
points rather than a formula because priority has no magnitude to feed one. P0 and P1
share a color, and bold is what sets P0 apart. There are almost no P0 tasks, and a hue
stronger than full magenta does not exist.

### 2.1 Without the theme's colors

When the painter has no priority scale, because stdout is redirected without
`TASKS_PALETTE`, the query failed, `TASKS_PALETTE` has no `magenta`, or the view never
queries (`show`), the priority role renders as it does today: P0 and P1 bold, the rest
plain. This is the documented look when the theme is unknown, not a silent fallback:
every view that needs the theme (§3.1) and lacks the scale says so on stderr (§3.3).
`show` needs no theme and prints no warning, as it does for dates today.

### 2.2 What carries it

- `table`'s priority column in `list`, `ready`, `tree` and `prime`.
- The priority in `quiet`'s briefs. `P-` (no priority recorded) stays unpainted. Today
  these briefs are never bold. With the role they gain P0/P1 bold in the no-scale case,
  which matches the table.
- `show`'s `priority:` field paints through the same role but never queries, so it keeps
  the §2.1 look, as it does for dates.

`parked` and `projects` have no priority column.

### 2.3 A theme whose slot 5 is not magenta

Generated themes do not promise that slot 5 is a magenta, or that it differs from
slot 6. The Noctalia theme on the author's machine today has color5 `#c8cb88` (an olive)
and color6 `#c9c8a9`, so the priority and date columns would share a hue there. The scale
follows the theme anyway: "the palette is the terminal's" is the principle both designs
rest on. A user who wants a different hue sets all four colors in `TASKS_PALETTE`, and
that turns the query off.

How far apart the steps sit depends on the theme too. The scale runs between two colors
the theme supplies, and nothing keeps them apart: `fg=#e5e3d7 bg=#13140d magenta=#404038`
is a valid palette whose magenta equals the scale's end, so all five priorities
paint the same color and only P0's bold remains. tasks prescribes the mixes, not how
distinct they look. It does not detect or correct a collapsed scale.

### 2.4 Why the end is dimmer than the date scale's

The first version ran to the date scale's old end (the foreground mixed 45% toward the
background) with P2 at 0.5 and P3 at 0.8, so P4 matched a two-year-old date and P2, the
default and 410 of the 518 open tasks on 2026-09-25, sat halfway. In kitty, P1 to P3 looked
nearly the same in both the live theme and a true magenta. Magenta and that old end are
close in lightness (OKLab 0.68 and 0.59 for the test palette), so the steps differed mostly
in chroma: adjacent OKLab distances of 0.12, 0.07 and 0.05. The dimmer end with even steps
gives 0.13, 0.13 and 0.12, and about 0.15 in the live theme. P4 no longer matches an old
date, and P2 carries more color than before; both were accepted for steps that read as
distinct. A variant that kept P2 and P3 magenta and lowered only their lightness separated
as well but needed a second mixing function; it was not taken.

## 3. Reading the theme

### 3.1 The query

The query from the date spec §3.1 gains `OSC 4;5;?`, sent between `OSC 11;?` and
`OSC 4;6;?`, and so asks for four colors before the DA1 fence. The reply parser needs four
replies. Fewer than four is `Unsupported` ("answered without the colors"), as fewer than
three was. More than four is still unparsable. When a terminal answers slot 6, it
answers slot 5 too, so dates lose nothing in practice.

The conditions for querying do not change. The gate that decides whether an output needs
the theme is `has_date_column` today. It is renamed `needs_theme` and covers the same
outputs (`list`, `prime`, `tree`, `parked`, `quiet`, `projects`): every one with a
priority column already has a date column.

### 3.2 `TASKS_PALETTE`

```
TASKS_PALETTE="fg=#e5e3d7 bg=#13140d cyan=#c7ca9a magenta=#d75fd7"
```

`magenta` is optional. `fg`, `bg` and `cyan` stay required, so every value that
validates today still validates. `magenta` follows the rules of the other keys: `#rrggbb`,
given once, any order. A malformed or repeated `magenta` is a `config` error. Unknown
keys are still errors.

The error message for a malformed value names the optional key:
`TASKS_PALETTE must be "fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]": …`.

### 3.3 Warnings

The date spec's §3.2 warning covers both scales now, so it no longer names dates:

```
warning: theme colors off: the terminal did not report its colors (timed out after 300 ms); set TASKS_PALETTE to supply them
```

A `TASKS_PALETTE` without `magenta` paints dates and leaves priorities in the §2.1 look.
It adds one line whenever the output displays priorities: `list`, `prime`, `tree` and
`quiet`. `parked` and `projects` need the theme for their dates but show no priority, so
every color they display works and they print nothing. The query gate (§3.1) stays shared;
this warning has its own, `shows_priority`.

```
warning: priority colors off: TASKS_PALETTE has no magenta; add magenta=#rrggbb
```

Both warnings go through the stderr painter, like the date warning today. Neither fires
when color is off or the output needs no theme, and the magenta warning never fires
alongside the query failure warning, which already covers it.

## 4. Shape

- **`src/palette.rs`**: `Palette` gains `magenta: Option<Rgb>`. `parse` accepts it,
  `parse_replies` reads four replies and always fills it, and `QUERY` gains `OSC 4;5;?`.
  The date scale's old end moves here as `Palette::old(&self) -> Rgb`.
- **`src/style.rs`**:

  ```rust
  pub enum Style { …, Priority(u8) }

  pub struct PriorityScale { steps: [Rgb; 5] }

  impl PriorityScale {
      pub fn new(magenta: Rgb, palette: &Palette) -> PriorityScale;
  }

  impl Painter {
      pub fn with_priority_scale(self, scale: PriorityScale) -> Painter;
  }
  ```

  `paint(Style::Priority(p), …)` emits `38;2;r;g;b` from the scale, prefixed `1;` at P0.
  Without a scale it emits `1` at P0 and P1 and nothing otherwise. `Recency::new` takes
  its old end from `Palette::old`.
- **`src/main.rs`**: with a palette resolved, the stdout painter gets its `Recency` as
  today. When `palette.magenta` is `Some`, it also gets a `PriorityScale`. When it is
  `None` and `output::shows_priority` holds, the §3.3 warning is queued. The query failure warning is reworded.
- **`src/output.rs`**: the `table` and `quiet_briefs` priority cells and `show`'s
  `priority:` field paint with `Style::Priority`. Pad first, paint last, as before.
  `has_date_column` becomes `needs_theme`, and `shows_priority` is added beside it.
- **`tests/common/mod.rs`**: `TEST_PALETTE` gains `magenta=#d75fd7`, so the whole suite
  runs with both scales on. Tests of the three-key case set their own value.

The JSON contract is untouched.

## 5. Testing

Unit (`src/style.rs`):

- `PriorityScale` returns magenta exactly at P0 and P1, the end exactly at P4, and
  exactly `magenta.mix(end, 1/3)` and `magenta.mix(end, 2/3)` at P2 and P3;
- with the test palette, whose magenta is lighter and more saturated than the
  end, P1 through P4 fall strictly in OKLab lightness. This asserts the fixture, not a
  guarantee for every theme (§2.3);
- `Style::Priority` with a scale: P0 is `1;38;2;…`, P1 through P4 are `38;2;…`, and
  visible width is preserved;
- without a scale: P0 and P1 bold, P2 through P4 unchanged; a disabled painter changes
  nothing.

Unit (`src/palette.rs`):

- `parse` accepts three keys (magenta `None`) and four in any order, and rejects a
  repeated or malformed `magenta`;
- the exchange: four replies then the fence give a palette with magenta; three replies
  then the fence give `Unsupported`.

End to end (`tests/cli.rs`):

- `list --pretty --color always` with the four-key test palette paints P1 with magenta's
  exact SGR, P0 with the same color bold, and P2 to P4 with their mixes' SGRs;
- the same with a three-key palette paints P0 and P1 bold, colors dates, and prints the
  §3.3 magenta warning;
- `projects` and `parked` with a three-key palette print no magenta warning, and color
  their dates;
- `quiet` paints a brief's priority;
- a colored table keeps the visible layout of an uncolored one (the existing check,
  whose rows now span several priorities);
- `TASKS_PALETTE` with a malformed `magenta` is a `config` error even without `--color`;
- the redirected, unset `TASKS_PALETTE` case prints the reworded warning, with P0 and P1
  bold.

Manual, in kitty: `tasks --pretty --color always list` paints priorities from the live
theme's slot 5. Piped through `less -R` with no `TASKS_PALETTE`, it queries nothing and
leaves P0 and P1 bold.

## 6. Out of scope

Setting one color in `TASKS_PALETTE` while the rest come from the query; a palette slot
other than 5; configurable steps; painting priority in prose lines (`prime`'s summaries,
`show`'s footers).
