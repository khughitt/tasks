//! The freeze while a migration is pending on this host, the restoring writes it admits,
//! and the resume's revalidation of the preflight inputs outside the journaled records
//! (spec §4.5 "Resume revalidation", "Restoring writes", "Freeze"; §4.6 "Other hosts").
//!
//! Restoring writes are judged per item, before the project-wide refusal, so a repair
//! inside the frozen successor is admitted. Two kinds of item exist:
//!
//! - a parent target S (`parent_targets`): a write to S is admitted when S fails the parent
//!   check before it, fails strictly fewer of its causes after it, introduces none, and
//!   changes nothing but `updated` and the fields of the causes it fixes
//!   ([`Failure::fields`]); a lane above S that a carried lane would nest inside is restored
//!   by clearing that ancestor's `lane`, changing nothing else on it;
//! - a (dependent, dropped-or-closed listed id) pair: its state is "the record is open and
//!   its `depends` names the id", expected present exactly when the journal's `release`
//!   for the id lists the dependent. A write is admitted when every pair it changes moves
//!   from mismatch to expectation, and it is either a change of exactly one `depends` entry
//!   with no other field changed (`updated` aside), or a reopen with no other field changed
//!   than those a reopen rewrites ([`REOPEN_FIELDS`]).

use super::MigrateCtx;
use super::journal::MigrationJournal;
use super::list::Decision;
use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::output::RefusalOut;
use crate::registry::Registry;
use crate::repo::Project;
use std::collections::{BTreeMap, BTreeSet};

/// What a reopen (`edit --status todo` from done or dropped) rewrites besides `status`.
const REOPEN_FIELDS: &[Field] = &[
    Field::Updated,
    Field::Status,
    Field::Completed,
    Field::Closed,
    Field::Defer,
    Field::Review,
];

#[derive(Clone, Copy)]
enum Field {
    Updated,
    Status,
    Completed,
    Closed,
    Defer,
    /// `reviewed`, `reviewed_at` and `awaiting_user`, which every status change clears.
    Review,
    /// `every` and its anchor `last_done`, which `--no-every` clears together.
    Every,
    Lane,
    Depends,
}

/// `after` equals `before` once `fields` are taken from `after`.
fn same_except(before: &Task, after: &Task, fields: &[Field]) -> bool {
    let mut moved = before.clone();
    for field in fields {
        match field {
            Field::Updated => moved.updated.clone_from(&after.updated),
            Field::Status => moved.status = after.status,
            Field::Completed => moved.completed.clone_from(&after.completed),
            Field::Closed => moved.closed.clone_from(&after.closed),
            Field::Defer => moved.defer.clone_from(&after.defer),
            Field::Review => {
                moved.reviewed = after.reviewed;
                moved.reviewed_at.clone_from(&after.reviewed_at);
                moved.awaiting_user = after.awaiting_user;
            }
            Field::Every => {
                moved.every = after.every;
                moved.last_done.clone_from(&after.last_done);
            }
            Field::Lane => moved.lane = after.lane,
            Field::Depends => moved.depends.clone_from(&after.depends),
        }
    }
    moved == *after
}

/// One edge from a record to an id a pending run names (listed or new). A `depends` edge
/// records whether its holder is open, so a status change that closes or reopens it is a
/// change of edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Edge {
    Depends(TaskId, bool),
    Parent(TaskId),
    WakeOn(TaskId),
}

fn edges(registry: &Registry, journal: &MigrationJournal, task: Option<&Task>) -> BTreeSet<Edge> {
    let Some(task) = task else {
        return BTreeSet::new();
    };
    let named = |id: &TaskId| {
        let id = registry.canonical_id(id);
        journal.names_id(&id).then_some(id)
    };
    let open = task.status.is_open();
    let mut out: BTreeSet<Edge> = task
        .depends
        .iter()
        .filter_map(|d| named(d).map(|d| Edge::Depends(d, open)))
        .collect();
    out.extend(task.parent.iter().filter_map(named).map(Edge::Parent));
    out.extend(task.wake_on.iter().filter_map(named).map(Edge::WakeOn));
    out
}

