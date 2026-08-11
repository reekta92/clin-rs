# Configuration Reference

Full reference of all configuration options for clin-rs.

---

## Configuration (`~/.config/clin/config.toml`)

### `[core]`

| Option | Type | Default | Description |
|---|---|---|---|
| `storage_path` | `PathBuf` | `~/.local/share/clin` | Custom vault storage path. Supports `~` and `$VAR`/`${VAR}` expansion (e.g., `~/notes`, `$HOME/vault`) |
| `mouse_enabled` | `bool` | `true` | Enable mouse support (clicking, scrolling, panning) |
| `confirm_on_delete` | `bool` | `true` | Show confirmation dialog before deleting notes |
| `default_folder` | `String` | — | Default folder for new notes (optional) |
| `confirm_on_quit` | `bool` | `false` | Ask for confirmation before quitting |
| `preview_wrap` | `bool` | `true` | Wrap markdown preview to pane width (toggle at runtime with Ctrl+w) |
| `syntax_highlighting` | `bool` | `true` | Enable syntax highlighting in markdown fenced code blocks (requires re-render) |
| `code_theme` | `string` | `"base16-ocean.dark"` | syntect theme name for code-block highlighting (unknown names fall back to plain) |
| `code_line_numbers` | `bool` | `true` | Show line numbers in fenced code blocks |
| `preview_wrap_indicator` | `bool` | `false` | Append a `┄` continuation glyph at the end of soft-wrapped preview lines |
| `link_url_max_length` | `usize` | `80` | Middle-truncate link/image URLs longer than this; `0` disables |
| `keybind_preset` | `enum` | `"default"` | Keybind preset: `"default"`, `"helix"`, `"vim"`, `"emacs"`. Applies to navigation, never text editing |
| `enable_key_sequences` | `bool` | `false` | Enable multi-key sequences (e.g. `"g g"`, `"Space f"`). Requires a preset that uses them |
| `preview_expand_mode` | `enum` | `"inline"` | Ctrl+e behavior: `"inline"` (maximize the preview pane) or `"external"` (run `preview_command` on the note) |
| `preview_command` | `String` | — | Command for external preview (Ctrl+e when `preview_expand_mode = "external"`). Shell-split with the note's temp file appended; falls back to `$PAGER`, then `less` |
| `auto_refresh` | `bool` | `true` | Reload notes list on external file changes (notify watcher) |

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
| `inline_info` | `bool` | `true` | Show inline metadata info (modification date, tags) in the notes list |
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
| `smart_folders_enabled` | `bool` | `false` | Enable virtual smart folders (Today, This Week, Untagged) |
| `folder_graph_preview` | `bool` | `true` | Show folder graph preview for all folders |
| `pinned_folders` | `array` | `[]` | List of always-pinned folder paths |
| `expanded_folders` | `array` | `[]` | List of always-expanded folder paths |
| `default_expand_depth` | `usize` | — | Default tree expand depth (`None` = remember per-folder state) |
| `custom_smart_folders` | `array` | `[]` | User-defined smart folder rules. Each entry: `{name, tags=[], title_contains=..., folder_prefix=..., updated_within_days=...}` |

### `[editor]`

| Option | Type | Default | Description |
|---|---|---|---|
| `external_command` | `String` | — | External editor command (e.g. `"nvim"`, `"code"`) |
| `external_enabled` | `bool` | `false` | Enable external editor mode |
| `preview_enabled` | `bool` | `false` | Show markdown preview panel in editor by default |
| `show_line_numbers` | `bool` | `true` | Show line numbers in the editor |
| `date_format` | `String` | `"%Y-%m-%d %H:%M"` | Format used by the insert-date action |
| `soft_wrap` | `bool` | `false` | Soft-wrap the editor body |
| `edit_mode_highlight` | `bool` | `true` | Highlight the active READ/EDIT mode |
| `ghost_syntax` | `bool` | `true` | Visually dim markdown delimiters (brackets, URLs, etc.) in edit view |
| `extended_markdown_features` | `bool` | `true` | Enable extended markdown highlighting features (bare URLs, bold italic combinations, description lists, footnotes) |

### `[ui]`


| Option | Type | Default | Description |
|---|---|---|---|
| `theme` | `enum` | `"default"` | Color theme. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for all 19 options |
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
| `pinned` | `String` | — | Hex color override for pinned items |
| `smart` | `String` | — | Hex color override for smart folder items |
| `subnote` | `String` | — | Hex color override for subnote items |
| `background_color` | `String` | — | Hex color override for solid background |

### `[statusline]`

Customizes the status lines (title bar at the top, status bar at the bottom) of the application.

| Option | Type | Default | Description |
|---|---|---|---|
| `header_left` | `String` | `"{title} {preview}"` | Default template for the left side of the title bar |
| `header_right` | `String` | — | Default template for the right side of the title bar |
| `footer_left` | `String` | `"{pending}{badge}{hints}"` | Default template for the left side of the status bar |
| `footer_right` | `String` | — | Default template for the right side of the status bar |

