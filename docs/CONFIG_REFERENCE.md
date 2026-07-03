# Configuration Reference

Full reference of all configuration options for clin-rs.

---

## General (`~/.config/clin/config.toml`)

### General Settings

| Option | Type | Default | Description |
|---|---|---|---|
| `storage_path` | `PathBuf` | `~/.local/share/clin` | Custom vault storage path. Supports `~` and `$VAR`/`${VAR}` expansion (e.g., `~/notes`, `$HOME/vault`) |
| `previous_storage_path` | `PathBuf` | — | Previous storage path for migration |
| `mouse_enabled` | `bool` | `true` | Enable mouse support (clicking, scrolling, panning) |
| `confirm_on_delete` | `bool` | `true` | Show confirmation dialog before deleting notes |
| `default_folder` | `String` | — | Default folder for new notes (optional) |
| `confirm_on_quit` | `bool` | `false` | Ask for confirmation before quitting |
| `preview_wrap` | `bool` | `true` | Wrap markdown preview to pane width (toggle at runtime with Ctrl+w) |
| `syntax_highlighting` | `bool` | `true` | Enable syntax highlighting in markdown fenced code blocks (requires re-render) |
| `keybind_preset` | `enum` | `"default"` | Keybind preset: `"default"`, `"helix"`, `"vim"`, `"emacs"`. Applies to navigation, never text editing |
| `enable_key_sequences` | `bool` | `false` | Enable multi-key sequences (e.g. `"g g"`, `"Space f"`). Requires a preset that uses them |
| `preview_expand_mode` | `enum` | `"inline"` | Ctrl+e behavior: `"inline"` (maximize the preview pane) or `"external"` (run `preview_command` on the note) |
| `preview_command` | `String` | — | Command for external preview (Ctrl+e when `preview_expand_mode = "external"`). Shell-split with the note's temp file appended; falls back to `$PAGER`, then `less` |

### `[list]`

| Option | Type | Default | Description |
|---|---|---|---|
| `preview_enabled` | `bool` | `true` | Show the preview pane in notes list by default |
| `preview_position` | `enum` | `"right"` | Preview pane position: `"left"`, `"right"` |
| `preview_encryption` | `bool` | `false` | Show previews of encrypted notes |
| `show_date_in_list` | `bool` | `true` | Show modification date in the notes list |
| `show_file_size` | `bool` | `false` | Show file size in the notes list |
| `date_format` | `String` | `"%Y-%m-%d"` | Date format for the notes list (chrono format) |
| `density` | `enum` | `"compact"` | Density of the notes list: `"comfortable"` or `"compact"` |
| `default_view` | `enum` | `"grid"` | Default view mode for the notes list: `"grid"` or `"tree"` |
| `default_sort_field` | `enum` | `"title"` | Default sort field: `"title"` or `"modified"` |
| `default_sort_order` | `enum` | `"ascending"` | Default sort order: `"ascending"` or `"descending"` |
| `pinned_on_top` | `bool` | `true` | Keep pinned notes at the top of the list |
| `calendar_enabled` | `bool` | `true` | Show a month calendar with note activity at the bottom of the notes list |
| `show_hidden_files` | `bool` | `false` | Show hidden files and folders (starting with ".") in the notes list |
| `show_all_files` | `bool` | `false` | Show every file in the vault, not just notes (.md/.txt/.clin/.draw/.canvas). Non-note files open in the OS default application |
| `folders_first` | `bool` | `true` | Show subfolders before files in the notes list (Tree and Grid layouts) |
| `preview_width_ratio` | `f32` | `0.43` | Preview pane width ratio (0.2–0.8) |
| `calendar_height` | `u16` | `9` | Calendar height in rows (9–20) |
| `calendar_position` | `enum` | `"bottom"` | Calendar position: `"top"`, `"bottom"` |
| `week_start` | `enum` | `"sunday"` | Start day for the rolling-weeks calendar: `"sunday"` or `"monday"` |
| `sections` | `array` | `["calendar","goals"]` | Bottom-strip widgets (max 2): `calendar`, `goals`, `draw`, `graf`. `calendar_enabled` controls strip on/off |

