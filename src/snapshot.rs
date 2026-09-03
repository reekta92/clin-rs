use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use ratatui::symbols::Marker;
use ratatui::widgets::canvas::Canvas;
use ratatui::widgets::*;

use crate::app_theme::AppThemeColors;
use crate::draw::state::DrawData;
use crate::pinstar::data::{CanvasData, CanvasNode};
use unicode_width::UnicodeWidthChar;

pub fn render_canvas_snapshot(
    data: &CanvasData,
    theme: &AppThemeColors,
    icon_mode: crate::config::IconMode,
    width: u16,
    height: u16,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Vec<Vec<(char, Style)>> {
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
    let zoom = (zoom_x.min(zoom_y) * scale).clamp(0.0001, 10.0);

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
                let sfx =
                    ((ax - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0) + offset_x;
                let sfy = ((ay - center_y) * zoom)
                    + (area.y as f64 + area.height as f64 / 2.0)
                    + offset_y;
                let stx =
                    ((bx - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0) + offset_x;
                let sty = ((by - center_y) * zoom)
                    + (area.y as f64 + area.height as f64 / 2.0)
                    + offset_y;
                crate::ui::braille::draw_braille_line(buf, sfx, sfy, stx, sty, theme.muted);
            }
        }

        // Pass 1: group nodes (drawn behind child nodes)
        for node in &data.nodes {
            if !matches!(node, CanvasNode::Group(_)) {
                continue;
            }
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let sx =
                ((nx - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0) + offset_x;
            let sy =
                ((ny - center_y) * zoom) + (area.y as f64 + area.height as f64 / 2.0) + offset_y;
            let sw = (nw * zoom).max(1.0);
            let sh = (nh * zoom).max(1.0);

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

            let title = match node.title() {
                Some(t) => t.to_string(),
                None => match node {
                    CanvasNode::File(n) => std::path::Path::new(&n.file)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&n.file)
                        .to_string(),
                    CanvasNode::Link(n) => n.url.clone(),
                    CanvasNode::Group(n) => n.label.clone().unwrap_or_default(),
                    CanvasNode::Text(_) => "".to_string(),
                },
            };

            let inner_text = node.text();

            let max_text_len = (node_rect.width.saturating_sub(2) as usize)
                * (node_rect.height.saturating_sub(2) as usize);
            let display_text = if inner_text.chars().count() > max_text_len && max_text_len > 10 {
                let mut s: String = inner_text
                    .chars()
                    .take(max_text_len.saturating_sub(1))
                    .collect();
                s.push('…');
                s
            } else {
                inner_text.to_string()
            };
            let is_image = matches!(node, CanvasNode::File(n) if is_image_ext(&n.file));

            if is_image {
                let icon = crate::ui::get_icon("\u{f03e}", "\u{1f5bc}", icon_mode);
                let filled_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(node_color))
                    .title(title)
                    .style(
                        Style::default()
                            .bg(node_color)
                            .fg(theme.bg.unwrap_or(Color::Reset)),
                    );

                let icon_line = Line::from(Span::styled(
                    icon,
                    Style::default().fg(theme.bg.unwrap_or(Color::Reset)),
                ))
                .alignment(Alignment::Center);

                let content_height = node_rect.height.saturating_sub(2);
                let empty_count = content_height / 2;

                let mut lines: Vec<Line> = Vec::new();
                for _ in 0..empty_count {
                    lines.push(Line::from(""));
                }
                lines.push(icon_line);

                let text = Paragraph::new(lines).block(filled_block);
                frame.render_widget(Clear, node_rect);
                frame.render_widget(text, node_rect);
            } else {
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
        }

        // Pass 2: non-group nodes (drawn on top of groups)
        for node in &data.nodes {
            if matches!(node, CanvasNode::Group(_)) {
                continue;
            }
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let sx =
                ((nx - center_x) * zoom) + (area.x as f64 + area.width as f64 / 2.0) + offset_x;
            let sy =
                ((ny - center_y) * zoom) + (area.y as f64 + area.height as f64 / 2.0) + offset_y;
            let sw = (nw * zoom).max(1.0);
            let sh = (nh * zoom).max(1.0);

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

            let title = match node.title() {
                Some(t) => t.to_string(),
                None => match node {
                    CanvasNode::File(n) => std::path::Path::new(&n.file)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&n.file)
                        .to_string(),
                    CanvasNode::Link(n) => n.url.clone(),
                    CanvasNode::Group(n) => n.label.clone().unwrap_or_default(),
                    CanvasNode::Text(_) => "".to_string(),
                },
            };

            let inner_text = node.text();

            let max_text_len = (node_rect.width.saturating_sub(2) as usize)
                * (node_rect.height.saturating_sub(2) as usize);
            let display_text = if inner_text.chars().count() > max_text_len && max_text_len > 10 {
                let mut s: String = inner_text
                    .chars()
                    .take(max_text_len.saturating_sub(1))
                    .collect();
                s.push('…');
                s
            } else {
                inner_text.to_string()
            };
            let is_image = matches!(node, CanvasNode::File(n) if is_image_ext(&n.file));

            if is_image {
                let icon = crate::ui::get_icon("\u{f03e}", "\u{1f5bc}", icon_mode);
                let filled_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(node_color))
                    .title(title)
                    .style(
                        Style::default()
                            .bg(node_color)
                            .fg(theme.bg.unwrap_or(Color::Reset)),
                    );

                let icon_line = Line::from(Span::styled(
                    icon,
                    Style::default().fg(theme.bg.unwrap_or(Color::Reset)),
                ))
                .alignment(Alignment::Center);

                let content_height = node_rect.height.saturating_sub(2);
                let empty_count = content_height / 2;

                let mut lines: Vec<Line> = Vec::new();
                for _ in 0..empty_count {
                    lines.push(Line::from(""));
                }
                lines.push(icon_line);

                let text = Paragraph::new(lines).block(filled_block);
                frame.render_widget(Clear, node_rect);
                frame.render_widget(text, node_rect);
            } else {
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
        }
    });

    extract_grid(terminal, width, height)
}

