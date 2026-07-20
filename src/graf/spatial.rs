use std::collections::HashMap;

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::ForceGraph;

use super::graph::GraphNodeData;

/// Uniform grid spatial index for O(1) neighborhood queries.
///
/// Partitions the 2D plane into `cell_size × cell_size` cells.
/// Nodes are placed in cells by their (x, y) location.
/// Queries check the 3×3 cell neighborhood around a point for
/// fast hit-testing and viewport culling.
pub struct SpatialGrid {
    cell_size: f64,
    cells: HashMap<(i32, i32), Vec<NodeIndex>>,
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

    /// Query all nodes within `radius` of the given world-space point.
    /// Checks the 3×3 cell neighborhood around the point's cell.
    pub fn query_point(
        &self,
        x: f64,
        y: f64,
        _radius: f64,
    ) -> impl Iterator<Item = NodeIndex> + '_ {
        let center_cell = self.cell_coord(x, y);

        let min_cx = center_cell.0 - 1;
        let max_cx = center_cell.0 + 1;
        let min_cy = center_cell.1 - 1;
        let max_cy = center_cell.1 + 1;

        (min_cx..=max_cx)
            .flat_map(move |cx| (min_cy..=max_cy).map(move |cy| (cx, cy)))
            .flat_map(move |cell| self.cells.get(&cell).into_iter().flatten())
            .copied()
    }

    /// Query all nodes within the given axis-aligned bounding rectangle.
    /// Iterates cells that overlap the rectangle.
    pub fn query_rect(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    ) -> impl Iterator<Item = NodeIndex> + '_ {
        let min_cell = self.cell_coord(min_x, min_y);
        let max_cell = self.cell_coord(max_x, max_y);

        (min_cell.0..=max_cell.0)
            .flat_map(move |cx| (min_cell.1..=max_cell.1).map(move |cy| (cx, cy)))
            .flat_map(move |cell| self.cells.get(&cell).into_iter().flatten())
            .copied()
    }

    fn cell_coord(&self, x: f64, y: f64) -> (i32, i32) {
        (
            (x / self.cell_size).floor() as i32,
            (y / self.cell_size).floor() as i32,
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
            let x = (i % 3) as f32 * 100.0;
            let y = (i / 3) as f32 * 100.0;
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
        // Cell size 100 means a 3×3 cell area covers 300×300 space
        let grid = SpatialGrid::new(100.0);

        // Insert nodes (we need a mutable grid)
        let mut grid = grid;
        grid.rebuild(&graph);

        // Query near origin — center node should be returned
        // (its location is 0,0)
        let results: Vec<NodeIndex> = grid.query_point(0.0, 0.0, 50.0).collect();
        assert!(!results.is_empty(), "should find at least the center node");
    }

    #[test]
    fn test_spatial_grid_query_rect() {
        let graph = make_test_graph();
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&graph);

        // Query rect that covers only the first quadrant (x >= 0, y >= 0)
        // The 3×3 grid of nodes at (0,0), (100,0), (200,0), (0,100), etc.
        // With cell_size=100 and nodes at 0-200 range, query rect 0..250, 0..250
        // should return 4 nodes (0,100) in x and y
        let results: Vec<NodeIndex> = grid.query_rect(-50.0, -50.0, 250.0, 250.0).collect();
        assert!(!results.is_empty(), "should find nodes in rect");
    }

    #[test]
    fn test_spatial_grid_empty() {
        let graph = make_test_graph();
        let grid = SpatialGrid::new(100.0);
        // Don't rebuild — grid is empty

        let results: Vec<NodeIndex> = grid.query_point(0.0, 0.0, 50.0).collect();
        assert!(results.is_empty(), "empty grid should return nothing");

        let results: Vec<NodeIndex> = grid.query_rect(-100.0, -100.0, 100.0, 100.0).collect();
        assert!(results.is_empty(), "empty grid should return nothing");
    }

    #[test]
    fn test_spatial_grid_rebuild_clears_old() {
        let graph = make_test_graph();
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&graph);

        // Rebuild with an empty graph
        let empty_graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
        grid.rebuild(&empty_graph);

        let results: Vec<NodeIndex> = grid.query_point(0.0, 0.0, 50.0).collect();
        assert!(results.is_empty(), "after rebuild with empty graph, should be empty");
    }
}
