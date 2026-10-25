//! The pretty `show` layout (docs/specs/2026-10-03-show-layout-design.md): ruled
//! sections, aligned overview rows, and prose wrapped at a capped measure. `next`
//! prints its task through the same renderer.

use crate::claims::Escalation;
use crate::model::Note;
use crate::output::{ParkInfo, ShowFields};
use crate::palette::Rgb;
use crate::present::{self, Span};
use crate::style::{Painter, Style};
use unicode_width::UnicodeWidthChar;

// ponytail: per-character widths can split joined emoji; use grapheme clusters if needed.
fn columns(chars: &[(char, Option<Style>)]) -> usize {
    chars.iter().map(|(c, _)| c.width().unwrap_or(0)).sum()
}

/// Paint runs of one style each. Adjacent pieces sharing a style merge into one run; the
/// visible text is unchanged, so `ColorMode::Never` output matches painting each piece
/// whole.
fn paint_runs(chars: &[(char, Option<Style>)], painter: &Painter) -> String {
    let mut rendered = String::new();
    let mut index = 0;
    while index < chars.len() {
        let style = chars[index].1;
        let end = chars[index..]
            .iter()
            .take_while(|(_, s)| *s == style)
            .count()
            + index;
        let text: String = chars[index..end].iter().map(|(c, _)| c).collect();
        rendered.push_str(&match style {
            Some(style) => painter.paint(style, &text),
            None => text,
        });
        index = end;
    }
    rendered
}

