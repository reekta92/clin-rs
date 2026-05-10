# clin-rs Documentation

Welcome to clin-rs technical documentation. This index lists all documentation files with brief descriptions.

For installation, quickstart, and general project info, see the [README.md](../README.md).

---

## Core Architecture

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture: view state machine, event loop, rendering pipeline, data flow, threading model, module map

## Views

- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Force-directed graph view (graf): node/edge construction, physics simulation, rendering, interaction, viewport, search
- [CANVAS.md](CANVAS.md) — Obsidian-compatible canvas view (pinstar): `.canvas` JSON schema, node types, interaction model, key types
- [DRAW.md](DRAW.md) — Freehand drawing canvas: `.draw` format, tool set, shape types, interaction

## Features

- [ENCRYPTION.md](ENCRYPTION.md) — Zero-knowledge encryption: ChaCha20-Poly1305, key management, `.clin` file format, encrypt/decrypt workflow
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system: 11 built-in themes, color derivation, per-color overrides, AppThemeColors
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — Command palette and Action trait: available actions, registration, how to add new actions
- [TEMPLATES.md](TEMPLATES.md) — Note template system: TOML file format, template variables, CLI usage

## Configuration

- [README.md](../README.md) — Quickstart, config.toml example, keybinds.toml example, CLI commands
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — Full configuration reference: all config.toml options, keybinds.toml, graf config sections

---

## File Index

```
docs/
├── INDEX.md              ← You are here
├── ARCHITECTURE.md       System overview
├── GRAPH_VIEW.md         Force-directed graph
├── CANVAS.md             Obsidian-compatible canvas
├── DRAW.md               Freehand drawing
├── ENCRYPTION.md         Zero-knowledge encryption
├── THEME_SYSTEM.md       Theme and color system
├── COMMAND_PALETTE.md    Command palette + Action trait
├── TEMPLATES.md          Note template system
├── CONFIG_REFERENCE.md   All config options
└── (README.md in root)
```
