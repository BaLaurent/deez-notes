use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

/// Top-level application configuration, loaded from TOML.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ui: UiConfig,
    pub sort: SortConfig,
    pub colors: ColorsConfig,
    /// User-defined custom themes (listed alongside built-in themes).
    pub themes: Vec<CustomThemeConfig>,
}

/// A custom theme defined in the config file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomThemeConfig {
    pub name: String,
    pub fg_primary: String,
    pub fg_secondary: String,
    pub accent: String,
    pub highlight: String,
    pub success: String,
    pub error: String,
    pub bg_main: String,
    pub bg_bar: String,
    pub bg_selection: String,
    pub fg_selection: String,
    #[serde(default)]
    pub tag_colors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Path to the notes directory. Supports ~ for home dir.
    pub notes_dir: String,
    /// Preferred editor binary. Empty means fall back to $EDITOR then defaults.
    pub editor: String,
    /// Preferred pager binary for read-only view mode. Empty means fall back to `cat`.
    pub pager: String,
    /// Arguments passed to the pager before the file path (e.g. `["--paging", "always"]`).
    pub pager_args: Vec<String>,
    /// Check for updates on startup (default: true).
    pub check_updates: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    /// Width of the side panel as a percentage (0..100).
    pub side_panel_width_percent: u16,
    /// Show tag badges in the note list.
    pub show_tags: bool,
    /// Show date column in the note list.
    pub show_dates: bool,
    /// strftime-compatible format string for dates.
    pub date_format: String,
    /// Whether search should include note content (not just titles).
    pub search_content: bool,
    /// Name of the selected theme (persisted across restarts).
    pub default_theme: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SortConfig {
    /// One of: "modified", "created", "title".
    pub default_mode: String,
    /// true = ascending (A-Z / oldest first), false = descending.
    pub default_ascending: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ColorsConfig {
    /// Ordered list of color names cycled across tags.
    pub tag_colors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

impl Default for GeneralConfig {
    fn default() -> Self {
        let notes_dir = dirs::home_dir()
            .map(|h| h.join("notes").to_string_lossy().into_owned())
            .unwrap_or_else(|| "~/notes".to_string());

        Self {
            notes_dir,
            editor: String::new(),
            pager: String::new(),
            pager_args: Vec::new(),
            check_updates: true,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            side_panel_width_percent: 30,
            show_tags: true,
            show_dates: true,
            date_format: "%Y-%m-%d".to_string(),
            search_content: true,
            default_theme: String::new(),
        }
    }
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            default_mode: "modified".to_string(),
            default_ascending: false,
        }
    }
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            tag_colors: vec![
                "cyan".to_string(),
                "magenta".to_string(),
                "yellow".to_string(),
                "green".to_string(),
                "red".to_string(),
                "blue".to_string(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load configuration from disk.
///
/// - If `path` is provided, reads from that file.
/// - Otherwise looks for `~/.config/deez-notes/config.toml`.
/// - Returns `Config::default()` when the file does not exist or contains
///   parse errors (parse errors are logged via eprintln).
pub fn load_config(path: Option<&Path>) -> Config {
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => match dirs::config_dir() {
            Some(dir) => dir.join("deez-notes").join("config.toml"),
            None => return Config::default(),
        },
    };

    if !config_path.exists() {
        return Config::default();
    }

    let contents = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "warning: failed to read config at {}: {}",
                config_path.display(),
                e
            );
            return Config::default();
        }
    };

    match toml::from_str::<Config>(&contents) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "warning: failed to parse config at {}: {}",
                config_path.display(),
                e
            );
            Config::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

/// Resolve the config file path (same logic as load_config).
pub fn resolve_config_path(path: Option<&Path>) -> Option<PathBuf> {
    match path {
        Some(p) => Some(p.to_path_buf()),
        None => dirs::config_dir().map(|dir| dir.join("deez-notes").join("config.toml")),
    }
}

