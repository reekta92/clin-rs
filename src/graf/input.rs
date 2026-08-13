use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::graph::{GrafMenuItem, GraphState, ModeBanner};

use crate::config::ClinConfig;
use crate::keybinds::{GraphAction, Keybinds};

#[derive(Debug)]
pub enum GraphInputAction {
    Quit,
    OpenFile(String),
    ToggleHelp,
    ToggleSearch,
    ToggleMinimap,
    ToggleLegend,
    ToggleGrid,
    ToggleStatus,
    Refresh,
    ReloadConfig,
    TogglePreview,
    ToggleLookingGlass,
    MenuAction(GrafMenuItem),
    ConnectionEvent {
        source_id: String,
        target_title: String,
        create: bool,
    },
    ClearFocus,
}

pub fn handle_graph_keys(
    state: &Arc<RwLock<GraphState>>,
    key: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
    seq_matcher: &mut crate::keybinds::KeyMatcher,
    area: Rect,
) -> Option<GraphInputAction> {
    let mut guard = state.write();

    // Context menu open: keys drive the menu exclusively.
    if let Some(menu) = guard.context_menu.as_mut() {
        seq_matcher.clear();
        let mut dispatch: Option<GraphInputAction> = None;
        let mut close = false;

        if keybinds.matches_graph(GraphAction::MenuClose, &key) {
            close = true;
        } else if keybinds.matches_graph(GraphAction::MenuUp, &key) {
            menu.move_up();
        } else if keybinds.matches_graph(GraphAction::MenuDown, &key) {
            menu.move_down();
        } else if keybinds.matches_graph(GraphAction::MenuSelect, &key) {
            if let Some(spec) = menu.items.get(menu.selected)
                && let Some(item) = crate::graf::graph::graf_menu_item_from_label(spec.label)
            {
                dispatch = Some(GraphInputAction::MenuAction(item));
                close = true;
            }
        } else if let KeyCode::Char(c) = key.code
            && let Some(idx) = menu.find_shortcut(c)
            && let Some(spec) = menu.items.get(idx)
            && let Some(item) = crate::graf::graph::graf_menu_item_from_label(spec.label)
        {
            dispatch = Some(GraphInputAction::MenuAction(item));
            close = true;
        }

        if close {
            guard.context_menu = None;
        }
        return dispatch;
    }

    // Escape: cancel connection modes, clear the focus filter (full rebuild),
    // or clear multi-select — before falling through to quit.
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
            return Some(GraphInputAction::ClearFocus);
        }
        if !guard.selection.extra.is_empty() {
            guard.selection.clear_set();
            guard.mode_banner = None;
            return None;
        }
    }

    if crate::events::is_universal_quit_key(&key) {
        return Some(GraphInputAction::Quit);
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_graph(seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            GraphAction::Quit => return Some(GraphInputAction::Quit),
            GraphAction::PanUp | GraphAction::MenuUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    select_in_direction(&mut guard, 0.0, 1.0);
                }
            }
            GraphAction::PanDown | GraphAction::MenuDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    select_in_direction(&mut guard, 0.0, -1.0);
                }
            }
            GraphAction::PanLeft => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    select_in_direction(&mut guard, -1.0, 0.0);
                }
            }
            GraphAction::PanRight | GraphAction::LocalGraph => {
                // `l` is shared with LocalGraph (menu-only); outside the menu it
                // keeps its historical "pan right" meaning.
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    select_in_direction(&mut guard, 1.0, 0.0);
                }
            }
            GraphAction::ZoomIn => {
                guard.viewport.zoom_in(config.graf.interaction.zoom_factor);
            }
            GraphAction::ZoomOut => {
                guard.viewport.zoom_out(config.graf.interaction.zoom_factor);
            }
            GraphAction::OpenNote | GraphAction::MenuSelect => {
                if let Some(idx) = guard.selection.primary
                    && let Some(node) = guard.simulation.get_graph().node_weight(idx)
                {
                    return Some(GraphInputAction::OpenFile(node.data.note_id.clone()));
                }
            }
            GraphAction::AutoFit => {
                let vp = guard
                    .viewport
                    .clone()
                    .auto_fit_from_graph(guard.simulation.get_graph(), 1.4);
                guard.viewport = vp;
            }
            GraphAction::Help => {
                return Some(GraphInputAction::ToggleHelp);
            }
            GraphAction::ToggleSearch => {
                return Some(GraphInputAction::ToggleSearch);
            }
            GraphAction::ToggleMinimap => {
                return Some(GraphInputAction::ToggleMinimap);
            }
            GraphAction::ToggleLegend => {
                return Some(GraphInputAction::ToggleLegend);
            }
            GraphAction::ToggleGrid => {
                return Some(GraphInputAction::ToggleGrid);
            }
            GraphAction::ToggleStatus => {
                return Some(GraphInputAction::ToggleStatus);
            }
            GraphAction::Refresh => {
                return Some(GraphInputAction::Refresh);
            }
            GraphAction::ReloadConfig => {
                return Some(GraphInputAction::ReloadConfig);
            }
            GraphAction::TogglePreview => {
                return Some(GraphInputAction::TogglePreview);
            }
            GraphAction::LookingGlass => {
                return Some(GraphInputAction::ToggleLookingGlass);
            }
            GraphAction::OpenContextMenu => {
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
                return None;
            }
            GraphAction::CreateConnection => {
                return Some(GraphInputAction::MenuAction(GrafMenuItem::CreateConnection));
            }
            GraphAction::DeleteConnection => {
                return Some(GraphInputAction::MenuAction(GrafMenuItem::DeleteConnection));
            }
            GraphAction::ShowGroup => {
                return Some(GraphInputAction::MenuAction(GrafMenuItem::ShowGroup));
            }
            GraphAction::DeleteNode => {
                return Some(GraphInputAction::MenuAction(GrafMenuItem::DeleteNode));
            }
            GraphAction::MenuClose => return Some(GraphInputAction::Quit),
        },
        crate::keybinds::MatchOutcome::Pending => return None,
        crate::keybinds::MatchOutcome::NoMatch => {}
    }

    None
}

