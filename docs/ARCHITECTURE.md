# Architecture

Overview of clin-rs system architecture — view state machine, event loop, rendering pipeline, data flow, and threading model.

---

## View State Machine

`App` has a `mode: ViewMode` enum that controls which view is active:

```rust
pub enum ViewMode {
    List,    // Note list / folder tree / search
    Edit,    // Built-in text editor
    Help,    // Tabbed help pages
    Graph,   // Force-directed graph (graf)
    Draw,    // Freehand drawing canvas
    Canvas,  // Obsidian-compatible node/edge canvas (pinstar)
    Backup,  // Git backup dashboard
    Outline,  // Header-based note outline
    Setup,   // First-run setup wizard
}
```

Transition rules:

```
List  ──Enter──► Edit
List  ──Ctrl+G─► Graph
List  ──Enter──► Draw    (on .draw file)
List  ──Enter──► Canvas  (on .canvas file)
Edit  ──Esc───► List
Graph ──Esc───► List
Draw  ──Esc───► List
Canvas──Esc───► List
List  ──?/F1──► Help
Help  ──Esc───► previous view
List  ──palette──► Backup   (via command palette backup.open)
List  ──palette──► Outline  (via command palette outline.open)
List  ──palette──► Setup   (via setup_wizard)
Setup ──Esc───► List
Backup ──Esc───► List
Outline ──Esc───► List
```

Each overlay view implements the [`OverlayView`] trait (see `src/overlay.rs`). Graph, Draw, Canvas, Backup, and Outline all integrate into the main event loop via `overlay_render()` and `overlay_handle_event()` — they do not take control of the terminal. Their state is owned by `App` and instantiated on view transition.

---

## Event Loop Structure

### Main Loop (`lib.rs` → `run_app()`)

```
main()
  └─ clap Cli::from_arg_matches() → Cli
      ├─ no subcommand → session::bootstrap_app() → run_tui_session() → run_app()
      ├─ --help / --version → clap output, exit
      ├─ Command::Notes → list/new/open/cat/quick/search
      │    └─ new opens TUI unless --body or --no-tui is supplied
      └─ Command::{Storage,Keybinds,Templates,Config,Cache} → operation, exit
```

### TUI Session (`run_tui_session`)

```
run_tui_session(app)
  ├─ session::start_session() → SessionGuard
  │    └─ raw mode, alternate screen, mouse capture, bracketed paste, terminal
  ├─ run_app(terminal, app)   ← main event loop
  ├─ session::finish_session() + SessionGuard cleanup
  └─ return Result
```

### Main Event Loop (`run_app()`)

`run_app()` owns generic application work: catalog/search/watcher
drains, backup scheduling, list and graph state, then generic rendering.

When `app.mode == ViewMode::Edit`, it enters `editor_session::run_editor_session`
and continues the generic loop only after Edit exits. Edit remains in same
process and uses same terminal, `App`, storage, workers, and event source.

```
generic loop
  └─ ViewMode::Edit
       └─ editor session
            ├─ initial dirty draw
            ├─ editor-only status, preview, and image polling
            ├─ poll and dispatch up to 64 ordered events
            ├─ coalesce only consecutive mouse-move or resize events
            └─ draw again only when dirty
```

The editor session does not drain catalog, watcher, search, or message
channels. Existing bounded channels retain their backpressure until the generic
loop resumes.

### Sub-view Overlays (OverlayView trait)

Five sub-views (Graph, Draw, Canvas, Backup, Outline) implement the [`OverlayView`] trait (see `src/overlay.rs`):

- [`overlay_render()`] — draws the overlay into a given screen area; called from `draw_ui()` during the main render pass
- [`overlay_handle_event()`] — handles one terminal event; returns [`OverlayResult`] indicating whether the overlay should stay active, exit, open help, or perform a view-specific action (open a note, jump to a line)

Their state is stored as `Option<X>` fields on `App` (e.g. `graph_plugin: Option<GrafPlugin>`, `draw_state: Option<DrawAppState>`, `canvas_state: Option<PinstarPlugin>`) . When the user enters Graph/Draw/Canvas/Backup/Outline, the state is created and owned by `App`. On exit, the state is dropped (set to `None`) and the previous view is restored via `return_mode`.

No sub-view takes terminal ownership or runs a separate event loop.

---

## App State (`app.rs`)

