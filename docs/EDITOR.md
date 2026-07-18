# Editor View

## Overview

A modal editor with READ and EDIT modes, a built-in find popup, soft-wrap support, sidebars with wikilink previews, and external editor handoff. Changes auto-save when navigating back to the notes list.

**Source:** `src/editor.rs` (runtime state), `src/ui/edit_view.rs` (rendering), `src/events/edit.rs` (input)

## Modes

The `EditMode` enum (READ/EDIT) is defined in `src/editor.rs`:

- **READ mode** — view-only rendered markdown. Supports select and clipboard operations (yank/copy). Navigate with `j`/`k`, `PageUp`/`PageDown`, `G`/`gg`.
- **EDIT mode** — text insertion enabled. Press `e`/`i` to enter, `Esc` steps back: EDIT→READ→list.

The `edit_mode_highlight` config option (`EditorConfig.edit_mode_highlight`, default `true`) controls visual highlighting of the active mode. A source-line map keeps READ and EDIT scroll positions in sync.

## Find Popup

A custom find popup replaces the legacy textarea search. State is stored in the `find_popup` field on `NoteEditor`. Triggered via the edit keybind scope.

## Soft Wrap

`EditorConfig.soft_wrap` (bool, default `false`) controls soft-wrapping of the editor body. Toggle via the command palette.

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
| `edit_mode_highlight` | bool | `true` | Highlight the active READ/EDIT mode |

Example:

```toml
[editor]
external_command = "nvim"
external_enabled = false
preview_enabled = false
show_line_numbers = true
date_format = "%Y-%m-%d %H:%M"
soft_wrap = false
edit_mode_highlight = true
```

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, view state machine
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets and sequence syntax
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — editor-related actions
