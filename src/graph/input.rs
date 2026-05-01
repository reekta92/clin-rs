use std::sync::{Arc, RwLock};

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::GraphState;

pub fn handle_graph_keys(app: &mut crate::app::App, key: KeyEvent) -> bool {
    let keybinds = &app.keybinds;

    if keybinds.matches_graph(crate::keybinds::GraphAction::Quit, &key) {
        app.close_graph_view();
        return false;
    }
    if keybinds.matches_graph(crate::keybinds::GraphAction::Help, &key) {
        app.open_help_page();
        return false;
    }

    if app.graph_state.is_none() {
        return false;
    }

    if keybinds.matches_graph(crate::keybinds::GraphAction::PanUp, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.pan_up();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::PanDown, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.pan_down();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::PanLeft, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.pan_left();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::PanRight, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.pan_right();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::ZoomIn, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.zoom_in();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::ZoomOut, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.viewport.zoom_out();
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::OpenNote, &key) {
        let note_id = {
            let state = app.graph_state.as_ref().unwrap();
            let guard = state.read().unwrap_or_else(|e| e.into_inner());
            guard.selected_node.and_then(|idx| {
                guard
                    .simulation
                    .get_graph()
                    .node_weight(idx)
                    .map(|n| n.data.note_id.clone())
            })
        };
        if let Some(id) = note_id {
            app.open_note_from_graph(&id);
        }
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::ToggleLabels, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        guard.show_labels = !guard.show_labels;
    } else if keybinds.matches_graph(crate::keybinds::GraphAction::AutoFit, &key) {
        let state = app.graph_state.as_ref().unwrap();
        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
        let viewport = guard.viewport.clone();
        let vp = viewport.auto_fit_from_graph(guard.simulation.get_graph());
        guard.viewport = vp;
    }

    false
}

#[derive(Default)]
pub struct GraphMouseState {
    pub drag_origin: Option<(u16, u16)>,
    pub is_panning: bool,
}

pub fn handle_graph_mouse(
    state: &Arc<RwLock<GraphState>>,
    mouse_event: MouseEvent,
    area: Rect,
    mouse_state: &mut GraphMouseState,
) {
    match mouse_event.kind {
        MouseEventKind::ScrollUp => {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            guard.viewport.zoom_in();
        }
        MouseEventKind::ScrollDown => {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            guard.viewport.zoom_out();
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let (wx, wy) = {
                let guard = state.read().unwrap_or_else(|e| e.into_inner());
                guard
                    .viewport
                    .screen_to_world(mouse_event.column, mouse_event.row, area)
            };

            let hit = {
                let guard = state.read().unwrap_or_else(|e| e.into_inner());
                guard.viewport.hit_test(wx, wy, &guard)
            };

            if let Some(node_idx) = hit {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.selected_node = Some(node_idx);
                guard.dragging_node = Some(node_idx);
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                mouse_state.is_panning = false;
            } else {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.selected_node = None;
                guard.dragging_node = None;
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
                mouse_state.is_panning = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let Some((orig_col, orig_row)) = mouse_state.drag_origin else {
                return;
            };

            if mouse_state.is_panning {
                let dx = (mouse_event.column as f64 - orig_col as f64) * 0.5;
                let dy = -(mouse_event.row as f64 - orig_row as f64) * 0.5;
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.viewport.pan(-dx, dy);
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
            } else {
                let (wx, wy) = {
                    let guard = state.read().unwrap_or_else(|e| e.into_inner());
                    guard
                        .viewport
                        .screen_to_world(mouse_event.column, mouse_event.row, area)
                };

                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                if let Some(node_idx) = guard.dragging_node {
                    let graph = guard.simulation.get_graph_mut();
                    if let Some(node) = graph.node_weight_mut(node_idx) {
                        node.location.x = wx as f32;
                        node.location.y = wy as f32;
                        node.velocity = fdg_sim::glam::Vec3::ZERO;
                    }
                    guard.is_settled = false;
                }
                mouse_state.drag_origin = Some((mouse_event.column, mouse_event.row));
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            {
                let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                guard.dragging_node = None;
            }
            mouse_state.drag_origin = None;
            mouse_state.is_panning = false;
        }
        _ => {}
    }
}
