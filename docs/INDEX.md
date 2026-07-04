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
- [CONTENT_TREE.md](CONTENT_TREE.md) — Content Tree view: nested outline parsing, collapsible subtrees, jump-to-section editor navigation
- [BASES.md](BASES.md) — Obsidian Bases view: table layout, expression engine, cell editing, column summaries

## Features

- [ENCRYPTION.md](ENCRYPTION.md) — Zero-knowledge encryption: ChaCha20-Poly1305, key management, `.clin` file format, encrypt/decrypt workflow
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system: 11 built-in themes, color derivation, per-color overrides, AppThemeColors
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — Command palette and Action trait: available actions, registration, how to add new actions
- [TEMPLATES.md](TEMPLATES.md) — Note template system: TOML file format, template variables, CLI usage

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
├── BACKUP.md             Git-based backup dashboard
├── CANVAS.md             Obsidian-compatible canvas
├── DRAW.md               Freehand drawing
├── CONTENT_TREE.md       Content tree outline
├── BASES.md              Obsidian-compatible Bases view
├── ENCRYPTION.md         Zero-knowledge encryption
├── THEME_SYSTEM.md       Theme and color system
├── COMMAND_PALETTE.md    Command palette + Action trait
├── TEMPLATES.md          Note template system
├── CONFIG_REFERENCE.md   All config options
├── KEYBIND_PRESETS.md     Keybind presets and sequence syntax
```
