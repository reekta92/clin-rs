use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};

use crate::config::ClinConfig;
use crate::storage::NoteSummary;

pub struct GraphNodeData {
    pub note_id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub link_count: usize,
    pub folder: String,
}

pub struct GraphState {
    pub simulation: Simulation<GraphNodeData, ()>,
    pub viewport: super::viewport::Viewport,
    pub selected_node: Option<NodeIndex>,
    pub dragging_node: Option<NodeIndex>,
    pub drag_target: Option<(f32, f32)>,
    pub is_settled: bool,
    pub alpha: f32,
    pub graph_bounds: (f64, f64, f64, f64),
    pub render_cache: Mutex<super::render::RenderCache>,
    pub mouse_pos: Option<(u16, u16)>,
    pub spatial_grid: super::spatial::SpatialGrid,
    pub physics_worker_active: bool,
}

pub fn build_graph(
    summaries: &[NoteSummary],
    config: &ClinConfig,
) -> anyhow::Result<ForceGraph<GraphNodeData, ()>> {
    let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
    let mut title_to_index: HashMap<String, NodeIndex> = HashMap::new();

    let show_orphan = config.graf.filter.show_orphan;

    // 1. Filter out notes excluded by tags
    let mut valid_summaries: Vec<&NoteSummary> = Vec::new();
    for summary in summaries {
        if !config.graf.filter.exclude_tags.is_empty()
            && summary
                .tags
                .iter()
                .any(|t| config.graf.filter.exclude_tags.contains(t))
        {
            continue;
        }
        valid_summaries.push(summary);
    }

    // 2. Map valid titles for edge validation
    let valid_titles: HashSet<String> = valid_summaries
        .iter()
        .map(|s| s.title.to_lowercase())
        .collect();

    // 3. Find titles that participate in at least one valid edge
    let mut has_valid_edge = HashSet::new();
    if !show_orphan {
        for summary in &valid_summaries {
            let source_title = summary.title.to_lowercase();
            for link in &summary.links {
                let target_title = link.to_lowercase();
                if target_title != source_title && valid_titles.contains(&target_title) {
                    has_valid_edge.insert(source_title.clone());
                    has_valid_edge.insert(target_title);
                }
            }
        }
    }

    // 4. Collect final candidates (excluding orphans if requested)
    let mut candidates: Vec<&NoteSummary> = Vec::new();
    for summary in valid_summaries {
        if !show_orphan && !has_valid_edge.contains(&summary.title.to_lowercase()) {
            continue;
        }
        candidates.push(summary);
    }

    // Apply max_node cap: keep most-connected nodes
    let max_node = config.graf.max_node;
    if max_node > 0 && candidates.len() > max_node {
        candidates.sort_by_key(|b| std::cmp::Reverse(b.links.len()));
        candidates.truncate(max_node);
    }

    // Insert into force graph
    for summary in &candidates {
        let data = GraphNodeData {
            note_id: summary.id.clone(),
            title: summary.title.clone(),
            tags: summary.tags.clone(),
            link_count: summary.links.len(),
            folder: summary.folder.clone(),
        };

        let idx = graph.add_force_node(&summary.title, data);
        title_to_index.insert(summary.title.to_lowercase(), idx);
    }

    let mut has_final_edge = std::collections::HashSet::new();

    for summary in summaries {
        let source_title = summary.title.to_lowercase();

        let source_idx = match title_to_index.get(&source_title) {
            Some(&idx) => idx,
            None => continue,
        };

        let mut seen_targets = std::collections::HashSet::new();
        for link in &summary.links {
            let target_lower = link.to_lowercase();
            if let Some(&target_idx) = title_to_index.get(&target_lower)
                && target_idx != source_idx
                && seen_targets.insert(target_idx)
                && graph.edges_connecting(source_idx, target_idx).count() == 0
            {
                graph.add_edge(source_idx, target_idx, ());
                has_final_edge.insert(source_idx);
                has_final_edge.insert(target_idx);
            }
        }
    }

    if !show_orphan {
        let mut to_remove = Vec::new();
        for idx in graph.node_indices() {
            if !has_final_edge.contains(&idx) {
                to_remove.push(idx);
            }
        }
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            graph.remove_node(idx);
        }
    }

    Ok(graph)
}

