#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub bytes: u64,
    pub parts: usize,
    pub sha256: String,
    pub catalog_sha256: String,
    pub caption: Option<String>,
}

impl Declaration {
    pub fn note(&self) -> String {
        let mut note = format!(
            "multipart: {} ({} bytes, {} parts, sha256 {}, catalog sha256 {})",
            self.name, self.bytes, self.parts, self.sha256, self.catalog_sha256
        );
        if let Some(caption) = &self.caption {
            note.push_str(": ");
            note.push_str(caption);
        }
        note
    }
}
pub fn parse_declaration_note(text: &str) -> Option<Declaration> {
    let (name, rest) = text.strip_prefix("multipart: ")?.split_once(" (")?;
    super::names::validate_logical_name(name).ok()?;
    let (bytes, rest) = rest.split_once(" bytes, ")?;
    let (parts, rest) = rest.split_once(" parts, sha256 ")?;
    let (sha256, rest) = rest.split_once(", catalog sha256 ")?;
    let (catalog_sha256, tail) = rest.split_once(')')?;
    if bytes.is_empty()
        || !bytes.bytes().all(|byte| byte.is_ascii_digit())
        || parts.is_empty()
        || !parts.bytes().all(|byte| byte.is_ascii_digit())
        || !valid_digest(sha256)
        || !valid_digest(catalog_sha256)
    {
        return None;
    }
    let parts = parts.parse().ok()?;
    if parts == 0 {
        return None;
    }
    let caption = if tail.is_empty() {
        None
    } else {
        Some(tail.strip_prefix(": ")?.to_owned())
    };
    Some(Declaration {
        name: name.into(),
        bytes: bytes.parse().ok()?,
        parts,
        sha256: sha256.into(),
        catalog_sha256: catalog_sha256.into(),
        caption,
    })
}

pub(super) fn valid_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_round_trips_and_rejects_near_misses() {
        let mut declaration = Declaration {
            name: "report.json".into(),
            bytes: 123,
            parts: 2,
            sha256: "a".repeat(64),
            catalog_sha256: "b".repeat(64),
            caption: None,
        };
        let plain = format!(
            "multipart: report.json (123 bytes, 2 parts, sha256 {}, catalog sha256 {})",
            "a".repeat(64),
            "b".repeat(64)
        );
        assert_eq!(declaration.note(), plain);
        assert_eq!(parse_declaration_note(&plain), Some(declaration.clone()));
        let oversized = plain
            .replace("123 bytes", "268435457 bytes")
            .replace("2 parts", "257 parts");
        let parsed =
            parse_declaration_note(&oversized).expect("audit must see oversized declarations");
        assert_eq!(parsed.bytes, 268435457);
        assert_eq!(parsed.parts, 257);
        declaration.caption = Some("a report: with (details)".into());
        assert_eq!(
            declaration.note(),
            format!("{plain}: a report: with (details)")
        );
        assert_eq!(
            parse_declaration_note(&declaration.note()),
            Some(declaration)
        );
        for bad in [
            plain.replace(", catalog sha256 ", ", "),
            plain.replace(&"a".repeat(64), &"g".repeat(64)),
            plain.replace(&"b".repeat(64), &"g".repeat(64)),
            plain.replace("2 parts", "0 parts"),
            plain.replace("report.json", ".hidden"),
            format!("{plain} trailing"),
            format!("{plain}:caption"),
            plain.replace("123 bytes", "-1 bytes"),
        ] {
            assert!(parse_declaration_note(&bad).is_none(), "{bad}");
        }
    }
}
