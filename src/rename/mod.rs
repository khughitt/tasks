pub mod classify;
pub mod inventory;
pub mod rewrite;
pub mod snapshot;

use crate::claims::{ClaimStore, Liveness};
use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::output::RenameOut;
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project, atomic_write};
use classify::{Recovery, classify};
use inventory::{Inventory, digest, rewrite_config_prefix};
use snapshot::{Invocation, observe};

/// The command holds both prefix locks and the registry lock for mutating calls.
/// Observation is repeated here after lock acquisition; explain needs none of the locks.
pub fn run(registry: &mut Registry, invocation: &Invocation, explain: bool) -> Result<RenameOut> {
    let snapshot = observe(registry, invocation, Inventory::load(&invocation.source)?)?;
    let recovery = classify(&snapshot);
    let mut warnings = Vec::new();
    let verdict = match &recovery {
        Recovery::Fresh => "fresh",
        Recovery::ResumeFiles => "resume_files",
        Recovery::ResumeRegistry => "resume_registry",
        Recovery::ResumeCleanup => "resume_cleanup",
        Recovery::Complete => "complete",
        Recovery::Refuse(reason) => {
            if !explain {
                let state = &snapshot.registry;
                let registry_old = state.old_key.as_ref() == Some(&invocation.root)
                    && state.new_key.is_none()
                    && state.alias.is_none();
                let registry_new = state.old_key.is_none()
                    && state.new_key.as_ref() == Some(&invocation.root)
                    && state.alias.as_ref() == Some(&invocation.target);
                return Err(
                    if registry.aliases.contains_key(&invocation.target)
                        || !(registry_old || registry_new)
                    {
                        Error::Config(reason.clone())
                    } else {
                        Error::Validation(reason.clone())
                    },
                );
            }
            warnings.push(reason.clone());
            "refuse"
        }
    };
    let mut out = RenameOut {
        prefix: invocation.target.clone(),
        previous: invocation.source.clone(),
        root: invocation.root.display().to_string(),
        tasks: 0,
        aliases: Vec::new(),
        recovery: verdict.into(),
        warnings,
    };
    out.aliases = registry
        .aliases
        .iter()
        .filter(|(_, live)| *live == &invocation.source || *live == &invocation.target)
        .map(|(alias, _)| alias.clone())
        .collect();
    if !out.aliases.contains(&invocation.source) {
        out.aliases.push(invocation.source.clone());
        out.aliases.sort();
    }
    if explain || recovery == Recovery::Complete {
        return Ok(out);
    }

    for pending in Inventory::pending()? {
        if pending.source == invocation.source {
            continue;
        }
        if pending.source == invocation.target || pending.target == invocation.target {
            return Err(Error::Config(format!(
                "prefix {:?} is reserved by unfinished rename {} -> {}",
                invocation.target, pending.source, pending.target
            )));
        }
        if pending.root == invocation.root || pending.target == invocation.source {
            return Err(Error::Validation(format!(
                "an unfinished rename {} -> {} already names this project; finish it first",
                pending.source, pending.target
            )));
        }
    }
    let project = Project::open(&invocation.root)?;
    authorize(&project, invocation, &mut out.warnings)?;
    let inventory = if recovery == Recovery::Fresh {
        if invocation.source == invocation.target {
            return Err(Error::Validation(format!(
                "source and target prefixes are both {:?}",
                invocation.source
            )));
        }
        if registry.is_taken(&invocation.target) {
            return Err(Error::Config(format!(
                "prefix {:?} is already taken",
                invocation.target
            )));
        }
        crate::commands::reject_pending_rename(&project)?;
        if let Some(dirty) = project.uncommitted_task_files()?
            && !dirty.is_empty()
        {
            return Err(Error::Validation(format!(
                "tasks/ has uncommitted changes: {}",
                dirty.join(", ")
            )));
        }
        let inventory = Inventory::build(&project, &invocation.target)?;
        inventory.save()?; // P2 is the first project/state mutation.
        if stop_after("inventory") {
            return Ok(out);
        }
        inventory
    } else {
        snapshot
            .inventory
            .expect("mutating recovery has an inventory")
    };

    if matches!(recovery, Recovery::Fresh | Recovery::ResumeFiles) {
        for entry in &inventory.entries {
            let source = project
                .tasks_dir()
                .join(format!("{}-{}.md", invocation.source, entry.hex));
            let dest = project
                .tasks_dir()
                .join(format!("{}-{}.md", invocation.target, entry.hex));
            let text = match std::fs::read_to_string(&source) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    verify_bytes(&dest, &entry.to)?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if digest(text.as_bytes()) != entry.from {
                return Err(Error::Validation(format!(
                    "{} changed during rename",
                    source.display()
                )));
            }
            inventory::validate_task_file(&source, &text)?;
            let rewritten = rewrite::rewrite_prefix(&text, &invocation.source, &invocation.target)?;
            if digest(rewritten.as_bytes()) != entry.to {
                return Err(Error::Validation(format!(
                    "{} transformation disagrees with inventory",
                    source.display()
                )));
            }
            match std::fs::read(&dest) {
                Ok(bytes) if digest(&bytes) == entry.to => {}
                Ok(_) => {
                    return Err(Error::Validation(format!(
                        "{} conflicts with inventory",
                        dest.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    atomic_write(&dest, rewritten.as_bytes())?;
                    out.tasks += 1;
                    if stop_after(&format!("file:{}", out.tasks)) {
                        return Ok(out);
                    }
                }
                Err(error) => return Err(error.into()),
            }
            // A digest never authorizes deleting the only surviving unverified copy.
            verify_bytes(&source, &entry.from)?;
            verify_bytes(&dest, &entry.to)?;
            std::fs::remove_file(source)?;
        }
        if stop_after("files") {
            return Ok(out);
        }
        let config = project.root.join(CONFIG_REL);
        verify_bytes(&config, &inventory.config_from)?;
        let rewritten =
            rewrite_config_prefix(&std::fs::read_to_string(&config)?, &invocation.target)?;
        if digest(rewritten.as_bytes()) != inventory.config_to {
            return Err(Error::Validation(format!(
                "{} transformation disagrees with inventory",
                config.display()
            )));
        }
        atomic_write(&config, rewritten.as_bytes())?;
        if stop_after("config") {
            return Ok(out);
        }
    }
    if recovery != Recovery::ResumeCleanup {
        registry.rename(&invocation.source, &invocation.target)?;
        registry.save()?;
        if stop_after("registry") {
            return Ok(out);
        }
    }
    // Keep the .lock inode: other processes can already be waiting on it.
    match std::fs::remove_file(ClaimStore::path_for(&invocation.source)?) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if stop_after("claims") {
        return Ok(out);
    }
    inventory.remove()?;
    Ok(out)
}

fn authorize(project: &Project, invocation: &Invocation, warnings: &mut Vec<String>) -> Result<()> {
    for prefix in [&invocation.source, &invocation.target] {
        for (id, claim) in ClaimStore::load(prefix)?.iter() {
            let live = crate::claims::liveness(claim);
            if live == Liveness::Live {
                return Err(Error::Claimed(
                    id.clone(),
                    Ctx::describe_claim(claim, &live),
                ));
            }
        }
    }
    match project.worktree_count()? {
        Some(count) if count > 1 => return Err(Error::Validation(format!("repository has {count} git worktrees; remove other worktrees before renaming"))),
        None => warnings.push("project is outside git; rollback is unavailable after source removal, use forward recovery".into()),
        _ => {}
    }
    Ok(())
}

fn verify_bytes(path: &std::path::Path, expected: &str) -> Result<()> {
    if digest(&std::fs::read(path)?) != expected {
        return Err(Error::Validation(format!(
            "{} disagrees with inventory",
            path.display()
        )));
    }
    Ok(())
}

fn stop_after(boundary: &str) -> bool {
    std::env::var("TASKS_RENAME_STOP_AFTER").is_ok_and(|value| value == boundary)
}
