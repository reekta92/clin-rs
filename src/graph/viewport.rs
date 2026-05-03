use ratatui::layout::Rect;

use fdg_sim::petgraph::graph::NodeIndex;

use super::GraphState;

pub const CELL_ASPECT: f64 = 0.5;

#[derive(Clone)]
pub struct Viewport {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub max_zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
            max_zoom: 100.0,
        }
    }
}

impl Viewport {
    // BUG-11: aspect parameter ignored. Dot markers need aspect correction but
    // braille/halfblock (99% of users) work correctly with fixed CELL_ASPECT.
    pub fn x_bounds(&self, _aspect: f64) -> [f64; 2] {
        let half_w = 100.0 / self.zoom;
        [self.center_x - half_w, self.center_x + half_w]
    }

    pub fn y_bounds(&self, _aspect: f64) -> [f64; 2] {
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

    pub fn auto_fit_from_graph(
        &self,
        graph: &fdg_sim::ForceGraph<super::GraphNodeData, ()>,
        auto_fit_padding: f64,
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
        let range = range_x.max(range_y) * auto_fit_padding;
        let full_zoom = 200.0 / range;
        vp.zoom = full_zoom;
        vp.max_zoom = full_zoom * (100.0_f64 / 0.5_f64).sqrt();
        vp
    }

    pub fn zoom_in(&mut self, factor: f64) {
        self.zoom *= factor;
        if self.zoom > self.max_zoom {
            self.zoom = self.max_zoom;
        }
    }

    pub fn zoom_out(&mut self, factor: f64) {
        self.zoom /= factor;
        if self.zoom < 0.01 {
            self.zoom = 0.01;
        }
    }

    pub fn center_on_node(&mut self, x: f32, y: f32) {
        self.center_x = x as f64;
        self.center_y = y as f64;
    }

    pub fn nearest_to_center(&self, state: &GraphState) -> Option<NodeIndex> {
        let graph = state.simulation.get_graph();
        let mut best: Option<(NodeIndex, f64)> = None;
        for idx in graph.node_indices() {
            let node = &graph[idx];
            let dx = node.location.x as f64 - self.center_x;
            let dy = node.location.y as f64 - self.center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            match best {
                Some((_, bd)) if dist >= bd => {}
                _ => best = Some((idx, dist)),
            }
        }
        best.map(|(idx, _)| idx)
    }

    pub fn nearest_in_direction(
        &self,
        state: &GraphState,
        origin_x: f64,
        origin_y: f64,
        dir_x: f64,
        dir_y: f64,
        exclude: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        let graph = state.simulation.get_graph();
        let dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if dir_len == 0.0 {
            return None;
        }
        let ndx = dir_x / dir_len;
        let ndy = dir_y / dir_len;

        const ANGLE_THRESHOLD: f64 = std::f64::consts::FRAC_PI_3;
        const ANGLE_WEIGHT: f64 = 80.0;

        let mut best: Option<(NodeIndex, f64)> = None;
        for idx in graph.node_indices() {
            if exclude == Some(idx) {
                continue;
            }
            let node = &graph[idx];
            let dx = node.location.x as f64 - origin_x;
            let dy = node.location.y as f64 - origin_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 1e-6 {
                continue;
            }
            let dot = (dx * ndx + dy * ndy) / dist;
            if dot < 0.0 {
                continue;
            }
            let angle = dot.acos();
            if angle > ANGLE_THRESHOLD {
                continue;
            }
            let score = ANGLE_WEIGHT * angle + dist;
            match best {
                Some((_, bs)) if score >= bs => {}
                _ => best = Some((idx, score)),
            }
        }
        best.map(|(idx, _)| idx)
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
