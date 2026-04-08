use std::hash::{Hash, Hasher};

use ratatui::text::Line;

use crate::core::note::Note;

// ---------------------------------------------------------------------------
// Enums — shared across all modules
// ---------------------------------------------------------------------------

/// How the note list is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// All variants intentionally share "By" prefix for semantic clarity (ByModified, ByCreated, ByTitle).
#[allow(clippy::enum_variant_names)]
pub enum SortMode {
    ByModified,
    ByCreated,
    ByTitle,
}

/// Which UI panel currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    SidePanel,
    MainPanel,
}

/// Top-level application mode — determines which input mapping is active
/// and which UI overlay (if any) is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,
    TagFilter,
    SortMenu,
    CreateNote,
    ConfirmDelete,
    Help,
    Rename,
    ThemeMenu,
    MoveNote,
    CreateFolder,
    ConfirmDeleteFolder,
}

/// A semantic action produced by the keybinding layer.
/// The app state machine consumes these — UI and input modules produce them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    NavigateUp,
    NavigateDown,
    Select,
    Edit,
    Create,
    Delete,
    Search,
    TagFilter,
    Sort,
    Refresh,
    ToggleFocus,
    Cancel,
    Quit,
    Help,
    Rename,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    Home,
    End,
    Char(char),
    Backspace,
    ViewReadOnly,
    ThemeMenu,
    MoveNote,
    CreateFolder,
}

// ---------------------------------------------------------------------------
// Markdown rendering cache
// ---------------------------------------------------------------------------

/// Cached result of markdown rendering to avoid re-parsing every frame.
pub struct MarkdownCache {
    /// Hash of the content string that was rendered.
    pub content_hash: u64,
    /// Width the content was rendered at.
    pub rendered_width: u16,
    /// Pre-rendered lines.
    pub lines: Vec<Line<'static>>,
}

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

/// Central application state, owned by the main event loop.
///
/// Note: notes are stored in `NoteManager` (single source of truth).
/// `filtered_indices` indexes into `NoteManager::notes()`.
pub struct AppState {
    /// Indices into `NoteManager::notes()` after search/filter/sort — this is what the UI displays.
    pub filtered_indices: Vec<usize>,
    /// Currently selected index within `filtered_indices`.
    pub selected_index: usize,
    /// Current search query (empty = no search active).
    pub search_query: String,
    /// Active tag filter (None = show all).
    pub active_tag_filter: Option<String>,
    /// Current sort mode.
    pub sort_mode: SortMode,
    /// Sort direction: true = ascending, false = descending.
    pub sort_ascending: bool,
    /// Which panel has focus.
    pub focus: PanelFocus,
    /// Current application mode (drives input handling & UI overlay).
    pub mode: AppMode,
    /// Scroll offset for the main panel (markdown preview).
    pub scroll_offset: usize,
    /// Temporary input buffer used by dialogs (create, rename, etc.).
    pub input_buffer: String,
    /// Optional status message shown in the status bar (auto-clears).
    pub status_message: Option<String>,
    /// Cached markdown rendering for the currently selected note.
    pub cached_markdown: Option<MarkdownCache>,
    /// Current folder being browsed (relative to notes_dir, empty = root).
    pub current_folder: PathBuf,
    /// Folder names displayed at the top of the side panel.
    pub display_folders: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            filtered_indices: Vec::new(),
            selected_index: 0,
            search_query: String::new(),
            active_tag_filter: None,
            sort_mode: SortMode::ByModified,
            sort_ascending: false,
            focus: PanelFocus::SidePanel,
            mode: AppMode::Normal,
            scroll_offset: 0,
            input_buffer: String::new(),
            status_message: None,
            cached_markdown: None,
            current_folder: PathBuf::new(),
            display_folders: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// App Action — signals returned from handle_action to the event loop
// ---------------------------------------------------------------------------

/// Action the event loop must perform after `handle_action` returns.
#[derive(Debug, PartialEq, Eq)]
pub enum AppAction {
    /// Nothing special — continue the event loop.
    None,
    /// Suspend the terminal and open the given path in an external editor.
    OpenEditor(std::path::PathBuf),
    /// Suspend the terminal and open the given path in a read-only viewer.
    OpenViewer(std::path::PathBuf),
    /// Quit the application.
    Quit,
}

// ---------------------------------------------------------------------------
// App — top-level application struct
// ---------------------------------------------------------------------------

use std::path::PathBuf;

use crate::config::settings::{self, Config};
use crate::config::theme::Theme;
use crate::core::note_manager::NoteManager;
use crate::core::search::fuzzy_search;
use crate::core::sort::sort_notes;
use crate::core::tags::{filter_by_tag, tag_filter_items};

/// Top-level application that ties all modules together.
pub struct App {
    pub note_manager: NoteManager,
    pub config: Config,
    /// Path to the config file on disk (for persisting changes).
    pub config_path: Option<PathBuf>,
    pub state: AppState,
    pub current_theme: Theme,
    /// All available themes: built-in + user-defined from config.
    pub available_themes: Vec<Theme>,
    pub sort_menu_index: usize,
    pub tag_filter_index: usize,
    pub theme_menu_index: usize,
    pub move_folder_index: usize,
    pub should_quit: bool,
    /// Whether the UI needs a redraw. Set to `true` on any input or resize event;
    /// cleared after `terminal.draw()` completes.
    pub dirty: bool,
}

impl App {
    /// Create a new App from a Config. Scans the notes directory, builds the
    /// initial filtered list, and applies the configured sort.
    pub fn new(config: Config, config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let notes_dir = config.resolve_notes_dir();
        let mut note_manager = NoteManager::new(notes_dir)?;
        note_manager.scan()?;

        let sort_mode = config.resolve_sort_mode();
        let sort_ascending = config.sort.default_ascending;

        let mut state = AppState {
            sort_mode,
            sort_ascending,
            ..AppState::default()
        };

        // filtered_indices and display_folders will be built by refilter() below.

        let mut available_themes = Theme::builtin_themes(&config.colors.tag_colors);
        for custom in &config.themes {
            available_themes.push(Theme::from_config(custom));
        }

        // Restore saved theme (fall back to first theme if not found).
        let saved_theme = &config.ui.default_theme;
        let theme_index = if saved_theme.is_empty() {
            0
        } else {
            available_themes
                .iter()
                .position(|t| t.name == *saved_theme)
                .unwrap_or(0)
        };
        let current_theme = available_themes[theme_index].clone();

        let mut app = Self {
            note_manager,
            config,
            config_path,
            state,
            current_theme,
            available_themes,
            sort_menu_index: 0,
            tag_filter_index: 0,
            theme_menu_index: theme_index,
            move_folder_index: 0,
            should_quit: false,
            dirty: true,
        };

        // Build initial filtered_indices with folder-awareness.
        app.refilter();

        // Load content for the initially selected note so the main panel
        // doesn't show "Loading..." on first render.
        app.load_selected_content();

        // Surface scan warnings if any files were skipped.
        if !app.note_manager.scan_warnings.is_empty() {
            let count = app.note_manager.scan_warnings.len();
            app.set_status(format!("{} note(s) skipped during scan", count));
        }

        Ok(app)
    }

