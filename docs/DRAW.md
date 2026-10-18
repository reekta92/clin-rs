# Draw View

Technical docs for the draw canvas module — a simple paint-style canvas for freehand drawing, shapes, and text.

---

## Overview

The draw view provides an infinite 2D canvas for freehand drawing, inserting predefined shapes, and adding text. Files use the `.draw` extension and are stored alongside notes in the vault.

**Engine:** [draw-rs](https://github.com/reekta92/draw-rs), consumed through the revision-pinned `draw` dependency. **Clin host:** `src/draw_adapter.rs` (`DrawPlugin`).

---

## File Format

Draw files are JSON with a `.draw` extension. Version 2 stores each element in a stable item wrapper:

```json
{
  "version": 2,
  "width": 1000.0,
  "height": 1000.0,
  "background": null,
  "elements": [
    {
      "id": "b7a2b8b7-9fd1-4bb9-b810-23d72e9e6a85",
      "element": { "Stroke": { "points": [[0, 0], [10, 20]], "color": [255, 0, 0] } },
      "transform": {
        "pivot_x": 5.0,
        "pivot_y": 10.0,
        "translate_x": 0.0,
        "translate_y": 0.0,
        "rotation_degrees": 0.0,
        "scale": 1.0
      }
    }
  ]
}
```

### Element Types

| Variant | Struct | Description |
|---|---|---|
| `Stroke` | `Stroke { points: Vec<(f64,f64)>, color: (u8,u8,u8) }` | Freehand vector of connected points |
| `Shape` | `Shape::Rect` | Rectangle (x, y, width, height) |
| `Shape` | `Shape::Ellipse` | Ellipse (x, y, width, height) |
| `Shape` | `Shape::Diamond` | Diamond (x, y, width, height) |
| `Shape` | `Shape::Line` | Line segment (x1, y1, x2, y2) |
| `Shape` | `Shape::Arrow` | Arrow (x1, y1, x2, y2) |
| `Text` | `Text { content, x, y, color }` | Text label at position |

### Backward Compatibility

Versions 0 and 1 migrate to v2 with fresh UUIDs and identity transforms. Saving migrated data writes v2. Legacy `Image` records are silently dropped. Future schema versions, duplicate IDs, and invalid transforms are rejected.

---

## Tool Set

`DrawTool` enum selects the active tool:

```rust
pub enum DrawTool {
    Cursor, // Select, move, rotate, scale, and open element actions
    Draw,   // Freehand drawing
    Erase,  // Click an element to erase it
    Text,   // Click to place, then inline editor
    Shape,  // Shape selector → click-drag-release to create
}
```

### Tool Details

| Tool | Behavior |
|---|---|
| **Cursor** | Selects one precise topmost item. Drag moves; Rotate or Scale mode shows matching handle; double-click text opens editor. |
| **Draw** | Left-click-drag draws a freehand stroke. Points are recorded at mouse movement intervals and stored as `Stroke`. |
| **Erase** | Click or drag across an element to remove it. |
| **Text** | Click a location to place text and open inline editor. |
| **Shape** | Opens a shape selector popup (Rect, Ellipse, Diamond, Line, Arrow). Click-drag-release creates a shape. |

---

## Interaction Model

### Mouse

| Gesture | Action |
|---|---|
| Left-click / drag | Current tool action; Cursor selects and moves items, or pans empty canvas on drag |
| Double-click text | Open text editor in Cursor mode |
| Right-click | Open item or empty-canvas action menu; drag past threshold pans |
| Middle-click-drag | Pan canvas |
| Scroll | Zoom in/out |

### Keyboard

| Key | Action |
|---|---|
| `a` | Select Cursor tool |
| `d`, `e`, `t`, `s` | Select Draw, Erase, Text, or Shape tool |
| Selected Cursor item shortcut | Runs matching context-menu action before tool shortcut |
| `c` / `v` | Copy selected item / enter paste placement |
| `Shift+G` | Toggle visual grid |
| `Ctrl+Z` | Undo latest committed draw change |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo latest draw change |
| `Esc` | Cancel highest-priority transient state, then exit Draw |

---

## Key Types

### `draw::DrawState`

Main state struct for the draw view:

```rust
pub struct DrawState {
    pub data: DrawData,
    pub viewport: Viewport,
    pub active_tool: DrawTool,
    pub selection: CanvasSelection<DrawItemId>,
    pub hovered: Option<DrawItemId>,
    pub interaction: Option<DrawInteraction>,
    pub grid: bool,
    // editor, history, menu, and interaction state; the save path is private
}
```

### `Viewport`

```rust
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
```

### `DrawTool`

```rust
pub enum DrawTool {
    Cursor,
    Draw,
    Erase,
    Text,
    Shape,
}
```

---

## Rendering

Rendering lives in upstream `draw::draw_view()` and `draw::draw_snapshot()`. Clin keeps its toolbar, icons, statusline and keybinding hints in the adapter:

- Uses ratatui's `Canvas` widget with Braille markers
- Adaptive dot grid remains private to the engine and does not appear in snapshots
- Vectors render in persisted order; text renders above vectors
- Item transforms apply translation, rotation, and scale around stored pivots
- Hover and selection redraw with blended color plus bounds and transform handles
- Current drawing, shape, paste, and transform previews remain transient

---

## Loading, saving, and lifecycle

`App` retains `Option<DrawPlugin>`. The adapter owns the document ID, clin keymap
and sequence matcher; `plugin.state` holds upstream data, history and gestures.
Opening an explicit ID validates its `.draw` extension and vault path, saves the
current owner, then loads the requested drawing before installing it. Reopening
the active drawing retains its history and viewport. A strip click opens the
previewed ID. A non-drawing selection opens the existing naming dialog.

`DrawState::load(path)` requires an existing regular UTF-8 file, resolves its
path, and enforces a 16 MiB limit. Invalid documents fail without replacement.
Creation in clin still uses `Storage::create_document()` for collision protection.
Loading legacy data never rewrites bytes; saving applies the supported migration.
Output is also bounded to 16 MiB, so successful saves can be reopened.

Completed edits, undo and redo save eagerly. Atomic replacement preserves
permissions, writes and syncs an exclusive temporary file, renames it, and syncs
the containing directory on Unix. Failed saves keep the live document and its
single recorded history entry; retry saving without repeating the edit. A failure
after rename may mean the new bytes were already published.

Ordinary quit, palette departure and navigation use the same interaction/save
guard. Unfinished gestures, changed fileless states and failed saves block
departure. Direct and queued overlay errors become temporary statuses and keep
Draw active. Concurrent external edits are not detected and may be overwritten.
Forced Ctrl+C/process termination remains outside these ordinary departure guards.

Clin resolves keys before mapping them exhaustively to semantic `DrawAction`s.
Text editors consume printable `q`; selectors, direct paste and context menus
retain their existing priority. Selected-item shortcuts precede tool bindings.
Clin transports system text clipboard data; the element clipboard remains owned
by `App` across drawing sessions. Theme and popup hints are mapped from the current
host settings at render time, preserving history and viewport across settings changes.
The host supplies the same canvas rectangle for rendering and input after resize.
List, strip and graph previews call the upstream committed-content painter.

## Standalone use and local development

The upstream repository also builds `draw <path.draw>`. Its parent directory must
exist. It uses default Draw bindings, local help and a textarea-local text
clipboard, without vault discovery or clin configuration. See the upstream README
for library embedding and terminal restoration behavior.

For paired development, temporarily replace the Git dependency with
`draw = { package = "draw-rs", path = "../draw-rs" }`. Restore the immutable Git
revision and regenerate `Cargo.lock` before committing the final integration.
The package is GPL-3.0-only and preserves clin source attribution.

## API migration

`clin::draw::state::*`, `clin::draw::geometry::*` and
`clin::draw::render::smooth_points` remain compatibility reexports.
The host-coupled `clin::draw::{app,input,render}` constructors and event/render
APIs are replaced by `clin::draw_adapter::DrawPlugin` and upstream `draw` APIs.
Use fallible `DrawPlugin::load(&storage, id, keybinds, matcher)`; there is no
infallible constructor that substitutes an empty drawing after a read error.
Access engine fields through `plugin.state`. Use `draw::DrawState::new()` only
for deliberate fileless embedding; changed fileless states cannot be saved.

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, rendering pipeline
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `CreateDrawAction`
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — colors for draw canvas rendering
