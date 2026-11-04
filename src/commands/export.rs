use super::Ctx;
use crate::error::{Error, Result};
use crate::multipart::publication::Integrity;
use crate::multipart::{
    audit, content::sha256_hex, git_objects::Revision, publication, validate::Validation,
};
use crate::output::{ExportOut, Output, RawOut};
use std::path::PathBuf;

pub enum ExportTarget {
    Path(PathBuf),
    Stdout,
    Verify,
}

pub fn run(
    ctx: Ctx,
    id: String,
    name: String,
    at: Option<String>,
    target: ExportTarget,
) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let revision = at
        .as_deref()
        .map(|at| Revision::resolve(&ctx.project.records, at))
        .transpose()?;
    let task = match &revision {
        Some(revision) => revision
            .task_at(&ctx.project, &id)?
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?,
        None => ctx.project.read_task(&id)?,
    };
    let bytes = audit::rebuild(&ctx.project, &task, &name, revision.as_ref())?;
    let validation: Option<Validation> = if matches!(target, ExportTarget::Verify) {
        Some(publication::validate(&ctx.project, &name, &bytes)?)
    } else {
        None
    };
    let path = match target {
        ExportTarget::Stdout => {
            return Ok(Output::Raw(RawOut {
                bytes,
                warnings: ctx.warnings,
            }));
        }
        ExportTarget::Path(path) => {
            if path.file_name().is_none() {
                return Err(Error::Validation("export output must name a file".into()));
            }
            write_atomic(&path, &bytes)?;
            Some(path.display().to_string())
        }
        ExportTarget::Verify => None,
    };
    Ok(Output::Export(ExportOut {
        id: id.to_string(),
        name,
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        commit: revision.map(|revision| revision.commit().to_string()),
        integrity: Integrity {
            passed: true,
            problems: Vec::new(),
        },
        validation,
        path,
        warnings: ctx.warnings,
    }))
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let temp = path.with_file_name(format!(
        ".tasks-export-{}-{:016x}",
        std::process::id(),
        fastrand::u64(..)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result.and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(temp);
        return Err(error.into());
    }
    Ok(())
}
