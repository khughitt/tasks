use crate::error::Result;
use crate::output::{InitOut, Output};
use crate::registry::Registry;

/// Takes no project context: the directory a stale prefix points at may be one you no
/// longer want to enter, or one that no longer exists.
pub fn run(prefix: String) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    let root = registry.project_root(registry.canonical_prefix(&prefix));
    super::reject_pending_rename_at(root, &prefix)?;
    let removed = registry.unregister(&prefix)?;
    registry.save()?;
    let warnings = removed
        .emptied_groups
        .iter()
        .map(|name| format!("group {name} lost its last member {prefix} and was deleted"))
        .collect();
    Ok(Output::Init(InitOut {
        prefix,
        root: removed.root.display().to_string(),
        warnings,
        aliases: removed.aliases,
    }))
}
