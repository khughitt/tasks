use super::model::*;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::path::{Path, PathBuf};

pub struct PinnedTask {
    pub basis: Basis,
    pub task: Task,
    pub history_bytes_read: u64,
    pub warnings: Vec<String>,
}
pub fn git(project: &Project, args: &[&str]) -> Result<Vec<u8>> {
    let out = crate::git::at(&root(project)?)
        .arg("--literal-pathspecs")
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Git {} failed: {}; recover the exact records basis before relying on retained state",
            args.first().copied().unwrap_or("read"),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}
fn string(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| invalid("Git metadata must be UTF-8"))
}
fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| invalid("records-relative path must be UTF-8"))
}
pub fn root(project: &Project) -> Result<PathBuf> {
    crate::record_commit::working_tree(&project.records).map_err(Error::Io)?.ok_or_else(||Error::RetainedPublicationUnsupported("Retained state needs a Git-backed records basis; use the manual immutable-snapshot recovery route for non-Git records".into()))
}
pub fn resolve(project: &Project, revision: &str) -> Result<String> {
    if !line(revision) || revision.starts_with('-') {
        return Err(invalid("invalid revision"));
    }
    let object = format!("{revision}^{{commit}}");
    let value = string(git(
        project,
        &["rev-parse", "--verify", "--end-of-options", &object],
    )?)?;
    let value = value.trim();
    if value.len() != 40
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid("unsupported Git object format"));
    }
    Ok(value.into())
}
pub fn blob(
    project: &Project,
    commit: &str,
    path: &str,
    max: Option<u64>,
    bytes_read: &mut u64,
) -> Result<Vec<u8>> {
    if !member_path(path) {
        return Err(invalid("unsafe Git blob path"));
    }
    let tree = git(project, &["ls-tree", "-z", commit, "--", path])?;
    if !(tree.starts_with(b"100644 blob ") || tree.starts_with(b"100755 blob "))
        || tree.split(|b| *b == 0).filter(|v| !v.is_empty()).count() != 1
    {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Missing or unsafe pinned file {commit}:{path}; restore exact bytes"
        )));
    }
    let object = format!("{commit}:{path}");
    if let Some(cap) = max {
        let size = string(git(project, &["cat-file", "-s", &object])?)?
            .trim()
            .parse::<u64>()
            .map_err(invalid)?;
        if size > cap {
            return Err(Error::AttachmentTooLarge(format!(
                "pinned {path} exceeds attachment cap {cap}"
            )));
        }
    }
    let bytes = git(project, &["cat-file", "blob", &object])?;
    *bytes_read = bytes_read
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| invalid("history byte counter overflow"))?;
    Ok(bytes)
}
fn task_at(project: &Project, id: &TaskId, basis: &Basis, bytes_read: &mut u64) -> Result<Task> {
    let raw = string(blob(
        project,
        &basis.commit,
        &basis.task_path,
        None,
        bytes_read,
    )?)?;
    let physical = crate::format::parse_physical(&raw, &basis.task_path)?;
    let task = crate::history::load_logical(physical, |a| {
        let relative =
            Path::new(&basis.records_subdir).join(format!("tasks/files/{id}/{}", a.file_name()));
        let relative = relative.strip_prefix(".").unwrap_or(&relative);
        blob(
            project,
            &basis.commit,
            &path_string(relative)?,
            Some(project.attachments_max_bytes),
            bytes_read,
        )
    })?;
    if task.id != *id {
        return Err(Error::RetainedHistoryBlocked(
            "Pinned task has a different id; restore the declared pairing".into(),
        ));
    }
    Ok(task)
}
pub fn validate_locator(project: &Project, id: &TaskId, basis: &Basis) -> Result<()> {
    basis.validate()?;
    let root = root(project)?;
    let sub = project.records.strip_prefix(&root).map_err(invalid)?;
    let sub = if sub.as_os_str().is_empty() {
        ".".into()
    } else {
        path_string(sub)?
    };
    let task_path = path_string(project.task_path(id).strip_prefix(&root).map_err(invalid)?)?;
    if basis.project != project.prefix
        || basis.layout != project.kind.as_str()
        || basis.records_subdir != sub
        || basis.task_path != task_path
    {
        return Err(Error::RetainedHistoryBlocked("Records pairing/layout changed; record a verified locator for the same exact history before retry".into()));
    }
    let out = crate::git::at(&project.records)
        .args([
            "merge-base",
            "--is-ancestor",
            &basis.commit,
            &basis.retained_ref,
        ])
        .output()?;
    if !out.status.success() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Basis {} is not reachable through {}; preserve or recover its named ref",
            basis.commit, basis.retained_ref
        )));
    }
    Ok(())
}
pub fn read_exact(project: &Project, id: &TaskId, basis: &Basis) -> Result<PinnedTask> {
    validate_locator(project, id, basis)?;
    let mut bytes = 0;
    let task = task_at(project, id, basis, &mut bytes)?;
    Ok(PinnedTask {
        basis: basis.clone(),
        task,
        history_bytes_read: bytes,
        warnings: Vec::new(),
    })
}
pub fn read(project: &Project, id: &TaskId, at: Option<&str>) -> Result<PinnedTask> {
    let root = root(project)?;
    let head = resolve(project, "HEAD")?;
    let commit = resolve(project, at.unwrap_or("HEAD"))?;
    let refs = string(git(
        project,
        &[
            "for-each-ref",
            "--contains",
            &commit,
            "--format=%(refname)",
            "refs/heads/",
        ],
    )?)?;
    let current = crate::git::at(&project.records)
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()?;
    let current_ref = if current.status.success() {
        string(current.stdout)?.trim().to_string()
    } else {
        String::new()
    };
    let retained_ref=refs.lines().find(|r|*r==current_ref).or_else(||refs.lines().next()).ok_or_else(||Error::RetainedHistoryBlocked("No retained local branch reaches this basis; preserve the exact history on a named branch".into()))?.to_string();
    let sub = project.records.strip_prefix(&root).map_err(invalid)?;
    let basis = Basis {
        project: project.prefix.clone(),
        layout: project.kind.as_str().into(),
        records_subdir: if sub.as_os_str().is_empty() {
            ".".into()
        } else {
            path_string(sub)?
        },
        commit,
        task_path: path_string(project.task_path(id).strip_prefix(&root).map_err(invalid)?)?,
        retained_ref,
    };
    let mut pinned = read_exact(project, id, &basis)?;
    if at.is_none() {
        let paths = std::iter::once(basis.task_path.clone())
            .chain(pinned.task.note_archives.iter().map(|a| {
                format!(
                    "{}/tasks/files/{id}/{}",
                    basis.records_subdir,
                    a.file_name()
                )
            }))
            .collect::<Vec<_>>();
        let mut args = vec!["diff", "--quiet", "HEAD", "--"];
        args.extend(paths.iter().map(String::as_str));
        let out = crate::git::at(&root)
            .arg("--literal-pathspecs")
            .args(args)
            .output()?;
        if !out.status.success() {
            return Err(Error::RetainedStaleBasis("Task or required history has unpublished changes; publish them before querying current retained state, or explicitly select a historical --at basis".into()));
        }
    }
    // A correction must identify actual bad bytes, not merely an ordinal at a guessed commit.
    for (index, note) in pinned.task.notes.iter().enumerate() {
        if let Some(raw) = note.text.strip_prefix("retention operation: ")
            && let Ok(operation) = decode_operation(raw)
            && let PhaseOutputs::Items(output) = operation.outputs
        {
            for relation in output.resolutions {
                validate_locator(project, id, &relation.bad.basis)?;
                let original = task_at(
                    project,
                    id,
                    &relation.bad.basis,
                    &mut pinned.history_bytes_read,
                )?;
                let ordinal = usize::try_from(relation.bad.ordinal).map_err(invalid)?;
                let prefix = original
                    .notes
                    .get(..ordinal)
                    .filter(|_| ordinal > 0 && ordinal <= index)
                    .ok_or_else(|| {
                        Error::RetainedHistoryBlocked(
                            "Correction points outside its preceding source history".into(),
                        )
                    })?;
                if pinned.task.notes.get(..ordinal) != Some(prefix) {
                    return Err(Error::RetainedHistoryBlocked(
                        "Correction source bytes differ from their exact pinned history".into(),
                    ));
                }
            }
        }
    }
    // A machine checkpoint's basis is evidence, not an unverified prose count.
    for (index, note) in pinned.task.notes.iter().enumerate() {
        if let Some(raw) = note.text.strip_prefix("retention checkpoint: {") {
            let raw = format!("{{{raw}");
            if let Ok(cp) = decode_checkpoint(&raw)
                && cp.complete
            {
                let old = cp.basis.as_ref().expect("complete checkpoint basis");
                validate_locator(project, id, old)?;
                let old_task = task_at(project, id, old, &mut pinned.history_bytes_read)?;
                if old_task.notes.len() > index
                    || pinned.task.notes.get(..old_task.notes.len())
                        != Some(old_task.notes.as_slice())
                {
                    return Err(Error::RetainedHistoryBlocked("Checkpoint basis is not the exact preceding logical history; reconcile evidence before use".into()));
                }
                if pinned.task.notes[old_task.notes.len()..index]
                    .iter()
                    .any(|n| {
                        [
                            "retained: ",
                            "attached: ",
                            "detached: ",
                            "retention operation: ",
                        ]
                        .iter()
                        .any(|p| n.text.starts_with(p))
                    })
                {
                    return Err(Error::RetainedHistoryBlocked("Retention/evidence changed between checkpoint basis and publication; reconcile that checkpoint before use".into()));
                }
                let state = super::state::replay(&old_task, old, Some(&cp.host))?;
                let counts = Counts {
                    kept_verified: state
                        .items
                        .iter()
                        .filter(|i| {
                            i.record
                                .as_ref()
                                .is_some_and(|r| r.state == "kept" && r.verification == "verified")
                        })
                        .count() as u64,
                    kept_unknown: state
                        .items
                        .iter()
                        .filter(|i| {
                            i.record
                                .as_ref()
                                .is_some_and(|r| r.state == "kept" && r.verification == "unknown")
                        })
                        .count() as u64,
                    released: state
                        .items
                        .iter()
                        .filter(|i| i.record.as_ref().is_some_and(|r| r.state == "released"))
                        .count() as u64,
                };
                if !state.complete || cp.counts.as_ref() != Some(&counts) {
                    return Err(Error::RetainedHistoryBlocked(
                        "Checkpoint counts/scope disagree with its exact basis".into(),
                    ));
                }
            }
        }
    }
    let ancestor = crate::git::at(&project.records)
        .args(["merge-base", "--is-ancestor", &basis.commit, &head])
        .output()?;
    if !ancestor.status.success() {
        pinned.warnings.push("Historical basis is outside current HEAD ancestry; it remains reachable through its named retained ref".into())
    }
    if at.is_none() && resolve(project, "HEAD")? != head {
        return Err(Error::RetainedStaleBasis(
            "Records HEAD changed during pinned read; retry".into(),
        ));
    }
    Ok(pinned)
}