### `[editor]`

| Option | Type | Default | Description |
|---|---|---|---|
| `external_command` | `String` | — | External editor command (e.g. `"nvim"`, `"code"`) |
| `external_enabled` | `bool` | `false` | Enable external editor mode |
| `preview_enabled` | `bool` | `false` | Show markdown preview panel in editor by default |
| `show_line_numbers` | `bool` | `true` | Show line numbers in the editor |

### `[ui]`

| Option | Type | Default | Description |
|---|---|---|---|
| `theme` | `enum` | `"default"` | Color theme. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for all 11 options |
| `background` | `enum` | `"transparent"` | Background mode: `"transparent"`, `"solid"` |
| `show_status_bar` | `bool` | `true` | Show the status bar at the bottom of the screen |
| `tab_icons_only` | `bool` | `false` | Show only Nerd Font icons (no text) on tab bars |
| `icon_mode` | `enum` | `"nerd"` | Icon display mode: `"nerd"`, `"unicode"`, `"none"` |
| `hint_bar_style` | `enum` | `"classic"` | Hint/status bar style: `"classic"`, `"accent"`, `"powerline_sharp"`, `"powerline_rounded"`, `"powerline_slanted"` |
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

### `[graf]`

| Option | Type | Default | Description |
|---|---|---|---|
| `preview_enabled` | `bool` | `false` | Enable the preview pane in Graph view |

### `[graf.visual]`

| Option | Type | Default | Description |
|---|---|---|---|
| `graph_background` | `enum` | `"solid"` | Background mode: `"transparent"`, `"solid"` |
| `node_color_mode` | `enum` | `"folder"` | How nodes are colored: `"tag"`, `"folder"`, `"link_count"`, `"uniform"` |
| `edge_color_mode` | `enum` | `"uniform"` | How edges are colored: `"source"`, `"target"`, `"uniform"` |
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
| `node_shape` | `enum` | `"circle"` | Node shape: `"circle"`, `"square"`, `"diamond"` |
| `label_offset` | `f64` | `4.0` | Distance of labels from nodes |
| `grid_divisions` | `usize` | `10` | Grid subdivision count |
#### `[graf.visual.colors]`

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

### `[graf.physics]`

| Option | Type | Default | Description |
|---|---|---|---|
| `ideal_distance` | `f64` | `80.0` | Target distance between connected nodes |

### `[graf.interaction]`

| Option | Type | Default | Description |
|---|---|---|---|
| `zoom_factor` | `f64` | `1.15` | Zoom multiplier per step (must be > 0) |
| `drag_sensitivity` | `f64` | `1.0` | Pan drag sensitivity |


### `[graf.filter]`

| Option | Type | Default | Description |
|---|---|---|---|
| `exclude_tags` | `Vec<String>` | `[]` | Tags to exclude from graph |
| `min_links` | `usize` | `0` | Minimum links for a node to appear |

### `[graf.search]`

| Option | Type | Default | Description |
|---|---|---|---|
| `max_results` | `usize` | `20` | Maximum search results |
| `max_visible` | `usize` | `10` | Maximum visible results at once |
### `[backup]`

| Option | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Enable auto-backups via git |
| `backup_on_save` | `bool` | `false` | Perform a backup commit whenever a note is saved |
| `backup_on_quit` | `bool` | `false` | Perform a backup commit when the app exits |
| `auto_backup_interval` | `u64` | — | Interval in minutes for automatic background backups |
| `auto_push` | `bool` | `false` | Automatically push commits to remote |
| `remote_url` | `String` | — | Remote git repository URL |
| `remote_name` | `String` | `"origin"` | Name of the git remote |

### `[goals]`

| Option | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Enable the daily word/note goals system |
| `word_goal` | `usize` | `500` | Daily target word count (incremental additions). Set to 0 to disable |
| `note_goal` | `usize` | `3` | Daily target note count (edited or created). Set to 0 to disable |
---

## Example config.toml

