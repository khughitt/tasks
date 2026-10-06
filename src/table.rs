//! Pretty tables: columns and rows of painted cells, laid out by comfy-table
//! (docs/specs/2026-10-06-comfy-tables-design.md §3). Views paint; this module places.

use crate::style::{Painter, Style};
use comfy_table::{ColumnConstraint, ContentArrangement, Table, Width, presets::NOTHING};
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
        Column {
            header,
            fit: Fit::Fixed,
            align: Align::Left,
        }
    }
    pub const fn right(header: &'static str) -> Column {
        Column {
            header,
            fit: Fit::Fixed,
            align: Align::Right,
        }
    }
    pub const fn wrap(header: &'static str, min: usize, max: Option<usize>) -> Column {
        Column {
            header,
            fit: Fit::Wrap { min, max },
            align: Align::Left,
        }
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
        Row {
            cells,
            details: Vec::new(),
        }
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
                Fit::Fixed => Some(widest(
                    sections.iter().flat_map(|rows| rows.iter()),
                    index,
                    column,
                    headers,
                )),
            })
            .collect(),
    )
}

fn widest<'a>(
    rows: impl Iterator<Item = &'a Row>,
    index: usize,
    column: &Column,
    headers: bool,
) -> usize {
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
        let kept: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&index| index != moved)
            .collect();
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
            column.set_cell_alignment(comfy_table::CellAlignment::Right);
        }
        // Constraint widths include the padding (comfy-table's absolute_width_with_padding).
        // A fixed column always carries a constraint: without one, Dynamic arrangement may
        // wrap it (plan review round 1).
        let constraint = match (columns[index].fit, mode) {
            (Fit::Fixed, _) => Some(match shared(index) {
                Some(_) => {
                    ColumnConstraint::Absolute(Width::Fixed(clamp(natural[index] + padding)))
                }
                None => ColumnConstraint::ContentWidth,
            }),
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

    let lines: Vec<String> = table.lines().map(|line| trim_visible_end(&line)).collect();
    let header_lines = usize::from(layout.headers);
    let has_details =
        matches!(mode, Mode::Stacked(_)) || rows.iter().any(|row| !row.details.is_empty());
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
    // Fixed columns never wrap and a row with details starts with a non-blank first cell,
    // so a row's first line starts with that cell and its continuation lines with the
    // cell's blank padding.
    let blocks = row_blocks(&lines[header_lines..]);
    assert_eq!(
        blocks.len(),
        rows.len(),
        "one block of lines per row:\n{lines:?}"
    );
    for (block, row) in blocks.iter().zip(rows) {
        for line in block {
            rendered.push_str(line);
            rendered.push('\n');
        }
        if let Mode::Stacked(moved) = mode {
            let text = row.cells.get(moved).cloned().unwrap_or_default();
            rendered.push_str(&detail(
                columns[moved].header,
                &Detail::Wrap(text),
                layout.width,
                painter,
            ));
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
    match width.map(|width| width.saturating_sub(indent)) {
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
                let prefix = if index == 0 {
                    lead.clone()
                } else {
                    " ".repeat(indent)
                };
                rendered.push_str(&trim_visible_end(&format!("{prefix}{line}")));
                rendered.push('\n');
            }
            rendered
        }
        _ => format!("{}\n", trim_visible_end(&format!("{lead}{text}"))),
    }
}

