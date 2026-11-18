use super::ReadCtx;
use crate::claims::{Liveness, WaitingOn};
use crate::cli::{FilterArgs, ListArgs};
use crate::dependency::Verdict;
use crate::error::Result;
use crate::filter::{Fields, TaskFilter, TextQuery, check_parent};
use crate::halt::{self, HaltSnapshot};
use crate::model::{Status, Task, TaskId};
use crate::needs::{Vocabularies, Without};
use crate::output::{
    Counts, DateColumn, DeferredSummary, HaltRow, ListOut, NextOut, Output, ParkedOut, ParkedRow,
    PeriodicSummary, PrimeOut, TaskSummary,
};
use crate::query::{
    Picked, Readiness, SortKey, deferred_omission, is_candidate, list_order, paused_omissions,
    readiness, sort_by_key, sort_periodic, sort_ready,
};
use crate::repo::Project;
use crate::scope::Scope;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use time::OffsetDateTime;

/// The captured scan answers first, then the read scope.
pub(super) fn scan_lookup(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<Task>> {
    match all.iter().find(|task| task.id == *id) {
        Some(task) => Ok(Some(task.clone())),
        None => ctx.resolve_task(id),
    }
}

pub(super) fn resolve_dependency(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<Task>> {
    crate::dependency::resolve(&ctx.registry, id, |id| scan_lookup(ctx, all, id))
}

/// `Some(true)` satisfied, `Some(false)` open, `None` unresolved: the known-closed map the
/// pickers consult.
pub(super) fn known_closed(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<bool>> {
    Ok(
        match crate::dependency::verdict(resolve_dependency(ctx, all, id)?.as_ref()) {
            Verdict::Satisfied => Some(true),
            Verdict::Open => Some(false),
            Verdict::Unresolved => None,
        },
    )
}

/// Whether the dependency positively satisfies; open and unresolved both hold.
pub(super) fn satisfied(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<bool> {
    Ok(
        crate::dependency::verdict(resolve_dependency(ctx, all, id)?.as_ref())
            == Verdict::Satisfied,
    )
}

/// The dependency view for rows of `scan` (docs/user-guide.md): one per scan, never
/// shared, so each row is judged against the copies its own checkout holds.
pub(super) fn review_deps(ctx: &mut ReadCtx, scan: &[Task]) -> Result<crate::review::Deps> {
    let deps = crate::review::Deps::of_scan(scan, |id| resolve_dependency(ctx, scan, id))?;
    for warning in deps.warnings(scan) {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    Ok(deps)
}

/// The supplying checkout owns missing local IDs, just as it owns records in its scan.
pub(super) fn recorded_review_deps(
    ctx: &mut ReadCtx,
    project: &Project,
    scan: &[Task],
) -> Result<crate::review::Deps> {
    let resolver = crate::resolve::Resolver::new(project, &ctx.registry, &ctx.host);
    let deps = crate::review::Deps::of_scan(scan, |id| {
        crate::dependency::resolve(&ctx.registry, id, |id| {
            match scan.iter().find(|task| task.id == *id) {
                Some(task) => Ok(Some(task.clone())),
                None => resolver.resolve_canonical_task(id),
            }
        })
    })?;
    for warning in deps.warnings(scan) {
        if !ctx.warnings.contains(&warning) {
            ctx.warnings.push(warning);
        }
    }
    Ok(deps)
}

/// `list`, and `search` when `text` is given: the text predicate narrows the same
/// selection, pools, order, and output.
pub fn list(mut ctx: ReadCtx, args: ListArgs, text: Option<TextQuery>) -> Result<Output> {
    let ListArgs {
        plan,
        reference,
        statuses,
        filter,
        sort,
        reverse,
        parked,
        waiting_on,
        periodic,
        deferred,
        scope: _,
    } = args;
    let sort = SortKey::parse(&sort)?;
    let mut filter = TaskFilter::parse(&filter, &statuses, &ctx.registry, &ctx.shorthand)?;
    if parked {
        let waiting_on = waiting_on.as_deref().map(WaitingOn::parse).transpose()?;
        return list_parked(ctx, filter, waiting_on, text.as_ref(), plan, reference);
    }
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(&all, &prefixes, &ctx.registry);
    check_parent(&filter, &all, |_| false)?;
    filter.resolve_under(&hierarchy, &ctx.registry)?;
    let deps = review_deps(&mut ctx, &all)?;
    let mut tasks = all.clone();
    tasks.retain(|task| {
        let periodic_ok = !periodic || task.every.is_some();
        let deferred_ok = !deferred || task.defer.is_some();
        // The default pool when --status is absent: open, or every status
        // for --periodic, since most of a healthy series is closed (docs/task-model.md).
        let pool_ok = !filter.statuses().is_empty() || periodic || task.status.is_open();
        periodic_ok
            && deferred_ok
            && pool_ok
            && plan
                .as_deref()
                .is_none_or(|wanted| task.plan.as_deref() == Some(wanted))
            && reference
                .as_deref()
                .is_none_or(|r| task.refs.iter().any(|e| e.reference == r))
            && filter.matches(&Fields::of_task(
                task,
                &hierarchy,
                &deps,
                &claims,
                &ctx.registry,
                now,
            ))
            && text.as_ref().is_none_or(|text| text.matches(task))
    });
    for task in &tasks {
        for dependency in &task.depends {
            if resolve_dependency(&ctx, &all, dependency)?.is_none() {
                ctx.warnings.push(format!(
                    "{}: dependency {dependency} is unreachable",
                    task.id
                ));
            }
        }
    }
    if periodic {
        sort_periodic(&mut tasks, now);
    } else if deferred {
        // Soonest first, due ones leading; every row has a date by the filter above.
        tasks.sort_by(|a, b| a.defer.cmp(&b.defer).then_with(|| a.id.cmp(&b.id)));
    } else {
        sort_by_key(&mut tasks, sort);
        if reverse {
            tasks.reverse();
        }
    }
    let colors = ctx.project_colors();
    let mut out = Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &hierarchy, &deps, Some(&claims), &ctx.registry, now))
            .collect(),
        halts: vec![],
        warnings: ctx.warnings,
        date: if periodic || deferred {
            DateColumn::Due
        } else {
            sort.date_column()
        },
    });
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}

fn list_parked(
    mut ctx: ReadCtx,
    mut filter: TaskFilter,
    waiting_on: Option<WaitingOn>,
    text: Option<&TextQuery>,
    plan: Option<String>,
    reference: Option<String>,
) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(&all, &prefixes, &ctx.registry);
    let rows =
        super::parked::rows_with_records(&mut ctx, &all, &hierarchy, &claims, now, waiting_on)?;
    let warnings = std::mem::take(&mut ctx.warnings);
    check_parent(&filter, &all, |parent| {
        rows.iter().any(|(row, _)| {
            Fields::of_row(row, &ctx.registry)
                .is_some_and(|fields| fields.parent.as_ref() == Some(parent))
        })
    })?;
    filter.resolve_under(&hierarchy, &ctx.registry)?;
    let tasks = rows
        .into_iter()
        .filter(|(_, task)| {
            plan.as_deref().is_none_or(|wanted| {
                task.as_ref()
                    .is_some_and(|task| task.plan.as_deref() == Some(wanted))
            })
        })
        .filter(|(_, task)| {
            reference.as_deref().is_none_or(|r| {
                task.as_ref()
                    .is_some_and(|task| task.refs.iter().any(|e| e.reference == r))
            })
        })
        .filter(|(_, task)| match (text, task) {
            // The text matches the copy the row shows; an unresolved park has none.
            (Some(text), Some(task)) => text.matches(task),
            (Some(_), None) => false,
            (None, _) => true,
        })
        .map(|(row, _)| row)
        .filter(|row| match Fields::of_row(row, &ctx.registry) {
            // An unresolved park has no record to match: shown only when nothing filters.
            None => filter.is_empty(),
            // The parked pool is every open status, shelved included, so a surviving
            // shelved park stays visible for cleanup.
            Some(fields) => {
                (!filter.statuses().is_empty() || fields.status.is_open())
                    && filter.matches(&fields)
            }
        })
        .collect();
    let mut out = Output::Parked(ParkedOut { tasks, warnings });
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&ctx.project_colors());
    out.paint_activity(&activity);
    Ok(out)
}

