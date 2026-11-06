use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The host-local configuration, `$XDG_CONFIG_HOME/tasks/config.toml` or
/// `$HOME/.config/tasks/config.toml`. Relay identity and document commands belong to a
/// machine, while the per-project `tasks/.config.toml` is committed and syncs
/// between hosts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostConfig {
    pub headers: bool,
    pub relay_identity: bool,
    pub docs_schemes: BTreeMap<String, String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    #[serde(default)]
    relay: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileDocs {
    #[serde(default)]
    schemes: BTreeMap<String, String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileOutput {
    #[serde(default)]
    headers: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileShape {
    #[serde(default)]
    output: FileOutput,
    #[serde(default)]
    identity: FileIdentity,
    #[serde(default)]
    docs: FileDocs,
}

impl HostConfig {
    pub fn path() -> Result<PathBuf> {
        Self::path_from(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
    }

    pub fn path_from(config_home: Option<&OsStr>, home: Option<&OsStr>) -> Result<PathBuf> {
        if let Some(base) = config_home.filter(|base| Path::new(base).is_absolute()) {
            return Ok(Path::new(base).join("tasks/config.toml"));
        }
        if let Some(base) = home.filter(|base| Path::new(base).is_absolute()) {
            return Ok(Path::new(base).join(".config/tasks/config.toml"));
        }
        Err(Error::Config(
            "host configuration needs an absolute XDG_CONFIG_HOME or HOME".into(),
        ))
    }

    /// An absent file is not an error: every field takes its default. A present but
    /// unreadable or invalid file is, because it was written on purpose.
    pub fn load() -> Result<HostConfig> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> Result<HostConfig> {
        let shown = path.display().to_string();
        match std::fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(Error::Io(format!("{shown}: {error}"))),
        }
        let text = std::fs::read_to_string(path)
            .map_err(|error| Error::Io(format!("{shown}: {error}")))?;
        Self::parse(&text, &shown)
    }

    pub fn parse(text: &str, path: &str) -> Result<HostConfig> {
        let file: FileShape =
            toml::from_str(text).map_err(|error| Error::Config(format!("{path}: {error}")))?;
        for (scheme, command) in &file.docs.schemes {
            if scheme == "files" {
                return Err(Error::Config(format!(
                    "{path}: [docs.schemes] key files is reserved for task attachments"
                )));
            }
            if !crate::format::valid_doc_scheme(scheme) {
                return Err(Error::Config(format!(
                    "{path}: [docs.schemes] key {scheme:?} must match [a-z][a-z0-9-]*"
                )));
            }
            if command.trim().is_empty() || command.contains('\0') {
                return Err(Error::Config(format!(
                    "{path}: [docs.schemes] command for {scheme} must be nonblank and NUL-free"
                )));
            }
        }
        Ok(HostConfig {
            headers: file.output.headers,
            relay_identity: file.identity.relay,
            docs_schemes: file.docs.schemes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_headers_defaults_and_preserves_existing_sections() {
        assert!(!HostConfig::parse("", "c.toml").unwrap().headers);
        let config = HostConfig::parse(
            "[identity]\nrelay = true\n[docs.schemes]\nstub = 'provider'\n[output]\nheaders = true\n",
            "c.toml",
        ).unwrap();
        assert!(config.headers);
        assert!(config.relay_identity);
        assert_eq!(config.docs_schemes["stub"], "provider");
    }

    #[test]
    fn output_headers_rejects_unknown_and_nonboolean_values() {
        for text in [
            "[output]\nheaders = 'true'\n",
            "[output]\nheaders = 1\n",
            "[output]\nheader = true\n",
            "[output]\nheaders = true\nunknown = false\n",
            "[output]\n[output]\n",
        ] {
            assert!(HostConfig::parse(text, "c.toml").is_err(), "{text}");
        }
    }

    #[test]
    fn host_path_requires_absolute_base() {
        use std::ffi::OsStr;
        let absolute = OsStr::new("/config");
        let home = OsStr::new("/home/tester");
        for xdg in [None, Some(OsStr::new("")), Some(OsStr::new("relative"))] {
            assert_eq!(
                HostConfig::path_from(xdg, Some(home)).unwrap(),
                PathBuf::from("/home/tester/.config/tasks/config.toml")
            );
        }
        assert_eq!(
            HostConfig::path_from(Some(absolute), None).unwrap(),
            PathBuf::from("/config/tasks/config.toml")
        );
        assert_eq!(
            HostConfig::path_from(Some(absolute), Some(home)).unwrap(),
            PathBuf::from("/config/tasks/config.toml")
        );
        for home in [None, Some(OsStr::new("")), Some(OsStr::new("relative"))] {
            assert!(HostConfig::path_from(None, home).is_err());
        }
    }

    #[test]
    fn external_doc_config_keeps_identity_and_validates_commands() {
        let config = HostConfig::parse(
            "[identity]\nrelay = true\n[docs.schemes]\nx = 'printf'\n",
            "c.toml",
        )
        .unwrap();
        assert!(config.relay_identity);
        assert!(HostConfig::parse("[docs.schemes]\n", "c.toml").is_ok());
        for text in [
            "[docs]\nunknown = true\n",
            "[docs.schemes]\nDocs = 'cat'\n",
            "[docs.schemes]\nx = ''\n",
            "[docs.schemes]\nx = '   '\n",
            "[docs.schemes]\nx = 1\n",
        ] {
            assert!(HostConfig::parse(text, "c.toml").is_err(), "{text:?}");
        }
    }

    #[test]
    fn the_files_scheme_is_reserved() {
        let error = HostConfig::parse("[docs.schemes]\nfiles = 'cat'\n", "c.toml")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("files is reserved for task attachments"),
            "{error}"
        );
    }

    #[test]
    fn a_config_without_the_key_leaves_relay_off() {
        assert_eq!(
            HostConfig::parse("", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false,
                ..HostConfig::default()
            }
        );
        assert_eq!(
            HostConfig::parse("[identity]\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false,
                ..HostConfig::default()
            }
        );
    }

    #[test]
    fn a_config_relay_key_is_read() {
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = true\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: true,
                ..HostConfig::default()
            }
        );
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = false\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false,
                ..HostConfig::default()
            }
        );
    }

    #[test]
    fn a_config_typo_is_loud_rather_than_silently_off() {
        for text in [
            "[identity]\nrelayy = true\n",   // unknown key in the table
            "[identityy]\nrelay = true\n",   // unknown table
            "[identity]\nrelay = \"yes\"\n", // wrong type
            "[identity\nrelay = true\n",     // malformed
        ] {
            let error = HostConfig::parse(text, "c.toml").unwrap_err().to_string();
            assert!(error.contains("c.toml"), "{text:?}: {error}");
        }
    }
}
