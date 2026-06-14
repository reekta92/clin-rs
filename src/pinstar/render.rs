use crate::app_theme::AppThemeColors;
use crate::constants::*;
use crate::pinstar::state::PinstarState;
use ratatui::{prelude::*, widgets::*};

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
        Some("1") | Some("red") => Color::Rgb(255, 82, 82),
        Some("2") | Some("orange") => Color::Rgb(255, 152, 0),
        Some("3") | Some("yellow") => Color::Rgb(255, 235, 59),
        Some("4") | Some("green") => Color::Rgb(76, 175, 80),
        Some("5") | Some("cyan") => Color::Rgb(0, 188, 212),
        Some("6") | Some("purple") => Color::Rgb(156, 39, 176),
        _ => theme.accent,
    }
}

pub fn draw_pinstar_view(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &AppThemeColors,
    area: ratatui::layout::Rect,
) {
    let total_area = area;
    let mut area = area;
    area.height = area.height.saturating_sub(1);

    let (editor_area, canvas_area) = if state.show_editor_pane {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);
        (Some(main_chunks[0]), main_chunks[1])
    } else {
        (None, area)
    };

    if let Some(editor_area) = editor_area {
        let editor_border_color = if state.editor_focus {
            theme.accent
        } else {
            theme.muted
        };
        let editor_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(editor_border_color))
            .title(" Source (JSON) ")
            .style(theme.preview_bg_style());

        let line_count = state.raw_editor.lines().len();
        let cursor_row = state.raw_editor.cursor().0;
        let scroll_row = crate::ui::get_textarea_scroll(&state.raw_editor).0;

        let content_area = editor_area;
        let digits = line_count.max(1).to_string().len() as u16;
        let gutter_width = digits + 1;
        let gutter_area = Rect::new(
            content_area.x,
            content_area.y,
            gutter_width.min(content_area.width),
            content_area.height,
        );
        let gutter = crate::ui::line_number_gutter(
            line_count,
            cursor_row,
            scroll_row,
            content_area.height,
            theme,
            1,
        );
        frame.render_widget(gutter, gutter_area);

        let editor_rect = Rect::new(
            content_area.x + gutter_area.width,
            content_area.y,
            content_area.width.saturating_sub(gutter_area.width),
            content_area.height,
        );

        state.raw_editor.set_block(editor_block);
        state.raw_editor.set_style(theme.preview_bg_style());
        state
            .raw_editor
            .set_cursor_line_style(if state.editor_focus {
                Style::default().bg(theme.preview_bg().unwrap_or(Color::DarkGray))
            } else {
                Style::default()
            });
        frame.render_widget(&state.raw_editor, editor_rect);

        if state.editor_focus {
            let cursor_bg = theme.preview_bg().unwrap_or(theme.highlight_bg);
            crate::ui::fill_cursor_line_bg(frame, &state.raw_editor, editor_rect, cursor_bg);
        }
    }

    let canvas_border_color = if !state.editor_focus || !state.show_editor_pane {
        theme.accent
    } else {
        theme.muted
    };
    let canvas_block = Block::default()
        .borders(Borders::NONE)
        .border_style(Style::default().fg(canvas_border_color))
        .style(theme.bg_style());
    frame.render_widget(canvas_block, canvas_area);

    if state.show_grid {
        let mut grid_step_x = 100.0;
        let mut grid_step_y = 50.0;
        while grid_step_y * state.zoom < 6.0 {
            grid_step_x *= 2.0;
            grid_step_y *= 2.0;
        }

        let (cx1, cy1) = state.screen_to_canvas(canvas_area.left(), canvas_area.top(), canvas_area);
        let (cx2, cy2) =
            state.screen_to_canvas(canvas_area.right(), canvas_area.bottom(), canvas_area);

        let min_cx = cx1.min(cx2);
        let max_cx = cx1.max(cx2);
        let min_cy = cy1.min(cy2);
        let max_cy = cy1.max(cy2);

        let start_x = (min_cx / grid_step_x).floor() * grid_step_x;
        let end_x = (max_cx / grid_step_x).ceil() * grid_step_x;
        let start_y = (min_cy / grid_step_y).floor() * grid_step_y;
        let end_y = (max_cy / grid_step_y).ceil() * grid_step_y;

        let buf = frame.buffer_mut();
        let mut cur_x = start_x;
        while cur_x <= end_x {
            let mut cur_y = start_y;
            while cur_y <= end_y {
                let sx = (((cur_x - state.viewport_x) * state.zoom)
                    + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0))
                    .round() as i32;
                let sy = (((cur_y - state.viewport_y) * state.zoom)
                    + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0))
                    .round() as i32;

                if sx >= canvas_area.left() as i32
                    && sx < canvas_area.right() as i32
                    && sy >= canvas_area.top() as i32
                    && sy < canvas_area.bottom() as i32
                    && sx >= 0
                    && sx < buf.area.width as i32
                    && sy >= 0
                    && sy < buf.area.height as i32
                    && let Some(cell) = buf.cell_mut((sx as u16, sy as u16))
                {
                    cell.set_char('·').set_fg(theme.muted);
                }
                cur_y += grid_step_y;
            }
            cur_x += grid_step_x;
        }
    }

    for node in &state.data.nodes {
        if let crate::pinstar::data::CanvasNode::Group(g) = node {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();

            let sx = ((nx - state.viewport_x) * state.zoom)
                + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
            let sy = ((ny - state.viewport_y) * state.zoom)
                + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
            let sw = nw * state.zoom;
            let sh = nh * state.zoom;

            if sx + sw < canvas_area.left() as f64
                || sx > canvas_area.right() as f64
                || sy + sh < canvas_area.top() as f64
                || sy > canvas_area.bottom() as f64
            {
                continue;
            }

            let left = sx.max(canvas_area.left() as f64);
            let top = sy.max(canvas_area.top() as f64);
            let right = (sx + sw).min(canvas_area.right() as f64);
            let bottom = (sy + sh).min(canvas_area.bottom() as f64);

            if right <= left || bottom <= top {
                continue;
            }

            let node_rect = Rect::new(
                left as u16,
                top as u16,
                (right - left) as u16,
                (bottom - top) as u16,
            );

            let is_selected = state.selected_node_id.as_ref() == Some(&g.id.to_string());
            let is_editing = is_selected && state.floating_editor.is_some();
            let base_color = get_node_color(g.color.as_deref(), theme);
            let border_color = if is_editing { theme.accent } else { base_color };

            let mut label = g.label.as_deref().unwrap_or("Group").to_string();
            if is_editing {
                label = format!("[EDITING] {label}");
            }

            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(
                    label,
                    Style::default().fg(if is_editing { theme.accent } else { base_color }),
                ))
                .style(theme.bg_style());

            if is_selected && !is_editing {
                block = block.border_set(ratatui::symbols::border::Set {
                    top_left: "┌",
                    top_right: "┐",
                    bottom_left: "└",
                    bottom_right: "┘",
                    vertical_left: "┆",
                    vertical_right: "┆",
                    horizontal_top: "┄",
                    horizontal_bottom: "┄",
                });
            } else {
                block = block.border_type(if is_editing {
                    BorderType::Rounded
                } else {
                    BorderType::Double
                });
            }

            frame.render_widget(block, node_rect);

            if is_selected {
                let corner_style = Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD);
                if node_rect.width > 0 && node_rect.height > 0 {
                    frame.render_widget(
                        Paragraph::new("⇘").style(corner_style),
                        Rect::new(node_rect.x, node_rect.y, 1, 1),
                    );
                    if node_rect.width > 1 {
                        frame.render_widget(
                            Paragraph::new("⇙").style(corner_style),
                            Rect::new(node_rect.x + node_rect.width - 1, node_rect.y, 1, 1),
                        );
                    }
                    if node_rect.height > 1 {
                        frame.render_widget(
                            Paragraph::new("⇗").style(corner_style),
                            Rect::new(node_rect.x, node_rect.y + node_rect.height - 1, 1, 1),
                        );
                    }
                    if node_rect.width > 1 && node_rect.height > 1 {
                        frame.render_widget(
                            Paragraph::new("⇖").style(corner_style),
                            Rect::new(
                                node_rect.x + node_rect.width - 1,
                                node_rect.y + node_rect.height - 1,
                                1,
                                1,
                            ),
                        );
                    }
                }
            }

            if state.resizing_node_id.as_ref() == Some(&g.id.to_string()) {
                let handle_text = "[↘]";
                let handle_style = Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD);
                let handle_rect = Rect::new(
                    (sx + sw - 3.0).max(0.0) as u16,
                    (sy + sh - 1.0).max(0.0) as u16,
                    3,
                    1,
                );
                frame.render_widget(Paragraph::new(handle_text).style(handle_style), handle_rect);
            }
        }
    }

    for edge in &state.data.edges {
        let from_node = state.data.nodes.iter().find(|n| n.id() == edge.from_node);
        let to_node = state.data.nodes.iter().find(|n| n.id() == edge.to_node);

        if let (Some(f), Some(t)) = (from_node, to_node) {
            let (fx, fy) = f.pos();
            let (fw, fh) = f.size();
            let (tx, ty) = t.pos();
            let (tw, th) = t.size();

            let scx = fx + fw / 2.0;
            let scy = fy + fh / 2.0;
            let tcx = tx + tw / 2.0;
            let tcy = ty + th / 2.0;

            let dx = tcx - scx;
            let dy = tcy - scy;

            let (ax, ay) = if dx.abs() > dy.abs() {
                if dx > 0.0 { (fx + fw, scy) } else { (fx, scy) }
            } else if dy > 0.0 {
                (scx, fy + fh)
            } else {
                (scx, fy)
            };

            let (bx, by) = if dx.abs() > dy.abs() {
                if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
            } else if dy > 0.0 {
                (tcx, ty)
            } else {
                (tcx, ty + th)
            };

            let sfx = ((ax - state.viewport_x) * state.zoom)
                + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
            let sfy = ((ay - state.viewport_y) * state.zoom)
                + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
            let stx = ((bx - state.viewport_x) * state.zoom)
                + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
            let sty = ((by - state.viewport_y) * state.zoom)
                + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);

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
                    if current_x >= canvas_area.left() as f64
                        && current_x < canvas_area.right() as f64
                        && current_y >= canvas_area.top() as f64
                        && current_y < canvas_area.bottom() as f64
                    {
                        let cell_x = current_x as u16;
                        let cell_y = current_y as u16;

                        let dot_x = ((current_x - cell_x as f64) * 2.0) as u16;
                        let dot_y = ((current_y - cell_y as f64) * 4.0) as u16;

                        if let Some(cell) = frame.buffer_mut().cell_mut((cell_x, cell_y)) {
                            let mut braille_char =
                                cell.symbol().chars().next().unwrap_or('\u{2800}');
                            if !('\u{2800}'..='\u{28FF}').contains(&braille_char) {
                                braille_char = '\u{2800}';
                            }

                            let dot_bit = match (dot_x, dot_y) {
                                (0, 0) => 0x01,
                                (0, 1) => 0x02,
                                (0, 2) => 0x04,
                                (1, 0) => 0x08,
                                (1, 1) => 0x10,
                                (1, 2) => 0x20,
                                (0, 3) => 0x40,
                                (1, 3) => 0x80,
                                _ => 0,
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

    for node in &state.data.nodes {
        if matches!(node, crate::pinstar::data::CanvasNode::Group(_)) {
            continue;
        }

        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = ((nx - state.viewport_x) * state.zoom)
            + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
        let sy = ((ny - state.viewport_y) * state.zoom)
            + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
        let sw = nw * state.zoom;
        let sh = nh * state.zoom;

        if sx + sw < canvas_area.left() as f64
            || sx > canvas_area.right() as f64
            || sy + sh < canvas_area.top() as f64
            || sy > canvas_area.bottom() as f64
        {
            continue;
        }

        let left = sx.max(canvas_area.left() as f64);
        let top = sy.max(canvas_area.top() as f64);
        let right = (sx + sw).min(canvas_area.right() as f64);
        let bottom = (sy + sh).min(canvas_area.bottom() as f64);

        if right <= left || bottom <= top {
            continue;
        }

        let node_rect = Rect::new(
            left as u16,
            top as u16,
            (right - left) as u16,
            (bottom - top) as u16,
        );

        frame.render_widget(Clear, node_rect);

        let is_selected = state.selected_node_id.as_ref() == Some(&node.id().to_string());
        let is_editing = is_selected && state.floating_editor.is_some();

        let node_color_attr = match node {
            crate::pinstar::data::CanvasNode::Text(n) => n.color.as_deref(),
            crate::pinstar::data::CanvasNode::File(n) => n.color.as_deref(),
            crate::pinstar::data::CanvasNode::Link(n) => n.color.as_deref(),
            _ => None,
        };

        let base_color = get_node_color(node_color_attr, theme);
        let border_color = if is_editing { theme.accent } else { base_color };

        let mut border_type = BorderType::Plain;
        if is_editing {
            border_type = BorderType::Double;
        }

        let mut node_title = match node {
            crate::pinstar::data::CanvasNode::File(n) => std::path::Path::new(&n.file)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&n.file)
                .to_string(),
            crate::pinstar::data::CanvasNode::Link(n) => n.url.clone(),
            _ => {
                if is_generated_id(node.id()) {
                    "".to_string()
                } else {
                    node.id().to_string()
                }
            }
        };

        if is_editing {
            node_title = format!("[EDITING] {node_title}");
        }

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                node_title,
                Style::default().fg(if is_editing { theme.accent } else { base_color }),
            ))
            .style(theme.bg_style());

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "┌",
                top_right: "┐",
                bottom_left: "└",
                bottom_right: "┘",
                vertical_left: "┆",
                vertical_right: "┆",
                horizontal_top: "┄",
                horizontal_bottom: "┄",
            });
        } else {
            block = block.border_type(border_type);
        }

        let text = Paragraph::new(node.text())
            .block(block)
            .style(Style::default().fg(theme.text))
            .wrap(Wrap { trim: false });
        frame.render_widget(text, node_rect);

        if is_selected {
            let corner_style = Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD);
            if node_rect.width > 0 && node_rect.height > 0 {
                frame.render_widget(
                    Paragraph::new("⇘").style(corner_style),
                    Rect::new(node_rect.x, node_rect.y, 1, 1),
                );
                if node_rect.width > 1 {
                    frame.render_widget(
                        Paragraph::new("⇙").style(corner_style),
                        Rect::new(node_rect.x + node_rect.width - 1, node_rect.y, 1, 1),
                    );
                }
                if node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("⇗").style(corner_style),
                        Rect::new(node_rect.x, node_rect.y + node_rect.height - 1, 1, 1),
                    );
                }
                if node_rect.width > 1 && node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("⇖").style(corner_style),
                        Rect::new(
                            node_rect.x + node_rect.width - 1,
                            node_rect.y + node_rect.height - 1,
                            1,
                            1,
                        ),
                    );
                }
            }
        }

        if state.resizing_node_id.as_ref() == Some(&node.id().to_string()) {
            let handle_text = "[↘]";
            let handle_style = Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD);
            let handle_rect = Rect::new(
                (sx + sw - 3.0).max(0.0) as u16,
                (sy + sh - 1.0).max(0.0) as u16,
                3,
                1,
            );
            frame.render_widget(Paragraph::new(handle_text).style(handle_style), handle_rect);
        }
    }

    if let Some(editor) = &mut state.floating_editor
        && let Some(node_id) = &state.selected_node_id
        && let Some(node) = state.data.nodes.iter().find(|n| n.id() == node_id)
    {
        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = ((nx - state.viewport_x) * state.zoom)
            + (canvas_area.x as f64 + canvas_area.width as f64 / 2.0);
        let sy = ((ny - state.viewport_y) * state.zoom)
            + (canvas_area.y as f64 + canvas_area.height as f64 / 2.0);
        let sw = nw * state.zoom;
        let sh = nh * state.zoom;

        let left = sx.max(canvas_area.left() as f64);
        let top = sy.max(canvas_area.top() as f64);
        let right = (sx + sw).min(canvas_area.right() as f64);
        let bottom = (sy + sh).min(canvas_area.bottom() as f64);

        if right > left && bottom > top {
            let editor_rect = Rect::new(
                left as u16,
                top as u16,
                (right - left) as u16,
                (bottom - top) as u16,
            );

            editor.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent))
                    .style(theme.bg_style()),
            );
            editor.set_style(theme.bg_style());

            frame.render_widget(Clear, editor_rect);
            frame.render_widget(&*editor, editor_rect);
        }
    }

    let mut hint_text = if state.footer_hint.is_empty() {
        CANVAS_HELP_HINTS.to_string()
    } else {
        state.footer_hint.clone()
    };
    if state.connection_source_id.is_some() {
        hint_text = "CONNECTION MODE: Select target node with mouse or Enter".to_string();
    } else if state.deleting_connection_source_id.is_some() {
        hint_text = "DELETE CONNECTION MODE: Select target node to remove link".to_string();
    } else if state.resizing_node_id.is_some() {
        hint_text = "RESIZE MODE: Drag mouse to resize, Left-click to confirm".to_string();
    }

    let hint_area = Rect::new(
        total_area.x,
        total_area.bottom().saturating_sub(1),
        total_area.width,
        1,
    );
    crate::ui::draw_status_bar(frame, hint_area, theme, None, &hint_text, None);

    if let Some(menu) = &state.context_menu {
        let menu_width = 25;
        let menu_height = menu.items.len() as u16;
        let menu_rect = Rect::new(
            area.x + menu.x.min(area.width.saturating_sub(menu_width)),
            area.y + menu.y.min(area.height.saturating_sub(menu_height)),
            menu_width,
            menu_height,
        );

        frame.render_widget(Clear, menu_rect);

        let items: Vec<ListItem> = menu
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == menu.selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                ListItem::new(format!("  {item}")).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::NONE)
                .style(theme.preview_bg_style()),
        );
        frame.render_widget(list, menu_rect);
    }

    if let Some(textarea) = &mut state.rename_popup {
        let content = crate::ui::draw_popup_frame(
            frame,
            area,
            "RENAME NODE",
            60,
            20,
            "Enter confirm · Esc cancel",
            theme,
        );

        textarea.set_style(theme.bg_style());
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .style(theme.bg_style()),
        );

        frame.render_widget(&*textarea, content);
    }
}

fn is_generated_id(id: &str) -> bool {
    if id.starts_with("node_") && id.len() <= 16 {
        return true;
    }
    if id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return true;
    }
    false
}
