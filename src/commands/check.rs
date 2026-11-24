use super::Ctx;
use crate::error::{Error, Result};
use crate::model::{Relation, Status, Task, TaskId};
use crate::output::{CheckOut, Finding, Output};
use crate::query::find_cycle;
use crate::refs::{Location, Presence};
use crate::resolve::{
    DocKind, DocRead, Located, LocatedEntry, Resolver, UnavailableCause, has_heading, step_headings,
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

fn lifecycle_findings(
    ctx: &Ctx,
    tasks: &[Task],
    errors: &mut Vec<Finding>,
    warnings: &mut Vec<Finding>,
) -> Result<()> {
    let mut error = |kind: &str, detail: String| {
        errors.push(finding(None, crate::repo::CONFIG_REL.into(), kind, detail));
    };
    if let Some(successor) = ctx
        .project
        .retired
        .as_ref()
        .and_then(|r| r.successor.as_ref())
    {
        match crate::scope::open_registered(&ctx.registry, successor, crate::scope::Origin::Prefix)
        {
            Ok(project) if project.retired.is_some() => error(
                "retired_successor_retired",
                format!("successor {successor:?} is retired"),
            ),
            Ok(project) if project.prefix != ctx.project.prefix => {}
            Ok(_) => error(
                "retired_successor_invalid",
                "a retired project's successor cannot be itself".into(),
            ),
            Err(reason) => error(
                "retired_successor_invalid",
                format!("successor {successor:?} cannot be opened: {reason}"),
            ),
        }
    }
    if let Err(reason) = crate::scope::check_predecessor_pairing(&ctx.registry, &ctx.project) {
        error("predecessor_registry_mismatch", reason.to_string());
    }
    let Some(pred) = &ctx.project.predecessor else {
        return Ok(());
    };
    let collisions: Vec<_> = tasks
        .iter()
        .filter(|t| pred.suffixes.contains(&t.id.hex))
        .map(|t| t.id.to_string())
        .collect();
    if !collisions.is_empty() {
        error(
            "predecessor_suffix_collision",
            format!(
                "local tasks use suffixes reserved by {:?}: {}",
                pred.prefix,
                collisions.join(", ")
            ),
        );
    }
    if !crate::scope::is_reachable(&ctx.registry, &pred.prefix)? {
        warnings.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "predecessor_unreachable",
            format!(
                "predecessor {:?} is not reachable on this host; frozen suffixes remain reserved",
                pred.prefix
            ),
        ));
        return Ok(());
    }
    let predecessor =
        crate::scope::open_registered(&ctx.registry, &pred.prefix, crate::scope::Origin::Prefix)?;
    if predecessor
        .retired
        .as_ref()
        .and_then(|r| r.successor.as_ref())
        .map(|s| ctx.registry.canonical_prefix(s))
        != Some(ctx.project.prefix.as_str())
    {
        error(
            "predecessor_not_retired_to_successor",
            format!(
                "predecessor {:?} must be retired to {:?}",
                pred.prefix, ctx.project.prefix
            ),
        );
    }
    let suffixes: BTreeSet<_> = predecessor.scan()?.into_iter().map(|t| t.id.hex).collect();
    if suffixes != pred.suffixes {
        error(
            "predecessor_suffix_mismatch",
            format!(
                "frozen suffixes differ from predecessor {:?}'s task inventory",
                pred.prefix
            ),
        );
    }
    Ok(())
}

