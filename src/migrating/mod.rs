//! Successor migration: carry, drop or close every open task of a project from a reviewed
//! decision list. See docs/project-operations.md (records).

pub mod apply;
pub mod classify;
pub mod commit;
pub mod draft;
pub mod freeze;
pub mod journal;
pub mod list;
pub mod preflight;
pub mod project;
pub mod release;
pub mod review;

use crate::commands::Ctx;
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

/// `id`'s record under a records directory: the layout every owned path of a run uses (the
/// journal's, the journal-less derivation's and the refusals naming them), so G9 and the
/// journal-less completion judge the same files.
pub fn record_path(records: &Path, id: &str) -> PathBuf {
    records.join(format!("tasks/{id}.md"))
}

/// One attachment file of `id` under a records directory; see `record_path`.
pub fn attachment_file(records: &Path, id: &str, name: &str) -> PathBuf {
    records.join(format!("tasks/files/{id}/{name}"))
}

pub struct MigrateCtx {
    pub source: Ctx,
    pub target: Project,
    pub registry: Registry,
}
