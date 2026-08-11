# Subnotes

## Overview

Subnotes are encrypted virtual notes attached to a physical parent note. They are browsable via a grid tab and a virtual tree folder in the notes list, with a radial braille graph and a manager popup.

**Source:** `src/storage.rs` (storage layer), `src/ui/popups.rs` (popup), `src/ui/list_view.rs` (grid tab + radial graph), `src/app/loading.rs` (virtual folder)

## Storage

`SubNote` struct and `SubNotePayload` enum are defined in `src/storage.rs:28-39`. Subnotes are stored in a single file `.clin/subnotes.bin` as a XOR-obfuscated, `bincode`-encoded `HashMap<parent_id, Vec<SubNote>>`. Optional ChaCha20-Poly1305 encryption is applied when the vault key is set.

Key methods:

| Method | Line | Purpose |
|---|---|---|
| `get_subnotes` | 1332 | Retrieve subnotes for a parent |
| `set_subnotes` | 1365 | Save subnotes for a parent |
| `migrate_subnotes_parent` | 1422 | Re-parent subnotes when a note is moved |
| `get_notes_with_subnotes` | 1457 | List parent notes that have subnotes |
| `get_all_subnotes` | 1474 | Enumerate all subnotes across all parents |

## Browsable Views

### Virtual Tree Folder

A virtual node at `VIRTUAL_SUBNOTES_PATH = "__clin_virtual__/subnotes"` (`src/app.rs:45-46`) is built in `src/app/loading.rs:494-520`. Each subnote renders as a `VisualItem::Subnote` variant (`src/ui/list_view.rs:99`), appearing in the notes list alongside regular notes.

### Grid Tab

When navigating into the subnotes virtual folder, the list view switches to a subnotes grid layout (detected at `src/ui/list_view.rs:942-943, 1038-1039, 1116`).

### Radial Graph

The radial graph is rendered by `render_subnote_graph_static` (`src/ui/list_view.rs:310-360`) using ratatui's `Canvas` widget. Nodes are positioned using `orbit_positions` with `HollowCircle` markers, and wikilink edges connect related subnotes. Zoom/pan state is tracked at `list_view.rs:164-169`. Mouse handling in `src/events/list.rs:742-795` supports click-to-select and drag-to-pan.

## Manager Popup

The `SubnotesPopup` struct and `SubnotesFocus` enum (`src/ui/popups.rs:278-291`) control the manager popup, rendered by `draw_subnotes_popup` (`src/ui/popups.rs:1393-1570`). Event handling in `src/events/mod.rs:732-810` supports:

- `Alt+N` — create a new subnote
- `Ctrl+E` — edit a subnote externally
- Navigation keys — move selection
- Delete — remove a subnote
- Save — persist changes

## Keybindings

| Action | Default Key | Scope |
|---|---|---|
| `ListAction::ManageSubnotes` | `Alt+s` | List view |
| `EditAction::ManageSubnotes` | `Alt+s` | Editor view |

Command-palette action `manage_subnotes` (`src/actions/mod.rs:212-220`) opens the subnotes manager.

## Connections

- [ENCRYPTION.md](ENCRYPTION.md) — encryption layer for subnote storage
- [LIST_VIEW.md](LIST_VIEW.md) — grid tab and virtual folder integration
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `manage_subnotes` action
