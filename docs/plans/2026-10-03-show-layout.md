# Reading layout for pretty `show` implementation plan

**Status:** draft (2026-10-03), tasks-606552.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tasks show --pretty` and `tasks next --pretty` print a layout made for reading:
ruled sections, aligned `key: value` overview rows, wrapped prose, and the same field colors
as `list`. Every view takes each field's look from one module.

**Architecture:** A new `src/present.rs` returns each field's text and role as a `Span`.
Every table view and the new renderer build field cells through it. A new
`src/show_text.rs` holds the layout and reuses `output.rs`'s styled word wrap. `main.rs`
gives `show` and `next` the terminal theme and a wrap width, as `list` already gets.

**Tech Stack:** Rust (edition 2024), clap 4, `unicode-width`. Unit tests sit beside the
code; integration tests are in `tests/cli.rs`, using `tests/common/mod.rs`, whose
`TestEnv::cmd` pins `TASKS_PALETTE` to `TEST_PALETTE`.

**Spec:** `docs/specs/2026-10-03-show-layout-design.md` (approved after review round 2).

## Global Constraints

- Work in `.worktrees/show-style` (branch `feat/show-style`). Run tests only through
  `just test-one <filter>` and `just test-fast`; never `cargo test` directly. Run
  `just check` before each commit.
- JSON output is byte-identical. New struct fields are `#[serde(skip)]`.
- Prose wraps at the terminal width capped at `MEASURE = 100`. With no width, nothing
  wraps and rules are 100 columns wide.
- Pad first, paint last. `Painter::paint` never receives a newline.
- After Task 1, `Style::Status(`, `Style::Priority(`, `Style::Date(` and `Style::Project(`
  are built only in `src/present.rs` and `src/style.rs` (spec §4.1).
- Table views (`list`, `ready`, `tree`, `prime`, `parked`, `quiet`, `lanes`, `projects`)
  keep their exact output: their existing tests must pass unchanged.
- Conventional commits, no AI attribution. Each commit that changes `tasks/` must pass
  `tasks check`.

## Review Focus

1. **Unterminated ``` fence in a body.** Everything after it prints verbatim and nothing
   panics. Test: `an_unterminated_fence_keeps_the_rest_verbatim`, Task 2.
2. **An authored note that starts like a marker** ("started the refactor", "parked the
   car"). It must stay authored text; only exact generated forms count as markers. Test:
   `authored_notes_that_resemble_markers_stay_text`, Task 2.
3. **A very narrow terminal** (`COLUMNS=10`). The output must not panic or lose words,
   though lines may exceed the width when the key column alone is wider. Test:
   `a_tiny_width_never_panics_or_drops_words`, Task 2.
4. **An empty task** (no body, no notes, no relations). Only the overview and description
   sections print, and the description reads `(no description)`. Test:
   `an_empty_task_prints_overview_and_an_empty_description`, Task 2.
5. **Piped output** (no terminal, `COLUMNS` unset). Long notes stay on one line and rules
   are 100 columns wide. Test: `without_a_width_nothing_wraps`, Task 2, and
   `piped_pretty_show_keeps_notes_on_one_line`, Task 3.

---

### Task 1: the `present` module

Moves every field's role into one module. No visible output changes. The heading role
waits for Task 2, its first caller: an unused item fails `just check` (`dead_code` is
denied through clippy's `-D warnings`).

**Files:**
- Create: `src/present.rs`
- Modify: `src/main.rs` (add `mod present;` after `mod palette;`)
- Modify: `src/output.rs` (`count_column`, the `projects` grid, `lane_lines`, `table`,
  `parked_table`, `quiet_briefs`; delete `date_role`)

**Interfaces:**
- Produces:
  - `present::Span { pub text: String, pub style: Option<Style> }` with
    `Span::plain(impl Into<String>) -> Span`, `Span::left(self, width: usize) -> Span`,
    and `Span::paint(&self, &Painter) -> String`.
  - Field functions:
    - `present::id(&str)`, `title(&str, Option<Rgb>)`, `project(&str, Option<Rgb>)`
    - `status(Status)`, `status_of(Option<Status>)`, `priority(u8)`
    - `size(Option<Size>)`, `complexity(Option<Complexity>)`,
      `process(Option<Process>, unset: &str)`
    - `owner(&str)`, `tags(&[String])`, `date(&str)`, `today()`, `claim(&ClaimInfo)`,
      `count(Status, usize)`

    All return `Span`.

- [ ] **Step 1: Write `src/present.rs` with its tests**

```rust
//! The look of each task field, decided once (show layout spec §4). Views ask this module
//! for a field's text and role, then pad and paint the result; none of them names a
//! field's role itself, so a field reads the same in every view.

use crate::model::{Complexity, Process, Size, Status};
use crate::output::ClaimInfo;
use crate::palette::Rgb;
use crate::style::{Painter, Style, When};

/// A field's text and the role it paints in. Pad the text first, paint last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Option<Style>,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            style: None,
        }
    }

    fn styled(text: impl Into<String>, style: Style) -> Span {
        Span {
            text: text.into(),
            style: Some(style),
        }
    }

    /// Left-aligned to `width` visible columns, before painting.
    pub fn left(self, width: usize) -> Span {
        Span {
            text: format!("{:<width$}", self.text),
            ..self
        }
    }

    pub fn paint(&self, painter: &Painter) -> String {
        match self.style {
            Some(style) => painter.paint(style, &self.text),
            None => self.text.clone(),
        }
    }
}

/// A task id, wherever it appears.
pub fn id(id: &str) -> Span {
    Span::styled(id, Style::Chrome)
}

/// A title in a row: the project's color when it has one, else plain.
pub fn title(title: &str, project: Option<Rgb>) -> Span {
    Span {
        text: title.into(),
        style: project.map(Style::Project),
    }
}

/// A project prefix: its color when assigned, else dim.
pub fn project(prefix: &str, color: Option<Rgb>) -> Span {
    Span::styled(prefix, color.map(Style::Project).unwrap_or(Style::Chrome))
}

pub fn status(status: Status) -> Span {
    Span::styled(status.as_str(), Style::Status(status))
}

/// A status that may be unknown: an unresolved dependency, or a park whose record is gone.
pub fn status_of(status: Option<Status>) -> Span {
    match status {
        Some(known) => self::status(known),
        None => Span::styled("?", Style::Chrome),
    }
}

pub fn priority(priority: u8) -> Span {
    Span::styled(format!("P{priority}"), Style::Priority(priority))
}

pub fn size(size: Option<Size>) -> Span {
    Span::plain(size.map(Size::as_str).unwrap_or("-"))
}

pub fn complexity(complexity: Option<Complexity>) -> Span {
    Span::plain(complexity.map(Complexity::as_str).unwrap_or("-"))
}

/// `unset` is what an unassessed task shows: `-` in a column, `unassessed` in `show`.
pub fn process(process: Option<Process>, unset: &str) -> Span {
    Span::plain(process.map(Process::as_str).unwrap_or(unset))
}

pub fn owner(owner: &str) -> Span {
    Span::styled(owner, Style::Chrome)
}

/// Tags comma-separated, without brackets; a caller that wants brackets adds them.
pub fn tags(tags: &[String]) -> Span {
    Span::styled(tags.join(", "), Style::Chrome)
}

/// A validated timestamp or `YYYY-MM-DD` day, shown as the day and painted by its age.
pub fn date(timestamp: &str) -> Span {
    let day = crate::time::day(timestamp);
    let date = crate::time::calendar_day(day)
        .expect("records and park entries carry validated dates");
    Span::styled(day, Style::Date(When::On(date)))
}

/// The due column's `now`, which names no date.
pub fn today() -> Span {
    Span::styled("now", Style::Date(When::Today))
}

/// `<owner> [<session>]`, with ` stale` inside the brackets when the claim is not live.
pub fn claim(claim: &ClaimInfo) -> Span {
    let stale = if claim.live { "" } else { " stale" };
    Span::styled(
        format!("{} [{}{stale}]", claim.owner, claim.session),
        Style::Chrome,
    )
}

