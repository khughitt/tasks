use crate::claims::{ClaimStore, Liveness};
use crate::commands::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::RenameOut;
use crate::registry::Registry;
use crate::rename::{inventory, root_identity};
use crate::repo::{Project, atomic_write};
use std::collections::BTreeSet;
use std::io::ErrorKind;

enum Stage {
    Fresh(String),
    ResumeRegistry,
    ResumeCleanup,
    Complete,
    Refuse(String),
}

pub fn run(
    registry: &mut Registry,
    project: &Project,
    old: &str,
    new: &str,
    explain: bool,
) -> Result<RenameOut> {
    let source = ClaimStore::load(old)?;
    let target = ClaimStore::load(new)?;
    let old_path = ClaimStore::path_for(old)?;
    let target_path = ClaimStore::path_for(new)?;
    let old_exists = exists(&old_path)?;
    let target_exists = exists(&target_path)?;
    let mut warnings = Vec::new();
    let mut tasks = BTreeSet::new();
    let mut stage = if let Err(error) =
        crate::commands::reject_pending_rename_at(Some(&project.root), old)
            .and_then(|()| crate::commands::reject_pending_rename_at(Some(&project.root), new))
    {
        Stage::Refuse(error.to_string())
    } else {
        classify_registry(registry, project, old, new)?
    };

    if !matches!(stage, Stage::Refuse(_)) {
        for path in inventory::task_paths(&project.tasks_dir())? {
            let text = std::fs::read_to_string(&path)?;
            let task = inventory::validate_task_file(&path, &text)
                .and_then(|()| crate::format::parse_task(&text, &path.display().to_string()));
            match task {
                Ok(task) if task.id.prefix == new => {
                    tasks.insert(task.id.to_string());
                }
                Ok(task) => {
                    stage = Stage::Refuse(format!("task {} does not use prefix {new:?}", task.id));
                    break;
                }
                Err(error) => {
                    stage = Stage::Refuse(error.to_string());
                    break;
                }
            }
        }
    }

    if !matches!(stage, Stage::Refuse(_)) {
        for id in source
            .parks()
            .map(|(id, _)| id)
            .chain(source.escalations().map(|(id, _)| id))
        {
            let parsed = TaskId::parse(id)?;
            if parsed.prefix != old {
                stage = Stage::Refuse(format!("state entry {id} does not belong to {old:?}"));
                break;
            }
            let renamed = format!("{new}-{}", parsed.hex);
            if !tasks.contains(&renamed) {
                warnings.push(format!("carried state for absent task {renamed}"));
            }
        }
    }

    stage = match stage {
        Stage::Fresh(_) => match source.adoption_target_text(old, new, &target) {
            Ok(Some(text)) => Stage::Fresh(text),
            Ok(None) if source.carries_nothing() => Stage::Fresh(String::new()),
            Ok(None) => Stage::ResumeRegistry,
            Err(error) => Stage::Refuse(error.to_string()),
        },
        Stage::ResumeCleanup if !source.carries_nothing() && !target_exists => {
            Stage::Refuse(format!(
                "{} is missing after registry adoption",
                target_path.display()
            ))
        }
        other => other,
    };

    let recovery = match &stage {
        Stage::Fresh(_) => "fresh",
        Stage::ResumeRegistry => "resume_registry",
        Stage::ResumeCleanup => "resume_cleanup",
        Stage::Complete => "complete",
        Stage::Refuse(reason) => {
            if !explain {
                return Err(Error::Validation(reason.clone()));
            }
            warnings.push(reason.clone());
            "refuse"
        }
    };
    let mut aliases: Vec<String> = registry
        .aliases
        .iter()
        .filter(|(_, live)| *live == old || *live == new)
        .map(|(alias, _)| alias.clone())
        .collect();
    if !aliases.iter().any(|alias| alias == old) {
        aliases.push(old.into());
        aliases.sort();
    }
    let (parks, escalations) = if matches!(stage, Stage::Fresh(_)) {
        (source.parks().count(), source.escalations().count())
    } else {
        (target.parks().count(), target.escalations().count())
    };
    let out = RenameOut {
        mode: Some("adopt".into()),
        prefix: new.into(),
        previous: old.into(),
        root: project.root.display().to_string(),
        tasks: 0,
        parks,
        escalations,
        aliases,
        recovery: recovery.into(),
        warnings,
    };
    if explain || matches!(stage, Stage::Complete | Stage::Refuse(_)) {
        return Ok(out);
    }

    if matches!(stage, Stage::Fresh(_) | Stage::ResumeRegistry) {
        for (id, claim) in source.iter().chain(target.iter()) {
            let live = crate::claims::liveness(claim);
            if live == Liveness::Live {
                return Err(Error::Claimed(
                    id.clone(),
                    Ctx::describe_claim(claim, &live),
                ));
            }
        }
    }
    if let Stage::Fresh(text) = stage
        && !text.is_empty()
    {
        atomic_write(&target_path, text.as_bytes())?;
        if std::fs::read(&target_path)? != text.as_bytes() {
            return Err(Error::Validation(format!(
                "{} changed during adoption",
                target_path.display()
            )));
        }
        if super::stop_after("store") {
            return Ok(out);
        }
    }
    if recovery == "fresh" || recovery == "resume_registry" {
        registry.adopt(old, new, &project.root)?;
        registry.save()?;
        if super::stop_after("registry") {
            return Ok(out);
        }
    }
    if old_exists {
        std::fs::remove_file(old_path)?;
    }
    if super::stop_after("claims") {
        return Ok(out);
    }
    Ok(out)
}

