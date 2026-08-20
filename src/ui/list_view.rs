use super::{
    PreviewHeaderInfo, build_tab_spans, draw_dim_vline, draw_status_bar, draw_view_title_bar,
    draw_view_title_bar_with_tabs, format_keybind_hints, popup_hint_line, preview_spans,
};
use crate::app::{App, VIRTUAL_PINNED_PATH, VIRTUAL_SMART_PATH, VIRTUAL_SUBNOTES_PATH, ViewMode};
#[cfg(test)]
use crate::app_theme::AppThemeColors;
use crate::keybinds::ListAction;
use ratatui::{prelude::*, widgets::*};
use unicode_width::UnicodeWidthStr;

const GRID_TILE_W: u16 = 10; // outer width incl. border
const GRID_TILE_H: u16 = 5; // outer height incl. border
const GRID_GAP: u16 = 1; // space between tiles (h and v)
const GRID_LEFT_MARGIN: u16 = 2; // left inset inside list_area
const GRID_TOP_MARGIN: u16 = 3; // top inset inside list_area

/// Viewport base span for SubnoteGraph zoom/pan: layout_r(10) + parent_r(3) + 2 padding.
/// Shared between renderer and mouse handler to prevent drift.
pub(crate) const SUBNOTE_GRAPH_BASE_SPAN: f64 = 15.0;

/// Viewport base span for FolderGraph zoom/pan: layout_r(10) + parent_r(3) + 2 padding.
/// Shared between renderer and mouse handler to prevent drift.
pub(crate) const FOLDER_GRAPH_BASE_SPAN: f64 = 15.0;

/// A hollow (outlined) circle drawn on a ratatui Canvas via Line segments.
struct HollowCircle {
    cx: f64,
    cy: f64,
    radius: f64,
    color: ratatui::style::Color,
}

impl ratatui::widgets::canvas::Shape for HollowCircle {
    fn draw(&self, painter: &mut ratatui::widgets::canvas::Painter) {
        let steps = 48u32;
        for i in 0..steps {
            let a1 = (i as f64) * std::f64::consts::TAU / steps as f64;
            let a2 = ((i + 1) as f64) * std::f64::consts::TAU / steps as f64;
            ratatui::widgets::canvas::Line {
                x1: self.cx + self.radius * a1.cos(),
                y1: self.cy + self.radius * a1.sin(),
                x2: self.cx + self.radius * a2.cos(),
                y2: self.cy + self.radius * a2.sin(),
                color: self.color,
            }
            .draw(painter);
        }
    }
}

/// Compute the per-section rectangles within the calendar strip area.
pub(crate) fn section_rects(cal_rect: Rect, active: &[crate::config::NotesSection]) -> Vec<Rect> {
    match active.len() {
        0 => Vec::new(),
        1 => {
            let w = cal_rect.width / 2;
            let x = cal_rect.x + (cal_rect.width - w) / 2;
            vec![Rect::new(x, cal_rect.y, w, cal_rect.height)]
        }
        _ => {
            let cs = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(cal_rect);
            vec![cs[0], cs[1]]
        }
    }
}

fn draw_strip_draw(
    frame: &mut Frame,
    rect: Rect,
    app: &App,
    bottom_border: bool,
    strip_rect: Rect,
) {
    use ratatui::widgets::{Block, Borders};

    // Strip-wide border so that a single centered section still gets a
    // full-width border.
    let borders = if bottom_border {
        Borders::BOTTOM
    } else {
        Borders::TOP
    };
    let strip_block = Block::default()
        .style(app.app_theme.bg_style())
        .borders(borders)
        .border_style(
            ratatui::style::Style::default()
                .fg(app.app_theme.border)
                .bg(app.app_theme.bg.unwrap_or(ratatui::style::Color::Reset)),
        );
    frame.render_widget(&strip_block, strip_rect);
    let inner = strip_block.inner(strip_rect);

    // Content area = section rect clipped to the border's inner area.
    let content_x = rect.x.max(inner.x);
    let content_y = rect.y.max(inner.y);
    let content_w = (rect.right().min(inner.right())).saturating_sub(content_x);
    let content_h = (rect.bottom().min(inner.bottom())).saturating_sub(content_y);
    let content_area = Rect::new(content_x, content_y, content_w, content_h);

    let (_, data) = match app.draw_preview.as_ref() {
        Some(pair) => pair,
        None => {
            let line = popup_hint_line(&app.app_theme, "No .draw file");
            let p = ratatui::widgets::Paragraph::new(line)
                .style(app.app_theme.bg_style())
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, content_area);
            return;
        }
    };
    if data.elements.is_empty() || content_area.width == 0 || content_area.height == 0 {
        let line = popup_hint_line(&app.app_theme, "No .draw file");
        let p = ratatui::widgets::Paragraph::new(line)
            .style(app.app_theme.bg_style())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, content_area);
        return;
    }
    let grid = crate::snapshot::render_draw_snapshot_with_bg(
        data,
        &app.app_theme,
        app.config.ui.icon_mode,
        content_area.width,
        content_area.height,
        1.0,
        0.0,
        0.0,
        app.app_theme.bg,
    );
    frame.render_widget(crate::snapshot::RenderedSnapshot::new(&grid), content_area);
}

fn draw_strip_graf(
    frame: &mut Frame,
    rect: Rect,
    app: &mut App,
    bottom_border: bool,
    strip_rect: Rect,
) {
    match app.graph_preview.as_mut() {
        Some(gs) => {
            // Progressive settle: 10 steps per frame to avoid blocking
            if !gs.is_settled && app.graph_preview_steps < 100 {
                for _ in 0..10 {
                    crate::graf::physics::simulation_step(gs, 0.12);
                    app.graph_preview_steps += 1;
                    if gs.is_settled {
                        break;
                    }
                }
            }
            let (wx_min, wx_max, wy_min, wy_max) = gs.graph_bounds;
            if wx_max - wx_min <= 0.0 || wy_max - wy_min <= 0.0 {
                return;
            }

            // Border spans the full strip rect (so it extends across the
            // entire strip even when a single section is centered)
            let borders = if bottom_border {
                Borders::BOTTOM
            } else {
                Borders::TOP
            };
            let block = Block::default()
                .style(app.app_theme.bg_style())
                .borders(borders)
                .border_style(
                    Style::default()
                        .fg(app.app_theme.border)
                        .bg(app.app_theme.bg.unwrap_or(Color::Reset)),
                );
            let outer_inner = block.inner(strip_rect);
            frame.render_widget(block, strip_rect);

            // Content area = intersection of section rect with border inner
            let content_x = rect.x.max(outer_inner.x);
            let content_y = rect.y.max(outer_inner.y);
            let content_w = (rect.right().min(outer_inner.right())).saturating_sub(content_x);
            let content_h = (rect.bottom().min(outer_inner.bottom())).saturating_sub(content_y);
            if content_w == 0 || content_h == 0 {
                return;
            }
            let iw = content_w as usize;
            let ih = content_h as usize;
            let sub_h = ih * 2; // 2× vertical sub-pixels for halfblocks
            let world_w = wx_max - wx_min;
            let world_h = wy_max - wy_min;

            // Preserve world aspect ratio — constrain the tighter axis and center
            let av_cols = content_w as f64;
            let av_rows_sub = sub_h as f64;
            let world_aspect = world_w / world_h;
            let grid_aspect = av_cols / av_rows_sub;
            let (draw_cols, draw_rows_sub) = if grid_aspect > world_aspect {
                // grid wider → constrain horizontal
                (av_rows_sub * world_aspect, av_rows_sub)
            } else {
                // grid taller → constrain vertical
                (av_cols, av_cols / world_aspect)
            };
            let col_off = ((av_cols - draw_cols) / 2.0) as isize;
            let row_off = ((av_rows_sub - draw_rows_sub) / 2.0) as isize;

            let world_to_col = |x: f64| -> usize {
                let t = (x - wx_min) / world_w;
                let v = (t * draw_cols).floor() as isize + col_off;
                v.clamp(0, (iw as isize) - 1) as usize
            };
            let world_to_subrow = |y: f64| -> usize {
                let t = (wy_max - y) / world_h;
                let v = (t * draw_rows_sub).floor() as isize + row_off;
                v.clamp(0, (sub_h as isize) - 1) as usize
            };

            // Build per-node colors via the render cache (respects graf node_color_mode config)
            let graph = gs.simulation.get_graph();
            let colors = app.config.theme_colors();
            let mut cache = gs.render_cache.lock();
            if cache.topology_dirty {
                cache.rebuild_topology(graph, &app.config, &colors, false);
            }

            // Map nodes to sub-pixel grid with per-node colors
            let grid_size = sub_h * iw;
            let mut grid: Vec<Option<Color>> = Vec::with_capacity(grid_size);
            grid.resize(grid_size, None);

            for idx in graph.node_indices() {
                let node = &graph[idx];
                let color = cache
                    .node_own_color
                    .get(&idx)
                    .copied()
                    .unwrap_or(app.app_theme.muted);
                let col = world_to_col(node.location.x as f64);
                let sub_row = world_to_subrow(node.location.y as f64);
                grid[sub_row * iw + col] = Some(color);
            }
            drop(cache);

            // Render using halfblocks (▀ top-half, ▄ bottom-half)
            let buf = frame.buffer_mut();
            for cell_row in 0..ih {
                let top_sub = cell_row * 2;
                let bot_sub = cell_row * 2 + 1;
                for col in 0..iw {
                    let top_color = grid[top_sub * iw + col];
                    let bot_color = grid[bot_sub * iw + col];
                    let x = content_x + col as u16;
                    let y = content_y + cell_row as u16;
                    let cell = match buf.cell_mut((x, y)) {
                        Some(c) => c,
                        None => continue,
                    };
                    let bg = app.app_theme.bg;
                    match (top_color, bot_color) {
                        (None, None) => {}
                        (Some(tc), None) => {
                            cell.set_symbol("▀");
                            cell.set_style(Style::default().fg(tc).bg(bg.unwrap_or(Color::Reset)));
                        }
                        (None, Some(bc)) => {
                            cell.set_symbol("▄");
                            cell.set_style(Style::default().fg(bc).bg(bg.unwrap_or(Color::Reset)));
                        }
                        (Some(tc), Some(bc)) => {
                            cell.set_symbol("▄");
                            cell.set_style(Style::default().fg(bc).bg(tc));
                        }
                    }
                }
            }
        }
        None => {
            let line = popup_hint_line(&app.app_theme, "Graph unavailable");
            let p = ratatui::widgets::Paragraph::new(line)
                .style(app.app_theme.bg_style())
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, rect);
        }
    }
}

/// Shorten a line segment so each endpoint stops at the border of a circle
/// of the given radius centered on that endpoint. Returns the original
/// endpoints unchanged when the two circles overlap (distance <= r1 + r2)
/// or the endpoints coincide, so degenerate cases don't produce NaNs.
fn shorten_segment_to_borders(
    x1: f64,
    y1: f64,
    r1: f64,
    x2: f64,
    y2: f64,
    r2: f64,
) -> (f64, f64, f64, f64) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist = dx.hypot(dy);
    if dist <= r1 + r2 || dist == 0.0 {
        return (x1, y1, x2, y2);
    }
    let ux = dx / dist;
    let uy = dy / dist;
    (x1 + ux * r1, y1 + uy * r1, x2 - ux * r2, y2 - uy * r2)
}

