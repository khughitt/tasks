use crate::error::{Error, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionList {
    pub version: u32,
    pub source: String,
    pub successor: String,
    pub drafted: String,
    #[serde(default)]
    pub reasons: BTreeMap<String, String>,
    #[serde(default, rename = "task")]
    pub tasks: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub digest: String,
    /// `carry`, `drop`, `close`, or `""` for undecided; see `Decision::parse`.
    pub decision: String,
    pub reason: Option<String>,
    pub parent: Option<String>,
    #[serde(default)]
    pub rm_depends: Vec<String>,
    #[serde(default)]
    pub release: Vec<String>,
    pub links: Option<String>,
    pub body: Option<String>,
}

// Read by the preflight and apply of the plan's later tasks.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Carry,
    Drop,
    Close,
}

#[allow(dead_code)]
impl Decision {
    /// `None` for anything but the three decisions, `""` (undecided) included; the caller
    /// tells those apart because only `""` is the draft's own value.
    pub fn parse(text: &str) -> Option<Decision> {
        match text {
            "carry" => Some(Decision::Carry),
            "drop" => Some(Decision::Drop),
            "close" => Some(Decision::Close),
            _ => None,
        }
    }
}

// Read by the preflight and apply of the plan's later tasks.
#[allow(dead_code)]
pub struct LoadedList {
    pub list: DecisionList,
    /// The file exactly as read.
    pub bytes: Vec<u8>,
    /// Lowercase hex sha256 of `bytes`, without a `sha256:` prefix.
    pub sha256: String,
}

pub fn load(path: &Path) -> Result<LoadedList> {
    let bytes = std::fs::read(path).map_err(|e| {
        Error::Validation(format!("cannot read decision list {}: {e}", path.display()))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        Error::Validation(format!(
            "decision list {} is not UTF-8: {e}",
            path.display()
        ))
    })?;
    let list: DecisionList = toml::from_str(text).map_err(|e| {
        Error::Validation(format!(
            "decision list {} is malformed: {e}",
            path.display()
        ))
    })?;
    if list.version != 1 {
        return Err(Error::Validation(format!(
            "decision list {} has version {}; this tasks reads version 1",
            path.display(),
            list.version
        )));
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(LoadedList {
        list,
        bytes,
        sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.toml");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }

    #[test]
    fn decisions_parse_and_undecided_is_not_one() {
        assert_eq!(Decision::parse("carry"), Some(Decision::Carry));
        assert_eq!(Decision::parse("drop"), Some(Decision::Drop));
        assert_eq!(Decision::parse("close"), Some(Decision::Close));
        assert_eq!(Decision::parse(""), None);
        assert_eq!(Decision::parse("keep"), None);
    }

    #[test]
    fn a_list_loads_with_its_hash_and_optional_keys() {
        let text = "version = 1\nsource = \"a\"\nsuccessor = \"b\"\ndrafted = \"2026-11-02T12:00:00Z\"\n\n[reasons]\nr = \"why\"\n\n[[task]]\nid = \"a-000001\"\ndigest = \"sha256:00\"\ndecision = \"drop\"\nreason = \"r\"\nrelease = [\"c-000002\"]\n";
        let (_dir, path) = write(text);
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.bytes, text.as_bytes());
        assert_eq!(loaded.sha256.len(), 64);
        assert_eq!(loaded.list.reasons["r"], "why");
        assert_eq!(loaded.list.tasks[0].release, ["c-000002"]);
        assert_eq!(loaded.list.tasks[0].parent, None);
    }

    #[test]
    fn malformed_toml_unknown_keys_and_other_versions_name_the_path() {
        for text in [
            "version = ",
            "version = 1\nsource = \"a\"\nsuccessor = \"b\"\ndrafted = \"d\"\nextra = 1\n",
            "version = 2\nsource = \"a\"\nsuccessor = \"b\"\ndrafted = \"d\"\n",
        ] {
            let (_dir, path) = write(text);
            let error = load(&path).err().unwrap();
            assert!(matches!(error, Error::Validation(_)), "{error}");
            assert!(error.to_string().contains("list.toml"), "{error}");
        }
    }
}
