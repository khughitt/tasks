use crate::error::Result;
use crate::link::{GitTree, holds_records, project_of_tree, refuse_moved};
use crate::output::{Output, WhereOut, WhereProject};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Kind, Project};
use std::path::{Path, PathBuf};

/// Project and stage policy at `path`, without crossing a git boundary
/// (ops linked-checkouts decision 4): the walk for `tasks/.config.toml` stops at the
/// repository's top level, and a hit is `records_moved` as in `locate`; on a miss the
/// pairing and link steps match its main checkout. An older in-tree worktree without
/// a config can still report the registered stage. `where` reads the registry only inside
/// git (decision 4), so outside git it can open a copied historical config that `locate`,
/// which reads the registry on every walk hit, would refuse.
pub fn run(path: Option<PathBuf>, dir: Option<&Path>) -> Result<Output> {
    let start = super::start_dir(dir)?;
    let path = crate::repo::canonical_start(&match path {
        Some(path) => start.join(path),
        None => start,
    })?;
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().expect("a file has a parent").to_path_buf()
    };
    let tree = GitTree::of(&dir)?;
    let walked = walk(&dir, tree.as_ref().map(|tree| tree.top.as_path()))?;
    let (project, holds) = match tree {
        None => (walked.map(|project| shown(project, None)), false),
        Some(tree) => {
            let registry = Registry::load()?;
            let main = if tree.is_main() {
                Some(tree.top.clone())
            } else if let Some(main) = tree.main_checkout() {
                Some(main)
            } else {
                tree.worktrees()?.into_iter().next()
            };
            let project = match walked {
                Some(project) => {
                    let authority = crate::scope::config_dir(&registry, &project.prefix);
                    let corresponding = main.as_ref().and_then(|main| {
                        project
                            .records
                            .strip_prefix(&tree.top)
                            .ok()
                            .map(|rel| main.join(rel))
                    });
                    let registered_root = registry
                        .project_root(&project.prefix)
                        .map(crate::rename::root_identity)
                        .transpose()?;
                    let is_registered_tree =
                        corresponding.is_some() && corresponding == registered_root;
                    if is_registered_tree {
                        refuse_moved(&registry, &project)?;
                    }
                    let registered = authority
                        .as_deref()
                        .filter(|root| *root == project.records || is_registered_tree);
                    Some(shown(project, registered))
                }
                None => {
                    let project = match project_of_tree(&registry, &tree)? {
                        Some(project) => Some(project),
                        None => registered_in_tree(&registry, main.as_deref())?,
                    };
                    project.map(|project| {
                        let registered = project.records.clone();
                        shown(project, Some(&registered))
                    })
                }
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

fn registered_in_tree(registry: &Registry, main: Option<&Path>) -> Result<Option<Project>> {
    let Some(main) = main else { return Ok(None) };
    for (prefix, root) in &registry.projects {
        if registry.records_dir(prefix).is_none() && crate::rename::root_identity(root)? == main {
            let project =
                crate::scope::open_registered(registry, prefix, crate::scope::Origin::Prefix)?;
            return Ok((project.kind == Kind::InTree).then_some(project));
        }
    }
    Ok(None)
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

fn shown(project: Project, registered: Option<&Path>) -> WhereProject {
    let mut retired = project.retired.is_some();
    // Project routing still describes the invoking worktree. Policy uses the
    // registered config even when this worktree predates a declaration change.
    let stage = match registered {
        Some(root) if root != project.records => Project::open(root).and_then(|authority| {
            if authority.prefix != project.prefix {
                return Err(crate::error::Error::Config(format!(
                    "{}: expected registered prefix {}",
                    root.join(CONFIG_REL).display(),
                    project.prefix
                )));
            }
            retired = authority.retired.is_some();
            authority.declared_stage()
        }),
        _ => project.declared_stage(),
    };
    let (stage, stage_error) = stage.map_or_else(
        |error| (None, Some(error.to_string())),
        |stage| (stage, None),
    );
    WhereProject {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        checkout: project.checkout.display().to_string(),
        records: project.records.display().to_string(),
        kind: project.kind,
        linked: project.kind == Kind::Linked,
        registered: registered.is_some(),
        retired,
        stage,
        stage_error,
        color: project.color,
    }
}
