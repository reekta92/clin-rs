use crate::keybinds::{CanvasAction, Keybinds};
use crate::pinstar::state::{PinstarMenuType, PinstarState, PinstarTextField};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui_textarea::{Input, TextArea};

pub fn handle_pinstar_mouse(
    state: &mut PinstarState,
    mouse: MouseEvent,
    area: ratatui::layout::Rect,
    app: &mut crate::app::App,
) -> bool {
    let mut area = area;
    area.height = area.height.saturating_sub(1);
    if state.rename_popup.is_some() {
        return true;
    }

    let (editor_area, canvas_area) = if state.show_editor_pane {
        let main_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(30),
                ratatui::layout::Constraint::Percentage(70),
            ])
            .split(area);
        (Some(main_chunks[0]), main_chunks[1])
    } else {
        (None, area)
    };

    let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
    state.last_mouse_canvas_pos = Some((cx, cy));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Right) => {
            if state.resizing_node_id.is_some() {
                state.resizing_node_id = None;
                state.is_dragging_resize_handle = false;
                let _ = state.save();
                state.sync_to_raw_editor();
                return true;
            }

            if editor_area
                .is_some_and(|rect| crate::events::contains_cell(rect, mouse.column, mouse.row))
                || state
                    .floating_editor_rect
                    .is_some_and(|rect| crate::events::contains_cell(rect, mouse.column, mouse.row))
            {
                return true;
            }

            let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
            state.mouse_dragged = false;
            state.select_rect_start = Some((cx, cy));
            true
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            state.mouse_dragged = true;
            if state.connection_source_id.is_none()
                && state.deleting_connection_source_id.is_none()
                && state.resizing_node_id.is_none()
            {
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                state.mouse_selecting = true;
                state.select_rect_end = Some((cx, cy));
                if let Some(start) = state.select_rect_start {
                    state.select_nodes_in_rect(start.0, start.1, cx, cy);
                }
            }
            true
        }
        MouseEventKind::Up(MouseButton::Right) => {
            if state.mouse_selecting {
                state.mouse_selecting = false;
                state.select_rect_start = None;
                state.select_rect_end = None;
                state.drag_start_pos = None;
                return true;
            }
            if !state.mouse_dragged {
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                // Edge hit takes precedence over node hit
                if state.select_edge_at(cx, cy).is_some() {
                    state.open_edge_context_menu(mouse.column, mouse.row);
                } else {
                    state.select_node_at(cx, cy);
                    if state.selected_node_id.is_none() {
                        state.selected_node_ids.clear();
                        state.selected_edge_id = None;
                    }
                    state.open_context_menu(mouse.column, mouse.row, cx, cy);
                }
            }
            state.drag_start_pos = None;
            true
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            state.last_mouse_pos = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            state.is_panning = false;
            state.last_mouse_pos = None;
            true
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            state.is_panning = true;
            if let Some((lx, ly)) = state.last_mouse_pos {
                let dx = mouse.column as f64 - lx as f64;
                let dy = mouse.row as f64 - ly as f64;
                state.pan(-dx, -dy);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                true
            } else {
                false
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let mut menu_action = None;
            let mut close_menu = false;

            if let Some(menu) = &state.context_menu {
                close_menu = true;
                let menu_width = menu
                    .items
                    .iter()
                    .map(|s| s.len() as u16 + 4)
                    .max()
                    .unwrap_or(25);
                let menu_height = menu.items.len() as u16;
                let menu_x = area.x + menu.x.min(area.width.saturating_sub(menu_width));
                let menu_y = area.y + menu.y.min(area.height.saturating_sub(menu_height));

                if mouse.column >= menu_x
                    && mouse.column < menu_x + menu_width
                    && mouse.row >= menu_y
                    && mouse.row < menu_y + menu_height
                {
                    let selected = (mouse.row - menu_y) as usize;
                    if selected < menu.items.len() {
                        menu_action = Some((selected, menu.menu_type, menu.x, menu.y));
                    }
                }
            }

            if close_menu {
                state.context_menu = None;
            }

            if let Some((selected, menu_type, mx, my)) = menu_action {
                execute_menu_action(state, selected, menu_type, mx, my);
                return true;
            }

            if let Some(floating_area) = state.floating_editor_rect
                && let Some(editor) = &mut state.floating_editor
                && crate::events::contains_cell(floating_area, mouse.column, mouse.row)
            {
                let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(editor);
                crate::events::move_textarea_cursor_to_mouse(
                    editor,
                    floating_area,
                    mouse.column,
                    mouse.row,
                    scroll_row,
                    scroll_col,
                );
                state.mouse_selection.begin(editor);
                state.text_selection_target = Some(PinstarTextField::Floating);
                return true;
            }

            if let Some(editor_area) = editor_area {
                if crate::events::contains_cell(editor_area, mouse.column, mouse.row) {
                    state.editor_focus = true;
                    let digits = state.raw_editor.lines().len().max(1).to_string().len() as u16;
                    let gutter_width = digits + 2;
                    let body_inner = ratatui::layout::Rect::new(
                        editor_area.x + gutter_width,
                        editor_area.y + 1,
                        editor_area.width.saturating_sub(gutter_width + 1),
                        editor_area.height.saturating_sub(1),
                    );
                    let (sr, sc) = crate::ui::get_textarea_scroll(&state.raw_editor);
                    crate::events::move_textarea_cursor_to_mouse(
                        &mut state.raw_editor,
                        body_inner,
                        mouse.column,
                        mouse.row,
                        sr,
                        sc,
                    );
                    state.mouse_selection.begin(&mut state.raw_editor);
                    state.text_selection_target = Some(PinstarTextField::Raw);
                    return true;
                } else {
                    state.editor_focus = false;
                }
            }

            let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);

            if state.connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_connection(&target_id);
                } else {
                    state.connection_source_id = None;
                }
                return true;
            }

            if state.deleting_connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_delete_connection(&target_id);
                } else {
                    state.deleting_connection_source_id = None;
                }
                return true;
            }

            if let Some(resizing_id) = &state.resizing_node_id
                && let Some(node) = state.data.nodes.iter().find(|n| n.id() == resizing_id)
            {
                let (nx, ny) = node.pos();
                let (nw, nh) = node.size();
                let handle_x = nx + nw;
                let handle_y = ny + nh;

                let tolerance = 10.0 / state.zoom;
                if cx >= handle_x - tolerance
                    && cx <= handle_x + tolerance
                    && cy >= handle_y - tolerance
                    && cy <= handle_y + tolerance
                {
                    state.is_dragging_resize_handle = true;
                    state.last_mouse_pos = Some((mouse.column, mouse.row));
                    return true;
                }
            }

            if state.floating_editor.is_some() {
                let prev_selected = state.selected_node_id.clone();
                let hit_node = state.select_node_at(cx, cy);

                if hit_node != prev_selected {
                    state.selected_node_id = prev_selected;
                    state.toggle_editor();
                    state.sync_to_raw_editor();
                    state.selected_node_id = hit_node.clone();

                    if hit_node.is_none() {
                        return true;
                    }
                } else {
                    return true;
                }
            }

            let is_double_click = if let Some((lx, ly, lt)) = state.last_click {
                lx == mouse.column && ly == mouse.row && lt.elapsed().as_millis() < 500
            } else {
                false
            };

            let hit_node = state.select_node_at(cx, cy);

            if is_double_click && hit_node.is_some() {
                state.toggle_editor();
                state.sync_to_raw_editor();
                state.last_click = None;
            } else if hit_node.is_some() {
                state.drag_start_pos = Some((cx, cy));
                state.capture_drag_nodes();
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            } else {
                state.selected_node_ids.clear();
                state.selected_edge_id = None;
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            }

            state.last_mouse_pos = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Left) => {
            state.is_panning = false;
            state.is_dragging_resize_handle = false;
            let notice = match state.text_selection_target.take() {
                Some(PinstarTextField::Raw) => state.mouse_selection.finish(&mut state.raw_editor),
                Some(PinstarTextField::Floating) => state
                    .floating_editor
                    .as_mut()
                    .and_then(|editor| state.mouse_selection.finish(editor)),
                None => None,
            };
            if let Some(notice) = notice {
                app.set_temporary_status(notice);
            }

            if state.drag_start_pos.is_some() {
                state.drag_start_pos = None;
                state.drag_captured_nodes.clear();
                let _ = state.save();
                state.sync_to_raw_editor();
            }
            state.last_mouse_pos = None;
            true
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if state.mouse_selection.active {
                state.mouse_selection.mark_drag();
                match state.text_selection_target {
                    Some(PinstarTextField::Raw) => {
                        if let Some(editor_area) = editor_area {
                            let digits =
                                state.raw_editor.lines().len().max(1).to_string().len() as u16;
                            let gutter_width = digits + 2;
                            let body_inner = ratatui::layout::Rect::new(
                                editor_area.x + gutter_width,
                                editor_area.y + 1,
                                editor_area.width.saturating_sub(gutter_width + 1),
                                editor_area.height.saturating_sub(1),
                            );
                            let (scroll_row, scroll_col) =
                                crate::ui::get_textarea_scroll(&state.raw_editor);
                            crate::events::move_textarea_cursor_to_mouse(
                                &mut state.raw_editor,
                                body_inner,
                                mouse.column,
                                mouse.row,
                                scroll_row,
                                scroll_col,
                            );
                        }
                    }
                    Some(PinstarTextField::Floating) => {
                        if let (Some(editor_area), Some(editor)) =
                            (state.floating_editor_rect, state.floating_editor.as_mut())
                        {
                            let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(editor);
                            crate::events::move_textarea_cursor_to_mouse(
                                editor,
                                editor_area,
                                mouse.column,
                                mouse.row,
                                scroll_row,
                                scroll_col,
                            );
                        }
                    }
                    None => {}
                }
                return true;
            }

            if state.resizing_node_id.is_some()
                && let Some((lx, ly)) = state.last_mouse_pos
            {
                let dw = mouse.column as f64 - lx as f64;
                let dh = mouse.row as f64 - ly as f64;
                state.resize_selected_node(dw / state.zoom, dh / state.zoom);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                state.sync_to_raw_editor();
                return true;
            }

            if let Some(last_pos) = state.drag_start_pos {
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                let dx = cx - last_pos.0;
                let dy = cy - last_pos.1;
                state.move_selected_node(dx, dy);
                state.drag_start_pos = Some((cx, cy));
                if state.show_editor_pane {
                    state.sync_to_raw_editor();
                }
                true
            } else if let Some((lx, ly)) = state.last_mouse_pos {
                state.is_panning = true;
                let dx = mouse.column as f64 - lx as f64;
                let dy = mouse.row as f64 - ly as f64;
                state.pan(-dx, -dy);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                true
            } else {
                false
            }
        }
        MouseEventKind::ScrollUp => {
            if state.show_editor_pane && mouse.column < canvas_area.x {
                state.raw_editor.scroll((-3, 0));
            } else {
                state.zoom_in();
            }
            true
        }
        MouseEventKind::ScrollDown => {
            if state.show_editor_pane && mouse.column < canvas_area.x {
                state.raw_editor.scroll((3, 0));
            } else {
                state.zoom_out();
            }
            true
        }
        _ => false,
    }
}

