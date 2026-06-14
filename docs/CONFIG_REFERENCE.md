# Configuration Reference

Full reference of all configuration options for clin-rs.

---

## General (`~/.config/clin/config.toml`)

### General Settings

| Option | Type | Default | Description |
|---|---|---|---|
| `storage_path` | `PathBuf` | `~/.local/share/clin` | Custom vault storage path |
| `previous_storage_path` | `PathBuf` | — | Previous storage path for migration (cleared after migration) |
| `external_editor` | `String` | — | External editor command (e.g. `"nvim"`, `"code"`) |
| `external_editor_enabled` | `bool` | `false` | Enable external editor mode (Toggleable via Command Palette) |
| `preview_enabled` | `bool` | `true` | Show the preview pane in notes list by default (Toggleable via Command Palette) |
| `graph_preview_enabled` | `bool` | `false` | Show the preview pane in graph view by default |
| `editor_preview_enabled` | `bool` | `false` | Show markdown preview panel in editor by default |
| `markdown_preview_enabled` | `bool` | `false` | *(deprecated — use `editor_preview_enabled`)* |
| `show_line_numbers` | `bool` | `true` | Show line numbers in the editor (Toggleable via Command Palette) |
| `confirm_on_delete` | `bool` | `true` | Show confirmation dialog before deleting notes (Toggleable via Command Palette) |
| `default_sort_field` | `enum` | `"title"` | Default sort field: `"title"` or `"modified"` (Toggleable via Command Palette) |
| `default_sort_order` | `enum` | `"ascending"` | Default sort order: `"ascending"` or `"descending"` (Toggleable via Command Palette) |
| `default_folder` | `String` | — | Default folder for new notes (optional) |
| `confirm_on_quit` | `bool` | `false` | Ask for confirmation before quitting (Toggleable via Command Palette) |
| `preview_encryption` | `bool` | `false` | Show previews of encrypted notes (Toggleable via Command Palette) |
| `pinned_on_top` | `bool` | `false` | Keep pinned notes at the top of the list (Toggleable via Command Palette) |
| `graph_label_mode` | `enum` | `"selected"` | *(moved to `[visual]`)* Node label display mode: `selected`, `neighbors`, `all` |

### `[theme]`

