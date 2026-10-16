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

## Mermaid Flowchart Previews

Fenced `mermaid` blocks (backtick or tilde fences) render inline as read-only,
terminal-native flowcharts through Pinstar. Edit their source in the body, not in
an embedded canvas editor. Diagrams default to **on**. **Alt+m** toggles all
Mermaid blocks in the main edit preview between diagrams and ordinary code
blocks, including malformed or non-flowchart blocks. This session-only preference
resets to on when clin restarts; list and linked-note previews are unaffected.
No collapse control is needed: preview scrolling tracks the editor's source line
and wrapped sub-row, rather than copying screen row numbers. Diagram height may differ from fenced-source height. Source anchors
are recalculated on resizing and wrapping changes; independent preview scrolling
lasts until the editor anchor moves or preview content is rebuilt.

The native parser is pinned to `merman-core` 0.6.0. Nodes, nested subgraphs,
directions, edge labels/markers, classes and basic terminal colors are retained.
Specialized shapes use labelled outlines; HTML/Markdown, icons/images, CSS,
curves, animation and browser configuration use explicit terminal substitutions.
Images are placeholders, remote assets are not fetched, links are text and
callbacks never execute. Substitution notices appear below the diagram.

Invalid, unsupported, too-wide or oversized diagrams retain their fenced source
with an explanation, never a silently incomplete graph. Preview limits are
64 KiB per block, 128 nodes, 256 edges, 32 groups, 16 nesting levels, 240 columns
and 512 diagram rows. Conservative pre-parse chart-body budgets also cap raw `&` characters at 32,
opening delimiters at 256, and `subgraph` words at 32, including labels/comments.
YAML frontmatter uses a non-expanding scanner with 16-level structure limit;
anchors/aliases are rejected, while literal `&`/`*` in strings remain supported.
Rejected input remains visible as source rather than risking parser expansion.
Rendering runs in the existing background Markdown layout pipeline and cache;
no temporary files containing note text are created.

## Focus and Preview

The current editor has **Title**, **Body**, and visible **Sidebar** focus; it has no READ/EDIT modal state or `EditMode` enum. Typing edits the focused text field directly. `Ctrl+t` cycles focus; `Tab` inserts a tab in the body. Presets change navigation outside text inputs, not the body editor.

`Ctrl+p` toggles the Markdown preview, `Alt+m` toggles Mermaid diagram rendering,
and `F11` expands the preview. `Esc` first closes an active popup/link preview; otherwise it saves and returns directly to List. `Ctrl+s` saves without leaving.

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

The `EditSidebar` on `NoteEditor` displays forward/back link panes alongside the editor. `[[wikilink]]` targets and back-references are resolved and listed. The `link_preview` state field tracks the active preview. `Ctrl+o` toggles Outline, `Ctrl+b` toggles Links, `Ctrl+e` toggles the left file-tree sidebar, and `Ctrl+t` cycles focus (Title → Body → Files → Sidebar). `Alt+l` previews the link under the cursor.

## Tabs + File Sidebar + Duplicate Titles