/// The freeze every record write passes (spec §4.5). With no pending migration on this
/// host it reads nothing. For each pending run: a restoring write is admitted; otherwise a
/// write to a record of its source or successor refuses, and in any other project a write
/// that changes an edge to an id the run names refuses.
pub fn check_write(
    registry: &Registry,
    project: &Project,
    before: Option<&Task>,
    after: &Task,
) -> Result<()> {
    let journals = MigrationJournal::pending()?;
    check_write_among(&journals, registry, project, before, after)
}

/// `check_write` reading `before` from disk, and only when a migration is pending: the
/// entry the central write paths use.
pub fn check_stored(registry: &Registry, project: &Project, after: &Task) -> Result<()> {
    let journals = MigrationJournal::pending()?;
    if journals.is_empty() {
        return Ok(());
    }
    let before = match project.read_task(&after.id) {
        Ok(task) => Some(task),
        Err(Error::TaskNotFound(_)) => None,
        Err(error) => return Err(error),
    };
    check_write_among(&journals, registry, project, before.as_ref(), after)
}

fn check_write_among(
    journals: &[MigrationJournal],
    registry: &Registry,
    project: &Project,
    before: Option<&Task>,
    after: &Task,
) -> Result<()> {
    let prefix = registry.canonical_prefix(&project.prefix);
    for journal in journals {
        if restoring(journal, registry, project, before, after)? {
            continue;
        }
        if journal.names_prefix(prefix) {
            return Err(journal.pending_error());
        }
        if edges(registry, journal, before) != edges(registry, journal, Some(after)) {
            return Err(match journal.pending_error() {
                Error::MovePending(detail) => Error::MovePending(format!(
                    "{detail}; until it completes, no edge to a task it lists may be added, removed, reopened or closed, and this write changes {}'s",
                    after.id
                )),
                other => other,
            });
        }
    }
    Ok(())
}

fn restoring(
    journal: &MigrationJournal,
    registry: &Registry,
    project: &Project,
    before: Option<&Task>,
    after: &Task,
) -> Result<bool> {
    let Some(before) = before else {
        return Ok(false);
    };
    if registry.canonical_prefix(&project.prefix) == journal.successor
        && restores_parent_target(journal, registry, project, before, after)?
    {
        return Ok(true);
    }
    Ok(restores_release(journal, registry, before, after))
}

