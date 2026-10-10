# Graph View (Graf)

Technical docs for the force-directed graph module — visualizes the note corpus as an interactive node graph based on `[[wikilinks]]` and tags.

---

## Overview

The graph view displays all notes as nodes with edges representing `[[wikilinks]]` connections between them. It uses upstream `graf`'s force-directed layout (`fdg_sim`) in a background thread for graphs up to 1,000 displayed nodes; larger graphs use a static clustered layout.

**Source:** `src/graf_adapter.rs` (`GrafPlugin`) integrates upstream [`graf`](https://github.com/reekta92/graf-rs), pinned by `Cargo.toml` / `Cargo.lock`. Graph construction, physics, rendering, spatial indexing, and viewport live in that crate. Clin owns note I/O, preview, quick search, config-error UI, statusline, and keybind mapping.

---

## Graph Construction

Clin creates `graf::NodeSpec` values from note summaries (ID, title, encryption state, tags, folder, links), then delegates construction to upstream `graf::graph::build_graph()`:

1. Exclude notes carrying a tag listed in `[graf.filter] exclude_tags`.
2. Resolve links against titles case-insensitively; self-links and unresolved links do not create edges.
3. With `show_orphan = false` (default), omit notes without a valid connection.
4. If `max_node` is positive and candidates exceed it, retain the most-connected candidates (ranked by link-list length). `0` removes this cap.
5. Build force nodes and edges among retained candidates.

Clin does not expose an `exclude_patterns` graph option. Vault listing rules and disabled file-view features still affect which notes reach the graph.

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

Upstream `graf::physics::start_physics()` owns simulation startup.

### Parameters

Clin exposes two options in `[graf.physics]` (`src/config/structs.rs`):

| Parameter | Default | Description |
|---|---|---|
| `ideal_distance` | 80.0 | Target distance between connected nodes |
| `tick_rate` | `"auto"` | `"auto"`: 16 ms steps up to 500 nodes, 33 ms for 501–1,000; `"fixed"`: 16 ms |

Other physics constants belong to the upstream library, not clin's config surface. More than 1,000 displayed nodes use static clustered placement even when `max_node = 0`.

### Thread Lifecycle

Graph is an `OverlayView` owned by `App` (see [ARCHITECTURE.md](ARCHITECTURE.md)):

```text
User enters Graph
  ├─ app.graph_plugin = Some(GrafPlugin::new(...))
  ├─ upstream physics shares Arc<RwLock<GraphState>>
  ├─ host calls overlay_render() / overlay_handle_event()
  └─ exit drops plugin, signals physics stop, restores return_mode
```

### Continuous vs Static Layout

Dynamic graphs continuously simulate with a minimum temperature; they do **not** sleep until a wake signal after an energy threshold. Empty graphs and graphs above the 1,000-node dynamic limit have no physics worker and are marked settled. Dragging and refresh update the appropriate dynamic/static layout.


---

## Rendering Pipeline

Upstream `graf::draw_graph_view()` uses ratatui's `Canvas`; `[graf.visual] canvas_marker` selects Braille (default), half-block, or dot markers. Clin renders the surrounding preview/search/status UI.

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
- **Size mode:** `fixed` (base 2.0) or `link_count`; `node_scale` sets screen-space scale (1–10, default 5) or `"automatic"`
- **Fill:** `dynamic` (default), `filled`, or `none`
- **Selection emphasis:** `grow` (default), `none`, `dim`, or `grow_dim`
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

- Controlled by `show_legend` (default `true`)
- Displays category/color information through the upstream renderer
- Legend position and item limits are not exposed by clin

### Render Cache

Render caches and spatial indexing belong to upstream `graf`; consult the pinned library implementation for their internal layout. Clin separately caches selected-note preview content.

---

## Interaction Model
### Preview Pane

The Graph view supports a preview pane identically to the List view. When enabled, it renders the contents of the currently selected node.

- **Positioning**: The preview pane uses the list preview position (`"left"` or `"right"`).
- **Toggling**: It can be toggled independently of the List view's preview state.


### Keyboard

| Key | Action |
|---|---|
| `Up`/`Down`/`Left`/`Right`, `k`/`j`/`h`/`l` | Pan viewport |
| `+` / `=` | Zoom in |
| `-` / `_` | Zoom out |
| `Enter` | Open selected node's note |
| `a` | Auto-fit view to all nodes |
| `/` | Toggle search popup |
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

Upstream `graf` owns the screen↔world transform, camera pan, zoom-to-cursor, and auto-fit. Viewport internals are library types, not a local clin module. Auto-fit contains the graph bounds with library-defined padding; the adapter uses upstream transforms for mouse hit-testing.

---

## Search

Clin's quick-search popup (`src/graf_adapter.rs`, `src/ui/quick_search.rs`) provides:

- Real-time filtering by node title
- Results limited by `max_results` / `max_visible`
- Keyboard navigation (up/down/enter)
- Selecting a node centers the viewport on it

---

## Configuration

All graf options are stored in the main `config.toml` under sections:

| Section | Purpose |
|---|---|
| `[graf]` | Display-node cap (`max_node`) and preview visibility |
| `[graf.visual]` | Node/edge style, scale/fill/selection, labels, minimap, legend, looking glass |
| `[graf.visual.colors]` | Per-color overrides (hex values) |
| `[graf.physics]` | Force simulation parameters |
| `[graf.interaction]` | Zoom factor and drag sensitivity |
| `[graf.filter]` | Node inclusion/exclusion rules |
| `[graf.search]` | Search popup behavior |
See [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) for full option documentation — that is the authoritative reference for all graf config sections and keys.

---

## Theme Palettes

Clin resolves palettes in `src/config/themes.rs` and `src/config/custom_themes.rs`. `clin_theme()` in `src/graf_adapter.rs` maps these colors and `AppThemeColors` into upstream `graf::ThemeColors`; `[graf.visual.colors]` overrides selected graph colors.

See [THEME_SYSTEM.md](THEME_SYSTEM.md) for details on themes and color derivation.

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, threading model
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — theme palettes used by the graph
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — all graph-related config options
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — `OpenGraphAction`
