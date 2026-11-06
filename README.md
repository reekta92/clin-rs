<div align="center">

<p align="center">
  <img width="256" height="256" alt="clin logo" src="assets/icon.svg" />
</p>

# clin

**Your Markdown vault, in your terminal.**

A free, open-source note manager inspired by [Obsidian](https://obsidian.md/), built in Rust.
Write, connect, organize, and schedule notes in one terminal app.

[![GitHub release](https://img.shields.io/github/v/release/reekta92/clin-rs.svg?logo=github)](https://github.com/reekta92/clin-rs/releases)
[![CI](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/reekta92/clin-rs/actions/workflows/ci.yml)
[![last commit](https://img.shields.io/github/last-commit/reekta92/clin-rs.svg)](https://github.com/reekta92/clin-rs/commits/main)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

[Features](#features) · [Install](#installation) · [Quick start](#quick-start) · [Wiki](https://github.com/reekta92/clin-rs/wiki)

</div>

## Screenshots

<table>
  <tr>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/138f402c-6e4a-4b9d-a1a7-6ef1dfff623b" alt="Notes view" width="100%"><br>
      <b>Notes</b><br>Browse your vault and preview(and edit) notes without leaving the terminal.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/4286b5e3-9c3a-4292-bbac-b999cd564313" alt="Editor view" width="100%"><br>
      <b>Editor</b><br>Markdown first text editor with dynamic styling, preview pane, zen mode for focused editing, text alignment including left-right or justified alignments and more. External editors also supported out of the box.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/c08d95db-9677-4754-9a5a-3fa710343fdb" alt="Navigation view" width="100%"><br>
      <b>Navigation</b><br>Both mouse and keyboard friendly workflows with visual feedback and modern sleek designed UI/UX.
    </td>
  </tr>
  <tr>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/1b7afd4f-623e-40fb-a8da-95523c7dbe12" alt="Calendar view" width="100%"><br>
      <b>Calendar</b><br>Calendar view to manage your tasks/events, with iCal and CalDav support.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/03bc1d77-d616-443a-9eb2-8ee7600750e0" alt="Organization view" width="100%"><br>
      <b>Organization</b><br>All the features you need to manage your knowledge base; use tags, sort notes into smart folders with custom rules, create note templates, create subnotes that are inside a note, create backward/forward links for related notes and many more.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/2d9c4810-e4b3-4c91-8589-cc0a4f237598" alt="Graph view" width="100%"><br>
      <b>Graph</b><br>Graph view powered by <a href="https://github.com/reekta92/graf">graf-rs</a> with realtime physics simulation, edit or create connections directly from the graph view, view local graphs only and more.
    </td>
  </tr>
  <tr>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/5f431a51-be2a-45df-9bb8-ac3608c66026" alt="Canvas view" width="100%"><br>
      <b>Canvas</b><br>Create canvases or edit .canvas files, powered by <a href="https://github.com/reekta92/pinstar">pinstar</a> create nodes, connections; edit node properties like colors, size, title and more; render images directly in the canvas; create group nodes to tidy all the nodes however you want, compatible with Obsidian.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/9ee57005-fc27-43b8-8f3a-5dda753b3dfb" alt="Drawing view" width="100%"><br>
      <b>Drawing</b><br>Create freehand drawing using braille characters powered by <a href="https://github.com/reekta92/draw-rs">draw-rs</a> edit drawings by rotating scaling or coloring them, create shapes, add text and more.
    </td>
    <td valign="top" width="33%">
      <img src="https://github.com/user-attachments/assets/9c6e8584-c8d2-487b-84b7-f990e0f6563f" alt="Bases view" width="100%"><br>
      <b>Bases</b><br>Create, interactively edit and configure .base files for creating custom databases or views like kanban, cards, table etc. to render dynamically inside note files, compatible with Obsidian.
    </td>
  </tr>
</table>

## Features

| Workflow | What you can do | Guide |
|---|---|---|
| **Notes & editor** | Browse folders, search, tag, pin, and edit Markdown with previews, wikilinks, and an external editor. | [Notes](docs/LIST_VIEW.md) · [Editor](docs/EDITOR.md) |
| **Graph** | Explore connections between notes through an interactive, force-directed wikilink graph. | [Graph](docs/GRAPH_VIEW.md) |
| **Canvas** | Arrange text, files, links, and images on an infinite canvas using Obsidian's `.canvas` format. | [Canvas](docs/CANVAS.md) |
| **Bases** | Query and organize note properties in tables, lists, cards, and Kanban layouts. | [Bases](docs/BASES.md) |
| **Calendar** | Schedule linked notes, switch month/week/day/agenda views, import/export iCalendar, and sync manually with CalDAV. | [Calendar](docs/CALENDAR.md) |
| **Drawing** | Sketch freehand, add shapes and text, and pan or zoom your drawings. | [Draw](docs/DRAW.md) |
| **Themes** | Choose from 19 built-in themes or supply a custom TOML theme; customize colors, icons, and status lines. | [Themes](docs/THEME_SYSTEM.md) |

Also includes [templates](docs/TEMPLATES.md), [outlines](docs/OUTLINE.md), [subnotes](docs/SUBNOTES.md), daily writing goals, and file/URL imports with optional conversion and OCR tools.

## Installation

Download a package from [Releases](https://github.com/reekta92/clin-rs/releases), or use your preferred package manager:

| Platform | Install |
|---|---|
| Linux, Windows, macOS (from source) | `cargo install clin-rs --locked` — Rust 1.90.0+ |
| Arch Linux | `yay -S clin-rs-bin` |
| Nix | `nix run github:reekta92/clin-rs` — flakes enabled |
| Debian / Ubuntu | `.deb` for amd64 or arm64 |
| Fedora / RHEL | `.rpm` for x86_64 or aarch64 |
| Other Linux | Archives for x86_64/aarch64; AppImage for x86_64 |
| Windows | x86_64 `.zip` |
| macOS | Apple Silicon `.dmg` or archive; no Intel prebuilt binary |

See the [installation guide](docs/INSTALLATION.md) for platform commands, source builds, and optional tools. A modern terminal is recommended; native images depend on terminal support. Nerd Fonts are optional: use Unicode, emoji, or text-only icons with a regular font.

> **Upgrading an encrypted vault?** This release uses plaintext and no longer provides encryption or backups. Preserve your originals and keys; use the previous release to decrypt notes and recover legacy drafts. Read [Upgrading](docs/UPGRADING.md) first.

## Quick start

```sh
clin                                         # Launch; first run opens setup
clin --vault /path/to/vault                   # Open an existing vault
clin notes quick "Meeting notes" "Standup"    # Save a note without opening the TUI
```

With the default keybind preset:

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move through the notes list |
| `Enter` | Open the selected note |
| `Ctrl+Space` / `F6` | Open the contextual command palette |
| `Ctrl+G` | Open the graph from Notes |
| `?` / `F1` | Open help from Notes |
| `F2` | Show current keybindings |
| `F4` | Switch vaults |

Use [setup](docs/SETUP.md) to choose your layout, features, appearance, and keybind preset. Open an existing Obsidian vault for Markdown, Canvas, and Bases workflows; Obsidian plugins are not executed. Review [Canvas](docs/CANVAS.md#obsidian-compatibility) and [Bases](docs/BASES.md) compatibility before relying on those workflows.

## CLI commands

Run `clin --help` or `clin notes --help` to discover commands. The [CLI guide](docs/CLI.md) covers note creation, shell input, templates, vault management, configuration, and cache commands.

## Documentation

Browse the **[Wiki](https://github.com/reekta92/clin-rs/wiki)** for all guides, or the [repository documentation index](docs/INDEX.md).

- **Customize:** [configuration](docs/CONFIG_REFERENCE.md), [keybind presets](docs/KEYBIND_PRESETS.md), [command palette](docs/COMMAND_PALETTE.md).
- **Terminal setup:** [images](docs/IMAGE_RENDERING.md), [clipboard and SSH](docs/EDITOR.md#clipboard-transport), [optional tools](docs/INSTALLATION.md#optional-tools).
- **Project:** [roadmap](ROADMAP.md), [architecture](docs/ARCHITECTURE.md), [contributing](CONTRIBUTING.md), [report a bug](https://github.com/reekta92/clin-rs/issues).

The Wiki syncs from `main`; documentation changes belong in the repository source files.

## Support & credits

Support development on [Patreon](https://www.patreon.com/MehmetDag/posts/clin-rs-project-163784358). Licensed under [GPL-3.0](LICENSE).

Built with [Ratatui](https://ratatui.rs), [Crossterm](https://github.com/crossterm-rs/crossterm), [graf](https://github.com/reekta92/graf), [pinstar](https://github.com/reekta92/pinstar), and [draw-rs](https://github.com/reekta92/draw-rs). Markdown previews use comrak and syntect.

<div align="left">

[![Patreon](https://img.shields.io/badge/Patreon-Support%20clin-f96854.svg?logo=patreon&logoColor=white)](https://www.patreon.com/MehmetDag/posts/clin-rs-project-163784358)
[![GitHub Downloads](https://img.shields.io/github/downloads/reekta92/clin-rs/total?label=Source&color=2ea44f&logo=github)](https://github.com/reekta92/clin-rs/releases)
[![Crates.io Downloads](https://img.shields.io/crates/d/clin-rs?label=crates.io&color=2ea44f&logo=rust)](https://crates.io/crates/clin-rs)



<a href="https://terminaltrove.com/clin" title="This tool is Tool of The Week on Terminal Trove, The $HOME of all things in the terminal"><img src="https://cdn.terminaltrove.com/media/badges/tool_of_the_week/svg/terminal_trove_tool_of_the_week_green_on_dark_grey_bg.svg" alt="Terminal Trove Tool of The Week" height="60" /></a>

</div>
