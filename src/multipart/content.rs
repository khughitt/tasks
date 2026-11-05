use crate::error::{Error, Result};
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn builtin(name: &str, whole: &[u8]) -> Result<()> {
    std::str::from_utf8(whole).map_err(|error| {
        Error::MultipartNotText(format!(
            "multipart attachment {name:?} is not UTF-8 text: {error}"
        ))
    })?;
    if name.ends_with(".json") {
        serde_json::from_slice::<serde_json::Value>(whole).map_err(|error| {
            Error::MultipartInvalidContent(format!(
                "multipart attachment {name:?} is invalid JSON: {error}"
            ))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_rejects_non_utf8_and_invalid_json_only_for_json_names() {
        assert_eq!(
            builtin("a.txt", &[255]).unwrap_err().kind(),
            "multipart_not_text"
        );
        assert_eq!(
            builtin("a.json", b"{bad}").unwrap_err().kind(),
            "multipart_invalid_content"
        );
        assert!(builtin("a.txt", b"{bad}").is_ok());
        assert!(builtin("a.json", br#"{"a": 1}"#).is_ok());
        assert!(builtin("empty.txt", b"").is_ok());
    }
    #[test]
    fn sha256_uses_standard_lowercase_hex() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
