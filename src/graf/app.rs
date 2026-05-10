use std::sync::Arc;
use std::sync::RwLock;

use fdg_sim::petgraph::graph::NodeIndex;

use crate::config::ClinConfig;
use crate::graph::input::GraphMouseState;
use crate::keybinds::Keybinds;
use crate::storage::Storage;

pub struct GrafAppState {
    pub graph_state: Option<Arc<RwLock<crate::graph::GraphState>>>,
    pub graph_kill_tx: Option<std::sync::mpsc::Sender<()>>,
    pub graph_mouse_state: GraphMouseState,
    pub storage: Storage,
    pub config_errors: Vec<String>,
    pub search_active: bool,
    pub search_query: String,
    pub search_results: Vec<(NodeIndex, String)>,
    pub search_selected: usize,
    pub search_cursor: usize,
    pub show_minimap: bool,
    pub show_legend: bool,
    pub show_grid: bool,
    pub show_status_bar: bool,
    pub config_reload_msg: Option<String>,
}

impl GrafAppState {
    pub fn new(config: &ClinConfig, storage: Storage, config_errors: Vec<String>) -> anyhow::Result<Self> {
        let graph_state = crate::graph::GraphState::new(&storage, config)?;
        let state = Arc::new(RwLock::new(graph_state));
        let (kill_tx, kill_rx) = std::sync::mpsc::channel();
        crate::graph::physics::start_physics(state.clone(), config, kill_rx);

        Ok(Self {
            graph_state: Some(state),
            graph_kill_tx: Some(kill_tx),
            graph_mouse_state: GraphMouseState::default(),
            storage,

            config_errors,
            search_active: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_cursor: 0,
            show_minimap: config.visual.show_minimap,
            show_legend: config.visual.show_legend,
            show_grid: config.visual.show_grid,
            show_status_bar: config.display.show_status_bar,
            config_reload_msg: None,
        })
    }

    pub fn refresh_simulation(&mut self, config: &ClinConfig) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        if let Ok(graph_state) = crate::graph::GraphState::new(&self.storage, config) {
            let state = Arc::new(RwLock::new(graph_state));
            let (kill_tx, kill_rx) = std::sync::mpsc::channel();
            crate::graph::physics::start_physics(state.clone(), config, kill_rx);
            self.graph_state = Some(state);
            self.graph_kill_tx = Some(kill_tx);
            self.search_results.clear();
            self.search_selected = 0;
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(kill_tx) = self.graph_kill_tx.take() {
            let _ = kill_tx.send(());
        }
        self.graph_state = None;
    }
}

pub enum EventAction {
    Quit,
    OpenFile(String),
    OpenHelp,
}

pub enum GrafResult {
    NoteOpened(String),
    Quit,
    OpenHelp,
}

