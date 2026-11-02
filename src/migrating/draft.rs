//! `migrate --draft`: the form the decision list is filled in from.

use super::MigrateCtx;
use crate::claims::ClaimSnapshot;
use crate::error::{Error, Result};
use crate::model::{Relation, Task, TaskId};
use crate::output::DraftOut;
use crate::refs::Citation;
use crate::resolve::{DocKind, LocatedEntry, Resolver};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

/// Every open record of the source plus every recurrence whose current occurrence is
/// `done` (a recurrence is live work that comes due again), in goal-tree order: a goal
/// before its children, roots and siblings by priority, then id.
pub fn eligible(ctx: &MigrateCtx) -> Result<Vec<Task>> {
    let mut tasks: Vec<Task> = ctx
        .source
        .project
        .scan()?
        .into_iter()
        .filter(|t| {
            t.status.is_open() || (t.every.is_some() && t.status == crate::model::Status::Done)
        })
        .collect();
    tasks.sort_by_key(|t| (t.priority, t.id.to_string()));
    let present: Vec<String> = tasks.iter().map(|t| t.id.to_string()).collect();
    let parent_of = |t: &Task| {
        t.parent
            .as_ref()
            .map(|p| ctx.registry.canonical_id(p).to_string())
            .filter(|p| present.contains(p))
    };
    let mut children: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut roots = Vec::new();
    for (at, task) in tasks.iter().enumerate() {
        match parent_of(task) {
            Some(parent) => children.entry(parent).or_default().push(at),
            None => roots.push(at),
        }
    }
    let mut order = Vec::new();
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(at) = stack.pop() {
        order.push(at);
        if let Some(kids) = children.get(&tasks[at].id.to_string()) {
            stack.extend(kids.iter().rev());
        }
    }
    if order.len() != tasks.len() {
        return Err(Error::Validation(
            "the source's parent links form a cycle among open tasks; run `tasks check`".into(),
        ));
    }
    let mut slots: Vec<Option<Task>> = tasks.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .map(|at| slots[at].take().unwrap())
        .collect())
}

pub fn write(ctx: &MigrateCtx, path: &Path) -> Result<DraftOut> {
    let tasks = eligible(ctx)?;
    let source = &ctx.source.project;
    let (scope, mut warnings) =
        crate::scope::Scope::open_all_with_retired(&ctx.registry, &source.root)?;
    let everything: BTreeMap<String, Task> = scope
        .scan()?
        .into_iter()
        .map(|t| (t.id.to_string(), t))
        .collect();
    let claims = ClaimSnapshot::load(std::iter::once(source.prefix.as_str()))?;
    let resolver = Resolver::new(&ctx.target, &ctx.registry);

    let mut doc = DocumentMut::new();
    doc["version"] = value(1);
    doc["source"] = value(source.prefix.as_str());
    doc["successor"] = value(ctx.target.prefix.as_str());
    doc["drafted"] = value(crate::time::now());
    let mut reasons = Table::new();
    reasons
        .decor_mut()
        .set_prefix("\n# Shared reasons, cited by name from a drop or close: name = \"text\"\n");
    doc["reasons"] = Item::Table(reasons);
    let mut entries = ArrayOfTables::new();
    for task in &tasks {
        let bytes = std::fs::read(source.task_path(&task.id))?;
        let mut table = Table::new();
        table["id"] = value(task.id.to_string());
        table["digest"] = value(format!("sha256:{:x}", Sha256::digest(&bytes)));
        table["decision"] = value("");
        let mut comment = String::from("\n");
        for line in context(ctx, task, &tasks, &everything, &claims, &resolver) {
            comment.push_str("# ");
            comment.push_str(&line);
            comment.push('\n');
        }
        table.decor_mut().set_prefix(comment);
        entries.push(table);
    }
    doc["task"] = Item::ArrayOfTables(entries);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => Error::Validation(format!(
                "{} exists; choose a new path (a draft never overwrites a list)",
                path.display()
            )),
            _ => Error::from(e),
        })?;
    file.write_all(doc.to_string().as_bytes())?;
    warnings.extend(ctx.source.warnings.iter().cloned());
    Ok(DraftOut {
        path: path.display().to_string(),
        tasks: tasks.len(),
        warnings,
    })
}

