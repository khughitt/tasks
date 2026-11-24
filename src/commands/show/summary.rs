//! Bound the JSON detail view after the ordinary read has succeeded.

use crate::error::{Error, Result};
use crate::model::Note;
use crate::output::{ShowOut, ShowSummary, ShowSummaryNotes};
use std::path::Path;

const BYTE_LIMIT: usize = 12_288;

pub(in crate::commands) fn apply(
    out: &mut ShowOut,
    dir: Option<&Path>,
    id: &str,
    no_fetch: bool,
    all_projects: bool,
) -> Result<()> {
    let full = full_argv(dir, id, no_fetch, all_projects)?;
    let task = &mut out.fields.task;
    let notes = std::mem::take(&mut task.notes);
    let creation = notes.iter().position(|note| {
        note.at == task.created
            && note.text == "created"
            && matches!(note.by.as_str(), "add" | "feedback")
    });
    out.summary = Some(ShowSummary {
        byte_limit: BYTE_LIMIT,
        complete: false,
        notes: ShowSummaryNotes {
            total: notes.len(),
            shown: 0,
            omitted: notes.len(),
        },
        full,
    });

    select(out, &notes, creation, 0);
    if !fits(out) {
        return Err(Error::ShowSummaryTooLarge);
    }
    let mut low = 0;
    let mut high = notes.len() - usize::from(creation.is_some());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        select(out, &notes, creation, middle);
        // A whole additional note outweighs changes to count digits and completeness.
        if fits(out) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    select(out, &notes, creation, low);
    Ok(())
}

fn select(out: &mut ShowOut, notes: &[Note], creation: Option<usize>, tail: usize) {
    let mut start = notes.len() - tail;
    if creation.is_some_and(|index| index >= start) {
        // The tail already crosses the creation note, so take one more ordinary note.
        start -= 1;
    }
    let shown = &mut out.fields.task.notes;
    shown.clear();
    if let Some(index) = creation.filter(|&index| index < start) {
        shown.push(notes[index].clone());
    }
    shown.extend_from_slice(&notes[start..]);

    let summary = out.summary.as_mut().expect("summary was initialized");
    summary.notes.shown = shown.len();
    summary.notes.omitted = notes.len() - shown.len();
    summary.complete = summary.notes.omitted == 0;
}

fn fits(out: &ShowOut) -> bool {
    // The renderer adds one LF to the compact JSON.
    serde_json::to_vec(out)
        .expect("show output serializes")
        .len()
        < BYTE_LIMIT
}

fn full_argv(
    dir: Option<&Path>,
    id: &str,
    no_fetch: bool,
    all_projects: bool,
) -> Result<Vec<String>> {
    let exe = std::env::current_exe()?;
    if !std::fs::metadata(&exe)?.is_file() {
        return Err(Error::Io(
            "show summary recovery executable must be a regular file".into(),
        ));
    }
    let directory = std::path::absolute(super::super::start_dir(dir)?)?;
    if !std::fs::metadata(&directory)?.is_dir() {
        return Err(Error::Io(format!(
            "show summary recovery directory must name a directory: {}",
            directory.display()
        )));
    }
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::Io("show summary recovery path must be valid UTF-8".into()))
    };
    let mut full = vec![
        text(&exe)?,
        "--json".into(),
        "-C".into(),
        text(&directory)?,
        "show".into(),
        id.into(),
    ];
    if no_fetch {
        full.push("--no-fetch".into());
    }
    if all_projects {
        full.push("--all-projects".into());
    }
    Ok(full)
}
