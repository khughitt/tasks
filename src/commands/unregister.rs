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
    let (root, aliases) = registry.unregister(&prefix)?;
    registry.save()?;
    Ok(Output::Init(InitOut {
        prefix,
        root: root.display().to_string(),
        warnings: Vec::new(),
        aliases,
    }))
}
