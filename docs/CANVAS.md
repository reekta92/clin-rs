# Canvas View (Pinstar)

Technical docs for the pinstar canvas module — an Obsidian-compatible node/edge canvas with interactive TUI rendering.

---

## Overview

The canvas view provides an infinite 2D space for visual note mapping. Users can place text nodes, link to files/URLs, group nodes, and connect them with edges. Canvas files use the `.canvas` extension and follow Obsidian's canvas JSON schema for compatibility.

**Source:** `src/pinstar/` — modules: `app`, `data`, `input`, `render`, `state`

---

## File Format

Canvas files are JSON with a `.canvas` extension. Schema:

```json
{
  "nodes": [
    { "type": "text",   "id": "...", "x": 0, "y": 0, "width": 200, "height": 100, "text": "...",              "color": "#ff0000" },
    { "type": "file",   "id": "...", "x": 0, "y": 0, "width": 200, "height": 100, "file": "...",              "color": "#00ff00" },
    { "type": "link",   "id": "...", "x": 0, "y": 0, "width": 200, "height": 100, "url": "...",               "color": "#0000ff" },
    { "type": "group",  "id": "...", "x": 0, "y": 0, "width": 200, "height": 100, "label": "...", "color": "#888888" }
  ],
  "edges": [
    { "id": "...", "fromNode": "...", "fromSide": "right", "toNode": "...", "toSide": "left", "label": "...", "color": "#cccccc" }
  ]
}
```

### Node Types

| Type | Struct | Description |
|---|---|---|
| `text` | `TextNode` | Inline text displayed in a floating box |
| `file` | `FileNode` | References a note file by path; opens on activation |
| `link` | `LinkNode` | URL link; opens on activation |
| `group` | `GroupNode` | Container with label; visually groups child nodes |

All nodes share fields: `id` (UUID), `x`, `y`, `width`, `height`, `color` (optional hex/rgb/named).

### Edge Schema

`CanvasEdge` connects two nodes:

```rust
pub struct CanvasEdge {
    pub id: String,
    pub from_node: String,
    pub from_side: Option<String>,   // "top", "right", "bottom", "left"
    pub to_node: String,
    pub to_side: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
}
```

### Obsidian Compatibility

The `.canvas` format matches Obsidian's canvas JSON spec exactly. Files created by clin can be opened in Obsidian and vice versa. `CanvasNode` variants map 1:1 to Obsidian's node types.

---

## Interaction Model

```
┌─────────────────────────────────────────────────────────┐
│  Canvas (infinite, zoomable, pannable)                  │
│                                                         │
│  ┌──────────┐         ┌──────────┐                      │
│  │ Text A   │◄────────│ File B   │                      │
│  │ (selected)│  edge   │          │                      │
│  └──────────┘         └──────────┘                      │
│                                                         │
│              ┌──────────────┐                           │
│              │ Group        │                           │
│              │   ┌────────┐ │  ┌───────┐               │
│              │   │ Link C │ │  │ Text D│               │
│              │   └────────┘ │  └───────┘               │
│              └──────────────┘                           │
└─────────────────────────────────────────────────────────┘
```

### Mouse

| Action | Gesture |
|---|---|
| Select node | Left-click on node |
| Select group | Left-click on empty area, drag rectangle |
| Move node | Left-click-drag on node body |
| Pan canvas | Middle-click-drag (or Ctrl+left-drag) |
| Zoom | Scroll wheel |
| Resize node | Drag bottom-right corner handle |
| Context menu | Right-click on node or empty space |
| Edit text | Double-click text node |
| Create edge | Right-click node → "Create Connection" → click target node |

### Keyboard

| Key | Action |
|---|---|
| Arrow keys | Directional node selection |
| `+` / `-` | Zoom in / out |
| `a` | Open context menu (add node / edge) |
| `Enter` | Edit selected text node |
| `Delete` | Delete selected node / edge |
| `Esc` | Deselect / exit canvas |
| `Ctrl+S` | Save canvas |
| `Ctrl+G` | Toggle grid |
| `Tab` | Toggle between canvas view and raw JSON editor |
| `?` / `F1` | Open help |

---

## Key Types

### `PinstarState`

The main state struct for the canvas view:

```rust
pub struct PinstarState {
    pub path: PathBuf,
    pub data: CanvasData,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub zoom: f64,
    pub selected_node_id: Option<String>,
    pub floating_editor: Option<TextArea<'static>>,
    pub raw_editor: TextArea<'static>,
    pub editor_focus: bool,
    pub context_menu: Option<PinstarContextMenu>,
    pub connection_source_id: Option<String>,
    pub resizing_node_id: Option<String>,
    pub is_dragging_resize_handle: bool,
    pub deleting_connection_source_id: Option<String>,
    pub show_editor_pane: bool,
    pub drag_captured_nodes: HashSet<String>,
    pub show_grid: bool,
    pub mouse_selecting: bool,
    pub mouse_dragged: bool,
    pub help_requested: bool,
    // ...
}
```

### `PinstarContextMenu`

```rust
pub struct PinstarContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
    pub items: Vec<String>,
    pub menu_type: PinstarMenuType,  // Canvas, Editor, ColorPicker
}
```

---

## Data Flow

```
Storage::list_note_ids()
    └─ discovers *.canvas files
        └─ PinstarState::load(path) -> reads JSON
            └─ CanvasData deserialized from serde_json
                │
User interacts → PinstarState mutated
    │
PinstarState::save() -> writes JSON to disk
    └─ serde_json::to_string_pretty()
```

### View Lifecycle

```
run_pinstar_view()
  ├─ PinstarState::load(path)
  ├─ Terminal taken over
  ├─ Loop:
  │   ├─ render_canvas() + overlays
  │   ├─ poll event → handle_mouse() / handle_keys()
  │   └─ state mutations
  ├─ PinstarState::save() on exit
  └─ Return PinstarResult
```

---

## Rendering

Rendering happens in `src/pinstar/render.rs`. The canvas uses:

- **Braille markers** (`ratatui::symbols::braille`) for node shapes and edge lines
- **half-block** characters for dense rendering
- Viewport transform: screen coordinates = (world - viewport) * zoom
- Nodes are drawn as bordered rectangles with type-specific labels
- Edges are drawn as lines between node centers at specified sides
- Grid is drawn when `show_grid` is enabled
- Context menu renders as a popup at cursor position
- Floating text editor renders within the selected text node area
- Raw JSON editor is available via `Tab` toggle — full TextArea with entire canvas JSON

---

## Connection with Other Systems

- [ARCHITECTURE.md](ARCHITECTURE.md) — overall state machine and event loop
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `CreateCanvasAction` (Ctrl+P → New Canvas)
- [ARCHITECTURE.md](ARCHITECTURE.md) — data flow and storage overview
- [ENCRYPTION.md](ENCRYPTION.md) — canvas files are not encrypted; only `.clin` notes are
