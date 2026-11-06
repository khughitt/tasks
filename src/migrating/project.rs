//! Projecting a decision list (docs/project-operations.md). Carried entries are rendered as `move`
//! renders them, then rewritten (parent, dependencies within the carried set, `wake_on`,
//! dropped links, a replacement body), and only then validated in the destination, over the
//! whole projected set at once. Dropped and closed entries are rendered through the
//! tracker's own status effects and must release exactly their open dependents. `preflight`
//! joins both with the list checks.

use super::MigrateCtx;
use super::list::{Decision, DecisionList, Entry, LoadedList};
use super::release::{self, Wide};
use crate::claims::{ClaimStore, Escalation, Park};
use crate::commands;
use crate::error::{Error, Result};
use crate::model::{Relation, Status, Task, TaskId};
use crate::moving::preflight::{
    RenderedPair, build_pair, collision, finish_pair, validate_destination_docs,
    validate_record_among,
};
use crate::output::RefusalOut;
use crate::refs::Citation;
use crate::resolve::{DocKind, LocatedEntry, Resolver};
use std::collections::{BTreeMap, BTreeSet};

/// The carried records in list order, the map from each carried id to its new id, and
/// every refusal.
pub type Projected = (Vec<Carried>, BTreeMap<TaskId, TaskId>, Vec<RefusalOut>);

pub struct Carried {
    pub from: TaskId,
    /// The source record's bytes as read.
    pub source_text: String,
    pub pair: RenderedPair,
    /// One per rewrite, in the order their notes were appended: the note text without its
    /// `migration: ` prefix and ` list: sha256:…` suffix.
    pub rewrites: Vec<String>,
    /// The diff from the current body to the replacement, when the list gives `body`.
    pub body_change: Option<String>,
}

/// A spec, plan, step or reference entry that does not resolve in the successor.
pub(crate) enum Link {
    Spec {
        path: String,
        error: String,
    },
    Plan {
        path: String,
        error: String,
    },
    /// The plan resolves there but has no such heading.
    Step {
        plan: String,
        step: String,
    },
    Ref {
        handle: String,
        reference: String,
    },
}

/// The links of `task` that do not resolve through `resolver` (the successor's). A document
/// that is absent or not addressable there is a link to report; any other failure to look
/// is an error. Output references are never checked.
pub(crate) fn unresolved_links(task: &Task, resolver: &Resolver) -> Result<Vec<Link>> {
    let mut links = Vec::new();
    let mut plan_resolves = true;
    for (kind, path) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
        let Some(path) = path else { continue };
        if crate::format::external_doc(path)?.is_some() {
            continue;
        }
        match resolver.resolve_doc(kind, path) {
            Ok(_) => {}
            Err(error @ (Error::DocNotFound(_) | Error::Ambiguous(_) | Error::Validation(_))) => {
                let (path, error) = (path.clone(), error.to_string());
                links.push(match kind {
                    DocKind::Spec => Link::Spec { path, error },
                    DocKind::Plan => {
                        plan_resolves = false;
                        Link::Plan { path, error }
                    }
                });
            }
            Err(error) => return Err(error),
        }
    }
    if let (Some(plan), Some(step)) = (&task.plan, &task.step)
        && plan_resolves
        && crate::format::external_doc(plan)?.is_none()
        && !resolver.step_exists(plan, step)?
    {
        links.push(Link::Step {
            plan: plan.clone(),
            step: step.clone(),
        });
    }
    for entry in &task.refs {
        if entry.relation == Relation::Output {
            continue;
        }
        let crate::refs::Location::Path(path) = crate::refs::location(&entry.reference)? else {
            continue;
        };
        if matches!(resolver.locate_entry(path)?, LocatedEntry::Missing) {
            links.push(Link::Ref {
                handle: entry.handle.clone(),
                reference: entry.reference.clone(),
            });
        }
    }
    Ok(links)
}

/// The handles `body` cites as `[ref:<handle>]`.
pub(crate) fn cited(body: &str) -> BTreeSet<String> {
    crate::refs::citations(body)
        .into_iter()
        .filter_map(|c| match c {
            Citation::Handle(h) => Some(h.to_string()),
            Citation::Malformed(_) => None,
        })
        .collect()
}

/// The one time every record of a run is stamped with: after every listed record's
/// `updated`, so each written copy is strictly newer than the one it replaces.
pub fn render_time(ctx: &MigrateCtx) -> Result<String> {
    let mut now = crate::time::now();
    for task in super::draft::eligible(ctx)? {
        now = now.max(crate::time::after(&task.updated)?);
    }
    Ok(now)
}

fn refuse(id: &TaskId, rule: &str, settle: impl Into<String>) -> RefusalOut {
    RefusalOut {
        id: id.to_string(),
        rule: rule.to_string(),
        settle: settle.into(),
    }
}

/// Destination checks whose failure is a property of the projected record, reported as a
/// refusal; anything else (I/O, a malformed config) stops the run.
fn refusable(error: &Error) -> bool {
    matches!(
        error,
        Error::Validation(_)
            | Error::Cycle(_)
            | Error::NestedLane(_)
            | Error::UnresolvableId(_)
            | Error::InvalidId(..)
            | Error::TaskNotFound(_)
            | Error::Ambiguous(_)
            | Error::DocNotFound(_)
            | Error::SpecialFile(_)
            | Error::Reference { .. }
            | Error::RefNotFound(_)
            | Error::Moved(_)
            | Error::MovePending(_)
            | Error::UnknownNeed(_)
            | Error::AttachmentUnsafe(_)
    )
}

struct Work<'a> {
    entry: &'a Entry,
    carried: Carried,
    /// The parent was refused or is in a refused cycle: validate as if it had none.
    parent_cut: bool,
    /// Unresolved links kept because the entry lacks `links = "drop"` (already refused).
    links_kept: Vec<Link>,
}

/// Appends one stamped rewrite note and records its text.
fn rewrite(
    source: &mut commands::Ctx,
    owner: &str,
    suffix: &str,
    carried: &mut Carried,
    text: String,
) -> Result<()> {
    commands::append_stamped_note(
        source,
        &mut carried.pair.target,
        owner,
        &format!("migration: {text}{suffix}"),
    )?;
    carried.rewrites.push(text);
    Ok(())
}

