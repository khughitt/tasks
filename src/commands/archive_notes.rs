use super::{Ctx, append_note, append_stamped_note, owner_name};
use crate::attachments::{self, DirState, EntryState};
use crate::error::{Error, Result};
use crate::history::ArchiveDescriptor;
use crate::model::{Task, TaskId};
use crate::output::{ArchiveApplyOut, ArchivePreview, ArchiveVerified, Output};
use crate::repo::Project;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn interrupt_at(boundary: &str) {
    if std::env::var("TASKS_ARCHIVE_STOP_AFTER").as_deref() == Ok(boundary) {
        std::process::exit(86);
    }
}

// Test-only fault injection, beside interrupt_at: proves a failed check refuses the apply.
fn fault(name: &str) -> bool {
    std::env::var("TASKS_ARCHIVE_FAULT").as_deref() == Ok(name)
}

fn note_section(raw: &str) -> &[u8] {
    let mut offset = 0;
    for line in raw.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "## Notes" {
            return &raw.as_bytes()[offset..];
        }
        offset += line.len();
    }
    &[]
}

struct Prepared {
    preview: ArchivePreview,
    task: Task,
    archive: ArchiveDescriptor,
    bytes: Vec<u8>,
}

fn prepare(ctx: &mut Ctx, mut task: Task, raw: &str) -> Result<Prepared> {
    let physical = crate::format::parse_physical(raw, &task.id.to_string())?;
    let bytes = note_section(raw).to_vec();
    let archive = ArchiveDescriptor {
        sha256: digest(&bytes),
        note_count: physical.inline_notes.len(),
    };
    let name = archive.file_name();
    let mut reasons = Vec::new();
    if archive.note_count == 0 {
        reasons.push("no inline notes".into());
    }
    if bytes.len() as u64 > ctx.project.attachments_max_bytes {
        reasons.push(format!(
            "archive exceeds attachment cap {} bytes; do not split or truncate history",
            ctx.project.attachments_max_bytes
        ));
    }
    let storage = || -> Result<()> {
        attachments::check_storage(&ctx.project, &task.id)?;
        let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
        match attachments::entry_state(&target)? {
            EntryState::Absent => Ok(()),
            EntryState::Unsafe(detail) => Err(Error::AttachmentUnsafe(detail)),
            EntryState::File(_) => {
                if ctx.project.read_archive(&task.id, &archive)? != bytes {
                    return Err(Error::AttachmentExists(format!(
                        "conflicting archive {name}"
                    )));
                }
                Ok(())
            }
        }
    };
    if let Err(error) = storage() {
        reasons.push(error.to_string());
    }
    let source_sha256 = digest(raw.as_bytes());
    let mut after = raw.len();
    if archive.note_count > 0 {
        let owner = owner_name(&ctx.project)?;
        task.note_archives.push(archive.clone());
        append_note(
            &mut task,
            &owner,
            &attachments::attached_note(&name, bytes.len() as u64, None),
        )?;
        append_stamped_note(
            ctx,
            &mut task,
            &owner,
            &format!(
                "archived-notes: v1 source={source_sha256} segment={} count={}",
                archive.sha256, archive.note_count
            ),
        )?;
        task.updated = crate::time::after(&task.updated)?;
        crate::format::validate_task(&task)?;
        crate::review::validate(&task)?;
        crate::defer::validate(&task)?;
        ctx.project.validate_docs(&task)?;
        let physical = crate::history::physical_for_save(&task, |descriptor| {
            if descriptor == &archive {
                Ok(bytes.clone())
            } else {
                ctx.project.read_archive(&task.id, descriptor)
            }
        })?;
        after = crate::format::serialize_physical(&physical).len();
    }
    if after >= raw.len() {
        reasons.push("no active-record byte saving".into());
    }
    Ok(Prepared {
        preview: ArchivePreview {
            id: task.id.to_string(),
            source_sha256,
            archive_name: name,
            archive_bytes: bytes.len() as u64,
            note_count: archive.note_count,
            record_bytes_before: raw.len() as u64,
            record_bytes_after: after as u64,
            eligible: reasons.is_empty(),
            reasons,
        },
        task,
        archive,
        bytes,
    })
}