/// A status count, dimmed at zero whatever its status: a red `0` under `blocked` reads
/// as an alarm.
pub fn count(status: Status, value: usize) -> Span {
    let style = if value == 0 {
        Style::Chrome
    } else {
        Style::Status(status)
    };
    Span::styled(value.to_string(), style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_carry_their_roles() {
        assert_eq!(id("xx-000001").style, Some(Style::Chrome));
        assert_eq!(owner("me").style, Some(Style::Chrome));
        assert_eq!(status(Status::Doing).style, Some(Style::Status(Status::Doing)));
        assert_eq!(status_of(None), Span::styled("?", Style::Chrome));
        assert_eq!(priority(1), Span::styled("P1", Style::Priority(1)));
        assert_eq!(size(None), Span::plain("-"));
        assert_eq!(complexity(Some(Complexity::Low)), Span::plain("low"));
        assert_eq!(process(None, "unassessed"), Span::plain("unassessed"));
        assert_eq!(process(Some(Process::Direct), "-"), Span::plain("direct"));
        assert_eq!(
            tags(&["a".into(), "b".into()]),
            Span::styled("a, b", Style::Chrome)
        );
        assert_eq!(today(), Span::styled("now", Style::Date(When::Today)));
        assert_eq!(title("T", None), Span::plain("T"));
        assert_eq!(project("xx", None).style, Some(Style::Chrome));
        assert_eq!(count(Status::Blocked, 0).style, Some(Style::Chrome));
        assert_eq!(
            count(Status::Blocked, 2).style,
            Some(Style::Status(Status::Blocked))
        );
    }

    #[test]
    fn a_date_shows_the_day_and_paints_by_it() {
        let span = date("2026-09-25T15:47:16Z");
        assert_eq!(span.text, "2026-09-25");
        let day = ::time::Date::from_calendar_date(2026, ::time::Month::September, 25).unwrap();
        assert_eq!(span.style, Some(Style::Date(When::On(day))));
        assert_eq!(date("2026-09-25").text, "2026-09-25");
    }

    #[test]
    fn left_pads_before_painting() {
        assert_eq!(id("ab").left(4).text, "ab  ");
    }

    /// Spec §4.1: the views ask this module, so a field cannot look different in two
    /// of them. Test code is exempt.
    #[test]
    fn field_roles_are_built_only_here() {
        for (file, text) in [("output.rs", include_str!("output.rs"))] {
            let code = text.split("#[cfg(test)]").next().unwrap();
            for role in [
                "Style::Status(",
                "Style::Priority(",
                "Style::Date(",
                "Style::Project(",
            ] {
                assert!(
                    !code.contains(role),
                    "{file} builds {role} itself; ask present instead"
                );
            }
        }
    }
}
```

Add `mod present;` to `src/main.rs` after `mod palette;`.

- [ ] **Step 2: Run the present tests to verify the guard fails**

Run: `just test-one present::`
Expected: `fields_carry_their_roles`, `a_date_shows_the_day_and_paints_by_it` and
`left_pads_before_painting` PASS. `field_roles_are_built_only_here` FAILS, because
`output.rs` still builds `Style::Status(` and the other roles.

- [ ] **Step 3: Route `output.rs`'s field cells through `present`**

In `src/output.rs`:

1. Add `use crate::present;` to the imports.
2. In `count_column`, set the column's style from the span:

```rust
fn count_column(label: &'static str, value: usize, status: Status) -> CountColumn {
    CountColumn {
        label,
        value,
        style: present::count(status, value).style,
    }
}
```

   Drop its doc comment's first sentence ("A zero is dimmed…"), which now lives on
   `present::count`.
3. In the `Output::Projects` grid, build the prefix cell and the activity date through
   `present`:

```rust
                let prefix = present::project(&row.prefix, row.color);
                let mut cells = vec![cell(prefix.text, Align::Left, prefix.style)];
```

```rust
                    (Some(_), Some(at)) => {
                        let at = present::date(at);
                        cell(at.text, Align::Left, at.style)
                    }
```

4. In `lane_lines`, build the id and priority through `present`:

```rust
        let id = present::id(&row.lane.id).left(id_width).paint(painter); // `TaskSummary.id` is a `String`
        let priority = present::priority(row.lane.priority).paint(painter);
```

5. In `table`, replace the date match and the field cells:

```rust
        let date = match date {
            DateColumn::Updated => Some(present::date(&row.updated)),
            DateColumn::Created => Some(present::date(&row.created)),
            DateColumn::Due => match (&row.periodic, &row.deferred) {
                (Some(periodic), _) => match (&periodic.due, periodic.due_now) {
                    (Some(due), _) => Some(present::date(due)),
                    (None, true) => Some(present::today()),
                    (None, false) => None,
                },
                (None, Some(deferred)) => Some(present::date(&deferred.until)),
                (None, None) => None,
            },
        };
        let date = date
            .map(|span| span.paint(painter))
            .unwrap_or_else(|| "-".into());
        let id = present::id(&row.id).left(id_width).paint(painter);
        let priority = present::priority(row.priority).paint(painter);
        let size = present::size(row.size).left(2).paint(painter);
        let complexity = present::complexity(row.complexity).left(4).paint(painter);
        let process = present::process(row.process, "-").left(7).paint(painter);
        let status = present::status(row.status).left(7).paint(painter);
        let tags = if row.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", present::tags(&row.tags).text)
        };
```

   Every field keeps the style its span carries: pad with `left`, then `paint`, never take
   `.text` alone for a field. The three columns are padded already, so drop their width
   specifiers from the `prefix` format string:

```rust
        let prefix = format!(
            "{id}  {priority} {size} {complexity} {process} {status} {mark}{kind}{date}  "
        );
```

   Replace the `owner` match with a span, so the owner piece takes its style from
   `present` too:

```rust
        let owner = match &row.claim {
            Some(claim) => Some(present::claim(claim)),
            None => row.owner.as_deref().map(present::owner),
        };
        let owner_text = owner
            .as_ref()
            .map(|span| format!(" @{}", span.text))
            .unwrap_or_default();
```

   In the `render_row` pieces, replace the title and tags entries:

```rust
                (row.title.as_str(), present::title(&row.title, row.project_color).style),
                (tags.as_str(), present::tags(&row.tags).style),
```

   and the owner entry:

```rust
                (owner_text.as_str(), owner.as_ref().and_then(|span| span.style)),
```

   Leave the cadence, deferral and quiet entries as they are: emphasis is not a field
   role.
6. In `parked_table`:

```rust
        let id = present::id(&row.id).left(id_width).paint(painter);
        let status = present::status_of(row.status).left(7).paint(painter);
        let process = present::process(row.process, "-").left(7).paint(painter);
        let parked = present::date(&park.at).paint(painter);
```

7. In `quiet_briefs`:

```rust
        let id = present::id(&row.id).left(id_width).paint(painter);
        let priority = match row.priority {
            Some(priority) => present::priority(priority).paint(painter),
            None => "P-".into(),
        };
```

   and `let parked = present::date(&park.at).paint(painter);`.
8. Delete `fn date_role` and its doc comment.
9. Leave `paint_field` and `show_text` as they are: Task 2 deletes both. In the meantime,
   change their three role constructions so the guard passes:
   - `"status" => Style::Status(task.status)` becomes
     `"status" => present::status(task.status).style.expect("a status has a role")`.
   - `"priority" => Style::Priority(task.priority)` becomes
     `"priority" => present::priority(task.priority).style.expect("a priority has a role")`.
   - In `show_text`'s `related_row`, `painter.paint(Style::Status(status), status.as_str())`
     becomes `present::status(status).paint(painter)`.

   In `parked_table`'s `prefix` format string, change `{process:<7}` to `{process}`; the
   span is padded already.

`status_of(None)` now paints `?` as chrome in `parked_table`, which already did that.
`present::claim` builds the same text the old `owner` match did: `" @owner [session]"`, or
`" @owner [session stale]"`.

- [ ] **Step 4: Run the guard and the fast suite**

Run: `just test-one present::`
Expected: all four PASS.

Run: `just test-fast`
Expected: PASS with no test edits. Any failure means a table's output changed: fix the
call site, not the test.

- [ ] **Step 5: Commit**

```bash
just check
git add src/present.rs src/main.rs src/output.rs
git commit -m "refactor(output): take every field's role from one present module"
```

---

### Task 2: the `show` layout

Adds the heading role, replaces the file-shaped `show` text with the spec §3 layout, and
updates the integration tests that assert the old text.

**Files:**
- Modify: `src/style.rs` (`Style::Heading`, `Painter::paint`, tests)
- Create: `src/show_text.rs`
- Modify: `src/main.rs` (add `mod show_text;` after `mod shorthand;`)
- Modify: `src/output.rs`:
  - add `ShowFields.project_color` and `Wrap::width`;
  - make `paint_runs` and `wrap_lines` `pub(crate)`;
  - route `Show`/`Next` in `pretty` to the new renderer;
  - delete `paint_frontmatter`, `paint_field` and `show_text`.
- Modify: `src/commands/show.rs` (set `project_color` in `describe`)
- Modify: `src/present.rs` (`heading`; the guard covers `show_text.rs`)
- Test: `src/show_text.rs` (unit), `src/style.rs` (unit), `tests/cli.rs` and
  `tests/attachments.rs` (existing pretty `show` and `next` assertions)

**Interfaces:**
- Consumes: `present::*` and `Span` (Task 1); `output::wrap_lines(&[(char, Option<Style>)], usize) -> Vec<Vec<(char, Option<Style>)>>`
  and `output::paint_runs(&[(char, Option<Style>)], &Painter) -> String`, both made
  `pub(crate)` here.
- Produces:
  - `Style::Heading(Option<Rgb>)`: bold, or bold in the color when one is given; and
    `present::heading(&str, Option<Rgb>) -> Span`.
  - `show_text::render(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String`
  - `show_text::MEASURE: usize = 100`
  - `ShowFields.project_color: Option<Rgb>` (`#[serde(skip)]`)
  - `Wrap::width(&self) -> Option<usize>`

- [ ] **Step 1: Write the failing style test**

In `src/style.rs`'s `painter_obeys_format_mode_stream_and_roles`, after the
`(Style::Warning, "33"),` entry of the role table, add:

```rust
            (Style::Heading(None), "1"),
```

And after that loop, add:

```rust
        assert_eq!(
            colored.paint(
                Style::Heading(Some(Rgb {
                    r: 1,
                    g: 2,
                    b: 3
                })),
                "x"
            ),
            "\x1b[1;38;2;1;2;3mx\x1b[0m"
        );
```

`Rgb` is imported further down in that test module (`use crate::palette::{Palette, Rgb};`).
Move that `use` to the top of `mod tests` if the compiler needs it earlier.

- [ ] **Step 2: Run it to verify it fails**

Run: `just test-one painter_obeys_format_mode_stream_and_roles`
Expected: compile error, `no variant named Heading`.

- [ ] **Step 3: Add the role**

In `src/style.rs`, add a variant to `pub enum Style` after `Priority(u8)`:

```rust
    /// A heading: a `show` section name, a task title shown as a heading, a body `#` line.
    /// Bold; in the project's color when it has one (show layout spec §4.2).
    Heading(Option<Rgb>),
```

In `Painter::paint`'s `match style`, before `Style::Status(Status::Idea)`, add:

```rust
            Style::Heading(None) => "1".into(),
            Style::Heading(Some(color)) => format!("1;38;2;{};{};{}", color.r, color.g, color.b),
```

In `src/present.rs`, after `title`, add:

```rust
/// Text shown as a heading: bold, in the project's color when it has one.
pub fn heading(text: &str, project: Option<Rgb>) -> Span {
    Span::styled(text, Style::Heading(project))
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `just test-one painter_obeys_format_mode_stream_and_roles`
Expected: PASS. (`just check` would still flag `heading` as unused here; its callers
arrive in Step 8.)

- [ ] **Step 5: Expose the wrap helpers and add the `ShowFields` color**

In `src/output.rs`:
- Change `fn paint_runs` to `pub(crate) fn paint_runs` and `fn wrap_lines` to
  `pub(crate) fn wrap_lines`.
- In `impl Wrap`, add:

```rust
    /// The width rows wrap at; `None` keeps every row on one line.
    pub fn width(&self) -> Option<usize> {
        self.width
    }
```

- In `pub struct ShowFields`, after `pub task: Task,`, add:

```rust
    /// Pretty-only: the title's color in `show`'s overview, from the record's project.
    #[serde(skip)]
    pub project_color: Option<crate::palette::Rgb>,
```

In `src/commands/show.rs`'s `describe`, add `project_color: project.color,` to the
`ShowFields { … }` literal.

- [ ] **Step 6: Write `src/show_text.rs` with failing tests**

Create the file with the tests first and a stub `render` that returns `String::new()`:

```rust
//! The pretty `show` layout (docs/specs/2026-10-03-show-layout-design.md): ruled
//! sections, aligned overview rows, and prose wrapped at a capped measure. `next`
//! prints its task through the same renderer.

use crate::claims::Escalation;
use crate::model::Note;
use crate::output::{ParkInfo, ShowFields, paint_runs, wrap_lines};
use crate::present::{self, Span};
use crate::style::{Painter, Style};

/// Prose never runs wider than this, however wide the terminal (spec §3.6).
pub const MEASURE: usize = 100;

pub fn render(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Complexity, HarnessProvenance, Process, Size, Status};
    use crate::output::{DepInfo, ParkInfo, Related};
    use crate::style::ColorMode;

    const BARE: &str = "---\nid: sci-000001\ntitle: Phone sheet hides Save\nstatus: doing\npriority: 1\ncreated: 2026-09-25T15:47:16Z\nupdated: 2026-10-02T08:03:41Z\ndepends: []\ntags: []\n---\n";

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, crate::output::Format::Pretty, false)
    }

    fn fields() -> ShowFields {
        ShowFields {
            task: crate::format::parse_task(BARE, "x").unwrap(),
            project_color: None,
            spec_path: None,
            plan_path: None,
            step_found: None,
            depends_on: vec![],
            parent: None,
            in_lane: None,
            children: vec![],
            claim: None,
            park: None,
            escalation: None,
            periodic: None,
            deferred: None,
            files: vec![],
        }
    }

    fn note(at: &str, by: &str, text: &str, session: Option<&str>) -> Note {
        Note {
            at: at.into(),
            by: by.into(),
            text: text.into(),
            provenance: session.map(|session| HarnessProvenance {
                harness_session: session.into(),
                harness_session_source: "CODEX_SESSION_ID".into(),
            }),
        }
    }

    /// The §3.1 example's task, trimmed.
    fn example() -> ShowFields {
        let mut o = fields();
        let t = &mut o.task;
        t.size = Some(Size::S);
        t.complexity = Some(Complexity::Low);
        t.process = Some(Process::Direct);
        t.owner = Some("fix/sheet".into());
        t.started = Some("2026-09-25T19:30:15Z".into());
        t.tags = vec!["web-ui".into(), "bug".into()];
        t.agent = Some("claude-code/claude-opus-5-5".into());
        t.body = "Observed on Android.\n- the browser ignores it".into();
        t.notes = vec![
            note("2026-09-25T15:52:18Z", "main", "Repro on Gboard.", None),
            note("2026-09-25T19:30:15Z", "fix/sheet", "started", Some("codex:abc")),
            note(
                "2026-09-25T19:43:04Z",
                "main",
                "parked (waiting on user, review): Check it on the phone.",
                Some("codex:abc"),
            ),
        ];
        o
    }

    /// The value of `key`'s overview row, whatever the key column's width.
    fn value(text: &str, key: &str) -> Option<String> {
        let prefix = format!("{key}:");
        text.lines()
            .find_map(|line| line.strip_prefix(&prefix).map(|rest| rest.trim().to_string()))
    }

    fn rule(name: &str) -> String {
        let head = format!("── {name} ");
        format!("{head}{}\n", "─".repeat(MEASURE - head.chars().count()))
    }

    #[test]
    fn the_example_renders_as_the_spec_shows() {
        let expected = [
            rule("overview"),
            "Phone sheet hides Save (sci-000001)\n".into(),
            "status:     doing\n".into(),
            "priority:   P1\n".into(),
            "size:       s\n".into(),
            "complexity: low\n".into(),
            "process:    direct\n".into(),
            "owner:      fix/sheet\n".into(),
            "tags:       web-ui, bug\n".into(),
            "created:    2026-09-25\n".into(),
            "updated:    2026-10-02\n".into(),
            "started:    2026-09-25\n".into(),
            "agent:      claude-code/claude-opus-5-5\n".into(),
            "\n".into(),
            rule("description"),
            "Observed on Android.\n".into(),
            "- the browser ignores it\n".into(),
            "\n".into(),
            rule("notes (3)"),
            "2026-09-25 15:52  main\n".into(),
            "    Repro on Gboard.\n".into(),
            "2026-09-25 19:30  fix/sheet  started  codex:abc\n".into(),
            "2026-09-25 19:43  main  parked (waiting on user, review)  codex:abc\n".into(),
            "    Check it on the phone.\n".into(),
        ]
        .concat();
        assert_eq!(render(&example(), &plain(), Some(120)), expected);
    }

    #[test]
    fn an_empty_task_prints_overview_and_an_empty_description() {
        let text = render(&fields(), &plain(), Some(80));
        assert_eq!(value(&text, "process").as_deref(), Some("unassessed"), "{text}");
        assert!(text.ends_with("(no description)\n"), "{text}");
        for absent in ["── parked", "── related", "── files", "── notes", "size:"] {
            assert!(!text.contains(absent), "{absent}: {text}");
        }
    }

    #[test]
    fn sections_follow_the_spec_order() {
        let mut o = example();
        o.park = Some(ParkInfo {
            at: "2026-09-25T19:43:04Z".into(),
            next_step: "Check it on the phone.".into(),
            waiting_on: crate::claims::WaitingOn::User,
            reason: Some(crate::claims::Reason::Review),
            needs: None,
            minutes: None,
            session: "codex:abc".into(),
            owner: "main".into(),
            host: "h".into(),
            worktree: "/w".into(),
        });
        o.parent = Some(Related {
            id: "sci-000002".into(),
            title: "Goal".into(),
            status: Status::Todo,
        });
        o.depends_on = vec![DepInfo {
            id: "fam-000003".into(),
            title: None,
            status: None,
            resolved: false,
        }];
        let text = render(&o, &plain(), Some(120));
        let order: Vec<usize> = ["── overview", "── parked", "── description", "── related", "── notes"]
            .iter()
            .map(|name| text.find(name).unwrap_or_else(|| panic!("{name}: {text}")))
            .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert!(
            text.contains("waiting on user, review since 2026-09-25\n    Check it on the phone.\n    session codex:abc in /w\n"),
            "{text}"
        );
        assert!(text.contains("parent      sci-000002  todo     Goal\n"), "{text}");
        assert!(text.contains("depends on  fam-000003  ?        (unresolved)\n"), "{text}");
    }

    #[test]
    fn recurrence_rows_cover_every_state() {
        let row = |every: &str, due: Option<&str>, due_now: bool, last: Option<&str>, completed: Option<&str>| {
            let mut o = fields();
            o.task.last_done = last.map(Into::into);
            o.task.completed = completed.map(Into::into);
            o.periodic = Some(crate::output::PeriodicInfo {
                every: every.into(),
                last_done: last.map(Into::into),
                due: due.map(Into::into),
                due_now,
            });
            render(&o, &plain(), Some(120))
        };
        let every = |text: String| value(&text, "every");
        assert_eq!(every(row("30d", None, false, None, None)).as_deref(), Some("30d"));
        assert_eq!(
            every(row("2w", Some("2026-10-10T00:00:00Z"), false, None, None)).as_deref(),
            Some("2w, due 2026-10-10")
        );
        assert_eq!(
            every(row("30d", None, true, None, None)).as_deref(),
            Some("30d, due now")
        );
        let reopened = row("30d", None, false, Some("2026-09-01T00:00:00Z"), None);
        assert_eq!(value(&reopened, "last done").as_deref(), Some("2026-09-01"), "{reopened}");
        let closed = row(
            "30d",
            Some("2026-10-01T00:00:00Z"),
            false,
            Some("2026-09-01T00:00:00Z"),
            Some("2026-09-01T00:00:00Z"),
        );
        assert!(!closed.contains("last done"), "{closed}");
    }

    #[test]
    fn a_missing_step_is_flagged() {
        let mut o = fields();
        o.task.step = Some("Task 3: emit".into());
        o.step_found = Some(false);
        let text = render(&o, &plain(), Some(120));
        assert_eq!(value(&text, "step").as_deref(), Some("Task 3: emit  MISSING"), "{text}");
    }

    #[test]
    fn body_lines_wrap_with_hanging_indents_and_keep_code_whole() {
        let mut o = fields();
        let long = "word ".repeat(30);
        o.task.body = format!(
            "# Heading\n- {long}\n12. {long}\n    indented {long}\n```\n{long}\n```\n{long}"
        );
        let text = render(&o, &plain(), Some(40));
        let body: Vec<&str> = text
            .split_once("── description")
            .unwrap()
            .1
            .lines()
            .skip(1)
            .collect();
        assert_eq!(body[0], "# Heading");
        assert!(body[1].starts_with("- word"));
        assert!(body[2].starts_with("  word"), "bullet hangs: {:?}", body[2]);
        let numbered = body.iter().position(|line| line.starts_with("12. ")).unwrap();
        assert!(body[numbered + 1].starts_with("    word"), "{:?}", body[numbered + 1]);
        assert!(body.contains(&format!("    indented {long}").as_str()), "indented kept whole");
        assert!(body.contains(&long.as_str()), "fenced kept whole");
        for line in &body {
            if !line.starts_with("    indented") && *line != long {
                assert!(line.chars().count() <= 40, "{line:?}");
            }
        }
    }

    #[test]
    fn an_unterminated_fence_keeps_the_rest_verbatim() {
        let mut o = fields();
        let long = "word ".repeat(30);
        o.task.body = format!("```\n{long}");
        let text = render(&o, &plain(), Some(40));
        assert!(text.contains(&format!("\n{long}\n")), "{text}");
    }

    #[test]
    fn authored_notes_that_resemble_markers_stay_text() {
        let mut o = fields();
        o.task.notes = vec![
            note("2026-09-25T15:52:18Z", "main", "started the refactor", None),
            note("2026-09-25T15:53:18Z", "main", "parked the car", None),
        ];
        let text = render(&o, &plain(), Some(120));
        assert!(text.contains("2026-09-25 15:52  main\n    started the refactor\n"), "{text}");
        assert!(text.contains("2026-09-25 15:53  main\n    parked the car\n"), "{text}");
    }

    #[test]
    fn generated_markers_are_recognised_in_every_form() {
        for (text, marker, rest) in [
            ("resumed", "resumed", None),
            ("completed; next due 2026-11-02", "completed; next due 2026-11-02", None),
            ("parked (waiting on agent): go on", "parked (waiting on agent)", Some("go on")),
            (
                "parked (waiting on user, quiet; idle, 50 min): run it",
                "parked (waiting on user, quiet; idle, 50 min)",
                Some("run it"),
            ),
        ] {
            assert_eq!(lifecycle(text), (Some(marker), rest), "{text}");
        }
    }

    #[test]
    fn marker_prefixes_on_authored_text_are_not_markers() {
        for text in [
            "completed; next due date is undecided",
            "completed; next due 2026-13-40",
            "parked (waiting on the vendor): call them",
            "parked (waiting on user, someday): later",
            "parked (waiting on user; 5 min, idle): wrong order",
            "parked (waiting on user)",
        ] {
            assert_eq!(lifecycle(text), (None, Some(text)), "{text}");
        }
    }

    #[test]
    fn a_long_note_header_wraps_with_a_two_column_continuation() {
        let mut o = fields();
        o.task.notes = vec![note(
            "2026-09-25T19:43:04Z",
            "feat/a-rather-long-branch-name-for-testing",
            "parked (waiting on user, review): Check it.",
            Some("codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5"),
        )];
        let text = render(&o, &plain(), Some(80));
        let notes = text.split_once("── notes (1)").unwrap().1;
        let lines: Vec<&str> = notes.lines().skip(1).collect();
        assert!(lines[0].starts_with("2026-09-25 19:43  feat/a-rather-long"), "{notes}");
        assert!(lines[1].starts_with("  ") && !lines[1].starts_with("   "), "{notes}");
        assert!(notes.contains("codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5"), "{notes}");
        assert!(notes.contains("\n    Check it.\n"), "{notes}");
        for line in text.lines() {
            assert!(line.chars().count() <= 80, "{line:?}");
        }
    }

    #[test]
    fn a_wide_terminal_is_capped_at_the_measure() {
        let mut o = fields();
        o.task.body = "word ".repeat(60);
        let text = render(&o, &plain(), Some(300));
        assert!(text.lines().all(|line| line.chars().count() <= MEASURE), "{text}");
    }

    #[test]
    fn without_a_width_nothing_wraps() {
        let mut o = fields();
        let long = "word ".repeat(60).trim_end().to_string();
        o.task.body = long.clone();
        o.task.notes = vec![note("2026-09-25T15:52:18Z", "main", &long, None)];
        let text = render(&o, &plain(), None);
        assert!(text.contains(&format!("\n{long}\n")), "{text}");
        assert!(text.contains(&format!("\n    {long}\n")), "{text}");
        assert!(text.starts_with(&rule("overview")), "{text}");
    }

    #[test]
    fn a_tiny_width_never_panics_or_drops_words() {
        // A word wider than the space left is split hard, so compare without whitespace.
        let text = render(&example(), &plain(), Some(10));
        let joined: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for word in ["Phone", "Gboard.", "codex:abc", "phone.", "web-ui,bug"] {
            assert!(joined.contains(word), "{word}: {text}");
        }
    }

    #[test]
    fn color_is_decoration_only() {
        let colored = Painter::new(ColorMode::Always, crate::output::Format::Pretty, false);
        let painted = render(&example(), &colored, Some(120));
        assert!(painted.contains("\x1b[1m── overview \x1b[0m"), "{painted:?}");
        assert!(painted.contains("\x1b[2m(sci-000001)\x1b[0m"), "{painted:?}");
        assert!(painted.contains("\x1b[1mstarted\x1b[0m"), "{painted:?}");
        let stripped = {
            let mut out = String::new();
            let mut rest = painted.as_str();
            while let Some(start) = rest.find("\x1b[") {
                out.push_str(&rest[..start]);
                let after = &rest[start + 2..];
                rest = &after[after.find('m').unwrap() + 1..];
            }
            out.push_str(rest);
            out
        };
        assert_eq!(stripped, render(&example(), &plain(), Some(120)));
    }
}
```

Add `mod show_text;` to `src/main.rs` after `mod shorthand;`.

The `ParkInfo` literal in `sections_follow_the_spec_order` must list every `ParkInfo`
field. Read `pub struct ParkInfo` in `src/output.rs` and match it exactly, including any
fields after `worktree`.

- [ ] **Step 7: Run the tests to verify they fail**

Run: `just test-one show_text::`
Expected: FAIL. The output is empty, so every assertion on it fails.

- [ ] **Step 8: Implement the renderer**

Replace the stub `render` with:

```rust
pub fn render(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let width = width.map(|width| width.min(MEASURE));
    let mut sections = vec![overview(o, painter, width)];
    if let Some(park) = &o.park {
        sections.push(parked(park, painter, width));
    }
    if let Some(escalation) = &o.escalation {
        sections.push(escalated(escalation, painter, width));
    }
    sections.push(description(&o.task.body, painter, width));
    if let Some(related) = related(o, painter, width) {
        sections.push(related);
    }
    if !o.files.is_empty() {
        sections.push(files(o, painter, width));
    }
    if !o.task.notes.is_empty() {
        sections.push(notes(&o.task.notes, painter, width));
    }
    sections.join("\n")
}

/// `── name ` in the heading role, then a dim rule out to the line width (spec §3.2).
fn rule(name: &str, painter: &Painter, width: Option<usize>) -> String {
    let head = format!("── {name} ");
    let fill = width.unwrap_or(MEASURE).saturating_sub(head.chars().count());
    let mut line = present::heading(&head, None).paint(painter);
    if fill > 0 {
        line.push_str(&painter.paint(Style::Chrome, &"─".repeat(fill)));
    }
    line.push('\n');
    line
}

/// `pieces` after `lead` (already painted, `lead_width` columns wide), wrapped at
/// `width`; continuation lines indent `rest` columns. One line when there is no width.
/// The available width is the line width less the wider of the two indents, so no
/// line runs past it.
fn block(
    lead: &str,
    lead_width: usize,
    pieces: &[(&str, Option<Style>)],
    rest: usize,
    painter: &Painter,
    width: Option<usize>,
) -> String {
    let chars: Vec<(char, Option<Style>)> = pieces
        .iter()
        .flat_map(|(text, style)| text.chars().map(move |c| (c, *style)))
        .collect();
    let lines = match width {
        Some(width) => wrap_lines(&chars, width.saturating_sub(lead_width.max(rest)).max(1)),
        None => vec![chars],
    };
    let mut rendered = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            rendered.push_str(lead);
        } else {
            rendered.push_str(&" ".repeat(rest));
        }
        rendered.push_str(&paint_runs(line, painter));
        rendered.push('\n');
    }
    if lines.is_empty() {
        rendered.push_str(lead.trim_end());
        rendered.push('\n');
    }
    rendered
}

