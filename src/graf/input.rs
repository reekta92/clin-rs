use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::graph::GraphState;
use super::viewport::CELL_ASPECT;
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
}

pub fn handle_graph_keys(
    state: &Arc<RwLock<GraphState>>,
    key: KeyEvent,
    keybinds: &Keybinds,
    config: &ClinConfig,
    seq_matcher: &mut crate::keybinds::KeyMatcher,
) -> Option<GraphInputAction> {
    let mut guard = state.write();

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    match keybinds.resolve_graph(seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            GraphAction::Quit => return Some(GraphInputAction::Quit),
            GraphAction::PanUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    select_in_direction(&mut guard, 0.0, 1.0);
                }
            }
            GraphAction::PanDown => {
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
            GraphAction::PanRight => {
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
            GraphAction::OpenNote => {
                if let Some(idx) = guard.selected_node
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
    let minimap_area = if config.graf.visual.show_minimap {
        Some(super::render::compute_minimap_area(area, config))
    } else {
        None
    };

    let in_minimap = minimap_area.is_some_and(|ma| {
        mouse_event.column >= ma.x
            && mouse_event.column < ma.x + ma.width
            && mouse_event.row >= ma.y
            && mouse_event.row < ma.y + ma.height
    });

    let inside_area = mouse_event.column >= area.x
        && mouse_event.column < area.x + area.width
        && mouse_event.row >= area.y
        && mouse_event.row < area.y + area.height;

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
            if in_minimap {
                if let Some(ma) = minimap_area {
                    let world = minimap_screen_to_world(
                        mouse_event.column,
                        mouse_event.row,
                        ma,
                        &state.read(),
                    );
                    let mut guard = state.write();
                    guard.viewport.center_x = world.0;
                    guard.viewport.center_y = world.1;
                    mouse_state.is_minimap_dragging = true;
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                }
            } else {
                let (wx, wy) = {
                    let guard = state.read();
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, area)
                };

                let hit = {
                    let guard = state.read();
                    guard.viewport.hit_test(wx, wy, &guard)
                };

                let is_double_click = mouse_state
                    .last_click_time
                    .is_some_and(|t| t.elapsed().as_millis() < 300);

                if let Some(node_idx) = hit {
                    let mut guard = state.write();
                    guard.selected_node = Some(node_idx);
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
                    if is_double_click {
                        guard.selected_node = None;
                    }
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
                    guard.viewport.center_x = world.0;
                    guard.viewport.center_y = world.1;
                    mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                }
            } else if mouse_state.is_panning {
                let mut guard = state.write();
                let dx_col = -(mouse_event.column as f64 - orig_col as f64);
                let dy_row = mouse_event.row as f64 - orig_row as f64;
                let vp = &guard.viewport;
                let world_dx = dx_col * 200.0 / (vp.zoom * area.width as f64)
                    * config.graf.interaction.drag_sensitivity;
                let world_dy = dy_row * 200.0 * CELL_ASPECT / (vp.zoom * area.height as f64)
                    * config.graf.interaction.drag_sensitivity;
                guard.viewport.center_x += world_dx;
                guard.viewport.center_y += world_dy;
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
            } else {
                let (wx, wy) = {
                    let guard = state.read();
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, area)
                };

                let mut guard = state.write();
                if let Some(node_idx) = guard.dragging_node {
                    let graph = guard.simulation.get_graph_mut();
                    if let Some(node) = graph.node_weight_mut(node_idx) {
                        node.location.x = wx as f32;
                        node.location.y = wy as f32;
                        node.velocity = fdg_sim::glam::Vec3::ZERO;
                    }
                    guard.drag_target = Some((wx as f32, wy as f32));
                    guard.is_settled = false;
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
        _ => {}
    }

    None
}

fn select_in_direction(guard: &mut GraphState, dx: f64, dy: f64) {
    if guard.selected_node.is_none() {
        guard.selected_node = guard.viewport.nearest_to_center(guard);
        if let Some(idx) = guard.selected_node {
            let graph = guard.simulation.get_graph();
            let node = &graph[idx];
            guard
                .viewport
                .center_on_node(node.location.x, node.location.y);
        }
        return;
    }

    let Some(idx) = guard.selected_node else {
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
            .nearest_in_direction(guard, ox, oy, dx, dy, guard.selected_node)
    {
        guard.selected_node = Some(next);
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
