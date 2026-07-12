use serde::{Deserialize, Serialize};

use super::structs::{
    FilterConfig, InteractionConfig, PhysicsConfig, SearchConfig, UiConfig, VisualConfig,
};

/// Merge a `toml::Value` into a `toml_edit::Item`, preserving comments/decor.
pub fn merge_toml_value(edit_item: &mut toml_edit::Item, toml_val: &toml::Value) {
    match toml_val {
        toml::Value::Table(toml_tbl) => {
            if !edit_item.is_table() {
                let decor = match edit_item {
                    toml_edit::Item::Value(v) => Some(v.decor().clone()),
                    toml_edit::Item::Table(t) => Some(t.decor().clone()),
                    _ => None,
                };
                let mut new_table = toml_edit::Table::new();
                if let Some(d) = decor {
                    *new_table.decor_mut() = d;
                }
                *edit_item = toml_edit::Item::Table(new_table);
            }
            if let Some(edit_tbl) = edit_item.as_table_mut() {
                let keys_to_remove: Vec<String> = edit_tbl
                    .iter()
                    .map(|(k, _)| k.to_string())
                    .filter(|k| !toml_tbl.contains_key(k))
                    .collect();
                for k in keys_to_remove {
                    edit_tbl.remove(&k);
                }

                for (k, v) in toml_tbl {
                    if let Some(edit_item) = edit_tbl.get_mut(k) {
                        merge_toml_value(edit_item, v);
                    } else {
                        let new_item = toml_value_to_item(v);
                        edit_tbl.insert(k, new_item);
                    }
                }
            }
        }
        toml::Value::Array(toml_arr) => {
            let is_existing_aot = matches!(edit_item, toml_edit::Item::ArrayOfTables(_));
            let is_new_aot = toml_arr.iter().any(|v| v.is_table());
            if is_existing_aot || is_new_aot {
                let mut new_aot = toml_edit::ArrayOfTables::new();
                for val in toml_arr {
                    if let toml_edit::Item::Table(t) = toml_value_to_item(val) {
                        new_aot.push(t);
                    }
                }
                *edit_item = toml_edit::Item::ArrayOfTables(new_aot);
            } else {
                let decor = match edit_item {
                    toml_edit::Item::Value(v) => Some(v.decor().clone()),
                    toml_edit::Item::Table(t) => Some(t.decor().clone()),
                    _ => None,
                };
                let mut edit_arr = toml_edit::Array::new();
                for val in toml_arr {
                    edit_arr.push(
                        toml_value_to_item(val)
                            .as_value()
                            .expect("toml_value_to_item for non-table/non-array returns value")
                            .clone(),
                    );
                }
                let mut new_item = toml_edit::Item::Value(toml_edit::Value::Array(edit_arr));
                if let Some(d) = decor
                    && let Some(v) = new_item.as_value_mut()
                {
                    *v.decor_mut() = d;
                }
                *edit_item = new_item;
            }
        }
        _ => {
            let decor = match edit_item {
                toml_edit::Item::Value(v) => Some(v.decor().clone()),
                toml_edit::Item::Table(t) => Some(t.decor().clone()),
                _ => None,
            };
            let mut new_item = toml_value_to_item(toml_val);
            if let Some(d) = decor {
                match &mut new_item {
                    toml_edit::Item::Value(v) => *v.decor_mut() = d,
                    toml_edit::Item::Table(t) => *t.decor_mut() = d,
                    _ => {}
                }
            }
            *edit_item = new_item;
        }
    }
}

/// Convert a `toml::Value` into `toml_edit::Item`.
pub fn toml_value_to_item(v: &toml::Value) -> toml_edit::Item {
    match v {
        toml::Value::String(s) => toml_edit::value(s),
        toml::Value::Integer(i) => toml_edit::value(*i),
        toml::Value::Float(f) => toml_edit::value(*f),
        toml::Value::Boolean(b) => toml_edit::value(*b),
        toml::Value::Datetime(dt) => toml_edit::value(dt.to_string()),
        toml::Value::Array(arr) => {
            if arr.iter().any(|v| v.is_table()) {
                let mut edit_aot = toml_edit::ArrayOfTables::new();
                for val in arr {
                    if let toml_edit::Item::Table(t) = toml_value_to_item(val) {
                        edit_aot.push(t);
                    } else {
                        panic!("Expected table in array of tables");
                    }
                }
                toml_edit::Item::ArrayOfTables(edit_aot)
            } else {
                let mut edit_arr = toml_edit::Array::new();
                for val in arr {
                    edit_arr.push(
                        toml_value_to_item(val)
                            .as_value()
                            .expect("toml_value_to_item for non-table/non-array returns value")
                            .clone(),
                    );
                }
                toml_edit::Item::Value(toml_edit::Value::Array(edit_arr))
            }
        }
        toml::Value::Table(tbl) => {
            let mut edit_tbl = toml_edit::Table::new();
            for (k, v) in tbl {
                edit_tbl.insert(k, toml_value_to_item(v));
            }
            toml_edit::Item::Table(edit_tbl)
        }
    }
}

