use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

use crate::config::theme::Theme;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Calculate a centered `Rect` for a popup of given width/height within an area.
/// Clamps to the area bounds if the popup is larger than the available space.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

// ---------------------------------------------------------------------------
// ConfirmDeleteDialog
// ---------------------------------------------------------------------------

/// Popup asking the user to confirm note deletion.
pub struct ConfirmDeleteDialog<'a> {
    note_title: &'a str,
    theme: &'a Theme,
}

impl<'a> ConfirmDeleteDialog<'a> {
    pub fn new(note_title: &'a str, theme: &'a Theme) -> Self {
        Self { note_title, theme }
    }
}

impl Widget for ConfirmDeleteDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(40, 6, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Delete Note ")
            .border_style(Style::default().fg(self.theme.error));

        let text = vec![
            Line::from(format!("Delete '{}'?", self.note_title)),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y]", Style::default().fg(self.theme.error).add_modifier(Modifier::BOLD)),
                Span::raw("es  "),
                Span::styled("[N]", Style::default().fg(self.theme.success).add_modifier(Modifier::BOLD)),
                Span::raw("o"),
            ]),
        ];

        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// TextInputDialog
// ---------------------------------------------------------------------------

/// Popup for text input (used by "New Note" and "Rename Note" modes).
pub struct TextInputDialog<'a> {
    title: &'a str,
    input: &'a str,
    theme: &'a Theme,
}

impl<'a> TextInputDialog<'a> {
    pub fn new(title: &'a str, input: &'a str, theme: &'a Theme) -> Self {
        Self { title, input, theme }
    }
}

impl Widget for TextInputDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(50, 5, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(format!(" {} ", self.title))
            .border_style(Style::default().fg(self.theme.accent));

        let display = format!("{}│", self.input);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                display,
                Style::default().fg(self.theme.fg_primary),
            )),
        ];

        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// HelpDialog
// ---------------------------------------------------------------------------

/// Full-screen-ish overlay showing keyboard shortcuts.
pub struct HelpDialog<'a> {
    theme: &'a Theme,
}

impl<'a> HelpDialog<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for HelpDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(60, 25, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Help - Keyboard Shortcuts ")
            .border_style(Style::default().fg(self.theme.success));

        let key_style = Style::default()
            .fg(self.theme.highlight)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(self.theme.fg_primary);

        let bindings: &[(&str, &str)] = &[
            ("Ctrl+N", "New note"),
            ("Ctrl+E", "Edit in editor"),
            ("Ctrl+V", "View read-only"),
            ("Ctrl+D", "Delete note/folder"),
            ("F2", "Rename note"),
            ("Ctrl+X", "Move note to folder"),
            ("Ctrl+G", "Create folder"),
            ("Backspace", "Go to parent folder"),
            ("Ctrl+F", "Search"),
            ("Ctrl+T", "Filter by tag"),
            ("Ctrl+S", "Sort notes"),
            ("Ctrl+R", "Refresh"),
            ("Ctrl+P", "Select theme"),
            ("Tab", "Switch panel"),
            ("\u{2191}\u{2193}/j/k", "Navigate"),
            ("Enter", "Select/Open/Enter folder"),
            ("Ctrl+K", "Show shortcuts"),
            ("Ctrl+Q", "Quit"),
            ("F1/?", "This help"),
        ];

        let lines: Vec<Line<'_>> = bindings
            .iter()
            .map(|(key, action)| {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{:<12}", key), key_style),
                    Span::styled(*action, desc_style),
                ])
            })
            .collect();

        Paragraph::new(lines)
            .block(block)
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// SortMenuDialog
// ---------------------------------------------------------------------------

/// Popup listing sort options with the selected one highlighted.
pub struct SortMenuDialog<'a> {
    selected: usize,
    theme: &'a Theme,
}

impl<'a> SortMenuDialog<'a> {
    pub fn new(selected: usize, theme: &'a Theme) -> Self {
        Self { selected, theme }
    }
}

impl Widget for SortMenuDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(30, 8, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Sort By ")
            .border_style(Style::default().fg(self.theme.highlight));

        let options = ["Modified Date", "Created Date", "Title"];

        let normal_style = Style::default().fg(self.theme.fg_primary);
        let highlight_style = Style::default()
            .fg(self.theme.fg_selection)
            .bg(self.theme.highlight)
            .add_modifier(Modifier::BOLD);

        let lines: Vec<Line<'_>> = options
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let style = if i == self.selected {
                    highlight_style
                } else {
                    normal_style
                };
                let prefix = if i == self.selected { " ▸ " } else { "   " };
                Line::from(Span::styled(format!("{}{}", prefix, label), style))
            })
            .collect();

        Paragraph::new(lines)
            .block(block)
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// ThemeMenuDialog
// ---------------------------------------------------------------------------

/// Popup listing available themes with the selected one highlighted.
pub struct ThemeMenuDialog<'a> {
    selected: usize,
    theme_names: &'a [String],
    theme: &'a Theme,
}