/// Trim trailing display whitespace without dropping the ANSI sequences around it.
fn trim_visible_end(text: &str) -> String {
    let mut end = 0;
    let mut escapes = Vec::new();
    let mut chars = text.char_indices();
    while let Some((index, c)) = chars.next() {
        if c == '\x1b' {
            let mut escape_end = index + c.len_utf8();
            for (offset, terminator) in chars.by_ref() {
                escape_end = offset + terminator.len_utf8();
                if terminator.is_ascii_alphabetic() {
                    break;
                }
            }
            escapes.push(index..escape_end);
        } else if !c.is_whitespace() {
            end = index + c.len_utf8();
        }
    }
    let mut trimmed = text[..end].to_string();
    for escape in escapes.into_iter().filter(|escape| escape.start >= end) {
        trimmed.push_str(&text[escape]);
    }
    trimmed
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
        Layout {
            width: Some(width),
            headers: false,
        }
    }

    #[test]
    fn an_id_only_width_aligns_a_section_with_other_columns() {
        let widths = Widths::id_only(15, 4);
        let text = render(
            &columns(),
            &[row("sci-000001")],
            &at(200),
            Some(&widths),
            &plain(),
        );
        assert!(text.starts_with("sci-000001       doing"), "{text}");
    }

    #[test]
    fn a_right_aligned_column_pads_on_the_left() {
        let columns = [Column::fixed("id"), Column::right("min")];
        let rows = [
            Row::new(vec!["a".into(), "5".into()]),
            Row::new(vec!["b".into(), "40".into()]),
        ];
        let text = render(&columns, &rows, &at(80), None, &plain());
        assert_eq!(text, "a   5\nb  40\n");
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
        let second_row = lines
            .iter()
            .position(|l| l.starts_with("sci-000002"))
            .unwrap();
        assert!(detail < second_row, "{text}");
        assert_eq!(lines[detail], format!("        in:     {path}"));
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
    fn a_long_fixed_cell_stays_whole_beside_wrapping_columns_and_details() {
        // fixed 40 + gaps 4 + wrap mins 20 + 30 = 94 <= 100: a table, not the stacked
        // fallback, so an unconstrained fixed column would be squeezed and wrapped here
        // (plan review round 2: with the old constraints this panics in row_blocks).
        let long = "a-fixed-cell-that-is-forty-columns-wide!";
        assert_eq!(long.len(), 40);
        let columns = [
            Column::fixed("id"),
            Column::wrap("title", 20, Some(30)),
            Column::wrap("next", 30, None),
        ];
        let mut first = Row::new(vec![long.into(), LONG.into(), LONG.into()]);
        first
            .details
            .push(("note", Detail::Wrap("after the first row".into())));
        let second = Row::new(vec!["sci-000002".into(), LONG.into(), LONG.into()]);
        let text = render(&columns, &[first, second], &at(100), None, &plain());
        assert!(
            !text.contains("next:"),
            "a table, not the stacked fallback:\n{text}"
        );
        assert!(text.lines().next().unwrap().starts_with(long), "{text}");
        assert!(
            text.lines().all(|line| visible_width(line) <= 100),
            "{text}"
        );
        let note = text
            .lines()
            .position(|l| l.contains("after the first row"))
            .unwrap();
        let next_row = text
            .lines()
            .position(|l| l.starts_with("sci-000002"))
            .unwrap();
        assert!(note < next_row, "{text}");
    }

    #[test]
    fn too_narrow_for_both_moves_the_last_wrap_column_under_the_row() {
        // fixed 10 + 5 + gaps 6 + mins 20 + 30 = 71 > 60, but 10 + 5 + 4 + 20 = 39 fits.
        let text = render(&columns(), &[row("sci-000001")], &at(60), None, &plain());
        let lines: Vec<&str> = text.lines().collect();
        let detail = lines
            .iter()
            .position(|l| l.trim_start().starts_with("next:"))
            .unwrap();
        assert!(detail > 0, "{text}");
        assert!(lines[detail].starts_with("        next:   alpha"), "{text}");
        for line in &lines {
            assert!(visible_width(line) <= 60, "{line:?}");
        }
    }

    #[test]
    fn the_moved_columns_header_leaves_with_it() {
        let layout = Layout {
            width: Some(60),
            headers: true,
        };
        let text = render(&columns(), &[row("sci-000001")], &layout, None, &plain());
        let header = text.lines().next().unwrap();
        assert!(
            header.contains("title") && !header.contains("next"),
            "{text}"
        );
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
        let text = render(
            &columns(),
            &[row("a-000001"), row("b-000002")],
            &layout,
            None,
            &plain(),
        );
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(
            text.lines().all(|l| l.matches("omega").count() == 2),
            "{text}"
        );
    }

    #[test]
    fn headers_show_only_when_asked_and_never_on_an_empty_table() {
        let layout = Layout {
            width: Some(200),
            headers: true,
        };
        let text = render(&columns(), &[row("sci-000001")], &layout, None, &plain());
        assert!(text.starts_with("id"), "{text}");
        assert!(text.lines().next().unwrap().contains("status"), "{text}");
        assert_eq!(render(&columns(), &[], &layout, None, &plain()), "");
    }

    #[test]
    fn shared_widths_line_up_sections_with_different_content() {
        let a = [Row::new(vec![
            "sci-000001".into(),
            "todo".into(),
            "x".into(),
            "y".into(),
        ])];
        let b = [Row::new(vec![
            "sci-000002".into(),
            "doing".into(),
            "x".into(),
            "y".into(),
        ])];
        let widths = shared_widths(&columns(), &[&a, &b], false);
        let one = render(&columns(), &a, &at(100), Some(&widths), &plain());
        let two = render(&columns(), &b, &at(100), Some(&widths), &plain());
        assert!(one.contains("sci-000001") && one.contains('x'), "{one}");
        assert!(two.contains("sci-000002") && two.contains('x'), "{two}");
        assert_eq!(one.find('x'), two.find('x'), "{one}{two}");
        let piped_one = render(&columns(), &a, &Layout::default(), Some(&widths), &plain());
        let piped_two = render(&columns(), &b, &Layout::default(), Some(&widths), &plain());
        assert_eq!(
            piped_one.find('x'),
            piped_two.find('x'),
            "{piped_one}{piped_two}"
        );
    }

    #[test]
    fn painted_cells_keep_their_alignment() {
        let painter = colored();
        let paint = |s: &str| painter.paint(crate::style::Style::Chrome, s);
        let rows = [
            Row::new(vec![
                paint("sci-000001"),
                "doing".into(),
                paint(LONG),
                paint(LONG),
            ]),
            Row::new(vec![
                paint("sci-2"),
                "todo".into(),
                paint(LONG),
                paint(LONG),
            ]),
        ];
        let text = render(&columns(), &rows, &at(100), None, &painter);
        let plain_text = render(
            &columns(),
            &[
                Row::new(vec![
                    "sci-000001".into(),
                    "doing".into(),
                    LONG.into(),
                    LONG.into(),
                ]),
                Row::new(vec![
                    "sci-2".into(),
                    "todo".into(),
                    LONG.into(),
                    LONG.into(),
                ]),
            ],
            &at(100),
            None,
            &plain(),
        );
        assert!(text.contains("\x1b["), "{text:?}");
        assert!(
            plain_text.contains("sci-000001") && plain_text.contains("alpha"),
            "{plain_text}"
        );
        assert_eq!(strip(&text), plain_text);
        for line in text.lines() {
            assert!(visible_width(line) <= 100, "{line:?}");
        }
    }

    #[test]
    fn painted_cells_and_details_have_no_visible_trailing_whitespace() {
        let columns = [Column::fixed("id"), Column::wrap("title", 20, None)];
        for layout in [Layout::default(), at(40)] {
            for painter in [plain(), colored()] {
                let paint = |text: &str| painter.paint(Style::Chrome, text);
                let mut row = Row::new(vec!["sci-000001".into(), paint("title  ")]);
                row.details = vec![
                    ("empty", Detail::Wrap(String::new())),
                    ("note", Detail::Wrap(paint("text  "))),
                    ("long", Detail::Wrap(paint(&format!("{LONG}  ")))),
                ];
                let text = render(&columns, &[row], &layout, None, &painter);
                let visible = strip(&text);
                assert!(
                    visible.contains("title") && visible.contains("text"),
                    "{text:?}"
                );
                assert!(visible.contains("empty:"), "{text:?}");
                assert!(visible.contains("omega"), "{text:?}");
                for line in visible.lines() {
                    assert_eq!(line, line.trim_end(), "{layout:?}: {text:?}");
                }
                if painter.enabled() {
                    assert!(
                        text.contains("\x1b[2m") && text.contains("\x1b[0m"),
                        "{text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn wide_characters_count_by_display_width() {
        let title = "漢字のタイトル ".repeat(8);
        let rows = [Row::new(vec![
            "sci-000001".into(),
            "doing".into(),
            title.clone(),
            title,
        ])];
        let text = render(&columns(), &rows, &at(100), None, &plain());
        assert!(
            text.contains("sci-000001") && text.contains("漢字"),
            "{text}"
        );
        for line in text.lines() {
            assert!(visible_width(line) <= 100, "{line:?}");
        }
    }
}
