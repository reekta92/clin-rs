<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />

# clin

**A TUI reimagination of [Obsidian](https://obsidian.md/) — feature-packed note management in your terminal.**

<div align="center">

[![Patreon](https://img.shields.io/badge/Patreon-Support%20clin-f96854.svg?logo=patreon&logoColor=white)](https://www.patreon.com/MehmetDag/posts/clin-rs-project-163784358)
[![Crates.io Downloads](https://img.shields.io/crates/d/clin-rs)](https://crates.io/crates/clin-rs)
</div>

[![CI](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml)
[![Release](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/dispatch-release.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![MSRV: 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-orange.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)
[![GitHub release](https://img.shields.io/github/v/release/reekta92/clin-rs.svg?logo=github)](https://github.com/reekta92/clin-rs/releases)
[![last commit](https://img.shields.io/github/last-commit/reekta92/clin-rs.svg)](https://github.com/reekta92/clin-rs/commits/main)


<a href="https://terminaltrove.com/clin" title="This tool is Tool of The Week on Terminal Trove, The $HOME of all things in the terminal"><img src="https://cdn.terminaltrove.com/media/badges/tool_of_the_week/svg/terminal_trove_tool_of_the_week_green_on_dark_grey_bg.svg" alt="Terminal Trove Tool of The Week" height="60" /></a>

</div>

## About
`clin` is a free, open-source terminal note manager inspired by Obsidian. It packs Obsidian's core features — markdown editing and rendering, `.canvas` files, and a force-directed graph view — into a roughly 2-5 MB Rust binary with minimal resource use, while keeping the UI approachable.

Drop an existing Obsidian vault into `clin` and it works out of the box. Image rendering, databases, and Obsidian plugins are not supported.

## Screenshots

<table>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/1de315ca-1585-4315-94b4-6e57c310b322" alt="Notes view" width="100%"><br><sub>Notes — grid layout with preview pane</sub></td>
    <td><img src="https://github.com/user-attachments/assets/a1f56629-9c40-4151-95b3-e35494106c43" alt="Graph view" width="100%"><br><sub>Graph — force-directed node visualization</sub></td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/ac972010-3a15-4faf-869a-11ea475e4ee0" alt="Canvas view" width="100%"><br><sub>Canvas — node/edge 2D canvas</sub></td>
    <td><img src="https://github.com/user-attachments/assets/ac3291f2-21a9-452a-b603-366f1d8bd3fe" alt="Editor view" width="100%"><br><sub>Editor — text editor with markdown preview</sub></td>
  </tr>
</table>

<details>
<summary><b>More screenshots</b></summary>
<table>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/aa67af28-be2a-472d-ac08-6d50e31dcd1f" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/b15238a8-f5ca-4ca0-b476-ad949b9e7add" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/db546168-5f4d-45ed-a0b4-0187b090edb2" alt="clin screenshot" width="100%"></td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/5e252087-d056-4adf-85ca-f0a322053ba3" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/6cd3f836-674a-4c55-99c2-b286bc5c4245" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/e8c123dd-cc31-432b-a9ba-cd50300d79d8" alt="clin screenshot" width="100%"></td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/d6ae988e-6016-4508-800f-ca534770e4d4" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/720ef7c4-0ccb-4de8-a320-aed5f5e717ab" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/fda03fa8-0c2d-420a-98a9-978dfec55bce" alt="clin screenshot" width="100%"></td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/a38e8cb8-7e85-447a-8389-e6ab2e5829c0" alt="clin screenshot" width="100%"></td>
    <td><img src="https://github.com/user-attachments/assets/7bc4eadc-72a1-4b6c-8f68-5e6cdb849956" alt="clin screenshot" width="100%"></td>
  </tr>
</table>
</details>

---

## Highlights

- **Notes view** — folder tree, tags, markdown preview pane (built-in renderer), search, filter, sort, pin, multi-select, trash management, file management (copy, paste, delete, rename, move), customizable bottom strip with activity heatmap, goals, and widget previews.
- **Editor view** — built-in text editor with mouse support, line numbers, undo/redo, and **external editor** integration (VISUAL/EDITOR env or config). Markdown preview pane alongside editor.
- **Graph view** — fully integrated force-directed graph visualization of your note corpus. Edges from `[[wikilinks]]`. Physics simulation, minimap, legend, search, configurable colors and layout. See [GRAPH_VIEW.md](docs/GRAPH_VIEW.md).
- **Canvas view** — Obsidian-compatible `.canvas` file format. Place text/file/link/group nodes on an infinite 2D canvas, connect them with edges. Right-click context menu, drag, resize, zoom. See [CANVAS.md](docs/CANVAS.md).
- **Draw view** — freehand drawing canvas with shapes (rect, ellipse, diamond, line, arrow), text, and eraser tool. `.draw` file format. See [DRAW.md](docs/DRAW.md).
- **Content tree view** — view to see the content of a `.md` file as a tree with headers being the parents and content being the children.
- **Git backup** — backup system using `git` as backend, initialize a repository and backup your notes automatically.
- **Command palette** (Ctrl+P) — extensible action system with encrypt/decrypt, theme switcher, OCR paste, canvas/draw creation, graph view. See [COMMAND_PALETTE.md](docs/COMMAND_PALETTE.md).
- **Theme system** — 19 built-in themes (default, TokyoNight, CatppuccinMocha, OneDark, Gruvbox, Dracula, Nord, RosePine, Everforest, Kanagawa, Solarized, Catppuccin Frappé, Catppuccin Macchiato, Rose Pine Moon, Gruvbox Material, GitHub Dark, Ayu Mirage, Synthwave '84, Material), transparent/solid backgrounds, per-color overrides. See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md).
- **Custom themes** — drop-in TOML themes in `~/.config/clin/themes/`, no recompile needed. See [THEME_SYSTEM.md](docs/THEME_SYSTEM.md).

- **Encryption** — on-demand ChaCha20-Poly1305 AEAD per-note encryption. `.clin` files with plaintext frontmatter for fast summary loading. See [ENCRYPTION.md](docs/ENCRYPTION.md).
- **Obsidian .canvas import** — existing Obsidian canvas files are read and rendered, **except for images**.
- **Templates** — TOML-based note templates with variable substitution (`{date}`, `{time}`, `{weekday}`, etc.). See [TEMPLATES.md](docs/TEMPLATES.md).
- **Goals system** — daily word-count and note-count goals with in-app progress bars. Configurable via `[goals]` config section and command palette.
- **Import & conversion** — import File/CSV/JSON/URL/Clipboard content as a new note or append to the current note. PDF, DOCX, HTML converted via external tools.

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

---

## Installation



| Platform | Method | Command |
|---|---|---|
| Any (from source) | Cargo build | `cargo build --release` |
| Any | Cargo install | `cargo install clin-rs` |
| Debian/Ubuntu (amd64) | `.deb` | `sudo dpkg -i clin-rs_*_amd64.deb` |
| Debian/Ubuntu (arm64) | `.deb` | `sudo dpkg -i clin-rs_*_arm64.deb` |
| Fedora/RHEL (x86_64) | `.rpm` | `sudo rpm -i clin-rs-*.x86_64.rpm` |
| Fedora/RHEL (aarch64) | `.rpm` | `sudo rpm -i clin-rs-*.aarch64.rpm` |
| Arch (local) | PKGBUILD | `makepkg -si` |
| Arch | AUR | `yay -S clin-rs-bin` |
| Nix | Flakes | `nix run github:reekta92/clin-rs` |
| Linux (portable x86_64) | AppImage | `./clin-*-x86_64.AppImage` |
| Linux (tar.gz x86_64) | binary | see details |
| Linux (tar.gz aarch64) | binary | see details |
| Windows | `.zip` | `Expand-Archive …` |
| macOS (Apple Silicon) | `.dmg`/`.tar.gz` | see details |

<details>
<summary><b>Source & Cargo</b> (cargo build + cargo install)</summary>

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

</details>

<details>
<summary><b>Debian / Ubuntu</b> (.deb, amd64 + arm64)</summary>

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo dpkg -i clin-rs_*_amd64.deb
```

### Debian/Ubuntu (aarch64)
Download the latest `.deb` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo dpkg -i clin-rs_*_arm64.deb
```

</details>

<details>
<summary><b>Fedora / RHEL</b> (.rpm, x86_64 + aarch64)</summary>

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-*.x86_64.rpm
```

### Fedora/RHEL (aarch64)
Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-*.aarch64.rpm
```

</details>

<details>
<summary><b>Arch Linux</b> (PKGBUILD + AUR)</summary>

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

</details>

<details>
<summary><b>Nix</b> (Flakes)</summary>

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

</details>

<details>
<summary><b>AppImage</b> (portable x86_64)</summary>

### AppImage
Download the latest `.AppImage` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
chmod +x clin-*-x86_64.AppImage
./clin-*-x86_64.AppImage
```

</details>

<details>
<summary><b>Linux</b> (tar.gz, x86_64 + aarch64)</summary>

### Other
Download the latest `.tar.gz` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
tar -xzf clin-rs-x86_64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

### Other (aarch64)
Download the latest `.tar.gz` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
tar -xzf clin-rs-aarch64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

</details>

<details>
<summary><b>Windows</b> (.zip)</summary>

### Windows
Download the latest `.zip` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```powershell
Expand-Archive -Path clin-rs-x86_64-pc-windows-msvc.zip -DestinationPath clin
cd clin
.\clin.exe
```

Windows builds are provided but TUI glyph rendering depends on your terminal. [Windows Terminal](https://apps.microsoft.com/detail/9n0dx20hk701) is recommended for the best experience.

</details>

<details>
<summary><b>macOS</b> (Apple Silicon)</summary>

### macOS (Apple Silicon)
Download the latest `.dmg` (or `.tar.gz`) from the [Releases](https://github.com/reekta92/clin-rs/releases) page.

```bash
# Using .dmg
hdiutil attach clin-rs-aarch64-apple-darwin.dmg
cp -r /Volumes/clin/clin.app /Applications/
hdiutil detach /Volumes/clin

# Or using .tar.gz
tar -xzf clin-rs-aarch64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

Apple Silicon only (no Intel build ships).

</details>

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
| **List / Notes** | Browse, search, filter, manage notes | Grid/Tree layout, format chooser, folders, tags, sort, pin, markdown preview, search, trash, copy/move/delete |
| **Editor** | Write and edit notes | Title + body, undo/redo, mouse support, line numbers, markdown preview pane, external editor |
| **Graph** | Visualize note connections | Force-directed layout, [[wikilinks]] edges, physics, preview pane, minimap, legend, search, grid, configurable colors |
| **Backup** | Git-based vault versioning | Status (staged/unstaged), commit history, diff preview, auto-push, remote sync |
| **Canvas** | Obsidian-compatible node/edge canvas | Text/file/link/group nodes, edges, drag/resize, context menu, raw JSON editor |
| **Draw** | Freehand drawing and shapes | Stroke, rect/ellipse/diamond/line/arrow, text, eraser, pan/zoom |
| **Content Tree** | Note outline and navigation | Header-based tree parsing, collapsible sections, jump-to-section |
| **Setup Wizard** | First-run onboarding / reopenable via palette | Theme/background/hint-bar/icon-mode/keybind-preset cycling with live markdown preview |

| Feature | Description |
|---|---|
| **Command Palette** (Ctrl+P) | Extensible action system: encrypt, decrypt, theme switch, OCR paste, create canvas/draw, open graph |
| **Encryption** | Per-note ChaCha20-Poly1305, `.clin` files, on-demand encrypt/decrypt, zero-knowledge |
| **Templates** | TOML-based with `{date}`, `{time}`, `{weekday}` variables |
| **Themes** | 19 built-in themes, transparent/solid backgrounds, per-color overrides, Nerd Font/Unicode/None icon modes, powerline/classic hint bar styles |
| **Goals** | Daily word-count and note-count goals with in-app progress bars, configurable via `[goals]` |
| **Import** | File/CSV/JSON/URL/Clipboard → new note or append to current; PDF/DOCX/HTML via external converters |
| **Keybinds** | Fully customizable via keybinds.toml, with Helix/Vim/Emacs presets |
| **OCR** | Clipboard image to text via `tesseract` (optional dependency) |

---

## Configuration

`~/.config/clin/config.toml` -> main configuration file (includes theme, graph settings, etc.)
`~/.config/clin/keybinds.toml` -> keybind configuration file

See the [full configuration reference](docs/CONFIG_REFERENCE.md) for all available options and [keybind presets](docs/KEYBIND_PRESETS.md) for editor-style presets.

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
calendar_enabled = true

[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = false
show_line_numbers = true

[ui]
theme = "tokyo_night"
background = "transparent"
show_status_bar = true
icon_mode = "nerd"
hint_bar_style = "classic"
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
clin notes new [OPTIONS] [TITLE]  Create a note (supports --body, --no-tui)
clin notes open <TITLE>           Open a note by title
clin notes cat <TITLE>            Print a note's body to stdout
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

clin config show                  Print the config file path
clin config edit                  Open config in $VISUAL or $EDITOR
clin config reset                 Reset config to default values

clin --version                    Print version
clin --config <PATH>              Override config file (global)
clin --vault <VAULT>              Override the storage/vault path (global)
clin --help                       Show help
```

---

## Documentation

Full technical documentation lives in [`docs/`](docs/INDEX.md):

- [Architecture](docs/ARCHITECTURE.md) — system overview, event loop, threading model
- [List View](docs/LIST_VIEW.md) — notes list view: Grid/Tree layout, format chooser, preview pane

- [Configuration Reference](docs/CONFIG_REFERENCE.md) — all config.toml and keybinds.toml options
- [Keybind Presets](docs/KEYBIND_PRESETS.md) — Helix, Vim, and Emacs presets and sequence syntax
- [Graph View](docs/GRAPH_VIEW.md) — force-directed graph visualization
- [Canvas](docs/CANVAS.md) — Obsidian-compatible canvas view
- [Backup](docs/BACKUP.md) — Git-based backup dashboard: vault state, history, diff preview, automation

- [Draw](docs/DRAW.md) — freehand drawing canvas
- [Content Tree](docs/CONTENT_TREE.md) — nested outline navigation
- [Encryption](docs/ENCRYPTION.md) — ChaCha20-Poly1305 per-note encryption
- [Theme System](docs/THEME_SYSTEM.md) — built-in themes and customization
- [Setup](docs/SETUP.md) — first-run setup wizard
- [Command Palette](docs/COMMAND_PALETTE.md) — extensible action system
- [Templates](docs/TEMPLATES.md) — TOML-based note templates

## Stats

<div align="center">
<img src="https://github-readme-activity-graph.vercel.app/graph?username=reekta92&theme=tokyo-night&area=true&hide_total_contributions=false" alt="Contribution activity" width="100%" />
</div>

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features and progress.

## Architecture

Full detail in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Support

`clin` is free and open-source. If it's useful to you, consider supporting its development on Patreon:

[![Patreon](https://img.shields.io/badge/Patreon-Support%20clin-f96854.svg?logo=patreon&logoColor=white)](https://www.patreon.com/MehmetDag/posts/clin-rs-project-163784358)

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

## Credits

Built with [Ratatui](https://ratatui.rs), [Crossterm](https://github.com/crossterm-rs/crossterm), and [fdg-sim](https://github.com/grantshandy/fdg). Markdown preview via built-in comrak/syntect renderer.
