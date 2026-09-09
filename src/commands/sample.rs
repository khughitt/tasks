//! `tasks sample`: a uniform draw from the curable pool, for the curate skill.
//!
//! The pool is the definition of "worth a maintenance look": open but not in hand
//! (`idea`, `todo`, `blocked`), not live-claimed, and not touched within the age window.
//! Age and status exclusions are silent. A live-claim omission is reported with the
//! message `ready` uses (minus its takeover hint), and a task whose latest note is a
//! `curate:` note still carrying a `proposal:` for the human is reported as pending, so
//! a reader can tell a small pool from a busy one and an unanswered proposal never
//! re-enters the pool by age alone.

use super::ReadCtx;
use crate::error::Result;
use crate::model::{Status, Task};
use crate::output::{DateColumn, ListOut, Output, TaskSummary};

/// The proposal text of a task whose most recent note is a curate note awaiting the
/// human's decision. Any later note clears it, which is how the human answers.
fn pending_proposal(task: &Task) -> Option<&str> {
    let text = task.notes.last()?.text.as_str();
    let rest = text.strip_prefix("curate:")?;
    rest.split_once("proposal:")
        .map(|(_, proposal)| proposal.trim())
}

pub fn sample(
    mut ctx: ReadCtx,
    count: usize,
    older_than: u64,
    seed: Option<u64>,
) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    // Bounded to <= 36500 at the CLI, so the cast is exact and the subtraction stays far
    // inside OffsetDateTime's range. Zero means no age check at all: a future-dated
    // record from clock skew is still admitted.
    let cutoff = (older_than > 0)
        .then(|| time::OffsetDateTime::now_utc() - time::Duration::days(older_than as i64));

    let mut pool: Vec<&Task> = Vec::new();
    for task in &all {
        if !matches!(task.status, Status::Idea | Status::Todo | Status::Blocked) {
            continue;
        }
        if let Some(cutoff) = cutoff
            && crate::time::parse(&task.updated)? > cutoff
        {
            continue;
        }
        if let Some(claim) = claims.live(&task.id) {
            ctx.warnings
                .push(super::list::claim_omission(&task.id, claim));
            continue;
        }
        if let Some(proposal) = pending_proposal(task) {
            ctx.warnings
                .push(format!("{} pending: {proposal}", task.id));
            continue;
        }
        pool.push(task);
    }
    // Scan order depends on the filesystem; a seed must not.
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    let pool_size = pool.len();
    if pool_size < count {
        let window = if older_than == 0 {
            "any age".to_string()
        } else {
            format!("not updated in {older_than} days")
        };
        ctx.warnings.push(format!(
            "asked for {count}; the pool holds {pool_size} (open, unclaimed, not pending, {window})"
        ));
    }

    let mut rng = match seed {
        Some(seed) => fastrand::Rng::with_seed(seed),
        None => fastrand::Rng::new(),
    };
    // Partial Fisher–Yates: the first `take` slots are a uniform draw without replacement.
    // `u64`, not `usize`: fastrand's usize generator differs between 32- and 64-bit
    // targets, and a seed must reproduce the same draw everywhere.
    let take = count.min(pool_size);
    for i in 0..take {
        let j = rng.u64(i as u64..pool_size as u64) as usize;
        pool.swap(i, j);
    }
    let drawn = &pool[..take];

    Ok(Output::List(ListOut {
        tasks: drawn
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry))
            .collect(),
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    }))
}
