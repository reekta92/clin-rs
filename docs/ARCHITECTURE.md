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
    Bases,   // Public-only .base queries and native authoring
    Outline,  // Header-based note outline
    Calendar, // Events, linked notes, iCalendar and CalDAV
    Setup,   // First-run setup wizard
}
```

Transition rules:

```
List  ──Enter──► Edit
List  ──Ctrl+G─► Graph
List  ──Enter──► Draw    (on .draw file)
List  ──Enter──► Canvas  (on .canvas file)
List  ──Enter──► Bases   (on .base file)
Bases ──Back───► previous view
Bases ──Enter──► public source editor / native image popup
Edit  ──Esc───► List
Graph ──Esc───► List
Draw  ──Esc───► List
Canvas──Esc───► List
List  ──?/F1──► Help
Help  ──Esc───► previous view
List  ──palette──► Outline  (via command palette outline.open)
List  ──palette──► Setup   (via setup_wizard)
List/Edit ──palette or date reference──► Calendar
Calendar ──open linked note──► Edit ──Esc──► Calendar
Calendar ──Esc──► List
Setup ──Esc───► List
Outline ──Esc───► List
```

Each overlay view implements the [`OverlayView`] trait (see `src/overlay.rs`). Graph, Draw, Canvas, Bases, Outline, and Calendar all integrate into the main event loop via `overlay_render()` and `overlay_handle_event()` — they do not take control of the terminal. Their state is owned by `App` and instantiated on view transition.

---

## Shared footer

`ui::chrome` owns the header/body/footer row geometry; the footer takes precedence on a one-row terminal. `ui::draw_ui` paints a fallback header and renders `ui::footer` after the active view and before global popups. View rendering uses `ui::view_content_area`; overlay renderers receive the rectangle above the footer. Views must not reserve or paint another application footer. Shared title-bar and statusline renderers apply `hint_bar_style`, including Calendar, Bases, and missing/loading view state. Header tabs publish their visible hit rectangles to the host, so rendering, clipping, hover and clicks agree. Setup uses the full terminal without application bars; editor Zen mode keeps its hidden header and visible footer. Graph's legacy status toggle cannot remove the application footer.

Adding a `ViewMode` requires an exhaustive hint choice in `ui::footer`; drawing the footer needs no new view-specific call. The acceptance test iterates every `ViewMode` and every hint style, so newly added variants join the check automatically. View-specific errors remain in content; Bases/Calendar footer details also carry query/sync status. Popup hints retain their own classic style.

## Calendar ownership

`calendar::view` owns the Calendar overlay and Notes mini-month; `calendar::ical` owns RFC 5545 parsing, recurrence, and schedule edits. `calendar::store` owns per-vault atomic event storage, stale-write checks, and note associations through guarded frontmatter writes. `calendar::dav` owns RFC 4791 discovery and conditional synchronization. A manual sync runs on a worker with a captured vault storage handle and a separate calendar lock; main/editor loops poll completion without blocking note editing. Calendar navigation reuses the shared save/departure and history paths. See [Calendar](CALENDAR.md) for protocol and recurrence limits.

## Event Loop Structure

### Main Loop (`lib.rs` → `run_app()`)

```
main()
  └─ clap Cli::from_arg_matches() → Cli
      ├─ no subcommand → session::bootstrap_app() → run_tui_session() → run_app()
      ├─ --help / --version → clap output, exit
      ├─ Command::Notes → list/new/open/cat/quick/search
      │    └─ new opens TUI unless --body, --stdin, --no-tui, or non-terminal stdin is supplied
      └─ Command::{Storage,Keybinds,Templates,Config,Cache} → operation, exit
```

### TUI Session (`run_tui_session`)

```
run_tui_session(app)
  ├─ session::start_session() → SessionGuard
  │    └─ signals, image worker, filesystem watcher
  ├─ TerminalGuard::enter() → raw mode, alternate screen, mouse, bracketed paste
  ├─ run_app(terminal, app)   ← main event loop
  ├─ TerminalGuard drop → terminal restoration
  ├─ session::finish_session() → recovery flush and watcher cleanup
  └─ return Result
