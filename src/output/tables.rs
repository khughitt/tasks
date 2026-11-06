use crate::lanes::{Causes, LaneRow, LaneState};
use crate::present;
use crate::style::{Painter, Style};

use super::contract::{
    CarriedOut, Counts, DateColumn, DroppedOut, HaltRow, MigrateOut, Output, ParkedRow,
    TaskSummary, TreeNode,
};
use super::project::count_columns;

fn halt_line(halts: &[HaltRow]) -> String {
    if halts.is_empty() {
        return String::new();
    }
    let named = halts
        .iter()
        .map(|halt| {
            let owner = halt
                .owner
                .as_ref()
                .map_or(String::new(), |owner| format!(", owner {owner}"));
            format!("{} (P{}, {}{})", halt.id, halt.priority, halt.title, owner)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let missing = if halts.iter().any(|halt| !halt.present_locally) {
        "; start a halt absent here from its registered checkout"
    } else {
        ""
    };
    format!("halt: {named}; allowed: halt work, linked work, or equally urgent priority{missing}\n")
}

/// The migration review the user reads (docs/task-model.md): refusals, the successor tree for the
/// carried tasks, drops with the dependents they release, closes, removed edges and dropped
/// links, then body diffs.
fn migrate_review(o: &MigrateOut) -> String {
    let named = |id: &str| match o.titles.get(id) {
        Some(title) => format!("{id} {title}"),
        None => id.to_string(),
    };
    let named_all = |ids: &[String]| ids.iter().map(|i| named(i)).collect::<Vec<_>>().join(", ");
    let mut text = format!("{} → {} ({})", o.source, o.successor, o.recovery);
    if !o.refusals.is_empty() {
        text.push_str("\n\nrefusals:");
        for r in &o.refusals {
            text.push_str(&format!("\n  {} [{}]: {}", r.id, r.rule, r.settle));
        }
    }
    if !o.carried.is_empty() {
        text.push_str("\n\ncarried:");
        let label = |row: &CarriedOut| {
            let mut label = row.id.as_ref().map_or_else(
                || named(&row.from),
                |id| format!("{} -> {id}", named(&row.from)),
            );
            if row.park {
                label.push_str(" [parked]");
            }
            if !row.attachments.is_empty() {
                label.push_str(&format!(" [{} attachments]", row.attachments.len()));
            }
            if row.rewrites.iter().any(|r| {
                r.starts_with("parent set to")
                    || r.contains("overridden; set to")
                    || r.contains("not carried; set to")
            }) {
                label.push_str(" (new parent)");
            }
            label
        };
        let carried_parent = |row: &CarriedOut| {
            row.parent
                .as_deref()
                .and_then(|p| p.strip_prefix("new:"))
                .map(str::to_string)
        };
        // Roots first: the successor's top level, then each successor parent.
        let mut places: Vec<Option<&str>> = Vec::new();
        for row in &o.carried {
            if carried_parent(row).is_none() && !places.contains(&row.parent.as_deref()) {
                places.push(row.parent.as_deref());
            }
        }
        places.sort();
        fn walk(
            text: &mut String,
            o: &MigrateOut,
            from: &str,
            depth: usize,
            label: &dyn Fn(&CarriedOut) -> String,
            carried_parent: &dyn Fn(&CarriedOut) -> Option<String>,
        ) {
            for row in o
                .carried
                .iter()
                .filter(|row| carried_parent(row).as_deref() == Some(from))
            {
                text.push_str(&format!("\n{}{}", "  ".repeat(depth), label(row)));
                walk(text, o, &row.from, depth + 1, label, carried_parent);
            }
        }
        for place in places {
            let depth = match place {
                None => 1,
                Some(parent) => {
                    text.push_str(&format!("\n  under {parent}:"));
                    2
                }
            };
            for row in o
                .carried
                .iter()
                .filter(|row| carried_parent(row).is_none() && row.parent.as_deref() == place)
            {
                text.push_str(&format!("\n{}{}", "  ".repeat(depth), label(row)));
                walk(&mut text, o, &row.from, depth + 1, &label, &carried_parent);
            }
        }
    }
    if !o.dropped.is_empty() {
        text.push_str("\n\ndropped:");
        let mut reasons: std::collections::BTreeMap<&str, Vec<&DroppedOut>> = Default::default();
        for d in &o.dropped {
            reasons.entry(d.reason.as_str()).or_default().push(d);
        }
        for (reason, rows) in reasons {
            text.push_str(&format!("\n  {reason}:"));
            for d in rows {
                text.push_str(&format!("\n    {}", named(&d.id)));
                if !d.released.is_empty() {
                    text.push_str(&format!(" (released: {})", named_all(&d.released)));
                }
            }
        }
    }
    if !o.closed.is_empty() {
        text.push_str("\n\nclosed:");
        for c in &o.closed {
            text.push_str(&format!("\n  {}: {}", named(&c.id), c.reason));
            if !c.released.is_empty() {
                text.push_str(&format!(" (released: {})", named_all(&c.released)));
            }
            if !c.open_depends.is_empty() {
                text.push_str(&format!(
                    " (open dependencies: {})",
                    c.open_depends.join(", ")
                ));
            }
        }
    }
    if !o.woken.is_empty() {
        text.push_str("\n\nwoken shelves:");
        for w in &o.woken {
            text.push_str(&format!("\n  {}: {}", named(&w.id), w.shelves.join(", ")));
        }
    }
    if !o.inbound.is_empty() {
        text.push_str("\n\noutside dependents of carried tasks:");
        for i in &o.inbound {
            text.push_str(&format!(
                "\n  {}: {}",
                named(&i.id),
                i.dependents.join(", ")
            ));
        }
    }
    let removed: Vec<(&str, &str)> = o
        .carried
        .iter()
        .flat_map(|row| {
            row.rewrites
                .iter()
                .filter(|r| r.starts_with("removed ") || r.contains(" dropped (not in "))
                .map(move |r| (row.from.as_str(), r.as_str()))
        })
        .collect();
    if !removed.is_empty() {
        text.push_str("\n\nremoved:");
        for (from, what) in removed {
            text.push_str(&format!("\n  {}: {what}", named(from)));
        }
    }
    if !o.body_changes.is_empty() {
        text.push_str("\n\nbody:");
        for change in &o.body_changes {
            text.push_str(&format!(
                "\n{}:\n{}",
                change.id,
                change.diff.trim_end_matches('\n')
            ));
        }
    }
    text
}

pub(super) fn pretty(out: &Output, painter: &Painter, layout: &crate::table::Layout) -> String {
    match out {
        Output::RetainedMutation(out) => {
            serde_json::to_string_pretty(out).expect("retained mutation serializes")
        }
        Output::RetainedState(out) => {
            serde_json::to_string_pretty(out).expect("retained state serializes")
        }
        Output::AttachmentEligibility(out) => out
            .attachments
            .iter()
            .map(|row| {
                if row.eligible {
                    format!("{}: eligible", row.path)
                } else {
                    format!(
                        "{}: ineligible\n{}",
                        row.path,
                        row.problems
                            .iter()
                            .map(|p| p.line())
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Init(o) => o.prefix.clone(),
        Output::Retire(o) => match &o.retired {
            Some(r) => {
                let successor = r
                    .successor
                    .as_ref()
                    .map(|s| format!(" → {s}"))
                    .unwrap_or_default();
                format!("{} retired {}{successor}: {}", o.prefix, r.date, r.reason)
            }
            None => format!("{} is not retired", o.prefix),
        },
        Output::Move(o) => {
            // A refusal before the destination id is known carries an empty id.
            let mut text = if o.id.is_empty() {
                format!("{} ({})", o.from, o.recovery)
            } else {
                format!("{} → {} ({})", o.from, o.id, o.recovery)
            };
            for id in &o.inbound {
                text.push_str(&format!("\n  inbound: {id}"));
            }
            for (root, entries) in &o.paths {
                for entry in entries {
                    text.push_str(&format!("\n  path: {root}/{entry}"));
                }
            }
            text
        }
        Output::MigrateDraft(o) => format!("{}: {} tasks to decide", o.path, o.tasks),
        Output::Migrate(o) => migrate_review(o),
        Output::Rename(o) => o.prefix.clone(),
        Output::Id(o) => o.id.clone(),
        Output::Commit(o) => {
            if o.committed > 0 {
                format!("committed {} records", o.committed)
            } else {
                "nothing to commit".into()
            }
        }
        Output::Add(o) => o.id.clone(),
        Output::Root(o) => o.root.clone(),
        Output::Where(o) => match &o.project {
            None => "no project".into(),
            Some(p) => {
                let prefix = present::project(&p.prefix, p.color).paint(painter);
                match p.kind {
                    crate::repo::Kind::Linked => {
                        format!("{prefix} {} (linked to {})", p.root, p.checkout)
                    }
                    crate::repo::Kind::Detached => {
                        format!("{prefix} {} (records at {})", p.root, p.records)
                    }
                    crate::repo::Kind::InTree => format!("{prefix} {}", p.root),
                }
            }
        },

        Output::Resolve(o) => o
            .results
            .iter()
            .map(|r| {
                let answer = r.id.as_deref().or(r.prefix.as_deref()).unwrap_or("");
                let via = r
                    .via
                    .map(|v| {
                        serde_json::to_value(v)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .unwrap_or_default();
                [r.input.as_str(), r.status, answer, via.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("  ")
            })
            .collect::<Vec<_>>()
            .join("\n"),

        Output::Group(o) => o.name.clone(),
        Output::Groups(o) => o
            .groups
            .iter()
            .map(|row| {
                let members: Vec<String> = row
                    .members
                    .iter()
                    .map(|member| {
                        let prefix = present::project(&member.prefix, member.color).paint(painter);
                        if member.reachable {
                            prefix
                        } else {
                            format!("{prefix} (unreachable)")
                        }
                    })
                    .collect();
                format!("{}  {}", row.name, members.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Projects(o) if o.projects.is_empty() => String::new(),
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
                        Some(counts) => cells.extend(
                            count_columns(counts, o.closed).into_iter().map(|column| {
                                let value = column.value.to_string();
                                column
                                    .style
                                    .map_or(value.clone(), |style| painter.paint(style, &value))
                            }),
                        ),
                        None => cells.extend(
                            count_columns(&Counts::default(), o.closed)
                                .iter()
                                .map(|_| painter.paint(Style::Chrome, "-")),
                        ),
                    }
                    cells.push(match (&row.counts, &row.last_activity) {
                        (None, _) => painter.paint(Style::Error, "unreachable"),
                        (Some(_), _) if row.retired.is_some() => {
                            painter.paint(Style::Chrome, "retired")
                        }
                        (Some(_), Some(at)) => present::date(at).paint(painter),
                        (Some(_), None) => painter.paint(Style::Chrome, "-"),
                    });
                    if o.paths {
                        cells.push(match row.kind {
                            Some(crate::repo::Kind::Detached) => {
                                format!("{} → {}", row.root, row.records)
                            }
                            _ => row.root.clone(),
                        });
                    }
                    Row::new(cells)
                })
                .collect();
            let always = crate::table::Layout {
                headers: true,
                ..*layout
            };
            crate::table::render(&columns, &rows, &always, None, painter)
        }
        Output::Show(o) => crate::show_text::render(&o.fields, painter, layout.width),
        Output::Doc(o) => o.body.clone(),
        Output::Raw(_) => unreachable!("raw bytes are written directly"),
        Output::Export(o) => {
            let mut text = format!(
                "{} {}: {} bytes, sha256 {}\nintegrity: {}",
                o.id,
                o.name,
                o.bytes,
                o.sha256,
                if o.integrity.passed {
                    "passed"
                } else {
                    "failed"
                }
            );
            if let Some(commit) = &o.commit {
                text.push_str(&format!("\ncommit: {commit}"));
            }
            if let Some(validation) = &o.validation {
                let verdict = if validation.passed {
                    "passed"
                } else {
                    match crate::multipart::publication::validation_cause(validation) {
                        "validation_unavailable" => "unavailable",
                        _ => "failed",
                    }
                };
                text.push_str(&format!("\nvalidation: {verdict} ({})", validation.basis));
                for run in validation
                    .validators
                    .iter()
                    .filter(|run| run.outcome != "passed")
                {
                    text.push_str(&format!("\nvalidator {:?}: {}", run.command, run.outcome));
                    if let Some(exit) = run.exit {
                        text.push_str(&format!(", exit {exit}"));
                    }
                    if !run.stderr.trim().is_empty() {
                        text.push_str(&format!("\n{}", run.stderr.trim()));
                    }
                }
            }
            text
        }
        Output::Next(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            match &o.next {
                Some(fields) => crate::show_text::render(fields, painter, layout.width),
                None => "nothing ready".into(),
            }
        ),
        Output::List(o) => format!(
            "{}{}",
            halt_line(&o.halts),
            task_table(&o.tasks, o.date, painter, layout)
        ),
        Output::Parked(o) => parked_table(
            &o.tasks,
            painter,
            layout,
            None,
            o.tasks.iter().any(|row| row.needs_review == Some(true)),
        ),
        Output::Quiet(o) => quiet_table(&o.tasks, painter, layout),
        Output::Goals(o) => goals_table(
            &o.goals,
            painter,
            layout,
            crate::time::parse(&o.observed_at).expect("observed_at is a stamp this run wrote"),
        ),
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
                        match (&row.claim.unverified, row.claim.live) {
                            (Some(_), _) => "unverifiable",
                            (None, true) => "live",
                            (None, false) => "stale",
                        }
                        .into(),
                        row.claim.worktree.clone(),
                    ])
                })
                .collect();
            crate::table::render(&columns, &rows, layout, None, painter)
                .trim_end_matches('\n')
                .to_string()
        }
        Output::Prime(o) => {
            let mut marks = Marks::of(&o.closeout)
                .or(Marks::of_tree(&o.roadmap))
                .or(Marks::of(&o.ready))
                .or(Marks::of(&o.doing));
            marks.review |= o.parked.iter().any(|row| row.needs_review == Some(true));
            let columns = task_columns(marks);
            let rows_of = |rows: &[TaskSummary]| -> Vec<crate::table::Row> {
                rows.iter()
                    .map(|row| task_row(row, DateColumn::Updated, painter, marks, 0))
                    .collect()
            };
            let closeout = rows_of(&o.closeout);
            let ready_ids: std::collections::HashSet<&str> =
                o.ready.iter().map(|row| row.id.as_str()).collect();
            let mut roadmap = Vec::new();
            let mut listed_under_ready = 0;
            for node in &o.roadmap {
                if node.summary.child_count > 0 {
                    tree_rows(std::slice::from_ref(node), 0, painter, marks, &mut roadmap);
                } else if ready_ids.contains(node.summary.id.as_str()) {
                    listed_under_ready += 1;
                } else {
                    roadmap.push(task_row(
                        &node.summary,
                        DateColumn::Updated,
                        painter,
                        marks,
                        0,
                    ));
                }
            }
            let ready = rows_of(&o.ready);
            let doing = rows_of(&o.doing);
            let mut shared = crate::table::shared_widths(
                &columns,
                &[&closeout, &roadmap, &ready, &doing],
                layout.headers,
            );
            let pad = if marks.review { 2 } else { 0 };
            let id = shared
                .id()
                .unwrap_or(0)
                .max(
                    o.parked
                        .iter()
                        .map(|row| row.id.len() + pad)
                        .max()
                        .unwrap_or(0),
                )
                .max(
                    o.lanes
                        .iter()
                        .map(|row| row.lane.id.len() + pad)
                        .max()
                        .unwrap_or(0),
                );
            shared.0[0] = Some(id);
            let section = |rows: &[crate::table::Row]| {
                crate::table::render(&columns, rows, layout, Some(&shared), painter)
            };
            let header = match (&o.prefix, &o.group) {
                (Some(prefix), _) => format!("project {prefix}"),
                (None, Some(group)) => format!("group {group}: projects {}", o.projects.join(", ")),
                (None, None) => format!("projects {}", o.projects.join(", ")),
            };
            // One row, so labels stay beside their values instead of over them - but the
            // columns and their colors are the same definition `projects` renders.
            let counts: Vec<String> = count_columns(&o.counts, o.closed)
                .into_iter()
                .map(|column| {
                    let value = column.value.to_string();
                    let value = match column.style {
                        Some(style) => painter.paint(style, &value),
                        None => value,
                    };
                    format!("{} {value}", column.label)
                })
                .collect();
            let mut rendered = format!("{}{header}\n{}\n", halt_line(&o.halts), counts.join("  "));
            if o.periodic.scheduled > 0 {
                let next = o
                    .periodic
                    .next_due
                    .as_deref()
                    .expect("scheduled recurrences have a next due date");
                let days = o
                    .periodic
                    .in_days
                    .expect("a next due date has a relative day count");
                rendered.push_str(&format!(
                    "periodic: {} scheduled, next due {} (in {days}d)\n",
                    o.periodic.scheduled,
                    crate::time::day(next)
                ));
            }
            if o.deferred.waiting > 0 || o.deferred.due > 0 {
                let mut line = match (&o.deferred.next, o.deferred.in_days) {
                    (Some(next), Some(days)) => {
                        format!(
                            "deferred: {} waiting, next {next} (in {days}d)",
                            o.deferred.waiting
                        )
                    }
                    _ => format!("deferred: {} waiting", o.deferred.waiting),
                };
                if o.deferred.due > 0 {
                    line.push_str(&format!("; {} due", o.deferred.due));
                }
                rendered.push_str(&line);
                rendered.push('\n');
            }

            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "closeout:")
            ));
            rendered.push_str(&section(&closeout));
            if !o.lanes.is_empty() {
                rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "lanes:")));
                rendered.push_str(&lane_table(
                    &o.lanes,
                    painter,
                    layout,
                    Some(&crate::table::Widths::id_only(id, 4)),
                    false,
                    marks.review,
                ));
            }
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "roadmap:")
            ));
            rendered.push_str(&section(&roadmap));
            rendered.push_str(&format!(
                "{listed_under_ready} childless root(s) are listed under ready\n"
            ));
            rendered.push_str(&format!(
                "\n{}\n",
                painter.paint(Style::Emphasis, "parked:")
            ));
            rendered.push_str(&parked_table(
                &o.parked,
                painter,
                layout,
                Some(&crate::table::Widths::id_only(id, 7)),
                marks.review,
            ));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "ready:")));
            rendered.push_str(&section(&ready));
            rendered.push_str(&format!("\n{}\n", painter.paint(Style::Emphasis, "doing:")));
            rendered.push_str(&section(&doing));
            rendered
        }
        Output::Lanes(o) => lane_table(&o.lanes, painter, layout, None, true, false),
        Output::Blockers(o) => crate::blockers_text::render(o, painter, layout),
        Output::Graph(o) => o.text.clone(),
        Output::Check(o) => {
            let mut rendered = String::new();
            if let Some(growth) = &o.growth {
                use crate::table::{Column, Row};
                let columns = [
                    Column::fixed("id"),
                    Column::right("record bytes"),
                    Column::right("body bytes"),
                    Column::right("notes"),
                    Column::right("note text bytes"),
                    Column::right("duplicate bytes"),
                    Column::right("archived bytes"),
                ];
                let rows: Vec<_> = growth
                    .iter()
                    .map(|row| {
                        Row::new(vec![
                            row.id.clone(),
                            row.record_bytes.to_string(),
                            row.body_bytes.to_string(),
                            row.note_count.to_string(),
                            row.note_text_bytes.to_string(),
                            row.duplicate_note_text_bytes.to_string(),
                            row.archived_bytes.to_string(),
                        ])
                    })
                    .collect();
                let headed = crate::table::Layout {
                    headers: true,
                    ..*layout
                };
                rendered.push_str(&crate::table::render(
                    &columns, &rows, &headed, None, painter,
                ));
                rendered.push_str("\nPlace bulky evidence in attachments or records documents; preserve original bytes before editing a body.\n");
            }

            for finding in &o.errors {
                let line = format!(
                    "error: {} [{}] {}",
                    finding.file, finding.kind, finding.detail
                );
                rendered.push_str(&painter.paint(Style::Error, &line));
                rendered.push('\n');
            }
            if o.errors.is_empty() {
                rendered.push_str(&painter.paint(Style::Ok, "ok"));
                rendered.push('\n');
            }
            rendered
        }
        Output::Tree(o) => {
            let marks = Marks::of_tree(&o.nodes);
            let mut rows = Vec::new();
            tree_rows(&o.nodes, 0, painter, marks, &mut rows);
            crate::table::render(&task_columns(marks), &rows, layout, None, painter)
        }
        Output::Tags(o) => {
            let mut rendered = String::new();
            for row in &o.tags {
                let parts: Vec<String> = row
                    .projects
                    .iter()
                    .map(|(prefix, count)| format!("{prefix} {count}"))
                    .collect();
                let breakdown = painter.paint(Style::Chrome, &format!("  ({})", parts.join(", ")));
                let meaning = row
                    .meaning
                    .as_deref()
                    .map(|meaning| format!("  {meaning}"))
                    .unwrap_or_default();
                rendered.push_str(&format!(
                    "{:>4}  {}{meaning}{breakdown}\n",
                    row.count, row.tag
                ));
            }
            rendered
        }
        Output::Docs(o) => {
            use crate::table::{Column, Detail, Row};
            let columns = [
                Column::fixed("kind"),
                Column::fixed("path"),
                Column::wrap("title", 20, None),
            ];
            let rows: Vec<Row> = o
                .docs
                .iter()
                .map(|doc| {
                    let path = if o.wide {
                        format!("{}:{}", doc.project, doc.path)
                    } else {
                        doc.path.clone()
                    };
                    let mut row = Row::new(vec![doc.kind.to_string(), path, doc.title.clone()]);
                    if !doc.tasks.is_empty() {
                        let links: Vec<String> = doc
                            .tasks
                            .iter()
                            .map(|link| match &link.handle {
                                Some(handle) => format!("{} {} {handle}", link.id, link.via),
                                None => format!("{} {}", link.id, link.via),
                            })
                            .collect();
                        row.details.push(("tasks", Detail::Wrap(links.join(", "))));
                    }
                    row
                })
                .collect();
            crate::table::render(&columns, &rows, layout, None, painter)
        }
        Output::Feedback(o) => format!("{} {}", o.action, o.id),
        Output::ArchivePreview(o) => format!(
            "{}: {} → {} bytes; {}",
            o.id,
            o.record_bytes_before,
            o.record_bytes_after,
            if o.eligible {
                "eligible".into()
            } else {
                o.reasons.join("; ")
            }
        ),
        Output::ArchiveApply(o) => format!("{}: {} ({})", o.id, o.action, o.archive_name),
        Output::Attach(o) if o.publication.is_some() => {
            let publication = o.publication.as_ref().expect("present");
            match publication.cause {
                Some(cause) => format!(
                    "local ({cause}); {}",
                    publication.next.as_deref().unwrap_or("")
                ),
                None => format!(
                    "published at {}",
                    publication.commit.as_deref().unwrap_or("")
                ),
            }
        }
        Output::Attach(o) => o
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Detach(o) if o.publication.is_some() => {
            let publication = o.publication.as_ref().expect("present");
            let mut text = match publication.cause {
                Some(cause) => format!("local ({cause})"),
                None => format!(
                    "published at {}",
                    publication.commit.as_deref().unwrap_or("")
                ),
            };
            if let Some(reuse) = publication.reuse {
                text.push_str(&format!("; reuse: {reuse}"));
            }
            if let Some(next) = &publication.next {
                text.push_str(&format!("; {next}"));
            }
            text
        }
        Output::Detach(o) if o.removed => format!(
            "removed {}; git history keeps committed bytes, uncommitted ones are gone",
            o.path
        ),
        Output::Detach(o) => format!("recorded {} as detached; no file was present", o.name),
    }
}