/// Greedy word wrap over already-styled characters: a break lands on a space run, which is
/// dropped there, and a word longer than `available` is hard-split at the width. A word
/// keeps its style across the split.
fn wrap_lines(
    chars: &[(char, Option<Style>)],
    available: usize,
) -> Vec<Vec<(char, Option<Style>)>> {
    let mut lines: Vec<Vec<(char, Option<Style>)>> = Vec::new();
    let mut line: Vec<(char, Option<Style>)> = Vec::new();
    let mut pending: Option<&[(char, Option<Style>)]> = None;
    let mut index = 0;
    while index < chars.len() {
        let spaces = chars[index].0 == ' ';
        let end = chars[index..]
            .iter()
            .take_while(|(c, _)| (*c == ' ') == spaces)
            .count()
            + index;
        let run = &chars[index..end];
        index = end;
        if spaces {
            if !line.is_empty() {
                pending = Some(run);
            }
            continue;
        }
        let sep = pending.take().unwrap_or(&[]);
        if columns(&line) + columns(sep) + columns(run) <= available {
            line.extend_from_slice(sep);
            line.extend_from_slice(run);
        } else if columns(run) > available {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            fill_hard_split(&mut lines, &mut line, run, available);
        } else {
            lines.push(std::mem::take(&mut line));
            line.extend_from_slice(run);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Chunk a word longer than the width into full-width lines, leaving the tail on the
/// current line.
fn fill_hard_split(
    lines: &mut Vec<Vec<(char, Option<Style>)>>,
    line: &mut Vec<(char, Option<Style>)>,
    word: &[(char, Option<Style>)],
    available: usize,
) {
    let mut word = word;
    while columns(word) > available {
        let mut used = 0;
        let split = word
            .iter()
            .take_while(|(c, _)| {
                let next = used + c.width().unwrap_or(0);
                if next > available {
                    false
                } else {
                    used = next;
                    true
                }
            })
            .count()
            .max(1);
        let (chunk, rest) = word.split_at(split);
        lines.push(chunk.to_vec());
        word = rest;
    }
    line.extend_from_slice(word);
}

/// Prose never runs wider than this, however wide the terminal (spec §3.6).
pub const MEASURE: usize = 100;

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
    if !o.inventory.is_empty() {
        sections.push(references(o, painter, width));
    }
    if !o.files.is_empty() {
        sections.push(files(o, painter, width));
    }
    if !o.task.notes.is_empty() {
        sections.push(notes(&o.task.notes, painter, width));
    }
    sections.join("\n")
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

fn spans(spans: &[Span]) -> Vec<(&str, Option<Style>)> {
    spans
        .iter()
        .map(|span| (span.text.as_str(), span.style))
        .collect()
}

fn overview(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = present::rule("overview", painter, width.unwrap_or(MEASURE));
    let project = color(o, &o.task.id.to_string());
    let title = present::heading(&o.task.title, project);
    let id = present::id(&format!("({})", o.task.id), project);
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

/// The colour of the project `id` belongs to, when the view knows one.
fn color(o: &ShowFields, id: &str) -> Option<Rgb> {
    crate::output::color_of(&o.project_colors, id)
}

/// Spec §3.3: one row per field with a value, in this order. `depends` and `parent` are
/// listed under `related` instead.
fn overview_rows(o: &ShowFields) -> Vec<(&'static str, Vec<Span>)> {
    let t = &o.task;
    let mut rows = vec![
        ("status", vec![present::status(t.status)]),
        ("priority", vec![present::priority(t.priority)]),
    ];
    if let Some(to) = &t.moved_to {
        let to = to.to_string();
        rows.push(("moved to", vec![present::id(&to, color(o, &to))]));
    }
    if let Some(from) = &t.moved_from {
        let from = from.to_string();
        rows.push(("moved from", vec![present::id(&from, color(o, &from))]));
    }
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
        let lane = lane.to_string();
        rows.push(("in lane", vec![present::id(&lane, color(o, &lane))]));
    }
    if t.parallel {
        rows.push(("parallel", vec![Span::plain("yes")]));
    }
    if t.reviewed {
        let mut value = vec![Span::plain("yes")];
        if let Some(at) = &t.reviewed_at {
            value.extend([Span::plain(" ("), present::date(at), Span::plain(")")]);
        }
        rows.push(("reviewed", value));
    }
    if t.awaiting_user {
        rows.push(("awaiting", vec![Span::plain("user")]));
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
    if let Some(closed) = &t.closed {
        rows.push(("closed", vec![present::date(closed)]));
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
    for (key, value) in [
        ("source", &t.source),
        ("agent", &t.agent),
        ("model", &t.model),
    ] {
        if let Some(value) = value {
            rows.push((key, vec![present::owner(value)]));
        }
    }
    if let Some(spec) = &t.spec {
        rows.push((
            "spec",
            vec![Span::plain(o.spec_path.clone().unwrap_or(spec.clone()))],
        ));
    }
    if let Some(plan) = &t.plan {
        rows.push((
            "plan",
            vec![Span::plain(o.plan_path.clone().unwrap_or(plan.clone()))],
        ));
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
    let mut rendered = present::rule("parked", painter, width.unwrap_or(MEASURE));
    rendered.push_str(&block(
        "",
        0,
        &spans(&[Span::plain(format!("waiting on {stop} since ")), since]),
        4,
        painter,
        width,
    ));
    rendered.push_str(&block(
        "    ",
        4,
        &[(&park.next_step, None)],
        4,
        painter,
        width,
    ));
    let session = format!("session {} in {}", park.session, park.worktree);
    rendered.push_str(&block(
        "    ",
        4,
        &[(&session, Some(Style::Chrome))],
        4,
        painter,
        width,
    ));
    rendered
}

fn escalated(escalation: &Escalation, painter: &Painter, width: Option<usize>) -> String {
    let since = present::date(&escalation.at);
    let mut rendered = present::rule("escalation", painter, width.unwrap_or(MEASURE));
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
    rendered.push_str(&block(
        "    ",
        4,
        &[(&session, Some(Style::Chrome))],
        4,
        painter,
        width,
    ));
    rendered
}

/// Spec §3.5: each body line wraps on its own; list items hang; fenced and indented
/// lines, and blank ones, print verbatim; `#` lines are headings.
fn description(body: &str, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = present::rule("description", painter, width.unwrap_or(MEASURE));
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
    // The last field is the row's project colour; an unresolved dependency has none.
    let mut rows: Vec<(&str, &str, Span, &str, Option<Rgb>)> = Vec::new();
    if let Some(parent) = &o.parent {
        rows.push((
            "parent",
            parent.id.as_str(),
            present::status(parent.status),
            parent.title.as_str(),
            color(o, &parent.id),
        ));
    }
    for dependency in &o.depends_on {
        rows.push((
            "depends on",
            dependency.id.as_str(),
            present::status_of(dependency.status),
            dependency.title.as_deref().unwrap_or("(unresolved)"),
            color(o, &dependency.id).filter(|_| dependency.resolved),
        ));
    }
    for child in &o.children {
        rows.push((
            "child",
            child.id.as_str(),
            present::status(child.status),
            child.title.as_str(),
            color(o, &child.id),
        ));
    }
    if rows.is_empty() {
        return None;
    }
    let label_width = rows.iter().map(|row| row.0.len()).max().unwrap_or(0);
    let id_width = rows
        .iter()
        .map(|row| row.1.chars().count())
        .max()
        .unwrap_or(0);
    // The longest status names (`blocked`, `shelved`, `dropped`) are seven wide.
    let status_width = 7;
    let lead_width = label_width + 2 + id_width + 2 + status_width + 2;
    let mut rendered = present::rule("related", painter, width.unwrap_or(MEASURE));
    for (label, id, status, title, project) in rows {
        let lead = format!(
            "{}  {}  {}  ",
            painter.paint(Style::Chrome, &format!("{label:<label_width$}")),
            present::id(id, project).left(id_width).paint(painter),
            status.left(status_width).paint(painter),
        );
        rendered.push_str(&block(
            &lead,
            lead_width,
            &[(title, present::title(title, project).style)],
            lead_width,
            painter,
            width,
        ));
    }
    Some(rendered)
}

fn references(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = present::rule("references", painter, width.unwrap_or(MEASURE));
    let required: Vec<&str> = o
        .inventory
        .iter()
        .filter(|row| row.required)
        .map(|row| row.label())
        .collect();
    if !required.is_empty() {
        rendered.push_str(&block(
            "",
            0,
            &[(&format!("read first: {}", required.join(", ")), None)],
            0,
            painter,
            width,
        ));
    }
    for row in &o.inventory {
        let mut facets = vec![
            row.relation
                .map(|relation| relation.as_str())
                .unwrap_or(row.origin.as_str())
                .to_string(),
        ];
        if row.required {
            facets.push("required".into());
        }
        if let Some(kind) = &row.kind {
            facets.push(format!("kind={kind}"));
        }
        let reference = if row
            .reference
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\\')
        {
            crate::frontmatter::quote(&row.reference)
        } else {
            row.reference.clone()
        };
        let mut text = format!("{} ({}): {reference}", row.label(), facets.join(", "));
        if row.status != crate::output::RowStatus::Present {
            text.push_str(&format!(" [{}]", row.status.as_str()));
        }
        if let Some(step) = &row.step {
            text.push_str(&format!(" (step: {step})"));
        }
        if let Some(description) = &row.description {
            text.push_str(&format!(" — {description}"));
        }
        rendered.push_str(&block("- ", 2, &[(&text, None)], 2, painter, width));
    }
    rendered
}

fn files(o: &ShowFields, painter: &Painter, width: Option<usize>) -> String {
    let mut rendered = present::rule("files", painter, width.unwrap_or(MEASURE));
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
    let mut rendered = present::rule(
        &format!("notes ({})", notes.len()),
        painter,
        width.unwrap_or(MEASURE),
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Complexity, HarnessProvenance, Process, Size, Status};
    use crate::output::{DepInfo, ParkInfo, Related};
    use crate::style::ColorMode;

    const BARE: &str = "---\nid: sci-000001\ntitle: Phone sheet hides Save\nstatus: doing\npriority: 1\ncreated: 2026-09-25T15:47:16Z\nupdated: 2026-10-02T08:03:41Z\ndepends: []\ntags: []\n---\n";

    #[test]
    fn a_word_longer_than_the_available_width_is_hard_split() {
        let chars: Vec<(char, Option<Style>)> = "abcdefghij".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 4);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn hard_split_does_not_join_the_previous_word() {
        let chars: Vec<(char, Option<Style>)> =
            "abcd bbbbbbbb".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 5);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["abcd", "bbbbb", "bbb"]);
    }

    #[test]
    fn a_break_drops_the_space_and_keeps_words_inline() {
        let chars: Vec<(char, Option<Style>)> = "ab cd ef".chars().map(|c| (c, None)).collect();
        let lines = wrap_lines(&chars, 6);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| line.iter().map(|(c, _)| c).collect())
            .collect();
        assert_eq!(texts, ["ab cd", "ef"]);
    }

    #[test]
    fn a_suffix_keeps_its_style_on_the_continuation_line() {
        let colored = Painter::new(ColorMode::Always, crate::output::Format::Pretty, false);
        let chars: Vec<(char, Option<Style>)> = "hello "
            .chars()
            .map(|c| (c, Some(Style::Chrome)))
            .chain("world".chars().map(|c| (c, Some(Style::Emphasis))))
            .collect();
        let lines = wrap_lines(&chars, 8);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(paint_runs(&lines[0], &colored), "\x1b[2mhello\x1b[0m");
        assert_eq!(paint_runs(&lines[1], &colored), "\x1b[1mworld\x1b[0m");
    }

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, crate::output::Format::Pretty, false)
    }

    fn fields() -> ShowFields {
        ShowFields {
            task: crate::format::parse_task(BARE, "x").unwrap(),
            project_colors: Default::default(),
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
            inventory: vec![],
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
            occurrence: Default::default(),
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
            note(
                "2026-09-25T19:30:15Z",
                "fix/sheet",
                "started",
                Some("codex:abc"),
            ),
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
        text.lines().find_map(|line| {
            line.strip_prefix(&prefix)
                .map(|rest| rest.trim().to_string())
        })
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
        assert_eq!(
            value(&text, "process").as_deref(),
            Some("unassessed"),
            "{text}"
        );
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
        let order: Vec<usize> = [
            "── overview",
            "── parked",
            "── description",
            "── related",
            "── notes",
        ]
        .iter()
        .map(|name| text.find(name).unwrap_or_else(|| panic!("{name}: {text}")))
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert!(
            text.contains("waiting on user, review since 2026-09-25\n    Check it on the phone.\n    session codex:abc in /w\n"),
            "{text}"
        );
        assert!(
            text.contains("parent      sci-000002  todo     Goal\n"),
            "{text}"
        );
        assert!(
            text.contains("depends on  fam-000003  ?        (unresolved)\n"),
            "{text}"
        );
    }

    #[test]
    fn recurrence_rows_cover_every_state() {
        let row = |every: &str,
                   due: Option<&str>,
                   due_now: bool,
                   last: Option<&str>,
                   completed: Option<&str>| {
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
        assert_eq!(
            every(row("30d", None, false, None, None)).as_deref(),
            Some("30d")
        );
        assert_eq!(
            every(row("2w", Some("2026-10-10T00:00:00Z"), false, None, None)).as_deref(),
            Some("2w, due 2026-10-10")
        );
        assert_eq!(
            every(row("30d", None, true, None, None)).as_deref(),
            Some("30d, due now")
        );
        let reopened = row("30d", None, false, Some("2026-09-01T00:00:00Z"), None);
        assert_eq!(
            value(&reopened, "last done").as_deref(),
            Some("2026-09-01"),
            "{reopened}"
        );
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
        assert_eq!(
            value(&text, "step").as_deref(),
            Some("Task 3: emit  MISSING"),
            "{text}"
        );
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
        let numbered = body
            .iter()
            .position(|line| line.starts_with("12. "))
            .unwrap();
        assert!(
            body[numbered + 1].starts_with("    word"),
            "{:?}",
            body[numbered + 1]
        );
        assert!(
            body.contains(&format!("    indented {long}").as_str()),
            "indented kept whole"
        );
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
        assert!(
            text.contains("2026-09-25 15:52  main\n    started the refactor\n"),
            "{text}"
        );
        assert!(
            text.contains("2026-09-25 15:53  main\n    parked the car\n"),
            "{text}"
        );
    }

    #[test]
    fn generated_markers_are_recognised_in_every_form() {
        for (text, marker, rest) in [
            ("resumed", "resumed", None),
            (
                "completed; next due 2026-11-02",
                "completed; next due 2026-11-02",
                None,
            ),
            (
                "parked (waiting on agent): go on",
                "parked (waiting on agent)",
                Some("go on"),
            ),
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
        assert!(
            lines[0].starts_with("2026-09-25 19:43  feat/a-rather-long"),
            "{notes}"
        );
        assert!(
            lines[1].starts_with("  ") && !lines[1].starts_with("   "),
            "{notes}"
        );
        assert!(
            notes.contains("codex:01a0da00-5aca-7b52-a31c-dd47fc2e25a5"),
            "{notes}"
        );
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
        assert!(
            text.lines().all(|line| line.chars().count() <= MEASURE),
            "{text}"
        );
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
        assert!(
            painted.contains("\x1b[1m── overview \x1b[0m"),
            "{painted:?}"
        );
        assert!(
            painted.contains("\x1b[2m(sci-000001)\x1b[0m"),
            "{painted:?}"
        );
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
