use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use crate::pinstar::state::{PinstarState, PinstarMenuType};
use ratatui_textarea::{Input, TextArea};

pub fn handle_pinstar_mouse(state: &mut PinstarState, mouse: MouseEvent, area: ratatui::layout::Rect) -> bool {
    // If rename popup is open, it captures all clicks (blocking background interaction)
    if state.rename_popup.is_some() {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            // Check if clicked outside? For now just block.
        }
        return true;
    }

    // Replicate dynamic layout from render.rs
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
            // If in resize mode, Right Click saves and exits
            if state.resizing_node_id.is_some() {
                state.resizing_node_id = None;
                let _ = state.save();
                return true;
            }

            if let Some(editor_area) = editor_area {
                if crate::events::contains_cell(editor_area, mouse.column, mouse.row) {
                    state.open_editor_context_menu(mouse.column, mouse.row);
                    return true;
                }
            }

            let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
            state.select_node_at(cx, cy);
            state.open_context_menu(mouse.column, mouse.row, cx, cy);
            true
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            state.last_mouse_pos = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            state.last_mouse_pos = None;
            true
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
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
            // Check if we clicked the EXT button in the status line (bottom row, left side)
            if mouse.row == area.bottom().saturating_sub(1) && mouse.column < 10 {
                state.ext_editor_enabled = !state.ext_editor_enabled;
                state.ext_focused = true;
                return true;
            }
            state.ext_focused = false;
            // 1. Check if context menu is open and if we clicked it
            let mut menu_action = None;
            let mut close_menu = false;
            
            if let Some(menu) = &state.context_menu {
                close_menu = true;
                let menu_width = 25;
                let menu_height = menu.items.len() as u16;
                
                if mouse.column >= menu.x && mouse.column < menu.x + menu_width &&
                   mouse.row >= menu.y && mouse.row < menu.y + menu_height {
                    
                    let selected = (mouse.row - menu.y) as usize;
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

            if let Some(editor_area) = editor_area {
                if crate::events::contains_cell(editor_area, mouse.column, mouse.row) {
                    state.editor_focus = true;
                    let digits = state.raw_editor.lines().len().max(1).to_string().len() as u16;
                    let gutter_width = digits + 1;
                    let body_inner = ratatui::layout::Rect::new(
                        editor_area.x + gutter_width,
                        editor_area.y + 1,
                        editor_area.width.saturating_sub(gutter_width),
                        editor_area.height.saturating_sub(1),
                    );
                    crate::events::move_textarea_cursor_to_mouse(&mut state.raw_editor, body_inner, mouse.column, mouse.row);
                    state.raw_editor.start_selection();
                    state.mouse_selecting = true;
                    state.mouse_dragged = false;
                    return true;
                } else {
                    state.editor_focus = false;
                }
            }

            let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);

            // 2. If in connection mode
            if state.connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_connection(&target_id);
                } else {
                    state.connection_source_id = None;
                }
                return true;
            }

            // 3. If in delete connection mode
            if state.deleting_connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_delete_connection(&target_id);
                } else {
                    state.deleting_connection_source_id = None;
                }
                return true;
            }

            // 4. If in resize mode
            if let Some(resizing_id) = &state.resizing_node_id {
                // Check if hit the handle (bottom right corner)
                if let Some(node) = state.data.nodes.iter().find(|n| n.id() == resizing_id) {
                    let (nx, ny) = node.pos();
                    let (nw, nh) = node.size();
                    let handle_x = nx + nw;
                    let handle_y = ny + nh;
                    
                    // Tolerance for "grabbing" the handle in canvas coords
                    let tolerance = 10.0 / state.zoom;
                    if cx >= handle_x - tolerance && cx <= handle_x + tolerance &&
                       cy >= handle_y - tolerance && cy <= handle_y + tolerance {
                        state.is_dragging_resize_handle = true;
                        state.last_mouse_pos = Some((mouse.column, mouse.row));
                        return true;
                    }
                }
            }

            // 5. If editing, check for cancel (click outside)
            if state.floating_editor.is_some() {
                let prev_selected = state.selected_node_id.clone();
                let hit_node = state.select_node_at(cx, cy);
                
                if hit_node != prev_selected {
                    // Temporarily restore previous selection so toggle_editor saves to the right node
                    state.selected_node_id = prev_selected;
                    state.toggle_editor();
                    // Apply the new selection
                    state.selected_node_id = hit_node.clone();
                    
                    if hit_node.is_none() {
                        return true; // Clicked empty space, consumed.
                    }
                } else {
                    return true; // Clicked inside the same node
                }
            }

            let is_double_click = if let Some((lx, ly, lt)) = state.last_click {
                lx == mouse.column && ly == mouse.row && lt.elapsed().as_millis() < 500
            } else {
                false
            };

            // Hit test for nodes
            let hit_node = state.select_node_at(cx, cy);

            if is_double_click && hit_node.is_some() {
                state.toggle_editor();
                state.last_click = None; 
            } else if let Some(_) = hit_node {
                state.drag_start_pos = Some((cx, cy));
                state.capture_drag_nodes();
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            } else {
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            }

            state.last_mouse_pos = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if state.mouse_selecting && !state.mouse_dragged {
                state.raw_editor.cancel_selection();
            }
            state.mouse_selecting = false;
            state.mouse_dragged = false;

            if state.drag_start_pos.is_some() {
                state.drag_start_pos = None;
                state.drag_captured_nodes.clear();
                let _ = state.save();
            }
            state.last_mouse_pos = None;
            true
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if state.mouse_selecting {
                state.mouse_dragged = true;
                if let Some(editor_area) = editor_area {
                    let digits = state.raw_editor.lines().len().max(1).to_string().len() as u16;
                    let gutter_width = digits + 1;
                    let body_inner = ratatui::layout::Rect::new(
                        editor_area.x + gutter_width,
                        editor_area.y + 1,
                        editor_area.width.saturating_sub(gutter_width),
                        editor_area.height.saturating_sub(1),
                    );
                    crate::events::move_textarea_cursor_to_mouse(&mut state.raw_editor, body_inner, mouse.column, mouse.row);
                    return true;
                }
            }

            if let Some(_) = state.resizing_node_id {
                if let Some((lx, ly)) = state.last_mouse_pos {
                    let dw = mouse.column as f64 - lx as f64;
                    let dh = mouse.row as f64 - ly as f64;
                    state.resize_selected_node(dw / state.zoom, dh / state.zoom);
                    state.last_mouse_pos = Some((mouse.column, mouse.row));
                    return true;
                }
            }

            if let Some(last_pos) = state.drag_start_pos {
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                let dx = cx - last_pos.0;
                let dy = cy - last_pos.1;
                state.move_selected_node(dx, dy);
                state.drag_start_pos = Some((cx, cy));
                true
            } else if let Some((lx, ly)) = state.last_mouse_pos {
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

fn execute_menu_action(state: &mut PinstarState, selected_index: usize, menu_type: PinstarMenuType, menu_x: u16, menu_y: u16) {
    if menu_type == PinstarMenuType::Editor {
        match selected_index {
            0 => { state.raw_editor.copy(); }
            1 => { state.raw_editor.cut(); let _ = state.sync_from_raw_editor(); }
            2 => { state.raw_editor.paste(); let _ = state.sync_from_raw_editor(); }
            3 => { state.raw_editor.select_all(); }
            _ => {}
        }
        return;
    }

    if menu_type == PinstarMenuType::ColorPicker {
        match selected_index {
            0 => state.set_node_color(None),
            1 => state.set_node_color(Some("#ff5252".to_string())),
            2 => state.set_node_color(Some("#4caf50".to_string())),
            3 => state.set_node_color(Some("#ffeb3b".to_string())),
            4 => state.set_node_color(Some("#00bcd4".to_string())),
            _ => {}
        }
        return;
    }

    let node_id = state.selected_node_id.clone();
    
    if let Some(id) = node_id {
        match selected_index {
            0 => state.start_connection(),
            1 => state.start_delete_connection(),
            2 => {
                // Rename Node - Initialize Popup
                let mut textarea = TextArea::from(vec![id.clone()]);
                textarea.set_cursor_line_style(ratatui::style::Style::default());
                textarea.set_block(ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title(" Rename Node (ID) - Enter to confirm, Esc to cancel "));
                state.rename_popup = Some(textarea);
            }
            3 => state.start_resize(),
            4 => {
                let items = vec![
                    "Default".to_string(),
                    "Red".to_string(),
                    "Green".to_string(),
                    "Yellow".to_string(),
                    "Blue".to_string(),
                ];
                state.context_menu = Some(crate::pinstar::state::PinstarContextMenu {
                    x: menu_x,
                    y: menu_y,
                    selected: 0,
                    items,

                    menu_type: PinstarMenuType::ColorPicker,
                });
            }
            5 => state.delete_node_connections(),
            6 => {
                let id_clone = id.clone();
                state.data.nodes.retain(|n| n.id() != id_clone);
                state.data.edges.retain(|e| e.from_node != id_clone && e.to_node != id_clone);
                state.selected_node_id = None;
                let _ = state.save();
                state.sync_to_raw_editor();
            }
            _ => {}
        }
    } else {
        match selected_index {
            0 => state.add_text_node(state.context_menu_pos.0, state.context_menu_pos.1),
            1 => state.add_group(state.context_menu_pos.0, state.context_menu_pos.1),
            _ => {}
        }
    }
}

pub fn handle_pinstar_event(state: &mut PinstarState, key: KeyEvent, running: &mut bool, area: ratatui::layout::Rect) -> bool {
    // 0. If rename popup is active
    if let Some(textarea) = &mut state.rename_popup {
        match key.code {
            KeyCode::Esc => {
                state.rename_popup = None;
            }
            KeyCode::Enter => {
                let new_id = textarea.lines().join("");
                state.rename_node(new_id);
                state.rename_popup = None;
            }
            _ => {
                textarea.input(Input::from(key));
            }
        }
        return true;
    }

    // 1. If context menu is active
    let mut menu_action = None;
    let mut close_menu = false;
    
    if let Some(menu) = &mut state.context_menu {
        match key.code {
            KeyCode::Esc => {
                close_menu = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                menu.selected = menu.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if menu.selected < menu.items.len() - 1 {
                    menu.selected += 1;
                }
            }
            KeyCode::Enter => {
                menu_action = Some((menu.selected, menu.menu_type, menu.x, menu.y));
                close_menu = true;
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
    } else if close_menu {
        return true;
    }
    
    if state.context_menu.is_some() {
        return true;
    }

    // 2. If floating editor is active
    if let Some(editor) = &mut state.floating_editor {
        match key.code {
            KeyCode::Esc => {
                state.toggle_editor();
                state.sync_to_raw_editor();
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.toggle_editor();
                state.sync_to_raw_editor();
            }
            _ => {
                editor.input(Input::from(key));
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
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                state.resizing_node_id = None;
                let _ = state.save();
                return true;
            }
            _ => {}
        }
    }

    if state.ext_focused {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => {
                state.ext_focused = false;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                state.ext_editor_enabled = !state.ext_editor_enabled;
            }
            _ => {}
        }
        return true;
    }

    if state.editor_focus {
        match key.code {
            KeyCode::Esc => {
                state.editor_focus = false;
            }
            KeyCode::Tab => {
                state.editor_focus = false;
                state.ext_focused = true;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = state.sync_from_raw_editor();
            }
            _ => {
                state.raw_editor.input(Input::from(key));
            }
        }
        return true;
    }

    match key.code {
        KeyCode::Esc => {
            if state.connection_source_id.is_some() {
                state.connection_source_id = None;
            } else {
                *running = false;
            }
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = state.save();
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.zoom_in();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.zoom_out();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            state.select_node_in_direction(-1.0, 0.0);
            state.center_on_selected();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            state.select_node_in_direction(1.0, 0.0);
            state.center_on_selected();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_node_in_direction(0.0, -1.0);
            state.center_on_selected();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_node_in_direction(0.0, 1.0);
            state.center_on_selected();
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            state.zoom_in();
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            state.zoom_out();
        }
        KeyCode::Char('i') | KeyCode::Enter => {
            let target_id_opt = state.selected_node_id.clone();
            if let Some(target_id) = target_id_opt {
                if state.connection_source_id.is_some() {
                    state.finish_connection(&target_id);
                } else if state.ext_editor_enabled {
                    state.trigger_ext_editor = true;
                } else {
                    state.toggle_editor();
                }
            }
        }
        KeyCode::Char('a') => {
            // Position menu visual at the bottom horizontal center of screen
            // Pre-clamped using approximate standard menu width (25) -> offset 12
            let menu_x = (area.width / 2).saturating_sub(12);
            let menu_y = area.height; // Fully bottom aligned
            
            // Set context creation targets at precise center of current view
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
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.show_grid = !state.show_grid;
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.show_editor_pane = !state.show_editor_pane;
            if !state.show_editor_pane {
                state.editor_focus = false;
            }
        }
        KeyCode::Tab => {
            if state.show_editor_pane {
                state.editor_focus = true;
            } else {
                state.ext_focused = true;
            }
        }
        _ => return false,
    }

    true
}