/// Renders every carried entry of a list that passed `check_list`, applies the rewrites
/// docs/project-operations.md allows, and validates the projected destination. Returns the carried records
/// in list order, the map from each carried id to its new id, and every refusal. Writes
/// nothing.
pub fn project_carried(ctx: &mut MigrateCtx, list: &LoadedList, now: &str) -> Result<Projected> {
    let suffix = format!(" list: sha256:{}", list.sha256);
    let source_prefix = ctx.source.project.prefix.clone();
    let succ = ctx.target.prefix.clone();
    let eligible: BTreeMap<TaskId, Task> = super::draft::eligible(ctx)?
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    // The decisions, keyed by canonical id. `preflight` passes only the entries whose id and
    // decision check_list accepted; anything else here is a caller error.
    let mut decided: BTreeMap<TaskId, Decision> = BTreeMap::new();
    let mut carry: Vec<(TaskId, &Entry)> = Vec::new();
    for entry in &list.list.tasks {
        let named = TaskId::parse(entry.id.trim())
            .map(|id| ctx.registry.canonical_id(&id))
            .ok()
            .filter(|id| eligible.contains_key(id));
        let (Some(id), Some(decision)) = (named, Decision::parse(&entry.decision)) else {
            return Err(Error::Validation(format!(
                "entry {:?} did not pass the list checks; run `tasks migrate <list> --explain`",
                entry.id
            )));
        };
        if decided.insert(id.clone(), decision).is_some() {
            return Err(Error::Validation(format!(
                "{id} is listed twice; run `tasks migrate <list> --explain`"
            )));
        }
        if decision == Decision::Carry {
            carry.push((id, entry));
        }
    }

    // Eligible tasks with no usable entry: check_list has refused each, so a carried task
    // under one is not refused a second time for its parent.
    let unsettled: BTreeSet<TaskId> = eligible
        .keys()
        .filter(|id| !decided.contains_key(*id))
        .cloned()
        .collect();

    // Every new id first: fresh in the destination and among themselves.
    let mut map: BTreeMap<TaskId, TaskId> = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for (id, _) in &carry {
        let mut fresh = None;
        for _ in 0..16 {
            let candidate = ctx.target.new_id()?;
            if taken.insert(candidate.clone()) {
                fresh = Some(candidate);
                break;
            }
        }
        let fresh = fresh.ok_or_else(|| {
            Error::Validation(format!("could not allocate a free {succ} id for {id}"))
        })?;
        map.insert(id.clone(), fresh);
    }
    let owner = commands::owner_name(&ctx.source.project)?;
    let mut refusals = Vec::new();
    let mut works = Vec::new();
    for (id, entry) in &carry {
        let mut pair = build_pair(
            &mut ctx.source,
            &ctx.target,
            &eligible[id],
            map[id].clone(),
            now,
            &suffix,
        )?;
        // A carried done recurrence keeps its completion stamps on the carried record; its
        // tombstone is moved, not closed, and carries neither (as `check` requires).
        if eligible[id].status == Status::Done {
            pair.tombstone.completed = None;
            pair.tombstone.closed = None;
        }
        let source_text = rendered_bytes(ctx, &eligible[id])?;
        works.push(Work {
            entry,
            carried: Carried {
                from: id.clone(),
                source_text,
                pair,
                rewrites: Vec::new(),
                body_change: None,
            },
            parent_cut: false,
            links_kept: Vec::new(),
        });
    }

    let host = ctx.source.host.clone();
    let resolver = Resolver::new(&ctx.target, &ctx.registry, &host);
    for work in &mut works {
        let id = work.carried.from.clone();
        let entry = work.entry;
        let canonical = |t: &TaskId| ctx.registry.canonical_id(t);

        // Parent.
        let v1_parent = work.carried.pair.target.parent.as_ref().map(canonical);
        let settle_parent = format!(
            "set `parent` to \"none\", a carried {source_prefix} id, or an open {succ} task id"
        );
        // `Err(None)`: the parent's own entry is refused, so this one is not refused again.
        let new_parent: std::result::Result<Option<TaskId>, Option<(&str, String)>> =
            match (&entry.parent, &v1_parent) {
                (None, None) => Ok(None),
                (None, Some(p)) => match map.get(p) {
                    Some(new) => Ok(Some(new.clone())),
                    None if unsettled.contains(p) => Err(None),
                    None => Err(Some((
                        "parent_required",
                        format!("its parent {p} is not carried; {settle_parent}"),
                    ))),
                },
                (Some(value), _) => parse_parent(ctx, &map, value)?
                    .map_err(|why| Some(("parent_invalid", format!("{why}; {settle_parent}")))),
            };
        match new_parent {
            Ok(parent) => {
                let text = match (&v1_parent, &entry.parent, &parent) {
                    (None, _, None) => None,
                    (None, _, Some(new)) => Some(format!("parent set to {new}")),
                    (Some(p), None, Some(new)) => Some(format!("parent {p} carried as {new}")),
                    (Some(p), _, parent) => {
                        let to = parent
                            .as_ref()
                            .map_or("none".to_string(), ToString::to_string);
                        if map.contains_key(p) {
                            Some(format!("parent {p} overridden; set to {to}"))
                        } else {
                            Some(format!("parent {p} not carried; set to {to}"))
                        }
                    }
                };
                work.carried.pair.target.parent = parent;
                if let Some(text) = text {
                    rewrite(&mut ctx.source, &owner, &suffix, &mut work.carried, text)?;
                }
            }
            Err(refused) => {
                if let Some((rule, settle)) = refused {
                    refusals.push(refuse(&id, rule, settle));
                }
                work.carried.pair.target.parent = None;
                work.parent_cut = true;
            }
        }

        // Dependencies.
        let depends: BTreeSet<TaskId> = work
            .carried
            .pair
            .target
            .depends
            .iter()
            .map(canonical)
            .collect();
        let mut removed = BTreeSet::new();
        for raw in &entry.rm_depends {
            let named = TaskId::parse(raw.trim()).map(|t| canonical(&t));
            match named {
                Ok(dep) if depends.contains(&dep) && decided.get(&dep) == Some(&Decision::Drop) => {
                    removed.insert(dep);
                }
                Ok(dep) if depends.contains(&dep) => refusals.push(refuse(
                    &id,
                    "bad_key",
                    format!("`rm_depends` names {dep}, which this list does not drop; only an edge to dropped work is removed, so take it out of `rm_depends`"),
                )),
                _ => refusals.push(refuse(
                    &id,
                    "bad_key",
                    format!("`rm_depends` names {raw:?}, which {id} does not depend on; take it out of `rm_depends`"),
                )),
            }
        }
        let mut kept = Vec::new();
        for dep in std::mem::take(&mut work.carried.pair.target.depends) {
            let c = canonical(&dep);
            if let Some(new) = map.get(&c) {
                kept.push(new.clone());
                rewrite(
                    &mut ctx.source,
                    &owner,
                    &suffix,
                    &mut work.carried,
                    format!("dependency {c} carried as {new}"),
                )?;
            } else if decided.get(&c) == Some(&Decision::Drop) {
                if removed.contains(&c) {
                    rewrite(
                        &mut ctx.source,
                        &owner,
                        &suffix,
                        &mut work.carried,
                        format!("removed dependency {c} (dropped)"),
                    )?;
                } else {
                    refusals.push(refuse(
                        &id,
                        "depends_on_dropped",
                        format!("{id} depends on {c}, which this list drops; add \"{c}\" to its `rm_depends` (the edge is removed and noted) or carry {c}: a drop must not make {id} ready unannounced"),
                    ));
                    kept.push(dep);
                }
            } else {
                kept.push(dep);
            }
        }
        work.carried.pair.target.depends = kept;

        // wake_on.
        let mut wake_on = Vec::new();
        for target in std::mem::take(&mut work.carried.pair.target.wake_on) {
            let c = canonical(&target);
            match map.get(&c) {
                Some(new) => {
                    wake_on.push(new.clone());
                    rewrite(
                        &mut ctx.source,
                        &owner,
                        &suffix,
                        &mut work.carried,
                        format!("wake_on {c} carried as {new}"),
                    )?;
                }
                None => wake_on.push(target),
            }
        }
        work.carried.pair.target.wake_on = wake_on;

        // Links.
        let drop_links = match entry.links.as_deref() {
            None => false,
            Some("drop") => true,
            Some(other) => {
                refusals.push(refuse(
                    &id,
                    "bad_key",
                    format!("`links` is {other:?}; its only value is \"drop\""),
                ));
                false
            }
        };
        let links = unresolved_links(&work.carried.pair.target, &resolver)?;
        let mut dropped_handles = BTreeSet::new();
        if !links.is_empty() && !drop_links {
            let named: Vec<String> = links
                .iter()
                .map(|link| match link {
                    Link::Spec { path, .. } => format!("spec {path}"),
                    Link::Plan { path, .. } => format!("plan {path}"),
                    Link::Step { plan, step } => format!("step {step:?} of {plan}"),
                    Link::Ref { handle, reference } => format!("ref {handle} {reference}"),
                })
                .collect();
            refusals.push(refuse(
                &id,
                "links_unresolved",
                format!("{} not in {succ}; set `links = \"drop\"` to clear them (each is noted with its old value), or add them to {succ}", named.join(", ")),
            ));
            work.links_kept = links;
        } else {
            for link in links {
                let target = &mut work.carried.pair.target;
                let mut texts = Vec::new();
                match link {
                    Link::Spec { path, .. } => {
                        target.spec = None;
                        texts.push(format!("spec link {path} dropped (not in {succ})"));
                    }
                    Link::Plan { path, .. } => {
                        target.plan = None;
                        texts.push(format!("plan link {path} dropped (not in {succ})"));
                        if let Some(step) = target.step.take() {
                            texts.push(format!("step {step:?} dropped with its plan"));
                        }
                    }
                    Link::Step { plan, step } => {
                        target.step = None;
                        texts.push(format!("step {step:?} of {plan} dropped (not in {succ})"));
                    }
                    Link::Ref { handle, reference } => {
                        target.refs.retain(|entry| entry.handle != handle);
                        texts.push(format!("ref {handle} {reference} dropped (not in {succ})"));
                        dropped_handles.insert(handle);
                    }
                }
                for text in texts {
                    rewrite(&mut ctx.source, &owner, &suffix, &mut work.carried, text)?;
                }
            }
        }

        // Cited references and the replacement body.
        let old_cites = cited(&work.carried.pair.target.body);
        let cut: Vec<String> = old_cites.intersection(&dropped_handles).cloned().collect();
        let list_cites = |handles: &[String]| {
            handles
                .iter()
                .map(|h| format!("[ref:{h}]"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match &entry.body {
            None if !cut.is_empty() => refusals.push(refuse(
                &id,
                "citation_dangling",
                format!("the body cites {}, whose entry `links = \"drop\"` removes; give `body`, a replacement body that does not cite it", list_cites(&cut)),
            )),
            None => {}
            Some(body) => match crate::format::validate_body(body) {
                Err(error) => refusals.push(refuse(&id, "bad_key", format!("`body`: {error}"))),
                Ok(()) => {
                    let remaining: BTreeSet<String> = work
                        .carried
                        .pair
                        .target
                        .refs
                        .iter()
                        .map(|entry| entry.handle.clone())
                        .collect();
                    let dangling: Vec<String> =
                        cited(body).difference(&remaining).cloned().collect();
                    if !dangling.is_empty() {
                        refusals.push(refuse(
                            &id,
                            "citation_dangling",
                            format!("the replacement `body` cites {}, which the carried record has no entry for; remove the citation from `body`", list_cites(&dangling)),
                        ));
                    }
                    work.carried.pair.target.body = body.clone();
                    let text = if cut.is_empty() {
                        "body replaced".to_string()
                    } else {
                        format!("body replaced; dropped citations: {}", list_cites(&cut))
                    };
                    rewrite(&mut ctx.source, &owner, &suffix, &mut work.carried, text)?;
                }
            },
        }
    }

    // The projected hierarchy: a parent cycle among carried records.
    let targets: Vec<Task> = works
        .iter()
        .map(|w| w.carried.pair.target.clone())
        .collect();
    let mut cycles = BTreeSet::new();
    let mut in_cycle = BTreeSet::new();
    for target in &targets {
        let Some(cycle) = crate::hierarchy::parent_cycle(&targets, &target.id, &ctx.registry)
        else {
            continue;
        };
        let members: BTreeSet<TaskId> = cycle.iter().cloned().collect();
        if cycles.insert(members.clone()) {
            let from = |new: &TaskId| {
                map.iter()
                    .find(|(_, n)| *n == new)
                    .map_or(new.to_string(), |(v1, _)| v1.to_string())
            };
            let first = works
                .iter()
                .find(|w| members.contains(&w.carried.pair.target.id))
                .map(|w| w.carried.from.clone())
                .expect("a cycle among carried records");
            refusals.push(refuse(
                &first,
                "parent_cycle",
                format!(
                    "the `parent` keys form a cycle ({}); change `parent` on one of them",
                    cycle.iter().map(from).collect::<Vec<_>>().join(" -> ")
                ),
            ));
        }
        in_cycle.extend(members);
    }

    // Render, and diff each replacement body against what the record will store.
    for work in &mut works {
        let pair = &mut work.carried.pair;
        if in_cycle.contains(&pair.target.id) {
            work.parent_cut = true;
        }
        finish_pair(pair)?;
        if work.entry.body.is_some() {
            let stored = crate::format::parse_physical(
                &pair.target_text,
                &format!("projected {}", pair.target.id),
            )?
            .body()
            .to_string();
            pair.target.body = stored;
            finish_pair(pair)?;
            let old = &eligible[&work.carried.from].body;
            if *old != pair.target.body {
                work.carried.body_change =
                    Some(crate::migrating::review::body_diff(old, &pair.target.body));
            }
        }
    }

    // Destination validation over the complete projected set. A record already refused for
    // its parent or its links is judged without them, so one cause yields one refusal.
    let views: Vec<Task> = works
        .iter()
        .map(|work| {
            let mut view = work.carried.pair.target.clone();
            if work.parent_cut {
                view.parent = None;
            }
            for link in &work.links_kept {
                match link {
                    Link::Spec { .. } => view.spec = None,
                    Link::Plan { .. } => {
                        view.plan = None;
                        view.step = None;
                    }
                    Link::Step { .. } => view.step = None,
                    Link::Ref { handle, .. } => view.refs.retain(|e| &e.handle != handle),
                }
            }
            view
        })
        .collect();
    for (work, view) in works.iter().zip(&views) {
        let from = &work.carried.from;
        let checked = validate_record_among(&ctx.target, &ctx.registry, view, &views)
            .and_then(|()| {
                validate_destination_docs(&ctx.target, &ctx.registry, view, from, &ctx.source.host)
            })
            .and_then(|()| crate::attachments::check_storage(&ctx.target, &view.id).map(|_| ()));
        match checked {
            Ok(()) => {}
            Err(error) if refusable(&error) => refusals.push(refuse(
                from,
                "destination_invalid",
                format!("in {succ}: {error}"),
            )),
            Err(error) => return Err(error),
        }
        if let Some((other, why)) = collision(&ctx.target, &ctx.registry, from)? {
            refusals.push(refuse(
                from,
                "destination_invalid",
                format!("{succ} already holds {other} for {from} ({why}); inspect it: drop one, or keep the re-file and drop {from}"),
            ));
        }
    }

    // The dependency graph as it will stand, following tombstone pointers as `move` does:
    // a carried v1 id leads to its new record.
    let by_id: BTreeMap<TaskId, &Task> = views.iter().map(|v| (v.id.clone(), v)).collect();
    let edges = |id: &TaskId| -> Result<Option<Vec<TaskId>>> {
        let id = ctx.registry.canonical_id(id);
        if let Some(view) = by_id.get(&id) {
            return Ok(Some(
                view.depends
                    .iter()
                    .map(|d| ctx.registry.canonical_id(d))
                    .collect(),
            ));
        }
        if let Some(new) = map.get(&id) {
            return Ok(Some(vec![new.clone()]));
        }
        Ok(resolver.resolve_task(&id)?.map(|task| {
            crate::tombstone::edges(&task)
                .iter()
                .map(|d| ctx.registry.canonical_id(d))
                .collect()
        }))
    };
    let mut seen = BTreeSet::new();
    for (work, view) in works.iter().zip(&views) {
        let from = &work.carried.from;
        match crate::query::find_cycle(&view.id, &edges) {
            Ok(None) => {}
            Ok(Some(cycle)) => {
                if seen.insert(cycle.iter().cloned().collect::<BTreeSet<_>>()) {
                    refusals.push(refuse(
                        from,
                        "dependency_cycle",
                        format!(
                            "once carried, the dependencies form a cycle ({}); remove one edge with `tasks dep <id> --rm <dependency>`",
                            cycle
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(" -> ")
                        ),
                    ));
                }
            }
            Err(Error::UnresolvableId(missing)) => refusals.push(refuse(
                from,
                "destination_invalid",
                format!("a dependency of {from} reaches {missing}, which cannot be resolved; remove that edge with `tasks dep <id> --rm {missing}`"),
            )),
            Err(error) => return Err(error),
        }
    }

    Ok((
        works.into_iter().map(|w| w.carried).collect(),
        map,
        refusals,
    ))
}

/// What a `parent` key names: `"none"`, a carried v1 id (as its new id), or an open
/// successor task. The inner `Err` says why it names none of these; the outer one is a
/// failure to read the successor.
fn parse_parent(
    ctx: &MigrateCtx,
    map: &BTreeMap<TaskId, TaskId>,
    value: &str,
) -> Result<std::result::Result<Option<TaskId>, String>> {
    if value == "none" {
        return Ok(Ok(None));
    }
    let Ok(id) = TaskId::parse(value.trim()).map(|id| ctx.registry.canonical_id(&id)) else {
        return Ok(Err(format!("`parent = {value:?}` is not a task id")));
    };
    if id.prefix == ctx.source.project.prefix {
        return Ok(map
            .get(&id)
            .map(|new| Some(new.clone()))
            .ok_or_else(|| format!("`parent` names {id}, which this list does not carry")));
    }
    if id.prefix != ctx.target.prefix {
        return Ok(Err(format!(
            "`parent` names {id}, which is neither a carried {} task nor a {} task",
            ctx.source.project.prefix, ctx.target.prefix
        )));
    }
    Ok(match ctx.target.read_task(&id) {
        Ok(task) if task.status.is_open() => Ok(Some(id)),
        Ok(task) => Err(format!(
            "`parent` names {id}, which is {}; a parent must be open",
            task.status.as_str()
        )),
        Err(Error::TaskNotFound(_)) => Err(format!(
            "`parent` names {id}, which does not exist in {}",
            ctx.target.prefix
        )),
        Err(error) => return Err(error),
    })
}

/// The bytes of `task`'s record, which must still parse to the record that was rendered:
/// the journal holds these bytes and `from_projection` checks them against the list.
fn rendered_bytes(ctx: &MigrateCtx, task: &Task) -> Result<String> {
    let (read, text) = ctx.source.project.read_task_with_raw(&task.id)?;
    if read != *task {
        return Err(Error::Validation(format!(
            "{} changed while the list was being projected; nothing was written; rerun",
            task.id
        )));
    }
    Ok(text)
}

/// A dropped or closed entry, rendered (docs/project-operations.md "Dropped and closed tasks").
pub struct Closed {
    pub id: TaskId,
    pub decision: Decision,
    /// The reason's text as the note records it.
    pub reason: String,
    /// The record's bytes as read.
    pub source_text: String,
    /// The record as the run writes it.
    pub closed_text: String,
    /// The open dependents this entry lets proceed (carried ones by their v1 ids).
    pub released: Vec<TaskId>,
    /// For a close: the dependencies still open once the run is complete.
    pub open_depends: Vec<TaskId>,
    pub source_park: Option<Park>,
    pub source_escalation: Option<Escalation>,
}

/// Everything a valid list's run writes and reports, rendered at one time.
pub struct Projection {
    /// Lowercase hex sha256 of the list, without `sha256:`.
    pub list_sha: String,
    /// The one time every record of the run is stamped with.
    // Kept for the journal-less recovery of the plan's Task 6.
    #[allow(dead_code)]
    pub now: String,
    pub carried: Vec<Carried>,
    pub closed: Vec<Closed>,
    /// Each carried v1 id to its new id.
    pub id_map: BTreeMap<TaskId, TaskId>,
    /// Each carried v1 id to the records outside the carried set that depend on it.
    pub inbound: BTreeMap<TaskId, Vec<TaskId>>,
    /// Each dropped or closed id to the shelves whose `wake_on` names it.
    pub woken: BTreeMap<TaskId, Vec<TaskId>>,
    /// Registered projects that could not be read for dependents.
    pub unread_projects: Vec<String>,
    /// The open successor tasks carried records are placed under.
    pub parent_targets: Vec<TaskId>,
}

/// `text` with every new id of `map` written `new:<v1 id>`: ids not assigned yet (a fresh
/// verdict, or a refused list) are named by the task they stand for. Only whole ids are
/// replaced, never a part of a longer word.
pub fn shown(map: &BTreeMap<TaskId, TaskId>, text: &str) -> String {
    let mut text = text.to_string();
    for (from, new) in map {
        let new = new.to_string();
        let mut out = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(at) = rest.find(&new) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + new.len()..].chars().next();
            let joined = |c: Option<char>| {
                c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            };
            out.push_str(&rest[..at]);
            if joined(before) || joined(after) {
                out.push_str(&new);
            } else {
                out.push_str(&format!("new:{from}"));
            }
            rest = &rest[at + new.len()..];
        }
        out.push_str(rest);
        text = out;
    }
    text
}

/// A reason a one-line note cannot hold; check_list refuses it as `bad_key`.
pub fn multi_line(reason: &str) -> bool {
    reason.contains(['\n', '\r'])
}

/// The text a `reason` stands for: a `[reasons]` key's text, else the literal.
pub fn reason_text<'a>(list: &'a DecisionList, reason: &'a str) -> &'a str {
    list.reasons.get(reason).map_or(reason, String::as_str)
}

/// The record `drop` (`Decision::Drop`) or `done` (`Decision::Close`) would write for
/// `task` at `now`, with `note` as its one stamped note. Applies the status effects only;
/// the caller checks. A carry is a caller error.
pub(crate) fn render_closing(
    task: &Task,
    decision: Decision,
    now: &str,
    model: Option<String>,
    owner: &str,
    note: &str,
    warnings: &mut Vec<String>,
) -> Result<Task> {
    let to = match decision {
        Decision::Drop => Status::Dropped,
        Decision::Close => Status::Done,
        Decision::Carry => {
            return Err(Error::Validation(format!(
                "{} is carried, not dropped or closed",
                task.id
            )));
        }
    };
    let mut closed = task.clone();
    commands::apply_status_effects(&mut closed, to, now, model)?;
    closed.updated = now.to_string();
    commands::append_stamped_note_to(warnings, &mut closed, owner, note)?;
    Ok(closed)
}

/// The checks `save` and `write_task` make on a record `drop` or `done` writes in `project`.
fn validate_closing(
    project: &crate::repo::Project,
    registry: &crate::registry::Registry,
    task: &Task,
) -> Result<()> {
    crate::format::validate_task(task)?;
    crate::review::validate(task)?;
    crate::defer::validate(task)?;
    project.validate_docs(task)?;
    crate::hierarchy::validate_parent(project, registry, task)?;
    crate::hierarchy::validate_lanes(project, registry, task)?;
    crate::hierarchy::validate_periodic(project, registry, task)?;
    crate::hierarchy::validate_defer(project, registry, task)
}

fn list_ids(ids: &[TaskId]) -> String {
    if ids.is_empty() {
        "none".to_string()
    } else {
        ids.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Renders every dropped and closed entry of a list that passed `check_list`, with the
/// docs/project-operations.md names: `release_mismatch`, `close_status`, `close_outputs`, and
/// `closed_invalid` for a record `drop`/`done` would not write. `done`'s refusals over open
/// descendants and open dependencies are not applied (docs/project-operations.md). Writes nothing.
// The joined `explain` reads the dependents once for both projections; this is the
// standalone entry point.
#[allow(dead_code)]
pub fn project_closed(
    ctx: &mut MigrateCtx,
    list: &LoadedList,
    carried_ids: &BTreeMap<TaskId, TaskId>,
    now: &str,
) -> Result<(Vec<Closed>, Vec<RefusalOut>)> {
    let wide = release::read(&ctx.registry, &ctx.source.project.root)?;
    ctx.source.warnings.extend(wide.warnings.iter().cloned());
    project_closed_in(ctx, list, carried_ids, now, &wide)
}

fn project_closed_in(
    ctx: &mut MigrateCtx,
    list: &LoadedList,
    carried_ids: &BTreeMap<TaskId, TaskId>,
    now: &str,
    wide: &Wide,
) -> Result<(Vec<Closed>, Vec<RefusalOut>)> {
    let suffix = format!(" list: sha256:{}", list.sha256);
    let succ = ctx.target.prefix.clone();
    let eligible: BTreeMap<TaskId, Task> = super::draft::eligible(ctx)?
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    let mut decided: BTreeMap<TaskId, (Decision, &Entry)> = BTreeMap::new();
    for entry in &list.list.tasks {
        let named = TaskId::parse(entry.id.trim())
            .map(|id| ctx.registry.canonical_id(&id))
            .ok()
            .filter(|id| eligible.contains_key(id));
        let (Some(id), Some(decision)) = (named, Decision::parse(&entry.decision)) else {
            return Err(Error::Validation(format!(
                "entry {:?} did not pass the list checks; run `tasks migrate <list> --explain`",
                entry.id
            )));
        };
        if decided.insert(id.clone(), (decision, entry)).is_some() {
            return Err(Error::Validation(format!(
                "{id} is listed twice; run `tasks migrate <list> --explain`"
            )));
        }
    }
    let closing: BTreeSet<TaskId> = decided
        .iter()
        .filter(|(_, (d, _))| *d != Decision::Carry)
        .map(|(id, _)| id.clone())
        .collect();
    // Dependents the list settles itself, or whose own entry is refused, are not released.
    let unsettled: BTreeSet<TaskId> = eligible
        .keys()
        .filter(|id| !decided.contains_key(*id))
        .cloned()
        .collect();
    let dependents = release::open_dependents(&wide.tasks, &ctx.registry, &closing);

    let owner = commands::owner_name(&ctx.source.project)?;
    let store = ClaimStore::load(&ctx.source.project.prefix)?;
    let resolver = Resolver::new(&ctx.source.project, &ctx.registry, &ctx.source.host);
    let mut closed = Vec::new();
    let mut refusals = Vec::new();
    for entry in &list.list.tasks {
        let id = ctx
            .registry
            .canonical_id(&TaskId::parse(entry.id.trim()).expect("checked above"));
        let (decision, _) = decided[&id];
        if decision == Decision::Carry {
            continue;
        }
        let task = &eligible[&id];
        // A close on a recurrence is check_list's `close_recurring`; nothing more is said.
        if decision == Decision::Close && task.every.is_some() {
            continue;
        }
        let verb = if decision == Decision::Drop {
            "drop"
        } else {
            "close"
        };

        // Status: `done`'s rule for a close; a drop of a recurrence whose occurrence is done
        // is the explicit coverage exception, which ends the cadence (docs/project-operations.md).
        let to = if decision == Decision::Drop {
            Status::Dropped
        } else {
            Status::Done
        };
        let done_recurrence = task.status == Status::Done && task.every.is_some();
        if !Status::can_transition(task.status, to)
            && !(decision == Decision::Drop && done_recurrence)
        {
            refusals.push(refuse(
                &id,
                &format!("{verb}_status"),
                format!(
                    "{id} is {}, which `tasks {}` refuses; carry it instead",
                    task.status.as_str(),
                    if decision == Decision::Drop {
                        "drop"
                    } else {
                        "done"
                    }
                ),
            ));
            continue;
        }
        if decision == Decision::Close {
            match crate::refs::require_outputs(&resolver, &ctx.source.project, task) {
                Ok(()) => {}
                Err(
                    error @ (Error::RefOutputMissing(_)
                    | Error::AttachmentUnsafe(_)
                    | Error::SpecialFile(_)),
                ) => refusals.push(refuse(
                    &id,
                    "close_outputs",
                    format!("{error}; or drop {id} instead"),
                )),
                Err(error) => return Err(error),
            }
        }

        // Release: exactly the open dependents, carried ones included only for a close.
        let released: Vec<TaskId> = dependents
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|dep| !closing.contains(*dep) && !unsettled.contains(*dep))
            .filter(|dep| decision == Decision::Close || !carried_ids.contains_key(*dep))
            .cloned()
            .collect();
        let mut named = Vec::new();
        let mut unknown = Vec::new();
        for raw in &entry.release {
            match TaskId::parse(raw.trim()) {
                Ok(dep) => named.push(ctx.registry.canonical_id(&dep)),
                Err(_) => unknown.push(format!("{raw:?}")),
            }
        }
        let named_set: BTreeSet<&TaskId> = named.iter().collect();
        let missing: Vec<String> = released
            .iter()
            .filter(|dep| !named_set.contains(dep))
            .map(ToString::to_string)
            .collect();
        let extra: Vec<String> = named_set
            .iter()
            .filter(|dep| !released.contains(dep))
            .map(ToString::to_string)
            .chain(unknown)
            .collect();
        let repeated = named_set.len() != named.len();
        if !missing.is_empty() || !extra.is_empty() || repeated {
            let mut why = Vec::new();
            if !missing.is_empty() {
                why.push(format!("not listed: {}", missing.join(", ")));
            }
            if !extra.is_empty() {
                why.push(format!("not open dependents of {id}: {}", extra.join(", ")));
            }
            if repeated {
                why.push("an id is listed twice".to_string());
            }
            let exact = released
                .iter()
                .map(|dep| format!("\"{dep}\""))
                .collect::<Vec<_>>()
                .join(", ");
            refusals.push(refuse(
                &id,
                "release_mismatch",
                format!(
                    "this {verb} lets {} proceed ({}); set `release = [{exact}]` to say so, or carry {id}",
                    list_ids(&released),
                    why.join("; ")
                ),
            ));
        }

        // A close's dependencies still open once the run is complete: those the list does
        // not close, carried ones (open in the successor) included.
        let mut open_depends = Vec::new();
        if decision == Decision::Close {
            for dep in &task.depends {
                let dep = ctx.registry.canonical_id(dep);
                if closing.contains(&dep) || open_depends.contains(&dep) {
                    continue;
                }
                let open = carried_ids.contains_key(&dep) || {
                    let found = crate::dependency::resolve(&ctx.registry, &dep, |id| {
                        resolver.resolve_task(id)
                    })?;
                    crate::dependency::verdict(found.as_ref())
                        != crate::dependency::Verdict::Satisfied
                };
                if open {
                    open_depends.push(dep);
                }
            }
        }

        let reason = reason_text(&list.list, entry.reason.as_deref().unwrap_or("").trim());
        // check_list's `bad_key`: no one-line note can be rendered from it.
        if multi_line(reason) {
            continue;
        }
        let mut note = format!(
            "{} in migration to {succ}: {reason}",
            if decision == Decision::Drop {
                "dropped"
            } else {
                "closed"
            }
        );
        if !released.is_empty() {
            note.push_str(&format!("; released: {}", list_ids(&released)));
        }
        if !open_depends.is_empty() {
            note.push_str(&format!("; still open: {}", list_ids(&open_depends)));
        }
        note.push_str(&suffix);
        let model = if decision == Decision::Close {
            commands::completion_model()?
        } else {
            None
        };
        let record = render_closing(
            task,
            decision,
            now,
            model,
            &owner,
            &note,
            &mut ctx.source.warnings,
        )?;
        match validate_closing(&ctx.source.project, &ctx.registry, &record) {
            Ok(()) => {}
            Err(error) if refusable(&error) => refusals.push(refuse(
                &id,
                "closed_invalid",
                format!("`tasks {verb}` would not write {id}: {error}"),
            )),
            Err(error) => return Err(error),
        }
        let source_text = rendered_bytes(ctx, task)?;
        let closed_text =
            crate::format::serialize_physical(&ctx.source.project.physical_for_save(&record)?);
        closed.push(Closed {
            id: id.clone(),
            decision,
            reason: reason.to_string(),
            source_text,
            closed_text,
            released,
            open_depends,
            source_park: store.park(&id).cloned(),
            source_escalation: store.escalation(&id).cloned(),
        });
    }
    Ok((closed, refusals))
}

/// The entries the projections can read: those naming an eligible task once, with a
/// decision. check_list has refused every other entry, and their tasks are left unsettled.
fn projectable(ctx: &MigrateCtx, list: &LoadedList) -> Result<LoadedList> {
    let eligible: BTreeSet<TaskId> = super::draft::eligible(ctx)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let named = |entry: &Entry| {
        TaskId::parse(entry.id.trim())
            .map(|id| ctx.registry.canonical_id(&id))
            .ok()
            .filter(|id| eligible.contains(id))
    };
    let mut count: BTreeMap<TaskId, usize> = BTreeMap::new();
    for id in list.list.tasks.iter().filter_map(named) {
        *count.entry(id).or_default() += 1;
    }
    let mut kept = list.list.clone();
    kept.tasks.retain(|entry| {
        named(entry).is_some_and(|id| count[&id] == 1) && Decision::parse(&entry.decision).is_some()
    });
    Ok(LoadedList {
        list: kept,
        bytes: list.bytes.clone(),
        sha256: list.sha256.clone(),
    })
}

/// The list checks and both projections, every refusal together (docs/project-operations.md: `--explain`
/// reports all failures). A header mismatch says the list is for another run, so nothing
/// is projected. The projection is `None` only then; refusal `settle` texts name unassigned
/// ids as `new:<v1 id>`.
pub fn explain(
    ctx: &mut MigrateCtx,
    list: &LoadedList,
) -> Result<(Option<Projection>, Vec<RefusalOut>)> {
    let mut refusals = super::preflight::check_list(ctx, list)?;
    if refusals.iter().any(|r| r.rule == "header_mismatch") {
        return Ok((None, refusals));
    }
    let list = projectable(ctx, list)?;
    let now = render_time(ctx)?;
    let (carried, id_map, more) = project_carried(ctx, &list, &now)?;
    refusals.extend(more);
    let wide = release::read(&ctx.registry, &ctx.source.project.root)?;
    ctx.source.warnings.extend(wide.warnings.iter().cloned());
    let (closed, more) = project_closed_in(ctx, &list, &id_map, &now, &wide)?;
    refusals.extend(more);
    for refusal in &mut refusals {
        refusal.settle = shown(&id_map, &refusal.settle);
    }

    let carried_set: BTreeSet<TaskId> = id_map.keys().cloned().collect();
    let inbound = release::dependents(&wide.tasks, &ctx.registry, &carried_set)
        .into_iter()
        .filter_map(|(id, deps)| {
            let outside: Vec<TaskId> = deps
                .into_iter()
                .filter(|dep| !carried_set.contains(dep))
                .collect();
            (!outside.is_empty()).then_some((id, outside))
        })
        .collect();
    // Every task the list drops or closes, whether or not its own entry was refused.
    let closing: BTreeSet<TaskId> = list
        .list
        .tasks
        .iter()
        .filter(|entry| Decision::parse(&entry.decision) != Some(Decision::Carry))
        .filter_map(|entry| TaskId::parse(entry.id.trim()).ok())
        .map(|id| ctx.registry.canonical_id(&id))
        .collect();
    let woken = release::woken_shelves(&wide.tasks, &ctx.registry, &closing)
        .into_iter()
        .filter_map(|(id, shelves)| {
            let open: Vec<TaskId> = shelves
                .into_iter()
                .filter(|shelf| !closing.contains(shelf))
                .collect();
            (!open.is_empty()).then_some((id, open))
        })
        .collect();
    let new_ids: BTreeSet<&TaskId> = id_map.values().collect();
    let parent_targets: Vec<TaskId> = carried
        .iter()
        .filter_map(|c| c.pair.target.parent.clone())
        .filter(|parent| !new_ids.contains(parent))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok((
        Some(Projection {
            list_sha: list.sha256.clone(),
            now,
            carried,
            closed,
            id_map,
            inbound,
            woken,
            unread_projects: wide.unread_projects,
            parent_targets,
        }),
        refusals,
    ))
}

/// `explain` for a run: the projection of a list with no refusal, or every refusal.
pub fn preflight(
    ctx: &mut MigrateCtx,
    list: &LoadedList,
) -> Result<std::result::Result<Projection, Vec<RefusalOut>>> {
    Ok(match explain(ctx, list)? {
        (Some(projection), refusals) if refusals.is_empty() => Ok(projection),
        (_, refusals) => Err(refusals),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-02T12:00:00Z";

    fn record(extra: &str) -> Task {
        crate::history::fixture(
            &format!(
                "---\nid: forge-000001\ntitle: T\n{extra}priority: 2\n\
                 created: 2026-09-09T00:00:00Z\nupdated: 2026-09-09T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
            "tasks/forge-000001.md",
        )
        .unwrap()
    }

    /// Renders, then reads the written text back and runs the record-level write checks.
    fn render(task: &Task, decision: Decision) -> (Task, String) {
        let mut warnings = Vec::new();
        let rendered = render_closing(
            task,
            decision,
            NOW,
            Some("m-1".into()),
            "owner",
            "dropped in migration to forge2: gone list: sha256:00",
            &mut warnings,
        )
        .unwrap();
        let text = crate::history::fixture_text(&rendered);
        let read = crate::history::fixture(&text, "tasks/forge-000001.md").unwrap();
        crate::format::validate_task(&read).unwrap();
        crate::review::validate(&read).unwrap();
        crate::defer::validate(&read).unwrap();
        (read, text)
    }

    #[test]
    fn a_dropped_reviewed_idea_loses_its_review() {
        let idea = record(
            "status: idea\nreviewed: true\nreviewed_at: 2026-10-17T09:00:00Z\nawaiting_user: true\n",
        );
        assert!(idea.reviewed);
        let (read, text) = render(&idea, Decision::Drop);
        assert_eq!(read.status, Status::Dropped);
        assert!(!read.reviewed && read.reviewed_at.is_none() && !read.awaiting_user);
        assert!(
            !text.contains("reviewed") && !text.contains("awaiting_user"),
            "{text}"
        );
        assert_eq!(read.closed.as_deref(), Some(NOW));
        assert_eq!(read.updated, NOW);
        assert_eq!(read.completed, None);
        assert_eq!(read.model, None, "a drop records no completion model");
        let note = read.notes.last().unwrap();
        assert_eq!(note.by, "owner");
        assert_eq!(
            note.text,
            "dropped in migration to forge2: gone list: sha256:00"
        );
    }

    #[test]
    fn a_dropped_shelf_spends_its_deferral_and_wake_on() {
        let shelf = record("status: shelved\ndefer: 2099-01-01\nwake_on: [ops-000002]\n");
        assert!(shelf.defer.is_some() && !shelf.wake_on.is_empty());
        let (read, text) = render(&shelf, Decision::Drop);
        assert_eq!(read.status, Status::Dropped);
        assert!(read.defer.is_none() && read.wake_on.is_empty());
        assert!(
            !text.contains("defer") && !text.contains("wake_on"),
            "{text}"
        );
    }

    #[test]
    fn a_dropped_done_recurrence_keeps_its_cadence_as_history() {
        let recurrence = record(
            "status: done\nevery: 7d\ncompleted: 2026-10-30T00:00:00Z\nclosed: 2026-10-30T00:00:00Z\nlast_done: 2026-10-30T00:00:00Z\n",
        );
        let (read, text) = render(&recurrence, Decision::Drop);
        assert_eq!(read.status, Status::Dropped);
        assert_eq!(read.every.map(|e| e.to_string()).as_deref(), Some("7d"));
        assert_eq!(read.last_done.as_deref(), Some("2026-10-30T00:00:00Z"));
        assert_eq!(read.closed.as_deref(), Some(NOW));
        assert!(text.contains("every: 7d"), "{text}");
    }

    #[test]
    fn a_close_completes_as_done_does() {
        let (read, _) = render(&record("status: doing\n"), Decision::Close);
        assert_eq!(read.status, Status::Done);
        assert_eq!(read.completed.as_deref(), Some(NOW));
        assert_eq!(read.closed.as_deref(), Some(NOW));
        assert_eq!(read.model.as_deref(), Some("m-1"));
    }

    #[test]
    fn a_carry_is_not_rendered_as_closed() {
        let error = render_closing(
            &record("status: todo\n"),
            Decision::Carry,
            NOW,
            None,
            "owner",
            "x",
            &mut Vec::new(),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("carried"), "{error}");
    }

    #[test]
    fn shown_replaces_whole_new_ids_only() {
        let map: BTreeMap<TaskId, TaskId> = [(
            TaskId::parse("forge-000001").unwrap(),
            TaskId::parse("fx-12ab34").unwrap(),
        )]
        .into_iter()
        .collect();
        assert_eq!(
            shown(
                &map,
                "fx-12ab34 under afx-12ab34, fx-12ab345 and (fx-12ab34)"
            ),
            "new:forge-000001 under afx-12ab34, fx-12ab345 and (new:forge-000001)"
        );
    }
}
