use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use deez_notes::app::SortMode;
use deez_notes::core::front_matter::{parse_front_matter, write_front_matter, FrontMatter};
use deez_notes::core::note_manager::NoteManager;
use deez_notes::core::search::fuzzy_search;
use deez_notes::core::sort::sort_notes;
use deez_notes::core::tags::{collect_all_tags, filter_by_tag};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a .md file with YAML front matter into the given directory.
fn write_md(dir: &Path, filename: &str, title: &str, tags: &[&str], body: &str) {
    let tags_yaml: Vec<String> = tags.iter().map(|t| format!("\"{}\"", t)).collect();
    let tags_str = tags_yaml.join(", ");
    let content = format!(
        "---\ntitle: \"{title}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: [{tags_str}]\n---\n\n{body}"
    );
    fs::write(dir.join(filename), content).unwrap();
}

/// Write a .md file with specific created/modified timestamps.
fn write_md_with_dates(
    dir: &Path,
    filename: &str,
    title: &str,
    created: &str,
    modified: &str,
) {
    let content = format!(
        "---\ntitle: \"{title}\"\ncreated: {created}\nmodified: {modified}\ntags: []\n---\n\nBody of {title}."
    );
    fs::write(dir.join(filename), content).unwrap();
}

// ===========================================================================
// 1. CRUD Complete Cycle
// ===========================================================================

#[test]
fn crud_complete_cycle() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    // Create
    let path = mgr.create_note("Test", std::path::Path::new("")).unwrap();
    assert!(path.exists(), "created file should exist on disk");
    assert_eq!(mgr.notes().len(), 1);

    // Get content
    let content = mgr.get_content(0).unwrap();
    assert!(content.is_empty() || content.trim().is_empty(), "new note body should be empty");

    // Modify content by writing file externally
    let raw = fs::read_to_string(&path).unwrap();
    let (fm, _body) = parse_front_matter(&raw);
    let fm = fm.unwrap();
    let new_body = "Updated content here";
    let output = write_front_matter(&fm, new_body);
    fs::write(&path, output).unwrap();

    // Refresh and verify content changed
    mgr.refresh_note(0).unwrap();
    let content = mgr.get_content(0).unwrap();
    assert!(
        content.contains("Updated content here"),
        "content should reflect the external modification"
    );

    // Rename
    let old_path = mgr.notes()[0].path.clone();
    mgr.rename_note(0, "Renamed").unwrap();
    assert!(!old_path.exists(), "old file should be gone after rename");
    assert!(mgr.notes()[0].path.exists(), "new file should exist");
    assert_eq!(mgr.notes()[0].title, "Renamed");

    // Delete
    let renamed_path = mgr.notes()[0].path.clone();
    mgr.delete_note(0).unwrap();
    assert!(!renamed_path.exists(), "file should be gone after delete");
    assert_eq!(mgr.notes().len(), 0, "manager should be empty after delete");
}

// ===========================================================================
// 2. Multiple Notes CRUD
// ===========================================================================

#[test]
fn multiple_notes_crud() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    // Create 5 notes
    let titles = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"];
    for title in &titles {
        mgr.create_note(title, std::path::Path::new("")).unwrap();
    }
    assert_eq!(mgr.notes().len(), 5);

    // Verify all 5 exist on disk
    for note in mgr.notes() {
        assert!(note.path.exists(), "note file should exist: {}", note.path.display());
    }

    // Find the index of "Gamma" (the middle one) and delete it
    let gamma_idx = mgr
        .notes()
        .iter()
        .position(|n| n.title == "Gamma")
        .expect("Gamma should exist");
    let gamma_path = mgr.notes()[gamma_idx].path.clone();
    mgr.delete_note(gamma_idx).unwrap();

    assert_eq!(mgr.notes().len(), 4);
    assert!(!gamma_path.exists(), "Gamma file should be gone");
    let remaining_titles: Vec<&str> = mgr.notes().iter().map(|n| n.title.as_str()).collect();
    assert!(!remaining_titles.contains(&"Gamma"), "Gamma should not be in the list");
    for expected in &["Alpha", "Beta", "Delta", "Epsilon"] {
        assert!(
            remaining_titles.contains(expected),
            "{} should still be present",
            expected
        );
    }

    // Rename "Beta" to "Beta Renamed"
    let beta_idx = mgr
        .notes()
        .iter()
        .position(|n| n.title == "Beta")
        .expect("Beta should exist");
    let old_beta_path = mgr.notes()[beta_idx].path.clone();
    mgr.rename_note(beta_idx, "Beta Renamed").unwrap();

    assert!(!old_beta_path.exists(), "old Beta file should be gone");
    assert_eq!(mgr.notes()[beta_idx].title, "Beta Renamed");
    assert!(mgr.notes()[beta_idx].path.exists(), "renamed Beta file should exist");
}

