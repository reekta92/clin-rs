use crate::app::ViewMode;
use crate::app_theme::AppThemeColors;
use crate::events::contains_cell;
use crate::keybinds::CanvasAction;
use crate::pinstar::state::PinstarState;
use ratatui::{prelude::*, widgets::*};

#[allow(dead_code)]
struct Proj {
    is_group: bool,
    pos: (f64, f64),  // canvas-space top-left
    size: (f64, f64), // canvas-space size
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
    scx: f64,
    scy: f64,
    on_screen: bool,
}

fn is_image_ext(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}
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
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>()
                && idx >= 1
                && idx <= crate::pinstar::COLOR_PICKER_PALETTE.len()
            {
                return crate::pinstar::COLOR_PICKER_PALETTE[idx - 1].2;
            }
            if let Some(entry) = crate::pinstar::COLOR_PICKER_PALETTE
                .iter()
                .find(|e| e.0 == s)
            {
                entry.2
            } else {
                theme.accent
            }
        }
        _ => theme.accent,
    }
}

pub fn draw_pinstar_view(
    frame: &mut Frame,
    state: &mut PinstarState,
    app: &mut crate::app::App,
    area: ratatui::layout::Rect,
    mouse_pos: Option<(u16, u16)>,
) {
    let theme_val = app.app_theme.clone();
    let theme = &theme_val;
    let total_area = area;
    let canvas_mouse_pos = if state.context_menu.is_some() {
        None
    } else {
        mouse_pos
    };
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
            .style(theme.bg_style());

        crate::ui::render_textarea_with_theme(
            frame,
            &mut state.raw_editor,
            editor_area,
            theme,
            state.editor_focus,
            true,
            editor_block,
            theme.bg_style(),
        );
    }

    // Per-frame projection invariants (reused by grid, group, edge, node passes).
    // Expression order is identical to the inline forms it replaces, so float
    // results are bit-identical.
    let origin_x = canvas_area.x as f64 + canvas_area.width as f64 / 2.0;
    let origin_y = canvas_area.y as f64 + canvas_area.height as f64 / 2.0;
    let z = state.zoom;
    let vx = state.viewport_x;
    let vy = state.viewport_y;
    let view_left = canvas_area.left() as f64;
    let view_right = canvas_area.right() as f64;
    let view_top = canvas_area.top() as f64;
    let view_bottom = canvas_area.bottom() as f64;

    let proj: Vec<Proj> = state
        .data
        .nodes
        .iter()
        .map(|n| {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            let sx = (nx - vx) * z + origin_x;
            let sy = (ny - vy) * z + origin_y;
            let sw = nw * z;
            let sh = nh * z;
            Proj {
                is_group: matches!(n, crate::pinstar::data::CanvasNode::Group(_)),
                pos: (nx, ny),
                size: (nw, nh),
                sx,
                sy,
                sw,
                sh,
                scx: sx + sw / 2.0,
                scy: sy + sh / 2.0,
                on_screen: !(sx + sw < view_left
                    || sx > view_right
                    || sy + sh < view_top
                    || sy > view_bottom),
            }
        })
        .collect();

    let config = &app.config;

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
                let sx = (((cur_x - vx) * z) + origin_x).round() as i32;
                let sy = (((cur_y - vy) * z) + origin_y).round() as i32;

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

    for (idx, p) in proj.iter().enumerate() {
        if !p.is_group {
            continue;
        }
        if !p.on_screen {
            continue;
        }
        let node = &state.data.nodes[idx];
        let crate::pinstar::data::CanvasNode::Group(g) = node else {
            continue;
        };

        let sx = p.sx;
        let sy = p.sy;
        let sw = p.sw;
        let sh = p.sh;

        let left = sx.max(view_left);
        let top = sy.max(view_top);
        let right = (sx + sw).min(view_right);
        let bottom = (sy + sh).min(view_bottom);
        if right <= left || bottom <= top {
            continue;
        }
        let node_rect = Rect::new(
            left as u16,
            top as u16,
            (right - left) as u16,
            (bottom - top) as u16,
        );

        let is_selected = state.selected_node_id.as_deref() == Some(g.id.as_str());
        let is_editing = is_selected && state.floating_editor.is_some();
        let base_color = get_node_color(g.color.as_deref(), theme);
        let border_color = if is_editing { theme.accent } else { base_color };

        let mut label = g.label.as_deref().unwrap_or("Group").to_string();
        if is_editing {
            label = format!("[EDITING] {label}");
        }

        let is_hovered = !is_selected
            && canvas_mouse_pos.is_some_and(|(col, row)| contains_cell(node_rect, col, row));
        let bg_style = if is_hovered {
            theme.hover_style()
        } else {
            theme.bg_style()
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                label,
                Style::default().fg(if is_editing { theme.accent } else { base_color }),
            ))
            .style(bg_style);

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "\u{250c}",
                top_right: "\u{2510}",
                bottom_left: "\u{2514}",
                bottom_right: "\u{2518}",
                vertical_left: "\u{2506}",
                vertical_right: "\u{2506}",
                horizontal_top: "\u{2504}",
                horizontal_bottom: "\u{2504}",
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
                    Paragraph::new("\u{21d8}").style(corner_style),
                    Rect::new(node_rect.x, node_rect.y, 1, 1),
                );
                if node_rect.width > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d9}").style(corner_style),
                        Rect::new(node_rect.x + node_rect.width - 1, node_rect.y, 1, 1),
                    );
                }
                if node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d7}").style(corner_style),
                        Rect::new(node_rect.x, node_rect.y + node_rect.height - 1, 1, 1),
                    );
                }
                if node_rect.width > 1 && node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d6}").style(corner_style),
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

        if state.resizing_node_id.as_deref() == Some(g.id.as_str()) {
            let handle_text = "[\u{2198}]";
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

    {
        use std::collections::HashMap;
        // Keys borrow state.data.nodes; map drops at end of this block,
        // before the non-group pass takes &mut state.image_cache.
        let id_index: HashMap<&str, usize> = state
            .data
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id(), i))
            .collect();

        for edge in &state.data.edges {
            let Some(&ia) = id_index.get(edge.from_node.as_str()) else {
                continue;
            };
            let Some(&ib) = id_index.get(edge.to_node.as_str()) else {
                continue;
            };
            let pf = &proj[ia];
            let pt = &proj[ib];

            let (fx, fy, fw, fh) = (pf.pos.0, pf.pos.1, pf.size.0, pf.size.1);
            let (tx, ty, tw, th) = (pt.pos.0, pt.pos.1, pt.size.0, pt.size.1);

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

            let sfx = (ax - vx) * z + origin_x;
            let sfy = (ay - vy) * z + origin_y;
            let stx = (bx - vx) * z + origin_x;
            let sty = (by - vy) * z + origin_y;

            // Cull edges whose screen bbox is entirely outside canvas
            let min_x = sfx.min(stx);
            let max_x = sfx.max(stx);
            let min_y = sfy.min(sty);
            let max_y = sfy.max(sty);
            if max_x < view_left || min_x > view_right || max_y < view_top || min_y > view_bottom {
                continue;
            }

            let mut current_x = sfx;
            let mut current_y = sfy;
            let dist = ((stx - sfx).powi(2) + (sty - sfy).powi(2)).sqrt();
            let steps = (dist * 4.0) as usize;
            if steps > 0 {
                let ddx = (stx - sfx) / steps as f64;
                let ddy = (sty - sfy) / steps as f64;
                for _ in 0..=steps {
                    if current_x >= view_left
                        && current_x < view_right
                        && current_y >= view_top
                        && current_y < view_bottom
                    {
                        let cell_x = current_x as u16;
                        let cell_y = current_y as u16;
                        let dot_x = ((current_x - cell_x as f64) * 2.0) as u16;
                        let dot_y = ((current_y - cell_y as f64) * 4.0) as u16;
                        crate::ui::braille::set_braille_dot(
                            frame.buffer_mut(),
                            cell_x,
                            cell_y,
                            dot_x,
                            dot_y,
                            theme.muted,
                        );
                    }
                    current_x += ddx;
                    current_y += ddy;
                }
            }
        }
    } // end edge-pass block

    for (idx, p) in proj.iter().enumerate() {
        if p.is_group {
            continue;
        }
        if !p.on_screen {
            continue;
        }
        let node = &state.data.nodes[idx];
        let sx = p.sx;
        let sy = p.sy;
        let sw = p.sw;
        let sh = p.sh;

        let left = sx.max(view_left);
        let top = sy.max(view_top);
        let right = (sx + sw).min(view_right);
        let bottom = (sy + sh).min(view_bottom);
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

        let is_selected = state.selected_node_id.as_deref() == Some(node.id());
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

        let mut node_title = match node.title() {
            Some(t) => t.to_string(),
            None => match node {
                crate::pinstar::data::CanvasNode::File(n) => std::path::Path::new(&n.file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&n.file)
                    .to_string(),
                crate::pinstar::data::CanvasNode::Link(n) => n.url.clone(),
                crate::pinstar::data::CanvasNode::Group(n) => n.label.clone().unwrap_or_default(),
                crate::pinstar::data::CanvasNode::Text(_) => "".to_string(),
            },
        };

        if is_editing {
            node_title = format!("[EDITING] {node_title}");
        }
        let is_hovered = !is_selected
            && canvas_mouse_pos.is_some_and(|(col, row)| contains_cell(node_rect, col, row));
        let bg_style = if is_hovered {
            theme.hover_style()
        } else {
            theme.bg_style()
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                node_title,
                Style::default().fg(if is_editing { theme.accent } else { base_color }),
            ))
            .style(bg_style);

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "\u{250c}",
                top_right: "\u{2510}",
                bottom_left: "\u{2514}",
                bottom_right: "\u{2518}",
                vertical_left: "\u{2506}",
                vertical_right: "\u{2506}",
                horizontal_top: "\u{2504}",
                horizontal_bottom: "\u{2504}",
            });
        } else {
            block = block.border_type(border_type);
        }

        let is_image_file = matches!(node, crate::pinstar::data::CanvasNode::File(n) if is_image_ext(&n.file))
            && state.image_picker.is_some();

        // Short-circuit: during transforms, skip pixel decode and render plain text
        if state.is_view_transforming() {
            let text = Paragraph::new(node.text())
                .block(block)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, node_rect);
            continue;
        }

        // Render pixel image if available
        if is_image_file {
            let file_path = match node {
                crate::pinstar::data::CanvasNode::File(n) => n.file.clone(),
                _ => String::new(),
            };
            let picker = state.image_picker.as_ref().expect("checked above");
            let key = crate::image_render::ImageKey {
                path: std::path::PathBuf::from(&file_path),
                mtime: 0,
            };
            if let Some(tx) = &state.image_decode_tx {
                state.image_cache.request(key.clone(), 2048, tx, picker);
            }
            if let Some(proto) = state.image_cache.get_proto(&key)
                && node_rect.width > 2
                && node_rect.height > 2
            {
                let inner_area = Rect::new(
                    node_rect.x + 1,
                    node_rect.y + 1,
                    node_rect.width.saturating_sub(2),
                    node_rect.height.saturating_sub(2),
                );
                frame.render_widget(block, node_rect);
                frame.render_stateful_widget(
                    ratatui_image::StatefulImage::default()
                        .resize(ratatui_image::Resize::Fit(None)),
                    inner_area,
                    proto,
                );
            } else {
                // No proto yet: render placeholder
                let text = Paragraph::new(node.text())
                    .block(block)
                    .style(Style::default().fg(theme.text))
                    .wrap(Wrap { trim: false });
                frame.render_widget(text, node_rect);
            }
        } else {
            let text = Paragraph::new(node.text())
                .block(block)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, node_rect);
        }

        if is_selected {
            let corner_style = Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD);
            if node_rect.width > 0 && node_rect.height > 0 {
                frame.render_widget(
                    Paragraph::new("\u{21d8}").style(corner_style),
                    Rect::new(node_rect.x, node_rect.y, 1, 1),
                );
                if node_rect.width > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d9}").style(corner_style),
                        Rect::new(node_rect.x + node_rect.width - 1, node_rect.y, 1, 1),
                    );
                }
                if node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d7}").style(corner_style),
                        Rect::new(node_rect.x, node_rect.y + node_rect.height - 1, 1, 1),
                    );
                }
                if node_rect.width > 1 && node_rect.height > 1 {
                    frame.render_widget(
                        Paragraph::new("\u{21d6}").style(corner_style),
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

        if state.resizing_node_id.as_deref() == Some(node.id()) {
            let handle_text = "[\u{2198}]";
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
    state.floating_editor_rect = None;

    if let Some(editor) = &mut state.floating_editor
        && let Some(node_id) = &state.selected_node_id
        && let Some(node) = state.data.nodes.iter().find(|n| n.id() == node_id)
    {
        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = ((nx - vx) * z) + origin_x;
        let sy = ((ny - vy) * z) + origin_y;
        let sw = nw * z;
        let sh = nh * z;

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
            state.floating_editor_rect =
                Some(Block::default().borders(Borders::ALL).inner(editor_rect));

            frame.render_widget(Clear, editor_rect);
            frame.render_widget(&*editor, editor_rect);
        }
    }

    let hint_area = Rect::new(
        total_area.x,
        total_area.bottom().saturating_sub(1),
        total_area.width,
        1,
    );
    let hint_line = if state.connection_source_id.is_some() {
        Line::from(vec![Span::styled(
            "CONNECTION MODE: Select target node with mouse or Enter",
            Style::default().fg(theme.muted),
        )])
    } else if state.deleting_connection_source_id.is_some() {
        Line::from(vec![Span::styled(
            "DELETE CONNECTION MODE: Select target node to remove link",
            Style::default().fg(theme.muted),
        )])
    } else if state.resizing_node_id.is_some() {
        Line::from(vec![Span::styled(
            "RESIZE MODE: Drag mouse to resize, Left-click to confirm",
            Style::default().fg(theme.muted),
        )])
    } else if state.footer_hint.is_empty() {
        let hints_items = vec![
            (
                format!(
                    "{}/{}",
                    state.keybinds.display_canvas(CanvasAction::MoveUp),
                    state.keybinds.display_canvas(CanvasAction::MoveDown)
                ),
                "move",
            ),
            (
                state.keybinds.display_canvas(CanvasAction::OpenContextMenu),
                "menu",
            ),
            (
                format!(
                    "{}/{}",
                    state.keybinds.display_canvas(CanvasAction::ZoomOut),
                    state.keybinds.display_canvas(CanvasAction::ZoomIn)
                ),
                "zoom",
            ),
            (
                state.keybinds.canvas_keys_display(CanvasAction::Quit),
                "back",
            ),
            (
                format!(
                    "F1/{}",
                    state.keybinds.canvas_keys_display(CanvasAction::Help)
                ),
                "help",
            ),
            ("F2".to_string(), "keybinds"),
        ];
        crate::ui::format_keybind_hints(theme, &hints_items)
    } else {
        Line::from(vec![Span::styled(
            state.footer_hint.clone(),
            Style::default().fg(theme.muted),
        )])
    };
    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Canvas);
    ctx.area = Some(hint_area);
    ctx.canvas = Some(state);
    ctx.hints = Some(hint_line.spans);
    if let Some(p) = &state.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        )]);
    }

    let (left_line, right_line) =
        crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::Canvas, theme);
    crate::ui::draw_status_bar(frame, hint_area, theme, left_line, right_line);

    if let Some(menu) = &state.context_menu {
        let menu_width = menu
            .items
            .iter()
            .map(|s| s.len() as u16 + 4)
            .max()
            .unwrap_or(25);
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
                ListItem::new(format!("  {item}  ")).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::NONE)
                .style(theme.preview_bg_style()),
        );
        let list_state =
            crate::ui::render_list_with_selection(frame, list, menu_rect, Some(menu.selected), 0);
        crate::ui::paint_list_hover(
            frame,
            menu_rect,
            &list_state,
            menu.items.len(),
            mouse_pos,
            theme.hover_style(),
        );
    }

    if let Some(textarea) = &mut state.rename_popup {
        let hints_items = &[
            (
                state
                    .keybinds
                    .display_canvas(crate::keybinds::CanvasAction::RenameConfirm),
                "confirm",
            ),
            (
                state
                    .keybinds
                    .display_canvas(crate::keybinds::CanvasAction::RenameCancel),
                "cancel",
            ),
        ];
        let content = crate::ui::draw_popup_frame(
            frame,
            area,
            "RENAME NODE",
            crate::ui::PopupSize::Prompt,
            crate::ui::PopupHints::Keybinds(hints_items),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::KeyMatcher;
    use crate::keybinds::Keybinds;
    use crate::pinstar::state::PinstarState;
    use ratatui::backend::TestBackend;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_draw_pinstar_view_with_editor() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.canvas");
        {
            let mut file = File::create(&path).unwrap();
            file.write_all(b"{\"nodes\":[],\"edges\":[]}").unwrap();
        }

        let keybinds = Keybinds::default();
        let seq_matcher = KeyMatcher::default();
        let mut state = PinstarState::load(&path, keybinds, seq_matcher).unwrap();
        state.show_editor_pane = true;
        state.editor_focus = true;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let data_dir = dir.path().join("data");
        let config_dir = dir.path().join("config");
        let notes_dir = dir.path().join("notes");
        let templates_dir = dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        let mut app = crate::app::App::new(storage).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_pinstar_view(f, &mut state, &mut app, area, None);
            })
            .unwrap();

        // Confirm we can also render with editor_focus = false
        state.editor_focus = false;
        terminal
            .draw(|f| {
                let area = f.area();
                draw_pinstar_view(f, &mut state, &mut app, area, None);
            })
            .unwrap();
    }

    #[test]
    #[ignore = "performance test, run manually"]
    fn pinstar_large_canvas_render_perf() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.canvas");

        use crate::pinstar::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
        let cols = 30usize;
        let rows = 20usize;
        let n = cols * rows; // 600
        let nodes: Vec<CanvasNode> = (0..n)
            .map(|i| {
                let cx = (i % cols) as f64 * 136.0;
                let cy = (i / cols) as f64 * 170.0;
                CanvasNode::Text(TextNode {
                    id: format!("n{i}"),
                    x: cx,
                    y: cy,
                    width: 60.0,
                    height: 40.0,
                    text: format!("node {i}"),
                    title: None,
                    color: None,
                })
            })
            .collect();
        let edges: Vec<CanvasEdge> = (0..n)
            .flat_map(|i| {
                [1usize, 2, 3].into_iter().filter_map(move |o| {
                    let to = (i + o) % n;
                    if to == i {
                        return None;
                    }
                    Some(CanvasEdge {
                        id: format!("e{i}_{to}"),
                        from_node: format!("n{i}"),
                        from_side: Some("right".to_string()),
                        to_node: format!("n{to}"),
                        to_side: Some("left".to_string()),
                        label: None,
                        color: None,
                        style: crate::pinstar::data::EdgeStyle::Solid,
                    })
                })
            })
            .collect();
        let data = CanvasData { nodes, edges };

        {
            let content = serde_json::to_string(&data).unwrap();
            std::fs::write(&path, content).unwrap();
        }

        let keybinds = crate::keybinds::Keybinds::default();
        let seq_matcher = crate::keybinds::KeyMatcher::default();
        let mut state =
            crate::pinstar::state::PinstarState::load(&path, keybinds, seq_matcher).unwrap();
        state.zoom = 0.05;
        state.viewport_x = 2000.0;
        state.viewport_y = 1700.0;

        let data_dir = dir.path().join("data");
        let config_dir = dir.path().join("config");
        let notes_dir = dir.path().join("notes");
        let templates_dir = dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        let mut app = crate::app::App::new(storage).unwrap();

        let backend = ratatui::backend::TestBackend::new(200, 80);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        // warm draw (fills any caches), then timed draw.
        terminal
            .draw(|f| {
                let area = f.area();
                super::draw_pinstar_view(f, &mut state, &mut app, area, None);
            })
            .unwrap();
        let t0 = std::time::Instant::now();
        terminal
            .draw(|f| {
                let area = f.area();
                super::draw_pinstar_view(f, &mut state, &mut app, area, None);
            })
            .unwrap();
        let elapsed = t0.elapsed();
        eprintln!("pinstar {n}n/1800e draw: {:?}", elapsed);
        // Budget placeholder: measured post-optimization baseline ~M ms; guard <= 2*M.
        assert!(elapsed.as_millis() < 100, "draw too slow: {:?}", elapsed);
    }
}
