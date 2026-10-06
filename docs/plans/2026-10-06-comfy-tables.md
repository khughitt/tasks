# Pretty tables on comfy-table Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every pretty table view renders through one module over comfy-table, with
side-by-side wrapping, shared widths in `prime`, project colours on ids and titles,
an optional header row, and warnings under a rule.

**Architecture:** A new `src/table.rs` takes columns (fixed or wrapping, with min/max)
and rows of already-painted cells, decides between a wrapped table, the stacked
fallback and an unwrapped table, and hands layout to comfy-table. The views in
`src/output.rs` build rows and call it; colours keep coming from `present` and the
`Painter`. A `ReadCtx` colour map fills `project_color` on every row a pretty view shows.

**Tech Stack:** Rust 2024, clap 4, comfy-table 8 (`custom_styling`), the existing
`style`/`present`/`palette` modules.

**Spec:** `docs/specs/2026-10-06-comfy-tables-design.md` (accepted, spec round 2).

## Global Constraints

- Dependency: `comfy-table = { version = "8", features = ["custom_styling"] }`, nothing else new.
- JSON output does not change: every new field is `#[serde(skip)]`; JSON tests stay untouched.
- Column gap is two spaces; no border characters; no line ends in whitespace.
- No width (piped or redirected): every row on one line, no column caps, every detail on one line.
- `claims` is never coloured: it opens no checkout.
- `projects` always shows its header row.
- `show`, `next` and `graph` keep their own layout.
- New CLI options land in ops `cli.toml` first and are adopted with `vendored adopt`.
- Gates: `just test-one <filter>` while working, `just test-fast` before every commit, never `cargo test`.
- Conventional commits, no AI attribution. Every task closes its step task (named in its commit step) with `tasks done` in the same commit as its code.

## Review Focus

1. A wide CJK or emoji title in a wrapping column: lines stay within the width and columns stay aligned (comfy-table measures display width; pin it in Task 2).
2. An empty table (no rows) with `--headers`: prints nothing, not a lone header (Task 2).
3. `prime` piped to a file: sections still align, because shared widths apply without a terminal (Task 6).
4. A parked row whose park names no resolvable task (an unresolved row: no status, no phase): renders `?` for status and `-` for phase, no panic (Task 5).
5. `--headers` combined with the narrow fallback: the moved column's header disappears with it (Task 2).

---

### Task 1: `--headers` in the shared CLI table (ops)

**Files:**
- Modify: ops `cli.toml` (the `[[cli.tasks.commands]]` row with `path = []`)

**Interfaces:**
- Produces: a root row `{ names = ["--headers"], value = "none" }` that tasks adopts in Task 7.

Work in an ops worktree (`work-link --ensure .worktrees`, `git worktree add .worktrees/cli-headers -b feat/cli-headers`, lock it), under ops-539e1a. Close tasks-8b152f in tasks when the ops merge lands.

- [ ] **Step 1: Add the row**

```toml
[[cli.tasks.commands]]
path = []
options = [
  { shared = "chdir", value = "path" },
  { names = ["--headers"], value = "none" },
]
```

- [ ] **Step 2: Run the table's tests**

Run: `just test-one tests.test_cli`
Expected: PASS (4 tests).

- [ ] **Step 3: Fast suite, commit, merge to ops main**

Run: `just test-fast` → OK. Commit `feat(cli): --headers on the tasks root (ops-539e1a)` with `tasks done ops-539e1a`, merge `--no-ff` into ops main, unlock and remove the worktree after `tt-report`.

---

### Task 2: The table module

**Files:**
- Modify: `Cargo.toml` (add the dependency), `src/main.rs` (add `mod table;` after `mod style;`)
- Create: `src/table.rs`

**Interfaces:**
- Produces (all `pub` in `crate::table`):
  - `enum Fit { Fixed, Wrap { min: usize, max: Option<usize> } }`
  - `enum Align { Left, Right }`
  - `struct Column { header: &'static str, fit: Fit, align: Align }` with `const fn fixed(header)`, `const fn right(header)`, `const fn wrap(header, min, max)`
  - `enum Detail { Wrap(String), Whole(String) }`
  - `struct Row { cells: Vec<String>, details: Vec<(&'static str, Detail)> }` with `fn new(cells: Vec<String>) -> Row`
  - `struct Layout { width: Option<usize>, headers: bool }` (`Clone, Copy, Default`)
  - `struct Widths(pub Vec<Option<usize>>)` with `fn id_only(id: usize, columns: usize) -> Widths` and `fn id(&self) -> Option<usize>`
  - `fn shared_widths(columns: &[Column], sections: &[&[Row]], headers: bool) -> Widths`
  - `fn render(columns: &[Column], rows: &[Row], layout: &Layout, widths: Option<&Widths>, painter: &Painter) -> String`
  - `fn visible_width(text: &str) -> usize` (moved here from `output.rs`; `output.rs` imports it)

- [ ] **Step 1: Add the dependency**

Run: `cargo add comfy-table@8 --features custom_styling`
Expected: `Cargo.toml` gains `comfy-table = { version = "8.0.1", features = ["custom_styling"] }`.

