use ratatui::style::Color;

use super::settings::CustomThemeConfig;

// ---------------------------------------------------------------------------
// Theme — semantic color roles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub accent: Color,
    pub highlight: Color,
    pub success: Color,
    pub error: Color,
    pub bg_main: Color,
    pub bg_bar: Color,
    pub bg_selection: Color,
    pub fg_selection: Color,
    pub tag_colors: Vec<Color>,
}

impl Theme {
    /// Terminal theme using ANSI standard colors — adapts to the user's terminal palette.
    /// All background colors use `Reset` so dynamic theme tools (Matugen, pywal, etc.)
    /// control the palette entirely. The bars blend with the terminal background.
    /// Takes tag_colors from config so the user's preferences are respected.
    pub fn terminal(config_tag_colors: &[String]) -> Self {
        let tag_colors = if config_tag_colors.is_empty() {
            vec![Color::Cyan, Color::Magenta, Color::Yellow, Color::Green, Color::Red, Color::Blue]
        } else {
            config_tag_colors.iter().map(|c| parse_color(c)).collect()
        };

        Self {
            name: "Terminal".into(),
            fg_primary: Color::Reset,
            fg_secondary: Color::DarkGray,
            accent: Color::Cyan,
            highlight: Color::Yellow,
            success: Color::Green,
            error: Color::Red,
            bg_main: Color::Reset,
            bg_bar: Color::Reset,
            bg_selection: Color::Cyan,
            fg_selection: Color::Black,
            tag_colors,
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            name: "Catppuccin".into(),
            fg_primary: Color::Rgb(205, 214, 244),
            fg_secondary: Color::Rgb(108, 112, 134),
            accent: Color::Rgb(137, 180, 250),
            highlight: Color::Rgb(249, 226, 175),
            success: Color::Rgb(166, 227, 161),
            error: Color::Rgb(243, 139, 168),
            bg_main: Color::Rgb(30, 30, 46),
            bg_bar: Color::Rgb(49, 50, 68),
            bg_selection: Color::Rgb(137, 180, 250),
            fg_selection: Color::Rgb(30, 30, 46),
            tag_colors: vec![
                Color::Rgb(137, 180, 250),
                Color::Rgb(203, 166, 247),
                Color::Rgb(249, 226, 175),
                Color::Rgb(166, 227, 161),
                Color::Rgb(243, 139, 168),
                Color::Rgb(116, 199, 236),
            ],
        }
    }

    pub fn monokai() -> Self {
        Self {
            name: "Monokai".into(),
            fg_primary: Color::Rgb(248, 248, 242),
            fg_secondary: Color::Rgb(117, 113, 94),
            accent: Color::Rgb(102, 217, 239),
            highlight: Color::Rgb(230, 219, 116),
            success: Color::Rgb(166, 226, 46),
            error: Color::Rgb(249, 38, 114),
            bg_main: Color::Rgb(39, 40, 34),
            bg_bar: Color::Rgb(62, 61, 50),
            bg_selection: Color::Rgb(102, 217, 239),
            fg_selection: Color::Rgb(39, 40, 34),
            tag_colors: vec![
                Color::Rgb(102, 217, 239),
                Color::Rgb(174, 129, 255),
                Color::Rgb(230, 219, 116),
                Color::Rgb(166, 226, 46),
                Color::Rgb(249, 38, 114),
                Color::Rgb(253, 151, 31),
            ],
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "Nord".into(),
            fg_primary: Color::Rgb(236, 239, 244),
            fg_secondary: Color::Rgb(76, 86, 106),
            accent: Color::Rgb(136, 192, 208),
            highlight: Color::Rgb(235, 203, 139),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
            bg_main: Color::Rgb(46, 52, 64),
            bg_bar: Color::Rgb(59, 66, 82),
            bg_selection: Color::Rgb(136, 192, 208),
            fg_selection: Color::Rgb(46, 52, 64),
            tag_colors: vec![
                Color::Rgb(136, 192, 208),
                Color::Rgb(180, 142, 173),
                Color::Rgb(235, 203, 139),
                Color::Rgb(163, 190, 140),
                Color::Rgb(191, 97, 106),
                Color::Rgb(129, 161, 193),
            ],
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            name: "Gruvbox".into(),
            fg_primary: Color::Rgb(235, 219, 178),
            fg_secondary: Color::Rgb(146, 131, 116),
            accent: Color::Rgb(131, 165, 152),
            highlight: Color::Rgb(250, 189, 47),
            success: Color::Rgb(184, 187, 38),
            error: Color::Rgb(251, 73, 52),
            bg_main: Color::Rgb(40, 40, 40),
            bg_bar: Color::Rgb(60, 56, 54),
            bg_selection: Color::Rgb(131, 165, 152),
            fg_selection: Color::Rgb(40, 40, 40),
            tag_colors: vec![
                Color::Rgb(131, 165, 152),
                Color::Rgb(211, 134, 155),
                Color::Rgb(250, 189, 47),
                Color::Rgb(184, 187, 38),
                Color::Rgb(251, 73, 52),
                Color::Rgb(69, 133, 136),
            ],
        }
    }

