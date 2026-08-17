use crate::app::ViewMode;
use crate::app_theme::AppThemeColors;
use crate::events::contains_cell;
use crate::keybinds::CanvasAction;
use crate::pinstar::state::PinstarState;
use ratatui::{prelude::*, widgets::*};

struct Proj {
    is_group: bool,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
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
                .find(|e| e.0.eq_ignore_ascii_case(s))
            {
                entry.2
            } else {
                theme.accent
            }
        }
        _ => theme.accent,
    }
}

fn get_edge_color(color: Option<&str>, selected: bool, theme: &AppThemeColors) -> Color {
    if selected {
        return theme.accent;
    }
    match color {
        Some(s) if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
            Color::Rgb(r, g, b)
        }
        _ => theme.muted,
    }
}

/// Color for an edge's text in the overlay: the edge's own color when set,
/// else the default text color.
fn edge_overlay_color(color: Option<&str>, theme: &AppThemeColors) -> Color {
    match color {
        Some(s) if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
            Color::Rgb(r, g, b)
        }
        _ => theme.text,
    }
}

/// Dimmed variant of a color, so "(no title)" text stays muted but still
/// hints at the edge's own color.
fn muted_edge_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(r / 2, g / 2, b / 2),
        _ => color,
    }
}

/// A resolved row for the edge-list overlay.
struct OverlayEdgeRow {
    index: usize,
    from_title: Option<String>,
    to_title: Option<String>,
    color: Option<String>,
}
#[allow(clippy::too_many_arguments)]
fn draw_braille_segment(
    buf: &mut ratatui::buffer::Buffer,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
    style: crate::pinstar::data::EdgeStyle,
    color: Color,
) {
    let mut current_x = x1;
    let mut current_y = y1;
    let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let steps = (dist * 4.0) as usize;
    if steps == 0 {
        return;
    }
    let ddx = (x2 - x1) / steps as f64;
    let ddy = (y2 - y1) / steps as f64;
    for step in 0..=steps {
        let draw = match style {
            crate::pinstar::data::EdgeStyle::Solid => true,
            crate::pinstar::data::EdgeStyle::Dashed => step % 16 < 8,
            crate::pinstar::data::EdgeStyle::Dotted => step % 8 == 0,
        };
        if draw
            && current_x >= view_left
            && current_x < view_right
            && current_y >= view_top
            && current_y < view_bottom
        {
            let cell_x = current_x as u16;
            let cell_y = current_y as u16;
            let dot_x = ((current_x - cell_x as f64) * 2.0) as u16;
            let dot_y = ((current_y - cell_y as f64) * 4.0) as u16;
            crate::ui::braille::set_braille_dot(buf, cell_x, cell_y, dot_x, dot_y, color);
        }
        current_x += ddx;
        current_y += ddy;
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
                sx,
                sy,
                sw,
                sh,
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

    let (cx1, cy1) = state.screen_to_canvas(canvas_area.left(), canvas_area.top(), canvas_area);
    let (cx2, cy2) = state.screen_to_canvas(canvas_area.right(), canvas_area.bottom(), canvas_area);
    crate::ui::draw_canvas_grid(
        frame,
        canvas_area,
        state.grid,
        crate::ui::CanvasGridProjection {
            world_left: cx1.min(cx2),
            world_right: cx1.max(cx2),
            world_top: cy1.min(cy2),
            world_bottom: cy1.max(cy2),
            origin_col: origin_x - vx * z,
            origin_row: origin_y - vy * z,
            cols_per_world_x: z,
            rows_per_world_y: z,
        },
        theme.muted,
        state.zoom,
    );
    // Select-rect pass: drawn AFTER group/node passes.
    // Uses buffer-cell bg mutation to avoid destroying node/edge characters.
    // Done later, after all rendering — see below.

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

        let is_primary = state.selection.primary.as_deref() == Some(g.id.as_str());
        let is_selected = is_primary || state.selection.extra.contains(g.id.as_str());
        let is_editing = is_primary && state.floating_editor.is_some();
        let base_color = get_node_color(g.color.as_deref(), theme);
        let border_color = if is_editing { theme.accent } else { base_color };

        let mut label = g.label.as_deref().unwrap_or("Group").to_string();
        if is_editing {
            label = format!("[EDITING] {label}");
        }

        let title_sh = (60.0 * z).ceil() as u16;
        let title_h = title_sh.max(1).min(node_rect.height);
        let title_rect = Rect::new(node_rect.x, node_rect.y, node_rect.width, title_h);

        let is_hovered = !is_selected
            && canvas_mouse_pos.is_some_and(|(col, row)| contains_cell(title_rect, col, row));

        let title_style = if is_hovered {
            theme.hover_style()
        } else {
            let mut s = Style::default().bg(base_color);
            if let Some(c) = theme.bg {
                s = s.fg(c);
            } else {
                s = s.fg(ratatui::style::Color::Black);
            }
            s
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(ratatui::text::Line::from(format!(" {label} ")).style(title_style))
            .style(if is_hovered {
                theme.hover_style()
            } else {
                theme.bg_style()
            });

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
        for edge in &state.data.edges {
            let Some(seg) = state.get_edge_segments(edge) else {
                continue;
            };
            let is_edge_selected = state.selected_edge_id.as_deref() == Some(edge.id.as_str());
            let edge_color = get_edge_color(edge.color.as_deref(), is_edge_selected, theme);
            for &(sx, sy, ex, ey) in seg.0.iter().take(seg.1) {
                let sfx = (sx - vx) * z + origin_x;
                let sfy = (sy - vy) * z + origin_y;
                let stx = (ex - vx) * z + origin_x;
                let sty = (ey - vy) * z + origin_y;
                // Cull per segment
                let min_x = sfx.min(stx);
                let max_x = sfx.max(stx);
                let min_y = sfy.min(sty);
                let max_y = sfy.max(sty);
                if max_x < view_left
                    || min_x > view_right
                    || max_y < view_top
                    || min_y > view_bottom
                {
                    continue;
                }
                draw_braille_segment(
                    frame.buffer_mut(),
                    sfx,
                    sfy,
                    stx,
                    sty,
                    view_left,
                    view_right,
                    view_top,
                    view_bottom,
                    edge.style,
                    edge_color,
                );
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

        // Multi-select: check both single-selection and multi-selection
        let is_primary = state.selection.primary.as_deref() == Some(node.id());
        let is_selected = is_primary || state.selection.extra.contains(node.id());
        let is_editing = is_primary && state.floating_editor.is_some();

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
    state.edge_overlay_rect = None;

    if let Some(editor) = &mut state.floating_editor
        && let Some(node_id) = &state.selection.primary
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

    // Select-rect overlay: drawn AFTER all nodes/edges/editor.
    // Uses buffer-cell bg mutation so node characters stay visible.
    if let (Some(start), Some(curr)) = (state.marquee.start, state.marquee.end)
        && state.right_down_screen.is_some()
    {
        let (sx1, sy1) = ((start.0 - vx) * z + origin_x, (start.1 - vy) * z + origin_y);
        let (sx2, sy2) = ((curr.0 - vx) * z + origin_x, (curr.1 - vy) * z + origin_y);
        let (min_x, max_x) = if sx1 < sx2 { (sx1, sx2) } else { (sx2, sx1) };
        let (min_y, max_y) = if sy1 < sy2 { (sy1, sy2) } else { (sy2, sy1) };
        let left = (min_x
            .max(canvas_area.left() as f64)
            .min(canvas_area.right() as f64)) as u16;
        let top = (min_y
            .max(canvas_area.top() as f64)
            .min(canvas_area.bottom() as f64)) as u16;
        let width = ((max_x - min_x).max(1.0)) as u16;
        let height = ((max_y - min_y).max(1.0)) as u16;
        let fill = crate::ui::canvas_overlay::muted_canvas_selection_fill(
            theme.accent,
            theme.highlight_bg,
        );
        let screen_rect = ratatui::layout::Rect::new(left, top, width, height);
        crate::ui::canvas_overlay::draw_canvas_rect_filled(frame, screen_rect, fill);
    }
    // Edge-list overlay: when a node is selected, list its connected edges
    // (1..n) in a bottom-right legend panel, titles colored by edge color.
    if state.selection.primary.is_some() {
        let edges = state.selected_node_edges();
        if !edges.is_empty() {
            let no_title = "(no title)".to_string();
            let resolved: Vec<OverlayEdgeRow> = edges
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let title_of = |id: &str| {
                        state
                            .data
                            .nodes
                            .iter()
                            .find(|n| n.id() == id)
                            .and_then(|n| n.title())
                            .map(|t| t.to_string())
                    };
                    OverlayEdgeRow {
                        index: i,
                        from_title: title_of(&e.from_node),
                        to_title: title_of(&e.to_node),
                        color: e.color.clone(),
                    }
                })
                .collect();

            let max_len = resolved
                .iter()
                .map(|r| {
                    format!(
                        "{} {} → {}",
                        r.index + 1,
                        r.from_title.as_deref().unwrap_or(no_title.as_str()),
                        r.to_title.as_deref().unwrap_or(no_title.as_str())
                    )
                    .chars()
                    .count()
                })
                .max()
                .unwrap_or(0);
            let overlay_width = (max_len + 4) as u16;
            let overlay_height = (edges.len() + 2) as u16;
            let overlay_rect = Rect::new(
                canvas_area.x + canvas_area.width.saturating_sub(overlay_width),
                canvas_area.y + canvas_area.height.saturating_sub(overlay_height),
                overlay_width,
                overlay_height,
            );
            let rows: Vec<ratatui::text::Line> =
                resolved
                    .iter()
                    .map(|r| {
                        let edge_color = edge_overlay_color(r.color.as_deref(), theme);
                        let mut spans = vec![Span::styled(
                            format!("{} ", r.index + 1),
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        )];
                        match &r.from_title {
                            Some(title) => spans
                                .push(Span::styled(title.clone(), Style::default().fg(edge_color))),
                            None => spans.push(Span::styled(
                                no_title.clone(),
                                Style::default().fg(muted_edge_color(edge_color)),
                            )),
                        }
                        spans.push(Span::styled(" → ", Style::default().fg(theme.muted)));
                        match &r.to_title {
                            Some(title) => spans
                                .push(Span::styled(title.clone(), Style::default().fg(edge_color))),
                            None => spans.push(Span::styled(
                                no_title.clone(),
                                Style::default().fg(muted_edge_color(edge_color)),
                            )),
                        }
                        ratatui::text::Line::from(spans)
                    })
                    .collect();
            let overlay = Paragraph::new(rows).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" EDGES ")
                    .border_style(Style::default().fg(theme.accent))
                    .style(theme.bg_style()),
            );
            frame.render_widget(Clear, overlay_rect);
            frame.render_widget(overlay, overlay_rect);
            // Hover highlight on overlay rows.
            let hover_inner = Rect::new(
                overlay_rect.x + 1,
                overlay_rect.y + 1,
                overlay_rect.width.saturating_sub(2),
                overlay_rect.height.saturating_sub(2),
            );
            crate::ui::paint_list_hover(
                frame,
                hover_inner,
                &ratatui::widgets::ListState::default(),
                edges.len(),
                mouse_pos,
                theme.hover_style(),
            );
            state.edge_overlay_rect = Some(overlay_rect);
        }
    }

    let hint_area = Rect::new(
        total_area.x,
        total_area.bottom().saturating_sub(1),
        total_area.width,
        1,
    );
    let hint_line = if state.footer_hint.is_empty() {
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
        crate::ui::canvas_menu::render_canvas_context_menu(frame, area, menu, theme, mouse_pos);
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
