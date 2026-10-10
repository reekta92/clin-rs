# List View (Notes)

The List view is the primary interface for browsing, searching, and managing your notes. It supports multiple layouts and rich previews for various file formats.

---

## Overview

The List view (`ViewMode::List`) provides a flexible way to interact with your note library. Whether you prefer a visual grid of cards or a structured file tree, the List view adapts to your workflow.

**Source:** `src/app.rs`, `src/ui/list_view.rs` (List rendering logic)

---

## Layout Options

You can toggle between two primary layouts:

### 1. Grid Layout (Default)
The Grid layout displays notes as cards. It is optimized for visual recognition and quick browsing.
- **Tabs**: Quickly switch between the full **Vault** and your **Pinned** notes.
- **"Create new..." Tile**: A dedicated tile in the grid that opens the format chooser to quickly start a new note.

### 2. Tree Layout
The Tree layout provides a hierarchical view of your folders and notes, similar to a traditional file explorer. It is ideal for navigating complex vault structures.
- **Smart Virtual Folders**: Dynamic groups like *Today* (updated in last 24h), *This Week*, *Untagged*, and one folder per *Tag*. Control with `[features] smart_folders = true` (enabled by default). *This Week* means updated within the last seven days, not the calendar week. Custom rules live in `[list] custom_smart_folders`.
- **Folder Pinning**: Pin folders to the top of the list for quick access by selecting a folder and pressing the pin key (`p`).
- **Inline Rename**: Rename notes and folders directly in the tree list by pressing the rename key (`r`). Press `Enter` to commit, or `Esc` to cancel.
- **Drag-to-Move**: Click and drag notes onto folders to move them. Alternatively, use the default `U` keyboard shortcut (`g u` in Helix/Vim presets) to move the selected note to its parent directory.
---

## Previews and Rendering

The List view features a configurable preview pane that renders the contents of the selected note:
- **Markdown**: Renders formatted text, lists, and code blocks.
- **Canvas Snapshots**: Shows a static preview of `.canvas` files.
- **Draw Snapshots**: Shows a static preview of `.draw` files.
- **Images**: Shows supported image files through terminal graphics when `[features] images` is enabled.
- **Encrypted notes**: Content previews require `[list] preview_encryption = true`; metadata remains visible.

### Preview Configuration
The preview pane can be toggled on/off and positioned on the left or right in configuration.

---

## Note Creation

When creating a new note (via the "Create new..." tile or keyboard shortcuts), a **Format Chooser** popup appears allowing you to select the file type:
- **Markdown (.md)**: Standard formatted text notes.
- **Plain Text (.txt)**: Unformatted text files.
- **Draw (.draw)**: Infinite canvas for hand-drawn diagrams and sketches.
- **Canvas (.canvas)**: Interactive node-based mapping.

---

## Core Interactions

### Organization
- **Pinning**: Important notes can be pinned to appear at the top of the grid or in the "Pinned" tab.
- **Sorting**: Sort your notes by title or last modified date, ascending or descending. Default: modified, descending.
- **Folders**: Organize notes into nested directories.

### Discovery
- **Searching**: Use the built-in search popup to find notes by title or content.
- **Filtering**: Search tokens include `f:` (folder), `g:` (body/grep), `p:` (pinned), `t:` (tag), and `sn:` (exclusive subnote title/content search). Plain text searches titles; tag and subnote tokens require their features to be enabled.

## Bottom Strip Widgets

The bottom strip is a configurable section below the notes list that displays up to two widgets at a time. Configure it via the `sections` array in the `[list]` config section (e.g., `sections = ["calendar", "goals"]`). The `[features] calendar` flag controls whether the strip is shown at all. Each widget also respects its corresponding feature toggle; legacy `[list] calendar_enabled` is migrated and then ignored.

Available widgets:
- **`calendar`**: A rolling-weeks GitHub-style activity heatmap showing note activity over time. The start day is configured via `week_start` (`"sunday"` or `"monday"`).
- **`goals`**: Daily word-count and note-count progress bars. Configure targets via the `[goals]` config section.
- **`draw`**: A mini preview pane for recent `.draw` files.
- **`graf`**: A mini preview pane for graph visualizations.
- **`todo`**: A todo.txt task widget.
