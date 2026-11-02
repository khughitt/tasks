use super::{open_ctx, reject_retired};
use crate::error::{Error, Result};
use crate::migrating::{MigrateCtx, draft, list};
use crate::output::Output;
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
        |registered, target| {
            format!(
                "{prefix} is migrated from its registered checkout, which foreign readers and check read; run `tasks -C {} migrate --to {target} ...`",
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
    _explain: bool,
) -> Result<Output> {
    if let Some(path) = draft_path {
        let to = to.ok_or_else(|| Error::Validation("--draft needs --to".into()))?;
        let ctx = open(dir, &to)?;
        return Ok(Output::MigrateDraft(draft::write(&ctx, &path)?));
    }
    let path =
        list_path.ok_or_else(|| Error::Validation("migrate needs a list or --draft".into()))?;
    list::load(&path)?;
    Err(Error::Validation(
        "migrate <list> is not implemented yet".into(),
    ))
}
