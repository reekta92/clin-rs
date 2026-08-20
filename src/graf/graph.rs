use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};

use crate::config::ClinConfig;
use crate::storage::NoteSummary;
use crate::ui::CanvasSelection;

pub struct GraphNodeData {
    pub note_id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub link_count: usize,
    pub folder: String,
}

pub fn graf_menu_specs(
    extra_multi: bool,
    node_selected: bool,
) -> Vec<crate::ui::CanvasMenuItemSpec> {
    if extra_multi {
        vec![
            crate::ui::CanvasMenuItemSpec::new("Show Group").shortcut('g'),
            crate::ui::CanvasMenuItemSpec::new("Delete Node").shortcut('x'),
        ]
    } else if node_selected {
        vec![
            crate::ui::CanvasMenuItemSpec::new("Create Connection").shortcut('c'),
            crate::ui::CanvasMenuItemSpec::new("Delete Connection").shortcut('d'),
            crate::ui::CanvasMenuItemSpec::new("Local Graph").shortcut('l'),
            crate::ui::CanvasMenuItemSpec::new("Delete Node").shortcut('x'),
        ]
    } else {
        vec![]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrafMenuItem {
    CreateConnection,
    DeleteConnection,
    LocalGraph,
    ShowGroup,
    DeleteNode,
}

pub fn graf_menu_item_from_label(label: &str) -> Option<GrafMenuItem> {
    match label {
        "Create Connection" => Some(GrafMenuItem::CreateConnection),
        "Delete Connection" => Some(GrafMenuItem::DeleteConnection),
        "Local Graph" => Some(GrafMenuItem::LocalGraph),
        "Show Group" => Some(GrafMenuItem::ShowGroup),
        "Delete Node" => Some(GrafMenuItem::DeleteNode),
        _ => None,
    }
}
pub(crate) fn nodes_in_rect<'a>(
    graph: &'a ForceGraph<GraphNodeData, ()>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> impl Iterator<Item = NodeIndex> + 'a {
    graph.node_indices().filter(move |idx| {
        let l = graph[*idx].location;
        (l.x as f64) >= min_x
            && (l.x as f64) <= max_x
            && (l.y as f64) >= min_y
            && (l.y as f64) <= max_y
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeBanner {
    CreateConnection,
    DeleteConnection,
    BoxSelect,
    LocalGraph,
    GroupedGraph,
}

pub struct GraphState {
    pub simulation: Simulation<GraphNodeData, ()>,
    pub viewport: super::viewport::Viewport,
    pub selection: CanvasSelection<NodeIndex>,
    pub dragging_node: Option<NodeIndex>,
    pub drag_target: Option<(f32, f32)>,
    pub is_settled: bool,
    pub alpha: f32,
    pub graph_bounds: (f64, f64, f64, f64),
    pub render_cache: Mutex<super::render::RenderCache>,
    pub mouse_pos: Option<(u16, u16)>,
    pub physics_worker_active: bool,
    pub physics_ideal_distance: f64,
    pub context_menu: Option<crate::ui::CanvasContextMenu>,
    pub connection_source: Option<NodeIndex>,
    pub deleting_connection_source: Option<NodeIndex>,
    pub marquee: crate::ui::MarqueeDragState,
    pub right_down_pos: Option<(u16, u16)>,
    pub mode_banner: Option<ModeBanner>,
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
            link_count: 0, // filled in below from total degree
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

    // link_count = total degree (outgoing wikilinks + backlinks), not just the
    // note's outgoing links. Matches GraphState::apply_connection_change.
    let indices: Vec<NodeIndex> = graph.node_indices().collect();
    for idx in indices {
        let degree = graph.edges(idx).count();
        if let Some(n) = graph.node_weight_mut(idx) {
            n.data.link_count = degree;
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
#[derive(Debug, Clone)]
struct StaticNode {
    index: NodeIndex,
    degree: usize,
    key: String,
    relative_pos: (f64, f64),
}

#[derive(Debug, Clone)]
struct StaticComponent {
    nodes: Vec<StaticNode>,
    key: String,
    center: (f64, f64),
    disk_radius: f64,
    envelope_radius: f64,
}

fn collect_static_components(graph: &ForceGraph<GraphNodeData, ()>) -> Vec<StaticComponent> {
    let mut visited = HashSet::new();
    let mut components = Vec::new();

    let mut start_nodes: Vec<NodeIndex> = graph.node_indices().collect();
    start_nodes.sort_by(|&a, &b| {
        let node_a = &graph[a];
        let node_b = &graph[b];
        node_a
            .data
            .note_id
            .cmp(&node_b.data.note_id)
            .then_with(|| a.cmp(&b))
    });

    for &start_node in &start_nodes {
        if visited.contains(&start_node) {
            continue;
        }

        let mut component_nodes = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        visited.insert(start_node);
        queue.push_back(start_node);

        while let Some(curr) = queue.pop_front() {
            component_nodes.push(curr);

            let mut neighbors: Vec<NodeIndex> = graph.neighbors(curr).collect();
            neighbors.sort_by(|&a, &b| {
                let node_a = &graph[a];
                let node_b = &graph[b];
                node_a
                    .data
                    .note_id
                    .cmp(&node_b.data.note_id)
                    .then_with(|| a.cmp(&b))
            });

            for nbr in neighbors {
                if !visited.contains(&nbr) {
                    visited.insert(nbr);
                    queue.push_back(nbr);
                }
            }
        }

        // Sort component's nodes deterministically by note_id and index to identify first_node key
        component_nodes.sort_by(|&a, &b| {
            let node_a = &graph[a];
            let node_b = &graph[b];
            node_a
                .data
                .note_id
                .cmp(&node_b.data.note_id)
                .then_with(|| a.cmp(&b))
        });

        if !component_nodes.is_empty() {
            let first_node_idx = component_nodes[0];
            let key = graph[first_node_idx].data.note_id.clone();

            let mut static_nodes: Vec<StaticNode> = component_nodes
                .into_iter()
                .map(|idx| StaticNode {
                    index: idx,
                    degree: graph.neighbors(idx).count(),
                    key: graph[idx].data.note_id.clone(),
                    relative_pos: (0.0, 0.0),
                })
                .collect();

            static_nodes.sort_by(|a, b| {
                b.degree
                    .cmp(&a.degree)
                    .then_with(|| a.key.cmp(&b.key))
                    .then_with(|| a.index.cmp(&b.index))
            });

            components.push(StaticComponent {
                nodes: static_nodes,
                key,
                center: (0.0, 0.0),
                disk_radius: 0.0,
                envelope_radius: 0.0,
            });
        }
    }

    // Sort final components by (Reverse(nodes.len()), key)
    components.sort_by(|a, b| {
        let len_cmp = b.nodes.len().cmp(&a.nodes.len());
        if len_cmp != std::cmp::Ordering::Equal {
            len_cmp
        } else {
            a.key.cmp(&b.key)
        }
    });

    components
}

const STATIC_LAYOUT_SLOT_RESERVE: f64 = 1.15;
const MAX_STATIC_LAYOUT_ANGULAR_JITTER: f64 = 0.15;

fn stable_layout_hash(component_key: &str, node_key: &str, ring: usize, stream: u8) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let prime = 0x0100_0000_01b3_u64;

    let mut update_hash = |byte: u8| {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(prime);
    };

    for &b in component_key.as_bytes() {
        update_hash(b);
    }
    update_hash(0xff);
    for &b in node_key.as_bytes() {
        update_hash(b);
    }
    update_hash(0xff);
    for &b in &(ring as u64).to_le_bytes() {
        update_hash(b);
    }
    update_hash(stream);

    hash
}

fn stable_layout_unit(component_key: &str, node_key: &str, ring: usize, stream: u8) -> f64 {
    stable_layout_hash(component_key, node_key, ring, stream) as f64 / u64::MAX as f64
}

fn layout_static_components(
    components: &mut [StaticComponent],
    spacing: f64,
) -> Option<Vec<(NodeIndex, fdg_sim::glam::Vec3)>> {
    let spacing = if spacing.is_finite() && spacing > 0.0 {
        spacing
    } else {
        80.0
    };

    for c in components.iter_mut() {
        let n = c.nodes.len();
        if n == 0 {
            c.disk_radius = 0.0;
            c.envelope_radius = 0.0;
        } else if n == 1 {
            c.nodes[0].relative_pos = (0.0, 0.0);
            c.disk_radius = 0.0;
            c.envelope_radius = spacing;
        } else {
            // Place nodes[0] (highest degree) at center (0, 0)
            c.nodes[0].relative_pos = (0.0, 0.0);

            // Group remaining nodes by degree
            let mut groups = Vec::new();
            {
                let mut current_group = Vec::new();
                let mut current_degree = c.nodes[1].degree;
                current_group.push(1);
                for idx in 2..n {
                    if c.nodes[idx].degree == current_degree {
                        current_group.push(idx);
                    } else {
                        groups.push(current_group);
                        current_group = vec![idx];
                        current_degree = c.nodes[idx].degree;
                    }
                }
                if !current_group.is_empty() {
                    groups.push(current_group);
                }
            }

            // Lay out groups in concentric rings
            let mut r = 1;
            let mut last_used_ring_radius = 0.0;

            for group in groups {
                let mut group_remaining = group.len();
                let mut group_idx = 0;

                while group_remaining > 0 {
                    let ring_radius = spacing * r as f64;
                    last_used_ring_radius = ring_radius;

                    // Calculate slot capacity for this ring
                    let mut slot_capacity = 1;
                    let upper_bound =
                        (2.0 * std::f64::consts::PI * ring_radius / spacing).ceil() as usize + 2;
                    for sc in (1..=upper_bound).rev() {
                        if sc == 1 {
                            slot_capacity = 1;
                            break;
                        }
                        let sin_val = (std::f64::consts::PI / sc as f64).sin();
                        if 2.0 * ring_radius * sin_val >= spacing * STATIC_LAYOUT_SLOT_RESERVE {
                            slot_capacity = sc;
                            break;
                        }
                    }

                    let used_slots = std::cmp::min(group_remaining, slot_capacity);
                    let sector_angle = 2.0 * std::f64::consts::PI / used_slots as f64;
                    let minimum_angle = 2.0 * ((spacing / (2.0 * ring_radius)).min(1.0)).asin();
                    let available_slack = (sector_angle - minimum_angle).max(0.0);
                    let jitter_limit = if used_slots <= 1 {
                        0.0
                    } else {
                        MAX_STATIC_LAYOUT_ANGULAR_JITTER.min(available_slack * 0.45)
                    };
                    let ring_phase =
                        stable_layout_unit(&c.key, "", r, 0) * 2.0 * std::f64::consts::PI;

                    for slot in 0..used_slots {
                        let node_idx = group[group_idx + slot];
                        let node_key = &c.nodes[node_idx].key;
                        let signed_jitter =
                            (stable_layout_unit(&c.key, node_key, r, 1) * 2.0 - 1.0) * jitter_limit;
                        let angle = ring_phase + sector_angle * slot as f64 + signed_jitter;
                        let rx = ring_radius * angle.cos();
                        let ry = ring_radius * angle.sin();
                        c.nodes[node_idx].relative_pos = (rx, ry);
                    }

                    group_remaining -= used_slots;
                    group_idx += used_slots;
                    r += 1;
                }
            }

            c.disk_radius = last_used_ring_radius;
            c.envelope_radius = c.disk_radius + spacing;
        }
    }

    if components.is_empty() {
        return Some(Vec::new());
    }

    let gap = spacing * 4.0;
    components[0].center = (0.0, 0.0);
    let mut occupied_outer_radius = components[0].envelope_radius;

    let mut idx = 1;
    while idx < components.len() {
        let remaining_count = components.len() - idx;
        let next_envelope = components[idx].envelope_radius;
        let ring_radius = occupied_outer_radius + next_envelope + gap;
        let ring_max_envelope = next_envelope;

        let mut slot_count = 1;
        for sc in (2..=remaining_count).rev() {
            let sin_val = (std::f64::consts::PI / sc as f64).sin();
            if 2.0 * ring_radius * sin_val >= 2.0 * ring_max_envelope + gap {
                slot_count = sc;
                break;
            }
        }

        for slot in 0..slot_count {
            let c_idx = idx + slot;
            let angle = 2.0 * std::f64::consts::PI * (slot as f64) / (slot_count as f64);
            let cx = ring_radius * angle.cos();
            let cy = ring_radius * angle.sin();
            components[c_idx].center = (cx, cy);
        }

        occupied_outer_radius = ring_radius + ring_max_envelope;
        idx += slot_count;
    }

    let mut node_positions = Vec::new();
    for c in components.iter() {
        let (cx, cy) = c.center;
        if !cx.is_finite() || !cy.is_finite() {
            return None;
        }

        for node in &c.nodes {
            let nx = cx + node.relative_pos.0;
            let ny = cy + node.relative_pos.1;
            if !nx.is_finite() || !ny.is_finite() {
                return None;
            }
            let pos = fdg_sim::glam::Vec3::new(nx as f32, ny as f32, 0.0);
            node_positions.push((node.index, pos));
        }
    }

    Some(node_positions)
}
impl GraphState {
    pub fn new(summaries: &[NoteSummary], config: &ClinConfig) -> anyhow::Result<Self> {
        let graph = build_graph(summaries, config)?;
        let simulation = create_simulation(graph, config);
        let mut state = Self {
            viewport: super::viewport::Viewport::default(),
            simulation,
            selection: CanvasSelection::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(super::render::RenderCache::new()),
            mouse_pos: None,
            physics_worker_active: false,
            physics_ideal_distance: config.graf.physics.ideal_distance,
            context_menu: None,
            connection_source: None,
            deleting_connection_source: None,
            marquee: crate::ui::MarqueeDragState::new(3),
            right_down_pos: None,
            mode_banner: None,
        };
        state.viewport = state
            .viewport
            .auto_fit_from_graph(state.simulation.get_graph(), 1.4);
        state.graph_bounds = super::render::compute_graph_bounds(state.simulation.get_graph());
        Ok(state)
    }
    pub fn reheat(&mut self, target: f32) {
        if self.physics_worker_active && target > self.alpha {
            self.alpha = target;
            self.is_settled = false;
        }
    }

    pub fn open_context_menu(&mut self, screen_x: u16, screen_y: u16, _world: (f64, f64)) {
        let specs = graf_menu_specs(
            !self.selection.extra.is_empty(),
            self.selection.primary.is_some(),
        );
        if specs.is_empty() {
            return;
        }
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(screen_x, screen_y, specs));
    }

    pub fn close_menu(&mut self) {
        self.context_menu = None;
    }

    /// Apply a connection change to the live graph without rebuilding the
    /// simulation: adds/removes the edge, dirties the render cache, lightly
    /// reheats physics. Returns true if the graph topology changed.
    pub fn apply_connection_change(
        &mut self,
        src: NodeIndex,
        tgt: NodeIndex,
        create: bool,
    ) -> bool {
        let graph = self.simulation.get_graph_mut();
        let existing = graph.find_edge(src, tgt);
        if create {
            if existing.is_none() {
                graph.add_edge(src, tgt, ());
            } else {
                return false;
            }
        } else {
            match existing {
                Some(e) => {
                    graph.remove_edge(e);
                }
                None => return false,
            }
        }
        // node.link_count in GraphNodeData is stale; update for both endpoints.
        let src_count = graph.edges(src).count();
        let tgt_count = graph.edges(tgt).count();
        if let Some(n) = graph.node_weight_mut(src) {
            n.data.link_count = src_count;
        }
        if let Some(n) = graph.node_weight_mut(tgt) {
            n.data.link_count = tgt_count;
        }
        {
            let mut cache = self.render_cache.lock();
            cache.topology_dirty = true;
            cache.minimap_dirty = true;
        }
        self.is_settled = false;
        if self.physics_worker_active {
            self.reheat(0.3);
        }
        true
    }

    pub(crate) fn apply_static_cluster_layout(&mut self, ideal_distance: f64) -> bool {
        let graph = self.simulation.get_graph();
        let node_count = graph.node_count();
        if node_count == 0 {
            return false;
        }

        let mut components = collect_static_components(graph);
        let node_positions = match layout_static_components(&mut components, ideal_distance) {
            Some(pos) => pos,
            None => return false,
        };

        let graph_mut = self.simulation.get_graph_mut();
        for (idx, pos) in node_positions {
            if let Some(node) = graph_mut.node_weight_mut(idx) {
                node.location = pos;
                node.old_location = pos;
                node.velocity = fdg_sim::glam::Vec3::ZERO;
            }
        }

        // Recompute derived state
        self.viewport = self.viewport.auto_fit_from_graph(graph_mut, 1.4);
        self.graph_bounds = super::render::compute_graph_bounds(graph_mut);

        self.is_settled = true;
        self.alpha = 0.0;
        self.physics_worker_active = false;
        self.render_cache.lock().minimap_dirty = true;

        true
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
    #[test]
    fn test_context_menu_node_and_multinode_shapes() {
        let config = ClinConfig::default();
        let summaries = vec![NoteSummary {
            id: "1".to_string(),
            title: "A".to_string(),
            updated_at: 0,
            folder: "".to_string(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        }];
        let mut state = GraphState::new(&summaries, &config).unwrap();

        // Node menu: 4 items, LocalGraph present, ShowGroup absent
        state
            .selection
            .select_only(fdg_sim::petgraph::graph::NodeIndex::new(0));
        state.open_context_menu(0, 0, (0.0, 0.0));
        let menu = state.context_menu.as_ref().unwrap();
        assert_eq!(menu.items.len(), 4);
        let labels: Vec<&str> = menu.items.iter().map(|s| s.label).collect();
        assert!(labels.contains(&"Local Graph"));
        assert!(!labels.contains(&"Show Group"));
        assert!(labels.contains(&"Create Connection"));
        assert!(labels.contains(&"Delete Connection"));
        assert!(labels.contains(&"Delete Node"));

        // MultiNode menu: 2 items, ShowGroup present, LocalGraph absent
        state.context_menu = None;
        state
            .selection
            .add(fdg_sim::petgraph::graph::NodeIndex::new(0));
        state.open_context_menu(0, 0, (0.0, 0.0));
        let menu = state.context_menu.as_ref().unwrap();
        assert_eq!(menu.items.len(), 2);
        let labels: Vec<&str> = menu.items.iter().map(|s| s.label).collect();
        assert!(labels.contains(&"Show Group"));
        assert!(labels.contains(&"Delete Node"));
        assert!(!labels.contains(&"Local Graph"));

        // Background (no selection) → no menu
        state.context_menu = None;
        state.selection.clear();
        state.open_context_menu(0, 0, (0.0, 0.0));
        assert!(state.context_menu.is_none());
    }

    #[test]
    fn test_menu_item_shortcut_round_trip() {
        let specs = graf_menu_specs(true, true);
        for spec in &specs {
            if let Some(c) = spec.shortcut {
                assert!(
                    c.is_ascii_lowercase(),
                    "{:?} shortcut not lowercase",
                    spec.label
                );
            }
            assert!(graf_menu_item_from_label(spec.label).is_some());
        }
    }
}
