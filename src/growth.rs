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
