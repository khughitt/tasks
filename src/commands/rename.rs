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
        && !crate::rename::snapshot::present(dir)?
        && let Some(renamed) = dir.parent().map(|parent| parent.join(new))
        && crate::rename::snapshot::present(&renamed.join(crate::repo::CONFIG_REL))?
    {
        let project = crate::repo::Project::open(&renamed)?;
        if project.prefix == new {
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
            reject_adopting_a_retired_project(&project)?;
            return crate::rename::adopt::run(&mut registry, &project, &old, &new, true)
                .map(Output::Rename);
        }
        // A checkout that holds no project fails before the locks are taken.
        adopt_project(&Registry::load()?, &start, &old, &new)?;
        let mut prefixes = [&old, &new];
        prefixes.sort();
        let _first = MutationLock::acquire(prefixes[0])?;
        let _second = MutationLock::acquire(prefixes[1])?;
        let _registry_lock = Registry::lock()?;
        let mut registry = Registry::load()?;
        let project = adopt_project(&registry, &start, &old, &new)?;
        reject_adopting_a_retired_project(&project)?;
        return crate::rename::adopt::run(&mut registry, &project, &old, &new, false)
            .map(Output::Rename);
    }
    let mut registry = Registry::load()?;
    let mut requested = invocation(&registry, &old, &new)?;
    if explain {
        // As the run refuses, so explain never reports a stage it would not reach.
        reject_renaming_a_retired_project(&registry, &requested.source)?;
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
            reject_renaming_a_retired_project(&registry, &requested.source)?;
            return crate::rename::run(&mut registry, &requested, false).map(Output::Rename);
        }
        requested = refreshed;
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring rename locks; retry".into(),
    ))
}

/// `--adopt` and its `--explain` refuse alike, so explain never reports a stage the run
/// would refuse.
fn reject_adopting_a_retired_project(project: &crate::repo::Project) -> Result<()> {
    if project.retired.is_some() {
        return Err(Error::RetiredProject(format!(
            "{} is retired; a retired project is not renamed",
            project.prefix
        )));
    }
    Ok(())
}

/// A retired project is not renamed (ops lifecycle spec §4). Read without the prefix
/// check, so a pending rename whose config already carries the new prefix still resumes;
/// a config that does not parse fails here as it would in the rename itself.
fn reject_renaming_a_retired_project(registry: &Registry, source: &str) -> Result<()> {
    if let Some(dir) = crate::scope::config_dir(registry, source)
        && dir.join(crate::repo::CONFIG_REL).exists()
        && crate::repo::Project::open(&dir)?.retired.is_some()
    {
        return Err(Error::RetiredProject(format!(
            "{source} is retired; a retired project is not renamed (`tasks unretire` in {} first)",
            dir.display()
        )));
    }
    Ok(())
}