/// A write to a parent target that fixes some of its causes and changes only their fields,
/// or a write to a lane above one that only clears `lane` and fixes that cause.
fn restores_parent_target(
    journal: &MigrationJournal,
    registry: &Registry,
    project: &Project,
    before: &Task,
    after: &Task,
) -> Result<bool> {
    let id = registry.canonical_id(&after.id);
    let carried = carried_targets(journal)?;
    if journal.parent_targets.contains(&id.to_string()) {
        let was = failures(project, registry, &carried, before, None)?;
        let now = failures(project, registry, &carried, after, None)?;
        let mut fields = vec![Field::Updated];
        for fixed in was.difference(&now) {
            fields.extend_from_slice(fixed.fields());
        }
        if !was.is_empty()
            && now.is_subset(&was)
            && now != was
            && same_except(before, after, &fields)
        {
            return Ok(true);
        }
    }
    if !(before.lane && !after.lane && same_except(before, after, &[Field::Updated, Field::Lane])) {
        return Ok(false);
    }
    for target in &journal.parent_targets {
        let target = match project.read_task(&TaskId::parse(target)?) {
            Ok(task) => task,
            Err(Error::TaskNotFound(_)) => continue,
            Err(error) => return Err(error),
        };
        let was = failures(project, registry, &carried, &target, None)?;
        if !was.contains(&Failure::InsideLane(id.clone())) {
            continue;
        }
        let now = failures(project, registry, &carried, &target, Some(after))?;
        if now.is_subset(&was) && !now.contains(&Failure::InsideLane(id.clone())) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether `task` is an open record whose `depends` names `listed`.
fn depends_on(registry: &Registry, task: &Task, listed: &TaskId) -> bool {
    task.status.is_open()
        && task
            .depends
            .iter()
            .any(|d| registry.canonical_id(d) == *listed)
}

fn restores_release(
    journal: &MigrationJournal,
    registry: &Registry,
    before: &Task,
    after: &Task,
) -> bool {
    let id = registry.canonical_id(&after.id).to_string();
    let mut changed = Vec::new();
    for entry in &journal.closed {
        let Ok(listed) = TaskId::parse(&entry.id) else {
            return false;
        };
        let (was, now) = (
            depends_on(registry, before, &listed),
            depends_on(registry, after, &listed),
        );
        if was == now {
            continue;
        }
        let expected = journal
            .release
            .get(&entry.id)
            .is_some_and(|released| released.contains(&id));
        if now != expected {
            return false;
        }
        changed.push(listed);
    }
    if changed.is_empty() {
        return false;
    }
    // Edges to the run's other ids (carried, new) stay as they are.
    let closing: BTreeSet<&str> = journal.closed.iter().map(|e| e.id.as_str()).collect();
    let others = |task: &Task| -> BTreeSet<Edge> {
        edges(registry, journal, Some(task))
            .into_iter()
            .filter(|edge| match edge {
                Edge::Depends(d, _) => !closing.contains(d.to_string().as_str()),
                _ => true,
            })
            .collect()
    };
    if others(before) != others(after) {
        return false;
    }
    let without = |task: &Task| -> Vec<TaskId> {
        task.depends
            .iter()
            .filter(|d| registry.canonical_id(d) != changed[0])
            .cloned()
            .collect()
    };
    let one_edge = changed.len() == 1
        && before.status == after.status
        && without(before) == without(after)
        && same_except(before, after, &[Field::Updated, Field::Depends]);
    let reopen = !before.status.is_open()
        && after.status.is_open()
        && same_except(before, after, REOPEN_FIELDS);
    one_edge || reopen
}

/// The carried records as the run writes them in the successor (inline notes only: the
/// parent check reads ids, parents and lanes).
fn carried_targets(journal: &MigrationJournal) -> Result<Vec<Task>> {
    journal
        .carried
        .iter()
        .map(|entry| {
            let physical =
                crate::format::parse_physical(&entry.target_text, &format!("{}.md", entry.target))?;
            let notes = physical.inline_notes.clone();
            Ok(physical.assemble(notes))
        })
        .collect()
}

/// Why a successor task may not hold the carried records placed under it: one cause per
/// item, each restored by one command.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Failure {
    Moved,
    /// Closed, by its status's name.
    Closed(&'static str),
    Recurring,
    Deferred,
    Lane,
    InsideLane(TaskId),
}

impl Failure {
    /// The fields the restoring write for this cause changes on the parent target, besides
    /// `updated`. A lane above it is restored on that ancestor, never by moving the target.
    fn fields(&self) -> &'static [Field] {
        match self {
            Failure::Moved | Failure::InsideLane(_) => &[],
            Failure::Closed(_) => REOPEN_FIELDS,
            Failure::Recurring => &[Field::Every],
            Failure::Deferred => &[Field::Defer],
            Failure::Lane => &[Field::Lane],
        }
    }

    fn settle(&self, target: &str) -> String {
        match self {
            Failure::Moved => format!(
                "parent target {target} was moved; no command restores it: restore its record from the synced history, or undo the run by hand"
            ),
            Failure::Closed(status) => format!(
                "parent target {target} is {status}; restore it with `tasks edit {target} --status todo`"
            ),
            Failure::Recurring => format!(
                "parent target {target} became a recurrence; restore it with `tasks edit {target} --no-every`"
            ),
            Failure::Deferred => format!(
                "parent target {target} became deferred; restore it with `tasks edit {target} --no-defer`"
            ),
            Failure::Lane => format!(
                "parent target {target} became a lane, which would nest a carried lane; restore it with `tasks edit {target} --no-lane`"
            ),
            Failure::InsideLane(lane) => format!(
                "parent target {target} is now inside lane {lane}, which would nest a carried lane; restore it with `tasks edit {lane} --no-lane`"
            ),
        }
    }
}

/// The causes for which `target` (one version of a parent target) fails the parent check
/// preflight made for it: open, not a recurrence, not deferred (a shelf aside), and no
/// lane at or above it when a carried lane is placed below it (one cause per lane above).
/// Ancestors are read from disk, except `overlay`, a record as a write would leave it.
fn failures(
    project: &Project,
    registry: &Registry,
    carried: &[Task],
    target: &Task,
    overlay: Option<&Task>,
) -> Result<BTreeSet<Failure>> {
    let id = registry.canonical_id(&target.id);
    let mut out = BTreeSet::new();
    if target.status == Status::Moved {
        out.insert(Failure::Moved);
    } else if !target.status.is_open() {
        out.insert(Failure::Closed(target.status.as_str()));
    }
    if target.every.is_some() {
        out.insert(Failure::Recurring);
    }
    if target.defer.is_some() && target.status != Status::Shelved {
        out.insert(Failure::Deferred);
    }
    let holds_lane = carried
        .iter()
        .filter(|t| t.parent.as_ref().map(|p| registry.canonical_id(p)) == Some(id.clone()))
        .any(|t| {
            t.lane
                || crate::hierarchy::descendants(carried, &t.id, registry)
                    .iter()
                    .any(|d| d.lane)
        });
    if holds_lane {
        if target.lane {
            out.insert(Failure::Lane);
        }
        let mut seen = BTreeSet::from([id.clone()]);
        let mut current = target.parent.as_ref().map(|p| registry.canonical_id(p));
        while let Some(ancestor) = current {
            if !seen.insert(ancestor.clone()) {
                break;
            }
            let record = match overlay {
                Some(task) if registry.canonical_id(&task.id) == ancestor => task.clone(),
                _ => match project.read_task(&ancestor) {
                    Ok(record) => record,
                    Err(Error::TaskNotFound(_)) => break,
                    Err(error) => return Err(error),
                },
            };
            if record.lane {
                out.insert(Failure::InsideLane(ancestor));
            }
            current = record.parent.as_ref().map(|p| registry.canonical_id(p));
        }
    }
    Ok(out)
}

fn refusal(id: &str, rule: &str, settle: String) -> RefusalOut {
    RefusalOut {
        id: id.to_string(),
        rule: rule.to_string(),
        settle,
    }
}

/// Spec §4.5 resume revalidation: the preflight checks whose inputs lie outside the
/// journaled records, each difference with the one command that restores it. Parent
/// targets are judged while the destination records are still to be written (resuming at
/// or before `destination`), releases while the closures are (at or before `closures`):
/// once a phase has written what an input decided, re-judging it can no longer refuse
/// anything the run would do. Carried records cannot gain a dependency on a dropped task
/// without changing journaled bytes, which classification already refuses.
pub fn revalidate(
    ctx: &MigrateCtx,
    journal: &MigrationJournal,
    from: super::classify::Phase,
) -> Result<Vec<RefusalOut>> {
    use super::classify::Phase;
    let mut out = Vec::new();
    if from <= Phase::Destination {
        let carried = carried_targets(journal)?;
        for target in &journal.parent_targets {
            let id = TaskId::parse(target)?;
            match ctx.target.read_task(&id) {
                Ok(task) => {
                    for failure in failures(&ctx.target, &ctx.registry, &carried, &task, None)? {
                        out.push(refusal(target, "parent_target", failure.settle(target)));
                    }
                }
                Err(Error::TaskNotFound(_)) => out.push(refusal(
                    target,
                    "parent_target",
                    format!(
                        "parent target {target} no longer exists in {}; no command restores it: restore its record from the synced history",
                        journal.successor
                    ),
                )),
                Err(error) => return Err(error),
            }
        }
    }
    if from <= Phase::Closures {
        out.extend(release_changes(ctx, journal)?);
    }
    Ok(out)
}

/// Each dropped or closed task's open dependents against its journaled `release`, counted
/// as preflight counted them: tasks the list also drops or closes are not dependents, a
/// carried record (either copy) counts as its v1 id for a close and not at all for a drop.
fn release_changes(ctx: &MigrateCtx, journal: &MigrationJournal) -> Result<Vec<RefusalOut>> {
    let registry = &ctx.registry;
    let wide = super::release::read(registry, &ctx.source.project.root)?;
    let closing: BTreeSet<TaskId> = journal
        .closed
        .iter()
        .map(|entry| TaskId::parse(&entry.id))
        .collect::<Result<_>>()?;
    let mut v1: BTreeMap<TaskId, TaskId> = BTreeMap::new();
    for entry in &journal.carried {
        let source = TaskId::parse(&entry.source)?;
        v1.insert(TaskId::parse(&entry.target)?, source.clone());
        v1.insert(source.clone(), source);
    }
    let dependents = super::release::open_dependents(&wide.tasks, registry, &closing);
    let records: BTreeMap<TaskId, &Task> = wide
        .tasks
        .iter()
        .map(|task| (registry.canonical_id(&task.id), task))
        .collect();
    let mut out = Vec::new();
    for entry in &journal.closed {
        let listed = TaskId::parse(&entry.id)?;
        let close = entry.decision() == Decision::Close;
        let current: BTreeSet<String> = dependents
            .get(&listed)
            .into_iter()
            .flatten()
            .filter(|dep| !closing.contains(*dep))
            .filter_map(|dep| match v1.get(dep) {
                Some(original) => close.then(|| original.to_string()),
                None => Some(dep.to_string()),
            })
            .collect();
        let expected: BTreeSet<String> = journal
            .release
            .get(&entry.id)
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let d = &entry.id;
        for added in current.difference(&expected) {
            out.push(refusal(
                added,
                "release_changed",
                format!(
                    "{added} now depends on {d}, which the run releases without it; restore it with `tasks dep {added} --rm {d}`"
                ),
            ));
        }
        for removed in expected.difference(&current) {
            let record = TaskId::parse(removed)
                .ok()
                .and_then(|id| records.get(&id).copied());
            let settle = match record {
                Some(task)
                    if !task.status.is_open()
                        && task.status != Status::Moved
                        && task
                            .depends
                            .iter()
                            .any(|dep| registry.canonical_id(dep) == listed) =>
                {
                    let unreleased = reopen_unreleased(journal, registry, task)?;
                    if unreleased.is_empty() {
                        format!(
                            "{removed} was closed, so the run would release nothing there; restore it with `tasks edit {removed} --status todo`"
                        )
                    } else {
                        format!(
                            "{removed} was closed, so the run would release nothing there; no single command restores it, since reopening it would also reopen its dependency on {}, which the run does not release it from: restore its record from the synced history",
                            unreleased.join(", ")
                        )
                    }
                }
                Some(task) if task.status.is_open() => format!(
                    "{removed} no longer depends on {d}, which the run releases it from; restore it with `tasks dep {removed} --on {d}`"
                ),
                Some(task) => format!(
                    "{removed} is {} without its dependency on {d}; no single command restores it: restore its record from the synced history",
                    task.status.as_str()
                ),
                None => format!(
                    "{removed}, which the run releases from {d}, can no longer be read; restore its record, or make its project reachable"
                ),
            };
            out.push(refusal(removed, "release_changed", settle));
        }
    }
    Ok(out)
}

/// The ids whose edges reopening the closed `task` would reactivate against the journal's
/// expectation: empty exactly when the freeze admits `tasks edit <task> --status todo` as a
/// restoring write (the reopen is simulated and judged by the freeze's own rule).
fn reopen_unreleased(
    journal: &MigrationJournal,
    registry: &Registry,
    task: &Task,
) -> Result<Vec<String>> {
    let mut reopened = task.clone();
    let now = crate::time::after(&task.updated)?;
    crate::commands::apply_status_effects(&mut reopened, Status::Todo, &now, None)?;
    reopened.updated = now;
    if restores_release(journal, registry, task, &reopened) {
        return Ok(Vec::new());
    }
    let id = registry.canonical_id(&task.id).to_string();
    Ok(task
        .depends
        .iter()
        .map(|dep| registry.canonical_id(dep))
        .filter(|dep| journal.names_id(dep))
        .filter(|dep| {
            !journal
                .release
                .get(&dep.to_string())
                .is_some_and(|released| released.contains(&id))
        })
        .map(|dep| dep.to_string())
        .collect())
}

/// The refusal over revalidation differences: `move_pending`, naming each. `fresh` is a
/// run that has just written its journal and nothing else; otherwise a resume.
pub fn refusal_error(journal: &MigrationJournal, refusals: &[RefusalOut], fresh: bool) -> Error {
    let state = if fresh {
        "cannot start: its preflight inputs changed while its journal was written; only the journal was written"
    } else {
        "cannot resume: its preflight inputs changed since the journal was written; nothing was written"
    };
    Error::MovePending(format!(
        "the pending migration of {} to {} {state}: {}; run each command, then `tasks migrate {}` again",
        journal.source,
        journal.successor,
        refusals
            .iter()
            .map(|r| format!("{} [{}]: {}", r.id, r.rule, r.settle))
            .collect::<Vec<_>>()
            .join("; "),
        crate::stale::quote(&journal.list_path.display().to_string())
    ))
}
