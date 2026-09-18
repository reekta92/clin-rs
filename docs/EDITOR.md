# Editor View

## Overview

A modal built-in editor with find popup, soft-wrap, sidebars with wikilink
previews, and external-editor handoff. `Esc` saves once when returning to
notes list.

**Source:** `src/editor.rs` (state), `src/editor_document.rs` (body buffer,
revision, snapshot, and change contract), `src/editor_session.rs` (in-process
event loop), `src/ui/edit_view.rs` (rendering), `src/events/edit.rs` (input).

Edit runs in dedicated same-process session. It draws initial frame, batches up
to 64 queued input events without reordering keys, coalesces only consecutive
mouse-move or resize events, and redraws only after dirty editor-local work.
Catalog, watcher, search, and other generic app queues wait until Edit exits.

`EditorDocument` currently wraps `ratatui-textarea` behind body APIs; title,
canvas JSON, popups, Draw, and Backup retain their own `TextArea` instances.

## Preview Lifecycle

Body mutations schedule Markdown preview from `EditorDocument::revision()`.
`EditorPreviewScheduler` starts with a 75 ms layout EWMA and submits after
`clamp(2 × EWMA, 150 ms, 750 ms)`. Title edits only redraw title chrome; they
never schedule body preview work. Initial open and explicit preview toggles
remain immediate.

## Modes

The `EditMode` enum (READ/EDIT) is defined in `src/editor.rs`:

- **READ mode** — view-only rendered markdown. Supports select and clipboard operations (yank/copy). Navigate with `j`/`k`, `PageUp`/`PageDown`, `G`/`gg`.
- **EDIT mode** — text insertion enabled. Press `e`/`i` to enter, `Esc` steps back: EDIT→READ→list.

The `edit_mode_highlight` config option (`EditorConfig.edit_mode_highlight`, default `true`) controls visual highlighting of the active mode. A source-line map keeps READ and EDIT scroll positions in sync.

## Text Selection and Clipboard

Keyboard selection: hold `Shift` while moving the cursor (arrows, Home/End,
PageUp/PageDown; `Ctrl+Shift+Arrow` selects word-wise) to extend a selection,
then use the copy/cut keybinds (`Ctrl+Shift+C`/`Ctrl+Shift+X` by default).
Mouse drag selects and copies immediately by default;
`EditorConfig.copy_on_select` (bool, default `true`) set to `false` keeps the
selection instead so it can be copied with the copy keybind.


## Find Popup

A custom find popup replaces the legacy textarea search. State is stored in the `find_popup` field on `NoteEditor`. Triggered via the edit keybind scope.

## Soft Wrap

`EditorConfig.soft_wrap` (bool, default `false`) controls soft-wrapping of the editor body. Toggle via the command palette.

## Zen Mode

`F8` (edit view, remappable) toggles zen mode: the footer hint bar is hidden,
the header bar is hidden except while the title field is focused (`Ctrl+t` to
cycle focus) or a transient status (clipboard notices, autosave errors) is
active, and the editor content is padded from the left and right edges.
`EditorConfig.zen_padding_percent` (default `15`) sets the padding as a
percent of the terminal width per side, clamped to 45. Overlays, popups, and
message toasts stay visible. Zen mode is per-session; it resets on restart.

By default, line numbers and the scrollbar are also hidden while in zen mode.
These can be restored by setting `zen_hide_line_numbers = false` and 
`zen_hide_scrollbar = false`.

### Focus Dimming

Zen mode supports an optional dimming feature to help focus on writing. When 
`zen_focus_dimming = true`, everything outside the active neighborhood is 
dimmed. The bright neighborhood is defined by `zen_focus_unit` (`"paragraph"` 
or `"line"`) and `zen_focus_context` (number of units above the cursor to keep 
bright).

For example, with `"paragraph"` and context `3`, the cursor's current 
paragraph plus the 3 paragraphs immediately above it remain at full brightness, 
while older text above and all text below the current paragraph are dimmed.

## Sidebars + Wikilink Previews

The `EditSidebar` on `NoteEditor` displays forward/back link panes alongside the editor. `[[wikilink]]` targets and back-references are resolved and listed. The `link_preview` state field tracks the active preview. Cycle focus with `Tab` to reach sidebars.

## External Editor

| Config Option | Type | Default | Description |
|---|---|---|---|
| `external_command` | Option\<String\> | `None` | Command for external editor |
| `external_enabled` | bool | `false` | Enable external editor mode |

Falls back to `$VISUAL` then `$EDITOR` when no command is configured. Toggle via `ToggleExternalEditorAction`.

## Insert Date

`InsertDateAction` (`src/actions/insert_date.rs`) inserts the current date/time at the cursor position using `EditorConfig.date_format` (default `"%Y-%m-%d %H:%M"`).

## Configuration

The `[editor]` section in `config.toml`:

| Option | Type | Default | Description |
|---|---|---|---|
| `external_command` | String | — | External editor command (e.g. `"nvim"`, `"code"`) |
| `external_enabled` | bool | `false` | Enable external editor mode |
| `preview_enabled` | bool | `false` | Show markdown preview panel by default |
| `show_line_numbers` | bool | `true` | Show line numbers |
| `date_format` | String | `"%Y-%m-%d %H:%M"` | Format for insert-date action |
| `soft_wrap` | bool | `false` | Soft-wrap the editor body |
| `copy_on_select` | bool | `true` | Copy to clipboard immediately when a mouse drag selects text |
| `edit_mode_highlight` | `bool` | `true` | Highlight the active READ/EDIT mode |
| `zen_padding_percent` | `u16` | `15` | Zen-mode padding per side, percent of width (max 45) |
| `zen_hide_line_numbers` | `bool` | `true` | Hide line numbers while zen mode is active |
| `zen_hide_scrollbar` | `bool` | `true` | Hide scrollbar while zen mode is active |
| `zen_focus_dimming` | `bool` | `false` | Dim content outside the active neighborhood in zen mode |
| `zen_focus_unit` | `String` | `"paragraph"` | Unit for focus neighborhood (`"paragraph"` or `"line"`) |
| `zen_focus_context` | `usize` | `3` | Number of units above the cursor to keep bright |

Example:

```toml
[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = false
show_line_numbers = true
date_format = "%Y-%m-%d %H:%M"
soft_wrap = false
copy_on_select = true
zen_hide_line_numbers = true
zen_hide_scrollbar = true
zen_focus_dimming = false
zen_focus_unit = "paragraph"
zen_focus_context = 3
```

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, view state machine
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets and sequence syntax
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — editor-related actions
