//! Two-phase retained artifact staging, multipart publication, and pre-save rollback.

use super::{model::invalid, publish::NewArtifact};
use crate::{
    attachments::{self, EntryState, Ledger},
    error::{Error, Result},
    model::{Task, TaskId},
    multipart::{
        self,
        audit::DeclState,
        catalog::{Catalog, CatalogPart},
        content::sha256_hex,
        cut::PartSlice,
        declaration::Declaration,
    },
    repo::Project,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedFile {
    pub name: String,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub create_new: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PreparedBatch {
    pub files: Vec<PlannedFile>,
    pub ledger_notes: Vec<String>,
    pub multipart_artifacts: Vec<(String, Vec<u8>)>,
    pub single_artifacts: Vec<(String, Vec<u8>)>,
}

fn is_manifest_name(name: &str) -> bool {
    name.starts_with("retained-manifest-") && name.ends_with(".json")
}

fn build_catalog(name: &str, bytes: &[u8], sha256: &str, ranges: &[PartSlice]) -> Catalog {
    let parts = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| CatalogPart {
            name: multipart::names::part_name(name, index + 1),
            bytes: range.len() as u64,
            sha256: sha256_hex(&bytes[range.clone()]),
        })
        .collect();
    Catalog {
        name: name.to_string(),
        bytes: bytes.len() as u64,
        sha256: sha256.to_string(),
        parts,
    }
}

pub(crate) fn is_live_attachment(task: &Task, name: &str, expected_sha256: &str) -> bool {
    attachments::ledger(task).get(name) == Some(&Ledger::Attached)
        || matches!(
            multipart::audit::declarations(task).get(name),
            Some(DeclState::Live(decl)) if decl.sha256 == expected_sha256
        )
}

pub(crate) fn artifact_note_count(
    project: &Project,
    task: &Task,
    artifact: &NewArtifact,
) -> Result<usize> {
    let digest = sha256_hex(&artifact.bytes);
    if is_live_attachment(task, &artifact.name, &digest) {
        return Ok(0);
    }
    if artifact.bytes.len() as u64 > project.attachments_max_bytes
        && is_manifest_name(&artifact.name)
    {
        return Ok(multipart::cut::cut(&artifact.bytes, project.attachments_max_bytes)?.len() + 2);
    }
    Ok(1)
}

pub(crate) fn expand_allowed_files(
    project: &Project,
    artifacts: &[NewArtifact],
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut allowed = BTreeMap::new();
    for artifact in artifacts {
        attachments::validate_name(&artifact.name)?;
        if artifact.bytes.len() as u64 <= project.attachments_max_bytes {
            allowed.insert(artifact.name.clone(), artifact.bytes.clone());
        } else if is_manifest_name(&artifact.name)
            && multipart::names::validate_logical_name(&artifact.name).is_ok()
            && let Ok(ranges) = multipart::cut::cut(&artifact.bytes, project.attachments_max_bytes)
        {
            let sha = sha256_hex(&artifact.bytes);
            let catalog_bytes =
                build_catalog(&artifact.name, &artifact.bytes, &sha, &ranges).to_bytes();
            if catalog_bytes.len() as u64 <= project.attachments_max_bytes {
                for (index, range) in ranges.iter().enumerate() {
                    allowed.insert(
                        multipart::names::part_name(&artifact.name, index + 1),
                        artifact.bytes[range.clone()].to_vec(),
                    );
                }
                allowed.insert(
                    multipart::names::catalog_name(&artifact.name),
                    catalog_bytes,
                );
            }
        }
    }
    Ok(allowed)
}

pub(crate) fn prepare_batch(
    project: &Project,
    task: &Task,
    artifacts: &[NewArtifact],
) -> Result<PreparedBatch> {
    let storage_exists = attachments::check_storage(project, &task.id)?;
    let ledger = attachments::ledger(task);
    let declarations = multipart::audit::declarations(task);
    let task_dir = attachments::task_dir(project, &task.id);
    let mut batch = PreparedBatch::default();
    let mut planned_names = BTreeSet::new();

    for artifact in artifacts {
        attachments::validate_name(&artifact.name)?;
        let artifact_sha = sha256_hex(&artifact.bytes);

        // 1. Reused live multipart manifest (`DeclState::Live` with matching sha256).
        if let Some(DeclState::Live(decl)) = declarations.get(&artifact.name)
            && decl.sha256 == artifact_sha
        {
            let rebuilt = multipart::audit::rebuild(project, task, &artifact.name, None)?;
            if rebuilt != artifact.bytes {
                return Err(invalid(
                    "immutable retained artifact name already holds different bytes",
                ));
            }
            let cap = project
                .attachments_max_bytes
                .min(multipart::RECOVERY_MAX_BYTES);
            for member in multipart::names::member_names(&artifact.name, decl.parts) {
                if !planned_names.insert(member.clone()) {
                    return Err(invalid("duplicate retained artifact target in batch"));
                }
                let path = task_dir.join(&member);
                let bytes = super::verify::read_local(&path, cap)?;
                batch.files.push(PlannedFile {
                    name: member,
                    path,
                    bytes,
                    create_new: false,
                });
            }
            batch
                .multipart_artifacts
                .push((artifact.name.clone(), artifact.bytes.clone()));
            continue;
        }

        // 2. Single-file artifact (`<= project.attachments_max_bytes`).
        if artifact.bytes.len() as u64 <= project.attachments_max_bytes {
            attachments::refuse_reserved(project, task, &artifact.name)?;
            if ledger.get(&artifact.name) == Some(&Ledger::Detached) {
                return Err(invalid(
                    "retained artifact name has a detached history; choose a fresh name before publishing",
                ));
            }
            if !planned_names.insert(artifact.name.clone()) {
                return Err(invalid("duplicate retained artifact target in batch"));
            }
            let path = task_dir.join(&artifact.name);
            let already_attached = ledger.get(&artifact.name) == Some(&Ledger::Attached);
            let create_new = match attachments::entry_state(&path)? {
                EntryState::Absent => {
                    if already_attached {
                        return Err(Error::AttachmentMissing(format!(
                            "required live attachment {} is missing on disk",
                            artifact.name
                        )));
                    }
                    true
                }
                EntryState::File(_) => {
                    if super::verify::read_local(&path, project.attachments_max_bytes)?
                        != artifact.bytes
                    {
                        return Err(invalid(
                            "immutable retained artifact name already holds different bytes",
                        ));
                    }
                    false
                }
                EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
            };
            if !already_attached {
                let note =
                    attachments::attached_note(&artifact.name, artifact.bytes.len() as u64, None);
                crate::format::validate_note_text(&note)?;
                batch.ledger_notes.push(note);
            }
            batch.files.push(PlannedFile {
                name: artifact.name.clone(),
                path,
                bytes: artifact.bytes.clone(),
                create_new,
            });
            batch
                .single_artifacts
                .push((artifact.name.clone(), artifact.bytes.clone()));
            continue;
        }

        // 3. Over-cap non-manifest artifact.
        if !is_manifest_name(&artifact.name) {
            return Err(Error::AttachmentTooLarge(
                "Retained artifact exceeds the configured cap; preserve raw bytes and use the labelled portable fallback before retry".into(),
            ));
        }

        // 4. New multipart manifest (`> project.attachments_max_bytes`).
        let ranges = super::verify::validate_multipart_manifest_limits(
            project,
            &artifact.name,
            &artifact.bytes,
        )?;
        attachments::refuse_reserved(project, task, &artifact.name)?;
        match ledger.get(&artifact.name) {
            Some(Ledger::Attached) => {
                return Err(Error::AttachmentExists(format!(
                    "{} already has {}",
                    task.id, artifact.name
                )));
            }
            Some(Ledger::Detached) => {
                return Err(invalid(
                    "retained artifact name has a detached history; choose a fresh name before publishing",
                ));
            }
            None => {}
        }
        match attachments::entry_state(&task_dir.join(&artifact.name))? {
            EntryState::Absent => {}
            EntryState::File(_) => {
                return Err(Error::AttachmentExists(format!(
                    "{} already holds {}; detach it or choose another --name",
                    task.id, artifact.name
                )));
            }
            EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        }

        let catalog = build_catalog(&artifact.name, &artifact.bytes, &artifact_sha, &ranges);
        let catalog_bytes = catalog.to_bytes();
        if catalog_bytes.len() as u64 > project.attachments_max_bytes {
            return Err(Error::AttachmentTooLarge(format!(
                "Manifest {}.catalog.json is {} bytes (exceeds {} byte per-file attachment cap in {}/tasks/.config.toml)",
                artifact.name,
                catalog_bytes.len(),
                project.attachments_max_bytes,
                project.prefix
            )));
        }
        let declaration = Declaration {
            name: artifact.name.clone(),
            bytes: artifact.bytes.len() as u64,
            parts: ranges.len(),
            sha256: catalog.sha256,
            catalog_sha256: sha256_hex(&catalog_bytes),
            caption: None,
        };

        let expected_members = multipart::names::member_names(&artifact.name, ranges.len());
        for (existing_member, state) in &ledger {
            if *state == Ledger::Attached
                && multipart::audit::is_member(existing_member, &artifact.name)
                && !expected_members.contains(existing_member)
            {
                return Err(Error::AttachmentExists(format!(
                    "{} already holds extra member {existing_member} of {}",
                    task.id, artifact.name
                )));
            }
        }
        if storage_exists {
            for entry in std::fs::read_dir(&task_dir)? {
                if let Some(existing_member) = entry?.file_name().to_str()
                    && multipart::audit::is_member(existing_member, &artifact.name)
                    && !expected_members.iter().any(|m| m == existing_member)
                {
                    return Err(Error::AttachmentExists(format!(
                        "{} already holds extra member {existing_member} of {}",
                        task.id, artifact.name
                    )));
                }
            }
        }

        let mut members: Vec<(String, Vec<u8>, String)> = ranges
            .iter()
            .enumerate()
            .map(|(index, range)| {
                (
                    multipart::names::part_name(&artifact.name, index + 1),
                    artifact.bytes[range.clone()].to_vec(),
                    format!("part {}/{} of {}", index + 1, ranges.len(), artifact.name),
                )
            })
            .collect();
        members.push((
            multipart::names::catalog_name(&artifact.name),
            catalog_bytes,
            format!("catalog of {}", artifact.name),
        ));

        for (member_name, member_bytes, caption) in members {
            attachments::validate_name(&member_name)?;
            attachments::refuse_reserved(project, task, &member_name)?;
            match ledger.get(&member_name) {
                Some(Ledger::Detached) => {
                    return Err(invalid(
                        "retained artifact name has a detached history; choose a fresh name before publishing",
                    ));
                }
                Some(Ledger::Attached) => {
                    return Err(Error::AttachmentExists(format!(
                        "{} already has {member_name}",
                        task.id
                    )));
                }
                None => {}
            }
            if !planned_names.insert(member_name.clone()) {
                return Err(invalid("duplicate retained artifact target in batch"));
            }
            let member_path = task_dir.join(&member_name);
            let create_new = match attachments::entry_state(&member_path)? {
                EntryState::Absent => true,
                EntryState::File(_) => {
                    if super::verify::read_local(&member_path, project.attachments_max_bytes)?
                        != member_bytes
                    {
                        return Err(invalid(
                            "immutable retained artifact name already holds different bytes",
                        ));
                    }
                    false
                }
                EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
            };
            let note =
                attachments::attached_note(&member_name, member_bytes.len() as u64, Some(&caption));
            crate::format::validate_note_text(&note)?;
            batch.ledger_notes.push(note);
            batch.files.push(PlannedFile {
                name: member_name,
                path: member_path,
                bytes: member_bytes,
                create_new,
            });
        }

        let decl_note = declaration.note();
        crate::format::validate_note_text(&decl_note)?;
        batch.ledger_notes.push(decl_note);
        batch
            .multipart_artifacts
            .push((artifact.name.clone(), artifact.bytes.clone()));
    }

    if !batch.ledger_notes.is_empty() && !task.refs.is_empty() {
        let mut preview = task.clone();
        for note in &batch.ledger_notes {
            crate::commands::append_note(&mut preview, "retained-preview", note)?;
        }
        crate::refs::refuse_members(project, &preview)?;
    }

    Ok(batch)
}

#[cfg(test)]
thread_local! {
    static FORCE_FAIL_AFTER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static FORCE_REMOVE_FILE_ERROR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static FORCE_REMOVE_DIR_ERROR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static UNLINKED_ORDER: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn fail_after(boundary: &str) -> Result<()> {
    #[cfg(test)]
    if FORCE_FAIL_AFTER.with(|slot| slot.borrow().as_deref() == Some(boundary)) {
        return Err(Error::Io(format!(
            "injected failure after {boundary} via test hook"
        )));
    }
    if std::env::var("TASKS_RETAINED_FAIL_AFTER").as_deref() == Ok(boundary) {
        return Err(Error::Io(format!(
            "injected failure after {boundary} via TASKS_RETAINED_FAIL_AFTER"
        )));
    }
    Ok(())
}

fn fail_after_phase(boundary: &str, phase: &str) -> Result<()> {
    fail_after(boundary)?;
    fail_after(&format!("{boundary}:{phase}"))
}

fn remove_created_file(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    {
        UNLINKED_ORDER.with(|order| order.borrow_mut().push(path.to_path_buf()));
        if let Some(msg) = FORCE_REMOVE_FILE_ERROR.with(|slot| slot.borrow().clone()) {
            return Err(std::io::Error::other(msg));
        }
    }
    std::fs::remove_file(path)
}

fn remove_created_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(msg) = FORCE_REMOVE_DIR_ERROR.with(|slot| slot.borrow().clone()) {
        return Err(std::io::Error::other(msg));
    }
    std::fs::remove_dir(path)
}

fn sync_dir_chain(start: &Path, records: &Path) -> Result<()> {
    let mut current = Some(start);
    while let Some(dir) = current {
        std::fs::File::open(dir)?.sync_all()?;
        if dir == records {
            break;
        }
        current = dir.parent();
    }
    Ok(())
}

pub(crate) fn write_and_save_with_rollback(
    project: &Project,
    id: &TaskId,
    phase: &str,
    prepared: &PreparedBatch,
    cleanup_failed: &mut bool,
    save: impl FnOnce(&PreparedBatch) -> Result<()>,
) -> Result<()> {
    let mut created_files: Vec<PathBuf> = Vec::new();
    let mut created_dir = false;
    let mut ensured_dir = false;
    let result = (|| -> Result<()> {
        for file in &prepared.files {
            if file.create_new {
                if !ensured_dir {
                    created_dir = attachments::ensure_task_dir(project, id)?;
                    ensured_dir = true;
                }
                attachments::write_new(&file.path, &file.bytes)?;
                created_files.push(file.path.clone());
            }
        }
        if !prepared.files.is_empty() {
            sync_dir_chain(&attachments::task_dir(project, id), &project.records)?;
        }
        super::publish::stop_phase("artifact", phase);
        fail_after_phase("artifact", phase)?;
        save(prepared)
    })();
    if let Err(mut error) = result {
        let mut all_files_removed = true;
        for path in created_files.iter().rev() {
            if let Err(cleanup) = remove_created_file(path) {
                all_files_removed = false;
                *cleanup_failed = true;
                error = error.with_suffix(&format!(
                    "; cleanup needs attention: could not remove {}: {cleanup}",
                    path.display()
                ));
            }
        }
        if created_dir && all_files_removed {
            let dir = attachments::task_dir(project, id);
            if let Err(cleanup) = remove_created_dir(&dir) {
                *cleanup_failed = true;
                error = error.with_suffix(&format!(
                    "; cleanup needs attention: could not remove {}: {cleanup}",
                    dir.display()
                ));
            }
        }
        return Err(error);
    }
    for file in &prepared.files {
        crate::written::record(&project.records, &file.path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(id: &str) -> Task {
        let raw = format!(
            "---\nid: {id}\ntitle: Test\nstatus: doing\npriority: 2\ncreated: 2026-11-17T00:00:00Z\nupdated: 2026-11-17T00:00:00Z\ndepends: []\ntags: []\n---\n"
        );
        crate::history::fixture(&raw, &format!("tasks/{id}.md")).unwrap()
    }

    fn reset_hooks() {
        FORCE_FAIL_AFTER.with(|s| *s.borrow_mut() = None);
        FORCE_REMOVE_FILE_ERROR.with(|s| *s.borrow_mut() = None);
        FORCE_REMOVE_DIR_ERROR.with(|s| *s.borrow_mut() = None);
        UNLINKED_ORDER.with(|s| s.borrow_mut().clear());
    }

    struct HookReset;
    impl Drop for HookReset {
        fn drop(&mut self) {
            reset_hooks();
        }
    }

    #[test]
    fn write_and_save_rolls_back_in_reverse_order_preserves_existing_and_removes_created_dir() {
        let _guard = HookReset;
        let temp = tempfile::tempdir().unwrap();
        let records = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(records.join("tasks")).unwrap();
        let project = Project::from_config(
            records.clone(),
            "prefix = 'tst'\n[attachments]\nmax_bytes = 2048\nmultipart_validators = []\n",
        )
        .unwrap();
        let id = TaskId::parse("tst-000001").unwrap();
        let task = sample_task("tst-000001");

        // 1. Fresh task_dir (`created_dir == true`): rollback removes files in reverse order AND
        //    removes newly created empty `tasks/files/<id>/`, keeping `written` empty.
        let a = NewArtifact {
            name: "first.json".into(),
            bytes: b"{\"a\":1}".to_vec(),
        };
        let b = NewArtifact {
            name: "second.json".into(),
            bytes: b"{\"b\":2}".to_vec(),
        };
        let prepared = prepare_batch(&project, &task, &[a, b]).unwrap();
        assert_eq!(prepared.files.len(), 2);
        assert!(prepared.files.iter().all(|f| f.create_new));

        FORCE_FAIL_AFTER.with(|s| *s.borrow_mut() = Some("artifact:items_published".into()));
        let mut cleanup_failed = false;
        let err = write_and_save_with_rollback(
            &project,
            &id,
            "items_published",
            &prepared,
            &mut cleanup_failed,
            |_| panic!("save must not run after fail_after_phase"),
        )
        .unwrap_err();
        assert!(!cleanup_failed);
        assert!(err.to_string().contains("artifact:items_published"));
        let unlinked = UNLINKED_ORDER.with(|o| o.borrow().clone());
        let task_dir = attachments::task_dir(&project, &id);
        assert_eq!(
            unlinked,
            vec![task_dir.join("second.json"), task_dir.join("first.json")],
            "newly created files must be unlinked in reverse creation order"
        );
        assert!(
            !task_dir.exists(),
            "newly created empty task_dir must be removed on rollback"
        );
        assert!(!crate::written::take().roots.contains_key(&records));

        // 2. Pre-existing file (`create_new == false`) + newly created files (`create_new == true`):
        //    only `create_new == true` files are unlinked; pre-existing file survives byte-for-byte.
        reset_hooks();
        std::fs::create_dir_all(&task_dir).unwrap();
        let existing_path = task_dir.join("existing.json");
        std::fs::write(&existing_path, b"{\"reused\":true}").unwrap();

        let mixed = prepare_batch(
            &project,
            &task,
            &[
                NewArtifact {
                    name: "existing.json".into(),
                    bytes: b"{\"reused\":true}".to_vec(),
                },
                NewArtifact {
                    name: "created-1.json".into(),
                    bytes: b"{\"c\":1}".to_vec(),
                },
                NewArtifact {
                    name: "created-2.json".into(),
                    bytes: b"{\"c\":2}".to_vec(),
                },
            ],
        )
        .unwrap();
        assert!(!mixed.files[0].create_new);
        assert!(mixed.files[1].create_new);
        assert!(mixed.files[2].create_new);

        let mut cleanup_failed = false;
        let err = write_and_save_with_rollback(
            &project,
            &id,
            "items_published",
            &mixed,
            &mut cleanup_failed,
            |_| Err(Error::Validation("simulated save failure".into())),
        )
        .unwrap_err();
        assert!(!cleanup_failed);
        assert_eq!(err.to_string(), "simulated save failure");
        assert_eq!(
            UNLINKED_ORDER.with(|o| o.borrow().clone()),
            vec![
                task_dir.join("created-2.json"),
                task_dir.join("created-1.json"),
            ]
        );
        assert_eq!(std::fs::read(&existing_path).unwrap(), b"{\"reused\":true}");
        assert!(!task_dir.join("created-1.json").exists());
        assert!(!task_dir.join("created-2.json").exists());
        assert!(!crate::written::take().roots.contains_key(&records));
    }

    #[test]
    fn unlink_and_rmdir_failures_set_cleanup_failed_and_append_cleanup_suffix() {
        let _guard = HookReset;
        let temp = tempfile::tempdir().unwrap();
        let records = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(records.join("tasks")).unwrap();
        let project = Project::from_config(records.clone(), "prefix = 'tst'\n").unwrap();
        let id = TaskId::parse("tst-000002").unwrap();
        let task = sample_task("tst-000002");

        let prepared = prepare_batch(
            &project,
            &task,
            &[NewArtifact {
                name: "stuck.json".into(),
                bytes: b"{\"stuck\":true}".to_vec(),
            }],
        )
        .unwrap();

        FORCE_FAIL_AFTER.with(|s| *s.borrow_mut() = Some("artifact:items_published".into()));
        FORCE_REMOVE_FILE_ERROR.with(|s| *s.borrow_mut() = Some("simulated EACCES".into()));

        let mut cleanup_failed = false;
        let err = write_and_save_with_rollback(
            &project,
            &id,
            "items_published",
            &prepared,
            &mut cleanup_failed,
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(cleanup_failed, "cleanup_failed must be set on unlink error");
        let stuck_path = attachments::task_dir(&project, &id).join("stuck.json");
        let expected_suffix = format!(
            "; cleanup needs attention: could not remove {}: simulated EACCES",
            stuck_path.display()
        );
        assert!(err.to_string().ends_with(&expected_suffix), "got: {err}");
        assert!(
            stuck_path.is_file(),
            "failed unlink leaves file on disk requiring operator attention"
        );
        assert!(!crate::written::take().roots.contains_key(&records));

        // Clean up stuck file, then test `remove_dir` failure on newly created `task_dir`.
        std::fs::remove_file(&stuck_path).unwrap();
        std::fs::remove_dir(attachments::task_dir(&project, &id)).unwrap();
        reset_hooks();
        FORCE_FAIL_AFTER.with(|s| *s.borrow_mut() = Some("artifact:items_published".into()));
        FORCE_REMOVE_DIR_ERROR.with(|s| *s.borrow_mut() = Some("simulated rmdir EBUSY".into()));

        let mut cleanup_failed_dir = false;
        let err_dir = write_and_save_with_rollback(
            &project,
            &id,
            "items_published",
            &prepared,
            &mut cleanup_failed_dir,
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(cleanup_failed_dir);
        let dir_path = attachments::task_dir(&project, &id);
        let expected_dir_suffix = format!(
            "; cleanup needs attention: could not remove {}: simulated rmdir EBUSY",
            dir_path.display()
        );
        assert!(
            err_dir.to_string().ends_with(&expected_dir_suffix),
            "got: {err_dir}"
        );
    }

    #[test]
    fn is_live_attachment_and_note_count_match_single_and_multipart_states() {
        let temp = tempfile::tempdir().unwrap();
        let records = temp.path().canonicalize().unwrap();
        let project = Project::from_config(
            records,
            "prefix = 'tst'\n[attachments]\nmax_bytes = 256\nmultipart_validators = []\n",
        )
        .unwrap();
        let mut task = sample_task("tst-000003");

        let small_bytes = b"{\"version\":1,\"entries\":[]}".to_vec();
        let small_sha = sha256_hex(&small_bytes);
        let small_name = format!("retained-manifest-{small_sha}.json");
        let small_art = NewArtifact {
            name: small_name.clone(),
            bytes: small_bytes.clone(),
        };
        assert!(!is_live_attachment(&task, &small_name, &small_sha));
        assert_eq!(artifact_note_count(&project, &task, &small_art).unwrap(), 1);

        crate::commands::append_note(
            &mut task,
            "test",
            &attachments::attached_note(&small_name, small_bytes.len() as u64, None),
        )
        .unwrap();
        assert!(is_live_attachment(&task, &small_name, &small_sha));
        assert_eq!(artifact_note_count(&project, &task, &small_art).unwrap(), 0);

        let mut big_entries = Vec::new();
        for i in 0..8 {
            big_entries.push(format!(
                "{{\"type\":\"file\",\"path\":\"f-{i}.txt\",\"sha256\":\"{}\"}}",
                "a".repeat(64)
            ));
        }
        let big_bytes =
            format!("{{\"version\":1,\"entries\":[{}]}}", big_entries.join(",")).into_bytes();
        assert!(big_bytes.len() > 256);
        let big_sha = sha256_hex(&big_bytes);
        let big_name = format!("retained-manifest-{big_sha}.json");
        let big_art = NewArtifact {
            name: big_name.clone(),
            bytes: big_bytes.clone(),
        };
        let expected_parts = multipart::cut::cut(&big_bytes, 256).unwrap().len();
        assert!(expected_parts >= 3);
        assert!(!is_live_attachment(&task, &big_name, &big_sha));
        assert_eq!(
            artifact_note_count(&project, &task, &big_art).unwrap(),
            expected_parts + 2
        );

        let decl = Declaration {
            name: big_name.clone(),
            bytes: big_bytes.len() as u64,
            parts: expected_parts,
            sha256: big_sha.clone(),
            catalog_sha256: "b".repeat(64),
            caption: None,
        };
        crate::commands::append_note(&mut task, "test", &decl.note()).unwrap();
        assert!(is_live_attachment(&task, &big_name, &big_sha));
        assert!(!is_live_attachment(&task, &big_name, &"c".repeat(64)));
        assert_eq!(artifact_note_count(&project, &task, &big_art).unwrap(), 0);
    }

    fn isolated_git(home: &Path, dir: &Path, args: &[&str]) -> String {
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(dir)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(args);
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn run_in_child(
        home: &Path,
        dir: &Path,
        argv: &[&str],
        inject_cleanup_error: bool,
    ) -> std::process::Output {
        let exe = std::env::current_exe().unwrap();
        let mut cmd = std::process::Command::new(exe);
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .env("TASKS_SESSION", "artifacts-cleanup-test")
            .env("TASKS_ARTIFACTS_CHILD_DIR", dir)
            .env(
                "TASKS_ARTIFACTS_CHILD_ARGS",
                serde_json::to_string(argv).unwrap(),
            );
        if inject_cleanup_error {
            cmd.env("TASKS_ARTIFACTS_CHILD_INJECT_CLEANUP_ERR", "1");
        }
        cmd.args([
            "--exact",
            "retained::artifacts::tests::checkpoint_run_skips_incomplete_when_cleanup_failed",
            "--nocapture",
        ])
        .output()
        .unwrap()
    }

    #[test]
    fn checkpoint_run_skips_incomplete_when_cleanup_failed() {
        if let Ok(dir) = std::env::var("TASKS_ARTIFACTS_CHILD_DIR") {
            let _guard = HookReset;
            if std::env::var("TASKS_ARTIFACTS_CHILD_INJECT_CLEANUP_ERR").is_ok() {
                FORCE_FAIL_AFTER
                    .with(|s| *s.borrow_mut() = Some("artifact:items_published".into()));
                FORCE_REMOVE_FILE_ERROR
                    .with(|s| *s.borrow_mut() = Some("simulated unlink EACCES".into()));
            }
            let args: Vec<String> =
                serde_json::from_str(&std::env::var("TASKS_ARTIFACTS_CHILD_ARGS").unwrap())
                    .unwrap();
            let mut argv = vec!["tasks".to_string(), "-C".to_string(), dir];
            argv.extend(args);
            let cli = <crate::cli::Cli as clap::Parser>::try_parse_from(argv).unwrap();
            let host = std::sync::Arc::new(crate::config::HostConfig::default());
            match crate::commands::run(cli, Some(host)) {
                Ok(out) => {
                    let rendered = crate::output::render(
                        &out,
                        crate::output::Format::Json,
                        &crate::style::Painter::new(
                            crate::style::ColorMode::Never,
                            crate::output::Format::Json,
                            false,
                        ),
                        &crate::table::Layout {
                            width: None,
                            headers: false,
                        },
                    );
                    println!("CHILD_JSON:{rendered}");
                    return;
                }
                Err(err) => {
                    eprintln!("CHILD_ERR:{err}");
                    std::process::exit(1);
                }
            }
        }

        let home = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let code = root.path().join("code");
        let records = root.path().join("records");
        for repo in [&code, &records] {
            std::fs::create_dir_all(repo).unwrap();
            isolated_git(home.path(), repo, &["init", "-q", "-b", "main"]);
            let canon = repo.canonicalize().unwrap();
            assert_eq!(
                Path::new(&isolated_git(
                    home.path(),
                    repo,
                    &["rev-parse", "--path-format=absolute", "--show-toplevel"]
                ))
                .canonicalize()
                .unwrap(),
                canon
            );
            assert!(
                Path::new(&isolated_git(
                    home.path(),
                    repo,
                    &["rev-parse", "--path-format=absolute", "--absolute-git-dir"]
                ))
                .canonicalize()
                .unwrap()
                .starts_with(&canon)
            );
            std::fs::write(repo.join("README.md"), "fixture\n").unwrap();
            isolated_git(home.path(), repo, &["add", "README.md"]);
            isolated_git(home.path(), repo, &["commit", "-qm", "seed"]);
        }

        let parse_child = |out: std::process::Output| -> serde_json::Value {
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).unwrap();
            let raw = stdout
                .lines()
                .find_map(|l| l.strip_prefix("CHILD_JSON:"))
                .unwrap();
            serde_json::from_str(raw).unwrap()
        };

        parse_child(run_in_child(
            home.path(),
            &code,
            &[
                "init",
                "--prefix",
                "sci",
                "--records",
                records.to_str().unwrap(),
            ],
            false,
        ));
        let sci_records = records.join("sci");
        let cfg_path = sci_records.join("tasks/.config.toml");
        let mut cfg_doc: toml::Value =
            toml::from_str(&std::fs::read_to_string(&cfg_path).unwrap()).unwrap();
        cfg_doc.as_table_mut().unwrap().insert(
            "git".into(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "commit".into(),
                toml::Value::Boolean(true),
            )])),
        );
        std::fs::write(&cfg_path, toml::to_string(&cfg_doc).unwrap()).unwrap();
        isolated_git(home.path(), &sci_records, &["add", "."]);
        isolated_git(
            home.path(),
            &sci_records,
            &["commit", "-qm", "enable commit"],
        );

        let added = parse_child(run_in_child(
            home.path(),
            &code,
            &["add", "Cleanup failure test"],
            false,
        ));
        let id = added["id"].as_str().unwrap().to_string();
        parse_child(run_in_child(home.path(), &code, &["start", &id], false));
        let state = parse_child(run_in_child(
            home.path(),
            &code,
            &["retained", "state", &id],
            false,
        ));

        let scratch = code.join("scratch-dir");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("item.txt"), "payload").unwrap();
        let req = serde_json::json!({
            "version": 1,
            "operation_id": "1".repeat(32),
            "host": crate::claims::hostname(),
            "inputs": [{"task": id, "basis": state["basis"]}],
            "declarations": [{
                "kind": "scratch",
                "path": scratch.to_str().unwrap(),
                "purpose": "Review",
                "cleanup_trigger": "After review"
            }]
        });
        let req_path = code.join("req.json");
        std::fs::write(&req_path, serde_json::to_vec(&req).unwrap()).unwrap();

        let failed = run_in_child(
            home.path(),
            &code,
            &[
                "retained",
                "checkpoint",
                &id,
                "--request",
                req_path.to_str().unwrap(),
            ],
            true,
        );
        assert_eq!(failed.status.code(), Some(1), "{failed:?}");
        let stderr = String::from_utf8(failed.stderr).unwrap();
        assert!(
            stderr.contains("; cleanup needs attention: could not remove "),
            "got: {stderr}"
        );
        assert!(stderr.contains("simulated unlink EACCES"), "got: {stderr}");
        assert!(
            !stderr.contains("failure receipt could not publish"),
            "checkpoint::run must skip incomplete when cleanup_failed is true: {stderr}"
        );
        let task_md = std::fs::read_to_string(sci_records.join(format!("tasks/{id}.md"))).unwrap();
        assert!(
            !task_md.contains("\"phase\":\"incomplete\""),
            "incomplete receipt must not be written over uncleaned directory:\n{task_md}"
        );
    }
}
