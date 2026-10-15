# Pretty tables on comfy-table — design

**Status:** implemented (2026-10-06; accepted spec round 2), tasks-02b1ea.
Builds on the tasks-8a14be spike (branch
`spike/comfy-table`, commit c2e0e0e). Supersedes the hand-rolled wrapping of
tasks-380515 and tasks-517584 for table views; `show` keeps its own layout
(docs/specs/2026-10-03-show-layout-design.md).

## 1. Problem

Every pretty table view lays itself out by hand. `table`, `parked_table`, `quiet_briefs`,
`lane_lines`, `grid_text` and the claims rows each pad with fixed widths (`{:<18}`,
`.left(7)`, `id_width`), and only the last column can wrap (`render_row`). Each fix added
a special case: `TITLE_FLOOR`, `Wrap::indent`, `LongWord::{Split, Keep}`, a detail line
for `next:` that still breaks the table's flow. The parked queue is the visible failure:
a long next step either runs off the table or turns each row into a paragraph.

The tasks-8a14be spike rendered the parked table with comfy-table 8. Title and next step
wrapped side by side in their own columns, painted cells stayed aligned, and the width
was ours. The user adopted it.

## 2. Decision

All pretty table views render through one small module, `src/table.rs`, that describes
columns and rows and hands layout to comfy-table. Views build rows of painted cells and
say which columns may wrap; the module owns width, wrapping, the narrow fallback, piped
output and headers. The palette, the painter, the colour scales, `present`, `show` and
every JSON shape stay as they are.

Alternatives rejected:

- **comfy-table called directly from each view.** Smaller first step, but the width
  policy, the narrow fallback and the headers flag would be repeated per view, which is
  how the current warts accumulated.
- **Keep the hand-rolled renderer and add multi-column wrapping.** Cheapest dependency
  story, but it extends the code this design exists to remove.

## 3. The table module

```rust
pub enum Fit {
    Fixed,                      // never wraps; as wide as its widest cell
    Wrap { min: usize, max: Option<usize> },  // wraps inside the column
}

pub struct Column { pub header: &'static str, pub fit: Fit, pub align: Align }

pub enum Detail { Wrap(String), Whole(String) }  // a `label: text` line under a row

pub struct Row { pub cells: Vec<String>, pub details: Vec<(&'static str, Detail)> }

pub struct Layout { pub width: Option<usize>, pub headers: bool }

/// One width per column; None lets the section size that column itself.
pub struct Widths(pub Vec<Option<usize>>);
pub fn shared_widths(columns: &[Column], sections: &[&[Row]], headers: bool) -> Widths;

pub fn render(
    columns: &[Column],
    rows: &[Row],
    layout: &Layout,
    widths: Option<&Widths>,
    painter: &Painter,
) -> String;
```

- **Cells arrive painted.** comfy-table's `custom_styling` measures them by visible width.
  The module never colours anything except the header row (chrome role).
- **Spacing.** No borders (`presets::NOTHING`), two spaces between columns, none after
  the last, no trailing whitespace on any line.
- **Shared widths.** A view that prints several sections with the same columns
  (prime's closeout, roadmap, ready and doing) computes `shared_widths` over all of
  them first, and each section renders with every fixed column at that width. A `todo`
  in one section and a `doing` in another then leave the date column in one place, as
  prime's date-alignment tests require today. A section with other columns (prime's
  parked and lanes blocks) shares only the id column: its `Widths` carries the shared id
  width and its own widths elsewhere.
- **Details.** `Detail::Wrap` lines wrap under their text at the layout width;
  `Detail::Whole` lines (a worktree path) are printed whole and left to the terminal, so
  they copy intact.

### 3.1 Width

- **A width** (from `COLUMNS`, then the terminal, as today): `ContentArrangement::Dynamic`
  at that width. Fixed columns take their content width; wrap columns share the rest,
  within their `min` and `max`.
- **Narrow fallback.** When the fixed columns leave less than the sum of the wrap
  columns' `min`, a view with two wrap columns moves the last one out of the table into
  a `Detail::Wrap` line under each row (the current stacked layout). If the remaining
  wrap column still cannot get its `min`, or the view has only one, the table renders
  unwrapped and the terminal wraps it, as today below the title floor.
- **No width** (piped, redirected): `ContentArrangement::Disabled`, no column caps, every
  row one line, every detail one line. Scripts and `grep` see whole text.
- **Long words** inside a wrap column are split at the column edge. Inside a cell this
  cannot be avoided; `Detail::Whole` covers the paths that must copy.

## 4. Views

| View | Columns (wrap columns in *italics*) | Details |
|---|---|---|
| `list`, `ready`, `search`, prime's closeout/ready/doing | id, P, size, cx, process, status, [`||`], [type], date, *title + suffixes* (min 20; tags, cadence, deferral, quiet recipe, owner) | — |
| `tree`, prime's roadmap | as `list`; the id cell carries the depth indent | — |
| `list --parked`, prime's parked | id, status, phase, waits on, parked, *title* (min 20, max 50), *next* (min 40) | next, in the narrow fallback |
| `quiet` | id, P, needs, minutes, parked, *title* (min 20, max 50), *next* (min 40) | `in:` (Whole); next, in the narrow fallback |
| `lanes`, prime's lanes | id, P, *title* (min 20), *state* (min 20) | guidance, active claims (Wrap), `lanes` only; state, in the narrow fallback |
| `projects` | as today; numbers right-aligned | — |
| `claims` | id, session, live, worktree | — |

