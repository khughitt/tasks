use super::{open_ctx, reject_retired};
use crate::claims::MutationLock;
use crate::error::{Error, Result};
use crate::migrating::classify::{MigrationRecovery, Phase, classify_with_journal};
use crate::migrating::journal::MigrationJournal;
use crate::migrating::list::{Decision, DecisionList, LoadedList};
use crate::migrating::{MigrateCtx, apply, draft, list, preflight, project};
use crate::model::TaskId;
use crate::output::{
    BodyChange, CarriedOut, ClosedOut, DroppedOut, InboundOut, MigrateOut, Output, RefusalOut,
    WokenOut,
};
use crate::registry::Registry;
use crate::rename::root_identity;
use std::path::{Path, PathBuf};

/// The source is the checkout the command runs in, the successor a registered project:
/// the checks `move` makes, then neither may be retired.
pub fn open(dir: Option<&Path>, to: &str) -> Result<MigrateCtx> {
    let source = open_ctx(dir)?;
    let prefix = source
        .registry
        .canonical_prefix(&source.project.prefix)
        .to_string();
    let (target, registry) = super::move_::open_destination(
        &source,
        &prefix,
        to,
        |registered, _| {
            format!(
                "{prefix} is migrated from its registered checkout, which foreign readers and check read; run `tasks -C {} migrate ...` there",
                crate::stale::quote(&registered.display().to_string())
            )
        },
    )?;
    reject_retired(&registry, &source.project)?;
    reject_retired(&registry, &target)?;
    Ok(MigrateCtx {
        source,
        target,
        registry,
    })
}

pub fn run(
    dir: Option<&Path>,
    list_path: Option<PathBuf>,
    to: Option<String>,
    draft_path: Option<PathBuf>,
    explain: bool,
) -> Result<Output> {
    if let Some(path) = draft_path {
        let to = to.ok_or_else(|| Error::Validation("--draft needs --to".into()))?;
        let ctx = open(dir, &to)?;
        return Ok(Output::MigrateDraft(draft::write(&ctx, &path)?));
    }
    let path =
        list_path.ok_or_else(|| Error::Validation("migrate needs a list or --draft".into()))?;
    let loaded = list::load(&path)?;
    if !explain {
        return apply_list(dir, &path, &loaded);
    }
    let mut ctx = open(dir, &loaded.list.successor)?;
    if let Some(journal) = MigrationJournal::load(&ctx.source.project.prefix)? {
        return Ok(Output::Migrate(explain_pending(&ctx, &journal, &loaded)?));
    }
    let (projection, refusals) = project::explain(&mut ctx, &loaded)?;
    let mut out = MigrateOut {
        source: ctx.source.project.prefix.clone(),
        successor: ctx.target.prefix.clone(),
        list: format!("sha256:{}", loaded.sha256),
        recovery: if refusals.is_empty() {
            "fresh"
        } else {
            "refuse"
        }
        .into(),
        carried: Vec::new(),
        dropped: Vec::new(),
        closed: Vec::new(),
        inbound: Vec::new(),
        woken: Vec::new(),
        unread_projects: Vec::new(),
        paths: Default::default(),
        warnings: Vec::new(),
        refusals,
        body_changes: Vec::new(),
    };
    if let Some(projection) = projection {
        fill(&mut out, &projection, &loaded.list, &ctx);
    }
    out.warnings = ctx.source.warnings.clone();
    out.warnings
        .extend(preflight::reason_warnings(&loaded.list));
    Ok(Output::Migrate(out))
}

/// `--explain` while this source has a pending run: the journal's report with the
/// verdict, `resume` with the new ids filled, or `refuse` with the reason. Takes no locks.
fn explain_pending(
    ctx: &MigrateCtx,
    journal: &MigrationJournal,
    loaded: &LoadedList,
) -> Result<MigrateOut> {
    let verdict = (|| {
        reject_other_operations(ctx, Some(journal))?;
        apply::check_identity(ctx, journal)?;
        Ok(classify_with_journal(
            journal,
            &apply::snapshot(ctx, journal)?,
            loaded,
        ))
    })();
    let reason = match verdict {
        Ok(MigrationRecovery::Resume(_)) => None,
        Ok(MigrationRecovery::Refuse(reason)) => Some(reason),
        Ok(other) => unreachable!("a journal never classifies as {other:?}"),
        Err(Error::MovePending(reason) | Error::Validation(reason)) => Some(reason),
        Err(error) => return Err(error),
    };
    let mut out = apply::output(
        ctx,
        journal,
        if reason.is_some() { "refuse" } else { "resume" },
    )?;
    if let Some(reason) = reason {
        // Spec §4.7: new ids are shown only for a `resume` verdict.
        for row in &mut out.carried {
            row.id = None;
        }
        out.refusals.push(RefusalOut {
            id: journal.source.clone(),
            rule: "pending_operation".into(),
            settle: reason,
        });
    }
    out.warnings
        .extend(preflight::reason_warnings(&loaded.list));
    Ok(out)
}

/// A pending rename, move or other migration naming either project; `own` is this
/// source's journal, which is the run being continued.
fn reject_other_operations(ctx: &MigrateCtx, own: Option<&MigrationJournal>) -> Result<()> {
    for project in [&ctx.source.project, &ctx.target] {
        super::reject_pending_rename(project)?;
        for journal in crate::moving::journal::Journal::pending()? {
            if journal.names_prefix(&project.prefix) {
                return Err(journal.pending_error());
            }
        }
        if let Some(journal) = crate::migrating::journal::pending_naming(
            &project.prefix,
            own.map(|journal| journal.source.as_str()),
        )? {
            return Err(journal.pending_error());
        }
    }
    Ok(())
}

