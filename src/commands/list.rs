use super::ReadCtx;
use crate::claims::WaitingOn;
use crate::cli::FilterArgs;
use crate::error::Result;
use crate::filter::{Fields, TaskFilter, check_parent};
use crate::halt::{self, HaltSnapshot};
use crate::model::{Status, Task, TaskId};
use crate::needs::{Vocabularies, Without};
use crate::output::{
    Counts, DateColumn, DeferredSummary, HaltRow, ListOut, NextOut, Output, ParkedOut,
    PeriodicSummary, PrimeOut, TaskSummary,
};
use crate::query::{
    Picked, Readiness, SortKey, deferred_omission, is_candidate, readiness, sort_by_key, sort_list,
    sort_periodic, sort_ready,
};
use crate::scope::Scope;
use std::collections::HashMap;
use time::OffsetDateTime;

pub(super) fn resolve_dependency(ctx: &ReadCtx, all: &[Task], id: &TaskId) -> Result<Option<Task>> {
    let id = ctx.registry.canonical_id(id);
    match all.iter().find(|task| task.id == id) {
        Some(task) => Ok(Some(task.clone())),
        None => ctx.resolve_task(&id),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn list(
    mut ctx: ReadCtx,
    statuses: Vec<String>,
    filter: FilterArgs,
    sort: String,
    reverse: bool,
    parked: bool,
    periodic: bool,
    deferred: bool,
) -> Result<Output> {
    let sort = SortKey::parse(&sort)?;
    let filter = TaskFilter::parse(&filter, &statuses, &ctx.registry, &ctx.shorthand)?;
    if parked {
        return list_parked(ctx, filter);
    }
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    check_parent(&filter, &all, |_| false)?;
    let mut tasks = all.clone();
    tasks.retain(|task| {
        let periodic_ok = !periodic || task.every.is_some();
        let deferred_ok = !deferred || task.defer.is_some();
        // The default pool when --status is absent: open minus shelved, or every status
        // for --periodic, since most of a healthy series is closed (spec §5.2).
        let pool_ok = !filter.statuses().is_empty()
            || periodic
            || (task.status.is_open() && task.status != Status::Shelved);
        periodic_ok
            && deferred_ok
            && pool_ok
            && filter.matches(&Fields::of_task(task, &claims, &ctx.registry))
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
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| {
                let mut row = TaskSummary::of(task, &all, Some(&claims), &ctx.registry, now);
                row.project_color = ctx
                    .scope
                    .projects()
                    .iter()
                    .find(|project| project.prefix == task.id.prefix)
                    .and_then(|project| project.color);
                row
            })
            .collect(),
        halts: vec![],
        warnings: ctx.warnings,
        date: if periodic || deferred {
            DateColumn::Due
        } else {
            sort.date_column()
        },
    }))
}

fn list_parked(mut ctx: ReadCtx, filter: TaskFilter) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let rows = super::parked::rows(&mut ctx, &all, &claims, now)?;
    let warnings = std::mem::take(&mut ctx.warnings);
    check_parent(&filter, &all, |parent| {
        rows.iter().any(|row| {
            Fields::of_row(row, &ctx.registry)
                .is_some_and(|fields| fields.parent.as_ref() == Some(parent))
        })
    })?;
    let tasks = rows
        .into_iter()
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
    Ok(Output::Parked(ParkedOut { tasks, warnings }))
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

/// Ready tasks in ready order; pushes a warning per unreachable dependency.
pub fn ready_tasks(
    ctx: &mut ReadCtx,
    all: &[Task],
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
            is_candidate(task, now) && filter.matches(&Fields::of_task(task, claims, &ctx.registry))
        })
        .collect();
    for task in &selected {
        for dependency in &task.depends {
            if closed.contains_key(dependency) {
                continue;
            }
            let value =
                resolve_dependency(ctx, all, dependency)?.map(|task| !task.status.is_open());
            closed.insert(dependency.clone(), value);
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
        let has_children = !crate::hierarchy::children(all, &task.id, &ctx.registry).is_empty();
        match readiness(task, has_children, &lookup, now) {
            Readiness::Ready => ready.push(task.clone()),
            Readiness::Deferred => deferred.push(task.clone()),
            Readiness::Not => {}
        }
    }
    sort_ready(&mut ready);
    ready.retain(|task| match claims.live(&task.id) {
        Some(claim) => {
            warnings.push(format!(
                "{} — {} to take it over",
                claim_omission(&task.id, claim),
                force_hint(&task.id.to_string(), snapshots)
            ));
            false
        }
        None => true,
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
        claims.live(&task.id).is_none()
            && !claims
                .park(&task.id)
                .is_some_and(|park| park.waiting_on == WaitingOn::User)
    });
    ctx.warnings.extend(warnings);
    Ok(Picked {
        tasks: ready,
        deferred,
    })
}

