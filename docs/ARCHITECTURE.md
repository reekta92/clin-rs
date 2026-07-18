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
    ContentTree,  // Header-based note outline
    Setup,  // First-run setup wizard
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
Help  ──Esc───► List
List  ──palette──► Backup   (via command palette backup.open)
List  ──palette──► ContentTree  (via command palette content_tree.open)
List  ──palette──► Setup   (via setup.open)
Setup ──Esc───► List
Backup ──Esc───► List
ContentTree ──Esc───► List
```

Each overlay view implements the [`OverlayView`] trait (see `src/overlay.rs`). Graph, Draw, Canvas, Backup, and ContentTree all integrate into the main event loop via `overlay_render()` and `overlay_handle_event()` — they do not take control of the terminal. Their state is owned by `App` and instantiated on view transition.

---

## Event Loop Structure

### Main Loop (`main.rs` → `run_app()`)

```
main()
  └─ parse_cli_command() → CliCommand
      ├─ CliCommand::Run → Storage::init() → App::new() → run_tui_session() → run_app()
      ├─ CliCommand::Help → print help, exit
      ├─ CliCommand::QuickNote → save note, exit
      ├─ CliCommand::NewAndOpen → save note → run_tui_session()
      └─ CliCommand::*Config → config operations, exit
```

### TUI Session (`run_tui_session`)

```
run_tui_session(app)
  ├─ enable_raw_mode()
  ├─ EnterAlternateScreen + EnableMouseCapture + EnableBracketedPaste
  ├─ Terminal::new(backend)
  ├─ run_app(terminal, app)   ← main event loop
  ├─ cleanup: disable_raw_mode, LeaveAlternateScreen, ...
  └─ return Result
```

### Main Event Loop (`run_app()`)

```
while !should_quit:
  // 1. Status tick
  app.tick_status()

  // 2. Render
  terminal.draw(|frame| draw_ui(frame, app, focus))
        │
        └─ draw_ui():
             ├─ match app.mode:
             │    ├─ List / Edit / Help / Setup → dedicated render
             │    └─ Graph/Draw/Canvas/Backup/ContentTree
             │       → state.overlay_render(frame, area, theme, config, status)
             └─ popups, palette

  // 3. Poll async renderers (markdown preview)
  poll_renderers() → may trigger another draw

  // 4. Handle events
  poll event with timeout → match mode → per-view handler
    ├─ List  → handle_list_keys() / handle_list_mouse()
    ├─ Edit  → handle_edit_keys() / handle_edit_mouse()
    ├─ Help  → handle_help_keys() + tab switching
    ├─ Setup → handle_setup_keys() / handle_setup_mouse()
    └─ Graph/Draw/Canvas/Backup/ContentTree
       → state.overlay_handle_event(event, terminal, config)
          returns OverlayResult::{Continue, Exit, OpenHelp, NoteOpened, JumpToLine}
          └─ Exit → state = None; mode = return_mode (restored to previous view)
```

### Sub-view Overlays (OverlayView trait)

Five sub-views (Graph, Draw, Canvas, Backup, ContentTree) implement the [`OverlayView`] trait (see `src/overlay.rs`):

- [`overlay_render()`] — draws the overlay into a given screen area; called from `draw_ui()` during the main render pass
- [`overlay_handle_event()`] — handles one terminal event; returns [`OverlayResult`] indicating whether the overlay should stay active, exit, open help, or perform a view-specific action (open a note, jump to a line)

Their state is stored as `Option<X>` fields on `App` (e.g. `graph_state: Option<GrafAppState>`, `draw_state: Option<DrawAppState>`). When the user enters Graph/Draw/Canvas/Backup/ContentTree, the state is created and owned by `App`. On exit, the state is dropped (set to `None`) and the previous view is restored via `return_mode`.

No sub-view takes terminal ownership or runs a separate event loop.

---

## App State (`app.rs`)

`App` is the central state struct. It owns everything:
```
App
  ├── storage: Storage                    // file I/O, encryption, templates
  ├── keybinds: Keybinds                  // loaded from keybinds.toml
  ├── notes: Vec<NoteSummary>             // filtered/sorted note list
  ├── editor: NoteEditor                  // TextArea for title + body
  ├── list: ListView                      // selection, sort, filter, preview
  ├── mode: ViewMode                      // current active view
  ├── command_palette: Option<CommandPalette>  // Ctrl+P popup
  ├── popups: PopupManager                // confirm, folder, tag, template, theme popups
  ├── app_theme: AppThemeColors           // derived colors from theme config
  ├── return_mode: Option<ViewMode>       // where to return after overlay exit
  ├── graph_state: Option<GrafAppState>        // force-directed graph overlay
  ├── draw_state: Option<DrawAppState>         // freehand drawing overlay
  ├── canvas_state: Option<PinstarState>       // node/edge canvas overlay
  ├── backup_state: Option<BackupState>        // git backup dashboard overlay
  ├── content_tree_state: Option<ContentTreeState>  // header-based outline overlay
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
    └── Overlay state (graph_state, draw_state, canvas_state, backup_state, content_tree_state)
          └─ Owned by App as Option<X>. Created on view transition via mode change.
             Dropped (set to None) on overlay exit. No separate event loop.
