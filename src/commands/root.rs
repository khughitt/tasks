use crate::error::Result;
use crate::model::TaskId;
use crate::output::{Output, RootOut};
use crate::registry::Registry;
use crate::scope::{Origin, open_registered, registry_warnings};
use std::path::Path;

/// Where the id's project lives, for a shell alias or dashboard to jump to. The task file
/// is not checked: a missing file is `show`'s to report, and the caller asked for the root.
pub fn run(id: String, dir: Option<&Path>) -> Result<Output> {
    let id = TaskId::parse(&id)?;
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let project = open_registered(&registry, &id.prefix, Origin::Id(&id))?;
    Ok(Output::Root(RootOut {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        warnings,
    }))
}
