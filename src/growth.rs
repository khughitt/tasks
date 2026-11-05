use crate::model::{Note, Task};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub struct GrowthRow {
    pub id: String,
    pub record_bytes: u64,
    pub body_bytes: u64,
    pub note_count: u64,
    pub note_text_bytes: u64,
    pub duplicate_note_text_bytes: u64,
    pub archived_bytes: u64,
}

pub fn comparison_key(text: &str) -> &str {
    text.trim_end_matches([' ', '\t'])
}

pub fn first_duplicate(notes: &[Note], text: &str) -> Option<usize> {
    let key = comparison_key(text);
    notes
        .iter()
        .position(|note| comparison_key(&note.text) == key)
}

pub fn measure(task: &Task, raw: &str, archived_bytes: u64) -> GrowthRow {
    let mut seen = HashSet::new();
    let mut note_text_bytes = 0;
    let mut duplicate_note_text_bytes = 0;
    for note in &task.notes {
        let bytes = note.text.len() as u64;
        note_text_bytes += bytes;
        if !seen.insert(comparison_key(&note.text)) {
            duplicate_note_text_bytes += bytes;
        }
    }
    GrowthRow {
        id: task.id.to_string(),
        record_bytes: raw.len() as u64,
        body_bytes: task.body.len() as u64,
        note_count: task.notes.len() as u64,
        note_text_bytes,
        duplicate_note_text_bytes,
        archived_bytes,
    }
}

/// Builds the `record_growth` warning detail shared by `tasks check` and write-time advisories.
/// Recommends shortening the body with `tasks edit <id> --body` only when the body is at least
/// half of the active record bytes (`body_bytes * 2 >= record_bytes`).
pub fn warning_detail(id: &str, record_bytes: u64, body_bytes: u64, threshold: u64) -> String {
    let base = format!(
        "active record {record_bytes} bytes exceeds threshold {threshold} bytes; place bulky \
         evidence in attachments (`tasks attach`) or records documents and link it; preview \
         lossless note archival with `tasks archive-notes {id}`"
    );
    if body_bytes.saturating_mul(2) >= record_bytes {
        format!(
            "{base}; preserve exact body bytes before using `tasks edit {id} --body` to shorten \
             a large body"
        )
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::warning_detail;

    #[test]
    fn growth_warning_detail_mentions_body_edit_only_when_body_dominates() {
        let notes_dominated = warning_detail("dot-a00001", 101, 50, 64);
        assert!(
            notes_dominated.contains("tasks archive-notes dot-a00001")
                && !notes_dominated.contains("--body"),
            "{notes_dominated}"
        );
        let body_dominated = warning_detail("dot-a00001", 101, 51, 64);
        assert!(
            body_dominated.contains("tasks archive-notes dot-a00001")
                && body_dominated.contains(
                    "preserve exact body bytes before using `tasks edit dot-a00001 --body` to shorten a large body"
                ),
            "{body_dominated}"
        );
    }
}
