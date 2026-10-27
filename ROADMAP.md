# Roadmap

Project-level ideas are not release commitments. Current functionality is
documented in the [README](README.md) and [documentation index](docs/INDEX.md).
Use the [release checklist](docs/RELEASE_CHECKLIST.md) for candidate acceptance;
track individual proposals and blockers in GitHub issues.

## Completed

- [x] **Orthagonal connections** — arrow like connections between nodes, toggleable
- [x] **Group titlebar** — add clickable titlebar for group nodes for easier navigation
- [x] **Connection properties** — more properties for connections between nodes; color, type, text etc.
- [x] **Create links** — create/remove wikilinks from graph view
- [x] **Right-click menu** — context actions on nodes
- [x] **Looking glass** — similar to minimap, a enlarged version of the selected note for easier identification.
- [x] **Consistent UI/UX** — literally hardest part of making a TUI, the UI/UX must be consistent accross the app
- [x] **todo.txt** — todo.txt standardization support
- [X] **Theme system** — 19 built-in themes, backgrounds, per-color overrides, theme switcher
- [X] **Trash** — move notes/folders to trash, restore, empty trash
- [X] **OCR paste** — clipboard image → OCR text (`tesseract`) via command palette
- [X] **Canvas view (pinstar)** — Obsidian-compatible `.canvas` files, 4 node types, edges, context menu
- [X] **Draw view** — freehand drawing, shapes, text, `.draw` file format
- [X] **Obsidian .canvas import** — read and display existing Obsidian canvas files
- [X] **Command palette** — extensible action system with search
- [X] **Templates** — TOML-based with variable substitution
- [X] **Markdown preview** — built-in renderer (comrak + syntect) in list preview and editor split pane
- [X] **External editor** — VISUAL/EDITOR env or configured command
- [X] **Folder management** — create, rename, move, collapse/expand
- [X] **Tag management** — add, remove, filter by tags
- [X] **Sorting & pinning** — sort by title/modified, pin notes to top
- [X] **Custom keybinds** — fully rebindable via keybinds.toml
- [X] **Graph view full integration (graf)** — upstream `graf` library with clin-owned preview, search, config, keybinds, and statusline
- [X] **Outline** — note hierarchy from headers
- [X] **Text search** — search note content via internal `SearchWorker`
- [X] **Batch tagging** — tag multiple notes at once
- [X] **Link objects** — connect objects with lines
- [X] **Grouping** — merge objects into groups
- [X] **PDF import** — convert PDFs to markdown
- [X] **CSV to markdown** — import CSV tables
- [X] **URL import** — fetch article content as formatted markdown
- [X] **Grid view notes** — file manager like grid/icon view in notes list
- [X] **Daily goals** — daily word count and note count goals with in-app progress bars
- [X] **Calendar** — rolling-week activity heatmap widget in the notes list view