// ===========================================================================
// 3. Empty Directory
// ===========================================================================

#[test]
fn empty_directory_scan() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    mgr.scan().unwrap();
    assert_eq!(mgr.notes().len(), 0, "empty dir should yield 0 notes");

    mgr.create_note("First Note", std::path::Path::new("")).unwrap();
    // Rescan to simulate a fresh load
    mgr.scan().unwrap();
    assert_eq!(mgr.notes().len(), 1, "should find 1 note after create + scan");
}

// ===========================================================================
// 4. Performance: 1000 Notes
// ===========================================================================

#[test]
fn performance_1000_notes_scan() {
    let tmp = TempDir::new().unwrap();

    // Write 1000 .md files with front matter
    for i in 0..1000 {
        let filename = format!("note-{:04}.md", i);
        let title = format!("Test Note {}", i);
        write_md(tmp.path(), &filename, &title, &["perf"], "Some body text.");
    }

    let start = Instant::now();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let elapsed = start.elapsed();

    assert_eq!(mgr.notes().len(), 1000);
    assert!(
        elapsed < Duration::from_secs(2),
        "scan of 1000 notes took {:?}, expected < 2s",
        elapsed
    );
}

#[test]
fn performance_1000_notes_fuzzy_search() {
    let tmp = TempDir::new().unwrap();

    for i in 0..1000 {
        let filename = format!("note-{:04}.md", i);
        let title = format!("Test Note {}", i);
        write_md(tmp.path(), &filename, &title, &[], "Body text.");
    }

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();

    let start = Instant::now();
    let results = fuzzy_search("test", notes, false);
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "should find matches for 'test'");
    assert!(
        elapsed < Duration::from_millis(500),
        "fuzzy_search with 1000 notes took {:?}, expected < 500ms",
        elapsed
    );
}

#[test]
fn performance_1000_notes_sort() {
    let tmp = TempDir::new().unwrap();

    for i in 0..1000 {
        let filename = format!("note-{:04}.md", i);
        let title = format!("Test Note {}", i);
        write_md(tmp.path(), &filename, &title, &[], "Body.");
    }

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();
    let mut indices: Vec<usize> = (0..notes.len()).collect();

    let start = Instant::now();
    sort_notes(&mut indices, notes, SortMode::ByTitle, true);
    let elapsed = start.elapsed();

    assert_eq!(indices.len(), 1000);
    assert!(
        elapsed < Duration::from_millis(100),
        "sort of 1000 notes took {:?}, expected < 100ms",
        elapsed
    );
}

// ===========================================================================
// 5. Search Integration
// ===========================================================================

#[test]
fn search_by_known_titles() {
    let tmp = TempDir::new().unwrap();
    let titles_and_files = [
        ("rust-tutorial.md", "Rust Tutorial"),
        ("python-guide.md", "Python Guide"),
        ("javascript-tips.md", "JavaScript Tips"),
        ("rust-async.md", "Rust Async"),
        ("go-basics.md", "Go Basics"),
    ];
    for (filename, title) in &titles_and_files {
        write_md(tmp.path(), filename, title, &[], "");
    }

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();

    // Search "rust" -> should match "Rust Tutorial" and "Rust Async"
    let results = fuzzy_search("rust", notes, false);
    let matched_titles: Vec<&str> = results.iter().map(|&i| notes[i].title.as_str()).collect();
    assert!(
        matched_titles.contains(&"Rust Tutorial"),
        "should find 'Rust Tutorial', got: {:?}",
        matched_titles
    );
    assert!(
        matched_titles.contains(&"Rust Async"),
        "should find 'Rust Async', got: {:?}",
        matched_titles
    );
    assert_eq!(
        results.len(),
        2,
        "only 2 notes should match 'rust', got: {:?}",
        matched_titles
    );

    // Search "xyz" -> should return empty
    let results = fuzzy_search("xyz", notes, false);
    assert!(results.is_empty(), "no notes should match 'xyz'");

    // Search "" -> should return all
    let results = fuzzy_search("", notes, false);
    assert_eq!(results.len(), 5, "empty query should return all notes");
}