| Option | Type | Default | Description |
|---|---|---|---|
| `theme` | `enum` | `"default"` | Color theme. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for all 11 options |
| `background` | `enum` | `"transparent"` | Background mode: `"transparent"`, `"solid"` |
| `accent` | `String` | — | Hex color override for accent (#ff6600) |
| `heading` | `String` | — | Hex color override for headings |
| `success` | `String` | — | Hex color override for success indicators |
| `destructive` | `String` | — | Hex color override for destructive actions |
| `muted` | `String` | — | Hex color override for muted/dim text |
| `text` | `String` | — | Hex color override for body text |
| `border` | `String` | — | Hex color override for borders |
| `tag` | `String` | — | Hex color override for tag labels |
| `folder` | `String` | — | Hex color override for folder labels |
| `background_color` | `String` | — | Hex color override for solid background |

### `[visual]`

| Option | Type | Default | Description |
|---|---|---|---|
| `theme` | `enum` | `"default"` | Color theme. One of: `default`, `tokyo_night`, `catppuccin_mocha`, `onedark`, `gruvbox`, `dracula`, `nord`, `rose_pine`, `everforest`, `kanagawa`, `solarized` |
| `background` | `enum` | `"transparent"` | Background mode: `"transparent"`, `"solid"` |
| `node_color_mode` | `enum` | `"folder"` | How nodes are colored: `"tag"`, `"folder"`, `"link_count"`, `"uniform"` |
| `edge_color_mode` | `enum` | `"source"` | How edges are colored: `"source"`, `"target"`, `"uniform"` |
| `label_mode` | `enum` | `"selected"` | Label visibility: `"selected"`, `"neighbors"`, `"all"`, `"none"` |
| `label_max_length` | `usize` | `20` | Max label character length (1–60) |
| `node_size` | `f64` | `2.0` | Base node size (1.0–5.0) |
| `node_size_mode` | `enum` | `"fixed"` | How node size is determined: `"fixed"`, `"link_count"` |
| `edge_thickness` | `u16` | `1` | Edge line thickness (1–3) |
| `show_legend` | `bool` | `true` | Show legend |
| `show_grid` | `bool` | `false` | Show background grid |
| `show_minimap` | `bool` | `false` | Show minimap |
| `minimap_position` | `enum` | `"top_right"` | Minimap corner: `"top_right"`, `"top_left"`, `"bottom_right"`, `"bottom_left"` |
| `minimap_width` | `u16` | `24` | Minimap width in cells |
| `minimap_height` | `u16` | `12` | Minimap height in cells |
| `canvas_marker` | `enum` | `"braille"` | Canvas rendering marker: `"braille"`, `"half_block"`, `"dot"` |
| `minimap_marker` | `enum` | `"half_block"` | Minimap rendering marker: `"braille"`, `"half_block"`, `"dot"` |
| `node_shape` | `enum` | `"circle"` | Node shape: `"circle"`, `"square"`, `"diamond"` |
| `label_offset` | `f64` | `4.0` | Distance of labels from nodes |
| `grid_divisions` | `usize` | `10` | Grid subdivision count |

#### `[visual.colors]`

All optional. Hex color strings like `"#ff6600"`. Override theme defaults.

| Option | Type | Default |
|---|---|---|
| `node_color` | `String` | Theme default |
| `edge_color` | `String` | Theme default |
| `label_color` | `String` | Theme default |
| `selection_ring_color` | `String` | Theme default |
| `border_color` | `String` | Theme default |
| `title_color` | `String` | Theme default |
| `grid_color` | `String` | Theme default |
| `legend_text_color` | `String` | Theme default |
| `status_bar_color` | `String` | Theme default |
| `background_color` | `String` | Theme default |

### `[physics]`

| Option | Type | Default | Description |
|---|---|---|---|
| `ideal_distance` | `f64` | `80.0` | Target distance between connected nodes |
| `damping` | `f32` | `0.95` | Physics damping factor per step |
| `max_iterations` | `usize` | `800` | Maximum simulation iterations |
| `gravity` | `f64` | `0.01` | Gravitational pull toward center |
| `cooling` | `bool` | `true` | Enable energy cooling over time |
| `prevent_overlapping` | `bool` | `true` | Prevent nodes from overlapping |
| `timestep` | `f64` | `0.016` | Simulation timestep (~60fps) |
| `thread_sleep_ms` | `u64` | `16` | Thread sleep between iterations (ms) |

### `[interaction]`

| Option | Type | Default | Description |
|---|---|---|---|
| `double_click_ms` | `u64` | `300` | Double-click timeout in milliseconds |
| `zoom_factor` | `f64` | `1.15` | Zoom multiplier per step (must be > 0) |
| `drag_sensitivity` | `f64` | `1.0` | Pan drag sensitivity |
| `auto_fit_padding` | `f64` | `1.4` | Padding factor for auto-fit view |
| `drag_scale` | `f64` | `200.0` | Drag-to-pan scale factor |

### `[display]`

| Option | Type | Default | Description |
|---|---|---|---|
| `show_status_bar` | `bool` | `true` | Show status bar |
| `status_format` | `String` | — | Custom status bar format. Variables: `{files}`, `{links}`, `{selected}`, `{date}`, `{time}`, `{size}`, `{ratio}` |
| `border_style` | `enum` | `"rounded"` | Border style: `"plain"`, `"rounded"`, `"double"`, `"none"` |
| `border_title` | `String` | `"graf"` | Border title text. Supports `{cwd}` placeholder |

### `[filter]`

| Option | Type | Default | Description |
|---|---|---|---|
| `exclude_tags` | `Vec<String>` | `[]` | Tags to exclude from graph |
| `exclude_patterns` | `Vec<String>` | `[]` | Path patterns to exclude (e.g. `"templates/"`) |
| `min_links` | `usize` | `0` | Minimum links for a node to appear |
| `max_nodes` | `usize` | `500` | Maximum nodes to display |

### `[legend]`

| Option | Type | Default | Description |
|---|---|---|---|
| `position` | `enum` | `"bottom_right"` | Legend corner: `"top_right"`, `"top_left"`, `"bottom_right"`, `"bottom_left"` |
| `max_items` | `usize` | `10` | Maximum legend items |

### `[search]`

| Option | Type | Default | Description |
|---|---|---|---|
| `max_results` | `usize` | `20` | Maximum search results |
| `max_visible` | `usize` | `10` | Maximum visible results at once |
| `popup_width` | `u16` | `50` | Search popup width in cells |
| `popup_y` | `u16` | `3` | Search popup Y position from top |
| `cursor_glyph` | `String` | `"▎"` | Search cursor character |

### `[editor]`

| Option | Type | Default | Description |
|---|---|---|---|
| `command` | `String` | `""` | External editor command for opening notes from graf view |

---

## Example config.toml

```toml
# General
storage_path = "/path/to/your/vault"
external_editor = "nvim"
external_editor_enabled = false
preview_enabled = true
editor_preview_enabled = true
show_line_numbers = true
confirm_on_delete = true
default_sort_field = "modified"
default_sort_order = "descending"
default_folder = "inbox"

[theme]
theme = "tokyo_night"
background = "transparent"
accent = "#ff6600"

[visual]
theme = "onedark"
node_color_mode = "folder"
label_mode = "selected"
node_size = 2.0
show_legend = true
show_minimap = false

[visual.colors]
node_color = "#ff6600"
border_color = "#334455"

[physics]
ideal_distance = 80.0
damping = 0.95
gravity = 0.01

[interaction]
zoom_factor = 1.15
drag_sensitivity = 1.0

[display]
show_status_bar = true
border_style = "rounded"

[filter]
min_links = 0
max_nodes = 500

[legend]
position = "bottom_right"
max_items = 10

[search]
max_results = 20
popup_width = 50

[editor]
command = "nano"
```

---

## Keybinds (`~/.config/clin/keybinds.toml`)

Key combos are strings like `"a"`, `"Enter"`, `"Ctrl+q"`, `"Ctrl+Shift+z"`, `"Alt+x"`, `"Super+c"`. Supported modifiers: `Ctrl`/`Control`, `Shift`, `Alt`, `Super`/`Meta`/`Cmd`.

### List Actions (`[list]`)

| Action | Default Keys | Description |
|---|---|---|
| `move_up` | `Up`, `k` | Move selection up |
| `move_down` | `Down`, `j` | Move selection down |
| `open` | `Enter` | Open selected item |
| `delete` | `d`, `Delete` | Delete item |
| `quit` | `q` | Quit application |
| `help` | `?`, `F1` | Show help |
| `open_location` | `f` | Open file location |
| `cycle_focus` | `Tab` | Cycle focus between panes |
| `confirm` | `y`, `Enter` | Confirm dialog |
| `cancel` | `n`, `Esc` | Cancel dialog |
| `toggle_button` | `Enter`, `Space` | Toggle focused button |
| `new_from_template` | `t` | Create note from template |
| `create_folder` | `n` | Create folder |
| `create_note` | `a` | Create note |
| `rename_folder` | `r` | Rename folder |
| `move_note` | `m` | Move note |
| `manage_tags` | `.` | Manage tags |
| `filter_tags` | `/` | Filter by tags |
| `collapse_folder` | `h` | Collapse folder |
| `expand_folder` | `l` | Expand folder |
| `open_command_palette` | `Ctrl+p`, `Shift+Enter` | Open command palette |
| `rename` | `r` | Context-sensitive rename |
| `duplicate` | `y` | Duplicate note |
| `toggle_pin` | `p` | Pin/unpin note |
| `cycle_sort` | `s` | Cycle sort options |
| `search` | `Ctrl+f` | Quick search by title |
| `jump_to_top` | `Shift+G` | Jump to top of list |
| `jump_to_bottom` | — | Jump to bottom of list |
| `page_up` | `Ctrl+u` | Half page up |
| `page_down` | `Ctrl+d` | Half page down |
| `open_trash` | `Shift+T` | Open trash view |
| `toggle_preview` | `Shift+P` | Toggle preview pane |
| `open_graph` | `Ctrl+g` | Open graph view |

### Edit Actions (`[edit]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Ctrl+q` | Quit editor |
| `back` | `Esc` | Go back |
| `cycle_focus` | `Tab` | Cycle focus between elements |
| `toggle_button` | `Enter`, `Space` | Toggle focused button |
| `select_all` | `Ctrl+a` | Select all text |
| `copy` | `Ctrl+c`, `Ctrl+Insert` | Copy selection |
| `cut` | `Ctrl+x`, `Shift+Delete` | Cut selection |
| `paste` | `Ctrl+v`, `Shift+Insert` | Paste from clipboard |
| `undo` | `Ctrl+z` | Undo |
| `redo` | `Ctrl+y`, `Ctrl+Shift+z` | Redo |
| `delete_word` | `Ctrl+Backspace` | Delete word before cursor |
| `delete_next_word` | `Ctrl+Delete` | Delete word after cursor |
| `move_to_top` | `Ctrl+Home` | Move cursor to top |
| `move_to_bottom` | `Ctrl+End` | Move cursor to bottom |
| `toggle_markdown_preview` | `Ctrl+p` | Toggle markdown preview |

### Help Actions (`[help]`)

| Action | Default Keys | Description |
|---|---|---|
| `close` | `Esc`, `q`, `?`, `F1` | Close help |
| `scroll_up` | `Up`, `k` | Scroll up |
| `scroll_down` | `Down`, `j` | Scroll down |

### Graph Actions (`[graph]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc` | Quit graph view |
| `pan_up` | `Up`, `k` | Jump to node above |
| `pan_down` | `Down`, `j` | Jump to node below |
| `pan_left` | `Left`, `h` | Jump to node left |
| `pan_right` | `Right`, `l` | Jump to node right |
| `zoom_in` | `+`, `Ctrl+j` | Zoom in |
| `zoom_out` | `-`, `Ctrl+k` | Zoom out |
| `open_note` | `Enter` | Open selected note |
| `auto_fit` | `a` | Auto-fit view to all nodes |
| `help` | `?`, `F1` | Show help |
| `toggle_search` | `f` | Toggle node search |
| `toggle_minimap` | `Shift+M` | Toggle minimap |
| `toggle_legend` | `Shift+L` | Toggle legend |
| `toggle_grid` | `Shift+G` | Toggle grid |
| `toggle_status` | `Shift+S` | Toggle status bar |
| `refresh` | `r` | Refresh simulation |
| `reload_config` | `Ctrl+r` | Reload config file |

---

## Migration Note

The old `graf.toml` file is **no longer used**. All graf options (`[visual]`, `[physics]`, `[interaction]`, `[display]`, `[filter]`, `[legend]`, `[search]`, `[editor]`) are now part of `config.toml`. The system auto-migrates settings from `graf.toml` on first read for backward compatibility.

---

## Environment Variable Overrides

All graf options can also be overridden via environment variables prefixed with `GRAF_` (e.g. `GRAF_VISUAL_THEME=dracula`, `GRAF_PHYSICS_GRAVITY=0.05`).

---

## See Also

- [README.md](../README.md) — Quickstart, installation, CLI commands
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system details and color reference
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Graph view configuration context