- [ ] **Step 2: Write the failing tests.** Create `src/table.rs` with the module doc, the imports and every type and helper from Step 4, but with `render`'s body replaced by `String::new()`, and put this test module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{ColorMode, Painter};

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, crate::output::Format::Pretty, false)
    }
    fn colored() -> Painter {
        Painter::new(ColorMode::Always, crate::output::Format::Pretty, false)
    }
    fn strip(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for t in chars.by_ref() {
                    if t.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }
    const LONG: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu \
        nu xi omicron pi rho sigma tau upsilon phi chi psi omega";
    fn columns() -> Vec<Column> {
        vec![
            Column::fixed("id"),
            Column::fixed("status"),
            Column::wrap("title", 20, Some(30)),
            Column::wrap("next", 30, None),
        ]
    }
    fn row(id: &str) -> Row {
        Row::new(vec![id.into(), "doing".into(), LONG.into(), LONG.into()])
    }
    fn at(width: usize) -> Layout {
        Layout { width: Some(width), headers: false }
    }

    #[test]
    fn two_wrap_columns_wrap_side_by_side_within_the_width() {
        let text = render(&columns(), &[row("sci-000001")], &at(100), None, &plain());
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        for line in &lines {
            assert!(visible_width(line) <= 100, "{line:?}");
            assert_eq!(line.trim_end(), *line, "trailing whitespace: {line:?}");
        }
        let title = lines[0].find("alpha").unwrap();
        let next = lines[0][title + 5..].find("alpha").unwrap() + title + 5;
        let continuation = lines[1];
        assert!(
            !continuation[title..title + 1].trim().is_empty(),
            "title continues in its column:\n{text}"
        );
        assert!(
            continuation.len() > next && !continuation[next..next + 1].trim().is_empty(),
            "next continues in its column:\n{text}"
        );
    }

    #[test]
    fn the_gap_is_two_spaces_and_fixed_columns_never_wrap() {
        let text = render(&columns(), &[row("sci-000001")], &at(100), None, &plain());
        assert!(text.starts_with("sci-000001  doing  alpha"), "{text}");
    }

    #[test]
    fn too_narrow_for_both_moves_the_last_wrap_column_under_the_row() {
        // fixed 10 + 5 + gaps 6 + mins 20 + 30 = 71 > 60, but 10 + 5 + 4 + 20 = 39 fits.
        let text = render(&columns(), &[row("sci-000001")], &at(60), None, &plain());
        let lines: Vec<&str> = text.lines().collect();
        let detail = lines.iter().position(|l| l.trim_start().starts_with("next:")).unwrap();
        assert!(detail > 0, "{text}");
        assert!(lines[detail].starts_with("        next:   alpha"), "{text}");
        for line in &lines {
            assert!(visible_width(line) <= 60, "{line:?}");
        }
    }

    #[test]
    fn the_moved_columns_header_leaves_with_it() {
        let layout = Layout { width: Some(60), headers: true };
        let text = render(&columns(), &[row("sci-000001")], &layout, None, &plain());
        let header = text.lines().next().unwrap();
        assert!(header.contains("title") && !header.contains("next"), "{text}");
    }

    #[test]
    fn narrower_still_renders_unwrapped_and_whole() {
        let text = render(&columns(), &[row("sci-000001")], &at(30), None, &plain());
        assert_eq!(text.lines().count(), 1, "{text}");
        assert_eq!(text.matches("omega").count(), 2, "{text}");
    }

    #[test]
    fn no_width_puts_every_row_on_one_line_with_no_caps() {
        let layout = Layout::default();
        let text = render(&columns(), &[row("a-000001"), row("b-000002")], &layout, None, &plain());
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(text.lines().all(|l| l.matches("omega").count() == 2), "{text}");
    }

    #[test]
    fn headers_show_only_when_asked_and_never_on_an_empty_table() {
        let layout = Layout { width: Some(200), headers: true };
        let text = render(&columns(), &[row("sci-000001")], &layout, None, &plain());
        assert!(text.starts_with("id"), "{text}");
        assert!(text.lines().next().unwrap().contains("status"), "{text}");
        assert_eq!(render(&columns(), &[], &layout, None, &plain()), "");
    }

    #[test]
    fn shared_widths_line_up_sections_with_different_content() {
        let a = [Row::new(vec!["sci-000001".into(), "todo".into(), "x".into(), "y".into()])];
        let b = [Row::new(vec!["sci-000002".into(), "doing".into(), "x".into(), "y".into()])];
        let widths = shared_widths(&columns(), &[&a, &b], false);
        let one = render(&columns(), &a, &at(100), Some(&widths), &plain());
        let two = render(&columns(), &b, &at(100), Some(&widths), &plain());
        assert_eq!(one.find('x'), two.find('x'), "{one}{two}");
        let piped_one = render(&columns(), &a, &Layout::default(), Some(&widths), &plain());
        let piped_two = render(&columns(), &b, &Layout::default(), Some(&widths), &plain());
        assert_eq!(piped_one.find('x'), piped_two.find('x'), "{piped_one}{piped_two}");
    }

    #[test]
    fn an_id_only_width_aligns_a_section_with_other_columns() {
        let widths = Widths::id_only(15, 4);
        let text = render(&columns(), &[row("sci-000001")], &at(200), Some(&widths), &plain());
        assert!(text.starts_with("sci-000001       doing"), "{text}");
    }

    #[test]
    fn painted_cells_keep_their_alignment() {
        let painter = colored();
        let paint = |s: &str| painter.paint(crate::style::Style::Chrome, s);
        let rows = [
            Row::new(vec![paint("sci-000001"), "doing".into(), paint(LONG), paint(LONG)]),
            Row::new(vec![paint("sci-2"), "todo".into(), paint(LONG), paint(LONG)]),
        ];
        let text = render(&columns(), &rows, &at(100), None, &painter);
        let plain_text = render(
            &columns(),
            &[
                Row::new(vec!["sci-000001".into(), "doing".into(), LONG.into(), LONG.into()]),
                Row::new(vec!["sci-2".into(), "todo".into(), LONG.into(), LONG.into()]),
            ],
            &at(100),
            None,
            &plain(),
        );
        assert_eq!(strip(&text), plain_text);
        for line in text.lines() {
            assert!(visible_width(line) <= 100, "{line:?}");
        }
    }

    #[test]
    fn wide_characters_count_by_display_width() {
        let title = "漢字のタイトル ".repeat(8);
        let rows = [Row::new(vec!["sci-000001".into(), "doing".into(), title.clone(), title])];
        let text = render(&columns(), &rows, &at(100), None, &plain());
        for line in text.lines() {
            assert!(visible_width(line) <= 100, "{line:?}");
        }
    }

    #[test]
    fn details_follow_their_row_and_whole_ones_are_never_split() {
        let path = "/a/long/checkout/path/that/runs/well/past/the/width/.worktrees/x";
        let mut first = row("sci-000001");
        first.details.push(("in", Detail::Whole(path.into())));
        let second = row("sci-000002");
        let text = render(&columns(), &[first, second], &at(80), None, &plain());
        let lines: Vec<&str> = text.lines().collect();
        let detail = lines.iter().position(|l| l.contains(path)).unwrap();
        let second_row = lines.iter().position(|l| l.starts_with("sci-000002")).unwrap();
        assert!(detail < second_row, "{text}");
        assert_eq!(lines[detail], format!("        in:     {path}"));
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `just test-one table::tests`
Expected: FAIL (the stub renders nothing).

- [ ] **Step 4: Implement the module** (above the tests in `src/table.rs`; move `visible_width` out of `src/output.rs` into here and `use crate::table::visible_width;` there)

```rust
//! Pretty tables: columns and rows of painted cells, laid out by comfy-table
//! (docs/specs/2026-10-06-comfy-tables-design.md §3). Views paint; this module places.

use crate::style::{Painter, Style};
use comfy_table::{
    CellAlignment, ColumnConstraint, ContentArrangement, Table, Width, presets::NOTHING,
};
use unicode_width::UnicodeWidthChar;

/// Spaces between two columns.
const GAP: usize = 2;
/// A detail line: eight spaces, then `label:` padded to seven columns and one space.
const DETAIL_INDENT: usize = 8;
const DETAIL_LABEL: usize = 7;
/// Below this many columns for its text, a detail line stays on one line.
const DETAIL_FLOOR: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Never wraps; as wide as its widest cell.
    Fixed,
    /// Wraps inside the column, at least `min` wide when the table has room.
    Wrap { min: usize, max: Option<usize> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub header: &'static str,
    pub fit: Fit,
    pub align: Align,
}

impl Column {
    pub const fn fixed(header: &'static str) -> Column {
        Column { header, fit: Fit::Fixed, align: Align::Left }
    }
    pub const fn right(header: &'static str) -> Column {
        Column { header, fit: Fit::Fixed, align: Align::Right }
    }
    pub const fn wrap(header: &'static str, min: usize, max: Option<usize>) -> Column {
        Column { header, fit: Fit::Wrap { min, max }, align: Align::Left }
    }
}

/// A `label: text` line under a row. `Wrap` wraps under its text; `Whole` is printed in
/// one piece so a path copies intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detail {
    Wrap(String),
    Whole(String),
}

#[derive(Debug, Clone, Default)]
pub struct Row {
    pub cells: Vec<String>,
    pub details: Vec<(&'static str, Detail)>,
}

impl Row {
    pub fn new(cells: Vec<String>) -> Row {
        Row { cells, details: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Layout {
    pub width: Option<usize>,
    pub headers: bool,
}

/// One width per column, `None` where a section sizes the column itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Widths(pub Vec<Option<usize>>);

impl Widths {
    /// Only the id column shared, for a section whose other columns differ.
    pub fn id_only(id: usize, columns: usize) -> Widths {
        let mut widths = vec![None; columns];
        if let Some(first) = widths.first_mut() {
            *first = Some(id);
        }
        Widths(widths)
    }

    pub fn id(&self) -> Option<usize> {
        self.0.first().copied().flatten()
    }
}

/// Every fixed column's widest cell across `sections`, so sections rendered apart line up.
pub fn shared_widths(columns: &[Column], sections: &[&[Row]], headers: bool) -> Widths {
    Widths(
        columns
            .iter()
            .enumerate()
            .map(|(index, column)| match column.fit {
                Fit::Wrap { .. } => None,
                Fit::Fixed => Some(widest(sections.iter().flat_map(|rows| rows.iter()), index, column, headers)),
            })
            .collect(),
    )
}

fn widest<'a>(rows: impl Iterator<Item = &'a Row>, index: usize, column: &Column, headers: bool) -> usize {
    rows.filter_map(|row| row.cells.get(index))
        .map(|cell| visible_width(cell))
        .chain(headers.then(|| visible_width(column.header)))
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Table,
    /// The column at this index left the table for a detail line under each row.
    Stacked(usize),
    Unwrapped,
}

fn fits(columns: &[Column], natural: &[usize], kept: &[usize], width: usize) -> bool {
    let cells: usize = kept
        .iter()
        .map(|&index| match columns[index].fit {
            Fit::Fixed => natural[index],
            Fit::Wrap { min, .. } => min.min(natural[index]),
        })
        .sum();
    cells + GAP * kept.len().saturating_sub(1) <= width
}

fn mode(columns: &[Column], natural: &[usize], width: Option<usize>) -> Mode {
    let Some(width) = width else {
        return Mode::Unwrapped;
    };
    let all: Vec<usize> = (0..columns.len()).collect();
    if fits(columns, natural, &all, width) {
        return Mode::Table;
    }
    let wraps: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&index| matches!(columns[index].fit, Fit::Wrap { .. }))
        .collect();
    if let [_, moved] = wraps[..] {
        let kept: Vec<usize> = all.iter().copied().filter(|&index| index != moved).collect();
        if fits(columns, natural, &kept, width) {
            return Mode::Stacked(moved);
        }
    }
    Mode::Unwrapped
}

fn clamp(width: usize) -> u16 {
    u16::try_from(width).unwrap_or(u16::MAX)
}

pub fn render(
    columns: &[Column],
    rows: &[Row],
    layout: &Layout,
    widths: Option<&Widths>,
    painter: &Painter,
) -> String {
    if rows.is_empty() || columns.is_empty() {
        return String::new();
    }
    let shared = |index: usize| widths.and_then(|widths| widths.0.get(index).copied().flatten());
    let natural: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let own = widest(rows.iter(), index, column, layout.headers);
            shared(index).map_or(own, |width| width.max(own))
        })
        .collect();
    let mode = mode(columns, &natural, layout.width);
    let kept: Vec<usize> = (0..columns.len())
        .filter(|&index| mode != Mode::Stacked(index))
        .collect();

    let mut table = Table::new();
    table.load_style(NOTHING).force_no_tty();
    match (mode, layout.width) {
        (Mode::Unwrapped, _) | (_, None) => {
            table.set_content_arrangement(ContentArrangement::Disabled);
        }
        (_, Some(width)) => {
            table
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_width(clamp(width));
        }
    }
    if layout.headers {
        table.set_header(
            kept.iter()
                .map(|&index| painter.paint(Style::Chrome, columns[index].header))
                .collect::<Vec<_>>(),
        );
    }
    for row in rows {
        table.add_row(
            kept.iter()
                .map(|&index| row.cells.get(index).cloned().unwrap_or_default())
                .collect::<Vec<_>>(),
        );
    }
    let last = kept.len() - 1;
    for (position, &index) in kept.iter().enumerate() {
        let column = table
            .column_mut(position)
            .expect("comfy-table holds one column per kept cell");
        let padding = if position == last { 0 } else { GAP };
        column.set_padding((0, clamp(padding)));
        if columns[index].align == Align::Right {
            column.set_cell_alignment(CellAlignment::Right);
        }
        // Constraint widths include the padding (comfy-table's absolute_width_with_padding).
        let constraint = match (columns[index].fit, mode) {
            (Fit::Fixed, _) => shared(index)
                .map(|width| ColumnConstraint::Absolute(Width::Fixed(clamp(natural[index].max(width) + padding)))),
            (Fit::Wrap { .. }, Mode::Unwrapped) => None,
            (Fit::Wrap { min, max }, _) => {
                let lower = Width::Fixed(clamp(min.min(natural[index]) + padding));
                Some(match max {
                    Some(max) => ColumnConstraint::Boundaries {
                        lower,
                        upper: Width::Fixed(clamp(max + padding)),
                    },
                    None => ColumnConstraint::LowerBoundary(lower),
                })
            }
        };
        if let Some(constraint) = constraint {
            column.set_constraint(constraint);
        }
    }

    let lines: Vec<String> = table.lines().map(|line| line.trim_end().to_string()).collect();
    let header_lines = usize::from(layout.headers);
    let has_details = matches!(mode, Mode::Stacked(_)) || rows.iter().any(|row| !row.details.is_empty());
    let mut rendered = String::new();
    for line in &lines[..header_lines] {
        rendered.push_str(line);
        rendered.push('\n');
    }
    if !has_details {
        for line in &lines[header_lines..] {
            rendered.push_str(line);
            rendered.push('\n');
        }
        return rendered;
    }
    // Every row with details starts with a non-blank first cell, and a continuation line
    // starts with that cell's blank padding, so a row starts where a line does not start
    // with a space.
    let blocks = row_blocks(&lines[header_lines..]);
    assert_eq!(blocks.len(), rows.len(), "one block of lines per row:\n{lines:?}");
    for (block, row) in blocks.iter().zip(rows) {
        for line in block {
            rendered.push_str(line);
            rendered.push('\n');
        }
        if let Mode::Stacked(moved) = mode {
            let text = row.cells.get(moved).cloned().unwrap_or_default();
            rendered.push_str(&detail(columns[moved].header, &Detail::Wrap(text), layout.width, painter));
        }
        for (label, item) in &row.details {
            rendered.push_str(&detail(label, item, layout.width, painter));
        }
    }
    rendered
}

fn row_blocks(lines: &[String]) -> Vec<Vec<&String>> {
    let mut blocks: Vec<Vec<&String>> = Vec::new();
    for line in lines {
        match blocks.last_mut() {
            Some(block) if line.starts_with(' ') || line.is_empty() => block.push(line),
            _ => blocks.push(vec![line]),
        }
    }
    blocks
}

fn detail(label: &str, item: &Detail, width: Option<usize>, painter: &Painter) -> String {
    let lead = format!(
        "{}{:<DETAIL_LABEL$} ",
        " ".repeat(DETAIL_INDENT),
        format!("{label}:")
    );
    let indent = DETAIL_INDENT + DETAIL_LABEL + 1;
    let lead = painter.paint(Style::Chrome, &lead);
    let (text, wrap) = match item {
        Detail::Wrap(text) => (text, true),
        Detail::Whole(text) => (text, false),
    };
    let available = width.map(|width| width.saturating_sub(indent));
    match available {
        Some(available) if wrap && available >= DETAIL_FLOOR => {
            let mut cell = Table::new();
            cell.load_style(NOTHING)
                .force_no_tty()
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_width(clamp(available))
                .add_row(vec![text.clone()]);
            if let Some(column) = cell.column_mut(0) {
                column.set_padding((0, 0));
            }
            let mut rendered = String::new();
            for (index, line) in cell.lines().enumerate() {
                if index == 0 {
                    rendered.push_str(&lead);
                } else {
                    rendered.push_str(&" ".repeat(indent));
                }
                rendered.push_str(line.trim_end());
                rendered.push('\n');
            }
            rendered
        }
        _ => format!("{lead}{text}\n"),
    }
}

/// Display columns of `text`, skipping ANSI escape sequences.
pub fn visible_width(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for terminator in chars.by_ref() {
                if terminator.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            count += c.width().unwrap_or(0);
        }
    }
    count
}
```

- [ ] **Step 5: Run the tests**

Run: `just test-one table::tests`
Expected: PASS (12 tests). If `the_gap_is_two_spaces_and_fixed_columns_never_wrap` or the detail-label test is off by the padding, check that constraints add `padding` (comfy-table subtracts it in `absolute_width_with_padding`).

- [ ] **Step 6: Fast suite and commit**

Run: `just test-fast` → all pass (the module is not yet used). Commit
`feat(pretty): table module over comfy-table (tasks-565420)` with `tasks done tasks-565420`.

---

### Task 3: Project colours on every row a pretty view shows

**Files:**
- Modify: `src/commands/mod.rs` (`ReadCtx`), `src/output.rs` (`ParkedRow`, a new `paint_projects`), `src/commands/list.rs` (list, parked, ready, prime), `src/commands/sample.rs`, `src/commands/tree.rs`, `src/commands/lanes.rs`, `src/commands/quiet.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `pub type ProjectColors = std::collections::HashMap<String, crate::palette::Rgb>;` in `src/output.rs`; `ReadCtx::project_colors(&self) -> ProjectColors`; `Output::paint_projects(&mut self, colors: &ProjectColors)`; `ParkedRow.project_color: Option<Rgb>` (`#[serde(skip)]`).

- [ ] **Step 1: Write the failing test** (next to `search_rows_carry_the_project_color` in `tests/cli.rs`)

```rust
#[test]
fn ready_tree_and_prime_rows_carry_the_project_color() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let goal = id_of(env.json(&sci, &["add", "Goal title", "--lane", "-b", "why"]));
    env.json(&sci, &["add", "Child title", "--parent", &goal, "--size", "s"]);
    write_project_color(&sci, "sci", "#123456");
    let paint = "\x1b[38;2;18;52;86m";
    // lanes paints its titles from Task 6, which pins it there.
    for args in [vec!["ready"], vec!["tree"], vec!["prime"]] {
        let mut full = vec!["--pretty", "--color", "always"];
        full.extend(args.iter().copied());
        let out = env.cmd(&sci).args(&full).output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            text.contains(&format!("{paint}Child title")) || text.contains(&format!("{paint}Goal title")),
            "{args:?}: {text}"
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `just test-one --test cli ready_tree_and_prime_rows_carry_the_project_color`
Expected: FAIL at `["ready"]` (only `list` and `search` fill the colour today).

- [ ] **Step 3: Implement**

In `src/output.rs`, beside `ParkedRow`:

```rust
/// Each in-scope prefix's configured colour (project colours spec; comfy-tables §5).
pub type ProjectColors = std::collections::HashMap<String, crate::palette::Rgb>;

fn color_of(colors: &ProjectColors, id: &str) -> Option<crate::palette::Rgb> {
    crate::model::TaskId::parse(id)
        .ok()
        .and_then(|id| colors.get(&id.prefix).copied())
}

fn paint_summary(colors: &ProjectColors, row: &mut TaskSummary) {
    row.project_color = color_of(colors, &row.id);
}

fn paint_tree(colors: &ProjectColors, nodes: &mut [TreeNode]) {
    for node in nodes {
        paint_summary(colors, &mut node.summary);
        paint_tree(colors, &mut node.children);
    }
}

impl Output {
    /// Fill `project_color` on every task row a pretty view of this output shows. `claims`
    /// is left alone: it opens no checkout (comfy-tables §5).
    pub fn paint_projects(&mut self, colors: &ProjectColors) {
        match self {
            Output::List(o) => o.tasks.iter_mut().for_each(|row| paint_summary(colors, row)),
            Output::Parked(o) => o.tasks.iter_mut().for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Quiet(o) => o.tasks.iter_mut().for_each(|row| row.project_color = color_of(colors, &row.id)),
            Output::Tree(o) => paint_tree(colors, &mut o.nodes),
            Output::Lanes(o) => o.lanes.iter_mut().for_each(|lane| paint_lane(colors, lane)),
            Output::Prime(o) => {
                for row in o.closeout.iter_mut().chain(o.ready.iter_mut()).chain(o.doing.iter_mut()) {
                    paint_summary(colors, row);
                }
                paint_tree(colors, &mut o.roadmap);
                for row in &mut o.parked {
                    row.project_color = color_of(colors, &row.id);
                }
                o.lanes.iter_mut().for_each(|lane| paint_lane(colors, lane));
            }
            _ => {}
        }
    }
}

fn paint_lane(colors: &ProjectColors, lane: &mut crate::lanes::LaneRow) {
    paint_summary(colors, &mut lane.lane);
    if let Some(pick) = &mut lane.pick {
        paint_summary(colors, pick);
    }
    lane.active.iter_mut().for_each(|row| paint_summary(colors, row));
}
```

Add to `ParkedRow` (after `title`): `#[serde(skip)] pub project_color: Option<crate::palette::Rgb>,`; set it from `summary.project_color` in `ParkedRow::resolved` and to `None` in `ParkedRow::unresolved` and in every test literal that builds a `ParkedRow` (search `ParkedRow {`).

In `src/commands/mod.rs`:

```rust
impl ReadCtx {
    /// Every in-scope project's configured colour, keyed by prefix.
    pub fn project_colors(&self) -> crate::output::ProjectColors {
        self.scope
            .projects()
            .iter()
            .filter_map(|project| project.color.map(|color| (project.prefix.clone(), color)))
            .collect()
    }
}
```

In each command, take the map before `ctx.warnings` is moved and apply it to the output before returning. `src/commands/list.rs::list` replaces the per-row block at its `ListOut` (the `row.project_color = ctx.scope.projects()...` closure) with:

```rust
    let colors = ctx.project_colors();
    let mut out = Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry, now))
            .collect(),
        halts: vec![],
        warnings: ctx.warnings,
        date: if periodic || deferred { DateColumn::Due } else { sort.date_column() },
    });
    out.paint_projects(&colors);
    Ok(out)
```

and applies the same two lines (`let colors = ctx.project_colors();` first, `out.paint_projects(&colors);` before `Ok(out)`) at the other constructions: `list_parked` (`Output::Parked`), the ready builder (`Output::List` near line 486), `prime` (`Output::Prime`), `src/commands/sample.rs` (`Output::List`), `src/commands/tree.rs` (`Output::Tree`), `src/commands/lanes.rs` (`Output::Lanes`), `src/commands/quiet.rs` (`Output::Quiet`). `src/commands/claims.rs` is not touched.

- [ ] **Step 4: Run the new test and the existing colour tests**

Run: `just test-one --test cli project_color` → PASS (the new test, `search_rows_carry_the_project_color`, `project_colors_*`).

- [ ] **Step 5: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): project colours on every row a pretty view shows (tasks-73c29c)` with `tasks done tasks-73c29c`.

---

### Task 4: The list family and tree on the table module

**Files:**
- Modify: `src/present.rs` (add `row_id`), `src/output.rs` (`table`, `tree_text`, `render`, `pretty`, the `Output::List` / `Output::Tree` arms; replace `Wrap` with `table::Layout`), `src/main.rs` (build a `Layout`)
- Test: `src/output.rs` unit tests, `tests/cli.rs`

**Interfaces:**
- Consumes: `crate::table::{Column, Fit, Row, Layout, Widths, render, shared_widths}` (Task 2); `TaskSummary.project_color` filled (Task 3).
- Produces:
  - `present::row_id(id: &str, project: Option<Rgb>) -> Span` (project colour, else chrome)
  - `output::task_columns(parallel: bool, kind: bool) -> Vec<table::Column>`
  - `output::task_row(row: &TaskSummary, date: DateColumn, painter: &Painter, parallel: bool, kind: bool, indent: usize) -> table::Row`
  - `output::tree_rows(nodes: &[TreeNode], depth: usize, painter: &Painter, parallel: bool, kind: bool, rows: &mut Vec<table::Row>)`
  - `output::render(out: &Output, format: Format, painter: &Painter, layout: &table::Layout) -> String` (was `wrap: Wrap`)

- [ ] **Step 1: Write the failing tests** (in `src/output.rs`'s test module, replacing `a_long_title_wraps_under_itself_at_the_terminal_width`)

```rust
    #[test]
    fn a_list_row_lays_out_its_columns_and_wraps_its_title_in_place() {
        let layout = crate::table::Layout { width: Some(80), headers: false };
        let rows = [long_row()];
        let columns = task_columns(false, false);
        let built: Vec<crate::table::Row> = rows
            .iter()
            .map(|row| task_row(row, DateColumn::Updated, &plain(), false, false, 0))
            .collect();
        let text = crate::table::render(&columns, &built, &layout, None, &plain());
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        let title = lines[0].find("alpha").unwrap();
        for line in &lines[1..] {
            assert_eq!(line.len() - line.trim_start().len(), title, "{text}");
        }
        assert!(lines.iter().all(|line| crate::table::visible_width(line) <= 80), "{text}");
    }

    #[test]
    fn a_tree_indents_children_inside_the_id_column() {
        let child = TreeNode { summary: row("xx-000002", false), children: vec![] };
        let root = TreeNode { summary: row("xx-000001", false), children: vec![child] };
        let mut built = Vec::new();
        tree_rows(&[root], 0, &plain(), false, false, &mut built);
        let text = crate::table::render(
            &task_columns(false, false),
            &built,
            &crate::table::Layout::default(),
            None,
            &plain(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("xx-000001"), "{text}");
        assert!(lines[1].starts_with("  xx-000002"), "{text}");
    }
```

and in `tests/cli.rs`:

```rust
#[test]
fn list_ids_take_the_project_color() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "A task"]));
    write_project_color(&sci, "sci", "#123456");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "always", "list"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&format!("\x1b[38;2;18;52;86m{id}")), "{text}");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one output::tests::a_` then `just test-one --test cli list_ids_take_the_project_color`
Expected: FAIL (`task_columns` and `task_row` do not exist; ids are chrome).

- [ ] **Step 3: Implement**

`src/present.rs`:

```rust
/// A task id in a table row: its project's colour when it has one, else chrome.
pub fn row_id(id: &str, project: Option<Rgb>) -> Span {
    Span {
        text: id.into(),
        style: Some(project.map_or(Style::Chrome, Style::Project)),
    }
}
```

`src/output.rs` — replace `table` with the column and row builders and a thin renderer:

```rust
pub fn task_columns(parallel: bool, kind: bool) -> Vec<crate::table::Column> {
    use crate::table::Column;
    let mut columns = vec![
        Column::fixed("id"),
        Column::fixed("P"),
        Column::fixed("size"),
        Column::fixed("cx"),
        Column::fixed("process"),
        Column::fixed("status"),
    ];
    if parallel {
        columns.push(Column::fixed("||"));
    }
    if kind {
        columns.push(Column::fixed("type"));
    }
    columns.push(Column::fixed("date"));
    columns.push(Column::wrap("title", 20, None));
    columns
}

