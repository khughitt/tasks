//! The goals report's counters, title clipping and responsive row details.
use super::GoalsOut;
use crate::ellipsis::{Measure, clip};
use crate::goals::{GoalRow, GoalSort};
use crate::present;
use crate::style::{Painter, Style};
use crate::table::{self, Column, Detail, Layout, Row};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Id,
    Status,
    Priority,
    Work,
    Ideas,
    Deps,
    Unresolved,
    Active,
    Title,
    Attention,
}

impl Field {
    const ALL: [Self; 10] = [
        Self::Id,
        Self::Status,
        Self::Priority,
        Self::Work,
        Self::Ideas,
        Self::Deps,
        Self::Unresolved,
        Self::Active,
        Self::Title,
        Self::Attention,
    ];
    fn header(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Status => "status",
            Self::Priority => "P",
            Self::Work => "work",
            Self::Ideas => "ideas",
            Self::Deps => "deps",
            Self::Unresolved => "unres",
            Self::Active => "active",
            Self::Title => "title",
            Self::Attention => "attention",
        }
    }
    fn column(self) -> Column {
        match self {
            Self::Work | Self::Ideas | Self::Deps | Self::Unresolved => {
                Column::right(self.header())
            }
            _ => Column::fixed(self.header()),
        }
    }
}

fn attention(row: &GoalRow, painter: &Painter, now: time::OffsetDateTime) -> String {
    let Some(flags) = &row.attention else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(flag) = &flags.stalled {
        let since =
            crate::time::parse(&flag.since).expect("since was parsed when the flag was built");
        let days = (now - since).whole_days();
        parts.push(painter.paint(
            Style::Error,
            &format!("{} stalled ({}, {days}d)", flag.count, flag.first),
        ));
    }
    if let Some(flag) = &flags.blocked {
        let lane = if flags.paused_lane.as_deref() == Some(flag.first.as_str()) {
            ", paused lane"
        } else {
            ""
        };
        parts.push(painter.paint(
            Style::Error,
            &format!("{} blocked ({}{lane})", flag.count, flag.first),
        ));
    }
    if let Some(flag) = &flags.you {
        parts.push(painter.paint(
            Style::Emphasis,
            &format!("{} you ({})", flag.count, flag.first),
        ));
    }
    parts.join(" · ")
}

fn cells(row: &GoalRow, painter: &Painter, now: time::OffsetDateTime) -> Vec<String> {
    let work = if row.pieces.total == 0 {
        "-".into()
    } else {
        format!("{}/{}", row.pieces.done, row.pieces.total)
    };
    let ideas = if row.pieces.ideas == 0 {
        "-".into()
    } else {
        format!("+{}", row.pieces.ideas)
    };
    let deps = row.dependencies.as_ref().map_or_else(
        || "-".into(),
        |d| {
            let satisfied = d
                .total
                .checked_sub(d.open)
                .and_then(|n| n.checked_sub(d.unresolved))
                .expect("dependency outcomes partition total");
            format!("{satisfied}/{}", d.total)
        },
    );
    let unresolved = match row.dependencies.as_ref().map(|d| d.unresolved) {
        Some(n) if n > 0 => format!("?{n}"),
        _ => "-".into(),
    };
    let mut values = vec![
        present::id(&row.id, row.project_color).paint(painter),
        present::status(row.status).paint(painter),
        present::priority(row.priority).paint(painter),
        work,
        ideas,
        deps,
        unresolved,
        present::date(row.closed.as_deref().unwrap_or(&row.last_activity)).paint(painter),
        row.title.clone(),
        attention(row, painter, now),
    ];
    if row.closed.is_some() {
        // Closed records carry no attention and use the same dim style as the divider.
        values = vec![
            row.id.clone(),
            row.status.as_str().into(),
            format!("P{}", row.priority),
            values[3].clone(),
            values[4].clone(),
            values[5].clone(),
            values[6].clone(),
            crate::time::day(
                row.closed
                    .as_deref()
                    .expect("closed row has a closure date"),
            )
            .into(),
            row.title.clone(),
            String::new(),
        ];
        for field in Field::ALL {
            if field != Field::Title {
                values[field as usize] = painter.paint(Style::Chrome, &values[field as usize]);
            }
        }
    }
    values
}

