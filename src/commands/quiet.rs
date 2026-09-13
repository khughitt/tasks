//! `tasks quiet`: work parked waiting for an idle host.

use super::ReadCtx;
use super::parked::{Prefer, rows_preferring};
use crate::claims::Reason;
use crate::error::Result;
use crate::output::{Output, ParkedRow, QuietOut};

pub fn run(mut ctx: ReadCtx, limit: Option<usize>) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let rows = rows_preferring(
        &mut ctx,
        &all,
        &claims,
        now,
        Prefer::Recorded,
        Some(Reason::Quiet),
    )?;
    let mut tasks: Vec<ParkedRow> = rows
        .into_iter()
        .filter(|row| {
            row.park
                .as_ref()
                .is_some_and(|park| park.reason == Some(Reason::Quiet))
        })
        .filter(|row| row.status.is_none_or(|status| status.is_open()))
        .collect();
    tasks.sort_by(|a, b| {
        let priority = |row: &ParkedRow| row.priority.unwrap_or(u8::MAX);
        let at = |row: &ParkedRow| {
            row.park
                .as_ref()
                .map(|park| park.at.clone())
                .unwrap_or_default()
        };
        priority(a)
            .cmp(&priority(b))
            .then_with(|| at(a).cmp(&at(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::Quiet(QuietOut {
        tasks,
        warnings: ctx.warnings,
    }))
}