`App` is the central state struct. It owns everything:
```
App
  ├── storage: Storage                    // file I/O, encryption, templates
  ├── keybinds: Keybinds                  // <config-dir>/keybinds/<preset>.toml
  ├── notes: Vec<NoteSummary>             // filtered/sorted note list
  ├── editor: NoteEditor                  // title TextArea + EditorDocument body
  ├── mode: ViewMode                      // current active view
  ├── command_palette: Option<CommandPalette>  // Ctrl+P popup
  ├── popups: PopupManager                // confirm, folder, tag, template, theme popups
  ├── app_theme: AppThemeColors           // derived colors from theme config
  ├── return_mode: Option<ViewMode>       // where to return after overlay exit
  ├── graph_plugin: Option<GrafPlugin>         // clin adapter over upstream graf
  ├── draw_state: Option<DrawAppState>         // freehand drawing overlay
  ├── canvas_state: Option<PinstarPlugin>      // clin adapter over upstream pinstar
  ├── backup_state: Option<BackupState>        // git backup dashboard overlay
  ├── outline_state: Option<OutlineState>  // header-based outline overlay
  └── ...status helpers, config, caches
```

### Data flow
```
Storage (filesystem)
    │
    ▼
App::new(storage)
    │
    ├── App::refresh_notes()
    │     └─ storage.list_note_ids() → load_note_summary() → sort/filter
    │
    ├── User interaction
    │     └─ Mutates app state (editor, list, command_palette, popups)
    │
    ├── App::autosave()
    │     └─ storage.save_note() → writes to disk
    │
    └── Overlay state (graph_plugin, draw_state, canvas_state, backup_state, outline_state)
          └─ Owned by App as Option<X>. Created on view transition via mode change.
             Dropped (set to None) on overlay exit. No separate event loop.
```

---

## Rendering Pipeline

```
lib.rs: terminal.draw(|frame| draw_ui(frame, app, focus))
  │
  └─ draw_ui()
       ├─ dark background block (if solid bg mode)
       └─ match app.mode:
            ├─ List  → draw_list_view()
            ├─ Edit  → draw_edit_view()
            ├─ Help  → draw_help_view()
            ├─ Graph → graph_plugin.overlay_render(frame, area, theme, config, status)
            ├─ Draw  → draw_state.overlay_render(frame, area, theme, config, status)
            ├─ Canvas→ canvas_state.overlay_render(frame, area, theme, config, status)
            ├─ Backup→ backup_state.overlay_render(frame, area, theme, config, status)
            ├─ Outline → outline_state.overlay_render(frame, area, theme, config, status)
            └─ Setup → draw_setup()
       │
       └─ if theme popup → draw_theme_popup()
```

List and Edit views use ratatui's `Layout` to split the terminal into panes:

**List view layout:**
```
┌─ Notes Pane ──┬──── Preview Pane ────┐
|  folder tree  │  markdown (built-in) or  │
│  note list    │  text preview        │
│  search bar   │                      │
├────── Status Bar ────────────────────┤
└──────────────────────────────────────┘
```

**Edit view layout:**
```
┌─ Title Bar ────────────────────────┐
│  [Title input]                     │
├─ Body Editor ───┬─ MD Preview ────┤
|  (TextArea)     │  (built-in render)  │
│                  │                  │
├── Status Bar ──────────────────────┤
└────────────────────────────────────┘
```

**Help view layout:**
```
┌─ Tab Bar (Notes · Editor · Graph · Draw · Canvas · Backup · Templates · About) ─┐
│                                                              │
│  Keybind index       │ Tab description + popup accordion     │
│  (paginated)         ├───────────────────────────────────────┤
│                      │ Suggested tips                       │
│                                                              │
├─ Hint line ──────────────────────────────────────────────────┤
└──────────────────────────────────────────────────────────────┘
```

---

## Module Map

