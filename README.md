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
- Read-only viewer mode (uses [`mcat`](https://github.com/Skardyy/mcat) if available, falls back to `cat`)
- Keyboard-driven with `Ctrl+` shortcuts and `j`/`k` navigation
- Configurable via TOML (`~/.config/deez-notes/config.toml`)

## Installation

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
| `Ctrl+R` | Refresh note list |
| `Ctrl+Q` | Quit |
| `F1` / `Ctrl+?` | Help |

## Configuration

Create `~/.config/deez-notes/config.toml`:

```toml
[general]
notes_dir = "~/notes"
editor = "nvim"

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