/// One row per lane: id, priority, title, then its state with the pick or its main
/// cause (docs/user-guide.md). `detail` adds the guidance and the active claims under each
/// row, as `tasks lanes` prints them; `prime` leaves them out.
fn lane_table(
    rows: &[LaneRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
    detail: bool,
    pad: bool,
) -> String {
    use crate::table::{Column, Detail, Row};
    let columns = [
        Column::fixed("id"),
        Column::fixed("P"),
        Column::wrap("title", 20, None),
        Column::wrap("state", 20, None),
    ];
    let task = |row: &TaskSummary| {
        format!(
            "{} {}",
            present::id(&row.id, row.project_color).paint(painter),
            present::title(&row.title, row.project_color).paint(painter)
        )
    };
    let built: Vec<Row> = rows
        .iter()
        .map(|row| {
            let state = match row.state {
                LaneState::Ready => {
                    let pick = row.pick.as_ref().expect("a ready lane has a pick");
                    format!(
                        "{} {}",
                        painter.paint(Style::Emphasis, "ready →"),
                        task(pick)
                    )
                }
                LaneState::Held => {
                    let first = row
                        .held
                        .first()
                        .expect("a held lane names what it waits for");
                    painter.paint(
                        Style::Emphasis,
                        &format!("held: {} ← {}", first.need, first.holder),
                    )
                }
                LaneState::Waiting => painter.paint(Style::Emphasis, &waiting_text(&row.causes)),
                LaneState::Paused => painter.paint(Style::Emphasis, "paused"),
                LaneState::Empty => painter.paint(Style::Emphasis, "empty"),
            };
            let mut built = Row::new(vec![
                format!(
                    "{}{}",
                    if pad { "  " } else { "" },
                    present::id(&row.lane.id, row.lane.project_color).paint(painter)
                ),
                present::priority(row.lane.priority).paint(painter),
                present::title(&row.lane.title, row.lane.project_color).paint(painter),
                state,
            ]);
            if detail {
                if let Some(guidance) = &row.guidance {
                    built
                        .details
                        .push(("guide", Detail::Wrap(guidance.clone())));
                }
                for active in &row.active {
                    let holder = active
                        .claim
                        .as_ref()
                        .map(|claim| {
                            painter.paint(
                                Style::Chrome,
                                &format!(" @{} [{}]", claim.owner, claim.session),
                            )
                        })
                        .unwrap_or_default();
                    built
                        .details
                        .push(("active", Detail::Wrap(format!("{}{holder}", task(active)))));
                }
            }
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, widths, painter)
}

/// `waiting: 2 user, 1 deferred`: every cause with its count, the largest first, ties in
/// table order.
fn waiting_text(causes: &Causes) -> String {
    let mut entries = causes.entries();
    entries.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let parts: Vec<String> = entries
        .iter()
        .map(|(cause, count)| format!("{count} {cause}"))
        .collect();
    format!("waiting: {}", parts.join(", "))
}

/// The optional marks a pretty task table reserves, decided once per command output so a
/// per-section decision cannot shift columns between adjacent sections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Marks {
    /// The `* ` prefix on the id cell (docs/user-guide.md).
    review: bool,
    /// The `||` column.
    parallel: bool,
    /// The type column.
    kind: bool,
}

impl Marks {
    fn of(rows: &[TaskSummary]) -> Marks {
        Marks {
            review: rows.iter().any(|row| row.needs_review),
            parallel: any_parallel(rows),
            kind: any_type(rows),
        }
    }

    fn of_tree(nodes: &[TreeNode]) -> Marks {
        Marks {
            review: any_needs_review_tree(nodes),
            parallel: any_parallel_tree(nodes),
            kind: any_type_tree(nodes),
        }
    }

    fn or(self, other: Marks) -> Marks {
        Marks {
            review: self.review || other.review,
            parallel: self.parallel || other.parallel,
            kind: self.kind || other.kind,
        }
    }
}

fn any_needs_review_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.needs_review || any_needs_review_tree(&node.children))
}

