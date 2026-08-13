use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::app_theme::AppThemeColors;
use crate::ui::{paint_list_hover, render_list_with_selection};

pub struct CanvasMenuItemSpec {
    pub label: &'static str,
    pub shortcut: Option<char>,
    pub color_hint: Option<Color>,
}

impl CanvasMenuItemSpec {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            shortcut: None,
            color_hint: None,
        }
    }
    pub const fn shortcut(mut self, c: char) -> Self {
        self.shortcut = Some(c);
        self
    }
    pub const fn color(mut self, c: Color) -> Self {
        self.color_hint = Some(c);
        self
    }
}

pub struct CanvasContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
    pub items: Vec<CanvasMenuItemSpec>,
}

impl CanvasContextMenu {
    pub fn new(x: u16, y: u16, items: Vec<CanvasMenuItemSpec>) -> Self {
        Self {
            x,
            y,
            selected: 0,
            items,
        }
    }
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }
    pub fn find_shortcut(&self, ch: char) -> Option<usize> {
        let cl = ch.to_ascii_lowercase();
        self.items
            .iter()
            .position(|i| i.shortcut.is_some_and(|s| s.to_ascii_lowercase() == cl))
    }
    pub fn rect(&self, area: Rect) -> Rect {
        let max_content = self
            .items
            .iter()
            .map(|i| {
                let base = i.label.chars().count();
                let square = if i.color_hint.is_some() { 3 } else { 0 }; // "■ "
                let shortcut = i.shortcut.map_or(0, |_| 2); // "c "
                base + square + shortcut + 4 // 2 left + 2 right pad
            })
            .max()
            .unwrap_or(0);
        let width = max_content.max(8) as u16;
        let height = self.items.len() as u16;
        let x = self
            .x
            .min(area.x.saturating_add(area.width.saturating_sub(width)));
        let y = self
            .y
            .min(area.y.saturating_add(area.height.saturating_sub(height)));
        Rect::new(x, y, width, height)
    }
    pub fn row_at(&self, rect: Rect, col: u16, row: u16) -> Option<usize> {
        if col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
        {
            let idx = (row - rect.y) as usize;
            (idx < self.items.len()).then_some(idx)
        } else {
            None
        }
    }
}

pub fn render_canvas_context_menu(
    frame: &mut Frame,
    area: Rect,
    menu: &CanvasContextMenu,
    theme: &AppThemeColors,
    mouse_pos: Option<(u16, u16)>,
) {
    let rect = menu.rect(area);
    frame.render_widget(Clear, rect);
    let items: Vec<ListItem> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let is_selected = i == menu.selected;
            let base = if is_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::styled("  ", base));
            if let Some(c) = spec.color_hint {
                spans.push(Span::styled("■ ", base.fg(c)));
            }
            let label = format!("{}  ", spec.label);
            spans.push(Span::styled(label, base));
            // dynamic padding so shortcut right-aligns.
            let content_len = spec.label.chars().count()
                + 4
                + if spec.color_hint.is_some() { 3 } else { 0 }
                + spec.shortcut.map_or(0, |_| 2);
            let pad = (rect.width as usize).saturating_sub(content_len);
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), base));
            }
            if let Some(c) = spec.shortcut {
                spans.push(Span::styled(
                    format!("{c} "),
                    Style::default().fg(theme.muted).bg(if is_selected {
                        theme.highlight_bg
                    } else {
                        Color::Reset
                    }),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .style(theme.preview_bg_style()),
    );
    let state = render_list_with_selection(frame, list, rect, Some(menu.selected), 0);
    paint_list_hover(
        frame,
        rect,
        &state,
        menu.items.len(),
        mouse_pos,
        theme.hover_style(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_width_accounts_for_color_and_shortcut() {
        let m = CanvasContextMenu::new(
            0,
            0,
            vec![
                CanvasMenuItemSpec::new("A"),
                CanvasMenuItemSpec::new("A").shortcut('c').color(Color::Red),
            ],
        );
        let rect = m.rect(Rect::new(0, 0, 80, 40));
        // "A" → 1 + 4 = 5; "A" + square(3) + shortcut(2) + 4 = 10.
        assert_eq!(rect.width, 10);
        assert_eq!(rect.height, 2);
    }

    #[test]
    fn move_down_bounds_at_last() {
        let mut m = CanvasContextMenu::new(0, 0, vec![CanvasMenuItemSpec::new("A")]);
        m.move_down();
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn move_up_saturates_at_zero() {
        let mut m = CanvasContextMenu::new(0, 0, vec![CanvasMenuItemSpec::new("A")]);
        m.move_up();
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn find_shortcut_case_insensitive() {
        let m = CanvasContextMenu::new(0, 0, vec![CanvasMenuItemSpec::new("A").shortcut('c')]);
        assert_eq!(m.find_shortcut('C'), Some(0));
        assert_eq!(m.find_shortcut('x'), None);
    }

    #[test]
    fn row_at_inside_outside() {
        let m = CanvasContextMenu::new(
            0,
            0,
            vec![CanvasMenuItemSpec::new("A"), CanvasMenuItemSpec::new("B")],
        );
        let rect = m.rect(Rect::new(0, 0, 80, 40));
        assert_eq!(m.row_at(rect, rect.x, rect.y), Some(0));
        assert_eq!(m.row_at(rect, rect.x, rect.y + 1), Some(1));
        assert_eq!(m.row_at(rect, rect.x, rect.y + 2), None);
        assert_eq!(m.row_at(rect, rect.x + rect.width, rect.y), None);
    }
}
