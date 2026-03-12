use chrono::{DateTime, Local, Utc};
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};

/// Parsed front matter from a Markdown note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontMatter {
    #[serde(default)]
    pub title: String,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub created: DateTime<Local>,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub modified: DateTime<Local>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Serialize DateTime<Local> as an ISO 8601 string without timezone offset
/// (e.g. "2025-03-11T14:30:00") to keep front matter clean.
fn serialize_datetime<S>(dt: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
    serializer.serialize_str(&s)
}

/// Deserialize a datetime string into DateTime<Local>.
/// Accepts ISO 8601 with or without timezone offset.
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_datetime_str(&s).ok_or_else(|| serde::de::Error::custom("invalid datetime format"))
}

/// Try multiple datetime formats to be tolerant of input variations.
fn parse_datetime_str(s: &str) -> Option<DateTime<Local>> {
    // Try RFC 3339 / ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    // Try ISO 8601 without timezone (treat as local)
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return naive.and_local_timezone(Local).single();
    }
    // Try with fractional seconds
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return naive.and_local_timezone(Local).single();
    }
    // Try UTC format and convert
    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
        return Some(dt.with_timezone(&Local));
    }
    None
}

/// Parse front matter from markdown content.
///
/// Uses `gray_matter` to split the document, then `serde_yml` to parse the YAML.
/// Returns `(Some(FrontMatter), body)` on success, or `(None, full_content)` if
/// front matter is absent or malformed.
pub fn parse_front_matter(content: &str) -> (Option<FrontMatter>, String) {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);

    // If gray_matter found no raw matter block, return None + full content
    if parsed.matter.is_empty() {
        return (None, content.to_string());
    }

    // Parse the raw YAML via serde_yml
    match serde_yml::from_str::<FrontMatter>(&parsed.matter) {
        Ok(fm) => (Some(fm), parsed.content),
        Err(_) => (None, content.to_string()),
    }
}

/// Serialize front matter and body into a complete markdown document.
pub fn write_front_matter(fm: &FrontMatter, body: &str) -> String {
    let yaml = serde_yml::to_string(fm).unwrap_or_default();
    // serde_yml adds a trailing newline; trim it so the output is tidy
    let yaml = yaml.trim_end();
    format!("---\n{yaml}\n---\n\n{body}")
}

