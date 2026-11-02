use super::{open_ctx, reject_retired};
use crate::error::{Error, Result};
use crate::migrating::{MigrateCtx, draft, list, preflight, project};
use crate::output::{BodyChange, CarriedOut, MigrateOut, Output};
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
    let mut refusals = preflight::check_list(&ctx, &loaded)?;
    let mut carried = Vec::new();
    let mut body_changes = Vec::new();
    if refusals.is_empty() {
        let now = project::render_time(&ctx)?;
        let (projected, map, more) = project::project_carried(&mut ctx, &loaded, &now)?;
        refusals.extend(more);
        // A fresh verdict assigns no ids: a new id is shown as `new:<v1 id>`.
        let shown = |text: &str| {
            map.iter().fold(text.to_string(), |text, (from, new)| {
                text.replace(&new.to_string(), &format!("new:{from}"))
            })
        };
        for item in projected {
            let target = &item.pair.target;
            if let Some(diff) = &item.body_change {
                body_changes.push(BodyChange {
                    id: item.from.to_string(),
                    diff: diff.clone(),
                });
            }
            carried.push(CarriedOut {
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
    }
    let mut warnings = ctx.source.warnings.clone();
    warnings.extend(preflight::reason_warnings(&loaded.list));
    Ok(Output::Migrate(MigrateOut {
        source: ctx.source.project.prefix.clone(),
        successor: ctx.target.prefix.clone(),
        list: format!("sha256:{}", loaded.sha256),
        recovery: if refusals.is_empty() {
            "fresh"
        } else {
            "refuse"
        }
        .into(),
        carried,
        dropped: Vec::new(),
        closed: Vec::new(),
        inbound: Vec::new(),
        woken: Vec::new(),
        unread_projects: Vec::new(),
        paths: Default::default(),
        warnings,
        refusals,
        body_changes,
    }))
}