pub fn task_row(
    row: &TaskSummary,
    date: DateColumn,
    painter: &Painter,
    parallel: bool,
    kind: bool,
    indent: usize,
) -> crate::table::Row {
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
    }
    .map(|span| span.paint(painter))
    .unwrap_or_else(|| "-".into());
    let mut cells = vec![
        format!(
            "{}{}",
            "  ".repeat(indent),
            present::row_id(&row.id, row.project_color).paint(painter)
        ),
        present::priority(row.priority).paint(painter),
        present::size(row.size).paint(painter),
        present::complexity(row.complexity).paint(painter),
        present::process(row.process, "-").paint(painter),
        present::status(row.status).paint(painter),
    ];
    if parallel {
        cells.push(if row.parallel { "||".into() } else { String::new() });
    }
    if kind {
        cells.push(
            type_letter(row)
                .map(|letter| painter.paint(Style::Emphasis, &letter.to_string()))
                .unwrap_or_default(),
        );
    }
    cells.push(date);
    cells.push(title_cell(row, painter));
    crate::table::Row::new(cells)
}

/// The title and the suffixes that ride with it: tags, a due cadence, a deferral, a quiet
/// recipe, the owner.
fn title_cell(row: &TaskSummary, painter: &Painter) -> String {
    let mut cell = present::title(&row.title, row.project_color).paint(painter);
    if !row.tags.is_empty() {
        let tags = present::tags(&row.tags);
        cell.push(' ');
        cell.push_str(&Span { text: format!("[{}]", tags.text), ..tags }.paint(painter));
    }
    if let Some(periodic) = row.periodic.as_ref().filter(|periodic| periodic.due_now) {
        let when = periodic
            .due
            .as_deref()
            .map(|due| crate::time::day(due).to_string())
            .unwrap_or_else(|| "now".into());
        cell.push_str("  ");
        cell.push_str(&painter.paint(Style::Emphasis, &format!("every {}, due {when}", periodic.every)));
    }
    if let Some(deferred) = &row.deferred {
        let text = if deferred.due {
            format!("due {}", deferred.until)
        } else {
            format!("defer {}", deferred.until)
        };
        cell.push_str("  ");
        cell.push_str(&painter.paint(Style::Emphasis, &text));
    }
    if let Some(park) = row.park.as_ref().filter(|park| park.reason == Some(crate::claims::Reason::Quiet)) {
        let text = format!(
            "waits on {}",
            crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes)
        );
        cell.push_str("  ");
        cell.push_str(&painter.paint(Style::Emphasis, &text));
    }
    let owner = match &row.claim {
        Some(claim) => Some(present::claim(claim)),
        None => row.owner.as_deref().map(present::owner),
    };
    if let Some(owner) = owner {
        cell.push(' ');
        cell.push_str(&Span { text: format!("@{}", owner.text), ..owner }.paint(painter));
    }
    cell
}

