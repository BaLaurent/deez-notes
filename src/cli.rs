//! Non-TUI command-line front-end over `NoteManager`.
//!
//! A second front-end (alongside the TUI) for discovering, reading, searching
//! and editing notes programmatically. No business logic lives here — every
//! command delegates to the core.

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Local};
use clap::Subcommand;
use serde::Serialize;

use crate::config::settings::Config;
use crate::core::note::Note;
use crate::core::note_manager::NoteManager;
use crate::core::search::fuzzy_search;
use crate::editor::external::open_in_editor;

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

/// Create a note in `folder` (empty = root); returns its absolute path.
/// The new note is appended last in `manager.notes()`.
pub fn create(
    manager: &mut NoteManager,
    title: &str,
    folder: &str,
) -> anyhow::Result<std::path::PathBuf> {
    manager.create_note(title, Path::new(folder))
}

/// Overwrite a note's body, preserving/refreshing its front matter.
pub fn set_body(manager: &NoteManager, idx: usize, body: &str) -> anyhow::Result<()> {
    manager.notes()[idx].save_content(body)
}

/// Delete a note from disk and from the in-memory list.
pub fn remove(manager: &mut NoteManager, idx: usize) -> anyhow::Result<()> {
    manager.delete_note(idx)
}

/// Create a symlink in `dest_dir` that points at the note's file.
///
/// The link is named `name` when given, otherwise the note's own filename
/// (e.g. `mabit.md`). The symlink target is the note's canonical absolute path,
/// so the link stays valid no matter where `dest_dir` is. Returns the path of
/// the created symlink. Fails if anything already exists at that path.
pub fn link(
    manager: &NoteManager,
    idx: usize,
    dest_dir: &Path,
    name: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let note_path = &manager.notes()[idx].path;
    let target = std::fs::canonicalize(note_path)
        .with_context(|| format!("failed to resolve note path: {}", note_path.display()))?;

    let link_name = match name {
        Some(n) => n.to_string(),
        None => target
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow::anyhow!("note has no filename"))?,
    };

    let link_path = dest_dir.join(&link_name);
    // `symlink_metadata` (unlike `exists`) does not follow links, so a broken
    // symlink already at the path is detected too.
    if link_path.symlink_metadata().is_ok() {
        anyhow::bail!("'{}' already exists", link_path.display());
    }
    create_symlink(&target, &link_path)
        .with_context(|| format!("failed to create symlink: {}", link_path.display()))?;
    Ok(link_path)
}

/// Create a filesystem symlink, dispatching to the platform-specific call.
#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Create a filesystem symlink, dispatching to the platform-specific call.
#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

/// Subcommands. Absence of a subcommand (handled in `main`) launches the TUI.
#[derive(Subcommand)]
pub enum Command {
    /// List notes (TAB-separated path + title, or --json).
    List {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        tag: Option<String>,
    },
    /// Print a note's markdown body to stdout.
    Get { note: String },
    /// Fuzzy-search notes by title.
    Search {
        query: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a note; body from stdin if piped, else opens $EDITOR.
    New {
        title: String,
        #[arg(long)]
        folder: Option<String>,
    },
    /// Overwrite a note's body; from stdin if piped, else opens $EDITOR.
    Set { note: String },
    /// Delete a note.
    Rm { note: String },
    /// Symlink a note into the current directory.
    Link {
        note: String,
        /// Name for the symlink (default: the note's filename).
        name: Option<String>,
    },
}

/// Run a CLI subcommand against a freshly scanned `NoteManager`. Never touches
/// the terminal (no raw mode / alternate screen).
pub fn run(command: Command, config: Config) -> anyhow::Result<()> {
    let mut manager = NoteManager::new(config.resolve_notes_dir())?;
    manager.scan()?;

    let editor_override = if config.general.editor.is_empty() {
        None
    } else {
        Some(config.general.editor.as_str())
    };

    match command {
        Command::List { json, folder, tag } => {
            println!("{}", list(&manager, folder.as_deref(), tag.as_deref(), json));
        }
        Command::Search { query, json } => {
            println!("{}", search(&manager, &query, json));
        }
        Command::Get { note } => {
            print!("{}", get(&mut manager, &note)?);
        }
        Command::New { title, folder } => {
            let path = create(&mut manager, &title, folder.as_deref().unwrap_or(""))?;
            let idx = manager.notes().len() - 1;
            if let Some(body) = piped_stdin()? {
                set_body(&manager, idx, &body)?;
            } else {
                open_in_editor(&path, editor_override)?;
            }
            println!("{}", relative_path(&manager, &manager.notes()[idx]));
        }
        Command::Set { note } => {
            let idx = resolve_note(&manager, &note)?;
            if let Some(body) = piped_stdin()? {
                set_body(&manager, idx, &body)?;
            } else {
                let path = manager.notes()[idx].path.clone();
                open_in_editor(&path, editor_override)?;
            }
        }
        Command::Rm { note } => {
            let idx = resolve_note(&manager, &note)?;
            remove(&mut manager, idx)?;
        }
        Command::Link { note, name } => {
            let idx = resolve_note(&manager, &note)?;
            let dest = std::env::current_dir()?;
            let link_path = link(&manager, idx, &dest, name.as_deref())?;
            println!("{}", link_path.display());
        }
    }
    Ok(())
}

/// Return stdin's full contents if stdin is piped (not a terminal), else `None`.
fn piped_stdin() -> anyhow::Result<Option<String>> {
    if std::io::stdin().is_terminal() {
        return Ok(None);
    }
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    Ok(Some(s))
}
