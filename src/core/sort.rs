use crate::app::SortMode;
use crate::core::note::Note;

/// Sort a vector of indices in-place based on the corresponding notes' fields.
///
/// `indices` contains positions into `notes`. After sorting, the indices are
/// reordered so that iterating them yields notes in the requested order.
///
/// - `ascending = true`:  oldest first (dates) or A-Z (title)
/// - `ascending = false`: newest first (dates) or Z-A (title)
pub fn sort_notes(indices: &mut [usize], notes: &[Note], mode: SortMode, ascending: bool) {
    indices.sort_by(|&a, &b| {
        let cmp = match mode {
            SortMode::ByModified => notes[a].modified.cmp(&notes[b].modified),
            SortMode::ByCreated => notes[a].created.cmp(&notes[b].created),
            SortMode::ByTitle => {
                let a_lower = notes[a].title.to_lowercase();
                let b_lower = notes[b].title.to_lowercase();
                a_lower.cmp(&b_lower)
            }
        };
        if ascending { cmp } else { cmp.reverse() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use std::path::PathBuf;

    fn make_note(title: &str, created_hour: u32, modified_hour: u32) -> Note {
        let created = Local
            .with_ymd_and_hms(2025, 1, 1, created_hour, 0, 0)
            .unwrap();
        let modified = Local
            .with_ymd_and_hms(2025, 1, 1, modified_hour, 0, 0)
            .unwrap();
        Note {
            path: PathBuf::from(format!("/tmp/{}.md", title)),
            title: title.to_string(),
            content: None,
            tags: Vec::new(),
            created,
            modified,
        }
    }

    #[test]
    fn sort_by_title_ascending() {
        let notes = vec![
            make_note("Cherry", 1, 1),
            make_note("apple", 2, 2),
            make_note("Banana", 3, 3),
        ];
        let mut indices = vec![0, 1, 2];
        sort_notes(&mut indices, &notes, SortMode::ByTitle, true);
        let titles: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
        assert_eq!(titles, vec!["apple", "Banana", "Cherry"]);
    }

    #[test]
    fn sort_by_title_descending() {
        let notes = vec![
            make_note("Cherry", 1, 1),
            make_note("apple", 2, 2),
            make_note("Banana", 3, 3),
        ];
        let mut indices = vec![0, 1, 2];
        sort_notes(&mut indices, &notes, SortMode::ByTitle, false);
        let titles: Vec<&str> = indices.iter().map(|&i| notes[i].title.as_str()).collect();
        assert_eq!(titles, vec!["Cherry", "Banana", "apple"]);
    }

    #[test]
    fn sort_by_modified_date() {
        let notes = vec![
            make_note("A", 1, 10), // modified at 10:00
            make_note("B", 2, 5),  // modified at 05:00
            make_note("C", 3, 8),  // modified at 08:00
        ];
        let mut indices = vec![0, 1, 2];

        sort_notes(&mut indices, &notes, SortMode::ByModified, true);
        assert_eq!(indices, vec![1, 2, 0]); // 05:00, 08:00, 10:00

        sort_notes(&mut indices, &notes, SortMode::ByModified, false);
        assert_eq!(indices, vec![0, 2, 1]); // 10:00, 08:00, 05:00
    }

    #[test]
    fn sort_by_created_date() {
        let notes = vec![
            make_note("A", 3, 1),  // created at 03:00
            make_note("B", 1, 2),  // created at 01:00
            make_note("C", 7, 3),  // created at 07:00
        ];
        let mut indices = vec![0, 1, 2];

        sort_notes(&mut indices, &notes, SortMode::ByCreated, true);
        assert_eq!(indices, vec![1, 0, 2]); // 01:00, 03:00, 07:00

        sort_notes(&mut indices, &notes, SortMode::ByCreated, false);
        assert_eq!(indices, vec![2, 0, 1]); // 07:00, 03:00, 01:00
    }

    #[test]
    fn empty_indices_no_crash() {
        let notes = vec![make_note("A", 1, 1)];
        let mut indices: Vec<usize> = Vec::new();
        sort_notes(&mut indices, &notes, SortMode::ByTitle, true);
        assert!(indices.is_empty());
    }

    #[test]
    fn sort_stability_same_title() {
        // Notes with same title should maintain relative order (sort_by is stable)
        let notes = vec![
            make_note("Same", 1, 1),
            make_note("Same", 2, 2),
            make_note("Same", 3, 3),
        ];
        let mut indices = vec![0, 1, 2];
        sort_notes(&mut indices, &notes, SortMode::ByTitle, true);
        // Since all titles are identical, stable sort preserves original order
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn sort_single_note() {
        let notes = vec![make_note("Only One", 5, 5)];
        let mut indices = vec![0];
        sort_notes(&mut indices, &notes, SortMode::ByTitle, true);
        assert_eq!(indices, vec![0]);

        sort_notes(&mut indices, &notes, SortMode::ByModified, false);
        assert_eq!(indices, vec![0]);

        sort_notes(&mut indices, &notes, SortMode::ByCreated, true);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn sort_with_identical_dates_no_crash() {
        let notes = vec![
            make_note("B", 5, 5),
            make_note("A", 5, 5),
            make_note("C", 5, 5),
        ];
        let mut indices = vec![0, 1, 2];

        // Sort by modified with all identical dates - should not crash
        sort_notes(&mut indices, &notes, SortMode::ByModified, true);
        // All dates identical, stable sort preserves order
        assert_eq!(indices, vec![0, 1, 2]);

        // Sort by created with all identical dates - should not crash
        sort_notes(&mut indices, &notes, SortMode::ByCreated, false);
        assert_eq!(indices, vec![0, 1, 2]);
    }
}
