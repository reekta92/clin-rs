# Graph View (Graf)

Technical docs for the force-directed graph module — visualizes the note corpus as an interactive node graph based on `[[wikilinks]]` and tags.

---

## Overview

The graph view displays all notes as nodes with edges representing `[[wikilinks]]` connections between them. It uses a force-directed layout simulation (`fdg_sim` crate) that runs in a background thread, settling into a stable configuration.

**Source:** `src/graf/` — modules: `app`, `graph`, `input`, `physics`, `render`, `state`, `themes`, `ui`, `util`, `viewport`

---

## Graph Construction

`build_graph()` in `src/graf/graph.rs` is the entry point. It:

1. Lists all note IDs from `Storage`
2. Loads each note summary (title, tags, folder, links)
3. Filters out nodes that don't meet criteria:
   - `min_links` — minimum link threshold
   - `exclude_tags` — skip notes with these tags
   - `exclude_patterns` — skip notes matching path patterns
   - `max_nodes` — cap total nodes
4. Creates force nodes with `GraphNodeData`
5. Creates edges from `[[wikilinks]]` extracted by `extract_wikilinks()`
6. Resolves links via title matching (case-insensitive)

```rust
pub struct GraphNodeData {
    pub note_id: String,
    pub title: String,
    pub is_encrypted: bool,
    pub tags: Vec<String>,
    pub link_count: usize,
    pub folder: String,
}
```

---

## Physics Simulation

Runs in a dedicated background thread (`start_physics()` in `src/graf/physics.rs`).

### Parameters

Only `ideal_distance` is a user-configurable option (in `[graf.physics]`, `PhysicsConfig` in `src/config/structs.rs`):

| Parameter | Default | Description |
|---|---|---|
| `ideal_distance` | 80.0 | Target distance between connected nodes |

All other simulation constants (`damping`, `max_iterations`, `gravity`, `cooling`, `timestep`, `thread_sleep_ms`, `prevent_overlapping`) are internal to `src/graf/physics.rs` and not exposed as config options.

### Thread Lifecycle

Graph is an `OverlayView` owned by `App` (see [ARCHITECTURE.md](ARCHITECTURE.md)). The physics thread is spawned on view entry and joined on exit:

```
User enters Graph view
  ├─ graf_state = Some(GrafAppState::new(...))
  ├─ start_physics() spawns background thread
  │     └─ Arc<AtomicBool> kill signal + channel
  │     └─ Loop:
  │         ├─ Check kill signal
  │         ├─ simulation.update(timestep)
  │         ├─ Apply gravity, drag targets
  │         ├─ Set is_settled if energy < threshold
  │         └─ Compute bounds, update render cache
  ├─ Main loop calls graf_state.overlay_render() each frame
  │     └─ Polls shared GraphState for positions
  └─ On overlay exit (OverlayResult::Exit):
       ├─ Send kill signal → join physics thread
       └─ graf_state = None; mode = return_mode

### Settling

The simulation is considered "settled" when total kinetic energy drops below `0.05 × node_count`. Once settled, the physics thread sleeps until a wake signal (e.g., drag, auto-fit, config reload).


---

## Rendering Pipeline

`draw_graph_view()` in `src/graf/render.rs`. Uses ratatui's `Canvas` widget with Braille markers for high-density node rendering.

### Layers

```
1. Background grid  ── optional, configurable divisions
2. Edges  ────────── colored lines between nodes
3. Nodes  ────────── shapes (circle, square, diamond)
4. Selection ring  ─ halo around selected node
5. Labels  ──────── title text (mode-controlled)
6. Minimap  ─────── small overview in corner
7. Legend  ───────── sorted by link count
8. Status bar  ───── file/link counts, position
```

### Node Rendering

- **Shape:** `circle` (default), `square`, or `diamond` — set via `node_shape`
- **Size:** `fixed` (default 2.0) or `link_count` (scaled by connections)
- **Color modes:** `folder` (by folder), `tag` (by first tag), `link_count` (heatmap), `uniform`
- **Labels:** controlled by `label_mode` — `selected`, `neighbors`, `all`, `none`

### Edge Rendering

- **Thickness:** 1–3 (configurable)
- **Color modes:** `source`, `target`, `uniform`
- Drawn as `Line` shapes via ratatui's `Painter`

### Minimap

- **Markers:** `half_block` (default) or `braille`/`dot`
- **Position:** `top_right` (default), `top_left`, `bottom_right`, `bottom_left`
- Shows a bird's-eye view of the entire graph with a viewport rectangle

### Legend

- **Position:** `bottom_right` (default) or other corners
- **Max items:** configurable (default 10)
- Shows nodes sorted by link count with color swatches

### Render Cache

`RenderCache` stores pre-computed edge data, node data, label data, legend data, and grid data. It's invalidated on topology changes via a `topology_dirty` flag to avoid redundant computation.

---

## Interaction Model
### Preview Pane

The Graph view supports a preview pane identically to the List view. When enabled, it renders the contents of the currently selected node.
+ **Positioning**: The preview pane respects the `list.preview_position` setting (right, bottom, etc.).
+ **Toggling**: Can be toggled on/off independently of the List view's preview state.


### Keyboard

| Key | Action |
|---|---|
| `Up`/`Down`/`Left`/`Right` | Directional node selection |
| `+` / `Ctrl+J` | Zoom in |
| `-` / `Ctrl+K` | Zoom out |
| `Enter` | Open selected node's note |
| `a` | Auto-fit view to all nodes |
| `f` | Toggle search popup |
| `Shift+M` | Toggle minimap |
| `Shift+L` | Toggle legend |
| `Shift+G` | Toggle grid |
| `Shift+P` | Toggle preview pane |
| `Shift+S` | Toggle status bar |
| `r` | Refresh simulation |
| `Ctrl+R` | Reload config |
| `?` / `F1` | Help |

### Mouse

| Gesture | Action |
|---|---|
| Left-click | Select node |
| Left-click-drag | Drag node (interrupts settling) |
| Scroll | Zoom in/out |
| Middle-click-drag | Pan viewport |
| Hover | Highlight connections (implementation varies) |

---

## Viewport

`Viewport` struct in `src/graf/viewport.rs` handles the screen↔world coordinate transform:

```rust
pub struct Viewport {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
}
```

- **World → Screen:** `screen = (world - offset) * zoom`
- **Screen → World:** `world = screen / zoom + offset`
- **Auto-fit:** Sets offset and zoom to contain all nodes within the terminal area with padding (`auto_fit_padding`)

---

## Search

The search popup (`src/graf/ui.rs`) provides:

- Real-time filtering by node title
- Results limited by `max_results` / `max_visible`
- Keyboard navigation (up/down/enter)
- Selecting a node centers the viewport on it

---

## Configuration

All graf options are stored in the main `config.toml` under sections:

| Section | Purpose |
|---|---|
| `[graf]` | Global graph settings: preview_enabled |
| `[graf.visual]` | Colors, node/edge style, labels, minimap, legend, grid |
| `[graf.visual.colors]` | Per-color overrides (hex values) |
| `[graf.physics]` | Force simulation parameters |
| `[graf.interaction]` | Zoom, drag, double-click settings |
| `[graf.filter]` | Node inclusion/exclusion rules |
| `[graf.search]` | Search popup behavior |
See [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) for full option documentation — that is the authoritative reference for all graf config sections and keys.

---

## Theme Palettes

Theme color palettes are defined in `src/graf/themes.rs`. Each theme provides ~10 color values:

```rust
pub fn theme_colors(theme: &Theme) -> HashMap<String, String> {
    match theme {
        Theme::TokyoNight => tokyo_night(),
        Theme::CatppuccinMocha => catppuccin_mocha(),
        // ...
    }
}
```

See [THEME_SYSTEM.md](THEME_SYSTEM.md) for details on themes and color derivation.

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, threading model
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — theme palettes used by the graph
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — all graph-related config options
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `OpenGraphAction`
