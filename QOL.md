# Quality of Life Features for Notes View

This document lists the QoL features added to the clin notes view.

## Feature Summary

| Feature | Keybind | Description |
|---------|---------|-------------|
| Rename Note/Folder | `r` | Context-sensitive rename (works on notes and folders) |
| Duplicate Note | `y` | Create a copy of the selected note |
| Pin/Unpin Note | `p` | Pin important notes to the top of the list |
| Sort Options | `s` | Cycle through sort options (Modified/Title, Asc/Desc) |
| Quick Search | `Ctrl+f` | Search notes by title in real-time |
| Jump to Top | `gg` | Vim-style jump to first item |
| Jump to Bottom | `G` (Shift+g) | Vim-style jump to last item |
| Page Up | `Ctrl+u` | Move up by half a page |
| Page Down | `Ctrl+d` | Move down by half a page |
| Trash View | `Shift+T` | Open trash to view/restore deleted notes |
| Preview Pane | `Shift+P` | Toggle split-view preview of selected note |

---

## Detailed Feature Descriptions

### 1. Rename Note/Folder (`r`)

Quick rename without opening the full editor. The `r` key is context-sensitive:
- When a **note** is selected: Opens a popup to rename the note title
- When a **folder** is selected: Opens a popup to rename the folder

The note's underlying file is automatically renamed to match the new title.

### 2. Duplicate Note (`y`)

Creates a copy of the currently selected note with "(Copy)" appended to the title. The duplicate:
- Preserves all content and tags
- Is created in the same folder as the original
- Inherits the encryption status of the original

### 3. Pin/Unpin Note (`p`)

Pin important notes to always appear at the top of their folder. Pinned notes:
- Display with a `*` indicator before the title
- Are sorted to the top within each folder
- Persist across sessions (stored in frontmatter)

### 4. Sort Options (`s`)

Cycle through different sorting modes:
1. **Modified (newest first)** - Default
2. **Modified (oldest first)**
3. **Title (A-Z)**
4. **Title (Z-A)**

Notes: Pinned notes always appear at the top regardless of sort order.

### 5. Quick Search (`Ctrl+f`)

Real-time fuzzy search by note title:
- Opens a search popup
- Results update as you type
- Selection jumps to first matching note
- `Enter` confirms and closes search
- `Esc` cancels and returns to original position

### 6. Vim-style Navigation

| Key | Action |
|-----|--------|
| `gg` | Jump to first item (double-tap `g` within 500ms) |
| `G` | Jump to last item |
| `Ctrl+u` | Page up (10 items) |
| `Ctrl+d` | Page down (10 items) |
| `j` / `k` | Move down / up (already existed) |

### 7. Trash View (`Shift+T`)

Soft delete with recovery option:
- Deleted notes are moved to `.trash/` folder instead of permanent deletion
- Open trash view to see deleted notes
- Keybinds in trash view:
  - `r` or `Enter`: Restore selected note
  - `d` or `Delete`: Permanently delete selected note
  - `E` (Shift+e): Empty entire trash
  - `Esc` or `q`: Close trash view

### 8. Preview Pane (`Shift+P`)

Split-view mode showing note content without opening editor:
- Toggle with `Shift+P`
- Shows first 50 lines of the selected note
- Preview updates automatically when selection changes
- Useful for quickly browsing note contents

---

## UI Indicators

| Indicator | Meaning |
|-----------|---------|
| `*` before title | Note is pinned |
| `[UENC]` | Note is unencrypted (plain .md/.txt) |
| `[ENC]` (red) | Note is encrypted but encryption is disabled |
| `(2h ago)` | Relative time since last modification |
| `[tag]` | Note has tags |

---

## Configuration

All new keybinds follow the existing customization pattern. They can be customized in the keybinds configuration file.

Default keybinds added:
```toml
[list]
rename = ["r"]
duplicate = ["y"]
toggle_pin = ["p"]
cycle_sort = ["s"]
search = ["ctrl+f"]
jump_to_top = ["shift+g"]
page_up = ["ctrl+u"]
page_down = ["ctrl+d"]
open_trash = ["shift+t"]
toggle_preview = ["shift+p"]
```

---

## Files Modified

- `src/storage.rs` - Added: `rename_note`, `duplicate_note`, `trash_note`, `restore_note`, `delete_from_trash`, `empty_trash`, `list_trash`, `toggle_pin`
- `src/frontmatter.rs` - Added `pinned` field to `Frontmatter` struct
- `src/app.rs` - Added new popup structs, sort options, navigation methods, and QoL feature handlers
- `src/keybinds.rs` - Added new `ListAction` variants for all QoL features
- `src/events.rs` - Added event handlers for new popups and keybinds
- `src/ui.rs` - Added rendering for preview pane, trash view, search popup, note rename popup, and pinned indicator
- `src/constants.rs` - Updated help text with new keybinds
