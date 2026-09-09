use crate::claims::MutationLock;
use crate::error::{Error, Result};
use crate::model::is_valid_prefix;
use crate::output::Output;
use crate::registry::Registry;
use crate::rename::inventory::Inventory;
use crate::rename::snapshot::Invocation;

/// Keep the old spelling for recovery; an older alias may also start a new rename.
fn invocation(registry: &Registry, old: &str, new: &str) -> Result<Invocation> {
    let mut pending = Inventory::pending()?;
    for inventory in &mut pending {
        inventory.root = crate::rename::root_identity(&inventory.root)?;
    }
    let live = registry.canonical_prefix(old);
    let live_root = registry
        .project_root(live)
        .map(crate::rename::root_identity)
        .transpose()?;
    let replay = registry
        .aliases
        .get(old)
        .is_some_and(|target| target == new);
    let baseline = pending
        .iter()
        .find(|inventory| inventory.source == old)
        .or_else(|| pending.iter().find(|inventory| inventory.source == live))
        .or_else(|| {
            pending.iter().find(|inventory| {
                replay
                    && inventory.target == new
                    && live_root.as_deref() == Some(inventory.root.as_path())
            })
        });
    let source = baseline
        .map(|inventory| inventory.source.as_str())
        .unwrap_or(if replay { old } else { live });
    if !is_valid_prefix(source) {
        return Err(Error::Config(format!("invalid source prefix {source:?}")));
    }
    let inventory = Inventory::load(source)?;
    let root = if let Some(root) = registry.project_root(registry.canonical_prefix(source)) {
        crate::rename::root_identity(root)?
    } else if let Some(inventory) = &inventory {
        crate::rename::root_identity(&inventory.root)?
    } else {
        return Err(Error::Config(format!("no project registered as {old:?}")));
    };
    Ok(Invocation {
        source: source.into(),
        target: new.into(),
        root,
    })
}

pub fn run(old: String, new: String, explain: bool) -> Result<Output> {
    for prefix in [&old, &new] {
        if !is_valid_prefix(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
            )));
        }
    }
    let mut registry = Registry::load()?;
    let mut requested = invocation(&registry, &old, &new)?;
    if explain {
        return crate::rename::run(&mut registry, &requested, true).map(Output::Rename);
    }
    for _ in 0..4 {
        let mut prefixes = [&requested.source, &requested.target];
        prefixes.sort();
        let _first = MutationLock::acquire(prefixes[0])?;
        let _second = if prefixes[0] != prefixes[1] {
            Some(MutationLock::acquire(prefixes[1])?)
        } else {
            None
        };
        let _registry_lock = Registry::lock()?;
        registry = Registry::load()?;
        let refreshed = invocation(&registry, &old, &new)?;
        if refreshed == requested {
            return crate::rename::run(&mut registry, &requested, false).map(Output::Rename);
        }
        requested = refreshed;
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring rename locks; retry".into(),
    ))
}
