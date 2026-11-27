//! The pretty `show` layout (docs/user-guide.md, "Goals, lanes and dependencies"): ruled
//! sections, aligned overview rows, and prose wrapped at a capped measure. `next`
//! prints its task through the same renderer.

use crate::claims::Escalation;
use crate::model::{Note, Status};
use crate::output::{ActionExplanation, DepInfo, ParkInfo, Related, ShowFields};
use crate::palette::Rgb;
use crate::present::{self, Span};
use crate::style::{Painter, Style};
use std::collections::HashMap;
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

/// Prose never runs wider than this, however wide the terminal (docs/user-guide.md).
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
    if !o.decisions.is_empty() {
        sections.push(decisions(&o.decisions, painter, width));
    }
    if let Some(actions) = o.actions.as_ref().filter(|actions| !actions.is_empty()) {
        sections.push(format!(
            "{}{}",
            present::rule("actions", painter, width.unwrap_or(MEASURE)),
            action_lines(actions, painter, width)
        ));
    }
    sections.push(description(&o.task.body, painter, width));
    if let Some(links) = links(o, painter, width) {
        sections.push(links);
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

pub(crate) fn action_lines(
    actions: &[ActionExplanation],
    painter: &Painter,
    width: Option<usize>,
) -> String {
    let mut rendered = String::new();
    for action in actions {
        let availability = if action.available {
            "available"
        } else {
            "held"
        };
        let mut line = format!(
            "{} ({})  {availability}",
            action.action.as_str(),
            action.actor.as_str()
        );
        for reason in &action.reasons {
            line.push_str("  ");
            line.push_str(reason.code);
            for key in ["task", "resource", "host", "date", "status"] {
                if let Some(value) = reason.evidence[key].as_str() {
                    line.push(' ');
                    line.push_str(value);
                }
            }
        }
        rendered.push_str(&block("  ", 2, &[(&line, None)], 4, painter, width));
    }
    rendered
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
    // The tldr reads under the title, dimmed; `long_title` is kept for the record only.
    if let Some(tldr) = &o.task.tldr {
        let tldr = Span {
            text: tldr.clone(),
            style: Some(Style::Chrome),
        };
        rendered.push_str(&block("", 0, &spans(&[tldr]), 0, painter, width));
    }
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

/// Current contract (docs/user-guide.md, "Goals, lanes and dependencies"): one row per field with a value, in this
/// order. Links (`parent`, `depends on`, `follows`, `wake on`, `related`, and a task `source`) are listed under `── links ──`
/// instead.
fn overview_rows(o: &ShowFields) -> Vec<(&'static str, Vec<Span>)> {
    let t = &o.task;
    let mut rows = vec![
        ("status", vec![present::status(t.status)]),
        ("priority", vec![present::priority(t.priority)]),
    ];
    if let Some(to) = &t.moved_to {
        let to = to.to_string();
        rows.push(("moved to", vec![present::id(&to, o.moved_to_color)]));
    }
    if let Some(from) = &t.moved_from {
        let from = from.to_string();
        rows.push(("moved from", vec![present::id(&from, o.moved_from_color)]));
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
    rows.push(("updated", vec![present::stamp(&t.updated)]));
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
    if o.source_task.is_none()
        && let Some(source) = &t.source
    {
        rows.push(("source", vec![present::owner(source)]));
    }
    if let Some(agent) = &t.agent {
        rows.push(("agent", vec![present::owner(agent)]));
    }
    if let Some(origin) = t.origin {
        rows.push(("origin", vec![present::owner(origin.as_str())]));
    }
    if let Some(model) = &t.model {
        rows.push(("model", vec![present::owner(model)]));
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

fn decisions(
    items: &[crate::output::LogicalDecision],
    painter: &Painter,
    width: Option<usize>,
) -> String {
    let mut rendered = present::rule("decisions", painter, width.unwrap_or(MEASURE));
    for d in items {
        let mut head = format!(
            "[{}] {} {} ({} by {} at {})",
            d.state,
            d.data.kind.as_str(),
            d.id,
            d.data.origin,
            d.by,
            d.at
        );
        if d.scope_changed_since_open == Some(true) {
            head.push_str("  [scope changed]");
        }
        if !d.open_dependents.tasks.is_empty() {
            head.push_str(&format!(
                "  [{} open dep(s)]",
                d.open_dependents.tasks.len()
            ));
        } else if !d.open_dependents.complete {
            head.push_str("  [dependents coverage incomplete]");
        }
        if let Some(seen) = &d.seen_scope {
            let short = &seen[..seen.len().min(12)];
            head.push_str(&format!("  scope:{short}"));
        }
        rendered.push_str(&block("  ", 2, &[(&head, None)], 4, painter, width));
        for line in d.data.text.lines() {
            rendered.push_str(&block("    ", 4, &[(line, None)], 4, painter, width));
        }
        if let Some(res) = &d.resolution {
            let mut rline = format!(
                "resolved: {} by {} at {}",
                res.data.outcome.as_str(),
                res.by,
                res.at
            );
            if !res.data.text.is_empty() {
                rline.push_str(" — ");
                rline.push_str(&res.data.text);
            }
            rendered.push_str(&block(
                "    ",
                4,
                &[(&rline, Some(Style::Chrome))],
                4,
                painter,
                width,
            ));
        }
    }
    rendered
}

/// Current contract (docs/user-guide.md): each body line wraps on its own; list items hang; fenced and indented
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

struct LinkTarget<'a> {
    id: &'a str,
    title: &'a str,
    status: Option<Status>,
    updated: Option<&'a str>,
}

impl<'a> From<&'a DepInfo> for LinkTarget<'a> {
    fn from(row: &'a DepInfo) -> Self {
        Self {
            id: &row.canonical_id,
            title: row.title.as_deref().unwrap_or("(unresolved)"),
            status: row.status,
            updated: row.updated.as_deref(),
        }
    }
}

impl<'a> From<&'a Related> for LinkTarget<'a> {
    fn from(row: &'a Related) -> Self {
        Self {
            id: &row.id,
            title: &row.title,
            status: Some(row.status),
            updated: Some(&row.updated),
        }
    }
}

/// Visit roles in precedence order so the first occurrence sets each row's position.
fn link_rows(o: &ShowFields) -> Vec<(LinkTarget<'_>, Vec<&'static str>)> {
    let mut rows: Vec<(LinkTarget<'_>, Vec<&'static str>)> = Vec::new();
    let mut positions: HashMap<&str, usize> = HashMap::new();
    let mut add = |role, target| {
        let target: LinkTarget<'_> = target;
        if let Some(&position) = positions.get(target.id) {
            let (current, roles) = &mut rows[position];
            if current.status.is_none() && target.status.is_some() {
                *current = target;
            }
            if role != "related"
                && !roles.contains(&role)
                && !(role == "depends on" && roles.contains(&"piece"))
            {
                roles.push(role);
            }
        } else {
            positions.insert(target.id, rows.len());
            rows.push((target, vec![role]));
        }
    };
    if let Some(parent) = &o.parent {
        add("parent", parent.into());
    } else if let Some(id) = &o.unresolved_parent {
        add(
            "parent",
            LinkTarget {
                id,
                title: "(unresolved)",
                status: None,
                updated: None,
            },
        );
    }
    for row in o.depends_on.iter().filter(|row| row.piece) {
        add("piece", row.into());
    }
    for row in o.depends_on.iter().filter(|row| !row.piece) {
        add("depends on", row.into());
    }
    for row in &o.dependents {
        add("dependent", row.into());
    }
    for row in &o.wake_targets {
        add("wake on", row.into());
    }
    for row in &o.follows_tasks {
        add("follows", row.into());
    }
    for row in &o.followed_by {
        add("followed by", row.into());
    }
    for row in &o.children {
        add("child", row.into());
    }
    if let Some(row) = &o.source_task {
        add("source", row.into());
    }
    for row in &o.related_to {
        add("related", row.into());
    }
    rows
}

/// Render the five columns with independent hanging wraps for roles and titles.
fn links(o: &ShowFields, painter: &Painter, width: Option<usize>) -> Option<String> {
    let rows = link_rows(o);
    let id_width = rows.iter().map(|(target, _)| target.id.len()).max()?;
    let labels: Vec<_> = rows.iter().map(|(_, roles)| roles.join(", ")).collect();
    let label_width = labels.iter().map(String::len).max()?;
    let status_width = 7;
    let date_width = 10;
    let fixed_width = id_width + status_width + date_width + 8;
    let label_budget = width.map_or(20, |width| {
        width.saturating_sub(fixed_width + 10).clamp(1, 20)
    });
    let label_width = label_width.min(label_budget);
    let lead_width = label_width + fixed_width;
    let mut rendered = present::rule("links", painter, width.unwrap_or(MEASURE));
    for ((target, _), labels) in rows.into_iter().zip(labels) {
        let project = target.status.and_then(|_| color(o, target.id));
        let status = present::status_of(target.status);
        let date = target
            .updated
            .map_or_else(|| Span::plain(""), present::date);
        let identity = format!(
            "{}  {}  {}  ",
            present::id(target.id, project)
                .left(id_width)
                .paint(painter),
            status.left(status_width).paint(painter),
            date.left(date_width).paint(painter),
        );
        let labels: Vec<_> = labels.chars().map(|c| (c, Some(Style::Chrome))).collect();
        let label_lines = wrap_lines(&labels, label_width);
        let title_style = present::title(target.title, project, None).style;
        let title: Vec<_> = target.title.chars().map(|c| (c, title_style)).collect();
        let title_lines = match width {
            Some(width) => wrap_lines(&title, width.saturating_sub(lead_width).max(1)),
            None => vec![title],
        };
        for line in 0..label_lines.len().max(title_lines.len()) {
            if let Some(label) = label_lines.get(line) {
                rendered.push_str(&paint_runs(label, painter));
                if line == 0 || title_lines.get(line).is_some() {
                    rendered.push_str(&" ".repeat(label_width - columns(label)));
                }
            } else {
                rendered.push_str(&" ".repeat(label_width));
            }
            if line == 0 {
                rendered.push_str("  ");
                rendered.push_str(&identity);
            } else if title_lines.get(line).is_some() {
                rendered.push_str(&" ".repeat(fixed_width));
            }
            if let Some(title) = title_lines.get(line) {
                rendered.push_str(&paint_runs(title, painter));
            }
            rendered.push('\n');
        }
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

/// Note structure (docs/task-model.md): a header (date, time, owner, marker, session), then the text indented 4.
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
        for label in [
            note.occurrence
                .root_session
                .as_ref()
                .map(|session| format!("root session {session}")),
            note.occurrence
                .origin
                .map(|origin| format!("origin {}", origin.as_str())),
            note.occurrence
                .origin_session
                .as_ref()
                .map(|session| format!("origin session {session}")),
        ]
        .into_iter()
        .flatten()
        {
            header.extend([
                Span::plain("  "),
                Span {
                    text: label,
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

/// Splits a generated lifecycle marker from its text (docs/task-model.md). Only the complete
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
            actions: None,
            task: crate::history::fixture(BARE, "x").unwrap(),
            task_revision: "0".repeat(64),
            needs_review: false,
            review_reason: None,
            project_colors: Default::default(),
            moved_to_color: None,
            moved_from_color: None,
            spec_path: None,
            plan_path: None,
            step_found: None,
            depends_on: vec![],
            related_to: vec![],
            follows_tasks: vec![],
            dependents: vec![],
            followed_by: vec![],
            wake_targets: vec![],
            source_task: None,
            parent: None,
            unresolved_parent: None,
            in_lane: None,
            children: vec![],
            claim: None,
            park: None,
            escalation: None,
            periodic: None,
            deferred: None,
            files: vec![],
            multipart: vec![],
            inventory: vec![],
            decisions: vec![],
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

    /// A minimal task example.
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
            "updated:    2026-10-02T08:03:41Z\n".into(),
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
        for absent in ["── parked", "── links", "── files", "── notes", "size:"] {
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
            decision_id: None,
            session: "codex:abc".into(),
            owner: "main".into(),
            host: "h".into(),
            worktree: "/w".into(),
        });
        o.parent = Some(Related {
            id: "sci-000002".into(),
            title: "Goal".into(),
            status: Status::Todo,
            updated: "2026-10-02T00:00:00Z".into(),
        });
        o.depends_on = vec![DepInfo {
            id: "fam-000003".into(),
            canonical_id: "fam-000003".into(),
            title: None,
            status: None,
            updated: None,
            resolved: false,
            piece: false,
        }];
        let text = render(&o, &plain(), Some(120));
        let order: Vec<usize> = [
            "── overview",
            "── parked",
            "── description",
            "── links",
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
            text.contains("parent      sci-000002  todo     2026-10-02  Goal\n"),
            "{text}"
        );
        assert!(
            text.contains("depends on  fam-000003  ?                    (unresolved)\n"),
            "{text}"
        );
    }

    #[test]
    fn links_render_in_role_order_and_overview_omits_wake_and_task_source() {
        let mut o = fields();
        o.task.wake_on = vec![crate::model::TaskId::parse("sci-000003").unwrap()];
        o.task.source = Some("sci-000005".into());
        o.parent = Some(Related {
            id: "sci-000001".into(),
            title: "Parent".into(),
            status: Status::Todo,
            updated: "2026-01-01T00:00:00Z".into(),
        });
        o.depends_on = vec![DepInfo {
            id: "sci-000002".into(),
            canonical_id: "sci-000002".into(),
            title: Some("Dep".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-02T00:00:00Z".into()),
            resolved: true,
            piece: false,
        }];
        o.follows_tasks = vec![DepInfo {
            id: "sci-000002".into(),
            canonical_id: "sci-000002".into(),
            title: Some("Dep".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-02T00:00:00Z".into()),
            resolved: true,
            piece: false,
        }];
        o.wake_targets = vec![DepInfo {
            id: "sci-000003".into(),
            canonical_id: "sci-000003".into(),
            title: Some("Wake".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-03T00:00:00Z".into()),
            resolved: true,
            piece: false,
        }];
        o.related_to = vec![DepInfo {
            id: "sci-000004".into(),
            canonical_id: "sci-000004".into(),
            title: Some("Rel".into()),
            status: Some(Status::Done),
            updated: Some("2026-01-04T00:00:00Z".into()),
            resolved: true,
            piece: false,
        }];
        o.source_task = Some(DepInfo {
            id: "sci-000005".into(),
            canonical_id: "sci-000005".into(),
            title: Some("Source".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-05T00:00:00Z".into()),
            resolved: true,
            piece: false,
        });
        o.children = vec![Related {
            id: "sci-000006".into(),
            title: "Child".into(),
            status: Status::Todo,
            updated: "2026-01-06T00:00:00Z".into(),
        }];

        let text = render(&o, &plain(), Some(120));
        assert!(!text.contains("wake on:"), "{text}");
        assert!(!text.contains("source:"), "{text}");

        let links = text.split_once("── links ").unwrap().1;
        let lines: Vec<_> = links
            .lines()
            .skip(1)
            .take_while(|l| !l.is_empty())
            .collect();
        let expected = [
            ("parent", "sci-000001  todo     2026-01-01  Parent"),
            (
                "depends on, follows",
                "sci-000002  todo     2026-01-02  Dep",
            ),
            ("wake on", "sci-000003  todo     2026-01-03  Wake"),
            ("child", "sci-000006  todo     2026-01-06  Child"),
            ("source", "sci-000005  todo     2026-01-05  Source"),
            ("related", "sci-000004  done     2026-01-04  Rel"),
        ];
        assert_eq!(
            lines,
            expected.map(|(role, target)| format!("{role:<19}  {target}")),
            "{links}"
        );

        // When source is opaque (source_task is None), it stays in overview.
        o.source_task = None;
        o.task.source = Some("https://example.com/item".into());
        let text_url = render(&o, &plain(), Some(120));
        assert!(
            value(&text_url, "source").as_deref() == Some("https://example.com/item"),
            "{text_url}"
        );
    }

    #[test]
    fn pretty_links_wrap_roles_and_full_titles_in_their_own_columns() {
        let mut o = fields();
        let id = "longname-000002";
        let title = "Alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let row = || DepInfo {
            id: id.into(),
            canonical_id: id.into(),
            title: Some(title.into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-01T00:00:00Z".into()),
            resolved: true,
            piece: false,
        };
        o.depends_on = vec![
            DepInfo {
                piece: true,
                ..row()
            },
            row(),
        ];
        o.wake_targets = vec![row()];
        o.follows_tasks = vec![row()];
        o.source_task = Some(row());
        o.related_to = vec![row()];

        let text = render(&o, &plain(), Some(80));
        assert_eq!(text.matches(id).count(), 1, "{text}");
        let links = text.split_once("── links ").unwrap().1;
        let rows: Vec<_> = links
            .lines()
            .skip(1)
            .take_while(|line| !line.is_empty())
            .collect();
        let title_start = rows[0].find("Alpha").unwrap();
        assert!(title_start <= 60, "{links}");
        assert!(rows.iter().all(|row| row.len() <= 80), "{links}");
        let roles = rows
            .iter()
            .map(|row| row[..20].trim_end())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            roles.trim_end(),
            "piece, wake on, follows, source",
            "{links}"
        );
        let wrapped_title = rows
            .iter()
            .map(|row| &row[title_start..])
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(wrapped_title, title);
    }

    #[test]
    fn follows_reader_and_related_rows_wrap_and_paint_dates() {
        for follows in [false, true] {
            let mut o = fields();
            o.parent = Some(Related {
                id: "sci-000001".into(),
                title: "Alpha beta gamma delta".into(),
                status: Status::Todo,
                updated: "2026-09-25T12:00:00Z".into(),
            });
            o.related_to = vec![
                DepInfo {
                    id: "fam-000002".into(),
                    canonical_id: "fam-000002".into(),
                    title: Some("Old target".into()),
                    status: Some(Status::Done),
                    updated: Some("2020-01-01T00:00:00Z".into()),
                    resolved: true,
                    piece: false,
                },
                DepInfo {
                    id: "fam-000003".into(),
                    canonical_id: "fam-000003".into(),
                    title: None,
                    status: None,
                    updated: None,
                    resolved: false,
                    piece: false,
                },
            ];

            if follows {
                o.follows_tasks = std::mem::take(&mut o.related_to);
            }
            let label = if follows { "follows" } else { "related" };
            // label_width = 7 ("related"), id_width = 10, status_width = 7, date_width = 10 -> lead_width = 42.
            let text = render(&o, &plain(), Some(56));
            let related_block = text.split_once("── links ").unwrap().1;
            let lines: Vec<&str> = related_block
                .lines()
                .skip(1)
                .take_while(|l| !l.is_empty())
                .collect();
            assert_eq!(
                lines,
                vec![
                    "parent   sci-000001  todo     2026-09-25  Alpha beta".to_string(),
                    "                                          gamma delta".to_string(),
                    format!("{label}  fam-000002  done     2020-01-01  Old target"),
                    format!("{label}  fam-000003  ?                    (unresolved)"),
                ],
                "{related_block}"
            );

            let palette = crate::palette::Palette {
                fg: Rgb {
                    r: 229,
                    g: 227,
                    b: 215,
                },
                bg: Rgb {
                    r: 19,
                    g: 20,
                    b: 13,
                },
                cyan: Rgb {
                    r: 0,
                    g: 215,
                    b: 255,
                },
                magenta: None,
            };
            let today = time::Date::from_calendar_date(2026, time::Month::September, 25).unwrap();
            let painter = Painter::new(ColorMode::Always, crate::output::Format::Pretty, false)
                .with_recency(crate::style::Recency::new(today, &palette));
            let painted = render(&o, &painter, Some(120));
            assert!(
                painted.contains("\x1b[38;2;0;215;255m2026-09-25\x1b[0m  Alpha beta gamma delta"),
                "{painted:?}"
            );
            assert!(
                painted.contains("\x1b[38;2;125;125;115m2020-01-01\x1b[0m  Old target"),
                "{painted:?}"
            );
            assert!(
                painted.contains(
                    "\x1b[2mfam-000003\x1b[0m  \x1b[2m?      \x1b[0m              (unresolved)"
                ),
                "{painted:?}"
            );
        }
    }

    #[test]
    fn follows_inverse_rows_order_color_dates_and_wrap() {
        let mut o = fields();
        o.source_task = Some(DepInfo {
            id: "sci-000001".into(),
            canonical_id: "sci-000001".into(),
            title: Some("Source".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-01T00:00:00Z".into()),
            resolved: true,
            piece: false,
        });
        o.followed_by = vec![Related {
            id: "fam-000002".into(),
            title: "Alpha beta gamma delta epsilon".into(),
            status: Status::Done,
            updated: "2020-01-01T00:00:00Z".into(),
        }];
        o.children = vec![Related {
            id: "sci-000003".into(),
            title: "Child".into(),
            status: Status::Todo,
            updated: "2026-01-03T00:00:00Z".into(),
        }];
        o.project_colors.insert(
            "fam".into(),
            Rgb {
                r: 51,
                g: 102,
                b: 153,
            },
        );
        let text = render(&o, &plain(), Some(56));
        let block = text.split_once("── links ").unwrap().1;
        assert!(block.find("child").unwrap() < block.find("source").unwrap());
        assert!(block.find("followed by").unwrap() < block.find("child").unwrap());
        let lines: Vec<_> = block
            .lines()
            .skip(1)
            .take_while(|line| !line.is_empty())
            .collect();
        assert!(lines.len() >= 5, "{lines:?}");
        assert!(
            lines.iter().all(|line| line.chars().count() <= 56),
            "{lines:?}"
        );
        assert!(block.contains("2020-01-01"));
        let painter = Painter::new(ColorMode::Always, crate::output::Format::Pretty, false);
        let painted = render(&o, &painter, Some(120));
        assert!(painted.contains("fam-000002\x1b[0m"), "{painted:?}");
    }

    #[test]
    fn dependent_rows_precede_followed_by_children_and_source() {
        let mut o = fields();
        let row = |id: &str, title: &str| Related {
            id: id.into(),
            title: title.into(),
            status: Status::Todo,
            updated: "2026-01-02T00:00:00Z".into(),
        };
        o.source_task = Some(DepInfo {
            id: "sci-000001".into(),
            canonical_id: "sci-000001".into(),
            title: Some("Source".into()),
            status: Some(Status::Todo),
            updated: Some("2026-01-01T00:00:00Z".into()),
            resolved: true,
            piece: false,
        });
        o.dependents = vec![row("fam-000004", "Hub")];
        o.followed_by = vec![row("sci-000002", "Later")];
        o.children = vec![row("sci-000003", "Child")];
        let text = render(&o, &plain(), Some(120));
        let block = text.split_once("── links ").unwrap().1;
        let at = |label: &str| {
            block
                .find(label)
                .unwrap_or_else(|| panic!("{label}: {block}"))
        };
        assert!(at("dependent  ") < at("followed by"));
        assert!(at("followed by") < at("child"));
        assert!(at("child") < at("source"));
        assert!(block.contains("fam-000004"), "{block}");
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

    #[test]
    fn show_pretty_puts_tldr_under_title() {
        let mut o = fields();
        o.task.tldr = Some("Save hides behind the keyboard".into());
        let text = render(&o, &plain(), Some(80));
        let lines: Vec<&str> = text.lines().collect();
        let title = lines
            .iter()
            .position(|line| line.starts_with("Phone sheet hides Save"))
            .unwrap();
        assert_eq!(lines[title + 1], "Save hides behind the keyboard", "{text}");
        assert!(lines[title + 2].starts_with("status:"), "{text}");
        let colored = Painter::new(ColorMode::Always, crate::output::Format::Pretty, false);
        let painted = render(&o, &colored, Some(80));
        assert!(
            painted.contains("\x1b[2mSave hides behind the keyboard\x1b[0m"),
            "{painted:?}"
        );
    }

    #[test]
    fn show_pretty_without_tldr_unchanged() {
        let mut o = fields();
        let before = render(&o, &plain(), Some(80));
        o.task.long_title = Some("A long sentence that pretty views never print".into());
        assert_eq!(render(&o, &plain(), Some(80)), before);
        assert!(!before.contains("never print"));
    }
}