pub fn tree_rows(
    nodes: &[TreeNode],
    depth: usize,
    painter: &Painter,
    parallel: bool,
    kind: bool,
    rows: &mut Vec<crate::table::Row>,
) {
    for node in nodes {
        rows.push(task_row(&node.summary, DateColumn::Updated, painter, parallel, kind, depth));
        tree_rows(&node.children, depth + 1, painter, parallel, kind, rows);
    }
}

fn task_table(
    rows: &[TaskSummary],
    date: DateColumn,
    painter: &Painter,
    layout: &crate::table::Layout,
) -> String {
    let parallel = any_parallel(rows);
    let kind = any_type(rows);
    let built: Vec<crate::table::Row> = rows
        .iter()
        .map(|row| task_row(row, date, painter, parallel, kind, 0))
        .collect();
    crate::table::render(&task_columns(parallel, kind), &built, layout, None, painter)
}
```

(`use crate::present::Span;` at the top of `output.rs` if it is not imported.) The `Output::List` arm becomes `format!("{}{}", halt_line(&o.halts), task_table(&o.tasks, o.date, painter, layout))`; the `Output::Tree` arm builds `tree_rows(&o.nodes, 0, painter, any_parallel_tree(&o.nodes), any_type_tree(&o.nodes), &mut rows)` and renders it with `task_columns` of the same flags. Delete `tree_text`.

`render` and `pretty` take `layout: &crate::table::Layout` instead of `wrap: Wrap`; the `Show` and `Next` arms pass `layout.width`. Until Task 5 and Task 6 replace them, the `Parked`, `Quiet`, `Prime` and `Lanes` arms build the old `Wrap` from `layout.width` (`layout.width.map_or(Wrap::NONE, Wrap::at)`), and prime's `table(...)`/`tree_text(...)` calls are rewritten in Task 6. In this task, so the crate compiles, each `table(&o.<section>, ...)` call in the prime arm becomes `task_table(&o.<section>, DateColumn::Updated, painter, layout)`, and the roadmap's `tree_text(std::slice::from_ref(node), ...)` call becomes:

```rust
                    let mut rows = Vec::new();
                    tree_rows(std::slice::from_ref(node), 0, painter, parallel_column, type_column, &mut rows);
                    rendered.push_str(&crate::table::render(
                        &task_columns(parallel_column, type_column),
                        &rows,
                        layout,
                        None,
                        painter,
                    ));
