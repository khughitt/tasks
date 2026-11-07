use crate::error::{Error, Result};
use crate::frontmatter::{self, Value};

/// Rewrite `id`, local `depends`/`parent`/`wake_on`/`related` references, and References entries
/// pointing into a `tasks/files/<old>-<hex>` directory the rename moves, preserving every
/// other byte: body prose and notes stay historical.
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
                ("depends" | "wake_on" | "related", Value::List(items)) => {
                    Value::List(items.iter().map(|item| move_one(item)).collect())
                }
                (key, Value::Scalar(v)) if crate::format::TIMESTAMP_KEYS.contains(&key) => {
                    Value::Raw(v)
                }
                (_, other) => other,
            };
            (key, value)
        })
        .collect();
    Ok(format!(
        "---\n{}---\n{}",
        frontmatter::serialize(&rewritten),
        rewrite_references(after, old, new)
    ))
}

fn rewrite_references(after: &str, old: &str, new: &str) -> String {
    let mut in_refs = false;
    let mut out = String::with_capacity(after.len());
    let mut read = 0;
    for raw in after.split_inclusive('\n') {
        read += raw.len();
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == crate::format::REFS_DELIMITER {
            in_refs = true;
        } else if line == crate::format::NOTES_DELIMITER {
            // Notes are history: copy them verbatim.
            out.push_str(raw);
            out.push_str(&after[read..]);
            return out;
        } else if in_refs && let Some(moved) = moved_reference(line, old, new) {
            out.push_str(&moved);
            out.push_str(&raw[line.len()..]);
            continue;
        }
        out.push_str(raw);
    }
    out
}

/// The entry line with its reference moved to `<new>-<hex>`, when the reference is a local
/// path at or under `tasks/files/<old>-<hex>`; None leaves the line untouched.
fn moved_reference(line: &str, old: &str, new: &str) -> Option<String> {
    let (head, rest) = line.strip_prefix("- ")?.split_once("): ")?;
    let (quote, path) = match rest.strip_prefix('"') {
        Some(path) => ("\"", path),
        None => ("", rest),
    };
    let tail = path
        .strip_prefix("tasks/files/")?
        .strip_prefix(old)?
        .strip_prefix('-')?;
    let hex = tail.get(..6)?;
    let after = &tail[6..];
    crate::model::TaskId::parse(&format!("{old}-{hex}")).ok()?;
    // The reference ends at its closing quote when quoted, else at the end or the space
    // before its description; anything else names a sibling, not this directory.
    let end = if quote.is_empty() { ' ' } else { '"' };
    if !(after.starts_with('/') || after.starts_with(end) || (quote.is_empty() && after.is_empty()))
    {
        return None;
    }
    Some(format!("- {head}): {quote}tasks/files/{new}-{hex}{after}"))
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
    fn references_into_moved_attachment_directories_follow_them() {
        let text = HAND_WRITTEN
            .replacen(
                "\n## Notes\n",
                "\nProse keeps tasks/files/dot-b11111/a.png.\n\
- z (context): tasks/files/dot-b11111/a.png\n\n## References\n\n\
- a (input, required, kind=image): tasks/files/dot-b11111/a.png — tasks/files/dot-b11111\n\
- b (context): \"tasks/files/dot-b11111\"\n\
- g (context): \"tasks/files/dot-b11111 x/y.md\"\n\
- h (context): tasks/files/dots-b11111/x.md\n\
- i (context): tasks/files/dot-B11111/x.md\n\
- j (context): tasks/files/dot-b1111g/x.md\n\
- c (context): tasks/files/dot-b111110/x.md\n\
- d (context): tasks/files/ops-c22222/x.md\n\
- e (context): docs/tasks/files/dot-b11111/x.md\n\
- f (context): files:a.png\n\
\n## Notes\n",
                1,
            )
            .replace("two spaces after the colon", "tasks/files/dot-b11111/a.png");
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        let body = |t: &str| t.split_once("\n---\n").unwrap().1.to_string();
        let expected = body(&text)
            .replacen(
                "kind=image): tasks/files/dot-b11111/",
                "kind=image): tasks/files/dots-b11111/",
                1,
            )
            .replacen(
                "\"tasks/files/dot-b11111\"",
                "\"tasks/files/dots-b11111\"",
                1,
            );
        assert_eq!(body(&out), expected);
        let crlf = text.replace(
            "- b (context): \"tasks/files/dot-b11111\"\n",
            "- b (context): \"tasks/files/dot-b11111\"\r\n",
        );
        let out = rewrite_prefix(&crlf, "dot", "dots").unwrap();
        assert!(
            out.contains("- b (context): \"tasks/files/dots-b11111\"\r\n- g"),
            "{out}"
        );
    }

    #[test]
    fn a_file_with_no_local_refs_changes_only_its_id() {
        let text =
            HAND_WRITTEN.replace("depends: [dot-b11111, ops-c22222]", "depends: [ops-c22222]");
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        assert!(out.contains("depends: [ops-c22222]"), "{out}");
    }

    /// A task that has been started and returned to `todo` keeps its `started` field
    /// (spec: `started` is stamped once and never cleared). A rename must leave it
    /// unquoted like `created`/`updated`, or a later read double-quotes it via
    /// `quote_timestamps` and fails to parse.
    #[test]
    fn a_started_field_stays_unquoted_and_reparses() {
        let text = HAND_WRITTEN.replace(
            "updated: 2026-09-05T09:00:00Z\n",
            "updated: 2026-09-05T09:00:00Z\nstarted: 2026-09-05T09:00:00Z\n",
        );
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        assert!(
            out.contains("started: 2026-09-05T09:00:00Z\n"),
            "timestamps stay unquoted: {out}"
        );
        // Reparsing the rewritten file must not choke on a double-quoted timestamp.
        rewrite_prefix(&out, "dots", "dot").unwrap();
    }

    #[test]
    fn reviewed_at_and_closed_stay_unquoted_and_reparse() {
        let text = HAND_WRITTEN.replace(
            "updated: 2026-09-05T09:00:00Z\n",
            "updated: 2026-09-05T09:00:00Z\nreviewed_at: 2026-09-05T09:00:00Z\n\
closed: 2026-09-06T09:00:00Z\n",
        );
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        assert!(out.contains("reviewed_at: 2026-09-05T09:00:00Z\n"), "{out}");
        assert!(out.contains("closed: 2026-09-06T09:00:00Z\n"), "{out}");
        rewrite_prefix(&out, "dots", "dot").unwrap();
    }
}