    pub fn darcula() -> Self {
        Self {
            name: "Darcula".into(),
            fg_primary: Color::Rgb(169, 183, 198),
            fg_secondary: Color::Rgb(96, 99, 102),
            accent: Color::Rgb(104, 151, 187),
            highlight: Color::Rgb(255, 198, 109),
            success: Color::Rgb(106, 135, 89),
            error: Color::Rgb(255, 107, 104),
            bg_main: Color::Rgb(43, 43, 43),
            bg_bar: Color::Rgb(60, 63, 65),
            bg_selection: Color::Rgb(33, 66, 131),
            fg_selection: Color::Rgb(169, 183, 198),
            tag_colors: vec![
                Color::Rgb(104, 151, 187),
                Color::Rgb(152, 118, 170),
                Color::Rgb(255, 198, 109),
                Color::Rgb(106, 135, 89),
                Color::Rgb(255, 107, 104),
                Color::Rgb(204, 120, 50),
            ],
        }
    }

    /// Build a custom theme from a config definition.
    pub fn from_config(cfg: &CustomThemeConfig) -> Self {
        Self {
            name: cfg.name.clone(),
            fg_primary: parse_color(&cfg.fg_primary),
            fg_secondary: parse_color(&cfg.fg_secondary),
            accent: parse_color(&cfg.accent),
            highlight: parse_color(&cfg.highlight),
            success: parse_color(&cfg.success),
            error: parse_color(&cfg.error),
            bg_main: parse_color(&cfg.bg_main),
            bg_bar: parse_color(&cfg.bg_bar),
            bg_selection: parse_color(&cfg.bg_selection),
            fg_selection: parse_color(&cfg.fg_selection),
            tag_colors: cfg.tag_colors.iter().map(|c| parse_color(c)).collect(),
        }
    }

    /// Return all 6 built-in themes. Terminal theme uses config tag_colors.
    pub fn builtin_themes(config_tag_colors: &[String]) -> Vec<Theme> {
        vec![
            Self::terminal(config_tag_colors),
            Self::catppuccin(),
            Self::monokai(),
            Self::nord(),
            Self::gruvbox(),
            Self::darcula(),
        ]
    }
}

// ---------------------------------------------------------------------------
// Color parsing utility
// ---------------------------------------------------------------------------