fn classify_registry(
    registry: &Registry,
    project: &Project,
    old: &str,
    new: &str,
) -> Result<Stage> {
    if project.prefix != new || old == new {
        return Ok(Stage::Refuse(format!(
            "checkout prefix must be {new:?}, and source must differ"
        )));
    }
    if let Some(live) = registry.aliases.get(old)
        && live != new
    {
        return Ok(Stage::Refuse(format!(
            "{old:?} is an alias of {live:?}; use the live key"
        )));
    }
    if registry.aliases.contains_key(new) {
        return Ok(Stage::Refuse(format!("target prefix {new:?} is retired")));
    }
    if registry.groups.contains_key(new) {
        return Ok(Stage::Refuse(format!(
            "target prefix {new:?} is the name of a group; remove it with `tasks group rm {new}` first"
        )));
    }
    let root = root_identity(&project.root)?;
    let source = registry.project_root(old);
    let target = registry.project_root(new);
    if let Some(old_root) = source
        && root_identity(old_root)? != root
        && old_root.join(crate::repo::CONFIG_REL).is_file()
    {
        return Ok(Stage::Refuse(format!(
            "old root {} still holds a tasks project",
            old_root.display()
        )));
    }
    if let Some(target_root) = target
        && root_identity(target_root)? != root
    {
        return Ok(Stage::Refuse(format!(
            "target {new:?} is registered at {}",
            target_root.display()
        )));
    }
    for (prefix, registered) in &registry.projects {
        if prefix != old && prefix != new && root_identity(registered)? == root {
            return Ok(Stage::Refuse(format!(
                "project {prefix:?} already names this root"
            )));
        }
    }
    if source.is_some() && !registry.aliases.contains_key(old) {
        Ok(Stage::Fresh(String::new()))
    } else if source.is_none()
        && target.is_some()
        && registry.aliases.get(old).is_some_and(|live| live == new)
    {
        let old_path = ClaimStore::path_for(old)?;
        if exists(&old_path)? {
            Ok(Stage::ResumeCleanup)
        } else {
            Ok(Stage::Complete)
        }
    } else {
        Ok(Stage::Refuse(format!(
            "registry cannot adopt {old:?} as {new:?}"
        )))
    }
}

fn exists(path: &std::path::Path) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