fn paragraph(text: &str, painter: &Painter, layout: &Layout) -> String {
    table::render(
        &[Column::wrap("", 1, None)],
        &[Row::new(vec![painter.paint(Style::Chrome, text)])],
        &Layout {
            headers: false,
            ..*layout
        },
        None,
        painter,
    )
}

pub(super) fn render(out: &GoalsOut, painter: &Painter, layout: &Layout) -> String {
    if out.goals.is_empty() {
        return "no goals you asked for; goals without an origin are not shown (tasks edit <id> --origin user)".into();
    }
    let now = crate::time::parse(&out.observed_at).expect("observed_at is a stamp this run wrote");
    let data: Vec<_> = out.goals.iter().map(|r| cells(r, painter, now)).collect();
    let has_ideas = out.goals.iter().any(|r| r.pieces.ideas > 0);
    let has_deps = out.goals.iter().any(|r| r.dependencies.is_some());
    let has_unresolved = out
        .goals
        .iter()
        .any(|r| r.dependencies.as_ref().is_some_and(|d| d.unresolved > 0));
    let has_attention = out
        .goals
        .iter()
        .any(|r| r.attention.as_ref().is_some_and(|a| !a.is_empty()));
    let mut fields: Vec<_> = Field::ALL
        .into_iter()
        .filter(|f| match f {
            Field::Ideas => has_ideas,
            Field::Deps => has_deps,
            Field::Unresolved => has_unresolved,
            Field::Attention => has_attention,
            _ => true,
        })
        .collect();
    let widths: Vec<_> = Field::ALL
        .into_iter()
        .map(|f| {
            data.iter()
                .map(|c| table::visible_width(&c[f as usize]))
                .chain(layout.headers.then(|| f.header().len()))
                .max()
                .expect("report has rows")
        })
        .collect();
    let min_title = widths[Field::Title as usize].min(20);
    let needed = |fields: &[Field]| {
        fields
            .iter()
            .map(|f| {
                if *f == Field::Title {
                    min_title
                } else {
                    widths[*f as usize]
                }
            })
            .sum::<usize>()
            + 2 * fields.len().saturating_sub(1)
    };
    let mut title_budget = 80;
    if let Some(width) = layout.width {
        for group in [
            &[Field::Attention][..],
            &[Field::Ideas, Field::Deps, Field::Unresolved],
            &[Field::Status, Field::Priority, Field::Active],
            &[Field::Work],
        ] {
            if needed(&fields) <= width {
                break;
            }
            fields.retain(|f| !group.contains(f));
        }
        let fixed = fields
            .iter()
            .filter(|f| **f != Field::Title)
            .map(|f| widths[*f as usize])
            .sum::<usize>()
            + 2 * fields.len().saturating_sub(1);
        title_budget = width.saturating_sub(fixed).min(80);
        if title_budget
            < if layout.headers {
                Field::Title.header().len()
            } else {
                1
            }
        {
            fields.retain(|f| *f != Field::Title);
        }
    }
    let orientation = match (out.order.sort, out.order.reverse) {
        (GoalSort::Project, false) => "project A–Z, then activity newest first",
        (GoalSort::Project, true) => "project Z–A, then activity oldest first",
        (GoalSort::Activity, false) => "activity newest first",
        (GoalSort::Activity, true) => "activity oldest first",
        (GoalSort::Priority, false) => "priority P0 first, then activity newest first",
        (GoalSort::Priority, true) => "priority P4 first, then activity oldest first",
    };
    let mut text = paragraph(&format!("sort: {orientation}"), painter, layout);
    if out.include_closed {
        text.push_str(&paragraph("closed rows use closure date", painter, layout));
    }
    let mut legend = vec!["work: done/total descendants"];
    if has_ideas {
        legend.push("+ideas: uncommitted");
    }
    if has_deps {
        legend.push("deps: satisfied/total outside goal");
    }
    if has_unresolved {
        legend.push("?unresolved: unknown targets");
    }
    text.push_str(&paragraph(&legend.join("; "), painter, layout));

    let mut open = Vec::new();
    let mut closed = Vec::new();
    for (goal, mut values) in out.goals.iter().zip(data) {
        let title = clip(
            &goal.title,
            if fields.contains(&Field::Title) {
                title_budget
            } else {
                80
            },
            Measure::DisplayCells,
        );
        values[Field::Title as usize] = if goal.closed.is_some() {
            painter.paint(Style::Chrome, &title)
        } else {
            present::title(&title, goal.project_color, goal.activity).paint(painter)
        };
        let mut row = Row::new(fields.iter().map(|f| values[*f as usize].clone()).collect());
        let mut progress = Vec::new();
        if !fields.contains(&Field::Work) {
            progress.push(format!("work: {}", values[Field::Work as usize]));
        }
        if has_ideas && !fields.contains(&Field::Ideas) && goal.pieces.ideas > 0 {
            progress.push(format!("ideas: +{}", goal.pieces.ideas));
        }
        if has_deps && !fields.contains(&Field::Deps) && goal.dependencies.is_some() {
            progress.push(format!("deps: {} satisfied", values[Field::Deps as usize]));
        }
        if has_unresolved
            && !fields.contains(&Field::Unresolved)
            && goal.dependencies.as_ref().is_some_and(|d| d.unresolved > 0)
        {
            progress.push(format!(
                "unresolved: {}",
                values[Field::Unresolved as usize]
            ));
        }
        if !progress.is_empty() {
            row.details
                .push(("progress", Detail::Wrap(progress.join("; "))));
        }
        if !fields.contains(&Field::Status) {
            row.details.push((
                "state",
                Detail::Wrap(format!(
                    "status: {}; priority: P{}; active: {}",
                    goal.status.as_str(),
                    goal.priority,
                    crate::time::day(goal.closed.as_deref().unwrap_or(&goal.last_activity))
                )),
            ));
        }
        if !fields.contains(&Field::Title) {
            row.details
                .push(("title", Detail::Wrap(values[Field::Title as usize].clone())));
        }
        if has_attention
            && !fields.contains(&Field::Attention)
            && goal.attention.as_ref().is_some_and(|a| !a.is_empty())
        {
            row.details.push((
                "attention",
                Detail::Wrap(values[Field::Attention as usize].clone()),
            ));
        }
        if goal.closed.is_some() {
            closed.push(row);
        } else {
            open.push(row);
        }
    }
    let columns: Vec<_> = fields.iter().map(|f| f.column()).collect();
    let shared = table::shared_widths(&columns, &[&open, &closed], layout.headers);
    text.push_str(&table::render(
        &columns,
        &open,
        layout,
        Some(&shared),
        painter,
    ));
    if !closed.is_empty() {
        text.push_str(&paragraph(
            "── closed in the last 14 days ──",
            painter,
            layout,
        ));
        text.push_str(&table::render(
            &columns,
            &closed,
            &Layout {
                headers: layout.headers && open.is_empty(),
                ..*layout
            },
            Some(&shared),
            painter,
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goals::{Attention, Dependencies, Flag, GoalOrder, GoalRow, GoalSort, Pieces};
    use crate::model::{Origin, Status};
    use crate::output::{Format, ParkEvidence};
    use crate::style::ColorMode;

    fn painter(color: bool) -> Painter {
        Painter::new(
            if color {
                ColorMode::Always
            } else {
                ColorMode::Never
            },
            Format::Pretty,
            false,
        )
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
    fn report_fixture() -> GoalsOut {
        let mut rows = Vec::new();
        for (id, status, done, total, ideas, deps, title) in [
            (
                "material-6062fd",
                Status::Doing,
                147,
                205,
                24,
                Some(Dependencies {
                    open: 4,
                    total: 17,
                    unresolved: 2,
                }),
                "Keep the multi-project goals report readable at ordinary terminal widths and retain each progress value",
            ),
            (
                "material-5d6b2c",
                Status::Todo,
                48,
                55,
                5,
                Some(Dependencies {
                    open: 0,
                    total: 3,
                    unresolved: 0,
                }),
                "Resource-aware rendering",
            ),
            (
                "tasks-82e836",
                Status::Doing,
                0,
                4,
                0,
                None,
                "Improve goals output sorting and progress readability",
            ),
        ] {
            rows.push(GoalRow {
                id: id.into(),
                title: title.into(),
                status,
                origin: Some(Origin::User),
                priority: 1,
                lane: false,
                project_color: crate::palette::Rgb::parse_hex("#aa8c0e"),
                activity: None,
                last_activity: "2026-11-08T10:00:00Z".into(),
                pieces: Pieces { done, total, ideas },
                dependencies: deps,
                attention: None,
                closed: None,
            });
        }
        rows[0].attention = Some(Attention {
            stalled: Some(Flag {
                count: 1,
                first: "material-6062fd".into(),
                since: "2026-11-05T10:00:00Z".into(),
            }),
            blocked: Some(Flag {
                count: 2,
                first: "material-2834d7".into(),
                since: "2026-11-06T10:00:00Z".into(),
            }),
            you: Some(Flag {
                count: 1,
                first: "material-2502a2".into(),
                since: "2026-11-07T10:00:00Z".into(),
            }),
            paused_lane: None,
        });
        GoalsOut {
            include_closed: false,
            observed_at: "2026-11-08T12:00:00Z".into(),
            park_evidence: ParkEvidence {
                host: "fixture".into(),
                scope: "host_local",
            },
            goals: rows,
            warnings: vec![],
            order: GoalOrder::default(),
        }
    }
    fn at(width: usize, headers: bool) -> Layout {
        Layout {
            width: Some(width),
            headers,
        }
    }
    fn check_width(text: &str, width: usize) {
        for line in text.lines() {
            assert!(
                crate::table::visible_width(line) <= width,
                "{} > {width}: {line:?}",
                crate::table::visible_width(line)
            );
        }
    }
    #[test]
    fn goals_80_columns_relocates_secondary_counters() {
        for headers in [false, true] {
            for color in [false, true] {
                let text = render(&report_fixture(), &painter(color), &at(80, headers));
                check_width(&text, 80);
                let text = strip(&text);
                assert!(
                    text.contains("147/205") && text.contains("ideas: +24"),
                    "{text}"
                );
                assert!(
                    text.contains("deps: 11/17 satisfied") && text.contains("unresolved: ?2"),
                    "{text}"
                );
                assert!(
                    text.lines()
                        .any(|l| l.starts_with("material-6062fd") && l.contains("147/205")),
                    "{text}"
                );
                assert!(text.contains("material-2502a2"));
                assert!(text.find("sort:").unwrap() < text.find("material-6062fd").unwrap());
            }
        }
    }
    #[test]
    fn goals_120_and_160_fit_more_inline_data() {
        for width in [120, 160] {
            let text = render(&report_fixture(), &painter(false), &at(width, true));
            check_width(&text, width);
            let main = text
                .lines()
                .find(|l| l.starts_with("material-6062fd"))
                .unwrap();
            assert!(
                main.contains("+24") && main.contains("11/17") && main.contains("?2"),
                "{text}"
            );
            assert!(
                text.lines()
                    .any(|l| l.contains("unres") && l.contains("title")),
                "{text}"
            );
        }
    }
    #[test]
    fn goals_dependencies_show_completed_progress() {
        let mut out = report_fixture();
        out.goals[0].dependencies = Some(Dependencies {
            open: 2,
            total: 5,
            unresolved: 1,
        });
        let text = render(&out, &painter(false), &at(120, false));
        let main = text
            .lines()
            .find(|l| l.starts_with("material-6062fd"))
            .unwrap();
        assert!(main.contains("2/5") && main.contains("?1"), "{text}");
        assert!(
            text.lines()
                .find(|l| l.starts_with("material-5d6b2c"))
                .unwrap()
                .contains("3/3")
        );
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["goals"][0]["dependencies"]["open"], 2);
        assert!(json.get("order").is_none());
    }
    #[test]
    fn goals_closed_only_headers_and_shared_widths() {
        let mut out = report_fixture();
        out.include_closed = true;
        for row in &mut out.goals {
            row.closed = Some("2026-11-07T00:00:00Z".into());
            row.status = Status::Done;
            row.attention = None;
        }
        let text = render(&out, &painter(false), &at(80, true));
        check_width(&text, 80);
        assert_eq!(
            text.lines()
                .filter(|l| l.contains("status") && l.contains("title"))
                .count(),
            1,
            "{text}"
        );
        assert!(text.contains("closure"), "{text}");
        assert!(text.contains("2026-11-07"));
        // Categories found only in closed rows still belong in the legend.
        out.goals[0].closed = None;
        out.goals[0].pieces.ideas = 0;
        out.goals[0].dependencies = None;
        let text = render(&out, &painter(false), &at(80, false));
        assert!(
            text.split("material-6062fd")
                .next()
                .unwrap()
                .contains("+ideas"),
            "{text}"
        );
    }
    #[test]
    fn goals_titles_unicode_and_no_width() {
        let mut out = report_fixture();
        out.goals[0].title = "界e\u{301}👩‍💻".repeat(50);
        for width in [60, 80, 120, 160] {
            let plain = render(&out, &painter(false), &at(width, false));
            let colored = render(&out, &painter(true), &at(width, false));
            check_width(&plain, width);
            assert_eq!(strip(&colored), plain);
            assert!(plain.contains('…'));
        }
        let plain = render(&out, &painter(false), &Layout::default());
        let line = plain
            .lines()
            .find(|l| l.starts_with("material-6062fd"))
            .unwrap();
        assert!(
            line.contains("+24") && line.contains("11/17") && line.contains("material-2502a2"),
            "{plain}"
        );
        assert!(line.contains('…'));
        out.goals[0].title = "Short".into();
        assert!(
            !render(&out, &painter(false), &Layout::default())
                .lines()
                .find(|l| l.starts_with("material-6062fd"))
                .unwrap()
                .contains('…')
        );
    }
    #[test]
    fn goals_orientation_names_every_selected_order() {
        for (sort, reverse, expected) in [
            (
                GoalSort::Project,
                false,
                "project A–Z, then activity newest first",
            ),
            (
                GoalSort::Project,
                true,
                "project Z–A, then activity oldest first",
            ),
            (GoalSort::Activity, false, "activity newest first"),
            (GoalSort::Activity, true, "activity oldest first"),
            (
                GoalSort::Priority,
                false,
                "priority P0 first, then activity newest first",
            ),
            (
                GoalSort::Priority,
                true,
                "priority P4 first, then activity oldest first",
            ),
        ] {
            let mut out = report_fixture();
            out.order = GoalOrder { sort, reverse };
            assert!(
                render(&out, &painter(false), &Layout::default())
                    .starts_with(&format!("sort: {expected}"))
            );
        }
    }
    #[test]
    fn goals_render_examples() {
        for width in [80, 120, 160] {
            for headers in [false, true] {
                for color in [false, true] {
                    let text = render(&report_fixture(), &painter(color), &at(width, headers));
                    check_width(&text, width);
                    println!("CASE width={width} cells; headers={headers}; color={color}\n{text}");
                }
            }
        }
    }
}
