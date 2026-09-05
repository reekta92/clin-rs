//! clin-side adapter for the upstream `graf` library.
//!
//! Owns host state the lib deliberately doesn't: preview pane, quick-search
//! popup, config-error screen, reload notification, note opening via
//! `Storage`, and the status bar (lib `draw_graph_view` omits it). Clin
//! keybinds resolve actions here ("clin keybinds win") and drive the lib via
//! `graf::apply_action` / `graf::handle_graph_mouse`.

use std::sync::Arc;

use parking_lot::RwLock;

use crossterm::event::KeyCode;

use graf::{
    FeatureFlags, GraphAction as LibAction, GraphState, MenuItem as LibMenuItem, ModeBanner,
    NodeSpec, Settings as GrafSettings, ThemeColors as GrafThemeColors,
};
use graf::{apply_action, draw_graph_view, handle_graph_mouse};

use fdg_sim::petgraph::graph::NodeIndex;

use crate::config::ClinConfig;
use crate::keybinds::{GraphAction, Keybinds};
use crate::list_view::PreviewContent;
use crate::markdown::MarkdownRenderer;
use crate::storage::Storage;

// ── Config / theme / spec mapping ───────────────────────────────────────────

fn map_background(b: &crate::config::Background) -> graf::Background {
    match b {
        crate::config::Background::Transparent => graf::Background::Transparent,
        crate::config::Background::Solid => graf::Background::Solid,
    }
}

fn map_node_color_mode(m: &crate::config::NodeColorMode) -> graf::NodeColorMode {
    match m {
        crate::config::NodeColorMode::Tag => graf::NodeColorMode::Tag,
        crate::config::NodeColorMode::Folder => graf::NodeColorMode::Folder,
        crate::config::NodeColorMode::LinkCount => graf::NodeColorMode::LinkCount,
        crate::config::NodeColorMode::Uniform => graf::NodeColorMode::Uniform,
    }
}

fn map_edge_color_mode(m: &crate::config::EdgeColorMode) -> graf::EdgeColorMode {
    match m {
        crate::config::EdgeColorMode::Source => graf::EdgeColorMode::Source,
        crate::config::EdgeColorMode::Target => graf::EdgeColorMode::Target,
        crate::config::EdgeColorMode::Uniform => graf::EdgeColorMode::Uniform,
    }
}

fn map_label_mode(m: &crate::config::LabelMode) -> graf::LabelMode {
    match m {
        crate::config::LabelMode::Selected => graf::LabelMode::Selected,
        crate::config::LabelMode::Neighbors => graf::LabelMode::Neighbors,
        crate::config::LabelMode::All => graf::LabelMode::All,
        crate::config::LabelMode::None => graf::LabelMode::None,
    }
}

fn map_node_size_mode(m: &crate::config::NodeSizeMode) -> graf::NodeSizeMode {
    match m {
        crate::config::NodeSizeMode::Fixed => graf::NodeSizeMode::Fixed,
        crate::config::NodeSizeMode::LinkCount => graf::NodeSizeMode::LinkCount,
    }
}

fn map_canvas_marker(m: &crate::config::CanvasMarker) -> graf::CanvasMarker {
    match m {
        crate::config::CanvasMarker::Braille => graf::CanvasMarker::Braille,
        crate::config::CanvasMarker::HalfBlock => graf::CanvasMarker::HalfBlock,
        crate::config::CanvasMarker::Dot => graf::CanvasMarker::Dot,
    }
}

fn map_node_shape(s: &crate::config::NodeShape) -> graf::NodeShape {
    match s {
        crate::config::NodeShape::Circle => graf::NodeShape::Circle,
        crate::config::NodeShape::Square => graf::NodeShape::Square,
        crate::config::NodeShape::Diamond => graf::NodeShape::Diamond,
    }
}

fn map_legend_position(p: &crate::config::LegendPosition) -> graf::LegendPosition {
    match p {
        crate::config::LegendPosition::TopRight => graf::LegendPosition::TopRight,
        crate::config::LegendPosition::TopLeft => graf::LegendPosition::TopLeft,
        crate::config::LegendPosition::BottomRight => graf::LegendPosition::BottomRight,
        crate::config::LegendPosition::BottomLeft => graf::LegendPosition::BottomLeft,
    }
}

fn map_tick_rate(r: &crate::config::PhysicsTickRate) -> graf::PhysicsTickRate {
    match r {
        crate::config::PhysicsTickRate::Auto => graf::PhysicsTickRate::Auto,
        crate::config::PhysicsTickRate::Fixed => graf::PhysicsTickRate::Fixed,
    }
}

/// Convert `ClinConfig` into lib `Settings`. Field-by-field with exhaustive
/// enum matches so a future upstream variant fails to compile here.
pub fn clin_settings(config: &ClinConfig) -> GrafSettings {
    let g = &config.graf;
    let mut settings = GrafSettings::default();
    settings.visual = graf::settings::VisualConfig {
        background: map_background(&g.visual.graph_background),
        theme: graf::settings::Theme::default(),
        node_color_mode: map_node_color_mode(&g.visual.node_color_mode),
        edge_color_mode: map_edge_color_mode(&g.visual.edge_color_mode),
        label_mode: map_label_mode(&g.visual.label_mode),
        label_max_length: g.visual.label_max_length,
        node_size: g.visual.node_size,
        node_size_mode: map_node_size_mode(&g.visual.node_size_mode),
        edge_thickness: g.visual.edge_thickness,
        show_legend: g.visual.show_legend,
        show_minimap: g.visual.show_minimap,
        minimap_position: map_legend_position(&g.visual.minimap_position),
        minimap_width: g.visual.minimap_width,
        minimap_height: g.visual.minimap_height,
        canvas_marker: map_canvas_marker(&g.visual.canvas_marker),
        minimap_marker: graf::settings::CanvasMarker::default(),
        node_shape: map_node_shape(&g.visual.node_shape),
        label_offset: g.visual.label_offset,
        show_looking_glass: g.visual.show_looking_glass,
        looking_glass_width: g.visual.looking_glass_width,
        looking_glass_height: g.visual.looking_glass_height,
        show_grid: true, // clin has no config knob; historical default
        colors: graf::settings::ColorOverrides {
            node_color: g.visual.colors.node_color,
            edge_color: g.visual.colors.edge_color,
            label_color: g.visual.colors.label_color,
            selection_ring_color: g.visual.colors.selection_ring_color,
            border_color: g.visual.colors.border_color,
            title_color: None,
            grid_color: None,
            legend_text_color: None,
            status_bar_color: None,
            background_color: g.visual.colors.background_color,
        },
        grid_divisions: graf::settings::VisualConfig::default().grid_divisions,
    };
    settings.physics = graf::settings::PhysicsConfig {
        ideal_distance: g.physics.ideal_distance,
        tick_rate: map_tick_rate(&g.physics.tick_rate),
        ..Default::default()
    };
    settings.interaction = graf::settings::InteractionConfig {
        zoom_factor: g.interaction.zoom_factor,
        drag_sensitivity: g.interaction.drag_sensitivity,
        ..Default::default()
    };
    settings.filter = graf::settings::FilterConfig {
        exclude_tags: g.filter.exclude_tags.clone(),
        show_orphan: g.filter.show_orphan,
        ..Default::default()
    };
    settings.search = graf::settings::SearchConfig {
        max_results: g.search.max_results,
        max_visible: g.search.max_visible,
        ..Default::default()
    };
    settings.display = graf::settings::DisplayConfig {
        show_status_bar: config.ui.show_status_bar,
        ..Default::default()
    };
    settings.preview_enabled = g.preview_enabled;
    settings.max_node = g.max_node;
    settings
}