pub fn render_draw_snapshot_with_size(
    data: &DrawData,
    theme: &AppThemeColors,
    icon_mode: crate::config::IconMode,
    width: u16,
    height: u16,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Vec<Vec<(char, Style)>> {
    render_draw_snapshot_with_bg(
        data,
        theme,
        icon_mode,
        width,
        height,
        scale,
        offset_x,
        offset_y,
        theme.preview_bg(),
    )
}
#[allow(clippy::too_many_arguments)]
pub fn render_draw_snapshot_with_bg(
    data: &DrawData,
    _theme: &AppThemeColors,
    _icon_mode: crate::config::IconMode,
    width: u16,
    height: u16,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
    bg_opt: Option<Color>,
) -> Vec<Vec<(char, Style)>> {
    if width == 0 || height == 0 {
        return empty_grid(width, height);
    }

    let (min_x, min_y, max_x, max_y) = draw_bounds(data);
    let padding = 20.0;
    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;
    let hw = ((max_x - min_x) / 2.0 + padding).max(10.0) / scale;
    let hh = ((max_y - min_y) / 2.0 + padding).max(10.0) / scale;
    let ratio_x = (2.0 * hw) / width as f64;
    let ratio_y = (2.0 * hh) / height as f64;
    let cx_shifted = cx - offset_x * ratio_x;
    let cy_shifted = cy + offset_y * ratio_y;
    let x_bounds = [cx_shifted - hw, cx_shifted + hw];
    let y_bounds = [cy_shifted - hh, cy_shifted + hh];

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    let _ = terminal.draw(|frame| {
        let area = frame.area();
        fill_buf_bg(frame.buffer_mut(), area, bg_opt);
        let bg_color = bg_opt.unwrap_or(Color::Reset);
        let canvas = Canvas::default()
            .block(Block::default().style(Style::default().bg(bg_color)))
            .background_color(bg_color)
            .marker(Marker::Braille)
            .x_bounds(x_bounds)
            .y_bounds(y_bounds)
            .paint(|ctx| {
                for item in data.elements.iter().filter(|item| {
                    !matches!(&item.element, crate::draw::state::DrawElement::Text(_))
                }) {
                    crate::draw::render::draw_item(ctx, item);
                }
                for item in data.elements.iter().filter(|item| {
                    matches!(&item.element, crate::draw::state::DrawElement::Text(_))
                }) {
                    crate::draw::render::draw_item(ctx, item);
                }
            });
        frame.render_widget(canvas, frame.area());
    });

    extract_grid(terminal, width, height)
}

pub struct RenderedSnapshot<'a> {
    grid: &'a [Vec<(char, Style)>],
    scroll_offset: u16,
    block: Option<Block<'a>>,
}

