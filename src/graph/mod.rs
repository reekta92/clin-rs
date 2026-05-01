pub mod input;
pub mod physics;
pub mod render;
pub mod viewport;

use std::collections::HashMap;

use anyhow::Result;
use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::storage::Storage;

pub struct GraphNodeData {
    pub note_id: String,
    pub title: String,
    pub is_encrypted: bool,
    pub tags: Vec<String>,
}

pub struct GraphState {
    pub simulation: Simulation<GraphNodeData, ()>,
    pub viewport: viewport::Viewport,
    pub selected_node: Option<NodeIndex>,
    pub dragging_node: Option<NodeIndex>,
    pub is_settled: bool,
}

static WIKILINK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap());

fn extract_wikilinks(content: &str) -> Vec<String> {
    WIKILINK_RE
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .collect()
}

pub fn build_graph(storage: &Storage) -> Result<ForceGraph<GraphNodeData, ()>> {
    let note_ids = storage.list_note_ids()?;
    let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();
    let mut title_to_index: HashMap<String, NodeIndex> = HashMap::new();

    for id in &note_ids {
        let summary = match storage.load_note_summary(id) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let is_encrypted = id.ends_with(".clin");
        let data = GraphNodeData {
            note_id: id.clone(),
            title: summary.title.clone(),
            is_encrypted,
            tags: summary.tags.clone(),
        };

        let idx = graph.add_force_node(&summary.title, data);
        title_to_index.insert(summary.title.to_lowercase(), idx);
    }

    for id in &note_ids {
        if id.ends_with(".clin") {
            continue;
        }

        let note = match storage.load_note(id) {
            Ok(n) => n,
            Err(_) => continue,
        };

        let source_title_lower = note.title.to_lowercase();
        let source_idx = match title_to_index.get(&source_title_lower) {
            Some(&idx) => idx,
            None => continue,
        };

        let links = extract_wikilinks(&note.content);
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

    Ok(graph)
}

pub fn create_simulation(graph: ForceGraph<GraphNodeData, ()>) -> Simulation<GraphNodeData, ()> {
    let force = fdg_sim::force::handy(80.0, 0.95, true, true);
    let params = SimulationParameters::new(800.0, fdg_sim::Dimensions::Two, force);
    Simulation::from_graph(graph, params)
}
