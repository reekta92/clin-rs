# clin-rs Documentation

Welcome to clin-rs technical documentation. This index lists all documentation files with brief descriptions.

For installation, quickstart, and general project info, see the [README.md](../README.md).

---

## Core Architecture

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture: view state machine, event loop, rendering pipeline, data flow, threading model, module map

## Views

- [LIST_VIEW.md](LIST_VIEW.md) — Notes list view: Grid/Tree layout, note format chooser, preview pane
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Force-directed graph view (graf): node/edge construction, physics simulation, rendering, interaction, viewport, search
- [BACKUP.md](BACKUP.md) — Git-based backup dashboard: vault state (staged/unstaged/untracked), history, diff preview, automation settings
- [CANVAS.md](CANVAS.md) — Obsidian-compatible canvas view (pinstar): `.canvas` JSON schema, node types, interaction model, key types
- [DRAW.md](DRAW.md) — Freehand drawing canvas: `.draw` format, tool set, shape types, interaction
- [OUTLINE.md](OUTLINE.md) — Outline view: nested outline parsing, collapsible subtrees, jump-to-section editor navigation
- [SETUP.md](SETUP.md) — First-run setup wizard: theme/background/hint-bar/icon-mode/keybind-preset cycling with live preview
- [HELP.md](HELP.md) — Help view: 3-pane layout, 8 tabs, keybind index, tips, popup accordion
- [EDITOR.md](EDITOR.md) — Editor view: READ/EDIT modes, find popup, soft-wrap, sidebars, wikilink previews, external editor

## Features

- [ENCRYPTION.md](ENCRYPTION.md) — Zero-knowledge encryption: ChaCha20-Poly1305, key management, `.clin` file format, encrypt/decrypt workflow
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system: 19 built-in themes, color derivation, per-color overrides, AppThemeColors
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — Command palette and Action trait: available actions, registration, how to add new actions
- [TEMPLATES.md](TEMPLATES.md) — Note template system: TOML file format, template variables, CLI usage
- [IMAGE_RENDERING.md](IMAGE_RENDERING.md) — Native image rendering: ratatui-image, sixel/kitty/iTerm protocols, [image] config, cache/worker
- [SUBNOTES.md](SUBNOTES.md) — Subnotes: encrypted attached notes, grid tab, virtual tree folder, radial graph, manager popup

## Configuration

- [README.md](../README.md) — Quickstart, config.toml example, keybinds.toml example, CLI commands
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — Full configuration reference: all config.toml options, keybinds.toml, graf config sections
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — Keybind presets: Helix, Vim, and Emacs presets and sequence syntax
---

## File Index

```
docs/
├── INDEX.md              ← You are here
├── ARCHITECTURE.md       System overview
├── LIST_VIEW.md          Notes list view
├── GRAPH_VIEW.md         Force-directed graph
├── HELP.md               Help: 3-pane, 8 tabs, keybind index, tips, popup accordion
├── BACKUP.md             Git-based backup dashboard
├── CANVAS.md             Obsidian-compatible canvas
├── DRAW.md               Freehand drawing
├── OUTLINE.md             Outline
├── EDITOR.md             Editor: READ/EDIT modes, find, soft-wrap, sidebars, wikilinks
├── SETUP.md              First-run setup wizard
├── ENCRYPTION.md         Zero-knowledge encryption
├── THEME_SYSTEM.md       Theme and color system
├── COMMAND_PALETTE.md    Command palette + Action trait
├── IMAGE_RENDERING.md    Native image rendering, ratatui-image, protocols
├── TEMPLATES.md          Note template system
├── SUBNOTES.md           Subnotes: encrypted virtual notes, grid tab, radial graph
├── CONFIG_REFERENCE.md   All config options
├── KEYBIND_PRESETS.md     Keybind presets and sequence syntax
```
