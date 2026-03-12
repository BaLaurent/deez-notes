use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Local};

use crate::core::front_matter::{new_front_matter, parse_front_matter, write_front_matter, FrontMatter};

/// A single Markdown note loaded from ~/notes/.
/// Content is lazy-loaded: `None` means not yet read from disk.
#[derive(Debug, Clone)]
pub struct Note {
    /// Absolute path to the .md file
    pub path: PathBuf,
    /// Title extracted from front matter (or filename fallback)
    pub title: String,
    /// Full markdown body (lazy-loaded, None until explicitly read)
    pub content: Option<String>,
    /// Tags from front matter
    pub tags: Vec<String>,
    /// Creation timestamp from front matter
    pub created: DateTime<Local>,
    /// Last modification timestamp (from filesystem or front matter)
    pub modified: DateTime<Local>,
}

impl Note {
    /// Load note metadata from a file path. Content is NOT loaded (lazy pattern).
    ///
    /// If the file has valid front matter, fields are extracted from it.
    /// Otherwise, title is derived from the filename and timestamps from file metadata.
    pub fn load_from_path(path: PathBuf) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read note: {}", path.display()))?;

        let (fm, _body) = parse_front_matter(&raw);

        match fm {
            Some(fm) => Ok(Note {
                title: fm.title,
                tags: fm.tags,
                created: fm.created,
                modified: fm.modified,
                path,
                content: None,
            }),
            None => {
                let title = Self::title_from_path(&path);
                let meta = std::fs::metadata(&path)
                    .with_context(|| format!("failed to read metadata: {}", path.display()))?;
                let created = meta
                    .created()
                    .map(DateTime::<Local>::from)
                    .unwrap_or_else(|_| Local::now());
                let modified = meta
                    .modified()
                    .map(DateTime::<Local>::from)
                    .unwrap_or_else(|_| Local::now());

                Ok(Note {
                    path,
                    title,
                    content: None,
                    tags: Vec::new(),
                    created,
                    modified,
                })
            }
        }
    }

    /// Lazy-load the markdown body from disk, discarding front matter.
    pub fn load_content(&mut self) -> anyhow::Result<()> {
        let raw = std::fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read note content: {}", self.path.display()))?;

        let (_fm, body) = parse_front_matter(&raw);
        self.content = Some(body);
        Ok(())
    }

    /// Load content if not yet loaded, then return a reference to it.
    pub fn ensure_content(&mut self) -> anyhow::Result<&str> {
        if self.content.is_none() {
            self.load_content()?;
        }
        Ok(self.content.as_deref().unwrap_or(""))
    }

    /// Write the note body to disk with updated front matter.
    /// The `modified` timestamp in the front matter is set to now.
    pub fn save_content(&self, body: &str) -> anyhow::Result<()> {
        let fm = FrontMatter {
            title: self.title.clone(),
            created: self.created,
            modified: Local::now(),
            tags: self.tags.clone(),
        };
        let output = write_front_matter(&fm, body);
        std::fs::write(&self.path, output)
            .with_context(|| format!("failed to write note: {}", self.path.display()))?;
        Ok(())
    }

    /// Create a new note file on disk with the given title.
    ///
    /// The filename is a slug of the title. If the file already exists,
    /// a numeric suffix (`-1`, `-2`, ...) is appended to avoid collisions.
    pub fn create_new(notes_dir: &Path, title: &str) -> anyhow::Result<Self> {
        let base_slug = truncate_slug(&slug::slugify(title), MAX_SLUG_LEN);
        let mut filename = format!("{}.md", base_slug);
        let mut path = notes_dir.join(&filename);
        let mut counter = 1u32;
        while path.exists() {
            filename = format!("{}-{}.md", base_slug, counter);
            path = notes_dir.join(&filename);
            counter += 1;
        }

        let fm = new_front_matter(title);
        let output = write_front_matter(&fm, "");
        std::fs::write(&path, &output)
            .with_context(|| format!("failed to create note: {}", path.display()))?;

        Ok(Note {
            path,
            title: fm.title,
            content: None,
            tags: fm.tags,
            created: fm.created,
            modified: fm.modified,
        })
    }

    /// Derive a human-readable title from a file path.
    ///
    /// Strips the `.md` extension, replaces `-` and `_` with spaces,
    /// and capitalizes the first letter.
    pub fn title_from_path(path: &Path) -> String {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");

        let spaced = stem.replace(['-', '_'], " ");

        let mut chars = spaced.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => {
                let upper: String = first.to_uppercase().collect();
                upper + chars.as_str()
            }
        }
    }
}

/// Maximum slug length to avoid filesystem filename-too-long errors.
/// Most filesystems limit filenames to 255 bytes; we leave room for
/// the `.md` extension and collision suffixes like `-999`.
pub(crate) const MAX_SLUG_LEN: usize = 200;