/// `check --staged`: audit the project's records as the index Git will publish them.
///
/// The index is the one Git hands a pre-commit hook in `GIT_INDEX_FILE`, else the
/// repository's own. The project is opened from the staged config at its real records
/// path, so relative paths, registry comparisons and Git reads are unchanged; records
/// content is read from a private copy of the staged tree. An ordinary appended note on a
/// tree this host already accepted takes the note route (`crate::staged::note_route`);
/// every other commit gets the full audit. A passing check records the staged tree.
pub fn run_staged(mut ctx: Ctx) -> Result<Output> {
    use crate::staged::{content::Content, note_route, snapshot, validated};
    let index = snapshot::index_file(&ctx.project.records, std::env::var_os(snapshot::INDEX_ENV))?;
    let snap = snapshot::read(&ctx.project.records, &index)?;
    let config = snapshot::tree_entry(&snap.top, &snap.tree, crate::repo::CONFIG_REL)?
        .filter(|entry| matches!(entry.mode.as_str(), "100644" | "100755"))
        .ok_or_else(|| {
            Error::Config(format!(
                "the staged records hold no regular {}",
                crate::repo::CONFIG_REL
            ))
        })?;
    let text = String::from_utf8(snapshot::blob(&snap.top, &config.oid)?).map_err(|_| {
        Error::Config(format!(
            "the staged {} is not UTF-8",
            crate::repo::CONFIG_REL
        ))
    })?;
    let mut project = crate::repo::Project::from_config(ctx.project.records.clone(), &text)?;
    if project.prefix != ctx.project.prefix {
        return Err(Error::Config(format!(
            "the staged {} names prefix {:?} but the working copy names {:?}; stage or \
             restore the config so they agree",
            crate::repo::CONFIG_REL,
            project.prefix,
            ctx.project.prefix
        )));
    }
    project.worktree = ctx.project.worktree.clone();
    let slot = validated::Slot::open(&snap.common, &project.records, &project.prefix)?;
    let accepted = validated::Passed {
        head: snap.head.as_ref().map(|head| head.commit.clone()),
        head_tree: snap.head.as_ref().and_then(|head| head.tree.clone()),
        tree: snap.tree.clone(),
    };
    let guard = crate::staged::guard::check_index(&project, &snap)?;
    if guard.errors.is_empty()
        && let Ok(qualified) = note_route::qualify(&project, &snap, &slot.passed()?)?
        && note_route::check(&project, &qualified).is_ok()
    {
        slot.record(accepted)?;
        return Ok(Output::Check(CheckOut {
            growth: None,
            errors: Vec::new(),
            warnings: guard.untouched_future_warnings,
        }));
    }
    let content = Content::materialize(&snap.top, &snap.tree)?;
    project.content = Some(content.root().to_path_buf());
    ctx.project = project;
    // Findings name the records directory, not the private copy they were read from.
    let copy = content.root().display().to_string();
    let records = snap.top.join(&snap.subdir);
    let records = records.display().to_string();
    let out = run(ctx, false).map_err(|error| {
        if error.to_string().contains(&copy) {
            error.with_suffix(&format!(
                " (paths under {copy} name the staged copy of {records})"
            ))
        } else {
            error
        }
    })?;
    let Output::Check(mut out) = out else {
        unreachable!("check returns a check report")
    };
    for finding in out.errors.iter_mut().chain(out.warnings.iter_mut()) {
        finding.file = finding.file.replace(&copy, &records);
        finding.detail = finding.detail.replace(&copy, &records);
    }
    for err in guard.errors {
        if !out
            .errors
            .iter()
            .any(|existing| existing.file == err.file && existing.kind == err.kind)
        {
            out.errors.push(err);
        }
    }
    if out.errors.is_empty() {
        slot.record(accepted)?;
    }
    Ok(Output::Check(out))
}

enum EffectiveStage {
    Declared(crate::repo::ProjectStage),
    Missing,
    /// This copy declares none and the registered copy did not open; the
    /// `retirement_unknown` finding already reports that open.
    Unknown,
}

