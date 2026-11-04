use super::catalog::Catalog;
use super::content::{builtin, sha256_hex};
use super::declaration::{Declaration, parse_declaration_note};
use super::git_objects::Revision;
use super::names::{catalog_name, member_names};
use super::{RECOVERY_MAX_BYTES, RECOVERY_MAX_PARTS};
use crate::attachments::{self, EntryState, Ledger, Problem, Severity};
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::repo::Project;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclState {
    Live(Declaration),
    Closed(Declaration),
}

impl DeclState {
    fn declaration(&self) -> &Declaration {
        match self {
            Self::Live(declaration) | Self::Closed(declaration) => declaration,
        }
    }
}

pub fn declarations(task: &Task) -> BTreeMap<String, DeclState> {
    let mut state = BTreeMap::new();
    for note in &task.notes {
        if let Some(declaration) = parse_declaration_note(&note.text) {
            state.insert(declaration.name.clone(), DeclState::Live(declaration));
        } else if let Some((Ledger::Detached, name)) = attachments::parse_ledger_note(&note.text)
            && let Some(entry) = state.get_mut(name)
        {
            *entry = DeclState::Closed(entry.declaration().clone());
        }
    }
    state
}

pub fn reserved_names(task: &Task) -> BTreeMap<String, String> {
    let mut reserved = BTreeMap::new();
    for (name, state) in declarations(task) {
        reserved.insert(name.clone(), name.clone());
        let declaration = state.declaration();
        if recovery_bounds(declaration).is_ok() {
            for member in member_names(&name, declaration.parts) {
                reserved.insert(member, name.clone());
            }
        }
    }
    reserved
}

/// The first closure after the latest declaration, before any ordinary name reuse.
fn closing_ordinal(task: &Task, name: &str) -> Option<usize> {
    let declaration = task.notes.iter().rposition(|note| {
        parse_declaration_note(&note.text).is_some_and(|decl| decl.name == name)
    })?;
    task.notes
        .iter()
        .enumerate()
        .skip(declaration + 1)
        .find_map(|(index, note)| {
            (attachments::parse_ledger_note(&note.text) == Some((Ledger::Detached, name)))
                .then_some(index)
        })
}

pub fn later_owned(task: &Task, logical: &str, member: &str) -> bool {
    closing_ordinal(task, logical).is_some_and(|closed| {
        task.notes.iter().skip(closed + 1).any(|note| {
            attachments::parse_ledger_note(&note.text).is_some_and(|(_, name)| name == member)
        })
    })
}