```toml
# General
# storage_path supports ~ and $VAR/${VAR} expansion: "~/notes", "$HOME/vault"
storage_path = "/path/to/your/vault"
mouse_enabled = true
confirm_on_delete = true

[list]
preview_enabled = true
preview_position = "right"
preview_encryption = false
show_date_in_list = true
show_file_size = false
date_format = "%Y-%m-%d"
density = "comfortable"
default_view = "grid"
default_sort_field = "modified"
default_sort_order = "descending"
calendar_enabled = true
pinned_on_top = true

[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = true
show_line_numbers = true

[backup]
enabled = false
backup_on_save = false
backup_on_quit = false
auto_backup_interval = 30
auto_push = false
remote_url = "https://github.com/user/my-notes.git"
remote_name = "origin"

[goals]
enabled = true
word_goal = 500
note_goal = 3

[ui]
theme = "tokyo_night"
background = "transparent"
show_status_bar = true
icon_mode = "nerd"
hint_bar_style = "classic"
accent = "#ff6600"

[graf]
preview_enabled = false

[graf.visual]
node_color_mode = "folder"
label_mode = "selected"
node_size = 2.0
show_legend = true
show_minimap = false

[graf.visual.colors]
node_color = "#ff6600"
border_color = "#334455"

[graf.physics]
ideal_distance = 80.0

[graf.interaction]
drag_sensitivity = 1.0


[graf.search]
max_results = 20
max_visible = 10
```

---

## Keybinds (`~/.config/clin/keybinds.toml`)

Key combos are strings like `"a"`, `"Enter"`, `"Ctrl+q"`, `"Ctrl+Shift+z"`, `"Alt+x"`, `"Super+c"`. Supported modifiers: `Ctrl`/`Control`, `Shift`, `Alt`, `Super`/`Meta`/`Cmd`.

### List Actions (`[list]`)

| Action | Default Keys | Description |
|---|---|---|
| `move_up` | `Up`, `k` | Move selection up |
| `move_down` | `Down`, `j` | Move selection down |
| `move_left` | `Left`, `h` | Move selection left (grid) |
| `move_right` | `Right`, `l` | Move selection right (grid) |
| `open` | `Enter`, `o` | Open selected item |
| `delete` | `d`, `Delete` | Delete item |
| `quit` | `q` | Quit application |
| `help` | `?`, `F1` | Show help |
| `open_location` | `Ctrl+l` | Open file location |
| `cycle_focus` | `Tab`, `BackTab` | Cycle focus between panes |
| `confirm` | `y`, `Enter` | Confirm dialog |
| `cancel` | `n`, `Esc` | Cancel dialog |
| `toggle_external_editor` | `e` | Open current note in $EDITOR |
| `new_from_template` | `t` | Create note from template |
| `create_folder` | `Shift+N` | Create folder |
| `create_note` | `n` | Create note |
| `rename_folder` | `r` | Rename folder |
| `rename` | `r` | Rename note (context) |
| `move_note` | `m` | Move note or folder |
| `manage_tags` | `.` | Manage tags |
| `open_command_palette` | `:`, `Ctrl+p` | Open command palette |
| `duplicate` | `y` | Duplicate note |
| `toggle_pin` | `p` | Pin/unpin note |
| `cycle_sort` | `s` | Cycle sort order |
| `search` | `/` | Search notes |
| `jump_to_top` | `Home`, `Ctrl+Up` | Jump to top of list |
| `jump_to_bottom` | `End`, `Ctrl+Down` | Jump to bottom of list |
| `page_up` | `Ctrl+u`, `PageUp` | Half page up |
| `page_down` | `Ctrl+d`, `PageDown` | Half page down |
| `open_trash` | `Shift+T` | Open trash view |
| `toggle_preview` | `Shift+P` | Toggle preview pane |
| `toggle_preview_fullscreen` | `Ctrl+e` | Preview/editor fullscreen |
| `toggle_preview_wrap` | `Ctrl+w` | Toggle word-wrap in preview |
| `preview_page_up` | `Shift+Up` | Page preview pane up |
| `preview_page_down` | `Shift+Down` | Page preview pane down |
| `toggle_calendar` | `Shift+C` | Toggle calendar |
| `open_graph` | `Ctrl+g` | Open graph view |
| `toggle_select_mode` | `v` | Toggle multi-select mode |
| `toggle_select_item` | `Space` | Toggle item selection |
| `collapse_all` | `c` | Collapse all folders |
| `refresh_notes` | `Ctrl+r` | Refresh notes (external changes) |

