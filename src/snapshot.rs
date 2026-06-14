use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use ratatui::symbols::Marker;
use ratatui::widgets::canvas::{Canvas, Context, Line as CanvasLine, Rectangle};
use ratatui::widgets::*;

use crate::app_theme::AppThemeColors;
use crate::draw::state::{DrawData, DrawElement, Shape, Stroke};
use crate::pinstar::data::{CanvasData, CanvasNode};

const PREVIEW_COLS: u16 = 78;

const PREVIEW_ROWS: u16 = 38;

pub fn render_canvas_snapshot(
    data: &CanvasData,
    theme: &AppThemeColors,
) -> Vec<Vec<(char, Style)>> {
    let width = PREVIEW_COLS;
    let height = PREVIEW_ROWS;

    if data.nodes.is_empty() || width == 0 || height == 0 {
        return empty_grid(width, height);
    }

    let (bounds_min_x, bounds_min_y, bounds_max_x, bounds_max_y) = canvas_bounds(data);
    let padding = 50.0;
    let min_x = bounds_min_x - padding;
    let min_y = bounds_min_y - padding;
    let max_x = bounds_max_x + padding;
    let max_y = bounds_max_y + padding;

    let content_w = (max_x - min_x).max(1.0);
    let content_h = (max_y - min_y).max(1.0);

    let zoom_x = (width as f64 - 4.0) / content_w;
    let zoom_y = (height as f64 - 4.0) / content_h;
    let zoom = zoom_x.min(zoom_y).clamp(0.01, 10.0);

    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    let _ = terminal.draw(|frame| {
        let area = frame.area();
        let preview_bg = theme.preview_bg();
        fill_buf_bg(frame.buffer_mut(), area, preview_bg);
        let buf = frame.buffer_mut();

        for edge in &data.edges {
            let from = data.nodes.iter().find(|n| n.id() == edge.from_node);
            let to = data.nodes.iter().find(|n| n.id() == edge.to_node);
            if let (Some(f), Some(t)) = (from, to) {
                let (fx, fy) = f.pos();
                let (fw, fh) = f.size();
                let (tx, ty) = t.pos();
                let (tw, th) = t.size();

                let ax = fx + fw / 2.0;
                let ay = fy + fh / 2.0;
                let bx = tx + tw / 2.0;
                let by = ty + th / 2.0;

                let sfx = ((ax - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0);
                let sfy = ((ay - center_y) * zoom) + (area.y as f64 + area.height as f64 / 2.0);
                let stx = ((bx - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0);
                let sty = ((by - center_y) * zoom) + (area.y as f64 + area.height as f64 / 2.0);

                draw_braille_line(buf, sfx, sfy, stx, sty, theme.muted);
            }
        }

        for node in &data.nodes {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();

            let sx = ((nx - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0);
            let sy = ((ny - center_y) * zoom) + (area.y as f64 + area.height as f64 / 2.0);
            let sw = (nw * zoom).max(4.0);
            let sh = (nh * zoom).max(2.0);

            if sx + sw < area.left() as f64
                || sx > area.right() as f64
                || sy + sh < area.top() as f64
                || sy > area.bottom() as f64
            {
                continue;
            }

            let left = sx.max(area.left() as f64) as u16;
            let top = sy.max(area.top() as f64) as u16;
            let right = (sx + sw).min(area.right() as f64) as u16;
            let bottom = (sy + sh).min(area.bottom() as f64) as u16;
            if right <= left || bottom <= top {
                continue;
            }

            let node_rect = Rect::new(left, top, right - left, bottom - top);

            let color_str = match node {
                CanvasNode::Text(n) => n.color.as_deref(),
                CanvasNode::File(n) => n.color.as_deref(),
                CanvasNode::Link(n) => n.color.as_deref(),
                CanvasNode::Group(_) => None,
            };
            let node_color = canvas_color_to_style(color_str, theme);

            let title = match node {
                CanvasNode::File(n) => std::path::Path::new(&n.file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&n.file)
                    .to_string(),
                CanvasNode::Link(n) => n.url.clone(),
                _ => node.id().to_string(),
            };

            let inner_text = node.text();

            let max_text_len = (node_rect.width.saturating_sub(2) as usize).min(
                node_rect.height.saturating_sub(2) as usize
                    * node_rect.width.saturating_sub(2) as usize,
            );
            let display_text = if inner_text.len() > max_text_len && max_text_len > 10 {
                format!("{}…", &inner_text[..max_text_len.saturating_sub(1)])
            } else {
                inner_text.to_string()
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(node_color))
                .title(title)
                .style(theme.bg_style());

            let text = Paragraph::new(display_text)
                .block(block)
                .style(Style::default().fg(theme.fg))
                .wrap(Wrap { trim: false });

            frame.render_widget(Clear, node_rect);
            frame.render_widget(text, node_rect);
        }
    });

    extract_grid(terminal, width, height)
}

pub fn render_draw_snapshot(data: &DrawData, theme: &AppThemeColors) -> Vec<Vec<(char, Style)>> {
    let width = PREVIEW_COLS;
    let height = PREVIEW_ROWS;

    if width == 0 || height == 0 {
        return empty_grid(width, height);
    }

    let (min_x, min_y, max_x, max_y) = draw_bounds(data);
    let padding = 20.0;
    let x_bounds = [min_x - padding, max_x + padding];
    let y_bounds = [min_y - padding, max_y + padding];

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    let _ = terminal.draw(|frame| {
        let area = frame.area();
        let bg_opt = theme.preview_bg();
        fill_buf_bg(frame.buffer_mut(), area, bg_opt);
        let bg_color = bg_opt.unwrap_or(Color::Reset);
        let canvas = Canvas::default()
            .block(Block::default().style(Style::default().bg(bg_color)))
            .background_color(bg_color)
            .marker(Marker::Braille)
            .x_bounds(x_bounds)
            .y_bounds(y_bounds)
            .paint(|ctx| {
                for element in &data.elements {
                    match element {
                        DrawElement::Stroke(stroke) => {
                            draw_stroke_lines(ctx, stroke);
                        }
                        DrawElement::Shape(shape) => {
                            draw_shape_on_canvas(ctx, shape);
                        }
                        DrawElement::Text(text) => {
                            let color = Color::Rgb(text.color.0, text.color.1, text.color.2);
                            ctx.print(
                                text.x,
                                text.y,
                                ratatui::text::Line::from(text.content.clone())
                                    .style(Style::default().fg(color)),
                            );
                        }
                    }
                }
            });
        frame.render_widget(canvas, frame.area());
    });

    extract_grid(terminal, width, height)
}

pub struct RenderedSnapshot<'a> {
    grid: Vec<(char, Style)>,
    cols: u16,
    rows: u16,
    scroll_offset: u16,
    block: Option<Block<'a>>,
}

impl<'a> RenderedSnapshot<'a> {
    pub fn new(grid: &[Vec<(char, Style)>]) -> Self {
        let rows = grid.len() as u16;
        let cols = grid.first().map(|r| r.len() as u16).unwrap_or(0);
        let flat: Vec<(char, Style)> = grid.iter().flat_map(|r| r.iter().copied()).collect();
        Self {
            grid: flat,
            cols,
            rows,
            scroll_offset: 0,
            block: None,
        }
    }

    pub fn scroll_offset(mut self, offset: u16) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl Widget for RenderedSnapshot<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = if let Some(block) = self.block {
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };

        let scroll = self.scroll_offset as usize;
        for (row_idx, buf_y) in (area.top()..area.bottom()).enumerate() {
            let src_row = scroll + row_idx;
            if src_row >= self.rows as usize {
                break;
            }
            for (col_idx, buf_x) in (area.left()..area.right()).enumerate() {
                if col_idx >= self.cols as usize {
                    break;
                }
                let idx = src_row * self.cols as usize + col_idx;
                if idx >= self.grid.len() {
                    break;
                }
                let (ch, style) = self.grid[idx];
                if let Some(cell) = buf.cell_mut((buf_x, buf_y)) {
                    cell.set_char(ch).set_style(style);
                }
            }
        }
    }
}

#[allow(deprecated)]
fn fill_buf_bg(buf: &mut Buffer, area: Rect, bg: Option<Color>) {
    let Some(bg) = bg else { return };
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y))
                && !cell.skip
            {
                cell.set_style(Style::default().bg(bg));
            }
        }
    }
}

