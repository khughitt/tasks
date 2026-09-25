# Recency color for pretty date columns — design

**Status:** draft (2026-09-25), tasks-142d2f. Amends docs/specs/2026-09-03-color-output-design.md
§2 ("no 256-color or truecolor values") and §6 for date columns only.

## 1. Problem

Every pretty table carries a date column, and reading it means reading digits. How
recently a task moved is the question that column answers, and it is a question of
degree: yesterday, last week and last month should look different at a glance, while
last year and three years ago need not.

The color-output design gives dates no treatment. Its palette is the sixteen ANSI slots
plus dim and bold, which can express a few steps but not a continuous scale.

## 2. Decision

When color is on, each date in a date column is painted on a continuous scale by its
distance from today: today in the theme's cyan at full strength, older dates fading
toward a dimmed foreground, and everything past two years the same faded color.

The colors come from the terminal's own theme, read from the terminal at run time, so the
scale follows whatever theme the user runs without tasks knowing about any theme tool.
This keeps the old design's principle, "the palette is the terminal's", and changes only
the means: the ends of the scale are the terminal's colors as RGB, and the steps between
them are 24-bit SGR codes (`38;2;r;g;b`).

### 2.1 The scale

For a date `d` and today's UTC date `today`:

```
age = |today - d| in whole days
t   = min(1, ln(1 + age) / ln(1 + 730))
```

The log compresses the tail and the clip at 730 days flattens it, so outliers cannot
squash the range:

| age      | t    |
|----------|------|
| 0 days   | 0.00 |
| 1 day    | 0.10 |
| 1 week   | 0.32 |
| 4 weeks  | 0.51 |
| 3 months | 0.68 |
| 1 year   | 0.89 |
| 2+ years | 1.00 |

The first month spans half the range; the second year spans a tenth.

The distance is absolute, so a future date (a due or deferral date) is saturated when it
is close and fades as it recedes, in the same way a past date does. `now` in the due
column is age 0. `-` is not a date and stays unpainted.

### 2.2 The ends

Three colors are read from the terminal: foreground (`OSC 10`), background (`OSC 11`), and
palette slot 6, cyan (`OSC 4;6`).

- **Recent** (`t = 0`): cyan.
- **Old** (`t = 1`): the foreground mixed 45% of the way toward the background.

The old end is a dimmed foreground, not the foreground itself, so the scale always has a
lightness contrast even when a theme's cyan is close to its foreground. A generated theme
makes this real: the current Noctalia palette on the author's machine has foreground
`#e5e3d7` and color6 `#c7ca9a`, which a plain foreground-to-cyan scale would barely
separate.

Both the mix and the interpolation run in OKLab, so equal steps of `t` look like equal
steps of change and the midpoint of two saturated colors does not go muddy.

### 2.3 What carries it

The date column and nothing else:

- `table`'s date column (updated, created or due), in `list`, `ready`, `tree` and `prime`.
- The park date in `parked` and `prime`'s parked section, and in `quiet`'s briefs.
- `projects`' activity column.

Not painted: `show`'s task text and footers, `prime`'s `periodic:` and `deferred:`
summary lines, and the trailing `every …, due …` / `due …` / `defer …` markers. Those
markers keep their bold emphasis, which says why the row is listed, not how old it is.

## 3. Reading the theme

### 3.1 The query

One exchange with the terminal, made only when all of these hold:

- the output format is pretty and the stdout painter is enabled (so never for an agent:
  color is opt-in and `auto` needs a terminal);
- the output has a date column (§2.3), so `tasks add --pretty` never touches the terminal;
- `TASKS_PALETTE` is unset (§3.3).

The query writes `OSC 10;?`, `OSC 11;?` and `OSC 4;6;?`, each ST-terminated, followed by a
primary device attributes request (`CSI c`, DA1) as a fence, and reads replies until the
DA1 reply arrives. Nearly every terminal answers DA1, so one that does not support a color
query is detected by the fence arriving without the color reply, not by waiting out the
timeout. The whole exchange is bounded by a 300 ms timeout for a terminal that answers
nothing at all.

It goes through the controlling terminal, never stdout: `terminal-trx` opens the first of
stderr, stdin, stdout and `/dev/tty` that is a terminal, puts it in raw mode behind a guard
that restores it on drop, and reads and writes there. So `tasks --pretty --color always
list | head` still queries the terminal, and the query bytes never land in the piped
output. (`xterm-query` was considered and rejected on exactly this point: it writes its
query to stdout.)

Replies are parsed with `xterm-color`, which parses the `rgb:rrrr/gggg/bbbb` X11 color
strings terminals send back.

Dependencies added: `terminal-trx` (libc only on Unix) and `xterm-color`. Polling the
tty descriptor for the timeout uses `libc::poll` (libc is already in the lock file).
`terminal-colorsaurus` wraps the same two crates but reads only foreground and background,
so it cannot supply cyan.