/// Whether a pretty rendering must reserve the parallel column. Decided once per command
/// output so a per-section decision cannot shift dates between adjacent siblings.
fn any_parallel(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| row.parallel)
}

fn any_parallel_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| node.summary.parallel || any_parallel_tree(&node.children))
}

/// The one-letter type marker for a summary row: `≡` for a lane, `p` for a record
/// carrying a cadence, and nothing otherwise. A lane is a goal and never recurs, so the
/// two never compete for the slot. A future type is another arm here, another letter in
/// the same slot; the column is reserved once per output (see `any_type`), so it is never
/// a layout change.
fn type_letter(row: &TaskSummary) -> Option<char> {
    if row.lane {
        return Some('≡');
    }
    row.periodic.as_ref().map(|_| 'p')
}

/// Whether a pretty rendering must reserve the type column. Same once-per-output rule as
/// `any_parallel`, so dates stay aligned between adjacent siblings.
fn any_type(rows: &[TaskSummary]) -> bool {
    rows.iter().any(|row| type_letter(row).is_some())
}

fn any_type_tree(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|node| type_letter(&node.summary).is_some() || any_type_tree(&node.children))
}

fn task_columns(marks: Marks) -> Vec<crate::table::Column> {
    use crate::table::Column;
    let mut columns = vec![
        Column::fixed("id"),
        Column::fixed("P"),
        Column::fixed("size"),
        Column::fixed("cx"),
        Column::fixed("process"),
        Column::fixed("status"),
    ];
    if marks.parallel {
        columns.push(Column::fixed("||"));
    }
    if marks.kind {
        columns.push(Column::fixed("type"));
    }
    columns.push(Column::fixed("date"));
    columns.push(Column::wrap("title", 20, None));
    columns
}

