use crate::error::Result;
use crate::output::{Counts, Output, ProjectRow, ProjectsOut};
use crate::registry::Registry;
use crate::scope::{Origin, is_reachable, open_registered, registry_warnings};
use std::path::Path;

/// The registry as rows. Missing roots/configs are unreachable rows; malformed reachable
/// configs still error because a listed project must agree with its registered prefix.
pub fn run(dir: Option<&Path>, closed: bool, paths: bool) -> Result<Output> {
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut rows = Vec::new();
    for (prefix, root) in &registry.projects {
        let reachable = is_reachable(root)?;
        let (counts, total, last_activity) = if reachable {
            let project = open_registered(&registry, prefix, Origin::Prefix)?;
            let tasks = project.scan()?;
            // Timestamps are fixed-width UTC with no fractional part, so the lexical max
            // is the chronological one.
            let last = tasks.iter().map(|task| task.updated.clone()).max();
            (Some(Counts::of(&tasks)), Some(tasks.len()), last)
        } else {
            (None, None, None)
        };
        rows.push(ProjectRow {
            prefix: prefix.clone(),
            root: root.display().to_string(),
            reachable,
            counts,
            total,
            last_activity,
        });
    }
    Ok(Output::Projects(ProjectsOut {
        projects: rows,
        warnings,
        closed,
        paths,
    }))
}