fn unverified(id: &TaskId, invariant: &str, detail: &str) -> Error {
    Error::ArchiveUnverified(format!(
        "{id}: archive apply failed its {invariant} check: {detail}"
    ))
}

/// The logical change apply may make, checked before anything is written: the old notes
/// unchanged and in order plus the attach note and receipt, one more descriptor, a later
/// `updated`, and nothing else. The attachment ledger is derived from the notes, so this
/// also holds it to the old ledger plus the segment.
fn verify_prepared(before: &Task, prepared: &Prepared) -> Result<Vec<&'static str>> {
    let after = &prepared.task;
    let id = &before.id;
    let refuse = |invariant, detail| Err(unverified(id, invariant, detail));
    let n = before.notes.len();
    let receipts = [
        attachments::attached_note(
            &prepared.preview.archive_name,
            prepared.bytes.len() as u64,
            None,
        ),
        format!(
            "archived-notes: v1 source={} segment={} count={}",
            prepared.preview.source_sha256, prepared.archive.sha256, prepared.archive.note_count
        ),
    ];
    let added: Vec<&str> = after.notes[n.min(after.notes.len())..]
        .iter()
        .map(|note| note.text.as_str())
        .collect();
    if after.notes.get(..n) != Some(before.notes.as_slice()) {
        return refuse("notes_appended", "an existing note changed or moved");
    }
    if added != receipts {
        return refuse(
            "notes_appended",
            "the added notes are not exactly the attach note and the receipt",
        );
    }
    let mut descriptors = before.note_archives.clone();
    descriptors.push(prepared.archive.clone());
    if after.note_archives != descriptors {
        return refuse(
            "fields_unchanged",
            "the descriptors are not the old ones plus the new segment",
        );
    }
    let mut rest = after.clone();
    rest.notes = before.notes.clone();
    rest.note_archives = before.note_archives.clone();
    rest.updated = before.updated.clone();
    if &rest != before {
        return refuse(
            "fields_unchanged",
            "a field other than notes, descriptors and updated changed",
        );
    }
    Ok(vec!["notes_appended", "fields_unchanged"])
}

/// Every entry under the task's attachment directory: a regular file's SHA-256, or the
/// file type of anything else.
fn files_snapshot(project: &Project, id: &TaskId) -> Result<BTreeMap<String, String>> {
    let dir = attachments::task_dir(project, id);
    let mut files = BTreeMap::new();
    match attachments::dir_state(&dir)? {
        DirState::Absent => return Ok(files),
        DirState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        DirState::Directory => {}
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let meta = std::fs::symlink_metadata(entry.path())?;
        let value = if meta.is_file() {
            digest(&std::fs::read(entry.path())?)
        } else {
            format!("{:?}", meta.file_type())
        };
        files.insert(entry.file_name().to_string_lossy().into_owned(), value);
    }
    Ok(files)
}

// A receipt is evidence only at its descriptor's exact logical boundary, after its attach.
fn prior_receipt(ctx: &Ctx, task: &Task, expected: &str) -> Result<Option<String>> {
    let mut sources = BTreeSet::new();
    let mut matched = None;
    for (index, note) in task.notes.iter().enumerate() {
        if !note.text.starts_with("archived-notes:") {
            continue;
        }
        let invalid = || {
            Error::Validation(format!(
                "{}: malformed or unbound archive receipt at logical note {}",
                task.id,
                index + 1
            ))
        };
        let fields: Vec<_> = note.text.split(' ').collect();
        if fields.len() != 5 || fields[0] != "archived-notes:" || fields[1] != "v1" {
            return Err(invalid());
        }
        let source = fields[2].strip_prefix("source=").ok_or_else(invalid)?;
        let segment = fields[3].strip_prefix("segment=").ok_or_else(invalid)?;
        let count = fields[4].strip_prefix("count=").ok_or_else(invalid)?;
        if !valid_hash(source) || !sources.insert(source) {
            return Err(invalid());
        }
        let descriptor =
            ArchiveDescriptor::parse(&format!("v1:{segment}:{count}")).map_err(|_| invalid())?;
        let mut end = 0;
        let bound = task.note_archives.iter().any(|archive| {
            end += archive.note_count;
            archive == &descriptor && index == end + 1
        });
        if !bound {
            return Err(invalid());
        }
        let bytes = ctx.project.read_archive(&task.id, &descriptor)?;
        let attached = &task.notes[index - 1];
        if attached.text
            != attachments::attached_note(&descriptor.file_name(), bytes.len() as u64, None)
            || attached.by != note.by
        {
            return Err(invalid());
        }
        if source == expected {
            matched = Some(descriptor.file_name());
        }
    }
    Ok(matched)
}

