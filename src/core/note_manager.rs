use std::path::PathBuf;

use anyhow::{bail, Context};

use crate::core::note::{truncate_slug, Note, MAX_SLUG_LEN};

/// Orchestrates scanning, CRUD, and refresh of notes in a directory.
pub struct NoteManager {
    pub notes_dir: PathBuf,
    pub notes: Vec<Note>,
    /// Warnings collected during the last `scan()` (e.g. unreadable files).
    pub scan_warnings: Vec<String>,
}

impl NoteManager {
    /// Create a new NoteManager for the given directory.
    /// Creates the directory if it does not exist.
    pub fn new(notes_dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&notes_dir)
            .with_context(|| format!("failed to create notes directory: {}", notes_dir.display()))?;

        Ok(Self {
            notes_dir,
            notes: Vec::new(),
            scan_warnings: Vec::new(),
        })
    }

    /// Scan the notes directory for .md files and load their metadata.
    /// Replaces any previously loaded notes.
    pub fn scan(&mut self) -> anyhow::Result<()> {
        self.notes.clear();
        self.scan_warnings.clear();

        let entries = std::fs::read_dir(&self.notes_dir)
            .with_context(|| format!("failed to read notes directory: {}", self.notes_dir.display()))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    self.scan_warnings.push(format!("failed to read directory entry: {err}"));
                    continue;
                }
            };

            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            match Note::load_from_path(path.clone()) {
                Ok(note) => self.notes.push(note),
                Err(err) => {
                    self.scan_warnings.push(format!("failed to load {}: {err}", path.display()));
                }
            }
        }

        Ok(())
    }

    /// Create a new note with the given title.
    /// Returns the path of the created note.
    pub fn create_note(&mut self, title: &str) -> anyhow::Result<PathBuf> {
        let note = Note::create_new(&self.notes_dir, title)?;
        let path = note.path.clone();
        self.notes.push(note);
        Ok(path)
    }

    /// Delete the note at the given index from disk and from the in-memory list.
    pub fn delete_note(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.notes.len() {
            bail!(
                "note index {} out of bounds (have {} notes)",
                index,
                self.notes.len()
            );
        }

        let note = &self.notes[index];
        std::fs::remove_file(&note.path)
            .with_context(|| format!("failed to delete note: {}", note.path.display()))?;

        self.notes.remove(index);
        Ok(())
    }

    /// Rename the note at the given index: update front matter title,
    /// rename the file on disk, and update the in-memory entry.
    pub fn rename_note(&mut self, index: usize, new_title: &str) -> anyhow::Result<()> {
        if index >= self.notes.len() {
            bail!(
                "note index {} out of bounds (have {} notes)",
                index,
                self.notes.len()
            );
        }

        // Load content if not already loaded
        self.notes[index].ensure_content()?;
        let body = self.notes[index]
            .content
            .clone()
            .unwrap_or_default();

        // Save old title for rollback
        let old_title = self.notes[index].title.clone();

        // Update the title in memory
        self.notes[index].title = new_title.to_string();

        // Write updated front matter + body to the current file
        if let Err(e) = self.notes[index].save_content(&body) {
            // Rollback title
            self.notes[index].title = old_title;
            return Err(e.context("failed to save during rename"));
        }

        // Generate new filename with collision avoidance
        let base_slug = truncate_slug(&slug::slugify(new_title), MAX_SLUG_LEN);
        let mut filename = format!("{}.md", base_slug);
        let mut new_path = self.notes_dir.join(&filename);
        let mut counter = 1u32;
        while new_path.exists() && new_path != self.notes[index].path {
            filename = format!("{}-{}.md", base_slug, counter);
            new_path = self.notes_dir.join(&filename);
            counter += 1;
        }

        // Rename the file on disk if the path actually changed
        if new_path != self.notes[index].path {
            if let Err(e) = std::fs::rename(&self.notes[index].path, &new_path) {
                // Rollback: restore old title and re-save
                self.notes[index].title = old_title;
                let _ = self.notes[index].save_content(&body); // best-effort rollback
                return Err(anyhow::anyhow!(e).context(format!(
                    "failed to rename {} -> {}",
                    self.notes[index].path.display(),
                    new_path.display()
                )));
            }
            self.notes[index].path = new_path;
        }

        Ok(())
    }

    /// Reload a note's metadata from disk, replacing the in-memory entry.
    pub fn refresh_note(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.notes.len() {
            bail!(
                "note index {} out of bounds (have {} notes)",
                index,
                self.notes.len()
            );
        }

        let path = self.notes[index].path.clone();
        let reloaded = Note::load_from_path(path)?;
        self.notes[index] = reloaded;
        Ok(())
    }

    /// Get the content of the note at the given index, lazy-loading if needed.
    pub fn get_content(&mut self, index: usize) -> anyhow::Result<String> {
        if index >= self.notes.len() {
            bail!(
                "note index {} out of bounds (have {} notes)",
                index,
                self.notes.len()
            );
        }

        let text = self.notes[index].ensure_content()?;
        Ok(text.to_string())
    }

    /// Read-only access to the notes slice.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: write a .md file with front matter into the given directory.
    fn write_md_file(dir: &std::path::Path, name: &str, title: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let content = format!(
            "---\ntitle: \"{title}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\n\n{body}"
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn new_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let notes_dir = tmp.path().join("new-notes-dir");
        assert!(!notes_dir.exists());

        let _mgr = NoteManager::new(notes_dir.clone()).unwrap();

        assert!(notes_dir.exists());
        assert!(notes_dir.is_dir());
    }

    #[test]
    fn scan_finds_md_files() {
        let tmp = TempDir::new().unwrap();
        write_md_file(tmp.path(), "one.md", "One", "body one");
        write_md_file(tmp.path(), "two.md", "Two", "body two");
        write_md_file(tmp.path(), "three.md", "Three", "body three");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        assert_eq!(mgr.notes().len(), 3);
    }

    #[test]
    fn scan_ignores_non_md() {
        let tmp = TempDir::new().unwrap();
        write_md_file(tmp.path(), "note.md", "Note", "body");

        // Create a non-md file
        let txt_path = tmp.path().join("readme.txt");
        std::fs::write(&txt_path, "not a note").unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        assert_eq!(mgr.notes().len(), 1);
        assert_eq!(mgr.notes()[0].title, "Note");
    }

    #[test]
    fn create_note_adds_to_list() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        assert_eq!(mgr.notes().len(), 0);

        let path = mgr.create_note("My New Note").unwrap();

        assert_eq!(mgr.notes().len(), 1);
        assert_eq!(mgr.notes()[0].title, "My New Note");
        assert!(path.exists());
    }

    #[test]
    fn delete_note_removes_file_and_entry() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        let path = mgr.create_note("Delete Me").unwrap();
        assert!(path.exists());
        assert_eq!(mgr.notes().len(), 1);

        mgr.delete_note(0).unwrap();

        assert!(!path.exists());
        assert_eq!(mgr.notes().len(), 0);
    }

    #[test]
    fn delete_note_out_of_bounds() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        let result = mgr.delete_note(0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("out of bounds"));
    }

    #[test]
    fn rename_note_changes_title_and_file() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        let old_path = mgr.create_note("Original Title").unwrap();
        assert!(old_path.exists());

        mgr.rename_note(0, "Renamed Title").unwrap();

        assert!(!old_path.exists(), "old file should be gone");
        assert_eq!(mgr.notes()[0].title, "Renamed Title");

        let new_filename = mgr.notes()[0].path.file_name().unwrap().to_str().unwrap();
        assert_eq!(new_filename, "renamed-title.md");
        assert!(mgr.notes()[0].path.exists());
    }

    #[test]
    fn refresh_note_reloads_from_disk() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        mgr.create_note("Refresh Test").unwrap();
        assert_eq!(mgr.notes()[0].title, "Refresh Test");

        // Modify the file externally: overwrite with new front matter
        let path = mgr.notes()[0].path.clone();
        let new_content = "---\ntitle: \"Updated Externally\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-06-01T00:00:00\ntags: [updated]\n---\n\nNew body.";
        std::fs::write(&path, new_content).unwrap();

        mgr.refresh_note(0).unwrap();

        assert_eq!(mgr.notes()[0].title, "Updated Externally");
        assert_eq!(mgr.notes()[0].tags, vec!["updated"]);
        // Content should be None again (lazy-loaded after refresh)
        assert!(mgr.notes()[0].content.is_none());
    }

    #[test]
    fn get_content_returns_body() {
        let tmp = TempDir::new().unwrap();
        write_md_file(tmp.path(), "content-test.md", "Content Test", "Hello from body");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();
        assert_eq!(mgr.notes().len(), 1);

        let content = mgr.get_content(0).unwrap();
        assert!(content.contains("Hello from body"));
    }
}
