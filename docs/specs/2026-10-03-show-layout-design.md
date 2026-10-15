# A reading layout for pretty `show` — design

**Status:** implemented (2026-10-03), tasks-606552; see docs/plans/2026-10-03-show-layout.md. Amends docs/specs/2026-09-03-color-output-design.md
§3 (`show`'s frontmatter values; the body and notes left plain),
docs/specs/2026-09-25-date-recency-color-design.md §2.3 (`show` not painted) and
docs/specs/2026-09-25-priority-color-design.md §2.1–§2.2 (`show` never queries).

## 1. Problem

`tasks show --pretty` prints the task file as it sits on disk, with seven frontmatter values
painted, followed by footers. It reads as a wall of text:

- Nothing separates the frontmatter, the body, `## Notes`, and the `# parked` footer except
  a blank line and a `#` the body may also contain.
- Dates are plain, and priority is plain unless it is P0 or P1, even though `list` colors
  both: `show` never reads the terminal theme.
- Notes run to the terminal edge and wrap mid-word. Lifecycle notes, the indented
  provenance JSON, and the authored notes all render at the same weight.
- Empty fields (`depends: []`) and file syntax (quoted titles, `[a, b]` lists) take space
  without telling the reader anything.

Each view also chooses the role for each field itself: `paint_field` in `show`, `table` in
`list`, `parked_table`, `quiet_briefs`. Nothing makes them agree.

## 2. Decision

Pretty `show` stops reproducing the file and gets a layout made for reading: sections
under ruled headings, aligned `key: value` rows for the fields, and wrapped prose. The
file is still available unchanged from `cat tasks/<id>.md` and from the JSON output,
which this design does not touch.

Both layouts were mocked on a real task and compared in the terminal, and the user chose
this one. They added two changes: the field block gets its own `overview` heading, and the
fields print one per row as `key: value` instead of being packed into a few lines.

## 3. Layout

`tasks next --pretty` renders its task the same way, because it calls the same renderer.

### 3.1 Example

Plain text, as it prints with `--color never` at a terminal 100 columns wide or wider:

```
── overview ────────────────────────────────────────────────────────────────────────────────────────
Phone scratch sheet: the Android keyboard hides the tags field and Save (mind6-160c42)
status:     doing
priority:   P1
size:       s
complexity: low
process:    direct
owner:      fix/scratch-keyboard
tags:       web-ui, capture, bug
created:    2026-09-25
updated:    2026-10-02
started:    2026-09-25
agent:      claude-code/claude-opus-5-5[1m]

── parked ──────────────────────────────────────────────────────────────────────────────────────────
waiting on user, review since 2026-09-25
    After the updated main build is deployed, check the scratch form in the Android Tailscale app
    with Gboard open: Title, Body, Tags and Save should all fit without sheet scrolling; then close
    this task if it passes.
    session codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5 in /work/mindful/v6

── description ─────────────────────────────────────────────────────────────────────────────────────
Observed on Android: with the keyboard up, the scratch sheet's tags field is half covered and Save
(and the rest of the sheet below it) needs a scroll to reach. …
- the browser ignores interactive-widget (Firefox for Android does), so dvh and bottom: 0 still
  measure the full screen: follow window.visualViewport (height/offsetTop, resize events) into a CSS
  variable;

── notes (6) ───────────────────────────────────────────────────────────────────────────────────────
2026-09-25 15:52  main
    Repro 2026-09-25: Android, Gboard, opened through the Tailscale app (Chromium-based). The
    viewport DOES resize: …
2026-09-25 19:30  fix/scratch-keyboard  started  codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5
2026-09-25 19:43  main  parked (waiting on user, review)  codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5
    After the updated main build is deployed, check the scratch form in the Android Tailscale app
    with Gboard open: Title, Body, Tags and Save should all fit without sheet scrolling; then close
    this task if it passes.
2026-10-02 08:03  main
    deployed a89de80-20261002-0402 (2026-10-02); Pixel gate can run now
```

### 3.2 Section headings

Each section opens with `── <name> ` followed by `─` up to the line width (§3.6), and the
whole heading prints in the heading role (§4.2). A section that counts something adds the
count, as in `notes (6)`. Sections are separated by one blank line. A section with nothing
to show is left out entirely, heading included. `overview` and `description` are the
exceptions and always print; an empty description prints `(no description)` in the chrome
role.

The sections, in order:

1. `overview` — §3.3.
2. `parked` — the park, when there is one. It comes before the description because it is
   the next action.
3. `escalation` — when there is one.
4. `description` — the body (§3.5).
5. `related` — the parent, the dependencies and the children (§3.4).
6. `files` — the attachments: path and size, one per line, as now.
7. `notes (N)` — in record order, oldest first (§3.5).

### 3.3 Overview

The first line is `<title> (<id>)`: the title in the heading role, the parenthesised id in
the id role. The title wraps under §3.6 with no indent.

Then one `key: value` row per field that has a value, in the order below. Keys are padded
so every value starts in the same column: the width of the longest key present, plus its
colon and one space. Unset fields have no row. Lists print comma-separated, with no
brackets or quotes.

| key | value | when shown |
|---|---|---|
| `status` | status | always |
| `priority` | `P<n>` | always |
| `size`, `complexity` | level | when set |
| `process` | `direct`, `planned`, or `unassessed` | always (unassessed is information) |
| `lane` | `yes` | when the task is a lane |
| `in lane` | lane id | when inside a lane and not one itself |
| `parallel` | `yes` | when marked |
| `needs` | names | when any |
| `owner` | owner | when set |
| `claim` | `<owner> [<session>]`, with `stale` appended when not live | when claimed |
| `tags` | tags | when any |
| `created`, `updated`, `started`, `completed` | `YYYY-MM-DD` | when set |
| `every` | interval; then `, due <date>` when a next due date exists, or `, due now` when due with no date; the interval alone otherwise (an open or dropped recurring task) | when recurring |
| `last done` | `YYYY-MM-DD` | when `last_done` is set and differs from `completed` (a reopened recurring task keeps `last_done` after `completed` is cleared) |
| `deferred` | `until <date>`, plus ` (due)` once it has passed | when deferred |
| `source`, `agent`, `model` | as stored | when set |
| `spec`, `plan` | resolved path, else the stored name | when set |
| `step` | heading, plus `  MISSING` (error role) when not found | when set |

`depends` and `parent` have no rows: the `related` section lists them with their titles.
`last_done` gets its own row only when `completed` does not already show it: completion stamps both to the same second, and a reopen clears `completed` but keeps `last_done`.

### 3.4 Related

One row per related task, with a fixed label column:
`parent`, then one `depends on` row per dependency, then one `child` row per child.
The row reads `<label>  <id>  <status>  <title>`. Labels and ids are padded so the columns
line up, and the title wraps under §3.6, indented to its own column. An unresolved
dependency shows `?` for its status and `(unresolved)` for its title, as now.
Each id and title takes the colour of the project the id belongs to, when that project
has one (tasks-68cb2b, after the tables in the comfy-tables spec §5); an unresolved
dependency and a project without a colour keep the chrome id and plain title.

### 3.5 Prose: description, park, and notes

**Description.** Each line of the body is wrapped on its own (§3.6), so the body's line
breaks are kept. A line that starts with `- `, `* `, or `<digits>. ` continues under its
text (a hanging indent), not under the marker. Lines inside a ``` fence, and lines that
start with a space or a tab, are never wrapped. A line starting with `#` prints in the
heading role. No other markdown is interpreted.

**Notes.** Each note is a header line followed by its text, indented 4 and wrapped:

- Header: `<YYYY-MM-DD> <HH:MM>  <owner>`. The date takes the date role; the time and the
  owner take the chrome role. A header wider than the line width wraps at spaces with
  continuation lines indented 2, so they cannot be mistaken for the text, which is
  indented 4; a single word wider than the line is split hard. No header is ever left to
  the terminal's own mid-word wrap.
- A note that is a generated lifecycle marker (`started`, `resumed`, `done`, `dropped`,
  `completed; next due <date>`, or `parked (waiting on …): <next step>`) puts the marker on
  the header in the marker role. For a park, the text after `: ` becomes the indented body.
  Markers are recognised by their generated text, the way other consumers recognise them.
- A note with provenance appends its `harness_session` to the header in full, in the
  chrome role, as the header's last word, so it is the first to move to a continuation
  line. `harness_session_source` is left out of the pretty output; JSON keeps both.
- A marker with no further text prints no body.

**Park.** `waiting on <describe_stop> since <date>`, the next step indented 4 and
wrapped, then `session <session> in <worktree>` indented 4 in the chrome role.
**Escalation** follows the same pattern: `needs at least <level> since <date>`, then the
session line.

### 3.6 Width

Prose wraps at the line width: the terminal width (`COLUMNS`, else the attached terminal,
as for `list`), capped at 100 columns. Long lines are harder to read, and the cap keeps a
wide terminal from producing them. With no width, because stdout is not a terminal and
`COLUMNS` is unset, nothing wraps: each body line and each note stays one line, still
indented. Heading rules are then drawn 100 columns wide. A word longer than the width is
split hard, as `list` does.

## 4. Field roles in one place

### 4.1 `present` module

A new module, `src/present.rs`, owns the mapping from field to look. It has one function
per field that returns a `Span` (the text plus its `Option<Style>`):
`id`, `title`, `status`, `priority`, `size`, `complexity`, `process`, `owner`, `tags`,
`date`, `claim`. Each caller pads the text and paints it, following the existing rule of
padding first and painting last. `table`, `parked_table`, `quiet_briefs`, `lane_lines`,
`tree_text`, the related rows, and the new `show` renderer all build their field cells
through it. After the change, `Style::Status`, `Style::Priority` and `Style::Date` are
built only in `present.rs` and `style.rs`, so a field looks the same in every view, and a
later change to one field's look (tag colors, tasks-b49ef8) is a change in one function.
The roles the fields get do not change in the table views (§4.2).

`style.rs` keeps the roles and the painter. `output.rs` loses `paint_frontmatter` and
`paint_field`; `show`'s layout moves to its own module, `src/show_text.rs`, so the
2,700-line `output.rs` does not grow.

### 4.2 Roles

| span | role |
|---|---|
| ids, owner (the field and a note's), claim, tags, agent/model/source, section rules after the name, provenance, session lines, note time | chrome (dim), as today |
| status | status hue, as today |
| priority | the priority scale, as in `list` (§5) |
| dates | the recency scale, as in `list` (§5) |
| section heading name, title, body `#` lines | **heading**: new role, bold |
| lifecycle markers | emphasis (bold), the role `list` gives cadence and deferral markers |
| `MISSING` step | error, as today |
| keys | chrome |
| size, complexity, process, prose | plain |

`heading` is a new `Style` variant painted bold, so a later change can make headings look
different from emphasis without touching the call sites. The title also takes the project
color when the project has one: in `show`, `heading` carries the optional `Rgb` and paints
it bold and colored. The `in lane`, `moved to` and `moved from` ids take their project's
colour in place of chrome, as related rows do (§3.4).

## 5. Theme and width wiring

`show` and `next` join `needs_theme` and `shows_priority`. Their dates and priorities then
paint from the terminal theme exactly as `list`'s do. They also pay the same cost: the
theme query, up to 300 ms of startup latency, when stdout is a terminal and
`TASKS_PALETTE` and `TASKS_THEME=default` are both absent. They print the same stderr
warnings when the theme is unknown. They also join the wrap-width selection in `main.rs`,
with the 100-column cap applied in the show renderer.

## 6. What stays

- JSON output is byte-identical: no field, key, or shape changes.
- `--color never`, and `--pretty` without a terminal, produce the same layout uncolored.
  `strip_ansi(colored) == plain` still holds for one width.
- The task file, `serialize_task`, and every writer are untouched.
- `show`'s exit codes, warnings and errors are unchanged, with two deliberate exceptions
  that pretty `show` and `next` now share with `list` (§5):
  - a theme the painter needs but cannot get adds the same stderr warning `list` prints;
  - an invalid `COLUMNS` (`0`, or not a positive integer) now fails a pretty `show` or
    `next` with the config error `list` gives, where it was ignored before.

  JSON output computes no width and does not query the terminal theme (it still reads and validates `TASKS_THEME` and `TASKS_PALETTE`, as today), so neither exception reaches it.

## 7. Testing

- Unit tests in `present.rs`: each function's text and role, including unset values.
- Unit tests in `show_text.rs` against a fixed `ShowFields` with a plain painter:
  - the §3.1 layout;
  - sections omitted when empty;
  - key alignment;
  - lifecycle-marker headers, including a park's next step moving to the body;
  - body wrapping: hanging indents, fenced and indented lines kept whole, `#` lines;
  - the 100-column cap; and no wrapping with no width.
- A note header test at `COLUMNS=80`: a `parked (waiting on user, review)` lifecycle note
  with provenance and a long branch-name owner wraps its header at spaces with the
  2-column continuation, and no line exceeds 80 columns.
- Overview rows for recurrence: a recurring open task (interval only), a due one with a
  date, one due now with no date, and a reopened one showing `last done`.
- `show` and `next` with `COLUMNS=0`: pretty fails with the config error, JSON succeeds
  with unchanged output.
- `tests/cli.rs`: the existing `show` painting test (`plain` vs `colored`) is rewritten
  for the new layout. It keeps the strip-ANSI invariant, checks the roles in §4.2 on
  status, priority, a date, a heading and an id, and pins `TASKS_PALETTE` so the date and
  priority colors are deterministic. The other tests that assert on pretty `show` text are
  updated to the new layout. `next --pretty` gets one test
  confirming it renders the same layout.
- The table views' existing color tests must pass unchanged, which shows the move to
  `present.rs` changed no output.

## 8. Out of scope

- Tag colors (tasks-b49ef8); this design only routes tags through `present::tags`.
- A raw or file layout under pretty: `cat` and JSON already provide it.
- Markdown beyond `#` lines, list hanging indents and fences: no emphasis, links or tables.
- Paging.
- Restyling `list`'s columns: the roles there stay as they are.