```

Task 6 replaces both with the shared-width sections.

`src/main.rs`: replace the `wrap_width` block's result with a `Layout`:

```rust
            let layout = if format == Format::Pretty
                && matches!(
                    &out,
                    output::Output::List(_)
                        | output::Output::Prime(_)
                        | output::Output::Parked(_)
                        | output::Output::Quiet(_)
                        | output::Output::Tree(_)
                        | output::Output::Show(_)
                        | output::Output::Next(_)
                        | output::Output::Lanes(_)
                ) {
                match output::terminal_width() {
                    Ok(width) => table::Layout { width, headers: false },
                    Err(error) => {
                        to_stderr(&warnings);
                        to_stderr(&format!("{}\n", output::render_error(&error)));
                        std::process::exit(1);
                    }
                }
            } else {
                table::Layout::default()
            };
            let rendered = output::render(&out, format, &stdout_painter, &layout);
```

- [ ] **Step 4: Run the tests**

Run: `just test-one output::tests` and `just test-one --test cli list`
Expected: the new tests PASS. Existing tests that pin the old list spacing (single spaces between P, size, cx, process, status) fail: update each to the two-space gap and leave its assertion's intent unchanged. `project_colors_reset_and_resume_on_wrapped_titles` must still pass unchanged except for the continuation-indent column, which it reads from the plain output.

- [ ] **Step 5: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): list, ready, search and tree on the table module (tasks-2ac5d1)` with `tasks done tasks-2ac5d1`.

---

### Task 5: Parked and quiet on the table module

**Files:**
- Modify: `src/output.rs` (`parked_table`, `quiet_briefs`, their arms; delete `detail_line`)
- Test: `src/output.rs` unit tests, `tests/cli.rs`

**Interfaces:**
- Consumes: `table::{Column, Row, Detail, Layout, Widths, render}`; `ParkedRow.project_color` (Task 3); `present::row_id` (Task 4).
- Produces: `output::parked_columns() -> Vec<table::Column>`, `output::parked_row(row: &ParkedRow, painter: &Painter) -> table::Row`, `output::parked_table(rows: &[ParkedRow], painter: &Painter, layout: &table::Layout, widths: Option<&table::Widths>) -> String`, `output::quiet_table(rows: &[ParkedRow], painter: &Painter, layout: &table::Layout) -> String`.

- [ ] **Step 1: Write the failing tests** (replace `a_parked_rows_next_step_wraps_under_its_text`, `a_parked_tables_wait_column_fits_its_longest_stop`, `quiet_briefs_wrap_title_and_next_under_their_text` and `a_detail_line_keeps_a_long_word_whole`; keep `parked_with` and `LONG_STEP`)