pub fn run(mut ctx: Ctx, id: String, expect: Option<String>) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let (task, raw) = if expect.is_some() {
        super::load_with_raw(&mut ctx, &id)?
    } else {
        ctx.project.read_task_with_raw(&id)?
    };
    crate::retained::publish::permission(
        &ctx.project,
        &task,
        crate::retained::publish::WriteAction::Archive,
    )?;
    let Some(expected) = expect else {
        prior_receipt(&ctx, &task, "")?;
        return Ok(Output::ArchivePreview(
            prepare(&mut ctx, task, &raw)?.preview,
        ));
    };
    if !valid_hash(&expected) {
        return Err(Error::Validation(
            "--expect must be 64 lowercase SHA-256 hex characters".into(),
        ));
    }
    ctx.refuse_foreign_live_claim(&id)?;
    if let Some(name) = prior_receipt(&ctx, &task, &expected)? {
        // A switched record may not have reached auto-commit before interruption.
        ctx.project.record_task_written(&task);
        super::finish(&mut ctx);
        return Ok(Output::ArchiveApply(ArchiveApplyOut {
            id: id.to_string(),
            source_sha256: expected,
            archive_name: name,
            record_bytes_before: raw.len() as u64,
            record_bytes_after: raw.len() as u64,
            action: "already_archived",
            verified: ArchiveVerified {
                invariants: vec!["receipt_bound", "segments_verified"],
                written: Vec::new(),
            },
            warnings: ctx.warnings,
        }));
    }
    if digest(raw.as_bytes()) != expected {
        return Err(Error::ConcurrentModification(
            id.to_string(),
            "source hash differs from preview; preview again".into(),
        ));
    }
    let before = task.clone();
    let mut prepared = prepare(&mut ctx, task, &raw)?;
    if !prepared.preview.eligible {
        return Err(Error::Validation(prepared.preview.reasons.join("; ")));
    }
    if fault("prepared-note") {
        prepared.task.notes[0].text.push_str(" (altered)");
    }
    let mut invariants = verify_prepared(&before, &prepared)?;
    let mut files = files_snapshot(&ctx.project, &id)?;
    files.insert(
        prepared.preview.archive_name.clone(),
        prepared.archive.sha256.clone(),
    );
    let target = attachments::task_dir(&ctx.project, &id).join(&prepared.preview.archive_name);
    interrupt_at("before-segment-publish");
    attachments::ensure_task_dir(&ctx.project, &id)?;
    match attachments::entry_state(&target)? {
        EntryState::Absent => attachments::write_new(&target, &prepared.bytes)?,
        EntryState::File(_)
            if ctx.project.read_archive(&id, &prepared.archive)? == prepared.bytes => {}
        _ => {
            return Err(Error::AttachmentUnsafe(format!(
                "{} changed before publication; nothing overwritten",
                target.display()
            )));
        }
    }
    crate::written::record(&ctx.project.records, &target);
    interrupt_at("after-segment-publish");
    if fault("stray-file") {
        attachments::write_new(&target.with_file_name("stray.txt"), b"stray")?;
    }
    // Revalidation uses the published bytes and all existing segments, then checks source last.
    ctx.project
        .write_task_guarded(&ctx.registry, &prepared.task, || {
            if std::fs::read(ctx.project.task_path(&id))? != raw.as_bytes() {
                return Err(Error::ConcurrentModification(
                    id.to_string(),
                    "source changed; preserve the orphan archive and preview again".into(),
                ));
            }
            if files_snapshot(&ctx.project, &id)? != files {
                return Err(unverified(
                    &id,
                    "files_unchanged",
                    "the task's files changed beyond the new segment; nothing switched",
                ));
            }
            interrupt_at("before-record-switch");
            Ok(())
        })
        .map_err(|error| {
            error.with_suffix(&format!(
                "; archive {} retained; verify exact bytes before retry",
                target.display()
            ))
        })?;
    invariants.push("files_unchanged");
    interrupt_at("after-record-switch");
    let record = ctx.project.task_path(&id);
    if ctx.project.read_task(&id)? != prepared.task {
        return Err(unverified(
            &id,
            "record_reloaded",
            &format!(
                "{} switched but reads back differently from the prepared record; \
                 inspect it against the source preview before any further write",
                record.display()
            ),
        ));
    }
    invariants.push("record_reloaded");
    let after = std::fs::metadata(&record)?.len();
    super::follow_holder(&mut ctx, &id, None, "the note archive landed");
    super::finish(&mut ctx);
    Ok(Output::ArchiveApply(ArchiveApplyOut {
        id: id.to_string(),
        source_sha256: expected,
        archive_name: prepared.preview.archive_name,
        record_bytes_before: raw.len() as u64,
        record_bytes_after: after,
        action: "archived",
        verified: ArchiveVerified {
            invariants,
            written: vec![record.display().to_string(), target.display().to_string()],
        },
        warnings: ctx.warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archived_fixture() -> (Task, Prepared) {
        let before = crate::history::fixture(
            "---\nid: arc-a00001\ntitle: T\nstatus: todo\npriority: 2\n\
             created: 2026-10-30T00:00:00Z\nupdated: 2026-10-30T00:00:00Z\n\
             depends: []\ntags: []\n---\n\nBody\n\n## Notes\n\n\
             - 2026-10-30T00:00:00Z (fixture): original evidence\n",
            "tasks/arc-a00001.md",
        )
        .unwrap();
        let bytes = b"## Notes\n\n- 2026-10-30T00:00:00Z (fixture): original evidence\n".to_vec();
        let archive = ArchiveDescriptor {
            sha256: digest(&bytes),
            note_count: 1,
        };
        let source = "a".repeat(64);
        let mut task = before.clone();
        let note = |text: String| crate::model::Note {
            text,
            ..before.notes[0].clone()
        };
        task.notes.push(note(attachments::attached_note(
            &archive.file_name(),
            bytes.len() as u64,
            None,
        )));
        task.notes.push(note(format!(
            "archived-notes: v1 source={source} segment={} count=1",
            archive.sha256
        )));
        task.note_archives.push(archive.clone());
        task.updated = "2026-10-31T00:00:00Z".into();
        let preview = ArchivePreview {
            id: before.id.to_string(),
            source_sha256: source,
            archive_name: archive.file_name(),
            archive_bytes: bytes.len() as u64,
            note_count: 1,
            record_bytes_before: 0,
            record_bytes_after: 0,
            eligible: true,
            reasons: Vec::new(),
        };
        let prepared = Prepared {
            preview,
            task,
            archive,
            bytes,
        };
        (before, prepared)
    }

    fn refusal(before: &Task, prepared: &Prepared) -> String {
        let error = verify_prepared(before, prepared).unwrap_err();
        assert_eq!(error.kind(), "archive_unverified");
        error.to_string()
    }

    #[test]
    fn verify_accepts_exactly_the_archive_change() {
        let (before, prepared) = archived_fixture();
        assert_eq!(
            verify_prepared(&before, &prepared).unwrap(),
            ["notes_appended", "fields_unchanged"]
        );
    }

    #[test]
    fn verify_refuses_any_other_change() {
        let (before, mut prepared) = archived_fixture();
        prepared.task.title.push('!');
        assert!(refusal(&before, &prepared).contains("fields_unchanged"));

        let (before, mut prepared) = archived_fixture();
        prepared.task.note_archives.push(prepared.archive.clone());
        assert!(refusal(&before, &prepared).contains("fields_unchanged"));

        let (before, mut prepared) = archived_fixture();
        prepared.task.notes.pop();
        assert!(refusal(&before, &prepared).contains("notes_appended"));

        let (before, mut prepared) = archived_fixture();
        prepared.task.notes[0].at = "2026-10-29T00:00:00Z".into();
        assert!(refusal(&before, &prepared).contains("notes_appended"));
    }
}