/// Compute positions on a circle of radius `r` for `n` items, starting from top
/// (angle = -π/2). Used by both SubnoteGraph and FolderGraph renderers.
pub(crate) fn orbit_positions(n: usize, r: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let angle = i as f64 * std::f64::consts::TAU / n as f64 - std::f64::consts::FRAC_PI_2;
            (r * angle.cos(), r * angle.sin())
        })
        .collect()
}
/// Render subnote graph on a ratatui Canvas with hollow circles, zoom/pan, and content reveal.
pub fn render_subnote_graph_static(
    frame: &mut Frame,
    rect: Rect,
    parent_title: &str,
    subnotes: &[crate::storage::SubNote],
    theme: &crate::app_theme::AppThemeColors,
) {
    use ratatui::symbols::Marker;
    use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};

    let zoom = 1.0;
    let pan_x = 0.0;
    let pan_y = 0.0;

    // Background
    let bg = theme.preview_bg_style();
    frame.render_widget(Block::default().style(bg), rect);

    if subnotes.is_empty() {
        let line = Line::from(vec![Span::styled(
            "No subnotes",
            Style::default().fg(theme.muted),
        )]);
        let p = ratatui::widgets::Paragraph::new(line)
            .style(theme.preview_bg_style())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, rect);
        return;
    }

    // World-coordinate layout: parent at origin, subnotes on a circle.
    let layout_r = 10.0_f64;
    let parent_r = 3.0_f64;
    let sub_r = 1.5_f64;
    let positions = orbit_positions(subnotes.len(), layout_r);

    // Viewport bounds from zoom/pan.
    let aspect = rect.width as f64 / rect.height as f64;
    let cell_aspect = 2.0; // terminal cells are ~2× taller than wide
    let span_x = SUBNOTE_GRAPH_BASE_SPAN / zoom;
    let span_y = span_x * cell_aspect / aspect;
    let x_bounds = [pan_x - span_x, pan_x + span_x];
    let y_bounds = [pan_y - span_y, pan_y + span_y];

    // For on-screen sizing of circles and title offsets.
    let cells_per_world = rect.width as f64 / (2.0 * span_x);

    // Build shapes.
    let mut edges: Vec<CanvasLine> = Vec::new();
    for &(sx, sy) in &positions {
        let (x1, y1, x2, y2) = shorten_segment_to_borders(0.0, 0.0, parent_r, sx, sy, sub_r);
        edges.push(CanvasLine {
            x1,
            y1,
            x2,
            y2,
            color: theme.border,
        });
    }
    // Wikilink edges: subnote -> subnote.
    let title_to_idx: std::collections::HashMap<String, usize> = subnotes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.title.to_lowercase(), i))
        .collect();
    let mut drawn: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for (i, sub) in subnotes.iter().enumerate() {
        let (sx, sy) = positions[i];
        for link in crate::storage::extract_wikilinks(&sub.content) {
            if let Some(&j) = title_to_idx.get(&link.to_lowercase())
                && j != i
            {
                let key = if i < j { (i, j) } else { (j, i) };
                if drawn.insert(key) {
                    let (tx, ty) = positions[j];
                    let (x1, y1, x2, y2) = shorten_segment_to_borders(sx, sy, sub_r, tx, ty, sub_r);
                    edges.push(CanvasLine {
                        x1,
                        y1,
                        x2,
                        y2,
                        color: theme.success,
                    });
                }
            }
        }
    }
    let parent_circle = HollowCircle {
        cx: 0.0,
        cy: 0.0,
        radius: parent_r,
        color: theme.accent,
    };
    let sub_circles: Vec<HollowCircle> = positions
        .iter()
        .map(|&(sx, sy)| HollowCircle {
            cx: sx,
            cy: sy,
            radius: sub_r,
            color: theme.tag,
        })
        .collect();

    // Render Canvas with Braille marker.
    let canvas = Canvas::default()
        .background_color(theme.preview_bg().unwrap_or(ratatui::style::Color::Reset))
        .block(Block::default().style(bg))
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for edge in &edges {
                ctx.draw(edge);
            }
            ctx.draw(&parent_circle);
            for sc in &sub_circles {
                ctx.draw(sc);
            }
        });
    frame.render_widget(canvas, rect);

    // Post-canvas: world -> screen transform for text overlay.
    let world_to_screen = |wx: f64, wy: f64| -> (f64, f64) {
        let col =
            rect.x as f64 + (wx - x_bounds[0]) / (x_bounds[1] - x_bounds[0]) * rect.width as f64;
        let row = rect.y as f64 + rect.height as f64
            - (wy - y_bounds[0]) / (y_bounds[1] - y_bounds[0]) * rect.height as f64;
        (col, row)
    };
    let buf = frame.buffer_mut();
    let max_title_len = (rect.width as f64 * 0.2 / zoom.max(1.0)).max(4.0) as usize;

    // Parent title.
    let (px, py) = world_to_screen(0.0, parent_r);
    draw_title_above(
        buf,
        px,
        py,
        parent_title,
        max_title_len,
        rect,
        Style::default().fg(theme.fg),
    );

    // Subnote titles.
    for (&(sx, sy), sub) in positions.iter().zip(subnotes.iter()) {
        let (scx, scy) = world_to_screen(sx, sy);
        draw_title_above(
            buf,
            scx,
            scy - sub_r * cells_per_world,
            &sub.title,
            max_title_len,
            rect,
            Style::default().fg(theme.fg),
        );
    }
}

/// Render a hierarchical folder graph on a ratatui Canvas with hollow circles, zoom/pan,
/// and focus transitions into subfolders (re-focus) or notes (content card).
pub fn render_folder_graph_static(
    frame: &mut Frame,
    rect: Rect,
    focused_label: &str,
    children: &[crate::list_view::FolderGraphNode],
    theme: &crate::app_theme::AppThemeColors,
) {
    use ratatui::symbols::Marker;
    use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};

    let zoom = 1.0;
    let pan_x = 0.0;
    let pan_y = 0.0;

    // Background
    let bg = theme.preview_bg_style();
    frame.render_widget(Block::default().style(bg), rect);

    if children.is_empty() {
        let line = Line::from(vec![Span::styled(
            "Empty folder",
            Style::default().fg(theme.muted),
        )]);
        let p = ratatui::widgets::Paragraph::new(line)
            .style(theme.preview_bg_style())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, rect);
        return;
    }

    // World-coordinate layout: parent at origin, children on a circle.
    let parent_r = 3.0_f64;
    let child_r = 1.5_f64;

    // Viewport bounds from zoom/pan.
    let aspect = rect.width as f64 / rect.height as f64;
    let cell_aspect = 2.0;
    let span_x = FOLDER_GRAPH_BASE_SPAN / zoom;
    let span_y = span_x * cell_aspect / aspect;
    let x_bounds = [pan_x - span_x, pan_x + span_x];
    let y_bounds = [pan_y - span_y, pan_y + span_y];
    let cells_per_world = rect.width as f64 / (2.0 * span_x);
    // Build shapes: parent->child edges.
    let mut edges: Vec<CanvasLine> = Vec::new();
    for child in children {
        let (x1, y1, x2, y2) =
            shorten_segment_to_borders(0.0, 0.0, parent_r, child.x, child.y, child_r);
        edges.push(CanvasLine {
            x1,
            y1,
            x2,
            y2,
            color: theme.border,
        });
    }
    // Wikilink edges between note children within the focused folder.
    let notes_only: Vec<&crate::list_view::FolderGraphNode> =
        children.iter().filter(|c| c.is_note).collect();
    if notes_only.len() > 1 {
        let title_to_idx: std::collections::HashMap<String, usize> = notes_only
            .iter()
            .enumerate()
            .map(|(i, n)| (n.label.to_lowercase(), i))
            .collect();
        let mut drawn: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for (i, child) in notes_only.iter().enumerate() {
            for link in &child.links {
                if let Some(&j) = title_to_idx.get(&link.to_lowercase())
                    && j != i
                {
                    let key = if i < j { (i, j) } else { (j, i) };
                    if drawn.insert(key) {
                        let tx = notes_only[j].x;
                        let ty = notes_only[j].y;
                        let (x1, y1, x2, y2) =
                            shorten_segment_to_borders(child.x, child.y, child_r, tx, ty, child_r);
                        edges.push(CanvasLine {
                            x1,
                            y1,
                            x2,
                            y2,
                            color: theme.success,
                        });
                    }
                }
            }
        }
    }

    let parent_circle = HollowCircle {
        cx: 0.0,
        cy: 0.0,
        radius: parent_r,
        color: theme.accent,
    };
    let child_circles: Vec<HollowCircle> = children
        .iter()
        .map(|c| HollowCircle {
            cx: c.x,
            cy: c.y,
            radius: child_r,
            color: if c.is_note { theme.tag } else { theme.folder },
        })
        .collect();

    // Render Canvas
    let canvas = Canvas::default()
        .background_color(theme.preview_bg().unwrap_or(ratatui::style::Color::Reset))
        .block(Block::default().style(bg))
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for edge in &edges {
                ctx.draw(edge);
            }
            ctx.draw(&parent_circle);
            for cc in &child_circles {
                ctx.draw(cc);
            }
        });
    frame.render_widget(canvas, rect);

    // Post-canvas: world -> screen transform for text overlay.
    let world_to_screen = |wx: f64, wy: f64| -> (f64, f64) {
        let col =
            rect.x as f64 + (wx - x_bounds[0]) / (x_bounds[1] - x_bounds[0]) * rect.width as f64;
        let row = rect.y as f64 + rect.height as f64
            - (wy - y_bounds[0]) / (y_bounds[1] - y_bounds[0]) * rect.height as f64;
        (col, row)
    };
    let buf = frame.buffer_mut();
    let max_title_len = (rect.width as f64 * 0.2 / zoom.max(1.0)).max(4.0) as usize;

    // Parent title.
    let (px, py) = world_to_screen(0.0, parent_r);
    draw_title_above(
        buf,
        px,
        py,
        focused_label,
        max_title_len,
        rect,
        Style::default().fg(theme.fg),
    );

    // Child labels.
    for child in children {
        let (scx, scy) = world_to_screen(child.x, child.y);
        draw_title_above(
            buf,
            scx,
            scy - child_r * cells_per_world,
            &child.label,
            max_title_len,
            rect,
            Style::default().fg(theme.fg),
        );
    }
}
/// Draw `text` centered above position (x, y_top) in the buffer, clamped to rect.
fn draw_title_above(
    buf: &mut ratatui::buffer::Buffer,
    x: f64,
    y_top: f64,
    text: &str,
    max_len: usize,
    rect: Rect,
    style: Style,
) {
    let truncated = crate::graf::util::truncate(text, max_len);
    if truncated.is_empty() {
        return;
    }
    let title_y = (y_top - 1.0).round() as u16;
    let title_y = title_y.max(rect.y);
    let start_x = (x - truncated.chars().count() as f64 / 2.0).round() as u16;
    for (col, ch) in (start_x..).zip(truncated.chars()) {
        if col >= rect.x
            && col < rect.right()
            && title_y >= rect.y
            && title_y < rect.bottom()
            && let Some(cell) = buf.cell_mut((col, title_y))
        {
            cell.set_char(ch).set_style(style);
        }
    }
}

