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
    let mut registry = Registry::load()?;
    // The registry is only mutated in memory here: `save` runs after `Project::init`, so
    // an init that fails leaves the registry exactly as it was.
    let displaced = if force {
        registry.repoint(&prefix, &root)
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
