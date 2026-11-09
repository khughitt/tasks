//! `tasks commit`: commits specific task IDs (`tasks/<id>.md` + `tasks/files/<id>/`),
//! records-relative paths (such as `docs/specs/...`), or every uncommitted file under
//! `tasks/` in one commit under the repository's `.commit-<common_hex>.lock`
//! (docs/project-operations.md). An explicit request, so it ignores `[git] commit` and
//! fails loudly when it cannot commit.
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::{CommitOut, Output};
use crate::rename::inventory::{Inventory, digest};
use crate::repo::{CONFIG_REL, Project};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
enum TargetKind {
    FullId(TaskId),
    BareSuffix(String),
    Path(String),
}

fn classify_target(raw: &str) -> TargetKind {
    let trimmed = raw.trim_end_matches(['.', ',']);
    if !raw.contains(['/', '\\']) && !trimmed.contains('.') {
        if let Ok(id) = TaskId::parse_input(raw) {
            return TargetKind::FullId(id);
        }
        if trimmed.len() == 6
            && trimmed
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return TargetKind::BareSuffix(trimmed.to_string());
        }
    }
    TargetKind::Path(raw.to_string())
}

fn is_tracked(records: &Path, relpath: &str) -> Result<bool> {
    let output = crate::git::at(records)
        .args([
            "--literal-pathspecs",
            "ls-files",
            "--error-unmatch",
            "--",
            relpath,
        ])
        .output()?;
    Ok(output.status.success())
}

fn normalize_records_relpath(records: &Path, raw: &str) -> Result<PathBuf> {
    if raw.trim().is_empty() {
        return Err(Error::Validation("path target cannot be empty".into()));
    }
    let raw_path = Path::new(raw);
    let canon_records = records
        .canonicalize()
        .map_err(|error| Error::Io(format!("{}: {error}", records.display())))?;
    let rel = if raw_path.is_absolute() {
        if let Ok(stripped) = raw_path.strip_prefix(records) {
            stripped.to_path_buf()
        } else if let Ok(stripped) = raw_path.strip_prefix(&canon_records) {
            stripped.to_path_buf()
        } else if let Ok(canon_raw) = raw_path.canonicalize()
            && let Ok(stripped) = canon_raw.strip_prefix(&canon_records)
        {
            stripped.to_path_buf()
        } else {
            return Err(Error::Validation(format!(
                "path '{}' lies outside records directory {}",
                raw_path.display(),
                records.display()
            )));
        }
    } else {
        raw_path.to_path_buf()
    };

    let mut clean = PathBuf::new();
    for component in rel.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                if part == ".git" {
                    return Err(Error::Validation(format!(
                        "path '{}' cannot reference .git",
                        raw
                    )));
                }
                clean.push(part);
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Validation(format!(
                    "path '{}' must be relative to records without '..' components",
                    raw
                )));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(Error::Validation(
            "path target cannot be '.' or the records root".into(),
        ));
    }
    let name = clean.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".tmp") || name.ends_with(".edit.md") {
        return Err(Error::Validation(format!(
            "path '{}' is a transient scratch file (.tmp or .edit.md) and cannot be committed",
            clean.display()
        )));
    }
    let joined = records.join(&clean);
    if joined.exists() {
        let canon_joined = joined
            .canonicalize()
            .map_err(|error| Error::Io(format!("{}: {error}", joined.display())))?;
        if !canon_joined.starts_with(&canon_records) {
            return Err(Error::Validation(format!(
                "path '{}' resolves outside records directory {}",
                clean.display(),
                records.display()
            )));
        }
    }
    Ok(clean)
}

