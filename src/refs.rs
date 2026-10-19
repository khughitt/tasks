use crate::error::{Error, Result};
use crate::model::{Ref, Relation};
use crate::{format, frontmatter};

pub fn parse_entry(text: &str) -> Result<Ref> {
    let (handle, rest) = text
        .split_once(" (")
        .ok_or_else(|| Error::Validation("reference needs a handle and attributes".into()))?;
    let (attributes, rest) = rest
        .split_once("): ")
        .ok_or_else(|| Error::Validation("reference attributes must end with ): ".into()))?;
    let mut attributes = attributes.split(", ");
    let relation = Relation::parse(attributes.next().expect("split has a first element"))?;
    let mut required = false;
    let mut kind = None;
    for attribute in attributes {
        if attribute == "required" {
            if required || kind.is_some() {
                return Err(Error::Validation(
                    "reference attribute order must be relation, required, kind".into(),
                ));
            }
            required = true;
        } else if let Some(value) = attribute.strip_prefix("kind=") {
            if kind.is_some() {
                return Err(Error::Validation(
                    "duplicate reference kind attribute".into(),
                ));
            }
            kind = Some(value.to_string());
        } else {
            return Err(Error::Validation(format!(
                "unknown reference attribute {attribute:?}"
            )));
        }
    }
    let (reference, tail) = if rest.starts_with('"') {
        frontmatter::take_quoted(rest).map_err(Error::Validation)?
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let reference = &rest[..end];
        if reference.contains(['"', '\\']) {
            return Err(Error::Validation(
                "reference containing a quote or backslash must be quoted".into(),
            ));
        }
        (reference.to_string(), &rest[end..])
    };
    let description = if tail.is_empty() {
        None
    } else {
        Some(
            tail.strip_prefix(" — ")
                .ok_or_else(|| {
                    Error::Validation("reference description must follow \" — \"".into())
                })?
                .to_string(),
        )
    };
    let entry = Ref {
        handle: handle.into(),
        relation,
        required,
        kind,
        reference,
        description,
    };
    validate_entry(&entry)?;
    Ok(entry)
}

pub fn render_entry(entry: &Ref) -> String {
    let mut text = format!("{} ({}", entry.handle, entry.relation.as_str());
    if entry.required {
        text.push_str(", required");
    }
    if let Some(kind) = &entry.kind {
        text.push_str(&format!(", kind={kind}"));
    }
    text.push_str("): ");
    if entry
        .reference
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\')
    {
        text.push_str(&frontmatter::quote(&entry.reference));
    } else {
        text.push_str(&entry.reference);
    }
    if let Some(description) = &entry.description {
        text.push_str(" — ");
        text.push_str(description);
    }
    text
}

pub fn validate_entry(entry: &Ref) -> Result<()> {
    if entry.handle.len() > 32 || !format::valid_doc_scheme(&entry.handle) {
        return Err(Error::Validation(format!(
            "reference handle {:?} must match [a-z][a-z0-9-]{{0,31}}",
            entry.handle
        )));
    }
    if let Some(kind) = &entry.kind
        && !format::valid_doc_scheme(kind)
    {
        return Err(Error::Validation(format!(
            "reference kind {kind:?} must match [a-z][a-z0-9-]*"
        )));
    }
    if let Some(description) = &entry.description {
        format::validate_line("reference description", description)?;
        if description.trim().is_empty() {
            return Err(Error::Validation(
                "reference description must not be empty".into(),
            ));
        }
    }
    validate_reference(&entry.reference)
}