/// Create a new FrontMatter with the given title and current timestamps.
pub fn new_front_matter(title: &str) -> FrontMatter {
    let now = Local::now();
    FrontMatter {
        title: title.to_string(),
        created: now,
        modified: now,
        tags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_front_matter() {
        let input = r#"---
title: "Ma note de test"
created: 2025-03-11T14:30:00
modified: 2025-03-11T15:45:00
tags: [rust, projet, important]
---

# Contenu ici

Du texte markdown."#;

        let (fm, body) = parse_front_matter(input);
        let fm = fm.expect("should parse valid front matter");

        assert_eq!(fm.title, "Ma note de test");
        assert_eq!(fm.tags, vec!["rust", "projet", "important"]);
        assert_eq!(fm.created.format("%Y-%m-%d").to_string(), "2025-03-11");
        assert_eq!(fm.modified.format("%H:%M:%S").to_string(), "15:45:00");
        assert!(body.contains("# Contenu ici"));
        assert!(body.contains("Du texte markdown."));
    }

    #[test]
    fn parse_missing_front_matter() {
        let input = "# Just a markdown file\n\nNo front matter here.";

        let (fm, body) = parse_front_matter(input);

        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn parse_corrupted_front_matter() {
        let input = "---\ntitle: [invalid yaml: {{{\n---\n\nSome body.";

        let (fm, body) = parse_front_matter(input);

        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn parse_empty_input() {
        let (fm, body) = parse_front_matter("");
        assert!(fm.is_none());
        assert_eq!(body, "");
    }

    #[test]
    fn parse_front_matter_missing_optional_fields() {
        // title and tags have serde defaults, so missing them should still parse
        let input = "---\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\n---\nBody";

        let (fm, body) = parse_front_matter(input);
        let fm = fm.expect("should parse with default fields");

        assert_eq!(fm.title, "");
        assert!(fm.tags.is_empty());
        assert_eq!(body, "Body");
    }

    #[test]
    fn write_front_matter_roundtrip() {
        let fm = FrontMatter {
            title: "Test Round Trip".to_string(),
            created: Local::now(),
            modified: Local::now(),
            tags: vec!["a".to_string(), "b".to_string()],
        };

        let output = write_front_matter(&fm, "Hello world");

        // Parse it back
        let (parsed, body) = parse_front_matter(&output);
        let parsed = parsed.expect("round-trip should produce valid front matter");

        assert_eq!(parsed.title, "Test Round Trip");
        assert_eq!(parsed.tags, vec!["a", "b"]);
        assert_eq!(body, "Hello world");
    }

    #[test]
    fn new_front_matter_creates_valid_fm() {
        let fm = new_front_matter("Nouvelle note");

        assert_eq!(fm.title, "Nouvelle note");
        assert!(fm.tags.is_empty());

        // created and modified should be very close to now
        let now = Local::now();
        let diff = now.signed_duration_since(fm.created);
        assert!(
            diff.num_seconds().abs() < 2,
            "created should be close to now"
        );
        let diff = now.signed_duration_since(fm.modified);
        assert!(
            diff.num_seconds().abs() < 2,
            "modified should be close to now"
        );

        // Verify it serializes correctly
        let output = write_front_matter(&fm, "");
        let (parsed, _) = parse_front_matter(&output);
        assert!(parsed.is_some(), "new_front_matter output should be parseable");
    }

    #[test]
    fn write_front_matter_format() {
        let fm = new_front_matter("Format Test");
        let output = write_front_matter(&fm, "Content here");

        assert!(output.starts_with("---\n"));
        assert!(output.contains("\n---\n\n"));
        assert!(output.ends_with("Content here"));
    }

    #[test]
    fn parse_front_matter_with_rfc3339_timezone() {
        let input = "---\ntitle: TZ test\ncreated: 2025-06-15T10:00:00+02:00\nmodified: 2025-06-15T10:00:00+02:00\ntags: []\n---\nBody";

        let (fm, _) = parse_front_matter(input);
        let fm = fm.expect("should parse RFC 3339 datetime");
        assert_eq!(fm.title, "TZ test");
    }

    #[test]
    fn parse_partial_corruption_missing_created() {
        // `created` has no serde default, so missing it should fail to parse
        // and return None (graceful fallback)
        let input = "---\ntitle: Missing Created\nmodified: 2025-01-01T00:00:00\ntags: []\n---\nBody";

        let (fm, body) = parse_front_matter(input);
        // serde_yml cannot fill in a missing DateTime<Local>, so parse fails
        assert!(fm.is_none(), "missing required `created` should fail parse");
        assert_eq!(body, input);
    }

    #[test]
    fn parse_partial_corruption_invalid_date_format() {
        let input = "---\ntitle: Bad Date\ncreated: not-a-date\nmodified: 2025-01-01T00:00:00\ntags: []\n---\nBody";

        let (fm, body) = parse_front_matter(input);
        assert!(fm.is_none(), "invalid date format should fail gracefully");
        assert_eq!(body, input);
    }

    #[test]
    fn parse_mixed_valid_title_invalid_tags() {
        // tags is expected to be a list; a plain string should fail parse
        let input = "---\ntitle: Good Title\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: not-a-list\n---\nBody";

        let (fm, _body) = parse_front_matter(input);
        // serde_yml tries to deserialize a string into Vec<String> which may
        // succeed (single element) or fail depending on the YAML parser behavior.
        // Either way, no crash.
        if let Some(fm) = fm {
            // If it did parse, title should be intact
            assert_eq!(fm.title, "Good Title");
        }
    }

    #[test]
    fn parse_front_matter_extra_unknown_fields() {
        let input = r#"---
title: "Known Title"
created: 2025-01-01T00:00:00
modified: 2025-01-01T00:00:00
tags: [a]
author: "Unknown Author"
priority: 5
custom_field: "whatever"
---

Body here."#;

        let (fm, body) = parse_front_matter(input);
        // serde_yml with default settings should either ignore unknown fields
        // or fail. Our FrontMatter does not use deny_unknown_fields, so it
        // should succeed.
        let fm = fm.expect("unknown fields should be ignored");
        assert_eq!(fm.title, "Known Title");
        assert_eq!(fm.tags, vec!["a"]);
        assert!(body.contains("Body here."));
    }

    #[test]
    fn parse_front_matter_very_long_title() {
        let long_title = "A".repeat(1200);
        let input = format!(
            "---\ntitle: \"{}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\nBody",
            long_title
        );

        let (fm, _body) = parse_front_matter(&input);
        let fm = fm.expect("very long title should parse");
        assert_eq!(fm.title.len(), 1200);
    }

    #[test]
    fn parse_front_matter_unicode_emoji_in_title_and_tags() {
        let input = "---\ntitle: \"Mes notes \u{1F4DD} caf\u{00E9}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: [\"\u{1F680}rocket\", \"\u{00E9}t\u{00E9}\"]\n---\nBody";

        let (fm, _body) = parse_front_matter(input);
        let fm = fm.expect("unicode/emoji should parse correctly");
        assert!(fm.title.contains('\u{1F4DD}'));
        assert!(fm.title.contains("caf\u{00E9}"));
        assert_eq!(fm.tags.len(), 2);
        assert!(fm.tags[0].contains('\u{1F680}'));
        assert!(fm.tags[1].contains('\u{00E9}'));
    }

    #[test]
    fn parse_front_matter_empty_tags_list() {
        let input = "---\ntitle: Empty Tags\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: []\n---\nBody";

        let (fm, _body) = parse_front_matter(input);
        let fm = fm.expect("empty tags should parse");
        assert!(fm.tags.is_empty());
    }

    #[test]
    fn write_front_matter_with_empty_body() {
        let fm = new_front_matter("Empty Body Note");
        let output = write_front_matter(&fm, "");

        assert!(output.starts_with("---\n"));
        assert!(output.contains("Empty Body Note"));
        assert!(output.contains("\n---\n\n"));

        // Verify round-trip: parse it back
        let (parsed, body) = parse_front_matter(&output);
        let parsed = parsed.expect("empty body note should round-trip");
        assert_eq!(parsed.title, "Empty Body Note");
        assert_eq!(body, "");
    }
}