/// Convert clin's 9-field graph `ThemeColors` into the lib's 14-field one.
pub fn clin_theme(config: &ClinConfig) -> GrafThemeColors {
    let c = config.theme_colors();
    // Base = upstream default palette; the 9 shared fields are overridden
    // from clin's own theme resolution below.
    let mut t = GrafThemeColors::resolve(&GrafSettings::default());
    t.node_colors = c.node_colors;
    t.edge_color = c.edge_color;
    t.border_color = c.border_color;
    t.label_color = c.label_color;
    t.selected_indicator_color = c.selected_indicator_color;
    t.background_color = c.background_color;
    t.minimap_border_color = c.minimap_border_color;
    t.minimap_viewport_color = c.minimap_viewport_color;
    t.minimap_bg_color = c.minimap_bg_color;
    t
}

pub fn note_specs(summaries: &[crate::storage::NoteSummary]) -> Vec<NodeSpec> {
    summaries
        .iter()
        .map(|n| NodeSpec {
            id: n.id.clone(),
            title: n.title.clone(),
            tags: n.tags.clone(),
            folder: n.folder.clone(),
            links: n.links.clone(),
        })
        .collect()
}

// ── Plugin state ────────────────────────────────────────────────────────────

#[derive(Default)]
struct CanvasFpsSampler {
    window_started_at: Option<std::time::Instant>,
    frames_in_window: u32,
    published_fps: Option<f64>,
}

