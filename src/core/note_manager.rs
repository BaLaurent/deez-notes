use std::path::PathBuf;

use anyhow::{bail, Context};

use crate::core::note::{truncate_slug, Note, MAX_SLUG_LEN};

/// Orchestrates scanning, CRUD, and refresh of notes in a directory.
pub struct NoteManager {
    pub notes_dir: PathBuf,
    pub notes: Vec<Note>,
    /// Relative paths of all discovered subdirectories (e.g. `["work", "personal"]`).
    pub folders: Vec<PathBuf>,
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
            folders: Vec::new(),
            scan_warnings: Vec::new(),
        })
    }

    /// Scan the notes directory recursively for .md files and subdirectories.
    /// Replaces any previously loaded notes and folders.
    pub fn scan(&mut self) -> anyhow::Result<()> {
        self.notes.clear();
        self.folders.clear();
        self.scan_warnings.clear();

        let root = self.notes_dir.clone();
        self.scan_dir_recursive(&root)?;

        self.folders.sort();
        Ok(())
    }

    /// Recursively scan a directory for .md files and subdirectories.
    fn scan_dir_recursive(&mut self, dir: &std::path::Path) -> anyhow::Result<()> {
        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    self.scan_warnings.push(format!("failed to read directory entry: {err}"));
                    continue;
                }
            };

            let path = entry.path();

            if path.is_dir() {
                // Store relative path from notes_dir
                if let Ok(relative) = path.strip_prefix(&self.notes_dir) {
                    self.folders.push(relative.to_path_buf());
                }
                self.scan_dir_recursive(&path)?;
                continue;
            }

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

    /// Create a new note with the given title inside the given folder
    /// (relative to `notes_dir`). Pass an empty path for the root.
    /// Returns the path of the created note.
    pub fn create_note(&mut self, title: &str, folder: &std::path::Path) -> anyhow::Result<PathBuf> {
        let target_dir = self.notes_dir.join(folder);
        let note = Note::create_new(&target_dir, title)?;
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

        // Generate new filename with collision avoidance.
        // Use the note's current parent directory so rename preserves folder location.
        let parent_dir = self.notes[index]
            .path
            .parent()
            .unwrap_or(&self.notes_dir)
            .to_path_buf();
        let base_slug = truncate_slug(&slug::slugify(new_title), MAX_SLUG_LEN);
        let mut filename = format!("{}.md", base_slug);
        let mut new_path = parent_dir.join(&filename);
        let mut counter = 1u32;
        while new_path.exists() && new_path != self.notes[index].path {
            filename = format!("{}-{}.md", base_slug, counter);
            new_path = parent_dir.join(&filename);
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

    // -- Folder operations ----------------------------------------------------

    /// Return the names of immediate subfolders of the given relative folder path.
    pub fn subfolders_of(&self, folder: &std::path::Path) -> Vec<String> {
        let mut result: Vec<String> = self
            .folders
            .iter()
            .filter(|f| {
                f.parent().map_or(false, |p| p == folder)
            })
            .filter_map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        result.sort();
        result
    }

    /// Return indices of notes whose parent directory matches `folder` exactly.
    /// `folder` is relative to `notes_dir` (empty path = root).
    pub fn notes_in_folder(&self, folder: &std::path::Path) -> Vec<usize> {
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, note)| {
                let relative = note
                    .path
                    .parent()
                    .and_then(|p| p.strip_prefix(&self.notes_dir).ok())
                    .unwrap_or_else(|| std::path::Path::new(""));
                relative == folder
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Create a new folder (subdirectory) at the given relative path.
    pub fn create_folder(&mut self, relative_path: &std::path::Path) -> anyhow::Result<()> {
        let full_path = self.notes_dir.join(relative_path);
        std::fs::create_dir(&full_path)
            .with_context(|| format!("failed to create folder: {}", full_path.display()))?;
        self.folders.push(relative_path.to_path_buf());
        self.folders.sort();
        Ok(())
    }

    /// Delete an empty folder at the given relative path.
    /// Fails if the folder is not empty (safe by default).
    pub fn delete_folder(&mut self, relative_path: &std::path::Path) -> anyhow::Result<()> {
        let full_path = self.notes_dir.join(relative_path);
        std::fs::remove_dir(&full_path)
            .with_context(|| format!("failed to delete folder (must be empty): {}", full_path.display()))?;
        self.folders.retain(|f| f != relative_path);
        Ok(())
    }

    /// Move a note to a different folder. `target_folder` is relative to `notes_dir`.
    pub fn move_note(&mut self, index: usize, target_folder: &std::path::Path) -> anyhow::Result<()> {
        if index >= self.notes.len() {
            bail!(
                "note index {} out of bounds (have {} notes)",
                index,
                self.notes.len()
            );
        }

        let filename = self.notes[index]
            .path
            .file_name()
            .context("note has no filename")?
            .to_os_string();

        let new_path = self.notes_dir.join(target_folder).join(&filename);

        if new_path == self.notes[index].path {
            return Ok(()); // already in target folder
        }

        std::fs::rename(&self.notes[index].path, &new_path)
            .with_context(|| format!(
                "failed to move {} -> {}",
                self.notes[index].path.display(),
                new_path.display()
            ))?;

        self.notes[index].path = new_path;
        Ok(())
    }

    /// Return all folder paths sorted, with an empty path (root) at position 0.
    /// Used by the MoveNote dialog.
    pub fn all_folder_paths(&self) -> Vec<PathBuf> {
        let mut result = vec![PathBuf::new()]; // root
        result.extend(self.folders.iter().cloned());
        result
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;
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

        let path = mgr.create_note("My New Note", Path::new("")).unwrap();

        assert_eq!(mgr.notes().len(), 1);
        assert_eq!(mgr.notes()[0].title, "My New Note");
        assert!(path.exists());
    }

    #[test]
    fn delete_note_removes_file_and_entry() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        let path = mgr.create_note("Delete Me", Path::new("")).unwrap();
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

        let old_path = mgr.create_note("Original Title", Path::new("")).unwrap();
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

        mgr.create_note("Refresh Test", Path::new("")).unwrap();
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

    // -- Recursive scan -------------------------------------------------------

    #[test]
    fn scan_finds_notes_in_subfolders() {
        let tmp = TempDir::new().unwrap();
        write_md_file(tmp.path(), "root.md", "Root Note", "at root");

        let sub = tmp.path().join("work");
        std::fs::create_dir(&sub).unwrap();
        write_md_file(&sub, "task.md", "Work Task", "in work");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        assert_eq!(mgr.notes().len(), 2);
        assert_eq!(mgr.folders.len(), 1);
        assert_eq!(mgr.folders[0], PathBuf::from("work"));
    }

    #[test]
    fn scan_finds_nested_subfolders() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("a")).unwrap();
        std::fs::create_dir(tmp.path().join("a/b")).unwrap();
        write_md_file(&tmp.path().join("a/b"), "deep.md", "Deep", "nested");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        assert_eq!(mgr.notes().len(), 1);
        assert!(mgr.folders.contains(&PathBuf::from("a")));
        assert!(mgr.folders.contains(&PathBuf::from("a/b")));
    }

    // -- Folder operations ----------------------------------------------------

    #[test]
    fn subfolders_of_root() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("work")).unwrap();
        std::fs::create_dir(tmp.path().join("personal")).unwrap();
        std::fs::create_dir(tmp.path().join("work/projects")).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        let root_subs = mgr.subfolders_of(Path::new(""));
        assert_eq!(root_subs, vec!["personal", "work"]);

        let work_subs = mgr.subfolders_of(Path::new("work"));
        assert_eq!(work_subs, vec!["projects"]);
    }

    #[test]
    fn notes_in_folder_filters_correctly() {
        let tmp = TempDir::new().unwrap();
        write_md_file(tmp.path(), "root.md", "Root", "at root");

        let sub = tmp.path().join("work");
        std::fs::create_dir(&sub).unwrap();
        write_md_file(&sub, "task.md", "Task", "in work");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        let root_notes = mgr.notes_in_folder(Path::new(""));
        assert_eq!(root_notes.len(), 1);

        let work_notes = mgr.notes_in_folder(Path::new("work"));
        assert_eq!(work_notes.len(), 1);
    }

    #[test]
    fn create_folder_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        mgr.create_folder(Path::new("projects")).unwrap();

        assert!(tmp.path().join("projects").is_dir());
        assert!(mgr.folders.contains(&PathBuf::from("projects")));
    }

    #[test]
    fn delete_folder_removes_empty_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("empty")).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();
        assert!(mgr.folders.contains(&PathBuf::from("empty")));

        mgr.delete_folder(Path::new("empty")).unwrap();
        assert!(!tmp.path().join("empty").exists());
        assert!(!mgr.folders.contains(&PathBuf::from("empty")));
    }

    #[test]
    fn delete_folder_fails_if_not_empty() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("notempty");
        std::fs::create_dir(&sub).unwrap();
        write_md_file(&sub, "note.md", "Note", "body");

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        let result = mgr.delete_folder(Path::new("notempty"));
        assert!(result.is_err());
    }

    #[test]
    fn move_note_to_subfolder() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        mgr.create_note("Movable", Path::new("")).unwrap();
        mgr.create_folder(Path::new("archive")).unwrap();

        let old_path = mgr.notes[0].path.clone();
        mgr.move_note(0, Path::new("archive")).unwrap();

        assert!(!old_path.exists());
        assert!(mgr.notes[0].path.exists());
        assert!(mgr.notes[0].path.starts_with(tmp.path().join("archive")));
    }

    #[test]
    fn move_note_to_root() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.create_note("InSub", Path::new("sub")).unwrap();

        assert!(mgr.notes[0].path.starts_with(&sub));

        mgr.move_note(0, Path::new("")).unwrap();
        assert!(mgr.notes[0].path.starts_with(tmp.path()));
        assert!(!mgr.notes[0].path.starts_with(&sub));
    }

    #[test]
    fn move_note_same_folder_noop() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

        mgr.create_note("Stay", Path::new("")).unwrap();
        let path_before = mgr.notes[0].path.clone();

        mgr.move_note(0, Path::new("")).unwrap();
        assert_eq!(mgr.notes[0].path, path_before);
    }

    #[test]
    fn all_folder_paths_includes_root() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("a")).unwrap();
        std::fs::create_dir(tmp.path().join("b")).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.scan().unwrap();

        let paths = mgr.all_folder_paths();
        assert_eq!(paths[0], PathBuf::new()); // root first
        assert!(paths.contains(&PathBuf::from("a")));
        assert!(paths.contains(&PathBuf::from("b")));
    }

    #[test]
    fn rename_note_in_subfolder_stays_in_subfolder() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("work");
        std::fs::create_dir(&sub).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.create_note("Original", Path::new("work")).unwrap();

        assert!(mgr.notes[0].path.starts_with(&sub));

        mgr.rename_note(0, "Renamed").unwrap();

        assert!(mgr.notes[0].path.starts_with(&sub), "renamed note should stay in subfolder");
        assert_eq!(mgr.notes[0].path.file_name().unwrap().to_str().unwrap(), "renamed.md");
    }

    #[test]
    fn create_note_in_subfolder() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("projects");
        std::fs::create_dir(&sub).unwrap();

        let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
        let path = mgr.create_note("Project Plan", Path::new("projects")).unwrap();

        assert!(path.starts_with(&sub));
        assert!(path.exists());
    }
}