// ===========================================================================
// 6. Tags Integration
// ===========================================================================

#[test]
fn tags_collect_and_filter() {
    let tmp = TempDir::new().unwrap();
    write_md(tmp.path(), "note1.md", "Note1", &["rust", "web"], "");
    write_md(tmp.path(), "note2.md", "Note2", &["rust", "cli"], "");
    write_md(tmp.path(), "note3.md", "Note3", &["python"], "");

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();

    // collect_all_tags returns lowercase-normalized, sorted
    let all_tags = collect_all_tags(notes);
    assert_eq!(all_tags, vec!["cli", "python", "rust", "web"]);

    // filter_by_tag "rust" should return Note1 and Note2
    let rust_indices = filter_by_tag(notes, "rust");
    assert_eq!(rust_indices.len(), 2, "two notes should have 'rust' tag");
    let rust_titles: Vec<&str> = rust_indices
        .iter()
        .map(|&i| notes[i].title.as_str())
        .collect();
    assert!(rust_titles.contains(&"Note1"));
    assert!(rust_titles.contains(&"Note2"));

    // filter_by_tag "python" should return only Note3
    let python_indices = filter_by_tag(notes, "python");
    assert_eq!(python_indices.len(), 1);
    assert_eq!(notes[python_indices[0]].title, "Note3");

    // filter_by_tag "nonexistent" should return empty
    let empty = filter_by_tag(notes, "nonexistent");
    assert!(empty.is_empty());
}

// ===========================================================================
// 7. Sort Integration
// ===========================================================================

#[test]
fn sort_by_title_ascending_and_descending() {
    let tmp = TempDir::new().unwrap();
    write_md(tmp.path(), "cherry.md", "Cherry", &[], "");
    write_md(tmp.path(), "apple.md", "Apple", &[], "");
    write_md(tmp.path(), "banana.md", "Banana", &[], "");

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();
    let mut indices: Vec<usize> = (0..notes.len()).collect();

    // Ascending by title
    sort_notes(&mut indices, notes, SortMode::ByTitle, true);
    let titles_asc: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
    assert_eq!(titles_asc, vec!["Apple", "Banana", "Cherry"]);

    // Descending by title
    sort_notes(&mut indices, notes, SortMode::ByTitle, false);
    let titles_desc: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
    assert_eq!(titles_desc, vec!["Cherry", "Banana", "Apple"]);
}

#[test]
fn sort_by_modified_descending() {
    let tmp = TempDir::new().unwrap();
    write_md_with_dates(
        tmp.path(),
        "old.md",
        "Old Note",
        "2025-01-01T00:00:00",
        "2025-01-01T00:00:00",
    );
    write_md_with_dates(
        tmp.path(),
        "mid.md",
        "Mid Note",
        "2025-06-01T00:00:00",
        "2025-06-01T00:00:00",
    );
    write_md_with_dates(
        tmp.path(),
        "new.md",
        "New Note",
        "2025-12-01T00:00:00",
        "2025-12-01T00:00:00",
    );

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();
    let notes = mgr.notes();
    let mut indices: Vec<usize> = (0..notes.len()).collect();

    // Descending by modified (newest first)
    sort_notes(&mut indices, notes, SortMode::ByModified, false);
    let titles: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
    assert_eq!(titles, vec!["New Note", "Mid Note", "Old Note"]);

    // Ascending by modified (oldest first)
    sort_notes(&mut indices, notes, SortMode::ByModified, true);
    let titles: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
    assert_eq!(titles, vec!["Old Note", "Mid Note", "New Note"]);
}