fn task_row(
    row: &TaskSummary,
    date: DateColumn,
    painter: &Painter,
    marks: Marks,
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
            "{}{}{}",
            present::review_mark(row.needs_review, marks.review).paint(painter),
            "  ".repeat(indent),
            present::id(&row.id, row.project_color).paint(painter)
        ),
        present::priority(row.priority).paint(painter),
        present::size(row.size).paint(painter),
        present::complexity(row.complexity).paint(painter),
        present::process(row.process, "-").paint(painter),
        present::status(row.status).paint(painter),
    ];
    if marks.parallel {
        cells.push(if row.parallel {
            "||".into()
        } else {
            String::new()
        });
    }
    if marks.kind {
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
        cell.push_str(
            &present::Span {
                text: format!("[{}]", tags.text),
                ..tags
            }
            .paint(painter),
        );
    }
    if let Some(periodic) = row.periodic.as_ref().filter(|periodic| periodic.due_now) {
        let when = periodic
            .due
            .as_deref()
            .map(|due| crate::time::day(due).to_string())
            .unwrap_or_else(|| "now".into());
        cell.push_str("  ");
        cell.push_str(&painter.paint(
            Style::Emphasis,
            &format!("every {}, due {when}", periodic.every),
        ));
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
    if let Some(park) = row
        .park
        .as_ref()
        .filter(|park| park.reason == Some(crate::claims::Reason::Quiet))
    {
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
        cell.push_str(
            &present::Span {
                text: format!("@{}", owner.text),
                ..owner
            }
            .paint(painter),
        );
    }
    cell
}

