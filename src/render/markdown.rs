use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::config::theme::Theme;

/// Number of spaces a tab character expands to in the rendered output.
const TAB_WIDTH: usize = 4;

/// Convert Markdown content into styled ratatui `Line` objects for TUI display.
pub fn render_markdown(content: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    if content.is_empty() {
        return Vec::new();
    }

    // Pre-process content to preserve visible indentation.
    // The Markdown parser strips leading spaces/tabs as indentation syntax,
    // so we replace them with non-breaking spaces (U+00A0) which the parser
    // treats as regular text content.
    let content = preserve_leading_whitespace(content);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(&content, options);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // State tracking
    let mut heading_level: Option<HeadingLevel> = None;
    let mut in_code_block = false;
    let mut in_blockquote = false;
    let mut in_emphasis = false;
    let mut in_strong = false;
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut at_item_start = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level);
            }
            Event::End(TagEnd::Heading(level)) => {
                let style = heading_style(level, theme);
                let finished: Vec<Span<'static>> = current_spans
                    .drain(..)
                    .map(|s| Span::styled(s.content.to_string(), style))
                    .collect();
                lines.push(Line::from(finished));
                // Blank line after H1 and H2
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    lines.push(Line::from(""));
                }
                heading_level = None;
            }

            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_line(&mut current_spans, &mut lines);
                // Blank line after paragraph
                lines.push(Line::from(""));
            }

            Event::Start(Tag::BlockQuote(_)) => {
                in_blockquote = true;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_blockquote = false;
            }

            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
            }

            Event::Start(Tag::List(first_number)) => {
                let kind = match first_number {
                    Some(start) => ListKind::Ordered(start),
                    None => ListKind::Unordered,
                };
                list_stack.push(kind);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                // Blank line after top-level list
                if list_stack.is_empty() {
                    lines.push(Line::from(""));
                }
            }

            Event::Start(Tag::Item) => {
                at_item_start = true;
            }
            Event::End(TagEnd::Item) => {
                flush_line(&mut current_spans, &mut lines);
                // Increment ordered list counter
                if let Some(ListKind::Ordered(start)) = list_stack.last_mut() {
                    *start += 1;
                }
            }

            Event::Start(Tag::Emphasis) => {
                in_emphasis = true;
            }
            Event::End(TagEnd::Emphasis) => {
                in_emphasis = false;
            }

            Event::Start(Tag::Strong) => {
                in_strong = true;
            }
            Event::End(TagEnd::Strong) => {
                in_strong = false;
            }

            Event::Start(Tag::Link { .. }) | Event::End(TagEnd::Link) => {
                // Render link text inline (no special decoration beyond what's inside)
            }

            Event::Text(text) => {
                let text_str = text.to_string();

                if in_code_block {
                    // Code blocks: split by lines, each prefixed with "  | "
                    for line_text in text_str.split('\n') {
                        if line_text.is_empty() && current_spans.is_empty() {
                            continue;
                        }
                        let mut code_spans = vec![Span::styled(
                            "  \u{2502} ".to_string(),
                            Style::default().fg(theme.fg_secondary),
                        )];
                        code_spans.push(Span::styled(
                            line_text.to_string(),
                            Style::default().fg(theme.success),
                        ));
                        lines.push(Line::from(code_spans));
                    }
                    continue;
                }

                if in_blockquote {
                    // Prepend blockquote prefix if starting a new line
                    if current_spans.is_empty() {
                        current_spans.push(Span::styled(
                            "\u{2502} ".to_string(),
                            Style::default().fg(theme.fg_secondary),
                        ));
                    }
                    current_spans.push(Span::styled(
                        text_str,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                    continue;
                }

                // List item prefix — indent grows with nesting depth
                if at_item_start {
                    if let Some(kind) = list_stack.last() {
                        let indent = "  ".repeat(list_stack.len());
                        let prefix = match kind {
                            ListKind::Unordered => format!("{}\u{2022} ", indent),
                            ListKind::Ordered(n) => format!("{}{}. ", indent, n),
                        };
                        current_spans.push(Span::raw(prefix));
                    }
                    at_item_start = false;
                }

                // Apply inline styles
                let style = inline_style(heading_level, in_strong, in_emphasis, theme);
                current_spans.push(Span::styled(text_str, style));
            }

            Event::Code(code) => {
                // Inline code
                let text_str = code.to_string();
                current_spans.push(Span::styled(
                    text_str,
                    Style::default().fg(theme.highlight),
                ));
            }

            Event::SoftBreak => {
                if in_code_block {
                    // Handled within Text event line splitting
                } else {
                    // Treat soft breaks as hard breaks so single newlines
                    // in the source render as visible line breaks in the TUI.
                    flush_line(&mut current_spans, &mut lines);
                }
            }

            Event::HardBreak => {
                flush_line(&mut current_spans, &mut lines);
            }

            Event::Rule => {
                flush_line(&mut current_spans, &mut lines);
                let rule_width = if width > 0 { width as usize } else { 40 };
                let rule_str: String = "\u{2500}".repeat(rule_width);
                lines.push(Line::from(Span::styled(
                    rule_str,
                    Style::default().fg(theme.fg_secondary),
                )));
                lines.push(Line::from(""));
            }

            // Ignore events we don't handle
            _ => {}
        }
    }

    // Flush any remaining spans
    flush_line(&mut current_spans, &mut lines);

    lines
}

