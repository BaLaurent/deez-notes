//! Non-TUI command-line front-end over `NoteManager`.
//!
//! A second front-end (alongside the TUI) for discovering, reading, searching
//! and editing notes programmatically. No business logic lives here — every
//! command delegates to the core.

use std::path::Path;

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::core::note::Note;
use crate::core::note_manager::NoteManager;
use crate::core::search::fuzzy_search;

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

/// List notes, optionally filtered by folder (exact, non-recursive) and/or tag.
pub fn list(manager: &NoteManager, folder: Option<&str>, tag: Option<&str>, json: bool) -> String {
    let notes = manager.notes();
    let base: Vec<usize> = match folder {
        Some(f) => manager.notes_in_folder(Path::new(f)),
        None => (0..notes.len()).collect(),
    };
    let indices: Vec<usize> = base
        .into_iter()
        .filter(|&i| match tag {
            Some(t) => notes[i].tags.iter().any(|nt| nt == t),
            None => true,
        })
        .collect();
    format_notes(manager, &indices, json)
}

/// Fuzzy-search notes by title, rendered like `list`.
pub fn search(manager: &NoteManager, query: &str, json: bool) -> String {
    let indices = fuzzy_search(query, manager.notes(), false);
    format_notes(manager, &indices, json)
}

/// Resolve a `<note>` argument to a note index: exact relative-path match first,
/// then fuzzy title fallback. Errors if nothing matches.
pub fn resolve_note(manager: &NoteManager, query: &str) -> anyhow::Result<usize> {
    let notes = manager.notes();
    if let Some(i) = notes.iter().position(|n| relative_path(manager, n) == query) {
        return Ok(i);
    }
    fuzzy_search(query, notes, false)
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("no note matches '{}'", query))
}

/// Read a note's markdown body (front matter stripped) to stdout-ready text.
pub fn get(manager: &mut NoteManager, query: &str) -> anyhow::Result<String> {
    let idx = resolve_note(manager, query)?;
    manager.get_content(idx)
}
