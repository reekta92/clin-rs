# Editor View

## Overview

A built-in title/body editor with find popup, soft-wrap, sidebars with wikilink
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

## Focus and Preview

The editor has **Title**, optional **Frontmatter**, **Body**, and visible **Sidebar** focus; it has no READ/EDIT modal state or `EditMode` enum. Typing edits the focused text field directly. `Ctrl+t` cycles focus, skipping hidden fields; `Tab` inserts a tab in text fields. Presets change navigation outside text inputs, not the body editor.

`Ctrl+p` toggles the Markdown preview and `F11` expands it. `Esc` first closes an active popup/link preview; otherwise it saves and returns directly to List. `Ctrl+s` saves without leaving.

`edit_mode_highlight` (default `true`) controls Markdown syntax highlighting over the editable body, despite its historical name. `ghost_syntax` dims Markdown delimiters, and `extended_markdown_features` enables additional syntax forms.

## Text Selection and Clipboard

Keyboard selection: hold `Shift` while moving the cursor (arrows, Home/End,
PageUp/PageDown; `Ctrl+Shift+Arrow` selects word-wise) to extend a selection,
then use the copy/cut keybinds (`Ctrl+c`/`Ctrl+x` by default). `Ctrl+v` pastes, `Ctrl+z` undoes, and `Ctrl+y` or `Ctrl+Shift+z` redoes.
Mouse drag selects and copies immediately by default;
`EditorConfig.copy_on_select` (bool, default `true`) set to `false` keeps the
selection instead so it can be copied with the copy keybind.


## Find Popup

A custom find popup replaces the legacy textarea search. State is stored in the `find_popup` field on `NoteEditor`. Triggered via the edit keybind scope.

## Soft Wrap

`EditorConfig.soft_wrap` (bool, default `false`) controls soft-wrapping of the editor body. Toggle with `F10`. With soft wrap enabled, `F9` cycles left/center/right/justified text alignment.

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

The `EditSidebar` on `NoteEditor` displays forward/back link panes alongside the editor. `[[wikilink]]` targets and back-references are resolved and listed. The `link_preview` state field tracks the active preview. `Ctrl+o` toggles Outline, `Ctrl+b` toggles Links, and `Ctrl+t` cycles focus to a visible sidebar. `Alt+l` previews the link under the cursor.

## Editable Frontmatter

`F7` (`toggle_frontmatter` in the edit keymap, remappable) shows a scrollable
**Frontmatter (YAML)** field above the body and focuses it immediately. Press it
again to hide the field and return to Body. Hiding does not discard pending
changes. Each newly opened note starts with the field hidden; a recovered
unsaved metadata draft opens it for correction. Explicitly opened frontmatter
remains visible in zen mode and alongside preview/sidebars.

Edit YAML contents without the surrounding `---` delimiters. Add, change, or
remove arbitrary custom fields, including lists and nested mappings. Selection,
clipboard shortcuts, Unicode input, multiline paste, and undo/redo use the
focused field. Find and go-to-line remain body-only. Frontmatter does not enter
Markdown preview, outline, body line numbering, wikilinks, or writing word counts.

Save, autosave, and Esc save metadata and body together. YAML must be a mapping
with unique keys; managed fields must have their existing types. Invalid YAML
blocks saving and leaving the note, without modifying its file or reporting
Saved. The raw pending text is kept in an encrypted recovery draft. Startup restores
an unsaved draft into Edit view for correction, even if its new note was never
saved to disk. If the disk header changed while
metadata was being edited, saving refuses to overwrite it and retains the draft;
resolve that conflict before retrying. There is no automatic merge UI.

Custom-field removals persist. `title`, `tags`, `pinned`, and `text_align` can also
be edited. A YAML title change updates the title control after a successful save;
if both fields have conflicting pending title changes, make their values agree
before saving. Clearing the field clears custom properties, tags, pinning, and
per-note alignment, but keeps the title control's value.

`updated_at`, `links`, and `original_ext` retain Clin's existing managed-field
save rules and may be regenerated rather than accepting a manual value. YAML
serialization can normalize field order, quotes, whitespace, and comments;
this is not a formatting-preserving properties editor. Whole-file external
editing and configurable managed-field output are not part of this toggle.

For vaults whose wikilinks use stable filenames such as `[[CL-01]]`, set
`rename_on_title_change = false` in `[notes]` to keep those filenames on title
changes. This feature does not change that setting or migrate broken links.
Only ordinary `.md`/`.txt` notes in the built-in editor support the toggle;
templates, canvas/draw, and external editing remain unchanged. `.clin` notes
still require decryption before editing, and note-file frontmatter remains
plaintext.

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
| `edit_mode_highlight` | `bool` | `true` | Apply Markdown highlighting to the editable body |
| `ghost_syntax` | `bool` | `true` | Dim Markdown delimiters |
| `extended_markdown_features` | `bool` | `true` | Highlight extended Markdown syntax |
| `text_align` | `enum` | `"left"` | Soft-wrapped text alignment: left, center, right, justified |
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

## Draft Recovery and Encrypted Notes

Editor mutations attempt to write an encrypted draft to `<vault>/.clin/editor_draft.bin`. Startup attempts to recover it into the note. A draft is removed only after a successful save; invalid YAML, title conflicts, or a changed disk header restores the pending draft into Edit view on startup. Legacy title/body drafts remain readable. This is best-effort recovery, not a guarantee against every crash or disk-write failure.

`.clin` notes cannot be edited directly: use **Decrypt Note** from the palette first. Draft writes exclude `.clin` IDs; encryption uses the application-wide key outside the vault. See [ENCRYPTION.md](ENCRYPTION.md).

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, view state machine
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets and sequence syntax
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — editor-related actions
