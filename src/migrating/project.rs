//! Projecting carried tasks (spec §4.3 "Carried tasks", §4.4 "Carried task"): each carried
//! entry is rendered as `move` renders it, then rewritten (parent, dependencies within the
//! carried set, `wake_on`, dropped links, a replacement body), and only then validated in
//! the destination, over the whole projected set at once.

use super::MigrateCtx;
use super::list::{Decision, Entry, LoadedList};
use crate::commands;
use crate::error::{Error, Result};
use crate::model::{Relation, Task, TaskId};
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
/// spec §4.3 allows, and validates the projected destination. Returns the carried records
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

    // The decisions, keyed by canonical id. check_list has refused anything else.
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
    // New ids are not final until the run: refusals name them by their v1 id.
    let shown = |id: &TaskId| match map.iter().find(|(_, new)| *new == id) {
        Some((from, _)) => format!("new:{from}"),
        None => id.to_string(),
    };

    let owner = commands::owner_name(&ctx.source.project)?;
    let mut refusals = Vec::new();
    let mut works = Vec::new();
    for (id, entry) in &carry {
        let pair = build_pair(
            &mut ctx.source,
            &ctx.target,
            &eligible[id],
            map[id].clone(),
            now,
            &suffix,
        )?;
        works.push(Work {
            entry,
            carried: Carried {
                from: id.clone(),
                pair,
                rewrites: Vec::new(),
                body_change: None,
            },
            parent_cut: false,
            links_kept: Vec::new(),
        });
    }

    let resolver = Resolver::new(&ctx.target, &ctx.registry);
    for work in &mut works {
        let id = work.carried.from.clone();
        let entry = work.entry;
        let canonical = |t: &TaskId| ctx.registry.canonical_id(t);

        // Parent.
        let v1_parent = work.carried.pair.target.parent.as_ref().map(canonical);
        let settle_parent = format!(
            "set `parent` to \"none\", a carried {source_prefix} id, or an open {succ} task id"
        );
        let new_parent: std::result::Result<Option<TaskId>, (&str, String)> =
            match (&entry.parent, &v1_parent) {
                (None, None) => Ok(None),
                (None, Some(p)) => match map.get(p) {
                    Some(new) => Ok(Some(new.clone())),
                    None => Err((
                        "parent_required",
                        format!("its parent {p} is not carried; {settle_parent}"),
                    )),
                },
                (Some(value), _) => parse_parent(ctx, &map, value)?
                    .map_err(|why| ("parent_invalid", format!("{why}; {settle_parent}"))),
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
            Err((rule, settle)) => {
                refusals.push(refuse(&id, rule, settle));
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
            .and_then(|()| validate_destination_docs(&ctx.target, &ctx.registry, view, from))
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
                            cycle.iter().map(shown).collect::<Vec<_>>().join(" -> ")
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
