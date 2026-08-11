# Keybind Presets

This document details the editor-style keybind presets available in clin-rs: Helix, Vim, and Emacs.

These presets apply to all navigation surfaces throughout the application (such as the main notes list, help view, graph view, and canvas), but **never affect text-input surfaces** (such as the note body editor, note title editor, search boxes, templates, tag managers, and popups).

---

## Configuration

Set the preset under `[core]` in the active configuration file; `clin config show` prints its path. Keybind files are stored at `<config-dir>/keybinds/<preset>.toml`.

```toml
[core]
# Choose from: "default", "helix", "vim", "emacs"
keybind_preset = "helix"

# Optional for the default preset. Vim, Helix, and Emacs enable their sequences automatically.
enable_key_sequences = true
```

---

## Preset Mappings

### Helix Preset

Helix style mappings rely on the `Space` key as a leader key for application-level commands, and selection-first navigation.

| View | Action | Key Sequence |
|---|---|---|
| List | Move Up | `k` / `Up` |
| List | Move Down | `j` / `Down` |
| List | Move Left | `h` / `Left` |
| List | Move Right | `l` / `Right` |
| List | Open Command Palette | `Space Space` |
| List | Jump to Top | `g g` / `G` |
| List | Page Up / Down | `Ctrl+b` / `Ctrl+f` |
| List | Create Note / Folder | `Space n` / `Space N` |
| List | New from Template | `Space t` |
| List | Toggle Pin | `Space p` |
| List | Open Graph View | `Space g` |
| List | Toggle Preview | `Space P` |
| List | Open Trash | `Space T` |
| List | Cycle Sort Mode | `Space s` |
| List | Manage Tags | `Space .` |
| List | Delete selected | `d` / `Delete` |
| List | Quit | `Ctrl+c` / `q` |
| Graph | Navigation | `h`/`j`/`k`/`l` / Arrows |
| Graph | Auto Fit | `Space a` |
| Graph | Refresh | `Space r` |
| Graph | Toggle Minimap | `Space m` |
| Graph | Zoom In / Out | `=` / `-` |

### Vim Preset

Vim style mappings support standard hjkl navigation, double-key operators, and Ex-style colon commands.

| View | Action | Key Sequence |
|---|---|---|
| List | Move Up | `k` / `Up` |
| List | Move Down | `j` / `Down` |
| List | Move Left | `h` / `Left` |
| List | Move Right | `l` / `Right` |
| List | Delete selected | `d d` / `d` / `Delete` |
| List | Jump to Top / Bottom | `g g` / `G` |
| List | Page Up / Down | `Ctrl+b` / `Ctrl+f` |
| List | Open Command Palette | `: ` (Colon then Space) |
| List | Quit | `: q` / `q` |
| Graph | Navigation | `h`/`j`/`k`/`l` / Arrows |
| Graph | Quit | `: q` / `q` |

### Emacs Preset

Emacs mappings use Ctrl-heavy bindings for navigation and Ctrl-x prefix commands.

| View | Action | Key Sequence |
|---|---|---|
| List | Move Up | `Ctrl+p` / `Up` |
| List | Move Down | `Ctrl+n` / `Down` |
| List | Move Left | `Ctrl+b` / `Left` |
| List | Move Right | `Ctrl+f` / `Right` |
| List | Page Up | `Ctrl+v` / `PageUp` |
| List | Open Command Palette | `Ctrl+x Ctrl+p` |
| List | Delete selected | `Ctrl+d` / `Delete` |
| List | Help | `Ctrl+h` / `F1` |
| List | Quit | `Ctrl+x Ctrl+c` / `q` |
| Graph | Navigation | `Ctrl+b`/`Ctrl+n`/`Ctrl+p`/`Ctrl+f` / Arrows |
| Graph | Quit | `Ctrl+x Ctrl+c` / `q` |

---

## TOML Custom Keybind Sequence Syntax

UI/help may display compact simple sequences (`gg`, `dd`, `gG`). Persisted TOML
always separates strokes with one ASCII space.

```toml
[list]
jump_to_top = ["g g"]
quit = ["Ctrl+x Ctrl+c", "q"]
```

Every token follows modifier parsing conventions (for example `Ctrl+Shift+Z`,
`Alt+Key`).
