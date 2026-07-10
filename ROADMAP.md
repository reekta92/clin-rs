# Roadmap

## Completed

- [X] **Theme system** — 19 built-in themes, backgrounds, per-color overrides, theme switcher
- [X] **Trash** — move notes/folders to trash, restore, empty trash
- [X] **OCR paste** — clipboard image → OCR text (`tesseract`) via command palette
- [X] **Canvas view (pinstar)** — Obsidian-compatible `.canvas` files, 4 node types, edges, context menu
- [X] **Draw view** — freehand drawing, shapes, text, `.draw` file format
- [X] **Obsidian .canvas import** — read and display existing Obsidian canvas files
- [X] **Command palette** — extensible action system with search
- [X] **Encryption** — on-demand ChaCha20-Poly1305, `.clin` files
- [X] **Templates** — TOML-based with variable substitution
- [X] **Markdown preview** — built-in renderer (comrak + syntect) in list preview and editor split pane
- [X] **External editor** — VISUAL/EDITOR env or configured command
- [X] **Folder management** — create, rename, move, collapse/expand
- [X] **Tag management** — add, remove, filter by tags
- [X] **Sorting & pinning** — sort by title/modified, pin notes to top
- [X] **Custom keybinds** — fully rebindable via keybinds.toml
- [X] **Graph view full integration (graf)** — `graf` is no longer external; physics, minimap, legend, search, config
- [X] **Tree outline** — note hierarchy from headers
- [X] **Text search** — search note content via `grep`/`ripgrep`
- [X] **Batch tagging** — tag multiple notes at once
- [X] **Link objects** — connect objects with lines
- [X] **Grouping** — merge objects into groups
- [X] **PDF import/export** — convert PDFs to/from markdown
- [X] **CSV to markdown** — import CSV tables
- [X] **URL import** — fetch article content as formatted markdown
- [X] **Git integration** — vault versioning and backup
- [X] **Grid view notes** — file manager like grid/icon view in notes list
- [X] **Daily goals** — daily word count and note count goals with in-app progress bars
- [X] **Calendar** — month calendar widget with note activity in the notes list view

- [X] **Setup wizard** — first-run single-screen onboarding: theme, background, hint bar style, icon mode, keybind preset cycling with live markdown preview
- [X] **Modular custom themes** — drop-in TOML themes in ~/.config/clin/themes/, no recompile
- [X] **Expanded theme library** — 19 built-in themes (added Catppuccin Frappé/Macchiato, Rose Pine Moon, Gruvbox Material, GitHub Dark, Ayu Mirage, Synthwave '84, Material)
- [X] **Show-all-files mode** — list every vault file, non-notes open in OS default app
- [X] **Folders-first toggle** — folders_before_files config + Ctrl+H shortcut
- [X] **Path expansion** — ~ and $VAR/${VAR} expansion in storage_path
- [X] **Smart folders** — virtual smart folders (Today, This Week, Untagged) with custom rules (tags, title, folder, age)
- [X] **Sub-notes** — virtual encrypted notes attached to physical notes, with full management UI
- [X] **Word frequency** — show most used words in note info popup
- [X] **Word & character metrics** — writing statistics and goals

## Planned

#### General
- [ ] **Consistent UI/UX** — literally hardest part of making a TUI, the UI/UX must be consistent accross the app
- [ ] **More filtypes** — more text filetypes(`org-mode`, `.gv`, `.puml`, `.md` mermaid, `.dot`) support to edit/view
- [ ] **Image rendering** — image rendering logic throught the app using `sixel` or `kitty image protocol`

#### Notes View

#### Editor
- [ ] **Rework as side panel** — replace editor view with a feature-rich side panel
- [ ] **Cursor insert** — insert content at cursor from command palette actions

#### Graph View
- [ ] **Date/time linking** — categorize nodes by note date
- [ ] **Create links** — create/remove wikilinks from graph view
- [ ] **Assign tags** — tag notes directly from graph
- [ ] **Right-click menu** — context actions on nodes

#### Canvas
- [ ] **Insert note links** — embed note references as objects
- [ ] **QOL** — UI improvements for telling the state of the node, text alignment options for nodes
- [ ] **Orthagonal connections** — arrow like connections between nodes, toggleable
- [ ] **Group titlebar** — add clickable titlebar for group nodes for easier navigation
- [ ] **Connection properties** — more properties for connections between nodes; color, type, text etc.
- [ ] **Node properties** — more properties for nodes like shapes(as tags), border type etc.
- [ ] **New node types** — more node types like link nodes, image nodes etc.

#### Draw
- [ ] **Text size** — changable text size
- [ ] **UI indicators** — indicators like how big is the canvas, the scale etc.
- [ ] **Draw smoothing** — experimental draw smoothing using a algorithm to redraw the last drawed section

#### Command Palette
- [ ] **Merge/split notes** — combine or divide notes
- [ ] **Advanced clipboard** — multi-selection copy/paste
- [ ] **Date calculator** — date/time calculator for doing operations like "today + 3 months" or "today <> 12/12/2026"(diff operation) etc.

#### To-Do
- [ ] **To-do view** — a new view for specifically managing to-dos
- [ ] **Scrum table support** — support for creating interactive scrum tables like to-do, doing, done etc.
- [ ] **Tasks** — longterm to-dos basically, can remind the user with a notification
- [ ] **todo.txt** — todo.txt standardization support

#### Databases
- [ ] **Database view** — database feature similar to Obsidian's databases

#### Configuration
- [ ] **Status line customization** — `status_format = "{title} | {word_count} words"`
- [ ] **Plugin support** — Lua scripting

#### Other
- [ ] **Calendar/time tools** — date calculator, timezone converter
- [ ] **AOD pinning** — overlay note on screen
