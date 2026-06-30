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
             │    ├─ List / Edit / Help → dedicated render
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
┌─ Tab Bar (Notes · Editor · Graph · Draw · Canvas · Templates · About) ─┐
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
├── app.rs            — App struct, view transitions, note operations
├── app_theme.rs      — AppThemeColors derivation from ThemeConfig
├── cli.rs            — CliCommand enum
├── config.rs         — ClinConfig, ThemeConfig, graf config sections
├── constants.rs      — Hints, strings, layout constants
├── editor.rs         — NoteEditor (title + body TextArea), popup state
├── events.rs         — Keyboard/mouse event handlers per view
├── frontmatter.rs    — YAML frontmatter parse/serialize
├── keybinds.rs       — Keybind loading, Keybinds struct
├── list_view.rs      — ListView state, VisualItem, PreviewContent, sort
├── markdown.rs       — MarkdownRenderer (built-in comrak/syntect renderer)
├── migration.rs      — Storage migration logic
├── palette.rs        — CommandPalette popup widget
├── popups.rs         — ConfirmPopup, FolderPopup, TagPopup, etc.
├── sanitize.rs       — Terminal output sanitization
├── snapshot.rs       — Backup/restore snapshots
├── storage.rs        — Note CRUD, encryption, key management
├── templates/        — modular template system
│   ├── mod.rs        — public re-exports
│   ├── model.rs      — Template schema + render
│   ├── variables.rs  — variable substitution
│   ├── store.rs      — filename sanitization
│   └── manager.rs    — TemplateManager orchestration
├── ui.rs             — draw_ui(), draw_list_view(), draw_edit_view(), ...
│
├── actions/
│   ├── mod.rs        — Action trait, ACTIONS registry
│   ├── encrypt.rs    — EncryptNoteAction
│   ├── decrypt.rs    — DecryptNoteAction
│   ├── graph.rs      — OpenGraphAction
│   ├── draw.rs       — CreateDrawAction
│   ├── pinstar.rs    — CreateCanvasAction
│   ├── ocr.rs        — OcrPasteAction
│   └── theme.rs      — SwitchThemeAction
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