/// Truncate a slug to `max_len` characters, breaking at a hyphen boundary
/// when possible to keep the slug clean.
pub(crate) fn truncate_slug(slug: &str, max_len: usize) -> String {
    if slug.len() <= max_len {
        return slug.to_string();
    }
    let truncated = &slug[..max_len];
    // Try to break at the last hyphen for a cleaner name
    match truncated.rfind('-') {
        Some(pos) if pos > max_len / 2 => truncated[..pos].to_string(),
        _ => truncated.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: write a temp .md file with the given content and return its path.
    fn write_temp_note(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_from_path_with_front_matter() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
title: "My Test Note"
created: 2025-03-11T14:30:00
modified: 2025-03-11T15:45:00
tags: [rust, testing]
---

Some body text here."#;
        let path = write_temp_note(&dir, "my-test-note.md", content);

        let note = Note::load_from_path(path.clone()).unwrap();

        assert_eq!(note.title, "My Test Note");
        assert_eq!(note.tags, vec!["rust", "testing"]);
        assert_eq!(note.created.format("%Y-%m-%d").to_string(), "2025-03-11");
        assert_eq!(note.modified.format("%H:%M:%S").to_string(), "15:45:00");
        assert!(note.content.is_none(), "content should be lazy-loaded");
        assert_eq!(note.path, path);
    }

    #[test]
    fn load_from_path_without_front_matter() {
        let dir = TempDir::new().unwrap();
        let content = "# Just markdown\n\nNo front matter.";
        let path = write_temp_note(&dir, "hello-world.md", content);

        let note = Note::load_from_path(path).unwrap();

        assert_eq!(note.title, "Hello world");
        assert!(note.tags.is_empty());
        assert!(note.content.is_none());
    }

    #[test]
    fn load_content_lazy() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
title: Lazy
created: 2025-01-01T00:00:00
modified: 2025-01-01T00:00:00
tags: []
---

Body content here."#;
        let path = write_temp_note(&dir, "lazy.md", content);

        let mut note = Note::load_from_path(path).unwrap();
        assert!(note.content.is_none());

        note.load_content().unwrap();
        assert!(note.content.is_some());
        assert!(note.content.as_ref().unwrap().contains("Body content here."));
    }

    #[test]
    fn ensure_content_loads_on_demand() {
        let dir = TempDir::new().unwrap();
        let content = r#"---
title: OnDemand
created: 2025-01-01T00:00:00
modified: 2025-01-01T00:00:00
tags: []
---

Loaded on demand."#;
        let path = write_temp_note(&dir, "ondemand.md", content);

        let mut note = Note::load_from_path(path).unwrap();
        assert!(note.content.is_none());

        let text = note.ensure_content().unwrap();
        assert!(text.contains("Loaded on demand."));
        // Second call should not re-read (content already loaded)
        assert!(note.content.is_some());
    }

    #[test]
    fn save_content_writes_fm() {
        let dir = TempDir::new().unwrap();
        let initial = r#"---
title: SaveTest
created: 2025-06-01T10:00:00
modified: 2025-06-01T10:00:00
tags: [save]
---

Old body."#;
        let path = write_temp_note(&dir, "save-test.md", initial);

        let note = Note::load_from_path(path.clone()).unwrap();
        note.save_content("New body content").unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("SaveTest"));
        assert!(raw.contains("New body content"));
        // Verify we can re-parse the saved file
        let reloaded = Note::load_from_path(path).unwrap();
        assert_eq!(reloaded.title, "SaveTest");
        assert_eq!(reloaded.tags, vec!["save"]);
    }

    #[test]
    fn create_new_generates_slug() {
        let dir = TempDir::new().unwrap();

        let note = Note::create_new(dir.path(), "My Awesome Note").unwrap();

        assert_eq!(note.title, "My Awesome Note");
        assert!(note.content.is_none());
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "my-awesome-note.md");
        assert!(note.path.exists());
    }

    #[test]
    fn create_new_avoids_collision() {
        let dir = TempDir::new().unwrap();

        let note1 = Note::create_new(dir.path(), "Duplicate Title").unwrap();
        let note2 = Note::create_new(dir.path(), "Duplicate Title").unwrap();

        assert_ne!(note1.path, note2.path);
        let name1 = note1.path.file_name().unwrap().to_str().unwrap();
        let name2 = note2.path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name1, "duplicate-title.md");
        assert_eq!(name2, "duplicate-title-1.md");
    }

    #[test]
    fn title_from_path_basic() {
        assert_eq!(
            Note::title_from_path(Path::new("/notes/hello-world.md")),
            "Hello world"
        );
        assert_eq!(
            Note::title_from_path(Path::new("/notes/my_great_note.md")),
            "My great note"
        );
        assert_eq!(
            Note::title_from_path(Path::new("/notes/simple.md")),
            "Simple"
        );
    }

    #[test]
    fn create_new_path_traversal_in_title() {
        let dir = TempDir::new().unwrap();

        // Titles containing path traversal should produce safe slugified filenames
        let note = Note::create_new(dir.path(), "../../../etc/passwd").unwrap();
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        // slug::slugify should strip or replace dangerous characters
        assert!(!filename.contains(".."));
        assert!(!filename.contains('/'));
        assert!(note.path.exists());
    }

    #[test]
    fn create_new_special_characters_in_title() {
        let dir = TempDir::new().unwrap();

        let note = Note::create_new(dir.path(), r#"Test ? | \ " < > * : note"#).unwrap();
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        // slug should strip all unsafe filesystem characters
        assert!(!filename.contains('?'));
        assert!(!filename.contains('|'));
        assert!(!filename.contains('\\'));
        assert!(!filename.contains('"'));
        assert!(!filename.contains('<'));
        assert!(!filename.contains('>'));
        assert!(!filename.contains('*'));
        assert!(!filename.contains(':'));
        assert!(note.path.exists());
    }

    #[test]
    fn create_new_unicode_emoji_in_title() {
        let dir = TempDir::new().unwrap();

        let note = Note::create_new(dir.path(), "\u{1F4DD} Ma note g\u{00E9}niale").unwrap();
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        // slug::slugify converts unicode to ASCII equivalents or strips them
        assert!(filename.ends_with(".md"));
        assert!(note.path.exists());
        assert_eq!(note.title, "\u{1F4DD} Ma note g\u{00E9}niale");
    }

    #[test]
    fn create_new_very_long_title() {
        let dir = TempDir::new().unwrap();
        let long_title = "A".repeat(500);

        let note = Note::create_new(dir.path(), &long_title).unwrap();
        assert!(note.path.exists());
        assert_eq!(note.title, long_title);
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        assert!(filename.ends_with(".md"));
    }

    #[test]
    fn create_new_empty_title() {
        let dir = TempDir::new().unwrap();

        let note = Note::create_new(dir.path(), "").unwrap();
        assert!(note.path.exists());
        // The file should have been created even with empty title
        let filename = note.path.file_name().unwrap().to_str().unwrap();
        assert!(filename.ends_with(".md"));
    }

    #[test]
    fn create_new_five_notes_same_title_all_unique() {
        let dir = TempDir::new().unwrap();

        let mut paths = Vec::new();
        for _ in 0..5 {
            let note = Note::create_new(dir.path(), "Same Title").unwrap();
            assert!(note.path.exists());
            assert!(!paths.contains(&note.path), "paths should be unique");
            paths.push(note.path);
        }
        assert_eq!(paths.len(), 5);

        // Verify filenames follow the collision pattern
        let filenames: Vec<String> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(filenames.contains(&"same-title.md".to_string()));
        assert!(filenames.contains(&"same-title-1.md".to_string()));
        assert!(filenames.contains(&"same-title-2.md".to_string()));
        assert!(filenames.contains(&"same-title-3.md".to_string()));
        assert!(filenames.contains(&"same-title-4.md".to_string()));
    }

    #[test]
    fn load_note_from_file_with_empty_content() {
        let dir = TempDir::new().unwrap();
        let path = write_temp_note(&dir, "empty.md", "");

        let note = Note::load_from_path(path).unwrap();
        // No front matter, so title derived from filename
        assert_eq!(note.title, "Empty");
        assert!(note.content.is_none());
    }

    #[test]
    fn load_note_from_file_with_only_front_matter_no_body() {
        let dir = TempDir::new().unwrap();
        let content = "---\ntitle: Only FM\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\n";
        let path = write_temp_note(&dir, "only-fm.md", content);

        let mut note = Note::load_from_path(path).unwrap();
        assert_eq!(note.title, "Only FM");

        note.load_content().unwrap();
        // Body should be empty or whitespace-only
        let body = note.content.as_deref().unwrap_or("");
        assert!(body.trim().is_empty(), "body should be empty but got: {:?}", body);
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let dir = TempDir::new().unwrap();
        let body_text = "This is the body.\n\nWith multiple paragraphs.\n\n- And a list\n- Of items";

        let note = Note::create_new(dir.path(), "Roundtrip Test").unwrap();
        note.save_content(body_text).unwrap();

        let mut reloaded = Note::load_from_path(note.path.clone()).unwrap();
        assert_eq!(reloaded.title, "Roundtrip Test");

        reloaded.load_content().unwrap();
        let reloaded_body = reloaded.content.as_deref().unwrap();
        assert_eq!(reloaded_body, body_text);
    }
}
