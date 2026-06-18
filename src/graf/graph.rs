use std::collections::HashMap;
use std::sync::Mutex;

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};

use crate::config::ClinConfig;
use crate::storage::Storage;

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
    pub graph_bounds: (f64, f64, f64, f64),
    pub render_cache: Mutex<super::render::RenderCache>,
}

pub fn build_graph(
    storage: &Storage,
    config: &ClinConfig,
) -> anyhow::Result<ForceGraph<GraphNodeData, ()>> {
    let note_ids = storage.list_note_ids(!config.list.show_hidden_files)?;
    let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
    let mut title_to_index: HashMap<String, NodeIndex> = HashMap::new();

    let mut summaries = Vec::new();
    let mut links_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut link_counts: HashMap<String, usize> = HashMap::new();

    for id in &note_ids {
        if let Ok(summary) = storage.load_note_summary(id) {
            link_counts.insert(id.clone(), summary.links.len());
            links_map.insert(id.clone(), summary.links.clone());
            summaries.push(summary);
        }
    }

    let min_links = config.graf.filter.min_links;

    for summary in &summaries {
        let lc = link_counts.get(&summary.id).copied().unwrap_or(0);
        if lc < min_links {
            continue;
        }

        if !config.graf.filter.exclude_tags.is_empty()
            && summary
                .tags
                .iter()
                .any(|t| config.graf.filter.exclude_tags.contains(t))
        {
            continue;
        }

        let data = GraphNodeData {
            note_id: summary.id.clone(),
            title: summary.title.clone(),
            tags: summary.tags.clone(),
            link_count: lc,
            folder: summary.folder.clone(),
        };

        let idx = graph.add_force_node(&summary.title, data);
        title_to_index.insert(summary.title.to_lowercase(), idx);
    }

    for summary in &summaries {
        if let Some(links) = links_map.get(&summary.id) {
            let source_title = summary.title.to_lowercase();

            let source_idx = match title_to_index.get(&source_title) {
                Some(&idx) => idx,
                None => continue,
            };

            let mut seen_targets = std::collections::HashSet::new();
            for link in links {
                let target_lower = link.to_lowercase();
                if let Some(&target_idx) = title_to_index.get(&target_lower)
                    && target_idx != source_idx
                    && seen_targets.insert(target_idx)
                    && graph.edges_connecting(source_idx, target_idx).count() == 0
                {
                    graph.add_edge(source_idx, target_idx, ());
                }
            }
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
    pub fn new(storage: &Storage, config: &ClinConfig) -> anyhow::Result<Self> {
        let graph = build_graph(storage, config)?;
        let simulation = create_simulation(graph, config);
        let mut state = Self {
            viewport: super::viewport::Viewport::default(),
            simulation,
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(super::render::RenderCache::new()),
        };
        state.viewport = state
            .viewport
            .auto_fit_from_graph(state.simulation.get_graph(), 1.4);
        state.graph_bounds = super::render::compute_graph_bounds(state.simulation.get_graph());
        Ok(state)
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