/// Helper struct used during migration from old `graf.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct GrafConfigOnly {
    #[serde(default)]
    pub visual: VisualConfig,
    #[serde(default)]
    pub physics: PhysicsConfig,
    #[serde(default)]
    pub interaction: InteractionConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub search: SearchConfig,
}

/// The embedded default config template shipped with the app.
pub fn default_config_content() -> &'static str {
    r###"# Clin Configuration File

# ── Core ─────────────────────────────────────────────────────────────────────

[core]
# Custom path for notes storage (e.g., "/home/user/vault").
# Supports leading ~ and $VAR/${VAR} expansion (e.g., "~/notes", "$HOME/vault").
# If not set, defaults to the standard data directory for your OS.
# storage_path = "/path/to/your/notes"

# Enable mouse support (clicking, scrolling, panning).
mouse_enabled = true

# Default folder for new notes (relative to vault root).
# default_folder = "inbox"

# Confirm before moving a note or folder to the trash.
confirm_on_delete = true
# Enable multi-key sequences (e.g. "g g", "Space f"). Off by default; automatically
# enabled when a preset (vim/helix/emacs) uses multi-key sequences.
enable_key_sequences = false

# Keybind preset ("default", "helix", "vim", "emacs").
# Applies to all navigation surfaces; never affects text editing.
# keybind_preset = "default"
# Ctrl+e behavior: "inline" (maximize the preview pane, default) or
# "external" (suspend the TUI and run preview_command on the note).
# preview_expand_mode = "inline"

# Command used for external preview (Ctrl+e when preview_expand_mode = "external").
# Runs as a shell-words-split program with the note's temp file appended.
# Falls back to $PAGER, then "less". Examples: "glow", "bat", "mdcat".
# preview_command = "glow"


# ── Display ──

[ui]
# Theme to use ("default", "tokyo_night", "catppuccin_mocha", "onedark", "gruvbox", etc.)
# Custom: drop ~/.config/clin/themes/<name>.toml and set theme = "<name>".
# Built-in names always work; a custom file with the same name takes priority.
theme = "default"

# Background style ("transparent" or "solid")
background = "transparent"

# Show the status bar at the bottom of the screen.
show_status_bar = true

# Icon mode ("nerd", "unicode", "none"). Controls icon rendering throughout the app.
icon_mode = "nerd"

# Show only Nerd Font icons (no text) on tab bars (Help, Notes, Backup, Palette).
tab_icons_only = false
#
# Hint bar style ("classic", "sharp", "rounded", "slanted")

# Color overrides (hex strings like "#ffffff").
# accent = "#ff0000"
# heading = "#00ff00"
# success = "#0000ff"
# destructive = "#ff00ff"
# muted = "#888888"
# text = "#ffffff"
# border = "#444444"
# tag = "#ffa500"
# folder = "#00ffff"
# background_color = "#000000"


# ── Statusline ───────────────────────────────────────────────────────────────

# Customize the title bar (header, top) and status bar (footer, bottom) text.
# Templates interpolate {variables} — e.g. {time}, {word_count}, {note_count}.
# Unknown variables render literally; use {{ }} for literal braces.
# Full variable list: docs/CONFIG_REFERENCE.md → "Template Interpolation Variables".
[statusline]
# Built-in defaults shown below (commented). Uncomment a line to override.
# header_left = "{title} {preview}"
# header_right = ""
# footer_left = "{pending}{badge}{hints}"
# footer_right = ""

# Per-view overrides — each sub-table accepts the same four fields:
#   list, edit, help, graph, draw, canvas, backup, content_tree
# Example:
# [statusline.list]
# footer_right = "{note_count} notes ({selected_count} selected)"

# ── List View ─────────────────────────────────────────────────────────────────

[list]
# Show the preview pane in the notes list by default.
preview_enabled = true

# Preview position ("left", "right").
preview_position = "right"

# Hide previews of encrypted notes.
preview_encryption = false

# Show modification date in the notes list.
show_date_in_list = true

# Show file size in the notes list.
show_file_size = false

# Date format for the notes list (chrono format, e.g., "%Y-%m-%d").
date_format = "%Y-%m-%d"

# Density of the notes list ("comfortable" or "compact").
density = "compact"

# Default view mode for the notes list ("grid" or "tree").
default_view = "grid"

