use super::{Ctx, append_note, append_stamped_note, owner_name};
use crate::attachments::{self, EntryState};
use crate::error::{Error, Result};
use crate::history::ArchiveDescriptor;
use crate::model::Task;
use crate::output::{ArchiveApplyOut, ArchivePreview, Output};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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
        crate::written::record(&ctx.project.records, &ctx.project.task_path(&id));
        crate::written::record(
            &ctx.project.records,
            &attachments::task_dir(&ctx.project, &id).join(&name),
        );
        super::finish(&mut ctx);
        return Ok(Output::ArchiveApply(ArchiveApplyOut {
            id: id.to_string(),
            source_sha256: expected,
            archive_name: name,
            record_bytes_before: raw.len() as u64,
            record_bytes_after: raw.len() as u64,
            action: "already_archived",
            warnings: ctx.warnings,
        }));
    }
    if digest(raw.as_bytes()) != expected {
        return Err(Error::ConcurrentModification(
            id.to_string(),
            "source hash differs from preview; preview again".into(),
        ));
    }
    let prepared = prepare(&mut ctx, task, &raw)?;
    if !prepared.preview.eligible {
        return Err(Error::Validation(prepared.preview.reasons.join("; ")));
    }
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
    // Revalidation uses the published bytes and all existing segments, then checks source last.
    ctx.project
        .write_task_guarded(&ctx.registry, &prepared.task, || {
            if std::fs::read(ctx.project.task_path(&id))? != raw.as_bytes() {
                return Err(Error::ConcurrentModification(
                    id.to_string(),
                    "source changed; preserve the orphan archive and preview again".into(),
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
    interrupt_at("after-record-switch");
    let after = std::fs::metadata(ctx.project.task_path(&id))?.len();
    super::follow_holder(&mut ctx, &id, None, "the note archive landed");
    super::finish(&mut ctx);
    Ok(Output::ArchiveApply(ArchiveApplyOut {
        id: id.to_string(),
        source_sha256: expected,
        archive_name: prepared.preview.archive_name,
        record_bytes_before: raw.len() as u64,
        record_bytes_after: after,
        action: "archived",
        warnings: ctx.warnings,
    }))
}
