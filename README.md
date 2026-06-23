<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />

# clin
 
**A TUI reimagination of [Obsidian](https://obsidian.md/) — feature-packed note management in your terminal.**

[![CI](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml)
[![Release](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![MSRV: 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-orange.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)

<a href="https://terminaltrove.com/clin" title="This tool is Tool of The Week on Terminal Trove, The $HOME of all things in the terminal"><img src="https://cdn.terminaltrove.com/media/badges/tool_of_the_week/svg/terminal_trove_tool_of_the_week_green_on_dark_grey_bg.svg" alt="Terminal Trove Tool of The Week" height="60" /></a>

</div>

> `clin` was originally an app I made when I got into C. It was really rough and basic, so I decided to remake it in Rust with more features and an improved user experience to better fit your workflow!

---

## About
`clin` is a FOSS, minimal(**around 2-5~ megabytes of binary**) and TUI alternative for the note management app Obsidian. It mainly provides the biggest features of Obsidian(markdown editing and rendering, `.canvas` files, graph view etc.) in a extremely compact package and really minimal resource usage, written in Rust at a TUI environment. It's goal to provide you a feature-packed note management app right at your terminal while keeping the **UI/UX easy and accessible**. It's 90% compatible with Obsidian **you can literally copy-paste your Obsidian vault to the `clin` vault and it just works without a problem**, **unsupported** stuff are **image rendering**, **databases**, **Obsidian plugins** and some small features here and there.

## Screenshots

<table>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/1de315ca-1585-4315-94b4-6e57c310b322" alt="Screenshot 1" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/aa67af28-be2a-472d-ac08-6d50e31dcd1f" alt="Screenshot 2" width="100%"></td>
       <td><img width="1888" height="1006" alt="image" src="https://github.com/user-attachments/assets/b15238a8-f5ca-4ca0-b476-ad949b9e7add" />
</td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/a1f56629-9c40-4151-95b3-e35494106c43" alt="Screenshot 3" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/db546168-5f4d-45ed-a0b4-0187b090edb2" alt="Screenshot 4" width="100%"></td>
    <td><img width="1888" height="1006" alt="image" src="https://github.com/user-attachments/assets/5e252087-d056-4adf-85ca-f0a322053ba3" />
</td>
       
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/ac972010-3a15-4faf-869a-11ea475e4ee0" alt="Screenshot 5" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/6cd3f836-674a-4c55-99c2-b286bc5c4245" alt="Screenshot 6" width="100%"></td>
       <td><img width="1888" height="1006" alt="image" src="https://github.com/user-attachments/assets/e8c123dd-cc31-432b-a9ba-cd50300d79d8" />
</td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/ac3291f2-21a9-452a-b603-366f1d8bd3fe" alt="Screenshot 7" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/d6ae988e-6016-4508-800f-ca534770e4d4" alt="Screenshot 8" width="100%"></td>
    <td><img width="1888" height="1006" alt="image" src="https://github.com/user-attachments/assets/720ef7c4-0ccb-4de8-a320-aed5f5e717ab" />
</td>
       
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/fda03fa8-0c2d-420a-98a9-978dfec55bce" alt="Screenshot 9" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/a38e8cb8-7e85-447a-8389-e6ab2e5829c0" alt="Screenshot 10" width="100%"></td>
       <td><img width="1888" height="1006" alt="image" src="https://github.com/user-attachments/assets/7bc4eadc-72a1-4b6c-8f68-5e6cdb849956" />
</td>
  </tr>
</table>

---

## Highlights

- **Notes view** — folder tree, tags, markdown preview pane (via `glow`), search, filter, sort, pin, multi-select, trash management, file management (copy, paste, delete, rename, move).
- **Editor view** — built-in text editor with mouse support, line numbers, undo/redo, and **external editor** integration (VISUAL/EDITOR env or config). Markdown preview pane alongside editor.
- **Graph view** — fully integrated force-directed graph visualization of your note corpus. Edges from `[[wikilinks]]`. Physics simulation, minimap, legend, search, configurable colors and layout. See [GRAPH_VIEW.md](docs/GRAPH_VIEW.md).
- **Canvas view** — Obsidian-compatible `.canvas` file format. Place text/file/link/group nodes on an infinite 2D canvas, connect them with edges. Right-click context menu, drag, resize, zoom. See [CANVAS.md](docs/CANVAS.md).
- **Draw view** — freehand drawing canvas with shapes (rect, ellipse, diamond, line, arrow), text, and eraser tool. `.draw` file format. See [DRAW.md](docs/DRAW.md).
- **Content tree view** — view to see the content of a `.md` file as a tree with headers being the parents and content being the children.
- **Git backup** — backup system using `git` as backend, initialize a repository and backup your notes automatically.
- **Command palette** (Ctrl+P) — extensible action system with encrypt/decrypt, theme switcher, OCR paste, canvas/draw creation, graph view. See [COMMAND_PALETTE.md](docs/COMMAND_PALETTE.md).
- **Theme system** — 11 built-in themes (TokyoNight, CatppuccinMocha, OneDark, Gruvbox, Dracula, Nord, RosePine, Everforest, Kanagawa, Solarized), transparent/solid backgrounds, per-color overrides. See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md).
- **Encryption** — on-demand ChaCha20-Poly1305 AEAD per-note encryption. `.clin` files with plaintext frontmatter for fast summary loading. See [ENCRYPTION.md](docs/ENCRYPTION.md).
- **Obsidian .canvas import** — existing Obsidian canvas files are read and rendered, **except for images**.
- **Templates** — TOML-based note templates with variable substitution (`{date}`, `{time}`, `{weekday}`, etc.). See [TEMPLATES.md](docs/TEMPLATES.md).

---

## Dependencies
These are highly recommended for the best experience:
- **A nerd font** — required for rendering **glyphs** which are highly used around the UI.
- **A modern terminal** — such as `kitty`, `ghostty` etc.

---

## Optional Tools

These tools are **optional** — clin works without them:

| Tool | Purpose | Package |
|---|---|---|
| `tesseract` | OCR paste (clipboard image → text) | `tesseract-ocr` |
| `wl-clipboard` | Clipboard access (Wayland) | `wl-clipboard` |
| `xclip` or `xsel` | Clipboard access (X11) | `xclip` |
| `glow` | Markdown preview rendering | `glow` |

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
sudo dpkg -i clin-rs_*_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-*.x86_64.rpm
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

### Nix (Flakes)
*Note: Requires `nix-command` and `flakes` experimental features enabled.*

Run directly:
```bash
nix run github:reekta92/clin-rs --extra-experimental-features "nix-command flakes"
```
Install to profile:
```bash
nix profile install github:reekta92/clin-rs
```
Add to your flake `inputs`:
```nix
{
  inputs.clin.url = "github:reekta92/clin-rs";
}
```
Then include `inputs.clin.packages.${system}.default` in your `environment.systemPackages` or `home.packages`.

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
clin notes quick "Meeting notes from today" "standup-2026-06-13"

# Create a note from a template
clin notes new --template diary

# Open a specific note
clin notes open "my-note"
```

Once inside the TUI: navigate with `j`/`k`, open notes with `Enter`, open the command palette with `Ctrl+P`, and view the graph with `Ctrl+G`. Press `?` for the full keybind reference.

---

## Features

| View | Purpose | Key Actions |
|---|---|---|
| **List / Notes** | Browse, search, filter, manage notes | Grid/Tree layout, format chooser, folders, tags, sort, pin, glow preview, search, trash, copy/move/delete |
| **Editor** | Write and edit notes | Title + body, undo/redo, mouse support, line numbers, markdown preview pane, external editor |
| **Graph** | Visualize note connections | Force-directed layout, [[wikilinks]] edges, physics, preview pane, minimap, legend, search, grid, configurable colors |
| **Backup** | Git-based vault versioning | Status (staged/unstaged), commit history, diff preview, auto-push, remote sync |
| **Canvas** | Obsidian-compatible node/edge canvas | Text/file/link/group nodes, edges, drag/resize, context menu, raw JSON editor |
| **Draw** | Freehand drawing and shapes | Stroke, rect/ellipse/diamond/line/arrow, text, eraser, pan/zoom |
| **Content Tree** | Note outline and navigation | Header-based tree parsing, collapsible sections, jump-to-section |

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
# General settings
storage_path = "/path/to/your/vault"
mouse_enabled = true
confirm_on_delete = true
confirm_on_quit = false

[list]
preview_enabled = true
preview_position = "right"
preview_encryption = false
show_date_in_list = true
show_file_size = false
date_format = "%Y-%m-%d"
density = "comfortable"
default_view = "grid"
default_sort_field = "title"
default_sort_order = "ascending"
pinned_on_top = true

[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = false
show_line_numbers = true

[theme]
theme = "tokyo_night"
background = "transparent"
# accent = "#ff6600"
```

See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md) for theme options and [CONFIG_REFERENCE.md](docs/CONFIG_REFERENCE.md) for all options and sections.

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
clin                              Launch interactive TUI (default)
clin notes list                   List note titles
clin notes new [-t,--template N] [TITLE]   Create a note and open it
clin notes open <TITLE>           Open a note by title
clin notes quick <CONTENT> [TITLE] Quick note, then exit
clin notes search <QUERY>         Search notes by title and content

clin storage show                 Show storage path
clin storage set <PATH>           Set a custom (absolute) storage path
clin storage reset                Reset to default storage path
clin storage migrate              Migrate data from a previous location

clin keybinds show                Show keybindings
clin keybinds export              Export keybinds as TOML
clin keybinds reset               Reset keybinds to defaults

clin templates list               List templates
clin templates init               Create example templates

clin config show                  Print effective config (TOML)
clin config path                  Print config file path
clin config edit                  Open config in $EDITOR
clin config reset                 Reset config to default values

clin completion <SHELL>           Generate shell completions
clin --version                    Print version
clin --config <PATH>              Override config file (global)
clin --help                       Show help
```

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