fn spans<'a>(spans: &'a [Span]) -> Vec<(&'a str, Option<Style>)> {
    spans.iter().map(|span| (span.text.as_str(), span.style)).collect()
}

fn overview(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = rule("overview", painter, width);
    let title = present::heading(&o.task.title, o.project_color);
    let id = present::id(&format!("({})", o.task.id));
    rendered.push_str(&block(
        "",
        0,
        &spans(&[title, Span::plain(" "), id]),
        0,
        painter,
        width,
    ));
    let rows = overview_rows(o);
    let key_width = rows.iter().map(|(key, _)| key.len() + 1).max().unwrap_or(0);
    for (key, value) in &rows {
        let lead = painter.paint(Style::Chrome, &format!("{:<key_width$}", format!("{key}:")));
        rendered.push_str(&block(
            &format!("{lead} "),
            key_width + 1,
            &spans(value),
            key_width + 1,
            painter,
            width,
        ));
    }
    rendered
}

/// Spec §3.3: one row per field with a value, in this order. `depends` and `parent` are
/// listed under `related` instead.
fn overview_rows(o: &ShowFields) -> Vec<(&'static str, Vec<Span>)> {
    let t = &o.task;
    let mut rows = vec![
        ("status", vec![present::status(t.status)]),
        ("priority", vec![present::priority(t.priority)]),
    ];
    if t.size.is_some() {
        rows.push(("size", vec![present::size(t.size)]));
    }
    if t.complexity.is_some() {
        rows.push(("complexity", vec![present::complexity(t.complexity)]));
    }
    rows.push(("process", vec![present::process(t.process, "unassessed")]));
    if t.lane {
        rows.push(("lane", vec![Span::plain("yes")]));
    } else if let Some(lane) = &o.in_lane {
        rows.push(("in lane", vec![present::id(&lane.to_string())]));
    }
    if t.parallel {
        rows.push(("parallel", vec![Span::plain("yes")]));
    }
    if !t.needs.is_empty() {
        rows.push(("needs", vec![Span::plain(t.needs.join(", "))]));
    }
    if let Some(owner) = &t.owner {
        rows.push(("owner", vec![present::owner(owner)]));
    }
    if let Some(claim) = &o.claim {
        rows.push(("claim", vec![present::claim(claim)]));
    }
    if !t.tags.is_empty() {
        rows.push(("tags", vec![present::tags(&t.tags)]));
    }
    rows.push(("created", vec![present::date(&t.created)]));
    rows.push(("updated", vec![present::date(&t.updated)]));
    if let Some(started) = &t.started {
        rows.push(("started", vec![present::date(started)]));
    }
    if let Some(completed) = &t.completed {
        rows.push(("completed", vec![present::date(completed)]));
    }
    if let Some(periodic) = &o.periodic {
        let mut value = vec![Span::plain(periodic.every.clone())];
        match (&periodic.due, periodic.due_now) {
            (Some(due), _) => value.extend([Span::plain(", due "), present::date(due)]),
            (None, true) => value.extend([Span::plain(", due "), Span::plain("now")]),
            (None, false) => {}
        }
        rows.push(("every", value));
    }
    // Completion stamps both to the same second; a reopen clears only `completed`.
    if let Some(last) = &t.last_done
        && t.completed.as_ref() != Some(last)
    {
        rows.push(("last done", vec![present::date(last)]));
    }
    if let Some(deferred) = &o.deferred {
        let mut value = vec![Span::plain("until "), present::date(&deferred.until)];
        if deferred.due {
            value.push(Span::plain(" (due)"));
        }
        rows.push(("deferred", value));
    }
    for (key, value) in [("source", &t.source), ("agent", &t.agent), ("model", &t.model)] {
        if let Some(value) = value {
            rows.push((key, vec![present::owner(value)]));
        }
    }
    if let Some(spec) = &t.spec {
        rows.push(("spec", vec![Span::plain(o.spec_path.clone().unwrap_or(spec.clone()))]));
    }
    if let Some(plan) = &t.plan {
        rows.push(("plan", vec![Span::plain(o.plan_path.clone().unwrap_or(plan.clone()))]));
    }
    if let Some(step) = &t.step {
        let mut value = vec![Span::plain(step.clone())];
        if o.step_found == Some(false) {
            value.extend([
                Span::plain("  "),
                Span {
                    text: "MISSING".into(),
                    style: Some(Style::Error),
                },
            ]);
        }
        rows.push(("step", value));
    }
    rows
}

