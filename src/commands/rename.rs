use crate::claims::MutationLock;
use crate::error::{Error, Result};
use crate::model::is_valid_prefix;
use crate::output::Output;
use crate::registry::Registry;
use crate::rename::inventory::Inventory;
use crate::rename::snapshot::Invocation;
use std::path::Path;

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
    let records = match &inventory {
        Some(Inventory {
            records_from: Some(from),
            records_to: Some(to),
            ..
        }) if crate::rename::snapshot::present(to)? && !crate::rename::snapshot::present(from)? => {
            to.clone()
        }
        Some(Inventory {
            records_from: Some(from),
            ..
        }) => from.clone(),
        // The live prefix, as for `root`: after `det -> dtx` a replay still says
        // `source = det`, and only `dtx` has registry entries.
        _ => crate::scope::config_dir(registry, registry.canonical_prefix(source))
            .unwrap_or_else(|| root.clone()),
    };
    Ok(Invocation {
        source: source.into(),
        target: new.into(),
        root,
        records: crate::rename::dir_identity(&records)?,
    })
}

/// The project `--adopt` adopts. When another host renamed a detached project, the sync
/// delivers its records directory under the new name: `[records]` still pairs `old` with
/// a directory that is gone, and its sibling named `new` holds a config with prefix `new`.
/// Otherwise the project `start` locates.
fn adopt_project(
    registry: &Registry,
    start: &Path,
    old: &str,
    new: &str,
) -> Result<crate::repo::Project> {
    if let Some(dir) = registry.records_dir(old)
        && let Some(renamed) = dir.parent().map(|parent| parent.join(new))
        && crate::rename::snapshot::present(&renamed.join(crate::repo::CONFIG_REL))?
    {
        let project = crate::repo::Project::open(&renamed)?;
        if project.prefix == new
            && (!crate::rename::snapshot::present(dir)?
                || crate::rename::root_identity(&project.root)?
                    == crate::rename::root_identity(start)?)
        {
            return Ok(project);
        }
    }
    crate::repo::Project::locate(start)
}

pub fn run(
    dir: Option<&Path>,
    old: String,
    new: String,
    explain: bool,
    adopt: bool,
    host: &crate::config::HostConfig,
) -> Result<Output> {
    for prefix in [&old, &new] {
        if !is_valid_prefix(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
            )));
        }
    }
    if adopt {
        if old == new {
            return Err(Error::Validation(format!(
                "source and target prefixes are both {old:?}"
            )));
        }
        let start = super::start_dir(dir)?;
        if explain {
            let mut registry = Registry::load()?;
            let project = adopt_project(&registry, &start, &old, &new)?;
            return crate::rename::adopt::run(&mut registry, &project, &old, &new, true, host)
                .map(Output::Rename);
        }
        // A checkout that holds no project fails before the locks are taken.
        adopt_project(&Registry::load()?, &start, &old, &new)?;
        for _ in 0..4 {
            let prefixes = rename_lock_prefixes(&Registry::load()?, &old, &new)?;
            let _locks = prefixes
                .iter()
                .map(|p| MutationLock::acquire(p))
                .collect::<Result<Vec<_>>>()?;
            let _registry_lock = Registry::lock()?;
            let mut registry = Registry::load()?;
            if prefixes != rename_lock_prefixes(&registry, &old, &new)? {
                continue;
            }
            let project = adopt_project(&registry, &start, &old, &new)?;
            return crate::rename::adopt::run(&mut registry, &project, &old, &new, false, host)
                .map(Output::Rename);
        }
        return Err(Error::Io(
            "the predecessor changed while acquiring adoption locks; retry".into(),
        ));
    }
    let mut registry = Registry::load()?;
    let mut requested = invocation(&registry, &old, &new)?;
    if explain {
        // As the run refuses, so explain never reports a stage it would not reach.
        return crate::rename::run(&mut registry, &requested, true, host).map(Output::Rename);
    }
    for _ in 0..4 {
        let prefixes = rename_lock_prefixes(&registry, &requested.source, &requested.target)?;
        let _locks = prefixes
            .iter()
            .map(|p| MutationLock::acquire(p))
            .collect::<Result<Vec<_>>>()?;
        let _registry_lock = Registry::lock()?;
        registry = Registry::load()?;
        let refreshed = invocation(&registry, &old, &new)?;
        if refreshed == requested
            && prefixes == rename_lock_prefixes(&registry, &refreshed.source, &refreshed.target)?
        {
            return crate::rename::run(&mut registry, &requested, false, host).map(Output::Rename);
        }
        requested = refreshed;
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring rename locks; retry".into(),
    ))
}

fn rename_lock_prefixes(registry: &Registry, source: &str, target: &str) -> Result<Vec<String>> {
    let mut prefixes = vec![source.to_string(), target.to_string()];
    if let Some(pred) = registry.aliases.get(target) {
        prefixes.push(pred.clone());
    }
    if let Some(inventory) = Inventory::load(source)?
        && let Some(handoff) = inventory.predecessor_handoff
    {
        prefixes.push(handoff.prefix);
    }
    prefixes.sort();
    prefixes.dedup();
    Ok(prefixes)
}
