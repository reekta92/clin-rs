# clin-rs Documentation

Welcome to clin-rs technical documentation. This index lists all documentation files with brief descriptions.

These documents are also available in the [GitHub Wiki](https://github.com/reekta92/clin-rs/wiki), automatically synced from `main`. Edit repository source files to update the wiki.

For installation, quickstart, and general project info, see the [README.md](../README.md).

---

## Core Architecture

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture: view state machine, event loop, rendering pipeline, data flow, threading model, module map
- [DEPENDENCY_PATCHES.md](DEPENDENCY_PATCHES.md) — Published compatibility parser, maintained YAML backend, safety evidence, and update policy
- [DEVELOPMENT.md](DEVELOPMENT.md) — Repository roles, workspace, CI and coordinated changes
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) — Release blockers, automated proof and interactive acceptance
- [PERFORMANCE.md](PERFORMANCE.md) — Worker/cache ownership, resource limits and reproducible benchmarks

## Views

- [BASES.md](BASES.md) — Native Bases: four layouts, public queries, guarded editing, live embeds, safety and acceptance checks
- [BASES_ACCEPTANCE.md](BASES_ACCEPTANCE.md) — Reproducible native workflow and compatibility checks
- [LIST_VIEW.md](LIST_VIEW.md) — Notes list view: Grid/Tree/List/Multi layouts, note format chooser, preview pane
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — Force-directed graph view (graf): node/edge construction, physics simulation, rendering, interaction, viewport, search
- [CALENDAR.md](CALENDAR.md) — Month/week/day/agenda, scheduling, note references, iCalendar and CalDAV
- [CANVAS.md](CANVAS.md) — Obsidian-compatible canvas view (pinstar): `.canvas` JSON schema, node types, interaction model, key types
- [DRAW.md](DRAW.md) — Freehand drawing canvas: `.draw` format, tool set, shape types, interaction
- [OUTLINE.md](OUTLINE.md) — Outline view: nested outline parsing, collapsible subtrees, jump-to-section editor navigation
- [SETUP.md](SETUP.md) — First-run setup wizard: vault selection, layout, feature presets, appearance, and keybinds with live preview
- [HELP.md](HELP.md) — Help view: 3-pane layout, up to 8 feature-filtered tabs, keybind index, tips, popup accordion
- [EDITOR.md](EDITOR.md) — Editor view: title/body editing and Markdown preview, find popup, soft-wrap, sidebars, wikilink previews, external editor

## Features

- [THEME_SYSTEM.md](THEME_SYSTEM.md) — Theme system: 19 built-in themes, color derivation, per-color overrides, AppThemeColors
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — Command palette and Action trait: available actions, registration, how to add new actions
- [TEMPLATES.md](TEMPLATES.md) — Note template system: TOML file format, template variables, CLI usage
- [IMAGE_RENDERING.md](IMAGE_RENDERING.md) — Native image rendering: ratatui-image, sixel/kitty/iTerm protocols, [image] config, cache/worker
- [SUBNOTES.md](SUBNOTES.md) — Subnotes: attached plaintext notes, grid tab, virtual tree folder, radial graph, manager popup

## Configuration

- [README.md](../README.md) — Quickstart, config example, keybind files, CLI commands
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — Full configuration reference: all config options, preset keybind files, graf config sections
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — Keybind presets: Helix, Vim, and Emacs presets and sequence syntax
---

- [UPGRADING.md](UPGRADING.md) — Retired features, legacy-data safety, stable filenames, and recovery privacy.