/// Parse a color string into a ratatui Color.
/// Supports named colors ("cyan", "red", …) and hex format ("#rrggbb").
pub fn parse_color(name: &str) -> Color {
    // Hex color: #rrggbb
    if let Some(hex) = name.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }

    match name {
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "yellow" => Color::Yellow,
        "green" => Color::Green,
        "red" => Color::Red,
        "blue" => Color::Blue,
        "white" => Color::White,
        "black" => Color::Black,
        "reset" => Color::Reset,
        _ => Color::White,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_theme_uses_config_tag_colors() {
        let colors = vec!["red".to_string(), "blue".to_string()];
        let theme = Theme::terminal(&colors);
        assert_eq!(theme.tag_colors.len(), 2);
        assert_eq!(theme.tag_colors[0], Color::Red);
        assert_eq!(theme.tag_colors[1], Color::Blue);
    }

    #[test]
    fn terminal_theme_empty_config_uses_defaults() {
        let theme = Theme::terminal(&[]);
        assert_eq!(theme.tag_colors.len(), 6);
    }

    #[test]
    fn terminal_theme_uses_ansi_colors() {
        let theme = Theme::terminal(&[]);
        assert_eq!(theme.fg_primary, Color::Reset);
        assert_eq!(theme.accent, Color::Cyan);
        assert_eq!(theme.highlight, Color::Yellow);
    }

    #[test]
    fn builtin_themes_returns_six() {
        let themes = Theme::builtin_themes(&[]);
        assert_eq!(themes.len(), 6);
        assert_eq!(themes[0].name, "Terminal");
        assert_eq!(themes[5].name, "Darcula");
    }

    #[test]
    fn all_builtins_have_tag_colors() {
        for theme in Theme::builtin_themes(&[]) {
            assert!(!theme.tag_colors.is_empty(), "{} has no tag_colors", theme.name);
        }
    }

    #[test]
    fn from_config_builds_custom_theme() {
        let cfg = CustomThemeConfig {
            name: "My Theme".into(),
            fg_primary: "#cdd6f4".into(),
            fg_secondary: "#6c7086".into(),
            accent: "#89b4fa".into(),
            highlight: "#f9e2af".into(),
            success: "#a6e3a1".into(),
            error: "#f38ba8".into(),
            bg_main: "#1e1e2e".into(),
            bg_bar: "#313244".into(),
            bg_selection: "#89b4fa".into(),
            fg_selection: "#1e1e2e".into(),
            tag_colors: vec!["#ff0000".into(), "cyan".into()],
        };
        let theme = Theme::from_config(&cfg);
        assert_eq!(theme.name, "My Theme");
        assert_eq!(theme.accent, Color::Rgb(0x89, 0xb4, 0xfa));
        assert_eq!(theme.tag_colors.len(), 2);
        assert_eq!(theme.tag_colors[0], Color::Rgb(255, 0, 0));
        assert_eq!(theme.tag_colors[1], Color::Cyan);
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("cyan"), Color::Cyan);
        assert_eq!(parse_color("magenta"), Color::Magenta);
        assert_eq!(parse_color("yellow"), Color::Yellow);
        assert_eq!(parse_color("green"), Color::Green);
        assert_eq!(parse_color("red"), Color::Red);
        assert_eq!(parse_color("blue"), Color::Blue);
        assert_eq!(parse_color("white"), Color::White);
        assert_eq!(parse_color("black"), Color::Black);
        assert_eq!(parse_color("reset"), Color::Reset);
    }

    #[test]
    fn parse_color_hex() {
        assert_eq!(parse_color("#ff0000"), Color::Rgb(255, 0, 0));
        assert_eq!(parse_color("#00ff00"), Color::Rgb(0, 255, 0));
        assert_eq!(parse_color("#1e1e2e"), Color::Rgb(30, 30, 46));
    }

    #[test]
    fn parse_color_invalid_hex_falls_back() {
        assert_eq!(parse_color("#gg0000"), Color::White);
        assert_eq!(parse_color("#fff"), Color::White);
        assert_eq!(parse_color("#"), Color::White);
    }

    #[test]
    fn parse_color_unknown_defaults_to_white() {
        assert_eq!(parse_color("unknown"), Color::White);
        assert_eq!(parse_color(""), Color::White);
    }
}
