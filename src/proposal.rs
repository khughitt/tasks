//! Pending proposals: a format-1 curate or scope note carrying a genuine dedicated
//! `proposal:` segment awaiting the human's decision. Format-2 tasks represent proposals
//! as named entries in `## Entries` instead of latest-note scraping.

use crate::model::Task;

/// The proposal text of a format-1 task whose most recent note is a curate or scope note
/// carrying a genuine nonempty dedicated `proposal:` segment. Returns `None` for format-2+
/// records (whose decisions live in `## Entries`) and rejects identifier mentions such as
/// `proposal::pending`.
pub fn pending(task: &Task) -> Option<&str> {
    if task.format != 1 {
        return None;
    }
    let text = task.notes.last()?.text.as_str();
    pending_in_note(text)
}

/// Extract a genuine nonempty dedicated `proposal:` verdict segment from a `curate:` or
/// `scope:` note line. Matches only at the start of a `;`-delimited segment (never inside
/// prose or an identifier like `proposal::pending`).
pub fn pending_in_note(text: &str) -> Option<&str> {
    let rest = text
        .strip_prefix("curate:")
        .or_else(|| text.strip_prefix("scope:"))?;
    let mut tail = rest;
    loop {
        let trimmed = tail.trim_start();
        if let Some(after) = trimmed.strip_prefix("proposal:")
            && !after.starts_with(':')
        {
            let body = after.trim();
            if !body.is_empty() {
                return Some(body);
            }
        }
        let idx = tail.find(';')?;
        tail = &tail[idx + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedicated_proposal_segment_matches_and_rejects_identifiers() {
        assert_eq!(
            pending_in_note("scope: drop; landed; proposal: drop it"),
            Some("drop it")
        );
        assert_eq!(
            pending_in_note("curate: stale; pass; guard: user; proposal: drop this; see abc"),
            Some("drop this; see abc")
        );
        assert_eq!(
            pending_in_note("curate: proposal: split into two"),
            Some("split into two")
        );
        assert_eq!(pending_in_note("scope: followed proposal::pending"), None);
        assert_eq!(
            pending_in_note("scope: drop; proposal::pending is broken"),
            None
        );
        assert_eq!(
            pending_in_note("scope: rescoped; checked proposal: handling"),
            None
        );
        assert_eq!(pending_in_note("scope: drop; proposal:   "), None);
        assert_eq!(pending_in_note("scope: drop; proposal:"), None);
        assert_eq!(pending_in_note("ordinary note; proposal: nope"), None);
    }
}