impl CanvasFpsSampler {
    fn record_frame(&mut self, now: std::time::Instant) {
        match self.window_started_at {
            None => {
                self.window_started_at = Some(now);
                self.frames_in_window = 0;
            }
            Some(start) => {
                self.frames_in_window += 1;
                let elapsed = now.duration_since(start);
                if elapsed >= std::time::Duration::from_millis(500) {
                    let elapsed_secs = elapsed.as_secs_f64();
                    if elapsed_secs > 0.0 {
                        self.published_fps = Some(self.frames_in_window as f64 / elapsed_secs);
                    }
                    self.window_started_at = Some(now);
                    self.frames_in_window = 0;
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PreviewRequestKey {
    note_id: String,
    width: u16,
    height: u16,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
}

pub struct GrafPlugin {
    pub graph_state: Option<Arc<RwLock<GraphState>>>,
    pub graph_kill_tx: Option<std::sync::mpsc::Sender<()>>,
    pub graph_mouse_state: graf::GraphMouseState,
    pub storage: Storage,
    pub notes: Vec<crate::storage::NoteSummary>,
    pub focus_note_ids: Option<std::collections::HashSet<String>>,
    pub config_errors: Vec<String>,
    pub search_popup: Option<crate::ui::quick_search::QuickSearch<(NodeIndex, String)>>,
    pub show_minimap: bool,
    pub show_legend: bool,
    pub grid: bool,
    pub show_status_bar: bool,
    pub show_looking_glass: bool,
    pub config_reload_msg: Option<String>,

    pub preview_enabled: bool,
    pub preview_content: Option<PreviewContent>,
    pub preview_note_id: Option<String>,
    pub last_preview_pane_width: u16,
    pub last_preview_pane_height: u16,
    pub preview_scale: f64,
    preview_request_key: Option<PreviewRequestKey>,
    pub pending_markdown_resize: Option<(u16, std::time::Instant)>,
    pub app_theme: crate::app_theme::AppThemeColors,
    pub preview_offset_x: f64,
    pub preview_offset_y: f64,
    pub keybinds: Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub mouse_pos: Option<(u16, u16)>,
    pub preview_drag_last_pos: Option<(u16, u16)>,
    canvas_fps_sampler: CanvasFpsSampler,
}

impl Drop for GrafPlugin {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl GrafPlugin {
    pub fn new(
        config: &ClinConfig,
        storage: Storage,
        summaries: Vec<crate::storage::NoteSummary>,
        config_errors: Vec<String>,
        keybinds: Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> anyhow::Result<Self> {
        let settings = clin_settings(config);
        let graph_state = GraphState::from_specs(&note_specs(&summaries), &settings)?;
        let state = Arc::new(RwLock::new(graph_state));
        let graph_kill_tx = graf::start_physics(state.clone(), &settings);

        Ok(Self {
            graph_state: Some(state),
            graph_kill_tx,
            graph_mouse_state: graf::GraphMouseState::default(),
            storage,
            notes: summaries,
            focus_note_ids: None,

            config_errors,
            search_popup: None,
            show_minimap: config.graf.visual.show_minimap,
            show_legend: config.graf.visual.show_legend,
            grid: true,
            show_status_bar: config.ui.show_status_bar,
            show_looking_glass: config.graf.visual.show_looking_glass,
            config_reload_msg: None,

            preview_enabled: config.graf.preview_enabled,
            preview_content: None,
            preview_note_id: None,
            last_preview_pane_width: 0,
            last_preview_pane_height: 0,
            preview_scale: 1.0,
            preview_offset_x: 0.0,
            preview_offset_y: 0.0,
            preview_request_key: None,
            pending_markdown_resize: None,
            app_theme: crate::app_theme::AppThemeColors::from_config(&config.ui, &mut Vec::new()),
            keybinds,
            seq_matcher,
            preview_drag_last_pos: None,
            mouse_pos: None,
            canvas_fps_sampler: CanvasFpsSampler::default(),
        })
    }

    pub fn canvas_fps(&self) -> Option<f64> {
        self.canvas_fps_sampler.published_fps
    }

    pub fn record_frame(&mut self, now: std::time::Instant) {
        self.canvas_fps_sampler.record_frame(now);
    }

    /// Lib settings for this call, with runtime view toggles overlaid.
    fn settings_for(&self, config: &ClinConfig) -> GrafSettings {
        let mut s = clin_settings(config);
        s.visual.show_minimap = self.show_minimap;
        s.visual.show_legend = self.show_legend;
        s.visual.show_grid = self.grid;
        s.visual.show_looking_glass = self.show_looking_glass;
        s.display.show_status_bar = self.show_status_bar;
        s
    }

    pub fn refresh_simulation(&mut self, config: &ClinConfig) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        let mut settings = self.settings_for(config);
        if self.focus_note_ids.is_some() {
            // Focus (local/group) subsets must render every selected node,
            // including ones without connections, regardless of show_orphan.
            settings.filter.show_orphan = true;
        }
        let filtered;
        let notes: &[crate::storage::NoteSummary] = match &self.focus_note_ids {
            Some(ids) => {
                filtered = self
                    .notes
                    .iter()
                    .filter(|n| ids.contains(&n.id))
                    .cloned()
                    .collect::<Vec<_>>();
                &filtered
            }
            None => &self.notes,
        };
        if let Ok(graph_state) = GraphState::from_specs(&note_specs(notes), &settings) {
            let state = Arc::new(RwLock::new(graph_state));
            let graph_kill_tx = graf::start_physics(state.clone(), &settings);
            self.graph_state = Some(state);
            self.graph_kill_tx = graph_kill_tx;
            self.search_popup = None;
        }
    }

    /// Enter a focus mode (local graph or group): rebuild the simulation with
    /// only the given subset of note ids, then mark the active mode banner.
    pub fn enter_focus(
        &mut self,
        config: &ClinConfig,
        ids: std::collections::HashSet<String>,
        mode: ModeBanner,
    ) {
        self.focus_note_ids = Some(ids);
        self.refresh_simulation(config);
        if let Some(gs) = &self.graph_state {
            gs.write().mode_banner = Some(mode);
        }
    }

    /// Exit focus mode: rebuild the full graph.
    pub fn exit_focus(&mut self, config: &ClinConfig) {
        self.focus_note_ids = None;
        self.refresh_simulation(config);
    }

    pub fn shutdown(&mut self) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        self.graph_state = None;
    }

    pub fn poll_renderers(&mut self, config: &ClinConfig) -> bool {
        let mut updated = false;

        if let Some((_, inst)) = self.pending_markdown_resize
            && inst.elapsed() >= std::time::Duration::from_millis(50)
        {
            self.pending_markdown_resize = None;
            self.update_preview(config, None);
            updated = true;
        }

        if let Some(PreviewContent::Markdown(renderer)) = &mut self.preview_content
            && renderer.poll()
        {
            updated = true;
        }

        updated
    }

    fn build_preview_key(&self) -> Option<PreviewRequestKey> {
        let note_id = if let Some(gs) = &self.graph_state {
            let guard = gs.read();
            if let Some(idx) = guard.selection.primary {
                guard
                    .simulation
                    .get_graph()
                    .node_weight(idx)
                    .map(|n| n.data.id.clone())
            } else {
                None
            }
        } else {
            None
        }?;

        let is_draw = note_id.ends_with(".draw");
        let is_canvas = note_id.ends_with(".canvas");
        let width = if is_draw || is_canvas {
            self.last_preview_pane_width
        } else {
            self.last_preview_pane_width.saturating_sub(2).max(40)
        };

        Some(PreviewRequestKey {
            note_id,
            width,
            height: self.last_preview_pane_height,
            scale: self.preview_scale,
            offset_x: self.preview_offset_x,
            offset_y: self.preview_offset_y,
        })
    }

    pub fn sync_preview(&mut self, config: &ClinConfig) {
        if !self.preview_enabled {
            self.preview_content = None;
            self.preview_note_id = None;
            self.preview_request_key = None;
            return;
        }

        let new_key = self.build_preview_key();

        if new_key.is_none() {
            self.preview_content = None;
            self.preview_note_id = None;
            self.preview_request_key = None;
            return;
        }

        if new_key != self.preview_request_key {
            let Some(key) = new_key else {
                return;
            };
            let old_width = self.preview_request_key.as_ref().map(|k| k.width);
            self.preview_note_id = Some(key.note_id.clone());
            self.preview_request_key = Some(key);
            self.update_preview(config, old_width);
        }
    }

    pub fn update_preview(&mut self, config: &ClinConfig, old_width: Option<u16>) {
        let Some(key) = self.preview_request_key.clone() else {
            self.preview_content = None;
            return;
        };

        let is_draw = key.note_id.ends_with(".draw");
        let is_canvas = key.note_id.ends_with(".canvas");
        let is_clin = key.note_id.ends_with(".clin");

        if config.list.preview_encryption && is_clin {
            self.preview_content = None;
            return;
        }

        if is_draw {
            let path = self.storage.note_path(&key.note_id);
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<crate::draw::state::DrawData>(&content) {
                        Ok(data) => {
                            let grid = crate::snapshot::render_draw_snapshot_with_size(
                                &data,
                                &self.app_theme,
                                config.ui.icon_mode,
                                key.width,
                                key.height,
                                key.scale,
                                key.offset_x,
                                key.offset_y,
                            );
                            self.preview_content = Some(PreviewContent::DrawGrid {
                                data: Box::new(data),
                                grid,
                            });
                        }
                        Err(_) => {
                            self.preview_content = None;
                        }
                    }
                }
                Err(_) => {
                    self.preview_content = None;
                }
            }
            return;
        }

        if is_canvas {
            let path = self.storage.note_path(&key.note_id);
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<pinstar::data::CanvasData>(&content) {
                    Ok(data) => {
                        let grid = crate::snapshot::render_canvas_snapshot(
                            &data,
                            &self.app_theme,
                            config.ui.icon_mode,
                            key.width,
                            key.height,
                            key.scale,
                            key.offset_x,
                            key.offset_y,
                        );
                        self.preview_content = Some(PreviewContent::CanvasGrid {
                            data: Box::new(data),
                            grid,
                        });
                    }
                    Err(_) => {
                        self.preview_content = None;
                    }
                },
                Err(_) => {
                    self.preview_content = None;
                }
            }
            return;
        }

        if let Ok(note) = self.storage.load_note(&key.note_id) {
            let mut renderer = match self.preview_content.take() {
                Some(PreviewContent::Markdown(r)) => *r,
                _ => MarkdownRenderer::new(),
            };
            let opts = crate::markdown::MdRenderOpts::from_config(config, Some(&key.note_id));
            let height = key.height;
            let viewport = crate::markdown::RenderViewport {
                start: renderer.visible_start(),
                height: height as usize,
            };

            let content_changed = renderer.is_changed(&note.content, &self.app_theme, &opts);
            let mut should_render = false;
            if content_changed || renderer.document().is_none() {
                should_render = true;
            } else if let Some(old_w) = old_width {
                if old_w == key.width {
                    renderer.set_viewport(viewport.start, viewport.height);
                } else {
                    let now = std::time::Instant::now();
                    if let Some((w, _)) = self.pending_markdown_resize {
                        if w != key.width {
                            self.pending_markdown_resize = Some((key.width, now));
                        }
                    } else {
                        self.pending_markdown_resize = Some((key.width, now));
                    }
                }
            } else {
                should_render = true;
            }

            if should_render {
                renderer.render_with(&note.content, key.width, &self.app_theme, &opts, viewport);
                self.pending_markdown_resize = None;
            }
            self.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
        } else {
            self.preview_content = None;
        }
    }

    pub fn overlay_update(&mut self, config: &mut ClinConfig) {
        self.sync_preview(config);
        let _ = self.poll_renderers(config);
    }
}

// ── Event plumbing ──────────────────────────────────────────────────────────

enum EventAction {
    Quit,
    OpenFile(String),
    OpenHelp,
    NoteModified(String),
}

/// Graph area inside the view: below the title bar, minus the preview pane.
fn graph_area(
    app_state: &GrafPlugin,
    config: &ClinConfig,
    term_area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let outer = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(term_area);
    let content_area = outer[1];
    if app_state.preview_enabled {
        let (constraints, main_idx) = match config.list.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    ratatui::layout::Constraint::Ratio(43, 100),
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ],
                2,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    ratatui::layout::Constraint::Min(0),
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Ratio(43, 100),
                ],
                0,
            ),
        };
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(constraints)
            .split(content_area);
        chunks[main_idx]
    } else {
        content_area
    }
}