```rust
    #[test]
    fn a_parked_rows_next_step_wraps_beside_its_title() {
        let layout = crate::table::Layout { width: Some(160), headers: false };
        let text = parked_table(&[parked_with(LONG_STEP, None)], &plain(), &layout, None);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("alpha") && lines[0].contains("Agent:"), "{text}");
        assert!(lines.len() > 1 && !lines.iter().any(|l| l.contains("next:")), "{text}");
        assert!(lines.iter().all(|l| crate::table::visible_width(l) <= 160), "{text}");
    }

    #[test]
    fn a_narrow_parked_table_moves_next_under_the_row() {
        let layout = crate::table::Layout { width: Some(100), headers: false };
        let text = parked_table(&[parked_with(LONG_STEP, None)], &plain(), &layout, None);
        assert!(text.contains("        next:   Agent:"), "{text}");
    }

    #[test]
    fn an_unresolved_park_renders_dashes() {
        let mut row = parked_with("x", None);
        row.status = None;
        row.phase = None;
        let text = parked_table(&[row], &plain(), &crate::table::Layout::default(), None);
        assert!(text.starts_with("xx-000001  ?  -  user"), "{text}");
    }

    #[test]
    fn quiet_keeps_its_checkout_whole_under_the_row() {
        let layout = crate::table::Layout { width: Some(100), headers: false };
        let row = parked_with(LONG_STEP, Some(crate::claims::Reason::Quiet));
        let worktree = row.park.as_ref().unwrap().worktree.clone();
        let text = quiet_table(&[row], &plain(), &layout);
        assert!(text.contains(&format!("        in:     {worktree}\n")), "{text}");
    }
```

and in `tests/cli.rs`:

```rust
#[test]
fn parked_rows_paint_ids_and_titles_with_the_project_color() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Parked title"]));
    write_park(&env, "sci", &id, "agent-a", "user", &sci.display().to_string());
    write_project_color(&sci, "sci", "#123456");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "always", "list", "--parked"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&format!("\x1b[38;2;18;52;86m{id}")), "{text}");
    assert!(text.contains("\x1b[38;2;18;52;86mParked"), "{text}");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one output::tests::a_parked` and `just test-one --test cli parked_rows_paint`
Expected: FAIL (old signatures; titles unpainted).

- [ ] **Step 3: Implement**

```rust
pub fn parked_columns() -> Vec<crate::table::Column> {
    use crate::table::Column;
    vec![
        Column::fixed("id"),
        Column::fixed("status"),
        Column::fixed("phase"),
        Column::fixed("waits on"),
        Column::fixed("parked"),
        Column::wrap("title", 20, Some(50)),
        Column::wrap("next", 40, None),
    ]
}

pub fn parked_row(row: &ParkedRow, painter: &Painter) -> crate::table::Row {
    let park = row.park.as_ref().expect("a parked row carries its park");
    crate::table::Row::new(vec![
        present::row_id(&row.id, row.project_color).paint(painter),
        present::status_of(row.status).paint(painter),
        row.phase.map(crate::model::Phase::as_str).unwrap_or("-").to_string(),
        crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes),
        present::date(&park.at).paint(painter),
        present::title(&row.title, row.project_color).paint(painter),
        painter.paint(Style::Chrome, &park.next_step),
    ])
}

pub fn parked_table(
    rows: &[ParkedRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
) -> String {
    let built: Vec<crate::table::Row> = rows
        .iter()
        .filter(|row| row.park.is_some())
        .map(|row| parked_row(row, painter))
        .collect();
    crate::table::render(&parked_columns(), &built, layout, widths, painter)
}

pub fn quiet_table(rows: &[ParkedRow], painter: &Painter, layout: &crate::table::Layout) -> String {
    use crate::table::{Column, Detail, Row};
    let columns = [
        Column::fixed("id"),
        Column::fixed("P"),
        Column::fixed("needs"),
        Column::right("min"),
        Column::fixed("parked"),
        Column::wrap("title", 20, Some(50)),
        Column::wrap("next", 40, None),
    ];
    let built: Vec<Row> = rows
        .iter()
        .filter_map(|row| row.park.as_ref().map(|park| (row, park)))
        .map(|(row, park)| {
            let mut built = Row::new(vec![
                present::row_id(&row.id, row.project_color).paint(painter),
                match row.priority {
                    Some(priority) => present::priority(priority).paint(painter),
                    None => "P-".into(),
                },
                park.needs.map(crate::claims::Needs::as_str).unwrap_or("-").to_string(),
                park.minutes.map_or_else(|| "-".into(), |minutes| format!("{minutes} min")),
                present::date(&park.at).paint(painter),
                present::title(&row.title, row.project_color).paint(painter),
                painter.paint(Style::Chrome, &park.next_step),
            ]);
            built.details.push(("in", Detail::Whole(park.worktree.clone())));
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, None, painter)
}
```

The `Output::Parked` arm calls `parked_table(&o.tasks, painter, layout, None)`; `Output::Quiet` calls `quiet_table(&o.tasks, painter, layout)`. Delete `quiet_briefs` and `detail_line`. Prime's parked call becomes `parked_table(&o.parked, painter, layout, None)` until Task 6.

- [ ] **Step 4: Run the tests**

Run: `just test-one output::tests` and `just test-one --test cli parked`, `just test-one --test cli quiet`
Expected: new tests PASS. Update existing CLI tests that look for the `next:` label or `waits on user, review` with a fixed 18-wide pad: `next:` now appears only in the narrow fallback; the wait column reads `user, review` with no `waits on` prefix (the header carries it). `quiet` briefs lose their blank separator lines.

- [ ] **Step 5: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): parked and quiet on the table module (tasks-9d8e5d)` with `tasks done tasks-9d8e5d`.

---

### Task 6: Prime with shared widths, and lanes

**Files:**
- Modify: `src/output.rs` (the `Output::Prime` and `Output::Lanes` arms; `lane_lines` becomes `lane_table`)
- Test: `src/output.rs` unit tests, `tests/cli.rs`

**Interfaces:**
- Consumes: `task_columns`, `task_row`, `tree_rows` (Task 4); `parked_table` (Task 5); `table::shared_widths`, `Widths::id_only`.
- Produces: `output::lane_table(rows: &[LaneRow], painter: &Painter, layout: &table::Layout, widths: Option<&table::Widths>, detail: bool) -> String`.

- [ ] **Step 1: Write the failing tests** (`tests/cli.rs`)

```rust
#[test]
fn prime_sections_share_every_fixed_width_when_piped() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let todo = id_of(env.json(&dir, &["add", "Ready one", "--size", "s"]));
    let doing = id_of(env.json(&dir, &["add", "Doing one", "--size", "xl"]));
    env.json(&dir, &["start", &doing]);
    let text = env.pretty(&dir, &["prime"]);
    let line = |id: &str| text.lines().find(|l| l.contains(id)).unwrap().to_string();
    let (a, b) = (line(&todo), line(&doing));
    assert_eq!(a.find("Ready"), b.find("Doing"), "titles align across sections:\n{text}");
}