pub fn open_and_run(
    dir: Option<&Path>,
    targets: Vec<String>,
    message: Option<String>,
    project: Option<String>,
    host: Arc<HostConfig>,
) -> Result<Output> {
    let message = match message {
        Some(msg) => {
            let trimmed = msg.trim().to_string();
            if trimmed.is_empty() {
                return Err(Error::Validation("commit message cannot be empty".into()));
            }
            Some(trimmed)
        }
        None => None,
    };

    let classified: Vec<TargetKind> = targets.iter().map(|t| classify_target(t)).collect();
    let all_full_ids = !classified.is_empty()
        && classified
            .iter()
            .all(|kind| matches!(kind, TargetKind::FullId(_)));

    let paths_only = classified
        .iter()
        .all(|kind| matches!(kind, TargetKind::Path(_)));
    let mut ctx = if paths_only {
        super::open_project_write_ctx_allow_retired(dir, project, host)?
    } else if project.is_some() {
        super::open_project_write_ctx(dir, project, host)?
    } else if all_full_ids {
        let registry = crate::registry::Registry::load()?;
        let mut distinct = BTreeSet::new();
        for kind in &classified {
            if let TargetKind::FullId(id) = kind {
                distinct.insert(registry.canonical_prefix(&id.prefix).to_string());
            }
        }
        if distinct.len() > 1 {
            let list: Vec<String> = distinct.into_iter().collect();
            return Err(Error::Validation(format!(
                "cannot commit task IDs from multiple projects in one command ({}); run `tasks commit` per project",
                list.join(", ")
            )));
        }
        super::open_id_write_ctx(dir, &targets[0], host)?
    } else {
        super::open_project_write_ctx(dir, None, host)?
    };

    // Normalize before granting the retired exception: directory targets and absolute
    // task paths must not bypass the same guard as IDs.
    for kind in &classified {
        if let TargetKind::Path(raw) = kind {
            let path = normalize_records_relpath(&ctx.project.records, raw)?;
            if path.starts_with("tasks") && path != Path::new(CONFIG_REL) {
                super::reject_retired(&ctx.registry, &ctx.project)?;
            }
        }
    }
    let receipt = if classified.is_empty() {
        Inventory::load_commit_receipt(&ctx.project.prefix)?
    } else {
        None
    };
    if classified.is_empty() && receipt.is_none() {
        super::reject_retired(&ctx.registry, &ctx.project)?;
    }
    let commit_dir = if let Some(inventory) = &receipt {
        validate_rename_receipt(&ctx.project, inventory)?;
        crate::record_commit::working_tree(&ctx.project.records)
            .map_err(Error::Validation)?
            .ok_or_else(|| Error::Validation("rename records are not in a git repository".into()))?
    } else {
        ctx.project.records.clone()
    };

    let Some(commit_lock) =
        crate::record_commit::try_lock(&commit_dir).map_err(Error::Validation)?
    else {
        return Err(Error::Validation(format!(
            "commit_unavailable: {} is not in a git repository (or git is missing)",
            ctx.project.records.display()
        )));
    };
    let recovery = commit_lock
        .recover(&commit_dir)
        .map_err(Error::Validation)?;
    ctx.warnings.extend(recovery.warnings);

    let (expanded_paths, subject) = if let Some(inventory) = &receipt {
        let files = rename_commit_paths(&ctx.project, inventory, &commit_dir)?;
        let subject = message.unwrap_or_else(|| {
            format!(
                "chore(tasks): rename {} -> {}",
                inventory.source, inventory.target
            )
        });
        (files, subject)
    } else if classified.is_empty() {
        let files = ctx.project.uncommitted_task_files()?.unwrap_or_default();
        let subject =
            message.unwrap_or_else(|| format!("chore(tasks): commit {} records", files.len()));
        (files, subject)
    } else {
        let mut expanded_set = BTreeSet::new();
        let mut task_ids = Vec::new();
        let mut seen_ids = BTreeSet::new();
        let mut has_path_target = false;

        for kind in classified {
            let resolved_kind = match kind {
                TargetKind::BareSuffix(hex) => {
                    if ctx.project.records.join(&hex).exists()
                        || is_tracked(&ctx.project.records, &hex)?
                    {
                        TargetKind::Path(hex)
                    } else {
                        let id = super::parse_id(&ctx.registry, &ctx.shorthand, &hex)?;
                        TargetKind::FullId(id)
                    }
                }
                other => other,
            };

            match resolved_kind {
                TargetKind::FullId(id) => {
                    let canonical = ctx.registry.canonical_prefix(&id.prefix).to_string();
                    if canonical != ctx.project.prefix {
                        return Err(Error::Validation(format!(
                            "task {id} belongs to project {:?}, not {:?}; cannot mix projects or combine cross-project task IDs with local paths",
                            canonical, ctx.project.prefix
                        )));
                    }
                    let id = TaskId {
                        prefix: canonical,
                        hex: id.hex,
                    };
                    let task_abs = ctx.project.task_path(&id);
                    let files_abs = crate::attachments::task_dir(&ctx.project, &id);
                    let task_rel = task_abs
                        .strip_prefix(&ctx.project.records)
                        .expect("task_path under records")
                        .display()
                        .to_string();
                    let files_rel = files_abs
                        .strip_prefix(&ctx.project.records)
                        .expect("task_dir under records")
                        .display()
                        .to_string();
                    let status = ctx
                        .project
                        .status_paths(&[&task_rel, &files_rel], false)?
                        .expect("under CommitLock");
                    if !task_abs.exists() && status.dirty.is_empty() {
                        return Err(Error::TaskNotFound(id.to_string()));
                    }
                    expanded_set.extend(status.dirty);
                    let id_str = id.to_string();
                    if seen_ids.insert(id_str.clone()) {
                        task_ids.push(id_str);
                    }
                }
                TargetKind::Path(raw) => {
                    has_path_target = true;
                    let relpath = normalize_records_relpath(&ctx.project.records, &raw)?;
                    let rel_str = relpath.display().to_string();
                    let status = ctx
                        .project
                        .status_paths(&[&rel_str], true)?
                        .expect("under CommitLock");
                    let tracked = is_tracked(&ctx.project.records, &rel_str)?;
                    if !status.ignored.is_empty() && status.dirty.is_empty() && !tracked {
                        return Err(Error::Validation(format!(
                            "path '{}' is ignored by git",
                            relpath.display()
                        )));
                    }
                    if status.dirty.is_empty()
                        && !ctx.project.records.join(&relpath).exists()
                        && !tracked
                    {
                        return Err(Error::Validation(format!(
                            "path '{}' does not exist under records",
                            relpath.display()
                        )));
                    }
                    expanded_set.extend(status.dirty);
                }
                TargetKind::BareSuffix(_) => unreachable!(),
            }
        }

        if has_path_target && message.is_none() {
            return Err(Error::Validation(
                "commit message (-m/--message) is required when committing path targets; omit -m only when committing task IDs or sweeping tasks/"
                    .into(),
            ));
        }
        let subject =
            message.unwrap_or_else(|| crate::record_commit::message("commit", &task_ids, None));
        let expanded_paths: Vec<String> = expanded_set.into_iter().collect();
        (expanded_paths, subject)
    };

    let paths: Vec<PathBuf> = expanded_paths.iter().map(PathBuf::from).collect();
    let left_uncommitted: Vec<PathBuf> = recovery
        .unstaged
        .into_iter()
        .filter(|p| !paths.contains(p))
        .collect();

    if paths.is_empty() {
        if receipt.is_some() {
            Inventory::remove_commit_receipt(&ctx.project.prefix)?;
        }
        ctx.warnings
            .extend(crate::record_commit::recovery_warning(&left_uncommitted));
        return Ok(Output::Commit(CommitOut {
            committed: 0,
            paths: Vec::new(),
            warnings: ctx.warnings,
        }));
    }

    let (refused, commit_warnings) = commit_lock.commit_paths(&commit_dir, &paths, &subject);
    ctx.warnings.extend(commit_warnings);
    ctx.warnings
        .extend(crate::record_commit::recovery_warning(&left_uncommitted));
    if !refused.is_empty() {
        return Err(Error::Validation(refused.join("\n")));
    }
    if receipt.is_some() {
        Inventory::remove_commit_receipt(&ctx.project.prefix)?;
    }
    Ok(Output::Commit(CommitOut {
        committed: expanded_paths.len(),
        paths: expanded_paths,
        warnings: ctx.warnings,
    }))
}

