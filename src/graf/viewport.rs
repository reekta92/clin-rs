use ratatui::layout::Rect;

use fdg_sim::petgraph::graph::NodeIndex;

use super::graph::GraphState;

pub const CELL_ASPECT: f64 = 0.5;
/// Lowest zoom-out permitted, expressed as a fraction of `auto_fit_zoom`
/// (i.e. `scale() >= MIN_SCALE`). Bounds screen_to_world so node-drag never
/// writes coordinates large enough to destabilise the force simulation.
const MIN_SCALE: f64 = 0.15;
/// Max |world coordinate| returned by screen_to_world. Chosen far above any
/// real graph span (auto-fit produces coords in the thousands) yet small enough
/// that `x as f32` stays finite and force arithmetic never overflows f32.
const WORLD_COORD_LIMIT: f64 = 1.0e18;

fn clamp_world(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(-WORLD_COORD_LIMIT, WORLD_COORD_LIMIT)
}

#[derive(Clone)]
pub struct Viewport {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub auto_fit_zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
            auto_fit_zoom: 1.0,
        }
    }
}

impl Viewport {
    pub fn x_bounds(&self, aspect: f64) -> [f64; 2] {
        let half_w = (100.0 * CELL_ASPECT * CELL_ASPECT * aspect) / self.zoom;
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

        let wx = x_left + ((col as f64 - area.x as f64) / area.width as f64) * (x_right - x_left);
        let wy = y_top - ((row as f64 - area.y as f64) / area.height as f64) * (y_top - y_bottom);
        (clamp_world(wx), clamp_world(wy))
    }

    #[must_use]
    pub fn auto_fit_from_graph(
        &self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
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
        vp.auto_fit_zoom = full_zoom;
        vp
    }

    pub fn scale(&self) -> f64 {
        self.zoom / self.auto_fit_zoom
    }

    pub fn zoom_in(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            let candidate = self.zoom * factor;
            if candidate.is_finite() && candidate > 0.0 && (100.0 / candidate).is_finite() {
                self.zoom = candidate;
            }
        }
    }

    pub fn zoom_out(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            let candidate = self.zoom / factor;
            if candidate.is_finite() && candidate > 0.0 && (100.0 / candidate).is_finite() {
                let min_zoom = MIN_SCALE * self.auto_fit_zoom;
                self.zoom = if min_zoom.is_finite() && min_zoom > 0.0 {
                    candidate.max(min_zoom)
                } else {
                    candidate
                };
            }
        }
    }

    pub fn center_on_node(&mut self, x: f32, y: f32) {
        let x_f64 = x as f64;
        let y_f64 = y as f64;
        if x_f64.is_finite() && y_f64.is_finite() {
            self.center_x = x_f64;
            self.center_y = y_f64;
        }
    }

    pub fn set_center(&mut self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.center_x = x;
            self.center_y = y;
        }
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        if dx.is_finite() && dy.is_finite() {
            let cx = self.center_x + dx;
            let cy = self.center_y + dy;
            if cx.is_finite() && cy.is_finite() {
                self.center_x = cx;
                self.center_y = cy;
            }
        }
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

    pub fn hit_test(&self, world_x: f64, world_y: f64, state: &GraphState) -> Option<NodeIndex> {
        let threshold = 8.0 / self.zoom;
        let mut best: Option<(NodeIndex, f64)> = None;
        let graph = state.simulation.get_graph();

        state
            .spatial_grid
            .for_each_near(world_x, world_y, threshold, |idx| {
                let node = &graph[idx];
                let dx = node.location.x as f64 - world_x;
                let dy = node.location.y as f64 - world_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < threshold {
                    match best {
                        Some((best_idx, best_dist)) => {
                            if dist < best_dist
                                || ((dist - best_dist).abs() < 1e-9
                                    && idx.index() < best_idx.index())
                            {
                                best = Some((idx, dist));
                            }
                        }
                        None => {
                            best = Some((idx, dist));
                        }
                    }
                }
            });

        best.map(|(idx, _)| idx)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graf::graph::GraphNodeData;
    use crate::graf::spatial::SpatialGrid;
    use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
    use parking_lot::Mutex;
    use parking_lot::RwLock;
    use std::sync::Arc;

    #[test]
    fn test_viewport_baseline_scale() {
        let vp = Viewport::default();
        assert_eq!(vp.scale(), 1.0);
    }

    #[test]
    fn test_viewport_zoom_beyond_old_caps() {
        let mut vp = Viewport::default();
        vp.zoom_in(200.0);
        assert_eq!(vp.zoom, 200.0);
        assert_eq!(vp.scale(), 200.0);

        vp.zoom = 1.0;
        vp.zoom_out(100.0);
        assert!((vp.zoom - 0.15).abs() < 1e-12);
        assert!((vp.scale() - 0.15).abs() < 1e-12);
    }

    #[test]
    fn test_viewport_invalid_candidate_rejection() {
        let mut vp = Viewport::default();

        vp.zoom_in(f64::NAN);
        assert_eq!(vp.zoom, 1.0);

        vp.zoom_in(f64::INFINITY);
        assert_eq!(vp.zoom, 1.0);

        vp.zoom_out(0.0);
        assert_eq!(vp.zoom, 1.0);

        vp.zoom_out(-2.0);
        assert_eq!(vp.zoom, 1.0);
    }

    #[test]
    fn test_viewport_center_and_pan_guards() {
        let mut vp = Viewport::default();

        vp.set_center(f64::NAN, 0.0);
        assert_eq!(vp.center_x, 0.0);

        vp.set_center(0.0, f64::INFINITY);
        assert_eq!(vp.center_y, 0.0);

        vp.pan_by(f64::NAN, 1.0);
        assert_eq!(vp.center_x, 0.0);

        vp.pan_by(1.0, f64::NEG_INFINITY);
        assert_eq!(vp.center_y, 0.0);
    }

    #[test]
    fn test_hit_test_parity_with_brute_force() {
        let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();

        let positions = vec![
            (0.0, 0.0),
            (10000.0, 10000.0),
            (10005.0, 10005.0),
            (-10000.0, -10000.0),
        ];

        let mut idxs = Vec::new();
        for (i, &(x, y)) in positions.iter().enumerate() {
            let data = GraphNodeData {
                note_id: format!("{i}"),
                title: format!("Node {i}"),
                tags: vec![],
                link_count: 0,
                folder: "".to_string(),
            };
            let idx = graph.add_force_node(format!("Node {i}"), data);
            graph.node_weight_mut(idx).unwrap().location.x = x as f32;
            graph.node_weight_mut(idx).unwrap().location.y = y as f32;
            idxs.push(idx);
        }

        let mut gs = GraphState {
            simulation: Simulation::from_graph(graph, SimulationParameters::default()),
            viewport: Viewport::default(),
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: true,
            alpha: 0.0,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: SpatialGrid::new(100.0),
            physics_worker_active: false,
        };

        for (i, &idx) in idxs.iter().enumerate() {
            let (x, y) = positions[i];
            let node = gs.simulation.get_graph_mut().node_weight_mut(idx).unwrap();
            node.location.x = x as f32;
            node.location.y = y as f32;
        }

        gs.spatial_grid.rebuild(gs.simulation.get_graph());

        let mut vp = Viewport::default();
        vp.zoom = 1.0;

        let hit = vp.hit_test(10001.0, 10001.0, &gs).unwrap();
        assert_eq!(hit, idxs[1]);

        let hit_equal = vp.hit_test(10002.5, 10002.5, &gs).unwrap();
        assert_eq!(hit_equal, idxs[1]);
    }
}
