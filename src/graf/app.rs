use anyhow::Context;
use parking_lot::RwLock;
use std::sync::Arc;

use fdg_sim::petgraph::graph::NodeIndex;

use crossterm::event::KeyCode;

use crate::config::ClinConfig;
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
    pub config_errors: Vec<String>,
    pub search_popup: Option<crate::ui::quick_search::QuickSearch<(NodeIndex, String)>>,
    pub show_minimap: bool,
    pub show_legend: bool,
    pub show_grid: bool,
    pub show_status_bar: bool,
    pub config_reload_msg: Option<String>,

    pub preview_enabled: bool,
    pub preview_content: Option<PreviewContent>,
    pub preview_note_id: Option<String>,
    pub last_preview_pane_width: u16,
    pub last_preview_pane_height: u16,
    pub preview_content_width: Option<u16>,
    pub preview_content_height: Option<u16>,
    pub preview_scale: f64,
    pub preview_content_scale: Option<f64>,
    pub app_theme: crate::app_theme::AppThemeColors,
    pub preview_offset_x: f64,
    pub preview_offset_y: f64,
    pub preview_content_offset_x: Option<f64>,
    pub preview_content_offset_y: Option<f64>,
    pub keybinds: Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub mouse_pos: Option<(u16, u16)>,
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
        config_errors: Vec<String>,
        keybinds: Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> anyhow::Result<Self> {
        let graph_state = crate::graf::graph::GraphState::new(&storage, config)?;
        let state = Arc::new(RwLock::new(graph_state));
        let (kill_tx, kill_rx) = std::sync::mpsc::channel();
        crate::graf::physics::start_physics(state.clone(), config, kill_rx);

        Ok(Self {
            graph_state: Some(state),
            graph_kill_tx: Some(kill_tx),
            graph_mouse_state: GraphMouseState::default(),
            storage,

            config_errors,
            search_popup: None,
            show_minimap: config.graf.visual.show_minimap,
            show_legend: config.graf.visual.show_legend,
            show_grid: config.graf.visual.show_grid,
            show_status_bar: config.ui.show_status_bar,
            config_reload_msg: None,

            preview_enabled: config.graf.preview_enabled,
            preview_content: None,
            preview_note_id: None,
            last_preview_pane_width: 0,
            last_preview_pane_height: 0,
            preview_content_width: None,
            preview_content_height: None,
            preview_scale: 1.0,
            preview_content_scale: None,
            preview_offset_x: 0.0,
            preview_offset_y: 0.0,
            preview_content_offset_x: None,
            preview_content_offset_y: None,
            app_theme: crate::app_theme::AppThemeColors::from_config(&config.ui),
            keybinds,
            seq_matcher,
            mouse_pos: None,
        })
    }

    pub fn refresh_simulation(&mut self, config: &ClinConfig) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        if let Ok(graph_state) = crate::graf::graph::GraphState::new(&self.storage, config) {
            let state = Arc::new(RwLock::new(graph_state));
            let (kill_tx, kill_rx) = std::sync::mpsc::channel();
            crate::graf::physics::start_physics(state.clone(), config, kill_rx);
            self.graph_state = Some(state);
            self.graph_kill_tx = Some(kill_tx);
            self.search_popup = None;
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        self.graph_state = None;
    }

    pub fn poll_renderers(&mut self) -> bool {
        if let Some(PreviewContent::Markdown(renderer)) = &mut self.preview_content {
            if renderer.poll() {
                if !renderer.pages_built() {
                    let visible = 34u16;
                    renderer.build_pages(visible, self.app_theme.preview_bg());
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn sync_preview(&mut self, config: &ClinConfig) {
        if !self.preview_enabled {
            return;
        }
        let selected_note_id = if let Some(gs) = &self.graph_state {
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
        };

        let size_changed = self.preview_content_width != Some(self.last_preview_pane_width)
            || self.preview_content_height != Some(self.last_preview_pane_height)
            || self.preview_content_scale != Some(self.preview_scale)
            || self.preview_content_offset_x != Some(self.preview_offset_x)
            || self.preview_content_offset_y != Some(self.preview_offset_y);

        if selected_note_id != self.preview_note_id || size_changed {
            self.preview_note_id = selected_note_id;
            self.update_preview(config);
        }
    }

    pub fn update_preview(&mut self, config: &ClinConfig) {
        if !self.preview_enabled {
            self.preview_content = None;
            return;
        }

        let Some(id) = self.preview_note_id.clone() else {
            self.preview_content = None;
            return;
        };

        let is_draw = id.ends_with(".draw");
        let is_canvas = id.ends_with(".canvas");
        let is_clin = id.ends_with(".clin");

        if config.list.preview_encryption && is_clin {
            self.preview_content = None;
            return;
        }

        if is_draw {
            let path = self.storage.note_path(&id);
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<crate::draw::state::DrawData>(&content) {
                        Ok(data) => {
                            let width = self.last_preview_pane_width;
                            let height = self.last_preview_pane_height;
                            let scale = self.preview_scale;
                            let offset_x = self.preview_offset_x;
                            let offset_y = self.preview_offset_y;
                            let grid = crate::snapshot::render_draw_snapshot_with_size(
                                &data,
                                &self.app_theme,
                                width,
                                height,
                                scale,
                                offset_x,
                                offset_y,
                            );
                            self.preview_content = Some(PreviewContent::DrawGrid(grid));
                            self.preview_content_width = Some(width);
                            self.preview_content_height = Some(height);
                            self.preview_content_scale = Some(scale);
                            self.preview_content_offset_x = Some(offset_x);
                            self.preview_content_offset_y = Some(offset_y);
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
            let path = self.storage.note_path(&id);
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content) {
                        Ok(data) => {
                            let width = self.last_preview_pane_width;
                            let height = self.last_preview_pane_height;
                            let scale = self.preview_scale;
                            let offset_x = self.preview_offset_x;
                            let offset_y = self.preview_offset_y;
                            let grid = crate::snapshot::render_canvas_snapshot(
                                &data,
                                &self.app_theme,
                                width,
                                height,
                                scale,
                                offset_x,
                                offset_y,
                            );
                            self.preview_content = Some(PreviewContent::CanvasGrid(grid));
                            self.preview_content_width = Some(width);
                            self.preview_content_height = Some(height);
                            self.preview_content_scale = Some(scale);
                            self.preview_content_offset_x = Some(offset_x);
                            self.preview_content_offset_y = Some(offset_y);
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

        if let Ok(note) = self.storage.load_note(&id) {
            let width = self.last_preview_pane_width.saturating_sub(2).max(40);
            let mut renderer = MarkdownRenderer::new(width);
            let opts = crate::markdown::MdRenderOpts::from_config(config);
            renderer.render_with(&note.content, width, &self.app_theme, &opts);
            self.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
            self.preview_content_width = Some(width);
            self.preview_content_height = Some(self.last_preview_pane_height);
            self.preview_content_scale = Some(self.preview_scale);
            self.preview_content_offset_x = Some(self.preview_offset_x);
            self.preview_content_offset_y = Some(self.preview_offset_y);
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
        let _ = self.poll_renderers();
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
        terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let keybinds = self.keybinds.clone();
        if let Some(action) = handle_event(event, self, &app.config, &keybinds, terminal)? {
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

fn handle_event(
    ev: crossterm::event::Event,
    app_state: &mut GrafAppState,
    config: &crate::config::ClinConfig,
    keybinds: &Keybinds,
    terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
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
            if let Some(graph_state) = &app_state.graph_state
                && let Some(action) = crate::graf::input::handle_graph_keys(
                    graph_state,
                    key,
                    keybinds,
                    config,
                    &mut app_state.seq_matcher,
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
                let size = terminal.size().context("failed to get terminal size")?;
                let full_area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
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
                let size = terminal.size().context("failed to get terminal size")?;
                let full_area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                let outer = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(full_area);
                let content_area = outer[1];
                let graph_area = if app_state.preview_enabled {
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

                if let Some(action) = crate::graf::input::handle_graph_mouse(
                    graph_state,
                    mouse_event,
                    graph_area,
                    &mut app_state.graph_mouse_state,
                    config,
                ) {
                    use crate::graf::input::GraphInputAction;
                    if let GraphInputAction::OpenFile(path) = action {
                        return Ok(Some(EventAction::OpenFile(path)));
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