fn validate_rename_receipt(project: &Project, inventory: &Inventory) -> Result<()> {
    let records = inventory.records_to.as_ref().unwrap_or(&inventory.root);
    if records != &project.records || inventory.target != project.prefix {
        return Err(Error::Validation(
            "rename receipt does not match this project's records".into(),
        ));
    }
    verify_receipt_digest(&project.records.join(CONFIG_REL), &inventory.config_to)?;
    for entry in &inventory.entries {
        let target = project
            .tasks_dir()
            .join(format!("{}-{}.md", inventory.target, entry.hex));
        verify_receipt_digest(&target, &entry.to)?;
        let source = project
            .tasks_dir()
            .join(format!("{}-{}.md", inventory.source, entry.hex));
        if source.try_exists()? {
            return Err(Error::Validation(format!(
                "rename source file remains: {}",
                source.display()
            )));
        }
        if let Some(from) = &inventory.records_from {
            let source = from.join(format!("tasks/{}-{}.md", inventory.source, entry.hex));
            if source.try_exists()? {
                return Err(Error::Validation(format!(
                    "rename source file remains: {}",
                    source.display()
                )));
            }
        }
    }
    Ok(())
}

fn verify_receipt_digest(path: &Path, expected: &str) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|error| {
        Error::Validation(format!(
            "rename receipt cannot read {}: {error}",
            path.display()
        ))
    })?;
    if digest(&bytes) != expected {
        return Err(Error::Validation(format!(
            "rename receipt digest mismatch: {}",
            path.display()
        )));
    }
    Ok(())
}

fn rename_commit_paths(
    project: &Project,
    inventory: &Inventory,
    toplevel: &Path,
) -> Result<Vec<String>> {
    let roots = match (&inventory.records_from, &inventory.records_to) {
        (Some(from), Some(to)) => vec![from.clone(), to.clone()],
        _ => vec![project.tasks_dir()],
    };
    let pathspecs: Vec<String> = roots
        .iter()
        .map(|path| {
            path.strip_prefix(toplevel)
                .map(|relative| relative.display().to_string())
                .map_err(|_| {
                    Error::Validation(format!(
                        "rename path {} is outside {}",
                        path.display(),
                        toplevel.display()
                    ))
                })
        })
        .collect::<Result<_>>()?;
    let pathspecs: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
    let repository = Project {
        records: toplevel.to_path_buf(),
        ..project.clone()
    };
    Ok(repository
        .status_paths(&pathspecs, false)?
        .expect("under CommitLock")
        .dirty)
}
