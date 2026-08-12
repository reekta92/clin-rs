use ratatui::layout::Rect;

use fdg_sim::petgraph::graph::NodeIndex;

use super::graph::GraphState;
use crate::config::{ClinConfig, NodeSizeMode};

pub const CELL_ASPECT: f64 = 0.5;
/// Lowest zoom-out permitted, expressed as a fraction of `auto_fit_zoom`
/// (i.e. `scale() >= MIN_SCALE`). Bounds screen_to_world so node-drag never
/// writes coordinates large enough to destabilise the force simulation.
const MIN_SCALE: f64 = 0.15;
/// Max |world coordinate| returned by screen_to_world. Chosen far above any
/// real graph span (auto-fit produces coords in the thousands) yet small enough
/// that `x as f32` stays finite and force arithmetic never overflows f32.
const WORLD_COORD_LIMIT: f64 = 1.0e18;

/// Visual-row slop added around a node's drawn body, and the minimum click
/// radius in screen rows so sub-pixel (zoomed-out) nodes stay clickable.
const HIT_SLOP_ROWS: f64 = 1.5;
const HIT_MIN_ROWS: f64 = 2.0;

fn clamp_world(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(-WORLD_COORD_LIMIT, WORLD_COORD_LIMIT)
}

/// World-space radius of a node, identical to the radius used for drawing.
/// Single source of truth for `fill_nodes`, the looking-glass preview, and
/// `Viewport::hit_test`.
pub fn node_world_radius(config: &ClinConfig, max_link_count: usize, link_count: usize) -> f64 {
    match config.graf.visual.node_size_mode {
        NodeSizeMode::Fixed => config.graf.visual.node_size,
        NodeSizeMode::LinkCount => {
            if max_link_count == 0 {
                config.graf.visual.node_size
            } else {
                config.graf.visual.node_size
                    * (1.0 + (link_count as f64 / max_link_count as f64) * 1.5)
            }
        }
    }
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

    pub fn world_to_screen(&self, wx: f64, wy: f64, area: Rect) -> (f64, f64) {
        let aspect = area.width as f64 / area.height as f64;
        let [x_left, x_right] = self.x_bounds(aspect);
        let [y_bottom, y_top] = self.y_bounds(aspect);

        let col = area.x as f64 + ((wx - x_left) / (x_right - x_left)) * area.width as f64;
        let row = area.y as f64 + ((y_top - wy) / (y_top - y_bottom)) * area.height as f64;
        (col, row)
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

    pub fn hit_test(
        &self,
        world_x: f64,
        world_y: f64,
        state: &GraphState,
        config: &ClinConfig,
        area: Rect,
        max_link_count: usize,
    ) -> Option<NodeIndex> {
        if !world_x.is_finite() || !world_y.is_finite() {
            return None;
        }
        let h = (area.height as f64).max(1.0);
        let world_per_row = (100.0 / self.zoom) / h;
        let pad_world = HIT_SLOP_ROWS * world_per_row;
        let min_hit_world = HIT_MIN_ROWS * world_per_row;

        let max_node_radius_world = config.graf.visual.node_size * 2.5;
        let query = max_node_radius_world + pad_world + min_hit_world;

        let graph = state.simulation.get_graph();
        let mut contained: Option<(NodeIndex, f64)> = None;
        let mut near: Option<(NodeIndex, f64)> = None;

        state
            .spatial_grid
            .for_each_near(world_x, world_y, query, |idx| {
                let node = &graph[idx];
                let dx = node.location.x as f64 - world_x;
                let dy = node.location.y as f64 - world_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if !dist.is_finite() {
                    return;
                }
                let nr = node_world_radius(config, max_link_count, node.data.link_count);
                let click_thresh = (nr + pad_world).max(min_hit_world);
                if dist <= nr {
                    match contained {
                        Some((bi, bd)) if dist >= bd && !(dist == bd && idx.index() < bi.index()) => {}
                        _ => contained = Some((idx, dist)),
                    }
                }
                if dist <= click_thresh {
                    match near {
                        Some((bi, bd)) if dist >= bd && !(dist == bd && idx.index() < bi.index()) => {}
                        _ => near = Some((idx, dist)),
                    }
                }
            });

        contained.or(near).map(|(idx, _)| idx)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graf::graph::GraphNodeData;
    use crate::graf::spatial::SpatialGrid;
    use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
    use parking_lot::Mutex;

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

        let positions = [
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
            selected_nodes: std::collections::HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: true,
            alpha: 0.0,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: SpatialGrid::new(100.0),
            physics_worker_active: false,
            physics_ideal_distance: 80.0,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
        };

        for (i, &idx) in idxs.iter().enumerate() {
            let (x, y) = positions[i];
            let node = gs.simulation.get_graph_mut().node_weight_mut(idx).unwrap();
            node.location.x = x as f32;
            node.location.y = y as f32;
        }

        gs.spatial_grid.rebuild(gs.simulation.get_graph());

        let vp = Viewport {
            zoom: 1.0,
            ..Default::default()
        };

        let config = ClinConfig::default();
        let area = Rect::new(0, 0, 80, 40);
        let max_lc = 0;

        let hit = vp.hit_test(10001.0, 10001.0, &gs, &config, area, max_lc).unwrap();
        assert_eq!(hit, idxs[1]);

        let hit_equal = vp.hit_test(10002.5, 10002.5, &gs, &config, area, max_lc).unwrap();
        assert_eq!(hit_equal, idxs[1]);
    }

    fn make_state(nodes: &[(f64, f64, usize)]) -> (GraphState, Vec<NodeIndex>) {
        let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
        let mut idxs = Vec::new();
        for (i, &(x, y, lc)) in nodes.iter().enumerate() {
            let data = GraphNodeData {
                note_id: format!("{i}"),
                title: format!("Node {i}"),
                tags: vec![],
                link_count: lc,
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
            selected_nodes: std::collections::HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: true,
            alpha: 0.0,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: SpatialGrid::new(100.0),
            physics_worker_active: false,
            physics_ideal_distance: 80.0,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
        };
        // `Simulation::from_graph` re-initialises node locations; re-apply the
        // explicit positions so the spatial grid reflects them.
        for (i, &idx) in idxs.iter().enumerate() {
            let (x, y, _) = nodes[i];
            let node = gs.simulation.get_graph_mut().node_weight_mut(idx).unwrap();
            node.location.x = x as f32;
            node.location.y = y as f32;
        }
        gs.spatial_grid.rebuild(gs.simulation.get_graph());
        (gs, idxs)
    }

    #[test]
    fn test_hit_test_containment_prefers_body_over_neighbor() {
        // A = large node (LinkCount max), B = small node; cursor sits inside A's
        // body but outside B's body, yet B's center is nearer. Containment-first
        // must return A; nearest-center (old behavior) would return B.
        let mut config = ClinConfig::default();
        config.graf.visual.node_size_mode = NodeSizeMode::LinkCount;
        // node_size 2.0: A (lc=4, max=4) → r=5.0 ; B (lc=0) → r=2.0.
        let (gs, idxs) = make_state(&[(0.0, 0.0, 4), (3.0, -2.5, 0)]);
        let vp = Viewport {
            zoom: 1.0,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 40);
        let max_lc = 4;

        assert_eq!(node_world_radius(&config, 4, 4), 5.0);
        assert_eq!(node_world_radius(&config, 4, 0), 2.0);

        // Cursor (3,0): dist to A = 3 <= 5 (contained); dist to B = 2.5 > 2 (not).
        let hit = vp.hit_test(3.0, 0.0, &gs, &config, area, max_lc).unwrap();
        assert_eq!(hit, idxs[0]);
    }

    #[test]
    fn test_hit_test_zoom_extremes() {
        let (gs, idxs) = make_state(&[
            (0.0, 0.0, 0),
            (10000.0, 10000.0, 0),
            (10005.0, 10005.0, 0),
        ]);
        let config = ClinConfig::default();
        let area = Rect::new(0, 0, 80, 40);
        let max_lc = 0;

        let vp_zoom_out = Viewport {
            zoom: 0.15,
            ..Default::default()
        };
        let vp_zoom_in = Viewport {
            zoom: 200.0,
            ..Default::default()
        };

        // Cursor on idx1's body (radius 2.0): (10001,10001) ~1.41 away.
        assert_eq!(
            vp_zoom_out.hit_test(10001.0, 10001.0, &gs, &config, area, max_lc),
            Some(idxs[1])
        );
        assert_eq!(
            vp_zoom_in.hit_test(10001.0, 10001.0, &gs, &config, area, max_lc),
            Some(idxs[1])
        );

        // Cursor far from every node → None at both extremes.
        assert_eq!(
            vp_zoom_out.hit_test(50000.0, 50000.0, &gs, &config, area, max_lc),
            None
        );
        assert_eq!(
            vp_zoom_in.hit_test(50000.0, 50000.0, &gs, &config, area, max_lc),
            None
        );
    }
}
