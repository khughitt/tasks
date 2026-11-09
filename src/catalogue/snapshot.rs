use crate::error::{Error, Result};
use crate::repo::CONFIG_REL;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub aliases: Vec<String>,
    pub purpose: String,
    pub source: SnapshotSource,
    pub refreshed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotSource {
    Head { commit: String, blob: String },
    Seeded,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityConfig {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub purpose: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    pub refreshed: String,
}

impl Snapshot {
    pub fn from_config(raw: IdentityConfig) -> Result<Self> {
        let invalid = |detail| Error::Config(format!("{CONFIG_REL}: [identity] {detail}"));
        for (field, text) in [("name", &raw.name), ("purpose", &raw.purpose)] {
            crate::format::validate_line(field, text)
                .map_err(|error| invalid(error.to_string()))?;
        }
        for alias in &raw.aliases {
            crate::format::validate_line("alias", alias)
                .map_err(|error| invalid(error.to_string()))?;
        }
        let source = match (raw.source.as_str(), raw.commit, raw.blob) {
            ("head", Some(commit), Some(blob)) => SnapshotSource::Head { commit, blob },
            ("head", _, _) => return Err(invalid("source head requires commit and blob".into())),
            ("seeded", None, None) => SnapshotSource::Seeded,
            ("seeded", _, _) => {
                return Err(invalid("source seeded takes no commit or blob".into()));
            }
            (other, _, _) => return Err(invalid(format!("unknown source {other:?}"))),
        };
        Ok(Self {
            name: raw.name,
            aliases: raw.aliases,
            purpose: raw.purpose,
            source,
            refreshed: raw.refreshed,
        })
    }

    pub fn to_table(&self) -> toml_edit::Table {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(&self.name);
        let aliases: toml_edit::Array = self.aliases.iter().map(String::as_str).collect();
        table["aliases"] = toml_edit::value(aliases);
        table["purpose"] = toml_edit::value(&self.purpose);
        match &self.source {
            SnapshotSource::Head { commit, blob } => {
                table["source"] = toml_edit::value("head");
                table["commit"] = toml_edit::value(commit);
                table["blob"] = toml_edit::value(blob);
            }
            SnapshotSource::Seeded => table["source"] = toml_edit::value("seeded"),
        }
        table["refreshed"] = toml_edit::value(&self.refreshed);
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    fn seeded() -> IdentityConfig {
        toml::from_str(
            "name = 'Catalogue'\naliases = ['Public catalogue']\npurpose = 'Keeps identities'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'",
        )
        .unwrap()
    }

    fn config_error(raw: IdentityConfig) {
        match Snapshot::from_config(raw).unwrap_err() {
            Error::Config(message) => assert!(message.contains("[identity]"), "{message}"),
            other => panic!("expected config error, got {other}"),
        }
    }

    #[test]
    fn head_snapshot_needs_commit_and_blob() {
        let mut raw = seeded();
        raw.source = "head".into();
        raw.commit = Some("a".repeat(40));
        config_error(raw);
        let mut raw = seeded();
        raw.source = "head".into();
        raw.blob = Some("b".repeat(40));
        config_error(raw);
    }

    #[test]
    fn seeded_snapshot_takes_no_commit() {
        let mut raw = seeded();
        raw.commit = Some("a".repeat(40));
        config_error(raw);
        let mut raw = seeded();
        raw.blob = Some("b".repeat(40));
        config_error(raw);
    }

    #[test]
    fn name_and_purpose_are_one_line() {
        let mut raw = seeded();
        raw.name = "Two\nlines".into();
        config_error(raw);
        let mut raw = seeded();
        raw.purpose = "Two\nlines".into();
        config_error(raw);
    }

    #[test]
    fn unknown_source_is_refused() {
        let mut raw = seeded();
        raw.source = "mirror".into();
        config_error(raw);
    }

    #[test]
    fn aliases_are_one_line() {
        let mut raw = seeded();
        raw.aliases = vec!["Two\nlines".into()];
        config_error(raw);
    }

    #[test]
    fn round_trips_through_to_table() {
        for source in [
            SnapshotSource::Seeded,
            SnapshotSource::Head {
                commit: "a".repeat(40),
                blob: "b".repeat(40),
            },
        ] {
            let expected = Snapshot {
                name: "Catalogue".into(),
                aliases: vec!["Public catalogue".into()],
                purpose: "Keeps identities".into(),
                source,
                refreshed: "2026-11-09T16:00:00Z".into(),
            };
            let raw = toml::from_str(&expected.to_table().to_string()).unwrap();
            assert_eq!(Snapshot::from_config(raw).unwrap(), expected);
        }
    }
}