/// Why a live claim keeps a task out of a read command's rows. `ready` appends the
/// takeover hint; `sample` does not, since a curator never starts a task.
pub fn claim_omission(id: &TaskId, claim: &crate::claims::Claim) -> String {
    format!(
        "{id} omitted: claimed by session {} in {}",
        claim.session, claim.worktree
    )
}

fn force_hint(id: &str, snapshots: &HashMap<String, HaltSnapshot>) -> String {
    let reason = if TaskId::parse(id)
        .ok()
        .and_then(|task| snapshots.get(&task.prefix))
        .is_some_and(|snapshot| !snapshot.halts().is_empty())
    {
        " --reason \"...\""
    } else {
        ""
    };
    format!("`tasks start --force {id}{reason}`")
}

/// A park this host recorded before its record closed, from another host or checkout:
/// the close came at or after the park, so no parked occurrence can be waiting. A recurring
/// record's next occurrence parked elsewhere still shows the previous, earlier completion.
fn closed_after_park(row: &ParkedRow) -> bool {
    let Some(park) = &row.park else {
        return false;
    };
    let closed = match row.status {
        Some(Status::Done) => row.completed.as_ref().or(row.updated.as_ref()),
        Some(Status::Dropped) => row.updated.as_ref(),
        _ => None,
    };
    match (
        closed.map(|at| crate::time::parse(at)),
        crate::time::parse(&park.at),
    ) {
        (Some(Ok(closed)), Ok(parked)) => closed >= parked,
        _ => false,
    }
}

/// The command that repeats a close, releasing whatever this host still records for it.
fn closing_hint(id: &str, status: Status) -> String {
    match status {
        Status::Dropped => format!("`tasks drop {id} \"<why>\"`"),
        _ => format!("`tasks done {id}`"),
    }
}

/// Ready tasks in ready order; pushes a warning per unreachable dependency. `deps` is
/// `all`'s dependency view (docs/user-guide.md).
#[allow(clippy::too_many_arguments)]
pub fn ready_tasks(
    ctx: &mut ReadCtx,
    all: &[Task],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    deps: &crate::review::Deps,
    claims: &crate::claims::ClaimSnapshot,
    snapshots: &HashMap<String, HaltSnapshot>,
    filter: &TaskFilter,
    now: OffsetDateTime,
) -> Result<Picked> {
    let mut warnings = Vec::new();
    let mut closed: HashMap<TaskId, Option<bool>> = HashMap::new();
    let selected: Vec<&Task> = all
        .iter()
        .filter(|task| {
            is_candidate(task, now)
                && filter.matches(&Fields::of_task(
                    task,
                    hierarchy,
                    deps,
                    claims,
                    &ctx.registry,
                    now,
                ))
        })
        .collect();
    for task in &selected {
        for dependency in &task.depends {
            if closed.contains_key(dependency) {
                continue;
            }
            closed.insert(dependency.clone(), known_closed(ctx, all, dependency)?);
        }
    }
    let lookup = |id: &TaskId| -> Option<bool> { closed.get(id).copied().flatten() };
    let mut ready = Vec::new();
    let mut deferred = Vec::new();
    for task in selected {
        for dependency in &task.depends {
            if lookup(dependency).is_none() {
                warnings.push(format!(
                    "{}: dependency {dependency} is unreachable",
                    task.id
                ));
            }
        }
        // A lane is a goal even before its first child exists.
        let canonical = ctx.registry.canonical_id(&task.id);
        let goal = crate::hierarchy::is_goal(task, hierarchy.child_count(&canonical) > 0);
        match readiness(task, goal, &lookup, now) {
            Readiness::Ready => ready.push(task.clone()),
            Readiness::Deferred => deferred.push(task.clone()),
            Readiness::Not => {}
        }
    }
    sort_ready(&mut ready);
    // Current contract (docs/user-guide.md): a paused lane keeps its subtree out of every picker. Its work
    // leaves here, before the claim and park checks that warn about what they drop, and
    // passes those same checks silently, so the paused count holds only work that would
    // otherwise be offered. A deferred task in a paused lane is in neither count.
    let (mut paused, mut ready): (Vec<Task>, Vec<Task>) = ready.into_iter().partition(|task| {
        hierarchy
            .paused_lane(&ctx.registry.canonical_id(&task.id))
            .is_some()
    });
    paused.retain(|task| {
        claims.held(&task.id).is_none()
            && !claims
                .park(&task.id)
                .is_some_and(|park| park.waiting_on == WaitingOn::User)
    });
    deferred.retain(|task| {
        hierarchy
            .paused_lane(&ctx.registry.canonical_id(&task.id))
            .is_none()
    });
    let mut me = None;
    ready.retain(|task| match claims.get(&task.id) {
        Some((claim, live)) if !live.is_stale() => {
            let release = match live {
                Liveness::Unverifiable(_) => {
                    let me = me.get_or_insert_with(|| {
                        crate::claims::resolve_identity(&ctx.host, &mut Vec::new())
                    });
                    super::release::hint_if_foreign(&task.id.to_string(), claim, me, &ctx.host)
                }
                _ => String::new(),
            };
            warnings.push(format!(
                "{} — {} to take it over{release}",
                claim_omission(&task.id, claim),
                force_hint(&task.id.to_string(), snapshots)
            ));
            false
        }
        _ => true,
    });
    ready.retain(|task| match claims.park(&task.id) {
        Some(park) if park.waiting_on == WaitingOn::User => {
            warnings.push(format!(
                "{} omitted: parked waiting on the user: {}",
                task.id, park.next_step
            ));
            false
        }
        _ => true,
    });
    deferred.retain(|task| {
        claims.held(&task.id).is_none()
            && !claims
                .park(&task.id)
                .is_some_and(|park| park.waiting_on == WaitingOn::User)
    });
    ctx.warnings.extend(warnings);
    Ok(Picked {
        tasks: ready,
        deferred,
        paused,
    })
}