fn tree_rows(
    nodes: &[TreeNode],
    depth: usize,
    painter: &Painter,
    marks: Marks,
    rows: &mut Vec<crate::table::Row>,
) {
    for node in nodes {
        rows.push(task_row(
            &node.summary,
            DateColumn::Updated,
            painter,
            marks,
            depth,
        ));
        tree_rows(&node.children, depth + 1, painter, marks, rows);
    }
}

fn task_table(
    rows: &[TaskSummary],
    date: DateColumn,
    painter: &Painter,
    layout: &crate::table::Layout,
) -> String {
    let marks = Marks::of(rows);
    let built: Vec<crate::table::Row> = rows
        .iter()
        .map(|row| task_row(row, date, painter, marks, 0))
        .collect();
    crate::table::render(&task_columns(marks), &built, layout, None, painter)
}

fn parked_columns() -> Vec<crate::table::Column> {
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

fn parked_row(row: &ParkedRow, painter: &Painter, review: bool) -> crate::table::Row {
    let park = row.park.as_ref().expect("a parked row carries its park");
    crate::table::Row::new(vec![
        format!(
            "{}{}",
            present::review_mark(row.needs_review == Some(true), review).paint(painter),
            present::id(&row.id, row.project_color).paint(painter)
        ),
        present::status_of(row.status).paint(painter),
        row.phase
            .map(crate::model::Phase::as_str)
            .unwrap_or("-")
            .to_string(),
        crate::claims::describe_stop(park.waiting_on, park.reason, park.needs, park.minutes),
        present::date(&park.at).paint(painter),
        present::title(&row.title, row.project_color).paint(painter),
        painter.paint(Style::Chrome, &park.next_step),
    ])
}

fn parked_table(
    rows: &[ParkedRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    widths: Option<&crate::table::Widths>,
    review: bool,
) -> String {
    let built: Vec<crate::table::Row> = rows
        .iter()
        .filter(|row| row.park.is_some())
        .map(|row| parked_row(row, painter, review))
        .collect();
    crate::table::render(&parked_columns(), &built, layout, widths, painter)
}

/// The pretty goals view (docs/user-guide.md): open rows, then, under `--closed`, a
/// separator and the recently closed rows, dimmed. Fixed columns line up across both sections.
fn goals_table(
    rows: &[crate::goals::GoalRow],
    painter: &Painter,
    layout: &crate::table::Layout,
    now: time::OffsetDateTime,
) -> String {
    use crate::table::{Column, Row};
    if rows.is_empty() {
        return "no goals you asked for; goals without an origin are not shown \
                (tasks edit <id> --origin user)"
            .into();
    }
    let columns = [
        Column::fixed("id"),
        Column::fixed("status"),
        Column::fixed("P"),
        Column::fixed("progress"),
        Column::fixed("active"),
        Column::wrap("title", 20, None),
        Column::fixed("attention"),
    ];
    let progress = |pieces: &crate::goals::Pieces| {
        let mut text = if pieces.total == 0 {
            "-".to_string()
        } else {
            format!("{}/{}", pieces.done, pieces.total)
        };
        if pieces.ideas > 0 {
            text.push_str(&format!(" +{} ideas", pieces.ideas));
        }
        text
    };
    // Prerequisites outside the subtree, shown only while one is not yet satisfied.
    let progress = |row: &crate::goals::GoalRow| {
        let mut text = progress(&row.pieces);
        if let Some(deps) = row
            .dependencies
            .as_ref()
            .filter(|deps| deps.open + deps.unresolved > 0)
        {
            text.push_str(&format!(" · {}/{} deps open", deps.open, deps.total));
            if deps.unresolved > 0 {
                text.push_str(&format!(", {} unresolved", deps.unresolved));
            }
        }
        text
    };
    let attention = |row: &crate::goals::GoalRow| {
        let Some(flags) = &row.attention else {
            return String::new();
        };
        let mut parts = Vec::new();
        if let Some(flag) = &flags.stalled {
            let since =
                crate::time::parse(&flag.since).expect("since was parsed when the flag was built");
            let days = (now - since).whole_days();
            let text = format!("{} stalled ({}, {days}d)", flag.count, flag.first);
            parts.push(painter.paint(Style::Error, &text));
        }
        if let Some(flag) = &flags.blocked {
            let lane = if flags.paused_lane.as_deref() == Some(flag.first.as_str()) {
                ", paused lane"
            } else {
                ""
            };
            let text = format!("{} blocked ({}{lane})", flag.count, flag.first);
            parts.push(painter.paint(Style::Error, &text));
        }
        if let Some(flag) = &flags.you {
            let text = format!("{} you ({})", flag.count, flag.first);
            parts.push(painter.paint(Style::Emphasis, &text));
        }
        parts.join(" · ")
    };
    let open: Vec<Row> = rows
        .iter()
        .filter(|row| row.closed.is_none())
        .map(|row| {
            Row::new(vec![
                present::id(&row.id, row.project_color).paint(painter),
                present::status(row.status).paint(painter),
                present::priority(row.priority).paint(painter),
                progress(row),
                present::date(&row.last_activity).paint(painter),
                present::title(&row.title, row.project_color).paint(painter),
                attention(row),
            ])
        })
        .collect();
    let closed: Vec<Row> = rows
        .iter()
        .filter(|row| row.closed.is_some())
        .map(|row| {
            let dim = |text: &str| painter.paint(Style::Chrome, text);
            Row::new(vec![
                dim(&row.id),
                dim(row.status.as_str()),
                dim(&format!("P{}", row.priority)),
                dim(&progress(row)),
                dim(crate::time::day(
                    row.closed.as_deref().unwrap_or(&row.last_activity),
                )),
                dim(&row.title),
                String::new(),
            ])
        })
        .collect();
    let widths = crate::table::shared_widths(&columns, &[&open, &closed], layout.headers);
    let mut text = crate::table::render(&columns, &open, layout, Some(&widths), painter);
    if !closed.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&painter.paint(Style::Chrome, "── closed in the last 14 days ──"));
        text.push('\n');
        let rest = crate::table::Layout {
            headers: false,
            ..*layout
        };
        text.push_str(&crate::table::render(
            &columns,
            &closed,
            &rest,
            Some(&widths),
            painter,
        ));
    }
    text
}