The edit view has a merged one-row header (status info + tabs) and a left file-tree pane. Tabs keep several notes open: the selected tab shows the note title and the only save indicator (`●` = unsaved) — click a tab (or `Alt+Right` / `Alt+Left`) to switch, click the selected tab to rename it inline, `Ctrl+n` for a new note, `Ctrl+w` (or click a tab's `×`, or middle-click it) to close the current tab (autosaves first). `Ctrl+Tab` is intentionally not used: most terminals swallow it for their own tab switching. The `+` button and `Ctrl+n` open the same new-file selector as the notes list (Markdown, Text, Draw, Canvas). Focus mode (`F8`) hides the tab strip and file sidebar along with the header and hint bar. The file tree (`Ctrl+e`) takes ~22% of the editor width (never a fixed column count) and is the notes list tree reused verbatim — same rows, folder icons, note counts and density, with collapse state shared with the notes list; click a note to open it in a tab, `Enter` on a selected row does the same, `Left`/`Right` collapse/expand folders. All tab and sidebar shortcuts are listed in the `F2` keybinds overlay and the `F1` help editor tab.

Note titles are not required to be unique: creating, renaming, or saving a duplicate title is allowed. Notes are identified by id (path), so title-based lookups (`[[wikilink]]` follow, backlinks, graph edges) resolve to the last matching note when titles collide.

## Wikilink Follow

`Enter` on a `[[wikilink]]` follows it: existing targets autosave and open, missing targets offer a confirm popup to create the note in the current note's folder (vault root for folder-less notes). Double-clicking a `[[wikilink]]` with the mouse does the same; single click only moves the cursor. `[[alias|Title]]` pipe syntax resolves to the part before `|`.

## Link Suggestions

Typing `[[` in the body opens a suggestion popup listing existing note titles filtered by what you typed (prefix matches rank first, up to 8 rows). `Up`/`Down` move, `Tab` or `Enter` completes to `[[Title]]`, `Esc` dismisses, and clicking a row accepts it. With nothing typed it shows the first notes in vault order; with no matches it says so — keep typing or finish the link manually. Typing past the `|` alias separator dismisses the popup (you are writing display text then). Toggle with `[editor] link_suggestions` (default `true`).

The popup only appears while the link text is actually being typed or edited: moving the cursor (arrow keys, mouse click) or skipping over an auto-inserted `]` dismisses it, so `Enter` on a finished link follows it instead of re-completing it. Typing another letter inside an existing link reopens suggestions around the cursor.

## Auto-Close Pairs

Typing `(` `[` `{` `"` `'` `` ` `` auto-inserts its counterpart and places the cursor between them (VS Code style). Typing the closing char when it is already under the cursor skips over it instead of duplicating; `Backspace` between an empty pair deletes both halves. Typing an opener around a keyboard/mouse selection wraps it. Quotes refuse to pair inside words (`don't` stays single). Toggle with `[editor] auto_close_pairs` (default `true`).

## Keyboard Text Selection

Title and body fields support:

| Keys | Selection |
|---|---|
| `Shift+Left/Right` | Extend by one character |
| `Ctrl+Shift+Left/Right` | Extend by one word |
| `Alt+Shift+Left/Right` | Extend by one word (alternative for terminal shortcut conflicts) |
| `Shift+Home/End` | Extend to the start/end of the line |

Keep holding the modifiers to extend selection across multiple words. An
unmodified arrow key clears the selection and moves the cursor.

### Kitty and Other Terminal Shortcuts

Kitty binds `Ctrl+Shift+Left/Right` to previous/next tab by default. Terminal
shortcuts run before clin receives input; enabling the Kitty keyboard protocol
cannot bypass them. Use `Alt+Shift+Left/Right`, or add these lines to
`~/.config/kitty/kitty.conf` to pass the original keys through:

```conf
map ctrl+shift+left
map ctrl+shift+right
```

Reload Kitty's configuration (`Ctrl+Shift+F5` with default shortcuts), or restart
Kitty. These mappings disable tab switching on those two keys. See
[Kitty keyboard shortcuts](https://sw.kovidgoyal.net/kitty/conf/#keyboard-shortcuts)
for mapping syntax.

For other terminals, remove conflicting shortcuts or configure them to send
arrow keys with both modifiers. clin can only select by word when the terminal
forwards the key and its modifiers; it cannot recover modifiers stripped by a
terminal or multiplexer.

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
| `auto_close_pairs` | bool | `true` | Auto-close brackets/quotes; skip-over closers; pair-aware Backspace |
| `link_suggestions` | bool | `true` | Suggest existing note titles while typing `[[...]]` |
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
auto_close_pairs = true
link_suggestions = true
zen_hide_line_numbers = true
zen_hide_scrollbar = true
zen_focus_dimming = false
zen_focus_unit = "paragraph"
zen_focus_context = 3
```

## Draft Recovery and Encrypted Notes

Editor mutations attempt to write an encrypted draft to `<vault>/.clin/editor_draft.bin`. Startup attempts to recover it into the note, then removes the readable draft even if decoding or saving fails. This is best-effort recovery, not a guarantee against every crash or disk-write failure.

`.clin` notes cannot be edited directly: use **Decrypt Note** from the palette first. Draft writes exclude `.clin` IDs; encryption uses the application-wide key outside the vault. See [ENCRYPTION.md](ENCRYPTION.md).

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, view state machine
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets and sequence syntax
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — editor-related actions
