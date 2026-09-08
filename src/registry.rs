use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: BTreeMap<String, PathBuf>,
    /// Retired prefix -> the live prefix it now resolves to. Never chains: every target is
    /// a live prefix, enforced on load, so resolution is always one hop.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl Registry {
    pub fn path() -> Result<PathBuf> {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home).join("tasks/projects.toml"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".config/tasks/projects.toml"));
        }
        Err(Error::Config(
            "neither XDG_CONFIG_HOME nor HOME is set".into(),
        ))
    }

    /// Serialize the full read-modify-write; atomic replacement alone can lose updates.
    pub fn lock() -> Result<crate::claims::MutationLock> {
        crate::claims::MutationLock::acquire_at(&Self::path()?.with_file_name("projects.lock"))
    }

    pub fn load() -> Result<Registry> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> Result<Registry> {
        if !path.exists() {
            return Ok(Registry::default());
        }
        let text = std::fs::read_to_string(path)?;
        let mut registry: Registry = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        for (prefix, root) in &mut registry.projects {
            if let Some(rest) = root.to_str().and_then(|root| root.strip_prefix("~/")) {
                let home = std::env::var_os("HOME").ok_or_else(|| {
                    Error::Config(format!("project {prefix} uses ~ but HOME is not set"))
                })?;
                *root = PathBuf::from(home).join(rest);
            }
        }
        for (alias, target) in &registry.aliases {
            if registry.projects.contains_key(alias) {
                return Err(Error::Config(format!(
                    "{}: alias {alias:?} collides with a live prefix",
                    path.display()
                )));
            }
            if !registry.projects.contains_key(target) {
                return Err(Error::Config(format!(
                    "{}: alias {alias:?} targets {target:?}, which is not a registered project",
                    path.display()
                )));
            }
        }
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(
            path,
            toml::to_string(self)
                .expect("registry serializes")
                .as_bytes(),
        )
    }

    /// Claims `prefix` for `root`. A prefix already pointing somewhere else is a conflict,
    /// never a silent takeover; the message names both ways out.
    pub fn register(&mut self, prefix: &str, root: &Path) -> Result<()> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is retired; it resolves to {target:?}"
            )));
        }
        if let Some(existing) = self.projects.get(prefix)
            && existing != root
        {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is already registered to {}; re-point it with \
                 `tasks init --prefix {prefix} --force`, or drop it with \
                 `tasks unregister {prefix}`",
                existing.display()
            )));
        }
        self.projects.insert(prefix.into(), root.into());
        Ok(())
    }

    /// Points `prefix` at `root` whatever it pointed at before, returning the displaced
    /// root when that changed anything. The deliberate override behind `init --force`.
    pub fn repoint(&mut self, prefix: &str, root: &Path) -> Result<Option<PathBuf>> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is retired; it resolves to {target:?}"
            )));
        }
        Ok(match self.projects.insert(prefix.into(), root.into()) {
            Some(previous) if previous != root => Some(previous),
            _ => None,
        })
    }

    /// Removes a project and every alias that targeted it. Leaving an alias behind would
    /// dangle the load invariant and fail every later command; and once the project is
    /// gone its ids cannot resolve anyway, alias or not.
    pub fn unregister(&mut self, prefix: &str) -> Result<(PathBuf, Vec<String>)> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "{prefix:?} is a retired prefix of {target:?}; unregister {target:?} to remove the project"
            )));
        }
        let root = self
            .projects
            .remove(prefix)
            .ok_or_else(|| Error::Config(format!("no project registered as {prefix:?}")))?;
        let dropped: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, target)| target.as_str() == prefix)
            .map(|(alias, _)| alias.clone())
            .collect();
        for alias in &dropped {
            self.aliases.remove(alias);
        }
        Ok((root, dropped))
    }

    pub fn project_root(&self, prefix: &str) -> Option<&Path> {
        self.projects.get(prefix).map(PathBuf::as_path)
    }

    /// The live prefix `prefix` resolves to. One hop: aliases never chain (§2).
    pub fn canonical_prefix<'a>(&'a self, prefix: &'a str) -> &'a str {
        self.aliases.get(prefix).map_or(prefix, String::as_str)
    }

    /// The same rule applied to an id. Only the prefix component moves; the hex is
    /// preserved by a rename, which is what makes this a rule rather than an index.
    pub fn canonical_id(&self, id: &TaskId) -> TaskId {
        let prefix = self.canonical_prefix(&id.prefix);
        if prefix == id.prefix {
            return id.clone();
        }
        TaskId {
            prefix: prefix.to_string(),
            hex: id.hex.clone(),
        }
    }

    /// Whether a prefix may be claimed: a live prefix and a retired one are both taken,
    /// because an id must never mean two projects.
    #[allow(dead_code)]
    pub fn is_taken(&self, prefix: &str) -> bool {
        self.projects.contains_key(prefix) || self.aliases.contains_key(prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskId;

    #[test]
    fn roundtrip_and_conflict() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("tasks/projects.toml");
        let mut r = Registry::load_from(&path).unwrap();
        assert!(r.projects.is_empty());
        r.register("sci", std::path::Path::new("/tmp/sci")).unwrap();
        r.register("sci", std::path::Path::new("/tmp/sci")).unwrap();
        assert!(
            r.register("sci", std::path::Path::new("/tmp/other"))
                .is_err()
        );
        r.save_to(&path).unwrap();
        let r2 = Registry::load_from(&path).unwrap();
        assert_eq!(
            r2.project_root("sci").unwrap(),
            std::path::Path::new("/tmp/sci")
        );
        assert!(r2.project_root("nope").is_none());
    }

    #[test]
    fn repoint_replaces_and_reports_the_displaced_root() {
        let mut r = Registry::default();
        assert_eq!(r.repoint("sci", Path::new("/tmp/a")).unwrap(), None);
        assert_eq!(
            r.repoint("sci", Path::new("/tmp/b")).unwrap(),
            Some(PathBuf::from("/tmp/a"))
        );
        assert_eq!(r.project_root("sci").unwrap(), Path::new("/tmp/b"));
        assert_eq!(
            r.repoint("sci", Path::new("/tmp/b")).unwrap(),
            None,
            "re-pointing at the same root displaces nothing"
        );
    }

    #[test]
    fn unregister_removes_once_and_then_reports_the_prefix_is_absent() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        assert_eq!(
            r.unregister("sci").unwrap(),
            (PathBuf::from("/tmp/a"), Vec::new())
        );
        assert!(r.project_root("sci").is_none());
        assert_eq!(r.unregister("sci").unwrap_err().kind(), "config");
        r.register("sci", Path::new("/tmp/c")).unwrap();
        assert_eq!(
            r.project_root("sci").unwrap(),
            Path::new("/tmp/c"),
            "the prefix is free again after removal"
        );
    }

    #[test]
    fn expands_leading_tilde_on_load() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("projects.toml");
        std::fs::write(&path, "[projects]\nsci = \"~/d/science\"\n").unwrap();
        unsafe {
            std::env::set_var("HOME", home.path());
        }
        let r = Registry::load_from(&path).unwrap();
        assert_eq!(
            r.project_root("sci").unwrap(),
            home.path().join("d/science")
        );
    }

    #[test]
    fn aliases_canonicalize_and_reserve_names() {
        let mut r = Registry::default();
        r.register("dots", Path::new("/tmp/dotfiles")).unwrap();
        r.aliases.insert("dot".into(), "dots".into());

        assert_eq!(r.canonical_prefix("dot"), "dots");
        assert_eq!(r.canonical_prefix("dots"), "dots");
        assert_eq!(
            r.canonical_prefix("nope"),
            "nope",
            "an unknown prefix is left alone"
        );

        let retired = TaskId::parse("dot-a00088").unwrap();
        assert_eq!(r.canonical_id(&retired).to_string(), "dots-a00088");
        let live = TaskId::parse("dots-a00088").unwrap();
        assert_eq!(r.canonical_id(&live).to_string(), "dots-a00088");

        assert!(r.is_taken("dots"), "a live prefix is taken");
        assert!(r.is_taken("dot"), "an alias is taken");
        assert!(!r.is_taken("free"));
    }

    #[test]
    fn load_rejects_a_dangling_or_colliding_alias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");

        std::fs::write(
            &path,
            "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndot = \"gone\"\n",
        )
        .unwrap();
        let error = Registry::load_from(&path).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(error.to_string().contains("dot"), "{error}");

        std::fs::write(
            &path,
            "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndots = \"dots\"\n",
        )
        .unwrap();
        assert_eq!(Registry::load_from(&path).unwrap_err().kind(), "config");
    }

    #[test]
    fn a_registry_without_an_aliases_table_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        std::fs::write(&path, "[projects]\nsci = \"/tmp/a\"\n").unwrap();
        let r = Registry::load_from(&path).unwrap();
        assert!(r.aliases.is_empty());
        assert_eq!(r.project_root("sci").unwrap(), Path::new("/tmp/a"));
    }
}