```

---

## Rendering Pipeline

```
main.rs: terminal.draw(|frame| draw_ui(frame, app, focus))
  │
  └─ draw_ui()
       ├─ dark background block (if solid bg mode)
       └─ match app.mode:
            ├─ List  → draw_list_view()
            ├─ Edit  → draw_edit_view()
            ├─ Help  → draw_help_view()
            ├─ Graph → graf_state.overlay_render(frame, area, theme, config, status)
            ├─ Draw  → draw_state.overlay_render(frame, area, theme, config, status)
            ├─ Canvas→ canvas_state.overlay_render(frame, area, theme, config, status)
            ├─ Backup→ backup_state.overlay_render(frame, area, theme, config, status)
            └─ ContentTree → content_tree_state.overlay_render(frame, area, theme, config, status)
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
┌─ Tab Bar (Notes · Editor · Graph · Draw · Canvas · Backup · ContentTree · Setup · Templates · About) ─┐
│                                                              │
│                Help content (scrollable)                     │
│                                                              │
├─ Hint line ──────────────────────────────────────────────────┤
└──────────────────────────────────────────────────────────────┘
```

---

## Module Map

```
src/
├── main.rs           — CLI parsing, terminal setup, main event loop
├── app.rs            — App struct, ViewMode enum, top-level App API
├── app/              — App logic split by concern (see app.rs for public API)
│   ├── views.rs      — View transitions (open_graph_view, open_backup_view, etc.)
│   ├── notes.rs      — Note CRUD helpers
│   ├── folders.rs    — Folder operations
│   ├── popups.rs     — Popup creation helpers
│   ├── tags.rs       — Tag management
│   ├── trash.rs      — Trash operations
│   ├── search.rs     — Search
│   ├── status.rs     — Status bar tick
│   ├── settings_ops.rs — Settings operations
│   ├── import_ops.rs — Import operations
│   └── loading.rs    — Async loading state
├── app_theme.rs      — AppThemeColors derivation from ThemeConfig
├── cli.rs            — CliCommand enum
├── constants.rs      — Hints, strings, layout constants
├── editor.rs         — NoteEditor (title + body TextArea), popup state
├── frontmatter.rs    — YAML frontmatter parse/serialize
├── image_render/     — Native image rendering: LRU cache, background decode worker, protocol picker
├── config/           — Config structs, defaults, merging, custom themes
│   ├── mod.rs        — public re-exports, legacy-key compat shim
│   ├── structs.rs    — ClinConfig, sub-config structs
│   ├── types.rs      — Theme enum, enums + FromStr/Display
│   ├── merge.rs      — TOML value merging logic
│   ├── defaults.rs   — Default config values
│   ├── custom_themes.rs — Drop-in TOML theme loading
│   ├── path.rs       — Config path resolution
│   └── de.rs         — Custom deserialization helpers
├── list_view.rs      — ListView state, VisualItem, PreviewContent, sort
├── markdown/         — Markdown renderer (built-in comrak/syntect)
│   ├── mod.rs        — MarkdownRenderer public API
│   ├── render.rs     — comrak parse + syntect highlight rendering
│   └── theme.rs      — Syntect theme mapping
├── migration.rs      — Storage migration logic
├── palette.rs        — CommandPalette popup widget
├── popups.rs         — ConfirmPopup, FolderPopup, TagPopup, etc.
├── events/           — Keyboard/mouse event handlers per view
│   ├── mod.rs        — Shared popup/palette dispatcher
│   ├── list.rs       — handle_list_keys(), handle_list_mouse()
│   ├── edit.rs       — handle_edit_keys(), handle_edit_mouse()
│   ├── help.rs       — handle_help_keys()
│   └── setup.rs      — handle_setup_keys(), handle_setup_mouse()
├── snapshot.rs       — Backup/restore snapshots
├── storage.rs        — Note CRUD, encryption, key management
├── keybinds/         — Keybind loading, Keybinds struct, presets
│   ├── mod.rs        — Keybinds struct, KeybindsToml
│   ├── types.rs      — Action enums per scope (ListAction, SetupAction, etc.)
│   ├── defaults.rs   — Default bindings + preset overrides (Helix/Vim/Emacs)
│   ├── combo.rs      — KeyCombo helpers
│   ├── matcher.rs    — MatchOutcome enum, sequence matcher
│   └── api.rs        — keybind_scope! macro, resolve_* methods
│   ├── help_meta.rs  — Action metadata (group + description) driving the help keybind index
├── templates/        — modular template system
│   ├── mod.rs        — public re-exports
│   ├── model.rs      — Template schema + render
│   ├── variables.rs  — variable substitution
│   ├── store.rs      — filename sanitization
│   └── manager.rs    — TemplateManager orchestration
├── ui/               — UI rendering: draw_ui() and per-view renderers
│   ├── mod.rs        — draw_ui(), shared layout helpers
│   ├── list_view.rs  — draw_list_view()
│   ├── edit_view.rs  — draw_edit_view()
│   ├── help.rs       — draw_help_view()
│   ├── help_content.rs — Help tab descriptions, suggestion pools, popup accordion content
│   ├── popups.rs     — Popup/dialog drawers + format_keybind_hints
│   ├── title_bar.rs  — Title bar, tab bar rendering
│   └── setup.rs      — draw_setup_view(), setup_layout()
│
├── actions/
│   ├── mod.rs        — Action trait, ACTIONS registry, OpenGraphAction, OpenBackupAction,
│   │                    CreateDrawAction, CreateCanvasAction, SwitchThemeAction,
│   │                    OpenSetupWizardAction, SwitchKeybindPresetAction,
│   │                    ToggleExternalEditorAction, ToggleLayoutAction
│   ├── content_tree.rs — OpenContentTreeAction
│   ├── decrypt.rs    — DecryptNoteAction
│   ├── encrypt.rs    — EncryptNoteAction
│   ├── import.rs     — ImportAction (File/CSV/JSON/URL/Clipboard → New/Append)
│   ├── ocr.rs        — OcrPasteAction
│   └── settings.rs   — Toggle actions: LayoutEditMode, PreviewPane/Wrap, Calendar,
│                        LineNumbers, ConfirmDelete, PinnedOnTop, ConfirmQuit,
│                        PreviewEncryption, CycleSort, ShowHiddenFiles/AllFiles,
│                        SetWordGoal, FoldersFirst, SetNoteGoal, CycleIconMode, HintBarStyle
│
├── backup/           — Git backup dashboard
│   ├── app.rs        — BackupState, OverlayView implementation
│   ├── git_ops.rs    — GitOps (git2 safe wrappers)
│   ├── input.rs      — Keyboard/mouse event handling
│   ├── render.rs     — Dashboard rendering
│   ├── state.rs      — BackupState, BackupInputMode, BackupSection
│   └── worker.rs     — Background worker for auto-backup
│
├── graf/             — Graph view (force-directed)
│   ├── app.rs        — GrafAppState, OverlayView implementation
│   ├── graph.rs      — build_graph(), GraphNodeData, edge resolution
│   ├── input.rs      — Keyboard/mouse event handling
│   ├── physics.rs    — Force simulation thread (fdg_sim)
│   ├── render.rs     — draw_graph_view(), minimap, legend, grid
│   ├── state.rs      — GraphState, RenderCache, search state
│   ├── themes.rs     — theme_colors() palette definitions
│   ├── ui.rs         — search popup, node labels
│   ├── util.rs       — Math helpers, color conversion
│   └── viewport.rs   — Viewport (screen↔world transform)
│
├── draw/             — Draw view (freehand + shapes)
│   ├── app.rs        — DrawAppState, OverlayView implementation
│   ├── input.rs      — Mouse/keyboard handlers
│   ├── render.rs     — Canvas + element rendering
│   └── state.rs      — DrawAppState, DrawData, DrawElement
│
├── content_tree/      — Content Tree view (header outline)
│   ├── app.rs         — ContentTreeState, OverlayView implementation
│   ├── input.rs       — Keyboard/mouse handlers
│   ├── render.rs      — Tree + detail rendering
│   ├── state.rs       — ContentTreeState, tree model
│   └── parse.rs       — Header outline parser
└── pinstar/          — Canvas view (Obsidian-compatible)
    ├── app.rs        — PinstarState, OverlayView implementation
    ├── data.rs       — CanvasData, CanvasNode, CanvasEdge (JSON schema)
    ├── input.rs      — Keyboard/mouse handlers
    ├── render.rs     — Canvas + node/edge rendering
    └── state.rs      — PinstarState, PinstarContextMenu
```

---

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
│  - Iterates at configurable speed (thread_sleep_ms)  │
│  - Sets topology_dirty flag on new frame              │
│  - Terminates via Arc<AtomicBool>                    │
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
```

---

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

Actions are registered in a `Lazy<Vec<Box<dyn Action>>>` in `actions/mod.rs`. The command palette (`src/palette.rs`) provides a searchable popup. See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for details.

### Theme System

`ThemeConfig` from config.toml → `AppThemeColors` derived at load time. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for details.

### Storage / Encryption

`Storage` handles all file I/O, key management, and ChaCha20-Poly1305 encryption for `.clin` files. See [ENCRYPTION.md](ENCRYPTION.md) for details.