/// Save configuration to disk.
///
/// Creates the parent directory if it does not exist.
/// Errors are silently ignored (best-effort persistence).
pub fn save_config(config: &Config, path: &Path) {
    let contents = match toml::to_string_pretty(config) {
        Ok(s) => s,
        Err(_) => return,
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let _ = std::fs::write(path, contents);
}

// ---------------------------------------------------------------------------
// Resolve helpers
// ---------------------------------------------------------------------------

impl Config {
    /// Expand `~` in `notes_dir` to the real home directory and return
    /// an absolute `PathBuf`.
    pub fn resolve_notes_dir(&self) -> PathBuf {
        let raw = &self.general.notes_dir;

        if let Some(rest) = raw.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        if raw == "~" {
            if let Some(home) = dirs::home_dir() {
                return home;
            }
        }

        PathBuf::from(raw)
    }


    /// Convert `sort.default_mode` string into the corresponding `SortMode`.
    pub fn resolve_sort_mode(&self) -> crate::app::SortMode {
        match self.sort.default_mode.as_str() {
            "created" => crate::app::SortMode::ByCreated,
            "title" => crate::app::SortMode::ByTitle,
            _ => crate::app::SortMode::ByModified,
        }
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = Config::default();

        // General
        assert!(
            cfg.general.notes_dir.ends_with("notes"),
            "notes_dir should end with 'notes', got: {}",
            cfg.general.notes_dir
        );
        assert!(cfg.general.editor.is_empty());
        assert!(cfg.general.pager.is_empty());
        assert!(cfg.general.pager_args.is_empty());

        // UI
        assert_eq!(cfg.ui.side_panel_width_percent, 30);
        assert!(cfg.ui.show_tags);
        assert!(cfg.ui.show_dates);
        assert_eq!(cfg.ui.date_format, "%Y-%m-%d");

        // Sort
        assert_eq!(cfg.sort.default_mode, "modified");
        assert!(!cfg.sort.default_ascending);

        // Colors
        assert_eq!(cfg.colors.tag_colors.len(), 6);
        assert_eq!(cfg.colors.tag_colors[0], "cyan");
    }

    #[test]
    fn parse_valid_toml() {
        let toml_str = r#"
[general]
notes_dir = "/tmp/my-notes"
editor = "vim"
pager = "mcat"
pager_args = ["--paging", "always"]

[ui]
side_panel_width_percent = 40
show_tags = false
show_dates = false
date_format = "%d/%m/%Y"
search_content = false

[sort]
default_mode = "title"
default_ascending = true

[colors]
tag_colors = ["red", "blue"]
"#;

        let cfg: Config = toml::from_str(toml_str).expect("valid TOML should parse");

        assert_eq!(cfg.general.notes_dir, "/tmp/my-notes");
        assert_eq!(cfg.general.editor, "vim");
        assert_eq!(cfg.general.pager, "mcat");
        assert_eq!(cfg.general.pager_args, vec!["--paging", "always"]);
        assert_eq!(cfg.ui.side_panel_width_percent, 40);
        assert!(!cfg.ui.show_tags);
        assert!(!cfg.ui.show_dates);
        assert_eq!(cfg.ui.date_format, "%d/%m/%Y");
        assert!(!cfg.ui.search_content);
        assert_eq!(cfg.sort.default_mode, "title");
        assert!(cfg.sort.default_ascending);
        assert_eq!(cfg.colors.tag_colors, vec!["red", "blue"]);
    }

    #[test]
    fn parse_partial_toml_fills_defaults() {
        let toml_str = r#"
[general]
editor = "helix"
"#;

        let cfg: Config = toml::from_str(toml_str).expect("partial TOML should parse");

        assert_eq!(cfg.general.editor, "helix");
        // notes_dir should fall back to default
        assert!(cfg.general.notes_dir.ends_with("notes"));
        // UI should be fully default
        assert_eq!(cfg.ui.side_panel_width_percent, 30);
        assert!(cfg.ui.show_tags);
    }

    #[test]
    fn load_config_returns_default_for_missing_file() {
        let path = Path::new("/tmp/deez-notes-nonexistent-config-12345.toml");
        let cfg = load_config(Some(path));
        assert_eq!(cfg.ui.side_panel_width_percent, 30);
    }

    #[test]
    fn load_config_returns_default_for_invalid_toml() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is [[[not valid toml").expect("write file");

        let cfg = load_config(Some(&path));
        // Should fall back to defaults without panicking
        assert_eq!(cfg.ui.side_panel_width_percent, 30);
        assert!(cfg.general.editor.is_empty());
    }

    #[test]
    fn resolve_notes_dir_expands_tilde() {
        let mut cfg = Config::default();
        cfg.general.notes_dir = "~/my-notes".to_string();

        let resolved = cfg.resolve_notes_dir();

        if let Some(home) = dirs::home_dir() {
            assert_eq!(resolved, home.join("my-notes"));
        }
    }

    #[test]
    fn resolve_notes_dir_absolute_path_unchanged() {
        let mut cfg = Config::default();
        cfg.general.notes_dir = "/srv/notes".to_string();

        assert_eq!(cfg.resolve_notes_dir(), PathBuf::from("/srv/notes"));
    }

    #[test]
    fn resolve_sort_mode_known_values() {
        let mut cfg = Config::default();

        cfg.sort.default_mode = "modified".to_string();
        assert_eq!(cfg.resolve_sort_mode(), crate::app::SortMode::ByModified);

        cfg.sort.default_mode = "created".to_string();
        assert_eq!(cfg.resolve_sort_mode(), crate::app::SortMode::ByCreated);

        cfg.sort.default_mode = "title".to_string();
        assert_eq!(cfg.resolve_sort_mode(), crate::app::SortMode::ByTitle);
    }

    #[test]
    fn resolve_sort_mode_unknown_falls_back_to_modified() {
        let mut cfg = Config::default();
        cfg.sort.default_mode = "nonsense".to_string();
        assert_eq!(cfg.resolve_sort_mode(), crate::app::SortMode::ByModified);
    }
}
