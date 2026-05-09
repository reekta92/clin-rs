pub mod input;
pub mod physics;
pub mod render;
pub mod viewport;

use std::collections::HashMap;
use std::sync::Mutex;

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::config::ClinConfig;
use crate::storage::Storage;

pub struct GraphNodeData {
    pub note_id: String,
    pub title: String,
    pub is_encrypted: bool,
    pub tags: Vec<String>,
    pub link_count: usize,
    pub folder: String,
}

pub struct GraphState {
    pub simulation: Simulation<GraphNodeData, ()>,
    pub viewport: viewport::Viewport,
    pub selected_node: Option<NodeIndex>,
    pub dragging_node: Option<NodeIndex>,
    pub drag_target: Option<(f32, f32)>,
    pub is_settled: bool,
    pub graph_bounds: (f64, f64, f64, f64),
    pub render_cache: Mutex<render::RenderCache>,
}

static WIKILINK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap());

fn extract_wikilinks(content: &str) -> Vec<String> {
    WIKILINK_RE
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .collect()
}

pub fn build_graph(storage: &Storage, config: &ClinConfig) -> anyhow::Result<ForceGraph<GraphNodeData, ()>> {
    let note_ids = storage.list_note_ids()?;
    let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
    let mut title_to_index: HashMap<String, NodeIndex> = HashMap::new();

    let mut links_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut link_counts: HashMap<String, usize> = HashMap::new();
    
    for id in &note_ids {
        if id.ends_with(".clin") { continue; }
        if let Ok(note) = storage.load_note(id) {
            let links = extract_wikilinks(&note.content);
            link_counts.insert(id.clone(), links.len());
            links_map.insert(id.clone(), links);
        }
    }

    let min_links = config.filter.min_links;
    
    for id in &note_ids {
        let lc = link_counts.get(id).copied().unwrap_or(0);
        if lc < min_links { continue; }
        
        let summary = match storage.load_note_summary(id) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if !config.filter.exclude_tags.is_empty() {
            if summary.tags.iter().any(|t| config.filter.exclude_tags.contains(t)) {
                continue;
            }
        }

        let is_encrypted = id.ends_with(".clin");
        let data = GraphNodeData {
            note_id: id.clone(),
            title: summary.title.clone(),
            is_encrypted,
            tags: summary.tags.clone(),
            link_count: lc,
            folder: summary.folder.clone(),
        };

        let idx = graph.add_force_node(&summary.title, data);
        title_to_index.insert(summary.title.to_lowercase(), idx);
    }

    for id in &note_ids {
        if let Some(links) = links_map.get(id) {
            let source_title = match storage.load_note_summary(id) {
                Ok(s) => s.title.to_lowercase(),
                Err(_) => continue,
            };
            
            let source_idx = match title_to_index.get(&source_title) {
                Some(&idx) => idx,
                None => continue,
            };

            let mut seen_targets = std::collections::HashSet::new();
            for link in links {
                let target_lower = link.to_lowercase();
                if let Some(&target_idx) = title_to_index.get(&target_lower) {
                    if target_idx != source_idx
                        && seen_targets.insert(target_idx)
                        && graph.edges_connecting(source_idx, target_idx).count() == 0
                    {
                        graph.add_edge(source_idx, target_idx, ());
                    }
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
    let force = fdg_sim::force::handy(
        config.physics.ideal_distance as f32,
        config.physics.damping,
        config.physics.cooling,
        config.physics.prevent_overlapping,
    );
    let params = SimulationParameters::new(
        config.physics.max_iterations as f32,
        fdg_sim::Dimensions::Two,
        force,
    );
    Simulation::from_graph(graph, params)
}

impl GraphState {
    pub fn new(storage: &Storage, config: &ClinConfig) -> anyhow::Result<Self> {
        let graph = build_graph(storage, config)?;
        let simulation = create_simulation(graph, config);
        let mut state = Self {
            viewport: viewport::Viewport::default(),
            simulation,
            selected_node: None,
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(render::RenderCache::new()),
        };
        state.viewport = state.viewport.auto_fit_from_graph(
            state.simulation.get_graph(),
            config.interaction.auto_fit_padding,
        );
        state.graph_bounds = render::compute_graph_bounds(state.simulation.get_graph());
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