// ===========================================================================
// 8. Front Matter Round-Trip
// ===========================================================================

#[test]
fn front_matter_round_trip() {
    let tmp = TempDir::new().unwrap();

    // Write a note with specific front matter
    write_md(
        tmp.path(),
        "round-trip.md",
        "Round Trip Test",
        &["alpha", "beta", "gamma"],
        "Original body content.",
    );

    // Read it back
    let raw = fs::read_to_string(tmp.path().join("round-trip.md")).unwrap();
    let (fm, body) = parse_front_matter(&raw);
    let fm = fm.expect("front matter should parse");

    assert_eq!(fm.title, "Round Trip Test");
    assert_eq!(fm.tags, vec!["alpha", "beta", "gamma"]);
    assert!(body.contains("Original body content."));

    // Modify tags and write back
    let updated_fm = FrontMatter {
        title: fm.title.clone(),
        created: fm.created,
        modified: fm.modified,
        tags: vec!["alpha".to_string(), "delta".to_string()],
    };
    let output = write_front_matter(&updated_fm, &body);
    fs::write(tmp.path().join("round-trip.md"), output).unwrap();

    // Read again and verify
    let raw2 = fs::read_to_string(tmp.path().join("round-trip.md")).unwrap();
    let (fm2, body2) = parse_front_matter(&raw2);
    let fm2 = fm2.expect("updated front matter should parse");

    assert_eq!(fm2.title, "Round Trip Test");
    assert_eq!(fm2.tags, vec!["alpha", "delta"]);
    assert!(body2.contains("Original body content."));
}

#[test]
fn front_matter_preserves_dates() {
    let tmp = TempDir::new().unwrap();
    write_md_with_dates(
        tmp.path(),
        "dates.md",
        "Date Test",
        "2025-03-15T10:30:00",
        "2025-06-20T14:45:00",
    );

    let raw = fs::read_to_string(tmp.path().join("dates.md")).unwrap();
    let (fm, _body) = parse_front_matter(&raw);
    let fm = fm.expect("front matter should parse");

    assert_eq!(fm.created.format("%Y-%m-%d").to_string(), "2025-03-15");
    assert_eq!(fm.modified.format("%H:%M:%S").to_string(), "14:45:00");

    // Round-trip: write and re-read
    let output = write_front_matter(&fm, "Body.");
    let (fm2, _) = parse_front_matter(&output);
    let fm2 = fm2.expect("round-tripped front matter should parse");

    assert_eq!(
        fm.created.format("%Y-%m-%dT%H:%M:%S").to_string(),
        fm2.created.format("%Y-%m-%dT%H:%M:%S").to_string(),
    );
    assert_eq!(
        fm.modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
        fm2.modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
    );
}

// ===========================================================================
// 9. Edge Cases
// ===========================================================================

#[test]
fn note_with_empty_content_only_front_matter() {
    let tmp = TempDir::new().unwrap();
    let content = "---\ntitle: \"Empty Body\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\n\n";
    fs::write(tmp.path().join("empty-body.md"), content).unwrap();

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    assert_eq!(mgr.notes().len(), 1);
    assert_eq!(mgr.notes()[0].title, "Empty Body");

    let body = mgr.get_content(0).unwrap();
    assert!(body.trim().is_empty(), "body should be empty or whitespace");
}

#[test]
fn note_with_no_front_matter() {
    let tmp = TempDir::new().unwrap();
    let content = "# Pure Markdown\n\nNo front matter at all.";
    fs::write(tmp.path().join("no-fm.md"), content).unwrap();

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    assert_eq!(mgr.notes().len(), 1);
    // Title should be derived from filename
    assert_eq!(mgr.notes()[0].title, "No fm");
    assert!(mgr.notes()[0].tags.is_empty());
}

