use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: BTreeMap<String, PathBuf>,
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
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, toml::to_string(self).expect("registry serializes"))?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    pub fn register(&mut self, prefix: &str, root: &Path) -> Result<()> {
        if let Some(existing) = self.projects.get(prefix)
            && existing != root
        {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is already registered to {}",
                existing.display()
            )));
        }
        self.projects.insert(prefix.into(), root.into());
        Ok(())
    }

    pub fn project_root(&self, prefix: &str) -> Option<&Path> {
        self.projects.get(prefix).map(PathBuf::as_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
