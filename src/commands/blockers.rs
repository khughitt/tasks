//! `tasks blockers`: one captured scan, the downstream-impact analysis of
//! `crate::blockers`, then this host's park and claim evidence on the ranked rows. Writes
//! nothing. See docs/specs/2026-10-25-blocker-ranking-design.md.

use super::ReadCtx;
use crate::blockers::{Coverage, CoverageMode, Impact};
use crate::claims::{ClaimSnapshot, WaitingOn};
use crate::cli::BlockersArgs;
use crate::error::Result;
use crate::model::Status;
use crate::output::{
    Attention, BlockerRow, BlockersOut, ClaimInfo, Output, ParkEvidence, ParkInfo,
};
use crate::scope::Scope;
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// Always stated: the persistence measures are a separate, unshipped collection.
pub const HISTORY_UNAVAILABLE: &str = "history metrics (blocking since, confirmed scope passes) \
     are unavailable; this report ranks observed downstream impact only";

pub fn run(mut ctx: ReadCtx, args: BlockersArgs) -> Result<Output> {
    let observed_at = crate::time::now();
    let now = crate::time::parse(&observed_at)?;
    let mode = if args.scope.all_projects {
        CoverageMode::AllProjects
    } else if args.scope.group.is_some() {
        CoverageMode::Group
    } else if args.scope.project.is_some() {
        CoverageMode::Project
    } else {
        CoverageMode::Local
    };
    let requested = match &ctx.scope {
        Scope::Local(project) => vec![project.prefix.clone()],
        Scope::All { members, .. } => members.clone(),
    };
    let scanned = ctx.scope.prefixes();
    let unreachable = requested
        .iter()
        .filter(|prefix| !scanned.contains(prefix))
        .cloned()
        .collect();
    let coverage = Coverage::new(mode, requested, scanned.clone(), unreachable);

    // The captured scan answers first, as for `ready`; the read scope answers the rest.
    let all = ctx.scope.scan()?;
    let analysis = crate::blockers::build(&all, &ctx.registry, coverage, |id| {
        match all.iter().find(|task| task.id == *id) {
            Some(task) => Ok(Some(task.clone())),
            None => ctx.resolve_task(id),
        }
    })?;

    // Every ranked prefix's store, before truncation, so a malformed one fails even at -n 0.
    let prefixes: BTreeSet<&str> = scanned
        .iter()
        .map(String::as_str)
        .chain(analysis.rows.iter().map(|row| row.task.id.prefix.as_str()))
        .collect();
    let claims = ClaimSnapshot::load(prefixes.into_iter())?;

    let total = analysis.rows.len();
    let blockers: Vec<BlockerRow> = analysis
        .rows
        .iter()
        .take(args.limit)
        .enumerate()
        .map(|(at, impact)| row(at + 1, impact, &claims, now))
        .collect();

    let mut warnings = std::mem::take(&mut ctx.warnings);
    warnings.push(HISTORY_UNAVAILABLE.into());
    let coverage = analysis.coverage;
    if !coverage.complete {
        warnings.push(
            "coverage is partial: counts are lower bounds within the requested projects, and \
             ranks are observed ranks that may change when missing coverage returns"
                .into(),
        );
    }
    if !coverage.unfollowed_targets.is_empty() {
        let ids: Vec<String> = coverage
            .unfollowed_targets
            .iter()
            .map(ToString::to_string)
            .collect();
        warnings.push(format!(
            "dependency paths through out-of-scope targets were not followed: {}; in-scope \
             impact or rows may be missing; rerun with --all-projects to follow them",
            ids.join(", ")
        ));
    }
    if !analysis.unresolved.is_empty() {
        warnings.push(format!(
            "{} dependency reference(s) could not be resolved; see unresolved",
            analysis.unresolved.len()
        ));
    }

    let colors = ctx.project_colors();
    let mut out = Output::Blockers(Box::new(BlockersOut {
        observed_at,
        coverage,
        park_evidence: ParkEvidence {
            host: crate::claims::hostname(),
            scope: "host_local",
        },
        goal_detection: "structural_only",
        ranking: "transitive_work,direct_work,priority,created,id",
        total,
        shown: blockers.len(),
        omitted: total - blockers.len(),
        limit: args.limit,
        blockers,
        unresolved: analysis.unresolved,
        cycles: analysis.cycles,
        warnings,
    }));
    out.paint_projects(&colors);
    Ok(out)
}

/// One ranked row with this host's evidence. A user wait needs a recorded signal; nothing
/// here ever says a row is safe for an agent to run.
fn row(rank: usize, impact: &Impact, claims: &ClaimSnapshot, now: OffsetDateTime) -> BlockerRow {
    let task = &impact.task;
    let park = claims.park(&task.id);
    let attention = match park {
        Some(park) if park.waiting_on == WaitingOn::User => Attention::User,
        None if task.status == Status::Idea && task.reviewed && task.awaiting_user => {
            Attention::User
        }
        _ => Attention::Unknown,
    };
    BlockerRow {
        rank,
        id: task.id.to_string(),
        title: task.title.clone(),
        project_color: None,
        status: task.status,
        priority: task.priority,
        goal: impact.goal,
        cycle: impact.cycle,
        direct_work_count: impact.direct_work_count,
        transitive_work_count: impact.transitive_work_count,
        direct_goal_count: impact.direct_goal_count,
        transitive_goal_count: impact.transitive_goal_count,
        direct_work_by_status: impact.direct_work_by_status,
        transitive_work_by_status: impact.transitive_work_by_status,
        dependencies_cleared_direct_count: impact.dependencies_cleared_direct_count,
        unmet_dependency_count: impact.unmet_dependency_count,
        unresolved_dependency_count: impact.unresolved_dependency_count,
        attention,
        awaiting_user: task.awaiting_user,
        task_needs: task.needs.clone(),
        defer: task
            .defer
            .filter(|_| crate::defer::is_deferred(task, now))
            .map(|defer| defer.to_string()),
        park: park.map(ParkInfo::of),
        claim: claims
            .get(&task.id)
            .filter(|(_, live)| !live.is_stale())
            .map(|(claim, live)| ClaimInfo::of(claim, live)),
        direct_examples: impact.direct_examples.clone(),
    }
}
