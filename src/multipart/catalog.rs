use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPart {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub parts: Vec<CatalogPart>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    name: String,
    bytes: u64,
    sha256: String,
    parts: Vec<CatalogPart>,
}

impl Catalog {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "{{\n  \"version\": 1,\n  \"name\": {},\n  \"bytes\": {},\n  \"sha256\": {},\n  \"parts\": [\n",
            serde_json::to_string(&self.name).expect("string serializes"),
            self.bytes,
            serde_json::to_string(&self.sha256).expect("string serializes")
        );
        for (index, part) in self.parts.iter().enumerate() {
            text.push_str("    ");
            text.push_str(&serde_json::to_string(part).expect("catalog part serializes"));
            if index + 1 < self.parts.len() {
                text.push(',');
            }
            text.push('\n');
        }
        text.push_str("  ]\n}\n");
        text.into_bytes()
    }

    pub fn parse(bytes: &[u8]) -> Result<Catalog> {
        let invalid = |detail: String| {
            Error::MultipartInvalidContent(format!("invalid multipart catalog: {detail}"))
        };
        if bytes.len() as u64 > super::RECOVERY_MAX_BYTES {
            return Err(invalid("catalog exceeds recovery byte ceiling".into()));
        }
        let doc: Document =
            serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
        if doc.version != 1
            || super::names::validate_logical_name(&doc.name).is_err()
            || !super::declaration::valid_digest(&doc.sha256)
            || doc.parts.is_empty()
            || doc.parts.len() > super::RECOVERY_MAX_PARTS
        {
            return Err(invalid(
                "invalid version, name, digest or part count".into(),
            ));
        }
        for (index, part) in doc.parts.iter().enumerate() {
            if part.name != super::names::part_name(&doc.name, index + 1)
                || !super::declaration::valid_digest(&part.sha256)
            {
                return Err(invalid(format!(
                    "invalid part {} name or digest",
                    index + 1
                )));
            }
        }
        Ok(Catalog {
            name: doc.name,
            bytes: doc.bytes,
            sha256: doc.sha256,
            parts: doc.parts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_serializes_one_part_per_line_and_round_trips() {
        let catalog = Catalog {
            name: "a.txt".into(),
            bytes: 3,
            sha256: "a".repeat(64),
            parts: vec![
                CatalogPart {
                    name: "a.txt.part-0001".into(),
                    bytes: 1,
                    sha256: "b".repeat(64),
                },
                CatalogPart {
                    name: "a.txt.part-0002".into(),
                    bytes: 2,
                    sha256: "c".repeat(64),
                },
            ],
        };
        let expected = format!(
            "{{\n  \"version\": 1,\n  \"name\": \"a.txt\",\n  \"bytes\": 3,\n  \"sha256\": \"{}\",\n  \"parts\": [\n    {{\"name\":\"a.txt.part-0001\",\"bytes\":1,\"sha256\":\"{}\"}},\n    {{\"name\":\"a.txt.part-0002\",\"bytes\":2,\"sha256\":\"{}\"}}\n  ]\n}}\n",
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64)
        );
        assert_eq!(catalog.to_bytes(), expected.as_bytes());
        assert_eq!(Catalog::parse(&catalog.to_bytes()).unwrap(), catalog);
        for bad in [
            expected.replace("\"version\": 1", "\"version\": 2"),
            expected.replace("a.txt.part-0002", "../outside"),
            expected.replace(&"a".repeat(64), "invalid"),
        ] {
            assert!(Catalog::parse(bad.as_bytes()).is_err());
        }
    }
}
