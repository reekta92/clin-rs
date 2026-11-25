# clin-tauri

A native desktop companion for [clin-rs](https://github.com/reekta92/clin-rs) — a Tauri v2 app that wraps the clin web interface and exposes a fully-featured GUI for your Markdown vault.

## Modes

clin-tauri runs in two modes:

| Mode | Command | What it does |
|------|---------|--------------|
| **Tauri** (default) | `clin-tauri` | Opens a native OS window (Tauri) |
| **Web** | `clin-tauri --web` | Starts a local HTTP server at `127.0.0.1:7778` and opens it in your default browser — no native window needed |

Both modes serve the same UI and read the same vault. The web mode is useful for headless servers, CI preview, or users who prefer a browser.

## Features

- **Graph view** — force-directed wikilink graph, physics simulation, local graph, focus mode (`f`)
- **Notes list** — sortable table, smart folders (Today / This Week / Pinned / Untagged / Broken Links)
- **Calendar** — month and agenda views, CalDAV sync
- **Bases** — table, kanban, and card views from `.base` files with filter
- **Canvas viewer** — read-only SVG render of `.canvas` files (Obsidian-compatible)
- **Inline editor** — source + rendered preview, wikilink autocomplete (`[[`), `Ctrl+S` to save
- **File management** — create, rename (with wikilink rewrite), move, delete, duplicate files and folders
- **Properties pane** — YAML frontmatter editor
- **Outline, Subnotes, Vault Health** side panels
- **Command palette** (`Ctrl+K`) with 17 commands
- **Full-text search** across vault
- **19 built-in themes** + live theme switching
- **Vault switcher** — switch between multiple vaults at runtime
- **Zen / focus mode** overlay
- **History navigation** (`Alt+←`/`Alt+→`)
- **Copy Markdown** to clipboard (⎘ in preview)
- **Export note** to HTML (↓ in preview)

### Tauri-native features (desktop only, not in web mode)

- **System tray icon** — left-click toggles window, right-click menu: show / hide / quit
- **Global shortcut** `Ctrl+Shift+C` — show/hide window from anywhere on the desktop
- **Native folder picker** — OS folder dialog when switching vault
- **OS notifications** — CalDAV sync results and errors
- **Clipboard plugin** — native clipboard write for "Copy Markdown"
- **Window state persistence** — size, position, and maximized state survive restarts

## Architecture

```
clin TUI ←→ shared vault ←→ clin-tauri GUI
```

Both tools read and write the same vault directory. All API calls inside the UI are routed through a `window.clinApi()` adapter that maps to Tauri `invoke()` IPC in native mode and plain `fetch()` in web/browser mode — the same HTML works in both.

## Install

### Quick install (Linux / macOS)

```bash
# Default: native Tauri window + app menu shortcut
curl -fsSL https://raw.githubusercontent.com/reekta92/clin-rs/main/install.sh | bash

# Web mode only (no native window, opens browser)
bash install.sh --mode=web

# Both shortcuts installed
bash install.sh --mode=both

# Also add a Desktop shortcut
bash install.sh --desktop

# Options
bash install.sh --help
```

### Quick install (Windows — PowerShell)

```powershell
# Default: native Tauri window + Start Menu shortcut
powershell -ExecutionPolicy Bypass -File install.ps1

# Web mode only
powershell -ExecutionPolicy Bypass -File install.ps1 -Mode Web

# Both shortcuts
powershell -ExecutionPolicy Bypass -File install.ps1 -Mode Both

# Also add Desktop shortcuts
powershell -ExecutionPolicy Bypass -File install.ps1 -Mode Both -Desktop
```

### Shortcuts created

| OS | Native shortcut | Web shortcut |
|----|----------------|-------------|
| Linux | `~/.local/share/applications/clin-tauri.desktop` | `clin-tauri-web.desktop` |
| macOS | `/Applications/clin-tauri.app` | — (run `clin-tauri --web` from terminal) |
| Windows (MSI) | Start Menu via MSI installer | `clin (web).lnk` in Start Menu |
| Windows (ZIP) | `clin.lnk` via WScript.Shell | `clin (web).lnk` via WScript.Shell |

Desktop shortcuts are opt-in (`--desktop` / `-Desktop`).

## Pre-compiled binaries

Pre-built binaries are published to [GitHub Releases](https://github.com/reekta92/clin-rs/releases) for every `v*` tag.
The install scripts (`install.sh` / `install.ps1`) download and install the right one automatically.

| Platform | Architecture | Artifact |
|----------|-------------|----------|
| Linux | x86_64 | `clin-tauri_*_linux-x86_64.AppImage` |
| Linux | aarch64 | `clin-tauri_*_linux-aarch64.AppImage` |
| Linux | x86_64 | `clin-tauri_*_amd64.deb` |
| Linux | aarch64 | `clin-tauri_*_arm64.deb` |
| macOS | x86_64 | `clin-tauri_*_macos-x86_64.dmg` |
| macOS | aarch64 (Apple Silicon) | `clin-tauri_*_macos-aarch64.dmg` |
| Windows | x86_64 | `clin-tauri_*_windows-x86_64.msi` + `.exe` |
| Windows | aarch64 | `clin-tauri_*_windows-aarch64.msi` + `.exe` |

## Build from source

### System requirements

| OS | Requirements |
|----|-------------|
| Linux | Rust 1.75+, Node 18+, system WebView libs (see below) |
| macOS | Rust 1.75+, Node 18+, Xcode Command Line Tools |
| Windows | Rust 1.75+, Node 18+, [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) |

**Linux — install system dependencies (Ubuntu/Debian):**
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libglib2.0-dev libssl-dev libgtk-3-dev \
  libappindicator3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

### Steps

```bash
git clone https://github.com/reekta92/clin-rs
cd clin-tauri
npm install
npm run tauri dev          # dev window with hot reload
npm run tauri build        # release binary → src-tauri/target/release/clin-tauri
```

**Cross-compile Linux aarch64 from x86_64:**
```bash
sudo apt-get install -y gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
npx tauri build --target aarch64-unknown-linux-gnu
```

**Release all 6 targets via CI (push a version tag):**
```bash
git tag v1.0.0 && git push origin v1.0.0
# GitHub Actions builds all targets and publishes a GitHub Release automatically
```

**Web mode only (no WebView required, any OS):**
```bash
npm run tauri build
./src-tauri/target/release/clin-tauri --web
# Opens http://127.0.0.1:7778 in your browser
```

Set `CLIN_VAULT=/path/to/vault` to override the default (`~/vault`).  
Set `CLIN_WEB_PORT=8080` to change the web mode port.

## CI / Releases

`.github/workflows/build.yml` builds for 6 targets on every push to `main` and every PR:

| Job | Runner | Target |
|-----|--------|--------|
| linux-x86_64 | ubuntu-22.04 | `x86_64-unknown-linux-gnu` |
| linux-aarch64 | ubuntu-22.04 (cross) | `aarch64-unknown-linux-gnu` |
| windows-x86_64 | windows-latest | `x86_64-pc-windows-msvc` |
| windows-aarch64 | windows-latest | `aarch64-pc-windows-msvc` |
| macos-x86_64 | macos-latest | `x86_64-apple-darwin` |
| macos-aarch64 | macos-latest | `aarch64-apple-darwin` |

`.github/workflows/release.yml` runs on `v*` tags and publishes a GitHub Release with all artifacts attached.

## Tauri IPC commands

35 Rust commands in `src-tauri/src/commands.rs` mirror the HTTP API:

| Command | HTTP equivalent |
|---------|----------------|
| `get_graph` | `GET /graph.json` |
| `get_tree` | `GET /tree` |
| `get_file` | `GET /file?path=` |
| `get_notes` | `GET /notes` |
| `get_pinned_notes` | `GET /notes/pinned` |
| `get_calendar` | `GET /calendar` |
| `get_properties` | `GET /properties?path=` |
| `get_outline` | `GET /outline?path=` |
| `get_subnotes` | `GET /subnotes?path=` |
| `get_bases` | `GET /bases` |
| `get_base` | `GET /base?path=` |
| `get_canvas_files` | `GET /canvas-files` |
| `get_canvas` | `GET /canvas?path=` |
| `get_themes` | `GET /themes` |
| `get_vaults` | `GET /vaults` |
| `get_templates` | `GET /templates` |
| `search_notes` | `GET /search?q=` |
| `get_smart_folder` | `GET /smart-folder?name=` |
| `open_file` | `POST /file/open` |
| `save_file` | `POST /file/save` |
| `create_file` | `POST /file/create` |
| `delete_file` | `POST /file/delete` |
| `delete_folder` | `POST /folder/delete` |
| `duplicate_file` | `POST /file/duplicate` |
| `rename_file` | `POST /rename` |
| `move_file` | `POST /file/move` |
| `create_folder` | `POST /folder/create` |
| `create_link` | `POST /link/create` |
| `delete_link` | `POST /link/delete` |
| `pin_note` | `POST /note/pin` |
| `save_properties` | `POST /properties/save` |
| `set_theme` | `POST /settings/theme` |
| `set_vault` | `POST /settings/vault` |
| `sync_calendar` | `POST /calendar/sync` |
| `pick_vault_dir` | Tauri-only: native folder dialog |

## License

MIT
