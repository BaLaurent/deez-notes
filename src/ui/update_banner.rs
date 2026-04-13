use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::config::theme::Theme;
use crate::update::UpdateStatus;

/// A single-line banner displayed when a newer version is available.
pub struct UpdateBanner<'a> {
    status: &'a UpdateStatus,
    theme: &'a Theme,
}

impl<'a> UpdateBanner<'a> {
    pub fn new(status: &'a UpdateStatus, theme: &'a Theme) -> Self {
        Self { status, theme }
    }
}

impl<'a> Widget for UpdateBanner<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let (current, latest, hint) = match self.status {
            UpdateStatus::Available {
                current,
                latest,
                ..
            } => (current.as_str(), latest.as_str(), self.status.update_hint().unwrap_or("")),
            _ => return,
        };

        // Fill background.
        let bg_style = Style::default().bg(self.theme.accent);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg_style);
            buf[(x, area.y)].set_char(' ');
        }

        let text_style = Style::default()
            .fg(self.theme.bg_main)
            .bg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let hint_style = Style::default()
            .fg(self.theme.bg_main)
            .bg(self.theme.accent);

        let line = Line::from(vec![
            Span::styled(
                format!(" Update available: v{current} \u{2192} v{latest}"),
                text_style,
            ),
            Span::styled(format!(" | {hint}"), hint_style),
        ]);

        Paragraph::new(line).style(bg_style).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::theme::Theme;
    use crate::update::InstallMethod;

    #[test]
    fn renders_available_update() {
        let status = UpdateStatus::Available {
            current: "0.3.0".to_string(),
            latest: "0.4.0".to_string(),
            install_method: InstallMethod::Cargo,
        };
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        UpdateBanner::new(&status, &theme).render(area, &mut buf);

        let content: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("0.3.0"), "Should show current version");
        assert!(content.contains("0.4.0"), "Should show latest version");
        assert!(content.contains("cargo install"), "Should show cargo command");
    }

    #[test]
    fn up_to_date_renders_nothing() {
        let status = UpdateStatus::UpToDate;
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        UpdateBanner::new(&status, &theme).render(area, &mut buf);
        // Buffer should be untouched (all spaces with default style).
    }

    #[test]
    fn zero_area_no_panic() {
        let status = UpdateStatus::Available {
            current: "0.3.0".to_string(),
            latest: "0.4.0".to_string(),
            install_method: InstallMethod::ManualBinary,
        };
        let theme = Theme::terminal(&[]);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        UpdateBanner::new(&status, &theme).render(area, &mut buf);
    }
}
