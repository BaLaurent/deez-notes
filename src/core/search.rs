use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Fuzzy-search notes by title (and optionally content), returning indices
/// sorted by match score descending (best matches first).
///
/// An empty query returns all indices in original order.
pub fn fuzzy_search(
    query: &str,
    notes: &[crate::core::note::Note],
    search_content: bool,
) -> Vec<usize> {
    if query.is_empty() {
        return (0..notes.len()).collect();
    }

    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();

    let mut scored: Vec<(usize, u32)> = notes
        .iter()
        .enumerate()
        .filter_map(|(idx, note)| {
            let title_score = pattern.score(
                Utf32Str::new(&note.title, &mut buf),
                &mut matcher,
            );

            let content_score = if search_content {
                note.content.as_deref().and_then(|text| {
                    pattern.score(Utf32Str::new(text, &mut buf), &mut matcher)
                })
            } else {
                None
            };

            let best = match (title_score, content_score) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

            best.map(|score| (idx, score))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(idx, _)| idx).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::note::Note;
    use chrono::Local;
    use std::path::PathBuf;

    fn make_note(title: &str, content: Option<&str>) -> Note {
        let now = Local::now();
        Note {
            path: PathBuf::from(format!("/tmp/{}.md", title)),
            title: title.to_string(),
            content: content.map(String::from),
            tags: Vec::new(),
            created: now,
            modified: now,
        }
    }

    #[test]
    fn empty_query_returns_all_indices() {
        let notes = vec![
            make_note("Alpha", None),
            make_note("Beta", None),
            make_note("Gamma", None),
        ];
        let result = fuzzy_search("", &notes, false);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn matching_title() {
        let notes = vec![
            make_note("Rust programming", None),
            make_note("Python scripting", None),
            make_note("Rustacean guide", None),
        ];
        let result = fuzzy_search("rust", &notes, false);
        assert!(!result.is_empty());
        // Both notes with "Rust" in title should match
        assert!(result.contains(&0));
        assert!(result.contains(&2));
        // "Python scripting" should not match "rust"
        assert!(!result.contains(&1));
    }

    #[test]
    fn no_matches() {
        let notes = vec![
            make_note("Alpha", None),
            make_note("Beta", None),
        ];
        let result = fuzzy_search("zzzzz", &notes, false);
        assert!(result.is_empty());
    }

    #[test]
    fn score_ordering() {
        let notes = vec![
            make_note("abcdef", None),      // weak match for "ace"
            make_note("ace of spades", None), // strong match for "ace"
        ];
        let result = fuzzy_search("ace", &notes, false);
        assert!(!result.is_empty());
        // "ace of spades" has a contiguous "ace" so it should rank first
        assert_eq!(result[0], 1);
    }

    #[test]
    fn content_search_finds_match_in_body() {
        let notes = vec![
            make_note("Unrelated title", Some("The quick brown fox")),
            make_note("Another title", None),
        ];
        // Without content search, "fox" won't match titles
        let without = fuzzy_search("fox", &notes, false);
        assert!(!without.contains(&0));

        // With content search, "fox" matches the body of note 0
        let with = fuzzy_search("fox", &notes, true);
        assert!(with.contains(&0));
    }

    #[test]
    fn special_regex_characters_in_query_no_crash() {
        let notes = vec![
            make_note("file.txt", None),
            make_note("test*pattern", None),
            make_note("[bracketed]", None),
        ];
        // These contain regex special chars; fuzzy search should not crash
        let result = fuzzy_search(".", &notes, false);
        // "." should match "file.txt" at minimum
        assert!(!result.is_empty());

        let result = fuzzy_search("*", &notes, false);
        // Should not panic
        let _ = result;

        let result = fuzzy_search("[", &notes, false);
        let _ = result;

        let result = fuzzy_search("(", &notes, false);
        let _ = result;

        let result = fuzzy_search("\\", &notes, false);
        let _ = result;
    }

    #[test]
    fn very_long_search_query_no_crash() {
        let notes = vec![
            make_note("Short title", None),
            make_note("Another note", None),
        ];
        let long_query = "x".repeat(1000);
        let result = fuzzy_search(&long_query, &notes, false);
        // Should return empty (no match), not crash
        assert!(result.is_empty());
    }

    #[test]
    fn unicode_search_matches() {
        let notes = vec![
            make_note("caf\u{00E9} latt\u{00E9}", None),
            make_note("\u{00FC}ber cool", None),
            make_note("plain ascii", None),
        ];
        let result = fuzzy_search("caf\u{00E9}", &notes, false);
        assert!(result.contains(&0), "should match unicode title");
    }

    #[test]
    fn case_insensitivity() {
        let notes = vec![
            make_note("hello world", None),
            make_note("HELLO WORLD", None),
            make_note("Hello World", None),
        ];
        let result = fuzzy_search("HELLO", &notes, false);
        // All three should match since fuzzy search uses CaseMatching::Ignore
        assert_eq!(result.len(), 3, "all case variants should match");
    }

    #[test]
    fn search_on_notes_with_no_content_works_on_titles() {
        let notes = vec![
            make_note("Important meeting notes", None),
            make_note("Shopping list", None),
            make_note("Project ideas", None),
        ];
        // content is None for all; title-only search should still work
        let result = fuzzy_search("meeting", &notes, true);
        assert!(result.contains(&0));
        assert!(!result.contains(&1));
        assert!(!result.contains(&2));
    }
}
