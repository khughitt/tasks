use super::{open_ctx, reject_retired};
use crate::error::{Error, Result};
use crate::migrating::list::{Decision, DecisionList};
use crate::migrating::{MigrateCtx, draft, list, preflight, project};
use crate::model::TaskId;
use crate::output::{
    BodyChange, CarriedOut, ClosedOut, DroppedOut, InboundOut, MigrateOut, Output, WokenOut,
};
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
        return Err(Error::Validation(
            "applying a decision list is not available yet; run with --explain to validate it"
                .into(),
        ));
    }
    let mut ctx = open(dir, &loaded.list.successor)?;
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
