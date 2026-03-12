use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Widget,
};

use crate::app::{AppMode, AppState};
use crate::core::note::Note;

/// Single-line status bar showing note count, active filter, and contextual shortcuts.
pub struct StatusBar<'a> {
    state: &'a AppState,
    notes: &'a [Note],
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a AppState, notes: &'a [Note]) -> Self {
        Self { state, notes }
    }

    /// Build the left-side info string: note count, filter, or search results.
    fn left_info(&self) -> String {
        let total = self.notes.len();
        let visible = self.state.filtered_indices.len();

        if !self.state.search_query.is_empty() {
            return format!("{} results for '{}'", visible, self.state.search_query);
        }

        match &self.state.active_tag_filter {
            Some(tag) => format!("{}/{} notes (filtered by: {})", visible, total, tag),
            None => format!("{} notes", total),
        }
    }

    /// Build shortcut spans for the current mode.
    fn shortcut_spans(&self) -> Vec<Span<'static>> {
        let key_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Color::DarkGray);
        let sep = Span::styled("  ", desc_style);

        match self.state.mode {
            AppMode::Normal => {
                let pairs = [
                    ("^N", " New"),
                    ("^E", " Edit"),
                    ("^V", " View"),
                    ("^D", " Del"),
                    ("^F", " Search"),
                    ("^T", " Tags"),
                    ("^S", " Sort"),
                    ("^Q", " Quit"),
                ];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
            AppMode::Search => {
                let pairs = [
                    ("Esc", " Cancel"),
                    ("Enter", " Select"),
                ];
                let mut spans = build_shortcut_spans(&pairs, key_style, desc_style, sep);
                spans.push(Span::styled("  ", desc_style));
                spans.push(Span::styled("Type to search...", desc_style));
                spans
            }
            AppMode::TagFilter => {
                let pairs = [
                    ("\u{2191}\u{2193}", " Navigate"),
                    ("Enter", " Select"),
                    ("Esc", " Cancel"),
                ];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
            AppMode::SortMenu => {
                let pairs = [
                    ("\u{2191}\u{2193}", " Navigate"),
                    ("Enter", " Select"),
                    ("Esc", " Cancel"),
                ];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
            AppMode::CreateNote | AppMode::Rename => {
                let pairs = [
                    ("Enter", " Confirm"),
                    ("Esc", " Cancel"),
                ];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
            AppMode::ConfirmDelete => {
                let pairs = [
                    ("y", " Yes"),
                    ("n", " No"),
                ];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
            AppMode::Help => {
                let pairs = [("Esc", " Close")];
                build_shortcut_spans(&pairs, key_style, desc_style, sep)
            }
        }
    }
}

/// Build a sequence of styled spans from key/description pairs separated by `sep`.
fn build_shortcut_spans(
    pairs: &[(&str, &str)],
    key_style: Style,
    desc_style: Style,
    sep: Span<'static>,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (key, desc)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(sep.clone());
        }
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(desc.to_string(), desc_style));
    }
    spans
}


impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let bg_style = Style::default().bg(Color::DarkGray);

        // Fill the entire line with background color.
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg_style);
            buf[(x, area.y)].set_char(' ');
        }

        let width = area.width as usize;

        // Left info
        let left_text = self.left_info();
        let left_style = Style::default().fg(Color::White).bg(Color::DarkGray);

        // Status message (center)
        let status_msg = self.state.status_message.clone().unwrap_or_default();
        let msg_style = Style::default().fg(Color::Yellow).bg(Color::DarkGray);

        // Right shortcuts
        let shortcut_spans = self.shortcut_spans();
        let right_text_len: usize = shortcut_spans.iter().map(|s| s.content.len()).sum();

        // Layout: [left] [gap] [status_message] [gap] [shortcuts]
        // Write left info
        let left_len = left_text.len().min(width);
        buf.set_string(area.x, area.y, &left_text[..left_len], left_style);

        // Write right-side shortcuts (right-aligned)
        if right_text_len < width {
            let right_start = area.x + (width - right_text_len) as u16;
            let mut x = right_start;
            for span in &shortcut_spans {
                let style = span.style.bg(Color::DarkGray);
                buf.set_string(x, area.y, span.content.as_ref(), style);
                x += span.content.len() as u16;
            }
        }

        // Write status message in the center (between left and right)
        if !status_msg.is_empty() {
            let available_center = width.saturating_sub(left_len + 2 + right_text_len + 2);
            let msg_len = status_msg.len().min(available_center);
            if msg_len > 0 {
                let center_start = left_len + 2;
                let remaining_gap = available_center.saturating_sub(msg_len);
                let offset = center_start + remaining_gap / 2;
                buf.set_string(
                    area.x + offset as u16,
                    area.y,
                    &status_msg[..msg_len],
                    msg_style,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_renders_without_panic() {
        let state = AppState::default();
        let notes: Vec<crate::core::note::Note> = vec![];
        let bar = StatusBar::new(&state, &notes);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn with_filter_shows_tag() {
        let notes: Vec<crate::core::note::Note> = vec![];
        let state = AppState {
            active_tag_filter: Some("rust".to_string()),
            filtered_indices: vec![],
            ..AppState::default()
        };
        let bar = StatusBar::new(&state, &notes);
        let area = Rect::new(0, 0, 120, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);

        // Extract text from buffer to verify the tag appears
        let rendered: String = (0..120).map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' ')).collect();
        assert!(rendered.contains("filtered by: rust"), "Expected 'filtered by: rust' in: {}", rendered);
    }

}