```

### Main Event Loop (`run_app()`)

`run_app()` owns generic application work: catalog/search/watcher
drains, list and graph state, then generic rendering.

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

The editor session defers generic catalog/search queues. It polls Calendar,
imports, recovery, renderer/image results, and message/status expirations.
Visible live Bases regions additionally drain watcher changes and the shared
query worker without replacing unsaved editor text.

### Sub-view Overlays (OverlayView trait)

Six sub-views (Graph, Draw, Canvas, Bases, Outline, Calendar) implement the [`OverlayView`] trait (see `src/overlay.rs`):

- [`overlay_render()`] — draws the overlay into a given screen area; called from `draw_ui()` during the main render pass
- [`overlay_handle_event()`] — handles one terminal event; returns [`OverlayResult`] indicating whether the overlay should stay active, exit, open help, or perform a view-specific action (open a note, jump to a line)

Draw uses the independent [draw-rs](https://github.com/reekta92/draw-rs) engine through `DrawPlugin.state`; its model, geometry, history, gestures, private grid/menu helpers and snapshot painter live upstream. The adapter maps current themes and resolved semantic actions, validates vault IDs, and retains failed saves. Ordinary departure checks interactions and saves before dropping the owner.

Their state is stored as `Option<X>` fields on `App` (e.g. `graph_plugin: Option<GrafPlugin>`, `draw_state: Option<DrawPlugin>`, `canvas_state: Option<PinstarPlugin>`) . When the user enters Graph/Draw/Canvas/Bases/Outline/Calendar, the state is created and owned by `App`. Graph, Draw, Canvas, and Outline release their active owner on exit;
Calendar retains its view state. Return routes use `return_mode`, while Bases
keeps its own previous mode and embedded-host stack.

Bases stores its original return mode on `BasesState`, preserving it through
public-source handoff and reopening. No sub-view takes terminal ownership or
runs a separate event loop.

Live Bases regions reuse that renderer and input state through
`bases/embedding.rs`; source identity and lossless inline splices belong to
`bases/embed_source.rs`. One query worker serves the active overlay or bounded
visible regions, sharing immutable public metadata. File metadata uses `Arc`
with copy-on-write updates so host edits and watcher deltas do not duplicate
every file's properties. Metadata generations invalidate dependent queries;
cancelled generations cannot install results. Markdown caches store reserved
source geometry, while app-owned region state supplies live results and focus.
Canvas integration projects existing Pinstar nodes through the host adapter;
there is no second Canvas engine or new dependency fork. `serde_json`'s existing
`raw_value` feature identifies exact text-node string ranges for guarded edits.

---

## App State (`app.rs`)

`App` is the central state struct. It owns everything:
```
App
  ├── storage: Storage                    // file I/O, guarded saves, templates
  ├── keybinds: Keybinds                  // <config-dir>/keybinds/<preset>.toml
  ├── notes: Vec<NoteSummary>             // filtered/sorted note list
  ├── editor: NoteEditor                  // title TextArea + EditorDocument body
  ├── mode: ViewMode                      // current active view
  ├── command_palette: Option<CommandPalette>  // shared contextual palette; configurable Ctrl+Space/F6
  ├── popups: PopupManager                // confirm, folder, tag, template, theme popups
  ├── app_theme: AppThemeColors           // derived colors from theme config
  ├── return_mode: Option<ViewMode>       // where to return after overlay exit
  ├── graph_plugin: Option<GrafPlugin>         // clin adapter over upstream graf
  ├── draw_state: Option<DrawPlugin>         // freehand drawing overlay
  ├── canvas_state: Option<PinstarPlugin>      // clin adapter over upstream pinstar
  ├── bases_state: Option<BasesState>      // native public queries and authoring
  ├── calendar_view: Option<calendar::view::View> // retained calendar selection
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
    └── Overlay state (graph_plugin, draw_state, canvas_state, bases_state, outline_state, calendar_view)
          └─ Owned by App as Option<X>. Created on view transition via mode change.
             Released or retained according to the view lifecycle. No separate event loop.
