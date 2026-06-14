<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />

# clin

**A TUI reimagination of Obsidian — encrypted note management in your terminal.**

[![CI](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml)
[![Release](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![MSRV: 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-orange.svg)](https://blog.rust-lang.org/2024/09/19/Rust-1.81.0.html)

</div>

> `clin` was originally an app I made when I got into C. It was really rough and basic, so I decided to remake it in Rust with more features and an improved user experience to better fit your workflow!

---

## Screenshots

<!-- Add screenshots/GIFs here. Recommended:
     - Main list view with preview pane
     - Graph view
     - Canvas view
     - Editor view with markdown preview
     Place images in assets/ and reference as assets/screenshot-list.png -->

> 📸 *Screenshots coming soon — see [Highlights](#highlights) for feature descriptions.*

---

## Highlights

- **Notes view** — folder tree, tags, markdown preview pane (via `glow`), search, filter, sort, pin, multi-select, trash management, file management (copy, paste, delete, rename, move).
- **Editor view** — built-in text editor with mouse support, line numbers, undo/redo, and **external editor** integration (VISUAL/EDITOR env or config). Markdown preview pane alongside editor.
- **Graph view** — fully integrated force-directed graph visualization of your note corpus. Edges from `[[wikilinks]]`. Physics simulation, minimap, legend, search, configurable colors and layout. See [GRAPH_VIEW.md](docs/GRAPH_VIEW.md).
- **Canvas view** — Obsidian-compatible `.canvas` file format. Place text/file/link/group nodes on an infinite 2D canvas, connect them with edges. Right-click context menu, drag, resize, zoom. See [CANVAS.md](docs/CANVAS.md).
- **Draw view** — freehand drawing canvas with shapes (rect, ellipse, diamond, line, arrow), text, and eraser tool. `.draw` file format. See [DRAW.md](docs/DRAW.md).
- **Command palette** (Ctrl+P) — extensible action system with encrypt/decrypt, theme switcher, OCR paste, canvas/draw creation, graph view. See [COMMAND_PALETTE.md](docs/COMMAND_PALETTE.md).
- **Theme system** — 11 built-in themes (TokyoNight, CatppuccinMocha, OneDark, Gruvbox, Dracula, Nord, RosePine, Everforest, Kanagawa, Solarized), transparent/solid backgrounds, per-color overrides. See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md).
- **Encryption** — on-demand ChaCha20-Poly1305 AEAD per-note encryption. `.clin` files with plaintext frontmatter for fast summary loading. See [ENCRYPTION.md](docs/ENCRYPTION.md).
- **Obsidian .canvas import** — existing Obsidian canvas files are read and rendered.
- **Templates** — TOML-based note templates with variable substitution (`{date}`, `{time}`, `{weekday}`, etc.). See [TEMPLATES.md](docs/TEMPLATES.md).

---

## Installation

### From Source
```bash
git clone https://github.com/reekta92/clin-rs.git
cd clin-rs
cargo build --release
```

### With Cargo
```bash
cargo install clin-rs
```

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo dpkg -i clin-rs_0.8.7-1_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-0.8.7-1.x86_64.rpm
```

### Arch Linux (PKGBUILD)
A `PKGBUILD` is included in the root of the repository.
```bash
git clone https://github.com/reekta92/clin-rs.git
cd clin-rs
makepkg -si
```

### Arch Linux (AUR)
```bash
yay -S clin-rs-bin
```

### AppImage
Download the latest `.AppImage` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
chmod +x clin-*-x86_64.AppImage
./clin-*-x86_64.AppImage
```

### Other
Download the latest `.tar.gz` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
tar -xzf clin-rs-x86_64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

> **Rust not installed?** Run `curl https://sh.rustup.rs -sSf | sh` to install Rust.

---

## Quick Start

```bash
# Launch the interactive TUI
clin

# Create a quick note
clin -q "Meeting notes from today" "standup-2026-06-13"

# Create a note from a template
clin -n --template diary

# Open a specific note
clin -e "my-note"
```

Once inside the TUI: navigate with `j`/`k`, open notes with `Enter`, open the command palette with `Ctrl+P`, and view the graph with `Ctrl+G`. Press `?` for the full keybind reference.

---

## Features

| View | Purpose | Key Actions |
|---|---|---|
| **List / Notes** | Browse, search, filter, manage notes | Folders, tags, sort, pin, glow preview, search, trash, copy/move/delete |
| **Editor** | Write and edit notes | Title + body, undo/redo, mouse support, line numbers, markdown preview pane, external editor |
| **Graph** | Visualize note connections | Force-directed layout, [[wikilinks]] edges, physics, minimap, legend, search, grid, configurable colors |
| **Canvas** | Obsidian-compatible node/edge canvas | Text/file/link/group nodes, edges, drag/resize, context menu, raw JSON editor |
| **Draw** | Freehand drawing and shapes | Stroke, rect/ellipse/diamond/line/arrow, text, eraser, pan/zoom |

| Feature | Description |
|---|---|
| **Command Palette** (Ctrl+P) | Extensible action system: encrypt, decrypt, theme switch, OCR paste, create canvas/draw, open graph |
| **Encryption** | Per-note ChaCha20-Poly1305, `.clin` files, on-demand encrypt/decrypt, zero-knowledge |
| **Templates** | TOML-based with `{date}`, `{time}`, `{weekday}` variables |
| **Themes** | 11 built-in themes, transparent/solid backgrounds, per-color overrides |
| **Keybinds** | Fully customizable via keybinds.toml |
| **OCR** | Clipboard image to text via `tesseract` (optional dependency) |

---

## Configuration

`~/.config/clin/config.toml` -> main configuration file (includes theme, graf settings, etc.)
`~/.config/clin/keybinds.toml` -> keybind configuration file

See the [full configuration reference](docs/CONFIG_REFERENCE.md) for all available options.

### config.toml example

```toml
# Custom vault storage path (default: ~/.local/share/clin)
storage_path = "/path/to/your/vault"

# External editor command (e.g. "nvim", "code", "nano")
external_editor = "nvim"
external_editor_enabled = false

# Show the preview pane by default
preview_enabled = true

# Show markdown preview in editor by default
editor_preview_enabled = false

# Show line numbers
show_line_numbers = true

# Confirm before deleting
confirm_on_delete = true

# Default sort
# default_sort_field = "title"   # "title" or "modified"
# default_sort_order = "ascending"  # "ascending" or "descending"

[theme]
theme = "tokyo_night"
background = "transparent"
# accent = "#ff6600"
# background_color = "#1a1a2e"
```

See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md) for theme options and [CONFIG_REFERENCE.md](docs/CONFIG_REFERENCE.md) for all graf sections.

### keybinds.toml example

See the [full keybinds reference](docs/CONFIG_REFERENCE.md) for all available actions and defaults.

```toml
[list]
move_up = ["Up", "k"]
move_down = ["Down", "j"]
open = ["Enter"]
delete = ["d", "Delete"]
quit = ["q"]
help = ["?", "F1"]
open_command_palette = ["Ctrl+p", "Shift+Enter"]
# ... see CONFIG_REFERENCE.md for full list

[edit]
back = ["Esc"]
cycle_focus = ["Tab"]
copy = ["Ctrl+c", "Ctrl+Insert"]
# ... see CONFIG_REFERENCE.md

[graph]
quit = ["Esc"]
pan_up = ["Up", "k"]
# ... see CONFIG_REFERENCE.md
```

---

## CLI Commands

```
NOTE OPERATIONS:
  clin                        Launch interactive app
  -n [TITLE]                Create a new note and open it
  -n -t, --template <NAME> [TITLE]
                              Create a new note from a template
  -q <CONTENT> [TITLE]      Create a quick note and exit
  -e <TITLE>                Open a specific note by title
  -l                        List note titles
  -h, --help                Show this help message

CONFIGURATION:
  --storage-path            Show current storage path
  --set-storage-path <PATH> Set custom storage path
  --reset-storage-path      Reset to default storage path
  --migrate-storage         Migrate data from previous storage location

KEYBINDS:
  --keybinds                Show current keybindings
  --export-keybinds         Export keybinds as TOML
  --reset-keybinds          Reset keybinds to defaults

TEMPLATES:
  --list-templates          List available templates
  --create-example-templates Create example templates

```

---

## Prerequisites (Optional)

These tools are **optional** — clin works without them:

| Tool | Purpose | Package |
|---|---|---|
| `tesseract` | OCR paste (clipboard image → text) | `tesseract-ocr` |
| `wl-clipboard` | Clipboard access (Wayland) | `wl-clipboard` |
| `xclip` or `xsel` | Clipboard access (X11) | `xclip` |
| `glow` | Markdown preview rendering | `glow` |

---

## Documentation

Full technical documentation lives in [`docs/`](docs/INDEX.md):

- [Architecture](docs/ARCHITECTURE.md) — system overview, event loop, threading model
- [Configuration Reference](docs/CONFIG_REFERENCE.md) — all config.toml and keybinds.toml options
- [Graph View](docs/GRAPH_VIEW.md) — force-directed graph visualization
- [Canvas](docs/CANVAS.md) — Obsidian-compatible canvas view
- [Draw](docs/DRAW.md) — freehand drawing canvas
- [Content Tree](docs/CONTENT_TREE.md) — nested outline navigation
- [Encryption](docs/ENCRYPTION.md) — ChaCha20-Poly1305 per-note encryption
- [Theme System](docs/THEME_SYSTEM.md) — built-in themes and customization
- [Command Palette](docs/COMMAND_PALETTE.md) — extensible action system
- [Templates](docs/TEMPLATES.md) — TOML-based note templates

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features and progress.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

## Credits

Built with [Ratatui](https://ratatui.rs), [Crossterm](https://github.com/crossterm-rs/crossterm), and [fdg-sim](https://github.com/grantshandy/fdg). Markdown preview powered by [glow](https://github.com/charmbracelet/glow).
