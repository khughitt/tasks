//! `tasks commit`: every uncommitted file under tasks/ in one commit, through the record
//! commit's procedure (record-commit spec §3.5). An explicit request, so it ignores the
//! `[git] commit` switch and fails loudly when it cannot commit.
use super::Ctx;
use crate::error::{Error, Result};
use crate::output::{CommitOut, Output};
use std::path::PathBuf;

pub fn run(ctx: Ctx) -> Result<Output> {
    let Some(files) = ctx.project.uncommitted_task_files()? else {
        return Err(Error::Validation(format!(
            "commit_unavailable: {} is not in a git repository (or git is missing)",
            ctx.project.root.display()
        )));
    };
    if files.is_empty() {
        return Ok(Output::Commit(CommitOut {
            committed: 0,
            paths: Vec::new(),
            warnings: ctx.warnings,
        }));
    }
    let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    let message = format!("chore(tasks): commit {} records", paths.len());
    let refused = crate::record_commit::commit_paths(&ctx.project.root, &paths, &message);
    if !refused.is_empty() {
        return Err(Error::Validation(refused.join("\n")));
    }
    Ok(Output::Commit(CommitOut {
        committed: files.len(),
        paths: files,
        warnings: ctx.warnings,
    }))
}
