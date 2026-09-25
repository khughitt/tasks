# Configurable color source: terminal theme or built-in defaults — design

**Status:** implemented (2026-09-25), tasks-ee6ca2; see docs/plans/2026-09-25-color-source.md.
Amends docs/specs/2026-09-25-priority-color-design.md §2.3 (the problem this answers) and
docs/specs/2026-09-25-date-recency-color-design.md (the query it can replace).

## 1. Problem

Today the only source for the truecolor scales is the terminal's theme, read by the OSC
query or `TASKS_PALETTE`. A generated theme can make palette slot 5 an olive nearly equal
to slot 6 (priority-color design §2.3), so the priority and date columns share a hue, and
the query itself has costs: up to 300 ms of startup latency, a raw-mode exchange that can
collide with type-ahead, and a "theme colors off" warning whenever a terminal does not
answer. A user who wants a stable, distinct look today has to hand-write four hex colors
in `TASKS_PALETTE`.

The ask: let users choose where the scales' colors come from — the terminal theme, as
today, or a built-in palette tasks ships.

## 2. Decision

A new environment variable, `TASKS_THEME`, selects the color source:

| value       | meaning                                                                 |
|-------------|-------------------------------------------------------------------------|
| unset       | `terminal` (today's behavior, unchanged)                                |
| `terminal`  | the terminal theme: `TASKS_PALETTE` if set, else the OSC query          |
| `default`   | the built-in palette; no query is sent. `TASKS_PALETTE`, when also set, wins (§2.2) |

Anything else is a config error naming `TASKS_THEME`, the way `TASKS_COLOR` rejects an
unknown mode. `TASKS_THEME` does not interact with `TASKS_COLOR` or `NO_COLOR`: those
decide whether color is on at all; `TASKS_THEME` decides which colors it uses.

### 2.1 The built-in palette

| slot    | color      |
|---------|------------|
| fg      | `#e5e3d7`  |
| bg      | `#13140d`  |
| cyan    | `#00d7ff`  |
| magenta | `#d75fd7`  |

These are the four colors the designs and tests have used as the representative palette
all along. `magenta` is always present, so the built-in palette never produces the "no
magenta" warning. (An overriding `TASKS_PALETTE` with only its three required keys still
does, exactly as today.)

`default` is a complete palette: it ships `fg` and `bg` too, so no query is sent. Nothing
paints `fg` or `bg` directly — they are only consumed through `Palette::old()` and the two
mixes toward them — but the mixed results become text foregrounds, and they are fixed.
The old-date endpoint is `#7d7d73` and the priority endpoint `#404038`: on a light
terminal the light `#7d7d73` sits close to the background in lightness, while the dark
`#404038` gains contrast there. No fixed palette can guarantee contrast against an
arbitrary background, and `default` does not try: it is intended for dark backgrounds,
not a scheme that adapts to any theme. The user confirms it looks right in their terminal
(a visual check; §4's tests only verify the arithmetic), and a user on a light theme
keeps `terminal` or supplies a `TASKS_PALETTE`.

### 2.2 Precedence: `TASKS_PALETTE` wins

When both are set, `TASKS_PALETTE` wins and `TASKS_THEME=default` has no effect.
`TASKS_PALETTE` names exact colors — the most explicit statement of intent — and the
existing pattern is that the most explicit setting wins (for color mode: `--color`, then
a non-empty `NO_COLOR`, then `TASKS_COLOR`). A user who sets both is replacing the
built-in palette wholesale with their own — partial overrides do not exist, the parser
requires `fg`, `bg` and `cyan` — not asking to be ignored. With `TASKS_THEME=terminal`
(or unset) behavior is exactly today's.

This is the one question where the alternative was genuinely close: `default` could have
suppressed `TASKS_PALETTE`, on the theory that choosing built-ins means "stop reading my
theme". Rejected: `TASKS_PALETTE` is not part of the theme, it is a tasks setting, and
silently ignoring an explicit palette the user set yesterday is worse than the one extra
line a user needs (`unset TASKS_PALETTE`).

### 2.3 Global, not per-scale

One setting governs both scales. Per-scale variables (`TASKS_THEME_DATE`,
`TASKS_THEME_PRIORITY`) multiply the surface for a need nothing has shown: the motivation
is a single generated theme whose slot 5 is olive, and both columns live in the same
window.

## 3. Mechanics

`palette::query` is called only when the resolved source is `terminal` and
`TASKS_PALETTE` is unset — the current condition. With `default`, `main` builds the
palette from the constants; the code path past that point (recency, priority scale,
warnings) is shared and unchanged. Concretely, `main`'s palette resolution becomes:

1. `TASKS_PALETTE` set and valid → that palette (query off; unchanged, including the
   no-magenta warning path).
2. else `TASKS_THEME` unset or `terminal` → query, as today.
3. else `TASKS_THEME=default` → the built-in palette, no query, no warning.

`TASKS_PALETTE` set and invalid still exits 1 before any of this is consulted.
`TASKS_THEME` invalid also exits 1, even when `TASKS_PALETTE` would win — a typo is
reported, not silently ignored, matching `TASKS_COLOR`'s rule that a losing value is
still parsed.

Redirected stdout keeps its rule: it never asks the terminal. Under `default` the
palette is known without a query, so redirected output with color forced on
(`--color always`) now paints instead of warning — the same improvement the palette
variable gives, without hand-writing hex values.

## 4. Documentation and tests

- README's color section gains `TASKS_THEME` beside `TASKS_COLOR` and `TASKS_PALETTE`,
  with the precedence table above.
- Unit tests: parse accepts the two values and rejects anything else; precedence
  (`TASKS_PALETTE` over `default`); the built-in palette's mixes match the existing
  expectations for that palette (the date and priority scale tests already run on these
  exact colors, so their expectations transfer directly).
- CLI tests: `TASKS_THEME=default` paints with the built-ins; an invalid value exits 1
  naming `TASKS_THEME`. That piped-stdout test cannot see the query, so a second test
  runs the binary on a pty that is its controlling terminal and observes the stream
  itself: under `default` no OSC query bytes appear and no warning is produced, and a
  control without `TASKS_THEME` shows the query bytes and the timeout warning, proving
  the observation can see a query. The test helpers inject exactly the proposed colors
  as `TASKS_PALETTE` (src/style.rs, src/output.rs, tests/common), so a built-in test
  that leaves them set can pass with `default` unimplemented: every test of the built-in
  path removes `TASKS_PALETTE` and clears an inherited `TASKS_THEME` first, in the
  helpers themselves.