/// The stage this copy declares, else its registered copy's when it is another copy (a
/// worktree branch cut before the declaration), as `where` reports policy and
/// `effective_retirement` reads retirement. An invalid value in this copy is an error
/// here; an invalid registered value is the registered checkout's own finding.
fn effective_stage(ctx: &Ctx) -> Result<EffectiveStage> {
    if let Some(stage) = ctx.project.declared_stage()? {
        return Ok(EffectiveStage::Declared(stage));
    }
    match crate::scope::config_dir(&ctx.registry, &ctx.project.prefix) {
        Some(dir) if dir != ctx.project.records => {
            let registered = crate::scope::open_registered(
                &ctx.registry,
                &ctx.project.prefix,
                crate::scope::Origin::Prefix,
            );
            Ok(match registered.map(|project| project.declared_stage()) {
                Ok(Ok(Some(stage))) => EffectiveStage::Declared(stage),
                Ok(Ok(None)) => EffectiveStage::Missing,
                Ok(Err(_)) | Err(_) => EffectiveStage::Unknown,
            })
        }
        _ => Ok(EffectiveStage::Missing),
    }
}

/// The stage this project declares, and what it shares with the other catalogued entries:
/// one stage per Git repository among registered active projects, and the words that name
/// it (docs/configuration.md). A collision is a warning: it is a fact about two entries,
/// and an error would block every commit of a project that did not introduce it.
fn catalogue_findings(
    ctx: &Ctx,
    retired: bool,
    errors: &mut Vec<Finding>,
    warnings: &mut Vec<Finding>,
) -> Result<()> {
    let config = crate::repo::CONFIG_REL;
    let catalogue = crate::catalogue::discovery::Catalogue::build(&ctx.registry);
    match effective_stage(ctx) {
        Err(error) => errors.push(finding(
            None,
            config.into(),
            "stage_invalid",
            error.to_string(),
        )),
        Ok(EffectiveStage::Missing) if !retired => errors.push(finding(
            None,
            config.into(),
            "stage_missing",
            format!(
                "project {:?} declares no stage; set stage = \"experimental\", \"pre-release\" \
                 or \"supported\" in its tracker config",
                ctx.project.prefix
            ),
        )),
        Ok(EffectiveStage::Missing | EffectiveStage::Unknown) => {}
        Ok(EffectiveStage::Declared(stage)) if !retired => {
            use crate::catalogue::stages::{conflicts, name};
            for conflict in conflicts(&ctx.registry, &catalogue, &ctx.project, stage)? {
                errors.push(finding(
                    None,
                    config.into(),
                    "stage_conflict",
                    format!(
                        "{} and {} share the Git repository {} but declare different stages \
                         ({} and {}); one Git root means one shared test policy, so declare \
                         one stage",
                        ctx.project.prefix,
                        conflict.prefix,
                        conflict.repository.display(),
                        name(stage),
                        name(conflict.stage)
                    ),
                ));
            }
        }
        Ok(EffectiveStage::Declared(_)) => {}
    }
    let live = ctx.registry.canonical_prefix(&ctx.project.prefix);
    let others = catalogue
        .entries
        .iter()
        .filter_map(|(prefix, state)| match state {
            crate::catalogue::discovery::EntryState::Read(project) if prefix != live => {
                Some(project.as_ref())
            }
            _ => None,
        });
    for collision in crate::catalogue::words::collisions(
        &ctx.registry,
        others.chain(std::iter::once(&ctx.project)),
    ) {
        if collision.involves(&ctx.project.prefix) {
            warnings.push(finding(
                None,
                config.into(),
                "word_collision",
                collision.detail(),
            ));
        }
    }
    Ok(())
}

