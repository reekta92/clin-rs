# Draw View

Technical docs for the draw canvas module — a simple paint-style canvas for freehand drawing, shapes, and text.

---

## Overview

The draw view provides an infinite 2D canvas for freehand drawing, inserting predefined shapes, and adding text. Files use the `.draw` extension and are stored alongside notes in the vault.

**Source:** `src/draw/` — modules: `app`, `input`, `render`, `state`

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
| **Cursor** | Selects one precise topmost item. Drag moves; visible handles rotate or scale; double-click text opens editor. |
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
| `v` | Select Cursor tool |
| `d`, `e`, `t`, `s` | Select Draw, Erase, Text, or Shape tool |
| Selected Cursor item shortcut | Runs matching context-menu action before tool shortcut |
| `Shift+G` | Toggle visual grid |
| `Ctrl+Shift+C` / `Ctrl+Shift+V` | Copy selected item / enter paste placement |
| `Ctrl+Z` | Undo latest committed draw change |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo latest draw change |
| `Esc` | Cancel highest-priority transient state, then exit Draw |

---

## Key Types

### `DrawAppState`

Main state struct for the draw view:

```rust
pub struct DrawAppState {
    pub data: DrawData,
    pub viewport: Viewport,
    pub active_tool: DrawTool,
    pub selection: CanvasSelection<DrawItemId>,
    pub hovered: Option<DrawItemId>,
    pub interaction: Option<DrawInteraction>,
    pub grid: CanvasGridState,
    // storage, editor, history, menu, and input state
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

Rendering in `src/draw/render.rs`:

- Uses ratatui's `Canvas` widget with Braille markers
- Shared adaptive dot grid renders before content and does not appear in snapshots
- Vectors render in persisted order; text renders above vectors
- Item transforms apply translation, rotation, and scale around stored pivots
- Hover and selection redraw with blended color plus bounds and transform handles
- Current drawing, shape, paste, and transform previews remain transient

---

## Data Flow

```
Storage::list_note_ids()
  └─ discovers *.draw files
      └─ DrawAppState::new(storage, file_id)
          └─ reads JSON from note_path(id)
              └─ serde_json::from_str() → DrawData
                  │
User interacts  →  DrawAppState mutated
  │
DrawAppState::save_draw()
  └─ serde_json::to_string() → writes to note_path(id)
```

### View Lifecycle

Draw is an `OverlayView` owned by `App` (see [ARCHITECTURE.md](ARCHITECTURE.md)):

```
User enters Draw view
  ├─ State created: DrawAppState::new(storage, note_id, theme)
  ├─ Owned by App as Option<DrawAppState>
  ├─ overlay_render() called from draw_ui() each frame
  ├─ Events dispatched to overlay_handle_event()
  │     └─ returns OverlayResult::{Continue, Exit}
  └─ On Exit:
       ├─ DrawAppState::save_draw() writes changes to disk
       └─ state = None; mode = return_mode
```

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, rendering pipeline
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `CreateDrawAction`
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — colors for draw canvas rendering
