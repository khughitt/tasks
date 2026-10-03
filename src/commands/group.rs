//! `tasks group set|rm` and `tasks groups`: named sets of registered projects
//! (docs/specs/2026-10-03-lanes-needs-groups-design.md §6). Registry-only, like
//! `unregister`: no project context, and the registry lock around every write.

use crate::error::{Error, Result};
use crate::output::{GroupMember, GroupOut, GroupRow, GroupsOut, Output};
use crate::registry::Registry;
use crate::rename::inventory::Inventory;
use crate::scope::{is_reachable, registry_warnings};
use std::path::Path;

pub fn set(name: String, prefixes: Vec<String>) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    reject_rename_reservation(&name)?;
    let members = registry.set_group(&name, &prefixes)?;
    registry.save()?;
    Ok(Output::Group(GroupOut {
        name,
        members,
        warnings: Vec::new(),
    }))
}

/// A name an unfinished rename reserves (its source or its target) stays free until that
/// rename finishes, as `init` keeps it. A group under the target would let the resume
/// rewrite files and config, then fail at the registry step. The caller holds the
/// registry lock, and a rename writes its inventory under that lock, so no reservation
/// can appear between this check and the save. `validation`, as every colliding group
/// name is.
fn reject_rename_reservation(name: &str) -> Result<()> {
    for pending in Inventory::pending()? {
        if pending.source == name || pending.target == name {
            return Err(Error::Validation(format!(
                "group name {name:?} is reserved by unfinished rename {} -> {}; \
                 finish it with `tasks rename {} {}` first",
                pending.source, pending.target, pending.source, pending.target
            )));
        }
    }
    Ok(())
}

pub fn rm(name: String) -> Result<Output> {
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    let members = registry.remove_group(&name)?;
    registry.save()?;
    Ok(Output::Group(GroupOut {
        name,
        members,
        warnings: Vec::new(),
    }))
}

/// Every group in name order, each member with the reachability test `projects` uses.
pub fn list(dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut groups = Vec::new();
    for (name, members) in &registry.groups {
        let mut rows = Vec::new();
        for prefix in members {
            // `load` guarantees every member is registered; say so if that ever breaks.
            let root = registry.project_root(prefix).ok_or_else(|| {
                Error::Config(format!(
                    "group {name:?} names {prefix:?}, which is not a registered project"
                ))
            })?;
            rows.push(GroupMember {
                prefix: prefix.clone(),
                reachable: is_reachable(root)?,
            });
        }
        groups.push(GroupRow {
            name: name.clone(),
            members: rows,
        });
    }
    Ok(Output::Groups(GroupsOut { groups, warnings }))
}
