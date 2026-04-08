use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::app::{AppState, PanelFocus};
use crate::config::settings::UiConfig;
use crate::config::theme::Theme;
use crate::core::note::Note;

/// Side panel widget displaying folders and notes with title, date, and tag badges.
pub struct SidePanel<'a> {
    state: &'a AppState,
    notes: &'a [Note],
    show_tags: bool,
    show_dates: bool,
    date_format: String,
    theme: &'a Theme,
}

impl<'a> SidePanel<'a> {
    pub fn new(state: &'a AppState, notes: &'a [Note], ui_config: &UiConfig, theme: &'a Theme) -> Self {
        Self {
            state,
            notes,
            show_tags: ui_config.show_tags,
            show_dates: ui_config.show_dates,
            date_format: ui_config.date_format.clone(),
            theme,
        }
    }
}

impl Widget for SidePanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let note_count = self.state.filtered_indices.len();
        let current_folder = &self.state.current_folder;

        let title = if current_folder.as_os_str().is_empty() {
            format!(" Notes ({note_count}) ")
        } else {
            format!(" Notes ({note_count}) — /{} ", current_folder.display())
        };

        let border_style = if self.state.focus == PanelFocus::SidePanel {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let folder_style = Style::default().fg(self.theme.highlight).add_modifier(Modifier::BOLD);

        // Build folder items first
        let folder_items: Vec<ListItem> = self
            .state
            .display_folders
            .iter()
            .map(|name| {
                let display = if name == ".." {
                    "\u{1F4C1} ..".to_string()
                } else {
                    format!("\u{1F4C1} {name}/")
                };
                ListItem::new(Line::from(Span::styled(display, folder_style)))
            })
            .collect();

        // Build note items
        let note_items: Vec<ListItem> = self
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
                        Style::default().fg(self.theme.fg_secondary),
                    ));
                }

                if self.show_tags && !note.tags.is_empty() {
                    for (i, tag) in note.tags.iter().enumerate() {
                        let color = if self.theme.tag_colors.is_empty() {
                            self.theme.fg_primary
                        } else {
                            self.theme.tag_colors[i % self.theme.tag_colors.len()]
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

        // Combine: folders then notes
        let mut items = folder_items;
        items.extend(note_items);

        let total = items.len();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .fg(self.theme.accent),
            )
            .highlight_symbol("> ");

        let selected = if total == 0 {
            None
        } else {
            Some(self.state.selected_index)
        };
        let mut list_state = ListState::default().with_selected(selected);

        StatefulWidget::render(list, area, buf, &mut list_state);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::settings::UiConfig;
    use crate::config::theme::Theme;
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
        let theme = Theme::terminal(&tag_colors);

        let panel = SidePanel::new(&state, &notes, &ui_config, &theme);
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
        let theme = Theme::terminal(&tag_colors);

        let panel = SidePanel::new(&state, &notes, &ui_config, &theme);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
    }
}
