# deez-notes

A fast, keyboard-driven TUI note manager for Markdown files. Built with Rust and [Ratatui](https://ratatui.rs).

![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- Browse and preview Markdown notes with rendered formatting
- YAML front matter support (title, tags, dates)
- Fuzzy search across note titles and content
- Tag filtering and sorting (by modified date, creation date, or title)
- Create, rename, and delete notes directly from the TUI
- Open notes in your preferred editor (`$EDITOR`, or configurable)
- Read-only viewer mode with configurable pager (falls back to `cat`)
- Keyboard-driven with `Ctrl+` shortcuts and `j`/`k` navigation
- Configurable via TOML (`~/.config/deez-notes/config.toml`)

## Installation

### Pre-built binaries

Download the latest release from the [releases page](https://github.com/BaLaurent/deez-notes/releases).

**Linux:**

```bash
# Download and install globally
curl -L https://github.com/BaLaurent/deez-notes/releases/latest/download/deez-notes-linux-amd64 -o deez-notes
chmod +x deez-notes
sudo mv deez-notes /usr/bin/deez-notes
```

**Windows:**

Download `deez-notes-windows-amd64.exe` from the [releases page](https://github.com/BaLaurent/deez-notes/releases) and add it to your PATH.

### From source

```bash
cargo install --path .
```

### Build manually

```bash
git clone https://github.com/BaLaurent/deez-notes.git
cd deez-notes
cargo build --release
# Binary is at ./target/release/deez-notes
```

## Usage

```
deez-notes [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]  Notes directory (default: ~/notes/)

Options:
  -c, --config <CONFIG>  Path to config file
      --editor <EDITOR>  Override editor binary
  -h, --help             Print help
  -V, --version          Print version
```

### Quick start

```bash
# Use default ~/notes/ directory
deez-notes

# Specify a notes directory
deez-notes ~/my-notes

# Use a custom editor
deez-notes --editor nvim
```

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `Enter` | Select / preview note |
| `Tab` | Toggle focus (side panel / main panel) |
| `PageUp` / `PageDown` | Scroll page |
| `Home` / `End` | Jump to first / last |

### Actions

| Key | Action |
|-----|--------|
| `Ctrl+E` | Edit note in external editor |
| `Ctrl+V` | View note in read-only viewer |
| `Ctrl+N` | Create new note |
| `Ctrl+D` / `Delete` | Delete note |
| `F2` | Rename note |
| `Ctrl+F` | Search notes |
| `Ctrl+T` | Filter by tag |
| `Ctrl+S` | Sort notes |
| `Ctrl+P` | Select theme |
| `Ctrl+R` | Refresh note list |
| `Ctrl+Q` | Quit |
| `F1` / `Ctrl+?` / `Ctrl+K` | Help |

## Configuration

Create `~/.config/deez-notes/config.toml`:

```toml
[general]
notes_dir = "~/notes"
editor = "nvim"
pager = "mcat"                       # pager for read-only view (Ctrl+V); falls back to cat
pager_args = ["--paging", "always"]  # arguments passed before the file path

[ui]
side_panel_width_percent = 30
show_tags = true
show_dates = true
date_format = "%Y-%m-%d"
search_content = true

[sort]
default_mode = "modified"    # "modified", "created", or "title"
default_ascending = false

[colors]
tag_colors = ["cyan", "magenta", "yellow", "green", "red", "blue"]
```

### Themes

deez-notes ships with 6 built-in themes: **Terminal** (ANSI default), **Catppuccin**, **Monokai**, **Nord**, **Gruvbox**, and **Darcula**. Press `Ctrl+P` to open the theme selector.

You can define custom themes in `config.toml` using `[[themes]]` blocks. Colors accept named values (`cyan`, `red`, `green`, ...) or hex format (`#rrggbb`).

```toml
[[themes]]
name = "Sakura"
fg_primary = "#e8d5c4"
fg_secondary = "#7a6b63"
accent = "#f0a0b0"
highlight = "#f5c6aa"
success = "#b4d9a0"
error = "#e06070"
bg_main = "#1e1518"
bg_bar = "#2a2025"
bg_selection = "#f0a0b0"
fg_selection = "#1e1518"
tag_colors = ["#f0a0b0", "#b4d9a0", "#f5c6aa", "#a0c4e8", "#d0a0e0", "#e0c080"]
```

Custom themes appear alongside built-in themes in the `Ctrl+P` selector.

| Role | Description |
|------|-------------|
| `fg_primary` | Main text |
| `fg_secondary` | Muted text, dates, placeholders |
| `accent` | Borders, labels, headings (H1) |
| `highlight` | Status messages, H2, inline code |
| `success` | Confirmations, code blocks, H3 |
| `error` | Deletion / danger |
| `bg_main` | Main background |
| `bg_bar` | Status bar and search bar background |
| `bg_selection` | Selected item background in overlays |
| `fg_selection` | Text on selected items |

## Note format

Notes are standard Markdown files with optional YAML front matter:

```markdown
---
title: My Note
tags: [rust, tui]
created: 2026-03-11
---

# My Note

Content goes here...
```

## License

MIT
