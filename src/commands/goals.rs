//! `tasks goals`: the goals the user asked for, across projects (records spec
//! docs/user-guide.md).

use super::ReadCtx;
use crate::error::Result;
use crate::output::{GoalsOut, Output, ParkEvidence};

/// `closed` adds the goals closed inside [`crate::goals::CLOSED_WINDOW`].
pub fn run(mut ctx: ReadCtx, closed: bool, order: crate::goals::GoalOrder) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let observed_at = crate::time::now();
    let now = crate::time::parse(&observed_at)?;
    let goals = crate::goals::goal_rows(&all, &claims, &ctx.registry, now, closed, order, |id| {
        super::list::scan_lookup(&ctx, &all, id)
    })?;
    let colors = ctx.project_colors();
    let mut out = Output::Goals(GoalsOut {
        order,
        observed_at,
        park_evidence: ParkEvidence {
            host: crate::claims::hostname(),
            scope: "host_local",
        },
        goals,
        warnings: std::mem::take(&mut ctx.warnings),
    });
    out.paint_projects(&colors);
    Ok(out)
}