### Edit Actions (`[edit]`)

| Action | Default Keys | Description |
|---|---|---|
| `back` | `Esc` | Return to notes (auto-saves) |
| `cycle_focus` | `Tab`, `BackTab` | Cycle focus (Title/Body) |
| `select_all` | `Ctrl+a` | Select all text |
| `copy` | `Ctrl+Shift+c`, `Ctrl+Insert` | Copy |
| `cut` | `Ctrl+Shift+x`, `Shift+Delete` | Cut |
| `paste` | `Ctrl+Shift+v`, `Shift+Insert` | Paste |
| `undo` | `Ctrl+z` | Undo |
| `redo` | `Ctrl+y`, `Ctrl+Shift+z` | Redo |
| `delete_word` | `Ctrl+Backspace` | Delete previous word |
| `delete_next_word` | `Ctrl+Delete` | Delete next word |
| `move_to_top` | `Ctrl+Home` | Move cursor to top |
| `move_to_bottom` | `Ctrl+End` | Move cursor to bottom |
| `toggle_markdown_preview` | `Ctrl+p` | Toggle markdown preview |
| `toggle_preview_fullscreen` | `Ctrl+e` | Preview fullscreen |
| `toggle_preview_wrap` | `Ctrl+w` | Toggle preview word-wrap |
| `preview_page_up` | `PageUp` | Page markdown preview up |
| `preview_page_down` | `PageDown` | Page markdown preview down |

### Help Actions (`[help]`)

| Action | Default Keys | Description |
|---|---|---|
| `close` | `Esc`, `q`, `?`, `F1` | Close help |
| `next_tab` | `Right`, `l`, `Tab` | Next help tab |
| `prev_tab` | `Left`, `h`, `BackTab` | Previous help tab |
| `scroll_up` | `Up`, `k` | Scroll up |
| `scroll_down` | `Down`, `j` | Scroll down |
| `search` | `/`, `Ctrl+f` | Search help |

> Note: digits `1`–`9` jump directly to the nine help tabs (Notes→About). These are fixed and not configurable in `keybinds.toml`.

### Graph Actions (`[graph]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Quit graph view |
| `pan_up` | `Up`, `k` | Jump to node above |
| `pan_down` | `Down`, `j` | Jump to node below |
| `pan_left` | `Left`, `h` | Jump to node left |
| `pan_right` | `Right`, `l` | Jump to node right |
| `zoom_in` | `+`, `=` | Zoom in |
| `zoom_out` | `-`, `_` | Zoom out |
| `open_note` | `Enter`, `o` | Open selected note |
| `auto_fit` | `a` | Auto-fit view to all nodes |
| `help` | `?`, `F1` | Show help |
| `toggle_search` | `/` | Toggle node search |
| `toggle_minimap` | `Shift+M` | Toggle minimap |
| `toggle_legend` | `Shift+L` | Toggle legend |
| `toggle_grid` | `Shift+G` | Toggle background grid |
| `toggle_status` | `Shift+S` | Toggle status bar |
| `toggle_preview` | `Shift+P` | Toggle preview |
| `refresh` | `r` | Refresh simulation |
| `reload_config` | `Ctrl+r` | Reload config file |

### Draw Actions (`[draw]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Exit draw view |
| `help` | `?` | Show help |
| `select_draw_tool` | `d` | Select freehand draw tool |
| `toggle_shape_selector` | `s` | Open shape picker |
| `select_text_tool` | `t` | Select text tool |
| `select_erase_tool` | `e` | Select erase tool |
| `shape_selector_up` | `Up`, `k` | Previous shape |
| `shape_selector_down` | `Down`, `j` | Next shape |
| `shape_selector_confirm` | `Enter` | Confirm shape |
| `shape_selector_cancel` | `Esc`, `q` | Cancel shape selection |
| `text_editor_confirm` | `Enter` | Confirm text edit |
| `text_editor_cancel` | `Esc` | Cancel text edit |
| `toggle_grid` | `Shift+G` | Toggle grid |

