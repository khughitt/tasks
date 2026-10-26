//! Pretty `tasks blockers`: a heading naming the dependent scope and coverage verdict, the
//! ranked table with continuation lines, the diagnostics, and a footer stating the
//! report's limits (blocker-ranking spec, initial output contract).

use crate::blockers::{CoverageMode, WorkByStatus};
use crate::output::{Attention, BlockerRow, BlockersOut};
use crate::present;
use crate::style::{Painter, Style};
use crate::table::{Column, Detail, Layout, Row};

const COLUMNS: [Column; 8] = [
    Column::right("rank"),
    Column::fixed("id"),
    Column::fixed("P"),
    Column::fixed("status"),
    Column::right("trans"),
    Column::right("direct"),
    Column::fixed("own"),
    Column::wrap("title", 20, None),
];

pub fn render(out: &BlockersOut, painter: &Painter, layout: &Layout) -> String {
    let coverage = &out.coverage;
    let scope = match coverage.mode {
        CoverageMode::Local => "local",
        CoverageMode::Project => "project",
        CoverageMode::Group => "group",
        CoverageMode::AllProjects => "all projects",
    };
    let verdict = if coverage.complete {
        "coverage complete"
    } else {
        "coverage partial"
    };
    let mut text = format!(
        "{}; {}\n",
        present::heading(
            &format!(
                "blockers: dependents in {scope} ({})",
                coverage.requested_projects.join(", ")
            ),
            None
        )
        .paint(painter),
        painter.paint(
            if coverage.complete {
                Style::Ok
            } else {
                Style::Error
            },
            verdict
        )
    );
    text.push_str(&painter.paint(
        Style::Chrome,
        &format!(
            "{} of {} by transitive work, direct work, priority, created, id; observed {}",
            out.shown, out.total, out.observed_at
        ),
    ));
    text.push('\n');
    let rows: Vec<Row> = out
        .blockers
        .iter()
        .map(|row| table_row(row, painter))
        .collect();
    text.push_str(&crate::table::render(
        &COLUMNS, &rows, layout, None, painter,
    ));

    if !out.unresolved.is_empty() {
        text.push_str(&format!("\nunresolved ({}):\n", out.unresolved.len()));
        for diagnostic in &out.unresolved {
            let canonical = diagnostic
                .canonical
                .as_ref()
                .map(|id| format!(" ({id})"))
                .unwrap_or_default();
            text.push_str(&format!(
                "  {} -> {}{canonical}: {}\n",
                diagnostic.dependent, diagnostic.reference, diagnostic.reason
            ));
        }
    }
    if !out.cycles.is_empty() {
        text.push_str(&format!("\ncycles ({}):\n", out.cycles.len()));
        for component in &out.cycles {
            let ids: Vec<String> = component.iter().map(ToString::to_string).collect();
            text.push_str(&format!("  {}\n", ids.join(", ")));
        }
    }

    let mut footer = vec![
        "goals: structural only (lanes and records with children); a childless cross-project \
         closeout hub counts as work"
            .to_string(),
        "a goal's impact is not attributed to a goal's children without an explicit dependency"
            .to_string(),
        format!(
            "parks and claims: host-local evidence from {}; absence is unknown, not free or \
             authorized",
            out.park_evidence.host
        ),
        "history: blocking since and confirmed scope passes are unknown".to_string(),
        "cleared counts dependency sets only; it does not mean ready or authorized".to_string(),
    ];
    if !coverage.complete {
        footer.push(
            "partial: counts are lower bounds and ranks may change when missing coverage \
             returns; see warnings"
                .to_string(),
        );
    }
    text.push('\n');
    for line in footer {
        text.push_str(&painter.paint(Style::Chrome, &line));
        text.push('\n');
    }
    text
}

fn table_row(row: &BlockerRow, painter: &Painter) -> Row {
    let mut built = Row::new(vec![
        row.rank.to_string(),
        present::id(&row.id, row.project_color).paint(painter),
        present::priority(row.priority).paint(painter),
        present::status(row.status).paint(painter),
        row.transitive_work_count.to_string(),
        row.direct_work_count.to_string(),
        format!(
            "{}/{}",
            row.unmet_dependency_count, row.unresolved_dependency_count
        ),
        present::title(&row.title, row.project_color).paint(painter),
    ]);
    let attention = match row.attention {
        Attention::User => "user",
        Attention::Unknown => "unknown",
    };
    built.details.push((
        "impact",
        Detail::Wrap(format!(
            "cleared {}; goals {}/{}; attention {attention}; direct {}; transitive {}",
            row.dependencies_cleared_direct_count,
            row.direct_goal_count,
            row.transitive_goal_count,
            breakdown(&row.direct_work_by_status),
            breakdown(&row.transitive_work_by_status),
        )),
    ));
    if row.cycle {
        built
            .details
            .push(("cycle", Detail::Wrap("investigate cycle".into())));
    }
    match &row.park {
        Some(park) if row.attention == Attention::User => {
            let why = park.reason.map_or("waiting", crate::claims::Reason::as_str);
            built
                .details
                .push(("user", Detail::Wrap(format!("{why}: {}", park.next_step))));
        }
        Some(park) => built.details.push((
            "park",
            Detail::Wrap(format!(
                "waiting on {}: {}",
                park.waiting_on.as_str(),
                park.next_step
            )),
        )),
        None if row.attention == Attention::User => built.details.push((
            "user",
            Detail::Wrap("reviewed idea awaiting the user".into()),
        )),
        None => {}
    }
    if let Some(needs) = row.park.as_ref().and_then(|park| park.needs) {
        built.details.push((
            "host",
            Detail::Wrap(format!("park host requirement {}", needs.as_str())),
        ));
    }
    if !row.task_needs.is_empty() {
        built
            .details
            .push(("needs", Detail::Wrap(row.task_needs.join(", "))));
    }
    if let Some(claim) = &row.claim {
        let state = claim
            .unverified
            .as_ref()
            .map(|why| format!(" (unverifiable: {why})"))
            .unwrap_or_default();
        built.details.push((
            "claim",
            Detail::Wrap(format!("{} on {}{state}", claim.owner, claim.host)),
        ));
    }
    if let Some(defer) = &row.defer {
        built
            .details
            .push(("defer", Detail::Wrap(format!("deferred until {defer}"))));
    }
    built
}

/// Nonzero statuses only, in status order; `none` when every count is zero.
fn breakdown(counts: &WorkByStatus) -> String {
    let parts: Vec<String> = [
        ("idea", counts.idea),
        ("todo", counts.todo),
        ("doing", counts.doing),
        ("blocked", counts.blocked),
        ("shelved", counts.shelved),
    ]
    .into_iter()
    .filter(|(_, n)| *n > 0)
    .map(|(status, n)| format!("{status} {n}"))
    .collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(", ")
    }
}
