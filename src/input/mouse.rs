use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// Semantic outcome of a mouse event, resolved against the current layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    /// Scroll the markdown preview (main panel) up one line.
    ScrollPreviewUp,
    /// Scroll the markdown preview (main panel) down one line.
    ScrollPreviewDown,
    /// Move the side-panel selection up one item.
    NavigateUp,
    /// Move the side-panel selection down one item.
    NavigateDown,
    /// Select the side-panel display item at this index (folders then notes).
    SelectDisplayIndex(usize),
    /// Nothing to do.
    None,
}

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Map a raw mouse event to a [`MouseAction`] given the side- and main-panel
/// rectangles and the side panel's current list scroll offset.
///
/// `list_offset` is the index of the first display item visible in the side
/// panel; it is needed so clicks map to the correct item once the list has
/// scrolled. The side panel has a 1-cell border, so its first content row is
/// `side_panel.y + 1`.
pub fn map_mouse_event(
    event: MouseEvent,
    side_panel: Rect,
    main_panel: Rect,
    list_offset: usize,
) -> MouseAction {
    let (col, row) = (event.column, event.row);
    match event.kind {
        MouseEventKind::ScrollDown => {
            if contains(main_panel, col, row) {
                MouseAction::ScrollPreviewDown
            } else if contains(side_panel, col, row) {
                MouseAction::NavigateDown
            } else {
                MouseAction::None
            }
        }
        MouseEventKind::ScrollUp => {
            if contains(main_panel, col, row) {
                MouseAction::ScrollPreviewUp
            } else if contains(side_panel, col, row) {
                MouseAction::NavigateUp
            } else {
                MouseAction::None
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let inner_top = side_panel.y.saturating_add(1);
            // Exclusive of the bottom border row.
            let inner_bottom = side_panel.y.saturating_add(side_panel.height.saturating_sub(1));
            if contains(side_panel, col, row) && row >= inner_top && row < inner_bottom {
                let list_row = (row - inner_top) as usize;
                MouseAction::SelectDisplayIndex(list_offset + list_row)
            } else {
                MouseAction::None
            }
        }
        _ => MouseAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    // side: x=0,w=20,y=1,h=10 (content rows 2..=9); main: x=20,w=30,y=1,h=10
    fn side() -> Rect {
        Rect::new(0, 1, 20, 10)
    }
    fn main() -> Rect {
        Rect::new(20, 1, 30, 10)
    }

    #[test]
    fn wheel_over_preview_scrolls_preview() {
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::ScrollDown, 25, 5), side(), main(), 0),
            MouseAction::ScrollPreviewDown
        );
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::ScrollUp, 25, 5), side(), main(), 0),
            MouseAction::ScrollPreviewUp
        );
    }

    #[test]
    fn wheel_over_side_panel_navigates() {
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::ScrollDown, 5, 5), side(), main(), 0),
            MouseAction::NavigateDown
        );
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::ScrollUp, 5, 5), side(), main(), 0),
            MouseAction::NavigateUp
        );
    }

    #[test]
    fn click_maps_row_to_display_index_with_offset() {
        // First content row is y+1 = 2 -> list row 0 -> offset + 0.
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::Down(MouseButton::Left), 3, 2), side(), main(), 0),
            MouseAction::SelectDisplayIndex(0)
        );
        // Row 4 -> list row 2; offset 5 -> index 7.
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::Down(MouseButton::Left), 3, 4), side(), main(), 5),
            MouseAction::SelectDisplayIndex(7)
        );
    }

    #[test]
    fn click_on_border_rows_is_ignored() {
        // Top border row (y=1) and bottom border row (y+h-1=10).
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::Down(MouseButton::Left), 3, 1), side(), main(), 0),
            MouseAction::None
        );
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::Down(MouseButton::Left), 3, 10), side(), main(), 0),
            MouseAction::None
        );
    }

    #[test]
    fn events_outside_panels_do_nothing() {
        assert_eq!(
            map_mouse_event(ev(MouseEventKind::ScrollDown, 5, 0), side(), main(), 0),
            MouseAction::None
        );
    }
}
