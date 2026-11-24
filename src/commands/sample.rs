//! `tasks sample`: a uniform draw from the curable pool, for the curate skill.
//!
//! The pool is the definition of "worth a maintenance look": open but not in hand
//! (`idea`, `todo`, `blocked`), not live-claimed, and not touched within the age window.
//! Age and status exclusions are silent. A live-claim omission is reported with the
//! message `ready` uses (minus its takeover hint), and a task whose latest note is a
//! `curate:` or `scope:` note still carrying a `proposal:` for the human is reported as pending, so
//! a reader can tell a small pool from a busy one and an unanswered proposal never
//! re-enters the pool by age alone.

use super::ReadCtx;
use crate::error::Result;
use crate::model::{Status, Task};
use crate::output::{DateColumn, ListOut, Output, TaskSummary};

pub(super) const DEFAULT_AGE_DAYS: i64 = 7;

pub(super) struct Window {
    pub days: i64,
    cutoff: Option<time::OffsetDateTime>,
}

impl Window {
    pub(super) fn new(days: i64, now: time::OffsetDateTime) -> Self {
        // The CLI caps the day count. Zero disables age filtering even with clock skew.
        Self {
            days,
            cutoff: (days > 0).then(|| now - time::Duration::days(days)),
        }
    }

    pub(super) fn recent(&self, task: &Task) -> Result<bool> {
        match self.cutoff {
            Some(cutoff) => Ok(crate::time::parse(&task.updated)? > cutoff),
            None => Ok(false),
        }
    }
}

pub fn sample(
    mut ctx: ReadCtx,
    limit: usize,
    older_than: i64,
    seed: Option<u64>,
) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let window = Window::new(older_than, now);

    let mut pool: Vec<&Task> = Vec::new();
    for task in &all {
        if !matches!(task.status, Status::Idea | Status::Todo | Status::Blocked) {
            continue;
        }
        if crate::defer::is_deferred(task, now) {
            continue;
        }
        if window.recent(task)? {
            continue;
        }
        if let Some(claim) = claims.held(&task.id) {
            ctx.warnings
                .push(super::list::claim_omission(&task.id, claim));
            continue;
        }
        let held_decisions = crate::decisions::holds_curate(task);
        if !held_decisions.is_empty() {
            for (_, data) in held_decisions {
                let first_line = data.text.lines().next().unwrap_or("");
                ctx.warnings.push(format!(
                    "{} pending: [{}] {first_line}",
                    task.id,
                    data.kind.as_str()
                ));
            }
            continue;
        }
        if let Some(proposal) = crate::proposal::pending(task) {
            ctx.warnings
                .push(format!("{} pending: {proposal}", task.id));
            continue;
        }
        pool.push(task);
    }
    // Scan order depends on the filesystem; a seed must not.
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    let pool_size = pool.len();
    if pool_size < limit {
        let window = if older_than == 0 {
            "any age".to_string()
        } else {
            format!("not updated in {older_than} days")
        };
        ctx.warnings.push(format!(
            "asked for {limit}; the pool holds {pool_size} (open, unclaimed, not pending, {window})"
        ));
    }

    let mut rng = match seed {
        Some(seed) => fastrand::Rng::with_seed(seed),
        None => fastrand::Rng::new(),
    };
    // Partial Fisher–Yates: the first `take` slots are a uniform draw without replacement.
    // `u64`, not `usize`: fastrand's usize generator differs between 32- and 64-bit
    // targets, and a seed must reproduce the same draw everywhere.
    let take = limit.min(pool_size);
    for i in 0..take {
        let j = rng.u64(i as u64..pool_size as u64) as usize;
        pool.swap(i, j);
    }
    let drawn = &pool[..take];

    let deps = super::list::review_deps(&mut ctx, &all)?;
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(&all, &prefixes, &ctx.registry);
    let colors = ctx.project_colors();
    let mut out = Output::List(ListOut {
        tasks: drawn
            .iter()
            .map(|task| TaskSummary::of(task, &hierarchy, &deps, Some(&claims), &ctx.registry, now))
            .collect(),
        halts: vec![],
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    });
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}