pub fn draw_list_view(frame: &mut Frame, app: &mut App) {
    let saved_mouse_pos = app.mouse_pos;
    if app.popups.active.is_some() {
        app.mouse_pos = None;
    }
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    let title = if app.layout_edit {
        "Notes - Editing Layout"
    } else {
        "Notes"
    };
    let in_select_mode = app.list.list_mode == crate::list_view::ListMode::Select;
    if in_select_mode {
        let badge_text = format!(
            " SELECT MODE \u{2014} {} selected ",
            app.list.selected_indices.len()
        );
        let header_rect = chunks[0];
        frame.render_widget(Clear, header_rect);
        frame.render_widget(
            Block::default().style(Style::default().bg(app.app_theme.accent)),
            header_rect,
        );
        let text_width = badge_text.chars().count() as u16;
        let label_x = header_rect.x + (header_rect.width.saturating_sub(text_width)) / 2;
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                badge_text,
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))),
            Rect::new(label_x, header_rect.y, text_width, 1),
        );
    } else {
        let preview_info = get_preview_info(app);
        let note = crate::statusline::active_note(app, ViewMode::List);
        let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::List);
        ctx.area = Some(chunks[0]);
        ctx.note = note;
        ctx.preview_info = preview_info.as_ref();
        if let Some(pi) = &preview_info {
            ctx.preview = Some(preview_spans(pi, &app.app_theme));
        }

        let detail = list_detail_value(app);
        if let Some(detail) = &detail {
            ctx.detail = Some(detail.spans_without(&[]));
            ctx.list_detail = Some(detail.clone());
        }
        if app.preview_fullscreen {
            let left_line = crate::statusline::render_header_left(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
            );
            let capacity = chunks[0]
                .width
                .saturating_sub(left_line.width().min(usize::from(chunks[0].width)) as u16);
            let right_line = crate::statusline::render_header_right(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
                Some(capacity),
            );
            draw_view_title_bar(
                frame,
                chunks[0],
                &app.app_theme,
                left_line,
                right_line,
                Some(app.status.as_ref()),
                app.load_spinner_tick,
            );
        } else if app.list.notes_layout == crate::config::NotesLayout::Grid {
            let mut tabs = vec![
                (
                    "Vault",
                    Some(crate::ui::get_icon(
                        "\u{f07b}",
                        "\u{1f4c1}",
                        app.config.ui.icon_mode,
                    )),
                ),
                (
                    "Pinned",
                    Some(crate::ui::get_icon(
                        "\u{f4cc}",
                        "\u{1f4cc}",
                        app.config.ui.icon_mode,
                    )),
                ),
            ];
            if app.config.list.smart_folders_enabled {
                tabs.push((
                    "Smart",
                    Some(crate::ui::get_icon(
                        "\u{f0e7}",
                        "\u{26a1}",
                        app.config.ui.icon_mode,
                    )),
                ));
            }
            // Subnotes tab always visible (like Pinned)
            tabs.push((
                "Subnotes",
                Some(crate::ui::get_icon(
                    "\u{f02c}",
                    "\u{1f3f7}",
                    app.config.ui.icon_mode,
                )),
            ));
            let selected_idx = if app.list.grid_folder == VIRTUAL_PINNED_PATH {
                1
            } else if app.list.grid_folder == VIRTUAL_SMART_PATH
                || app.list.grid_folder.starts_with('@')
            {
                2
            } else if app.list.grid_folder == VIRTUAL_SUBNOTES_PATH
                || crate::app::App::is_subnotes_parent_grid_path(&app.list.grid_folder)
            {
                if app.config.list.smart_folders_enabled {
                    3
                } else {
                    2
                }
            } else {
                0
            };
            let hovered = app.mouse_pos.and_then(|(col, row)| {
                if row == chunks[0].y {
                    let region = crate::ui::title_bar_tabs_region(chunks[0], title);
                    crate::ui::hit_test_tabs(
                        &tabs,
                        chunks[0].x,
                        chunks[0].width,
                        region.x,
                        col,
                        app.config.ui.tab_icons_only,
                        app.config.ui.icon_mode,
                    )
                } else {
                    None
                }
            });
            let tab_spans = build_tab_spans(
                &tabs,
                selected_idx,
                hovered,
                &app.app_theme,
                app.config.ui.tab_icons_only,
                app.config.ui.icon_mode,
            );

            let left_line = crate::statusline::render_header_left(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
            );
            let tab_width = tab_spans
                .iter()
                .map(|span| span.content.width() as u16)
                .fold(0u16, u16::saturating_add);
            let tabs_rect = crate::ui::title_bar_tabs_rect(chunks[0], title, tab_width);
            let occupied = chunks[0]
                .x
                .saturating_add(left_line.width().min(usize::from(chunks[0].width)) as u16)
                .max(tabs_rect.right());
            let right_line = crate::statusline::render_header_right(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
                Some(chunks[0].right().saturating_sub(occupied)),
            );
            draw_view_title_bar_with_tabs(
                frame,
                chunks[0],
                title,
                &app.app_theme,
                left_line,
                tab_spans,
                right_line,
                Some(app.status.as_ref()),
                app.load_spinner_tick,
            );
        } else {
            let left_line = crate::statusline::render_header_left(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
            );
            let capacity = chunks[0]
                .width
                .saturating_sub(left_line.width().min(usize::from(chunks[0].width)) as u16);
            let right_line = crate::statusline::render_header_right(
                &ctx,
                &app.config.statusline,
                ViewMode::List,
                &app.app_theme,
                Some(capacity),
            );
            draw_view_title_bar(
                frame,
                chunks[0],
                &app.app_theme,
                left_line,
                right_line,
                Some(app.status.as_ref()),
                app.load_spinner_tick,
            );
        }
    }

    let (list_area, preview_area, calendar_area) = list_view_layout(
        area,
        app.list.preview_enabled,
        app.preview_position,
        app.list.calendar_enabled,
        app.preview_fullscreen,
        app.list.preview_width_ratio,
        app.list.calendar_height,
        app.config.list.calendar_position,
    );
    if let Some(p) = preview_area {
        app.list.last_preview_pane_width = p.width;
        app.list.last_preview_pane_height = p.height;
    }

    if !app.preview_fullscreen {
        let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;

        if is_grid {
            app.list.grid_tiles.clear();
            app.list.last_scroll = None;

            // --- render directory breadcrumbs at the top of the list area ---
            let is_pinned = app.list.grid_folder == VIRTUAL_PINNED_PATH;
            let is_smart =
                app.list.grid_folder == VIRTUAL_SMART_PATH || app.list.grid_folder.starts_with('@');
            let is_subnotes = app.list.grid_folder == VIRTUAL_SUBNOTES_PATH
                || crate::app::App::is_subnotes_parent_grid_path(&app.list.grid_folder);
            let mut spans = Vec::new();
            if is_pinned {
                spans.push(Span::styled(
                    format!(
                        " {} Pinned",
                        crate::ui::get_icon("\u{f4cc}", "\u{1f4cc}", app.config.ui.icon_mode)
                    ),
                    Style::default()
                        .fg(app.app_theme.pinned)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if is_smart {
                let smart_icon =
                    crate::ui::get_icon("\u{f0e7}", "\u{26a1}", app.config.ui.icon_mode);
                let smart_text = format!(" {smart_icon} Smart");
                let smart_w = smart_text.chars().count() as u16;
                let is_hovered = app.mouse_pos.is_some_and(|(col, row)| {
                    row == list_area.y + 1
                        && col >= list_area.x
                        && col < list_area.x + smart_w
                        && app.list.grid_folder != VIRTUAL_SMART_PATH
                });
                spans.push(Span::styled(
                    smart_text,
                    if is_hovered {
                        app.app_theme.hover_style()
                    } else {
                        Style::default()
                            .fg(app.app_theme.smart)
                            .add_modifier(Modifier::BOLD)
                    },
                ));
                if app.list.grid_folder.starts_with('@') {
                    let label = if app.list.grid_folder == "@today" {
                        "Today"
                    } else if app.list.grid_folder == "@week" {
                        "This Week"
                    } else if app.list.grid_folder == "@untagged" {
                        "Untagged"
                    } else if let Some(tag) = app.list.grid_folder.strip_prefix("@tag:") {
                        tag
                    } else if let Some(custom) = app.list.grid_folder.strip_prefix("@custom:") {
                        custom
                    } else if app.list.grid_folder == "@tagged" {
                        "Tagged"
                    } else {
                        &app.list.grid_folder
                    };
                    spans.push(Span::styled(
                        " / ",
                        Style::default().fg(app.app_theme.muted),
                    ));
                    spans.push(Span::styled(
                        label.to_string(),
                        Style::default().fg(app.app_theme.fg),
                    ));
                }
            } else if is_subnotes {
                let sub_icon =
                    crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", app.config.ui.icon_mode);
                let sub_text = format!(" {sub_icon} Subnotes");
                let sub_w = sub_text.chars().count() as u16;
                let is_hovered = app.mouse_pos.is_some_and(|(col, row)| {
                    row == list_area.y + 1
                        && col >= list_area.x
                        && col < list_area.x + sub_w
                        && app.list.grid_folder != VIRTUAL_SUBNOTES_PATH
                });
                spans.push(Span::styled(
                    sub_text,
                    if is_hovered {
                        app.app_theme.hover_style()
                    } else {
                        Style::default()
                            .fg(app.app_theme.subnote)
                            .add_modifier(Modifier::BOLD)
                    },
                ));
                if crate::app::App::is_subnotes_parent_grid_path(&app.list.grid_folder) {
                    let parent_id =
                        crate::app::App::subnotes_parent_id_from_grid_path(&app.list.grid_folder);
                    let label = app
                        .notes
                        .iter()
                        .find(|n| n.id == parent_id)
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| parent_id.to_string());
                    spans.push(Span::styled(
                        " / ",
                        Style::default().fg(app.app_theme.muted),
                    ));
                    spans.push(Span::styled(label, Style::default().fg(app.app_theme.fg)));
                }
            } else {
                let vault_icon =
                    crate::ui::get_icon("\u{f07b}", "\u{1f4c1}", app.config.ui.icon_mode);
                let vault_text = format!(" {vault_icon} Vault");
                let vault_w = vault_text.chars().count() as u16;
                let is_hovered = app.mouse_pos.is_some_and(|(col, row)| {
                    row == list_area.y + 1
                        && col >= list_area.x
                        && col < list_area.x + vault_w
                        && !app.list.grid_folder.is_empty()
                });
                spans.push(Span::styled(
                    vault_text,
                    if is_hovered {
                        app.app_theme.hover_style()
                    } else {
                        Style::default()
                            .fg(app.app_theme.folder)
                            .add_modifier(Modifier::BOLD)
                    },
                ));
                if !app.list.grid_folder.is_empty() {
                    let parts: Vec<&str> = app.list.grid_folder.split('/').collect();
                    let mut current_path = String::new();
                    let mut offset = list_area.x + vault_w;
                    for (part_idx, part) in parts.iter().enumerate() {
                        spans.push(Span::styled(
                            " / ",
                            Style::default().fg(app.app_theme.muted),
                        ));
                        offset += 3;
                        let part_w = part.chars().count() as u16;
                        if !current_path.is_empty() {
                            current_path.push('/');
                        }
                        current_path.push_str(part);

                        let is_part_hovered = app.mouse_pos.is_some_and(|(col, row)| {
                            row == list_area.y + 1
                                && col >= offset
                                && col < offset + part_w
                                && part_idx < parts.len() - 1
                        });

                        spans.push(Span::styled(
                            part.to_string(),
                            if is_part_hovered {
                                app.app_theme.hover_style()
                            } else {
                                Style::default().fg(app.app_theme.fg)
                            },
                        ));
                        offset += part_w;
                    }
                }
            }
            let dir_rect = Rect::new(list_area.x, list_area.y + 1, list_area.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), dir_rect);

            // --- columns / visible rows ---
            let cols = ((list_area.width.saturating_sub(GRID_LEFT_MARGIN + GRID_GAP))
                / (GRID_TILE_W + GRID_GAP))
                .max(1) as usize;
            let rows = ((list_area.height.saturating_sub(GRID_TOP_MARGIN + GRID_GAP))
                / (GRID_TILE_H + GRID_GAP)) as usize;
            app.list.grid_columns = cols; // events.rs grid nav reads this (Up/Down move by cols)

            let len = app.list.visual_list.len();
            let total_rows = len.div_ceil(cols);
            let max_scroll = total_rows.saturating_sub(rows);

            // grid_scroll is viewport-row offset. Keep selected tile visible without blank
            // overscroll below a final partial row.
            if len > 0 && rows > 0 {
                let sel_row = app.list.visual_index.min(len.saturating_sub(1)) / cols;
                let mut scroll = app.list.grid_scroll.min(max_scroll);
                if sel_row < scroll {
                    scroll = sel_row;
                } else if sel_row >= scroll.saturating_add(rows) {
                    scroll = sel_row.saturating_add(1).saturating_sub(rows);
                }
                app.list.grid_scroll = scroll.min(max_scroll);
            } else {
                app.list.grid_scroll = 0;
                app.list.scroll_drag = None;
            }

            let start = app.list.grid_scroll.saturating_mul(cols);
            let count = (rows * cols).min(len.saturating_sub(start));
            let buf = frame.buffer_mut();

            for i in 0..count {
                let vi = start + i;
                if vi >= len {
                    break;
                }
                let row = i / cols;
                let col = i % cols;
                let tile_rect = ratatui::layout::Rect::new(
                    list_area.x + GRID_LEFT_MARGIN + (col as u16) * (GRID_TILE_W + GRID_GAP),
                    list_area.y + GRID_TOP_MARGIN + (row as u16) * (GRID_TILE_H + GRID_GAP),
                    GRID_TILE_W,
                    GRID_TILE_H,
                );
                let is_selected = vi == app.list.visual_index;
                let in_selection = app.list.selected_indices.contains(&vi);
                let is_hovered = app
                    .mouse_pos
                    .is_some_and(|(col, row)| crate::events::contains_cell(tile_rect, col, row));

                // --- resolve (icon char, glyph color, display name): SAME mapping the old code used ---
                let item = &app.list.visual_list[vi];
                let (icon_char, text_label, glyph_color, raw_name) = match item {
                    crate::app::VisualItem::Folder {
                        path,
                        name,
                        is_pinned,
                        ..
                    } => {
                        let is_pinned = *is_pinned;
                        let is_parent = name == "..";
                        let is_subnotes = !is_parent
                            && (path.as_str() == crate::app::VIRTUAL_SUBNOTES_PATH
                                || crate::app::App::is_subnotes_parent_grid_path(path));
                        let (ic, label) = if is_subnotes {
                            (
                                crate::ui::get_char(
                                    '\u{f15b}',
                                    '\u{1f4c3}',
                                    app.config.ui.icon_mode,
                                ),
                                "SN",
                            )
                        } else if is_parent {
                            (
                                crate::ui::get_char(
                                    '\u{f062}',
                                    '\u{2b06}',
                                    app.config.ui.icon_mode,
                                ),
                                "^",
                            )
                        } else {
                            // Always folder icon — if pinned, pin glyph goes top-right
                            (
                                crate::ui::get_char(
                                    '\u{f07b}',
                                    '\u{1f4c1}',
                                    app.config.ui.icon_mode,
                                ),
                                "F",
                            )
                        };
                        let col = if is_pinned {
                            app.app_theme.pinned
                        } else if is_subnotes {
                            app.app_theme.subnote
                        } else {
                            app.app_theme.folder
                        };
                        (ic, label, col, name.clone())
                    }
                    crate::app::VisualItem::SmartFolder { kind, label, .. } => {
                        let (ic, text_label) = match kind {
                            crate::list_view::SmartFolderKind::Today => (
                                crate::ui::get_char(
                                    '\u{f133}',
                                    '\u{1f4c5}',
                                    app.config.ui.icon_mode,
                                ),
                                "Today",
                            ),
                            crate::list_view::SmartFolderKind::ThisWeek => (
                                crate::ui::get_char(
                                    '\u{f073}',
                                    '\u{1f5d3}',
                                    app.config.ui.icon_mode,
                                ),
                                "Week",
                            ),
                            crate::list_view::SmartFolderKind::Untagged => (
                                crate::ui::get_char(
                                    '\u{f187}',
                                    '\u{1f4e5}',
                                    app.config.ui.icon_mode,
                                ),
                                "Untag",
                            ),
                            crate::list_view::SmartFolderKind::Tag(_) => (
                                crate::ui::get_char(
                                    '\u{f02c}',
                                    '\u{1f3f7}',
                                    app.config.ui.icon_mode,
                                ),
                                "Tag",
                            ),
                            crate::list_view::SmartFolderKind::Custom(_) => (
                                crate::ui::get_char(
                                    '\u{f0e7}',
                                    '\u{26a1}',
                                    app.config.ui.icon_mode,
                                ),
                                "Custom",
                            ),
                            crate::list_view::SmartFolderKind::Tagged => (
                                crate::ui::get_char(
                                    '\u{f0e7}',
                                    '\u{26a1}',
                                    app.config.ui.icon_mode,
                                ),
                                "Tagged",
                            ),
                        };
                        (ic, text_label, app.app_theme.smart, label.clone())
                    }
                    crate::app::VisualItem::Note {
                        summary_idx,
                        is_clin,
                        is_draw,
                        is_canvas,
                        ..
                    } => {
                        let s = &app.notes[*summary_idx];
                        let is_image = std::path::Path::new(&s.id)
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(crate::storage::is_image_ext);
                        let is_unknown = {
                            let ext = std::path::Path::new(&s.id)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("");
                            !*is_clin
                                && !*is_draw
                                && !*is_canvas
                                && !is_image
                                && ext != "md"
                                && ext != "txt"
                        };
                        let col = if s.pinned {
                            app.app_theme.pinned
                        } else if *is_clin {
                            app.app_theme.destructive
                        } else if *is_draw {
                            app.app_theme.success
                        } else if *is_canvas {
                            app.app_theme.accent
                        } else if is_image {
                            app.app_theme.warning
                        } else {
                            app.app_theme.text
                        };
                        let ic = if s.pinned {
                            crate::ui::get_char('\u{f4cc}', '\u{1f4cc}', app.config.ui.icon_mode)
                        } else if *is_clin {
                            crate::ui::get_char('\u{f023}', '\u{1f512}', app.config.ui.icon_mode)
                        } else if *is_draw {
                            crate::ui::get_char('\u{f1fc}', '\u{270f}', app.config.ui.icon_mode)
                        } else if *is_canvas {
                            crate::ui::get_char('\u{f005}', '\u{2b50}', app.config.ui.icon_mode)
                        } else if is_image {
                            crate::ui::get_char('\u{f1c5}', '\u{1f5bc}', app.config.ui.icon_mode)
                        } else if is_unknown {
                            '?'
                        } else {
                            crate::ui::get_char('\u{f15c}', '\u{1f4c4}', app.config.ui.icon_mode)
                        };
                        let label = if *is_clin {
                            "CX"
                        } else if *is_draw {
                            "D"
                        } else if *is_canvas {
                            "C"
                        } else if is_unknown {
                            "?"
                        } else {
                            "MD"
                        };
                        (ic, label, col, s.title.clone())
                    }
                    crate::app::VisualItem::CreateNew { .. } => (
                        crate::ui::get_char('\u{f067}', '\u{2795}', app.config.ui.icon_mode),
                        "+",
                        app.app_theme.success,
                        "Create...".to_string(),
                    ),
                    crate::app::VisualItem::Subnote {
                        parent_id,
                        subnote_idx,
                        ..
                    } => {
                        let ic =
                            crate::ui::get_char('\u{f02c}', '\u{1f3f7}', app.config.ui.icon_mode);
                        let title = app
                            .subnotes_view_cache
                            .iter()
                            .find_map(|(p, subs)| {
                                if p == parent_id {
                                    subs.get(*subnote_idx).map(|s| s.title.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| format!("subnote {}", subnote_idx + 1));
                        (ic, "SN", app.app_theme.subnote, title)
                    }
                };

                // --- tile border (plain border = "button") ---
                let mut block = Block::default().borders(Borders::ALL);
                // Selected tiles get accent bg. Cursor-on-selected gets a brighter border
                // so the cursor position remains visible on already-selected tiles.
                if in_selection {
                    block = block.style(Style::default().bg(app.app_theme.accent));
                } else if is_hovered && !is_selected {
                    block = block.style(app.app_theme.hover_style());
                }
                let border_fg = if is_selected && in_selection {
                    app.app_theme.highlight_fg
                } else if is_selected {
                    app.app_theme.highlight_bg
                } else if in_selection {
                    app.app_theme.accent
                } else {
                    app.app_theme.border
                };
                block = block.border_style(Style::default().fg(border_fg));
                let inner = block.inner(tile_rect);
                block.render(tile_rect, buf); // paints border
                let icon_fg = if in_selection {
                    app.app_theme.highlight_fg
                } else {
                    glyph_color
                };
                let base_style = if in_selection {
                    Style::default().bg(app.app_theme.accent)
                } else if is_hovered && !is_selected {
                    app.app_theme.hover_style()
                } else {
                    Style::default()
                };
                let icon_style =
                    base_style
                        .fg(icon_fg)
                        .add_modifier(if is_selected || in_selection {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        });
                buf.set_style(Rect::new(inner.x, inner.y, inner.width, 1), icon_style);
                let inner_w = inner.width as usize;
                if app.config.ui.icon_mode == crate::config::IconMode::None {
                    let label_chars: Vec<char> = text_label.chars().collect();
                    let label_start =
                        inner.x + ((inner_w.saturating_sub(label_chars.len())) / 2) as u16;
                    for (k, ch) in label_chars.iter().enumerate() {
                        if let Some(cell) = buf.cell_mut((label_start + k as u16, inner.y)) {
                            cell.set_char(*ch).set_style(icon_style);
                        }
                    }
                } else {
                    use unicode_width::UnicodeWidthChar;
                    let w = UnicodeWidthChar::width(icon_char).unwrap_or(1) as u16;
                    let icon_x = inner.x + (inner_w.saturating_sub(w as usize) / 2) as u16;
                    buf.set_string(icon_x, inner.y, icon_char.to_string(), icon_style);
                }

                // --- tag icon: top right corner for items that have tags ---
                let has_tags = match item {
                    crate::app::VisualItem::Note { summary_idx, .. } => {
                        !app.notes[*summary_idx].tags.is_empty()
                    }
                    _ => false,
                };
                if has_tags {
                    let tag_x = inner.x + inner.width.saturating_sub(1);
                    let tag_fg = if in_selection {
                        app.app_theme.highlight_fg
                    } else {
                        app.app_theme.tag
                    };
                    let tag_style =
                        base_style
                            .fg(tag_fg)
                            .add_modifier(if is_selected || in_selection {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            });
                    use unicode_width::UnicodeWidthChar;
                    let tg = crate::ui::get_char('\u{f02b}', '\u{1f3f7}', app.config.ui.icon_mode);
                    let tw = UnicodeWidthChar::width(tg).unwrap_or(1) as u16;
                    let tg_x = tag_x.saturating_sub(tw.saturating_sub(1));
                    buf.set_string(tg_x, inner.y, tg.to_string(), tag_style);
                }

                // --- pin icon: top right corner for pinned folders ---
                let is_pinned_folder = matches!(
                    item,
                    crate::app::VisualItem::Folder {
                        is_pinned: true,
                        ..
                    }
                );
                if is_pinned_folder {
                    let pin_x = inner.x + inner.width.saturating_sub(1);
                    let pin_fg = if in_selection {
                        app.app_theme.highlight_fg
                    } else {
                        app.app_theme.pinned
                    };
                    let pin_style =
                        base_style
                            .fg(pin_fg)
                            .add_modifier(if is_selected || in_selection {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            });
                    let pg = crate::ui::get_char('\u{f4cc}', '\u{1f4cc}', app.config.ui.icon_mode);
                    let pw = unicode_width::UnicodeWidthChar::width(pg).unwrap_or(1) as u16;
                    let pg_x = pin_x.saturating_sub(pw.saturating_sub(1));
                    buf.set_string(pg_x, inner.y, pg.to_string(), pin_style);
                }

                // --- name: sanitize, truncate to inner width, center, write on the bottom row (row 2) ---
                let sanitized = crate::sanitize::sanitize_for_terminal(&raw_name);
                let mut chars: Vec<char> = sanitized.chars().collect();
                if chars.len() > inner_w {
                    chars.truncate(inner_w - 1);
                    chars.push('…');
                }
                let pad = inner_w.saturating_sub(chars.len());
                let left = pad / 2;
                let name_style = if is_selected || in_selection {
                    if in_selection {
                        Style::default()
                            .fg(app.app_theme.highlight_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::BOLD)
                    }
                } else if is_hovered {
                    app.app_theme.hover_style()
                } else {
                    Style::default()
                };
                let mut name_string: String = " ".repeat(left);
                name_string.extend(chars.iter());
                let name_row = inner.y + 2; // bottom row of the 3 inner rows
                for (k, ch) in name_string.chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((inner.x + k as u16, name_row)) {
                        cell.set_char(ch).set_style(name_style);
                    }
                }

                // --- record tile for mouse hit-testing ---
                app.list.grid_tiles.push(crate::list_view::GridTile {
                    visual_index: vi,
                    rect: tile_rect,
                });
            }
            // do NOT render a List widget here; do NOT touch list_state (tree view still uses it).
            if len > 0 && rows > 0 {
                let meta = crate::ui::scrollbar::ScrollbarMeta {
                    track: crate::ui::scrollbar::track_rect(list_area),
                    content_len: total_rows,
                    viewport_len: rows,
                };
                app.list.last_scroll = Some(meta);
                if !crate::ui::scrollbar::overflows(total_rows, rows) {
                    app.list.scroll_drag = None;
                } else if app.config.ui.scrollbars {
                    crate::ui::scrollbar::draw_scrollbar(
                        frame,
                        list_area,
                        meta.content_len,
                        meta.viewport_len,
                        app.list.grid_scroll,
                        max_scroll,
                        &app.app_theme,
                    );
                }
            }
        } else {
            let total_len = app.list.visual_list.len();
            let viewport_len = list_area.height.saturating_sub(2) as usize;

            let max_off = total_len.saturating_sub(viewport_len);

            let offset = if app.config.ui.scrollbar_pan_mode
                && let Some(off) = app.list.list_viewport_offset
            {
                off.min(max_off)
            } else {
                let mut o = app.list.list_state.offset();
                if o > max_off {
                    o = max_off;
                }
                if app.list.visual_index < o {
                    o = app.list.visual_index;
                } else if app.list.visual_index >= o + viewport_len {
                    o = app
                        .list
                        .visual_index
                        .saturating_add(1)
                        .saturating_sub(viewport_len);
                }
                o
            };
            *app.list.list_state.offset_mut() = offset;

            let end = (offset + viewport_len).min(total_len);

            let inner_x = list_area.x + 2;
            let inner_y = list_area.y + 1;
            let inner_w = list_area.width.saturating_sub(4);
            let inner_h = list_area.height.saturating_sub(2);
            let hovered_visual_index = app.mouse_pos.and_then(|(col, row)| {
                if col >= inner_x
                    && col < inner_x + inner_w
                    && row >= inner_y
                    && row < inner_y + inner_h
                {
                    crate::ui::list_index_at(row, inner_y, 1, offset, total_len)
                } else {
                    None
                }
            });

            let mut items = Vec::with_capacity(end.saturating_sub(offset));
            for idx in offset..end {
                let item = app.format_visual_item(idx);
                let in_selection = app.list.selected_indices.contains(&idx);
                let is_cursor = idx == app.list.visual_index;
                if is_cursor && in_selection {
                    items.push(
                        item.style(
                            Style::default()
                                .bg(app.app_theme.accent)
                                .fg(app.app_theme.highlight_fg)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        ),
                    );
                } else if in_selection {
                    items.push(
                        item.style(
                            Style::default()
                                .bg(app.app_theme.accent)
                                .fg(app.app_theme.highlight_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                } else if is_cursor {
                    items.push(
                        item.style(
                            Style::default()
                                .fg(app.app_theme.highlight_fg)
                                .bg(app.app_theme.highlight_bg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                } else if Some(idx) == hovered_visual_index {
                    items.push(item.style(app.app_theme.hover_style()));
                } else {
                    items.push(item);
                }
            }
            let mut rel_state = ListState::default();
            if (offset..end).contains(&app.list.visual_index) {
                rel_state.select(Some(app.list.visual_index - offset));
            } else {
                rel_state.select(None);
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .style(app.app_theme.bg_style())
                        .borders(Borders::NONE)
                        .padding(Padding::new(2, 2, 1, 1)),
                )
                .highlight_style(
                    Style::default()
                        .fg(app.app_theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                );

            frame.render_stateful_widget(list, list_area, &mut rel_state);
            let content_len = total_len;
            let meta = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::track_rect(list_area),
                content_len,
                viewport_len,
            };
            app.list.last_scroll = Some(meta);
            if app.config.ui.scrollbars {
                let (pos, max_pos) = if app.config.ui.scrollbar_pan_mode {
                    (offset, max_off)
                } else {
                    (app.list.visual_index, total_len.saturating_sub(1))
                };
                crate::ui::scrollbar::draw_scrollbar(
                    frame,
                    list_area,
                    content_len,
                    viewport_len,
                    pos,
                    max_pos,
                    &app.app_theme,
                );
            }
        }
    }
    if let Some(preview_rect) = preview_area {
        let hide_encrypted = app.preview_encryption
            && app
                .list
                .visual_list
                .get(app.list.visual_index)
                .is_some_and(|item| {
                    matches!(item, crate::app::VisualItem::Note { is_clin: true, .. })
                });

        let content_is_current = app.list.preview_content_index == Some(app.list.visual_index);
        let is_current_or_pending = content_is_current || app.list.pending_preview_update;

        // SubnoteGraph: render statically from cache (no physics, no GraphState).
        // We bypass draw_preview_pane because it doesn't have access to App.
        let mut rendered_graph = false;
        if (content_is_current || app.list.pending_preview_update)
            && let Some(crate::list_view::PreviewContent::SubnoteGraph { parent_id }) =
                app.list.preview_content.as_ref()
        {
            // Look up parent title + subnote titles from the cache (no physics, no GraphState).
            let parent_title = app
                .notes
                .iter()
                .find(|n| n.id == *parent_id)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| parent_id.clone());
            let subnotes: Vec<crate::storage::SubNote> = app
                .subnotes_view_cache
                .iter()
                .find(|(p, _)| p == parent_id)
                .map(|(_, subs)| subs.clone())
                .unwrap_or_default();
            render_subnote_graph_static(
                frame,
                preview_rect,
                &parent_title,
                &subnotes,
                &app.app_theme,
            );
            rendered_graph = true;
        }
        if !rendered_graph
            && (content_is_current || app.list.pending_preview_update)
            && let Some(crate::list_view::PreviewContent::FolderGraph {
                root_path: _,
                focused_path,
            }) = app.list.preview_content.as_ref()
        {
            // Extract all immutable data before the mutable render call.
            let (children, label) = app.folder_graph_children(focused_path);
            let positions = orbit_positions(children.len(), 10.0);
            let positioned: Vec<crate::list_view::FolderGraphNode> = children
                .iter()
                .zip(positions.iter())
                .map(|(n, &(x, y))| crate::list_view::FolderGraphNode { x, y, ..n.clone() })
                .collect();
            render_folder_graph_static(frame, preview_rect, &label, &positioned, &app.app_theme);
            rendered_graph = true;
        }
        if !rendered_graph {
            let content = if is_current_or_pending {
                app.list.preview_content.as_ref()
            } else {
                None
            };
            crate::preview::draw_preview_pane(
                frame,
                preview_rect,
                &app.app_theme,
                content,
                hide_encrypted,
                app.list.snapshot_scroll_offset,
                app.config.ui.icon_mode,
            );
        }

        // Overlay decoded images on the preview text
        if let Some(crate::list_view::PreviewContent::Markdown(renderer)) =
            &app.list.preview_content
            && let Some(doc) = renderer.document()
            && let (Some(picker), Some(decode_tx)) = (&app.image_picker, &app.image_decode_tx)
        {
            let page = renderer.current_page_range();
            let scroll = app.list.snapshot_scroll_offset as usize;
            let start = page.start.saturating_add(scroll).min(page.end);
            let block = Block::default()
                .style(app.app_theme.preview_bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 2, 1, 1));
            let inner = block.inner(preview_rect);
            let end = (start + inner.height as usize).min(page.end);
            let range = start..end;
            let col_width = inner.width;

            for (local_line_idx, url) in doc.image_slots(range) {
                let resolved = app.storage.resolve_attachment(url);
                let path = resolved.unwrap_or_else(|| app.storage.notes_dir.join(url));
                if !path.exists() {
                    continue;
                }
                let key = crate::image_render::ImageKey { path };
                if app.list.image_cache.get_proto(&key).is_none() {
                    app.list
                        .image_cache
                        .request(key.clone(), 512, decode_tx, picker);
                }
                if let Some(proto) = app.list.image_cache.get_proto(&key) {
                    let row = inner.y + local_line_idx as u16;
                    let max_h = app.config.image.preview_rows as u16;
                    let img_rect = Rect::new(
                        inner.x,
                        row,
                        col_width.min(inner.width),
                        max_h.min(inner.bottom().saturating_sub(row)),
                    );
                    if img_rect.width > 1 && img_rect.height > 1 {
                        frame.render_widget(Clear, img_rect);
                        frame.render_widget(
                            Block::default().style(app.app_theme.preview_bg_style()),
                            img_rect,
                        );
                        frame.render_stateful_widget(
                            ratatui_image::StatefulImage::default()
                                .resize(ratatui_image::Resize::Fit(None)),
                            img_rect,
                            proto,
                        );
                    }
                }
            }
        }

        // Overlay standalone image file on the preview pane
        if (content_is_current || app.list.pending_preview_update)
            && let Some(crate::list_view::PreviewContent::Image(path)) = &app.list.preview_content
            && let (Some(picker), Some(decode_tx)) = (&app.image_picker, &app.image_decode_tx)
        {
            let inner_pad = 2_u16;
            let col_width = preview_rect.width.saturating_sub(2 * inner_pad);
            let key = crate::image_render::ImageKey { path: path.clone() };
            if app.list.image_cache.get_proto(&key).is_none() {
                app.list
                    .image_cache
                    .request(key.clone(), 512, decode_tx, picker);
            }
            if let Some(proto) = app.list.image_cache.get_proto(&key) {
                // available area (full preview minus padding) — the bounding box
                let max_w = col_width.min(preview_rect.width.saturating_sub(2));
                let max_h = preview_rect.height.saturating_sub(2);
                let bound_rect =
                    Rect::new(preview_rect.x + inner_pad, preview_rect.y + 1, max_w, max_h);
                // clear the full bounding box (removes any "Image loading..." text)
                if bound_rect.width > 1 && bound_rect.height > 1 {
                    frame.render_widget(Clear, bound_rect);
                    frame.render_widget(
                        Block::default().style(app.app_theme.preview_bg_style()),
                        bound_rect,
                    );
                }
                // get actual rendered image size after Fit scaling
                let rendered = proto.size_for(
                    ratatui_image::Resize::Fit(None),
                    ratatui::layout::Size::new(max_w, max_h),
                );
                // center the image within the available area
                let offset_x = (max_w.saturating_sub(rendered.width)) / 2;
                let offset_y = (max_h.saturating_sub(rendered.height)) / 2;
                let img_rect = Rect::new(
                    bound_rect.x + offset_x,
                    bound_rect.y + offset_y,
                    rendered.width.min(max_w),
                    rendered.height.min(max_h),
                );
                if img_rect.width > 1 && img_rect.height > 1 {
                    frame.render_stateful_widget(
                        ratatui_image::StatefulImage::default()
                            .resize(ratatui_image::Resize::Fit(None)),
                        img_rect,
                        proto,
                    );
                }
            }
        }
    }
    if let Some(cal_rect) = calendar_area {
        let bottom_border =
            app.config.list.calendar_position == crate::config::CalendarPosition::Top;
        let active = app.active_strip_sections_for(cal_rect.width);
        let rects = section_rects(cal_rect, &active);
        for (sec, r) in active.iter().zip(rects.iter().copied()) {
            match sec {
                crate::config::NotesSection::Calendar => crate::calendar::draw_calendar(
                    frame,
                    r,
                    &app.app_theme,
                    app.note_index
                        .as_ref()
                        .map(|i| &i.activity_by_day)
                        .unwrap_or(&std::collections::HashMap::new()),
                    bottom_border,
                    app.list.week_start,
                    cal_rect,
                ),
                crate::config::NotesSection::Goals => {
                    let _ = app.get_current_goals_progress();
                    crate::goals::draw_goals_progress(
                        frame,
                        r,
                        &app.app_theme,
                        &app.goals_progress,
                        &app.config.goals,
                        bottom_border,
                        cal_rect,
                    );
                }
                crate::config::NotesSection::Draw => {
                    app.ensure_draw_preview();
                    draw_strip_draw(frame, r, app, bottom_border, cal_rect);
                }
                crate::config::NotesSection::Graf => {
                    app.ensure_graph_preview();
                    draw_strip_graf(frame, r, app, bottom_border, cal_rect);
                }
                crate::config::NotesSection::Todo => {
                    crate::todo::update_todo_state(&app.storage, &mut app.todo_state);
                    crate::todo::draw_todo(
                        frame,
                        r,
                        &app.app_theme,
                        &app.todo_state,
                        bottom_border,
                        cal_rect,
                    );
                }
            }
        }
    }
    let kb = &app.keybinds;
    let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;
    let hints_items = if is_grid {
        vec![
            (
                format!(
                    "{}/{}/{}/{}",
                    kb.display_list(ListAction::MoveLeft),
                    kb.display_list(ListAction::MoveDown),
                    kb.display_list(ListAction::MoveUp),
                    kb.display_list(ListAction::MoveRight)
                ),
                "move",
            ),
            (kb.display_list(ListAction::Open), "open"),
            (kb.list_keys_display(ListAction::Quit), "quit"),
            (
                format!("F1/{}", kb.list_keys_display(ListAction::Help)),
                "help",
            ),
            ("F2".to_string(), "keybinds"),
        ]
    } else {
        vec![
            (
                format!(
                    "{}/{}",
                    kb.display_list(ListAction::MoveDown),
                    kb.display_list(ListAction::MoveUp)
                ),
                "move",
            ),
            (kb.display_list(ListAction::Open), "open"),
            (kb.display_list(ListAction::CollapseAll), "collapse"),
            (kb.display_list(ListAction::ExpandAll), "expand"),
            (kb.list_keys_display(ListAction::Quit), "quit"),
            (
                format!("F1/{}", kb.list_keys_display(ListAction::Help)),
                "help",
            ),
            ("F2".to_string(), "keybinds"),
        ]
    };
    let default_hints = format_keybind_hints(&app.app_theme, &hints_items);

    let hint = if in_select_mode {
        let select_items = vec![
            (kb.display_list(ListAction::ToggleSelectItem), "toggle"),
            (kb.display_list(ListAction::MoveNote), "move"),
            (kb.display_list(ListAction::ManageTags), "tag"),
            (
                kb.display_list(ListAction::RemoveTagsFromSelected),
                "remove tags",
            ),
            (kb.display_list(ListAction::Delete), "delete"),
            (kb.display_list(ListAction::ToggleSelectMode), "exit"),
        ];
        format_keybind_hints(&app.app_theme, &select_items)
    } else if app.layout_edit {
        let layout_items = vec![
            ("drag".to_string(), "borders/panes"),
            ("Tab".to_string(), "swap sections"),
            ("Space/click".to_string(), "cycle section"),
            ("a".to_string(), "add/remove section"),
            ("s".to_string(), "preview"),
            ("c".to_string(), "calendar"),
            ("←→ ↑↓".to_string(), "resize"),
            ("Esc".to_string(), "done"),
        ];
        format_keybind_hints(&app.app_theme, &layout_items)
    } else {
        default_hints
    };
    let badge_spans = if app.app_theme.hint_bar_style.has_filled_cells() {
        crate::ui::ext_badge_spans(
            app.editor.external_editor_enabled,
            &app.app_theme,
            Some(app.app_theme.accent),
        )
    } else {
        crate::ui::ext_badge_spans(app.editor.external_editor_enabled, &app.app_theme, None)
    };
    let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::List);
    ctx.area = Some(chunks[2]);
    ctx.hints = Some(hint.spans);
    ctx.badge = Some(badge_spans);
    if let Some(p) = &app.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.accent),
        )]);
    }

    let (left_line, right_line) = crate::statusline::render_footer(
        &ctx,
        &app.config.statusline,
        ViewMode::List,
        &app.app_theme,
    );
    draw_status_bar(frame, chunks[2], &app.app_theme, left_line, right_line);
    if app.list.preview_enabled && !app.preview_fullscreen {
        let ratio_num = (app.list.preview_width_ratio.clamp(0.2, 0.8) * 100.0).round() as u32;
        let constraints = match app.preview_position {
            crate::config::PreviewPosition::Left => [
                Constraint::Ratio(ratio_num, 100),
                Constraint::Length(1),
                Constraint::Min(0),
            ],
            crate::config::PreviewPosition::Right => [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Ratio(ratio_num, 100),
            ],
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        if app.layout_edit {
            let divider_area = full_cols[1];
            let accent = app.app_theme.heading;
            let buf = frame.buffer_mut();
            for row in divider_area.top()..divider_area.bottom() {
                if let Some(cell) = buf.cell_mut((divider_area.x, row)) {
                    cell.set_char('║');
                    cell.set_fg(accent);
                }
            }
            let mid_row = divider_area.top() + divider_area.height / 2;
            if let Some(cell) = buf.cell_mut((divider_area.x, mid_row)) {
                cell.set_char('⇄');
                cell.set_fg(accent);
            }
        } else {
            draw_dim_vline(frame, full_cols[1], app.app_theme.muted);
        }
    }
    if app.layout_edit && app.list.calendar_enabled {
        let hdiv_y = match app.config.list.calendar_position {
            crate::config::CalendarPosition::Bottom => list_area.y + list_area.height,
            crate::config::CalendarPosition::Top => list_area.y.saturating_sub(1),
        };
        let accent = app.app_theme.heading;
        let buf = frame.buffer_mut();
        for col in list_area.left()..list_area.right() {
            if let Some(cell) = buf.cell_mut((col, hdiv_y)) {
                cell.set_char('═');
                cell.set_fg(accent);
            }
        }
        let mid_col = list_area.left() + list_area.width / 2;
        if let Some(cell) = buf.cell_mut((mid_col, hdiv_y)) {
            cell.set_char('⇅');
            cell.set_fg(accent);
        }
    }

    app.mouse_pos = saved_mouse_pos;
}

pub(crate) fn list_view_layout(
    area: Rect,
    preview_enabled: bool,
    preview_position: crate::config::PreviewPosition,
    calendar_enabled: bool,
    preview_fullscreen: bool,
    preview_width_ratio: f32,
    calendar_height: u16,
    calendar_position: crate::config::CalendarPosition,
) -> (Rect, Option<Rect>, Option<Rect>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    if preview_fullscreen {
        return (chunks[1], Some(chunks[1]), None);
    }

    let (list_column, preview_area) = if preview_enabled {
        let ratio_num = (preview_width_ratio.clamp(0.2, 0.8) * 100.0).round() as u32;
        let (constraints, list_idx, p_idx) = match preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(ratio_num, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(ratio_num, 100),
                ],
                0,
                2,
            ),
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        (full_cols[list_idx], Some(full_cols[p_idx]))
    } else {
        (
            Rect::new(area.x, area.y + 1, area.width, chunks[1].height),
            None,
        )
    };

    let (list_area, calendar_area) = if calendar_enabled {
        let cal_h = calendar_height.max(9);
        let sp = match calendar_position {
            crate::config::CalendarPosition::Bottom => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(5), Constraint::Length(cal_h)])
                .split(list_column),
            crate::config::CalendarPosition::Top => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(cal_h), Constraint::Min(5)])
                .split(list_column),
        };
        match calendar_position {
            crate::config::CalendarPosition::Bottom => (sp[0], Some(sp[1])),
            crate::config::CalendarPosition::Top => (sp[1], Some(sp[0])),
        }
    } else {
        (list_column, None)
    };

    (list_area, preview_area, calendar_area)
}

