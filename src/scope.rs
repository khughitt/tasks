use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
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

/// The first two outcomes of spec §3.2: a root that exists and holds a config. The third
/// (a config that does not parse or disagrees with the key) surfaces from
/// `open_registered`, which every reachable root then goes through.
pub fn is_reachable(root: &Path) -> Result<bool> {
    Ok(root.try_exists()? && has_config(root)?)
}

/// The two warnings every registry-wide command shares: an empty registry, and a `cwd`
/// inside a project the registry does not know, which would otherwise vanish from its
/// own portfolio view. The only look at `cwd` a wide command takes, and read-only.
pub fn registry_warnings(registry: &Registry, cwd: &Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if registry.projects.is_empty() {
        warnings.push("registry is empty".into());
    }
    match Project::locate(cwd) {
        Ok(current) if !registry.projects.contains_key(&current.prefix) => {
            warnings.push(format!(
                "current project {} is not registered",
                current.prefix
            ));
        }
        Ok(_) | Err(Error::NoProject(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(warnings)
}

/// What a read command looks at: one project, or every reachable registered project.
pub enum Scope {
    Local(Project),
    All(Vec<Project>),
}

impl Scope {
    /// Every registered project that is reachable, in registry (prefix) order, plus the
    /// warnings the walk produced. Never locates a local project; the only look at `cwd`
    /// is to warn when it lies inside a project the registry does not know.
    pub fn open_all(registry: &Registry, cwd: &Path) -> Result<(Scope, Vec<String>)> {
        let mut warnings = registry_warnings(registry, cwd)?;
        let mut projects = Vec::new();
        for (prefix, root) in &registry.projects {
            if !is_reachable(root)? {
                warnings.push(format!(
                    "project {prefix} at {} is unreachable",
                    root.display()
                ));
                continue;
            }
            projects.push(open_registered(registry, prefix, Origin::Prefix)?);
        }
        Ok((Scope::All(projects), warnings))
    }

    pub fn projects(&self) -> &[Project] {
        match self {
            Scope::Local(project) => std::slice::from_ref(project),
            Scope::All(projects) => projects,
        }
    }

    #[allow(dead_code)] // Used by later registry-wide commands.
    pub fn prefixes(&self) -> Vec<String> {
        self.projects()
            .iter()
            .map(|project| project.prefix.clone())
            .collect()
    }

    /// The union of every project's tasks, projects in scope order.
    pub fn scan(&self) -> Result<Vec<Task>> {
        let mut all = Vec::new();
        for project in self.projects() {
            all.extend(project.scan()?);
        }
        Ok(all)
    }

    /// One scan per project, for commands that group by project (`tree`).
    #[allow(dead_code)] // Used by the later registry-wide tree command.
    pub fn scan_each(&self) -> Result<Vec<(&Project, Vec<Task>)>> {
        self.projects()
            .iter()
            .map(|project| Ok((project, project.scan()?)))
            .collect()
    }

    /// A project in scope answers for its own prefix; anything else goes through the
    /// registry, leniently, as dependency resolution always has.
    pub fn resolve_task(&self, registry: &Registry, id: &TaskId) -> Result<Option<Task>> {
        match self
            .projects()
            .iter()
            .find(|project| project.prefix == id.prefix)
        {
            Some(project) => crate::resolve::read_present(project, id),
            None => crate::resolve::resolve_registered(registry, id),
        }
    }
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

    #[test]
    fn open_all_skips_unreachable_and_errors_on_malformed() {
        let good = tempfile::tempdir().unwrap();
        write_config(good.path(), "sci");
        let gone = tempfile::tempdir().unwrap();
        let mut registry = registry_with("sci", good.path());
        registry.register("fam", gone.path()).unwrap();
        let (scope, warnings) = Scope::open_all(&registry, gone.path()).unwrap();
        assert_eq!(scope.prefixes(), ["sci"]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("project fam at "), "{warnings:?}");

        std::fs::create_dir_all(gone.path().join("tasks")).unwrap();
        std::fs::write(gone.path().join(CONFIG_REL), "not toml = [").unwrap();
        assert!(matches!(
            Scope::open_all(&registry, gone.path()),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn scope_and_dependency_resolution_share_config_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(CONFIG_REL)).unwrap();
        let registry = registry_with("sci", dir.path());
        let id = TaskId::parse("sci-000001").unwrap();
        assert!(!is_reachable(dir.path()).unwrap());
        assert!(
            crate::resolve::resolve_registered(&registry, &id)
                .unwrap()
                .is_none(),
            "dependency resolution treats a non-file config as unreachable"
        );
        let (scope, warnings) = Scope::open_all(&registry, dir.path()).unwrap();
        assert!(scope.projects().is_empty());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(matches!(
            open_registered(&registry, "sci", Origin::Prefix),
            Err(Error::Config(_))
        ));

        std::fs::remove_dir_all(dir.path().join(CONFIG_REL)).unwrap();
        write_config(dir.path(), "zzz");
        assert!(matches!(
            crate::resolve::resolve_registered(&registry, &id),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn open_all_warns_on_empty_registry_and_unregistered_cwd() {
        let registry = Registry::default();
        let nowhere = tempfile::tempdir().unwrap();
        let (_, warnings) = Scope::open_all(&registry, nowhere.path()).unwrap();
        assert_eq!(warnings, ["registry is empty"]);

        let lone = tempfile::tempdir().unwrap();
        write_config(lone.path(), "lon");
        let (_, warnings) = Scope::open_all(&registry, lone.path()).unwrap();
        assert_eq!(
            warnings,
            ["registry is empty", "current project lon is not registered"]
        );
    }
}
