# Configuration Reference

Full reference of all configuration options for clin-rs.

## Folder-local note template variables

Note template variables use a separate `.clin-template.toml` file inside a physical vault folder, not a section of the global config:

```toml
inherit_parent = true

[variables]
book = "Example Book"
chapter = "04"
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `inherit_parent` | Boolean | `true` | Include ancestor-folder values up to the vault root. `false` cuts off ancestors above this config. |
| `[variables]` entries | String | Empty map | Case-sensitive custom values used in note template title/body; nearer folders override ancestors. |

Names must match `[A-Za-z_][A-Za-z0-9_]*`. Built-ins (`date`, `datetime`, `time`, `weekday`, `year`, `month`, `day`) cannot be overridden. Unknown tokens stay literal and warn; invalid configuration blocks template-based creation. These note variables are separate from the UI/statusline interpolation variables documented below.

Use **Edit Folder Template Variables** in the command palette to create/edit the selected physical folder's config, or select its muted `.clin-template.toml` item and press Enter. A gear badge on the folder item indicates its own local config; inherited-only variables do not add a badge. CLI creation selects the destination with `--folder` in `clin notes new -t ...`. See [Template System](TEMPLATES.md#folder-local-custom-variables) for inheritance, validation, and CLI precedence details.

---

## Configuration

Run `clin config show` to print the active configuration file path. `clin --config <PATH>` relocates the config file, preset keybinds, and custom themes, but not application data/cache roots. `clin config reset` resets canonical config values; it keeps vault files, encryption keys, local state, caches, themes, and keybind files.

**Sources:** `src/config/structs.rs` (schema/defaults), `src/config/types.rs` (enum values), `src/config/mod.rs` (loading/migration), `src/keybinds/defaults.rs` (default bindings).

## Paths

The default vault is `<Documents>/clin Vault`. Documents resolves through platform user-directory settings, falling back to `~/Documents`; thus the fallback vault is `~/Documents/clin Vault`. Notes live directly in the vault root, including for default storage; templates live in `<vault>/.clin/templates/`.

| Data | Location |
|---|---|
| Active config | `clin config show`; Linux: `$XDG_CONFIG_HOME/clin/config.toml` or `~/.config/clin/config.toml`; macOS: `~/.config/clin/config.toml`; Windows: platform `ProjectDirs` config directory |
| Keybinds / custom themes | `<active-config-dir>/keybinds/<preset>.toml` / `<active-config-dir>/themes/*.toml` |
| Note-summary cache | `<application-cache-dir>/vaults/<vault-digest>/note_cache_plain_v4.bin`; `clin cache reset` removes current vault's cache and legacy caches |
| Templates | `<vault>/.clin/templates/` |
| Subnotes | `<vault>/.clin/subnotes.bin` (obfuscated plaintext; see [Subnotes](SUBNOTES.md)) |
| Recovery | `<vault>/.clin/document-recovery-v2/` (private plaintext drafts and transactions); legacy `editor_draft.bin` is retained, not replayed |
| Calendar | `<vault>/.clin/calendar.json` |
| Local state | `<application-data-local-dir>/state.json` (goals, vault preferences, command usage) |
| Image attachments | `<vault>/<image.attachments_subdir>/` |

Application data/cache paths come from `directories::ProjectDirs::from("com", "clin", "clin")`, independently of `--config` and `--vault`.

### `[features]`

Master feature toggles. Supported values: `true` / `"enabled"`, `false` / `"disabled"`, and `"deleted"`. Setting a flag to `false` / `"disabled"` skips the feature's startup work and hides its entry points (keybinds, palette actions, help tabs, statusline tokens). Setting it to `"deleted"` also clears its keybinds, making them available for custom mappings. Changes to `images` require a restart (it controls a background worker); all others apply at runtime via the command palette's `settings.toggle_*` actions.

Legacy per-section flags (`[image] enabled`, `[goals] enabled`, `[list] calendar_enabled`, `[list] smart_folders_enabled`) are migrated into this table automatically on first launch; those old keys are then ignored.

| Option | Type | Default | Description |
|---|---|---|---|
| `graph_view` | `enum` | `true` | Enable the force-directed Graph view |
| `canvas_view` | `enum` | `true` | Enable the `.canvas` view |
| `bases_view` | `enum` | `true` | Enable native `.base` views, authoring and live public embeds/sidebar |
| `draw_view` | `enum` | `true` | Enable the `.draw` view |
| `outline_view` | `enum` | `true` | Enable the Outline view (overlay + editor sidebar) |
| `help_view` | `enum` | `true` | Enable the in-app Help view |
| `tags` | `enum` | `true` | Enable the tag manager and tag UI surfaces |
| `trash` | `enum` | `true` | Enable the trash view/restore UI (deletes still go to trash) |
| `subnotes` | `enum` | `true` | Enable attached virtual notes, their section and manager |
| `templates` | `enum` | `true` | Enable note templates |
| `import` | `enum` | `true` | Enable import (File/CSV/JSON/URL/Clipboard) and OCR paste |
| `images` | `enum` | `true` | Enable native pixel image rendering (restart required) |
| `goals` | `enum` | `true` | Enable the daily word/note goals system |
| `calendar` | `enum` | `true` | Show the Notes widget strip (Calendar view remains available) |
| `smart_folders` | `enum` | `true` | Enable virtual smart folders (Today, This Week, Untagged) |

### `[core]`

| Option | Type | Default | Description |
|---|---|---|---|
| `storage_path` | `PathBuf` | platform default | Custom vault storage path. Supports `~` and `$VAR`/`${VAR}` expansion (e.g., `~/notes`, `$HOME/vault`) |
| `vaults` | `array<PathBuf>` | `[]` | Extra vaults shown in the F4 vault switcher. Supports `~` and `$VAR`/`${VAR}` expansion. Vault removal via the switcher removes entries from this list; files on disk are never touched. Not persisted while `--vault` override is active |
| `vault_names` | `map<String, String>` | `{}` | Display names keyed by vault path; renamed through the vault switcher |
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
| `enable_key_sequences` | `bool` | `false` | Enable multi-key sequences (e.g. `"g g"`, `"Space f"`). Sequence-capable presets enable them automatically |
| `preview_expand_mode` | `enum` | `"inline"` | Ctrl+e behavior: `"inline"` (maximize the preview pane) or `"external"` (run `preview_command` on the note) |
| `preview_command` | `String` | — | Command for external preview (Ctrl+e when `preview_expand_mode = "external"`). Shell-split with the note's temp file appended; falls back to `$PAGER`, then `less` |
| `auto_refresh` | `bool` | `true` | Reload notes list on external file changes (notify watcher) |

### `[list]`

| Option | Type | Default | Description |
|---|---|---|---|
| `preview_enabled` | `bool` | `true` | Show the preview pane in notes list by default |
| `preview_position` | `enum` | `"right"` | Preview pane position: `"left"`, `"right"` |
| `show_file_size` | `bool` | `false` | Show file size in the notes list |
| `date_format` | `String` | `"%Y-%m-%d"` | Date format for inline note metadata (chrono format) |
| `density` | `enum` | `"compact"` | Density of the notes list: `"comfortable"` or `"compact"` |
| `default_view` | `enum` | `"grid"` | Notes layout: `"grid"` tiles; `"tree"` expanding folders; `"list"` directory rows; `"multi"` directory rows with selected-folder contents on the right |
| `section_tabs` | `bool` | `false` | Show Vault, Pinned, Smart, and Subnotes as header tabs in all four layouts; otherwise show them as folder items |
| `default_sort_field` | `enum` | — | Optional initial sort field: `"title"` or `"modified"` |
| `default_sort_order` | `enum` | — | Optional initial sort order: `"ascending"` or `"descending"` |
| `inline_info` | `bool` | `true` | Show inline metadata info (modification date, tags) in the notes list |
| `pinned_on_top` | `bool` | `false` | Keep pinned notes at the top of the list |
| `show_hidden_files` | `bool` | `false` | Show hidden files and folders (starting with ".") in the notes list |
| `show_all_files` | `bool` | `false` | Show every file in the vault, not just notes (.md/.txt/.clin/.draw/.canvas/.base). Non-note files open in the OS default application |
| `skip_dirs` | `array<String>` | `[]` | Regular expressions matched against directory names; invalid expressions are ignored |
| `folders_first` | `bool` | `true` | Show subfolders before files in every notes layout |
| `preview_width_ratio` | `f32` | `0.43` | Preview pane width ratio (0.2–0.8) |
| `calendar_height` | `u16` | `9` | Calendar height in rows (9–20) |
| `calendar_position` | `enum` | `"bottom"` | Calendar position: `"top"`, `"bottom"` |
| `week_start` | `enum` | `"sunday"` | Start day for Calendar and rolling-week activity: `"sunday"` or `"monday"` |
| `sections` | `array` | `["calendar","goals"]` | Bottom-strip widgets (max 2): `calendar`, `activity`, `goals`, `draw`, `graf`, `todo` |
| `folder_graph_preview` | `bool` | `false` | Show graph preview for folders |
| `pinned_folders` | `array` | `[]` | List of always-pinned folder paths |
| `default_expand_depth` | `usize` | — | Default tree expand depth (`None` = remember per-folder state) |
| `custom_smart_folders` | `array` | `[]` | User-defined smart folder rules. Each entry: `{name, tags=[], title_contains=..., folder_prefix=..., updated_within_days=...}` |

The **Select Notes Layout** command opens a searchable list with the current layout selected. Choose a row and press Enter; Esc returns without applying another layout.

**Toggle Notes Section Tabs** switches `list.section_tabs` for every layout and saves the choice. With tabs off (the default), Tree expands section folders in place, while Grid, List, and Multi open them as directories. With tabs on, all layouts display only the active section; Tab/Shift+Tab or a mouse click changes sections. Smart and Subnotes follow their feature settings.

Directory layouts (`grid`, `list`, `multi`) navigate into folders. In List and Multi, Enter or Right opens a folder; Left returns to its parent. Grid keeps its tile-navigation shortcuts. Multi shows the selected folder's children in the right column; clicking a child enters that folder and selects the child. A browser narrower than 24 cells uses one column.

Scrollbars share adjacent pane dividers in Notes, Edit (preview or sidebar), and Outline, and share popup list borders while preserving their corners. Scrollbar colors stay unchanged on mouse hover. Layout editing resizes the preview by one terminal cell per horizontal keypress. Every main view except Setup shares header and footer bars controlled by `hint_bar_style` and the statusline templates. Zen mode keeps its hidden editor header and visible footer. Header tabs remain plain muted text with an accent selection; notifications temporarily replace the header and disable its tabs. Normal footer bars show at most five primary shortcuts; selection and layout-edit modes show their complete mode-specific controls. Crowded footers hide the clin version watermark first, then shorten with an ellipsis as needed, preserving status information and Powerline edges. Help side panes wrap to the available cells and support mouse-wheel scrolling, so their content remains reachable when larger fonts reduce the terminal size.

### `[editor]`

| Option | Type | Default | Description |
|---|---|---|---|
| `external_command` | `String` | — | External editor command (e.g. `"nvim"`, `"code"`) |
| `external_enabled` | `bool` | `false` | Enable external editor mode |
| `preview_enabled` | `bool` | `false` | Show markdown preview panel in editor by default |
| `show_line_numbers` | `bool` | `true` | Show line numbers in the editor |
| `frontmatter_mode` | `enum` | `"properties"` | F7 presentation: `"properties"` or `"raw_yaml"`; region remains hidden initially |
| `date_format` | `String` | `"%Y-%m-%d %H:%M"` | Format used by the insert-date action |
| `soft_wrap` | `bool` | `false` | Soft-wrap the editor body |
| `copy_on_select` | `bool` | `true` | Copy to clipboard immediately when a mouse drag selects text; `false` keeps the selection for the copy keybind |
| `edit_mode_highlight` | `bool` | `true` | Apply Markdown syntax highlighting to the editable body |
| `ghost_syntax` | `bool` | `true` | Visually dim markdown delimiters (brackets, URLs, etc.) in edit view |
| `extended_markdown_features` | `bool` | `true` | Enable extended markdown highlighting features (bare URLs, bold italic combinations, description lists, footnotes) |
| `text_align` | `enum` | `"left"` | Text alignment (`"left"`, `"center"`, `"right"`, `"justified"`). Active only when `soft_wrap` is `true` |
| `zen_padding_percent` | `u16` | `15` | Percent of terminal width padded on each side in zen mode; clamped to 45 |
| `zen_hide_line_numbers` | `bool` | `true` | Hide line numbers while zen mode is active |
| `zen_hide_scrollbar` | `bool` | `true` | Hide scrollbar while zen mode is active |
| `zen_focus_dimming` | `bool` | `false` | Dim content outside the active neighborhood in zen mode |
| `zen_focus_unit` | `enum` | `"paragraph"` | Unit for focus neighborhood (`"paragraph"`, `"line"`) |
| `zen_focus_context` | `usize` | `3` | Number of units above the cursor to keep bright |
### `[notes.frontmatter]`

Automatic note-output policy. Existing body-only saves do not add absent managed fields. Disabled fields retain their existing bytes; explicit Title/property/YAML/tag/pin/alignment edits still save. New notes use enabled output. Generated links are unique in first-seen order. Filenames never follow title changes automatically.

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `title` | `bool` | `true` | Automatic initial title output |
| `tags` | `bool` | `true` | Automatic initial tags output |
| `pinned` | `bool` | `true` | Automatic initial pin output; explicit pin actions always persist |
| `links` | `bool` | `true` | Generated body wikilinks |
| `text_align` | `bool` | `true` | Automatic alignment output where produced; explicit alignment always persists |
| `updated_at` | `enum` | `"unix"` | `"unix"`, `"iso8601"` (UTC RFC 3339), or `"none"` |
| `original_ext` | `bool` | `true` | Accepted compatibility no-op; there is no automatic extension producer |

```toml
[notes.frontmatter]
links = false
updated_at = "iso8601"
```

Unix, ISO date (`YYYY-MM-DD`), and RFC 3339 timestamps are accepted on read. Disabled or uninterpretable fields are preserved. To maintain a field manually, disable its automatic output; changing the policy does not delete any existing value. The retired `notes.rename_on_title_change` option remains retired. Nested policy settings and comments survive config cleanup.

### `[ui]`


| Option | Type | Default | Description |
|---|---|---|---|
| `theme` | `enum` | `"default"` | Color theme. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for all 19 options |
| `background` | `enum` | `"transparent"` | Background mode: `"transparent"`, `"solid"` |
| `show_status_bar` | `bool` | `true` | Show Graph shortcut hints; the shared footer always remains visible |
| `tab_icons_only` | `bool` | `false` | Show only icons on tab bars; text labels remain in `"none"` mode |
| `icon_mode` | `enum` | `"nerd"` | `"nerd"`: single-cell Font Awesome icons (Nerd Font required); `"unicode"`: single-cell text symbols; `"emoji"`: emoji icons (terminal emoji support required); `"none"`: text labels without decorative icons. Unicode/Emoji/None use portable bar separators |
| `scrollbars` | `bool` | `true` | Show mouse-draggable scrollbars on scrollable regions |
| `scrollbar_pan_mode` | `bool` | `false` | Notes list scrollbar pans without moving selection |
| `hint_bar_style` | `enum` | `"classic"` | Hint/status bar style: `"classic"`, `"sharp"`, `"rounded"`, `"slanted"`, `"bubbles"`, `"blurred"`, `"chips"`, `"brackets"`, `"compact"`, `"sharp_gradient"`, `"rounded_gradient"`, `"slanted_gradient"`, `"hexagon"` |
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
- `[statusline.outline]` — overrides for the outline view

Bases and Calendar use the global templates; `[statusline.bases]` and
`[statusline.calendar]` are not supported. Default right-side templates vary by
view (counts, cursor, date, or version); unset values use those built-in defaults.

Each override sub-table accepts: `header_left`, `header_right`, `footer_left`, and `footer_right` fields.

#### Template Interpolation Variables

Variables are enclosed in `{}` (e.g. `{time}`). Escapes `{{` and `}}` render literal braces. Unknown variables render literally (e.g. `{bogus}`).

##### Global / App (all views)
- `{view}`: Active view name (`Notes`, `Editor`, `Help`, `Graph`, etc.)
- `{status}`: Current status text or empty if "Ready"
- `{vault}`: Base folder name of the vault directory
- `{vault_path}`: Full absolute path to the vault directory
- `{version}`: Application package version
- `{fps}`: Measured render rate; Graph uses its adapter metric (`--` until available)

##### Date & Time (all views)
- `{time}`: Current local time (`%H:%M`)
- `{date}`: Current local date formatted per `date_format` configuration
- `{datetime}`: Local date and time combined
- `{weekday}`: Full weekday name (e.g., `Monday`)
- `{year}`, `{month}`, `{day}`, `{hour}`, `{minute}`, `{second}`: Respective time parts

##### Config Echo (all views)
- `{theme}`: Active theme name
- `{preset}`: Keybind preset (`default`, `helix`, `vim`, `emacs`)
- `{icon_mode}`: Icon mode (`nerd`, `unicode`, `emoji`, `none`)
- `{hint_bar_style}`: Style variant (`classic`, `sharp`, `rounded`, `slanted`, `bubbles`, `blurred`, `chips`, `brackets`, `compact`, `sharp_gradient`, `rounded_gradient`, `slanted_gradient`, `hexagon`)
- `{background}`: Background style (`transparent`, `solid`)

##### Goals (all views)
- `{goal_words}`: Words written today
- `{goal_target}`: Daily word count goal
- `{goal_notes}`: Notes modified today
- `{goal_note_target}`: Daily note count goal
- `{goal_date}`: Tracked date for goals progress

##### List View (`Notes` view)
- `{title}`: Header title text (e.g. `"Notes"`, `"Notes - Editing Layout"`)
- `{sort}`: Styled sort indicator (such as `A-z` or modified-date direction)
- `{sort_field}`: Current sorting field (`title`, `modified`)
- `{sort_order}`: Current sorting order (`ascending`, `descending`)
- `{layout}`: Active notes layout (`tree`, `grid`, `list`, `multi`)
- `{density}`: Active list density (`compact`, `comfortable`)
- `{section}`: Active notes section (`sections`, `vault`, `pinned`, `smart`, `subnotes`)
- `{folder}`: Active notes directory path
- `{folder_count}`: Number of folders in the cache
- `{tag_count}`: Number of unique live tags in the vault
- `{note_count}`: Total notes in the notes directory
- `{visual_index}`: 1-based cursor index in the list
- `{visual_total}`: Total visible items in the list
- `{selected_count}`: Number of selected notes
- `{select_mode}`: `on`/`off` depending on selection mode
- `{tag_to_assign}`: Name of tag to assign, or empty
- `{search}`: Active search query text
- `{grep}`: Search content-mode status (`on`/`off`)
- `{tag_filter}`, `{folder_filter}`: Active query filter parameters
- `{pinned_count}`: Total pinned notes
- `{pinned_on_top}`, `{folders_first}`, `{list_preview}`, `{calendar}`, `{layout_edit}`: Config and view states (`on`/`off`)

##### Bases View

Bases uses the global statusline templates. Its header contains saved-view tabs and `{bases_file}` (the public document ID). Tabs switch saved views without rewriting the document; each view keeps its configured layout. Narrow headers keep the active tab visible; keyboard switching reaches every saved view. Footer `{detail}` contains public query counts, checked rows, active search and refresh status. `{bases_rows}` is the displayed public
row count; `{bases_matched}` counts matches before the limit; `{bases_selected}`
counts checked rows. `{bases_view}` and `{bases_property}` identify the selected
saved view and displayed property. In Bases, `{visual_total}`, `{note_count}` and
`{selected_count}` resolve to these public query counts. `{visual_index}` is the
1-based result row, or zero on a group heading. Encrypted Notes context is absent.

##### Calendar View

Calendar uses the global statusline templates. `{calendar_date}` is the selected
date formatted with `date_format` (invalid or time-only directives fall back to
`YYYY-MM-DD`); the default header shows it beside plain
Month/Week/Day/Agenda tabs. Footer `{detail}` shows errors, sync conflicts and
sync progress, with errors taking priority. Warning information takes precedence
over the version watermark on narrow terminals. Custom templates can place
`{detail}` elsewhere or omit it. Calendar panes use borderless headings and the
existing darker preview background for the event sidebar; transparent themes
retain terminal transparency. Popup controls keep their existing styling.

##### Note Context (List + Edit views)
- `{note_title}`: Title of the selected/edited note
- `{note_id}`: Filename/ID of the note
- `{note_folder}`: Folder directory of the note
- `{note_format}`: File format extension (`md`, `txt`, `clin`, `draw`, `canvas`, `base`)
- `{note_size}`: Note file size formatted (e.g. `1.2 KB`)
- `{note_links}`: Number of wikilinks in the note
- `{tags}`: Comma-separated tags of the note
- `{has_tags}`: `on`/`off` depending on tag presence
- `{note_pinned}`: Note pinned status (`on`/`off`)
- `{note_updated}`, `{note_updated_rel}`: Absolute date and relative time of last update
- `{prev_note}`, `{next_note}`: Filenames of the previous and next notes in visual order

##### Editor (`Editor` view)
- `{word_count}`: Current document total word count (remains total only, even if text is selected)
- `{word_count_display}`: Document total word count, or `(selected) total` if text is selected
- `{line_count}`: Total lines in the editor
- `{char_count}`: Document total character count, including whitespace and line breaks (Unicode scalars, not bytes; remains total only)
- `{char_count_display}`: Document total character count, or `(selected) total` if text is selected; uses the same character-count rules as `{char_count}`
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

The default editor header uses both selection-aware counts. Custom headers can opt in:

```toml
[statusline.edit]
header_right = "{word_count_display}w {char_count_display}c {cursor_line}:{cursor_col}"
```


##### Help View

- `{help_page}`: Current 1-based keybind page
- `{help_total_pages}`: Total keybind pages for the active tab

##### Graph View (`Graph` view)
- `{node_count}`, `{edge_count}`: Graph nodes and edges
- `{selected_node}`: Label of the selected node or `"none"`
- `{viewport_size}`: Graph viewport coverage percentage (e.g. `"45%"`)
- `{scale}`: Graph zoom scale relative to auto-fit (e.g. `"1.00×"`)
- `{graph_settled}`: Force-directed simulation settlement status (`on`/`off`)
- `{label_mode}`, `{node_color_mode}`, `{edge_color_mode}`, `{node_size_mode}`, `{zoom}`: Graph configuration and zoom status
- `{show_grid}`, `{show_legend}`, `{show_minimap}`: Current grid, legend, and minimap visibility (`on`/`off`)

##### Draw View (`Draw` view)
- `{tool}`: Active draw tool (`cursor`, `draw`, `erase`, `text`, `shape`)
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
| `cache_size` | `usize` | `32` | LRU cache entry count |
| `preview_rows` | `u8` | `8` | Rows occupied by preview images |
| `attachments_subdir` | `String` | `"attachments"` | Subdirectory for pasted/imported image attachments |

Example:
```toml
[features]
images = true

[image]
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
| `node_scale` | `integer or string` | `5` | Screen-space node scale: integer 1–10 or `"automatic"` |
| `node_fill` | `enum` | `"dynamic"` | Fill style: `"dynamic"`, `"filled"`, `"none"` |
| `selection_focus` | `enum` | `"grow"` | Selected-node emphasis: `"none"`, `"grow"`, `"dim"`, `"grow_dim"` |
| `edge_thickness` | `u16` | `1` | Edge line thickness (1–3) |
| `show_legend` | `bool` | `true` | Show legend |
| `show_minimap` | `bool` | `false` | Show minimap |
| `minimap_position` | `enum` | `"top_right"` | Minimap corner: `"top_right"`, `"top_left"`, `"bottom_right"`, `"bottom_left"` |
| `minimap_width` | `u16` | `24` | Minimap width in cells |
| `minimap_height` | `u16` | `12` | Minimap height in cells |
| `canvas_marker` | `enum` | `"braille"` | Canvas rendering marker: `"braille"`, `"half_block"`, `"dot"` |
| `node_shape` | `enum` | `"circle"` | Node shape: `"circle"`, `"square"`, `"diamond"` |
| `label_offset` | `f64` | `4.0` | Distance of labels from nodes |
| `show_looking_glass` | `bool` | `true` | Show zoom looking glass around cursor |
| `looking_glass_width` | `u16` | `24` | Width of the looking glass in cells |
| `looking_glass_height` | `u16` | `12` | Height of the looking glass in cells |
#### `[graf.visual.colors]`

All optional. Hex color strings like `"#ff6600"`. Override theme defaults.

| Option | Type | Default |
|---|---|---|
| `node_color` | `String` | Theme default |
| `edge_color` | `String` | Theme default |
| `label_color` | `String` | Theme default |
| `selection_ring_color` | `String` | Theme default |
| `border_color` | `String` | Theme default |
| `background_color` | `String` | Theme default |

### `[graf.physics]`

| Option | Type | Default | Description |
|---|---|---|---|
| `ideal_distance` | `f64` | `80.0` | Target distance between connected nodes |
| `tick_rate` | `enum` | `"auto"` | Simulation tick rate: `"auto"`, `"fixed"` |

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
### `[goals]`

| Option | Type | Default | Description |
|---|---|---|---|
| `word_goal` | `usize` | `500` | Daily target word count (incremental additions). Set to 0 to disable |
| `note_goal` | `usize` | `3` | Daily target note count (edited or created). Set to 0 to disable |
---

## Example config.toml

```toml
[features]
calendar = true
goals = true
images = true
smart_folders = true

[core]
# storage_path supports ~ and $VAR/${VAR} expansion: "~/notes", "$HOME/vault"
storage_path = "/path/to/your/vault"
mouse_enabled = true
confirm_on_delete = true

[list]
preview_enabled = true
preview_position = "right"
show_file_size = false
date_format = "%Y-%m-%d"
density = "comfortable"
default_view = "grid"
section_tabs = false
default_sort_field = "modified"
default_sort_order = "descending"
pinned_on_top = true

[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = true
show_line_numbers = true

[goals]
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

## Keybinds

Keybind files are stored under the active configuration directory at `keybinds/<preset>.toml`. Runtime help shows the active preset path. Tables below describe the **default** preset; feature toggles may hide bindings.

### Global Palette Binding (`[global]`)

| Action | Default Keys | Description |
|---|---|---|
| `command_palette` | `Ctrl+Space`, `F6` | Contextual palette in all ten views; matching view bindings and modal input take priority |

### Global Function Keys

| Key | Action |
|---|---|
| `F1` | Contextual help (except Setup); Calendar uses its own help dialog; closes Help when already open |
| `F2` | Quick keybind dropdown |
| `F3` | Message history overlay |
| `F4` | Vault switcher |
| `F5` | Force full redraw |

These global handlers take precedence over per-view bindings when modal input
does not own the key. In particular, F2 opens Quick Keybinds in Bases; use `e` to
edit a property.

### List Actions (`[list]`)

| Action | Default Keys | Description |
|---|---|---|
| `move_up` | `Up`, `k` | Move up |
| `move_down` | `Down`, `j` | Move down |
| `move_left` | `Left`, `h` | Move left (grid) |
| `move_right` | `Right`, `l` | Move right (grid) |
| `open` | `Enter`, `o` | Open selected item |
| `delete` | `d`, `Delete` | Delete |
| `quit` | `q` | Quit |
| `help` | `?` | Help |
| `open_location` | `Ctrl+l` | Open file location |
| `cycle_focus` | `Tab` | Cycle focus between panes |
| `reverse_cycle_focus` | `BackTab` | Reverse cycle focus between panes |
| `confirm` | `y`, `Enter` | Confirm action |
| `cancel` | `n`, `Esc` | Cancel action |
| `toggle_external_editor` | `Alt+e` | Toggle external editor |
| `new_from_template` | `t` | New note from template |
| `create_folder` | `Shift+N` | Create new folder |
| `create_note` | `n` | Create new note |
| `rename_folder` | `r` | Rename folder |
| `move_note` | `m` | Move note or folder |
| `move_to_parent` | `U` | Move note to parent folder |
| `manage_tags` | `.` | Manage tags |
| `remove_tags_from_selected` | `Ctrl+.` | Remove tags from selected notes |
| `open_command_palette` | `:`, `Ctrl+p` | Open command palette |
| `rename` | `r` | Rename note |
| `duplicate` | `y` | Duplicate note |
| `toggle_pin` | `p` | Toggle pin |
| `cycle_sort` | `s` | Cycle sort order |
| `search` | `/` | Search |
| `jump_to_top` | `Home`, `Ctrl+Up` | Jump to top |
| `page_up` | `Ctrl+u`, `PageUp` | Scroll up half page |
| `page_down` | `Ctrl+d`, `PageDown` | Scroll down half page |
| `jump_to_bottom` | `End`, `Ctrl+Down` | Jump to bottom |
| `open_trash` | `Shift+T` | Open trash |
| `toggle_preview` | `Shift+P` | Toggle preview pane |
| `toggle_preview_fullscreen` | `Ctrl+e` | Toggle preview fullscreen |
| `toggle_wrap` | `Ctrl+w` | Toggle word wrap (editor and preview) |
| `preview_page_up` | `Shift+Up` | Page preview up |
| `preview_page_down` | `Shift+Down` | Page preview down |
| `toggle_calendar` | `Shift+C` | Toggle calendar |
| `toggle_folders_first` | `Ctrl+h` | Toggle folders-first sort |
| `open_graph` | `Ctrl+g` | Open graph view |
| `toggle_select_mode` | `v` | Toggle select mode |
| `toggle_select_item` | `Space` | Toggle select item |
| `collapse_all` | `c` | Collapse all folders |
| `expand_all` | `e` | Expand all folders |
| `expand_to_level` | `Shift+E` | Expand folders to level (e.g. 3E) |
| `refresh_notes` | `Ctrl+r` | Refresh notes (external changes) |
| `manage_subnotes` | `Alt+s` | Manage subnotes |
| `show_info` | `i` | Show note info |

### Edit Actions (`[edit]`)

| Action | Default Keys | Description |
|---|---|---|
| `back` | `Esc` | Return to notes (auto-saves) |
| `save` | `Ctrl+s` | Save |
| `cycle_focus` | `Ctrl+t` | Cycle focus (Title, visible Frontmatter, Body, visible Sidebar) |
| `insert_tab` | `Tab` | Insert tab character |
| `select_all` | `Ctrl+a`, `Ctrl+Shift+a` | Select all |
| `copy` | `Ctrl+c` | Copy |
| `cut` | `Ctrl+x` | Cut |
| `paste` | `Ctrl+v` | Paste |
| `undo` | `Ctrl+z` | Undo |
| `redo` | `Ctrl+y`, `Ctrl+Shift+z` | Redo |
| `delete_word` | `Ctrl+Backspace` | Delete previous word |
| `delete_next_word` | `Ctrl+Delete` | Delete next word |
| `move_to_top` | `Ctrl+Home` | Move cursor to top |
| `move_to_bottom` | `Ctrl+End` | Move cursor to bottom |
| `toggle_frontmatter` | `F7` | Show/hide the properties/raw-YAML region |
| `toggle_mermaid_rendering` | `Alt+m` | Toggle Mermaid diagrams in the main editor preview |
| `toggle_bases` | `Ctrl+g Ctrl+b` | Toggle the active-file Bases sidebar |
| `toggle_markdown_preview` | `Ctrl+p` | Toggle markdown preview |
| `toggle_preview_fullscreen` | `F11` | Toggle preview fullscreen |
| `toggle_zen_mode` | `F8` | Toggle zen mode |
| `cycle_alignment` | `F9` | Cycle text alignment (requires soft wrap) |
| `toggle_wrap` | `F10` | Toggle word wrap (editor and preview) |
| `preview_page_up` | `PageUp` | Page preview up |
| `preview_page_down` | `PageDown` | Page preview down |
| `manage_subnotes` | `Alt+s` | Manage subnotes |
| `paste_image` | `Ctrl+g Ctrl+i` | Paste image from clipboard |
| `insert_image_from_file` | `Ctrl+g Ctrl+f` | Insert image from file |
| `find` | `Ctrl+f` | Find in document |
| `go_to_line` | `Ctrl+g` | Go to line number |
| `insert_date` | `Ctrl+;` | Insert date/time |
| `toggle_outline` | `Ctrl+o` | Toggle outline pane |
| `toggle_links` | `Ctrl+b` | Toggle links pane |
| `preview_link` | `Alt+l` | Preview linked note under cursor |

### Help Actions (`[help]`)

| Action | Default Keys | Description |
|---|---|---|
| `close` | `Esc`, `q`, `?`, `F1` | Close help |
| `next_tab` | `Right`, `l`, `Tab` | Next help tab |
| `prev_tab` | `Left`, `h`, `BackTab` | Previous help tab |
| `scroll_up` | `Up`, `k` | Scroll up |
| `scroll_down` | `Down`, `j` | Scroll down |
| `search` | `/`, `Ctrl+f` | Search help |
| `reroll` | `r` | Reroll help tips |

> Digits `1`–`8` jump to visible help tabs by position; disabled-feature tabs are omitted. These shortcuts apply when no configured help binding matches.

### Graph Actions (`[graph]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Quit graph view |
| `pan_up` | `Up`, `k` | Pan up |
| `pan_down` | `Down`, `j` | Pan down |
| `pan_left` | `Left`, `h` | Pan left |
| `pan_right` | `Right`, `l` | Pan right |
| `zoom_in` | `+`, `=` | Zoom in |
| `zoom_out` | `-`, `_` | Zoom out |
| `open_note` | `Enter`, `o` | Open selected note |
| `auto_fit` | `a` | Auto-fit graph to viewport |
| `help` | `?` | Help |
| `toggle_search` | `/` | Search nodes |
| `toggle_minimap` | `Shift+M` | Toggle minimap |
| `toggle_legend` | `Shift+L` | Toggle legend |
| `toggle_grid` | `Shift+G` | Toggle background grid |
| `toggle_status` | `Shift+S` | Toggle Graph shortcuts |
| `refresh` | `r` | Refresh physics |
| `toggle_preview` | `Shift+P` | Toggle preview |
| `create_connection` | `c` | Create connection |
| `delete_connection` | `d` | Delete connection |
| `local_graph` | `l` | Local graph |
| `show_group` | `g` | Show group |
| `delete_node` | `x` | Delete node |
| `menu_close` | `Esc` | Close menu |
| `menu_up` | `Up`, `k` | Menu up |
| `menu_down` | `Down`, `j` | Menu down |
| `menu_select` | `Enter` | Select menu item |
| `looking_glass` | `Shift+O` | Toggle looking glass |

### Draw Actions (`[draw]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Exit draw view |
| `help` | `?` | Help |
| `select_draw_tool` | `d` | Draw freehand strokes |
| `select_cursor_tool` | `a` | Select and transform element |
| `toggle_shape_selector` | `s` | Shape tool (opens picker) |
| `select_text_tool` | `t` | Place text label |
| `select_erase_tool` | `e` | Erase elements |
| `shape_selector_up` | `Up`, `k` | Select previous shape |
| `shape_selector_down` | `Down`, `j` | Select next shape |
| `shape_selector_confirm` | `Enter` | Confirm shape selection |
| `shape_selector_cancel` | `Esc`, `q` | Cancel shape selection |
| `toggle_color_selector` | `c` | Color picker (opens picker) |
| `color_selector_up` | `Up`, `k` | Select previous color |
| `color_selector_down` | `Down`, `j` | Select next color |
| `color_selector_confirm` | `Enter` | Confirm color selection |
| `color_selector_cancel` | `Esc`, `q` | Cancel color selection |
| `text_editor_confirm` | `Enter` | Confirm text edit |
| `text_editor_cancel` | `Esc` | Cancel text edit |
| `menu_close` | `Esc` | Close context menu |
| `menu_up` | `Up`, `k` | Select previous menu item |
| `menu_down` | `Down`, `j` | Select next menu item |
| `menu_select` | `Enter` | Activate selected menu item |
| `copy` | `c` | Copy selected element |
| `paste` | `v` | Paste copied element |
| `undo` | `Ctrl+z` | Undo last draw change |
| `redo` | `Ctrl+y`, `Ctrl+Shift+z` | Redo last draw change |
| `toggle_grid` | `Shift+G` | Toggle grid overlay |

### Canvas Actions (`[canvas]`)

| Action | Default Keys | Description |
|---|---|---|
| `quit` | `Esc`, `q` | Quit canvas view |
| `undo` | `Ctrl+z` | Undo last canvas edit |
| `redo` | `Ctrl+y`, `Ctrl+Shift+z` | Redo canvas edit |
| `save` | `Ctrl+s` | Save canvas file |
| `zoom_fine_in` | `>`, `]` | Zoom in (fine) |
| `zoom_fine_out` | `<`, `[` | Zoom out (fine) |
| `zoom_in` | `+`, `=` | Zoom in |
| `zoom_out` | `-`, `_` | Zoom out |
| `move_left` | `Left`, `h` | Move left |
| `move_right` | `Right`, `l` | Move right |
| `move_up` | `Up`, `k` | Move up |
| `move_down` | `Down`, `j` | Move down |
| `edit_or_connect` | `i`, `Enter` | Open / edit / connect |
| `open_context_menu` | `a` | Context menu |
| `create_connection` | `c` | Create connection |
| `delete_connection` | `d` | Delete connection |
| `rename_node` | `r` | Rename selected node |
| `resize_mode` | `s` | Enter resize mode |
| `set_color` | `o` | Set color of selected node(s) |
| `delete_node` | `x` | Delete selected node(s) |
| `delete_all_connections` | `b` | Delete all connections |
| `add_text_node` | `t` | Add text node at cursor |
| `add_group` | `g` | Add group at cursor |
| `add_image_node` | `m` | Add image node at cursor |
| `toggle_grid` | `Shift+G` | Toggle grid |
| `toggle_orthogonal` | `Ctrl+o` | Toggle orthogonal edge routing |
| `toggle_editor_pane` | `Ctrl+e` | Toggle editor pane |
| `cycle_focus` | `Tab`, `BackTab` | Cycle focus |
| `help` | `?` | Help |
| `rename_confirm` | `Enter` | Rename confirm |
| `rename_cancel` | `Esc` | Rename cancel |
| `menu_close` | `Esc` | Close context menu |
| `menu_up` | `Up` | Menu select up |
| `menu_down` | `Down` | Menu select down |
| `menu_select` | `Enter` | Menu confirm |
| `close_editor` | `Esc` | Close editor |
| `close_editor_alt` | `Ctrl+Enter` | Close editor (alt) |
| `confirm_resize` | `Enter` | Resize confirm |
| `cancel_resize` | `Esc` | Resize cancel |
| `editor_unfocus` | `Esc` | Exit editor focus |

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
| `help` | `?` | Show help |

### Bases Actions (`[bases]`)

| Action | Default Keys | Description |
|---|---|---|
| `back` | `Esc`, `q` | Return to the host view |
| `help` | `?` | Contextual Bases help |
| `move_up` | `k`, `Up` | Previous row |
| `move_down` | `j`, `Down` | Next row |
| `move_left` | `h`, `Left` | Previous column/group |
| `move_right` | `l`, `Right` | Next column/group |
| `page_up` | `PageUp`, `Ctrl+u` | Previous page |
| `page_down` | `PageDown`, `Ctrl+d` | Next page |
| `first` | `Home` | First row |
| `last` | `End` | Last row |
| `open` | `Enter` | Open public source/image or toggle a group |
| `refresh` | `r` | Refresh public metadata and query |
| `next_view` | `Tab` | Next saved view |
| `previous_view` | `BackTab` | Previous saved view |
| `edit` | `e`, `F2` | Edit property; F2 is intercepted by global Quick Keybinds |
| `clear` | `Delete` | Clear property with confirmation |
| `copy` | `y` | Copy property value |
| `paste` | `p` | Paste property value |
| `toggle_select` | `Space` | Toggle checked row |
| `select_all` | `Ctrl+a` | Select visible results |
| `undo` | `u` | Guarded undo |
| `redo` | `Ctrl+r` | Guarded redo |
| `configure` | `:` | Open Bases palette category |
| `command_palette` | `Ctrl+p` | Open shared command palette |
| `new_note` | `n` | Create public Markdown note |
| `export` | `x` | Export displayed values |
| `move_to_group` | `m` | Assign grouping property/move to folder |
| `search` | `/` | Find displayed values |
| `toggle_group` | `z` | Collapse/expand focused group |
| `open_cover` | `v` | Preview configured cover |
| `previous_property` | `[` | Focus previous property |
| `next_property` | `]` | Focus next property |

Calendar has fixed view-local keys rather than a `[calendar]` keybind table;
see [Calendar](CALENDAR.md#keys-and-palette).

### Setup Actions (`[setup]`)

| Action | Default Keys | Description |
|---|---|---|
| `up` | `Up`, `k` | Previous option |
| `down` | `Down`, `j` | Next option |
| `cycle_next` | `Right`, `l`, `Space` | Next option value |
| `cycle_prev` | `Left`, `h` | Previous option value |
| `activate` | `Enter` | Activate row or finish on Done |
| `finish` | `Esc` | Request setup exit |

---

## Migration Note

The old `graf.toml` file is **no longer used**. All graf options (`[graf.visual]`, `[graf.physics]`, `[graf.interaction]`, `[graf.filter]`, `[graf.search]`) are now part of `config.toml`. The system auto-migrates settings from `graf.toml` on first read for backward compatibility.

---

## See Also

- [README.md](../README.md) — Quickstart, installation, CLI commands
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system details and color reference
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Graph view configuration context

## Calendar and CalDAV

The Calendar view is available from the command palette independently of the Notes widget strip. `calendar` selects the month widget; `activity` selects the note-activity heatmap.

```toml
[[calendar.caldav]]
name = "Work"
url = "https://example.org/dav/"
username = "user"
password_env = "CLIN_CALDAV_PASSWORD"
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `calendar.caldav` | array of tables | `[]` | Accounts for manual CalDAV synchronization |
| `name` | string | `""` | Display prefix for discovered calendars |
| `url` | string | `""` | HTTPS account/home/collection endpoint; required per account |
| `username` | string | `""` | Basic-auth username; configure together with `password_env` |
| `password_env` | string | `""` | Environment-variable name containing the password/app password |

Set the named variable before starting clin. Password values stay outside config. Only localhost permits HTTP; redirects and cross-origin discovery are rejected. See [Calendar](CALENDAR.md) for scheduling, sync conflicts, limits, and the local protocol simulation.
