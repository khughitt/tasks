//! Successor migration: carry, drop or close every open task of a project from a reviewed
//! decision list. See docs/specs/2026-11-02-successor-migration-design.md (records).

pub mod draft;
pub mod list;

use crate::commands::Ctx;
use crate::registry::Registry;
use crate::repo::Project;

pub struct MigrateCtx {
    pub source: Ctx,
    pub target: Project,
    pub registry: Registry,
}