You can also customize these per-view by adding nested overrides:
- `[statusline.list]` — overrides for the notes list view
- `[statusline.edit]` — overrides for the editor view
- `[statusline.help]` — overrides for the help view
- `[statusline.graph]` — overrides for the graph view
- `[statusline.draw]` — overrides for the drawing view
- `[statusline.canvas]` — overrides for the canvas view
- `[statusline.backup]` — overrides for the backup view
- `[statusline.outline]` — overrides for the outline view

Each override sub-table accepts: `header_left`, `header_right`, `footer_left`, and `footer_right` fields.

#### Template Interpolation Variables

Variables are enclosed in `{}` (e.g. `{time}`). Escapes `{{` and `}}` render literal braces. Unknown variables render literally (e.g. `{bogus}`).

##### Global / App (all views)
- `{view}`: Active view name (`Notes`, `Editor`, `Help`, `Graph`, etc.)
- `{status}`: Current status text or empty if "Ready"
- `{vault}`: Base folder name of the vault directory
- `{vault_path}`: Full absolute path to the vault directory
- `{version}`: Application package version

##### Date & Time (all views)
- `{time}`: Current local time (`%H:%M`)
- `{date}`: Current local date formatted per `date_format` configuration
- `{datetime}`: Local date and time combined
- `{weekday}`: Full weekday name (e.g., `Monday`)
- `{year}`, `{month}`, `{day}`, `{hour}`, `{minute}`, `{second}`: Respective time parts

##### Config Echo (all views)
- `{theme}`: Active theme name
- `{preset}`: Keybind preset (`default`, `helix`, `vim`, `emacs`)
- `{icon_mode}`: Icon mode (`nerd`, `unicode`, `none`)
- `{hint_bar_style}`: Style variant (`classic`, `accent`, `powerline_sharp`, `powerline_rounded`, `powerline_slanted`)
- `{background}`: Background style (`transparent`, `solid`)

##### Goals (all views)
- `{goal_words}`: Words written today
- `{goal_target}`: Daily word count goal
- `{goal_notes}`: Notes modified today
- `{goal_note_target}`: Daily note count goal
- `{goal_date}`: Tracked date for goals progress

##### List View (`Notes` view)
- `{title}`: Header title text (e.g. `"Notes"`, `"Notes - Editing Layout"`)
- `{sort_field}`: Current sorting field (`title`, `modified`)
- `{sort_order}`: Current sorting order (`ascending`, `descending`)
- `{layout}`: Active notes layout (`tree`, `grid`)
- `{density}`: Active list density (`compact`, `comfortable`)
- `{section}`: Active notes section (`vault`, `pinned`, `smart`)
- `{folder}`: Active grid folder path
- `{folder_count}`: Number of folders in the cache
- `{tag_count}`: Number of unique live tags in the vault
- `{note_count}`: Total notes in the notes directory
- `{visual_index}`: 1-based cursor index in the list
- `{visual_total}`: Total visible items in the list
- `{selected_count}`: Number of selected notes
- `{select_mode}`: `on`/`off` depending on selection mode
- `{tag_to_assign}`: Name of tag to assign, or empty
- `{search}`: Active search query text
- `{grep}`: Search grep mode status (`on`/`off`)
- `{tag_filter}`, `{folder_filter}`: Active query filter parameters
- `{pinned_count}`: Total pinned notes
- `{pinned_on_top}`, `{folders_first}`, `{list_preview}`, `{calendar}`, `{layout_edit}`: Config and view states (`on`/`off`)

##### Note Context (List + Edit views)
- `{note_title}`: Title of the selected/edited note
- `{note_id}`: Filename/ID of the note
- `{note_folder}`: Folder directory of the note
- `{note_format}`: File format extension (`md`, `txt`, `clin`, `draw`, `canvas`)
- `{note_size}`: Note file size formatted (e.g. `1.2 KB`)
- `{note_links}`: Number of wikilinks in the note
- `{tags}`: Comma-separated tags of the note
- `{has_tags}`: `on`/`off` depending on tag presence
- `{note_pinned}`: Note pinned status (`on`/`off`)
- `{note_updated}`, `{note_updated_rel}`: Absolute date and relative time of last update
- `{prev_note}`, `{next_note}`: Filenames of the previous and next notes in visual order

##### Editor (`Editor` view)
- `{word_count}`: Current word count
- `{line_count}`: Total lines in the editor
- `{char_count}`: Character count
- `{cursor_line}`: 1-based cursor line row
- `{cursor_col}`: 1-based cursor column
- `{modified}`: `on`/`off` depending on unsaved changes
- `{reading_time}`: Estimated reading time in minutes
- `{header_count}`: Count of headings parsed in outline
- `{task_count}`: Count of checkboxes parsed
- `{has_tasks}`: `on`/`off` depending on task presence
- `{has_frontmatter}`: `on`/`off` depending on YAML frontmatter presence
- `{words_added}`: Net words added since opening the note
- `{editing_id}`: Note ID being edited
- `{editing_template}`: `on`/`off` if editing a template
- `{line_numbers}`, `{editor_preview}`, `{ext_editor}`, `{ext_editor_enabled}`: Editor configuration/process states

