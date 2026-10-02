use crate::error::Result;
use crate::output::{Output, RootOut};
use crate::registry::Registry;
use crate::scope::{Origin, open_registered, registry_warnings};
use crate::shorthand::Shorthand;
use std::path::Path;

/// Where the id's project lives, for a shell alias or dashboard to jump to. The task file
/// is not checked: a missing file is `show`'s to report, and the caller asked for the root.
pub fn run(id: String, dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let start = super::start_dir(dir)?;
    let id = super::parse_id(&registry, &Shorthand::new(start.clone()), &id)?;
    let warnings = registry_warnings(&registry, &start)?;
    let project = open_registered(&registry, &id.prefix, Origin::Id(&id))?;
    Ok(Output::Root(RootOut {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        warnings,
    }))
}
