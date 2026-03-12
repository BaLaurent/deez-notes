use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppMode, KeyAction};

/// Maps a raw crossterm key event to a semantic [`KeyAction`] based on the
/// current [`AppMode`]. Returns `None` for unmapped keys.
pub fn map_key_event(event: KeyEvent, mode: &AppMode) -> Option<KeyAction> {
    let code = event.code;
    let mods = event.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);

    match mode {
        // -----------------------------------------------------------------
        // Normal mode — full command palette
        // -----------------------------------------------------------------
        AppMode::Normal => match (code, ctrl) {
            (KeyCode::Up, false) | (KeyCode::Char('k'), false) => Some(KeyAction::NavigateUp),
            (KeyCode::Down, false) | (KeyCode::Char('j'), false) => Some(KeyAction::NavigateDown),
            (KeyCode::Enter, false) => Some(KeyAction::Select),
            (KeyCode::Char('e'), true) => Some(KeyAction::Edit),
            (KeyCode::Char('n'), true) => Some(KeyAction::Create),
            (KeyCode::Char('d'), true) => Some(KeyAction::Delete),
            (KeyCode::Delete, false) => Some(KeyAction::Delete),
            (KeyCode::Char('f'), true) => Some(KeyAction::Search),
            (KeyCode::Char('t'), true) => Some(KeyAction::TagFilter),
            (KeyCode::Char('s'), true) => Some(KeyAction::Sort),
            (KeyCode::Char('r'), true) => Some(KeyAction::Refresh),
            (KeyCode::Char('v'), true) => Some(KeyAction::ViewReadOnly),
            (KeyCode::Tab, false) => Some(KeyAction::ToggleFocus),
            (KeyCode::Esc, false) => Some(KeyAction::Cancel),
            (KeyCode::Char('q'), true) => Some(KeyAction::Quit),
            (KeyCode::Char('?'), true) => Some(KeyAction::Help),
            (KeyCode::F(1), false) => Some(KeyAction::Help),
            (KeyCode::F(2), false) => Some(KeyAction::Rename),
            (KeyCode::PageUp, false) => Some(KeyAction::PageUp),
            (KeyCode::PageDown, false) => Some(KeyAction::PageDown),
            (KeyCode::Home, false) => Some(KeyAction::Home),
            (KeyCode::End, false) => Some(KeyAction::End),
            _ => None,
        },

        // -----------------------------------------------------------------
        // Text-input modes: Search, CreateNote, Rename
        // -----------------------------------------------------------------
        AppMode::Search | AppMode::CreateNote | AppMode::Rename => match (code, ctrl) {
            (KeyCode::Esc, _) => Some(KeyAction::Cancel),
            (KeyCode::Enter, false) => Some(KeyAction::Select),
            (KeyCode::Backspace, false) => Some(KeyAction::Backspace),
            (KeyCode::Up, false) => Some(KeyAction::NavigateUp),
            (KeyCode::Down, false) => Some(KeyAction::NavigateDown),
            (KeyCode::Char(c), false) => Some(KeyAction::Char(c)),
            _ => None,
        },

        // -----------------------------------------------------------------
        // List-selection modes: TagFilter, SortMenu
        // -----------------------------------------------------------------
        AppMode::TagFilter | AppMode::SortMenu => match (code, ctrl) {
            (KeyCode::Up, false) | (KeyCode::Char('k'), false) => Some(KeyAction::NavigateUp),
            (KeyCode::Down, false) | (KeyCode::Char('j'), false) => Some(KeyAction::NavigateDown),
            (KeyCode::Enter, false) => Some(KeyAction::Select),
            (KeyCode::Esc, _) => Some(KeyAction::Cancel),
            _ => None,
        },

        // -----------------------------------------------------------------
        // ConfirmDelete — y/Enter = confirm, n/Escape = cancel
        // -----------------------------------------------------------------
        AppMode::ConfirmDelete => match (code, ctrl) {
            (KeyCode::Char('y'), false) | (KeyCode::Enter, false) => Some(KeyAction::Select),
            (KeyCode::Char('n'), false) | (KeyCode::Esc, _) => Some(KeyAction::Cancel),
            _ => None,
        },

        // -----------------------------------------------------------------
        // Help — any dismiss key closes the overlay
        // -----------------------------------------------------------------
        AppMode::Help => match (code, ctrl) {
            (KeyCode::Esc, _)
            | (KeyCode::F(1), false)
            | (KeyCode::Char('q'), false) => Some(KeyAction::Cancel),
            (KeyCode::Char('?'), true) => Some(KeyAction::Cancel),
            _ => None,
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    /// Helper: build a KeyEvent with the given code and modifiers.
    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn plain(code: KeyCode) -> KeyEvent {
        key(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        key(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    // -- Normal mode -------------------------------------------------------

    #[test]
    fn normal_arrow_up() {
        assert_eq!(
            map_key_event(plain(KeyCode::Up), &AppMode::Normal),
            Some(KeyAction::NavigateUp),
        );
    }

    #[test]
    fn normal_k_navigates_up() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('k')), &AppMode::Normal),
            Some(KeyAction::NavigateUp),
        );
    }

    #[test]
    fn normal_arrow_down() {
        assert_eq!(
            map_key_event(plain(KeyCode::Down), &AppMode::Normal),
            Some(KeyAction::NavigateDown),
        );
    }

    #[test]
    fn normal_j_navigates_down() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('j')), &AppMode::Normal),
            Some(KeyAction::NavigateDown),
        );
    }

    #[test]
    fn normal_enter_selects() {
        assert_eq!(
            map_key_event(plain(KeyCode::Enter), &AppMode::Normal),
            Some(KeyAction::Select),
        );
    }

    #[test]
    fn normal_ctrl_e_edits() {
        assert_eq!(
            map_key_event(ctrl('e'), &AppMode::Normal),
            Some(KeyAction::Edit),
        );
    }

    #[test]
    fn normal_ctrl_n_creates() {
        assert_eq!(
            map_key_event(ctrl('n'), &AppMode::Normal),
            Some(KeyAction::Create),
        );
    }

    #[test]
    fn normal_ctrl_d_deletes() {
        assert_eq!(
            map_key_event(ctrl('d'), &AppMode::Normal),
            Some(KeyAction::Delete),
        );
    }

    #[test]
    fn normal_delete_key_deletes() {
        assert_eq!(
            map_key_event(plain(KeyCode::Delete), &AppMode::Normal),
            Some(KeyAction::Delete),
        );
    }

    #[test]
    fn normal_ctrl_f_searches() {
        assert_eq!(
            map_key_event(ctrl('f'), &AppMode::Normal),
            Some(KeyAction::Search),
        );
    }

    #[test]
    fn normal_ctrl_t_tag_filter() {
        assert_eq!(
            map_key_event(ctrl('t'), &AppMode::Normal),
            Some(KeyAction::TagFilter),
        );
    }

    #[test]
    fn normal_ctrl_s_sorts() {
        assert_eq!(
            map_key_event(ctrl('s'), &AppMode::Normal),
            Some(KeyAction::Sort),
        );
    }

    #[test]
    fn normal_ctrl_v_views_read_only() {
        assert_eq!(
            map_key_event(ctrl('v'), &AppMode::Normal),
            Some(KeyAction::ViewReadOnly),
        );
    }

    #[test]
    fn normal_ctrl_r_refreshes() {
        assert_eq!(
            map_key_event(ctrl('r'), &AppMode::Normal),
            Some(KeyAction::Refresh),
        );
    }

    #[test]
    fn normal_tab_toggles_focus() {
        assert_eq!(
            map_key_event(plain(KeyCode::Tab), &AppMode::Normal),
            Some(KeyAction::ToggleFocus),
        );
    }

    #[test]
    fn normal_escape_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Esc), &AppMode::Normal),
            Some(KeyAction::Cancel),
        );
    }

    #[test]
    fn normal_ctrl_q_quits() {
        assert_eq!(
            map_key_event(ctrl('q'), &AppMode::Normal),
            Some(KeyAction::Quit),
        );
    }

    #[test]
    fn normal_ctrl_question_help() {
        assert_eq!(
            map_key_event(
                key(KeyCode::Char('?'), KeyModifiers::CONTROL),
                &AppMode::Normal,
            ),
            Some(KeyAction::Help),
        );
    }

    #[test]
    fn normal_f1_help() {
        assert_eq!(
            map_key_event(plain(KeyCode::F(1)), &AppMode::Normal),
            Some(KeyAction::Help),
        );
    }

    #[test]
    fn normal_f2_rename() {
        assert_eq!(
            map_key_event(plain(KeyCode::F(2)), &AppMode::Normal),
            Some(KeyAction::Rename),
        );
    }

    #[test]
    fn normal_page_up() {
        assert_eq!(
            map_key_event(plain(KeyCode::PageUp), &AppMode::Normal),
            Some(KeyAction::PageUp),
        );
    }

    #[test]
    fn normal_page_down() {
        assert_eq!(
            map_key_event(plain(KeyCode::PageDown), &AppMode::Normal),
            Some(KeyAction::PageDown),
        );
    }

    #[test]
    fn normal_home() {
        assert_eq!(
            map_key_event(plain(KeyCode::Home), &AppMode::Normal),
            Some(KeyAction::Home),
        );
    }

    #[test]
    fn normal_end() {
        assert_eq!(
            map_key_event(plain(KeyCode::End), &AppMode::Normal),
            Some(KeyAction::End),
        );
    }

    #[test]
    fn normal_unmapped_returns_none() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('z')), &AppMode::Normal),
            None,
        );
    }

    // -- Search mode -------------------------------------------------------

    #[test]
    fn search_escape_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Esc), &AppMode::Search),
            Some(KeyAction::Cancel),
        );
    }

    #[test]
    fn search_enter_selects() {
        assert_eq!(
            map_key_event(plain(KeyCode::Enter), &AppMode::Search),
            Some(KeyAction::Select),
        );
    }

    #[test]
    fn search_char_input() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('a')), &AppMode::Search),
            Some(KeyAction::Char('a')),
        );
    }

    #[test]
    fn search_backspace() {
        assert_eq!(
            map_key_event(plain(KeyCode::Backspace), &AppMode::Search),
            Some(KeyAction::Backspace),
        );
    }

    #[test]
    fn search_arrow_up_navigates() {
        assert_eq!(
            map_key_event(plain(KeyCode::Up), &AppMode::Search),
            Some(KeyAction::NavigateUp),
        );
    }

    #[test]
    fn search_arrow_down_navigates() {
        assert_eq!(
            map_key_event(plain(KeyCode::Down), &AppMode::Search),
            Some(KeyAction::NavigateDown),
        );
    }

    // -- ConfirmDelete mode ------------------------------------------------

    #[test]
    fn confirm_y_selects() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('y')), &AppMode::ConfirmDelete),
            Some(KeyAction::Select),
        );
    }

    #[test]
    fn confirm_n_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('n')), &AppMode::ConfirmDelete),
            Some(KeyAction::Cancel),
        );
    }

    // -- Help mode ---------------------------------------------------------

    #[test]
    fn help_escape_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Esc), &AppMode::Help),
            Some(KeyAction::Cancel),
        );
    }

    #[test]
    fn help_q_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Char('q')), &AppMode::Help),
            Some(KeyAction::Cancel),
        );
    }

    #[test]
    fn help_f1_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::F(1)), &AppMode::Help),
            Some(KeyAction::Cancel),
        );
    }

    // -- TagFilter mode ----------------------------------------------------

    #[test]
    fn tag_filter_enter_selects() {
        assert_eq!(
            map_key_event(plain(KeyCode::Enter), &AppMode::TagFilter),
            Some(KeyAction::Select),
        );
    }

    #[test]
    fn tag_filter_escape_cancels() {
        assert_eq!(
            map_key_event(plain(KeyCode::Esc), &AppMode::TagFilter),
            Some(KeyAction::Cancel),
        );
    }

    #[test]
    fn rapid_key_sequences_map_independently() {
        // Simulate multiple keys in quick succession; each maps independently
        let keys = vec![
            (plain(KeyCode::Up), &AppMode::Normal, Some(KeyAction::NavigateUp)),
            (plain(KeyCode::Down), &AppMode::Normal, Some(KeyAction::NavigateDown)),
            (plain(KeyCode::Enter), &AppMode::Normal, Some(KeyAction::Select)),
            (ctrl('e'), &AppMode::Normal, Some(KeyAction::Edit)),
            (ctrl('n'), &AppMode::Normal, Some(KeyAction::Create)),
        ];

        for (event, mode, expected) in keys {
            assert_eq!(
                map_key_event(event, mode),
                expected,
                "key sequence should map independently"
            );
        }
    }

    #[test]
    fn all_ctrl_combinations_no_crash() {
        // Test all printable Ctrl+<char> combinations in Normal mode
        // Unmapped ones should return None, never crash
        for c in 'a'..='z' {
            let result = map_key_event(ctrl(c), &AppMode::Normal);
            // Some are mapped (e, n, d, f, t, s, r, q), others should be None
            match c {
                'e' => assert_eq!(result, Some(KeyAction::Edit)),
                'n' => assert_eq!(result, Some(KeyAction::Create)),
                'd' => assert_eq!(result, Some(KeyAction::Delete)),
                'f' => assert_eq!(result, Some(KeyAction::Search)),
                't' => assert_eq!(result, Some(KeyAction::TagFilter)),
                's' => assert_eq!(result, Some(KeyAction::Sort)),
                'r' => assert_eq!(result, Some(KeyAction::Refresh)),
                'v' => assert_eq!(result, Some(KeyAction::ViewReadOnly)),
                'q' => assert_eq!(result, Some(KeyAction::Quit)),
                _ => assert_eq!(result, None, "Ctrl+{} should be unmapped", c),
            }
        }
    }

    #[test]
    fn all_ctrl_combinations_in_search_mode_no_crash() {
        // In Search mode, Ctrl+<char> combos should all return None
        // (since chars only map without Ctrl modifier)
        for c in 'a'..='z' {
            let result = map_key_event(ctrl(c), &AppMode::Search);
            // In Search/CreateNote/Rename, Char(c) maps only with false ctrl
            // ctrl chars should return None
            assert_eq!(result, None, "Ctrl+{} in Search should be None", c);
        }
    }
}
