# Subnotes

## Overview

Subnotes are virtual notes attached to a physical parent note. Subnotes saved under `.clin` parents use encryption; plaintext-parent subnotes do not. They are browsable via a grid tab and a virtual tree folder in the notes list, with a radial braille graph and a manager popup.

**Source:** `src/storage.rs` (storage layer), `src/popups.rs` (popup state), `src/ui/popups.rs` (popup rendering), `src/events/mod.rs` (popup input), `src/ui/list_view.rs` (grid tab + radial graph), `src/app/loading.rs` (virtual folder)

## Storage

`SubNote` and `SubNotePayload` are defined in `src/storage.rs`. `<vault>/.clin/subnotes.bin` stores an XOR-obfuscated, bincode-encoded `HashMap<String, SubNotePayload>`:

- `Plain(Vec<SubNote>)` for plaintext parents.
- `Encrypted(Vec<u8>)` for subnotes saved under parent IDs ending in `.clin`; the payload uses ChaCha20-Poly1305 and the application key.

**XOR obfuscation is not encryption.** A key existing on disk does not encrypt every subnote. Unix database writes use mode `0600`.

Key methods:

| Method | Purpose |
|---|---|
| `get_subnotes` | Retrieve subnotes for a parent |
| `set_subnotes` | Save subnotes, selecting plain/encrypted payload by parent ID |
| `migrate_subnotes_parent` | Re-key the database entry when a parent ID changes |
| `get_notes_with_subnotes` | List parent notes that have subnotes |
| `get_all_subnotes` | Enumerate all subnotes across all parents |

## Browsable Views

### Virtual Tree Folder

A virtual node at `VIRTUAL_SUBNOTES_PATH = "__clin_virtual__/subnotes"` (`src/app.rs`) is built in `src/app/loading.rs`. Each subnote renders as a `VisualItem::Subnote` variant (`src/list_view.rs`), appearing alongside regular notes.

### Grid Tab

When navigating into the subnotes virtual folder, the list view switches to a subnotes grid layout in `src/ui/list_view.rs`.

### Radial Graph

The radial graph is rendered by `render_subnote_graph_static` in `src/ui/list_view.rs` using ratatui's `Canvas` widget. Nodes are positioned using `orbit_positions` with `HollowCircle` markers, and wikilink edges connect related subnotes. Zoom/pan state is tracked in `src/list_view.rs`. Mouse handling in `src/events/list.rs` supports click-to-select and drag-to-pan.

## Manager Popup

`SubnotesPopup` and `SubnotesFocus` in `src/popups.rs` control the manager popup, rendered by `draw_subnotes_popup` in `src/ui/popups.rs`. Event handling in `src/events/mod.rs` supports:

- `Alt+N` — create a new subnote
- `Ctrl+E` — edit a subnote externally
- Navigation keys — move selection
- Delete — remove a subnote
- `Tab` — cycle List/Title/Content focus
- Close — save dirty changes

## Search

The notes search popup supports the `sn:` prefix to exclusively search subnotes (matches both title and content). Accepting a result automatically navigates to that subnote in the list and opens the manager popup.

## Keybindings

| Action | Default Key | Scope |
|---|---|---|
| `ListAction::ManageSubnotes` | `Alt+s` | List view |
| `EditAction::ManageSubnotes` | `Alt+s` | Editor view |

Command-palette action `manage_subnotes_list` in `src/actions/mod.rs` opens the subnotes manager.

## Connections

- [ENCRYPTION.md](ENCRYPTION.md) — encryption layer for subnote storage
- [LIST_VIEW.md](LIST_VIEW.md) — grid tab and virtual folder integration
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `manage_subnotes_list` action