`list --parked` drops the process column (it stays in `show` and the JSON); the spike
showed the queue reads better with the width given to next. The wait column keeps its
full text (`user, review`).

## 5. Colour

- **Ids and titles take the project colour** in every table that renders task rows,
  where the project has one. This closes tasks-9b5b0a. Rows without a project colour
  keep today's roles.
- **One place fills the colour.** Today `TaskSummary::of` leaves `project_color` unset
  and only `list` and `search` fill it afterwards (`commands/list.rs`). A `ReadCtx`
  helper resolves a prefix's colour from the projects in scope, and every command that
  renders task rows applies it: `list`, `search`, `ready`, `tree`, `prime` (every
  section, the roadmap tree included), `lanes` (whose `lane`, `pick` and `active` rows
  are already `TaskSummary` values, so they need no new field) and the parked rows.
  `ParkedRow` gains the one new serde-skipped `project_color` field.
- **Outside tables** (tasks-68cb2b): `show` and `next` paint related rows, the lane and
  move pointers by each id's project; `groups` and `where` paint prefixes as `projects`
  does. The colours stay out of JSON.
- **`claims` stays uncoloured.** It reads the claim stores without opening any checkout,
  so a missing checkout changes nothing (`commands/claims.rs`); colours live in checkout
  configuration, and reading them would weaken that contract.
- The colour is the project's `color` key in `tasks/.config.toml`, its only source.
  When this was designed no project set one; since 2026-10-15 every registered project
  does (ops-40f2a6). tasks-8203e6 had proposed taking colours from familiar instead and
  closed without that: familiar's identity slots are shared between projects and publish
  no hex (ops-b31634).
- Status, priority, date recency and the other roles render as today. The spike's
  missing colours came from unpainted title cells and are covered by the first bullet.

## 6. Headers

- A global `--headers` option shows a header row on every table view, in the chrome
  role. Like `--color`, it is accepted with JSON and changes nothing there.
- `projects` keeps its header row always, as today; its numbers mean nothing without
  labels.
- The option lands in ops `cli.toml` first and is adopted with `vendored adopt`. A config
  default is tasks-0e366f, which waits on ops-9122f2's cross-project config convention.

## 7. Warnings

In pretty mode the warnings follow the output under a rule in `show`'s heading style,
`── warnings (5) ─────`, drawn to the layout width, one warning per line without the
`warning:` prefix the heading now carries. `show`'s `rule` moves where both can use it.
With no warnings nothing is printed. JSON warnings are unchanged.

## 8. What goes

`render_row`, `render_wrapped`, `LongWord`, `wrap_lines_with`,
`TITLE_FLOOR`, `Wrap::indent`, `grid_text` and its `Cell`/`Align`, `detail_line`, and the
fixed paddings in each view. `wrap_lines`, `fill_hard_split`, `paint_runs` and
`columns` stay, moved into `show_text.rs`, the one non-table user. `Span::left` goes if nothing else pads.

## 9. Dependency

`comfy-table = { version = "8", features = ["custom_styling"] }`: 20 crates (crossterm,
console, ansi-str and its parser; Windows shims never build here), +215 KB on the
release binary (+3.8%), +7 s on a clean release build (tasks-8a14be).

## 10. Testing

- **Module unit tests:** fixed and wrap columns at several widths; min/max honoured; the
  narrow fallback; no width gives one line per row and no caps; headers on and off; a
  shared id width; painted cells keep alignment (`ColorMode::Always`, ANSI stripped);
  no trailing whitespace; `Detail::Whole` never split.
- **View tests:** each view's columns and order; parked next beside its title; quiet's
  `in:` whole; prime's sections share the id column; tree indent.
- **CLI tests:** `--headers` on each table command and its no-op under JSON; the
  warnings rule after the output and on the early-exit paths (tasks-517584's test kept);
  with a configured project colour, painted ids and titles in `list`, `search`, `ready`,
  `tree`, every `prime` section, `lanes` and `list --parked`, and none in `claims`;
  prime's existing date-alignment tests kept.
- **Width:** on a realistic fixture at 80, 120 and 236 columns, every line fits within
  `COLUMNS` wherever wrapping is feasible (the wrap columns reach their `min`). Where the
  design permits overflow, the tests assert the text is intact instead: the unwrapped
  narrow fallback (each row on one line, nothing split), `Detail::Whole` paths (whole,
  on their own line), and fixed-only tables wider than the width.
- Existing tests that pin today's exact padding change with their view; JSON tests do
  not change.

## 11. Out of scope

- Where project colours come from (settled by tasks-8203e6: the `color` key) and a
  config file for defaults (tasks-0e366f).
- `show`, `next` and `graph` output, which are not tables.
- Truncating cells: nothing is cut; narrow terminals fall back as in §3.1.