```

---

## Rendering Pipeline

```
terminal.draw(|frame| ui::draw_ui(frame, app, focus))
  └─ draw_ui()
       ├─ theme background and fallback shared header
       ├─ List / Edit / Help / Setup → corresponding native renderer
       ├─ Graph / Draw / Canvas / Bases / Outline / Calendar
       │    └─ overlay_render(frame, view_area, app)
       ├─ live Bases host regions
       ├─ shared footer (except Setup)
       └─ active popups, palette, messages and global overlays
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
┌─ Tab Bar (Notes · Editor · Graph · Draw · Canvas · Templates · Bases · About) ─┐
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
├── calendar.rs           — Calendar module root and note-activity heatmap
├── calendar/             — Calendar view, event store, iCalendar, CalDAV
├── bases/                — Public queries, four layouts, authoring, live embeds
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
├── storage.rs            — Note persistence, subnotes, vault management
├── storage/              — Guarded documents, metadata, moves, drafts and recovery
├── frontmatter/          — Lossless properties/raw-YAML editing
├── todo.rs               — todo.txt parsing and rendering
├── app/                  — App logic: catalog, edit panes, folder preview, etc.
│   ├── catalog.rs        — Background note catalog worker
│   ├── edit_panes.rs     — Editor sidebar management
│   ├── folders.rs        — Folder tree, move, duplicate, pin
│   ├── navigation.rs     — Guarded view/document history
│   ├── preview_load.rs   — Bounded background preview source loading
│   ├── command_palette.rs — Palette context, opening and execution
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
│   ├── chrome.rs         — Shared header/body/footer geometry
│   ├── footer.rs         — Central application footer
│   ├── icons.rs          — Icon catalog and display-mode fallbacks
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
│   ├── structure.rs      — Shared Comrak options and Markdown metadata
│   ├── source_highlight.rs — Per-line source highlighter for editable Markdown
│   ├── style.rs          — RenderLine type, MarkdownTheme palette
│   ├── cache.rs          — Cached markdown output with revalidation
│   ├── todotxt.rs        — Render plugin for todo.txt items
│   ├── widget.rs         — Ratatui Widget impl for RenderLine slices
│   └── worker.rs         — Cancelable background render thread
├── templates.rs          — Template system: data model, substitution, persistence
├── image_render/         — Native image rendering
│   ├── cache.rs          — LRU image cache
│   ├── worker.rs         — Bounded decode and protocol-encoding worker
│   └── remote.rs         — Guarded public HTTP(S) image transport
├── graf_adapter.rs       — GrafPlugin: host preview/search/config/status, note I/O
├── pinstar_adapter.rs    — PinstarPlugin: host keybinds/status/clipboard/images
├── snapshot.rs           — Canvas and Draw preview snapshots
├── draw_adapter.rs       — DrawPlugin: host IDs/keybinds/status/clipboard over draw-rs
├── outline/              — Header outline view
│   ├── app.rs            — OutlineState, OverlayView implementation
│   ├── input.rs          — Keyboard/mouse handlers
│   ├── render.rs         — Tree + detail rendering
│   ├── state.rs          — OutlineState, tree model
│   └── parse.rs          — Header outline parser
```

---

Graph construction, physics, spatial indexing, viewport, and rendering live in [`graf`](https://github.com/reekta92/graf). Canvas schema, state, gestures, and rendering live in [`pinstar`](https://github.com/reekta92/pinstar). Drawing state, input, geometry, and rendering live in [`draw-rs`](https://github.com/reekta92/draw-rs). `Cargo.toml` declares dependency requirements and immutable Git revisions; `Cargo.lock` records exact packages. Clin adapters own host integration and guarded document saves. Outline remains local.

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

Other background work includes catalog enumeration, search, folder previews, image decoding, and recovery persistence. Worker results are installed by the host loop; Editor deliberately defers generic queues until it exits. Feature flags gate worker startup (image worker changes require restart).

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

Actions are registered in a `LazyLock<Vec<Box<dyn Action>>>` in `actions/mod.rs`. The command palette (`src/palette.rs`) provides one searchable popup across all ten views. `actions/context.rs` owns explicit view/target eligibility, used for listing and execution. `app/command_palette.rs` owns modal opening, binding-aware global shortcut availability, matcher cleanup, and shared activation. Both dispatchers route palette input before background copy/force-quit. Overlay palette requests are applied after `put_back`. `app/navigation.rs` tracks bounded session history at both dispatcher boundaries and prepares saved destinations before releasing owners. Command usage is shared by palette launches and direct bindings, persisted through the existing local state with coordinated updates.

Palette top-level navigation in `app/views.rs` guards writes, saves active/suspended editors and document owners, prepares destinations, then commits cleanup. Help/Setup retain their caller's existing return metadata; explicit Notes navigation terminates at List. Shared settings synchronize retained owner copies; Setup previews remain pending until Finish/Cancel. See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for details.

### Theme System

`UiConfig` from `[ui]` in config.toml → `AppThemeColors` derived at load time. See [THEME_SYSTEM.md](THEME_SYSTEM.md) for details.


## Persistence ownership

A loaded `Note` carries an immutable raw-source `DocumentSnapshot`. Ordinary note saves reuse guarded public document storage for baseline validation, read-only checks, permission-preserving atomic publication, and native write coordination. New notes publish without replacing an existing file. Ordinary saves keep filenames stable.

`storage::drafts::DraftWorker` coalesces latest recovery sources off the input thread. Save/session boundaries flush it before recovery cleanup. `storage::document_recovery` stores version-2 plaintext drafts and transaction journals under private Unix permissions; old encrypted journals remain separate and untouched.

There is no backup worker, Git dashboard, encryption action, or key lifecycle. Native terminal clipboard transport and shell stdin ingestion are documented with their limitations.

## Library ownership

`markdown::structure` owns shared Comrak options and Markdown metadata. Preview rendering, outlines, link extraction and heading/task metrics use that syntax; outlines retain Comrak source positions for editor jumps. The source highlighter remains incremental. The todo widget and both renderers share `markdown::todotxt`; the widget still selects pending tasks with a priority or creation date.

`which` owns executable discovery across platforms. `http` owns cancellable HTTP execution and bounded response reads using the existing reqwest/Tokio dependencies. URL import and automatic image loading retain separate redirect, destination and size policies. Optional Trafilatura runs on the single downloaded HTML response through the bounded converter runner; failures fall back to the general converters. See [URL import verification](COMMAND_PALETTE.md).

Body search uses literal lowercase-and-substring matching in Clin's search
worker; it does not invoke an external ripgrep process.
