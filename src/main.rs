use std::io::{self, Stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use deez_notes::app::{App, AppAction, AppMode, KeyAction};
use deez_notes::{cli, config, editor, input, ui};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "deez-notes", version, about = "TUI Markdown note manager")]
struct Cli {
    /// Notes directory (default: ~/notes/)
    #[arg()]
    directory: Option<String>,

    /// Path to config file
    #[arg(short, long)]
    config: Option<String>,

    /// Override editor binary
    #[arg(long)]
    editor: Option<String>,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config (from CLI path or default location).
    let config_path = cli.config.as_deref().map(Path::new);
    let resolved_config_path = config::settings::resolve_config_path(config_path);
    let mut config = config::settings::load_config(config_path);

    // Apply CLI overrides.
    if let Some(dir) = cli.directory {
        config.general.notes_dir = dir;
    }
    if let Some(ed) = cli.editor {
        config.general.editor = ed;
    }

    // CLI mode: a subcommand runs without ever touching the terminal.
    if let Some(command) = cli.command {
        return cli::run(command, config);
    }

    // Install panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stderr(), Show);
        original_hook(info);
    }));

    // Setup terminal.
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build the application.
    let mut app = App::new(config, resolved_config_path)?;

    // Run the event loop.
    let result = run_app(&mut terminal, &mut app);

    // Cleanup terminal (always, even on error).
    let _ = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal::disable_raw_mode();

    result
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut status_set_at: Option<Instant> = None;
    let mut update_banner_shown_at: Option<Instant> = None;
    // Last left-click (time, display index), for double-click detection.
    let mut last_click: Option<(Instant, usize)> = None;

    loop {
        // Auto-clear status message after 3 seconds.
        if let Some(set_at) = status_set_at {
            if set_at.elapsed() >= Duration::from_secs(3) {
                app.state.status_message = None;
                status_set_at = None;
                app.dirty = true;
            }
        }
        if app.state.status_message.is_some() && status_set_at.is_none() {
            status_set_at = Some(Instant::now());
        }
        if app.state.status_message.is_none() {
            status_set_at = None;
        }

        // Poll for background update check result.
        app.poll_update_check();

        // Auto-dismiss update banner after 60 seconds.
        if app.show_update_banner() && update_banner_shown_at.is_none() {
            update_banner_shown_at = Some(Instant::now());
        }
        if let Some(shown_at) = update_banner_shown_at {
            if shown_at.elapsed() >= Duration::from_secs(60) {
                app.update_dismissed = true;
                update_banner_shown_at = None;
                app.dirty = true;
            }
        }

        if app.dirty {
            let area: ratatui::layout::Rect = terminal.size()?.into();
            let layout = ui::layout::compute_layout(
                area,
                app.config.ui.side_panel_width_percent,
                app.show_update_banner(),
            );
            let main_panel_width = layout.main_panel.width.saturating_sub(2); // borders
            app.ensure_markdown_cache(main_panel_width);
            terminal.draw(|frame| draw_ui(frame, app))?;
            app.dirty = false;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    app.dirty = true;
                    // Dismiss update banner on any keypress.
                    if app.show_update_banner() {
                        app.update_dismissed = true;
                    }
                    if let Some(action) =
                        input::keybindings::map_key_event(key_event, &app.state.mode)
                    {
                        let app_action = app.handle_action(action)?;
                        if dispatch_app_action(app, terminal, app_action)? {
                            break;
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    app.dirty = true;
                    let app_action =
                        handle_mouse(mouse_event, app, terminal, &mut last_click)?;
                    if dispatch_app_action(app, terminal, app_action)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    app.dirty = true;
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Print a prompt and block until the user presses any key.
///
/// Uses raw mode temporarily so a single keypress is enough (no Enter needed).
fn wait_for_keypress() {
    use std::io::Write;

    eprint!("\n--- Press any key to return ---");
    let _ = io::stderr().flush();

    // Enable raw mode so we get a single keypress without waiting for Enter.
    let _ = terminal::enable_raw_mode();
    loop {
        if event::poll(Duration::from_secs(60)).unwrap_or(false) {
            if let Ok(Event::Key(k)) = event::read() {
                if k.kind == KeyEventKind::Press {
                    break;
                }
            }
        }
    }
    let _ = terminal::disable_raw_mode();
}

/// How long after a click a second click on the same item still counts as a
/// double-click.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

/// Translate a mouse event into an app action using the current layout.
///
/// Mouse input is only honoured in `Normal` mode; while a dialog or overlay is
/// open the mouse is ignored. The wheel scrolls the markdown preview when over
/// the main panel and moves the selection when over the side panel. A single
/// left click selects the side-panel item under the cursor; a double click on
/// the same item activates it (enters a folder or opens a note in the editor),
/// exactly like pressing Enter.
///
/// `last_click` carries the previous click's time and index across calls so
/// double-clicks can be detected (crossterm does not deliver a native
/// double-click event).
fn handle_mouse(
    mouse_event: crossterm::event::MouseEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    last_click: &mut Option<(Instant, usize)>,
) -> Result<AppAction> {
    // Mouse is honoured in Normal mode and in Search mode (so search results stay
    // clickable); other dialogs and overlays ignore it.
    if !matches!(app.state.mode, AppMode::Normal | AppMode::Search) {
        return Ok(AppAction::None);
    }

    let area: ratatui::layout::Rect = terminal.size()?.into();
    let layout = ui::layout::compute_layout(
        area,
        app.config.ui.side_panel_width_percent,
        app.show_update_banner(),
    );

    use input::mouse::MouseAction;
    let action = input::mouse::map_mouse_event(
        mouse_event,
        layout.side_panel,
        layout.main_panel,
        layout.search_bar,
        app.list_state.offset(),
    );

    match action {
        MouseAction::ScrollPreviewUp => app.handle_action(KeyAction::ScrollUp),
        MouseAction::ScrollPreviewDown => app.handle_action(KeyAction::ScrollDown),
        MouseAction::NavigateUp => app.handle_action(KeyAction::NavigateUp),
        MouseAction::NavigateDown => app.handle_action(KeyAction::NavigateDown),
        MouseAction::FocusSearch => app.handle_action(KeyAction::Search),
        MouseAction::SelectDisplayIndex(idx) => {
            // Clicking a result confirms an active search, like pressing Enter.
            if app.state.mode == AppMode::Search {
                app.handle_action(KeyAction::Select)?;
            }
            let now = Instant::now();
            let is_double = last_click
                .map(|(t, i)| i == idx && now.duration_since(t) <= DOUBLE_CLICK_WINDOW)
                .unwrap_or(false);
            app.select_display_index(idx);
            if is_double {
                *last_click = None;
                app.handle_action(KeyAction::Select)
            } else {
                *last_click = Some((now, idx));
                Ok(AppAction::None)
            }
        }
        MouseAction::None => Ok(AppAction::None),
    }
}

/// Carry out an [`AppAction`], suspending and restoring the terminal around
/// external editor/viewer invocations. Shared by the keyboard and mouse paths.
/// Returns `true` when the application should quit.
fn dispatch_app_action(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    action: AppAction,
) -> Result<bool> {
    match action {
        AppAction::OpenEditor(path) => {
            // Suspend terminal.
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
            terminal::disable_raw_mode()?;
            execute!(io::stdout(), Show)?;

            let real_idx = app.selected_note_real_index();

            let editor_override = if app.config.general.editor.is_empty() {
                None
            } else {
                Some(app.config.general.editor.as_str())
            };
            let result = editor::external::open_in_editor(&path, editor_override);

            // Resume terminal.
            terminal::enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
            terminal.clear()?;

            if let Err(e) = result {
                app.set_status(format!("Editor error: {}", e));
            } else if let Some(idx) = real_idx {
                app.after_editor(idx)?;
            }
            app.dirty = true;
        }
        AppAction::OpenViewer(path) => {
            // Suspend terminal (leave alternate screen so output is visible).
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
            terminal::disable_raw_mode()?;
            execute!(io::stdout(), Show)?;

            let pager = if app.config.general.pager.is_empty() {
                None
            } else {
                Some(app.config.general.pager.as_str())
            };
            let result = editor::external::open_in_viewer(
                &path,
                pager,
                &app.config.general.pager_args,
            );

            match &result {
                Ok(is_pager) => {
                    if !is_pager {
                        wait_for_keypress();
                    }
                }
                Err(e) => {
                    eprintln!("\nViewer error: {}", e);
                    wait_for_keypress();
                }
            }

            // Resume terminal.
            terminal::enable_raw_mode()?;
            execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
            terminal.clear()?;

            if let Err(e) = result {
                app.set_status(format!("Viewer error: {}", e));
            }
            app.dirty = true;
        }
        AppAction::Quit => return Ok(true),
        AppAction::None => {}
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// UI drawing
// ---------------------------------------------------------------------------

fn draw_ui(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    let show_banner = app.show_update_banner();
    let layout = ui::layout::compute_layout(
        area,
        app.config.ui.side_panel_width_percent,
        show_banner,
    );
    let notes = app.note_manager.notes();

    let theme = &app.current_theme;

    // Fill the entire frame with the theme background color.
    frame.render_widget(
        ratatui::widgets::Block::default().style(ratatui::style::Style::default().bg(theme.bg_main)),
        area,
    );

    // Update banner (conditional).
    if let (true, Some(banner_area), Some(status)) =
        (show_banner, layout.update_banner, &app.update_status)
    {
        frame.render_widget(
            ui::update_banner::UpdateBanner::new(status, theme),
            banner_area,
        );
    }

    // Base widgets.
    frame.render_widget(ui::search_bar::SearchBar::new(&app.state, theme), layout.search_bar);
    frame.render_stateful_widget(
        ui::side_panel::SidePanel::new(&app.state, notes, &app.config.ui, theme),
        layout.side_panel,
        &mut app.list_state,
    );
    frame.render_widget(ui::main_panel::MainPanel::new(&app.state, notes, theme), layout.main_panel);
    frame.render_widget(ui::status_bar::StatusBar::new(&app.state, notes, theme), layout.status_bar);

    // Overlays based on current mode.
    match app.state.mode {
        AppMode::ConfirmDelete => {
            if let Some(real_idx) = app.selected_note_real_index() {
                let title = &notes[real_idx].title;
                frame.render_widget(ui::dialogs::ConfirmDeleteDialog::new(title, theme), area);
            }
        }
        AppMode::CreateNote => {
            frame.render_widget(
                ui::dialogs::TextInputDialog::new("New Note", &app.state.input_buffer, theme),
                area,
            );
        }
        AppMode::Rename => {
            frame.render_widget(
                ui::dialogs::TextInputDialog::new("Rename Note", &app.state.input_buffer, theme),
                area,
            );
        }
        AppMode::Help => {
            frame.render_widget(ui::dialogs::HelpDialog::new(theme), area);
        }
        AppMode::SortMenu => {
            frame.render_widget(ui::dialogs::SortMenuDialog::new(app.sort_menu_index, theme), area);
        }
        AppMode::ThemeMenu => {
            let theme_names: Vec<String> = app.available_themes.iter().map(|t| t.name.clone()).collect();
            frame.render_widget(
                ui::dialogs::ThemeMenuDialog::new(app.theme_menu_index, &theme_names, theme),
                area,
            );
        }
        AppMode::TagFilter => {
            let items = deez_notes::core::tags::tag_filter_items(notes);
            let height = (items.len() as u16 + 2).min(area.height);
            let overlay = ui::dialogs::centered_rect(40, height, area);
            frame.render_widget(
                ui::filter_bar::FilterBar::new(&app.state, notes, app.tag_filter_index, theme),
                overlay,
            );
        }
        AppMode::MoveNote => {
            let folder_paths = app.note_manager.all_folder_paths();
            let labels: Vec<String> = folder_paths
                .iter()
                .map(|p| {
                    if p.as_os_str().is_empty() {
                        "(Root)".to_string()
                    } else {
                        p.display().to_string()
                    }
                })
                .collect();
            frame.render_widget(
                ui::dialogs::MoveNoteDialog::new(app.move_folder_index, &labels, theme),
                area,
            );
        }
        AppMode::CreateFolder => {
            frame.render_widget(
                ui::dialogs::TextInputDialog::new("New Folder", &app.state.input_buffer, theme),
                area,
            );
        }
        AppMode::ConfirmDeleteFolder => {
            if let Some(name) = app.selected_folder_name() {
                frame.render_widget(
                    ui::dialogs::ConfirmDeleteFolderDialog::new(name, theme),
                    area,
                );
            }
        }
        _ => {}
    }
}
