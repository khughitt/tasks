use super::{Ctx, append_note, load, owner_name, save};
use crate::attachments::{self, EntryState, Ledger};
use crate::error::{Error, Result};
use crate::output::{AttachOut, DetachOut, FileInfo, Output};
use std::path::{Path, PathBuf};

pub enum Source {
    Path(PathBuf),
    Stdin,
    Clipboard,
}

impl Source {
    pub fn from_args(source: Option<String>, clipboard: bool) -> Result<Source> {
        match (source, clipboard) {
            (None, true) => Ok(Source::Clipboard),
            (Some(source), false) if source == "-" => Ok(Source::Stdin),
            (Some(source), false) => Ok(Source::Path(source.into())),
            _ => Err(Error::Validation(
                "give exactly one of a path, `-`, or --clipboard".into(),
            )),
        }
    }
}

/// A blank caption would strip to a ledger note with a trailing colon and nothing
/// after it, so it is rejected the way `detach`'s empty `why` already is.
fn validate_caption(caption: &str) -> Result<()> {
    if caption.trim().is_empty() {
        return Err(Error::Validation(
            "attachment caption must not be blank".into(),
        ));
    }
    crate::format::validate_line("attachment caption", caption)
}

fn basename(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(String::from)
        .ok_or_else(|| {
            Error::InvalidAttachmentName(format!(
                "{} has no usable file name; pass --name",
                path.display()
            ))
        })
}

/// Spec, "attach": the file is written before the ledger, and a rerun with the same
/// bytes under the same name finishes an interrupted attach.
pub fn attach(
    mut ctx: Ctx,
    id: String,
    source: Source,
    name: Option<String>,
    caption: Option<String>,
) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    let max = ctx.project.attachments_max_bytes;
    // Storage safety comes before any read of the source.
    attachments::check_storage(&ctx.project, &task.id)?;
    // A given --name and the caption are validated before any source read, so a bad
    // one fails without running wl-paste or consuming stdin.
    if let Some(name) = &name {
        attachments::validate_name(name)?;
    }
    if let Some(caption) = &caption {
        validate_caption(caption)?;
    }
    let (name, bytes) = match source {
        Source::Path(path) => {
            let name = match name {
                Some(name) => name,
                None => basename(&path)?,
            };
            attachments::validate_name(&name)?;
            let file = std::fs::File::open(&path)
                .map_err(|error| Error::Io(format!("{}: {error}", path.display())))?;
            let bytes = attachments::read_capped(file, max, &path.display().to_string()).map_err(
                |error| match error {
                    Error::Io(detail) => Error::Io(format!("{}: {detail}", path.display())),
                    other => other,
                },
            )?;
            (name, bytes)
        }
        Source::Stdin => {
            let name = name.ok_or_else(|| {
                Error::InvalidAttachmentName("an attachment from stdin needs --name".into())
            })?;
            attachments::validate_name(&name)?;
            (
                name,
                attachments::read_capped(std::io::stdin().lock(), max, "stdin")?,
            )
        }
        Source::Clipboard => {
            let image = crate::clipboard::read(max)?;
            let name = name.unwrap_or_else(|| {
                crate::clipboard::default_name(&crate::time::now(), image.extension)
            });
            attachments::validate_name(&name)?;
            (name, image.bytes)
        }
    };
    let note = attachments::attached_note(&name, bytes.len() as u64, caption.as_deref());
    crate::format::validate_note_text(&note)?;

    let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
    let live = attachments::ledger(&task).get(&name) == Some(&Ledger::Attached);
    let state = attachments::entry_state(&target)?;
    if live && state != EntryState::Absent {
        return Err(Error::AttachmentExists(format!(
            "{} already has {name}",
            task.id
        )));
    }
    let write = match state {
        EntryState::Absent => true,
        EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        EntryState::File(size) => {
            if size != bytes.len() as u64 || std::fs::read(&target)? != bytes {
                return Err(Error::AttachmentExists(format!(
                    "{} holds an unrecorded {name} with different bytes; detach it or choose another --name",
                    task.id
                )));
            }
            false // an interrupted attach left these bytes; this run adds only the note
        }
    };

    // Identity and the claim store resolve before any write, as for `note`.
    ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    let created_dir = if write {
        let created = attachments::ensure_task_dir(&ctx.project, &task.id)?;
        attachments::write_new(&target, &bytes)?;
        created
    } else {
        false
    };
    append_note(&mut task, &owner, &note)?;
    if let Err(error) = save(&mut ctx, &mut task) {
        if !write {
            return Err(error);
        }
        let mut cleanup = std::fs::remove_file(&target);
        if cleanup.is_ok() && created_dir {
            cleanup = std::fs::remove_dir(target.parent().expect("attachment has a directory"));
        }
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup) => error.with_suffix(&format!(
                "; also could not remove {}: {cleanup}",
                target.display()
            )),
        });
    }
    Ok(Output::Attach(AttachOut {
        id: task.id.to_string(),
        file: FileInfo {
            name,
            path: target.display().to_string(),
            bytes: bytes.len() as u64,
        },
        warnings: ctx.warnings,
    }))
}

/// Spec, "detach": the ledger is written before the file is deleted, and a rerun
/// finishes an interrupted detach without a second note.
pub fn detach(mut ctx: Ctx, id: String, name: String, why: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    attachments::validate_name(&name)?;
    if why.trim().is_empty() {
        return Err(Error::Validation("detach needs a reason".into()));
    }
    let note = attachments::detached_note(&name, &why);
    crate::format::validate_note_text(&note)?;
    let dir_exists = attachments::check_storage(&ctx.project, &task.id)?;
    let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
    let present = dir_exists
        && match attachments::entry_state(&target)? {
            EntryState::File(_) => true,
            EntryState::Absent => false,
            EntryState::Unsafe(detail) => return Err(Error::AttachmentUnsafe(detail)),
        };
    let recorded = attachments::ledger(&task).get(&name).copied();
    if !present && recorded != Some(Ledger::Attached) {
        return Err(Error::AttachmentMissing(format!(
            "{} has no attachment {name}",
            task.id
        )));
    }
    ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    if recorded != Some(Ledger::Detached) {
        append_note(&mut task, &owner, &note)?;
        save(&mut ctx, &mut task)?;
    }
    if present {
        std::fs::remove_file(&target)?;
        let dir = target.parent().expect("attachment has a directory");
        if std::fs::read_dir(dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
        }
    }
    Ok(Output::Detach(DetachOut {
        id: task.id.to_string(),
        name,
        path: target.display().to_string(),
        removed: present,
        warnings: ctx.warnings,
    }))
}
