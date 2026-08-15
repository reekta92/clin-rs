use ratatui::{Frame, layout::Rect, style::Color};

/// Transient visibility state for canvas-like live views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanvasGridState {
    pub visible: bool,
}

impl Default for CanvasGridState {
    fn default() -> Self {
        Self { visible: true }
    }
}

impl CanvasGridState {
    pub(crate) fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

/// Affine world-to-terminal projection used by [`draw_canvas_grid`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasGridProjection {
    pub world_left: f64,
    pub world_right: f64,
    pub world_top: f64,
    pub world_bottom: f64,
    pub origin_col: f64,
    pub origin_row: f64,
    pub cols_per_world_x: f64,
    pub rows_per_world_y: f64,
}

impl CanvasGridProjection {
    fn is_valid(self) -> bool {
        [
            self.world_left,
            self.world_right,
            self.world_top,
            self.world_bottom,
            self.origin_col,
            self.origin_row,
            self.cols_per_world_x,
            self.rows_per_world_y,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.cols_per_world_x != 0.0
            && self.rows_per_world_y != 0.0
    }
}

/// Draw adaptive grid dots before view content so later view rendering replaces them.
pub(crate) fn draw_canvas_grid(
    frame: &mut Frame,
    area: Rect,
    state: CanvasGridState,
    projection: CanvasGridProjection,
    muted: Color,
    zoom: f64,
) {
    if !state.visible
        || area.is_empty()
        || !projection.is_valid()
        || !zoom.is_finite()
        || zoom <= 0.0
    {
        return;
    }

    let min_x = projection.world_left.min(projection.world_right);
    let max_x = projection.world_left.max(projection.world_right);
    let min_y = projection.world_top.min(projection.world_bottom);
    let max_y = projection.world_top.max(projection.world_bottom);
    let mut grid_step_x: f64 = 100.0;
    let mut grid_step_y: f64 = 100.0;
    while grid_step_y * zoom < 6.0 {
        grid_step_x *= 2.0;
        grid_step_y *= 2.0;
    }
    // Compensate for terminal cell aspect ratio (~2:1 height:width) so grid appears square
    grid_step_y *= projection.cols_per_world_x.abs() / (2.0 * projection.rows_per_world_y.abs());
    let step_x = grid_step_x;
    let step_y = grid_step_y;
    if !step_x.is_finite() || !step_y.is_finite() || step_x == 0.0 || step_y == 0.0 {
        return;
    }

    let Some(start_x) = grid_index(min_x, step_x, f64::floor) else {
        return;
    };
    let Some(end_x) = grid_index(max_x, step_x, f64::ceil) else {
        return;
    };
    let Some(start_y) = grid_index(min_y, step_y, f64::floor) else {
        return;
    };
    let Some(end_y) = grid_index(max_y, step_y, f64::ceil) else {
        return;
    };

    let width = i64::from(area.width);
    let height = i64::from(area.height);
    let max_dots = width.saturating_mul(height).saturating_mul(4).max(1);
    let x_count = end_x.saturating_sub(start_x).saturating_add(1);
    let y_count = end_y.saturating_sub(start_y).saturating_add(1);
    if x_count.saturating_mul(y_count) > max_dots {
        return;
    }

    let left = f64::from(area.left());
    let right = f64::from(area.right());
    let top = f64::from(area.top());
    let bottom = f64::from(area.bottom());
    let buffer = frame.buffer_mut();
    for x_index in start_x..=end_x {
        let world_x = x_index as f64 * step_x;
        let col = projection.origin_col + world_x * projection.cols_per_world_x;
        if !col.is_finite() {
            continue;
        }
        let col = col.round();
        if col < left || col >= right {
            continue;
        }
        for y_index in start_y..=end_y {
            let world_y = y_index as f64 * step_y;
            let row = projection.origin_row + world_y * projection.rows_per_world_y;
            if !row.is_finite() {
                continue;
            }
            let row = row.round();
            if row < top || row >= bottom {
                continue;
            }
            if let Some(cell) = buffer.cell_mut((col as u16, row as u16)) {
                cell.set_char('·').set_fg(muted);
            }
        }
    }
}

fn grid_index(value: f64, step: f64, round: fn(f64) -> f64) -> Option<i64> {
    let index = round(value / step);
    (index >= i64::MIN as f64 && index <= i64::MAX as f64).then_some(index as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn projection(
        cols_per_world_x: f64,
        rows_per_world_y: f64,
        step_x: f64,
        step_y: f64,
    ) -> CanvasGridProjection {
        let (world_top, world_bottom) = if rows_per_world_y.is_sign_negative() {
            (-step_y, 0.0)
        } else {
            (0.0, step_y)
        };
        CanvasGridProjection {
            world_left: 0.0,
            world_right: step_x,
            world_top,
            world_bottom,
            origin_col: 0.0,
            origin_row: 0.0,
            cols_per_world_x,
            rows_per_world_y,
        }
    }

    fn render_grid(
        area: Rect,
        state: CanvasGridState,
        projection: CanvasGridProjection,
        zoom: f64,
    ) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(24, 16)).unwrap();
        terminal
            .draw(|frame| draw_canvas_grid(frame, area, state, projection, Color::DarkGray, zoom))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn preserves_pinstar_density_across_projections() {
        let area = Rect::new(0, 0, 21, 11);
        let zoom = 1.0;
        // At zoom=1.0:
        // step_x = 100
        // cols_per_world_x = 0.2, rows_per_world_y = 0.1
        // compensation = 0.2 / (2 * 0.1) = 1.0
        // step_y = 100 * 1.0 = 100.0
        // If we want a dot at col 20 and row 10, then cols_per_world_x * 100 = 20 => 0.2
        // and rows_per_world_y * 100 = 10 => 0.1
        for proj in [
            projection(0.2, 0.1, 100.0, 100.0),
            projection(0.2, -0.1, 100.0, 100.0),
        ] {
            let buffer = render_grid(area, CanvasGridState::default(), proj, zoom);
            assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "·");
            assert_eq!(buffer.cell((20, 10)).unwrap().symbol(), "·");
            assert_eq!(buffer.cell((10, 5)).unwrap().symbol(), " ");
        }
    }

