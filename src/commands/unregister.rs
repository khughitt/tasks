use crate::error::Result;
use crate::output::{InitOut, Output};
use crate::registry::Registry;

/// Takes no project context: the directory a stale prefix points at may be one you no
/// longer want to enter, or one that no longer exists.
pub fn run(prefix: String) -> Result<Output> {
    let mut registry = Registry::load()?;
    let root = registry.unregister(&prefix)?;
    registry.save()?;
    Ok(Output::Init(InitOut {
        prefix,
        root: root.display().to_string(),
        warnings: Vec::new(),
    }))
}