#[derive(Clone, Debug)]
enum ListKind {
    Ordered(u64),
    Unordered,
}

fn heading_style(level: HeadingLevel, theme: &Theme) -> Style {
    match level {
        HeadingLevel::H1 => Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
        HeadingLevel::H2 => Style::default()
            .fg(theme.highlight)
            .add_modifier(Modifier::BOLD),
        HeadingLevel::H3 => Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().add_modifier(Modifier::BOLD),
    }
}

fn inline_style(heading: Option<HeadingLevel>, strong: bool, emphasis: bool, theme: &Theme) -> Style {
    if let Some(level) = heading {
        return heading_style(level, theme);
    }
    let mut style = Style::default();
    if strong {
        style = style.add_modifier(Modifier::BOLD);
    }
    if emphasis {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

fn flush_line(spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}

/// Replace leading whitespace on each line with non-breaking spaces (U+00A0).
/// This prevents the Markdown parser from consuming indentation as syntax
/// while keeping it visible in the rendered output.
fn preserve_leading_whitespace(content: &str) -> String {
    let nbsp = '\u{00a0}';
    let tab_spaces = TAB_WIDTH;
    let mut result = String::with_capacity(content.len());

    for line in content.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }

        let mut chars = line.chars().peekable();
        // Convert leading whitespace to non-breaking spaces
        while let Some(&ch) = chars.peek() {
            match ch {
                ' ' => {
                    result.push(nbsp);
                    chars.next();
                }
                '\t' => {
                    for _ in 0..tab_spaces {
                        result.push(nbsp);
                    }
                    chars.next();
                }
                _ => break,
            }
        }
        // Append the rest of the line unchanged
        result.extend(chars);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::theme::Theme;

    fn test_theme() -> Theme {
        Theme::terminal(&[])
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        let result = render_markdown("", 80, &test_theme());
        assert!(result.is_empty());
    }

    #[test]
    fn heading_h1_is_bold_accent() {
        let theme = test_theme();
        let result = render_markdown("# Hello", 80, &theme);
        assert!(!result.is_empty());
        let first = &result[0];
        let span = &first.spans[0];
        let style = span.style;
        assert_eq!(style.fg, Some(theme.accent));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn heading_h2_is_bold_highlight() {
        let theme = test_theme();
        let result = render_markdown("## World", 80, &theme);
        assert!(!result.is_empty());
        let first = &result[0];
        let span = &first.spans[0];
        let style = span.style;
        assert_eq!(style.fg, Some(theme.highlight));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn heading_h3_is_bold_success() {
        let theme = test_theme();
        let result = render_markdown("### Section", 80, &theme);
        assert!(!result.is_empty());
        let first = &result[0];
        let span = &first.spans[0];
        let style = span.style;
        assert_eq!(style.fg, Some(theme.success));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn h1_has_blank_line_after() {
        let result = render_markdown("# Title", 80, &test_theme());
        // Should be: heading line, blank line
        assert!(result.len() >= 2);
        assert!(result[1].spans.is_empty() || result[1].to_string().is_empty());
    }

    #[test]
    fn bold_text_has_bold_modifier() {
        let result = render_markdown("some **bold** text", 80, &test_theme());
        assert!(!result.is_empty());
        let line = &result[0];
        // Find the span containing "bold"
        let bold_span = line.spans.iter().find(|s| s.content.contains("bold"));
        assert!(bold_span.is_some());
        assert!(bold_span
            .expect("bold span missing")
            .style
            .add_modifier
            .contains(Modifier::BOLD));
    }

    #[test]
    fn italic_text_has_italic_modifier() {
        let result = render_markdown("some *italic* text", 80, &test_theme());
        assert!(!result.is_empty());
        let line = &result[0];
        let italic_span = line.spans.iter().find(|s| s.content.contains("italic"));
        assert!(italic_span.is_some());
        assert!(italic_span
            .expect("italic span missing")
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
    }

    #[test]
    fn inline_code_has_highlight_color() {
        let theme = test_theme();
        let result = render_markdown("use `code` here", 80, &theme);
        assert!(!result.is_empty());
        let line = &result[0];
        let code_span = line.spans.iter().find(|s| s.content.contains("code"));
        assert!(code_span.is_some());
        assert_eq!(
            code_span.expect("code span missing").style.fg,
            Some(theme.highlight)
        );
    }

    #[test]
    fn code_block_lines_are_prefixed() {
        let theme = test_theme();
        let input = "```\nlet x = 1;\n```";
        let result = render_markdown(input, 80, &theme);
        // Find a line that contains the code
        let code_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("let x")));
        assert!(code_line.is_some(), "code block line not found");
        let code_line = code_line.expect("code line missing");
        // First span should be the prefix
        assert!(code_line.spans[0].content.contains('\u{2502}'));
        assert_eq!(code_line.spans[0].style.fg, Some(theme.fg_secondary));
        // Second span is the content
        assert_eq!(code_line.spans[1].style.fg, Some(theme.success));
    }

    #[test]
    fn unordered_list_has_bullet() {
        let result = render_markdown("- item one\n- item two", 80, &test_theme());
        let has_bullet = result
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains('\u{2022}')));
        assert!(has_bullet, "bullet character not found in list");
    }

    #[test]
    fn ordered_list_has_numbers() {
        let result = render_markdown("1. first\n2. second", 80, &test_theme());
        let has_number = result
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("1.")));
        assert!(has_number, "ordered list number not found");
    }

    #[test]
    fn horizontal_rule_spans_width() {
        let result = render_markdown("---", 40, &test_theme());
        let rule_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains('\u{2500}')));
        assert!(rule_line.is_some(), "horizontal rule not found");
        let rule_line = rule_line.expect("rule missing");
        let rule_text: String = rule_line.spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(rule_text.chars().count(), 40);
    }

    #[test]
    fn blockquote_has_prefix_and_italic() {
        let result = render_markdown("> quoted text", 80, &test_theme());
        let quote_line = result.iter().find(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("quoted"))
        });
        assert!(quote_line.is_some(), "blockquote line not found");
        let quote_line = quote_line.expect("quote line missing");
        // Should have a prefix span with the vertical bar
        assert!(quote_line.spans[0].content.contains('\u{2502}'));
        // The text span should be italic
        let text_span = quote_line
            .spans
            .iter()
            .find(|s| s.content.contains("quoted"));
        assert!(text_span
            .expect("quoted span missing")
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
    }

    #[test]
    fn paragraphs_separated_by_blank_lines() {
        let result = render_markdown("First paragraph.\n\nSecond paragraph.", 80, &test_theme());
        // There should be blank lines separating paragraphs
        let blank_count = result
            .iter()
            .filter(|l| l.spans.is_empty() || l.to_string().is_empty())
            .count();
        assert!(
            blank_count >= 1,
            "expected blank lines between paragraphs, got {}",
            blank_count
        );
    }

    #[test]
    fn nested_formatting_bold_inside_italic_no_crash() {
        let input = "*italic and **bold inside** italic*";
        let result = render_markdown(input, 80, &test_theme());
        // Should not crash; just verify we get output
        assert!(!result.is_empty());
        // The text content should be present somewhere in the spans
        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(all_text.contains("italic"));
        assert!(all_text.contains("bold inside"));
    }

    #[test]
    fn very_long_lines_handled() {
        let long_line = "x".repeat(10000);
        let result = render_markdown(&long_line, 80, &test_theme());
        assert!(!result.is_empty());
        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert_eq!(all_text.len(), 10000);
    }

    #[test]
    fn empty_heading_no_crash() {
        let result = render_markdown("# ", 80, &test_theme());
        // Should not crash; may produce an empty heading line
        // (pulldown-cmark treats "# " as a heading with empty text)
        let _ = result;
    }

    #[test]
    fn code_block_with_no_language_renders() {
        let theme = test_theme();
        let input = "```\nfn main() {}\n```";
        let result = render_markdown(input, 80, &theme);
        let code_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("fn main")));
        assert!(code_line.is_some(), "code block without language should render");
        let code_line = code_line.unwrap();
        // Should still have the prefix
        assert!(code_line.spans[0].content.contains('\u{2502}'));
        // Code text should use success color
        assert_eq!(code_line.spans[1].style.fg, Some(theme.success));
    }

    #[test]
    fn deeply_nested_lists_render() {
        let input = "- Level 1\n  - Level 2\n    - Level 3\n      - Level 4\n        - Level 5";
        let result = render_markdown(input, 80, &test_theme());
        // Should not crash and should produce some output
        assert!(!result.is_empty());
        // At least the top-level items should be present
        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(all_text.contains("Level 1"));
        assert!(all_text.contains("Level 5"));
    }

    #[test]
    fn soft_break_produces_separate_lines() {
        // A single newline in a paragraph produces a SoftBreak event.
        // We want it rendered as a visible line break, not collapsed to a space.
        let input = "line one\nline two\nline three";
        let result = render_markdown(input, 80, &test_theme());

        let line_texts: Vec<String> = result
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
            .collect();

        assert!(
            line_texts.len() >= 3,
            "expected at least 3 separate lines, got {}: {:?}",
            line_texts.len(),
            line_texts
        );
        assert!(line_texts.iter().any(|l| l.contains("line one")));
        assert!(line_texts.iter().any(|l| l.contains("line two")));
        assert!(line_texts.iter().any(|l| l.contains("line three")));
    }

    #[test]
    fn tabs_are_preserved_as_indentation() {
        // Tabs should be expanded to non-breaking spaces so they appear
        // as visible indentation instead of being swallowed by the parser.
        let input = "Idées :\n\tUne barre de progrès\n\tAjouter un truc";
        let result = render_markdown(input, 80, &test_theme());

        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();

        let expected_indent = "\u{00a0}".repeat(4);
        assert!(
            all_text.contains(&format!("{}Une barre", expected_indent)),
            "tab indentation not preserved: {:?}",
            all_text
        );
    }

    #[test]
    fn leading_spaces_are_preserved_as_indentation() {
        // Leading spaces should also be preserved so that manually
        // indented content (like "    - item") keeps its visual indent.
        let input = "Idées :\n    - Une barre de progrès\n    - Ajouter un truc";
        let result = render_markdown(input, 80, &test_theme());

        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();

        let expected_indent = "\u{00a0}".repeat(4);
        assert!(
            all_text.contains(&format!("{}- Une barre", expected_indent)),
            "leading space indentation not preserved: {:?}",
            all_text
        );
    }
}
