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

use parking_lot::RwLock;
use std::sync::Arc;

use fdg_sim::petgraph::graph::NodeIndex;

use crossterm::event::KeyCode;

use crate::config::ClinConfig;
use crate::graf::graph::{FocusFilter, GrafMenuItem, ModeBanner};
use crate::graf::input::GraphMouseState;
use crate::keybinds::Keybinds;
use crate::list_view::PreviewContent;
use crate::markdown::MarkdownRenderer;
use crate::storage::Storage;

pub struct GrafAppState {
    pub graph_state: Option<Arc<RwLock<crate::graf::graph::GraphState>>>,
    pub graph_kill_tx: Option<std::sync::mpsc::Sender<()>>,
    pub graph_mouse_state: GraphMouseState,
    pub storage: Storage,
    pub notes: Vec<crate::storage::NoteSummary>,
    pub config_errors: Vec<String>,
    pub search_popup: Option<crate::ui::quick_search::QuickSearch<(NodeIndex, String)>>,
    pub show_minimap: bool,
    pub show_legend: bool,
    pub show_grid: bool,
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

impl Drop for GrafAppState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl GrafAppState {
    pub fn new(
        config: &ClinConfig,
        storage: Storage,
        summaries: Vec<crate::storage::NoteSummary>,
        config_errors: Vec<String>,
        keybinds: Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> anyhow::Result<Self> {
        let graph_state = crate::graf::graph::GraphState::new(&summaries, config)?;
        let state = Arc::new(RwLock::new(graph_state));
        let graph_kill_tx = crate::graf::physics::start_physics(state.clone(), config);

        Ok(Self {
            graph_state: Some(state),
            graph_kill_tx,
            graph_mouse_state: GraphMouseState::default(),
            storage,
            notes: summaries,

            config_errors,
            search_popup: None,
            show_minimap: config.graf.visual.show_minimap,
            show_legend: config.graf.visual.show_legend,
            show_grid: config.graf.visual.show_grid,
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

    pub fn refresh_simulation(&mut self, config: &ClinConfig) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        if let Ok(graph_state) = crate::graf::graph::GraphState::new(&self.notes, config) {
            let state = Arc::new(RwLock::new(graph_state));
            let graph_kill_tx = crate::graf::physics::start_physics(state.clone(), config);
            self.graph_state = Some(state);
            self.graph_kill_tx = graph_kill_tx;
            self.search_popup = None;
        }
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
            if let Some(idx) = guard.selected_node {
                guard
                    .simulation
                    .get_graph()
                    .node_weight(idx)
                    .map(|n| n.data.note_id.clone())
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
                Ok(content) => {
                    match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content) {
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
                    }
                }
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
            let opts = crate::markdown::MdRenderOpts::from_config(config);
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
}

pub enum EventAction {
    Quit,
    OpenFile(String),
    OpenHelp,
}

impl GrafAppState {
    pub fn overlay_update(&mut self, config: &mut crate::config::ClinConfig) {
        self.sync_preview(config);
        let _ = self.poll_renderers(config);
    }
}

impl crate::overlay::OverlayView for GrafAppState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        crate::graf::ui::draw_ui(frame, self, &app.config, area, &app.app_theme);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        term_area: ratatui::layout::Rect,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let keybinds = self.keybinds.clone();
        if let Some(action) = handle_event(event, self, &app.config, &keybinds, term_area)? {
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
            }
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}

fn graph_area(
    app_state: &GrafAppState,
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
    let mut area = if app_state.preview_enabled {
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
    };
    // A mode banner occupies the top row; keep mouse hit-testing in sync.
    if app_state
        .graph_state
        .as_ref()
        .is_some_and(|g| g.read().mode_banner.is_some())
    {
        area = ratatui::layout::Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
    }
    area
}

fn add_wikilink_to_note(
    storage: &mut Storage,
    note_id: &str,
    target_title: &str,
) -> anyhow::Result<()> {
    let mut note = storage.load_note(note_id)?;
    let link = format!("[[{target_title}]]");
    if !note.content.contains(&link) {
        if let Some(idx) = note.content.find("\n## Links\n") {
            note.content
                .insert_str(idx + "\n## Links\n".len(), &format!("{link}\n"));
        } else {
            note.content.push_str(&format!("\n\n## Links\n{link}\n"));
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
    let mut note = storage.load_note(note_id)?;
    let pattern = format!("[[{target_title}");
    let mut changed = false;
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
                out.push_str(rest[..start].trim_end_matches(['\n', ' ']));
                rest = &after[end + 2..];
                changed = true;
                continue;
            }
        }
        out.push_str(&rest[..start + pattern.len()]);
        rest = &rest[start + pattern.len()..];
    }
    out.push_str(rest);
    note.content = out;
    if changed {
        storage.save_note(note_id, &note)?;
    }
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

fn execute_menu_action(state: &mut GrafAppState, config: &ClinConfig, item: GrafMenuItem) {
    use GrafMenuItem::{CreateConnection, DeleteConnection, DeleteNode, LocalGraph, ShowGroup};
    let Some(graph) = state.graph_state.as_ref() else {
        return;
    };
    match item {
        CreateConnection => {
            let mut g = graph.write();
            if let Some(src) = g.selected_node {
                g.connection_source = Some(src);
                g.mode_banner = Some(ModeBanner::CreateConnection);
                g.context_menu = None;
            }
        }
        DeleteConnection => {
            let mut g = graph.write();
            if let Some(src) = g.selected_node {
                g.deleting_connection_source = Some(src);
                g.mode_banner = Some(ModeBanner::DeleteConnection);
                g.context_menu = None;
            }
        }
        LocalGraph => {
            let mut g = graph.write();
            if let Some(anchor) = g.selected_node {
                let neighbors: std::collections::HashSet<fdg_sim::petgraph::graph::NodeIndex> =
                    g.simulation.get_graph().neighbors(anchor).collect();
                g.focus_filter = Some(FocusFilter::Local { anchor, neighbors });
                g.mode_banner = Some(ModeBanner::LocalGraph);
                g.context_menu = None;
            }
        }
        ShowGroup => {
            let mut g = graph.write();
            if !g.selected_nodes.is_empty() {
                g.focus_filter = Some(FocusFilter::Group(g.selected_nodes.clone()));
                g.mode_banner = Some(ModeBanner::GroupedGraph);
                g.context_menu = None;
            }
        }
        DeleteNode => {
            let ids: Vec<String> = {
                let g = graph.read();
                let mut v = Vec::new();
                if let Some(idx) = g.selected_node
                    && let Some(n) = g.simulation.get_graph().node_weight(idx)
                {
                    v.push(n.data.note_id.clone());
                }
                for idx in &g.selected_nodes {
                    if let Some(n) = g.simulation.get_graph().node_weight(*idx) {
                        let id = n.data.note_id.clone();
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

fn handle_event(
    ev: crossterm::event::Event,
    app_state: &mut GrafAppState,
    config: &crate::config::ClinConfig,
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
                        if let Some(crate::list_view::PreviewContent::Markdown(renderer)) =
                            &mut app_state.preview_content
                        {
                            renderer.prev_page();
                            return Ok(None);
                        }
                    }
                    KeyCode::Char('l') => {
                        if let Some(crate::list_view::PreviewContent::Markdown(renderer)) =
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
            if let Some(graph_state) = &app_state.graph_state
                && let Some(action) = crate::graf::input::handle_graph_keys(
                    graph_state,
                    key,
                    keybinds,
                    config,
                    &mut app_state.seq_matcher,
                    graph_area,
                )
            {
                use crate::graf::input::GraphInputAction;
                match action {
                    GraphInputAction::Quit => return Ok(Some(EventAction::Quit)),
                    GraphInputAction::ToggleHelp => {
                        return Ok(Some(EventAction::OpenHelp));
                    }
                    GraphInputAction::ToggleSearch => {
                        app_state.search_popup = Some(crate::ui::quick_search::QuickSearch::new(
                            "Search",
                            &app_state.app_theme,
                        ));
                        return Ok(None);
                    }
                    GraphInputAction::ToggleMinimap => {
                        app_state.show_minimap = !app_state.show_minimap;
                        return Ok(None);
                    }
                    GraphInputAction::ToggleLegend => {
                        app_state.show_legend = !app_state.show_legend;
                        return Ok(None);
                    }
                    GraphInputAction::ToggleGrid => {
                        app_state.show_grid = !app_state.show_grid;
                        return Ok(None);
                    }
                    GraphInputAction::ToggleStatus => {
                        app_state.show_status_bar = !app_state.show_status_bar;
                        return Ok(None);
                    }
                    GraphInputAction::ToggleLookingGlass => {
                        app_state.show_looking_glass = !app_state.show_looking_glass;
                        return Ok(None);
                    }
                    GraphInputAction::OpenFile(path) => {
                        return Ok(Some(EventAction::OpenFile(path)));
                    }
                    GraphInputAction::Refresh => {
                        app_state.refresh_simulation(config);
                        return Ok(None);
                    }
                    GraphInputAction::ReloadConfig => {
                        return Ok(None);
                    }
                    GraphInputAction::TogglePreview => {
                        app_state.preview_enabled = !app_state.preview_enabled;
                        if app_state.preview_enabled {
                            app_state.sync_preview(config);
                        } else {
                            app_state.preview_content = None;
                            app_state.preview_note_id = None;
                            app_state.preview_request_key = None;
                        }
                        return Ok(None);
                    }
                    GraphInputAction::MenuAction(item) => {
                        execute_menu_action(app_state, config, item);
                        return Ok(None);
                    }
                    GraphInputAction::ConnectionEvent {
                        source_id,
                        target_title,
                        create,
                    } => {
                        let result = if create {
                            add_wikilink_to_note(&mut app_state.storage, &source_id, &target_title)
                        } else {
                            remove_wikilink_from_note(
                                &mut app_state.storage,
                                &source_id,
                                &target_title,
                            )
                        };
                        if result.is_ok() {
                            app_state.notes = refresh_note_summaries(&app_state.storage);
                            app_state.refresh_simulation(config);
                        }
                        return Ok(None);
                    }
                }
            }
            Ok(None)
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
                                    guard.selected_node = Some(idx);
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

                if let Some(action) = crate::graf::input::handle_graph_mouse(
                    graph_state,
                    mouse_event,
                    graph_area,
                    &mut app_state.graph_mouse_state,
                    config,
                ) {
                    use crate::graf::input::GraphInputAction;
                    match action {
                        GraphInputAction::OpenFile(path) => {
                            return Ok(Some(EventAction::OpenFile(path)));
                        }
                        GraphInputAction::MenuAction(item) => {
                            execute_menu_action(app_state, config, item);
                            return Ok(None);
                        }
                        GraphInputAction::ConnectionEvent {
                            source_id,
                            target_title,
                            create,
                        } => {
                            let result = if create {
                                add_wikilink_to_note(
                                    &mut app_state.storage,
                                    &source_id,
                                    &target_title,
                                )
                            } else {
                                remove_wikilink_from_note(
                                    &mut app_state.storage,
                                    &source_id,
                                    &target_title,
                                )
                            };
                            if result.is_ok() {
                                app_state.notes = refresh_note_summaries(&app_state.storage);
                                app_state.refresh_simulation(config);
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
    app_state: &mut GrafAppState,
    key: crossterm::event::KeyEvent,
    config: &crate::config::ClinConfig,
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
                    guard.selected_node = Some(idx);
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

fn run_search(app_state: &mut GrafAppState, config: &crate::config::ClinConfig) {
    let popup = match &mut app_state.search_popup {
        Some(popup) => popup,
        None => return,
    };
    let query = popup.query();
    if let Some(graph_state) = &app_state.graph_state {
        let guard = graph_state.read();
        popup.results = crate::graf::graph::search_nodes(
            &guard.simulation,
            &query,
            config.graf.search.max_results,
        );
    }
    popup.selected = 0;
    popup.scroll_offset = 0;
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_canvas_fps_sampler_deterministic() {
        let mut sampler = CanvasFpsSampler::default();

        let t0 = Instant::now();
        sampler.record_frame(t0);
        assert_eq!(sampler.published_fps, None);
        assert_eq!(sampler.frames_in_window, 0);

        let t1 = t0 + Duration::from_millis(100);
        sampler.record_frame(t1);
        let t2 = t0 + Duration::from_millis(200);
        sampler.record_frame(t2);
        let t3 = t0 + Duration::from_millis(300);
        sampler.record_frame(t3);
        let t4 = t0 + Duration::from_millis(400);
        sampler.record_frame(t4);
        assert_eq!(sampler.published_fps, None);
        assert_eq!(sampler.frames_in_window, 4);

        let t5 = t0 + Duration::from_millis(500);
        sampler.record_frame(t5);
        assert_eq!(sampler.published_fps, Some(10.0));
        assert_eq!(sampler.frames_in_window, 0);
        assert_eq!(sampler.window_started_at, Some(t5));

        let t6 = t5 + Duration::from_millis(100);
        sampler.record_frame(t6);
        assert_eq!(sampler.published_fps, Some(10.0));
        assert_eq!(sampler.frames_in_window, 1);
    }

    #[test]
    fn test_sync_preview_request_key_identity() {
        let temp_dir = tempfile::tempdir().unwrap();
        let notes_dir = temp_dir.path().join("notes");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(notes_dir.join("a.md"), "content a").unwrap();

        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir,
            notes_dir,
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.data_dir).unwrap();
        std::fs::create_dir_all(&storage.templates_dir).unwrap();

        let mut config = crate::config::ClinConfig::default();
        config.graf.filter.show_orphan = true;
        let keybinds = crate::keybinds::Keybinds::default();
        let seq_matcher = crate::keybinds::KeyMatcher::new();

        let mut app_state = GrafAppState::new(
            &config,
            storage,
            vec![crate::storage::NoteSummary {
                id: "a.md".to_string(),
                title: "a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            }],
            vec![],
            keybinds,
            seq_matcher,
        )
        .unwrap();

        let gs_ref = app_state.graph_state.as_ref().unwrap();
        let node_idx = {
            let guard = gs_ref.read();
            guard.simulation.get_graph().node_indices().next().unwrap()
        };

        gs_ref.write().selected_node = Some(node_idx);
        app_state.preview_enabled = true;
        app_state.last_preview_pane_width = 100;
        app_state.last_preview_pane_height = 40;

        app_state.sync_preview(&config);
        assert!(app_state.preview_request_key.is_some());
        let original_key = app_state.preview_request_key.clone().unwrap();
        assert_eq!(original_key.note_id, "a.md");
        assert_eq!(original_key.width, 98);

        app_state.sync_preview(&config);
        assert_eq!(
            app_state.preview_request_key.as_ref().unwrap(),
            &original_key
        );

        app_state.last_preview_pane_width = 80;
        app_state.sync_preview(&config);
        let new_key = app_state.preview_request_key.clone().unwrap();
        assert_ne!(new_key, original_key);
        assert_eq!(new_key.width, 78);

        let note_path = app_state.storage.note_path("a.md");
        let _ = std::fs::remove_file(note_path);

        app_state.last_preview_pane_width = 60;
        app_state.sync_preview(&config);
        assert!(app_state.preview_request_key.is_some());
        assert!(app_state.preview_content.is_none());

        let key_after_fail = app_state.preview_request_key.clone().unwrap();

        app_state.sync_preview(&config);
        assert_eq!(
            app_state.preview_request_key.as_ref().unwrap(),
            &key_after_fail
        );
    }
}
