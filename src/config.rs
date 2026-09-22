// The relay level is this file's only consumer and arrives in Task 5, which removes this.
#![allow(dead_code)]

use crate::error::{Error, Result};
use std::path::PathBuf;

/// The host-local configuration, `$XDG_CONFIG_HOME/tasks/config.toml` or
/// `$HOME/.config/tasks/config.toml`. Host-local deliberately: relay availability is a
/// property of a machine, and the per-project `tasks/.config.toml` is committed and syncs
/// between hosts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostConfig {
    pub relay_identity: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    #[serde(default)]
    relay: bool,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileShape {
    #[serde(default)]
    identity: FileIdentity,
}

impl HostConfig {
    pub fn path() -> Result<PathBuf> {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home).join("tasks/config.toml"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".config/tasks/config.toml"));
        }
        Err(Error::Config(
            "neither XDG_CONFIG_HOME nor HOME is set".into(),
        ))
    }

    /// An absent file is not an error: every field takes its default. A present but
    /// unreadable or invalid file is, because it was written on purpose.
    pub fn load() -> Result<HostConfig> {
        let path = HostConfig::path()?;
        let shown = path.display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => HostConfig::parse(&text, &shown),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HostConfig::default()),
            Err(error) => Err(Error::Io(format!("{shown}: {error}"))),
        }
    }

    pub fn parse(text: &str, path: &str) -> Result<HostConfig> {
        let file: FileShape =
            toml::from_str(text).map_err(|error| Error::Config(format!("{path}: {error}")))?;
        Ok(HostConfig {
            relay_identity: file.identity.relay,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_without_the_key_leaves_relay_off() {
        assert_eq!(
            HostConfig::parse("", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false
            }
        );
        assert_eq!(
            HostConfig::parse("[identity]\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false
            }
        );
    }

    #[test]
    fn a_config_relay_key_is_read() {
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = true\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: true
            }
        );
        assert_eq!(
            HostConfig::parse("[identity]\nrelay = false\n", "c.toml").unwrap(),
            HostConfig {
                relay_identity: false
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