    /// Main state machine. Dispatches a `KeyAction` based on the current mode.
    /// Returns an `AppAction` telling the event loop what to do next.
    pub fn handle_action(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match self.state.mode {
            AppMode::Normal => self.handle_normal(action),
            AppMode::Search => self.handle_search(action),
            AppMode::CreateNote => self.handle_create_note(action),
            AppMode::Rename => self.handle_rename(action),
            AppMode::ConfirmDelete => self.handle_confirm_delete(action),
            AppMode::TagFilter => self.handle_tag_filter(action),
            AppMode::SortMenu => self.handle_sort_menu(action),
            AppMode::ThemeMenu => self.handle_theme_menu(action),
            AppMode::Help => self.handle_help(action),
            AppMode::MoveNote => self.handle_move_note(action),
            AppMode::CreateFolder => self.handle_create_folder(action),
            AppMode::ConfirmDeleteFolder => self.handle_confirm_delete_folder(action),
        }
    }

    // -- Normal mode ----------------------------------------------------------

    fn handle_normal(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::NavigateUp => {
                if self.state.selected_index > 0 {
                    self.state.selected_index -= 1;
                    self.state.scroll_offset = 0;
                    self.load_selected_content();
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                let max = self.total_display_count().saturating_sub(1);
                if self.state.selected_index < max {
                    self.state.selected_index += 1;
                    self.state.scroll_offset = 0;
                    self.load_selected_content();
                }
                Ok(AppAction::None)
            }
            KeyAction::Select | KeyAction::Edit => {
                // If a folder is selected, enter it
                if let Some(folder_name) = self.selected_folder_name().map(|s| s.to_string()) {
                    if folder_name == ".." {
                        self.navigate_parent_folder();
                    } else {
                        let new_folder = self.state.current_folder.join(&folder_name);
                        self.state.current_folder = new_folder;
                        self.state.selected_index = 0;
                        self.refilter();
                    }
                    return Ok(AppAction::None);
                }
                // Otherwise, open note in editor
                if let Some(real_idx) = self.selected_note_real_index() {
                    let path = self.note_manager.notes()[real_idx].path.clone();
                    Ok(AppAction::OpenEditor(path))
                } else {
                    Ok(AppAction::None)
                }
            }
            KeyAction::Create => {
                self.state.mode = AppMode::CreateNote;
                self.state.input_buffer.clear();
                Ok(AppAction::None)
            }
            KeyAction::Delete => {
                // If a folder is selected, confirm folder deletion
                if self.is_folder_selected() {
                    if self.selected_folder_name().map_or(true, |n| n == "..") {
                        return Ok(AppAction::None); // can't delete ".."
                    }
                    self.state.mode = AppMode::ConfirmDeleteFolder;
                    return Ok(AppAction::None);
                }
                if !self.state.filtered_indices.is_empty() {
                    self.state.mode = AppMode::ConfirmDelete;
                }
                Ok(AppAction::None)
            }
            KeyAction::Backspace => {
                self.navigate_parent_folder();
                Ok(AppAction::None)
            }
            KeyAction::MoveNote => {
                if self.selected_note_real_index().is_some() {
                    self.move_folder_index = 0;
                    self.state.mode = AppMode::MoveNote;
                }
                Ok(AppAction::None)
            }
            KeyAction::CreateFolder => {
                self.state.mode = AppMode::CreateFolder;
                self.state.input_buffer.clear();
                Ok(AppAction::None)
            }
            KeyAction::Search => {
                self.state.mode = AppMode::Search;
                self.state.search_query.clear();
                self.state.input_buffer.clear();
                Ok(AppAction::None)
            }
            KeyAction::TagFilter => {
                self.state.mode = AppMode::TagFilter;
                self.tag_filter_index = 0;
                Ok(AppAction::None)
            }
            KeyAction::Sort => {
                self.state.mode = AppMode::SortMenu;
                self.sort_menu_index = match self.state.sort_mode {
                    SortMode::ByModified => 0,
                    SortMode::ByCreated => 1,
                    SortMode::ByTitle => 2,
                };
                Ok(AppAction::None)
            }
            KeyAction::Refresh => {
                self.note_manager.scan()?;
                self.refilter();
                if self.note_manager.scan_warnings.is_empty() {
                    self.set_status("Refreshed from disk");
                } else {
                    let count = self.note_manager.scan_warnings.len();
                    self.set_status(format!("Refreshed — {} note(s) skipped", count));
                }
                Ok(AppAction::None)
            }
            KeyAction::ToggleFocus => {
                self.state.focus = match self.state.focus {
                    PanelFocus::SidePanel => PanelFocus::MainPanel,
                    _ => PanelFocus::SidePanel,
                };
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.search_query.clear();
                self.state.active_tag_filter = None;
                self.refilter();
                Ok(AppAction::None)
            }
            KeyAction::Quit => {
                self.should_quit = true;
                Ok(AppAction::Quit)
            }
            KeyAction::Help => {
                self.state.mode = AppMode::Help;
                Ok(AppAction::None)
            }
            KeyAction::Rename => {
                if let Some(real_idx) = self.selected_note_real_index() {
                    self.state.mode = AppMode::Rename;
                    self.state.input_buffer =
                        self.note_manager.notes()[real_idx].title.clone();
                }
                Ok(AppAction::None)
            }
            KeyAction::ScrollUp => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
                Ok(AppAction::None)
            }
            KeyAction::ScrollDown => {
                self.state.scroll_offset += 1;
                if let Some(max) = self.max_scroll_offset() {
                    self.state.scroll_offset = self.state.scroll_offset.min(max);
                }
                Ok(AppAction::None)
            }
            KeyAction::PageUp => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(10);
                Ok(AppAction::None)
            }
            KeyAction::PageDown => {
                self.state.scroll_offset += 10;
                if let Some(max) = self.max_scroll_offset() {
                    self.state.scroll_offset = self.state.scroll_offset.min(max);
                }
                Ok(AppAction::None)
            }
            KeyAction::Home => {
                self.state.scroll_offset = 0;
                Ok(AppAction::None)
            }
            KeyAction::End => {
                self.state.scroll_offset = self.max_scroll_offset().unwrap_or(0);
                Ok(AppAction::None)
            }
            KeyAction::ViewReadOnly => {
                if let Some(real_idx) = self.selected_note_real_index() {
                    let path = self.note_manager.notes()[real_idx].path.clone();
                    Ok(AppAction::OpenViewer(path))
                } else {
                    Ok(AppAction::None)
                }
            }
            KeyAction::ThemeMenu => {
                self.state.mode = AppMode::ThemeMenu;
                // Pre-select the current theme in the menu.
                self.theme_menu_index = self
                    .available_themes
                    .iter()
                    .position(|t| t.name == self.current_theme.name)
                    .unwrap_or(0);
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- Search mode ----------------------------------------------------------

    fn handle_search(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Char(c) => {
                self.state.search_query.push(c);
                self.refilter();
                Ok(AppAction::None)
            }
            KeyAction::Backspace => {
                self.state.search_query.pop();
                self.refilter();
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.search_query.clear();
                self.refilter();
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::NavigateUp => {
                if self.state.selected_index > 0 {
                    self.state.selected_index -= 1;
                    self.state.scroll_offset = 0;
                    self.load_selected_content();
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                let max = self.state.filtered_indices.len().saturating_sub(1);
                if self.state.selected_index < max {
                    self.state.selected_index += 1;
                    self.state.scroll_offset = 0;
                    self.load_selected_content();
                }
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- CreateNote mode ------------------------------------------------------

    fn handle_create_note(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Char(c) => {
                self.state.input_buffer.push(c);
                Ok(AppAction::None)
            }
            KeyAction::Backspace => {
                self.state.input_buffer.pop();
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                let title = self.state.input_buffer.clone();
                if title.is_empty() {
                    self.state.mode = AppMode::Normal;
                    return Ok(AppAction::None);
                }
                let path = self.note_manager.create_note(&title, &self.state.current_folder)?;
                self.refilter();
                self.state.mode = AppMode::Normal;
                self.set_status(format!("Created: {}", title));
                Ok(AppAction::OpenEditor(path))
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- Rename mode ----------------------------------------------------------

    fn handle_rename(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Char(c) => {
                self.state.input_buffer.push(c);
                Ok(AppAction::None)
            }
            KeyAction::Backspace => {
                self.state.input_buffer.pop();
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                let new_title = self.state.input_buffer.clone();
                if let Some(real_idx) = self.selected_note_real_index() {
                    if !new_title.is_empty() {
                        self.note_manager.rename_note(real_idx, &new_title)?;
                        self.refilter();
                        self.set_status(format!("Renamed to: {}", new_title));
                    }
                }
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- ConfirmDelete mode ---------------------------------------------------

    fn handle_confirm_delete(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Select => {
                if let Some(real_idx) = self.selected_note_real_index() {
                    let title = self.note_manager.notes()[real_idx].title.clone();
                    self.note_manager.delete_note(real_idx)?;
                    self.refilter();
                    self.set_status(format!("Deleted: {}", title));
                }
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- TagFilter mode -------------------------------------------------------

    fn handle_tag_filter(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        let items = tag_filter_items(self.note_manager.notes());
        let item_count = items.len();

        match action {
            KeyAction::NavigateUp => {
                if self.tag_filter_index > 0 {
                    self.tag_filter_index -= 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                if item_count > 0 && self.tag_filter_index < item_count - 1 {
                    self.tag_filter_index += 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                if self.tag_filter_index == 0 {
                    // "(All notes)" — clear filter
                    self.state.active_tag_filter = None;
                } else if let Some(tag) = items.get(self.tag_filter_index) {
                    self.state.active_tag_filter = Some(tag.clone());
                }
                self.refilter();
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- SortMenu mode --------------------------------------------------------

    fn handle_sort_menu(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        const SORT_OPTION_COUNT: usize = 3;

        match action {
            KeyAction::NavigateUp => {
                if self.sort_menu_index > 0 {
                    self.sort_menu_index -= 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                if self.sort_menu_index < SORT_OPTION_COUNT - 1 {
                    self.sort_menu_index += 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                let new_mode = match self.sort_menu_index {
                    0 => SortMode::ByModified,
                    1 => SortMode::ByCreated,
                    _ => SortMode::ByTitle,
                };
                if new_mode == self.state.sort_mode {
                    self.state.sort_ascending = !self.state.sort_ascending;
                } else {
                    self.state.sort_mode = new_mode;
                }
                self.resort();
                self.state.mode = AppMode::Normal;

                // Persist sort preferences.
                self.config.sort.default_mode = match self.state.sort_mode {
                    SortMode::ByModified => "modified",
                    SortMode::ByCreated => "created",
                    SortMode::ByTitle => "title",
                }
                .to_string();
                self.config.sort.default_ascending = self.state.sort_ascending;
                self.persist_config();

                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- Theme menu mode ------------------------------------------------------

    fn handle_theme_menu(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        let count = self.available_themes.len();

        match action {
            KeyAction::NavigateUp => {
                if self.theme_menu_index > 0 {
                    self.theme_menu_index -= 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                if self.theme_menu_index < count.saturating_sub(1) {
                    self.theme_menu_index += 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                self.select_theme(self.theme_menu_index);
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- Help mode ------------------------------------------------------------

    fn handle_help(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- MoveNote mode --------------------------------------------------------

    fn handle_move_note(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        let folder_list = self.note_manager.all_folder_paths();
        let count = folder_list.len();

        match action {
            KeyAction::NavigateUp => {
                if self.move_folder_index > 0 {
                    self.move_folder_index -= 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::NavigateDown => {
                if self.move_folder_index < count.saturating_sub(1) {
                    self.move_folder_index += 1;
                }
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                if let Some(real_idx) = self.selected_note_real_index() {
                    if let Some(target) = folder_list.get(self.move_folder_index) {
                        let target = target.clone();
                        self.note_manager.move_note(real_idx, &target)?;
                        let display_name = if target.as_os_str().is_empty() {
                            "(root)".to_string()
                        } else {
                            target.display().to_string()
                        };
                        self.set_status(format!("Moved to: {}", display_name));
                        self.refilter();
                    }
                }
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- CreateFolder mode ----------------------------------------------------

    fn handle_create_folder(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Char(c) => {
                self.state.input_buffer.push(c);
                Ok(AppAction::None)
            }
            KeyAction::Backspace => {
                self.state.input_buffer.pop();
                Ok(AppAction::None)
            }
            KeyAction::Select => {
                let name = self.state.input_buffer.clone();
                if !name.is_empty() {
                    let folder_path = self.state.current_folder.join(&name);
                    self.note_manager.create_folder(&folder_path)?;
                    self.refilter();
                    self.set_status(format!("Folder created: {}", name));
                }
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- ConfirmDeleteFolder mode ---------------------------------------------

    fn handle_confirm_delete_folder(&mut self, action: KeyAction) -> anyhow::Result<AppAction> {
        match action {
            KeyAction::Select => {
                if let Some(folder_name) = self.selected_folder_name().map(|s| s.to_string()) {
                    if folder_name != ".." {
                        let folder_path = self.state.current_folder.join(&folder_name);
                        match self.note_manager.delete_folder(&folder_path) {
                            Ok(()) => {
                                self.set_status(format!("Deleted folder: {}", folder_name));
                                self.refilter();
                            }
                            Err(e) => {
                                self.set_status(format!("Cannot delete: {}", e));
                            }
                        }
                    }
                }
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            KeyAction::Cancel => {
                self.state.mode = AppMode::Normal;
                Ok(AppAction::None)
            }
            _ => Ok(AppAction::None),
        }
    }

    // -- Public helpers -------------------------------------------------------

    /// Return the notes slice from the single source of truth (NoteManager).
    pub fn notes(&self) -> &[Note] {
        self.note_manager.notes()
    }

    /// Called by the event loop after an editor session completes.
    /// Refreshes the edited note and resyncs state.
    pub fn after_editor(&mut self, real_idx: usize) -> anyhow::Result<()> {
        if real_idx < self.note_manager.notes().len() {
            self.note_manager.refresh_note(real_idx)?;
            self.load_selected_content();
        }
        self.invalidate_markdown_cache();
        Ok(())
    }

    /// Rebuild `filtered_indices` and `display_folders` from current state.
    pub fn refilter(&mut self) {
        let notes = self.note_manager.notes();
        let has_search = !self.state.search_query.is_empty();
        let has_tag_filter = self.state.active_tag_filter.is_some();

        // When searching or filtering by tag: global mode (all notes, no folders)
        // When browsing: scope to current folder
        let mut indices: Vec<usize> = if has_search || has_tag_filter {
            (0..notes.len()).collect()
        } else {
            self.note_manager.notes_in_folder(&self.state.current_folder)
        };

        // Apply tag filter using HashSet for O(n) lookup instead of O(n²).
        if let Some(ref tag) = self.state.active_tag_filter {
            let tag_indices: std::collections::HashSet<usize> =
                filter_by_tag(notes, tag).into_iter().collect();
            indices.retain(|i| tag_indices.contains(i));
        }

        // Apply search filter using HashSet for O(n) lookup instead of O(n²).
        if has_search {
            let search_indices: std::collections::HashSet<usize> =
                fuzzy_search(&self.state.search_query, notes, self.config.ui.search_content)
                    .into_iter()
                    .collect();
            indices.retain(|i| search_indices.contains(i));
        }

        // Sort.
        sort_notes(
            &mut indices,
            notes,
            self.state.sort_mode,
            self.state.sort_ascending,
        );

        self.state.filtered_indices = indices;

        // Build display_folders: only in browse mode (no search/filter)
        self.state.display_folders = if has_search || has_tag_filter {
            Vec::new()
        } else {
            let mut folders = self.note_manager.subfolders_of(&self.state.current_folder);
            if !self.state.current_folder.as_os_str().is_empty() {
                folders.insert(0, "..".to_string());
            }
            folders
        };

        // Clamp selected_index to the total display count.
        let max = self.total_display_count().saturating_sub(1);
        if self.state.selected_index > max {
            self.state.selected_index = max;
        }
        self.state.scroll_offset = 0;

        self.load_selected_content();
    }

    /// If a note is selected, ensure its content is loaded (lazy loading).
    pub fn load_selected_content(&mut self) {
        if let Some(folder_name) = self.selected_folder_name().map(|s| s.to_string()) {
            self.build_folder_preview(&folder_name);
            return;
        }
        if let Some(real_idx) = self.selected_note_real_index() {
            if let Err(e) = self.note_manager.get_content(real_idx) {
                self.set_status(format!("Failed to load note: {}", e));
            }
        }
        self.invalidate_markdown_cache();
    }

    /// Build a preview of a folder's contents and store it in the markdown cache.
    fn build_folder_preview(&mut self, folder_name: &str) {
        let folder_path = if folder_name == ".." {
            self.state
                .current_folder
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default()
        } else {
            self.state.current_folder.join(folder_name)
        };

        let subfolders = self.note_manager.subfolders_of(&folder_path);
        let note_indices = self.note_manager.notes_in_folder(&folder_path);
        let notes = self.note_manager.notes();

        let mut lines: Vec<Line<'static>> = Vec::new();

        for sub in &subfolders {
            lines.push(Line::from(format!("\u{1F4C1} {sub}/")));
        }

        for &idx in &note_indices {
            let note = &notes[idx];
            let date = note.modified.format("%Y-%m-%d").to_string();
            lines.push(Line::from(format!("  {} | {}", note.title, date)));
        }

        if lines.is_empty() {
            lines.push(Line::from("(empty)"));
        }

        self.state.cached_markdown = Some(MarkdownCache {
            content_hash: 0,
            rendered_width: 0,
            lines,
        });
    }

    /// Convert `selected_index` to the real index in the notes vec.
    /// Returns `None` if a folder is selected or no notes are displayed.
    pub fn selected_note_real_index(&self) -> Option<usize> {
        let folder_count = self.state.display_folders.len();
        if self.state.selected_index < folder_count {
            return None; // a folder is selected
        }
        let note_list_idx = self.state.selected_index - folder_count;
        self.state.filtered_indices.get(note_list_idx).copied()
    }

    /// Whether the currently selected item is a folder entry.
    pub fn is_folder_selected(&self) -> bool {
        self.state.selected_index < self.state.display_folders.len()
    }

    /// Return the name of the currently selected folder, or `None`.
    pub fn selected_folder_name(&self) -> Option<&str> {
        self.state
            .display_folders
            .get(self.state.selected_index)
            .map(|s| s.as_str())
    }

    /// Total number of items displayed (folders + notes).
    pub fn total_display_count(&self) -> usize {
        self.state.display_folders.len() + self.state.filtered_indices.len()
    }

    /// Navigate to the parent folder (no-op if at root).
    fn navigate_parent_folder(&mut self) {
        if self.state.current_folder.as_os_str().is_empty() {
            return; // already at root
        }
        self.state.current_folder = self
            .state
            .current_folder
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        self.state.selected_index = 0;
        self.refilter();
    }

    /// Invalidate the markdown rendering cache.
    pub fn invalidate_markdown_cache(&mut self) {
        self.state.cached_markdown = None;
    }

    /// Pre-render markdown for the selected note if cache is stale.
    pub fn ensure_markdown_cache(&mut self, width: u16) {
        // Get the content of the currently selected note (borrow, not clone).
        let real_idx = match self.selected_note_real_index() {
            Some(idx) => idx,
            None => return,
        };
        let content = match self.note_manager.notes()[real_idx].content.as_deref() {
            Some(c) => c,
            None => return,
        };

        let content_hash = hash_string(content);

        // Check if cache is still valid.
        if let Some(ref cache) = self.state.cached_markdown {
            if cache.content_hash == content_hash && cache.rendered_width == width {
                return; // Cache hit
            }
        }

        // Cache miss — render and store.
        let lines = crate::render::markdown::render_markdown(content, width, &self.current_theme);
        self.state.cached_markdown = Some(MarkdownCache {
            content_hash,
            rendered_width: width,
            lines,
        });
    }

    /// Return the total line count from the markdown cache, or 0 if none.
    pub fn cached_line_count(&self) -> usize {
        self.state
            .cached_markdown
            .as_ref()
            .map_or(0, |c| c.lines.len())
    }

    /// Return the max scroll offset if a markdown cache exists, or `None`
    /// when there is no cache (meaning scroll is unconstrained until render).
    pub fn max_scroll_offset(&self) -> Option<usize> {
        self.state
            .cached_markdown
            .as_ref()
            .map(|c| c.lines.len().saturating_sub(1))
    }

    /// Apply the theme at the given index in available_themes and persist.
    pub fn select_theme(&mut self, index: usize) {
        if let Some(theme) = self.available_themes.get(index) {
            self.current_theme = theme.clone();
            self.invalidate_markdown_cache();
            self.set_status(format!("Theme: {}", self.current_theme.name));
            self.config.ui.default_theme = self.current_theme.name.clone();
            self.persist_config();
        }
    }

    /// Set a status message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.state.status_message = Some(msg.into());
    }

    /// Write the current config to disk (best-effort, errors silently ignored).
    fn persist_config(&self) {
        if let Some(path) = &self.config_path {
            settings::save_config(&self.config, path);
        }
    }

    // -- Private helpers ------------------------------------------------------

    /// Re-sort `filtered_indices` without rebuilding the filter.
    fn resort(&mut self) {
        sort_notes(
            &mut self.state.filtered_indices,
            self.note_manager.notes(),
            self.state.sort_mode,
            self.state.sort_ascending,
        );
    }
}

// ---------------------------------------------------------------------------
// Module-level helpers
// ---------------------------------------------------------------------------

fn hash_string(s: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Config;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: write a .md file with front matter into the given directory.
    fn write_md(dir: &std::path::Path, name: &str, title: &str, tags: &[&str]) {
        let tags_str = tags
            .iter()
            .map(|t| format!("\"{}\"", t))
            .collect::<Vec<_>>()
            .join(", ");
        let content = format!(
            "---\ntitle: \"{title}\"\ncreated: 2025-01-01T00:00:00\nmodified: 2025-01-01T00:00:00\ntags: [{tags_str}]\n---\n\nBody of {title}."
        );
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn test_config(dir: &std::path::Path) -> Config {
        let mut cfg = Config::default();
        cfg.general.notes_dir = dir.to_string_lossy().into_owned();
        cfg.sort.default_mode = "title".to_string();
        cfg.sort.default_ascending = true;
        cfg
    }

    fn setup_app(dir: &TempDir) -> App {
        write_md(dir.path(), "alpha.md", "Alpha", &["rust"]);
        write_md(dir.path(), "beta.md", "Beta", &["rust", "tui"]);
        write_md(dir.path(), "gamma.md", "Gamma", &["python"]);
        App::new(test_config(dir.path()), None).unwrap()
    }

    // -- Construction ---------------------------------------------------------

    #[test]
    fn new_loads_notes_and_builds_indices() {
        let dir = TempDir::new().unwrap();
        let app = setup_app(&dir);

        assert_eq!(app.notes().len(), 3);
        assert_eq!(app.state.filtered_indices.len(), 3);
        assert!(!app.should_quit);
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    #[test]
    fn new_applies_config_sort_mode() {
        let dir = TempDir::new().unwrap();
        let app = setup_app(&dir);

        assert_eq!(app.state.sort_mode, SortMode::ByTitle);
        assert!(app.state.sort_ascending);

        // Sorted ascending by title: Alpha, Beta, Gamma
        let titles: Vec<&str> = app
            .state
            .filtered_indices
            .iter()
            .map(|&i| app.notes()[i].title.as_str())
            .collect();
        assert_eq!(titles, vec!["Alpha", "Beta", "Gamma"]);
    }

    #[test]
    fn new_empty_directory() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let app = App::new(cfg, None).unwrap();

        assert!(app.notes().is_empty());
        assert!(app.state.filtered_indices.is_empty());
        assert_eq!(app.state.selected_index, 0);
    }

    // -- Normal mode navigation -----------------------------------------------

    #[test]
    fn navigate_down_increments_index() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.state.selected_index, 0);
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.state.selected_index, 1);
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.state.selected_index, 2);
    }

    #[test]
    fn navigate_down_clamps_at_end() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.selected_index = 2;
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.state.selected_index, 2);
    }

    #[test]
    fn navigate_up_decrements_index() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.selected_index = 2;
        app.handle_action(KeyAction::NavigateUp).unwrap();
        assert_eq!(app.state.selected_index, 1);
    }

    #[test]
    fn navigate_up_clamps_at_zero() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.state.selected_index, 0);
        app.handle_action(KeyAction::NavigateUp).unwrap();
        assert_eq!(app.state.selected_index, 0);
    }

    #[test]
    fn navigate_resets_scroll_offset() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.scroll_offset = 42;
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.state.scroll_offset, 0);
    }

    // -- Mode transitions -----------------------------------------------------

    #[test]
    fn create_enters_create_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::Create).unwrap();
        assert_eq!(app.state.mode, AppMode::CreateNote);
        assert!(app.state.input_buffer.is_empty());
    }

    #[test]
    fn delete_enters_confirm_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::Delete).unwrap();
        assert_eq!(app.state.mode, AppMode::ConfirmDelete);
    }

    #[test]
    fn delete_noop_when_no_notes() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let mut app = App::new(cfg, None).unwrap();

        app.handle_action(KeyAction::Delete).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    #[test]
    fn search_enters_search_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.search_query = "old query".to_string();
        app.handle_action(KeyAction::Search).unwrap();
        assert_eq!(app.state.mode, AppMode::Search);
        assert!(app.state.search_query.is_empty());
    }

    #[test]
    fn tag_filter_enters_tag_filter_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::TagFilter).unwrap();
        assert_eq!(app.state.mode, AppMode::TagFilter);
        assert_eq!(app.tag_filter_index, 0);
    }

    #[test]
    fn sort_enters_sort_menu_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::Sort).unwrap();
        assert_eq!(app.state.mode, AppMode::SortMenu);
        // Current sort mode is ByTitle (index 2)
        assert_eq!(app.sort_menu_index, 2);
    }

    #[test]
    fn help_enters_help_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::Help).unwrap();
        assert_eq!(app.state.mode, AppMode::Help);
    }

    #[test]
    fn help_cancel_returns_to_normal() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Help;
        app.handle_action(KeyAction::Cancel).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    #[test]
    fn rename_enters_rename_mode_with_current_title() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::Rename).unwrap();
        assert_eq!(app.state.mode, AppMode::Rename);
        // The first note in sorted order is "Alpha"
        assert!(!app.state.input_buffer.is_empty());
    }

    #[test]
    fn rename_noop_when_no_notes() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let mut app = App::new(cfg, None).unwrap();

        app.handle_action(KeyAction::Rename).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    // -- Quit -----------------------------------------------------------------

    #[test]
    fn quit_sets_flag_and_returns_quit_action() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        let action = app.handle_action(KeyAction::Quit).unwrap();
        assert!(app.should_quit);
        assert_eq!(action, AppAction::Quit);
    }

    // -- Toggle focus ---------------------------------------------------------

    #[test]
    fn toggle_focus_cycles() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.state.focus, PanelFocus::SidePanel);
        app.handle_action(KeyAction::ToggleFocus).unwrap();
        assert_eq!(app.state.focus, PanelFocus::MainPanel);
        app.handle_action(KeyAction::ToggleFocus).unwrap();
        assert_eq!(app.state.focus, PanelFocus::SidePanel);
    }

    // -- Scroll ---------------------------------------------------------------

    #[test]
    fn scroll_up_down() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::ScrollDown).unwrap();
        assert_eq!(app.state.scroll_offset, 1);
        app.handle_action(KeyAction::ScrollDown).unwrap();
        assert_eq!(app.state.scroll_offset, 2);
        app.handle_action(KeyAction::ScrollUp).unwrap();
        assert_eq!(app.state.scroll_offset, 1);
    }

    #[test]
    fn scroll_up_clamps_at_zero() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.state.scroll_offset, 0);
        app.handle_action(KeyAction::ScrollUp).unwrap();
        assert_eq!(app.state.scroll_offset, 0);
    }

    #[test]
    fn page_up_down() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.handle_action(KeyAction::PageDown).unwrap();
        assert_eq!(app.state.scroll_offset, 10);
        app.handle_action(KeyAction::PageUp).unwrap();
        assert_eq!(app.state.scroll_offset, 0);
    }

    #[test]
    fn home_end() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        // Build markdown cache so max_scroll_offset() returns Some
        app.ensure_markdown_cache(80);
        let max = app.max_scroll_offset().unwrap_or(0);

        app.handle_action(KeyAction::End).unwrap();
        assert_eq!(app.state.scroll_offset, max);
        app.handle_action(KeyAction::Home).unwrap();
        assert_eq!(app.state.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_clamps_at_max() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);
        app.ensure_markdown_cache(80);
        let max = app.max_scroll_offset().unwrap_or(0);

        // Set scroll near max and try scrolling past
        app.state.scroll_offset = max;
        app.handle_action(KeyAction::ScrollDown).unwrap();
        assert_eq!(app.state.scroll_offset, max);
    }

    #[test]
    fn page_down_clamps_at_max() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);
        app.ensure_markdown_cache(80);
        let max = app.max_scroll_offset().unwrap_or(0);

        app.state.scroll_offset = max.saturating_sub(5);
        app.handle_action(KeyAction::PageDown).unwrap();
        assert_eq!(app.state.scroll_offset, max);
    }

