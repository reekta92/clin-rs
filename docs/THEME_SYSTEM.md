# Theme System

Technical docs for the theme and color system — 19 built-in themes, per-color overrides, background modes, and color derivation.

---

## Custom Themes

Drop a TOML file into `~/.config/clin/themes/<name>.toml` to make `<name>` available
as a theme. Set `theme = "<name>"` in `config.toml` to activate it.

### File Schema

```toml
# ~/.config/clin/themes/my_theme.toml
[chrome]
accent = "#7aa2f7"
heading = "#e0af68"
success = "#9ece6a"
warning = "#e0af68"
destructive = "#f7768e"
muted = "#565f89"
text = "#c0caf5"
fg = "#ffffff"
border = "#414868"
tag = "#bb9af7"
folder = "#7dcfff"
highlight_fg = "#1a1b26"
highlight_bg = "#7aa2f7"
background = "#1a1b26"   # optional → transparent when absent

[graph]
nodes = ["#7aa2f7","#bb9af7","#7dcfff","#e0af68","#9ece6a","#f7768e","#94e2d5","#ff9e64"]
chrome = "#565f89"
title  = "#bb9af7"
text   = "#cbccd5"
fg     = "#ffffff"
grid   = "#383c5f"
bg     = "#1a1b26"       # optional
```

### Lookup Order

1. **Custom dir** — `<name>.toml` in `~/.config/clin/themes/` is checked first.
2. **Built-in** — if no custom file found, the name is matched against built-in themes.
3. **Fallback** — unknown names silently resolve to the Default theme.

A custom theme with the same name as a built-in overrides it.

### Switcher

The theme switcher (Ctrl+P → "Switch Theme") lists built-in themes first, then
appends any custom theme names found in the themes directory. Selecting a custom
name applies it immediately.

### No-Recompile Path

To add a theme without recompiling, write a TOML file with the schema above and
select it by name. This is the preferred way to introduce new themes.

## Overview

clin has a flexible theme system with 19 built-in themes, transparent and solid background modes, and per-color overrides via config.toml. The theme affects all views: list, editor, graph, canvas, draw, and popups.

---

## Architecture

```
config.toml [theme] section
         │
         ▼
ThemeConfig struct (config.rs)
    ├── theme: Theme enum
    ├── background: Background enum
    └── per-color overrides (Option<String> hex)
         │
         ▼
AppThemeColors::from_config(&ThemeConfig)
    ├── Theme enum → graf/themes.rs → ThemeColors palette
    ├── Apply per-color overrides
    └── Return AppThemeColors struct
         │
         ▼
Used everywhere in rendering (app_theme field on App)
    ├── draw_ui() — background block, panes
    ├── draw_list_view() — selection, tags, folder colors
    ├── draw_edit_view() — title bar, editor, status bar
    ├── draw_help_view() — tab bar, content
    ├── draw_canvas() / draw_graph_view() — panel colors
    └── popups — all popup rendering
```

---

## Built-in Themes

| # | Name | Config Value |
|---|---|---|
| 1 | Default | `"default"` |
| 2 | Tokyo Night | `"tokyo_night"` / `"tokyonight"` |
| 3 | Catppuccin Mocha | `"catppuccin_mocha"` / `"catppuccinmocha"` |
| 4 | One Dark | `"onedark"` |
| 5 | Gruvbox | `"gruvbox"` |
| 6 | Dracula | `"dracula"` |
| 7 | Nord | `"nord"` |
| 8 | Rose Pine | `"rose_pine"` / `"rosepine"` |
| 9 | Everforest | `"everforest"` |
| 10 | Kanagawa | `"kanagawa"` |
| 11 | Solarized | `"solarized"` / `"solarized_dark"` |
| 12 | Catppuccin Frappé | `"catppuccin_frappe"` / `"catppuccinfrappe"` |
| 13 | Catppuccin Macchiato | `"catppuccin_macchiato"` / `"catppuccinmacchiato"` |
| 14 | Rose Pine Moon | `"rose_pine_moon"` / `"rosepinemoon"` |
| 15 | Gruvbox Material | `"gruvbox_material"` / `"gruvboxmaterial"` |
| 16 | GitHub Dark | `"github_dark"` / `"githubdark"` |
| 17 | Ayu Mirage | `"ayu_mirage"` / `"ayumirage"` |
| 18 | Synthwave '84 | `"synthwave"` / `"synthwave84"` |
| 19 | Material | `"material"` / `"material_theme"` |

Each theme defines an 8-color palette for nodes plus chrome, title, text, foreground, grid, and background colors in `src/graf/themes.rs`.

---

## Theme Config Options

All in `[theme]` section of `config.toml`:

```toml
[theme]
theme = "tokyo_night"           # Theme name
background = "transparent"      # "transparent" or "solid"

# Per-color overrides (optional hex values)
accent = "#ff6600"
heading = "#ffaa00"
success = "#00ff66"
destructive = "#ff0044"
muted = "#445566"
text = "#aabbcc"
border = "#334455"
tag = "#ff66aa"
folder = "#66aaff"
background_color = "#1a1a2e"
```

### Default Theme

When `theme = "default"`, the system uses hardcoded ratatui colors:
- `accent`: Cyan
- `heading`: Yellow
- `success`: Green
- `destructive`: Red
- `muted`: DarkGray
- `text`: Reset (terminal default)
- `background`: Black (solid) or None (transparent)

---

## Color Derivation

`derive_color()` in `src/app_theme.rs` creates variant shades from base colors:

```rust
fn derive_color(base: Option<Color>, delta: i16) -> Option<Color> {
    base.map(|c| match c {
        Color::Rgb(r, g, b) => {
            let clamp = |v: i16| v.clamp(0, 255) as u8;
            Color::Rgb(
                clamp(r as i16 + delta),
                clamp(g as i16 + delta),
                clamp(b as i16 + delta),
            )
        }
        other => other,
    })
}
```

Used for:

| Method | Delta | Purpose |
|---|---|---|
| `preview_bg()` | -15 | Preview pane background |
| `title_bar_bg()` | -10 | Title/tab bar background |
| `hint_line_bg()` | -8 | Status/hint line background |

---

## AppThemeColors Struct

```rust
pub struct AppThemeColors {
    pub accent: Color,       // Primary accent (selected items, highlights)
    pub heading: Color,      // Section headings
    pub success: Color,      // Success indicators
    pub warning: Color,      // Warnings
    pub destructive: Color,  // Destructive actions (delete)
    pub muted: Color,        // Dim/muted text
    pub text: Color,         // Body text
    pub fg: Color,           // Foreground (white/bright)
    pub bg: Option<Color>,   // Background (None = transparent)
    pub border: Color,       // Borders and dividers
    pub tag: Color,          // Tag labels
    pub folder: Color,       // Folder labels
    pub highlight_fg: Color, // Text on highlighted bg
    pub highlight_bg: Color, // Selection highlight background
}
```

Style convenience methods:

```rust
impl AppThemeColors {
    pub fn bg_style(&self) -> Style;              // Background style
    pub fn preview_bg_style(&self) -> Style;      // Preview pane style
    pub fn title_bar_bg_style(&self) -> Style;    // Title bar style
    pub fn hint_line_bg_style(&self) -> Style;    // Hint line style
    pub fn pane_bg(&self) -> Option<Color>;        // Pane background
}
```

---

## Per-View Color Usage

| View | Uses |
|---|---|
| List | `accent` (selection), `tag` (tags), `folder` (folder names), `muted` (hints), `border` (dividers), `bg` (background) |
| Edit | `accent` (cursor), `highlight_fg`/`highlight_bg` (title bar), `muted` (status), `border` (dividers) |
| Help | `accent` (current tab), `muted` (other tabs), `title_bar_bg_style` (tab bar), `bg_style` (content) |
| Graph | Uses graf `ThemeColors` palette for node/edge/grid colors; `app_theme` for UI shell (status bar, borders) |
| Canvas | Uses `app_theme` for UI shell; node/edge colors from individual `.canvas` file data |
| Popups | `bg_style`, `accent` (selected item), `muted` (dim items), `border` |

---

## Adding a New Theme

1. Add variant to `Theme` enum in `src/config.rs`:
   ```rust
   pub enum Theme {
       // ... existing ...
       MyNewTheme,
   }
   ```
2. Add palette entry in `src/graf/themes.rs`:
   ```rust
   const PALETTES: [ThemePalette; 11] = [
       // ... existing ...
       ThemePalette {
           nodes: [[...], [...], ...],
           chrome: [...],
           title: [...],
           text: [...],
           fg: [...],
           grid: [...],
           bg: [...],
       },
   ];
   ```
3. Add parse/display mapping in `Theme::from_str()` and `Theme::fmt()` in `config.rs`.
4. Build and test — theme is auto-detected from config and applied on startup.

---

## Theme Switcher

The `SwitchThemeAction` (via command palette, Ctrl+P) opens a theme popup. Users can cycle through themes live — the change applies immediately and saves to `config.toml`.

See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for action details.

---

## Connections

- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — `[theme]` config section
- [ARCHITECTURE.md](ARCHITECTURE.md) — how `AppThemeColors` flows through the rendering pipeline
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — graf theme palettes
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `SwitchThemeAction`