fn describe(ctx: &MigrateCtx, everything: &BTreeMap<String, Task>, id: &TaskId) -> String {
    let id = ctx.registry.canonical_id(id);
    match everything.get(&id.to_string()) {
        Some(t) => format!("{id} ({}, {})", t.status.as_str(), t.title),
        None => format!("{id} (not found)"),
    }
}

/// The lines written above one entry; each is a fact the decision turns on.
fn context(
    ctx: &MigrateCtx,
    task: &Task,
    drafted: &[Task],
    everything: &BTreeMap<String, Task>,
    claims: &ClaimSnapshot,
    resolver: &Resolver,
) -> Vec<String> {
    let mut lines = vec![
        format!("{}  [{}]", task.title, task.status.as_str()),
        format!("priority: {}", task.priority),
    ];
    if let Some(every) = &task.every {
        lines.push(format!("recurs every {every}"));
    }
    if let Some(parent) = &task.parent {
        lines.push(format!("parent: {}", describe(ctx, everything, parent)));
    }
    let kids: Vec<String> = drafted
        .iter()
        .filter(|t| {
            t.parent
                .as_ref()
                .is_some_and(|p| ctx.registry.canonical_id(p) == task.id)
        })
        .map(|t| t.id.to_string())
        .collect();
    if !kids.is_empty() {
        lines.push(format!("children: {}", kids.join(", ")));
    }
    for dep in &task.depends {
        lines.push(format!("depends: {}", describe(ctx, everything, dep)));
    }
    let dependents: Vec<String> = everything
        .values()
        .filter(|t| t.id.prefix != task.id.prefix && t.status.is_open())
        .filter(|t| {
            t.depends
                .iter()
                .any(|d| ctx.registry.canonical_id(d) == task.id)
        })
        .map(|t| format!("{} ({})", t.id, t.title))
        .collect();
    for dependent in dependents {
        lines.push(format!("open dependent in another project: {dependent}"));
    }
    if let Some(claim) = claims.held(&task.id) {
        lines.push(format!("LIVE CLAIM by {}", claim.owner));
    }
    if let Some(park) = claims.park(&task.id) {
        lines.push(format!(
            "parked by {} ({}): {}",
            park.owner,
            park.waiting_on.as_str(),
            park.next_step
        ));
    }
    lines.extend(unresolved(ctx, task, resolver));
    lines
}

/// Spec, plan, step and local reference entries that do not resolve in the successor,
/// with any `[ref:<handle>]` citation of them in the body.
fn unresolved(ctx: &MigrateCtx, task: &Task, resolver: &Resolver) -> Vec<String> {
    let mut lines = Vec::new();
    let project = &ctx.target.prefix;
    for (kind, path) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
        if let Some(path) = path
            && !matches!(crate::format::external_doc(path), Ok(Some(_)))
            && let Err(error) = resolver.resolve_doc(kind, path)
        {
            lines.push(format!(
                "{} {path} does not resolve in {project}: {error}",
                kind.name()
            ));
        }
    }
    if let (Some(plan), Some(step)) = (&task.plan, &task.step)
        && !matches!(resolver.step_exists(plan, step), Ok(true))
    {
        lines.push(format!("step {step:?} of {plan} not found in {project}"));
    }
    let cited: Vec<&str> = crate::refs::citations(&task.body)
        .into_iter()
        .filter_map(|c| match c {
            Citation::Handle(h) => Some(h),
            Citation::Malformed(_) => None,
        })
        .collect();
    for entry in &task.refs {
        if entry.relation == Relation::Output {
            continue;
        }
        let Ok(crate::refs::Location::Path(path)) = crate::refs::location(&entry.reference) else {
            continue;
        };
        let missing = matches!(resolver.locate_entry(path), Ok(LocatedEntry::Missing));
        if missing {
            let cite = if cited.contains(&entry.handle.as_str()) {
                format!(" (cited as [ref:{}])", entry.handle)
            } else {
                String::new()
            };
            lines.push(format!(
                "ref {} {} does not exist in {project}{cite}",
                entry.handle, entry.reference
            ));
        }
    }
    lines
}