fn parked(park: &ParkInfo, painter: &Painter, width: Option<usize>) -> String {
    let stop = crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes);
    let since = present::date(&park.at);
    let mut rendered = rule("parked", painter, width);
    rendered.push_str(&block(
        "",
        0,
        &spans(&[Span::plain(format!("waiting on {stop} since ")), since]),
        4,
        painter,
        width,
    ));
    rendered.push_str(&block("    ", 4, &[(&park.next_step, None)], 4, painter, width));
    let session = format!("session {} in {}", park.session, park.worktree);
    rendered.push_str(&block("    ", 4, &[(&session, Some(Style::Chrome))], 4, painter, width));
    rendered
}

fn escalated(escalation: &Escalation, painter: &Painter, width: Option<usize>) -> String {
    let since = present::date(&escalation.at);
    let mut rendered = rule("escalation", painter, width);
    let needs = format!("needs at least {} since ", escalation.level.as_str());
    rendered.push_str(&block(
        "",
        0,
        &spans(&[Span::plain(needs), since]),
        4,
        painter,
        width,
    ));
    let session = format!("session {}", escalation.session);
    rendered.push_str(&block("    ", 4, &[(&session, Some(Style::Chrome))], 4, painter, width));
    rendered
}

/// Spec §3.5: each body line wraps on its own; list items hang; fenced and indented
/// lines, and blank ones, print verbatim; `#` lines are headings.
fn description(body: &str, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = rule("description", painter, width);
    if body.trim().is_empty() {
        rendered.push_str(&painter.paint(Style::Chrome, "(no description)"));
        rendered.push('\n');
        return rendered;
    }
    let mut fenced = false;
    for line in body.lines() {
        let fence = line.trim_start().starts_with("```");
        if fence || fenced || line.is_empty() || line.starts_with([' ', '\t']) {
            if fence {
                fenced = !fenced;
            }
            rendered.push_str(line);
            rendered.push('\n');
            continue;
        }
        let style = line.starts_with('#').then_some(Style::Heading(None));
        rendered.push_str(&block("", 0, &[(line, style)], hang(line), painter, width));
    }
    rendered
}