fn add_wikilink_to_note(
    storage: &mut Storage,
    note_id: &str,
    target_title: &str,
) -> anyhow::Result<()> {
    if note_id.ends_with(".canvas") || note_id.ends_with(".draw") {
        return Ok(());
    }

    let mut note = storage.load_note(note_id)?;
    let link = format!("[[{target_title}]]");
    if !note.content.contains(&link) {
        if let Some(idx) = note.content.find("\n## Links\n") {
            note.content
                .insert_str(idx + "\n## Links\n".len(), &format!("{link}\n"));
        } else if let Some(idx) = note.content.find("\n## Links") {
            if idx + "\n## Links".len() == note.content.len() {
                note.content.push_str(&format!("\n{link}\n"));
            } else {
                let ensure_newline = if note.content.ends_with('\n') {
                    ""
                } else {
                    "\n"
                };
                note.content
                    .push_str(&format!("{ensure_newline}\n## Links\n{link}\n"));
            }
        } else {
            let ensure_newline = if note.content.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            note.content
                .push_str(&format!("{ensure_newline}\n## Links\n{link}\n"));
        }
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(note.updated_at);
        note.updated_at = time;
        storage.save_note(note_id, &note)?;
    }
    Ok(())
}

fn remove_wikilink_from_note(
    storage: &mut Storage,
    note_id: &str,
    target_title: &str,
) -> anyhow::Result<()> {
    if note_id.ends_with(".canvas") || note_id.ends_with(".draw") {
        return Ok(());
    }

    let mut note = storage.load_note(note_id)?;
    let pattern = format!("[[{target_title}");
    let mut out = String::with_capacity(note.content.len());
    let mut rest = note.content.as_str();
    while let Some(start) = rest.find(&pattern) {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = &after[..end];
            let name = match inner.find('|') {
                Some(p) => &inner[..p],
                None => inner,
            }
            .trim();
            if name.eq_ignore_ascii_case(target_title) {
                let mut prefix = &rest[..start];
                if prefix.ends_with(' ') {
                    prefix = prefix.trim_end_matches(' ');
                }
                out.push_str(prefix);

                let rest_after = &after[end + 2..];
                let consume_newline =
                    rest_after.starts_with('\n') || rest_after.starts_with("\r\n");

                if consume_newline && prefix.ends_with('\n') {
                    rest = if let Some(stripped) = rest_after.strip_prefix("\r\n") {
                        stripped
                    } else {
                        &rest_after[1..]
                    };
                } else {
                    rest = rest_after;
                }
                continue;
            }
        }
        out.push_str(&rest[..start + pattern.len()]);
        rest = &rest[start + pattern.len()..];
    }
    out.push_str(rest);

    let trimmed = out.trim_end();
    if trimmed.ends_with("## Links") {
        let new_len = trimmed.len() - "## Links".len();
        let mut new_out = trimmed[..new_len].trim_end().to_string();
        if !new_out.is_empty() {
            new_out.push('\n');
        }
        out = new_out;
    }

    note.content = out;
    // Always save the note to ensure frontmatter stays in sync with content.
    // If the link existed in frontmatter but not in the body (ghost link),
    // this self-heals the file by purging it from the frontmatter.
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(note.updated_at);
    note.updated_at = time;
    storage.save_note(note_id, &note)?;

    Ok(())
}

fn refresh_note_summaries(storage: &Storage) -> Vec<crate::storage::NoteSummary> {
    storage
        .list_note_ids(false, false)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| storage.load_note_summary(&id).ok())
        .collect()
}

/// Persist a wikilink edit and apply the resulting edge change to the live
/// simulation without rebuilding it (positions/viewport/physics preserved).
fn apply_connection(
    state: &mut GrafPlugin,
    source_id: &str,
    target_title: &str,
    create: bool,
) -> Option<String> {
    let mut resolved_source_id = source_id.to_string();
    let mut resolved_target_title = target_title.to_string();

    if !create {
        let source_has_link = state.notes.iter().any(|n| {
            n.id == source_id && n.links.iter().any(|l| l.eq_ignore_ascii_case(target_title))
        });

        if !source_has_link
            && let Some(target_note) = state
                .notes
                .iter()
                .find(|n| n.title.eq_ignore_ascii_case(target_title))
            && let Some(source_note) = state.notes.iter().find(|n| n.id == source_id)
        {
            let source_title = &source_note.title;
            if target_note
                .links
                .iter()
                .any(|l| l.eq_ignore_ascii_case(source_title))
            {
                resolved_source_id = target_note.id.clone();
                resolved_target_title = source_title.clone();
            }
        }
    }

    let result = if create {
        add_wikilink_to_note(
            &mut state.storage,
            &resolved_source_id,
            &resolved_target_title,
        )
    } else {
        remove_wikilink_from_note(
            &mut state.storage,
            &resolved_source_id,
            &resolved_target_title,
        )
    };
    if result.is_err() {
        return None;
    }
    // Keep state.notes in sync (used by the search popup and a later manual rebuild).
    if let Some(src_summary) = state.notes.iter_mut().find(|n| n.id == resolved_source_id) {
        if create {
            if !src_summary
                .links
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&resolved_target_title))
            {
                src_summary.links.push(resolved_target_title.to_string());
            }
        } else {
            src_summary
                .links
                .retain(|l| !l.eq_ignore_ascii_case(&resolved_target_title));
        }
    }
    // Mutate the live graph; do NOT rebuild the simulation.
    let Some(gs) = state.graph_state.as_ref() else {
        return Some(source_id.to_string());
    };
    let (src_idx, tgt_idx) = {
        let g = gs.read();
        let graph = g.simulation.get_graph();
        let src = graph
            .node_indices()
            .find(|i| graph[*i].data.id == resolved_source_id);
        let tgt = graph.node_indices().find(|i| {
            graph[*i]
                .data
                .title
                .eq_ignore_ascii_case(&resolved_target_title)
        });
        (src, tgt)
    };
    if let (Some(s), Some(t)) = (src_idx, tgt_idx) {
        let mut g = gs.write();
        graf::apply_connection_change(&mut g.simulation, s, t, create);
    }
    Some(resolved_source_id)
}

