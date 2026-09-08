use crate::error::{Error, Result};
use crate::frontmatter::{self, Value};

/// Rewrite `id` and local `depends`/`parent` references while preserving the body bytes.
#[allow(dead_code)] // Consumed by the rename inventory in Task 9/11.
pub fn rewrite_prefix(text: &str, old: &str, new: &str) -> Result<String> {
    let rest = text.strip_prefix("---\n").ok_or_else(|| Error::Parse {
        file: String::new(),
        detail: "missing opening ---".into(),
    })?;
    let (fm, after) = rest.split_once("\n---\n").ok_or_else(|| Error::Parse {
        file: String::new(),
        detail: "missing closing ---".into(),
    })?;
    let pairs = frontmatter::parse(&format!("{}\n", crate::format::quote_timestamps(fm)))?;

    let move_one = |value: &str| -> String {
        match value.split_once('-') {
            Some((prefix, hex)) if prefix == old => format!("{new}-{hex}"),
            _ => value.to_string(),
        }
    };
    let rewritten: Vec<(String, Value)> = pairs
        .into_iter()
        .map(|(key, value)| {
            let value = match (key.as_str(), value) {
                ("id", Value::Scalar(v)) | ("parent", Value::Scalar(v)) => {
                    Value::Scalar(move_one(&v))
                }
                ("depends", Value::List(items)) => {
                    Value::List(items.iter().map(|item| move_one(item)).collect())
                }
                ("created", Value::Scalar(v)) | ("updated", Value::Scalar(v)) => Value::Raw(v),
                (_, other) => other,
            };
            (key, value)
        })
        .collect();
    Ok(format!(
        "---\n{}---\n{after}",
        frontmatter::serialize(&rewritten)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAND_WRITTEN: &str = "---\nid: dot-a00088\ntitle: Hand written\nstatus: todo\n\
priority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\n\
depends: [dot-b11111, ops-c22222]\ntags: []\n---\n\n\nBody   with  odd    spacing.\n\n\n\
## Notes\n\n- 2026-09-01T00:00:00Z (keith):   two spaces after the colon\n";

    #[test]
    fn rewrites_ids_and_local_refs_and_keeps_every_other_byte() {
        let out = rewrite_prefix(HAND_WRITTEN, "dot", "dots").unwrap();
        assert!(out.contains("id: dots-a00088"));
        assert!(out.contains("depends: [dots-b11111, ops-c22222]"), "{out}");
        assert!(
            out.contains("created: 2026-09-01T00:00:00Z"),
            "timestamps stay unquoted"
        );
        let body = |t: &str| t.split_once("\n---\n").unwrap().1.to_string();
        assert_eq!(
            body(&out),
            body(HAND_WRITTEN),
            "post-frontmatter bytes are untouched"
        );
    }

    #[test]
    fn a_file_with_no_local_refs_changes_only_its_id() {
        let text =
            HAND_WRITTEN.replace("depends: [dot-b11111, ops-c22222]", "depends: [ops-c22222]");
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        assert!(out.contains("depends: [ops-c22222]"), "{out}");
    }
}