#[test]
fn note_with_very_long_content() {
    let tmp = TempDir::new().unwrap();

    // Generate ~100KB of content
    let long_body = "x".repeat(100_000);
    write_md(
        tmp.path(),
        "long-note.md",
        "Long Note",
        &[],
        &long_body,
    );

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    assert_eq!(mgr.notes().len(), 1);
    let content = mgr.get_content(0).unwrap();
    assert!(
        content.len() >= 100_000,
        "content should be at least 100KB, got {} bytes",
        content.len()
    );
}

#[test]
fn non_md_files_ignored() {
    let tmp = TempDir::new().unwrap();
    write_md(tmp.path(), "real-note.md", "Real Note", &[], "Body.");
    fs::write(tmp.path().join("readme.txt"), "Not a note").unwrap();
    fs::write(tmp.path().join("data.json"), "{}").unwrap();
    fs::write(tmp.path().join("image.png"), [0u8; 10]).unwrap();

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    assert_eq!(mgr.notes().len(), 1, "only .md files should be loaded");
    assert_eq!(mgr.notes()[0].title, "Real Note");
}

#[test]
fn hidden_md_files_are_loaded() {
    // The current implementation loads any .md file — including hidden ones.
    // This test documents that behavior.
    let tmp = TempDir::new().unwrap();
    write_md(tmp.path(), ".hidden.md", "Hidden Note", &[], "Secret.");
    write_md(tmp.path(), "visible.md", "Visible Note", &[], "Public.");

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    // Both files have .md extension, so both should be loaded
    assert_eq!(mgr.notes().len(), 2);
}

#[test]
fn subdirectories_are_scanned_recursively() {
    let tmp = TempDir::new().unwrap();
    write_md(tmp.path(), "note.md", "Note", &[], "Body.");

    // Create a subdirectory with a note inside
    fs::create_dir(tmp.path().join("subdir")).unwrap();
    fs::write(tmp.path().join("subdir").join("nested.md"), "---\ntitle: Nested\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\n\n").unwrap();

    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();
    mgr.scan().unwrap();

    // Both notes should be found (scan is recursive)
    assert_eq!(mgr.notes().len(), 2);
    assert_eq!(mgr.folders.len(), 1);
    assert_eq!(mgr.folders[0], std::path::PathBuf::from("subdir"));
}

// ===========================================================================
// 10. Concurrent-like Scenarios
// ===========================================================================

#[test]
fn file_deleted_externally_then_scan() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    let path = mgr.create_note("Ephemeral", std::path::Path::new("")).unwrap();
    assert_eq!(mgr.notes().len(), 1);

    // Manually delete the file from disk
    fs::remove_file(&path).unwrap();

    // Rescan should handle the missing file gracefully
    mgr.scan().unwrap();
    assert_eq!(
        mgr.notes().len(),
        0,
        "after external delete + scan, should have 0 notes"
    );
}

#[test]
fn file_modified_externally_then_refresh() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    mgr.create_note("Mutable Note", std::path::Path::new("")).unwrap();
    assert_eq!(mgr.notes()[0].title, "Mutable Note");

    // Modify the file on disk
    let path = mgr.notes()[0].path.clone();
    let new_content = "---\ntitle: \"Externally Modified\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-06-15T12:00:00\ntags: [external]\n---\n\nNew body from external edit.";
    fs::write(&path, new_content).unwrap();

    // Refresh the specific note
    mgr.refresh_note(0).unwrap();

    assert_eq!(mgr.notes()[0].title, "Externally Modified");
    assert_eq!(mgr.notes()[0].tags, vec!["external"]);

    // Content should be lazy-loaded (None after refresh)
    assert!(mgr.notes()[0].content.is_none());

    // Load content and verify
    let content = mgr.get_content(0).unwrap();
    assert!(content.contains("New body from external edit."));
}

#[test]
fn create_note_then_delete_file_then_delete_note_errors() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = NoteManager::new(tmp.path().to_path_buf()).unwrap();

    let path = mgr.create_note("Doomed", std::path::Path::new("")).unwrap();

    // Delete file externally
    fs::remove_file(&path).unwrap();

    // Trying to delete via manager should fail since the file is already gone
    let result = mgr.delete_note(0);
    assert!(result.is_err(), "delete should error when file is missing");
}