pub fn validate_reference(reference: &str) -> Result<()> {
    if reference.starts_with('@') {
        return Err(Error::Validation(
            "cross-project references are not supported yet".into(),
        ));
    }
    if let Some(name) = reference.strip_prefix("files:") {
        return crate::attachments::validate_name(name)
            .map_err(|error| Error::Validation(error.to_string()));
    }
    if format::external_doc(reference)?.is_some() {
        return Ok(());
    }
    format::validate_line("reference", reference)?;
    if reference.ends_with('/') {
        return Err(Error::Validation(format!(
            "reference must have no trailing slash; use {}",
            reference.trim_end_matches('/')
        )));
    }
    if !format::normalized_rel(reference) || reference.contains('\0') {
        return Err(Error::Validation(format!(
            "reference {reference:?} must be a normalized repo-relative path"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Relation;

    #[test]
    fn an_entry_round_trips_every_field() {
        let text = "brief (input, required, kind=brief): docs/notes/b.md — admission rules, §7";
        let entry = parse_entry(text).unwrap();
        assert_eq!(entry.handle, "brief");
        assert_eq!(entry.relation, Relation::Input);
        assert!(entry.required);
        assert_eq!(entry.kind.as_deref(), Some("brief"));
        assert_eq!(entry.reference, "docs/notes/b.md");
        assert_eq!(entry.description.as_deref(), Some("admission rules, §7"));
        assert_eq!(render_entry(&entry), text);
        let bare = parse_entry("r1 (context): files:after.png").unwrap();
        assert_eq!(
            (bare.required, bare.kind.clone(), bare.description.clone()),
            (false, None, None)
        );
        assert_eq!(render_entry(&bare), "r1 (context): files:after.png");
    }

    #[test]
    fn a_reference_needing_quotes_round_trips() {
        let text = r#"rb (output, required): "docs/run books/a \"v2\" \\ — x.md" — revised here"#;
        let entry = parse_entry(text).unwrap();
        assert_eq!(entry.reference, r#"docs/run books/a "v2" \ — x.md"#);
        assert_eq!(entry.description.as_deref(), Some("revised here"));
        assert_eq!(render_entry(&entry), text);
        let ext = parse_entry(r#"t (context): "stub:a b""#).unwrap();
        assert_eq!(ext.reference, "stub:a b");
    }

    #[test]
    fn handle_boundaries_and_kind_without_required_are_supported() {
        let handle = "a".repeat(32);
        let entry =
            parse_entry(&format!("{handle} (context, kind=brief-2): results/run-12")).unwrap();
        assert_eq!(entry.handle, handle);
        assert!(!entry.required);
        assert_eq!(entry.kind.as_deref(), Some("brief-2"));
        assert!(parse_entry(&format!("a{handle} (context): a.md")).is_err());
    }

    #[test]
    fn malformed_entries_are_refused_with_the_reason() {
        for (text, needle) in [
            (" (input): a.md", "handle"),
            ("b (input, kind=Bad): a.md", "kind"),
            ("b (input, kind=): a.md", "kind"),
            ("b (input, required, required): a.md", "order"),
            ("b (input, kind=x, kind=y): a.md", "attribute"),
            ("b (input): a.md —   ", "description"),
            ("b (input): a\\b.md", "quoted"),
            ("b (input): a\"b.md", "quoted"),
            (r#"b (input): "a\nb.md""#, "escape"),
            ("b (input): /x.md", "normalized"),
            ("b (input): ./x.md", "normalized"),
            ("b (input): docs//x.md", "normalized"),
            ("b (input): stub:", "nonempty"),
            ("Brief (input): a.md", "handle"),
            ("b (uses): a.md", "relation"),
            ("b (input, pinned): a.md", "attribute"),
            ("b (input, kind=x, required): a.md", "order"),
            ("b (input): a.md trailing", " — "),
            ("b (input): a.md — ", "description"),
            ("b (input): results/run-12/", "results/run-12"),
            (
                "b (input): @lore/docs/x.md",
                "cross-project references are not supported yet",
            ),
            ("b (input): docs/../x.md", "normalized"),
            ("b (input): files:.hidden", "attachment name"),
            ("b (input): \"unterminated", "unterminated"),
        ] {
            let error = parse_entry(text).unwrap_err();
            assert!(matches!(error, Error::Validation(_)), "{text}: {error}");
            let error = error.to_string();
            assert!(error.contains(needle), "{text}: {error}");
        }
    }
}
