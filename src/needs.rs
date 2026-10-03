//! Shared resources a task's work uses, drawn from the vocabulary its project declares in
//! `[needs]` (docs/specs/2026-10-03-lanes-needs-groups-design.md §4).

use crate::error::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};

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

/// One `[needs.<name>]` table (spec §4.1). `meaning` is one required line; `exclusive`
/// says the resource serves one session at a time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedDecl {
    pub meaning: String,
    #[serde(default)]
    pub exclusive: bool,
}

/// A project's declared needs, by name. Committed with the project like `[tags]`.
pub type Vocabulary = BTreeMap<String, NeedDecl>;

fn declared_list(vocab: &Vocabulary) -> String {
    if vocab.is_empty() {
        "none are declared".into()
    } else {
        format!(
            "declared: {}",
            vocab.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

/// Every name must be declared (spec §4.2, on write). The first that is not fails
/// `unknown_need`, naming what the project does declare.
pub fn require_declared(vocab: &Vocabulary, names: &[String]) -> Result<()> {
    match names.iter().find(|name| !vocab.contains_key(*name)) {
        None => Ok(()),
        Some(name) => Err(Error::UnknownNeed(format!(
            "need {name:?} is not declared in {}'s [needs]; {}",
            crate::repo::CONFIG_REL,
            declared_list(vocab)
        ))),
    }
}

/// The needs a claim holds (spec §4.4): those its own project declares exclusive. An
/// undeclared name is ignored. Sorted and deduped.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exclusive holds (slice 2) are its first caller")
)]
pub fn exclusive_of(vocab: &Vocabulary, needs: &[String]) -> Vec<String> {
    needs
        .iter()
        .filter(|need| vocab.get(*need).is_some_and(|decl| decl.exclusive))
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
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

    fn vocab(entries: &[(&str, bool)]) -> Vocabulary {
        entries
            .iter()
            .map(|(name, exclusive)| {
                (
                    (*name).to_string(),
                    NeedDecl {
                        meaning: format!("{name} meaning"),
                        exclusive: *exclusive,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn require_declared_names_the_first_undeclared_need_and_the_vocabulary() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        assert!(require_declared(&declared, &["quiet".into(), "owner".into()]).is_ok());
        assert!(require_declared(&declared, &[]).is_ok());
        let error = require_declared(&declared, &["quiet".into(), "gpu".into()]).unwrap_err();
        assert_eq!(error.kind(), "unknown_need");
        let detail = error.to_string();
        assert!(detail.contains("\"gpu\""), "{detail}");
        assert!(detail.contains("declared: owner, quiet"), "{detail}");
        let error = require_declared(&Vocabulary::new(), &["quiet".into()]).unwrap_err();
        assert!(error.to_string().contains("none are declared"), "{error}");
    }

    #[test]
    fn exclusive_of_keeps_declared_exclusive_needs_sorted_once() {
        let declared = vocab(&[("quiet", true), ("gpu", true), ("owner", false)]);
        let needs: Vec<String> = ["quiet", "owner", "gpu", "quiet", "undeclared"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(exclusive_of(&declared, &needs), ["gpu", "quiet"]);
        assert!(exclusive_of(&declared, &["owner".into()]).is_empty());
    }
}