- [X] **Setup wizard** — first-run onboarding: vault, theme, background, hint bar style, icons, notes layout, feature presets, and keybinds with live preview
- [X] **Modular custom themes** — drop-in TOML themes in the active config directory's themes/ subdirectory, no recompile
- [X] **Expanded theme library** — 19 built-in themes (added Catppuccin Frappé/Macchiato, Rose Pine Moon, Gruvbox Material, GitHub Dark, Ayu Mirage, Synthwave '84, Material)
- [X] **Show-all-files mode** — list every vault file, non-notes open in OS default app
- [X] **Native image rendering** — pixel image rendering via ratatui-image (sixel/kitty/iTerm/halfblocks auto-detected) across canvas, notes preview, and editor preview; controlled by `[features] images` and `[image]` settings (Draw v2 has no image elements)
- [X] **Folders-first toggle** — folders_first config + Ctrl+H shortcut
- [X] **Path expansion** — ~ and $VAR/${VAR} expansion in storage_path
- [X] **Smart folders** — virtual smart folders (Today, This Week, Untagged) with custom rules (tags, title, folder, age)
- [X] **Sub-notes** — virtual notes attached to physical notes, with full management UI
- [X] **Word frequency** — show most used words in note info popup
- [X] **Word & character metrics** — writing statistics and goals
- [X] **Draw smoothing** — drawing canvas stroke smoothing implemented via a binomial filter
- [X] **Status line customization** — fully customizable headers and footers per view layout via `[statusline]`
- [X] **Canvas navigation** — mouse-drag panning and zoom-to-cursor for canvas and draw previews
- [X] **Global UI hover highlights** — mouse hover highlights for TUI panels and interactive lists
- [X] **Editor enhancements** — right-click editor context menu and merged title bar
- [X] **Help view (3-pane)** — tabbed help with auto-generated keybind index, per-tab descriptions, popup accordion, preset-aware tips, page indicator
- [X] **Subnotes browsable view** — Subnotes grid tab + virtual tree folder in notes list, radial braille graph with zoom/pan, subnotes manager popup (add/edit/delete)
- [X] **Editor text and preview** — title/body editing, selection/clipboard, Markdown source highlighting, optional rendered preview (current editor has no READ/EDIT modal state)
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
- [X] **Config-gated markdown features** — per-feature toggles for syntax highlighting, code theme, code line numbers, preview wrap, wrap indicator, link URL max length
- [X] **Hint bar style customization** — custom `hint_bar_style` options (`classic`, `sharp`, `rounded`, `slanted`, `bubbles`, `blurred`, `chips`, `brackets`, `compact`, `sharp_gradient`, `rounded_gradient`, `slanted_gradient`, `hexagon`)
- [X] **CLI mode** — list/new/open/cat/quick/search, config, storage, keybind, template, and cache tools via `clin` subcommands
- [X] **Feature toggles** — `[features]` controls views/features; disabled entry points are hidden, deleted features release keybinds, image workers require restart
- [X] **Selection-aware metrics** — editor header shows selected/total word and character counts via statusline tokens
- [x] **todo.txt** — todo.txt standardization support
- [X] **Custom template variables** — folder-local variables and their configuration UI
- [X] **Bases** — public `.base` queries, four layouts, native editing and live embeds

## Planned

#### General
- [ ] **More filtypes** — more text filetypes(`.org`, `.gv`, `.puml`, `.md` mermaid, `.dot`) support to edit/view
- [ ] **Keybind settings** — set keybinds directly from the TUI OR open the keybind config at the default editor in the help view

#### Notes View
- [ ] **Notebook files** — a text file type where you can embed interactable drawings(via `draw`), schemes(via `pinstar`) etc. At it's core it's a markdown file with a special property, so it will be compatible with other `.md` editors.

#### Edit View
- [ ] **Actions side pane** — a side pane that allows you to do some special actions like inserting an OCR result etc.
- [x] **Properties edit** — hidden F7 properties/raw-YAML region above Body, lossless edits, managed-output policy, guarded recovery, and live Bases host metadata

#### Graph View
- [ ] **Date/time linking** — categorize nodes by note date
- [ ] **Assign tags** — tag notes directly from graph

#### Pinstar View
- [ ] **Insert note links** — embed note references as objects
- [ ] **QOL** — UI improvements for telling the state of the node, text alignment options for nodes
- [ ] **Additional node properties** — beyond shipped titles and text-node shapes, such as configurable border styles
- [ ] **Additional node types** — beyond shipped text/file/link/group nodes (image files already render through terminal graphics or placeholders)
- [ ] **Keyboard mode** — keyboard focused mode that allows for doing mouse actions like moving nodes, panning etc. with keyboard commands only, similar to `vim` commands
- [ ] **Jump to** — kinda like find in file/quick search, searches a string through entire node titles, content etc. allows for jumping to that node

#### Draw View
- [ ] **Text size** — changable text size
- [ ] **UI indicators** — indicators like how big is the canvas, the scale etc.

#### Command Palette
- [ ] **Merge/split notes** — combine or divide notes
- [ ] **Advanced clipboard** — multi-selection copy/paste
- [ ] **Date calculator** — date/time calculator for doing operations like "today + 3 months" or "today <> 12/12/2026"(diff operation) etc.

#### To-Do View & Calendar
- [ ] **To-do view** — a new view for specifically managing to-dos
- [ ] **Scrum table support** — support for creating interactive scrum tables like to-do, doing, done etc.
- [ ] **Tasks** — longterm to-dos basically, can remind the user with a notification
- [x] **Interactive calendar** — month/week/day/agenda with scheduled events and linked notes; see [Calendar](docs/CALENDAR.md)
- [x] **Calendar integration** — iCalendar import/export and manual CalDAV synchronization

#### Project Management View
- [ ] **Project management view** — a new view that specifically focuses on managing project documentation files/wikis.
- [ ] **Assigning docs to files** — assign specific sections of a document file or the entire file to 1 or more project files so when any of the project files updates, user is notified that the documentation requires updates too.
- [ ] **Notifications** — notify users when documents/wiki requires update.
- [ ] **Context aware search** — a search action that allows user to search a context that shows similar results from the docs/wiki. For example searching "command palette/how" shows the "How" section of the "Command Palette" system.

#### Configuration
- [ ] **Plugin support** — a plugin system to allow users create new views, command palette actions, preview pane types for specific filetypes and possibly custom popups

#### Other
- [ ] **Calendar/time tools** — date calculator, timezone converter
