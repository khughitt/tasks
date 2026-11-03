use crate::attachments::validate_name;
use crate::error::{Error, Result};

pub fn validate_logical_name(name: &str) -> Result<()> {
    validate_name(name)?;
    if name.len() > 115 {
        return Err(Error::InvalidAttachmentName(format!(
            "multipart attachment name {name:?} must be at most 115 bytes"
        )));
    }
    Ok(())
}

pub fn part_name(name: &str, index: usize) -> String {
    format!("{name}.part-{index:04}")
}

pub fn catalog_name(name: &str) -> String {
    format!("{name}.catalog.json")
}

pub fn member_names(name: &str, parts: usize) -> Vec<String> {
    (1..=parts)
        .map(|index| part_name(name, index))
        .chain(std::iter::once(catalog_name(name)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_name_limit_is_115_bytes() {
        assert!(validate_logical_name(&"a".repeat(115)).is_ok());
        assert!(validate_logical_name(&"a".repeat(116)).is_err());
        assert_eq!(catalog_name(&"a".repeat(115)).len(), 128);
        for name in ["", ".hidden", "a/b", "a b", "é"] {
            assert!(validate_logical_name(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn member_names_are_zero_padded_and_end_with_the_catalog() {
        assert_eq!(
            member_names("report.txt", 2),
            [
                "report.txt.part-0001",
                "report.txt.part-0002",
                "report.txt.catalog.json"
            ]
        );
    }
}