#[derive(Default)]
pub struct GraphMouseState {
    pub drag_origin: Option<(u16, u16)>,
    pub is_panning: bool,
    pub last_click_time: Option<Instant>,
    pub last_clicked_node: Option<fdg_sim::petgraph::graph::NodeIndex>,
    pub is_minimap_dragging: bool,
}

pub fn handle_graph_mouse(
    state: &Arc<RwLock<GraphState>>,
    mouse_event: MouseEvent,
    area: Rect,
    mouse_state: &mut GraphMouseState,
    config: &ClinConfig,
) -> Option<GraphInputAction> {
    let canvas = super::render::canvas_area(area, config.ui.show_status_bar);
    let minimap_area = if config.graf.visual.show_minimap {
        Some(super::render::compute_minimap_area(canvas, config))
    } else {
        None
    };

    let in_minimap = minimap_area.is_some_and(|ma| {
        mouse_event.column >= ma.x
            && mouse_event.column < ma.x + ma.width
            && mouse_event.row >= ma.y
            && mouse_event.row < ma.y + ma.height
    });

    let inside_area = mouse_event.column >= canvas.x
        && mouse_event.column < canvas.x + canvas.width
        && mouse_event.row >= canvas.y
        && mouse_event.row < canvas.y + canvas.height;

    match mouse_event.kind {
        MouseEventKind::ScrollUp => {
            if !inside_area {
                return None;
            }
            let mut guard = state.write();
            guard.viewport.zoom_in(config.graf.interaction.zoom_factor);
        }
        MouseEventKind::ScrollDown => {
            if !inside_area {
                return None;
            }
            let mut guard = state.write();
            guard.viewport.zoom_out(config.graf.interaction.zoom_factor);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if !inside_area {
                return None;
            }
            // Context menu: click inside activates a row, click outside dismisses.
            {
                let mut guard = state.write();
                if let Some(menu) = guard.context_menu.take() {
                    let rect = menu.rect(canvas);
                    if let Some(idx) = menu.row_at(rect, mouse_event.column, mouse_event.row)
                        && let Some(spec) = menu.items.get(idx)
                        && let Some(item) =
                            crate::graf::graph::graf_menu_item_from_label(spec.label)
                    {
                        return Some(GraphInputAction::MenuAction(item));
                    }
                    return None;
                }
            }
            // Connection mode: clicking a target completes (or cancels) the link.
            {
                let mut conn_action: Option<GraphInputAction> = None;
                let mut in_conn_mode = false;
                {
                    let guard = state.read();
                    let src_idx = guard.connection_source.or(guard.deleting_connection_source);
                    if src_idx.is_some() {
                        in_conn_mode = true;
                        let create = guard.connection_source.is_some();
                        let source_id = src_idx
                            .and_then(|idx| guard.simulation.get_graph().node_weight(idx))
                            .map(|n| n.data.note_id.clone());
                        let (wx, wy) = guard.viewport.screen_to_world(
                            mouse_event.column,
                            mouse_event.row,
                            canvas,
                        );
                        let max_lc = guard.render_cache.lock().max_link_count;
                        let target_idx = guard
                            .viewport
                            .hit_test(wx, wy, &guard, config, canvas, max_lc);
                        if let (Some(src), Some(source_id), Some(tidx)) =
                            (src_idx, source_id, target_idx)
                            && src != tidx
                            && let Some(target_title) = guard
                                .simulation
                                .get_graph()
                                .node_weight(tidx)
                                .map(|n| n.data.title.clone())
                        {
                            conn_action = Some(GraphInputAction::ConnectionEvent {
                                source_id,
                                target_title,
                                create,
                            });
                        }
                    }
                }
                if in_conn_mode {
                    let mut g = state.write();
                    g.connection_source = None;
                    g.deleting_connection_source = None;
                    g.mode_banner = None;
                    if let Some(a) = conn_action {
                        return Some(a);
                    }
                    return None;
                }
            }
            if in_minimap {
                if let Some(ma) = minimap_area {
                    let world = minimap_screen_to_world(
                        mouse_event.column,
                        mouse_event.row,
                        ma,
                        &state.read(),
                    );
                    let mut guard = state.write();
                    guard.viewport.set_center(world.0, world.1);
                    mouse_state.is_minimap_dragging = true;
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                }
            } else {
                let (wx, wy) = {
                    let guard = state.read();
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, canvas)
                };

                let hit = {
                    let guard = state.read();
                    let max_lc = guard.render_cache.lock().max_link_count;
                    guard
                        .viewport
                        .hit_test(wx, wy, &guard, config, canvas, max_lc)
                };

                let is_double_click = mouse_state
                    .last_click_time
                    .is_some_and(|t| t.elapsed().as_millis() < 300);

                if let Some(node_idx) = hit {
                    let mut guard = state.write();
                    guard.selection.select_only(node_idx);
                    guard.dragging_node = Some(node_idx);
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                    mouse_state.is_panning = false;
                    mouse_state.last_clicked_node = Some(node_idx);

                    if is_double_click
                        && let Some(node) = guard.simulation.get_graph().node_weight(node_idx)
                    {
                        mouse_state.last_click_time = Some(Instant::now());
                        return Some(GraphInputAction::OpenFile(node.data.note_id.clone()));
                    }
                } else {
                    let mut guard = state.write();
                    guard.selection.clear();
                    guard.dragging_node = None;
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                    mouse_state.is_panning = true;
                    mouse_state.last_clicked_node = None;
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let (orig_col, orig_row) = mouse_state.drag_origin?;

            if mouse_state.is_minimap_dragging {
                if let Some(ma) = minimap_area {
                    let world = minimap_screen_to_world(
                        mouse_event.column,
                        mouse_event.row,
                        ma,
                        &state.read(),
                    );
                    let mut guard = state.write();
                    guard.viewport.set_center(world.0, world.1);
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                }
            } else if mouse_state.is_panning {
                let mut guard = state.write();
                let aspect = canvas.width as f64 / canvas.height.max(1) as f64;
                let [xl, xr] = guard.viewport.x_bounds(aspect);
                let [yb, yt] = guard.viewport.y_bounds(aspect);
                let world_per_col = ((xr - xl) / canvas.width.max(1) as f64).abs();
                let world_per_row = ((yt - yb) / canvas.height.max(1) as f64).abs();
                let world_dx = -world_per_col
                    * (mouse_event.column as f64 - orig_col as f64)
                    * config.graf.interaction.drag_sensitivity;
                let world_dy = world_per_row
                    * (mouse_event.row as f64 - orig_row as f64)
                    * config.graf.interaction.drag_sensitivity;
                guard.viewport.pan_by(world_dx, world_dy);
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
            } else {
                let (wx, wy) = {
                    let guard = state.read();
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, canvas)
                };

                let mut guard = state.write();
                if let Some(node_idx) = guard.dragging_node {
                    let graph = guard.simulation.get_graph_mut();
                    if let Some(node) = graph.node_weight_mut(node_idx) {
                        node.location.x = wx as f32;
                        node.location.y = wy as f32;
                        node.velocity = fdg_sim::glam::Vec3::ZERO;
                    }
                    if guard.physics_worker_active {
                        guard.drag_target = Some((wx as f32, wy as f32));
                        guard.reheat(0.4);
                    } else {
                        guard.alpha = 0.0;
                        guard.is_settled = true;
                        let state_ref = &mut *guard;
                        let bounds =
                            super::render::compute_graph_bounds(state_ref.simulation.get_graph());
                        state_ref.graph_bounds = bounds;
                        state_ref
                            .spatial_grid
                            .rebuild(state_ref.simulation.get_graph());
                        state_ref.render_cache.lock().minimap_dirty = true;
                    }
                }
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            {
                let mut guard = state.write();
                guard.dragging_node = None;
                guard.drag_target = None;
            }
            mouse_state.drag_origin = None;
            mouse_state.is_panning = false;
            mouse_state.is_minimap_dragging = false;
            mouse_state.last_click_time = Some(Instant::now());
        }

        MouseEventKind::Down(MouseButton::Right) => {
            if !inside_area {
                return None;
            }
            let (wx, wy) = {
                let guard = state.read();
                guard
                    .viewport
                    .screen_to_world(mouse_event.column, mouse_event.row, canvas)
            };
            let mut guard = state.write();
            guard.right_down_pos = Some((mouse_event.column, mouse_event.row));
            guard.marquee.on_down(wx, wy);
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            let start = {
                let guard = state.read();
                guard.right_down_pos
            };
            let (sx, sy) = start?;
            let dragging = {
                let guard = state.read();
                guard
                    .marquee
                    .is_dragging_screen(mouse_event.column, mouse_event.row, sx, sy)
            };
            if dragging {
                let (wx, wy) = {
                    let guard = state.read();
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, canvas)
                };
                let mut guard = state.write();
                if guard.mode_banner.is_none() {
                    guard.mode_banner = Some(ModeBanner::BoxSelect);
                }
                guard.marquee.on_drag(wx, wy);
                guard.context_menu = None;
            }
        }
        MouseEventKind::Up(MouseButton::Right) => {
            let start_screen = {
                let guard = state.read();
                guard.right_down_pos
            };
            let Some((sx, sy)) = start_screen else {
                let mut g = state.write();
                g.right_down_pos = None;
                g.marquee.clear();
                return None;
            };

            let dragging = {
                let guard = state.read();
                guard
                    .marquee
                    .is_dragging_screen(mouse_event.column, mouse_event.row, sx, sy)
            };

            let mut guard = state.write();
            guard.right_down_pos = None;
            let commit_rect = guard.marquee.commit_rect();
            guard.marquee.clear();

            if !dragging {
                // Click → context menu.
                if guard.selection.extra.is_empty() {
                    let (wx, wy) =
                        guard
                            .viewport
                            .screen_to_world(mouse_event.column, mouse_event.row, canvas);
                    let max_lc = guard.render_cache.lock().max_link_count;
                    if let Some(idx) = guard
                        .viewport
                        .hit_test(wx, wy, &guard, config, canvas, max_lc)
                    {
                        guard.selection.select_only(idx);
                    }
                    guard.open_context_menu(mouse_event.column, mouse_event.row, (wx, wy));
                } else {
                    guard.open_context_menu(mouse_event.column, mouse_event.row, (0.0, 0.0));
                }
            } else if let Some((min_x, min_y, max_x, max_y)) = commit_rect {
                // Box-select commit: collect enclosed nodes.
                let mut enclosed: Vec<fdg_sim::petgraph::graph::NodeIndex> = Vec::new();
                {
                    let graph = guard.simulation.get_graph();
                    for idx in graph.node_indices() {
                        let node = &graph[idx];
                        let nx = node.location.x as f64;
                        let ny = node.location.y as f64;
                        if nx >= min_x && nx <= max_x && ny >= min_y && ny <= max_y {
                            enclosed.push(idx);
                        }
                    }
                }
                let primary = enclosed.first().copied();
                guard
                    .selection
                    .replace_set(enclosed.into_iter().collect(), primary);
                if guard.mode_banner == Some(ModeBanner::BoxSelect) {
                    guard.mode_banner = None;
                }
            }
        }
        _ => {}
    }

    None
}

