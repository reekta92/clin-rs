use ratatui::{prelude::*, widgets::*};
use crate::pinstar::state::PinstarState;
use crate::app_theme::AppThemeColors;

fn get_node_color(color_code: Option<&str>, theme: &AppThemeColors) -> Color {
    match color_code {
        Some(s) if s.starts_with('#') => {
            if s.len() == 7 {
                let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
                let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
                let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
                Color::Rgb(r, g, b)
            } else {
                theme.accent
            }
        }
        Some("1") | Some("red") => Color::Rgb(255, 82, 82),      // Vibrant Red
        Some("2") | Some("orange") => Color::Rgb(255, 152, 0),   // Vivid Orange
        Some("3") | Some("yellow") => Color::Rgb(255, 235, 59),  // Bright Yellow
        Some("4") | Some("green") => Color::Rgb(76, 175, 80),    // Leaf Green
        Some("5") | Some("cyan") => Color::Rgb(0, 188, 212),     // Sky Blue
        Some("6") | Some("purple") => Color::Rgb(156, 39, 176),  // Deep Purple
        _ => theme.accent,
    }
}

pub fn draw_pinstar_view(frame: &mut Frame, state: &mut PinstarState, theme: &AppThemeColors) {
    let area = frame.area();
    
    let (editor_area, canvas_area) = if state.show_editor_pane {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(70),
            ])
            .split(area);
        (Some(main_chunks[0]), main_chunks[1])
    } else {
        (None, area)
    };

    // 1. Draw Raw Editor (30%) if enabled
    if let Some(editor_area) = editor_area {
        let editor_border_color = if state.editor_focus { theme.accent } else { theme.muted };
        let editor_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(editor_border_color))
            .title(" Source (JSON) ");
        state.raw_editor.set_block(editor_block);
        frame.render_widget(&state.raw_editor, editor_area);
    }

    // 2. Draw Canvas (70% or 100%)
    let canvas_border_color = if !state.editor_focus || !state.show_editor_pane { theme.accent } else { theme.muted };
    let canvas_block = Block::default()
        .borders(Borders::NONE)
        .border_style(Style::default().fg(canvas_border_color))
        .style(theme.bg_style());
    frame.render_widget(canvas_block, canvas_area);

    // 2a. Draw Edges FIRST (behind nodes)
    for edge in &state.data.edges {
        let from_node = state.data.nodes.iter().find(|n| n.id() == edge.from_node);
        let to_node = state.data.nodes.iter().find(|n| n.id() == edge.to_node);

        if let (Some(f), Some(t)) = (from_node, to_node) {
            let (fx, fy) = f.pos();
            let (fw, fh) = f.size();
            let (tx, ty) = t.pos();
            let (tw, th) = t.size();

            // Calculate border anchor points
            let scx = fx + fw / 2.0;
            let scy = fy + fh / 2.0;
            let tcx = tx + tw / 2.0;
            let tcy = ty + th / 2.0;

            let dx = tcx - scx;
            let dy = tcy - scy;

            let (ax, ay) = if dx.abs() > dy.abs() {
                if dx > 0.0 { (fx + fw, scy) } else { (fx, scy) }
            } else {
                if dy > 0.0 { (scx, fy + fh) } else { (scx, fy) }
            };

            let (bx, by) = if dx.abs() > dy.abs() {
                if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
            } else {
                if dy > 0.0 { (tcx, ty) } else { (tcx, ty + th) }
            };

            let sfx = ((ax - state.viewport_x) * state.zoom) + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
            let sfy = ((ay - state.viewport_y) * state.zoom) + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
            let stx = ((bx - state.viewport_x) * state.zoom) + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
            let sty = ((by - state.viewport_y) * state.zoom) + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);

            let mut current_x = sfx;
            let mut current_y = sfy;
            let target_x = stx;
            let target_y = sty;

            let dist = ((target_x - current_x).powi(2) + (target_y - current_y).powi(2)).sqrt();
            let steps = (dist * 4.0) as usize; 

            if steps > 0 {
                let dx = (target_x - current_x) / steps as f64;
                let dy = (target_y - current_y) / steps as f64;

                for _ in 0..=steps {
                    if current_x >= canvas_area.left() as f64 && current_x < canvas_area.right() as f64 &&
                       current_y >= canvas_area.top() as f64 && current_y < canvas_area.bottom() as f64 {
                        
                        let cell_x = current_x as u16;
                        let cell_y = current_y as u16;
                        
                        let dot_x = ((current_x - cell_x as f64) * 2.0) as u16;
                        let dot_y = ((current_y - cell_y as f64) * 4.0) as u16;

                        if let Some(cell) = frame.buffer_mut().cell_mut((cell_x, cell_y)) {
                            let mut braille_char = cell.symbol().chars().next().unwrap_or('\u{2800}');
                            if !('\u{2800}'..='\u{28FF}').contains(&braille_char) {
                                braille_char = '\u{2800}';
                            }

                            let dot_bit = match (dot_x, dot_y) {
                                (0, 0) => 0x01, (0, 1) => 0x02, (0, 2) => 0x04,
                                (1, 0) => 0x08, (1, 1) => 0x10, (1, 2) => 0x20,
                                (0, 3) => 0x40, (1, 3) => 0x80, _ => 0,
                            };

                            let new_code = (braille_char as u32 - 0x2800) | dot_bit;
                            if let Some(c) = char::from_u32(0x2800 + new_code) {
                                cell.set_char(c).set_fg(theme.muted);
                            }
                        }
                    }
                    current_x += dx;
                    current_y += dy;
                }
            }
        }
    }

    // 2b. Draw Nodes SECOND (above edges)
    for node in &state.data.nodes {
        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();
        
        let sx = ((nx - state.viewport_x) * state.zoom) + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
        let sy = ((ny - state.viewport_y) * state.zoom) + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
        let sw = nw * state.zoom;
        let sh = nh * state.zoom;

        if sx + sw < canvas_area.left() as f64 || sx > canvas_area.right() as f64 || 
           sy + sh < canvas_area.top() as f64 || sy > canvas_area.bottom() as f64 {
            continue; 
        }

        if sx < canvas_area.left() as f64 || sy < canvas_area.top() as f64 {
            continue;
        }

        let node_rect = Rect::new(
            sx as u16,
            sy as u16,
            sw.min(canvas_area.right() as f64 - sx) as u16,
            sh.min(canvas_area.bottom() as f64 - sy) as u16,
        );

        // CLEAR the node area explicitly before rendering content to prevent edge "bleed through"
        frame.render_widget(Clear, node_rect);

        let is_selected = state.selected_node_id.as_ref() == Some(&node.id().to_string());
        
        let node_color_attr = match node {
            crate::pinstar::data::CanvasNode::Text(n) => n.color.as_deref(),
            crate::pinstar::data::CanvasNode::File(n) => n.color.as_deref(),
            crate::pinstar::data::CanvasNode::Link(n) => n.color.as_deref(),
            crate::pinstar::data::CanvasNode::Group(n) => n.color.as_deref(),
        };

        let base_color = get_node_color(node_color_attr, theme);
        let border_color = if is_selected {
            theme.accent
        } else {
            base_color
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(node.id())
            .style(theme.bg_style());

        match node {
            crate::pinstar::data::CanvasNode::Group(g) => {
                let group_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(border_color))
                    .title(g.label.as_deref().unwrap_or("Group"))
                    .style(theme.bg_style());
                frame.render_widget(group_block, node_rect);
            }
            _ => {
                let text = Paragraph::new(node.text())
                    .block(block)
                    .style(Style::default().fg(theme.text)) // Force text color to stay consistent
                    .wrap(Wrap { trim: false });
                frame.render_widget(text, node_rect);
            }
        }
    }

    // Draw floating editor if active
    if let Some(editor) = &mut state.floating_editor {
        if let Some(node_id) = &state.selected_node_id {
            if let Some(node) = state.data.nodes.iter().find(|n| n.id() == node_id) {
                let (nx, ny) = node.pos();
                let (nw, nh) = node.size();
                
                let sx = ((nx - state.viewport_x) * state.zoom) + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
                let sy = ((ny - state.viewport_y) * state.zoom) + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
                let sw = nw * state.zoom;
                let sh = nh * state.zoom;

                let editor_rect = Rect::new(
                    sx.max(canvas_area.left() as f64) as u16,
                    sy.max(canvas_area.top() as f64) as u16,
                    sw as u16,
                    sh as u16,
                );

                frame.render_widget(Clear, editor_rect);
                frame.render_widget(&*editor, editor_rect);
            }
        }
    }

    // Draw hint line at the bottom
    let mut hint_text = "Pinstar View · Tab: switch focus · Esc: back · Arrows: select · i/Enter: edit · Ctrl+S: save".to_string();
    if state.connection_source_id.is_some() {
        hint_text = "CONNECTION MODE: Select target node with mouse or Enter".to_string();
    }
    
    let hint = Paragraph::new(Span::styled(
        hint_text,
        Style::default().fg(theme.muted),
    )).style(theme.hint_line_bg_style());
    
    let hint_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    frame.render_widget(hint, hint_area);

    // 3. Draw Context Menu
    if let Some(menu) = &state.context_menu {
        let menu_width = 25;
        let menu_height = menu.items.len() as u16 + 2;
        let menu_rect = Rect::new(
            menu.x.min(area.width.saturating_sub(menu_width)),
            menu.y.min(area.height.saturating_sub(menu_height)),
            menu_width,
            menu_height,
        );

        frame.render_widget(Clear, menu_rect);
        
        let items: Vec<ListItem> = menu.items.iter().enumerate().map(|(i, item)| {
            let style = if i == menu.selected {
                Style::default().fg(theme.highlight_fg).bg(theme.highlight_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            ListItem::new(format!("  {}", item)).style(style)
        }).collect();

        let list = List::new(items).block(Block::default().borders(Borders::NONE).style(theme.bg_style()));
        frame.render_widget(list, menu_rect);
    }

    // 4. Draw Rename Popup
    if let Some(textarea) = &state.rename_popup {
        let popup_area = centered_rect(60, 20, area); // Increased height and width for visibility
        frame.render_widget(Clear, popup_area);
        frame.render_widget(&*textarea, popup_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