pub fn publication_basis(
    project: &Project,
    id: &TaskId,
    context: &Basis,
    prefix: &[crate::model::Note],
    bytes_read: &mut u64,
) -> Result<Basis> {
    if prefix.is_empty() {
        return Err(invalid("publication source requires a logical note"));
    }
    validate_locator(project, id, context)?;
    let commits = string(git(
        project,
        &[
            "log",
            "--first-parent",
            "--reverse",
            "--format=%H",
            &context.retained_ref,
            "--",
            &context.task_path,
        ],
    )?)?;
    let commits = commits.lines().collect::<Vec<_>>();
    let mut low = 0;
    let mut high = commits.len();
    // Operation history is append-only; archival preserves this logical prefix.
    while low < high {
        let mid = low + (high - low) / 2;
        let mut candidate = context.clone();
        candidate.commit = commits[mid].into();
        let task = task_at(project, id, &candidate, bytes_read)?;
        if task.notes.starts_with(prefix) {
            high = mid
        } else {
            low = mid + 1
        }
    }
    let commit = commits.get(low).ok_or_else(|| {
        Error::RetainedHistoryBlocked(
            "Original publication is unavailable on its retained branch; recover exact history"
                .into(),
        )
    })?;
    let mut result = context.clone();
    result.commit = (*commit).into();
    if !task_at(project, id, &result, bytes_read)?
        .notes
        .starts_with(prefix)
    {
        return Err(invalid("original publication prefix does not match"));
    }
    Ok(result)
}
