use super::{Ctx, append_note, load, owner_name, save};
use crate::attachments::{self, EntryState, Ledger};
use crate::error::{Error, Result};
use crate::multipart::{
    self,
    audit::DeclState,
    catalog::{Catalog, CatalogPart},
    content::sha256_hex,
    declaration::Declaration,
    publication::{self, Publication},
};
use crate::output::{AttachOut, DetachOut, FileInfo, MultipartOut, Output};
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
    multipart: bool,
    name: Option<String>,
    caption: Option<String>,
) -> Result<Output> {
    let mut task = load(&mut ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    let max = if multipart {
        ctx.project
            .multipart
            .max_bytes
            .min(multipart::RECOVERY_MAX_BYTES)
    } else {
        ctx.project.attachments_max_bytes
    };
    if multipart
        && (matches!(&source, Source::Clipboard)
            || matches!(&source, Source::Paths(paths) if paths.len() != 1))
    {
        return Err(Error::Validation(
            "--multipart needs one path or a single - with --name".into(),
        ));
    }
    if multipart && (max == 0 || ctx.project.multipart.max_parts == 0) {
        return Err(Error::MultipartTooLarge(
            "multipart admissions disabled by multipart_max_bytes or multipart_max_parts".into(),
        ));
    }
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
                let bytes = read_source(file, max, &path.display().to_string(), multipart)
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
                read_source(std::io::stdin().lock(), max, "stdin", multipart)?,
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
    let (sources, declaration, mut validation) = if multipart {
        let (name, bytes) = sources.into_iter().next().expect("one source checked");
        multipart::names::validate_logical_name(&name)?;
        refuse_reserved(&task, &name)?;
        if attachments::ledger(&task).get(&name) == Some(&Ledger::Attached) {
            return Err(Error::AttachmentExists(format!(
                "{} already has {name}",
                task.id
            )));
        }
        multipart::content::builtin(&name, &bytes)?;
        let validation = publication::validate(&ctx.project, &name, &bytes)?;
        if !validation.passed {
            return Err(Error::MultipartInvalidContent(format!(
                "multipart whole-document validation {}: {}",
                publication::validation_cause(&validation),
                serde_json::to_string(&validation).expect("validation serializes")
            )));
        }
        let ranges = multipart::cut::cut(&bytes, ctx.project.attachments_max_bytes)?;
        let limit = ctx
            .project
            .multipart
            .max_parts
            .min(multipart::RECOVERY_MAX_PARTS);
        if ranges.len() > limit {
            return Err(Error::MultipartTooLarge(format!(
                "{} parts exceeds [attachments] multipart_max_parts = {limit}",
                ranges.len()
            )));
        }
        let parts: Vec<_> = ranges
            .iter()
            .enumerate()
            .map(|(index, range)| CatalogPart {
                name: multipart::names::part_name(&name, index + 1),
                bytes: range.len() as u64,
                sha256: sha256_hex(&bytes[range.clone()]),
            })
            .collect();
        let catalog = Catalog {
            name: name.clone(),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            parts,
        };
        let catalog_bytes = catalog.to_bytes();
        if catalog_bytes.len() as u64 > ctx.project.attachments_max_bytes {
            return Err(Error::AttachmentTooLarge(format!(
                "multipart catalog exceeds [attachments] max_bytes = {}",
                ctx.project.attachments_max_bytes
            )));
        }
        let declaration = Declaration {
            name: name.clone(),
            bytes: bytes.len() as u64,
            parts: ranges.len(),
            sha256: catalog.sha256,
            catalog_sha256: sha256_hex(&catalog_bytes),
            caption: caption.clone(),
        };
        let mut sources: Vec<_> = ranges
            .iter()
            .enumerate()
            .map(|(index, range)| {
                (
                    multipart::names::part_name(&name, index + 1),
                    bytes[range.clone()].to_vec(),
                    Some(format!("part {}/{} of {name}", index + 1, ranges.len())),
                )
            })
            .collect();
        sources.push((
            multipart::names::catalog_name(&name),
            catalog_bytes,
            Some(format!("catalog of {name}")),
        ));
        (sources, Some(declaration), Some(validation))
    } else {
        (
            sources
                .into_iter()
                .map(|(name, bytes)| (name, bytes, caption.clone()))
                .collect(),
            None,
            None,
        )
    };
    let ledger = attachments::ledger(&task);
    let mut pending = Vec::new();
    let mut files = Vec::new();
    for (name, bytes, caption) in sources {
        refuse_reserved(&task, &name)?;
        let note = attachments::attached_note(&name, bytes.len() as u64, caption.as_deref());
        crate::format::validate_note_text(&note)?;
        let target = attachments::task_dir(&ctx.project, &task.id).join(&name);
        let live = ledger.get(&name) == Some(&Ledger::Attached);
        let state = attachments::entry_state(&target)?;
        if live && (multipart || state != EntryState::Absent) {
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

    if let Some(declaration) = &declaration {
        append_note(&mut task, &owner, &declaration.note())?;
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
    let publication = if let Some(declaration) = &declaration {
        let commits = super::finish_report(&mut ctx);
        if let Some(cause) =
            multipart::git_objects::commit_cause(ctx.project.commit_records, &ctx.warnings)
        {
            Some(Publication::local(&task.id, &declaration.name, cause))
        } else if let Some(committed) = commits.into_iter().find(|commit| {
            commit
                .paths
                .contains(&ctx.project.records.join(format!("tasks/{}.md", task.id)))
                && ctx.project.records.starts_with(&commit.toplevel)
        }) {
            let (result, report) = publication::report(
                &ctx.project,
                &task.id,
                &declaration.name,
                committed.commit,
                &mut ctx.warnings,
            );
            validation = report;
            Some(result)
        } else {
            Some(Publication::local(
                &task.id,
                &declaration.name,
                "verify_failed",
            ))
        }
    } else {
        super::finish(&mut ctx);
        None
    };
    let multipart = declaration.map(|d| MultipartOut {
        catalog: multipart::names::catalog_name(&d.name),
        name: d.name,
        bytes: d.bytes,
        sha256: d.sha256,
        parts: d.parts,
        caption: d.caption,
    });
    Ok(Output::Attach(AttachOut {
        id: task.id.to_string(),
        files,
        multipart,
        publication,
        validation,
        warnings: ctx.warnings,
    }))
}

fn read_source(
    reader: impl std::io::Read,
    max: u64,
    what: &str,
    multipart: bool,
) -> Result<Vec<u8>> {
    attachments::read_capped(reader, max, what).map_err(|error| match error {
        Error::AttachmentTooLarge(_) if multipart => Error::MultipartTooLarge(format!(
            "{what} exceeds [attachments] multipart_max_bytes = {max}"
        )),
        Error::AttachmentTooLarge(_) => {
            error.with_suffix("; use --multipart for one oversized UTF-8 text source")
        }
        other => other,
    })
}

fn refuse_reserved(task: &crate::model::Task, name: &str) -> Result<()> {
    let declarations = multipart::audit::declarations(task);
    if let Some(logical) = multipart::audit::reserved_names(task).get(name)
        && matches!(declarations.get(logical), Some(DeclState::Live(_)))
    {
        if logical == name {
            return Err(Error::AttachmentExists(format!(
                "{} already has multipart attachment {name}; run tasks commit, then tasks export {} {name} --at HEAD --verify",
                task.id, task.id
            )));
        }
        return Err(Error::MultipartMember(format!(
            "{name} is a member of multipart attachment {logical}; use its logical name"
        )));
    }
    Ok(())
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