pub fn run_graf_view(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    storage: crate::storage::Storage,
    config: &mut crate::config::ClinConfig,
    keybinds: &Keybinds,
) -> anyhow::Result<GrafResult> {
    let mut app_state = GrafAppState::new(config, storage, vec![])?;
    let mut running = true;
    let mut result = GrafResult::Quit;

    while running {
        terminal.draw(|frame| {
            crate::graf::ui::draw_ui(frame, &app_state, config, keybinds);
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(16))? {
            loop {
                let ev = crossterm::event::read()?;
                if let Some(action) = handle_event(ev, &mut app_state, config, keybinds, terminal)? {
                    match action {
                        EventAction::Quit => {
                            app_state.shutdown();
                            result = GrafResult::Quit;
                            running = false;
                        }
                        EventAction::OpenFile(id) => {
                            app_state.shutdown();
                            result = GrafResult::NoteOpened(id);
                            running = false;
                        }
                        EventAction::OpenHelp => {
                            app_state.shutdown();
                            result = GrafResult::OpenHelp;
                            running = false;
                        }
                    }
                }

                if !running || !crossterm::event::poll(std::time::Duration::from_millis(0))? {
                    break;
                }
            }
        }
    }
    Ok(result)
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
            if app_state.search_active {
                handle_search_keys(app_state, key, config);
                return Ok(None);
            }

            if let Some(graph_state) = &app_state.graph_state
                && let Some(action) = crate::graph::input::handle_graph_keys(graph_state, key, keybinds, config)
            {
                use crate::graph::input::GraphInputAction;
                match action {
                    GraphInputAction::Quit => return Ok(Some(EventAction::Quit)),
                    GraphInputAction::ToggleHelp => {
                        return Ok(Some(EventAction::OpenHelp));
                    }
                    GraphInputAction::ToggleSearch => {
                        app_state.search_active = true;
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
                }
            }
            Ok(None)
        }
        crossterm::event::Event::Mouse(mouse_event) => {
            if app_state.search_active {
                return Ok(None);
            }
            if let Some(graph_state) = &app_state.graph_state {
                let size = terminal.size().unwrap();
                let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                if let Some(action) = crate::graph::input::handle_graph_mouse(
                    graph_state,
                    mouse_event,
                    area,
                    &mut app_state.graph_mouse_state,
                    config,
                ) {
                    use crate::graph::input::GraphInputAction;
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
) {
    use crossterm::event::{KeyCode, KeyModifiers};

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Esc => {
            app_state.search_active = false;
            app_state.search_query.clear();
            app_state.search_results.clear();
            app_state.search_selected = 0;
            app_state.search_cursor = 0;
        }
        KeyCode::Enter => {
            if let Some(&(idx, _)) = app_state.search_results.get(app_state.search_selected) {
                let (nx, ny) = if let Some(graph_state) = &app_state.graph_state {
                    let guard = graph_state.read().unwrap_or_else(|e| e.into_inner());
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
                    let mut guard = graph_state.write().unwrap_or_else(|e| e.into_inner());
                    guard.selected_node = Some(idx);
                    guard.viewport.center_on_node(nx as f32, ny as f32);
                }
            }
            app_state.search_active = false;
            app_state.search_query.clear();
            app_state.search_results.clear();
            app_state.search_selected = 0;
            app_state.search_cursor = 0;
        }
        KeyCode::Up => {
            if app_state.search_selected > 0 {
                app_state.search_selected -= 1;
            }
        }
        KeyCode::Down => {
            if !app_state.search_results.is_empty()
                && app_state.search_selected < app_state.search_results.len() - 1
            {
                app_state.search_selected += 1;
            }
        }
        KeyCode::BackTab | KeyCode::Tab if shift => {
            if !app_state.search_results.is_empty() {
                app_state.search_selected = app_state
                    .search_selected
                    .checked_sub(1)
                    .unwrap_or(app_state.search_results.len() - 1);
            }
        }
        KeyCode::Tab => {
            if !app_state.search_results.is_empty() {
                app_state.search_selected =
                    (app_state.search_selected + 1) % app_state.search_results.len();
            }
        }
        KeyCode::Backspace => {
            if app_state.search_cursor > 0 {
                let prev = app_state.search_query[..app_state.search_cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                app_state
                    .search_query
                    .replace_range(prev..app_state.search_cursor, "");
                app_state.search_cursor = prev;
                run_search(app_state, config);
            }
        }
        KeyCode::Delete => {
            if app_state.search_cursor < app_state.search_query.len() {
                let next = app_state.search_query[app_state.search_cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app_state.search_cursor + i)
                    .unwrap_or(app_state.search_query.len());
                app_state
                    .search_query
                    .replace_range(app_state.search_cursor..next, "");
                run_search(app_state, config);
            }
        }
        KeyCode::Left => {
            if app_state.search_cursor > 0 {
                app_state.search_cursor = app_state.search_query[..app_state.search_cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        }
        KeyCode::Right => {
            if app_state.search_cursor < app_state.search_query.len() {
                app_state.search_cursor = app_state.search_query[app_state.search_cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app_state.search_cursor + i)
                    .unwrap_or(app_state.search_query.len());
            }
        }
        KeyCode::Home => {
            app_state.search_cursor = 0;
        }
        KeyCode::End => {
            app_state.search_cursor = app_state.search_query.len();
        }
        KeyCode::Char('h') if ctrl => {
            delete_word_back(app_state);
            run_search(app_state, config);
        }
        KeyCode::Char('w') if ctrl => {
            delete_word_back(app_state);
            run_search(app_state, config);
        }
        KeyCode::Char('u') if ctrl => {
            app_state.search_query.clear();
            app_state.search_cursor = 0;
            run_search(app_state, config);
        }
        KeyCode::Char('a') if ctrl => {
            app_state.search_cursor = 0;
        }
        KeyCode::Char('e') if ctrl => {
            app_state.search_cursor = app_state.search_query.len();
        }
        KeyCode::Char(c) if !ctrl => {
            const MAX_SEARCH_LEN: usize = 256;
            if app_state.search_query.len() < MAX_SEARCH_LEN {
                app_state.search_query.insert(app_state.search_cursor, c);
                app_state.search_cursor += c.len_utf8();
                run_search(app_state, config);
            }
        }
        _ => {}
    }
}

fn delete_word_back(app_state: &mut GrafAppState) {
    if app_state.search_cursor == 0 {
        return;
    }
    let query = &app_state.search_query[..app_state.search_cursor];
    let trimmed = query.trim_end_matches(|c: char| c.is_whitespace());
    let cut_to = trimmed
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    app_state
        .search_query
        .replace_range(cut_to..app_state.search_cursor, "");
    app_state.search_cursor = cut_to;
}

fn run_search(app_state: &mut GrafAppState, config: &crate::config::ClinConfig) {
    if let Some(graph_state) = &app_state.graph_state {
        let guard = graph_state.read().unwrap_or_else(|e| e.into_inner());
        app_state.search_results = crate::graph::search_nodes(
            &guard.simulation,
            &app_state.search_query,
            config.search.max_results,
        );
    }
    app_state.search_selected = 0;
}
