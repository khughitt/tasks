//! `tasks lanes`, and the `lanes` rows `prime` shares with it (lanes design §5). The
//! builder is `crate::lanes`; this module gathers what it reads.

use super::ReadCtx;
use crate::claims::ClaimSnapshot;
use crate::cli::WithoutArgs;
use crate::error::Result;
use crate::halt::HaltSnapshot;
use crate::holds::{HoldSnapshot, Mine};
use crate::lanes::{Inputs, LaneRow};
use crate::model::{Complexity, Task, TaskId};
use crate::needs::Without;
use crate::output::{LanesOut, Output};
use std::collections::HashMap;
use time::OffsetDateTime;

pub fn run(
    mut ctx: ReadCtx,
    without: WithoutArgs,
    max_complexity: Option<String>,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    // The strict-flag, lenient-variable union `ready`, `next`, and `prime` build.
    let without = Without::from_env(&without.without, &super::list::vocabularies(&ctx.scope))?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let halts = super::list::halt_snapshots(&mut ctx, &all);
    let lanes = rows(&mut ctx, &all, &claims, &halts, cutoff, &without, now)?;
    Ok(Output::Lanes(LanesOut {
        lanes,
        warnings: ctx.warnings,
    }))
}

/// Every open, unshelved lane in scope as a `LaneRow`. Dependencies are resolved only for
/// `dependency_readers`, as `ready_tasks` resolves only its candidates' (lanes design §5.1
/// step 1), so a dependency that only closed, shelved, or paused work names can never
/// fail the view or `prime`. Holds are read as `ready` reads them (§4.5).
pub(super) fn rows(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    halts: &HashMap<String, HaltSnapshot>,
    cutoff: Option<Complexity>,
    without: &Without,
    now: OffsetDateTime,
) -> Result<Vec<LaneRow>> {
    if !all
        .iter()
        .any(|task| task.lane && crate::hierarchy::is_active(task))
    {
        return Ok(Vec::new());
    }
    let mut closed: HashMap<TaskId, Option<bool>> = HashMap::new();
    for task in crate::lanes::dependency_readers(all, claims, &ctx.registry, now) {
        for dependency in &task.depends {
            if closed.contains_key(dependency) {
                continue;
            }
            let value = super::list::resolve_dependency(ctx, all, dependency)?
                .map(|found| !found.status.is_open());
            closed.insert(dependency.clone(), value);
        }
    }
    let (holds, mine) = load_holds(ctx, now)?;
    let vocabularies = super::list::vocabularies(&ctx.scope);
    let dependency = |id: &TaskId| closed.get(id).copied().flatten();
    Ok(crate::lanes::build(&Inputs {
        all,
        claims,
        registry: &ctx.registry,
        dependency: &dependency,
        halts,
        cutoff,
        without,
        holds: &holds,
        vocabularies: &vocabularies,
        mine: &mine,
        now,
    }))
}

/// The hold gate's inputs, read as `retain_unheld` reads them: a scope whose projects
/// declare no exclusive need reads no claim store and resolves no identity, and an empty
/// snapshot needs no identity. A warning another section of the same command already gave
/// (`prime` also runs `retain_unheld`) is not repeated.
fn load_holds(ctx: &mut ReadCtx, now: OffsetDateTime) -> Result<(HoldSnapshot, Mine)> {
    let exclusive = ctx
        .scope
        .projects()
        .iter()
        .any(|project| project.needs.values().any(|need| need.exclusive));
    if !exclusive {
        return Ok((HoldSnapshot::default(), Mine::default()));
    }
    let (holds, mut fresh) = HoldSnapshot::load(&ctx.registry, now);
    let mine = if holds.is_empty() {
        Mine::default()
    } else {
        super::list::view_holds(&holds, &mut fresh)?
    };
    for warning in fresh {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    Ok((holds, mine))
}
