use super::{Ctx, append_note, load, owner_name, save};
use crate::attachments::{self, EntryState, Ledger};
use crate::error::{Error, Result};
use crate::output::{AttachOut, DetachOut, FileInfo, Output};
use std::path::{Path, PathBuf};

pub enum Source {
    Paths(Vec<PathBuf>),
    Stdin,
    Clipboard,
}

impl Source {
    pub fn from_args(sources: Vec<String>, clipboard: bool) -> Result<Source> {
        match (sources.as_slice(), clipboard) {
            ([], true) => Ok(Source::Clipboard),
            ([source], false) if source == "-" => Ok(Source::Stdin),
            (paths, false) if !paths.is_empty() && paths.iter().all(|path| path != "-") => {
                Ok(Source::Paths(paths.iter().map(PathBuf::from).collect()))
            }
            _ => Err(Error::Validation(
                "give one or more paths, a single `-`, or --clipboard".into(),
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
    let mut task = load(&mut ctx, &id)?;
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
    let sources = match source {
        Source::Paths(paths) => {
            if paths.len() > 1 && name.is_some() {
                return Err(Error::Validation("--name needs a single source".into()));
            }
            let mut sources = Vec::new();
            let mut names = std::collections::HashSet::new();
            for path in paths {
                let name = match &name {
                    Some(name) => name.clone(),
                    None => basename(&path)?,
                };
                attachments::validate_name(&name)?;
                if !names.insert(name.clone()) {
                    return Err(Error::AttachmentExists(format!(
                        "more than one source would be stored as {name}"
                    )));
                }
                let file = std::fs::File::open(&path)
                    .map_err(|error| Error::Io(format!("{}: {error}", path.display())))?;
                let bytes = attachments::read_capped(file, max, &path.display().to_string())
                    .map_err(|error| match error {
                        Error::Io(detail) => Error::Io(format!("{}: {detail}", path.display())),
                        other => other,
                    })?;
                sources.push((name, bytes));
            }
            sources
        }
        Source::Stdin => {
            let name = name.ok_or_else(|| {
                Error::InvalidAttachmentName("an attachment from stdin needs --name".into())
            })?;
            vec![(
                name,
                attachments::read_capped(std::io::stdin().lock(), max, "stdin")?,
            )]
        }
        Source::Clipboard => {
            let image = crate::clipboard::read(max)?;
            let name = name.unwrap_or_else(|| {
                crate::clipboard::default_name(&crate::time::now(), image.extension)
            });
            attachments::validate_name(&name)?;
            vec![(name, image.bytes)]
        }
    };
    let ledger = attachments::ledger(&task);
    let mut pending = Vec::new();
    let mut files = Vec::new();
    for (name, bytes) in sources {
        let note = attachments::attached_note(&name, bytes.len() as u64, caption.as_deref());
        crate::format::validate_note_text(&note)?;
        let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
        let live = ledger.get(&name) == Some(&Ledger::Attached);
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
                false // an interrupted attach left these bytes; add only the note
            }
        };
        files.push(FileInfo {
            name,
            path: target.display().to_string(),
            bytes: bytes.len() as u64,
        });
        pending.push((target, bytes, write));
        append_note(&mut task, &owner, &note)?;
    }

    // Identity and the claim store resolve before any write, as for `note`.
    let me = ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    let mut written = Vec::new();
    let mut created_dir = false;
    let result = (|| {
        for (target, bytes, write) in &pending {
            if *write {
                if written.is_empty() {
                    created_dir = attachments::ensure_task_dir(&ctx.project, &task.id)?;
                }
                attachments::write_new(target, bytes)?;
                written.push(target);
            }
        }
        save(&mut ctx, &mut task)
    })();
    if let Err(mut error) = result {
        for target in written.iter().rev() {
            if let Err(cleanup) = std::fs::remove_file(target) {
                error = error.with_suffix(&format!(
                    "; also could not remove {}: {cleanup}",
                    target.display()
                ));
            }
        }
        if created_dir {
            let dir = attachments::task_dir(&ctx.project, &task.id);
            if let Err(cleanup) = std::fs::remove_dir(&dir) {
                error = error.with_suffix(&format!(
                    "; also could not remove {}: {cleanup}",
                    dir.display()
                ));
            }
        }
        return Err(error);
    }
    for (target, _, _) in &pending {
        crate::written::record(&ctx.project.records, target);
    }
    super::follow_holder(&mut ctx, &task.id, Some(&me), "the attachments landed");
    super::finish(&mut ctx);
    Ok(Output::Attach(AttachOut {
        id: task.id.to_string(),
        files,
        warnings: ctx.warnings,
    }))
}

/// Spec, "detach": the ledger is written before the file is deleted, and a rerun
/// finishes an interrupted detach without a second note.
pub fn detach(mut ctx: Ctx, id: String, name: String, why: String) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    attachments::validate_name(&name)?;
    if task
        .note_archives
        .iter()
        .any(|archive| archive.file_name() == name)
    {
        return Err(Error::Validation(format!(
            "{}: {name} is protected immutable note history and cannot be detached",
            task.id
        )));
    }
    if let Some(entry) = task
        .refs
        .iter()
        .find(|entry| entry.reference == format!("files:{name}"))
    {
        return Err(Error::RefCitesAttachment(format!(
            "{} reference {} names {name}; remove it first with `tasks edit {} --rm-ref {}`",
            task.id, entry.handle, task.id, entry.handle
        )));
    }
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
    let me = ctx.resolve_for_guard()?;
    ctx.claims_mut()?;
    if recorded != Some(Ledger::Detached) {
        append_note(&mut task, &owner, &note)?;
        save(&mut ctx, &mut task)?;
        super::follow_holder(&mut ctx, &task.id, Some(&me), "the detach landed");
    }
    if present {
        std::fs::remove_file(&target)?;
        crate::written::record(&ctx.project.records, &target);
        let dir = target.parent().expect("attachment has a directory");
        if std::fs::read_dir(dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
        }
    }
    super::finish(&mut ctx);
    Ok(Output::Detach(DetachOut {
        id: task.id.to_string(),
        name,
        path: target.display().to_string(),
        removed: present,
        warnings: ctx.warnings,
    }))
}
