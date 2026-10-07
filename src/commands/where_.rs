use crate::error::Result;
use crate::link::{GitTree, holds_records, linked_project};
use crate::output::{Output, WhereOut, WhereProject};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::path::{Path, PathBuf};

/// Which project a command run at `path` would open, without crossing a git boundary
/// (ops linked-checkouts decision 4): the walk for `tasks/.config.toml` stops at the
/// repository's top level, then the link step matches its main checkout. Outside git it
/// walks as `locate` does and reads no registry.
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
        None => (walked.map(|p| shown(p, false)), false),
        Some(tree) => {
            let registry = Registry::load()?;
            let project = match walked {
                Some(project) => Some(shown(project, false)),
                None => linked_project(&registry, &tree)?.map(|p| shown(p, true)),
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

fn shown(project: Project, linked: bool) -> WhereProject {
    WhereProject {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        checkout: linked
            .then(|| project.checkout.map(|c| c.display().to_string()))
            .flatten(),
        linked,
    }
}
