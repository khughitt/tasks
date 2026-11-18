//! `tasks goals`: the goals the user asked for, across projects (records spec
//! docs/user-guide.md). Every number counts the goal's members, read through the registry
//! whatever the scope, so narrowing never changes a row (docs/task-model.md).

use super::ReadCtx;
use crate::error::Result;
use crate::membership::Membership;
use crate::output::{GoalsOut, Output, ParkEvidence};
use std::collections::BTreeSet;

/// `closed` adds the goals closed inside [`crate::goals::CLOSED_WINDOW`].
pub fn run(mut ctx: ReadCtx, closed: bool, order: crate::goals::GoalOrder) -> Result<Output> {
    let all = ctx.scope.scan()?;
    let prefixes = ctx.scope.prefixes();
    let observed_at = crate::time::now();
    let now = crate::time::parse(&observed_at)?;
    let mut membership = Membership::new(&ctx.registry, &prefixes, &all);
    let records = crate::hierarchy::project_records(&all, &prefixes, &ctx.registry);
    let mut indexes = crate::hierarchy::ProjectIndexes::new();
    for prefix in records.keys() {
        let index = membership
            .hierarchy(prefix)?
            .expect("preseeded project in Membership has an index");
        indexes.insert(prefix.clone(), index);
    }
    let hierarchy = crate::hierarchy::HierarchyView::new(records, indexes, &ctx.registry);
    // Only active goals carry attention, so only their members widen the claim stores.
    let (active, _) = crate::goals::shown(&all, &hierarchy, &ctx.registry, now, closed)?;
    for goal in active {
        membership.members(goal)?;
    }
    let stores: BTreeSet<String> = prefixes
        .into_iter()
        .chain(membership.reached_prefixes())
        .collect();
    let claims = crate::claims::ClaimSnapshot::load(stores.iter().map(String::as_str))?;
    let goals = crate::goals::goal_rows(
        &all,
        &hierarchy,
        &mut membership,
        &claims,
        &ctx.registry,
        now,
        closed,
        order,
    )?;
    // A project the scope already reported as unreachable is not reported twice.
    for warning in membership.take_warnings() {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    let colors = ctx.project_colors();
    let mut out = Output::Goals(GoalsOut {
        include_closed: closed,
        order,
        observed_at,
        park_evidence: ParkEvidence {
            host: crate::claims::hostname(),
            scope: "host_local",
        },
        goals,
        warnings: std::mem::take(&mut ctx.warnings),
    });
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}