fn execute_menu_action(
    state: &mut PinstarState,
    selected_index: usize,
    menu_type: PinstarMenuType,
    menu_x: u16,
    menu_y: u16,
) {
    if menu_type == PinstarMenuType::ColorPicker {
        if selected_index == 0 {
            state.set_node_color(None);
        } else if let Some(entry) = crate::pinstar::COLOR_PICKER_PALETTE.get(selected_index - 1) {
            state.set_node_color(Some(entry.1.to_string()));
        }
        state.sync_to_raw_editor();
        return;
    }

    if menu_type == PinstarMenuType::EdgeMenu {
        match selected_index {
            0 => {
                let mut items = vec!["Default".to_string()];
                let mut color_hints: Vec<Option<ratatui::style::Color>> = vec![None];
                for (name, _, color) in crate::pinstar::COLOR_PICKER_PALETTE {
                    let capitalized = name
                        .chars()
                        .next()
                        .unwrap_or(' ')
                        .to_uppercase()
                        .to_string()
                        + &name[1..];
                    items.push(capitalized);
                    color_hints.push(Some(*color));
                }
                state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                    x: menu_x,
                    y: menu_y,
                    selected: 0,
                    items,
                    color_hints,
                    menu_type: PinstarMenuType::EdgeColorPicker,
                });
            }
            1 => {
                state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                    x: menu_x,
                    y: menu_y,
                    selected: 0,
                    items: vec![
                        "Solid".to_string(),
                        "Dashed".to_string(),
                        "Dotted".to_string(),
                    ],
                    color_hints: Vec::new(),
                    menu_type: PinstarMenuType::EdgeStylePicker,
                });
            }
            _ => {}
        }
        return;
    }

    if menu_type == PinstarMenuType::EdgeColorPicker {
        if selected_index == 0 {
            state.set_edge_color(None);
        } else if let Some(entry) = crate::pinstar::COLOR_PICKER_PALETTE.get(selected_index - 1) {
            state.set_edge_color(Some(entry.1.to_string()));
        }
        state.selected_edge_id = None;
        return;
    }

    if menu_type == PinstarMenuType::EdgeStylePicker {
        let style = match selected_index {
            0 => crate::pinstar::data::EdgeStyle::Solid,
            1 => crate::pinstar::data::EdgeStyle::Dashed,
            2 => crate::pinstar::data::EdgeStyle::Dotted,
            _ => return,
        };
        state.set_edge_style(style);
        state.selected_edge_id = None;
        return;
    }

    let node_id = state.selected_node_id.clone();

    if let Some(id) = node_id {
        match selected_index {
            0 => state.start_connection(),
            1 => state.start_delete_connection(),
            2 => {
                let current_title = state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == id)
                    .and_then(|n| n.title())
                    .unwrap_or("")
                    .to_string();
                let mut textarea = TextArea::from(vec![current_title]);
                textarea.set_cursor_line_style(ratatui::style::Style::default());
                textarea.set_block(
                    ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(" Rename Node - Enter to confirm, Esc to cancel "),
                );
                state.rename_popup = Some(textarea);
            }
            3 => state.start_resize(),
            4 => {
                let mut items = vec!["Default".to_string()];
                let mut color_hints: Vec<Option<ratatui::style::Color>> = vec![None];
                for (name, _, color) in crate::pinstar::COLOR_PICKER_PALETTE {
                    let capitalized = name
                        .chars()
                        .next()
                        .unwrap_or(' ')
                        .to_uppercase()
                        .to_string()
                        + &name[1..];
                    items.push(capitalized);
                    color_hints.push(Some(*color));
                }
                state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                    x: menu_x,
                    y: menu_y,
                    selected: 0,
                    items,
                    color_hints,
                    menu_type: PinstarMenuType::ColorPicker,
                });
            }
            5 => state.delete_node_connections(),
            6 => state.delete_selected_node(),
            _ => {}
        }
    } else {
        match selected_index {
            0 => state.add_text_node(state.context_menu_pos.0, state.context_menu_pos.1),
            1 => state.add_group(state.context_menu_pos.0, state.context_menu_pos.1),
            2 => state.add_image_node(state.context_menu_pos.0, state.context_menu_pos.1),
            _ => {}
        }
    }
    state.sync_to_raw_editor();
}

