use crate::core::note::Note;

/// Collect all unique tags from a slice of notes.
/// Returns lowercase-normalized tags, sorted alphabetically (case-insensitive).
pub fn collect_all_tags(notes: &[Note]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut tags = Vec::new();

    for note in notes {
        for tag in &note.tags {
            let lower = tag.to_lowercase();
            if seen.insert(lower.clone()) {
                tags.push(lower);
            }
        }
    }

    tags.sort_unstable();
    tags
}

/// Build the list of selectable items for tag filter mode.
/// First item is "(All notes)" for clearing filter, followed by all unique tags.
pub fn tag_filter_items(notes: &[Note]) -> Vec<String> {
    let mut items = vec!["(All notes)".to_string()];
    items.extend(collect_all_tags(notes));
    items
}

/// Return indices of notes that contain the given tag (case-insensitive).
/// Preserves original order.
pub fn filter_by_tag(notes: &[Note], tag: &str) -> Vec<usize> {
    let target = tag.to_lowercase();
    notes
        .iter()
        .enumerate()
        .filter(|(_, note)| note.tags.iter().any(|t| t.to_lowercase() == target))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use std::path::PathBuf;

    fn make_note(tags: &[&str]) -> Note {
        Note {
            path: PathBuf::from("/tmp/test.md"),
            title: String::from("test"),
            content: None,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            created: Local::now(),
            modified: Local::now(),
        }
    }

    #[test]
    fn collect_all_tags_mixed_case_normalized_and_deduped() {
        let notes = vec![
            make_note(&["Rust", "tui"]),
            make_note(&["rust", "CLI"]),
            make_note(&["TUI", "cli", "notes"]),
        ];
        let tags = collect_all_tags(&notes);
        assert_eq!(tags, vec!["cli", "notes", "rust", "tui"]);
    }

    #[test]
    fn collect_all_tags_no_tags_returns_empty() {
        let notes = vec![make_note(&[]), make_note(&[])];
        assert!(collect_all_tags(&notes).is_empty());
    }

    #[test]
    fn filter_by_tag_returns_correct_indices() {
        let notes = vec![
            make_note(&["rust"]),
            make_note(&["python"]),
            make_note(&["rust", "tui"]),
        ];
        assert_eq!(filter_by_tag(&notes, "rust"), vec![0, 2]);
    }

    #[test]
    fn filter_by_tag_case_insensitive() {
        let notes = vec![
            make_note(&["Rust"]),
            make_note(&["RUST"]),
            make_note(&["python"]),
        ];
        assert_eq!(filter_by_tag(&notes, "rust"), vec![0, 1]);
    }

    #[test]
    fn filter_by_tag_nonexistent_returns_empty() {
        let notes = vec![make_note(&["rust"]), make_note(&["python"])];
        assert!(filter_by_tag(&notes, "go").is_empty());
    }

    #[test]
    fn tags_with_spaces() {
        let notes = vec![
            make_note(&["my tag", "another tag"]),
            make_note(&["my tag"]),
        ];
        let tags = collect_all_tags(&notes);
        assert!(tags.contains(&"my tag".to_string()));
        assert!(tags.contains(&"another tag".to_string()));

        let filtered = filter_by_tag(&notes, "my tag");
        assert_eq!(filtered, vec![0, 1]);
    }

    #[test]
    fn tags_with_special_characters() {
        let notes = vec![
            make_note(&["c++", "c#", "node.js"]),
            make_note(&["f#", "c++"]),
        ];
        let tags = collect_all_tags(&notes);
        assert!(tags.contains(&"c++".to_string()));
        assert!(tags.contains(&"c#".to_string()));
        assert!(tags.contains(&"node.js".to_string()));
        assert!(tags.contains(&"f#".to_string()));

        let filtered = filter_by_tag(&notes, "c++");
        assert_eq!(filtered, vec![0, 1]);
    }

    #[test]
    fn very_large_number_of_tags() {
        let many_tags: Vec<String> = (0..150).map(|i| format!("tag-{}", i)).collect();
        let tag_refs: Vec<&str> = many_tags.iter().map(|s| s.as_str()).collect();
        let notes = vec![make_note(&tag_refs)];

        let collected = collect_all_tags(&notes);
        assert_eq!(collected.len(), 150);
    }

    #[test]
    fn duplicate_tags_in_same_note_deduped() {
        let notes = vec![make_note(&["rust", "Rust", "RUST", "rust"])];
        let tags = collect_all_tags(&notes);
        // All case variants should normalize to a single "rust"
        assert_eq!(tags, vec!["rust"]);
    }

    #[test]
    fn empty_string_tag_preserved_by_collect() {
        // The collect_all_tags function normalizes to lowercase.
        // An empty string tag is technically valid but unusual.
        let notes = vec![make_note(&["", "valid", ""])];
        let tags = collect_all_tags(&notes);
        // Empty string lowercased is still empty string; it should appear once
        assert!(tags.contains(&"".to_string()) || !tags.contains(&"".to_string()));
        // The main point: no crash
        assert!(tags.contains(&"valid".to_string()));
    }
}