`TERM` is still never consulted (color-output §2): `TERM=dumb` gets no special case.
A terminal that cannot answer takes the no-answer path below.

### 3.2 When the terminal does not answer

An unsupported terminal, a timeout, a multiplexer that swallows the query, or a pager
reading the terminal at the same moment all end the same way: dates are printed without
color, exactly as today, and one line goes to stderr:

```
warning: date colors off: the terminal did not report its colors (timed out after 300 ms); set TASKS_PALETTE to supply them
```

The reason names what happened (timed out, answered without the colors, unparsable reply,
no terminal). Everything else in the output keeps its color. Nothing falls back silently
to other colors.

### 3.3 `TASKS_PALETTE`

```
TASKS_PALETTE="fg=#e5e3d7 bg=#13140d cyan=#c7ca9a"
```

When set, the query is skipped and these colors are used. It exists for three reasons:
a terminal that cannot answer, a user who wants different ends than the theme gives, and
tests, which must never query the terminal of the developer running them.

All three keys are required, each a `#rrggbb` value, separated by spaces, in any order.
Like `TASKS_COLOR`, it is validated whenever it is set, even when color ends up off, and a
malformed value is a `config` error before any work is done.

`TestEnv::cmd` in `tests/cli.rs` sets a fixed `TASKS_PALETTE` for every child, so no test
reaches the query, beside removing `TASKS_COLOR` and `NO_COLOR` as it does today.

## 4. Shape

- **`src/palette.rs`** — `Rgb`, `Palette { fg, bg, cyan }`, `Palette::from_env(value)`
  parsing `TASKS_PALETTE`, and `Palette::query(timeout)` running §3.1. The exchange is
  written against a `Read + Write` pair plus a readiness wait, so the protocol (fence
  handling, reply order, parse errors) is unit-tested with in-memory streams. The
  `terminal-trx` handle is the only production implementation.
- **`src/style.rs`** — the painter gains a date role:

  ```rust
  pub enum Style { …, Date(time::Date) }

  pub struct Recency { today: time::Date, recent: Rgb, old: Rgb }

  impl Painter {
      pub fn with_recency(self, recency: Recency) -> Painter;
  }
  ```

  `Recency::new(today, &palette)` computes the two ends (§2.2). `paint(Style::Date(d), …)`
  computes `t` (§2.1), interpolates in OKLab, and emits `38;2;r;g;b`. A painter without a
  `Recency`, or a disabled one, returns the date text unchanged. Call sites still name a
  role, never a color, and still pad first and paint last.
- **`src/main.rs`** — after the command runs, if the stdout painter is enabled and the
  output has a date column, it resolves the palette (`TASKS_PALETTE`, else the query) and
  attaches a `Recency` built with today's UTC date. A failed query becomes the stderr
  warning of §3.2 through the stderr painter, and the stdout painter stays without one.
- **`src/output.rs`** — the §2.3 call sites paint their date cell with `Style::Date`.
  `pretty` stays a pure function of its inputs: today and the palette arrive inside the
  painter, so tests pin both.

The JSON contract is untouched: nothing here is reachable from the JSON branch.

## 5. Testing

Unit (`src/style.rs`):

- `t` at the table's ages in §2.1, including the clip past 730 days and a future date
  taking its absolute distance;
- OKLab interpolation returns the exact ends at `t = 0` and `t = 1`, and the old end sits
  between foreground and background in lightness;
- `Style::Date` preserves visible width, and is the identity without a `Recency`.

Unit (`src/palette.rs`):

- `TASKS_PALETTE` accepts keys in any order and rejects a missing key, an unknown key and
  a malformed hex value, each with a message naming the problem;
- the exchange, over in-memory streams: all three replies then the fence → a palette;
  the fence first → "answered without the colors"; a garbled reply → a parse error;
  replies terminated by BEL and by ST both parse; nothing ready before the timeout →
  timeout.

End to end (`tests/cli.rs`):

- with `--pretty --color always` and a fixed `TASKS_PALETTE`, `list` paints today's date
  with the cyan's exact SGR and a date three years old with the old end's;
- the same command without `--color` has no escape sequences;
- a colored table keeps the same visible column layout as an uncolored one;
- a malformed `TASKS_PALETTE` is a `config` error even without `--color`.

Manual: in kitty, `tasks --pretty --color always list` reads the live theme, and the same
command piped through `head` does too without escape bytes in the piped text; in a terminal
multiplexer, either the colors arrive or the §3.2 warning does.

## 6. Out of scope

Caching the queried palette between runs; choosing a palette slot other than 6; a
256-color quantization for terminals without truecolor; coloring dates in `show` or in
prose lines; a date scale horizon other than 730 days; any configuration of the scale.
