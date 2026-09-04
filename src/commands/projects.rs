use crate::error::Result;
use crate::output::{Counts, Output, ProjectRow, ProjectsOut};
use crate::registry::Registry;
use crate::scope::{Origin, is_reachable, open_registered, registry_warnings};
use std::path::Path;

/// The registry as rows. Missing roots/configs are unreachable rows; malformed reachable
/// configs still error because a listed project must agree with its registered prefix.
pub fn run(dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut rows = Vec::new();
    for (prefix, root) in &registry.projects {
        let reachable = is_reachable(root)?;
        let counts = if reachable {
            let project = open_registered(&registry, prefix, Origin::Prefix)?;
            Some(Counts::of(&project.scan()?))
        } else {
            None
        };
        rows.push(ProjectRow {
            prefix: prefix.clone(),
            root: root.display().to_string(),
            reachable,
            counts,
        });
    }
    Ok(Output::Projects(ProjectsOut {
        projects: rows,
        warnings,
    }))
}