# Default sorting field for the notes list ("title" or "modified").
# default_sort_field = "title"

# Default sorting order ("ascending" or "descending").
# default_sort_order = "ascending"

# Keep pinned notes at the top of the list.
pinned_on_top = true

# Show hidden files and folders (starting with ".") in the notes list.
show_hidden_files = false

# Show ALL files in the vault (any extension), not just notes (.md/.txt/.clin/.draw/.canvas).
# Non-note files open in the OS default application.
show_all_files = false


# Show a month calendar (with note activity) at the bottom of the notes view.
calendar_enabled = true

# Enable smart virtual folders in the notes list (e.g. Today, Week, Untagged).
smart_folders_enabled = false

# Custom smart folder rules.
# [[list.custom_smart_folders]]
# name = "Active Projects"
# tags = ["project", "active"]
# title_contains = "draft"
# folder_prefix = "work/"
# updated_within_days = 7

# Preview pane width ratio (0.2-0.8). Default 0.43.
# preview_width_ratio = 0.43

# Calendar height in rows (9-20). Default 9.
# calendar_height = 9

# Calendar position ("top", "bottom"). Default "bottom".
# calendar_position = "bottom"
#
# Bottom-strip sections, left-to-right. Max 2. One of: calendar, goals, draw, graf.
# sections = ["calendar", "goals"]

# ── Editor ────────────────────────────────────────────────────────────────────

[editor]
# External editor command (e.g., "nvim", "code", "nano").
# external_command = "nvim"

# Enable external editor mode by default.
external_enabled = false

# Show the markdown preview in the editor view by default.
preview_enabled = false

# Show line numbers in the editor.
show_line_numbers = true

[backup]
# Enable auto-backups via git.
enabled = false

# Perform a backup commit whenever a note is saved.
backup_on_save = false

# Perform a backup commit when the app exits.
backup_on_quit = false

# Automatically push changes to the remote repository.
auto_push = false

# Remote URL for git push (e.g., "git@github.com:user/repo.git").
# remote_url = ""

# Remote name for git push (defaults to "origin").
# remote_name = "origin"
# Interval in minutes for automatic background backups.
# auto_backup_interval = 30

# ── Graph View (Graf) ─────────────────────────────────────────────────────────

[graf]
# Enable preview pane in graph view.
preview_enabled = false

[graf.visual]
# Graph background style ("solid", "transparent")
graph_background = "solid"

# Node color mode ("folder", "tag", "uniform", "link_count")
node_color_mode = "folder"

# Edge color mode ("uniform", "source", "target")
edge_color_mode = "uniform"

# Label display mode ("selected", "neighbors", "all", "none")
label_mode = "selected"

# Maximum length of node labels.
label_max_length = 20

# Base size for nodes.
node_size = 2.0

# Node size mode ("fixed", "link_count").
node_size_mode = "fixed"

# Thickness of edges (1-3).
edge_thickness = 1

# Show legend in graph view.
show_legend = true

# Show background grid.
show_grid = false

# Show minimap in graph view.
show_minimap = false

# Minimap position ("top_right", "top_left", "bottom_right", "bottom_left").
minimap_position = "top_right"

# Minimap dimensions.
minimap_width = 24
minimap_height = 12

# Marker type for canvas rendering ("braille", "half_block", "dot").
canvas_marker = "braille"

# Node shape ("circle", "square", "diamond").
node_shape = "circle"

# Offset for node labels.
label_offset = 4.0

# Number of grid divisions.
grid_divisions = 10

[graf.visual.colors]
# Custom colors for graph elements (hex strings).
# node_color = "#ffffff"
# edge_color = "#444444"
# label_color = "#888888"
# selection_ring_color = "#ff0000"
# border_color = "#444444"
# title_color = "#ffffff"
# grid_color = "#222222"
# legend_text_color = "#888888"
# status_bar_color = "#000000"
# background_color = "#000000"

[graf.physics]
# Ideal distance between nodes.
ideal_distance = 80.0

[graf.interaction]
# Zoom sensitivity factor.
zoom_factor = 1.15

# Drag sensitivity factor.
drag_sensitivity = 1.0

[graf.filter]
# List of tags to exclude from graph view.
exclude_tags = []

# Minimum number of links for a node to be visible.
min_links = 0

[graf.search]
# Maximum results to show in graph search.
max_results = 20

# Maximum visible search results.
max_visible = 10

# ── Goals System ──────────────────────────────────────────────────────────────

[goals]
# Enable the daily word/note goals system.
enabled = true

# Daily target word count (incremental additions). Set to 0 to disable.
word_goal = 500

# Daily target note count (edited or created). Set to 0 to disable.
note_goal = 3
"###
}
