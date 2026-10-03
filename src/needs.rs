//! Shared resources a task's work uses, drawn from the vocabulary its project declares in
//! `[needs]` (docs/specs/2026-10-03-lanes-needs-groups-design.md §4).

use crate::error::{Error, Result};

/// The tag grammar the spec gives need names (§4.1): lowercase ASCII letters, digits, and
/// `-`, never starting with `-`, so a name cannot read as a flag. Shared with the
/// vocabulary keys, the record field, and the `--need` flags.
pub fn validate_name(name: &str) -> Result<()> {
    let well_formed = !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if well_formed {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "need {name:?} must be lowercase letters, digits, and '-', not starting with '-'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_tag_grammar() {
        for good in ["quiet", "gpu-0", "2d", "needs-owner"] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", "Quiet", "-quiet", "a_b", "a b", "qüiet", "quiet\n"] {
            let error = validate_name(bad).unwrap_err();
            assert_eq!(error.kind(), "validation", "{bad:?}");
        }
    }
}