/// The width of a list marker opening `line` (`- `, `* `, `12. `), which its wrapped
/// lines indent past; 0 for any other line.
fn hang(line: &str) -> usize {
    if line.starts_with("- ") || line.starts_with("* ") {
        return 2;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        digits + 2
    } else {
        0
    }
}

/// Spec §3.4: `<label>  <id>  <status>  <title>`, columns aligned, titles hanging.
fn related(o: &ShowFields, painter: &Painter, width: Option<usize>) -> Option<String> {
    let mut rows: Vec<(&str, &str, Span, &str)> = Vec::new();
    if let Some(parent) = &o.parent {
        rows.push(("parent", parent.id.as_str(), present::status(parent.status), parent.title.as_str()));
    }
    for dependency in &o.depends_on {
        rows.push((
            "depends on",
            dependency.id.as_str(),
            present::status_of(dependency.status),
            dependency.title.as_deref().unwrap_or("(unresolved)"),
        ));
    }
    for child in &o.children {
        rows.push(("child", child.id.as_str(), present::status(child.status), child.title.as_str()));
    }
    if rows.is_empty() {
        return None;
    }
    let label_width = rows.iter().map(|row| row.0.len()).max().unwrap_or(0);
    let id_width = rows.iter().map(|row| row.1.chars().count()).max().unwrap_or(0);
    // The longest status names (`blocked`, `shelved`, `dropped`) are seven wide.
    let status_width = 7;
    let lead_width = label_width + 2 + id_width + 2 + status_width + 2;
    let mut rendered = rule("related", painter, width);
    for (label, id, status, title) in rows {
        let lead = format!(
            "{}  {}  {}  ",
            painter.paint(Style::Chrome, &format!("{label:<label_width$}")),
            present::id(id).left(id_width).paint(painter),
            status.left(status_width).paint(painter),
        );
        rendered.push_str(&block(&lead, lead_width, &[(title, None)], lead_width, painter, width));
    }
    Some(rendered)
}

