use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Pre-computed layout rectangles for all UI regions.
pub struct AppLayout {
    pub side_panel: Rect,
    pub main_panel: Rect,
    pub search_bar: Rect,
    pub status_bar: Rect,
}

/// Compute the layout for the application given the terminal area.
///
/// Layout structure:
/// ```text
/// ┌─────────────────────────────────────────┐
/// │ search_bar (1 row)                       │
/// ├────────────┬────────────────────────────┤
/// │            │                            │
/// │ side_panel │     main_panel             │
/// │ (N%)       │     (100-N%)               │
/// │            │                            │
/// ├────────────┴────────────────────────────┤
/// │ status_bar (1 row)                       │
/// └─────────────────────────────────────────┘
/// ```
pub fn compute_layout(area: Rect, side_panel_width_percent: u16) -> AppLayout {
    let pct = side_panel_width_percent.clamp(10, 80);

    // Vertical split: search_bar (1) | content (fill) | status_bar (1)
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let search_bar = vertical[0];
    let content_area = vertical[1];
    let status_bar = vertical[2];

    // Horizontal split of content: side_panel | main_panel
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(pct),
            Constraint::Percentage(100 - pct),
        ])
        .split(content_area);

    AppLayout {
        side_panel: horizontal[0],
        main_panel: horizontal[1],
        search_bar,
        status_bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_basic_proportions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = compute_layout(area, 30);

        // search_bar: 1 row at top
        assert_eq!(layout.search_bar.height, 1);
        assert_eq!(layout.search_bar.y, 0);
        assert_eq!(layout.search_bar.width, 80);

        // status_bar: 1 row at bottom
        assert_eq!(layout.status_bar.height, 1);
        assert_eq!(layout.status_bar.y, 23);
        assert_eq!(layout.status_bar.width, 80);

        // Content area fills the remaining 22 rows
        let content_height = 24 - 2; // minus search_bar and status_bar
        assert_eq!(layout.side_panel.height, content_height);
        assert_eq!(layout.main_panel.height, content_height);

        // Side panel width ~ 30% of 80 = 24
        let expected_side = (80 * 30) / 100;
        assert_eq!(layout.side_panel.width, expected_side);

        // main_panel takes the rest
        assert_eq!(
            layout.side_panel.width + layout.main_panel.width,
            80
        );
    }

    #[test]
    fn layout_clamps_percentage() {
        let area = Rect::new(0, 0, 100, 30);

        // 5% should be clamped to 10%
        let layout_low = compute_layout(area, 5);
        let expected_10 = (100 * 10) / 100;
        assert_eq!(layout_low.side_panel.width, expected_10);

        // 95% should be clamped to 80%
        let layout_high = compute_layout(area, 95);
        let expected_80 = (100 * 80) / 100;
        assert_eq!(layout_high.side_panel.width, expected_80);
    }

    #[test]
    fn layout_small_terminal() {
        let area = Rect::new(0, 0, 20, 5);
        let layout = compute_layout(area, 30);

        // Should not panic
        assert!(layout.search_bar.width <= 20);
        assert!(layout.status_bar.width <= 20);
        assert!(layout.side_panel.width + layout.main_panel.width <= 20);

        // All rects should have non-negative dimensions (Rect uses u16, so always >= 0)
        // and should fit within the original area
        assert!(layout.search_bar.x + layout.search_bar.width <= area.width);
        assert!(layout.status_bar.x + layout.status_bar.width <= area.width);
        assert!(layout.side_panel.y + layout.side_panel.height <= area.y + area.height);
        assert!(layout.main_panel.y + layout.main_panel.height <= area.y + area.height);
    }

    #[test]
    fn layout_full_width() {
        let area = Rect::new(0, 0, 100, 50);
        let layout = compute_layout(area, 30);

        // No horizontal overlap between side and main panels
        assert_eq!(layout.main_panel.x, layout.side_panel.x + layout.side_panel.width);

        // No vertical overlap: search_bar, content area, status_bar are stacked
        assert!(layout.search_bar.y + layout.search_bar.height <= layout.side_panel.y);
        assert!(layout.side_panel.y + layout.side_panel.height <= layout.status_bar.y);

        // Full width coverage
        assert_eq!(layout.search_bar.width, 100);
        assert_eq!(layout.status_bar.width, 100);
        assert_eq!(layout.side_panel.width + layout.main_panel.width, 100);
    }
}