fn handle_direct_action(
    state: &mut PinstarState,
    action: CanvasAction,
    app: &mut crate::app::App,
    area: ratatui::layout::Rect,
) -> bool {
    match action {
        CanvasAction::CreateConnection => {
            if state.selected_node_id.is_some() {
                state.start_connection();
            } else {
                app.set_temporary_status("Select a node first to create a connection");
            }
        }
        CanvasAction::DeleteConnection => {
            if state.selected_node_id.is_some() {
                state.start_delete_connection();
            } else {
                app.set_temporary_status("Select a node first to delete connections");
            }
        }
        CanvasAction::RenameNode => {
            if let Some(id) = state.selected_node_id.clone() {
                let current_title = state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == id)
                    .and_then(|n| n.title())
                    .unwrap_or("")
                    .to_string();
                let mut textarea = TextArea::from(vec![current_title]);
                textarea.set_cursor_line_style(ratatui::style::Style::default());
                textarea.set_block(
                    ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(" Rename Node - Enter to confirm, Esc to cancel "),
                );
                state.rename_popup = Some(textarea);
            } else {
                app.set_temporary_status("Select a node first to rename");
            }
        }
        CanvasAction::ResizeMode => {
            if state.selected_node_id.is_some() {
                state.start_resize();
            } else {
                app.set_temporary_status("Select a node first to resize");
            }
        }
        CanvasAction::SetColor => {
            if state.selected_node_id.is_none()
                && state.selected_edge_id.is_none()
                && state.selected_node_ids.is_empty()
            {
                app.set_temporary_status("Select a node or edge first to set color");
                return true;
            }
            let menu_x = (area.width / 2).saturating_sub(12);
            let menu_y = area.height;
            let mut items = vec!["Default".to_string()];
            let mut color_hints: Vec<Option<ratatui::style::Color>> = vec![None];
            for (name, _, color) in crate::pinstar::COLOR_PICKER_PALETTE {
                let capitalized = name
                    .chars()
                    .next()
                    .unwrap_or(' ')
                    .to_uppercase()
                    .to_string()
                    + &name[1..];
                items.push(capitalized);
                color_hints.push(Some(*color));
            }
            state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                x: menu_x,
                y: menu_y,
                selected: 0,
                items,
                color_hints,
                menu_type: PinstarMenuType::ColorPicker,
            });
        }
        CanvasAction::DeleteNode => {
            state.delete_selected_node();
            state.sync_to_raw_editor();
        }
        CanvasAction::DeleteAllConnections => {
            state.delete_node_connections();
            state.sync_to_raw_editor();
        }
        CanvasAction::AddTextNode => {
            state.add_text_node(state.viewport_x, state.viewport_y);
            state.sync_to_raw_editor();
        }
        CanvasAction::AddGroup => {
            state.add_group(state.viewport_x, state.viewport_y);
            state.sync_to_raw_editor();
        }
        CanvasAction::AddImageNode => {
            state.add_image_node(state.viewport_x, state.viewport_y);
            state.sync_to_raw_editor();
        }
        _ => {}
    }
    true
}