fn files(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = rule("files", painter, width);
    for file in &o.files {
        rendered.push_str(&format!(
            "- {} ({})\n",
            file.path,
            crate::attachments::human_size(file.bytes)
        ));
    }
    rendered
}

/// Spec §3.5: a header (date, time, owner, marker, session), then the text indented 4.
fn notes(notes: &[Note], painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = rule(&format!("notes ({})", notes.len()), painter, width);
    for note in notes {
        let (marker, text) = lifecycle(&note.text);
        // Note timestamps are validated `YYYY-MM-DDTHH:MM:SSZ` when the record is read.
        let mut header = vec![
            present::date(&note.at),
            Span::plain(" "),
            Span {
                text: note.at[11..16].into(),
                style: Some(Style::Chrome),
            },
            Span::plain("  "),
            present::owner(&note.by),
        ];
        if let Some(marker) = marker {
            header.extend([
                Span::plain("  "),
                Span {
                    text: marker.into(),
                    style: Some(Style::Emphasis),
                },
            ]);
        }
        if let Some(provenance) = &note.provenance {
            header.extend([
                Span::plain("  "),
                Span {
                    text: provenance.harness_session.clone(),
                    style: Some(Style::Chrome),
                },
            ]);
        }
        rendered.push_str(&block("", 0, &spans(&header), 2, painter, width));
        if let Some(text) = text {
            rendered.push_str(&block("    ", 4, &[(text, None)], 4, painter, width));
        }
    }
    rendered
}

/// Splits a generated lifecycle marker from its text (spec §3.5). Only the complete
/// forms the writers generate count: `started` is all marker, `completed; next due
/// <date>` too, `parked (waiting on <stop>): <next>` is a marker and the next step, and
/// anything else, however it begins, is authored text.
fn lifecycle(text: &str) -> (Option<&str>, Option<&str>) {
    if matches!(text, "started" | "resumed" | "done" | "dropped") {
        return (Some(text), None);
    }
    if let Some(day) = text.strip_prefix("completed; next due ")
        && crate::time::calendar_day(day).is_ok_and(|date| date.to_string() == day)
    {
        return (Some(text), None);
    }
    if let Some(rest) = text.strip_prefix("parked (waiting on ")
        && let Some((stop, next)) = rest.split_once("): ")
        && is_generated_stop(stop)
    {
        return (Some(&text[..text.len() - next.len() - 2]), Some(next));
    }
    (None, Some(text))
}