pub fn removal_members(task: &Task, decl: &Declaration) -> Result<Vec<String>> {
    recovery_bounds(decl)?;
    let ledger = attachments::ledger(task);
    Ok(member_names(&decl.name, decl.parts)
        .into_iter()
        .filter(|member| {
            !later_owned(task, &decl.name, member) && ledger.get(member) == Some(&Ledger::Detached)
        })
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
pub enum RemovalState {
    FilesRemain,
    Uncommitted,
    Complete { commit: String },
}

pub fn removal_state(project: &Project, task: &Task, decl: &Declaration) -> Result<RemovalState> {
    recovery_bounds(decl)?;
    attachments::check_storage(project, &task.id)?;
    let dir = attachments::task_dir(project, &task.id);
    for member in member_names(&decl.name, decl.parts) {
        if !later_owned(task, &decl.name, &member)
            && attachments::entry_state(&dir.join(member))? != EntryState::Absent
        {
            return Ok(RemovalState::FilesRemain);
        }
    }
    if crate::record_commit::working_tree(&project.records)
        .map_err(Error::Validation)?
        .is_none()
    {
        return Ok(RemovalState::Uncommitted);
    }
    Ok(match removal_at(project, task, decl, "HEAD")? {
        Some(commit) => RemovalState::Complete { commit },
        None => RemovalState::Uncommitted,
    })
}

/// Checks this closure's exact history and remaining ownership at one immutable basis.
pub fn removal_at(
    project: &Project,
    task: &Task,
    decl: &Declaration,
    commit: &str,
) -> Result<Option<String>> {
    recovery_bounds(decl)?;
    let Some(closed) = closing_ordinal(task, &decl.name) else {
        return Ok(None);
    };
    let revision = Revision::resolve(&project.records, commit)?;
    let Some(published) = revision.task_at(project, &task.id)? else {
        return Ok(None);
    };
    if declarations(&published).get(&decl.name) != Some(&DeclState::Closed(decl.clone()))
        || published.notes.get(..=closed) != task.notes.get(..=closed)
        || closing_ordinal(&published, &decl.name) != Some(closed)
    {
        return Ok(None);
    }
    let toplevel = crate::record_commit::working_tree(&project.records)
        .map_err(Error::Validation)?
        .ok_or_else(|| Error::Validation("records are outside a Git work tree".into()))?;
    let dir = attachments::task_dir(project, &task.id);
    let relative = dir
        .strip_prefix(toplevel)
        .map_err(|error| Error::Validation(error.to_string()))?;
    for member in member_names(&decl.name, decl.parts) {
        if !later_owned(&published, &decl.name, &member)
            && revision
                .read(&relative.join(member), super::RECOVERY_MAX_BYTES)?
                .is_some()
        {
            return Ok(None);
        }
    }
    Ok(Some(revision.commit().to_string()))
}

pub fn removal_next(project: &Project, task: &Task, decl: &Declaration) -> String {
    let retry = format!("tasks detach {} {} \"<why>\"", task.id, decl.name);
    match crate::record_commit::working_tree(&project.records) {
        Ok(None) => format!(
            "put the records directory in a Git work tree, then run tasks commit and rerun {retry}; reuse stays blocked until removal is published"
        ),
        Err(error) => format!("resolve Git discovery failure ({error}), then rerun {retry}"),
        Ok(Some(_)) => format!(
            "rerun {retry}; if commit_disabled or commit_deferred, run tasks commit or clear the deferring cause, then rerun"
        ),
    }
}

fn invalid(declaration: &Declaration, member: &str, detail: impl std::fmt::Display) -> Error {
    Error::MultipartInvalidContent(format!(
        "multipart artifact {} member {member}: {detail}",
        declaration.name
    ))
}

pub(crate) fn recovery_bounds(declaration: &Declaration) -> Result<()> {
    if declaration.bytes > RECOVERY_MAX_BYTES || declaration.parts > RECOVERY_MAX_PARTS {
        return Err(invalid(
            declaration,
            &declaration.name,
            format!(
                "declaration exceeds recovery ceilings ({RECOVERY_MAX_BYTES} bytes, {RECOVERY_MAX_PARTS} parts)"
            ),
        ));
    }
    Ok(())
}

pub fn is_member(name: &str, logical: &str) -> bool {
    name == catalog_name(logical)
        || name
            .strip_prefix(logical)
            .and_then(|tail| tail.strip_prefix(".part-"))
            .is_some_and(|suffix| {
                suffix.len() == 4 && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
}

/// Rebuilds and validates one artifact; the loader reads at most the supplied cap.
/// Recovery uses compiled ceilings, independent of current admission settings.
pub fn rebuild_with(
    declaration: &Declaration,
    per_file_cap: u64,
    ledger: &BTreeMap<String, Ledger>,
    mut load: impl FnMut(&str, u64) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    recovery_bounds(declaration)?;
    if ledger.get(&declaration.name) == Some(&Ledger::Attached) {
        return Err(invalid(
            declaration,
            &declaration.name,
            "logical name is also an ordinary attachment",
        ));
    }
    let members = member_names(&declaration.name, declaration.parts);
    for (name, entry) in ledger {
        if *entry == Ledger::Attached
            && is_member(name, &declaration.name)
            && !members.contains(name)
        {
            return Err(invalid(declaration, name, "extra live member"));
        }
    }
    let cap = per_file_cap.min(RECOVERY_MAX_BYTES);
    let mut read = |name: &str| -> Result<Vec<u8>> {
        if ledger.get(name) != Some(&Ledger::Attached) {
            return Err(invalid(
                declaration,
                name,
                "member is not live in the attachment ledger",
            ));
        }
        let bytes = load(name, cap).map_err(|error| invalid(declaration, name, error))?;
        if bytes.len() as u64 > cap {
            return Err(invalid(declaration, name, "member exceeds per-file cap"));
        }
        Ok(bytes)
    };
    let catalog_member = catalog_name(&declaration.name);
    let bytes = read(&catalog_member)?;
    if !sha256_hex(&bytes).eq_ignore_ascii_case(&declaration.catalog_sha256) {
        return Err(invalid(
            declaration,
            &catalog_member,
            "catalog sha256 mismatch",
        ));
    }
    let catalog =
        Catalog::parse(&bytes).map_err(|error| invalid(declaration, &catalog_member, error))?;
    if catalog.name != declaration.name
        || catalog.bytes != declaration.bytes
        || catalog.parts.len() != declaration.parts
        || !catalog.sha256.eq_ignore_ascii_case(&declaration.sha256)
    {
        return Err(invalid(
            declaration,
            &catalog_member,
            "catalog does not match declaration",
        ));
    }
    let mut total = 0u64;
    for part in &catalog.parts {
        if part.bytes > cap {
            return Err(invalid(
                declaration,
                &part.name,
                "catalog member exceeds per-file cap",
            ));
        }
        total = total
            .checked_add(part.bytes)
            .filter(|total| *total <= declaration.bytes)
            .ok_or_else(|| {
                invalid(
                    declaration,
                    &part.name,
                    "catalog byte total exceeds declaration",
                )
            })?;
    }
    if total != declaration.bytes {
        return Err(invalid(
            declaration,
            &catalog_member,
            "catalog byte total does not match declaration",
        ));
    }
    let mut whole = Vec::new();
    for part in &catalog.parts {
        let bytes = read(&part.name)?;
        if bytes.len() as u64 != part.bytes
            || !sha256_hex(&bytes).eq_ignore_ascii_case(&part.sha256)
        {
            return Err(invalid(
                declaration,
                &part.name,
                "part size or sha256 mismatch",
            ));
        }
        whole.extend_from_slice(&bytes);
    }
    if whole.len() as u64 != declaration.bytes
        || !sha256_hex(&whole).eq_ignore_ascii_case(&declaration.sha256)
    {
        return Err(invalid(
            declaration,
            &declaration.name,
            "whole size or sha256 mismatch",
        ));
    }
    builtin(&declaration.name, &whole)
        .map_err(|error| invalid(declaration, &declaration.name, error))?;
    Ok(whole)
}

/// Reads one live attachment at the selected basis, without running declared validators.
pub fn rebuild(
    project: &Project,
    task: &Task,
    name: &str,
    revision: Option<&Revision>,
) -> Result<Vec<u8>> {
    attachments::validate_name(name)?;
    let ledger = attachments::ledger(task);
    let dir = attachments::task_dir(project, &task.id);
    let relative = if revision.is_some() {
        let root = crate::record_commit::working_tree(&project.records)
            .map_err(Error::Validation)?
            .ok_or_else(|| Error::Validation("records are outside a Git work tree".into()))?;
        Some(
            dir.strip_prefix(root)
                .map_err(|error| Error::Validation(error.to_string()))?
                .to_path_buf(),
        )
    } else {
        None
    };
    let load = |member: &str, cap| {
        if let Some(revision) = revision {
            return revision
                .read(&relative.as_ref().unwrap().join(member), cap)?
                .ok_or_else(|| {
                    Error::AttachmentMissing(format!(
                        "{member} is missing at {}",
                        revision.commit()
                    ))
                });
        }
        attachments::check_storage(project, &task.id)?;
        let path = dir.join(member);
        match attachments::entry_state(&path)? {
            EntryState::File(_) => {
                attachments::read_capped(std::fs::File::open(path)?, cap, member)
            }
            EntryState::Absent => Err(Error::AttachmentMissing(format!("{member} is missing"))),
            EntryState::Unsafe(detail) => Err(Error::AttachmentUnsafe(detail)),
        }
    };
    match declarations(task).remove(name) {
        Some(DeclState::Live(declaration)) => {
            recovery_bounds(&declaration)?;
            let members = member_names(name, declaration.parts);
            let check_member = |member: &str| {
                if is_member(member, name) && !members.iter().any(|expected| expected == member) {
                    return Err(invalid(
                        &declaration,
                        member,
                        "extra member in attachment directory",
                    ));
                }
                Ok(())
            };
            if let Some(revision) = revision {
                revision
                    .verify_members(relative.as_ref().unwrap(), name, &members)
                    .map_err(|error| invalid(&declaration, name, error))?;
            } else if attachments::check_storage(project, &task.id)? {
                for entry in std::fs::read_dir(&dir)? {
                    let entry = entry?;
                    if let Some(member) = entry.file_name().to_str() {
                        check_member(member)?;
                    }
                }
            }
            rebuild_with(&declaration, project.attachments_max_bytes, &ledger, load)
        }
        Some(DeclState::Closed(_)) if !later_owned(task, name, name) => {
            Err(Error::AttachmentMissing(format!("{name} is detached")))
        }
        _ if ledger.get(name) == Some(&Ledger::Attached) => load(name, super::RECOVERY_MAX_BYTES),
        _ => Err(Error::AttachmentMissing(format!(
            "{name} is not a live attachment"
        ))),
    }
}

pub fn audit(project: &Project, task: &Task) -> Result<Vec<Problem>> {
    let states = declarations(task);
    let ledger = attachments::ledger(task);
    let dir = attachments::task_dir(project, &task.id);
    let mut problems = Vec::new();
    let problem = |kind, member: &str, detail| Problem {
        severity: Severity::Error,
        kind,
        id: Some(task.id.clone()),
        file: format!("tasks/files/{}/{member}", task.id),
        detail,
    };
    if let Err(error) = crate::refs::refuse_members(project, task) {
        problems.push(problem("multipart_broken", "", error.to_string()));
    }
    for (name, state) in states {
        let declaration = state.declaration();
        if let Err(error) = recovery_bounds(declaration) {
            problems.push(problem("multipart_broken", &name, error.to_string()));
            continue;
        }
        let members = member_names(&name, declaration.parts);
        if task.status == Status::Moved
            && let Some(to) = &task.moved_to
            && members.iter().all(|member| {
                task.notes
                    .iter()
                    .rev()
                    .find(|note| {
                        attachments::parse_ledger_note(&note.text)
                            .is_some_and(|(_, name)| name == member)
                    })
                    .is_some_and(|note| {
                        note.text == attachments::detached_note(member, &format!("moved to {to}"))
                    })
            })
        {
            continue;
        }
        if matches!(state, DeclState::Closed(_)) {
            for member in members {
                if !later_owned(task, &name, &member)
                    && attachments::entry_state(&dir.join(&member))? != EntryState::Absent
                {
                    problems.push(problem("multipart_leftover", &member, format!(
                        "closed multipart artifact {name} still has member {member}; rerun tasks detach {} {name} \"<why>\"", task.id
                    )));
                }
            }
            let detail = match removal_state(project, task, declaration) {
                Ok(RemovalState::Uncommitted) => Some(removal_next(project, task, declaration)),
                Err(error) => Some(format!(
                    "{error}; {}",
                    removal_next(project, task, declaration)
                )),
                _ => None,
            };
            if let Some(detail) = detail {
                problems.push(Problem {
                    severity: Severity::Warning,
                    ..problem("multipart_removal_unpublished", &name, detail)
                });
            }
            continue;
        }
        if attachments::dir_state(&dir)? == attachments::DirState::Directory {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                if let Some(member) = entry.file_name().to_str()
                    && is_member(member, &name)
                    && !members.iter().any(|expected| expected == member)
                {
                    problems.push(problem(
                        "multipart_broken",
                        member,
                        invalid(declaration, member, "extra member on disk").to_string(),
                    ));
                }
            }
        }
        let rebuilt = rebuild_with(
            declaration,
            project.attachments_max_bytes,
            &ledger,
            |member, cap| {
                attachments::check_storage(project, &task.id)?;
                let path = dir.join(member);
                match attachments::entry_state(&path)? {
                    EntryState::File(_) => {
                        attachments::read_capped(std::fs::File::open(&path)?, cap, member)
                    }
                    EntryState::Absent => {
                        Err(Error::AttachmentMissing(format!("{member} is missing")))
                    }
                    EntryState::Unsafe(detail) => Err(Error::AttachmentUnsafe(detail)),
                }
            },
        );
        if let Err(error) = rebuilt {
            problems.push(problem("multipart_broken", &name, error.to_string()));
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservations_survive_closure_and_declarations_follow_note_order() {
        let declaration = Declaration {
            name: "evidence.txt".into(),
            bytes: 6,
            parts: 1,
            sha256: "a".repeat(64),
            catalog_sha256: "b".repeat(64),
            caption: None,
        };
        let text = String::from(
            "---\nid: tst-000001\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-11-03T00:00:00Z\nupdated: 2026-11-03T00:00:00Z\ndepends: []\ntags: []\n---\n",
        );
        let mut task = crate::history::fixture(&text, "tasks/tst-000001.md").unwrap();
        for note in [
            declaration.note(),
            attachments::detached_note("evidence.txt", "obsolete"),
        ] {
            crate::commands::append_note(&mut task, "test", &note).unwrap();
        }
        assert_eq!(
            declarations(&task).get("evidence.txt"),
            Some(&DeclState::Closed(declaration.clone()))
        );
        assert_eq!(
            reserved_names(&task),
            BTreeMap::from([
                ("evidence.txt".into(), "evidence.txt".into()),
                ("evidence.txt.part-0001".into(), "evidence.txt".into()),
                ("evidence.txt.catalog.json".into(), "evidence.txt".into()),
            ])
        );
        crate::commands::append_note(&mut task, "test", &declaration.note()).unwrap();
        assert_eq!(
            declarations(&task).get("evidence.txt"),
            Some(&DeclState::Live(declaration))
        );
    }

    #[test]
    fn recovery_ceiling_rejects_before_loading_any_member() {
        for (bytes, parts) in [(RECOVERY_MAX_BYTES + 1, 1), (1, usize::MAX)] {
            let declaration = Declaration {
                name: "evidence.txt".into(),
                bytes,
                parts,
                sha256: "a".repeat(64),
                catalog_sha256: "b".repeat(64),
                caption: None,
            };
            let error = rebuild_with(&declaration, 1024, &BTreeMap::new(), |_, _| {
                panic!("must not read")
            })
            .unwrap_err();
            assert_eq!(error.kind(), "multipart_invalid_content");
        }
    }
}
