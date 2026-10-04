use crate::error::{Error, Result};
use crate::output::{InitOut, Output};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

const SKILL_REL: &str = "skills/tasks/SKILL.md";

pub fn run(dir: Option<&Path>, prefix: Option<String>, force: bool) -> Result<Output> {
    let root = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    }
    .canonicalize()?;
    let prefix = match prefix {
        Some(prefix) => prefix,
        None => default_prefix(&root)?,
    };
    // Lock order (retired-projects plan): the project lock, then the registry lock, as
    // rename takes them. `init --force` repoints the root `retire` and every writer consult.
    let _project_lock = crate::claims::MutationLock::acquire(&prefix)?;
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    for inventory in crate::rename::inventory::Inventory::pending()? {
        if inventory.target == prefix {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is reserved by unfinished rename {} -> {}",
                inventory.source, inventory.target
            )));
        }
    }
    super::reject_pending_rename_at(Some(&root), &prefix)?;
    // The registry is only mutated in memory here: `save` runs after `Project::init`, so
    // an init that fails leaves the registry exactly as it was.
    if force {
        reject_repointing_a_retired_project(&registry, &prefix, &root)?;
    }
    let displaced = if force {
        registry.repoint(&prefix, &root)?
    } else {
        registry.register(&prefix, &root)?;
        None
    };
    let project = Project::init(&root, &prefix)?;
    registry.save()?;
    let mut warnings = Vec::new();
    if let Some(previous) = displaced {
        warnings.push(format!(
            "prefix {prefix:?} was registered to {}",
            previous.display()
        ));
    }
    if !skill_installed(&project.root) {
        warnings.push(
            "no tasks skill found at user or project level; install skills/tasks (see README)"
                .into(),
        );
    }
    Ok(Output::Init(InitOut {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        warnings,
        aliases: Vec::new(),
    }))
}

fn default_prefix(root: &Path) -> Result<String> {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Error::Config("cannot derive a prefix from this directory; pass --prefix".into())
        })?;
    let prefix = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(3)
        .collect::<String>()
        .to_lowercase();
    if !crate::model::is_valid_prefix(&prefix) {
        return Err(Error::Config(format!(
            "derived prefix {prefix:?} is invalid; pass --prefix"
        )));
    }
    Ok(prefix)
}

fn skill_installed(root: &Path) -> bool {
    let mut candidates = vec![root.join(".claude").join(SKILL_REL)];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".claude").join(SKILL_REL));
        candidates.push(home.join(".agents").join(SKILL_REL));
    }
    candidates.iter().any(|path| path.is_file())
}

/// `init --force` moves the authority that `retire` and every writer consult, so it is
/// not done for a retired project: refused when the new root's config declares
/// `[retired]`, or when the currently registered root opens and declares it. An old root
/// that does not open does not block: repointing away from a moved or broken checkout is
/// what `--force` is for, and the new root is read either way.
fn reject_repointing_a_retired_project(
    registry: &Registry,
    prefix: &str,
    root: &Path,
) -> Result<()> {
    let refuse = |where_: &Path| {
        Err(Error::RetiredProject(format!(
            "{prefix} is retired (declared in {}); run `tasks unretire` there before re-pointing it",
            where_.display()
        )))
    };
    if root.join(crate::repo::CONFIG_REL).exists() && Project::open(root)?.retired.is_some() {
        return refuse(root);
    }
    if let Some(current) = registry.project_root(prefix)
        && current != root
        && let Ok(project) = Project::open(current)
        && project.retired.is_some()
    {
        return refuse(current);
    }
    Ok(())
}
