use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::app::{AppMode, AppState};
use crate::core::note::Note;
use crate::core::tags::tag_filter_items;

/// Tag filter bar widget.
///
/// In **Normal mode** (when a tag filter is active): renders a small inline
/// indicator showing the active filter.
///
/// In **TagFilter mode**: renders a bordered overlay with a selectable list
/// of all tags, plus an "(All notes)" option to clear the filter.
pub struct FilterBar<'a> {
    state: &'a AppState,
    notes: &'a [Note],
    /// Index of the currently highlighted tag in the tag list (for TagFilter mode navigation).
    selected_tag_index: usize,
}

impl<'a> FilterBar<'a> {
    pub fn new(state: &'a AppState, notes: &'a [Note], selected_tag_index: usize) -> Self {
        Self {
            state,
            notes,
            selected_tag_index,
        }
    }
}

impl<'a> Widget for FilterBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.state.mode {
            AppMode::TagFilter => render_tag_overlay(self.notes, self.selected_tag_index, area, buf),
            _ => render_filter_indicator(self.state, area, buf),
        }
    }
}

/// Render the inline filter indicator (Normal mode, when a tag filter is active).
fn render_filter_indicator(state: &AppState, area: Rect, buf: &mut Buffer) {
    let tag = match &state.active_tag_filter {
        Some(t) => t,
        None => return, // nothing to show
    };

    let line = Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{}]", tag),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (Ctrl+T to change)", Style::default().fg(Color::DarkGray)),
    ]);

    Paragraph::new(line).render(area, buf);
}

/// Render the tag selection overlay (TagFilter mode).
fn render_tag_overlay(notes: &[Note], selected: usize, area: Rect, buf: &mut Buffer) {
    let items = tag_filter_items(notes);

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Span::styled(label.clone(), style))
        })
        .collect();

    // Clear the area first so the overlay sits on a clean background.
    Clear.render(area, buf);

    let block = Block::default()
        .title(" Filter by Tag ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(list_items).block(block);
    list.render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::note::Note;
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
    fn tag_filter_items_starts_with_all() {
        let notes: Vec<Note> = vec![];
        let items = tag_filter_items(&notes);
        assert_eq!(items[0], "(All notes)");
    }

    #[test]
    fn tag_filter_items_includes_tags() {
        let notes = vec![
            make_note(&["rust", "tui"]),
            make_note(&["cli"]),
        ];
        let items = tag_filter_items(&notes);
        assert_eq!(items[0], "(All notes)");
        assert!(items.contains(&"rust".to_string()));
        assert!(items.contains(&"tui".to_string()));
        assert!(items.contains(&"cli".to_string()));
        // Tags are sorted alphabetically after "(All notes)"
        assert_eq!(items, vec!["(All notes)", "cli", "rust", "tui"]);
    }

    #[test]
    fn renders_without_panic() {
        let state = AppState::default();
        let notes: Vec<Note> = vec![];
        let widget = FilterBar::new(&state, &notes, 0);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn renders_indicator_when_filter_active() {
        let state = AppState {
            active_tag_filter: Some("rust".to_string()),
            ..AppState::default()
        };
        let notes: Vec<Note> = vec![];
        let widget = FilterBar::new(&state, &notes, 0);
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Extract the rendered text from the buffer.
        let rendered: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(rendered.contains("Filter:"));
        assert!(rendered.contains("[rust]"));
    }

    #[test]
    fn renders_overlay_in_tag_filter_mode() {
        let notes = vec![make_note(&["rust"]), make_note(&["cli"])];
        let state = AppState {
            mode: AppMode::TagFilter,
            ..AppState::default()
        };
        let widget = FilterBar::new(&state, &notes, 1);
        let area = Rect::new(0, 0, 30, 8);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // The block title should appear in the rendered output.
        let rendered: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(rendered.contains("Filter by Tag"));
    }
}