impl<'a> RenderedSnapshot<'a> {
    pub fn new(grid: &'a [Vec<(char, Style)>]) -> Self {
        Self {
            grid,
            scroll_offset: 0,
            block: None,
        }
    }

    #[must_use]
    pub fn scroll_offset(mut self, offset: u16) -> Self {
        self.scroll_offset = offset;
        self
    }

    #[must_use]
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

        let rows = self.grid.len();

        let scroll = self.scroll_offset as usize;
        for (row_idx, buf_y) in (area.top()..area.bottom()).enumerate() {
            let src_row = scroll + row_idx;
            if src_row >= rows {
                break;
            }
            let row = &self.grid[src_row];
            let mut buf_x = area.left();
            for &(ch, style) in row {
                if buf_x >= area.right() {
                    break;
                }
                let (safe_ch, w) = if ch.is_control() {
                    (' ', 1u16)
                } else {
                    (ch, UnicodeWidthChar::width(ch).unwrap_or(0) as u16)
                };
                if w == 0 {
                    continue;
                }
                if buf_x as u32 + w as u32 > area.right() as u32 {
                    break;
                }
                if let Some(cell) = buf.cell_mut((buf_x, buf_y)) {
                    cell.set_char(safe_ch).set_style(style);
                }
                buf_x += w;
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
    for item in &data.elements {
        if let Some(bounds) = crate::draw::geometry::transformed_bounds(item) {
            min_x = min_x.min(bounds.min_x);
            min_y = min_y.min(bounds.min_y);
            max_x = max_x.max(bounds.max_x);
            max_y = max_y.max(bounds.max_y);
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
        Some(s) if s.starts_with('#') => crate::config::parse_hex_color(s).unwrap_or(theme.accent),
        Some("1") | Some("red") => Color::Rgb(255, 82, 82),
        Some("2") | Some("orange") => Color::Rgb(255, 152, 0),
        Some("3") | Some("yellow") => Color::Rgb(255, 235, 59),
        Some("4") | Some("green") => Color::Rgb(76, 175, 80),
        Some("5") | Some("cyan") => Color::Rgb(0, 188, 212),
        Some("6") | Some("purple") => Color::Rgb(156, 39, 176),
        _ => theme.accent,
    }
}

/// Check if a file path has a common image extension.
fn is_image_ext(file: &str) -> bool {
    let ext = match std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
    {
        Some(e) => e.to_ascii_lowercase(),
        None => return false,
    };
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif" | "avif"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::state::{DrawElement, DrawItem, Shape};
    use crate::pinstar::data::{GroupNode, TextNode};
    use ratatui::backend::TestBackend;

    /// Regression guard: a grid cell containing a control char must not
    /// reach `Cell::set_char` as-is (ratatui debug-assert!-panics on
    /// control chars in `cell_width.rs`).
    #[test]
    fn rendered_snapshot_replaces_control_char() {
        let grid = vec![vec![('\n', Style::default())]];
        let snapshot = RenderedSnapshot::new(&grid);

        let backend = TestBackend::new(1, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(snapshot, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), " ", "control char replaced by space");
    }

    /// Wide chars (e.g. CJK) must advance buf_x by their visual width so
    /// the next grid cell lands after the continuation cell.
    #[test]
    fn rendered_snapshot_writes_wide_char_to_two_cells() {
        let grid = vec![vec![('中', Style::default()), ('b', Style::default())]];
        let snapshot = RenderedSnapshot::new(&grid);

        let backend = TestBackend::new(4, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(snapshot, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        // '中' occupies cells (0,0) (visual) — cell (1,0) is NOT 'b'
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "中");
        // 'b' lands at (2,0) — AFTER the wide-char continuation cell
        assert_ne!(
            buffer.cell((1, 0)).unwrap().symbol(),
            "b",
            "cell (1,0) should not be 'b' — wide char '中' occupies cols 0-1"
        );
        assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), "b");
    }

    #[test]
    fn canvas_snapshot_fits_wide_content() {
        let data = CanvasData {
            nodes: vec![
                CanvasNode::Text(TextNode {
                    id: "n1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    width: 500.0,
                    height: 700.0,
                    text: "A".to_string(),
                    title: None,
                    color: None,
                }),
                CanvasNode::Text(TextNode {
                    id: "n2".to_string(),
                    x: 7000.0,
                    y: 0.0,
                    width: 500.0,
                    height: 700.0,
                    text: "B".to_string(),
                    title: None,
                    color: None,
                }),
            ],
            edges: vec![],
        };
        let theme = AppThemeColors::default();
        let grid = render_canvas_snapshot(
            &data,
            &theme,
            crate::config::IconMode::default(),
            40,
            20,
            1.0,
            0.0,
            0.0,
        );
        let left_has_content = (0..=19).any(|col| {
            grid.iter()
                .any(|row| row.get(col).is_some_and(|(ch, _)| *ch != ' '))
        });
        assert!(
            left_has_content,
            "left half should have content from node near x=0"
        );
        let right_has_content = (20..=39).any(|col| {
            grid.iter()
                .any(|row| row.get(col).is_some_and(|(ch, _)| *ch != ' '))
        });
        assert!(
            right_has_content,
            "right half should have content from node near x=7000"
        );
    }

    #[test]
    fn draw_snapshot_renders_content() {
        let data = DrawData {
            version: crate::draw::state::DRAW_SCHEMA_VERSION,
            width: 500.0,
            height: 500.0,
            background: None,
            elements: vec![DrawItem::new(DrawElement::Shape(Shape::Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                color: (255, 0, 0),
            }))],
        };
        let theme = AppThemeColors::default();
        let grid = render_draw_snapshot_with_size(
            &data,
            &theme,
            crate::config::IconMode::default(),
            40,
            10,
            1.0,
            0.0,
            0.0,
        );
        // Per-axis scaling: drawing fills the pane (no letterboxing from uniform fit)
        let has_content = grid.iter().any(|row| row.iter().any(|(ch, _)| *ch != ' '));
        assert!(has_content, "drawing must be visible in the preview");
    }

    /// Regression: wide canvas bounds remain visible at preview size.
    #[test]
    fn canvas_snapshot_fits_wide_fixture() {
        let data = CanvasData {
            nodes: vec![
                CanvasNode::Text(TextNode {
                    id: "left".to_string(),
                    x: 0.0,
                    y: 0.0,
                    width: 500.0,
                    height: 700.0,
                    text: "Left".to_string(),
                    title: None,
                    color: None,
                }),
                CanvasNode::Text(TextNode {
                    id: "right".to_string(),
                    x: 7180.0,
                    y: 0.0,
                    width: 500.0,
                    height: 700.0,
                    text: "Right".to_string(),
                    title: None,
                    color: None,
                }),
            ],
            edges: vec![],
        };
        let theme = AppThemeColors::default();
        let grid = render_canvas_snapshot(
            &data,
            &theme,
            crate::config::IconMode::default(),
            78,
            38,
            1.0,
            0.0,
            0.0,
        );
        assert_eq!(grid.len(), 38, "height matches");
        assert_eq!(grid[0].len(), 78, "width matches");
        assert!(
            grid.iter().any(|row| row.iter().any(|(ch, _)| *ch != ' ')),
            "wide fixture must produce visible content"
        );
    }

    /// Verify file nodes inside groups are visible (not cleared by later group render).
    #[test]
    fn file_nodes_visible_inside_groups() {
        let data = CanvasData {
            nodes: vec![
                CanvasNode::Text(TextNode {
                    id: "f1".to_string(),
                    x: 100.0,
                    y: 100.0,
                    width: 200.0,
                    height: 100.0,
                    text: "hello".to_string(),
                    title: Some("file1".to_string()),
                    color: Some("#ff0000".to_string()),
                }),
                CanvasNode::Group(GroupNode {
                    id: "g1".to_string(),
                    x: 50.0,
                    y: 50.0,
                    width: 300.0,
                    height: 200.0,
                    label: Some("group".to_string()),
                    color: Some("#0000ff".to_string()),
                }),
            ],
            edges: vec![],
        };
        let theme = AppThemeColors::default();
        let grid = render_canvas_snapshot(
            &data,
            &theme,
            crate::config::IconMode::default(),
            60,
            30,
            1.0,
            0.0,
            0.0,
        );
        let has_file_content = grid.iter().any(|row| {
            let s: String = row.iter().map(|(ch, _)| *ch).collect();
            s.contains("hello") || s.contains("file1")
        });
        assert!(
            has_file_content,
            "file node must render on top of group, not be cleared"
        );
    }
}
