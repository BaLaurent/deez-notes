//! Non-TUI command-line front-end over `NoteManager`.
//!
//! A second front-end (alongside the TUI) for discovering, reading, searching
//! and editing notes programmatically. No business logic lives here — every
//! command delegates to the core.

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::core::note::Note;
use crate::core::note_manager::NoteManager;

/// Relative path of a note from the notes directory (the stable CLI identifier).
pub fn relative_path(manager: &NoteManager, note: &Note) -> String {
    note.path
        .strip_prefix(&manager.notes_dir)
        .unwrap_or(&note.path)
        .to_string_lossy()
        .into_owned()
}

/// Plain-data view of a note for JSON output (no lazy `content`).
#[derive(Serialize)]
struct NoteInfo {
    path: String,
    title: String,
    tags: Vec<String>,
    created: DateTime<Local>,
    modified: DateTime<Local>,
}

/// Render a set of note indices either as TAB-separated lines or a JSON array.
pub fn format_notes(manager: &NoteManager, indices: &[usize], json: bool) -> String {
    let notes = manager.notes();
    if json {
        let infos: Vec<NoteInfo> = indices
            .iter()
            .map(|&i| NoteInfo {
                path: relative_path(manager, &notes[i]),
                title: notes[i].title.clone(),
                tags: notes[i].tags.clone(),
                created: notes[i].created,
                modified: notes[i].modified,
            })
            .collect();
        serde_json::to_string_pretty(&infos).unwrap_or_default()
    } else {
        indices
            .iter()
            .map(|&i| format!("{}\t{}", relative_path(manager, &notes[i]), notes[i].title))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// List notes, optionally filtered by folder and/or tag. (Filters wired in Task 2.)
pub fn list(manager: &NoteManager, _folder: Option<&str>, _tag: Option<&str>, json: bool) -> String {
    let indices: Vec<usize> = (0..manager.notes().len()).collect();
    format_notes(manager, &indices, json)
}