/// Whether `stop` is exactly what `claims::describe_stop` writes for some inputs: parse
/// it into those inputs, render them again, and compare. The round trip rejects unknown
/// words, a wrong order, and repeats without restating the grammar here.
fn is_generated_stop(stop: &str) -> bool {
    use crate::claims::{Needs, Reason, WaitingOn, describe_stop};
    let (head, recipe) = stop
        .split_once("; ")
        .map_or((stop, None), |(head, recipe)| (head, Some(recipe)));
    let (who, reason) = head
        .split_once(", ")
        .map_or((head, None), |(who, reason)| (who, Some(reason)));
    let Ok(who) = WaitingOn::parse(who) else {
        return false;
    };
    let Ok(reason) = reason.map(Reason::parse).transpose() else {
        return false;
    };
    let (mut needs, mut minutes) = (None, None);
    for part in recipe.into_iter().flat_map(|recipe| recipe.split(", ")) {
        match part.strip_suffix(" min") {
            Some(count) => match count.parse::<u32>() {
                Ok(count) => minutes = Some(count),
                Err(_) => return false,
            },
            None => match Needs::parse(part) {
                Ok(parsed) => needs = Some(parsed),
                Err(_) => return false,
            },
        }
    }
    describe_stop(who, reason, needs, minutes) == stop
}
```

Notes on this code:
- `present::owner` is used for `source`/`agent`/`model` because spec §4.2 gives them the
  same chrome role. If a reviewer prefers a clearer name, add `present::meta(&str) -> Span`
  with the same body and use it here.
- If `o.task.id` does not implement `Display`, use `o.task.id.as_str()` or whatever
  accessor `TaskId` has.
- The `if let … && …` chain needs edition 2024, which the crate uses. If clippy rejects
  it, nest the two conditions.

- [ ] **Step 9: Run the unit tests to verify they pass**

Run: `just test-one show_text::`
Expected: all PASS. If `the_example_renders_as_the_spec_shows` fails, diff the actual
output against the expected text: the expected text is the spec, so change the code,
not the expectation.

- [ ] **Step 10: Switch `pretty` to the renderer and delete the old one**

In `src/output.rs`'s `fn pretty`:

```rust
        Output::Show(o) => crate::show_text::render(&o.fields, painter, wrap.width()),
        Output::Next(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            match &o.next {
                Some(fields) => crate::show_text::render(fields, painter, wrap.width()),
                None => "nothing ready".into(),
            }
        ),
```

Delete `paint_frontmatter`, `paint_field`, and `show_text` (with its `related_row`
closure), and any imports that become unused (the compiler names them).

In `src/present.rs`'s `field_roles_are_built_only_here`, extend the file list:

```rust
        for (file, text) in [
            ("output.rs", include_str!("output.rs")),
            ("show_text.rs", include_str!("show_text.rs")),
        ] {
```

`show_text.rs` builds `Style::Chrome`, `Style::Emphasis` and `Style::Error` directly.
Those are not field roles, so the guard allows them.

- [ ] **Step 11: Update the integration tests that read the old text**

Add this helper to `tests/cli.rs` next to `strip_ansi`:

```rust
/// The value of `key`'s row in pretty `show`'s overview, whatever the key column's width.
fn overview_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(|rest| rest.trim().to_string()))
}
```

Run `just test-fast` and update each failing assertion to the new layout as follows. Keep
what each test checks; change only how it reads the text.

| test | old assertion | new assertion |
|---|---|---|
| `next_is_the_head_of_ready_in_show_shape` | `title: Top` and `# depends on` | `text.contains("Top (")` and `text.contains("── related")` and `text.contains("depends on")` |
| `park_appears_in_show_and_list_json_and_pretty_show` | `# parked` | `── parked` (keep the other two) |
| `add_resolves_spec_plan_and_step` | `\n\x1b[31m# step MISSING\x1b[0m\n` | `\x1b[31mMISSING\x1b[0m` |
| `show_resolves_local_and_foreign_dependencies` | `\x1b[2m{aid}\x1b[0m [todo] Local dep` | `\x1b[2m{aid}\x1b[0m  todo     Local dep` (two spaces, `todo` padded to 7, two spaces) and the same shape for `{fid}`; `aid` and `fid` are equal-length ids, so no extra padding |
| `show_pretty_prints_the_model_line` | `model: claude-fable-5-1\n` | `overview_value(&text, "model").as_deref() == Some("claude-fable-5-1")` |
| `show_reports_parent_and_children_and_list_filters_by_parent` | `\x1b[2m{one}\x1b[0m [todo] One`, and `[\x1b[31mblocked\x1b[0m] Two` | `\x1b[2m{one}\x1b[0m  todo     One` and `\x1b[2m{two}\x1b[0m  \x1b[31mblocked\x1b[0m  Two` |
| `colored_show_keeps_bold_priority_without_a_warning` | `priority: \x1b[1m1\x1b[0m\n` | leave for Task 3 (the theme wiring changes it); for now assert `\x1b[1mP1\x1b[0m` |
| `colored_show_paints_frontmatter_values_with_table_roles` | file-shaped spans | rename to `colored_show_paints_fields_with_table_roles`; keep the strip-ANSI and JSON-has-no-escapes checks; assert `\x1b[2m({id})\x1b[0m`, `\x1b[33mdoing\x1b[0m`, `\x1b[1mP0\x1b[0m` (no theme yet), `\x1b[2m{owner}\x1b[0m`, `\x1b[2mnow\x1b[0m` (tags), `\x1b[2m{parent}\x1b[0m`, `\x1b[2m{dep}\x1b[0m`; drop the "body and notes stay plain" and "ordinary priority stays plain" checks, which the spec reverses |
| `source_is_set_by_add_replaced_and_cleared_by_edit` | `source: "mail:<42@example.org>"` | `overview_value(&pretty, "source").as_deref() == Some("mail:<42@example.org>")` |
| `a_due_row_shows_its_cadence_and_due_date` | `every: 30d`, `due: now`; `every: 2w`, `due: 2` | `overview_value(&shown, "every").as_deref() == Some("30d, due now")`; `overview_value(&shown, "every").unwrap().starts_with("2w, due 2")` |
| `capability_park_validates_the_level_and_records_the_escalation` | `# escalation` | `── escalation`; keep `agent, capability` |
| `process_survives_ready_and_parked_next` | `Process: planned`, `Process: unassessed` | `overview_value(&text, "process").as_deref() == Some("planned")` / `Some("unassessed")` |
| `summary_rows_and_parked_rows_carry_agent` | `agent: codex/gpt-6\n` | `overview_value(&text, "agent").as_deref() == Some("codex/gpt-6")` |

In `tests/attachments.rs`, the attach/show test asserts `pretty.contains("# files")`.
Change it to `pretty.contains("── files")`, and keep its path and size check
(`"{path} (2 B)"`) as it is: the files section still prints `- <path> (<size>)`.

The quiet-park test (`a_quiet_park_records_…`), the park-reason test, the halt `next`
test, and `a_closed_stdout_reader_ends_the_command_quietly` read phrases the new layout
keeps (`waiting on user, quiet; idle, 50 min since`, `waiting on user, review since`,
`registered checkout`), so they should pass unchanged. If one fails, change its
assertion to the new text the same way.

Any other failing pretty-`show` assertion is updated by the same rule: find the field's
new spelling in spec §3, and use `overview_value` for overview rows.

- [ ] **Step 12: Run the fast suite**

Run: `just test-fast`
Expected: PASS, including `tests/attachments.rs`.

- [ ] **Step 13: Commit**

```bash
just check
git add src/style.rs src/show_text.rs src/main.rs src/output.rs src/present.rs src/commands/show.rs tests/cli.rs tests/attachments.rs
git commit -m "feat(show): print pretty show in a reading layout"
```

---

### Task 3: theme and width for `show` and `next`

Dates and priorities paint from the terminal theme, as in `list`, and prose wraps at the
terminal width.

