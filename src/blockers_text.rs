//! Pretty `tasks blockers`: the root blockers as one table with labelled columns, then a
//! footer that counts, explains the columns and states the scope, then the diagnostics.
//! Folded rows, park steps and claims stay in JSON and `show` (docs/user-guide.md).

use crate::blockers::CoverageMode;
use crate::ellipsis::{Measure, clip};
use crate::model::Status;
use crate::output::{Attention, BlockerRow, BlockersOut};
use crate::present;
use crate::style::{Painter, Style};
use crate::table::{Column, Layout, Row, paragraph, visible_width};

const COLUMNS: [Column; 9] = [
    Column::right("#"),
    Column::fixed("id"),
    Column::fixed("P"),
    Column::fixed("state"),
    Column::right("work"),
    Column::right("clears"),
    Column::right("ideas"),
    Column::fixed("top"),
    Column::fixed("title"),
];
/// A title never takes more display cells than this, even on a wide terminal.
const TITLE_CAP: usize = 80;
const GAP: usize = 2;

/// Headers always show: the count columns do not explain themselves.
pub fn render(out: &BlockersOut, painter: &Painter, layout: &Layout) -> String {
    let layout = Layout {
        headers: true,
        ..*layout
    };
    let roots: Vec<&BlockerRow> = out
        .blockers
        .iter()
        .filter(|row| row.counted_through.is_empty())
        .collect();
    let plain: Vec<[String; 8]> = roots
        .iter()
        .enumerate()
        .map(|(at, row)| cells(at + 1, row))
        .collect();
    let title_budget = layout.width.map(|width| {
        let fixed: usize = (0..8)
            .map(|at| {
                plain
                    .iter()
                    .map(|cells| visible_width(&cells[at]))
                    .chain([COLUMNS[at].header.len()])
                    .max()
                    .unwrap_or(0)
                    + GAP
            })
            .sum();
        width.saturating_sub(fixed).clamp(1, TITLE_CAP)
    });
    let rows: Vec<Row> = roots
        .iter()
        .zip(&plain)
        .map(|(row, cells)| painted(row, cells, title_budget, painter))
        .collect();
    let mut text = crate::table::render(&COLUMNS, &rows, &layout, None, painter);
    if !text.is_empty() {
        text.push('\n');
    }
    for line in footer(out, roots.len()) {
        text.push_str(&paragraph(&line, painter, &layout));
    }

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
    text
}

/// The unpainted cells before the title: #, id, P, state, work, clears, ideas, top.
fn cells(rank: usize, row: &BlockerRow) -> [String; 8] {
    let count = |n: usize| {
        if n == 0 {
            "-".to_string()
        } else {
            n.to_string()
        }
    };
    [
        rank.to_string(),
        row.id.clone(),
        format!("P{}", row.priority),
        state(row).text().to_string(),
        count(row.transitive_committed_count),
        count(row.committed_cleared_count),
        count(row.transitive_work_by_status.idea),
        row.top_waiting_priority
            .map_or("-".to_string(), |p| format!("P{p}")),
    ]
}

fn painted(
    row: &BlockerRow,
    plain: &[String; 8],
    title_budget: Option<usize>,
    painter: &Painter,
) -> Row {
    let quiet = |text: &str| {
        if text == "-" {
            painter.paint(Style::Chrome, text)
        } else {
            text.to_string()
        }
    };
    let title = title_budget.map_or_else(
        || row.title.clone(),
        |budget| clip(&row.title, budget, Measure::DisplayCells),
    );
    Row::new(vec![
        plain[0].clone(),
        present::id(&row.id, row.project_color).paint(painter),
        present::priority(row.priority).paint(painter),
        state(row).paint(painter),
        quiet(&plain[4]),
        quiet(&plain[5]),
        quiet(&plain[6]),
        row.top_waiting_priority
            .map_or_else(|| quiet("-"), |p| present::priority(p).paint(painter)),
        present::title(&title, row.project_color, row.activity).paint(painter),
    ])
}

/// One word for what stands between a row and someone finishing it: a word of this view,
/// or the record's own status painted as status everywhere.
enum State {
    Word(&'static str, Style),
    Status(Status),
}

impl State {
    fn text(&self) -> &'static str {
        match self {
            State::Word(word, _) => word,
            State::Status(status) => status.as_str(),
        }
    }

    fn paint(&self, painter: &Painter) -> String {
        match self {
            State::Word(word, style) => painter.paint(*style, word),
            State::Status(status) => present::status(*status).paint(painter),
        }
    }
}

/// Most pressing first: a cycle, the user, a shelf or idea, a deferral, its own open
/// dependencies, then what this host knows of who holds it.
fn state(row: &BlockerRow) -> State {
    if row.cycle {
        State::Word("cycle", Style::Error)
    } else if row.attention == Attention::User {
        State::Word("user", Style::Warning)
    } else if row.status == Status::Shelved {
        State::Word("shelved", Style::Chrome)
    } else if row.status == Status::Idea {
        State::Status(Status::Idea)
    } else if row.defer.is_some() {
        State::Word("deferred", Style::Chrome)
    } else if row.unmet_dependency_count > 0 || row.unresolved_dependency_count > 0 {
        State::Word("waits", Style::Chrome)
    } else if row.status == Status::Blocked {
        State::Status(Status::Blocked)
    } else if row.claim.is_some() {
        State::Word("claimed", Style::Emphasis)
    } else if row.park.is_some() {
        State::Word("parked", Style::Chrome)
    } else {
        State::Status(row.status)
    }
}

/// What the table holds, what its columns mean, and what the scan covered.
fn footer(out: &BlockersOut, shown: usize) -> Vec<String> {
    let mut count = format!("{shown} of {} root blockers, ranked by work", out.roots);
    if out.folded > 0 {
        count.push_str(&format!(
            "; {} more wait on one of these and count through it",
            out.folded
        ));
    }
    let coverage = &out.coverage;
    let projects = coverage.requested_projects.len();
    let scope = match coverage.mode {
        CoverageMode::Local | CoverageMode::Project => coverage.requested_projects.join(", "),
        CoverageMode::Group => format!("a group of {projects} projects"),
        CoverageMode::AllProjects => format!("all {projects} projects"),
    };
    let reach = if coverage.complete {
        "coverage complete".to_string()
    } else {
        let mut gaps = Vec::new();
        if !coverage.unfollowed_targets.is_empty() {
            gaps.push(format!(
                "{} paths leave it (--all-projects follows them)",
                coverage.unfollowed_targets.len()
            ));
        }
        if !coverage.unreachable_projects.is_empty() {
            gaps.push(format!(
                "{} projects unreachable",
                coverage.unreachable_projects.len()
            ));
        }
        if !out.unresolved.is_empty() {
            gaps.push(format!("{} references unresolved", out.unresolved.len()));
        }
        format!("partial, so counts are lower bounds: {}", gaps.join(", "))
    };
    vec![
        count,
        "work: todo/doing/blocked tasks waiting on it, at any depth · clears: those it alone \
         still holds · top: highest priority waiting"
            .into(),
        format!(
            "dependents in {scope}; {reach} · parks and claims from {}",
            out.park_evidence.host
        ),
    ]
}