fn select_in_direction(guard: &mut GraphState, dx: f64, dy: f64) {
    if guard.selection.primary.is_none() {
        let nearest = guard.viewport.nearest_to_center(guard);
        if let Some(idx) = nearest {
            guard.selection.select_only(idx);
            let graph = guard.simulation.get_graph();
            let node = &graph[idx];
            guard
                .viewport
                .center_on_node(node.location.x, node.location.y);
        }
        return;
    }

    let Some(idx) = guard.selection.primary else {
        return;
    };
    let (ox, oy) = {
        let graph = guard.simulation.get_graph();
        let node = &graph[idx];
        (node.location.x as f64, node.location.y as f64)
    };

    if let Some(next) =
        guard
            .viewport
            .nearest_in_direction(guard, ox, oy, dx, dy, guard.selection.primary)
    {
        guard.selection.select_only(next);
        let graph = guard.simulation.get_graph();
        let node = &graph[next];
        guard
            .viewport
            .center_on_node(node.location.x, node.location.y);
    }
}

fn minimap_screen_to_world(
    col: u16,
    row: u16,
    minimap_area: Rect,
    state: &GraphState,
) -> (f64, f64) {
    let (wx_min, wx_max, wy_min, wy_max) = state.graph_bounds;
    let inner_x = minimap_area.x + 1;
    let inner_y = minimap_area.y + 1;
    let inner_w = minimap_area.width.saturating_sub(2);
    let inner_h = minimap_area.height.saturating_sub(2);

    if inner_w == 0 || inner_h == 0 {
        return (0.0, 0.0);
    }

    let rel_x = (col as f64 - inner_x as f64) / inner_w as f64;
    let rel_y = 1.0 - (row as f64 - inner_y as f64) / inner_h as f64;

    let wx = wx_min + rel_x * (wx_max - wx_min);
    let wy = wy_min + rel_y * (wy_max - wy_min);
    (wx, wy)
}
