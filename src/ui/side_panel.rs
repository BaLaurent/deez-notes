use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::app::{AppState, PanelFocus};
use crate::config::settings::UiConfig;
use crate::core::note::Note;

/// Side panel widget displaying the list of notes with title, date, and tag badges.
pub struct SidePanel<'a> {
    state: &'a AppState,
    notes: &'a [Note],
    show_tags: bool,
    show_dates: bool,
    date_format: String,
    tag_colors: Vec<Color>,
}

impl<'a> SidePanel<'a> {
    pub fn new(state: &'a AppState, notes: &'a [Note], ui_config: &UiConfig, tag_colors: &[String]) -> Self {
        Self {
            state,
            notes,
            show_tags: ui_config.show_tags,
            show_dates: ui_config.show_dates,
            date_format: ui_config.date_format.clone(),
            tag_colors: tag_colors.iter().map(|c| parse_color(c)).collect(),
        }
    }
}

impl Widget for SidePanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let count = self.state.filtered_indices.len();
        let title = format!(" Notes ({count}) ");

        let border_style = if self.state.focus == PanelFocus::SidePanel {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let items: Vec<ListItem> = self
            .state
            .filtered_indices
            .iter()
            .map(|&idx| {
                let note = &self.notes[idx];
                let mut spans: Vec<Span> = Vec::new();

                spans.push(Span::raw(&note.title));

                if self.show_dates {
                    let date_str = note.modified.format(&self.date_format).to_string();
                    spans.push(Span::styled(
                        format!(" | {date_str}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                if self.show_tags && !note.tags.is_empty() {
                    for (i, tag) in note.tags.iter().enumerate() {
                        let color = if self.tag_colors.is_empty() {
                            Color::White
                        } else {
                            self.tag_colors[i % self.tag_colors.len()]
                        };
                        spans.push(Span::styled(
                            format!(" [{tag}]"),
                            Style::default().fg(color),
                        ));
                    }
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .fg(Color::Cyan),
            )
            .highlight_symbol("> ");

        let selected = if self.state.filtered_indices.is_empty() {
            None
        } else {
            Some(self.state.selected_index)
        };
        let mut list_state = ListState::default().with_selected(selected);

        StatefulWidget::render(list, area, buf, &mut list_state);
    }
}


fn parse_color(name: &str) -> Color {
    match name {
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "yellow" => Color::Yellow,
        "green" => Color::Green,
        "red" => Color::Red,
        "blue" => Color::Blue,
        "white" => Color::White,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::settings::UiConfig;
    use crate::core::note::Note;
    use chrono::Local;
    use std::path::PathBuf;

    fn make_note(title: &str, tags: Vec<String>) -> Note {
        Note {
            path: PathBuf::from(format!("/tmp/{}.md", title)),
            title: title.to_string(),
            content: None,
            tags,
            created: Local::now(),
            modified: Local::now(),
        }
    }

    #[test]
    fn empty_list_renders_without_panic() {
        let state = AppState::default();
        let notes: Vec<Note> = vec![];
        let ui_config = UiConfig::default();
        let tag_colors: Vec<String> = vec!["cyan".into(), "magenta".into()];

        let panel = SidePanel::new(&state, &notes, &ui_config, &tag_colors);
        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
    }

    #[test]
    fn with_notes_renders_without_panic() {
        let notes = vec![
            make_note("First Note", vec!["rust".into(), "test".into()]),
            make_note("Second Note", vec!["python".into()]),
            make_note("Third Note", vec![]),
        ];
        let state = AppState {
            filtered_indices: vec![0, 1, 2],
            selected_index: 1,
            ..AppState::default()
        };

        let ui_config = UiConfig::default();
        let tag_colors: Vec<String> = vec!["cyan".into(), "magenta".into(), "yellow".into()];

        let panel = SidePanel::new(&state, &notes, &ui_config, &tag_colors);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
    }

    #[test]
    fn parse_color_known_colors() {
        assert_eq!(parse_color("cyan"), Color::Cyan);
        assert_eq!(parse_color("magenta"), Color::Magenta);
        assert_eq!(parse_color("yellow"), Color::Yellow);
        assert_eq!(parse_color("green"), Color::Green);
        assert_eq!(parse_color("red"), Color::Red);
        assert_eq!(parse_color("blue"), Color::Blue);
        assert_eq!(parse_color("white"), Color::White);
        // Unknown falls back to White.
        assert_eq!(parse_color("unknown"), Color::White);
        assert_eq!(parse_color(""), Color::White);
    }
}