    #[test]
    fn projects_clips_and_styles_dots_once() {
        let area = Rect::new(2, 3, 10, 7);
        let mut p = projection(0.2, 0.1, 100.0, 100.0);
        p.origin_col = 2.0;
        p.origin_row = 3.0;
        let buffer = render_grid(area, CanvasGridState::default(), p, 1.0);

        let dot = buffer.cell((2, 3)).unwrap();
        assert_eq!(dot.symbol(), "·");
        assert_eq!(dot.style().fg, Some(Color::DarkGray));
        assert_eq!(buffer.cell((1, 3)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((2, 10)).unwrap().symbol(), " ");
    }

    #[test]
    fn hidden_or_invalid_grid_leaves_buffer_unchanged() {
        let area = Rect::new(0, 0, 21, 11);
        let mut hidden = CanvasGridState::default();
        hidden.toggle();
        assert_eq!(
            render_grid(area, hidden, projection(0.1, 0.1, 100.0, 50.0), 1.0)
                .cell((0, 0))
                .unwrap()
                .symbol(),
            " "
        );

        let mut invalid = projection(0.1, 0.1, 100.0, 50.0);
        invalid.origin_col = f64::NAN;
        assert_eq!(
            render_grid(area, CanvasGridState::default(), invalid, 1.0)
                .cell((0, 0))
                .unwrap()
                .symbol(),
            " "
        );
    }

    #[test]
    fn later_content_replaces_grid_dot() {
        let mut terminal = Terminal::new(TestBackend::new(24, 16)).unwrap();
        terminal
            .draw(|frame| {
                draw_canvas_grid(
                    frame,
                    Rect::new(0, 0, 21, 11),
                    CanvasGridState::default(),
                    projection(0.1, 0.1, 100.0, 50.0),
                    Color::DarkGray,
                    1.0,
                );
                frame
                    .buffer_mut()
                    .cell_mut((0, 0))
                    .unwrap()
                    .set_char('x')
                    .set_fg(Color::Red);
            })
            .unwrap();

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "x");
        assert_eq!(cell.style().fg, Some(Color::Red));
    }

    #[test]
    fn adaptive_doubling_at_low_zoom() {
        let area = Rect::new(0, 0, 21, 11);
        let zoom = 0.01;
        // At zoom = 0.01:
        // cols_per_world_x = 0.025, rows_per_world_y = 0.0125
        // compensation = 0.025 / (2 * 0.0125) = 1.0
        // Initial step_y = 100.0 * 1.0 = 100.0. step_y * zoom = 1.0 < 6.0
        // Doubling happens until step_y * zoom >= 6.0.
        // 100 -> 200 (2.0) -> 400 (4.0) -> 800 (8.0)
        // grid_step_x = 800.0
        // grid_step_y = 800.0
        // Set cols/rows per world to make dots appear at 0, 20
        // cols_per_world_x * 800.0 = 20 => 0.025
        // rows_per_world_y * 800.0 = 10 => 0.0125
        let buffer = render_grid(
            area,
            CanvasGridState::default(),
            projection(0.025, 0.0125, 800.0, 800.0),
            zoom,
        );
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "·");
        assert_eq!(buffer.cell((20, 10)).unwrap().symbol(), "·");
        assert_eq!(buffer.cell((10, 5)).unwrap().symbol(), " ");
    }
}
