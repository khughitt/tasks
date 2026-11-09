//! Validate and freeze the retired generation before reclaiming its old prefix.
use super::inventory::{Inventory, PredecessorHandoff, digest, task_paths};
use super::snapshot::Invocation;
use crate::claims::ClaimStore;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::collections::BTreeSet;

pub(crate) fn check_handoff_bound(registry: &Registry, project: &Project) -> Result<()> {
    registry.check_rename_bound(&project.prefix)?;
    let participates = || {
        Error::Validation(format!(
            "{} participates in a predecessor handoff; later generation renames or unretire are unsupported because historical ID routing must be preserved",
            project.prefix
        ))
    };
    if project.predecessor.is_some() {
        return Err(participates());
    }
    if let Some(successor) = project
        .retired
        .as_ref()
        .and_then(|r| r.successor.as_deref())
    {
        let successor_project = crate::scope::open_registered_config(registry, successor, crate::scope::Origin::Prefix)
            .map_err(|error| Error::Validation(format!(
                "cannot verify the handoff bound for {}: register and pair declared successor {successor:?} before changing this retired project ({error})", project.prefix
            )))?;
        if successor_project
            .predecessor
            .as_ref()
            .is_some_and(|p| p.prefix == project.prefix)
        {
            return Err(participates());
        }
    }
    Ok(())
}

pub(super) fn validate(
    registry: &Registry,
    invocation: &Invocation,
    inventory: Option<&Inventory>,
) -> Result<Option<PredecessorHandoff>> {
    let existing = inventory.and_then(|i| i.predecessor_handoff.as_ref());
    let Some(prefix) = existing
        .map(|h| &h.prefix)
        .or_else(|| registry.aliases.get(&invocation.target))
    else {
        return Ok(None);
    };
    if prefix == &invocation.source || prefix == &invocation.target {
        return Ok(None);
    }
    let pred =
        crate::scope::open_registered_config(registry, prefix, crate::scope::Origin::Prefix)?;
    crate::commands::reject_pending_operations(Some(&pred.root), prefix)?;
    let store = ClaimStore::load(prefix)?;
    if store.iter().next().is_some() || !store.carries_nothing() {
        return Err(Error::Validation(format!(
            "predecessor {prefix} has claims, parks or escalations; clear them before handoff"
        )));
    }
    if Inventory::load_commit_receipt(prefix)?.is_some() {
        return Err(Error::Validation(format!(
            "predecessor rename is uncommitted; run `tasks commit --project {prefix}` first"
        )));
    }
    let config_path = pred.records.join(CONFIG_REL);
    let bytes = std::fs::read(&config_path)?;
    let current = digest(&bytes);
    let successor = pred.retired.as_ref().and_then(|r| r.successor.as_deref());
    if let Some(successor) = successor
        && successor != invocation.source
        && registry.canonical_prefix(successor) == invocation.source
    {
        return Err(Error::Validation(format!(
            "handoff with prior successor alias history is unsupported: predecessor {prefix} names {successor}, which resolves to {}; fresh-host handoff preserves only the immediate successor prefix",
            invocation.source
        )));
    }
    if pred.predecessor.is_some()
        || !(successor == Some(invocation.source.as_str())
            || existing.is_some_and(|h| {
                current == h.config_to && successor == Some(invocation.target.as_str())
            }))
    {
        return Err(Error::Config(format!(
            "predecessor {prefix} must be retired with successor {} and have no predecessor",
            invocation.source
        )));
    }
    if let Some(h) = existing {
        if config_path != h.config_path || (current != h.config_from && current != h.config_to) {
            return Err(Error::Validation(format!(
                "predecessor {prefix} config differs from rename inventory"
            )));
        }
    } else {
        if let Some(status) = pred.status_paths(&["tasks"], true)?
            && (!status.dirty.is_empty() || !status.ignored.is_empty())
        {
            return Err(Error::Validation(format!(
                "predecessor {prefix} has uncommitted tasks/ changes; commit Step 1 first"
            )));
        }
        if pred.kind == crate::repo::Kind::Detached {
            let parent = pred.records.parent().expect("detached records parent");
            let scope = Project {
                records: parent.into(),
                ..pred.clone()
            };
            if let Some(status) = scope.status_paths(&[&invocation.target], true)?
                && (!status.dirty.is_empty() || !status.ignored.is_empty())
            {
                return Err(Error::Validation(format!(
                    "records/{} has uncommitted Step 1 changes for {prefix}",
                    invocation.target
                )));
            }
        }
    }
    let suffixes = scan_suffixes(&pred.tasks_dir(), prefix)?;
    if let Some(h) = existing
        && suffixes != h.suffixes
    {
        return Err(Error::Validation(format!(
            "predecessor {prefix} task suffixes differ from inventory"
        )));
    }
    let source_suffixes = match inventory {
        Some(i) => i.entries.iter().map(|e| e.hex.clone()).collect(),
        None => scan_suffixes(&invocation.records.join("tasks"), &invocation.source)?,
    };
    if let Some(hex) = suffixes.intersection(&source_suffixes).next() {
        return Err(Error::Validation(format!(
            "predecessor suffix collision: {prefix}-{hex} and {}-{hex}",
            invocation.source
        )));
    }
    if let Some(h) = existing {
        return Ok(Some(h.clone()));
    }
    let mut config: toml::Value =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|e| Error::Config(e.to_string()))?)
            .map_err(|e| Error::Config(e.to_string()))?;
    config["retired"]["successor"] = invocation.target.clone().into();
    let config_payload = toml::to_string(&config).map_err(|e| Error::Config(e.to_string()))?;
    Ok(Some(PredecessorHandoff {
        source_config: std::fs::read_to_string(invocation.records.join(CONFIG_REL))?,
        prefix: prefix.clone(),
        suffixes,
        config_path,
        config_from: current,
        config_to: digest(config_payload.as_bytes()),
        config_payload,
    }))
}

fn scan_suffixes(dir: &std::path::Path, prefix: &str) -> Result<BTreeSet<String>> {
    task_paths(dir)?
        .iter()
        .map(|p| {
            let id = TaskId::parse(p.file_stem().and_then(|n| n.to_str()).unwrap_or(""))?;
            if id.prefix != prefix {
                return Err(Error::Validation(format!(
                    "unexpected task {} in predecessor/source inventory",
                    id
                )));
            }
            Ok(id.hex)
        })
        .collect()
}
