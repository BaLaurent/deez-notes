use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::app::{AppMode, AppState};

/// Stateless widget that renders the search bar with text input for fuzzy search.
pub struct SearchBar<'a> {
    state: &'a AppState,
}

impl<'a> SearchBar<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for SearchBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let prefix = Span::styled(" Search: ", Style::default().fg(Color::Cyan));

        let content_span = match self.state.mode {
            AppMode::Search => {
                // Active search mode: show query text followed by cursor indicator.
                let mut text = self.state.search_query.clone();
                text.push('\u{258C}'); // ▌ block cursor
                Span::styled(text, Style::default().fg(Color::White))
            }
            _ if self.state.search_query.is_empty() => {
                // Normal mode, no query: show placeholder.
                Span::styled(
                    "Press Ctrl+F to search...",
                    Style::default().fg(Color::DarkGray),
                )
            }
            _ => {
                // Normal mode, query active: show the current filter text.
                Span::styled(
                    self.state.search_query.clone(),
                    Style::default().fg(Color::Yellow),
                )
            }
        };

        let line = Line::from(vec![prefix, content_span]);

        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(Color::Rgb(30, 30, 46)));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn empty_query_normal_mode_shows_placeholder() {
        let state = AppState::default();
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state);
        widget.render(area, &mut buf);

        // The buffer should contain the placeholder text.
        let content: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            content.contains("Ctrl+F"),
            "Placeholder should mention Ctrl+F, got: {content:?}"
        );
    }

    #[test]
    fn active_search_shows_cursor() {
        let state = AppState {
            mode: AppMode::Search,
            search_query: "hello".to_string(),
            ..AppState::default()
        };

        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state);
        widget.render(area, &mut buf);

        // The buffer should contain the query text.
        let content: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            content.contains("hello"),
            "Should contain search query, got: {content:?}"
        );
    }

    #[test]
    fn renders_without_panic() {
        let state = AppState::default();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state);
        widget.render(area, &mut buf);
    }

    #[test]
    fn nonempty_query_normal_mode_shows_query() {
        let state = AppState {
            search_query: "rust".to_string(),
            ..AppState::default()
        };

        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state);
        widget.render(area, &mut buf);

        let content: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            content.contains("rust"),
            "Should show active filter query, got: {content:?}"
        );
    }
}