```
src/
├── bin/
│   └── clin.rs           — Binary entry point; main() calls lib::run()
├── lib.rs                — Crate root: entry point, TUI loop, CLI dispatch
├── app.rs                — Central App struct, ViewMode enum, top-level API
├── app_theme.rs          — AppThemeColors derivation from UiConfig
├── calendar.rs           — GitHub-style activity heatmap
├── cli.rs                — CLI argument definitions (clap-derive)
├── console.rs            — Colored CLI output and clap theme
├── editor.rs             — NoteEditor title/body focus, sidebar, preview state
├── editor_document.rs    — EditorDocument body, revision, snapshots, change contract
├── editor_session.rs     — Dedicated event loop for edit mode
├── event_source.rs       — Crossterm/channel event stream abstraction
├── frontmatter.rs        — YAML frontmatter parse/serialize
├── fsutil.rs             — Atomic file I/O and secure temp files
├── goals.rs              — Writing goals progress tracking and rendering
├── list_view.rs          — ListView state, VisualItem, PreviewContent, sort
├── local_state.rs        — Versioned local state persistence
├── migration.rs          — File migration with interactive conflict resolution
├── note_index.rs         — In-memory note index for search and filtering
├── overlay.rs            — OverlayView trait, OverlayResult enum
├── palette.rs            — CommandPalette popup widget
├── paths.rs              — Platform-aware application path discovery
├── perf_tests.rs         — Performance benchmarks (ignored by default)
├── popups.rs             — Modal popup types and PopupManager
├── preview.rs            — Preview pane dispatcher
├── setup.rs              — First-run setup wizard constants and state
├── session.rs            — Terminal bootstrap and teardown orchestration
├── statusline.rs         — Statusline/header/footer template rendering
├── storage.rs            — Note persistence, encryption, vault management
├── todo.rs               — todo.txt parsing and rendering
├── app/                  — App logic: catalog, edit panes, folder preview, etc.
│   ├── catalog.rs        — Background note catalog worker
│   ├── edit_panes.rs     — Editor sidebar management
│   ├── folders.rs        — Folder tree, move, duplicate, pin
│   ├── import_ops.rs     — File/URL import orchestration
│   ├── loading.rs        — Catalog refresh, previews, subnote virtual folders
│   ├── messages.rs       — Status/message overlay and queue
│   ├── notes.rs          — Core note lifecycle
│   ├── popups.rs         — Non-editor popup dialogs
│   ├── search.rs         — Search popup UI
│   ├── search_worker.rs  — Background search worker (rayon)
│   ├── settings_ops.rs   — Toggleable settings, layout persistence
│   ├── tags.rs           — Tag CRUD operations
│   ├── trash.rs          — Trash lifecycle
│   ├── vaults.rs         — F4 vault switcher overlay: open/switch/add/remove
│   └── views.rs          — View-mode switching
├── config/               — Config schema, loading, merging, custom themes
│   ├── mod.rs            — ClinConfig lifecycle, re-exports
│   ├── structs.rs        — All config data structures
│   ├── types.rs          — Enum types and parsing
│   ├── merge.rs          — Comment-preserving TOML merge
│   ├── themes.rs         — Built-in theme palette definitions
│   ├── custom_themes.rs  — Drop-in TOML theme loading
│   ├── path.rs           — Path expansion (~, $VAR)
├── events/               — Keyboard/mouse event handlers per view
│   ├── mod.rs            — Shared utilities, popup dispatch
│   ├── list.rs           — List view key/mouse handlers
│   ├── edit.rs           — Edit view key/mouse handlers
│   ├── help.rs           — Help view key/mouse handlers
│   ├── setup.rs          — Setup wizard key/mouse handlers
│   └── popup_mouse.rs    — Centralized popup mouse dispatch
├── keybinds/             — Keybind loading, Keybinds struct, presets
│   ├── mod.rs            — Keybinds/KeybindsToml structs, re-exports
│   ├── types.rs          — Action enums for all scopes
│   ├── api.rs            — Persistence, macros, resolution, hints
│   ├── defaults.rs       — Default bindings + preset overrides
│   ├── combo.rs          — KeyCombo representation and parsing
│   ├── matcher.rs        — Key event matcher with sequence buffering
│   └── help_meta.rs      — Action metadata for help UI
├── ui/                   — Terminal rendering: draw_ui() and per-view renderers
│   ├── mod.rs            — Central UI dispatcher, shared helpers
│   ├── canvas_grid.rs    — Adaptive grid state/rendering for local Draw
│   ├── canvas_menu.rs    — Context menu for canvas/draw
│   ├── canvas_overlay.rs — Shared canvas drawing overlays (marquee, grid)
│   ├── canvas_selection.rs — Multi-select node/edge state
│   ├── list_view.rs      — Main list/grid view rendering
│   ├── edit_view.rs      — Editor body rendering
│   ├── help.rs           — Full help view
│   ├── message_overlay.rs— Toast/message popup overlay
│   ├── popups.rs         — Popup and status-bar rendering
│   ├── title_bar.rs      — Title bar / tab bar rendering
│   ├── setup.rs          — Setup wizard rendering
│   ├── vault_switcher.rs — Vault selection overlay
│   ├── quick_search.rs   — Generic quick-search popup
│   ├── scrollbar.rs      — Auto-hiding vertical scrollbar
│   ├── quick_keybinds.rs — Quick keybind-hint dropdown
│   └── braille.rs        — Braille sub-pixel dot and line drawing
├── actions/              — Action trait ecosystem and ACTIONS registry
│   ├── mod.rs            — Action trait, macros, ACTIONS LazyLock
│   ├── decrypt.rs        — Decrypt .clin to .md
│   ├── encrypt.rs        — Encrypt .md to .clin
│   ├── import.rs         — Import from external sources (File/CSV/JSON/URL/Clipboard)
│   ├── info.rs           — Show note/folder metrics popup
│   ├── insert_date.rs    — Insert current date/time at cursor
│   ├── ocr.rs            — OCR paste and image attachment
│   ├── outline.rs        — Open outline tree
│   ├── rasterize.rs      — Rasterize note spacing (remove blank lines)
│   └── settings.rs       — Toggle/cycle actions for all settings
├── markdown/             — GFM markdown rendering pipeline
│   ├── mod.rs            — MarkdownRenderer, render_builtin_sync
│   ├── builtin.rs        — Core comrak → grid renderer
│   ├── source_highlight.rs — Per-line source highlighter for editable Markdown
│   ├── style.rs          — RenderLine type, MarkdownTheme palette
│   ├── cache.rs          — Cached markdown output with revalidation
│   ├── todotxt.rs        — Render plugin for todo.txt items
│   ├── widget.rs         — Ratatui Widget impl for RenderLine slices
│   └── worker.rs         — Cancelable background render thread
├── templates.rs          — Template system: data model, substitution, persistence
├── image_render/         — Native image rendering
│   ├── cache.rs          — LRU image cache
│   └── worker.rs         — Background decode worker
├── backup/               — Git backup dashboard
│   ├── app.rs            — BackupState, OverlayView implementation
│   ├── git_ops.rs        — GitOps safe wrappers
│   ├── input.rs          — Keyboard/mouse event handling
│   ├── render.rs         — Dashboard rendering
│   ├── state.rs          — BackupState, BackupSection, BackupInputMode
│   └── worker.rs         — Background auto-backup worker
├── graf_adapter.rs       — GrafPlugin: host preview/search/config/status, note I/O
├── pinstar_adapter.rs    — PinstarPlugin: host keybinds/status/clipboard/images
├── snapshot.rs           — Canvas and Draw preview snapshots
├── draw/                 — Freehand drawing overlay
│   ├── geometry.rs       — Affine transform and bounding box math
│   ├── input.rs          — Mouse/keyboard handlers
│   ├── render.rs         — Canvas + element rendering
│   └── state.rs          — DrawData, DrawElement, DrawTool
├── outline/              — Header outline view
│   ├── app.rs            — OutlineState, OverlayView implementation
│   ├── input.rs          — Keyboard/mouse handlers
│   ├── render.rs         — Tree + detail rendering
│   ├── state.rs          — OutlineState, tree model
│   └── parse.rs          — Header outline parser
```

