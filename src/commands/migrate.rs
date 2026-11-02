use super::{open_ctx, reject_retired};
use crate::error::{Error, Result};
use crate::migrating::{MigrateCtx, draft, list, preflight};
use crate::output::{MigrateOut, Output};
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
    let ctx = open(dir, &loaded.list.successor)?;
    let refusals = preflight::check_list(&ctx, &loaded)?;
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
        carried: Vec::new(),
        dropped: Vec::new(),
        closed: Vec::new(),
        inbound: Vec::new(),
        woken: Vec::new(),
        unread_projects: Vec::new(),
        paths: Default::default(),
        warnings,
        refusals,
        body_changes: Vec::new(),
    }))
}
