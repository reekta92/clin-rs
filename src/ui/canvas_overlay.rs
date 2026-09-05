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
        let (min_x, max_x) = (s.0.min(e.0), s.0.max(e.0));
        let (min_y, max_y) = (s.1.min(e.1), s.1.max(e.1));
        Some((min_x, min_y, max_x, max_y))
    }
    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
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
}