---

Graph construction, physics, spatial indexing, viewport, and node rendering live in upstream [`graf`](https://github.com/reekta92/graf-rs), not `src/graf/`. Canvas schema, state, gestures, and rendering live in upstream [`pinstar`](https://github.com/reekta92/pinstar). `Cargo.toml` and `Cargo.lock` pin the library versions; clin adapters own host integration. Draw and Outline remain local overlays.

## Threading Model

```
┌──────────────────────────────────────────────────────┐
│  Main Thread (event loop + rendering)                │
│  - crossterm event polling                           │
│  - ratatui Frame rendering                           │
│  - File I/O (Storage)                                │
│  - State mutation (App)                              │
└──────────────────────────────────────────────────────┘
         │
         │ spawn / join
         ▼
┌──────────────────────────────────────────────────────┐
│  Physics Thread (graf)                               │
│  - Runs fdg_sim force simulation in background       │
│  - tick_rate = auto/fixed; rate depends on node count │
│  - Continuous layout for up to 1,000 displayed nodes │
│  - Larger graphs use static clustered layout        │
│  - Stops via kill channel / sender disconnection     │
└──────────────────────────────────────────────────────┘
         │
         │ spawn / join (oneshot)
         ▼
┌──────────────────────────────────────────────────────┐
│  Markdown Render Thread                              │
│  - comrak GFM parse → AST walk → Vec<RenderLine>     │
│  - Optionally syntect-highlights fenced code blocks   │
│  - Runs in a cancelable background thread             │
│  - Result polled by main loop via poll_renderers()    │
└──────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────┐
│  Exit Watchdog Thread                                │
│  - Spawned once in run()                             │
│  - Hard-exits (code 130) on FORCE_QUIT, or on        │
│    SHOULD_EXIT not consumed within 3s                │
│  - Breaks the crossterm poll spin on a dead PTY      │
│    (issue #168); inert for CLI subcommands           │
└──────────────────────────────────────────────────────┘
```

---

Other background work includes catalog enumeration, search, folder previews, image decoding, and auto-backup. Worker results are installed by the host loop; Editor deliberately defers generic queues until it exits. Feature flags gate worker startup (images/backup changes require restart).

## Key Patterns

### Command Palette / Action System

Extensible action system via `Action` trait:

```rust
pub trait Action: Send + Sync {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;
}
```

Actions are registered in a `LazyLock<Vec<Box<dyn Action>>>` in `actions/mod.rs`. The command palette (`src/palette.rs`) provides a searchable popup. See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for details.

### Theme System

`UiConfig` from `[ui]` in config.toml → `AppThemeColors` derived at load time. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for details.

### Storage / Encryption

`Storage` handles all file I/O, key management, and ChaCha20-Poly1305 encryption for `.clin` files. See [ENCRYPTION.md](ENCRYPTION.md) for details.
