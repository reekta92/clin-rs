# CHANGELOG

All notable changes comparing the previous stable release (`v0.5.2` from `clin-old`) to the current version (`v0.7.0`) of Clin.

---

## [0.7.0] - 2026-05-12

### Overview
This release constitutes a massive architectural overhaul and product expansion, maturing **Clin** from a basic encrypted note-taker into a highly extensible, feature-complete spatial and knowledge management tool. 

The primary focus of this upgrade includes monolithic struct decomposition for improved codebase reliability, alongside the introduction of high-impact graphical features: **Canvas** visualization and direct terminal **Drawing**.

---

### MAJOR FEATURES & BREAKTHROUGHS

#### Obsidian-Compatible Canvas (`pinstar`)
Introduced a robust **Canvas layout** providing a visual mapping experience compatible with standard Canvas JSON formats.
- **Node Types**: Integrated support for Text, Local File, Link, and Group container nodes.
- **Spatial Interactions**: Fully scalable infinite viewport with zoom levels, smooth drag-and-drop repositioning, and resizing handlers.
- **Dynamic Connections**: Interactive connection establishment between nodes with intelligent orthogonal routing.
- **Integrated Tools**: Built-in context menus, inline text editor overlays, and a raw raw JSON editing pane available for high-precision control.
- **Files**: Persistent state loaded and saved via `.canvas` extension files.

#### Terminal Drawing Engine (`draw`)
Empowered users with the capability to express non-textual data using a vector-based ASCII drawing system directly inside the terminal.
- **Tools Suite**: Select between dynamic Stroke drawing, Erasing, and explicit Shape drawing.
- **Primitive Support**: Renders Rectangles, Ellipses, Diamonds, Lines, and Arrows using customizable coordinates.
- **Infinite Canvas**: Decouples geometry from terminal boundaries through an independent viewport manipulation system.
- **Text Layering**: Add floating text elements over any diagram area.
- **Files**: Visualizations serialised into human-readable `.draw` JSON assets.

#### Upgraded Graph System (`graf`)
The foundational Graph view underwent an intense modularization drive and engineering polish.
- **Physics Performance**: Extracted physics calculations into asynchronous-capable isolation with tunable cooldown thresholds and overlap prevention strategies.
- **Navigation**: Introduction of interactive HUD enhancements including a navigational minimap, color legends, and explicit grid layers.
- **Color Modes**: Support logic to dynamically color nodes dynamically based on parent folder depth, tag associations, or linkage volumes.
- **Rendering**: Separation of concerns through the distinct `render.rs`, `input.rs`, and `viewport.rs` modules for ultra-responsive rendering ticks.

#### Comprehensive Code Architecture Decomposition
A foundational internal refactor shattered the monolithic `App` object (a 'God struct') into loosely-coupled stateful components. 
- **`NoteEditor` Extraction**: Encapsulated editing logic, cursor management, and undo operations.
- **`ListView` Encapsulation**: Moved sorting, filtering logic, and search cache management from the root core.
- **`PopupManager` Separation**: Centralized modal state logic avoiding state pollution in global navigation cycles.
- **Maintenance & Testing**: Highly improved testability and cognitive performance during development.

#### Advanced Theme & Cosmetic Subsystem
Dramatically enhanced visual accessibility through a global unified theming infrastructure.
- **Presets**: Out-of-the-box deployment of 11 premium preset themes (e.g., *Tokyo Night*, *Catppuccin Mocha*, *Nord*, *Everforest*).
- **Flexibility**: Seamless configuration toggles between Solid visual buffers and background Transparency.
- **Color-Overrides**: Deep YAML-backed customization support allows manual adjustment of accents, headings, and alert colors.

---

### FUNCTIONAL UPDATES & REFINEMENTS

#### Configuration Overhaul
- Consolidated disparate configuration schemas. The separate `graf.toml` legacy format is now structurally embedded within the main `config.toml`.
- Implemented **Automatic Migration**: Detects legacy `graf.toml` files on boot, merges configurations into the new hierarchical schema, and safely archives the old copy to `*.migrated`.

#### Storage & Content Intelligence
- **Wikilink Discovery**: Frontmatter parsing subsystem now parses `[[wikilinks]]` in real-time, establishing internal graph relations derived automatically from your content.
- **Safe Delete (Trash)**: Operations upgraded to move objects to a staging trash subsystem rather than executing destructive deletes immediately.
- **Multi-Format Routing**: Full extension awareness determining if logic routes to the markdown engine, canvas compiler, or draw engine.

#### Command Palette & Keybinds
- **Global Launcher**: Added `Ctrl+P` launcher housing rapid action triggers: encrypt, create canvas, flip theme, initiate drawing, or graph toggles.
- **Exhaustive Modifiers**: Refined cross-view context handling to reduce input collisions, improving simultaneous modifier usage responsiveness.

#### User Template Engine Upgrade
- Transitioned from code-defined templates to a dynamic filesystem configuration subsystem.
- Expanded support for smart injection variables: inserts `{date}`, `{time}`, and `{weekday}` at insertion time.
- Embedded example generator (`--create-example-templates`) provides canonical structures instantly.

#### Command Line Interface (CLI) Expansion
Major growth in non-interactive capabilities.
- `clin -q "content" [title]`: Create fire-and-forget rapid notes.
- `clin -n [--template <name>] [title]`: Programmatically build from specific template schema.
- Storage tools provided: `--set-storage-path`, `--reset-storage-path`, and specific system tools to `--migrate-storage`.

---

### BACKWARD COMPATIBILITY & UPGRADE GUIDE

#### Binary Rebranding
- **Change**: Binary naming structure transitioned from `clin-rs` to standard alias `clin`.
- **Impact**: User scripts, aliasing conventions, and symbolic environment paths relying on `clin-rs` execution will require trivial updating to reference `clin`.

#### Dependencies Upgrades
Important crate upgrades were merged impacting the terminal rendering runtime:
- `crossterm`: 0.28 $\rightarrow$ 0.29
- `ratatui`: 0.29 $\rightarrow$ 0.30
- `tui-textarea` $\rightarrow$ `ratatui-textarea` (0.9)

#### Configuration Auto-Migration
The upgrade path maintains perfect schema stability. On standard installation:
- Default config persistence directory continues executing consistently at `ProjectDirs::from("com", "clin", "clin")`. 
- Existing users will have no breakages, as data directory resolutions map identically.
- Automatic in-place legacy format conversion for the visual graph configurations triggers instantly without data-loss risks.

#### File Format Stability
- All existing `.md` (Markdown) and `.clin` (Encrypted) user vault contents are fully read/write compatible and backward reachable without translation operations required.
- New `.canvas` and `.draw` resources will not render appropriately if downgraded below v0.7.0, though they remain safely stored as standard parseable JSON payloads.