fn halt_snapshots(ctx: &mut ReadCtx, all: &[Task]) -> HashMap<String, HaltSnapshot> {
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
    if matches!(ctx.scope, Scope::All(_)) {
        let scoped = ctx.scope.prefixes();
        for prefix in ctx.registry.projects.keys() {
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

/// Lanes/needs design §4.5: drop tasks held back by another session's exclusive hold,
/// with one warning per need and holder; order is unchanged. Whether a hold is this
/// session's is the read views' rule, `read_holds`: with a resolved identity, as
/// `occupants` decides a claim (the identity, or else the claim's process proof); with
/// none, every hold counts as another session's. A scope whose projects declare no
/// exclusive need reads no claim store and resolves no identity, so its output is exactly
/// what it was before holds existed.
fn retain_unheld(ctx: &mut ReadCtx, tasks: &mut Vec<Task>, now: OffsetDateTime) -> Result<()> {
    let vocabularies: HashMap<String, crate::needs::Vocabulary> = ctx
        .scope
        .projects()
        .iter()
        .filter(|project| project.needs.values().any(|need| need.exclusive))
        .map(|project| (project.prefix.clone(), project.needs.clone()))
        .collect();
    if vocabularies.is_empty() {
        return Ok(());
    }
    let (snapshot, warnings) = crate::holds::HoldSnapshot::load(&ctx.registry, now);
    ctx.warnings.extend(warnings);
    if snapshot.is_empty() {
        return Ok(());
    }
    let me = crate::claims::resolve_identity(&mut ctx.warnings);
    let mine = super::read_holds(&snapshot, &me)?;
    let mut held = crate::holds::HeldWarnings::default();
    tasks.retain(|task| {
        let Some(vocabulary) = vocabularies.get(&task.id.prefix) else {
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

/// Every in-scope project's vocabulary, by prefix: the names `--without` may use, and
/// the vocabulary each task's needs are judged by (spec §4.3). It borrows the scope
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
    let filter = TaskFilter::parse(&filter, &[], &ctx.registry, &ctx.shorthand)?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    check_parent(&filter, &all, |_| false)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let mut picked = ready_tasks(&mut ctx, &all, &claims, &snapshots, &filter, now)?;
    let halts = halt_rows(&snapshots, &all);
    let hidden = retain_allowed(&mut picked.tasks, &snapshots);
    warn_hidden(&mut ctx, hidden);
    if !without.is_empty() {
        let vocabs = vocabularies(&ctx.scope);
        let hidden = without.retain(&mut picked.tasks, &vocabs);
        let _ = without.retain(&mut picked.deferred, &vocabs);
        ctx.warnings.extend(without.warning(hidden));
    }
    retain_unheld(&mut ctx, &mut picked.tasks, now)?;
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut picked.tasks, cutoff, &claims);
        ctx.warnings
            .extend(crate::complexity::warnings(cutoff, &hidden));
        let _ = crate::complexity::apply(&mut picked.deferred, cutoff, &claims);
    }
    if let Some(warning) = deferred_omission(&picked.deferred) {
        ctx.warnings.push(warning);
    }
    let mut tasks = picked.tasks;
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::List(ListOut {
        tasks: tasks
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry, now))
            .collect(),
        halts,
        warnings: ctx.warnings,
        date: DateColumn::Updated,
    }))
}

/// The head of `ready` in the show shape, so a caller can start on it without a second
/// lookup. Nothing ready is a normal state: null, warnings, exit 0.
pub fn next(
    mut ctx: ReadCtx,
    max_complexity: Option<String>,
    without: Vec<String>,
) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(max_complexity.as_deref())?;
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let _ = super::parked::rows(&mut ctx, &all, &claims, now)?;
    let candidates = super::parked::candidates(&mut ctx, &all, &claims, now)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let ready = ready_tasks(
        &mut ctx,
        &all,
        &claims,
        &snapshots,
        &TaskFilter::default(),
        now,
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
    // On the whole pool, so parked-agent candidates pass the same gate.
    retain_unheld(&mut ctx, &mut pool, now)?;
    if let Some(cutoff) = cutoff {
        let hidden = crate::complexity::apply(&mut pool, cutoff, &claims);
        ctx.warnings
            .extend(crate::complexity::warnings(cutoff, &hidden));
        let _ = crate::complexity::apply(&mut omitted, cutoff, &claims);
    }
    if let Some(warning) = deferred_omission(&omitted) {
        ctx.warnings.push(warning);
    }
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
/// scan, the way park design §5.3 resolves a store-only park, so `doing` never drops a
/// live claim without a word. None is a `ready` or `next` candidate: `start` reads the
/// local file, so the warning names where to resume instead.
fn claimed_elsewhere(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &crate::claims::ClaimSnapshot,
) -> Vec<(Task, Vec<Task>)> {
    let mut found = Vec::new();
    for (key, (claim, liveness)) in claims.iter() {
        if *liveness != crate::claims::Liveness::Live {
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
        match super::parked::scan_recorded(&id, worktree) {
            Ok(Some(resolved)) => {
                ctx.warnings.push(format!(
                    "{id} is claimed in {}; resume it from that checkout",
                    claim.worktree
                ));
                found.push(resolved);
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
    found
}

pub fn prime(mut ctx: ReadCtx, closed: bool, without: Vec<String>) -> Result<Output> {
    let cutoff = crate::complexity::cutoff(None)?;
    let without = Without::from_env(&without, &vocabularies(&ctx.scope))?;
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
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
        .filter(|task| crate::defer::is_deferred(task, now))
        .filter_map(|task| task.defer)
        .collect();
    let next = waiting.iter().min().copied();
    let deferred = DeferredSummary {
        waiting: waiting.len(),
        next: next.map(|defer| defer.to_string()),
        in_days: next.map(|defer| crate::defer::days_until(defer.date(), now)),
        due: all
            .iter()
            .filter(|task| crate::defer::is_due(task, now))
            .count(),
    };
    let counts = Counts::of(&all);
    let parked = super::parked::rows(&mut ctx, &all, &claims, now)?;
    let snapshots = halt_snapshots(&mut ctx, &all);
    let mut ready = ready_tasks(
        &mut ctx,
        &all,
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
    retain_unheld(&mut ctx, &mut ready, now)?;
    let mut doing: Vec<Task> = all
        .iter()
        .filter(|task| task.status == Status::Doing || claims.live(&task.id).is_some())
        .cloned()
        .collect();
    let elsewhere = claimed_elsewhere(&mut ctx, &all, &claims);
    doing.extend(elsewhere.iter().map(|(task, _)| task.clone()));
    sort_list(&mut doing);
    let roadmap = crate::hierarchy::forest(
        &all,
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
    // goals are named in a warning instead, so none of them goes silent.
    let mut closeout: Vec<Task> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    for task in &all {
        // spec §4.3: todo, doing, or blocked; an idea is open but not a candidate
        if !matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
            || crate::hierarchy::children(&all, &task.id, &ctx.registry).is_empty()
            || !crate::hierarchy::open_descendants(&all, &task.id, &ctx.registry).is_empty()
        {
            continue;
        }
        let mut open = Vec::new();
        for dependency in &task.depends {
            // An unreachable dependency counts as open, exactly as `done` treats it: what
            // we cannot resolve, we cannot call closed.
            match resolve_dependency(&ctx, &all, dependency)? {
                Some(dependency) if !dependency.status.is_open() => {}
                _ => open.push(dependency.to_string()),
            }
        }
        if open.is_empty() {
            closeout.push(task.clone());
        } else {
            held.push(format!(
                "{} has no open children but is held by open dependencies: {}; it joins \
                 closeout when they close",
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
    ctx.warnings.extend(held);
    let wide = matches!(ctx.scope, Scope::All(_));
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
        if let Some(claim) = claims.live(&task.id)
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
        ctx.warnings.push(format!(
            "{id} has a stale claim from session {} ({why}); {} to take it over",
            claim.session,
            force_hint(id, &snapshots)
        ));
    }
    Ok(Output::Prime(PrimeOut {
        prefix: match &ctx.scope {
            Scope::Local(project) => Some(project.prefix.clone()),
            Scope::All(_) => None,
        },
        projects: ctx.scope.prefixes(),
        counts,
        periodic,
        deferred,
        closed,
        ready: ready
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry, now))
            .collect(),
        parked,
        doing: doing
            .iter()
            .map(|task| {
                let scan = elsewhere
                    .iter()
                    .find(|(found, _)| found.id == task.id)
                    .map_or(all.as_slice(), |(_, scan)| scan.as_slice());
                TaskSummary::of(task, scan, Some(&claims), &ctx.registry, now)
            })
            .collect(),
        roadmap,
        closeout: closeout
            .iter()
            .map(|task| TaskSummary::of(task, &all, Some(&claims), &ctx.registry, now))
            .collect(),
        halts,
        warnings: ctx.warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_resolution_prefers_the_captured_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "sci").unwrap();
        let dependency =
            crate::commands::add::blank(&project, "Dependency".into(), Status::Todo, None).unwrap();
        std::fs::write(project.root.join(crate::repo::CONFIG_REL), "not toml = [").unwrap();
        let mut registry = crate::registry::Registry::default();
        registry.register("sci", &project.root).unwrap();
        let ctx = ReadCtx {
            scope: Scope::All(vec![]),
            registry,
            warnings: vec![],
            shorthand: crate::shorthand::Shorthand::new(dir.path().to_path_buf()),
        };

        assert_eq!(
            resolve_dependency(&ctx, std::slice::from_ref(&dependency), &dependency.id).unwrap(),
            Some(dependency)
        );
    }
}
