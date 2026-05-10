use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use crate::pinstar::state::PinstarState;
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
    let (_, canvas_area) = if state.show_editor_pane {
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

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Right) => {
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
            // 1. Check if context menu is open and if we clicked it
            if let Some(menu) = &state.context_menu {
                let menu_width = 20;
                let menu_height = menu.items.len() as u16 + 2;
                
                if mouse.column >= menu.x && mouse.column < menu.x + menu_width &&
                   mouse.row > menu.y && mouse.row < menu.y + menu_height - 1 {
                    
                    let selected = (mouse.row - menu.y - 1) as usize;
                    if selected < menu.items.len() {
                        execute_menu_action(state, selected);
                        state.context_menu = None;
                        return true;
                    }
                }
                // Clicked outside menu, close it
                state.context_menu = None;
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
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            } else {
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            }

            state.last_mouse_pos = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if state.drag_start_pos.is_some() {
                state.drag_start_pos = None;
                let _ = state.save();
            }
            state.last_mouse_pos = None;
            true
        }
        MouseEventKind::Drag(MouseButton::Left) => {
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
            state.zoom_in();
            true
        }
        MouseEventKind::ScrollDown => {
            state.zoom_out();
            true
        }
        _ => false,
    }
}

fn execute_menu_action(state: &mut PinstarState, selected_index: usize) {
    let node_id = state.selected_node_id.clone();
    
    if let Some(id) = node_id {
        let is_group = state.data.nodes.iter().any(|n| n.id() == id && matches!(n, crate::pinstar::data::CanvasNode::Group(_)));
        
        let effective_action = if is_group {
            match selected_index {
                0 => 1, 1 => 2, 2 => 3, 3 => 4, 4 => 5, 5 => 6, 6 => 7, _ => 99,
            }
        } else {
            selected_index
        };

        match effective_action {
            0 => state.start_connection(),
            1 => {
                // Rename Node - Initialize Popup
                let mut textarea = TextArea::from(vec![id.clone()]);
                textarea.set_cursor_line_style(ratatui::style::Style::default());
                textarea.set_block(ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title(" Rename Node (ID) - Enter to confirm, Esc to cancel "));
                state.rename_popup = Some(textarea);
            }
            2 => state.set_node_color(None),
            3 => state.set_node_color(Some("#ff5252".to_string())), 
            4 => state.set_node_color(Some("#4caf50".to_string())), 
            5 => state.set_node_color(Some("#ffeb3b".to_string())), 
            6 => state.set_node_color(Some("#00bcd4".to_string())), 
            7 => state.delete_node_connections(),
            8 => {
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

pub fn handle_pinstar_event(state: &mut PinstarState, key: KeyEvent, running: &mut bool) -> bool {
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
    if let Some(menu) = &mut state.context_menu {
        match key.code {
            KeyCode::Esc => {
                state.context_menu = None;
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
                let selected = menu.selected;
                execute_menu_action(state, selected);
                state.context_menu = None;
            }
            _ => {}
        }
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

    if state.editor_focus {
        match key.code {
            KeyCode::Esc => {
                state.editor_focus = false;
            }
            KeyCode::Tab => {
                state.editor_focus = false;
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
                } else {
                    state.toggle_editor();
                }
            }
        }
        KeyCode::Char('a') => {
            if let Some(id) = &state.selected_node_id {
                if state.data.nodes.iter().any(|n| n.id() == id) {
                    state.open_context_menu(50, 20, state.viewport_x, state.viewport_y);
                }
            } else {
                state.open_context_menu(50, 20, state.viewport_x, state.viewport_y);
            }
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
            }
        }
        _ => return false,
    }

    true
}