pub(super) fn halt_snapshots(ctx: &mut ReadCtx, all: &[Task]) -> HashMap<String, HaltSnapshot> {
    let mut snapshots = HashMap::new();
    for project in ctx.scope.projects() {
        let local: Vec<Task> = all
            .iter()
            .filter(|task| task.id.prefix == project.prefix)
            .cloned()
            .collect();
        match halt::snapshot(project, &ctx.registry, &local) {
            Ok(snapshot) => {
                snapshots.insert(project.prefix.clone(), snapshot);
            }
            Err(error) => ctx.warnings.push(format!(
                "{}: halt state unknown ({error}); start will check again",
                project.prefix
            )),
        }
    }
    // Only what the scope asked for: under --group a non-member's checkout is no gap.
    if let Scope::All { members, .. } = &ctx.scope {
        let scoped = ctx.scope.prefixes();
        for prefix in members {
            if !scoped.contains(prefix) {
                ctx.warnings.push(format!(
                    "{prefix}: halt state unknown (registered checkout unreachable)"
                ));
            }
        }
    }
    snapshots
}

fn halt_rows(snapshots: &HashMap<String, HaltSnapshot>, all: &[Task]) -> Vec<HaltRow> {
    let mut rows: Vec<HaltRow> = snapshots
        .values()
        .flat_map(|snapshot| {
            snapshot.halts().iter().map(|halt| HaltRow {
                id: halt.id.to_string(),
                title: halt.title.clone(),
                owner: halt.owner.clone(),
                priority: halt.priority,
                present_locally: all.iter().any(|task| task.id == halt.id),
            })
        })
        .collect();
    rows.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    rows
}

fn retain_allowed(tasks: &mut Vec<Task>, snapshots: &HashMap<String, HaltSnapshot>) -> usize {
    let before = tasks.len();
    tasks.retain(|task| {
        snapshots
            .get(&task.id.prefix)
            .is_none_or(|halt| halt.allows(task))
    });
    before - tasks.len()
}

fn warn_hidden(ctx: &mut ReadCtx, hidden: usize) {
    if hidden > 0 {
        ctx.warnings
            .push(format!("{hidden} ready task(s) hidden by halt"));
    }
}

/// Current contract (docs/user-guide.md): one `<n> task(s) hidden by paused lane <id>` warning per lane,
/// counted before exclusive holds. The pickers already applied the dependency, defer,
/// claim, and park checks to `paused`; this applies the halt, `--without`, and cutoff
/// gates the caller applies to its own list, without their warnings, then counts each
/// task once under its lane.
pub(crate) fn warn_paused(
    ctx: &mut ReadCtx,
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    mut paused: Vec<Task>,
    snapshots: &HashMap<String, HaltSnapshot>,
    without: &Without,
    cutoff: Option<crate::model::Complexity>,
    claims: &crate::claims::ClaimSnapshot,
) {
    let _ = retain_allowed(&mut paused, snapshots);
    // Each task is hidden only where its own project declares the need.
    let _ = without.retain(&mut paused, &vocabularies(&ctx.scope));
    if let Some(cutoff) = cutoff {
        let _ = crate::complexity::apply(&mut paused, cutoff, claims);
    }
    let mut by_lane: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
    for task in &paused {
        let canonical = ctx.registry.canonical_id(&task.id);
        if let Some(lane) = hierarchy.paused_lane(&canonical) {
            by_lane
                .entry(lane.id.clone())
                .or_default()
                .insert(task.id.clone());
        }
    }
    ctx.warnings.extend(paused_omissions(&by_lane));
}

/// Current contract (docs/user-guide.md): drop tasks held back by another session's exclusive hold,
/// with one warning per need and holder; order is unchanged. Whether a hold is this
/// session's is the read views' rule, `read_holds`: with a resolved identity, as
/// `occupants` decides a claim (the identity, or else the claim's process proof); with
/// none, every hold counts as another session's, and a warning names the resolution error.
/// Callers run it after every other gate, the complexity cutoff included, so a task those
/// hide is never counted as waiting. A scope whose projects declare no exclusive need
/// reads no claim store and resolves no identity, so its output is exactly what it was
/// before holds existed.
fn retain_unheld(ctx: &mut ReadCtx, tasks: &mut Vec<Task>, now: OffsetDateTime) -> Result<()> {
    let mut vocabularies = vocabularies(&ctx.scope);
    vocabularies.retain(|_, vocabulary| vocabulary.values().any(|need| need.exclusive));
    if vocabularies.is_empty() {
        return Ok(());
    }
    let (snapshot, warnings) = crate::holds::HoldSnapshot::load(&ctx.registry, now);
    ctx.warnings.extend(warnings);
    if snapshot.is_empty() {
        return Ok(());
    }
    let mine = view_holds(&snapshot, &mut ctx.warnings, &ctx.host)?;
    let mut held = crate::holds::HeldWarnings::default();
    tasks.retain(|task| {
        let Some(vocabulary) = vocabularies.get(task.id.prefix.as_str()) else {
            return true;
        };
        match crate::holds::held_back(&snapshot, vocabulary, task, &mine) {
            Some((need, holder)) => {
                held.add(&need, &holder);
                false
            }
            None => true,
        }
    });
    ctx.warnings.extend(held.into_warnings());
    Ok(())
}

/// The caller's own holds in `snapshot`, by the read views' rule (`read_holds`): the
/// session's identity is resolved here, and when that fails every hold counts as another
/// session's, with a warning naming the error.
pub(super) fn view_holds(
    snapshot: &crate::holds::HoldSnapshot,
    warnings: &mut Vec<String>,
    host: &crate::config::HostConfig,
) -> Result<crate::holds::Mine> {
    let me = crate::claims::resolve_identity(host, warnings);
    if let crate::claims::Resolution::Failed(error) = &me {
        warnings.push(format!(
            "session identity unresolved ({error}); every hold counts as another session's"
        ));
    }
    super::read_holds(snapshot, &me, host)
}

