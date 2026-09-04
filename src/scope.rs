use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::path::Path;

/// How a prefix reached the resolver, which decides the error kind when it cannot be
/// opened (spec §3.1): an id that cannot be followed is `unresolvable_id`; a prefix a
/// person typed is `config`. A registered project whose config disagrees with the
/// registry is `config` either way.
#[derive(Clone, Copy)]
pub enum Origin<'a> {
    Id(&'a TaskId),
    Prefix,
}

/// The config file exists and is a file. `try_exists` so a permission error surfaces
/// instead of reading as absent; `is_file` so a directory at that path is not mistaken
/// for a config. Every reachability test in the tool goes through this one function.
pub fn has_config(root: &Path) -> Result<bool> {
    let config = root.join(CONFIG_REL);
    Ok(config.try_exists()? && config.is_file())
}

/// Opens the project registered as `prefix`, applying the three checks every
/// cross-project path shares: registered, config present, prefix agrees.
pub fn open_registered(registry: &Registry, prefix: &str, origin: Origin) -> Result<Project> {
    let fail = |detail: String| match origin {
        Origin::Id(id) => Error::UnresolvableId(format!("{id}: {detail}")),
        Origin::Prefix => Error::Config(detail),
    };
    let Some(root) = registry.project_root(prefix) else {
        return Err(fail(format!("no project registered as {prefix:?}")));
    };
    if !has_config(root)? {
        return Err(fail(format!(
            "project {prefix:?} at {} has no {CONFIG_REL}; run `tasks init` there",
            root.display()
        )));
    }
    let project = Project::open(root)?;
    if project.prefix != prefix {
        return Err(Error::Config(format!(
            "registry maps {prefix:?} to {}, whose prefix is {:?}; fix the registry",
            root.display(),
            project.prefix
        )));
    }
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with(prefix: &str, root: &Path) -> Registry {
        let mut registry = Registry::default();
        registry.register(prefix, root).unwrap();
        registry
    }

    fn write_config(root: &Path, prefix: &str) {
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::write(root.join(CONFIG_REL), format!("prefix = \"{prefix}\"\n")).unwrap();
    }

    #[test]
    fn unregistered_prefix_kind_follows_origin() {
        let registry = Registry::default();
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::UnresolvableId(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn missing_config_kind_follows_origin() {
        let dir = tempfile::tempdir().unwrap();
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::UnresolvableId(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn prefix_mismatch_is_config_whatever_the_origin() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "fam");
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Id(&id)),
            Err(Error::Config(_))
        ));
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn opens_a_matching_project() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "sci");
        let registry = registry_with("sci", dir.path());
        let project = open_registered(&registry, "sci", Origin::Prefix).unwrap();
        assert_eq!(project.prefix, "sci");
        assert_eq!(project.root, dir.path().canonicalize().unwrap());
    }
}
