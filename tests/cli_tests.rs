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

#[test]
fn list_filters_by_folder_and_tag() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("work")).unwrap();
    write_md(dir.path(), "root.md", "Root", &["home"], "b");
    write_md(&dir.path().join("work"), "task.md", "Task", &["urgent"], "b");
    write_md(&dir.path().join("work"), "idea.md", "Idea", &["home"], "b");
    let m = manager_with(&dir);

    // Folder filter: exact folder, non-recursive.
    let work = cli::list(&m, Some("work"), None, false);
    assert!(work.contains("work/task.md"), "got: {work}");
    assert!(work.contains("work/idea.md"), "got: {work}");
    assert!(!work.contains("root.md"), "got: {work}");

    // Folder + tag.
    let urgent = cli::list(&m, Some("work"), Some("urgent"), false);
    assert_eq!(urgent, "work/task.md\tTask");
}

#[test]
fn search_finds_by_fuzzy_title() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &[], "b");
    write_md(dir.path(), "beta.md", "Beta", &[], "b");
    let m = manager_with(&dir);

    let out = cli::search(&m, "alph", false);

    assert!(out.contains("alpha.md\tAlpha"), "got: {out}");
    assert!(!out.contains("beta.md"), "got: {out}");
}

#[test]
fn resolve_note_prefers_exact_path_then_fuzzy() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &[], "b");
    write_md(dir.path(), "beta.md", "Beta Notes", &[], "b");
    let m = manager_with(&dir);

    // Exact relative path.
    let by_path = cli::resolve_note(&m, "beta.md").unwrap();
    assert_eq!(cli::relative_path(&m, &m.notes()[by_path]), "beta.md");

    // Fuzzy title fallback.
    let by_title = cli::resolve_note(&m, "beta notes").unwrap();
    assert_eq!(cli::relative_path(&m, &m.notes()[by_title]), "beta.md");

    // No match -> error.
    assert!(cli::resolve_note(&m, "zzz-nope").is_err());
}

#[test]
fn get_returns_body_without_front_matter() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &[], "Hello body.");
    let mut m = manager_with(&dir);

    let body = cli::get(&mut m, "alpha.md").unwrap();

    assert!(body.contains("Hello body."), "got: {body}");
    assert!(!body.contains("title:"), "front matter leaked: {body}");
}

#[test]
fn create_then_set_body_writes_file() {
    let dir = TempDir::new().unwrap();
    let mut m = manager_with(&dir);

    let path = cli::create(&mut m, "My Note", "").unwrap();
    let idx = m.notes().len() - 1; // create_note pushes the new note last
    cli::set_body(&m, idx, "Fresh content.").unwrap();

    let on_disk = fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("Fresh content."), "got: {on_disk}");
    assert!(on_disk.contains("title:"), "front matter missing: {on_disk}");
}

#[test]
fn remove_deletes_file() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "alpha.md", "Alpha", &[], "b");
    let mut m = manager_with(&dir);
    let idx = cli::resolve_note(&m, "alpha.md").unwrap();

    cli::remove(&mut m, idx).unwrap();

    assert!(!dir.path().join("alpha.md").exists());
}

#[test]
fn link_defaults_to_note_filename_and_points_at_it() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("perso")).unwrap();
    write_md(&dir.path().join("perso"), "mabit.md", "Mabit", &[], "Mon contenu.");
    let m = manager_with(&dir);
    let idx = cli::resolve_note(&m, "perso/mabit.md").unwrap();

    let dest = TempDir::new().unwrap();
    let link_path = cli::link(&m, idx, dest.path(), None).unwrap();

    assert_eq!(link_path, dest.path().join("mabit.md"));
    assert!(fs::symlink_metadata(&link_path).unwrap().file_type().is_symlink());
    // Following the link reaches the note's real body.
    let through = fs::read_to_string(&link_path).unwrap();
    assert!(through.contains("Mon contenu."), "got: {through}");
    // The stored target is the note's canonical absolute path.
    let target = fs::read_link(&link_path).unwrap();
    assert_eq!(target, fs::canonicalize(&m.notes()[idx].path).unwrap());
}

#[test]
fn link_uses_custom_name_when_given() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "mabit.md", "Mabit", &[], "b");
    let m = manager_with(&dir);
    let idx = cli::resolve_note(&m, "mabit.md").unwrap();

    let dest = TempDir::new().unwrap();
    let link_path = cli::link(&m, idx, dest.path(), Some("alias.md")).unwrap();

    assert_eq!(link_path, dest.path().join("alias.md"));
    assert!(fs::symlink_metadata(&link_path).unwrap().file_type().is_symlink());
}

#[test]
fn link_fails_when_destination_already_exists() {
    let dir = TempDir::new().unwrap();
    write_md(dir.path(), "mabit.md", "Mabit", &[], "b");
    let m = manager_with(&dir);
    let idx = cli::resolve_note(&m, "mabit.md").unwrap();

    let dest = TempDir::new().unwrap();
    fs::write(dest.path().join("mabit.md"), "occupied").unwrap();

    let err = cli::link(&m, idx, dest.path(), None).unwrap_err();
    assert!(err.to_string().contains("already exists"), "got: {err}");
    // The existing file is left untouched.
    assert_eq!(fs::read_to_string(dest.path().join("mabit.md")).unwrap(), "occupied");
}