/// Every in-scope project's vocabulary, by prefix: the names `--without` may use, and
/// the vocabulary each task's needs are judged by (docs/task-model.md). It borrows the scope
/// alone, so a caller may hold it while pushing to `ctx.warnings`.
pub(super) fn vocabularies(scope: &Scope) -> Vocabularies<'_> {
    scope
        .projects()
        .iter()
        .map(|project| (project.prefix.as_str(), &project.needs))
        .collect()
}

pub fn ready(
    mut ctx: ReadCtx,
    filter: FilterArgs,
    limit: Option<usize>,
    max_complexity: Option<String>,
    without: Vec<String>,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
    let mut filter = TaskFilter::parse(&filter, &[], &ctx.registry, &ctx.shorthand)?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(&all, &prefixes, &ctx.registry);
    check_parent(&filter, &all, |_| false)?;
    filter.resolve_under(&hierarchy, &ctx.registry)?;
    let deps = review_deps(&mut ctx, &all)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let mut picked = ready_tasks(
        &mut ctx, &all, &hierarchy, &deps, &claims, &snapshots, &filter, now,
    )?;
    let halts = halt_rows(&snapshots, &all);
    let hidden = retain_allowed(&mut picked.tasks, &snapshots);
    warn_hidden(&mut ctx, hidden);
    if !without.is_empty() {
        let vocabs = vocabularies(&ctx.scope);
        let hidden = without.retain(&mut picked.tasks, &vocabs);
        let _ = without.retain(&mut picked.deferred, &vocabs);
        ctx.warnings.extend(without.warning(hidden));
    }
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut picked.tasks, cutoff, &claims);
        ctx.warnings
            .extend(crate::complexity::warnings(cutoff, &hidden));
        let _ = crate::complexity::apply(&mut picked.deferred, cutoff, &claims);
    }
    retain_unheld(&mut ctx, &mut picked.tasks, now)?;
    if let Some(warning) = deferred_omission(&picked.deferred) {
        ctx.warnings.push(warning);
    }
    warn_paused(
        &mut ctx,
        &hierarchy,
        picked.paused,
        &snapshots,
        &without,
        cutoff,
        &claims,
    );
    let mut tasks = picked.tasks;
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    let colors = ctx.project_colors();
    let mut out = Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &hierarchy, &deps, Some(&claims), &ctx.registry, now))
            .collect(),
        halts,
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    });
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}

/// The head of `ready` in the show shape, so a caller can start on it without a second
/// lookup. Nothing ready is a normal state: null, warnings, exit 0.
pub fn next(
    mut ctx: ReadCtx,
    max_complexity: Option<String>,
    without: Vec<String>,
    under: Option<String>,
    fetch: crate::resolve::Fetch,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
    let mut filter = TaskFilter::parse(
        &FilterArgs {
            under,
            ..FilterArgs::default()
        },
        &[],
        &ctx.registry,
        &ctx.shorthand,
    )?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let prefixes = ctx.scope.prefixes();
    let hierarchy = crate::hierarchy::view_of(&all, &prefixes, &ctx.registry);
    filter.resolve_under(&hierarchy, &ctx.registry)?;
    let deps = review_deps(&mut ctx, &all)?;
    let _ = super::parked::rows(&mut ctx, &all, &hierarchy, &claims, now)?;
    let candidates =
        super::parked::candidates(&mut ctx, &all, &hierarchy, &deps, &claims, &filter, now)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let ready = ready_tasks(
        &mut ctx, &all, &hierarchy, &deps, &claims, &snapshots, &filter, now,
    )?;
    // One pool in pick order — parked candidates first, then the ready list — with each
    // task once, so a parked todo that is also ready is hidden and counted once.
    let mut pool = candidates.tasks;
    for task in ready.tasks {
        if !pool.iter().any(|candidate| candidate.id == task.id) {
            pool.push(task);
        }
    }
    let halts = halt_rows(&snapshots, &all);
    let hidden = retain_allowed(&mut pool, &snapshots);
    warn_hidden(&mut ctx, hidden);
    let mut omitted = candidates.deferred;
    for task in ready.deferred {
        if !omitted.iter().any(|held| held.id == task.id) {
            omitted.push(task);
        }
    }
    if !without.is_empty() {
        let vocabs = vocabularies(&ctx.scope);
        let hidden = without.retain(&mut pool, &vocabs);
        let _ = without.retain(&mut omitted, &vocabs);
        ctx.warnings.extend(without.warning(hidden));
    }
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut pool, cutoff, &claims);
        ctx.warnings
            .extend(crate::complexity::warnings(cutoff, &hidden));
        let _ = crate::complexity::apply(&mut omitted, cutoff, &claims);
    }
    // On the whole pool, so parked-agent candidates pass the same gate.
    retain_unheld(&mut ctx, &mut pool, now)?;
    if let Some(warning) = deferred_omission(&omitted) {
        ctx.warnings.push(warning);
    }
    // A parked todo is both a candidate and a ready row; `warn_paused` counts ids in a
    // set, so it is counted once.
    let mut paused = candidates.paused;
    paused.extend(ready.paused);
    warn_paused(
        &mut ctx, &hierarchy, paused, &snapshots, &without, cutoff, &claims,
    );
    let next = match pool.into_iter().next() {
        None => None,
        Some(task) => {
            let project = ctx
                .scope
                .projects()
                .iter()
                .find(|project| project.prefix == task.id.prefix)
                .expect("a ready task was scanned from a project in scope");
            let mut warnings = Vec::new();
            let fields = super::show::describe(
                project,
                &ctx.registry,
                task,
                &all,
                Some(&claims),
                &mut warnings,
                now,
                fetch,
                &ctx.host,
            )?;
            ctx.warnings.extend(warnings);
            Some(fields)
        }
    };
    Ok(Output::Next(Box::new(NextOut {
        next,
        halts,
        warnings: ctx.warnings,
    })))
}

/// Live claims on tasks this scan does not hold: work started in a checkout whose branch
/// has not merged. Each is read from the claim's recorded worktree, with that checkout's
/// scan, the docs/ownership.md resolves a store-only park, so `doing` never drops a
/// live claim without a word. None is a `ready` or `next` candidate: `start` reads the
/// local file, so the warning names where to resume instead.
fn claimed_elsewhere(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &crate::claims::ClaimSnapshot,
) -> (Vec<(Project, Vec<Task>)>, Vec<(usize, Task)>) {
    claimed_elsewhere_with(ctx, all, claims, super::parked::open_recorded)
}

