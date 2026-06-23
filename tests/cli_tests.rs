use std::fs;
use std::path::Path;

use tempfile::TempDir;

use deez_notes::cli;
use deez_notes::core::note_manager::NoteManager;

/// Write a .md file with YAML front matter into the given directory.
fn write_md(dir: &Path, filename: &str, title: &str, tags: &[&str], body: &str) {
    let tags_yaml: Vec<String> = tags.iter().map(|t| format!("\"{}\"", t)).collect();
    let content = format!(
        "---\ntitle: \"{title}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: [{}]\n---\n\n{body}",
        tags_yaml.join(", ")
    );
    fs::write(dir.join(filename), content).unwrap();
}

fn manager_with(dir: &TempDir) -> NoteManager {
    let mut m = NoteManager::new(dir.path().to_path_buf()).unwrap();
    m.scan().unwrap();
    m
}

#[test]
fn list_human_outputs_tab_separated_path_and_title() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &[], "body");
    let m = manager_with(&dir);

    let out = cli::list(&m, None, None, false);

    assert_eq!(out, "alpha.md\tAlpha");
}

#[test]
fn list_json_outputs_array_with_path_and_title() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &["work"], "body");
    let m = manager_with(&dir);

    let out = cli::list(&m, None, None, true);

    assert!(out.contains("\"path\": \"alpha.md\""), "got: {out}");
    assert!(out.contains("\"title\": \"Alpha\""), "got: {out}");
    assert!(out.contains("\"work\""), "got: {out}");
}