**Files:**
- Modify: `src/output.rs` (`needs_theme`, `shows_priority`, `theme_gates_cover_dates_and_priorities`)
- Modify: `src/main.rs` (wrap-width selection)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `show_text::render(…, width)` (Task 2), `Wrap::width` (Task 2).

- [ ] **Step 1: Write the failing integration tests**

In `tests/cli.rs`, replace `colored_show_keeps_bold_priority_without_a_warning` with:

```rust
#[test]
fn colored_show_paints_dates_and_priorities_from_the_theme() {
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
    assert!(text.contains("\x1b[38;2;215;95;215mP1\x1b[0m"), "{text:?}");
    assert!(text.contains("\x1b[38;2;0;215;255m"), "created today paints: {text:?}");
    assert!(String::from_utf8(out.stderr).unwrap().is_empty());
}

#[test]
fn colored_show_warns_like_list_when_the_palette_lacks_magenta() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Urgent", "-p", "1"]));
    let out = env
        .cmd(&dir)
        .env("TASKS_PALETTE", "fg=#e5e3d7 bg=#13140d cyan=#00d7ff")
        .args(["--pretty", "--color", "always", "show", &id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\x1b[1mP1\x1b[0m"), "{text:?}");
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("priority colors off: TASKS_PALETTE has no magenta")
    );
}

#[test]
fn pretty_show_and_next_wrap_at_columns_and_json_ignores_it() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let long = "word ".repeat(40);
    let id = id_of(env.json(&dir, &["add", "Wrapped", "-b", long.trim_end()]));
    for command in [vec!["show", id.as_str()], vec!["next"]] {
        let out = env
            .cmd(&dir)
            .env("COLUMNS", "60")
            .arg("--pretty")
            .args(&command)
            .output()
            .unwrap();
        assert!(out.status.success(), "{command:?}");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("── description"), "{command:?}: {text}");
        assert!(
            text.lines().all(|line| line.chars().count() <= 60),
            "{command:?}: {text}"
        );

        let invalid = env
            .cmd(&dir)
            .env("COLUMNS", "0")
            .arg("--pretty")
            .args(&command)
            .output()
            .unwrap();
        assert_eq!(invalid.status.code(), Some(1), "{command:?}");
        assert!(
            String::from_utf8(invalid.stderr).unwrap().contains("COLUMNS"),
            "{command:?}"
        );

        let json = env
            .cmd(&dir)
            .env("COLUMNS", "0")
            .args(&command)
            .output()
            .unwrap();
        assert!(json.status.success(), "{command:?}: JSON ignores COLUMNS");
        let plain = env.cmd(&dir).args(&command).output().unwrap();
        assert_eq!(json.stdout, plain.stdout, "{command:?}: JSON unchanged");
    }
}

#[test]
fn piped_pretty_show_keeps_notes_on_one_line() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Piped"]));
    let long = "word ".repeat(40);
    env.json(&dir, &["note", &id, long.trim_end()]);
    let text = env.pretty(&dir, &["show", &id]);
    assert!(text.contains(&format!("\n    {}\n", long.trim_end())), "{text}");
}
```

In `columns_only_applies_to_pretty_tables`, change the loop to cover only `--json list`
and `--json show`. Then add an assertion that `--pretty show` with `COLUMNS=wide` now
fails:

```rust
    for args in [vec!["--json", "list"], vec!["--json", "show", &id]] {
```

```rust
    let pretty = env
        .cmd(&sci)
        .env("COLUMNS", "wide")
        .args(["--pretty", "show", &id])
        .output()
        .unwrap();
    assert_eq!(pretty.status.code(), Some(1), "pretty show now reads COLUMNS");
```

Also rename the test to `columns_applies_to_pretty_output_only`.

In `colored_show_paints_fields_with_table_roles` (renamed in Task 2), change
`\x1b[1mP0\x1b[0m` to `\x1b[1;38;2;215;95;215mP0\x1b[0m`: the theme now paints P0 bold
magenta.

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test cli show`
Expected: FAIL. `colored_show_paints_dates_and_priorities_from_the_theme` finds no
magenta, the wrap test finds lines wider than 60, the invalid-`COLUMNS` case succeeds,
and `columns_applies_to_pretty_output_only` sees pretty `show` succeed.

- [ ] **Step 3: Wire the theme and the width**

In `src/output.rs`, add `Output::Show(_) | Output::Next(_)` to both `needs_theme` and
`shows_priority`:

```rust
pub fn needs_theme(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Lanes(_)
            | Output::Tree(_)
            | Output::Parked(_)
            | Output::Quiet(_)
            | Output::Projects(_)
            | Output::Show(_)
            | Output::Next(_)
    )
}
```

```rust
pub fn shows_priority(out: &Output) -> bool {
    matches!(
        out,
        Output::List(_)
            | Output::Prime(_)
            | Output::Lanes(_)
            | Output::Tree(_)
            | Output::Quiet(_)
            | Output::Show(_)
            | Output::Next(_)
    )
}
```

Update the doc comments: `needs_theme`'s "Every output with a priority column also has a
date column" still holds, and `show`'s overview has both. In
`theme_gates_cover_dates_and_priorities`, add:

```rust
        let show = Output::Next(Box::new(NextOut {
            next: None,
            halts: vec![],
            warnings: vec![],
        }));
        assert!(needs_theme(&show));
        assert!(shows_priority(&show));
```

In `src/main.rs`, add the two variants to the wrap-width selection:

```rust
                    output::Output::List(_)
                        | output::Output::Prime(_)
                        | output::Output::Parked(_)
                        | output::Output::Tree(_)
                        | output::Output::Show(_)
                        | output::Output::Next(_)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `just test-one --test cli show`
Expected: PASS.

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
just check
git add src/output.rs src/main.rs tests/cli.rs
git commit -m "feat(show): paint show from the terminal theme and wrap it at the terminal width"
```

---

### Task 4: docs and record

**Files:**
- Modify: `README.md` (process paragraph near line 426; color section near line 492)
- Modify: `docs/specs/2026-10-03-show-layout-design.md` (status line)
- Modify: `docs/specs/2026-09-03-color-output-design.md`,
  `docs/specs/2026-09-25-date-recency-color-design.md`,
  `docs/specs/2026-09-25-priority-color-design.md` (status lines)
- Modify: `docs/plans/2026-10-03-show-layout.md` (status line)

- [ ] **Step 1: README**

In the process paragraph, replace "show and next print `Process: direct`, `Process:
planned`, or `Process: unassessed`" with "show and next print a `process:` row reading
`direct`, `planned`, or `unassessed`".

After the paragraph that starts "With color on, date columns fade by age", add:

```markdown
`show` and `next` print their task in a reading layout: an `overview` section with one
aligned `key: value` row per set field, then the park, the description, related tasks,
attached files, and the notes, each under a ruled heading. Prose wraps at the terminal
width (or `COLUMNS`), capped at 100 columns, and stays unwrapped when piped. Fields take
the colors `list` gives them. The task file itself is `tasks/<id>.md`, and JSON output
carries every field.
```

- [ ] **Step 2: Spec status lines**

- `docs/specs/2026-10-03-show-layout-design.md`: change `**Status:** approved (2026-10-03)`
  to `**Status:** implemented (2026-10-03); see docs/plans/2026-10-03-show-layout.md`.
- Each of the three amended specs: append to its status paragraph the sentence
  "`show`'s layout and colors are superseded by docs/specs/2026-10-03-show-layout-design.md
  (tasks-606552)."
- This plan: change `**Status:** draft (2026-10-03)` to `**Status:** executed (<date>)`.

- [ ] **Step 3: Verify the live binary and close**

Check the worktree's build by explicit path, never by installing it over the host's
`tasks` before the branch merges:

```bash
cargo build
COLUMNS=100 TASKS_PALETTE="fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7" \
  target/debug/tasks --pretty --color always show tasks-606552 | head -40
```

Expected: the reading layout, with colored dates and priority. Reinstall with
`cargo install --path .` from the main checkout after the merge, as AGENTS.md asks.
Then:

```bash
just check
git add README.md docs/
tasks done tasks-606552 "pretty show and next print the reading layout through present field roles; theme colors and capped wrapping"
git add tasks/
git commit -m "docs: record the show reading layout (tasks-606552)"
```