fn identity(ctx: &MigrateCtx) -> Result<(String, String, PathBuf, PathBuf)> {
    Ok((
        ctx.source.project.prefix.clone(),
        ctx.target.prefix.clone(),
        root_identity(&ctx.source.project.root)?,
        root_identity(&ctx.target.root)?,
    ))
}

/// The apply: `move`'s locks (both prefixes in sorted order, then the registry), with the
/// projects re-opened under them and the same bounded retry.
fn apply_list(dir: Option<&Path>, path: &Path, loaded: &LoadedList) -> Result<Output> {
    let mut requested = open(dir, &loaded.list.successor)?;
    for _ in 0..4 {
        let mut prefixes = [
            requested.source.project.prefix.clone(),
            requested.target.prefix.clone(),
        ];
        prefixes.sort();
        let _first = MutationLock::acquire(&prefixes[0])?;
        let _second = MutationLock::acquire(&prefixes[1])?;
        let _registry = Registry::lock()?;
        let mut refreshed = open(dir, &loaded.list.successor)?;
        if identity(&refreshed)? == identity(&requested)? {
            let mut out = decide(&mut refreshed, path, loaded)?;
            out.warnings
                .extend(preflight::reason_warnings(&loaded.list));
            return Ok(Output::Migrate(out));
        }
        requested = refreshed;
    }
    Err(Error::Io(
        "the projects' identities kept changing while acquiring migration locks; retry".into(),
    ))
}

/// Resume a pending run of this source, or preflight the list and start one.
fn decide(ctx: &mut MigrateCtx, path: &Path, loaded: &LoadedList) -> Result<MigrateOut> {
    let journal = MigrationJournal::load(&ctx.source.project.prefix)?;
    reject_other_operations(ctx, journal.as_ref())?;
    if let Some(journal) = journal {
        apply::check_identity(ctx, &journal)?;
        return match classify_with_journal(&journal, &apply::snapshot(ctx, &journal)?, loaded) {
            MigrationRecovery::Resume(phase) => apply::run(ctx, journal, phase),
            MigrationRecovery::Refuse(reason) => Err(Error::MovePending(reason)),
            other => unreachable!("a journal never classifies as {other:?}"),
        };
    }
    match project::preflight(ctx, loaded)? {
        Ok(projection) => {
            let journal = MigrationJournal::from_projection(ctx, &projection, loaded, path)?;
            apply::run(ctx, journal, Phase::Journal)
        }
        Err(refusals) => Err(Error::Validation(format!(
            "the decision list does not pass its checks; nothing was written: {}; run `tasks migrate {} --explain` for the review",
            refusals
                .iter()
                .map(|r| format!("{} [{}]: {}", r.id, r.rule, r.settle))
                .collect::<Vec<_>>()
                .join("; "),
            crate::stale::quote(&path.display().to_string())
        ))),
    }
}

/// The rows of a projection. A fresh verdict assigns no ids, so a new id is shown as
/// `new:<v1 id>`.
fn fill(
    out: &mut MigrateOut,
    projection: &project::Projection,
    list: &DecisionList,
    ctx: &MigrateCtx,
) {
    let shown = |text: &str| project::shown(&projection.id_map, text);
    for item in &projection.carried {
        let target = &item.pair.target;
        if let Some(diff) = &item.body_change {
            out.body_changes.push(BodyChange {
                id: item.from.to_string(),
                diff: diff.clone(),
            });
        }
        out.carried.push(CarriedOut {
            from: item.from.to_string(),
            id: None,
            parent: target.parent.as_ref().map(|p| shown(&p.to_string())),
            rewrites: item.rewrites.iter().map(|r| shown(r)).collect(),
            attachments: item
                .pair
                .attachments
                .iter()
                .map(|a| a.name.clone())
                .collect(),
            park: item.pair.source_park.is_some(),
        });
    }
    let reasons: std::collections::BTreeMap<TaskId, &str> = list
        .tasks
        .iter()
        .filter_map(|entry| {
            let id = TaskId::parse(entry.id.trim()).ok()?;
            Some((
                ctx.registry.canonical_id(&id),
                project::reason_text(list, entry.reason.as_deref()?.trim()),
            ))
        })
        .collect();
    let ids = |ids: &[TaskId]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    for item in &projection.closed {
        let reason = reasons
            .get(&item.id)
            .copied()
            .unwrap_or_default()
            .to_string();
        match item.decision {
            Decision::Drop => out.dropped.push(DroppedOut {
                id: item.id.to_string(),
                reason,
                released: ids(&item.released),
            }),
            Decision::Close => out.closed.push(ClosedOut {
                id: item.id.to_string(),
                reason,
                open_depends: ids(&item.open_depends),
                released: ids(&item.released),
            }),
            Decision::Carry => unreachable!("a carried entry is never rendered as closed"),
        }
    }
    out.inbound = projection
        .inbound
        .iter()
        .map(|(id, deps)| InboundOut {
            id: id.to_string(),
            dependents: ids(deps),
        })
        .collect();
    out.woken = projection
        .woken
        .iter()
        .map(|(id, shelves)| WokenOut {
            id: id.to_string(),
            shelves: ids(shelves),
        })
        .collect();
    out.unread_projects = projection.unread_projects.clone();
}