#[test]
fn lanes_render_as_a_table_with_details() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let lane = id_of(env.json(&sci, &["add", "Lane title", "--lane", "-b", "Lane guidance here."]));
    env.json(&sci, &["add", "Step", "--parent", &lane, "--size", "s"]);
    let text = env.pretty(&sci, &["lanes"]);
    assert!(text.lines().next().unwrap().starts_with(&lane), "{text}");
    assert!(text.contains("        guide:  Lane guidance here."), "{text}");
    write_project_color(&sci, "sci", "#123456");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "always", "lanes"])
        .output()
        .unwrap();
    let colored = String::from_utf8(out.stdout).unwrap();
    assert!(colored.contains("\x1b[38;2;18;52;86mLane title"), "{colored}");
}
```

Keep both existing tests that assert "dates must start in the same column across prime's blocks".

- [ ] **Step 2: Run them to see them fail**

Run: `just test-one --test cli prime_sections_share` and `just test-one --test cli lanes_render_as`
Expected: FAIL (sections are sized apart since Task 4; lanes still use `lane_lines`).

- [ ] **Step 3: Implement**

`lane_table`:

```rust
fn lane_table(
    rows: &[LaneRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
    detail: bool,
) -> String {
    use crate::table::{Column, Detail, Row};
    let columns = [
        Column::fixed("id"),
        Column::fixed("P"),
        Column::wrap("title", 20, None),
        Column::wrap("state", 20, None),
    ];
    let built: Vec<Row> = rows
        .iter()
        .map(|row| {
            let state = match row.state {
                LaneState::Ready => {
                    let pick = row.pick.as_ref().expect("a ready lane has a pick");
                    format!("ready → {} {}", pick.id, pick.title)
                }
                LaneState::Held => {
                    let first = row.held.first().expect("a held lane names what it waits for");
                    format!("held: {} ← {}", first.need, first.holder)
                }
                LaneState::Waiting => waiting_text(&row.causes),
                LaneState::Paused => "paused".into(),
                LaneState::Empty => "empty".into(),
            };
            let mut built = Row::new(vec![
                present::row_id(&row.lane.id, row.lane.project_color).paint(painter),
                present::priority(row.lane.priority).paint(painter),
                present::title(&row.lane.title, row.lane.project_color).paint(painter),
                painter.paint(Style::Emphasis, &state),
            ]);
            if detail {
                if let Some(guidance) = &row.guidance {
                    built.details.push(("guide", Detail::Wrap(guidance.clone())));
                }
                for active in &row.active {
                    let holder = active
                        .claim
                        .as_ref()
                        .map(|claim| format!(" @{} [{}]", claim.owner, claim.session))
                        .unwrap_or_default();
                    built.details.push((
                        "active",
                        Detail::Wrap(painter.paint(Style::Chrome, &format!("{} {}{holder}", active.id, active.title))),
                    ));
                }
            }
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, widths, painter)
}
```

The `Output::Lanes` arm calls `lane_table(&o.lanes, painter, layout, None, true)`; delete `lane_lines`.

The `Output::Prime` arm keeps its header, counts, periodic and deferred lines and its section labels, and builds every section's rows before rendering any:

```rust
            let parallel = any_parallel(&o.closeout) || any_parallel_tree(&o.roadmap)
                || any_parallel(&o.ready) || any_parallel(&o.doing);
            let kind = any_type(&o.closeout) || any_type_tree(&o.roadmap)
                || any_type(&o.ready) || any_type(&o.doing);
            let columns = task_columns(parallel, kind);
            let rows_of = |rows: &[TaskSummary]| -> Vec<crate::table::Row> {
                rows.iter()
                    .map(|row| task_row(row, DateColumn::Updated, painter, parallel, kind, 0))
                    .collect()
            };
            let closeout = rows_of(&o.closeout);
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut roadmap = Vec::new();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    tree_rows(std::slice::from_ref(node), 0, painter, parallel, kind, &mut roadmap);
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    roadmap.push(task_row(&node.summary, DateColumn::Updated, painter, parallel, kind, 0));
                }
            }
            let ready = rows_of(&o.ready);
            let doing = rows_of(&o.doing);
            let parked: Vec<crate::table::Row> =
                o.parked.iter().filter(|row| row.park.is_some()).map(|row| parked_row(row, painter)).collect();
            let shared = crate::table::shared_widths(
                &columns,
                &[&closeout, &roadmap, &ready, &doing],
                layout.headers,
            );
            let id = crate::table::shared_widths(&parked_columns(), &[&parked], layout.headers)
                .id()
                .unwrap_or(0)
                .max(shared.id().unwrap_or(0))
                .max(o.lanes.iter().map(|row| row.lane.id.len()).max().unwrap_or(0));
            let mut shared = shared;
            shared.0[0] = Some(id);
            let section = |rows: &[crate::table::Row]| {
                crate::table::render(&columns, rows, layout, Some(&shared), painter)
            };
```

then, in today's order and under today's labels: `section(&closeout)`; the lanes block as `lane_table(&o.lanes, painter, layout, Some(&crate::table::Widths::id_only(id, 4)), false)`; `section(&roadmap)` followed by the `listed_under_ready` line; `crate::table::render(&parked_columns(), &parked, layout, Some(&crate::table::Widths::id_only(id, 7)), painter)`; `section(&ready)`; `section(&doing)`. Delete the `Wrap`-based fallbacks Task 4 left in this arm.

- [ ] **Step 4: Run the tests**

Run: `just test-one --test cli prime` and `just test-one --test cli lanes`
Expected: PASS, including the two existing date-alignment tests. Update existing lanes tests for the `guide:`/`active:` labels.

- [ ] **Step 5: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): prime shares its widths; lanes on the table module (tasks-ff7985)` with `tasks done tasks-ff7985`.

---

### Task 7: Projects, claims and the `--headers` option

**Files:**
- Modify: `tools/cli.toml` (adopt from ops), `src/cli.rs` (`Cli.headers`), `src/main.rs` (`Layout.headers`), `src/output.rs` (`Output::Projects` and `Output::Claims` arms; delete `grid_text`, `Cell`, `cell`, the private `Align`), `README.md`, `skills/tasks/SKILL.md`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: Task 1's merged `cli.toml`; `table::{Column, Row, Layout, render}`.
- Produces: `Cli.headers: bool`.

- [ ] **Step 1: Adopt the table**

Run: `"$(tasks --pretty root ops-539e1a)/bin/vendored" adopt cli.toml` in the worktree (`tasks root` prints the registered ops checkout).
Expected: `tools/cli.toml from <ops merge sha>`; `just test-one --test cli cli_vocabulary` now FAILS (the parser lacks `--headers`).

- [ ] **Step 2: Write the failing tests**

```rust
#[test]
fn headers_label_each_table_view_and_leave_json_alone() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "A task", "--size", "s"]));
    write_park(&env, "sci", &id, "agent-a", "user", &sci.display().to_string());
    for (args, first) in [
        (vec!["list"], "id"),
        (vec!["ready"], "id"),
        (vec!["list", "--parked"], "id"),
        (vec!["tree"], "id"),
    ] {
        let mut full = vec!["--headers"];
        full.extend(args.iter().copied());
        let text = env.pretty(&sci, &full);
        assert!(text.lines().next().unwrap().starts_with(first), "{args:?}: {text}");
        assert!(!env.pretty(&sci, &args).starts_with("id"), "{args:?}");
        let mut json = vec!["--headers"];
        json.extend(args.iter().copied());
        assert_eq!(env.json(&sci, &json), env.json(&sci, &args), "{args:?}");
    }
}

#[test]
fn claims_is_a_table_and_never_coloured() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "A task"]));
    env.json(&sci, &["start", &id]);
    write_project_color(&sci, "sci", "#123456");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "--color", "always", "claims"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&id) && !text.contains("38;2;18;52;86"), "{text}");
}
```

`projects_pretty_prints_one_header_and_aligned_columns` stays as the projects test.

- [ ] **Step 3: Run them to see them fail**

Run: `just test-one --test cli headers_label` → FAIL (`--headers` unknown).

- [ ] **Step 4: Implement**

`src/cli.rs`, in `Cli` after `color`:

```rust
    /// Label each column of a pretty table with a header row. Never applies to JSON.
    #[arg(long, global = true)]
    pub headers: bool,
```

`src/main.rs`: `Ok(width) => table::Layout { width, headers: cli.headers }` (bind `let headers = cli.headers;` where `cli` is still in scope if it has been moved by then).

`Output::Projects` arm — same cells as today, rendered by the module with headers always on:

```rust
        Output::Projects(o) => {
            use crate::table::{Column, Row};
            let mut columns = vec![Column::fixed("project")];
            for column in count_columns(&Counts::default(), o.closed) {
                columns.push(Column::right(column.label));
            }
            columns.push(Column::fixed("activity"));
            if o.paths {
                columns.push(Column::fixed("root"));
            }
            let rows: Vec<Row> = o
                .projects
                .iter()
                .map(|row| {
                    let mut cells = vec![present::project(&row.prefix, row.color).paint(painter)];
                    match &row.counts {
                        Some(counts) => cells.extend(count_columns(counts, o.closed).into_iter().map(|column| {
                            let value = column.value.to_string();
                            column.style.map_or(value.clone(), |style| painter.paint(style, &value))
                        })),
                        None => cells.extend(
                            count_columns(&Counts::default(), o.closed)
                                .iter()
                                .map(|_| painter.paint(Style::Chrome, "-")),
                        ),
                    }
                    cells.push(match (&row.counts, &row.last_activity) {
                        (None, _) => painter.paint(Style::Error, "unreachable"),
                        (Some(_), _) if row.retired.is_some() => painter.paint(Style::Chrome, "retired"),
                        (Some(_), Some(at)) => present::date(at).paint(painter),
                        (Some(_), None) => painter.paint(Style::Chrome, "-"),
                    });
                    if o.paths {
                        cells.push(row.root.clone());
                    }
                    Row::new(cells)
                })
                .collect();
            let always = crate::table::Layout { headers: true, ..*layout };
            crate::table::render(&columns, &rows, &always, None, painter)
        }
```

