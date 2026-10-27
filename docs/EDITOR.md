# Editor View

Press unmodified `F6` for editor settings, applicable body commands, shared settings, and guarded navigation. Custom editor F6 bindings take priority. Title/frontmatter/sidebar focus hides body insertion; Markdown-only commands are unavailable in structured public/template source buffers. Cancelling preserves focus, cursor, selection, and pending edits. Palette navigation saves the editor before departure; failure retains the buffer and recovery draft. `Ctrl+p` still toggles Markdown preview.

## Overview

A built-in title/body editor with find popup, soft-wrap, sidebars with wikilink
previews, and external-editor handoff. `Esc` saves once when returning to
the calling view (normally Notes).

**Source:** `src/editor.rs` (state), `src/editor_document.rs` (body buffer,
revision, snapshot, and change contract), `src/editor_session.rs` (in-process
event loop), `src/ui/edit_view.rs` (rendering), `src/events/edit.rs` (input).

Edit runs in dedicated same-process session. It draws initial frame, batches up
to 64 queued input events without reordering keys, coalesces only consecutive
mouse-move or resize events, and redraws only after dirty editor-local work.
Catalog, search, and other generic app queues wait until Edit exits. While live
Bases regions are visible, Edit narrowly drains watcher changes and the shared
public query worker, without replacing unsaved editor text.

`EditorDocument` currently wraps `ratatui-textarea` behind body APIs; title,
canvas JSON, popups, and Draw retain their own `TextArea` instances.

## Frontmatter properties

**F7** (`edit.toggle_frontmatter`, remappable) shows or hides one region above Body. It starts hidden on existing and new Markdown/text notes, opens with focus, and retains edits and undo when hidden. The command palette also offers **Toggle Frontmatter**. Sidebars remain available. Metadata stays outside Markdown preview, outline, body links, and writing counts.

The default properties presentation lists fields in source order with their YAML types. Use arrows or the mouse to select, **a** to add, **Enter** to edit (or toggle a boolean), and **Delete** to remove. The value input accepts YAML scalars, null, lists, and mappings. **Tab** switches Name/Value, **Ctrl+Enter** applies, and **Esc** cancels the value input. Removing a field and setting `null` are different operations. Changed values follow the same declared types as Bases, read from `.obsidian/types.json`; an unrelated existing type mismatch does not block editing another field. Complex keys or unsafe alias/tag patches require raw editing.

To edit YAML directly in the same region:

```toml
[editor]
frontmatter_mode = "raw_yaml" # default: "properties"
```

Raw mode supports multiline editing, paste, selection, clipboard, mouse scrolling, and undo/redo. Edit the YAML inside the delimiters. Invalid input remains editable and blocks save, exit, and navigation. Switching presentation/configuration retains pending input; an open property value input remains available until applied or cancelled.

Explicit header edits/removals win over automatic managed output for that save. Conflicting edits to Title and YAML `title` require correction before saving. Removing `title` uses the filename for display. Existing filenames remain stable after title edits, so `[[CL-01]]` keeps its target; renaming a file is a separate explicit operation.

Body-only saves preserve unrelated header bytes, comments, quoting, order, blank lines, BOM, newline convention, and Unix permissions. Missing managed fields stay missing. `[notes.frontmatter]` controls automatic output; see [Configuration](CONFIG_REFERENCE.md#notesfrontmatter). A later body edit can refresh enabled links/timestamps already present. Uninterpretable managed values remain untouched.

Invalid YAML, unfinished property inputs, and Title conflicts use private repair-only recovery records. On restart the newest repair draft opens in the editor without publishing invalid content; other repair drafts remain on disk. Correct and save the restored input. External source changes still reject saving against the original baseline. See [Upgrading](UPGRADING.md) for recovery privacy and compatibility.

Templates, verbatim `.base`/Canvas/Draw source, attachments, legacy encrypted notes, and virtual subnotes have no frontmatter region. Whole-file external editing remains unchanged.

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

The native parser uses `merman-core-clin` 0.6.0 with the upstream 0.6 parser API. Nodes, nested subgraphs,
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

The editor has **Title**, **Body**, visible **Frontmatter**, and **Sidebar** focus; typing is not modal. Typing edits the focused text field directly. `Ctrl+t` cycles focus; `Tab` inserts a tab in the body. Presets change navigation outside text inputs, not the body editor.

`Ctrl+p` toggles the Markdown preview, `Alt+m` toggles Mermaid diagram rendering,
and `F11` expands the preview. `Esc` first closes an active popup/link preview; otherwise it saves and restores the return view (normally Notes, or Calendar/Bases when entered from there). `Ctrl+s` saves without leaving.

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

`F8` (edit view, remappable) toggles zen mode: the header bar is hidden except while the title field is focused (`Ctrl+t` to
cycle focus) or a transient status (clipboard notices, autosave errors) is
active, and the editor content is padded from the left and right edges.
`EditorConfig.zen_padding_percent` (default `15`) sets the padding as a
percent of the terminal width per side, clamped to 45. Overlays, popups, and
message toasts and the shared footer stay visible. Zen mode is per-session; it resets on restart.

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

**Ctrl+g Ctrl+b** (`edit.ToggleBases`) opens the `.base` link under the cursor in
a live sidebar using the current public file as `this`; repeat to close it.
Public Markdown and linked-note previews also render named and fenced Bases
definitions. Click a region to use Bases controls; Esc returns focus to the
editor. The Bases palette binding expands the region into its full view and
returns to the same host afterward. See [BASES.md](BASES.md) for privacy, source
anchors, refresh, guarded writes, and resource limits.

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
| `frontmatter_mode` | enum | `"properties"` | F7 presentation: `"properties"` or `"raw_yaml"` |
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

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, view state machine
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets and sequence syntax
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — editor-related actions

Ordinary public Markdown editors use lossless guarded saving, including live
Bases regions. The editor still presents the body
and title separately. It retains the original YAML, line endings, trailing body
bytes, permissions, and opening baseline; title edits do not rename the file.
Inline configuration saves update the containing buffer. Metadata-only writes can
rebase pending body edits and migrate their plaintext recovery draft. Conflicts
retain the pending editor and reject stale saves. A source handoff suspends and
restores the containing editor, including unsaved text.
## Save and recovery guarantees

Ordinary saves preserve filename identity and Unix permissions, reject read-only files, and check the loaded raw source against disk. Conflicts and save failures remain unsaved; autosave retries instead of claiming success. Explicit file rename warns that incoming references are not rewritten.

Recovery drafts are coalesced off the input thread, with durable flushes at save/session boundaries. New records are plaintext with private Unix permissions; old encrypted records are retained, never converted or deleted automatically. See [Upgrading](UPGRADING.md) for privacy and legacy-data requirements.

`Ctrl+C` copies an active selection. Without selection it reports that nothing is selected, not an emergency process exit. Navigation presets do not turn the body into Vim or Helix.

## Clipboard transport

Copy and cut use the native system clipboard when available. On SSH, or when
native clipboard access fails, clin can send bounded OSC 52 output to the
terminal, including tmux passthrough. The terminal and tmux must permit OSC 52;
clin cannot confirm delivery. “Copy sent to terminal (OSC 52)” means only that
the escape sequence was written. Failed system copies remain in the app-local
clipboard, available to clin's text fields for the current process.

Existing notes retain trailing empty lines. Unchanged saves retain raw CRLF and
frontmatter formatting. Body edits preserve unrelated frontmatter comments,
quoting, and unknown properties. Adding a newline is an edit. Template saves
check their loaded source before replacement; templates do not get note recovery
drafts.