    // -- Cancel clears search and filter --------------------------------------

    #[test]
    fn cancel_in_normal_clears_search_and_filter() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.search_query = "test".to_string();
        app.state.active_tag_filter = Some("rust".to_string());
        app.handle_action(KeyAction::Cancel).unwrap();

        assert!(app.state.search_query.is_empty());
        assert!(app.state.active_tag_filter.is_none());
        assert_eq!(app.state.filtered_indices.len(), 3);
    }

    // -- Search mode ----------------------------------------------------------

    #[test]
    fn search_char_appends_and_filters() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Search;
        app.handle_action(KeyAction::Char('a')).unwrap();
        assert_eq!(app.state.search_query, "a");
        // "Alpha" should match "a", others may or may not
        assert!(!app.state.filtered_indices.is_empty());
    }

    #[test]
    fn search_backspace_pops() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Search;
        app.state.search_query = "ab".to_string();
        app.handle_action(KeyAction::Backspace).unwrap();
        assert_eq!(app.state.search_query, "a");
    }

    #[test]
    fn search_select_confirms() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Search;
        app.handle_action(KeyAction::Select).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    #[test]
    fn search_cancel_clears_query_and_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Search;
        app.state.search_query = "test".to_string();
        app.handle_action(KeyAction::Cancel).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert!(app.state.search_query.is_empty());
        assert_eq!(app.state.filtered_indices.len(), 3);
    }

    #[test]
    fn search_navigate_within_results() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Search;
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.state.selected_index, 1);
        app.handle_action(KeyAction::NavigateUp).unwrap();
        assert_eq!(app.state.selected_index, 0);
    }

    // -- CreateNote mode ------------------------------------------------------

    #[test]
    fn create_note_typing() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::CreateNote;
        app.handle_action(KeyAction::Char('H')).unwrap();
        app.handle_action(KeyAction::Char('i')).unwrap();
        assert_eq!(app.state.input_buffer, "Hi");
    }

    #[test]
    fn create_note_select_creates_and_opens() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::CreateNote;
        app.state.input_buffer = "New Note".to_string();
        let action = app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.notes().len(), 4);
        assert!(matches!(action, AppAction::OpenEditor(_)));
    }

    #[test]
    fn create_note_empty_title_cancels() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::CreateNote;
        app.state.input_buffer.clear();
        let action = app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.notes().len(), 3);
        assert_eq!(action, AppAction::None);
    }

    #[test]
    fn create_note_cancel_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::CreateNote;
        app.state.input_buffer = "test".to_string();
        app.handle_action(KeyAction::Cancel).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.notes().len(), 3);
    }

    // -- Rename mode ----------------------------------------------------------

    #[test]
    fn rename_select_renames() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        // Enter rename mode for the first note
        app.handle_action(KeyAction::Rename).unwrap();
        assert_eq!(app.state.mode, AppMode::Rename);

        app.state.input_buffer = "Renamed Alpha".to_string();
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        // Find the renamed note
        let has_renamed = app.notes().iter().any(|n| n.title == "Renamed Alpha");
        assert!(has_renamed);
    }

    #[test]
    fn rename_cancel_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::Rename;
        app.handle_action(KeyAction::Cancel).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    // -- ConfirmDelete mode ---------------------------------------------------

    #[test]
    fn confirm_delete_select_deletes() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.notes().len(), 3);
        app.state.mode = AppMode::ConfirmDelete;
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.notes().len(), 2);
    }

    #[test]
    fn confirm_delete_cancel_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::ConfirmDelete;
        app.handle_action(KeyAction::Cancel).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.notes().len(), 3);
    }

    #[test]
    fn confirm_delete_clamps_index() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        // Select the last note
        app.state.selected_index = 2;
        app.state.mode = AppMode::ConfirmDelete;
        app.handle_action(KeyAction::Select).unwrap();

        // selected_index should be clamped to new max
        assert!(app.state.selected_index <= app.state.filtered_indices.len().saturating_sub(1));
    }

    // -- TagFilter mode -------------------------------------------------------

    #[test]
    fn tag_filter_navigate() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::TagFilter;
        app.tag_filter_index = 0;

        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.tag_filter_index, 1);
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.tag_filter_index, 2);
    }

    #[test]
    fn tag_filter_navigate_clamps_up() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::TagFilter;
        app.tag_filter_index = 0;
        app.handle_action(KeyAction::NavigateUp).unwrap();
        assert_eq!(app.tag_filter_index, 0);
    }

    #[test]
    fn tag_filter_select_all_clears() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.active_tag_filter = Some("rust".to_string());
        app.state.mode = AppMode::TagFilter;
        app.tag_filter_index = 0; // "(All notes)"
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert!(app.state.active_tag_filter.is_none());
        assert_eq!(app.state.filtered_indices.len(), 3);
    }

    #[test]
    fn tag_filter_select_tag_filters() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::TagFilter;
        // Items: ["(All notes)", "python", "rust", "tui"]
        // index 2 = "rust"
        app.tag_filter_index = 2;
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.state.active_tag_filter.as_deref(), Some("rust"));
        // Only Alpha and Beta have "rust" tag
        assert_eq!(app.state.filtered_indices.len(), 2);
    }

    #[test]
    fn tag_filter_cancel_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::TagFilter;
        app.handle_action(KeyAction::Cancel).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    // -- SortMenu mode --------------------------------------------------------

    #[test]
    fn sort_menu_navigate() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::SortMenu;
        app.sort_menu_index = 0;

        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.sort_menu_index, 1);
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.sort_menu_index, 2);
        // Clamps at 2
        app.handle_action(KeyAction::NavigateDown).unwrap();
        assert_eq!(app.sort_menu_index, 2);
    }

    #[test]
    fn sort_menu_select_changes_mode() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::SortMenu;
        app.sort_menu_index = 0; // ByModified
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.mode, AppMode::Normal);
        assert_eq!(app.state.sort_mode, SortMode::ByModified);
    }

    #[test]
    fn sort_menu_select_same_toggles_ascending() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        // Current mode is ByTitle (ascending)
        assert_eq!(app.state.sort_mode, SortMode::ByTitle);
        assert!(app.state.sort_ascending);

        app.state.mode = AppMode::SortMenu;
        app.sort_menu_index = 2; // ByTitle again
        app.handle_action(KeyAction::Select).unwrap();

        assert_eq!(app.state.sort_mode, SortMode::ByTitle);
        assert!(!app.state.sort_ascending); // toggled
    }

    #[test]
    fn sort_menu_cancel_returns() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.mode = AppMode::SortMenu;
        app.handle_action(KeyAction::Cancel).unwrap();
        assert_eq!(app.state.mode, AppMode::Normal);
    }

    // -- Refilter -------------------------------------------------------------

    #[test]
    fn refilter_respects_tag_and_search() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        // Filter by "rust" tag
        app.state.active_tag_filter = Some("rust".to_string());
        app.refilter();
        assert_eq!(app.state.filtered_indices.len(), 2);

        // Further filter by search "alpha"
        app.state.search_query = "alpha".to_string();
        app.refilter();
        assert_eq!(app.state.filtered_indices.len(), 1);
    }

    #[test]
    fn refilter_clamps_selected_index() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.state.selected_index = 2;
        app.state.search_query = "nonexistent_xyz".to_string();
        app.refilter();
        assert_eq!(app.state.selected_index, 0);
    }

    // -- selected_note_real_index ---------------------------------------------

    #[test]
    fn selected_note_real_index_returns_correct() {
        let dir = TempDir::new().unwrap();
        let app = setup_app(&dir);

        let real = app.selected_note_real_index();
        assert!(real.is_some());
        // The first filtered note's real index should point to a valid note
        let idx = real.unwrap();
        assert!(idx < app.notes().len());
    }

    #[test]
    fn selected_note_real_index_none_when_empty() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let app = App::new(cfg, None).unwrap();

        assert!(app.selected_note_real_index().is_none());
    }

    // -- Refresh --------------------------------------------------------------

    #[test]
    fn refresh_rescans_disk() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        assert_eq!(app.notes().len(), 3);

        // Add another note on disk
        write_md(dir.path(), "delta.md", "Delta", &[]);
        app.handle_action(KeyAction::Refresh).unwrap();

        assert_eq!(app.notes().len(), 4);
        assert_eq!(app.state.filtered_indices.len(), 4);
    }

    // -- Select / Edit returns OpenEditor action ------------------------------

    #[test]
    fn select_returns_open_editor() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        let action = app.handle_action(KeyAction::Select).unwrap();
        assert!(matches!(action, AppAction::OpenEditor(_)));
    }

    #[test]
    fn view_read_only_returns_open_viewer() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        let action = app.handle_action(KeyAction::ViewReadOnly).unwrap();
        assert!(matches!(action, AppAction::OpenViewer(_)));
    }

    #[test]
    fn view_read_only_noop_when_no_notes() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let mut app = App::new(cfg, None).unwrap();

        let action = app.handle_action(KeyAction::ViewReadOnly).unwrap();
        assert_eq!(action, AppAction::None);
    }

    #[test]
    fn select_noop_when_no_notes() {
        let dir = TempDir::new().unwrap();
        let cfg = test_config(dir.path());
        let mut app = App::new(cfg, None).unwrap();

        let action = app.handle_action(KeyAction::Select).unwrap();
        assert_eq!(action, AppAction::None);
    }

    // -- set_status -----------------------------------------------------------

    #[test]
    fn set_status_sets_message() {
        let dir = TempDir::new().unwrap();
        let mut app = setup_app(&dir);

        app.set_status("Hello!");
        assert_eq!(app.state.status_message.as_deref(), Some("Hello!"));
    }
}