pub fn create_simulation(
    graph: ForceGraph<GraphNodeData, ()>,
    config: &ClinConfig,
) -> Simulation<GraphNodeData, ()> {
    let force = fdg_sim::force::handy(config.graf.physics.ideal_distance as f32, 0.95, true, true);
    let params = SimulationParameters::new(800.0, fdg_sim::Dimensions::Two, force);
    Simulation::from_graph(graph, params)
}
impl GraphState {
    pub fn new(summaries: &[NoteSummary], config: &ClinConfig) -> anyhow::Result<Self> {
        let graph = build_graph(summaries, config)?;
        let simulation = create_simulation(graph, config);
        let mut state = Self {
            viewport: super::viewport::Viewport::default(),
            simulation,
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(super::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: super::spatial::SpatialGrid::new(config.graf.physics.ideal_distance),
            physics_worker_active: false,
        };
        state.viewport = state
            .viewport
            .auto_fit_from_graph(state.simulation.get_graph(), 1.4);
        state.graph_bounds = super::render::compute_graph_bounds(state.simulation.get_graph());
        // Rebuild spatial index after initial graph is placed
        state.spatial_grid.rebuild(state.simulation.get_graph());
        Ok(state)
    }
    pub fn reheat(&mut self, target: f32) {
        if self.physics_worker_active && target > self.alpha {
            self.alpha = target;
            self.is_settled = false;
        }
    }
}

pub fn search_nodes(
    sim: &fdg_sim::Simulation<GraphNodeData, ()>,
    query: &str,
    max_results: usize,
) -> Vec<(fdg_sim::petgraph::graph::NodeIndex, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    let q = query.to_lowercase();
    let graph = sim.get_graph();
    let mut results: Vec<(fdg_sim::petgraph::graph::NodeIndex, String)> = graph
        .node_indices()
        .filter_map(|idx| {
            let node = &graph[idx];
            let title_match = node.data.title.to_lowercase().contains(&q);
            let path_match = node.data.note_id.to_lowercase().contains(&q);
            let tag_match = node.data.tags.iter().any(|t| t.to_lowercase().contains(&q));
            if title_match || path_match || tag_match {
                Some((idx, node.data.title.clone()))
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| {
        let a_starts = a.1.to_lowercase().starts_with(&q);
        let b_starts = b.1.to_lowercase().starts_with(&q);
        match (a_starts, b_starts) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.cmp(&b.1),
        }
    });

    results.truncate(max_results);
    results
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_force_graph_max_node_truncation() {
        let mut config = ClinConfig::default();
        config.graf.max_node = 2;

        let summaries = vec![
            NoteSummary {
                id: "1".to_string(),
                title: "Note 1".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["Note 2".to_string(), "Note 3".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "2".to_string(),
                title: "Note 2".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["Note 1".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "3".to_string(),
                title: "Note 3".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
        ];

        let graph = build_graph(&summaries, &config).unwrap();
        assert_eq!(graph.node_count(), 2);
        // Note 1 (2 links) and Note 2 (1 link) should be kept, Note 3 (0 links) truncated
        let titles: Vec<_> = graph
            .node_weights()
            .map(|n| n.data.title.as_str())
            .collect();
        assert!(titles.contains(&"Note 1"));
        assert!(titles.contains(&"Note 2"));
        assert!(!titles.contains(&"Note 3"));
    }
    #[test]
    fn test_build_force_graph_show_orphan() {
        let summaries = vec![
            NoteSummary {
                id: "1".to_string(),
                title: "Connected 1".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["Connected 2".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "2".to_string(),
                title: "Connected 2".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "3".to_string(),
                title: "Orphan 1".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["Dead Link Target".to_string()],
                size_bytes: 0,
            },
        ];

        // Default: show_orphan = false
        let config_hide = ClinConfig::default();
        let graph_hide = build_graph(&summaries, &config_hide).unwrap();
        assert_eq!(graph_hide.node_count(), 2);
        let titles_hide: Vec<_> = graph_hide
            .node_weights()
            .map(|n| n.data.title.as_str())
            .collect();
        assert!(titles_hide.contains(&"Connected 1"));
        assert!(titles_hide.contains(&"Connected 2"));
        assert!(!titles_hide.contains(&"Orphan 1"));

        // show_orphan = true
        let mut config_show = ClinConfig::default();
        config_show.graf.filter.show_orphan = true;
        let graph_show = build_graph(&summaries, &config_show).unwrap();
        assert_eq!(graph_show.node_count(), 3);
        let titles_show: Vec<_> = graph_show
            .node_weights()
            .map(|n| n.data.title.as_str())
            .collect();
        assert!(titles_show.contains(&"Connected 1"));
        assert!(titles_show.contains(&"Connected 2"));
        assert!(titles_show.contains(&"Orphan 1"));
    }
    #[test]
    fn test_build_force_graph_self_link_orphan() {
        let summaries = vec![NoteSummary {
            id: "1".to_string(),
            title: "Self Linker".to_string(),
            updated_at: 0,
            folder: "".to_string(),
            tags: vec![],
            pinned: false,
            links: vec!["Self Linker".to_string()],
            size_bytes: 0,
        }];

        let config_hide = ClinConfig::default();
        let graph_hide = build_graph(&summaries, &config_hide).unwrap();
        assert_eq!(graph_hide.node_count(), 0);

        let mut config_show = ClinConfig::default();
        config_show.graf.filter.show_orphan = true;
        let graph_show = build_graph(&summaries, &config_show).unwrap();
        assert_eq!(graph_show.node_count(), 1);
    }

    #[test]
    fn test_build_force_graph_truncated_orphan_removal() {
        let summaries = vec![
            NoteSummary {
                id: "1".to_string(),
                title: "A".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["B".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "2".to_string(),
                title: "B".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["A".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "3".to_string(),
                title: "C".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["D".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "4".to_string(),
                title: "D".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
        ];

        let mut config = ClinConfig::default();
        config.graf.max_node = 3;
        let graph = build_graph(&summaries, &config).unwrap();
        // Pre-filter includes A, B, C, D.
        // Truncation (max_node = 3): A(1 link), B(1 link), C(1 link) kept; D(0 links) dropped.
        // Edge building: A<->B edge built. C->D edge cannot be built because D was truncated.
        // Post-filter scrubs C because it has no final edges.
        // Final graph only contains A and B.
        assert_eq!(graph.node_count(), 2);
        let titles: Vec<_> = graph
            .node_weights()
            .map(|n| n.data.title.as_str())
            .collect();
        assert!(titles.contains(&"A"));
        assert!(titles.contains(&"B"));
        assert!(!titles.contains(&"C"));
        assert!(!titles.contains(&"D"));
        assert_eq!(graph.edge_count(), 1);
    }
}
