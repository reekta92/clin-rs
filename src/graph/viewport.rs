use ratatui::layout::Rect;

use super::GraphState;

const PAN_AMOUNT: f64 = 5.0;
const ZOOM_FACTOR: f64 = 1.15;

#[derive(Clone)]
pub struct Viewport {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    pub fn x_bounds(&self, _aspect: f64) -> [f64; 2] {
        let half_w = 100.0 / self.zoom;
        [self.center_x - half_w, self.center_x + half_w]
    }

    pub fn y_bounds(&self, _aspect: f64) -> [f64; 2] {
        const CELL_ASPECT: f64 = 0.5;
        let half_h = 100.0 * CELL_ASPECT / self.zoom;
        [self.center_y - half_h, self.center_y + half_h]
    }

    pub fn screen_to_world(&self, col: u16, row: u16, area: Rect) -> (f64, f64) {
        let aspect = area.width as f64 / area.height as f64;
        let [x_left, x_right] = self.x_bounds(aspect);
        let [y_bottom, y_top] = self.y_bounds(aspect);

        let wx = x_left + (col as f64 / area.width as f64) * (x_right - x_left);
        let wy = y_top - (row as f64 / area.height as f64) * (y_top - y_bottom);
        (wx, wy)
    }

    pub fn auto_fit(&mut self, state: &GraphState) {
        let graph = state.simulation.get_graph();
        if graph.node_count() == 0 {
            *self = Viewport::default();
            return;
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for node in graph.node_weights() {
            let x = node.location.x as f64;
            let y = node.location.y as f64;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        self.center_x = (min_x + max_x) / 2.0;
        self.center_y = (min_y + max_y) / 2.0;

        let range_x = (max_x - min_x).max(1.0);
        let range_y = (max_y - min_y).max(1.0);
        let range = range_x.max(range_y) * 1.4;
        self.zoom = 200.0 / range;
    }

    pub fn auto_fit_from_graph(
        &self,
        graph: &fdg_sim::ForceGraph<super::GraphNodeData, ()>,
    ) -> Viewport {
        let mut vp = self.clone();
        if graph.node_count() == 0 {
            return Viewport::default();
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for node in graph.node_weights() {
            let x = node.location.x as f64;
            let y = node.location.y as f64;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        vp.center_x = (min_x + max_x) / 2.0;
        vp.center_y = (min_y + max_y) / 2.0;

        let range_x = (max_x - min_x).max(1.0);
        let range_y = (max_y - min_y).max(1.0);
        let range = range_x.max(range_y) * 1.4;
        vp.zoom = 200.0 / range;
        vp
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        let scale = 100.0 / self.zoom;
        self.center_x += dx * scale;
        self.center_y += dy * scale;
    }

    pub fn pan_up(&mut self) {
        self.pan(0.0, PAN_AMOUNT * 0.15);
    }

    pub fn pan_down(&mut self) {
        self.pan(0.0, -PAN_AMOUNT * 0.15);
    }

    pub fn pan_left(&mut self) {
        self.pan(-PAN_AMOUNT * 0.15, 0.0);
    }

    pub fn pan_right(&mut self) {
        self.pan(PAN_AMOUNT * 0.15, 0.0);
    }

    pub fn zoom_in(&mut self) {
        self.zoom *= ZOOM_FACTOR;
    }

    pub fn zoom_out(&mut self) {
        self.zoom /= ZOOM_FACTOR;
        if self.zoom < 0.01 {
            self.zoom = 0.01;
        }
    }

    pub fn hit_test(
        &self,
        world_x: f64,
        world_y: f64,
        state: &GraphState,
    ) -> Option<fdg_sim::petgraph::graph::NodeIndex> {
        let graph = state.simulation.get_graph();
        let threshold = 8.0 / self.zoom;
        let mut best: Option<(fdg_sim::petgraph::graph::NodeIndex, f64)> = None;

        for idx in graph.node_indices() {
            let node = &graph[idx];
            let dx = node.location.x as f64 - world_x;
            let dy = node.location.y as f64 - world_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < threshold {
                match best {
                    Some((_, best_dist)) if dist >= best_dist => {}
                    _ => best = Some((idx, dist)),
                }
            }
        }

        best.map(|(idx, _)| idx)
    }
}
