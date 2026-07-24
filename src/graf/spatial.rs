use std::collections::HashMap;

use fdg_sim::ForceGraph;
use fdg_sim::petgraph::graph::NodeIndex;

use super::graph::GraphNodeData;

/// Uniform grid spatial index for O(1) neighborhood queries.
///
/// Partitions the 2D plane into `cell_size × cell_size` cells.
/// Nodes are placed in cells by their (x, y) location.
/// Queries check the 3×3 cell neighborhood around a point for
/// fast hit-testing and viewport culling.
pub struct SpatialGrid {
    cell_size: f64,
    cells: HashMap<(i64, i64), Vec<NodeIndex>>,
}

impl SpatialGrid {
    /// Create a new grid with the given cell size.
    /// Cell size is floored to a minimum of 20.0 to prevent excessive
    /// cell counts when nodes cluster tightly.
    pub fn new(cell_size: f64) -> Self {
        Self {
            cell_size: cell_size.max(20.0),
            cells: HashMap::new(),
        }
    }

    /// Remove all entries from the grid.
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Rebuild the grid from all nodes in the force graph.
    /// O(n) — inserts every node into its cell.
    pub fn rebuild(&mut self, graph: &ForceGraph<GraphNodeData, ()>) {
        self.cells.clear();
        for idx in graph.node_indices() {
            let node = &graph[idx];
            let cell = self.cell_coord(node.location.x as f64, node.location.y as f64);
            self.cells.entry(cell).or_default().push(idx);
        }
    }

    pub fn for_each_in_rect(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        mut visit: impl FnMut(NodeIndex),
    ) {
        if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
            return;
        }

        let min_cell = self.cell_coord(min_x, min_y);
        let max_cell = self.cell_coord(max_x, max_y);

        let dx = (max_cell.0.saturating_sub(min_cell.0)).saturating_add(1);
        let dy = (max_cell.1.saturating_sub(min_cell.1)).saturating_add(1);
        let span_area = dx.saturating_mul(dy);

        let occupied_count = self.cells.len() as i64;

        if span_area > 0 && span_area < occupied_count {
            for cx in min_cell.0..=max_cell.0 {
                for cy in min_cell.1..=max_cell.1 {
                    if let Some(nodes) = self.cells.get(&(cx, cy)) {
                        for &idx in nodes {
                            visit(idx);
                        }
                    }
                }
            }
        } else {
            for (&(cx, cy), nodes) in &self.cells {
                if cx >= min_cell.0 && cx <= max_cell.0 && cy >= min_cell.1 && cy <= max_cell.1 {
                    for &idx in nodes {
                        visit(idx);
                    }
                }
            }
        }
    }

    pub fn for_each_near(&self, x: f64, y: f64, radius: f64, visit: impl FnMut(NodeIndex)) {
        if !x.is_finite() || !y.is_finite() || !radius.is_finite() || radius < 0.0 {
            return;
        }
        self.for_each_in_rect(x - radius, y - radius, x + radius, y + radius, visit);
    }

    fn cell_coord(&self, x: f64, y: f64) -> (i64, i64) {
        (
            (x / self.cell_size).floor() as i64,
            (y / self.cell_size).floor() as i64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graf::graph::GraphNodeData;
    use fdg_sim::{ForceGraph, ForceGraphHelper};

    fn make_test_graph() -> ForceGraph<GraphNodeData, ()> {
        let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();

        // Node at origin
        let data = GraphNodeData {
            note_id: "0".into(),
            title: "center".into(),
            tags: vec![],
            link_count: 0,
            folder: "".into(),
        };
        let _ = graph.add_force_node("center", data);

        // Nodes at various positions
        for i in 0..9 {
            let data = GraphNodeData {
                note_id: format!("{i}"),
                title: format!("node_{i}"),
                tags: vec![],
                link_count: 0,
                folder: "".into(),
            };
            let _ = graph.add_force_node(format!("node_{i}"), data);
        }

        graph
    }

    #[test]
    fn test_spatial_grid_query_point() {
        let graph = make_test_graph();
        let grid = SpatialGrid::new(100.0);
        let mut grid = grid;
        grid.rebuild(&graph);

        let mut results = Vec::new();
        grid.for_each_near(0.0, 0.0, 50.0, |idx| results.push(idx));
        assert!(!results.is_empty(), "should find at least the center node");
    }

    #[test]
    fn test_spatial_grid_query_rect() {
        let graph = make_test_graph();
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&graph);

        let mut results = Vec::new();
        grid.for_each_in_rect(-50.0, -50.0, 250.0, 250.0, |idx| results.push(idx));
        assert!(!results.is_empty(), "should find nodes in rect");
    }

    #[test]
    fn test_spatial_grid_empty() {
        let grid = SpatialGrid::new(100.0);

        let mut results = Vec::new();
        grid.for_each_near(0.0, 0.0, 50.0, |idx| results.push(idx));
        assert!(results.is_empty(), "empty grid should return nothing");

        let mut results = Vec::new();
        grid.for_each_in_rect(-100.0, -100.0, 100.0, 100.0, |idx| results.push(idx));
        assert!(results.is_empty(), "empty grid should return nothing");
    }

    #[test]
    fn test_spatial_grid_rebuild_clears_old() {
        let graph = make_test_graph();
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&graph);

        let empty_graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
        grid.rebuild(&empty_graph);

        let mut results = Vec::new();
        grid.for_each_near(0.0, 0.0, 50.0, |idx| results.push(idx));
        assert!(
            results.is_empty(),
            "after rebuild with empty graph, should be empty"
        );
    }
}