fn get_item_name(app: &App, idx: usize) -> Option<String> {
    if let Some(item) = app.list.visual_list.get(idx) {
        match item {
            crate::list_view::VisualItem::Folder { name, .. } => Some(name.clone()),
            crate::list_view::VisualItem::SmartFolder { label, .. } => Some(label.clone()),
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                app.notes.get(*summary_idx).map(|n| n.title.clone())
            }
            crate::list_view::VisualItem::CreateNew { .. } => Some("Create...".to_string()),
            crate::list_view::VisualItem::Subnote {
                parent_id,
                subnote_idx,
                ..
            } => app.subnotes_view_cache.iter().find_map(|(p, subs)| {
                if p == parent_id {
                    subs.get(*subnote_idx).map(|s| s.title.clone())
                } else {
                    None
                }
            }),
        }
    } else {
        None
    }
}

pub fn get_preview_info(app: &App) -> Option<PreviewHeaderInfo> {
    if !app.preview_fullscreen {
        return None;
    }

    let current_index = if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note_pos) = app.notes.iter().position(|n| &n.id == id) {
                app.list.visual_list.iter().position(|item| {
                    if let crate::list_view::VisualItem::Note { summary_idx, .. } = item {
                        *summary_idx == note_pos
                    } else {
                        false
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        Some(app.list.visual_index)
    };

    if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note) = app.notes.iter().find(|n| &n.id == id) {
                let folder = if note.folder.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", note.folder)
                };
                let title = crate::events::get_title_text(&app.editor.title_editor).into_owned();
                let title = if title.is_empty() {
                    "Untitled note".to_string()
                } else {
                    title
                };
                let (prev_name, next_name) = if let Some(idx) = current_index {
                    let prev = if idx > 0 {
                        get_item_name(app, idx - 1)
                    } else {
                        None
                    };
                    let next = if idx + 1 < app.list.visual_list.len() {
                        get_item_name(app, idx + 1)
                    } else {
                        None
                    };
                    (prev, next)
                } else {
                    (None, None)
                };
                return Some(PreviewHeaderInfo {
                    path: folder,
                    item_name: title,
                    prev_name,
                    next_name,
                });
            }
        } else if let Some(path) = &app.editor.template_edit_path {
            let parent = path
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let folder = if parent.is_empty() {
                "Templates".to_string()
            } else {
                format!("Templates/{}", parent)
            };
            return Some(PreviewHeaderInfo {
                path: folder,
                item_name: filename,
                prev_name: None,
                next_name: None,
            });
        }
    }

    if let Some(item) = app.list.visual_list.get(app.list.visual_index) {
        let (path, item_name) = match item {
            crate::list_view::VisualItem::Folder { path, name, .. } => {
                if path.is_empty() {
                    ("Vault".to_string(), "Vault".to_string())
                } else if path == VIRTUAL_PINNED_PATH {
                    ("Pinned".to_string(), "Pinned".to_string())
                } else if path == VIRTUAL_SMART_PATH {
                    ("Smart".to_string(), "Smart".to_string())
                } else if let Some(slash_idx) = path.rfind('/') {
                    let parent = &path[..slash_idx];
                    (format!("Vault/{}", parent), name.clone())
                } else {
                    ("Vault".to_string(), name.clone())
                }
            }
            crate::list_view::VisualItem::SmartFolder { label, .. } => {
                ("Smart".to_string(), label.clone())
            }

            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                let note = app.notes.get(*summary_idx)?;
                let folder = if note.folder.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", note.folder)
                };
                (folder, note.title.clone())
            }
            crate::list_view::VisualItem::Subnote {
                parent_id,
                subnote_idx,
                ..
            } => {
                let title = app
                    .subnotes_view_cache
                    .iter()
                    .find_map(|(p, subs)| {
                        if p == parent_id {
                            subs.get(*subnote_idx).map(|s| s.title.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| format!("subnote {}", subnote_idx + 1));
                ("Subnotes".to_string(), title)
            }
            crate::list_view::VisualItem::CreateNew { path, .. } => {
                let folder = if path.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", path)
                };
                (folder, "Create...".to_string())
            }
        };

        let idx = app.list.visual_index;
        let prev_name = if idx > 0 {
            get_item_name(app, idx - 1)
        } else {
            None
        };
        let next_name = if idx + 1 < app.list.visual_list.len() {
            get_item_name(app, idx + 1)
        } else {
            None
        };

        Some(PreviewHeaderInfo {
            path,
            item_name,
            prev_name,
            next_name,
        })
    } else {
        None
    }
}

