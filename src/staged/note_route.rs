//! The ordinary-note route: accept a staged commit whose only change to the project is
//! notes appended to one existing record, without repeating the full audit.
//!
//! The route only ever accepts. Anything it cannot establish sends the commit to the full
//! staged audit, which gives the verdict. It qualifies a commit when all of these hold:
//!
//! - The project declares no retention recovery (recovery's boundary is decided against
//!   HEAD; those projects are small enough to audit in full).
//! - HEAD's records tree is a tree this host's binary already accepted in a staged check
//!   (see [`super::validated`]), checked against a HEAD that is HEAD or one of its
//!   ancestors.
//! - The staged tree differs from HEAD's in exactly one path, an existing regular task
//!   record `tasks/<id>.md` modified in place.
//! - Both versions parse, and the staged one differs only by `updated` and by inline notes
//!   appended after the existing ones: the same fields, body and note archives.
//! - No appended note is structured: none records attachments, multipart parts,
//!   retention, an archive or a move.
//!
//! Under those conditions every rule of the full audit sees the same input as it did for
//! the accepted tree, except the appended notes themselves, which no rule reads unless
//! they are structured, and the HEAD retention history of records HEAD has changed since
//! the accepted check. [`check`] validates that history. What the route does not repeat
//! is anything outside the staged tree that changed since the accepted check: other
//! projects, the code checkout, external documents and Git refs. The next full staged
//! audit or a plain `tasks check` reports those.

use super::snapshot::{self, Snapshot};
use super::validated::Passed;
use crate::error::Result;
use crate::format::PhysicalTask;
use crate::model::{Note, TaskId};
use crate::repo::Project;

/// Why a commit goes to the full staged audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Full {
    Recovery,
    NoHead,
    Unvalidated,
    NotOneRecord,
    NotAppend,
    Structured,
}

/// A commit the note route may accept, once [`check`] passes.
#[derive(Debug)]
pub(crate) struct Qualified {
    /// Records whose HEAD version changed since the accepted check.
    pub(crate) head_changed: Vec<TaskId>,
}

/// Whether a note records a structured operation that some rule of the audit reads.
fn structured(note: &Note) -> bool {
    crate::retained::publish::reserved(&note.text)
        || note.text.starts_with("archived-notes:")
        || note.text.starts_with("moved from ")
        || note.text.starts_with("moved to ")
}

fn record_id(path: &str) -> Option<TaskId> {
    let stem = path.strip_prefix("tasks/")?.strip_suffix(".md")?;
    if stem.contains('/') || stem.starts_with('.') {
        return None;
    }
    TaskId::parse(stem).ok()
}

fn physical(top: &std::path::Path, oid: &str, file: &str) -> Result<Option<PhysicalTask>> {
    let Ok(text) = String::from_utf8(snapshot::blob(top, oid)?) else {
        return Ok(None);
    };
    Ok(crate::format::parse_physical(&text, file).ok())
}

/// Decides the route for `snap` given the trees this host accepted for the project.
pub(crate) fn qualify(
    project: &Project,
    snap: &Snapshot,
    passed: &[Passed],
) -> Result<std::result::Result<Qualified, Full>> {
    if project.retention_recovery.is_some() {
        return Ok(Err(Full::Recovery));
    }
    let Some(head) = &snap.head else {
        return Ok(Err(Full::NoHead));
    };
    let Some(head_tree) = &head.tree else {
        return Ok(Err(Full::NoHead));
    };
    let mut base = None;
    for candidate in passed.iter().filter(|p| &p.tree == head_tree) {
        if let (Some(checked), Some(_)) = (&candidate.head, &candidate.head_tree)
            && snapshot::is_ancestor(&snap.top, checked, &head.commit)?
        {
            base = Some(candidate);
            break;
        }
    }
    let Some(base) = base else {
        return Ok(Err(Full::Unvalidated));
    };
    let changes = snapshot::changes(&snap.top, head_tree, &snap.tree)?;
    let [change] = &changes[..] else {
        return Ok(Err(Full::NotOneRecord));
    };
    let Some(id) = record_id(&change.path) else {
        return Ok(Err(Full::NotOneRecord));
    };
    if change.status != 'M' || change.old_mode != "100644" || change.new_mode != "100644" {
        return Ok(Err(Full::NotOneRecord));
    }
    let file = format!("tasks/{id}.md");
    let (Some(old), Some(new)) = (
        physical(&snap.top, &change.old_oid, &file)?,
        physical(&snap.top, &change.new_oid, &file)?,
    ) else {
        return Ok(Err(Full::NotAppend));
    };
    if old.id() != &id || new.id() != &id {
        return Ok(Err(Full::NotAppend));
    }
    let Some(appended) = old.appended_notes(&new) else {
        return Ok(Err(Full::NotAppend));
    };
    if appended.iter().any(structured) {
        return Ok(Err(Full::Structured));
    }
    let base_head_tree = base
        .head_tree
        .as_deref()
        .expect("a base is chosen only with its HEAD tree");
    let mut head_changed = Vec::new();
    for change in snapshot::changes(&snap.top, base_head_tree, head_tree)? {
        if let Some(changed) = record_id(&change.path) {
            head_changed.push(changed);
        }
    }
    Ok(Ok(Qualified { head_changed }))
}

/// Runs what the note route repeats: the lineage and the HEAD retention history of the
/// records HEAD changed since the accepted check. An error sends the commit to the full
/// audit, which reports it.
pub(crate) fn check(project: &Project, qualified: &Qualified) -> Result<()> {
    crate::retained::lineage::check_head(project, &qualified.head_changed)
}