fn quiet_table(rows: &[ParkedRow], painter: &Painter, layout: &crate::table::Layout) -> String {
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
                present::id(&row.id, row.project_color).paint(painter),
                match row.priority {
                    Some(priority) => present::priority(priority).paint(painter),
                    None => "P-".into(),
                },
                park.needs
                    .map(crate::claims::Needs::as_str)
                    .unwrap_or("-")
                    .to_string(),
                park.minutes
                    .map_or_else(|| "-".into(), |minutes| format!("{minutes} min")),
                present::date(&park.at).paint(painter),
                present::title(&row.title, row.project_color).paint(painter),
                painter.paint(Style::Chrome, &park.next_step),
            ]);
            built
                .details
                .push(("in", Detail::Whole(park.worktree.clone())));
            built
        })
        .collect();
    crate::table::render(&columns, &built, layout, None, painter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use crate::output::{Format, ListOut, ParkInfo, PeriodicInfo};
    use crate::style::ColorMode;

    fn row(id: &str, parallel: bool) -> TaskSummary {
        TaskSummary {
            id: id.into(),
            title: format!("title {id}"),
            project_color: None,
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            owner: None,
            created: "2026-09-06T00:00:00Z".into(),
            updated: "2026-09-06T00:00:00Z".into(),
            started: None,
            completed: None,
            tags: vec![],
            source: None,
            step: None,
            model: None,
            agent: None,
            origin: None,
            depends: vec![],
            wake_on: vec![],
            parent: None,
            in_lane: None,
            child_count: 0,
            open_descendant_count: 0,
            claim: None,
            park: None,
            escalation: None,
            periodic: None,
            deferred: None,
            needs: vec![],
            parallel,
            lane: false,
            reviewed: false,
            awaiting_user: false,
            reviewed_at: None,
            closed: None,
            needs_review: false,
            review_reason: None,
        }
    }

    fn recurring(id: &str) -> TaskSummary {
        let mut row = row(id, false);
        row.periodic = Some(PeriodicInfo {
            every: "30d".into(),
            last_done: None,
            due: None,
            due_now: true,
        });
        row
    }

    fn layout_at(width: usize) -> crate::table::Layout {
        crate::table::Layout {
            width: Some(width),
            headers: false,
        }
    }

    fn render_tasks(
        rows: &[TaskSummary],
        date: DateColumn,
        painter: &Painter,
        parallel: bool,
        kind: bool,
        layout: crate::table::Layout,
    ) -> String {
        let marks = Marks {
            parallel,
            kind,
            ..Marks::of(rows)
        };
        let built: Vec<_> = rows
            .iter()
            .map(|row| task_row(row, date, painter, marks, 0))
            .collect();
        crate::table::render(&task_columns(marks), &built, &layout, None, painter)
    }

    fn render_tree(
        nodes: &[TreeNode],
        painter: &Painter,
        parallel: bool,
        kind: bool,
        layout: crate::table::Layout,
    ) -> String {
        let marks = Marks {
            parallel,
            kind,
            ..Marks::of_tree(nodes)
        };
        let mut built = Vec::new();
        tree_rows(nodes, 0, painter, marks, &mut built);
        crate::table::render(&task_columns(marks), &built, &layout, None, painter)
    }

    fn plain() -> Painter {
        Painter::new(ColorMode::Never, Format::Pretty, false)
    }

    #[test]
    fn the_marker_column_is_absent_when_nothing_is_marked() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_parallel(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(!text.contains("||"), "{text}");
        assert!(text.contains("todo  2026-09-06"), "{text}");
    }

    #[test]
    fn the_type_column_is_absent_when_nothing_recurs() {
        let rows = [row("xx-000001", false), row("xx-000002", false)];
        assert!(!any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(!text.contains(" p "), "{text}");
        assert!(text.contains("todo  2026-09-06"), "{text}");
    }

    #[test]
    fn a_recurring_row_carries_the_type_letter_and_keeps_its_neighbour_aligned() {
        let rows = [recurring("xx-000001"), row("xx-000002", false)];
        assert!(any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            true,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo  p  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo     2026-09-06"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn the_type_letter_is_painted_but_still_ascii() {
        let rows = [recurring("xx-000001")];
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &colored,
            false,
            true,
            crate::table::Layout::default(),
        );
        assert!(text.contains("\x1b[1mp\x1b[0m "), "{text:?}");
    }

    #[test]
    fn any_type_tree_finds_a_recurring_descendant() {
        let nodes = vec![TreeNode {
            summary: row("xx-000001", false),
            children: vec![TreeNode {
                summary: recurring("xx-000002"),
                children: vec![],
            }],
        }];
        assert!(any_type_tree(&nodes));
    }

    #[test]
    fn a_lane_row_carries_the_lane_mark_in_the_type_column() {
        let mut lane = row("xx-000001", false);
        lane.lane = true;
        let rows = [lane, row("xx-000002", false)];
        assert!(any_type(&rows));
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            true,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("todo  ≡  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo     2026-09-06"), "{}", lines[1]);
    }

    #[test]
    fn mixed_siblings_share_one_column_layout() {
        // The case a per-call decision inside `table` gets wrong: an unmarked sibling
        // must reserve the same width as its marked neighbour, or the date and title
        // shift between adjacent lines.
        let nodes = vec![
            TreeNode {
                summary: row("xx-000001", true),
                children: vec![],
            },
            TreeNode {
                summary: row("xx-000002", false),
                children: vec![],
            },
        ];
        assert!(any_parallel_tree(&nodes));
        let text = render_tree(
            &nodes,
            &plain(),
            true,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("todo  ||  2026-09-06"), "{}", lines[0]);
        assert!(lines[1].contains("todo      2026-09-06"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn ids_of_different_lengths_share_one_column() {
        // `--all-projects` mixes prefixes, so ids differ in length; the id column must
        // be as wide as the longest one, or every later column shifts per row.
        let out = Output::List(ListOut {
            tasks: vec![row("forge-0e720e", false), row("nrp-8e8fde", false)],
            halts: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        let text = pretty(&out, &plain(), &crate::table::Layout::default());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[1].starts_with("nrp-8e8fde    P2"), "{}", lines[1]);
        assert_eq!(
            lines[0].find("2026-09-06"),
            lines[1].find("2026-09-06"),
            "dates must land in the same column:\n{text}"
        );
    }

    #[test]
    fn any_parallel_tree_finds_a_marked_descendant() {
        let nodes = vec![TreeNode {
            summary: row("xx-000001", false),
            children: vec![TreeNode {
                summary: row("xx-000002", true),
                children: vec![],
            }],
        }];
        assert!(any_parallel_tree(&nodes));
    }

    fn dated() -> Painter {
        let today = ::time::Date::from_calendar_date(2026, ::time::Month::September, 6).unwrap();
        let palette = crate::palette::Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false)
            .with_recency(crate::style::Recency::new(today, &palette))
    }

    fn scaled() -> Painter {
        let palette =
            crate::palette::Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7")
                .unwrap();
        Painter::new(ColorMode::Always, Format::Pretty, false).with_priority_scale(
            crate::style::PriorityScale::new(palette.magenta.unwrap(), &palette),
        )
    }

    #[test]
    fn the_priority_column_carries_the_priority_role() {
        let mut urgent = row("xx-000001", false);
        urgent.priority = 0;
        let rows = [urgent, row("xx-000002", false)];
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &scaled(),
            false,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[1;38;2;215;95;215mP0\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(
            lines[1].contains("\x1b[38;2;163;88;159mP2\x1b[0m"),
            "{:?}",
            lines[1]
        );
    }

    #[test]
    fn the_date_column_carries_the_recency_role() {
        // row is updated 2026-09-06, the painter's today: full cyan.
        let rows = [row("xx-000001", false)];
        let text = render_tasks(
            &rows,
            DateColumn::Updated,
            &dated(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert!(
            text.contains("\x1b[38;2;0;215;255m2026-09-06\x1b[0m"),
            "{text:?}"
        );
    }

    #[test]
    fn a_due_column_paints_now_as_today_and_leaves_a_dash_plain() {
        let due_now = recurring("xx-000001");
        let undated = row("xx-000002", false);
        let text = render_tasks(
            &[due_now, undated],
            DateColumn::Due,
            &dated(),
            false,
            false,
            crate::table::Layout::default(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("\x1b[38;2;0;215;255mnow\x1b[0m"),
            "{:?}",
            lines[0]
        );
        assert!(lines[1].contains("todo  -    "), "{:?}", lines[1]);
        assert!(!lines[1].contains("38;2"), "{:?}", lines[1]);
    }

    fn long_row() -> TaskSummary {
        let mut wrapped = row("xx-000001", false);
        wrapped.title = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu".into();
        wrapped
    }

    #[test]
    fn a_list_row_lays_out_its_columns_and_wraps_its_title_in_place() {
        let layout = crate::table::Layout {
            width: Some(80),
            headers: false,
        };
        let built: Vec<crate::table::Row> = [long_row()]
            .iter()
            .map(|row| task_row(row, DateColumn::Updated, &plain(), Marks::default(), 0))
            .collect();
        let text = crate::table::render(
            &task_columns(Marks::default()),
            &built,
            &layout,
            None,
            &plain(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        let title = lines[0].find("alpha").unwrap();
        for line in &lines[1..] {
            assert_eq!(line.len() - line.trim_start().len(), title, "{text}");
        }
        assert!(
            lines
                .iter()
                .all(|line| crate::table::visible_width(line) <= 80),
            "{text}"
        );
        let mut words = vec![&lines[0][title..]];
        words.extend(lines[1..].iter().map(|line| line.trim_start()));
        assert_eq!(words.join(" "), long_row().title, "{text}");
    }

    #[test]
    fn a_tree_indents_children_inside_the_id_column() {
        let child = TreeNode {
            summary: row("xx-000002", false),
            children: vec![],
        };
        let root = TreeNode {
            summary: row("xx-000001", false),
            children: vec![child],
        };
        let mut built = Vec::new();
        tree_rows(&[root], 0, &plain(), Marks::default(), &mut built);
        let text = crate::table::render(
            &task_columns(Marks::default()),
            &built,
            &crate::table::Layout::default(),
            None,
            &plain(),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("xx-000001"), "{text}");
        assert!(lines[1].starts_with("  xx-000002"), "{text}");
    }

    #[test]
    fn wide_characters_fit_the_terminal_columns() {
        let mut row = long_row();
        row.title = "界".repeat(40);
        let text = render_tasks(
            &[row],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        for line in text.lines() {
            let columns: usize = line.chars().map(|c| if c == '界' { 2 } else { 1 }).sum();
            assert!(columns <= 80, "{columns} columns: {line:?}");
        }
    }

    #[test]
    fn a_spaces_only_title_keeps_the_row_without_trailing_whitespace() {
        let mut row = long_row();
        row.title = " ".repeat(40);
        let text = render_tasks(
            &[row],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
        assert!(text.starts_with("xx-000001"), "{text:?}");
        assert_eq!(text.trim_end(), text.lines().next().unwrap(), "{text:?}");
    }

    #[test]
    fn a_row_that_fits_the_width_stays_one_line_and_byte_identical() {
        let rows = [row("xx-000001", false)];
        let wrapped = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(80),
        );
        let unwrapped = render_tasks(
            &rows,
            DateColumn::Updated,
            &plain(),
            false,
            false,
            crate::table::Layout::default(),
        );
        assert_eq!(wrapped.lines().count(), 1, "{wrapped:?}");
        assert_eq!(wrapped, unwrapped);
    }

    #[test]
    fn a_row_below_the_title_floor_stays_unwrapped() {
        // Fixed cells and gaps take 42 columns, so at width 60 only 18 remain
        // for the title: below the floor, the row overflows unwrapped.
        let text = render_tasks(
            &[long_row()],
            DateColumn::Updated,
            &plain(),
            false,
            false,
            layout_at(60),
        );
        assert_eq!(text.lines().count(), 1, "{text:?}");
    }

    #[test]
    fn a_due_cadence_is_painted_once_when_wrapped() {
        let mut due = recurring("xx-000001");
        due.title = long_row().title;
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render_tasks(
            &[due],
            DateColumn::Due,
            &colored,
            false,
            true,
            layout_at(80),
        );
        assert!(text.contains("\x1b[1m"), "{text:?}");
        assert!(!text.contains("\x1b[1m\x1b[1m"), "{text:?}");
    }

    #[test]
    fn a_quiet_park_marker_wraps_with_the_row_and_is_painted_once() {
        let mut quiet = long_row();
        quiet.park = Some(ParkInfo {
            at: "2026-09-06T00:00:00Z".into(),
            next_step: "rerun the preflight".into(),
            waiting_on: crate::claims::WaitingOn::User,
            reason: Some(crate::claims::Reason::Quiet),
            needs: Some(crate::claims::Needs::Idle),
            minutes: Some(40),
            session: "s".into(),
            owner: "o".into(),
            host: "h".into(),
            worktree: "w".into(),
        });
        let marker = "waits on user, quiet; idle, 40 min";
        let render = |painter: &Painter, layout| {
            render_tasks(
                std::slice::from_ref(&quiet),
                DateColumn::Updated,
                painter,
                false,
                false,
                layout,
            )
        };
        assert!(render(&plain(), crate::table::Layout::default()).contains(marker));
        let wrapped = render(&plain(), layout_at(80));
        assert!(wrapped.lines().count() > 1, "{wrapped:?}");
        let joined: Vec<&str> = wrapped.lines().map(str::trim).collect();
        assert!(joined.join(" ").contains(marker), "{wrapped:?}");
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let text = render(&colored, layout_at(80));
        assert!(text.contains("\x1b[1m"), "{text:?}");
        assert!(!text.contains("\x1b[1m\x1b[1m"), "{text:?}");
    }

    #[test]
    fn a_tree_node_wraps_under_its_own_indentation() {
        let mut child = long_row();
        child.id = "xx-000002".into();
        let nodes = vec![TreeNode {
            summary: long_row(),
            children: vec![TreeNode {
                summary: child,
                children: vec![],
            }],
        }];
        let text = render_tree(&nodes, &plain(), false, false, layout_at(80));
        let lines: Vec<&str> = text.lines().collect();
        let child_line = lines
            .iter()
            .position(|line| line.starts_with("  xx-000002"))
            .expect("child row: {text:?}");
        let title_start = lines[child_line].find("alpha").expect("child title");
        let continuation = lines[child_line + 1];
        assert_eq!(
            continuation.len() - continuation.trim_start().len(),
            title_start,
            "{text:?}"
        );
    }

    fn parked_with(next_step: &str, reason: Option<crate::claims::Reason>) -> ParkedRow {
        let mut parked = ParkedRow::resolved(long_row(), crate::model::Phase::Implementing);
        let quiet = reason == Some(crate::claims::Reason::Quiet);
        parked.park = Some(ParkInfo {
            at: "2026-09-06T00:00:00Z".into(),
            next_step: next_step.into(),
            waiting_on: crate::claims::WaitingOn::User,
            reason,
            needs: quiet.then_some(crate::claims::Needs::Headless),
            minutes: quiet.then_some(39),
            session: "s".into(),
            owner: "o".into(),
            host: "h".into(),
            worktree: "/a/long/checkout/path/that/runs/well/past/the/width/.worktrees/x".into(),
        });
        parked
    }

    const LONG_STEP: &str = "Agent: run the prepared smoke pilot and the full matrix, write and \
        attach the evidence, then hand the contact sheet to the owner for a judgement";

    #[test]
    fn a_parked_row_wraps_its_title() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let text = parked_table(
            &[parked_with("next step", None)],
            &plain(),
            &layout,
            None,
            false,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() > 1, "{text}");
        let title_start = lines[0].find("alpha").unwrap();
        assert_eq!(
            lines[1].len() - lines[1].trim_start().len(),
            title_start,
            "{text}"
        );
    }

    #[test]
    fn a_parked_tables_wait_column_fits_its_longest_stop() {
        let rows = [
            parked_with("a", Some(crate::claims::Reason::Quiet)),
            parked_with("b", Some(crate::claims::Reason::Review)),
        ];
        let text = parked_table(
            &rows,
            &plain(),
            &crate::table::Layout::default(),
            None,
            false,
        );
        let dates: Vec<usize> = text
            .lines()
            .map(|line| line.find("2026-09-06").unwrap())
            .collect();
        assert_eq!(dates.len(), 2, "{text}");
        assert_eq!(dates[0], dates[1], "{text}");
        assert!(text.contains("39 min  2026-09-06"), "{text}");
    }

    #[test]
    fn a_parked_rows_next_step_wraps_beside_its_title() {
        let layout = crate::table::Layout {
            width: Some(160),
            headers: false,
        };
        let text = parked_table(
            &[parked_with(LONG_STEP, None)],
            &plain(),
            &layout,
            None,
            false,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains("alpha") && lines[0].contains("Agent:"),
            "{text}"
        );
        assert!(
            lines.len() > 1 && !lines.iter().any(|l| l.contains("next:")),
            "{text}"
        );
        assert!(
            lines.iter().all(|l| crate::table::visible_width(l) <= 160),
            "{text}"
        );
    }

    #[test]
    fn a_narrow_parked_table_moves_next_under_the_row() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let text = parked_table(
            &[parked_with(LONG_STEP, None)],
            &plain(),
            &layout,
            None,
            false,
        );
        assert!(text.contains("        next:   Agent:"), "{text}");
    }

    #[test]
    fn an_unresolved_park_renders_placeholders() {
        let mut row = parked_with("x", None);
        row.status = None;
        row.phase = None;
        let text = parked_table(
            &[row],
            &plain(),
            &crate::table::Layout::default(),
            None,
            false,
        );
        assert!(text.starts_with("xx-000001  ?  -  user"), "{text}");
    }

    #[test]
    fn quiet_keeps_its_checkout_whole_under_the_row() {
        let layout = crate::table::Layout {
            width: Some(100),
            headers: false,
        };
        let row = parked_with(LONG_STEP, Some(crate::claims::Reason::Quiet));
        let worktree = row.park.as_ref().unwrap().worktree.clone();
        let text = quiet_table(&[row], &plain(), &layout);
        assert!(
            text.contains(&format!("        in:     {worktree}\n")),
            "{text}"
        );
    }
}