fn empty_grid(width: u16, height: u16) -> Vec<Vec<(char, Style)>> {
    let empty_style = Style::default();
    (0..height)
        .map(|_| vec![(' ', empty_style); width as usize])
        .collect()
}

fn extract_grid(
    terminal: Terminal<TestBackend>,
    width: u16,
    height: u16,
) -> Vec<Vec<(char, Style)>> {
    let buffer = terminal.backend().buffer();
    let mut grid = Vec::with_capacity(height as usize);
    for y in 0..height {
        let mut row = Vec::with_capacity(width as usize);
        for x in 0..width {
            if let Some(cell) = buffer.cell((x, y)) {
                let ch = cell.symbol().chars().next().unwrap_or(' ');
                row.push((ch, cell.style()));
            } else {
                row.push((' ', Style::default()));
            }
        }
        grid.push(row);
    }
    grid
}

fn canvas_bounds(data: &CanvasData) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for node in &data.nodes {
        let (x, y) = node.pos();
        let (w, h) = node.size();
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }
    if min_x == f64::MAX {
        (0.0, 0.0, 100.0, 100.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

fn draw_bounds(data: &DrawData) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for el in &data.elements {
        match el {
            DrawElement::Stroke(s) => {
                for &(px, py) in &s.points {
                    min_x = min_x.min(px);
                    min_y = min_y.min(py);
                    max_x = max_x.max(px);
                    max_y = max_y.max(py);
                }
            }
            DrawElement::Shape(shape) => match shape {
                Shape::Rect {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    min_x = min_x.min(*x);
                    min_y = min_y.min(*y);
                    max_x = max_x.max(x + width);
                    max_y = max_y.max(y + height);
                }
                Shape::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    min_x = min_x.min(*x);
                    min_y = min_y.min(*y);
                    max_x = max_x.max(x + width);
                    max_y = max_y.max(y + height);
                }
                Shape::Diamond {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    min_x = min_x.min(*x);
                    min_y = min_y.min(*y);
                    max_x = max_x.max(x + width);
                    max_y = max_y.max(y + height);
                }
                Shape::Line { x1, y1, x2, y2, .. } => {
                    min_x = min_x.min(x1.min(*x2));
                    min_y = min_y.min(y1.min(*y2));
                    max_x = max_x.max(x1.max(*x2));
                    max_y = max_y.max(y1.max(*y2));
                }
                Shape::Arrow { x1, y1, x2, y2, .. } => {
                    min_x = min_x.min(x1.min(*x2));
                    min_y = min_y.min(y1.min(*y2));
                    max_x = max_x.max(x1.max(*x2));
                    max_y = max_y.max(y1.max(*y2));
                }
            },
            DrawElement::Text(t) => {
                min_x = min_x.min(t.x);
                min_y = min_y.min(t.y);
                max_x = max_x.max(t.x + t.content.len() as f64 * 8.0);
                max_y = max_y.max(t.y + 12.0);
            }
        }
    }
    if min_x == f64::MAX {
        (0.0, 0.0, 100.0, 100.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

fn canvas_color_to_style(color: Option<&str>, theme: &AppThemeColors) -> Color {
    match color {
        Some(s) if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
            Color::Rgb(r, g, b)
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

fn draw_braille_line(buf: &mut Buffer, mut x1: f64, mut y1: f64, x2: f64, y2: f64, color: Color) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist * 2.0) as usize;
    if steps == 0 {
        return;
    }
    let sx = dx / steps as f64;
    let sy = dy / steps as f64;

    for _ in 0..=steps {
        let cx = x1 as u16;
        let cy = y1 as u16;
        let dot_x = ((x1 - cx as f64) * 2.0) as u16;
        let dot_y = ((y1 - cy as f64) * 4.0) as u16;

        if let Some(cell) = buf.cell_mut((cx, cy)) {
            let mut braile_code = match cell.symbol().chars().next() {
                Some(c) if ('\u{2800}'..='\u{28FF}').contains(&c) => c as u32 - 0x2800,
                _ => 0,
            };
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
            braile_code |= dot_bit;
            if let Some(c) = char::from_u32(0x2800 + braile_code) {
                cell.set_char(c).set_fg(color);
            }
        }

        x1 += sx;
        y1 += sy;
    }
}

fn draw_stroke_lines(ctx: &mut Context, stroke: &Stroke) {
    let color = Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2);
    for window in stroke.points.windows(2) {
        if let [p1, p2] = window {
            ctx.draw(&CanvasLine {
                x1: p1.0,
                y1: p1.1,
                x2: p2.0,
                y2: p2.1,
                color,
            });
        }
    }
}

fn draw_shape_on_canvas(ctx: &mut Context, shape: &Shape) {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            color,
        } => {
            ctx.draw(&Rectangle {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color: Color::Rgb(color.0, color.1, color.2),
            });
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            let rx = width / 2.0;
            let ry = height / 2.0;
            let cx_center = x + rx;
            let cy_center = y + ry;
            let segments = 32;
            for i in 0..segments {
                let angle1 = (i as f64 / segments as f64) * 2.0 * std::f64::consts::PI;
                let angle2 = ((i + 1) as f64 / segments as f64) * 2.0 * std::f64::consts::PI;
                ctx.draw(&CanvasLine {
                    x1: cx_center + rx * angle1.cos(),
                    y1: cy_center + ry * angle1.sin(),
                    x2: cx_center + rx * angle2.cos(),
                    y2: cy_center + ry * angle2.sin(),
                    color,
                });
            }
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            let p1 = (x + width / 2.0, *y);
            let p2 = (x + width, y + height / 2.0);
            let p3 = (x + width / 2.0, y + height);
            let p4 = (*x, y + height / 2.0);
            for (start, end) in [(p1, p2), (p2, p3), (p3, p4), (p4, p1)] {
                ctx.draw(&CanvasLine {
                    x1: start.0,
                    y1: start.1,
                    x2: end.0,
                    y2: end.1,
                    color,
                });
            }
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            ctx.draw(&CanvasLine {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                color: Color::Rgb(color.0, color.1, color.2),
            });
        }
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            ctx.draw(&CanvasLine {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                color,
            });
            let angle = (y2 - y1).atan2(x2 - x1);
            let head_len = 5.0;
            let head_angle = std::f64::consts::PI / 6.0;
            ctx.draw(&CanvasLine {
                x1: *x2,
                y1: *y2,
                x2: x2 - head_len * (angle - head_angle).cos(),
                y2: y2 - head_len * (angle - head_angle).sin(),
                color,
            });
            ctx.draw(&CanvasLine {
                x1: *x2,
                y1: *y2,
                x2: x2 - head_len * (angle + head_angle).cos(),
                y2: y2 - head_len * (angle + head_angle).sin(),
                color,
            });
        }
    }
}
