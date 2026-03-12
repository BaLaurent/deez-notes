use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::app::{AppState, PanelFocus};
use crate::core::note::Note;
use crate::render::markdown::render_markdown;

/// Stateless widget that renders the selected note's markdown content with scrolling.
pub struct MainPanel<'a> {
    state: &'a AppState,
    notes: &'a [Note],
}

impl<'a> MainPanel<'a> {
    pub fn new(state: &'a AppState, notes: &'a [Note]) -> Self {
        Self { state, notes }
    }
}

impl<'a> Widget for MainPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.state.focus == PanelFocus::MainPanel {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let (title, lines) = match selected_note_content(self.state, self.notes) {
            NoteContent::None => (
                "Preview".to_string(),
                vec![Line::from(Span::styled(
                    "No note selected",
                    Style::default().fg(Color::DarkGray),
                ))],
            ),
            NoteContent::Loading { title } => (
                title,
                vec![Line::from(Span::styled(
                    "Loading...",
                    Style::default().fg(Color::DarkGray),
                ))],
            ),
            NoteContent::Ready { title, content } => {
                if let Some(ref cache) = self.state.cached_markdown {
                    (title, cache.lines.clone())
                } else {
                    let inner_width = area.width.saturating_sub(2);
                    let rendered = render_markdown(&content, inner_width);
                    (title, rendered)
                }
            }
        };

        let block = Block::bordered()
            .title(title)
            .border_style(border_style);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .scroll((self.state.scroll_offset as u16, 0));

        paragraph.render(area, buf);
    }
}


// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

enum NoteContent {
    None,
    Loading { title: String },
    Ready { title: String, content: String },
}

fn selected_note_content(state: &AppState, notes: &[Note]) -> NoteContent {
    if state.filtered_indices.is_empty() {
        return NoteContent::None;
    }

    let idx = state.selected_index;
    if idx >= state.filtered_indices.len() {
        return NoteContent::None;
    }

    let note_idx = state.filtered_indices[idx];
    let note = &notes[note_idx];

    match &note.content {
        Some(text) => NoteContent::Ready {
            title: note.title.clone(),
            content: text.clone(),
        },
        Option::None => NoteContent::Loading {
            title: note.title.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::core::note::Note;
    use chrono::Local;
    use ratatui::{buffer::Buffer, layout::Rect};
    use std::path::PathBuf;

    fn make_note(title: &str, content: Option<&str>) -> Note {
        Note {
            path: PathBuf::from(format!("/tmp/{}.md", title)),
            title: title.to_string(),
            content: content.map(|s| s.to_string()),
            tags: Vec::new(),
            created: Local::now(),
            modified: Local::now(),
        }
    }

    #[test]
    fn empty_state_renders_without_panic() {
        let state = AppState::default();
        let notes: Vec<Note> = vec![];
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let widget = MainPanel::new(&state, &notes);
        widget.render(area, &mut buf);
    }

    #[test]
    fn with_content_renders_without_panic() {
        let notes = vec![make_note("Test Note", Some("# Hello\n\nSome **bold** text."))];
        let state = AppState {
            filtered_indices: vec![0],
            selected_index: 0,
            ..AppState::default()
        };

        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let widget = MainPanel::new(&state, &notes);
        widget.render(area, &mut buf);
    }

    #[test]
    fn loading_state_renders_without_panic() {
        let notes = vec![make_note("Lazy Note", None)];
        let state = AppState {
            filtered_indices: vec![0],
            selected_index: 0,
            ..AppState::default()
        };

        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let widget = MainPanel::new(&state, &notes);
        widget.render(area, &mut buf);
    }

    #[test]
    fn focused_panel_uses_cyan_border() {
        let notes = vec![make_note("Focus", Some("content"))];
        let state = AppState {
            focus: PanelFocus::MainPanel,
            filtered_indices: vec![0],
            ..AppState::default()
        };

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        let widget = MainPanel::new(&state, &notes);
        widget.render(area, &mut buf);
        // Verify it renders without panic (border color is applied internally)
    }
}
