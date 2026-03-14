use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::app::{AppMode, AppState};
use crate::config::theme::Theme;

/// Stateless widget that renders the search bar with text input for fuzzy search.
pub struct SearchBar<'a> {
    state: &'a AppState,
    theme: &'a Theme,
}

impl<'a> SearchBar<'a> {
    pub fn new(state: &'a AppState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl<'a> Widget for SearchBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill the entire line with background color first (same as StatusBar).
        let bg_style = Style::default().bg(self.theme.bg_bar);
        for x in area.x..area.x + area.width {
            for y in area.y..area.y + area.height {
                buf[(x, y)].set_style(bg_style);
                buf[(x, y)].set_char(' ');
            }
        }

        let prefix = Span::styled(" Search: ", Style::default().fg(self.theme.accent));

        let content_span = match self.state.mode {
            AppMode::Search => {
                // Active search mode: show query text followed by cursor indicator.
                let mut text = self.state.search_query.clone();
                text.push('\u{258C}'); // ▌ block cursor
                Span::styled(text, Style::default().fg(self.theme.fg_primary))
            }
            _ if self.state.search_query.is_empty() => {
                // Normal mode, no query: show placeholder.
                Span::styled(
                    "Press Ctrl+F to search...",
                    Style::default().fg(self.theme.fg_secondary),
                )
            }
            _ => {
                // Normal mode, query active: show the current filter text.
                Span::styled(
                    self.state.search_query.clone(),
                    Style::default().fg(self.theme.highlight),
                )
            }
        };

        let line = Line::from(vec![prefix, content_span]);

        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(self.theme.bg_bar));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::theme::Theme;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn empty_query_normal_mode_shows_placeholder() {
        let state = AppState::default();
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state, &theme);
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

        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state, &theme);
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
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state, &theme);
        widget.render(area, &mut buf);
    }

    #[test]
    fn nonempty_query_normal_mode_shows_query() {
        let state = AppState {
            search_query: "rust".to_string(),
            ..AppState::default()
        };

        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        let widget = SearchBar::new(&state, &theme);
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
