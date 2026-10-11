use super::Ctx;
use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::output::{CheckOut, Finding, Output};
use crate::query::find_cycle;
use crate::resolve::{
    DocKind, DocRead, Located, Resolver, UnavailableCause, has_heading, step_headings,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

fn external_check_order(tasks: &[Task]) -> Result<Vec<String>> {
    let mut tiers = BTreeMap::<String, u8>::new();
    let mut add = |reference: &str, tier: u8| -> Result<()> {
        if crate::format::external_doc(reference)?.is_some() {
            tiers
                .entry(reference.into())
                .and_modify(|current| *current = (*current).min(tier))
                .or_insert(tier);
        }
        Ok(())
    };
    for task in tasks {
        if let Some(plan) = &task.plan {
            add(
                plan,
                if !task.status.is_open() {
                    3
                } else if task.step.is_some() {
                    0
                } else {
                    1
                },
            )?;
        }
        if task.status.is_open()
            && let Some(spec) = &task.spec
        {
            add(spec, 2)?;
        }
    }
    let mut ordered: Vec<_> = tiers.into_iter().collect();
    ordered.sort_by(|(a_ref, a_tier), (b_ref, b_tier)| (a_tier, a_ref).cmp(&(b_tier, b_ref)));
    Ok(ordered
        .into_iter()
        .map(|(reference, _)| reference)
        .collect())
}

enum Counterpart {
    Present(Box<Task>),
    Missing,
    Unreachable,
}

fn finding(task: Option<&Task>, file: String, kind: &str, detail: String) -> Finding {
    Finding {
        id: task.map(|task| task.id.to_string()),
        file,
        kind: kind.into(),
        detail,
    }
}

/// Records one cycle finding, deduped by its member set (order-independent) through
/// `seen`, at the cycle's lowest member.
fn record_cycle(
    cycle: &[TaskId],
    kind: &str,
    seen: &mut BTreeSet<Vec<String>>,
    errors: &mut Vec<Finding>,
) {
    let mut key: Vec<String> = cycle[..cycle.len() - 1]
        .iter()
        .map(ToString::to_string)
        .collect();
    key.sort();
    if seen.insert(key) {
        let lowest = cycle[..cycle.len() - 1].iter().min().unwrap();
        let path: Vec<String> = cycle.iter().map(ToString::to_string).collect();
        errors.push(Finding {
            id: Some(lowest.to_string()),
            file: format!("tasks/{lowest}.md"),
            kind: kind.into(),
            detail: path.join(" -> "),
        });
    }
}

pub fn run(ctx: Ctx) -> Result<Output> {
    let pending: Vec<_> = crate::moving::journal::Journal::pending()?
        .into_iter()
        .filter(|journal| journal.names_prefix(ctx.registry.canonical_prefix(&ctx.project.prefix)))
        .collect();
    let (tasks, parse_errors) = ctx.project.scan_lenient();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for journal in &pending {
        warnings.push(finding(
            None,
            format!(
                "tasks/{}.md",
                if journal.source_id().prefix == ctx.project.prefix {
                    &journal.source
                } else {
                    &journal.target
                }
            ),
            "move_pending",
            journal.pending_error().to_string(),
        ));
    }

    for error in parse_errors {
        let file = match &error {
            Error::Parse { file, .. } => file.clone(),
            _ => "tasks/".into(),
        };
        errors.push(finding(None, file, "parse", error.to_string()));
    }

    if let Some(checkout) = &ctx.project.checkout
        && !checkout.is_dir()
    {
        warnings.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "checkout_missing",
            format!(
                "[checkout] path names {}, which is not on this machine",
                checkout.display()
            ),
        ));
    }

    let mut resolver = Resolver::new(&ctx.project, &ctx.registry);
    resolver.set_fetch_budget(crate::doc_fetch::OPTIONAL_BUDGET);
    let unchecked = resolver.prefetch_external(&external_check_order(&tasks)?)?;
    if !unchecked.is_empty() {
        warnings.push(finding(
            None,
            "tasks/".into(),
            "doc_unchecked",
            format!(
                "external fetch budget exhausted; unchecked references: {}",
                unchecked.join(", ")
            ),
        ));
    }
    let linked_plans: BTreeSet<_> = tasks.iter().filter_map(|task| task.plan.as_ref()).collect();
    let mut plan_bodies = BTreeMap::new();
    for plan in linked_plans {
        if (crate::format::external_doc(plan)?.is_some()
            || resolver.locate(plan)?.into_path().is_some())
            && let DocRead::Available(doc) = resolver.read(DocKind::Plan, plan)?
        {
            plan_bodies.insert(plan.clone(), doc.body);
        }
    }
    let foreign = |id: &TaskId| -> std::result::Result<Option<Task>, String> {
        match resolver.resolve_task(id) {
            Ok(task) => Ok(task),
            Err(Error::Parse { file, detail }) => Err(format!("{file}: {detail}")),
            Err(error) => Err(error.to_string()),
        }
    };

    let record = |id: &TaskId| -> Result<Option<Task>> {
        let id = ctx.registry.canonical_id(id);
        if id.prefix == ctx.project.prefix {
            Ok(tasks
                .iter()
                .find(|task| ctx.registry.canonical_id(&task.id) == id)
                .cloned())
        } else {
            resolver.resolve_task(&id)
        }
    };
    let counterpart = |id: &TaskId| -> Result<Counterpart> {
        let id = ctx.registry.canonical_id(id);
        if id.prefix == ctx.project.prefix {
            return Ok(match record(&id)? {
                Some(task) => Counterpart::Present(Box::new(task)),
                None => Counterpart::Missing,
            });
        }
        let Some(root) = ctx.registry.project_root(&id.prefix) else {
            return Ok(Counterpart::Unreachable);
        };
        if !crate::scope::is_reachable(root)? {
            return Ok(Counterpart::Unreachable);
        }
        Ok(match foreign(&id) {
            Ok(Some(task)) => Counterpart::Present(Box::new(task)),
            Ok(None) => Counterpart::Missing,
            Err(_) => Counterpart::Unreachable,
        })
    };
    let same =
        |a: &TaskId, b: &TaskId| ctx.registry.canonical_id(a) == ctx.registry.canonical_id(b);

    for task in &tasks {
        let file = format!("tasks/{}.md", task.id);
        // A retired project holds no open work (ops lifecycle spec §4); one that reappears,
        // say from a sync conflict, is an error to close or a reason to unretire.
        if ctx.project.retired.is_some() && task.status.is_open() {
            errors.push(finding(
                Some(task),
                file.clone(),
                "open_in_retired",
                format!(
                    "{} is {} in a retired project; close it, or run `tasks unretire`",
                    task.id,
                    task.status.as_str()
                ),
            ));
        }
        // Only a project that keeps a dictionary is held to it, and only its open work:
        // a closed task's tags are history, not vocabulary.
        if let Some(dictionary) = &ctx.project.tags
            && task.status.is_open()
        {
            for tag in task.tags.iter().filter(|tag| {
                !dictionary.contains_key(*tag)
                    && !(ctx.project.feedback.is_some()
                        && crate::commands::feedback::writes_tag(task, tag))
            }) {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "undefined_tag",
                    format!(
                        "tag {tag:?} has no entry in {}'s [tags]",
                        crate::repo::CONFIG_REL
                    ),
                ));
            }
        }
        // Lanes-needs spec §4.2: reads carry an undeclared need as-is; `check` refuses
        // it on every record, open or closed, since a reopen or an acquire reads it again.
        for need in &task.needs {
            if let Err(error) =
                crate::needs::require_declared(&ctx.project.needs, std::slice::from_ref(need))
            {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "unknown_need",
                    error.to_string(),
                ));
            }
        }
        if let Some(completed) = &task.completed
            && task.status != Status::Done
        {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "completed_stamp_on_open_task",
                format!(
                    "completed {completed} but status is {}; only a hand edit outside tasks leaves this",
                    task.status.as_str()
                ),
            ));
        }
        for dependency in &task.depends {
            let dependency_id = ctx.registry.canonical_id(dependency);
            if &dependency_id != dependency {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "retired_prefix",
                    format!(
                        "depends on {dependency} through retired prefix {:?}; it is now {dependency_id}",
                        dependency.prefix
                    ),
                ));
            }
            if task.status.is_open()
                && task.status != Status::Shelved
                && let Ok(Some(dependency_task)) = foreign(&dependency_id)
                && dependency_task.status == Status::Shelved
            {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "shelved_dep",
                    format!("depends on shelved {dependency}: unshelve it or drop the dependency"),
                ));
            }
            if let Ok(Some(dependency_task)) = record(dependency)
                && dependency_task.status == Status::Moved
                && let Some(to) = dependency_task.moved_to.clone()
            {
                let detail = match crate::tombstone::follow(Some(dependency_task), &record) {
                    Ok(Some(end)) => format!(
                        "depends on {dependency}, which moved to {}; run `tasks dep {} --on {} --rm {dependency}`",
                        end.id, task.id, end.id
                    ),
                    Ok(None) | Err(_) => format!(
                        "depends on {dependency}, which moved to {to}; its chain cannot be followed to a live record here"
                    ),
                };
                warnings.push(finding(Some(task), file.clone(), "moved_dep", detail));
            }
            if dependency_id.prefix == ctx.project.prefix {
                if !ctx.project.task_path(&dependency_id).try_exists()? {
                    errors.push(finding(
                        Some(task),
                        file.clone(),
                        "dangling_dep",
                        format!("depends on missing {dependency}"),
                    ));
                }
            } else {
                match foreign(dependency) {
                    Ok(Some(_)) => {}
                    Ok(None) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "unreachable_dep",
                        format!("depends on unreachable {dependency}"),
                    )),
                    Err(detail) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "foreign_unparsable",
                        format!("dependency {dependency} cannot be read: {detail}"),
                    )),
                }
            }
        }
        if let Some(parent) = &task.parent {
            let parent_id = ctx.registry.canonical_id(parent);
            if parent_id.prefix != ctx.project.prefix {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "foreign_parent",
                    format!("parent {parent} is not in this project"),
                ));
            } else if !ctx.project.task_path(&parent_id).try_exists()? {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "dangling_parent",
                    format!("parent {parent} does not exist"),
                ));
            }
        }
        if let Some(parent) = &task.parent
            && task.status.is_open()
            && task.every.is_none()
            && let Some(parent_task) = tasks
                .iter()
                .find(|candidate| candidate.id == ctx.registry.canonical_id(parent))
            && !parent_task.status.is_open()
        {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "open_child_of_closed_parent",
                format!(
                    "open under {} which is {}",
                    parent,
                    parent_task.status.as_str()
                ),
            ));
        }
        for (pointer, is_source) in [(&task.moved_to, true), (&task.moved_from, false)] {
            let Some(pointer) = pointer else { continue };
            let direction = if is_source { "to" } else { "from" };
            match counterpart(pointer)? {
                Counterpart::Present(other)
                    if if is_source {
                        other.moved_from.as_ref().is_some_and(|from| same(from, &task.id))
                    } else {
                        other.status == Status::Moved
                            && other.moved_to.as_ref().is_some_and(|to| same(to, &task.id))
                    } => {}
                Counterpart::Unreachable => warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "move_counterpart_unreachable",
                    format!(
                        "{} moved {direction} {pointer}, whose project {} is not reachable here; dependencies on it stay blocked",
                        task.id,
                        ctx.registry.canonical_id(pointer).prefix
                    ),
                )),
                state => {
                    let (source, target) = if is_source { (&task.id, pointer) } else { (pointer, &task.id) };
                    if pending.iter().any(|journal| {
                        same(&journal.source_id(), source) && same(&journal.target_id(), target)
                    }) {
                        continue;
                    }
                    let problem = match state {
                        Counterpart::Missing => "is missing".to_string(),
                        Counterpart::Present(_) if is_source => {
                            format!("does not name {} as moved_from", task.id)
                        }
                        Counterpart::Present(_) => {
                            format!("is not a tombstone naming {} as moved_to", task.id)
                        }
                        Counterpart::Unreachable => unreachable!(),
                    };
                    errors.push(finding(
                        Some(task),
                        file.clone(),
                        "move_incomplete",
                        format!(
                            "{} moved {direction} {pointer}, but {pointer} {problem}; finish the move on the host that started it",
                            task.id
                        ),
                    ));
                }
            }
        }
        // Spec §6: the plan step is the unit of delegation, and rating it is the
        // planner's duty. Elsewhere a missing rating is not a finding, like size.
        if task.status.is_open() && task.step.is_some() && task.complexity.is_none() {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "unrated_step",
                "plan step without a complexity rating".into(),
            ));
        }
        if task.status == Status::Doing && task.process.is_none() {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "process_missing",
                "doing task without a process decision".into(),
            ));
        }
        if task.every.is_some() {
            let kids = crate::hierarchy::children(&tasks, &task.id, &ctx.registry);
            if crate::hierarchy::is_goal(task, !kids.is_empty()) {
                let detail = if kids.is_empty() {
                    "is a lane and has a cadence; a goal is never ready, so the cadence can \
                     never fire"
                        .to_string()
                } else {
                    format!(
                        "has a cadence and children ({}); a goal is never ready, so the cadence can never fire",
                        kids.iter()
                            .map(|kid| kid.id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                errors.push(finding(Some(task), file.clone(), "periodic_goal", detail));
            }
        }
        if task.defer.is_some() {
            let kids = crate::hierarchy::children(&tasks, &task.id, &ctx.registry);
            if crate::hierarchy::is_goal(task, !kids.is_empty()) {
                let detail = if kids.is_empty() {
                    "is a lane and is deferred; a goal is never ready, so the deferral hides \
                     nothing"
                        .to_string()
                } else {
                    format!(
                        "is deferred and has children ({}); a goal is never ready, so the deferral hides nothing",
                        kids.iter()
                            .map(|kid| kid.id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                errors.push(finding(Some(task), file.clone(), "deferred_goal", detail));
            }
            // spec §3.2: the status rule lives here and in the writers, not in parsing.
            if task.status != Status::Moved && !crate::defer::can_carry(task.status) {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "defer_status",
                    format!(
                        "is {} and carries defer; a deferral may sit only on an idea, todo, or blocked task",
                        task.status.as_str()
                    ),
                ));
            }
        }
        // Review-state spec §3.2: the writers refuse it, so only a hand edit lands here.
        if task.reviewed && task.status != Status::Idea {
            errors.push(finding(
                Some(task),
                file.clone(),
                "review_status",
                format!(
                    "is {} and carries reviewed; only an idea carries it",
                    task.status.as_str()
                ),
            ));
        }
        // A merge or a hand edit can leave a lane inside a lane.
        if task.lane
            && let Some(outer) = crate::hierarchy::enclosing_lane(&tasks, task, &ctx.registry)
        {
            errors.push(finding(
                Some(task),
                file.clone(),
                "nested_lane",
                format!(
                    "is a lane inside lane {}; a sub-effort inside a lane is an ordinary \
                     child goal",
                    outer.id
                ),
            ));
        }
        // A lane is often filed before its steps exist, so a childless one is a reminder,
        // not an error.
        if task.lane
            && crate::hierarchy::is_active(task)
            && crate::hierarchy::children(&tasks, &task.id, &ctx.registry).is_empty()
        {
            warnings.push(finding(
                Some(task),
                file.clone(),
                "childless_lane",
                format!(
                    "is a lane with no steps yet; add them with `tasks add \"<step>\" --parent {}`",
                    task.id
                ),
            ));
        }
        // Spec §7: link drift is held against open work only. A closed record's links are
        // history, and a later plan revision that merges its heading away is not an error.
        if task.status.is_open() {
            for (kind, path) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
                let Some(path) = path else { continue };
                if crate::format::external_doc(path)?.is_some() {
                    if let DocRead::Unavailable { reference, cause } = resolver.read(kind, path)?
                        && !matches!(cause, UnavailableCause::BudgetExhausted)
                    {
                        warnings.push(finding(
                            Some(task),
                            file.clone(),
                            "doc_unreachable",
                            format!("{} {reference}: {}", kind.name(), cause.detail()),
                        ));
                    }
                    continue;
                }
                match resolver.locate(path)? {
                    Located::Here(_) => {}
                    Located::Main { root, .. } => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "doc_in_main_checkout",
                        format!(
                            "{} {path} is not in this checkout; read from the main checkout at {}",
                            kind.name(),
                            root.display()
                        ),
                    )),
                    Located::Missing => errors.push(finding(
                        Some(task),
                        file.clone(),
                        "doc_missing",
                        format!("{} {path} does not exist", kind.name()),
                    )),
                }
            }
            if let (Some(plan), Some(step)) = (&task.plan, &task.step)
                && let Some(body) = plan_bodies.get(plan)
                && !has_heading(body, step)
            {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "step_missing",
                    format!("heading {step:?} not found in {plan}"),
                ));
            }
        }
    }

    let mut seen = BTreeSet::new();
    let unreachable = RefCell::new(BTreeSet::new());
    let edges = |id: &TaskId| -> Result<Option<Vec<TaskId>>> {
        let id = ctx.registry.canonical_id(id);
        if let Some(task) = tasks.iter().find(|task| task.id == id) {
            return Ok(Some(
                crate::tombstone::edges(task)
                    .iter()
                    .map(|dependency| ctx.registry.canonical_id(dependency))
                    .collect(),
            ));
        }
        match foreign(&id) {
            Ok(Some(task)) => Ok(Some(
                crate::tombstone::edges(&task)
                    .iter()
                    .map(|dependency| ctx.registry.canonical_id(dependency))
                    .collect(),
            )),
            Ok(None) | Err(_) => {
                unreachable.borrow_mut().insert(id);
                Ok(Some(vec![]))
            }
        }
    };
    for task in &tasks {
        unreachable.borrow_mut().clear();
        match find_cycle(&task.id, &edges) {
            Ok(None) => {}
            Ok(Some(cycle)) => record_cycle(&cycle, "cycle", &mut seen, &mut errors),
            Err(error) => return Err(error),
        }
        for id in std::mem::take(&mut *unreachable.borrow_mut()) {
            warnings.push(finding(
                Some(task),
                format!("tasks/{}.md", task.id),
                "cycle_unverifiable",
                format!("cannot verify acyclicity through unreachable {id}"),
            ));
        }
    }

    let mut seen_parent_cycles = BTreeSet::new();
    for task in &tasks {
        if let Some(cycle) = crate::hierarchy::parent_cycle(&tasks, &task.id, &ctx.registry) {
            record_cycle(&cycle, "parent_cycle", &mut seen_parent_cycles, &mut errors);
        }
    }

    for (plan, text) in &plan_bodies {
        for heading in step_headings(text) {
            let linked = tasks.iter().any(|task| {
                task.plan.as_ref() == Some(plan) && task.step.as_deref() == Some(&heading)
            });
            if !linked {
                warnings.push(finding(
                    None,
                    plan.clone(),
                    "unlinked_step",
                    format!("heading {heading:?} has no task"),
                ));
            }
        }
    }

    for problem in crate::attachments::audit(&ctx.project, &tasks)? {
        let finding = Finding {
            id: problem.id.map(|id| id.to_string()),
            file: problem.file,
            kind: problem.kind.into(),
            detail: problem.detail,
        };
        match problem.severity {
            crate::attachments::Severity::Error => errors.push(finding),
            crate::attachments::Severity::Warning => warnings.push(finding),
        }
    }

    Ok(Output::Check(CheckOut { errors, warnings }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(plan: Option<&str>, spec: Option<&str>, step: Option<&str>, status: &str) -> Task {
        let mut text = format!(
            "---\nid: xx-000001\ntitle: Test\nstatus: {status}\npriority: 2\ncreated: 2026-10-11T00:00:00Z\nupdated: 2026-10-11T00:00:00Z\ndepends: []\ntags: []\n"
        );
        for (name, value) in [("plan", plan), ("spec", spec), ("step", step)] {
            if let Some(value) = value {
                text.push_str(&format!(
                    "{name}: {}\n",
                    serde_json::to_string(value).unwrap()
                ));
            }
        }
        text.push_str("---\n");
        crate::format::parse_task(&text, "fixture").unwrap()
    }

    #[test]
    fn external_check_order_prioritizes_stepped_plans_and_shared_references() {
        let mut tasks = vec![
            task(None, Some("stub:a"), None, "todo"),
            task(Some("stub:c"), None, None, "done"),
            task(Some("stub:b"), None, None, "todo"),
            task(Some("stub:z"), None, Some("Task 1: x"), "todo"),
            task(None, Some("stub:z"), None, "todo"),
            task(None, Some("stub:ignored"), None, "done"),
        ];
        let expected = ["stub:z", "stub:b", "stub:a", "stub:c"];
        assert_eq!(external_check_order(&tasks).unwrap(), expected);
        tasks.reverse();
        assert_eq!(external_check_order(&tasks).unwrap(), expected);
    }
}