fn execute_menu_action(state: &mut GrafPlugin, config: &ClinConfig, item: LibMenuItem) {
    use LibMenuItem::{CreateConnection, DeleteConnection, DeleteNode, LocalGraph, ShowGroup};
    let Some(graph) = state.graph_state.as_ref() else {
        return;
    };
    match item {
        CreateConnection => {
            let mut g = graph.write();
            if let Some(src) = g.selection.primary {
                g.connection_source = Some(src);
                g.mode_banner = Some(ModeBanner::CreateConnection);
                g.context_menu = None;
            }
        }
        DeleteConnection => {
            let mut g = graph.write();
            if let Some(src) = g.selection.primary {
                g.deleting_connection_source = Some(src);
                g.mode_banner = Some(ModeBanner::DeleteConnection);
                g.context_menu = None;
            }
        }
        LocalGraph => {
            let ids: std::collections::HashSet<String> = {
                let g = graph.read();
                let mut ids = std::collections::HashSet::new();
                if let Some(anchor) = g.selection.primary {
                    let graph_ref = g.simulation.get_graph();
                    if let Some(n) = graph_ref.node_weight(anchor) {
                        ids.insert(n.data.id.clone());
                    }
                    for nbr in graph_ref.neighbors(anchor) {
                        if let Some(n) = graph_ref.node_weight(nbr) {
                            ids.insert(n.data.id.clone());
                        }
                    }
                }
                ids
            };
            if !ids.is_empty() {
                state.enter_focus(config, ids, ModeBanner::LocalGraph);
            }
        }
        ShowGroup => {
            let ids: std::collections::HashSet<String> = {
                let g = graph.read();
                g.selection
                    .extra
                    .iter()
                    .filter_map(|idx| g.simulation.get_graph().node_weight(*idx))
                    .map(|n| n.data.id.clone())
                    .collect()
            };
            if !ids.is_empty() {
                state.enter_focus(config, ids, ModeBanner::GroupedGraph);
            }
        }
        DeleteNode => {
            let ids: Vec<String> = {
                let g = graph.read();
                let mut v = Vec::new();
                if let Some(idx) = g.selection.primary
                    && let Some(n) = g.simulation.get_graph().node_weight(idx)
                {
                    v.push(n.data.id.clone());
                }
                for idx in &g.selection.extra {
                    if let Some(n) = g.simulation.get_graph().node_weight(*idx) {
                        let id = n.data.id.clone();
                        if !v.contains(&id) {
                            v.push(id);
                        }
                    }
                }
                v
            };
            for id in ids {
                let _ = state.storage.trash_note(&id);
            }
            state.notes = refresh_note_summaries(&state.storage);
            state.refresh_simulation(config);
        }
    }
}

/// Signals produced while resolving a key against clin keybinds; applied
/// after the graph write-guard is dropped (lib `apply_action` takes its own).
enum ResolvedKey {
    Quit,
    OpenHelp,
    ToggleSearch,
    ToggleMinimap,
    ToggleLegend,
    ToggleGrid,
    ToggleStatus,
    ToggleLookingGlass,
    Refresh,
    TogglePreview,
    ClearFocus,
    OpenNote,
    MenuAction(LibMenuItem),
    /// (lib action, repeat count)
    Apply(LibAction, usize),
}