### Canvas Actions (`[canvas]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Quit canvas view |
| `save` | `Ctrl+s` | Save canvas |
| `zoom_fine_in` | `>`, `]` | Zoom in (fine) |
| `zoom_fine_out` | `<`, `[` | Zoom out (fine) |
| `zoom_in` | `+`, `=` | Zoom in |
| `zoom_out` | `-`, `_` | Zoom out |
| `move_left` | `Left`, `h` | Move selection left |
| `move_right` | `Right`, `l` | Move selection right |
| `move_up` | `Up`, `k` | Move selection up |
| `move_down` | `Down`, `j` | Move selection down |
| `edit_or_connect` | `i`, `Enter`, `o` | Edit node / connect |
| `open_context_menu` | `a` | Open context menu |
| `toggle_grid` | `Shift+G` | Toggle grid |
| `toggle_editor_pane` | `Ctrl+e` | Toggle editor pane |
| `cycle_focus` | `Tab`, `BackTab` | Cycle focus |
| `help` | `?` | Show help |
| `rename_confirm` | `Enter` | Confirm rename |
| `rename_cancel` | `Esc` | Cancel rename |
| `menu_close` | `Esc` | Close context menu |
| `menu_up` | `Up`, `k` | Menu up |
| `menu_down` | `Down`, `j` | Menu down |
| `menu_select` | `Enter` | Menu confirm |
| `close_editor` | `Esc` | Close editor |
| `close_editor_alt` | `Ctrl+Enter` | Close editor (alt) |
| `confirm_resize` | `Enter` | Confirm resize |
| `cancel_resize` | `Esc` | Cancel resize |
| `editor_unfocus` | `Esc` | Exit editor focus |
| `editor_sync_raw` | `Ctrl+s` | Save raw editor changes |

### Backup Actions (`[backup]`)

| Action | Default Keys | Description |
|---|---|---|
| `back` | `Esc`, `q` | Back to list |
| `move_down` | `j`, `Down` | Move selection down |
| `move_up` | `k`, `Up` | Move selection up |
| `scroll_diff_down` | `Ctrl+d`, `PageDown` | Scroll diff down |
| `scroll_diff_up` | `Ctrl+u`, `PageUp` | Scroll diff up |
| `refresh` | `r` | Refresh status |
| `enter_commit` | `c` | Enter commit message |
| `confirm_commit` | `Enter` | Confirm commit |
| `cancel_commit` | `Esc` | Cancel commit |
| `push` | `p` | Push to remote |
| `open_settings` | `,`, `Shift+S` | Open settings |
| `close_settings` | `Esc`, `q` | Close settings |
| `toggle_file_select` | `Space` | Toggle file select |
| `cycle_section` | `Tab`, `BackTab` | Cycle sections |
| `next_field` | `j`, `Down` | Next settings field |
| `prev_field` | `k`, `Up` | Previous settings field |
| `activate_field` | `Enter` | Activate settings field |
| `confirm_edit_field` | `Enter` | Confirm field edit |
| `cancel_edit_field` | `Esc` | Cancel field edit |

### Content Tree Actions (`[content_tree]`)

| Action | Default Keys | Description |
|---|---|---|
| `move_up` | `k`, `Up` | Move selection up |
| `move_down` | `j`, `Down` | Move selection down |
| `toggle_collapse` | `Tab`, `Left`, `Right`, `h`, `l` | Toggle collapse/expand |
| `expand_all` | `e` | Expand all |
| `collapse_all` | `c` | Collapse all |
| `open` | `Enter`, `o` | Jump to section |
| `back` | `Esc`, `q` | Back |
| `help` | `?`, `F1` | Show help |

---

## Migration Note

The old `graf.toml` file is **no longer used**. All graf options (`[graf.visual]`, `[graf.physics]`, `[graf.interaction]`, `[graf.filter]`, `[graf.search]`) are now part of `config.toml`. The system auto-migrates settings from `graf.toml` on first read for backward compatibility.

---


---

## See Also

- [README.md](../README.md) — Quickstart, installation, CLI commands
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system details and color reference
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Graph view configuration context
