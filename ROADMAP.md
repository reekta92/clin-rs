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
- [X] **Outline** — note hierarchy from headers
- [X] **Text search** — search note content via internal `SearchWorker`
- [X] **Batch tagging** — tag multiple notes at once
- [X] **Link objects** — connect objects with lines
- [X] **Grouping** — merge objects into groups
- [X] **PDF import** — convert PDFs to markdown
- [X] **CSV to markdown** — import CSV tables
- [X] **URL import** — fetch article content as formatted markdown
- [X] **Git integration** — vault versioning and backup
- [X] **Grid view notes** — file manager like grid/icon view in notes list
- [X] **Daily goals** — daily word count and note count goals with in-app progress bars
- [X] **Calendar** — rolling-week activity heatmap widget in the notes list view

- [X] **Setup wizard** — first-run single-screen onboarding: theme, background, hint bar style, icon mode, keybind preset cycling with live markdown preview
- [X] **Modular custom themes** — drop-in TOML themes in ~/.config/clin/themes/, no recompile
- [X] **Expanded theme library** — 19 built-in themes (added Catppuccin Frappé/Macchiato, Rose Pine Moon, Gruvbox Material, GitHub Dark, Ayu Mirage, Synthwave '84, Material)
- [X] **Show-all-files mode** — list every vault file, non-notes open in OS default app
- [X] **Native image rendering** — pixel image rendering via ratatui-image (sixel/kitty/iTerm/halfblocks auto-detected) across canvas, draw, notes preview, and editor preview; configurable via `[image]` section
- [X] **Folders-first toggle** — folders_first config + Ctrl+H shortcut
- [X] **Path expansion** — ~ and $VAR/${VAR} expansion in storage_path
- [X] **Smart folders** — virtual smart folders (Today, This Week, Untagged) with custom rules (tags, title, folder, age)
- [X] **Sub-notes** — virtual encrypted notes attached to physical notes, with full management UI
- [X] **Word frequency** — show most used words in note info popup
- [X] **Word & character metrics** — writing statistics and goals
- [X] **Draw smoothing** — drawing canvas stroke smoothing implemented via a binomial filter
- [X] **Status line customization** — fully customizable headers and footers per view layout via `[statusline]`
- [X] **Canvas navigation** — mouse-drag panning and zoom-to-cursor for canvas and draw previews
- [X] **Global UI hover highlights** — mouse hover highlights for TUI panels and interactive lists
- [X] **Editor enhancements** — right-click editor context menu and merged title bar
- [X] **Help view (3-pane)** — tabbed help with auto-generated keybind index, per-tab descriptions, popup accordion, preset-aware tips, page indicator
- [X] **Subnotes browsable view** — Subnotes grid tab + virtual tree folder in notes list, radial braille graph with zoom/pan, subnotes manager popup (add/edit/delete, encrypted)
- [X] **Editor READ/EDIT modes** — modal editing with read-mode select+clipboard, mode highlight, source-line map for READ↔EDIT scroll sync
- [X] **Editor find popup** — custom find popup replacing textarea search
- [X] **Editor soft-wrap toggle** — configurable soft-wrap for the editor body
- [X] **Editor sidebars + wikilink previews** — forward/back links pane alongside the editor
- [X] **Insert date action** — command-palette action inserting the current date at cursor (configurable format)
- [X] **Show Info popup** — per-note and per-folder metrics (word/char count, headers, tasks, top words)
- [X] **Auto-refresh** — notify-based watcher reloads the notes list on external file changes (configurable via `core.auto_refresh`)
- [X] **Quick search redesign** — in-header-bar quick search with mouse support
- [X] **Preview pane enhancements** — zoom-to-cursor, mouse-drag panning, in-memory cache, scale-to-fill on expand
- [X] **Tree folder enhancements** — expand-all, expand-to-level (with count prefix), recursive folder counts in header, folder state persistence, dim empty folders
- [X] **Scrollbars** — scrollbars on all scrollable panes
- [X] **Global hover highlights** — mouse hover highlights across all interactive TUI panels and lists
- [X] **Backup libgit2 auth callbacks** — push/pull auth callbacks for remote sync
- [X] **Config-gated markdown features** — per-feature toggles for syntax highlighting, code theme, code line numbers, preview wrap, wrap indicator, link URL max length
- [X] **Pin status preserved through encrypt/decrypt** — pinned flag survives `.md` ↔ `.clin` conversion
- [X] **Hint bar style customization** — custom `hint_bar_style` options (`classic`, `sharp`, `rounded`, `slanted`, `bubbles`, `blur`, `chips`, `brackets`, `compact`, `sharp_gradient`, `rounded_gradient`, `slanted_gradient`, `hexagon`)
- [X] **CLI mode** — list, quick-note, find, config, storage, and keybind tools via `clin` subcommands

## Planned

#### General
- [ ] **Consistent UI/UX** — literally hardest part of making a TUI, the UI/UX must be consistent accross the app
- [ ] **More filtypes** — more text filetypes(`org-mode`, `.gv`, `.puml`, `.md` mermaid, `.dot`) support to edit/view

#### Notes View
- [ ] **Notebook files** — a text file type where you can embed interactable drawings(via `draw`), schemes(via `pinstar`) etc. At it's core it's a markdown file with a special property, so it will be compatible with other `.md` editors.

#### Edit View
- [ ] **Actions side pane** — a side pane that allows you to do some special actions like inserting an OCR result etc.
- [ ] **Cursor insert** — insert content at cursor from command palette actions

#### Graph View
- [ ] **Date/time linking** — categorize nodes by note date
- [ ] **Create links** — create/remove wikilinks from graph view
- [ ] **Assign tags** — tag notes directly from graph
- [ ] **Right-click menu** — context actions on nodes
- [ ] **Looking glass** — similar to minimap, a enlarged version of the selected note for easier identification.

#### Pinstar View
- [ ] **Insert note links** — embed note references as objects
- [ ] **QOL** — UI improvements for telling the state of the node, text alignment options for nodes
- [ ] **Orthagonal connections** — arrow like connections between nodes, toggleable
- [ ] **Group titlebar** — add clickable titlebar for group nodes for easier navigation
- [ ] **Connection properties** — more properties for connections between nodes; color, type, text etc.
- [ ] **Node properties** — more properties for nodes like shapes(as tags), border type etc.
- [ ] **New node types** — more node types like link nodes, etc. (image nodes implemented as placeholders)

#### Draw View
- [ ] **Text size** — changable text size
- [ ] **UI indicators** — indicators like how big is the canvas, the scale etc.

#### Command Palette
- [ ] **Merge/split notes** — combine or divide notes
- [ ] **Advanced clipboard** — multi-selection copy/paste
- [ ] **Date calculator** — date/time calculator for doing operations like "today + 3 months" or "today <> 12/12/2026"(diff operation) etc.

#### To-Do View
- [ ] **To-do view** — a new view for specifically managing to-dos
- [ ] **Scrum table support** — support for creating interactive scrum tables like to-do, doing, done etc.
- [ ] **Tasks** — longterm to-dos basically, can remind the user with a notification
- [ ] **todo.txt** — todo.txt standardization support

#### Project Management View
- [ ] **Project management view** — a new view that specifically focuses on managing project documentation files/wikis.
- [ ] **Assigning docs to files** — assign specific sections of a document file or the entire file to 1 or more project files so when any of the project files updates, user is notified that the documentation requires updates too.
- [ ] **Notifications** — notify users when documents/wiki requires update.
- [ ] **Context aware search** — a search action that allows user to search a context that shows similar results from the docs/wiki. For example searching "command palette/how" shows the "How" section of the "Command Palette" system.

#### Databases View
- [ ] **Database view** — database feature similar to Obsidian's databases

#### Configuration
- [ ] **Plugin support** — a plugin system to allow users create new views, command palette actions, preview pane types for specific filetypes and possibly custom popups

#### Other
- [ ] **Calendar/time tools** — date calculator, timezone converter
