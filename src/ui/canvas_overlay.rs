use ratatui::{Frame, layout::Rect, style::Color};

pub struct MarqueeDragState {
    pub start: Option<(f64, f64)>,
    pub end: Option<(f64, f64)>,
    pub threshold_cells: u32,
}

impl MarqueeDragState {
    pub fn new(threshold_cells: u32) -> Self {
        Self {
            start: None,
            end: None,
            threshold_cells,
        }
    }
    pub fn on_down(&mut self, x: f64, y: f64) {
        self.start = Some((x, y));
        self.end = Some((x, y));
    }
    pub fn on_drag(&mut self, x: f64, y: f64) {
        self.end = Some((x, y));
    }
    pub fn is_dragging_screen(
        &self,
        sx_now: u16,
        sy_now: u16,
        sx_start: u16,
        sy_start: u16,
    ) -> bool {
        let moved = (sx_now as i32 - sx_start as i32).unsigned_abs()
            + (sy_now as i32 - sy_start as i32).unsigned_abs();
        moved > self.threshold_cells
    }
    pub fn commit_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let s = self.start?;
        let e = self.end?;
        let (min_x, max_x) = if s.0 < e.0 { (s.0, e.0) } else { (e.0, s.0) };
        let (min_y, max_y) = if s.1 < e.1 { (s.1, e.1) } else { (e.1, s.1) };
        Some((min_x, min_y, max_x, max_y))
    }
    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }
}

/// Marquee fill color shared by graf and pinstar.
pub fn muted_canvas_selection_fill(accent: Color, highlight_bg: Color) -> Color {
    match accent {
        Color::Rgb(r, g, b) => Color::Rgb(r / 4, g / 4, b / 4),
        _ => highlight_bg,
    }
}

/// Translucent marquee fill preserving every underlying glyph and foreground.
pub fn draw_canvas_rect_filled(frame: &mut Frame, rect: Rect, fill: Color) {
    let buf = frame.buffer_mut();
    for row in rect.y..rect.y.saturating_add(rect.height) {
        for col in rect.x..rect.x.saturating_add(rect.width) {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_bg(fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_down_sets_start_end() {
        let mut m = MarqueeDragState::new(3);
        m.on_down(1.0, 2.0);
        assert_eq!(m.start, Some((1.0, 2.0)));
        assert_eq!(m.end, Some((1.0, 2.0)));
    }

    #[test]
    fn on_drag_updates_end() {
        let mut m = MarqueeDragState::new(3);
        m.on_down(1.0, 2.0);
        m.on_drag(5.0, 6.0);
        assert_eq!(m.end, Some((5.0, 6.0)));
    }

    #[test]
    fn clear_nukes_both() {
        let mut m = MarqueeDragState::new(3);
        m.on_down(1.0, 2.0);
        m.clear();
        assert_eq!(m.start, None);
        assert_eq!(m.end, None);
    }

    #[test]
    fn commit_rect_normalizes_both_directions() {
        let mut m = MarqueeDragState::new(3);
        m.on_down(10.0, 10.0);
        m.on_drag(2.0, 4.0);
        assert_eq!(m.commit_rect(), Some((2.0, 4.0, 10.0, 10.0)));

        let mut m = MarqueeDragState::new(3);
        m.on_down(2.0, 4.0);
        m.on_drag(10.0, 10.0);
        assert_eq!(m.commit_rect(), Some((2.0, 4.0, 10.0, 10.0)));
    }

    #[test]
    fn is_dragging_screen_boundary() {
        let m = MarqueeDragState::new(3);
        // Manhattan move of 3 → not dragging.
        assert!(!m.is_dragging_screen(3, 0, 0, 0));
        assert!(!m.is_dragging_screen(2, 1, 0, 0));
        // Manhattan move of 4 → dragging.
        assert!(m.is_dragging_screen(4, 0, 0, 0));
        assert!(m.is_dragging_screen(2, 2, 0, 0));
    }
    #[test]
    fn marquee_fill_preserves_underlying_glyphs() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(6, 6)).unwrap();
        terminal
            .draw(|frame| {
                for y in 1..5 {
                    for x in 1..5 {
                        frame
                            .buffer_mut()
                            .cell_mut((x, y))
                            .unwrap()
                            .set_symbol("x")
                            .set_fg(Color::Red);
                    }
                }
                draw_canvas_rect_filled(frame, Rect::new(1, 1, 4, 4), Color::Blue);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        for y in 1..5 {
            for x in 1..5 {
                let cell = buf.cell((x, y)).unwrap();
                assert_eq!(cell.symbol(), "x");
                assert_eq!(cell.style().fg, Some(Color::Red));
                assert_eq!(cell.style().bg, Some(Color::Blue));
            }
        }
    }

    #[test]
    fn muted_selection_fill_uses_accent_or_highlight_fallback() {
        assert_eq!(
            muted_canvas_selection_fill(Color::Rgb(96, 64, 32), Color::Cyan),
            Color::Rgb(24, 16, 8),
        );
        assert_eq!(
            muted_canvas_selection_fill(Color::Yellow, Color::Cyan),
            Color::Cyan,
        );
    }
}
