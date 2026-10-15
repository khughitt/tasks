//! `tasks retire` / `tasks unretire` (ops lifecycle spec §3–§4). Lock order: this
//! project's MutationLock only; the registry is read, never locked, because every command
//! that changes this prefix's root (`init --force`, `rename`) takes this lock first.

use crate::claims::{ClaimStore, MutationLock};
use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project, Retirement, validate_retirement};
use crate::scope::Origin;
use std::path::Path;

pub fn retire(here: Project, successor: Option<String>, reason: String) -> Result<Retirement> {
    let _lock = MutationLock::acquire(&here.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &here)?;
    let retirement = Retirement {
        // `Date`'s Display is YYYY-MM-DD.
        date: time::OffsetDateTime::now_utc().date().to_string(),
        successor,
        reason,
    };
    validate_retirement(&retirement)?;
    if let Some(s) = &retirement.successor {
        let live = registry.canonical_prefix(s);
        let usable = live != project.prefix
            && registry.project_root(live).is_some()
            && crate::scope::open_registered(&registry, live, Origin::Prefix)?
                .retired
                .is_none();
        if !usable {
            return Err(Error::Config(format!(
                "successor {s:?} must be a registered, unretired prefix other than {}",
                project.prefix
            )));
        }
    }
    reject_live_claims(&project)?;
    reject_open_copies(&project)?;
    write_table(&project, Some(&retirement))?;
    Ok(retirement)
}

pub fn unretire(here: Project) -> Result<()> {
    let _lock = MutationLock::acquire(&here.prefix)?;
    let registry = Registry::load()?;
    let project = authority(&registry, &here)?;
    if project.retired.is_some() {
        write_table(&project, None)?;
    }
    Ok(())
}

/// The registered checkout, re-opened under the lock, with the pending-operation freeze
/// applied as every writer applies it; a linked worktree is refused.
fn authority(registry: &Registry, here: &Project) -> Result<Project> {
    super::reject_pending_operations(Some(&here.root), &here.prefix)?;
    let root = registry
        .project_root(&here.prefix)
        .ok_or_else(|| Error::Config(format!("{} is not registered on this host", here.prefix)))?;
    if root != here.root.as_path() {
        return Err(Error::NotRegisteredCheckout(format!(
            "run this in the registered checkout of {}: {}",
            here.prefix,
            root.display()
        )));
    }
    let project = Project::open(root)?;
    super::reject_stale_local(registry, &project)?;
    Ok(project)
}

fn reject_live_claims(project: &Project) -> Result<()> {
    let store = ClaimStore::load(&project.prefix)?;
    for (id, claim) in store.iter() {
        let live = crate::claims::liveness(claim);
        if !live.is_stale() {
            let hint = match live {
                crate::claims::Liveness::Unverifiable(_) => {
                    super::release::hint(id, &claim.session)
                }
                _ => String::new(),
            };
            return Err(Error::Claimed(
                id.clone(),
                super::Ctx::describe_claim(claim, &live) + &hint,
            ));
        }
    }
    Ok(())
}

/// Every checkout's tasks/, the registered one included: a todo filed in a worktree and
/// never merged is open work too. Anything that cannot be read stops the retirement.
fn reject_open_copies(project: &Project) -> Result<()> {
    let unreadable = |what: &Path, e: &dyn std::fmt::Display| {
        Error::UnreadableCheckout(format!("{}: {e}", what.display()))
    };
    let mut open = Vec::new();
    for root in project.checkout_roots()? {
        let dir = root.join("tasks");
        for entry in std::fs::read_dir(&dir).map_err(|e| unreadable(&dir, &e))? {
            let path = entry.map_err(|e| unreadable(&dir, &e))?.path();
            // Records only, as `Project::task_files` reads them: a dot-prefixed stem is a
            // kept edit temp (`.<id>.<hex>.edit.md`), not a task.
            if path.extension().is_none_or(|x| x != "md")
                || path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            let raw = std::fs::read_to_string(&path).map_err(|e| unreadable(&path, &e))?;
            let file = format!("tasks/{}", path.file_name().unwrap().to_string_lossy());
            let task = crate::format::parse_task(&raw, &file).map_err(|e| unreadable(&path, &e))?;
            if task.status.is_open() {
                open.push(format!(
                    "{} ({}) in {}",
                    task.id,
                    task.status.as_str(),
                    root.display()
                ));
            }
        }
    }
    if open.is_empty() {
        Ok(())
    } else {
        Err(Error::OpenTasks(format!(
            "close these first: {}",
            open.join(", ")
        )))
    }
}

fn write_table(project: &Project, retirement: Option<&Retirement>) -> Result<()> {
    let path = project.root.join(CONFIG_REL);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| Error::Config(format!("{CONFIG_REL}: {e}")))?;
    match retirement {
        Some(r) => {
            let mut table = toml_edit::Table::new();
            table["date"] = toml_edit::value(&r.date);
            if let Some(s) = &r.successor {
                table["successor"] = toml_edit::value(s);
            }
            table["reason"] = toml_edit::value(&r.reason);
            doc["retired"] = toml_edit::Item::Table(table);
        }
        None => {
            doc.remove("retired");
        }
    }
    crate::repo::atomic_write(&path, doc.to_string().as_bytes())
}
