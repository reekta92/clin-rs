# Roadmap

Tracking planned features and improvements for clin.

✅ = Completed | 🚧 = In Progress | ⬜ = Planned

## Completed

- [X] **Theme system** — 11 built-in themes, backgrounds, per-color overrides, theme switcher
- [X] **Trash** — move notes/folders to trash, restore, empty trash
- [X] **OCR paste** — clipboard image → OCR text (`tesseract`) via command palette
- [X] **Canvas view (pinstar)** — Obsidian-compatible `.canvas` files, 4 node types, edges, context menu
- [X] **Draw view** — freehand drawing, shapes, text, `.draw` file format
- [X] **Obsidian .canvas import** — read and display existing Obsidian canvas files
- [X] **Command palette** — extensible action system with search
- [X] **Encryption** — on-demand ChaCha20-Poly1305, `.clin` files
- [X] **Templates** — TOML-based with variable substitution
- [X] **Markdown preview** — `glow`-based rendering in list preview and editor split pane
- [X] **External editor** — VISUAL/EDITOR env or configured command
- [X] **Folder management** — create, rename, move, collapse/expand
- [X] **Tag management** — add, remove, filter by tags
- [X] **Sorting & pinning** — sort by title/modified, pin notes to top
- [X] **Custom keybinds** — fully rebindable via keybinds.toml
- [X] **Graph view full integration (graf)** — `graf` is no longer external; physics, minimap, legend, search, config

## Planned

#### Notes View
- [ ] **Text search** — search note content via `grep`/`ripgrep`
- [ ] **Smart folders** — auto-move tagged notes to specific folders
- [ ] **Word & character metrics** — writing statistics and goals
- [ ] **Batch tagging** — tag multiple notes at once

#### Editor
- [ ] **Rework as side panel** — replace editor view with a feature-rich side panel
- [ ] **Cursor insert** — insert content at cursor from command palette actions

#### Graph View
- [ ] **Date/time linking** — categorize nodes by note date
- [ ] **Create links** — create/remove wikilinks from graph view
- [ ] **Assign tags** — tag notes directly from graph
- [ ] **Right-click menu** — context actions on nodes

#### Canvas
- [ ] **Link objects** — connect objects with lines
- [ ] **Grouping** — merge objects into groups
- [ ] **Insert note links** — embed note references as objects

#### Command Palette
- [ ] **PDF import/export** — convert PDFs to/from markdown
- [ ] **CSV to markdown** — import CSV tables
- [ ] **URL import** — fetch article content as formatted markdown
- [ ] **Sub-notes** — virtual encrypted notes attached to physical notes
- [ ] **Merge/split notes** — combine or divide notes
- [ ] **Advanced clipboard** — multi-selection copy/paste
- [ ] **Dynamic variables** — insert realtime values
- [ ] **Word frequency** — show most used words
- [ ] **Git integration** — vault versioning and backup

#### Configuration
- [ ] **Status line customization** — `status_format = "{title} | {word_count} words"`
- [ ] **Plugin support** — Lua scripting

#### Other
- [ ] **Tree outline** — note hierarchy from headers
- [ ] **Calendar/time tools** — date calculator, timezone converter
- [ ] **AOD pinning** — overlay note on screen