fn resolve_graph_key(
    app_state: &mut GrafPlugin,
    key: crossterm::event::KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
    area: ratatui::layout::Rect,
) -> Option<ResolvedKey> {
    let gs = app_state.graph_state.clone()?;

    // Context menu open: keys drive the menu exclusively.
    {
        let mut guard = gs.write();
        if let Some(menu) = guard.context_menu.as_mut() {
            app_state.seq_matcher.clear();
            let mut dispatch: Option<ResolvedKey> = None;
            let mut close = false;

            if keybinds.matches_graph(GraphAction::MenuClose, &key) {
                close = true;
            } else if keybinds.matches_graph(GraphAction::MenuUp, &key) {
                menu.move_up();
            } else if keybinds.matches_graph(GraphAction::MenuDown, &key) {
                menu.move_down();
            } else if keybinds.matches_graph(GraphAction::MenuSelect, &key) {
                if let Some(spec) = menu.items.get(menu.selected)
                    && let Some(item) = graf::menu_item_from_label(spec.label)
                {
                    dispatch = Some(ResolvedKey::MenuAction(item));
                    close = true;
                }
            } else if let KeyCode::Char(c) = key.code
                && let Some(idx) = menu.find_shortcut(c)
                && let Some(spec) = menu.items.get(idx)
                && let Some(item) = graf::menu_item_from_label(spec.label)
            {
                dispatch = Some(ResolvedKey::MenuAction(item));
                close = true;
            }

            if close {
                guard.context_menu = None;
            }
            return dispatch;
        }

        // Escape: cancel connection modes, clear the focus filter (full
        // rebuild), or clear multi-select — before falling through to quit.
        if key.code == KeyCode::Esc {
            if guard.connection_source.is_some() || guard.deleting_connection_source.is_some() {
                guard.connection_source = None;
                guard.deleting_connection_source = None;
                guard.mode_banner = None;
                return None;
            }
            if matches!(
                guard.mode_banner,
                Some(ModeBanner::LocalGraph | ModeBanner::GroupedGraph)
            ) {
                return Some(ResolvedKey::ClearFocus);
            }
            if !guard.selection.extra.is_empty() {
                guard.selection.clear_set();
                guard.mode_banner = None;
                return None;
            }
        }
    }

    if crate::events::is_universal_quit_key(&key) {
        return Some(ResolvedKey::Quit);
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    let matcher_result = keybinds.resolve_graph(&mut app_state.seq_matcher, key, seq, counts);
    match matcher_result {
        crate::keybinds::MatchOutcome::Matched(action, count) => {
            let n = count.unwrap_or(1) as usize;
            match action {
                GraphAction::Quit => Some(ResolvedKey::Quit),
                GraphAction::PanUp | GraphAction::MenuUp => {
                    Some(ResolvedKey::Apply(LibAction::PanUp, n))
                }
                GraphAction::PanDown | GraphAction::MenuDown => {
                    Some(ResolvedKey::Apply(LibAction::PanDown, n))
                }
                GraphAction::PanLeft => Some(ResolvedKey::Apply(LibAction::PanLeft, n)),
                GraphAction::PanRight | GraphAction::LocalGraph => {
                    // `l` is shared with LocalGraph (menu-only); outside the
                    // menu it keeps its historical "pan right" meaning.
                    Some(ResolvedKey::Apply(LibAction::PanRight, n))
                }
                GraphAction::ZoomIn => Some(ResolvedKey::Apply(LibAction::ZoomIn, 1)),
                GraphAction::ZoomOut => Some(ResolvedKey::Apply(LibAction::ZoomOut, 1)),
                GraphAction::OpenNote | GraphAction::MenuSelect => Some(ResolvedKey::OpenNote),
                GraphAction::AutoFit => Some(ResolvedKey::Apply(LibAction::AutoFit, 1)),
                GraphAction::Help => Some(ResolvedKey::OpenHelp),
                GraphAction::ToggleSearch => Some(ResolvedKey::ToggleSearch),
                GraphAction::ToggleMinimap => Some(ResolvedKey::ToggleMinimap),
                GraphAction::ToggleLegend => Some(ResolvedKey::ToggleLegend),
                GraphAction::ToggleGrid => Some(ResolvedKey::ToggleGrid),
                GraphAction::ToggleStatus => Some(ResolvedKey::ToggleStatus),
                GraphAction::Refresh => Some(ResolvedKey::Refresh),
                GraphAction::TogglePreview => Some(ResolvedKey::TogglePreview),
                GraphAction::LookingGlass => Some(ResolvedKey::ToggleLookingGlass),
                GraphAction::OpenContextMenu => {
                    let mut guard = gs.write();
                    let (sx, sy) = match guard.selection.primary {
                        Some(idx) => {
                            let node = guard.simulation.get_graph().node_weight(idx);
                            match node {
                                Some(n) => {
                                    let (cx, cy) = guard.viewport.world_to_screen(
                                        n.location.x as f64,
                                        n.location.y as f64,
                                        area,
                                    );
                                    (cx as u16, cy as u16)
                                }
                                None => (area.x + 2, area.y + 2),
                            }
                        }
                        None => (area.x + 2, area.y + 2),
                    };
                    guard.open_context_menu(sx, sy, (0.0, 0.0));
                    None
                }
                GraphAction::CreateConnection => {
                    Some(ResolvedKey::MenuAction(LibMenuItem::CreateConnection))
                }
                GraphAction::DeleteConnection => {
                    Some(ResolvedKey::MenuAction(LibMenuItem::DeleteConnection))
                }
                GraphAction::ShowGroup => Some(ResolvedKey::MenuAction(LibMenuItem::ShowGroup)),
                GraphAction::DeleteNode => Some(ResolvedKey::MenuAction(LibMenuItem::DeleteNode)),
                GraphAction::MenuClose => Some(ResolvedKey::Quit),
            }
        }
        crate::keybinds::MatchOutcome::Pending => None,
        crate::keybinds::MatchOutcome::NoMatch => None,
    }
}

fn handle_event(
    ev: crossterm::event::Event,
    app_state: &mut GrafPlugin,
    config: &ClinConfig,
    keybinds: &Keybinds,
    term_area: ratatui::layout::Rect,
) -> anyhow::Result<Option<EventAction>> {
    match ev {
        crossterm::event::Event::Key(key) => {
            if app_state.search_popup.is_some() {
                app_state.seq_matcher.clear();
                handle_search_keys(app_state, key, config, keybinds);
                return Ok(None);
            }
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
            {
                match key.code {
                    KeyCode::Char('h') => {
                        if let Some(PreviewContent::Markdown(renderer)) =
                            &mut app_state.preview_content
                        {
                            renderer.prev_page();
                            return Ok(None);
                        }
                    }
                    KeyCode::Char('l') => {
                        if let Some(PreviewContent::Markdown(renderer)) =
                            &mut app_state.preview_content
                        {
                            renderer.next_page();
                            return Ok(None);
                        }
                    }
                    _ => {}
                }
            }
            let graph_area = graph_area(app_state, config, term_area);
            let Some(resolved) = resolve_graph_key(app_state, key, keybinds, config, graph_area)
            else {
                return Ok(None);
            };
            let settings = app_state.settings_for(config);
            let out = match resolved {
                ResolvedKey::Quit => Some(EventAction::Quit),
                ResolvedKey::OpenHelp => Some(EventAction::OpenHelp),
                ResolvedKey::ToggleSearch => {
                    app_state.search_popup = Some(crate::ui::quick_search::QuickSearch::new(
                        "Search",
                        &app_state.app_theme,
                    ));
                    None
                }
                ResolvedKey::ToggleMinimap => {
                    app_state.show_minimap = !app_state.show_minimap;
                    None
                }
                ResolvedKey::ToggleLegend => {
                    app_state.show_legend = !app_state.show_legend;
                    None
                }
                ResolvedKey::ToggleGrid => {
                    app_state.grid = !app_state.grid;
                    None
                }
                ResolvedKey::ToggleStatus => {
                    app_state.show_status_bar = !app_state.show_status_bar;
                    None
                }
                ResolvedKey::ToggleLookingGlass => {
                    app_state.show_looking_glass = !app_state.show_looking_glass;
                    None
                }
                ResolvedKey::OpenNote => {
                    let gs = app_state.graph_state.as_ref();
                    gs.and_then(|gs| apply_action(gs, LibAction::OpenSelected, &settings))
                        .and_then(|a| match a {
                            LibAction::OpenFile(id) => Some(EventAction::OpenFile(id)),
                            _ => None,
                        })
                }
                ResolvedKey::Refresh => {
                    app_state.refresh_simulation(config);
                    None
                }
                ResolvedKey::TogglePreview => {
                    app_state.preview_enabled = !app_state.preview_enabled;
                    if app_state.preview_enabled {
                        app_state.sync_preview(config);
                    } else {
                        app_state.preview_content = None;
                        app_state.preview_note_id = None;
                        app_state.preview_request_key = None;
                    }
                    None
                }
                ResolvedKey::MenuAction(item) => {
                    execute_menu_action(app_state, config, item);
                    None
                }
                ResolvedKey::ClearFocus => {
                    app_state.exit_focus(config);
                    None
                }
                ResolvedKey::Apply(action, n) => {
                    if let Some(gs) = app_state.graph_state.as_ref() {
                        for _ in 0..n {
                            apply_action(gs, action.clone(), &settings);
                        }
                    }
                    None
                }
            };
            Ok(out)
        }
        crossterm::event::Event::Mouse(mouse_event) => {
            app_state.mouse_pos = Some((mouse_event.column, mouse_event.row));
            if let Some(graph_state) = &app_state.graph_state {
                graph_state.write().mouse_pos = Some((mouse_event.column, mouse_event.row));
            }
            if let Some(popup) = &mut app_state.search_popup {
                let max_visible = config.graf.search.max_visible;
                let full_area = term_area;
                let outer = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(full_area);
                let content_area = outer[1];
                if let Some(action) = crate::ui::quick_search::handle_quick_search_mouse(
                    popup,
                    mouse_event,
                    content_area,
                    max_visible,
                    config.ui.icon_mode,
                ) {
                    match action {
                        crate::ui::quick_search::QuickSearchAction::Submit => {
                            if let Some(&(idx, _)) = popup.results.get(popup.selected) {
                                let (nx, ny) = if let Some(graph_state) = &app_state.graph_state {
                                    let guard = graph_state.read();
                                    let graph = guard.simulation.get_graph();
                                    if let Some(node) = graph.node_weight(idx) {
                                        (node.location.x as f64, node.location.y as f64)
                                    } else {
                                        (0.0, 0.0)
                                    }
                                } else {
                                    (0.0, 0.0)
                                };
                                if let Some(graph_state) = &app_state.graph_state {
                                    let mut guard = graph_state.write();
                                    guard.selection.select_only(idx);
                                    guard.viewport.center_on_node(nx as f32, ny as f32);
                                }
                            }
                            app_state.search_popup = None;
                        }
                        crate::ui::quick_search::QuickSearchAction::Cancel => {
                            app_state.search_popup = None;
                        }
                        crate::ui::quick_search::QuickSearchAction::Edited => {
                            run_search(app_state, config);
                        }
                        crate::ui::quick_search::QuickSearchAction::Navigated => {}
                    }
                }
                return Ok(None);
            }
            if let Some(graph_state) = &app_state.graph_state {
                let graph_area = graph_area(app_state, config, term_area);
                let settings = app_state.settings_for(config);

                if let Some(action) = handle_graph_mouse(
                    graph_state,
                    mouse_event,
                    graph_area,
                    &mut app_state.graph_mouse_state,
                    &settings,
                    app_state.show_status_bar,
                ) {
                    match action {
                        LibAction::OpenFile(path) => {
                            return Ok(Some(EventAction::OpenFile(path)));
                        }
                        LibAction::MenuAction(item) => {
                            execute_menu_action(app_state, config, item);
                            return Ok(None);
                        }
                        LibAction::ConnectionEvent {
                            source_id,
                            target_title,
                            create,
                        } => {
                            let mod_id =
                                apply_connection(app_state, &source_id, &target_title, create);
                            if let Some(id) = mod_id {
                                return Ok(Some(EventAction::NoteModified(id)));
                            }
                            return Ok(None);
                        }
                        _ => {}
                    }
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn handle_search_keys(
    app_state: &mut GrafPlugin,
    key: crossterm::event::KeyEvent,
    config: &ClinConfig,
    keybinds: &crate::keybinds::Keybinds,
) {
    let popup = match &mut app_state.search_popup {
        Some(popup) => popup,
        None => return,
    };
    match crate::ui::quick_search::handle_quick_search_keys(
        popup,
        key,
        keybinds,
        config.graf.search.max_visible,
    ) {
        crate::ui::quick_search::QuickSearchAction::Submit => {
            if let Some(&(idx, _)) = popup.results.get(popup.selected) {
                let (nx, ny) = if let Some(graph_state) = &app_state.graph_state {
                    let guard = graph_state.read();
                    let graph = guard.simulation.get_graph();
                    if let Some(node) = graph.node_weight(idx) {
                        (node.location.x as f64, node.location.y as f64)
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0)
                };
                if let Some(graph_state) = &app_state.graph_state {
                    let mut guard = graph_state.write();
                    guard.selection.select_only(idx);
                    guard.viewport.center_on_node(nx as f32, ny as f32);
                }
            }
            app_state.search_popup = None;
        }
        crate::ui::quick_search::QuickSearchAction::Cancel => {
            app_state.search_popup = None;
        }
        crate::ui::quick_search::QuickSearchAction::Edited => {
            run_search(app_state, config);
        }
        _ => {}
    }
}

fn run_search(app_state: &mut GrafPlugin, config: &ClinConfig) {
    let popup = match &mut app_state.search_popup {
        Some(popup) => popup,
        None => return,
    };
    let query = popup.query();
    if let Some(graph_state) = &app_state.graph_state {
        let guard = graph_state.read();
        popup.results =
            graf::search_nodes(&guard.simulation, &query, config.graf.search.max_results);
    }
    popup.selected = 0;
    popup.scroll_offset = 0;
}

// ── Rendering ───────────────────────────────────────────────────────────────

use crate::app::ViewMode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::Span;

impl crate::overlay::OverlayView for GrafPlugin {
    fn overlay_render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let config = app.config.clone();
        draw_ui(frame, self, &config, &app.app_theme, area);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _app: &mut crate::app::App,
        term_area: Rect,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let config = _app.config.clone();
        let keybinds = self.keybinds.clone();
        if let Some(action) = handle_event(event, self, &config, &keybinds, term_area)? {
            match action {
                EventAction::Quit => {
                    self.shutdown();
                    return Ok(crate::overlay::OverlayResult::Exit);
                }
                EventAction::OpenFile(id) => {
                    self.shutdown();
                    return Ok(crate::overlay::OverlayResult::NoteOpened(id));
                }
                EventAction::OpenHelp => {
                    return Ok(crate::overlay::OverlayResult::OpenHelp(
                        crate::app::HelpTab::Graph,
                    ));
                }
                EventAction::NoteModified(id) => {
                    return Ok(crate::overlay::OverlayResult::NoteModified(id));
                }
            }
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}

fn draw_ui(
    frame: &mut Frame,
    state: &mut GrafPlugin,
    config: &ClinConfig,
    theme: &crate::app_theme::AppThemeColors,
    area: Rect,
) {
    let (graph_area, preview_area) = if state.preview_enabled {
        let (constraints, main_idx, p_idx) = match config.list.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(43, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(43, 100),
                ],
                0,
                2,
            ),
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);
        let p_area = full_cols[p_idx];
        state.last_preview_pane_width = p_area.width;
        state.last_preview_pane_height = p_area.height;
        (full_cols[main_idx], Some((p_area, full_cols[1])))
    } else {
        (area, None)
    };

    if !state.config_errors.is_empty() {
        draw_config_errors(frame, area, &state.config_errors, config);
        return;
    }

    let colors = config.theme_colors();

    if let Some(graph_state) = &state.graph_state {
        let guard = graph_state.read();
        let settings = state.settings_for(config);
        let graf_theme = clin_theme(config);
        let flags = FeatureFlags {
            show_legend: state.show_legend,
            grid: state.grid,
            show_minimap: state.show_minimap,
            show_status_bar: state.show_status_bar,
        };
        draw_graph_view(frame, graph_area, &guard, &settings, &graf_theme, &flags);
        if state.show_status_bar {
            draw_status_bar(
                frame,
                area,
                &guard,
                config,
                theme,
                &state.keybinds,
                state.seq_matcher.pending_display().as_deref(),
                state.grid,
            );
        }
    }

    if let Some((p_area, sep_area)) = preview_area {
        draw_preview(frame, p_area, state, config);
        draw_dim_vline(frame, sep_area, state.app_theme.muted);
    }

    if let Some(popup) = &state.search_popup {
        let max_visible = config.graf.search.max_visible;
        let theme = &state.app_theme;
        let popup_width = (50u16).min(area.width.saturating_sub(4));
        crate::ui::quick_search::draw_quick_search(
            frame,
            area,
            popup,
            theme,
            max_visible,
            move |(_, title), is_selected, theme: &crate::app_theme::AppThemeColors| {
                let style = if is_selected {
                    ratatui::style::Style::default().fg(theme.fg)
                } else {
                    ratatui::style::Style::default().fg(theme.highlight_fg)
                };
                let prefix = if is_selected { "▸ " } else { "  " };
                let display = crate::fsutil::truncate_ellipsis(
                    title,
                    (popup_width as usize).saturating_sub(6),
                );
                ratatui::text::Line::styled(format!("{prefix}{display}"), style)
            },
            config.ui.icon_mode,
        );
    }

    if let Some(ref msg) = state.config_reload_msg {
        draw_reload_notification(frame, area, msg, &colors, theme);
    }
}

/// Status bar the lib's `draw_graph_view` deliberately omits (host-owned).
#[allow(clippy::too_many_arguments)]
fn draw_status_bar(
    frame: &mut Frame,
    area: Rect,
    state: &GraphState,
    config: &ClinConfig,
    app_theme: &crate::app_theme::AppThemeColors,
    keybinds: &Keybinds,
    pending: Option<&str>,
    grid_visible: bool,
) {
    use ratatui::style::Style;

    let status_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        1,
    );
    let hints_items = vec![
        (
            format!(
                "{}/{}",
                keybinds.display_graph(GraphAction::PanUp),
                keybinds.display_graph(GraphAction::PanDown)
            ),
            "pan",
        ),
        (
            format!(
                "{}/{}",
                keybinds.display_graph(GraphAction::ZoomOut),
                keybinds.display_graph(GraphAction::ZoomIn)
            ),
            "zoom",
        ),
        (keybinds.display_graph(GraphAction::ToggleLegend), "labels"),
        (keybinds.display_graph(GraphAction::AutoFit), "fit"),
        (keybinds.graph_keys_display(GraphAction::Quit), "quit"),
        (
            format!("F1/{}", keybinds.graph_keys_display(GraphAction::Help)),
            "help",
        ),
        ("F2".to_string(), "keybinds"),
    ];
    let hint_line = crate::ui::format_keybind_hints(app_theme, &hints_items);
    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Graph);
    ctx.area = Some(status_area);
    ctx.graph = Some(state);
    ctx.graph_grid_visible = grid_visible;
    ctx.hints = Some(hint_line.spans);
    if let Some(p) = pending {
        ctx.pending = Some(vec![Span::styled(
            format!("{p} "),
            Style::default()
                .fg(app_theme.highlight_fg)
                .bg(app_theme.accent),
        )]);
    }

    let (left_line, right_line) =
        crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::Graph, app_theme);
    crate::ui::draw_status_bar(frame, status_area, app_theme, left_line, right_line);
}

fn draw_config_errors(frame: &mut Frame, area: Rect, errors: &[String], _config: &ClinConfig) {
    let config_path = crate::config::ClinConfig::config_path()
        .unwrap_or_default()
        .display()
        .to_string();
    let mut lines = vec!["Config Errors".to_string(), "".to_string()];
    for err in errors {
        lines.push(format!("  - {err}"));
        if let Some(suggestion) = suggest_fix(err) {
            lines.push(format!("    -> {suggestion}"));
        }
    }
    lines.push("".to_string());
    lines.push(format!("Fix: {config_path}"));
    lines.push("Press any key to close".to_string());

    let text = lines.join("\n");
    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Config Error")
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .alignment(ratatui::layout::Alignment::Left);

    let max_width = lines.iter().map(|l| l.len()).max().unwrap_or(0) + 4;
    let height = lines.len() as u16 + 2;
    let popup_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(max_width as u16)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width: max_width.min(area.width as usize) as u16,
        height: height.min(area.height),
    };

    frame.render_widget(paragraph, popup_area);
}

fn suggest_fix(err: &str) -> Option<String> {
    let err_lower = err.to_lowercase();
    if err_lower.contains("theme") {
        return Some("Valid themes: default, tokyonight, catppuccinmocha, onedark, gruvbox, dracula, nord, rosepine, everforest, kanagawa, solarized".to_string());
    }
    if err_lower.contains("background") {
        return Some("Valid backgrounds: transparent, solid".to_string());
    }
    if err_lower.contains("node_color_mode") {
        return Some("Valid modes: tag, folder, linkcount, uniform".to_string());
    }
    if err_lower.contains("edge_color_mode") {
        return Some("Valid modes: source, target, uniform".to_string());
    }
    if err_lower.contains("label_mode") {
        return Some("Valid modes: selected, neighbors, all, none".to_string());
    }
    if err_lower.contains("node_size_mode") {
        return Some("Valid modes: fixed, linkcount".to_string());
    }
    if err_lower.contains("legend_position") {
        return Some("Valid positions: topright, topleft, bottomright, bottomleft".to_string());
    }
    None
}

fn draw_reload_notification(
    frame: &mut Frame,
    area: Rect,
    msg: &str,
    colors: &crate::config::ThemeColors,
    theme: &crate::app_theme::AppThemeColors,
) {
    let width = (msg.len() as u16 + 4).min(area.width);
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height) / 2;

    let popup_area = ratatui::layout::Rect::new(x, y, width, height);

    let is_error = msg.starts_with("Config error");
    let border_color = if is_error {
        theme.destructive
    } else {
        colors.border_color
    };

    let paragraph = ratatui::widgets::Paragraph::new(msg)
        .style(ratatui::style::Style::default().fg(colors.label_color))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(border_color)),
        );

    frame.render_widget(paragraph, popup_area);
}

fn draw_preview(frame: &mut Frame, preview_rect: Rect, state: &GrafPlugin, config: &ClinConfig) {
    let hide_encrypted = config.list.preview_encryption
        && state
            .preview_note_id
            .as_ref()
            .is_some_and(|id| id.ends_with(".clin"));

    crate::preview::draw_preview_pane(
        frame,
        preview_rect,
        &state.app_theme,
        state.preview_content.as_ref(),
        hide_encrypted,
        0,
        config.ui.icon_mode,
    );
}

fn draw_dim_vline(frame: &mut Frame, area: Rect, color: Color) {
    let buf = frame.buffer_mut();
    for row in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.x, row)) {
            cell.set_symbol("│");
            cell.set_fg(color);
        }
    }
}