pub fn run(ctx: Ctx, growth: bool) -> Result<Output> {
    // A code tree whose index still holds the config is on the pre-move side of the
    // migration: the walk missed it only because the working tree lost it.
    if ctx.project.kind == crate::repo::Kind::Detached
        && let Some(top) = &ctx.project.worktree
        && crate::link::config_in_index(top)?
    {
        return Err(crate::link::records_moved(
            &ctx.project.prefix,
            &ctx.project.records,
        ));
    }
    let pending: Vec<_> = crate::moving::journal::Journal::pending()?
        .into_iter()
        .filter(|journal| journal.names_prefix(ctx.registry.canonical_prefix(&ctx.project.prefix)))
        .collect();
    let migrations: Vec<_> = crate::migrating::journal::MigrationJournal::pending()?
        .into_iter()
        .filter(|journal| journal.names_prefix(ctx.registry.canonical_prefix(&ctx.project.prefix)))
        .collect();
    let snapshot = if ctx.project.content.is_none() {
        Some(crate::attachments::transaction::read_guard(
            &ctx.project,
            |_| true,
        )?)
    } else {
        None // The staged tree is already an immutable snapshot.
    };
    let audit = ctx.project.scan_audit_with_raw();
    let records = audit.records;
    let unsupported_records = audit.unsupported;
    let parse_errors = audit.errors;
    let mut growth_rows: Vec<_> = records
        .iter()
        .map(|(task, raw)| {
            let bytes = crate::history::archived_bytes(task, |archive| {
                ctx.project.read_archive(&task.id, archive)
            })?;
            Ok(crate::growth::measure(task, raw, bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    growth_rows.sort_by(|left, right| {
        right
            .record_bytes
            .cmp(&left.record_bytes)
            .then_with(|| left.id.cmp(&right.id))
    });
    let tasks: Vec<_> = records.into_iter().map(|(task, _)| task).collect();
    let attachment_problems = crate::attachments::audit(&ctx.project, &tasks)?;
    drop(snapshot);
    let mut errors = Vec::new();
    // A worktree made before the retirement holds no `[retired]` until a merge; the
    // registered checkout's declaration covers it, as it does for writes. One that cannot
    // be read is an error finding, so the rest of the report still runs.
    let retired = match super::effective_retirement(&ctx.registry, &ctx.project) {
        Ok(retirement) => retirement.is_some(),
        Err(error) => {
            errors.push(finding(
                None,
                crate::repo::CONFIG_REL.into(),
                "retirement_unknown",
                error.to_string(),
            ));
            false
        }
    };
    let authority = crate::scope::config_dir(&ctx.registry, &ctx.project.prefix)
        .unwrap_or_else(|| ctx.project.records.clone());
    // A retired project takes no writes, so the record cannot be closed as it stands;
    // a worktree's open copy is a stale branch of work the retirement already closed.
    let way_out = if authority == ctx.project.records {
        "run `tasks unretire`, close it, then retire again".to_string()
    } else {
        format!(
            "this worktree's copy is stale ({} declares the retirement); update or remove \
             this worktree",
            authority.display()
        )
    };
    let mut warnings = Vec::new();
    if ctx.project.color.is_none() {
        warnings.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "color_missing",
            format!("project {:?} has no color; choose a color and set color = \"#RRGGBB\" in its tracker config", ctx.project.prefix),
        ));
    }
    lifecycle_findings(&ctx, &tasks, &mut errors, &mut warnings)?;
    catalogue_findings(&ctx, retired, &mut errors, &mut warnings)?;
    let threshold = ctx.project.growth_warn_bytes();
    for row in &growth_rows {
        if row.record_bytes > threshold {
            warnings.push(Finding {
                id: Some(row.id.clone()),
                file: format!("tasks/{}.md", row.id),
                kind: "record_growth".into(),
                detail: crate::growth::warning_detail(
                    &row.id,
                    row.record_bytes,
                    row.body_bytes,
                    threshold,
                ),
            });
        }
    }
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
    for journal in &migrations {
        warnings.push(finding(
            None,
            "tasks/".into(),
            "move_pending",
            journal.pending_error().to_string(),
        ));
    }

    for future in &unsupported_records {
        warnings.push(Finding {
            id: Some(future.id.to_string()),
            file: future.path.clone(),
            kind: "unsupported_record_version".into(),
            detail: future.warning(),
        });
    }

    for error in parse_errors {
        let file = match &error {
            Error::Parse { file, .. } => file.clone(),
            _ => "tasks/".into(),
        };
        errors.push(finding(None, file, "parse", error.to_string()));
    }

    // In a Git-backed records checkout, plain `tasks check` also enforces the changed-index
    // record guard (today's records pre-commit hook invokes plain `tasks check`).
    if ctx.project.content.is_none()
        && let Some(guard) = crate::staged::guard::check_plain_git_index(&ctx.project)?
    {
        for err in guard.errors {
            if !errors
                .iter()
                .any(|existing| existing.file == err.file && existing.kind == err.kind)
            {
                errors.push(err);
            }
        }
    }

    let checkout = &ctx.project.checkout;
    if ctx.project.kind != crate::repo::Kind::InTree && !checkout.is_dir() {
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
    let root = ctx
        .registry
        .project_root(&ctx.project.prefix)
        .unwrap_or(&ctx.project.root);
    if crate::catalogue::source::freshness(root, ctx.project.identity.as_ref()).0
        == crate::catalogue::source::Freshness::Behind
    {
        warnings.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "identity_behind",
            format!(
                "identity snapshot is behind {}; run `tasks identity refresh {}`",
                root.display(),
                ctx.project.prefix
            ),
        ));
    }

    let mut resolver = Resolver::new(&ctx.project, &ctx.registry, &ctx.host);
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
    // A moved tombstone's plan link is history: the destination owns the live work, so a
    // plan only tombstones link is not audited for heading coverage here, and a moved step
    // never covers a heading of a plan a local record still links.
    let live = |task: &&Task| task.status != Status::Moved;
    let linked_plans: BTreeSet<_> = tasks
        .iter()
        .filter(live)
        .filter_map(|task| task.plan.as_ref())
        .collect();
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
        let canonical = ctx.registry.canonical_id(id);
        if canonical.prefix == ctx.project.prefix {
            Ok(tasks.iter().find(|task| task.id == canonical).cloned())
        } else {
            resolver.resolve_task(id)
        }
    };
    let counterpart = |id: &TaskId| -> Result<Counterpart> {
        let canonical = ctx.registry.canonical_id(id);
        if canonical.prefix == ctx.project.prefix {
            if unsupported_records.iter().any(|u| u.id == canonical) {
                return Ok(Counterpart::Unreachable);
            }
            return Ok(match record(id)? {
                Some(task) => Counterpart::Present(Box::new(task)),
                None => Counterpart::Missing,
            });
        }
        if !crate::scope::is_reachable(&ctx.registry, &canonical.prefix)? {
            return Ok(Counterpart::Unreachable);
        }
        Ok(match foreign(id) {
            Ok(Some(task)) => Counterpart::Present(Box::new(task)),
            Ok(None) => Counterpart::Missing,
            Err(_) => Counterpart::Unreachable,
        })
    };
    let same =
        |a: &TaskId, b: &TaskId| ctx.registry.canonical_id(a) == ctx.registry.canonical_id(b);

    for task in &tasks {
        let file = format!("tasks/{}.md", task.id);
        if task.status.is_open() {
            for problem in crate::refs::audit(task) {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    problem.kind,
                    problem.detail,
                ));
            }
        }
        // A retired project holds no open work (docs/project-operations.md); one that reappears,
        // say from a sync conflict, is an error to close or a reason to unretire.
        if retired && task.status.is_open() {
            errors.push(finding(
                Some(task),
                file.clone(),
                "open_in_retired",
                format!(
                    "{} is {} in a retired project; {way_out}",
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
                !dictionary.contains_key(*tag) && !crate::commands::feedback::writes_tag(task, tag)
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
        // Current contract (docs/user-guide.md): reads carry an undeclared need as-is; `check` refuses
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
                && let Ok(Some(dependency_task)) = foreign(dependency)
                && dependency_task.status == Status::Shelved
            {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "shelved_dep",
                    format!(
                        "depends on shelved {dependency}, keeping this task blocked: replace the shelf with a dependency (`tasks unshelve {dependency}`, then `tasks dep {dependency} --on <awaited>`), unshelve {dependency} or drop the edge (`tasks dep {} --rm {dependency}`) if the wait is no longer intended, or keep both when the shelf records a wake condition no task can stand for",
                        task.id
                    ),
                ));
            }
            // Closed (`done`, `dropped`) and tombstone (`moved`) records' dependencies are
            // history and never evaluated for readiness, so only an open dependent (`idea`,
            // `todo`, `doing`, `blocked`, `shelved`) needs the swap (move spec: Dependency
            // evaluation).
            if task.status.is_open()
                && let Ok(Some(dependency_task)) = record(dependency)
                && dependency_task.status == Status::Moved
                && let Some(to) = dependency_task.moved_to.clone()
            {
                let end = crate::tombstone::follow(Some(dependency_task), &record);
                match end {
                    // A piece that moved into this project is now same-project work, which
                    // belongs under the goal as a child.
                    Ok(Some(end))
                        if crate::pieces::is_piece(&ctx.registry, task, dependency)
                            && ctx.registry.canonical_id(&end.id).prefix
                                == ctx.registry.canonical_id(&task.id).prefix =>
                    {
                        warnings.push(finding(
                            Some(task),
                            file.clone(),
                            "piece_now_local",
                            format!(
                                "piece {dependency} moved to {} in this project; run `tasks dep {} --rm {dependency}`, then `tasks edit {} --parent {}`",
                                end.id, task.id, end.id, task.id
                            ),
                        ));
                    }
                    // A piece keeps its mark on the successor: declare it first, then drop
                    // the tombstone edge, which unmarks the old spelling.
                    Ok(Some(end)) if crate::pieces::is_piece(&ctx.registry, task, dependency) => {
                        warnings.push(finding(
                            Some(task),
                            file.clone(),
                            "moved_dep",
                            format!(
                                "piece {dependency} moved to {}; run `tasks edit {} --piece {}`, then `tasks dep {} --rm {dependency}`",
                                end.id, task.id, end.id, task.id
                            ),
                        ));
                    }
                    Ok(Some(end)) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "moved_dep",
                        format!(
                            "depends on {dependency}, which moved to {}; run `tasks dep {} --on {} --rm {dependency}`",
                            end.id, task.id, end.id
                        ),
                    )),
                    Ok(None) | Err(_) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "moved_dep",
                        format!(
                            "depends on {dependency}, which moved to {to}; its chain cannot be followed to a live record here"
                        ),
                    )),
                }
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
        let mut follows_seen = BTreeSet::new();
        for target in &task.follows {
            let id = ctx.registry.canonical_id(target);
            if id == ctx.registry.canonical_id(&task.id) {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "self_follows",
                    "a task cannot follow itself".into(),
                ));
            }
            if !follows_seen.insert(id.clone()) {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "duplicate_follows",
                    format!("duplicate follows target {target}"),
                ));
            }
            if id.prefix == ctx.project.prefix && !ctx.project.task_path(&id).try_exists()? {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "dangling_follows",
                    format!("follows missing {target}"),
                ));
            }
        }
        if !task.follows.is_empty() && task.status == Status::Moved {
            errors.push(finding(
                Some(task),
                file.clone(),
                "follows_status",
                "a tombstone cannot carry follows".into(),
            ));
        }
        // Current contract (docs/task-model.md, "Goal pieces"): the writers refuse these, so
        // only a hand edit lands here.
        let own = ctx.registry.canonical_id(&task.id);
        let mut pieces_seen = BTreeSet::new();
        for piece in &task.pieces {
            let id = ctx.registry.canonical_id(piece);
            if id == own {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "self_piece",
                    "a task cannot be its own piece".into(),
                ));
            } else if id.prefix == own.prefix {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "local_piece",
                    crate::pieces::local_piece_remedy(&task.id, piece),
                ));
            }
            if !pieces_seen.insert(id.clone()) {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "duplicate_piece",
                    format!("duplicate piece {piece}"),
                ));
            }
            if !task
                .depends
                .iter()
                .any(|dependency| ctx.registry.canonical_id(dependency) == id)
            {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "piece_not_dependency",
                    format!(
                        "piece {piece} is not a dependency; a piece must stay one: run `tasks dep {} --on {piece}`",
                        task.id
                    ),
                ));
            }
        }
        if !task.pieces.is_empty() && task.lane {
            errors.push(finding(
                Some(task),
                file.clone(),
                "lane_pieces",
                format!(
                    "is a lane and declares pieces ({}); a lane holds only local children",
                    crate::pieces::listed(task)
                ),
            ));
        }
        if !task.pieces.is_empty() && task.status == Status::Moved {
            errors.push(finding(
                Some(task),
                file.clone(),
                "piece_status",
                "a tombstone cannot carry pieces".into(),
            ));
        }
        for related in &task.related {
            let related_id = ctx.registry.canonical_id(related);
            if related_id.prefix == ctx.project.prefix
                && !ctx.project.task_path(&related_id).try_exists()?
            {
                warnings.push(finding(
                    Some(task),
                    file.clone(),
                    "dangling_related",
                    format!("relates to missing {related}"),
                ));
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
                    }) || migrations.iter().any(|journal| {
                        journal.carried.iter().any(|entry| {
                            source.to_string() == entry.source && target.to_string() == entry.target
                        })
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
        // Current contract (docs/task-model.md): the plan step is the unit of delegation, and rating it is the
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
                let detail = if !kids.is_empty() {
                    format!(
                        "has a cadence and children ({}); a goal is never ready, so the cadence can never fire",
                        kids.iter()
                            .map(|kid| kid.id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else if !task.pieces.is_empty() {
                    format!(
                        "has a cadence and pieces ({}); a goal is never ready, so the cadence can never fire",
                        crate::pieces::listed(task)
                    )
                } else {
                    "is a lane and has a cadence; a goal is never ready, so the cadence can \
                     never fire"
                        .to_string()
                };
                errors.push(finding(Some(task), file.clone(), "periodic_goal", detail));
            }
        }
        if !task.wake_on.is_empty() && task.status != Status::Shelved {
            errors.push(finding(
                Some(task),
                file.clone(),
                "wake_status",
                "only a shelved task carries wake_on".into(),
            ));
        }
        if !task.related.is_empty() && task.status == Status::Moved {
            errors.push(finding(
                Some(task),
                file.clone(),
                "related_status",
                "a tombstone cannot carry related".into(),
            ));
        }
        if task.defer.is_some() {
            if task.every.is_some() && task.status != Status::Shelved {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "defer_periodic",
                    "only a shelf combines every with a review checkpoint".into(),
                ));
            }
            let kids = crate::hierarchy::children(&tasks, &task.id, &ctx.registry);
            if task.status != Status::Shelved && crate::hierarchy::is_goal(task, !kids.is_empty()) {
                let detail = if !kids.is_empty() {
                    format!(
                        "is deferred and has children ({}); a goal is never ready, so the deferral hides nothing",
                        kids.iter()
                            .map(|kid| kid.id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else if !task.pieces.is_empty() {
                    format!(
                        "is deferred and has pieces ({}); a goal is never ready, so the deferral hides nothing",
                        crate::pieces::listed(task)
                    )
                } else {
                    "is a lane and is deferred; a goal is never ready, so the deferral hides \
                     nothing"
                        .to_string()
                };
                errors.push(finding(Some(task), file.clone(), "deferred_goal", detail));
            }
            // Current contract (docs/task-model.md): the status rule lives here and in the writers, not in parsing.
            if task.status != Status::Moved && !crate::defer::can_carry(task.status) {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "defer_status",
                    format!(
                        "is {} and carries defer; a date may sit only on an idea, todo, blocked, or shelved task",
                        task.status.as_str()
                    ),
                ));
            }
        }
        // Current contract (docs/user-guide.md): the writers refuse these, so only a hand edit lands here.
        for issue in crate::review::metadata_issues(task) {
            let detail = match issue {
                crate::review::MetadataIssue::ReviewedNonIdea(status) => format!(
                    "is {} and carries reviewed; only an idea carries it",
                    status.as_str()
                ),
                crate::review::MetadataIssue::AwaitingUserWithoutReviewed => {
                    "carries awaiting_user without reviewed; only a reviewed idea carries it".into()
                }
                crate::review::MetadataIssue::ReviewedAtWithoutReviewed => {
                    "carries reviewed_at without reviewed".into()
                }
            };
            errors.push(finding(Some(task), file.clone(), "review_status", detail));
        }
        if task.closed.is_some() && !matches!(task.status, Status::Done | Status::Dropped) {
            errors.push(finding(
                Some(task),
                file.clone(),
                "closed_status",
                format!(
                    "is {} and carries closed; only a done or dropped task carries it",
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
        // Current contract (docs/task-model.md): link drift is held against open work only. A closed record's links are
        // history, and a later plan revision that merges its heading away is not an error.
        if task.status.is_open() {
            for entry in &task.refs {
                if matches!(crate::refs::location(&entry.reference)?, Location::External) {
                    continue;
                }
                let presence = crate::refs::present(&resolver, &ctx.project, task, entry)?;
                if entry.relation == Relation::Output && !matches!(&presence, Presence::Special(_))
                {
                    continue;
                }
                let handle = &entry.handle;
                match presence {
                    Presence::Here(_path) => {}
                    Presence::Main { root, path } => warnings.push(finding(
                        Some(task), file.clone(), "ref_in_main_checkout",
                        format!("reference {handle} {} is not in this checkout; read from {} in the main checkout at {}", entry.reference, path.display(), root.display()),
                    )),
                    Presence::Absent(path) => errors.push(finding(
                        Some(task), file.clone(), "ref_missing",
                        format!("reference {handle} {} does not exist (looked for {})", entry.reference, path.display()),
                    )),
                    Presence::Special(path) => errors.push(finding(
                        Some(task), file.clone(), "special_file",
                        format!("reference {handle}: {} is not a regular file or directory", path.display()),
                    )),
                    Presence::Unsafe { path, detail } => errors.push(finding(
                        Some(task), file.clone(), "ref_missing",
                        format!("reference {handle}: {} is not a usable attachment ({detail})", path.display()),
                    )),
                }
            }
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
                    Located::Here(_) | Located::Code(_) => {}
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
                    Located::Missing => match resolver.locate_entry(path)? {
                        LocatedEntry::Special(path) => errors.push(finding(
                            Some(task),
                            file.clone(),
                            "special_file",
                            format!(
                                "{} {} is not a regular file or directory",
                                kind.name(),
                                path.display()
                            ),
                        )),
                        _ => errors.push(finding(
                            Some(task),
                            file.clone(),
                            "doc_missing",
                            format!("{} {path} does not exist", kind.name()),
                        )),
                    },
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
        if let Some(task) = tasks.iter().find(|task| task.id == *id) {
            return Ok(Some(
                crate::tombstone::edges(task)
                    .iter()
                    .map(|dependency| ctx.registry.canonical_id(dependency))
                    .collect(),
            ));
        }
        match resolver.resolve_canonical_task(id) {
            Ok(Some(task)) => Ok(Some(
                crate::tombstone::edges(&task)
                    .iter()
                    .map(|dependency| ctx.registry.canonical_id(dependency))
                    .collect(),
            )),
            Ok(None) | Err(_) => {
                unreachable.borrow_mut().insert(id.clone());
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
            let linked = tasks.iter().filter(live).any(|task| {
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

    if let Err(error) = crate::retained::lineage::check_live(&ctx.project, &tasks) {
        errors.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "retention_lineage",
            error.to_string(),
        ));
    }
    if let Err(error) = crate::retained::recovery::check_live(&ctx.project, &tasks) {
        errors.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "retention_recovery",
            error.to_string(),
        ));
    }
    for problem in attachment_problems {
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

    Ok(Output::Check(CheckOut {
        errors,
        warnings,
        growth: growth.then_some(growth_rows),
    }))
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
        crate::history::fixture(&text, "fixture").unwrap()
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
