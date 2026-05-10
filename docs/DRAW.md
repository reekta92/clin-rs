# Draw View

Technical docs for the draw canvas module — a simple paint-style canvas for freehand drawing, shapes, and text.

---

## Overview

The draw view provides an infinite 2D canvas for freehand drawing, inserting predefined shapes, and adding text. Files use the `.draw` extension and are stored alongside notes in the vault.

**Source:** `src/draw/` — modules: `app`, `input`, `render`, `state`

---

## File Format

Draw files are JSON with a `.draw` extension. Schema:

```json
{
  "version": 1,
  "width": 1000.0,
  "height": 1000.0,
  "background": null,
  "elements": [
    { "Stroke": { "points": [[0,0], [10,20], [30,50]], "color": [255, 0, 0] } },
    { "Shape": { "Rect": { "x": 100, "y": 50, "width": 200, "height": 100, "color": [0, 255, 0] } } },
    { "Shape": { "Ellipse": { "x": 300, "y": 150, "width": 80, "height": 60, "color": [0, 0, 255] } } },
    { "Shape": { "Diamond": { "x": 500, "y": 200, "width": 100, "height": 80, "color": [255, 255, 0] } } },
    { "Shape": { "Line": { "x1": 10, "y1": 10, "x2": 200, "y2": 200, "color": [128, 0, 128] } } },
    { "Shape": { "Arrow": { "x1": 50, "y1": 300, "x2": 250, "y2": 350, "color": [0, 128, 255] } } },
    { "Text": { "content": "Hello", "x": 400, "y": 400, "color": [255, 255, 255] } }
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

The custom deserializer handles old `CanvasData` format (pre-version field). If no `version` field is found in the JSON, data is loaded as `version: 0` with elements parsed from the old `nodes`/`edges` format.

---

## Tool Set

`DrawTool` enum selects the active tool:

```rust
pub enum DrawTool {
    Draw,   // Freehand drawing
    Erase,  // Click an element to erase it
    Text,   // Click to place, then inline editor
    Shape,  // Shape selector → click-drag-release to create
}
```

### Tool Details

| Tool | Behavior |
|---|---|
| **Draw** | Left-click-drag draws a freehand stroke. Points are recorded at mouse movement intervals and stored as `Stroke`. |
| **Erase** | Hover over an element and left-click to delete it. The element nearest to the click point within a threshold is removed. |
| **Text** | Click a location to place the text cursor. A floating `TextArea` editor opens for typing. Press `Esc` to close the editor. |
| **Shape** | Opens a shape selector popup (Rect, Ellipse, Diamond, Line, Arrow). Click-drag-release creates the shape. A preview element follows the cursor during drag. |

---

## Interaction Model

### Mouse

| Gesture | Action |
|---|---|
| Left-click-drag | Draw stroke / create shape / pan (middle button) |
| Left-click | Place text cursor / select element to erase |
| Middle-click-drag | Pan canvas |
| Scroll | Zoom in/out |
| Right-click | Edit existing text element |

### Keyboard

| Key | Action |
|---|---|
| `d` | Select Draw tool |
| `e` | Select Erase tool |
| `t` | Select Text tool |
| `s` | Select Shape tool (with selector popup) |
| `r`/`c`/`d`/`l`/`a` | Shape type shortcuts (Rect/Circle/Diamond/Line/Arrow) |
| `+` / `-` | Zoom in / out |
| `Esc` | Exit draw view |
| `Ctrl+S` | Save |
| Arrow keys | Pan canvas |

---

## Key Types

### `DrawAppState`

Main state struct for the draw view:

```rust
pub struct DrawAppState {
    pub data: DrawData,
    pub viewport: Viewport,
    pub storage: Storage,
    pub current_file: Option<String>,
    pub running: bool,
    pub active_tool: DrawTool,
    pub current_stroke: Option<Stroke>,
    pub last_area: Rect,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub text_editor: Option<(usize, TextArea<'static>)>,
    pub last_click: Option<(u16, u16, Instant)>,
    pub theme: AppThemeColors,
    pub active_shape_type: DrawShapeType,
    pub show_shape_selector: bool,
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<DrawElement>,
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
- Elements are drawn in order (Stroke → Shape → Text) with proper layering
- Current stroke preview renders during active drawing
- Shape preview renders during shape creation drag
- Text elements render with ratatui `Paragraph` overlays
- Viewport transform applies to all element positions

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

```
run_draw_view()
  ├─ DrawAppState::new(storage, note_id, theme)
  ├─ Terminal taken over
  ├─ Loop:
  │   ├─ draw_canvas() → frame.render_widget()
  │   ├─ poll event → handle_event() → modify state
  │   └─ auto-save on tool/zoom changes
  ├─ Save on exit
  └─ Return
```

---

## Connections

- [[ARCHITECTURE.md]] — event loop, rendering pipeline
- [[COMMAND_PALETTE.md]] — `CreateDrawAction`
- [[THEME_SYSTEM.md]] — colors for draw canvas rendering