pub fn handle_pinstar_event(
    state: &mut PinstarState,
    key: KeyEvent,
    running: &mut bool,
    area: ratatui::layout::Rect,
    keybinds: &Keybinds,
    app: &mut crate::app::App,
) -> bool {
    let config = &app.config;
    if let Some(textarea) = &mut state.rename_popup {
        state.seq_matcher.clear();
        match key.code {
            _ if keybinds.matches_canvas(CanvasAction::RenameCancel, &key) => {
                state.rename_popup = None;
            }
            _ if keybinds.matches_canvas(CanvasAction::RenameConfirm, &key) => {
                let new_id = textarea.lines().join("");
                state.rename_node(new_id);
                state.rename_popup = None;
            }
            _ => {
                if !apply_text_shortcuts(keybinds, textarea, key) {
                    textarea.input(Input::from(key));
                }
            }
        }
        return true;
    }

    let mut menu_action = None;
    let mut close_menu = false;
    let mut direct_action: Option<CanvasAction> = None;

    if let Some(menu) = &mut state.context_menu {
        state.seq_matcher.clear();
        match key.code {
            _ if keybinds.matches_canvas(CanvasAction::MenuClose, &key) => {
                close_menu = true;
            }
            _ if keybinds.matches_canvas(CanvasAction::MenuUp, &key) => {
                menu.selected = menu.selected.saturating_sub(1);
            }
            _ if keybinds.matches_canvas(CanvasAction::MenuDown, &key) => {
                if menu.selected + 1 < menu.items.len() {
                    menu.selected += 1;
                }
            }
            _ if keybinds.matches_canvas(CanvasAction::MenuSelect, &key) => {
                menu_action = Some((menu.selected, menu.menu_type, menu.x, menu.y));
                close_menu = true;
            }
            _ if keybinds.matches_canvas(CanvasAction::CreateConnection, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::CreateConnection);
            }
            _ if keybinds.matches_canvas(CanvasAction::DeleteConnection, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::DeleteConnection);
            }
            _ if keybinds.matches_canvas(CanvasAction::RenameNode, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::RenameNode);
            }
            _ if keybinds.matches_canvas(CanvasAction::ResizeMode, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::ResizeMode);
            }
            _ if keybinds.matches_canvas(CanvasAction::SetColor, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::SetColor);
            }
            _ if keybinds.matches_canvas(CanvasAction::DeleteNode, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::DeleteNode);
            }
            _ if keybinds.matches_canvas(CanvasAction::DeleteAllConnections, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::DeleteAllConnections);
            }
            _ if keybinds.matches_canvas(CanvasAction::AddTextNode, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::AddTextNode);
            }
            _ if keybinds.matches_canvas(CanvasAction::AddGroup, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::AddGroup);
            }
            _ if keybinds.matches_canvas(CanvasAction::AddImageNode, &key) => {
                close_menu = true;
                direct_action = Some(CanvasAction::AddImageNode);
            }
            _ => {}
        }
    }

    if close_menu {
        state.context_menu = None;
    }

    if let Some((selected, menu_type, mx, my)) = menu_action {
        execute_menu_action(state, selected, menu_type, mx, my);
        return true;
    } else if close_menu && direct_action.is_none() {
        return true;
    }

    if let Some(action) = direct_action {
        return handle_direct_action(state, action, app, area);
    }

    if state.context_menu.is_some() {
        return true;
    }

    if let Some(editor) = &mut state.floating_editor {
        state.seq_matcher.clear();
        match key.code {
            _ if keybinds.matches_canvas(CanvasAction::CloseEditor, &key) => {
                state.toggle_editor();
                state.sync_to_raw_editor();
            }
            _ if keybinds.matches_canvas(CanvasAction::CloseEditorAlt, &key) => {
                state.toggle_editor();
                state.sync_to_raw_editor();
            }
            _ => {
                if !apply_text_shortcuts(keybinds, editor, key) {
                    editor.input(Input::from(key));
                }
                if let Some(node_id) = &state.selected_node_id {
                    let text = editor.lines().join("\n");
                    for node in &mut state.data.nodes {
                        if node.id() == node_id {
                            node.set_text(text);
                            break;
                        }
                    }
                    let _ = state.save();
                }
            }
        }
        return true;
    }

    if state.resizing_node_id.is_some() {
        state.seq_matcher.clear();
        match key.code {
            _ if keybinds.matches_canvas(CanvasAction::ConfirmResize, &key)
                || keybinds.matches_canvas(CanvasAction::CancelResize, &key) =>
            {
                state.resizing_node_id = None;
                state.is_dragging_resize_handle = false;
                let _ = state.save();
                return true;
            }
            _ => {}
        }
    }

    if state.editor_focus {
        state.seq_matcher.clear();
        if keybinds.matches_canvas(CanvasAction::EditorUnfocus, &key) {
            state.editor_focus = false;
        } else {
            if !apply_text_shortcuts(keybinds, &mut state.raw_editor, key) {
                state.raw_editor.input(Input::from(key));
            }
            let _ = state.sync_from_raw_editor();
        }
        return true;
    }

    let seq = config.sequences_enabled();
    let counts = config.counts_enabled();
    if crate::events::is_universal_quit_key(&key) {
        if state.connection_source_id.is_some() {
            state.connection_source_id = None;
        } else {
            *running = false;
        }
        return true;
    }

    match keybinds.resolve_canvas(&mut state.seq_matcher, key, seq, counts) {
        crate::keybinds::MatchOutcome::Matched(action, count) => match action {
            CanvasAction::Quit => {
                if state.connection_source_id.is_some() {
                    state.connection_source_id = None;
                } else {
                    *running = false;
                }
            }
            CanvasAction::Save => {
                let _ = state.save();
            }
            CanvasAction::Undo => {
                let _ = state.undo();
            }
            CanvasAction::Redo => {
                let _ = state.redo();
            }
            CanvasAction::ZoomFineIn => {
                state.zoom_in();
            }
            CanvasAction::ZoomFineOut => {
                state.zoom_out();
            }
            CanvasAction::MoveLeft => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.select_node_in_direction(-1.0, 0.0);
                    state.center_on_selected();
                }
            }
            CanvasAction::MoveRight => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.select_node_in_direction(1.0, 0.0);
                    state.center_on_selected();
                }
            }
            CanvasAction::MoveUp => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.select_node_in_direction(0.0, -1.0);
                    state.center_on_selected();
                }
            }
            CanvasAction::MoveDown => {
                let n = count.unwrap_or(1) as usize;
                for _ in 0..n {
                    state.select_node_in_direction(0.0, 1.0);
                    state.center_on_selected();
                }
            }
            CanvasAction::ZoomIn => {
                state.zoom_in();
            }
            CanvasAction::ZoomOut => {
                state.zoom_out();
            }
            CanvasAction::EditOrConnect => {
                let target_id_opt = state.selected_node_id.clone();
                if let Some(target_id) = target_id_opt {
                    if state.connection_source_id.is_some() {
                        state.finish_connection(&target_id);
                    } else {
                        state.toggle_editor();
                    }
                }
            }
            CanvasAction::OpenContextMenu => {
                let menu_x = (area.width / 2).saturating_sub(12);
                let menu_y = area.height;

                let cx = state.viewport_x;
                let cy = state.viewport_y;

                if let Some(id) = &state.selected_node_id {
                    if state.data.nodes.iter().any(|n| n.id() == id) {
                        state.open_context_menu(menu_x, menu_y, cx, cy);
                    }
                } else {
                    state.open_context_menu(menu_x, menu_y, cx, cy);
                }
            }
            CanvasAction::ToggleGrid => {
                state.show_grid = !state.show_grid;
            }
            CanvasAction::ToggleOrthogonal => {
                state.orthogonal_connections = !state.orthogonal_connections;
                let status = if state.orthogonal_connections {
                    "Orthogonal connections: on"
                } else {
                    "Orthogonal connections: off"
                };
                app.set_temporary_status(status);
                // Persist per-vault
                if let Ok(vault_id) = crate::local_state::vault_identity_path(&app.storage.data_dir)
                {
                    let vault_key = vault_id.to_string_lossy().into_owned();
                    if let Ok(paths) = crate::paths::AppPaths::discover(
                        crate::config::ClinConfig::config_path().unwrap_or_default(),
                    ) {
                        let _ = crate::local_state::LocalState::update(&paths.state_path(), |s| {
                            s.vaults
                                .entry(vault_key.clone())
                                .or_default()
                                .canvas_orthogonal = state.orthogonal_connections;
                            Ok(())
                        });
                    }
                }
            }
            CanvasAction::ToggleEditorPane => {
                state.show_editor_pane = !state.show_editor_pane;
                if !state.show_editor_pane {
                    state.editor_focus = false;
                }
            }
            CanvasAction::CycleFocus => {
                if state.show_editor_pane {
                    state.editor_focus = true;
                }
            }
            CanvasAction::Help => {
                state.help_requested = true;
                *running = false;
            }
            CanvasAction::CreateConnection => {
                if state.selected_node_id.is_some() {
                    state.start_connection();
                } else {
                    app.set_temporary_status("Select a node first to create a connection");
                }
            }
            CanvasAction::DeleteConnection => {
                if state.selected_node_id.is_some() {
                    state.start_delete_connection();
                } else {
                    app.set_temporary_status("Select a node first to delete connections");
                }
            }
            CanvasAction::RenameNode => {
                if let Some(id) = state.selected_node_id.clone() {
                    let current_title = state
                        .data
                        .nodes
                        .iter()
                        .find(|n| n.id() == id)
                        .and_then(|n| n.title())
                        .unwrap_or("")
                        .to_string();
                    let mut textarea = TextArea::from(vec![current_title]);
                    textarea.set_cursor_line_style(ratatui::style::Style::default());
                    textarea.set_block(
                        ratatui::widgets::Block::default()
                            .borders(ratatui::widgets::Borders::ALL)
                            .title(" Rename Node - Enter to confirm, Esc to cancel "),
                    );
                    state.rename_popup = Some(textarea);
                } else {
                    app.set_temporary_status("Select a node first to rename");
                }
            }
            CanvasAction::ResizeMode => {
                if state.selected_node_id.is_some() {
                    state.start_resize();
                } else {
                    app.set_temporary_status("Select a node first to resize");
                }
            }
            CanvasAction::SetColor => {
                if state.selected_node_id.is_none()
                    && state.selected_edge_id.is_none()
                    && state.selected_node_ids.is_empty()
                {
                    app.set_temporary_status("Select a node or edge first to set color");
                } else {
                    let menu_x = (area.width / 2).saturating_sub(12);
                    let menu_y = area.height;
                    let mut items = vec!["Default".to_string()];
                    let mut color_hints: Vec<Option<ratatui::style::Color>> = vec![None];
                    for (name, _, color) in crate::pinstar::COLOR_PICKER_PALETTE {
                        let capitalized = name
                            .chars()
                            .next()
                            .unwrap_or(' ')
                            .to_uppercase()
                            .to_string()
                            + &name[1..];
                        items.push(capitalized);
                        color_hints.push(Some(*color));
                    }
                    state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                        x: menu_x,
                        y: menu_y,
                        selected: 0,
                        items,
                        color_hints,
                        menu_type: PinstarMenuType::ColorPicker,
                    });
                }
            }
            CanvasAction::DeleteNode => {
                state.delete_selected_node();
                state.sync_to_raw_editor();
            }
            CanvasAction::DeleteAllConnections => {
                state.delete_node_connections();
                state.sync_to_raw_editor();
            }
            CanvasAction::AddTextNode => {
                state.add_text_node(state.viewport_x, state.viewport_y);
                state.sync_to_raw_editor();
            }
            CanvasAction::AddGroup => {
                state.add_group(state.viewport_x, state.viewport_y);
                state.sync_to_raw_editor();
            }
            CanvasAction::AddImageNode => {
                state.add_image_node(state.viewport_x, state.viewport_y);
                state.sync_to_raw_editor();
            }
            _ => {
                if keybinds.matches_canvas(CanvasAction::Quit, &key) {
                    *running = false;
                }
            }
        },
        crate::keybinds::MatchOutcome::Pending => return true,
        crate::keybinds::MatchOutcome::NoMatch => return false,
    }

    true
}
