//! One Git repository, one stage: registered active projects whose code checkouts share a
//! repository must declare the same stage, since test policy keys on the Git top level
//! (docs/configuration.md).
use super::discovery::{Catalogue, EntryState};
use crate::error::Result;
use crate::registry::Registry;
use crate::repo::{Project, ProjectStage};
use std::path::{Path, PathBuf};

/// A registered project sharing `project`'s Git repository under a different stage.
pub struct Conflict {
    pub prefix: String,
    pub stage: ProjectStage,
    pub repository: PathBuf,
}

fn repository(checkout: &Path) -> Result<Option<PathBuf>> {
    if !checkout.is_dir() {
        return Ok(None);
    }
    Ok(crate::link::GitTree::of(checkout)?
        .map(|tree| tree.common.canonicalize().unwrap_or(tree.common)))
}

/// Registered, unretired projects other than `project` whose checkout shares its Git
/// repository and declares another valid stage. Only projects whose stage differs are
/// asked for their repository. An unreadable repository of another project is that
/// project's own check to report; `project`'s own is an error here.
pub fn conflicts(
    registry: &Registry,
    catalogue: &Catalogue,
    project: &Project,
    stage: ProjectStage,
) -> Result<Vec<Conflict>> {
    let live = registry.canonical_prefix(&project.prefix);
    let mut ours = None;
    let mut found = Vec::new();
    for (prefix, state) in &catalogue.entries {
        let EntryState::Read(other) = state else {
            continue;
        };
        if prefix == live || !registry.projects.contains_key(prefix) || other.retired.is_some() {
            continue;
        }
        let Ok(Some(other_stage)) = other.declared_stage() else {
            continue;
        };
        if other_stage == stage {
            continue;
        }
        let Ok(Some(theirs)) = repository(&other.checkout) else {
            continue;
        };
        if ours.is_none() {
            match repository(&project.checkout)? {
                Some(path) => ours = Some(path),
                None => return Ok(Vec::new()),
            }
        }
        if ours.as_ref() == Some(&theirs) {
            found.push(Conflict {
                prefix: prefix.clone(),
                stage: other_stage,
                repository: theirs,
            });
        }
    }
    Ok(found)
}

pub fn name(stage: ProjectStage) -> &'static str {
    match stage {
        ProjectStage::Experimental => "experimental",
        ProjectStage::PreRelease => "pre-release",
        ProjectStage::Supported => "supported",
    }
}
