<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>  

# ****clin is not a text editor!****

> `clin` was originally an app I made when I got into C. It was really rough and basic, so I decided to remake it in Rust with more features and an improved user experience to better fit your workflow!

---

`clin` is a TUI reimagination of Obsidian. Its goal is to provide a feature-complete note management tool like Obsidian, but as a TUI rather than a GUI.

---

## Highlights

- **Notes view** — folder tree, tags, markdown preview pane (via `glow`), search, filter, sort, pin, file management (copy, paste, delete, rename, move).
- **Editor view** — built-in text editor with mouse support, line numbers, undo/redo, and **external editor** integration (VISUAL/EDITOR env or config). Markdown preview pane alongside editor.
- **Graph view** — fully integrated force-directed graph visualization of your note corpus. Edges from `[[wikilinks]]`. Physics simulation, minimap, legend, search, configurable colors and layout. See [GRAPH_VIEW.md](docs/GRAPH_VIEW.md).
- **Canvas view** — Obsidian-compatible `.canvas` file format. Place text/file/link/group nodes on an infinite 2D canvas, connect them with edges. Right-click context menu, drag, resize, zoom. See [CANVAS.md](docs/CANVAS.md).
- **Draw view** — freehand drawing canvas with shapes (rect, ellipse, diamond, line, arrow), text, and eraser tool. `.draw` file format. See [DRAW.md](docs/DRAW.md).
- **Command palette** (Ctrl+P) — extensible action system with encrypt/decrypt, theme switcher, OCR paste, canvas/draw creation, graph view. See [COMMAND_PALETTE.md](docs/COMMAND_PALETTE.md).
- **Theme system** — 11 built-in themes (TokyoNight, CatppuccinMocha, OneDark, Gruvbox, Dracula, Nord, RosePine, Everforest, Kanagawa, Solarized), transparent/solid backgrounds, per-color overrides. See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md).
- **Encryption** — on-demand ChaCha20-Poly1305 AEAD per-note encryption. `.clin` files with plaintext frontmatter for fast summary loading. See [ENCRYPTION.md](docs/ENCRYPTION.md).
- **Obsidian .canvas import** — existing Obsidian canvas files are read and rendered.
- **Templates** — TOML-based note templates with variable substitution (`{date}`, `{time}`, `{weekday}`, etc.). See [TEMPLATES.md](docs/TEMPLATES.md).

## Roadmap

### Completed
- [X] **Theme system** — 11 built-in themes, backgrounds, per-color overrides, theme switcher
- [X] **Trash** — move notes/folders to trash, restore, empty trash
- [X] **OCR paste** — clipboard image → OCR text (`tesseract`) via command palette
- [X] **Canvas view (pinstar)** — Obsidian-compatible `.canvas` files, 4 node types, edges, context menu
- [X] **Draw view** — freehand drawing, shapes, text, `.draw` file format
- [X] **Obsidian .canvas import** — read and display existing Obsidian canvas files
- [X] **Command palette** — extensible action system with search
- [X] **Encryption** — on-demand ChaCha20-Poly1305, `.clin` files
- [X] **Templates** — TOML-based with variable substitution
- [X] **Markdown preview** — `glow`-based rendering in list preview and editor split pane
- [X] **External editor** — VISUAL/EDITOR env or configured command
- [X] **Folder management** — create, rename, move, collapse/expand
- [X] **Tag management** — add, remove, filter by tags
- [X] **Sorting & pinning** — sort by title/modified, pin notes to top
- [X] **Custom keybinds** — fully rebindable via keybinds.toml
- [X] **Graph view full integration** — `graf` is no longer external; physics, minimap, legend, search, config

### In Progress / Future

#### Notes View
- [ ] **Text search** — search note content via `grep`/`ripgrep`
- [ ] **Smart folders** — auto-move tagged notes to specific folders
- [ ] **Word & character metrics** — writing statistics and goals
- [ ] **Batch tagging** — tag multiple notes at once

#### Editor
- [ ] **Rework as side panel** — replace editor view with a feature-rich side panel
- [ ] **Cursor insert** — insert content at cursor from command palette actions

#### Graph View
- [ ] **Date/time linking** — categorize nodes by note date
- [ ] **Create links** — create/remove wikilinks from graph view
- [ ] **Assign tags** — tag notes directly from graph
- [ ] **Right-click menu** — context actions on nodes

#### Canvas
- [ ] **Link objects** — connect objects with lines
- [ ] **Grouping** — merge objects into groups
- [ ] **Insert note links** — embed note references as objects

#### Command Palette
- [ ] **PDF import/export** — convert PDFs to/from markdown
- [ ] **CSV to markdown** — import CSV tables
- [ ] **URL import** — fetch article content as formatted markdown
- [ ] **Sub-notes** — virtual encrypted notes attached to physical notes
- [ ] **Merge/split notes** — combine or divide notes
- [ ] **Advanced clipboard** — multi-selection copy/paste
- [ ] **Dynamic variables** — insert realtime values
- [ ] **Word frequency** — show most used words

#### Configuration
- [ ] **Status line customization** — `status_format = "{title} | {word_count} words"`
- [ ] **Plugin support** — Lua scripting

#### Other
- [ ] **Git integration** — vault versioning and backup
- [ ] **Tree outline** — note hierarchy from headers
- [ ] **Calendar/time tools** — date calculator, timezone converter
- [ ] **AOD pinning** — overlay note on screen


---
## Configuration

`~/.config/clin/config.toml` -> main configuration file (includes theme, graf settings, etc.)
`~/.config/clin/keybinds.toml` -> keybind configuration file

> **Note:** The old `~/.config/clin/graf.toml` file is legacy. All graf settings (`[visual]`, `[physics]`, `[interaction]`, `[display]`, `[filter]`, `[legend]`, `[search]`, `[editor]`) are now part of `config.toml`. The system auto-migrates `graf.toml` values on first read.

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

## Prerequisites (Optional)

These tools are **optional** — clin works without them:

| Tool | Purpose | Package |
|---|---|---|
| `tesseract` | OCR paste (clipboard image → text) | `tesseract-ocr` |
| `wl-clipboard` | Clipboard access (Wayland) | `wl-clipboard` |
| `xclip` or `xsel` | Clipboard access (X11) | `xclip` |
| `glow` | Markdown preview rendering | `glow` |

## Installation

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo dpkg -i clin-rs_latest_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo rpm -i clin-rs-latest.x86_64.rpm
```

### Arch Linux (PKGBUILD)
A `PKGBUILD` is included in the root of the repository.
```bash
git clone https://github.com/reekta/clin-rs.git
cd clin
makepkg -si
```

### Other
Download the latest `.tar.gz` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
tar -xzf clin-rs-latest-x86_64.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

### From Source
```bash
cargo run
```

### With Cargo
```bash
cargo install clin-rs
```

> **Rust not installed?** Run `curl https://sh.rustup.rs -sSf | sh` to install Rust.


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
