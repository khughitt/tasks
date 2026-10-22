use crate::error::Result;
use crate::link::{GitTree, holds_records, project_of_tree, refuse_moved};
use crate::output::{Output, WhereOut, WhereProject};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Kind, Project};
use std::path::{Path, PathBuf};

/// Which project a command run at `path` would open, without crossing a git boundary
/// (ops linked-checkouts decision 4): the walk for `tasks/.config.toml` stops at the
/// repository's top level, and a hit is `records_moved` as in `locate`; on a miss the
/// pairing and link steps match its main checkout. `where` reads the registry only inside
/// git (decision 4), so outside git it can open a copied historical config that `locate`,
/// which reads the registry on every walk hit, would refuse.
pub fn run(path: Option<PathBuf>, dir: Option<&Path>) -> Result<Output> {
    let start = super::start_dir(dir)?;
    let path = match path {
        Some(path) => start.join(path),
        None => start,
    }
    .canonicalize()?;
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().expect("a file has a parent").to_path_buf()
    };
    let tree = GitTree::of(&dir)?;
    let walked = walk(&dir, tree.as_ref().map(|tree| tree.top.as_path()))?;
    let (project, holds) = match tree {
        None => (walked.map(shown), false),
        Some(tree) => {
            let registry = Registry::load()?;
            let project = match walked {
                Some(project) => {
                    refuse_moved(&registry, &project)?;
                    Some(shown(project))
                }
                None => project_of_tree(&registry, &tree)?.map(shown),
            };
            (project, holds_records(&registry, &tree)?)
        }
    };
    Ok(Output::Where(WhereOut {
        project,
        holds_records: holds,
        warnings: Vec::new(),
    }))
}

fn walk(dir: &Path, stop: Option<&Path>) -> Result<Option<Project>> {
    for path in dir.ancestors() {
        if path.join(CONFIG_REL).is_file() {
            return Project::open(path).map(Some);
        }
        if Some(path) == stop {
            break;
        }
    }
    Ok(None)
}

fn shown(project: Project) -> WhereProject {
    WhereProject {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        checkout: project.checkout.display().to_string(),
        records: project.records.display().to_string(),
        kind: project.kind,
        linked: project.kind == Kind::Linked,
        color: project.color,
    }
}