fn claimed_elsewhere_with(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &crate::claims::ClaimSnapshot,
    mut open_recorded: impl FnMut(
        &TaskId,
        &std::path::Path,
    ) -> Result<Option<(Project, Task, Vec<Task>)>>,
) -> (Vec<(Project, Vec<Task>)>, Vec<(usize, Task)>) {
    struct CachedCheckout {
        project: Project,
        scan: Vec<Task>,
        seen_worktrees: Vec<std::path::PathBuf>,
    }

    let mut scans: Vec<CachedCheckout> = Vec::new();
    let mut found: Vec<(usize, Task)> = Vec::new();
    for (key, (claim, liveness)) in claims.iter() {
        if liveness.is_stale() {
            continue;
        }
        let id = match TaskId::parse(key) {
            Ok(id) => ctx.registry.canonical_id(&id),
            Err(error) => {
                ctx.warnings.push(format!(
                    "claim entry {key:?} is not a task id ({error}); skipped"
                ));
                continue;
            }
        };
        if all.iter().any(|task| task.id == id) {
            continue;
        }
        let worktree = std::path::Path::new(&claim.worktree);
        if let Some(at) = scans.iter().position(|entry| {
            entry.project.prefix == id.prefix
                && entry.seen_worktrees.iter().any(|seen| seen == worktree)
        }) {
            if let Some(task) = scans[at].scan.iter().find(|task| task.id == id).cloned() {
                ctx.warnings.push(format!(
                    "{id} is claimed in {}; resume it from that checkout",
                    claim.worktree
                ));
                found.push((at, task));
            } else {
                ctx.warnings.push(format!(
                    "{id} is claimed in {}, which is unavailable",
                    claim.worktree
                ));
            }
            continue;
        }
        match open_recorded(&id, worktree) {
            Ok(Some((project, task, scan))) => {
                ctx.warnings.push(format!(
                    "{id} is claimed in {}; resume it from that checkout",
                    claim.worktree
                ));
                let record_seen = |seen: &mut Vec<std::path::PathBuf>| {
                    let wt = worktree.to_path_buf();
                    if !seen.contains(&wt) {
                        seen.push(wt);
                    }
                    if let Ok(canon) = worktree.canonicalize()
                        && !seen.contains(&canon)
                    {
                        seen.push(canon);
                    }
                };
                let at = match scans.iter().position(|existing| {
                    existing.project.prefix == project.prefix
                        && existing.project.records == project.records
                        && existing.project.resume_root() == project.resume_root()
                        && existing.project.root == project.root
                        && existing.scan.iter().any(|t| t.id == task.id)
                }) {
                    Some(existing) => {
                        record_seen(&mut scans[existing].seen_worktrees);
                        existing
                    }
                    None => {
                        let mut seen_worktrees = Vec::new();
                        record_seen(&mut seen_worktrees);
                        scans.push(CachedCheckout {
                            project,
                            scan,
                            seen_worktrees,
                        });
                        scans.len() - 1
                    }
                };
                found.push((at, task));
            }
            Ok(None) => ctx.warnings.push(format!(
                "{id} is claimed in {}, which is unavailable",
                claim.worktree
            )),
            Err(error) => ctx.warnings.push(format!(
                "{id} is claimed in {}, which is unavailable ({error})",
                claim.worktree
            )),
        }
    }
    (
        scans
            .into_iter()
            .map(|entry| (entry.project, entry.scan))
            .collect(),
        found,
    )
}