pub(crate) fn list_detail_value(app: &App) -> Option<crate::statusline::ListHeaderDetail> {
    use crate::statusline::{ListHeaderDetail, ListHeaderField};

    let item = app.list.visual_list.get(app.list.visual_index)?;
    match item {
        crate::app::VisualItem::Note { summary_idx, .. } => {
            let note = app.notes.get(*summary_idx)?;
            let muted = Style::default().fg(app.app_theme.muted);
            let tag_icon_style = Style::default()
                .fg(app.app_theme.tag)
                .add_modifier(Modifier::BOLD);
            let mut groups = Vec::new();

            let clock = crate::ui::get_icon("\u{f017}", "\u{23f0}", app.config.ui.icon_mode);
            let mut age = Vec::new();
            if !clock.is_empty() {
                age.push(Span::styled(clock, muted));
                age.push(Span::styled(" ", muted));
            }
            age.push(Span::styled(
                crate::statusline::list_relative_age(note.updated_at),
                muted,
            ));
            groups.push((ListHeaderField::Age, age));

            if !note.tags.is_empty() {
                let tag_icon =
                    crate::ui::get_icon("\u{f02b}", "\u{1f3f7}", app.config.ui.icon_mode);
                let mut tags = Vec::new();
                if !tag_icon.is_empty() {
                    tags.push(Span::styled(tag_icon, tag_icon_style));
                    tags.push(Span::styled(" ", tag_icon_style));
                }
                tags.push(Span::styled(
                    crate::statusline::compact_list_tags(&note.tags),
                    Style::default().fg(app.app_theme.fg),
                ));
                groups.push((ListHeaderField::Tags, tags));
            }

            if app.list.show_file_size {
                groups.push((
                    ListHeaderField::Size,
                    vec![Span::styled(
                        crate::ui::format_size(note.size_bytes),
                        Style::default().fg(app.app_theme.muted),
                    )],
                ));
            }
            Some(ListHeaderDetail::new(groups))
        }
        crate::app::VisualItem::Folder {
            name,
            note_count,
            recursive_count,
            ..
        } if name != ".." => {
            let icon = crate::ui::get_icon("\u{f0ca}", "\u{1f4cb}", app.config.ui.icon_mode);
            let count = if *recursive_count > *note_count {
                format!("{note_count}+{}", recursive_count - note_count)
            } else {
                note_count.to_string()
            };
            let mut spans = Vec::new();
            if !icon.is_empty() {
                spans.push(Span::styled(
                    icon,
                    Style::default().fg(app.app_theme.folder),
                ));
                spans.push(Span::styled(" ", Style::default().fg(app.app_theme.folder)));
            }
            spans.push(Span::styled(count, Style::default().fg(app.app_theme.fg)));
            Some(ListHeaderDetail::new(vec![(ListHeaderField::Count, spans)]))
        }
        crate::app::VisualItem::SmartFolder { note_count, .. } => {
            let icon = crate::ui::get_icon("\u{f0ca}", "\u{1f4cb}", app.config.ui.icon_mode);
            let mut spans = Vec::new();
            if !icon.is_empty() {
                spans.push(Span::styled(icon, Style::default().fg(app.app_theme.tag)));
                spans.push(Span::styled(" ", Style::default().fg(app.app_theme.tag)));
            }
            spans.push(Span::styled(
                note_count.to_string(),
                Style::default().fg(app.app_theme.fg),
            ));
            Some(ListHeaderDetail::new(vec![(ListHeaderField::Count, spans)]))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ViewMode;
    use crate::config::CalendarPosition;
    use crate::config::PreviewPosition;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend};

    fn grid_test_app(items: usize) -> (tempfile::TempDir, App) {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = crate::storage::Storage {
            data_dir: temp_dir.path().join("data"),
            config_dir: temp_dir.path().join("config"),
            notes_dir: temp_dir.path().join("notes"),
            templates_dir: temp_dir.path().join("templates"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        for path in [
            &storage.data_dir,
            &storage.config_dir,
            &storage.notes_dir,
            &storage.templates_dir,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }

        let mut app = App::new(storage).unwrap();
        app.list.visual_list = (0..items)
            .map(|i| crate::list_view::VisualItem::CreateNew {
                path: format!("item-{i}"),
                depth: 0,
            })
            .collect();
        app.list.notes_layout = crate::config::NotesLayout::Grid;
        app.list.preview_enabled = false;
        app.list.calendar_enabled = false;
        app.config.ui.scrollbars = true;
        (temp_dir, app)
    }

    fn draw_grid(terminal: &mut Terminal<TestBackend>, app: &mut App) {
        terminal.draw(|frame| draw_list_view(frame, app)).unwrap();
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn grid_scrollbar_drag_selects_first_tile_of_bottom_view() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = grid_test_app(10);
        let mut terminal = Terminal::new(TestBackend::new(36, 18)).unwrap();
        draw_grid(&mut terminal, &mut app);

        let meta = app.list.last_scroll.expect("grid scrollbar metadata");
        assert_eq!((meta.content_len, meta.viewport_len), (4, 2));
        let bottom = meta.track.bottom().saturating_sub(1);
        crate::events::handle_list_mouse(
            &mut app,
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                meta.track.x,
                meta.track.y,
            ),
            Rect::new(0, 0, 36, 18),
        );
        crate::events::handle_list_mouse(
            &mut app,
            mouse(
                MouseEventKind::Drag(MouseButton::Left),
                meta.track.x,
                bottom,
            ),
            Rect::new(0, 0, 36, 18),
        );
        crate::events::handle_list_mouse(
            &mut app,
            mouse(MouseEventKind::Up(MouseButton::Left), meta.track.x, bottom),
            Rect::new(0, 0, 36, 18),
        );

        assert_eq!(app.list.grid_scroll, 2);
        assert_eq!(app.list.visual_index, 6);
        assert!(app.list.scroll_drag.is_none());
        assert_eq!(
            app.list.last_scroll.expect("metadata remains").content_len,
            4
        );
        assert_eq!(
            app.list.last_scroll.expect("metadata remains").viewport_len,
            2
        );

        draw_grid(&mut terminal, &mut app);
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((meta.track.x, bottom))
                .unwrap()
                .symbol(),
            "█"
        );
    }

    #[test]
    fn grid_scrollbar_track_click_reaches_bottom() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = grid_test_app(10);
        let mut terminal = Terminal::new(TestBackend::new(36, 18)).unwrap();
        draw_grid(&mut terminal, &mut app);

        let meta = app.list.last_scroll.expect("grid scrollbar metadata");
        crate::events::handle_list_mouse(
            &mut app,
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                meta.track.x,
                meta.track.bottom().saturating_sub(1),
            ),
            Rect::new(0, 0, 36, 18),
        );

        assert_eq!(app.list.grid_scroll, 2);
        assert_eq!(app.list.visual_index, 6);
    }

    #[test]
    fn grid_scrollbar_fit_and_empty_states_do_not_scroll() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = grid_test_app(6);
        let mut terminal = Terminal::new(TestBackend::new(36, 18)).unwrap();
        draw_grid(&mut terminal, &mut app);

        let meta = app.list.last_scroll.expect("fit grid metadata");
        assert_eq!((meta.content_len, meta.viewport_len), (2, 2));
        assert_eq!(app.list.grid_scroll, 0);
        assert!(app.list.scroll_drag.is_none());
        crate::events::handle_list_mouse(
            &mut app,
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                meta.track.x,
                meta.track.bottom().saturating_sub(1),
            ),
            Rect::new(0, 0, 36, 18),
        );
        assert_eq!(app.list.grid_scroll, 0);
        assert_eq!(app.list.visual_index, 0);

        app.list.visual_list.clear();
        draw_grid(&mut terminal, &mut app);
        assert!(app.list.last_scroll.is_none());
        assert_eq!(app.list.grid_scroll, 0);
        assert!(app.list.grid_tiles.is_empty());
    }

    #[test]
    fn calendar_never_overlaps_preview_and_stays_in_list_column() {
        let area = Rect::new(0, 0, 80, 24);
        for &position in &[PreviewPosition::Left, PreviewPosition::Right] {
            let (list_area, preview_area, calendar_area) = list_view_layout(
                area,
                true, // preview enabled
                position,
                true,  // calendar enabled
                false, // preview_fullscreen
                0.43,  // preview_width_ratio (default)
                9,     // calendar_height (default)
                CalendarPosition::Bottom,
            );

            let cal = calendar_area.expect("calendar enabled");
            let preview = preview_area.expect("preview enabled");

            // Calendar is strictly underneath list_area.
            assert_eq!(cal.x, list_area.x);
            assert_eq!(cal.width, list_area.width);
            assert_eq!(cal.y, list_area.y + list_area.height);

            // Calendar and preview are disjoint (separated in x since y overlaps).
            let disjoint = cal.right() <= preview.x || preview.right() <= cal.x;
            assert!(disjoint, "calendar must not overlap preview @ {position:?}");
        }
    }

    /// With preview off the calendar spans the full content width at the bottom.
    #[test]
    fn calendar_full_width_when_no_preview() {
        let area = Rect::new(0, 0, 80, 24);
        let (list_area, preview_area, calendar_area) = list_view_layout(
            area,
            false,
            PreviewPosition::Right,
            true,
            false,
            0.43,
            9,
            CalendarPosition::Bottom,
        );

        assert!(preview_area.is_none());
        let cal = calendar_area.expect("calendar enabled");
        assert_eq!(cal.width, list_area.width);
        assert_eq!(cal.y, list_area.y + list_area.height);
        assert_eq!(cal.height, 9);
    }

    /// With the calendar disabled there is no calendar area and list is full.
    #[test]
    fn no_calendar_area_when_disabled() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, preview_area, calendar_area) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            false,
            false,
            0.43,
            9,
            CalendarPosition::Bottom,
        );
        assert!(calendar_area.is_none());
        assert!(preview_area.is_some());
    }

    #[test]
    fn test_get_preview_info() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
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
        let mut app = App::new(storage).unwrap();

        // 1. preview_fullscreen is false
        app.preview_fullscreen = false;
        assert!(get_preview_info(&app).is_none());

        app.preview_fullscreen = true;

        // 2. ViewMode::List, empty list
        app.mode = ViewMode::List;
        app.list.visual_list.clear();
        assert!(get_preview_info(&app).is_none());

        // 3. ViewMode::List, selected folder
        app.list.visual_list = vec![crate::list_view::VisualItem::Folder {
            path: "work/projects".to_string(),
            name: "projects".to_string(),
            depth: 1,
            is_expanded: false,
            note_count: 0,
            recursive_count: 0,
            stale: false,
            is_pinned: false,
        }];
        app.list.visual_index = 0;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work".to_string(),
                item_name: "projects".to_string(),
                prev_name: None,
                next_name: None,
            })
        );

        // 4. ViewMode::List, selected note with prev and next
        let note = crate::storage::NoteSummary {
            id: "note1".to_string(),
            title: "My Note".to_string(),
            updated_at: 0,
            folder: "work/projects".to_string(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        };
        app.notes = vec![note];
        app.list.visual_list = vec![
            crate::list_view::VisualItem::Folder {
                path: "work/projects".to_string(),
                name: "projects".to_string(),
                depth: 1,
                is_expanded: true,
                note_count: 1,
                recursive_count: 1,
                stale: false,
                is_pinned: false,
            },
            crate::list_view::VisualItem::Note {
                summary_idx: 0,
                depth: 2,
                is_clin: false,
                is_draw: false,
                is_canvas: false,
                in_virtual_pinned_folder: false,
            },
            crate::list_view::VisualItem::Folder {
                path: "other".to_string(),
                name: "other".to_string(),
                depth: 1,
                is_expanded: false,
                note_count: 0,
                recursive_count: 0,
                stale: false,
                is_pinned: false,
            },
        ];
        app.list.visual_index = 1;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "My Note".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );

        // 5. ViewMode::Edit, editing a note
        app.mode = ViewMode::Edit;
        app.editor.editing_id = Some("note1".to_string());
        app.editor.title_editor = crate::events::make_title_editor(
            "Unsaved Title Change",
            app.app_theme.highlight_fg,
            app.app_theme.highlight_bg,
        );
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "Unsaved Title Change".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );
    }

    #[test]
    fn preview_width_ratio_controls_preview_width() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, preview_43, _) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            true,
            false,
            0.43,
            9,
            CalendarPosition::Bottom,
        );
        let (_, preview_70, _) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            true,
            false,
            0.70,
            9,
            CalendarPosition::Bottom,
        );
        let p43 = preview_43.expect("preview enabled");
        let p70 = preview_70.expect("preview enabled");
        assert!(
            p70.width > p43.width,
            "0.70 ratio should give wider preview than 0.43"
        );
    }

    #[test]
    fn calendar_height_controls_calendar_height() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, _, cal_9) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            true,
            false,
            0.43,
            9,
            CalendarPosition::Bottom,
        );
        let (_, _, cal_14) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            true,
            false,
            0.43,
            14,
            CalendarPosition::Bottom,
        );
        let c9 = cal_9.expect("calendar enabled");
        let c14 = cal_14.expect("calendar enabled");
        assert!(
            c14.height > c9.height,
            "height 14 should give taller calendar than 9"
        );
        assert_eq!(c14.height, 14);
    }

    #[test]
    fn calendar_height_clamped_to_at_least_9() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, _, cal_3) = list_view_layout(
            area,
            true,
            PreviewPosition::Right,
            true,
            false,
            0.43,
            3,
            CalendarPosition::Bottom,
        );
        let c3 = cal_3.expect("calendar enabled");
        assert_eq!(
            c3.height, 9,
            "calendar height should be clamped to at least 9 to be visible"
        );
    }

    #[test]
    fn section_rects_zero_active() {
        let r = Rect::new(0, 0, 100, 10);
        let rects = section_rects(r, &[]);
        assert!(rects.is_empty());
    }

    #[test]
    fn section_rects_one_active() {
        let r = Rect::new(0, 0, 100, 10);
        let active = [crate::config::NotesSection::Calendar];
        let rects = section_rects(r, &active);
        assert_eq!(rects.len(), 1);
        // Single section centered at 50% width: 100/2=50 wide, x=(100-50)/2=25
        assert_eq!(rects[0], Rect::new(25, 0, 50, 10));
    }

    #[test]
    fn section_rects_two_active_equal_halves() {
        let r = Rect::new(0, 0, 100, 10);
        let active = [
            crate::config::NotesSection::Calendar,
            crate::config::NotesSection::Goals,
        ];
        let rects = section_rects(r, &active);
        assert_eq!(rects.len(), 2);
        // widths differ by at most 1
        let diff = (rects[0].width as i16 - rects[1].width as i16).unsigned_abs();
        assert!(diff <= 1, "halves must be equal width within 1, got {diff}");
        assert_eq!(rects[0].x, r.x);
        assert_eq!(rects[0].y, r.y);
        assert_eq!(rects[0].height, r.height);
        assert_eq!(rects[1].y, r.y);
        assert_eq!(rects[1].height, r.height);
        assert_eq!(rects[1].right(), r.right());
    }

    #[test]
    fn orbit_positions_count_and_radius() {
        for n in [0, 1, 2, 3, 4, 8] {
            let r = 10.0;
            let pos = orbit_positions(n, r);
            assert_eq!(pos.len(), n, "count mismatch for n={n}");
            if n > 0 {
                // First point is at angle -π/2 → (0, -r).
                let (x, y) = pos[0];
                assert!((x - 0.0).abs() < 1e-12, "first x not 0: {x}");
                assert!((y - (-r)).abs() < 1e-12, "first y not -r: {y}");
            }
        }
    }
    #[test]
    fn list_header_compact_fields() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = grid_test_app(0);
        app.notes = vec![crate::storage::NoteSummary {
            id: "note.md".into(),
            title: "Note".into(),
            updated_at: crate::ui::now_unix_secs(),
            folder: String::new(),
            tags: vec![
                "123456789012".into(),
                "界界界界界界界".into(),
                "third".into(),
                "fourth".into(),
                "fifth".into(),
            ],
            pinned: false,
            links: vec![],
            size_bytes: 1_536,
        }];
        app.list.visual_list = vec![crate::list_view::VisualItem::Note {
            summary_idx: 0,
            depth: 0,
            is_clin: false,
            is_draw: false,
            is_canvas: false,
            in_virtual_pinned_folder: false,
        }];
        app.list.show_file_size = true;

        for (field, order, expected) in [
            (
                crate::list_view::SortField::Title,
                crate::list_view::SortOrder::Ascending,
                "A-z",
            ),
            (
                crate::list_view::SortField::Title,
                crate::list_view::SortOrder::Descending,
                "Z-a",
            ),
            (
                crate::list_view::SortField::Modified,
                crate::list_view::SortOrder::Ascending,
                "✎▲",
            ),
            (
                crate::list_view::SortField::Modified,
                crate::list_view::SortOrder::Descending,
                "✎▼",
            ),
        ] {
            app.list.sort_field = field;
            app.list.sort_order = order;
            app.config.ui.icon_mode = crate::config::IconMode::Unicode;
            let mut ctx = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
            ctx.note = app.notes.first();
            assert_eq!(ctx.resolve("sort").as_deref(), Some(expected));
        }
        app.config.ui.icon_mode = crate::config::IconMode::Nerd;
        let ctx = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        assert_eq!(ctx.resolve("sort").as_deref(), Some("\u{f03eb}▼"));
        app.list.sort_field = crate::list_view::SortField::Modified;
        app.config.ui.icon_mode = crate::config::IconMode::None;
        let ctx = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        assert_eq!(ctx.resolve("sort").as_deref(), Some("M▼"));

        assert_eq!(crate::statusline::compact_list_tags(&[]), "");
        assert_eq!(
            crate::statusline::compact_list_tags(&["123456789012".into()]),
            "123456789012"
        );
        assert_eq!(
            crate::statusline::compact_list_tags(&["界界界界界界界".into()]),
            "界界界界界…"
        );
        assert_eq!(
            crate::statusline::compact_list_tags(&app.notes[0].tags),
            "123456789012, 界界界界界… +3"
        );
        let note_detail = Line::from(
            list_detail_value(&app)
                .expect("note detail")
                .spans_without(&[]),
        )
        .to_string();
        assert!(note_detail.contains("123456789012, 界界界界界… +3"));
        assert!(note_detail.contains(&crate::ui::format_size(1_536)));

        app.list.visual_list = vec![crate::list_view::VisualItem::Folder {
            path: "folder".into(),
            name: "folder".into(),
            depth: 0,
            is_expanded: false,
            note_count: 2,
            recursive_count: 5,
            stale: false,
            is_pinned: false,
        }];
        assert!(
            Line::from(
                list_detail_value(&app)
                    .expect("folder detail")
                    .spans_without(&[])
            )
            .to_string()
            .contains("2+3")
        );
        app.list.visual_list = vec![crate::list_view::VisualItem::SmartFolder {
            kind: crate::list_view::SmartFolderKind::Today,
            label: "Today".into(),
            depth: 0,
            is_expanded: false,
            note_count: 5,
        }];
        assert_eq!(
            Line::from(
                list_detail_value(&app)
                    .expect("smart detail")
                    .spans_without(&[])
            )
            .to_string()
            .trim(),
            "5"
        );
    }
    fn render_list_header_right<'a>(
        app: &'a App,
        theme: &AppThemeColors,
        max_width: Option<u16>,
    ) -> Option<Line<'a>> {
        let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::List);
        ctx.note = crate::statusline::active_note(app, ViewMode::List);
        if let Some(detail) = list_detail_value(app) {
            ctx.detail = Some(detail.spans_without(&[]));
            ctx.list_detail = Some(detail);
        }
        crate::statusline::render_header_right(
            &ctx,
            &app.config.statusline,
            ViewMode::List,
            theme,
            max_width,
        )
    }

    #[test]
    fn list_header_drops_fields_by_priority() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let (_temp_dir, mut app) = grid_test_app(0);
        app.notes = vec![crate::storage::NoteSummary {
            id: "note.md".into(),
            title: "Note".into(),
            updated_at: crate::ui::now_unix_secs(),
            folder: String::new(),
            tags: vec!["alpha".into(), "beta".into()],
            pinned: false,
            links: vec![],
            size_bytes: 1_536,
        }];
        app.list.visual_list = vec![crate::list_view::VisualItem::Note {
            summary_idx: 0,
            depth: 0,
            is_clin: false,
            is_draw: false,
            is_canvas: false,
            in_virtual_pinned_folder: false,
        }];
        app.list.show_file_size = true;
        app.list.sort_field = crate::list_view::SortField::Modified;
        app.list.sort_order = crate::list_view::SortOrder::Descending;
        app.config.ui.icon_mode = crate::config::IconMode::None;
        app.config.statusline.list = Some(crate::config::StatuslineOverride {
            header_left: None,
            header_right: Some("L {sort} {detail} {tags} {note_updated_rel} {note_size} R".into()),
            footer_left: None,
            footer_right: None,
        });

        for style in [
            crate::config::HintBarStyle::Classic,
            crate::config::HintBarStyle::Sharp,
            crate::config::HintBarStyle::Bubbles,
            crate::config::HintBarStyle::Brackets,
            crate::config::HintBarStyle::Hexagon,
        ] {
            let theme = AppThemeColors {
                hint_bar_style: style,
                ..Default::default()
            };
            let full = render_list_header_right(&app, &theme, None).expect("full header");
            assert!(full.to_string().contains("1.5 KB"));
            let without_size =
                render_list_header_right(&app, &theme, Some(full.width().saturating_sub(1) as u16))
                    .expect("header without size");
            assert!(!without_size.to_string().contains("1.5 KB"), "{style:?}");
            let without_age = render_list_header_right(
                &app,
                &theme,
                Some(without_size.width().saturating_sub(1) as u16),
            )
            .expect("header without age");
            assert!(!without_age.to_string().contains("just now"), "{style:?}");
            let without_sort = render_list_header_right(
                &app,
                &theme,
                Some(without_age.width().saturating_sub(1) as u16),
            )
            .expect("header without sort");
            assert!(!without_sort.to_string().contains("M▼"), "{style:?}");
            let literals_only = render_list_header_right(
                &app,
                &theme,
                Some(without_sort.width().saturating_sub(1) as u16),
            )
            .expect("literal header");
            assert!(literals_only.to_string().contains('L'), "{style:?}");
            assert!(literals_only.to_string().contains('R'), "{style:?}");
        }

        app.config
            .statusline
            .list
            .as_mut()
            .expect("list override")
            .header_right = Some("{sort} {detail}".into());
        app.list.visual_list = vec![crate::list_view::VisualItem::Folder {
            path: "folder".into(),
            name: "folder".into(),
            depth: 0,
            is_expanded: false,
            note_count: 2,
            recursive_count: 5,
            stale: false,
            is_pinned: false,
        }];
        let theme = AppThemeColors::default();
        let full = render_list_header_right(&app, &theme, None).expect("folder header");
        let without_count =
            render_list_header_right(&app, &theme, Some(full.width().saturating_sub(1) as u16))
                .expect("folder header without count");
        assert!(!without_count.to_string().contains("2+3"));
        assert!(without_count.to_string().contains("M▼"));
    }
}