##### Graph View (`Graph` view)
- `{node_count}`, `{edge_count}`: Graph nodes and edges
- `{selected_node}`: Label of the selected node or `"none"`
- `{viewport_size}`: Graph viewport coverage percentage (e.g. `"45%"`)
- `{scale}`: Graph zoom scale relative to auto-fit (e.g. `"1.00×"`)
- `{graph_settled}`: Force-directed simulation settlement status (`on`/`off`)
- `{label_mode}`, `{node_color_mode}`, `{edge_color_mode}`, `{node_size_mode}`, `{zoom}`: Graph configuration and zoom status
- `{show_grid}`, `{show_legend}`, `{show_minimap}`: Grid, legend, and minimap settings (`on`/`off`)

##### Draw View (`Draw` view)
- `{tool}`: Active draw tool (`draw`, `erase`, `text`, `shape`)
- `{shape}`: Active shape type (`rect`, `ellipse`, `diamond`, `line`, `arrow`)
- `{element_count}`: Total drawn elements
- `{draw_width}`, `{draw_height}`: Canvas drawing boundaries
- `{draw_grid}`, `{draw_zoom}`, `{text_editing}`: Grid status, zoom level, and text editing flags

##### Canvas View (`Canvas` view)
- `{canvas_nodes}`, `{canvas_edges}`: Total nodes and connections in canvas
- `{canvas_zoom}`: Zoom level
- `{canvas_pan_x}`, `{canvas_pan_y}`: Pan offset coordinates
- `{canvas_selected}`: Selected connection or node ID
- `{canvas_grid}`, `{canvas_editor}`: Grid and editor panel settings (`on`/`off`)

##### Outline View (`Outline` view)
- `{outline_nodes}`, `{outline_headers}`: Total outline nodes and headers
- `{outline_visible}`, `{outline_cursor}`: Visible node count and cursor position
- `{outline_depth}`, `{outline_max_depth}`, `{outline_expanded}`: Node depth and expansion states
- `{outline_heading}`: Title text of the selected heading
- `{outline_note}`: Title of parent note
- `{outline_error}`: Load error text or empty

##### Backup View (`Backup` view)
- `{branch}`: Active git branch name
- `{ahead}`, `{behind}`: Commit difference from remote tracking branch
- `{staged}`, `{unstaged}`, `{untracked}`: File category modification counts
- `{commit_count}`: Total commit log history entries
- `{last_commit}`: Short 7-character commit hash of HEAD
- `{last_commit_msg}`, `{last_commit_author}`, `{last_commit_time}`: Message, author name, and relative date of last commit
- `{remote}`, `{remote_url}`: Configured remote repository name and URL
- `{backup_section}`: Active section panel (`status` or `history`)
- `{input_mode}`: Input mode (`normal`, `edit_commit`, `edit_settings`, `edit_settings_field`)
- `{auto_push}`, `{repo_dirty}`: Auto push status and repository clean/dirty flags (`on`/`off`)
- `{modified_text}`: Text string (`modified` or `clean`)

##### Composites
These inject pre-styled groups of cells (e.g., from tab/status systems) and remain opaque to powerline text splitting:
- `{preview}`: Markdown preview breadcrumbs (path/name) and prev/next links
- `{detail}`: Notes list item detail row (modification date/time and tags list)
- `{hints}`: Active view mode keybind shortcuts help bar
- `{badge}`: External editor status badge (`ext:on`/`ext:off`)
- `{pending}`: Pending keybind sequence buffer indicator

---

### `[image]`

| Option | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Master toggle for native pixel image rendering |
| `max_dimension` | `u32` | `2048` | Maximum decode dimension in pixels |
| `cache_size` | `usize` | `32` | LRU cache entry count |
| `preview_rows` | `u8` | `8` | Rows occupied by preview images |
| `attachments_subdir` | `String` | `"attachments"` | Subdirectory for pasted/imported image attachments |

Example:
```toml
[image]
enabled = true
max_dimension = 2048
cache_size = 32
preview_rows = 8
attachments_subdir = "attachments"
```

### `[graf]`

| Option | Type | Default | Description |
|---|---|---|---|
| `max_node` | `usize` | `500` | Maximum number of nodes to simulate and display (0 = unlimited) |
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
| `show_orphan` | `bool` | `false` | Show isolated notes with no valid links |

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
[core]
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
pinned = "#ffaa00"
smart = "#ff66aa"
subnote = "#66ccff"

[graf]
max_node = 500
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

> Note: digits `1`–`7` jump directly to the first seven help tabs (Notes→Templates); the eighth tab (About) is reached via `Tab`/`Right`. These are fixed and not configurable in `keybinds.toml`.

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

### Outline Actions (`[outline]`)

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