impl<'a> ThemeMenuDialog<'a> {
    pub fn new(selected: usize, theme_names: &'a [String], theme: &'a Theme) -> Self {
        Self { selected, theme_names, theme }
    }
}

impl Widget for ThemeMenuDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = (self.theme_names.len() as u16 + 2).min(area.height);
        let popup = centered_rect(30, height, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Theme ")
            .border_style(Style::default().fg(self.theme.accent));

        let normal_style = Style::default().fg(self.theme.fg_primary);
        let highlight_style = Style::default()
            .fg(self.theme.fg_selection)
            .bg(self.theme.accent)
            .add_modifier(Modifier::BOLD);

        let lines: Vec<Line<'_>> = self
            .theme_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == self.selected {
                    highlight_style
                } else {
                    normal_style
                };
                let prefix = if i == self.selected { " ▸ " } else { "   " };
                Line::from(Span::styled(format!("{}{}", prefix, name), style))
            })
            .collect();

        Paragraph::new(lines)
            .block(block)
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// MoveNoteDialog
// ---------------------------------------------------------------------------

/// Popup listing all folders for moving a note to a different location.
pub struct MoveNoteDialog<'a> {
    selected: usize,
    folder_labels: &'a [String],
    theme: &'a Theme,
}

impl<'a> MoveNoteDialog<'a> {
    pub fn new(selected: usize, folder_labels: &'a [String], theme: &'a Theme) -> Self {
        Self { selected, folder_labels, theme }
    }
}

impl Widget for MoveNoteDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = (self.folder_labels.len() as u16 + 2).min(area.height);
        let popup = centered_rect(40, height, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Move to folder ")
            .border_style(Style::default().fg(self.theme.accent));

        let normal_style = Style::default().fg(self.theme.fg_primary);
        let highlight_style = Style::default()
            .fg(self.theme.fg_selection)
            .bg(self.theme.accent)
            .add_modifier(Modifier::BOLD);

        let lines: Vec<Line<'_>> = self
            .folder_labels
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == self.selected {
                    highlight_style
                } else {
                    normal_style
                };
                let prefix = if i == self.selected { " \u{25b8} " } else { "   " };
                Line::from(Span::styled(format!("{}{}", prefix, name), style))
            })
            .collect();

        Paragraph::new(lines)
            .block(block)
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// ConfirmDeleteFolderDialog
// ---------------------------------------------------------------------------

/// Popup asking the user to confirm folder deletion.
pub struct ConfirmDeleteFolderDialog<'a> {
    folder_name: &'a str,
    theme: &'a Theme,
}

impl<'a> ConfirmDeleteFolderDialog<'a> {
    pub fn new(folder_name: &'a str, theme: &'a Theme) -> Self {
        Self { folder_name, theme }
    }
}

impl Widget for ConfirmDeleteFolderDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(45, 7, area);
        Clear.render(popup, buf);

        let block = Block::bordered()
            .title(" Delete Folder ")
            .border_style(Style::default().fg(self.theme.error));

        let text = vec![
            Line::from(format!("Delete folder '{}'?", self.folder_name)),
            Line::from("(must be empty)"),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y]", Style::default().fg(self.theme.error).add_modifier(Modifier::BOLD)),
                Span::raw("es  "),
                Span::styled("[N]", Style::default().fg(self.theme.success).add_modifier(Modifier::BOLD)),
                Span::raw("o"),
            ]),
        ];

        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(popup, buf);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::theme::Theme;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn centered_rect_basic() {
        let area = Rect::new(0, 0, 100, 50);
        let r = centered_rect(40, 10, area);
        assert_eq!(r.x, 30);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 40);
        assert_eq!(r.height, 10);
    }

    #[test]
    fn centered_rect_clamped() {
        let area = Rect::new(0, 0, 20, 10);
        let r = centered_rect(40, 20, area);
        assert_eq!(r.width, 20);
        assert_eq!(r.height, 10);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
    }

    #[test]
    fn confirm_delete_renders_without_panic() {
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        ConfirmDeleteDialog::new("My Note", &theme).render(area, &mut buf);
    }

    #[test]
    fn text_input_renders_without_panic() {
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        TextInputDialog::new("New Note", "hello", &theme).render(area, &mut buf);
    }

    #[test]
    fn help_dialog_renders_without_panic() {
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        HelpDialog::new(&theme).render(area, &mut buf);
    }

    #[test]
    fn sort_menu_renders_without_panic() {
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        SortMenuDialog::new(1, &theme).render(area, &mut buf);
    }

    #[test]
    fn theme_menu_renders_without_panic() {
        let theme = Theme::terminal(&[]);
        let names = vec!["Terminal".into(), "Catppuccin".into(), "Custom".into()];
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        ThemeMenuDialog::new(0, &names, &theme).render(area, &mut buf);
    }
}