pub fn prime(
    mut ctx: ReadCtx,
    closed: bool,
    without: Vec<String>,
    full: Option<Vec<String>>,
    format: crate::output::Format,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(None)?;
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let prefixes = ctx.scope.prefixes();
    let registry = ctx.registry.clone();
    let mut membership = (format == crate::output::Format::Pretty)
        .then(|| crate::membership::Membership::new(&registry, &prefixes, &all));
    let hierarchy = match &mut membership {
        Some(membership) => {
            let records = crate::hierarchy::project_records(&all, &prefixes, &registry);
            let mut indexes = crate::hierarchy::ProjectIndexes::new();
            for prefix in records.keys() {
                let index = membership
                    .hierarchy(prefix)?
                    .expect("preseeded project in Membership has an index");
                indexes.insert(prefix.clone(), index);
            }
            crate::hierarchy::HierarchyView::new(records, indexes, &registry)
        }
        None => crate::hierarchy::view_of(&all, &prefixes, &registry),
    };
    let deps = review_deps(&mut ctx, &all)?;
    let upcoming: Vec<OffsetDateTime> = all
        .iter()
        .filter_map(crate::periodic::due)
        .filter(|due| *due > now)
        .collect();
    let next = upcoming.iter().min().copied();
    let periodic = PeriodicSummary {
        scheduled: upcoming.len(),
        next_due: next.map(crate::time::format),
        in_days: next.map(|due| crate::periodic::days_until(due, now)),
    };
    let waiting: Vec<crate::defer::Defer> = all
        .iter()
        .filter(|task| task.status != Status::Shelved && crate::defer::is_deferred(task, now))
        .filter_map(|task| task.defer)
        .collect();
    let next = waiting.iter().min().copied();
    let deferred = DeferredSummary {
        waiting: waiting.len(),
        next: next.map(|defer| defer.to_string()),
        in_days: next.map(|defer| crate::defer::days_until(defer.date(), now)),
        due: all
            .iter()
            .filter(|task| task.status != Status::Shelved && crate::defer::is_due(task, now))
            .count(),
    };
    let counts = Counts::of(&all);
    let (left, parked): (Vec<ParkedRow>, Vec<ParkedRow>) =
        super::parked::rows(&mut ctx, &all, &hierarchy, &claims, now)?
            .into_iter()
            .partition(closed_after_park);
    for row in &left {
        let park = row.park.as_ref().expect("closed_after_park found a park");
        let status = row.status.expect("closed_after_park found a status");
        ctx.warnings.push(format!(
            "{} was {} after this host parked it from {}; {} clears the park",
            row.id,
            status.as_str(),
            park.worktree,
            closing_hint(&row.id, status)
        ));
    }
    let snapshots = halt_snapshots(&mut ctx, &all);
    let mut ready = ready_tasks(
        &mut ctx,
        &all,
        &hierarchy,
        &deps,
        &claims,
        &snapshots,
        &TaskFilter::default(),
        now,
    )?
    .tasks;
    let halts = halt_rows(&snapshots, &all);
    let hidden = retain_allowed(&mut ready, &snapshots);
    warn_hidden(&mut ctx, hidden);
    if !without.is_empty() {
        let hidden = without.retain(&mut ready, &vocabularies(&ctx.scope));
        ctx.warnings.extend(without.warning(hidden));
    }
    let mut doing: Vec<(Option<usize>, Task)> = all
        .iter()
        .filter(|task| task.status == Status::Doing || claims.held(&task.id).is_some())
        .map(|task| (None, task.clone()))
        .collect();
    let (elsewhere_scans, elsewhere_rows) = claimed_elsewhere(&mut ctx, &all, &claims);
    doing.extend(
        elsewhere_rows
            .into_iter()
            .map(|(at, task)| (Some(at), task)),
    );
    // Each distinct checkout read from claims is judged once with its own scan and cache.
    let elsewhere_deps: Vec<_> = elsewhere_scans
        .iter()
        .map(|(project, scan)| recorded_review_deps(&mut ctx, project, scan))
        .collect::<Result<_>>()?;
    let elsewhere_activities: Vec<_> = elsewhere_scans
        .iter()
        .map(|(_, scan)| {
            let mut activity = crate::activity::ActivityIndex::default();
            activity.extend_scan(scan, &registry, now);
            activity
        })
        .collect();
    let mut elsewhere_memberships: Vec<Option<crate::membership::Membership<'_>>> = elsewhere_scans
        .iter()
        .map(|(project, scan)| {
            (format == crate::output::Format::Pretty).then(|| {
                crate::membership::Membership::new(
                    &registry,
                    std::slice::from_ref(&project.prefix),
                    scan,
                )
            })
        })
        .collect();
    let mut elsewhere_views = Vec::with_capacity(elsewhere_scans.len());
    for ((project, scan), membership_opt) in
        elsewhere_scans.iter().zip(elsewhere_memberships.iter_mut())
    {
        let opened = std::slice::from_ref(&project.prefix);
        let view = match membership_opt {
            Some(em) => {
                let records = crate::hierarchy::project_records(scan, opened, &registry);
                let mut indexes = crate::hierarchy::ProjectIndexes::new();
                for prefix in records.keys() {
                    let index = em
                        .hierarchy(prefix)?
                        .expect("preseeded project in Membership has an index");
                    indexes.insert(prefix.clone(), index);
                }
                crate::hierarchy::HierarchyView::new(records, indexes, &registry)
            }
            None => crate::hierarchy::view_of(scan, opened, &registry),
        };
        elsewhere_views.push(view);
    }
    doing.sort_by(|(_, a), (_, b)| list_order(a, b));
    let mut roadmap = crate::hierarchy::forest(
        &all,
        &hierarchy,
        &deps,
        None,
        false,
        crate::hierarchy::Shelved::Hidden,
        Some(&claims),
        &ctx.registry,
        now,
    );
    // closeout is an invitation to run `done`, so it holds only what `done` will accept.
    // Children are one gate and dependencies are the other; listing a goal its dependencies
    // still hold would invite a close the tool then refuses with `open_dependencies`. Held
    // goals are named in a warning instead, so none of them goes silent. A goal still
    // waiting on one of its own declared pieces is unfinished work, not held, so it stays
    // out silently (docs/task-model.md). A childless lane is not a candidate.
    let mut closeout: Vec<Task> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    'goals: for task in &all {
        let canonical = ctx.registry.canonical_id(&task.id);
        // Current contract (docs/task-model.md): todo, doing, or blocked; an idea is open but not a candidate
        if !matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
            || (task.pieces.is_empty() && hierarchy.child_count(&canonical) == 0)
            || hierarchy.open_descendant_count(&canonical) > 0
        {
            continue;
        }
        let mut open = Vec::new();
        for dependency in &task.depends {
            // An unreachable dependency counts as open, exactly as `done` treats it: what
            // we cannot resolve, we cannot call closed.
            if !satisfied(&ctx, &all, dependency)? {
                if crate::pieces::is_piece(&ctx.registry, task, dependency) {
                    continue 'goals;
                }
                open.push(dependency.to_string());
            }
        }
        if open.is_empty() {
            closeout.push(task.clone());
        } else {
            held.push(format!(
                "{} has no open children or pieces but is held by open prerequisites: {}; it \
                 joins closeout when they close",
                task.id,
                open.join(", ")
            ));
        }
    }
    sort_ready(&mut closeout);
    if let Some(cutoff) = cutoff {
        let mut hidden = crate::complexity::apply(&mut ready, cutoff, &claims);
        let from_closeout = crate::complexity::apply(&mut closeout, cutoff, &claims);
        hidden.above += from_closeout.above;
        hidden.unassessed += from_closeout.unassessed;
        ctx.warnings
            .extend(crate::complexity::warnings(cutoff, &hidden));
    }
    retain_unheld(&mut ctx, &mut ready, now)?;
    ctx.warnings.extend(held);
    // The same builder as `tasks lanes`, under this session's cutoff and `--without`.
    let mut lanes = super::lanes::rows(
        &mut ctx, &all, &hierarchy, &deps, &claims, &snapshots, cutoff, &without, now,
    )?;
    let wide = matches!(ctx.scope, Scope::All { .. });
    for project in ctx.scope.projects() {
        if let Some(files) = project.uncommitted_task_files()?
            && !files.is_empty()
        {
            let message = format!("uncommitted task files: {}", files.join(", "));
            ctx.warnings.push(if wide {
                format!("{}: {message}", project.prefix)
            } else {
                message
            });
        }
    }
    for task in &all {
        if let Some(claim) = claims.held(&task.id)
            && matches!(task.status, Status::Todo | Status::Idea)
        {
            ctx.warnings.push(format!(
                "{} is claimed as doing in {} but this checkout's copy says {}; the two copies will conflict on merge",
                task.id,
                claim.worktree,
                task.status.as_str()
            ));
        }
    }
    for (id, claim, why) in claims.stale() {
        // A closed record refuses `start` unless it recurs, so its closing command is what
        // releases the claim.
        let closed = all
            .iter()
            .find(|task| task.id.to_string() == *id)
            .filter(|task| {
                task.status == Status::Dropped
                    || (task.status == Status::Done && task.every.is_none())
            });
        ctx.warnings.push(match closed {
            Some(task) => format!(
                "{id} has a stale claim from session {} ({why}); {} to release it",
                claim.session,
                closing_hint(id, task.status)
            ),
            None => format!(
                "{id} has a stale claim from session {} ({why}); {} to take it over",
                claim.session,
                force_hint(id, &snapshots)
            ),
        });
    }
    let mut ready_rows: Vec<TaskSummary> = ready
        .iter()
        .map(|task| TaskSummary::of(task, &hierarchy, &deps, Some(&claims), &registry, now))
        .collect();
    let mut doing_rows: Vec<TaskSummary> = Vec::with_capacity(doing.len());
    for (source, task) in &doing {
        let (view, row_deps) = source.map_or((&hierarchy, &deps), |at| {
            (&elsewhere_views[at], &elsewhere_deps[at])
        });
        let mut row = TaskSummary::of(task, view, row_deps, Some(&claims), &registry, now);
        if let Some(at) = *source {
            row.activity = elsewhere_activities[at].get(&task.id);
            if let Some(em) = elsewhere_memberships[at].as_mut() {
                row.goal_work = crate::output::GoalWork::of(task, row.child_count, em)?;
            }
        } else if let Some(membership) = membership.as_mut() {
            row.goal_work = crate::output::GoalWork::of(task, row.child_count, membership)?;
        }
        doing_rows.push(row);
    }
    let mut closeout_rows: Vec<TaskSummary> = closeout
        .iter()
        .map(|task| TaskSummary::of(task, &hierarchy, &deps, Some(&claims), &registry, now))
        .collect();

    if let Some(membership) = membership.as_mut() {
        for (task, row) in ready.iter().zip(ready_rows.iter_mut()) {
            row.goal_work = crate::output::GoalWork::of(task, row.child_count, membership)?;
        }
        for lane_row in &mut lanes {
            let parsed = TaskId::parse(&lane_row.lane.id).expect("lane summary id is valid");
            let canonical = registry.canonical_id(&parsed);
            let lane_task = hierarchy
                .get(&canonical)
                .expect("lane came from captured hierarchy");
            lane_row.lane.goal_work =
                crate::output::GoalWork::of(lane_task, lane_row.lane.child_count, membership)?;
        }
        super::tree::populate_tree_goal_work(&mut roadmap, &hierarchy, &registry, membership)?;
        for (task, row) in closeout.iter().zip(closeout_rows.iter_mut()) {
            row.goal_work = crate::output::GoalWork::of(task, row.child_count, membership)?;
        }
        for warning in membership.take_warnings() {
            if !ctx.warnings.contains(&warning) {
                ctx.warnings.push(warning);
            }
        }
    }
    for em in elsewhere_memberships.iter_mut().flatten() {
        for warning in em.take_warnings() {
            if !ctx.warnings.contains(&warning) {
                ctx.warnings.push(warning);
            }
        }
    }

    let colors = ctx.project_colors();
    let mut out = Output::Prime(Box::new(PrimeOut {
        summary: None,
        prefix: match &ctx.scope {
            Scope::Local(project) => Some(project.prefix.clone()),
            Scope::All { .. } => None,
        },
        group: ctx.scope.group().map(str::to_string),
        projects: ctx.scope.prefixes(),
        counts,
        periodic,
        deferred,
        closed,
        ready: ready_rows,
        parked,
        lanes,
        doing: doing_rows,
        roadmap,
        closeout: closeout_rows,
        halts,
        warnings: ctx.warnings,
    }));
    if let Some(full) = full
        && let Output::Prime(prime) = &mut out
    {
        crate::prime_summary::bound(prime, full)?;
    }
    let activity = crate::activity::ActivityIndex::of_scan(&all, &ctx.registry, now);
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_resolution_prefers_the_captured_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "sci", None).unwrap();
        let dependency =
            crate::commands::add::blank(&project, "Dependency".into(), Status::Todo, None).unwrap();
        std::fs::write(project.root.join(crate::repo::CONFIG_REL), "not toml = [").unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.register("sci", &project.root).unwrap();
        registry.aliases.insert("old".into(), "sci".into());
        let ctx = ReadCtx {
            host: std::sync::Arc::new(crate::config::HostConfig::default()),
            scope: Scope::All {
                projects: vec![],
                members: vec![],
                group: None,
            },
            registry,
            warnings: vec![],
            shorthand: crate::shorthand::Shorthand::new(dir.path().to_path_buf()),
        };

        assert_eq!(
            resolve_dependency(&ctx, std::slice::from_ref(&dependency), &dependency.id).unwrap(),
            Some(dependency.clone())
        );
        // An alias spelling canonicalizes before the scan is consulted, and the shared
        // verdict judges the captured copy without opening the broken root.
        let alias = TaskId {
            prefix: "old".into(),
            hex: dependency.id.hex.clone(),
        };
        let mut done = dependency.clone();
        done.status = Status::Done;
        assert_eq!(
            known_closed(&ctx, std::slice::from_ref(&dependency), &alias).unwrap(),
            Some(false)
        );
        assert!(satisfied(&ctx, std::slice::from_ref(&done), &alias).unwrap());
    }

    #[test]
    fn claimed_elsewhere_separates_shared_roots_and_reuses_symlinked_worktrees() {
        let dir = tempfile::tempdir().unwrap();
        let shared_code = dir.path().join("shared-code");
        std::fs::create_dir_all(&shared_code).unwrap();
        let shared_code = shared_code.canonicalize().unwrap();

        let make_detached = |name: &str| -> crate::repo::Project {
            let rec = dir.path().join(name);
            let _ = crate::repo::Project::init(&rec, "aa", None).unwrap();
            std::fs::write(
                rec.join(crate::repo::CONFIG_REL),
                format!(
                    "prefix = \"aa\"\n\n[checkout]\npath = \"{}\"\nkind = \"detached\"\n",
                    shared_code.display()
                ),
            )
            .unwrap();
            crate::repo::Project::open(&rec).unwrap()
        };

        let main_proj = make_detached("main-rec");
        let det_1 = make_detached("det-rec-1");
        let det_2 = make_detached("det-rec-2");
        assert_eq!(det_1.root, det_2.root);
        assert_ne!(det_1.records, det_2.records);

        let mut registry = crate::registry::Registry::default();
        registry.register("aa", &shared_code).unwrap();

        let t1 = crate::commands::add::blank(&det_1, "Det 1 task A".into(), Status::Doing, None)
            .unwrap();
        let t2 = crate::commands::add::blank(&det_1, "Det 1 task B".into(), Status::Doing, None)
            .unwrap();
        let t3 = crate::commands::add::blank(&det_2, "Det 2 task C".into(), Status::Doing, None)
            .unwrap();
        det_1.write_task(&registry, &t1).unwrap();
        det_1.write_task(&registry, &t2).unwrap();
        det_2.write_task(&registry, &t3).unwrap();

        let symlink_wt_1 = dir.path().join("symlink-det-1");
        std::os::unix::fs::symlink(&det_1.records, &symlink_wt_1).unwrap();

        let make_claim = |wt: &std::path::Path| {
            (
                crate::claims::Claim {
                    owner: "worker".into(),
                    session: "s1".into(),
                    root_session: None,
                    pid: None,
                    pid_start: None,
                    boot_id: None,
                    pid_view: None,
                    host: "h".into(),
                    worktree: wt.display().to_string(),
                    started: "2026-11-17T00:00:00Z".into(),
                    seen: "2026-11-17T00:00:00Z".into(),
                    holds: vec![],
                },
                crate::claims::Liveness::Live,
            )
        };

        // 1. Two claims via symlink_wt_1 + one claim in det_2 (which shares project.root with det_1):
        let mut by_id = BTreeMap::new();
        by_id.insert(t1.id.to_string(), make_claim(&symlink_wt_1));
        by_id.insert(t2.id.to_string(), make_claim(&symlink_wt_1));
        by_id.insert(t3.id.to_string(), make_claim(&det_2.records));
        let claims =
            crate::claims::ClaimSnapshot::from_parts(by_id, BTreeMap::new(), BTreeMap::new());

        let mut ctx = ReadCtx {
            host: std::sync::Arc::new(crate::config::HostConfig::default()),
            scope: Scope::Local(Box::new(main_proj.clone())),
            registry,
            warnings: vec![],
            shorthand: crate::shorthand::Shorthand::new(dir.path().to_path_buf()),
        };

        let mut open_calls = 0usize;
        let (scans, found) = claimed_elsewhere_with(&mut ctx, &[], &claims, |id, wt| {
            open_calls += 1;
            crate::commands::parked::open_recorded(id, wt)
        });

        assert_eq!(
            open_calls, 2,
            "second claim in symlink_wt_1 must reuse cached scan without re-running open_recorded"
        );
        assert_eq!(
            scans.len(),
            2,
            "distinct Kind::Detached records checkouts sharing project.root must keep separate scans"
        );
        assert_eq!(found.len(), 3);
        for (at, task) in &found {
            assert!(
                scans[*at].1.iter().any(|s| s.id == task.id),
                "paired scan must contain task {}",
                task.id
            );
        }

        // 2. Two Kind::Detached code worktrees sharing both project.root and project.records
        //    while differing in project.worktree (resume_root()):
        let code_wt_a = dir.path().join("code-wt-a");
        let code_wt_b = dir.path().join("code-wt-b");
        std::fs::create_dir_all(&code_wt_a).unwrap();
        std::fs::create_dir_all(&code_wt_b).unwrap();
        let mut by_id_wt = BTreeMap::new();
        by_id_wt.insert(t1.id.to_string(), make_claim(&code_wt_a));
        by_id_wt.insert(t2.id.to_string(), make_claim(&code_wt_b));
        let claims_wt =
            crate::claims::ClaimSnapshot::from_parts(by_id_wt, BTreeMap::new(), BTreeMap::new());
        let scan_1 = det_1.scan().unwrap();
        let (wt_scans, wt_found) = claimed_elsewhere_with(&mut ctx, &[], &claims_wt, |id, wt| {
            let mut proj = det_1.clone();
            proj.worktree = Some(wt.canonicalize().unwrap());
            let task = scan_1.iter().find(|t| t.id == *id).cloned().unwrap();
            Ok(Some((proj, task, scan_1.clone())))
        });
        assert_eq!(
            wt_scans.len(),
            2,
            "distinct code worktrees with different resume_root() must stay distinct for Resolver"
        );
        assert_ne!(wt_scans[0].0.resume_root(), wt_scans[1].0.resume_root());
        assert_eq!(wt_found.len(), 2);

        // 3. Deterministic ID order (early_id < mid_id < late_id):
        //    - early_id claimed via symlink_wt_1 (det_1, whose resume_root() == shared_code!)
        //    - mid_id claimed via canonical det_1.records (reuses det_1 without rescan)
        //    - late_id claimed at shared_code (resolves via git/registry to det_2, NOT det_1!)
        let mut t_early =
            crate::commands::add::blank(&det_1, "Early det_1".into(), Status::Doing, None).unwrap();
        t_early.id = TaskId::parse("aa-000001").unwrap();
        let mut t_mid =
            crate::commands::add::blank(&det_1, "Mid det_1".into(), Status::Doing, None).unwrap();
        t_mid.id = TaskId::parse("aa-000002").unwrap();
        let mut t_late =
            crate::commands::add::blank(&det_2, "Late det_2".into(), Status::Doing, None).unwrap();
        t_late.id = TaskId::parse("aa-fffffe").unwrap();
        det_1.write_task(&ctx.registry, &t_early).unwrap();
        det_1.write_task(&ctx.registry, &t_mid).unwrap();
        det_2.write_task(&ctx.registry, &t_late).unwrap();

        let det_2_scan = det_2.scan().unwrap();
        let mut by_id_order = BTreeMap::new();
        by_id_order.insert(t_early.id.to_string(), make_claim(&symlink_wt_1));
        by_id_order.insert(t_mid.id.to_string(), make_claim(&det_1.records));
        by_id_order.insert(t_late.id.to_string(), make_claim(&shared_code));
        let claims_order =
            crate::claims::ClaimSnapshot::from_parts(by_id_order, BTreeMap::new(), BTreeMap::new());
        ctx.warnings.clear();
        let mut order_opens = 0usize;
        let (ord_scans, ord_found) =
            claimed_elsewhere_with(&mut ctx, &[], &claims_order, |id, wt| {
                order_opens += 1;
                if wt == shared_code {
                    let task = det_2_scan.iter().find(|t| t.id == *id).cloned().unwrap();
                    Ok(Some((det_2.clone(), task, det_2_scan.clone())))
                } else {
                    crate::commands::parked::open_recorded(id, wt)
                }
            });
        assert_eq!(
            order_opens, 2,
            "canonical det_1.records must reuse symlink_wt_1 scan while shared_code opens det_2"
        );
        assert_eq!(
            ord_scans.len(),
            2,
            "shared_code claim must not falsely match det_1's resume_root()"
        );
        assert_eq!(
            ord_found.len(),
            3,
            "late_id at shared_code must not be dropped as unavailable; warnings: {:?}",
            ctx.warnings
        );

        // 4. Post-open structural guard: if a second symlink opens the same checkout after a
        //    concurrent write added a new task absent from the first captured scan, it keeps a
        //    scan containing that task rather than pairing it with the stale scan.
        let symlink_wt_1b = dir.path().join("symlink-det-1b");
        std::os::unix::fs::symlink(&det_1.records, &symlink_wt_1b).unwrap();
        let mut t_added =
            crate::commands::add::blank(&det_1, "Concurrent task".into(), Status::Doing, None)
                .unwrap();
        t_added.id = TaskId::parse("aa-ffffff").unwrap();
        let stale_scan = det_1.scan().unwrap();
        let mut fresh_scan = stale_scan.clone();
        fresh_scan.push(t_added.clone());
        let mut by_id_race = BTreeMap::new();
        by_id_race.insert(t_early.id.to_string(), make_claim(&symlink_wt_1));
        by_id_race.insert(t_added.id.to_string(), make_claim(&symlink_wt_1b));
        let claims_race =
            crate::claims::ClaimSnapshot::from_parts(by_id_race, BTreeMap::new(), BTreeMap::new());
        let (race_scans, race_found) =
            claimed_elsewhere_with(&mut ctx, &[], &claims_race, |id, wt| {
                if wt == symlink_wt_1 {
                    Ok(Some((det_1.clone(), t_early.clone(), stale_scan.clone())))
                } else {
                    assert_eq!(*id, t_added.id);
                    Ok(Some((det_1.clone(), t_added.clone(), fresh_scan.clone())))
                }
            });
        assert_eq!(race_scans.len(), 2);
        for (at, task) in &race_found {
            assert!(race_scans[*at].1.iter().any(|s| s.id == task.id));
        }
    }
}