`Output::Claims` arm:

```rust
        Output::Claims(o) => {
            use crate::table::{Column, Row};
            let columns = [
                Column::fixed("id"),
                Column::fixed("session"),
                Column::fixed("live"),
                Column::fixed("worktree"),
            ];
            let rows: Vec<Row> = o
                .claims
                .iter()
                .map(|row| {
                    Row::new(vec![
                        row.id.clone(),
                        row.claim.session.clone(),
                        if row.claim.live { "live" } else { "stale" }.into(),
                        row.claim.worktree.clone(),
                    ])
                })
                .collect();
            crate::table::render(&columns, &rows, layout, None, painter)
                .trim_end_matches('\n')
                .to_string()
        }
```

Add `Output::Projects(_)` and `Output::Claims(_)` to `main.rs`'s list of outputs that get a width. Delete `grid_text`, `Cell`, `cell` and the private `Align` in `output.rs`.

README.md, in the command list after the `list --parked --waiting-on` line:

```
    tasks --headers list             # label each column of a pretty table
```

`skills/tasks/SKILL.md`, at the end of step 5's `list --parked` sentence: "`--headers` labels the columns of any pretty table."

- [ ] **Step 5: Run the tests**

Run: `just test-one --test cli headers`, `just test-one --test cli claims`, `just test-one --test cli projects`, `just test-one --test cli cli_vocabulary`
Expected: PASS.

- [ ] **Step 6: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): --headers; projects and claims on the table module (tasks-674d5d)` with `tasks done tasks-674d5d` (the commit holds `tools/cli.toml` with the implementation).

---

### Task 8: Warnings under a rule

**Files:**
- Modify: `src/present.rs` (add `rule`), `src/show_text.rs` (use it), `src/output.rs` (`pretty_warnings`), `src/main.rs`
- Test: `tests/cli.rs` (`pretty_warnings_follow_the_output_and_survive_an_early_exit` and the tests that grep `warning:` in pretty stderr)

**Interfaces:**
- Produces: `present::rule(name: &str, painter: &Painter, width: usize) -> String`; `output::pretty_warnings(warnings: &[String], painter: &Painter, width: Option<usize>) -> String`.

- [ ] **Step 1: Write the failing test** (update the existing one in `tests/cli.rs`)

In `pretty_warnings_follow_the_output_and_survive_an_early_exit`, replace `merged.find("warning: sci-eeeeee")` with `merged.find("── warnings (1) ")` and add, after the order assertion:

```rust
    let rule = merged.find("── warnings (1) ").unwrap();
    assert!(merged[rule..].contains("\nsci-eeeeee"), "warnings follow the rule unprefixed:\n{merged}");
    assert!(!merged.contains("warning: sci-eeeeee"), "{merged}");
```

and in its early-exit half, `text.contains("── warnings (1) ")` instead of `warning: sci-eeeeee`.

- [ ] **Step 2: Run it to see it fail**

Run: `just test-one --test cli pretty_warnings_follow`
Expected: FAIL (no rule yet).

- [ ] **Step 3: Implement**

`src/present.rs` (moved from `show_text::rule`, taking a resolved width):

```rust
/// `── name ` in the heading role, then a dim rule out to `width` (show layout §3.2).
pub fn rule(name: &str, painter: &Painter, width: usize) -> String {
    let head = format!("── {name} ");
    let fill = width.saturating_sub(head.chars().count());
    let mut line = heading(&head, None).paint(painter);
    if fill > 0 {
        line.push_str(&painter.paint(Style::Chrome, &"─".repeat(fill)));
    }
    line.push('\n');
    line
}
```

`src/show_text.rs`: delete its `rule` and call `present::rule(name, painter, width.unwrap_or(MEASURE))` at each former call site.

`src/output.rs`:

```rust
/// stderr text for warnings in pretty mode: a `── warnings (n)` rule, then one per line.
pub fn pretty_warnings(warnings: &[String], painter: &Painter, width: Option<usize>) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut rendered = present::rule(
        &format!("warnings ({})", warnings.len()),
        painter,
        width.unwrap_or(crate::show_text::MEASURE),
    );
    for warning in warnings {
        rendered.push_str(warning);
        rendered.push('\n');
    }
    rendered
}
```

`src/main.rs`: compute the warnings text after the layout is known, `output::pretty_warnings(&warnings, &stderr_painter, layout.width)`; on the invalid-`COLUMNS` path, where no width exists, pass `None`.

- [ ] **Step 4: Run the tests**

Run: `just test-one --test cli warning`
Expected: PASS. Update every other test that looked for `warning: <text>` on pretty stderr to look for `<text>` after the rule; JSON `warnings` assertions do not change.

- [ ] **Step 5: Fast suite and commit**

Run: `just test-fast` → all pass. Commit `feat(pretty): warnings under a rule after the output (tasks-014c55)` with `tasks done tasks-014c55`.

---

### Task 9: Remove the hand-rolled layout and prove widths on a realistic fixture

**Files:**
- Modify: `src/output.rs` (delete `render_row`, `render_wrapped`, `LongWord`, `wrap_lines_with`, `TITLE_FLOOR`, `Wrap`, `id_width`, `id_width_tree`; move `wrap_lines`, `fill_hard_split`, `paint_runs`, `columns` into `src/show_text.rs`), `src/show_text.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: everything above. Produces nothing new.

- [ ] **Step 1: Write the width test**

```rust
#[test]
fn every_table_view_fits_where_it_can_and_keeps_text_whole_where_it_cannot() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    for n in 0..4 {
        let id = id_of(env.json(&sci, &[
            "add",
            &format!("A realistic title number {n} that runs long enough to need wrapping in a table"),
            "--size", "m", "--tag", "pretty",
        ]));
        if n % 2 == 0 {
            env.json(&sci, &["start", &id]);
        }
        write_park(&env, "sci", &id, "agent-a", "user", &sci.display().to_string());
    }
    for width in ["80", "120", "236"] {
        for args in [vec!["list"], vec!["ready"], vec!["list", "--parked"], vec!["prime"], vec!["tree"]] {
            let out = env
                .cmd(&sci)
                .env("COLUMNS", width)
                .args(["--pretty"])
                .args(&args)
                .output()
                .unwrap();
            let text = String::from_utf8(out.stdout).unwrap();
            let limit: usize = width.parse().unwrap();
            for line in text.lines() {
                let fixed_overflow = !line.starts_with(' ') && line.chars().count() > limit
                    && line.contains("A realistic title number");
                assert!(
                    line.chars().count() <= limit || fixed_overflow,
                    "COLUMNS={width} {args:?}: {line:?}"
                );
                if fixed_overflow {
                    assert!(line.contains("that runs long enough to need wrapping in a table"),
                        "an unwrapped row keeps its title whole: {line:?}");
                }
            }
        }
    }
}
```

(The parks written by `write_park` carry the next step `finish §3`; the long-step behaviour is pinned by Task 5's unit tests.)

- [ ] **Step 2: Run it**

Run: `just test-one --test cli every_table_view_fits`
Expected: PASS (behaviour is complete; this pins it). If it fails, the failing line names the view and width: fix the view's column fits, not the test.

- [ ] **Step 3: Delete the dead layout code**

Remove the items listed under **Files**. `cargo build` (through `just check`) lists any remaining caller; there should be none outside `show_text.rs`, which now owns `wrap_lines`, `fill_hard_split`, `paint_runs` and `columns` (move them with their doc comments and unit tests, and change `show_text.rs`'s import from `crate::output::{..., paint_runs, wrap_lines}` to the local items).

- [ ] **Step 4: Gates**

Run: `just check` → clean (no dead-code warnings); `just test-fast` → all pass.

- [ ] **Step 5: Commit**

Commit `refactor(pretty): remove the hand-rolled table layout (tasks-f52fca)` with `tasks done tasks-f52fca`.
